//! In-memory workbook with a dependency graph and incremental recalculation.
//!
//! * Each formula cell records the areas it reads (static precedents, with
//!   defined names and table references resolved). Small areas are indexed per
//!   cell; large areas are kept in a range list.
//! * Editing a cell marks it changed; [`Workbook::recalculate`] collects the
//!   transitive dependents (dirty propagation) plus volatile cells, orders them
//!   by strongly connected components (Tarjan) so precedents are computed first,
//!   and evaluates them. Dynamic references (`INDIRECT`, `OFFSET`) that read a
//!   not-yet-computed cell defer the reader until that cell is computed.
//! * A component with more than one cell (or a self reference) is a circular
//!   reference: it evaluates to `#CALC!` and is reported by
//!   [`Workbook::circular_references`], unless iterative calculation is enabled
//!   (ECMA-376 Part 1, §18.2.2 calcPr `iterate`, `iterateCount`, `iterateDelta`).
//! * Array results spill into neighbouring cells; a non-empty cell in the way
//!   yields `#SPILL!`.

use std::cell::RefCell;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};

use crate::ast::Expr;
use crate::date::DateSystem;
use crate::eval::{static_areas, struct_area, Ctx, EvalConfig, TextFormatter};
use crate::format::format;
use crate::functions::is_volatile_function;
use crate::fxhash::{FxHashMap, FxHashSet};
use crate::lookupindex::ExactIndex;
use crate::parser::{parse_with, ParseError, ParseOptions};
use crate::provider::{CellProvider, PivotInfo, TableInfo, WebProvider};
use crate::rangeindex::RangeIndex;
use crate::reference::{parse_a1_cell, MAX_COLS, MAX_ROWS};
use crate::shift::{
    adjust_for_change, adjust_span, is_row_change, offset_relative, StructuralChange,
};
use crate::value::{parse_number_text, Area, Array, ErrorKind, Value};

/// Address of a cell in a workbook.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CellId {
    /// Sheet index.
    pub sheet: usize,
    /// Row (0-based).
    pub row: u32,
    /// Column (0-based).
    pub col: u32,
}

impl CellId {
    /// Construct a cell id.
    pub fn new(sheet: usize, row: u32, col: u32) -> Self {
        CellId { sheet, row, col }
    }
}

/// Iterative calculation settings (ECMA-376 Part 1, §18.2.2 calcPr).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IterativeConfig {
    /// Maximum number of iterations (`iterateCount`, default 100).
    pub max_iterations: u32,
    /// Stop when no value changes by more than this (`iterateDelta`, default 0.001).
    pub max_change: f64,
}

impl Default for IterativeConfig {
    fn default() -> Self {
        IterativeConfig {
            max_iterations: 100,
            max_change: 0.001,
        }
    }
}

#[derive(Debug)]
struct Formula {
    expr: Expr,
    /// Canonical text, formatted on first use (opening a file sets 100 000s of
    /// formulas whose text is rarely asked for).
    text: std::sync::OnceLock<String>,
    /// `(rows, columns)` of a legacy array formula: its result fills exactly that many
    /// cells (ECMA-376 Part 1, §18.17.2.4 array formulas; §18.3.1.40 `f` with
    /// `t="array"` and `ref`) instead of spilling.
    fixed: Option<(u32, u32)>,
}

impl Formula {
    fn new(expr: Expr, text: Option<String>) -> Formula {
        let cell = std::sync::OnceLock::new();
        if let Some(t) = text {
            let _ = cell.set(t);
        }
        Formula {
            expr,
            text: cell,
            fixed: None,
        }
    }

    fn with_fixed(mut self, fixed: Option<(u32, u32)>) -> Formula {
        self.fixed = fixed.filter(|&(r, c)| r > 0 && c > 0);
        self
    }

    fn text(&self) -> &str {
        self.text.get_or_init(|| format!("={}", format(&self.expr)))
    }
}

#[derive(Debug, Default)]
struct CellData {
    formula: Option<Arc<Formula>>,
    value: Value,
}

#[derive(Debug, Default)]
struct Sheet {
    name: String,
    cells: FxHashMap<(u32, u32), CellData>,
    rows: u32,
    cols: u32,
    hidden_rows: FxHashSet<u32>,
    /// Row spans (first, last) of the active filters: a hidden row inside one is filtered out.
    filter_spans: Vec<(u32, u32)>,
}

/// Formula cells reading one cell directly. Most cells have a single reader: it is
/// kept inline (opening a sheet registers a million of these).
#[derive(Debug)]
enum Readers {
    One(CellId),
    Many(Vec<CellId>),
}

impl Readers {
    fn push(&mut self, id: CellId) {
        match self {
            Readers::One(a) => *self = Readers::Many(vec![*a, id]),
            Readers::Many(v) => v.push(id),
        }
    }

    /// Remove every `id`; returns whether no reader is left.
    fn remove(&mut self, id: CellId) -> bool {
        match self {
            Readers::One(a) => *a == id,
            Readers::Many(v) => {
                v.retain(|d| *d != id);
                v.is_empty()
            }
        }
    }

    fn as_slice(&self) -> &[CellId] {
        match self {
            Readers::One(a) => std::slice::from_ref(a),
            Readers::Many(v) => v,
        }
    }
}

/// Areas up to this size are indexed cell by cell in the dependency graph.
const SMALL_AREA: u64 = 64;
/// Maximum number of extra passes to settle spill ranges.
const SPILL_PASSES: usize = 8;

/// An in-memory workbook implementing [`CellProvider`].
pub struct Workbook {
    sheets: Vec<Sheet>,
    names: FxHashMap<(Option<usize>, String), Expr>,
    tables: Vec<TableInfo>,
    /// Pivot table reports `GETPIVOTDATA` reads.
    pivots: Vec<Arc<PivotInfo>>,
    config: EvalConfig,
    iterative: Option<IterativeConfig>,
    rng: AtomicU64,
    precedents: FxHashMap<CellId, Vec<Area>>,
    dependents: FxHashMap<CellId, Readers>,
    range_deps: RangeIndex,
    volatile: FxHashSet<CellId>,
    changed: Vec<CellId>,
    spills: FxHashMap<CellId, Area>,
    spill_owner: FxHashMap<CellId, CellId>,
    spill_blocked: FxHashSet<CellId>,
    spill_changed: Vec<CellId>,
    circular: FxHashSet<CellId>,
    evaluations: AtomicUsize,
    /// Cells (re)calculated by the last [`Workbook::recalculate`].
    recalculated: Vec<CellId>,
    /// Materialized large areas, valid during one recalculation pass.
    area_cache: RwLock<FxHashMap<Area, Arc<Array>>>,
    /// Exact-match indexes of lookup vectors, valid during one recalculation pass.
    index_cache: RwLock<FxHashMap<Area, Arc<ExactIndex>>>,
    /// Decimal places numbers of a cell are rounded to when read ("precision
    /// as displayed", §18.2.2 calcPr `fullPrecision`).
    round_to: FxHashMap<CellId, i32>,
    /// Formulas calling `WEBSERVICE` loaded with a cached value (§18.3.1.96 `v`): while no
    /// web provider is installed their evaluation keeps that value instead of `#VALUE!`.
    web_cached: FxHashSet<CellId>,
}

impl Default for Workbook {
    fn default() -> Self {
        Self::new()
    }
}

impl Workbook {
    /// An empty workbook without sheets.
    pub fn new() -> Self {
        Workbook {
            sheets: Vec::new(),
            names: FxHashMap::default(),
            tables: Vec::new(),
            pivots: Vec::new(),
            config: EvalConfig::default(),
            iterative: None,
            rng: AtomicU64::new(0x2545_F491_4F6C_DD1D),
            precedents: FxHashMap::default(),
            dependents: FxHashMap::default(),
            range_deps: RangeIndex::default(),
            volatile: FxHashSet::default(),
            changed: Vec::new(),
            spills: FxHashMap::default(),
            spill_owner: FxHashMap::default(),
            spill_blocked: FxHashSet::default(),
            spill_changed: Vec::new(),
            circular: FxHashSet::default(),
            evaluations: AtomicUsize::new(0),
            recalculated: Vec::new(),
            area_cache: RwLock::new(FxHashMap::default()),
            index_cache: RwLock::new(FxHashMap::default()),
            round_to: FxHashMap::default(),
            web_cached: FxHashSet::default(),
        }
    }

    /// A workbook with the given sheet names.
    pub fn with_sheets(names: &[&str]) -> Self {
        let mut wb = Self::new();
        for n in names {
            wb.add_sheet(n);
        }
        wb
    }

    /// Append a sheet and return its index.
    pub fn add_sheet(&mut self, name: &str) -> usize {
        self.sheets.push(Sheet {
            name: name.to_owned(),
            ..Sheet::default()
        });
        self.sheets.len() - 1
    }

    /// Make room for `additional` more cells on `sheet` (bulk loading).
    pub fn reserve_cells(&mut self, sheet: usize, additional: usize) {
        if let Some(s) = self.sheets.get_mut(sheet) {
            s.cells.reserve(additional);
        }
    }

