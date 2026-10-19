use std::os::windows::ffi::OsStrExt as _;
use std::path::{Path, PathBuf};

use super::identity;

fn verbatim(path: &Path) -> bool {
    path.to_string_lossy().starts_with(r"\\?\")
}

#[test]
fn a_short_path_reads_plain() {
    let root = tempfile::tempdir().expect("a temp dir");
    let new = identity(&root.path().join("new.md")).expect("a target");
    assert!(!verbatim(&new), "{}", new.display());
}

#[test]
fn a_trailing_space_name_stays_verbatim() {
    let root = tempfile::tempdir().expect("a temp dir");
    let real = std::fs::canonicalize(root.path()).expect("real");
    let spaced = identity(&PathBuf::from(format!("{} ", real.join("guard.md").display())))
        .expect("a target");
    assert!(verbatim(&spaced), "{}", spaced.display());
    assert!(spaced.to_string_lossy().ends_with("guard.md "));
}

#[test]
fn a_long_new_name_stays_verbatim() {
    let root = tempfile::tempdir().expect("a temp dir");
    let dir = root.path().join("d".repeat(150));
    std::fs::create_dir(&dir).expect("a folder");
    let long = identity(&dir.join(format!("{}.md", "x".repeat(120)))).expect("a target");
    assert!(long.as_os_str().encode_wide().count() > 260);
    assert!(verbatim(&long), "{}", long.display());
}
