//! Text without attachments, from BODYSTRUCTURE (§12.3).

use std::collections::HashMap;

use imap_proto::{BodyParams, BodyStructure, MessageSection, SectionPath};

use crate::{Error, Result};

/// A message's MIME tree; paths are IMAP part numbers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Node {
    /// `body` parts come down; the rest wait for a request.
    Leaf { path: Vec<u32>, body: bool },
    Multi {
        path: Vec<u32>,
        boundary: String,
        children: Vec<Node>,
    },
}

impl Node {
    /// The reader's rules: `mime/walk.rs`.
    pub(super) fn read(structure: &BodyStructure) -> Result<Self> {
        match structure {
            BodyStructure::Multipart { .. } => body(structure, Vec::new()),
            single => Ok(Node::Leaf {
                path: vec![1],
                body: !attachment(single) && shown(single),
            }),
        }
    }

    /// True when nothing is left out.
    pub fn complete(&self) -> bool {
        match self {
            Node::Leaf { body, .. } => *body,
            Node::Multi { children, .. } => children.iter().all(Node::complete),
        }
    }

    /// Part numbers left out, as `2` or `1.3`.
    pub fn missing(&self) -> Vec<String> {
        let mut out = Vec::new();
        self.walk(&mut |node| {
            if let Node::Leaf { path, body: false } = node {
                out.push(number(path));
            }
        });
        out
    }

    /// The FETCH sections that rebuild it.
    pub fn sections(&self) -> Vec<String> {
        let mut out = vec!["HEADER".to_owned()];
        if let Node::Leaf { body, .. } = self {
            if *body {
                out.push("1".into());
            }
            return out;
        }
        self.walk(&mut |node| match node {
            Node::Leaf { path, body } => {
                out.push(mime(path));
                if *body {
                    out.push(number(path));
                }
            }
            Node::Multi { path, .. } if !path.is_empty() => out.push(mime(path)),
            Node::Multi { .. } => {}
        });
        out
    }

    /// The message with left-out parts empty.
    pub fn build(&self, got: &HashMap<String, Vec<u8>>) -> Result<Vec<u8>> {
        let mut out = header(take(got, "HEADER")?);
        match self {
            Node::Leaf { body: true, .. } => out.extend_from_slice(take(got, "1")?),
            Node::Leaf { body: false, .. } => {}
            multi => content(multi, got, &mut out)?,
        }
        Ok(out)
    }

    fn walk(&self, each: &mut impl FnMut(&Node)) {
        each(self);
        if let Node::Multi { children, .. } = self {
            for child in children {
                child.walk(each);
            }
        }
    }
}

/// The key a FETCH section comes back under.
pub(super) fn section_key(section: &Option<SectionPath>) -> String {
    let named = |part: &MessageSection| match part {
        MessageSection::Header => "HEADER",
        MessageSection::Mime => "MIME",
        MessageSection::Text => "TEXT",
    };
    match section {
        None => String::new(),
        Some(SectionPath::Full(part)) => named(part).to_owned(),
        Some(SectionPath::Part(path, None)) => number(path),
        Some(SectionPath::Part(path, Some(part))) => format!("{}.{}", number(path), named(part)),
    }
}

/// RFC 2183 §2.8, §2.9: the whole entity waits.
fn body(structure: &BodyStructure, path: Vec<u32>) -> Result<Node> {
    if attachment(structure) {
        return Ok(Node::Leaf { path, body: false });
    }
    inside(structure, path)
}

fn inside(structure: &BodyStructure, path: Vec<u32>) -> Result<Node> {
    let BodyStructure::Multipart { common, bodies, .. } = structure else {
        return Ok(Node::Leaf {
            body: shown(structure),
            path,
        });
    };
    let boundary = param(&common.ty.params, "boundary")
        .ok_or_else(|| Error::Protocol("a multipart came without a boundary".into()))?;
    let related = common.ty.subtype.eq_ignore_ascii_case("related");
    let root = related.then(|| root(&common.ty.params, bodies));
    let children = bodies
        .iter()
        .zip(1..)
        .enumerate()
        .map(|(at, (child, number))| {
            let mut next = path.clone();
            next.push(number);
            match root {
                None => body(child, next),
                Some(Root::At(root)) if root == at => inside(child, next),
                Some(Root::Unseen) if multipart(child) => inside(child, next),
                // RFC 2387 §4, §6.1: one entity, no disposition.
                Some(_) => Ok(Node::Leaf {
                    body: id(child).is_some(),
                    path: next,
                }),
            }
        })
        .collect::<Result<_>>()?;
    Ok(Node::Multi {
        path,
        boundary: boundary.to_owned(),
        children,
    })
}

