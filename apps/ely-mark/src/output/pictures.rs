//! Where an exported page's pictures go.

use std::{
    fs::File,
    io,
    path::{Path, PathBuf},
};

use base64::Engine as _;

use super::address::scheme;

/// Where pictures go.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Pictures {
    /// Inside the page, as data.
    Inline,
    /// Copied into this new folder beside the page.
    Folder(PathBuf),
    /// Copied into this folder, in formats MuPDF reads.
    Pdf(PathBuf),
    /// Addresses as the text writes them.
    Written,
}

/// Picture formats MuPDF reads from files.
const READABLE: [&str; 8] = ["png", "jpg", "jpeg", "gif", "bmp", "tif", "tiff", "svg"];

/// Inline pictures MuPDF reads.
const READABLE_DATA: [&str; 3] = [
    "data:image/png;base64,",
    "data:image/jpeg;base64,",
    "data:image/gif;base64,",
];

pub(super) struct Placed<'a> {
    beside: Option<&'a Path>,
    pictures: &'a Pictures,
    /// Pictures copied so far, and their names.
    pub copied: Vec<(PathBuf, String)>,
    /// This export made the picture folder.
    pub made: bool,
}

/// A picture address's file; none for other schemes.
pub fn picture_file(url: &str, beside: Option<&Path>) -> Option<PathBuf> {
    if url.to_lowercase().starts_with("file://") {
        return Some(file_path(&url["file://".len()..]));
    }
    if scheme(url).is_some() {
        return None;
    }
    let path = PathBuf::from(decoded(url));
    match (path.is_absolute(), beside) {
        (true, _) => Some(path),
        (false, Some(beside)) => Some(beside.join(path)),
        (false, None) => None,
    }
}

impl<'a> Placed<'a> {
    pub fn new(beside: Option<&'a Path>, pictures: &'a Pictures) -> Self {
        Self {
            beside,
            pictures,
            copied: Vec::new(),
            made: false,
        }
    }

    pub fn pdf(&self) -> bool {
        matches!(self.pictures, Pictures::Pdf(_))
    }

    /// The `src` for `url`; none when it is not carried.
    pub fn place(&mut self, url: &str) -> io::Result<Option<String>> {
        if matches!(self.pictures, Pictures::Written) {
            return Ok(Some(url.to_owned()));
        }
        let lower = url.to_lowercase();
        if lower.starts_with("http://") || lower.starts_with("https://") {
            return Ok((!self.pdf()).then(|| url.to_owned()));
        }
        if lower.starts_with("data:") {
            let readable = match self.pdf() {
                true => READABLE_DATA.iter().any(|kind| lower.starts_with(kind)),
                false => lower.starts_with("data:image/"),
            };
            return Ok(readable.then(|| url.to_owned()));
        }
        match picture_file(url, self.beside) {
            Some(path) => self.carried(&path),
            None => Ok(None),
        }
    }

    fn carried(&mut self, path: &Path) -> io::Result<Option<String>> {
        let extension = path
            .extension()
            .map(|extension| extension.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        let folder = match self.pictures {
            Pictures::Inline => {
                let Ok(bytes) = std::fs::read(path) else {
                    return Ok(None);
                };
                let data = base64::engine::general_purpose::STANDARD.encode(bytes);
                return Ok(Some(format!("data:{};base64,{data}", mime(&extension))));
            }
            Pictures::Pdf(_) if !READABLE.contains(&extension.as_str()) => return Ok(None),
            Pictures::Folder(folder) | Pictures::Pdf(folder) => folder.clone(),
            Pictures::Written => unreachable!("written addresses are not carried"),
        };
        let named = match self.copied.iter().find(|(from, _)| from == path) {
            Some((_, name)) => name.clone(),
            None => {
                let Ok(mut source) = File::open(path) else {
                    return Ok(None);
                };
                if !source.metadata()?.is_file() {
                    return Ok(None);
                }
                let name = self.copy(&mut source, path, &folder)?;
                self.copied.push((path.to_path_buf(), name.clone()));
                name
            }
        };
        let shelf = folder
            .file_name()
            .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
        Ok(Some(format!("{}/{}", encoded(&shelf), encoded(&named))))
    }

    /// Copies `source` under a name still free.
    fn copy(&mut self, source: &mut File, path: &Path, folder: &Path) -> io::Result<String> {
        if !self.made {
            // A folder already there is someone's: refused.
            std::fs::create_dir(folder).map_err(|error| match error.kind() {
                io::ErrorKind::AlreadyExists => {
                    io::Error::new(error.kind(), folder.display().to_string())
                }
                _ => error,
            })?;
            self.made = true;
        }
        let file = path
            .file_name()
            .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
        let (stem, extension) = file.rsplit_once('.').unwrap_or((file.as_str(), ""));
        for count in 1.. {
            let name = match (count, extension) {
                (1, _) => file.clone(),
                (_, "") => format!("{stem}-{count}"),
                _ => format!("{stem}-{count}.{extension}"),
            };
            // The file system judges which names are the same.
            match File::create_new(folder.join(&name)) {
                Ok(mut kept) => {
                    io::copy(source, &mut kept)?;
                    return Ok(name);
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error),
            }
        }
        unreachable!("names never run out")
    }
}

fn mime(extension: &str) -> &'static str {
    match extension {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        "tif" | "tiff" => "image/tiff",
        "avif" => "image/avif",
        _ => "application/octet-stream",
    }
}

/// A `file:` address's path; a drive keeps no leading slash.
fn file_path(rest: &str) -> PathBuf {
    let path = decoded(rest.strip_prefix("localhost").unwrap_or(rest));
    let drive = path.len() > 2 && path.as_bytes()[2] == b':' && path.starts_with('/');
    PathBuf::from(if drive { &path[1..] } else { &path })
}

/// `%XX` undone; bytes that are not UTF-8 stay as written.
fn decoded(url: &str) -> String {
    let bytes = url.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        let hex = bytes
            .get(at + 1..at + 3)
            .and_then(|pair| std::str::from_utf8(pair).ok());
        match (
            bytes[at],
            hex.and_then(|pair| u8::from_str_radix(pair, 16).ok()),
        ) {
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
    String::from_utf8(out).unwrap_or_else(|_| url.to_owned())
}

/// A name as one path segment of a URL.
fn encoded(name: &str) -> String {
    name.bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                (byte as char).to_string()
            }
            _ => format!("%{byte:02X}"),
        })
        .collect()
}
