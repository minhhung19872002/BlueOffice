//! Show Changes for workbooks (CORE-VERSION-001): a cell-level comparison of
//! two workbooks, sheet by sheet.
//!
//! The comparison is our own design (no file-format spec covers it):
//!
//! * sheets are paired by name, then — for sheets left over — by their stable
//!   `sheet/@sheetId` (ECMA-376 Part 1, §18.2.19), which reports a rename;
//!   sheets still unpaired were added or removed;
//! * columns are aligned first, by the multiset of the contents of each column
//!   (insensitive to rows inserted elsewhere), then rows by the contents of
//!   each row in the aligned columns ([`align`]: common prefix / suffix,
//!   longest common subsequence of row signatures, patience anchors for long
//!   sequences, and similarity pairing inside the gaps). A row or column
//!   inserted or deleted is one change, not a change of every cell after it;
//! * formulas are compared after the old formula's references were moved by
//!   the inserted / deleted rows and columns (`bo_formula::adjust_for_change`),
//!   so `=B5*2` that became `=B6*2` because a row was inserted above is not a
//!   change. Shared-formula followers (§18.3.1.40 `f` `t="shared"`) are
//!   expanded from their master; a formula cell's cached value is not compared
//!   (it follows its inputs);
//! * formatting is compared through the resolved cell format (§18.8.10
//!   `cellXfs`: number format code, font, fill, border, alignment, protection),
//!   so two workbooks numbering their styles differently compare equal.

use std::collections::{BTreeMap, HashMap};
use std::hash::{Hash, Hasher};

use bo_formula::{adjust_for_change, format, offset_relative, parse, Expr, StructuralChange};
use bo_model::sheet::{
    col_to_letters, Cell, CellValue, FormulaKind, Stylesheet, Workbook, Worksheet,
};
use serde::Serialize;

/// What changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ChangeKind {
    /// A sheet exists only in the new workbook.
    SheetAdded,
    /// A sheet exists only in the old workbook.
    SheetRemoved,
    /// Same sheet (by `sheetId`), new name.
    SheetRenamed,
    /// Rows inserted (`row`, `count` in the new sheet).
    RowsInserted,
    /// Rows deleted (`old_address` in the old sheet; `row` = where they were in the new one).
    RowsDeleted,
    /// Columns inserted (`col`, `count` in the new sheet).
    ColsInserted,
    /// Columns deleted.
    ColsDeleted,
    /// An empty cell got a value or formula.
    CellAdded,
    /// A cell's value or formula was removed.
    CellCleared,
    /// A constant value changed.
    ValueChanged,
    /// A formula changed (or a constant became a formula, or the reverse).
    FormulaChanged,
    /// Same content, different formatting.
    FormatChanged,
}

/// One difference between two workbooks.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SheetChange {
    /// Kind of change.
    pub kind: ChangeKind,
    /// Sheet name in the new workbook (the old name for a removed sheet).
    pub sheet: String,
    /// Index of the sheet in the new workbook (`None`: removed).
    pub sheet_index: Option<usize>,
    /// Old sheet name (renamed sheets).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old_sheet: Option<String>,
    /// Address in the new sheet: `B4`, `5:7` (rows), `C:D` (columns); empty for sheets.
    pub address: String,
    /// Address in the old sheet when it differs from `address` (moved cells, deleted rows).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old_address: Option<String>,
    /// Zero-based row in the new sheet (the first row of a range).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub row: Option<u32>,
    /// Zero-based column in the new sheet.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub col: Option<u32>,
    /// Number of rows / columns inserted or deleted.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub count: Option<u32>,
    /// Old content (`=` + formula, or the value as text).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old: Option<String>,
    /// New content.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new: Option<String>,
}

