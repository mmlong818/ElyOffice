//! The lines above an answered or forwarded message.

use ely_mail_engine::{Mailbox, compose::Earlier};
use gpui::App;

use crate::{mail::text, view::local_time, writing::How};

fn person(mailbox: &Mailbox) -> String {
    match &mailbox.name {
        Some(name) if !name.trim().is_empty() => format!("{name} <{}>", mailbox.email),
        _ => mailbox.email.clone(),
    }
}

fn people(list: &[Mailbox]) -> String {
    list.iter().map(person).collect::<Vec<_>>().join(", ")
}

/// The line or lines that introduce the earlier message.
pub fn lead(how: How, mail: &ely_mail_engine::Mail, words: String, cx: &App) -> Earlier {
    let sender = people(&mail.from);
    let date = mail.date.map(local_time);
    if how != How::Forward {
        let lead = match &date {
            Some(date) => text(
                cx,
                "mail.write.wrote",
                &[("sender", &sender), ("date", date)],
            ),
            None => text(cx, "mail.write.wrote_undated", &[("sender", &sender)]),
        };
        return Earlier::Quoted {
            lead: lead.to_string(),
            text: words,
        };
    }
    let line = |key: &str, value: &str| text(cx, key, &[("value", value)]).to_string();
    let mut lead = vec![
        text(cx, "mail.write.forwarded", &[]).to_string(),
        line("mail.write.lead.from", &sender),
    ];
    lead.extend(date.map(|date| line("mail.write.lead.date", &date)));
    lead.extend(
        mail.subject
            .iter()
            .map(|it| line("mail.write.lead.subject", it)),
    );
    lead.push(line("mail.write.lead.to", &people(&mail.to)));
    if !mail.cc.is_empty() {
        lead.push(line("mail.write.lead.cc", &people(&mail.cc)));
    }
    Earlier::Forwarded { lead, text: words }
}
