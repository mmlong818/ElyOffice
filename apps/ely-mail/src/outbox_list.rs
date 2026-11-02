//! The Outbox list, a picked message, and quitting (C12.14).

use ely_gpui_component::{
    buttons::{Button, ButtonVariant},
    data_display::DescriptionList,
    feedback::{Alert, EmptyState},
    forms::Switch,
    layout::ScrollArea,
    mail::Mail as Row,
    overlays::ConfirmDialog,
    primitives::{IconName, Severity},
};
use ely_mail_engine::{Account, AccountId, Filing, Going, OutboxId, Protocol, Queued};
use gpui::{
    AnyElement, App, Context, InteractiveElement as _, IntoElement, ParentElement as _,
    SharedString, Styled as _, div,
};

use crate::{
    boxes::Open,
    mail::{Mail, text},
    outbox::copies,
    view::local_time,
};

/// A list key for a queued message, apart from others.
pub fn key(id: OutboxId) -> SharedString {
    format!("out-{}", id.0).into()
}

/// What the user may do with it now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Act {
    Retry,
    Cancel,
    Edit,
    Forget,
}

/// Where it stands, in the reader's words.
pub fn state(queued: &Queued, cx: &App) -> SharedString {
    let error = queued.error.as_deref();
    let said = |key: &str| text(cx, key, &[("error", error.unwrap_or_default())]);
    match (queued.going, queued.filing, error) {
        (Going::Queued, _, None) => said("mail.outbox.queued"),
        (Going::Queued, _, Some(_)) => said("mail.outbox.waiting"),
        (Going::Sending, _, _) => said("mail.outbox.sending"),
        (Going::Accepted, Some(Filing::Failed), _) => said("mail.outbox.unfiled"),
        (Going::Accepted, _, Some(_)) => said("mail.outbox.filing_later"),
        (Going::Accepted, _, None) => said("mail.outbox.accepted"),
        (Going::Failed, _, _) => said("mail.outbox.failed"),
        (Going::Unsure, _, None) => said("mail.outbox.stopped"),
        (Going::Unsure, _, Some(_)) => said("mail.outbox.unsure"),
        (Going::Cancelled, _, _) => said("mail.outbox.cancelled"),
    }
}

/// Recipients stand where a sender would.
pub fn rows(queued: &[Queued], none: &(SharedString, SharedString), cx: &App) -> Vec<Row> {
    queued
        .iter()
        .map(|each| {
            let to = each.unsent.to.join(", ");
            let to: SharedString = match to.is_empty() {
                true => none.0.clone(),
                false => to.into(),
            };
            let subject: SharedString = match each.unsent.subject.is_empty() {
                true => none.1.clone(),
                false => each.unsent.subject.clone().into(),
            };
            let row = match jiff::Timestamp::from_second(each.queued) {
                Ok(at) => Row::new(key(each.id), to, subject, at),
                Err(error) => {
                    log::warn!("mail: queued {} time unshown: {error}", each.id.0);
                    Row::undated(key(each.id), to, subject)
                }
            };
            row.snippet(state(each, cx).to_string())
        })
        .collect()
}

/// The buttons a state allows, with their words.
pub fn acts(queued: &Queued) -> Vec<(Act, &'static str)> {
    match (queued.going, queued.filing, &queued.error) {
        (Going::Queued, _, None) => vec![(Act::Cancel, "mail.outbox.cancel")],
        (Going::Queued, _, Some(_)) => vec![
            (Act::Retry, "mail.outbox.retry"),
            (Act::Cancel, "mail.outbox.cancel"),
        ],
        (Going::Sending, _, _) | (Going::Accepted, Some(Filing::Waiting), None) => Vec::new(),
        (Going::Accepted, Some(Filing::Waiting), Some(_)) => {
            vec![(Act::Retry, "mail.outbox.file_again")]
        }
        (Going::Accepted, _, _) => vec![
            (Act::Retry, "mail.outbox.file_again"),
            (Act::Forget, "mail.outbox.forget"),
        ],
        (Going::Failed, _, _) => vec![
            (Act::Retry, "mail.outbox.retry"),
            (Act::Edit, "mail.outbox.edit"),
            (Act::Forget, "mail.outbox.forget"),
        ],
        (Going::Unsure, _, _) => vec![
            (Act::Forget, "mail.outbox.forget"),
            (Act::Edit, "mail.outbox.edit"),
            (Act::Retry, "mail.outbox.send_again"),
        ],
        (Going::Cancelled, _, _) => vec![
            (Act::Retry, "mail.outbox.send"),
            (Act::Edit, "mail.outbox.edit"),
            (Act::Forget, "mail.outbox.forget"),
        ],
    }
}

impl Mail {
    fn picked_queued(&self) -> Option<&Queued> {
        let key = self.listing.chosen.as_ref()?.key.clone();
        self.queued.iter().find(|each| self::key(each.id) == key)
    }

