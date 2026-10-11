//! Export and print: HTML, linked HTML, PDF, text (MARK-11).

use std::{
    io,
    path::{Path, PathBuf},
};

use ely_gpui_component::{
    buttons::{Button, ButtonVariant, SegmentedControl},
    overlays::Dialog,
    primitives::Severity,
};
use ely_office::{cancel, files::file_name, in_window, prelude::*};
use ely_ui::{commands, i18n::text};
use ely_web_content::{Event, WebView};
use gpui::{
    AnyElement, App, AppContext as _, Context, Entity, IntoElement as _, ParentElement as _,
    Subscription, Task, Window,
};
use serde_json::Value;

use crate::desk::Desk;
use crate::paper;

/// What an export writes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Output {
    Page,
    Linked,
    Pdf,
    Text,
}

impl Output {
    const ALL: [Output; 4] = [Output::Page, Output::Linked, Output::Pdf, Output::Text];

    fn key(self) -> &'static str {
        match self {
            Output::Page => "page",
            Output::Linked => "linked",
            Output::Pdf => "pdf",
            Output::Text => "text",
        }
    }

    fn extension(self) -> &'static str {
        match self {
            Output::Page | Output::Linked => "html",
            Output::Pdf => "pdf",
            Output::Text => "txt",
        }
    }
}

/// What an export lacks: images, diagrams, formulas.
#[derive(Clone, Debug, Default)]
pub struct Lost {
    images: Vec<String>,
    diagrams: Vec<u64>,
    /// Each broken formula's line and KaTeX's message.
    formulas: Vec<(u64, String)>,
}

impl Lost {
    pub fn of(images: Vec<String>, rendered: &Value) -> Lost {
        let diagrams = rendered["failed"].as_array().into_iter().flatten();
        let formulas = rendered["formulas"].as_array().into_iter().flatten();
        Lost {
            images,
            diagrams: diagrams.filter_map(Value::as_u64).collect(),
            formulas: formulas
                .filter_map(|f| Some((f["line"].as_u64()?, f["error"].as_str()?.to_owned())))
                .collect(),
        }
    }

    fn whole(&self) -> bool {
        self.images.is_empty() && self.diagrams.is_empty() && self.formulas.is_empty()
    }
}

/// No export lands on a file a tab holds (FILE-06).
fn not_open(desk: &Desk, path: &Path) -> io::Result<()> {
    match desk.holder(path)? {
        Some(_) => Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("{} is open in ElyMark", file_name(path)),
        )),
        None => Ok(()),
    }
}

/// A page printing to PDF in a view nobody sees.
pub struct Printing {
    _view: Entity<WebView>,
    _heard: Subscription,
}

impl Desk {
    pub fn on_export(&mut self, _: &commands::Export, _: &mut Window, cx: &mut Context<Self>) {
        if self.active_tab().is_some() {
            self.output = Some(Output::Page);
            cx.notify();
        }
    }

    pub fn on_print(&mut self, _: &commands::Print, _: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.active_tab().map(|tab| tab.id) else {
            return;
        };
        self.render_pdf(id, None, cx);
    }

    /// Asks where, renders the whole text, then writes it.
    fn export_to(&mut self, output: Output, cx: &mut Context<Self>) {
        let Some(tab) = self.active_tab() else {
            return;
        };
        let id = tab.id;
        let stem = tab
            .name
            .rsplit_once('.')
            .map_or(tab.name.as_ref(), |(stem, _)| stem)
            .to_owned();
        let dir = ely_office::folder(tab).unwrap_or_else(std::env::temp_dir);
        let chosen = cx.prompt_for_new_path(&dir, Some(&format!("{stem}.{}", output.extension())));
        cx.spawn(async move |desk, cx| {
            let chosen = match chosen.await {
                Ok(Ok(path)) => Ok(path),
                Ok(Err(error)) => Err(error.to_string()),
                Err(error) => Err(error.to_string()),
            };
            let done = desk.update(cx, |desk, cx| match chosen {
                Ok(Some(path)) => {
                    if let Err(error) = not_open(desk, &path) {
                        return desk.exported(Err(error), cx);
                    }
                    match output {
                        Output::Pdf => desk.render_pdf(id, Some(path), cx),
                        _ => desk.write_output(id, output, path, cx),
                    }
                }
                Ok(None) => log::info!("mark: the export panel was cancelled"),
                Err(why) => desk.panel_failed(&why, cx),
            });
            if done.is_err() {
                log::info!("mark: the window closed before the export panel");
            }
        })
        .detach();
    }

