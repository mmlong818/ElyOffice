//! Worktrees, submodules and LFS files (TREE-12).

use std::{
    io,
    path::{Path, PathBuf},
};

use crate::git::Git;

fn bad(what: &str) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!("an unread listing {what:?}"),
    )
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Worktree {
    pub path: PathBuf,
    pub head: Option<String>,
    /// Short name; none when detached.
    pub branch: Option<String>,
    pub bare: bool,
    /// Why it is locked, maybe empty.
    pub locked: Option<String>,
    /// Why Git would prune it.
    pub prunable: Option<String>,
}

/// `worktree list --porcelain -z`: fields, records end empty.
pub fn worktrees(git: &Git, root: &Path) -> io::Result<Vec<Worktree>> {
    let out = git.run(root, &["worktree", "list", "--porcelain", "-z"])?;
    let text = std::str::from_utf8(&out).map_err(|_| bad("not UTF-8"))?;
    let mut found = Vec::new();
    let mut now: Option<Worktree> = None;
    for field in text.split('\0') {
        if field.is_empty() {
            found.extend(now.take());
            continue;
        }
        let (key, value) = field.split_once(' ').unwrap_or((field, ""));
        let tree = match (&mut now, key) {
            (None, "worktree") => {
                now = Some(Worktree {
                    path: PathBuf::from(value),
                    ..Worktree::default()
                });
                continue;
            }
            (Some(tree), _) => tree,
            (None, _) => return Err(bad(field)),
        };
        match key {
            "HEAD" => tree.head = Some(value.to_string()),
            "branch" => {
                let short = value.strip_prefix("refs/heads/").unwrap_or(value);
                tree.branch = Some(short.to_string());
            }
            "bare" => tree.bare = true,
            "locked" => tree.locked = Some(value.to_string()),
            "prunable" => tree.prunable = Some(value.to_string()),
            "detached" => {}
            _ => return Err(bad(field)),
        }
    }
    found.extend(now);
    // Real paths, as the open repository's root is.
    for tree in &mut found {
        match dunce::canonicalize(&tree.path) {
            Ok(real) => tree.path = real,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    Ok(found)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Checkout {
    /// Not cloned into its folder yet.
    Missing,
    At(String),
    Unread(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Submodule {
    pub path: String,
    /// The commit this repository records.
    pub recorded: String,
    pub conflicted: bool,
    pub checkout: Checkout,
}

/// Gitlinks in the index, and what each folder holds.
pub fn submodules(git: &Git, root: &Path) -> io::Result<Vec<Submodule>> {
    let out = git.run(root, &["ls-files", "--stage", "-z"])?;
    let text = std::str::from_utf8(&out).map_err(|_| bad("not UTF-8"))?;
    let mut found: Vec<Submodule> = Vec::new();
    for entry in text.split('\0').filter(|entry| !entry.is_empty()) {
        let (head, path) = entry.split_once('\t').ok_or_else(|| bad(entry))?;
        let mut parts = head.split(' ');
        let (Some(mode), Some(oid), Some(stage)) = (parts.next(), parts.next(), parts.next())
        else {
            return Err(bad(entry));
        };
        if mode != "160000" {
            continue;
        }
        if let Some(known) = found.iter_mut().find(|known| known.path == path) {
            known.conflicted = true;
            continue;
        }
        let folder = root.join(path);
        // Without its own `.git`, Git here would be ours.
        let checkout = if !folder.join(".git").exists() {
            Checkout::Missing
        } else {
            match git.run(&folder, &["rev-parse", "HEAD"]) {
                Ok(head) => Checkout::At(String::from_utf8_lossy(&head).trim().to_string()),
                Err(error) => Checkout::Unread(error.to_string()),
            }
        };
        found.push(Submodule {
            path: path.to_string(),
            recorded: oid.to_string(),
            conflicted: stage != "0",
            checkout,
        });
    }
    Ok(found)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LfsFile {
    pub path: String,
    pub size: u64,
    /// The work tree holds the content, not the pointer.
    pub checked_out: bool,
    /// The object is in this repository's store.
    pub downloaded: bool,
}

/// `git lfs ls-files` as JSON; `files` is null with none.
pub fn lfs(git: &Git, root: &Path) -> io::Result<Vec<LfsFile>> {
    // LFS reads HEAD first; before a commit there is nothing.
    if !git.born(root)? {
        return Ok(Vec::new());
    }
    let out = git.run(root, &["lfs", "ls-files", "--long", "--json"])?;
    let read: serde_json::Value = serde_json::from_slice(&out).map_err(io::Error::other)?;
    let files = match &read["files"] {
        serde_json::Value::Null => return Ok(Vec::new()),
        serde_json::Value::Array(files) => files,
        other => return Err(bad(&other.to_string())),
    };
    files
        .iter()
        .map(|file| {
            Ok(LfsFile {
                path: file["name"]
                    .as_str()
                    .ok_or_else(|| bad("name"))?
                    .to_string(),
                size: file["size"].as_u64().ok_or_else(|| bad("size"))?,
                checked_out: file["checkout"].as_bool().ok_or_else(|| bad("checkout"))?,
                downloaded: file["downloaded"]
                    .as_bool()
                    .ok_or_else(|| bad("downloaded"))?,
            })
        })
        .collect()
}
