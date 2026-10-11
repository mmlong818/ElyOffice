//! A CEF page drawn by GPUI: frames in, events out.

use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use anyhow::{Result, bail};
use cef::{wrapper::message_router::BrowserSideCallback, *};
use ely_gpui_component::{
    buttons::{Button, ButtonVariant},
    feedback::ErrorView,
    primitives::IconName,
};
use gpui::{
    AnyElement, App, AppContext as _, Bounds, Context, CursorStyle, ElementInputHandler, Entity,
    EventEmitter, FocusHandle, Focusable, Hitbox, HitboxBehavior, InteractiveElement as _,
    IntoElement, ParentElement as _, Pixels, Render, RenderImage, Styled as _, Window, canvas, div,
};
use image::{Frame, ImageBuffer, Rgba};

use crate::{
    engine::Engine,
    surface::{Shared, Surface, file_url},
};

/// A page's answer to one `elyQuery`.
#[derive(Clone)]
pub struct Reply(Arc<Mutex<dyn BrowserSideCallback>>);

impl Reply {
    pub fn ok(&self, text: &str) {
        self.0.lock().expect("the reply lock").success_str(text);
    }

    pub fn fail(&self, message: &str) {
        self.0.lock().expect("the reply lock").failure(1, message);
    }
}

/// What the page tells its owner.
pub enum Event {
    Loaded,
    Query(String, Reply),
    /// A link to leave by; the owner decides.
    Leave(String),
    /// The page cannot show: load, start or run failed.
    Failed(String),
    /// A PDF print ended: where, and whether it worked.
    Printed(PathBuf, bool),
}

pub struct WebView {
    pub(crate) surface: Shared,
    image: Option<Arc<RenderImage>>,
    /// Replaced frames, freed from GPUI's atlas next paint.
    stale: Vec<Arc<RenderImage>>,
    pub(crate) focus: FocusHandle,
    pub(crate) bounds: Bounds<Pixels>,
    pub(crate) composing: Option<std::ops::Range<usize>>,
    /// The key whose text the input handler brings next.
    pub(crate) typing: Option<cef::KeyEvent>,
    url: String,
    /// Shown instead of the page until reloaded.
    fault: Option<String>,
    /// Takes a paste before the page; true when it did.
    pub(crate) paste: Option<Paste>,
}

/// Takes a paste before the page; true when it did.
pub(crate) type Paste = std::rc::Rc<dyn Fn(&mut Window, &mut App) -> bool>;

impl EventEmitter<Event> for WebView {}

impl Focusable for WebView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl WebView {
    /// Its frames, for a closing owner to free.
    pub fn retire_all(&mut self) -> ely_gpui_component::primitives::Retired {
        let mut images = ely_gpui_component::primitives::Retired::default();
        for image in self.image.take().into_iter().chain(self.stale.drain(..)) {
            images.push(image);
        }
        images
    }

    /// Opens `page`, a file in `root`, in a new browser.
    pub fn open(page: &Path, root: &Path, window: &mut Window, cx: &mut App) -> Result<Entity<Self>> {
        Self::opened(page, root, true, window, cx)
    }

    /// Opens `page` with its scripts off: shown, never run.
    pub fn open_inert(page: &Path, root: &Path, window: &mut Window, cx: &mut App) -> Result<Entity<Self>> {
        Self::opened(page, root, false, window, cx)
    }

    fn opened(
        page: &Path,
        root: &Path,
        scripts: bool,
        window: &mut Window,
        cx: &mut App,
    ) -> Result<Entity<Self>> {
        if !cx.has_global::<Engine>() {
            bail!("the web engine has not started");
        }
        let root = std::fs::canonicalize(root)?;
        let page = std::fs::canonicalize(page)?;
        let mut surface = Surface::new(root, window.scale_factor());
        surface.scripts = scripts;
        let surface = Arc::new(Mutex::new(surface));
        let url = file_url(&page);
        if !surface.lock().expect("the surface lock").navigable(&url) {
            bail!("{} is outside its content folder", page.display());
        }
        log::info!("web: a browser asked for {url}");
        crate::engine::ask_browser(surface.clone(), url.clone());
        let view = cx.new(|cx| WebView {
            surface,
            image: None,
            stale: Vec::new(),
            focus: cx.focus_handle(),
            bounds: Bounds::default(),
            composing: None,
            typing: None,
            url,
            fault: None,
            paste: None,
        });
        crate::input::follow_focus(&view, window, cx);
        view.update(cx, |_, cx| {
            cx.observe_global::<ely_gpui_component::theme::Theme>(|view, cx| view.theme(cx))
                .detach();
        });
        cx.global_mut::<Engine>().views.push(view.downgrade());
        Ok(view)
    }

