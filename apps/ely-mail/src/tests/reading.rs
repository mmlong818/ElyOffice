use std::{
    io::{BufRead, BufReader, Write},
    net::TcpListener,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    thread,
};

use ely_gpui_component::documents::MarkdownBlock;
use ely_mail_engine::{Fetched, Key, MailboxId, MessageId, Role, Store};
use gpui::{App, Context, Entity, TestAppContext, VisualTestContext};

use super::{Keys, account, settle, window};
use crate::{
    letter::{self, Words},
    mail::Mail,
    reading::{Each, Open},
};

/// Serves a PNG to every request; counts them.
pub(super) fn pictures() -> (String, Arc<AtomicUsize>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let served = Arc::new(AtomicUsize::new(0));
    let count = served.clone();
    thread::spawn(move || {
        for stream in listener.incoming() {
            let stream = stream.unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut line = String::new();
            while reader.read_line(&mut line).unwrap() > 2 {
                line.clear();
            }
            let mut stream = stream;
            let head = "HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nContent-Length: 4\r\n\r\n";
            stream.write_all(head.as_bytes()).unwrap();
            stream.write_all(b"\x89PNG").unwrap();
            count.fetch_add(1, Ordering::SeqCst);
        }
    });
    (format!("http://127.0.0.1:{port}/a.png"), served)
}

/// HTML with a logo, a web picture, a waiting file.
fn letter(subject: &str, web: &str) -> Vec<u8> {
    format!(
        "From: Ann <ann@example.com>\r\nTo: Bo <bo@example.com>\r\nSubject: {subject}\r\n\
         Date: 3 Jan 2024 10:00:00 +0000\r\n\
         Content-Type: multipart/mixed; boundary=m\r\n\r\n\
         --m\r\nContent-Type: multipart/related; boundary=r\r\n\r\n\
         --r\r\nContent-Type: text/html\r\n\r\n\
         <p>Hello <b>Bo</b></p><script>steal()</script>\
         <img src=\"cid:logo@ex\" alt=\"Logo\"><img src=\"{web}\">\r\n\
         --r\r\nContent-Type: image/png\r\nContent-ID: <logo@ex>\r\n\
         Content-Transfer-Encoding: base64\r\n\r\niVBORw==\r\n--r--\r\n\
         --m\r\nContent-Type: application/pdf\r\n\
         Content-Disposition: attachment; filename=report.pdf\r\n\r\n\r\n--m--\r\n"
    )
    .into_bytes()
}

fn seeded(dir: &std::path::Path, web: &str) -> MailboxId {
    let mut store = Store::open(dir).unwrap();
    let ann = store.add_account(&account("bo@example.com")).unwrap();
    let [inbox] = store
        .set_folders(ann, &[("INBOX", Some(Role::Inbox))])
        .unwrap()
        .try_into()
        .unwrap();
    for (uid, subject) in [(1, "first"), (2, "second")] {
        let bytes = letter(subject, web);
        let missing = ["2".to_owned()];
        let text = Fetched::Text {
            bytes: &bytes,
            missing: &missing,
        };
        let key = Key::Imap {
            uidvalidity: 1,
            uid,
        };
        store.put(inbox, &key, &[], text).unwrap();
    }
    let headers = b"From: Cy <cy@example.com>\r\nSubject: old\r\n\
                    Date: 1 Jan 2024 10:00:00 +0000\r\n\r\n";
    let key = Key::Imap {
        uidvalidity: 1,
        uid: 3,
    };
    store
        .put(inbox, &key, &[], Fetched::Headers(headers))
        .unwrap();
    inbox
}

fn pick(mail: &Entity<Mail>, subject: &str, cx: &mut VisualTestContext) {
    settle(
        cx,
        |app| matches!(&mail.read(app).listing.mails, Some(Ok(rows)) if rows.len() == 3),
    );
    let key = cx.update(|_, app| {
        let rows = mail.read(app).listing.mails.clone().unwrap().unwrap();
        rows.iter()
            .find(|row| row.subject == subject)
            .unwrap()
            .key
            .clone()
    });
    cx.update(|_, app| mail.update(app, |mail, cx| mail.select(&[key], cx)));
    settle(cx, |app| opened(mail, app).is_some());
}

pub(super) fn opened<'a>(mail: &Entity<Mail>, app: &'a App) -> Option<&'a Open> {
    match &picked(mail, app)?.open {
        Ok(open) => Some(open),
        Err(error) => panic!("the letter failed: {error}"),
    }
}

