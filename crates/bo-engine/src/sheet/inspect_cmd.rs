//! BlueSheet commands of the "Check document" panel (CORE-INSPECT-001):
//! `inspect`, `inspectRemove`, `inspectKeep`.

use super::cmd::Recalc;
use super::SheetEngine;
use crate::inspect::{self, Category};
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

    pub(super) fn inspect_keep(&mut self, categories: &[Category]) -> Result<Value> {
        for c in categories {
            self.scrub.set(*c, false);
        }
        self.inspect_json()
    }
}