    /// The browser, on the pump's next turn, outside any update.
    pub(crate) fn on_browser(&self, work: impl FnOnce(&Browser) + 'static) {
        let surface = self.surface.clone();
        crate::engine::later(move || {
            // Not held while CEF runs: its callbacks lock it too.
            let browser = surface.lock().expect("the surface lock").browser.clone();
            if let Some(browser) = browser {
                work(&browser);
            }
        });
    }

    pub(crate) fn on_host(&self, work: impl FnOnce(&BrowserHost) + 'static) {
        self.on_browser(move |browser| {
            if let Some(host) = browser.host() {
                work(&host);
            }
        });
    }

    /// Lets images under `dir` load; one folder at a time.
    pub fn allow_images(&self, dir: &Path) -> Result<()> {
        let dir = std::fs::canonicalize(dir)?;
        log::info!("web: images may come from {}", dir.display());
        self.surface.lock().expect("the surface lock").images = Some(dir);
        Ok(())
    }

    /// `take` sees pastes first; true keeps them from the page.
    pub fn intercept_paste(&mut self, take: impl Fn(&mut Window, &mut App) -> bool + 'static) {
        self.paste = Some(std::rc::Rc::new(take));
    }

    /// Runs script in the page's main frame.
    pub fn run_js(&self, code: &str) {
        let code = CefString::from(code);
        self.on_browser(move |browser| {
            if let Some(frame) = browser.main_frame() {
                frame.execute_java_script(Some(&code), None, 0);
            }
        });
    }

    /// The app's colors and fonts, into the page.
    fn theme(&mut self, cx: &mut Context<Self>) {
        self.run_js(&crate::tokens::script(&crate::tokens::tokens(cx)));
    }

    /// Zoom as a factor; CEF counts in 1.2 steps.
    pub fn set_zoom(&self, factor: f64) {
        let level = factor.ln() / 1.2f64.ln();
        self.on_host(move |host| host.set_zoom_level(level));
    }

    /// Takes what the handlers left; says if anything came.
    fn take(&mut self, cx: &mut Context<Self>) {
        let (frame, events) = {
            let mut surface = self.surface.lock().expect("the surface lock");
            let mut events = Vec::new();
            if std::mem::take(&mut surface.loaded) {
                events.push(Event::Loaded);
            }
            for (request, callback) in surface.queries.drain(..) {
                events.push(Event::Query(request, Reply(callback)));
            }
            events.extend(surface.leaving.drain(..).map(Event::Leave));
            events.extend(
                surface
                    .printed
                    .drain(..)
                    .map(|(path, ok)| Event::Printed(path, ok)),
            );
            events.extend(surface.failed.take().map(Event::Failed));
            if let Some(why) = crate::engine::failed().filter(|_| !surface.closed) {
                surface.closed = true;
                events.push(Event::Failed(why));
            }
            (surface.frame.take(), events)
        };
        if let Some((width, height, bgra)) = frame {
            match ImageBuffer::<Rgba<u8>, _>::from_raw(width, height, bgra) {
                Some(picture) => {
                    let image = Arc::new(RenderImage::new([Frame::new(picture)]));
                    self.stale.extend(self.image.replace(image));
                    cx.notify();
                }
                None => log::error!("web: a {width}×{height} frame was short"),
            }
        }
        for event in events {
            match &event {
                // Each load starts unthemed; the tokens go in first.
                Event::Loaded => {
                    self.fault = None;
                    self.theme(cx);
                }
                Event::Failed(why) => {
                    log::error!("web: {} failed: {why}", self.url);
                    self.fault = Some(why.clone());
                }
                _ => {}
            }
            cx.notify();
            cx.emit(event);
        }
    }

    /// Follows the room GPUI gave; CEF resizes to it.
    fn place(&mut self, bounds: Bounds<Pixels>, scale: f32) {
        self.bounds = bounds;
        let size = (f32::from(bounds.size.width), f32::from(bounds.size.height));
        let (resized, rescaled) = {
            let mut surface = self.surface.lock().expect("the surface lock");
            let changed = (surface.size != size, surface.scale != scale);
            (surface.size, surface.scale) = (size, scale);
            changed
        };
        if resized || rescaled {
            self.on_host(move |host| {
                if rescaled {
                    host.notify_screen_info_changed();
                }
                host.was_resized();
            });
        }
    }

