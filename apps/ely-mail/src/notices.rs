//! New mail as system notifications (MAIL-15).

use ely_mail_engine::{AccountId, Summary};
use ely_ui::settings::switches::{self, NOTIFY};
use gpui::{Context, SystemNotification};

use crate::{
    boxes::Open,
    mail::{Mail, text},
};

/// Tags of new-mail notices start so.
const TAG: &str = "mail-new-";
/// Senders named before "and others".
const NAMED: usize = 3;

impl Mail {
    /// Clicking a notice brings the inboxes forward.
    pub(crate) fn answer_notices(cx: &mut Context<Self>) {
        let this = cx.weak_entity();
        cx.on_system_notification_response(move |response, cx| {
            if !response.tag.starts_with(TAG) {
                return;
            }
            log::info!("mail: a notice was opened");
            ely_ui::tray::show_main(cx);
            this.update(cx, |mail, cx| {
                // A search or filter may hide the new mail.
                mail.stop_seeking(cx);
                mail.filter = ely_mail_engine::Filter::default();
                mail.open_box(Open::Inboxes, cx);
            })
            .ok();
        });
    }

    /// Tells of `arrived`, unless muted or the window is front.
    pub(crate) fn announce(&mut self, id: AccountId, arrived: Vec<Summary>, cx: &mut Context<Self>) {
        let count = arrived.len();
        if count == 0 {
            return;
        }
        if !switches::on(NOTIFY, cx) {
            return log::info!("mail: {count} new, notifications are off");
        }
        if ely_ui::tray::main_in_front(cx) {
            return log::info!("mail: {count} new, the window is in front");
        }
        let mut senders: Vec<String> = Vec::new();
        for from in arrived.iter().flat_map(|each| each.from.first()) {
            let name = from.name.clone().unwrap_or_else(|| from.email.clone());
            if !senders.contains(&name) {
                senders.push(name);
            }
        }
        let mut named = senders.iter().take(NAMED).cloned().collect::<Vec<_>>().join(", ");
        if senders.len() > NAMED {
            named = text(cx, "mail.notify.more", &[("senders", &named)]).to_string();
        }
        let body = ely_ui::i18n::counted(
            cx,
            "mail.notify.body",
            count as u64,
            &[("count", &count.to_string()), ("senders", &named)],
        );
        cx.show_system_notification(SystemNotification {
            tag: format!("{TAG}{}", id.0).into(),
            title: text(cx, "mail.notify.title", &[]),
            body,
            actions: Vec::new(),
        });
        log::info!("mail: told of {count} new messages for account {}", id.0);
    }
}
