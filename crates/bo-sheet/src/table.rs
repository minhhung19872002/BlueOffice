//! Tables (ECMA-376 Part 1, §18.5): creating a table over a range, the totals row
//! (§18.5.1.3 `tableColumn/@totalsRowFunction`, `totalsRowFormula`, `totalsRowLabel`),
//! calculated columns (`calculatedColumnFormula`), automatic expansion, resizing, header row
//! and style options (§18.5.1.5 `tableStyleInfo`), renaming, and converting a table back to a
//! plain range (structured references rewritten as A1 references).
//!
//! Every function edits the [`Workbook`] in place; the engine wraps each call in one undo step.

use bo_formula::{
    format, offset_relative, parse, CellAddr, Expr, RefKind, Reference, SheetRef, StructItem,
    StructRef, TableInfo,
};
use bo_model::sheet::{
    Cell, CellRange, CellRef, CellValue, Formula, SlicerSource, Table, TableColumn, TableStyleInfo,
    Workbook, Worksheet, MAX_COLS, MAX_ROWS,
};

use crate::numfmt::Locale;
use crate::render::format_cell;

/// Totals row functions (§18.18.86 `ST_TotalsRowFunction`).
pub const TOTAL_FUNCTIONS: [&str; 10] = [
    "none",
    "sum",
    "min",
    "max",
    "average",
    "count",
    "countNums",
    "stdDev",
    "var",
    "custom",
];

/// `SUBTOTAL` function number of a totals row function: the 10x forms skip hidden and
/// filtered rows (§18.17.7.315 SUBTOTAL). `count` counts non-empty cells (`COUNTA`, 3),
/// `countNums` counts numbers (`COUNT`, 2).
#[must_use]
pub fn subtotal_code(function: &str) -> Option<u32> {
    Some(match function {
        "average" => 101,
        "countNums" => 102,
        "count" => 103,
        "max" => 104,
        "min" => 105,
        "stdDev" => 107,
        "sum" => 109,
        "var" => 110,
        _ => return None,
    })
}

/// The table has a header row (`headerRowCount`, default 1).
#[must_use]
pub fn has_header(t: &Table) -> bool {
    t.header_row_count.unwrap_or(1) > 0
}

/// The table has a totals row (`totalsRowCount`, default 0).
#[must_use]
pub fn has_totals(t: &Table) -> bool {
    t.totals_row_count.unwrap_or(0) > 0
}

/// First and last data row; `None` when the table has no data row.
#[must_use]
pub fn data_rows(t: &Table) -> Option<(u32, u32)> {
    let a = t.range.first.row + u32::from(has_header(t));
    let b = t.range.last.row.checked_sub(u32::from(has_totals(t)))?;
    (a <= b).then_some((a, b))
}

/// Index of the table of `ws` containing (`row`, `col`).
#[must_use]
pub fn table_at(ws: &Worksheet, row: u32, col: u32) -> Option<usize> {
    ws.tables
        .iter()
        .position(|t| t.range.contains(CellRef::new(row, col)))
}

/// Geometry of a table for structured-reference resolution.
#[must_use]
pub fn table_info(t: &Table, sheet: usize) -> TableInfo {
    TableInfo {
        name: t.name.clone(),
        sheet,
        first_row: t.range.first.row,
        last_row: t.range.last.row,
        first_col: t.range.first.col,
        columns: t.columns.iter().map(|c| c.name.clone()).collect(),
        header_row: has_header(t),
        totals_row: has_totals(t),
    }
}

fn overlaps(a: &CellRange, b: &CellRange) -> bool {
    a.first.row <= b.last.row
        && b.first.row <= a.last.row
        && a.first.col <= b.last.col
        && b.first.col <= a.last.col
}

fn is_empty(ws: &Worksheet, row: u32, col: u32) -> bool {
    ws.cells
        .get(&(row, col))
        .is_none_or(|c| c.formula.is_none() && matches!(c.value, CellValue::Empty))
}

/// Text of a cell as shown.
fn cell_text(wb: &Workbook, ws: &Worksheet, row: u32, col: u32, locale: &Locale) -> String {
    ws.cells
        .get(&(row, col))
        .map(|c| format_cell(wb, c, locale).text)
        .unwrap_or_default()
        .trim()
        .to_owned()
}

/// Put a text / formula into a cell, keeping its style.
fn put(ws: &mut Worksheet, row: u32, col: u32, value: CellValue, formula: Option<&str>) {
    let style = ws.cells.get(&(row, col)).map_or(0, |c| c.style);
    if formula.is_none() && matches!(value, CellValue::Empty) && style == 0 {
        ws.cells.remove(&(row, col));
        return;
    }
    let cell = Cell {
        value,
        formula: formula.map(Formula::normal),
        style,
        ..Cell::default()
    };
    ws.cells.insert((row, col), cell);
}

/// Clear the content of a cell, keeping its style.
fn clear(ws: &mut Worksheet, row: u32, col: u32) {
    put(ws, row, col, CellValue::Empty, None);
}

