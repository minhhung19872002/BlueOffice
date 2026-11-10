//! Report computation of a pivot table (ECMA-376 Part 1, §18.10).
//!
//! Steps, following the model of §18.10.1.73 `pivotTableDefinition`:
//! 1. **Records** of the cache pass the report filters (`pageFields`) and the
//!    hidden items of the axis fields (`item/@h`).
//! 2. **Label / value / top-N filters** (§18.10.1.35 `filter`) are applied
//!    field by field, outer row fields first, then the column fields; each
//!    filter compares the items under one parent item, so subtotals and grand
//!    totals only include the items left visible.
//! 3. **Aggregation** of every data field for every pair of row / column
//!    prefixes (the item, its parents' subtotals and the grand totals) with
//!    the `subtotal` function (§18.18.17); calculated fields are evaluated
//!    over the sums of the fields they name.
//! 4. **Ordering** of the items under each parent: item order (`manual`),
//!    `ascending` / `descending` by caption, or by a data field (`autoSortScope`).
//! 5. **"Show values as"** (`showDataAs`, §18.18.70, and the x14 additions) is
//!    computed from the aggregated cells.
//! 6. **Layout**: compact, outline or tabular rows with subtotals at the top or
//!    bottom, blank lines, repeated labels; columns with subtotals; grand totals
//!    (`rowGrandTotals` / `colGrandTotals`). The `rowItems` / `colItems` lines
//!    (§18.10.1.84 / §18.10.1.10) are produced alongside the cells.

use super::calc::{self, CalcField};
use super::group;
use super::{value_name, PivotLabels, PivotSpec};
use bo_formula::DateSystem;
use bo_model::sheet::{
    pivot_item_cmp, CellError, CellValue, PivotAxisItem, PivotCache, PivotFieldFilter,
    PivotFilterType, PivotFunction, PivotItemKey, PivotLayout, PivotShowAs, PivotSort,
    PIVOT_BASE_NEXT, PIVOT_BASE_PREVIOUS,
};
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

/// Maximum number of report cells.
const MAX_REPORT_CELLS: u64 = 2_000_000;

/// Running aggregate of one data cell.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Acc {
    count: u64,
    nums: u64,
    sum: f64,
    sumsq: f64,
    prod: f64,
    min: f64,
    max: f64,
    /// Error of a data model measure (`0`: none, see [`err_code`]).
    err: u8,
}

/// Code of an error stored in an [`Acc`] (a data model measure's result).
fn err_code(e: &CellError) -> u8 {
    match e {
        CellError::Null => 1,
        CellError::Div0 => 2,
        CellError::Value => 3,
        CellError::Ref => 4,
        CellError::Name => 5,
        CellError::Num => 6,
        CellError::NA => 7,
        _ => 8,
    }
}

fn code_err(c: u8) -> CellError {
    match c {
        1 => CellError::Null,
        2 => CellError::Div0,
        3 => CellError::Value,
        4 => CellError::Ref,
        5 => CellError::Name,
        6 => CellError::Num,
        7 => CellError::NA,
        _ => CellError::Calc,
    }
}

/// A constraint of a report cell on one cache field, handed to a [`MeasureSource`].
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct FieldFilter {
    /// Cache field.
    pub field: u32,
    /// Values kept (or dropped, `exclude`).
    pub values: Vec<CellValue>,
    /// Drop `values` instead of keeping them.
    pub exclude: bool,
}

/// Values of data model measures (`datamodel::pivot`): a data field over a measure
/// field takes, in every report cell, the measure evaluated in the cell's filters.
pub(crate) trait MeasureSource {
    /// Cache field `field` is a measure.
    fn is_measure(&self, field: u32) -> bool;
    /// Value of measure field `field` under `filters`.
    fn measure(&self, field: u32, filters: &[FieldFilter]) -> CellValue;
}

impl Default for Acc {
    fn default() -> Self {
        Acc {
            count: 0,
            nums: 0,
            sum: 0.0,
            sumsq: 0.0,
            prod: 1.0,
            min: f64::INFINITY,
            max: f64::NEG_INFINITY,
            err: 0,
        }
    }
}

impl Acc {
    /// An aggregate holding a measure's value (blank: no values).
    fn of_value(v: &CellValue) -> Acc {
        match v {
            CellValue::Number(n) => Acc::of_number(*n),
            CellValue::Error(e) => Acc {
                count: 1,
                err: err_code(e),
                ..Acc::default()
            },
            CellValue::Empty => Acc::default(),
            _ => Acc {
                count: 1,
                ..Acc::default()
            },
        }
    }

    /// An aggregate holding the single number `v` (a calculated item's result).
    fn of_number(v: f64) -> Acc {
        Acc {
            count: 1,
            nums: 1,
            sum: v,
            sumsq: v * v,
            prod: v,
            min: v,
            max: v,
            err: 0,
        }
    }

    /// Add another aggregate (totals including calculated items).
    fn merge(&mut self, o: &Acc) {
        self.count += o.count;
        self.nums += o.nums;
        self.sum += o.sum;
        self.sumsq += o.sumsq;
        self.prod *= o.prod;
        self.min = self.min.min(o.min);
        self.max = self.max.max(o.max);
    }

    fn add(&mut self, v: &CellValue) {
        if matches!(v, CellValue::Empty) {
            return;
        }
        self.count += 1;
        if let CellValue::Number(n) = v {
            self.nums += 1;
            self.sum += n;
            self.sumsq += n * n;
            self.prod *= n;
            self.min = self.min.min(*n);
            self.max = self.max.max(*n);
        }
    }

    /// Result of `f` (§18.18.17); a group without values is blank.
    fn result(&self, f: PivotFunction) -> CellValue {
        if self.count == 0 {
            return CellValue::Empty;
        }
        if self.err != 0 {
            return CellValue::Error(code_err(self.err));
        }
        let n = self.nums as f64;
        let var = |pop: bool| {
            let d = if pop { n } else { n - 1.0 };
            if d <= 0.0 {
                return None;
            }
            Some(((self.sumsq - self.sum * self.sum / n) / d).max(0.0))
        };
        let num = |x: Option<f64>| x.map_or(CellValue::Error(CellError::Div0), CellValue::Number);
        match f {
            PivotFunction::Sum => CellValue::Number(self.sum),
            PivotFunction::Count => CellValue::Number(self.count as f64),
            PivotFunction::CountNums => CellValue::Number(n),
            PivotFunction::Average => num((self.nums > 0).then(|| self.sum / n)),
            PivotFunction::Max => CellValue::Number(if self.nums > 0 { self.max } else { 0.0 }),
            PivotFunction::Min => CellValue::Number(if self.nums > 0 { self.min } else { 0.0 }),
            PivotFunction::Product => {
                CellValue::Number(if self.nums > 0 { self.prod } else { 0.0 })
            }
            PivotFunction::Var => num(var(false)),
            PivotFunction::Varp => num(var(true)),
            PivotFunction::StdDev => num(var(false).map(f64::sqrt)),
            PivotFunction::StdDevp => num(var(true).map(f64::sqrt)),
        }
    }
}

/// Kind of a report body row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowKind {
    /// Column headers (above the first data row).
    Header,
    /// An outline / compact group line (label, values when subtotals are on top).
    Group,
    /// An item line with values.
    Data,
    /// "<item> Total".
    Subtotal,
    /// An empty separator line (`insertBlankRow`, §18.10.1.33).
    Blank,
    /// The grand total line.
    Grand,
}

/// Kind of a report data column.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColKind {
    /// A column of values for one column item.
    Data,
    /// A subtotal column of an outer column field.
    Subtotal,
    /// The grand total column.
    Grand,
}

