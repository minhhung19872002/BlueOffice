//! Outlines (row / column groups), Auto Outline and Subtotal.
//!
//! * A grouped row carries `outlineLevel` (ECMA-376 Part 1, §18.3.1.73 `row`), a
//!   grouped column the same attribute on `col` (§18.3.1.13); `collapsed` marks the
//!   summary row / column of a hidden group.
//! * `sheetFormatPr/@outlineLevelRow` / `@outlineLevelCol` (§18.3.1.81) hold the
//!   highest level in use.
//! * `sheetPr/outlinePr/@summaryBelow` / `@summaryRight` (§18.3.1.61 `outlinePr`)
//!   say on which side of its detail a summary row / column sits.
//!
//! Levels are capped at [`MAX_LEVEL`] (`outlineLevel` is an unsigned byte; seven
//! nested groups is our limit, matching the eight level buttons of the outline bar).

use crate::calc::FormulaAdjuster;
use crate::datatools::ConsolidateFn;
use crate::numfmt::Locale;
use crate::ops::{delete_rows, insert_rows};
use crate::render::format_cell;
use crate::rowcol::update_cols;
use bo_formula::{Expr, RefKind};
use bo_model::sheet::{
    col_to_letters, Cell, CellRange, CellRef, CellValue, Formula, RowInfo, Workbook, Worksheet,
    MAX_COLS, MAX_ROWS,
};
use serde::Serialize;
use std::collections::BTreeSet;

/// Deepest outline level.
pub const MAX_LEVEL: u8 = 7;

fn set_row_level(ws: &mut Worksheet, r: u32, level: u8) {
    let info = ws.rows.entry(r).or_default();
    info.outline_level = level.min(MAX_LEVEL);
    if level == 0 {
        info.collapsed = false;
    }
    if *info == RowInfo::default() {
        ws.rows.remove(&r);
    }
}

/// Recomputes `outlineLevelRow` / `outlineLevelCol` (§18.3.1.81) from the rows and columns.
pub fn refresh_levels(ws: &mut Worksheet) {
    ws.format.outline_level_row = ws.rows.values().map(|r| r.outline_level).max().unwrap_or(0);
    ws.format.outline_level_col = ws.cols.iter().map(|c| c.outline_level).max().unwrap_or(0);
}

/// Groups (`ungroup = false`: one level deeper) or ungroups (one level up) rows
/// (`rows = true`) or columns `first..=last`. Returns whether anything changed.
pub fn group(ws: &mut Worksheet, rows: bool, first: u32, last: u32, ungroup: bool) -> bool {
    let (first, last) = (first.min(last), first.max(last));
    let step = |l: u8| {
        if ungroup {
            l.saturating_sub(1)
        } else {
            (l + 1).min(MAX_LEVEL)
        }
    };
    let mut changed = false;
    if rows {
        for r in first..=last.min(MAX_ROWS - 1) {
            let old = ws.rows.get(&r).map_or(0, |i| i.outline_level);
            if step(old) != old {
                set_row_level(ws, r, step(old));
                changed = true;
            }
        }
    } else {
        update_cols(ws, first, last, |c| {
            let new = step(c.outline_level);
            if new != c.outline_level {
                c.outline_level = new;
                if new == 0 {
                    c.collapsed = false;
                }
                changed = true;
            }
        });
    }
    refresh_levels(ws);
    changed
}

/// Removes the outline of `range` (all rows and columns it spans; `None` = the whole
/// sheet). Hidden rows / columns stay hidden.
pub fn clear_outline(ws: &mut Worksheet, range: Option<CellRange>) {
    match range {
        None => {
            let rows: Vec<u32> = ws.rows.keys().copied().collect();
            for r in rows {
                set_row_level(ws, r, 0);
            }
            if !ws.cols.is_empty() {
                update_cols(ws, 0, MAX_COLS - 1, |c| {
                    c.outline_level = 0;
                    c.collapsed = false;
                });
            }
        }
        Some(r) => {
            let rows: Vec<u32> = ws
                .rows
                .range(r.first.row..=r.last.row)
                .map(|(k, _)| *k)
                .collect();
            for row in rows {
                set_row_level(ws, row, 0);
            }
            if r.first.col > 0 || r.last.col < MAX_COLS - 1 {
                update_cols(ws, r.first.col, r.last.col, |c| {
                    c.outline_level = 0;
                    c.collapsed = false;
                });
            }
        }
    }
    refresh_levels(ws);
}

