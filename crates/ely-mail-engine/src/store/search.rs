//! The full-text index: words, and CJK characters each.

use std::path::Path;

use rusqlite::{Transaction, params};

use super::{MessageId, Store, messages::load_at};
use crate::{Mail, Mailbox, Result, read};

/// Replaces the message's row in the full-text index.
pub(super) fn index(tx: &Transaction, id: MessageId, mail: &Mail, whole: bool) -> Result<()> {
    let (body, files) = if whole {
        contents(id, mail)
    } else {
        Default::default()
    };
    let row = Row {
        subject: mail.subject.as_deref().unwrap_or_default(),
        sender: &people(&[&mail.from]),
        recipients: &people(&[&mail.to, &mail.cc]),
        body: &body,
        files: &files,
    };
    put(tx, id, &row)
}

struct Row<'a> {
    subject: &'a str,
    sender: &'a str,
    recipients: &'a str,
    body: &'a str,
    files: &'a str,
}

fn put(tx: &Transaction, id: MessageId, row: &Row) -> Result<()> {
    tx.execute("DELETE FROM search WHERE rowid = ?1", [id.0])?;
    tx.execute(
        "INSERT INTO search (rowid, subject, sender, recipients, body, files)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            id.0,
            words(row.subject),
            words(row.sender),
            words(row.recipients),
            words(row.body),
            words(row.files)
        ],
    )?;
    Ok(())
}

/// Every message indexed again from its row and original.
pub(super) fn refill(tx: &Transaction, dir: &Path) -> Result<usize> {
    let ids: Vec<i64> = {
        let mut query = tx.prepare("SELECT id FROM messages")?;
        query
            .query_map([], |row| row.get(0))?
            .collect::<rusqlite::Result<_>>()?
    };
    tx.execute("DELETE FROM search", [])?;
    for &id in &ids {
        let (subject, from, to, cc, content): (
            Option<String>,
            String,
            String,
            String,
            Option<String>,
        ) = tx.query_row(
            "SELECT subject, sender, recipients, copies, content FROM messages WHERE id = ?1",
            [id],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )?;
        let from: Vec<Mailbox> = serde_json::from_str(&from)?;
        let to: Vec<Mailbox> = serde_json::from_str(&to)?;
        let cc: Vec<Mailbox> = serde_json::from_str(&cc)?;
        let (body, files) = match content {
            Some(hash) => contents(MessageId(id), &read(&load_at(dir, &hash)?)?),
            None => Default::default(),
        };
        let row = Row {
            subject: subject.as_deref().unwrap_or_default(),
            sender: &people(&[&from]),
            recipients: &people(&[&to, &cc]),
            body: &body,
            files: &files,
        };
        put(tx, MessageId(id), &row)?;
    }
    Ok(ids.len())
}

impl Store {
    /// Rebuilt from the index rows and the originals.
    pub fn rebuild_search(&mut self) -> Result<usize> {
        self.writable()?;
        let tx = self.conn.transaction()?;
        // A damaged index may not delete.
        let made: String = tx.query_row(
            "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'search'",
            [],
            |row| row.get(0),
        )?;
        tx.execute_batch(&format!("DROP TABLE search; {made};"))?;
        let count = refill(&tx, &self.dir)?;
        tx.commit()?;
        log::info!("mail: search index rebuilt for {count} messages");
        Ok(count)
    }
}

/// Chinese and Japanese: each character is a word.
pub(super) fn words(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        if unspaced(ch) {
            out.push(' ');
            out.push(ch);
            out.push(' ');
        } else {
            out.push(ch);
        }
    }
    out
}

/// Han, kana and their compatibility forms.
fn unspaced(ch: char) -> bool {
    matches!(ch,
        '\u{3040}'..='\u{30FF}'
        | '\u{31F0}'..='\u{31FF}'
        | '\u{3400}'..='\u{4DBF}'
        | '\u{4E00}'..='\u{9FFF}'
        | '\u{F900}'..='\u{FAFF}'
        | '\u{FF66}'..='\u{FF9F}'
        | '\u{20000}'..='\u{3134F}')
}

fn people(lists: &[&[Mailbox]]) -> String {
    let mut out = String::new();
    for person in lists.iter().copied().flatten() {
        if let Some(name) = &person.name {
            out.push_str(name);
            out.push(' ');
        }
        out.push_str(&person.email);
        out.push('\n');
    }
    out
}

/// Every plain and HTML body, and the attachment names.
fn contents(id: MessageId, mail: &Mail) -> (String, String) {
    let mut bodies: Vec<String> = mail.text.iter().cloned().collect();
    if let Some(html) = &mail.html {
        let (text, short) = crate::html::text(html);
        if short {
            log::warn!(
                "mail: message {} nests HTML too deep; its later text is not indexed",
                id.0
            );
        }
        bodies.push(text);
    }
    let body = bodies.join(
        "
",
    );
    let files = mail
        .attachments
        .iter()
        .filter_map(|part| part.name.as_deref())
        .collect::<Vec<_>>()
        .join("\n");
    (body, files)
}
