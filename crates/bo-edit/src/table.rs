//! Table editing (ECMA-376 Part 1, §17.4): rows and columns, merging and splitting
//! cells (§17.4.17 gridSpan, §17.4.85 vMerge), deleting the table, cell shading
//! (§17.4.33 shd), borders (§17.4.39 tblBorders, §17.4.67 tcBorders), table width and
//! alignment (§17.4.63 tblW, §17.4.29 jc), repeated header rows (§17.4.49 tblHeader)
//! and moving between cells with Tab.
//!
//! Every edit replaces the whole table block in one [`Op::Blocks`].

use crate::{id_of, EditError, Op, Pos, Result, Selection, Session, TableState};
use bo_model::index::{blocks_at, body_paragraphs};
use bo_model::{
    Alignment, Block, Border, Borders, Cell, CellProps, CellPropsChange, Color, Document,
    ParaProps, Paragraph, RevisionInfo, Row, RowProps, Table, TableProps, TablePropsChange,
    TableWidth, Twips, VMerge,
};
use serde::{Deserialize, Serialize};

/// Border presets of the table ribbon.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BorderPreset {
    #[default]
    All,
    None,
    Outside,
    Inside,
}

/// A table editing command.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum TableEdit {
    InsertRow {
        #[serde(default)]
        below: bool,
    },
    InsertColumn {
        #[serde(default)]
        right: bool,
    },
    DeleteRow,
    DeleteColumn,
    DeleteTable,
    /// Merge the selected cells into one (gridSpan / vMerge).
    MergeCells,
    /// Unmerge the caret's cell, or split an unmerged cell into two columns.
    SplitCell,
    /// Fill of the selected cells (`None` removes it).
    Shading {
        hex: Option<String>,
    },
    Borders {
        preset: BorderPreset,
    },
    /// Preferred width (`width_pct` percent of the text width, else `width` twips,
    /// neither = auto) and alignment.
    Properties {
        #[serde(default)]
        width_pct: Option<u32>,
        #[serde(default)]
        width: Option<i32>,
        #[serde(default)]
        align: Option<Alignment>,
    },
    /// Repeat the rows down to the selection as header rows on every page.
    HeaderRow {
        on: bool,
    },
}

/// Where a body paragraph sits in its innermost table.
#[derive(Debug, Clone, PartialEq, Eq)]
struct CellAt {
    /// Container path of the table block and its index there.
    container: Vec<usize>,
    index: usize,
    row: usize,
    cell: usize,
}

fn cell_at(doc: &Document, id: usize) -> Option<CellAt> {
    let paths = body_paragraphs(doc);
    let p = paths.get(id)?;
    if p.len() < 5 {
        return None;
    }
    let n = p.len();
    Some(CellAt {
        container: p[..n - 4].to_vec(),
        index: p[n - 4],
        row: p[n - 3],
        cell: p[n - 2],
    })
}

fn table_of<'d>(doc: &'d Document, at: &CellAt) -> Option<&'d Table> {
    match blocks_at(doc, &at.container)?.get(at.index)? {
        Block::Table(t) => Some(t),
        _ => None,
    }
}

/// Grid column where each cell of a row starts (§17.4.17 gridSpan).
fn starts(row: &Row) -> Vec<usize> {
    let mut g = 0;
    row.cells
        .iter()
        .map(|c| {
            let s = g;
            g += c.props.grid_span.max(1) as usize;
            s
        })
        .collect()
}

fn span(c: &Cell) -> usize {
    c.props.grid_span.max(1) as usize
}

/// Index of the cell covering grid column `g`.
fn cell_covering(row: &Row, g: usize) -> Option<usize> {
    starts(row)
        .iter()
        .zip(&row.cells)
        .position(|(&s, c)| s <= g && g < s + span(c))
}

fn empty_para_like(c: &Cell) -> Paragraph {
    let props = c
        .blocks
        .iter()
        .find_map(Block::as_paragraph)
        .map(|p| ParaProps {
            unknown: Vec::new(),
            mark_ins: None,
            mark_del: None,
            change: None,
            ..p.props.clone()
        })
        .unwrap_or_default();
    Paragraph::new(props, Vec::new())
}

