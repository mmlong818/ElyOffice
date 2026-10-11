//! The built page, live: each package renders offline.

use std::{path::Path, time::Duration};

use ely_gpui_component::theme::{ActiveTheme as _, Mode, Theme};
use ely_platform::AppId;
use ely_web_content::{Event, WebView};
use gpui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, Styled as _,
    Window, div,
};

/// What the page holds, each block scrolled in and mounted.
const CHECK: &str = r#"
(async () => {
  const view = document.querySelector('#document > div:not([hidden])') ?? document;
  const settle = () => new Promise((done) => setTimeout(done, 700));
  const seen = (node) => {
    if (!node || !node.checkVisibility({ opacityProperty: true, visibilityProperty: true })) return false;
    node.scrollIntoView({ block: 'nearest' });
    const box = node.getBoundingClientRect();
    if (box.width < 1 || box.height < 1) return false;
    const top = document.elementFromPoint(box.x + box.width / 2, box.y + Math.min(box.height / 2, 8));
    return Boolean(top && (node.contains(top) || top.contains(node)));
  };
  const blocks = [];
  for (const block of view.querySelectorAll('.milkdown-code-block')) {
    block.scrollIntoView({ block: 'center' });
    await settle();
    const diagram = block.querySelector('.ely-diagram svg');
    const shape = diagram?.querySelector('.node rect, .node polygon');
    const box = diagram?.getBoundingClientRect();
    blocks.push({
      language: block.querySelector('.language-button')?.textContent.trim() ?? '',
      editors: block.querySelectorAll('.cm-editor').length,
      editable: block.querySelector('.cm-content')?.contentEditable ?? 'none',
      code: seen(block.querySelector('.cm-line')),
      math: seen(block.querySelector('.preview .katex')),
      diagram: box ? [Math.round(box.width), Math.round(box.height)] : [0, 0],
      drawn: seen(diagram),
      fill: shape ? getComputedStyle(shape).fill : '',
      first: window.firstFill ?? '',
      failure: seen(block.querySelector('.ely-failure')) ? block.querySelector('.ely-failure').textContent : '',
    });
  }
  document.scrollingElement.scrollTop = 0;
  await settle();
  const root = document.getElementById('document');
  window.elyQuery({
    request: JSON.stringify({
      step: 'STEP',
      mode: root.dataset.mode,
      editable: view.querySelector('.ProseMirror')?.contentEditable ?? 'none',
      title: view.querySelector('.milkdown h1')?.textContent ?? '',
      heading: seen(view.querySelector('.milkdown h1')),
      inline: seen(view.querySelector('.milkdown p .katex')),
      display: seen(view.querySelector('.milkdown p .katex-display')),
      tasks: view.querySelectorAll('li[data-item-type="task"]').length,
      boxes: [...view.querySelectorAll('li[data-item-type="task"]')].map((item) => {
        const mark = getComputedStyle(item, '::before');
        const range = document.createRange();
        range.selectNodeContents(item.querySelector('p'));
        const line = range.getClientRects()[0];
        const top = item.getBoundingClientRect().top + parseFloat(mark.marginTop);
        const size = parseFloat(getComputedStyle(item).fontSize);
        const shown = mark.display !== 'none' && mark.visibility === 'visible' && parseFloat(mark.opacity) > 0;
        // A laid-out box pushes the text 1.5em in.
        const inset = (item.querySelector('p').getBoundingClientRect().left - item.getBoundingClientRect().left) / size;
        const drawn = shown && mark.content !== 'none' && parseFloat(mark.borderTopWidth) > 0 && Math.abs(inset - 1.5) < 0.05;
        return [parseFloat(mark.width) / size, parseFloat(mark.height) / size, top + parseFloat(mark.height) / 2 - (line.top + line.height / 2), drawn];
      }),
      cells: view.querySelectorAll('.milkdown td').length,
      notes: view.querySelectorAll('[data-type="footnote_definition"]').length,
      blocks,
      font: [...document.fonts].some((f) => f.family === 'KaTeX_Main' && f.status === 'loaded'),
      source: seen(view.querySelector(':scope > .cm-editor .cm-line')),
      first: view.querySelector('.cm-line')?.textContent ?? '',
      editors: document.querySelectorAll('#document > div:not([hidden])').length,
      text: view.textContent.slice(0, 40),
      bg: getComputedStyle(document.documentElement).getPropertyValue('--ely-bg').trim(),
    }),
    onSuccess: () => {},
    onFailure: () => {},
  });
})();
"#;

