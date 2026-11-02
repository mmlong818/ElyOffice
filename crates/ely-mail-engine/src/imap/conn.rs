//! Tagged commands, literals and replies on one stream.

use base64::{Engine, engine::general_purpose::STANDARD};
use imap_proto::{Response, Status};
use tokio::io::{AsyncRead, AsyncWrite};

use crate::{
    Error, Result,
    wire::{Wire, words},
};

/// One command argument.
pub(super) enum Arg<'a> {
    /// Written as is.
    Atom(&'a str),
    /// Quoted, or a literal when quotes cannot carry it.
    Text(&'a str),
    /// Always a literal: a whole message.
    Bytes(&'a [u8]),
}

/// OK, or the words of NO or BAD.
pub(super) type Outcome = std::result::Result<(), String>;

pub(super) struct Conn<S> {
    pub(super) wire: Wire<S>,
    tags: u32,
}

impl<S: AsyncRead + AsyncWrite + Unpin> Conn<S> {
    pub(super) fn new(io: S) -> Self {
        Self {
            wire: Wire::new(io),
            tags: 0,
        }
    }

    /// `* BYE` ends with the server's words.
    pub(super) async fn greeting(&mut self) -> Result<()> {
        self.wire
            .next(|response, raw| match response {
                Response::Data {
                    status: Status::Ok, ..
                } => Ok(()),
                Response::Data {
                    status: Status::Bye,
                    ..
                } => Err(Error::Server(words(raw))),
                Response::Data { status, .. } => Err(Error::Protocol(format!(
                    "the greeting was {status:?}, not OK"
                ))),
                _ => Err(Error::Protocol("the first line was not a greeting".into())),
            })
            .await?
    }

    fn tag(&mut self) -> String {
        self.tags += 1;
        format!("A{}", self.tags)
    }

    /// Sends a command; replies go to `each`, tagged last.
    pub(super) async fn run(
        &mut self,
        verb: &str,
        args: &[Arg<'_>],
        mut each: impl FnMut(&Response, &[u8]),
    ) -> Result<Outcome> {
        let tag = self.tag();
        log::debug!("mail: IMAP {tag} {verb}");
        let mut line = format!("{tag} {verb}").into_bytes();
        for arg in args {
            line.push(b' ');
            match arg {
                Arg::Atom(atom) => line.extend_from_slice(atom.as_bytes()),
                Arg::Text(text) => match quoted(text)? {
                    Some(quoted) => line.extend_from_slice(quoted.as_bytes()),
                    None => {
                        let bytes = text.as_bytes();
                        if let Err(words) = self.literal(&tag, &mut line, bytes, &mut each).await? {
                            return Ok(Err(words));
                        }
                    }
                },
                Arg::Bytes(bytes) => {
                    if let Err(words) = self.literal(&tag, &mut line, bytes, &mut each).await? {
                        return Ok(Err(words));
                    }
                }
            }
        }
        line.extend_from_slice(b"\r\n");
        self.wire.write(&line).await?;
        loop {
            match self.step(&tag, &mut each).await? {
                Step::Done(outcome) => return Ok(outcome),
                Step::Untagged => {}
                Step::Continue => {
                    return Err(Error::Protocol(
                        "the server asked for more than was sent".into(),
                    ));
                }
            }
        }
    }

    /// Sends `line` with a literal's length, then its bytes.
    async fn literal(
        &mut self,
        tag: &str,
        line: &mut Vec<u8>,
        bytes: &[u8],
        each: &mut impl FnMut(&Response, &[u8]),
    ) -> Result<Outcome> {
        line.extend_from_slice(format!("{{{}}}\r\n", bytes.len()).as_bytes());
        self.wire.write(line).await?;
        line.clear();
        let outcome = self.proceed(tag, each).await?;
        if outcome.is_ok() {
            line.extend_from_slice(bytes);
        }
        Ok(outcome)
    }

    /// Waits for `+` before a literal.
    async fn proceed(
        &mut self,
        tag: &str,
        each: &mut impl FnMut(&Response, &[u8]),
    ) -> Result<Outcome> {
        loop {
            match self.step(tag, each).await? {
                Step::Continue => return Ok(Ok(())),
                Step::Done(Ok(())) => {
                    return Err(Error::Protocol(
                        "the command ended before its literal".into(),
                    ));
                }
                Step::Done(refused) => return Ok(refused),
                Step::Untagged => {}
            }
        }
    }

    /// Answers the first challenge; empty after, so a refusal ends.
    pub(super) async fn xoauth2(&mut self, initial: &str) -> Result<Outcome> {
        let tag = self.tag();
        self.wire
            .write(format!("{tag} AUTHENTICATE XOAUTH2\r\n").as_bytes())
            .await?;
        let mut answer = Some(STANDARD.encode(initial));
        loop {
            match self.step(&tag, &mut |_, _| {}).await? {
                Step::Continue => {
                    let line = answer.take().unwrap_or_default() + "\r\n";
                    self.wire.write(line.as_bytes()).await?;
                }
                Step::Done(outcome) => return Ok(outcome),
                Step::Untagged => {}
            }
        }
    }

    async fn step(&mut self, tag: &str, each: &mut impl FnMut(&Response, &[u8])) -> Result<Step> {
        self.wire
            .next(|response, raw| match response {
                Response::Continue(_) => Ok(Step::Continue),
                Response::Done {
                    tag: done, status, ..
                } => {
                    if done.0 != tag {
                        return Err(Error::Protocol(format!(
                            "a reply for {} came while waiting for {tag}",
                            done.0
                        )));
                    }
                    each(response, raw);
                    match status {
                        Status::Ok => Ok(Step::Done(Ok(()))),
                        Status::No | Status::Bad => Ok(Step::Done(Err(words(raw)))),
                        other => Err(Error::Protocol(format!("{tag} ended with {other:?}"))),
                    }
                }
                untagged => {
                    each(untagged, raw);
                    Ok(Step::Untagged)
                }
            })
            .await?
    }
}

enum Step {
    Continue,
    Untagged,
    Done(Outcome),
}

/// `None` when only a literal can carry the text.
fn quoted(text: &str) -> Result<Option<String>> {
    if text.contains('\0') {
        return Err(Error::Invalid("IMAP cannot carry a NUL byte".into()));
    }
    if !text
        .bytes()
        .all(|byte| byte.is_ascii() && byte != b'\r' && byte != b'\n')
    {
        return Ok(None);
    }
    let mut quoted = String::with_capacity(text.len() + 2);
    quoted.push('"');
    for char in text.chars() {
        if char == '"' || char == '\\' {
            quoted.push('\\');
        }
        quoted.push(char);
    }
    quoted.push('"');
    Ok(Some(quoted))
}
