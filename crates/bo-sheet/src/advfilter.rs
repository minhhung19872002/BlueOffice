//! Advanced filter: rows of a list are kept when they satisfy a *criteria range*,
//! either by hiding the other rows (filter in place) or by copying the matching
//! rows to another place on the sheet.
//!
//! The criteria range starts with a row of column labels; every following row is
//! one alternative (rows are joined with OR) whose non-empty cells must all hold
//! (AND). A criterion cell holds a comparison written as text: `>100`, `<=5`,
//! `<>x`, `=abc` (whole text, `*` / `?` wildcards, `~` escapes) or a plain value
//! (numbers compare equal, text matches cells that *begin* with it). A cell whose
//! label is blank or names no column of the list and that holds a formula is a
//! *formula criterion*: the formula is written for the first data row of the list
//! and is evaluated for every row with its relative references moved down; a
//! TRUE (non-zero) result keeps the row. These rules follow the publicly
//! documented behaviour of spreadsheet advanced filters; where a case is not
//! documented the choice made here is stated in the code.
//!
//! The results are recorded with the built-in defined names of ECMA-376 Part 1,
//! §18.2.5 `definedName`: `_xlnm._FilterDatabase` (the list), `_xlnm.Criteria`
//! and `_xlnm.Extract` (the copy destination), all scoped to the sheet; filtering
//! in place sets `sheetPr/@filterMode` (§18.3.1.82).

use crate::cells::{abs_ref, to_formula_value, ModelCells};
use crate::filter::wildcard_match;
use crate::numfmt::Locale;
use crate::render::format_cell;
use bo_formula::{parse, Expr, Value};
use bo_model::sheet::{Cell, CellRange, CellRef, CellValue, DefinedName, RowInfo, Workbook};
use std::collections::HashSet;

/// Built-in name of the filtered list (§18.2.5).
pub const FILTER_DATABASE: &str = "_xlnm._FilterDatabase";
/// Built-in name of the criteria range (§18.2.5).
pub const CRITERIA: &str = "_xlnm.Criteria";
/// Built-in name of the copy destination (§18.2.5).
pub const EXTRACT: &str = "_xlnm.Extract";

/// What an advanced filter runs on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdvancedFilter {
    /// The list, its label row first.
    pub list: CellRange,
    /// Sheet of the criteria range.
    pub criteria_sheet: usize,
    /// Criteria range, its label row first.
    pub criteria: CellRange,
    /// Copy the matching rows here (on the list's sheet) instead of hiding the
    /// others: a single cell (top-left of the output, unlimited rows) or a range
    /// (its rows limit the output). Labels in its first row pick and order the
    /// copied columns.
    pub copy_to: Option<CellRange>,
    /// Drop rows equal to an earlier kept row (on the output columns).
    pub unique: bool,
}

/// Result of [`apply`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    /// Matching data rows (after removing duplicates).
    pub matched: usize,
    /// Rows hidden (filter in place).
    pub hidden: usize,
    /// Cells written (copy): label row plus the matching rows.
    pub extract: Option<CellRange>,
}

/// Why an advanced filter could not run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdvError {
    /// The list has no label row or no data row, or a sheet index is wrong.
    BadList,
    /// A label of the copy destination names no column of the list.
    BadExtractLabel,
    /// The copy destination overlaps the list.
    ExtractOverlapsList,
}

impl std::fmt::Display for AdvError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            AdvError::BadList => "the list needs a label row and at least one data row",
            AdvError::BadExtractLabel => "a label of the copy destination is not in the list",
            AdvError::ExtractOverlapsList => "the copy destination overlaps the list",
        })
    }
}

/// Comparison of a criterion cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Op {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    /// No operator: numbers compare equal, text matches the start of the cell.
    Begins,
}

#[derive(Debug, Clone)]
enum Crit {
    /// Absolute list column, operator and operand text.
    Cond { col: u32, op: Op, operand: String },
    /// Formula written for the first data row (`origin`), evaluated on `sheet`.
    Formula {
        expr: Expr,
        origin: (u32, u32),
        sheet: usize,
    },
}

fn norm(s: &str) -> String {
    s.trim().to_lowercase()
}

