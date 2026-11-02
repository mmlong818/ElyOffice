use ely_mail_engine::{Draft, Mail, Mailbox, build, read};
use mail_parser::{MessageParser, MimeHeaders, PartType};

/// Lines joined with CRLF, as servers send them.
fn crlf(lines: &[&str]) -> Vec<u8> {
    lines.join("\r\n").into_bytes()
}

/// A new message carrying `mail`'s attachments.
fn forward(mail: &Mail) -> Vec<u8> {
    let draft = Draft {
        from: Mailbox {
            name: None,
            email: "a@example.com".into(),
        },
        to: Vec::new(),
        cc: Vec::new(),
        bcc: Vec::new(),
        subject: "fwd".into(),
        text: "see attached".into(),
        html: None,
        attachments: mail.attachments.clone(),
        in_reply_to: None,
        references: Vec::new(),
        date: 0,
    };
    build(&draft).unwrap().bytes
}

#[test]
fn a_forwarded_text_attachment_keeps_its_charset() {
    let mail = read(&crlf(&[
        "Content-Type: multipart/mixed; boundary=b",
        "",
        "--b",
        "Content-Type: text/plain",
        "",
        "body",
        "--b",
        "Content-Type: text/plain; charset=iso-8859-1",
        "Content-Transfer-Encoding: quoted-printable",
        "Content-Disposition: attachment; filename=cafe.txt",
        "",
        "Caf=E9",
        "--b--",
        "",
    ]))
    .unwrap();
    let sent = forward(&mail);
    let message = MessageParser::default().parse(&sent).unwrap();
    let file = message.attachment(0).unwrap();
    assert_eq!(file.text_contents(), Some("Café"));
    assert_eq!(read(&sent).unwrap().attachments, mail.attachments);
}

#[test]
fn a_forwarded_multipart_stays_a_multipart() {
    let mail = read(&crlf(&[
        "Content-Type: multipart/mixed; boundary=b",
        "",
        "--b",
        "Content-Type: text/plain",
        "",
        "body",
        "--b",
        "Content-Type: multipart/alternative; boundary=a",
        "Content-Disposition: attachment; filename=alt.mime",
        "",
        "--a",
        "Content-Type: text/plain",
        "",
        "alt plain",
        "--a",
        "Content-Type: text/html",
        "",
        "<p>alt html</p>",
        "--a--",
        "--b--",
        "",
    ]))
    .unwrap();
    let sent = forward(&mail);
    let message = MessageParser::default().parse(&sent).unwrap();
    let alt = message
        .parts
        .iter()
        .find(|part| part.content_type().and_then(|t| t.subtype()) == Some("alternative"))
        .unwrap();
    assert_eq!(alt.attachment_name(), Some("alt.mime"));
    assert_eq!(alt.content_transfer_encoding(), Some("7bit"));
    let PartType::Multipart(children) = &alt.body else {
        panic!("{:?}", alt.body)
    };
    let texts: Vec<_> = children
        .iter()
        .map(|&id| message.parts[id as usize].text_contents())
        .collect();
    assert_eq!(texts, [Some("alt plain"), Some("<p>alt html</p>")]);

    let back = read(&sent).unwrap();
    assert_eq!(back.text.as_deref(), Some("see attached"));
    let [file] = back.attachments.as_slice() else {
        panic!("{:?}", back.attachments)
    };
    assert_eq!(file.content_type, "multipart/alternative");
    assert_eq!(file.params, mail.attachments[0].params);
}

#[test]
fn a_forwarded_message_stays_unencoded() {
    let mail = read(&crlf(&[
        "Content-Type: multipart/mixed; boundary=b",
        "",
        "--b",
        "Content-Type: text/plain",
        "",
        "body",
        "--b",
        "Content-Type: message/rfc822",
        "Content-Disposition: attachment; filename=old.eml",
        "",
        "Subject: old",
        "Content-Type: text/plain; charset=utf-8",
        "Content-Transfer-Encoding: 8bit",
        "",
        "您好",
        "--b--",
        "",
    ]))
    .unwrap();
    let sent = forward(&mail);
    let message = MessageParser::default().parse(&sent).unwrap();
    let old = message
        .parts
        .iter()
        .find(|part| matches!(part.body, PartType::Message(_)))
        .unwrap();
    assert_eq!(old.content_transfer_encoding(), Some("8bit"));
    let PartType::Message(inner) = &old.body else {
        unreachable!()
    };
    assert_eq!(inner.subject(), Some("old"));
    assert_eq!(inner.body_text(0).as_deref(), Some("您好"));
}

#[test]
fn a_forwarded_file_without_a_name_goes_unnamed() {
    let mail = read(&crlf(&[
        "Content-Type: multipart/mixed; boundary=b",
        "",
        "--b",
        "Content-Type: text/plain",
        "",
        "body",
        "--b",
        "Content-Type: application/pdf",
        "Content-Disposition: attachment",
        "Content-Transfer-Encoding: base64",
        "",
        "JVBERi0=",
        "--b--",
        "",
    ]))
    .unwrap();
    assert_eq!(mail.attachments.len(), 1);
    assert_eq!(mail.attachments[0].name, None);
    let sent = forward(&mail);
    let again = read(&sent).unwrap();
    assert_eq!(again.attachments.len(), 1, "the file still goes");
    assert_eq!(again.attachments[0].name, None);
    assert_eq!(again.attachments[0].bytes, b"%PDF-");
    assert_eq!(again.attachments[0].content_type, "application/pdf");
}
