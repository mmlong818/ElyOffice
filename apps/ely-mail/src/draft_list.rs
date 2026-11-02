//! The Drafts list and a picked draft (C12.13).

use ely_gpui_component::{
    buttons::{Button, ButtonVariant},
    data_display::DescriptionList,
    feedback::EmptyState,
    forms::Switch,
    layout::ScrollArea,
    mail::Mail as Row,
    overlays::ConfirmDialog,
    primitives::IconName,
};
use ely_mail_engine::{AccountId, DraftId, Protocol, Shelved};
use gpui::{
    AnyElement, Context, InteractiveElement as _, IntoElement, ParentElement as _, SharedString,
    Styled as _, div,
};

use crate::{
    boxes::Open,
    drafts::holder,
    mail::{Mail, text},
    view::local_time,
};

/// A list key for a draft, apart from messages.
pub fn key(id: DraftId) -> SharedString {
    format!("draft-{}", id.0).into()
}

/// Recipients stand where a sender would.
pub fn rows(drafts: &[Shelved], none: &(SharedString, SharedString)) -> Vec<Row> {
    drafts
        .iter()
        .map(|draft| {
            let to = draft.unsent.to.join(", ");
            let to: SharedString = match to.is_empty() {
                true => none.0.clone(),
                false => to.into(),
            };
            let subject: SharedString = match draft.unsent.subject.is_empty() {
                true => none.1.clone(),
                false => draft.unsent.subject.clone().into(),
            };
            let row = match jiff::Timestamp::from_second(draft.saved) {
                Ok(at) => Row::new(key(draft.id), to, subject, at),
                Err(error) => {
                    log::warn!("mail: draft {} time unshown: {error}", draft.id.0);
                    Row::undated(key(draft.id), to, subject)
                }
            };
            let first = draft
                .unsent
                .words
                .lines()
                .find(|line| !line.trim().is_empty());
            match first {
                Some(line) => row.snippet(line.trim().to_owned()),
                None => row,
            }
        })
        .collect()
}

impl Mail {
    /// The list shows drafts; it reads them again.
    pub fn drafts_changed(&mut self, cx: &mut Context<Self>) {
        if self.open == Open::Drafts {
            self.read_mail(cx);
        }
    }

    fn picked_draft(&self) -> Option<&Shelved> {
        let key = self.listing.chosen.as_ref()?.key.clone();
        self.shelved.iter().find(|draft| self::key(draft.id) == key)
    }

    pub fn draft_pane(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(draft) = self.picked_draft() else {
            let words = text(cx, "mail.drafts.pick", &[]);
            return EmptyState::new("mail-draft-none", IconName::File, words).into_any_element();
        };
        let label = |key: &str| text(cx, key, &[]);
        let from = self
            .accounts()
            .iter()
            .find(|(id, _)| *id == draft.account)
            .map_or_else(String::new, |(_, account)| account.address.clone());
        let mut details = DescriptionList::new()
            .text(label("mail.write.from"), from)
            .text(label("mail.drafts.to"), draft.unsent.to.join(", "));
        for (key, list) in [
            ("mail.drafts.cc", &draft.unsent.cc),
            ("mail.drafts.bcc", &draft.unsent.bcc),
        ] {
            if !list.is_empty() {
                details = details.text(label(key), list.join(", "));
            }
        }
        let details = details.text(label("mail.drafts.saved"), local_time(draft.saved));
        let id = draft.id;
        let (open, delete) = (cx.weak_entity(), cx.weak_entity());
        let buttons = div()
            .flex()
            .gap_2()
            .child(
                Button::new("mail-draft-open", label("mail.drafts.continue"))
                    .variant(ButtonVariant::Primary)
                    .on_click(move |_, _, cx| {
                        open.update(cx, |mail, cx| mail.reopen(id, cx)).ok();
                    }),
            )
            .child(
                Button::new("mail-draft-delete", label("mail.drafts.delete")).on_click(
                    move |_, _, cx| {
                        delete.update(cx, |mail, cx| mail.ask_delete(id, cx)).ok();
                    },
                ),
            );
        let words = div()
            .debug_selector(|| "mail-draft-words".into())
            .whitespace_normal()
            .children(
                draft
                    .unsent
                    .words
                    .lines()
                    .map(|line| div().child(line.to_owned())),
            );
        let page = div()
            .p_6()
            .flex()
            .flex_col()
            .gap_4()
            .child(details)
            .child(buttons)
            .child(words);
        ScrollArea::new("mail-draft-scroll")
            .vertical()
            .size_full()
            .child(page)
            .into_any_element()
    }

