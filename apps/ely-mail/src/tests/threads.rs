//! The reading pane shows the picked message's conversation (C12.11).

use std::{fs, path::Path, sync::atomic::Ordering};

use ely_mail_engine::{Fetched, Key, MailboxId, MessageId, Role, Store};
use gpui::{App, Entity, TestAppContext, TestDispatcher, VisualTestContext};

use super::{
    Keys, account,
    reading::{on_picked, picked, picture_came, pictures},
    settle, window,
};
use crate::{
    mail::Mail,
    reading::{SHOWN, Thread},
};

/// A message `id`, `hour` hours into 2024, answering `parent`.
fn note(id: &str, parent: Option<&str>, hour: u32, words: &str) -> Vec<u8> {
    let (day, hour) = (hour / 24 + 1, hour % 24);
    let mut head = format!(
        "From: Ann <ann@example.com>\r\nTo: Bo <bo@example.com>\r\nSubject: plan\r\n\
         Date: {day} Jan 2024 {hour:02}:00:00 +0000\r\nMessage-ID: <{id}>\r\n"
    );
    if let Some(parent) = parent {
        head.push_str(&format!(
            "In-Reply-To: <{parent}>\r\nReferences: <{parent}>\r\n"
        ));
    }
    format!("{head}Content-Type: text/html\r\n\r\n<p>{words}</p>\r\n").into_bytes()
}

struct Seeded {
    inbox: MailboxId,
    sent: MailboxId,
    uid: u32,
}

impl Seeded {
    fn new(dir: &Path) -> Self {
        let mut store = Store::open(dir).unwrap();
        let bo = store.add_account(&account("bo@example.com")).unwrap();
        let [inbox, sent] = store
            .set_folders(
                bo,
                &[("INBOX", Some(Role::Inbox)), ("Sent", Some(Role::Sent))],
            )
            .unwrap()
            .try_into()
            .unwrap();
        Self {
            inbox,
            sent,
            uid: 0,
        }
    }

    fn put(&mut self, dir: &Path, mailbox: MailboxId, bytes: &[u8]) -> MessageId {
        self.uid += 1;
        let key = Key::Imap {
            uidvalidity: 1,
            uid: self.uid,
        };
        let mut store = Store::open(dir).unwrap();
        store
            .put(mailbox, &key, &[], Fetched::Whole(bytes))
            .unwrap()
    }
}

fn thread<'a>(mail: &Entity<Mail>, app: &'a App) -> Option<&'a Thread> {
    match &mail.read(app).reading.shown.as_ref()?.thread {
        Some(Ok(thread)) => Some(thread),
        _ => None,
    }
}

fn members(mail: &Entity<Mail>, app: &App) -> Vec<MessageId> {
    let thread = thread(mail, app).unwrap();
    thread.letters.iter().map(|each| each.stored.id).collect()
}

/// Picks `id` once `rows` messages are listed.
fn pick(mail: &Entity<Mail>, id: MessageId, rows: usize, cx: &mut VisualTestContext) {
    settle(
        cx,
        |app| matches!(&mail.read(app).listing.mails, Some(Ok(listed)) if listed.len() == rows),
    );
    let key = id.0.to_string().into();
    cx.update(|_, app| mail.update(app, |mail, cx| mail.select(&[key], cx)));
    settle(cx, |app| picked(mail, app).is_some());
}

