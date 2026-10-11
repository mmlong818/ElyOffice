//! Markdown files as the shell's engine (§14.3).

use std::{
    convert::Infallible,
    hash::{DefaultHasher, Hash as _, Hasher as _},
    io,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use ely_office::{Engine, Host, Load};
use ely_web_content::WebView;
use gpui::{App, AppContext as _, Context, Entity, FocusHandle, Window};

use crate::text::Source;
use crate::view::{Failed, MarkView, Reported};

/// The bundled page every tab shows.
pub struct Pages {
    pub dir: PathBuf,
}

/// A file as read, and its text now.
pub struct Text {
    pub source: Source,
    pub current: String,
    /// The last write's mark, until history reads it.
    written: Option<String>,
}

pub type Doc = Arc<Mutex<Text>>;

/// A load: the file as read, and where it is.
pub struct Opened {
    pub source: Source,
    pub path: PathBuf,
}

/// The history mark: this text's length and hash.
pub fn mark(text: &str) -> String {
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    format!("{}:{:016x}", text.len(), hasher.finish())
}

pub struct Mark;

impl Engine for Mark {
    type Worker = Pages;
    type Doc = Doc;
    type View = MarkView;
    type Loaded = Opened;
    type Import = Infallible;

    fn load(_: &mut Pages, from: Load<Infallible>) -> io::Result<Opened> {
        let path = match from {
            Load::File(path, _) => path,
            Load::Import(_, never) => match *never {},
        };
        // The shell reads PermissionDenied as a locked file.
        let bytes = std::fs::read(path).map_err(|error| match error.kind() {
            io::ErrorKind::PermissionDenied => io::Error::other(error),
            _ => error,
        })?;
        Ok(Opened {
            source: Source::decode(bytes)?,
            path: path.to_path_buf(),
        })
    }

    fn open(
        worker: &Arc<Mutex<Pages>>,
        loaded: Opened,
        window: &mut Window,
        cx: &mut App,
    ) -> io::Result<(Doc, Entity<MarkView>, String)> {
        let Opened {
            source: loaded,
            path,
        } = loaded;
        let folder = path
            .parent()
            .expect("a file sits in a folder")
            .to_path_buf();
        let dir = worker.lock().expect("the pages lock").dir.clone();
        let web =
            WebView::open(&dir.join("index.html"), &dir, window, cx).map_err(io::Error::other)?;
        let mark = mark(&loaded.text);
        let current = loaded.text.clone();
        let doc = Arc::new(Mutex::new(Text {
            source: loaded,
            current,
            written: None,
        }));
        web.read(cx)
            .allow_images(&folder)
            .map_err(io::Error::other)?;
        let view = cx.new(|cx| MarkView::new(web, Arc::clone(&doc), mark.clone(), folder, cx));
        Ok((doc, view, mark))
    }

    /// After a write, what it wrote: later edits stay unsaved.
    fn history(_: &mut Pages, doc: &Doc) -> io::Result<String> {
        let mut text = doc.lock().expect("the text lock");
        Ok(text.written.take().unwrap_or_else(|| mark(&text.current)))
    }

    fn write(_: &mut Pages, doc: &Doc, path: &Path, format: &str) -> io::Result<()> {
        if !matches!(format, "md" | "markdown") {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                format!("writing .{format}"),
            ));
        }
        let mut text = doc.lock().expect("the text lock");
        std::fs::write(path, text.source.encode(&text.current))?;
        text.written = Some(mark(&text.current));
        Ok(())
    }

    fn close(_: &mut Pages, _: Doc) -> io::Result<()> {
        Ok(())
    }

    fn retire(view: &Entity<MarkView>, cx: &mut App) -> ely_gpui_component::primitives::Retired {
        let web = view.read(cx).web.clone();
        web.update(cx, |web, _| web.retire_all())
    }

    fn settled(view: &MarkView) -> bool {
        view.settled()
    }

    fn edited(view: &MarkView) -> u64 {
        view.changed()
    }

    fn edit_pending(view: &MarkView) -> bool {
        !view.settled()
    }

    fn focus(view: &MarkView) -> FocusHandle {
        view.focus.clone()
    }

    fn saved(view: &Entity<MarkView>, mark: &str, cx: &mut App) {
        view.update(cx, |view, cx| view.saved(mark, cx));
    }

    fn watch<H: Host<Engine = Self>>(
        view: &Entity<MarkView>,
        cx: &mut Context<H>,
        reported: impl Fn(&mut H, &Entity<MarkView>, bool, &mut Context<H>) + 'static,
        failed: impl Fn(&mut H, &str, &mut Context<H>) + 'static,
    ) {
        cx.subscribe(view, move |host, _, error: &Failed, cx| {
            failed(host, &error.0, cx)
        })
        .detach();
        cx.subscribe(view, move |host, view, _: &Reported, cx| {
            let modified = view.read(cx).modified();
            reported(host, &view, modified, cx)
        })
        .detach();
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use ely_office::Engine as _;

    use super::{Mark, Pages, Text, mark};
    use crate::text::Source;

    #[test]
    fn a_save_marks_the_text_it_wrote() {
        let dir = tempfile::tempdir().expect("a temp dir");
        let source = Source::decode(b"Alpha.\n".to_vec()).expect("reads");
        let current = source.text.clone();
        let doc = Arc::new(Mutex::new(Text {
            source,
            current,
            written: None,
        }));
        let mut pages = Pages {
            dir: dir.path().to_path_buf(),
        };
        let path = dir.path().join("save.md");
        Mark::write(&mut pages, &doc, &path, "md").expect("writes");
        // An edit lands between the write and the history read.
        doc.lock().expect("the text lock").current = "xAlpha.\n".into();
        let saved = Mark::history(&mut pages, &doc).expect("a mark");
        assert_eq!(saved, mark("Alpha.\n"));
        assert_eq!(
            Mark::history(&mut pages, &doc).expect("a mark"),
            mark("xAlpha.\n")
        );
    }
}
