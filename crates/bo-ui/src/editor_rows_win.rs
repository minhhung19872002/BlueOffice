//! Row buttons of the ribbon (a child of `reader_window`, used by BlueEditor's Organize and
//! Comment tabs): a small icon with its label on the right, three to a column, as the
//! reference editor shows Move / Duplicate / Replace. Sizes measured at 125 % with 110 % text
//! (`docs/ui/REPORT-editor.md`, round 4): rows 29 px apart from 15 px below the ribbon body's
//! top, 22 px icons 4 px from the column's left, labels 26 px from it, a column ends 11 px
//! after its widest label.
use super::{reader_icons, rounded, tri, Control, Kind, ReaderWindow, TEXT};
use crate::reader_icons::Ink;
use crate::{Painter, TextMeasure};
use bo_render::Rgba;

/// Row pitch, first row's top below the body's top, icon size and offset, label offset and
/// the space after the widest label (logical pixels).
const ROW_H: f32 = 23.2;
const ROW_TOP: f32 = 2.4;
const ICON: f32 = 17.6;
const ICON_X: f32 = 3.2;
const LABEL_X: f32 = 20.8;
const AFTER: f32 = 8.8;
/// Drop-down arrow space after a label.
const ARROW: f32 = 11.2;

/// The column of row buttons being laid out.
#[derive(Default)]
pub(super) struct Rows {
    /// Index in `controls` of the column's first row and how many rows it has.
    first: usize,
    n: usize,
    /// Widest row so far.
    w: f32,
}

impl ReaderWindow {
    /// A reader pointer tool beyond Hand / Select is on (drawing, measure, eraser, area
    /// highlight, stamps, SnapShot): BlueEditor turns its own tools off when one is chosen.
    pub(crate) fn extra_tool_on(&self) -> bool {
        self.tabs4.tool.is_some() || self.draw.tool.is_some()
    }

    /// The status bar message (tests).
    #[cfg(test)]
    pub(crate) fn status_text(&self) -> &str {
        &self.status
    }

    /// Lay out row button `id` at `x` (the column's left); returns the next x.
    pub(super) fn row_layout(
        &mut self,
        rows: &mut Rows,
        id: &'static str,
        kind: Kind,
        x: f32,
        by: f32,
        m: &dyn TextMeasure,
    ) -> f32 {
        let Kind::Row(_, label, dropdown) = kind else {
            return x;
        };
        let mut x = x;
        if rows.n == 3 {
            x = self.rows_flush(rows, x);
        }
        if rows.n == 0 {
            rows.first = self.controls.len();
        }
        let text = self.t(label);
        let style = self.style(self.look.label, false);
        let lw = self.measure(m, &text, style);
        let w =
            (self.px(LABEL_X) + lw + if dropdown { self.px(ARROW) } else { 0.0 } + self.px(AFTER))
                .round();
        let y = by + self.px(ROW_TOP) + rows.n as f32 * self.px(ROW_H);
        self.controls.push(Control {
            id,
            kind,
            r: [x, y, w, self.px(ROW_H) - 1.0],
        });
        rows.w = rows.w.max(w);
        rows.n += 1;
        x
    }

    /// End the column of row buttons (all rows as wide as the widest); returns the next x.
    pub(super) fn rows_flush(&mut self, rows: &mut Rows, x: f32) -> f32 {
        if rows.n == 0 {
            return x;
        }
        let w = rows.w;
        for c in self.controls.iter_mut().skip(rows.first).take(rows.n) {
            c.r[2] = w;
        }
        *rows = Rows::default();
        x + w
    }

    /// Paint row button `c`.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn paint_row(
        &self,
        p: &mut dyn Painter,
        c: &Control,
        ink: Ink,
        on: bool,
        hot: bool,
        down: bool,
    ) {
        let Kind::Row(icon, label, dropdown) = c.kind else {
            return;
        };
        let r = c.r;
        let off = !self.enabled(c.id);
        let grey = Rgba(175, 175, 176, 255);
        let (ink, label_ink) = if off {
            (
                Ink {
                    ink: grey,
                    accent: grey,
                },
                grey,
            )
        } else {
            (ink, TEXT)
        };
        if !off && (on || down || hot) {
            let fill = if on || down {
                self.look.pressed
            } else {
                self.look.hover
            };
            p.path(
                &rounded(r[0], r[1], r[2], r[3], self.px(2.4)),
                Some(fill),
                None,
            );
        }
        let is = self.px(ICON);
        reader_icons::draw_weight(
            p,
            icon,
            r[0] + self.px(ICON_X),
            r[1] + ((r[3] - is) / 2.0).round(),
            is,
            ink,
            self.look.icon_stroke,
        );
        let style = self.style(self.look.label, false);
        let text = self.t(label);
        let ty = r[1] + ((r[3] - style.size * 1.33) / 2.0).round();
        let lx = r[0] + self.px(LABEL_X);
        p.text(&text, lx, ty, style, label_ink);
        if dropdown {
            let lw = self.width_of(&text, style);
            tri(
                p,
                lx + lw + self.px(3.2),
                r[1] + r[3] / 2.0 - self.px(1.2),
                self.px(3.2),
                label_ink,
            );
        }
    }
}
