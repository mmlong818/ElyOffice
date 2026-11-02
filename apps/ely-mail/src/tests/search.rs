use std::time::Duration;

use ely_mail_engine::{Fetched, Field, Key, Store};
use gpui::{Entity, TestAppContext, VisualTestContext, px, size};
use jiff::{Timestamp, tz::TimeZone};

use super::{
    Keys,
    panes::{seeded, shows},
    settle, window,
};
use crate::{boxes::Open, mail::Mail};

fn typed(mail: &Entity<Mail>, words: &str, cx: &mut VisualTestContext) {
    cx.update(|_, app| {
        let input = mail.read(app).seeking.input.clone();
        input.update(app, |input, cx| input.set_text(words, cx));
    });
    cx.executor().advance_clock(Duration::from_millis(250));
}

fn unsearched(mail: &Entity<Mail>, cx: &mut VisualTestContext) -> Option<u64> {
    cx.update(|_, app| mail.read(app).seeking.unsearched)
}

#[test]
fn typing_searches_every_account_and_folder() {
    let dir = tempfile::tempdir().unwrap();
    seeded(dir.path());
    let mut cx = TestAppContext::single();
    let (mail, cx) = window(&mut cx, dir.path(), Keys::default());
    shows(&mail, cx, &["new", "bob", "old"]);
    typed(&mail, "kept", cx);
    shows(&mail, cx, &["kept"]);
    assert_eq!(unsearched(&mail, cx), Some(0));
    typed(&mail, "", cx);
    shows(&mail, cx, &["new", "bob", "old"]);
    assert_eq!(unsearched(&mail, cx), None);
}

#[test]
fn a_field_and_days_narrow_the_search() {
    let dir = tempfile::tempdir().unwrap();
    seeded(dir.path());
    let mut cx = TestAppContext::single();
    let (mail, cx) = window(&mut cx, dir.path(), Keys::default());
    shows(&mail, cx, &["new", "bob", "old"]);
    typed(&mail, "hi", cx);
    cx.update(|_, app| mail.update(app, |mail, cx| mail.set_field(Field::Subject, cx)));
    shows(&mail, cx, &[]);
    cx.update(|_, app| mail.update(app, |mail, cx| mail.set_field(Field::Body, cx)));
    let at: Timestamp = "2024-01-03T10:00:00Z".parse().unwrap();
    let day = at.to_zoned(TimeZone::system()).date();
    cx.update(|_, app| mail.update(app, |mail, cx| mail.set_days(Some((day, day)), cx)));
    shows(&mail, cx, &["new"]);
    cx.update(|_, app| mail.update(app, |mail, cx| mail.set_days(None, cx)));
    settle(cx, |app| match &mail.read(app).listing.mails {
        Some(Ok(rows)) => rows.len() == 4,
        _ => false,
    });
}

#[test]
fn days_alone_search() {
    let dir = tempfile::tempdir().unwrap();
    seeded(dir.path());
    let mut cx = TestAppContext::single();
    let (mail, cx) = window(&mut cx, dir.path(), Keys::default());
    shows(&mail, cx, &["new", "bob", "old"]);
    let at: Timestamp = "2024-01-01T10:00:00Z".parse().unwrap();
    let day = at.to_zoned(TimeZone::system()).date();
    cx.update(|_, app| mail.update(app, |mail, cx| mail.set_days(Some((day, day)), cx)));
    shows(&mail, cx, &["old"]);
}

#[test]
fn opening_a_folder_ends_the_search() {
    let dir = tempfile::tempdir().unwrap();
    let (ann, archive) = seeded(dir.path());
    let mut cx = TestAppContext::single();
    let (mail, cx) = window(&mut cx, dir.path(), Keys::default());
    typed(&mail, "new", cx);
    shows(&mail, cx, &["new"]);
    let open = Open::Mailbox(ann, archive);
    cx.update(|_, app| mail.update(app, |mail, cx| mail.open_box(open, cx)));
    cx.executor().advance_clock(Duration::from_millis(250));
    shows(&mail, cx, &["kept"]);
    cx.update(|_, app| assert!(mail.read(app).seeking.input.read(app).is_empty()));
    assert_eq!(unsearched(&mail, cx), None);
}

#[test]
fn mail_with_only_headers_here_is_counted() {
    let dir = tempfile::tempdir().unwrap();
    let (_, archive) = seeded(dir.path());
    let mut store = Store::open(dir.path()).unwrap();
    let headers = b"From: Cy <cy@example.com>\r\nSubject: head\r\n\
        Date: 2 Jan 2024 10:00:00 +0000\r\n\r\n";
    let key = Key::Imap {
        uidvalidity: 1,
        uid: 9,
    };
    store
        .put(archive, &key, &[], Fetched::Headers(headers))
        .unwrap();
    drop(store);
    let mut cx = TestAppContext::single();
    let (mail, cx) = window(&mut cx, dir.path(), Keys::default());
    typed(&mail, "hi", cx);
    settle(cx, |app| mail.read(app).seeking.unsearched == Some(1));
}

