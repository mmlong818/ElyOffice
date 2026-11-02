//! Originals set aside until their transaction commits.

use std::{
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
};

use rusqlite::Connection;

use super::{CONTENT, disk};
use crate::{Error, Result};

const MARK: &str = ".gone";

fn original(dir: &Path, hash: &str) -> PathBuf {
    dir.join(CONTENT).join(&hash[..2]).join(hash)
}

fn parked(dir: &Path, hash: &str) -> PathBuf {
    dir.join(CONTENT)
        .join(&hash[..2])
        .join(format!(".{hash}{MARK}"))
}

/// Renamed, not deleted, so a rollback can put them back.
pub(super) struct Aside {
    dir: PathBuf,
    moved: Vec<String>,
}

impl Aside {
    pub fn new(dir: &Path) -> Self {
        Self {
            dir: dir.to_owned(),
            moved: Vec::new(),
        }
    }

    /// Once no row names the hash; runs before commit.
    pub fn drop_unused(&mut self, conn: &Connection, hash: &str) -> Result<()> {
        let used: bool = conn.query_row(
            "SELECT EXISTS (SELECT 1 FROM messages WHERE content = ?1)",
            [hash],
            |row| row.get(0),
        )?;
        if used {
            return Ok(());
        }
        match fs::rename(original(&self.dir, hash), parked(&self.dir, hash)) {
            Ok(()) => {
                self.moved.push(hash.to_owned());
                Ok(())
            }
            Err(err) if err.kind() == ErrorKind::NotFound => {
                log::warn!("mail: original {hash} was already gone");
                Ok(())
            }
            Err(err) => Err(disk("deleting an original", err)),
        }
    }

    /// Deletes on success; puts every file back on failure.
    pub fn settle<T>(self, done: Result<T>) -> Result<T> {
        match done {
            Ok(value) => {
                for hash in &self.moved {
                    if let Err(err) = fs::remove_file(parked(&self.dir, hash)) {
                        log::error!("mail: original {hash} stays set aside: {err}");
                    }
                }
                Ok(value)
            }
            Err(err) => {
                let mut stuck = Vec::new();
                for hash in self.moved.iter().rev() {
                    if let Err(back) =
                        fs::rename(parked(&self.dir, hash), original(&self.dir, hash))
                    {
                        stuck.push(format!("{hash}: {back}"));
                    }
                }
                if stuck.is_empty() {
                    Err(err)
                } else {
                    Err(Error::Store(format!(
                        "{err}; originals not put back (restored at next start): {}",
                        stuck.join("; ")
                    )))
                }
            }
        }
    }
}

/// After a crash: back if named, deleted if not.
pub(super) fn recover(conn: &Connection, dir: &Path) -> Result<()> {
    let mut back = 0;
    let mut gone = 0;
    let shards =
        fs::read_dir(dir.join(CONTENT)).map_err(|err| disk("reading the originals", err))?;
    for shard in shards {
        let shard = shard.map_err(|err| disk("reading the originals", err))?;
        let files = fs::read_dir(shard.path()).map_err(|err| disk("reading the originals", err))?;
        for file in files {
            let name = file
                .map_err(|err| disk("reading the originals", err))?
                .file_name();
            let Some(hash) = name
                .to_str()
                .and_then(|name| name.strip_prefix('.'))
                .and_then(|name| name.strip_suffix(MARK))
                .filter(|hash| hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit()))
            else {
                continue;
            };
            let named: bool = conn.query_row(
                "SELECT EXISTS (SELECT 1 FROM messages WHERE content = ?1)",
                [hash],
                |row| row.get(0),
            )?;
            if named {
                fs::rename(parked(dir, hash), original(dir, hash))
                    .map_err(|err| disk("restoring an original", err))?;
                back += 1;
            } else {
                fs::remove_file(parked(dir, hash))
                    .map_err(|err| disk("deleting an original", err))?;
                gone += 1;
            }
        }
    }
    if back + gone > 0 {
        log::warn!(
            "mail: {back} originals restored and {gone} deleted after an interrupted change"
        );
    }
    Ok(())
}
