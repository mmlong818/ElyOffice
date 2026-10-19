//! Markdown as an HTML page or plain text (MARK-11).

mod address;
mod files;
mod html;
mod pictures;
mod plain;
#[cfg(test)]
mod tests;

use std::{io, path::Path};

use ely_gpui_component::documents::markdown_tree;

pub use files::{Format, Target, write};
pub use pictures::{Pictures, picture_file};
pub use plain::plain;

/// A page and what it could not carry.
#[derive(Debug, Default)]
pub struct Page {
    pub html: String,
    /// Pictures left out, as the document wrote them.
    pub missing: Vec<String>,
    /// Lines of diagrams kept as code.
    pub diagrams: Vec<usize>,
    /// Lines of formulas kept as code, and why.
    pub formulas: Vec<(usize, String)>,
}

/// The page's look: light, as printed.
const STYLE: &str = "body{font-family:sans-serif;font-size:11pt;line-height:1.5;color:#1f2328;max-width:48rem;margin:2rem auto;padding:0 1rem}\
pre{background:#f6f8fa;padding:.75em;white-space:pre-wrap}\
code{font-family:monospace}\
table{border-collapse:collapse}th,td{border:1px solid #d0d7de;padding:.3em .6em}\
blockquote{margin-left:0;padding-left:1em;border-left:3px solid #d0d7de;color:#59636e}\
img,svg{max-width:100%}div.math{text-align:center}span.math svg{vertical-align:middle}";

/// `text`'s body as HTML; pictures go as `pictures` says.
pub fn body(text: &str, beside: Option<&Path>, pictures: &Pictures) -> io::Result<Page> {
    page_of(text, &mut pictures::Placed::new(beside, pictures))
}

/// `text`'s body; `placed` keeps what it copied.
fn page_of(text: &str, placed: &mut pictures::Placed) -> io::Result<Page> {
    let tree = markdown_tree(text);
    let mut page = Page::default();
    html::write(&tree, text, placed, &mut page)?;
    log::info!(
        "mark: a page of {} bytes, {} pictures copied, {} missing, {} diagrams and {} formulas as code",
        page.html.len(),
        placed.copied.len(),
        page.missing.len(),
        page.diagrams.len(),
        page.formulas.len()
    );
    Ok(page)
}

/// A whole HTML document around `body`.
pub fn document(title: &str, body: &str) -> String {
    format!(
        "<!doctype html>\n<html><head><meta charset=\"utf-8\"><title>{}</title><style>{STYLE}</style></head>\n<body>\n{body}\n</body></html>\n",
        escape(title)
    )
}

/// Text safe between tags.
fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Text safe inside a quoted attribute.
fn attribute(text: &str) -> String {
    escape(text).replace('"', "&quot;")
}
