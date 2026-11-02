//! mbox files: one message per From line, mboxrd quoted.

use std::io::{self, BufRead, Write};

/// Messages one at a time, so large files stream.
pub struct Reader<R> {
    input: R,
    /// The `From ` line that opened the next message.
    started: bool,
    line: Vec<u8>,
    done: bool,
}

impl<R: BufRead> Reader<R> {
    pub fn new(input: R) -> Self {
        Self { input, started: false, line: Vec::new(), done: false }
    }

    /// The next message, CRLF ended, its `>From ` lines unquoted.
    pub fn next_message(&mut self) -> io::Result<Option<Vec<u8>>> {
        if self.done {
            return Ok(None);
        }
        let mut message: Vec<u8> = Vec::new();
        loop {
            self.line.clear();
            let read = self.input.read_until(b'\n', &mut self.line)?;
            let line = self.line.strip_suffix(b"\n").unwrap_or(&self.line);
            let line = line.strip_suffix(b"\r").unwrap_or(line);
            if read == 0 || line.starts_with(b"From ") {
                self.done = read == 0;
                match (self.started, read) {
                    // Text before the first `From ` is no message.
                    (false, 0) if message.iter().any(|byte| !byte.is_ascii_whitespace()) => {
                        return Err(io::Error::new(io::ErrorKind::InvalidData, "the file has no mbox From line"));
                    }
                    (false, 0) => return Ok(None),
                    (false, _) => {
                        if message.iter().any(|byte| !byte.is_ascii_whitespace()) {
                            return Err(io::Error::new(io::ErrorKind::InvalidData, "text before the first From line"));
                        }
                        self.started = true;
                        continue;
                    }
                    (true, _) => {
                        // The blank line before a From line is the file's.
                        if message.ends_with(b"\r\n\r\n") {
                            message.truncate(message.len() - 2);
                        }
                        return Ok(Some(message));
                    }
                }
            }
            message.extend_from_slice(unquoted(line));
            message.extend_from_slice(b"\r\n");
        }
    }
}

/// mboxrd: `>From `, `>>From ` lose one `>`.
fn unquoted(line: &[u8]) -> &[u8] {
    let quotes = line.iter().take_while(|byte| **byte == b'>').count();
    match quotes > 0 && line[quotes..].starts_with(b"From ") {
        true => &line[1..],
        false => line,
    }
}

/// Only all-CRLF mail, ending in one, survives mbox.
pub fn lines_fit(message: &[u8]) -> bool {
    message.ends_with(b"\r\n") && message.iter().enumerate().all(|(at, byte)| match byte {
        b'\r' => message.get(at + 1) == Some(&b'\n'),
        b'\n' => at > 0 && message[at - 1] == b'\r',
        _ => true,
    })
}

/// One message after its From line, mboxrd quoted, LF ended.
pub fn write(out: &mut impl Write, sender: &str, at: i64, message: &[u8]) -> io::Result<()> {
    let sender = match sender.is_empty() || sender.contains(char::is_whitespace) {
        true => "MAILER-DAEMON",
        false => sender,
    };
    let date = jiff::Timestamp::from_second(at)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.to_string()))?
        .strftime("%a %b %e %H:%M:%S %Y");
    writeln!(out, "From {sender} {date}")?;
    for line in message.split_inclusive(|byte| *byte == b'\n') {
        let line = line.strip_suffix(b"\n").unwrap_or(line);
        let line = line.strip_suffix(b"\r").unwrap_or(line);
        let quotes = line.iter().take_while(|byte| **byte == b'>').count();
        if line[quotes..].starts_with(b"From ") {
            out.write_all(b">")?;
        }
        out.write_all(line)?;
        out.write_all(b"\n")?;
    }
    // A blank line ends each message.
    out.write_all(b"\n")
}

#[cfg(test)]
mod tests;
