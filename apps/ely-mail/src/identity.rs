//! An account's sender name and signature, on its page (C12.12).

use ely_gpui_component::{
    buttons::{Button, ButtonVariant},
    feedback::Alert,
    forms::{Input, TextInput},
    mail::SignatureEditor,
    primitives::Severity,
};
use ely_mail_engine::{AccountId, Identity};
use gpui::{
    AnyElement, Context, InteractiveElement as _, IntoElement, ParentElement as _, Styled as _,
    Window, div,
};

use crate::mail::{Mail, text};

/// The open account's identity, read once per visit.
pub struct Editing {
    pub account: AccountId,
    /// Numbers the read; fields start over with it.
    pub read: u64,
    pub kept: Option<Result<Identity, String>>,
    pub signature: String,
}

impl Mail {
    pub fn read_identity(&mut self, account: AccountId, cx: &mut Context<Self>) {
        self.asked += 1;
        let read = self.asked;
        self.identity = Some(Editing {
            account,
            read,
            kept: None,
            signature: String::new(),
        });
        self.with_store(
            cx,
            move |store| store.identity(account),
            move |mail, kept, cx| {
                let Some(editing) = mail.identity.as_mut().filter(|each| each.read == read) else {
                    return log::info!("mail: a stale identity read was dropped");
                };
                if let Ok(identity) = &kept {
                    editing.signature = identity.signature.clone();
                }
                editing.kept = Some(kept);
                cx.notify();
            },
        );
    }

    pub fn save_identity(
        &mut self,
        account: AccountId,
        identity: Identity,
        cx: &mut Context<Self>,
    ) {
        self.with_store(
            cx,
            move |store| store.set_identity(account, &identity),
            |mail, saved, cx| match saved {
                Ok(()) => {
                    let words = text(cx, "mail.identity.saved", &[]);
                    mail.toast(words, Severity::Success, cx);
                }
                Err(error) => mail.failed("mail.identity.unsaved", &error, cx),
            },
        );
    }

    pub fn identity_editor(
        &self,
        account: AccountId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let editing = self
            .identity
            .as_ref()
            .filter(|each| each.account == account)?;
        let kept = match editing.kept.as_ref()? {
            Ok(kept) => kept,
            Err(error) => {
                let words = text(cx, "mail.identity.unread", &[("error", error)]);
                let alert = Alert::new("mail-identity-unread", Severity::Danger, words);
                return Some(alert.into_any_element());
            }
        };
        let read = editing.read;
        let named = kept.name.clone().unwrap_or_default();
        let label = text(cx, "mail.identity.name", &[]);
        let name = window.use_keyed_state(("mail-identity-name", read), cx, |window, cx| {
            let mut input = TextInput::new(window, cx).label(label.clone());
            input.set_placeholder(label, cx);
            input.set_text(named, cx);
            input
        });
        let this = cx.weak_entity();
        let signature =
            SignatureEditor::new(("mail-identity-signature", read), editing.signature.clone())
                .on_change({
                    let this = this.clone();
                    move |words, _, cx| {
                        this.update(cx, |mail, _| {
                            if let Some(editing) =
                                mail.identity.as_mut().filter(|each| each.read == read)
                            {
                                editing.signature = words.to_string();
                            }
                        })
                        .ok();
                    }
                });
        let field = name.clone();
        let save = Button::new("mail-identity-save", text(cx, "mail.identity.save", &[]))
            .variant(ButtonVariant::Primary)
            .on_click(move |_, _, cx| {
                let name = field.read(cx).text().to_string();
                this.update(cx, |mail, cx| {
                    let Some(editing) = mail.identity.as_ref().filter(|each| each.read == read)
                    else {
                        return;
                    };
                    let identity = Identity {
                        name: Some(name),
                        signature: editing.signature.clone(),
                    };
                    mail.save_identity(account, identity, cx);
                })
                .ok();
            });
        let part = div()
            .debug_selector(|| "mail-identity".into())
            .flex()
            .flex_col()
            .gap_3()
            .child(Input::new(&name))
            .child(signature)
            .child(div().child(save));
        Some(part.into_any_element())
    }
}
