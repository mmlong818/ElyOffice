use base64::{Engine, engine::general_purpose::STANDARD};
use ely_mail_engine::{Error, Secret, Security, Smtp};
use fake::{
    Step, certs, expect, login, send, serve,
    smtp::{logged_in, transaction},
};

#[tokio::test]
async fn starttls_password_login_sends_one_message() {
    let certs = certs();
    let plain = STANDARD.encode("\0ann@example.com\0pw 1");
    let (server, script) = serve(
        &certs,
        Security::StartTls,
        vec![
            send("220 mx ready\n"),
            expect("EHLO [127.0.0.1]"),
            send("250-mx\n250 STARTTLS\n"),
            expect("STARTTLS"),
            send("220 go\n"),
            Step::StartTls,
            expect("EHLO [127.0.0.1]"),
            send("250-mx\n250 AUTH PLAIN\n"),
            expect(&format!("AUTH PLAIN {plain}")),
            send("235 ok\n"),
            expect("MAIL FROM:<ann@example.com>"),
            send("250 ok\n"),
            expect("RCPT TO:<bo@example.com>"),
            send("250 ok\n"),
            expect("RCPT TO:<cy@example.com>"),
            send("250 ok\n"),
            expect("DATA"),
            send("354 go\n"),
            Step::Data("Subject: Hi\n\nHello\n..dot".into()),
            send("250 queued as 7\n"),
            expect("QUIT"),
            send("221 bye\n"),
        ],
    )
    .await;
    let mut smtp = Smtp::connect(
        &server,
        &login(Secret::Password("pw 1".into())),
        &certs.trusted,
    )
    .await
    .unwrap();
    let to = ["bo@example.com".to_owned(), "cy@example.com".to_owned()];
    smtp.send(
        "ann@example.com",
        &to,
        b"Subject: Hi\r\n\r\nHello\r\n.dot\r\n",
    )
    .await
    .unwrap();
    smtp.quit().await.unwrap();
    script.await.unwrap().unwrap();
}

#[tokio::test]
async fn implicit_tls_xoauth2() {
    let certs = certs();
    let token = STANDARD.encode("user=ann@example.com\x01auth=Bearer tok-1\x01\x01");
    let (server, script) = serve(
        &certs,
        Security::Tls,
        vec![
            send("220 mx ready\n"),
            expect("EHLO [127.0.0.1]"),
            send("250-mx\n250 AUTH XOAUTH2\n"),
            expect(&format!("AUTH XOAUTH2 {token}")),
            send("235 ok\n"),
        ],
    )
    .await;
    Smtp::connect(
        &server,
        &login(Secret::Token("tok-1".into())),
        &certs.trusted,
    )
    .await
    .unwrap();
    script.await.unwrap().unwrap();
}

#[tokio::test]
async fn a_refused_password_keeps_the_server_words() {
    let certs = certs();
    let (server, script) = serve(
        &certs,
        Security::Tls,
        vec![
            send("220 mx ready\n"),
            expect("EHLO [127.0.0.1]"),
            send("250-mx\n250 AUTH PLAIN\n"),
            expect(&format!(
                "AUTH PLAIN {}",
                STANDARD.encode("\0ann@example.com\0bad")
            )),
            send("535 5.7.8 Username and Password not accepted\n"),
        ],
    )
    .await;
    let err = Smtp::connect(
        &server,
        &login(Secret::Password("bad".into())),
        &certs.trusted,
    )
    .await
    .err()
    .unwrap();
    assert!(
        matches!(&err, Error::Rejected(text) if text.contains("535") && text.contains("not accepted")),
        "{err}"
    );
    script.await.unwrap().unwrap();
}

#[tokio::test]
async fn a_server_without_starttls_is_refused() {
    let certs = certs();
    let (server, script) = serve(
        &certs,
        Security::StartTls,
        vec![
            send("220 mx ready\n"),
            expect("EHLO [127.0.0.1]"),
            send("250-mx\n250 AUTH PLAIN\n"),
        ],
    )
    .await;
    let err = Smtp::connect(
        &server,
        &login(Secret::Password("pw".into())),
        &certs.trusted,
    )
    .await
    .err()
    .unwrap();
    assert!(
        matches!(&err, Error::Server(text) if text.contains("STARTTLS")),
        "{err}"
    );
    script.await.unwrap().unwrap();
}

#[tokio::test]
async fn a_refused_recipient_keeps_the_server_words() {
    let certs = certs();
    let (server, script) = serve(
        &certs,
        Security::Tls,
        vec![
            send("220 mx ready\n"),
            expect("EHLO [127.0.0.1]"),
            send("250-mx\n250 AUTH PLAIN\n"),
            expect(&format!(
                "AUTH PLAIN {}",
                STANDARD.encode("\0ann@example.com\0pw")
            )),
            send("235 ok\n"),
            expect("MAIL FROM:<ann@example.com>"),
            send("250 ok\n"),
            expect("RCPT TO:<nobody@example.com>"),
            send("550 5.1.1 No such user\n"),
            expect("QUIT"),
            send("221 bye\n"),
        ],
    )
    .await;
    let mut smtp = Smtp::connect(
        &server,
        &login(Secret::Password("pw".into())),
        &certs.trusted,
    )
    .await
    .unwrap();
    let err = smtp
        .send(
            "ann@example.com",
            &["nobody@example.com".to_owned()],
            b"Subject: x\r\n\r\nx\r\n",
        )
        .await
        .unwrap_err();
    assert!(
        matches!(&err, Error::Server(text) if text.contains("550") && text.contains("No such user")),
        "{err}"
    );
    smtp.quit().await.unwrap();
    script.await.unwrap().unwrap();
}

