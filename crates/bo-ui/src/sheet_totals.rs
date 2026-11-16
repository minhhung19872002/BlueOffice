//! Totals row drop-down of a table (round 35): with the active cell in a table's totals row a
//! drop-down button sits just right of the cell, as the reference; it lists the total
//! functions (None, Average, Count, Count Numbers, Max, Min, Sum, StdDev, Var, More
//! Functions…) and writes the choice with engine `tableTotal` (a SUBTOTAL formula, ECMA-376
//! Part 1, §18.5.1.3 `totalsRowFunction`).
use super::SheetWindow;
use crate::{Input, Key, Painter, PathSeg, TextStyle};
use bo_render::Rgba;
use serde_json::json;

/// Total functions: (engine name, label key, fallback).
pub(super) const TOTALS: [(&str, &str, &str); 10] = [
    ("none", "sheetNative.totNone", "None"),
    ("average", "sheetNative.totAverage", "Average"),
    ("count", "sheetNative.totCount", "Count"),
    ("countNums", "sheetNative.totCountNums", "Count Numbers"),
    ("max", "sheetNative.totMax", "Max"),
    ("min", "sheetNative.totMin", "Min"),
    ("sum", "sheetNative.totSum", "Sum"),
    ("stdDev", "sheetNative.totStdDev", "StdDev"),
    ("var", "sheetNative.totVar", "Var"),
    ("custom", "sheetNative.totMore", "More Functions..."),
];

/// The open function list of the totals drop-down.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct TotalsList {
    /// Window rectangle.
    pub rect: [f32; 4],
    /// Row height (device pixels).
    pub row: f32,
    /// Highlighted row (the current function, then the pointer).
    pub hot: Option<usize>,
}

impl SheetWindow {
    /// (table index, column of the table) when the active cell is in a totals row.
    pub(super) fn totals_cell(&self) -> Option<(usize, usize)> {
        let (r, c) = self.view.active();
        let ws = self
            .view
            .engine()
            .workbook()?
            .sheets
            .get(self.view.sheet())?;
        ws.tables.iter().enumerate().find_map(|(i, t)| {
            let totals = t.totals_row_count.unwrap_or(0) > 0;
            (totals && r == t.range.last.row && c >= t.range.first.col && c <= t.range.last.col)
                .then(|| (i, (c - t.range.first.col) as usize))
        })
    }

    /// Window rectangle of the totals drop-down button.
    pub(super) fn totals_button(&self) -> Option<[f32; 4]> {
        self.totals_cell()?;
        if self.view.editing() {
            return None;
        }
        let (r, c) = self.view.active();
        let [_, top, right, bottom] = self.view.cell_box(r, c)?;
        let h = (bottom - top).max(self.px(12.0));
        Some([right + 2.0, top, h, h])
    }

    pub(super) fn paint_totals_button(&self, p: &mut dyn Painter) {
        let Some([x, y, w, h]) = self.totals_button() else {
            return;
        };
        p.fill_rect(x, y, w, h, Rgba(0xab, 0xab, 0xab, 255));
        p.fill_rect(
            x + 1.0,
            y + 1.0,
            w - 2.0,
            h - 2.0,
            Rgba(0xff, 0xff, 0xff, 255),
        );
        let k = (w * 0.18).round().max(3.0);
        let (cx, cy) = ((x + w / 2.0).round(), (y + h / 2.0).round());
        p.path(
            &[
                PathSeg::M(cx - k, cy - k / 2.0),
                PathSeg::L(cx + k, cy - k / 2.0),
                PathSeg::L(cx, cy + k / 2.0),
                PathSeg::Z,
            ],
            Some(Rgba(0x44, 0x44, 0x44, 255)),
            None,
        );
    }

    /// A press on the totals button opens the function list.
    pub(super) fn totals_press(&mut self, x: f32, y: f32) -> bool {
        let Some([bx, by, bw, bh]) = self.totals_button() else {
            return false;
        };
        if !(x >= bx && x < bx + bw && y >= by && y < by + bh) {
            return false;
        }
        if self.totals_list.take().is_some() {
            return true;
        }
        // The column's current function is shown selected.
        let current = self.totals_cell().and_then(|(t, c)| {
            let ws = self
                .view
                .engine()
                .workbook()?
                .sheets
                .get(self.view.sheet())?;
            let f = ws
                .tables
                .get(t)?
                .columns
                .get(c)?
                .totals_row_function
                .clone()?;
            TOTALS
                .iter()
                .position(|(name, ..)| name.eq_ignore_ascii_case(&f))
        });
        // A compact list box under the button, as the reference: 20 px rows at 125 % in
        // the UI font and a 1 px border (the metrics of the AutoFilter list).
        let (row, w) = (self.px(16.0), self.px(112.0));
        let h = row * TOTALS.len() as f32 + 2.0;
        let left = (bx + bw - w).max(0.0);
        // Every item shows (as the reference): under the button, else above it, else as
        // low as the window allows.
        let top = if by + bh + h <= self.size.1 {
            by + bh
        } else if by - h >= 0.0 {
            by - h
        } else {
            (self.size.1 - h).max(0.0)
        };
        self.totals_list = Some(TotalsList {
            rect: [left, top, w, h],
            row,
            hot: current,
        });
        true
    }