    /// HTML or text, assembled and written off the main thread.
    fn write_output(&mut self, id: u64, output: Output, path: PathBuf, cx: &mut Context<Self>) {
        let Some(tab) = self.tab(id) else {
            return;
        };
        let (view, folder) = (tab.view.clone(), tab.view.read(cx).folder.clone());
        let open = tab.session.path().map(Path::to_path_buf);
        let style = self.pages_dir().join("export.css");
        let kind = if output == Output::Text {
            "text"
        } else {
            "html"
        };
        let desk = cx.weak_entity();
        view.update(cx, |view, cx| {
            view.render(
                kind,
                move |rendered, cx: &mut App| {
                    // Checked again: tabs may have opened meanwhile.
                    let free = desk.update(cx, |desk, _| not_open(desk, &path));
                    let free = free.map_err(io::Error::other).and_then(|free| free);
                    let job = cx.background_spawn(async move {
                        free?;
                        let rendered = rendered.map_err(io::Error::other)?;
                        write(output, &rendered, &folder, &style, &path, open.as_deref())
                    });
                    cx.spawn(async move |cx| {
                        let written = job.await;
                        desk.update(cx, |desk, cx| desk.exported(written, cx)).ok();
                    })
                    .detach();
                },
                cx,
            )
        });
    }

    fn exported(&mut self, written: io::Result<(PathBuf, Lost)>, cx: &mut Context<Self>) {
        match written {
            Ok((path, lost)) if lost.whole() => {
                log::info!("mark: exported {}", path.display());
                let said = text(cx, "mark.export.done", &[("name", &file_name(&path))]);
                self.toast_output(said, path, cx);
            }
            Ok((path, lost)) => {
                // Counts only: image names come from the text.
                log::warn!(
                    "mark: exported {} without {} images and {} diagrams; formulas failed at lines {:?}",
                    path.display(),
                    lost.images.len(),
                    lost.diagrams.len(),
                    lost.formulas.iter().map(|(line, _)| line).collect::<Vec<_>>()
                );
                self.warn_lost(&lost, Some(path), cx);
            }
            Err(error) => {
                log::error!("mark: an export failed: {error}");
                let said = text(cx, "mark.export.failed", &[("error", &error.to_string())]);
                self.toast(said, Severity::Danger, cx);
            }
        }
    }

    /// Says which images, diagrams and formulas an output lacks.
    fn warn_lost(&mut self, lost: &Lost, output: Option<PathBuf>, cx: &mut Context<Self>) {
        let mut said = Vec::new();
        if !lost.images.is_empty() {
            said.push(text(
                cx,
                "mark.export.missing",
                &[("list", &lost.images.join(", "))],
            ));
        }
        if !lost.diagrams.is_empty() {
            said.push(text(
                cx,
                "mark.export.diagrams",
                &[(
                    "list",
                    &lost
                        .diagrams
                        .iter()
                        .map(u64::to_string)
                        .collect::<Vec<_>>()
                        .join(", "),
                )],
            ));
        }
        for (line, error) in &lost.formulas {
            let line = line.to_string();
            said.push(text(
                cx,
                "mark.export.formula",
                &[("line", &line), ("error", error)],
            ));
        }
        // Names and messages quote the text: no log.
        let said: gpui::SharedString = said.join(" ").into();
        match (output, self.core.root.upgrade()) {
            (Some(path), Some(root)) => root.update(cx, |root, cx| root.toast_output(said, Severity::Warning, path, cx)),
            _ => self.toast_unlogged(said, Severity::Warning, cx),
        }
    }