const TEXTS: &str = r#"{"searchLanguage":"s","noResult":"n","copy":"c","preview":"p","loading":"l","edit":"e","hide":"h"}"#;

/// Steps in order; each reports once.
const STEPS: [&str; 13] = [
    "visual", "themed", "preview", "source", "race", "language", "midtheme", "release", "asked",
    "stale", "owner", "find", "look",
];

/// Empty and broken queries: errors, never CodeMirror's own panel.
const FIND: &str = r#"
(async () => {
  const report = {};
  for (const mode of ['source', 'visual']) {
    await ely.open('a{b alpha\n', mode, TEXTS, 1, location.href);
    const empty = { search: '' };
    ely.find(empty);
    ely.step({ spec: empty, forward: true });
    ely.replace({ spec: empty, all: false });
    for (const search of ['a{', '\\a']) ely.find({ search, regexp: true });
    ely.unfind();
    report[mode] = document.querySelectorAll('.cm-panels .cm-search').length;
    await ely.open('\u{1F600} alpha\n', mode, TEXTS, 1, location.href);
    ely.find({ search: '(?=.)', regexp: true });
    ely.unfind();
  }
  await new Promise((done) => setTimeout(done, 300));
  window.elyQuery({ request: JSON.stringify({ step: 'find', panels: report }), onSuccess: () => {}, onFailure: () => {} });
})();
"#;

/// The writing view: dim blocks, narrow serif column, centered caret.
const LOOK: &str = r#"
(async () => {
  const paragraphs = Array.from({ length: 80 }, (_, n) => `Paragraph ${n + 1} of the look.`)
  await ely.open(paragraphs.join('\n\n') + '\n', 'visual', TEXTS, 1, location.href)
  ely.look({ focus: true, typewriter: true, width: 'narrow', font: 'serif', size: 120 })
  ely.reveal({ line: 159, query: '' })
  await new Promise((done) => setTimeout(done, 600))
  const blocks = [...document.querySelectorAll('.milkdown .ProseMirror > *')]
  const dim = blocks.filter((block) => getComputedStyle(block).opacity === '0.35').length
  const bright = document.querySelector('.ely-current')
  const caret = window.getSelection().getRangeAt(0).getBoundingClientRect()
  const page = document.getElementById('document')
  const report = {
    step: 'look',
    blocks: blocks.length,
    dim,
    bright: bright ? getComputedStyle(bright).opacity : 'none',
    centered: Math.abs(caret.top - window.innerHeight / 2) < window.innerHeight / 4,
    width: getComputedStyle(page).maxWidth,
    serif: getComputedStyle(page).fontFamily.includes('serif'),
  }
  ely.look({ focus: false, typewriter: false, width: 'medium', font: 'sans', size: 100 })
  window.elyQuery({ request: JSON.stringify(report), onSuccess: () => {}, onFailure: () => {} })
})();
"#;

/// Picks Rust in the first block's list, once mounted.
const TO_RUST: &str = r#"
(function pick() {
  const block = document.querySelector('#document > div:not([hidden]) .milkdown-code-block');
  const button = block?.querySelector('.language-button');
  if (!button) return requestAnimationFrame(pick);
  button.click();
  setTimeout(() => [...block.querySelectorAll('.language-list-item')]
    .find((item) => item.textContent.trim() === 'Rust').click(), 30);
})();
"#;

/// Says when a diagram waits, drawn not yet.
const PENDING: &str = r#"
(function wait() {
  if (!document.querySelector('.ely-diagram:empty')) return requestAnimationFrame(wait);
  new MutationObserver((_, watch) => {
    const shape = document.querySelector('.ely-diagram svg .node rect');
    if (!shape) return;
    window.firstFill = getComputedStyle(shape).fill;
    watch.disconnect();
  }).observe(document.body, { childList: true, subtree: true });
  window.elyQuery({ request: JSON.stringify({ kind: 'pending' }), onSuccess: () => {}, onFailure: () => {} });
})();
"#;

/// A diagram slow enough to change things mid-draw.
fn big() -> String {
    let mut source = String::from("```mermaid\ngraph TD\n");
    for at in 0..150 {
        source += &format!("  N{at} --> N{}\n  N{at} --> N{}\n", at + 1, (at * 7) % 151);
    }
    serde_json::to_string(&(source + "```\n")).expect("json")
}