/// A computed report: cells relative to the top-left of the page-field area.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Layout {
    /// `(row, col, value)`.
    pub cells: Vec<(u32, u32, CellValue)>,
    /// Rows above the body taken by report filters (filters + one blank row).
    pub page_rows: u32,
    /// Body size.
    pub height: u32,
    /// Body width in columns.
    pub width: u32,
    /// Body row of the (last) header line.
    pub first_header_row: u32,
    /// Body row of the first item / value line.
    pub first_data_row: u32,
    /// Body column of the first value column (after the row labels).
    pub first_data_col: u32,
    /// Body rows drawn bold (headers, subtotals, grand total).
    pub bold_rows: Vec<u32>,
    /// Body columns holding values: `(col, value index)`.
    pub data_cols: Vec<(u32, usize)>,
    /// Kind of every body row (headers included).
    pub row_kinds: Vec<RowKind>,
    /// Kind of every entry of [`Layout::data_cols`].
    pub col_kinds: Vec<ColKind>,
    /// Indented label cells (compact layout): `(body row, col, level)`.
    pub indents: Vec<(u32, u32, u32)>,
    /// Item label cells: `(body row, col, cache field)` (number formats of dates).
    pub label_fields: Vec<(u32, u32, u32)>,
    /// Value fields on rows (`dataOnRows`): the value index of every body row
    /// showing values; [`Layout::data_cols`] then all name value 0.
    pub data_rows: Vec<(u32, usize)>,
    /// `rowItems` / `colItems`.
    pub row_items: Vec<PivotAxisItem>,
    /// Column axis items written as `colItems` (§18.10.1.13).
    pub col_items: Vec<PivotAxisItem>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Axis {
    Row,
    Col,
}

/// How a data field is computed from the aggregate slots.
enum Measure {
    Plain(usize),
    Calc(CalcField, Vec<(u32, usize)>),
    /// A data model measure: its slot holds the measure's value (filled after aggregation).
    Model(usize),
}

/// A record that passed the filters, with its row and column item keys.
struct Path {
    rec: usize,
    row: Vec<u32>,
    col: Vec<u32>,
}

impl Path {
    fn key(&self, axis: Axis) -> &[u32] {
        match axis {
            Axis::Row => &self.row,
            Axis::Col => &self.col,
        }
    }
}

/// Row and column item paths of a report cell.
type CellKey = (Vec<u32>, Vec<u32>);

type Cells = HashMap<CellKey, Vec<Acc>>;

/// Full row and column item paths of a record (borrowed from its [`Path`]).
type LeafKey<'p> = (&'p [u32], &'p [u32]);

/// One line of the row axis.
#[derive(Debug, Clone)]
struct Line {
    kind: RowKind,
    key: Vec<u32>,
    /// The line shows values (group lines only with subtotals on top).
    values: bool,
}

/// A date filter value: a serial number or an ISO 8601 date.
fn date_value(s: &str, sys: DateSystem) -> Option<f64> {
    let t = s.trim();
    t.parse::<f64>().ok().or_else(|| {
        // ISO dates only (locale independent).
        t.starts_with(|c: char| c.is_ascii_digit())
            .then(|| {
                crate::numfmt::parse_typed_date(
                    t,
                    &crate::numfmt::Locale::EN_US,
                    sys == DateSystem::Date1904,
                )
            })
            .flatten()
            .filter(|_| t.len() >= 8 && t.as_bytes().get(4) == Some(&b'-'))
            .map(|d| d.serial)
    })
}

/// Whether the date `serial` passes the date filter `flt` (§18.18.60
/// `ST_PivotFilterType`): fixed dates compare whole days; relative periods
/// (weeks start on Sunday) need `today`, without it they keep every item.
fn date_matches(flt: &PivotFieldFilter, serial: f64, today: Option<f64>, sys: DateSystem) -> bool {
    use PivotFilterType as T;
    let d1904 = sys == DateSystem::Date1904;
    let day = serial.floor();
    let parts = |s: f64| {
        crate::numfmt::serial_to_datetime(s, d1904, 0).map(|d| (d.year, d.month, d.weekday))
    };
    let Some((y, m, _)) = parts(day) else {
        return false;
    };
    let kind = flt.kind;
    if kind.is_fixed_date() {
        let Some(a) = date_value(&flt.value1, sys).map(f64::floor) else {
            return false;
        };
        let b = date_value(&flt.value2, sys).map(f64::floor);
        return match kind {
            T::DateEqual => day == a,
            T::DateNotEqual => day != a,
            T::DateOlderThan => day < a,
            T::DateOlderThanOrEqual => day <= a,
            T::DateNewerThan => day > a,
            T::DateNewerThanOrEqual => day >= a,
            T::DateBetween => b.is_some_and(|b| day >= a.min(b) && day <= a.max(b)),
            T::DateNotBetween => b.is_some_and(|b| day < a.min(b) || day > a.max(b)),
            _ => true,
        };
    }
    let k = kind as u32;
    if (T::Q1 as u32..=T::Q4 as u32).contains(&k) {
        return (m - 1) / 3 == k - T::Q1 as u32;
    }
    if (T::M1 as u32..=T::M12 as u32).contains(&k) {
        return m - 1 == k - T::M1 as u32;
    }
    let Some(t) = today.map(f64::floor) else {
        return true;
    };
    let Some((ty, tm, tw)) = parts(t) else {
        return true;
    };
    let month = |yy: i32, mm: u32| i64::from(yy) * 12 + i64::from(mm);
    let quarter = |yy: i32, mm: u32| i64::from(yy) * 4 + i64::from((mm - 1) / 3);
    let (mine_m, cur_m) = (month(y, m), month(ty, tm));
    let (mine_q, cur_q) = (quarter(y, m), quarter(ty, tm));
    let week = t - f64::from(tw);
    match kind {
        T::Tomorrow => day == t + 1.0,
        T::Today => day == t,
        T::Yesterday => day == t - 1.0,
        T::ThisWeek => day >= week && day < week + 7.0,
        T::LastWeek => day >= week - 7.0 && day < week,
        T::NextWeek => day >= week + 7.0 && day < week + 14.0,
        T::ThisMonth => mine_m == cur_m,
        T::LastMonth => mine_m == cur_m - 1,
        T::NextMonth => mine_m == cur_m + 1,
        T::ThisQuarter => mine_q == cur_q,
        T::LastQuarter => mine_q == cur_q - 1,
        T::NextQuarter => mine_q == cur_q + 1,
        T::ThisYear => y == ty,
        T::LastYear => y == ty - 1,
        T::NextYear => y == ty + 1,
        T::YearToDate => y == ty && day <= t,
        _ => true,
    }
}

/// The computation context of one report.
pub(crate) struct Engine<'a> {
    cache: &'a PivotCache,
    spec: &'a PivotSpec,
    labels: &'a PivotLabels,
    sys: DateSystem,
    /// Shared item index of every database field.
    index: Vec<HashMap<PivotItemKey, u32>>,
    measures: Vec<Measure>,
    /// Cache field aggregated by each slot.
    slot_fields: Vec<u32>,
    cells: Cells,
    row_children: HashMap<Vec<u32>, Vec<u32>>,
    col_children: HashMap<Vec<u32>, Vec<u32>>,
    /// Data model measures (a pivot table over the workbook data model).
    model: Option<&'a dyn MeasureSource>,
}