    /// Tries the page again; a lost browser is made anew.
    pub fn reload(&mut self, cx: &mut Context<Self>) {
        self.fault = None;
        let made = self
            .surface
            .lock()
            .expect("the surface lock")
            .browser
            .is_some();
        match made {
            true => self.on_browser(|browser| browser.reload()),
            false => crate::engine::ask_browser(self.surface.clone(), self.url.clone()),
        }
        cx.notify();
    }

    /// The fault, with a way to try again.
    fn fault_view(&self, why: &str, cx: &mut Context<Self>) -> AnyElement {
        let this = cx.weak_entity();
        let title = ely_ui::i18n::text(cx, "web.failed", &[]);
        let again = Button::new("web-reload", ely_ui::i18n::text(cx, "web.reload", &[]))
            .variant(ButtonVariant::Primary)
            .on_click(move |_, window, cx| {
                // The page, not the gone button, takes keys.
                this.update(cx, |view, cx| {
                    view.reload(cx);
                    window.focus(&view.focus, cx);
                })
                .ok();
            });
        let view =
            ErrorView::new("web-failed", IconName::TriangleAlert, title).detail(why.to_string());
        // Without a running engine nothing can try again.
        match crate::engine::failed() {
            Some(_) => view.into_any_element(),
            None => view.action(again).into_any_element(),
        }
    }

    fn cursor(&self) -> CursorStyle {
        match self.surface.lock().expect("the surface lock").cursor {
            CursorType::HAND => CursorStyle::PointingHand,
            CursorType::IBEAM => CursorStyle::IBeam,
            CursorType::CROSS => CursorStyle::Crosshair,
            CursorType::EASTWESTRESIZE | CursorType::COLUMNRESIZE => CursorStyle::ResizeLeftRight,
            CursorType::NORTHSOUTHRESIZE | CursorType::ROWRESIZE => CursorStyle::ResizeUpDown,
            CursorType::NOTALLOWED => CursorStyle::OperationNotAllowed,
            _ => CursorStyle::Arrow,
        }
    }
}

impl Drop for WebView {
    fn drop(&mut self) {
        self.surface.lock().expect("the surface lock").closed = true;
        self.on_host(|host| host.close_browser(true as _));
    }
}

/// After each pump: every live view takes its news.
pub(crate) fn deliver(cx: &mut App) {
    let views = std::mem::take(&mut cx.global_mut::<Engine>().views);
    let alive: Vec<_> = views
        .into_iter()
        .filter(|view| view.update(cx, |view, cx| view.take(cx)).is_ok())
        .collect();
    cx.global_mut::<Engine>().views.extend(alive);
}

impl Render for WebView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if let Some(why) = self.fault.clone() {
            return div()
                .size_full()
                .child(self.fault_view(&why, cx))
                .into_any_element();
        }
        let this = cx.entity();
        let paint = this.clone();
        let element = canvas(
            move |bounds, window, cx| {
                let scale = window.scale_factor();
                this.update(cx, |view, _| view.place(bounds, scale));
                window.insert_hitbox(bounds, HitboxBehavior::Normal)
            },
            move |bounds, hitbox: Hitbox, window, cx| {
                let (image, stale, cursor, focus) = paint.update(cx, |view, _| {
                    let stale = std::mem::take(&mut view.stale);
                    (view.image.clone(), stale, view.cursor(), view.focus.clone())
                });
                for old in stale {
                    if let Err(error) = window.drop_image(old) {
                        log::error!("web: an old frame stayed: {error}");
                    }
                }
                window.set_cursor_style(cursor, &hitbox);
                window.handle_input(&focus, ElementInputHandler::new(bounds, paint.clone()), cx);
                let Some(image) = image else { return };
                let painted =
                    window.paint_image(bounds, bounds, Default::default(), image, 0, false);
                if let Err(error) = painted {
                    log::error!("web: a frame was not painted: {error}");
                }
            },
        )
        .size_full();
        crate::input::listen(div().size_full(), cx)
            .track_focus(&self.focus)
            .child(element)
            .into_any_element()
    }
}