impl SheetChange {
    fn sheet_level(kind: ChangeKind, sheet: &str, index: Option<usize>) -> Self {
        SheetChange {
            kind,
            sheet: sheet.to_owned(),
            sheet_index: index,
            old_sheet: None,
            address: String::new(),
            old_address: None,
            row: None,
            col: None,
            count: None,
            old: None,
            new: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Sequence alignment
// ---------------------------------------------------------------------------

/// One step of an alignment of an old and a new sequence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// Old item `.0` corresponds to new item `.1`.
    Both(usize, usize),
    /// Old item only (deleted).
    Old(usize),
    /// New item only (inserted).
    New(usize),
}

/// Largest `old × new` table filled by the quadratic passes.
const DP_LIMIT: usize = 2_000_000;
/// Smallest similarity at which two different items of a gap are paired.
const PAIR_THRESHOLD: f64 = 0.5;

/// Aligns `old` and `new` (items given by a signature hash): items with equal
/// signatures are matched along a longest common subsequence (patience anchors
/// first when the table would be too large); in each gap left between matches,
/// equally many old and new items are paired in order (edited in place), and
/// otherwise the pairs with `sim ≥ 0.5` maximising the total similarity are
/// kept and the rest are deletions / insertions. Steps come in sequence order.
pub fn align(old: &[u64], new: &[u64], sim: &dyn Fn(usize, usize) -> f64) -> Vec<Step> {
    let mut out = Vec::with_capacity(old.len().max(new.len()));
    align_range(old, new, 0..old.len(), 0..new.len(), sim, &mut out);
    out
}

fn align_range(
    old: &[u64],
    new: &[u64],
    a: std::ops::Range<usize>,
    b: std::ops::Range<usize>,
    sim: &dyn Fn(usize, usize) -> f64,
    out: &mut Vec<Step>,
) {
    let (mut a0, mut a1, mut b0, mut b1) = (a.start, a.end, b.start, b.end);
    // Common prefix.
    while a0 < a1 && b0 < b1 && old[a0] == new[b0] {
        out.push(Step::Both(a0, b0));
        a0 += 1;
        b0 += 1;
    }
    // Common suffix (pushed at the end).
    let mut suffix = 0;
    while a0 < a1 && b0 < b1 && old[a1 - 1] == new[b1 - 1] {
        a1 -= 1;
        b1 -= 1;
        suffix += 1;
    }
    let (la, lb) = (a1 - a0, b1 - b0);
    if la > 0 && lb > 0 {
        let anchors = if la.saturating_mul(lb) <= DP_LIMIT {
            lcs(old, new, a0..a1, b0..b1)
        } else {
            patience(old, new, a0..a1, b0..b1)
        };
        let recurse = la.saturating_mul(lb) > DP_LIMIT;
        let (mut i, mut j) = (a0, b0);
        for &(ai, bj) in anchors.iter().chain(std::iter::once(&(a1, b1))) {
            if recurse && !anchors.is_empty() && (ai > i || bj > j) {
                align_range(old, new, i..ai, j..bj, sim, out);
            } else {
                fill_gap(i..ai, j..bj, sim, out);
            }
            if ai < a1 {
                out.push(Step::Both(ai, bj));
            }
            i = ai + 1;
            j = bj + 1;
        }
    } else {
        fill_gap(a0..a1, b0..b1, sim, out);
    }
    for k in 0..suffix {
        out.push(Step::Both(a1 + k, b1 + k));
    }
}

/// Longest common subsequence of equal signatures (classic table).
fn lcs(
    old: &[u64],
    new: &[u64],
    a: std::ops::Range<usize>,
    b: std::ops::Range<usize>,
) -> Vec<(usize, usize)> {
    let (la, lb) = (a.len(), b.len());
    let w = lb + 1;
    let mut t = vec![0u32; (la + 1) * w];
    for i in (0..la).rev() {
        for j in (0..lb).rev() {
            t[i * w + j] = if old[a.start + i] == new[b.start + j] {
                t[(i + 1) * w + j + 1] + 1
            } else {
                t[(i + 1) * w + j].max(t[i * w + j + 1])
            };
        }
    }
    let (mut i, mut j) = (0, 0);
    let mut out = Vec::new();
    while i < la && j < lb {
        if old[a.start + i] == new[b.start + j] {
            out.push((a.start + i, b.start + j));
            i += 1;
            j += 1;
        } else if t[(i + 1) * w + j] >= t[i * w + j + 1] {
            i += 1;
        } else {
            j += 1;
        }
    }
    out
}

/// Anchors of patience diff: signatures occurring exactly once on each side,
/// kept along the longest increasing run of their new positions.
fn patience(
    old: &[u64],
    new: &[u64],
    a: std::ops::Range<usize>,
    b: std::ops::Range<usize>,
) -> Vec<(usize, usize)> {
    let mut count: HashMap<u64, (u32, u32, usize, usize)> = HashMap::new();
    for i in a.clone() {
        let e = count.entry(old[i]).or_insert((0, 0, i, 0));
        e.0 += 1;
    }
    for j in b {
        if let Some(e) = count.get_mut(&new[j]) {
            e.1 += 1;
            e.3 = j;
        }
    }
    let mut pairs: Vec<(usize, usize)> = count
        .values()
        .filter(|e| e.0 == 1 && e.1 == 1)
        .map(|e| (e.2, e.3))
        .collect();
    pairs.sort_unstable();
    // Longest increasing subsequence of the new positions (patience sorting).
    let mut tails: Vec<usize> = Vec::new();
    let mut prev = vec![usize::MAX; pairs.len()];
    for k in 0..pairs.len() {
        let pos = tails.partition_point(|&t| pairs[t].1 < pairs[k].1);
        if pos > 0 {
            prev[k] = tails[pos - 1];
        }
        if pos == tails.len() {
            tails.push(k);
        } else {
            tails[pos] = k;
        }
    }
    let mut out = Vec::new();
    let mut k = tails.last().copied().unwrap_or(usize::MAX);
    while k != usize::MAX {
        out.push(pairs[k]);
        k = prev[k];
    }
    out.reverse();
    out
}

/// Pairs the items of a gap between two matches.
fn fill_gap(
    a: std::ops::Range<usize>,
    b: std::ops::Range<usize>,
    sim: &dyn Fn(usize, usize) -> f64,
    out: &mut Vec<Step>,
) {
    let (la, lb) = (a.len(), b.len());
    if la == lb {
        out.extend(a.zip(b).map(|(i, j)| Step::Both(i, j)));
        return;
    }
    if la == 0 || lb == 0 || la.saturating_mul(lb) > DP_LIMIT {
        let n = la.min(lb);
        out.extend(
            a.clone()
                .zip(b.clone())
                .take(n)
                .map(|(i, j)| Step::Both(i, j)),
        );
        out.extend(a.skip(n).map(Step::Old));
        out.extend(b.skip(n).map(Step::New));
        return;
    }
    // Weighted alignment: best total similarity of the kept pairs.
    let w = lb + 1;
    let mut t = vec![0f64; (la + 1) * w];
    let s = |i: usize, j: usize| {
        let v = sim(a.start + i, b.start + j);
        if v >= PAIR_THRESHOLD {
            v
        } else {
            f64::NEG_INFINITY
        }
    };
    for i in (0..la).rev() {
        for j in (0..lb).rev() {
            let pair = s(i, j) + t[(i + 1) * w + j + 1];
            t[i * w + j] = pair.max(t[(i + 1) * w + j]).max(t[i * w + j + 1]);
        }
    }
    let (mut i, mut j) = (0, 0);
    while i < la && j < lb {
        let here = t[i * w + j];
        let pair = s(i, j) + t[(i + 1) * w + j + 1];
        if pair.is_finite() && (pair - here).abs() < 1e-9 {
            out.push(Step::Both(a.start + i, b.start + j));
            i += 1;
            j += 1;
        } else if (t[(i + 1) * w + j] - here).abs() < 1e-9 {
            out.push(Step::Old(a.start + i));
            i += 1;
        } else {
            out.push(Step::New(b.start + j));
            j += 1;
        }
    }
    out.extend((a.start + i..a.end).map(Step::Old));
    out.extend((b.start + j..b.end).map(Step::New));
}

fn hash_of<T: Hash + ?Sized>(v: &T) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    v.hash(&mut h);
    h.finish()
}

/// Similarity of two sorted multisets: common items / larger size (1 when both empty).
fn multiset_sim<T: Ord>(a: &[T], b: &[T]) -> f64 {
    let max = a.len().max(b.len());
    if max == 0 {
        return 1.0;
    }
    let (mut i, mut j, mut common) = (0, 0, 0usize);
    while i < a.len() && j < b.len() {
        match a[i].cmp(&b[j]) {
            std::cmp::Ordering::Equal => {
                common += 1;
                i += 1;
                j += 1;
            }
            std::cmp::Ordering::Less => i += 1,
            std::cmp::Ordering::Greater => j += 1,
        }
    }
    common as f64 / max as f64
}

// ---------------------------------------------------------------------------
// Cell contents
// ---------------------------------------------------------------------------

/// Content of one cell as compared.
#[derive(Debug, Clone, PartialEq)]
struct Content {
    /// Formula text (shared followers expanded), without `=`.
    formula: Option<String>,
    /// Constant value as text (`None` = empty).
    value: Option<String>,
    /// Resolved formatting.
    format: String,
}

impl Content {
    fn is_empty(&self) -> bool {
        self.formula.is_none() && self.value.is_none()
    }

