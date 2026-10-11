//! A conflict's three stages and hand-kept result (TREE-09).

use std::{io, path::Path, rc::Rc};

use ely_gpui_component::{
    git::{MARKER, Region, RegionKind, Take, conflicts, marked, resolve},
    primitives::Severity,
};
use gpui::Context;

use crate::{
    asks::Ask,
    git::Git,
    tree::{Tree, text},
    work::Write,
};

/// What Git holds for a conflicted path.
#[derive(Clone, Debug, PartialEq)]
pub enum Stages {
    /// Text on all three sides, and the marked work file.
    Text {
        sides: [Option<String>; 3],
        work: String,
        regions: Vec<Region>,
        /// Marker width, from `conflict-marker-size`.
        size: usize,
    },
    /// Links, submodules, bytes, missing sides: taken whole.
    Whole { present: [bool; 3] },
}

/// Stage, mode and blob id from `ls-files -u -z`.
fn staged(bytes: &[u8]) -> io::Result<Vec<(usize, String, String)>> {
    let text = std::str::from_utf8(bytes).map_err(io::Error::other)?;
    text.split('\0')
        .filter(|record| !record.is_empty())
        .map(|record| {
            let bad = || {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("an unread stage {record:?}"),
                )
            };
            let (head, _) = record.split_once('\t').ok_or_else(bad)?;
            let mut fields = head.split(' ');
            let (mode, id, stage) = (
                fields.next().ok_or_else(bad)?,
                fields.next().ok_or_else(bad)?,
                fields.next().ok_or_else(bad)?,
            );
            let stage: usize = stage.parse().map_err(|_| bad())?;
            if !(1..=3).contains(&stage) {
                return Err(bad());
            }
            Ok((stage, mode.to_string(), id.to_string()))
        })
        .collect()
}

/// Each block's base, from Git's own diff3 merge.
fn bases(
    git: &Git,
    root: &Path,
    sides: &[Option<String>; 3],
    size: usize,
) -> io::Result<Vec<Vec<String>>> {
    let dir = tempfile::tempdir()?;
    let mut names = Vec::new();
    for (side, text) in ["base", "ours", "theirs"].iter().zip(sides) {
        let file = dir.path().join(side);
        std::fs::write(&file, text.as_deref().unwrap_or(""))?;
        names.push(file.to_string_lossy().into_owned());
    }
    let out = git.run_counted(
        root,
        &[
            "merge-file",
            "-p",
            "--zdiff3",
            &format!("--marker-size={size}"),
            &names[1],
            &names[0],
            &names[2],
        ],
    )?;
    let merged = String::from_utf8(out).map_err(io::Error::other)?;
    Ok(marked(&merged, size)
        .into_iter()
        .filter(|region| region.kind == RegionKind::Conflict)
        .map(|region| region.base)
        .collect())
}

/// The marker width `conflict-marker-size` sets for `path`.
fn marker_size(git: &Git, root: &Path, path: &str) -> io::Result<usize> {
    let said = String::from_utf8(git.run(
        root,
        &["check-attr", "-z", "conflict-marker-size", "--", path],
    )?)
    .map_err(io::Error::other)?;
    // `-z` gives path, attribute and value, each NUL-ended.
    Ok(said.split('\0').nth(2).map_or(MARKER, git_width))
}

/// As Git's `strtol_i`: whole value, else 7.
fn git_width(value: &str) -> usize {
    match value
        .trim_start_matches(|c: char| c.is_ascii_whitespace() || c == '\x0b')
        .parse::<i32>()
    {
        Ok(size) if size > 0 => size as usize,
        _ => MARKER,
    }
}

/// A NUL early on: Git calls it binary.
fn binary(bytes: &[u8]) -> bool {
    bytes[..bytes.len().min(8000)].contains(&0)
}

/// What Git holds for `path`: marked text, or whole files.
pub fn read(git: &Git, root: &Path, path: &str) -> io::Result<Stages> {
    let mut sides: [Option<String>; 3] = Default::default();
    let (mut present, mut text) = ([false; 3], true);
    for (stage, mode, id) in staged(&git.run(root, &["ls-files", "-u", "-z", "--", path])?)? {
        present[stage - 1] = true;
        // Links and submodules are no text to merge by line.
        if mode == "120000" || mode == "160000" {
            text = false;
            continue;
        }
        let bytes = git.run(root, &["cat-file", "blob", &id])?;
        match String::from_utf8(bytes)
            .ok()
            .filter(|side| !binary(side.as_bytes()))
        {
            Some(side) => sides[stage - 1] = Some(side),
            None => text = false,
        }
    }
    let file = root.join(path);
    let work = match std::fs::symlink_metadata(&file) {
        Ok(meta) if meta.file_type().is_file() => String::from_utf8(std::fs::read(&file)?).ok(),
        _ => None,
    };
    let Some(work) = work.filter(|_| text && present[1] && present[2]) else {
        return Ok(Stages::Whole { present });
    };
    let size = marker_size(git, root, path)?;
    let mut regions = marked(&work, size);
    let blocks = conflicts(&work, size);
    let bases = bases(git, root, &sides, size)?;
    // Fill only blocks Git left without a base.
    if bases.len() == blocks.len() {
        let conflicted = regions
            .iter_mut()
            .filter(|region| region.kind == RegionKind::Conflict);
        for ((region, block), base) in conflicted.zip(&blocks).zip(bases) {
            if block.base.is_none() {
                region.base = base;
            }
        }
    }
    Ok(Stages::Text {
        sides,
        work,
        regions,
        size,
    })
}

