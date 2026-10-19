//! Page images, text, optimized copies, repair reports (PDF-13, PDF-14).

use std::{io, path::Path};

use ely_ipc::{Pack, Unpack};

use crate::client::{Doc, Worker};
use crate::proto::tag;

/// How an optimized copy is written.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Optimizing {
    pub compress: bool,
    /// Images above `from` dpi go to `to`; 0 keeps them.
    pub from: u32,
    pub to: u32,
}

fn utf8(path: &Path) -> io::Result<&str> {
    path.to_str().ok_or_else(|| io::Error::other("a path that is not UTF-8"))
}

impl Worker {
    /// Page `page` as a PNG at `dpi`, white as printed.
    pub fn export_image(&mut self, doc: Doc, page: usize, dpi: f32, path: &Path) -> io::Result<()> {
        let ask = self.on(&doc)?.u64(page as u64).u64(u64::from(dpi.to_bits())).str(utf8(path)?);
        self.request(tag::EXPORT_IMAGE, &ask.bytes()).map(|_| ())
    }

    /// Every page's text, UTF-8; form feeds part the pages.
    pub fn export_text(&mut self, doc: Doc, path: &Path) -> io::Result<()> {
        self.request(tag::EXPORT_TEXT, &self.whole(&doc)?.str(utf8(path)?).bytes()).map(|_| ())
    }

    pub fn optimize(&mut self, doc: Doc, out: &Path, how: Optimizing) -> io::Result<()> {
        let ask = self.whole(&doc)?.str(utf8(out)?).u64(u64::from(how.compress)).u64(u64::from(how.from)).u64(u64::from(how.to));
        self.request(tag::OPTIMIZE, &ask.bytes()).map(|_| ())
    }

    /// The document as it is now, written to print.
    pub fn print_copy(&mut self, doc: Doc, out: &Path) -> io::Result<()> {
        self.request(tag::PRINT_COPY, &self.whole(&doc)?.str(utf8(out)?).bytes()).map(|_| ())
    }

    /// HTML `page` in `dir`, with its pictures, as a PDF.
    pub fn html_pdf(&mut self, dir: &Path, page: &str, out: &Path) -> io::Result<()> {
        let ask = Pack::new().str(utf8(dir)?).str(page).str(utf8(out)?);
        self.request(tag::HTML_PDF, &ask.bytes()).map(|_| ())
    }

    /// Decodes every picture; true if any failed.
    pub fn check_pictures(&mut self, doc: Doc) -> io::Result<bool> {
        let reply = self.request(tag::CHECK_PICTURES, &self.whole(&doc)?.bytes())?;
        Ok(Unpack::new(&reply).u64()? != 0)
    }

    /// MuPDF's warnings if the file was repaired to open.
    pub fn repair_report(&mut self, doc: Doc) -> io::Result<Option<String>> {
        let reply = self.request(tag::REPAIR_REPORT, &self.whole(&doc)?.bytes())?;
        let mut reply = Unpack::new(&reply);
        Ok(match reply.u64()? {
            0 => None,
            _ => Some(reply.str()?),
        })
    }
}
