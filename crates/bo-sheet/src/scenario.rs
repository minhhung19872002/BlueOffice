//! What-if scenarios: named sets of values for a group of input ("changing") cells of a
//! worksheet (ECMA-376 Part 1, §18.3.1 `scenarios` / `scenario` / `inputCells`).
//!
//! * [`show`] enters a scenario's values into its changing cells;
//! * [`merge`] copies scenarios from another sheet, renaming clashes;
//! * [`summary`] recalculates chosen result cells with each scenario applied (on the formula
//!   engine's copy of the workbook, so the sheet is untouched) and [`summary_sheet`] lays the
//!   numbers out on a new worksheet.

use bo_model::sheet::{
    Cell, CellRange, CellRef, CellValue, Scenario, ScenarioCell, Scenarios, Workbook, Worksheet,
};
use serde::{Deserialize, Serialize};

use crate::formula::{load, to_cell_value, to_value};

/// Most changing cells one scenario may have.
pub const MAX_CHANGING_CELLS: usize = 32;
/// Longest scenario name (characters).
pub const MAX_NAME: usize = 255;

/// Why a scenario was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScenarioError {
    /// The name is empty or too long.
    BadName,
    /// Another scenario of the sheet has this name.
    DuplicateName,
    /// No changing cell, or more than [`MAX_CHANGING_CELLS`].
    BadCells,
    /// The number of values differs from the number of changing cells.
    ValueCount,
    /// No scenario at that index.
    NotFound,
}

impl ScenarioError {
    /// Message for the host.
    #[must_use]
    pub fn message(self) -> &'static str {
        match self {
            ScenarioError::BadName => "scenario name is empty or too long",
            ScenarioError::DuplicateName => "a scenario with this name already exists",
            ScenarioError::BadCells => "a scenario needs 1 to 32 changing cells",
            ScenarioError::ValueCount => "one value is needed per changing cell",
            ScenarioError::NotFound => "no such scenario",
        }
    }
}

/// The text a cell value is stored as in `inputCells/@val`.
#[must_use]
pub fn value_text(v: &CellValue) -> String {
    match v {
        CellValue::Empty => String::new(),
        CellValue::Number(n) => format!("{n}"),
        CellValue::Bool(b) => if *b { "TRUE" } else { "FALSE" }.to_owned(),
        CellValue::Error(e) => e.as_str().to_owned(),
        CellValue::Date(s) => s.clone(),
        CellValue::Text(_) | CellValue::RichText(_) => v.as_text().unwrap_or_default(),
    }
}

/// The cell value a stored `val` stands for: a number, `TRUE` / `FALSE`, empty, else text.
#[must_use]
pub fn parse_value(s: &str) -> CellValue {
    let t = s.trim();
    if t.is_empty() {
        return CellValue::Empty;
    }
    if let Ok(n) = t.parse::<f64>() {
        if n.is_finite() {
            return CellValue::Number(n);
        }
    }
    if t.eq_ignore_ascii_case("true") {
        return CellValue::Bool(true);
    }
    if t.eq_ignore_ascii_case("false") {
        return CellValue::Bool(false);
    }
    CellValue::Text(s.to_owned())
}

/// The current value of a cell as scenario text.
#[must_use]
pub fn current_text(ws: &Worksheet, c: CellRef) -> String {
    ws.cell(c.row, c.col)
        .map(|x| value_text(&x.value))
        .unwrap_or_default()
}

/// Expand ranges to their cells (row by row), without duplicates.
#[must_use]
pub fn cells_of(ranges: &[CellRange]) -> Vec<CellRef> {
    let mut out: Vec<CellRef> = Vec::new();
    for r in ranges {
        for row in r.first.row..=r.last.row {
            for col in r.first.col..=r.last.col {
                let c = CellRef::new(row, col);
                if !out.contains(&c) {
                    out.push(c);
                    if out.len() > MAX_CHANGING_CELLS {
                        return out;
                    }
                }
            }
        }
    }
    out
}

