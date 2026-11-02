use std::collections::HashSet;

use base64::Engine as _;
use html5ever::{LocalName, local_name};

use super::{Block, Image, Inline, NESTED, Page, Source, Style, link};
use crate::{
    Part,
    html::{self, Data, Node},
};

pub(super) fn page(text: &str, inline: &[Part]) -> Page {
    let (tree, short) = html::parse(text);
    let nodes = tree.nodes();
    let mut build = Build {
        nodes: &nodes,
        inline,
        remote: Vec::new(),
        seen: HashSet::new(),
        trackers: 0,
    };
    let mut flow = Flow::default();
    build.walk(0, &mut flow, &Context::default());
    flow.end();
    Page {
        blocks: flow.blocks,
        remote: build.remote,
        trackers: build.trackers,
        short,
    }
}

struct Build<'a> {
    nodes: &'a [Node],
    inline: &'a [Part],
    remote: Vec<String>,
    seen: HashSet<String>,
    trackers: usize,
}

/// What encloses a node: styles, a link, its depth.
#[derive(Clone, Default)]
struct Context {
    styles: Vec<Style>,
    link: Option<String>,
    nested: usize,
}

/// Blocks being filled, and the open paragraph.
#[derive(Default)]
struct Flow {
    blocks: Vec<Block>,
    run: Inline,
    heading: Option<u8>,
    /// White space seen since the last word.
    space: bool,
    /// The last word's styles and link.
    last: Context,
}

impl Flow {
    /// HTML collapses white space between words.
    fn text(&mut self, text: &str, context: &Context) {
        let mut word = String::new();
        for ch in text.chars() {
            if ch.is_ascii_whitespace() {
                self.put(&mut word, context);
                self.space = true;
                continue;
            }
            if self.space && !self.run.text.is_empty() && !self.run.text.ends_with('\n') {
                self.put(&mut word, context);
                // A gap takes what both its words share.
                let styles: Vec<Style> = (context.styles.iter())
                    .filter(|style| self.last.styles.contains(style))
                    .copied()
                    .collect();
                let link = (context.link.as_deref()).filter(|_| self.last.link == context.link);
                self.run.push(" ", &styles, link);
            }
            self.space = false;
            word.push(ch);
        }
        self.put(&mut word, context);
    }

    fn put(&mut self, word: &mut String, context: &Context) {
        if word.is_empty() {
            return;
        }
        self.run
            .push(word, &context.styles, context.link.as_deref());
        self.last = context.clone();
        word.clear();
    }

    fn line(&mut self) {
        self.run.trim_end();
        self.run.text.push('\n');
        self.space = false;
    }

    /// Closes the paragraph; blank ones vanish.
    fn end(&mut self) {
        let mut run = std::mem::take(&mut self.run);
        self.space = false;
        run.trim();
        if run.text.is_empty() {
            return;
        }
        self.blocks.push(match self.heading {
            Some(level) => Block::Heading(level, run),
            None => Block::Text(run),
        });
    }

    fn push(&mut self, block: Block) {
        self.end();
        self.blocks.push(block);
    }
}

