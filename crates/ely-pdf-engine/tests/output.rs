//! Images, text, optimized copies, printing leave, repaired files (C11.8).

use std::path::Path;

use ely_pdf_engine::{
    Doc, Lock, Optimizing, Worker,
    proto::may,
    sample::{Extras, one_page, pdf_with},
};

fn worker() -> Worker {
    Worker::spawn_from(Path::new(env!("CARGO_BIN_EXE_ely-pdf-worker"))).expect("the worker")
}

fn opened(path: &Path) -> (Worker, Doc) {
    let mut worker = worker();
    let doc = worker.open(path).expect("opens").doc;
    (worker, doc)
}

fn two_pages(dir: &Path) -> std::path::PathBuf {
    let path = dir.join("doc.pdf");
    std::fs::write(&path, pdf_with(&[(612.0, 792.0, &["First page"]), (612.0, 792.0, &["Second page"])], &[], &Extras::default())).expect("writes");
    path
}

#[test]
fn a_page_exports_as_a_png_of_its_size() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let (mut worker, doc) = opened(&two_pages(dir.path()));
    let png = dir.path().join("page.png");
    worker.export_image(doc, 1, 144.0, &png).expect("exports");
    let bytes = std::fs::read(&png).expect("reads");
    assert_eq!(&bytes[1..4], b"PNG");
    let size = |at: usize| u32::from_be_bytes(bytes[at..at + 4].try_into().expect("four bytes"));
    assert_eq!((size(16), size(20)), (1224, 1584), "twice the points");
    assert!(worker.export_image(doc, 0, 5000.0, &png).is_err(), "an absurd resolution");
}

#[test]
fn text_exports_page_by_page() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let (mut worker, doc) = opened(&two_pages(dir.path()));
    let txt = dir.path().join("doc.txt");
    worker.export_text(doc, &txt).expect("exports");
    let text = std::fs::read_to_string(&txt).expect("UTF-8");
    let pages: Vec<&str> = text.split('\u{c}').collect();
    assert_eq!(pages.len(), 2);
    assert!(pages[0].contains("First page") && pages[1].contains("Second page"), "{text:?}");
}

/// A 600-pixel picture drawn 100 points wide: 432 dpi.
fn heavy(dir: &Path) -> std::path::PathBuf {
    let pixels = vec![120u8; 600 * 600 * 3];
    let mut picture = format!("<< /Type /XObject /Subtype /Image /Width 600 /Height 600 /ColorSpace /DeviceRGB /BitsPerComponent 8 /Length {} >>\nstream\n", pixels.len()).into_bytes();
    picture.extend_from_slice(&pixels);
    picture.extend_from_slice(b"\nendstream");
    let path = dir.join("heavy.pdf");
    let mut bytes = one_page("", " /Resources << /XObject << /Im 5 0 R >> >>", "q 100 0 0 100 72 600 cm /Im Do Q", &["PICTURE".to_string()]);
    let at = bytes.windows(7).position(|w| w == b"PICTURE").expect("the slot");
    bytes.splice(at..at + 7, picture);
    std::fs::write(&path, &bytes).expect("writes");
    path
}

#[test]
fn an_optimized_copy_is_smaller_and_its_picture_downsampled() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let source = heavy(dir.path());
    let (mut worker, doc) = opened(&source);
    let out = dir.path().join("small.pdf");
    worker.optimize(doc, &out, Optimizing { compress: true, from: 300, to: 150 }).expect("optimizes");
    let (before, after) = (std::fs::metadata(&source).expect("size").len(), std::fs::metadata(&out).expect("size").len());
    assert!(after * 10 < before, "{before} -> {after}");
    let mut other = self::worker();
    let read = other.open(&out).expect("opens").doc;
    assert_eq!(other.pages(read).expect("pages"), 1);
    let text = String::from_utf8_lossy(&std::fs::read(&out).expect("reads")).into_owned();
    let width: u32 = text.split("/Width ").nth(1).and_then(|rest| rest.split(|c: char| !c.is_ascii_digit()).next()).and_then(|n| n.parse().ok()).expect("a width");
    assert!((205..=212).contains(&width), "150 dpi over 100 points, not {width}");
    assert!(worker.optimize(doc, &out, Optimizing { compress: true, from: 150, to: 300 }).is_err(), "up is not down");
}

#[test]
fn printing_and_copying_need_leave() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let (mut worker, doc) = opened(&two_pages(dir.path()));
    let limited = dir.path().join("limited.pdf");
    worker.protect(doc, &limited, Some(&Lock { user: String::new(), owner: "owner".into(), allow: may::EDIT })).expect("protects");
    let (mut other, read) = opened(&limited);
    for refused in [other.print_copy(read, &dir.path().join("p.pdf")), other.export_text(read, &dir.path().join("t.txt"))] {
        let refused = refused.expect_err("refused");
        assert!(ely_pdf_engine::refused(&refused).is_some(), "refused: {refused}");
    }
    worker.print_copy(doc, &dir.path().join("print.pdf")).expect("an unprotected file prints");
}

