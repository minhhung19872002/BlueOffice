//! BlueDoc structures shared next to the body text (prompt §5.2, §6 Phase 6):
//!
//! * **top-level tables, cell by cell** — the body line of a table is the
//!   embed `{"t": key}`; the structure lives in `meta`: `tb:<t>` table
//!   properties (§17.4.60 tblPr), `tr:<t>:<row>` = `{"o": order, "p": trPr}`,
//!   `tl:<t>:<col>` = `{"o": order, "w": gridCol width}` (§17.4.16), `tc:<t>:<row>:<col>`
//!   cell properties (§17.4.70 tcPr); every cell's content is the nested text
//!   `c:<t>:<row>:<col>` (lines like the body), so concurrent edits of
//!   different cells — and of the same cell — merge like body text. A row
//!   holds the cells whose keys exist, in column order (rows and columns are
//!   ordered by [`super::order`] keys, so concurrent inserts/deletes converge);
//! * **stories** (nested texts, same line format): footnotes `fn:<id>` and
//!   endnotes `en:<id>` (§17.11), comment content `cm:<id>` (§17.13.4),
//!   headers/footers `hf:<section>:<kind>` (§17.10);
//! * **definitions** (last writer wins per key): styles `st:<id>` (§17.7.4.17),
//!   numbering of the opened file `nm:<numId>` / `an:<abstractNumId>` (§17.9),
//!   comment metadata `cm:<id>` (author, date, thread, done; [MS-DOCX] §2.5.2),
//!   document settings `doc` (`evenAndOddHeaders`, §17.10.1).

