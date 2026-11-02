use ely_mail_engine::{Draft, Error, Mail, Mailbox, Part, build, read};

/// The sample as a server sends it: CRLF lines.
fn sample() -> Vec<u8> {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/mail/sample.eml"
    );
    let text = std::fs::read_to_string(path).unwrap();
    text.replace("\r\n", "\n")
        .replace('\n', "\r\n")
        .into_bytes()
}

fn mailbox(name: Option<&str>, email: &str) -> Mailbox {
    Mailbox {
        name: name.map(str::to_owned),
        email: email.to_owned(),
    }
}

#[test]
fn the_sample_reads_whole() {
    let mail = read(&sample()).unwrap();
    assert_eq!(mail.subject.as_deref(), Some("Café report"));
    assert_eq!(mail.from, [mailbox(Some("张三"), "zhang@example.com")]);
    assert_eq!(
        mail.to,
        [
            mailbox(Some("Bo Smith"), "bo@example.com"),
            mailbox(None, "cy@example.org")
        ]
    );
    assert_eq!(mail.cc, [mailbox(None, "dee@example.net")]);
    assert_eq!(mail.date, Some(1_792_391_400));
    assert_eq!(mail.message_id.as_deref(), Some("m1@example.com"));
    assert_eq!(mail.in_reply_to, ["m0@example.com"]);
    assert_eq!(mail.references, ["r1@example.com", "m0@example.com"]);
    assert_eq!(mail.text.as_deref(), Some("Café numbers attached."));
    assert_eq!(
        mail.html.as_deref(),
        Some(r#"<p>Caf&eacute; <img src="cid:logo@example.com"></p>"#)
    );
    assert_eq!(
        mail.inline,
        [Part {
            name: None,
            content_type: "image/png".into(),
            params: Vec::new(),
            content_id: Some("logo@example.com".into()),
            bytes: b"\x89PNG\r\n\x1a\n".to_vec(),
            section: Some("1.2".into()),
        }]
    );
    assert_eq!(
        mail.attachments,
        [Part {
            name: Some("report.pdf".into()),
            content_type: "application/pdf".into(),
            params: Vec::new(),
            content_id: Some("att@example.com".into()),
            bytes: b"%PDF-1.4\n".to_vec(),
            section: Some("2".into()),
        }]
    );
    assert!(
        mail.headers
            .starts_with("Return-Path: <zhang@example.com>\r\n")
    );
    assert!(mail.headers.contains("X-Custom: kept as sent\r\n"));
    assert!(!mail.headers.contains("--mix"));
}

fn reply_to(mail: &Mail) -> Draft {
    let every_byte: Vec<u8> = (0..=255).collect();
    let mut attachments = mail.attachments.clone();
    attachments.push(Part {
        name: Some("bytes.bin".into()),
        content_type: "application/octet-stream".into(),
        params: Vec::new(),
        content_id: None,
        bytes: every_byte,
        section: None,
    });
    Draft {
        from: mail.from[0].clone(),
        to: mail.to.clone(),
        cc: mail.cc.clone(),
        bcc: Vec::new(),
        subject: format!("Re: {}", mail.subject.as_deref().unwrap()),
        text: mail.text.clone().unwrap(),
        html: mail.html.clone(),
        attachments,
        in_reply_to: mail.message_id.clone(),
        references: mail.references.clone(),
        date: 1_792_400_000,
    }
}

#[test]
fn a_reply_built_from_the_sample_reads_back_the_same() {
    let mail = read(&sample()).unwrap();
    let draft = reply_to(&mail);
    let built = build(&draft).unwrap();
    let back = read(&built.bytes).unwrap();
    assert_eq!(back.subject.as_deref(), Some("Re: Café report"));
    assert_eq!(back.from, std::slice::from_ref(&draft.from));
    assert_eq!(back.to, draft.to);
    assert_eq!(back.cc, draft.cc);
    assert_eq!(back.date, Some(draft.date));
    assert_eq!(back.text, Some(draft.text.clone()));
    assert_eq!(back.html, draft.html);
    assert_eq!(back.message_id.as_deref(), Some(built.message_id.as_str()));
    assert!(
        built.message_id.ends_with("@example.com"),
        "{}",
        built.message_id
    );
    assert_eq!(back.in_reply_to, ["m1@example.com"]);
    assert_eq!(back.references, ["r1@example.com", "m0@example.com"]);
    let sections: Vec<_> = (back.attachments.iter())
        .map(|part| part.section.as_deref())
        .collect();
    assert_eq!(sections, [Some("2"), Some("3")]);
    let unnumbered = |parts: &[Part]| -> Vec<Part> {
        let bare = |part: &Part| Part {
            section: None,
            ..part.clone()
        };
        parts.iter().map(bare).collect()
    };
    assert_eq!(
        unnumbered(&back.attachments),
        unnumbered(&draft.attachments)
    );
    let bcc = |line: &str| line.to_ascii_lowercase().starts_with("bcc:");
    assert!(!back.headers.lines().any(bcc), "{}", back.headers);
    let again = build(&draft).unwrap();
    assert_ne!(again.message_id, built.message_id);
}

#[test]
fn bodies_are_only_what_the_sender_wrote() {
    let html = read(b"Content-Type: text/html\r\n\r\n<p>only html</p>").unwrap();
    assert_eq!(html.text, None);
    assert_eq!(html.html.as_deref(), Some("<p>only html</p>"));

    let mut draft = reply_to(&read(&sample()).unwrap());
    draft.html = None;
    let plain = read(&build(&draft).unwrap().bytes).unwrap();
    assert_eq!(plain.html, None);
    assert_eq!(plain.text.as_deref(), Some("Café numbers attached."));
}

#[test]
fn a_line_break_in_a_header_never_leaves() {
    let base = reply_to(&read(&sample()).unwrap());
    let mut subject = base.clone();
    subject.subject = "Hi\r\nBcc: hidden@example.com".into();
    let mut name = base.clone();
    name.to[0].name = Some("Bo\nBcc: hidden@example.com".into());
    let mut file = base.clone();
    file.attachments[0].name = Some("a.pdf\r\nX-Evil: 1".into());
    let mut email = base.clone();
    email.cc[0].email = "dee@example.net>\r\nBcc: <hidden@example.com".into();
    let mut cid = base.clone();
    cid.attachments[0].content_id = Some("a@b>\r\nX-Evil: 1".into());
    for draft in [subject, name, file, email, cid] {
        let err = build(&draft).err().unwrap();
        assert!(matches!(err, Error::Invalid(_)), "{err}");
    }
}

#[test]
fn bytes_that_are_no_message_are_an_error() {
    let err = read(b"").unwrap_err();
    assert!(matches!(err, Error::Mime(_)), "{err}");
}