    /// Evaluation settings (date system, fixed "now").
    pub fn config(&self) -> &EvalConfig {
        &self.config
    }

    /// Select the date system; marks everything for recalculation.
    pub fn set_date_system(&mut self, system: DateSystem) {
        self.config.date_system = system;
        self.mark_all_formulas();
    }

    /// Fix the value of `NOW()` (serial) or `None` for the system clock.
    pub fn set_now(&mut self, now: Option<f64>) {
        self.config.now = now;
    }

    /// Install the number-format renderer used by `TEXT` (`None` restores the
    /// built-in compact one); marks everything for recalculation.
    pub fn set_text_formatter(&mut self, formatter: Option<TextFormatter>) {
        self.config.text_formatter = formatter;
        self.mark_all_formulas();
    }

    /// Install (or remove with `None`) the fetcher used by `WEBSERVICE`. Web access is off
    /// by default; hosts install a provider only when an administrator enables it.
    /// `WEBSERVICE` is volatile, so the next [`Workbook::recalculate`] uses the new provider
    /// without recalculating anything else.
    pub fn set_web_provider(&mut self, provider: Option<Arc<dyn WebProvider>>) {
        self.config.web_provider = provider;
    }

    /// Seed the generator used by `RAND`/`RANDBETWEEN`/`RANDARRAY`.
    pub fn seed_random(&mut self, seed: u64) {
        self.rng.store(seed.max(1), Ordering::Relaxed);
    }

    /// Enable or disable iterative calculation of circular references.
    pub fn set_iterative(&mut self, cfg: Option<IterativeConfig>) {
        self.iterative = cfg;
        self.mark_all_formulas();
    }

    /// Hide or show a row (affects `SUBTOTAL` 101-111 / `AGGREGATE`).
    pub fn set_row_hidden(&mut self, sheet: usize, row: u32, hidden: bool) {
        if let Some(s) = self.sheets.get_mut(sheet) {
            if hidden {
                s.hidden_rows.insert(row);
            } else {
                s.hidden_rows.remove(&row);
            }
        }
        self.mark_all_formulas();
    }

    /// The row spans (first, last data row) of the active filters of `sheet`: hidden rows
    /// inside them are filtered out, which `SUBTOTAL` 1-11 skips too. Formulas are marked for
    /// recalculation when the spans change.
    pub fn set_filter_spans(&mut self, sheet: usize, spans: Vec<(u32, u32)>) {
        let Some(s) = self.sheets.get_mut(sheet) else {
            return;
        };
        if s.filter_spans == spans {
            return;
        }
        s.filter_spans = spans;
        self.mark_all_formulas();
    }

    /// Bulk loading: [`Workbook::set_filter_spans`] without marking the formulas.
    pub fn load_filter_spans(&mut self, sheet: usize, spans: Vec<(u32, u32)>) {
        if let Some(s) = self.sheets.get_mut(sheet) {
            s.filter_spans = spans;
        }
    }

    /// Replace the hidden rows of `sheet` at once (an AutoFilter applied to a
    /// million rows): every formula is marked for recalculation once, and only
    /// when the set changes ([`Workbook::set_row_hidden`] marks them per row).
    pub fn set_hidden_rows(&mut self, sheet: usize, rows: impl IntoIterator<Item = u32>) {
        let Some(s) = self.sheets.get_mut(sheet) else {
            return;
        };
        let rows: FxHashSet<u32> = rows.into_iter().collect();
        if s.hidden_rows == rows {
            return;
        }
        s.hidden_rows = rows;
        self.mark_all_formulas();
    }

    // ------------------------------------------------------------ editing

    fn touch(&mut self, id: CellId) {
        if let Some(s) = self.sheets.get_mut(id.sheet) {
            s.rows = s.rows.max(id.row + 1);
            s.cols = s.cols.max(id.col + 1);
        }
    }

    fn after_edit(&mut self, id: CellId) {
        self.changed.push(id);
        // Writing into a spill range blocks it; the anchor must re-evaluate.
        if let Some(owner) = self.spill_owner.get(&id).copied() {
            if owner != id {
                self.changed.push(owner);
            }
        }
        // Anchors blocked earlier may now be free.
        let blocked: Vec<CellId> = self.spill_blocked.iter().copied().collect();
        self.changed.extend(blocked);
    }

    /// Set a constant value (`Value::Empty` clears the cell).
    pub fn set_value(&mut self, sheet: usize, row: u32, col: u32, value: impl Into<Value>) {
        if sheet >= self.sheets.len() {
            return;
        }
        let id = CellId::new(sheet, row, col);
        self.remove_formula(id);
        let value = value.into();
        let cells = &mut self.sheets[sheet].cells;
        if matches!(value, Value::Empty) {
            cells.remove(&(row, col));
        } else {
            cells.insert(
                (row, col),
                CellData {
                    formula: None,
                    value,
                },
            );
            self.touch(id);
        }
        self.after_edit(id);
    }

    /// Set a formula (with or without leading `=`).
    pub fn set_formula(
        &mut self,
        sheet: usize,
        row: u32,
        col: u32,
        formula: &str,
    ) -> Result<(), ParseError> {
        let opts = ParseOptions {
            origin_row: row,
            origin_col: col,
            ..ParseOptions::default()
        };
        let expr = parse_with(formula, &opts)?;
        self.set_formula_expr(sheet, row, col, expr, None, None);
        Ok(())
    }

    /// Set a legacy array formula (entered with Ctrl+Shift+Enter, ECMA-376 Part 1,
    /// §18.17.2.4) anchored at `row`, `col` over `rows` × `cols` cells. Its result fills
    /// exactly that range: a single value repeats in every cell, a one-row (one-column)
    /// result repeats down (across), and cells beyond a larger result show `#N/A`; a
    /// bigger result is cut to the range. It never spills or reports `#SPILL!`. The other
    /// cells of the range must hold nothing.
    ///
    /// # Errors
    /// The formula does not parse.
    pub fn set_array_formula(
        &mut self,
        sheet: usize,
        row: u32,
        col: u32,
        formula: &str,
        rows: u32,
        cols: u32,
    ) -> Result<(), ParseError> {
        let opts = ParseOptions {
            origin_row: row,
            origin_col: col,
            ..ParseOptions::default()
        };
        let expr = parse_with(formula, &opts)?;
        self.set_formula_expr(sheet, row, col, expr, None, Some((rows, cols)));
        Ok(())
    }

    fn set_formula_expr(
        &mut self,
        sheet: usize,
        row: u32,
        col: u32,
        expr: Expr,
        text: Option<String>,
        fixed: Option<(u32, u32)>,
    ) {
        if sheet >= self.sheets.len() {
            return;
        }
        let id = CellId::new(sheet, row, col);
        self.remove_formula(id);
        let (areas, volatile) = self.collect_deps(&expr, id);
        self.register(id, areas);
        if volatile {
            self.volatile.insert(id);
        }
        let old = self.sheets[sheet]
            .cells
            .remove(&(row, col))
            .map(|c| c.value)
            .unwrap_or_default();
        self.sheets[sheet].cells.insert(
            (row, col),
            CellData {
                formula: Some(Arc::new(Formula::new(expr, text).with_fixed(fixed))),
                value: old,
            },
        );
        self.touch(id);
        self.after_edit(id);
    }

    /// Bulk loading (opening a file): a formula whose result may already be known.
    ///
    /// With `cached: Some(v)` the cell takes `v` (the value saved with the file,
    /// ECMA-376 Part 1, §18.3.1.96 `v`) and is *not* marked for calculation, so
    /// [`Workbook::recalculate`] only evaluates formulas without a cached value,
    /// volatile formulas and their dependents. `cached: None` behaves like
    /// [`Workbook::set_formula`]. The formula text is stored as given (no
    /// re-formatting of the parsed expression).
    pub fn load_formula(
        &mut self,
        sheet: usize,
        row: u32,
        col: u32,
        formula: &str,
        cached: Option<Value>,
    ) -> Result<(), ParseError> {
        if sheet >= self.sheets.len() {
            return Ok(());
        }
        let opts = ParseOptions {
            origin_row: row,
            origin_col: col,
            ..ParseOptions::default()
        };
        let expr = parse_with(formula, &opts)?;
        let id = CellId::new(sheet, row, col);
        if self.sheets[sheet].cells.contains_key(&(row, col)) {
            self.remove_formula(id);
        }
        let (areas, volatile) = self.collect_deps(&expr, id);
        self.register(id, areas);
        if volatile {
            self.volatile.insert(id);
        }
        let known = cached.is_some();
        if known && volatile && formula.to_ascii_uppercase().contains("WEBSERVICE") {
            self.web_cached.insert(id);
        }
        self.sheets[sheet].cells.insert(
            (row, col),
            CellData {
                // Text formatted on first use, like `set_formula`.
                formula: Some(Arc::new(Formula::new(expr, None))),
                value: cached.unwrap_or_default(),
            },
        );
        self.touch(id);
        if !known {
            self.after_edit(id);
        }
        Ok(())
    }

