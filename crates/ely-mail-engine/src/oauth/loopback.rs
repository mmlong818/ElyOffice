use std::time::Duration;

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};

use crate::{Error, Result};

const LIMIT: usize = 8 * 1024;
const READ: Duration = Duration::from_secs(10);
const PAGE: &str = "ElyMail: sign-in finished. You can close this tab.";

/// Serves redirects until one carries a code or error.
pub(super) async fn code(listener: &TcpListener, state: &str) -> Result<String> {
    loop {
        let (mut stream, _) = listener
            .accept()
            .await
            .map_err(|err| Error::SignIn(format!("loopback accept failed: {err}")))?;
        let Some(target) = request(&mut stream).await else {
            continue;
        };
        let query = target.split_once('?').map_or("", |(_, query)| query);
        let pairs: Vec<(String, String)> = form_urlencoded::parse(query.as_bytes())
            .map(|(key, value)| (key.into_owned(), value.into_owned()))
            .collect();
        let field = |name: &str| {
            pairs
                .iter()
                .find(|(key, _)| key == name)
                .map(|(_, v)| v.clone())
        };
        if field("code").is_none() && field("error").is_none() {
            reply(&mut stream, "404 Not Found", "").await;
            continue;
        }
        reply(&mut stream, "200 OK", PAGE).await;
        if field("state").as_deref() != Some(state) {
            return Err(Error::SignIn("redirect state does not match".into()));
        }
        if let Some(error) = field("error") {
            return Err(Error::SignIn(match field("error_description") {
                Some(text) => format!("{error}: {text}"),
                None => error,
            }));
        }
        return field("code").ok_or_else(|| Error::SignIn("redirect without a code".into()));
    }
}

/// The request target of one GET, None for anything else.
async fn request(stream: &mut TcpStream) -> Option<String> {
    let mut head = Vec::new();
    let mut chunk = [0u8; 1024];
    while !head.windows(4).any(|window| window == b"\r\n\r\n") {
        let read = tokio::time::timeout(READ, stream.read(&mut chunk)).await;
        match read {
            Ok(Ok(0)) | Ok(Err(_)) | Err(_) => return None,
            Ok(Ok(count)) => head.extend_from_slice(&chunk[..count]),
        }
        if head.len() > LIMIT {
            log::warn!("oauth: loopback request over {LIMIT} bytes dropped");
            return None;
        }
    }
    let line = head.split(|byte| *byte == b'\n').next()?;
    let line = std::str::from_utf8(line).ok()?.trim_end();
    let mut parts = line.split(' ');
    match (parts.next(), parts.next()) {
        (Some("GET"), Some(target)) => Some(target.to_owned()),
        _ => None,
    }
}

async fn reply(stream: &mut TcpStream, status: &str, body: &str) {
    let text = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/plain; charset=utf-8\r\n\
         Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    if let Err(err) = stream.write_all(text.as_bytes()).await {
        log::warn!("oauth: loopback reply failed: {err}");
    }
}
