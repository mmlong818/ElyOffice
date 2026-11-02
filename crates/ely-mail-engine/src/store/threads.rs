//! Conversations by Message-ID, In-Reply-To and References (§12.3).

use std::collections::{BTreeSet, HashMap, HashSet};

use rusqlite::{OptionalExtension, Transaction, params};

use super::{MessageId, Store, Stored};
use crate::{Error, Mail, Result};

/// Each id a message is or names, once.
pub(super) fn link(tx: &Transaction, id: MessageId, mail: &Mail) -> Result<()> {
    tx.execute("DELETE FROM links WHERE message = ?1", [id.0])?;
    let named = mail.message_id.iter().chain(&mail.in_reply_to);
    for each in named.chain(&mail.references) {
        if !each.is_empty() {
            tx.execute(
                "INSERT OR IGNORE INTO links (message, id) VALUES (?1, ?2)",
                params![id.0, each],
            )?;
        }
    }
    Ok(())
}

// Spam may name a real Message-ID; it joins no conversation.
const SHARING: &str = "SELECT l.message FROM links l
     JOIN messages m ON m.id = l.message JOIN mailboxes b ON b.id = coalesce(m.moving, m.mailbox)
     WHERE l.id = ?1 AND b.account = ?2 AND NOT m.erasing AND coalesce(b.role, '') NOT IN ('junk', 'trash')";

impl Store {
    /// Linked messages in `id`'s account, oldest first, each Message-ID once.
    pub fn thread(&self, id: MessageId) -> Result<Vec<Stored>> {
        let account: i64 = self
            .conn
            .query_row(
                "SELECT b.account FROM messages m JOIN mailboxes b ON b.id = m.mailbox
                 WHERE m.id = ?1",
                [id.0],
                |row| row.get(0),
            )
            .optional()?
            .ok_or_else(|| Error::Store(format!("no message {}", id.0)))?;
        let mut sharing = self.conn.prepare(SHARING)?;
        let mut naming = self
            .conn
            .prepare("SELECT id FROM links WHERE message = ?1")?;
        let mut members = BTreeSet::from([id.0]);
        let mut walked = HashSet::new();
        let mut next = vec![id.0];
        while let Some(message) = next.pop() {
            let ids: Vec<String> = naming
                .query_map([message], |row| row.get(0))?
                .collect::<rusqlite::Result<_>>()?;
            for named in ids {
                if !walked.insert(named.clone()) {
                    continue;
                }
                for found in sharing.query_map(params![named, account], |row| row.get(0))? {
                    let found: i64 = found?;
                    if members.insert(found) {
                        next.push(found);
                    }
                }
            }
        }
        let stored = (members.into_iter())
            .map(|member| self.get(MessageId(member)))
            .collect::<Result<Vec<_>>>()?;
        let mut thread = once_each(stored, id);
        thread.sort_by_key(|each| (each.date, each.id.0));
        log::info!(
            "mail: message {} is one of {} in its thread",
            id.0,
            thread.len()
        );
        Ok(thread)
    }
}

/// Keeps `picked`, else a copy with its body.
fn once_each(stored: Vec<Stored>, picked: MessageId) -> Vec<Stored> {
    let mut kept: Vec<Stored> = Vec::new();
    let mut at: HashMap<String, usize> = HashMap::new();
    for each in stored {
        let Some(message_id) = each.message_id.clone() else {
            kept.push(each);
            continue;
        };
        match at.get(&message_id) {
            None => {
                at.insert(message_id, kept.len());
                kept.push(each);
            }
            Some(&ix) => {
                let held = &kept[ix];
                let better = held.id != picked && (each.id == picked || each.body && !held.body);
                if better {
                    kept[ix] = each;
                }
            }
        }
    }
    kept
}
