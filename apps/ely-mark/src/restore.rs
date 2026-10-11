//! The session kept across launches (MARK-12, §6.2, §6.3).

use std::{
    io,
    path::{Path, PathBuf},
    time::SystemTime,
};

use ely_files::{Crash, Marker, Recovery};
use ely_office::prelude::*;
use ely_ui::i18n::text;
use gpui::Context;
use serde::{Deserialize, Serialize};

use crate::desk::Desk;
use crate::engine::mark;
use crate::page::Mode;
use crate::view::Place;

/// A kept tab: file, mode, place, recovery folder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Kept {
    path: Option<PathBuf>,
    name: String,
    mode: String,
    place: Option<Place>,
    slot: String,
}

#[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
struct Saved {
    tabs: Vec<Kept>,
}

/// A kept tab yet to show; recovered text copied.
struct Waiting {
    kept: Kept,
    real: Option<PathBuf>,
    text: Option<(String, SystemTime)>,
    /// The untitled tab made for it, once made.
    tab: Option<u64>,
}

/// The session's files and what still waits to restore.
pub struct Keeping {
    dir: PathBuf,
    crash: Option<Crash>,
    waiting: Vec<Waiting>,
    written: Option<String>,
    /// A quit began: the kept list stays as then.
    pub leaving: bool,
    /// The list as kept when the quit began.
    frozen: Vec<Kept>,
}

fn recovery(dir: &Path, slot: &str) -> PathBuf {
    dir.join("recovery").join(slot)
}

/// The newest point's text in `slot`, after a crash.
fn recovered(dir: &Path, slot: &str) -> io::Result<Option<(String, SystemTime)>> {
    let Some(point) = Recovery::new(recovery(dir, slot))
        .points()?
        .into_iter()
        .next()
    else {
        return Ok(None);
    };
    let bytes = std::fs::read(&point.content)?;
    let text = String::from_utf8(bytes)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    Ok(Some((text, point.at)))
}

impl Keeping {
    /// Reads the last session; a crash brings its unsaved text.
    pub fn start(dir: &Path) -> io::Result<(Keeping, Vec<PathBuf>, Vec<String>)> {
        let marker = Crash::check(dir, ely_files::alive)?;
        log::info!("mark: the last session ended {marker:?}");
        let crash = Crash::open(dir, std::process::id())?;
        let saved: Saved = match std::fs::read(dir.join("session.json")) {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(io::Error::other)?,
            Err(error) if error.kind() == io::ErrorKind::NotFound => Saved::default(),
            Err(error) => return Err(error),
        };
        let (mut waiting, mut missing) = (Vec::new(), Vec::new());
        for kept in saved.tabs {
            let text = match marker {
                Marker::Crashed => recovered(dir, &kept.slot)?,
                Marker::Clean | Marker::Discarded => None,
            };
            if let Some(path) = &kept.path {
                match std::fs::canonicalize(path) {
                    // The file reopens as it is on disk.
                    Ok(real) => waiting.push(Waiting {
                        kept: Kept {
                            slot: String::new(),
                            ..kept.clone()
                        },
                        real: Some(real),
                        text: None,
                        tab: None,
                    }),
                    Err(error) => {
                        log::warn!("mark: a kept file does not open: {error}");
                        missing.push(format!("{}: {error}", path.display()));
                    }
                }
            }
            // Recovered text never overwrites a file: it is a copy.
            if text.is_some() {
                let kept = Kept { path: None, ..kept };
                waiting.push(Waiting {
                    kept,
                    real: None,
                    text,
                    tab: None,
                });
            }
        }
        if marker != Marker::Crashed {
            clear(&dir.join("recovery"))?;
        }
        let paths = waiting
            .iter()
            .filter_map(|waiting| waiting.real.clone())
            .collect();
        let keeping = Keeping {
            dir: dir.to_path_buf(),
            crash: Some(crash),
            waiting,
            written: None,
            leaving: false,
            frozen: Vec::new(),
        };
        Ok((keeping, paths, missing))
    }

