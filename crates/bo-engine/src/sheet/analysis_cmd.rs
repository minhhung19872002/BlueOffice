//! The `analysis` command: runs a BlueSheet analysis tool (`bo_sheet::analysis`) on the values
//! of its input ranges and writes the output table at a cell or on a new sheet (one undo step).

use super::cmd::{At, Recalc, Rng};
use super::{SheetEngine, MAX_COLS, MAX_ROWS};
use crate::{EngineError, Result};
use bo_model::sheet::{CellError, CellRange, CellRef, CellValue, Workbook, Worksheet};
use bo_sheet::analysis::{self, Grid, Item, Out, OutError, Tool};
use bo_sheet::style::StylePatch;
use serde_json::{json, Value};

fn bad(msg: impl Into<String>) -> EngineError {
    EngineError::Sheet(msg.into())
}

/// The values of `range` on sheet `si` as analysis input.
fn grid(wb: &Workbook, si: usize, range: CellRange) -> Grid {
    let ws = &wb.sheets[si];
    (range.first.row..=range.last.row)
        .map(|r| {
            (range.first.col..=range.last.col)
                .map(|c| match ws.cells.get(&(r, c)).map(|cell| &cell.value) {
                    None | Some(CellValue::Empty) => Item::Empty,
                    Some(CellValue::Number(n)) => Item::Num(*n),
                    Some(v) => Item::Text(v.as_text().unwrap_or_default()),
                })
                .collect()
        })
        .collect()
}

fn overlaps(a: CellRange, b: CellRange) -> bool {
    a.first.row <= b.last.row
        && b.first.row <= a.last.row
        && a.first.col <= b.last.col
        && b.first.col <= a.last.col
}

/// A sheet name not yet used: `base`, else `base (2)`, `base (3)`, …
pub(super) fn free_name(wb: &Workbook, base: &str) -> String {
    let taken = |n: &str| wb.sheets.iter().any(|s| s.name.eq_ignore_ascii_case(n));
    if !taken(base) {
        return base.to_owned();
    }
    // At most n + 1 candidates can be taken by the n sheets, so this always finds one.
    (2..=wb.sheets.len() + 2)
        .map(|i| format!("{base} ({i})"))
        .find(|n| !taken(n))
        .unwrap_or_else(|| base.to_owned())
}

