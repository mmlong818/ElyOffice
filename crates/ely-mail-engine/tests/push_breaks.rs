//! The queue when a connection breaks or races (§12.3).

mod queue;

use ely_mail_engine::{
    Account, Change, Error, Fetched, Key, Secret, Security, push::Dropped, sync,
};
use fake::{certs, expect, send, serve};
use queue::{Here, caps, change, dropped, here, listed, present, push, select, steps, waiting};

/// 2026-10-19 12:00 UTC; bodies from 21 July.
const NOW: i64 = 1_792_411_200;
const SINCE: &str = "{tag} UID SEARCH SINCE 21-Jul-2026";
const NAMED: &[u8] =
    b"From: a@example.com\r\nMessage-ID: <m1@example.com>\r\nSubject: Hi\r\n\r\nhello\r\n";

/// Logs in and runs `steps`; the server then hangs up.
async fn broken(here: &Here, steps: Vec<fake::Step>) -> Error {
    let certs = certs();
    let mut script = vec![
        send("* OK ready\n"),
        expect("{tag} LOGIN \"ann@example.com\" \"pw\""),
        send("{tag} OK done\n"),
    ];
    script.extend(steps);
    let (incoming, _task) = serve(&certs, Security::Tls, script).await;
    let account = Account {
        incoming,
        ..fake::account("ann@example.com")
    };
    let secret = Secret::Password("pw".into());
    sync::changes(&account, here.id, secret, &certs.trusted, &here.store)
        .await
        .unwrap_err()
}

/// A whole sync of INBOX alone.
async fn synced(here: &Here, steps: Vec<fake::Step>) {
    let certs = certs();
    let mut script = vec![
        send("* OK ready\n"),
        expect("{tag} LOGIN \"ann@example.com\" \"pw\""),
        send("{tag} OK done\n"),
        expect("{tag} CAPABILITY"),
        send("* CAPABILITY IMAP4rev1\n{tag} OK caps\n"),
        expect("{tag} LIST \"\" *"),
        send("* LIST () \"/\" INBOX\n{tag} OK listed\n"),
        expect("{tag} EXAMINE \"INBOX\""),
        send("* 1 EXISTS\n* OK [UIDVALIDITY 7] v\n{tag} OK in\n"),
        expect("{tag} UID FETCH 1:* (UID FLAGS)"),
    ];
    script.extend(steps);
    script.push(expect("{tag} LOGOUT"));
    script.push(send("* BYE\n{tag} OK bye\n"));
    let (incoming, task) = serve(&certs, Security::Tls, script).await;
    let account = Account {
        incoming,
        ..fake::account("ann@example.com")
    };
    let secret = Secret::Password("pw".into());
    sync::imap(
        &account,
        here.id,
        secret,
        &certs.trusted,
        &here.store,
        NOW,
        &|| {},
    )
    .await
    .unwrap();
    task.await.unwrap().unwrap();
}

/// The user flags `uid` 1 now.
fn flag_now(here: &Here) -> fake::Step {
    let store = here.store.clone();
    let id = store
        .lock()
        .unwrap()
        .find(here.inbox, &key(1))
        .unwrap()
        .unwrap();
    fake::Step::Run(Box::new(move || {
        let change = Change::Flagged(true);
        store.lock().unwrap().change(&[id], change).unwrap();
    }))
}

fn key(uid: u32) -> Key {
    Key::Imap {
        uidvalidity: 7,
        uid,
    }
}

fn flags(here: &Here, uid: u32) -> Vec<String> {
    let store = here.store.lock().unwrap();
    let id = store.find(here.inbox, &key(uid)).unwrap().unwrap();
    store.get(id).unwrap().flags
}