    /// The untitled tabs to make: their place here and name.
    pub fn untitled(&self) -> Vec<(usize, String)> {
        let waiting = self.waiting.iter().enumerate();
        let untitled = waiting.filter(|(_, waiting)| waiting.kept.path.is_none() && waiting.tab.is_none());
        untitled.map(|(at, waiting)| (at, waiting.kept.name.clone())).collect()
    }

    /// Entry `at` belongs to untitled tab `tab`.
    pub fn made(&mut self, at: usize, tab: u64) {
        self.waiting[at].tab = Some(tab);
    }

    /// What tab `id` takes: its file or its copy.
    fn take(&mut self, real: Option<&PathBuf>, id: u64) -> Option<Waiting> {
        let at = self.waiting.iter().position(|waiting| match (&waiting.real, real) {
            (Some(kept), Some(open)) => kept == open,
            (None, None) => waiting.tab == Some(id),
            _ => false,
        })?;
        Some(self.waiting.remove(at))
    }

    /// An end that never asked: the lock and points stay.
    pub fn hold(&mut self) {
        if self.crash.take().is_some() {
            log::warn!("mark: the session ends unasked; its points stay");
        }
    }

    /// A clean end: no crash next time, no stale points.
    pub fn close(&mut self) {
        let Some(crash) = self.crash.take() else {
            return;
        };
        if let Err(error) = crash
            .close()
            .and_then(|()| clear(&self.dir.join("recovery")))
        {
            log::error!("mark: the session did not close cleanly: {error}");
        }
    }
}

/// Removes every slot folder in `dir` not in `live`.
fn sweep(dir: &Path, live: &[&str]) -> io::Result<()> {
    let entries = match std::fs::read_dir(dir) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        entries => entries?,
    };
    for entry in entries {
        let entry = entry?;
        if !live.contains(&entry.file_name().to_string_lossy().as_ref()) {
            clear(&entry.path())?;
        }
    }
    Ok(())
}

fn clear(dir: &Path) -> io::Result<()> {
    match std::fs::remove_dir_all(dir) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
        _ => Ok(()),
    }
}

impl Desk {
    /// A tab just shown takes its kept mode, place, text.
    pub fn restore_shown(&mut self, id: u64, cx: &mut Context<Self>) {
        let Some(tab) = self.tab_mut(id).filter(|tab| !tab.extra.restored) else {
            return;
        };
        // Each tab takes one kept entry, on its first show.
        tab.extra.restored = true;
        let tab = self.tab(id).expect("open");
        let real = tab
            .session
            .path()
            .and_then(|path| std::fs::canonicalize(path).ok());
        let Some(Waiting {
            kept, text: back, ..
        }) = self.keeping.take(real.as_ref(), id)
        else {
            return;
        };
        let mode = Mode::parse(&kept.mode).unwrap_or(Mode::Visual);
        log::info!("mark: restores tab {id} as {}", mode.name());
        let tab = self.tab_mut(id).expect("open");
        // A recovered copy keeps writing to the same points.
        tab.extra.slot = kept.slot.clone();
        let view = tab.view.clone();
        if let Some((_, at)) = &back {
            let at = humantime(*at);
            let said = text(
                cx,
                "mark.restore.recovered",
                &[("name", &kept.name), ("at", &at)],
            );
            self.toast(said, ely_gpui_component::primitives::Severity::Info, cx);
        }
        let back = back.map(|(back, _)| back);
        view.update(cx, |view, cx| view.reopen(mode, kept.place, back, cx));
    }

