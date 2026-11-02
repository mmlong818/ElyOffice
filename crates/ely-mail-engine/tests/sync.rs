use std::sync::Mutex;

use ely_mail_engine::{
    Account, AccountId, Error, Key, MailboxId, Role, Secret, Server, Store, sync,
};
use fake::{account, certs, expect, send, serve};
use tokio::task::JoinHandle;

/// 2026-10-19 12:00 UTC; bodies from 21 July.
const NOW: i64 = 1_792_411_200;
const SINCE: &str = "{tag} UID SEARCH SINCE 21-Jul-2026";

const OLD: &str = "Subject: Old\nFrom: a@example.com\n\n";
const NEW: &str = "Subject: New\nFrom: b@example.com\n\nfresh\n";

/// A FETCH line carrying `text` as a literal.
fn literal(uid: u32, flags: &str, section: &str, text: &str) -> String {
    let crlf = text.replace('\n', "\r\n");
    format!(
        "* {uid} FETCH (UID {uid} FLAGS ({flags}) {section} {{{}}}\n{text})\n",
        crlf.len()
    )
}

/// A one-part text message's structure, and the tag.
fn plain(uid: u32) -> String {
    format!(
        "* {uid} FETCH (UID {uid} BODYSTRUCTURE (\"TEXT\" \"PLAIN\" NIL NIL NIL \"7BIT\" 7 1))\n\
         {{tag}} OK structure\n"
    )
}

async fn server(steps: Vec<fake::Step>) -> (Account, JoinHandle<Result<(), String>>, fake::Certs) {
    let certs = certs();
    let mut script = vec![
        send("* OK ready\n"),
        expect("{tag} LOGIN \"ann@example.com\" \"pw\""),
        send("{tag} OK done\n"),
    ];
    script.extend(steps);
    script.push(expect("{tag} LOGOUT"));
    script.push(send("* BYE\n{tag} OK bye\n"));
    let (incoming, task) = serve(&certs, ely_mail_engine::Security::Tls, script).await;
    let account = Account {
        incoming: Server { ..incoming },
        ..account("ann@example.com")
    };
    (account, task, certs)
}

async fn run(
    store: &Mutex<Store>,
    id: AccountId,
    steps: Vec<fake::Step>,
) -> Result<sync::Report, Error> {
    let (account, task, certs) = server(steps).await;
    let secret = Secret::Password("pw".into());
    let done = sync::imap(&account, id, secret, &certs.trusted, store, NOW, &|| {}).await;
    if done.is_ok() {
        task.await.unwrap().unwrap();
    }
    done
}

fn first_sync() -> Vec<fake::Step> {
    vec![
        expect("{tag} CAPABILITY"),
        send("* CAPABILITY IMAP4rev1\n{tag} OK caps\n"),
        expect("{tag} LIST \"\" *"),
        send("* LIST (\\HasNoChildren) \"/\" INBOX\n"),
        send("* LIST (\\HasNoChildren \\Sent) \"/\" Sent\n"),
        send("* LIST (\\Noselect) \"/\" Archive\n"),
        send("{tag} OK listed\n"),
        expect("{tag} EXAMINE \"INBOX\""),
        send("* 2 EXISTS\n* OK [UIDVALIDITY 7] v\n{tag} OK in\n"),
        expect("{tag} UID FETCH 1:* (UID FLAGS)"),
        send("* 1 FETCH (UID 1 FLAGS (\\Seen))\n* 2 FETCH (UID 2 FLAGS (\\Recent))\n"),
        send("{tag} OK flags\n"),
        expect(SINCE),
        send("* SEARCH 2\n{tag} OK found\n"),
        expect("{tag} UID FETCH 1 (UID FLAGS BODY.PEEK[HEADER])"),
        send(&literal(1, "\\Seen", "BODY[HEADER]", OLD)),
        send("{tag} OK headers\n"),
        expect("{tag} UID FETCH 2 (UID BODYSTRUCTURE)"),
        send(&plain(2)),
        expect("{tag} UID FETCH 2 (UID FLAGS BODY.PEEK[])"),
        send(&literal(2, "\\Recent", "BODY[]", NEW)),
        send("{tag} OK bodies\n"),
        expect("{tag} EXAMINE \"Sent\""),
        send("* 0 EXISTS\n* OK [UIDVALIDITY 3] v\n{tag} OK in\n"),
    ]
}

