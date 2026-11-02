//! Drafts kept while writing, listed, reopened, removed (C12.13).

use ely_gpui_component::primitives::Severity;
use ely_mail_engine::{Role, Store};
use ely_platform::AppId;
use ely_ui::Root;
use gpui::{Entity, TestAppContext, VisualTestContext, WindowHandle};

use super::{
    Keys, account,
    leaving::kept,
    settle, window,
    writing::{NOTE, composing, draft, opened, seeded},
};
mod deleting;

use crate::{
    boxes::Open,
    compose::{Compose, PAUSE},
    draft_list,
    mail::Mail,
};

pub(super) fn with<R>(
    compose: WindowHandle<Compose>,
    cx: &mut VisualTestContext,
    act: impl FnOnce(&mut Compose, &mut gpui::Window, &mut gpui::Context<Compose>) -> R,
) -> R {
    cx.update(|_, app| compose.update(app, act).unwrap())
}

pub(super) fn write(compose: WindowHandle<Compose>, words: &str, cx: &mut VisualTestContext) {
    with(compose, cx, |compose, _, cx| {
        compose
            .document
            .update(cx, |document, cx| document.set_text(words, cx))
    });
}

/// Lets typing stop, then waits on the store.
pub(super) fn pause(mail: &Entity<Mail>, cx: &mut VisualTestContext, drafts: usize) {
    cx.executor().advance_clock(PAUSE);
    settle(cx, |app| {
        let store = mail.read(app).store.clone().unwrap();
        let count = store.lock().unwrap().drafts().unwrap().len();
        count == drafts
    });
}

pub(super) fn new_message(
    mail: &Entity<Mail>,
    cx: &mut VisualTestContext,
) -> WindowHandle<Compose> {
    cx.update(|_, app| mail.update(app, |mail, cx| mail.write_new(cx)));
    opened(cx)
}

/// A shell around the window, to read its toasts.
pub(super) fn rooted(mail: &Entity<Mail>, cx: &mut VisualTestContext) -> Entity<Root> {
    cx.update(|window, app| {
        let root = ely_ui::tool_root(AppId::Mail, mail.clone().into(), Vec::new(), window, app);
        mail.update(app, |mail, _| mail.root = root.downgrade());
        root
    })
}

pub(super) fn told(root: &Entity<Root>, severity: Severity, cx: &mut VisualTestContext) -> bool {
    cx.update(|_, app| root.read(app).toasts(app))
        .iter()
        .any(|(_, shown)| *shown == Some(severity))
}

pub(super) fn problem(
    compose: WindowHandle<Compose>,
    cx: &mut VisualTestContext,
) -> Option<Severity> {
    cx.update(|_, app| compose.read(app).unwrap().problem.clone())
        .map(|(severity, _)| severity)
}

#[test]
fn words_are_kept_once_typing_stops() {
    let dir = tempfile::tempdir().unwrap();
    seeded(dir.path(), format!("{NOTE}\r\nx").as_bytes(), &[]);
    let mut cx = TestAppContext::single();
    let (mail, cx) = window(&mut cx, dir.path(), Keys::default());
    let compose = new_message(&mail, cx);
    write(compose, "Lunch?", cx);
    cx.run_until_parked();
    assert!(kept(&mail, cx).is_empty(), "not while typing");
    pause(&mail, cx, 1);
    let [first] = kept(&mail, cx).try_into().unwrap();
    assert_eq!(first.unsent.words, "Lunch?");
    write(compose, "Lunch at one?", cx);
    with(compose, cx, |compose, _, cx| {
        compose.edited(draft(&["ann@example.com"], "Plan"), cx)
    });
    cx.executor().advance_clock(PAUSE);
    settle(cx, |app| {
        let store = mail.read(app).store.clone().unwrap();
        let drafts = store.lock().unwrap().drafts().unwrap();
        drafts.len() == 1 && drafts[0].unsent.subject == "Plan"
    });
    let [second] = kept(&mail, cx).try_into().unwrap();
    assert_eq!(second.id, first.id, "the same draft, written again");
    assert_eq!(second.unsent.words, "Lunch at one?");
    assert_eq!(second.unsent.to, ["ann@example.com"]);
    write(compose, "", cx);
    with(compose, cx, |compose, _, cx| {
        compose.edited(draft(&[], ""), cx)
    });
    pause(&mail, cx, 0);
    assert!(
        cx.update(|_, app| compose.read(app).unwrap().shelf.id.is_none()),
        "back to blank, the draft goes"
    );
}

