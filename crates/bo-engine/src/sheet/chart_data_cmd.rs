//! Chart data commands of BlueSheet: the data a chart plots (Select Data Source), Switch
//! Row/Column and Move Chart (to another sheet or a new sheet). Series references are
//! `c:ser/c:tx`, `c:cat`, `c:val` formulas (ECMA-376 Part 1, §21.2.2.215 tx, §21.2.2.24 cat,
//! §21.2.2.224 val). Edits go through [`SheetEngine::edit`] (one undo step each).

use super::cmd::Recalc;
use super::SheetEngine;
use crate::{EngineError, Result};
use bo_model::sheet::{CellRange, CellRef, CellValue, Chart, ChartSeries, Workbook, Worksheet};
use bo_sheet::cells::{abs_ref, resolve_ref};
use serde::Deserialize;
use serde_json::{json, Value};

fn bad(msg: impl Into<String>) -> EngineError {
    EngineError::Sheet(msg.into())
}

/// One series of `setChartData` (Select Data Source › Edit Series).
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SeriesSpec {
    /// Series name reference (`=Sheet1!$B$1`).
    pub name_ref: Option<String>,
    /// Series name typed as text (used when there is no reference).
    pub name: Option<String>,
    /// Values reference (`=Sheet1!$B$2:$B$5`).
    pub val_ref: Option<String>,
    /// Category labels reference; absent keeps the chart's categories.
    pub cat_ref: Option<String>,
}

/// The block of cells a chart plots: sheet, range (with the name row / column and the category
/// column / row), and whether each series is a row of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Source {
    pub sheet: usize,
    pub range: CellRange,
    pub by_row: bool,
}

fn union(a: CellRange, b: CellRange) -> CellRange {
    CellRange::new(
        CellRef::new(a.first.row.min(b.first.row), a.first.col.min(b.first.col)),
        CellRef::new(a.last.row.max(b.last.row), a.last.col.max(b.last.col)),
    )
}

fn all_series(c: &Chart) -> impl Iterator<Item = &ChartSeries> {
    c.plots.iter().flat_map(|p| p.series.iter())
}

/// The block of cells chart `c` (on sheet `sheet`) plots, when its series are the rows or the
/// columns of one rectangle (the charts Insert › Charts makes and most others).
pub(crate) fn source(wb: &Workbook, sheet: usize, c: &Chart) -> Option<Source> {
    let mut si = None;
    let mut vals = Vec::new();
    let mut extra: Option<CellRange> = None;
    for s in all_series(c) {
        let (vs, vr) = resolve_ref(wb, sheet, s.val_ref.as_deref()?)?;
        if *si.get_or_insert(vs) != vs {
            return None;
        }
        vals.push(vr);
        for f in [s.name_ref.as_deref(), s.cat_ref.as_deref()]
            .into_iter()
            .flatten()
        {
            let (fs, fr) = resolve_ref(wb, sheet, f)?;
            if fs != vs {
                return None;
            }
            extra = Some(extra.map_or(fr, |e| union(e, fr)));
        }
    }
    let first = *vals.first()?;
    let same = |f: fn(&CellRange) -> (u32, u32)| vals.iter().all(|v| f(v) == f(&first));
    let cols = |r: &CellRange| (r.first.col, r.last.col);
    let rows = |r: &CellRange| (r.first.row, r.last.row);
    // A single cell per series counts as a column (the reference's default orientation).
    let by_row = if vals.iter().all(|v| v.cols() == 1) && same(rows) {
        false
    } else if vals.iter().all(|v| v.rows() == 1) && same(cols) {
        true
    } else {
        return None;
    };
    let mut range = vals.iter().skip(1).fold(first, |a, b| union(a, *b));
    if let Some(e) = extra {
        range = union(range, e);
    }
    Some(Source {
        sheet: si?,
        range,
        by_row,
    })
}

fn is_text(ws: &Worksheet, r: u32, c: u32) -> bool {
    ws.cell(r, c)
        .is_some_and(|c| matches!(c.value, CellValue::Text(_) | CellValue::RichText(_)))
}