impl Build<'_> {
    fn walk(&mut self, at: usize, flow: &mut Flow, context: &Context) {
        match &self.nodes[at].data {
            Data::Text(text) => flow.text(text, context),
            Data::Document => self.children(at, flow, context),
            Data::Other => {}
            Data::Element { name, attrs, .. } => {
                self.element(at, &name.local, attrs, flow, context)
            }
        }
    }

    fn children(&mut self, at: usize, flow: &mut Flow, context: &Context) {
        for &child in &self.nodes[at].children {
            self.walk(child, flow, context);
        }
    }

    fn element(
        &mut self,
        at: usize,
        name: &LocalName,
        attrs: &[(LocalName, String)],
        flow: &mut Flow,
        context: &Context,
    ) {
        if !self.shows(at) {
            return;
        }
        let styled = |style| {
            let mut inner = context.clone();
            inner.styles.push(style);
            inner
        };
        match *name {
            local_name!("br") => flow.line(),
            local_name!("img") => self.image(attrs, flow, context),
            local_name!("hr") => flow.push(Block::Rule),
            local_name!("pre") | local_name!("listing") | local_name!("xmp") => {
                let code = self.raw(at);
                let code = code.trim_end_matches(['\n', '\r']);
                if !code.trim().is_empty() {
                    flow.push(Block::Code(code.to_owned()));
                }
            }
            local_name!("blockquote") => self.quote(at, flow, context),
            local_name!("ul") | local_name!("ol") | local_name!("menu") => {
                let start = (*name == local_name!("ol")).then(|| {
                    // HTML integers are 32-bit; larger ones count from 1.
                    let start =
                        value(attrs, "start").and_then(|start| start.trim().parse::<u32>().ok());
                    start.map_or(1, u64::from)
                });
                self.list(at, start, flow, context);
            }
            local_name!("a") => {
                let mut inner = context.clone();
                inner.link = value(attrs, "href").and_then(link);
                self.children(at, flow, &inner);
            }
            local_name!("b") | local_name!("strong") => {
                self.children(at, flow, &styled(Style::Strong))
            }
            local_name!("i") | local_name!("em") | local_name!("cite") | local_name!("var") => {
                self.children(at, flow, &styled(Style::Emphasis))
            }
            local_name!("s") | local_name!("strike") | local_name!("del") => {
                self.children(at, flow, &styled(Style::Strike))
            }
            local_name!("u") | local_name!("ins") => {
                self.children(at, flow, &styled(Style::Underline))
            }
            local_name!("code") | local_name!("tt") | local_name!("kbd") | local_name!("samp") => {
                self.children(at, flow, &styled(Style::Code))
            }
            _ => match heading(name) {
                Some(level) => {
                    flow.end();
                    let outer = flow.heading.replace(level);
                    self.children(at, flow, context);
                    flow.end();
                    flow.heading = outer;
                }
                None if block(name) => {
                    flow.end();
                    self.children(at, flow, context);
                    flow.end();
                }
                None => self.children(at, flow, context),
            },
        }
    }

    fn quote(&mut self, at: usize, flow: &mut Flow, context: &Context) {
        flow.end();
        if context.nested >= NESTED {
            self.children(at, flow, context);
            return flow.end();
        }
        let inner = Context {
            nested: context.nested + 1,
            ..context.clone()
        };
        let mut quoted = Flow::default();
        self.children(at, &mut quoted, &inner);
        quoted.end();
        if !quoted.blocks.is_empty() {
            flow.push(Block::Quote(quoted.blocks));
        }
    }

    /// Each `li` an item; stray content joins the last.
    fn list(&mut self, at: usize, start: Option<u64>, flow: &mut Flow, context: &Context) {
        flow.end();
        if context.nested >= NESTED {
            self.children(at, flow, context);
            return flow.end();
        }
        let inner = Context {
            nested: context.nested + 1,
            ..context.clone()
        };
        let mut items: Vec<Vec<Block>> = Vec::new();
        let mut stray = Flow::default();
        for &child in &self.nodes[at].children {
            let item = matches!(
                &self.nodes[child].data,
                Data::Element { name, .. } if name.local == local_name!("li")
            );
            if !item {
                self.walk(child, &mut stray, &inner);
                continue;
            }
            if !self.shows(child) {
                continue;
            }
            stray.end();
            if let Some(last) = items.last_mut() {
                last.append(&mut stray.blocks);
            }
            let mut each = Flow::default();
            self.children(child, &mut each, &inner);
            each.end();
            items.push(each.blocks);
        }
        stray.end();
        match items.last_mut() {
            Some(last) => last.append(&mut stray.blocks),
            None => items.extend((!stray.blocks.is_empty()).then_some(stray.blocks)),
        }
        if !items.is_empty() {
            flow.push(Block::List { start, items });
        }
    }

    fn image(&mut self, attrs: &[(LocalName, String)], flow: &mut Flow, context: &Context) {
        let source = match value(attrs, "src") {
            Some(src) => self.source(src),
            None => Source::Broken,
        };
        if let Source::Remote(url) = &source {
            if pixel(attrs) {
                self.trackers += 1;
                return;
            }
            if self.seen.insert(url.clone()) {
                self.remote.push(url.clone());
            }
        }
        let alt = value(attrs, "alt").unwrap_or_default().trim().to_owned();
        flow.push(Block::Image(Image {
            alt,
            source,
            link: context.link.clone(),
        }));
    }

    fn source(&self, src: &str) -> Source {
        let src = src.trim();
        let lower = src.to_ascii_lowercase();
        if lower.starts_with("http://") || lower.starts_with("https://") {
            return Source::Remote(src.to_owned());
        }
        if src.starts_with("//") {
            return Source::Remote(format!("https:{src}"));
        }
        if lower.starts_with("cid:") {
            // RFC 2392: the address is URL-encoded.
            let id = unescaped(&src[4..]);
            let found = self.inline.iter().position(|part| {
                part.content_id
                    .as_deref()
                    .is_some_and(|own| own.trim_matches(['<', '>']) == id)
            });
            return found.map_or(Source::Broken, Source::Inline);
        }
        if lower.starts_with("data:") {
            return data(&src[5..]).unwrap_or(Source::Broken);
        }
        Source::Broken
    }

    /// Text under `at` as written, for `pre`.
    fn raw(&self, at: usize) -> String {
        let mut out = String::new();
        let mut stack = vec![at];
        while let Some(next) = stack.pop() {
            match &self.nodes[next].data {
                Data::Text(text) => out.push_str(text),
                Data::Element { name, attrs, .. } if html::hidden(&name.local) || unseen(attrs) => {
                }
                Data::Element { name, .. } if *name.local == *"br" => out.push('\n'),
                _ => stack.extend(self.nodes[next].children.iter().rev()),
            }
        }
        out
    }

    /// False when hidden; its web pictures count as kept back.
    fn shows(&mut self, at: usize) -> bool {
        let Data::Element { name, attrs, .. } = &self.nodes[at].data else {
            return true;
        };
        if !html::hidden(&name.local) && !unseen(attrs) {
            return true;
        }
        self.trackers += self.remote_under(at);
        false
    }

    /// Remote pictures inside a hidden subtree.
    fn remote_under(&self, at: usize) -> usize {
        let mut count = 0;
        let mut stack = vec![at];
        while let Some(next) = stack.pop() {
            if let Data::Element { name, attrs, .. } = &self.nodes[next].data {
                let src = value(attrs, "src").map(|src| self.source(src));
                if name.local == local_name!("img") && matches!(src, Some(Source::Remote(_))) {
                    count += 1;
                }
            }
            stack.extend(&self.nodes[next].children);
        }
        count
    }
}