#[tokio::test]
async fn a_copy_kept_before_the_source_goes_is_not_made_twice() {
    let (here, ids) = here(&[3]);
    change(&here, &ids, Change::Move(here.work));
    let script = steps(vec![
        caps("UIDPLUS"),
        select(7, 1),
        present("3", &[3]),
        vec![
            expect("{tag} UID COPY 3 \"Work\""),
            send("{tag} OK [COPYUID 7 3 9] copied\n"),
            expect("{tag} UID STORE 3 +FLAGS.SILENT (\\Deleted)"),
        ],
    ]);
    broken(&here, script).await;
    // Kept where it went; only the source waits.
    let found = here.store.lock().unwrap().find(here.work, &key(9));
    assert_eq!(found.unwrap(), Some(ids[0]));
    assert_eq!(listed(&here, here.work), ids);
    assert_eq!(waiting(&here), 1);
    let script = steps(vec![
        caps("UIDPLUS"),
        select(7, 1),
        present("3", &[3]),
        vec![
            expect("{tag} UID STORE 3 +FLAGS.SILENT (\\Deleted)"),
            send("{tag} OK stored\n"),
            expect("{tag} UID EXPUNGE 3"),
            send("* 1 EXPUNGE\n{tag} OK gone\n"),
        ],
    ]);
    assert_eq!(push(&here, script).await.unwrap(), []);
    assert_eq!(waiting(&here), 0);
}

#[tokio::test]
async fn a_move_without_uids_finds_it_by_message_id_for_a_newer_change() {
    let (here, _) = here(&[]);
    let id = (here.store.lock().unwrap())
        .put(here.inbox, &key(1), &[], Fetched::Whole(NAMED))
        .unwrap();
    change(&here, &[id], Change::Move(here.work));
    let store = here.store.clone();
    let script = steps(vec![
        caps("MOVE"),
        select(7, 1),
        present("1", &[1]),
        vec![
            expect("{tag} UID MOVE 1 \"Work\""),
            fake::Step::Run(Box::new(move || {
                let change = Change::Flagged(true);
                store.lock().unwrap().change(&[id], change).unwrap();
            })),
            send("* 1 EXPUNGE\n{tag} OK done\n"),
            expect("{tag} SELECT \"Work\""),
            send("* 1 EXISTS\n* OK [UIDVALIDITY 7] v\n{tag} OK [READ-WRITE] in\n"),
            expect("{tag} UID SEARCH HEADER Message-ID \"<m1@example.com>\""),
            send("* SEARCH 40\n{tag} OK found\n"),
        ],
    ]);
    assert_eq!(push(&here, script).await.unwrap(), []);
    let found = here.store.lock().unwrap().find(here.work, &key(40));
    assert_eq!(found.unwrap(), Some(id));
    // The flag waits to go to its new place.
    assert_eq!(waiting(&here), 1);
}

#[tokio::test]
async fn a_newer_change_to_a_move_lost_to_view_is_told() {
    let (here, ids) = here(&[1]);
    change(&here, &ids, Change::Move(here.work));
    let (store, id) = (here.store.clone(), ids[0]);
    let script = steps(vec![
        caps("MOVE"),
        select(7, 1),
        present("1", &[1]),
        vec![
            expect("{tag} UID MOVE 1 \"Work\""),
            fake::Step::Run(Box::new(move || {
                let change = Change::Flagged(true);
                store.lock().unwrap().change(&[id], change).unwrap();
            })),
            send("* 1 EXPUNGE\n{tag} OK done\n"),
        ],
    ]);
    assert_eq!(push(&here, script).await.unwrap(), [Dropped::Unplaced]);
}

#[tokio::test]
async fn a_refusal_outlives_a_broken_logout() {
    let (here, ids) = here(&[1]);
    change(&here, &ids, Change::Move(here.work));
    let script = steps(vec![
        caps("MOVE"),
        select(7, 1),
        present("1", &[1]),
        vec![
            expect("{tag} UID MOVE 1 \"Work\""),
            send("{tag} NO cannot move\n"),
            expect("{tag} LOGOUT"),
        ],
    ]);
    broken(&here, script).await;
    let refused = Dropped::Refused("NO cannot move".into());
    assert_eq!(dropped(&here), [refused]);
    assert_eq!(waiting(&here), 0);
}

