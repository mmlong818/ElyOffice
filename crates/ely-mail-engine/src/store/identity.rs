//! How an account signs what it sends (C12.12).

use rusqlite::{OptionalExtension, params};

use super::{AccountId, Store};
use crate::{Error, Result};

/// A sender name and a signature; empty for none.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Identity {
    pub name: Option<String>,
    pub signature: String,
}

impl Store {
    /// An account never given one has neither.
    pub fn identity(&self, account: AccountId) -> Result<Identity> {
        let row = self
            .conn
            .query_row(
                "SELECT name, signature FROM identities WHERE account = ?1",
                [account.0],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        Ok(match row {
            Some((name, signature)) => Identity { name, signature },
            None => Identity::default(),
        })
    }

    pub fn set_identity(&self, account: AccountId, identity: &Identity) -> Result<()> {
        self.writable()?;
        let name = identity.name.as_deref().map(str::trim);
        if name.is_some_and(|name| name.contains(['\r', '\n'])) {
            return Err(Error::Invalid("a sender name has a line break".into()));
        }
        let name = name.filter(|name| !name.is_empty());
        self.conn.execute(
            "INSERT INTO identities (account, name, signature) VALUES (?1, ?2, ?3)
             ON CONFLICT (account) DO UPDATE SET name = ?2, signature = ?3",
            params![account.0, name, identity.signature],
        )?;
        log::info!("mail: account {} identity saved", account.0);
        Ok(())
    }
}