/// Build a scenario from its parts, checking the name and the cells.
///
/// # Errors
/// See [`ScenarioError`].
pub fn build(
    name: &str,
    cells: &[CellRef],
    values: &[String],
    comment: Option<String>,
    user: Option<String>,
    locked: bool,
    hidden: bool,
) -> Result<Scenario, ScenarioError> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > MAX_NAME {
        return Err(ScenarioError::BadName);
    }
    if cells.is_empty() || cells.len() > MAX_CHANGING_CELLS {
        return Err(ScenarioError::BadCells);
    }
    if values.len() != cells.len() {
        return Err(ScenarioError::ValueCount);
    }
    Ok(Scenario {
        name: name.to_owned(),
        locked,
        hidden,
        user: user.filter(|u| !u.trim().is_empty()),
        comment: comment.filter(|c| !c.is_empty()),
        cells: cells
            .iter()
            .zip(values)
            .map(|(c, v)| ScenarioCell {
                cell: *c,
                value: v.clone(),
                ..ScenarioCell::default()
            })
            .collect(),
    })
}

/// Add a scenario (`index: None`) or replace the one at `index`; returns its index.
///
/// # Errors
/// [`ScenarioError::DuplicateName`] when another scenario has the name (case-insensitive),
/// [`ScenarioError::NotFound`] for a bad index.
pub fn save(
    list: &mut Scenarios,
    index: Option<usize>,
    sc: Scenario,
) -> Result<usize, ScenarioError> {
    if index.is_some_and(|i| i >= list.items.len()) {
        return Err(ScenarioError::NotFound);
    }
    let clash = list
        .items
        .iter()
        .enumerate()
        .any(|(i, s)| Some(i) != index && same_name(&s.name, &sc.name));
    if clash {
        return Err(ScenarioError::DuplicateName);
    }
    Ok(if let Some(i) = index {
        list.items[i] = sc;
        i
    } else {
        list.items.push(sc);
        list.items.len() - 1
    })
}

/// Delete the scenario at `index`, keeping `current` / `show` on the same scenarios.
///
/// # Errors
/// [`ScenarioError::NotFound`] for a bad index.
pub fn delete(list: &mut Scenarios, index: usize) -> Result<(), ScenarioError> {
    if index >= list.items.len() {
        return Err(ScenarioError::NotFound);
    }
    list.items.remove(index);
    let fix = |v: Option<u32>| -> Option<u32> {
        let v = v?;
        let i = usize::try_from(v).ok()?;
        match i.cmp(&index) {
            std::cmp::Ordering::Less => Some(v),
            std::cmp::Ordering::Equal => None,
            std::cmp::Ordering::Greater => Some(v - 1),
        }
    };
    list.current = fix(list.current);
    list.show = fix(list.show);
    Ok(())
}

fn same_name(a: &str, b: &str) -> bool {
    a.trim().to_lowercase() == b.trim().to_lowercase()
}

/// A name not used by `list`: `name`, else `name (2)`, `name (3)`, …
fn free_name(list: &Scenarios, name: &str) -> String {
    if !list.items.iter().any(|s| same_name(&s.name, name)) {
        return name.to_owned();
    }
    let mut n = 2;
    loop {
        let cand = format!("{name} ({n})");
        if !list.items.iter().any(|s| same_name(&s.name, &cand)) {
            return cand;
        }
        n += 1;
    }
}

/// Copy every scenario of `from` into `into`; a clashing name gets a ` (n)` suffix.
/// Returns how many were copied.
pub fn merge(into: &mut Scenarios, from: &[Scenario]) -> usize {
    for sc in from {
        let mut copy = sc.clone();
        copy.name = free_name(into, &sc.name);
        into.items.push(copy);
    }
    from.len()
}

/// Enter the scenario's values into its changing cells (formulas there are replaced by the
/// constants). Cells marked `deleted` are skipped.
pub fn show(ws: &mut Worksheet, sc: &Scenario) {
    for c in sc.cells.iter().filter(|c| !c.deleted) {
        let cell = ws.cells.entry((c.cell.row, c.cell.col)).or_default();
        cell.formula = None;
        cell.value = parse_value(&c.value);
    }
}

/// Values for the summary report.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    /// The changing cells of all scenarios, in order of first use.
    pub changing: Vec<CellRef>,
    /// The result cells.
    pub results: Vec<CellRef>,
    /// Current values: changing cells, then result cells.
    pub current: Vec<CellValue>,
    /// Per scenario: its name and the values of the changing cells, then the result cells.
    pub scenarios: Vec<(String, Vec<CellValue>)>,
}

