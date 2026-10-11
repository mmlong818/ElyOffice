//! Markdown files in the shared shell (§14).

use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use ely_gpui_component::{
    buttons::SegmentedControl,
    feedback::EmptyState,
    forms::TextInput,
    primitives::IconName,
    shell::{StatusBar, StatusBarItem},
    theme::ControlSize,
};
use ely_office::{Core, Host, Kind, prelude::*};
use ely_platform::{AppId, startup};
use ely_ui::{Root, Shell, commands, i18n::text};
use gpui::{
    App, AppContext as _, Context, Entity, InteractiveElement as _, IntoElement,
    ParentElement as _, PathPromptOptions, Render, Styled as _, Window, div,
};

use crate::engine::{Mark, Pages};
use crate::keys::{
    CheckDisk, ColumnLeft, ColumnRight, DeleteColumn, DeleteRow, InsertTable, PreviewMode,
    RowAbove, RowBelow, SourceMode, VisualMode,
};
use crate::page::Mode;
use crate::view::MarkView;

pub struct Desk {
    pub core: Core<Desk>,
    /// The tab being renamed and its name field.
    pub naming: Option<(u64, Entity<TextInput>)>,
    pub folder: Option<crate::folder::Folder>,
    /// Folder listings so far; searches redo on a new one.
    pub listings: u64,
    /// Files and folders opened lately, newest first.
    pub recent: Vec<PathBuf>,
    pub recent_file: PathBuf,
    /// A file asked to open at a place.
    pub going: Option<(PathBuf, serde_json::Value)>,
    /// The path each tab last put in the recent list.
    pub remembered: HashMap<u64, PathBuf>,
    side: Entity<crate::side::Side>,
    pub finder: Option<crate::finding::Finder>,
    pub watcher: Option<notify::RecommendedWatcher>,
    /// The folders the watcher covers.
    pub watched: Vec<PathBuf>,
    pub comparing: Option<crate::compare::Comparing>,
    pub keeping: crate::restore::Keeping,
    /// The export dialog's choice, while it is open.
    pub output: Option<crate::export::Output>,
    /// Pages printing to PDF, by job; several may run.
    pub printing: HashMap<u64, crate::export::Printing>,
    pub print_jobs: u64,
    pub look: crate::look::Look,
    pub look_file: PathBuf,
    /// Git's word on the shown file, and which tab asked.
    pub tracked: Option<crate::vcs::Tracked>,
    asked: Option<u64>,
    /// The shown file's fingerprint when Git was last asked.
    asked_print: Option<ely_files::Fingerprint>,
}

impl Host for Desk {
    type Extra = crate::outside::Extra;
    type Engine = Mark;
    const KIND: Kind = Kind {
        app: AppId::Mark,
        kept: &["md", "markdown"],
        save_as: &["md"],
        macros: None,
    };

    fn core(&self) -> &Core<Self> {
        &self.core
    }

    fn core_mut(&mut self) -> &mut Core<Self> {
        &mut self.core
    }

    /// A quit records the open tabs first.
    fn quitting(&mut self, cx: &mut Context<Self>) {
        self.leave(cx);
    }

    /// A quit or close stopped: the kept list follows again.
    fn close_cancelled(&mut self, cx: &mut Context<Self>) {
        self.keeping.leaving = false;
        self.keep(cx);
    }

    /// An autosave met an outside change: compare, never guess.
    fn conflicted(&mut self, id: u64, cx: &mut Context<Self>) -> bool {
        log::info!("mark: a save of tab {id} met an outside change");
        if let Some(tab) = self.tab_mut(id) {
            tab.extra.policy.external_modified();
        }
        self.look(id, cx);
        true
    }

    /// A closed tab's unsaved text is gone for good (§6.2).
    fn tab_closed(&mut self, tab: &ely_office::Tab<Self::Extra, Mark>, cx: &mut Context<Self>) {
        let path = tab.session.path().map(std::path::Path::to_path_buf);
        self.drop_slot(&tab.extra.slot, path, tab.name.to_string(), cx);
    }