use super::{
    apply_entries, diff_cells, line_of, order, BuildCtx, CollabChange, DeltaBuilder, Entry,
    LineCtx, Seg, TextDelta, TextOp,
};
use bo_model::{
    AbstractNum, Block, CellProps, Comment, Document, HeaderFooter, Num, Paragraph, Row, RowProps,
    Section, Style, Table, TableProps, Twips,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

// ---------------------------------------------------------------------------
// Stories
// ---------------------------------------------------------------------------

/// Shadow of one nested text (lines of blocks, tables as `{"b"}` embeds).
#[derive(Debug, Clone, Default)]
pub(crate) struct Story {
    pub entries: Vec<Entry>,
}

fn entry_of(b: &Block, ctx: &LineCtx<'_>) -> Entry {
    let line = line_of(b, None, None, ctx);
    Entry {
        block: b.clone(),
        sect: None,
        len: line.len16(),
        line,
    }
}

impl Story {
    pub fn from_blocks(blocks: &[Block], ctx: &LineCtx<'_>) -> Story {
        Story {
            entries: blocks.iter().map(|b| entry_of(b, ctx)).collect(),
        }
    }

    /// The whole story as one delta of inserts.
    pub fn full_delta(&self) -> Vec<TextOp> {
        let mut cells = Vec::new();
        for e in &self.entries {
            e.line.explode(&mut cells);
        }
        let mut b = DeltaBuilder::default();
        for (u, a) in &cells {
            b.insert(u, a);
        }
        b.finish()
    }

    pub fn reset_change(&self) -> TextDelta {
        let d = self.full_delta();
        TextDelta {
            reset: true,
            deltas: if d.is_empty() { Vec::new() } else { vec![d] },
        }
    }

    /// Delta turning the shadow into `blocks` (the shadow is updated).
    pub fn update(&mut self, blocks: &[Block], ctx: &LineCtx<'_>) -> Vec<TextOp> {
        let (no, nn) = (self.entries.len(), blocks.len());
        let n = no.min(nn);
        let mut p = 0;
        while p < n && self.entries[p].block == blocks[p] {
            p += 1;
        }
        let mut s = 0;
        while s < n - p && self.entries[no - 1 - s].block == blocks[nn - 1 - s] {
            s += 1;
        }
        if p == no && p == nn {
            return Vec::new();
        }
        let fresh: Vec<Entry> = blocks[p..nn - s].iter().map(|b| entry_of(b, ctx)).collect();
        let mut old = Vec::new();
        for e in &self.entries[p..no - s] {
            e.line.explode(&mut old);
        }
        let mut new = Vec::new();
        for e in &fresh {
            e.line.explode(&mut new);
        }
        let mut b = DeltaBuilder::default();
        b.retain(
            self.entries[..p].iter().map(|e| e.len).sum(),
            serde_json::Map::new(),
        );
        diff_cells(&mut b, &old, &new);
        self.entries.splice(p..no - s, fresh);
        b.finish()
    }

    pub fn blocks(&self) -> Vec<Block> {
        self.entries.iter().map(|e| e.block.clone()).collect()
    }

    /// Blocks of a container that needs at least one paragraph.
    pub fn content(&self) -> Vec<Block> {
        let b = self.blocks();
        if b.is_empty() {
            vec![Block::Paragraph(Paragraph::default())]
        } else {
            b
        }
    }
}

/// Applies a remote change of one nested text to `stories`.
pub(crate) fn apply_text(
    stories: &mut BTreeMap<String, Story>,
    key: &str,
    change: Option<&TextDelta>,
    ctx: &mut BuildCtx<'_>,
) -> Result<(), String> {
    let Some(tc) = change else {
        stories.remove(key);
        return Ok(());
    };
    let st = stories.entry(key.to_owned()).or_default();
    if tc.reset {
        st.entries.clear();
    }
    for d in &tc.deltas {
        apply_entries(&mut st.entries, d, ctx)?;
    }
    Ok(())
}

/// Shares `blocks` as the story `key` (created when missing).
fn share_story(
    stories: &mut BTreeMap<String, Story>,
    key: &str,
    blocks: &[Block],
    ctx: &LineCtx<'_>,
    out: &mut CollabChange,
) {
    if let Some(st) = stories.get_mut(key) {
        let d = st.update(blocks, ctx);
        if !d.is_empty() {
            out.texts.insert(
                key.to_owned(),
                Some(TextDelta {
                    reset: false,
                    deltas: vec![d],
                }),
            );
        }
    } else {
        let st = Story::from_blocks(blocks, ctx);
        out.texts.insert(key.to_owned(), Some(st.reset_change()));
        stories.insert(key.to_owned(), st);
    }
}

fn drop_story(stories: &mut BTreeMap<String, Story>, key: &str, out: &mut CollabChange) {
    if stories.remove(key).is_some() {
        out.texts.insert(key.to_owned(), None);
    }
}

// ---------------------------------------------------------------------------
// Tables
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct RowEntry {
    o: f64,
    p: RowProps,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct ColEntry {
    o: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    w: Option<Twips>,
}

/// Shared structure of one table (what `meta` holds for it).
#[derive(Debug, Clone, Default)]
pub(crate) struct TableShadow {
    pub props: Option<String>,
    /// Row id → (order, `RowEntry` JSON).
    pub rows: BTreeMap<String, (f64, String)>,
    pub cols: BTreeMap<String, (f64, String)>,
    /// (row id, col id) → `CellProps` JSON.
    pub cells: BTreeMap<(String, String), String>,
}

fn sorted(m: &BTreeMap<String, (f64, String)>) -> Vec<String> {
    let mut v: Vec<(&String, f64)> = m.iter().map(|(k, (o, _))| (k, *o)).collect();
    v.sort_by(|a, b| order::cmp((a.1, a.0), (b.1, b.0)));
    v.into_iter().map(|(k, _)| k.clone()).collect()
}

fn ord_of(json: &str) -> f64 {
    serde_json::from_str::<Value>(json)
        .ok()
        .and_then(|v| v.get("o").and_then(Value::as_f64))
        .unwrap_or(0.0)
}

pub(crate) fn cell_story(t: &str, r: &str, c: &str) -> String {
    format!("c:{t}:{r}:{c}")
}

impl TableShadow {
    /// Applies one `meta` entry of this table (`rest` = key after `<prefix>:<t>`).
    pub fn set(&mut self, kind: &str, rest: &str, v: Option<&String>) {
        match kind {
            "tb" => self.props = v.cloned(),
            "tr" | "tl" => {
                let m = if kind == "tr" {
                    &mut self.rows
                } else {
                    &mut self.cols
                };
                match v {
                    Some(j) => {
                        m.insert(rest.to_owned(), (ord_of(j), j.clone()));
                    }
                    None => {
                        m.remove(rest);
                    }
                }
            }
            "tc" => {
                let Some((r, c)) = rest.split_once(':') else {
                    return;
                };
                let k = (r.to_owned(), c.to_owned());
                match v {
                    Some(j) => {
                        self.cells.insert(k, j.clone());
                    }
                    None => {
                        self.cells.remove(&k);
                    }
                }
            }
            _ => {}
        }
    }

    pub fn is_empty(&self) -> bool {
        self.props.is_none() && self.rows.is_empty() && self.cols.is_empty()
    }

    /// The model table (cells in column order, contents from `stories`).
    pub fn build(&self, key: &str, stories: &BTreeMap<String, Story>) -> Block {
        let cols = sorted(&self.cols);
        let rows = sorted(&self.rows);
        if rows.is_empty() {
            return Block::Paragraph(Paragraph::default());
        }
        let props: TableProps = self
            .props
            .as_deref()
            .and_then(|j| serde_json::from_str(j).ok())
            .unwrap_or_default();
        let grid = cols
            .iter()
            .filter_map(|c| {
                self.cols
                    .get(c)
                    .and_then(|(_, j)| serde_json::from_str::<ColEntry>(j).ok())
                    .and_then(|e| e.w)
            })
            .collect();
        let rows = rows
            .iter()
            .map(|r| {
                let props = self
                    .rows
                    .get(r)
                    .and_then(|(_, j)| serde_json::from_str::<RowEntry>(j).ok())
                    .map(|e| e.p)
                    .unwrap_or_default();
                let cells = cols
                    .iter()
                    .filter_map(|c| {
                        let pj = self.cells.get(&(r.clone(), c.clone()))?;
                        let props: CellProps = serde_json::from_str(pj).unwrap_or_default();
                        let blocks = stories.get(&cell_story(key, r, c)).map_or_else(
                            || vec![Block::Paragraph(Paragraph::default())],
                            Story::content,
                        );
                        Some(bo_model::Cell { props, blocks })
                    })
                    .collect();
                Row { props, cells }
            })
            .collect();
        Block::Table(Table {
            props,
            grid,
            rows,
            src: None,
        })
    }
}

/// Grid column where each cell of `row` starts.
fn starts(row: &Row) -> Vec<usize> {
    let mut g = 0usize;
    row.cells
        .iter()
        .map(|c| {
            let at = g;
            g += c.props.grid_span.max(1) as usize;
            at
        })
        .collect()
}

fn n_cols(t: &Table) -> usize {
    t.rows
        .iter()
        .map(|r| {
            r.cells
                .iter()
                .map(|c| c.props.grid_span.max(1) as usize)
                .sum()
        })
        .chain([t.grid.len()])
        .max()
        .unwrap_or(0)
}

/// The cell of `row` starting at grid column `g`.
fn cell_at(row: &Row, st: &[usize], g: usize) -> Option<usize> {
    st.iter()
        .position(|&x| x == g)
        .filter(|&i| i < row.cells.len())
}

/// Pairs old and new items: equal prefix/suffix, then in order; `None` = new.
fn pair(no: usize, nn: usize, same: impl Fn(usize, usize) -> bool) -> Vec<Option<usize>> {
    let n = no.min(nn);
    let mut p = 0;
    while p < n && same(p, p) {
        p += 1;
    }
    let mut s = 0;
    while s < n - p && same(no - 1 - s, nn - 1 - s) {
        s += 1;
    }
    let mid_old = no - s - p;
    (0..nn)
        .map(|j| {
            if j < p {
                Some(j)
            } else if j >= nn - s {
                Some(no - (nn - j))
            } else if j - p < mid_old {
                Some(j)
            } else {
                None
            }
        })
        .collect()
}

/// Ids for the new order: paired items keep theirs, new ones get fresh ids.
fn ids_for(
    paired: &[Option<usize>],
    old_ids: &[String],
    fresh: &mut impl FnMut() -> String,
) -> Vec<String> {
    paired
        .iter()
        .map(|p| match p.and_then(|i| old_ids.get(i)) {
            Some(id) => id.clone(),
            None => fresh(),
        })
        .collect()
}

pub(crate) struct TableDiff<'a> {
    pub key: &'a str,
    pub ctx: &'a LineCtx<'a>,
}

impl TableDiff<'_> {
    /// Shares the changes `old` → `new` of table `key` (`old = None`: a new table).
    #[allow(clippy::too_many_lines)]
    pub fn run(
        &self,
        tables: &mut BTreeMap<String, TableShadow>,
        stories: &mut BTreeMap<String, Story>,
        old: Option<&Table>,
        new: &Table,
        fresh: &mut impl FnMut() -> String,
        out: &mut CollabChange,
    ) {
        let key = self.key;
        let sh = tables.entry(key.to_owned()).or_default();
        let mut old_rows = sorted(&sh.rows);
        let mut old_cols = sorted(&sh.cols);
        let mut old = old;
        if let Some(o) = old {
            if old_rows.len() != o.rows.len() || old_cols.len() < n_cols(o) {
                // The shadow lost track of this table: share it again.
                old = None;
            }
        }
        if old.is_none() {
            old_rows.clear();
            old_cols.clear();
        }
        let empty = Table::default();
        let o = old.unwrap_or(&empty);
        // Rows.
        let row_pair = pair(o.rows.len(), new.rows.len(), |a, b| {
            o.rows[a] == new.rows[b]
        });
        let row_ids = ids_for(&row_pair, &old_rows, fresh);
        // Columns: same width and same cells starting there in every row.
        let ost: Vec<Vec<usize>> = o.rows.iter().map(starts).collect();
        let nst: Vec<Vec<usize>> = new.rows.iter().map(starts).collect();
        let (oc, nc) = (n_cols(o), n_cols(new));
        let col_same = |a: usize, b: usize| {
            o.grid.get(a) == new.grid.get(b)
                && o.rows.len() == new.rows.len()
                && (0..o.rows.len()).all(|r| {
                    let x = cell_at(&o.rows[r], &ost[r], a).map(|i| &o.rows[r].cells[i]);
                    let y = cell_at(&new.rows[r], &nst[r], b).map(|i| &new.rows[r].cells[i]);
                    x == y
                })
        };
        let col_pair = pair(oc, nc, col_same);
        let col_ids = ids_for(&col_pair, &old_cols, fresh);
        // Table properties.
        let pj = serde_json::to_string(&new.props).unwrap_or_default();
        if sh.props.as_ref() != Some(&pj) {
            out.meta.insert(format!("tb:{key}"), Some(pj.clone()));
            sh.props = Some(pj);
        }
        // Row and column entries (order keys only change for moved/new items).
        let axis = |m: &mut BTreeMap<String, (f64, String)>,
                    ids: &[String],
                    json: &dyn Fn(usize, f64) -> String,
                    prefix: &str,
                    out: &mut CollabChange| {
            let known: Vec<Option<f64>> = ids.iter().map(|i| m.get(i).map(|e| e.0)).collect();
            let refs: Vec<&str> = ids.iter().map(String::as_str).collect();
            let keys = order::assign(&known, &refs);
            let keep: BTreeSet<&String> = ids.iter().collect();
            let gone: Vec<String> = m.keys().filter(|k| !keep.contains(k)).cloned().collect();
            for g in gone {
                m.remove(&g);
                out.meta.insert(format!("{prefix}:{key}:{g}"), None);
            }
            for (i, id) in ids.iter().enumerate() {
                let j = json(i, keys[i]);
                if m.get(id).map(|e| &e.1) != Some(&j) {
                    out.meta
                        .insert(format!("{prefix}:{key}:{id}"), Some(j.clone()));
                    m.insert(id.clone(), (keys[i], j));
                }
            }
        };
        let row_json = |i: usize, o: f64| {
            serde_json::to_string(&RowEntry {
                o,
                p: new.rows[i].props.clone(),
            })
            .unwrap_or_default()
        };
        axis(&mut sh.rows, &row_ids, &row_json, "tr", out);
        let col_json = |i: usize, o: f64| {
            serde_json::to_string(&ColEntry {
                o,
                w: new.grid.get(i).copied(),
            })
            .unwrap_or_default()
        };
        axis(&mut sh.cols, &col_ids, &col_json, "tl", out);
        // Cells.
        let mut present = BTreeSet::new();
        for (ri, row) in new.rows.iter().enumerate() {
            let rid = &row_ids[ri];
            let old_row = row_pair[ri].map(|i| (&o.rows[i], &ost[i]));
            for (ci, cell) in row.cells.iter().enumerate() {
                let g = nst[ri][ci];
                let Some(cid) = col_ids.get(g) else { continue };
                let k = (rid.clone(), cid.clone());
                present.insert(k.clone());
                let skey = cell_story(key, rid, cid);
                let old_cell = old_row.and_then(|(r, st)| {
                    let og = col_pair[g]?;
                    cell_at(r, st, og).map(|i| &r.cells[i])
                });
                if old_cell == Some(cell)
                    && sh.cells.contains_key(&k)
                    && stories.contains_key(&skey)
                {
                    continue;
                }
                let cj = serde_json::to_string(&cell.props).unwrap_or_default();
                if sh.cells.get(&k) != Some(&cj) {
                    out.meta
                        .insert(format!("tc:{key}:{rid}:{cid}"), Some(cj.clone()));
                    sh.cells.insert(k, cj);
                }
                share_story(stories, &skey, &cell.blocks, self.ctx, out);
            }
        }
        let gone: Vec<(String, String)> = sh
            .cells
            .keys()
            .filter(|k| !present.contains(*k))
            .cloned()
            .collect();
        for (r, c) in gone {
            sh.cells.remove(&(r.clone(), c.clone()));
            out.meta.insert(format!("tc:{key}:{r}:{c}"), None);
            drop_story(stories, &cell_story(key, &r, &c), out);
        }
    }
}