    /// Bulk loading: a constant cell. Unlike [`Workbook::set_value`] it does not
    /// mark the cell as edited (formulas reading it carry their own cached value
    /// or are marked by [`Workbook::load_formula`]).
    pub fn load_value(&mut self, sheet: usize, row: u32, col: u32, value: impl Into<Value>) {
        let value = value.into();
        let Some(s) = self.sheets.get_mut(sheet) else {
            return;
        };
        if matches!(value, Value::Empty) || s.cells.contains_key(&(row, col)) {
            self.set_value(sheet, row, col, value);
            return;
        }
        s.cells.insert(
            (row, col),
            CellData {
                formula: None,
                value,
            },
        );
        self.touch(CellId::new(sheet, row, col));
    }

    /// Bulk loading: a hidden row, without marking every formula for
    /// calculation (cached `SUBTOTAL` results already account for it).
    pub fn load_row_hidden(&mut self, sheet: usize, row: u32) {
        if let Some(s) = self.sheets.get_mut(sheet) {
            s.hidden_rows.insert(row);
        }
    }

    /// Set a cell from user input: `=...` is a formula, numbers/booleans/error
    /// literals are parsed, empty clears, anything else is text.
    pub fn set_input(
        &mut self,
        sheet: usize,
        row: u32,
        col: u32,
        input: &str,
    ) -> Result<(), ParseError> {
        if input.starts_with('=') && input.len() > 1 {
            return self.set_formula(sheet, row, col, input);
        }
        let v = if input.is_empty() {
            Value::Empty
        } else if let Some(n) = parse_number_text(input) {
            Value::Number(n)
        } else if input.eq_ignore_ascii_case("TRUE") {
            Value::Bool(true)
        } else if input.eq_ignore_ascii_case("FALSE") {
            Value::Bool(false)
        } else if let Some(e) = ErrorKind::parse(input) {
            Value::Error(e)
        } else {
            Value::Text(input.to_owned())
        };
        self.set_value(sheet, row, col, v);
        Ok(())
    }

    /// [`Workbook::set_input`] addressed by an A1 string such as `"B3"`.
    pub fn set_a1(&mut self, sheet: usize, a1: &str, input: &str) -> Result<(), ParseError> {
        let addr = parse_a1_cell(a1).ok_or_else(|| ParseError {
            pos: 0,
            message: format!("invalid cell address {a1}"),
        })?;
        self.set_input(sheet, addr.row, addr.col, input)
    }

    /// Permute rows `start..start + perm.len()` inside columns `c0..=c1` of `sheet`
    /// (a sort): row `start + n` receives the constants of row `start + perm[n]`.
    /// Formula cells of the moved rows are removed — the caller sets them again at
    /// their new place, their references translated. Every moved position is marked
    /// as edited, so the next [`Workbook::recalculate`] updates the formulas reading
    /// them. Returns `false` (nothing changed) when a moved cell takes part in a
    /// spill range or is rounded to its displayed precision.
    pub fn permute_rows(
        &mut self,
        sheet: usize,
        start: u32,
        c0: u32,
        c1: u32,
        perm: &[u32],
    ) -> bool {
        if sheet >= self.sheets.len() || c1 < c0 {
            return false;
        }
        let moved: Vec<u32> = (0..perm.len() as u32)
            .filter(|&n| perm[n as usize] != n)
            .collect();
        if moved.is_empty() {
            return true;
        }
        let special =
            !self.spill_owner.is_empty() || !self.spills.is_empty() || !self.round_to.is_empty();
        if special {
            for &n in &moved {
                for c in c0..=c1 {
                    let id = CellId::new(sheet, start + n, c);
                    if self.spill_owner.contains_key(&id)
                        || self.spills.contains_key(&id)
                        || self.round_to.contains_key(&id)
                    {
                        return false;
                    }
                }
            }
        }
        let width = (c1 - c0 + 1) as usize;
        let mut slot_of = vec![u32::MAX; perm.len()];
        for (k, &n) in moved.iter().enumerate() {
            slot_of[n as usize] = k as u32;
        }
        if moved.len() * width * 4 >= self.sheets[sheet].cells.len()
            && self.permute_constants(sheet, start, c0, c1, perm, &moved, &slot_of)
        {
            self.mark_moved(sheet, c0, c1, start, &moved);
            return true;
        }
        let mut taken: Vec<Option<Value>> = Vec::with_capacity(moved.len() * width);
        let mut formulas = Vec::new();
        {
            let cells = &mut self.sheets[sheet].cells;
            for &n in &moved {
                for c in c0..=c1 {
                    match cells.remove(&(start + n, c)) {
                        Some(d) if d.formula.is_some() => {
                            formulas.push(CellId::new(sheet, start + n, c));
                            taken.push(None);
                        }
                        Some(d) => taken.push(Some(d.value)),
                        None => taken.push(None),
                    }
                }
            }
        }
        for id in formulas {
            self.remove_formula(id);
        }
        let cells = &mut self.sheets[sheet].cells;
        for &n in &moved {
            let k = slot_of[perm[n as usize] as usize] as usize;
            for (j, c) in (c0..=c1).enumerate() {
                if let Some(value) = taken.get_mut(k * width + j).and_then(Option::take) {
                    cells.insert(
                        (start + n, c),
                        CellData {
                            formula: None,
                            value,
                        },
                    );
                }
            }
        }
        self.mark_moved(sheet, c0, c1, start, &moved);
        true
    }

    /// Marks the cells of the moved rows as edited.
    fn mark_moved(&mut self, sheet: usize, c0: u32, c1: u32, start: u32, moved: &[u32]) {
        self.changed.reserve(moved.len() * (c1 - c0 + 1) as usize);
        for &n in moved {
            for c in c0..=c1 {
                self.changed.push(CellId::new(sheet, start + n, c));
            }
        }
        // Anchors blocked earlier may now be free.
        self.changed.extend(self.spill_blocked.iter().copied());
    }

    /// [`Workbook::permute_rows`] of a block without formulas: the values move
    /// through the map's own entries (one pass over the sheet, no removal and
    /// re-insertion of every cell). Returns `false`, changing nothing, when a cell of
    /// the moved rows holds a formula.
    #[allow(clippy::too_many_arguments)]
    fn permute_constants(
        &mut self,
        sheet: usize,
        start: u32,
        c0: u32,
        c1: u32,
        perm: &[u32],
        moved: &[u32],
        slot_of: &[u32],
    ) -> bool {
        let width = (c1 - c0 + 1) as usize;
        let cells = &mut self.sheets[sheet].cells;
        let mut slots: Vec<Option<&mut CellData>> = Vec::new();
        slots.resize_with(moved.len() * width, || None);
        for (&(r, c), d) in cells.iter_mut() {
            if r < start || c < c0 || c > c1 {
                continue;
            }
            let Some(&k) = slot_of.get((r - start) as usize) else {
                continue;
            };
            if k == u32::MAX {
                continue;
            }
            if d.formula.is_some() {
                return false;
            }
            slots[k as usize * width + (c - c0) as usize] = Some(d);
        }
        // Moved line `k` takes the values of line `src[k]`; the lines swap along the
        // permutation's cycles (one line held aside per cycle).
        let src: Vec<usize> = moved
            .iter()
            .map(|&n| slot_of[perm[n as usize] as usize] as usize)
            .collect();
        let mut removed = Vec::new();
        let mut added = Vec::new();
        let mut done = vec![false; moved.len()];
        let mut tmp: Vec<Option<Value>> = Vec::with_capacity(width);
        let take = |slots: &mut [Option<&mut CellData>], i: usize| {
            slots[i].as_mut().map(|d| std::mem::take(&mut d.value))
        };
        for first in 0..moved.len() {
            if done[first] {
                continue;
            }
            tmp.clear();
            tmp.extend((0..width).map(|j| take(&mut slots, first * width + j)));
            let mut to = first;
            loop {
                done[to] = true;
                let from = src[to];
                for j in 0..width {
                    let v = if from == first {
                        tmp[j].take()
                    } else {
                        take(&mut slots, from * width + j)
                    };
                    let key = (start + moved[to], c0 + j as u32);
                    match (&mut slots[to * width + j], v) {
                        (Some(d), Some(v)) => d.value = v,
                        (Some(_), None) => removed.push(key),
                        (None, Some(v)) => added.push((key, v)),
                        (None, None) => {}
                    }
                }
                if from == first {
                    break;
                }
                to = from;
            }
        }
        drop(slots);
        for k in removed {
            cells.remove(&k);
        }
        for (k, value) in added {
            cells.insert(
                k,
                CellData {
                    formula: None,
                    value,
                },
            );
        }
        true
    }

    /// Clear a cell.
    pub fn clear(&mut self, sheet: usize, row: u32, col: u32) {
        self.set_value(sheet, row, col, Value::Empty);
    }

    /// Calculated value of a cell (spilled values included).
    pub fn value(&self, sheet: usize, row: u32, col: u32) -> Value {
        self.read(CellId::new(sheet, row, col))
    }

