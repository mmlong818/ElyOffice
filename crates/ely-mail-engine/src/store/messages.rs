use std::{fs, io::ErrorKind, path::Path};

use rusqlite::{OptionalExtension, Row, ToSql, Transaction, params};
use sha2::{Digest, Sha256};

use super::{CONTENT, MailboxId, MessageId, Store, aside::Aside, disk, rebuilt, search, threads};
use crate::{Error, Mail, Mailbox, Result, read};

/// RFC 3501 §2.3.1.1: identity is mailbox, UIDVALIDITY and UID.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Key {
    Imap {
        uidvalidity: u32,
        uid: u32,
    },
    /// POP3's UIDL, or `mbox:N` for mail imported here.
    Pop {
        uidl: String,
    },
}

/// What a sync brought down.
pub enum Fetched<'a> {
    Headers(&'a [u8]),
    /// Left-out parts are empty and named by part number.
    Text {
        bytes: &'a [u8],
        missing: &'a [String],
    },
    Whole(&'a [u8]),
}

/// What a write keeps: a header block, or an original.
pub(super) enum Kept<'a> {
    Head(&'a [u8]),
    Body {
        content: &'a str,
        missing: &'a [String],
    },
}

/// A message as indexed; the original is read apart.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stored {
    pub id: MessageId,
    pub mailbox: MailboxId,
    pub key: Key,
    pub flags: Vec<String>,
    pub message_id: Option<String>,
    pub in_reply_to: Vec<String>,
    pub references: Vec<String>,
    pub subject: Option<String>,
    pub from: Vec<Mailbox>,
    pub to: Vec<Mailbox>,
    pub cc: Vec<Mailbox>,
    pub date: Option<i64>,
    /// None until the body is here.
    pub attachments: Option<u32>,
    pub body: bool,
    /// Part numbers not downloaded; their content is empty.
    pub missing: Vec<String>,
}

const COLUMNS: &str = "id, mailbox, uidvalidity, uid, uidl, message_id, in_reply_to, refs, \
                       subject, sender, recipients, copies, date, attachments, content, missing";

impl Store {
    pub fn find(&self, mailbox: MailboxId, key: &Key) -> Result<Option<MessageId>> {
        let found = match key {
            Key::Imap { uidvalidity, uid } => self.conn.query_row(
                "SELECT id FROM messages WHERE mailbox = ?1 AND uidvalidity = ?2 AND uid = ?3",
                params![mailbox.0, uidvalidity, uid],
                |row| row.get(0),
            ),
            Key::Pop { uidl } => self.conn.query_row(
                "SELECT id FROM messages WHERE mailbox = ?1 AND uidl = ?2",
                params![mailbox.0, uidl],
                |row| row.get(0),
            ),
        };
        Ok(found.optional()?.map(MessageId))
    }

    /// Deleted here, waiting for the server.
    fn deleted(&self, mailbox: MailboxId, key: &Key) -> Result<bool> {
        let found = match key {
            Key::Imap { uidvalidity, uid } => self.conn.query_row(
                "SELECT 1 FROM changes WHERE mailbox = ?1 AND uidvalidity = ?2 AND uid = ?3
                 AND expunge",
                params![mailbox.0, uidvalidity, uid],
                |_| Ok(()),
            ),
            Key::Pop { uidl } => self.conn.query_row(
                "SELECT 1 FROM pop_gone WHERE mailbox = ?1 AND uidl = ?2",
                params![mailbox.0, uidl],
                |_| Ok(()),
            ),
        };
        Ok(found.optional()?.is_some())
    }

