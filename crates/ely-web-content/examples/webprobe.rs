//! A live WebView: input, fence, failures, release, paint.

use std::{
    net::{TcpListener, UdpSocket},
    path::{Path, PathBuf},
    time::Duration,
};

use ely_platform::AppId;
use ely_web_content::{Event, WebView};
use gpui::{
    App, AppContext as _, AsyncWindowContext, Bounds, Context, ElementInputHandler, Entity,
    Focusable as _, InputHandler as _, IntoElement, KeyUpEvent, Keystroke, ParentElement as _,
    PlatformInput, Render, Styled as _, WeakEntity, Window, div,
};

/// How many views open and close to show their release.
const CHURN: usize = 20;
/// Views dropped before the pump makes their browsers.
const UNMADE: usize = 5;
/// Views dropped within 3 ms steps of opening.
const EARLY: u64 = 10;

struct Probe {
    view: Entity<WebView>,
    /// Pages beside the main one, each with its own test.
    others: Vec<Entity<WebView>>,
    /// The ratio before zoom, to compare after.
    before: Option<f64>,
    /// Fenced off: nothing may reach it.
    server: TcpListener,
    /// WebRTC's way out, by UDP; nothing may reach it either.
    udp: UdpSocket,
    churned: usize,
    crashed: bool,
    fixtures: PathBuf,
    work: PathBuf,
}

fn typed(text: &str) -> Keystroke {
    Keystroke {
        key_char: Some(text.into()),
        ..Keystroke::parse(text).expect("a keystroke")
    }
}

/// Waits, then runs `then` with the probe and its window.
fn later(
    this: WeakEntity<Probe>,
    ms: u64,
    window: &mut Window,
    cx: &mut Context<Probe>,
    then: impl FnOnce(&mut Probe, &mut Window, &mut Context<Probe>) + 'static,
) {
    cx.spawn_in(window, async move |_, cx: &mut AsyncWindowContext| {
        cx.background_executor()
            .timer(Duration::from_millis(ms))
            .await;
        let _ = this.update_in(cx, then);
    })
    .detach();
}

