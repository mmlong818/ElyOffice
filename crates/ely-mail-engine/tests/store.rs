use std::path::Path;

use ely_mail_engine::{Error, Fetched, Key, MailboxId, MessageId, Store};
use rusqlite::Connection;
use sha2::{Digest, Sha256};

/// The sample as a server sends it: CRLF lines.
fn sample() -> Vec<u8> {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/mail/sample.eml"
    );
    let text = std::fs::read_to_string(path).unwrap();
    text.replace("\r\n", "\n")
        .replace('\n', "\r\n")
        .into_bytes()
}

fn headers(raw: &[u8]) -> &[u8] {
    let end = raw.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
    &raw[..end + 4]
}

fn inbox(dir: &Path) -> (Store, MailboxId) {
    let store = Store::open(dir).unwrap();
    let account = store
        .add_account(&fake::account("ann@example.com"))
        .unwrap();
    let inbox = store.mailbox(account, "INBOX").unwrap();
    (store, inbox)
}

fn imap(uidvalidity: u32, uid: u32) -> Key {
    Key::Imap { uidvalidity, uid }
}

/// Rows whose index holds the phrase.
fn found(dir: &Path, phrase: &str) -> Vec<i64> {
    let conn = Connection::open(dir.join("mail.db")).unwrap();
    let mut rows = conn
        .prepare("SELECT rowid FROM search WHERE search MATCH ?1 ORDER BY rowid")
        .unwrap();
    rows.query_map([format!("\"{phrase}\"")], |row| row.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect()
}

fn original_path(dir: &Path, raw: &[u8]) -> std::path::PathBuf {
    let hash: String = Sha256::digest(raw)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    dir.join("content").join(&hash[..2]).join(hash)
}

#[test]
fn a_whole_message_is_indexed_and_kept_by_its_hash() {
    let dir = tempfile::tempdir().unwrap();
    let (mut store, inbox) = inbox(dir.path());
    let raw = sample();
    let id = store
        .put(inbox, &imap(7, 42), &["\\Seen"], Fetched::Whole(&raw))
        .unwrap();
    let stored = store.get(id).unwrap();
    assert_eq!(stored.key, imap(7, 42));
    assert_eq!(stored.subject.as_deref(), Some("Café report"));
    assert_eq!(stored.from[0].email, "zhang@example.com");
    assert_eq!(stored.message_id.as_deref(), Some("m1@example.com"));
    assert_eq!(stored.flags, ["\\Seen"]);
    assert_eq!(stored.attachments, Some(1));
    assert!(stored.body);
    assert_eq!(store.original(id).unwrap().as_deref(), Some(raw.as_slice()));
    assert_eq!(std::fs::read(original_path(dir.path(), &raw)).unwrap(), raw);
    assert_eq!(found(dir.path(), "numbers"), [id.0]);
    assert_eq!(found(dir.path(), "report.pdf"), [id.0]);
    let sizes = store.sizes().unwrap();
    assert_eq!(sizes.originals, raw.len() as u64);
    assert!(sizes.search > 0 && sizes.index > 0, "{sizes:?}");
}

#[test]
fn headers_first_then_the_whole_message() {
    let dir = tempfile::tempdir().unwrap();
    let (mut store, inbox) = inbox(dir.path());
    let raw = sample();
    let key = imap(7, 42);
    let id = store
        .put(inbox, &key, &[], Fetched::Headers(headers(&raw)))
        .unwrap();
    let stored = store.get(id).unwrap();
    assert_eq!(stored.subject.as_deref(), Some("Café report"));
    assert_eq!((stored.body, stored.attachments), (false, None));
    assert_eq!(store.original(id).unwrap(), None);
    assert_eq!(store.head(id).unwrap(), headers(&raw));
    assert_eq!(found(dir.path(), "report"), [id.0]);
    assert!(found(dir.path(), "numbers").is_empty());

    let again = store
        .put(inbox, &key, &["\\Flagged"], Fetched::Headers(headers(&raw)))
        .unwrap();
    assert_eq!(again, id);
    assert_eq!(store.get(id).unwrap().flags, ["\\Flagged"]);

    assert_eq!(
        store.put(inbox, &key, &[], Fetched::Whole(&raw)).unwrap(),
        id
    );
    assert_eq!(store.get(id).unwrap().attachments, Some(1));
    assert_eq!(found(dir.path(), "numbers"), [id.0]);
    let gone = store.head(id).unwrap_err().to_string();
    assert!(gone.contains("kept no header block"), "{gone}");
}

#[test]
fn uidvalidity_is_part_of_the_identity() {
    let dir = tempfile::tempdir().unwrap();
    let (mut store, inbox) = inbox(dir.path());
    let raw = sample();
    let old = store
        .put(inbox, &imap(1, 5), &[], Fetched::Headers(headers(&raw)))
        .unwrap();
    let new = store
        .put(inbox, &imap(2, 5), &[], Fetched::Headers(headers(&raw)))
        .unwrap();
    assert_ne!(old, new);
    assert_eq!(store.find(inbox, &imap(1, 5)).unwrap(), Some(old));
    let pop = Key::Pop { uidl: "abc".into() };
    let id = store
        .put(inbox, &pop, &[], Fetched::Headers(headers(&raw)))
        .unwrap();
    assert_eq!(store.get(id).unwrap().key, pop);
}

#[test]
fn a_shared_original_goes_with_its_last_message() {
    let dir = tempfile::tempdir().unwrap();
    let (mut store, inbox) = inbox(dir.path());
    let raw = sample();
    let file = original_path(dir.path(), &raw);
    let one = store
        .put(inbox, &imap(1, 1), &[], Fetched::Whole(&raw))
        .unwrap();
    let two = store
        .put(inbox, &imap(1, 2), &[], Fetched::Whole(&raw))
        .unwrap();
    assert_eq!(store.sizes().unwrap().originals, raw.len() as u64);
    store.remove(one).unwrap();
    assert!(file.exists());
    assert_eq!(found(dir.path(), "numbers"), [two.0]);
    store.remove(two).unwrap();
    assert!(!file.exists());
    assert!(found(dir.path(), "numbers").is_empty());
    let err = store.remove(MessageId(two.0)).unwrap_err();
    assert!(
        matches!(&err, Error::Store(text) if text.contains("no message")),
        "{err}"
    );
}

#[test]
fn a_damaged_original_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let (mut store, inbox) = inbox(dir.path());
    let raw = sample();
    let id = store
        .put(inbox, &imap(1, 1), &[], Fetched::Whole(&raw))
        .unwrap();
    std::fs::write(original_path(dir.path(), &raw), b"not the message").unwrap();
    let err = store.original(id).unwrap_err();
    assert!(
        matches!(&err, Error::Store(text) if text.contains("damaged")),
        "{err}"
    );
}