    /// One key, one message; headers again only set flags.
    pub fn put(
        &mut self,
        mailbox: MailboxId,
        key: &Key,
        flags: &[&str],
        fetched: Fetched,
    ) -> Result<MessageId> {
        self.writable()?;
        if self.deleted(mailbox, key)? {
            return Err(Error::Deleted);
        }
        let existing = self.find(mailbox, key)?;
        let (bytes, missing): (&[u8], &[String]) = match (fetched, existing) {
            (Fetched::Headers(_), Some(id)) => {
                self.set_flags(id, flags)?;
                return Ok(id);
            }
            (Fetched::Headers(bytes), None) => {
                let mail = read(bytes)?;
                return self.write(mailbox, key, flags, &mail, Kept::Head(bytes), None);
            }
            (Fetched::Text { bytes, missing }, _) => (bytes, missing),
            (Fetched::Whole(bytes), _) => (bytes, &[][..]),
        };
        let mail = read(bytes)?;
        let (content, made) = self.keep(bytes)?;
        let body = Kept::Body {
            content: &content,
            missing,
        };
        let written = self.write(mailbox, key, flags, &mail, body, existing);
        if written.is_err() && made {
            let mut aside = Aside::new(&self.dir);
            let gone = aside.drop_unused(&self.conn, &content);
            if let Err(gone) = aside.settle(gone) {
                log::error!("mail: an unused original stays after a failed write: {gone}");
            }
        }
        written
    }