impl Probe {
    fn main_heard(
        &mut self,
        _: &Entity<WebView>,
        event: &Event,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            // Held so the gate sees the page drawn again.
            Event::Loaded if self.crashed => {
                println!("probe: recovered");
                let this = cx.weak_entity();
                later(this, 4000, window, cx, |_, _, cx| cx.quit());
            }
            Event::Loaded => {
                let port = self.server.local_addr().expect("an address").port();
                let image = format!(
                    "new Image().src = 'http://127.0.0.1:{port}/pixel?x=1'; \
                     try {{ new WebSocket('ws://127.0.0.1:{port}/socket'); }} catch (e) {{}} \
                     try {{ const peer = new RTCPeerConnection({{ iceServers: \
                       [{{ urls: 'stun:127.0.0.1:{udp}' }}] }}); peer.createDataChannel('x'); \
                       peer.createOffer().then((offer) => peer.setLocalDescription(offer)); \
                     }} catch (e) {{}}",
                    udp = self.udp.local_addr().expect("an address").port()
                );
                self.view.read(cx).run_js(&image);
                self.view.read(cx).run_js("report('loaded')");
            }
            Event::Query(said, reply) => {
                reply.ok("ok");
                let read: serde_json::Value = serde_json::from_str(said).expect("json");
                self.step(&read, window, cx);
            }
            Event::Leave(url) => println!("probe: leave {url}"),
            Event::Printed(path, ok) => println!("probe: printed {} {ok}", path.display()),
            // Recovery comes from the shown Reload button.
            Event::Failed(why) => {
                println!("probe: main failed");
                eprintln!("probe: because {why}");
                self.crashed = true;
            }
        }
    }

    fn step(&mut self, read: &serde_json::Value, window: &mut Window, cx: &mut Context<Self>) {
        let this = cx.weak_entity();
        match read["step"].as_str() {
            Some("loaded") => {
                self.before = read["ratio"].as_f64();
                println!("probe: outside={}", read["outside"]);
                self.edit(window, cx);
            }
            Some("edited") => {
                println!("probe: value={}", read["value"].as_str().unwrap_or("?"));
                println!(
                    "probe: focused={}",
                    read["focused"].as_str().unwrap_or("none")
                );
                println!("probe: fg={}", read["fg"].as_str().unwrap_or(""));
                let reached = self.server.accept().is_ok();
                println!(
                    "probe: network={}",
                    if reached { "reached" } else { "refused" }
                );
                let heard = self.udp.recv(&mut [0u8; 2048]).is_ok();
                println!("probe: udp={}", if heard { "reached" } else { "silent" });
                println!("probe: rtc={}", read["rtc"].as_str().unwrap_or("?"));
                println!(
                    "probe: network={}",
                    if reached { "reached" } else { "refused" }
                );
                self.view.read(cx).set_zoom(1.25);
                later(this, 800, window, cx, |probe, _, cx| {
                    probe.view.read(cx).run_js("report('zoomed')");
                });
            }
            Some("zoomed") => {
                let zoom = read["ratio"].as_f64().unwrap_or(0.0) / self.before.unwrap_or(1.0);
                println!("probe: zoom={zoom:.2}");
                // A page cannot leave its bundled folder.
                self.view
                    .read(cx)
                    .run_js("location.href = 'https://example.com/'");
                later(this, 800, window, cx, |probe, _, cx| {
                    probe.view.read(cx).run_js("report('stayed')");
                });
            }
            Some("stayed") => {
                println!("probe: stayed");
                self.churn(window, cx);
            }
            other => println!("probe: unknown step {other:?}"),
        }
    }

    /// Compose 你好, commit it, then type A.
    fn edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let view = self.view.clone();
        // Input runs outside this update; GPUI redraws during it.
        cx.spawn_in(window, async move |_, cx| {
            let typed_in = cx.update(|window, cx| {
                let focus = view.read(cx).focus_handle(cx);
                window.focus(&focus, cx);
                let mut ime = ElementInputHandler::new(Bounds::default(), view.clone());
                ime.replace_and_mark_text_in_range(None, "ni", Some(2..2), window, cx);
                ime.replace_and_mark_text_in_range(Some(0..2), "你好", None, window, cx);
                ime.replace_text_in_range(None, "你好", window, cx);
                let stroke = typed("a");
                window.dispatch_keystroke(stroke.clone(), cx);
                window.dispatch_event(PlatformInput::KeyUp(KeyUpEvent { keystroke: stroke }), cx);
            });
            if typed_in.is_err() {
                return;
            }
            cx.background_executor()
                .timer(Duration::from_millis(500))
                .await;
            view.update(cx, |view, _| view.run_js("report('edited')"));
        })
        .detach();
    }

    /// Opens and drops pages; each must free its memory.
    fn churn(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let page = self.fixtures.join("probe.html");
        if self.churned == CHURN {
            println!("probe: closed {CHURN}");
            for _ in 0..UNMADE {
                drop(WebView::open(&page, &self.fixtures, window, cx).expect("a view"));
            }
            for step in 0..EARLY {
                let view = WebView::open(&page, &self.fixtures, window, cx).expect("a view");
                self.others.push(view.clone());
                later(cx.weak_entity(), step * 3, window, cx, move |probe, _, _| {
                    probe.others.retain(|other| *other != view);
                });
            }
            println!("probe: dropped {UNMADE} unmade, {EARLY} early");
            let this = cx.weak_entity();
            later(this, 2000, window, cx, |_, _, _| {
                // The log marks what freed before the hold.
                log::info!("probe: hold");
                println!("probe: hold");
            });
            return;
        }
        self.churned += 1;
        let view = WebView::open(&page, &self.fixtures, window, cx).expect("a view");
        cx.subscribe_in(&view, window, |probe, view, event, window, cx| {
            if matches!(event, Event::Loaded) {
                probe.others.retain(|other| other != view);
                probe.churn(window, cx);
            }
        })
        .detach();
        self.others.push(view);
    }

    /// A page named like a URL, one gone, one inert.
    fn edges(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let page = self.fixtures.join("inert.html");
        let inert = WebView::open_inert(&page, &self.fixtures, window, cx).expect("a view");
        cx.subscribe_in(&inert, window, |_, _, event, _, _| match event {
            Event::Loaded => println!("probe: inert loaded"),
            Event::Query(_, reply) => {
                println!("probe: inert ran");
                reply.ok("");
            }
            _ => {}
        })
        .detach();
        // The same page with scripts: it must run.
        let scripted = WebView::open(&page, &self.fixtures, window, cx).expect("a view");
        cx.subscribe_in(&scripted, window, |_, _, event, window, cx| {
            if let Event::Query(_, reply) = event {
                println!("probe: scripted ran");
                reply.ok("");
                later(cx.weak_entity(), 2000, window, cx, |_, _, _| println!("probe: inert checked"));
            }
        })
        .detach();
        self.others.extend([inert, scripted]);
        let hash = self.work.join("hash");
        std::fs::create_dir_all(&hash).expect("makes");
        std::fs::copy(
            self.fixtures.join("probe.html"),
            hash.join("name#part.html"),
        )
        .expect("copies");
        let named = WebView::open(&hash.join("name#part.html"), &hash, window, cx).expect("a view");
        cx.subscribe_in(&named, window, |_, _, event, _, _| match event {
            Event::Loaded => println!("probe: hash loaded"),
            Event::Failed(_) => println!("probe: hash failed"),
            _ => {}
        })
        .detach();
        let gone = self.work.join("gone");
        std::fs::create_dir_all(&gone).expect("makes");
        let page = gone.join("page.html");
        std::fs::copy(self.fixtures.join("probe.html"), &page).expect("copies");
        let missing = WebView::open(&page, &gone, window, cx).expect("a view");
        std::fs::remove_file(&page).expect("removes");
        cx.subscribe_in(&missing, window, |_, _, event, _, _| match event {
            Event::Loaded => println!("probe: missing loaded"),
            Event::Failed(_) => println!("probe: missing failed"),
            _ => {}
        })
        .detach();
        self.others.extend([named, missing]);
    }
}

