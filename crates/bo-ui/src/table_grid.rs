//! Insert › Table drop-down of BlueDoc: a 10 × 8 grid to pick the table size by pointing, its
//! "c × r Table" title, then text commands, as on a desktop word processor.
use crate::theme::Theme;
use crate::{Painter, TextStyle};
use bo_render::Rgba;

const COLS: usize = 10;
const ROWS: usize = 8;
const CELL_EDGE: Rgba = Rgba(171, 171, 171, 255);
const MENU_EDGE: Rgba = Rgba(198, 198, 198, 255);

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
    /// Labels of the commands under the grid (already translated).
    commands: Vec<String>,
}

impl TableGrid {
    pub(crate) fn open(anchor: [f32; 4], commands: Vec<String>, scale: f32) -> Self {
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

    fn cell(&self) -> f32 {
        self.px(19.0)
    }

    fn title_h(&self) -> f32 {
        self.px(28.0)
    }

    fn grid_origin(&self) -> (f32, f32) {
        (
            self.x + self.px(8.0),
            self.y + self.title_h() + self.px(4.0),
        )
    }

    fn item_h(&self) -> f32 {
        self.px(28.0)
    }

    fn items_top(&self) -> f32 {
        self.grid_origin().1 + ROWS as f32 * self.cell() + self.px(10.0)
    }

    fn size(&self) -> (f32, f32) {
        let w = COLS as f32 * self.cell() + self.px(16.0);
        let h =
            self.items_top() - self.y + self.commands.len() as f32 * self.item_h() + self.px(4.0);
        (w.max(self.px(220.0)), h)
    }

    pub(crate) fn contains(&self, x: f32, y: f32) -> bool {
        let (w, h) = self.size();
        x >= self.x && x < self.x + w && y >= self.y && y < self.y + h
    }

    fn pick_at(&self, x: f32, y: f32) -> Option<GridPick> {
        let (gx, gy) = self.grid_origin();
        let c = self.cell();
        if x >= gx && y >= gy && x < gx + COLS as f32 * c && y < gy + ROWS as f32 * c {
            let col = ((x - gx) / c) as usize + 1;
            let row = ((y - gy) / c) as usize + 1;
            return Some(GridPick::Size(col.min(COLS), row.min(ROWS)));
        }
        let top = self.items_top();
        if y >= top && self.contains(x, y) {
            let i = ((y - top) / self.item_h()) as usize;
            return (i < self.commands.len()).then_some(GridPick::Command(i));
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
        p.fill_rect(self.x, self.y, w, h, MENU_EDGE);
        p.fill_rect(self.x + 1.0, self.y + 1.0, w - 2.0, h - 2.0, t.surface);
        let style = TextStyle {
            size: self.px(12.0),
            bold: false,
        };
        // Title band: the size being picked, else the heading.
        p.fill_rect(
            self.x + 1.0,
            self.y + 1.0,
            w - 2.0,
            self.title_h() - 1.0,
            Rgba(240, 240, 240, 255),
        );
        let heading = match self.hover {
            Some((c, r)) if c < 100 => size_label(c, r),
            _ => title.to_owned(),
        };
        let ty = |top: f32, hh: f32| top + (hh - style.size * 1.33) / 2.0;
        p.text(
            &heading,
            self.x + self.px(10.0),
            ty(self.y, self.title_h()),
            TextStyle {
                bold: true,
                ..style
            },
            t.text,
        );
        let (gx, gy) = self.grid_origin();
        let c = self.cell();
        let inner = c - self.px(3.0);
        let (hc, hr) = match self.hover {
            Some((c, r)) if c < 100 => (c, r),
            _ => (0, 0),
        };
        for row in 0..ROWS {
            for col in 0..COLS {
                let (x, y) = (gx + col as f32 * c, gy + row as f32 * c);
                let on = col < hc && row < hr;
                let (edge, fill) = if on {
                    (t.accent, Rgba(255, 228, 196, 255))
                } else {
                    (CELL_EDGE, Rgba(255, 255, 255, 255))
                };
                p.fill_rect(x, y, inner, inner, edge);
                p.fill_rect(x + 1.0, y + 1.0, inner - 2.0, inner - 2.0, fill);
            }
        }
        let top = self.items_top();
        p.fill_rect(self.x + 1.0, top - self.px(5.0), w - 2.0, 1.0, MENU_EDGE);
        for (i, label) in self.commands.iter().enumerate() {
            let y = top + i as f32 * self.item_h();
            if self.hover == Some((100 + i, 0)) {
                p.fill_rect(self.x + 1.0, y, w - 2.0, self.item_h(), t.hover);
            }
            p.text(
                label,
                self.x + self.px(34.0),
                ty(y, self.item_h()),
                style,
                t.text,
            );
        }
    }
}
