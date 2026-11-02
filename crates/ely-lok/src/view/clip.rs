//! The clipboard for LokView (LOK-06, TABLE-08).

use std::{
    io,
    sync::atomic::{AtomicU64, Ordering},
};

use ely_platform::clipboard::{self, Clip};

use crate::{Doc, Worker};

/// The clipboard payload for copied LOK flavors.
pub(super) fn clip_of(flavors: Vec<(String, Vec<u8>)>) -> Option<Clip> {
    let take = |mime: &str| {
        flavors
            .iter()
            .find(|(name, _)| name == mime)
            .map(|(_, data)| data.clone())
    };
    let text =
        take("text/plain;charset=utf-8").map(|data| String::from_utf8_lossy(&data).into_owned());
    if let (Some(png), None) = (take("image/png"), &text) {
        return Some(Clip::Png(png));
    }
    match (take("text/html"), text) {
        (Some(html), text) => Some(Clip::Html {
            html: String::from_utf8_lossy(&html).into_owned(),
            text: text.unwrap_or_default(),
        }),
        (None, Some(text)) => Some(Clip::Text(text)),
        (None, None) => None,
    }
}

/// The LOK flavor for a clipboard payload.
pub(super) fn flavor_of(clip: Clip) -> (&'static str, Vec<u8>) {
    match clip {
        Clip::Text(text) => ("text/plain;charset=utf-8", text.into_bytes()),
        Clip::Html { html, .. } => ("text/html", html.into_bytes()),
        Clip::Png(png) => ("image/png", png),
    }
}

/// Copies out, marked; a cut deletes after the write.
pub(super) fn copy_out(
    worker: &mut Worker,
    doc: &Doc,
    cut: bool,
    write: impl FnOnce(&Clip) -> io::Result<()>,
) -> io::Result<Option<String>> {
    let Some(mut clip) = clip_of(worker.copy(doc)?) else {
        return Ok(None);
    };
    let mark = if let Clip::Html { html, .. } = &mut clip {
        static COPIES: AtomicU64 = AtomicU64::new(0);
        let mark = format!(
            "{}-{}",
            std::process::id(),
            COPIES.fetch_add(1, Ordering::SeqCst)
        );
        html.push_str(&format!("<!--{MARK}{mark}-->"));
        Some(mark)
    } else {
        None
    };
    write(&clip)?;
    if cut {
        worker.cut(doc)?;
    }
    Ok(mark)
}

/// Ely's copies end their HTML with this and a mark.
const MARK: &str = "ely-copy:";

/// The mark of Ely's copy in `html`, if any.
pub(super) fn mark_of(html: &str) -> Option<&str> {
    let start = html.rfind(&format!("<!--{MARK}"))? + 4 + MARK.len();
    let end = html[start..].find("-->")?;
    Some(&html[start..start + end])
}

/// Pastes the system clipboard.
pub(super) fn paste(worker: &mut Worker, doc: &Doc) -> io::Result<()> {
    let Some(clip) = clipboard::read().map_err(|error| io::Error::other(format!("{error:#}")))? else {
        return Ok(());
    };
    let (mime, data) = flavor_of(clip);
    worker.paste(doc, mime, &data)
}
