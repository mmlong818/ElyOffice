//! The disk's version against unsaved text, over their base (§14.4).

use std::rc::Rc;

use ely_files::Fingerprint;
use ely_gpui_component::{
    buttons::{Button, ButtonVariant},
    git::{self as merging, Region, Take, ThreeWayMerge},
    overlays::Dialog,
    primitives::Severity,
};
use ely_office::prelude::*;
use ely_ui::i18n::text;
use gpui::{AnyElement, Context, IntoElement as _, ParentElement as _, Styled as _, Window, div};

use crate::desk::Desk;
use crate::outside::Outside;
use crate::text::Source;

/// A comparison open over tab `id`.
pub struct Comparing {
    pub id: u64,
    disk: Source,
    /// The disk as read; a choice checks it first.
    fingerprint: Fingerprint,
    regions: Rc<Vec<Region>>,
    takes: Vec<Option<Take>>,
}

impl Desk {
    /// Mine, the base and the disk's, side by side.
    pub fn compare(&mut self, id: u64, cx: &mut Context<Self>) {
        // The page's newest text first.
        if !self.settled(id, cx) {
            return self.when_settled(id, move |desk, _, cx| desk.compare(id, cx));
        }
        let Some(tab) = self.tab(id) else {
            return;
        };
        let Outside::Changed(disk, fingerprint) = &tab.extra.outside else {
            return;
        };
        let mine = tab.doc.lock().expect("the text lock").current.clone();
        let regions = merging::regions(&tab.extra.base, &mine, &disk.text);
        let conflicts = regions
            .iter()
            .filter(|region| region.kind == merging::RegionKind::Conflict)
            .count();
        log::info!("mark: compares tab {id}: {conflicts} conflicts");
        self.comparing = Some(Comparing {
            id,
            disk: disk.clone(),
            fingerprint: *fingerprint,
            regions: Rc::new(regions),
            takes: vec![None; conflicts],
        });
        cx.notify();
    }

    fn take(&mut self, conflict: usize, take: Take, cx: &mut Context<Self>) {
        if let Some(slot) = self
            .comparing
            .as_mut()
            .and_then(|comparing| comparing.takes.get_mut(conflict))
        {
            *slot = Some(take);
            cx.notify();
        }
    }

    /// Settles tab `id`: the disk is the new base.
    fn settle(&mut self, choice: Choice, window: &mut Window, cx: &mut Context<Self>) {
        let Some(comparing) = self.comparing.as_ref() else {
            return;
        };
        let id = comparing.id;
        let now = self
            .tab(id)
            .and_then(|tab| tab.session.path())
            .and_then(|path| Fingerprint::of(path).ok());
        // The disk moved on: compare again, never write blind.
        if now != Some(comparing.fingerprint) {
            log::warn!("mark: tab {id} changed on disk during the comparison");
            let said = text(cx, "mark.compare.again", &[]);
            self.toast(said, Severity::Warning, cx);
            return self.look(id, cx);
        }
        let Some(Comparing {
            id,
            disk,
            fingerprint,
            regions,
            takes,
        }) = self.comparing.take()
        else {
            return;
        };
        log::info!("mark: tab {id} settles with {choice:?}");
        match choice {
            Choice::Disk => self.take_disk(id, disk, fingerprint, cx),
            // The checked save: a later disk change conflicts again.
            Choice::Mine => {
                if let Some(tab) = self.tab_mut(id) {
                    tab.session.rebase(fingerprint);
                    tab.extra.restart_autosave();
                }
                self.set_outside(id, Outside::Clear, cx);
                self.save(id, None, window, cx);
            }
            Choice::Merged => {
                let (merged, open) = merging::result(&regions, &takes);
                if open > 0 {
                    return log::error!("mark: a merge with {open} open conflicts");
                }
                // Disk text is saved; the merge edits it.
                if let Some(view) = self.adopt_disk(id, &disk, fingerprint, cx) {
                    view.update(cx, |view, cx| view.adopt(merged, cx));
                }
            }
        }
    }

    pub fn compare_dialog(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let comparing = self.comparing.as_ref()?;
        let name = self.tab(comparing.id)?.name.to_string();
        let this = cx.entity();
        let labels = [
            "mark.compare.mine",
            "mark.compare.base",
            "mark.compare.disk",
        ]
        .map(|key| text(cx, key, &[]).to_string());
        let merge = ThreeWayMerge::new(
            "mark-compare",
            [&labels[0], &labels[1], &labels[2]],
            comparing.regions.clone(),
        )
        .takes(comparing.takes.clone())
        .on_take({
            let this = this.clone();
            move |conflict, take, _, cx| this.update(cx, |desk, cx| desk.take(conflict, take, cx))
        });
        let open = comparing.takes.iter().filter(|take| take.is_none()).count();
        let title = text(cx, "mark.compare.title", &[("name", &name)]);
        let close = {
            let this = this.clone();
            move |_: &mut Window, cx: &mut gpui::App| {
                this.update(cx, |desk, cx| {
                    desk.comparing = None;
                    cx.notify();
                })
            }
        };
        let mut dialog = Dialog::new("mark-compare-dialog", title, close)
            .fullscreen()
            .child(div().flex_1().min_h_0().size_full().child(merge));
        for (choice, key, variant) in [
            (
                Choice::Disk,
                "mark.compare.use_disk",
                ButtonVariant::Secondary,
            ),
            (
                Choice::Mine,
                "mark.compare.keep_mine",
                ButtonVariant::Secondary,
            ),
            (
                Choice::Merged,
                "mark.compare.use_merged",
                ButtonVariant::Primary,
            ),
        ] {
            let label = text(cx, key, &[]);
            let this = this.clone();
            let disabled = choice == Choice::Merged && open > 0;
            dialog = dialog.action(move |_| {
                Button::new(key, label)
                    .variant(variant)
                    .disabled(disabled)
                    .on_click(move |_, window, cx| {
                        this.update(cx, |desk, cx| desk.settle(choice, window, cx))
                    })
            });
        }
        Some(dialog.into_any_element())
    }
}

/// How a comparison ends.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Choice {
    Disk,
    Mine,
    Merged,
}