fn key(uidvalidity: u32, uid: u32) -> Key {
    Key::Imap { uidvalidity, uid }
}

fn inbox(store: &Store, id: AccountId) -> MailboxId {
    store.folders(id).unwrap()[0].id
}

fn opened() -> (tempfile::TempDir, Mutex<Store>, AccountId) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).unwrap();
    let id = store.add_account(&account("ann@example.com")).unwrap();
    (dir, Mutex::new(store), id)
}

#[tokio::test]
async fn a_first_sync_takes_headers_and_recent_bodies() {
    let (_dir, store, id) = opened();
    let report = run(&store, id, first_sync()).await.unwrap();
    assert_eq!(
        report,
        sync::Report {
            folders: 2,
            headers: 1,
            bodies: 1,
            removed: 0,
            deleted: 0,
            pushed: Default::default(),
        }
    );
    let store = store.lock().unwrap();
    let folders: Vec<_> = store
        .folders(id)
        .unwrap()
        .into_iter()
        .map(|folder| (folder.name, folder.role))
        .collect();
    assert_eq!(
        folders,
        [
            ("INBOX".to_owned(), Some(Role::Inbox)),
            ("Sent".to_owned(), Some(Role::Sent))
        ]
    );
    let inbox = inbox(&store, id);
    let old = store
        .get(store.find(inbox, &key(7, 1)).unwrap().unwrap())
        .unwrap();
    assert_eq!((old.body, old.flags), (false, vec!["\\Seen".to_owned()]));
    assert_eq!(old.subject.as_deref(), Some("Old"));
    let new = store.find(inbox, &key(7, 2)).unwrap().unwrap();
    assert_eq!(
        store.original(new).unwrap().unwrap(),
        NEW.replace('\n', "\r\n").as_bytes()
    );
    assert!(
        store.get(new).unwrap().flags.is_empty(),
        "\\Recent is not kept"
    );
    let synced = store.synced(id).unwrap();
    assert_eq!(
        (synced.at, synced.messages, synced.uncached),
        (Some(NOW), 2, 1)
    );
}

#[tokio::test]
async fn deletions_flags_and_dropped_folders_follow_the_server() {
    let (_dir, store, id) = opened();
    run(&store, id, first_sync()).await.unwrap();
    let report = run(
        &store,
        id,
        vec![
            expect("{tag} CAPABILITY"),
            send("* CAPABILITY IMAP4rev1\n{tag} OK caps\n"),
            expect("{tag} LIST \"\" *"),
            send("* LIST () \"/\" INBOX\n{tag} OK listed\n"),
            expect("{tag} EXAMINE \"INBOX\""),
            send("* 1 EXISTS\n* OK [UIDVALIDITY 7] v\n{tag} OK in\n"),
            expect("{tag} UID FETCH 1:* (UID FLAGS)"),
            send("* 1 FETCH (UID 1 FLAGS (\\Seen \\Flagged))\n{tag} OK flags\n"),
            expect(SINCE),
            send("* SEARCH\n{tag} OK found\n"),
        ],
    )
    .await
    .unwrap();
    assert_eq!(report.removed, 1);
    let store = store.lock().unwrap();
    assert_eq!(store.folders(id).unwrap().len(), 1);
    let inbox = inbox(&store, id);
    assert_eq!(store.find(inbox, &key(7, 2)).unwrap(), None);
    let old = store
        .get(store.find(inbox, &key(7, 1)).unwrap().unwrap())
        .unwrap();
    assert_eq!(old.flags, ["\\Flagged", "\\Seen"]);
    assert_eq!(store.sizes().unwrap().originals, 0);
}