/// Series references for the data in `range` of sheet `sheet`, one series per row (`by_row`) or
/// per column: a leading text row / column holds the series names and a leading label column /
/// row the categories, as the reference's Select Data Source reads a range.
pub(crate) fn series_for(
    wb: &Workbook,
    sheet: usize,
    range: CellRange,
    by_row: bool,
) -> Vec<ChartSeries> {
    let Some(ws) = wb.sheets.get(sheet) else {
        return Vec::new();
    };
    // Work in (major, minor) coordinates: a series runs along `minor`.
    let at = |major: u32, minor: u32| {
        if by_row {
            (major, minor)
        } else {
            (minor, major)
        }
    };
    let (maj0, maj1, min0, min1) = if by_row {
        (
            range.first.row,
            range.last.row,
            range.first.col,
            range.last.col,
        )
    } else {
        (
            range.first.col,
            range.last.col,
            range.first.row,
            range.last.row,
        )
    };
    // Names: the first cell of each series line is text (any of them).
    let names = min1 > min0
        && (maj0..=maj1).any(|m| {
            let (r, c) = at(m, min0);
            is_text(ws, r, c)
        });
    let v0 = min0 + u32::from(names);
    // Categories: the first series line holds only labels (text, dates or blanks).
    let cats = maj1 > maj0
        && (v0..=min1).all(|n| {
            let (r, c) = at(maj0, n);
            bo_sheet::chart_recommend::is_label(wb, ws, r, c)
        });
    let s0 = maj0 + u32::from(cats);
    if s0 > maj1 || v0 > min1 {
        return Vec::new();
    }
    let name = &ws.name;
    let line = |m: u32, a: u32, b: u32| {
        let (r0, c0) = at(m, a);
        let (r1, c1) = at(m, b);
        abs_ref(
            name,
            CellRange::new(CellRef::new(r0, c0), CellRef::new(r1, c1)),
        )
    };
    let cat_ref = cats.then(|| line(maj0, v0, min1));
    (s0..=maj1)
        .map(|m| ChartSeries {
            name_ref: names.then(|| line(m, min0, min0)),
            cat_ref: cat_ref.clone(),
            val_ref: Some(line(m, v0, min1)),
            ..ChartSeries::default()
        })
        .collect()
}

/// Put `series` into chart `c` in place of its series: formatting stays with the series of the
/// same position, the plot groups keep their counts (extra series join the last group, groups
/// left empty go, the first always stays).
fn replace_series(c: &mut Chart, series: Vec<ChartSeries>) {
    let mut old: Vec<ChartSeries> = Vec::new();
    let mut counts = Vec::new();
    for p in &mut c.plots {
        counts.push(p.series.len());
        old.append(&mut p.series);
    }
    let merged: Vec<ChartSeries> = series
        .into_iter()
        .enumerate()
        .map(|(i, s)| {
            let mut out = old.get(i).cloned().unwrap_or_default();
            out.name_ref = s.name_ref;
            out.name = s.name;
            out.cat_ref = s.cat_ref;
            out.val_ref = s.val_ref;
            out.categories = s.categories;
            out.values = s.values;
            out
        })
        .collect();
    let n = c.plots.len();
    let mut it = merged.into_iter();
    for (k, p) in c.plots.iter_mut().enumerate() {
        if k + 1 == n {
            p.series.extend(it.by_ref());
        } else {
            p.series.extend(it.by_ref().take(counts[k]));
        }
    }
    let mut first = true;
    c.plots
        .retain(|p| std::mem::take(&mut first) || !p.series.is_empty());
    c.dirty = true;
}

impl SheetEngine {
    /// `chartData`: the chart's data range (when it is one block), orientation and series.
    pub(super) fn chart_data(&self, sheet: usize, index: usize) -> Result<Value> {
        let wb = self.wb()?;
        let c = self
            .sheet(sheet)?
            .charts
            .get(index)
            .ok_or_else(|| bad("no such chart"))?;
        let live = self.live_chart(sheet, c);
        let src = source(wb, sheet, c);
        let series: Vec<Value> = all_series(&live)
            .map(|s| {
                json!({
                    "name": s.name,
                    "nameRef": s.name_ref,
                    "valRef": s.val_ref,
                    "catRef": s.cat_ref,
                    "categories": s.categories,
                })
            })
            .collect();
        Ok(json!({
            "range": src.and_then(|s| wb.sheets.get(s.sheet).map(|ws| abs_ref(&ws.name, s.range))),
            "byRow": src.is_some_and(|s| s.by_row),
            "series": series,
            "categories": all_series(c).next().and_then(|s| s.cat_ref.clone()),
        }))
    }