/// Displayed text of a cell (empty for none).
fn shown(wb: &Workbook, cell: Option<&Cell>, locale: &Locale) -> String {
    let Some(c) = cell else {
        return String::new();
    };
    // Text under General (0) or `@` (49) shows as itself (§18.8.31 numFmt: the text
    // section of both is `@`): skips the formatter on a million-row list.
    if let CellValue::Text(t) = &c.value {
        if wb
            .styles
            .xf(c.style)
            .is_none_or(|x| x.num_fmt_id == 0 || x.num_fmt_id == 49)
        {
            return t.clone();
        }
    }
    format_cell(wb, c, locale).text
}

/// Split `>=10` into its operator and operand.
fn split_op(s: &str) -> (Op, &str) {
    for (p, op) in [
        (">=", Op::Ge),
        ("<=", Op::Le),
        ("<>", Op::Ne),
        (">", Op::Gt),
        ("<", Op::Lt),
        ("=", Op::Eq),
    ] {
        if let Some(rest) = s.strip_prefix(p) {
            return (op, rest);
        }
    }
    (Op::Begins, s)
}

/// Number an operand stands for: a plain number or a date typed in the locale.
fn operand_number(s: &str, locale: &Locale, date1904: bool) -> Option<f64> {
    let t = s.trim();
    if t.is_empty() {
        return None;
    }
    if let Ok(n) = t.parse::<f64>() {
        return Some(n);
    }
    if locale.decimal != '.' {
        if let Ok(n) = t.replace(locale.decimal, ".").parse::<f64>() {
            return Some(n);
        }
    }
    crate::numfmt::parse_typed_date(t, locale, date1904).map(|d| d.serial)
}

/// Text of a criterion cell's value (its cached result for a formula).
fn criterion_text(v: &CellValue) -> String {
    match v {
        CellValue::Number(n) => n.to_string(),
        CellValue::Bool(b) => if *b { "TRUE" } else { "FALSE" }.to_owned(),
        other => other.as_text().unwrap_or_default(),
    }
}

/// Does a data cell satisfy one comparison?
fn cond_match(
    op: Op,
    operand: &str,
    value: &CellValue,
    text: &str,
    locale: &Locale,
    date1904: bool,
) -> bool {
    let blank = text.is_empty() && matches!(value, CellValue::Empty | CellValue::Text(_));
    if operand.is_empty() {
        // `=` alone: blank cells; `<>` alone: non-blank cells (own choice for the
        // other operators: nothing matches).
        return match op {
            Op::Eq => blank,
            Op::Ne => !blank,
            Op::Begins => true,
            _ => false,
        };
    }
    let num = match value {
        CellValue::Number(n) => Some(*n),
        _ => None,
    };
    if let (Some(n), Some(x)) = (num, operand_number(operand, locale, date1904)) {
        return match op {
            Op::Eq | Op::Begins => (n - x).abs() < 1e-9,
            Op::Ne => (n - x).abs() >= 1e-9,
            Op::Lt => n < x,
            Op::Le => n <= x,
            Op::Gt => n > x,
            Op::Ge => n >= x,
        };
    }
    match op {
        Op::Begins => !blank && wildcard_match(&format!("{operand}*"), text),
        Op::Eq => wildcard_match(operand, text),
        Op::Ne => !wildcard_match(operand, text),
        // Ordering comparisons on text only compare text with text.
        _ => {
            let is_text = matches!(value, CellValue::Text(_) | CellValue::RichText(_));
            if !is_text || operand_number(operand, locale, date1904).is_some() {
                return false;
            }
            let (a, b) = (text.to_lowercase(), operand.to_lowercase());
            match op {
                Op::Lt => a < b,
                Op::Le => a <= b,
                Op::Gt => a > b,
                _ => a >= b,
            }
        }
    }
}

fn truthy(v: &Value) -> bool {
    match v.first() {
        Value::Bool(b) => *b,
        Value::Number(n) => *n != 0.0,
        _ => false,
    }
}

