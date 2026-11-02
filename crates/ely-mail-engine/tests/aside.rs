use std::{fs, path::Path};

use ely_mail_engine::{Fetched, Key, Store};
use fake::account;
use sha2::{Digest, Sha256};

fn hash(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn parked(dir: &Path, hash: &str) -> std::path::PathBuf {
    dir.join("content")
        .join(&hash[..2])
        .join(format!(".{hash}.gone"))
}

fn key(uid: u32) -> Key {
    Key::Imap {
        uidvalidity: 1,
        uid,
    }
}

#[test]
fn a_removal_that_fails_midway_keeps_every_original() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = Store::open(dir.path()).unwrap();
    let ann = store.add_account(&account("ann@example.com")).unwrap();
    let inbox = store.mailbox(ann, "INBOX").unwrap();
    let one: &[u8] = b"Subject: One\r\n\r\nfirst\r\n";
    let two: &[u8] = b"Subject: Two\r\n\r\nsecond\r\n";
    let a = store.put(inbox, &key(1), &[], Fetched::Whole(one)).unwrap();
    let b = store.put(inbox, &key(2), &[], Fetched::Whole(two)).unwrap();
    let last = hash(one).max(hash(two));
    fs::create_dir(parked(dir.path(), &last)).unwrap();
    store.remove_account(ann).unwrap_err();
    assert_eq!(store.accounts().unwrap().len(), 1);
    assert_eq!(store.original(a).unwrap().unwrap(), one);
    assert_eq!(store.original(b).unwrap().unwrap(), two);
    fs::remove_dir(parked(dir.path(), &last)).unwrap();
    store.remove_account(ann).unwrap();
    assert_eq!(store.sizes().unwrap().originals, 0);
}

#[test]
fn a_restart_puts_back_what_a_crash_left_aside() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = Store::open(dir.path()).unwrap();
    let ann = store.add_account(&account("ann@example.com")).unwrap();
    let inbox = store.mailbox(ann, "INBOX").unwrap();
    let kept: &[u8] = b"Subject: Kept\r\n\r\nstill named\r\n";
    let id = store
        .put(inbox, &key(1), &[], Fetched::Whole(kept))
        .unwrap();
    drop(store);
    let named = hash(kept);
    let original = dir.path().join("content").join(&named[..2]).join(&named);
    fs::rename(&original, parked(dir.path(), &named)).unwrap();
    let orphan = hash(b"nobody names this");
    let shard = dir.path().join("content").join(&orphan[..2]);
    fs::create_dir_all(&shard).unwrap();
    fs::write(parked(dir.path(), &orphan), b"nobody names this").unwrap();
    let store = Store::open(dir.path()).unwrap();
    assert_eq!(store.original(id).unwrap().unwrap(), kept);
    assert!(!parked(dir.path(), &named).exists());
    assert!(!parked(dir.path(), &orphan).exists());
    assert_eq!(store.sizes().unwrap().originals, kept.len() as u64);
}