/// A name unique among `names` (case-insensitive): `base`, `base2`, `base3`, …
fn unique_name(base: &str, names: &[String]) -> String {
    let mut name = base.to_owned();
    let mut n = 2;
    while names
        .iter()
        .any(|x| x.to_lowercase() == name.to_lowercase())
    {
        name = format!("{base}{n}");
        n += 1;
    }
    name
}

/// Turn `range` of sheet `sheet` into a table whose first row holds the column names (blank
/// or repeated headers get `Cột n`). Returns the table's index on the sheet.
///
/// # Errors
/// A range of one row, or one that overlaps another table.
pub fn create_table(
    wb: &mut Workbook,
    sheet: usize,
    range: CellRange,
    locale: &Locale,
) -> Result<usize, String> {
    if range.last.row <= range.first.row {
        return Err("a table needs a header row and data".into());
    }
    let ws = wb.sheets.get(sheet).ok_or("no such sheet")?;
    if ws.tables.iter().any(|t| overlaps(&t.range, &range)) {
        return Err("the range overlaps a table".into());
    }
    // Column names from the header row, unique (§18.5.1.3 tableColumn/@name).
    let mut names: Vec<String> = Vec::new();
    for (k, col) in (range.first.col..=range.last.col).enumerate() {
        let text = cell_text(wb, ws, range.first.row, col, locale);
        let base = if text.is_empty() {
            format!("Cột {}", k + 1)
        } else {
            text
        };
        names.push(unique_name(&base, &names));
    }
    // Table ids and names are unique in the workbook.
    let id = wb
        .sheets
        .iter()
        .flat_map(|s| &s.tables)
        .map(|t| t.id)
        .max()
        .unwrap_or(0)
        + 1;
    let mut n = id;
    let mut name = format!("Bảng{n}");
    while name_taken(wb, &name, None) {
        n += 1;
        name = format!("Bảng{n}");
    }
    let ws = &mut wb.sheets[sheet];
    // The header row shows the column names.
    for (k, col) in (range.first.col..=range.last.col).enumerate() {
        if is_empty(ws, range.first.row, col) {
            put(
                ws,
                range.first.row,
                col,
                CellValue::Text(names[k].clone()),
                None,
            );
        }
    }
    ws.tables.push(Table {
        id,
        name: name.clone(),
        display_name: name,
        range,
        totals_row_shown: Some(false),
        columns: names
            .into_iter()
            .enumerate()
            .map(|(k, name)| TableColumn {
                id: k as u32 + 1,
                name,
                ..TableColumn::default()
            })
            .collect(),
        style: Some(TableStyleInfo {
            name: Some("TableStyleMedium2".into()),
            show_row_stripes: true,
            ..TableStyleInfo::default()
        }),
        auto_filter: Some(format!("<autoFilter ref=\"{range}\"/>")),
        ..Table::default()
    });
    Ok(ws.tables.len() - 1)
}

/// A table or defined name other than table `except` (sheet, index) uses `name`.
fn name_taken(wb: &Workbook, name: &str, except: Option<(usize, usize)>) -> bool {
    let lower = name.to_lowercase();
    wb.sheets.iter().enumerate().any(|(si, s)| {
        s.tables
            .iter()
            .enumerate()
            .any(|(ti, t)| Some((si, ti)) != except && t.name.to_lowercase() == lower)
    }) || wb
        .defined_names
        .iter()
        .any(|d| d.name.to_lowercase() == lower)
}

/// A valid table name: starts with a letter, `_` or `\`, then letters, digits, `_` or `.`;
/// not a cell reference (§18.5.1.2 `table/@name`, same rules as defined names §18.2.5).
#[must_use]
pub fn valid_table_name(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first.is_alphabetic() || first == '_' || first == '\\') {
        return false;
    }
    if !chars.all(|c| c.is_alphanumeric() || c == '_' || c == '.') || name.chars().count() > 255 {
        return false;
    }
    let up = name.to_ascii_uppercase();
    if CellRef::parse(name).is_some() || up == "R" || up == "C" {
        return false;
    }
    // R1C1-style references (R2C3, R, C5).
    let r1c1 = |s: &str| {
        let s = s
            .strip_prefix('R')
            .map_or(s, |r| r.trim_start_matches(|c: char| c.is_ascii_digit()));
        let s = s
            .strip_prefix('C')
            .map_or(s, |r| r.trim_start_matches(|c: char| c.is_ascii_digit()));
        s.is_empty()
    };
    !r1c1(&up)
}

// ---------------------------------------------------------------------------
// Formula rewriting
// ---------------------------------------------------------------------------

/// Where a formula lives: sheet and cell (`None` for defined names).
#[derive(Clone, Copy)]
struct Site {
    sheet: Option<usize>,
    row: u32,
    col: u32,
}

