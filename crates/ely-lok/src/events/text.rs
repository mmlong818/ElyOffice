//! Print-twips anchors for local text geometry.

use std::io;

use super::{
    Events,
    parse::{cursor_of, selection_of},
};
use crate::ipc::{Pack, Unpack};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
/// Cell anchors shared with the cursor and text selection.
pub struct TextAnchors {
    pub cursor: Option<(i32, i32)>,
    pub selection: Option<(i32, i32)>,
}

impl TextAnchors {
    pub(crate) fn pack(self, mut pack: Pack) -> Pack {
        for point in [self.cursor, self.selection] {
            pack = pack.u64(u64::from(point.is_some()));
            if let Some((x, y)) = point {
                pack = pack.i64(x.into()).i64(y.into());
            }
        }
        pack
    }

    pub(crate) fn unpack(unpack: &mut Unpack) -> io::Result<Self> {
        let mut point = || -> io::Result<_> {
            match unpack.u64()? {
                0 => Ok(None),
                1 => Ok(Some((
                    i32::try_from(unpack.i64()?).map_err(io::Error::other)?,
                    i32::try_from(unpack.i64()?).map_err(io::Error::other)?,
                ))),
                _ => Err(io::Error::other("a bad text anchor flag")),
            }
        };
        Ok(Self {
            cursor: point()?,
            selection: point()?,
        })
    }
}

pub(super) fn cursor(batch: &mut Events, text: &str) {
    match cursor_of(text) {
        Ok((rect, anchor)) => {
            batch.cursor = rect;
            batch.text.cursor = anchor;
        }
        Err(_) => {
            batch.cursor = None;
            batch.text.cursor = None;
            batch.faults.push(format!("an unreadable cursor: {text}"));
        }
    }
}

pub(super) fn selection(batch: &mut Events, text: &str) {
    match selection_of(text) {
        Ok((rects, anchor)) => {
            batch.selection = rects;
            batch.text.selection = anchor;
        }
        Err(_) => {
            batch.selection.clear();
            batch.text.selection = None;
            batch
                .faults
                .push(format!("an unreadable selection: {text}"));
        }
    }
}