#[test]
fn a_chosen_range_fits_a_narrow_window() {
    let dir = tempfile::tempdir().unwrap();
    seeded(dir.path());
    let mut cx = TestAppContext::single();
    let (mail, cx) = window(&mut cx, dir.path(), Keys::default());
    shows(&mail, cx, &["new", "bob", "old"]);
    cx.simulate_resize(size(px(640.), px(800.)));
    let first = "2025-12-30".parse().unwrap();
    let last = "2026-12-30".parse().unwrap();
    cx.update(|_, app| mail.update(app, |mail, cx| mail.set_days(Some((first, last)), cx)));
    settle(cx, |app| mail.read(app).seeking.unsearched.is_some());
    let pane = cx.debug_bounds("mail-filters").expect("the filters");
    let days = cx.debug_bounds("mail-search-days").expect("the days");
    let clear = cx
        .debug_bounds("mail-search-any-time")
        .expect("the clear button");
    assert!(
        days.right() <= clear.left(),
        "{days:?} runs under {clear:?}"
    );
    assert!(clear.right() <= pane.right(), "{clear:?} is past {pane:?}");
}

#[test]
fn a_found_message_moves_within_its_own_account() {
    let dir = tempfile::tempdir().unwrap();
    let (_ann, archive) = seeded(dir.path());
    let mut cx = TestAppContext::single();
    let (mail, cx) = window(&mut cx, dir.path(), Keys::default());
    cx.update(|_, app| {
        mail.update(app, |mail, cx| {
            mail.open_box(Open::Mailbox(_ann, archive), cx)
        })
    });
    shows(&mail, cx, &["kept"]);
    typed(&mail, "bob", cx);
    shows(&mail, cx, &["bob"]);
    settle(cx, |app| mail.read(app).syncs.len() == 2);
    let (key, bob) = cx.update(|_, app| {
        let (key, fact) = mail.read(app).listing.facts.iter().next().unwrap();
        (key.clone(), fact.account)
    });
    cx.update(|_, app| mail.update(app, |mail, cx| mail.select(&[key], cx)));
    let targets = cx.update(|_, app| {
        let mail = mail.read(app);
        let picked = mail.listing.picked();
        mail.move_targets(&picked)
            .map(|(account, folders)| (account, folders.iter().map(|f| f.id).collect::<Vec<_>>()))
    });
    assert_eq!(targets, Some((bob, vec![])));
}

#[test]
fn a_window_opened_from_the_tray_keeps_the_search() {
    let dir = tempfile::tempdir().unwrap();
    seeded(dir.path());
    let mut cx = TestAppContext::single();
    let (mail, cx) = window(&mut cx, dir.path(), Keys::default());
    typed(&mail, "kept", cx);
    shows(&mail, cx, &["kept"]);
    let before = cx.update(|_, app| mail.read(app).seeking.input.clone());
    cx.update(|window, app| mail.update(app, |mail, cx| mail.rehome(window, cx)));
    let after = cx.update(|_, app| mail.read(app).seeking.input.clone());
    assert_ne!(before, after, "a new box for the new window");
    assert_eq!(cx.update(|_, app| after.read(app).text().to_owned()), "kept");
    typed(&mail, "", cx);
    shows(&mail, cx, &["new", "bob", "old"]);
}

#[test]
fn a_new_mail_notice_ends_the_search() {
    let dir = tempfile::tempdir().unwrap();
    seeded(dir.path());
    let mut cx = TestAppContext::single();
    let (mail, cx) = window(&mut cx, dir.path(), Keys::default());
    shows(&mail, cx, &["new", "bob", "old"]);
    typed(&mail, "kept", cx);
    shows(&mail, cx, &["kept"]);
    let response = gpui::SystemNotificationResponse { tag: "mail-new-1".into(), action_id: None };
    cx.simulate_system_notification_response(response);
    cx.run_until_parked();
    shows(&mail, cx, &["new", "bob", "old"]);
    cx.update(|_, app| assert!(mail.read(app).seeking.input.read(app).is_empty()));
    assert_eq!(cx.update(|_, app| mail.read(app).open), Open::Inboxes);
}

#[test]
fn a_new_mail_notice_clears_the_filters() {
    let dir = tempfile::tempdir().unwrap();
    seeded(dir.path());
    let mut cx = TestAppContext::single();
    let (mail, cx) = window(&mut cx, dir.path(), Keys::default());
    shows(&mail, cx, &["new", "bob", "old"]);
    let filter = ely_mail_engine::Filter { flagged: true, attachments: true, ..Default::default() };
    cx.update(|_, app| mail.update(app, |mail, cx| mail.set_filter(filter, cx)));
    let response = gpui::SystemNotificationResponse { tag: "mail-new-1".into(), action_id: None };
    cx.simulate_system_notification_response(response);
    cx.run_until_parked();
    shows(&mail, cx, &["new", "bob", "old"]);
    assert_eq!(cx.update(|_, app| mail.read(app).filter), ely_mail_engine::Filter::default());
}
