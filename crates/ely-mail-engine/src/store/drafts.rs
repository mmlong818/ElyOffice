//! Messages being written, kept until sent or thrown away (C12.13).

use rusqlite::{Connection, OptionalExtension, Row, Transaction, params};
use serde::{Deserialize, Serialize};

use super::{AccountId, Store, now};
use crate::{Error, Part, Result, compose::Earlier};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DraftId(pub i64);

/// A message as written, before it is built.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Unsent {
    pub to: Vec<String>,
    pub cc: Vec<String>,
    pub bcc: Vec<String>,
    pub subject: String,
    pub words: String,
    pub markdown: bool,
    pub earlier: Option<Earlier>,
    pub in_reply_to: Option<String>,
    pub references: Vec<String>,
    /// The answered message kept only its headers.
    pub headers_only: bool,
    /// Forwarded files left out, not downloaded.
    pub missing: usize,
}

/// Where the server holds a draft's copy.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Remote {
    pub mailbox: String,
    pub uidvalidity: u32,
    pub uid: u32,
}

/// A server copy, and the account holding it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Left {
    pub account: AccountId,
    pub remote: Remote,
}

/// A draft kept, and a copy its old sender holds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stowed {
    pub id: DraftId,
    pub left: Option<Left>,
}

/// A kept draft, without its files.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shelved {
    pub id: DraftId,
    pub account: AccountId,
    /// Seconds since 1970.
    pub saved: i64,
    pub unsent: Unsent,
    /// Always held by `account`.
    pub remote: Option<Remote>,
}

const COLUMNS: &str = "id, account, saved, body, remote_mailbox, remote_validity, remote_uid";

impl Store {
    /// Keeps `unsent`; `files` of `None` leaves them as they are.
    pub fn shelve(
        &mut self,
        id: Option<DraftId>,
        account: AccountId,
        unsent: &Unsent,
        files: Option<&[Part]>,
    ) -> Result<Stowed> {
        self.writable()?;
        let body = serde_json::to_string(unsent)?;
        let saved = now()?;
        let tx = self.conn.transaction()?;
        let (id, left) = match id {
            Some(id) => {
                let sql = format!("SELECT {COLUMNS} FROM drafts WHERE id = ?1");
                let was = tx
                    .query_row(&sql, [id.0], shelved)
                    .optional()?
                    .transpose()?
                    .ok_or_else(|| Error::Store(format!("draft {} is gone", id.0)))?;
                // A copy stays with the account that holds it.
                let left = match was.remote {
                    Some(remote) if was.account != account => Some(Left {
                        account: was.account,
                        remote,
                    }),
                    _ => None,
                };
                tx.execute(
                    "UPDATE drafts SET account = ?2, saved = ?3, body = ?4 WHERE id = ?1",
                    params![id.0, account.0, saved, body],
                )?;
                if left.is_some() {
                    clear_remote(&tx, id)?;
                }
                (id, left)
            }
            None => (insert(&tx, account, saved, &body)?, None),
        };
        if let Some(files) = files {
            write_files(&tx, FILES, id.0, files)?;
        }
        tx.commit()?;
        log::info!("mail: draft {} kept", id.0);
        Ok(Stowed { id, left })
    }

    /// Every draft, the newest first.
    pub fn drafts(&self) -> Result<Vec<Shelved>> {
        let sql = format!("SELECT {COLUMNS} FROM drafts ORDER BY saved DESC, id DESC");
        let mut query = self.conn.prepare(&sql)?;
        let rows = query.query_map([], shelved)?;
        rows.map(|row| row?).collect()
    }

    pub fn draft(&self, id: DraftId) -> Result<Option<Shelved>> {
        let sql = format!("SELECT {COLUMNS} FROM drafts WHERE id = ?1");
        self.conn
            .query_row(&sql, [id.0], shelved)
            .optional()?
            .transpose()
    }

    pub fn draft_files(&self, id: DraftId) -> Result<Vec<Part>> {
        read_files(&self.conn, FILES, id.0)
    }

    /// Forgets it; gives the server copy still to remove.
    pub fn unshelve(&self, id: DraftId) -> Result<Option<Left>> {
        self.writable()?;
        let left = self.draft(id)?.and_then(|draft| {
            let account = draft.account;
            draft.remote.map(|remote| Left { account, remote })
        });
        self.conn
            .execute("DELETE FROM drafts WHERE id = ?1", [id.0])?;
        log::info!("mail: draft {} dropped", id.0);
        Ok(left)
    }

    /// Records `account`'s copy; gives it back if unowned now.
    pub fn set_remote(
        &self,
        id: DraftId,
        account: AccountId,
        remote: &Remote,
    ) -> Result<Option<Left>> {
        self.writable()?;
        let changed = self.conn.execute(
            "UPDATE drafts SET remote_mailbox = ?3, remote_validity = ?4, remote_uid = ?5
             WHERE id = ?1 AND account = ?2",
            params![
                id.0,
                account.0,
                remote.mailbox,
                remote.uidvalidity,
                remote.uid
            ],
        )?;
        if changed == 1 {
            return Ok(None);
        }
        log::info!("mail: draft {} no longer takes that copy", id.0);
        Ok(Some(Left {
            account,
            remote: remote.clone(),
        }))
    }

