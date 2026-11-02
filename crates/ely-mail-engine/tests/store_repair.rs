use std::path::Path;

use ely_mail_engine::{Error, Fetched, Key, MailboxId, MessageId, Store};
use rusqlite::Connection;
use sha2::{Digest, Sha256};

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

fn inbox(dir: &Path) -> (Store, MailboxId) {
    let store = Store::open(dir).unwrap();
    let account = store
        .add_account(&fake::account("ann@example.com"))
        .unwrap();
    let inbox = store.mailbox(account, "INBOX").unwrap();
    (store, inbox)
}

fn imap(uid: u32) -> Key {
    Key::Imap {
        uidvalidity: 1,
        uid,
    }
}

fn search(dir: &Path, phrase: &str) -> rusqlite::Result<Vec<i64>> {
    let conn = Connection::open(dir.join("mail.db"))?;
    let mut rows = conn.prepare("SELECT rowid FROM search WHERE search MATCH ?1")?;
    rows.query_map([format!("\"{phrase}\"")], |row| row.get(0))?
        .collect()
}

fn found(dir: &Path, phrase: &str) -> Vec<i64> {
    search(dir, phrase).unwrap()
}

fn original_path(dir: &Path, raw: &[u8]) -> std::path::PathBuf {
    let hash: String = Sha256::digest(raw)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    dir.join("content").join(&hash[..2]).join(hash)
}

#[test]
fn a_corrupt_search_index_rebuilds() {
    let dir = tempfile::tempdir().unwrap();
    let (mut store, inbox) = inbox(dir.path());
    let id = store
        .put(inbox, &imap(1), &[], Fetched::Whole(&sample()))
        .unwrap();
    let damaged = Connection::open(dir.path().join("mail.db"))
        .unwrap()
        .execute("UPDATE search_data SET block = X'00' WHERE id > 10", [])
        .unwrap();
    assert!(damaged > 0);
    assert!(search(dir.path(), "numbers").is_err());
    assert_eq!(store.rebuild_search().unwrap(), 1);
    assert_eq!(found(dir.path(), "numbers"), [id.0]);
    drop(store);
    Store::open(dir.path()).unwrap();
    assert_eq!(found(dir.path(), "numbers"), [id.0]);
}

#[test]
fn a_damaged_original_is_rewritten_by_a_new_copy() {
    let dir = tempfile::tempdir().unwrap();
    let (mut store, inbox) = inbox(dir.path());
    let raw = sample();
    let id = store
        .put(inbox, &imap(1), &[], Fetched::Whole(&raw))
        .unwrap();
    std::fs::write(original_path(dir.path(), &raw), b"damaged").unwrap();
    store
        .put(inbox, &imap(2), &[], Fetched::Whole(&raw))
        .unwrap();
    assert_eq!(store.original(id).unwrap().as_deref(), Some(raw.as_slice()));

    std::fs::write(original_path(dir.path(), &raw), b"damaged").unwrap();
    assert_eq!(
        store
            .put(inbox, &imap(1), &[], Fetched::Whole(&raw))
            .unwrap(),
        id
    );
    assert_eq!(store.original(id).unwrap().as_deref(), Some(raw.as_slice()));
}

#[test]
fn a_failed_write_keeps_no_new_original() {
    let dir = tempfile::tempdir().unwrap();
    let (mut store, inbox) = inbox(dir.path());
    let raw = sample();
    let nowhere = MailboxId(999);
    assert!(
        store
            .put(nowhere, &imap(1), &[], Fetched::Whole(&raw))
            .is_err()
    );
    assert_eq!(store.sizes().unwrap().originals, 0);

    let id = store
        .put(inbox, &imap(1), &[], Fetched::Whole(&raw))
        .unwrap();
    assert!(
        store
            .put(nowhere, &imap(2), &[], Fetched::Whole(&raw))
            .is_err()
    );
    assert_eq!(store.original(id).unwrap().as_deref(), Some(raw.as_slice()));
}