/// An empty cell with the properties of `like`, spanning `cols` grid columns of `width`.
fn empty_cell(like: &Cell, cols: u32, width: Option<Twips>) -> Cell {
    let mut props = like.props.clone();
    props.grid_span = cols.max(1);
    props.v_merge = None;
    if let Some(w) = width {
        props.width = Some(TableWidth::Dxa(w));
    }
    Cell {
        props,
        blocks: vec![Block::Paragraph(empty_para_like(like))],
    }
}

fn is_empty_cell(c: &Cell) -> bool {
    c.blocks.iter().all(|b| match b {
        Block::Paragraph(p) => p.plain_text().is_empty() && p.runs.is_empty(),
        _ => false,
    })
}

/// Record tracked property changes of `t` against `old` (same rows and cells):
/// §17.13.5.34 tblPrChange and §17.13.5.36 tcPrChange keep the properties before the
/// first tracked change.
fn record_prop_changes(old: &Table, t: &mut Table, info: &RevisionInfo) {
    let bare_t = |p: &TableProps| TableProps {
        change: None,
        ..p.clone()
    };
    if t.props.change.is_none() && bare_t(&t.props) != bare_t(&old.props) {
        t.props.change = Some(Box::new(TablePropsChange {
            info: info.clone(),
            old: bare_t(&old.props),
        }));
    }
    let bare_c = |p: &CellProps| CellProps {
        change: None,
        ..p.clone()
    };
    for (ro, rn) in old.rows.iter().zip(&mut t.rows) {
        for (co, cn) in ro.cells.iter().zip(&mut rn.cells) {
            if cn.props.change.is_none() && bare_c(&cn.props) != bare_c(&co.props) {
                cn.props.change = Some(Box::new(CellPropsChange {
                    info: info.clone(),
                    old: bare_c(&co.props),
                }));
            }
        }
    }
}

/// Ensure the grid covers every row (malformed files can have short grids).
fn normalize_grid(t: &mut Table) {
    let cols = t
        .rows
        .iter()
        .map(|r| r.cells.iter().map(span).sum::<usize>())
        .max()
        .unwrap_or(0);
    if t.grid.len() < cols {
        let w = t.grid.last().copied().unwrap_or(Twips(1440));
        t.grid.resize(cols, w);
    }
}

fn nil_border() -> Border {
    Border {
        style: "nil".into(),
        size: 0,
        color: None,
        space: 0,
    }
}

impl Session {
    /// The table holding the caret.
    pub fn table_state(&self) -> Option<TableState> {
        let (a, b) = self.selection.ordered();
        let at = cell_at(&self.doc, a.para)?;
        let t = table_of(&self.doc, &at)?;
        let row = t.rows.get(at.row)?;
        let cell = row.cells.get(at.cell)?;
        let other = cell_at(&self.doc, b.para);
        let multi = other.as_ref().is_some_and(|o| {
            o.container == at.container
                && o.index == at.index
                && (o.row != at.row || o.cell != at.cell)
        });
        let (width_pct, width) = match t.props.width {
            Some(TableWidth::Pct(p)) => (Some(p / 50), 0),
            Some(TableWidth::Dxa(w)) => (None, w.0),
            _ => (None, 0),
        };
        Some(TableState {
            rows: t.rows.len(),
            cols: t.grid.len().max(row.cells.iter().map(span).sum()),
            row: at.row,
            col: starts(row).get(at.cell).copied().unwrap_or(0),
            header_row: row.props.header,
            shading: cell.props.shading.map(Color::to_hex),
            align: t.props.align.unwrap_or_default(),
            width_pct,
            width,
            multi_cell: multi,
            merged: span(cell) > 1 || cell.props.v_merge.is_some(),
        })
    }

    /// Selected cell rectangle `(rows, grid columns)` of the caret's table (inclusive).
    fn cell_rect(&self, at: &CellAt, t: &Table) -> ((usize, usize), (usize, usize)) {
        let b = self.selection.ordered().1;
        let range = |c: &CellAt| -> Option<(usize, usize, usize)> {
            let row = t.rows.get(c.row)?;
            let s = *starts(row).get(c.cell)?;
            Some((c.row, s, s + span(row.cells.get(c.cell)?) - 1))
        };
        let first = range(at).unwrap_or((at.row, 0, 0));
        let last = cell_at(&self.doc, b.para)
            .filter(|o| o.container == at.container && o.index == at.index)
            .and_then(|o| range(&o))
            .unwrap_or(first);
        (
            (first.0.min(last.0), first.0.max(last.0)),
            (first.1.min(last.1), first.2.max(last.2)),
        )
    }

