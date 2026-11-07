//! Table editing (ECMA-376 Part 1, §21.1.3): cell selection, cell text,
//! inserting/deleting rows and columns, merging/splitting cells
//! (§21.1.3.16 tc gridSpan/rowSpan/hMerge/vMerge), cell fill and the
//! table style options of `a:tblPr` (§21.1.3.15).

use super::geom::{BoxR, EMU_PT};
use super::text;
use bo_model::slide::{
    Fill, GridCol, ListStyle, Paragraph, RunProps, Table, TableCell, TableRow, TextBody, XmlElement,
};
use serde::Serialize;

/// Selected cell range (inclusive, grid coordinates).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TableSel {
    pub id: u32,
    pub r0: usize,
    pub c0: usize,
    pub r1: usize,
    pub c1: usize,
}

impl TableSel {
    #[must_use]
    pub fn norm(self) -> TableSel {
        TableSel {
            r0: self.r0.min(self.r1),
            r1: self.r0.max(self.r1),
            c0: self.c0.min(self.c1),
            c1: self.c0.max(self.c1),
            ..self
        }
    }
}

/// Rows × grid columns.
pub fn dims(t: &Table) -> (usize, usize) {
    (t.rows.len(), t.grid.len())
}

fn is_anchor(c: &TableCell) -> bool {
    c.h_merge != Some(true) && c.v_merge != Some(true)
}

fn span(c: &TableCell) -> (usize, usize) {
    (
        c.row_span.unwrap_or(1).max(1) as usize,
        c.grid_span.unwrap_or(1).max(1) as usize,
    )
}

/// Make every row hold one cell per grid column.
pub fn normalize(t: &mut Table) {
    let n = t.grid.len();
    for r in &mut t.rows {
        while r.cells.len() < n {
            r.cells.push(empty_cell(None));
        }
    }
}

/// Merge anchors: (row, col, row span, col span).
pub fn anchors(t: &Table) -> Vec<(usize, usize, usize, usize)> {
    let (rows, cols) = dims(t);
    let mut out = Vec::new();
    for (r, row) in t.rows.iter().enumerate() {
        for (c, cell) in row.cells.iter().enumerate().take(cols) {
            if is_anchor(cell) {
                let (rs, cs) = span(cell);
                out.push((r, c, rs.min(rows - r), cs.min(cols - c)));
            }
        }
    }
    out
}

/// The anchor covering grid cell (r, c).
pub fn anchor_of(t: &Table, r: usize, c: usize) -> (usize, usize) {
    anchors(t)
        .into_iter()
        .find(|&(ar, ac, rs, cs)| r >= ar && r < ar + rs && c >= ac && c < ac + cs)
        .map_or((r, c), |(ar, ac, _, _)| (ar, ac))
}

/// Grow a selection so it does not cut merged cells.
pub fn expand(t: &Table, s: TableSel) -> TableSel {
    let mut s = s.norm();
    loop {
        let mut grown = s;
        for (ar, ac, rs, cs) in anchors(t) {
            let (er, ec) = (ar + rs - 1, ac + cs - 1);
            let overlaps = ar <= s.r1 && er >= s.r0 && ac <= s.c1 && ec >= s.c0;
            if overlaps {
                grown.r0 = grown.r0.min(ar);
                grown.c0 = grown.c0.min(ac);
                grown.r1 = grown.r1.max(er);
                grown.c1 = grown.c1.max(ec);
            }
        }
        if grown == s {
            return s;
        }
        s = grown;
    }
}

/// Column x and row y edges in points.
pub fn edges(t: &Table) -> (Vec<f64>, Vec<f64>) {
    let mut xs = vec![0.0];
    for g in &t.grid {
        xs.push(xs.last().copied().unwrap_or(0.0) + g.w as f64 / EMU_PT);
    }
    let mut ys = vec![0.0];
    for r in &t.rows {
        ys.push(ys.last().copied().unwrap_or(0.0) + r.h as f64 / EMU_PT);
    }
    (xs, ys)
}