#[test]
fn sizes_count_the_write_ahead_log() {
    let dir = tempfile::tempdir().unwrap();
    let (mut store, inbox) = inbox(dir.path());
    let id = store
        .put(inbox, &imap(1), &[], Fetched::Whole(&sample()))
        .unwrap();
    for turn in 0..600 {
        let flag = if turn % 2 == 0 { "\\Seen" } else { "\\Flagged" };
        store.set_flags(id, &[flag]).unwrap();
    }
    let disk: u64 = ["mail.db", "mail.db-wal", "mail.db-shm"]
        .iter()
        .map(|name| std::fs::metadata(dir.path().join(name)).unwrap().len())
        .sum();
    let sizes = store.sizes().unwrap();
    assert!(disk > 1_000_000, "{disk}");
    assert_eq!(sizes.index + sizes.search, disk);
}

#[test]
fn flags_for_a_missing_message_fail() {
    let dir = tempfile::tempdir().unwrap();
    let (mut store, _) = inbox(dir.path());
    let err = store.set_flags(MessageId(5), &[]).unwrap_err();
    assert!(
        matches!(&err, Error::Store(text) if text.contains("no message")),
        "{err}"
    );
}

#[test]
fn html_text_follows_the_parser_states() {
    let dir = tempfile::tempdir().unwrap();
    let (mut store, inbox) = inbox(dir.path());
    let html = b"Subject: page\r\nContent-Type: text/html\r\n\r\n\
        <title>headline</title><script/>trackingpixel()</script>\
        <textarea><script>ordinarycontent</script></textarea>\
        <xmp><b>literalmarkup</b></xmp><iframe>fallbacktext</iframe>\
        <svg><style/>vectorlabel<script>vectorcode()</script></svg>\
        <svg><foreignObject><script/>trackingforeign()</script>\
        <textarea><script>ordinaryforeign</script></textarea></foreignObject></svg>\
        <math><annotation-xml encoding=\"text/html\"><script/>annotationcode()</script>\
        </annotation-xml></math><p>closingword</p>";
    let id = store
        .put(inbox, &imap(1), &[], Fetched::Whole(html))
        .unwrap();
    for round in 0..2 {
        for shown in [
            "ordinarycontent",
            "<b>literalmarkup",
            "vectorlabel",
            "<script>ordinaryforeign",
            "closingword",
        ] {
            assert_eq!(found(dir.path(), shown), [id.0], "{shown} {round}");
        }
        for hidden in [
            "headline",
            "trackingpixel",
            "fallbacktext",
            "vectorcode",
            "trackingforeign",
            "annotationcode",
        ] {
            assert!(found(dir.path(), hidden).is_empty(), "{hidden} {round}");
        }
        store.rebuild_search().unwrap();
    }
}

#[cfg(windows)]
#[test]
fn a_failed_publish_leaves_no_staged_copy() {
    let dir = tempfile::tempdir().unwrap();
    let (mut store, inbox) = inbox(dir.path());
    let raw = sample();
    let id = store
        .put(inbox, &imap(1), &[], Fetched::Whole(&raw))
        .unwrap();
    let path = original_path(dir.path(), &raw);
    std::fs::write(&path, b"damaged").unwrap();
    let mut locked = std::fs::metadata(&path).unwrap().permissions();
    locked.set_readonly(true);
    std::fs::set_permissions(&path, locked.clone()).unwrap();
    assert!(
        store
            .put(inbox, &imap(1), &[], Fetched::Whole(&raw))
            .is_err()
    );
    #[allow(clippy::permissions_set_readonly_false)]
    locked.set_readonly(false);
    std::fs::set_permissions(&path, locked).unwrap();
    store.remove(id).unwrap();
    assert_eq!(store.sizes().unwrap().originals, 0);
    let left: Vec<_> = std::fs::read_dir(path.parent().unwrap()).unwrap().collect();
    assert!(left.is_empty(), "{left:?}");
}

#[test]
fn deep_html_stops_at_its_depth_limit() {
    let dir = tempfile::tempdir().unwrap();
    let (mut store, inbox) = inbox(dir.path());
    let mut html = b"Subject: deep
Content-Type: text/html

"
    .to_vec();
    html.extend("<div>".repeat(500).bytes());
    html.extend(b"shallowword");
    html.extend("<div>".repeat(200_000).bytes());
    html.extend(b"bottomword");
    let started = std::time::Instant::now();
    let id = store
        .put(inbox, &imap(1), &[], Fetched::Whole(&html))
        .unwrap();
    assert!(started.elapsed().as_secs() < 5, "{:?}", started.elapsed());
    assert_eq!(found(dir.path(), "shallowword"), [id.0]);
    assert!(found(dir.path(), "bottomword").is_empty());
}
