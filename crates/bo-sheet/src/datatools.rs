//! Data tools: Text to Columns, Remove Duplicates, Consolidate and the list of
//! cells that violate their data validation ("circle invalid data").
//!
//! Outline groups, Subtotal and Auto Outline live in [`crate::outline`].
//!
//! The behaviour below is our own design on top of the cell model (ECMA-376
//! Part 1, §18.3.1.4 `c`); where a choice was needed (case-insensitive
//! comparison, blank labels skipped, …) it is documented on the function.

use crate::calc::FormulaAdjuster;
use crate::numfmt::{datetime_to_serial, Locale};
use crate::render::format_cell;
use bo_model::sheet::{Cell, CellError, CellRange, CellValue, Workbook, Worksheet};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ------------------------------------------------------------------ Text to Columns

/// How the text of a cell is cut into fields.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "mode",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum SplitMode {
    /// Fields separated by any of `delimiters`.
    Delimited {
        /// Delimiter characters (tab, `;`, `,`, space, any other).
        delimiters: Vec<char>,
        /// A run of delimiters separates only two fields.
        #[serde(default)]
        consecutive: bool,
        /// Text qualifier (`"` or `'`): delimiters inside a qualified field are text;
        /// a doubled qualifier inside it is one qualifier character.
        #[serde(default)]
        qualifier: Option<char>,
    },
    /// Fields of fixed width: `breaks` are character positions (0 < b < length)
    /// where a new field starts.
    Fixed {
        /// Break lines (character offsets).
        breaks: Vec<usize>,
    },
}

/// Cuts `text` into fields.
pub fn split_text(text: &str, mode: &SplitMode) -> Vec<String> {
    match mode {
        SplitMode::Delimited {
            delimiters,
            consecutive,
            qualifier,
        } => split_delimited(text, delimiters, *consecutive, *qualifier),
        SplitMode::Fixed { breaks } => split_fixed(text, breaks),
    }
}

fn split_delimited(
    text: &str,
    delimiters: &[char],
    consecutive: bool,
    qualifier: Option<char>,
) -> Vec<String> {
    let mut fields = Vec::new();
    let mut cur = String::new();
    let mut chars = text.chars().peekable();
    let mut at_start = true;
    while let Some(ch) = chars.next() {
        if at_start && Some(ch) == qualifier {
            // Qualified field: read to the closing qualifier.
            let q = ch;
            loop {
                match chars.next() {
                    None => break,
                    Some(c) if c == q => {
                        if chars.peek() == Some(&q) {
                            cur.push(q);
                            chars.next();
                        } else {
                            break;
                        }
                    }
                    Some(c) => cur.push(c),
                }
            }
            at_start = false;
            continue;
        }
        if delimiters.contains(&ch) {
            fields.push(std::mem::take(&mut cur));
            if consecutive {
                while chars.peek().is_some_and(|c| delimiters.contains(c)) {
                    chars.next();
                }
            }
            at_start = true;
            continue;
        }
        cur.push(ch);
        at_start = false;
    }
    fields.push(cur);
    fields
}

fn split_fixed(text: &str, breaks: &[usize]) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut cuts: Vec<usize> = breaks
        .iter()
        .copied()
        .filter(|&b| b > 0 && b < chars.len())
        .collect();
    cuts.sort_unstable();
    cuts.dedup();
    let mut out = Vec::with_capacity(cuts.len() + 1);
    let mut start = 0;
    for b in cuts.into_iter().chain(std::iter::once(chars.len())) {
        // Fixed-width text is padded with spaces; the padding is not part of the value.
        out.push(chars[start..b].iter().collect::<String>().trim().to_owned());
        start = b;
    }
    out
}

/// Order of day, month and year in a date column.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DateOrder {
    /// Day, month, year.
    Dmy,
    /// Month, day, year.
    Mdy,
    /// Year, month, day.
    Ymd,
    /// Day, year, month.
    Dym,
    /// Month, year, day.
    Myd,
    /// Year, day, month.
    Ydm,
}

/// What a column of Text to Columns becomes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ColumnFormat {
    /// Numbers, dates and logical values are recognised; anything else is text.
    #[default]
    General,
    /// Kept as text (cell format `@`).
    Text,
    /// A date written in the given order.
    Date(DateOrder),
    /// Not written.
    Skip,
}