#[tokio::test]
async fn a_refused_token_answers_the_error_challenge_empty() {
    let certs = certs();
    let token = STANDARD.encode("user=ann@example.com\x01auth=Bearer tok-1\x01\x01");
    let (server, script) = serve(
        &certs,
        Security::Tls,
        vec![
            send("220 mx ready\n"),
            expect("EHLO [127.0.0.1]"),
            send("250-mx\n250 AUTH XOAUTH2\n"),
            expect(&format!("AUTH XOAUTH2 {token}")),
            send(&format!("334 {}\n", STANDARD.encode(r#"{"status":"400"}"#))),
            expect(""),
            send("535-5.7.8 Username and Password not accepted\n535 5.7.8 Learn more\n"),
        ],
    )
    .await;
    let err = Smtp::connect(
        &server,
        &login(Secret::Token("tok-1".into())),
        &certs.trusted,
    )
    .await
    .err()
    .unwrap();
    assert!(
        matches!(&err, Error::Rejected(text) if text.contains("535") && text.contains("not accepted")),
        "{err}"
    );
    script.await.unwrap().unwrap();
}

#[tokio::test]
async fn an_envelope_with_a_line_break_or_bracket_never_leaves() {
    let certs = certs();
    let (server, script) = serve(
        &certs,
        Security::Tls,
        vec![
            send("220 mx ready\n"),
            expect("EHLO [127.0.0.1]"),
            send("250-mx\n250 AUTH PLAIN\n"),
            expect(&format!(
                "AUTH PLAIN {}",
                STANDARD.encode("\0ann@example.com\0pw")
            )),
            send("235 ok\n"),
            expect("QUIT"),
            send("221 bye\n"),
        ],
    )
    .await;
    let mut smtp = Smtp::connect(
        &server,
        &login(Secret::Password("pw".into())),
        &certs.trusted,
    )
    .await
    .unwrap();
    let bad = [
        (
            "ann@example.com",
            "bo@example.com>\r\nRCPT TO:<hidden@example.com",
        ),
        (
            "ann@example.com>\r\nRCPT TO:<hidden@example.com",
            "bo@example.com",
        ),
        ("ann@example.com", "bo @example.com"),
        ("ann@example.com", ""),
        (
            "ann@example.com",
            "\"bo\r\nRCPT TO:<hidden@example.com>\"@example.com",
        ),
        ("ann@example.com", "\"bo@example.com"),
        ("ann@example.com", "bo@exa mple.com"),
    ];
    for (from, to) in bad {
        let err = smtp
            .send(from, &[to.to_owned()], b"Subject: x\r\n\r\nx\r\n")
            .await
            .unwrap_err();
        assert!(matches!(&err, Error::Invalid(_)), "{err}");
    }
    let err = smtp
        .send(
            "ann@example.com",
            &["bo@example.com".to_owned()],
            b"a\n.b\r\n",
        )
        .await
        .unwrap_err();
    assert!(
        matches!(&err, Error::Invalid(text) if text.contains("bare")),
        "{err}"
    );
    smtp.quit().await.unwrap();
    script.await.unwrap().unwrap();
}

#[tokio::test]
async fn a_quoted_local_part_is_a_mailbox() {
    let from = r#""ann \"a\" lee"@example.com"#;
    let to = r#""bo smith"@[192.0.2.1]"#;
    let (mut smtp, script) = logged_in(
        "250-mx\n250 AUTH PLAIN\n",
        transaction(
            &format!("MAIL FROM:<{from}>"),
            &format!("RCPT TO:<{to}>"),
            "Subject: x\n\nx",
        ),
    )
    .await;
    smtp.send(from, &[to.to_owned()], b"Subject: x\r\n\r\nx\r\n")
        .await
        .unwrap();
    script.await.unwrap().unwrap();
}

#[tokio::test]
async fn a_dot_opening_the_first_line_is_stuffed() {
    let (mut smtp, script) = logged_in(
        "250-mx\n250 AUTH PLAIN\n",
        transaction(
            "MAIL FROM:<ann@example.com>",
            "RCPT TO:<bo@example.com>",
            "..X-Trace: yes\nSubject: x\n\n..\nend",
        ),
    )
    .await;
    smtp.send(
        "ann@example.com",
        &["bo@example.com".to_owned()],
        b".X-Trace: yes\r\nSubject: x\r\n\r\n.\r\nend",
    )
    .await
    .unwrap();
    script.await.unwrap().unwrap();
}