    /// Save As or rename moves where images come from.
    fn observed(&mut self, id: u64, view: &Entity<MarkView>, cx: &mut Context<Self>) {
        self.watch_folders(cx);
        let look = serde_json::to_value(&self.look).expect("a look serializes");
        view.update(cx, |view, cx| view.set_look(look, cx));
        let changed = view.read(cx).changed();
        self.autosave(id, changed, cx);
        let recheck = self.tab_mut(id).is_some_and(|tab| {
            if !tab.dirty(cx) {
                tab.extra.base = tab.doc.lock().expect("the text lock").current.clone();
            }
            tab.extra.recheck
        });
        if recheck {
            self.look(id, cx);
        }
        if view.read(cx).shown {
            self.restore_shown(id, cx);
        }
        // A save changes what Git sees.
        let print = self.active_tab().and_then(|tab| tab.session.fingerprint());
        if print != self.asked_print {
            self.asked_print = print;
            self.ask_git(cx);
        }
        let path = self
            .tab(id)
            .and_then(|tab| tab.session.path().map(PathBuf::from));
        if let Some(path) = path {
            view.update(cx, |view, _| view.attachable = true);
            if let Some(folder) = path.parent() {
                view.update(cx, |view, cx| view.moved(folder.to_path_buf(), cx));
            }
            self.track_recent(id, &path, cx);
            let real = std::fs::canonicalize(&path).ok();
            if self
                .going
                .as_ref()
                .is_some_and(|(file, _)| Some(file) == real.as_ref())
                && let Some((_, place)) = self.going.take()
            {
                view.update(cx, |view, cx| view.reveal(place, cx));
            }
        }
        cx.notify();
    }
}

/// The window root; files given at launch open.
pub fn open(app: AppId, window: &mut Window, cx: &mut App) -> Entity<Root> {
    let fail = |error: String| -> ! { startup::fail(app, &error) };
    let data = ely_platform::data_dir(app).unwrap_or_else(|error| fail(format!("{error:#}")));
    if let Err(error) = ely_web_content::start(&data, cx) {
        fail(format!("{error:#}"));
    }
    let dir = crate::page::content().unwrap_or_else(|error| fail(format!("{error:#}")));
    let work = ely_office::fresh_work(&data).unwrap_or_else(|error| fail(format!("{error}")));
    crate::keys::bind(cx);
    let pages = Arc::new(Mutex::new(Pages { dir }));
    let recent_file = data.join("recent.json");
    let look_file = data.join("look.json");
    // An unreadable look is reported; the default shows.
    let (look, look_unread) = match crate::look::load(&look_file) {
        Ok(look) => (look, None),
        Err(error) => (crate::look::Look::default(), Some(error)),
    };
    let (keeping, kept, missing) =
        crate::restore::Keeping::start(&data).unwrap_or_else(|error| fail(format!("{error}")));
    // An unreadable list is reported, never overwritten.
    let (recent, unread) = match ely_files::recent::recent(&recent_file) {
        Ok(list) => (list, None),
        Err(error) => (Vec::new(), Some(error)),
    };
    let desk = cx.new(|cx| {
        let this = cx.entity();
        let side = cx.new(|cx| crate::side::Side::new(&this, window, cx));
        Desk {
            core: Core::new(pages, work, cx),
            naming: None,
            folder: None,
            listings: 0,
            recent,
            recent_file,
            going: None,
            remembered: HashMap::new(),
            side,
            finder: None,
            watcher: None,
            watched: Vec::new(),
            comparing: None,
            keeping,
            output: None,
            printing: Default::default(),
            print_jobs: 0,
            look: look.clone(),
            look_file,
            tracked: None,
            asked: None,
            asked_print: None,
        }
    });
    let menus = crate::keys::menus(&look, cx);
    let root = ely_ui::document_root(
        app,
        desk.clone().into(),
        &commands::RENAMING,
        menus,
        window,
        cx,
    );
    let asked = ely_ui::launch_files(cx);
    desk.update(cx, |desk, cx| {
        desk.core.root = root.downgrade();
        desk.own_window(window, cx);
        window.focus(&desk.core.focus, cx);
        // Launch files stand alone; unsaved text still returns.
        if asked.is_empty() {
            for path in kept {
                desk.open_path(path, cx);
            }
        }
        for (at, name) in desk.keeping.untitled() {
            // Tab numbers come from `next` as the load starts.
            let id = desk.core.next;
            desk.new_from_text(&name, "", cx);
            if desk.core.next > id {
                desk.keeping.made(at, id);
            }
        }
        for path in asked {
            desk.open_path(path, cx);
        }
        desk.keep_every(cx);
        cx.on_release(|desk, _| desk.keeping.close()).detach();
        // Another shown tab asks Git again.
        cx.observe_self(|desk, cx| {
            let shown = desk.active_tab().map(|tab| tab.id);
            if shown != desk.asked {
                desk.asked = shown;
                desk.tracked = None;
                desk.ask_git(cx);
            }
        })
        .detach();
        cx.on_app_quit(|desk, cx| {
            // A system end skips asking: unsaved tabs keep points.
            if desk.core.tabs.iter().any(|tab| tab.dirty(cx)) {
                desk.keeping.hold();
            } else {
                desk.keeping.close();
            }
            async {}
        })
        .detach();
        for gone in missing {
            let said = text(cx, "mark.restore.missing", &[("file", &gone)]);
            desk.toast(said, ely_gpui_component::primitives::Severity::Warning, cx);
        }
        if let Some(error) = look_unread {
            log::error!("mark: the look did not read: {error}");
            let said = text(cx, "office.error.engine", &[("error", &error.to_string())]);
            desk.toast(said, ely_gpui_component::primitives::Severity::Danger, cx);
        }
        if let Some(error) = unread {
            log::error!("mark: the recent list did not read: {error}");
            let said = text(cx, "office.error.engine", &[("error", &error.to_string())]);
            desk.toast(said, ely_gpui_component::primitives::Severity::Danger, cx);
        }
        // Back in front: the folder may have changed.
        cx.observe_window_activation(window, |desk, window, cx| {
            if window.is_window_active() {
                desk.relist(cx);
                desk.ask_git(cx);
            }
        })
        .detach();
    });
    let opening = desk.clone();
    ely_ui::on_open(cx, move |path, cx| {
        opening.update(cx, |desk, cx| desk.open_path(path, cx))
    });
    let closing = desk.clone();
    window.on_window_should_close(cx, move |window, cx| {
        closing.update(cx, |desk, cx| {
            desk.leave(cx);
            desk.should_close(window, cx)
        })
    });
    root
}