/// Grid cell at a point of the table box (points from its top-left corner).
pub fn cell_at(t: &Table, x: f64, y: f64) -> Option<(usize, usize)> {
    let (xs, ys) = edges(t);
    let c = xs.windows(2).position(|w| x >= w[0] && x < w[1])?;
    let r = ys.windows(2).position(|w| y >= w[0] && y < w[1])?;
    Some(anchor_of(t, r, c))
}

/// A cell as seen by the UI.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CellOut {
    pub row: usize,
    pub col: usize,
    pub row_span: usize,
    pub col_span: usize,
    pub rect: BoxR,
}

/// Table editing state for the UI.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TableOut {
    pub id: u32,
    pub rows: usize,
    pub cols: usize,
    /// Selection `[r0, c0, r1, c1]` (inclusive).
    pub sel: [usize; 4],
    pub cells: Vec<CellOut>,
    pub first_row: bool,
    pub band_row: bool,
    /// Text of the first selected cell (editing format).
    pub text: String,
}

/// UI state of a table placed in `frame` (the graphic frame's box).
pub fn table_out(t: &Table, frame: &BoxR, s: TableSel) -> TableOut {
    let (xs, ys) = edges(t);
    let cells = anchors(t)
        .into_iter()
        .map(|(r, c, rs, cs)| {
            let (x0, x1) = (xs[c], xs[(c + cs).min(xs.len() - 1)]);
            let (y0, y1) = (ys[r], ys[(r + rs).min(ys.len() - 1)]);
            let (cx, cy) = frame.to_slide(
                f64::midpoint(x0, x1) - frame.w / 2.0,
                f64::midpoint(y0, y1) - frame.h / 2.0,
            );
            CellOut {
                row: r,
                col: c,
                row_span: rs,
                col_span: cs,
                rect: BoxR {
                    cx,
                    cy,
                    w: x1 - x0,
                    h: y1 - y0,
                    rot: frame.rot,
                },
            }
        })
        .collect();
    let (flags, _) = bo_pptx::table_style::table_props(t.props.as_ref());
    let text = t
        .rows
        .get(s.r0)
        .and_then(|r| r.cells.get(s.c0))
        .and_then(|c| c.text.as_ref())
        .map(text::body_text)
        .unwrap_or_default();
    let (rows, cols) = dims(t);
    TableOut {
        id: s.id,
        rows,
        cols,
        sel: [s.r0, s.c0, s.r1, s.c1],
        cells,
        first_row: flags.first_row,
        band_row: flags.band_row,
        text,
    }
}

/// An empty cell, formatted like `like` (cell properties, paragraph mark).
pub fn empty_cell(like: Option<&TableCell>) -> TableCell {
    let end = like
        .and_then(|c| c.text.as_ref())
        .and_then(|t| t.paragraphs.first())
        .and_then(|p| p.end_props.clone())
        .unwrap_or_else(|| RunProps {
            lang: Some(super::build::LANG.into()),
            ..RunProps::default()
        });
    TableCell {
        text: Some(TextBody {
            list_style: Some(ListStyle::default()),
            paragraphs: vec![Paragraph {
                end_props: Some(end),
                ..Paragraph::default()
            }],
            ..TextBody::default()
        }),
        props: like.and_then(|c| c.props.clone()),
        ..TableCell::default()
    }
}

/// Set the text of cell (r, c).
pub fn set_cell_text(t: &mut Table, r: usize, c: usize, value: &str) -> bool {
    let Some(cell) = t.rows.get_mut(r).and_then(|row| row.cells.get_mut(c)) else {
        return false;
    };
    let tb = cell.text.get_or_insert_with(|| TextBody {
        list_style: Some(ListStyle::default()),
        ..TextBody::default()
    });
    text::set_text(tb, value);
    true
}

