//! Files handed to the app: arguments, Finder, second copies.

use std::{io, path::PathBuf, time::Duration};

use ely_platform::instance::Door;
use futures::{StreamExt, channel::mpsc};
use gpui::{AnyWindowHandle, App, Application, Global};

/// How long a hand-over waits for the window.
const TAKING: Duration = Duration::from_secs(5);

struct Asked(Option<mpsc::UnboundedReceiver<PathBuf>>);

impl Global for Asked {}

/// A second copy's files; `taken` answers once they are queued.
pub(crate) struct Hand {
    files: Vec<PathBuf>,
    taken: std::sync::mpsc::Sender<()>,
}

/// What the system and other copies hand the running app.
pub(crate) struct Handed {
    asked: mpsc::UnboundedReceiver<PathBuf>,
    hands: Hands,
}

/// Second copies' hand-overs, for the main window.
pub(crate) struct Hands {
    hands: mpsc::UnboundedReceiver<Hand>,
    queue: mpsc::UnboundedSender<PathBuf>,
}

/// Launch arguments, relative to where the app was started.
pub(crate) fn launch_args() -> io::Result<Vec<PathBuf>> {
    std::env::args_os().skip(1).map(std::path::absolute).collect()
}

/// Queues `files`, then files the system or copies ask for.
pub(crate) fn listen(application: &Application, files: Vec<PathBuf>, door: Door) -> io::Result<Handed> {
    let (queue, asked) = mpsc::unbounded();
    for path in files {
        queue.unbounded_send(path).ok();
    }
    let (sender, hands) = mpsc::unbounded();
    door.serve(move |files| {
        let (taken, took) = std::sync::mpsc::channel();
        sender.unbounded_send(Hand { files, taken }).is_ok() && took.recv_timeout(TAKING).is_ok()
    })?;
    let system = queue.clone();
    application.on_open_urls(move |urls| {
        for url in urls {
            match file_path(&url) {
                Some(path) => {
                    log::info!("run: the system asks to open a file");
                    system.unbounded_send(path).ok();
                }
                None => log::warn!("run: not a file URL, ignored"),
            }
        }
    });
    Ok(Handed { asked, hands: Hands { hands, queue } })
}

/// Keeps the files for `on_open`; the hand-overs go to `home`.
pub(crate) fn keep(handed: Handed, cx: &mut App) -> Hands {
    cx.set_global(Asked(Some(handed.asked)));
    handed.hands
}

/// No files asked at launch, for tests.
#[cfg(any(test, feature = "test-support"))]
pub fn install_no_launch_files(cx: &mut App) {
    cx.set_global(Asked(Some(mpsc::unbounded().1)));
}

/// Files asked before the app started: arguments and Finder's.
pub fn launch_files(cx: &mut App) -> Vec<PathBuf> {
    let asked = cx.global_mut::<Asked>().0.as_mut();
    let asked = asked.expect("launch_files comes before on_open");
    std::iter::from_fn(|| asked.try_recv().ok()).collect()
}

/// Hands each file asked from now on to `open`.
pub fn on_open(cx: &mut App, mut open: impl FnMut(PathBuf, &mut App) + 'static) {
    let mut asked = cx
        .global_mut::<Asked>()
        .0
        .take()
        .expect("on_open runs once per app");
    cx.spawn(async move |cx| {
        while let Some(path) = asked.next().await {
            cx.update(|cx| open(path, cx));
        }
    })
    .detach();
}

/// Brings `main` forward for each hand-over, then queues its files.
pub(crate) fn home(hands: Hands, main: AnyWindowHandle, cx: &mut App) {
    let Hands { mut hands, queue } = hands;
    cx.spawn(async move |cx| {
        while let Some(hand) = hands.next().await {
            let raised = cx.update(|cx| {
                cx.activate(true);
                main.update(cx, |_, window, _| window.activate_window())
            });
            // No answer: the other copy says the files went untaken.
            if let Err(error) = raised {
                log::warn!("run: the main window is gone, so a hand-over is refused: {error}");
                continue;
            }
            for path in hand.files {
                queue.unbounded_send(path).ok();
            }
            hand.taken.send(()).ok();
        }
    })
    .detach();
}

