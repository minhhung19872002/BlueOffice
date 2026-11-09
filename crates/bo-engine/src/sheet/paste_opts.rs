//! Extra BlueSheet paste options (CORE-PASTE-001): keep the copied columns'
//! widths (ECMA-376 Part 1, §18.3.1.13 col `@width`) and paste without borders
//! (§18.8.4 border), applied in the same undo step as the paste.

use super::SheetEngine;
use crate::Result;
use bo_model::sheet::{CellRange, Workbook};
use bo_sheet::geometry::SheetMetrics;
use bo_sheet::style::{BorderPreset, BorderSpec};

/// What a paste does besides the cells.
#[derive(Debug, Clone, Default)]
pub(super) struct PasteExtras {
    /// `(destination column, width in points)` of copied columns with a set width.
    widths: Vec<(u32, f64)>,
    /// Metrics of the destination sheet (width conversion).
    metrics: Option<SheetMetrics>,
    /// Remove the borders of the pasted cells.
    no_borders: bool,
}

impl SheetEngine {
    /// Prepare the extras of a paste at column `col` of `sheet` (before the edit).
    pub(super) fn paste_extras(
        &mut self,
        sheet: usize,
        col: u32,
        transpose: bool,
        col_widths: bool,
        no_borders: bool,
    ) -> Result<PasteExtras> {
        let mut out = PasteExtras {
            no_borders,
            ..PasteExtras::default()
        };
        // Transposed columns become rows: no width to carry over.
        let Some(clip) = self.clip.clone().filter(|_| col_widths && !transpose) else {
            return Ok(out);
        };
        let (first, last) = (clip.range.first.col, clip.range.last.col);
        let explicit: Vec<u32> = {
            let src = self
                .wb()?
                .sheets
                .get(clip.sheet)
                .map(|ws| ws.cols.as_slice())
                .unwrap_or_default();
            (first..=last)
                .filter(|c| {
                    src.iter()
                        .any(|x| x.min <= *c && *c <= x.max && (x.width.is_some() || x.hidden))
                })
                .collect()
        };
        if explicit.is_empty() {
            return Ok(out);
        }
        let g = self.geometry(clip.sheet)?;
        out.widths = explicit
            .into_iter()
            .map(|c| (col + (c - first), g.col_width(c)))
            .collect();
        out.metrics = Some(self.geometry(sheet)?.metrics);
        Ok(out)
    }
}

/// Apply the extras to the pasted `target` range (inside the paste's edit).
pub(super) fn apply(wb: &mut Workbook, sheet: usize, target: CellRange, extras: &PasteExtras) {
    if extras.no_borders {
        let spec = BorderSpec {
            preset: BorderPreset::None,
            style: String::new(),
            color: None,
        };
        bo_sheet::style::apply_borders(wb, sheet, target, &spec);
    }
    if let (Some(m), Some(ws)) = (extras.metrics, wb.sheets.get_mut(sheet)) {
        for &(c, pt) in &extras.widths {
            bo_sheet::rowcol::set_col_width(ws, &m, c, c, pt);
        }
    }
}