    /// [`Workbook::value`] addressed by an A1 string; `#REF!` if invalid.
    pub fn value_a1(&self, sheet: usize, a1: &str) -> Value {
        match parse_a1_cell(a1) {
            Some(a) => self.value(sheet, a.row, a.col),
            None => Value::Error(ErrorKind::Ref),
        }
    }

    /// Formula text of a cell (with leading `=`).
    pub fn formula(&self, sheet: usize, row: u32, col: u32) -> Option<&str> {
        self.sheets
            .get(sheet)?
            .cells
            .get(&(row, col))?
            .formula
            .as_ref()
            .map(|f| f.text())
    }

    /// The area an array formula at this anchor spills into.
    pub fn spill_range(&self, sheet: usize, row: u32, col: u32) -> Option<Area> {
        self.spills.get(&CellId::new(sheet, row, col)).copied()
    }

    /// Define a name (workbook scope when `scope` is `None`).
    pub fn define_name(
        &mut self,
        name: &str,
        scope: Option<usize>,
        formula: &str,
    ) -> Result<(), ParseError> {
        let expr = parse_with(formula, &ParseOptions::default())?;
        self.names.insert((scope, name.to_uppercase()), expr);
        self.rebuild_dependencies();
        Ok(())
    }

    /// Register a table for structured references.
    pub fn add_table(&mut self, table: TableInfo) {
        self.tables
            .retain(|t| !t.name.eq_ignore_ascii_case(&table.name));
        self.tables.push(table);
        self.rebuild_dependencies();
    }

    /// Set the pivot table reports `GETPIVOTDATA` reads (replacing the previous ones).
    /// `GETPIVOTDATA` is volatile, so the next recalculation reads the new reports.
    pub fn set_pivots(&mut self, pivots: Vec<PivotInfo>) {
        self.pivots = pivots.into_iter().map(Arc::new).collect();
    }

    /// Cells found in circular references during the last recalculations.
    pub fn circular_references(&self) -> Vec<CellId> {
        let mut v: Vec<CellId> = self.circular.iter().copied().collect();
        v.sort_unstable();
        v
    }

    /// Evaluate an ad-hoc formula as if it were in (`sheet`, `row`, `col`)
    /// against current values (no spilling, nothing stored).
    pub fn evaluate(
        &self,
        sheet: usize,
        row: u32,
        col: u32,
        formula: &str,
    ) -> Result<Value, ParseError> {
        let opts = ParseOptions {
            origin_row: row,
            origin_col: col,
            ..ParseOptions::default()
        };
        let expr = parse_with(formula, &opts)?;
        let mut ctx = Ctx::new(self, sheet, row, col, &self.config, &self.rng);
        let v = ctx.eval(&expr);
        Ok(ctx.finalize(v))
    }

    /// Copy a cell to another position (copy/paste): constants are copied as is,
    /// formulas have their relative references shifted by the distance between
    /// the cells (ECMA-376 Part 1, §18.17.2.3). References pushed off the grid
    /// become `#REF!`.
    pub fn copy_cell(&mut self, from: CellId, to: CellId) {
        let Some(src) = self
            .sheets
            .get(from.sheet)
            .and_then(|s| s.cells.get(&(from.row, from.col)))
        else {
            self.set_value(to.sheet, to.row, to.col, Value::Empty);
            return;
        };
        if let Some(f) = &src.formula {
            let dr = i64::from(to.row) - i64::from(from.row);
            let dc = i64::from(to.col) - i64::from(from.col);
            let expr = offset_relative(&f.expr, dr, dc);
            self.set_formula_expr(to.sheet, to.row, to.col, expr, None, f.fixed);
        } else {
            let v = src.value.clone();
            self.set_value(to.sheet, to.row, to.col, v);
        }
    }

    /// Insert/delete rows or columns on `sheet`, moving cells and adjusting
    /// every formula reference (deleted references become `#REF!`).
    pub fn apply_structural_change(&mut self, sheet: usize, change: StructuralChange) {
        let Some(changed_name) = self.sheets.get(sheet).map(|s| s.name.clone()) else {
            return;
        };
        // Move cells on the changed sheet.
        let old = std::mem::take(&mut self.sheets[sheet].cells);
        let mut moved = FxHashMap::default();
        for ((r, c), data) in old {
            let pos = match change {
                StructuralChange::InsertRows { at, count } => {
                    Some((if r >= at { r + count } else { r }, c))
                }
                StructuralChange::InsertCols { at, count } => {
                    Some((r, if c >= at { c + count } else { c }))
                }
                StructuralChange::DeleteRows { at, count } => {
                    if r >= at && r < at + count {
                        None
                    } else {
                        Some((if r >= at + count { r - count } else { r }, c))
                    }
                }
                StructuralChange::DeleteCols { at, count } => {
                    if c >= at && c < at + count {
                        None
                    } else {
                        Some((r, if c >= at + count { c - count } else { c }))
                    }
                }
            };
            if let Some(p) = pos {
                if p.0 < MAX_ROWS && p.1 < MAX_COLS {
                    moved.insert(p, data);
                }
            }
        }
        self.sheets[sheet].cells = moved;
        // Rewrite every formula in the workbook.
        let mut rewrites = Vec::new();
        for (si, s) in self.sheets.iter().enumerate() {
            for (&(r, c), data) in &s.cells {
                if let Some(f) = &data.formula {
                    let e = adjust_for_change(&f.expr, change, &s.name, &changed_name);
                    rewrites.push((si, r, c, e, f.fixed));
                }
            }
        }
        for (si, r, c, e, fixed) in rewrites {
            if let Some(cell) = self.sheets[si].cells.get_mut(&(r, c)) {
                cell.formula = Some(Arc::new(Formula::new(e, None).with_fixed(fixed)));
            }
        }
        // Defined names: unqualified references belong to the scope sheet; for
        // workbook-scoped names they are left alone (no sheet context).
        let names = std::mem::take(&mut self.names);
        for ((scope, name), expr) in names {
            let home = scope
                .and_then(|s| self.sheets.get(s))
                .map_or("\0", |s| s.name.as_str())
                .to_owned();
            let e = adjust_for_change(&expr, change, &home, &changed_name);
            self.names.insert((scope, name), e);
        }
        self.adjust_tables(sheet, change);
        self.spills.clear();
        self.spill_owner.clear();
        self.spill_blocked.clear();
        self.rebuild_dependencies();
    }

    /// Move/resize tables on `sheet` for a structural change (ECMA-376 Part 1,
    /// §18.5.1.2 table `ref`); a table whose rows or columns are all deleted is removed.
    fn adjust_tables(&mut self, sheet: usize, change: StructuralChange) {
        let rows = is_row_change(change);
        let mut kept = Vec::with_capacity(self.tables.len());
        for mut t in std::mem::take(&mut self.tables) {
            if t.sheet != sheet {
                kept.push(t);
                continue;
            }
            if rows {
                match adjust_span(t.first_row, t.last_row, change) {
                    Some((lo, hi)) => {
                        t.first_row = lo;
                        t.last_row = hi;
                    }
                    None => continue,
                }
            } else {
                let (lo, hi) = (t.first_col, t.last_col());
                let Some((nlo, nhi)) = adjust_span(lo, hi, change) else {
                    continue;
                };
                match change {
                    StructuralChange::InsertCols { at, count } if at > lo && at <= hi => {
                        let pos = (at - lo) as usize;
                        for k in 0..count as usize {
                            let name = format!("Column{}", t.columns.len() + 1);
                            t.columns.insert(pos + k, name);
                        }
                    }
                    StructuralChange::DeleteCols { at, count } => {
                        let from = at.max(lo);
                        let to = at.saturating_add(count).min(hi + 1);
                        if from < to {
                            t.columns.drain((from - lo) as usize..(to - lo) as usize);
                        }
                    }
                    _ => {}
                }
                t.first_col = nlo;
                debug_assert_eq!(t.last_col(), nhi);
            }
            kept.push(t);
        }
        self.tables = kept;
    }

    // ------------------------------------------------------------ graph

    fn remove_formula(&mut self, id: CellId) {
        self.unregister(id);
        self.volatile.remove(&id);
        self.web_cached.remove(&id);
        self.circular.remove(&id);
        self.clear_spill(id);
        self.spill_blocked.remove(&id);
    }

    fn clear_spill(&mut self, id: CellId) {
        if let Some(area) = self.spills.remove(&id) {
            for r in area.r1..=area.r2 {
                for c in area.c1..=area.c2 {
                    let cell = CellId::new(area.sheet, r, c);
                    if cell != id {
                        self.spill_owner.remove(&cell);
                        self.changed.push(cell);
                    }
                }
            }
        }
    }

    fn register(&mut self, id: CellId, areas: Vec<Area>) {
        for a in &areas {
            if a.size() <= SMALL_AREA {
                for r in a.r1..=a.r2 {
                    for c in a.c1..=a.c2 {
                        match self.dependents.entry(CellId::new(a.sheet, r, c)) {
                            std::collections::hash_map::Entry::Occupied(mut e) => {
                                e.get_mut().push(id);
                            }
                            std::collections::hash_map::Entry::Vacant(e) => {
                                e.insert(Readers::One(id));
                            }
                        }
                    }
                }
            } else {
                self.range_deps.insert(*a, id);
            }
        }
        self.precedents.insert(id, areas);
    }

