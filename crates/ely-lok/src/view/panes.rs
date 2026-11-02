//! Frozen rows and columns stay; the rest scrolls (TABLE-02).

/// One axis: frozen extent and scroll past it, in twips.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct Axis {
    pub frozen: i32,
    pub scroll: i32,
}

impl Axis {
    /// Whether a document span intersects a visible pane.
    pub fn visible(self, start: i32, length: i32, view: i32) -> bool {
        if length < 0 || view <= 0 {
            return false;
        }
        let (start, end) = (
            i64::from(start),
            i64::from(start) + i64::from(length.max(1)),
        );
        let (frozen, scroll, view) = (
            i64::from(self.frozen),
            i64::from(self.scroll),
            i64::from(view),
        );
        (frozen > 0 && start < frozen.min(view) && end > 0)
            || (frozen < view && start < scroll + view && end > scroll + frozen)
    }

    /// Where document `doc` shows, in twips from the view's start.
    pub fn show(self, doc: i32) -> i32 {
        if doc < self.frozen {
            doc
        } else {
            (doc - self.scroll).max(self.frozen)
        }
    }

    /// The document point under view twips `at`.
    pub fn doc(self, at: i32) -> i32 {
        if at < self.frozen {
            at
        } else {
            at + self.scroll
        }
    }

    /// Each pane: document start and end, and its view start.
    pub fn panes(self, length: i32) -> Vec<(i32, i32, i32)> {
        let mut panes = Vec::with_capacity(2);
        if self.frozen > 0 {
            panes.push((0, self.frozen, 0));
        }
        let start = self.frozen + self.scroll;
        panes.push((start, start + (length - self.frozen).max(0), self.frozen));
        panes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hidden_spans_do_not_become_edge_anchors() {
        let axis = Axis {
            frozen: 0,
            scroll: 4000,
        };
        for (start, length, visible) in [
            (317, 0, false),
            (4000, 0, true),
            (6999, 0, true),
            (7000, 0, false),
            (3990, 20, true),
            (500, 100, false),
            (-10, 5, false),
            (-10, 20, false),
        ] {
            assert_eq!(
                axis.visible(start, length, 3000),
                visible,
                "{start}, {length}"
            );
        }
    }

    #[test]
    fn frozen_spans_stay_visible_but_the_hidden_band_does_not() {
        let axis = Axis {
            frozen: 300,
            scroll: 1000,
        };
        for (start, length, visible) in [
            (100, 0, true),
            (400, 20, false),
            (1290, 20, true),
            (1300, 0, true),
            (299, 10, true),
            (3000, 0, false),
        ] {
            assert_eq!(
                axis.visible(start, length, 2000),
                visible,
                "{start}, {length}"
            );
        }
        let axis = Axis {
            frozen: 1000,
            scroll: 500,
        };
        assert!(axis.visible(200, 20, 500));
        assert!(!axis.visible(600, 20, 500));
        assert!(!axis.visible(1600, 20, 500));
        assert!(!axis.visible(0, 1, 0));
    }

    #[test]
    fn the_frozen_part_stays_and_the_rest_scrolls() {
        let axis = Axis {
            frozen: 300,
            scroll: 1000,
        };
        assert_eq!(axis.show(100), 100, "frozen rows stay");
        assert_eq!(axis.show(1500), 500, "the rest moves by the scroll");
        assert_eq!(axis.show(800), 300, "the hidden band folds to the line");
        assert_eq!(axis.doc(100), 100);
        assert_eq!(axis.doc(500), 1500);
        assert_eq!(axis.panes(2000), [(0, 300, 0), (1300, 3000, 300)]);
    }

    #[test]
    fn without_a_freeze_one_pane_scrolls() {
        let axis = Axis {
            frozen: 0,
            scroll: 400,
        };
        assert_eq!(axis.show(1000), 600);
        assert_eq!(axis.doc(600), 1000);
        assert_eq!(axis.panes(2000), [(400, 2400, 0)]);
    }
}
