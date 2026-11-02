//! Ordinary files save themselves when chosen (§6.2).

use std::time::Duration;

use ely_files::SaveState;
use gpui::{AsyncApp, Context, WeakEntity};

use crate::{Engine, Host, Saving as _, in_window};

/// How long typing rests before a save.
const QUIET: Duration = Duration::from_secs(2);

pub trait Autosaving: Host {
    /// From the app's window, never its tests.
    fn start_autosave(cx: &mut Context<Self>) {
        cx.spawn(async move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            loop {
                cx.background_executor().timer(QUIET).await;
                if in_window(&this, cx, |host, window, cx| host.autosave(window, cx)).is_err() {
                    break;
                }
            }
        })
        .detach();
    }

    /// Saves each eligible tab whose edits rested a whole beat.
    fn autosave(&mut self, window: &mut gpui::Window, cx: &mut Context<Self>) {
        let on = ely_ui::settings::switches::on(ely_ui::settings::switches::AUTOSAVE, cx);
        let core = self.core();
        if !on || core.stopped.is_some() || core.dialog.is_some() {
            self.core_mut().quiet.clear();
            return;
        }
        let edits: Vec<(u64, u64)> = core
            .tabs
            .iter()
            .filter(|tab| {
                let ours = tab.session.path().and_then(|path| path.extension()).and_then(|ext| ext.to_str()).is_some_and(
                    |ext| Self::KIND.save_as.iter().any(|format| format.eq_ignore_ascii_case(ext)),
                );
                ours && tab.risk.is_none()
                    && !tab.exporting
                    && tab.session.state() == SaveState::Dirty
                    && Self::Engine::settled(tab.view.read(cx))
            })
            .map(|tab| (tab.id, Self::Engine::edited(tab.view.read(cx))))
            .collect();
        let due: Vec<u64> = edits
            .iter()
            .filter(|(id, edited)| self.core().quiet.get(id) == Some(edited))
            .map(|(id, _)| *id)
            .collect();
        self.core_mut().quiet = edits.into_iter().filter(|(id, _)| !due.contains(id)).collect();
        for id in due {
            log::info!("office: autosaving tab {id}");
            self.save(id, None, window, cx);
        }
    }
}

impl<H: Host> Autosaving for H {}
