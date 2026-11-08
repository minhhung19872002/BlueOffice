//! Live collaboration for BlueSheet (prompt §6 Phase 6): the workbook ↔
//! `bo-collab` (`cells` and `meta` maps), designed so that concurrent
//! structural edits and cell edits converge on every replica.
//!
//! * **Stable row/column ids.** Row `i` of the file the room was seeded with
//!   has id `"i"` and order key `i`; rows inserted during the session get an
//!   id of their replica (`n<client>.<counter>`) and an order key between
//!   their neighbours (`ra:<sheet>:<id>` = key); deleted rows are marked
//!   (`rd:<sheet>:<id>`). Columns: `ca:` / `cd:`. A row's position is its
//!   rank among the live rows ([`Axis`]), so an insert and a concurrent edit
//!   below it both land where intended, whatever order replicas see them.
//! * **Cells** live in the `cells` map under `"<sheet>!<row id>,<col id>"`
//!   (value, or formula), formats in `meta` `fm:<same key>` (the resolved
//!   format — font, fill, border, number format, alignment — not a style
//!   index, ECMA-376 Part 1 §18.8.10), so a value and a format set
//!   concurrently both survive. **Formulas are stored with their references
//!   as row/column ids** (§18.17 formulas): every replica renders them back
//!   to A1 text for its current rows and columns, so a reference follows the
//!   rows inserted or deleted by anyone; a reference to a deleted row becomes
//!   `#REF!`.
//! * **Row/column properties** (§18.3.1.73 row, §18.3.1.13 col): `rp:<sheet>:<row id>`,
//!   `cp:<sheet>:<first col id>` (with the span); **merges** (§18.3.1.55):
//!   `mg:<sheet>!<top-left ids>` = bottom-right ids.
//! * **Sheets** (§18.2.19): `sh:<sheet>` = order key + the sheet without its
//!   cells (name, visibility, views, page setup …): add, rename, delete and
//!   reorder converge (order keys, see [`crate::collab::order`]).
//! * **Conditional formats / data validation** (§18.3.1.18, §18.3.1.33):
//!   `cf:<sheet>` (with the differential formats it uses) and `dv:<sheet>`,
//!   last writer wins per sheet, their ranges also by row/column ids in
//!   `cfa:<sheet>` / `dva:<sheet>` so they follow concurrent row/column
//!   inserts and deletes (hyperlinks, AutoFilter and tables of `sh:<sheet>`
//!   likewise in `sha:<sheet>`); **charts** `ch:<sheet>:<i>` and **pivot
//!   tables** `pv:<sheet>:<i>` (+ caches `pc:<i>`) last writer wins per
//!   object; defined names `wb`.

use super::{SheetEngine, MAX_COLS, MAX_ROWS};
use crate::collab::{order, CollabChange};
use crate::{EngineError, Result};
use bo_formula::{CellAddr, Expr, RefKind, Reference, SheetRef};
use bo_model::sheet::{
    Border, Cell, CellRange, CellRef, CellValue, CellXf, ColInfo, ConditionalFormat,
    DataValidation, DefinedName, Dxf, Fill, Font, Formula, FormulaKind, PivotCache, PivotTable,
    RawAttr, RowInfo, Stylesheet, Workbook, Worksheet,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap};

// ---------------------------------------------------------------------------
// Axis
// ---------------------------------------------------------------------------

/// Rows (or columns) of one sheet: the infinite base sequence `0, 1, 2 …`
/// of the seeded file, plus inserted ids with order keys, minus deleted ids.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Axis {
    added: BTreeMap<String, f64>,
    deleted: BTreeSet<String>,
    /// Live inserted ids sorted by (key, id).
    sorted: Vec<(f64, String)>,
    /// Deleted base indices.
    del_base: BTreeSet<u32>,
}

fn base_index(id: &str) -> Option<u32> {
    id.parse().ok()
}

impl Axis {
    fn rebuild(&mut self) {
        self.sorted = self
            .added
            .iter()
            .filter(|(id, _)| !self.deleted.contains(*id))
            .map(|(id, k)| (*k, id.clone()))
            .collect();
        self.sorted
            .sort_by(|a, b| order::cmp((a.0, &a.1), (b.0, &b.1)));
        self.del_base = self.deleted.iter().filter_map(|d| base_index(d)).collect();
    }

    fn set_added(&mut self, id: &str, key: Option<f64>) {
        match key {
            Some(k) => self.added.insert(id.to_owned(), k),
            None => self.added.remove(id),
        };
        self.rebuild();
    }

    fn set_deleted(&mut self, id: &str, on: bool) {
        if on {
            self.deleted.insert(id.to_owned());
        } else {
            self.deleted.remove(id);
        }
        self.rebuild();
    }

    /// Number of base indices `b` that sort before an inserted key `k`
    /// (a base index equal to `k` sorts first: base ids are digits).
    fn base_le(k: f64) -> u64 {
        if k < 0.0 {
            0
        } else {
            // Truncation is the floor for k ≥ 0.
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let f = k.floor().min(f64::from(u32::MAX)) as u64;
            f + 1
        }
    }

    fn live_base_in(&self, from: u64, to: u64) -> u64 {
        if to <= from {
            return 0;
        }
        let dead = self
            .del_base
            .iter()
            .filter(|&&d| u64::from(d) >= from && u64::from(d) < to)
            .count() as u64;
        (to - from) - dead
    }

    /// The `n`-th live base index at or after `from`.
    fn nth_base(&self, from: u64, n: u64) -> u64 {
        let mut b = from + n;
        for &d in self
            .del_base
            .range(u32::try_from(from).unwrap_or(u32::MAX)..)
        {
            if u64::from(d) <= b {
                b += 1;
            } else {
                break;
            }
        }
        b
    }

    /// Order key of an id.
    fn key(&self, id: &str) -> Option<f64> {
        self.added
            .get(id)
            .copied()
            .or_else(|| base_index(id).map(f64::from))
    }

    /// Id at a position.
    pub fn id_at(&self, pos: u32) -> String {
        let mut rem = u64::from(pos);
        let mut b = 0u64;
        for (k, id) in &self.sorted {
            let lim = Self::base_le(*k).max(b);
            let live = self.live_base_in(b, lim);
            if rem < live {
                return self.nth_base(b, rem).to_string();
            }
            rem -= live;
            b = lim;
            if rem == 0 {
                return id.clone();
            }
            rem -= 1;
        }
        self.nth_base(b, rem).to_string()
    }

    /// Position of a live id.
    pub fn pos_of(&self, id: &str) -> Option<u32> {
        if self.deleted.contains(id) {
            return None;
        }
        let p = if let Some(k) = self.added.get(id) {
            let i = self
                .sorted
                .iter()
                .position(|(_, x)| x == id)
                .unwrap_or(self.sorted.len()) as u64;
            self.live_base_in(0, Self::base_le(*k)) + i
        } else {
            let b = u64::from(base_index(id)?);
            let before = self
                .sorted
                .iter()
                .take_while(|(k, _)| *k < b as f64)
                .count() as u64;
            self.live_base_in(0, b) + before
        };
        u32::try_from(p).ok()
    }

    /// Ids touched by inserts or deletes (every other id is a live base row).
    fn special(&self) -> BTreeSet<String> {
        self.added.keys().chain(&self.deleted).cloned().collect()
    }
}

// ---------------------------------------------------------------------------
// Shared values
// ---------------------------------------------------------------------------

