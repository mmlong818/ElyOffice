//! One Markdown file in a page, its text returned.

use ely_web_content::{Event, Reply, WebView};
use gpui::{
    Context, Entity, EventEmitter, ExternalPaths, FocusHandle, Focusable as _,
    InteractiveElement as _, IntoElement, ParentElement as _, Render, Styled as _, Window, div,
};
use serde_json::Value;

use std::path::PathBuf;

use crate::engine::{Doc, mark};
use crate::page::Mode;

/// The page reported a change or new text.
pub struct Reported;

/// One heading of the document's outline.
#[derive(Clone, Debug, PartialEq)]
pub struct Heading {
    pub level: usize,
    pub title: String,
}
/// Where the reader was: a file line and the scroll.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Place {
    pub line: u64,
    /// Sessions from before the column was kept read zero.
    #[serde(default)]
    pub column: u64,
    pub scroll: f64,
}

/// Text past this many bytes opens as source (PERF-07).
pub const LARGE: usize = 2 * 1024 * 1024;

/// A render's answer: the page's reply, or why not.
type Rendered = Box<dyn FnOnce(Result<Value, String>, &mut gpui::App)>;

/// Renders the page will never answer end with `why`.
fn end_renders(pending: Vec<Rendered>, why: &str, cx: &mut gpui::App) {
    if pending.is_empty() {
        return;
    }
    log::warn!("mark: {} renders end: {why}", pending.len());
    let why = why.to_string();
    cx.defer(move |cx| {
        for done in pending {
            done(Err(why.clone()), cx);
        }
    });
}

/// The page failed; the shell shows why.
pub struct Failed(pub String);

pub struct MarkView {
    pub(crate) web: Entity<WebView>,
    doc: Doc,
    /// Page loads, changes and owed renders.
    tally: crate::tally::Tally<Rendered>,
    /// The mark at the last load or save.
    saved: String,
    pub mode: Mode,
    /// Headings as the shown editor reads them.
    pub outline: Vec<Heading>,
    /// Shown since the last page load.
    pub shown: bool,
    /// A place to bring into view once shown.
    waiting: Option<Value>,
    /// Where the page's search stands, while one runs.
    pub found: Option<crate::finding::Found>,
    /// The file's folder: links and images resolve here.
    pub(crate) folder: PathBuf,
    /// A saved file: images can go beside it.
    pub attachable: bool,
    /// Past `LARGE`: source alone, every byte still saved (MARK-14).
    pub large: bool,
    /// The caret's line and the scroll, as last reported.
    pub place: Option<Place>,
    /// The writing view, as the page applies it (MARK-09).
    look: Value,
    pub focus: FocusHandle,
}

impl EventEmitter<Reported> for MarkView {}
impl EventEmitter<Failed> for MarkView {}

impl MarkView {
    pub fn new(
        web: Entity<WebView>,
        doc: Doc,
        saved: String,
        folder: PathBuf,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus = web.read(cx).focus_handle(cx);
        let large = doc.lock().expect("the text lock").current.len() > LARGE;
        cx.subscribe(&web, Self::heard).detach();
        let paste = crate::attach::paste(cx.weak_entity());
        web.update(cx, |web, _| web.intercept_paste(paste));
        MarkView {
            web,
            doc,
            tally: crate::tally::Tally::new(),
            saved,
            mode: if large { Mode::Source } else { Mode::Visual },
            large,
            outline: Vec::new(),
            shown: false,
            waiting: None,
            found: None,
            attachable: false,
            place: None,
            look: Value::Null,
            folder,
            focus,
        }
    }

    /// Every change's text has arrived.
    pub fn settled(&self) -> bool {
        self.tally.settled()
    }

    /// The newest change the page announced.
    pub fn changed(&self) -> u64 {
        self.tally.changed()
    }

    /// The text's size now, in bytes.
    pub fn bytes(&self) -> usize {
        self.doc.lock().expect("the text lock").current.len()
    }

    pub fn modified(&self) -> bool {
        mark(&self.doc.lock().expect("the text lock").current) != self.saved
    }

