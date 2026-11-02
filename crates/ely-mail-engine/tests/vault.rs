use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use ely_mail_engine::{Backend, Error, Kept, Vault};

#[derive(Clone, Default)]
struct Fake {
    saved: Arc<Mutex<HashMap<String, String>>>,
    refuse: Option<&'static str>,
}

impl Backend for Fake {
    fn get(&self, name: &str) -> Result<Option<String>, String> {
        match self.refuse {
            Some(reason) => Err(reason.into()),
            None => Ok(self.saved.lock().unwrap().get(name).cloned()),
        }
    }

    fn set(&self, name: &str, secret: &str) -> Result<(), String> {
        match self.refuse {
            Some(reason) => Err(reason.into()),
            None => {
                self.saved
                    .lock()
                    .unwrap()
                    .insert(name.into(), secret.into());
                Ok(())
            }
        }
    }

    fn delete(&self, name: &str) -> Result<(), String> {
        match self.refuse {
            Some(reason) => Err(reason.into()),
            None => {
                self.saved.lock().unwrap().remove(name);
                Ok(())
            }
        }
    }
}

#[test]
fn secrets_go_to_the_system_store() {
    let fake = Fake::default();
    let vault = Vault::new(Box::new(fake.clone()));
    assert_eq!(vault.get("a").unwrap(), None);
    assert_eq!(vault.put("a", "rt-1"), Kept::System);
    assert_eq!(fake.saved.lock().unwrap()["a"], "rt-1");
    assert_eq!(vault.get("a").unwrap().as_deref(), Some("rt-1"));
    vault.remove("a").unwrap();
    assert!(fake.saved.lock().unwrap().is_empty());
}

#[test]
fn a_refused_store_keeps_the_secret_for_this_session() {
    let vault = Vault::new(Box::new(Fake {
        refuse: Some("locked"),
        ..Fake::default()
    }));
    assert_eq!(vault.put("a", "pw"), Kept::Session("locked".into()));
    assert_eq!(vault.get("a").unwrap().as_deref(), Some("pw"));
    let err = vault.get("b").err().unwrap();
    assert!(matches!(err, Error::Vault(text) if text.contains("locked")));
    let err = vault.remove("a").err().unwrap();
    assert!(matches!(err, Error::Vault(_)));
    assert_eq!(
        vault.get("a").unwrap().as_deref(),
        Some("pw"),
        "a failed remove keeps the session copy"
    );
}

#[test]
fn a_new_secret_replaces_the_session_copy() {
    let fake = Fake {
        refuse: Some("locked"),
        ..Fake::default()
    };
    let vault = Vault::new(Box::new(fake));
    vault.put("a", "old");
    assert_eq!(vault.put("a", "new"), Kept::Session("locked".into()));
    assert_eq!(vault.get("a").unwrap().as_deref(), Some("new"));
}