    fn pages_dir(&self) -> PathBuf {
        self.core.worker.lock().expect("the pages lock").dir.clone()
    }

    /// The whole text as PDF; then to `to`, else printed.
    fn render_pdf(&mut self, id: u64, to: Option<PathBuf>, cx: &mut Context<Self>) {
        // Each job has its own files; none cancels another.
        self.print_jobs += 1;
        let n = self.print_jobs;
        let pdf = self.core.work.join(format!("print-{n}.pdf"));
        let Some(tab) = self.tab(id) else {
            return;
        };
        let (view, folder, name) = (
            tab.view.clone(),
            tab.view.read(cx).folder.clone(),
            tab.name.to_string(),
        );
        let open = tab.session.path().map(Path::to_path_buf);
        let (style, work) = (self.pages_dir().join("export.css"), self.core.work.clone());
        let desk = cx.weak_entity();
        view.update(cx, |view, cx| {
            view.render(
                "html",
                move |rendered, cx: &mut App| {
                    let staged = work.join(format!("print-{n}.html"));
                    let page = match rendered {
                        Ok(rendered) => {
                            let print = paper::Print { folder, style, name, page: staged.clone() };
                            paper::stage(cx.background_executor(), rendered, print)
                        }
                        Err(why) => Task::ready(Err(io::Error::other(why))),
                    };
                    cx.spawn(async move |cx| {
                        let page = page.await;
                        in_window(&desk, cx, |desk, window, cx| match page {
                            Ok(lost) => {
                                let job = Job {
                                    pdf,
                                    to,
                                    open,
                                    lost,
                                };
                                desk.print_page(n, &staged, job, window, cx)
                            }
                            Err(error) => desk.exported(Err(error), cx),
                        })
                        .ok();
                    })
                    .detach();
                },
                cx,
            )
        });
    }

    /// Chromium prints the staged page; the PDF goes where asked.
    fn print_page(&mut self, n: u64, page: &Path, job: Job, window: &mut Window, cx: &mut Context<Self>) {
        let root = page
            .parent()
            .expect("a staged page sits in a folder")
            .to_path_buf();
        // A document's own scripts never run here.
        let view = match WebView::open_inert(page, &root, window, cx) {
            Ok(view) => view,
            Err(error) => return self.exported(Err(io::Error::other(error)), cx),
        };
        let heard = cx.subscribe(&view, move |desk, view, event: &Event, cx| match event {
            Event::Loaded => view.read(cx).print_pdf(&job.pdf),
            Event::Printed(path, ok) => {
                log::info!("mark: job {n} printed {}, ok {ok}", path.display());
                desk.printing.remove(&n);
                let printed = ok
                    .then(|| path.clone())
                    .ok_or_else(|| io::Error::other("Chromium did not print the page"));
                desk.printed(printed, job.clone(), cx);
            }
            Event::Failed(why) => {
                desk.printing.remove(&n);
                desk.exported(Err(io::Error::other(why.clone())), cx);
            }
            Event::Query(_, reply) => reply.ok(""),
            Event::Leave(_) => {}
        });
        self.printing.insert(n, Printing {
            _view: view,
            _heard: heard,
        });
    }

    fn printed(&mut self, pdf: io::Result<PathBuf>, job: Job, cx: &mut Context<Self>) {
        let pdf = match pdf {
            Ok(pdf) => pdf,
            Err(error) => return self.exported(Err(error), cx),
        };
        let Some(to) = job.to else {
            if !job.lost.whole() {
                self.warn_lost(&job.lost, None, cx);
            }
            return ely_office::print_later(pdf, cx);
        };
        // Never over the open file, not even through a link.
        let moved = not_open(self, &to)
            .and_then(|()| std::fs::read(&pdf))
            .and_then(|bytes| ely_files::export(&to, job.open.as_deref(), &bytes, 0).map(|_| ()));
        let _ = std::fs::remove_file(&pdf);
        self.exported(moved.map(|()| (to, job.lost)), cx);
    }

