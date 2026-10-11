//! What CEF's handlers leave the view; the page fence.

use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use cef::{wrapper::message_router::BrowserSideCallback, *};

pub(crate) type Shared = Arc<Mutex<Surface>>;

pub(crate) struct Surface {
    /// Points, as GPUI lays the view out.
    pub size: (f32, f32),
    pub scale: f32,
    pub frame: Option<(u32, u32, Vec<u8>)>,
    /// Composing characters' boxes, in points.
    pub ime: Vec<(f32, f32, f32, f32)>,
    pub cursor: CursorType,
    pub loaded: bool,
    /// The main frame's load failed since it started.
    pub errored: bool,
    pub queries: Vec<(String, Arc<Mutex<dyn BrowserSideCallback>>)>,
    pub leaving: Vec<String>,
    /// PDFs printed: where, and whether it worked.
    pub printed: Vec<(PathBuf, bool)>,
    /// Why the page cannot show: load, renderer or start.
    pub failed: Option<String>,
    /// Made on the pump's turn, after the view.
    pub browser: Option<Browser>,
    /// The view went before its browser came.
    pub closed: bool,
    /// Whether the page's scripts run.
    pub scripts: bool,
    /// The only folder pages load from, real path.
    root: PathBuf,
    /// A folder images alone may also come from.
    pub images: Option<PathBuf>,
}

impl Surface {
    pub fn new(root: PathBuf, scale: f32) -> Self {
        Surface {
            size: (1.0, 1.0),
            scale,
            frame: None,
            ime: Vec::new(),
            cursor: CursorType::POINTER,
            loaded: false,
            errored: false,
            queries: Vec::new(),
            leaving: Vec::new(),
            printed: Vec::new(),
            failed: None,
            browser: None,
            closed: false,
            scripts: true,
            root,
            images: None,
        }
    }

    /// A file under `root`, by its real path.
    fn holds(root: &Path, url: &str) -> bool {
        let Some(path) = url.strip_prefix("file://") else {
            return false;
        };
        let path = path.split(['?', '#']).next().unwrap_or_default();
        let Ok(path) = String::from_utf8(decoded(path)) else {
            return false;
        };
        #[cfg(windows)]
        let path = windows_path(&path);
        std::fs::canonicalize(path).is_ok_and(|real| real.starts_with(root))
    }

    /// Where the page may go: its own files, or blank.
    pub fn navigable(&self, url: &str) -> bool {
        url == "about:blank" || Self::holds(&self.root, url)
    }

    /// What a page loads: its files, data, allowed images.
    pub fn loadable(&self, url: &str, image: bool) -> bool {
        let pictured = image
            && self
                .images
                .as_deref()
                .is_some_and(|dir| Self::holds(dir, url));
        self.navigable(url)
            || url.starts_with("data:")
            || url.starts_with("blob:file://")
            || pictured
    }
}

impl Drop for Surface {
    fn drop(&mut self) {
        log::info!("web: a page's memory is freed");
    }
}

/// `%XX` decoded; a file URL's path.
fn decoded(text: &str) -> Vec<u8> {
    let bytes = text.as_bytes();
    let (mut out, mut at) = (Vec::with_capacity(bytes.len()), 0);
    while at < bytes.len() {
        let hex = bytes
            .get(at + 1..at + 3)
            .and_then(|pair| u8::from_str_radix(std::str::from_utf8(pair).ok()?, 16).ok());
        match (bytes[at], hex) {
            (b'%', Some(byte)) => {
                out.push(byte);
                at += 3;
            }
            (byte, _) => {
                out.push(byte);
                at += 1;
            }
        }
    }
    out
}