    pub fn saved(&mut self, mark: &str, cx: &mut Context<Self>) {
        self.saved = mark.to_owned();
        cx.emit(Reported);
    }

    /// The same text in `mode` (MARK-02).
    pub fn switch(&mut self, mode: Mode, cx: &mut Context<Self>) {
        if self.large && mode != Mode::Source {
            log::info!("mark: a large file stays in source");
            let said = ely_ui::i18n::text(cx, "mark.large.modes", &[]).to_string();
            return cx.emit(Failed(said));
        }
        log::info!("mark: switches to {}", mode.name());
        self.mode = mode;
        crate::page::switch(self.web.read(cx), mode);
        cx.notify();
    }

    /// Searches the page for `spec` (MARK-06).
    pub fn find(&mut self, spec: Value, cx: &mut Context<Self>) {
        crate::page::call(self.web.read(cx), "find", &spec);
    }

    /// Steps under `spec`, the box's own; tabs keep older ones.
    pub fn step(&mut self, spec: Value, forward: bool, cx: &mut Context<Self>) {
        let ask = serde_json::json!({ "spec": spec, "forward": forward });
        crate::page::call(self.web.read(cx), "step", &ask);
    }

    pub fn replace(&mut self, spec: Value, all: bool, cx: &mut Context<Self>) {
        let ask = serde_json::json!({ "spec": spec, "all": all });
        crate::page::call(self.web.read(cx), "replace", &ask);
    }

    /// A table step: insert, rows and columns (MARK-06).
    pub fn table(&mut self, op: &str, cx: &mut Context<Self>) {
        crate::page::call(self.web.read(cx), "table", &Value::from(op));
    }

    pub fn unfind(&mut self, cx: &mut Context<Self>) {
        self.found = None;
        crate::page::call(self.web.read(cx), "unfind", &Value::Null);
    }

    /// Brings `{"heading": n}` or `{"line": n}` into view.
    pub fn reveal(&mut self, place: Value, cx: &mut Context<Self>) {
        match self.shown {
            true => crate::page::call(self.web.read(cx), "reveal", &place),
            false => self.waiting = Some(place),
        }
    }

    /// The disk's text replaces a clean one; the place stays.
    pub fn refresh(&mut self, source: crate::text::Source, cx: &mut Context<Self>) {
        let text = source.text.clone();
        {
            let mut doc = self.doc.lock().expect("the text lock");
            doc.current = text.clone();
            doc.source = source;
        }
        self.saved = mark(&text);
        // Reports and renders from before belong to the old text.
        let lapsed = self.tally.restart();
        end_renders(lapsed, "the text was replaced from disk", cx);
        crate::page::refresh(self.web.read(cx), &text, self.tally.load());
        cx.emit(Reported);
    }

    /// The writing view's settings, applied now and at each show.
    pub fn set_look(&mut self, look: Value, cx: &mut Context<Self>) {
        if look == self.look {
            return;
        }
        if self.shown {
            crate::page::call(self.web.read(cx), "look", &look);
        }
        self.look = look;
    }

    /// The whole text as `kind`; `done` takes it.
    pub fn render(
        &mut self,
        kind: &str,
        done: impl FnOnce(Result<Value, String>, &mut gpui::App) + 'static,
        cx: &mut Context<Self>,
    ) {
        match self.tally.ask(Box::new(done)) {
            Ok(ask) => {
                let ask = serde_json::json!({ "kind": kind, "ask": ask });
                crate::page::call(self.web.read(cx), "render", &ask);
            }
            Err((done, why)) => end_renders(vec![done], &why, cx),
        }
    }

    /// `text` replaces the editor's, as one undoable edit.
    pub fn adopt(&mut self, text: String, cx: &mut Context<Self>) {
        crate::page::call(self.web.read(cx), "adopt", &Value::from(text));
    }