    fn unregister(&mut self, id: CellId) {
        let Some(areas) = self.precedents.remove(&id) else {
            return;
        };
        for a in &areas {
            if a.size() <= SMALL_AREA {
                for r in a.r1..=a.r2 {
                    for c in a.c1..=a.c2 {
                        let key = CellId::new(a.sheet, r, c);
                        if let Some(v) = self.dependents.get_mut(&key) {
                            if v.remove(id) {
                                self.dependents.remove(&key);
                            }
                        }
                    }
                }
            }
        }
        self.range_deps.remove_owner(id);
    }

    /// Static precedents of the formula in cell `id` and whether it is volatile.
    fn collect_deps(&self, expr: &Expr, id: CellId) -> (Vec<Area>, bool) {
        let mut areas = Vec::new();
        let mut volatile = false;
        self.collect_into(expr, id, &mut areas, &mut volatile, 0);
        (areas, volatile)
    }

    fn collect_into(
        &self,
        expr: &Expr,
        id: CellId,
        areas: &mut Vec<Area>,
        volatile: &mut bool,
        depth: u32,
    ) {
        if depth > 32 {
            return;
        }
        let sheet = id.sheet;
        expr.walk(&mut |e| match e {
            Expr::Ref(r) => areas.extend(static_areas(r, self, sheet)),
            Expr::Name { sheet: q, name } => {
                let scope = q
                    .as_ref()
                    .and_then(|s| self.sheet_index(&s.first))
                    .unwrap_or(sheet);
                if let Some(def) = self.defined_name(scope, name) {
                    let at = CellId { sheet: scope, ..id };
                    self.collect_into(&def, at, areas, volatile, depth + 1);
                } else if let Some(t) = self.table(name) {
                    areas.push(Area::new(
                        t.sheet,
                        t.first_row,
                        t.first_col,
                        t.last_row,
                        t.last_col(),
                    ));
                }
            }
            Expr::Structured(s) => {
                if let Ok(a) = struct_area(self, s, sheet, id.row, id.col) {
                    areas.push(a);
                }
            }
            Expr::Call { name, .. } => {
                if is_volatile_function(name) {
                    *volatile = true;
                }
                if crate::functions::lookup(name).is_none() {
                    if let Some(def) = self.defined_name(sheet, name) {
                        self.collect_into(&def, id, areas, volatile, depth + 1);
                    }
                }
            }
            _ => {}
        });
    }

    fn rebuild_dependencies(&mut self) {
        let mut cells = Vec::new();
        for (si, s) in self.sheets.iter().enumerate() {
            for (&(r, c), d) in &s.cells {
                if let Some(f) = &d.formula {
                    cells.push((CellId::new(si, r, c), f.clone()));
                }
            }
        }
        self.precedents.clear();
        self.dependents.clear();
        self.range_deps.clear();
        self.volatile.clear();
        for (id, f) in cells {
            let (areas, volatile) = self.collect_deps(&f.expr, id);
            self.register(id, areas);
            if volatile {
                self.volatile.insert(id);
            }
            self.changed.push(id);
        }
    }

    fn mark_all_formulas(&mut self) {
        for (si, s) in self.sheets.iter().enumerate() {
            for (&(r, c), d) in &s.cells {
                if d.formula.is_some() {
                    self.changed.push(CellId::new(si, r, c));
                }
            }
        }
    }

    fn is_formula(&self, id: CellId) -> bool {
        self.sheets
            .get(id.sheet)
            .and_then(|s| s.cells.get(&(id.row, id.col)))
            .is_some_and(|c| c.formula.is_some())
    }

    // ------------------------------------------------------------ auditing

    /// Areas the formula of a cell reads directly (defined names and table
    /// references resolved); whole-column / whole-row references are clipped
    /// to the used part of their sheet. Empty for constants. Dynamic references
    /// (`INDIRECT`, `OFFSET`) are not known statically and are not listed.
    pub fn direct_precedents(&self, sheet: usize, row: u32, col: u32) -> Vec<Area> {
        let Some(areas) = self.precedents.get(&CellId::new(sheet, row, col)) else {
            return Vec::new();
        };
        let mut out: Vec<Area> = Vec::new();
        for a in areas {
            let (rows, cols) = self.used_extent(a.sheet);
            let r2 = if a.r2 >= MAX_ROWS - 1 {
                a.r2.min(rows.max(a.r1 + 1) - 1)
            } else {
                a.r2
            };
            let c2 = if a.c2 >= MAX_COLS - 1 {
                a.c2.min(cols.max(a.c1 + 1) - 1)
            } else {
                a.c2
            };
            let clipped = Area::new(a.sheet, a.r1, a.c1, r2, c2);
            if !out.contains(&clipped) {
                out.push(clipped);
            }
        }
        out
    }

    /// Formula cells reading a cell directly, sorted.
    pub fn direct_dependents(&self, sheet: usize, row: u32, col: u32) -> Vec<CellId> {
        let mut out = Vec::new();
        self.for_each_reader(CellId::new(sheet, row, col), &mut |d| out.push(d));
        out.sort_unstable();
        out.dedup();
        out
    }

    /// Step-by-step evaluation of the formula of a cell against the current
    /// values (`None` for a cell without a formula).
    pub fn trace(&self, sheet: usize, row: u32, col: u32) -> Option<crate::trace::FormulaTrace> {
        let f = self
            .sheets
            .get(sheet)?
            .cells
            .get(&(row, col))?
            .formula
            .clone()?;
        Some(crate::trace::trace_formula(
            &f.expr,
            self,
            sheet,
            row,
            col,
            &self.config,
        ))
    }

    /// "Precision as displayed" (ECMA-376 Part 1, §18.2.2 calcPr
    /// `fullPrecision="0"`): numbers read from this cell are rounded to
    /// `decimals` places (negative: tens, hundreds…); `None` restores full
    /// precision. Marks the cell's readers for recalculation.
    pub fn set_display_precision(
        &mut self,
        sheet: usize,
        row: u32,
        col: u32,
        decimals: Option<i32>,
    ) {
        let id = CellId::new(sheet, row, col);
        let changed = match decimals {
            Some(d) => self.round_to.insert(id, d) != Some(d),
            None => self.round_to.remove(&id).is_some(),
        };
        if changed {
            self.after_edit(id);
        }
    }

    /// Removes every "precision as displayed" rounding.
    pub fn clear_display_precision(&mut self) {
        let ids: Vec<CellId> = self.round_to.keys().copied().collect();
        self.round_to.clear();
        for id in ids {
            self.after_edit(id);
        }
    }

    /// Formula cells reading `cell` directly (static precedents).
    fn for_each_reader(&self, cell: CellId, f: &mut dyn FnMut(CellId)) {
        if let Some(ds) = self.dependents.get(&cell) {
            for d in ds.as_slice() {
                f(*d);
            }
        }
        self.range_deps.for_each_reader(cell, f);
    }

    /// Formula cells depending (transitively) on the seeds, plus seed formula
    /// cells, with the reader edges between them.
    fn propagate(&self, seeds: Vec<CellId>) -> DirtyGraph {
        // Bulk edits (loading a sheet): every formula is dirty; start from the
        // formulas instead of the (many more) edited constants.
        let seeds = if seeds.len() >= self.precedents.len().max(1) * 4 {
            self.precedents.keys().copied().collect()
        } else {
            seeds
        };
        // Visited cell → node index (`u32::MAX` for non-formula cells).
        let mut seen: FxHashMap<CellId, u32> = FxHashMap::default();
        let mut nodes = Vec::new();
        let mut edges: Vec<(u32, CellId)> = Vec::new();
        let mut stack = seeds;
        while let Some(c) = stack.pop() {
            if seen.contains_key(&c) {
                continue;
            }
            let ci = if self.is_formula(c) {
                nodes.push(c);
                (nodes.len() - 1) as u32
            } else {
                u32::MAX
            };
            seen.insert(c, ci);
            self.for_each_reader(c, &mut |d| {
                if ci != u32::MAX {
                    edges.push((ci, d));
                }
                if !seen.contains_key(&d) {
                    stack.push(d);
                }
            });
        }
        let mut succ = vec![Vec::new(); nodes.len()];
        for (ci, d) in edges {
            if let Some(&di) = seen.get(&d) {
                if di != u32::MAX {
                    succ[ci as usize].push(di);
                }
            }
        }
        DirtyGraph { nodes, succ }
    }
    // ------------------------------------------------------------ recalc