    /// Apply a table command at the selection (one undo step).
    pub fn edit_table(&mut self, cmd: &TableEdit) -> Result<Selection> {
        let a = self.selection.ordered().0;
        let at = cell_at(&self.doc, a.para)
            .ok_or_else(|| EditError::Unsupported("the caret is not in a table".into()))?;
        let old = table_of(&self.doc, &at)
            .cloned()
            .ok_or(EditError::BadPath)?;
        let ((r0, r1), (g0, g1)) = self.cell_rect(&at, &old);
        // Track changes: deleted rows are marked, not removed (§17.13.5.12 del in trPr).
        if self.track {
            match cmd {
                TableEdit::DeleteRow => return self.track_delete_rows(&at, old, r0, r1),
                TableEdit::DeleteTable => {
                    let n = old.rows.len().saturating_sub(1);
                    return self.track_delete_rows(&at, old, 0, n);
                }
                _ => {}
            }
        }
        let mut t = old.clone();
        normalize_grid(&mut t);
        let base = t.clone();
        // Row inserted by this edit (tracked as an insertion, §17.13.5.19).
        let mut new_row: Option<usize> = None;
        let tracks_props = matches!(
            cmd,
            TableEdit::Shading { .. } | TableEdit::Borders { .. } | TableEdit::Properties { .. }
        );
        // Caret target after the edit: (row, cell) or None = the block replacing the table.
        let mut caret: Option<(usize, usize)> = Some((at.row, at.cell));
        let mut remove_table = false;
        let cur_start = starts(&t.rows[at.row]).get(at.cell).copied().unwrap_or(0);
        match cmd {
            TableEdit::InsertRow { below } => {
                let src_i = if *below { r1 } else { r0 };
                let src = t.rows[src_i].clone();
                let neighbour = if *below {
                    t.rows.get(src_i + 1)
                } else {
                    Some(&src)
                };
                let cells = src
                    .cells
                    .iter()
                    .zip(starts(&src))
                    .map(|(c, s)| {
                        let mut n = empty_cell(c, c.props.grid_span, None);
                        // Inside a vertical merge the new row continues it.
                        let cont = neighbour
                            .and_then(|r| cell_covering(r, s).map(|i| &r.cells[i]))
                            .is_some_and(|nc| nc.props.v_merge == Some(VMerge::Continue));
                        if cont {
                            n.props.v_merge = Some(VMerge::Continue);
                        }
                        n
                    })
                    .collect();
                let row = Row {
                    props: RowProps {
                        header: src.props.header && !below,
                        unknown: Vec::new(),
                        ..src.props.clone()
                    },
                    cells,
                };
                let ins = if *below { src_i + 1 } else { src_i };
                t.rows.insert(ins, row);
                new_row = Some(ins);
                caret = Some((ins, at.cell.min(t.rows[ins].cells.len() - 1)));
            }
            TableEdit::InsertColumn { right } => {
                let g = if *right { g1 + 1 } else { g0 };
                let w = t.grid.get(cur_start).copied().unwrap_or(Twips(1440));
                let total: i32 = t.grid.iter().map(|w| w.0).sum();
                insert_grid_column(&mut t, g, w, at.row);
                // Keep the table width: scale the columns back (§17.4.48 gridCol).
                if total > 0 {
                    scale(&mut t, total);
                }
                let new_cell = cell_covering(&t.rows[at.row], g).unwrap_or(at.cell);
                caret = Some((at.row, new_cell));
            }
            TableEdit::DeleteRow => {
                if r1 - r0 + 1 >= t.rows.len() {
                    remove_table = true;
                } else {
                    for g in 0..t.grid.len() {
                        // A deleted merge start passes on to the first row kept below.
                        let starts_merge = (r0..=r1).any(|r| {
                            cell_covering(&t.rows[r], g).is_some_and(|i| {
                                t.rows[r].cells[i].props.v_merge == Some(VMerge::Restart)
                            })
                        });
                        if let (true, Some(next)) = (starts_merge, t.rows.get_mut(r1 + 1)) {
                            if let Some(i) = cell_covering(next, g) {
                                if next.cells[i].props.v_merge == Some(VMerge::Continue) {
                                    next.cells[i].props.v_merge = Some(VMerge::Restart);
                                }
                            }
                        }
                    }
                    t.rows.drain(r0..=r1);
                    let r = r0.min(t.rows.len() - 1);
                    caret = Some((r, cell_covering(&t.rows[r], cur_start).unwrap_or(0)));
                }
            }
            TableEdit::DeleteColumn => {
                if g0 == 0 && g1 + 1 >= t.grid.len() {
                    remove_table = true;
                } else {
                    for row in &mut t.rows {
                        for g in (g0..=g1).rev() {
                            let Some(i) = cell_covering(row, g) else {
                                continue;
                            };
                            let c = &mut row.cells[i];
                            if c.props.grid_span > 1 {
                                c.props.grid_span -= 1;
                                if let Some(TableWidth::Dxa(w)) = &mut c.props.width {
                                    w.0 = (w.0 - t.grid[g].0).max(0);
                                }
                            } else {
                                row.cells.remove(i);
                            }
                        }
                    }
                    t.grid.drain(g0..=g1);
                    t.rows.retain(|r| !r.cells.is_empty());
                    let r = at.row.min(t.rows.len().saturating_sub(1));
                    caret = t
                        .rows
                        .get(r)
                        .map(|row| (r, cell_covering(row, g0.min(t.grid.len() - 1)).unwrap_or(0)));
                }
            }
            TableEdit::DeleteTable => remove_table = true,
            TableEdit::MergeCells => {
                if r0 == r1 && g0 == g1 {
                    return Ok(self.selection);
                }
                merge(&mut t, r0, r1, g0, g1)?;
                caret = Some((r0, cell_covering(&t.rows[r0], g0).unwrap_or(0)));
            }
            TableEdit::SplitCell => {
                let row = &t.rows[at.row];
                let c = row.cells[at.cell].clone();
                if span(&c) > 1 || c.props.v_merge.is_some() {
                    unmerge(&mut t, at.row, at.cell);
                } else {
                    // Split into two columns: a new grid column inside this one.
                    let w = t.grid[cur_start];
                    t.grid[cur_start] = Twips(w.0 - w.0 / 2);
                    t.grid.insert(cur_start + 1, Twips(w.0 / 2));
                    for (ri, row) in t.rows.iter_mut().enumerate() {
                        let Some(i) = cell_covering(row, cur_start) else {
                            continue;
                        };
                        if ri == at.row {
                            let half = Twips(w.0 / 2);
                            let mut n = empty_cell(&row.cells[i], 1, Some(half));
                            n.props.v_merge = None;
                            row.cells[i].props.width = Some(TableWidth::Dxa(Twips(w.0 - half.0)));
                            row.cells.insert(i + 1, n);
                        } else {
                            row.cells[i].props.grid_span = row.cells[i].props.grid_span.max(1) + 1;
                        }
                    }
                }
            }
            TableEdit::Shading { hex } => {
                let c = hex.as_deref().and_then(Color::from_hex);
                for r in r0..=r1 {
                    for g in g0..=g1 {
                        if let Some(i) = cell_covering(&t.rows[r], g) {
                            t.rows[r].cells[i].props.shading = c;
                        }
                    }
                }
            }
            TableEdit::Borders { preset } => {
                let on = Border::single(4);
                let off = nil_border();
                let pick = |b: bool| Some(if b { on.clone() } else { off.clone() });
                let (outer, inner) = match preset {
                    BorderPreset::All => (true, true),
                    BorderPreset::None => (false, false),
                    BorderPreset::Outside => (true, false),
                    BorderPreset::Inside => (false, true),
                };
                t.props.borders = Some(Borders {
                    top: pick(outer),
                    left: pick(outer),
                    bottom: pick(outer),
                    right: pick(outer),
                    inside_h: pick(inner),
                    inside_v: pick(inner),
                });
                for row in &mut t.rows {
                    for c in &mut row.cells {
                        c.props.borders = None;
                    }
                }
            }
            TableEdit::Properties {
                width_pct,
                width,
                align,
            } => {
                t.props.width = match (width_pct, width) {
                    (Some(p), _) => Some(TableWidth::Pct((*p).clamp(1, 500) * 50)),
                    (None, Some(w)) if *w > 0 => Some(TableWidth::Dxa(Twips(*w))),
                    _ => Some(TableWidth::Auto),
                };
                if let Some(a) = align {
                    t.props.align = Some(*a);
                    // §17.4.51 tblInd only applies to left-aligned tables.
                    if *a != Alignment::Left {
                        t.props.indent = None;
                    }
                }
            }
            TableEdit::HeaderRow { on } => {
                if *on {
                    for r in &mut t.rows[..=r1] {
                        r.props.header = true;
                    }
                } else {
                    for r in &mut t.rows[r0..] {
                        r.props.header = false;
                    }
                }
            }
        }
        if !remove_table && t == old {
            return Ok(self.selection);
        }
        let container = at.container.clone();
        let index = at.index;
        self.edit(move |tx, sel| {
            let blocks = blocks_at(tx.doc, &container).ok_or(EditError::BadPath)?;
            if remove_table {
                // A table never ends its container: keep a paragraph after it (§17.4.66).
                let last = index + 1 >= blocks.len();
                let insert = if last {
                    vec![Block::Paragraph(Paragraph::default())]
                } else {
                    Vec::new()
                };
                tx.push(Op::Blocks {
                    path: container.clone(),
                    index,
                    remove: vec![Block::Table(old)],
                    insert,
                })?;
                let mut path = container.clone();
                path.push(index);
                let id = id_of(tx.doc, &path)
                    .or_else(|| body_paragraphs(tx.doc).len().checked_sub(1))
                    .ok_or(EditError::BadPath)?;
                return Ok(Selection::caret(Pos::new(id, 0)));
            }
            let mut t = t;
            if let Some(tr) = tx.track.as_mut() {
                if let Some(i) = new_row {
                    t.rows[i].props.ins = Some(tr.mark());
                } else if tracks_props {
                    record_prop_changes(&base, &mut t, &tr.mark().info);
                }
            }
            tx.push(Op::Blocks {
                path: container.clone(),
                index,
                remove: vec![Block::Table(old)],
                insert: vec![Block::Table(t)],
            })?;
            let Some((r, c)) = caret else {
                return Ok(sel);
            };
            let mut path = container.clone();
            path.extend([index, r, c, 0]);
            Ok(id_of(tx.doc, &path).map_or(sel, |id| Selection::caret(Pos::new(id, 0))))
        })
    }