#[test]
fn the_search_index_rebuilds_from_the_store() {
    let dir = tempfile::tempdir().unwrap();
    let (mut store, inbox) = inbox(dir.path());
    let whole = store
        .put(inbox, &imap(1, 1), &[], Fetched::Whole(&sample()))
        .unwrap();
    let html = b"Subject: news\r\nContent-Type: text/html\r\n\r\n\
        <style>.hidden{color:red}</style><p>Quarterly<b>results</b></p>\
        <script>trackingpixel()</script>";
    let page = store
        .put(inbox, &imap(1, 2), &[], Fetched::Whole(html))
        .unwrap();
    assert_eq!(found(dir.path(), "results"), [page.0]);
    assert!(found(dir.path(), "tracking").is_empty());
    assert!(found(dir.path(), "hidden").is_empty());
    assert!(found(dir.path(), "Quarterlyresults").is_empty());

    Connection::open(dir.path().join("mail.db"))
        .unwrap()
        .execute("DELETE FROM search", [])
        .unwrap();
    assert!(found(dir.path(), "numbers").is_empty());
    assert_eq!(store.rebuild_search().unwrap(), 2);
    assert_eq!(found(dir.path(), "numbers"), [whole.0]);
    assert_eq!(found(dir.path(), "zhang"), [whole.0]);
    assert_eq!(found(dir.path(), "results"), [page.0]);
}
