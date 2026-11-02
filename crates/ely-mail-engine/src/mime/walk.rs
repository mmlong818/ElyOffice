use mail_parser::{
    Encoding, Message, MessagePart, MimeHeaders, PartType,
    decoders::{base64::base64_decode, quoted_printable::quoted_printable_decode},
};

use super::{Part, Shown};
use crate::{Error, Result};

/// Deeper trees are refused before the walk.
const DEPTH: usize = 64;

/// What a MIME tree shows: bodies, `cid:` targets, attachments.
#[derive(Default)]
pub(super) struct Shape {
    pub text: Vec<String>,
    pub html: Vec<String>,
    pub shown: Vec<Shown>,
    pub inline: Vec<Part>,
    pub attachments: Vec<Part>,
}

pub(super) fn shape(message: &Message) -> Shape {
    let mut shape = Shape::default();
    // RFC 3501 §6.4.5: a lone body is part 1.
    let root = match message.parts[0].content_type() {
        Some(kind) if kind.c_type.eq_ignore_ascii_case("multipart") => Vec::new(),
        _ => vec![1],
    };
    Walk { message }.body(0, &root, &mut shape);
    shape
}

/// Iterative, so no tree can exhaust the stack.
pub(super) fn depth(message: &Message) -> Result<()> {
    let mut stack = vec![(message, 0, 1)];
    while let Some((message, id, level)) = stack.pop() {
        if level > DEPTH {
            return Err(Error::Mime(format!("MIME parts nest deeper than {DEPTH}")));
        }
        match &message.parts[id as usize].body {
            PartType::Multipart(children) => {
                stack.extend(children.iter().map(|&child| (message, child, level + 1)));
            }
            PartType::Message(inner) => stack.push((inner, 0, level + 1)),
            _ => {}
        }
    }
    Ok(())
}

/// Drops a deep tree without recursion.
pub(super) fn dispose(message: Message) {
    let mut stack = vec![message];
    while let Some(mut message) = stack.pop() {
        for part in &mut message.parts {
            if let PartType::Message(inner) = std::mem::take(&mut part.body) {
                stack.push(inner);
            }
        }
    }
}

/// RFC 6531 §3.4: 8-bit header bytes at any depth.
pub(super) fn utf8_headers(message: &Message) -> bool {
    let mut stack = vec![message];
    while let Some(message) = stack.pop() {
        // Part offsets index the whole buffer.
        let raw = message.raw_message.as_ref();
        for part in &message.parts {
            let header = &raw[part.offset_header as usize..part.offset_body as usize];
            if !header.is_ascii() {
                return true;
            }
            // Encoded parts carry no 8-bit bytes.
            match &part.body {
                PartType::Message(inner) if part.encoding == Encoding::None => stack.push(inner),
                _ => {}
            }
        }
    }
    false
}

struct Walk<'a, 'x> {
    message: &'a Message<'x>,
}