#[test]
fn a_picked_message_reads_with_its_conversation() {
    let dir = tempfile::tempdir().unwrap();
    let mut seeded = Seeded::new(dir.path());
    let (inbox, sent) = (seeded.inbox, seeded.sent);
    let root = seeded.put(
        dir.path(),
        inbox,
        &note("a@x", None, 1, "Shall we meet <b>Friday</b>?"),
    );
    let reply = seeded.put(
        dir.path(),
        sent,
        &note("b@x", Some("a@x"), 2, "Friday works"),
    );
    let last = seeded.put(dir.path(), inbox, &note("c@x", Some("b@x"), 3, "Booked"));
    let alone = seeded.put(
        dir.path(),
        inbox,
        &note("d@x", None, 4, "Same subject, new talk"),
    );
    let mut cx = TestAppContext::single();
    let (mail, cx) = window(&mut cx, dir.path(), Keys::default());
    // Sent is not listed; its reply still reads.
    pick(&mail, root, 3, cx);
    cx.update(|_, app| {
        assert_eq!(members(&mail, app), [root, reply, last]);
        let middle = &thread(&mail, app).unwrap().letters[1];
        let open = middle.open.as_ref().unwrap();
        assert_eq!(open.body().unwrap().snippet, "Friday works");
        let first = &thread(&mail, app).unwrap().letters[0];
        let open = first.open.as_ref().unwrap();
        assert_eq!(open.body().unwrap().snippet, "Shall we meet Friday?");
    });
    for (selector, shown) in [
        (format!("letter {}", root.0), true),
        (format!("letter {}", last.0), true),
        (format!("fold {}", reply.0), true),
        (format!("letter {}", reply.0), false),
        (format!("fold {}", alone.0), false),
    ] {
        assert_eq!(seen(cx, selector.clone()), shown, "{selector}");
    }
    pick(&mail, alone, 3, cx);
    cx.update(|_, app| assert_eq!(members(&mail, app), [alone]));
}

#[test]
fn a_reply_that_arrives_joins_and_loaded_pictures_stay() {
    let (web, served) = pictures();
    let dir = tempfile::tempdir().unwrap();
    let mut seeded = Seeded::new(dir.path());
    let inbox = seeded.inbox;
    let words = format!("Look <img src=\"{web}\">");
    let root = seeded.put(dir.path(), inbox, &note("a@x", None, 1, &words));
    let mut cx = TestAppContext::single();
    let (mail, cx) = window(&mut cx, dir.path(), Keys::default());
    pick(&mail, root, 1, cx);
    on_picked(&mail, cx, Mail::load_pictures);
    settle(cx, |app| picture_came(&mail, &web, app));
    let reply = seeded.put(dir.path(), inbox, &note("b@x", Some("a@x"), 2, "Nice"));
    cx.update(|_, app| mail.update(app, |mail, cx| mail.read_mail(cx)));
    settle(cx, |app| {
        thread(&mail, app).is_some_and(|t| t.letters.len() == 2)
    });
    cx.update(|_, app| {
        assert_eq!(members(&mail, app), [root, reply]);
        assert!(picture_came(&mail, &web, app), "the root kept its picture");
    });
    assert_eq!(served.load(Ordering::SeqCst), 1, "nothing fetched again");
}

#[test]
fn a_long_thread_shows_its_newest_and_says_how_many_are_not() {
    let dir = tempfile::tempdir().unwrap();
    let mut seeded = Seeded::new(dir.path());
    let inbox = seeded.inbox;
    let total = SHOWN + 2;
    let mut ids = Vec::new();
    for at in 0..total {
        let parent = at.checked_sub(1).map(|up| format!("m{up}@x"));
        let bytes = note(&format!("m{at}@x"), parent.as_deref(), at as u32, "hi");
        ids.push(seeded.put(dir.path(), inbox, &bytes));
    }
    let mut cx = TestAppContext::single();
    let (mail, cx) = window(&mut cx, dir.path(), Keys::default());
    pick(&mail, ids[total - 1], total, cx);
    cx.update(|_, app| {
        let thread = thread(&mail, app).unwrap();
        assert_eq!(thread.older, 2);
        assert_eq!(thread.letters.len(), SHOWN);
        assert!(!members(&mail, app).contains(&ids[0]));
    });
    assert!(seen(cx, "thread-older".into()));
    pick(&mail, ids[0], total, cx);
    cx.update(|_, app| {
        let thread = thread(&mail, app).unwrap();
        assert_eq!((thread.older, thread.letters.len()), (1, SHOWN + 1));
        assert!(
            members(&mail, app).contains(&ids[0]),
            "the picked one shows"
        );
    });
}

