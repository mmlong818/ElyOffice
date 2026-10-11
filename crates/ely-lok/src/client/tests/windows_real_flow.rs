//! Windows-only real Office format flows against the accepted packaged runtime.

use super::*;
use std::{
    path::{Path, PathBuf},
    process::Command,
};

const DOCX_SHA256: &str = "30D747345CF2B03869A9B575AA37F6AD1DCB6E00D10399ABAB31A27D7B7E137F";
const XLSX_SHA256: &str = "8A78B75275834A9F475015A943EFCC265614E9A019BD95950D6020504D326723";
const PPTX_SHA256: &str = "8D182EAE8A01B57B7A37045D513166AC250D14459FE05D4B1864B77A6F8B6912";
const MERGEDLO_SHA256: &str = "FE1109F8C6831CF1C10D29D4F65683A54BC022D7A587520D504D5BBB46A90F70";
const EVENT_TIMEOUT_MS: u64 = 10_000;

struct FixtureCase {
    dir: tempfile::TempDir,
    fixture: PathBuf,
    profile: PathBuf,
    output: PathBuf,
    runtime: PathBuf,
}

fn hash(path: &Path) -> String {
    let result = Command::new("certutil.exe")
        .args(["-hashfile"])
        .arg(path)
        .arg("SHA256")
        .output()
        .unwrap_or_else(|error| panic!("certutil.exe must hash {}: {error}", path.display()));
    assert!(
        result.status.success(),
        "certutil.exe failed for {}: {}",
        path.display(),
        String::from_utf8_lossy(&result.stderr)
    );
    let output = String::from_utf8_lossy(&result.stdout);
    output
        .lines()
        .map(str::trim)
        .find(|line| line.len() == 64 && line.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .map(str::to_ascii_uppercase)
        .unwrap_or_else(|| {
            panic!(
                "certutil returned no SHA-256 for {}: {output}",
                path.display()
            )
        })
}

fn required_env(name: &str) -> PathBuf {
    let value = std::env::var_os(name)
        .unwrap_or_else(|| panic!("required Windows flow environment variable {name} is unset"));
    let path = PathBuf::from(value);
    assert!(
        path.is_absolute(),
        "{name} must be absolute: {}",
        path.display()
    );
    path
}

fn fixture_case(filename: &str, expected_hash: &str) -> FixtureCase {
    let runtime = required_env("ELY_LOK_DIR");
    assert!(
        runtime.is_dir(),
        "runtime directory is missing: {}",
        runtime.display()
    );
    let mergedlo = runtime.join("mergedlo.dll");
    assert!(mergedlo.is_file(), "runtime lacks {}", mergedlo.display());
    assert_eq!(
        hash(&mergedlo),
        MERGEDLO_SHA256,
        "accepted mergedlo.dll changed"
    );

    let worker = required_env("ELY_LOK_WORKER");
    assert!(
        worker.is_file(),
        "fresh worker is missing: {}",
        worker.display()
    );
    let temp = required_env("TEMP");
    let temp_text = temp.to_string_lossy();
    assert!(
        temp_text.len() >= 3 && temp_text[..3].eq_ignore_ascii_case("D:\\"),
        "TEMP must be on D:, got {}",
        temp.display()
    );
    let output_root = required_env("ELY_LOK_TEST_OUTPUT_DIR");
    assert!(
        output_root.starts_with(&temp),
        "test output must be below TEMP on D:"
    );

    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/office")
        .join(filename);
    assert!(
        source.is_file(),
        "source fixture is missing: {}",
        source.display()
    );
    assert_eq!(
        hash(&source),
        expected_hash,
        "tracked source fixture SHA-256 changed"
    );

    let fixture = required_env("ELY_LOK_TEST_FIXTURE_COPY");
    assert_eq!(fixture.file_name(), Path::new(filename).file_name());
    assert!(fixture.starts_with(&temp), "fixture copy must be on D:");
    assert!(
        fixture.is_file(),
        "fixture copy is missing: {}",
        fixture.display()
    );
    assert_eq!(
        hash(&fixture),
        expected_hash,
        "isolated fixture copy SHA-256 mismatch"
    );
    let lock_name = format!(".~lock.{filename}#");
    assert!(
        !fixture.with_file_name(lock_name).exists(),
        "isolated fixture has a lock sibling: {}",
        fixture.display()
    );
    let fixture_dir = fixture.parent().expect("fixture has a parent directory");
    let entries = std::fs::read_dir(fixture_dir)
        .unwrap_or_else(|error| panic!("fixture directory lists: {error}"))
        .map(|entry| entry.expect("fixture directory entry").file_name())
        .collect::<Vec<_>>();
    assert_eq!(entries.len(), 1, "only one primary fixture may be staged");
    assert_eq!(entries[0], std::ffi::OsString::from(filename));

    let dir = tempfile::tempdir_in(&output_root).expect("D: test workspace");
    let profile = dir.path().join("profile");
    let extension = Path::new(filename).extension().unwrap().to_string_lossy();
    let isolated_output = dir.path().join(format!("edited.{extension}"));
    FixtureCase {
        dir,
        fixture,
        profile,
        output: isolated_output,
        runtime,
    }
}

fn worker(case: &FixtureCase) -> Worker {
    Worker::spawn(&case.runtime, &case.profile).expect("fresh packaged worker starts")
}

fn event_baseline(worker: &mut Worker, doc: &Doc) -> Events {
    worker
        .poll_events(doc, 0, 0)
        .expect("read current event baseline without waiting")
}

fn assert_event_after_edit(worker: &mut Worker, doc: &Doc, before: &Events, target_revision: u64) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(EVENT_TIMEOUT_MS);
    let mut known = before.seq;
    loop {
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        assert!(
            !remaining.is_zero(),
            "edit event timeout at revision {target_revision}"
        );
        let timeout_ms = remaining.as_millis().clamp(1, EVENT_TIMEOUT_MS as u128) as u64;
        let events = worker
            .poll_events(doc, known, timeout_ms)
            .expect("worker events after edit");
        if events.seq > before.seq && events.revision >= target_revision {
            return;
        }
        known = events.seq;
    }
}