    /// Shows again as `mode` at `place`; recovered text unsaved.
    pub fn reopen(
        &mut self,
        mode: Mode,
        place: Option<Place>,
        text: Option<String>,
        cx: &mut Context<Self>,
    ) {
        let mut doc = self.doc.lock().expect("the text lock");
        if let Some(text) = text {
            doc.current = text;
        }
        let current = doc.current.clone();
        drop(doc);
        let mode = if self.large { Mode::Source } else { mode };
        self.mode = mode;
        let at = (mode, self.tally.load(), place);
        crate::page::open(self.web.read(cx), &current, at, &self.folder, cx);
        cx.emit(Reported);
    }

    /// The file moved; images and links follow it.
    pub fn moved(&mut self, folder: PathBuf, cx: &mut Context<Self>) {
        if folder == self.folder {
            return;
        }
        if let Err(error) = self.web.read(cx).allow_images(&folder) {
            log::error!("mark: images cannot follow the file: {error:#}");
            return cx.emit(Failed(format!("{error:#}")));
        }
        crate::page::folder(self.web.read(cx), &folder);
        self.folder = folder;
    }

    fn heard(&mut self, web: Entity<WebView>, event: &Event, cx: &mut Context<Self>) {
        match event {
            // A reload shows the last text that arrived.
            Event::Loaded => {
                let lapsed = self.tally.loaded();
                end_renders(lapsed, "the page reloaded", cx);
                self.shown = false;
                let text = self.doc.lock().expect("the text lock").current.clone();
                let at = (self.mode, self.tally.load(), self.place);
                crate::page::open(web.read(cx), &text, at, &self.folder, cx);
                cx.emit(Reported);
            }
            Event::Query(said, reply) => self.query(said, reply, cx),
            Event::Leave(url) => self.follow(url, cx),
            Event::Failed(why) => {
                let lapsed = self.tally.fail(why.clone());
                end_renders(lapsed, why, cx);
                cx.emit(Failed(why.clone()));
            }
            Event::Printed(path, _) => {
                log::error!("mark: an editor page printed {}", path.display())
            }
        }
    }

    /// A clicked link: web and mail open outside.
    fn follow(&mut self, url: &str, cx: &mut Context<Self>) {
        if ely_office::outward(url) {
            log::info!("mark: opens a link outside");
            return cx.open_url(url);
        }
        log::info!("mark: a link of another kind was refused");
        let said = ely_ui::i18n::text(cx, "office.link.refused", &[]).to_string();
        cx.emit(Failed(said));
    }

