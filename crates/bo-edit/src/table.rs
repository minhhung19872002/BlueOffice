//! Table editing (ECMA-376 Part 1, §17.4): rows and columns, merging and splitting
//! cells (§17.4.17 gridSpan, §17.4.85 vMerge), deleting the table, cell shading
//! (§17.4.33 shd), borders (§17.4.39 tblBorders, §17.4.67 tcBorders), table width and
//! alignment (§17.4.63 tblW, §17.4.29 jc), repeated header rows (§17.4.49 tblHeader)
//! and moving between cells with Tab.
//!
//! Every edit replaces the whole table block in one [`Op::Blocks`].

use crate::para;
use crate::revisions::Tracker;
use crate::table_tools::{self as tools, AutoFit, SortKey};
use crate::{id_of, EditError, Op, Pos, Result, Selection, Session, TableState};
use bo_model::index::{blocks_at, body_paragraphs};
use bo_model::{
    Alignment, Block, Border, Borders, Cell, CellMerge, CellPropsChange, Color, Document,
    GridChange, ParaProps, Paragraph, RevMark, Row, RowProps, Table, TableProps, TablePropsChange,
    TableView, TableWidth, TextDirection, Twips, VMerge,
};
use serde::{Deserialize, Serialize};

/// Border presets of the table ribbon.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BorderPreset {
    /// Outer edges and inner gridlines.
    #[default]
    All,
    /// No borders.
    None,
    /// Outer edges of the table only.
    Outside,
    /// Inner gridlines only.
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
    /// Insert a row next to the caret's row.
    InsertRow {
        /// Insert below (else above).
        #[serde(default)]
        below: bool,
    },
    /// Insert a column next to the caret's column.
    InsertColumn {
        /// Insert to the right (else left).
        #[serde(default)]
        right: bool,
    },
    /// Delete the selected rows.
    DeleteRow,
    /// Delete the selected columns.
    DeleteColumn,
    /// Insert one cell left of the selected cells (`right`: after them) in each
    /// selected row; the following cells shift right.
    InsertCell {
        /// Insert after (right of) the selected cells instead of before them.
        #[serde(default)]
        right: bool,
    },
    /// Delete the selected cells; the following cells shift left.
    DeleteCell,
    /// Delete the whole table.
    DeleteTable,
    /// Merge the selected cells into one (gridSpan / vMerge).
    MergeCells,
    /// Unmerge the caret's cell, or split an unmerged cell into two columns.
    SplitCell,
    /// Fill of the selected cells (`None` removes it).
    Shading {
        /// Fill colour `RRGGBB`; `None` removes it.
        hex: Option<String>,
    },
    /// Apply a border preset to the table (`w:tblBorders`).
    Borders {
        /// Which edges get borders.
        preset: BorderPreset,
    },
    /// Preferred width (`width_pct` percent of the text width, else `width` twips,
    /// neither = auto) and alignment.
    Properties {
        /// Width in percent of the text width.
        #[serde(default)]
        width_pct: Option<u32>,
        /// Width in twips (used when `width_pct` is `None`).
        #[serde(default)]
        width: Option<i32>,
        /// Table alignment.
        #[serde(default)]
        align: Option<Alignment>,
    },
    /// Repeat the rows down to the selection as header rows on every page.
    HeaderRow {
        /// Turn header rows on or off.
        on: bool,
    },
    /// Give the selected rows (all rows for a single cell) the same height (§17.4.81
    /// trHeight). `heights`: the measured height of every row, in twips (filled in by
    /// the engine from the layout; empty = use the rows' own heights).
    DistributeRows {
        /// Measured row heights in twips, one per row.
        #[serde(default)]
        heights: Vec<i32>,
    },
    /// Give the selected grid columns (all for a single cell) the same width.
    DistributeColumns,
    /// AutoFit (§17.4.53 tblLayout, §17.4.63 tblW). `widths`: content widths per grid
    /// column in twips for [`AutoFit::Contents`] (filled in by the engine).
    AutoFit {
        /// How to size the columns.
        mode: AutoFit,
        /// Content widths per grid column, in twips.
        #[serde(default)]
        widths: Vec<i32>,
    },
    /// Sort the selected rows (all rows for a single cell) by up to three keys.
    Sort {
        /// Sort keys, most significant first.
        keys: Vec<SortKey>,
        /// The first row of the sorted range is a header and stays in place.
        #[serde(default)]
        header: bool,
    },
    /// Text direction of the selected cells (§17.4.72); `None` = horizontal.
    TextDirection {
        /// New direction.
        #[serde(default)]
        dir: Option<TextDirection>,
    },
    /// Let the text wrap around the table (§17.4.58 tblpPr) or keep it in line.
    Wrap {
        /// Text wraps around the table.
        around: bool,
    },
    /// Replace the table by paragraphs, cells joined by `separator` (`"\t"`, `","`,
    /// `"\n"` = a paragraph per cell, or any text).
    ToText {
        /// Text between the cells of a row.
        separator: String,
    },
    /// Turn the selected paragraphs into a table (the caret need not be in a table):
    /// cells split at `separator`, or with `"\n"` one cell per paragraph and `cols`
    /// cells per row.
    FromText {
        /// Text between the cells of a row.
        separator: String,
        /// Number of columns (paragraph separator only).
        #[serde(default)]
        cols: Option<usize>,
    },
}