/// Shows outline level `level` (1 = only the outermost summaries) of rows or columns:
/// rows / columns whose outline level is `level` or deeper are hidden, the shallower
/// grouped ones shown; summary rows / columns next to a hidden group get `collapsed`.
pub fn show_level(ws: &mut Worksheet, rows: bool, level: u8, summary_after: bool) {
    let level = level.max(1);
    if rows {
        let keys: Vec<u32> = ws
            .rows
            .iter()
            .filter(|(_, i)| i.outline_level > 0)
            .map(|(k, _)| *k)
            .collect();
        for &r in &keys {
            if let Some(i) = ws.rows.get_mut(&r) {
                i.hidden = i.outline_level >= level;
            }
        }
        // `collapsed` on the row next to a hidden group.
        let lvl = |ws: &Worksheet, r: u32| ws.rows.get(&r).map_or(0, |i| i.outline_level);
        let hidden = |ws: &Worksheet, r: u32| ws.rows.get(&r).is_some_and(|i| i.hidden);
        let mut marks = BTreeSet::new();
        for &r in &keys {
            let summary = if summary_after {
                r.checked_add(1)
            } else {
                r.checked_sub(1)
            };
            // A hidden summary is marked too: showing its outer group later keeps this
            // one collapsed (level 1, then + on the grand total, shows the subtotals).
            if let Some(s) = summary.filter(|&s| s < MAX_ROWS) {
                if hidden(ws, r) && lvl(ws, s) < lvl(ws, r) {
                    marks.insert(s);
                }
            }
        }
        let all: BTreeSet<u32> = ws
            .rows
            .keys()
            .copied()
            .chain(marks.iter().copied())
            .collect();
        for r in all {
            let c = marks.contains(&r);
            let info = ws.rows.entry(r).or_default();
            info.collapsed = c;
            if *info == RowInfo::default() {
                ws.rows.remove(&r);
            }
        }
    } else {
        let spans: Vec<(u32, u32, u8)> = ws
            .cols
            .iter()
            .filter(|c| c.outline_level > 0)
            .map(|c| (c.min, c.max, c.outline_level))
            .collect();
        for (a, b, l) in &spans {
            update_cols(ws, *a, *b, |c| c.hidden = *l >= level);
        }
        let lvl = |ws: &Worksheet, c: u32| ws.col_info(c).map_or(0, |i| i.outline_level);
        let hidden = |ws: &Worksheet, c: u32| ws.col_info(c).is_some_and(|i| i.hidden);
        let mut marks = BTreeSet::new();
        for (a, b, _) in &spans {
            for c in *a..=*b {
                let summary = if summary_after {
                    c.checked_add(1)
                } else {
                    c.checked_sub(1)
                };
                if let Some(s) = summary.filter(|&s| s < MAX_COLS) {
                    if hidden(ws, c) && lvl(ws, s) < lvl(ws, c) {
                        marks.insert(s);
                    }
                }
            }
        }
        if !ws.cols.is_empty() || !marks.is_empty() {
            update_cols(ws, 0, MAX_COLS - 1, |c| c.collapsed = false);
            for s in marks {
                update_cols(ws, s, s, |c| c.collapsed = true);
            }
        }
    }
}

/// One group of the outline bar: a maximal run `first..=last` of rows / columns at
/// outline `level` or deeper, with the summary row / column next to it (after the
/// run when `summaryBelow` / `summaryRight`, §18.3.1.61; `None` at the sheet edge).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OutlineGroup {
    /// Outline level of the group (1 = outermost).
    pub level: u8,
    /// First row / column of the detail.
    pub first: u32,
    /// Last row / column of the detail.
    pub last: u32,
    /// The summary row / column (where the +/− button sits).
    pub summary: Option<u32>,
    /// Every row / column of the detail is hidden (the button shows +).
    pub collapsed: bool,
}