impl Desk {
    pub fn active_view(&self) -> Option<Entity<MarkView>> {
        self.active_tab().map(|tab| tab.view.clone())
    }

    /// Asks for files; each opens in its own tab.
    fn open_picked(&mut self, _: &commands::Open, _: &mut Window, cx: &mut Context<Self>) {
        let picked = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: true,
            multiple: true,
            prompt: None,
        });
        cx.spawn(async move |this, cx| {
            let picked = match picked.await {
                Ok(Ok(paths)) => Ok(paths),
                Ok(Err(error)) => Err(error.to_string()),
                Err(error) => Err(error.to_string()),
            };
            let done = this.update(cx, |desk, cx| match picked {
                Ok(Some(paths)) => paths.into_iter().for_each(|path| desk.open_path(path, cx)),
                Ok(None) => log::info!("mark: the open panel was cancelled"),
                Err(error) => desk.panel_failed(&error, cx),
            });
            if done.is_err() {
                log::info!("mark: the window closed before the panel");
            }
        })
        .detach();
    }

    /// An empty untitled file (MARK-01).
    fn on_new(&mut self, _: &commands::New, _: &mut Window, cx: &mut Context<Self>) {
        self.new_from_text("untitled.md", "", cx);
    }

    fn table(&mut self, op: &str, cx: &mut Context<Self>) {
        if let Some(view) = self.active_view() {
            view.update(cx, |view, cx| view.table(op, cx));
        }
    }

    fn switch(&mut self, mode: Mode, cx: &mut Context<Self>) {
        if let Some(view) = self.active_view() {
            view.update(cx, |view, cx| view.switch(mode, cx));
        }
    }

    fn status(&self, cx: &mut Context<Self>) -> StatusBar {
        let state = StatusBarItem::new("mark-state").label(self.save_state(cx));
        let Some(view) = self.active_view() else {
            return StatusBar::new().left(state);
        };
        let git = self.git_item(cx);
        let this = cx.entity();
        let modes = Mode::ALL.into_iter().fold(
            SegmentedControl::new("mark-mode", view.read(cx).mode.name()).size(ControlSize::Sm),
            |control, mode| {
                let icon = match mode {
                    Mode::Visual => IconName::PenLine,
                    Mode::Source => IconName::Code,
                    Mode::Preview => IconName::Eye,
                };
                control.segment(mode.name(), crate::keys::label(mode, cx), Some(icon))
            },
        );
        let modes = modes.on_change(move |key, _, cx| {
            if let Some(mode) = Mode::parse(key) {
                this.update(cx, |desk, cx| desk.switch(mode, cx));
            }
        });
        let bar = StatusBar::new().left(state);
        let bar = match git {
            Some(git) => bar.left(git),
            None => bar,
        };
        bar.right(modes)
    }
}