    /// Recalculate cells affected by edits since the last recalculation, plus
    /// volatile cells.
    pub fn recalculate(&mut self) {
        self.evaluations.store(0, Ordering::Relaxed);
        let mut seeds = std::mem::take(&mut self.changed);
        seeds.extend(self.volatile.iter().copied());
        let dirty = self.propagate(seeds);
        self.recalculated = dirty.nodes.clone();
        self.calc_set(&dirty);
        for _ in 0..SPILL_PASSES {
            if self.spill_changed.is_empty() {
                break;
            }
            let seeds = std::mem::take(&mut self.spill_changed);
            let dirty = self.propagate(seeds);
            self.recalculated.extend(dirty.nodes.iter().copied());
            self.calc_set(&dirty);
        }
        self.recalculated.sort_unstable();
        self.recalculated.dedup();
        self.spill_changed.clear();
    }

    /// Number of formula evaluations performed by the last
    /// [`Workbook::recalculate`] (diagnostics for incremental recalculation).
    pub fn last_recalc_evaluations(&self) -> usize {
        self.evaluations.load(Ordering::Relaxed)
    }

    /// Cells whose value the last [`Workbook::recalculate`] may have changed
    /// (formula cells that were evaluated, plus edited cells), sorted.
    pub fn last_recalculated(&self) -> &[CellId] {
        &self.recalculated
    }

    /// Recalculate every formula.
    pub fn recalculate_all(&mut self) {
        self.mark_all_formulas();
        self.recalculate();
    }

    fn clear_pass_caches(&self) {
        if let Ok(mut c) = self.area_cache.write() {
            c.clear();
        }
        if let Ok(mut c) = self.index_cache.write() {
            c.clear();
        }
    }

    fn calc_set(&mut self, dirty: &DirtyGraph) {
        self.clear_pass_caches();
        if dirty.nodes.is_empty() {
            return;
        }
        let order = dirty.order(PAR_MIN_LEVEL);
        let mut pending: FxHashSet<CellId> = dirty.nodes.iter().copied().collect();
        let mut in_progress = FxHashSet::default();
        for step in order {
            match step {
                Step::Seq(list) => {
                    for i in list {
                        self.calc_one(dirty.nodes[i as usize], &mut pending, &mut in_progress);
                    }
                }
                Step::Level(list) => {
                    let ids: Vec<CellId> = list.iter().map(|&i| dirty.nodes[i as usize]).collect();
                    self.calc_level(&ids, &mut pending, &mut in_progress);
                }
                Step::Cycle(scc) => {
                    let ids: Vec<CellId> = scc.iter().map(|&i| dirty.nodes[i as usize]).collect();
                    self.calc_cycle(&ids, &pending);
                    for id in ids {
                        pending.remove(&id);
                    }
                }
            }
        }
    }

    /// Evaluate one topological level: the cells do not read each other
    /// statically, so they are evaluated in parallel on native targets (not
    /// when a volatile function is involved, to keep `RAND` sequences
    /// deterministic). Cells whose dynamic references hit uncomputed cells
    /// fall back to [`Workbook::calc_one`].
    fn calc_level(
        &mut self,
        ids: &[CellId],
        pending: &mut FxHashSet<CellId>,
        in_progress: &mut FxHashSet<CellId>,
    ) {
        let parallel = ids.iter().all(|id| !self.volatile.contains(id));
        let results = if parallel {
            self.eval_parallel(ids, pending)
        } else {
            None
        };
        let Some(results) = results else {
            for id in ids {
                self.calc_one(*id, pending, in_progress);
            }
            return;
        };
        let mut retry = Vec::new();
        for (id, (v, missed)) in ids.iter().zip(results) {
            if missed {
                retry.push(*id);
            } else {
                self.store(*id, v);
                if !self.circular.is_empty() {
                    self.circular.remove(id);
                }
                pending.remove(id);
            }
        }
        for id in retry {
            self.calc_one(id, pending, in_progress);
        }
    }

    /// Evaluate independent cells on worker threads; `None` when threads are
    /// unavailable. Each result tells whether a not-yet-computed cell was read.
    #[cfg(not(target_arch = "wasm32"))]
    fn eval_parallel(
        &self,
        ids: &[CellId],
        pending: &FxHashSet<CellId>,
    ) -> Option<Vec<(Value, bool)>> {
        let threads = std::thread::available_parallelism()
            .map_or(1, std::num::NonZeroUsize::get)
            .min(MAX_THREADS);
        if threads < 2 {
            return None;
        }
        let chunk = ids.len().div_ceil(threads);
        let empty = FxHashSet::default();
        let eval_chunk = |part: &[CellId]| -> Vec<(Value, bool)> {
            part.iter()
                .map(|&id| {
                    let view = RecalcView {
                        wb: self,
                        pending,
                        in_progress: &empty,
                        misses: RefCell::new(Vec::new()),
                        cache: true,
                    };
                    let v = self.eval_cell(id, &view);
                    let missed = view
                        .misses
                        .into_inner()
                        .iter()
                        .any(|m| *m != id && pending.contains(m));
                    (v, missed)
                })
                .collect()
        };
        std::thread::scope(|s| {
            let handles: Vec<_> = ids
                .chunks(chunk)
                .map(|part| s.spawn(move || eval_chunk(part)))
                .collect();
            let mut out = Vec::with_capacity(ids.len());
            for h in handles {
                out.extend(h.join().ok()?);
            }
            Some(out)
        })
    }

    #[cfg(target_arch = "wasm32")]
    #[allow(clippy::unused_self)]
    fn eval_parallel(
        &self,
        _ids: &[CellId],
        _pending: &FxHashSet<CellId>,
    ) -> Option<Vec<(Value, bool)>> {
        None
    }
    fn eval_cell(&self, id: CellId, view: &dyn CellProvider) -> Value {
        let Some(f) = self
            .sheets
            .get(id.sheet)
            .and_then(|s| s.cells.get(&(id.row, id.col)))
            .and_then(|c| c.formula.clone())
        else {
            return Value::Empty;
        };
        self.evaluations.fetch_add(1, Ordering::Relaxed);
        let mut ctx = Ctx::new(view, id.sheet, id.row, id.col, &self.config, &self.rng);
        let v = ctx.eval(&f.expr);
        if ctx.web_offline && self.web_cached.contains(&id) {
            // `WEBSERVICE` without a provider: keep the value saved with the file.
            if let Some(c) = self.sheets[id.sheet].cells.get(&(id.row, id.col)) {
                return c.value.clone();
            }
        }
        ctx.finalize(v)
    }

    /// Evaluate one cell; dynamic reads of uncomputed cells are computed first.
    fn calc_one(
        &mut self,
        id: CellId,
        pending: &mut FxHashSet<CellId>,
        in_progress: &mut FxHashSet<CellId>,
    ) {
        let mut stack = vec![id];
        while let Some(&top) = stack.last() {
            if !pending.contains(&top) {
                stack.pop();
                continue;
            }
            in_progress.insert(top);
            let (v, misses) = {
                let view = RecalcView {
                    wb: self,
                    pending,
                    in_progress,
                    misses: RefCell::new(Vec::new()),
                    cache: true,
                };
                let v = self.eval_cell(top, &view);
                (v, view.misses.into_inner())
            };
            let fresh: Vec<CellId> = misses
                .into_iter()
                .filter(|m| pending.contains(m) && !in_progress.contains(m))
                .collect();
            if fresh.is_empty() || stack.len() > 100_000 {
                self.store(top, v);
                if !self.circular.is_empty() {
                    self.circular.remove(&top);
                }
                pending.remove(&top);
                in_progress.remove(&top);
                stack.pop();
            } else {
                stack.extend(fresh);
            }
        }
    }

    fn calc_cycle(&mut self, ids: &[CellId], pending: &FxHashSet<CellId>) {
        self.circular.extend(ids.iter().copied());
        let Some(it) = self.iterative else {
            for id in ids {
                self.store(*id, Value::Error(ErrorKind::Calc));
            }
            return;
        };
        let empty = FxHashSet::default();
        for _ in 0..it.max_iterations.max(1) {
            let mut max_delta: f64 = 0.0;
            for id in ids {
                let old = self.read(*id);
                let v = {
                    let view = RecalcView {
                        wb: self,
                        pending: &empty,
                        in_progress: pending,
                        misses: RefCell::new(Vec::new()),
                        cache: false,
                    };
                    self.eval_cell(*id, &view)
                };
                self.store(*id, v);
                // Compared as read back (rounded under "precision as displayed").
                let v = self.read(*id);
                let delta = match (&old, &v) {
                    (Value::Number(a), Value::Number(b)) => (a - b).abs(),
                    (Value::Empty, Value::Number(b)) => b.abs(),
                    (a, b) if a == b => 0.0,
                    _ => f64::INFINITY,
                };
                max_delta = max_delta.max(delta);
            }
            if max_delta < it.max_change {
                break;
            }
        }
    }