    /// `setChartData`: plot `range` (series by row or by column; default: as now, or by column),
    /// or the given `series` list; `categories` sets every series' category labels.
    pub(super) fn set_chart_data(
        &mut self,
        sheet: usize,
        index: usize,
        range: Option<&str>,
        by_row: Option<bool>,
        series: Option<&[SeriesSpec]>,
        categories: Option<&str>,
    ) -> Result<Value> {
        let wb = self.wb()?;
        let c = self
            .sheet(sheet)?
            .charts
            .get(index)
            .ok_or_else(|| bad("no such chart"))?;
        let now = source(wb, sheet, c);
        let mut new: Vec<ChartSeries> = if let Some(f) = range {
            let (si, r) = resolve_ref(wb, sheet, f).ok_or_else(|| bad("not a range"))?;
            let by_row = by_row.unwrap_or_else(|| now.is_some_and(|s| s.by_row));
            series_for(wb, si, r, by_row)
        } else if let Some(list) = series {
            list.iter()
                .map(|s| {
                    let opt = |v: &Option<String>| {
                        v.as_deref()
                            .map(str::trim)
                            .filter(|t| !t.is_empty() && *t != "=")
                            .map(|t| t.trim_start_matches('=').to_owned())
                    };
                    ChartSeries {
                        name_ref: opt(&s.name_ref),
                        name: s.name.clone().filter(|n| !n.trim().is_empty()),
                        val_ref: opt(&s.val_ref),
                        cat_ref: opt(&s.cat_ref),
                        ..ChartSeries::default()
                    }
                })
                .collect()
        } else if let Some(s) = now.filter(|_| by_row.is_some()) {
            series_for(wb, s.sheet, s.range, by_row.unwrap_or(s.by_row))
        } else {
            all_series(c).cloned().collect()
        };
        if let Some(f) = categories {
            let f = f.trim().trim_start_matches('=');
            for s in &mut new {
                s.cat_ref = (!f.is_empty()).then(|| f.to_owned());
            }
        }
        if new.is_empty() || new.iter().any(|s| s.val_ref.is_none()) {
            return Err(bad("no data for a chart"));
        }
        for s in &new {
            for f in [&s.val_ref, &s.name_ref, &s.cat_ref].into_iter().flatten() {
                resolve_ref(wb, sheet, f).ok_or_else(|| bad(format!("not a range: {f}")))?;
            }
        }
        // Cached data of the new references.
        let locale = self.locale;
        for s in &mut new {
            let (name, cats, vals) = bo_sheet::chart::series_data(wb, sheet, s, &locale);
            s.name = name.or_else(|| s.name.take());
            s.categories = cats;
            s.values = vals;
        }
        self.edit(sheet, Recalc::No, |wb, _| {
            let c = wb.sheets[sheet]
                .charts
                .get_mut(index)
                .ok_or_else(|| bad("no such chart"))?;
            replace_series(c, new);
            Ok(json!({ "ok": true }))
        })
    }

    /// `switchRowColumn`: series become categories and categories series.
    pub(super) fn switch_row_column(&mut self, sheet: usize, index: usize) -> Result<Value> {
        let wb = self.wb()?;
        let c = self
            .sheet(sheet)?
            .charts
            .get(index)
            .ok_or_else(|| bad("no such chart"))?;
        let s = source(wb, sheet, c).ok_or_else(|| bad("the chart's data is not one range"))?;
        let f = abs_ref(&wb.sheets[s.sheet].name, s.range);
        self.set_chart_data(sheet, index, Some(&f), Some(!s.by_row), None, None)
    }