impl Walk<'_, '_> {
    fn part(&self, id: u32) -> &MessagePart<'_> {
        &self.message.parts[id as usize]
    }

    fn body(&self, id: u32, path: &[u32], out: &mut Shape) {
        let part = self.part(id);
        // RFC 2183 §2.8, §2.9: the whole entity.
        if !inline(part) {
            out.attachments.push(self.leaf(part, path));
            return;
        }
        self.content(id, path, out, true);
    }

    /// A related root is no `cid:` target.
    fn content(&self, id: u32, path: &[u32], out: &mut Shape, target: bool) {
        let part = self.part(id);
        match &part.body {
            PartType::Multipart(children) => {
                self.target(part, path, target, out);
                match multipart(part).as_str() {
                    "alternative" => self.alternative(children, path, out),
                    "related" => self.related(part, children, path, out),
                    _ => (children.iter().zip(1..))
                        .for_each(|(&child, n)| self.body(child, &under(path, n), out)),
                }
            }
            PartType::Text(text) if plain(part) => {
                out.text.push(text.to_string());
                out.shown.push(Shown::Text(text.to_string()));
                self.target(part, path, target, out);
            }
            PartType::Html(html) => {
                out.html.push(html.to_string());
                out.shown.push(Shown::Html(html.to_string()));
                self.target(part, path, target, out);
            }
            _ => {
                let leaf = self.leaf(part, path);
                let message = matches!(part.body, PartType::Message(_));
                match leaf.content_id {
                    Some(_) if !message => out.inline.push(leaf),
                    _ => out.attachments.push(leaf),
                }
            }
        }
    }

    /// RFC 2392: any part with a Content-ID.
    fn target(&self, part: &MessagePart, path: &[u32], target: bool, out: &mut Shape) {
        if target && part.content_id().is_some() {
            out.inline.push(self.leaf(part, path));
        }
    }

    /// RFC 2046 §5.1.4: the last supported version wins.
    fn alternative(&self, children: &[u32], path: &[u32], out: &mut Shape) {
        let versions: Vec<Shape> = (children.iter().zip(1..))
            .map(|(&child, n)| {
                let mut version = Shape::default();
                self.body(child, &under(path, n), &mut version);
                version
            })
            .collect();
        let text = versions.iter().rposition(|v| !v.text.is_empty());
        let html = versions.iter().rposition(|v| !v.html.is_empty());
        // §12.4: the reader reads HTML first.
        let read = html.or(text);
        for (at, version) in versions.into_iter().enumerate() {
            let bodiless = version.text.is_empty() && version.html.is_empty();
            if Some(at) != text && Some(at) != html && !bodiless {
                continue;
            }
            if Some(at) == read {
                out.shown.extend(version.shown);
            }
            if Some(at) == text {
                out.text.extend(version.text);
            }
            if Some(at) == html {
                out.html.extend(version.html);
            }
            out.inline.extend(version.inline);
            out.attachments.extend(version.attachments);
        }
    }

    /// RFC 2387: `start` names the root; §4 voids dispositions.
    fn related(&self, part: &MessagePart, children: &[u32], path: &[u32], out: &mut Shape) {
        let start = part
            .content_type()
            .and_then(|kind| kind.attribute("start"))
            .map(|start| start.trim().trim_start_matches('<').trim_end_matches('>'));
        let root = match start {
            None => 0,
            Some(start) => children
                .iter()
                .position(|&child| self.part(child).content_id() == Some(start))
                .unwrap_or_else(|| {
                    log::warn!("mail: related start names no part; first part is the root");
                    0
                }),
        };
        for (at, &child) in children.iter().enumerate() {
            let path = under(path, at as u32 + 1);
            if at == root {
                self.content(child, &path, out, false);
                continue;
            }
            // RFC 2387 §6.1: a resource is one entity.
            let leaf = self.leaf(self.part(child), &path);
            match leaf.content_id {
                Some(_) => out.inline.push(leaf),
                None => out.attachments.push(leaf),
            }
        }
    }

    fn leaf(&self, part: &MessagePart, path: &[u32]) -> Part {
        let kind = part.content_type();
        let content_type = kind.map_or_else(
            // RFC 2046 §5.1.5: digest children are messages.
            || match part.body {
                PartType::Message(_) => "message/rfc822".to_owned(),
                _ => "text/plain".to_owned(),
            },
            |kind| match &kind.c_subtype {
                Some(subtype) => format!("{}/{subtype}", kind.c_type),
                None => kind.c_type.to_string(),
            },
        );
        let params = kind
            .and_then(|kind| kind.attributes())
            .into_iter()
            .flatten()
            .filter(|attr| !attr.name.eq_ignore_ascii_case("name"))
            .map(|attr| (attr.name.to_ascii_lowercase(), attr.value.to_string()))
            .collect();
        Part {
            name: part.attachment_name().map(str::to_owned),
            content_type: content_type.to_ascii_lowercase(),
            params,
            content_id: part.content_id().map(str::to_owned),
            bytes: self.octets(part),
            section: Some(section(path)),
        }
    }

    /// Transfer-decoded bytes; text keeps its own charset.
    fn octets(&self, part: &MessagePart) -> Vec<u8> {
        let raw = self.message.raw_message();
        let body = &raw[part.offset_body as usize..part.offset_end as usize];
        match &part.body {
            PartType::Binary(bytes) | PartType::InlineBinary(bytes) => bytes.to_vec(),
            PartType::Message(message) => message.raw_message().to_vec(),
            PartType::Multipart(_) => {
                raw[part.offset_header as usize..part.offset_end as usize].to_vec()
            }
            PartType::Text(_) | PartType::Html(_) => {
                let decoded = match part.encoding {
                    Encoding::None => Some(body.to_vec()),
                    Encoding::QuotedPrintable => quoted_printable_decode(body),
                    Encoding::Base64 => base64_decode(body),
                };
                decoded.unwrap_or_else(|| {
                    log::warn!("mail: a text attachment kept its encoded bytes");
                    body.to_vec()
                })
            }
        }
    }
}

fn under(path: &[u32], n: u32) -> Vec<u32> {
    let mut path = path.to_vec();
    path.push(n);
    path
}

/// The IMAP part number; a whole body is `TEXT`.
fn section(path: &[u32]) -> String {
    if path.is_empty() {
        return "TEXT".into();
    }
    let numbers: Vec<String> = path.iter().map(u32::to_string).collect();
    numbers.join(".")
}

/// RFC 2183 §2.8: unknown types are attachments.
fn inline(part: &MessagePart) -> bool {
    part.content_disposition()
        .is_none_or(|disposition| disposition.c_type.eq_ignore_ascii_case("inline"))
}

fn multipart(part: &MessagePart) -> String {
    part.content_type()
        .and_then(|kind| kind.c_subtype.as_deref())
        .unwrap_or_default()
        .to_ascii_lowercase()
}

/// RFC 2045 §5.2: no Content-Type is text/plain.
fn plain(part: &MessagePart) -> bool {
    part.content_type().is_none_or(|kind| {
        kind.c_subtype
            .as_deref()
            .is_none_or(|subtype| subtype.eq_ignore_ascii_case("plain"))
    })
}
