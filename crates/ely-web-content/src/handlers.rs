//! What CEF calls back: per process, and per browser.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex, OnceLock},
};

use cef::{
    wrapper::message_router::{
        BrowserSideCallback, BrowserSideHandler, BrowserSideRouter, MessageRouterBrowserSide,
        MessageRouterBrowserSideHandlerCallbacks, MessageRouterConfig, MessageRouterRendererSide,
        MessageRouterRendererSideHandlerCallbacks, RendererSideRouter,
    },
    *,
};

use crate::{fence::Requests, surface::Shared};

/// The page calls `window.elyQuery({request, onSuccess, onFailure})`.
fn config() -> MessageRouterConfig {
    MessageRouterConfig {
        js_query_function: "elyQuery".into(),
        js_cancel_function: "elyQueryCancel".into(),
        ..Default::default()
    }
}

/// Each browser's surface, by CEF's browser id.
fn surfaces() -> &'static Mutex<HashMap<i32, Shared>> {
    static SURFACES: OnceLock<Mutex<HashMap<i32, Shared>>> = OnceLock::new();
    SURFACES.get_or_init(Default::default)
}

pub(crate) fn adopt(browser: &Browser, surface: Shared) {
    surfaces()
        .lock()
        .expect("the surface lock")
        .insert(browser.identifier(), surface);
}

fn surface_of(browser: Option<&Browser>) -> Option<Shared> {
    let id = browser?.identifier();
    surfaces()
        .lock()
        .expect("the surface lock")
        .get(&id)
        .cloned()
}

/// Queries go to the view whose page asked.
struct Queries;

impl BrowserSideHandler for Queries {
    fn on_query_str(
        &self,
        browser: Option<Browser>,
        _frame: Option<Frame>,
        _query_id: i64,
        request: &str,
        _persistent: bool,
        callback: Arc<Mutex<dyn BrowserSideCallback>>,
    ) -> bool {
        let Some(surface) = surface_of(browser.as_ref()) else {
            return false;
        };
        surface
            .lock()
            .expect("the surface lock")
            .queries
            .push((request.to_string(), callback));
        true
    }
}

pub(crate) fn router() -> &'static Arc<BrowserSideRouter> {
    static ROUTER: OnceLock<Arc<BrowserSideRouter>> = OnceLock::new();
    ROUTER.get_or_init(|| {
        let router = BrowserSideRouter::new(config());
        router.add_handler(Arc::new(Queries), true);
        router
    })
}

fn renderer_router() -> &'static Arc<RendererSideRouter> {
    static ROUTER: OnceLock<Arc<RendererSideRouter>> = OnceLock::new();
    ROUTER.get_or_init(|| RendererSideRouter::new(config()))
}

/// WebRTC reaches the network by UDP; pages lose it.
const NO_PEERS: &str = "for (const name of ['RTCPeerConnection', 'webkitRTCPeerConnection', \
    'RTCDataChannel', 'RTCSessionDescription', 'RTCIceCandidate', 'RTCRtpSender', \
    'RTCRtpReceiver', 'RTCRtpTransceiver']) delete globalThis[name];";

wrap_app! {
    struct ElyApp;

    impl App {
        fn on_before_command_line_processing(
            &self,
            _process_type: Option<&CefString>,
            command_line: Option<&mut CommandLine>,
        ) {
            let Some(line) = command_line else { return };
            // No keychain prompt; the content keeps no secrets.
            line.append_switch(Some(&"use-mock-keychain".into()));
            line.append_switch(Some(&"disable-extensions".into()));
            // Offline pages: all traffic meets a dead proxy.
            line.append_switch_with_value(Some(&"proxy-server".into()), Some(&"127.0.0.1:9".into()));
            line.append_switch_with_value(
                Some(&"proxy-bypass-list".into()),
                Some(&"<-loopback>".into()),
            );
            // WebRTC sends UDP past proxies; it may use proxies only.
            line.append_switch_with_value(
                Some(&"force-webrtc-ip-handling-policy".into()),
                Some(&"disable_non_proxied_udp".into()),
            );
        }

        fn browser_process_handler(&self) -> Option<BrowserProcessHandler> {
            Some(Pump::new())
        }

        fn render_process_handler(&self) -> Option<RenderProcessHandler> {
            Some(Renderer::new())
        }
    }
}

pub(crate) fn app() -> cef::App {
    ElyApp::new()
}

wrap_browser_process_handler! {
    struct Pump;

    impl BrowserProcessHandler {
        fn on_schedule_message_pump_work(&self, delay_ms: i64) {
            crate::engine::schedule(delay_ms);
        }
    }
}