impl Render for Desk {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let tab = self.active_tab();
        let title = tab.map_or_else(|| AppId::Mark.name().into(), |tab| tab.name.clone());
        let body = match tab {
            Some(tab) => div()
                .size_full()
                .flex()
                .flex_col()
                .children(self.outside_banner(cx))
                .children(large_banner(&tab.view, cx))
                .child(div().flex_1().min_h_0().child(tab.view.clone()))
                .into_any_element(),
            None => {
                let none = text(cx, "command.blocked.no_document", &[]);
                EmptyState::new("mark-empty", IconName::File, none).into_any_element()
            }
        };
        // Focus mode folds tabs, side and status away.
        let shell = match self.look.focus {
            true => Shell::new(title, &self.core.focus),
            false => Shell::new(title, &self.core.focus)
                .tabs(self.tab_strip(cx))
                .left(Some(self.side.clone().into()))
                .status(self.status(cx)),
        }
        .child(body);
        let root = div()
            .size_full()
            .key_context(commands::CONTEXT)
            .track_focus(&self.core.focus)
            .on_action(cx.listener(Self::open_picked))
            .on_action(cx.listener(Self::on_new))
            .on_action(cx.listener(Self::on_save))
            .on_action(cx.listener(Self::on_save_as))
            .on_action(cx.listener(Self::on_rename))
            .on_action(cx.listener(Self::on_find))
            .on_action(cx.listener(Self::on_export))
            .on_action(cx.listener(Self::on_print))
            .on_action(cx.listener(Self::on_close_tab))
            .on_action(cx.listener(|desk, _: &InsertTable, _, cx| desk.table("insert", cx)))
            .on_action(cx.listener(|desk, _: &RowAbove, _, cx| desk.table("row-above", cx)))
            .on_action(cx.listener(|desk, _: &RowBelow, _, cx| desk.table("row-below", cx)))
            .on_action(cx.listener(|desk, _: &ColumnLeft, _, cx| desk.table("column-left", cx)))
            .on_action(cx.listener(|desk, _: &ColumnRight, _, cx| desk.table("column-right", cx)))
            .on_action(cx.listener(|desk, _: &DeleteRow, _, cx| desk.table("delete-row", cx)))
            .on_action(cx.listener(|desk, _: &DeleteColumn, _, cx| desk.table("delete-column", cx)))
            .on_action(cx.listener(|desk, _: &CheckDisk, _, cx| desk.look_all(cx)))
            .on_action(cx.listener(|desk, _: &VisualMode, _, cx| desk.switch(Mode::Visual, cx)))
            .on_action(cx.listener(|desk, _: &SourceMode, _, cx| desk.switch(Mode::Source, cx)))
            .on_action(cx.listener(|desk, _: &PreviewMode, _, cx| desk.switch(Mode::Preview, cx)))
            .child(shell)
            .children(self.core_dialog(cx))
            .children(self.naming_element(cx))
            .children(self.find_bar(cx))
            .children(self.compare_dialog(cx))
            .children(self.export_dialog(cx));
        Self::look_actions(root, cx)
    }
}

/// A large file says why it shows as source.
fn large_banner(
    view: &Entity<MarkView>,
    cx: &mut Context<Desk>,
) -> Option<ely_gpui_component::feedback::Banner> {
    if !view.read(cx).large {
        return None;
    }
    let size = view.read(cx).bytes() as f64 / (1024.0 * 1024.0);
    let said = text(cx, "mark.large", &[("size", &format!("{size:.1}"))]);
    Some(ely_gpui_component::feedback::Banner::new(
        "mark-large",
        ely_gpui_component::primitives::Severity::Info,
        said,
    ))
}