/// A text conflict as the middle holds it, being worked.
pub struct Merging {
    pub stages: Stages,
    pub regions: Rc<Vec<Region>>,
    pub takes: Vec<Option<Take>>,
    /// The text last loaded or saved, to tell unsaved edits.
    pub saved: String,
}

impl Merging {
    /// This file's marker width.
    pub fn size(&self) -> usize {
        match self.stages {
            Stages::Text { size, .. } => size,
            Stages::Whole { .. } => MARKER,
        }
    }

    /// None for a whole-file conflict.
    pub fn new(stages: Stages) -> Option<Self> {
        let Stages::Text { work, regions, .. } = &stages else {
            return None;
        };
        let count = regions
            .iter()
            .filter(|region| region.kind == RegionKind::Conflict)
            .count();
        let (regions, saved) = (Rc::new(regions.clone()), work.clone());
        Some(Merging {
            stages,
            regions,
            takes: vec![None; count],
            saved,
        })
    }
}

impl Tree {
    /// Settles conflict `at` in the result, among those open.
    pub fn take(&mut self, at: usize, take: Take, cx: &mut Context<Self>) {
        let Some(merging) = self.merging.as_mut() else {
            return;
        };
        let result = self.merge_editor.read(cx).text().to_string();
        let open = merging.takes.iter().filter(|taken| taken.is_none()).count();
        if merging.takes.get(at).copied().flatten().is_some() {
            let words = text(cx, "tree.conflict.taken", &[]);
            return self.toast(words, Severity::Warning, cx);
        }
        // Hand edits that moved the markers end block choices.
        if conflicts(&result, merging.size()).len() != open {
            let words = text(cx, "tree.conflict.hand", &[]);
            return self.toast(words, Severity::Warning, cx);
        }
        let place = merging.takes[..at]
            .iter()
            .filter(|taken| taken.is_none())
            .count();
        let Some(settled) = resolve(&result, place, take, merging.size()) else {
            return;
        };
        merging.takes[at] = Some(take);
        self.merge_editor
            .update(cx, |editor, cx| editor.set_text(settled, cx));
        cx.notify();
    }

    /// Queues the result for its work file; staging is apart.
    pub fn save_merge(&mut self, cx: &mut Context<Self>) {
        let Some(shown) = &self.shown else { return };
        let (repo, path) = (shown.repo, shown.path.clone());
        let text = self.merge_editor.read(cx).text().to_string();
        self.write(repo, Write::Save { path, text }, cx);
    }

    /// A save landed; it counts if still this file's.
    pub fn merge_saved(&mut self, id: u64, path: &str, text: &str, cx: &mut Context<Self>) {
        let here = self
            .shown
            .as_ref()
            .is_some_and(|shown| shown.repo == id && shown.path == path);
        if let Some(merging) = self.merging.as_mut().filter(|_| here) {
            merging.saved = text.to_string();
            cx.notify();
        }
    }

    /// Marks the saved result resolved; markers left ask first.
    pub fn mark_resolved(&mut self, cx: &mut Context<Self>) {
        let Some(shown) = &self.shown else { return };
        let (repo, path) = (shown.repo, shown.path.clone());
        let result = self.merge_editor.read(cx).text().to_string();
        if self
            .merging
            .as_ref()
            .is_some_and(|merging| merging.saved != result)
        {
            let words = text(cx, "tree.conflict.unsaved", &[]);
            return self.toast(words, Severity::Warning, cx);
        }
        let write = Write::Git(vec!["add".into(), "--".into(), path.clone()]);
        let size = self.merging.as_ref().map_or(MARKER, Merging::size);
        if conflicts(&result, size).is_empty() {
            return self.write(repo, write, cx);
        }
        self.ask = Some(Ask::Confirm {
            repo,
            title: text(cx, "tree.conflict.marked.title", &[("name", &path)]),
            message: text(cx, "tree.conflict.marked.message", &[("name", &path)]),
            go: text(cx, "tree.conflict.marked.go", &[]),
            write,
        });
        cx.notify();
    }

    /// One side's whole file into the work tree.
    pub fn take_side(&mut self, ours: bool, cx: &mut Context<Self>) {
        let Some(shown) = &self.shown else { return };
        let side = if ours { "--ours" } else { "--theirs" };
        let write = Write::Git(vec![
            "checkout".into(),
            side.into(),
            "--".into(),
            shown.path.clone(),
        ]);
        let repo = shown.repo;
        self.write(repo, write, cx);
    }

    /// Settles the conflict by removing the file; asks first.
    pub fn resolve_deleted(&mut self, cx: &mut Context<Self>) {
        let Some(shown) = &self.shown else { return };
        let (repo, path) = (shown.repo, shown.path.clone());
        self.ask = Some(Ask::Confirm {
            repo,
            title: text(cx, "tree.conflict.delete.title", &[("name", &path)]),
            message: text(cx, "tree.conflict.delete.message", &[("name", &path)]),
            go: text(cx, "tree.conflict.delete.go", &[]),
            write: Write::Git(vec!["rm".into(), "--".into(), path]),
        });
        cx.notify();
    }
}