    /// Store a computed value, placing or clearing its spill range.
    fn store(&mut self, id: CellId, v: Value) {
        let fixed = self.sheets[id.sheet]
            .cells
            .get(&(id.row, id.col))
            .and_then(|c| c.formula.as_ref())
            .and_then(|f| f.fixed);
        let v = match fixed {
            Some((rows, cols)) => fit_array(v, rows, cols),
            None => v,
        };
        // Fast path: a scalar result for a cell that does not spill.
        if !matches!(v, Value::Array(_)) && !self.spills.contains_key(&id) {
            if !self.spill_blocked.is_empty() {
                self.spill_blocked.remove(&id);
            }
            if let Some(cell) = self.sheets[id.sheet].cells.get_mut(&(id.row, id.col)) {
                cell.value = v;
            }
            return;
        }
        let old_area = self.spills.get(&id).copied();
        self.clear_spill_silent(id);
        self.spill_blocked.remove(&id);
        let v = match v {
            Value::Array(a) if a.rows() * a.cols() > 1 => {
                let r2 = u64::from(id.row) + a.rows() as u64 - 1;
                let c2 = u64::from(id.col) + a.cols() as u64 - 1;
                if r2 >= u64::from(MAX_ROWS) || c2 >= u64::from(MAX_COLS) {
                    self.spill_blocked.insert(id);
                    Value::Error(ErrorKind::Spill)
                } else {
                    let area = Area::new(id.sheet, id.row, id.col, r2 as u32, c2 as u32);
                    // A legacy array formula owns its range: nothing blocks it.
                    if fixed.is_none() && self.spill_is_blocked(id, &area) {
                        self.spill_blocked.insert(id);
                        Value::Error(ErrorKind::Spill)
                    } else {
                        for r in area.r1..=area.r2 {
                            for c in area.c1..=area.c2 {
                                let cell = CellId::new(id.sheet, r, c);
                                if cell != id {
                                    self.spill_owner.insert(cell, id);
                                }
                            }
                        }
                        self.spills.insert(id, area);
                        let s = &mut self.sheets[id.sheet];
                        s.rows = s.rows.max(area.r2 + 1);
                        s.cols = s.cols.max(area.c2 + 1);
                        Value::Array(a)
                    }
                }
            }
            Value::Array(a) => a.get(0, 0).clone(),
            v => v,
        };
        let new_area = self.spills.get(&id).copied();
        let spilled_changed = old_area != new_area
            || (new_area.is_some() && {
                let cur = self.sheets[id.sheet]
                    .cells
                    .get(&(id.row, id.col))
                    .map(|c| &c.value);
                cur != Some(&v)
            });
        if let Some(cell) = self.sheets[id.sheet].cells.get_mut(&(id.row, id.col)) {
            cell.value = v;
        }
        if spilled_changed {
            self.clear_pass_caches();
            for a in [old_area, new_area].into_iter().flatten() {
                for r in a.r1..=a.r2 {
                    for c in a.c1..=a.c2 {
                        let cell = CellId::new(a.sheet, r, c);
                        if cell != id {
                            self.spill_changed.push(cell);
                        }
                    }
                }
            }
        }
    }

    fn clear_spill_silent(&mut self, id: CellId) {
        // (Legacy array formulas use the same spill bookkeeping for their fixed range.)
        if let Some(area) = self.spills.remove(&id) {
            for r in area.r1..=area.r2 {
                for c in area.c1..=area.c2 {
                    let cell = CellId::new(area.sheet, r, c);
                    if self.spill_owner.get(&cell) == Some(&id) {
                        self.spill_owner.remove(&cell);
                    }
                }
            }
        }
    }

    fn spill_is_blocked(&self, id: CellId, area: &Area) -> bool {
        let sheet = &self.sheets[id.sheet];
        for r in area.r1..=area.r2 {
            for c in area.c1..=area.c2 {
                if (r, c) == (id.row, id.col) {
                    continue;
                }
                if let Some(cell) = sheet.cells.get(&(r, c)) {
                    if cell.formula.is_some() || !matches!(cell.value, Value::Empty) {
                        return true;
                    }
                }
                if let Some(owner) = self.spill_owner.get(&CellId::new(id.sheet, r, c)) {
                    if *owner != id {
                        return true;
                    }
                }
            }
        }
        false
    }

    fn read(&self, id: CellId) -> Value {
        let v = self.read_full(id);
        if self.round_to.is_empty() {
            return v;
        }
        match (v, self.round_to.get(&id)) {
            (Value::Number(n), Some(&d)) => Value::Number(round_decimals(n, d)),
            (v, _) => v,
        }
    }

    fn read_full(&self, id: CellId) -> Value {
        let Some(sheet) = self.sheets.get(id.sheet) else {
            return Value::Error(ErrorKind::Ref);
        };
        if let Some(c) = sheet.cells.get(&(id.row, id.col)) {
            return match &c.value {
                Value::Array(a) => a.get(0, 0).clone(),
                v => v.clone(),
            };
        }
        if let Some(owner) = self.spill_owner.get(&id) {
            if let Some(Value::Array(a)) = self
                .sheets
                .get(owner.sheet)
                .and_then(|s| s.cells.get(&(owner.row, owner.col)))
                .map(|c| &c.value)
            {
                let r = (id.row - owner.row) as usize;
                let c = (id.col - owner.col) as usize;
                if r < a.rows() && c < a.cols() {
                    return a.get(r, c).clone();
                }
            }
        }
        Value::Empty
    }
}

/// Round half away from zero to `d` decimal places (negative `d`: to tens,
/// hundreds…), after removing binary noise beyond 15 significant digits.
fn round_decimals(n: f64, d: i32) -> f64 {
    let d = d.clamp(-15, 15);
    let m = 10f64.powi(d);
    let r = crate::value::round_sig15(n * m).round() / m;
    if r.is_finite() {
        r
    } else {
        n
    }
}

/// Topological levels at least this large are evaluated in parallel.
const PAR_MIN_LEVEL: usize = 512;
/// Upper bound on worker threads for one level.
#[cfg(not(target_arch = "wasm32"))]
const MAX_THREADS: usize = 8;

/// Dirty formula cells of one recalculation pass and the edges between them.
struct DirtyGraph {
    nodes: Vec<CellId>,
    /// `succ[i]`: dirty formula cells reading node `i` (one entry per reference).
    succ: Vec<Vec<u32>>,
}

/// One evaluation step of a recalculation pass.
enum Step {
    /// Nodes evaluated one by one, in order.
    Seq(Vec<u32>),
    /// A large topological level: independent nodes.
    Level(Vec<u32>),
    /// A circular reference (strongly connected component).
    Cycle(Vec<u32>),
}

impl DirtyGraph {
    /// Evaluation order: Kahn's topological sort by levels for the acyclic
    /// part (small consecutive levels merged into one sequential step), then
    /// the remaining nodes (cycles and their dependents) ordered by Tarjan SCC
    /// with precedents first.
    fn order(&self, par_min: usize) -> Vec<Step> {
        let n = self.nodes.len();
        let mut indeg = vec![0u32; n];
        for s in &self.succ {
            for &d in s {
                indeg[d as usize] += 1;
            }
        }
        let mut steps = Vec::new();
        let mut seq: Vec<u32> = Vec::new();
        let mut level: Vec<u32> = (0..n as u32).filter(|&i| indeg[i as usize] == 0).collect();
        let mut done = 0;
        while !level.is_empty() {
            done += level.len();
            let mut next = Vec::new();
            for &i in &level {
                for &d in &self.succ[i as usize] {
                    let e = &mut indeg[d as usize];
                    *e -= 1;
                    if *e == 0 {
                        next.push(d);
                    }
                }
            }
            if level.len() >= par_min {
                if !seq.is_empty() {
                    steps.push(Step::Seq(std::mem::take(&mut seq)));
                }
                steps.push(Step::Level(level));
            } else {
                seq.extend(level);
            }
            level = next;
        }
        if done < n {
            // Remaining nodes: build their precedent lists and order the SCCs.
            let rest: Vec<u32> = (0..n as u32).filter(|&i| indeg[i as usize] > 0).collect();
            let mut local = vec![u32::MAX; n];
            for (k, &i) in rest.iter().enumerate() {
                local[i as usize] = k as u32;
            }
            let mut preds = vec![Vec::new(); rest.len()];
            for (k, &i) in rest.iter().enumerate() {
                for &d in &self.succ[i as usize] {
                    let ld = local[d as usize];
                    if ld != u32::MAX {
                        preds[ld as usize].push(k);
                    }
                }
            }
            for scc in tarjan(&preds) {
                let cyclic = scc.len() > 1 || preds[scc[0]].contains(&scc[0]);
                if cyclic {
                    if !seq.is_empty() {
                        steps.push(Step::Seq(std::mem::take(&mut seq)));
                    }
                    steps.push(Step::Cycle(scc.iter().map(|&k| rest[k]).collect()));
                } else {
                    seq.push(rest[scc[0]]);
                }
            }
        }
        if !seq.is_empty() {
            steps.push(Step::Seq(seq));
        }
        steps
    }
}