/// `(index, outline level, hidden)` of every grouped row / column, in order.
fn grouped_lines(ws: &Worksheet, rows: bool) -> Vec<(u32, u8, bool)> {
    if rows {
        ws.rows
            .iter()
            .filter(|(_, i)| i.outline_level > 0)
            .map(|(k, i)| (*k, i.outline_level, i.hidden))
            .collect()
    } else {
        let mut v: Vec<(u32, u8, bool)> = ws
            .cols
            .iter()
            .filter(|c| c.outline_level > 0)
            .flat_map(|c| (c.min..=c.max.min(MAX_COLS - 1)).map(|i| (i, c.outline_level, c.hidden)))
            .collect();
        v.sort_unstable_by_key(|l| l.0);
        v
    }
}

/// The groups of the row (`rows`) or column outline, outer levels first, at most
/// `limit` of them. `summary_after`: summaries below / right of their detail.
pub fn outline_groups(
    ws: &Worksheet,
    rows: bool,
    summary_after: bool,
    limit: usize,
) -> Vec<OutlineGroup> {
    let lines = grouped_lines(ws, rows);
    let max = lines.iter().map(|l| l.1).max().unwrap_or(0);
    let end = if rows { MAX_ROWS } else { MAX_COLS };
    let mut out = Vec::new();
    for level in 1..=max {
        let mut run: Option<(u32, u32, bool)> = None;
        let flush = |run: Option<(u32, u32, bool)>, out: &mut Vec<OutlineGroup>| {
            if let Some((first, last, collapsed)) = run {
                let summary = if summary_after {
                    last.checked_add(1).filter(|&s| s < end)
                } else {
                    first.checked_sub(1)
                };
                out.push(OutlineGroup {
                    level,
                    first,
                    last,
                    summary,
                    collapsed,
                });
            }
        };
        for &(i, l, hidden) in &lines {
            if l < level {
                continue;
            }
            match &mut run {
                Some((_, last, collapsed)) if *last + 1 == i => {
                    *last = i;
                    *collapsed &= hidden;
                }
                _ => {
                    flush(run.take(), &mut out);
                    run = Some((i, i, hidden));
                }
            }
            if out.len() >= limit {
                return out;
            }
        }
        flush(run, &mut out);
        if out.len() >= limit {
            out.truncate(limit);
            return out;
        }
    }
    out
}

fn line_collapsed(ws: &Worksheet, rows: bool, i: u32) -> bool {
    if rows {
        ws.rows.get(&i).is_some_and(|r| r.collapsed)
    } else {
        ws.col_info(i).is_some_and(|c| c.collapsed)
    }
}

fn set_lines(
    ws: &mut Worksheet,
    rows: bool,
    first: u32,
    last: u32,
    f: impl Fn(&mut bool, &mut bool),
) {
    if rows {
        for r in first..=last {
            let info = ws.rows.entry(r).or_default();
            f(&mut info.hidden, &mut info.collapsed);
            if *info == RowInfo::default() {
                ws.rows.remove(&r);
            }
        }
    } else {
        update_cols(ws, first, last, |c| f(&mut c.hidden, &mut c.collapsed));
    }
}

fn pick_group(groups: &[OutlineGroup], at: u32, show: bool) -> Option<OutlineGroup> {
    let wanted = |g: &&OutlineGroup| g.collapsed == show;
    groups
        .iter()
        .filter(wanted)
        .find(|g| g.summary == Some(at))
        .or_else(|| {
            groups
                .iter()
                .filter(wanted)
                .filter(|g| g.first <= at && at <= g.last)
                .max_by_key(|g| g.level)
        })
        .copied()
}

/// The group [`set_detail`] would show / hide, if any.
pub fn detail_target(
    ws: &Worksheet,
    rows: bool,
    at: u32,
    show: bool,
    summary_after: bool,
) -> Option<OutlineGroup> {
    pick_group(
        &outline_groups(ws, rows, summary_after, usize::MAX),
        at,
        show,
    )
}

