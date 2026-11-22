//! Insert › Table drop-down of BlueDoc, laid out after the reference desktop word processor's
//! as captured at 125 % on this machine (round 11): a 283 × 457 px popup with a grey
//! "Insert Table" / "c x r Table" band, a 7 × 6 grid of 37 px cells 39 px apart (the hovered
//! ones framed in orange), then the command rows (36 px): Insert Table…, Draw Table,
//! Convert Text to Table…, an embedded spreadsheet (not available), Quick Tables ▸. Icons are BlueOffice's own.
use crate::theme::Theme;
use crate::{Painter, TextStyle};
use bo_render::{PathSeg, Rgba};

const COLS: usize = 7;
const ROWS: usize = 6;
/// Logical sizes (device px at 125 % ÷ 1.25).
const W: f32 = 226.4;
const BAND: f32 = 28.0;
const GRID_X: f32 = 5.6;
const GRID_Y: f32 = 31.2;
const PITCH: f32 = 31.2;
const CELL: f32 = 29.6;
const ITEM_H: f32 = 28.8;
const ITEMS_Y: f32 = 220.8;
const TEXT_X: f32 = 36.8;
const POPUP_EDGE: Rgba = Rgba(148, 148, 148, 255);
const BAND_FILL: Rgba = Rgba(235, 235, 235, 255);
const CELL_EDGE: Rgba = Rgba(118, 118, 118, 255);
const HOT_OUT: Rgba = Rgba(239, 72, 16, 255);
const HOT_IN: Rgba = Rgba(255, 226, 148, 255);
const INK: Rgba = Rgba(38, 38, 38, 255);
const DIM: Rgba = Rgba(160, 160, 160, 255);

/// What a press in the drop-down picked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GridPick {
    /// A table of (columns, rows).
    Size(usize, usize),
    /// Command entry `n` of the list under the grid.
    Command(usize),
}

/// The open drop-down.
pub(crate) struct TableGrid {
    /// Top-left corner.
    x: f32,
    y: f32,
    scale: f32,
    /// Hovered (columns, rows) size, or command index + 100 in `.0` with rows 0.
    hover: Option<(usize, usize)>,
    /// The commands under the grid (already translated) and whether each can run.
    commands: Vec<(String, bool)>,
}

impl TableGrid {
    pub(crate) fn open(anchor: [f32; 4], commands: Vec<(String, bool)>, scale: f32) -> Self {
        Self {
            x: anchor[0],
            y: anchor[1] + anchor[3],
            scale,
            hover: None,
            commands,
        }
    }

    fn px(&self, v: f32) -> f32 {
        (v * self.scale).round()
    }

    fn grid_origin(&self) -> (f32, f32) {
        (self.x + self.px(GRID_X), self.y + self.px(GRID_Y))
    }

    fn items_top(&self) -> f32 {
        self.y + self.px(ITEMS_Y)
    }

    fn size(&self) -> (f32, f32) {
        let h = ITEMS_Y + self.commands.len() as f32 * ITEM_H + 3.0;
        (self.px(W), self.px(h))
    }

    pub(crate) fn contains(&self, x: f32, y: f32) -> bool {
        let (w, h) = self.size();
        x >= self.x && x < self.x + w && y >= self.y && y < self.y + h
    }

    fn pick_at(&self, x: f32, y: f32) -> Option<GridPick> {
        let (gx, gy) = self.grid_origin();
        let c = PITCH * self.scale;
        if x >= gx && y >= gy && x < gx + COLS as f32 * c && y < gy + ROWS as f32 * c {
            let col = ((x - gx) / c) as usize + 1;
            let row = ((y - gy) / c) as usize + 1;
            return Some(GridPick::Size(col.min(COLS), row.min(ROWS)));
        }
        let top = self.items_top();
        if y >= top && self.contains(x, y) {
            let i = ((y - top) / self.px(ITEM_H)) as usize;
            return self
                .commands
                .get(i)
                .filter(|c| c.1)
                .map(|_| GridPick::Command(i));
        }
        None
    }

    pub(crate) fn hover(&mut self, x: f32, y: f32) -> bool {
        let h = match self.pick_at(x, y) {
            Some(GridPick::Size(c, r)) => Some((c, r)),
            Some(GridPick::Command(i)) => Some((100 + i, 0)),
            None => None,
        };
        let changed = h != self.hover;
        self.hover = h;
        changed
    }

    /// Device rectangle of command row `i` (Quick Tables opens beside its row).
    pub(crate) fn row_rect(&self, i: usize) -> [f32; 4] {
        let (w, _) = self.size();
        let ih = self.px(ITEM_H);
        [self.x, self.items_top() + i as f32 * ih, w, ih]
    }

    pub(crate) fn press(&self, x: f32, y: f32) -> Option<GridPick> {
        self.pick_at(x, y)
    }

