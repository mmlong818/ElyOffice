use ely_mail_engine::{Error, read};

/// Lines joined with CRLF, as servers send them.
fn crlf(lines: &[&str]) -> Vec<u8> {
    lines.join("\r\n").into_bytes()
}

/// `levels` multipart/mixed containers around one text part.
fn nested(levels: usize) -> Vec<u8> {
    let mut lines = Vec::new();
    for level in 0..levels {
        lines.push(format!("Content-Type: multipart/mixed; boundary=b{level}"));
        lines.push(String::new());
        lines.push(format!("--b{level}"));
    }
    lines.push("Content-Type: text/plain".into());
    lines.push(String::new());
    lines.push("deep".into());
    for level in (0..levels).rev() {
        lines.push(format!("--b{level}--"));
    }
    lines.push(String::new());
    lines.join("\r\n").into_bytes()
}

#[test]
fn deep_nesting_is_an_error_not_a_crash() {
    let err = read(&nested(2048)).unwrap_err();
    assert!(
        matches!(&err, Error::Mime(text) if text.contains("deeper than 64")),
        "{err}"
    );
    let mail = read(&nested(63)).unwrap();
    assert_eq!(mail.text.as_deref(), Some("deep"));
}

#[test]
fn multibyte_charsets_decode() {
    let mail = read(&crlf(&[
        "Subject: =?GB2312?B?1tDOxA==?=",
        "Content-Type: text/plain; charset=gb2312",
        "Content-Transfer-Encoding: base64",
        "",
        "1tDOxA==",
        "",
    ]))
    .unwrap();
    assert_eq!(mail.subject.as_deref(), Some("中文"));
    assert_eq!(mail.text.as_deref(), Some("中文"));
}

#[test]
fn a_related_root_marked_attachment_is_still_the_body() {
    let mail = read(&crlf(&[
        "Content-Type: multipart/related; boundary=r; type=\"text/html\"",
        "",
        "--r",
        "Content-Type: text/html",
        "Content-Disposition: attachment; filename=body.html",
        "",
        "<p>root</p>",
        "--r",
        "Content-Type: image/png",
        "Content-ID: <img@example.com>",
        "",
        "png",
        "--r--",
        "",
    ]))
    .unwrap();
    assert_eq!(mail.html.as_deref(), Some("<p>root</p>"));
    assert!(mail.attachments.is_empty());
    let ids: Vec<_> = mail
        .inline
        .iter()
        .map(|p| p.content_id.as_deref())
        .collect();
    assert_eq!(ids, [Some("img@example.com")]);
}

#[test]
fn a_composite_related_resource_keeps_its_entity() {
    let mail = read(&crlf(&[
        "Content-Type: multipart/related; boundary=r; type=\"text/html\"",
        "",
        "--r",
        "Content-Type: text/html",
        "",
        "<p>root</p>",
        "--r",
        "Content-Type: multipart/alternative; boundary=a",
        "Content-ID: <entity@example.com>",
        "Content-Disposition: inline; filename=entity.mime",
        "",
        "--a",
        "Content-Type: text/plain",
        "",
        "inner",
        "--a--",
        "--r--",
        "",
    ]))
    .unwrap();
    assert_eq!(mail.html.as_deref(), Some("<p>root</p>"));
    assert!(mail.attachments.is_empty());
    let [entity] = mail.inline.as_slice() else {
        panic!("{:?}", mail.inline)
    };
    assert_eq!(entity.content_id.as_deref(), Some("entity@example.com"));
    assert_eq!(entity.name.as_deref(), Some("entity.mime"));
    assert_eq!(entity.content_type, "multipart/alternative");
    let bytes = String::from_utf8(entity.bytes.clone()).unwrap();
    assert!(
        bytes.starts_with("Content-Type: multipart/alternative"),
        "{bytes}"
    );
    assert!(bytes.contains("inner"), "{bytes}");
}

#[test]
fn a_text_part_with_a_content_id_is_a_cid_target() {
    let mail = read(&crlf(&[
        "Content-Type: multipart/mixed; boundary=b",
        "",
        "--b",
        "Content-Type: text/plain",
        "",
        "body",
        "--b",
        "Content-Type: text/html; charset=utf-8",
        "Content-ID: <frame@example.com>",
        "Content-Disposition: inline",
        "",
        "<p>frame</p>",
        "--b--",
        "",
    ]))
    .unwrap();
    assert_eq!(mail.text.as_deref(), Some("body"));
    let [frame] = mail.inline.as_slice() else {
        panic!("{:?}", mail.inline)
    };
    assert_eq!(frame.content_id.as_deref(), Some("frame@example.com"));
    assert_eq!(frame.content_type, "text/html");
    assert_eq!(frame.params, [("charset".to_owned(), "utf-8".to_owned())]);
    assert_eq!(frame.bytes, b"<p>frame</p>");
}

#[test]
fn an_unknown_disposition_is_an_attachment() {
    let mail = read(&crlf(&[
        "Content-Type: multipart/mixed; boundary=b",
        "",
        "--b",
        "Content-Type: text/plain",
        "",
        "body",
        "--b",
        "Content-Type: text/plain",
        "Content-Disposition: x-file; filename=file.txt",
        "",
        "private attachment",
        "--b--",
        "",
    ]))
    .unwrap();
    assert_eq!(mail.text.as_deref(), Some("body"));
    let [file] = mail.attachments.as_slice() else {
        panic!("{:?}", mail.attachments)
    };
    assert_eq!(file.name.as_deref(), Some("file.txt"));
    assert_eq!(file.bytes, b"private attachment");
}
