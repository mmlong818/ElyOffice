//! Read, flag, move, delete in the window (C12.15).

use ely_mail_engine::{AccountId, Change, Fetched, Key, MailboxId, Role, Store, push::Dropped};
use gpui::{Entity, TestAppContext, VisualTestContext};

use super::{Keys, account, settle, window};
use crate::{boxes::Open, mail::Mail, organize::dropped_words};

const MAIL: &[u8] =
    b"From: a@example.com\r\nSubject: Hi\r\nDate: 1 Jan 2024 10:00:00 +0000\r\n\r\nhi\r\n";

struct Seeded {
    ann: AccountId,
    inbox: MailboxId,
    trash: MailboxId,
    archive: MailboxId,
}

/// Ann's inbox holds one unread message.
fn seeded(dir: &std::path::Path) -> Seeded {
    let mut store = Store::open(dir).unwrap();
    let ann = store.add_account(&account("ann@example.com")).unwrap();
    let folders = [
        ("INBOX", Some(Role::Inbox)),
        ("Trash", Some(Role::Trash)),
        ("Archive", Some(Role::Archive)),
    ];
    let [inbox, trash, archive] = store
        .set_folders(ann, &folders)
        .unwrap()
        .try_into()
        .unwrap();
    store.open_mailbox(inbox, 1).unwrap();
    let key = Key::Imap {
        uidvalidity: 1,
        uid: 1,
    };
    store.put(inbox, &key, &[], Fetched::Whole(MAIL)).unwrap();
    Seeded {
        ann,
        inbox,
        trash,
        archive,
    }
}

fn open(mail: &Entity<Mail>, open: Open, cx: &mut VisualTestContext) {
    cx.update(|_, app| mail.update(app, |mail, cx| mail.open_box(open, cx)));
}

fn rows(mail: &Entity<Mail>, cx: &mut VisualTestContext) -> usize {
    cx.update(|_, app| match &mail.read(app).listing.mails {
        Some(Ok(rows)) => rows.len(),
        _ => usize::MAX,
    })
}

/// Picks the only message, once listed.
fn pick_it(mail: &Entity<Mail>, cx: &mut VisualTestContext) {
    settle(cx, |app| mail.read(app).listing.facts.len() == 1);
    let key = cx.update(|_, app| mail.read(app).listing.facts.keys().next().unwrap().clone());
    cx.update(|_, app| mail.update(app, |mail, cx| mail.select(&[key], cx)));
}

fn offered(mail: &Entity<Mail>, cx: &mut VisualTestContext) -> Vec<&'static str> {
    cx.update(|_, app| {
        let mail = mail.read(app);
        let offers = mail.offered(&mail.listing.picked());
        offers.into_iter().map(|(id, ..)| id).collect()
    })
}

fn waiting(mail: &Entity<Mail>, id: AccountId, cx: &mut VisualTestContext) -> Option<u64> {
    cx.update(|_, app| {
        let syncing = mail.read(app).syncs.get(&id)?;
        syncing.synced.map(|synced| synced.waiting)
    })
}

#[test]
fn reading_one_marks_it_read_and_queues_it() {
    let dir = tempfile::tempdir().unwrap();
    let seeded = seeded(dir.path());
    let mut cx = TestAppContext::single();
    let (mail, cx) = window(&mut cx, dir.path(), Keys::default());
    open(&mail, Open::Mailbox(seeded.ann, seeded.inbox), cx);
    pick_it(&mail, cx);
    settle(cx, |app| {
        let facts = &mail.read(app).listing.facts;
        facts.values().all(|fact| fact.seen)
    });
    // The server is unreachable: the change waits.
    settle(cx, |app| {
        let syncing = mail.read(app).syncs.get(&seeded.ann);
        syncing
            .is_some_and(|each| each.task.is_none() && each.synced.is_some_and(|s| s.waiting == 1))
    });
    assert_eq!(waiting(&mail, seeded.ann, cx), Some(1));
}

#[test]
fn the_star_flags_and_unflags() {
    let dir = tempfile::tempdir().unwrap();
    let seeded = seeded(dir.path());
    let mut cx = TestAppContext::single();
    let (mail, cx) = window(&mut cx, dir.path(), Keys::default());
    open(&mail, Open::Mailbox(seeded.ann, seeded.inbox), cx);
    settle(cx, |app| mail.read(app).listing.facts.len() == 1);
    let key = cx.update(|_, app| mail.read(app).listing.facts.keys().next().unwrap().clone());
    let star = key.clone();
    cx.update(|_, app| mail.update(app, |mail, cx| mail.star(&star, true, cx)));
    settle(cx, |app| mail.read(app).listing.facts[&key].flagged);
    cx.update(|_, app| mail.update(app, |mail, cx| mail.star(&star, false, cx)));
    settle(cx, |app| !mail.read(app).listing.facts[&key].flagged);
}