/// Insert a row at index `at` (0..=rows), formatted like its neighbour.
pub fn insert_row(t: &mut Table, at: usize) {
    normalize(t);
    let (rows, cols) = dims(t);
    let at = at.min(rows);
    let tpl = if at > 0 { at - 1 } else { 0 };
    let h = t.rows.get(tpl).map_or(370_840, |r| r.h);
    let mut cells = Vec::with_capacity(cols);
    let anchors = anchors(t);
    for c in 0..cols {
        // Inside a vertical merge: the new cell continues it.
        if let Some(&(ar, ac, rs, _)) = anchors
            .iter()
            .find(|&&(ar, ac, rs, cs)| ar < at && at < ar + rs && c >= ac && c < ac + cs)
        {
            if c == ac {
                if let Some(a) = t.rows.get_mut(ar).and_then(|row| row.cells.get_mut(ac)) {
                    a.row_span = Some((rs + 1) as u32);
                }
            }
            let like = t.rows.get(tpl).and_then(|row| row.cells.get(c)).cloned();
            let mut cell = empty_cell(like.as_ref());
            cell.v_merge = Some(true);
            cell.h_merge = (c > ac).then_some(true);
            cells.push(cell);
            continue;
        }
        let like = t.rows.get(tpl).and_then(|row| row.cells.get(c)).cloned();
        cells.push(empty_cell(like.as_ref()));
    }
    t.rows.insert(
        at,
        TableRow {
            h,
            cells,
            extra: Vec::new(),
        },
    );
}

/// Insert a column at grid index `at` (0..=cols), formatted like its neighbour.
pub fn insert_col(t: &mut Table, at: usize) {
    normalize(t);
    let (_, cols) = dims(t);
    let at = at.min(cols);
    let tpl = if at > 0 { at - 1 } else { 0 };
    let w = t.grid.get(tpl).map_or(914_400, |g| g.w);
    let anchors = anchors(t);
    for (r, row) in t.rows.iter_mut().enumerate() {
        let like = row.cells.get(tpl).cloned();
        let mut cell = empty_cell(like.as_ref());
        if let Some(&(ar, ac, _, cs)) = anchors
            .iter()
            .find(|&&(ar, ac, rs, cs)| ac < at && at < ac + cs && r >= ar && r < ar + rs)
        {
            cell.h_merge = Some(true);
            cell.v_merge = (r > ar).then_some(true);
            if r == ar {
                if let Some(a) = row.cells.get_mut(ac) {
                    a.grid_span = Some((cs + 1) as u32);
                }
            }
        }
        row.cells.insert(at.min(row.cells.len()), cell);
    }
    t.grid.insert(
        at,
        GridCol {
            w,
            extra: Vec::new(),
        },
    );
}

/// Delete rows `r0..=r1` (merged cells crossing them shrink or move down).
pub fn delete_rows(t: &mut Table, r0: usize, r1: usize) -> bool {
    normalize(t);
    let (rows, _) = dims(t);
    if r0 > r1 || r1 >= rows || r1 - r0 + 1 >= rows {
        return false;
    }
    let n = r1 - r0 + 1;
    for (ar, ac, rs, _) in anchors(t) {
        let end = ar + rs - 1;
        if rs == 1 || end < r0 || ar > r1 {
            continue;
        }
        let removed = end.min(r1) + 1 - ar.max(r0);
        let left = rs - removed;
        if ar >= r0 {
            // The anchor row goes: the first remaining row of the merge takes over.
            if left > 0 {
                let src = t.rows[ar].cells[ac].clone();
                let dst = &mut t.rows[r1 + 1].cells[ac];
                *dst = TableCell {
                    row_span: (left > 1).then_some(left as u32),
                    v_merge: None,
                    h_merge: None,
                    ..src
                };
            }
        } else {
            t.rows[ar].cells[ac].row_span = (left > 1).then_some(left as u32);
        }
    }
    t.rows.drain(r0..r0 + n);
    true
}