/// The picked letter, once its thread is read.
pub(super) fn picked<'a>(mail: &Entity<Mail>, app: &'a App) -> Option<&'a Each> {
    let shown = mail.read(app).reading.shown.as_ref()?;
    match &shown.thread {
        Some(Ok(thread)) => thread
            .letters
            .iter()
            .find(|each| each.stored.id == shown.id),
        Some(Err(error)) => panic!("the thread failed: {error}"),
        None => None,
    }
}

/// Runs `act` on the picked letter.
pub(super) fn on_picked(
    mail: &Entity<Mail>,
    cx: &mut VisualTestContext,
    act: impl FnOnce(&mut Mail, MessageId, &mut Context<Mail>),
) {
    cx.update(|_, app| {
        let id = picked(mail, app).unwrap().stored.id;
        mail.update(app, |mail, cx| act(mail, id, cx));
    });
}

pub(super) fn picture_came(mail: &Entity<Mail>, url: &str, app: &App) -> bool {
    opened(mail, app).is_some_and(|open| matches!(open.pictures.get(url), Some(Ok(_))))
}

#[test]
fn an_html_letter_reads_without_running_or_loading() {
    let (web, served) = pictures();
    let dir = tempfile::tempdir().unwrap();
    seeded(dir.path(), &web);
    let mut cx = TestAppContext::single();
    let (mail, cx) = window(&mut cx, dir.path(), Keys::default());
    pick(&mail, "first", cx);
    cx.update(|_, app| {
        let open = opened(&mail, app).unwrap();
        let shown: Vec<String> = (open.blocks.iter())
            .map(|block| match block {
                MarkdownBlock::Paragraph(line) => line.text.clone(),
                MarkdownBlock::Image { url, .. } => url.clone(),
                other => panic!("unexpected {other:?}"),
            })
            .collect();
        assert_eq!(shown, ["Hello Bo", "ely-inline:0", web.as_str()]);
        assert!(matches!(open.pictures.get("ely-inline:0"), Some(Ok(_))));
        assert!(!open.pictures.contains_key(&web));
        let body = open.body().unwrap();
        assert_eq!(body.remote, std::slice::from_ref(&web));
        assert_eq!(body.files, [("report.pdf".to_owned(), None)]);
        assert!(!open.trusted && !open.fetching);
    });
    cx.run_until_parked();
    assert_eq!(served.load(Ordering::SeqCst), 0, "nothing was fetched");
}

#[test]
fn allowed_pictures_come_and_a_trusted_sender_needs_no_asking() {
    let (web, served) = pictures();
    let dir = tempfile::tempdir().unwrap();
    seeded(dir.path(), &web);
    let mut cx = TestAppContext::single();
    let (mail, cx) = window(&mut cx, dir.path(), Keys::default());
    pick(&mail, "first", cx);
    on_picked(&mail, cx, Mail::load_pictures);
    settle(cx, |app| picture_came(&mail, &web, app));
    assert_eq!(served.load(Ordering::SeqCst), 1);
    on_picked(&mail, cx, |mail, id, cx| mail.trust_sender(id, true, cx));
    settle(cx, |app| {
        opened(&mail, app).is_some_and(|open| open.trusted)
    });
    pick(&mail, "second", cx);
    settle(cx, |app| picture_came(&mail, &web, app));
    assert_eq!(served.load(Ordering::SeqCst), 2);
    on_picked(&mail, cx, |mail, id, cx| mail.trust_sender(id, false, cx));
    settle(cx, |app| {
        opened(&mail, app).is_some_and(|open| !open.trusted)
    });
    let store = Store::open(dir.path()).unwrap();
    assert!(!store.trusts("ann@example.com").unwrap());
}

#[test]
fn a_message_with_only_headers_says_so() {
    let dir = tempfile::tempdir().unwrap();
    seeded(dir.path(), "https://example.invalid/a.png");
    let mut cx = TestAppContext::single();
    let (mail, cx) = window(&mut cx, dir.path(), Keys::default());
    pick(&mail, "old", cx);
    let id = cx.update(|_, app| {
        let each = picked(&mail, app).unwrap();
        let open = opened(&mail, app).unwrap();
        assert!(open.body().is_none());
        assert_eq!(each.sender(), Some("cy@example.com"));
        assert!(open.letter.headers.contains("Subject: old"));
        each.stored.id.0
    });
    let toggle = format!("mail-headers-{id}");
    assert!(super::threads::seen(cx, toggle), "no header toggle");
}

