use base64::{Engine, engine::general_purpose::STANDARD};
use ely_mail_engine::{Error, Pop, Secret, Security};
use fake::{Step, certs, expect, login, send, serve};

const TOKEN_LINE: &str = "user=ann@example.com\x01auth=Bearer tok-1\x01\x01";

#[tokio::test]
async fn a_password_login_lists_uids_and_fetches() {
    let certs = certs();
    let (server, script) = serve(
        &certs,
        Security::Tls,
        vec![
            send("+OK ready\n"),
            expect("USER ann@example.com"),
            send("+OK\n"),
            expect("PASS pw 1"),
            send("+OK in\n"),
            expect("UIDL"),
            send("+OK\n1 aa1\n2 bb2\n.\n"),
            expect("RETR 2"),
            send("+OK 27 octets\nSubject: Hi\n\nHello\n..dot\n...\n.\n"),
            expect("QUIT"),
            send("+OK bye\n"),
        ],
    )
    .await;
    let mut pop = Pop::connect(
        &server,
        &login(Secret::Password("pw 1".into())),
        &certs.trusted,
    )
    .await
    .unwrap();
    assert_eq!(
        pop.uids().await.unwrap(),
        [(1, "aa1".to_owned()), (2, "bb2".to_owned())]
    );
    let message = String::from_utf8(pop.fetch(2).await.unwrap()).unwrap();
    assert_eq!(message, "Subject: Hi\r\n\r\nHello\r\n.dot\r\n..\r\n");
    pop.quit().await.unwrap();
    script.await.unwrap().unwrap();
}

#[tokio::test]
async fn stls_then_xoauth2_sends_the_token_after_the_plus() {
    let certs = certs();
    let (server, script) = serve(
        &certs,
        Security::StartTls,
        vec![
            send("+OK ready\n"),
            expect("STLS"),
            send("+OK begin TLS\n"),
            Step::StartTls,
            expect("AUTH XOAUTH2"),
            send("+ \n"),
            expect(&STANDARD.encode(TOKEN_LINE)),
            send("+OK in\n"),
            expect("QUIT"),
            send("+OK bye\n"),
        ],
    )
    .await;
    let pop = Pop::connect(
        &server,
        &login(Secret::Token("tok-1".into())),
        &certs.trusted,
    )
    .await
    .unwrap();
    pop.quit().await.unwrap();
    script.await.unwrap().unwrap();
}

#[tokio::test]
async fn a_refused_token_answers_the_error_challenge_empty() {
    let certs = certs();
    let (server, script) = serve(
        &certs,
        Security::Tls,
        vec![
            send("+OK ready\n"),
            expect("AUTH XOAUTH2"),
            send("+ \n"),
            expect(&STANDARD.encode(TOKEN_LINE)),
            send(&format!("+ {}\n", STANDARD.encode(r#"{"status":"401"}"#))),
            expect(""),
            send("-ERR Authentication failure: unknown user name or bad password.\n"),
        ],
    )
    .await;
    let err = Pop::connect(
        &server,
        &login(Secret::Token("tok-1".into())),
        &certs.trusted,
    )
    .await
    .err()
    .unwrap();
    assert!(
        matches!(&err, Error::Rejected(text) if text.contains("bad password")),
        "{err}"
    );
    script.await.unwrap().unwrap();
}

#[tokio::test]
async fn a_refused_password_keeps_the_server_words() {
    let certs = certs();
    let (server, script) = serve(
        &certs,
        Security::Tls,
        vec![
            send("+OK ready\n"),
            expect("USER ann@example.com"),
            send("+OK\n"),
            expect("PASS bad"),
            send("-ERR [AUTH] Username and password not accepted.\n"),
        ],
    )
    .await;
    let err = Pop::connect(
        &server,
        &login(Secret::Password("bad".into())),
        &certs.trusted,
    )
    .await
    .err()
    .unwrap();
    assert!(
        matches!(&err, Error::Rejected(text) if text.contains("not accepted")),
        "{err}"
    );
    script.await.unwrap().unwrap();
}

#[tokio::test]
async fn a_greeting_err_keeps_the_server_words() {
    let certs = certs();
    let (server, script) = serve(&certs, Security::Tls, vec![send("-ERR maildrop busy\n")]).await;
    let err = Pop::connect(
        &server,
        &login(Secret::Password("pw".into())),
        &certs.trusted,
    )
    .await
    .err()
    .unwrap();
    assert!(
        matches!(&err, Error::Server(text) if text == "maildrop busy"),
        "{err}"
    );
    script.await.unwrap().unwrap();
}

#[tokio::test]
async fn a_connection_lost_before_the_pass_reply_is_an_error() {
    let certs = certs();
    let (server, script) = serve(
        &certs,
        Security::Tls,
        vec![
            send("+OK ready\n"),
            expect("USER ann@example.com"),
            send("+OK\n"),
            expect("PASS pw"),
        ],
    )
    .await;
    let err = Pop::connect(
        &server,
        &login(Secret::Password("pw".into())),
        &certs.trusted,
    )
    .await
    .err()
    .unwrap();
    assert!(matches!(&err, Error::Connect(_)), "{err}");
    script.await.unwrap().unwrap();
}

#[tokio::test]
async fn plain_bytes_after_stls_are_refused() {
    let certs = certs();
    let (server, _script) = serve(
        &certs,
        Security::StartTls,
        vec![
            send("+OK ready\n"),
            expect("STLS"),
            send("+OK begin TLS\nUIDL\n"),
        ],
    )
    .await;
    let err = Pop::connect(
        &server,
        &login(Secret::Password("pw".into())),
        &certs.trusted,
    )
    .await
    .err()
    .unwrap();
    assert!(
        matches!(&err, Error::Protocol(text) if text.contains("after STLS")),
        "{err}"
    );
}

#[tokio::test]
async fn a_line_break_in_the_login_never_leaves() {
    let certs = certs();
    let (server, script) = serve(&certs, Security::Tls, vec![send("+OK ready\n")]).await;
    let err = Pop::connect(
        &server,
        &login(Secret::Password("pw\r\nDELE 1".into())),
        &certs.trusted,
    )
    .await
    .err()
    .unwrap();
    assert!(matches!(&err, Error::Invalid(_)), "{err}");
    script.await.unwrap().unwrap();
}