/// Delete grid columns `c0..=c1`.
pub fn delete_cols(t: &mut Table, c0: usize, c1: usize) -> bool {
    normalize(t);
    let (_, cols) = dims(t);
    if c0 > c1 || c1 >= cols || c1 - c0 + 1 >= cols {
        return false;
    }
    for (ar, ac, _, cs) in anchors(t) {
        let end = ac + cs - 1;
        if cs == 1 || end < c0 || ac > c1 {
            continue;
        }
        let removed = end.min(c1) + 1 - ac.max(c0);
        let left = cs - removed;
        if ac >= c0 {
            if left > 0 {
                let src = t.rows[ar].cells[ac].clone();
                let dst = &mut t.rows[ar].cells[c1 + 1];
                *dst = TableCell {
                    grid_span: (left > 1).then_some(left as u32),
                    h_merge: None,
                    v_merge: None,
                    ..src
                };
                // Rows below in the merge keep their vMerge continuation cells.
            }
        } else {
            t.rows[ar].cells[ac].grid_span = (left > 1).then_some(left as u32);
        }
    }
    for row in &mut t.rows {
        let end = (c1 + 1).min(row.cells.len());
        if c0 < end {
            row.cells.drain(c0..end);
        }
    }
    t.grid.drain(c0..=c1);
    true
}

/// Merge the cells of a range into its top-left cell (texts are joined).
pub fn merge(t: &mut Table, s: TableSel) -> bool {
    normalize(t);
    let s = expand(t, s);
    if s.r0 == s.r1 && s.c0 == s.c1 {
        return false;
    }
    let mut paras: Vec<Paragraph> = Vec::new();
    for (ar, ac, _, _) in anchors(t) {
        if ar < s.r0 || ar > s.r1 || ac < s.c0 || ac > s.c1 {
            continue;
        }
        if let Some(tb) = &t.rows[ar].cells[ac].text {
            if !tb.text().trim().is_empty() {
                paras.extend(tb.paragraphs.iter().cloned());
            }
        }
    }
    for r in s.r0..=s.r1 {
        for c in s.c0..=s.c1 {
            let cell = &mut t.rows[r].cells[c];
            cell.grid_span = None;
            cell.row_span = None;
            cell.h_merge = (c > s.c0).then_some(true);
            cell.v_merge = (r > s.r0).then_some(true);
            if (r, c) != (s.r0, s.c0) {
                let like = cell.clone();
                cell.text = empty_cell(Some(&like)).text;
            }
        }
    }
    let a = &mut t.rows[s.r0].cells[s.c0];
    a.grid_span = (s.c1 > s.c0).then_some((s.c1 - s.c0 + 1) as u32);
    a.row_span = (s.r1 > s.r0).then_some((s.r1 - s.r0 + 1) as u32);
    if !paras.is_empty() {
        if let Some(tb) = &mut a.text {
            tb.paragraphs = paras;
        }
    }
    true
}

/// Split merged cells inside a range back into single cells.
pub fn split(t: &mut Table, s: TableSel) -> bool {
    normalize(t);
    let s = s.norm();
    let mut done = false;
    for (ar, ac, rs, cs) in anchors(t) {
        if ar < s.r0 || ar > s.r1 || ac < s.c0 || ac > s.c1 || (rs == 1 && cs == 1) {
            continue;
        }
        for r in ar..ar + rs {
            for c in ac..ac + cs {
                let cell = &mut t.rows[r].cells[c];
                cell.grid_span = None;
                cell.row_span = None;
                cell.h_merge = None;
                cell.v_merge = None;
            }
        }
        done = true;
    }
    done
}

/// Fill of the selected cells (§21.1.3.17 tcPr fill; `None` = style fill).
pub fn set_fill(t: &mut Table, s: TableSel, fill: Option<&Fill>) {
    normalize(t);
    let s = s.norm();
    for r in s.r0..=s.r1.min(t.rows.len().saturating_sub(1)) {
        for c in s.c0..=s.c1.min(t.grid.len().saturating_sub(1)) {
            let cell = &mut t.rows[r].cells[c];
            let props = cell.props.get_or_insert_with(Default::default);
            props.fill = fill.cloned();
        }
    }
}

/// Set a boolean attribute of `a:tblPr` (`firstRow`, `bandRow`, …).
pub fn set_flag(t: &mut Table, name: &str, on: bool) {
    let p = t.props.get_or_insert_with(|| XmlElement::new("a:tblPr"));
    p.attrs.retain(|(k, _)| k != name);
    if on {
        // §21.1.3.15 attribute order is not significant; keep it stable.
        p.attrs.push((name.into(), "1".into()));
    }
}