/// Iterative Tarjan SCC; components are emitted with precedents first.
fn tarjan(adj: &[Vec<usize>]) -> Vec<Vec<usize>> {
    let n = adj.len();
    let mut index = vec![usize::MAX; n];
    let mut low = vec![0usize; n];
    let mut on_stack = vec![false; n];
    let mut stack = Vec::new();
    let mut out = Vec::new();
    let mut counter = 0usize;
    for root in 0..n {
        if index[root] != usize::MAX {
            continue;
        }
        let mut call: Vec<(usize, usize)> = vec![(root, 0)];
        index[root] = counter;
        low[root] = counter;
        counter += 1;
        stack.push(root);
        on_stack[root] = true;
        while let Some(&mut (v, ref mut ei)) = call.last_mut() {
            if *ei < adj[v].len() {
                let w = adj[v][*ei];
                *ei += 1;
                if index[w] == usize::MAX {
                    index[w] = counter;
                    low[w] = counter;
                    counter += 1;
                    stack.push(w);
                    on_stack[w] = true;
                    call.push((w, 0));
                } else if on_stack[w] {
                    low[v] = low[v].min(index[w]);
                }
            } else {
                call.pop();
                if let Some(&(u, _)) = call.last() {
                    low[u] = low[u].min(low[v]);
                }
                if low[v] == index[v] {
                    let mut comp = Vec::new();
                    while let Some(w) = stack.pop() {
                        on_stack[w] = false;
                        comp.push(w);
                        if w == v {
                            break;
                        }
                    }
                    out.push(comp);
                }
            }
        }
    }
    out
}

impl CellProvider for Workbook {
    fn sheet_count(&self) -> usize {
        self.sheets.len()
    }

    fn sheet_name(&self, sheet: usize) -> Option<&str> {
        self.sheets.get(sheet).map(|s| s.name.as_str())
    }

    fn cell_value(&self, sheet: usize, row: u32, col: u32) -> Value {
        self.read(CellId::new(sheet, row, col))
    }

    fn cell_formula(&self, sheet: usize, row: u32, col: u32) -> Option<String> {
        self.formula(sheet, row, col).map(str::to_owned)
    }

    fn defined_name(&self, sheet: usize, name: &str) -> Option<Expr> {
        let up = name.to_uppercase();
        self.names
            .get(&(Some(sheet), up.clone()))
            .or_else(|| self.names.get(&(None, up)))
            .cloned()
    }

    fn table(&self, name: &str) -> Option<TableInfo> {
        self.tables
            .iter()
            .find(|t| t.name.eq_ignore_ascii_case(name))
            .cloned()
    }

    fn table_at(&self, sheet: usize, row: u32, col: u32) -> Option<TableInfo> {
        self.tables
            .iter()
            .find(|t| {
                t.sheet == sheet
                    && row >= t.first_row
                    && row <= t.last_row
                    && col >= t.first_col
                    && col <= t.last_col()
            })
            .cloned()
    }

    fn used_extent(&self, sheet: usize) -> (u32, u32) {
        self.sheets.get(sheet).map_or((0, 0), |s| (s.rows, s.cols))
    }

    fn is_row_hidden(&self, sheet: usize, row: u32) -> bool {
        self.sheets
            .get(sheet)
            .is_some_and(|s| s.hidden_rows.contains(&row))
    }

    fn is_row_filtered(&self, sheet: usize, row: u32) -> bool {
        self.sheets.get(sheet).is_some_and(|s| {
            s.filter_spans.iter().any(|&(a, b)| row >= a && row <= b)
                && s.hidden_rows.contains(&row)
        })
    }

    fn pivot_at(&self, sheet: usize, row: u32, col: u32) -> Option<Arc<PivotInfo>> {
        self.pivots
            .iter()
            .find(|p| p.area.contains(sheet, row, col))
            .cloned()
    }

    fn spill_area(&self, sheet: usize, row: u32, col: u32) -> Option<Area> {
        let id = CellId::new(sheet, row, col);
        self.spills.get(&id).copied().or_else(|| {
            // A formula returning a single value is a 1x1 "spill".
            self.is_formula(id)
                .then(|| Area::cell(sheet, row, col))
                .filter(|_| !matches!(self.read(id), Value::Error(ErrorKind::Spill)))
        })
    }
}

/// Provider used during recalculation: records reads of cells that are still
/// waiting to be computed.
struct RecalcView<'a> {
    wb: &'a Workbook,
    pending: &'a FxHashSet<CellId>,
    in_progress: &'a FxHashSet<CellId>,
    misses: RefCell<Vec<CellId>>,
    /// Whether materialized areas may be cached (not during iteration).
    cache: bool,
}

impl CellProvider for RecalcView<'_> {
    fn sheet_count(&self) -> usize {
        self.wb.sheet_count()
    }

    fn sheet_name(&self, sheet: usize) -> Option<&str> {
        self.wb.sheet_name(sheet)
    }

    fn sheet_index(&self, name: &str) -> Option<usize> {
        self.wb.sheet_index(name)
    }

    fn cell_value(&self, sheet: usize, row: u32, col: u32) -> Value {
        let id = CellId::new(sheet, row, col);
        if !self.pending.is_empty() && self.pending.contains(&id) && !self.in_progress.contains(&id)
        {
            self.misses.borrow_mut().push(id);
        }
        self.wb.read(id)
    }

    fn cell_formula(&self, sheet: usize, row: u32, col: u32) -> Option<String> {
        self.wb.cell_formula(sheet, row, col)
    }

    fn defined_name(&self, sheet: usize, name: &str) -> Option<Expr> {
        self.wb.defined_name(sheet, name)
    }

    fn table(&self, name: &str) -> Option<TableInfo> {
        self.wb.table(name)
    }

    fn table_at(&self, sheet: usize, row: u32, col: u32) -> Option<TableInfo> {
        self.wb.table_at(sheet, row, col)
    }

    fn used_extent(&self, sheet: usize) -> (u32, u32) {
        self.wb.used_extent(sheet)
    }

    fn is_row_hidden(&self, sheet: usize, row: u32) -> bool {
        self.wb.is_row_hidden(sheet, row)
    }

    fn is_row_filtered(&self, sheet: usize, row: u32) -> bool {
        self.wb.is_row_filtered(sheet, row)
    }

    fn spill_area(&self, sheet: usize, row: u32, col: u32) -> Option<Area> {
        self.wb.spill_area(sheet, row, col)
    }

    fn pivot_at(&self, sheet: usize, row: u32, col: u32) -> Option<Arc<PivotInfo>> {
        self.wb.pivot_at(sheet, row, col)
    }

    fn area_array(&self, area: &Area) -> Option<Arc<Array>> {
        if !self.cache {
            return None;
        }
        if let Some(a) = self.wb.area_cache.read().ok()?.get(area) {
            return Some(a.clone());
        }
        // Only areas whose cells are all final in this pass can be cached.
        let mut data = Vec::with_capacity(area.size() as usize);
        for r in area.r1..=area.r2 {
            for c in area.c1..=area.c2 {
                let id = CellId::new(area.sheet, r, c);
                if self.pending.contains(&id) {
                    return None;
                }
                data.push(self.wb.read(id));
            }
        }
        let arr = Arc::new(Array::new(area.rows() as usize, area.cols() as usize, data));
        if let Ok(mut c) = self.wb.area_cache.write() {
            c.insert(*area, arr.clone());
        }
        Some(arr)
    }

    fn exact_index(&self, area: &Area) -> Option<Arc<ExactIndex>> {
        if !self.cache {
            return None;
        }
        if let Some(ix) = self.wb.index_cache.read().ok()?.get(area) {
            return Some(ix.clone());
        }
        // Like `area_array`: only vectors whose cells are all final.
        let mut data = Vec::with_capacity(area.size() as usize);
        for r in area.r1..=area.r2 {
            for c in area.c1..=area.c2 {
                let id = CellId::new(area.sheet, r, c);
                if self.pending.contains(&id) {
                    return None;
                }
                data.push(self.wb.read(id));
            }
        }
        let ix = Arc::new(ExactIndex::build(data.iter()));
        if let Ok(mut c) = self.wb.index_cache.write() {
            c.insert(*area, ix.clone());
        }
        Some(ix)
    }
}

/// The result of a legacy array formula fitted to its `rows` × `cols` range (ECMA-376
/// Part 1, §18.17.2.4): a single value repeats, a one-row / one-column array repeats down /
/// across, positions beyond a larger array are `#N/A`, and a bigger array is cut.
fn fit_array(v: Value, rows: u32, cols: u32) -> Value {
    let (rows, cols) = (rows as usize, cols as usize);
    if rows * cols <= 1 {
        return match v {
            Value::Array(a) => a.get(0, 0).clone(),
            v => v,
        };
    }
    let data: Vec<Value> = match &v {
        Value::Array(a) => {
            let (ar, ac) = (a.rows(), a.cols());
            (0..rows)
                .flat_map(|r| (0..cols).map(move |c| (r, c)))
                .map(|(r, c)| {
                    let rr = if ar == 1 { 0 } else { r };
                    let cc = if ac == 1 { 0 } else { c };
                    if rr < ar && cc < ac {
                        a.get(rr, cc).clone()
                    } else {
                        Value::Error(ErrorKind::NA)
                    }
                })
                .collect()
        }
        v => vec![v.clone(); rows * cols],
    };
    Value::Array(Arc::new(Array::new(rows, cols, data)))
}
