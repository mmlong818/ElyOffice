use ely_mail_engine::{Error, Identity, Store};
use fake::account;

#[test]
fn an_identity_is_kept_per_account_and_goes_with_it() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).unwrap();
    let ann = store.add_account(&account("ann@example.com")).unwrap();
    let bo = store.add_account(&account("bo@example.com")).unwrap();
    assert_eq!(store.identity(ann).unwrap(), Identity::default());
    let signed = Identity {
        name: Some("  Ann Lee ".into()),
        signature: "Ann\nAtrium".into(),
    };
    store.set_identity(ann, &signed).unwrap();
    drop(store);
    let mut store = Store::open(dir.path()).unwrap();
    let kept = store.identity(ann).unwrap();
    assert_eq!(kept.name.as_deref(), Some("Ann Lee"));
    assert_eq!(kept.signature, "Ann\nAtrium");
    assert_eq!(store.identity(bo).unwrap(), Identity::default());
    let blank = Identity {
        name: Some(" ".into()),
        signature: String::new(),
    };
    store.set_identity(ann, &blank).unwrap();
    assert_eq!(store.identity(ann).unwrap(), Identity::default());
    store.set_identity(ann, &signed).unwrap();
    store.remove_account(ann).unwrap();
    let again = store.add_account(&account("ann@example.com")).unwrap();
    assert_eq!(store.identity(again).unwrap(), Identity::default());
}

#[test]
fn a_name_on_two_lines_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).unwrap();
    let ann = store.add_account(&account("ann@example.com")).unwrap();
    let broken = Identity {
        name: Some("Ann\r\nBcc: x@example.com".into()),
        signature: String::new(),
    };
    let err = store.set_identity(ann, &broken).unwrap_err();
    assert!(matches!(err, Error::Invalid(_)), "{err}");
    assert_eq!(store.identity(ann).unwrap(), Identity::default());
}

#[test]
fn a_mailbox_knows_its_account() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = Store::open(dir.path()).unwrap();
    store.add_account(&account("ann@example.com")).unwrap();
    let bo = store.add_account(&account("bo@example.com")).unwrap();
    let [inbox] = store
        .set_folders(bo, &[("INBOX", None)])
        .unwrap()
        .try_into()
        .unwrap();
    assert_eq!(store.account_of(inbox).unwrap(), bo);
}