/// Criteria rows: each is a list of criteria joined with AND; `None` for a row
/// without any criterion (it matches every row).
fn read_criteria(
    wb: &Workbook,
    spec: &AdvancedFilter,
    headers: &[String],
    locale: &Locale,
) -> Vec<Option<Vec<Crit>>> {
    let Some(cs) = wb.sheets.get(spec.criteria_sheet) else {
        return Vec::new();
    };
    let c = spec.criteria;
    let labels: Vec<String> = (c.first.col..=c.last.col)
        .map(|col| norm(&shown(wb, cs.cell(c.first.row, col), locale)))
        .collect();
    let data_row = spec.list.first.row + 1;
    let mut rows = Vec::new();
    for r in c.first.row + 1..=c.last.row {
        let mut crits = Vec::new();
        for (i, col) in (c.first.col..=c.last.col).enumerate() {
            let Some(cell) = cs.cell(r, col) else {
                continue;
            };
            let target = headers
                .iter()
                .position(|h| !labels[i].is_empty() && *h == labels[i]);
            match (target, &cell.formula) {
                (None, Some(f)) if !f.text.is_empty() => {
                    if let Ok(expr) = parse(&f.text) {
                        crits.push(Crit::Formula {
                            expr,
                            origin: (data_row, col),
                            sheet: spec.criteria_sheet,
                        });
                    }
                }
                (Some(t), _) => {
                    let s = criterion_text(&cell.value);
                    if s.is_empty() {
                        continue;
                    }
                    let (op, operand) = split_op(&s);
                    crits.push(Crit::Cond {
                        col: spec.list.first.col + t as u32,
                        op,
                        operand: operand.to_owned(),
                    });
                }
                // Own choice: a constant under a label that is not in the list is ignored.
                _ => {}
            }
        }
        rows.push((!crits.is_empty()).then_some(crits));
    }
    rows
}

/// Labels of the list (normalised), left to right.
fn list_headers(wb: &Workbook, sheet: usize, list: CellRange, locale: &Locale) -> Vec<String> {
    let Some(ws) = wb.sheets.get(sheet) else {
        return Vec::new();
    };
    (list.first.col..=list.last.col)
        .map(|c| norm(&shown(wb, ws.cell(list.first.row, c), locale)))
        .collect()
}

/// Output columns of a copy (absolute list columns), from the destination labels.
fn output_columns(
    wb: &Workbook,
    sheet: usize,
    spec: &AdvancedFilter,
    headers: &[String],
    locale: &Locale,
) -> Result<Vec<u32>, AdvError> {
    let all: Vec<u32> = (spec.list.first.col..=spec.list.last.col).collect();
    let Some(dest) = spec.copy_to else {
        return Ok(all);
    };
    let ws = wb.sheets.get(sheet).ok_or(AdvError::BadList)?;
    let labels: Vec<String> = (dest.first.col..=dest.last.col)
        .map(|c| norm(&shown(wb, ws.cell(dest.first.row, c), locale)))
        .collect();
    if labels.iter().all(String::is_empty) {
        return Ok(all);
    }
    labels
        .iter()
        .map(|l| {
            headers
                .iter()
                .position(|h| !l.is_empty() && h == l)
                .map(|i| spec.list.first.col + i as u32)
                .ok_or(AdvError::BadExtractLabel)
        })
        .collect()
}

/// Data rows (absolute) of the list that satisfy the criteria, in order, with
/// duplicates (on `columns`) removed when `spec.unique`.
fn matching(
    wb: &Workbook,
    sheet: usize,
    spec: &AdvancedFilter,
    columns: &[u32],
    locale: &Locale,
    now: Option<f64>,
) -> Vec<u32> {
    let Some(ws) = wb.sheets.get(sheet) else {
        return Vec::new();
    };
    let headers = list_headers(wb, sheet, spec.list, locale);
    let criteria = read_criteria(wb, spec, &headers, locale);
    let any_row = criteria.is_empty() || criteria.iter().any(Option::is_none);
    let m = ModelCells::new(wb, now);
    let mut seen: HashSet<String> = HashSet::new();
    let mut out = Vec::new();
    for r in spec.list.first.row + 1..=spec.list.last.row {
        let ok = any_row
            || criteria.iter().flatten().any(|crits| {
                crits.iter().all(|c| match c {
                    Crit::Cond { col, op, operand } => {
                        let cell = ws.cell(r, *col);
                        let value = cell.map_or(CellValue::Empty, |c| c.value.clone());
                        let text = shown(wb, cell, locale);
                        cond_match(*op, operand, &value, &text, locale, wb.date1904)
                    }
                    Crit::Formula {
                        expr,
                        origin,
                        sheet,
                    } => truthy(&m.eval_expr(*sheet, expr, *origin, r, origin.1)),
                })
            });
        if !ok {
            continue;
        }
        if spec.unique {
            let key: Vec<String> = columns
                .iter()
                .map(|c| {
                    let cell = ws.cell(r, *c);
                    match cell.map(|c| to_formula_value(&c.value)) {
                        Some(Value::Number(n)) => format!("n{n}"),
                        _ => format!("t{}", shown(wb, cell, locale).to_lowercase()),
                    }
                })
                .collect();
            if !seen.insert(key.join("\u{1f}")) {
                continue;
            }
        }
        out.push(r);
    }
    out
}

