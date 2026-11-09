//! What-if scenario commands over the JSON bridge (ECMA-376 Part 1, §18.3.1 `scenarios`):
//! list, add / edit, delete, show, merge from another sheet and the summary report.
//!
//! While a sheet is protected with `sheetProtection/@scenarios` (§18.3.1.85), hidden
//! scenarios are not listed, locked ones cannot be changed or deleted and none can be added.

use super::cmd::Recalc;
use super::protect::scenarios_protected;
use super::SheetEngine;
use crate::{EngineError, Result};
use bo_model::sheet::{sqref_to_string, CellRange, CellRef, Scenario, Scenarios};
use bo_sheet::scenario::{self as sc, ScenarioError, SummaryLabels};
use serde_json::{json, Value};

fn bad(msg: impl Into<String>) -> EngineError {
    EngineError::Sheet(msg.into())
}

fn err(e: ScenarioError) -> EngineError {
    bad(e.message())
}

const PROTECTED: &str = super::protect::SHEET_PROTECTED;

/// A1 cells / ranges separated by commas, semicolons or spaces (`$` and a sheet prefix
/// are ignored).
fn parse_ranges(s: &str) -> Result<Vec<CellRange>> {
    s.split(|c: char| c == ',' || c == ';' || c.is_whitespace())
        .filter(|t| !t.is_empty())
        .map(|t| {
            let t = t.rsplit_once('!').map_or(t, |(_, r)| r).replace('$', "");
            CellRange::parse(&t)
                .or_else(|| CellRef::parse(&t).map(|c| CellRange::new(c, c)))
                .ok_or_else(|| bad(format!("invalid cell address '{t}'")))
        })
        .collect()
}

fn scenario_json(i: usize, s: &Scenario) -> Value {
    json!({
        "index": i,
        "name": s.name,
        "cells": s.cells.iter().map(|c| c.cell.to_string()).collect::<Vec<_>>(),
        "values": s.cells.iter().map(|c| c.value.clone()).collect::<Vec<_>>(),
        "comment": s.comment,
        "user": s.user,
        "locked": s.locked,
        "hidden": s.hidden,
    })
}

/// Fields of a scenario add / edit request.
pub(super) struct SaveRequest {
    pub index: Option<usize>,
    pub name: String,
    pub cells: String,
    pub values: Option<Vec<String>>,
    pub comment: Option<String>,
    pub user: Option<String>,
    pub locked: bool,
    pub hidden: bool,
}

impl SheetEngine {
    /// `{ items, current, show, results, protected }`; hidden scenarios are left out while
    /// scenarios are protected (each item keeps its real `index`).
    pub(super) fn scenario_list(&self, sheet: usize) -> Result<Value> {
        let ws = self.sheet(sheet)?;
        let locked_down = scenarios_protected(ws);
        let empty = Scenarios::default();
        let list = ws.scenarios.as_ref().unwrap_or(&empty);
        let items: Vec<Value> = list
            .items
            .iter()
            .enumerate()
            .filter(|(_, s)| !(locked_down && s.hidden))
            .map(|(i, s)| scenario_json(i, s))
            .collect();
        Ok(json!({
            "items": items,
            "current": list.current,
            "show": list.show,
            "results": sqref_to_string(&list.sqref),
            "protected": locked_down,
        }))
    }

    /// Add or replace a scenario (one undo step); returns `{ index }`.
    pub(super) fn scenario_save(&mut self, sheet: usize, req: SaveRequest) -> Result<Value> {
        let ws = self.sheet(sheet)?;
        let existing = ws
            .scenarios
            .as_ref()
            .and_then(|l| req.index.and_then(|i| l.items.get(i)));
        if req.index.is_some() && existing.is_none() {
            return Err(err(ScenarioError::NotFound));
        }
        if scenarios_protected(ws) && existing.is_none_or(|s| s.locked) {
            return Err(bad(PROTECTED));
        }
        let cells = sc::cells_of(&parse_ranges(&req.cells)?);
        let values = match req.values {
            Some(v) => v,
            // Without values: what the scenario had for a cell, else the cell's value now.
            None => cells
                .iter()
                .map(|c| {
                    existing
                        .and_then(|s| s.cells.iter().find(|x| x.cell == *c))
                        .map_or_else(|| sc::current_text(ws, *c), |x| x.value.clone())
                })
                .collect(),
        };
        let user = req.user.or_else(|| existing.and_then(|s| s.user.clone()));
        let scenario = sc::build(
            &req.name,
            &cells,
            &values,
            req.comment,
            user,
            req.locked,
            req.hidden,
        )
        .map_err(err)?;
        let index = req.index;
        self.edit(sheet, Recalc::No, |wb, _| {
            let list = wb.sheets[sheet]
                .scenarios
                .get_or_insert_with(Scenarios::default);
            let i = sc::save(list, index, scenario).map_err(err)?;
            Ok(json!({ "index": i }))
        })
    }