    /// Whether the account's drafts go to its server too.
    pub fn drafts_up(&self, account: AccountId) -> Result<bool> {
        let up = self
            .conn
            .query_row(
                "SELECT drafts_up FROM accounts WHERE id = ?1",
                [account.0],
                |row| row.get(0),
            )
            .optional()?;
        up.ok_or_else(|| Error::Store(format!("no account {}", account.0)))
    }

    pub fn set_drafts_up(&self, account: AccountId, up: bool) -> Result<()> {
        self.writable()?;
        let changed = self.conn.execute(
            "UPDATE accounts SET drafts_up = ?2 WHERE id = ?1",
            params![account.0, up],
        )?;
        if changed != 1 {
            return Err(Error::Store(format!("no account {}", account.0)));
        }
        log::info!("mail: account {} drafts to the server: {up}", account.0);
        Ok(())
    }
}

fn clear_remote(tx: &Transaction, id: DraftId) -> Result<()> {
    tx.execute(
        "UPDATE drafts SET remote_mailbox = NULL, remote_validity = NULL, remote_uid = NULL
         WHERE id = ?1",
        [id.0],
    )?;
    Ok(())
}

/// Removes a draft that must be there; gives its copy.
pub(super) fn take(tx: &Transaction, id: DraftId) -> Result<Option<Left>> {
    let sql = format!("SELECT {COLUMNS} FROM drafts WHERE id = ?1");
    let draft = tx
        .query_row(&sql, [id.0], shelved)
        .optional()?
        .transpose()?
        .ok_or_else(|| Error::Store(format!("draft {} is gone", id.0)))?;
    tx.execute("DELETE FROM drafts WHERE id = ?1", [id.0])?;
    let account = draft.account;
    Ok(draft.remote.map(|remote| Left { account, remote }))
}

/// A new draft row.
pub(super) fn insert(
    tx: &Transaction,
    account: AccountId,
    saved: i64,
    body: &str,
) -> Result<DraftId> {
    tx.execute(
        "INSERT INTO drafts (account, saved, body) VALUES (?1, ?2, ?3)",
        params![account.0, saved, body],
    )?;
    Ok(DraftId(tx.last_insert_rowid()))
}

/// The table, and the column naming the owner.
pub(super) type Files = (&'static str, &'static str);

pub(super) const FILES: Files = ("draft_files", "draft");

/// Replaces the owner's files with `files`, in order.
pub(super) fn write_files(
    tx: &Transaction,
    (table, owner): Files,
    id: i64,
    files: &[Part],
) -> Result<()> {
    tx.execute(&format!("DELETE FROM {table} WHERE {owner} = ?1"), [id])?;
    let sql = format!(
        "INSERT INTO {table} ({owner}, place, name, content_type, params, content_id, bytes)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)"
    );
    for (place, part) in files.iter().enumerate() {
        let params = serde_json::to_string(&part.params)?;
        tx.execute(
            &sql,
            params![
                id,
                place as i64,
                part.name,
                part.content_type,
                params,
                part.content_id,
                part.bytes
            ],
        )?;
    }
    Ok(())
}

pub(super) fn read_files(conn: &Connection, (table, owner): Files, id: i64) -> Result<Vec<Part>> {
    let mut query = conn.prepare(&format!(
        "SELECT name, content_type, params, content_id, bytes FROM {table}
         WHERE {owner} = ?1 ORDER BY place"
    ))?;
    let rows = query.query_map([id], |row| {
        let params: String = row.get(2)?;
        Ok((row.get(0)?, row.get(1)?, params, row.get(3)?, row.get(4)?))
    })?;
    rows.map(|row| {
        let (name, content_type, params, content_id, bytes) = row?;
        let params = serde_json::from_str(&params)?;
        Ok(Part {
            name,
            content_type,
            params,
            content_id,
            bytes,
            section: None,
        })
    })
    .collect()
}

fn shelved(row: &Row) -> rusqlite::Result<Result<Shelved>> {
    let body: String = row.get(3)?;
    let mailbox: Option<String> = row.get(4)?;
    let validity: Option<u32> = row.get(5)?;
    let uid: Option<u32> = row.get(6)?;
    let (id, account, saved) = (row.get(0)?, row.get(1)?, row.get(2)?);
    Ok((|| {
        let unsent = serde_json::from_str(&body)?;
        let remote = match (mailbox, validity, uid) {
            (Some(mailbox), Some(uidvalidity), Some(uid)) => Some(Remote {
                mailbox,
                uidvalidity,
                uid,
            }),
            (None, None, None) => None,
            _ => return Err(Error::Store(format!("draft {id} has half a server copy"))),
        };
        Ok(Shelved {
            id: DraftId(id),
            account: AccountId(account),
            saved,
            unsent,
            remote,
        })
    })())
}
