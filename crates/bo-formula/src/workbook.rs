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

use std::cell::{Cell, RefCell};
use std::sync::Arc;

use crate::ast::Expr;
use crate::date::DateSystem;
use crate::eval::{static_areas, Ctx, EvalConfig};
use crate::format::format;
use crate::functions::is_volatile_function;
use crate::fxhash::{FxHashMap, FxHashSet};
use crate::parser::{parse_with, ParseError, ParseOptions};
use crate::provider::{CellProvider, TableInfo};
use crate::reference::{parse_a1_cell, MAX_COLS, MAX_ROWS};
use crate::shift::{adjust_for_change, StructuralChange};
use crate::value::{parse_number_text, Area, ErrorKind, Value};

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
    text: String,
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
    config: EvalConfig,
    iterative: Option<IterativeConfig>,
    rng: Cell<u64>,
    precedents: FxHashMap<CellId, Vec<Area>>,
    dependents: FxHashMap<CellId, Vec<CellId>>,
    range_deps: Vec<(Area, CellId)>,
    volatile: FxHashSet<CellId>,
    changed: Vec<CellId>,
    spills: FxHashMap<CellId, Area>,
    spill_owner: FxHashMap<CellId, CellId>,
    spill_blocked: FxHashSet<CellId>,
    spill_changed: Vec<CellId>,
    circular: FxHashSet<CellId>,
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
            config: EvalConfig::default(),
            iterative: None,
            rng: Cell::new(0x2545_F491_4F6C_DD1D),
            precedents: FxHashMap::default(),
            dependents: FxHashMap::default(),
            range_deps: Vec::new(),
            volatile: FxHashSet::default(),
            changed: Vec::new(),
            spills: FxHashMap::default(),
            spill_owner: FxHashMap::default(),
            spill_blocked: FxHashSet::default(),
            spill_changed: Vec::new(),
            circular: FxHashSet::default(),
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

    /// Seed the generator used by `RAND`/`RANDBETWEEN`/`RANDARRAY`.
    pub fn seed_random(&mut self, seed: u64) {
        self.rng.set(seed.max(1));
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
        self.set_formula_expr(sheet, row, col, expr, None);
        Ok(())
    }

    fn set_formula_expr(
        &mut self,
        sheet: usize,
        row: u32,
        col: u32,
        expr: Expr,
        text: Option<String>,
    ) {
        if sheet >= self.sheets.len() {
            return;
        }
        let id = CellId::new(sheet, row, col);
        self.remove_formula(id);
        let text = text.unwrap_or_else(|| format!("={}", format(&expr)));
        let (areas, volatile) = self.collect_deps(&expr, sheet);
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
                formula: Some(Arc::new(Formula { expr, text })),
                value: old,
            },
        );
        self.touch(id);
        self.after_edit(id);
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
            .map(|f| f.text.as_str())
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
                    rewrites.push((si, r, c, e));
                }
            }
        }
        for (si, r, c, e) in rewrites {
            if let Some(cell) = self.sheets[si].cells.get_mut(&(r, c)) {
                let text = format!("={}", format(&e));
                cell.formula = Some(Arc::new(Formula { expr: e, text }));
            }
        }
        self.spills.clear();
        self.spill_owner.clear();
        self.spill_blocked.clear();
        self.rebuild_dependencies();
    }

    // ------------------------------------------------------------ graph

    fn remove_formula(&mut self, id: CellId) {
        self.unregister(id);
        self.volatile.remove(&id);
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
                        self.dependents
                            .entry(CellId::new(a.sheet, r, c))
                            .or_default()
                            .push(id);
                    }
                }
            } else {
                self.range_deps.push((*a, id));
            }
        }
        self.precedents.insert(id, areas);
    }

    fn unregister(&mut self, id: CellId) {
        let Some(areas) = self.precedents.remove(&id) else {
            return;
        };
        let mut has_large = false;
        for a in &areas {
            if a.size() <= SMALL_AREA {
                for r in a.r1..=a.r2 {
                    for c in a.c1..=a.c2 {
                        let key = CellId::new(a.sheet, r, c);
                        if let Some(v) = self.dependents.get_mut(&key) {
                            v.retain(|d| *d != id);
                            if v.is_empty() {
                                self.dependents.remove(&key);
                            }
                        }
                    }
                }
            } else {
                has_large = true;
            }
        }
        if has_large {
            self.range_deps.retain(|(_, d)| *d != id);
        }
    }

    /// Static precedents of an expression and whether it is volatile.
    fn collect_deps(&self, expr: &Expr, sheet: usize) -> (Vec<Area>, bool) {
        let mut areas = Vec::new();
        let mut volatile = false;
        self.collect_into(expr, sheet, &mut areas, &mut volatile, 0);
        (areas, volatile)
    }

    fn collect_into(
        &self,
        expr: &Expr,
        sheet: usize,
        areas: &mut Vec<Area>,
        volatile: &mut bool,
        depth: u32,
    ) {
        if depth > 32 {
            return;
        }
        expr.walk(&mut |e| match e {
            Expr::Ref(r) => areas.extend(static_areas(r, self, sheet)),
            Expr::Name { sheet: q, name } => {
                let scope = q
                    .as_ref()
                    .and_then(|s| self.sheet_index(&s.first))
                    .unwrap_or(sheet);
                if let Some(def) = self.defined_name(scope, name) {
                    self.collect_into(&def, scope, areas, volatile, depth + 1);
                }
            }
            Expr::Structured(s) => {
                let t = match &s.table {
                    Some(n) => self.table(n),
                    None => None,
                };
                if let Some(t) = t {
                    areas.push(Area::new(
                        t.sheet,
                        t.first_row,
                        t.first_col,
                        t.last_row,
                        t.last_col(),
                    ));
                } else if s.table.is_none() {
                    // In-table shorthand: depend on every table on this sheet.
                    for t in self.tables.iter().filter(|t| t.sheet == sheet) {
                        areas.push(Area::new(
                            t.sheet,
                            t.first_row,
                            t.first_col,
                            t.last_row,
                            t.last_col(),
                        ));
                    }
                }
            }
            Expr::Call { name, .. } => {
                if is_volatile_function(name) {
                    *volatile = true;
                }
                if crate::functions::lookup(name).is_none() {
                    if let Some(def) = self.defined_name(sheet, name) {
                        self.collect_into(&def, sheet, areas, volatile, depth + 1);
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
            let (areas, volatile) = self.collect_deps(&f.expr, id.sheet);
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

    /// Formula cells depending (transitively) on the seeds, plus seed formula cells.
    fn propagate(&self, seeds: Vec<CellId>) -> FxHashSet<CellId> {
        let mut dirty = FxHashSet::default();
        let mut seen = FxHashSet::default();
        let mut stack = seeds;
        while let Some(c) = stack.pop() {
            if !seen.insert(c) {
                continue;
            }
            if self.is_formula(c) {
                dirty.insert(c);
            }
            if let Some(ds) = self.dependents.get(&c) {
                stack.extend(ds.iter().filter(|d| !seen.contains(d)));
            }
            for (a, d) in &self.range_deps {
                if a.contains(c.sheet, c.row, c.col) && !seen.contains(d) {
                    stack.push(*d);
                }
            }
        }
        dirty
    }

    // ------------------------------------------------------------ recalc

    /// Recalculate cells affected by edits since the last recalculation, plus
    /// volatile cells.
    pub fn recalculate(&mut self) {
        let mut seeds = std::mem::take(&mut self.changed);
        seeds.extend(self.volatile.iter().copied());
        let dirty = self.propagate(seeds);
        self.calc_set(dirty);
        for _ in 0..SPILL_PASSES {
            if self.spill_changed.is_empty() {
                break;
            }
            let seeds = std::mem::take(&mut self.spill_changed);
            let dirty = self.propagate(seeds);
            self.calc_set(dirty);
        }
        self.spill_changed.clear();
    }

    /// Recalculate every formula.
    pub fn recalculate_all(&mut self) {
        self.mark_all_formulas();
        self.recalculate();
    }

    /// Successor lists (dirty precedents) for Tarjan ordering.
    fn adjacency(&self, nodes: &[CellId], index: &FxHashMap<CellId, usize>) -> Vec<Vec<usize>> {
        nodes
            .iter()
            .map(|id| {
                let mut out = Vec::new();
                if let Some(areas) = self.precedents.get(id) {
                    for a in areas {
                        if a.size() <= nodes.len() as u64 {
                            for r in a.r1..=a.r2 {
                                for c in a.c1..=a.c2 {
                                    if let Some(&j) = index.get(&CellId::new(a.sheet, r, c)) {
                                        out.push(j);
                                    }
                                }
                            }
                        } else {
                            for (j, n) in nodes.iter().enumerate() {
                                if a.contains(n.sheet, n.row, n.col) {
                                    out.push(j);
                                }
                            }
                        }
                    }
                }
                out
            })
            .collect()
    }

    fn calc_set(&mut self, dirty: FxHashSet<CellId>) {
        if dirty.is_empty() {
            return;
        }
        let mut nodes: Vec<CellId> = dirty.iter().copied().collect();
        nodes.sort_unstable();
        let index: FxHashMap<CellId, usize> =
            nodes.iter().enumerate().map(|(i, n)| (*n, i)).collect();
        let adj = self.adjacency(&nodes, &index);
        let sccs = tarjan(&adj);
        let mut pending = dirty;
        let mut in_progress = FxHashSet::default();
        for scc in sccs {
            let cyclic = scc.len() > 1 || adj[scc[0]].contains(&scc[0]);
            if cyclic {
                let ids: Vec<CellId> = scc.iter().map(|&i| nodes[i]).collect();
                self.calc_cycle(&ids, &pending);
                for id in ids {
                    pending.remove(&id);
                }
            } else {
                self.calc_one(nodes[scc[0]], &mut pending, &mut in_progress);
            }
        }
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
        let mut ctx = Ctx::new(view, id.sheet, id.row, id.col, &self.config, &self.rng);
        let v = ctx.eval(&f.expr);
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
                self.circular.remove(&top);
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
                    };
                    self.eval_cell(*id, &view)
                };
                let delta = match (&old, &v) {
                    (Value::Number(a), Value::Number(b)) => (a - b).abs(),
                    (Value::Empty, Value::Number(b)) => b.abs(),
                    (a, b) if a == b => 0.0,
                    _ => f64::INFINITY,
                };
                max_delta = max_delta.max(delta);
                self.store(*id, v);
            }
            if max_delta < it.max_change {
                break;
            }
        }
    }

    /// Store a computed value, placing or clearing its spill range.
    fn store(&mut self, id: CellId, v: Value) {
        let old_area = self.spills.get(&id).copied();
        let old_value = self.read(id);
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
                    if self.spill_is_blocked(id, &area) {
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
        let _ = old_value;
    }

    fn clear_spill_silent(&mut self, id: CellId) {
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
}

/// Provider used during recalculation: records reads of cells that are still
/// waiting to be computed.
struct RecalcView<'a> {
    wb: &'a Workbook,
    pending: &'a FxHashSet<CellId>,
    in_progress: &'a FxHashSet<CellId>,
    misses: RefCell<Vec<CellId>>,
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
}