/// Where a body paragraph sits in its innermost table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CellAt {
    /// Container path of the table block and its index there.
    pub(crate) container: Vec<usize>,
    pub(crate) index: usize,
    pub(crate) row: usize,
    pub(crate) cell: usize,
}

pub(crate) fn cell_at(doc: &Document, id: usize) -> Option<CellAt> {
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

pub(crate) fn table_of<'d>(doc: &'d Document, at: &CellAt) -> Option<&'d Table> {
    match blocks_at(doc, &at.container)?.get(at.index)? {
        Block::Table(t) => Some(t),
        _ => None,
    }
}

/// Grid column where each cell of a row starts (§17.4.17 gridSpan).
pub(crate) fn starts(row: &Row) -> Vec<usize> {
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

pub(crate) fn span(c: &Cell) -> usize {
    c.props.grid_span.max(1) as usize
}

/// Index of the cell covering grid column `g`.
pub(crate) fn cell_covering(row: &Row, g: usize) -> Option<usize> {
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
    let mut props = like.props.bare();
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

/// Cells created by an edit: (row, index of the cell in the new row).
type Inserted = Vec<(usize, usize)>;

/// Record the changes from `old` to `t` (same rows; `inserted` cells are new) as
/// tracked changes of `mark` (ECMA-376 Part 1, §17.13.5): new cells §17.13.5.4
/// cellIns, vertical merges §17.13.5.6 cellMerge, cell properties §17.13.5.36
/// tcPrChange, row properties §17.13.5.37 trPrChange, table properties §17.13.5.34
/// tblPrChange and the grid §17.13.5.33 tblGridChange. An existing change keeps the
/// state before the first tracked change.
fn record_tracked(old: &Table, t: &mut Table, inserted: &[(usize, usize)], mark: &RevMark) {
    let info = &mark.info;
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
    if t.grid != old.grid && t.grid_change.is_none() {
        t.grid_change = Some(Box::new(GridChange {
            info: info.clone(),
            old: old.grid.clone(),
        }));
    }
    for (ri, (ro, rn)) in old.rows.iter().zip(&mut t.rows).enumerate() {
        if rn.props.change.is_none() && !rn.props.same_base(&ro.props) {
            rn.props.change = Some(Box::new(ro.props.snapshot(info.clone())));
        }
        let mut olds = ro.cells.iter();
        for (ci, cn) in rn.cells.iter_mut().enumerate() {
            if inserted.contains(&(ri, ci)) {
                cn.props = cn.props.bare();
                cn.props.ins = Some(mark.clone());
                continue;
            }
            let Some(co) = olds.next() else { continue };
            if cn.props.v_merge != co.props.v_merge && cn.props.merge.is_none() {
                cn.props.merge = Some(Box::new(CellMerge {
                    info: info.clone(),
                    v_merge: cn.props.v_merge,
                    v_merge_orig: co.props.v_merge,
                }));
            }
            let (mut a, mut b) = (cn.props.bare(), co.props.bare());
            a.v_merge = None;
            b.v_merge = None;
            if cn.props.change.is_none() && a != b {
                cn.props.change = Some(Box::new(CellPropsChange {
                    info: info.clone(),
                    old: co.props.bare(),
                }));
            }
        }
    }
}

/// Mark the content of a cell deleted (§17.13.5.14 del, §17.13.5.15 on paragraph
/// marks but the last, §17.13.5.12 for rows of nested tables); the author's own
/// insertions are removed.
fn mark_blocks_deleted(blocks: &mut [Block], tr: &Tracker, mark: &RevMark) {
    let last = blocks
        .iter()
        .rposition(|b| matches!(b, Block::Paragraph(_)));
    for (i, b) in blocks.iter_mut().enumerate() {
        match b {
            Block::Paragraph(p) => {
                let mut atoms = para::flatten(p);
                let l = para::len(&atoms);
                crate::revisions::mark_deleted(&mut atoms, 0, l, tr, mark);
                let mut q = para::with_atoms(p, atoms);
                if Some(i) != last && q.props.mark_del.is_none() {
                    q.props.mark_del = Some(mark.clone());
                }
                *p = q;
            }
            Block::Table(t) => {
                for r in &mut t.rows {
                    if r.props.del.is_none() {
                        r.props.del = Some(mark.clone());
                    }
                }
            }
            Block::Opaque(_) => {}
        }
    }
}

/// Copy of `blocks` marked inserted by `mark` (§17.13.5.18 ins on runs, §17.13.5.16
/// on paragraph marks, §17.13.5.19 on rows of nested tables).
fn mark_blocks_inserted(blocks: &[Block], mark: &RevMark) -> Vec<Block> {
    blocks
        .iter()
        .map(|b| match b {
            Block::Paragraph(p) => {
                let mut q = p.clone();
                q.src = None;
                for r in &mut q.runs {
                    r.props.ins = Some(mark.clone());
                }
                q.props.mark_ins = Some(mark.clone());
                Block::Paragraph(q)
            }
            Block::Table(t) => {
                let mut t = t.clone();
                t.src = None;
                for r in &mut t.rows {
                    r.props.ins = Some(mark.clone());
                }
                Block::Table(t)
            }
            Block::Opaque(_) => b.clone(),
        })
        .collect()
}

/// `target` followed by the tracked insertion of `moved` (content merged into a
/// cell). The last inserted paragraph takes over the mark of the cell's last
/// paragraph, whose own mark becomes inserted, so rejecting restores `target`
/// exactly (§17.13.5.16: a paragraph whose mark is removed joins the next one).
fn append_inserted(target: &[Block], moved: &[Block], mark: &RevMark) -> Vec<Block> {
    let mut add = mark_blocks_inserted(moved, mark);
    let (Some(Block::Paragraph(last_old)), Some(Block::Paragraph(last_new))) =
        (target.last(), add.last_mut())
    else {
        let mut v = target.to_vec();
        v.extend(add);
        return v;
    };
    last_new.props = last_old.props.clone();
    last_new.src.clone_from(&last_old.src);
    let mut out = target[..target.len() - 1].to_vec();
    if !(target.len() == 1 && last_old.runs.is_empty()) {
        let mut first = last_old.clone();
        first.props.mark_ins = Some(mark.clone());
        out.push(Block::Paragraph(first));
    }
    out.extend(add);
    out
}

/// Ensure the grid covers every row (malformed files can have short grids).
///
/// Rows are measured as they are once every tracked change is accepted and once
/// every one is rejected (§17.13.5.3 cellDel, §17.13.5.4 cellIns): the markup
/// view shows inserted and deleted cells side by side and can be wider than
/// either real table, which must not widen the grid.
fn normalize_grid(t: &mut Table) {
    let width = |g: &bo_model::TableGeometry| {
        g.rows
            .iter()
            .map(|r| {
                r.iter()
                    .filter(|p| !p.hidden)
                    .map(|p| p.start + p.span)
                    .max()
                    .unwrap_or(0)
            })
            .max()
            .unwrap_or(0)
    };
    let cols = width(&t.geometry(TableView::Final)).max(width(&t.geometry(TableView::Original)));
    if t.grid.len() < cols {
        let w = t.grid.last().copied().unwrap_or(Twips(1440));
        t.grid.resize(cols, w);
    }
}

/// Rows a sort covers: the selected rows, or every row below the repeated header rows
/// (§17.4.49 tblHeader) when the selection is in one row; `header` keeps the first of
/// them in place.
fn sort_range(t: &Table, r0: usize, r1: usize, header: bool) -> (usize, usize) {
    let (mut lo, hi) = if r0 == r1 {
        let heads = t.rows.iter().take_while(|r| r.props.header).count();
        (heads, t.rows.len().saturating_sub(1))
    } else {
        (r0, r1)
    };
    if header {
        lo += 1;
    }
    (lo, hi)
}

/// Width available to a table in `container` (twips): the section's text width for a
/// top-level table, the grid width of the enclosing cell (less the default cell
/// margins, §17.4.42 tblCellMar) for a nested one.
fn avail_width(doc: &Document, container: &[usize]) -> Twips {
    let section = container.first().copied().unwrap_or(0);
    let page = doc.sections.get(section).map_or(Twips(9355), |s| {
        Twips(s.page.width.0 - s.page.margin_left.0 - s.page.margin_right.0)
    });
    let n = container.len();
    if n < 4 {
        return page;
    }
    let parent = blocks_at(doc, &container[..n - 3]).and_then(|b| match b.get(container[n - 3]) {
        Some(Block::Table(t)) => Some(t),
        _ => None,
    });
    let Some(t) = parent else { return page };
    let Some(row) = t.rows.get(container[n - 2]) else {
        return page;
    };
    let (Some(&s), Some(c)) = (
        starts(row).get(container[n - 1]),
        row.cells.get(container[n - 1]),
    ) else {
        return page;
    };
    let w: i32 = t.grid.iter().skip(s).take(span(c)).map(|g| g.0).sum();
    if w > 216 {
        Twips(w - 216)
    } else {
        page
    }
}

impl Session {
    /// The table holding the caret and the width available to it (twips), for
    /// measuring it before a table tool.
    pub fn caret_table(&self) -> Option<(&Table, Twips)> {
        let a = self.selection.ordered().0;
        let at = cell_at(&self.doc, a.para)?;
        let t = table_of(&self.doc, &at)?;
        Some((t, avail_width(&self.doc, &at.container)))
    }

    /// Turn the selected paragraphs (side by side in one container) into a table.
    fn text_to_table(&mut self, sep: &str, cols: Option<usize>) -> Result<Selection> {
        if self.track {
            return Err(EditError::Unsupported(
                "converting text to a table is not recorded as a tracked change".into(),
            ));
        }
        let (a, b) = self.selection.ordered();
        let paths = body_paragraphs(&self.doc);
        let pa = paths.get(a.para).ok_or(EditError::BadPosition)?;
        let pb = paths.get(b.para).ok_or(EditError::BadPosition)?;
        let (prefix, i0) = crate::split_path(pa);
        let (prefix_b, mut i1) = crate::split_path(pb);
        if prefix != prefix_b {
            return Err(EditError::Unsupported(
                "select paragraphs that follow each other".into(),
            ));
        }
        // A selection ending at the start of a (non-empty) paragraph does not include it.
        let ends_before = b.offset == 0 && self.para_text(b.para).is_some_and(|t| !t.is_empty());
        if ends_before && i1 > i0 {
            i1 -= 1;
        }
        let blocks = blocks_at(&self.doc, &prefix).ok_or(EditError::BadPath)?;
        let removed: Vec<Block> = blocks.get(i0..=i1).ok_or(EditError::BadPath)?.to_vec();
        let paras: Vec<Paragraph> = removed
            .iter()
            .map(|b| b.as_paragraph().cloned())
            .collect::<Option<_>>()
            .ok_or_else(|| EditError::Unsupported("the selection holds a table".into()))?;
        let need_after = i1 + 1 >= blocks.len();
        let width = avail_width(&self.doc, &prefix);
        let table = tools::paragraphs_to_table(&paras, sep, cols, width);
        self.edit(move |tx, _| {
            let mut insert = vec![Block::Table(table)];
            // A table never ends its container (§17.4.66).
            if need_after {
                insert.push(Block::Paragraph(Paragraph::default()));
            }
            tx.push(Op::Blocks {
                path: prefix.clone(),
                index: i0,
                remove: removed,
                insert,
            })?;
            let mut path = prefix;
            path.extend([i0, 0, 0, 0]);
            let id = id_of(tx.doc, &path).ok_or(EditError::BadPath)?;
            Ok(Selection::caret(Pos::new(id, 0)))
        })
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
            direction: cell.props.text_direction,
            wrap: t.props.float.is_some(),
            fixed_layout: t.props.fixed_layout,
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
        if let TableEdit::FromText { separator, cols } = cmd {
            return self.text_to_table(separator, *cols);
        }
        let a = self.selection.ordered().0;
        let at = cell_at(&self.doc, a.para)
            .ok_or_else(|| EditError::Unsupported("the caret is not in a table".into()))?;
        let old = table_of(&self.doc, &at)
            .cloned()
            .ok_or(EditError::BadPath)?;
        let ((r0, r1), (g0, g1)) = self.cell_rect(&at, &old);
        match cmd {
            TableEdit::ToText { .. } | TableEdit::Sort { .. } if self.track => {
                return Err(EditError::Unsupported(
                    "this table command is not recorded as a tracked change".into(),
                ));
            }
            TableEdit::ToText { separator } => {
                let blocks = tools::table_to_blocks(&old, separator);
                let container = at.container.clone();
                let index = at.index;
                return self.edit(move |tx, _| {
                    tx.push(Op::Blocks {
                        path: container.clone(),
                        index,
                        remove: vec![Block::Table(old)],
                        insert: blocks,
                    })?;
                    let mut path = container;
                    path.push(index);
                    let id = id_of(tx.doc, &path).ok_or(EditError::BadPath)?;
                    Ok(Selection::caret(Pos::new(id, 0)))
                });
            }
            TableEdit::Sort { header, .. } => {
                let (lo, hi) = sort_range(&old, r0, r1, *header);
                if let Some(why) = tools::sort_blocker(&old, lo, hi) {
                    return Err(EditError::Unsupported(why.into()));
                }
            }
            _ => {}
        }
        // Track changes: deleted rows are marked, not removed (§17.13.5.12 del in trPr).
        if self.track {
            match cmd {
                TableEdit::DeleteRow => return self.track_delete_rows(&at, old, r0, r1),
                TableEdit::DeleteTable => {
                    let n = old.rows.len().saturating_sub(1);
                    return self.track_delete_rows(&at, old, 0, n);
                }
                // Deleted columns and cells are marked (§17.13.5.3 cellDel).
                TableEdit::DeleteColumn => {
                    if g0 == 0 && g1 + 1 >= old.grid.len() {
                        let n = old.rows.len().saturating_sub(1);
                        return self.track_delete_rows(&at, old, 0, n);
                    }
                    let n = old.rows.len().saturating_sub(1);
                    return self.track_delete_cells(&at, old, (0, n), (g0, g1));
                }
                TableEdit::DeleteCell => {
                    return self.track_delete_cells(&at, old, (r0, r1), (g0, g1));
                }
                TableEdit::MergeCells => {
                    if r0 == r1 && g0 == g1 {
                        return Ok(self.selection);
                    }
                    return self.track_merge(&at, old, (r0, r1), (g0, g1));
                }
                _ => {}
            }
        }
        let mut t = old.clone();
        normalize_grid(&mut t);
        let base = t.clone();
        // Row inserted by this edit (tracked as an insertion, §17.13.5.19).
        let mut new_row: Option<usize> = None;
        // Cells inserted by this edit (tracked as §17.13.5.4 cellIns).
        let mut inserted: Inserted = Vec::new();
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
                    // A cell awaiting deletion (§17.13.5.3 cellDel, e.g. merged away by a
                    // tracked merge) is not part of the row being copied: the new row
                    // spans the grid like the row will once the change is accepted.
                    .filter(|(c, _)| c.props.del.is_none())
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
                    .collect::<Vec<_>>();
                // ECMA-376 Part 1, §17.4.79 tr holds at least one tc: a row whose cells all
                // await deletion gets one empty cell spanning the grid.
                let mut cells = cells;
                if cells.is_empty() {
                    if let Some(like) = src.cells.first() {
                        let cols = u32::try_from(t.grid.len()).unwrap_or(u32::MAX);
                        cells.push(empty_cell(like, cols, None));
                    }
                }
                let row = Row {
                    props: RowProps {
                        header: src.props.header && !below,
                        unknown: Vec::new(),
                        ins: None,
                        del: None,
                        change: None,
                        // §17.5.2.32 row-level controls stay on the rows they were on.
                        controls: Vec::new(),
                        controls_end: 0,
                        ..src.props.clone()
                    },
                    cells,
                };
                let ins = if *below { src_i + 1 } else { src_i };
                t.rows.insert(ins, row);
                new_row = Some(ins);
                caret = Some((ins, at.cell.min(t.rows[ins].cells.len().saturating_sub(1))));
            }
            TableEdit::InsertColumn { right } => {
                let g = if *right { g1 + 1 } else { g0 };
                let w = t.grid.get(cur_start).copied().unwrap_or(Twips(1440));
                let total: i32 = t.grid.iter().map(|w| w.0).sum();
                inserted = insert_grid_column(&mut t, g, w, at.row);
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
                    crate::row_controls::drain_rows_keep_controls(&mut t, r0, r1);
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
            TableEdit::InsertCell { right } => {
                // One cell per selected row, before the selection (or after it).
                let w = t.grid.get(cur_start).copied().unwrap_or(Twips(1440));
                for r in r0..=r1 {
                    let row = &mut t.rows[r];
                    let i = if *right {
                        cell_covering(row, g1).map_or(row.cells.len(), |i| i + 1)
                    } else {
                        cell_covering(row, g0).unwrap_or(row.cells.len())
                    };
                    let like = row
                        .cells
                        .get(i.min(row.cells.len().saturating_sub(1)))
                        .cloned()
                        .unwrap_or_default();
                    row.cells.insert(i, empty_cell(&like, 1, Some(w)));
                    inserted.push((r, i));
                    if r == at.row {
                        caret = Some((r, i));
                    }
                }
                normalize_grid(&mut t);
            }
            TableEdit::DeleteCell => {
                for r in (r0..=r1).rev() {
                    let row = &mut t.rows[r];
                    let mut gone: Vec<usize> =
                        (g0..=g1).filter_map(|g| cell_covering(row, g)).collect();
                    gone.dedup();
                    for i in gone.into_iter().rev() {
                        row.cells.remove(i);
                    }
                    if row.cells.is_empty() {
                        t.rows.remove(r);
                    }
                }
                if t.rows.is_empty() {
                    remove_table = true;
                } else {
                    let r = at.row.min(t.rows.len() - 1);
                    caret = Some((r, at.cell.min(t.rows[r].cells.len().saturating_sub(1))));
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
                    inserted = unmerge(&mut t, at.row, at.cell);
                } else {
                    // Split into two columns: a new grid column inside this one. A ragged row
                    // can reach past the grid (§17.4.49 tblGrid is only a hint); extend it first.
                    while t.grid.len() <= cur_start {
                        t.grid.push(Twips(1440));
                    }
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
                            inserted.push((ri, i + 1));
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
            TableEdit::DistributeRows { heights } => {
                tools::distribute_rows(&mut t, r0, r1, heights);
            }
            TableEdit::DistributeColumns => tools::distribute_columns(&mut t, g0, g1),
            TableEdit::AutoFit { mode, widths } => {
                let avail = avail_width(&self.doc, &at.container);
                tools::auto_fit(&mut t, *mode, widths, avail);
            }
            TableEdit::Sort { keys, header } => {
                let (lo, hi) = sort_range(&t, r0, r1, *header);
                tools::sort_rows(&mut t, lo, hi, keys);
            }
            TableEdit::TextDirection { dir } => {
                tools::set_direction(&mut t, (r0, r1), (g0, g1), *dir);
            }
            TableEdit::Wrap { around } => tools::set_wrap(&mut t, *around),
            // Handled before the table is copied.
            TableEdit::ToText { .. } | TableEdit::FromText { .. } => {}
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
                } else {
                    record_tracked(&base, &mut t, &inserted, &tr.mark());
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

    /// Mark the cells of rows `r0..=r1` lying within grid columns `g0..=g1` as deleted
    /// (§17.13.5.3 cellDel; a deleted column is a deleted cell in every row). Cells
    /// spanning past the range are kept: accepting the deletion narrows them.
    fn track_delete_cells(
        &mut self,
        at: &CellAt,
        old: Table,
        (r0, r1): (usize, usize),
        (g0, g1): (usize, usize),
    ) -> Result<Selection> {
        let container = at.container.clone();
        let index = at.index;
        let author = self.author.clone();
        self.edit(move |tx, sel| {
            let mark = tx
                .track
                .as_mut()
                .map(Tracker::mark)
                .ok_or(EditError::BadPath)?;
            let mut t = old.clone();
            let spans = t.markup_spans();
            for r in (r0..=r1.min(t.rows.len().saturating_sub(1))).rev() {
                for (ci, &(s, k)) in spans[r].iter().enumerate().rev() {
                    let c = &mut t.rows[r].cells[ci];
                    if !(s >= g0 && s + k - 1 <= g1) || c.props.del.is_some() {
                        continue;
                    }
                    // A cell the author inserted is removed (cellIns and cellDel
                    // cannot both mark a cell, CT_TcPr).
                    if c.props
                        .ins
                        .as_ref()
                        .is_some_and(|m| m.info.author == author)
                    {
                        t.rows[r].cells.remove(ci);
                    } else {
                        c.props.del = Some(mark.clone());
                    }
                }
            }
            t.rows.retain(|r| !r.cells.is_empty());
            // Grid columns no row reaches any more go (rejecting restores them from
            // §17.13.5.33 tblGridChange).
            let reach = t
                .markup_spans()
                .iter()
                .map(|r| r.last().map_or(0, |(s, k)| s + k))
                .max()
                .unwrap_or(0)
                .max(1);
            if t.grid.len() > reach {
                if t.grid_change.is_none() {
                    t.grid_change = Some(Box::new(GridChange {
                        info: mark.info.clone(),
                        old: t.grid.clone(),
                    }));
                }
                t.grid.truncate(reach);
            }
            if t == old {
                return Ok(sel);
            }
            if t.rows.is_empty() {
                let blocks = blocks_at(tx.doc, &container).ok_or(EditError::BadPath)?;
                let insert = if index + 1 >= blocks.len() {
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
                return Ok(sel);
            }
            tx.push(Op::Blocks {
                path: container.clone(),
                index,
                remove: vec![Block::Table(old)],
                insert: vec![Block::Table(t)],
            })?;
            Ok(sel)
        })?;
        self.fix_selection();
        Ok(self.selection)
    }

    /// Tracked merge of grid rectangle rows `r0..=r1`, columns `g0..=g1`: the first
    /// cell of each row spans the columns (§17.13.5.36 tcPrChange), the cells merged
    /// into it are deleted cells (§17.13.5.3 cellDel), the rows below continue the
    /// vertical merge (§17.13.5.6 cellMerge) with their text marked deleted, and the
    /// merged text is inserted into the top-left cell (§17.13.5.18).
    fn track_merge(
        &mut self,
        at: &CellAt,
        old: Table,
        (r0, r1): (usize, usize),
        (g0, g1): (usize, usize),
    ) -> Result<Selection> {
        let container = at.container.clone();
        let index = at.index;
        let rect_err = || EditError::Unsupported("cells do not form a rectangle".into());
        self.edit(move |tx, sel| {
            let tr = tx.track.as_mut().ok_or(EditError::BadPath)?;
            let mark = tr.mark();
            let tr = tr.clone();
            let mut t = old.clone();
            let mut moved: Vec<Block> = Vec::new();
            for r in r0..=r1 {
                let row = &mut t.rows[r];
                let st = starts(row);
                let (Some(i), Some(j)) = (cell_covering(row, g0), cell_covering(row, g1)) else {
                    return Err(rect_err());
                };
                if st[i] != g0 || st[j] + span(&row.cells[j]) - 1 != g1 {
                    return Err(rect_err());
                }
                if r > r0 && !is_empty_cell(&row.cells[i]) {
                    moved.extend(row.cells[i].blocks.iter().cloned());
                    mark_blocks_deleted(&mut row.cells[i].blocks, &tr, &mark);
                }
                for c in &mut row.cells[i + 1..=j] {
                    if !is_empty_cell(c) {
                        moved.extend(c.blocks.iter().cloned());
                    }
                    if c.props.del.is_none() {
                        c.props.del = Some(mark.clone());
                    }
                }
                let first = &mut row.cells[i];
                first.props.grid_span = (g1 - g0 + 1) as u32;
                if let Some(TableWidth::Dxa(_)) = first.props.width {
                    first.props.width = Some(TableWidth::Dxa(Twips(
                        old.grid
                            .iter()
                            .skip(g0)
                            .take(g1 - g0 + 1)
                            .map(|w| w.0)
                            .sum(),
                    )));
                }
                if r1 > r0 {
                    first.props.v_merge = Some(if r == r0 {
                        VMerge::Restart
                    } else {
                        VMerge::Continue
                    });
                }
            }
            let top_cell = cell_covering(&t.rows[r0], g0).unwrap_or(0);
            if let Some(top) = t.rows[r0].cells.get_mut(top_cell) {
                top.blocks = append_inserted(&top.blocks, &moved, &mark);
            }
            record_tracked(&old, &mut t, &[], &mark);
            tx.push(Op::Blocks {
                path: container.clone(),
                index,
                remove: vec![Block::Table(old)],
                insert: vec![Block::Table(t)],
            })?;
            let mut path = container.clone();
            path.extend([index, r0, top_cell, 0]);
            Ok(id_of(tx.doc, &path).map_or(sel, |id| Selection::caret(Pos::new(id, 0))))
        })
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
fn insert_grid_column(t: &mut Table, g: usize, w: Twips, own: usize) -> Inserted {
    let g = g.min(t.grid.len());
    t.grid.insert(g, w);
    let mut inserted = Vec::new();
    for (ri, row) in t.rows.iter_mut().enumerate() {
        let st = starts(row);
        let total: usize = row.cells.iter().map(span).sum();
        if g >= total {
            let like = row.cells.last().cloned().unwrap_or(Cell {
                props: bo_model::CellProps::default(),
                blocks: Vec::new(),
            });
            row.cells.push(empty_cell(&like, 1, Some(w)));
            inserted.push((ri, row.cells.len() - 1));
            continue;
        }
        let Some(i) = st.iter().position(|&s| s == g) else {
            // Inside a spanning cell.
            if let Some(i) = cell_covering(row, g) {
                if ri == own {
                    let like = row.cells[i].clone();
                    row.cells.insert(i + 1, empty_cell(&like, 1, Some(w)));
                    inserted.push((ri, i + 1));
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
        inserted.push((ri, i));
    }
    inserted
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
fn unmerge(t: &mut Table, r: usize, c: usize) -> Inserted {
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
    let mut inserted = Vec::new();
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
                inserted.push((ri, i + k));
            }
        }
    }
    inserted
}