#[tokio::test]
async fn a_refusal_cut_short_is_not_undone_yet() {
    let (here, ids) = here(&[1]);
    change(&here, &ids, Change::Flagged(true));
    let script = steps(vec![
        caps(""),
        select(7, 1),
        present("1", &[1]),
        vec![
            expect("{tag} UID STORE 1 +FLAGS.SILENT (\\Flagged)"),
            send("{tag} NO read-only folder\n"),
            expect("{tag} UID FETCH 1 (UID FLAGS)"),
        ],
    ]);
    broken(&here, script).await;
    assert_eq!(waiting(&here), 1);
    let flags = here.store.lock().unwrap().get(ids[0]).unwrap().flags;
    assert_eq!(flags, ["\\Flagged"]);
}

#[tokio::test]
async fn what_is_known_is_kept_when_a_later_step_breaks() {
    let (here, ids) = here(&[1, 2]);
    change(&here, &ids[..1], Change::Flagged(true));
    change(&here, &ids[1..], Change::Move(here.work));
    let script = steps(vec![
        caps("MOVE"),
        select(7, 2),
        present("1:2", &[1, 2]),
        vec![
            expect("{tag} UID STORE 1 +FLAGS.SILENT (\\Flagged)"),
            send("{tag} NO read-only folder\n"),
            expect("{tag} UID FETCH 1 (UID FLAGS)"),
            send("* 1 FETCH (UID 1 FLAGS ())\n{tag} OK flags\n"),
            expect("{tag} UID MOVE 2 \"Work\""),
        ],
    ]);
    broken(&here, script).await;
    let refused = Dropped::Refused("NO read-only folder".into());
    assert_eq!(dropped(&here), [refused]);
    assert_eq!(flags(&here, 1), Vec::<String>::new());
    // The move's fate is unknown: it waits.
    assert_eq!(waiting(&here), 1);
}

#[tokio::test]
async fn server_flags_read_mid_change_keep_the_change() {
    let (here, _) = here(&[1]);
    let script = vec![
        flag_now(&here),
        send("* 1 FETCH (UID 1 FLAGS (\\Seen \\Answered))\n{tag} OK flags\n"),
        expect(SINCE),
        send("* SEARCH\n{tag} OK found\n"),
    ];
    synced(&here, script).await;
    assert_eq!(flags(&here, 1), ["\\Answered", "\\Flagged", "\\Seen"]);
    assert_eq!(waiting(&here), 1);
}

#[tokio::test]
async fn a_body_downloaded_mid_change_keeps_the_change() {
    let (here, _) = here(&[]);
    let mail = b"From: a@example.com\r\nSubject: Hi\r\n\r\n";
    (here.store.lock().unwrap())
        .put(here.inbox, &key(1), &[], Fetched::Headers(mail))
        .unwrap();
    let body = "From: a@example.com\nSubject: Hi\n\nhello\n";
    let script = vec![
        send("* 1 FETCH (UID 1 FLAGS ())\n{tag} OK flags\n"),
        expect(SINCE),
        send("* SEARCH 1\n{tag} OK found\n"),
        expect("{tag} UID FETCH 1 (UID BODYSTRUCTURE)"),
        send(
            "* 1 FETCH (UID 1 BODYSTRUCTURE (\"TEXT\" \"PLAIN\" NIL NIL NIL \"7BIT\" 7 1))\n\
             {tag} OK structure\n",
        ),
        expect("{tag} UID FETCH 1 (UID FLAGS BODY.PEEK[])"),
        flag_now(&here),
        send(&format!(
            "* 1 FETCH (UID 1 FLAGS () BODY[] {{{}}}\n{body})\n{{tag}} OK bodies\n",
            body.replace('\n', "\r\n").len()
        )),
    ];
    synced(&here, script).await;
    assert_eq!(flags(&here, 1), ["\\Flagged"]);
}