/// Removes every shared entry of table `key`.
pub(crate) fn drop_table(
    tables: &mut BTreeMap<String, TableShadow>,
    stories: &mut BTreeMap<String, Story>,
    key: &str,
    out: &mut CollabChange,
) {
    let Some(sh) = tables.remove(key) else { return };
    out.meta.insert(format!("tb:{key}"), None);
    for r in sh.rows.keys() {
        out.meta.insert(format!("tr:{key}:{r}"), None);
    }
    for c in sh.cols.keys() {
        out.meta.insert(format!("tl:{key}:{c}"), None);
    }
    for (r, c) in sh.cells.keys() {
        out.meta.insert(format!("tc:{key}:{r}:{c}"), None);
        drop_story(stories, &cell_story(key, r, c), out);
    }
}

/// Table key of a body line (`{"t": key}` embed).
pub(crate) fn table_key(line: &super::Line) -> Option<&str> {
    line.segs.iter().find_map(|s| match s {
        Seg::Embed(v, _) => v.get("t").and_then(Value::as_str),
        Seg::Text(..) => None,
    })
}

/// Splits a table `meta` key into (kind, table, rest).
pub(crate) fn split_table_key(k: &str) -> Option<(&str, &str, &str)> {
    let (kind, rest) = k.split_once(':')?;
    if !matches!(kind, "tb" | "tr" | "tl" | "tc") {
        return None;
    }
    match rest.split_once(':') {
        Some((t, r)) => Some((kind, t, r)),
        None => Some((kind, rest, "")),
    }
}