fn days_in_month(y: i32, m: u32) -> u32 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 => 29,
        2 => 28,
        _ => 0,
    }
}

/// Serial of a date written as three numbers in `order`, separated by `/`, `-`,
/// `.` or spaces (ECMA-376 Part 1, §18.17.4 date serials). Two-digit years
/// 00–29 are 20xx and 30–99 are 19xx (the same rule as typed dates).
pub fn parse_date_order(text: &str, order: DateOrder, date1904: bool) -> Option<f64> {
    let parts: Vec<&str> = text
        .trim()
        .split(['/', '-', '.', ' '])
        .filter(|p| !p.is_empty())
        .collect();
    if parts.len() != 3
        || parts
            .iter()
            .any(|p| p.len() > 4 || !p.bytes().all(|b| b.is_ascii_digit()))
    {
        return None;
    }
    let n = |i: usize| parts[i].parse::<u32>().ok();
    let (yi, mi, di) = match order {
        DateOrder::Dmy => (2, 1, 0),
        DateOrder::Mdy => (2, 0, 1),
        DateOrder::Ymd => (0, 1, 2),
        DateOrder::Dym => (1, 2, 0),
        DateOrder::Myd => (1, 0, 2),
        DateOrder::Ydm => (0, 2, 1),
    };
    let (y, m, d) = (n(yi)?, n(mi)?, n(di)?);
    if parts[mi].len() > 2 || parts[di].len() > 2 {
        return None;
    }
    let y = match parts[yi].len() {
        1 | 2 if y < 30 => 2000 + y,
        1 | 2 => 1900 + y,
        4 => y,
        _ => return None,
    };
    let y = i32::try_from(y).ok()?;
    if !(1..=12).contains(&m) || d == 0 || d > days_in_month(y, m) {
        return None;
    }
    datetime_to_serial(y, m, d, 0, 0, 0.0, date1904)
}

// ------------------------------------------------------------------ Remove Duplicates

/// Comparison key of a value: numbers by value, text without regard to case
/// (our choice: `"Hà Nội"` and `"HÀ NỘI"` are duplicates), blanks equal.
fn value_key(v: &CellValue) -> String {
    match v {
        CellValue::Empty => String::new(),
        CellValue::Number(n) => format!("n{n}"),
        CellValue::Bool(b) => format!("b{b}"),
        CellValue::Error(e) => format!("e{}", e.as_str()),
        CellValue::Date(d) => format!("d{d}"),
        other => format!("t{}", other.as_text().unwrap_or_default().to_lowercase()),
    }
}

/// Result of [`remove_duplicates`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DedupeResult {
    /// Rows removed.
    pub removed: u32,
    /// Data rows left (the header row not counted).
    pub remaining: u32,
}

/// Removes the rows of `range` whose values in `cols` (absolute columns; empty =
/// every column of the range) repeat an earlier row. The rows below move up
/// inside the range (cells outside it stay); formulas of moved cells are
/// translated through `adj` like a sort; the freed rows at the bottom are cleared.
pub fn remove_duplicates(
    ws: &mut Worksheet,
    range: CellRange,
    cols: &[u32],
    header: bool,
    adj: &dyn FormulaAdjuster,
) -> DedupeResult {
    let (c0, c1) = (range.first.col, range.last.col);
    let keys: Vec<u32> = if cols.is_empty() {
        (c0..=c1).collect()
    } else {
        cols.iter()
            .copied()
            .filter(|c| (c0..=c1).contains(c))
            .collect()
    };
    let start = range.first.row + u32::from(header);
    if start > range.last.row {
        return DedupeResult {
            removed: 0,
            remaining: 0,
        };
    }
    let total = range.last.row - start + 1;
    if keys.is_empty() {
        return DedupeResult {
            removed: 0,
            remaining: total,
        };
    }
    let mut seen = std::collections::HashSet::new();
    let mut kept = Vec::new();
    for r in start..=range.last.row {
        let key: Vec<String> = keys
            .iter()
            .map(|&c| {
                ws.cell(r, c)
                    .map_or(String::new(), |cell| value_key(&cell.value))
            })
            .collect();
        if seen.insert(key) {
            kept.push(r);
        }
    }
    let remaining = kept.len() as u32;
    if remaining == total {
        return DedupeResult {
            removed: 0,
            remaining,
        };
    }
    // Take the rows out, then put the kept ones back from `start` down.
    let mut lines: HashMap<u32, Vec<(u32, Cell)>> = HashMap::new();
    for r in start..=range.last.row {
        let ks: Vec<(u32, u32)> = ws.cells.range((r, c0)..=(r, c1)).map(|(&k, _)| k).collect();
        let line = ks
            .into_iter()
            .filter_map(|k| ws.cells.remove(&k).map(|cell| (k.1, cell)))
            .collect();
        lines.insert(r, line);
    }
    for (n, from) in kept.into_iter().enumerate() {
        let to = start + n as u32;
        let d = i64::from(to) - i64::from(from);
        for (c, mut cell) in lines.remove(&from).unwrap_or_default() {
            if let Some(f) = &mut cell.formula {
                if d != 0 && !f.text.is_empty() {
                    if let Some(t) = adj.translate(&f.text, d, 0) {
                        f.text = t;
                    }
                }
            }
            ws.cells.insert((to, c), cell);
        }
    }
    DedupeResult {
        removed: total - remaining,
        remaining,
    }
}

