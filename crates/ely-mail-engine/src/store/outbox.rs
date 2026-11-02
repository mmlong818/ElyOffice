//! Messages handed over to send, until done (C12.14, §12.4).

use rusqlite::{OptionalExtension, Row, params};

use super::{
    AccountId, DraftId, Left, Store,
    drafts::{self, Files},
    now,
};
use crate::{Error, Part, Result, Unsent};

const FILES: Files = ("outbox_files", "item");

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct OutboxId(pub i64);

/// Where a queued message stands; drafts stay apart.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Going {
    Queued,
    Sending,
    /// The server took it; the Sent copy may wait.
    Accepted,
    Failed,
    /// It went out; whether it was taken is unknown.
    Unsure,
    Cancelled,
}

/// An accepted message's copy for the Sent folder.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Filing {
    Waiting,
    Failed,
}

/// One queued message, without its bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Queued {
    pub id: OutboxId,
    pub account: AccountId,
    pub going: Going,
    pub filing: Option<Filing>,
    pub message_id: String,
    pub unsent: Unsent,
    /// Seconds since 1970.
    pub queued: i64,
    pub tried: Option<i64>,
    /// None for a send ElyMail stopped during.
    pub error: Option<String>,
}

/// A built message: with Bcc, for the Sent copy.
pub struct Letter {
    pub envelope: Vec<String>,
    pub message_id: String,
    pub bytes: Vec<u8>,
}

/// What a send hands to the server.
pub struct Handed {
    pub account: AccountId,
    pub envelope: Vec<String>,
    pub bytes: Vec<u8>,
}

/// How a send ended.
pub enum Landing {
    /// `file` keeps it for its Sent copy.
    Accepted {
        file: bool,
    },
    /// The server was not reached; it stays queued.
    Waiting(String),
    Failed(String),
    Unsure(String),
}

/// How placing its Sent copy ended.
pub enum Filed {
    Done,
    /// The server was not reached; it stays waiting.
    Later(String),
    Failed(String),
}

impl Going {
    fn key(self) -> &'static str {
        match self {
            Going::Queued => "queued",
            Going::Sending => "sending",
            Going::Accepted => "accepted",
            Going::Failed => "failed",
            Going::Unsure => "unsure",
            Going::Cancelled => "cancelled",
        }
    }

    fn from_key(key: &str) -> Result<Going> {
        let all = [
            Going::Queued,
            Going::Sending,
            Going::Accepted,
            Going::Failed,
            Going::Unsure,
            Going::Cancelled,
        ];
        all.into_iter()
            .find(|going| going.key() == key)
            .ok_or_else(|| Error::Store(format!("an outbox state {key:?}")))
    }
}

const COLUMNS: &str = "id, account, state, filing, message_id, body, queued, tried, error";

impl Store {
    /// Queues it, its `draft` gone in the same step.
    pub fn enqueue(
        &mut self,
        account: AccountId,
        draft: Option<DraftId>,
        unsent: &Unsent,
        files: &[Part],
        letter: &Letter,
    ) -> Result<(OutboxId, Option<Left>)> {
        self.writable()?;
        if letter.envelope.is_empty() {
            return Err(Error::Invalid("a message needs a recipient".into()));
        }
        let body = serde_json::to_string(unsent)?;
        let envelope = serde_json::to_string(&letter.envelope)?;
        let queued = now()?;
        let tx = self.conn.transaction()?;
        let left = match draft {
            Some(draft) => drafts::take(&tx, draft)?,
            None => None,
        };
        tx.execute(
            "INSERT INTO outbox (account, state, message_id, body, envelope, bytes, queued)
             VALUES (?1, 'queued', ?2, ?3, ?4, ?5, ?6)",
            params![
                account.0,
                letter.message_id,
                body,
                envelope,
                letter.bytes,
                queued
            ],
        )?;
        let id = OutboxId(tx.last_insert_rowid());
        drafts::write_files(&tx, FILES, id.0, files)?;
        super::contacts::remember(&tx, unsent.to.iter().chain(&unsent.cc).chain(&unsent.bcc))?;
        tx.commit()?;
        log::info!("mail: message {} queued, draft {draft:?} taken", id.0);
        Ok((id, left))
    }

