//! Rendered Markdown as files: HTML pages and their images (MARK-11).

use std::path::{Path, PathBuf};

use base64::Engine as _;

/// A whole HTML page: the body, its title, its stylesheet.
pub fn page(title: &str, style: &str, body: &str) -> String {
    format!(
        "<!doctype html>\n<html>\n<head>\n<meta charset=\"utf-8\">\n<title>{}</title>\n{style}\n</head>\n<body>\n{body}\n</body>\n</html>\n",
        escape(title)
    )
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// `%XX` decoded, as links to files are written.
fn decoded(text: &str) -> String {
    let bytes = text.as_bytes();
    let (mut out, mut at) = (Vec::with_capacity(bytes.len()), 0);
    while at < bytes.len() {
        let hex = bytes
            .get(at + 1..at + 3)
            .and_then(|pair| u8::from_str_radix(std::str::from_utf8(pair).ok()?, 16).ok());
        match (bytes[at], hex) {
            (b'%', Some(byte)) => (out.push(byte), at += 3),
            (byte, _) => (out.push(byte), at += 1),
        };
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// One path segment for a URL: reserved bytes percent-escaped.
pub fn segment(name: &str) -> String {
    let mut out = String::new();
    for byte in name.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

/// A file image's path; web links stay as they are.
fn local(src: &str, folder: &Path) -> Option<PathBuf> {
    if src.starts_with("data:") || src.starts_with('#') {
        return None;
    }
    if let Some(path) = src.strip_prefix("file://") {
        return Some(PathBuf::from(decoded(path)));
    }
    if src.contains("://") {
        return None;
    }
    let path = PathBuf::from(decoded(src.split(['?', '#']).next().unwrap_or(src)));
    Some(if path.is_absolute() {
        path
    } else {
        folder.join(path)
    })
}

fn kind(path: &Path) -> &'static str {
    let extension = path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default();
    match extension.to_lowercase().as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "bmp" => "image/bmp",
        "avif" => "image/avif",
        _ => "application/octet-stream",
    }
}

/// What the print page is made of.
pub struct Print {
    pub folder: std::path::PathBuf,
    pub style: std::path::PathBuf,
    pub name: String,
    pub page: std::path::PathBuf,
}

/// Writes the print page off the window thread, images inline.
pub fn stage(
    background: &gpui::BackgroundExecutor,
    rendered: serde_json::Value,
    print: Print,
) -> gpui::Task<std::io::Result<crate::export::Lost>> {
    background.spawn(async move {
        let (html, images) = Marked::read(&rendered)?.inline(&print.folder);
        let style = format!("<style>{}</style>", std::fs::read_to_string(&print.style)?);
        std::fs::write(&print.page, page(&print.name, &style, &html))?;
        Ok(crate::export::Lost::of(images, &rendered))
    })
}

/// The page's HTML; image `src` values replaced by numbered tokens.
#[derive(Clone, Debug, Default)]
pub struct Marked {
    pub html: String,
    /// The original `src` values, decoded, in order.
    pub images: Vec<String>,
    pub token: String,
}

impl Marked {
    /// From the page's answer; anything missing is an error.
    pub fn read(rendered: &serde_json::Value) -> std::io::Result<Marked> {
        let bad = || std::io::Error::other("the page's render lacks its parts");
        let html = rendered["html"].as_str().ok_or_else(bad)?.to_owned();
        let token = rendered["token"].as_str().ok_or_else(bad)?.to_owned();
        let images = rendered["images"].as_array().ok_or_else(bad)?;
        let images = images
            .iter()
            .map(|src| src.as_str().map(str::to_owned).ok_or_else(bad));
        let images = images.collect::<std::io::Result<_>>()?;
        Ok(Marked {
            html,
            images,
            token,
        })
    }

    /// Each mark swapped for `each` of its file; unread listed.
    fn rewrite(
        &self,
        folder: &Path,
        mut each: impl FnMut(&Path) -> Option<String>,
    ) -> (String, Vec<String>) {
        let (mut html, mut missing) = (self.html.clone(), Vec::new());
        for (n, src) in self.images.iter().enumerate() {
            let new = match local(src, folder) {
                None => src.clone(),
                Some(path) => each(&path).unwrap_or_else(|| {
                    missing.push(src.clone());
                    src.clone()
                }),
            };
            // Quoted, so mark 1 never matches inside mark 10.
            let mark = format!("\"{}{n}\"", self.token);
            html = html.replacen(&mark, &format!("\"{}\"", escape(&new)), 1);
        }
        (html, missing)
    }

    /// Images as data URLs: one file carries all (self-contained).
    pub fn inline(&self, folder: &Path) -> (String, Vec<String>) {
        self.rewrite(folder, |path| {
            let bytes = std::fs::read(path).ok()?;
            let data = base64::engine::general_purpose::STANDARD.encode(bytes);
            Some(format!("data:{};base64,{data}", kind(path)))
        })
    }

    /// Images linked into the `assets` folder; and the copies.
    pub fn linked(
        &self,
        folder: &Path,
        assets: &str,
    ) -> (String, Vec<(PathBuf, String)>, Vec<String>) {
        let mut copies: Vec<(PathBuf, String)> = Vec::new();
        let (html, missing) = self.rewrite(folder, |path| {
            if !path.is_file() {
                return None;
            }
            if let Some((_, name)) = copies.iter().find(|(from, _)| from == path) {
                return Some(format!("{}/{}", segment(assets), segment(name)));
            }
            let file = path.file_name()?.to_string_lossy().into_owned();
            // Same names from different folders stay apart.
            let name = format!("{}-{file}", copies.len() + 1);
            let url = format!("{}/{}", segment(assets), segment(&name));
            copies.push((path.to_path_buf(), name));
            Some(url)
        });
        (html, copies, missing)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// HTML as the page marks it.
    fn marked(images: &[&str]) -> Marked {
        let token = "ely-image-t-".to_owned();
        let html = (0..images.len())
            .map(|n| format!("<img alt=\"a > b src='x'\" src=\"{token}{n}\">"))
            .collect();
        Marked {
            html,
            images: images.iter().map(|src| src.to_string()).collect(),
            token,
        }
    }

    #[test]
    fn images_inline_and_links_stay() {
        let dir = tempfile::tempdir().expect("a temp dir");
        std::fs::create_dir(dir.path().join("assets")).expect("assets");
        std::fs::write(dir.path().join("assets/a b.png"), b"png").expect("an image");
        let page = marked(&["assets/a%20b.png", "https://e.x/y.png", "gone.png"]);
        let (out, missing) = page.inline(dir.path());
        assert!(out.contains(r#"src="data:image/png;base64,cG5n""#), "{out}");
        assert!(out.contains(r#"src="https://e.x/y.png""#));
        assert!(
            out.contains(r#"alt="a > b src='x'""#),
            "alt text stays: {out}"
        );
        assert_eq!(missing, vec!["gone.png".to_string()]);
    }

    #[test]
    fn linked_images_copy_once_under_encoded_names() {
        let dir = tempfile::tempdir().expect("a temp dir");
        std::fs::write(dir.path().join("a#b.png"), b"1").expect("an image");
        let mut page = marked(&["a%23b.png"; 11]);
        page.images[10] = "a%23b.png".into();
        let (out, copies, missing) = page.linked(dir.path(), "report#1_files");
        assert_eq!(
            out.matches("src=\"report%231_files/1-a%23b.png\"").count(),
            11,
            "{out}"
        );
        assert_eq!(copies.len(), 1);
        assert!(missing.is_empty());
    }

    #[test]
    fn a_page_lacking_its_parts_is_an_error() {
        assert!(Marked::read(&serde_json::json!({ "html": "<p>" })).is_err());
    }

    #[gpui::test]
    fn the_print_page_is_written_off_the_window_thread(cx: &mut gpui::TestAppContext) {
        let dir = tempfile::tempdir().expect("a temp dir");
        std::fs::write(dir.path().join("a.png"), b"png").expect("an image");
        std::fs::write(dir.path().join("export.css"), "p {}").expect("a style");
        let rendered = serde_json::json!({
            "html": "<p>hi</p><img src=\"T0\"><img src=\"T1\">",
            "token": "T",
            "images": ["a.png", "gone.png"],
            "failed": [3],
        });
        let print = Print {
            folder: dir.path().into(),
            style: dir.path().join("export.css"),
            name: "notes".into(),
            page: dir.path().join("print-1.html"),
        };
        let staging = stage(&cx.executor(), rendered, print);
        assert!(!dir.path().join("print-1.html").exists(), "staged on the window thread");
        let lost = cx.foreground_executor().block_test(staging).expect("staged");
        let written = std::fs::read_to_string(dir.path().join("print-1.html")).expect("the page");
        assert!(written.contains("<style>p {}</style>") && written.contains("data:image/png;base64,cG5n"));
        assert_eq!(format!("{lost:?}"), r#"Lost { images: ["gone.png"], diagrams: [3], formulas: [] }"#);
    }

    #[test]
    fn titles_escape() {
        assert!(page("a<b>&", "", "").contains("<title>a&lt;b&gt;&amp;</title>"));
    }
}
