//! Row and column edits: sizes, hiding, freeze panes and merged cells.
//!
//! * Column widths are stored in characters (ECMA-376 Part 1, §18.3.1.13
//!   `col/@width`), converted from points with the maximum digit width.
//! * Row heights are points (§18.3.1.73 `row/@ht` with `customHeight`).
//! * Freeze panes write a frozen `pane` (§18.3.1.66) into the first sheet view.
//! * Merges are `mergeCell` ranges (§18.3.1.55); only the top-left cell keeps its
//!   content, as spreadsheet applications do.

use crate::geometry::{SheetMetrics, PT_PER_PX};
use bo_model::sheet::{
    CellRange, CellRef, ColInfo, Pane, PaneState, SheetView, Worksheet, MAX_COLS, MAX_ROWS,
};

/// Column width in characters for a width in points (inverse of
/// [`SheetMetrics::width_to_pt`]: `Truncate(px / MDW × 256) / 256`).
pub fn pt_to_width(m: &SheetMetrics, pt: f64) -> f64 {
    let px = (pt / PT_PER_PX).max(0.0).round();
    ((px / m.mdw_px.max(1.0)) * 256.0).trunc() / 256.0
}

/// Splits the `cols` records so that `first` and `last` fall on record
/// boundaries and every column in `first..=last` has a record; returns the
/// indexes of the records covering the span.
fn col_span(ws: &mut Worksheet, first: u32, last: u32) -> Vec<usize> {
    let mut out: Vec<ColInfo> = Vec::with_capacity(ws.cols.len() + 2);
    let mut cols = std::mem::take(&mut ws.cols);
    cols.sort_by_key(|c| c.min);
    let mut next = first;
    for c in cols {
        if c.max < first || c.min > last {
            if c.min > last && next <= last {
                out.push(ColInfo {
                    min: next,
                    max: last,
                    ..ColInfo::default()
                });
                next = last + 1;
            }
            out.push(c);
            continue;
        }
        if c.min < first {
            out.push(ColInfo {
                max: first - 1,
                ..c.clone()
            });
        }
        let a = c.min.max(first);
        let b = c.max.min(last);
        if next < a {
            out.push(ColInfo {
                min: next,
                max: a - 1,
                ..ColInfo::default()
            });
        }
        out.push(ColInfo {
            min: a,
            max: b,
            ..c.clone()
        });
        next = b + 1;
        if c.max > last {
            out.push(ColInfo { min: last + 1, ..c });
        }
    }
    if next <= last {
        out.push(ColInfo {
            min: next,
            max: last,
            ..ColInfo::default()
        });
    }
    out.sort_by_key(|c| c.min);
    ws.cols = out;
    ws.cols
        .iter()
        .enumerate()
        .filter(|(_, c)| c.min >= first && c.max <= last)
        .map(|(i, _)| i)
        .collect()
}

/// Drops `col` records that carry nothing and merges equal neighbours.
fn tidy_cols(ws: &mut Worksheet) {
    ws.cols.retain(|c| {
        c.width.is_some() || c.hidden || c.style.is_some() || c.outline_level > 0 || c.collapsed
    });
    let mut out: Vec<ColInfo> = Vec::with_capacity(ws.cols.len());
    for c in std::mem::take(&mut ws.cols) {
        if let Some(prev) = out.last_mut() {
            let same = ColInfo {
                min: prev.min,
                max: prev.max,
                ..c.clone()
            } == *prev;
            if same && prev.max + 1 == c.min {
                prev.max = c.max;
                continue;
            }
        }
        out.push(c);
    }
    ws.cols = out;
}

/// Sets the width of columns `first..=last` in points.
pub fn set_col_width(ws: &mut Worksheet, m: &SheetMetrics, first: u32, last: u32, pt: f64) {
    let (first, last) = (first.min(MAX_COLS - 1), last.min(MAX_COLS - 1));
    let w = pt_to_width(m, pt.clamp(0.0, 1_500.0));
    for i in col_span(ws, first, last) {
        let c = &mut ws.cols[i];
        if w <= 0.0 {
            c.hidden = true;
        } else {
            c.width = Some(w);
            c.custom_width = true;
            c.hidden = false;
            c.best_fit = false;
        }
    }
    tidy_cols(ws);
}

