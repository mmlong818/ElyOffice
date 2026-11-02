use ely_mail_engine::Store;

#[test]
fn a_trusted_sender_stays_trusted_in_any_case_until_undone() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).unwrap();
    assert!(!store.trusts("ann@example.com").unwrap());
    store.set_trusted("Ann@Example.com", true).unwrap();
    store.set_trusted("ann@example.com", true).unwrap();
    drop(store);
    let store = Store::open(dir.path()).unwrap();
    assert!(store.trusts("ANN@example.COM").unwrap());
    assert!(!store.trusts("bo@example.com").unwrap());
    store.set_trusted("ann@EXAMPLE.com", false).unwrap();
    assert!(!store.trusts("ann@example.com").unwrap());
}