    /// Every ten seconds: unsaved text to points, tabs to disk.
    pub fn keep(&mut self, cx: &mut Context<Self>) {
        let dir = self.keeping.dir.clone();
        let started = std::time::UNIX_EPOCH
            .elapsed()
            .map_or(0, |since| since.as_millis());
        let mut kept = Vec::new();
        for tab in &mut self.core.tabs {
            if tab.extra.slot.is_empty() {
                tab.extra.slot = format!("{started}-{}", tab.id);
            }
            let view = tab.view.read(cx);
            kept.push(Kept {
                path: tab.session.path().map(Path::to_path_buf),
                name: tab.name.to_string(),
                mode: view.mode.name().to_owned(),
                place: view.place,
                slot: tab.extra.slot.clone(),
            });
            let current = tab.doc.lock().expect("the text lock").current.clone();
            let point = mark(&current);
            if !tab.dirty(cx) || tab.extra.pointed.as_ref() == Some(&point) || tab.extra.pointing {
                continue;
            }
            tab.extra.pointing = true;
            let (id, slot) = (tab.id, recovery(&dir, &tab.extra.slot));
            cx.spawn(async move |desk, cx| {
                let folder = slot.clone();
                let wrote = cx
                    .background_executor()
                    .spawn(async move {
                        let now = SystemTime::now();
                        let limits = ely_platform::storage::limits(ely_platform::AppId::Mark)?;
                        let mut recovery = Recovery::new(folder).with_limits(limits.keep, limits.megabytes * 1024 * 1024);
                        recovery.write_once(current.as_bytes(), None, now)
                    })
                    .await;
                desk.update(cx, |desk, _| desk.pointed(id, point, wrote, &slot))
                    .ok();
            })
            .detach();
        }
        // Closed or discarded tabs' points go (§6.2).
        let live = kept.iter().map(|kept| kept.slot.as_str());
        let live: Vec<&str> = live
            .chain(
                self.keeping
                    .waiting
                    .iter()
                    .map(|waiting| waiting.kept.slot.as_str()),
            )
            .collect();
        if let Err(error) = sweep(&dir.join("recovery"), &live) {
            log::error!("mark: old recovery points stay: {error}");
        }
        if !self.keeping.leaving {
            self.keeping.frozen = kept.clone();
            self.write_session(Saved { tabs: kept });
        }
    }

    fn pointed(&mut self, id: u64, point: String, wrote: io::Result<Option<PathBuf>>, slot: &Path) {
        // The tab closed while it wrote: the point goes too.
        let Some(tab) = self.tab_mut(id) else {
            if let Err(error) = clear(slot) {
                log::error!("mark: a closed tab's point stays: {error}");
            }
            return;
        };
        tab.extra.pointing = false;
        match wrote {
            Ok(_) => tab.extra.pointed = Some(point),
            Err(error) => log::error!("mark: a recovery point for tab {id} failed: {error}"),
        }
    }

    fn write_session(&mut self, saved: Saved) {
        let json = serde_json::to_string_pretty(&saved).expect("the session serializes");
        if self.keeping.written.as_ref() == Some(&json) {
            return;
        }
        let file = self.keeping.dir.join("session.json");
        match ely_files::save_atomic(&file, json.as_bytes(), 0) {
            Ok(_) => self.keeping.written = Some(json),
            Err(error) => log::error!("mark: the session did not save: {error}"),
        }
    }

    /// Keeps every ten seconds while the window lives (§6.2).
    pub fn keep_every(&mut self, cx: &mut Context<Self>) {
        cx.spawn(async move |desk, cx| {
            loop {
                cx.background_executor().timer(Recovery::INTERVAL).await;
                if desk.update(cx, |desk, cx| desk.keep(cx)).is_err() {
                    break;
                }
            }
        })
        .detach();
    }

    /// Slot `slot` closed: its points go, untitled entries too.
    pub fn drop_slot(
        &mut self,
        slot: &str,
        path: Option<PathBuf>,
        name: String,
        cx: &mut Context<Self>,
    ) {
        if !slot.is_empty()
            && let Err(error) = clear(&recovery(&self.keeping.dir, slot))
        {
            log::error!("mark: a closed tab's points stay: {error}");
        }
        if !self.keeping.leaving {
            return self.keep(cx);
        }
        // Discarded untitled text goes; a first save keeps its file.
        match path {
            None => self.keeping.frozen.retain(|kept| kept.slot != slot),
            Some(path) => {
                let saved = self
                    .keeping
                    .frozen
                    .iter_mut()
                    .filter(|kept| kept.slot == slot);
                for kept in saved {
                    (kept.path, kept.name) = (Some(path.clone()), name.clone());
                }
            }
        }
        let tabs = self.keeping.frozen.clone();
        self.write_session(Saved { tabs });
    }