#[test]
fn a_damaged_letter_fails_alone() {
    let dir = tempfile::tempdir().unwrap();
    let mut seeded = Seeded::new(dir.path());
    let inbox = seeded.inbox;
    let root = seeded.put(dir.path(), inbox, &note("a@x", None, 1, "first"));
    let broken = note("b@x", Some("a@x"), 2, "second");
    let reply = seeded.put(dir.path(), inbox, &broken);
    let original = damage(&dir.path().join("content"), &broken);
    let mut cx = TestAppContext::single();
    let (mail, cx) = window(&mut cx, dir.path(), Keys::default());
    pick(&mail, root, 2, cx);
    cx.update(|_, app| {
        let thread = thread(&mail, app).unwrap();
        assert_eq!(members(&mail, app), [root, reply]);
        assert!(thread.letters[0].open.is_ok());
        let error = thread.letters[1].open.as_ref().err().unwrap();
        assert!(error.contains("damaged"), "{error}");
    });
    let error = format!("letter-error {}", reply.0);
    assert!(seen(cx, error), "the newest stands open, failed");
    fs::write(original, &broken).unwrap();
    cx.update(|_, app| mail.update(app, |mail, cx| mail.read_letter(cx)));
    settle(cx, |app| {
        thread(&mail, app).is_some_and(|thread| thread.letters[1].open.is_ok())
    });
}

#[test]
fn a_trust_change_reaches_a_reply_still_being_read() {
    // Seeds order the plan's trust read around the write.
    for seed in 0..6 {
        for trusted in [false, true] {
            trust_while_reading(seed, trusted);
        }
    }
}

fn trust_while_reading(seed: u64, trusted: bool) {
    let (web, served) = pictures();
    let dir = tempfile::tempdir().unwrap();
    let mut seeded = Seeded::new(dir.path());
    let inbox = seeded.inbox;
    let store = Store::open(dir.path()).unwrap();
    store.set_trusted("ann@example.com", !trusted).unwrap();
    drop(store);
    let root = seeded.put(dir.path(), inbox, &note("a@x", None, 1, "first"));
    let mut cx = TestAppContext::build(TestDispatcher::new(seed), None);
    let (mail, cx) = window(&mut cx, dir.path(), Keys::default());
    pick(&mail, root, 1, cx);
    let words = format!("See <img src=\"{web}\">");
    let reply = seeded.put(dir.path(), inbox, &note("b@x", Some("a@x"), 2, &words));
    cx.update(|_, app| {
        mail.update(app, |mail, cx| {
            mail.read_letter(cx);
            mail.trust_sender(root, trusted, cx);
        })
    });
    settle(cx, |app| {
        thread(&mail, app).is_some_and(|thread| thread.letters.len() == 2)
    });
    cx.run_until_parked();
    let came = |app: &App| {
        let letters = &thread(&mail, app).unwrap().letters;
        assert_eq!(letters[1].stored.id, reply);
        let open = letters[1].open.as_ref().unwrap();
        assert_eq!(open.trusted, trusted, "seed {seed}: the reply follows");
        matches!(open.pictures.get(web.as_str()), Some(Ok(_)))
    };
    if trusted {
        settle(cx, came);
    } else {
        assert!(!cx.update(|_, app| came(app)));
    }
    let fetched = served.load(Ordering::SeqCst);
    assert_eq!(fetched, usize::from(trusted), "seed {seed}");
}

/// Debug selectors are static; tests leak theirs.
pub(super) fn seen(cx: &mut VisualTestContext, selector: String) -> bool {
    cx.debug_bounds(selector.leak()).is_some()
}

/// Overwrites the original holding `bytes`; returns its path.
fn damage(content: &Path, bytes: &[u8]) -> std::path::PathBuf {
    for shard in fs::read_dir(content).unwrap() {
        for file in fs::read_dir(shard.unwrap().path()).unwrap() {
            let path = file.unwrap().path();
            if fs::read(&path).unwrap() == bytes {
                fs::write(&path, b"junk").unwrap();
                return path;
            }
        }
    }
    panic!("no original held those bytes");
}