// ------------------------------------------------------------------ Consolidate

/// Summary function of Consolidate (the values of ECMA-376 Part 1, §18.18.17
/// `ST_DataConsolidateFunction`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ConsolidateFn {
    /// Sum of the numbers.
    #[default]
    Sum,
    /// Number of non-empty values.
    Count,
    /// Average of the numbers.
    Average,
    /// Largest number.
    Max,
    /// Smallest number.
    Min,
    /// Product of the numbers.
    Product,
    /// Number of numbers.
    CountNums,
    /// Sample standard deviation.
    StdDev,
    /// Population standard deviation.
    StdDevp,
    /// Sample variance.
    Var,
    /// Population variance.
    Varp,
}

impl ConsolidateFn {
    /// The worksheet function computing the same summary (§18.17.7), for linked output.
    pub fn function_name(self) -> &'static str {
        match self {
            ConsolidateFn::Sum => "SUM",
            ConsolidateFn::Count => "COUNTA",
            ConsolidateFn::Average => "AVERAGE",
            ConsolidateFn::Max => "MAX",
            ConsolidateFn::Min => "MIN",
            ConsolidateFn::Product => "PRODUCT",
            ConsolidateFn::CountNums => "COUNT",
            ConsolidateFn::StdDev => "STDEV",
            ConsolidateFn::StdDevp => "STDEVP",
            ConsolidateFn::Var => "VAR",
            ConsolidateFn::Varp => "VARP",
        }
    }

    /// The `SUBTOTAL` function number (1–11) of the same summary.
    pub fn subtotal_number(self) -> u8 {
        match self {
            ConsolidateFn::Average => 1,
            ConsolidateFn::CountNums => 2,
            ConsolidateFn::Count => 3,
            ConsolidateFn::Max => 4,
            ConsolidateFn::Min => 5,
            ConsolidateFn::Product => 6,
            ConsolidateFn::StdDev => 7,
            ConsolidateFn::StdDevp => 8,
            ConsolidateFn::Sum => 9,
            ConsolidateFn::Var => 10,
            ConsolidateFn::Varp => 11,
        }
    }

    /// The summary of `values` with the semantics of the worksheet function of the
    /// same name: numbers only (text and logical values in references are ignored),
    /// `COUNTA` counts every non-empty value.
    pub fn apply(self, values: &[&CellValue]) -> CellValue {
        use ConsolidateFn as F;
        let nums: Vec<f64> = values
            .iter()
            .filter_map(|v| match v {
                CellValue::Number(n) => Some(*n),
                _ => None,
            })
            .collect();
        if self != ConsolidateFn::Count && self != ConsolidateFn::CountNums {
            if let Some(CellValue::Error(e)) =
                values.iter().find(|v| matches!(v, CellValue::Error(_)))
            {
                return CellValue::Error(e.clone());
            }
        }
        let n = nums.len() as f64;
        let sum: f64 = nums.iter().sum();
        let ss = |mean: f64| nums.iter().map(|x| (x - mean).powi(2)).sum::<f64>();
        // MAX / MIN / PRODUCT of no numbers is 0; AVERAGE, STDEV, VAR … need enough
        // numbers, else #DIV/0! (ECMA-376 Part 1, §18.17.7).
        if nums.is_empty() && matches!(self, F::Max | F::Min | F::Product) {
            return CellValue::Number(0.0);
        }
        let too_few = match self {
            F::Average | F::StdDevp | F::Varp => nums.is_empty(),
            F::StdDev | F::Var => nums.len() < 2,
            _ => false,
        };
        if too_few {
            return CellValue::Error(CellError::Div0);
        }
        match self {
            F::Sum => CellValue::Number(sum),
            F::Count => CellValue::Number(
                values
                    .iter()
                    .filter(|v| match v {
                        CellValue::Empty => false,
                        CellValue::Text(t) => !t.is_empty(),
                        _ => true,
                    })
                    .count() as f64,
            ),
            F::CountNums => CellValue::Number(n),
            F::Average => CellValue::Number(sum / n),
            F::Max => CellValue::Number(nums.iter().copied().fold(f64::MIN, f64::max)),
            F::Min => CellValue::Number(nums.iter().copied().fold(f64::MAX, f64::min)),
            F::Product => CellValue::Number(nums.iter().product()),
            ConsolidateFn::StdDev => CellValue::Number((ss(sum / n) / (n - 1.0)).sqrt()),
            ConsolidateFn::Var => CellValue::Number(ss(sum / n) / (n - 1.0)),
            ConsolidateFn::StdDevp => CellValue::Number((ss(sum / n) / n).sqrt()),
            ConsolidateFn::Varp => CellValue::Number(ss(sum / n) / n),
        }
    }
}