    pub(super) fn write(
        &mut self,
        mailbox: MailboxId,
        key: &Key,
        flags: &[&str],
        mail: &Mail,
        kept: Kept,
        existing: Option<MessageId>,
    ) -> Result<MessageId> {
        let (content, missing, head) = match kept {
            Kept::Head(head) => (None, &[][..], Some(head)),
            Kept::Body { content, missing } => (Some(content), missing, None),
        };
        let (uidvalidity, uid, uidl) = match key {
            Key::Imap { uidvalidity, uid } => (Some(*uidvalidity), Some(*uid), None),
            Key::Pop { uidl } => (None, None, Some(uidl.as_str())),
        };
        let attachments = content.map(|_| mail.attachments.len() as u32);
        let ident = rebuilt::ident(mail);
        let json = [
            serde_json::to_string(&mail.in_reply_to)?,
            serde_json::to_string(&mail.references)?,
            serde_json::to_string(&mail.from)?,
            serde_json::to_string(&mail.to)?,
            serde_json::to_string(&mail.cc)?,
            serde_json::to_string(missing)?,
        ];
        let mut values: Vec<&dyn ToSql> = vec![
            &mail.message_id,
            &json[0],
            &json[1],
            &mail.subject,
            &json[2],
            &json[3],
            &json[4],
            &mail.date,
            &attachments,
            &content,
            &json[5],
            &head,
        ];
        let tx = self.conn.transaction()?;
        let mut aside = Aside::new(&self.dir);
        let done = (|| -> Result<MessageId> {
            let mut old: Option<String> = None;
            let id = match &existing {
                Some(id) => {
                    old = tx.query_row(
                        "SELECT content FROM messages WHERE id = ?1",
                        [id.0],
                        |row| row.get(0),
                    )?;
                    values.push(&id.0);
                    tx.execute(
                        "UPDATE messages SET message_id = ?1, in_reply_to = ?2, refs = ?3,
                         subject = ?4, sender = ?5, recipients = ?6, copies = ?7, date = ?8,
                         attachments = ?9, content = ?10, missing = ?11, head = ?12, trimmed = 0
                         WHERE id = ?13",
                        values.as_slice(),
                    )?;
                    *id
                }
                None => {
                    values.extend::<[&dyn ToSql; 5]>([&mailbox.0, &uidvalidity, &uid, &uidl, &ident]);
                    tx.execute(
                        "INSERT INTO messages (message_id, in_reply_to, refs, subject, sender,
                         recipients, copies, date, attachments, content, missing, head, mailbox,
                         uidvalidity, uid, uidl, ident) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9,
                         ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)",
                        values.as_slice(),
                    )?;
                    MessageId(tx.last_insert_rowid())
                }
            };
            replace_flags(&tx, id, flags)?;
            threads::link(&tx, id, mail)?;
            search::index(&tx, id, mail, content.is_some())?;
            if let Some(old) = old.filter(|old| Some(old.as_str()) != content) {
                aside.drop_unused(&tx, &old)?;
            }
            tx.commit()?;
            Ok(id)
        })();
        aside.settle(done)
    }

    pub fn set_flags(&mut self, id: MessageId, flags: &[&str]) -> Result<()> {
        self.writable()?;
        self.content(id)?;
        let tx = self.conn.transaction()?;
        replace_flags(&tx, id, flags)?;
        tx.commit()?;
        Ok(())
    }

    pub fn get(&self, id: MessageId) -> Result<Stored> {
        let mut stored = self.conn.query_row(
            &format!("SELECT {COLUMNS} FROM messages WHERE id = ?1 AND NOT erasing"),
            [id.0],
            |found| Ok(row(found)),
        )??;
        let mut query = self
            .conn
            .prepare("SELECT flag FROM flags WHERE message = ?1 ORDER BY flag")?;
        stored.flags = query
            .query_map([id.0], |row| row.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        Ok(stored)
    }

    /// The original, checked against its hash; None for headers only.
    pub fn original(&self, id: MessageId) -> Result<Option<Vec<u8>>> {
        self.content(id)?
            .map(|hash| load_at(&self.dir, &hash))
            .transpose()
    }

    /// The header block kept while the body is not.
    pub fn head(&self, id: MessageId) -> Result<Vec<u8>> {
        let head: Option<Vec<u8>> = self
            .conn
            .query_row("SELECT head FROM messages WHERE id = ?1", [id.0], |row| {
                row.get(0)
            })
            .optional()?
            .ok_or_else(|| Error::Store(format!("no message {}", id.0)))?;
        head.ok_or_else(|| Error::Store(format!("message {} kept no header block", id.0)))
    }

    pub fn remove(&mut self, id: MessageId) -> Result<()> {
        self.writable()?;
        self.content(id)?;
        let tx = self.conn.transaction()?;
        let mut aside = Aside::new(&self.dir);
        let done = (|| -> Result<()> {
            delete(&tx, &mut aside, id)?;
            Ok(tx.commit()?)
        })();
        aside.settle(done)
    }

    fn content(&self, id: MessageId) -> Result<Option<String>> {
        self.conn
            .query_row(
                "SELECT content FROM messages WHERE id = ?1",
                [id.0],
                |row| row.get(0),
            )
            .optional()?
            .ok_or_else(|| Error::Store(format!("no message {}", id.0)))
    }

    /// The hash, and whether the file is new.
    fn keep(&self, bytes: &[u8]) -> Result<(String, bool)> {
        let hash = hex(&Sha256::digest(bytes));
        let path = self.dir.join(CONTENT).join(&hash[..2]).join(&hash);
        let made = match fs::read(&path) {
            Ok(held) if hex(&Sha256::digest(&held)) == hash => return Ok((hash, false)),
            Ok(_) => {
                log::warn!("mail: original {hash} was damaged; rewritten from the server copy");
                false
            }
            Err(err) if err.kind() == ErrorKind::NotFound => true,
            Err(err) => return Err(disk("checking an original", err)),
        };
        let shard = path.parent().expect("a shard folder");
        fs::create_dir_all(shard).map_err(|err| disk("storing an original", err))?;
        let staged = shard.join(format!(".{hash}.part"));
        if let Err(err) = fs::write(&staged, bytes).and_then(|()| fs::rename(&staged, &path)) {
            return Err(match fs::remove_file(&staged) {
                Ok(()) => disk("storing an original", err),
                Err(gone) if gone.kind() == ErrorKind::NotFound => disk("storing an original", err),
                Err(left) => Error::Store(format!(
                    "storing an original: {err}; the staged copy stayed: {left}"
                )),
            });
        }
        Ok((hash, made))
    }
}

