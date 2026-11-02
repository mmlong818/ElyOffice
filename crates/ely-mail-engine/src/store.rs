//! The local index, the originals and the search index.

use std::{
    fs, io,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use rusqlite::{Connection, OpenFlags, OptionalExtension, params};

use crate::{Error, Result};

mod accounts;
mod aside;
mod changes;
mod contacts;
mod drafts;
mod find;
mod folders;
mod identity;
mod listing;
mod mbox;
mod messages;
mod outbox;
mod pushed;
mod rebuilt;
mod rules;
mod schema;
mod search;
#[cfg(test)]
mod tests;
mod threads;
mod trim;
mod trusted;

pub use accounts::Cached;
pub use changes::Change;
pub use contacts::{Contact, ContactId, Imported};
pub use drafts::{DraftId, Left, Remote, Shelved, Stowed, Unsent};
pub use find::{Field, Found, Search};
pub use folders::{Held, LocalFolder, Synced};
pub use identity::Identity;
pub use listing::{Filter, Scope, Summary};
pub use mbox::{Packed, Unpacked};
pub use messages::{Fetched, Key, Stored};
pub use outbox::{Filed, Filing, Going, Handed, Landing, Letter, OutboxId, Queued};
pub(crate) use pushed::{Done, Pending, Settled};
pub use rules::{Action, Arrivals, Match, Rule, RuleId};

const DB: &str = "mail.db";
const CONTENT: &str = "content";
const BACKUPS: &str = "backups";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AccountId(pub i64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MailboxId(pub i64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MessageId(pub i64);

/// Bytes on disk per kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sizes {
    pub index: u64,
    pub search: u64,
    pub originals: u64,
}

pub struct Store {
    conn: Connection,
    dir: PathBuf,
    /// Why writes are refused, when they are.
    read_only: Option<String>,
}

impl Store {
    pub fn open(dir: &Path) -> Result<Self> {
        open(dir, schema::STEPS)
    }

    pub fn read_only(&self) -> Option<&str> {
        self.read_only.as_deref()
    }

    fn writable(&self) -> Result<()> {
        match &self.read_only {
            Some(reason) => Err(Error::ReadOnly(reason.clone())),
            None => Ok(()),
        }
    }

    /// The server mailbox by name, made on first use.
    pub fn mailbox(&self, account: AccountId, name: &str) -> Result<MailboxId> {
        let name = mailbox_name(name);
        let find = "SELECT id FROM mailboxes WHERE account = ?1 AND name = ?2 AND NOT local";
        if let Some(id) = self
            .conn
            .query_row(find, params![account.0, name], |row| row.get(0))
            .optional()?
        {
            return Ok(MailboxId(id));
        }
        self.writable()?;
        self.conn.execute(
            "INSERT INTO mailboxes (account, name) VALUES (?1, ?2)",
            params![account.0, name],
        )?;
        Ok(MailboxId(self.conn.last_insert_rowid()))
    }

    /// The index counts the WAL and shared-memory files.
    pub fn sizes(&self) -> Result<Sizes> {
        let mut files = 0;
        for name in [DB.to_owned(), format!("{DB}-wal"), format!("{DB}-shm")] {
            files += match fs::metadata(self.dir.join(&name)) {
                Ok(meta) => meta.len(),
                Err(err) if err.kind() == io::ErrorKind::NotFound => 0,
                Err(err) => return Err(disk("measuring the index", err)),
            };
        }
        let search: i64 = self.conn.query_row(
            "SELECT coalesce(sum(pgsize), 0) FROM dbstat
             WHERE name = 'search' OR name GLOB 'search_*'",
            [],
            |row| row.get(0),
        )?;
        let search = u64::try_from(search)
            .map_err(|_| Error::Store(format!("a negative size: {search}")))?;
        let index = files.checked_sub(search).ok_or_else(|| {
            Error::Store(format!("search pages {search} exceed the files {files}"))
        })?;
        Ok(Sizes {
            index,
            search,
            originals: folder_size(&self.dir.join(CONTENT))?,
        })
    }
}

fn open(dir: &Path, steps: &[&str]) -> Result<Store> {
    let path = dir.join(DB);
    fs::create_dir_all(dir.join(CONTENT)).map_err(|err| disk("making the store", err))?;
    let mut conn = Connection::open(&path)?;
    let version: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    let version = usize::try_from(version)
        .map_err(|_| Error::Store(format!("schema version {version} is negative")))?;
    let target = steps.len();
    let refused = if version > target {
        Some(format!(
            "written by a newer ElyMail (schema v{version}, this one reads v{target})"
        ))
    } else {
        // Parked originals return before the upgrade reads them.
        if version > 0 {
            aside::recover(&conn, dir)?;
        }
        (version < target)
            .then(|| upgrade(&mut conn, dir, version, steps).err())
            .flatten()
    };
    if let Some(reason) = refused {
        log::warn!("mail: store opened read-only: {reason}");
        drop(conn);
        let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX;
        let conn = Connection::open_with_flags(&path, flags)?;
        return Ok(Store {
            conn,
            dir: dir.to_owned(),
            read_only: Some(reason),
        });
    }
    conn.pragma_update(None, "foreign_keys", true)?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    if version == 0 {
        aside::recover(&conn, dir)?;
    }
    log::info!("mail: store opened at schema v{target}");
    Ok(Store {
        conn,
        dir: dir.to_owned(),
        read_only: None,
    })
}

/// A backup comes first; a failure keeps the old data.
fn upgrade(
    conn: &mut Connection,
    dir: &Path,
    version: usize,
    steps: &[&str],
) -> std::result::Result<(), String> {
    if version > 0 {
        backup(conn, dir, version).map_err(|err| format!("no backup before upgrade: {err}"))?;
    }
    migrate(conn, dir, version, steps).map_err(|err| {
        format!(
            "upgrade from schema v{version} to v{} failed: {err}",
            steps.len()
        )
    })?;
    log::info!("mail: store schema v{version} upgraded to v{}", steps.len());
    Ok(())
}

fn migrate(conn: &mut Connection, dir: &Path, version: usize, steps: &[&str]) -> Result<()> {
    // A rebuilt parent would cascade into its children.
    conn.pragma_update(None, "foreign_keys", false)?;
    let tx = conn.transaction()?;
    for step in &steps[version..] {
        tx.execute_batch(step)?;
    }
    if version < schema::IDENTS && steps.len() >= schema::IDENTS {
        let count = rebuilt::fill(&tx, dir)?;
        log::info!("mail: {count} messages given their identity in the upgrade");
    }
    if version < schema::REINDEX && steps.len() >= schema::REINDEX {
        let count = search::refill(&tx, dir)?;
        log::info!("mail: {count} messages indexed again in the upgrade");
    }
    if tx.prepare("PRAGMA foreign_key_check")?.exists([])? {
        return Err(Error::Store("rows point at missing rows".into()));
    }
    tx.pragma_update(None, "user_version", steps.len() as i64)?;
    Ok(tx.commit()?)
}

fn backup(conn: &Connection, dir: &Path, version: usize) -> Result<()> {
    let folder = dir.join(BACKUPS);
    fs::create_dir_all(&folder).map_err(|err| disk("making the backup folder", err))?;
    let now = now()?;
    let target = folder.join(format!("mail-v{version}-{now}.db"));
    let target = target
        .to_str()
        .ok_or_else(|| Error::Store("the backup path is not UTF-8".into()))?;
    conn.execute("VACUUM INTO ?1", [target])?;
    log::info!("mail: store schema v{version} backed up before upgrade");
    Ok(())
}

/// Seconds since 1970.
fn now() -> Result<i64> {
    let since = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| Error::Store(format!("the clock is before 1970: {err}")))?;
    Ok(since.as_secs() as i64)
}

/// Two levels: `content/ab/abcd…`.
fn folder_size(folder: &Path) -> Result<u64> {
    let mut total = 0;
    for shard in fs::read_dir(folder).map_err(|err| disk("reading the originals", err))? {
        let shard = shard.map_err(|err| disk("reading the originals", err))?;
        for file in fs::read_dir(shard.path()).map_err(|err| disk("reading the originals", err))? {
            let meta = file
                .and_then(|file| file.metadata())
                .map_err(|err| disk("reading the originals", err))?;
            total += meta.len();
        }
    }
    Ok(total)
}

fn disk(step: &str, err: io::Error) -> Error {
    Error::Store(format!("{step}: {err}"))
}

/// RFC 3501 §5.1: INBOX in any case is INBOX.
fn mailbox_name(name: &str) -> &str {
    match name.eq_ignore_ascii_case("INBOX") {
        true => "INBOX",
        false => name,
    }
}