    /// Paint over the window; `title` is the "Insert Table" heading, `size_label` formats
    /// the hovered size (`3x4 Table`).
    pub(crate) fn paint(
        &self,
        p: &mut dyn Painter,
        t: &Theme,
        title: &str,
        size_label: &dyn Fn(usize, usize) -> String,
    ) {
        let (w, h) = self.size();
        p.fill_rect(self.x, self.y, w, h, POPUP_EDGE);
        p.fill_rect(
            self.x + 1.0,
            self.y + 1.0,
            w - 2.0,
            h - 2.0,
            Rgba(255, 255, 255, 255),
        );
        let style = TextStyle {
            size: self.px(13.2),
            bold: false,
        };
        // Band: the size being picked, else the heading (bold dark grey on light grey).
        p.fill_rect(
            self.x + 1.0,
            self.y + 1.0,
            w - 2.0,
            self.px(BAND),
            BAND_FILL,
        );
        let heading = match self.hover {
            Some((c, r)) if c < 100 => size_label(c, r),
            _ => title.to_owned(),
        };
        let ty = |top: f32, hh: f32| top + (hh - style.size * 1.33) / 2.0;
        p.text(
            &heading,
            self.x + self.px(8.0),
            ty(self.y + 1.0, self.px(BAND)),
            TextStyle {
                bold: true,
                ..style
            },
            Rgba(80, 80, 80, 255),
        );
        let (gx, gy) = self.grid_origin();
        let (pitch, cell) = (PITCH * self.scale, self.px(CELL));
        let (hc, hr) = match self.hover {
            Some((c, r)) if c < 100 => (c, r),
            _ => (0, 0),
        };
        for row in 0..ROWS {
            for col in 0..COLS {
                let (x, y) = (
                    (gx + col as f32 * pitch).round(),
                    (gy + row as f32 * pitch).round(),
                );
                if col < hc && row < hr {
                    p.fill_rect(x, y, cell, cell, HOT_OUT);
                    p.fill_rect(x + 1.0, y + 1.0, cell - 2.0, cell - 2.0, HOT_IN);
                    p.fill_rect(
                        x + 2.0,
                        y + 2.0,
                        cell - 4.0,
                        cell - 4.0,
                        Rgba(255, 255, 255, 255),
                    );
                } else {
                    p.fill_rect(x, y, cell, cell, CELL_EDGE);
                    p.fill_rect(
                        x + 1.0,
                        y + 1.0,
                        cell - 2.0,
                        cell - 2.0,
                        Rgba(255, 255, 255, 255),
                    );
                }
            }
        }
        let top = self.items_top();
        let ih = self.px(ITEM_H);
        for (i, (label, on)) in self.commands.iter().enumerate() {
            let y = top + i as f32 * ih;
            if *on && self.hover == Some((100 + i, 0)) {
                p.fill_rect(self.x + 2.0, y, w - 4.0, ih, t.hover);
            }
            let color = if *on { INK } else { DIM };
            self.icon(
                p,
                i,
                self.x + self.px(6.4),
                y + (ih - self.px(16.0)) / 2.0,
                color,
            );
            p.text(label, self.x + self.px(TEXT_X), ty(y, ih), style, color);
            // Quick Tables opens a sub-menu: ▸ at the right.
            if i == 4 {
                let (ax, ay, k) = (self.x + w - self.px(14.0), y + ih / 2.0, self.px(3.5));
                p.path(
                    &[
                        PathSeg::M(ax, ay - k),
                        PathSeg::L(ax + k, ay),
                        PathSeg::L(ax, ay + k),
                        PathSeg::Z,
                    ],
                    Some(color),
                    None,
                );
            }
        }
    }

    /// Own 16 px icons: a table, a pencil over a table, text lines into a table, a sheet with
    /// a green corner, a table with a header band.
    fn icon(&self, p: &mut dyn Painter, i: usize, x: f32, y: f32, ink: Rgba) {
        let s = self.px(16.0);
        let lw = self.scale.max(1.0);
        let grid = |p: &mut dyn Painter, x: f32, y: f32, w: f32, h: f32| {
            p.path(
                &[
                    PathSeg::M(x, y),
                    PathSeg::L(x + w, y),
                    PathSeg::L(x + w, y + h),
                    PathSeg::L(x, y + h),
                    PathSeg::Z,
                    PathSeg::M(x + w / 3.0, y),
                    PathSeg::L(x + w / 3.0, y + h),
                    PathSeg::M(x + 2.0 * w / 3.0, y),
                    PathSeg::L(x + 2.0 * w / 3.0, y + h),
                    PathSeg::M(x, y + h / 2.0),
                    PathSeg::L(x + w, y + h / 2.0),
                ],
                None,
                Some((ink, lw)),
            );
        };
        match i {
            1 => {
                grid(p, x, y + s * 0.3, s * 0.7, s * 0.6);
                p.path(
                    &[
                        PathSeg::M(x + s * 0.45, y + s * 0.75),
                        PathSeg::L(x + s, y + s * 0.15),
                    ],
                    None,
                    Some((Rgba(0, 102, 204, 255), lw * 2.0)),
                );
            }
            2 => {
                for k in 0..3 {
                    p.fill_rect(x, y + s * (0.15 + k as f32 * 0.2), s * 0.45, lw, ink);
                }
                grid(p, x + s * 0.5, y + s * 0.4, s * 0.5, s * 0.5);
            }
            3 => {
                grid(p, x, y + s * 0.1, s * 0.8, s * 0.8);
                p.fill_rect(
                    x + s * 0.55,
                    y + s * 0.55,
                    s * 0.45,
                    s * 0.45,
                    Rgba(33, 115, 70, 255),
                );
            }
            4 => {
                p.fill_rect(x, y + s * 0.15, s, s * 0.22, Rgba(120, 120, 120, 255));
                grid(p, x, y + s * 0.15, s, s * 0.75);
            }
            _ => grid(p, x, y + s * 0.15, s, s * 0.75),
        }
    }
}
