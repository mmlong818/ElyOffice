use ely_mail_engine::{Error, read};

/// `levels` messages, each enclosing the next.
fn enclosed(levels: usize) -> Vec<u8> {
    let mut mail = b"Subject: core\r\n\r\nhi\r\n".to_vec();
    for _ in 0..levels {
        let mut outer = b"Content-Type: message/rfc822\r\n\r\n".to_vec();
        outer.extend_from_slice(&mail);
        mail = outer;
    }
    mail
}

#[test]
fn enclosed_messages_count_toward_the_depth() {
    let err = read(&enclosed(128)).unwrap_err();
    assert!(
        matches!(&err, Error::Mime(text) if text.contains("64")),
        "{err}"
    );
    let shallow = read(&enclosed(8)).unwrap();
    assert_eq!(shallow.attachments.len(), 1);
}

#[test]
fn a_very_deep_enclosure_is_refused_without_a_crash() {
    let err = read(&enclosed(8192)).unwrap_err();
    assert!(
        matches!(&err, Error::Mime(text) if text.contains("64")),
        "{err}"
    );
}

#[test]
fn an_attached_message_is_its_own_bytes() {
    let inner = "Subject: inner\r\nContent-Type: text/plain\r\n\r\nhello";
    let raw = format!(
        "Subject: outer\r\nContent-Type: multipart/mixed; boundary=b\r\n\r\n\
         --b\r\nContent-Type: text/plain\r\n\r\nbody\r\n\
         --b\r\nContent-Type: message/rfc822\r\n\r\n{inner}\r\n--b--\r\n"
    );
    let mail = read(raw.as_bytes()).unwrap();
    let [message] = mail.attachments.as_slice() else {
        panic!("{:?}", mail.attachments)
    };
    assert_eq!(message.bytes, inner.as_bytes());
}

#[test]
fn a_multipart_with_a_content_id_is_a_cid_target() {
    let raw = "Content-Type: multipart/mixed; boundary=m\r\n\r\n\
        --m\r\nContent-Type: text/html\r\n\r\n<a href=\"cid:compound@example.com\">x</a>\r\n\
        --m\r\nContent-Type: multipart/alternative; boundary=a\r\n\
        Content-ID: <compound@example.com>\r\n\
        Content-Disposition: inline; filename=compound.mime\r\n\r\n\
        --a\r\nContent-Type: text/plain\r\n\r\ninner\r\n--a--\r\n--m--\r\n";
    let mail = read(raw.as_bytes()).unwrap();
    let [target] = mail.inline.as_slice() else {
        panic!("{:?}", mail.inline)
    };
    assert_eq!(target.content_id.as_deref(), Some("compound@example.com"));
    assert_eq!(target.content_type, "multipart/alternative");
    assert_eq!(target.name.as_deref(), Some("compound.mime"));
    assert!(
        target
            .bytes
            .starts_with(b"Content-Type: multipart/alternative")
    );
    assert!(target.bytes.ends_with(b"--a--"), "{:?}", target.bytes);
    assert!(mail.attachments.is_empty(), "{:?}", mail.attachments);
}