/// Show Detail (`show`) / Hide Detail of the group at row / column `at` — the +/−
/// button of the outline bar, Data › Outline › Show / Hide Detail. The group is the
/// one whose summary is `at`, otherwise the innermost one containing `at`. Hiding
/// hides the whole detail and marks the summary `collapsed`; showing unhides it but
/// keeps the inner groups whose summary is still marked collapsed hidden. Returns
/// whether anything changed.
pub fn set_detail(
    ws: &mut Worksheet,
    rows: bool,
    at: u32,
    show: bool,
    summary_after: bool,
) -> bool {
    let groups = outline_groups(ws, rows, summary_after, usize::MAX);
    let Some(g) = pick_group(&groups, at, show) else {
        return false;
    };
    if show {
        // Inner groups still marked collapsed stay hidden.
        let keep: Vec<(u32, u32)> = groups
            .iter()
            .filter(|n| n.level > g.level && g.first <= n.first && n.last <= g.last)
            .filter(|n| n.summary.is_some_and(|s| line_collapsed(ws, rows, s)))
            .map(|n| (n.first, n.last))
            .collect();
        set_lines(ws, rows, g.first, g.last, |h, _| *h = false);
        for (a, b) in keep {
            set_lines(ws, rows, a, b, |h, _| *h = true);
        }
    } else {
        set_lines(ws, rows, g.first, g.last, |h, _| *h = true);
    }
    if let Some(s) = g.summary {
        set_lines(ws, rows, s, s, |_, c| *c = !show);
    }
    true
}

/// A detail span found by [`auto_outline`]: `first..=last` summarised by a row /
/// column after it (`after = true`) or before it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Span {
    first: u32,
    last: u32,
    after: bool,
}

/// Same-sheet references of a formula as `(r1, c1, r2, c2)`.
fn local_refs(text: &str, sheet_name: &str) -> Vec<(u32, u32, u32, u32)> {
    let Ok(expr) = bo_formula::parse(text) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    expr.walk(&mut |e| {
        if let Expr::Ref(r) = e {
            let local = match &r.sheet {
                None => true,
                Some(s) => s.last.is_none() && s.first.eq_ignore_ascii_case(sheet_name),
            };
            if local && matches!(r.kind, RefKind::Cell(_) | RefKind::Range(..)) {
                out.push(r.bounds());
            }
        }
    });
    out
}

/// Merges intervals that overlap or touch.
fn merge_intervals(mut v: Vec<(u32, u32)>) -> Vec<(u32, u32)> {
    v.sort_unstable();
    let mut out: Vec<(u32, u32)> = Vec::new();
    for (a, b) in v {
        match out.last_mut() {
            Some(last) if a <= last.1.saturating_add(1) => last.1 = last.1.max(b),
            _ => out.push((a, b)),
        }
    }
    out
}

/// The detail span of a summary at `at` whose references along the line cover the
/// intervals `iv` (rows of the same column, or columns of the same row): they must
/// form one block right next to `at` (`=B5+B8` over two subtotals is no block).
fn span_of(iv: Vec<(u32, u32)>, at: u32) -> Option<Span> {
    let merged = merge_intervals(iv);
    if merged.len() != 1 {
        return None;
    }
    merged.iter().find_map(|&(a, b)| {
        if at > 0 && b == at - 1 {
            Some(Span {
                first: a,
                last: b,
                after: true,
            })
        } else if a == at + 1 {
            Some(Span {
                first: a,
                last: b,
                after: false,
            })
        } else {
            None
        }
    })
}

/// Result of [`auto_outline`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoOutline {
    /// Deepest row level created (0 = none).
    pub row_levels: u8,
    /// Deepest column level created (0 = none).
    pub col_levels: u8,
}

