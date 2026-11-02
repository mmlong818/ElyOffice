//! mbox import and export (MAIL-13); imports never reach a server.

use std::{
    io::{BufRead, BufWriter, Write},
    path::Path,
};

use rusqlite::{OptionalExtension, params};

use super::{AccountId, Fetched, Key, MailboxId, Store, now};
use crate::{Error, Mailbox, Result, mbox};

/// An mbox file read into a folder of its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Unpacked {
    pub folder: MailboxId,
    pub messages: usize,
}

/// A folder written as an mbox file.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Packed {
    pub written: usize,
    /// Not here whole, or raw binary: left out.
    pub left: usize,
}

impl Store {
    /// Every message, read, into a new folder; all or none.
    pub fn import_mbox(&mut self, account: AccountId, name: &str, input: impl BufRead) -> Result<Unpacked> {
        self.writable()?;
        let folder = self.import_folder(account, name)?;
        let mut reader = mbox::Reader::new(input);
        let mut count = 0;
        let read = (|| -> Result<()> {
            let unread = |count: usize, err| Error::Invalid(format!("message {} of the file: {err}", count + 1));
            while let Some(bytes) = reader.next_message().map_err(|err| unread(count, err))? {
                count += 1;
                let key = Key::Pop { uidl: format!("mbox:{count}") };
                self.put(folder, &key, &["\\Seen"], Fetched::Whole(&bytes))
                    .map_err(|err| Error::Invalid(format!("message {count} of the file: {err}")))?;
            }
            Ok(())
        })();
        if let Err(error) = read {
            // A failed import leaves no folder.
            self.without_mail(folder, None, false, |tx| tx.execute("DELETE FROM mailboxes WHERE id = ?1", [folder.0]))?;
            return Err(error);
        }
        log::info!("mail: {count} messages imported into folder {}", folder.0);
        Ok(Unpacked { folder, messages: count })
    }

    /// Whole messages, oldest first; `replace` lets an old file go.
    pub fn export_mbox(&self, mailbox: MailboxId, path: &Path, replace: bool) -> Result<Packed> {
        let failed = |err: std::io::Error| Error::Store(format!("{}: {err}", path.display()));
        let dir = path.parent().ok_or_else(|| Error::Invalid(format!("{} has no folder", path.display())))?;
        // A folder gone meanwhile is no empty one.
        let there: Option<i64> =
            self.conn.query_row("SELECT id FROM mailboxes WHERE id = ?1", [mailbox.0], |row| row.get(0)).optional()?;
        if there.is_none() {
            return Err(Error::Invalid("the folder is gone".into()));
        }
        let mut staging = tempfile::NamedTempFile::new_in(dir).map_err(failed)?;
        let rows: Vec<(i64, String, Option<i64>, bool)> = self
            .conn
            .prepare(
                "SELECT id, sender, date, missing = '[]' FROM messages
                 WHERE coalesce(moving, mailbox) = ?1 AND NOT erasing ORDER BY date, id",
            )?
            .query_map([mailbox.0], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)))?
            .collect::<rusqlite::Result<_>>()?;
        let mut packed = Packed::default();
        let mut out = BufWriter::new(staging.as_file_mut());
        for (id, sender, date, whole) in rows {
            // mbox line breaks would change raw binary bytes.
            let bytes = self.original(super::MessageId(id))?.filter(|bytes| whole && mbox::lines_fit(bytes));
            let Some(bytes) = bytes else {
                packed.left += 1;
                continue;
            };
            let sender: Vec<Mailbox> = serde_json::from_str(&sender)?;
            let from = sender.first().map_or("", |each| each.email.as_str());
            // The From line needs a date; undated mail takes now.
            let at = match date {
                Some(at) => at,
                None => now()?,
            };
            mbox::write(&mut out, from, at, &bytes).map_err(failed)?;
            packed.written += 1;
        }
        out.flush().map_err(failed)?;
        drop(out);
        staging.as_file().sync_all().map_err(failed)?;
        match replace {
            true => staging.persist(path).map_err(|err| failed(err.error))?,
            false => staging.persist_noclobber(path).map_err(|err| failed(err.error))?,
        };
        log::info!("mail: folder {} exported: {packed:?}", mailbox.0);
        Ok(packed)
    }

    /// Removes an imported folder and its mail; server folders refuse.
    pub fn remove_imported(&mut self, mailbox: MailboxId) -> Result<()> {
        self.writable()?;
        let local: Option<bool> = self
            .conn
            .query_row("SELECT local FROM mailboxes WHERE id = ?1", [mailbox.0], |row| row.get(0))
            .optional()?;
        match local {
            Some(true) => {}
            Some(false) => return Err(Error::Invalid("only an imported folder is removed here".into())),
            None => return Err(Error::Store(format!("no mailbox {}", mailbox.0))),
        }
        self.without_mail(mailbox, None, false, |tx| tx.execute("DELETE FROM mailboxes WHERE id = ?1", [mailbox.0]))?;
        log::info!("mail: imported folder {} removed", mailbox.0);
        Ok(())
    }

    /// A local folder named `name`, or `name 2` and on.
    fn import_folder(&mut self, account: AccountId, name: &str) -> Result<MailboxId> {
        let name = name.trim();
        if name.is_empty() {
            return Err(Error::Invalid("an imported folder needs a name".into()));
        }
        let mut number = 1;
        loop {
            let named = match number {
                1 => name.to_owned(),
                _ => format!("{name} {number}"),
            };
            let added = self.conn.execute(
                "INSERT OR IGNORE INTO mailboxes (account, name, local) VALUES (?1, ?2, 1)",
                params![account.0, named],
            )?;
            if added == 1 {
                return Ok(MailboxId(self.conn.last_insert_rowid()));
            }
            number += 1;
        }
    }
}