    pub fn outbox_pane(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(queued) = self.picked_queued() else {
            let words = text(cx, "mail.outbox.pick", &[]);
            return EmptyState::new("mail-outbox-none", IconName::Send, words).into_any_element();
        };
        let label = |key: &str| text(cx, key, &[]);
        let from = self
            .accounts()
            .iter()
            .find(|(id, _)| *id == queued.account)
            .map_or_else(String::new, |(_, account)| account.address.clone());
        let mut details = DescriptionList::new()
            .text(label("mail.write.from"), from)
            .text(label("mail.drafts.to"), queued.unsent.to.join(", "))
            .text(label("mail.outbox.state"), state(queued, cx))
            .text(label("mail.outbox.queued_at"), local_time(queued.queued));
        if let Some(tried) = queued.tried {
            details = details.text(label("mail.outbox.tried_at"), local_time(tried));
        }
        let help = match queued.going {
            Going::Unsure => Some(("mail.outbox.unsure_help", Severity::Warning)),
            Going::Accepted => Some(("mail.outbox.accepted_help", Severity::Info)),
            _ => None,
        };
        if help.is_some() {
            let id = format!("<{}>", queued.message_id);
            details = details.text(label("mail.outbox.message_id"), id);
        }
        let help = help.map(|(key, severity)| Alert::new("mail-outbox-help", severity, label(key)));
        let id = queued.id;
        let buttons = acts(queued)
            .into_iter()
            .enumerate()
            .map(|(at, (act, key))| {
                let this = cx.weak_entity();
                let button =
                    Button::new(("mail-outbox-act", at), label(key)).on_click(move |_, _, cx| {
                        this.update(cx, |mail, cx| mail.outbox_act(id, act, cx))
                            .ok();
                    });
                match at {
                    0 => button.variant(ButtonVariant::Primary),
                    _ => button,
                }
            });
        let page = div()
            .p_6()
            .flex()
            .flex_col()
            .gap_4()
            .child(details)
            .children(help)
            .child(
                div()
                    .debug_selector(|| "mail-outbox-acts".into())
                    .flex()
                    .gap_2()
                    .children(buttons),
            );
        ScrollArea::new("mail-outbox-scroll")
            .vertical()
            .size_full()
            .child(page)
            .into_any_element()
    }

    /// Edit opens it as a draft; Retry starts a round.
    pub fn outbox_act(&mut self, id: OutboxId, act: Act, cx: &mut Context<Self>) {
        log::info!("mail: queued {} asked {act:?}", id.0);
        self.with_store(
            cx,
            move |store| match act {
                Act::Retry => store.retry(id).map(|()| None),
                Act::Cancel => store.cancel(id).map(|()| None),
                Act::Forget => store.forget(id).map(|()| None),
                Act::Edit => store.unqueue(id).map(Some),
            },
            move |mail, done, cx| {
                match done {
                    Ok(Some(draft)) => mail.reopen(draft, cx),
                    Ok(None) if act == Act::Retry => mail.run_outbox(true, cx),
                    Ok(None) => {}
                    Err(error) => mail.failed("mail.outbox.unsaved", &error, cx),
                }
                mail.read_mail(cx);
            },
        );
    }

    /// False asks first while the Outbox has work.
    pub fn may_quit(&mut self, cx: &mut Context<Self>) -> bool {
        if self.pending == 0 || self.quit_anyway {
            return true;
        }
        log::info!("mail: {} queued messages hold the quit", self.pending);
        self.quitting = true;
        // From the tray: the question needs the window.
        cx.defer(|cx| {
            ely_ui::tray::show_main(cx);
        });
        cx.notify();
        false
    }

    pub fn quit_dialog(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.quitting {
            return None;
        }
        let n = self.pending as u64;
        let mut message =
            ely_ui::i18n::counted(cx, "mail.outbox.quit.message", n, &[("n", &n.to_string())])
                .to_string();
        if self.runner.sending.is_some() {
            message = format!("{message} {}", text(cx, "mail.outbox.quit.sending", &[]));
        }
        let (stay, go) = (cx.weak_entity(), cx.weak_entity());
        let dialog = ConfirmDialog::new(
            "mail-outbox-quit",
            text(cx, "mail.outbox.quit.title", &[]),
            message,
            move |_, cx| {
                stay.update(cx, |mail, cx| {
                    mail.quitting = false;
                    cx.notify();
                })
                .ok();
            },
        )
        .confirm(text(cx, "mail.outbox.quit.go", &[]))
        .on_confirm(move |_, cx| {
            go.update(cx, |mail, _| mail.quit_anyway = true).ok();
            ely_ui::commands::quit(cx);
        });
        Some(dialog.into_any_element())
    }

    /// Reads whether this account files its own copy.
    pub fn read_sent_copy(&mut self, id: AccountId, cx: &mut Context<Self>) {
        self.sent_copy = None;
        self.with_store(
            cx,
            move |store| store.sent_copy(id),
            move |mail, read, cx| match read {
                Ok(copy) if mail.open == Open::Account(id) => mail.sent_copy = Some((id, copy)),
                Ok(_) => {}
                Err(error) => mail.failed("mail.outbox.unread", &error, cx),
            },
        );
    }

    pub fn sent_copy_switch(
        &self,
        id: AccountId,
        account: &Account,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        if account.protocol != Protocol::Imap {
            return None;
        }
        let (shown, choice) = self.sent_copy?;
        if shown != id {
            return None;
        }
        let on = copies(account, choice);
        let provider = copies(account, None);
        let this = cx.weak_entity();
        let switch = Switch::new("mail-sent-copy", on)
            .label(text(cx, "mail.outbox.copy", &[]))
            .on_change(move |on, _, cx| {
                this.update(cx, |mail, cx| mail.set_sent_copy(id, on, cx))
                    .ok();
            });
        let note = (!provider).then(|| text(cx, "mail.outbox.copy_note", &[]));
        Some(
            div()
                .debug_selector(|| "mail-sent-copy".into())
                .flex()
                .flex_col()
                .gap_1()
                .child(switch)
                .children(note.map(|note| div().text_sm().child(note)))
                .into_any_element(),
        )
    }

    fn set_sent_copy(&mut self, id: AccountId, on: bool, cx: &mut Context<Self>) {
        self.with_store(
            cx,
            move |store| store.set_sent_copy(id, Some(on)),
            move |mail, done, cx| match done {
                Ok(()) if mail.open == Open::Account(id) => mail.sent_copy = Some((id, Some(on))),
                Ok(()) => {}
                Err(error) => mail.failed("mail.outbox.unsaved_choice", &error, cx),
            },
        );
    }
}
