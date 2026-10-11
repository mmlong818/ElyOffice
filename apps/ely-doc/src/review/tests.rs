use gpui::{Entity, TestAppContext};

use super::*;
use crate::actions::Dialog;
use crate::refs::InsertComment;
use crate::tests::{blank, docs, settle, settled, text_of, type_into, with_window, written};
use ely_office::prelude::*;

/// The review panel on the active tab.
fn panel(cx: &mut TestAppContext, docs: &Entity<Docs>) -> Entity<Review> {
    cx.update(|app| {
        let tab = docs.read(app).active_tab().expect("a tab");
        let (view, doc) = (tab.view.clone(), tab.doc.clone());
        let review = docs.read(app).review.clone();
        review.update(app, |review, cx| review.show(Some((&view, &doc)), cx));
        review
    })
}

#[test]
fn comments_thread_and_changes_accept_or_reject() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let mut cx = TestAppContext::single();
    let Some(docs) = docs(&mut cx, dir.path()) else {
        eprintln!("SKIP: no LOK build");
        return;
    };
    let id = blank(&mut cx, &docs);
    type_into(&mut cx, &docs, "Hello");
    let review = panel(&mut cx, &docs);
    with_window(&mut cx, &docs, |docs, window, cx| {
        docs.on_comment(&InsertComment, window, cx);
        let Some(Dialog::Ask { ask, .. }) = &docs.dialog else {
            panic!("the comment prompt is open");
        };
        let ask = ask.clone();
        docs.answer(&ask, "First note", cx);
    });
    assert_eq!(
        cx.update(|app| docs.read(app).side),
        Side::Review,
        "the panel shows"
    );
    settle(&cx, |app| review.read(app).threads.len() == 1);
    let key = cx.update(|app| review.read(app).threads[0].key.clone());
    cx.update(|app| review.update(app, |review, cx| review.select(&key, cx)));
    cx.update(|app| review.update(app, |review, cx| review.reply("An answer", cx)));
    settle(&cx, |app| {
        review
            .read(app)
            .threads
            .first()
            .is_some_and(|thread| thread.comments.len() == 2)
    });
    with_window(&mut cx, &docs, |docs, window, cx| {
        docs.on_track(&TrackChanges, window, cx)
    });
    settle(&cx, |app| {
        let tab = docs.read(app).active_tab().expect("a tab");
        tab.view.read(app).state(".uno:TrackChanges") == Some("true")
    });
    type_into(&mut cx, &docs, " kept");
    settle(&cx, |app| review.read(app).changes.len() == 1);
    let index = cx.update(|app| review.read(app).changes[0].index.to_string());
    cx.update(|app| review.update(app, |review, cx| review.run("track.accept", &[&index], cx)));
    settle(&cx, |app| review.read(app).changes.is_empty());
    type_into(&mut cx, &docs, " gone");
    settle(&cx, |app| review.read(app).changes.len() == 1);
    with_window(&mut cx, &docs, |docs, window, cx| {
        docs.on_reject_all(&RejectAll, window, cx)
    });
    settle(&cx, |app| review.read(app).changes.is_empty());
    settled(&cx, &docs);
    let path = dir.path().join("review.odt");
    with_window(&mut cx, &docs, |docs, window, cx| {
        docs.write_as(id, path.clone(), None, window, cx)
    });
    written(&cx, &path, None);
    let text = text_of(&mut cx, &docs, &path);
    for mark in ["First note", "An answer", " kept"] {
        assert!(text.contains(mark), "{mark} lands");
    }
    assert!(!text.contains("gone"), "the rejected words go");
}

#[test]
fn one_unreadable_time_spares_the_other_notes() {
    let note = |id: &str, parent: &str, at: &str| Note {
        id: id.into(),
        parent: parent.into(),
        author: "A".into(),
        text: "t".into(),
        resolved: false,
        at: at.into(),
        anchor: (0, 0, 0, 0),
    };
    // The kernel writes an empty date as 0-00-00.
    let mut undated = note("2", "0", "0-00-00");
    undated.author = "Undated".into();
    let notes = vec![note("1", "0", "2025-03-01T10:00:00"), undated, note("3", "1", "2025-03-02T09:00:00")];
    let (threads, unread) = threads(&notes);
    assert_eq!(threads.len(), 1);
    assert_eq!(threads[0].comments.len(), 2, "the reply stays in its thread");
    assert_eq!((unread.len(), unread[0].author.as_str(), unread[0].at.as_str()), (1, "Undated", "0-00-00"));
    // Through the refresh path: changes stay, the line names both.
    let dir = tempfile::tempdir().expect("a temp dir");
    let mut cx = gpui::TestAppContext::single();
    let Some(docs) = crate::tests::docs(&mut cx, dir.path()) else {
        eprintln!("SKIP: no LOK build");
        return;
    };
    let review = cx.update(|app| docs.read(app).review.clone());
    let change = Change { index: 7, author: "B".into(), kind: "Insert".into(), description: String::new(), at: "2025-03-01T10:00:00".into(), anchor: None };
    let lines = cx.update(|cx| review.update(cx, |review, cx| {
        review.take_lists(notes, vec![change]);
        assert_eq!(review.changes.len(), 1, "the changes stay");
        review.unread_lines(cx)
    }));
    assert_eq!(lines.len(), 1);
    assert!(lines[0].contains("Undated") && lines[0].contains(&unread[0].error), "{}", lines[0]);
}