/// The related root, or one hidden in a multipart.
#[derive(Clone, Copy)]
enum Root {
    At(usize),
    Unseen,
}

/// RFC 2387: `start` names the root, else the first.
fn root(params: &BodyParams, bodies: &[BodyStructure]) -> Root {
    let Some(start) = param(params, "start") else {
        return Root::At(0);
    };
    if let Some(at) = bodies
        .iter()
        .position(|child| id(child).is_some_and(|id| bare(id) == bare(start)))
    {
        return Root::At(at);
    }
    if bodies.iter().any(multipart) {
        return Root::Unseen;
    }
    log::warn!("mail: related start names no part; first part is the root");
    Root::At(0)
}

fn multipart(structure: &BodyStructure) -> bool {
    matches!(structure, BodyStructure::Multipart { .. })
}

/// Plain and HTML text, and `cid:` targets.
fn shown(structure: &BodyStructure) -> bool {
    match structure {
        BodyStructure::Text { common, .. }
            if ["plain", "html"]
                .iter()
                .any(|kind| common.ty.subtype.eq_ignore_ascii_case(kind)) =>
        {
            true
        }
        BodyStructure::Text { .. } | BodyStructure::Basic { .. } => id(structure).is_some(),
        BodyStructure::Message { .. } | BodyStructure::Multipart { .. } => false,
    }
}

/// RFC 2183 §2.8: unknown dispositions are attachments.
fn attachment(structure: &BodyStructure) -> bool {
    let common = match structure {
        BodyStructure::Basic { common, .. }
        | BodyStructure::Text { common, .. }
        | BodyStructure::Message { common, .. }
        | BodyStructure::Multipart { common, .. } => common,
    };
    common
        .disposition
        .as_ref()
        .is_some_and(|it| !it.ty.eq_ignore_ascii_case("inline"))
}

fn id<'a>(structure: &'a BodyStructure) -> Option<&'a str> {
    match structure {
        BodyStructure::Basic { other, .. }
        | BodyStructure::Text { other, .. }
        | BodyStructure::Message { other, .. } => other.id.as_deref(),
        BodyStructure::Multipart { .. } => None,
    }
}

fn bare(id: &str) -> &str {
    id.trim().trim_start_matches('<').trim_end_matches('>')
}

fn param<'a>(params: &'a BodyParams, name: &str) -> Option<&'a str> {
    params
        .iter()
        .flatten()
        .find(|(key, _)| key.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.as_ref())
}

fn content(node: &Node, got: &HashMap<String, Vec<u8>>, out: &mut Vec<u8>) -> Result<()> {
    match node {
        Node::Leaf { path, body: true } => out.extend_from_slice(take(got, &number(path))?),
        Node::Leaf { body: false, .. } => {}
        Node::Multi {
            boundary, children, ..
        } => {
            for child in children {
                let path = match child {
                    Node::Leaf { path, .. } | Node::Multi { path, .. } => path,
                };
                out.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
                out.extend(header(take(got, &mime(path))?));
                content(child, got, out)?;
                out.extend_from_slice(b"\r\n");
            }
            out.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
        }
    }
    Ok(())
}

/// A header block ends with a blank line.
fn header(bytes: &[u8]) -> Vec<u8> {
    let mut out = bytes.to_vec();
    if out.is_empty() {
        return b"\r\n".to_vec();
    }
    while !out.ends_with(b"\r\n\r\n") {
        out.extend_from_slice(b"\r\n");
    }
    out
}

fn take<'a>(got: &'a HashMap<String, Vec<u8>>, key: &str) -> Result<&'a [u8]> {
    got.get(key)
        .map(Vec::as_slice)
        .ok_or_else(|| Error::Protocol(format!("section {key} did not come")))
}

/// A whole message's body is `TEXT`.
fn number(path: &[u32]) -> String {
    if path.is_empty() {
        return "TEXT".into();
    }
    path.iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(".")
}

fn mime(path: &[u32]) -> String {
    format!("{}.MIME", number(path))
}

#[cfg(test)]
mod tests;