    /// Mark rows `r0..=r1` as deleted (§17.13.5.12); rows the author inserted are removed.
    fn track_delete_rows(
        &mut self,
        at: &CellAt,
        old: Table,
        r0: usize,
        r1: usize,
    ) -> Result<Selection> {
        let container = at.container.clone();
        let index = at.index;
        let author = self.author.clone();
        self.edit(move |tx, sel| {
            let mark = tx
                .track
                .as_mut()
                .map(crate::revisions::Tracker::mark)
                .ok_or(EditError::BadPath)?;
            let mut t = old.clone();
            let mut keep = Vec::with_capacity(t.rows.len());
            for (i, mut row) in std::mem::take(&mut t.rows).into_iter().enumerate() {
                if (r0..=r1).contains(&i) && row.props.del.is_none() {
                    let own = row
                        .props
                        .ins
                        .as_ref()
                        .is_some_and(|m| m.info.author == author);
                    if own {
                        continue;
                    }
                    row.props.del = Some(mark.clone());
                }
                keep.push(row);
            }
            t.rows = keep;
            let insert = if t.rows.is_empty() {
                let blocks = blocks_at(tx.doc, &container).ok_or(EditError::BadPath)?;
                if index + 1 >= blocks.len() {
                    vec![Block::Paragraph(Paragraph::default())]
                } else {
                    Vec::new()
                }
            } else {
                vec![Block::Table(t)]
            };
            tx.push(Op::Blocks {
                path: container.clone(),
                index,
                remove: vec![Block::Table(old)],
                insert,
            })?;
            Ok(sel)
        })?;
        self.fix_selection();
        Ok(self.selection)
    }