    /// `moveChartTo`: move chart `index` of `sheet` onto sheet `to` (same anchor cells), or onto
    /// a new sheet `new_sheet` placed before `sheet` where it fills the window as the reference's
    /// chart sheet does (gridlines off, anchored from A1). Returns the sheet and chart index.
    pub(super) fn move_chart_to(
        &mut self,
        sheet: usize,
        index: usize,
        to: Option<usize>,
        new_sheet: Option<&str>,
    ) -> Result<Value> {
        let wb = self.wb()?;
        if wb
            .sheets
            .get(sheet)
            .and_then(|s| s.charts.get(index))
            .is_none()
        {
            return Err(bad("no such chart"));
        }
        if new_sheet.is_some() {
            super::protect::check_structure(wb)?;
        }
        let name = match new_sheet {
            Some(n) => {
                let n = n.trim();
                if n.is_empty()
                    || n.chars().count() > 31
                    || n.contains(['[', ']', ':', '*', '?', '/', '\\'])
                {
                    return Err(bad(format!("invalid sheet name '{n}'")));
                }
                if wb.sheets.iter().any(|s| s.name.eq_ignore_ascii_case(n)) {
                    return Err(bad(format!("sheet '{n}' already exists")));
                }
                Some(n.to_owned())
            }
            None => None,
        };
        let target = match (&name, to) {
            (Some(_), _) => sheet,
            (None, Some(t)) if t < wb.sheets.len() => t,
            _ => return Err(bad("no such sheet")),
        };
        if name.is_none() && target == sheet {
            return Ok(json!({ "sheet": sheet, "index": index }));
        }
        let out = self.edit(sheet, Recalc::No, |wb, _| {
            let mut chart = wb.sheets[sheet].charts.remove(index);
            // The chart part is written again for its new drawing.
            chart.dirty = true;
            let dest = if let Some(n) = name {
                let mut ws = Worksheet::new(&n);
                ws.sheet_id = wb.sheets.iter().map(|s| s.sheet_id).max().unwrap_or(0) + 1;
                if ws.views.is_empty() {
                    ws.views.push(bo_model::sheet::SheetView::default());
                }
                for v in &mut ws.views {
                    v.show_grid_lines = Some(false);
                }
                // About one screen of the default grid (A1:O30), as a chart sheet fills the
                // window.
                chart.from = CellRef::new(0, 0);
                chart.to = Some(CellRef::new(30, 15));
                chart.anchor = bo_model::sheet::ChartAnchor::default();
                chart.pos = None;
                chart.ext = None;
                // Before the chart's sheet, keeping sheet-scoped names on their sheets.
                wb.sheets.push(ws);
                let last = wb.sheets.len() - 1;
                super::cmd::move_sheet(wb, last, sheet);
                sheet
            } else {
                target
            };
            wb.sheets[dest].charts.push(chart);
            let index = wb.sheets[dest].charts.len() - 1;
            Ok(json!({ "sheet": dest, "index": index }))
        })?;
        // A new sheet shifts the others: the formula engine and the geometry follow.
        let added = self
            .wb
            .as_deref()
            .is_some_and(|wb| wb.sheets.len() != self.geometry.len());
        if added {
            if let Some(wb) = self.wb.as_deref_mut() {
                self.live.rebuild(wb);
            }
            self.invalidate(None);
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::super::SheetEngine;
    use serde_json::{json, Value};

    fn run(e: &mut SheetEngine, cmd: &Value) -> Value {
        serde_json::from_str(&e.exec_json(&cmd.to_string()).unwrap()).unwrap()
    }

    /// Sales by quarter for two years: A1:C4 with names in row 1, quarters in column A.
    fn book() -> SheetEngine {
        let mut e = SheetEngine::new();
        e.new_workbook("Book").unwrap();
        let rows = [
            ["", "2025", "2026"],
            ["Q1", "5", "6"],
            ["Q2", "7", "8"],
            ["Q3", "4", "9"],
        ];
        for (r, row) in rows.iter().enumerate() {
            for (c, v) in row.iter().enumerate() {
                if !v.is_empty() {
                    e.set_input(0, r as u32, c as u32, v).unwrap();
                }
            }
        }
        // Years typed as text so they read as series names.
        e.set_input(0, 0, 1, "'2025").unwrap();
        e.set_input(0, 0, 2, "'2026").unwrap();
        let rng = json!({ "r0": 0, "c0": 0, "r1": 3, "c1": 2 });
        run(
            &mut e,
            &json!({ "cmd": "insertChart", "sheet": 0, "range": rng, "kind": "column" }),
        );
        e
    }

    fn names(e: &SheetEngine) -> Vec<String> {
        e.workbook()
            .map(|w| w.sheets.iter().map(|s| s.name.clone()).collect())
            .unwrap_or_default()
    }

    fn data(e: &mut SheetEngine) -> Value {
        run(e, &json!({ "cmd": "chartData", "sheet": 0, "index": 0 }))
    }

    #[test]
    fn chart_data_reads_the_block_and_switches_rows_and_columns() {
        let mut e = book();
        let d = data(&mut e);
        assert_eq!(d["range"], "Sheet1!$A$1:$C$4");
        assert_eq!(d["byRow"], false);
        assert_eq!(d["series"].as_array().unwrap().len(), 2);
        assert_eq!(d["series"][0]["name"], "2025");
        assert_eq!(d["series"][0]["valRef"], "Sheet1!$B$2:$B$4");
        assert_eq!(d["categories"], "Sheet1!$A$2:$A$4");
        run(
            &mut e,
            &json!({ "cmd": "switchRowColumn", "sheet": 0, "index": 0 }),
        );
        let d = data(&mut e);
        assert_eq!(d["byRow"], true);
        assert_eq!(d["range"], "Sheet1!$A$1:$C$4");
        let s = d["series"].as_array().unwrap();
        assert_eq!(s.len(), 3);
        assert_eq!(s[1]["name"], "Q2");
        assert_eq!(s[1]["valRef"], "Sheet1!$B$3:$C$3");
        assert_eq!(s[1]["catRef"], "Sheet1!$B$1:$C$1");
        // Switching back gives the first arrangement; undo too.
        run(
            &mut e,
            &json!({ "cmd": "switchRowColumn", "sheet": 0, "index": 0 }),
        );
        assert_eq!(data(&mut e)["series"].as_array().unwrap().len(), 2);
        e.undo().unwrap();
        assert_eq!(data(&mut e)["series"].as_array().unwrap().len(), 3);
        // Saved and opened again, the switched chart keeps its series.
        let bytes = e.save("xlsx", 0).unwrap();
        let mut back = SheetEngine::new();
        back.open(&bytes, "b.xlsx").unwrap();
        let d = data(&mut back);
        assert_eq!(d["byRow"], true);
        assert_eq!(d["series"][2]["valRef"], "Sheet1!$B$4:$C$4");
    }

    #[test]
    fn set_chart_data_range_and_series_list() {
        let mut e = book();
        // A smaller range: one series.
        run(
            &mut e,
            &json!({ "cmd": "setChartData", "sheet": 0, "index": 0, "range": "=Sheet1!$A$1:$B$4" }),
        );
        let d = data(&mut e);
        assert_eq!(d["series"].as_array().unwrap().len(), 1);
        assert_eq!(d["range"], "Sheet1!$A$1:$B$4");
        // Edit Series: an explicit list with a typed name and new categories.
        run(
            &mut e,
            &json!({ "cmd": "setChartData", "sheet": 0, "index": 0,
                "series": [
                    { "name": "Plan", "valRef": "=Sheet1!$C$2:$C$4" },
                    { "nameRef": "=Sheet1!$B$1", "valRef": "=Sheet1!$B$2:$B$4" }
                ],
                "categories": "=Sheet1!$A$2:$A$4" }),
        );
        let d = data(&mut e);
        assert_eq!(d["series"][0]["name"], "Plan");
        assert_eq!(d["series"][1]["name"], "2025");
        assert_eq!(d["series"][0]["catRef"], "Sheet1!$A$2:$A$4");
        // A reference that does not resolve is refused and nothing changes.
        let bad = json!({ "cmd": "setChartData", "sheet": 0, "index": 0,
            "series": [{ "valRef": "=Nope!$Z" }] });
        assert!(e.exec_json(&bad.to_string()).is_err());
        assert_eq!(data(&mut e)["series"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn move_chart_to_another_sheet_and_a_new_sheet() {
        let mut e = book();
        e.add_sheet(Some("Other")).unwrap();
        let r = run(
            &mut e,
            &json!({ "cmd": "moveChartTo", "sheet": 0, "index": 0, "to": 1 }),
        );
        assert_eq!(r, json!({ "sheet": 1, "index": 0 }));
        let on = |e: &mut SheetEngine, s: usize| {
            run(e, &json!({ "cmd": "charts", "sheet": s }))
                .as_array()
                .map_or(0, Vec::len)
        };
        assert_eq!((on(&mut e, 0), on(&mut e, 1)), (0, 1));
        // Its data still refers to the first sheet.
        let d = run(
            &mut e,
            &json!({ "cmd": "chartData", "sheet": 1, "index": 0 }),
        );
        assert_eq!(d["series"][0]["valRef"], "Sheet1!$B$2:$B$4");
        // To a new sheet placed before the chart's sheet, without gridlines.
        let r = run(
            &mut e,
            &json!({ "cmd": "moveChartTo", "sheet": 1, "index": 0, "newSheet": "Chart1" }),
        );
        assert_eq!(r, json!({ "sheet": 1, "index": 0 }));
        assert_eq!(names(&e), vec!["Sheet1", "Chart1", "Other"]);
        assert_eq!(on(&mut e, 1), 1);
        assert_eq!(on(&mut e, 2), 0);
        // A taken name is refused.
        let dup = json!({ "cmd": "moveChartTo", "sheet": 1, "index": 0, "newSheet": "other" });
        assert!(e.exec_json(&dup.to_string()).is_err());
        let bytes = e.save("xlsx", 0).unwrap();
        let mut back = SheetEngine::new();
        back.open(&bytes, "b.xlsx").unwrap();
        assert_eq!(on(&mut back, 1), 1);
        e.undo().unwrap();
        assert_eq!(names(&e), vec!["Sheet1", "Other"]);
    }
}
