//! Rich, HTML code and plain text copies (R4.10).

use std::path::PathBuf;

use ely_gpui_component::primitives::Severity;
use ely_office::prelude::*;
use ely_platform::clipboard::{self, Clip};
use ely_ui::i18n::text;
use gpui::{ClipboardItem, Context, Div, InteractiveElement as _, actions};

use crate::desk::Desk;
use crate::output::{self, Page, Pictures};

actions!(mark, [CopyRich, CopyHtml, CopyPlain]);

/// What a copy turns the Markdown into.
#[derive(Clone, Copy)]
enum As {
    Rich,
    Html,
    Plain,
}

/// A converted copy, ready for the clipboard.
enum Copied {
    Rich(Clip, Page),
    Html(String, Page),
    Plain(String),
}

impl Desk {
    pub(super) fn copy_actions(root: Div, cx: &mut Context<Self>) -> Div {
        root.on_action(cx.listener(|desk, _: &CopyRich, _, cx| desk.copy(As::Rich, cx)))
            .on_action(cx.listener(|desk, _: &CopyHtml, _, cx| desk.copy(As::Html, cx)))
            .on_action(cx.listener(|desk, _: &CopyPlain, _, cx| desk.copy(As::Plain, cx)))
    }

    /// The selection or whole text, and the file's folder.
    fn copied_text(&mut self, cx: &mut Context<Self>) -> Option<(String, Option<PathBuf>)> {
        let Some(view) = self.active_view() else {
            self.no_document(cx);
            return None;
        };
        let document = view.read(cx).document().read(cx);
        let selected = document.selected(cx);
        let markdown = match selected.is_empty() {
            true => document.text(cx),
            false => selected,
        };
        let beside = (self.active_tab())
            .and_then(|tab| tab.session.path())
            .and_then(|path| path.parent())
            .map(PathBuf::from);
        Some((markdown, beside))
    }

    fn copy_failed(&mut self, error: &std::io::Error, cx: &mut Context<Self>) {
        let said = text(cx, "mark.copy.failed", &[("error", &error.to_string())]);
        self.toast(said, Severity::Danger, cx);
    }

    /// Converts in the background, then writes the clipboard.
    fn copy(&mut self, kind: As, cx: &mut Context<Self>) {
        let Some((markdown, beside)) = self.copied_text(cx) else {
            return;
        };
        let before = witness(cx);
        self.copying = Some(cx.spawn(async move |desk, cx| {
            let converted = cx
                .background_executor()
                .spawn(async move { convert(kind, markdown, beside) })
                .await;
            let landed = desk.update(cx, |desk, cx| {
                desk.copying = None;
                // A later copy, here or elsewhere, wins.
                if witness(cx) != before {
                    return log::info!("mark: the clipboard changed first; a copy is dropped");
                }
                desk.copied(converted, cx);
            });
            if landed.is_err() {
                log::warn!("mark: a copy finished after its window closed");
            }
        }));
    }

    fn copied(&mut self, converted: std::io::Result<Copied>, cx: &mut Context<Self>) {
        let written = converted.and_then(|copied| match copied {
            Copied::Rich(clip, page) => clipboard::write(&clip)
                .map(|()| Some(page))
                .map_err(|error| std::io::Error::other(format!("{error:#}"))),
            Copied::Html(html, page) => {
                log::info!("mark: {} bytes copied as HTML code", html.len());
                cx.write_to_clipboard(ClipboardItem::new_string(html));
                Ok(Some(page))
            }
            Copied::Plain(plain) => {
                log::info!("mark: {} bytes copied as plain text", plain.len());
                cx.write_to_clipboard(ClipboardItem::new_string(plain));
                Ok(None)
            }
        });
        match written {
            Ok(Some(page)) => self.left_out(&page, cx),
            Ok(None) => {}
            Err(error) => self.copy_failed(&error, cx),
        }
    }
}

/// What the clipboard holds, its HTML included.
fn witness(cx: &mut Context<Desk>) -> (Option<ClipboardItem>, Result<Option<String>, String>) {
    let html = clipboard::html().map_err(|error| {
        log::warn!("mark: the clipboard's HTML did not read: {error:#}");
        format!("{error:#}")
    });
    (cx.read_from_clipboard(), html)
}

/// The copy as `kind`, converted off the window thread.
fn convert(kind: As, markdown: String, beside: Option<PathBuf>) -> std::io::Result<Copied> {
    let beside = beside.as_deref();
    match kind {
        As::Rich => output::body(&markdown, beside, &Pictures::Inline).map(|mut page| {
            let html = std::mem::take(&mut page.html);
            let clip = Clip::Html {
                html,
                text: markdown,
            };
            Copied::Rich(clip, page)
        }),
        As::Html => output::body(&markdown, beside, &Pictures::Written)
            .map(|mut page| Copied::Html(std::mem::take(&mut page.html), page)),
        As::Plain => output::plain(&markdown).map(Copied::Plain),
    }
}