/// Builds the outline of `range` from its formulas (our own detection rule): a
/// cell whose same-sheet references in its own column form a block of rows
/// directly above (or below) it is a summary row of that block; references in its
/// own row forming a block of columns directly left (or right) of it make a summary
/// column. Each row / column gets as level the number of blocks containing it. The
/// previous outline of `range` is replaced; `summaryBelow` / `summaryRight`
/// (§18.3.1.61) follow the direction found.
pub fn auto_outline(ws: &mut Worksheet, range: CellRange, rows: bool, cols: bool) -> AutoOutline {
    let mut row_spans = BTreeSet::new();
    let mut col_spans = BTreeSet::new();
    let name = ws.name.clone();
    for (&(r, c), cell) in &ws.cells {
        if !range.contains(CellRef::new(r, c)) {
            continue;
        }
        let Some(f) = cell.formula.as_ref().filter(|f| !f.text.is_empty()) else {
            continue;
        };
        let refs = local_refs(&f.text, &name);
        let inside =
            |(r1, c1, r2, c2): (u32, u32, u32, u32)| !(r1 <= r && r <= r2 && c1 <= c && c <= c2);
        if rows {
            let iv: Vec<(u32, u32)> = refs
                .iter()
                .copied()
                .filter(|&b| b.1 == c && b.3 == c && inside(b))
                .map(|b| (b.0, b.2))
                .collect();
            if let Some(s) = span_of(iv, r) {
                if s.first >= range.first.row && s.last <= range.last.row {
                    row_spans.insert(s);
                }
            }
        }
        if cols {
            let iv: Vec<(u32, u32)> = refs
                .iter()
                .copied()
                .filter(|&b| b.0 == r && b.2 == r && inside(b))
                .map(|b| (b.1, b.3))
                .collect();
            if let Some(s) = span_of(iv, c) {
                if s.first >= range.first.col && s.last <= range.last.col {
                    col_spans.insert(s);
                }
            }
        }
    }
    let mut out = AutoOutline::default();
    let level = |spans: &BTreeSet<Span>, x: u32| -> u8 {
        let n = spans.iter().filter(|s| s.first <= x && x <= s.last).count();
        u8::try_from(n).unwrap_or(MAX_LEVEL).min(MAX_LEVEL)
    };
    if rows {
        let keys: Vec<u32> = ws
            .rows
            .range(range.first.row..=range.last.row)
            .map(|(k, _)| *k)
            .collect();
        for r in keys {
            set_row_level(ws, r, 0);
        }
        let touched: BTreeSet<u32> = row_spans.iter().flat_map(|s| s.first..=s.last).collect();
        for r in touched {
            let l = level(&row_spans, r);
            set_row_level(ws, r, l);
            out.row_levels = out.row_levels.max(l);
        }
        if !row_spans.is_empty() {
            let all_before = row_spans.iter().all(|s| !s.after);
            ws.props.summary_below = if all_before {
                Some(false)
            } else if ws.props.summary_below == Some(false) {
                None
            } else {
                ws.props.summary_below
            };
        }
    }
    if cols {
        update_cols(ws, range.first.col, range.last.col, |c| {
            c.outline_level = 0;
            c.collapsed = false;
        });
        let touched: BTreeSet<u32> = col_spans.iter().flat_map(|s| s.first..=s.last).collect();
        for c in touched {
            let l = level(&col_spans, c);
            update_cols(ws, c, c, |ci| ci.outline_level = l);
            out.col_levels = out.col_levels.max(l);
        }
        if !col_spans.is_empty() {
            let all_before = col_spans.iter().all(|s| !s.after);
            ws.props.summary_right = if all_before {
                Some(false)
            } else if ws.props.summary_right == Some(false) {
                None
            } else {
                ws.props.summary_right
            };
        }
    }
    refresh_levels(ws);
    out
}

// ------------------------------------------------------------------ Subtotal

/// Options of [`subtotal`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubtotalSpec {
    /// The list, its header row first.
    pub range: CellRange,
    /// Absolute column whose changes start a new group.
    pub group_by: u32,
    /// Summary function (`SUBTOTAL` numbers 1–11).
    pub function: ConsolidateFn,
    /// Absolute columns that get a subtotal.
    pub columns: Vec<u32>,
    /// Remove the subtotals already in the list first.
    pub replace: bool,
    /// A manual page break after each group (§18.3.1.74 `rowBreaks`).
    pub page_breaks: bool,
    /// Subtotal rows below their group (otherwise above).
    pub summary_below: bool,
    /// Caption of a group's subtotal row; `{0}` is the group's value.
    pub label: Option<String>,
    /// Caption of the grand total row.
    pub grand_label: Option<String>,
}