#[test]
fn a_draft_reopens_as_it_was_and_once() {
    let dir = tempfile::tempdir().unwrap();
    seeded(dir.path(), format!("{NOTE}\r\nx").as_bytes(), &[]);
    let mut cx = TestAppContext::single();
    let (mail, cx) = window(&mut cx, dir.path(), Keys::default());
    let compose = new_message(&mail, cx);
    write(compose, "**Lunch?**", cx);
    with(compose, cx, |compose, _, cx| {
        let mut addressed = draft(&["ann@example.com"], "Plan");
        addressed.bcc = vec!["cy@example.com".into()];
        compose.edited(addressed, cx);
    });
    let file = dir.path().join("menu.txt");
    std::fs::write(&file, "soup").unwrap();
    with(compose, cx, |compose, _, cx| compose.attach(vec![file], cx));
    settle(cx, |app| compose.read(app).unwrap().reading == 0);
    pause(&mail, cx, 1);
    with(compose, cx, |_, window, _| window.remove_window());
    settle(cx, |app| compose.read(app).is_err());
    cx.update(|_, app| mail.update(app, |mail, cx| mail.open_box(Open::Drafts, cx)));
    settle(
        cx,
        |app| matches!(&mail.read(app).listing.mails, Some(Ok(rows)) if rows.len() == 1),
    );
    let [shelved] = kept(&mail, cx).try_into().unwrap();
    let key = draft_list::key(shelved.id);
    cx.update(|_, app| mail.update(app, |mail, cx| mail.select(&[key], cx)));
    settle(cx, |_| true);
    assert!(
        cx.debug_bounds("mail-draft-words").is_some(),
        "the draft shows"
    );
    for _ in 0..2 {
        cx.update(|_, app| mail.update(app, |mail, cx| mail.reopen(shelved.id, cx)));
        settle(cx, |_| true);
    }
    let [again] = composing(cx).try_into().unwrap();
    cx.update(|_, app| {
        let again = again.read(app).unwrap();
        assert_eq!(again.shelf.id, Some(shelved.id));
        assert_eq!(again.to, ["ann@example.com"]);
        assert_eq!(again.bcc, ["cy@example.com"]);
        assert_eq!(again.subject.as_ref(), "Plan");
        assert_eq!(again.files.len(), 1);
        assert_eq!(again.document.read(app).text(app), "**Lunch?**");
    });
    let shut = with(again, cx, |compose, window, cx| {
        compose.may_close(window, cx)
    });
    assert!(shut, "untouched, it closes as it is");
    assert_eq!(
        kept(&mail, cx),
        std::slice::from_ref(&shelved),
        "the draft stays unchanged"
    );
    with(again, cx, |_, window, _| window.remove_window());
    settle(cx, |app| again.read(app).is_err());
    cx.update(|_, app| {
        mail.update(app, |mail, cx| {
            mail.reopen(shelved.id, cx);
            mail.reopen(shelved.id, cx);
        })
    });
    settle(cx, |_| true);
    assert_eq!(composing(cx).len(), 1, "asked twice at once, one window");
}

/// Holds whatever connects, until told to let go.
pub(super) fn silent_server() -> (u16, std::sync::mpsc::Sender<()>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    listener.set_nonblocking(true).unwrap();
    let (stop, stopped) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut held = Vec::new();
        while stopped.try_recv().is_err() {
            if let Ok((stream, _)) = listener.accept() {
                held.push(stream);
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    });
    (port, stop)
}

#[test]
fn a_server_copy_failing_after_its_window_closed_says_so_here() {
    let dir = tempfile::tempdir().unwrap();
    let (port, stop) = silent_server();
    let mut quiet = account("bo@example.com");
    quiet.incoming.port = port;
    quiet.outgoing.port = port;
    let mut store = Store::open(dir.path()).unwrap();
    let bo = store.add_account(&quiet).unwrap();
    store
        .set_folders(
            bo,
            &[("INBOX", Some(Role::Inbox)), ("Drafts", Some(Role::Drafts))],
        )
        .unwrap();
    store.set_drafts_up(bo, true).unwrap();
    drop(store);
    let keys = Keys::default();
    keys.kept
        .lock()
        .unwrap()
        .insert("account:bo@example.com".into(), "pw".into());
    let mut cx = TestAppContext::single();
    let (mail, cx) = window(&mut cx, dir.path(), keys);
    let root = rooted(&mail, cx);
    let compose = new_message(&mail, cx);
    write(compose, "words", cx);
    let shut = with(compose, cx, |compose, window, cx| {
        compose.may_close(window, cx)
    });
    assert!(!shut, "kept first");
    settle(cx, |app| compose.read(app).is_err());
    assert!(
        !cx.update(|_, app| mail.read(app).pushing.is_empty()),
        "still going up"
    );
    stop.send(()).unwrap();
    settle(cx, |app| {
        let toasts = root.read(app).toasts(app);
        toasts
            .iter()
            .any(|(_, shown)| *shown == Some(Severity::Warning))
    });
}

#[test]
fn spacing_alone_is_an_edit_and_spaces_alone_are_blank() {
    let dir = tempfile::tempdir().unwrap();
    seeded(dir.path(), format!("{NOTE}\r\nx").as_bytes(), &[]);
    let mut cx = TestAppContext::single();
    let (mail, cx) = window(&mut cx, dir.path(), Keys::default());
    let compose = new_message(&mail, cx);
    write(compose, "   ", cx);
    let shut = with(compose, cx, |compose, window, cx| {
        compose.may_close(window, cx)
    });
    assert!(shut, "only spaces, nothing to keep");
    write(compose, "hello", cx);
    pause(&mail, cx, 1);
    write(compose, "    hello", cx);
    let shut = with(compose, cx, |compose, window, cx| {
        compose.may_close(window, cx)
    });
    assert!(!shut, "the indent is unkept yet");
    settle(cx, |app| compose.read(app).is_err());
    assert_eq!(kept(&mail, cx)[0].unsent.words, "    hello");
}

