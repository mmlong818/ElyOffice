//! MuPDF in its own process: protocol and client (PDF-01).

mod client;
mod content;
mod edit;
mod marks;
mod ocr;
mod output;
mod redact;
mod secure;
mod sign;
mod structure;
pub mod proto;
#[cfg(any(test, feature = "test-support"))]
pub mod sample;

pub use client::{Doc, Opened, Refused, Tile, Worker, is_stale, refused};
pub use content::{Drawn, TextBlock, Writing, Wrote};
pub use ocr::{OcrLine, tessdata};
pub use output::Optimizing;
pub use redact::{Cleaning, Reach, Redacted, Region};
pub use secure::Lock;
pub use sign::{Digest, Signature, Signed, Signing};
pub use structure::{Attachment, BookmarkChange, Link, LinkChange, Properties, Target};
pub use marks::{Annot, Field, FieldKind, Marks, NewAnnot, NewField, Shape};
pub use proto::{Bookmark, Char};