#[tokio::test]
async fn a_new_uidvalidity_rebuilds_the_mailbox() {
    let (_dir, store, id) = opened();
    run(&store, id, first_sync()).await.unwrap();
    run(
        &store,
        id,
        vec![
            expect("{tag} CAPABILITY"),
            send("* CAPABILITY IMAP4rev1\n{tag} OK caps\n"),
            expect("{tag} LIST \"\" *"),
            send("* LIST () \"/\" INBOX\n{tag} OK listed\n"),
            expect("{tag} EXAMINE \"INBOX\""),
            send("* 1 EXISTS\n* OK [UIDVALIDITY 8] v\n{tag} OK in\n"),
            expect("{tag} UID FETCH 1:* (UID FLAGS)"),
            send("* 1 FETCH (UID 1 FLAGS ())\n{tag} OK flags\n"),
            expect(SINCE),
            send("* SEARCH 1\n{tag} OK found\n"),
            expect("{tag} UID FETCH 1 (UID BODYSTRUCTURE)"),
            send(&plain(1)),
            expect("{tag} UID FETCH 1 (UID FLAGS BODY.PEEK[])"),
            send(&literal(1, "", "BODY[]", NEW)),
            send("{tag} OK bodies\n"),
        ],
    )
    .await
    .unwrap();
    let store = store.lock().unwrap();
    let inbox = inbox(&store, id);
    assert_eq!(store.find(inbox, &key(7, 1)).unwrap(), None);
    let fresh = store.find(inbox, &key(8, 1)).unwrap().unwrap();
    assert_eq!(store.get(fresh).unwrap().subject.as_deref(), Some("New"));
    assert_eq!(store.synced(id).unwrap().messages, 1);
}

#[tokio::test]
async fn a_failed_sync_keeps_the_last_time() {
    let (_dir, store, id) = opened();
    let err = run(
        &store,
        id,
        vec![
            expect("{tag} CAPABILITY"),
            send("* CAPABILITY IMAP4rev1\n{tag} OK caps\n"),
            expect("{tag} LIST \"\" *"),
            send("{tag} NO [UNAVAILABLE] Try again later\n"),
        ],
    )
    .await
    .unwrap_err();
    assert!(
        matches!(&err, Error::Server(text) if text.contains("Try again later")),
        "{err}"
    );
    assert_eq!(store.lock().unwrap().synced(id).unwrap().at, None);
}

/// INBOX: UID 1 old, UID 2 recent; up to SEARCH.
fn inbox_of_two() -> Vec<fake::Step> {
    vec![
        expect("{tag} CAPABILITY"),
        send("* CAPABILITY IMAP4rev1\n{tag} OK caps\n"),
        expect("{tag} LIST \"\" *"),
        send("* LIST () \"/\" INBOX\n{tag} OK listed\n"),
        expect("{tag} EXAMINE \"INBOX\""),
        send("* 2 EXISTS\n* OK [UIDVALIDITY 7] v\n{tag} OK in\n"),
        expect("{tag} UID FETCH 1:* (UID FLAGS)"),
        send("* 1 FETCH (UID 1 FLAGS (\\Seen))\n* 2 FETCH (UID 2 FLAGS ())\n"),
        send("{tag} OK flags\n"),
        expect(SINCE),
        send("* SEARCH 2\n{tag} OK found\n"),
    ]
}

#[tokio::test]
async fn an_examine_without_exists_does_not_delete_mail() {
    let (_dir, store, id) = opened();
    run(&store, id, first_sync()).await.unwrap();
    let err = run(
        &store,
        id,
        vec![
            expect("{tag} CAPABILITY"),
            send("* CAPABILITY IMAP4rev1\n{tag} OK caps\n"),
            expect("{tag} LIST \"\" *"),
            send("* LIST () \"/\" INBOX\n* LIST () \"/\" Sent\n{tag} OK listed\n"),
            expect("{tag} EXAMINE \"INBOX\""),
            send("* OK [UIDVALIDITY 7] v\n{tag} OK in\n"),
        ],
    )
    .await
    .unwrap_err();
    assert!(
        matches!(&err, Error::Protocol(text) if text.contains("EXISTS")),
        "{err}"
    );
    let store = store.lock().unwrap();
    let synced = store.synced(id).unwrap();
    assert_eq!((synced.at, synced.messages), (Some(NOW), 2));
}

#[tokio::test]
async fn concurrent_flags_do_not_make_a_body_fetch_fail() {
    let (_dir, store, id) = opened();
    let mut steps = first_sync();
    let body = steps
        .iter()
        .position(|step| matches!(step, fake::Step::Send(text) if text.contains("BODY[]")))
        .unwrap();
    steps.insert(body + 1, send("* 2 FETCH (UID 2 FLAGS (\\Seen))\n"));
    run(&store, id, steps).await.unwrap();
    let store = store.lock().unwrap();
    let new = store.find(inbox(&store, id), &key(7, 2)).unwrap().unwrap();
    let new = store.get(new).unwrap();
    assert_eq!((new.body, new.flags), (true, vec!["\\Seen".to_owned()]));
}

