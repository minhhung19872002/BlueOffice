//! BlueReader round 7: keyboard order of the Print dialog. Tab / Shift+Tab move a focus ring
//! through every control in reading order (left column top to bottom, then the middle
//! column, the preview's buttons, then Page Setting, OK, Cancel), as a Windows dialog does;
//! a text box with the ring takes typing; Space presses the control with the ring.
use super::{Field, Part, PrintDialog};
use crate::Painter;
use bo_render::Rgba;

/// Focus ring: dark blue at full strength (≥ 3 : 1 on the dialog's white, WCAG 1.4.11).
const RING: Rgba = Rgba(0, 84, 153, 255);

impl PrintDialog {
    /// Controls in keyboard order.
    pub(super) fn tab_order(&self) -> Vec<Part> {
        let s = self.scale;
        let f = self.frame;
        let mut v: Vec<(u8, f32, f32, Part)> = self
            .parts
            .iter()
            .filter(|(p, _)| {
                !matches!(
                    p,
                    Part::Close
                        | Part::Item(_)
                        | Part::CopiesUp
                        | Part::CopiesDown
                        | Part::TileZoomUp
                        | Part::TileZoomDown
                        | Part::OverlapUp
                        | Part::OverlapDown
                )
            })
            .map(|&(p, r)| {
                let (x, y) = ((r[0] - f[0]) / s, (r[1] - f[1]) / s);
                let col = match (p, x, y) {
                    (Part::PageSetting | Part::Ok | Part::Cancel, ..) => 3,
                    // The top row (printer, copies, options) reads left to right first.
                    (_, _, y) if y < 120.0 => 0,
                    (_, x, _) if x < 480.0 => 1,
                    (_, x, _) if x < 800.0 => 2,
                    _ => 4,
                };
                let col = if col == 4 { 3 } else { col };
                (col, y, x, p)
            })
            .collect();
        v.sort_by(|a, b| {
            a.0.cmp(&b.0).then(if a.0 == 0 {
                a.1.total_cmp(&b.1).then(a.2.total_cmp(&b.2))
            } else if a.0 == 3 {
                // Preview buttons, Page Setting, OK, Cancel: by row then left to right.
                (a.1 / 30.0)
                    .floor()
                    .total_cmp(&(b.1 / 30.0).floor())
                    .then(a.2.total_cmp(&b.2))
            } else {
                a.1.total_cmp(&b.1).then(a.2.total_cmp(&b.2))
            })
        });
        v.into_iter().map(|t| t.3).collect()
    }

    /// Tab (or Shift+Tab when `back`): the next control takes the focus ring.
    pub(super) fn tab_key(&mut self, back: bool) {
        let order = self.tab_order();
        if order.is_empty() {
            return;
        }
        let n = order.len();
        let at = self
            .kbd
            .and_then(|k| order.iter().position(|p| *p == k))
            .or_else(|| {
                // Start from the text box with the typing focus.
                let p = match self.focus {
                    Field::Copies => Part::Copies,
                    Field::Pages => Part::PagesText,
                    Field::Scale => Part::ScaleText,
                    Field::TileZoom => Part::TileZoom,
                    Field::Overlap => Part::Overlap,
                };
                order.iter().position(|q| *q == p)
            });
        let next = match (at, back) {
            (Some(i), false) => (i + 1) % n,
            (Some(i), true) => (i + n - 1) % n,
            (None, false) => 0,
            (None, true) => n - 1,
        };
        let part = order[next];
        self.kbd = Some(part);
        self.focus = match part {
            Part::Copies => Field::Copies,
            Part::PagesText => Field::Pages,
            Part::ScaleText => Field::Scale,
            Part::TileZoom => Field::TileZoom,
            Part::Overlap => Field::Overlap,
            _ => self.focus,
        };
    }

    /// Whether the control with the ring takes typing.
    pub(super) fn ring_on_text(&self) -> bool {
        matches!(
            self.kbd,
            None | Some(
                Part::Copies | Part::PagesText | Part::ScaleText | Part::TileZoom | Part::Overlap
            )
        )
    }

    /// The control with the ring and its centre, for Space.
    pub(super) fn ring_centre(&self) -> Option<(f32, f32)> {
        let k = self.kbd?;
        let r = self.rect(k);
        (r[2] > 0.0).then(|| (r[0] + r[2] / 2.0, r[1] + r[3] / 2.0))
    }

    /// The focus ring.
    pub(super) fn paint_ring(&self, p: &mut dyn Painter) {
        let Some(k) = self.kbd else {
            return;
        };
        let r = self.rect(k);
        if r[2] <= 0.0 {
            return;
        }
        let w = self.scale.max(1.0).round();
        let (x, y, rw, rh) = (
            (r[0] - 2.0).round(),
            (r[1] - 2.0).round(),
            (r[2] + 4.0).round(),
            (r[3] + 4.0).round(),
        );
        p.fill_rect(x, y, rw, w, RING);
        p.fill_rect(x, y + rh - w, rw, w, RING);
        p.fill_rect(x, y, w, rh, RING);
        p.fill_rect(x + rw - w, y, w, rh, RING);
    }
}
