use std::sync::Mutex;

use ely_mail_engine::{
    Account, AccountId, Error, Key, Protocol, Role, Secret, Security, Store, sync,
};
use fake::{account, certs, expect, send, serve};

const NOW: i64 = 1_792_411_200;

fn opened(keep: bool) -> (tempfile::TempDir, Mutex<Store>, Account, AccountId) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).unwrap();
    let account = Account {
        protocol: Protocol::Pop { keep },
        ..account("ann@example.com")
    };
    let id = store.add_account(&account).unwrap();
    (dir, Mutex::new(store), account, id)
}

async fn run(
    store: &Mutex<Store>,
    account: &Account,
    id: AccountId,
    steps: Vec<fake::Step>,
) -> Result<sync::Report, Error> {
    stepped(store, account, id, steps, &|| {}).await
}

/// A login, then `steps`; `step` as the sync gives it.
async fn stepped(
    store: &Mutex<Store>,
    account: &Account,
    id: AccountId,
    steps: Vec<fake::Step>,
    step: &(dyn Fn() + Sync),
) -> Result<sync::Report, Error> {
    let certs = certs();
    let mut script = vec![
        send("+OK ready\n"),
        expect("USER ann@example.com"),
        send("+OK\n"),
        expect("PASS pw"),
        send("+OK in\n"),
    ];
    script.extend(steps);
    let (incoming, task) = serve(&certs, Security::Tls, script).await;
    let account = Account {
        incoming,
        ..account.clone()
    };
    let secret = Secret::Password("pw".into());
    let done = sync::pop(&account, id, secret, &certs.trusted, store, NOW, step).await;
    if done.is_ok() {
        task.await.unwrap().unwrap();
    }
    done
}

fn subjects(store: &Mutex<Store>, id: AccountId) -> Vec<(String, String)> {
    let store = store.lock().unwrap();
    let [inbox] = store.folders(id).unwrap().try_into().unwrap();
    assert_eq!(
        (inbox.name.as_str(), inbox.role),
        ("INBOX", Some(Role::Inbox))
    );
    let mut out: Vec<_> = store
        .uidls(inbox.id)
        .unwrap()
        .into_iter()
        .map(|uidl| {
            let key = Key::Pop { uidl: uidl.clone() };
            let found = store.find(inbox.id, &key).unwrap().unwrap();
            let stored = store.get(found).unwrap();
            assert!(stored.body);
            (uidl, stored.subject.unwrap())
        })
        .collect();
    out.sort();
    out
}

fn quit() -> [fake::Step; 2] {
    [expect("QUIT"), send("+OK bye\n")]
}

#[tokio::test]
async fn new_mail_comes_down_once_and_stays_on_the_server() {
    let (_dir, store, account, id) = opened(true);
    let mut first = vec![
        expect("UIDL"),
        send("+OK\n1 aa1\n2 bb2\n.\n"),
        expect("RETR 1"),
        send("+OK\nSubject: One\n\nhi\n.\n"),
        expect("RETR 2"),
        send("+OK\nSubject: Two\n\nhi\n.\n"),
    ];
    first.extend(quit());
    let report = run(&store, &account, id, first).await.unwrap();
    assert_eq!((report.bodies, report.deleted), (2, 0));
    assert_eq!(store.lock().unwrap().synced(id).unwrap().at, Some(NOW));
    let mut second = vec![
        expect("UIDL"),
        send("+OK\n1 aa1\n2 bb2\n3 cc3\n.\n"),
        expect("RETR 3"),
        send("+OK\nSubject: Three\n\nhi\n.\n"),
    ];
    second.extend(quit());
    let report = run(&store, &account, id, second).await.unwrap();
    assert_eq!((report.bodies, report.deleted), (1, 0));
    let held = subjects(&store, id);
    assert_eq!(
        held,
        [("aa1", "One"), ("bb2", "Two"), ("cc3", "Three")]
            .map(|(uidl, subject)| (uidl.to_owned(), subject.to_owned()))
    );
}

#[tokio::test]
async fn without_keep_each_stored_message_is_deleted() {
    let (_dir, store, account, id) = opened(false);
    let mut first = vec![
        expect("UIDL"),
        send("+OK\n1 aa1\n.\n"),
        expect("RETR 1"),
        send("+OK\nSubject: One\n\nhi\n.\n"),
        expect("DELE 1"),
        send("+OK deleted\n"),
    ];
    first.extend(quit());
    run(&store, &account, id, first).await.unwrap();
    // A held message the server still lists goes too.
    let mut second = vec![
        expect("UIDL"),
        send("+OK\n1 aa1\n2 bb2\n.\n"),
        expect("DELE 1"),
        send("+OK deleted\n"),
        expect("RETR 2"),
        send("+OK\nSubject: Two\n\nhi\n.\n"),
        expect("DELE 2"),
        send("+OK deleted\n"),
    ];
    second.extend(quit());
    let report = run(&store, &account, id, second).await.unwrap();
    assert_eq!((report.bodies, report.deleted), (1, 2));
    assert_eq!(subjects(&store, id).len(), 2);
}