#[tokio::test]
async fn inbox_casing_does_not_destroy_cached_mail() {
    let (_dir, store, id) = opened();
    run(&store, id, first_sync()).await.unwrap();
    let mut store = store.lock().unwrap();
    let kept = store
        .set_folders(id, &[("inbox", Some(Role::Inbox))])
        .unwrap();
    let folders = store.folders(id).unwrap();
    assert_eq!(folders.len(), 1);
    assert_eq!(
        (kept[0], folders[0].name.as_str()),
        (folders[0].id, "INBOX")
    );
    let new = store.find(kept[0], &key(7, 2)).unwrap().unwrap();
    assert_eq!(
        store.original(new).unwrap().unwrap(),
        NEW.replace('\n', "\r\n").as_bytes()
    );
}

#[tokio::test]
async fn roles_are_asked_for_when_the_server_can_list_them() {
    let (_dir, store, id) = opened();
    run(
        &store,
        id,
        vec![
            expect("{tag} CAPABILITY"),
            send("* CAPABILITY IMAP4rev1 LIST-EXTENDED SPECIAL-USE\n{tag} OK caps\n"),
            expect("{tag} LIST \"\" * RETURN (SPECIAL-USE)"),
            send("* LIST (\\Sent) \"/\" Outbox\n{tag} OK listed\n"),
            expect("{tag} EXAMINE \"Outbox\""),
            send("* 0 EXISTS\n* OK [UIDVALIDITY 3] v\n{tag} OK in\n"),
        ],
    )
    .await
    .unwrap();
    let folders = store.lock().unwrap().folders(id).unwrap();
    assert_eq!(folders[0].role, Some(Role::Sent));
}

/// One FETCH line carrying each section as a literal.
fn sections(uid: u32, parts: &[(&str, &str)]) -> String {
    let mut line = format!("* {uid} FETCH (UID {uid} FLAGS ()");
    for (name, text) in parts {
        let crlf = text.replace('\n', "\r\n");
        line.push_str(&format!(" BODY[{name}] {{{}}}\n{text}", crlf.len()));
    }
    line + ")\n{tag} OK text\n"
}

#[tokio::test]
async fn a_recent_body_sync_leaves_attachments_on_the_server() {
    let (_dir, store, id) = opened();
    let header = "Subject: Report\nFrom: b@example.com\nMIME-Version: 1.0\n\
                  Content-Type: multipart/mixed; boundary=\"b1\"\n\n";
    let structure = "* 2 FETCH (UID 2 BODYSTRUCTURE ((\"TEXT\" \"PLAIN\" (\"CHARSET\" \"us-ascii\") \
                     NIL NIL \"7BIT\" 6 1)(\"APPLICATION\" \"PDF\" (\"NAME\" \"report.pdf\") NIL NIL \
                     \"BASE64\" 12 NIL (\"ATTACHMENT\" (\"FILENAME\" \"report.pdf\")) NIL NIL) \
                     \"MIXED\" (\"BOUNDARY\" \"b1\") NIL NIL NIL))\n{tag} OK structure\n";
    let mut steps = inbox_of_two();
    steps.extend([
        expect("{tag} UID FETCH 1 (UID FLAGS BODY.PEEK[HEADER])"),
        send(&literal(1, "\\Seen", "BODY[HEADER]", OLD)),
        send("{tag} OK headers\n"),
        expect("{tag} UID FETCH 2 (UID BODYSTRUCTURE)"),
        send(structure),
        expect(
            "{tag} UID FETCH 2 (UID FLAGS BODY.PEEK[HEADER] BODY.PEEK[1.MIME] BODY.PEEK[1] \
             BODY.PEEK[2.MIME])",
        ),
        send(&sections(
            2,
            &[
                ("HEADER", header),
                ("1.MIME", "Content-Type: text/plain; charset=us-ascii\n\n"),
                ("1", "hello\n"),
                (
                    "2.MIME",
                    "Content-Type: application/pdf; name=\"report.pdf\"\n\
                     Content-Disposition: attachment; filename=\"report.pdf\"\n\
                     Content-Transfer-Encoding: base64\n\n",
                ),
            ],
        )),
    ]);
    run(&store, id, steps).await.unwrap();
    let store = store.lock().unwrap();
    let report = store.find(inbox(&store, id), &key(7, 2)).unwrap().unwrap();
    let stored = store.get(report).unwrap();
    assert_eq!(
        (stored.body, stored.missing.clone()),
        (true, vec!["2".to_owned()])
    );
    assert_eq!(stored.attachments, Some(1));
    let mail = ely_mail_engine::read(&store.original(report).unwrap().unwrap()).unwrap();
    assert_eq!(mail.text.as_deref().map(str::trim_end), Some("hello"));
    assert_eq!(mail.attachments[0].name.as_deref(), Some("report.pdf"));
    assert!(mail.attachments[0].bytes.is_empty());
    assert_eq!(store.synced(id).unwrap().partial, 1);
}

