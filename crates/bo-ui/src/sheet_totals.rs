//! Totals row drop-down of a table (round 35): with the active cell in a table's totals row a
//! drop-down button sits just right of the cell, as the reference; it lists the total
//! functions (None, Average, Count, Count Numbers, Max, Min, Sum, StdDev, Var, More
//! Functions…) and writes the choice with engine `tableTotal` (a SUBTOTAL formula, ECMA-376
//! Part 1, §18.5.1.3 `totalsRowFunction`).
use super::SheetWindow;
use crate::popup::{Choice, Menu, MenuItem};
use crate::{Painter, PathSeg};
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
        let items = TOTALS
            .iter()
            .map(|(f, key, fallback)| MenuItem {
                label: self.t(key, fallback),
                choice: Choice::Command(format!("total.{f}")),
                checked: false,
                dim: *f == "custom",
            })
            .collect();
        self.menu = Some(Menu::open(
            items,
            [bx, by + bh, bw, 0.0],
            self.size,
            self.scale,
        ));
        true
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
