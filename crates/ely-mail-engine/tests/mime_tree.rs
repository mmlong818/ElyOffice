use ely_mail_engine::{Draft, Mailbox, Part, Shown, build, read};

/// Lines joined with CRLF, as servers send them.
fn crlf(lines: &[&str]) -> Vec<u8> {
    lines.join("\r\n").into_bytes()
}

fn part(name: Option<&str>, kind: &str, id: Option<&str>, bytes: &[u8]) -> Part {
    Part {
        name: name.map(str::to_owned),
        content_type: kind.to_owned(),
        params: Vec::new(),
        content_id: id.map(str::to_owned),
        bytes: bytes.to_vec(),
        section: None,
    }
}

/// The part as read at IMAP part `section`.
fn at(section: &str, part: Part) -> Part {
    Part {
        section: Some(section.into()),
        ..part
    }
}

#[test]
fn text_attachments_keep_their_own_bytes() {
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
        "Content-Disposition: attachment; filename=qp.txt",
        "",
        "Caf=E9",
        "--b",
        "Content-Type: text/plain; charset=iso-8859-1",
        "Content-Disposition: attachment; filename=raw.txt",
        "",
        "Caf\u{e9}",
        "--b",
        "Content-Type: text/plain; charset=iso-8859-1",
        "Content-Transfer-Encoding: base64",
        "Content-Disposition: attachment; filename=b64.txt",
        "",
        "Q2Fm6Q==",
        "--b--",
        "",
    ]))
    .unwrap();
    assert_eq!(mail.text.as_deref(), Some("body"));
    let bytes: Vec<&[u8]> = mail
        .attachments
        .iter()
        .map(|p| p.bytes.as_slice())
        .collect();
    assert_eq!(bytes, [b"Caf\xe9", "Caf\u{e9}".as_bytes(), b"Caf\xe9"]);
}

#[test]
fn a_built_latin1_text_attachment_reads_back_as_built() {
    let latin1 = part(Some("cafe.txt"), "text/plain", None, b"Caf\xe9\r\n");
    let draft = Draft {
        from: Mailbox {
            name: None,
            email: "a@example.com".into(),
        },
        to: Vec::new(),
        cc: Vec::new(),
        bcc: Vec::new(),
        subject: "s".into(),
        text: "t".into(),
        html: None,
        attachments: vec![latin1.clone()],
        in_reply_to: None,
        references: Vec::new(),
        date: 0,
    };
    let back = read(&build(&draft).unwrap().bytes).unwrap();
    assert_eq!(back.attachments, [at("2", latin1)]);
}

#[test]
fn a_related_root_named_by_start_can_come_after_its_image() {
    let mail = read(&crlf(&[
        "Content-Type: multipart/related; boundary=r; type=\"text/html\"; start=\"<root@example.com>\"",
        "",
        "--r",
        "Content-Type: image/png",
        "Content-ID: <img@example.com>",
        "Content-Transfer-Encoding: base64",
        "",
        "iVBORw==",
        "--r",
        "Content-Type: text/html",
        "Content-ID: <root@example.com>",
        "",
        "<img src=\"cid:img@example.com\">",
        "--r--",
        "",
    ]))
    .unwrap();
    assert_eq!(
        mail.html.as_deref(),
        Some("<img src=\"cid:img@example.com\">")
    );
    assert_eq!(
        mail.inline,
        [at(
            "1",
            part(None, "image/png", Some("img@example.com"), b"\x89PNG")
        )]
    );
    assert!(mail.attachments.is_empty());
}

#[test]
fn a_related_image_marked_attachment_is_still_a_cid_target() {
    let mail = read(&crlf(&[
        "Content-Type: multipart/related; boundary=r",
        "",
        "--r",
        "Content-Type: text/html",
        "",
        "<img src=\"cid:img@example.com\">",
        "--r",
        "Content-Type: image/png",
        "Content-ID: <img@example.com>",
        "Content-Disposition: attachment; filename=img.png",
        "Content-Transfer-Encoding: base64",
        "",
        "iVBORw==",
        "--r--",
        "",
    ]))
    .unwrap();
    assert_eq!(
        mail.inline,
        [at(
            "2",
            part(
                Some("img.png"),
                "image/png",
                Some("img@example.com"),
                b"\x89PNG"
            )
        )]
    );
    assert!(mail.attachments.is_empty());
}