/// Table of a cell story key `c:<t>:<r>:<c>`.
pub(crate) fn story_table(k: &str) -> Option<&str> {
    k.strip_prefix("c:")?.split(':').next()
}

// ---------------------------------------------------------------------------
// Document parts
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CommentEntry {
    o: f64,
    c: Comment,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
struct DocSettings {
    #[serde(default)]
    eo: bool,
}

const HF_KINDS: [&str; 6] = ["hd", "hf", "he", "fd", "ff", "fe"];

fn hf_slot<'a>(s: &'a Section, kind: &str) -> Option<&'a Vec<Block>> {
    let hf = if kind.starts_with('h') {
        &s.headers
    } else {
        &s.footers
    };
    match &kind[1..] {
        "d" => hf.default.as_ref(),
        "f" => hf.first.as_ref(),
        _ => hf.even.as_ref(),
    }
}

fn hf_slot_mut<'a>(s: &'a mut Section, kind: &str) -> &'a mut Option<Vec<Block>> {
    let hf: &mut HeaderFooter = if kind.starts_with('h') {
        &mut s.headers
    } else {
        &mut s.footers
    };
    match &kind[1..] {
        "d" => &mut hf.default,
        "f" => &mut hf.first,
        _ => &mut hf.even,
    }
}

/// Header/footer contents replaced by empty placeholders (they travel as stories).
pub(crate) fn strip_headers(s: &mut Section) {
    for hf in [&mut s.headers, &mut s.footers] {
        for slot in [&mut hf.default, &mut hf.first, &mut hf.even] {
            if let Some(b) = slot.as_mut() {
                b.clear();
            }
        }
    }
}

