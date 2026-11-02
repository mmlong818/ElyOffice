//! Deleting drafts: from the list, while reopening or sending (C12.13).

use ely_gpui_component::primitives::Severity;
use ely_mail_engine::DraftId;
use gpui::{Entity, TestAppContext, VisualTestContext};

use super::{
    super::{
        Keys,
        leaving::kept,
        settle, window,
        writing::{NOTE, composing, draft, seeded},
    },
    new_message, pause, problem, rooted, told, with, write,
};
use crate::mail::Mail;

#[test]
fn deleting_a_draft_removes_it_and_closes() {
    let dir = tempfile::tempdir().unwrap();
    seeded(dir.path(), format!("{NOTE}\r\nx").as_bytes(), &[]);
    let mut cx = TestAppContext::single();
    let (mail, cx) = window(&mut cx, dir.path(), Keys::default());
    let compose = new_message(&mail, cx);
    write(compose, "never mind", cx);
    pause(&mail, cx, 1);
    with(compose, cx, |compose, window, cx| {
        compose.confirm_discard(window, cx)
    });
    settle(cx, |app| compose.read(app).is_err());
    assert!(kept(&mail, cx).is_empty());
}

/// Through the main window's dialog: its last stop confirms.
fn press_confirm(mail: &Entity<Mail>, cx: &mut VisualTestContext) {
    cx.update(|window, app| window.focus(&mail.read(app).focus.clone(), app));
    for _ in 0..2 {
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
    }
    cx.simulate_keystrokes("shift-tab enter");
    cx.simulate_event(gpui::KeyUpEvent {
        keystroke: gpui::Keystroke::parse("enter").unwrap(),
    });
    cx.run_until_parked();
}

/// Saves one draft and closes its window.
fn shelve_one(mail: &Entity<Mail>, cx: &mut VisualTestContext) -> DraftId {
    let compose = new_message(mail, cx);
    write(compose, "never mind", cx);
    pause(mail, cx, 1);
    with(compose, cx, |_, window, _| window.remove_window());
    settle(cx, |app| compose.read(app).is_err());
    kept(mail, cx)[0].id
}

#[test]
fn deleting_through_the_dialog_removes_the_draft() {
    let dir = tempfile::tempdir().unwrap();
    seeded(dir.path(), format!("{NOTE}\r\nx").as_bytes(), &[]);
    let mut cx = TestAppContext::single();
    let (mail, cx) = window(&mut cx, dir.path(), Keys::default());
    let id = shelve_one(&mail, cx);
    cx.update(|_, app| mail.update(app, |mail, cx| mail.ask_delete(id, cx)));
    press_confirm(&mail, cx);
    settle(cx, |app| mail.read(app).deleting.is_none());
    pause(&mail, cx, 0);
}

#[test]
fn a_draft_being_sent_stays_when_deleted_from_the_list() {
    let dir = tempfile::tempdir().unwrap();
    seeded(dir.path(), format!("{NOTE}\r\nx").as_bytes(), &[]);
    let mut cx = TestAppContext::single();
    let (mail, cx) = window(&mut cx, dir.path(), Keys::default());
    let compose = new_message(&mail, cx);
    write(compose, "Sent soon", cx);
    with(compose, cx, |compose, _, cx| {
        compose.edited(draft(&["ann@example.com"], "Hi"), cx)
    });
    pause(&mail, cx, 1);
    let id = kept(&mail, cx)[0].id;
    with(compose, cx, |compose, _, cx| compose.begin_sending(cx));
    cx.update(|_, app| mail.update(app, |mail, cx| mail.ask_delete(id, cx)));
    cx.run_until_parked();
    assert!(
        !cx.update(|_, app| compose.read(app).unwrap().discarding),
        "no question while sending"
    );
    assert_eq!(problem(compose, cx), Some(Severity::Warning));
    // Asked before its window opened, confirmed after.
    cx.update(|_, app| mail.update(app, |mail, _| mail.deleting = Some(id)));
    press_confirm(&mail, cx);
    settle(cx, |_| true);
    assert!(
        cx.update(|_, app| compose.read(app).is_ok()),
        "the writer stays"
    );
    assert_eq!(kept(&mail, cx).len(), 1, "the draft stays");
}

#[test]
fn deleting_a_draft_while_it_reopens_leaves_no_window() {
    let dir = tempfile::tempdir().unwrap();
    seeded(dir.path(), format!("{NOTE}\r\nx").as_bytes(), &[]);
    let mut cx = TestAppContext::single();
    let (mail, cx) = window(&mut cx, dir.path(), Keys::default());
    let root = rooted(&mail, cx);
    for opened_first in [true, false] {
        let compose = new_message(&mail, cx);
        write(compose, "never mind", cx);
        pause(&mail, cx, 1);
        with(compose, cx, |_, window, _| window.remove_window());
        settle(cx, |app| compose.read(app).is_err());
        let [shelved] = kept(&mail, cx).try_into().unwrap();
        cx.update(|_, app| {
            mail.update(app, |mail, cx| {
                mail.reopen(shelved.id, cx);
                mail.ask_delete(shelved.id, cx);
                assert_eq!(mail.deleting, Some(shelved.id), "no window yet");
            })
        });
        if opened_first {
            settle(cx, |app| app.windows().len() == 2);
        }
        press_confirm(&mail, cx);
        settle(cx, |app| {
            let store = mail.read(app).store.clone().unwrap();
            let none = store.lock().unwrap().drafts().unwrap().is_empty();
            none && mail.read(app).reopening.is_empty() && app.windows().len() == 1
        });
        cx.run_until_parked();
        assert!(composing(cx).is_empty(), "opened first: {opened_first}");
        assert!(!told(&root, Severity::Danger, cx), "no failure to read it");
    }
}

#[test]
fn deleting_while_files_load_waits_for_them() {
    let dir = tempfile::tempdir().unwrap();
    seeded(dir.path(), format!("{NOTE}\r\nx").as_bytes(), &[]);
    let mut cx = TestAppContext::single();
    let (mail, cx) = window(&mut cx, dir.path(), Keys::default());
    let file = dir.path().join("menu.txt");
    std::fs::write(&file, "soup").unwrap();
    for path in [file, dir.path().join("gone.txt")] {
        let compose = new_message(&mail, cx);
        write(compose, "never mind", cx);
        with(compose, cx, |compose, window, cx| {
            compose.attach(vec![path], cx);
            compose.confirm_discard(window, cx);
        });
        settle(cx, |app| compose.read(app).is_err());
    }
    assert!(kept(&mail, cx).is_empty());
}