    /// Every queued message, the oldest first.
    pub fn outbox(&self) -> Result<Vec<Queued>> {
        let sql = format!("SELECT {COLUMNS} FROM outbox ORDER BY id");
        let mut query = self.conn.prepare(&sql)?;
        let rows = query.query_map([], queued)?;
        rows.map(|row| row?).collect()
    }

    pub fn queued(&self, id: OutboxId) -> Result<Queued> {
        let sql = format!("SELECT {COLUMNS} FROM outbox WHERE id = ?1");
        self.conn
            .query_row(&sql, [id.0], queued)
            .optional()?
            .transpose()?
            .ok_or_else(|| Error::Store(format!("queued message {} is gone", id.0)))
    }

    /// Those to send now; `waiting` adds the unreached.
    pub fn to_send(&self, waiting: bool) -> Result<Vec<OutboxId>> {
        let mut query = self.conn.prepare(
            "SELECT id FROM outbox
             WHERE state = 'queued' AND (?1 OR error IS NULL) ORDER BY id",
        )?;
        let rows = query.query_map([waiting], |row| row.get(0))?;
        rows.map(|row| Ok(OutboxId(row?))).collect()
    }

    /// Marks it sending; None if no longer queued.
    pub fn take_queued(&self, id: OutboxId) -> Result<Option<Handed>> {
        self.writable()?;
        let changed = self.conn.execute(
            "UPDATE outbox SET state = 'sending', tried = ?2
             WHERE id = ?1 AND state = 'queued'",
            params![id.0, now()?],
        )?;
        if changed == 0 {
            log::info!("mail: message {} is no longer queued", id.0);
            return Ok(None);
        }
        let (account, envelope, bytes): (i64, String, Vec<u8>) = self.conn.query_row(
            "SELECT account, envelope, bytes FROM outbox WHERE id = ?1",
            [id.0],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;
        Ok(Some(Handed {
            account: AccountId(account),
            envelope: serde_json::from_str(&envelope)?,
            bytes,
        }))
    }

    /// Records how a send ended; accepted alone may go.
    pub fn landed(&self, id: OutboxId, landing: Landing) -> Result<()> {
        self.writable()?;
        let (state, filing, error) = match &landing {
            Landing::Accepted { file: false } => {
                let gone = self.conn.execute(
                    "DELETE FROM outbox WHERE id = ?1 AND state = 'sending'",
                    [id.0],
                )?;
                if gone != 1 {
                    return Err(not_sending(id));
                }
                log::info!("mail: message {} accepted and done", id.0);
                return Ok(());
            }
            Landing::Accepted { file: true } => ("accepted", Some("waiting"), None),
            Landing::Waiting(error) => ("queued", None, Some(error.as_str())),
            Landing::Failed(error) => ("failed", None, Some(error.as_str())),
            Landing::Unsure(error) => ("unsure", None, Some(error.as_str())),
        };
        let changed = self.conn.execute(
            "UPDATE outbox SET state = ?2, filing = ?3, error = ?4
             WHERE id = ?1 AND state = 'sending'",
            params![id.0, state, filing, error],
        )?;
        if changed == 0 {
            return Err(not_sending(id));
        }
        log::info!("mail: message {} is {state}", id.0);
        Ok(())
    }

    /// Accepted messages whose Sent copy waits.
    pub fn to_file(&self) -> Result<Vec<OutboxId>> {
        let mut query = self.conn.prepare(
            "SELECT id FROM outbox WHERE state = 'accepted' AND filing = 'waiting' ORDER BY id",
        )?;
        let rows = query.query_map([], |row| row.get(0))?;
        rows.map(|row| Ok(OutboxId(row?))).collect()
    }

    /// The account and bytes of a copy to file.
    pub fn filing(&self, id: OutboxId) -> Result<Option<(AccountId, Vec<u8>)>> {
        let row = self
            .conn
            .query_row(
                "SELECT account, bytes FROM outbox
                 WHERE id = ?1 AND state = 'accepted' AND filing = 'waiting'",
                [id.0],
                |row| Ok((AccountId(row.get(0)?), row.get(1)?)),
            )
            .optional()?;
        Ok(row)
    }

    /// Done, the record goes; else the copy waits or failed.
    pub fn filed(&self, id: OutboxId, filed: Filed) -> Result<()> {
        self.writable()?;
        let changed = match &filed {
            Filed::Done => self.conn.execute(
                "DELETE FROM outbox WHERE id = ?1 AND state = 'accepted'",
                [id.0],
            )?,
            Filed::Later(error) | Filed::Failed(error) => {
                let filing = match filed {
                    Filed::Later(_) => "waiting",
                    _ => "failed",
                };
                self.conn.execute(
                    "UPDATE outbox SET filing = ?2, error = ?3
                     WHERE id = ?1 AND state = 'accepted'",
                    params![id.0, filing, error],
                )?
            }
        };
        if changed != 1 {
            return Err(Error::Store(format!("message {} was not accepted", id.0)));
        }
        log::info!("mail: message {} Sent copy settled", id.0);
        Ok(())
    }

    /// The user asks again: to send, or to file.
    pub fn retry(&self, id: OutboxId) -> Result<()> {
        self.writable()?;
        let changed = self.conn.execute(
            "UPDATE outbox SET
                 state = CASE state WHEN 'accepted' THEN 'accepted' ELSE 'queued' END,
                 filing = CASE state WHEN 'accepted' THEN 'waiting' END,
                 error = NULL
             WHERE id = ?1 AND (state IN ('failed', 'unsure', 'cancelled')
                 OR (state IN ('queued', 'accepted') AND error IS NOT NULL))",
            [id.0],
        )?;
        refused(changed, id, "tried again")
    }

    /// Only a message not yet out can be held back.
    pub fn cancel(&self, id: OutboxId) -> Result<()> {
        self.writable()?;
        let changed = self.conn.execute(
            "UPDATE outbox SET state = 'cancelled', error = NULL
             WHERE id = ?1 AND state IN ('queued', 'failed')",
            [id.0],
        )?;
        refused(changed, id, "cancelled")
    }

    /// Back to a draft, to write on; the record goes.
    pub fn unqueue(&mut self, id: OutboxId) -> Result<DraftId> {
        self.writable()?;
        let held = self.queued(id)?;
        if !matches!(held.going, Going::Failed | Going::Unsure | Going::Cancelled) {
            return Err(Error::Invalid(format!(
                "message {} cannot be edited now",
                id.0
            )));
        }
        let files = drafts::read_files(&self.conn, FILES, id.0)?;
        let body = serde_json::to_string(&held.unsent)?;
        let tx = self.conn.transaction()?;
        let draft = drafts::insert(&tx, held.account, now()?, &body)?;
        drafts::write_files(&tx, drafts::FILES, draft.0, &files)?;
        tx.execute("DELETE FROM outbox WHERE id = ?1", [id.0])?;
        tx.commit()?;
        log::info!("mail: message {} is draft {} again", id.0, draft.0);
        Ok(draft)
    }

    /// Removes a record no send waits on.
    pub fn forget(&self, id: OutboxId) -> Result<()> {
        self.writable()?;
        let changed = self.conn.execute(
            "DELETE FROM outbox WHERE id = ?1 AND state NOT IN ('queued', 'sending')
                 AND NOT (state = 'accepted' AND filing = 'waiting')",
            [id.0],
        )?;
        refused(changed, id, "removed")
    }

    /// At start: a send cut off is unknown, never resent.
    pub fn recover_outbox(&self) -> Result<usize> {
        self.writable()?;
        let cut = self.conn.execute(
            "UPDATE outbox SET state = 'unsure', error = NULL WHERE state = 'sending'",
            [],
        )?;
        if cut > 0 {
            log::warn!("mail: {cut} sends were cut off; their outcome is unknown");
        }
        Ok(cut)
    }

    /// Messages a quit would leave unsent or unfiled.
    pub fn pending(&self) -> Result<usize> {
        let count: i64 = self.conn.query_row(
            "SELECT count(*) FROM outbox WHERE state IN ('queued', 'sending')
                 OR (state = 'accepted' AND filing = 'waiting')",
            [],
            |row| row.get(0),
        )?;
        Ok(count as usize)
    }

    /// The user's choice; None follows the provider.
    pub fn sent_copy(&self, account: AccountId) -> Result<Option<bool>> {
        let row = self
            .conn
            .query_row(
                "SELECT sent_copy FROM accounts WHERE id = ?1",
                [account.0],
                |row| row.get(0),
            )
            .optional()?;
        row.ok_or_else(|| Error::Store(format!("no account {}", account.0)))
    }

    pub fn set_sent_copy(&self, account: AccountId, copy: Option<bool>) -> Result<()> {
        self.writable()?;
        let changed = self.conn.execute(
            "UPDATE accounts SET sent_copy = ?2 WHERE id = ?1",
            params![account.0, copy],
        )?;
        refused_account(changed, account)?;
        log::info!("mail: account {} Sent copies: {copy:?}", account.0);
        Ok(())
    }

    /// The outgoing server's last stated SIZE, in bytes.
    pub fn smtp_limit(&self, account: AccountId) -> Result<Option<u64>> {
        let row: Option<Option<i64>> = self
            .conn
            .query_row(
                "SELECT smtp_limit FROM accounts WHERE id = ?1",
                [account.0],
                |row| row.get(0),
            )
            .optional()?;
        let limit = row.ok_or_else(|| Error::Store(format!("no account {}", account.0)))?;
        Ok(limit.map(|bytes| bytes as u64))
    }

    pub fn set_smtp_limit(&self, account: AccountId, limit: Option<u64>) -> Result<()> {
        self.writable()?;
        let limit = limit.map(|bytes| bytes as i64);
        let changed = self.conn.execute(
            "UPDATE accounts SET smtp_limit = ?2 WHERE id = ?1",
            params![account.0, limit],
        )?;
        refused_account(changed, account)
    }
}

fn not_sending(id: OutboxId) -> Error {
    Error::Store(format!("message {} was not being sent", id.0))
}

fn refused(changed: usize, id: OutboxId, what: &str) -> Result<()> {
    if changed != 1 {
        return Err(Error::Invalid(format!(
            "message {} cannot be {what} now",
            id.0
        )));
    }
    log::info!("mail: message {} {what}", id.0);
    Ok(())
}

fn refused_account(changed: usize, account: AccountId) -> Result<()> {
    match changed {
        1 => Ok(()),
        _ => Err(Error::Store(format!("no account {}", account.0))),
    }
}

fn queued(row: &Row) -> rusqlite::Result<Result<Queued>> {
    let state: String = row.get(2)?;
    let filing: Option<String> = row.get(3)?;
    let body: String = row.get(5)?;
    let (id, account, message_id) = (row.get(0)?, row.get(1)?, row.get(4)?);
    let (queued, tried, error) = (row.get(6)?, row.get(7)?, row.get(8)?);
    Ok((|| {
        let filing = match filing.as_deref() {
            None => None,
            Some("waiting") => Some(Filing::Waiting),
            Some("failed") => Some(Filing::Failed),
            Some(other) => return Err(Error::Store(format!("a Sent copy state {other:?}"))),
        };
        Ok(Queued {
            id: OutboxId(id),
            account: AccountId(account),
            going: Going::from_key(&state)?,
            filing,
            message_id,
            unsent: serde_json::from_str(&body)?,
            queued,
            tried,
            error,
        })
    })())
}