    pub fn export_dialog(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let chosen = self.output?;
        let this = cx.entity();
        let kinds = Output::ALL.into_iter().fold(
            SegmentedControl::new("mark-export-kind", chosen.key()),
            |control, output| {
                control.segment(
                    output.key(),
                    text(cx, &format!("mark.export.{}", output.key()), &[]),
                    None,
                )
            },
        );
        let kinds = kinds.on_change({
            let this = this.clone();
            move |key, _, cx| {
                let picked = Output::ALL
                    .into_iter()
                    .find(|output| key.as_ref() == output.key());
                this.update(cx, |desk, cx| {
                    desk.output = picked;
                    cx.notify();
                })
            }
        });
        let close = {
            let this = this.clone();
            move |_: &mut Window, cx: &mut App| {
                this.update(cx, |desk, cx| {
                    desk.output = None;
                    cx.notify();
                })
            }
        };
        let label = text(cx, "mark.export.go", &[]);
        let dialog = Dialog::new("mark-export", text(cx, "mark.export", &[]), close)
            .child(kinds)
            .action(cancel(cx))
            .action(move |close| {
                Button::new("mark-export-go", label)
                    .variant(ButtonVariant::Primary)
                    .on_click(move |_, window, cx| {
                        close(window, cx);
                        this.update(cx, |desk, cx| desk.export_to(chosen, cx));
                    })
            });
        Some(dialog.into_any_element())
    }
}

/// A PDF in flight: where printed, where bound, what lacks.
#[derive(Clone)]
struct Job {
    pdf: PathBuf,
    to: Option<PathBuf>,
    /// The tab's own file, never overwritten.
    open: Option<PathBuf>,
    lost: Lost,
}

/// Writes `output` at `path`; and what it left out.
fn write(
    output: Output,
    rendered: &Value,
    folder: &Path,
    style: &Path,
    path: &Path,
    open: Option<&Path>,
) -> io::Result<(PathBuf, Lost)> {
    let title = path
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_default();
    let (bytes, missing) = match output {
        Output::Text => (
            rendered["text"]
                .as_str()
                .unwrap_or_default()
                .as_bytes()
                .to_vec(),
            Vec::new(),
        ),
        Output::Page => {
            let (html, missing) = paper::Marked::read(rendered)?.inline(folder);
            let style = format!("<style>{}</style>", std::fs::read_to_string(style)?);
            (paper::page(&title, &style, &html).into_bytes(), missing)
        }
        Output::Linked => {
            let assets = format!("{title}_files");
            let place = path.with_file_name(&assets);
            let (html, copies, missing) = paper::Marked::read(rendered)?.linked(folder, &assets);
            // A folder already there is never written into.
            std::fs::create_dir(&place)?;
            let copied = copies
                .iter()
                .try_for_each(|(from, name)| std::fs::copy(from, place.join(name)).map(|_| ()))
                .and_then(|()| std::fs::copy(style, place.join("export.css")).map(|_| ()));
            if let Err(error) = copied {
                let _ = std::fs::remove_dir_all(&place);
                return Err(error);
            }
            let style = format!(
                "<link rel=\"stylesheet\" href=\"{}/export.css\">",
                paper::segment(&assets)
            );
            (paper::page(&title, &style, &html).into_bytes(), missing)
        }
        Output::Pdf => unreachable!("PDF prints in a page"),
    };
    if let Err(error) = ely_files::export(path, open, &bytes, 0) {
        if output == Output::Linked {
            let _ = std::fs::remove_dir_all(path.with_file_name(format!("{title}_files")));
        }
        return Err(error);
    }
    Ok((path.to_path_buf(), Lost::of(missing, rendered)))
}
