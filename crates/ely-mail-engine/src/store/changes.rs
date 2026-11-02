//! Changes made here first, queued for the server (§12.3).

use rusqlite::{OptionalExtension, Transaction, params};

use super::{AccountId, MailboxId, MessageId, Store, aside::Aside, messages};
use crate::{Error, Result};

/// What the reader asks of messages.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Change {
    Seen(bool),
    Flagged(bool),
    /// To Archive, else Gmail's All Mail.
    Archive,
    /// To Trash; for good when it cannot, if `erase`.
    Delete {
        erase: bool,
    },
    /// From Trash to the inbox.
    Restore,
    Move(MailboxId),
}

/// One message as a change reads it.
struct Here {
    id: MessageId,
    account: AccountId,
    mailbox: MailboxId,
    /// Where it shows: a move not yet made.
    shown: MailboxId,
    imap: Option<(u32, u32)>,
    uidl: Option<String>,
}

impl Store {
    /// All or none; the accounts whose server has new work.
    pub fn change(&mut self, ids: &[MessageId], change: Change) -> Result<Vec<AccountId>> {
        self.writable()?;
        let tx = self.conn.transaction()?;
        let mut aside = Aside::new(&self.dir);
        let done = (|| -> Result<Vec<AccountId>> {
            let mut pushed = Vec::new();
            for &id in ids {
                let here = here(&tx, id)?;
                if apply(&tx, &mut aside, &here, change)? && !pushed.contains(&here.account) {
                    pushed.push(here.account);
                }
            }
            // An emptied change stays: a push may be under way.
            tx.commit()?;
            Ok(pushed)
        })();
        let pushed = aside.settle(done)?;
        log::info!("mail: {} messages changed: {change:?}", ids.len());
        Ok(pushed)
    }

    /// How many `Delete` would remove for good.
    pub fn forever(&self, ids: &[MessageId]) -> Result<usize> {
        let tx = self.conn.unchecked_transaction()?;
        let mut count = 0;
        for &id in ids {
            let here = here(&tx, id)?;
            if here.uidl.is_some() || trash(&tx, &here)?.is_none() {
                count += 1;
            }
        }
        Ok(count)
    }

    /// Changes not yet on the account's server.
    pub fn changes_waiting(&self, account: AccountId) -> Result<u64> {
        let count: i64 = self.conn.query_row(
            "SELECT count(*) FROM changes c JOIN mailboxes b ON b.id = c.mailbox
             WHERE b.account = ?1 AND (c.seen IS NOT NULL OR c.flagged IS NOT NULL
                 OR c.target IS NOT NULL OR c.expunge)",
            [account.0],
            |row| row.get(0),
        )?;
        u64::try_from(count).map_err(|_| Error::Store(format!("a negative count: {count}")))
    }
}

fn here(tx: &Transaction, id: MessageId) -> Result<Here> {
    tx.query_row(
        "SELECT b.account, m.mailbox, coalesce(m.moving, m.mailbox), m.uidvalidity, m.uid, m.uidl
         FROM messages m JOIN mailboxes b ON b.id = m.mailbox
         WHERE m.id = ?1 AND NOT m.erasing",
        [id.0],
        |row| {
            let imap = match (row.get(3)?, row.get(4)?) {
                (Some(validity), Some(uid)) => Some((validity, uid)),
                _ => None,
            };
            Ok(Here {
                id,
                account: AccountId(row.get(0)?),
                mailbox: MailboxId(row.get(1)?),
                shown: MailboxId(row.get(2)?),
                imap,
                uidl: row.get(5)?,
            })
        },
    )
    .optional()?
    .ok_or_else(|| Error::Store(format!("no message {}", id.0)))
}

