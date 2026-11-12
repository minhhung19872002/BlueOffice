//! BlueSheet commands of the "Check document" panel (CORE-INSPECT-001):
//! `inspect`, `inspectRemove`, `inspectKeep`, `inspectFix` (quick fixes of the
//! accessibility checker: unmerge, table header row, sheet name).

use super::cmd::{Recalc, Rng, SheetCommand};
use super::SheetEngine;
use crate::inspect::{self, Category, Fix, Target};
use crate::Result;
use serde_json::Value;

impl SheetEngine {
    /// Check document report of the open workbook.
    pub fn inspect_workbook(&self) -> Result<inspect::Report> {
        Ok(inspect::sheet::report(self.wb()?, &self.scrub))
    }

    pub(super) fn inspect_json(&self) -> Result<Value> {
        serde_json::to_value(self.inspect_workbook()?)
            .map_err(|e| crate::EngineError::Sheet(e.to_string()))
    }

    pub(super) fn inspect_remove(&mut self, categories: &[Category]) -> Result<Value> {
        for c in categories {
            self.scrub.set(*c, true);
        }
        let model: Vec<Category> = categories
            .iter()
            .copied()
            .filter(|c| {
                matches!(
                    c,
                    Category::Comments
                        | Category::HiddenRows
                        | Category::HiddenColumns
                        | Category::HiddenSheets
                        | Category::HeadersFooters
                )
            })
            .collect();
        if !model.is_empty() {
            self.edit(0, Recalc::All, |wb, adj| {
                inspect::sheet::remove(wb, &model, adj)
            })?;
            // Rows, columns or sheets may have gone: every sheet is laid out again.
            self.invalidate(None);
        }
        self.inspect_json()
    }

    /// Apply an accessibility quick fix (one undo step); returns the new report, with
    /// `error` when the fix does not apply.
    pub(super) fn inspect_fix(&mut self, fix: &Fix) -> Result<Value> {
        let res: std::result::Result<(), String> = match fix {
            Fix::Unmerge {
                target:
                    Target::Range {
                        sheet,
                        r0,
                        c0,
                        r1,
                        c1,
                    },
            } => self
                .exec(SheetCommand::Unmerge {
                    sheet: *sheet,
                    range: Rng {
                        r0: *r0,
                        c0: *c0,
                        r1: *r1,
                        c1: *c1,
                    },
                })
                .map(|_| ())
                .map_err(|e| e.to_string()),
            Fix::HeaderRow {
                target: Target::Range { sheet, r0, c0, .. },
            } => {
                let table = self.wb()?.sheets.get(*sheet).and_then(|ws| {
                    ws.tables
                        .iter()
                        .position(|t| t.range.first.row == *r0 && t.range.first.col == *c0)
                });
                match table {
                    Some(table) => self
                        .table_update(
                            *sheet,
                            table,
                            &super::table_cmd::TableUpdate {
                                header_row: Some(true),
                                ..Default::default()
                            },
                        )
                        .map(|_| ())
                        .map_err(|e| e.to_string()),
                    None => Err("no table there".to_owned()),
                }
            }
            Fix::Rename {
                target: Target::Sheet { sheet },
                text,
            } => self
                .rename_sheet(*sheet, text.trim())
                .map_err(|e| e.to_string()),
            _ => Err("this fix does not apply to a workbook".to_owned()),
        };
        let mut v = self.inspect_json()?;
        if let (Err(e), Value::Object(m)) = (res, &mut v) {
            m.insert("error".into(), Value::String(e));
        }
        Ok(v)
    }

    pub(super) fn inspect_keep(&mut self, categories: &[Category]) -> Result<Value> {
        for c in categories {
            self.scrub.set(*c, false);
        }
        self.inspect_json()
    }
}