impl SheetEngine {
    /// Runs `tool` on `input` / `input2` (A1 references, optionally sheet-qualified, relative to
    /// `sheet`) and writes the result at `dest`, or on a new sheet named `sheet_name`.
    /// Returns `{ sheet, range }` of the written table.
    pub(super) fn analysis(
        &mut self,
        sheet: usize,
        tool: &Tool,
        input: Option<&str>,
        input2: Option<&str>,
        dest: Option<At>,
        sheet_name: Option<&str>,
    ) -> Result<Value> {
        let vi = self.locale == bo_sheet::numfmt::Locale::VI_VN;
        self.sheet(sheet)?;
        let wb = self.wb()?;
        let read = |text: Option<&str>| -> Result<Option<(usize, CellRange, Grid)>> {
            match text.map(str::trim).filter(|t| !t.is_empty()) {
                None => Ok(None),
                Some(t) => {
                    let (si, r) = bo_sheet::cells::resolve_ref(wb, sheet, &t.replace('$', ""))
                        .ok_or_else(|| bad("analysis: badRange"))?;
                    if u64::from(r.last.row - r.first.row + 1)
                        * u64::from(r.last.col - r.first.col + 1)
                        > analysis::MAX_OUTPUT_CELLS as u64
                    {
                        return Err(bad("analysis: tooLarge"));
                    }
                    Ok(Some((si, r, grid(wb, si, r))))
                }
            }
        };
        let first = read(input)?;
        if first.is_none() && tool.uses_input() {
            return Err(bad("analysis: noInput"));
        }
        let second = if tool.uses_second_input() {
            read(input2)?
        } else {
            None
        };
        // The output may not overwrite its own input (checked again for the whole table below).
        if let Some(at) = dest {
            let cell = CellRange::new(CellRef::new(at.row, at.col), CellRef::new(at.row, at.col));
            for (si, r, _) in first.iter().chain(second.iter()) {
                if *si == at.sheet && overlaps(*r, cell) {
                    return Err(bad("analysis: overlap"));
                }
            }
        }
        let empty = Grid::new();
        let table = analysis::run(
            tool,
            first.as_ref().map_or(&empty, |f| &f.2),
            second.as_ref().map(|s| &s.2),
            vi,
        )
        .map_err(|e| bad(format!("analysis: {}", e.code())))?;
        let (h, w) = (table.rows.len() as u32, table.width() as u32);
        if h == 0 || w == 0 {
            return Err(bad("analysis: tooFew"));
        }
        if let Some(at) = dest {
            if at.sheet >= wb.sheets.len() {
                return Err(bad("no such sheet"));
            }
            if at.row.saturating_add(h) > MAX_ROWS || at.col.saturating_add(w) > MAX_COLS {
                return Err(bad("analysis: tooLarge"));
            }
            let out = CellRange::new(
                CellRef::new(at.row, at.col),
                CellRef::new(at.row + h - 1, at.col + w - 1),
            );
            for (si, r, _) in first.iter().chain(second.iter()) {
                if *si == at.sheet && overlaps(*r, out) {
                    return Err(bad("analysis: overlap"));
                }
            }
        }
        let new_sheet = dest.is_none();
        let name = sheet_name
            .map(str::trim)
            .filter(|n| !n.is_empty())
            .map(str::to_owned);
        // ECMA-376 §18.2.19 sheet names: at most 31 characters, none of []:*?/\.
        if let Some(n) = &name {
            if n.chars().count() > 31 || n.contains(['[', ']', ':', '*', '?', '/', '\\']) {
                return Err(bad("analysis: badName"));
            }
        }
        let out = self.edit(sheet, Recalc::All, |wb, _| {
            let (ds, at) = if let Some(a) = dest {
                (a.sheet, CellRef::new(a.row, a.col))
            } else {
                {
                    super::protect::check_structure(wb)?;
                    let base = name
                        .unwrap_or_else(|| (if vi { "Phân tích" } else { "Analysis" }).to_owned());
                    let mut ws = Worksheet::new(&free_name(wb, &base));
                    ws.sheet_id = wb.sheets.iter().map(|s| s.sheet_id).max().unwrap_or(0) + 1;
                    wb.sheets.push(ws);
                    (wb.sheets.len() - 1, CellRef::new(0, 0))
                }
            };
            let range = CellRange::new(at, CellRef::new(at.row + h - 1, at.col + w - 1));
            let ws = &mut wb.sheets[ds];
            let mut percent: Vec<(u32, u32)> = Vec::new();
            for (i, row) in (0u32..).zip(&table.rows) {
                for j in 0..w {
                    let pos = (at.row + i, at.col + j);
                    let value = match row.get(j as usize).unwrap_or(&Out::Empty) {
                        Out::Empty => CellValue::Empty,
                        Out::Text(t) => CellValue::Text(t.clone()),
                        Out::Num(n) => CellValue::Number(*n),
                        Out::Percent(n) => {
                            percent.push(pos);
                            CellValue::Number(*n)
                        }
                        Out::Error(e) => CellValue::Error(match e {
                            OutError::NA => CellError::NA,
                            OutError::Div0 => CellError::Div0,
                            OutError::Num => CellError::Num,
                        }),
                    };
                    if matches!(value, CellValue::Empty) {
                        if let Some(c) = ws.cells.get_mut(&pos) {
                            c.formula = None;
                            c.value = CellValue::Empty;
                        }
                        continue;
                    }
                    let cell = ws.cells.entry(pos).or_default();
                    cell.formula = None;
                    cell.value = value;
                }
            }
            let patch = StylePatch {
                num_fmt: Some("0.00%".to_owned()),
                ..StylePatch::default()
            };
            for (r, c) in percent {
                let one = CellRange::new(CellRef::new(r, c), CellRef::new(r, c));
                bo_sheet::style::apply_patch(wb, ds, one, &patch);
            }
            Ok(json!({ "sheet": ds, "range": Rng::of(range) }))
        })?;
        if new_sheet {
            // A new sheet needs its own geometry slot.
            self.invalidate(None);
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn engine() -> SheetEngine {
        let mut e = SheetEngine::with_bundled_fonts();
        e.new_workbook("Book1.xlsx").unwrap();
        e
    }

    #[allow(clippy::needless_pass_by_value)]
    fn run(e: &mut SheetEngine, v: Value) -> std::result::Result<Value, String> {
        e.exec_json(&v.to_string())
            .map(|s| serde_json::from_str(&s).unwrap())
            .map_err(|err| err.to_string())
    }

    fn value(e: &SheetEngine, sheet: usize, row: u32, col: u32) -> CellValue {
        e.wb().unwrap().sheets[sheet]
            .cells
            .get(&(row, col))
            .map(|c| c.value.clone())
            .unwrap_or_default()
    }

    #[test]
    fn writes_at_cell_and_undoes() {
        let mut e = engine();
        // hand: A2:A6 = 1..5 under a header — mean 3, sample variance 2.5.
        e.set_input(0, 0, 0, "Data").unwrap();
        for i in 1..=5u32 {
            e.set_input(0, i, 0, &i.to_string()).unwrap();
        }
        let out = run(
            &mut e,
            json!({
                "cmd": "analysis", "sheet": 0, "input": "$A$1:$A$6",
                "tool": { "tool": "descriptive", "labels": true },
                "dest": { "sheet": 0, "row": 0, "col": 2 },
            }),
        )
        .unwrap();
        assert_eq!(out["sheet"], 0);
        assert_eq!(out["range"]["c0"], 2);
        assert_eq!(out["range"]["c1"], 3);
        assert_eq!(value(&e, 0, 0, 2), CellValue::Text("Data".into()));
        // The default locale is vi-VN.
        assert_eq!(value(&e, 0, 2, 2), CellValue::Text("Trung bình".into()));
        assert_eq!(value(&e, 0, 2, 3), CellValue::Number(3.0));
        assert_eq!(value(&e, 0, 7, 3), CellValue::Number(2.5));
        assert!(e.undo().unwrap());
        assert_eq!(value(&e, 0, 2, 3), CellValue::Empty);
    }

    #[test]
    fn new_sheet_errors_and_percent_format() {
        let mut e = engine();
        for (i, v) in [3, 1, 4, 1, 5].iter().enumerate() {
            e.set_input(0, i as u32, 0, &v.to_string()).unwrap();
        }
        let out = run(
            &mut e,
            json!({
                "cmd": "analysis", "sheet": 0, "input": "Sheet1!A1:A5",
                "tool": { "tool": "rankPercentile" }, "sheetName": "Ranks",
            }),
        )
        .unwrap();
        assert_eq!(out["sheet"], 1);
        let wb = e.wb().unwrap();
        assert_eq!(wb.sheets[1].name, "Ranks");
        // hand: the largest value 5 is point 5, rank 1, percent 100% (built-in format 10, 0.00%).
        assert_eq!(value(&e, 1, 1, 0), CellValue::Number(5.0));
        assert_eq!(value(&e, 1, 1, 3), CellValue::Number(1.0));
        let cell = &wb.sheets[1].cells[&(1, 3)];
        assert_eq!(wb.styles.xf(cell.style).map(|x| x.num_fmt_id), Some(10));
        // Output over the input is refused; a tool error reports its code.
        let err = run(
            &mut e,
            json!({
                "cmd": "analysis", "sheet": 0, "input": "A1:A5",
                "tool": { "tool": "fourier" }, "dest": { "sheet": 0, "row": 2, "col": 0 },
            }),
        )
        .unwrap_err();
        assert!(err.contains("analysis: overlap"), "{err}");
        let err = run(
            &mut e,
            json!({
                "cmd": "analysis", "sheet": 0, "input": "A1:A5",
                "tool": { "tool": "rankPercentile" }, "sheetName": "a/b",
            }),
        )
        .unwrap_err();
        assert!(err.contains("analysis: badName"), "{err}");
        let err = run(
            &mut e,
            json!({
                "cmd": "analysis", "sheet": 0, "input": "A1:A5",
                "tool": { "tool": "fourier" }, "dest": { "sheet": 0, "row": 0, "col": 4 },
            }),
        )
        .unwrap_err();
        assert!(err.contains("analysis: powerOfTwo"), "{err}");
        // Random numbers need no input.
        let out = run(
            &mut e,
            json!({
                "cmd": "analysis", "sheet": 0,
                "tool": { "tool": "random", "columns": 2, "count": 3, "seed": 9,
                          "dist": { "kind": "uniform", "lo": 0, "hi": 1 } },
                "dest": { "sheet": 0, "row": 0, "col": 6 },
            }),
        )
        .unwrap();
        assert_eq!(out["range"]["r1"], 2);
        assert_eq!(out["range"]["c1"], 7);
        assert!(matches!(value(&e, 0, 2, 7), CellValue::Number(x) if (0.0..1.0).contains(&x)));
    }
}