/// Whether the server has new work.
fn apply(tx: &Transaction, aside: &mut Aside, here: &Here, change: Change) -> Result<bool> {
    match change {
        Change::Seen(on) => flag(tx, here, "\\Seen", "seen", on),
        Change::Flagged(on) => flag(tx, here, "\\Flagged", "flagged", on),
        Change::Archive => {
            imap_only(here, "archived")?;
            let target = role(tx, here.account, "archive")?
                .or(role(tx, here.account, "all")?)
                .ok_or_else(|| Error::Invalid("the server has no Archive folder".into()))?;
            relocate(tx, here, target)
        }
        Change::Delete { erase: asked } => match (&here.uidl, trash(tx, here)?) {
            (_, None) | (Some(_), _) if !asked => Err(Error::Invalid(
                "deleting it would remove it for good; ask first".into(),
            )),
            (Some(uidl), _) => {
                tx.execute(
                    "INSERT OR IGNORE INTO pop_gone (mailbox, uidl) VALUES (?1, ?2)",
                    params![here.mailbox.0, uidl],
                )?;
                messages::delete(tx, aside, here.id)?;
                Ok(false)
            }
            (None, Some(trash)) => relocate(tx, here, trash),
            (None, None) => erase(tx, here),
        },
        Change::Restore => {
            imap_only(here, "restored")?;
            if shown_role(tx, here)?.as_deref() != Some("trash") {
                return Err(Error::Invalid("only mail in Trash is restored".into()));
            }
            let inbox = role(tx, here.account, "inbox")?
                .ok_or_else(|| Error::Invalid("the account has no inbox".into()))?;
            relocate(tx, here, inbox)
        }
        Change::Move(target) => {
            imap_only(here, "moved")?;
            let theirs: Option<(i64, bool)> = tx
                .query_row(
                    "SELECT account, local FROM mailboxes WHERE id = ?1",
                    [target.0],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()?;
            match theirs {
                Some((account, false)) if account == here.account.0 => {}
                // The server has no imported folder.
                Some((_, true)) => return Err(Error::Invalid("mail is not moved into an imported folder".into())),
                _ => return Err(Error::Invalid("mail moves within its own account".into())),
            }
            relocate(tx, here, target)
        }
    }
}

fn imap_only(here: &Here, verb: &str) -> Result<()> {
    match here.imap {
        Some(_) => Ok(()),
        None => Err(Error::Invalid(format!("mail kept only here is not {verb}"))),
    }
}

/// Local flags now; IMAP's server later.
fn flag(tx: &Transaction, here: &Here, flag: &str, column: &str, on: bool) -> Result<bool> {
    let was: bool = tx.query_row(
        "SELECT EXISTS (SELECT 1 FROM flags WHERE message = ?1 AND flag = ?2 COLLATE NOCASE)",
        params![here.id.0, flag],
        |row| row.get(0),
    )?;
    messages::put_flag(tx, here.id, flag, on)?;
    if here.imap.is_none() {
        return Ok(false);
    }
    // The first change keeps the server's state.
    let set = format!(
        "{column} = {}, {column}_was = COALESCE({column}_was, {})",
        i64::from(on),
        i64::from(was)
    );
    queue(tx, here, &set)?;
    Ok(true)
}

/// Shown at `target` now; moved on the server later.
fn relocate(tx: &Transaction, here: &Here, target: MailboxId) -> Result<bool> {
    if target == here.shown {
        return Ok(false);
    }
    let (moving, aim) = match target == here.mailbox {
        true => (None, "target = NULL".to_owned()),
        false => (Some(target.0), format!("target = {}", target.0)),
    };
    tx.execute(
        "UPDATE messages SET moving = ?1 WHERE id = ?2",
        params![moving, here.id.0],
    )?;
    queue(tx, here, &aim)?;
    Ok(true)
}

/// Hidden now; gone once the server expunges it.
fn erase(tx: &Transaction, here: &Here) -> Result<bool> {
    queue(tx, here, "target = NULL, expunge = 1")?;
    tx.execute(
        "UPDATE messages SET erasing = 1, moving = NULL WHERE id = ?1",
        [here.id.0],
    )?;
    Ok(true)
}

/// `set` is the change's new part.
fn queue(tx: &Transaction, here: &Here, set: &str) -> Result<()> {
    let (uidvalidity, uid) = here
        .imap
        .ok_or_else(|| Error::Store(format!("message {} has no UID", here.id.0)))?;
    tx.execute(
        "INSERT OR IGNORE INTO changes (mailbox, uidvalidity, uid, version)
         VALUES (?1, ?2, ?3, 0)",
        params![here.mailbox.0, uidvalidity, uid],
    )?;
    tx.execute(
        &format!(
            "UPDATE changes SET {set}, version = version + 1
             WHERE mailbox = ?1 AND uidvalidity = ?2 AND uid = ?3"
        ),
        params![here.mailbox.0, uidvalidity, uid],
    )?;
    Ok(())
}

/// Trash, unless the mail is in it already.
fn trash(tx: &Transaction, here: &Here) -> Result<Option<MailboxId>> {
    if shown_role(tx, here)?.as_deref() == Some("trash") {
        return Ok(None);
    }
    role(tx, here.account, "trash")
}

fn shown_role(tx: &Transaction, here: &Here) -> Result<Option<String>> {
    Ok(tx.query_row(
        "SELECT role FROM mailboxes WHERE id = ?1",
        [here.shown.0],
        |row| row.get(0),
    )?)
}

fn role(tx: &Transaction, account: AccountId, role: &str) -> Result<Option<MailboxId>> {
    Ok(tx
        .query_row(
            "SELECT id FROM mailboxes WHERE account = ?1 AND role = ?2 ORDER BY id LIMIT 1",
            params![account.0, role],
            |row| row.get(0).map(MailboxId),
        )
        .optional()?)
}