#[tokio::test]
async fn an_offline_folder_takes_every_message_whole() {
    let (_dir, store, id) = opened();
    run(&store, id, first_sync()).await.unwrap();
    let mailbox = inbox(&store.lock().unwrap(), id);
    store.lock().unwrap().set_offline(mailbox, true).unwrap();
    let steps = vec![
        expect("{tag} CAPABILITY"),
        send("* CAPABILITY IMAP4rev1\n{tag} OK caps\n"),
        expect("{tag} LIST \"\" *"),
        send("* LIST (\\HasNoChildren) \"/\" INBOX\n"),
        send("* LIST (\\HasNoChildren \\Sent) \"/\" Sent\n{tag} OK listed\n"),
        expect("{tag} EXAMINE \"INBOX\""),
        send("* 2 EXISTS\n* OK [UIDVALIDITY 7] v\n{tag} OK in\n"),
        expect("{tag} UID FETCH 1:* (UID FLAGS)"),
        send("* 1 FETCH (UID 1 FLAGS (\\Seen))\n* 2 FETCH (UID 2 FLAGS ())\n"),
        send("{tag} OK flags\n"),
        expect("{tag} UID FETCH 1 (UID FLAGS BODY.PEEK[])"),
        send(&literal(1, "\\Seen", "BODY[]", &format!("{OLD}kept\n"))),
        send("{tag} OK bodies\n"),
        expect("{tag} EXAMINE \"Sent\""),
        send("* 0 EXISTS\n* OK [UIDVALIDITY 3] v\n{tag} OK in\n"),
    ];
    let report = run(&store, id, steps).await.unwrap();
    assert_eq!((report.headers, report.bodies), (0, 1));
    let mut store = store.lock().unwrap();
    assert!(store.folders(id).unwrap()[0].offline);
    let held = store.open_mailbox(mailbox, 7).unwrap();
    assert!(held[&1].whole && held[&2].whole);
    let old = store.find(mailbox, &key(7, 1)).unwrap().unwrap();
    let bytes = store.original(old).unwrap().unwrap();
    assert!(String::from_utf8(bytes).unwrap().ends_with("kept\r\n"));
}

#[tokio::test]
async fn each_step_finds_the_folders_and_the_mail_so_far() {
    let (_dir, store, id) = opened();
    let (account, task, certs) = server(first_sync()).await;
    let seen = Mutex::new(Vec::new());
    let step = || {
        let store = store.lock().unwrap();
        let folders = store.folders(id).unwrap().len();
        let messages = store.synced(id).unwrap().messages;
        seen.lock().unwrap().push((folders, messages));
    };
    let secret = Secret::Password("pw".into());
    sync::imap(&account, id, secret, &certs.trusted, &store, NOW, &step)
        .await
        .unwrap();
    task.await.unwrap().unwrap();
    assert_eq!(*seen.lock().unwrap(), [(2, 0), (2, 2), (2, 2)]);
}

#[tokio::test]
async fn mail_trimmed_for_the_cap_stays_headers_only() {
    let (_dir, store, id) = opened();
    run(&store, id, first_sync()).await.unwrap();
    assert_eq!(store.lock().unwrap().trim_downloads(0, || true).unwrap(), 1);
    // The recent message is trimmed: no body is asked for.
    run(&store, id, inbox_of_two()).await.unwrap();
    let store = store.lock().unwrap();
    let recent = store.find(inbox(&store, id), &key(7, 2)).unwrap().unwrap();
    assert!(!store.get(recent).unwrap().body);
}
