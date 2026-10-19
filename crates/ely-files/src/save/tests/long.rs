use std::os::windows::ffi::OsStrExt as _;
use std::path::{Path, PathBuf};

use super::super::*;

/// UTF-16 units Windows counts in `path`.
fn units(path: &Path) -> usize {
    path.as_os_str().encode_wide().count()
}

/// A folder under `root` whose path is `want` units long.
fn folder_of(root: &Path, want: usize) -> PathBuf {
    let name = "d".repeat(want - units(root) - 1);
    let dir = root.join(name);
    std::fs::create_dir(&dir).expect("the long folder");
    assert_eq!(units(&dir), want);
    dir
}

#[test]
fn an_existing_short_file_in_a_long_folder_saves() {
    let root = tempfile::tempdir().expect("a temp dir");
    let target = folder_of(root.path(), 250).join("a.md");
    std::fs::write(&target, "old").expect("the file");
    save_atomic(&target, b"new", 1).expect("the staging file fits");
    assert_eq!(std::fs::read(&target).expect("reads"), b"new");
}

#[test]
fn a_new_file_past_the_old_limit_saves() {
    let root = tempfile::tempdir().expect("a temp dir");
    let dir = folder_of(root.path(), 200);
    let target = dir.join(format!("{}.md", "x".repeat(72 - 3)));
    assert_eq!(units(&target), 273);
    save_atomic(&target, b"long", 1).expect("the long file saves");
    assert_eq!(std::fs::read(&target).expect("reads"), b"long");
}

#[test]
fn a_trailing_space_name_never_overwrites_the_open_file() {
    let root = tempfile::tempdir().expect("a temp dir");
    let open = std::fs::canonicalize(root.path()).expect("real").join("guard.md");
    std::fs::write(&open, "keep").expect("the open file");
    let literal = PathBuf::from(format!("{} ", open.display()));
    assert!(literal.to_string_lossy().starts_with(r"\\?\"), "a verbatim path: {}", literal.display());
    export(&literal, Some(&open), b"exported", 1).expect("the export lands");
    assert_eq!(std::fs::read(&open).expect("reads"), b"keep");
    assert_eq!(std::fs::read(&literal).expect("the literal file"), b"exported");
}

#[test]
fn a_saved_session_keeps_the_plain_path_unless_it_needs_more() {
    let root = tempfile::tempdir().expect("a temp dir");
    let mut session = crate::Session::new_file();
    let short = root.path().join("short.md");
    let saved = save_as(&mut session, &short, b"short").expect("saves");
    assert!(!saved.path.to_string_lossy().starts_with(r"\\?\"), "{}", saved.path.display());
    assert_eq!(session.path(), Some(saved.path.as_path()));
    let dir = folder_of(root.path(), 200);
    let long = dir.join(format!("{}.md", "x".repeat(69)));
    let saved = save_as(&mut session, &long, b"long").expect("saves");
    assert!(saved.path.to_string_lossy().starts_with(r"\\?\"), "{}", saved.path.display());
    assert_eq!(std::fs::read(&saved.path).expect("reads"), b"long");
}
