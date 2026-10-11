//! The shown file: reading it, picking lines, staging them (TREE-04).

use std::{collections::BTreeSet, sync::Arc};

use ely_gpui_component::git::ChangeSection;
use gpui::{AppContext as _, Context};

use crate::{
    diff::{self, Space},
    shown::{self, View},
    status::{Entry, State},
    tree::Tree,
    work::Write,
};

/// A line's place in the viewer: stretch, then line.
pub type Place = (usize, usize);

pub struct Shown {
    pub repo: u64,
    pub section: ChangeSection,
    pub path: String,
    pub view: Option<Result<View, String>>,
    pub picked: BTreeSet<Place>,
    /// Where a Shift-press range starts.
    pub anchor: Option<Place>,
    /// Folded stretches opened.
    pub opened: Vec<usize>,
    /// Bumped per read; an older read is dropped.
    serial: u64,
}

/// Whether `entry` is listed in `section`.
fn listed(entry: &Entry, section: ChangeSection) -> bool {
    match (&entry.state, section) {
        (State::Tracked { staged, .. }, ChangeSection::Staged) => *staged != '.',
        (State::Tracked { unstaged, .. }, ChangeSection::Unstaged) => *unstaged != '.',
        (State::Untracked, ChangeSection::Untracked) => true,
        (State::Conflicted(_), ChangeSection::Conflicted) => true,
        _ => false,
    }
}

impl Tree {
    /// Shows `path` as listed in `section`.
    pub fn show(&mut self, path: &str, section: ChangeSection, cx: &mut Context<Self>) {
        let Some(repo) = self.active else { return };
        // A new pick leaves any file history or blame.
        self.look = None;
        self.shown = Some(Shown {
            repo,
            section,
            path: path.to_string(),
            view: None,
            picked: BTreeSet::new(),
            anchor: None,
            opened: Vec::new(),
            serial: 0,
        });
        self.merging = None;
        self.read_shown(cx);
    }

    /// The shown file's entry, if its repository still lists it.
    fn shown_entry(&self) -> Option<(Entry, bool)> {
        let shown = self.shown.as_ref()?;
        let Some(Ok(status)) = &self.opened(shown.repo)?.status else {
            return None;
        };
        let entry = status
            .entries
            .iter()
            .find(|entry| entry.path == shown.path && listed(entry, shown.section))?;
        Some((entry.clone(), status.head.oid.is_some()))
    }

    /// Rereads the shown file; it goes when no longer listed.
    pub fn read_shown(&mut self, cx: &mut Context<Self>) {
        let Some(git) = self.git.as_ref().ok().cloned() else {
            return;
        };
        let Some((entry, born)) = self.shown_entry() else {
            if self.shown.take().is_some() {
                cx.notify();
            }
            return;
        };
        let space = self.space;
        let Some(repo) = self.shown.as_ref().map(|shown| shown.repo) else {
            return;
        };
        let Some(root) = self.opened(repo).map(|opened| opened.repo.root.clone()) else {
            return;
        };
        let Some(shown) = &mut self.shown else { return };
        // Unique across files, so a late read is dropped.
        self.reads += 1;
        shown.serial = self.reads;
        let (serial, section) = (shown.serial, shown.section);
        let task =
            cx.background_spawn(
                async move { shown::read(&git, &root, &entry, section, space, born) },
            );
        cx.spawn(async move |this, cx| {
            let found = task.await;
            this.update(cx, |tree, cx| tree.shown_read(serial, found, cx))
                .ok();
        })
        .detach();
    }

    pub(crate) fn shown_read(
        &mut self,
        serial: u64,
        found: std::io::Result<shown::Found>,
        cx: &mut Context<Self>,
    ) {
        let Some(shown) = self.shown.as_mut().filter(|shown| shown.serial == serial) else {
            return;
        };
        let view = found
            .and_then(shown::view)
            .map_err(|error| error.to_string());
        if let Err(error) = &view {
            log::warn!("tree: reading {} failed: {error}", shown.path);
        }
        // Picks hold only while the diff is unchanged.
        let same = matches!((&shown.view, &view), (Some(Ok(View::Text { seen: was, .. })), Ok(View::Text { seen: now, .. })) if was == now);
        if !same {
            (shown.picked, shown.anchor, shown.opened) = (BTreeSet::new(), None, Vec::new());
        }
        let stages = match &view {
            Ok(View::Conflict(stages)) => Some(stages.clone()),
            _ => None,
        };
        shown.view = Some(view);
        if let Some(stages) = stages {
            self.adopt_merge(stages, cx);
        }
        cx.notify();
    }

