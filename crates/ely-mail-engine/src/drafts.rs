//! A draft's copy on the server, kept current (MAIL-06).

use crate::{Account, Error, Imap, Protocol, Remote, Result, Secret, TlsConnector};

/// The new copy, and why the old one stayed.
#[derive(Debug, PartialEq, Eq)]
pub struct Placed {
    pub remote: Remote,
    pub left: Option<String>,
}

/// Saves the new copy first, then removes `old`.
pub async fn put(
    account: &Account,
    secret: Secret,
    tls: &TlsConnector,
    mailbox: &str,
    message: &[u8],
    old: Option<&Remote>,
) -> Result<Placed> {
    let mut imap = connect(account, secret, tls).await?;
    let remote = imap.put_draft(mailbox, message).await?;
    let left = match old {
        None => None,
        Some(old) => match imap.drop_draft(old).await {
            Ok(true) => None,
            Ok(false) => Some("the drafts folder was renumbered".to_owned()),
            Err(err) => Some(err.to_string()),
        },
    };
    leave(imap);
    Ok(Placed { remote, left })
}

/// False when the folder was renumbered, so it stays.
pub async fn take(
    account: &Account,
    secret: Secret,
    tls: &TlsConnector,
    remote: &Remote,
) -> Result<bool> {
    let mut imap = connect(account, secret, tls).await?;
    let gone = imap.drop_draft(remote).await?;
    leave(imap);
    Ok(gone)
}

pub(crate) async fn connect(account: &Account, secret: Secret, tls: &TlsConnector) -> Result<Imap> {
    if account.protocol != Protocol::Imap {
        return Err(Error::Invalid(
            "a POP3 account has no server folders".into(),
        ));
    }
    Imap::connect(&account.incoming, &account.login(secret), tls).await
}

/// Done already: the logout neither waits nor counts.
pub(crate) fn leave(imap: Imap) {
    tokio::spawn(async move {
        if let Err(err) = imap.logout().await {
            log::warn!("mail: IMAP logout after placing a copy failed: {err}");
        }
    });
}
