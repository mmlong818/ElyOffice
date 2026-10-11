use serde_json::json;

use super::{Store, VERSION};

fn store() -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::at(dir.path().join("ElyOffice").join("settings.json"));
    (dir, store)
}

#[test]
fn an_absent_file_is_empty_and_a_write_makes_it_versioned() {
    let (_dir, store) = store();
    assert!(store.read().unwrap().is_empty());
    store.set("appearance", json!("dark")).unwrap();
    let read = store.read().unwrap();
    assert_eq!(read["appearance"], "dark");
    assert_eq!(read["version"], VERSION);
}

#[test]
fn a_write_keeps_fields_another_app_wrote() {
    let (_dir, store) = store();
    std::fs::create_dir_all(store.path().parent().unwrap()).unwrap();
    std::fs::write(
        store.path(),
        r#"{"version":1,"later":{"kept":true},"language":"en"}"#,
    )
    .unwrap();
    store.set("language", json!("zh-CN")).unwrap();
    let read = store.read().unwrap();
    assert_eq!(read["language"], "zh-CN");
    assert_eq!(read["later"], json!({"kept": true}), "unknown fields stay");
}

#[test]
fn two_writers_at_once_lose_nothing() {
    let (_dir, store) = store();
    let writers: Vec<_> = (0..8)
        .map(|n| {
            let store = store.clone();
            std::thread::spawn(move || store.set(&format!("field{n}"), json!(n)).unwrap())
        })
        .collect();
    for writer in writers {
        writer.join().unwrap();
    }
    let read = store.read().unwrap();
    assert!((0..8).all(|n| read[&format!("field{n}")] == n), "{read:?}");
}

#[test]
fn a_newer_or_broken_file_is_refused_and_left_alone() {
    let (_dir, store) = store();
    std::fs::create_dir_all(store.path().parent().unwrap()).unwrap();
    for text in [r#"{"version":2}"#, r#"{"version":"1"}"#, "[1]", "{"] {
        std::fs::write(store.path(), text).unwrap();
        assert!(store.read().is_err(), "{text} is refused");
        assert!(
            store.set("language", json!("en")).is_err(),
            "{text} is not overwritten"
        );
        assert_eq!(std::fs::read_to_string(store.path()).unwrap(), text);
    }
}

#[test]
fn shared_fields_type_strictly_and_default_when_absent() {
    use super::{Appearance, Language, Motion, Shared};
    let fields = |text: &str| serde_json::from_str::<serde_json::Map<_, _>>(text).unwrap();
    assert_eq!(
        Shared::from_fields(&fields("{}")).unwrap(),
        Shared::default()
    );
    let read = Shared::from_fields(&fields(
        r#"{"appearance":"dark","language":"zh-CN","motion":"reduce","omarchy_sync":false,"author":" Ana "}"#,
    ))
    .unwrap();
    assert_eq!(
        (
            read.appearance,
            read.language,
            read.motion,
            read.omarchy_sync,
            read.author.as_str()
        ),
        (
            Appearance::Dark,
            Language::Chinese,
            Motion::Reduced,
            false,
            "Ana"
        )
    );
    for bad in [
        r#"{"appearance":"blue"}"#,
        r#"{"omarchy_sync":"yes"}"#,
        r#"{"author":3}"#,
    ] {
        assert!(Shared::from_fields(&fields(bad)).is_err(), "{bad}");
    }
}

#[test]
fn a_write_overtaken_while_waiting_leaves_the_file() {
    let (_dir, store) = store();
    store.set("author", json!("Ana")).unwrap();
    let held = crate::lock::FileLock::take(&store.path().parent().unwrap().join(".settings.lock"))
        .unwrap();
    let waiting = {
        let store = store.clone();
        std::thread::spawn(move || store.set_if("author", json!("Bob"), || false).unwrap())
    };
    std::thread::sleep(std::time::Duration::from_millis(200));
    drop(held);
    assert!(!waiting.join().unwrap(), "the overtaken write is skipped");
    assert_eq!(store.read().unwrap()["author"], "Ana");
}