/// Rewrite every formula of the workbook (cells, defined names, table column formulas,
/// conditional formats, data validations) with `f`; texts change only when the expression does.
fn rewrite_formulas(wb: &mut Workbook, f: &mut dyn FnMut(Expr, Site) -> Expr) {
    let mut apply = |text: &mut String, site: Site| {
        if text.is_empty() {
            return;
        }
        let Ok(expr) = parse(text) else { return };
        let new = expr.map(&mut |e| f(e, site));
        if new != expr {
            *text = format(&new);
        }
    };
    for dn in &mut wb.defined_names {
        apply(
            &mut dn.formula,
            Site {
                sheet: None,
                row: u32::MAX,
                col: u32::MAX,
            },
        );
    }
    for (si, ws) in wb.sheets.iter_mut().enumerate() {
        for (&(r, c), cell) in &mut ws.cells {
            if let Some(fm) = &mut cell.formula {
                apply(
                    &mut fm.text,
                    Site {
                        sheet: Some(si),
                        row: r,
                        col: c,
                    },
                );
            }
        }
        for t in &mut ws.tables {
            let first = t.range.first;
            let header = u32::from(t.header_row_count.unwrap_or(1) > 0);
            for (k, c) in t.columns.iter_mut().enumerate() {
                let site = Site {
                    sheet: Some(si),
                    row: first.row + header,
                    col: first.col + k as u32,
                };
                for s in [&mut c.calculated_formula, &mut c.totals_row_formula]
                    .into_iter()
                    .flatten()
                {
                    apply(s, site);
                }
            }
        }
        for cf in &mut ws.conditional_formats {
            let at = cf.sqref.first().map_or(CellRef::new(0, 0), |r| r.first);
            let site = Site {
                sheet: Some(si),
                row: at.row,
                col: at.col,
            };
            for r in &mut cf.rules {
                for s in &mut r.formulas {
                    apply(s, site);
                }
            }
        }
        for dv in &mut ws.data_validations {
            let at = dv.sqref.first().map_or(CellRef::new(0, 0), |r| r.first);
            let site = Site {
                sheet: Some(si),
                row: at.row,
                col: at.col,
            };
            for s in [&mut dv.formula1, &mut dv.formula2].into_iter().flatten() {
                apply(s, site);
            }
        }
    }
}

/// The structured reference `s` written at `site` points at table `info`.
fn refers_to(s: &StructRef, site: Site, info: &TableInfo) -> bool {
    match &s.table {
        Some(n) => n.eq_ignore_ascii_case(&info.name),
        None => {
            site.sheet == Some(info.sheet)
                && site.row >= info.first_row
                && site.row <= info.last_row
                && site.col >= info.first_col
                && site.col <= info.last_col()
        }
    }
}

/// Rename the table at (`sheet`, `ti`); formulas follow.
///
/// # Errors
/// An invalid or already used name.
pub fn rename_table(wb: &mut Workbook, sheet: usize, ti: usize, name: &str) -> Result<(), String> {
    let name = name.trim();
    let old = wb
        .sheets
        .get(sheet)
        .and_then(|s| s.tables.get(ti))
        .ok_or("no such table")?
        .name
        .clone();
    if old == name {
        return Ok(());
    }
    if !valid_table_name(name) {
        return Err("invalid table name".into());
    }
    if name_taken(wb, name, Some((sheet, ti))) {
        return Err("the name is already used".into());
    }
    rewrite_formulas(wb, &mut |e, _| match e {
        Expr::Structured(mut s)
            if s.table
                .as_ref()
                .is_some_and(|t| t.eq_ignore_ascii_case(&old)) =>
        {
            s.table = Some(name.to_owned());
            Expr::Structured(s)
        }
        Expr::Name {
            sheet: None,
            name: n,
        } if n.eq_ignore_ascii_case(&old) => Expr::Name {
            sheet: None,
            name: name.to_owned(),
        },
        other => other,
    });
    let t = &mut wb.sheets[sheet].tables[ti];
    t.name = name.to_owned();
    t.display_name = name.to_owned();
    Ok(())
}

/// Rename column `k` of the table at (`sheet`, `ti`) in the model and in formulas.
fn rename_column(wb: &mut Workbook, sheet: usize, ti: usize, k: usize, name: &str) {
    let info = table_info(&wb.sheets[sheet].tables[ti], sheet);
    let old = info.columns[k].clone();
    if old == name {
        return;
    }
    let ren = |c: String| {
        if c.to_lowercase() == old.to_lowercase() {
            name.to_owned()
        } else {
            c
        }
    };
    rewrite_formulas(wb, &mut |e, site| match e {
        Expr::Structured(s) if refers_to(&s, site, &info) => Expr::Structured(StructRef {
            table: s.table,
            items: s
                .items
                .into_iter()
                .map(|i| match i {
                    StructItem::Column(c) => StructItem::Column(ren(c)),
                    StructItem::ColumnRange(a, b) => StructItem::ColumnRange(ren(a), ren(b)),
                    other => other,
                })
                .collect(),
        }),
        other => other,
    });
    wb.sheets[sheet].tables[ti].columns[k].name = name.to_owned();
}

// ---------------------------------------------------------------------------
// Totals row
// ---------------------------------------------------------------------------