/// Result of [`subtotal`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SubtotalResult {
    /// Groups found.
    pub groups: u32,
    /// The list after the insertion (header, details, subtotals and grand total).
    pub range: CellRange,
    /// Deepest row outline level.
    pub levels: u8,
}

/// Whether a row of the list is a subtotal row (a cell with a `SUBTOTAL` formula).
fn is_subtotal_row(ws: &Worksheet, r: u32, c0: u32, c1: u32) -> bool {
    ws.cells.range((r, c0)..=(r, c1)).any(|(_, cell)| {
        cell.formula.as_ref().is_some_and(|f| {
            let t = f.text.trim_start().to_ascii_uppercase();
            t.starts_with("SUBTOTAL(") || t.starts_with("_XLFN.SUBTOTAL(")
        })
    })
}

/// Deletes the subtotal rows of `range` (sheet `sheet`) and its row outline and page
/// breaks. Returns the list's new range.
pub fn remove_subtotals(
    wb: &mut Workbook,
    sheet: usize,
    range: CellRange,
    adj: &dyn FormulaAdjuster,
) -> Result<CellRange, String> {
    let ws = wb.sheets.get(sheet).ok_or("no such sheet")?;
    let rows: Vec<u32> = (range.first.row..=range.last.row)
        .filter(|&r| is_subtotal_row(ws, r, range.first.col, range.last.col))
        .collect();
    for &r in rows.iter().rev() {
        delete_rows(wb, sheet, r, 1, adj).map_err(|e| format!("{e:?}"))?;
    }
    let last = range.last.row - rows.len() as u32;
    let new = CellRange::new(
        range.first,
        CellRef::new(last.max(range.first.row), range.last.col),
    );
    let ws = &mut wb.sheets[sheet];
    ws.page_setup
        .row_breaks
        .retain(|&b| b < new.first.row || b > new.last.row);
    clear_outline(
        ws,
        Some(CellRange::new(
            CellRef::new(new.first.row, 0),
            CellRef::new(new.last.row, MAX_COLS - 1),
        )),
    );
    Ok(new)
}