/// What the parts looked like after the last comparison (cheap equality first).
#[derive(Debug, Default)]
pub(crate) struct Seen {
    styles: Option<bo_model::StyleSheet>,
    numbering: Option<bo_model::Numbering>,
    comments: Option<Vec<Comment>>,
    footnotes: Option<BTreeMap<i32, Vec<Block>>>,
    endnotes: Option<BTreeMap<i32, Vec<Block>>>,
    headers: Option<Vec<(HeaderFooter, HeaderFooter)>>,
    eo: Option<bool>,
}

impl Seen {
    /// The document as it is now counts as shared (after applying remote changes).
    pub fn update(&mut self, doc: &Document) {
        self.styles = Some(doc.styles.clone());
        self.numbering = Some(doc.numbering.clone());
        self.comments = Some(doc.comments.clone());
        self.footnotes = Some(doc.footnotes.clone());
        self.endnotes = Some(doc.endnotes.clone());
        self.headers = Some(headers_of(doc));
        self.eo = Some(doc.even_and_odd_headers);
    }
}

fn headers_of(doc: &Document) -> Vec<(HeaderFooter, HeaderFooter)> {
    doc.sections
        .iter()
        .map(|s| (s.headers.clone(), s.footers.clone()))
        .collect()
}

/// Shares the parts that changed since the last call.
pub(crate) fn share_parts(
    doc: &Document,
    base_nums: &BTreeSet<i32>,
    seen: &mut Seen,
    meta: &mut BTreeMap<String, String>,
    stories: &mut BTreeMap<String, Story>,
    ctx: &LineCtx<'_>,
    out: &mut CollabChange,
) {
    let set = |meta: &mut BTreeMap<String, String>,
               out: &mut CollabChange,
               k: String,
               v: Option<String>| {
        if meta.get(&k) != v.as_ref() {
            match &v {
                Some(j) => meta.insert(k.clone(), j.clone()),
                None => meta.remove(&k),
            };
            out.meta.insert(k, v);
        }
    };
    if seen.styles.as_ref() != Some(&doc.styles) {
        let mut now = BTreeSet::new();
        for s in &doc.styles.styles {
            let k = format!("st:{}", s.id);
            set(meta, out, k.clone(), serde_json::to_string(s).ok());
            now.insert(k);
        }
        let gone: Vec<String> = meta
            .keys()
            .filter(|k| k.starts_with("st:") && !now.contains(*k))
            .cloned()
            .collect();
        for k in gone {
            set(meta, out, k, None);
        }
        seen.styles = Some(doc.styles.clone());
    }
    if seen.numbering.as_ref() != Some(&doc.numbering) {
        // Only the lists of the opened file are shared by id (lists created
        // during the session travel by kind with the paragraphs).
        let abs: BTreeSet<i32> = doc
            .numbering
            .nums
            .iter()
            .filter(|n| base_nums.contains(&n.id))
            .map(|n| n.abstract_id)
            .collect();
        for n in doc
            .numbering
            .nums
            .iter()
            .filter(|n| base_nums.contains(&n.id))
        {
            set(
                meta,
                out,
                format!("nm:{}", n.id),
                serde_json::to_string(n).ok(),
            );
        }
        for a in doc
            .numbering
            .abstracts
            .iter()
            .filter(|a| abs.contains(&a.id))
        {
            set(
                meta,
                out,
                format!("an:{}", a.id),
                serde_json::to_string(a).ok(),
            );
        }
        seen.numbering = Some(doc.numbering.clone());
    }
    if seen.comments.as_ref() != Some(&doc.comments) {
        let known: Vec<Option<f64>> = doc
            .comments
            .iter()
            .map(|c| meta.get(&format!("cm:{}", c.id)).map(|j| ord_of(j)))
            .collect();
        let ids: Vec<String> = doc.comments.iter().map(|c| c.id.to_string()).collect();
        let refs: Vec<&str> = ids.iter().map(String::as_str).collect();
        let keys = order::assign(&known, &refs);
        let mut now = BTreeSet::new();
        for (i, c) in doc.comments.iter().enumerate() {
            let k = format!("cm:{}", c.id);
            let mut bare = c.clone();
            bare.blocks.clear();
            let j = serde_json::to_string(&CommentEntry {
                o: keys[i],
                c: bare,
            })
            .ok();
            set(meta, out, k.clone(), j);
            share_story(stories, &k, &c.blocks, ctx, out);
            now.insert(k);
        }
        let gone: Vec<String> = meta
            .keys()
            .filter(|k| k.starts_with("cm:") && !now.contains(*k))
            .cloned()
            .collect();
        for k in gone {
            set(meta, out, k.clone(), None);
            drop_story(stories, &k, out);
        }
        seen.comments = Some(doc.comments.clone());
    }
    for (endnote, notes) in [(false, &doc.footnotes), (true, &doc.endnotes)] {
        let slot = if endnote {
            &mut seen.endnotes
        } else {
            &mut seen.footnotes
        };
        if slot.as_ref() == Some(notes) {
            continue;
        }
        let prefix = if endnote { "en:" } else { "fn:" };
        for (id, blocks) in notes {
            share_story(stories, &format!("{prefix}{id}"), blocks, ctx, out);
        }
        let gone: Vec<String> = stories
            .keys()
            .filter(|k| {
                k.strip_prefix(prefix)
                    .and_then(|i| i.parse::<i32>().ok())
                    .is_some_and(|i| !notes.contains_key(&i))
            })
            .cloned()
            .collect();
        for k in gone {
            drop_story(stories, &k, out);
        }
        *slot = Some(notes.clone());
    }
    let hfs = headers_of(doc);
    if seen.headers.as_ref() != Some(&hfs) {
        let mut now = BTreeSet::new();
        for (si, s) in doc.sections.iter().enumerate() {
            for kind in HF_KINDS {
                if let Some(blocks) = hf_slot(s, kind) {
                    let k = format!("hf:{si}:{kind}");
                    share_story(stories, &k, blocks, ctx, out);
                    now.insert(k);
                }
            }
        }
        let gone: Vec<String> = stories
            .keys()
            .filter(|k| k.starts_with("hf:") && !now.contains(*k))
            .cloned()
            .collect();
        for k in gone {
            drop_story(stories, &k, out);
        }
        seen.headers = Some(hfs);
    }
    if seen.eo != Some(doc.even_and_odd_headers) {
        let j = serde_json::to_string(&DocSettings {
            eo: doc.even_and_odd_headers,
        })
        .ok();
        set(meta, out, "doc".into(), j);
        seen.eo = Some(doc.even_and_odd_headers);
    }
}