    /// Tab in a table: select the next (or previous) cell's text; Tab in the last cell
    /// adds a row. Returns `None` outside tables.
    pub fn next_cell(&mut self, back: bool) -> Result<Option<Selection>> {
        let focus = self.selection.focus;
        let Some(at) = cell_at(&self.doc, focus.para) else {
            return Ok(None);
        };
        let Some(t) = table_of(&self.doc, &at).cloned() else {
            return Ok(None);
        };
        let row = &t.rows[at.row];
        let target = if back {
            if at.cell > 0 {
                Some((at.row, at.cell - 1))
            } else if at.row > 0 {
                Some((at.row - 1, t.rows[at.row - 1].cells.len().saturating_sub(1)))
            } else {
                None
            }
        } else if at.cell + 1 < row.cells.len() {
            Some((at.row, at.cell + 1))
        } else if at.row + 1 < t.rows.len() {
            Some((at.row + 1, 0))
        } else {
            None
        };
        let Some((r, c)) = target else {
            if back {
                return Ok(Some(self.selection));
            }
            // Tab in the last cell: new row below, caret in its first cell.
            self.set_selection(Selection::caret(focus));
            self.edit_table(&TableEdit::InsertRow { below: true })?;
            let Some(at2) = cell_at(&self.doc, self.selection.focus.para) else {
                return Ok(Some(self.selection));
            };
            let mut path = at2.container.clone();
            path.extend([at2.index, at2.row, 0, 0]);
            if let Some(id) = id_of(&self.doc, &path) {
                self.selection = Selection::caret(Pos::new(id, 0));
                if let Some(tx) = self.undo.last_mut() {
                    tx.after = self.selection;
                }
            }
            return Ok(Some(self.selection));
        };
        let cell = &t.rows[r].cells[c];
        let mut first = at.container.clone();
        first.extend([at.index, r, c, 0]);
        let Some(start) = id_of(&self.doc, &first) else {
            return Ok(Some(self.selection));
        };
        let n = cell
            .blocks
            .iter()
            .filter(|b| matches!(b, Block::Paragraph(_)))
            .count()
            .max(1);
        let last = start + n - 1;
        let len = self.para_text(last).map_or(0, |s| s.len());
        self.set_selection(Selection {
            anchor: Pos::new(start, 0),
            focus: Pos::new(last, len),
        });
        Ok(Some(self.selection))
    }
}

