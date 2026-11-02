use ely_mail_engine::{
    Error, Left, Remote, Store, Unsent,
    compose::{Earlier, attachment},
};
use fake::account;

fn unsent(subject: &str) -> Unsent {
    Unsent {
        to: vec!["bo@example.com".into()],
        cc: Vec::new(),
        bcc: vec!["cy@example.com".into()],
        subject: subject.into(),
        words: "Lunch at **one**?".into(),
        markdown: true,
        earlier: Some(Earlier::Quoted {
            lead: "Bo wrote:".into(),
            text: "Lunch?".into(),
        }),
        in_reply_to: Some("<a@x>".into()),
        references: vec!["<a@x>".into()],
        headers_only: false,
        missing: 1,
    }
}

#[test]
fn a_draft_and_its_files_outlast_the_store() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = Store::open(dir.path()).unwrap();
    let ann = store.add_account(&account("ann@example.com")).unwrap();
    let file = attachment("plan.pdf", b"%PDF".to_vec());
    let id = store
        .shelve(
            None,
            ann,
            &unsent("Plan"),
            Some(std::slice::from_ref(&file)),
        )
        .unwrap()
        .id;
    drop(store);
    let mut store = Store::open(dir.path()).unwrap();
    let kept = store.draft(id).unwrap().expect("kept");
    assert_eq!(kept.account, ann);
    assert_eq!(kept.unsent, unsent("Plan"));
    assert_eq!(kept.remote, None);
    assert_eq!(store.draft_files(id).unwrap(), std::slice::from_ref(&file));
    store
        .shelve(Some(id), ann, &unsent("Plan 2"), None)
        .unwrap();
    assert_eq!(
        store.draft_files(id).unwrap(),
        [file],
        "files left as they were"
    );
    assert_eq!(store.draft(id).unwrap().unwrap().unsent.subject, "Plan 2");
    store
        .shelve(Some(id), ann, &unsent("Plan 2"), Some(&[]))
        .unwrap();
    assert!(store.draft_files(id).unwrap().is_empty());
}

#[test]
fn drafts_list_newest_first_and_go_with_their_account() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = Store::open(dir.path()).unwrap();
    let ann = store.add_account(&account("ann@example.com")).unwrap();
    let bo = store.add_account(&account("bo@example.com")).unwrap();
    let first = store.shelve(None, ann, &unsent("one"), None).unwrap().id;
    let second = store.shelve(None, bo, &unsent("two"), None).unwrap().id;
    let ids: Vec<_> = store.drafts().unwrap().iter().map(|it| it.id).collect();
    assert_eq!(ids, [second, first]);
    store.remove_account(bo).unwrap();
    let ids: Vec<_> = store.drafts().unwrap().iter().map(|it| it.id).collect();
    assert_eq!(ids, [first]);
    let err = store
        .shelve(Some(second), ann, &unsent("two"), None)
        .unwrap_err();
    assert!(matches!(err, Error::Store(_)), "{err}");
}

#[test]
fn dropping_a_draft_gives_its_server_copy() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = Store::open(dir.path()).unwrap();
    let ann = store.add_account(&account("ann@example.com")).unwrap();
    let file = attachment("a.txt", b"a".to_vec());
    let id = store
        .shelve(None, ann, &unsent("x"), Some(&[file]))
        .unwrap()
        .id;
    let copy = Remote {
        mailbox: "Drafts".into(),
        uidvalidity: 7,
        uid: 41,
    };
    assert_eq!(store.set_remote(id, ann, &copy).unwrap(), None);
    assert_eq!(
        store.draft(id).unwrap().unwrap().remote.as_ref(),
        Some(&copy)
    );
    let left = Left {
        account: ann,
        remote: copy.clone(),
    };
    assert_eq!(store.unshelve(id).unwrap(), Some(left.clone()));
    assert_eq!(store.draft(id).unwrap(), None);
    assert!(store.draft_files(id).unwrap().is_empty());
    assert_eq!(store.set_remote(id, ann, &copy).unwrap(), Some(left));
}

#[test]
fn a_new_sender_hands_back_the_old_accounts_copy() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = Store::open(dir.path()).unwrap();
    let ann = store.add_account(&account("ann@example.com")).unwrap();
    let bo = store.add_account(&account("bo@example.com")).unwrap();
    let id = store.shelve(None, ann, &unsent("x"), None).unwrap().id;
    let copy = Remote {
        mailbox: "Drafts".into(),
        uidvalidity: 7,
        uid: 41,
    };
    store.set_remote(id, ann, &copy).unwrap();
    let kept = store.shelve(Some(id), bo, &unsent("x"), None).unwrap();
    let left = Left {
        account: ann,
        remote: copy.clone(),
    };
    assert_eq!(kept.left, Some(left.clone()));
    assert_eq!(store.draft(id).unwrap().unwrap().remote, None);
    assert_eq!(store.unshelve(id).unwrap(), None, "bo holds no copy");
    let id = store.shelve(None, bo, &unsent("y"), None).unwrap().id;
    assert_eq!(
        store.set_remote(id, ann, &copy).unwrap(),
        Some(left),
        "a copy placed for ann after the move goes back"
    );
    assert_eq!(store.draft(id).unwrap().unwrap().remote, None);
}

#[test]
fn drafts_stay_local_until_asked() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).unwrap();
    let ann = store.add_account(&account("ann@example.com")).unwrap();
    assert!(!store.drafts_up(ann).unwrap());
    store.set_drafts_up(ann, true).unwrap();
    drop(store);
    let store = Store::open(dir.path()).unwrap();
    assert!(store.drafts_up(ann).unwrap());
}