/// Inserts subtotal rows in the list `spec.range` of sheet `sheet`: at each change
/// of the value in `group_by` a row with the group's caption and
/// `SUBTOTAL(n, …)` (§18.17.7 SUBTOTAL; nested subtotals are not counted twice)
/// over the group's cells of each chosen column, plus a grand total row; then
/// builds the row outline with [`auto_outline`]. Errors: `noData`, `badColumn`.
pub fn subtotal(
    wb: &mut Workbook,
    sheet: usize,
    spec: &SubtotalSpec,
    adj: &dyn FormulaAdjuster,
    locale: &Locale,
) -> Result<SubtotalResult, String> {
    let mut range = spec.range;
    let (c0, c1) = (range.first.col, range.last.col);
    if !(c0..=c1).contains(&spec.group_by)
        || spec.columns.is_empty()
        || spec.columns.iter().any(|c| !(c0..=c1).contains(c))
    {
        return Err("badColumn".into());
    }
    if spec.replace {
        range = remove_subtotals(wb, sheet, range, adj)?;
    }
    let header = range.first.row;
    if range.last.row <= header {
        return Err("noData".into());
    }
    let vi = *locale == Locale::VI_VN;
    let label = spec
        .label
        .clone()
        .unwrap_or_else(|| (if vi { "Tổng: {0}" } else { "Total: {0}" }).to_owned());
    let grand = spec
        .grand_label
        .clone()
        .unwrap_or_else(|| (if vi { "Tổng cộng" } else { "Total of all" }).to_owned());
    // Runs of equal values (displayed text, case-insensitive); existing subtotal rows
    // end a run and get no subtotal of their own.
    let mut groups: Vec<(u32, u32, String)> = Vec::new();
    {
        let wbr: &Workbook = wb;
        let ws = &wbr.sheets[sheet];
        let mut cur: Option<(u32, u32, String, String)> = None;
        for r in header + 1..=range.last.row {
            if is_subtotal_row(ws, r, c0, c1) {
                if let Some((a, b, _, t)) = cur.take() {
                    groups.push((a, b, t));
                }
                continue;
            }
            let text = ws
                .cell(r, spec.group_by)
                .map(|c| format_cell(wbr, c, locale).text)
                .unwrap_or_default();
            let key = text.trim().to_lowercase();
            match &mut cur {
                Some((_, b, k, _)) if *k == key => *b = r,
                _ => {
                    if let Some((a, b, _, t)) = cur.take() {
                        groups.push((a, b, t));
                    }
                    cur = Some((r, r, key, text));
                }
            }
        }
        if let Some((a, b, _, t)) = cur {
            groups.push((a, b, t));
        }
    }
    if groups.is_empty() {
        return Err("noData".into());
    }
    let n = spec.function.subtotal_number();
    let formula = |col: u32, a: u32, b: u32| {
        let l = col_to_letters(col);
        Formula::normal(&format!("SUBTOTAL({n},{l}{}:{l}{})", a + 1, b + 1))
    };
    // Writes a summary row at `at` over rows `a..=b`, styles taken from row `like`.
    let write = |wb: &mut Workbook, at: u32, a: u32, b: u32, like: u32, caption: &str| {
        let ws = &mut wb.sheets[sheet];
        let style_of = |ws: &Worksheet, c: u32| ws.cell(like, c).map_or(0, |x| x.style);
        let s = style_of(ws, spec.group_by);
        let mut cell = Cell::new(CellValue::Text(caption.to_owned()));
        cell.style = s;
        ws.set_cell(at, spec.group_by, cell);
        for &c in &spec.columns {
            if c == spec.group_by {
                continue;
            }
            let mut cell = Cell::new(CellValue::Empty);
            cell.formula = Some(formula(c, a, b));
            cell.style = style_of(ws, c);
            ws.set_cell(at, c, cell);
        }
    };
    let insert = |wb: &mut Workbook, at: u32| {
        insert_rows(wb, sheet, at, 1, adj).map_err(|e| format!("{e:?}"))
    };
    let count = groups.len() as u32;
    let mut summary_rows = Vec::with_capacity(groups.len());
    if spec.summary_below {
        for (i, (a, b, text)) in groups.iter().enumerate().rev() {
            insert(wb, b + 1)?;
            write(wb, b + 1, *a, *b, *b, &label.replace("{0}", text));
            // Rows of the groups below moved down by the rows inserted before them.
            summary_rows.push(b + 1 + i as u32);
        }
        let end = range.last.row + count;
        insert(wb, end + 1)?;
        write(wb, end + 1, header + 1, end, end, &grand);
        range.last.row = end + 1;
    } else {
        for (i, (a, b, text)) in groups.iter().enumerate().rev() {
            insert(wb, *a)?;
            write(wb, *a, a + 1, b + 1, a + 1, &label.replace("{0}", text));
            // +1 for the grand total row inserted above everything below.
            summary_rows.push(a + i as u32 + 1);
        }
        let end = range.last.row + count;
        insert(wb, header + 1)?;
        write(wb, header + 1, header + 2, end + 1, header + 2, &grand);
        range.last.row = end + 1;
    }
    let ws = &mut wb.sheets[sheet];
    if spec.page_breaks {
        // `rowBreaks/brk/@id` (§18.3.1.74, §18.3.1.6) is the row after which the page breaks.
        summary_rows.sort_unstable();
        let breaks: Vec<u32> = if spec.summary_below {
            summary_rows[..summary_rows.len() - 1].to_vec()
        } else {
            summary_rows[1..].iter().map(|r| r - 1).collect()
        };
        for b in breaks {
            if !ws.page_setup.row_breaks.contains(&b) {
                ws.page_setup.row_breaks.push(b);
            }
        }
        ws.page_setup.row_breaks.sort_unstable();
    }
    let rows_range = CellRange::new(
        CellRef::new(range.first.row, c0),
        CellRef::new(range.last.row, c1),
    );
    let o = auto_outline(ws, rows_range, true, false);
    // §18.3.1.61: summary rows above their detail.
    ws.props.summary_below = if spec.summary_below {
        None
    } else {
        Some(false)
    };
    Ok(SubtotalResult {
        groups: count,
        range,
        levels: o.row_levels,
    })
}

#[cfg(test)]
mod tests;