    fn query(&mut self, said: &str, reply: &Reply, cx: &mut Context<Self>) {
        let said: Value = match serde_json::from_str(said) {
            Ok(said) => said,
            Err(error) => {
                log::error!("mark: the page sent bad JSON: {error}");
                return reply.fail("bad JSON");
            }
        };
        if !self.tally.current(said["generation"].as_u64()) {
            log::info!("mark: a report from an earlier page load was dropped");
            return reply.ok("");
        }
        let version = said["version"].as_u64();
        match (said["kind"].as_str(), version, said["text"].as_str()) {
            // A fallback shows source; the tab follows.
            (Some("shown"), ..) => {
                log::info!(
                    "mark: shown as {}, fallback {}",
                    said["mode"],
                    said["fallback"]
                );
                if let Some(mode) = said["mode"].as_str().and_then(Mode::parse) {
                    self.mode = mode;
                    cx.notify();
                }
                if let Some(what) = said["broke"].as_str() {
                    log::error!("mark: the visual editor broke ({what}); source shows");
                    cx.emit(Failed(ely_ui::i18n::text(cx, "mark.visual.broke", &[("what", what)]).to_string()));
                }
                self.shown = true;
                if let Some(place) = self.waiting.take() {
                    crate::page::call(self.web.read(cx), "reveal", &place);
                }
                if !self.look.is_null() {
                    crate::page::call(self.web.read(cx), "look", &self.look);
                }
                cx.emit(Reported);
            }
            (Some("failed"), ..) => {
                let Some(what) = said["what"].as_str() else {
                    log::error!("mark: the page sent a failure without its name");
                    return reply.fail("bad failure");
                };
                log::error!("mark: the page built no editor ({what})");
                cx.emit(Failed(ely_ui::i18n::text(cx, "mark.page.broke", &[("what", what)]).to_string()));
            }
            (Some("missed"), ..) => match said["line"].as_u64() {
                Some(line) => log::warn!("mark: nothing to reveal at line {line}"),
                None => log::warn!("mark: no such heading to reveal"),
            },
            (Some("outline"), ..) => match crate::page::headings(&said["items"]) {
                Some(outline) => {
                    self.outline = outline;
                    cx.notify();
                }
                None => {
                    log::error!("mark: the page sent a bad outline");
                    return reply.fail("bad outline");
                }
            },
            (Some("found"), ..) => match found(&said) {
                Some(found) => {
                    self.found = Some(found);
                    cx.notify();
                }
                None => {
                    log::error!("mark: the page sent a bad search state");
                    return reply.fail("bad search state");
                }
            },
            (Some("link"), ..) => match said["href"].as_str() {
                Some(url) => self.follow(url, cx),
                None => {
                    log::error!("mark: the page sent a link without its address");
                    return reply.fail("bad link");
                }
            },
            (Some("table"), ..) if said["done"] == false => {
                let key = match self.mode {
                    Mode::Visual => "mark.table.none",
                    _ => "mark.table.visual",
                };
                cx.emit(Failed(ely_ui::i18n::text(cx, key, &[]).to_string()));
            }
            (Some("inserted"), ..) if said["done"] == false => {
                cx.emit(Failed(
                    ely_ui::i18n::text(cx, "mark.attach.preview", &[]).to_string(),
                ));
            }
            (Some("table" | "inserted"), ..) => {}
            (Some("rendered"), ..) => {
                let ask = said["ask"].as_u64().unwrap_or_default();
                let Some(done) = self.tally.answered(ask) else {
                    log::error!("mark: a render nobody asked for came back");
                    return reply.fail("unasked render");
                };
                let result = match said["error"].as_str() {
                    Some(error) => Err(error.to_owned()),
                    None => Ok(said.clone()),
                };
                done(result, cx);
            }
            (Some("place"), ..) => match (
                said["line"].as_u64(),
                said["column"].as_u64(),
                said["scroll"].as_f64(),
            ) {
                (Some(line), Some(column), Some(scroll)) => {
                    self.place = Some(Place {
                        line,
                        column,
                        scroll,
                    })
                }
                _ => {
                    log::error!("mark: the page sent a bad place");
                    return reply.fail("bad place");
                }
            },
            (Some("changed"), Some(version), _) => {
                self.tally.change(version);
                cx.emit(Reported);
            }
            // Only the newest text counts.
            (Some("text"), Some(version), Some(text)) => {
                if !self.tally.text(version) {
                    log::info!("mark: an older text was dropped");
                } else {
                    self.doc.lock().expect("the text lock").current = text.to_owned();
                    cx.emit(Reported);
                }
            }
            _ => {
                log::error!("mark: the page asked something unknown");
                return reply.fail("unknown request");
            }
        }
        reply.ok("");
    }
}

/// A search state from the page; anything malformed is None.
fn found(said: &Value) -> Option<crate::finding::Found> {
    if let Some(error) = said["error"].as_str() {
        let error = Some(error.to_owned());
        return Some(crate::finding::Found {
            current: None,
            total: 0,
            error,
        });
    }
    let total = said["total"].as_u64()? as usize;
    let current = match &said["current"] {
        Value::Null => None,
        at => Some(at.as_u64()? as usize),
    };
    Some(crate::finding::Found {
        current,
        total,
        error: None,
    })
}

impl Render for MarkView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Dropped files: images go beside the document.
        let drop = cx.listener(|view, dropped: &ExternalPaths, _, cx| {
            view.attach(crate::attach::Attach::Files(dropped.paths().to_vec()), cx)
        });
        div().size_full().on_drop(drop).child(self.web.clone())
    }
}

#[cfg(test)]
mod tests;