#[test]
fn an_attached_multipart_is_one_attachment_and_no_body() {
    let entity = [
        "Content-Type: multipart/alternative; boundary=in",
        "Content-Disposition: attachment; filename=alternatives.mime",
        "",
        "--in",
        "Content-Type: text/plain",
        "",
        "private attachment",
        "--in--",
    ];
    let mut lines = vec![
        "Content-Type: multipart/mixed; boundary=out",
        "",
        "--out",
        "Content-Type: text/plain",
        "",
        "body",
        "--out",
    ];
    lines.extend(entity);
    lines.extend(["--out--", ""]);
    let mail = read(&crlf(&lines)).unwrap();
    assert_eq!(mail.text.as_deref(), Some("body"));
    assert_eq!(mail.attachments.len(), 1);
    let attached = &mail.attachments[0];
    assert_eq!(attached.name.as_deref(), Some("alternatives.mime"));
    assert_eq!(attached.content_type, "multipart/alternative");
    assert_eq!(attached.bytes, crlf(&entity));
}

#[test]
fn alternatives_of_one_type_are_chosen_not_joined() {
    let mail = read(&crlf(&[
        "Content-Type: multipart/alternative; boundary=a",
        "",
        "--a",
        "Content-Type: text/plain; charset=us-ascii",
        "",
        "Cafe",
        "--a",
        "Content-Type: text/plain; charset=utf-8",
        "",
        "Café",
        "--a",
        "Content-Type: text/calendar",
        "",
        "BEGIN:VCALENDAR",
        "--a--",
        "",
    ]))
    .unwrap();
    assert_eq!(mail.text.as_deref(), Some("Café"));
    assert_eq!(
        mail.attachments,
        [at(
            "3",
            part(None, "text/calendar", None, b"BEGIN:VCALENDAR")
        )]
    );
}

#[test]
fn a_digest_child_without_a_type_is_a_message() {
    let enclosed = "From: x@example.com\r\nSubject: inner\r\n\r\nhello";
    let mut lines = vec!["Content-Type: multipart/digest; boundary=d", "", "--d", ""];
    lines.push(enclosed);
    lines.extend(["--d--", ""]);
    let mail = read(&crlf(&lines)).unwrap();
    assert_eq!(mail.attachments.len(), 1);
    assert_eq!(mail.attachments[0].content_type, "message/rfc822");
    let inner = read(&mail.attachments[0].bytes).unwrap();
    assert_eq!(inner.subject.as_deref(), Some("inner"));
}

#[test]
fn parts_carry_the_numbers_imap_fetches_them_by() {
    let lone = read(&crlf(&[
        "Content-Type: application/pdf",
        "Content-Disposition: attachment; filename=a.pdf",
        "",
        "%PDF",
        "",
    ]))
    .unwrap();
    assert_eq!(lone.attachments[0].section.as_deref(), Some("1"));
    let whole = read(&crlf(&[
        "Content-Type: multipart/mixed; boundary=w",
        "Content-Disposition: attachment; filename=all.mime",
        "",
        "--w",
        "Content-Type: text/plain",
        "",
        "inside",
        "--w--",
        "",
    ]))
    .unwrap();
    assert_eq!(whole.attachments[0].section.as_deref(), Some("TEXT"));
    let nested = read(&crlf(&[
        "Content-Type: multipart/mixed; boundary=o",
        "",
        "--o",
        "Content-Type: text/plain",
        "",
        "body",
        "--o",
        "Content-Type: multipart/mixed; boundary=i",
        "",
        "--i",
        "Content-Type: text/plain",
        "",
        "more",
        "--i",
        "Content-Type: application/pdf",
        "Content-Disposition: attachment; filename=deep.pdf",
        "",
        "%PDF",
        "--i--",
        "--o",
        "Content-Type: message/rfc822",
        "",
        "Subject: old",
        "",
        "old body",
        "--o--",
        "",
    ]))
    .unwrap();
    let sections: Vec<_> = (nested.attachments.iter())
        .map(|part| part.section.as_deref())
        .collect();
    assert_eq!(sections, [Some("2.2"), Some("3")]);
}

#[test]
fn mixed_bodies_read_in_order_and_alternatives_once() {
    let mail = read(&crlf(&[
        "Content-Type: multipart/mixed; boundary=m",
        "",
        "--m",
        "Content-Type: text/plain",
        "",
        "Appointment is at 09:00",
        "--m",
        "Content-Type: text/html",
        "",
        "<p>Directions to the clinic</p>",
        "--m",
        "Content-Type: multipart/alternative; boundary=a",
        "",
        "--a",
        "Content-Type: text/plain",
        "",
        "Bring your card",
        "--a",
        "Content-Type: text/html",
        "",
        "<p>Bring your card</p>",
        "--a--",
        "--m",
        "Content-Type: multipart/alternative; boundary=t",
        "",
        "--t",
        "Content-Type: text/plain",
        "",
        "Plain only",
        "--t--",
        "--m--",
        "",
    ]))
    .unwrap();
    assert_eq!(
        mail.shown,
        [
            Shown::Text("Appointment is at 09:00".into()),
            Shown::Html("<p>Directions to the clinic</p>".into()),
            Shown::Html("<p>Bring your card</p>".into()),
            Shown::Text("Plain only".into()),
        ]
    );
}