#[test]
fn a_file_unread_while_closing_keeps_the_window() {
    let dir = tempfile::tempdir().unwrap();
    seeded(dir.path(), format!("{NOTE}\r\nx").as_bytes(), &[]);
    let mut cx = TestAppContext::single();
    let (mail, cx) = window(&mut cx, dir.path(), Keys::default());
    let compose = new_message(&mail, cx);
    write(compose, "see attached", cx);
    let gone = dir.path().join("gone.txt");
    let shut = with(compose, cx, |compose, window, cx| {
        compose.attach(vec![gone], cx);
        compose.may_close(window, cx)
    });
    assert!(!shut, "waits for the file");
    settle(cx, |app| compose.read(app).unwrap().reading == 0);
    assert_eq!(problem(compose, cx), Some(Severity::Danger));
    write(compose, "see below", cx);
    pause(&mail, cx, 1);
    assert!(
        cx.update(|_, app| compose.read(app).is_ok()),
        "the failed file ended that close"
    );
    let shut = with(compose, cx, |compose, window, cx| {
        compose.may_close(window, cx)
    });
    assert!(shut, "kept already, it closes");
    assert_eq!(kept(&mail, cx)[0].unsent.words, "see below");
}

#[test]
fn a_draft_the_store_refuses_keeps_the_window() {
    let dir = tempfile::tempdir().unwrap();
    seeded(dir.path(), format!("{NOTE}\r\nx").as_bytes(), &[]);
    let mut cx = TestAppContext::single();
    let (mail, cx) = window(&mut cx, dir.path(), Keys::default());
    let compose = new_message(&mail, cx);
    write(compose, "words", cx);
    let from = cx.update(|_, app| compose.read(app).unwrap().from);
    let mut other = Store::open(dir.path()).unwrap();
    other.remove_account(from).unwrap();
    let shut = with(compose, cx, |compose, window, cx| {
        compose.may_close(window, cx)
    });
    assert!(!shut);
    settle(cx, |_| true);
    assert!(
        cx.update(|_, app| compose.read(app).is_ok()),
        "the words stay"
    );
    assert_eq!(problem(compose, cx), Some(Severity::Danger));
}

#[test]
fn a_server_copy_needs_a_drafts_folder_and_says_why_not() {
    let dir = tempfile::tempdir().unwrap();
    seeded(dir.path(), format!("{NOTE}\r\nx").as_bytes(), &[]);
    let mut cx = TestAppContext::single();
    let (mail, cx) = window(&mut cx, dir.path(), Keys::default());
    let compose = new_message(&mail, cx);
    let from = cx.update(|_, app| compose.read(app).unwrap().from);
    cx.update(|_, app| mail.update(app, |mail, cx| mail.open_box(Open::Account(from), cx)));
    settle(cx, |app| mail.read(app).drafts_up == Some((from, false)));
    assert!(
        cx.debug_bounds("mail-drafts-up").is_some(),
        "the switch shows"
    );
    let switch = cx.debug_bounds("mail-drafts-up").unwrap();
    cx.simulate_click(switch.center(), gpui::Modifiers::none());
    settle(cx, |app| mail.read(app).drafts_up == Some((from, true)));
    write(compose, "words", cx);
    pause(&mail, cx, 1);
    settle(cx, |_| true);
    assert_eq!(
        problem(compose, cx),
        Some(Severity::Info),
        "no Drafts folder"
    );
    let store = cx.update(|_, app| mail.read(app).store.clone().unwrap());
    store
        .lock()
        .unwrap()
        .set_folders(
            from,
            &[("INBOX", Some(Role::Inbox)), ("Drafts", Some(Role::Drafts))],
        )
        .unwrap();
    write(compose, "more words", cx);
    cx.executor().advance_clock(PAUSE);
    settle(cx, |app| {
        let problem = compose.read(app).unwrap().problem.clone();
        problem.is_some_and(|(severity, _)| severity == Severity::Warning)
    });
    assert!(cx.update(|_, app| mail.read(app).pushing.is_empty()));
}

#[test]
fn drafts_have_no_mail_filters() {
    let dir = tempfile::tempdir().unwrap();
    seeded(dir.path(), format!("{NOTE}\r\nx").as_bytes(), &[]);
    let mut cx = TestAppContext::single();
    let (mail, cx) = window(&mut cx, dir.path(), Keys::default());
    settle(cx, |_| true);
    assert!(cx.debug_bounds("mail-filters").is_some(), "inboxes filter");
    cx.update(|_, app| mail.update(app, |mail, cx| mail.open_box(Open::Drafts, cx)));
    settle(cx, |_| true);
    assert!(cx.debug_bounds("mail-filters").is_none());
}