#[tokio::test]
async fn a_uidl_listed_twice_comes_down_once() {
    for keep in [true, false] {
        let (_dir, store, account, id) = opened(keep);
        let mut steps = vec![
            expect("UIDL"),
            send("+OK\n1 aa1\n2 aa1\n.\n"),
            expect("RETR 1"),
            send("+OK\nSubject: One\n\nhi\n.\n"),
        ];
        if !keep {
            steps.extend([
                expect("DELE 1"),
                send("+OK deleted\n"),
                expect("DELE 2"),
                send("+OK deleted\n"),
            ]);
        }
        steps.extend(quit());
        let report = run(&store, &account, id, steps).await.unwrap();
        let deleted = if keep { 0 } else { 2 };
        assert_eq!((report.bodies, report.deleted), (1, deleted));
        assert_eq!(subjects(&store, id).len(), 1);
    }
}

#[tokio::test]
async fn a_refused_message_stops_the_sync_before_any_delete() {
    let (_dir, store, account, id) = opened(false);
    let steps = vec![
        expect("UIDL"),
        send("+OK\n1 aa1\n.\n"),
        expect("RETR 1"),
        send("-ERR gone\n"),
    ];
    let error = run(&store, &account, id, steps).await.unwrap_err();
    assert!(
        matches!(error, Error::Server(ref text) if text == "gone"),
        "{error}"
    );
    assert_eq!(store.lock().unwrap().synced(id).unwrap().at, None);
    assert!(subjects(&store, id).is_empty());
}

#[tokio::test]
async fn an_imap_account_is_refused() {
    let (_dir, store, account, id) = opened(true);
    let account = Account {
        protocol: Protocol::Imap,
        ..account
    };
    let secret = Secret::Password("pw".into());
    let error = sync::pop(&account, id, secret, &certs().trusted, &store, NOW, &|| {})
        .await
        .unwrap_err();
    assert!(matches!(error, Error::Invalid(_)), "{error}");
}

#[tokio::test]
async fn mail_deleted_here_does_not_come_down_again() {
    let (_dir, store, account, id) = opened(true);
    let mut first = vec![
        expect("UIDL"),
        send("+OK\n1 aa1\n.\n"),
        expect("RETR 1"),
        send("+OK\nSubject: One\n\nhi\n.\n"),
    ];
    first.extend(quit());
    run(&store, &account, id, first).await.unwrap();
    let inbox = store.lock().unwrap().folders(id).unwrap()[0].id;
    let key = Key::Pop { uidl: "aa1".into() };
    let one = store.lock().unwrap().find(inbox, &key).unwrap().unwrap();
    let delete = ely_mail_engine::Change::Delete { erase: true };
    store.lock().unwrap().change(&[one], delete).unwrap();
    let mut again = vec![expect("UIDL"), send("+OK\n1 aa1\n.\n")];
    again.extend(quit());
    let report = run(&store, &account, id, again).await.unwrap();
    assert_eq!(report.bodies, 0);
    assert_eq!(store.lock().unwrap().find(inbox, &key).unwrap(), None);
    // Once the server lets it go, so does the record.
    let mut gone = vec![expect("UIDL"), send("+OK\n.\n")];
    gone.extend(quit());
    run(&store, &account, id, gone).await.unwrap();
    assert!(store.lock().unwrap().uidls(inbox).unwrap().is_empty());
}

#[tokio::test]
async fn the_inbox_is_stored_before_the_mail_comes() {
    let (_dir, store, account, id) = opened(true);
    let mut steps = vec![
        expect("UIDL"),
        send(
            "+OK
1 aa1
.
",
        ),
        expect("RETR 1"),
        send(
            "+OK
Subject: One

hi
.
",
        ),
    ];
    steps.extend(quit());
    let seen = Mutex::new(Vec::new());
    let step = || {
        let store = store.lock().unwrap();
        let folders = store.folders(id).unwrap().len();
        let messages = store.synced(id).unwrap().messages;
        seen.lock().unwrap().push((folders, messages));
    };
    stepped(&store, &account, id, steps, &step).await.unwrap();
    assert_eq!(*seen.lock().unwrap(), [(1, 0)]);
}