wrap_render_process_handler! {
    struct Renderer;

    impl RenderProcessHandler {
        fn on_context_created(
            &self,
            browser: Option<&mut Browser>,
            frame: Option<&mut Frame>,
            context: Option<&mut V8Context>,
        ) {
            // Before any page script: no peer connections at all.
            if let Some(context) = context.as_ref() {
                let code = CefString::from(NO_PEERS);
                let (mut value, mut thrown) = (None, None);
                if context.eval(Some(&code), None, 0, Some(&mut value), Some(&mut thrown)) != 1 {
                    eprintln!("web: WebRTC could not be removed from a page");
                }
            }
            renderer_router().on_context_created(
                browser.map(|b| b.clone()),
                frame.map(|f| f.clone()),
                context.map(|c| c.clone()),
            );
        }

        fn on_context_released(
            &self,
            browser: Option<&mut Browser>,
            frame: Option<&mut Frame>,
            context: Option<&mut V8Context>,
        ) {
            renderer_router().on_context_released(
                browser.map(|b| b.clone()),
                frame.map(|f| f.clone()),
                context.map(|c| c.clone()),
            );
        }

        fn on_process_message_received(
            &self,
            browser: Option<&mut Browser>,
            frame: Option<&mut Frame>,
            source_process: ProcessId,
            message: Option<&mut ProcessMessage>,
        ) -> ::std::os::raw::c_int {
            renderer_router().on_process_message_received(
                browser.map(|b| b.clone()),
                frame.map(|f| f.clone()),
                Some(source_process),
                message.map(|m| m.clone()),
            ) as _
        }
    }
}

wrap_render_handler! {
    struct Paint {
        surface: Shared,
    }

    impl RenderHandler {
        fn view_rect(&self, _browser: Option<&mut Browser>, rect: Option<&mut Rect>) {
            let Some(rect) = rect else { return };
            let surface = self.surface.lock().expect("the surface lock");
            // CEF needs a page at least one point square.
            rect.width = (surface.size.0.round() as i32).max(1);
            rect.height = (surface.size.1.round() as i32).max(1);
        }

        fn screen_info(
            &self,
            _browser: Option<&mut Browser>,
            screen_info: Option<&mut ScreenInfo>,
        ) -> ::std::os::raw::c_int {
            let Some(info) = screen_info else { return 0 };
            info.device_scale_factor = self.surface.lock().expect("the surface lock").scale;
            1
        }

        fn on_paint(
            &self,
            _browser: Option<&mut Browser>,
            type_: PaintElementType,
            _dirty_rects: Option<&[Rect]>,
            buffer: *const u8,
            width: ::std::os::raw::c_int,
            height: ::std::os::raw::c_int,
        ) {
            // Popups (select menus) are not drawn; pages avoid them.
            if type_ != PaintElementType::VIEW || buffer.is_null() || width <= 0 || height <= 0 {
                return;
            }
            let length = width as usize * height as usize * 4;
            let bgra = unsafe { std::slice::from_raw_parts(buffer, length) }.to_vec();
            self.surface.lock().expect("the surface lock").frame =
                Some((width as u32, height as u32, bgra));
        }

        fn on_ime_composition_range_changed(
            &self,
            _browser: Option<&mut Browser>,
            _selected_range: Option<&Range>,
            character_bounds: Option<&[Rect]>,
        ) {
            let bounds = character_bounds.unwrap_or_default();
            self.surface.lock().expect("the surface lock").ime = bounds
                .iter()
                .map(|r| (r.x as f32, r.y as f32, r.width as f32, r.height as f32))
                .collect();
        }
    }
}

wrap_display_handler! {
    struct Display {
        surface: Shared,
    }

    impl DisplayHandler {
        fn on_console_message(
            &self,
            _browser: Option<&mut Browser>,
            level: LogSeverity,
            _message: Option<&CefString>,
            _source: Option<&CefString>,
            line: ::std::os::raw::c_int,
        ) -> ::std::os::raw::c_int {
            // Words and sources a document may forge stay out.
            if level.get_raw() >= LogSeverity::WARNING.get_raw() {
                log::warn!("web: the page warned at line {line}");
            }
            0
        }

        fn on_cursor_change(
            &self,
            _browser: Option<&mut Browser>,
            _cursor: CursorHandle,
            type_: CursorType,
            _custom_cursor_info: Option<&CursorInfo>,
        ) -> ::std::os::raw::c_int {
            self.surface.lock().expect("the surface lock").cursor = type_;
            1
        }
    }
}

wrap_load_handler! {
    struct Load {
        surface: Shared,
    }

    impl LoadHandler {
        fn on_load_start(
            &self,
            _browser: Option<&mut Browser>,
            frame: Option<&mut Frame>,
            _transition_type: TransitionType,
        ) {
            if frame.is_some_and(|frame| frame.is_main() == 1) {
                self.surface.lock().expect("the surface lock").errored = false;
            }
        }

        fn on_load_error(
            &self,
            _browser: Option<&mut Browser>,
            frame: Option<&mut Frame>,
            error_code: Errorcode,
            error_text: Option<&CefString>,
            failed_url: Option<&CefString>,
        ) {
            // A stopped load is no failure; a fenced one is.
            if frame.is_none_or(|frame| frame.is_main() != 1) || error_code == Errorcode::ABORTED {
                return;
            }
            let text = error_text.map(CefString::to_string).unwrap_or_default();
            let url = failed_url.map(CefString::to_string).unwrap_or_default();
            let mut surface = self.surface.lock().expect("the surface lock");
            surface.errored = true;
            surface.failed = Some(format!("{text} ({}) {url}", error_code.get_raw()));
        }

        fn on_load_end(
            &self,
            _browser: Option<&mut Browser>,
            frame: Option<&mut Frame>,
            _http_status_code: ::std::os::raw::c_int,
        ) {
            if frame.is_some_and(|frame| frame.is_main() == 1) {
                let mut surface = self.surface.lock().expect("the surface lock");
                surface.loaded = !surface.errored;
            }
        }
    }
}

