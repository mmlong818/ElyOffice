use super::*;
use gpui::TestAppContext;

/// Steps the scheduler until `done` or ten seconds pass.
fn settle(cx: &TestAppContext, done: impl Fn(&App) -> bool) -> bool {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while std::time::Instant::now() < deadline {
        if !cx.executor().tick() {
            cx.executor()
                .advance_clock(std::time::Duration::from_millis(20));
        }
        if cx.update(|app| done(app)) {
            return true;
        }
    }
    false
}

/// The view's 800 by 600 pixel area.
fn area() -> Bounds<Pixels> {
    Bounds {
        origin: point(px(0.0), px(0.0)),
        size: size(px(800.0), px(600.0)),
    }
}

/// A user profile whose dark scheme paints the page black.
const DARK_PROFILE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<oor:items xmlns:oor="http://openoffice.org/2001/registry" xmlns:xs="http://www.w3.org/2001/XMLSchema" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
<item oor:path="/org.openoffice.Office.UI/ColorScheme/ColorSchemes"><node oor:name="TEST_DARK" oor:op="replace"><node oor:name="DocColor"><prop oor:name="Color" oor:op="fuse"><value>0</value></prop></node></node></item>
</oor:items>
"#;

/// Shows a view in a window.
struct Host(Entity<LokView>);

impl gpui::Render for Host {
    fn render(&mut self, _: &mut gpui::Window, _: &mut gpui::Context<Self>) -> impl gpui::IntoElement {
        self.0.clone()
    }
}

#[test]
fn a_dark_app_paints_the_page_white() {
    let Some(install) = crate::client::tests::install() else {
        eprintln!("SKIP: no pinned LOK build");
        return;
    };
    let mut cx = TestAppContext::single();
    cx.update(|app| {
        ely_gpui_component::init_for_tests(app);
        ely_gpui_component::theme::Theme::set_mode_now(ely_gpui_component::theme::Mode::Dark, app);
    });
    // A dark profile, as an old Ely left.
    let dir = tempfile::tempdir().expect("a temp dir");
    let profile = dir.path().join("profile");
    std::fs::create_dir_all(profile.join("user")).expect("a profile");
    std::fs::write(profile.join("user/registrymodifications.xcu"), DARK_PROFILE).expect("the scheme writes");
    let source = dir.path().join("note.fodt");
    std::fs::write(&source, crate::client::tests::FODT).expect("the fixture writes");
    let mut started = crate::Worker::spawn(&install, &profile).expect("the worker");
    let doc = started.load(&crate::file_url(&source)).expect("the document loads");
    let laid = super::super::layout(&mut started, &doc).expect("the layout reads");
    let worker = Arc::new(Mutex::new(started));
    let shade = || {
        let mut worker = worker.lock().expect("the worker");
        let tile = doc.paint_tile(&mut worker, (1440, 1440, 3840, 3840), (64, 64)).expect("a tile");
        let sum: u64 = tile.pixels.chunks_exact(4).map(|px| u64::from(px[0]) + u64::from(px[1]) + u64::from(px[2])).sum();
        sum / (64 * 64 * 3)
    };
    worker.lock().expect("the worker").run(&doc, "view.theme", &["TEST_DARK"]).expect("the dark scheme");
    let dark = shade();
    assert!(dark < 100, "the kernel starts dark: {dark}");
    let view = cx.update(|app| LokView::open(Arc::clone(&worker), doc.clone(), laid, app));
    let shown = view.clone();
    cx.add_window(move |_, _| Host(shown));
    assert!(
        settle(&cx, |app| view.read(app).lit() && view.read(app).settled()),
        "the scheme goes out"
    );
    assert_eq!(paper(), gpui::white(), "the page mask is the scheme's white");
    let light = shade();
    assert!(light > 240, "the view paints the page white: {light}");
}

#[test]
fn a_failed_tile_reports_and_retries_on_a_move() {
    let Some(install) = crate::client::tests::install() else {
        eprintln!("SKIP: no pinned LOK build");
        return;
    };
    let cx = TestAppContext::single();
    let (view, _) = cx.update(|app| live_view(app, &install));
    let errors = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    cx.update(|app| {
        let errors = errors.clone();
        app.subscribe(&view, move |_, error: &LokError, _| {
            errors.borrow_mut().push(error.0.clone())
        })
        .detach();
        view.update(app, |view, cx| {
            let geom = view.geom;
            view.prepare(area());
            view.fetching.remove(&(0, 0));
            view.finish_tile((0, 0), geom, Err(io::Error::other("no paint")), cx);
            assert!(view.failed.contains(&((0, 0), view.generation)));
            view.prepare(area());
            assert!(!view.fetching.contains(&(0, 0)), "no retry storm in place");
            view.scroll_by(0.0, -40.0, cx);
            view.prepare(area());
            assert!(view.fetching.contains(&(0, 0)), "a move retries the tile");
        });
    });
    assert!(
        settle(&cx, |_| !errors.borrow().is_empty()),
        "the failure reaches the app"
    );
    assert!(
        errors.borrow()[0].contains("no paint"),
        "{:?}",
        errors.borrow()
    );
}