    /// A quit or close begins: the tabs now stay kept.
    pub fn leave(&mut self, cx: &mut Context<Self>) {
        self.keeping.leaving = false;
        self.keep(cx);
        self.keeping.leaving = true;
    }
}

/// A recovery time as this machine's clock reads it.
fn humantime(at: SystemTime) -> String {
    match jiff::Timestamp::try_from(at) {
        Ok(at) => at
            .to_zoned(jiff::tz::TimeZone::system())
            .strftime("%Y-%m-%d %H:%M")
            .to_string(),
        Err(error) => {
            log::warn!("mark: a recovery time is out of range: {error}");
            error.to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_place_without_a_column_reads_zero() {
        let kept: super::Kept = serde_json::from_str(
            r#"{"path":null,"name":"a.md","mode":"source","place":{"line":3,"scroll":0},"slot":"s"}"#,
        )
        .expect("an older place reads");
        let place = kept.place.expect("a place");
        assert_eq!((place.line, place.column), (3, 0));
    }

    #[test]
    fn each_untitled_copy_goes_to_the_tab_made_for_it() {
        let dir = tempfile::tempdir().expect("a temp dir");
        let kept = |name: &str, mode: &str, slot: &str| super::Kept {
            path: None,
            name: name.into(),
            mode: mode.into(),
            place: Some(super::Place { line: slot.len() as u64, column: 1, scroll: 0.0 }),
            slot: slot.into(),
        };
        let tabs = vec![kept("Untitled1", "visual", "a"), kept("Untitled2", "source", "bb")];
        for (slot, text) in [("a", "first"), ("bb", "second")] {
            let mut points = super::Recovery::new(super::recovery(dir.path(), slot));
            points.write_once(text.as_bytes(), None, std::time::SystemTime::now()).expect("a point");
        }
        let saved = serde_json::to_vec(&super::Saved { tabs: tabs.clone() }).expect("a list");
        std::fs::write(dir.path().join("session.json"), saved).expect("the list");
        // A pid no process has: the last run crashed.
        std::fs::write(dir.path().join("session.lock"), "open 4294967295").expect("a lock");
        let (mut keeping, ..) = super::Keeping::start(dir.path()).expect("a session");
        let made: Vec<_> = keeping.untitled();
        assert_eq!(made, [(0, "Untitled1".to_string()), (1, "Untitled2".to_string())]);
        for (at, _) in made {
            keeping.made(at, 10 + at as u64);
        }
        assert!(keeping.take(None, 12).is_none(), "a new tab takes nothing");
        // The second tab shows first: each still takes its own.
        for (id, want, text) in [(11, &tabs[1], "second"), (10, &tabs[0], "first")] {
            let got = keeping.take(None, id).expect("its copy");
            assert_eq!((&got.kept, got.text.map(|(text, _)| text)), (want, Some(text.to_string())));
        }
    }

    #[test]
    fn an_unasked_end_keeps_the_lock_and_points_past_release() {
        let dir = tempfile::tempdir().expect("a temp dir");
        let point = dir.path().join("recovery/s/point");
        let (lock, slot) = (dir.path().join("session.lock"), point.parent().expect("a slot"));
        let (mut held, ..) = super::Keeping::start(dir.path()).expect("a session");
        std::fs::create_dir_all(slot).expect("a slot");
        std::fs::write(&point, "unsaved").expect("a point");
        held.hold();
        held.close();
        assert!(lock.exists() && point.exists(), "release after quit wiped them");
        drop(held);
        let (mut clean, ..) = super::Keeping::start(dir.path()).expect("a session");
        std::fs::create_dir_all(slot).expect("a slot");
        std::fs::write(&point, "unsaved").expect("a point");
        clean.close();
        assert!(!lock.exists() && !point.exists(), "a clean end clears");
    }
}