/// Sets the height of rows `first..=last` in points (0 hides them).
pub fn set_row_height(ws: &mut Worksheet, first: u32, last: u32, pt: f64) {
    let pt = pt.clamp(0.0, 409.0);
    for r in first..=last.min(MAX_ROWS - 1) {
        let info = ws.rows.entry(r).or_default();
        if pt <= 0.0 {
            info.hidden = true;
        } else {
            info.height = Some((pt * 4.0).round() / 4.0);
            info.custom_height = true;
            info.hidden = false;
        }
    }
}

/// Hides or shows rows (`rows = true`) or columns.
pub fn set_hidden(ws: &mut Worksheet, rows: bool, first: u32, last: u32, hidden: bool) {
    if rows {
        for r in first..=last.min(MAX_ROWS - 1) {
            let info = ws.rows.entry(r).or_default();
            info.hidden = hidden;
            if *info == bo_model::sheet::RowInfo::default() {
                ws.rows.remove(&r);
            }
        }
    } else {
        for i in col_span(ws, first.min(MAX_COLS - 1), last.min(MAX_COLS - 1)) {
            ws.cols[i].hidden = hidden;
        }
        tidy_cols(ws);
    }
}

/// Freezes the first `rows` rows and `cols` columns (0, 0 unfreezes).
pub fn freeze(ws: &mut Worksheet, rows: u32, cols: u32) {
    if ws.views.is_empty() {
        ws.views.push(SheetView {
            tab_selected: true,
            ..SheetView::default()
        });
    }
    let view = &mut ws.views[0];
    // Selections refer to panes that may no longer exist (§18.3.1.78 `pane`).
    view.selections.clear();
    if rows == 0 && cols == 0 {
        view.pane = None;
        return;
    }
    // §18.18.52 ST_Pane: the pane holding the active cell.
    let active = match (rows > 0, cols > 0) {
        (true, true) => "bottomRight",
        (true, false) => "bottomLeft",
        _ => "topRight",
    };
    view.pane = Some(Pane {
        x_split: f64::from(cols),
        y_split: f64::from(rows),
        top_left_cell: Some(CellRef::new(rows, cols).to_string()),
        active_pane: Some(active.into()),
        state: PaneState::Frozen,
    });
}

fn overlaps(a: &CellRange, b: &CellRange) -> bool {
    a.first.row <= b.last.row
        && b.first.row <= a.last.row
        && a.first.col <= b.last.col
        && b.first.col <= a.last.col
}

/// Merges `range` (or each of its rows when `across`). Existing merges that
/// overlap are replaced; contents other than the top-left cell of each merge
/// are cleared (values only, formatting stays).
pub fn merge(ws: &mut Worksheet, range: CellRange, across: bool) {
    let parts: Vec<CellRange> = if across {
        (range.first.row..=range.last.row)
            .map(|r| {
                CellRange::new(
                    CellRef::new(r, range.first.col),
                    CellRef::new(r, range.last.col),
                )
            })
            .collect()
    } else {
        vec![range]
    };
    ws.merges.retain(|m| !overlaps(m, &range));
    for p in parts {
        if p.first == p.last {
            continue;
        }
        let keys: Vec<(u32, u32)> = ws
            .cells
            .range((p.first.row, p.first.col)..=(p.last.row, p.last.col))
            .filter(|(&(r, c), _)| {
                p.contains(CellRef::new(r, c)) && (r, c) != (p.first.row, p.first.col)
            })
            .map(|(&k, _)| k)
            .collect();
        for k in keys {
            if let Some(cell) = ws.cells.get_mut(&k) {
                cell.value = bo_model::sheet::CellValue::Empty;
                cell.formula = None;
                if cell.style == 0 {
                    ws.cells.remove(&k);
                }
            }
        }
        ws.merges.push(p);
    }
}

/// Removes merges overlapping `range`; returns how many were removed.
pub fn unmerge(ws: &mut Worksheet, range: CellRange) -> usize {
    let before = ws.merges.len();
    ws.merges.retain(|m| !overlaps(m, &range));
    before - ws.merges.len()
}