/// One source range of Consolidate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConsolidateSource {
    /// 0-based sheet index.
    pub sheet: usize,
    /// The range, label row / column included.
    pub range: CellRange,
}

/// `(sheet, row, col)` of a source cell.
pub type SourceCell = (usize, u32, u32);

/// One output cell of Consolidate: its value and the source cells it summarises.
#[derive(Debug, Clone, PartialEq)]
pub struct ConsolidatedCell {
    /// The summary.
    pub value: CellValue,
    /// `(sheet, row, col)` of the contributing source cells, in source order.
    pub sources: Vec<(usize, u32, u32)>,
}

/// The table built by [`consolidate`]; labels are `None` when not used.
#[derive(Debug, Clone, PartialEq)]
pub struct Consolidated {
    /// Row labels (left column) in order of first appearance.
    pub row_labels: Option<Vec<CellValue>>,
    /// Column labels (top row) in order of first appearance.
    pub col_labels: Option<Vec<CellValue>>,
    /// Body: `cells[r][c]`, `None` where no source has a cell.
    pub cells: Vec<Vec<Option<ConsolidatedCell>>>,
}

/// Matching key of a label: its displayed text, trimmed, without regard to case.
fn label_key(wb: &Workbook, cell: Option<&Cell>, locale: &Locale) -> String {
    cell.map(|c| format_cell(wb, c, locale).text.trim().to_lowercase())
        .unwrap_or_default()
}