#[test]
fn a_body_that_arrives_replaces_the_headers_shown() {
    let (web, served) = pictures();
    let dir = tempfile::tempdir().unwrap();
    let inbox = seeded(dir.path(), &web);
    let mut cx = TestAppContext::single();
    let (mail, cx) = window(&mut cx, dir.path(), Keys::default());
    pick(&mail, "first", cx);
    on_picked(&mail, cx, Mail::load_pictures);
    settle(cx, |app| picture_came(&mail, &web, app));
    // Nothing changed: the pictures stay, none fetched again.
    cx.update(|_, app| mail.update(app, |mail, cx| mail.read_mail(cx)));
    cx.run_until_parked();
    cx.update(|_, app| assert!(picture_came(&mail, &web, app)));
    assert_eq!(served.load(Ordering::SeqCst), 1);

    pick(&mail, "old", cx);
    let whole = b"From: Cy <cy@example.com>\r\nSubject: old\r\n\
                  Date: 1 Jan 2024 10:00:00 +0000\r\n\r\nThe old body\r\n";
    let key = Key::Imap {
        uidvalidity: 1,
        uid: 3,
    };
    Store::open(dir.path())
        .unwrap()
        .put(inbox, &key, &[], Fetched::Whole(whole))
        .unwrap();
    cx.update(|_, app| mail.update(app, |mail, cx| mail.read_mail(cx)));
    settle(cx, |app| {
        opened(&mail, app).is_some_and(|open| open.body().is_some())
    });
    cx.update(|_, app| {
        let open = opened(&mail, app).unwrap();
        let [MarkdownBlock::Paragraph(line)] = &open.blocks[..] else {
            panic!("{:?}", open.blocks);
        };
        assert_eq!(line.text, "The old body");
    });
}

#[test]
fn mail_never_draws_svg() {
    let words = Words {
        broken: "broken".into(),
        waiting: "waiting".into(),
        not_picture: "not a picture".into(),
        svg: "no svg".into(),
        unnamed: "unnamed".into(),
    };
    let svg = br#"<svg xmlns="http://www.w3.org/2000/svg"
        xmlns:xlink="http://www.w3.org/1999/xlink" width="9" height="9">
        <image xlink:href="C:/private.png" width="9" height="9"/></svg>"#;
    for kind in ["image/svg+xml", "Image/SVG+XML"] {
        match letter::held(kind, svg.to_vec(), &words) {
            Err(why) => assert_eq!(why, "no svg"),
            Ok(_) => panic!("{kind} was held"),
        }
    }
    assert!(letter::held("image/png", b"\x89PNG".to_vec(), &words).is_ok());
}

#[test]
fn every_body_of_a_mixed_letter_shows() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = Store::open(dir.path()).unwrap();
    let ann = store.add_account(&account("bo@example.com")).unwrap();
    let [inbox] = store
        .set_folders(ann, &[("INBOX", Some(Role::Inbox))])
        .unwrap()
        .try_into()
        .unwrap();
    let raw = b"From: Ann <ann@example.com>\r\nSubject: visit\r\n\
                Content-Type: multipart/mixed; boundary=m\r\n\r\n\
                --m\r\nContent-Type: text/plain\r\n\r\nAppointment is at 09:00\r\n\
                --m\r\nContent-Type: text/html\r\n\r\n<p>Directions to the clinic</p>\r\n\
                --m\r\nContent-Type: multipart/alternative; boundary=a\r\n\r\n\
                --a\r\nContent-Type: text/plain\r\n\r\nBring your card\r\n\
                --a\r\nContent-Type: text/html\r\n\r\n<p>Bring your card</p>\r\n\
                --a--\r\n--m--\r\n";
    let key = Key::Imap {
        uidvalidity: 1,
        uid: 1,
    };
    let id = store.put(inbox, &key, &[], Fetched::Whole(raw)).unwrap();
    let words = Words {
        broken: "broken".into(),
        waiting: "waiting".into(),
        not_picture: "not a picture".into(),
        svg: "no svg".into(),
        unnamed: "unnamed".into(),
    };
    let kept = letter::Kept::Original(raw.to_vec());
    let read = letter::letter(&store.get(id).unwrap(), kept, &words).unwrap();
    let shown: Vec<String> = (read.body.unwrap().blocks.iter())
        .map(|block| match block {
            MarkdownBlock::Paragraph(line) => line.text.clone(),
            other => panic!("unexpected {other:?}"),
        })
        .collect();
    assert_eq!(
        shown,
        [
            "Appointment is at 09:00",
            "Directions to the clinic",
            "Bring your card"
        ]
    );
}
