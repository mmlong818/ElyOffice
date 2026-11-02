//! A sent message's copy in the Sent folder (C12.14).

use crate::{
    Account, Result, Secret, TlsConnector,
    drafts::{connect, leave},
};

/// Places `message` in `mailbox`, marked read.
pub async fn file(
    account: &Account,
    secret: Secret,
    tls: &TlsConnector,
    mailbox: &str,
    message: &[u8],
) -> Result<()> {
    let mut imap = connect(account, secret, tls).await?;
    imap.put_sent(mailbox, message).await?;
    leave(imap);
    Ok(())
}