impl Render for Probe {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(self.view.clone())
    }
}

fn open(app: AppId, window: &mut Window, cx: &mut App) -> Entity<ely_ui::Root> {
    let work = tempfile::tempdir().expect("a temp dir").keep();
    ely_web_content::start(&work, cx).expect("CEF starts");
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/web");
    let fixtures = std::fs::canonicalize(fixtures).expect("fixtures");
    let view = WebView::open(&fixtures.join("probe.html"), &fixtures, window, cx).expect("a view");
    let server = TcpListener::bind("127.0.0.1:0").expect("a port");
    server.set_nonblocking(true).expect("nonblocking");
    let udp = UdpSocket::bind("127.0.0.1:0").expect("a port");
    udp.set_nonblocking(true).expect("nonblocking");
    let probe = cx.new(|cx| {
        cx.subscribe_in(&view, window, Probe::main_heard).detach();
        let mut probe = Probe {
            view,
            others: Vec::new(),
            before: None,
            server,
            udp,
            churned: 0,
            crashed: false,
            fixtures,
            work,
        };
        probe.edges(window, cx);
        probe
    });
    ely_ui::root(app, Some(probe.into()), window, cx)
}

fn main() {
    if let Some(code) = ely_web_content::subprocess() {
        std::process::exit(code);
    }
    ely_ui::run(AppId::Mark, open);
}