/// Data rows (absolute) an advanced filter keeps.
pub fn matching_rows(
    wb: &Workbook,
    sheet: usize,
    spec: &AdvancedFilter,
    locale: &Locale,
    now: Option<f64>,
) -> Result<Vec<u32>, AdvError> {
    if spec.list.rows() < 2 || sheet >= wb.sheets.len() {
        return Err(AdvError::BadList);
    }
    let headers = list_headers(wb, sheet, spec.list, locale);
    let columns = output_columns(wb, sheet, spec, &headers, locale)?;
    Ok(matching(wb, sheet, spec, &columns, locale, now))
}

fn overlaps(a: CellRange, b: CellRange) -> bool {
    a.first.row <= b.last.row
        && b.first.row <= a.last.row
        && a.first.col <= b.last.col
        && b.first.col <= a.last.col
}

/// Set (or replace) a sheet-scoped built-in name.
fn set_name(wb: &mut Workbook, sheet: usize, name: &str, range: CellRange, hidden: bool) {
    let sheet_name = wb.sheets[sheet].name.clone();
    let formula = abs_ref(&sheet_name, range);
    let local = Some(sheet as u32);
    if let Some(d) = wb
        .defined_names
        .iter_mut()
        .find(|d| d.local_sheet == local && d.name.eq_ignore_ascii_case(name))
    {
        d.formula = formula;
        return;
    }
    wb.defined_names.push(DefinedName {
        name: name.to_owned(),
        formula,
        local_sheet: local,
        hidden,
        attrs: Vec::new(),
    });
}

/// Range of a sheet-scoped name on that sheet.
fn named_range(wb: &Workbook, sheet: usize, name: &str) -> Option<CellRange> {
    let d = wb
        .defined_names
        .iter()
        .find(|d| d.local_sheet == Some(sheet as u32) && d.name.eq_ignore_ascii_case(name))?;
    let body = d.formula.rsplit('!').next()?;
    CellRange::parse(&body.replace('$', ""))
}

fn set_filter_mode(wb: &mut Workbook, sheet: usize, on: bool) {
    let props = &mut wb.sheets[sheet].props;
    props.attrs.retain(|(k, _)| k != "filterMode");
    if on {
        props.present = true;
        props.attrs.push(("filterMode".into(), "1".into()));
    }
}

