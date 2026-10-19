//! Old tiles stay under a page until new ones land.

use std::sync::Arc;

use gpui::RenderImage;

use crate::layout::{Placed, visible};
use crate::pages::{TILE, TileKey};
use crate::view::PdfView;

/// A tile no longer current and its page's size then.
pub struct Old {
    pub key: TileKey,
    pub image: Arc<RenderImage>,
    pub size: (f32, f32),
}

/// A page on screen: place, device size, tiles in view.
pub struct Showing {
    pub place: Placed,
    pub device: (i32, i32),
    pub keys: Vec<(TileKey, [i32; 4])>,
}

/// Where `key`'s tile sat in its page, as fractions.
fn fraction(key: &TileKey, size: (f32, f32)) -> [f32; 4] {
    let (w, h) = match key.rotate.rem_euclid(180) {
        90 => (size.1, size.0),
        _ => size,
    };
    let scale = key.scale as f32 / 1000.0;
    let (dw, dh) = ((w * scale).ceil(), (h * scale).ceil());
    let (x, y) = ((key.tx * TILE) as f32, (key.ty * TILE) as f32);
    [x / dw, y / dh, (TILE as f32).min(dw - x) / dw, (TILE as f32).min(dh - y) / dh]
}

impl PdfView {
    /// The pages in view and the tiles each needs.
    pub fn showing(&self, sf: f32) -> Vec<Showing> {
        let laid = self.laid_out();
        let (room_w, room_h) = (f32::from(self.room.size.width), f32::from(self.room.size.height));
        let (left, top) = (-f32::from(self.scroll.offset().x), -f32::from(self.scroll.offset().y));
        let scale = (laid.scale * sf * 1000.0).round() as u32;
        visible(&laid, top, top + room_h)
            .filter(|p| p.x < left + room_w && p.x + p.w > left)
            .map(|place| {
                let (dw, dh) = ((place.w * sf).ceil() as i32, (place.h * sf).ceil() as i32);
                let ys = crate::pages::span(top - place.y, top + room_h - place.y, sf, dh);
                let xs = crate::pages::span(left - place.x, left + room_w - place.x, sf, dw);
                let keys = ys
                    .flat_map(|ty| xs.clone().map(move |tx| (tx, ty)))
                    .map(|(tx, ty)| {
                        let key = TileKey { page: place.page, scale, rotate: self.rotate, tx, ty };
                        (key, [tx * TILE, ty * TILE, TILE.min(dw - tx * TILE), TILE.min(dh - ty * TILE)])
                    })
                    .collect();
                Showing { place: *place, device: (dw, dh), keys }
            })
            .collect()
    }

    /// What a page draws, bottom first, in page fractions.
    pub fn pieces(&self, showing: &Showing) -> Vec<(Arc<RenderImage>, [f32; 4])> {
        let (dw, dh) = (showing.device.0 as f32, showing.device.1 as f32);
        let current = showing.keys.iter().filter_map(|(key, rect)| {
            let at = [rect[0] as f32 / dw, rect[1] as f32 / dh, rect[2] as f32 / dw, rect[3] as f32 / dh];
            self.tiles.get(key).map(|image| (image.clone(), at))
        });
        if showing.keys.iter().all(|(key, _)| self.tiles.contains_key(key)) {
            return current.collect();
        }
        // Old tiles of this page fill in underneath.
        let page = showing.place.page;
        let size = self.sizes[page];
        let scale = showing.keys.first().map(|(key, _)| key.scale);
        let rescaled = self
            .tiles
            .iter()
            .filter(|(key, _)| key.page == page && key.rotate == self.rotate && Some(key.scale) != scale)
            .map(|(key, image)| (image.clone(), fraction(key, size)));
        let kept = self
            .backdrop
            .iter()
            .filter(|old| old.key.page == page && old.key.rotate == self.rotate && old.size == size)
            .map(|old| (old.image.clone(), fraction(&old.key, old.size)));
        kept.chain(rescaled).chain(current).collect()
    }

    /// Keeps every tile under its page, as before an edit.
    pub fn keep_all(&mut self) {
        let sizes = &self.sizes;
        let old = self.tiles.drain().map(|(key, image)| Old { key, image, size: sizes[key.page] });
        self.backdrop.extend(old);
    }

    /// Moves unseen tiles under their page; drops covered ones.
    pub fn settle(&mut self, showing: &[Showing]) {
        let shown = |page: usize| showing.iter().find(|s| s.place.page == page);
        let seen = |key: &TileKey| shown(key.page).is_some_and(|s| s.keys.iter().any(|(k, _)| k == key));
        let leaving: Vec<TileKey> = self.tiles.keys().filter(|key| !seen(key)).copied().collect();
        for key in leaving {
            let image = self.tiles.remove(&key).expect("a tile just listed");
            match shown(key.page) {
                Some(_) => self.backdrop.push(Old { key, image, size: self.sizes[key.page] }),
                None => self.retired.push(image),
            }
        }
        let covered = |page: usize| shown(page).is_none_or(|s| s.keys.iter().all(|(key, _)| self.tiles.contains_key(key)));
        let (sizes, rotate) = (&self.sizes, self.rotate);
        let (gone, kept): (Vec<Old>, Vec<Old>) = std::mem::take(&mut self.backdrop).into_iter().partition(|old| {
            covered(old.key.page) || old.key.rotate != rotate || sizes.get(old.key.page) != Some(&old.size)
        });
        self.backdrop = kept;
        for old in gone {
            self.retired.push(old.image);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tile_keeps_its_place_as_a_fraction_of_its_page() {
        let key = TileKey { page: 0, scale: 2000, rotate: 0, tx: 1, ty: 0 };
        // A 612 by 792 page at 2x: 1224 by 1584.
        assert_eq!(fraction(&key, (612.0, 792.0)), [512.0 / 1224.0, 0.0, 512.0 / 1224.0, 512.0 / 1584.0]);
        let turned = TileKey { rotate: 90, ..key };
        assert_eq!(fraction(&turned, (612.0, 792.0)), [512.0 / 1584.0, 0.0, 512.0 / 1584.0, 512.0 / 1224.0]);
    }
}