fn assert_revision_advanced(worker: &mut Worker, doc: &Doc, before: u64) -> u64 {
    let after = worker
        .now(doc)
        .expect("read document revision after edit")
        .rev;
    assert!(
        after > before,
        "edit advances document revision: {before} -> {after}"
    );
    after
}

fn close_reopen(worker: &mut Worker, doc: Doc, path: &Path, kind: &str) -> Doc {
    worker
        .close_doc(doc)
        .expect("edited source document closes");
    worker
        .load(&crate::file_url(path))
        .unwrap_or_else(|error| panic!("saved {kind} reopens: {error}"))
}

fn finish(mut worker: Worker, doc: Doc, case: &FixtureCase, output_kind: &str) {
    worker.close_doc(doc).expect("reopened document closes");
    worker
        .shutdown()
        .expect("packaged worker shuts down cleanly");
    assert!(worker.ended().is_some(), "the worker PID was reaped");
    assert_eq!(
        worker
            .supervisor
            .active_processes()
            .expect("query worker Job Object"),
        0,
        "no worker process remains in its owned Job Object"
    );
    assert!(
        !case.profile.join(".lock").exists(),
        "no LibreOffice profile lock remains"
    );
    if case.profile.join("tmp").exists() {
        let leftovers = std::fs::read_dir(case.profile.join("tmp"))
            .expect("profile temp directory lists")
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().starts_with("lu"))
            .collect::<Vec<_>>();
        assert!(
            leftovers.is_empty(),
            "no LibreOffice temp files remain: {leftovers:?}"
        );
    }
    let source_lock = case.fixture.with_file_name(format!(
        ".~lock.{}#",
        case.fixture.file_name().unwrap().to_string_lossy()
    ));
    assert!(!source_lock.exists(), "no staged fixture lock remains");
    let output_lock = case.output.with_file_name(format!(
        ".~lock.{}#",
        case.output.file_name().unwrap().to_string_lossy()
    ));
    assert!(!output_lock.exists(), "no saved {output_kind} lock remains");
}

#[test]
fn docx_edits_saves_reopens_and_shuts_down() {
    let case = fixture_case("t.docx", DOCX_SHA256);
    let mut worker = worker(&case);
    let doc = worker
        .load(&crate::file_url(&case.fixture))
        .expect("isolated DOCX loads");
    let baseline_events = event_baseline(&mut worker, &doc);
    let before_revision = worker
        .now(&doc)
        .expect("read DOCX revision before edit")
        .rev;
    click(&mut worker, &doc, 150, 150);
    worker
        .run(&doc, "edit.select_all", &[])
        .expect("select DOCX text");
    worker
        .paste(
            &doc,
            "text/plain;charset=utf-8",
            b"ElyOffice DOCX persisted edit",
        )
        .expect("replace selected DOCX text");
    let edit_revision = assert_revision_advanced(&mut worker, &doc, before_revision);
    assert_event_after_edit(&mut worker, &doc, &baseline_events, edit_revision);
    doc.save_as(&mut worker, &crate::file_url(&case.output), "docx")
        .expect("edited DOCX saves in its original format");
    let reopened = close_reopen(&mut worker, doc, &case.output, "DOCX");
    click(&mut worker, &reopened, 150, 150);
    worker
        .run(&reopened, "edit.select_all", &[])
        .expect("select reopened DOCX text");
    assert_eq!(
        reopened
            .selection_text(&mut worker)
            .expect("reopened DOCX text"),
        "ElyOffice DOCX persisted edit",
        "the edit persists in the reopened DOCX model"
    );
    finish(worker, reopened, &case, "DOCX");
}