/// The cell formula of column `k`'s total, e.g. `SUBTOTAL(109,Bảng1[Số tiền])`.
fn total_formula(t: &Table, k: usize) -> Option<String> {
    let c = &t.columns[k];
    match c.totals_row_function.as_deref() {
        Some("custom") => c.totals_row_formula.clone(),
        Some(f) => {
            let code = subtotal_code(f)?;
            Some(format(&Expr::Call {
                name: "SUBTOTAL".into(),
                args: vec![
                    Expr::Number(f64::from(code)),
                    Expr::Structured(StructRef {
                        table: Some(t.name.clone()),
                        items: vec![StructItem::Column(c.name.clone())],
                    }),
                ],
            }))
        }
        None => None,
    }
}

/// Write the totals row cells of a table from its columns' functions and labels.
fn write_totals(ws: &mut Worksheet, ti: usize) {
    let t = ws.tables[ti].clone();
    if !has_totals(&t) {
        return;
    }
    let row = t.range.last.row;
    for (k, c) in t.columns.iter().enumerate() {
        let col = t.range.first.col + k as u32;
        if let Some(f) = total_formula(&t, k) {
            put(ws, row, col, CellValue::Empty, Some(&f));
        } else if let Some(label) = &c.totals_row_label {
            put(ws, row, col, CellValue::Text(label.clone()), None);
        } else {
            clear(ws, row, col);
        }
    }
}

/// Keep the table's `autoFilter/@ref` on its header and data rows (§18.3.1.2).
fn sync_filter(t: &mut Table) {
    let last = t.range.last.row - u32::from(has_totals(t));
    let r = CellRange::new(t.range.first, CellRef::new(last, t.range.last.col));
    let Some(af) = t.auto_filter.as_mut() else {
        return;
    };
    if let Some(i) = af.find(" ref=\"") {
        let start = i + 6;
        if let Some(len) = af[start..].find('"') {
            af.replace_range(start..start + len, &r.to_string());
        }
    }
}

/// Show or hide the totals row of the table at (`sheet`, `ti`). Shown for the first time, the
/// first column gets `label` and the last column a sum (a count when it holds no numbers).
///
/// # Errors
/// The row below the table is not empty, or the table would have no data row.
pub fn set_totals_row(
    wb: &mut Workbook,
    sheet: usize,
    ti: usize,
    show: bool,
    label: &str,
) -> Result<(), String> {
    let ws = wb.sheets.get_mut(sheet).ok_or("no such sheet")?;
    let t = ws.tables.get(ti).ok_or("no such table")?.clone();
    if show == has_totals(&t) {
        return Ok(());
    }
    if show {
        let row = t.range.last.row + 1;
        if row >= MAX_ROWS {
            return Err("no room below the table".into());
        }
        let below = CellRange::new(
            CellRef::new(row, t.range.first.col),
            CellRef::new(row, t.range.last.col),
        );
        if (below.first.col..=below.last.col).any(|c| !is_empty(ws, row, c))
            || ws.tables.iter().any(|o| overlaps(&o.range, &below))
        {
            return Err("the row below the table is not empty".into());
        }
        let numeric = data_rows(&t).is_some_and(|(a, b)| {
            (a..=b).any(|r| {
                ws.cells
                    .get(&(r, t.range.last.col))
                    .is_some_and(|c| matches!(c.value, CellValue::Number(_)))
            })
        });
        let tm = &mut ws.tables[ti];
        let fresh = tm
            .columns
            .iter()
            .all(|c| c.totals_row_function.is_none() && c.totals_row_label.is_none());
        if fresh {
            let n = tm.columns.len();
            if n > 1 {
                tm.columns[0].totals_row_label = Some(label.to_owned());
            }
            if let Some(c) = tm.columns.last_mut() {
                c.totals_row_function = Some(if numeric { "sum" } else { "count" }.into());
            }
        }
        tm.range.last.row = row;
        tm.totals_row_count = Some(1);
        tm.totals_row_shown = Some(true);
        write_totals(ws, ti);
    } else {
        let row = t.range.last.row;
        for c in t.range.first.col..=t.range.last.col {
            clear(ws, row, c);
        }
        let tm = &mut ws.tables[ti];
        tm.range.last.row = row - 1;
        tm.totals_row_count = None;
    }
    sync_filter(&mut ws.tables[ti]);
    Ok(())
}

/// Set the totals row function of column `k` (one of [`TOTAL_FUNCTIONS`]; `custom` takes
/// `formula`, without `=`) and write the totals cell.
///
/// # Errors
/// No totals row, unknown function or column, or a custom formula that does not parse.
pub fn set_total_function(
    wb: &mut Workbook,
    sheet: usize,
    ti: usize,
    k: usize,
    function: &str,
    formula: Option<&str>,
) -> Result<(), String> {
    let ws = wb.sheets.get_mut(sheet).ok_or("no such sheet")?;
    let t = ws.tables.get_mut(ti).ok_or("no such table")?;
    if !has_totals(t) {
        return Err("the table has no totals row".into());
    }
    if !TOTAL_FUNCTIONS.contains(&function) {
        return Err(format!("unknown totals function '{function}'"));
    }
    let col = t.columns.get_mut(k).ok_or("no such column")?;
    match function {
        "none" => {
            col.totals_row_function = None;
            col.totals_row_formula = None;
        }
        "custom" => {
            let f = formula
                .map(|f| f.trim().trim_start_matches('=').trim().to_owned())
                .filter(|f| !f.is_empty())
                .ok_or("a custom total needs a formula")?;
            parse(&f).map_err(|e| e.to_string())?;
            col.totals_row_function = Some("custom".into());
            col.totals_row_formula = Some(f);
            col.totals_row_label = None;
        }
        f => {
            col.totals_row_function = Some(f.to_owned());
            col.totals_row_formula = None;
            col.totals_row_label = None;
        }
    }
    write_totals(ws, ti);
    Ok(())
}