wrap_life_span_handler! {
    struct Life {
        surface: Shared,
    }

    impl LifeSpanHandler {
        fn on_after_created(&self, browser: Option<&mut Browser>) {
            if let Some(browser) = browser {
                log::info!("web: browser {} made", browser.identifier());
            }
        }

        fn on_before_close(&self, browser: Option<&mut Browser>) {
            let browser = browser.map(|b| b.clone());
            router().on_before_close(browser.clone());
            if let Some(browser) = browser {
                surfaces().lock().expect("the surface lock").remove(&browser.identifier());
            }
            // The browser holds these handlers; they hold the surface.
            self.surface.lock().expect("the surface lock").browser = None;
        }
    }
}

wrap_client! {
    struct Handlers {
        paint: RenderHandler,
        display: DisplayHandler,
        load: LoadHandler,
        life: LifeSpanHandler,
        requests: RequestHandler,
        keys: Option<KeyboardHandler>,
    }

    impl Client {
        fn keyboard_handler(&self) -> Option<KeyboardHandler> {
            self.keys.clone()
        }

        fn render_handler(&self) -> Option<RenderHandler> {
            Some(self.paint.clone())
        }

        fn display_handler(&self) -> Option<DisplayHandler> {
            Some(self.display.clone())
        }

        fn load_handler(&self) -> Option<LoadHandler> {
            Some(self.load.clone())
        }

        fn life_span_handler(&self) -> Option<LifeSpanHandler> {
            Some(self.life.clone())
        }

        fn request_handler(&self) -> Option<RequestHandler> {
            Some(self.requests.clone())
        }

        fn on_process_message_received(
            &self,
            browser: Option<&mut Browser>,
            frame: Option<&mut Frame>,
            source_process: ProcessId,
            message: Option<&mut ProcessMessage>,
        ) -> ::std::os::raw::c_int {
            router().on_process_message_received(
                browser.map(|b| b.clone()),
                frame.map(|f| f.clone()),
                source_process,
                message.map(|m| m.clone()),
            ) as _
        }
    }
}

/// A browser's handlers, all sharing its surface.
pub(crate) fn client(surface: &Shared) -> cef::Client {
    Handlers::new(
        Paint::new(surface.clone()),
        Display::new(surface.clone()),
        Load::new(surface.clone()),
        Life::new(surface.clone()),
        Requests::new(surface.clone()),
        keys(),
    )
}

#[cfg(target_os = "macos")]
fn keys() -> Option<KeyboardHandler> {
    Some(crate::edit::Keys::new())
}

/// Elsewhere Chromium binds these keys itself.
#[cfg(not(target_os = "macos"))]
fn keys() -> Option<KeyboardHandler> {
    None
}


#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex, OnceLock};

    use cef::{CefString, ImplDisplayHandler, LogSeverity};

    struct Kept;

    fn kept() -> &'static Mutex<Vec<String>> {
        static KEPT: OnceLock<Mutex<Vec<String>>> = OnceLock::new();
        KEPT.get_or_init(Mutex::default)
    }

    impl log::Log for Kept {
        fn enabled(&self, _: &log::Metadata) -> bool {
            true
        }

        fn log(&self, record: &log::Record) {
            kept().lock().expect("the kept lock").push(record.args().to_string());
        }

        fn flush(&self) {}
    }

    #[test]
    fn a_pages_words_and_sources_never_reach_the_log() {
        log::set_boxed_logger(Box::new(Kept)).expect("the only logger");
        log::set_max_level(log::LevelFilter::Trace);
        let surface = crate::surface::Surface::new(std::env::temp_dir(), 1.0);
        let display = super::Display::new(Arc::new(Mutex::new(surface)));
        let said = CefString::from("ely: M11_SECRET");
        let at = CefString::from("file:///content/M11_SECRET.js");
        for level in [LogSeverity::WARNING, LogSeverity::ERROR, LogSeverity::INFO] {
            display.on_console_message(None, level, Some(&said), Some(&at), 7);
        }
        let records = kept().lock().expect("the kept lock").clone();
        let warned = records.iter().filter(|said| *said == "web: the page warned at line 7");
        assert_eq!(warned.count(), 2, "warnings and errors stay seen: {records:#?}");
        assert!(!records.iter().any(|said| said.contains("M11_SECRET")), "{records:#?}");
    }
}
