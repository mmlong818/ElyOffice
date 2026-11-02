//! Senders whose remote pictures load (§12.4).

use rusqlite::OptionalExtension;

use super::Store;
use crate::Result;

impl Store {
    /// Not distinguished by ASCII case.
    pub fn trusts(&self, address: &str) -> Result<bool> {
        let found = self
            .conn
            .query_row(
                "SELECT 1 FROM trusted WHERE address = ?1",
                [address],
                |_| Ok(()),
            )
            .optional()?;
        Ok(found.is_some())
    }

    pub fn set_trusted(&self, address: &str, trusted: bool) -> Result<()> {
        self.writable()?;
        if trusted {
            self.conn.execute(
                "INSERT OR IGNORE INTO trusted (address) VALUES (?1)",
                [address],
            )?;
        } else {
            self.conn
                .execute("DELETE FROM trusted WHERE address = ?1", [address])?;
        }
        // The address names a person: not logged.
        log::info!("mail: a sender's pictures load: {trusted}");
        Ok(())
    }
}
