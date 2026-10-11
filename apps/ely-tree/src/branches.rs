//! Branches and tags as `for-each-ref` lists them (TREE-07).

use std::{io, path::Path};

use crate::git::Git;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ref {
    /// `main`, `origin/main`, `v1`.
    pub name: String,
    /// `refs/heads/main`: names a branch and a tag apart.
    pub full: String,
    pub kind: Kind,
    pub current: bool,
    pub upstream: Option<String>,
    /// Ahead and behind; None without a live upstream.
    pub track: Option<(usize, usize)>,
    /// The commit it names, through an annotated tag.
    pub commit: String,
    pub subject: String,
    /// When made; a tag on a blob has none.
    pub at: Option<i64>,
    /// An annotated tag's own message.
    pub message: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Local,
    Remote,
    Tag,
}

const FORMAT: &str = "--format=%(refname)%00%(HEAD)%00%(upstream:short)%00%(upstream:track,nobracket)%00%(objecttype)%00%(objectname)%00%(*objectname)%00%(contents:subject)%00%(*subject)%00%(creatordate:unix)%1e";

fn bad(what: &str) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!("an unread ref {what:?}"),
    )
}

/// `ahead 2, behind 1` as (2, 1); `gone` as none.
fn track(words: &str) -> Option<(usize, usize)> {
    if words == "gone" {
        return None;
    }
    let mut counts = (0, 0);
    for part in words.split(", ").filter(|part| !part.is_empty()) {
        match part.split_once(' ')? {
            ("ahead", n) => counts.0 = n.parse().ok()?,
            ("behind", n) => counts.1 = n.parse().ok()?,
            _ => return None,
        }
    }
    Some(counts)
}

pub fn parse(bytes: &[u8]) -> io::Result<Vec<Ref>> {
    let text = std::str::from_utf8(bytes).map_err(|_| bad("not UTF-8"))?;
    let mut refs = Vec::new();
    for record in text
        .split('\u{1e}')
        .map(|record| record.trim_start_matches('\n'))
        .filter(|record| !record.is_empty())
    {
        let fields: Vec<&str> = record.split('\0').collect();
        let [
            full,
            head,
            upstream,
            tracking,
            object_kind,
            object,
            peeled,
            subject,
            peeled_subject,
            at,
        ] = fields[..]
        else {
            return Err(bad(record));
        };
        // Names come off the full ref; short ones clash.
        let (kind, name) = if let Some(name) = full.strip_prefix("refs/heads/") {
            (Kind::Local, name)
        } else if let Some(name) = full
            .strip_prefix("refs/remotes/")
            .filter(|name| !name.ends_with("/HEAD"))
        {
            (Kind::Remote, name)
        } else if let Some(name) = full.strip_prefix("refs/tags/") {
            (Kind::Tag, name)
        } else {
            // `origin/HEAD` points at a branch; it is not one.
            continue;
        };
        let annotated = object_kind == "tag";
        refs.push(Ref {
            name: name.to_string(),
            full: full.to_string(),
            kind,
            current: head == "*",
            upstream: (!upstream.is_empty()).then(|| upstream.to_string()),
            track: (!upstream.is_empty()).then(|| track(tracking)).flatten(),
            commit: if annotated { peeled } else { object }.to_string(),
            subject: if annotated { peeled_subject } else { subject }.to_string(),
            at: match at {
                "" => None,
                at => Some(at.parse().map_err(|_| bad(record))?),
            },
            message: annotated.then(|| subject.to_string()),
        });
    }
    Ok(refs)
}

/// Every branch, remote branch and tag.
pub fn read(git: &Git, root: &Path) -> io::Result<Vec<Ref>> {
    parse(&git.run(
        root,
        &[
            "for-each-ref",
            FORMAT,
            "refs/heads",
            "refs/remotes",
            "refs/tags",
        ],
    )?)
}