/// Run an advanced filter on sheet `sheet`: hide the rows that do not match
/// (filter in place; an AutoFilter overlapping the list is removed, own choice)
/// or copy the matching rows to `spec.copy_to` (values and formats; formulas are
/// copied as their results, own choice). Records the built-in names.
pub fn apply(
    wb: &mut Workbook,
    sheet: usize,
    spec: &AdvancedFilter,
    locale: &Locale,
    now: Option<f64>,
) -> Result<Outcome, AdvError> {
    if spec.list.rows() < 2 || sheet >= wb.sheets.len() || spec.criteria_sheet >= wb.sheets.len() {
        return Err(AdvError::BadList);
    }
    if spec.copy_to.is_some_and(|d| spec.list.contains(d.first)) {
        return Err(AdvError::ExtractOverlapsList);
    }
    let headers = list_headers(wb, sheet, spec.list, locale);
    let columns = output_columns(wb, sheet, spec, &headers, locale)?;
    let rows = matching(wb, sheet, spec, &columns, locale, now);
    set_name(wb, sheet, FILTER_DATABASE, spec.list, true);
    set_name(wb, spec.criteria_sheet, CRITERIA, spec.criteria, false);
    let Some(dest) = spec.copy_to else {
        let ws = &mut wb.sheets[sheet];
        if ws
            .auto_filter
            .as_ref()
            .is_some_and(|af| overlaps(af.range, spec.list))
        {
            ws.auto_filter = None;
        }
        let keep: HashSet<u32> = rows.iter().copied().collect();
        let mut hidden = 0;
        for r in spec.list.first.row + 1..=spec.list.last.row {
            let hide = !keep.contains(&r);
            hidden += usize::from(hide);
            let info = ws.rows.entry(r).or_default();
            info.hidden = hide;
            if *info == RowInfo::default() {
                ws.rows.remove(&r);
            }
        }
        set_filter_mode(wb, sheet, hidden > 0);
        return Ok(Outcome {
            matched: rows.len(),
            hidden,
            extract: None,
        });
    };
    let width = columns.len() as u32;
    let limit = (dest.rows() > 1).then(|| dest.rows() - 1);
    let n = limit.map_or(rows.len(), |l| rows.len().min(l as usize));
    let out = CellRange::new(
        dest.first,
        CellRef::new(dest.first.row + n as u32, dest.first.col + width - 1),
    );
    if overlaps(out, spec.list) {
        return Err(AdvError::ExtractOverlapsList);
    }
    // Clear the previous output and the destination.
    let old = named_range(wb, sheet, EXTRACT);
    let ws = &mut wb.sheets[sheet];
    let mut clear = |r: CellRange| {
        let keys: Vec<(u32, u32)> = ws
            .cells
            .keys()
            .filter(|(row, col)| r.contains(CellRef::new(*row, *col)))
            .copied()
            .collect();
        for k in keys {
            ws.cells.remove(&k);
        }
    };
    if let Some(o) = old.filter(|o| !overlaps(*o, spec.list)) {
        clear(o);
    }
    if limit.is_some() {
        clear(CellRange::new(
            CellRef::new(dest.first.row, dest.first.col),
            CellRef::new(dest.last.row, dest.first.col + width - 1),
        ));
    }
    let copy = |ws: &bo_model::sheet::Worksheet, r: u32, c: u32| -> Option<Cell> {
        ws.cell(r, c).map(|cell| {
            let mut cell = cell.clone();
            cell.formula = None;
            cell
        })
    };
    let mut writes: Vec<((u32, u32), Option<Cell>)> = Vec::new();
    for (i, &c) in columns.iter().enumerate() {
        let to_col = dest.first.col + i as u32;
        writes.push(((dest.first.row, to_col), copy(ws, spec.list.first.row, c)));
        for (j, &r) in rows.iter().take(n).enumerate() {
            writes.push(((dest.first.row + 1 + j as u32, to_col), copy(ws, r, c)));
        }
    }
    for (k, cell) in writes {
        match cell {
            Some(cell) => {
                ws.cells.insert(k, cell);
            }
            None => {
                ws.cells.remove(&k);
            }
        }
    }
    set_name(wb, sheet, EXTRACT, out, false);
    Ok(Outcome {
        matched: rows.len(),
        hidden: 0,
        extract: Some(out),
    })
}

/// Show every row hidden by a filter on sheet `sheet` (advanced filter in place
/// or AutoFilter; AutoFilter criteria are cleared). Returns the rows shown.
pub fn show_all(wb: &mut Workbook, sheet: usize) -> usize {
    if sheet >= wb.sheets.len() {
        return 0;
    }
    let mut ranges = Vec::new();
    if let Some(r) = named_range(wb, sheet, FILTER_DATABASE) {
        ranges.push(r);
    }
    let ws = &mut wb.sheets[sheet];
    if let Some(af) = ws.auto_filter.as_mut() {
        af.columns.clear();
        ranges.push(af.range);
    }
    let mut shown = 0;
    for r in ranges {
        for row in r.first.row + 1..=r.last.row {
            if let Some(info) = ws.rows.get_mut(&row) {
                if info.hidden {
                    info.hidden = false;
                    shown += 1;
                }
                if *info == RowInfo::default() {
                    ws.rows.remove(&row);
                }
            }
        }
    }
    set_filter_mode(wb, sheet, false);
    shown
}

#[cfg(test)]
#[path = "advfilter_tests.rs"]
mod tests;