/// The row, and its original once unused.
pub(super) fn delete(tx: &Transaction, aside: &mut Aside, id: MessageId) -> Result<()> {
    let content: Option<String> = tx.query_row(
        "SELECT content FROM messages WHERE id = ?1",
        [id.0],
        |row| row.get(0),
    )?;
    tx.execute("DELETE FROM messages WHERE id = ?1", [id.0])?;
    if let Some(hash) = content {
        aside.drop_unused(tx, &hash)?;
    }
    Ok(())
}

/// The server's flags, under changes not yet sent.
fn replace_flags(tx: &Transaction, id: MessageId, flags: &[&str]) -> Result<()> {
    tx.execute("DELETE FROM flags WHERE message = ?1", [id.0])?;
    for flag in flags {
        tx.execute(
            "INSERT OR IGNORE INTO flags (message, flag) VALUES (?1, ?2)",
            params![id.0, flag],
        )?;
    }
    let has = |name: &str| flags.iter().any(|it| it.eq_ignore_ascii_case(name));
    // A refused change returns to these.
    tx.execute(
        "UPDATE changes SET seen_was = CASE WHEN seen IS NULL THEN seen_was ELSE ?1 END,
             flagged_was = CASE WHEN flagged IS NULL THEN flagged_was ELSE ?2 END
         WHERE (mailbox, uidvalidity, uid) =
             (SELECT mailbox, uidvalidity, uid FROM messages WHERE id = ?3)",
        params![has("\\Seen"), has("\\Flagged"), id.0],
    )?;
    let queued: Option<(Option<bool>, Option<bool>)> = tx
        .query_row(
            "SELECT c.seen, c.flagged FROM changes c JOIN messages m ON m.mailbox = c.mailbox
             AND m.uidvalidity = c.uidvalidity AND m.uid = c.uid WHERE m.id = ?1",
            [id.0],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    let (seen, flagged) = queued.unwrap_or_default();
    for (flag, on) in [("\\Seen", seen), ("\\Flagged", flagged)] {
        if let Some(on) = on {
            put_flag(tx, id, flag, on)?;
        }
    }
    Ok(())
}

/// One system flag on or off.
pub(super) fn put_flag(tx: &Transaction, id: MessageId, flag: &str, on: bool) -> Result<()> {
    // RFC 3501 §2.3.2: system flags match in any case.
    tx.execute(
        "DELETE FROM flags WHERE message = ?1 AND flag = ?2 COLLATE NOCASE",
        params![id.0, flag],
    )?;
    if on {
        tx.execute(
            "INSERT INTO flags (message, flag) VALUES (?1, ?2)",
            params![id.0, flag],
        )?;
    }
    Ok(())
}

fn row(row: &Row) -> Result<Stored> {
    let key = match row.get::<_, Option<String>>(4)? {
        Some(uidl) => Key::Pop { uidl },
        None => Key::Imap {
            uidvalidity: row.get(2)?,
            uid: row.get(3)?,
        },
    };
    let json = |at: usize| -> Result<String> { Ok(row.get(at)?) };
    Ok(Stored {
        id: MessageId(row.get(0)?),
        mailbox: MailboxId(row.get(1)?),
        key,
        flags: Vec::new(),
        message_id: row.get(5)?,
        in_reply_to: serde_json::from_str(&json(6)?)?,
        references: serde_json::from_str(&json(7)?)?,
        subject: row.get(8)?,
        from: serde_json::from_str(&json(9)?)?,
        to: serde_json::from_str(&json(10)?)?,
        cc: serde_json::from_str(&json(11)?)?,
        date: row.get(12)?,
        attachments: row.get(13)?,
        body: row.get::<_, Option<String>>(14)?.is_some(),
        missing: serde_json::from_str(&json(15)?)?,
    })
}

pub(super) fn load_at(dir: &Path, hash: &str) -> Result<Vec<u8>> {
    let path = dir.join(CONTENT).join(&hash[..2]).join(hash);
    let bytes = fs::read(&path).map_err(|err| disk("reading an original", err))?;
    if hex(&Sha256::digest(&bytes)) != hash {
        return Err(Error::Store(format!("original {hash} is damaged")));
    }
    Ok(bytes)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