/// Scale the grid (and fixed cell widths) to a total of `total` twips.
fn scale(t: &mut Table, total: i32) {
    let now: i32 = t.grid.iter().map(|w| w.0).sum();
    if now <= 0 || now == total {
        return;
    }
    let k = f64::from(total) / f64::from(now);
    for w in &mut t.grid {
        w.0 = (f64::from(w.0) * k).round() as i32;
    }
    let grid = t.grid.clone();
    for row in &mut t.rows {
        let st = starts(row);
        for (c, s) in row.cells.iter_mut().zip(st) {
            if let Some(TableWidth::Dxa(w)) = &mut c.props.width {
                let n = c.props.grid_span.max(1) as usize;
                *w = Twips(grid.iter().skip(s).take(n).map(|w| w.0).sum());
            }
        }
    }
}

/// Insert a grid column of width `w` before grid column `g` (`g` = grid length appends).
/// The row `own` always gets a new cell; other rows get one unless a spanning cell
/// crosses `g`, which is widened instead.
fn insert_grid_column(t: &mut Table, g: usize, w: Twips, own: usize) {
    let g = g.min(t.grid.len());
    t.grid.insert(g, w);
    for (ri, row) in t.rows.iter_mut().enumerate() {
        let st = starts(row);
        let total: usize = row.cells.iter().map(span).sum();
        if g >= total {
            let like = row.cells.last().cloned().unwrap_or(Cell {
                props: bo_model::CellProps::default(),
                blocks: Vec::new(),
            });
            row.cells.push(empty_cell(&like, 1, Some(w)));
            continue;
        }
        let Some(i) = st.iter().position(|&s| s == g) else {
            // Inside a spanning cell.
            if let Some(i) = cell_covering(row, g) {
                if ri == own {
                    let like = row.cells[i].clone();
                    row.cells.insert(i + 1, empty_cell(&like, 1, Some(w)));
                } else {
                    row.cells[i].props.grid_span = row.cells[i].props.grid_span.max(1) + 1;
                }
            }
            continue;
        };
        let like = row.cells[i].clone();
        let mut n = empty_cell(&like, 1, Some(w));
        // Continue a vertical merge started above in the same place.
        n.props.v_merge = None;
        row.cells.insert(i, n);
    }
}

