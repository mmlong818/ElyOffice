//! A worker's stderr, carried into the app's log.

use std::{
    io::{BufRead as _, BufReader, Read},
    sync::{Arc, Mutex},
    thread::JoinHandle,
};

/// The relaying thread, and the worker's last error record.
pub struct Relay {
    last: Arc<Mutex<Option<String>>>,
    thread: Option<JoinHandle<()>>,
}

impl Relay {
    /// After the worker exits: its last error record.
    pub fn last_error(&mut self) -> Option<String> {
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        self.last.lock().expect("the relay lock").clone()
    }
}

/// Logs the worker's own records; engine text stays out.
pub fn relay(stderr: impl Read + Send + 'static, who: &'static str) -> Relay {
    let last = Arc::new(Mutex::new(None));
    let kept = Arc::clone(&last);
    let thread = std::thread::spawn(move || {
        let mut withheld = 0u64;
        let (mut reader, mut bytes) = (BufReader::new(stderr), Vec::new());
        loop {
            bytes.clear();
            match reader.read_until(b'\n', &mut bytes) {
                Ok(0) => break,
                Ok(_) => {}
                Err(error) => return log::warn!(target: who, "stderr unreadable: {error}"),
            }
            // Engines may print other encodings; keep reading past them.
            let line = String::from_utf8_lossy(&bytes);
            let line = line.trim_end_matches(['\n', '\r']);
            match parse(line) {
                Some((level, target, message)) => {
                    if level == log::Level::Error {
                        *kept.lock().expect("the relay lock") = Some(message.to_string());
                    }
                    log::log!(target: target, level, "{message}")
                }
                // A crash stack names code, never documents.
                None if frame(line) => log::error!(target: who, "{line}"),
                None => {
                    withheld += 1;
                    log::debug!(target: who, "native output withheld ({withheld} lines)");
                }
            }
        }
    });
    Relay { last, thread: Some(thread) }
}

/// `[time LEVEL target] message`, as env_logger writes it.
fn parse(line: &str) -> Option<(log::Level, &str, &str)> {
    let (head, message) = line.strip_prefix('[')?.split_once("] ")?;
    let mut words = head.split_whitespace();
    let (_, level, target) = (words.next()?, words.next()?, words.next()?);
    Some((level.parse().ok()?, target, message))
}

/// `3   ely-lok-worker   0x0001 symbol + 12` from backtrace_symbols_fd.
fn frame(line: &str) -> bool {
    let mut words = line.split_whitespace();
    words.next().is_some_and(|n| n.parse::<u32>().is_ok())
        && words.next().is_some()
        && words.next().is_some_and(|address| address.starts_with("0x"))
}

#[cfg(test)]
mod tests {
    use super::{frame, parse};

    #[test]
    fn a_record_keeps_its_level_and_target() {
        let line = "[2026-10-08T12:00:00Z WARN  ely_lok::worker] request failed";
        assert_eq!(parse(line), Some((log::Level::Warn, "ely_lok::worker", "request failed")));
        assert_eq!(parse("warning: syntax error: unknown keyword: 'Zq'"), None);
    }

    #[test]
    fn a_line_in_another_encoding_stops_nothing() {
        let stderr: &[u8] = b"caf\xe9 opened\n[2026-10-08T12:00:00Z ERROR ely_pdf::worker] it broke\n";
        let mut relay = super::relay(stderr, "test");
        assert_eq!(relay.last_error().as_deref(), Some("it broke"));
    }

    #[test]
    fn stack_frames_pass_and_engine_text_does_not() {
        assert!(frame("3   ely-lok-worker    0x0000000104f3c960 report + 52"));
        assert!(!frame("syntax error: unknown keyword: 'ZqxPrivate42'"));
    }
}