    /// Delete a scenario (one undo step).
    pub(super) fn scenario_delete(&mut self, sheet: usize, index: usize) -> Result<Value> {
        let ws = self.sheet(sheet)?;
        let s = ws
            .scenarios
            .as_ref()
            .and_then(|l| l.items.get(index))
            .ok_or_else(|| err(ScenarioError::NotFound))?;
        if scenarios_protected(ws) && s.locked {
            return Err(bad(PROTECTED));
        }
        self.edit(sheet, Recalc::No, |wb, _| {
            let ws = &mut wb.sheets[sheet];
            if let Some(list) = ws.scenarios.as_mut() {
                sc::delete(list, index).map_err(err)?;
                if list.items.is_empty() && list.sqref.is_empty() {
                    ws.scenarios = None;
                }
            }
            Ok(json!({ "ok": true }))
        })
    }

    /// Enter a scenario's values into its changing cells (one undo step; locked cells of a
    /// protected sheet refuse the edit).
    pub(super) fn scenario_show(&mut self, sheet: usize, index: usize) -> Result<Value> {
        let ws = self.sheet(sheet)?;
        let s = ws
            .scenarios
            .as_ref()
            .and_then(|l| l.items.get(index))
            .ok_or_else(|| err(ScenarioError::NotFound))?;
        if scenarios_protected(ws) && s.hidden {
            return Err(err(ScenarioError::NotFound));
        }
        let s = s.clone();
        self.edit(sheet, Recalc::All, |wb, _| {
            let ws = &mut wb.sheets[sheet];
            sc::show(ws, &s);
            if let Some(list) = ws.scenarios.as_mut() {
                let i = u32::try_from(index).ok();
                list.current = i;
                list.show = i;
            }
            Ok(json!({ "ok": true }))
        })
    }

    /// Copy the scenarios of sheet `from` into `sheet` (one undo step); `{ merged }`.
    pub(super) fn scenario_merge(&mut self, sheet: usize, from: usize) -> Result<Value> {
        let ws = self.sheet(sheet)?;
        if scenarios_protected(ws) {
            return Err(bad(PROTECTED));
        }
        if from == sheet {
            return Err(bad("choose another sheet to merge from"));
        }
        let src = self.sheet(from)?;
        let hide = scenarios_protected(src);
        let items: Vec<Scenario> = src
            .scenarios
            .as_ref()
            .map(|l| {
                l.items
                    .iter()
                    .filter(|s| !(hide && s.hidden))
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();
        if items.is_empty() {
            return Ok(json!({ "merged": 0 }));
        }
        self.edit(sheet, Recalc::No, |wb, _| {
            let list = wb.sheets[sheet]
                .scenarios
                .get_or_insert_with(Scenarios::default);
            Ok(json!({ "merged": sc::merge(list, &items) }))
        })
    }

    /// Summary report on a new sheet (one undo step); `results` are the result cells
    /// (absent: those chosen last). Returns `{ sheet }`, the new sheet's index.
    pub(super) fn scenario_summary(
        &mut self,
        sheet: usize,
        results: Option<&str>,
        labels: Option<SummaryLabels>,
    ) -> Result<Value> {
        super::protect::check_structure(self.wb()?)?;
        let ws = self.sheet(sheet)?;
        let locked_down = scenarios_protected(ws);
        let list = ws.scenarios.clone().unwrap_or_default();
        let shown: Vec<&Scenario> = list
            .items
            .iter()
            .filter(|s| !(locked_down && s.hidden))
            .collect();
        if shown.is_empty() {
            return Err(bad("the sheet has no scenarios"));
        }
        let ranges = match results.map(str::trim) {
            Some(t) => parse_ranges(t)?,
            None => list.sqref.clone(),
        };
        let mut cells: Vec<CellRef> = Vec::new();
        for r in &ranges {
            for row in r.first.row..=r.last.row {
                for col in r.first.col..=r.last.col {
                    let c = CellRef::new(row, col);
                    if !cells.contains(&c) {
                        cells.push(c);
                    }
                }
            }
            if cells.len() > 1000 {
                return Err(bad("too many result cells"));
            }
        }
        let labels = labels.unwrap_or_default();
        let wb = self.wb()?;
        let summary = sc::summary(wb, sheet, &shown, &cells);
        let report = sc::summary_sheet(wb, sheet, &shown, &summary, &labels);
        let out = self.edit(sheet, Recalc::All, |wb, _| {
            if !locked_down {
                if let Some(l) = wb.sheets[sheet].scenarios.as_mut() {
                    l.sqref = ranges;
                }
            }
            wb.sheets.push(report);
            Ok(json!({ "sheet": wb.sheets.len() - 1 }))
        })?;
        // A new sheet needs its own geometry slot.
        self.invalidate(None);
        Ok(out)
    }
}