#[test]
fn a_change_while_paused_counts_as_waiting() {
    let dir = tempfile::tempdir().unwrap();
    let seeded = seeded(dir.path());
    let mut cx = TestAppContext::single();
    let (mail, cx) = window(&mut cx, dir.path(), Keys::default());
    cx.update(|_, app| mail.update(app, |mail, cx| mail.set_paused(true, cx)));
    open(&mail, Open::Mailbox(seeded.ann, seeded.inbox), cx);
    settle(cx, |app| mail.read(app).listing.facts.len() == 1);
    let key = cx.update(|_, app| mail.read(app).listing.facts.keys().next().unwrap().clone());
    cx.update(|_, app| mail.update(app, |mail, cx| mail.star(&key, true, cx)));
    settle(cx, |app| mail.read(app).listing.facts[&key].flagged);
    settle(cx, |app| {
        let syncing = mail.read(app).syncs.get(&seeded.ann);
        syncing.is_some_and(|each| each.synced.is_some_and(|s| s.waiting == 1))
    });
}

#[test]
fn delete_moves_to_trash_and_trash_asks_before_for_good() {
    let dir = tempfile::tempdir().unwrap();
    let seeded = seeded(dir.path());
    let mut cx = TestAppContext::single();
    let (mail, cx) = window(&mut cx, dir.path(), Keys::default());
    let delete = |mail: &Entity<Mail>, cx: &mut VisualTestContext| {
        cx.update(|_, app| mail.update(app, |mail, cx| mail.delete_picked(cx)));
    };
    open(&mail, Open::Mailbox(seeded.ann, seeded.inbox), cx);
    pick_it(&mail, cx);
    assert!(!offered(&mail, cx).contains(&"mail-act-restore"));
    delete(&mail, cx);
    settle(cx, |app| mail.read(app).listing.facts.is_empty());
    assert!(cx.update(|_, app| mail.read(app).erasing.is_none()));
    open(&mail, Open::Mailbox(seeded.ann, seeded.trash), cx);
    pick_it(&mail, cx);
    assert!(offered(&mail, cx).contains(&"mail-act-restore"));
    cx.update(|_, app| mail.update(app, |mail, cx| mail.organize(Change::Restore, cx)));
    settle(cx, |app| mail.read(app).listing.facts.is_empty());
    open(&mail, Open::Mailbox(seeded.ann, seeded.inbox), cx);
    pick_it(&mail, cx);
    delete(&mail, cx);
    open(&mail, Open::Mailbox(seeded.ann, seeded.trash), cx);
    pick_it(&mail, cx);
    delete(&mail, cx);
    settle(cx, |app| mail.read(app).erasing.is_some());
    // Asked, nothing gone yet.
    assert_eq!(rows(&mail, cx), 1);
    press_confirm(&mail, cx);
    settle(cx, |app| mail.read(app).listing.facts.is_empty());
    assert!(cx.update(|_, app| mail.read(app).erasing.is_none()));
}

/// The dialog's last stop confirms.
pub(super) fn press_confirm(mail: &Entity<Mail>, cx: &mut VisualTestContext) {
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

#[test]
fn archive_shows_only_where_an_archive_takes_it() {
    let dir = tempfile::tempdir().unwrap();
    let seeded = seeded(dir.path());
    let mut cx = TestAppContext::single();
    let (mail, cx) = window(&mut cx, dir.path(), Keys::default());
    // Archive needs the account's folders, read after its sync.
    settle(cx, |app| {
        let syncing = mail.read(app).syncs.get(&seeded.ann);
        syncing.is_some_and(|each| each.folders.len() == 3)
    });
    open(&mail, Open::Mailbox(seeded.ann, seeded.inbox), cx);
    pick_it(&mail, cx);
    assert!(offered(&mail, cx).contains(&"mail-act-archive"));
    cx.update(|_, app| mail.update(app, |mail, cx| mail.organize(Change::Archive, cx)));
    settle(cx, |app| mail.read(app).listing.facts.is_empty());
    open(&mail, Open::Mailbox(seeded.ann, seeded.archive), cx);
    pick_it(&mail, cx);
    assert!(!offered(&mail, cx).contains(&"mail-act-archive"));
}

#[test]
fn dropped_changes_name_their_reason() {
    let dir = tempfile::tempdir().unwrap();
    seeded(dir.path());
    let mut cx = TestAppContext::single();
    let (_mail, cx) = window(&mut cx, dir.path(), Keys::default());
    let said = |dropped: &[Dropped], cx: &mut VisualTestContext| {
        cx.update(|_, app| dropped_words(dropped, app).map(|words| words.to_string()))
    };
    assert_eq!(said(&[], cx), None);
    let two = [Dropped::Gone, Dropped::Refused("NO full".into())];
    assert_eq!(
        said(&two, cx).unwrap(),
        "2 changes were not made on the server and are undone: \
         the message is no longer on the server"
    );
    let one = [Dropped::Unsupported("MOVE".into())];
    assert_eq!(
        said(&one, cx).unwrap(),
        "A change was not made on the server and is undone: \
         the server does not support MOVE"
    );
}