/// A reference of a shared formula with ids instead of positions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct IdRef {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    s: Option<String>,
    /// 0 cell, 1 range, 2 rows, 3 columns.
    k: u8,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    r: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    c: Vec<String>,
    a: Vec<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct SharedFormula {
    /// Formula text with references replaced by `\u{E000}<n>\u{E001}`.
    t: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    x: Vec<IdRef>,
    #[serde(default)]
    kind: FormulaKind,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    ca: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    attrs: Vec<RawAttr>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct SharedCell {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    v: Option<CellValue>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    f: Option<SharedFormula>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    cm: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    vm: Option<u32>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    ph: bool,
}

/// A cell format by value (§18.8.10 xf with its font, fill, border, numFmt).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct StyleVal {
    xf: CellXf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    font: Option<Font>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    fill: Option<Fill>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    border: Option<Border>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    nf: Option<String>,
}

fn style_json(st: &Stylesheet, idx: u32) -> Option<String> {
    if idx == 0 {
        return None;
    }
    let xf = st.cell_xfs.get(idx as usize)?;
    let nf = st
        .num_fmts
        .iter()
        .find(|f| f.id == xf.num_fmt_id)
        .map(|f| f.code.clone());
    let v = StyleVal {
        font: st.fonts.get(xf.font_id as usize).cloned(),
        fill: st.fills.get(xf.fill_id as usize).cloned(),
        border: st.borders.get(xf.border_id as usize).cloned(),
        nf,
        xf: CellXf {
            font_id: 0,
            fill_id: 0,
            border_id: 0,
            ..xf.clone()
        },
    };
    serde_json::to_string(&v).ok()
}