    /// A window that holds it asks there.
    pub fn ask_delete(&mut self, id: DraftId, cx: &mut Context<Self>) {
        if let Some(compose) = holder(id, cx) {
            compose
                .update(cx, |compose, window, cx| {
                    window.activate_window();
                    compose.discard(window, cx);
                })
                .ok();
            return;
        }
        self.deleting = Some(id);
        cx.notify();
    }

    /// A window opened meanwhile drops it and closes.
    fn confirm_delete(&mut self, id: DraftId, cx: &mut Context<Self>) {
        if let Some(compose) = holder(id, cx) {
            log::info!("mail: draft {} goes with its window", id.0);
            // The window drops it through Mail, busy here.
            cx.defer(move |cx| {
                compose
                    .update(cx, |compose, window, cx| {
                        compose.confirm_discard(window, cx)
                    })
                    .ok();
            });
            return;
        }
        if self.reopening.contains(&id) {
            self.unwanted.insert(id);
            return;
        }
        self.drop_draft(id, None, cx);
    }

    pub fn delete_dialog(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        // The close clears it before the confirm runs.
        let id = self.deleting?;
        let (cancel, confirm) = (cx.weak_entity(), cx.weak_entity());
        let dialog = ConfirmDialog::new(
            "mail-draft-delete-dialog",
            text(cx, "mail.write.discard.title", &[]),
            text(cx, "mail.write.discard.message", &[]),
            move |_, cx| {
                cancel
                    .update(cx, |mail, cx| {
                        mail.deleting = None;
                        cx.notify();
                    })
                    .ok();
            },
        )
        .confirm(text(cx, "mail.write.discard.go", &[]))
        .destructive()
        .on_confirm(move |_, cx| {
            confirm
                .update(cx, |mail, cx| mail.confirm_delete(id, cx))
                .ok();
        });
        Some(dialog.into_any_element())
    }

    /// Reads whether this account's drafts go up.
    pub fn read_drafts_up(&mut self, id: AccountId, cx: &mut Context<Self>) {
        self.drafts_up = None;
        self.with_store(
            cx,
            move |store| store.drafts_up(id),
            move |mail, read, cx| match read {
                Ok(up) if mail.open == Open::Account(id) => mail.drafts_up = Some((id, up)),
                Ok(_) => {}
                Err(error) => mail.failed("mail.drafts.unread", &error, cx),
            },
        );
    }

    pub fn drafts_switch(
        &self,
        id: AccountId,
        protocol: Protocol,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        if protocol != Protocol::Imap {
            return None;
        }
        let (shown, up) = self.drafts_up?;
        if shown != id {
            return None;
        }
        let this = cx.weak_entity();
        let switch = Switch::new("mail-drafts-up", up)
            .label(text(cx, "mail.drafts.up", &[]))
            .on_change(move |on, _, cx| {
                this.update(cx, |mail, cx| mail.set_drafts_up(id, on, cx))
                    .ok();
            });
        Some(
            div()
                .debug_selector(|| "mail-drafts-up".into())
                .child(switch)
                .into_any_element(),
        )
    }

    fn set_drafts_up(&mut self, id: AccountId, up: bool, cx: &mut Context<Self>) {
        self.with_store(
            cx,
            move |store| store.set_drafts_up(id, up),
            move |mail, done, cx| match done {
                Ok(()) if mail.open == Open::Account(id) => mail.drafts_up = Some((id, up)),
                Ok(()) => {}
                Err(error) => mail.failed("mail.drafts.unsaved_choice", &error, cx),
            },
        );
    }
}