/// Bytes as URL text, reserved ones escaped.
fn escaped(bytes: &[u8]) -> String {
    let mut out = String::new();
    for &byte in bytes {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

/// A path as a file URL, every reserved byte escaped.
pub fn file_url(path: &Path) -> String {
    #[cfg(windows)]
    return windows_url(&path.to_string_lossy());
    #[cfg(not(windows))]
    format!("file://{}", escaped(path.as_os_str().as_encoded_bytes()))
}

/// A drive or UNC path as `file:///C:/…` or `file://host/…`.
#[cfg(any(windows, test))]
fn windows_url(path: &str) -> String {
    let path = match path.strip_prefix(r"\\?\UNC\") {
        Some(rest) => format!(r"\\{rest}"),
        None => path.strip_prefix(r"\\?\").unwrap_or(path).to_owned(),
    };
    let slashed = |text: &str| escaped(text.replace('\\', "/").as_bytes());
    match path.strip_prefix(r"\\") {
        Some(unc) => {
            let (host, rest) = unc.split_once('\\').unwrap_or((unc, ""));
            format!("file://{}/{}", slashed(host), slashed(rest))
        }
        None if path.get(1..2) == Some(":") => {
            format!("file:///{}{}", &path[..2], slashed(&path[2..]))
        }
        None => format!("file:///{}", slashed(&path)),
    }
}

/// A decoded file URL's rest as a Windows path.
#[cfg(any(windows, test))]
fn windows_path(rest: &str) -> String {
    match rest.strip_prefix('/') {
        Some(drive) => drive.replace('/', "\\"),
        None => format!(r"\\{}", rest.replace('/', "\\")),
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    fn surface(root: &Path) -> super::Surface {
        super::Surface::new(std::fs::canonicalize(root).expect("real"), 1.0)
    }

    #[test]
    fn only_real_files_under_the_root_load() {
        let dir = tempfile::tempdir().expect("a temp dir");
        let root = dir.path().join("content");
        let outside = dir.path().join("outside");
        std::fs::create_dir_all(&root).expect("makes");
        std::fs::create_dir_all(&outside).expect("makes");
        std::fs::write(root.join("index.html"), "x").expect("writes");
        std::fs::write(root.join("a #b?.html"), "x").expect("writes");
        std::fs::write(outside.join("o.html"), "x").expect("writes");
        let surface = surface(&root);
        let url = |path: &Path| super::file_url(&std::fs::canonicalize(path).expect("real"));
        assert!(surface.navigable(&url(&root.join("index.html"))));
        assert!(
            surface.navigable(&url(&root.join("a #b?.html"))),
            "reserved bytes"
        );
        assert!(surface.navigable("about:blank"));
        assert!(!surface.navigable(&url(&outside.join("o.html"))));
        let up = format!("{}/../outside/o.html", super::file_url(&surface.root));
        assert!(!surface.navigable(&up));
        assert!(!surface.navigable(&format!("{}/missing.html", super::file_url(&surface.root))));
        assert!(!surface.navigable("https://example.com/"));
        assert!(
            !surface.navigable("data:text/html,x")
                && surface.loadable("data:image/png;base64,x", true)
        );
        assert!(!surface.loadable("http://127.0.0.1:9/pixel", true));
    }

    #[test]
    fn an_image_folder_admits_images_alone() {
        let dir = tempfile::tempdir().expect("a temp dir");
        let (root, docs) = (dir.path().join("content"), dir.path().join("docs"));
        std::fs::create_dir_all(&root).expect("makes");
        std::fs::create_dir_all(&docs).expect("makes");
        std::fs::write(docs.join("a.png"), "x").expect("writes");
        let mut surface = surface(&root);
        let picture = super::file_url(&std::fs::canonicalize(docs.join("a.png")).expect("real"));
        assert!(
            !surface.loadable(&picture, true),
            "not before it is allowed"
        );
        surface.images = Some(std::fs::canonicalize(&docs).expect("real"));
        assert!(surface.loadable(&picture, true));
        assert!(!surface.loadable(&picture, false), "a script from there");
        assert!(!surface.navigable(&picture));
    }

    #[cfg(unix)]
    #[test]
    fn a_link_out_of_the_root_is_outside() {
        let dir = tempfile::tempdir().expect("a temp dir");
        let root = dir.path().join("content");
        let outside = dir.path().join("outside");
        std::fs::create_dir_all(&root).expect("makes");
        std::fs::create_dir_all(&outside).expect("makes");
        std::fs::write(outside.join("o.html"), "x").expect("writes");
        std::os::unix::fs::symlink(&outside, root.join("link")).expect("links");
        let surface = surface(&root);
        let through = format!("{}/link/o.html", super::file_url(&surface.root));
        assert!(!surface.navigable(&through));
    }

    #[test]
    fn windows_paths_make_valid_file_urls_and_back() {
        let cases = [
            (r"C:\Users\A b\doc.md", "file:///C:/Users/A%20b/doc.md"),
            (r"\\?\C:\notes\doc.md", "file:///C:/notes/doc.md"),
            (r"\\server\share\doc.md", "file://server/share/doc.md"),
            (r"\\?\UNC\server\share\doc.md", "file://server/share/doc.md"),
        ];
        for (path, url) in cases {
            assert_eq!(super::windows_url(path), url, "{path}");
        }
        assert_eq!(
            super::windows_path("/C:/Users/A b/doc.md"),
            r"C:\Users\A b\doc.md"
        );
        assert_eq!(
            super::windows_path("server/share/doc.md"),
            r"\\server\share\doc.md"
        );
    }

    #[test]
    fn percent_escapes_decode_and_bad_ones_stay() {
        assert_eq!(super::decoded("a%20b%e4%b8%ad"), "a b中".as_bytes());
        assert_eq!(super::decoded("100%"), b"100%");
        assert_eq!(super::decoded("%zz"), b"%zz");
        assert_eq!(
            super::file_url(Path::new("/a b/中#?.html")),
            "file:///a%20b/%E4%B8%AD%23%3F.html"
        );
    }
}