    fn shown(&self) -> Option<String> {
        self.formula
            .as_ref()
            .map(|f| format!("={f}"))
            .or_else(|| self.value.clone())
    }

    /// Signature used to align rows / columns: formulas with every reference
    /// blanked (moved formulas still match), constants as they are.
    fn signature(&self) -> String {
        match &self.formula {
            Some(f) => match parse(f) {
                Ok(e) => {
                    let blank = e.map(&mut |x| match x {
                        Expr::Ref(_) => Expr::Name {
                            sheet: None,
                            name: "_R".into(),
                        },
                        other => other,
                    });
                    format!("f:{}", format(&blank))
                }
                Err(_) => format!("f:{f}"),
            },
            None => format!("v:{}", self.value.as_deref().unwrap_or("")),
        }
    }
}

/// Text of a constant value (`None` for an empty cell).
fn value_text(v: &CellValue) -> Option<String> {
    match v {
        CellValue::Empty => None,
        CellValue::Number(n) => Some(number_text(*n)),
        CellValue::Text(_) | CellValue::RichText(_) => v.as_text(),
        CellValue::Bool(b) => Some(if *b { "TRUE" } else { "FALSE" }.to_owned()),
        CellValue::Error(e) => Some(e.as_str().to_owned()),
        CellValue::Date(d) => Some(d.clone()),
    }
}

/// Shortest text of a number (no trailing `.0`).
fn number_text(n: f64) -> String {
    if n.fract() == 0.0 && n.abs() < 1e15 {
        format!("{}", n as i64)
    } else {
        format!("{n}")
    }
}

/// Resolved format of a `cellXfs` record (ids replaced by what they point to).
fn format_key(styles: &Stylesheet, style: u32) -> String {
    let Some(xf) = styles.cell_xfs.get(style as usize) else {
        return String::new();
    };
    let code = styles
        .num_fmts
        .iter()
        .find(|f| f.id == xf.num_fmt_id)
        .map_or_else(|| format!("#{}", xf.num_fmt_id), |f| f.code.clone());
    // `General` (id 0) and an absent record read the same.
    let code = if code == "#0" { String::new() } else { code };
    format!(
        "{code}|{:?}|{:?}|{:?}|{:?}|{:?}",
        styles.fonts.get(xf.font_id as usize),
        styles.fills.get(xf.fill_id as usize),
        styles.borders.get(xf.border_id as usize),
        xf.alignment,
        xf.protection,
    )
}

/// The cells of one worksheet as compared.
struct Grid {
    cells: BTreeMap<(u32, u32), Content>,
    /// Format of a cell without a record (`cellXfs` 0).
    default_format: String,
    /// One past the last used row / column.
    rows: usize,
    cols: usize,
}

impl Grid {
    fn new(ws: &Worksheet, styles: &Stylesheet) -> Grid {
        let default_format = format_key(styles, 0);
        // Masters of shared formulas (§18.3.1.40): index → (row, col, parsed text).
        let mut masters: HashMap<u32, (u32, u32, Expr)> = HashMap::new();
        for (&(r, c), cell) in &ws.cells {
            if let Some(f) = &cell.formula {
                if let FormulaKind::Shared { index, .. } = f.kind {
                    if !f.text.is_empty() {
                        if let Ok(e) = parse(&f.text) {
                            masters.entry(index).or_insert((r, c, e));
                        }
                    }
                }
            }
        }
        let mut cells = BTreeMap::new();
        let (mut rows, mut cols) = (0usize, 0usize);
        for (&(r, c), cell) in &ws.cells {
            let content = content_of(cell, r, c, &masters, styles);
            if content.is_empty() && content.format == default_format {
                continue;
            }
            rows = rows.max(r as usize + 1);
            cols = cols.max(c as usize + 1);
            cells.insert((r, c), content);
        }
        Grid {
            cells,
            default_format,
            rows,
            cols,
        }
    }
}

fn content_of(
    cell: &Cell,
    r: u32,
    c: u32,
    masters: &HashMap<u32, (u32, u32, Expr)>,
    styles: &Stylesheet,
) -> Content {
    let formula = cell.formula.as_ref().and_then(|f| {
        if !f.text.is_empty() {
            return Some(f.text.clone());
        }
        let FormulaKind::Shared { index, .. } = f.kind else {
            return None;
        };
        let (mr, mc, e) = masters.get(&index)?;
        Some(format(&offset_relative(
            e,
            i64::from(r) - i64::from(*mr),
            i64::from(c) - i64::from(*mc),
        )))
    });
    Content {
        value: if formula.is_some() {
            None
        } else {
            value_text(&cell.value)
        },
        formula,
        format: format_key(styles, cell.style),
    }
}

/// Alignment of one pair of sheets.
struct SheetAlign {
    rows: Vec<Step>,
    cols: Vec<Step>,
}

/// Aligns the columns, then the rows, of two grids.
fn align_grids(old: &Grid, new: &Grid) -> SheetAlign {
    // Columns: sorted multisets of cell signatures.
    let col_sets = |g: &Grid| {
        let mut sets: Vec<Vec<String>> = vec![Vec::new(); g.cols];
        for (&(_, c), content) in &g.cells {
            if !content.is_empty() {
                sets[c as usize].push(content.signature());
            }
        }
        for s in &mut sets {
            s.sort_unstable();
        }
        sets
    };
    let (oc, nc) = (col_sets(old), col_sets(new));
    let oh: Vec<u64> = oc.iter().map(hash_of).collect();
    let nh: Vec<u64> = nc.iter().map(hash_of).collect();
    let cols = pair_tail(align(&oh, &nh, &|i, j| multiset_sim(&oc[i], &nc[j])));
    // Old column → new column of the aligned pairs.
    let mut col_map: HashMap<u32, u32> = HashMap::new();
    for s in &cols {
        if let Step::Both(i, j) = *s {
            col_map.insert(i as u32, j as u32);
        }
    }
    // Rows: signatures of the cells in aligned columns, keyed by the new column.
    let row_sigs = |g: &Grid, map: Option<&HashMap<u32, u32>>| {
        let mut rows: Vec<Vec<(u32, String)>> = vec![Vec::new(); g.rows];
        for (&(r, c), content) in &g.cells {
            if content.is_empty() {
                continue;
            }
            let key = match map {
                Some(m) => match m.get(&c) {
                    Some(&k) => k,
                    None => continue,
                },
                None => c,
            };
            rows[r as usize].push((key, content.signature()));
        }
        for r in &mut rows {
            r.sort_unstable();
        }
        rows
    };
    let paired_new: std::collections::HashSet<u32> = col_map.values().copied().collect();
    let or = row_sigs(old, Some(&col_map));
    let mut nr = row_sigs(new, None);
    for r in &mut nr {
        r.retain(|(c, _)| paired_new.contains(c));
    }
    let oh: Vec<u64> = or.iter().map(hash_of).collect();
    let nh: Vec<u64> = nr.iter().map(hash_of).collect();
    let rows = pair_tail(align(&oh, &nh, &|i, j| multiset_sim(&or[i], &nr[j])));
    SheetAlign { rows, cols }
}

/// Rows (columns) only after the end of the other side were typed into empty
/// rows, not inserted: a trailing run of insertions (or of deletions) is
/// paired with the empty rows past the other side's end.
fn pair_tail(mut steps: Vec<Step>) -> Vec<Step> {
    let start = steps
        .iter()
        .rposition(|s| matches!(s, Step::Both(..)))
        .map_or(0, |k| k + 1);
    let (mut i, mut j) = match start.checked_sub(1).map(|k| steps[k]) {
        Some(Step::Both(i, j)) => (i + 1, j + 1),
        _ => (0, 0),
    };
    let tail = &mut steps[start..];
    if tail.iter().all(|s| matches!(s, Step::New(_))) {
        for s in tail {
            if let Step::New(n) = *s {
                *s = Step::Both(i, n);
                i += 1;
            }
        }
    } else if tail.iter().all(|s| matches!(s, Step::Old(_))) {
        for s in tail {
            if let Step::Old(o) = *s {
                *s = Step::Both(o, j);
                j += 1;
            }
        }
    }
    steps
}

/// Runs of consecutive unpaired steps: `(first index, count)`; `at` = the
/// index on the other side where the run sits.
fn runs(steps: &[Step], old_side: bool) -> Vec<(u32, u32, u32)> {
    let mut out: Vec<(u32, u32, u32)> = Vec::new();
    let mut other = 0u32;
    for s in steps {
        match (*s, old_side) {
            (Step::Old(i), true) | (Step::New(i), false) => {
                let i = i as u32;
                match out.last_mut() {
                    Some(last) if last.0 + last.1 == i && last.2 == other => last.1 += 1,
                    _ => out.push((i, 1, other)),
                }
            }
            (Step::Both(i, j), _) => other = if old_side { j as u32 + 1 } else { i as u32 + 1 },
            _ => {}
        }
    }
    out
}

/// Structural changes that turn old coordinates into new ones: deletions from
/// the bottom up, then insertions from the top down.
fn structural(steps: &[Step], rows: bool) -> Vec<StructuralChange> {
    let mut out = Vec::new();
    for (at, count, _) in runs(steps, true).into_iter().rev() {
        out.push(if rows {
            StructuralChange::DeleteRows { at, count }
        } else {
            StructuralChange::DeleteCols { at, count }
        });
    }
    for (at, count, _) in runs(steps, false) {
        out.push(if rows {
            StructuralChange::InsertRows { at, count }
        } else {
            StructuralChange::InsertCols { at, count }
        });
    }
    out
}

fn rows_label(first: u32, count: u32) -> String {
    if count == 1 {
        format!("{}", first + 1)
    } else {
        format!("{}:{}", first + 1, first + count)
    }
}

fn cols_label(first: u32, count: u32) -> String {
    if count == 1 {
        col_to_letters(first)
    } else {
        format!(
            "{}:{}",
            col_to_letters(first),
            col_to_letters(first + count - 1)
        )
    }
}

fn a1(row: u32, col: u32) -> String {
    format!("{}{}", col_to_letters(col), row + 1)
}

/// Normalised formula text (`None` when it does not parse).
fn normalised(text: &str, moves: &[(String, Vec<StructuralChange>)], home: &str) -> String {
    match parse(text) {
        Ok(mut e) => {
            for (sheet, changes) in moves {
                for ch in changes {
                    e = adjust_for_change(&e, *ch, home, sheet);
                }
            }
            format(&e)
        }
        Err(_) => text.to_owned(),
    }
}

/// Differences between `old` and `new`, sheet by sheet (see the module docs).
pub fn diff_workbooks(old: &Workbook, new: &Workbook) -> Vec<SheetChange> {
    let mut out = Vec::new();
    // Pair the sheets: by name, then by a sheetId unique on both sides.
    let mut pair: Vec<Option<usize>> = vec![None; new.sheets.len()];
    let mut used = vec![false; old.sheets.len()];
    for (ni, ns) in new.sheets.iter().enumerate() {
        if let Some(oi) = old
            .sheets
            .iter()
            .position(|os| os.name.to_lowercase() == ns.name.to_lowercase())
        {
            pair[ni] = Some(oi);
            used[oi] = true;
        }
    }
    let unique =
        |wb: &Workbook, id: u32| wb.sheets.iter().filter(|s| s.sheet_id == id).count() == 1;
    for (ni, ns) in new.sheets.iter().enumerate() {
        if pair[ni].is_some() || !unique(new, ns.sheet_id) {
            continue;
        }
        if let Some(oi) = old
            .sheets
            .iter()
            .position(|os| os.sheet_id == ns.sheet_id)
            .filter(|&oi| !used[oi] && unique(old, ns.sheet_id))
        {
            pair[ni] = Some(oi);
            used[oi] = true;
            out.push(SheetChange {
                old_sheet: Some(old.sheets[oi].name.clone()),
                ..SheetChange::sheet_level(ChangeKind::SheetRenamed, &ns.name, Some(ni))
            });
        }
    }
    for (oi, os) in old.sheets.iter().enumerate() {
        if !used[oi] {
            out.push(SheetChange::sheet_level(
                ChangeKind::SheetRemoved,
                &os.name,
                None,
            ));
        }
    }
    for (ni, ns) in new.sheets.iter().enumerate() {
        if pair[ni].is_none() {
            out.push(SheetChange::sheet_level(
                ChangeKind::SheetAdded,
                &ns.name,
                Some(ni),
            ));
        }
    }
    // Align every pair of sheets first: formulas on any sheet may point into any other.
    let pairs: Vec<(usize, usize, Grid, Grid, SheetAlign)> = pair
        .iter()
        .enumerate()
        .filter_map(|(ni, oi)| oi.map(|oi| (oi, ni)))
        .map(|(oi, ni)| {
            let og = Grid::new(&old.sheets[oi], &old.styles);
            let ng = Grid::new(&new.sheets[ni], &new.styles);
            let al = align_grids(&og, &ng);
            (oi, ni, og, ng, al)
        })
        .collect();
    let moves: Vec<(String, Vec<StructuralChange>)> = pairs
        .iter()
        .map(|(oi, _, _, _, al)| {
            let mut ch = structural(&al.rows, true);
            ch.extend(structural(&al.cols, false));
            (old.sheets[*oi].name.clone(), ch)
        })
        .collect();
    for (oi, ni, og, ng, al) in &pairs {
        diff_sheet(
            &old.sheets[*oi].name,
            &new.sheets[*ni].name,
            *ni,
            og,
            ng,
            al,
            &moves,
            &mut out,
        );
    }
    out
}

#[allow(clippy::too_many_arguments)]
fn diff_sheet(
    old_name: &str,
    name: &str,
    index: usize,
    og: &Grid,
    ng: &Grid,
    al: &SheetAlign,
    moves: &[(String, Vec<StructuralChange>)],
    out: &mut Vec<SheetChange>,
) {
    let base = SheetChange::sheet_level(ChangeKind::CellAdded, name, Some(index));
    for (at, count, _) in runs(&al.rows, false) {
        out.push(SheetChange {
            kind: ChangeKind::RowsInserted,
            address: rows_label(at, count),
            row: Some(at),
            count: Some(count),
            ..base.clone()
        });
    }
    for (at, count, other) in runs(&al.rows, true) {
        out.push(SheetChange {
            kind: ChangeKind::RowsDeleted,
            address: rows_label(other, 1),
            old_address: Some(rows_label(at, count)),
            row: Some(other),
            count: Some(count),
            ..base.clone()
        });
    }
    for (at, count, _) in runs(&al.cols, false) {
        out.push(SheetChange {
            kind: ChangeKind::ColsInserted,
            address: cols_label(at, count),
            col: Some(at),
            count: Some(count),
            ..base.clone()
        });
    }
    for (at, count, other) in runs(&al.cols, true) {
        out.push(SheetChange {
            kind: ChangeKind::ColsDeleted,
            address: col_to_letters(other),
            old_address: Some(cols_label(at, count)),
            col: Some(other),
            count: Some(count),
            ..base.clone()
        });
    }
    let row_pairs: Vec<(u32, u32)> = al
        .rows
        .iter()
        .filter_map(|s| match *s {
            Step::Both(i, j) => Some((i as u32, j as u32)),
            _ => None,
        })
        .collect();
    let col_pairs: Vec<(u32, u32)> = al
        .cols
        .iter()
        .filter_map(|s| match *s {
            Step::Both(i, j) => Some((i as u32, j as u32)),
            _ => None,
        })
        .collect();
    let old_row: HashMap<u32, u32> = row_pairs.iter().map(|&(o, n)| (n, o)).collect();
    let old_col: HashMap<u32, u32> = col_pairs.iter().map(|&(o, n)| (n, o)).collect();
    let new_row: HashMap<u32, u32> = row_pairs.iter().copied().collect();
    let new_col: HashMap<u32, u32> = col_pairs.iter().copied().collect();
    // Every position holding a cell on either side, in new coordinates.
    let mut keys: std::collections::BTreeSet<(u32, u32)> = ng.cells.keys().copied().collect();
    for &(r, c) in og.cells.keys() {
        if let (Some(&nr), Some(&nc)) = (new_row.get(&r), new_col.get(&c)) {
            keys.insert((nr, nc));
        }
    }
    let empty = Content {
        formula: None,
        value: None,
        format: String::new(),
    };
    for (r, c) in keys {
        // Cells of inserted rows / columns are covered by that change.
        let (Some(&or), Some(&oc)) = (old_row.get(&r), old_col.get(&c)) else {
            continue;
        };
        let o = og.cells.get(&(or, oc));
        let n = ng.cells.get(&(r, c));
        let (o_c, n_c) = (o.unwrap_or(&empty), n.unwrap_or(&empty));
        let kind = if o_c.is_empty() && !n_c.is_empty() {
            Some(ChangeKind::CellAdded)
        } else if !o_c.is_empty() && n_c.is_empty() {
            Some(ChangeKind::CellCleared)
        } else if o_c.formula.is_some() || n_c.formula.is_some() {
            let same = match (&o_c.formula, &n_c.formula) {
                (Some(a), Some(b)) => normalised(a, moves, old_name) == normalised(b, &[], name),
                _ => false,
            };
            (!same).then_some(ChangeKind::FormulaChanged)
        } else if o_c.value != n_c.value {
            Some(ChangeKind::ValueChanged)
        } else {
            None
        };
        let kind = kind.or_else(|| {
            // An absent cell has the default format of its workbook.
            let fo = o.map_or(og.default_format.as_str(), |x| x.format.as_str());
            let fnew = n.map_or(ng.default_format.as_str(), |x| x.format.as_str());
            (fo != fnew).then_some(ChangeKind::FormatChanged)
        });
        let Some(kind) = kind else { continue };
        let (old_addr, new_addr) = (a1(or, oc), a1(r, c));
        out.push(SheetChange {
            kind,
            address: new_addr.clone(),
            old_address: (old_addr != new_addr).then_some(old_addr),
            row: Some(r),
            col: Some(c),
            old: o_c.shown(),
            new: n_c.shown(),
            ..base.clone()
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bo_model::sheet::{Cell, CellXf, Font, Formula};

    fn sheet(name: &str, id: u32, cells: &[(&str, &str)]) -> Worksheet {
        let mut ws = Worksheet::new(name);
        ws.sheet_id = id;
        for (a, v) in cells {
            let r = bo_model::sheet::CellRef::parse(a).expect("ref");
            let cell = if let Some(f) = v.strip_prefix('=') {
                Cell {
                    formula: Some(Formula::normal(f)),
                    ..Cell::default()
                }
            } else if let Ok(n) = v.parse::<f64>() {
                Cell::new(CellValue::Number(n))
            } else {
                Cell::new(CellValue::Text((*v).to_owned()))
            };
            ws.cells.insert((r.row, r.col), cell);
        }
        ws
    }

    fn book(sheets: Vec<Worksheet>) -> Workbook {
        Workbook {
            sheets,
            ..Workbook::new()
        }
    }

    fn summary(ch: &[SheetChange]) -> Vec<String> {
        ch.iter()
            .map(|c| {
                format!(
                    "{:?} {}!{}{}{}",
                    c.kind,
                    c.sheet,
                    c.address,
                    c.old.as_ref().map(|o| format!(" {o}")).unwrap_or_default(),
                    c.new
                        .as_ref()
                        .map(|n| format!(" -> {n}"))
                        .unwrap_or_default(),
                )
            })
            .collect()
    }

    /// A small table: header and four data rows with a total formula per row.
    fn table() -> Vec<(&'static str, &'static str)> {
        vec![
            ("A1", "Item"),
            ("B1", "Qty"),
            ("C1", "Total"),
            ("A2", "Pen"),
            ("B2", "3"),
            ("C2", "=B2*2"),
            ("A3", "Ink"),
            ("B3", "5"),
            ("C3", "=B3*2"),
            ("A4", "Pad"),
            ("B4", "7"),
            ("C4", "=B4*2"),
            ("A5", "Tag"),
            ("B5", "9"),
            ("C5", "=B5*2"),
            ("B6", "=SUM(B2:B5)"),
        ]
    }

    #[test]
    fn identical_workbooks_have_no_changes() {
        let a = book(vec![sheet("Data", 1, &table())]);
        assert_eq!(diff_workbooks(&a, &a.clone()), Vec::<SheetChange>::new());
    }

    #[test]
    fn value_formula_added_and_cleared_cells() {
        let old = book(vec![sheet("Data", 1, &table())]);
        let mut cells = table();
        cells[4] = ("B2", "4"); // 3 → 4
        cells[8] = ("C3", "=B3*3"); // formula edited
        cells.retain(|(a, _)| *a != "A5"); // cleared
        cells.push(("D2", "note")); // added
        let new = book(vec![sheet("Data", 1, &cells)]);
        assert_eq!(
            summary(&diff_workbooks(&old, &new)),
            vec![
                "ValueChanged Data!B2 3 -> 4",
                "CellAdded Data!D2 -> note",
                "FormulaChanged Data!C3 =B3*2 -> =B3*3",
                "CellCleared Data!A5 Tag",
            ]
        );
    }

    #[test]
    fn inserted_row_is_one_change_not_every_cell_below() {
        let old = book(vec![sheet("Data", 1, &table())]);
        // A row "Cap 4" inserted at row 3; everything below moves down one row,
        // formulas follow (C4 "=B3*2" → C5 "=B5*2" etc., SUM grows to B2:B6).
        let new = book(vec![sheet(
            "Data",
            1,
            &[
                ("A1", "Item"),
                ("B1", "Qty"),
                ("C1", "Total"),
                ("A2", "Pen"),
                ("B2", "3"),
                ("C2", "=B2*2"),
                ("A3", "Cap"),
                ("B3", "4"),
                ("C3", "=B3*2"),
                ("A4", "Ink"),
                ("B4", "5"),
                ("C4", "=B4*2"),
                ("A5", "Pad"),
                ("B5", "7"),
                ("C5", "=B5*2"),
                ("A6", "Tag"),
                ("B6", "9"),
                ("C6", "=B6*2"),
                ("B7", "=SUM(B2:B6)"),
            ],
        )]);
        let ch = diff_workbooks(&old, &new);
        assert_eq!(summary(&ch), vec!["RowsInserted Data!3"]);
        assert_eq!((ch[0].row, ch[0].count), (Some(2), Some(1)));
    }

    #[test]
    fn deleted_rows_and_inserted_column() {
        let old = book(vec![sheet("Data", 1, &table())]);
        // Rows 3–4 (Ink, Pad) deleted; a column "Unit" inserted before C.
        let new = book(vec![sheet(
            "Data",
            1,
            &[
                ("A1", "Item"),
                ("B1", "Qty"),
                ("C1", "Unit"),
                ("D1", "Total"),
                ("A2", "Pen"),
                ("B2", "3"),
                ("C2", "pcs"),
                ("D2", "=B2*2"),
                ("A3", "Tag"),
                ("B3", "9"),
                ("C3", "pcs"),
                ("D3", "=B3*2"),
                ("B4", "=SUM(B2:B3)"),
            ],
        )]);
        let ch = diff_workbooks(&old, &new);
        assert_eq!(
            summary(&ch),
            vec!["RowsDeleted Data!3", "ColsInserted Data!C"]
        );
        assert_eq!(ch[0].old_address.as_deref(), Some("3:4"));
        assert_eq!(ch[0].count, Some(2));
    }

    #[test]
    fn sheets_added_removed_and_renamed() {
        let old = book(vec![
            sheet("Jan", 1, &[("A1", "1")]),
            sheet("Feb", 2, &[("A1", "2")]),
            sheet("Old", 3, &[("A1", "3")]),
        ]);
        let new = book(vec![
            sheet("January", 1, &[("A1", "1")]),
            sheet("Feb", 2, &[("A1", "20")]),
            sheet("Mar", 4, &[("A1", "4")]),
        ]);
        assert_eq!(
            summary(&diff_workbooks(&old, &new)),
            vec![
                "SheetRenamed January!",
                "SheetRemoved Old!",
                "SheetAdded Mar!",
                "ValueChanged Feb!A1 2 -> 20",
            ]
        );
    }

    #[test]
    fn appended_rows_are_added_cells() {
        let old = book(vec![sheet("S", 1, &[("A1", "x"), ("A2", "y")])]);
        let new = book(vec![sheet(
            "S",
            1,
            &[("A1", "x"), ("A2", "y"), ("A3", "z")],
        )]);
        assert_eq!(
            summary(&diff_workbooks(&old, &new)),
            vec!["CellAdded S!A3 -> z"]
        );
    }

    #[test]
    fn formatting_is_compared_by_resolved_format() {
        let mut old = book(vec![sheet("S", 1, &[("A1", "x"), ("A2", "y")])]);
        let mut new = old.clone();
        // New workbook: an extra (unused) font first, so the bold font has another id.
        let bold = Font {
            bold: true,
            ..old.styles.fonts[0].clone()
        };
        old.styles.fonts.push(bold.clone());
        old.styles.cell_xfs.push(CellXf {
            font_id: 1,
            ..old.styles.cell_xfs[0].clone()
        });
        new.styles.fonts.push(Font::default());
        new.styles.fonts.push(bold);
        new.styles.cell_xfs.push(CellXf {
            font_id: 2,
            ..new.styles.cell_xfs[0].clone()
        });
        // A1 bold on both sides (different style ids): no change. A2 made bold.
        old.sheets[0].cells.get_mut(&(0, 0)).expect("A1").style = 1;
        new.sheets[0].cells.get_mut(&(0, 0)).expect("A1").style = 1;
        new.sheets[0].cells.get_mut(&(1, 0)).expect("A2").style = 1;
        assert_eq!(
            summary(&diff_workbooks(&old, &new)),
            vec!["FormatChanged S!A2 y -> y"]
        );
    }

    #[test]
    fn shared_formula_followers_are_expanded() {
        let mut old = book(vec![sheet("S", 1, &[("A1", "1"), ("A2", "2")])]);
        let ws = &mut old.sheets[0];
        ws.cells.insert(
            (0, 1),
            Cell {
                formula: Some(Formula {
                    text: "A1*10".into(),
                    kind: FormulaKind::Shared {
                        index: 0,
                        range: Some("B1:B2".into()),
                    },
                    ..Formula::default()
                }),
                ..Cell::default()
            },
        );
        ws.cells.insert(
            (1, 1),
            Cell {
                formula: Some(Formula {
                    text: String::new(),
                    kind: FormulaKind::Shared {
                        index: 0,
                        range: None,
                    },
                    ..Formula::default()
                }),
                ..Cell::default()
            },
        );
        // The same formulas written out in full: no change.
        let same = book(vec![sheet(
            "S",
            1,
            &[("A1", "1"), ("A2", "2"), ("B1", "=A1*10"), ("B2", "=A2*10")],
        )]);
        assert_eq!(diff_workbooks(&old, &same), Vec::<SheetChange>::new());
        let edited = book(vec![sheet(
            "S",
            1,
            &[("A1", "1"), ("A2", "2"), ("B1", "=A1*10"), ("B2", "=A2*20")],
        )]);
        assert_eq!(
            summary(&diff_workbooks(&old, &edited)),
            vec!["FormulaChanged S!B2 =A2*10 -> =A2*20"]
        );
    }

    #[test]
    fn long_sheets_use_patience_anchors() {
        // 6 000 numbered rows (too many for the quadratic table); row 1 500 deleted,
        // a row inserted before row 2 000 and row 4 500 edited.
        let old_cells: Vec<(String, String)> = (1..=6000)
            .map(|r| (format!("A{r}"), format!("{}", r * 7)))
            .collect();
        let mut new_vals: Vec<u32> = (1..=6000).map(|r| r * 7).collect();
        new_vals[4499] = 1;
        new_vals.insert(1999, 5);
        new_vals.remove(1499);
        let new_cells: Vec<(String, String)> = new_vals
            .iter()
            .enumerate()
            .map(|(i, v)| (format!("A{}", i + 1), v.to_string()))
            .collect();
        let refs = |v: &[(String, String)]| -> Vec<(String, String)> { v.to_vec() };
        let mk = |v: Vec<(String, String)>| {
            let pairs: Vec<(&str, &str)> =
                v.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
            book(vec![sheet("S", 1, &pairs)])
        };
        let ch = diff_workbooks(&mk(refs(&old_cells)), &mk(new_cells));
        assert_eq!(
            summary(&ch),
            vec![
                "RowsInserted S!1999",
                "RowsDeleted S!1500",
                "ValueChanged S!A4500 31500 -> 1",
            ]
        );
        assert_eq!(ch[1].old_address.as_deref(), Some("1500"));
    }

    #[test]
    fn align_pairs_edited_items_in_equal_gaps() {
        // a b c d → a X c d e: b edited in place (equal gap), e inserted.
        let old = [1, 2, 3, 4];
        let new = [1, 9, 3, 4, 5];
        let steps = align(&old, &new, &|_, _| 0.0);
        assert_eq!(
            steps,
            vec![
                Step::Both(0, 0),
                Step::Both(1, 1),
                Step::Both(2, 2),
                Step::Both(3, 3),
                Step::New(4)
            ]
        );
    }
}