/// Short sources, slow draws: blocks mount at once.
fn chains(blocks: usize) -> String {
    let mut source = String::new();
    for block in 0..blocks {
        let side = |name: &str| {
            let nodes: Vec<String> = (0..20).map(|at| format!("{name}{block}x{at}")).collect();
            nodes.join(" & ")
        };
        source += &format!(
            "```mermaid\ngraph LR\n{} --> {}\n```\n\n",
            side("A"),
            side("B")
        );
    }
    serde_json::to_string(&source).expect("json")
}

/// Leaves for source mid-draw; counts long stalls after.
const RELEASE: &str = r#"
(async () => {
  const sleep = (ms) => new Promise((done) => setTimeout(done, ms));
  await ely.open(BIG, 'preview', TEXTS, 1, location.href);
  await sleep(50);
  await ely.open('x', 'source', TEXTS, 1, location.href);
  let last = performance.now();
  let long = 0;
  const tick = setInterval(() => {
    const now = performance.now();
    if (now - last > 500) long += 1;
    last = now;
  }, 20);
  await sleep(7000);
  clearInterval(tick);
  window.elyQuery({ request: JSON.stringify({ step: 'release', long }), onSuccess: () => {}, onFailure: () => {} });
})();
"#;

fn flip(cx: &mut App) {
    let mode = match cx.theme().is_dark() {
        true => Mode::Light,
        false => Mode::Dark,
    };
    Theme::set_mode_now(mode, cx);
}

struct Probe {
    view: Entity<WebView>,
    step: usize,
    /// A fresh page: Mermaid's bundle not yet loaded.
    fresh: bool,
    /// Find reports that carried an error, and every total.
    errors: usize,
    totals: Vec<u64>,
}

impl Probe {
    /// Starts the current step; its check follows.
    fn begin(&mut self, cx: &mut Context<Self>) {
        let markdown =
            serde_json::to_string(include_str!("../../../tests/fixtures/mark/extended.md"))
                .expect("json");
        let step = STEPS[self.step];
        match step {
            "themed" => {
                flip(cx);
                self.check(step, 2500, cx);
            }
            // Two quick shows: only the last may stay.
            "race" => {
                self.view.read(cx).run_js(&format!(
                    "ely.open('# OLD', 'visual', {TEXTS}, 1, location.href); \
                     ely.open('# NEW', 'source', {TEXTS}, 1, location.href)"
                ));
                self.check(step, 2500, cx);
            }
            // Two modes asked at once: the last one shows.
            "asked" => {
                self.view.read(cx).run_js(&format!(
                    "ely.open('# M\\n', 'source', {TEXTS}, 1, location.href) \
                     .then(() => {{ ely.mode('visual'); ely.mode('preview') }})"
                ));
                self.check(step, 3000, cx);
            }
            // A request while visual's shown is unanswered wins.
            "stale" => {
                self.view.read(cx).run_js(&format!(
                    "ely.open('# M\\n', 'source', {TEXTS}, 1, location.href) \
                     .then(() => {{ ely.mode('visual'); ely.mode('preview') }})"
                ));
                self.check(step, 3000, cx);
            }
            // An old answer leaves a newer build's request alone.
            "owner" => {
                self.view.read(cx).run_js(&format!(
                    "ely.open('# M\\n', 'source', {TEXTS}, 1, location.href) \
                     .then(() => ely.mode('visual'))"
                ));
                self.check(step, 3000, cx);
            }
            "find" => self.view.read(cx).run_js(&FIND.replace("TEXTS", TEXTS)),
            "look" => self.view.read(cx).run_js(&LOOK.replace("TEXTS", TEXTS)),
            "release" => {
                let script = RELEASE.replace("BIG", &chains(3)).replace("TEXTS", TEXTS);
                self.view.read(cx).run_js(&script);
            }
            // Changes land while the bundle still loads.
            "language" | "midtheme" if !self.fresh => {
                self.fresh = true;
                self.view.read(cx).run_js("location.reload()");
            }
            "language" | "midtheme" => {
                self.fresh = false;
                let show = format!("ely.open({}, 'visual', {TEXTS}, 1, location.href)", big());
                self.view.read(cx).run_js(&show);
            }
            mode => self.view.read(cx).run_js(&format!(
                "ely.open({markdown}, '{mode}', {TEXTS}, 1, location.href)"
            )),
        }
    }