// ---------------------------------------------------------------------------
// Header row, style options
// ---------------------------------------------------------------------------

/// Show or hide the header row. Hidden, its cells are cleared (the names stay in the
/// columns) and the filter buttons go; shown again, it takes the row above the table.
///
/// # Errors
/// No data row would remain, or the row above is missing / not empty.
pub fn set_header_row(
    wb: &mut Workbook,
    sheet: usize,
    ti: usize,
    show: bool,
) -> Result<(), String> {
    let ws = wb.sheets.get_mut(sheet).ok_or("no such sheet")?;
    let t = ws.tables.get(ti).ok_or("no such table")?.clone();
    if show == has_header(&t) {
        return Ok(());
    }
    if show {
        let row = t
            .range
            .first
            .row
            .checked_sub(1)
            .ok_or("no row above the table")?;
        let above = CellRange::new(
            CellRef::new(row, t.range.first.col),
            CellRef::new(row, t.range.last.col),
        );
        if (above.first.col..=above.last.col).any(|c| !is_empty(ws, row, c))
            || ws.tables.iter().any(|o| overlaps(&o.range, &above))
        {
            return Err("the row above the table is not empty".into());
        }
        for (k, c) in t.columns.iter().enumerate() {
            put(
                ws,
                row,
                t.range.first.col + k as u32,
                CellValue::Text(c.name.clone()),
                None,
            );
        }
        let tm = &mut ws.tables[ti];
        tm.range.first.row = row;
        tm.header_row_count = None;
        tm.auto_filter = Some(String::from("<autoFilter ref=\"A1\"/>"));
        sync_filter(tm);
    } else {
        let (a, b) = data_rows(&t).ok_or("the table has no data row")?;
        if a > b {
            return Err("the table has no data row".into());
        }
        let row = t.range.first.row;
        for c in t.range.first.col..=t.range.last.col {
            clear(ws, row, c);
        }
        let tm = &mut ws.tables[ti];
        tm.range.first.row = row + 1;
        tm.header_row_count = Some(0);
        tm.auto_filter = None;
    }
    Ok(())
}

/// Style changes of a table: `tableStyleInfo` (§18.5.1.5) and the filter buttons.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TableStylePatch {
    /// Style name (e.g. `TableStyleMedium2`); empty = no style.
    #[serde(default)]
    pub style: Option<String>,
    /// `showRowStripes`.
    #[serde(default)]
    pub banded_rows: Option<bool>,
    /// `showColumnStripes`.
    #[serde(default)]
    pub banded_columns: Option<bool>,
    /// `showFirstColumn`.
    #[serde(default)]
    pub first_column: Option<bool>,
    /// `showLastColumn`.
    #[serde(default)]
    pub last_column: Option<bool>,
    /// Filter buttons in the header row (the table's `autoFilter`, §18.3.1.2).
    #[serde(default)]
    pub filter_button: Option<bool>,
}

