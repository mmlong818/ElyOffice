use ely_mail_engine::{Draft, Error, Mailbox, Part, build};
use fake::smtp::{logged_in, transaction};

#[tokio::test]
async fn a_utf8_header_needs_smtputf8() {
    let message = "Subject: 您好\r\n\r\nhi\r\n";
    let (mut smtp, script) = logged_in(
        "250-mx\n250-SMTPUTF8\n250 AUTH PLAIN\n",
        transaction(
            "MAIL FROM:<ann@example.com> SMTPUTF8",
            "RCPT TO:<bo@example.com>",
            "Subject: 您好\n\nhi",
        ),
    )
    .await;
    let to = ["bo@example.com".to_owned()];
    smtp.send("ann@example.com", &to, message.as_bytes())
        .await
        .unwrap();
    script.await.unwrap().unwrap();

    let (mut smtp, script) = logged_in("250-mx\n250 AUTH PLAIN\n", vec![]).await;
    let err = smtp
        .send("ann@example.com", &to, message.as_bytes())
        .await
        .unwrap_err();
    assert!(
        matches!(&err, Error::Invalid(text) if text.contains("headers")),
        "{err}"
    );
    script.await.unwrap().unwrap();
}

#[tokio::test]
async fn an_8bit_body_needs_8bitmime() {
    let message = "Subject: hi\r\n\r\n您好\r\n".as_bytes();
    let to = ["bo@example.com".to_owned()];
    let (mut smtp, script) = logged_in(
        "250-mx\n250-8BITMIME\n250 AUTH PLAIN\n",
        transaction(
            "MAIL FROM:<ann@example.com> BODY=8BITMIME",
            "RCPT TO:<bo@example.com>",
            "Subject: hi\n\n您好",
        ),
    )
    .await;
    smtp.send("ann@example.com", &to, message).await.unwrap();
    script.await.unwrap().unwrap();

    let (mut smtp, script) = logged_in("250-mx\n250 AUTH PLAIN\n", vec![]).await;
    let err = smtp
        .send("ann@example.com", &to, message)
        .await
        .unwrap_err();
    assert!(
        matches!(&err, Error::Invalid(text) if text.contains("8-bit")),
        "{err}"
    );
    script.await.unwrap().unwrap();
}

#[tokio::test]
async fn a_utf8_address_needs_smtputf8() {
    let to = "用户@例子.广告";
    let (mut smtp, script) = logged_in(
        "250-mx\n250-SMTPUTF8\n250 AUTH PLAIN\n",
        transaction(
            "MAIL FROM:<ann@example.com> SMTPUTF8",
            &format!("RCPT TO:<{to}>"),
            "Subject: x\n\nx",
        ),
    )
    .await;
    smtp.send(
        "ann@example.com",
        &[to.to_owned()],
        b"Subject: x\r\n\r\nx\r\n",
    )
    .await
    .unwrap();
    script.await.unwrap().unwrap();

    let (mut smtp, script) = logged_in("250-mx\n250 AUTH PLAIN\n", vec![]).await;
    let err = smtp
        .send(
            "ann@example.com",
            &[to.to_owned()],
            b"Subject: x\r\n\r\nx\r\n",
        )
        .await
        .unwrap_err();
    assert!(
        matches!(&err, Error::Invalid(text) if text.contains("non-ASCII")),
        "{err}"
    );
    script.await.unwrap().unwrap();
}

/// A forwarded message/global under ASCII outer headers.
fn forwarded() -> Vec<u8> {
    let enclosed = Part {
        name: Some("old.eml".into()),
        content_type: "message/global".into(),
        params: Vec::new(),
        content_id: None,
        bytes: "Subject: 您好\r\n\r\nhi\r\n".as_bytes().to_vec(),
        section: None,
    };
    let draft = Draft {
        from: Mailbox {
            name: None,
            email: "ann@example.com".into(),
        },
        to: Vec::new(),
        cc: Vec::new(),
        bcc: Vec::new(),
        subject: "fwd".into(),
        text: "see attached".into(),
        html: None,
        attachments: vec![enclosed],
        in_reply_to: None,
        references: Vec::new(),
        date: 0,
    };
    build(&draft).unwrap().bytes
}

#[tokio::test]
async fn an_enclosed_utf8_header_needs_smtputf8() {
    let to = ["bo@example.com".to_owned()];
    let built = forwarded();
    let (mut smtp, script) = logged_in("250-mx\n250-8BITMIME\n250 AUTH PLAIN\n", vec![]).await;
    let err = smtp.send("ann@example.com", &to, &built).await.unwrap_err();
    assert!(
        matches!(&err, Error::Invalid(text) if text.contains("headers")),
        "{err}"
    );
    script.await.unwrap().unwrap();

    let message = "Content-Type: multipart/mixed; boundary=b\r\n\r\n--b\r\n\
        Content-Type: message/global\r\n\r\nSubject: 您好\r\n\r\nhi\r\n--b--\r\n";
    let (mut smtp, script) = logged_in(
        "250-mx\n250-8BITMIME\n250-SMTPUTF8\n250 AUTH PLAIN\n",
        transaction(
            "MAIL FROM:<ann@example.com> BODY=8BITMIME SMTPUTF8",
            "RCPT TO:<bo@example.com>",
            message.replace("\r\n", "\n").trim_end(),
        ),
    )
    .await;
    smtp.send("ann@example.com", &to, message.as_bytes())
        .await
        .unwrap();
    script.await.unwrap().unwrap();
}
