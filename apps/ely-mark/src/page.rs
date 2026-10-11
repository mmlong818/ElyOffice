//! The bundled page: its folder and its input.

use std::path::{Path, PathBuf};

use anyhow::{Result, bail};
use ely_web_content::WebView;
use gpui::App;
use serde_json::{Value, json};

use crate::view::Heading;

/// Beside the app, its Resources, else this build's.
pub fn content() -> Result<PathBuf> {
    let exe = std::env::current_exe()?;
    let dir = exe.parent().unwrap_or(Path::new("."));
    let built = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../engines/content/ely-mark");
    let mut candidates = vec![dir.join("content"), dir.join("../Resources/content")];
    #[cfg(target_os = "windows")]
    candidates.insert(0, dir.join("Resources/content/ely-mark"));
    candidates.push(built);
    for dir in candidates {
        if dir.join("index.html").is_file() {
            log::info!("mark: page from {}", dir.display());
            return Ok(dir);
        }
    }
    bail!("ElyMark's page is not built; run scripts/build-mark-content.sh")
}

/// How the page shows a document (MARK-02).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Visual,
    Source,
    Preview,
}

impl Mode {
    pub const ALL: [Mode; 3] = [Mode::Visual, Mode::Source, Mode::Preview];

    pub fn name(self) -> &'static str {
        match self {
            Mode::Visual => "visual",
            Mode::Source => "source",
            Mode::Preview => "preview",
        }
    }

    pub fn parse(name: &str) -> Option<Mode> {
        Mode::ALL.into_iter().find(|mode| mode.name() == name)
    }
}

/// Shows `markdown` as `mode` after load `load`, maybe placed.
pub fn open(
    view: &WebView,
    markdown: &str,
    (mode, load, place): (Mode, u64, Option<crate::view::Place>),
    folder: &Path,
    cx: &App,
) {
    let text = |key| ely_ui::i18n::text(cx, key, &[]).to_string();
    let texts = json!({
        "searchLanguage": text("mark.search_language"),
        "noResult": text("mark.no_result"),
        "copy": text("mark.copy"),
        "preview": text("mark.preview"),
        "loading": text("mark.loading"),
        "edit": text("mark.edit"),
        "hide": text("mark.hide"),
    });
    // Relative links and images resolve in the document's folder.
    let base = format!("{}/", ely_web_content::file_url(folder));
    view.run_js(&format!(
        "ely.open({}, '{}', {texts}, {load}, {}, {})",
        json!(markdown),
        mode.name(),
        json!(base),
        json!(place)
    ));
}

/// The disk's text in place; reports start a new generation.
pub fn refresh(view: &WebView, markdown: &str, load: u64) {
    view.run_js(&format!("ely.refresh({}, {load})", json!(markdown)));
}

/// The same text in another mode; pending edits go first.
pub fn switch(view: &WebView, mode: Mode) {
    view.run_js(&format!("ely.mode('{}')", mode.name()));
}

/// The folder for relative links and images.
pub fn folder(view: &WebView, folder: &Path) {
    let base = format!("{}/", ely_web_content::file_url(folder));
    view.run_js(&format!("ely.base({})", json!(base)));
}

/// Calls `ely.<name>(argument)` in the page.
pub fn call(view: &WebView, name: &str, argument: &serde_json::Value) {
    view.run_js(&format!("ely.{name}({argument})"));
}

/// Headings from the page; anything malformed is None.
pub fn headings(items: &Value) -> Option<Vec<Heading>> {
    let heading = |item: &Value| {
        let level = item["level"]
            .as_u64()
            .filter(|level| (1..=6).contains(level))?;
        let title = item["title"].as_str()?.to_owned();
        Some(Heading {
            level: level as usize,
            title,
        })
    };
    items.as_array()?.iter().map(heading).collect()
}
