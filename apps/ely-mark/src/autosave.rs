//! ElyMark saves a second after typing stops (§6.2).

use std::time::{Duration, Instant};

use ely_files::{Action, DEBOUNCE, SaveState};
use ely_office::{in_window, prelude::*};
use gpui::{Context, Window};

use crate::desk::Desk;
use crate::outside::Outside;

impl Desk {
    /// A page change restarts the wait; finished saves report.
    pub fn autosave(&mut self, id: u64, changed: u64, cx: &mut Context<Self>) {
        let Some(tab) = self.tab_mut(id) else {
            return;
        };
        let saving = tab.exporting || tab.session.state() == SaveState::Saving;
        if let Some(revision) = tab.extra.writing.filter(|_| !saving) {
            let ok = tab.session.state() != SaveState::SaveFailed;
            tab.extra.policy.save_finished(revision, ok);
            tab.extra.writing = None;
        }
        if tab.session.path().is_some() {
            tab.extra.policy.set_has_path();
        }
        // A page reload restarts its count: any difference is news.
        if changed == tab.extra.heard {
            return;
        }
        tab.extra.heard = changed;
        self.save_after_pause(id, cx);
    }

    /// Counts unsaved text as an edit; saves after the pause.
    pub fn save_after_pause(&mut self, id: u64, cx: &mut Context<Self>) {
        let Some(tab) = self.tab_mut(id) else {
            return;
        };
        tab.extra.policy.edit(Instant::now());
        cx.spawn(async move |desk, cx| {
            let wait = DEBOUNCE + Duration::from_millis(50);
            cx.background_executor().timer(wait).await;
            in_window(&desk, cx, |desk, window, cx| {
                desk.autosave_due(id, window, cx)
            })
            .ok();
        })
        .detach();
    }

    fn autosave_due(&mut self, id: u64, window: &mut Window, cx: &mut Context<Self>) {
        let Some(tab) = self.tab_mut(id) else {
            return;
        };
        if !matches!(tab.extra.outside, Outside::Clear) || tab.extra.writing.is_some() {
            return;
        }
        let Action::SaveDue(revision) = tab.extra.policy.tick(Instant::now()) else {
            return;
        };
        if !tab.dirty(cx) {
            return tab.extra.policy.save_finished(revision, true);
        }
        log::info!("mark: autosaves tab {id}");
        tab.extra.writing = Some(revision);
        self.save(id, None, window, cx);
    }
}
