use std::io;

use imap_proto::{Response, Status};
use nom::{Err as NomErr, Needed};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::{Error, Result};

const CHUNK: usize = 16 * 1024;

/// IMAP responses off a byte stream; keeps the last BYE.
pub(crate) struct Wire<S> {
    io: S,
    buf: Vec<u8>,
    /// Bytes the next parse needs before it can finish.
    needs: usize,
    bye: Option<String>,
}

impl<S: AsyncRead + AsyncWrite + Unpin> Wire<S> {
    pub fn new(io: S) -> Self {
        Self {
            io,
            buf: Vec::new(),
            needs: 0,
            bye: None,
        }
    }

    /// Hands the next response and its raw bytes to `read`.
    pub async fn next<R>(&mut self, mut read: impl FnMut(&Response, &[u8]) -> R) -> Result<R> {
        loop {
            if self.buf.len() >= self.needs && !self.buf.is_empty() {
                let parsed = match Response::parse(&self.buf) {
                    Ok((rest, response)) => {
                        let used = self.buf.len() - rest.len();
                        let raw = &self.buf[..used];
                        if let Response::Data {
                            status: Status::Bye,
                            ..
                        } = response
                        {
                            self.bye = Some(words(raw));
                        }
                        Some((used, read(&response, raw)))
                    }
                    Err(NomErr::Incomplete(Needed::Size(more))) => {
                        self.needs = self.buf.len() + more.get();
                        None
                    }
                    Err(NomErr::Incomplete(Needed::Unknown)) => {
                        self.needs = self.buf.len() + 1;
                        None
                    }
                    Err(_) => {
                        return Err(Error::Protocol(
                            "a reply from the server could not be read".into(),
                        ));
                    }
                };
                if let Some((used, value)) = parsed {
                    self.buf.drain(..used);
                    self.needs = 0;
                    return Ok(value);
                }
            }
            self.buf.reserve(CHUNK);
            match self.io.read_buf(&mut self.buf).await {
                Ok(0) => return Err(self.closed(io::ErrorKind::UnexpectedEof.into())),
                Ok(_) => {}
                Err(err) => return Err(self.closed(err)),
            }
        }
    }

    pub async fn write(&mut self, bytes: &[u8]) -> Result<()> {
        let sent = async {
            self.io.write_all(bytes).await?;
            self.io.flush().await
        };
        sent.await.map_err(|err| self.closed(err))
    }

    /// The stream for TLS; bytes already read would be plaintext.
    pub fn into_inner(self) -> Result<S> {
        if !self.buf.is_empty() {
            return Err(Error::Protocol(
                "the server sent data before TLS began".into(),
            ));
        }
        Ok(self.io)
    }

    /// After BYE, a dropped line is the server's words.
    fn closed(&self, err: io::Error) -> Error {
        match &self.bye {
            Some(words) => Error::Server(words.clone()),
            None => Error::Connect(err),
        }
    }
}

/// The line after its tag, as the server wrote it.
pub(crate) fn words(raw: &[u8]) -> String {
    let line = raw.strip_suffix(b"\r\n").unwrap_or(raw);
    let text = line.splitn(2, |&byte| byte == b' ').nth(1).unwrap_or(line);
    String::from_utf8_lossy(text).into_owned()
}
