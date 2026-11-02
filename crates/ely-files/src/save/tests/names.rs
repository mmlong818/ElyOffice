use super::*;

#[test]
fn an_import_copies_and_never_deletes() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let source = dir.path().join("photo.png");
    write(&source, "png-bytes");
    let attachments = dir.path().join("attachments");
    let first = import_attachment(&source, &attachments).expect("the import lands");
    assert!(source.try_exists().expect("the source check"));
    assert_eq!(read(&source), "png-bytes");
    assert_eq!(read(&first), "png-bytes");
    let second = import_attachment(&source, &attachments).expect("the second import");
    assert_ne!(first, second);
    assert_eq!(read(&first), "png-bytes");
    assert_eq!(read(&second), "png-bytes");
}

#[test]
fn written_bytes_take_a_free_name() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let first = write_attachment("pasted.png", b"one", dir.path()).expect("the first write");
    let second = write_attachment("pasted.png", b"two", dir.path()).expect("the second write");
    assert_ne!(first, second);
    assert_eq!((read(&first), read(&second)), ("one".into(), "two".into()));
}

#[test]
fn a_numbered_import_name_fits_long_file_names() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let source = dir.path().join(format!("{}.png", "x".repeat(251)));
    write(&source, "png");
    let attachments = dir.path().join("attachments");
    import_attachment(&source, &attachments).expect("the first import");
    let second = import_attachment(&source, &attachments).expect("the second import");
    let len = second.file_name().expect("a name").len();
    assert!(len <= 255, "the numbered name fits: {len}");
    assert_eq!(read(&second), "png");
}

#[test]
fn written_bytes_stay_in_their_folder() {
    let root = tempfile::tempdir().expect("a temp dir");
    let dir = root.path().join("in");
    let outside = root.path().join("escaped.pdf").display().to_string();
    let names = ["../escaped.pdf", "a/../../escaped.pdf", "sub/escaped.pdf", "..", ".", ""];
    for name in names.into_iter().chain([outside.as_str()]) {
        let refused = write_attachment(name, b"x", &dir).expect_err(name);
        assert_eq!(refused.kind(), io::ErrorKind::InvalidInput, "{name}");
    }
    assert!(!root.path().join("escaped.pdf").exists());
    let kept: Vec<_> = std::fs::read_dir(root.path()).expect("the root").collect();
    assert_eq!(kept.len(), usize::from(dir.exists()), "only the folder itself");
}

#[cfg(windows)]
#[test]
fn windows_device_and_stream_names_are_refused() {
    let dir = tempfile::tempdir().expect("a temp dir");
    for name in ["NUL", "con.txt", "Com1.pdf", "a.pdf:stream", r"a\..\b.pdf"] {
        let refused = write_attachment(name, b"x", dir.path()).expect_err(name);
        assert_eq!(refused.kind(), io::ErrorKind::InvalidInput, "{name}");
    }
}
