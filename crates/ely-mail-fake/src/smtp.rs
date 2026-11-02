use base64::{Engine, engine::general_purpose::STANDARD};
use ely_mail_engine::{Secret, Security, Smtp};

use super::{Step, certs, expect, login, send, serve};

pub async fn logged_in(
    ehlo: &str,
    steps: Vec<Step>,
) -> (Smtp, tokio::task::JoinHandle<Result<(), String>>) {
    let certs = certs();
    let mut script = vec![
        send("220 mx ready\n"),
        expect("EHLO [127.0.0.1]"),
        send(ehlo),
        expect(&format!(
            "AUTH PLAIN {}",
            STANDARD.encode("\0ann@example.com\0pw")
        )),
        send("235 ok\n"),
    ];
    script.extend(steps);
    let (server, script) = serve(&certs, Security::Tls, script).await;
    let smtp = Smtp::connect(
        &server,
        &login(Secret::Password("pw".into())),
        &certs.trusted,
    )
    .await
    .unwrap();
    (smtp, script)
}

/// MAIL, RCPT and DATA with the server agreeing.
pub fn transaction(mail: &str, rcpt: &str, data: &str) -> Vec<Step> {
    vec![
        expect(mail),
        send("250 ok\n"),
        expect(rcpt),
        send("250 ok\n"),
        expect("DATA"),
        send("354 go\n"),
        Step::Data(data.to_owned()),
        send("250 queued\n"),
    ]
}