/// Recalculate `results` with each scenario applied in turn, starting from the sheet's
/// current values each time (the workbook itself is not changed).
#[must_use]
pub fn summary(wb: &Workbook, sheet: usize, list: &[&Scenario], results: &[CellRef]) -> Summary {
    let ws = &wb.sheets[sheet];
    let mut changing: Vec<CellRef> = Vec::new();
    for sc in list {
        for c in sc.cells.iter().filter(|c| !c.deleted) {
            if !changing.contains(&c.cell) {
                changing.push(c.cell);
            }
        }
    }
    let system = if wb.date1904 {
        bo_formula::DateSystem::Date1904
    } else {
        bo_formula::DateSystem::Date1900
    };
    let mut fw = load(wb, None);
    fw.recalculate_all();
    let saved: Vec<bo_formula::Value> = changing
        .iter()
        .map(|c| fw.value(sheet, c.row, c.col))
        .collect();
    let read_result = |fw: &bo_formula::Workbook, c: &CellRef| {
        let v = fw.value(sheet, c.row, c.col);
        if matches!(v, bo_formula::Value::Empty) {
            CellValue::Empty
        } else {
            to_cell_value(&v)
        }
    };
    let mut current: Vec<CellValue> = changing
        .iter()
        .map(|c| {
            ws.cell(c.row, c.col)
                .map(|x| x.value.clone())
                .unwrap_or_default()
        })
        .collect();
    current.extend(results.iter().map(|c| read_result(&fw, c)));
    let mut out = Vec::with_capacity(list.len());
    for sc in list {
        let mut vals = Vec::with_capacity(changing.len() + results.len());
        for c in &changing {
            let v = sc
                .cells
                .iter()
                .find(|x| x.cell == *c && !x.deleted)
                .map_or(CellValue::Empty, |x| parse_value(&x.value));
            if sc.cells.iter().any(|x| x.cell == *c && !x.deleted) {
                fw.set_value(sheet, c.row, c.col, to_value(&v, system));
            }
            vals.push(v);
        }
        fw.recalculate();
        vals.extend(results.iter().map(|c| read_result(&fw, c)));
        out.push((sc.name.clone(), vals));
        // Back to the current values before the next scenario.
        for (c, v) in changing.iter().zip(&saved) {
            fw.set_value(sheet, c.row, c.col, v.clone());
        }
        fw.recalculate();
    }
    Summary {
        changing,
        results: results.to_vec(),
        current,
        scenarios: out,
    }
}

/// Captions written into the summary sheet (localized by the host).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SummaryLabels {
    /// Name of the new sheet (a number is added when taken).
    pub sheet: String,
    /// Heading in the first cell; `{sheet}` is replaced by the source sheet's name.
    pub title: String,
    /// Header of the current values column.
    pub current: String,
    /// Caption of the changing cells block.
    pub changing: String,
    /// Caption of the result cells block.
    pub results: String,
    /// Row caption for the scenarios' comments.
    pub comment: String,
    /// Note under the table.
    pub note: String,
}

impl Default for SummaryLabels {
    fn default() -> Self {
        SummaryLabels {
            sheet: "Scenario report".into(),
            title: "Scenarios of {sheet}".into(),
            current: "Now".into(),
            changing: "Inputs".into(),
            results: "Outcomes".into(),
            comment: "Notes".into(),
            note: "\"Now\" holds the sheet's values when the report was made; each scenario \
                   column is recalculated with that scenario's inputs."
                .into(),
        }
    }
}

/// A sheet name not used in `wb`, at most 31 characters (§18.2.19 `sheet/@name`).
fn free_sheet_name(wb: &Workbook, base: &str) -> String {
    let clean: String = base
        .chars()
        .filter(|c| !matches!(c, ':' | '\\' | '/' | '?' | '*' | '[' | ']'))
        .take(31)
        .collect();
    let base = if clean.trim().is_empty() {
        "Report".to_owned()
    } else {
        clean
    };
    let taken = |n: &str| wb.sheets.iter().any(|s| s.name.eq_ignore_ascii_case(n));
    if !taken(&base) {
        return base;
    }
    let mut n = 2;
    loop {
        let suffix = format!(" {n}");
        let head: String = base.chars().take(31 - suffix.chars().count()).collect();
        let cand = format!("{head}{suffix}");
        if !taken(&cand) {
            return cand;
        }
        n += 1;
    }
}

