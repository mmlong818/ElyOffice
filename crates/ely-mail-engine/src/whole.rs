//! One message's left-out parts, fetched on demand (MAIL-10).

use std::sync::Mutex;

use crate::{
    Account, AccountId, Error, Fetched, Imap, Key, MessageId, Result, Secret, Store, TlsConnector,
    drafts, sync::lock,
};

/// Brings `message` down whole, attachments too, and keeps it.
pub async fn whole(
    account: &Account,
    id: AccountId,
    secret: Secret,
    tls: &TlsConnector,
    store: &Mutex<Store>,
    message: MessageId,
) -> Result<()> {
    let (stored, folder) = {
        let store = lock(store)?;
        let stored = store.get(message)?;
        let folders = store.folders(id)?;
        let folder = folders
            .into_iter()
            .find(|folder| folder.id == stored.mailbox)
            .ok_or_else(|| Error::Store(format!("message {} is not this account's", message.0)))?;
        (stored, folder.name)
    };
    let Key::Imap { uidvalidity, uid } = stored.key else {
        return Err(Error::Store("POP3 keeps every message whole".into()));
    };
    let mut imap = Imap::connect(&account.incoming, &account.login(secret), tls).await?;
    let opened = imap.examine(&folder).await?;
    if opened.uidvalidity != uidvalidity {
        return Err(Error::Server(format!("{folder} changed on the server; sync, then try again")));
    }
    let got = imap.bodies(&[uid]).await?.pop();
    drafts::leave(imap);
    let got = got.ok_or_else(|| Error::Server(format!("the message is no longer in {folder}")))?;
    let flags: Vec<&str> = got.flags.iter().map(String::as_str).collect();
    let mut store = lock(store)?;
    // A sync may have removed or rebuilt it meanwhile.
    if store.find(stored.mailbox, &stored.key)? != Some(message) {
        log::info!("mail: message {} went while it came down; not kept", message.0);
        return Err(Error::Deleted);
    }
    store.put(stored.mailbox, &stored.key, &flags, Fetched::Whole(&got.bytes))?;
    log::info!("mail: message {} came down whole, {} bytes", message.0, got.bytes.len());
    Ok(())
}