/// Consolidates `sources` with `func`. With `top_row` / `left_col` the first row /
/// column of each source holds labels and cells are matched by label (a source
/// row or column whose label is blank is skipped — our choice); otherwise by
/// position from the top-left data cell.
pub fn consolidate(
    wb: &Workbook,
    sources: &[ConsolidateSource],
    func: ConsolidateFn,
    top_row: bool,
    left_col: bool,
    locale: &Locale,
) -> Consolidated {
    let mut row_keys: Vec<String> = Vec::new();
    let mut col_keys: Vec<String> = Vec::new();
    let mut row_labels: Vec<CellValue> = Vec::new();
    let mut col_labels: Vec<CellValue> = Vec::new();
    // (row index, col index) → contributing cells.
    let mut acc: HashMap<(usize, usize), Vec<SourceCell>> = HashMap::new();
    let index_of =
        |keys: &mut Vec<String>, labels: &mut Vec<CellValue>, k: String, v: CellValue| {
            if let Some(i) = keys.iter().position(|x| *x == k) {
                i
            } else {
                keys.push(k);
                labels.push(v);
                keys.len() - 1
            }
        };
    for src in sources {
        let Some(ws) = wb.sheets.get(src.sheet) else {
            continue;
        };
        let r = src.range;
        let data_r0 = r.first.row + u32::from(top_row);
        let data_c0 = r.first.col + u32::from(left_col);
        if data_r0 > r.last.row || data_c0 > r.last.col {
            continue;
        }
        // Column index of each data column of this source.
        let mut cmap: Vec<Option<usize>> = Vec::new();
        for c in data_c0..=r.last.col {
            let i = if top_row {
                let cell = ws.cell(r.first.row, c);
                let k = label_key(wb, cell, locale);
                if k.is_empty() {
                    None
                } else {
                    let v = cell.map(|x| x.value.clone()).unwrap_or_default();
                    Some(index_of(&mut col_keys, &mut col_labels, k, v))
                }
            } else {
                Some((c - data_c0) as usize)
            };
            cmap.push(i);
        }
        for row in data_r0..=r.last.row {
            let ri = if left_col {
                let cell = ws.cell(row, r.first.col);
                let k = label_key(wb, cell, locale);
                if k.is_empty() {
                    continue;
                }
                let v = cell.map(|x| x.value.clone()).unwrap_or_default();
                index_of(&mut row_keys, &mut row_labels, k, v)
            } else {
                (row - data_r0) as usize
            };
            for (j, c) in (data_c0..=r.last.col).enumerate() {
                let Some(ci) = cmap[j] else { continue };
                if ws.cell(row, c).is_some() {
                    acc.entry((ri, ci)).or_default().push((src.sheet, row, c));
                } else {
                    // An empty source cell still makes the output cell exist.
                    acc.entry((ri, ci)).or_default();
                }
            }
        }
    }
    let rows = if left_col {
        row_keys.len()
    } else {
        acc.keys().map(|k| k.0 + 1).max().unwrap_or(0)
    };
    let cols = if top_row {
        col_keys.len()
    } else {
        acc.keys().map(|k| k.1 + 1).max().unwrap_or(0)
    };
    let empty = CellValue::Empty;
    let cells = (0..rows)
        .map(|ri| {
            (0..cols)
                .map(|ci| {
                    let src = acc.get(&(ri, ci))?;
                    let values: Vec<&CellValue> = src
                        .iter()
                        .map(|&(s, r, c)| {
                            wb.sheets[s].cell(r, c).map_or(&empty, |cell| &cell.value)
                        })
                        .collect();
                    Some(ConsolidatedCell {
                        value: func.apply(&values),
                        sources: src.clone(),
                    })
                })
                .collect()
        })
        .collect();
    Consolidated {
        row_labels: left_col.then_some(row_labels),
        col_labels: top_row.then_some(col_labels),
        cells,
    }
}

// ------------------------------------------------------------------ Circle invalid data

/// Cells of sheet `sheet` holding a value that violates their data validation rule
/// (§18.3.1.32), as `(row, col)` in row order; at most `limit` cells. Empty cells
/// are not listed (our choice: there is nothing to circle).
pub fn invalid_cells(
    wb: &Workbook,
    sheet: usize,
    locale: &Locale,
    now: Option<f64>,
    limit: usize,
) -> Vec<(u32, u32)> {
    let Some(ws) = wb.sheets.get(sheet) else {
        return Vec::new();
    };
    let mut out = std::collections::BTreeSet::new();
    for dv in &ws.data_validations {
        if dv.kind.as_deref().unwrap_or("none") == "none" {
            continue;
        }
        for r in &dv.sqref {
            for row in r.first.row..=r.last.row {
                for (&(rr, cc), cell) in ws.cells.range((row, r.first.col)..=(row, r.last.col)) {
                    let blank = match &cell.value {
                        CellValue::Empty => true,
                        CellValue::Text(t) => t.is_empty(),
                        _ => false,
                    };
                    if blank || out.contains(&(rr, cc)) {
                        continue;
                    }
                    if crate::validation::check(wb, sheet, rr, cc, &cell.value, locale, now)
                        .is_err()
                    {
                        out.insert((rr, cc));
                    }
                }
                if out.len() >= limit {
                    return out.into_iter().take(limit).collect();
                }
            }
        }
    }
    out.into_iter().collect()
}

#[cfg(test)]
mod tests;