/// Hidden spans (first, last) of rows or columns, for UI navigation.
pub fn hidden_spans(ws: &Worksheet, rows: bool) -> Vec<(u32, u32)> {
    let mut out: Vec<(u32, u32)> = Vec::new();
    let mut push = |a: u32, b: u32| match out.last_mut() {
        Some(last) if last.1 + 1 >= a => last.1 = last.1.max(b),
        _ => out.push((a, b)),
    };
    if rows {
        for (&r, info) in &ws.rows {
            if info.hidden || info.height == Some(0.0) {
                push(r, r);
            }
        }
    } else {
        let mut cols: Vec<&ColInfo> = ws
            .cols
            .iter()
            .filter(|c| c.hidden || c.width == Some(0.0))
            .collect();
        cols.sort_by_key(|c| c.min);
        for c in cols {
            push(c.min, c.max);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::SheetGeometry;
    use bo_model::sheet::{Cell, CellValue};

    #[test]
    fn column_widths_split_and_merge_records() {
        let m = SheetMetrics::default();
        let mut ws = Worksheet::new("S");
        ws.cols.push(ColInfo {
            min: 0,
            max: 9,
            width: Some(12.0),
            custom_width: true,
            ..ColInfo::default()
        });
        set_col_width(&mut ws, &m, 3, 4, 105.0);
        let g = SheetGeometry::with_metrics(&ws, m);
        assert_eq!(g.col_width(3), 105.0);
        assert_eq!(g.col_width(4), 105.0);
        assert_eq!(g.col_width(2), m.width_to_pt(12.0));
        assert_eq!(g.col_width(5), m.width_to_pt(12.0));
        assert_eq!(ws.cols.len(), 3);
        // Columns outside any record get one.
        set_col_width(&mut ws, &m, 20, 20, 30.0);
        let g = SheetGeometry::with_metrics(&ws, m);
        assert_eq!(g.col_width(20), 30.0);
        assert_eq!(g.col_width(19), 48.0);
        // Same width again merges neighbours.
        set_col_width(&mut ws, &m, 0, 9, m.width_to_pt(12.0));
        assert_eq!(ws.cols.len(), 2);
        set_hidden(&mut ws, false, 1, 1, true);
        let g = SheetGeometry::with_metrics(&ws, m);
        assert_eq!(g.col_width(1), 0.0);
        assert_eq!(hidden_spans(&ws, false), vec![(1, 1)]);
        set_hidden(&mut ws, false, 1, 1, false);
        assert_eq!(hidden_spans(&ws, false), Vec::<(u32, u32)>::new());
    }

    #[test]
    fn width_round_trip() {
        let m = SheetMetrics::default();
        for pt in [48.0, 105.0, 30.0, 6.0] {
            assert_eq!(m.width_to_pt(pt_to_width(&m, pt)), pt);
        }
    }

    #[test]
    fn rows_hide_and_freeze() {
        let mut ws = Worksheet::new("S");
        set_row_height(&mut ws, 2, 3, 30.0);
        set_hidden(&mut ws, true, 5, 6, true);
        let g = SheetGeometry::with_metrics(&ws, SheetMetrics::default());
        assert_eq!(g.row_height(2), 30.0);
        assert_eq!(g.row_height(5), 0.0);
        assert_eq!(hidden_spans(&ws, true), vec![(5, 6)]);
        set_hidden(&mut ws, true, 5, 6, false);
        assert!(!ws.rows.contains_key(&5));
        freeze(&mut ws, 1, 2);
        assert_eq!(crate::viewport::frozen_counts(&ws), (1, 2));
        let p = ws.views[0].pane.as_ref().unwrap();
        assert_eq!(p.top_left_cell.as_deref(), Some("C2"));
        assert_eq!(p.active_pane.as_deref(), Some("bottomRight"));
        freeze(&mut ws, 0, 0);
        assert_eq!(crate::viewport::frozen_counts(&ws), (0, 0));
    }

    #[test]
    fn merge_and_unmerge() {
        let mut ws = Worksheet::new("S");
        ws.set_cell(0, 0, Cell::new(CellValue::Text("a".into())));
        ws.set_cell(0, 1, Cell::new(CellValue::Text("b".into())));
        ws.set_cell(1, 1, Cell::new(CellValue::Number(2.0)).with_style(3));
        let r = CellRange::parse("A1:B2").unwrap();
        merge(&mut ws, r, false);
        assert_eq!(ws.merges, vec![r]);
        assert!(ws.cell(0, 1).is_none());
        assert_eq!(ws.cell(1, 1).unwrap().value, CellValue::Empty);
        assert_eq!(ws.cell(1, 1).unwrap().style, 3);
        assert_eq!(ws.cell(0, 0).unwrap().value, CellValue::Text("a".into()));
        merge(&mut ws, CellRange::parse("A1:C3").unwrap(), true);
        assert_eq!(ws.merges.len(), 3, "one merge per row");
        assert_eq!(unmerge(&mut ws, CellRange::parse("B2").unwrap()), 1);
        assert_eq!(ws.merges.len(), 2);
    }
}
