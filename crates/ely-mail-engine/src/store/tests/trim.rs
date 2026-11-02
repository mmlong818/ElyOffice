use super::account;
use crate::store::{Fetched, Key, Store};
use crate::Role;

/// A message dated `day` of January 2024 with `body`.
fn dated(day: u32, body: &str) -> Vec<u8> {
    format!("From: cy@example.com\r\nSubject: day {day}\r\nDate: {day} Jan 2024 10:00:00 +0000\r\n\r\n{body}\r\n")
        .into_bytes()
}

#[test]
fn downloads_trim_oldest_to_the_cap_and_stay_trimmed() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = Store::open(dir.path()).unwrap();
    let ann = store.add_account(&account("ann@example.com")).unwrap();
    let [inbox, kept] = store
        .set_folders(ann, &[("INBOX", Some(Role::Inbox)), ("Kept", None)])
        .unwrap()
        .try_into()
        .unwrap();
    store.set_offline(kept, true).unwrap();
    store.open_mailbox(inbox, 1).unwrap();
    let put = |store: &mut Store, mailbox, uid, bytes: &[u8]| {
        store.put(mailbox, &Key::Imap { uidvalidity: 1, uid }, &[], Fetched::Whole(bytes)).unwrap()
    };
    let (old, mid, new) = (dated(1, &"a".repeat(400)), dated(2, &"b".repeat(400)), dated(3, &"c".repeat(400)));
    // Stored out of date order: age, not arrival, decides.
    let mid_id = put(&mut store, inbox, 1, &mid);
    let old_id = put(&mut store, inbox, 2, &old);
    let new_id = put(&mut store, inbox, 3, &new);
    let offline = put(&mut store, kept, 1, &dated(1, &"d".repeat(4000)));
    let cap = (mid.len() + new.len()) as u64;
    assert_eq!(store.trim_downloads(cap, || true).unwrap(), 1);
    assert_eq!(store.original(old_id).unwrap(), None);
    assert!(store.original(mid_id).unwrap().is_some() && store.original(new_id).unwrap().is_some());
    assert!(store.original(offline).unwrap().is_some(), "an offline folder is never trimmed");
    let held = store.open_mailbox(inbox, 1).unwrap();
    assert!(held[&2].trimmed && !held[&2].body, "trimmed stays trimmed through syncs");
    assert!(!held[&1].trimmed);
    // Opened again, it comes down whole and counts as fresh.
    put(&mut store, inbox, 2, &old);
    assert!(!store.open_mailbox(inbox, 1).unwrap()[&2].trimmed);
    assert_eq!(store.forget_downloads().unwrap(), 3);
    let held = store.open_mailbox(inbox, 1).unwrap();
    assert!(held.values().all(|each| !each.body && !each.trimmed), "a clean-up lets syncs bring bodies back");
    for (uid, bytes) in [(1, &mid), (2, &old), (3, &new)] {
        put(&mut store, inbox, uid, bytes);
    }
    assert_eq!(store.trim_downloads(0, || true).unwrap(), 3, "a zero cap leaves only offline mail");
    assert!(store.original(offline).unwrap().is_some());
}

#[test]
fn a_file_an_offline_copy_holds_is_no_spare_room() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = Store::open(dir.path()).unwrap();
    let ann = store.add_account(&account("ann@example.com")).unwrap();
    let [inbox, kept] = store
        .set_folders(ann, &[("INBOX", Some(Role::Inbox)), ("Kept", None)])
        .unwrap()
        .try_into()
        .unwrap();
    store.set_offline(kept, true).unwrap();
    let put = |store: &mut Store, mailbox, uid, bytes: &[u8]| {
        store.put(mailbox, &Key::Imap { uidvalidity: 1, uid }, &[], Fetched::Whole(bytes)).unwrap()
    };
    let (shared, new) = (dated(1, &"a".repeat(4000)), dated(2, &"b".repeat(400)));
    let old = put(&mut store, inbox, 1, &shared);
    put(&mut store, kept, 1, &shared);
    let new_id = put(&mut store, inbox, 2, &new);
    // Only the new body is spare room, and it fits.
    assert_eq!(store.trim_downloads(new.len() as u64, || true).unwrap(), 0);
    assert!(store.original(new_id).unwrap().is_some(), "the cap holds it");
    assert!(store.original(old).unwrap().is_some(), "its file stays for the offline copy anyway");
    assert_eq!(store.trim_downloads(0, || true).unwrap(), 1, "trimming the held one frees nothing");
    assert!(store.original(old).unwrap().is_some() && store.original(new_id).unwrap().is_none());
}

#[test]
fn a_newer_cap_stops_a_trim_already_running() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = Store::open(dir.path()).unwrap();
    let ann = store.add_account(&account("ann@example.com")).unwrap();
    let [inbox] = store.set_folders(ann, &[("INBOX", Some(Role::Inbox))]).unwrap().try_into().unwrap();
    store.open_mailbox(inbox, 1).unwrap();
    let ids: Vec<_> = (1..=3)
        .map(|day| {
            let key = Key::Imap { uidvalidity: 1, uid: day };
            store.put(inbox, &key, &[], Fetched::Whole(&dated(day, "body"))).unwrap()
        })
        .collect();
    // The cap changes once the first message is trimmed.
    let asked = std::cell::Cell::new(0);
    let still = || {
        asked.set(asked.get() + 1);
        asked.get() == 1
    };
    assert_eq!(store.trim_downloads(0, still).unwrap(), 1);
    assert!(store.original(ids[0]).unwrap().is_none());
    assert!(store.original(ids[1]).unwrap().is_some() && store.original(ids[2]).unwrap().is_some());
}
