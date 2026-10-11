//! Sheet numbers hold only in the state they were read.

use super::*;

#[test]
fn a_rename_asked_before_a_delete_renames_nothing() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let mut cx = TestAppContext::single();
    let Some((window, book)) = book(&mut cx, dir.path()) else {
        eprintln!("SKIP: no LOK build");
        return;
    };
    blank(&mut cx, &book);
    for (name, at) in [("B", "2"), ("C", "3")] {
        fresh(&cx, &book);
        update(&mut cx, &book, |book, cx| book.run_active("sheet.insert", &[name, at], cx));
    }
    settle(&cx, |app| sheet_names(app, &book).len() == 3);
    fresh(&cx, &book);
    update(&mut cx, &book, |book, cx| book.show_sheet(1, cx));
    cx.update_window(window.into(), |_, window, app| book.update(app, |book, cx| book.ask_rename(window, cx)))
        .expect("the window lives");
    // The first sheet goes while the dialog is open.
    update(&mut cx, &book, |book, cx| book.run_active("sheet.delete", &["1"], cx));
    settle(&cx, |app| sheet_names(app, &book) == ["B", "C"]);
    fresh(&cx, &book);
    update(&mut cx, &book, |book, cx| book.rename_asked("Renamed B", cx));
    fresh(&cx, &book);
    assert_eq!(cx.update(|app| sheet_names(app, &book)), ["B", "C"], "C keeps its name");
}

#[test]
fn a_failed_command_still_moves_the_state_on() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let mut cx = TestAppContext::single();
    let Some((_window, book)) = book(&mut cx, dir.path()) else {
        eprintln!("SKIP: no LOK build");
        return;
    };
    blank(&mut cx, &book);
    // No merged cell to split: the kernel refuses.
    update(&mut cx, &book, |book, cx| book.run_active("cells.split", &[], cx));
    fresh(&cx, &book);
    update(&mut cx, &book, |book, cx| book.run_active("sheet.rename", &["Plan", "1"], cx));
    settle(&cx, |app| sheet_names(app, &book) == ["Plan"]);
}

#[test]
fn a_failed_print_says_printing_failed() {
    use ely_gpui_component::primitives::Severity;
    let dir = tempfile::tempdir().expect("a temp dir");
    let mut cx = TestAppContext::single();
    let Some((window, book)) = book(&mut cx, dir.path()) else {
        eprintln!("SKIP: no LOK build");
        return;
    };
    // The window's toasts live on its root.
    let root = window
        .update(&mut cx, |_, window, cx| {
            let root = ely_ui::document_root(ely_platform::AppId::Table, book.clone().into(), &ely_ui::commands::FILE, Vec::new(), window, cx);
            book.update(cx, |book, _| book.core.root = root.downgrade());
            root
        })
        .expect("the window");
    let gone = dir.path().join("gone.pdf");
    update(&mut cx, &book, |_, cx| ely_office::print_later(gone, cx));
    cx.run_until_parked();
    let (toasts, prefix) = cx.update(|app| (root.read(app).toasts(app), ely_office::text(app, "office.print.failed", &[("error", "")])));
    assert!(toasts.iter().any(|(text, severity)| *severity == Some(Severity::Danger) && text.starts_with(prefix.as_ref())), "{toasts:?}");
}