#[test]
fn xlsx_edits_saves_reopens_and_shuts_down() {
    let case = fixture_case("t.xlsx", XLSX_SHA256);
    let mut worker = worker(&case);
    let doc = worker
        .load(&crate::file_url(&case.fixture))
        .expect("isolated XLSX loads");
    let baseline_events = event_baseline(&mut worker, &doc);
    let before_revision = worker
        .now(&doc)
        .expect("read XLSX revision before edit")
        .rev;
    worker
        .run(&doc, "cell.goto", &["A1"])
        .expect("select workbook A1");
    worker
        .run(&doc, "cell.enter", &["ElyOffice XLSX persisted edit"])
        .expect("edit workbook A1");
    let edit_revision = assert_revision_advanced(&mut worker, &doc, before_revision);
    assert_event_after_edit(&mut worker, &doc, &baseline_events, edit_revision);
    doc.save_as(&mut worker, &crate::file_url(&case.output), "xlsx")
        .expect("edited XLSX saves in its original format");
    let reopened = close_reopen(&mut worker, doc, &case.output, "XLSX");
    worker
        .run(&reopened, "cell.goto", &["A1"])
        .expect("select reopened A1");
    assert_eq!(
        reopened
            .selection_text(&mut worker)
            .expect("reopened A1 value"),
        "ElyOffice XLSX persisted edit",
        "the edited cell persists in the reopened XLSX model"
    );
    finish(worker, reopened, &case, "XLSX");
}

fn show_slides(worker: &mut Worker, doc: &Doc) -> Vec<(String, bool)> {
    doc.sheets(worker)
        .expect("presentation model reads")
        .list
        .into_iter()
        .map(|slide| (slide.name, slide.visible))
        .collect()
}

#[test]
fn pptx_edits_duplicates_hides_saves_reopens_and_shuts_down() {
    let case = fixture_case("t.pptx", PPTX_SHA256);
    let mut worker = worker(&case);
    let doc = worker
        .load(&crate::file_url(&case.fixture))
        .expect("isolated PPTX loads");
    let baseline_events = event_baseline(&mut worker, &doc);
    let before_revision = worker
        .now(&doc)
        .expect("read PPTX revision before edit")
        .rev;
    let baseline = doc.sheets(&mut worker).expect("initial slide list");
    assert!(
        baseline.list.len() >= 2,
        "the presentation has multiple slides"
    );
    doc.show_sheet(&mut worker, 0).expect("show first slide");
    // t.pptx's first slide has an empty title placeholder centered at these twips.
    click(&mut worker, &doc, 10_764, 3_402);
    let title = "ElyOffice PPTX persisted edit";
    for key in title.chars() {
        worker
            .post_key(&doc, key as u32, 0)
            .expect("type title text");
    }
    let original_first = baseline.list[0].name.clone();
    worker
        .run(&doc, "slide.duplicate", &[])
        .expect("duplicate first slide");
    let duplicated = doc.sheets(&mut worker).expect("duplicated slide list");
    assert_eq!(
        duplicated.list.len(),
        baseline.list.len() + 1,
        "one slide was duplicated"
    );
    assert!(
        baseline
            .list
            .iter()
            .enumerate()
            .all(|(index, slide)| baseline.list[..index]
                .iter()
                .all(|earlier| earlier.name != slide.name)),
        "the fixture's original slide names are unique"
    );
    let new_names = duplicated
        .list
        .iter()
        .filter(|slide| {
            !baseline
                .list
                .iter()
                .any(|original| original.name == slide.name)
        })
        .map(|slide| slide.name.clone())
        .collect::<Vec<_>>();
    assert_eq!(
        new_names.len(),
        1,
        "duplicating adds one uniquely named slide"
    );
    let duplicate_name = new_names.into_iter().next().expect("duplicate name");
    worker
        .run(&doc, "slide.last", &[])
        .expect("move duplicate to end");
    let moved = doc.sheets(&mut worker).expect("moved slide list");
    assert_eq!(moved.list.last().expect("last slide").name, duplicate_name);
    assert_eq!(
        moved.list.first().expect("first slide").name,
        original_first
    );
    worker
        .run(&doc, "slide.hide", &[])
        .expect("hide duplicate at end");
    let expected = show_slides(&mut worker, &doc);
    assert!(
        !expected.last().expect("last slide").1,
        "moved duplicate is hidden"
    );
    let edit_revision = assert_revision_advanced(&mut worker, &doc, before_revision);
    assert_event_after_edit(&mut worker, &doc, &baseline_events, edit_revision);
    doc.save_as(&mut worker, &crate::file_url(&case.output), "pptx")
        .expect("edited PPTX saves in its original format");
    let reopened = close_reopen(&mut worker, doc, &case.output, "PPTX");
    let actual = show_slides(&mut worker, &reopened);
    assert_eq!(
        actual.first().map(|slide| slide.0.as_str()),
        Some(title),
        "the reopened first slide carries the edited title"
    );
    let mut expected_after_reopen = expected.clone();
    expected_after_reopen[0].0 = title.to_string();
    assert_eq!(
        actual, expected_after_reopen,
        "slide order and hidden state persist; the first slide name reflects its saved title"
    );
    let flat = case.dir.path().join("reopened.fodp");
    reopened
        .save_as(&mut worker, &crate::file_url(&flat), "fodp")
        .expect("reopened presentation exports for text assertion");
    let xml = std::fs::read_to_string(flat).expect("reopened presentation XML");
    assert!(xml.contains(title), "slide text persists after reopen");
    finish(worker, reopened, &case, "PPTX");
}