/// `$B$3` form of a cell.
fn absolute(c: CellRef) -> String {
    let a1 = c.to_string();
    let split = a1.find(|ch: char| ch.is_ascii_digit()).unwrap_or(a1.len());
    format!("${}${}", &a1[..split], &a1[split..])
}

/// The summary laid out on a new worksheet named after `labels.sheet`:
///
/// ```text
/// A1  title
/// row 3:  [blank] [blank] current  scenario 1  scenario 2 …
/// row 4:  notes   (blank)          comment 1   comment 2   (only when a scenario has one)
///         inputs
///         [blank] $B$1    value    value       value
///         outcomes
///         [blank] $B$5    value    value       value
///         note
/// ```
///
/// Value cells reuse the source cell's style index so number formats carry over.
#[must_use]
pub fn summary_sheet(
    wb: &Workbook,
    sheet: usize,
    list: &[&Scenario],
    s: &Summary,
    labels: &SummaryLabels,
) -> Worksheet {
    let src = &wb.sheets[sheet];
    let mut ws = Worksheet::new(&free_sheet_name(wb, &labels.sheet));
    ws.sheet_id = wb.sheets.iter().map(|w| w.sheet_id).max().unwrap_or(0) + 1;
    let text = |t: &str| CellValue::Text(t.to_owned());
    ws.set_cell(
        0,
        0,
        Cell::new(text(&labels.title.replace("{sheet}", &src.name))),
    );
    let mut row = 2;
    ws.set_cell(row, 2, Cell::new(text(&labels.current)));
    for (i, (name, _)) in s.scenarios.iter().enumerate() {
        ws.set_cell(row, 3 + col(i), Cell::new(text(name)));
    }
    row += 1;
    if list.iter().any(|sc| sc.comment.is_some()) {
        ws.set_cell(row, 0, Cell::new(text(&labels.comment)));
        for (i, sc) in list.iter().enumerate() {
            if let Some(c) = &sc.comment {
                ws.set_cell(row, 3 + col(i), Cell::new(text(c)));
            }
        }
        row += 1;
    }
    let style_of = |c: CellRef| src.cell(c.row, c.col).map_or(0, |x| x.style);
    let block = |ws: &mut Worksheet, row: &mut u32, caption: &str, cells: &[CellRef], at| {
        ws.set_cell(*row, 0, Cell::new(text(caption)));
        *row += 1;
        for (k, c) in cells.iter().enumerate() {
            ws.set_cell(*row, 1, Cell::new(text(&absolute(*c))));
            let style = style_of(*c);
            let put = |ws: &mut Worksheet, col: u32, v: &CellValue| {
                if !matches!(v, CellValue::Empty) {
                    let mut cell = Cell::new(v.clone());
                    cell.style = style;
                    ws.set_cell(*row, col, cell);
                }
            };
            put(ws, 2, &s.current[at + k]);
            for (i, (_, vals)) in s.scenarios.iter().enumerate() {
                put(ws, 3 + col(i), &vals[at + k]);
            }
            *row += 1;
        }
    };
    block(&mut ws, &mut row, &labels.changing, &s.changing, 0);
    if !s.results.is_empty() {
        block(
            &mut ws,
            &mut row,
            &labels.results,
            &s.results,
            s.changing.len(),
        );
    }
    if !labels.note.is_empty() {
        ws.set_cell(row + 1, 0, Cell::new(text(&labels.note)));
    }
    ws
}