impl<'a> Engine<'a> {
    pub(crate) fn new(
        cache: &'a PivotCache,
        spec: &'a PivotSpec,
        labels: &'a PivotLabels,
        sys: DateSystem,
    ) -> Result<Engine<'a>, String> {
        Self::with_measures(cache, spec, labels, sys, None)
    }

    /// [`Engine::new`] for a pivot table over the data model.
    pub(crate) fn with_measures(
        cache: &'a PivotCache,
        spec: &'a PivotSpec,
        labels: &'a PivotLabels,
        sys: DateSystem,
        model: Option<&'a dyn MeasureSource>,
    ) -> Result<Engine<'a>, String> {
        let nf = cache.fields.len() as u32;
        let used = spec
            .rows
            .iter()
            .chain(&spec.cols)
            .copied()
            .chain(spec.values.iter().map(|d| d.field))
            .chain(spec.filters.iter().map(|f| f.field))
            .chain(spec.field_filters.iter().map(|f| f.field));
        if used.into_iter().any(|f| f >= nf) {
            return Err("unknown pivot field".into());
        }
        for f in spec.rows.iter().chain(&spec.cols) {
            if cache.fields[*f as usize].formula.is_some() {
                return Err("a calculated field can only be a value field".into());
            }
            if model.is_some_and(|m| m.is_measure(*f)) {
                return Err("a measure can only be a value field".into());
            }
        }
        // Item indexes are only looked up for the axis and page fields (and the base
        // fields of their discrete groups): a field of a million distinct values that
        // is only summed needs none.
        let mut indexed = vec![false; cache.fields.len()];
        for f in spec
            .rows
            .iter()
            .chain(&spec.cols)
            .copied()
            .chain(spec.filters.iter().map(|f| f.field))
        {
            indexed[f as usize] = true;
            if let Some(b) = cache.fields[f as usize].group.as_ref().and_then(|g| g.base) {
                if let Some(x) = indexed.get_mut(b as usize) {
                    *x = true;
                }
            }
        }
        // Over the data model, hidden items of any field filter the records.
        if model.is_some() {
            for (i, o) in spec.fields.iter().enumerate() {
                if !o.hidden.is_empty() {
                    if let Some(x) = indexed.get_mut(i) {
                        *x = true;
                    }
                }
            }
        }
        let index = cache
            .fields
            .iter()
            .zip(&indexed)
            .map(|(f, used)| {
                if !used {
                    return HashMap::new();
                }
                f.items
                    .iter()
                    .enumerate()
                    .map(|(i, v)| (PivotItemKey::of(v), i as u32))
                    .collect()
            })
            .collect();
        let mut slot_fields: Vec<u32> = Vec::new();
        let slot = |f: u32, slots: &mut Vec<u32>| {
            slots.iter().position(|s| *s == f).unwrap_or_else(|| {
                slots.push(f);
                slots.len() - 1
            })
        };
        let mut measures = Vec::with_capacity(spec.values.len());
        for d in &spec.values {
            let field = &cache.fields[d.field as usize];
            if model.is_some_and(|m| m.is_measure(d.field)) {
                // Its own slot (never shared with a plain field): records add nothing to it.
                slot_fields.push(d.field);
                measures.push(Measure::Model(slot_fields.len() - 1));
                continue;
            }
            if let Some(formula) = &field.formula {
                let c = calc::compile(formula, &cache.fields)?;
                let slots = c
                    .refs
                    .iter()
                    .map(|(_, f)| (*f, slot(*f, &mut slot_fields)))
                    .collect();
                measures.push(Measure::Calc(c, slots));
            } else {
                measures.push(Measure::Plain(slot(d.field, &mut slot_fields)));
            }
        }
        Ok(Engine {
            cache,
            spec,
            labels,
            sys,
            index,
            measures,
            slot_fields,
            cells: HashMap::new(),
            row_children: HashMap::new(),
            col_children: HashMap::new(),
            model,
        })
    }

    /// The record value of field `f` (a grouping field reads its base field).
    fn value<'r>(&self, rec: &'r [CellValue], f: u32) -> &'r CellValue {
        let fld = &self.cache.fields[f as usize];
        let src = match &fld.group {
            Some(g) if fld.derived => g.base.unwrap_or(f),
            _ => f,
        };
        rec.get(src as usize).unwrap_or(&CellValue::Empty)
    }

    /// Items of field `f` as shown on an axis.
    pub(crate) fn items(&self, f: u32) -> &'a [CellValue] {
        let fld = &self.cache.fields[f as usize];
        match &fld.group {
            Some(g) if g.range.is_some() || !g.discrete.is_empty() => &g.items,
            _ => &fld.items,
        }
    }

    /// Header caption of field `f`: its name; over the data model, the column of
    /// `Table[Column]`.
    fn field_caption(&self, f: u32) -> String {
        let n = &self.cache.fields[f as usize].name;
        if self.model.is_some() {
            if let Some((_, c)) = n.split_once('[') {
                return c.strip_suffix(']').unwrap_or(c).to_owned();
            }
        }
        n.clone()
    }

    fn grouped(&self, f: u32) -> bool {
        self.cache.fields[f as usize]
            .group
            .as_ref()
            .is_some_and(|g| g.range.is_some() || !g.discrete.is_empty())
    }

    /// Axis item index of a record for field `f`.
    fn item(&self, rec: &[CellValue], f: u32) -> u32 {
        let fld = &self.cache.fields[f as usize];
        let v = self.value(rec, f);
        if let Some(g) = &fld.group {
            if let Some(r) = &g.range {
                return group::bucket(r, v, g.items.len(), self.sys);
            }
            if !g.discrete.is_empty() {
                let base = g.base.unwrap_or(f) as usize;
                let bi = self
                    .index
                    .get(base)
                    .and_then(|m| m.get(&PivotItemKey::of(v)))
                    .copied()
                    .unwrap_or(0);
                return g.discrete.get(bi as usize).copied().unwrap_or(0);
            }
        }
        self.index[f as usize]
            .get(&PivotItemKey::of(v))
            .copied()
            .unwrap_or(0)
    }

    /// Caption of item `x` of field `f`.
    pub(crate) fn label(&self, f: u32, x: u32) -> CellValue {
        match self.items(f).get(x as usize) {
            None | Some(CellValue::Empty) => CellValue::Text(self.labels.blank.clone()),
            Some(v) => v.clone(),
        }
    }

    fn add(&self, accs: &mut [Acc], rec: &[CellValue]) {
        for (s, f) in self.slot_fields.iter().enumerate() {
            if self.model.is_some_and(|m| m.is_measure(*f)) {
                continue;
            }
            accs[s].add(self.value(rec, *f));
        }
    }

    fn new_accs(&self) -> Vec<Acc> {
        vec![Acc::default(); self.slot_fields.len()]
    }

    /// Result of data field `vi` from its slots.
    fn result(&self, vi: usize, accs: &[Acc]) -> CellValue {
        match &self.measures[vi] {
            Measure::Plain(s) => accs[*s].result(self.spec.values[vi].function),
            Measure::Model(s) => accs[*s].result(PivotFunction::Sum),
            Measure::Calc(c, slots) => {
                if slots.iter().all(|(_, s)| accs[*s].count == 0) {
                    return CellValue::Empty;
                }
                c.eval(|f| {
                    slots
                        .iter()
                        .find(|(x, _)| *x == f)
                        .map_or(0.0, |(_, s)| accs[*s].sum)
                })
            }
        }
    }

    fn axis_fields(&self, axis: Axis) -> &'a [u32] {
        match axis {
            Axis::Row => &self.spec.rows,
            Axis::Col => &self.spec.cols,
        }
    }

    /// Records passing the report filters and hidden items, with their keys.
    fn paths(&self) -> Vec<Path> {
        let key_set = |f: u32, values: &[CellValue]| -> HashSet<u32> {
            let keys: Vec<PivotItemKey> = values.iter().map(PivotItemKey::of).collect();
            self.items(f)
                .iter()
                .enumerate()
                .filter(|(_, v)| keys.contains(&PivotItemKey::of(v)))
                .map(|(i, _)| i as u32)
                .collect()
        };
        let pages: Vec<(u32, HashSet<u32>)> = self
            .spec
            .filters
            .iter()
            .filter(|f| !f.selected.is_empty())
            .map(|f| (f.field, key_set(f.field, &f.selected)))
            .collect();
        // Over the data model, hidden items filter every field (slicers, filters of
        // fields not on an axis); otherwise only the axis fields.
        let all: Vec<u32> = if self.model.is_some() {
            (0..self.cache.fields.len() as u32).collect()
        } else {
            self.spec
                .rows
                .iter()
                .chain(&self.spec.cols)
                .copied()
                .collect()
        };
        let hidden: Vec<(u32, HashSet<u32>)> = all
            .iter()
            .filter_map(|f| {
                let h = &self.spec.field(*f).hidden;
                (!h.is_empty()).then(|| (*f, key_set(*f, h)))
            })
            .collect();
        let mut out = Vec::with_capacity(self.cache.records.len());
        let mut memo: HashMap<(u32, &'a str), u32> = HashMap::new();
        let cache: &'a PivotCache = self.cache;
        for (ri, rec) in cache.records.iter().enumerate() {
            let rec: &'a [CellValue] = rec;
            if !pages
                .iter()
                .all(|(f, s)| s.contains(&self.item_memo(rec, *f, &mut memo)))
            {
                continue;
            }
            if hidden
                .iter()
                .any(|(f, s)| s.contains(&self.item_memo(rec, *f, &mut memo)))
            {
                continue;
            }
            let mut key = |fields: &[u32]| -> Vec<u32> {
                fields
                    .iter()
                    .map(|f| self.item_memo(rec, *f, &mut memo))
                    .collect()
            };
            let row = key(&self.spec.rows);
            let col = key(&self.spec.cols);
            out.push(Path { rec: ri, row, col });
        }
        out
    }

    /// [`Engine::item`] remembering the item of every distinct text of a field, so
    /// that records do not each build a lower-cased item key.
    fn item_memo(
        &self,
        rec: &'a [CellValue],
        f: u32,
        memo: &mut HashMap<(u32, &'a str), u32>,
    ) -> u32 {
        if let CellValue::Text(s) = self.value(rec, f) {
            if let Some(x) = memo.get(&(f, s.as_str())) {
                return *x;
            }
            let x = self.item(rec, f);
            memo.insert((f, s.as_str()), x);
            return x;
        }
        self.item(rec, f)
    }

    /// Numeric result of data field `vi` (`None` when blank or not a number).
    fn number(&self, vi: usize, accs: Option<&Vec<Acc>>) -> Option<f64> {
        match self.result(vi, accs?) {
            CellValue::Number(n) => Some(n),
            _ => None,
        }
    }

    /// The items of `items` (siblings of one parent) that `flt` keeps.
    fn filter_items(
        &self,
        flt: &PivotFieldFilter,
        parent: &[u32],
        items: &[u32],
        agg: &HashMap<Vec<u32>, Vec<Acc>>,
    ) -> Vec<u32> {
        use PivotFilterType as T;
        let f = flt.field;
        if flt.kind.is_date() {
            // Date filters compare the items' dates; grouped items are captions.
            if self.grouped(f) {
                return items.to_vec();
            }
            let values = self.items(f);
            return items
                .iter()
                .copied()
                .filter(|x| {
                    values
                        .get(*x as usize)
                        .and_then(|v| group::number_of(v, self.sys))
                        .is_some_and(|d| date_matches(flt, d, self.labels.today, self.sys))
                })
                .collect();
        }
        if flt.kind.is_caption() {
            let a = flt.value1.to_lowercase();
            let b = flt.value2.to_lowercase();
            let cmp = |text: &str, other: &str| -> Ordering {
                match (text.trim().parse::<f64>(), other.trim().parse::<f64>()) {
                    (Ok(x), Ok(y)) => x.partial_cmp(&y).unwrap_or(Ordering::Equal),
                    _ => text.cmp(other),
                }
            };
            return items
                .iter()
                .copied()
                .filter(|x| {
                    let t = super::value_text(&self.label(f, *x)).to_lowercase();
                    match flt.kind {
                        T::CaptionEqual => t == a,
                        T::CaptionNotEqual => t != a,
                        T::CaptionBeginsWith => t.starts_with(&a),
                        T::CaptionNotBeginsWith => !t.starts_with(&a),
                        T::CaptionEndsWith => t.ends_with(&a),
                        T::CaptionNotEndsWith => !t.ends_with(&a),
                        T::CaptionContains => t.contains(&a),
                        T::CaptionNotContains => !t.contains(&a),
                        T::CaptionGreaterThan => cmp(&t, &a) == Ordering::Greater,
                        T::CaptionGreaterThanOrEqual => cmp(&t, &a) != Ordering::Less,
                        T::CaptionLessThan => cmp(&t, &a) == Ordering::Less,
                        T::CaptionLessThanOrEqual => cmp(&t, &a) != Ordering::Greater,
                        T::CaptionBetween => {
                            cmp(&t, &a) != Ordering::Less && cmp(&t, &b) != Ordering::Greater
                        }
                        T::CaptionNotBetween => {
                            cmp(&t, &a) == Ordering::Less || cmp(&t, &b) == Ordering::Greater
                        }
                        _ => true,
                    }
                })
                .collect();
        }
        let m = flt.measure as usize;
        if m >= self.spec.values.len() {
            return items.to_vec();
        }
        let value = |x: u32| {
            let mut k = parent.to_vec();
            k.push(x);
            self.number(m, agg.get(&k))
        };
        if flt.kind.is_top10() {
            let mut scored: Vec<(u32, f64)> = items
                .iter()
                .filter_map(|x| value(*x).map(|v| (*x, v)))
                .collect();
            scored.sort_by(|a, b| {
                let o = a.1.partial_cmp(&b.1).unwrap_or(Ordering::Equal);
                if flt.top {
                    o.reverse()
                } else {
                    o
                }
            });
            let total: f64 = scored.iter().map(|s| s.1).sum();
            let mut keep = Vec::new();
            let mut acc = 0.0;
            for (i, (x, v)) in scored.iter().enumerate() {
                let take = match flt.kind {
                    T::Count => (i as f64) < flt.count.floor(),
                    T::Percent => acc < total * flt.count / 100.0,
                    _ => acc < flt.count,
                };
                if !take {
                    break;
                }
                acc += v;
                keep.push(*x);
            }
            return items.iter().copied().filter(|x| keep.contains(x)).collect();
        }
        let (Ok(a), b) = (
            flt.value1.trim().parse::<f64>(),
            flt.value2.trim().parse::<f64>().unwrap_or(f64::NAN),
        ) else {
            return items.to_vec();
        };
        items
            .iter()
            .copied()
            .filter(|x| {
                let Some(v) = value(*x) else { return false };
                match flt.kind {
                    T::ValueEqual => v == a,
                    T::ValueNotEqual => v != a,
                    T::ValueGreaterThan => v > a,
                    T::ValueGreaterThanOrEqual => v >= a,
                    T::ValueLessThan => v < a,
                    T::ValueLessThanOrEqual => v <= a,
                    T::ValueBetween => v >= a.min(b) && v <= a.max(b),
                    T::ValueNotBetween => v < a.min(b) || v > a.max(b),
                    _ => true,
                }
            })
            .collect()
    }

    /// Apply the label / value / top-N filters level by level (outer first).
    fn apply_field_filters(&self, paths: &mut Vec<Path>) {
        for axis in [Axis::Row, Axis::Col] {
            for (level, f) in self.axis_fields(axis).iter().enumerate() {
                let filters: Vec<&PivotFieldFilter> = self
                    .spec
                    .field_filters
                    .iter()
                    .filter(|x| x.field == *f)
                    .collect();
                if filters.is_empty() {
                    continue;
                }
                let mut agg: HashMap<Vec<u32>, Vec<Acc>> = HashMap::new();
                let mut siblings: HashMap<Vec<u32>, Vec<u32>> = HashMap::new();
                for p in paths.iter() {
                    let key = &p.key(axis)[..=level];
                    let slot = agg.entry(key.to_vec()).or_insert_with(|| {
                        siblings
                            .entry(key[..level].to_vec())
                            .or_default()
                            .push(key[level]);
                        self.new_accs()
                    });
                    self.add(slot, &self.cache.records[p.rec]);
                }
                let mut keep: HashSet<Vec<u32>> = HashSet::new();
                for (parent, items) in &mut siblings {
                    items.sort_unstable();
                    let mut shown = items.clone();
                    for flt in &filters {
                        shown = self.filter_items(flt, parent, &shown, &agg);
                    }
                    for x in shown {
                        let mut k = parent.clone();
                        k.push(x);
                        keep.insert(k);
                    }
                }
                paths.retain(|p| keep.contains(&p.key(axis)[..=level]));
            }
        }
    }

    /// Aggregate every (row prefix, column prefix) pair.
    ///
    /// Records are first summed per (full row path, full column path) — borrowed
    /// keys, no allocation per record — and the leaf aggregates are then merged into
    /// every prefix (subtotals and grand totals).
    fn aggregate(&mut self, paths: &[Path]) {
        // Leaves in order of first appearance, so that totals add up the same way on
        // every run.
        let mut at: HashMap<LeafKey<'_>, usize> = HashMap::new();
        let mut leaves: Vec<(LeafKey<'_>, Vec<Acc>)> = Vec::new();
        for p in paths {
            let rec = &self.cache.records[p.rec];
            let key = (p.row.as_slice(), p.col.as_slice());
            let i = *at.entry(key).or_insert_with(|| {
                leaves.push((key, self.new_accs()));
                leaves.len() - 1
            });
            self.add(&mut leaves[i].1, rec);
        }
        let mut cells: Cells = HashMap::new();
        for ((row, col), accs) in leaves {
            for lr in 0..=row.len() {
                for lc in 0..=col.len() {
                    match cells.entry((row[..lr].to_vec(), col[..lc].to_vec())) {
                        std::collections::hash_map::Entry::Occupied(mut e) => {
                            for (a, b) in e.get_mut().iter_mut().zip(&accs) {
                                a.merge(b);
                            }
                        }
                        std::collections::hash_map::Entry::Vacant(e) => {
                            e.insert(accs.clone());
                        }
                    }
                }
            }
        }
        self.cells = cells;
        self.apply_calc_items();
        self.apply_model_measures();
    }

    /// Data model measures: every report cell gets the measure evaluated with the
    /// cell's items, the report filters and the hidden items as filters.
    fn apply_model_measures(&mut self) {
        let Some(model) = self.model else {
            return;
        };
        let slots: Vec<(usize, u32)> = self
            .measures
            .iter()
            .filter_map(|m| match m {
                Measure::Model(s) => Some((*s, self.slot_fields[*s])),
                _ => None,
            })
            .collect();
        if slots.is_empty() {
            return;
        }
        let mut base: Vec<FieldFilter> = Vec::new();
        for f in &self.spec.filters {
            if !f.selected.is_empty() {
                base.push(FieldFilter {
                    field: f.field,
                    values: f.selected.clone(),
                    exclude: false,
                });
            }
        }
        for (i, o) in self.spec.fields.iter().enumerate() {
            if !o.hidden.is_empty() {
                base.push(FieldFilter {
                    field: i as u32,
                    values: o.hidden.clone(),
                    exclude: true,
                });
            }
        }
        let keys: Vec<CellKey> = self.cells.keys().cloned().collect();
        for key in keys {
            let mut ctx = base.clone();
            for (fields, items) in [(&self.spec.rows, &key.0), (&self.spec.cols, &key.1)] {
                for (f, x) in fields.iter().zip(items.iter()) {
                    let v = self
                        .items(*f)
                        .get(*x as usize)
                        .cloned()
                        .unwrap_or(CellValue::Empty);
                    ctx.push(FieldFilter {
                        field: *f,
                        values: vec![v],
                        exclude: false,
                    });
                }
            }
            let values: Vec<(usize, Acc)> = slots
                .iter()
                .map(|(s, f)| (*s, Acc::of_value(&model.measure(*f, &ctx))))
                .collect();
            if let Some(accs) = self.cells.get_mut(&key) {
                for (s, a) in values {
                    accs[s] = a;
                }
            }
        }
    }

    /// Calculated items (§18.10.1.8): for every report cell under an item of
    /// the field, the formula over the sums of the items it names (same
    /// context on the other fields); subtotals and grand totals include them.
    /// Every aggregate of the item holds that one number (Count shows 1).
    fn apply_calc_items(&mut self) {
        let cache = self.cache;
        for ci in &cache.calculated_items {
            let f = ci.field;
            let Some((axis, level)) = self.axis_of(f) else {
                continue;
            };
            if self.grouped(f) || (f as usize) >= self.cache.fields.len() {
                continue;
            }
            let items = self.items(f);
            let key = PivotItemKey::of(&CellValue::Text(ci.name.clone()));
            let Some(x) = items.iter().position(|v| PivotItemKey::of(v) == key) else {
                continue;
            };
            let x = x as u32;
            if self
                .spec
                .field(f)
                .hidden
                .iter()
                .any(|h| PivotItemKey::of(h) == key)
            {
                continue;
            }
            let names: Vec<bo_model::sheet::PivotCacheField> = items
                .iter()
                .map(|v| bo_model::sheet::PivotCacheField {
                    name: super::value_text(v),
                    ..bo_model::sheet::PivotCacheField::default()
                })
                .collect();
            let Ok(calc) = calc::compile(&ci.formula, &names) else {
                continue;
            };
            if calc.refs.iter().any(|(_, i)| *i == x) {
                continue;
            }
            let at = |k: &(Vec<u32>, Vec<u32>), i: u32| {
                let mut k = k.clone();
                match axis {
                    Axis::Row => k.0[level] = i,
                    Axis::Col => k.1[level] = i,
                }
                k
            };
            let own = |k: &(Vec<u32>, Vec<u32>)| match axis {
                Axis::Row => k.0.len(),
                Axis::Col => k.1.len(),
            };
            let mut made: Cells = HashMap::new();
            let keys: Vec<(Vec<u32>, Vec<u32>)> = self
                .cells
                .keys()
                .filter(|k| own(k) > level)
                .cloned()
                .collect();
            for k in keys {
                let target = at(&k, x);
                if made.contains_key(&target) || self.cells.contains_key(&target) {
                    continue;
                }
                if !calc
                    .refs
                    .iter()
                    .any(|(_, i)| self.cells.contains_key(&at(&target, *i)))
                {
                    continue;
                }
                let accs: Vec<Acc> = (0..self.slot_fields.len())
                    .map(|s| {
                        let v = calc
                            .eval(|i| self.cells.get(&at(&target, i)).map_or(0.0, |a| a[s].sum));
                        match v {
                            CellValue::Number(n) => Acc::of_number(n),
                            _ => Acc {
                                count: 1,
                                ..Acc::default()
                            },
                        }
                    })
                    .collect();
                made.insert(target, accs);
            }
            // Totals of the parents (and grand totals) include the item.
            let mut totals: Vec<(CellKey, Vec<Acc>)> = Vec::new();
            for (k, accs) in &made {
                if own(k) != level + 1 {
                    continue;
                }
                for j in 0..=level {
                    let mut p = k.clone();
                    match axis {
                        Axis::Row => p.0.truncate(j),
                        Axis::Col => p.1.truncate(j),
                    }
                    totals.push((p, accs.clone()));
                }
            }
            let n = self.slot_fields.len();
            for (p, accs) in totals {
                let slot = self
                    .cells
                    .entry(p)
                    .or_insert_with(|| vec![Acc::default(); n]);
                for (a, b) in slot.iter_mut().zip(&accs) {
                    a.merge(b);
                }
            }
            self.cells.extend(made);
        }
    }

    fn cell_accs(&self, rp: &[u32], cp: &[u32]) -> Option<&Vec<Acc>> {
        self.cells.get(&(rp.to_vec(), cp.to_vec()))
    }

    /// Ordered children of every prefix of an axis.
    fn children(&self, axis: Axis) -> HashMap<Vec<u32>, Vec<u32>> {
        let fields = self.axis_fields(axis);
        let mut ch: HashMap<Vec<u32>, Vec<u32>> = HashMap::new();
        for (rp, cp) in self.cells.keys() {
            let (key, other) = match axis {
                Axis::Row => (rp, cp),
                Axis::Col => (cp, rp),
            };
            if !other.is_empty() || key.is_empty() {
                continue;
            }
            ch.entry(key[..key.len() - 1].to_vec())
                .or_default()
                .push(key[key.len() - 1]);
        }
        for (parent, items) in &mut ch {
            let level = parent.len();
            let f = fields[level];
            let opts = self.spec.field(f);
            let grouped = self.grouped(f);
            let values = self.items(f);
            let by_label = |a: &u32, b: &u32| -> Ordering {
                if grouped {
                    a.cmp(b)
                } else {
                    match (values.get(*a as usize), values.get(*b as usize)) {
                        (Some(x), Some(y)) => pivot_item_cmp(x, y).then(a.cmp(b)),
                        _ => a.cmp(b),
                    }
                }
            };
            match (opts.sort_by, opts.sort) {
                (Some(v), sort) if (v as usize) < self.spec.values.len() => {
                    let score = |x: u32| {
                        let mut k = parent.clone();
                        k.push(x);
                        let accs = match axis {
                            Axis::Row => self.cell_accs(&k, &[]),
                            Axis::Col => self.cell_accs(&[], &k),
                        };
                        self.number(v as usize, accs)
                    };
                    let desc = sort == PivotSort::Descending;
                    items.sort_by(|a, b| match (score(*a), score(*b)) {
                        (Some(x), Some(y)) => {
                            let o = x.partial_cmp(&y).unwrap_or(Ordering::Equal);
                            if desc { o.reverse() } else { o }.then(a.cmp(b))
                        }
                        (Some(_), None) => Ordering::Less,
                        (None, Some(_)) => Ordering::Greater,
                        (None, None) => a.cmp(b),
                    });
                }
                (_, PivotSort::Ascending) => items.sort_by(by_label),
                (_, PivotSort::Descending) => items.sort_by(|a, b| by_label(b, a)),
                (_, PivotSort::Manual) => items.sort_unstable(),
            }
        }
        ch
    }

    fn siblings(&self, axis: Axis, parent: &[u32]) -> &[u32] {
        let ch = match axis {
            Axis::Row => &self.row_children,
            Axis::Col => &self.col_children,
        };
        ch.get(parent).map_or(&[], Vec::as_slice)
    }

    /// Axis and level of a cache field, if it is a row or column field.
    fn axis_of(&self, f: u32) -> Option<(Axis, usize)> {
        if let Some(l) = self.spec.rows.iter().position(|x| *x == f) {
            return Some((Axis::Row, l));
        }
        self.spec
            .cols
            .iter()
            .position(|x| *x == f)
            .map(|l| (Axis::Col, l))
    }

    fn raw(&self, rp: &[u32], cp: &[u32], vi: usize) -> CellValue {
        self.cell_accs(rp, cp)
            .map_or(CellValue::Empty, |a| self.result(vi, a))
    }

    fn raw_num(&self, rp: &[u32], cp: &[u32], vi: usize) -> Option<f64> {
        self.number(vi, self.cell_accs(rp, cp))
    }

    /// The value shown in a data cell ("show values as" applied).
    pub(crate) fn shown(&self, rp: &[u32], cp: &[u32], vi: usize) -> CellValue {
        use PivotShowAs as S;
        let d = &self.spec.values[vi];
        let raw = self.raw(rp, cp, vi);
        let CellValue::Number(v) = raw else {
            return raw;
        };
        let ratio = |den: Option<f64>| match den {
            Some(x) if x != 0.0 => CellValue::Number(v / x),
            _ => CellValue::Error(CellError::Div0),
        };
        // Keys along the base field's axis.
        let with = |axis: Axis, key: &[u32]| match axis {
            Axis::Row => self.raw_num(key, cp, vi),
            Axis::Col => self.raw_num(rp, key, vi),
        };
        let own = |axis: Axis| match axis {
            Axis::Row => rp,
            Axis::Col => cp,
        };
        match d.show_as {
            S::Normal => raw,
            S::PercentOfTotal => ratio(self.raw_num(&[], &[], vi)),
            S::PercentOfRow => ratio(self.raw_num(rp, &[], vi)),
            S::PercentOfCol => ratio(self.raw_num(&[], cp, vi)),
            S::Index => {
                let gt = self.raw_num(&[], &[], vi);
                let rt = self.raw_num(rp, &[], vi);
                let ct = self.raw_num(&[], cp, vi);
                match (gt, rt, ct) {
                    (Some(g), Some(r), Some(c)) if r * c != 0.0 => {
                        CellValue::Number(v * g / (r * c))
                    }
                    _ => CellValue::Error(CellError::Div0),
                }
            }
            S::PercentOfParentRow | S::PercentOfParentCol => {
                let axis = if d.show_as == S::PercentOfParentRow {
                    Axis::Row
                } else {
                    Axis::Col
                };
                let key = own(axis);
                if key.is_empty() {
                    return ratio(Some(v));
                }
                ratio(with(axis, &key[..key.len() - 1]))
            }
            S::PercentOfParent => {
                let Some((axis, level)) = self.axis_of(d.base_field) else {
                    return CellValue::Error(CellError::NA);
                };
                let key = own(axis);
                if key.len() <= level {
                    return CellValue::Empty;
                }
                ratio(with(axis, &key[..=level]))
            }
            S::Difference | S::Percent | S::PercentDiff => {
                let Some((axis, level)) = self.axis_of(d.base_field) else {
                    return CellValue::Error(CellError::NA);
                };
                let key = own(axis);
                if key.len() <= level {
                    return CellValue::Empty;
                }
                let sib = self.siblings(axis, &key[..level]);
                let base = match d.base_item {
                    PIVOT_BASE_PREVIOUS | PIVOT_BASE_NEXT => {
                        let pos = sib.iter().position(|x| *x == key[level]);
                        let next = d.base_item == PIVOT_BASE_NEXT;
                        let Some(b) = pos.and_then(|p| {
                            if next {
                                sib.get(p + 1).copied()
                            } else {
                                p.checked_sub(1).and_then(|q| sib.get(q).copied())
                            }
                        }) else {
                            return CellValue::Empty;
                        };
                        b
                    }
                    b => b,
                };
                if base == key[level] {
                    return if d.show_as == S::Percent {
                        CellValue::Number(1.0)
                    } else {
                        CellValue::Empty
                    };
                }
                let mut bk = key.to_vec();
                bk[level] = base;
                let Some(bv) = with(axis, &bk) else {
                    return CellValue::Error(CellError::NA);
                };
                match d.show_as {
                    S::Difference => CellValue::Number(v - bv),
                    S::Percent => ratio(Some(bv)),
                    _ if bv == 0.0 => CellValue::Error(CellError::Div0),
                    _ => CellValue::Number((v - bv) / bv),
                }
            }
            S::RunTotal | S::PercentOfRunningTotal | S::RankAscending | S::RankDescending => {
                let Some((axis, level)) = self.axis_of(d.base_field) else {
                    return CellValue::Error(CellError::NA);
                };
                let key = own(axis);
                if key.len() <= level {
                    return CellValue::Empty;
                }
                let sib = self.siblings(axis, &key[..level]);
                let at = |x: u32| {
                    let mut k = key.to_vec();
                    k[level] = x;
                    with(axis, &k)
                };
                match d.show_as {
                    S::RankAscending | S::RankDescending => {
                        let asc = d.show_as == S::RankAscending;
                        let better = sib
                            .iter()
                            .filter_map(|x| at(*x))
                            .filter(|o| if asc { *o < v } else { *o > v })
                            .count();
                        CellValue::Number((better + 1) as f64)
                    }
                    _ => {
                        let mut run = 0.0;
                        for x in sib {
                            run += at(*x).unwrap_or(0.0);
                            if *x == key[level] {
                                break;
                            }
                        }
                        if d.show_as == S::RunTotal {
                            CellValue::Number(run)
                        } else {
                            let total: f64 = sib.iter().filter_map(|x| at(*x)).sum();
                            if total == 0.0 {
                                CellValue::Error(CellError::Div0)
                            } else {
                                CellValue::Number(run / total)
                            }
                        }
                    }
                }
            }
        }
    }

    /// Lines of the row axis in display order.
    fn row_lines(&self) -> Vec<Line> {
        let mut out = Vec::new();
        let mut prefix = Vec::new();
        self.emit_rows(&mut prefix, &mut out);
        out
    }

    fn emit_rows(&self, prefix: &mut Vec<u32>, out: &mut Vec<Line>) {
        let level = prefix.len();
        let m = self.spec.rows.len();
        let f = self.spec.rows[level];
        let opts = self.spec.field(f);
        let items = self.siblings(Axis::Row, prefix).to_vec();
        for x in items {
            prefix.push(x);
            if level + 1 == m {
                out.push(Line {
                    kind: RowKind::Data,
                    key: prefix.clone(),
                    values: true,
                });
            } else {
                let top = opts.subtotal && opts.subtotal_top;
                match self.spec.layout {
                    PivotLayout::Tabular => {
                        self.emit_rows(prefix, out);
                        if opts.subtotal {
                            out.push(Line {
                                kind: RowKind::Subtotal,
                                key: prefix.clone(),
                                values: true,
                            });
                        }
                    }
                    PivotLayout::Outline | PivotLayout::Compact => {
                        out.push(Line {
                            kind: RowKind::Group,
                            key: prefix.clone(),
                            values: top,
                        });
                        self.emit_rows(prefix, out);
                        if opts.subtotal && !top {
                            out.push(Line {
                                kind: RowKind::Subtotal,
                                key: prefix.clone(),
                                values: true,
                            });
                        }
                    }
                }
            }
            if opts.blank_row {
                out.push(Line {
                    kind: RowKind::Blank,
                    key: prefix.clone(),
                    values: false,
                });
            }
            prefix.pop();
        }
    }

    /// Column entries in display order: `(kind, key)`.
    fn col_entries(&self) -> Vec<(ColKind, Vec<u32>)> {
        let mut out = Vec::new();
        let mut prefix = Vec::new();
        self.emit_cols(&mut prefix, &mut out);
        out
    }

    fn emit_cols(&self, prefix: &mut Vec<u32>, out: &mut Vec<(ColKind, Vec<u32>)>) {
        let level = prefix.len();
        let n = self.spec.cols.len();
        let f = self.spec.cols[level];
        let opts = self.spec.field(f);
        let items = self.siblings(Axis::Col, prefix).to_vec();
        for x in items {
            prefix.push(x);
            if level + 1 == n {
                out.push((ColKind::Data, prefix.clone()));
            } else {
                self.emit_cols(prefix, out);
                if opts.subtotal {
                    out.push((ColKind::Subtotal, prefix.clone()));
                }
            }
            prefix.pop();
        }
    }

    /// "<item> Total" caption.
    fn total_caption(&self, f: u32, x: u32) -> CellValue {
        let item = super::value_text(&self.label(f, x));
        CellValue::Text(self.labels.subtotal.replace("{item}", &item))
    }

    /// Compute the report.
    pub(crate) fn layout(mut self) -> Result<Layout, String> {
        let mut paths = self.paths();
        self.apply_field_filters(&mut paths);
        self.aggregate(&paths);
        self.row_children = self.children(Axis::Row);
        self.col_children = self.children(Axis::Col);
        let spec = self.spec;
        let labels = self.labels;
        let k = spec.values.len();
        let m = spec.rows.len() as u32;
        let n = spec.cols.len() as u32;
        let compact = spec.layout == PivotLayout::Compact;
        // `dataOnRows` (§18.10.1.73): the values pseudo-field is the innermost
        // row field — every line with values is repeated once per value field.
        let data_rows = spec.data_on_rows && k > 1;
        let names: Vec<String> = spec
            .values
            .iter()
            .map(|d| value_name(self.cache, d))
            .collect();
        let caption = if k == 1 {
            names[0].clone()
        } else {
            labels.values.clone()
        };
        // Label columns (one more for the value names, except in compact form).
        let lc = if m == 0 {
            u32::from(n > 0 || data_rows)
        } else if compact {
            1
        } else {
            m + u32::from(data_rows)
        };
        // Column entries (× value fields unless they are on the rows).
        let mut entries: Vec<(ColKind, Vec<u32>)> = if n == 0 {
            vec![(ColKind::Data, Vec::new())]
        } else {
            self.col_entries()
        };
        if n > 0 && spec.row_grand_totals {
            entries.push((ColKind::Grand, Vec::new()));
        }
        let mut columns: Vec<(usize, usize)> = Vec::new();
        if k > 0 {
            for (e, _) in entries.iter().enumerate() {
                if data_rows {
                    columns.push((e, 0));
                } else {
                    columns.extend((0..k).map(|v| (e, v)));
                }
            }
        }
        let hr = if n == 0 {
            1
        } else {
            1 + n + u32::from(k > 1 && !data_rows)
        };
        let mut lines: Vec<Line> = if m == 0 {
            vec![Line {
                kind: RowKind::Data,
                key: Vec::new(),
                values: true,
            }]
        } else {
            self.row_lines()
        };
        if m > 0 && spec.col_grand_totals {
            lines.push(Line {
                kind: RowKind::Grand,
                key: Vec::new(),
                values: true,
            });
        }
        // Report lines with the value field each one shows (data on rows):
        // `(line, value index, first line of the item)`.
        let mut rows: Vec<(Line, Option<usize>, bool)> = Vec::with_capacity(lines.len());
        for line in lines {
            if !data_rows || !line.values {
                rows.push((line, None, true));
                continue;
            }
            let lead = compact && m > 0;
            if lead {
                // Compact form: the item line, then the value names below it.
                rows.push((
                    Line {
                        values: false,
                        ..line.clone()
                    },
                    None,
                    true,
                ));
            }
            for vi in 0..k {
                rows.push((line.clone(), Some(vi), vi == 0 && !lead));
            }
        }
        let height = hr + rows.len() as u32;
        let width = (lc + columns.len() as u32).max(1);
        if u64::from(height) * u64::from(width) > MAX_REPORT_CELLS {
            return Err("pivot table too large".into());
        }
        let p = spec.filters.len() as u32;
        let page_rows = if p > 0 { p + 1 } else { 0 };
        let mut out = Layout {
            page_rows,
            height,
            width,
            first_header_row: u32::from(n > 0),
            first_data_row: hr,
            first_data_col: lc,
            ..Layout::default()
        };
        let put = |out: &mut Layout, r: u32, c: u32, v: CellValue| {
            if !matches!(v, CellValue::Empty) {
                out.cells.push((page_rows + r, c, v));
            }
        };
        // Report filters (page fields).
        for (i, f) in spec.filters.iter().enumerate() {
            let i = i as u32;
            out.cells
                .push((i, 0, CellValue::Text(self.field_caption(f.field))));
            let sel = match f.selected.as_slice() {
                [] => CellValue::Text(labels.all.clone()),
                [CellValue::Empty] => CellValue::Text(labels.blank.clone()),
                [one] => one.clone(),
                _ => CellValue::Text(labels.multiple.clone()),
            };
            out.cells.push((i, 1, sel));
        }
        let field_name = |f: u32| CellValue::Text(self.field_caption(f));
        // Header rows.
        if n > 0 {
            put(&mut out, 0, 0, CellValue::Text(caption.clone()));
            let head = if compact {
                CellValue::Text(labels.col_labels.clone())
            } else {
                field_name(spec.cols[0])
            };
            put(&mut out, 0, lc, head);
            let mut prev: Option<&Vec<u32>> = None;
            for (x, (e, vi)) in columns.iter().enumerate() {
                let col = lc + x as u32;
                if *vi != 0 {
                    continue;
                }
                let (kind, key) = &entries[*e];
                match kind {
                    ColKind::Grand => {
                        put(
                            &mut out,
                            1,
                            col,
                            CellValue::Text(labels.grand_total.clone()),
                        );
                    }
                    ColKind::Subtotal => {
                        let l = key.len() - 1;
                        let v = self.total_caption(spec.cols[l], key[l]);
                        put(&mut out, 1 + l as u32, col, v);
                    }
                    ColKind::Data => {
                        for (j, f) in spec.cols.iter().enumerate() {
                            let first = prev.is_none_or(|p| p[..=j] != key[..=j]);
                            if first {
                                put(&mut out, 1 + j as u32, col, self.label(*f, key[j]));
                                out.label_fields.push((1 + j as u32, col, *f));
                            }
                        }
                        prev = Some(key);
                    }
                }
            }
            if k > 1 && !data_rows {
                for (x, (_, vi)) in columns.iter().enumerate() {
                    put(
                        &mut out,
                        n + 1,
                        lc + x as u32,
                        CellValue::Text(names[*vi].clone()),
                    );
                }
            }
        } else if !data_rows {
            for (x, (_, vi)) in columns.iter().enumerate() {
                put(
                    &mut out,
                    0,
                    lc + x as u32,
                    CellValue::Text(names[*vi].clone()),
                );
            }
        }
        if m > 0 {
            if compact {
                put(
                    &mut out,
                    hr - 1,
                    0,
                    CellValue::Text(labels.row_labels.clone()),
                );
            } else {
                for (l, f) in spec.rows.iter().enumerate() {
                    put(&mut out, hr - 1, l as u32, field_name(*f));
                }
            }
        }
        if data_rows && !compact {
            put(
                &mut out,
                hr - 1,
                lc - 1,
                CellValue::Text(labels.values.clone()),
            );
        }
        out.bold_rows.extend(0..hr);
        out.row_kinds.extend((0..hr).map(|_| RowKind::Header));
        // Body.
        let mut prev_data: Option<Vec<u32>> = None;
        for (i, (line, line_vi, first)) in rows.iter().enumerate() {
            let row = hr + i as u32;
            out.row_kinds.push(line.kind);
            let level = line.key.len().saturating_sub(1);
            let label_col = |l: usize| if compact { 0 } else { l as u32 };
            let label = |out: &mut Layout, l: usize, v: CellValue, f: Option<u32>| {
                let col = label_col(l);
                put(out, row, col, v);
                if compact && l > 0 {
                    out.indents.push((row, col, l as u32));
                }
                if let Some(f) = f {
                    out.label_fields.push((row, col, f));
                }
            };
            if let Some(vi) = line_vi {
                // The value field's name in its label column.
                let (col, indent) = if compact {
                    (0, if m == 0 { 0 } else { line.key.len() as u32 + 1 })
                } else {
                    (lc - 1, 0)
                };
                put(&mut out, row, col, CellValue::Text(names[*vi].clone()));
                if indent > 0 && line.kind != RowKind::Grand {
                    out.indents.push((row, col, indent));
                } else if compact && line.kind == RowKind::Grand && m > 0 {
                    out.indents.push((row, col, 1));
                }
            }
            if *first {
                match line.kind {
                    RowKind::Grand => {
                        label(
                            &mut out,
                            0,
                            CellValue::Text(labels.grand_total.clone()),
                            None,
                        );
                        out.bold_rows.push(row);
                    }
                    RowKind::Blank | RowKind::Header => {}
                    RowKind::Data if m == 0 => {
                        if n > 0 && !data_rows {
                            put(&mut out, row, 0, CellValue::Text(caption.clone()));
                        }
                    }
                    RowKind::Data | RowKind::Group | RowKind::Subtotal => {
                        let own_from =
                            if spec.layout == PivotLayout::Tabular && line.kind == RowKind::Data {
                                0
                            } else {
                                level
                            };
                        // Outer labels: the first line of a group, or every line when repeated.
                        if !compact {
                            for l in 0..level {
                                let f = spec.rows[l];
                                let first = line.kind == RowKind::Data
                                    && own_from == 0
                                    && prev_data.as_ref().is_none_or(|p| p[..=l] != line.key[..=l]);
                                if first || spec.field(f).repeat_labels {
                                    label(&mut out, l, self.label(f, line.key[l]), Some(f));
                                }
                            }
                        }
                        let f = spec.rows[level];
                        if line.kind == RowKind::Subtotal {
                            label(
                                &mut out,
                                level,
                                self.total_caption(f, line.key[level]),
                                None,
                            );
                            out.bold_rows.push(row);
                        } else {
                            label(&mut out, level, self.label(f, line.key[level]), Some(f));
                            if line.kind == RowKind::Group {
                                out.bold_rows.push(row);
                            }
                        }
                        if line.kind == RowKind::Data {
                            prev_data = Some(line.key.clone());
                        }
                    }
                }
            } else if matches!(line.kind, RowKind::Subtotal | RowKind::Grand) {
                out.bold_rows.push(row);
            }
            if !line.values {
                continue;
            }
            if let Some(vi) = line_vi {
                out.data_rows.push((row, *vi));
            }
            for (x, (e, vi)) in columns.iter().enumerate() {
                let ck = &entries[*e].1;
                let v = self.shown(&line.key, ck, line_vi.unwrap_or(*vi));
                put(&mut out, row, lc + x as u32, v);
            }
        }
        out.data_cols = columns
            .iter()
            .enumerate()
            .map(|(x, (_, vi))| (lc + x as u32, *vi))
            .collect();
        out.col_kinds = columns.iter().map(|(e, _)| entries[*e].0).collect();
        // rowItems (§18.10.1.84): one `i` per body line; with the values on
        // the rows the data field index ends each line's path (`i`).
        out.row_items = rows
            .iter()
            .map(|(l, vi, _)| {
                let mut items = if l.kind == RowKind::Grand {
                    vec![0]
                } else {
                    l.key.clone()
                };
                if let Some(vi) = vi {
                    if l.kind == RowKind::Grand {
                        items = vec![*vi as u32];
                    } else {
                        items.push(*vi as u32);
                    }
                }
                PivotAxisItem {
                    kind: match l.kind {
                        RowKind::Subtotal => "default".into(),
                        RowKind::Blank => "blank".into(),
                        RowKind::Grand => "grand".into(),
                        _ => String::new(),
                    },
                    repeat: 0,
                    data_index: vi.map_or(0, |v| v as u32),
                    items,
                }
            })
            .collect();
        // colItems (§18.10.1.10): one `i` per data column; the values
        // pseudo-field adds the data field index when there are several.
        out.col_items = columns
            .iter()
            .map(|(e, vi)| {
                let (kind, key) = &entries[*e];
                let mut items = if *kind == ColKind::Grand {
                    vec![0]
                } else {
                    key.clone()
                };
                if k > 1 && !data_rows {
                    if *kind == ColKind::Grand {
                        items = vec![*vi as u32];
                    } else {
                        items.push(*vi as u32);
                    }
                }
                PivotAxisItem {
                    kind: match kind {
                        ColKind::Data => String::new(),
                        ColKind::Subtotal => "default".into(),
                        ColKind::Grand => "grand".into(),
                    },
                    repeat: 0,
                    data_index: if data_rows { 0 } else { *vi as u32 },
                    items,
                }
            })
            .collect();
        Ok(out)
    }
}