/// A `file://` URL's path, percent escapes decoded.
fn file_path(url: &str) -> Option<PathBuf> {
    let rest = url.strip_prefix("file://")?;
    let rest = rest.strip_prefix("localhost").unwrap_or(rest);
    if !rest.starts_with('/') {
        return None;
    }
    let mut bytes = Vec::with_capacity(rest.len());
    let mut raw = rest.bytes();
    while let Some(byte) = raw.next() {
        if byte != b'%' {
            bytes.push(byte);
            continue;
        }
        let hex = [raw.next()?, raw.next()?];
        bytes.push(u8::from_str_radix(std::str::from_utf8(&hex).ok()?, 16).ok()?);
    }
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        Some(PathBuf::from(std::ffi::OsString::from_vec(bytes)))
    }
    #[cfg(not(unix))]
    {
        // `file:///C:/a` holds `/C:/a`.
        let text = String::from_utf8(bytes).ok()?;
        Some(PathBuf::from(text.strip_prefix('/').unwrap_or(&text)))
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::file_path;
    use std::path::PathBuf;

    #[test]
    fn file_urls_decode_and_others_do_not() {
        assert_eq!(file_path("file:///Users/a/b%20c%23.md"), Some(PathBuf::from("/Users/a/b c#.md")));
        assert_eq!(file_path("file://localhost/tmp/x"), Some(PathBuf::from("/tmp/x")));
        assert_eq!(file_path("file:///a/%E4%BD%A0.md"), Some(PathBuf::from("/a/你.md")));
        assert_eq!(file_path("https://example.com/a"), None);
        assert_eq!(file_path("file:///a/%zz"), None);
        assert_eq!(file_path("file:///a/%4"), None);
    }
}

#[cfg(test)]
mod home_tests {
    use super::*;
    use gpui::{Context, IntoElement, Render, TestAppContext, Window};

    struct Blank;

    impl Render for Blank {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            gpui::div()
        }
    }

    /// `home` on `main`; returns its hand-over and file queues.
    fn homed(main: AnyWindowHandle, cx: &mut TestAppContext) -> (mpsc::UnboundedSender<Hand>, mpsc::UnboundedReceiver<PathBuf>) {
        let (sender, hands) = mpsc::unbounded();
        let (queue, asked) = mpsc::unbounded();
        cx.update(|cx| home(Hands { hands, queue }, main, cx));
        (sender, asked)
    }

    fn handed(sender: &mpsc::UnboundedSender<Hand>) -> std::sync::mpsc::Receiver<()> {
        let (taken, took) = std::sync::mpsc::channel();
        sender.unbounded_send(Hand { files: vec![PathBuf::from("x.md")], taken }).expect("sent");
        took
    }

    #[gpui::test]
    fn a_hand_over_raises_the_main_window_then_queues(cx: &mut TestAppContext) {
        let main: AnyWindowHandle = cx.add_window(|_, _| Blank).into();
        let _settings = cx.add_window(|_, _| Blank);
        let (sender, mut asked) = homed(main, cx);
        assert_ne!(cx.update(|cx| cx.active_window()), Some(main), "another window is in front");
        let took = handed(&sender);
        cx.run_until_parked();
        assert_eq!(cx.update(|cx| cx.active_window()), Some(main));
        assert_eq!(asked.try_recv().ok(), Some(PathBuf::from("x.md")));
        assert!(took.try_recv().is_ok(), "the other copy hears it was taken");
    }

    #[gpui::test]
    fn a_hand_over_without_the_main_window_is_refused(cx: &mut TestAppContext) {
        let main: AnyWindowHandle = cx.add_window(|_, _| Blank).into();
        let _settings = cx.add_window(|_, _| Blank);
        let (sender, mut asked) = homed(main, cx);
        main.update(cx, |_, window, _| window.remove_window()).expect("closed");
        cx.run_until_parked();
        let took = handed(&sender);
        cx.run_until_parked();
        assert!(took.try_recv().is_err(), "no answer, so the other copy reports it");
        assert!(asked.try_recv().is_err(), "nothing queued for a gone window");
    }
}