/// Merge grid rectangle rows `r0..=r1`, columns `g0..=g1` (§17.4.17, §17.4.85).
fn merge(t: &mut Table, r0: usize, r1: usize, g0: usize, g1: usize) -> Result<()> {
    let mut moved: Vec<Block> = Vec::new();
    for r in r0..=r1 {
        let row = &mut t.rows[r];
        let (Some(i), Some(j)) = (cell_covering(row, g0), cell_covering(row, g1)) else {
            return Err(EditError::Unsupported(
                "cells do not form a rectangle".into(),
            ));
        };
        let st = starts(row);
        if st[i] != g0 || st[j] + span(&row.cells[j]) - 1 != g1 {
            return Err(EditError::Unsupported(
                "cells do not form a rectangle".into(),
            ));
        }
        let mut first = row.cells[i].clone();
        for c in row.cells.drain(i + 1..=j) {
            if !is_empty_cell(&c) {
                first.blocks.extend(c.blocks);
            }
        }
        first.props.grid_span = (g1 - g0 + 1) as u32;
        if let Some(TableWidth::Dxa(_)) = first.props.width {
            first.props.width = Some(TableWidth::Dxa(Twips(
                t.grid.iter().skip(g0).take(g1 - g0 + 1).map(|w| w.0).sum(),
            )));
        }
        if r1 > r0 {
            if r == r0 {
                first.props.v_merge = Some(VMerge::Restart);
            } else {
                first.props.v_merge = Some(VMerge::Continue);
                if !is_empty_cell(&first) {
                    moved.append(&mut first.blocks);
                }
                first.blocks = vec![Block::Paragraph(empty_para_like(&row.cells[i]))];
            }
        }
        row.cells[i] = first;
    }
    if !moved.is_empty() {
        let row = &mut t.rows[r0];
        if let Some(i) = cell_covering(row, g0) {
            let c = &mut row.cells[i];
            if is_empty_cell(c) {
                c.blocks = moved;
            } else {
                c.blocks.extend(moved);
            }
        }
    }
    Ok(())
}

/// Undo merges of cell (`r`, `c`): split its grid span into single cells and end its
/// vertical merge.
fn unmerge(t: &mut Table, r: usize, c: usize) {
    let g = starts(&t.rows[r]).get(c).copied().unwrap_or(0);
    let n = span(&t.rows[r].cells[c]);
    let grid = t.grid.clone();
    let width_of = |k: usize| grid.get(k).copied().unwrap_or(Twips(1440));
    // Rows of the vertical merge (this one and the continuing rows below).
    let mut rows = vec![r];
    if t.rows[r].cells[c].props.v_merge.is_some() {
        for (ri, row) in t.rows.iter().enumerate().skip(r + 1) {
            match cell_covering(row, g).map(|i| row.cells[i].props.v_merge) {
                Some(Some(VMerge::Continue)) => rows.push(ri),
                _ => break,
            }
        }
    }
    for ri in rows {
        let row = &mut t.rows[ri];
        let Some(i) = cell_covering(row, g) else {
            continue;
        };
        row.cells[i].props.v_merge = None;
        if n > 1 {
            row.cells[i].props.grid_span = 1;
            row.cells[i].props.width = Some(TableWidth::Dxa(width_of(g)));
            let like = row.cells[i].clone();
            for k in 1..n {
                row.cells
                    .insert(i + k, empty_cell(&like, 1, Some(width_of(g + k))));
            }
        }
    }
}
