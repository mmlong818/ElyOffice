#![cfg(windows)]


use std::{fs::File, os::windows::fs::OpenOptionsExt, path::Path};

use ely_mail_engine::{Fetched, Key, MailboxId, Store};
use sha2::{Digest, Sha256};

const ONE: &[u8] = b"Subject: one\r\n\r\nfirst body\r\n";
const TWO: &[u8] = b"Subject: two\r\n\r\nsecond body\r\n";

fn inbox(dir: &Path) -> (Store, MailboxId) {
    let store = Store::open(dir).unwrap();
    let account = store
        .add_account(&fake::account("ann@example.com"))
        .unwrap();
    let inbox = store.mailbox(account, "INBOX").unwrap();
    (store, inbox)
}

fn imap() -> Key {
    Key::Imap {
        uidvalidity: 1,
        uid: 1,
    }
}

fn original_path(dir: &Path, raw: &[u8]) -> std::path::PathBuf {
    let hash: String = Sha256::digest(raw)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    dir.join("content").join(&hash[..2]).join(hash)
}

/// Read sharing only: delete fails.
fn hold(path: &Path) -> File {
    File::options().read(true).share_mode(1).open(path).unwrap()
}

#[test]
fn a_held_original_keeps_its_message_for_retry() {
    let dir = tempfile::tempdir().unwrap();
    let (mut store, inbox) = inbox(dir.path());
    let id = store.put(inbox, &imap(), &[], Fetched::Whole(ONE)).unwrap();
    let path = original_path(dir.path(), ONE);
    let held = hold(&path);
    assert!(store.remove(id).is_err());
    assert_eq!(store.get(id).unwrap().subject.as_deref(), Some("one"));
    assert_eq!(store.original(id).unwrap().as_deref(), Some(ONE));
    drop(held);
    store.remove(id).unwrap();
    assert!(store.get(id).is_err());
    assert!(!path.exists());
    assert_eq!(store.sizes().unwrap().originals, 0);
}

#[test]
fn a_held_old_original_keeps_the_old_message() {
    let dir = tempfile::tempdir().unwrap();
    let (mut store, inbox) = inbox(dir.path());
    let id = store.put(inbox, &imap(), &[], Fetched::Whole(ONE)).unwrap();
    let old = original_path(dir.path(), ONE);
    let new = original_path(dir.path(), TWO);
    let held = hold(&old);
    assert!(store.put(inbox, &imap(), &[], Fetched::Whole(TWO)).is_err());
    assert_eq!(store.get(id).unwrap().subject.as_deref(), Some("one"));
    assert_eq!(store.original(id).unwrap().as_deref(), Some(ONE));
    assert!(!new.exists());
    drop(held);
    store.put(inbox, &imap(), &[], Fetched::Whole(TWO)).unwrap();
    assert_eq!(store.original(id).unwrap().as_deref(), Some(TWO));
    assert!(!old.exists());
    assert_eq!(store.sizes().unwrap().originals, TWO.len() as u64);
}