fn intern_style(st: &mut Stylesheet, json: &str) -> u32 {
    let Ok(v) = serde_json::from_str::<StyleVal>(json) else {
        return 0;
    };
    let mut xf = v.xf;
    if let Some(f) = v.font {
        xf.font_id = st.add_font(f);
    }
    if let Some(f) = v.fill {
        xf.fill_id = st.add_fill(f);
    }
    if let Some(b) = v.border {
        xf.border_id = st.add_border(b);
    }
    if let Some(code) = v.nf {
        xf.num_fmt_id = st.add_num_fmt(&code);
    }
    st.add_xf(xf)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SheetEntry {
    o: f64,
    w: Worksheet,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ColEntry {
    i: ColInfo,
    n: u32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct CfEntry {
    cf: Vec<ConditionalFormat>,
    #[serde(default)]
    dx: BTreeMap<u32, Dxf>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
struct WbEntry {
    names: Vec<DefinedName>,
}

/// The sheet without what is shared separately.
fn bare(ws: &Worksheet) -> Worksheet {
    Worksheet {
        cells: BTreeMap::new(),
        rows: BTreeMap::new(),
        cols: Vec::new(),
        merges: Vec::new(),
        data_validations: Vec::new(),
        conditional_formats: Vec::new(),
        charts: Vec::new(),
        pivots: Vec::new(),
        sheet_id: 0,
        ..ws.clone()
    }
}

/// `ws` with the shared bare fields of `b`.
fn merge_bare(ws: &Worksheet, b: Worksheet) -> Worksheet {
    Worksheet {
        cells: ws.cells.clone(),
        rows: ws.rows.clone(),
        cols: ws.cols.clone(),
        merges: ws.merges.clone(),
        data_validations: ws.data_validations.clone(),
        conditional_formats: ws.conditional_formats.clone(),
        charts: ws.charts.clone(),
        pivots: ws.pivots.clone(),
        sheet_id: ws.sheet_id,
        ..b
    }
}

/// Model cells compared cheaply: a formula's cached value does not count.
fn same(a: &Cell, b: &Cell) -> bool {
    a.formula == b.formula
        && (a.formula.is_some() || a.value == b.value)
        && a.style == b.style
        && a.cm == b.cm
        && a.vm == b.vm
        && a.show_phonetic == b.show_phonetic
}

fn ph(i: usize) -> String {
    format!("\u{E000}{i}\u{E001}")
}

/// Last model values per sheet (cheap equality before serializing).
#[derive(Debug, Clone, Default)]
struct Seen {
    bare: Option<Worksheet>,
    rows: Option<BTreeMap<u32, RowInfo>>,
    cols: Option<Vec<ColInfo>>,
    merges: Option<Vec<CellRange>>,
    cf: Option<Vec<ConditionalFormat>>,
    dv: Option<Vec<DataValidation>>,
    charts: Option<Vec<bo_model::sheet::Chart>>,
    pivots: Option<Vec<PivotTable>>,
}

/// A structural edit made locally (recorded by [`SheetEngine::structural`]).
#[derive(Debug, Clone, Copy)]
pub(crate) struct LocalStruct {
    pub sheet: usize,
    pub rows: bool,
    pub at: u32,
    pub count: u32,
    pub delete: bool,
}

/// Shadow of the shared workbook.
#[derive(Debug, Default)]
pub(crate) struct SheetCollab {
    joining: bool,
    client: u64,
    counter: u64,
    /// Local `sheet_id` → sheet key.
    by_id: BTreeMap<u32, String>,
    meta: BTreeMap<String, String>,
    cells: HashMap<String, String>,
    /// (sheet key, rows?) → axis.
    axes: BTreeMap<(String, bool), Axis>,
    /// Model cell at each shared key after the last comparison.
    snap: BTreeMap<String, Cell>,
    seen: BTreeMap<String, Seen>,
    seen_wb: Option<(Vec<DefinedName>, Vec<PivotCache>)>,
    pub pending: Vec<LocalStruct>,
}

fn cell_key(sk: &str, r: &str, c: &str) -> String {
    format!("{sk}!{r},{c}")
}

fn split_cell_key(k: &str) -> Option<(&str, &str, &str)> {
    let (sk, rest) = k.split_once('!')?;
    let (r, c) = rest.split_once(',')?;
    Some((sk, r, c))
}

/// Puts `v` under `k` when it differs from what `meta` holds.
fn put(meta: &mut BTreeMap<String, String>, out: &mut CollabChange, k: String, v: Option<String>) {
    if meta.get(&k) == v.as_ref() {
        return;
    }
    match &v {
        Some(j) => meta.insert(k.clone(), j.clone()),
        None => meta.remove(&k),
    };
    out.meta.insert(k, v);
}

/// Keys of `meta` starting with `p`.
fn keys_with(meta: &BTreeMap<String, String>, p: &str) -> Vec<String> {
    meta.range(p.to_owned()..)
        .take_while(|(k, _)| k.starts_with(p))
        .map(|(k, _)| k.clone())
        .collect()
}

impl SheetCollab {
    fn new(seed: bool, client: u64) -> SheetCollab {
        SheetCollab {
            joining: !seed,
            client,
            ..SheetCollab::default()
        }
    }

    fn fresh(&mut self, prefix: char) -> String {
        self.counter += 1;
        format!("{prefix}{:x}.{:08}", self.client, self.counter)
    }

    fn axis(&self, sk: &str, rows: bool) -> Axis {
        self.axes
            .get(&(sk.to_owned(), rows))
            .cloned()
            .unwrap_or_default()
    }

    /// Sheet keys of the model sheets (new sheets get a key).
    fn sheet_keys(&mut self, wb: &Workbook, seed_ids: bool) -> Vec<String> {
        let mut out = Vec::with_capacity(wb.sheets.len());
        for ws in &wb.sheets {
            let k = if let Some(k) = self.by_id.get(&ws.sheet_id) {
                k.clone()
            } else {
                let k = if seed_ids {
                    format!("s{}", ws.sheet_id)
                } else {
                    self.fresh('x')
                };
                self.by_id.insert(ws.sheet_id, k.clone());
                k
            };
            out.push(k);
        }
        out
    }

    /// Sheet key of a sheet name (formula references).
    fn key_of_name(wb: &Workbook, keys: &[String], name: &str) -> Option<String> {
        wb.sheets
            .iter()
            .position(|s| s.name.eq_ignore_ascii_case(name))
            .and_then(|i| keys.get(i).cloned())
    }

    // ------------------------------------------------------------ formulas

    fn formula_shared(
        &self,
        wb: &Workbook,
        keys: &[String],
        host: &str,
        f: &Formula,
    ) -> SharedFormula {
        let mut x = Vec::new();
        let t = match bo_formula::parse(&f.text) {
            Ok(expr) => {
                let mapped = expr.map(&mut |e| match e {
                    Expr::Ref(r) => {
                        let target = match &r.sheet {
                            None => Some(host.to_owned()),
                            Some(SheetRef { first, last: None }) => {
                                Self::key_of_name(wb, keys, first)
                            }
                            Some(_) => None,
                        };
                        let Some(sk) = target else {
                            return Expr::Ref(r);
                        };
                        let (ra, ca) = (self.axis(&sk, true), self.axis(&sk, false));
                        let s = r.sheet.as_ref().map(|s| s.first.clone());
                        let id = match r.kind {
                            RefKind::Cell(a) => IdRef {
                                s,
                                k: 0,
                                r: vec![ra.id_at(a.row)],
                                c: vec![ca.id_at(a.col)],
                                a: vec![a.row_abs, a.col_abs],
                            },
                            RefKind::Range(a, b) => IdRef {
                                s,
                                k: 1,
                                r: vec![ra.id_at(a.row), ra.id_at(b.row)],
                                c: vec![ca.id_at(a.col), ca.id_at(b.col)],
                                a: vec![a.row_abs, a.col_abs, b.row_abs, b.col_abs],
                            },
                            RefKind::Rows {
                                first,
                                last,
                                first_abs,
                                last_abs,
                            } => IdRef {
                                s,
                                k: 2,
                                r: vec![ra.id_at(first), ra.id_at(last)],
                                c: Vec::new(),
                                a: vec![first_abs, last_abs],
                            },
                            RefKind::Cols {
                                first,
                                last,
                                first_abs,
                                last_abs,
                            } => IdRef {
                                s,
                                k: 3,
                                r: Vec::new(),
                                c: vec![ca.id_at(first), ca.id_at(last)],
                                a: vec![first_abs, last_abs],
                            },
                        };
                        x.push(id);
                        Expr::Name {
                            sheet: None,
                            name: ph(x.len() - 1),
                        }
                    }
                    other => other,
                });
                bo_formula::format(&mapped)
            }
            Err(_) => f.text.clone(),
        };
        SharedFormula {
            t,
            x,
            kind: f.kind.clone(),
            ca: f.calc_always,
            attrs: f.attrs.clone(),
        }
    }

    /// A1 text of a shared formula for the current rows and columns.
    fn formula_text(
        &self,
        wb: &Workbook,
        keys: &[String],
        host: &str,
        f: &SharedFormula,
    ) -> String {
        let mut out = String::with_capacity(f.t.len());
        let mut rest = f.t.as_str();
        while let Some(i) = rest.find('\u{E000}') {
            out.push_str(&rest[..i]);
            let after = &rest[i + '\u{E000}'.len_utf8()..];
            let Some(j) = after.find('\u{E001}') else {
                rest = after;
                break;
            };
            let idx: usize = after[..j].parse().unwrap_or(usize::MAX);
            out.push_str(&self.ref_text(wb, keys, host, f.x.get(idx)));
            rest = &after[j + '\u{E001}'.len_utf8()..];
        }
        out.push_str(rest);
        out
    }

    fn ref_text(&self, wb: &Workbook, keys: &[String], host: &str, r: Option<&IdRef>) -> String {
        const REF: &str = "#REF!";
        let Some(r) = r else { return REF.into() };
        let sk = match &r.s {
            None => host.to_owned(),
            Some(n) => match Self::key_of_name(wb, keys, n) {
                Some(k) => k,
                None => return REF.into(),
            },
        };
        let (ra, ca) = (self.axis(&sk, true), self.axis(&sk, false));
        let rows: Option<Vec<u32>> = r.r.iter().map(|id| ra.pos_of(id)).collect();
        let cols: Option<Vec<u32>> = r.c.iter().map(|id| ca.pos_of(id)).collect();
        let (Some(rows), Some(cols)) = (rows, cols) else {
            return REF.into();
        };
        let flag = |i: usize| r.a.get(i).copied().unwrap_or(false);
        let addr = |row: u32, col: u32, ra: bool, ca: bool| CellAddr {
            row,
            col,
            row_abs: ra,
            col_abs: ca,
        };
        let kind = match (r.k, rows.as_slice(), cols.as_slice()) {
            (0, [row], [col]) => RefKind::Cell(addr(*row, *col, flag(0), flag(1))),
            (1, [r1, r2], [c1, c2]) => RefKind::Range(
                addr(*r1, *c1, flag(0), flag(1)),
                addr(*r2, *c2, flag(2), flag(3)),
            ),
            (2, [r1, r2], []) => RefKind::Rows {
                first: *r1,
                last: *r2,
                first_abs: flag(0),
                last_abs: flag(1),
            },
            (3, [], [c1, c2]) => RefKind::Cols {
                first: *c1,
                last: *c2,
                first_abs: flag(0),
                last_abs: flag(1),
            },
            _ => return REF.into(),
        };
        if rows.iter().any(|&x| x >= MAX_ROWS) || cols.iter().any(|&x| x >= MAX_COLS) {
            return REF.into();
        }
        bo_formula::format_reference(
            &Reference {
                sheet: r.s.clone().map(SheetRef::single),
                kind,
            },
            &bo_formula::ParseOptions::default(),
        )
    }

    fn cell_json(&self, wb: &Workbook, keys: &[String], host: &str, c: &Cell) -> Option<String> {
        let f = c
            .formula
            .as_ref()
            .map(|f| self.formula_shared(wb, keys, host, f));
        let v = (f.is_none() && c.value != CellValue::Empty).then(|| c.value.clone());
        if f.is_none() && v.is_none() && c.cm.is_none() && c.vm.is_none() {
            return None;
        }
        serde_json::to_string(&SharedCell {
            v,
            f,
            cm: c.cm,
            vm: c.vm,
            ph: c.show_phonetic,
        })
        .ok()
    }

    // --------------------------------------------------------- local diff

    /// Applies a local structural edit to the axis; shares it.
    fn record(&mut self, sk: &str, op: LocalStruct, out: &mut CollabChange) {
        let mut ax = self.axis(sk, op.rows);
        let (pa, pd) = if op.rows { ("ra", "rd") } else { ("ca", "cd") };
        if op.delete {
            let ids: Vec<String> = (op.at..op.at.saturating_add(op.count))
                .map(|p| ax.id_at(p))
                .collect();
            for id in ids {
                ax.set_deleted(&id, true);
                put(
                    &mut self.meta,
                    out,
                    format!("{pd}:{sk}:{id}"),
                    Some("1".into()),
                );
            }
        } else {
            let lo = op.at.checked_sub(1).and_then(|p| ax.key(&ax.id_at(p)));
            let hi = ax.key(&ax.id_at(op.at));
            let n = f64::from(op.count.max(1));
            for j in 0..op.count {
                let j = f64::from(j);
                let k = match (lo, hi) {
                    (Some(a), Some(b)) => a + (b - a) * (j + 1.0) / (n + 1.0),
                    (None, Some(b)) => b - n + j,
                    (Some(a), None) => a + 1.0 + j,
                    (None, None) => j,
                };
                let id = self.fresh('n');
                ax.set_added(&id, Some(k));
                put(
                    &mut self.meta,
                    out,
                    format!("{pa}:{sk}:{id}"),
                    Some(k.to_string()),
                );
            }
        }
        self.axes.insert((sk.to_owned(), op.rows), ax);
    }

    #[allow(clippy::too_many_lines)]
    fn local_change(&mut self, wb: &Workbook, seed: bool) -> CollabChange {
        let mut out = CollabChange::default();
        if self.joining {
            self.pending.clear();
            return out;
        }
        let keys = self.sheet_keys(wb, seed);
        for op in std::mem::take(&mut self.pending) {
            if let Some(sk) = keys.get(op.sheet).cloned() {
                self.record(&sk, op, &mut out);
            }
        }
        // Sheets and their order.
        let known: Vec<Option<f64>> = keys
            .iter()
            .map(|k| {
                self.meta
                    .get(&format!("sh:{k}"))
                    .and_then(|j| serde_json::from_str::<serde_json::Value>(j).ok())
                    .and_then(|v| v.get("o").and_then(serde_json::Value::as_f64))
            })
            .collect();
        let refs: Vec<&str> = keys.iter().map(String::as_str).collect();
        let ords = order::assign(&known, &refs);
        let live: BTreeSet<&String> = keys.iter().collect();
        for k in keys_with(&self.meta, "sh:") {
            if !live.contains(&k[3..].to_owned()) {
                put(&mut self.meta, &mut out, k, None);
            }
        }
        let mut style_memo: HashMap<u32, Option<String>> = HashMap::new();
        for (si, ws) in wb.sheets.iter().enumerate() {
            let sk = keys[si].clone();
            let mut seen = self.seen.remove(&sk).unwrap_or_default();
            let b = bare(ws);
            if seen.bare.as_ref() != Some(&b) || known[si] != Some(ords[si]) {
                let j = serde_json::to_string(&SheetEntry {
                    o: ords[si],
                    w: b.clone(),
                })
                .ok();
                put(&mut self.meta, &mut out, format!("sh:{sk}"), j);
                let (ra, ca) = (self.axis(&sk, true), self.axis(&sk, false));
                let at = BareAt::of(ws, &ra, &ca);
                let j = (!at.is_empty())
                    .then(|| serde_json::to_string(&at).ok())
                    .flatten();
                put(&mut self.meta, &mut out, format!("sha:{sk}"), j);
                seen.bare = Some(b);
            }
            let (ra, ca) = (self.axis(&sk, true), self.axis(&sk, false));
            let mut rmemo: HashMap<u32, String> = HashMap::new();
            let mut cmemo: HashMap<u32, String> = HashMap::new();
            let mut rid = |r: u32| rmemo.entry(r).or_insert_with(|| ra.id_at(r)).clone();
            let mut cid = |c: u32| cmemo.entry(c).or_insert_with(|| ca.id_at(c)).clone();
            // Cells.
            let mut visited = BTreeSet::new();
            for (&(r, c), cell) in &ws.cells {
                let key = cell_key(&sk, &rid(r), &cid(c));
                visited.insert(key.clone());
                if self.snap.get(&key).is_some_and(|s| same(s, cell)) {
                    continue;
                }
                let v = self.cell_json(wb, &keys, &sk, cell);
                if self.cells.get(&key) != v.as_ref() {
                    match &v {
                        Some(j) => self.cells.insert(key.clone(), j.clone()),
                        None => self.cells.remove(&key),
                    };
                    out.cells.insert(key.clone(), v);
                }
                let st = style_memo
                    .entry(cell.style)
                    .or_insert_with(|| style_json(&wb.styles, cell.style))
                    .clone();
                put(&mut self.meta, &mut out, format!("fm:{key}"), st);
                self.snap.insert(key, cell.clone());
            }
            let prefix = format!("{sk}!");
            let gone: Vec<String> = self
                .snap
                .range(prefix.clone()..)
                .take_while(|(k, _)| k.starts_with(&prefix))
                .map(|(k, _)| k.clone())
                .filter(|k| !visited.contains(k))
                .collect();
            for key in gone {
                let Some((_, r, c)) = split_cell_key(&key) else {
                    continue;
                };
                // Cells of deleted rows/columns stay (an undo brings them back).
                if ra.pos_of(r).is_none() || ca.pos_of(c).is_none() {
                    continue;
                }
                self.snap.remove(&key);
                if self.cells.remove(&key).is_some() {
                    out.cells.insert(key.clone(), None);
                }
                put(&mut self.meta, &mut out, format!("fm:{key}"), None);
            }
            // Row properties.
            if seen.rows.as_ref() != Some(&ws.rows) {
                let mut now = BTreeSet::new();
                for (&r, info) in &ws.rows {
                    let k = format!("rp:{sk}:{}", rid(r));
                    put(
                        &mut self.meta,
                        &mut out,
                        k.clone(),
                        serde_json::to_string(info).ok(),
                    );
                    now.insert(k);
                }
                for k in keys_with(&self.meta, &format!("rp:{sk}:")) {
                    let id = &k[4 + sk.len()..];
                    if !now.contains(&k) && ra.pos_of(id).is_some() {
                        put(&mut self.meta, &mut out, k, None);
                    }
                }
                seen.rows = Some(ws.rows.clone());
            }
            // Column properties by first column.
            if seen.cols.as_ref() != Some(&ws.cols) {
                let mut now = BTreeSet::new();
                for ci in &ws.cols {
                    let k = format!("cp:{sk}:{}", cid(ci.min));
                    let e = ColEntry {
                        i: ColInfo {
                            min: 0,
                            max: 0,
                            ..ci.clone()
                        },
                        n: ci.max.saturating_sub(ci.min) + 1,
                    };
                    put(
                        &mut self.meta,
                        &mut out,
                        k.clone(),
                        serde_json::to_string(&e).ok(),
                    );
                    now.insert(k);
                }
                for k in keys_with(&self.meta, &format!("cp:{sk}:")) {
                    let id = &k[4 + sk.len()..];
                    if !now.contains(&k) && ca.pos_of(id).is_some() {
                        put(&mut self.meta, &mut out, k, None);
                    }
                }
                seen.cols = Some(ws.cols.clone());
            }
            // Merges by top-left cell.
            if seen.merges.as_ref() != Some(&ws.merges) {
                let mut now = BTreeSet::new();
                for m in &ws.merges {
                    let k = format!("mg:{}", cell_key(&sk, &rid(m.first.row), &cid(m.first.col)));
                    let v = format!("{},{}", rid(m.last.row), cid(m.last.col));
                    put(&mut self.meta, &mut out, k.clone(), Some(v));
                    now.insert(k);
                }
                for k in keys_with(&self.meta, &format!("mg:{sk}!")) {
                    if !now.contains(&k) {
                        put(&mut self.meta, &mut out, k, None);
                    }
                }
                seen.merges = Some(ws.merges.clone());
            }
            if seen.cf.as_ref() != Some(&ws.conditional_formats) {
                let mut e = CfEntry {
                    cf: ws.conditional_formats.clone(),
                    dx: BTreeMap::new(),
                };
                for rule in e.cf.iter().flat_map(|c| &c.rules) {
                    if let Some(d) = rule.dxf_id.and_then(|i| wb.styles.dxfs.get(i as usize)) {
                        e.dx.insert(rule.dxf_id.unwrap_or(0), d.clone());
                    }
                }
                let j = (!e.cf.is_empty())
                    .then(|| serde_json::to_string(&e).ok())
                    .flatten();
                put(&mut self.meta, &mut out, format!("cf:{sk}"), j);
                let at = ranges_by_id(
                    ws.conditional_formats.iter().map(|c| &c.sqref),
                    &mut rid,
                    &mut cid,
                );
                put(&mut self.meta, &mut out, format!("cfa:{sk}"), at);
                seen.cf = Some(ws.conditional_formats.clone());
            }
            if seen.dv.as_ref() != Some(&ws.data_validations) {
                let j = (!ws.data_validations.is_empty())
                    .then(|| serde_json::to_string(&ws.data_validations).ok())
                    .flatten();
                put(&mut self.meta, &mut out, format!("dv:{sk}"), j);
                let at = ranges_by_id(
                    ws.data_validations.iter().map(|d| &d.sqref),
                    &mut rid,
                    &mut cid,
                );
                put(&mut self.meta, &mut out, format!("dva:{sk}"), at);
                seen.dv = Some(ws.data_validations.clone());
            }
            if seen.charts.as_ref() != Some(&ws.charts) {
                let ats: Vec<Option<String>> = ws
                    .charts
                    .iter()
                    .map(|ch| {
                        let at = ChartAt {
                            f: [rid(ch.from.row), cid(ch.from.col)],
                            t: ch.to.map(|t| [rid(t.row), cid(t.col)]),
                            s: ch
                                .plots
                                .iter()
                                .flat_map(|p| &p.series)
                                .map(|s| {
                                    [&s.name_ref, &s.cat_ref, &s.val_ref].map(|r| {
                                        r.as_ref().map(|text| {
                                            let f = Formula {
                                                text: text.clone(),
                                                ..Formula::default()
                                            };
                                            self.formula_shared(wb, &keys, &sk, &f)
                                        })
                                    })
                                })
                                .collect(),
                        };
                        serde_json::to_string(&at).ok()
                    })
                    .collect();
                for (i, (c, at)) in ws.charts.iter().zip(ats).enumerate() {
                    put(
                        &mut self.meta,
                        &mut out,
                        format!("ch:{sk}:{i}"),
                        serde_json::to_string(c).ok(),
                    );
                    put(&mut self.meta, &mut out, format!("cha:{sk}:{i}"), at);
                }
                for p in ["ch", "cha"] {
                    for k in keys_with(&self.meta, &format!("{p}:{sk}:")) {
                        let i: usize = k[p.len() + 2 + sk.len()..].parse().unwrap_or(usize::MAX);
                        if i >= ws.charts.len() {
                            put(&mut self.meta, &mut out, k, None);
                        }
                    }
                }
                seen.charts = Some(ws.charts.clone());
            }
            if seen.pivots.as_ref() != Some(&ws.pivots) {
                for (i, p) in ws.pivots.iter().enumerate() {
                    put(
                        &mut self.meta,
                        &mut out,
                        format!("pv:{sk}:{i}"),
                        serde_json::to_string(p).ok(),
                    );
                }
                for k in keys_with(&self.meta, &format!("pv:{sk}:")) {
                    let i: usize = k[4 + sk.len()..].parse().unwrap_or(usize::MAX);
                    if i >= ws.pivots.len() {
                        put(&mut self.meta, &mut out, k, None);
                    }
                }
                seen.pivots = Some(ws.pivots.clone());
            }
            self.seen.insert(sk, seen);
        }
        let wbs = (wb.defined_names.clone(), wb.pivot_caches.clone());
        if self.seen_wb.as_ref() != Some(&wbs) {
            let j = serde_json::to_string(&WbEntry {
                names: wb.defined_names.clone(),
            })
            .ok();
            put(&mut self.meta, &mut out, "wb".into(), j);
            for (i, c) in wb.pivot_caches.iter().enumerate() {
                put(
                    &mut self.meta,
                    &mut out,
                    format!("pc:{i}"),
                    serde_json::to_string(c).ok(),
                );
            }
            for k in keys_with(&self.meta, "pc:") {
                let i: usize = k[3..].parse().unwrap_or(usize::MAX);
                if i >= wb.pivot_caches.len() {
                    put(&mut self.meta, &mut out, k, None);
                }
            }
            self.seen_wb = Some(wbs);
        }
        out
    }
}

// ---------------------------------------------------------------------------
// Remote changes
// ---------------------------------------------------------------------------

/// Structural differences of an axis: ids to delete (old positions,
/// descending) and to insert (new positions, ascending).
/// A chart's anchor cells by row/column ids and its series references
/// (§21.2.2.215 `f` of `tx`/`cat`/`val`) as id formulas (`cha:<sheet>:<i>`,
/// next to the positional `ch:<sheet>:<i>`).
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ChartAt {
    f: [String; 2],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    t: Option<[String; 2]>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    s: Vec<[Option<SharedFormula>; 3]>,
}

/// Ranges of a sheet's bare properties by row/column ids (`sha:<sheet>`):
/// hyperlinks (§18.3.1.47), the AutoFilter (§18.3.1.2) and tables (§18.5.1.2),
/// in the order of the shared `sh:<sheet>`; placed on the receiver's axes
/// like [`place_by_id`].
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
struct BareAt {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    hl: Vec<[String; 4]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    af: Option<[String; 4]>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    tb: Vec<[String; 4]>,
}

impl BareAt {
    fn of(ws: &Worksheet, ra: &Axis, ca: &Axis) -> BareAt {
        let ids = |r: &CellRange| {
            [
                ra.id_at(r.first.row),
                ca.id_at(r.first.col),
                ra.id_at(r.last.row),
                ca.id_at(r.last.col),
            ]
        };
        BareAt {
            hl: ws.hyperlinks.iter().map(|h| ids(&h.range)).collect(),
            af: ws.auto_filter.as_ref().map(|a| ids(&a.range)),
            tb: ws.tables.iter().map(|t| ids(&t.range)).collect(),
        }
    }

    fn is_empty(&self) -> bool {
        self.hl.is_empty() && self.af.is_none() && self.tb.is_empty()
    }

    fn place(&self, ws: &mut Worksheet, ra: &Axis, ca: &Axis) {
        let at = |ids: &[String; 4]| -> Option<CellRange> {
            let [r1, c1, r2, c2] = ids;
            Some(CellRange::new(
                CellRef::new(ra.pos_of(r1)?, ca.pos_of(c1)?),
                CellRef::new(ra.pos_of(r2)?, ca.pos_of(c2)?),
            ))
        };
        if self.hl.len() == ws.hyperlinks.len() {
            for (h, ids) in ws.hyperlinks.iter_mut().zip(&self.hl) {
                if let Some(r) = at(ids) {
                    h.range = r;
                }
            }
        }
        if let (Some(a), Some(ids)) = (ws.auto_filter.as_mut(), &self.af) {
            if let Some(r) = at(ids) {
                a.range = r;
            }
        }
        if self.tb.len() == ws.tables.len() {
            for (t, ids) in ws.tables.iter_mut().zip(&self.tb) {
                if let Some(r) = at(ids) {
                    t.range = r;
                }
            }
        }
    }
}

/// The ranges of each rule (`sqref`, §18.3.1.18 / §18.3.1.32) as row/column ids
/// `[top, left, bottom, right]` (`cfa:` / `dva:` next to the positional
/// `cf:` / `dv:`), so a receiver whose rows or columns moved concurrently
/// places them where the sender meant; `None` when there are no rules.
fn ranges_by_id<'a>(
    sqrefs: impl Iterator<Item = &'a Vec<CellRange>>,
    rid: &mut impl FnMut(u32) -> String,
    cid: &mut impl FnMut(u32) -> String,
) -> Option<String> {
    let at: Vec<Vec<[String; 4]>> = sqrefs
        .map(|s| {
            s.iter()
                .map(|r| {
                    [
                        rid(r.first.row),
                        cid(r.first.col),
                        rid(r.last.row),
                        cid(r.last.col),
                    ]
                })
                .collect()
        })
        .collect();
    (!at.is_empty())
        .then(|| serde_json::to_string(&at).ok())
        .flatten()
}

/// Positions of the ranges of [`ranges_by_id`] on this replica's axes; a range
/// whose rows or columns were deleted (or a sender without ids) keeps its
/// positional value.
fn place_by_id<'a>(
    sqrefs: impl Iterator<Item = &'a mut Vec<CellRange>>,
    at: Option<&String>,
    ra: &Axis,
    ca: &Axis,
) {
    let Some(at) = at.and_then(|j| serde_json::from_str::<Vec<Vec<[String; 4]>>>(j).ok()) else {
        return;
    };
    for (sqref, ids) in sqrefs.zip(&at) {
        if sqref.len() != ids.len() {
            continue;
        }
        for (r, [r1, c1, r2, c2]) in sqref.iter_mut().zip(ids) {
            if let (Some(a), Some(b), Some(c), Some(d)) =
                (ra.pos_of(r1), ca.pos_of(c1), ra.pos_of(r2), ca.pos_of(c2))
            {
                *r = CellRange::new(CellRef::new(a, b), CellRef::new(c, d));
            }
        }
    }
}

fn axis_steps(old: &Axis, new: &Axis) -> (Vec<u32>, Vec<u32>) {
    let mut cand = old.special();
    cand.extend(new.special());
    let mut del = Vec::new();
    let mut ins = Vec::new();
    for id in cand {
        let (po, pn) = (old.pos_of(&id), new.pos_of(&id));
        let moved = old.added.get(&id) != new.added.get(&id);
        match (po, pn) {
            (Some(p), None) => del.push(p),
            (None, Some(p)) => ins.push(p),
            (Some(p), Some(q)) if moved => {
                del.push(p);
                ins.push(q);
            }
            _ => {}
        }
    }
    del.sort_unstable_by(|a, b| b.cmp(a));
    ins.sort_unstable();
    (del, ins)
}

/// The shared sheet keys in order.
fn shared_sheets(meta: &BTreeMap<String, String>) -> Vec<(String, SheetEntry)> {
    let mut v: Vec<(String, SheetEntry)> = keys_with(meta, "sh:")
        .into_iter()
        .filter_map(|k| {
            let e: SheetEntry = serde_json::from_str(meta.get(&k)?).ok()?;
            Some((k[3..].to_owned(), e))
        })
        .collect();
    v.sort_by(|a, b| order::cmp((a.1.o, &a.0), (b.1.o, &b.0)));
    v
}

impl SheetEngine {
    /// Binds the open workbook to a collaboration session; the seeder gets
    /// the whole workbook as the change to publish. `client` is the replica
    /// client id (ids of new rows, columns and sheets).
    pub fn collab_start(&mut self, seed: bool, client: u64) -> Result<CollabChange> {
        let wb = self.wb.as_ref().ok_or(EngineError::NoDocument)?;
        let mut c = SheetCollab::new(seed, client);
        let ch = if seed {
            c.local_change(wb, true)
        } else {
            CollabChange::default()
        };
        self.collab = Some(c);
        Ok(ch)
    }

    /// [`Self::collab_start`] with the change as JSON.
    pub fn collab_start_json(&mut self, seed: bool, client: u64) -> Result<String> {
        Ok(serde_json::to_string(&self.collab_start(seed, client)?).unwrap_or_default())
    }

    /// Leaves the collaboration session.
    pub fn collab_stop(&mut self) {
        self.collab = None;
    }

    /// What the local edits since the last call changed (call after every
    /// mutating command while collaborating).
    pub fn collab_local(&mut self) -> Result<CollabChange> {
        let wb = self.wb.as_ref().ok_or(EngineError::NoDocument)?;
        Ok(match self.collab.as_mut() {
            Some(c) => c.local_change(wb, false),
            None => CollabChange::default(),
        })
    }

    /// [`Self::collab_local`] as JSON.
    pub fn collab_local_json(&mut self) -> Result<String> {
        Ok(serde_json::to_string(&self.collab_local()?).unwrap_or_default())
    }

    /// [`Self::collab_apply`] with a JSON change; returns the workbook summary as JSON.
    pub fn collab_apply_json(&mut self, json: &str) -> Result<String> {
        let ch: CollabChange =
            serde_json::from_str(json).map_err(|e| EngineError::BadCommand(e.to_string()))?;
        self.collab_apply(&ch)?;
        Ok(serde_json::to_string(&self.info()?).unwrap_or_default())
    }

    /// Applies remote changes (and collaborative undo/redo) to the workbook,
    /// outside the local history.
    #[allow(clippy::too_many_lines)]
    pub fn collab_apply(&mut self, change: &CollabChange) -> Result<()> {
        let mut c = self
            .collab
            .take()
            .ok_or_else(|| EngineError::BadCommand("collaboration not started".into()))?;
        let r = self.apply_with(&mut c, change);
        self.collab = Some(c);
        r
    }

    #[allow(clippy::too_many_lines)]
    fn apply_with(&mut self, c: &mut SheetCollab, change: &CollabChange) -> Result<()> {
        if change.is_empty() {
            return Ok(());
        }
        let joining = c.joining;
        let old_axes = c.axes.clone();
        let mut touched_sheets: BTreeSet<String> = BTreeSet::new();
        let mut axis_hit: BTreeSet<(String, bool)> = BTreeSet::new();
        let mut cell_keys: BTreeSet<String> = BTreeSet::new();
        for (k, v) in &change.meta {
            match v {
                Some(j) => c.meta.insert(k.clone(), j.clone()),
                None => c.meta.remove(k),
            };
            let Some((kind, rest)) = k.split_once(':') else {
                continue;
            };
            match kind {
                "ra" | "rd" | "ca" | "cd" => {
                    let Some((sk, id)) = rest.split_once(':') else {
                        continue;
                    };
                    let rows = kind.starts_with('r');
                    let ax = c.axes.entry((sk.to_owned(), rows)).or_default();
                    if kind.ends_with('a') {
                        ax.set_added(id, v.as_deref().and_then(|s| s.parse().ok()));
                    } else {
                        ax.set_deleted(id, v.is_some());
                    }
                    axis_hit.insert((sk.to_owned(), rows));
                }
                "fm" => {
                    cell_keys.insert(rest.to_owned());
                }
                _ => {
                    let sk = rest.split([':', '!']).next().unwrap_or(rest);
                    touched_sheets.insert(sk.to_owned());
                }
            }
        }
        for (k, v) in &change.cells {
            match v {
                Some(j) => c.cells.insert(k.clone(), j.clone()),
                None => c.cells.remove(k),
            };
            cell_keys.insert(k.clone());
        }
        let adjuster = self.adjuster;
        let wb = self.wb.as_mut().ok_or(EngineError::NoDocument)?;
        if joining {
            if !c.meta.keys().any(|k| k.starts_with("sh:")) {
                return Ok(());
            }
            // The shared workbook replaces this replica's copy.
            wb.sheets.clear();
            c.by_id.clear();
            c.snap.clear();
            c.seen.clear();
            c.joining = false;
        }
        // Sheets: add, delete, reorder, bare properties.
        let shared = shared_sheets(&c.meta);
        let cur_keys = c.sheet_keys(wb, false);
        let mut by_key: BTreeMap<String, Worksheet> = cur_keys
            .iter()
            .cloned()
            .zip(std::mem::take(&mut wb.sheets))
            .collect();
        let mut next_id = by_key.values().map(|w| w.sheet_id).max().unwrap_or(0);
        let mut names: BTreeSet<String> = BTreeSet::new();
        for (sk, e) in &shared {
            let mut ws = if let Some(ws) = by_key.remove(sk) {
                if joining || touched_sheets.contains(sk) {
                    merge_bare(&ws, e.w.clone())
                } else {
                    ws
                }
            } else {
                next_id += 1;
                c.by_id.insert(next_id, sk.clone());
                touched_sheets.insert(sk.clone());
                let mut ws = merge_bare(&Worksheet::default(), e.w.clone());
                ws.sheet_id = next_id;
                ws
            };
            // Hyperlinks, AutoFilter and tables where the sender meant them on
            // this replica's rows and columns (see `BareAt`).
            if joining || touched_sheets.contains(sk) {
                if let Some(at) = c
                    .meta
                    .get(&format!("sha:{sk}"))
                    .and_then(|j| serde_json::from_str::<BareAt>(j).ok())
                {
                    at.place(&mut ws, &c.axis(sk, true), &c.axis(sk, false));
                }
            }
            // Two sheets added at once with the same name: number the later one.
            let mut n = 2;
            let base = ws.name.clone();
            while names.contains(&ws.name.to_lowercase()) {
                ws.name = format!("{base} ({n})");
                n += 1;
            }
            names.insert(ws.name.to_lowercase());
            wb.sheets.push(ws);
        }
        for (_, ws) in by_key {
            c.by_id.remove(&ws.sheet_id);
        }
        if wb.sheets.is_empty() {
            wb.sheets.push(Worksheet::new("Sheet1"));
        }
        let keys = c.sheet_keys(wb, false);
        let idx: BTreeMap<String, usize> = keys.iter().cloned().zip(0..).collect();
        // Rows and columns inserted/deleted by others (formulas adjust as for a local edit).
        if !joining {
            for (sk, rows) in &axis_hit {
                let Some(&si) = idx.get(sk) else { continue };
                let empty = Axis::default();
                let old = old_axes.get(&(sk.clone(), *rows)).unwrap_or(&empty);
                let new = c.axis(sk, *rows);
                let (del, ins) = axis_steps(old, &new);
                for p in del {
                    let r = if *rows {
                        bo_sheet::ops::delete_rows(wb, si, p, 1, &adjuster)
                    } else {
                        bo_sheet::ops::delete_cols(wb, si, p, 1, &adjuster)
                    };
                    r.map_err(|e| EngineError::Sheet(e.to_string()))?;
                }
                for p in ins {
                    let r = if *rows {
                        bo_sheet::ops::insert_rows(wb, si, p, 1, &adjuster)
                    } else {
                        bo_sheet::ops::insert_cols(wb, si, p, 1, &adjuster)
                    };
                    r.map_err(|e| EngineError::Sheet(e.to_string()))?;
                }
                touched_sheets.insert(sk.clone());
            }
        }
        // Cells to reconcile: changed ones, and every shared cell of a
        // sheet whose rows/columns changed.
        let structural: BTreeSet<&String> = axis_hit.iter().map(|(s, _)| s).collect();
        let mut all: Vec<String> = cell_keys.into_iter().collect();
        if joining || !structural.is_empty() {
            let extra: Vec<String> = c
                .cells
                .keys()
                .cloned()
                .chain(
                    c.meta
                        .keys()
                        .filter_map(|k| k.strip_prefix("fm:").map(str::to_owned)),
                )
                .filter(|k| {
                    joining
                        || split_cell_key(k)
                            .is_some_and(|(sk, _, _)| structural.contains(&sk.to_owned()))
                })
                .collect();
            all.extend(extra);
            all.sort();
            all.dedup();
        }
        let mut changed: BTreeMap<usize, Vec<(u32, u32)>> = BTreeMap::new();
        let mut style_memo: HashMap<String, u32> = HashMap::new();
        for key in &all {
            let Some((sk, r, col)) = split_cell_key(key) else {
                continue;
            };
            let Some(&si) = idx.get(sk) else { continue };
            let (ra, ca) = (c.axis(sk, true), c.axis(sk, false));
            let (Some(row), Some(cn)) = (ra.pos_of(r), ca.pos_of(col)) else {
                continue;
            };
            if row >= MAX_ROWS || cn >= MAX_COLS {
                continue;
            }
            let model = wb.sheets[si].cells.get(&(row, cn)).cloned();
            let want_v = c.cells.get(key);
            let want_s = c.meta.get(&format!("fm:{key}"));
            let have_v = model.as_ref().and_then(|m| c.cell_json(wb, &keys, sk, m));
            let have_s = model.as_ref().and_then(|m| style_json(&wb.styles, m.style));
            if have_v.as_ref() == want_v && have_s.as_ref() == want_s {
                if let Some(m) = model {
                    c.snap.insert(key.clone(), m);
                }
                continue;
            }
            let mut cell = Cell::default();
            if let Some(sc) = want_v.and_then(|j| serde_json::from_str::<SharedCell>(j).ok()) {
                cell.value = sc.v.unwrap_or_default();
                cell.formula = sc.f.as_ref().map(|f| Formula {
                    text: c.formula_text(wb, &keys, sk, f),
                    kind: f.kind.clone(),
                    calc_always: f.ca,
                    attrs: f.attrs.clone(),
                });
                cell.cm = sc.cm;
                cell.vm = sc.vm;
                cell.show_phonetic = sc.ph;
            }
            if let Some(j) = want_s {
                cell.style = *style_memo
                    .entry(j.clone())
                    .or_insert_with(|| intern_style(&mut wb.styles, j));
            }
            let empty = want_v.is_none() && want_s.is_none();
            let ws = &mut wb.sheets[si];
            if empty {
                ws.cells.remove(&(row, cn));
                c.snap.remove(key);
            } else {
                ws.cells.insert((row, cn), cell.clone());
                c.snap.insert(key.clone(), cell);
            }
            changed.entry(si).or_default().push((row, cn));
        }
        // Row/column properties, merges, rules and objects of touched sheets.
        let rebuild_all = joining || !structural.is_empty();
        for (si, sk) in keys.iter().enumerate() {
            let hit = |p: &str| {
                change
                    .meta
                    .keys()
                    .any(|k| k.starts_with(&format!("{p}:{sk}")))
            };
            let (ra, ca) = (c.axis(sk, true), c.axis(sk, false));
            let ws = &mut wb.sheets[si];
            let any = rebuild_all && (joining || structural.contains(sk));
            if any || hit("rp") {
                ws.rows = keys_with(&c.meta, &format!("rp:{sk}:"))
                    .into_iter()
                    .filter_map(|k| {
                        let pos = ra.pos_of(&k[4 + sk.len()..])?;
                        let info: RowInfo = serde_json::from_str(c.meta.get(&k)?).ok()?;
                        Some((pos, info))
                    })
                    .collect();
            }
            if any || hit("cp") {
                let mut cols: Vec<ColInfo> = keys_with(&c.meta, &format!("cp:{sk}:"))
                    .into_iter()
                    .filter_map(|k| {
                        let pos = ca.pos_of(&k[4 + sk.len()..])?;
                        let e: ColEntry = serde_json::from_str(c.meta.get(&k)?).ok()?;
                        Some(ColInfo {
                            min: pos,
                            max: pos + e.n.max(1) - 1,
                            ..e.i
                        })
                    })
                    .collect();
                cols.sort_by_key(|x| x.min);
                ws.cols = cols;
            }
            if any || hit("mg") {
                ws.merges = keys_with(&c.meta, &format!("mg:{sk}!"))
                    .into_iter()
                    .filter_map(|k| {
                        let (_, r1, c1) = split_cell_key(&k[3..])?;
                        let (r2, c2) = c.meta.get(&k)?.split_once(',')?;
                        let a = CellRef::new(ra.pos_of(r1)?, ca.pos_of(c1)?);
                        let b = CellRef::new(ra.pos_of(r2)?, ca.pos_of(c2)?);
                        Some(CellRange::new(a, b))
                    })
                    .collect();
            }
            if joining || hit("cf") || hit("cfa") {
                let e: CfEntry = c
                    .meta
                    .get(&format!("cf:{sk}"))
                    .and_then(|j| serde_json::from_str(j).ok())
                    .unwrap_or_default();
                let mut map = BTreeMap::new();
                for (old, d) in e.dx {
                    let i = if let Some(i) = wb.styles.dxfs.iter().position(|x| *x == d) {
                        i
                    } else {
                        wb.styles.dxfs.push(d);
                        wb.styles.dxfs.len() - 1
                    };
                    map.insert(old, u32::try_from(i).unwrap_or(0));
                }
                let mut cf = e.cf;
                for rule in cf.iter_mut().flat_map(|x| &mut x.rules) {
                    rule.dxf_id = rule.dxf_id.and_then(|d| map.get(&d).copied());
                }
                let at = c.meta.get(&format!("cfa:{sk}"));
                place_by_id(cf.iter_mut().map(|x| &mut x.sqref), at, &ra, &ca);
                wb.sheets[si].conditional_formats = cf;
            }
            // Charts: anchors and series references where the sender meant
            // them on this replica's rows and columns (`cha:`).
            let charts = (joining || hit("ch") || hit("cha")).then(|| {
                let wb_ref: &Workbook = wb;
                (0..)
                    .map_while(|i| c.meta.get(&format!("ch:{sk}:{i}")).map(|j| (i, j)))
                    .filter_map(|(i, j)| {
                        let mut ch: bo_model::sheet::Chart = serde_json::from_str(j).ok()?;
                        if let Some(at) = c
                            .meta
                            .get(&format!("cha:{sk}:{i}"))
                            .and_then(|j| serde_json::from_str::<ChartAt>(j).ok())
                        {
                            if let (Some(r), Some(col)) = (ra.pos_of(&at.f[0]), ca.pos_of(&at.f[1]))
                            {
                                ch.from = CellRef::new(r, col);
                            }
                            if let Some([r, col]) = &at.t {
                                if let (Some(r), Some(col)) = (ra.pos_of(r), ca.pos_of(col)) {
                                    ch.to = Some(CellRef::new(r, col));
                                }
                            }
                            let series = ch.plots.iter_mut().flat_map(|p| &mut p.series);
                            for (s, refs) in series.zip(&at.s) {
                                for (slot, sf) in [&mut s.name_ref, &mut s.cat_ref, &mut s.val_ref]
                                    .into_iter()
                                    .zip(refs)
                                {
                                    if let (Some(slot), Some(sf)) = (slot.as_mut(), sf) {
                                        *slot = c.formula_text(wb_ref, &keys, sk, sf);
                                    }
                                }
                            }
                        }
                        Some(ch)
                    })
                    .collect::<Vec<bo_model::sheet::Chart>>()
            });
            let ws = &mut wb.sheets[si];
            if joining || hit("dv") || hit("dva") {
                let mut dv: Vec<DataValidation> = c
                    .meta
                    .get(&format!("dv:{sk}"))
                    .and_then(|j| serde_json::from_str(j).ok())
                    .unwrap_or_default();
                let at = c.meta.get(&format!("dva:{sk}"));
                place_by_id(dv.iter_mut().map(|x| &mut x.sqref), at, &ra, &ca);
                ws.data_validations = dv;
            }
            if let Some(charts) = charts {
                ws.charts = charts;
            }
            if joining || hit("pv") {
                ws.pivots = (0..)
                    .map_while(|i| c.meta.get(&format!("pv:{sk}:{i}")))
                    .filter_map(|j| serde_json::from_str(j).ok())
                    .collect();
            }
        }
        if joining || change.meta.keys().any(|k| k == "wb") {
            if let Some(e) = c
                .meta
                .get("wb")
                .and_then(|j| serde_json::from_str::<WbEntry>(j).ok())
            {
                wb.defined_names = e.names;
            }
        }
        if joining || change.meta.keys().any(|k| k.starts_with("pc:")) {
            wb.pivot_caches = (0..)
                .map_while(|i| c.meta.get(&format!("pc:{i}")))
                .filter_map(|j| serde_json::from_str(j).ok())
                .collect();
        }
        // Everything now shown counts as shared.
        let _ = c.local_change(wb, false);
        // Recalculate (structure may have changed: rebuild) and redraw.
        let wb = self.wb.as_mut().ok_or(EngineError::NoDocument)?;
        if joining || !structural.is_empty() || !touched_sheets.is_empty() {
            self.live.rebuild(wb);
        } else {
            for (si, cells) in &changed {
                self.live.update(wb, *si, cells);
            }
        }
        self.geometry = (0..wb.sheets.len()).map(|_| None).collect();
        self.undo.clear();
        self.redo.clear();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn axis_positions_follow_inserts_and_deletes() {
        let mut a = Axis::default();
        assert_eq!(a.id_at(5), "5");
        a.set_added("n1", Some(2.5));
        assert_eq!(a.id_at(3), "n1");
        assert_eq!(a.id_at(4), "3");
        assert_eq!(a.pos_of("3"), Some(4));
        assert_eq!(a.pos_of("n1"), Some(3));
        a.set_deleted("1", true);
        assert_eq!(a.pos_of("1"), None);
        assert_eq!(a.id_at(1), "2");
        assert_eq!(a.id_at(2), "n1");
        assert_eq!(a.pos_of("n1"), Some(2));
        for p in 0..20 {
            assert_eq!(a.pos_of(&a.id_at(p)), Some(p), "{p}");
        }
        let old = a.clone();
        a.set_added("n2", Some(-1.0));
        a.set_deleted("n1", true);
        let (del, ins) = axis_steps(&old, &a);
        assert_eq!(del, vec![2]);
        assert_eq!(ins, vec![0]);
    }
}
