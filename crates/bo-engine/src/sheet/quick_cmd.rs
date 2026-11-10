//! Quick Analysis and recommended charts of BlueSheet (CORE-DESIGN-001). Both commands only
//! read the workbook; the UI applies its choices through the normal commands (conditional
//! formats, cell inputs, charts, tables, sparklines) in one `batch`, i.e. one undo step.
//!
//! * `recommendCharts { sheet, range, width?, height?, limit? }` → `{ shape, charts }`: chart
//!   types ranked by [`bo_sheet::chart_recommend`] with an SVG preview of each, drawn by the
//!   shared chart renderer from the chart `insertChart` would insert.
//! * `quickAnalysis { sheet, range }` → the data shape, the numeric rows and columns, and
//!   whether the row below / the column right of the range are free for totals.

use super::cmd::{current_region, Rng};
use super::SheetEngine;
use crate::{EngineError, Result};
use bo_model::sheet::CellValue;
use bo_sheet::chart_recommend as cr;
use serde::Deserialize;
use serde_json::{json, Value};

fn bad(msg: impl Into<String>) -> EngineError {
    EngineError::Sheet(msg.into())
}

/// A Quick Analysis command.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "cmd", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum QuickCommand {
    /// Ranked chart types for a range, with previews `width` × `height` points.
    RecommendCharts {
        /// 0-based sheet index.
        sheet: usize,
        /// The selected data.
        range: Rng,
        /// Preview width in points (default 160).
        #[serde(default)]
        width: Option<f64>,
        /// Preview height in points (default 100).
        #[serde(default)]
        height: Option<f64>,
        /// Most recommendations (default 6).
        #[serde(default)]
        limit: Option<usize>,
    },
    /// What Quick Analysis offers for a range.
    QuickAnalysis {
        /// 0-based sheet index.
        sheet: usize,
        /// The selected cells.
        range: Rng,
    },
}

impl QuickCommand {
    /// Parses a JSON command when it is a Quick Analysis command.
    pub fn parse(v: &Value) -> Option<QuickCommand> {
        let name = v.get("cmd")?.as_str()?;
        if !matches!(name, "recommendCharts" | "quickAnalysis") {
            return None;
        }
        serde_json::from_value(v.clone()).ok()
    }
}

impl SheetEngine {
    /// Runs a Quick Analysis command (nothing changes).
    pub(super) fn quick_command(&self, cmd: &QuickCommand) -> Result<Value> {
        match *cmd {
            QuickCommand::RecommendCharts {
                sheet,
                range,
                width,
                height,
                limit,
            } => {
                let wb = self.wb()?;
                // The data `insertChart` would plot.
                let r = current_region(self.sheet(sheet)?, range.range());
                let shape = cr::shape_of(wb, sheet, r);
                let (w, h) = (
                    width.unwrap_or(160.0).clamp(40.0, 600.0),
                    height.unwrap_or(100.0).clamp(30.0, 400.0),
                );
                let charts: Vec<Value> = cr::recommend(wb, sheet, r, limit.unwrap_or(6))
                    .into_iter()
                    .filter_map(|rec| {
                        let chart = bo_sheet::chart::new_chart(wb, sheet, r, rec.kind)?;
                        let svg = bo_sheet::chart_style::preview_svg(
                            &chart,
                            w,
                            h,
                            &bo_sheet::chart::default_color,
                        );
                        Some(json!({
                            "kind": rec.kind, "score": rec.score, "reason": rec.reason, "svg": svg,
                        }))
                    })
                    .collect();
                Ok(json!({ "shape": shape, "range": Rng::of(r), "charts": charts }))
            }
            QuickCommand::QuickAnalysis { sheet, range } => {
                let wb = self.wb()?;
                let ws = self.sheet(sheet)?;
                let r = range.range();
                if r.rows() * r.cols() < 2 {
                    return Err(bad("select at least two cells"));
                }
                let number = |row: u32, col: u32| {
                    matches!(
                        ws.cell(row, col).map(|c| &c.value),
                        Some(CellValue::Number(_))
                    )
                };
                let empty = |row: u32, col: u32| {
                    ws.cell(row, col)
                        .is_none_or(|c| matches!(c.value, CellValue::Empty) && c.formula.is_none())
                };
                let shape = cr::shape_of(wb, sheet, r);
                let top = r.first.row + u32::from(shape.as_ref().is_some_and(|s| s.header));
                let left = r.first.col + shape.as_ref().map_or(0, |s| s.label_cols);
                let numeric_cols: Vec<u32> = (r.first.col..=r.last.col)
                    .filter(|&c| (top..=r.last.row).any(|row| number(row, c)))
                    .collect();
                let numeric_rows: Vec<u32> = (top..=r.last.row)
                    .filter(|&row| (left..=r.last.col).any(|c| number(row, c)))
                    .collect();
                let below = r.last.row + 1;
                let right = r.last.col + 1;
                let below_free =
                    below < super::MAX_ROWS && (r.first.col..=r.last.col).all(|c| empty(below, c));
                let right_free = right < super::MAX_COLS
                    && (r.first.row..=r.last.row).all(|row| empty(row, right));
                let region = current_region(ws, r);
                Ok(json!({
                    "shape": shape,
                    "header": shape.as_ref().is_some_and(|s| s.header),
                    "labelCols": shape.as_ref().map_or(0, |s| s.label_cols),
                    "numericCols": numeric_cols,
                    "numericRows": numeric_rows,
                    "belowFree": below_free,
                    "rightFree": right_free,
                    "region": Rng::of(region),
                }))
            }
        }
    }
}
