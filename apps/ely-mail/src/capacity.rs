//! How much downloaded mail is kept (§18.1, §18.3).

use ely_gpui_component::{
    forms::{Choice, Select},
    settings::SettingsRow,
    theme::ControlSize,
};
use ely_platform::settings::Store as Settings;
use gpui::{AnyElement, App, Context, IntoElement, WeakEntity};
use serde_json::Value;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

use crate::mail::{Mail, text};

/// The caps offered, in GB.
pub const CAPS: [u64; 3] = [1, 5, 20];
const KEY: &str = "mail_cache";
const GB: u64 = 1 << 30;

/// The cap and its latest change.
#[derive(Default)]
pub struct Cap {
    /// None until read; a bad saved value says why.
    pub gb: Option<Result<Option<u64>, String>>,
    /// Shared with writes and trims: stale ones stop.
    turn: Arc<AtomicU64>,
}

/// Absent or null is no cap; else one offered.
pub fn parse(saved: Option<&Value>) -> Result<Option<u64>, String> {
    match saved {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Number(n)) => match n.as_u64().filter(|gb| CAPS.contains(gb)) {
            Some(gb) => Ok(Some(gb)),
            None => Err(format!("{KEY} cannot be {n}")),
        },
        Some(other) => Err(format!("{KEY} cannot be {other}")),
    }
}

impl Mail {
    /// Reads the saved cap off the window thread.
    pub(crate) fn read_cap(&mut self, cx: &mut Context<Self>) {
        let settings = Settings::at(self.settings.clone());
        let read = cx.background_executor().spawn(async move {
            settings.read().map_err(|error| error.to_string()).and_then(|saved| parse(saved.get(KEY)))
        });
        cx.spawn(async move |this, cx| {
            let gb = read.await;
            if let Err(error) = &gb {
                log::error!("mail: the download cap is unreadable: {error}");
            }
            this.update(cx, |mail, cx| {
                mail.cap.gb = Some(gb);
                cx.refresh_windows();
            })
            .ok();
        })
        .detach();
    }

    /// The cap in bytes; unreadable counts as none.
    pub(crate) fn cap_bytes(&self) -> Option<u64> {
        match &self.cap.gb {
            Some(Ok(Some(gb))) => Some(gb * GB),
            _ => None,
        }
    }

    /// Takes a cap, trims to it, writes the latest.
    pub(crate) fn set_cap(&mut self, gb: Option<u64>, cx: &mut Context<Self>) {
        log::info!("mail: download cap {gb:?} GB");
        self.cap.gb = Some(Ok(gb));
        let turn = self.cap.turn.fetch_add(1, Ordering::SeqCst) + 1;
        let (latest, settings) = (self.cap.turn.clone(), Settings::at(self.settings.clone()));
        let value = gb.map_or(Value::Null, Value::from);
        // Only the latest choice is written.
        let written = cx
            .background_executor()
            .spawn(async move { settings.set_if(KEY, value, || latest.load(Ordering::SeqCst) == turn) });
        let latest = self.cap.turn.clone();
        cx.spawn(async move |this, cx| {
            let written = written.await;
            this.update(cx, |mail, cx| match written {
                Err(error) if latest.load(Ordering::SeqCst) == turn => {
                    mail.failed("mail.storage.cap_unsaved", &error.to_string(), cx)
                }
                Err(_) => log::info!("mail: an older cap failed to save; a newer one follows"),
                Ok(_) => {}
            })
            .ok();
        })
        .detach();
        if let Some(cap) = self.cap_bytes() {
            self.trim_to(cap, cx);
        }
        cx.refresh_windows();
    }

    /// The oldest downloads to headers, until the rest fit.
    pub(crate) fn trim_to(&mut self, cap: u64, cx: &mut Context<Self>) {
        let (latest, turn) = (self.cap.turn.clone(), self.cap.turn.load(Ordering::SeqCst));
        // A newer cap stops this trim; its own trim follows.
        let trim = move |store: &mut ely_mail_engine::Store| {
            store.trim_downloads(cap, || latest.load(Ordering::SeqCst) == turn)
        };
        self.with_store(cx, trim, |mail, trimmed, cx| {
            if let Err(error) = trimmed {
                return mail.failed("mail.storage.untrimmed", &error, cx);
            }
            mail.read_sizes(cx);
            mail.read_letter(cx);
        });
    }

    pub(crate) fn cap_row(&self, this: &WeakEntity<Mail>, cx: &App) -> AnyElement {
        let row = SettingsRow::new(text(cx, "mail.storage.cap", &[]));
        let gb = match &self.cap.gb {
            None => return row.into_any_element(),
            Some(Err(error)) => {
                let words = text(cx, "mail.storage.cap_unread", &[("error", error)]);
                return row.description(words).into_any_element();
            }
            Some(Ok(gb)) => *gb,
        };
        let mut choices = vec![Choice::new("none", text(cx, "mail.storage.cap_none", &[]))];
        choices.extend(CAPS.map(|gb| Choice::new(gb.to_string(), format!("{gb} GB"))));
        let picked = gb.map_or("none".to_owned(), |gb| gb.to_string());
        let this = this.clone();
        let select = Select::new("mail-storage-cap", choices)
            .label(text(cx, "mail.storage.cap", &[]))
            .selected(picked)
            .size(ControlSize::Sm)
            .on_change(move |value, _, cx| {
                let gb = value.parse::<u64>().ok();
                this.update(cx, |mail, cx| mail.set_cap(gb, cx)).ok();
            });
        row.description(text(cx, "mail.storage.cap_hint", &[])).control(select).into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::parse;

    #[test]
    fn a_cap_reads_strictly() {
        assert_eq!(parse(None), Ok(None));
        assert_eq!(parse(Some(&json!(null))), Ok(None));
        assert_eq!(parse(Some(&json!(5))), Ok(Some(5)));
        for bad in [json!(3), json!(-1), json!("5"), json!(true), json!(1.5)] {
            assert!(parse(Some(&bad)).is_err(), "{bad}");
        }
    }
}