fn col(i: usize) -> u32 {
    u32::try_from(i).unwrap_or(u32::MAX - 4)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn book(cells: &[(&str, &str)]) -> Workbook {
        let mut ws = Worksheet::new("Plan");
        for (a1, input) in cells {
            let r = CellRef::parse(a1).unwrap_or_default();
            if let Some(f) = input.strip_prefix('=') {
                ws.set_cell(r.row, r.col, Cell::default().with_formula(f));
            } else {
                ws.set_cell(r.row, r.col, Cell::new(parse_value(input)));
            }
        }
        let mut wb = Workbook::default();
        wb.sheets.push(ws);
        crate::formula::BoFormula::new().recalculate(&mut wb);
        wb
    }

    fn cr(a1: &str) -> CellRef {
        CellRef::parse(a1).unwrap_or_default()
    }

    fn sc(name: &str, cells: &[(&str, &str)]) -> Scenario {
        let refs: Vec<CellRef> = cells.iter().map(|(a, _)| cr(a)).collect();
        let vals: Vec<String> = cells.iter().map(|(_, v)| (*v).to_owned()).collect();
        build(name, &refs, &vals, None, None, true, false).unwrap_or_default()
    }

    use crate::calc::FormulaEngine;

    #[test]
    fn values_round_trip_as_text() {
        assert_eq!(value_text(&CellValue::Number(0.05)), "0.05");
        assert_eq!(value_text(&CellValue::Number(200_000.0)), "200000");
        assert_eq!(value_text(&CellValue::Bool(true)), "TRUE");
        assert_eq!(parse_value("200000"), CellValue::Number(200_000.0));
        assert_eq!(parse_value("-1.5"), CellValue::Number(-1.5));
        assert_eq!(parse_value("false"), CellValue::Bool(false));
        assert_eq!(parse_value(""), CellValue::Empty);
        assert_eq!(parse_value("Hà Nội"), CellValue::Text("Hà Nội".into()));
    }

    #[test]
    fn build_checks_name_and_cells() {
        let c = [cr("A1")];
        let v = ["1".to_owned()];
        assert_eq!(
            build(" ", &c, &v, None, None, false, false),
            Err(ScenarioError::BadName)
        );
        assert_eq!(
            build("x", &[], &[], None, None, false, false),
            Err(ScenarioError::BadCells)
        );
        assert_eq!(
            build("x", &c, &[], None, None, false, false),
            Err(ScenarioError::ValueCount)
        );
        // 6 × 6 = 36 cells, over the limit of 32.
        let many = cells_of(&[CellRange::parse("A1:F6").unwrap_or_default()]);
        assert!(many.len() > MAX_CHANGING_CELLS);
        let ok = build(" Best ", &c, &v, Some(String::new()), None, true, false);
        assert_eq!(ok.as_ref().map(|s| s.name.as_str()), Ok("Best"));
        assert_eq!(ok.map(|s| s.comment), Ok(None));
    }

    #[test]
    fn save_delete_and_merge_keep_names_unique() {
        let mut list = Scenarios::default();
        assert_eq!(save(&mut list, None, sc("Low", &[("A1", "1")])), Ok(0));
        assert_eq!(save(&mut list, None, sc("High", &[("A1", "9")])), Ok(1));
        assert_eq!(
            save(&mut list, None, sc("low", &[("A1", "2")])),
            Err(ScenarioError::DuplicateName)
        );
        // Editing a scenario may keep its own name.
        assert_eq!(save(&mut list, Some(0), sc("Low", &[("A1", "3")])), Ok(0));
        assert_eq!(list.items[0].cells[0].value, "3");
        assert_eq!(
            save(&mut list, Some(5), sc("Mid", &[("A1", "5")])),
            Err(ScenarioError::NotFound)
        );
        list.current = Some(1);
        list.show = Some(0);
        assert_eq!(delete(&mut list, 0), Ok(()));
        // "High" moved from 1 to 0; the deleted one was shown.
        assert_eq!(list.current, Some(0));
        assert_eq!(list.show, None);
        let other = [sc("High", &[("B1", "1")]), sc("Other", &[("B1", "2")])];
        assert_eq!(merge(&mut list, &other), 2);
        let names: Vec<&str> = list.items.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["High", "High (2)", "Other"]);
    }

    #[test]
    fn show_enters_values_over_formulas() {
        let mut wb = book(&[("A1", "1"), ("A2", "=A1*2")]);
        show(
            &mut wb.sheets[0],
            &sc("S", &[("A1", "5"), ("A2", "7"), ("A3", "x")]),
        );
        let ws = &wb.sheets[0];
        assert_eq!(
            ws.cell(0, 0).map(|c| &c.value),
            Some(&CellValue::Number(5.0))
        );
        let a2 = ws.cell(1, 0);
        assert!(a2.is_some_and(|c| c.formula.is_none()));
        assert_eq!(a2.map(|c| &c.value), Some(&CellValue::Number(7.0)));
        assert_eq!(
            ws.cell(2, 0).map(|c| &c.value),
            Some(&CellValue::Text("x".into()))
        );
    }

    #[test]
    fn summary_recalculates_each_scenario_from_the_current_values() {
        // Price B1 = 10, quantity B2 = 100, cost B3 = 600; profit B4 = B1*B2 - B3.
        let wb = book(&[
            ("B1", "10"),
            ("B2", "100"),
            ("B3", "600"),
            ("B4", "=B1*B2-B3"),
        ]);
        let a = sc("Cheap", &[("B1", "8"), ("B2", "150")]);
        // "Costly" changes only the cost: B1 and B2 keep their current values.
        let b = sc("Costly", &[("B3", "900")]);
        let list = [&a, &b];
        let s = summary(&wb, 0, &list, &[cr("B4")]);
        assert_eq!(s.changing, [cr("B1"), cr("B2"), cr("B3")]);
        let n = CellValue::Number;
        // Current: 10, 100, 600 → profit 10·100 − 600 = 400.
        assert_eq!(s.current, [n(10.0), n(100.0), n(600.0), n(400.0)]);
        // Cheap: 8·150 − 600 = 600.
        assert_eq!(s.scenarios[0].0, "Cheap");
        assert_eq!(
            s.scenarios[0].1,
            [n(8.0), n(150.0), CellValue::Empty, n(600.0)]
        );
        // Costly starts again from 10 and 100: 10·100 − 900 = 100.
        assert_eq!(
            s.scenarios[1].1,
            [CellValue::Empty, CellValue::Empty, n(900.0), n(100.0)]
        );
        // The workbook is untouched.
        assert_eq!(wb.sheets[0].cell(0, 1).map(|c| &c.value), Some(&n(10.0)));

        let mut a = a;
        a.comment = Some("low price".into());
        let list = [&a, &b];
        let s = summary(&wb, 0, &list, &[cr("B4")]);
        let labels = SummaryLabels::default();
        let ws = summary_sheet(&wb, 0, &list, &s, &labels);
        assert_eq!(ws.name, "Scenario report");
        assert_eq!(ws.sheet_id, 2);
        let at = |a1: &str| ws.cell_a1(a1).map(|c| c.value.clone()).unwrap_or_default();
        let t = |s: &str| CellValue::Text(s.into());
        assert_eq!(at("A1"), t("Scenarios of Plan"));
        assert_eq!(at("C3"), t("Now"));
        assert_eq!(at("D3"), t("Cheap"));
        assert_eq!(at("E3"), t("Costly"));
        assert_eq!(at("A4"), t("Notes"));
        assert_eq!(at("D4"), t("low price"));
        assert_eq!(at("A5"), t("Inputs"));
        assert_eq!(at("B6"), t("$B$1"));
        assert_eq!(at("C6"), n(10.0));
        assert_eq!(at("D6"), n(8.0));
        assert_eq!(at("E6"), CellValue::Empty);
        assert_eq!(at("B8"), t("$B$3"));
        assert_eq!(at("E8"), n(900.0));
        assert_eq!(at("A9"), t("Outcomes"));
        assert_eq!(at("B10"), t("$B$4"));
        assert_eq!(at("C10"), n(400.0));
        assert_eq!(at("D10"), n(600.0));
        assert_eq!(at("E10"), n(100.0));
        assert!(matches!(at("A12"), CellValue::Text(_)));
    }

    #[test]
    fn report_sheet_name_is_free() {
        let mut wb = book(&[("A1", "1")]);
        wb.sheets.push(Worksheet::new("Scenario report"));
        assert_eq!(free_sheet_name(&wb, "Scenario report"), "Scenario report 2");
        assert_eq!(free_sheet_name(&wb, "a/b"), "ab");
        assert_eq!(absolute(cr("AB12")), "$AB$12");
    }
}
