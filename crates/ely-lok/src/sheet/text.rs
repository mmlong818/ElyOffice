//! Text offsets stay local to a pixel-aligned cell anchor.

use super::Geometry;

impl Geometry {
    /// Text geometry at `ppt` device pixels per twip.
    pub fn text_rect(
        &self,
        (x, y, w, h): crate::Rect,
        anchor: Option<(i32, i32)>,
        ppt: f64,
    ) -> crate::Rect {
        if let Some((ax, ay)) = anchor {
            return (
                self.columns.tile(ax, ppt) + x,
                self.rows.tile(ay, ppt) + y,
                w,
                h,
            );
        }
        let (left, top) = (self.columns.tile(x, ppt), self.rows.tile(y, ppt));
        let (right, bottom) = (self.columns.tile(x + w, ppt), self.rows.tile(y + h, ppt));
        (left, top, right - left, bottom - top)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sheet() -> Geometry {
        super::super::geometry(
            r#"{
            "columns":{"sizes":"1280:999", "hidden":"0:999"},
            "rows":{"sizes":"256:1999", "hidden":"0:1999"}
        }"#,
        )
        .unwrap()
    }

    #[test]
    fn text_keeps_its_offset_at_distant_rows_and_scales() {
        let sheet = sheet();
        for (ppt, anchor, want) in [
            (1.0 / 15.0, (2560, 0), (2867, 15, 0, 292)),
            (1.0 / 15.0, (2560, 2560), (2867, 2565, 0, 292)),
            (1.0 / 15.0, (2560, 256000), (2867, 255015, 0, 292)),
            (1.0 / 12.0, (2560, 256000), (2861, 252015, 0, 292)),
            (1.0 / 30.0, (2560, 256000), (2837, 240015, 0, 292)),
        ] {
            assert_eq!(sheet.text_rect((317, 15, 0, 292), Some(anchor), ppt), want);
        }
    }

    #[test]
    fn an_absolute_text_rectangle_still_follows_sheet_geometry() {
        assert_eq!(
            sheet().text_rect((2560, 256000, 1280, 256), None, 1.0 / 15.0),
            (2550, 255000, 1275, 255)
        );
    }
}