fn value<'a>(attrs: &'a [(LocalName, String)], name: &str) -> Option<&'a str> {
    attrs
        .iter()
        .find(|(key, _)| **key == *name)
        .map(|(_, value)| value.as_str())
}

/// Hidden by attribute or inline style.
fn unseen(attrs: &[(LocalName, String)]) -> bool {
    if value(attrs, "hidden").is_some() {
        return true;
    }
    let Some(style) = value(attrs, "style") else {
        return false;
    };
    let mut collapsed = (false, false);
    for declaration in style.split(';') {
        let Some((key, value)) = declaration.split_once(':') else {
            continue;
        };
        let key = key.trim().to_ascii_lowercase();
        let value = value.trim().to_ascii_lowercase();
        let value = match value.strip_suffix("important").map(str::trim_end) {
            Some(rest) if rest.ends_with('!') => rest[..rest.len() - 1].trim_end(),
            _ => value.as_str(),
        };
        match (key.as_str(), value) {
            ("display", "none") | ("visibility", "hidden") | ("mso-hide", "all") => return true,
            ("max-height", "0" | "0px") => collapsed.0 = true,
            ("overflow", "hidden") => collapsed.1 = true,
            _ => {}
        }
    }
    collapsed == (true, true)
}

/// A one- or two-pixel picture is a tracker.
fn pixel(attrs: &[(LocalName, String)]) -> bool {
    let small = |name| {
        value(attrs, name).is_some_and(|size| {
            let size = size.trim().trim_end_matches("px");
            size.parse::<f32>().is_ok_and(|size| size <= 2.0)
        })
    };
    small("width") && small("height")
}

fn heading(name: &LocalName) -> Option<u8> {
    match *name {
        local_name!("h1") => Some(1),
        local_name!("h2") => Some(2),
        local_name!("h3") => Some(3),
        local_name!("h4") => Some(4),
        local_name!("h5") => Some(5),
        local_name!("h6") => Some(6),
        _ => None,
    }
}

/// Elements that break the line around them.
fn block(name: &LocalName) -> bool {
    matches!(
        *name,
        local_name!("p")
            | local_name!("div")
            | local_name!("section")
            | local_name!("article")
            | local_name!("header")
            | local_name!("footer")
            | local_name!("main")
            | local_name!("aside")
            | local_name!("nav")
            | local_name!("center")
            | local_name!("address")
            | local_name!("figure")
            | local_name!("figcaption")
            | local_name!("form")
            | local_name!("fieldset")
            | local_name!("details")
            | local_name!("summary")
            | local_name!("dl")
            | local_name!("dt")
            | local_name!("dd")
            | local_name!("li")
            | local_name!("table")
            | local_name!("caption")
            | local_name!("thead")
            | local_name!("tbody")
            | local_name!("tfoot")
            | local_name!("tr")
            | local_name!("td")
            | local_name!("th")
            | local_name!("body")
            | local_name!("html")
    )
}

/// `%XX` decoded; a broken escape stays.
fn unescaped(text: &str) -> String {
    let bytes = text.as_bytes();
    let (mut out, mut at) = (Vec::with_capacity(bytes.len()), 0);
    while at < bytes.len() {
        let hex = bytes
            .get(at + 1..at + 3)
            .and_then(|pair| std::str::from_utf8(pair).ok())
            .and_then(|pair| u8::from_str_radix(pair, 16).ok());
        match (bytes[at], hex) {
            (b'%', Some(byte)) => (out.push(byte), at += 3),
            (byte, _) => (out.push(byte), at += 1),
        };
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// A base64 picture after `data:`.
fn data(rest: &str) -> Option<Source> {
    let (head, payload) = rest.split_once(',')?;
    let content_type = head.strip_suffix(";base64")?.trim().to_ascii_lowercase();
    if !content_type.starts_with("image/") {
        return None;
    }
    let payload: String = payload.chars().filter(|ch| !ch.is_whitespace()).collect();
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(payload)
        .ok()?;
    Some(Source::Data {
        content_type,
        bytes,
    })
}
