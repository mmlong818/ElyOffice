//! The fields every app reads from the shared file.

use std::io;

use serde_json::{Map, Value};

use super::Store;

/// Light, dark, or as the system is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Appearance {
    #[default]
    System,
    Light,
    Dark,
}

/// The interface language; System follows the OS.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Language {
    #[default]
    System,
    English,
    Chinese,
}

/// Interface motion; System follows the OS setting.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Motion {
    #[default]
    System,
    Reduced,
    Full,
}

pub const APPEARANCE: &str = "appearance";
pub const LANGUAGE: &str = "language";
pub const MOTION: &str = "motion";
pub const OMARCHY_SYNC: &str = "omarchy_sync";
pub const AUTHOR: &str = "author";
pub const CHECK_UPDATES: &str = "check_updates";

/// The shared settings; absent fields take their defaults.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shared {
    pub appearance: Appearance,
    pub language: Language,
    pub motion: Motion,
    pub omarchy_sync: bool,
    /// Signs comments and changes; empty means the account name.
    pub author: String,
    /// Asks for a newer release at launch; off unless chosen.
    pub check_updates: bool,
}

impl Default for Shared {
    fn default() -> Self {
        Self {
            appearance: Appearance::System,
            language: Language::System,
            motion: Motion::System,
            omarchy_sync: true,
            author: String::new(),
            check_updates: false,
        }
    }
}

impl Appearance {
    pub const ALL: [(Self, &'static str); 3] = [
        (Self::System, "system"),
        (Self::Light, "light"),
        (Self::Dark, "dark"),
    ];
}

impl Language {
    pub const ALL: [(Self, &'static str); 3] = [
        (Self::System, "system"),
        (Self::English, "en-US"),
        (Self::Chinese, "zh-CN"),
    ];
}

impl Motion {
    pub const ALL: [(Self, &'static str); 3] = [
        (Self::System, "system"),
        (Self::Reduced, "reduce"),
        (Self::Full, "full"),
    ];
}

/// The stored word for `choice` among `all`.
pub fn word<T: PartialEq>(all: &[(T, &'static str)], choice: &T) -> &'static str {
    all.iter()
        .find(|(each, _)| each == choice)
        .map(|(_, word)| *word)
        .expect("every choice has a word")
}

impl Shared {
    /// The shared file as read now.
    pub fn load() -> io::Result<Self> {
        Self::from_fields(&Store::shared()?.read()?)
    }

    /// Known fields, typed; a wrong value is an error.
    pub fn from_fields(fields: &Map<String, Value>) -> io::Result<Self> {
        let defaults = Self::default();
        Ok(Self {
            appearance: pick(fields, APPEARANCE, &Appearance::ALL)?.unwrap_or(defaults.appearance),
            language: pick(fields, LANGUAGE, &Language::ALL)?.unwrap_or(defaults.language),
            motion: pick(fields, MOTION, &Motion::ALL)?.unwrap_or(defaults.motion),
            check_updates: match fields.get(CHECK_UPDATES) {
                None => defaults.check_updates,
                Some(Value::Bool(on)) => *on,
                Some(other) => return Err(wrong(CHECK_UPDATES, other)),
            },
            omarchy_sync: match fields.get(OMARCHY_SYNC) {
                None => defaults.omarchy_sync,
                Some(Value::Bool(on)) => *on,
                Some(other) => return Err(wrong(OMARCHY_SYNC, other)),
            },
            author: match fields.get(AUTHOR) {
                None => defaults.author,
                Some(Value::String(name)) => name.trim().to_string(),
                Some(other) => return Err(wrong(AUTHOR, other)),
            },
        })
    }
}

fn pick<T: Copy>(
    fields: &Map<String, Value>,
    key: &str,
    all: &[(T, &'static str)],
) -> io::Result<Option<T>> {
    match fields.get(key) {
        None => Ok(None),
        Some(Value::String(word)) => match all.iter().find(|(_, each)| each == word) {
            Some((choice, _)) => Ok(Some(*choice)),
            None => Err(wrong(key, &Value::String(word.clone()))),
        },
        Some(other) => Err(wrong(key, other)),
    }
}

fn wrong(key: &str, value: &Value) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!("settings: {key} cannot be {value}"),
    )
}