    /// Checks after `wait`, when drawing is done.
    fn check(&self, step: &'static str, wait: u64, cx: &mut Context<Self>) {
        let view = self.view.clone();
        cx.spawn(async move |_, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(wait))
                .await;
            view.update(cx, |view, _| view.run_js(&CHECK.replace("STEP", step)));
        })
        .detach();
    }

    fn heard(
        &mut self,
        _: &Entity<WebView>,
        event: &Event,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            Event::Loaded => self.begin(cx),
            Event::Query(said, reply) => {
                let read: serde_json::Value = serde_json::from_str(said).expect("json");
                // Some answers wait; meanwhile newer modes are asked.
                let (wait, then) = match (STEPS.get(self.step), read["mode"].as_str()) {
                    (Some(&"stale"), Some("visual")) => (300, "ely.mode('source')"),
                    (Some(&"owner"), Some("visual")) => {
                        (300, "ely.mode('preview'); ely.mode('source')")
                    }
                    (Some(&"owner"), Some("preview")) => (600, ""),
                    _ => (0, ""),
                };
                if !then.is_empty() {
                    self.view.read(cx).run_js(then);
                }
                let reply = reply.clone();
                cx.spawn(async move |_, cx| {
                    cx.background_executor()
                        .timer(Duration::from_millis(wait))
                        .await;
                    reply.ok("");
                })
                .detach();
                if read["kind"] == "shown" {
                    println!("mark: shown {}", read["mode"]);
                    let Some(&step) = STEPS.get(self.step) else {
                        return;
                    };
                    // Mid-draw: a new language, or a new theme.
                    match step {
                        "language" => self.view.read(cx).run_js(TO_RUST),
                        "midtheme" => return self.view.read(cx).run_js(PENDING),
                        _ if step == read["mode"] => {}
                        _ => return,
                    }
                    self.check(step, 5000, cx);
                    return;
                }
                if read["kind"] == "found" {
                    self.errors += read["error"].is_string() as usize;
                    self.totals.extend(read["total"].as_u64());
                    return;
                }
                // Edits report too; the session tests them.
                if ["changed", "text", "outline", "place"]
                    .contains(&read["kind"].as_str().unwrap_or_default())
                {
                    return;
                }
                if read["kind"] == "pending" {
                    // Its draw takes seconds; flip once it has begun.
                    println!("mark: pending");
                    cx.spawn(async move |this, cx| {
                        cx.background_executor()
                            .timer(Duration::from_millis(600))
                            .await;
                        this.update(cx, |probe, cx| {
                            flip(cx);
                            probe.check("midtheme", 6000, cx);
                        })
                        .ok();
                    })
                    .detach();
                    return;
                }
                let mut read = read;
                if read["step"] == "find" {
                    read["errors"] = self.errors.into();
                    read["totals"] = self.totals.clone().into();
                }
                println!("mark: {read}");
                self.step += 1;
                if self.step == STEPS.len() {
                    println!("mark: done");
                    return cx.quit();
                }
                // The first view stays a while for the window's pixels.
                let wait = if self.step == 1 { 3000 } else { 0 };
                cx.spawn(async move |this, cx| {
                    cx.background_executor()
                        .timer(Duration::from_millis(wait))
                        .await;
                    this.update(cx, |probe, cx| probe.begin(cx)).ok();
                })
                .detach();
            }
            Event::Leave(url) => println!("mark: leave {url}"),
            Event::Printed(path, ok) => println!("mark: printed {} {ok}", path.display()),
            Event::Failed(why) => {
                println!("mark: failed {why}");
                cx.quit();
            }
        }
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
    let content = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../engines/content/ely-mark");
    let view = WebView::open(&content.join("index.html"), &content, window, cx).expect("a view");
    let probe = cx.new(|cx| {
        cx.subscribe_in(&view, window, Probe::heard).detach();
        Probe {
            view,
            step: 0,
            fresh: false,
            errors: 0,
            totals: Vec::new(),
        }
    });
    ely_ui::root(app, Some(probe.into()), window, cx)
}

fn main() {
    if let Some(code) = ely_web_content::subprocess() {
        std::process::exit(code);
    }
    ely_ui::run(AppId::Mark, open);
}