    /// Input while the function list is open: `Some(redraw)` when it took the input.
    pub(super) fn totals_list_input(&mut self, input: &Input) -> Option<bool> {
        let l = self.totals_list.as_mut()?;
        let [x0, y0, w, h] = l.rect;
        let row = l.row;
        let at = move |x: f32, y: f32| {
            (x >= x0 && x < x0 + w && y >= y0 + 1.0 && y < y0 + h - 1.0)
                .then(|| (((y - y0 - 1.0) / row) as usize).min(TOTALS.len() - 1))
        };
        Some(match *input {
            Input::Move { x, y } => {
                let hit = at(x, y);
                if hit.is_some() && hit != l.hot {
                    l.hot = hit;
                    true
                } else {
                    false
                }
            }
            Input::Press { x, y, .. } => {
                let hit = at(x, y);
                self.totals_list = None;
                match hit {
                    Some(i) if TOTALS[i].0 != "custom" => {
                        self.totals_action(&format!("total.{}", TOTALS[i].0));
                    }
                    // More Functions…: the Insert Function dialog for the totals cell.
                    Some(_) => {
                        self.open_insert_function();
                    }
                    // A press elsewhere closes the list and is handled as usual (a press on
                    // the button only closes it).
                    None => {
                        if let Some([bx, by, bw, bh]) = self.totals_button() {
                            if x >= bx && x < bx + bw && y >= by && y < by + bh {
                                return Some(true);
                            }
                        }
                        return None;
                    }
                }
                true
            }
            Input::Key(Key::Escape, _) => {
                self.totals_list = None;
                true
            }
            Input::Key(k @ (Key::Down | Key::Up), _) => {
                let n = TOTALS.len();
                let i = l.hot.unwrap_or(0);
                l.hot = Some(if k == Key::Down {
                    (i + 1).min(n - 1)
                } else {
                    i.saturating_sub(1)
                });
                true
            }
            Input::Key(Key::Enter, _) => {
                let i = l.hot;
                self.totals_list = None;
                match i {
                    Some(i) if TOTALS[i].0 != "custom" => {
                        self.totals_action(&format!("total.{}", TOTALS[i].0));
                    }
                    Some(_) => {
                        self.open_insert_function();
                    }
                    None => {}
                }
                true
            }
            Input::Release { .. } | Input::Drag { .. } | Input::Wheel(_) => true,
            _ => return None,
        })
    }

    /// The open function list: white with a 1 px grey border, the hot / current row in the
    /// system selection blue with white text.
    pub(super) fn paint_totals_list(&self, p: &mut dyn Painter) {
        let Some(l) = &self.totals_list else {
            return;
        };
        let [x, y, w, h] = l.rect;
        p.fill_rect(x, y, w, h, Rgba(0x82, 0x87, 0x90, 255));
        p.fill_rect(x + 1.0, y + 1.0, w - 2.0, h - 2.0, Rgba(255, 255, 255, 255));
        let style = TextStyle {
            size: self.px(12.0),
            bold: false,
        };
        for (i, (_, key, fallback)) in TOTALS.iter().enumerate() {
            let ry = y + 1.0 + i as f32 * l.row;
            let hot = l.hot == Some(i);
            if hot {
                p.fill_rect(x + 1.0, ry, w - 2.0, l.row, Rgba(0x00, 0x78, 0xD7, 255));
            }
            let ink = if hot {
                Rgba(255, 255, 255, 255)
            } else {
                Rgba(0, 0, 0, 255)
            };
            let ty = (ry + (l.row - style.size * 1.33) / 2.0).round();
            p.text(&self.t(key, fallback), x + self.px(3.0), ty, style, ink);
        }
    }

    /// `total.<function>` menu commands.
    pub(super) fn totals_action(&mut self, id: &str) -> Option<bool> {
        let f = id.strip_prefix("total.")?;
        let (table, column) = self.totals_cell()?;
        Some(self.view.run(json!({
            "cmd": "tableTotal", "table": table, "column": column, "function": f,
        })))
    }
}