/// The parts as `meta` and the stories now say (`all` = rebuild everything,
/// e.g. for a late joiner; otherwise only kinds whose keys are in `touched`).
pub(crate) struct Rebuilt {
    pub styles: Option<bo_model::StyleSheet>,
    pub numbering: Option<bo_model::Numbering>,
    pub comments: Option<Vec<Comment>>,
    pub footnotes: Option<BTreeMap<i32, Vec<Block>>>,
    pub endnotes: Option<BTreeMap<i32, Vec<Block>>>,
    pub eo: Option<bool>,
}

fn by_prefix<'a>(
    meta: &'a BTreeMap<String, String>,
    p: &'a str,
) -> impl Iterator<Item = (&'a str, &'a String)> {
    meta.range(p.to_owned()..)
        .take_while(move |(k, _)| k.starts_with(p))
        .map(move |(k, v)| (&k[p.len()..], v))
}

pub(crate) fn rebuild_parts(
    doc: &Document,
    meta: &BTreeMap<String, String>,
    stories: &BTreeMap<String, Story>,
    touched: &BTreeSet<String>,
    all: bool,
) -> Rebuilt {
    let hit = |p: &str| all || touched.iter().any(|k| k.starts_with(p));
    let styles = hit("st:").then(|| {
        let mut out = doc.styles.clone();
        let mut list: Vec<Style> = Vec::new();
        let mut seen = BTreeSet::new();
        for s in &doc.styles.styles {
            let Some(j) = meta.get(&format!("st:{}", s.id)) else {
                if !all {
                    // Not shared (yet): keep.
                    list.push(s.clone());
                }
                continue;
            };
            seen.insert(s.id.clone());
            if serde_json::to_string(s).ok().as_ref() == Some(j) {
                list.push(s.clone());
            } else if let Ok(n) = serde_json::from_str::<Style>(j) {
                list.push(n);
            }
        }
        for (id, j) in by_prefix(meta, "st:") {
            if !seen.contains(id) {
                if let Ok(n) = serde_json::from_str::<Style>(j) {
                    list.push(n);
                }
            }
        }
        // Styles removed by others.
        if !all {
            list.retain(|s| {
                meta.contains_key(&format!("st:{}", s.id))
                    || !touched.contains(&format!("st:{}", s.id))
            });
        }
        out.styles = list;
        out
    });
    let numbering = (hit("nm:") || hit("an:")).then(|| {
        let mut out = doc.numbering.clone();
        for (id, j) in by_prefix(meta, "nm:") {
            let Ok(n) = serde_json::from_str::<Num>(j) else {
                continue;
            };
            match out.nums.iter_mut().find(|x| x.id.to_string() == id) {
                Some(x) => *x = n,
                None => out.nums.push(n),
            }
        }
        for (id, j) in by_prefix(meta, "an:") {
            let Ok(a) = serde_json::from_str::<AbstractNum>(j) else {
                continue;
            };
            match out.abstracts.iter_mut().find(|x| x.id.to_string() == id) {
                Some(x) => *x = a,
                None => out.abstracts.push(a),
            }
        }
        out
    });
    let comments = hit("cm:").then(|| {
        let mut list: Vec<(f64, String, Comment)> = by_prefix(meta, "cm:")
            .filter_map(|(id, j)| {
                let e: CommentEntry = serde_json::from_str(j).ok()?;
                let mut c = e.c;
                c.blocks = stories
                    .get(&format!("cm:{id}"))
                    .map(Story::blocks)
                    .unwrap_or_default();
                Some((e.o, id.to_owned(), c))
            })
            .collect();
        list.sort_by(|a, b| order::cmp((a.0, &a.1), (b.0, &b.1)));
        list.into_iter().map(|x| x.2).collect()
    });
    let notes = |prefix: &str| {
        stories
            .iter()
            .filter_map(|(k, st)| {
                let id: i32 = k.strip_prefix(prefix)?.parse().ok()?;
                Some((id, st.blocks()))
            })
            .collect::<BTreeMap<i32, Vec<Block>>>()
    };
    let footnotes = (all || touched.iter().any(|k| k.starts_with("fn:"))).then(|| notes("fn:"));
    let endnotes = (all || touched.iter().any(|k| k.starts_with("en:"))).then(|| notes("en:"));
    let eo = (all || touched.contains("doc")).then(|| {
        meta.get("doc")
            .and_then(|j| serde_json::from_str::<DocSettings>(j).ok())
            .map_or(doc.even_and_odd_headers, |d| d.eo)
    });
    Rebuilt {
        styles,
        numbering,
        comments,
        footnotes,
        endnotes,
        eo,
    }
}

/// Sections with header/footer contents taken from the stories; `None`
/// when nothing differs.
pub(crate) fn rebuilt_headers(
    doc: &Document,
    stories: &BTreeMap<String, Story>,
    touched: &BTreeSet<String>,
    all: bool,
) -> Option<Vec<Section>> {
    let mut changed = false;
    let mut out = doc.sections.clone();
    for (si, s) in out.iter_mut().enumerate() {
        for kind in HF_KINDS {
            let slot = hf_slot_mut(s, kind);
            let Some(cur) = slot.as_mut() else { continue };
            let k = format!("hf:{si}:{kind}");
            if !all && !touched.contains(&k) {
                continue;
            }
            if let Some(st) = stories.get(&k) {
                let b = st.blocks();
                if *cur != b {
                    *cur = b;
                    changed = true;
                }
            }
        }
    }
    changed.then_some(out)
}