#[test]
fn a_damaged_file_opens_as_a_repaired_copy_and_stays_as_it_was() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let path = two_pages(dir.path());
    let mut bytes = std::fs::read(&path).expect("reads");
    // Breaks every cross-reference offset.
    let at = bytes.windows(9).rposition(|w| w == b"startxref").expect("the trailer");
    bytes.truncate(at);
    bytes.extend_from_slice(b"startxref\n9\n%%EOF\n");
    std::fs::write(&path, &bytes).expect("writes");
    let (mut worker, mut doc) = opened(&path);
    let report = worker.repair_report(doc).expect("asks").expect("repaired");
    assert!(!report.trim().is_empty(), "MuPDF says what it found");
    assert_eq!(worker.pages(doc).expect("pages"), 2);
    worker.rotate_pages(doc, &[0], 90).expect("edits the copy");
    doc = worker.now(doc).expect("the state");
    worker.save(doc, &dir.path().join("fixed.pdf")).expect("saves the copy");
    assert_eq!(std::fs::read(&path).expect("reads"), bytes, "the damaged original stays");
    let elsewhere = tempfile::tempdir().expect("a temp dir");
    let (mut fresh, clean) = opened(&two_pages(elsewhere.path()));
    assert_eq!(fresh.repair_report(clean).expect("asks"), None);
}

#[test]
fn a_broken_object_with_a_sound_trailer_is_repaired_on_open() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let path = two_pages(dir.path());
    let mut bytes = std::fs::read(&path).expect("reads");
    // The page tree's offset points nowhere; startxref is sound.
    let table = bytes.windows(6).position(|w| w == b"\nxref\n").expect("the table") + 6;
    let entries = table + bytes[table..].iter().position(|b| *b == b'\n').expect("a header") + 1;
    let at = entries + 2 * 20;
    bytes[at..at + 10].copy_from_slice(b"0000000009");
    std::fs::write(&path, &bytes).expect("writes");
    let (mut worker, mut doc) = opened(&path);
    assert!(worker.repair_report(doc).expect("asks").is_some(), "found at open, not mid-edit");
    worker.rotate_pages(doc, &[0], 90).expect("edits");
    doc = worker.now(doc).expect("the state");
    worker.save(doc, &dir.path().join("fixed.pdf")).expect("saves");
    assert_eq!(std::fs::read(&path).expect("reads"), bytes);
}

#[test]
fn a_broken_stream_under_a_sound_table_is_damage_and_fails_exports() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let broken = "<< /Length 12 /Filter /FlateDecode >>\nstream\nnot a zlib!!\nendstream".to_string();
    let path = dir.path().join("doc.pdf");
    let bytes = one_page("", " /Contents 5 0 R", "", &[broken]);
    std::fs::write(&path, &bytes).expect("writes");
    let (mut worker, mut doc) = opened(&path);
    let report = worker.repair_report(doc).expect("asks").expect("damage is reported");
    assert!(!report.trim().is_empty());
    assert!(worker.export_text(doc, &dir.path().join("doc.txt")).is_err(), "partial text is no export");
    worker.rotate_pages(doc, &[0], 90).expect("edits the copy");
    doc = worker.now(doc).expect("the state");
    worker.save(doc, &dir.path().join("fixed.pdf")).expect("saves the copy");
    assert_eq!(std::fs::read(&path).expect("reads"), bytes, "the original stays");
}

#[test]
fn a_print_copy_carries_no_lock() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let (mut worker, doc) = opened(&two_pages(dir.path()));
    let locked = dir.path().join("locked.pdf");
    worker.protect(doc, &locked, Some(&Lock { user: "user".into(), owner: "owner".into(), allow: may::PRINT })).expect("protects");
    let (mut other, read) = opened(&locked);
    assert!(other.unlock(read, "user").expect("tries"));
    let print = dir.path().join("print.pdf");
    other.print_copy(read, &print).expect("writes");
    let mut third = self::worker();
    let opened = third.open(&print).expect("opens");
    assert!(!opened.locked, "the printer needs no password");
    assert_eq!(third.pages(opened.doc).expect("pages"), 2);
}

#[test]
fn a_broken_picture_shows_when_drawn_or_checked() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let picture = "<< /Type /XObject /Subtype /Image /Width 4 /Height 4 /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /FlateDecode /Length 12 >>\nstream\nnot a zlib!!\nendstream".to_string();
    let path = dir.path().join("doc.pdf");
    let bytes = one_page("", " /Resources << /XObject << /Im 5 0 R >> >>", "q 100 0 0 100 72 600 cm /Im Do Q", &[picture]);
    std::fs::write(&path, &bytes).expect("writes");
    let (mut worker, doc) = opened(&path);
    assert_eq!(worker.repair_report(doc).expect("asks"), None, "pictures wait to be drawn");
    let tile = worker.render(doc, 0, (1.0, 0), [0, 0, 612, 792], false).expect("draws");
    assert!(tile.damaged, "drawing finds it");
    assert!(worker.repair_report(doc).expect("asks").is_some());
    let (mut other, again) = opened(&path);
    assert!(other.check_pictures(again).expect("checks"), "the check finds it undrawn");
    assert_eq!(std::fs::read(&path).expect("reads"), bytes);
}
