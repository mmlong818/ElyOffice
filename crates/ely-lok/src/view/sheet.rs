//! Calc reports print twips; tiles and clicks take tile twips.

use std::{io, sync::Mutex};

use gpui::Context;

use super::{Layout, LokView, TILE_TWIPS};
use crate::{Doc, Rect, Worker, sheet::Geometry};

/// A layout and the client zoom it was measured at.
pub(super) type Measured = io::Result<(Layout, (i32, i32))>;

/// Reads the layout under the worker lock.
pub(super) fn measure(worker: &Mutex<Worker>, doc: &Doc) -> Measured {
    let mut worker = worker.lock().expect("the worker lock");
    let (size, zoom) = doc.size_at(&mut worker)?;
    Ok(((size, doc.pages(&mut worker)?), zoom))
}

impl LokView {
    /// Lays the sheet out the kernel's way.
    pub fn set_geometry(&mut self, geometry: Geometry, cx: &mut Context<Self>) {
        if self.sheet.as_ref() != Some(&geometry) {
            self.sheet = Some(geometry);
            self.clamp_scroll();
            cx.notify();
        }
    }

    /// Device pixels per twip, as the kernel paints our tiles.
    fn ppt(&self) -> f64 {
        f64::from(self.geom.tile_pixels()) / f64::from(TILE_TWIPS)
    }

    /// A print-twips point in tile twips.
    pub(super) fn tile_of(&self, (x, y): (i32, i32)) -> (i32, i32) {
        match &self.sheet {
            Some(sheet) => (sheet.columns.tile(x, self.ppt()), sheet.rows.tile(y, self.ppt())),
            None => (x, y),
        }
    }

    /// A tile-twips point in print twips.
    pub(super) fn print_of(&self, (x, y): (i32, i32)) -> (i32, i32) {
        match &self.sheet {
            Some(sheet) => (sheet.columns.print(x, self.ppt()), sheet.rows.print(y, self.ppt())),
            None => (x, y),
        }
    }

    /// A print-twips rect in tile twips.
    pub(super) fn tile_rect(&self, (x, y, w, h): Rect) -> Rect {
        let (left, top) = self.tile_of((x, y));
        let (right, bottom) = self.tile_of((x + w, y + h));
        (left, top, right - left, bottom - top)
    }

    /// The frozen extent in tile twips.
    pub(super) fn frozen_tiles(&self) -> (i32, i32) {
        self.tile_of(self.frozen)
    }

    /// Rescales, keeping the scrolling pane's start in place.
    pub(super) fn keep_scroll(&mut self, change: impl FnOnce(&mut Self)) {
        let start = self.scrolled_start();
        self.carry_size(change);
        let ((x, y), (fx, fy)) = (self.tile_of(start), self.frozen_tiles());
        self.scroll = (x - fx, y - fy);
    }

    /// Rescales; the measured size moves to the new scale too.
    pub(super) fn carry_size(&mut self, change: impl FnOnce(&mut Self)) {
        // Until measured again, scrolls clamp against this.
        let print = self.print_of(self.doc_size);
        change(self);
        self.doc_size = self.tile_of(print);
    }

    /// A sheet size the kernel measured at `zoom`, at ours.
    pub(super) fn at_scale(&self, size: (i32, i32), (pixels, twips): (i32, i32)) -> (i32, i32) {
        let Some(sheet) = self.sheet.as_ref().filter(|_| pixels > 0 && twips > 0) else {
            return size;
        };
        let theirs = f64::from(pixels) / f64::from(twips);
        let print = (sheet.columns.print(size.0, theirs), sheet.rows.print(size.1, theirs));
        self.tile_of(print)
    }

    /// Where the scrolling pane starts, in print twips.
    pub fn scrolled_start(&self) -> (i32, i32) {
        let (fx, fy) = self.frozen_tiles();
        self.print_of((fx + self.scroll.0, fy + self.scroll.1))
    }
}
