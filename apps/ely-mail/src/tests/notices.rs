use ely_mail_engine::{AccountId, Mailbox, MailboxId, MessageId, Store, Summary};
use gpui::TestAppContext;

use super::{Keys, account, window};

fn from(name: Option<&str>, email: &str) -> Summary {
    Summary {
        id: MessageId(1),
        account: AccountId(1),
        mailbox: MailboxId(1),
        role: None,
        pop: false,
        from: vec![Mailbox { name: name.map(str::to_owned), email: email.into() }],
        subject: Some("private words".into()),
        date: None,
        seen: false,
        flagged: false,
        attachments: None,
    }
}

#[test]
fn new_mail_names_its_senders_never_its_words() {
    let dir = tempfile::tempdir().unwrap();
    let id = Store::open(dir.path()).unwrap().add_account(&account("ann@example.com")).unwrap();
    let mut cx = TestAppContext::single();
    // As the app's start does.
    cx.update(|cx| cx.set_app_identity("ElyOffice.ElyMail", "ElyMail"));
    let (mail, vcx) = window(&mut cx, dir.path(), Keys::default());
    let arrived = vec![
        from(Some("Bob"), "bob@example.com"),
        from(None, "cy@example.com"),
        from(Some("Bob"), "bob@example.com"),
    ];
    vcx.update(|_, app| mail.update(app, |mail, cx| mail.announce(id, arrived, cx)));
    vcx.update(|_, app| mail.update(app, |mail, cx| mail.announce(id, vec![from(Some("Dee"), "d@example.com")], cx)));
    vcx.update(|_, app| mail.update(app, |mail, cx| mail.announce(id, Vec::new(), cx)));
    let shown = cx.shown_system_notifications();
    let told: Vec<_> = shown.iter().map(|each| (each.tag.to_string(), each.body.to_string())).collect();
    let tag = format!("mail-new-{}", id.0);
    assert_eq!(
        told,
        [(tag.clone(), "3 new messages from Bob, cy@example.com".to_owned()), (tag, "From Dee".to_owned())]
    );
    assert!(shown.iter().all(|each| !each.body.contains("private")), "no subject or text");
}

#[test]
fn muted_mail_tells_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let id = Store::open(dir.path()).unwrap().add_account(&account("ann@example.com")).unwrap();
    let mut cx = TestAppContext::single();
    // As the app's start does.
    cx.update(|cx| cx.set_app_identity("ElyOffice.ElyMail", "ElyMail"));
    let (mail, vcx) = window(&mut cx, dir.path(), Keys::default());
    vcx.update(|_, app| ely_ui::settings::switches::set(ely_ui::settings::switches::NOTIFY, false, app));
    vcx.update(|_, app| mail.update(app, |mail, cx| mail.announce(id, vec![from(Some("Bob"), "b@example.com")], cx)));
    assert!(cx.shown_system_notifications().is_empty());
}