/// Frame size of a table: grid width × row heights (EMU).
pub fn size(t: &Table) -> (i64, i64) {
    (
        t.grid.iter().map(|g| g.w).sum(),
        t.rows.iter().map(|r| r.h).sum(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table(rows: usize, cols: usize) -> Table {
        let mut t = Table {
            grid: (0..cols)
                .map(|_| GridCol {
                    w: 914_400,
                    extra: vec![],
                })
                .collect(),
            ..Table::default()
        };
        for r in 0..rows {
            t.rows.push(TableRow {
                h: 370_840,
                cells: (0..cols)
                    .map(|c| {
                        let mut cell = empty_cell(None);
                        let tb = cell.text.as_mut().unwrap();
                        text::set_text(tb, &format!("{r}{c}"));
                        cell
                    })
                    .collect(),
                extra: vec![],
            });
        }
        t
    }

    fn txt(t: &Table, r: usize, c: usize) -> String {
        t.rows[r].cells[c].text.as_ref().unwrap().text()
    }

    #[test]
    fn merge_split_and_selection_expansion() {
        let mut t = table(3, 3);
        let s = TableSel {
            id: 1,
            r0: 0,
            c0: 0,
            r1: 1,
            c1: 1,
        };
        assert!(merge(&mut t, s));
        assert_eq!(txt(&t, 0, 0), "00\n01\n10\n11");
        assert_eq!(t.rows[0].cells[0].grid_span, Some(2));
        assert_eq!(t.rows[0].cells[0].row_span, Some(2));
        assert_eq!(t.rows[0].cells[1].h_merge, Some(true));
        assert_eq!(t.rows[1].cells[0].v_merge, Some(true));
        assert_eq!(anchor_of(&t, 1, 1), (0, 0));
        // A selection touching the merge grows to include it.
        let e = expand(
            &t,
            TableSel {
                id: 1,
                r0: 1,
                c0: 1,
                r1: 2,
                c1: 2,
            },
        );
        assert_eq!((e.r0, e.c0, e.r1, e.c1), (0, 0, 2, 2));
        assert!(split(&mut t, s));
        assert!(t.rows.iter().all(|r| r.cells.iter().all(is_anchor)));
    }

    #[test]
    fn insert_and_delete_rows_and_columns_through_merges() {
        let mut t = table(3, 3);
        merge(
            &mut t,
            TableSel {
                id: 1,
                r0: 0,
                c0: 0,
                r1: 1,
                c1: 0,
            },
        );
        // A row inside the vertical merge extends it.
        insert_row(&mut t, 1);
        assert_eq!(dims(&t), (4, 3));
        assert_eq!(t.rows[0].cells[0].row_span, Some(3));
        assert_eq!(t.rows[1].cells[0].v_merge, Some(true));
        assert_eq!(txt(&t, 1, 1), "");
        insert_col(&mut t, 3);
        assert_eq!(dims(&t), (4, 4));
        assert!(t.rows.iter().all(|r| r.cells.len() == 4));
        // Deleting the anchor row moves the merge down.
        assert!(delete_rows(&mut t, 0, 0));
        assert_eq!(dims(&t), (3, 4));
        assert_eq!(t.rows[0].cells[0].row_span, Some(2));
        assert_eq!(t.rows[0].cells[0].v_merge, None);
        assert!(delete_cols(&mut t, 1, 2));
        assert_eq!(dims(&t), (3, 2));
        assert!(!delete_rows(&mut t, 0, 2), "cannot delete every row");
        assert_eq!(size(&t), (2 * 914_400, 3 * 370_840));
        set_flag(&mut t, "firstRow", true);
        set_flag(&mut t, "bandRow", false);
        let (f, _) = bo_pptx::table_style::table_props(t.props.as_ref());
        assert!(f.first_row && !f.band_row);
    }
}