/// Apply `p` to the table at (`sheet`, `ti`).
///
/// # Errors
/// No such table, or filter buttons asked for a table without header row.
pub fn set_style(
    wb: &mut Workbook,
    sheet: usize,
    ti: usize,
    p: &TableStylePatch,
) -> Result<(), String> {
    let t = wb
        .sheets
        .get_mut(sheet)
        .and_then(|s| s.tables.get_mut(ti))
        .ok_or("no such table")?;
    let info = t.style.get_or_insert_with(TableStyleInfo::default);
    if let Some(s) = &p.style {
        let s = s.trim();
        info.name = (!s.is_empty()).then(|| s.to_owned());
    }
    if let Some(v) = p.banded_rows {
        info.show_row_stripes = v;
    }
    if let Some(v) = p.banded_columns {
        info.show_column_stripes = v;
    }
    if let Some(v) = p.first_column {
        info.show_first_column = v;
    }
    if let Some(v) = p.last_column {
        info.show_last_column = v;
    }
    match p.filter_button {
        Some(true) if t.auto_filter.is_none() => {
            if !has_header(t) {
                return Err("filter buttons need a header row".into());
            }
            t.auto_filter = Some(String::from("<autoFilter ref=\"A1\"/>"));
            sync_filter(t);
        }
        Some(false) => t.auto_filter = None,
        _ => {}
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Resize, expansion, calculated columns
// ---------------------------------------------------------------------------

/// Resize the table at (`sheet`, `ti`) to `range` (header and totals rows included). The header
/// row stays where it is; new columns take their names from the header cells; calculated columns
/// fill the new rows; the totals row moves to the new last row.
///
/// # Errors
/// The header row moves, the range does not overlap the table, leaves no data row, or overlaps
/// another table; the new totals row is not empty.
pub fn resize_table(
    wb: &mut Workbook,
    sheet: usize,
    ti: usize,
    range: CellRange,
    locale: &Locale,
) -> Result<(), String> {
    let ws = wb.sheets.get(sheet).ok_or("no such sheet")?;
    let t = ws.tables.get(ti).ok_or("no such table")?.clone();
    if range == t.range {
        return Ok(());
    }
    if range.first.row != t.range.first.row {
        return Err("the header row must stay in the same row".into());
    }
    if !overlaps(&range, &t.range) {
        return Err("the new range must overlap the table".into());
    }
    if range.last.row >= MAX_ROWS || range.last.col >= MAX_COLS {
        return Err("the range is outside the sheet".into());
    }
    let extra = u32::from(has_header(&t)) + u32::from(has_totals(&t));
    if range.rows() < extra + 1 {
        return Err("the table needs a data row".into());
    }
    if ws
        .tables
        .iter()
        .enumerate()
        .any(|(i, o)| i != ti && overlaps(&o.range, &range))
    {
        return Err("the range overlaps another table".into());
    }
    let totals = has_totals(&t);
    if totals
        && range.last.row != t.range.last.row
        && (range.first.col..=range.last.col).any(|c| !is_empty(ws, range.last.row, c))
    {
        return Err("the new totals row is not empty".into());
    }
    // Columns: kept ones keep their settings, new ones are named from the header cells.
    let mut names: Vec<String> = Vec::new();
    let mut columns: Vec<TableColumn> = Vec::new();
    let mut next_id = t.columns.iter().map(|c| c.id).max().unwrap_or(0) + 1;
    let mut header_writes: Vec<(u32, String)> = Vec::new();
    for (k, col) in (range.first.col..=range.last.col).enumerate() {
        if (t.range.first.col..=t.range.last.col).contains(&col) {
            let c = t.columns[(col - t.range.first.col) as usize].clone();
            names.push(c.name.clone());
            columns.push(c);
            continue;
        }
        let text = if has_header(&t) {
            cell_text(wb, ws, range.first.row, col, locale)
        } else {
            String::new()
        };
        let base = if text.is_empty() {
            format!("Cột {}", k + 1)
        } else {
            text.clone()
        };
        let name = unique_name(&base, &names);
        if has_header(&t) && name != text {
            header_writes.push((col, name.clone()));
        }
        names.push(name.clone());
        columns.push(TableColumn {
            id: next_id,
            name,
            ..TableColumn::default()
        });
        next_id += 1;
    }
    // Names of the kept columns must stay unique against the new ones.
    let ws = &mut wb.sheets[sheet];
    if totals && range.last.row != t.range.last.row {
        for c in t.range.first.col..=t.range.last.col {
            clear(ws, t.range.last.row, c);
        }
    }
    for (col, name) in header_writes {
        put(ws, range.first.row, col, CellValue::Text(name), None);
    }
    let old = t.range;
    let tm = &mut ws.tables[ti];
    tm.range = range;
    tm.columns = columns;
    sync_filter(tm);
    if totals {
        write_totals(ws, ti);
    }
    // Calculated columns fill the rows added at the bottom.
    let tm = &ws.tables[ti];
    if let Some((a, b)) = data_rows(tm) {
        let old_last = old.last.row - u32::from(totals);
        if b > old_last {
            fill_calculated(ws, ti, (old_last + 1).max(a), b);
        }
    }
    Ok(())
}

/// Write the calculated column formulas (§18.5.1.3 `calculatedColumnFormula`) of the table
/// `ti` into its empty cells of rows `r0..=r1` (a table whose rows were rewritten, e.g. a
/// refreshed query table); the cells written.
pub fn fill_calculated_rows(ws: &mut Worksheet, ti: usize, r0: u32, r1: u32) -> Vec<(u32, u32)> {
    if ti >= ws.tables.len() || r0 > r1 {
        return Vec::new();
    }
    fill_calculated(ws, ti, r0, r1)
}

/// Write the calculated column formulas of the table into its empty cells of rows `r0..=r1`.
fn fill_calculated(ws: &mut Worksheet, ti: usize, r0: u32, r1: u32) -> Vec<(u32, u32)> {
    let t = ws.tables[ti].clone();
    let Some((a, _)) = data_rows(&t) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (k, c) in t.columns.iter().enumerate() {
        let Some(base) = &c.calculated_formula else {
            continue;
        };
        let Ok(expr) = parse(base) else { continue };
        let col = t.range.first.col + k as u32;
        for r in r0..=r1 {
            if is_empty(ws, r, col) {
                let f = format(&offset_relative(&expr, i64::from(r) - i64::from(a), 0));
                put(ws, r, col, CellValue::Empty, Some(&f));
                out.push((r, col));
            }
        }
    }
    out
}

/// The table expansion an edit of `cells` asks for: values typed right below a table (without
/// totals row) add rows, values typed right of it add columns. Returns the table and its new
/// range.
#[must_use]
pub fn expansion_for(
    wb: &Workbook,
    sheet: usize,
    cells: &[(u32, u32)],
) -> Option<(usize, CellRange)> {
    let ws = wb.sheets.get(sheet)?;
    let filled: Vec<(u32, u32)> = cells
        .iter()
        .copied()
        .filter(|&(r, c)| !is_empty(ws, r, c) && table_at(ws, r, c).is_none())
        .collect();
    if filled.is_empty() {
        return None;
    }
    for (ti, t) in ws.tables.iter().enumerate() {
        let mut range = t.range;
        if !has_totals(t) {
            while range.last.row + 1 < MAX_ROWS
                && filled.iter().any(|&(r, c)| {
                    r == range.last.row + 1 && (t.range.first.col..=t.range.last.col).contains(&c)
                })
            {
                range.last.row += 1;
            }
        }
        while range.last.col + 1 < MAX_COLS
            && filled.iter().any(|&(r, c)| {
                c == range.last.col + 1 && (t.range.first.row..=t.range.last.row).contains(&r)
            })
        {
            range.last.col += 1;
        }
        if range != t.range
            && !ws
                .tables
                .iter()
                .enumerate()
                .any(|(i, o)| i != ti && overlaps(&o.range, &range))
        {
            return Some((ti, range));
        }
    }
    None
}

/// `e` with in-table structured references (`[@Col]`) naming table `table`.
fn qualified(e: &Expr, table: &str) -> Expr {
    e.map(&mut |x| match x {
        Expr::Structured(StructRef { table: None, items }) => Expr::Structured(StructRef {
            table: Some(table.to_owned()),
            items,
        }),
        other => other,
    })
}

/// Normalized text of `f` (in table `table`) moved by `d_row` rows.
fn shifted(f: &str, d_row: i64, table: &str) -> Option<String> {
    let e = parse(f).ok()?;
    Some(format(&offset_relative(&qualified(&e, table), d_row, 0)))
}

/// Name the table in the in-table structured references (`[@Col]` → `Bảng1[[#This Row],[Col]]`)
/// of formulas typed in `cells`: files store table formulas with the table name. Only the
/// spelling changes, so this needs no undo step of its own.
pub fn qualify(wb: &mut Workbook, sheet: usize, cells: &[(u32, u32)]) {
    let Some(ws) = wb.sheets.get_mut(sheet) else {
        return;
    };
    for &(r, c) in cells {
        let Some(ti) = table_at(ws, r, c) else {
            continue;
        };
        let name = ws.tables[ti].name.clone();
        let Some(f) = ws.cells.get_mut(&(r, c)).and_then(|x| x.formula.as_mut()) else {
            continue;
        };
        if f.text.is_empty() || !f.text.contains('[') {
            continue;
        }
        let Ok(e) = parse(&f.text) else { continue };
        let q = qualified(&e, &name);
        if q != e {
            f.text = format(&q);
        }
    }
}

/// The calculated column an edit of the single cell `cell` creates: a formula typed into a data
/// cell of a table column whose other data cells are empty (or all hold the column's current
/// calculated formula). Returns (table, column, formula as written in the first data row).
#[must_use]
pub fn calculated_for(
    wb: &Workbook,
    sheet: usize,
    cell: (u32, u32),
) -> Option<(usize, usize, String)> {
    let ws = wb.sheets.get(sheet)?;
    let (r, c) = cell;
    let text = ws.cells.get(&(r, c))?.formula.as_ref()?.text.clone();
    if text.is_empty() {
        return None;
    }
    let ti = table_at(ws, r, c)?;
    let t = &ws.tables[ti];
    let (a, b) = data_rows(t)?;
    if r < a || r > b || a == b {
        return None;
    }
    let k = (c - t.range.first.col) as usize;
    let base = shifted(&text, i64::from(a) - i64::from(r), &t.name)?;
    let others = (a..=b).filter(|&x| x != r);
    match &t.columns[k].calculated_formula {
        None => {
            let mut others = others;
            others.all(|x| is_empty(ws, x, c)).then_some((ti, k, base))
        }
        Some(calc) => {
            let calc = shifted(calc, 0, &t.name)?;
            if calc == base {
                return None;
            }
            // Only a column without exceptions follows a new formula.
            let mut others = others;
            others
                .all(|x| {
                    ws.cells
                        .get(&(x, c))
                        .and_then(|cl| cl.formula.as_ref())
                        .and_then(|f| shifted(&f.text, i64::from(a) - i64::from(x), &t.name))
                        .is_some_and(|f| f == calc)
                })
                .then_some((ti, k, base))
        }
    }
}

/// Make column `k` of the table at (`sheet`, `ti`) a calculated column of `base` (the formula
/// of the first data row): every data cell gets the formula moved to its row
/// (`calculatedColumnFormula`, §18.5.1.3).
pub fn set_calculated(wb: &mut Workbook, sheet: usize, ti: usize, k: usize, base: &str) {
    let Some(ws) = wb.sheets.get_mut(sheet) else {
        return;
    };
    let Some(t) = ws.tables.get(ti).cloned() else {
        return;
    };
    let Some((a, b)) = data_rows(&t) else { return };
    let Ok(expr) = parse(base) else { return };
    let col = t.range.first.col + k as u32;
    for r in a..=b {
        let f = format(&offset_relative(&expr, i64::from(r) - i64::from(a), 0));
        put(ws, r, col, CellValue::Empty, Some(&f));
    }
    if let Some(c) = ws.tables[ti].columns.get_mut(k) {
        c.calculated_formula = Some(base.to_owned());
    }
}

/// Keep the column names of tables in step with header cells typed in `cells`: the name follows
/// the text (formulas use the new name); an empty or repeated header gets a unique name written
/// back into the cell. Returns whether anything changed.
pub fn sync_headers(
    wb: &mut Workbook,
    sheet: usize,
    cells: &[(u32, u32)],
    locale: &Locale,
) -> bool {
    let mut changed = false;
    for &(r, c) in cells {
        let Some(ws) = wb.sheets.get(sheet) else {
            return changed;
        };
        let Some(ti) = table_at(ws, r, c) else {
            continue;
        };
        let t = &ws.tables[ti];
        if !has_header(t) || r != t.range.first.row {
            continue;
        }
        let k = (c - t.range.first.col) as usize;
        let text = cell_text(wb, ws, r, c, locale);
        let others: Vec<String> = t
            .columns
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != k)
            .map(|(_, x)| x.name.clone())
            .collect();
        let base = if text.is_empty() {
            format!("Cột {}", k + 1)
        } else {
            text.clone()
        };
        let name = unique_name(&base, &others);
        let has_formula = ws.cells.get(&(r, c)).is_some_and(|x| x.formula.is_some());
        if name != text || has_formula {
            put(
                &mut wb.sheets[sheet],
                r,
                c,
                CellValue::Text(name.clone()),
                None,
            );
            changed = true;
        }
        if wb.sheets[sheet].tables[ti].columns[k].name != name {
            rename_column(wb, sheet, ti, k, &name);
            changed = true;
        }
    }
    changed
}

// ---------------------------------------------------------------------------
// Convert to range
// ---------------------------------------------------------------------------

/// Turn the table at (`sheet`, `ti`) back into a plain range: structured references to it become
/// A1 references (`$C$2:$C$9`; `[#This Row]` keeps the row relative: `$C2`), its slicers go, and
/// its look is written into the cells as direct formatting.
///
/// # Errors
/// A slicer of the table read from the file (its parts cannot be dropped).
pub fn convert_to_range(wb: &mut Workbook, sheet: usize, ti: usize) -> Result<(), String> {
    let t = wb
        .sheets
        .get(sheet)
        .and_then(|s| s.tables.get(ti))
        .ok_or("no such table")?
        .clone();
    let bound = |s: &bo_model::sheet::Slicer| matches!(s.source, SlicerSource::Table { table_id, .. } if table_id == t.id);
    if wb
        .sheets
        .iter()
        .flat_map(|s| &s.slicers)
        .any(|s| bound(s) && !s.part.is_empty())
    {
        return Err("remove the table's slicers first".into());
    }
    let info = table_info(&t, sheet);
    let sheet_name = wb.sheets[sheet].name.clone();
    let to_ref = |items: &[StructItem], site: Site| -> Expr {
        let row = if site.sheet == Some(sheet) {
            site.row
        } else {
            u32::MAX
        };
        match info.resolve(items, row) {
            Ok((r1, c1, r2, c2)) => {
                let this_row = items.contains(&StructItem::ThisRow);
                let addr = |row: u32, col: u32| CellAddr {
                    row,
                    col,
                    row_abs: !this_row,
                    col_abs: true,
                };
                let kind = if (r1, c1) == (r2, c2) {
                    RefKind::Cell(addr(r1, c1))
                } else {
                    RefKind::Range(addr(r1, c1), addr(r2, c2))
                };
                Expr::Ref(Reference {
                    sheet: (site.sheet != Some(sheet))
                        .then(|| SheetRef::single(sheet_name.clone())),
                    kind,
                })
            }
            Err(e) => Expr::Error(e),
        }
    };
    rewrite_formulas(wb, &mut |e, site| match e {
        Expr::Structured(s) if refers_to(&s, site, &info) => to_ref(&s.items, site),
        Expr::Name { sheet: None, name } if name.eq_ignore_ascii_case(&info.name) => {
            to_ref(&[StructItem::Data], site)
        }
        other => other,
    });
    // The look of the table stays as direct formatting.
    crate::table_style::bake(wb, sheet, &t);
    let gone: Vec<String> = wb
        .sheets
        .iter()
        .flat_map(|s| &s.slicers)
        .filter(|s| bound(s))
        .map(|s| s.cache.clone())
        .collect();
    for ws in &mut wb.sheets {
        ws.slicers.retain(|s| !bound(s));
    }
    wb.defined_names.retain(|d| !gone.contains(&d.name));
    wb.sheets[sheet].tables.remove(ti);
    Ok(())
}

#[cfg(test)]
#[path = "table_tests.rs"]
mod tests;
