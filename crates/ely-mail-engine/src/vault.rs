use std::{collections::HashMap, sync::Mutex};

use crate::{Error, Result};

/// A secret store backend, or a test double.
pub trait Backend: Send + Sync {
    fn get(&self, name: &str) -> std::result::Result<Option<String>, String>;
    fn set(&self, name: &str, secret: &str) -> std::result::Result<(), String>;
    fn delete(&self, name: &str) -> std::result::Result<(), String>;
}

/// The system credential store (§18.2).
pub struct Keyring;

const SERVICE: &str = "ElyMail";

impl Backend for Keyring {
    fn get(&self, name: &str) -> std::result::Result<Option<String>, String> {
        match entry(name)?.get_password() {
            Ok(secret) => Ok(Some(secret)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(err) => Err(err.to_string()),
        }
    }

    fn set(&self, name: &str, secret: &str) -> std::result::Result<(), String> {
        entry(name)?
            .set_password(secret)
            .map_err(|err| err.to_string())
    }

    fn delete(&self, name: &str) -> std::result::Result<(), String> {
        match entry(name)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(err) => Err(err.to_string()),
        }
    }
}

fn entry(name: &str) -> std::result::Result<keyring::Entry, String> {
    keyring::Entry::new(SERVICE, name).map_err(|err| err.to_string())
}

/// Where a kept secret lives.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Kept {
    System,
    /// The system store refused; gone when the app quits.
    Session(String),
}

/// Passwords and refresh tokens: system store, else memory.
pub struct Vault {
    backend: Box<dyn Backend>,
    session: Mutex<HashMap<String, String>>,
}

impl Vault {
    pub fn system() -> Self {
        Self::new(Box::new(Keyring))
    }

    pub fn new(backend: Box<dyn Backend>) -> Self {
        Self {
            backend,
            session: Mutex::new(HashMap::new()),
        }
    }

    /// Stores a secret; never on disk outside the system store.
    pub fn put(&self, name: &str, secret: &str) -> Kept {
        self.session().remove(name);
        match self.backend.set(name, secret) {
            Ok(()) => {
                log::info!("vault: {name} kept in the system store");
                Kept::System
            }
            Err(reason) => {
                log::warn!("vault: system store refused {name}, this session only: {reason}");
                self.session().insert(name.to_owned(), secret.to_owned());
                Kept::Session(reason)
            }
        }
    }

    /// The secret, None when never stored.
    pub fn get(&self, name: &str) -> Result<Option<String>> {
        if let Some(secret) = self.session().get(name) {
            return Ok(Some(secret.clone()));
        }
        self.backend
            .get(name)
            .map_err(|reason| Error::Vault(format!("reading {name}: {reason}")))
    }

    /// A refused delete keeps the session copy too.
    pub fn remove(&self, name: &str) -> Result<()> {
        self.backend
            .delete(name)
            .map_err(|reason| Error::Vault(format!("deleting {name}: {reason}")))?;
        self.session().remove(name);
        Ok(())
    }

    fn session(&self) -> std::sync::MutexGuard<'_, HashMap<String, String>> {
        self.session.lock().expect("vault lock")
    }
}