    /// A conflict read again keeps unsaved hand edits.
    fn adopt_merge(&mut self, stages: crate::conflict::Stages, cx: &mut Context<Self>) {
        let text = self.merge_editor.read(cx).text().to_string();
        match &self.merging {
            Some(merging) if merging.stages == stages => {}
            Some(merging) if merging.saved != text => {
                let words = crate::tree::text(cx, "tree.conflict.kept", &[]);
                self.toast(words, ely_gpui_component::primitives::Severity::Warning, cx);
            }
            _ => {
                self.merging = crate::conflict::Merging::new(stages);
                let work = self
                    .merging
                    .as_ref()
                    .map(|merging| merging.saved.clone())
                    .unwrap_or_default();
                self.merge_editor
                    .update(cx, |editor, cx| editor.set_text(work, cx));
            }
        }
    }

    pub fn set_space(&mut self, space: Space, cx: &mut Context<Self>) {
        self.space = space;
        self.read_shown(cx);
        cx.notify();
    }

    /// Toggles a line; Shift takes a range.
    pub fn pick_line(&mut self, place: Place, shift: bool, cx: &mut Context<Self>) {
        let Some(shown) = &mut self.shown else { return };
        let range = shown.anchor.filter(|anchor| shift && anchor.0 == place.0);
        match range {
            Some(anchor) => {
                let (from, to) = (anchor.1.min(place.1), anchor.1.max(place.1));
                shown.picked.extend((from..=to).map(|line| (place.0, line)));
            }
            None if !shown.picked.remove(&place) => {
                shown.picked.insert(place);
            }
            None => {}
        }
        shown.anchor = Some(place);
        cx.notify();
    }

    /// Stages, or unstages, every change in one hunk.
    pub fn stage_hunk(&mut self, stretch: usize, cx: &mut Context<Self>) {
        let Some(Shown {
            view: Some(Ok(View::Text { stretches, .. })),
            ..
        }) = &self.shown
        else {
            return;
        };
        let Some(ely_gpui_component::git::Stretch::Hunk { lines, .. }) = stretches.get(stretch)
        else {
            return;
        };
        let places: BTreeSet<Place> = (0..lines.len()).map(|line| (stretch, line)).collect();
        self.stage_places(&places, cx);
    }

    pub fn stage_picked(&mut self, cx: &mut Context<Self>) {
        let Some(shown) = &self.shown else { return };
        let picked = shown.picked.clone();
        self.stage_places(&picked, cx);
    }

    /// Cuts a patch of `places` and queues it.
    fn stage_places(&mut self, places: &BTreeSet<Place>, cx: &mut Context<Self>) {
        let Some((entry, _)) = self.shown_entry() else {
            return;
        };
        let space = self.space;
        let Some(shown) = &self.shown else { return };
        let Some(Ok(View::Text {
            diff, seen, hunks, ..
        })) = &shown.view
        else {
            return;
        };
        let chosen: BTreeSet<(usize, usize)> = places
            .iter()
            .filter_map(|(stretch, line)| {
                hunks
                    .get(*stretch)
                    .copied()
                    .flatten()
                    .map(|hunk| (hunk, *line))
            })
            .collect();
        let reverse = shown.section == ChangeSection::Staged;
        let Some(patch) = diff::patch(diff, &chosen, reverse) else {
            return log::info!("tree: no changed line was picked");
        };
        let write = Write::Patch {
            patch,
            reverse,
            check: shown::args(&entry, shown.section, space),
            seen: Arc::clone(seen),
        };
        let repo = shown.repo;
        self.write(repo, write, cx);
    }
}
