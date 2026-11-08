//! Expression evaluator (ECMA-376 Part 1, §18.17.2 operators and §18.17.7 functions).

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use crate::ast::{BinaryOp, Expr, StructItem, StructRef, UnaryOp};
use crate::date::DateSystem;
use crate::functions::{self, FnKind, Lift};
use crate::provider::{CellProvider, TableInfo};
use crate::reference::{RefKind, Reference, SheetRef};
use crate::value::{compare_values, text_to_number, Area, Array, ErrorKind, Lambda, Value};

/// Result type used inside the evaluator and function implementations.
pub(crate) type R = Result<Value, ErrorKind>;

/// Value handed to a [`TextFormatter`]: the first argument of `TEXT` after
/// conversion (ECMA-376 Part 1, §18.17.7 TEXT).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FormatInput<'a> {
    /// A number (numeric text has already been converted).
    Number(f64),
    /// Text that is not a number; only a text section (`@`) applies to it.
    Text(&'a str),
}

/// Number-format renderer used by `TEXT` (ECMA-376 Part 1, §18.8.31 format
/// codes). Returns `None` when the value cannot be shown with the code (e.g. a
/// negative serial with a date code), which `TEXT` reports as `#VALUE!`.
///
/// The host installs its full number-format engine through
/// [`EvalConfig::text_formatter`] (BlueSheet does this in `bo-sheet`); without
/// one, `TEXT` uses the compact built-in renderer.
pub type TextFormatter = fn(FormatInput<'_>, &str, DateSystem) -> Option<String>;

/// Settings for an evaluation.
#[derive(Clone, Copy, Debug, Default)]
pub struct EvalConfig {
    /// Date system for serial numbers.
    pub date_system: DateSystem,
    /// Fixed "now" serial for `NOW`/`TODAY` (tests, reproducible recalcs);
    /// `None` reads the system clock (UTC).
    pub now: Option<f64>,
    /// Number-format renderer for `TEXT`; `None` uses the built-in compact one.
    pub text_formatter: Option<TextFormatter>,
}

impl PartialEq for EvalConfig {
    fn eq(&self, other: &Self) -> bool {
        // Formatters compare by address: equal addresses mean the same function.
        let same_formatter = match (self.text_formatter, other.text_formatter) {
            (Some(a), Some(b)) => std::ptr::fn_addr_eq(a, b),
            (None, None) => true,
            _ => false,
        };
        self.date_system == other.date_system && self.now == other.now && same_formatter
    }
}

const MAX_DEPTH: u32 = 200;
/// Areas larger than this are clipped to the sheet's used extent when iterated.
const CLIP_THRESHOLD: u64 = 1 << 20;
/// Areas at least this large are materialized through [`CellProvider::area_array`].
const AREA_CACHE_MIN: u64 = 256;

/// Evaluate an expression as the formula of cell (`sheet`, `row`, `col`).
///
/// The result is a cell value: references are dereferenced (multi-cell ranges
/// become arrays), a blank result becomes `0`, and a `LAMBDA` value becomes `#CALC!`.
pub fn evaluate(
    expr: &Expr,
    provider: &dyn CellProvider,
    sheet: usize,
    row: u32,
    col: u32,
    config: &EvalConfig,
) -> Value {
    let rng = AtomicU64::new(0x9E37_79B9_7F4A_7C15);
    let mut ctx = Ctx::new(provider, sheet, row, col, config, &rng);
    let v = ctx.eval(expr);
    ctx.finalize(v)
}

/// Evaluation context of one formula.
pub(crate) struct Ctx<'a> {
    pub provider: &'a dyn CellProvider,
    pub sheet: usize,
    pub row: u32,
    pub col: u32,
    pub date_system: DateSystem,
    pub text_formatter: Option<TextFormatter>,
    now: Option<f64>,
    rng: &'a AtomicU64,
    scopes: Vec<(String, Value)>,
    depth: u32,
}

impl<'a> Ctx<'a> {
    pub(crate) fn new(
        provider: &'a dyn CellProvider,
        sheet: usize,
        row: u32,
        col: u32,
        config: &EvalConfig,
        rng: &'a AtomicU64,
    ) -> Self {
        Ctx {
            provider,
            sheet,
            row,
            col,
            date_system: config.date_system,
            text_formatter: config.text_formatter,
            now: config.now,
            rng,
            scopes: Vec::new(),
            depth: 0,
        }
    }

    /// Convert a raw evaluation result into a storable cell value.
    pub(crate) fn finalize(&self, v: Value) -> Value {
        match self.deref(v) {
            Value::Empty => Value::Number(0.0),
            Value::Lambda(_) => Value::Error(ErrorKind::Calc),
            Value::Array(a) => {
                if a.rows() == 1 && a.cols() == 1 {
                    return self.finalize(a.get(0, 0).clone());
                }
                let data = a
                    .data()
                    .iter()
                    .map(|v| match v {
                        Value::Empty => Value::Number(0.0),
                        Value::Lambda(_) | Value::Array(_) | Value::Ref(_) | Value::Refs(_) => {
                            Value::Error(ErrorKind::Value)
                        }
                        v => v.clone(),
                    })
                    .collect();
                Value::Array(Arc::new(Array::new(a.rows(), a.cols(), data)))
            }
            v => v,
        }
    }

    /// Current date-time serial.
    pub(crate) fn now(&self) -> f64 {
        self.now
            .unwrap_or_else(|| crate::date::now_serial(self.date_system))
    }

    /// Next pseudo-random number in `[0, 1)` (xorshift64*).
    pub(crate) fn random(&self) -> f64 {
        let mut x = self.rng.load(Ordering::Relaxed);
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.rng.store(x, Ordering::Relaxed);
        let r = x.wrapping_mul(0x2545_F491_4F6C_DD1D);
        (r >> 11) as f64 / (1u64 << 53) as f64
    }

    // ---------------------------------------------------------------- core

    pub(crate) fn eval(&mut self, e: &Expr) -> Value {
        match e {
            Expr::Number(n) => Value::Number(*n),
            Expr::Text(s) => Value::Text(s.clone()),
            Expr::Bool(b) => Value::Bool(*b),
            Expr::Error(err) => Value::Error(*err),
            Expr::Missing => Value::Empty,
            Expr::Array(rows) => {
                let data: Vec<Vec<Value>> = rows
                    .iter()
                    .map(|r| r.iter().map(|x| self.eval(x)).collect())
                    .collect();
                Array::from_rows(data).map_or(Value::Error(ErrorKind::Value), Value::from)
            }
            Expr::Ref(r) => self.eval_ref(r),
            Expr::Name { sheet, name } => self.eval_name(sheet.as_ref(), name),
            Expr::Structured(s) => self.eval_struct(s).unwrap_or_else(Value::Error),
            Expr::Unary(op, inner) => self.eval_unary(*op, inner),
            Expr::Binary(op, a, b) => self.eval_binary(*op, a, b),
            Expr::Call { name, args } => self.call(name, args).unwrap_or_else(Value::Error),
            Expr::Invoke { callee, args } => {
                let f = self.eval(callee);
                let Value::Lambda(lam) = f else {
                    return match f {
                        Value::Error(e) => Value::Error(e),
                        _ => Value::Error(ErrorKind::Value),
                    };
                };
                let vals: Vec<Value> = args.iter().map(|a| self.eval(a)).collect();
                self.invoke_lambda(&lam, vals).unwrap_or_else(Value::Error)
            }
        }
    }

    /// Resolve a sheet qualifier into an inclusive sheet index range.
    fn resolve_sheets(&self, s: Option<&SheetRef>) -> Result<(usize, usize), ErrorKind> {
        let Some(s) = s else {
            return Ok((self.sheet, self.sheet));
        };
        let first = self.provider.sheet_index(&s.first).ok_or(ErrorKind::Ref)?;
        let last = match &s.last {
            Some(l) => self.provider.sheet_index(l).ok_or(ErrorKind::Ref)?,
            None => first,
        };
        Ok((first.min(last), first.max(last)))
    }

    fn eval_ref(&self, r: &Reference) -> Value {
        let (s1, s2) = match self.resolve_sheets(r.sheet.as_ref()) {
            Ok(x) => x,
            Err(e) => return Value::Error(e),
        };
        let (r1, c1, r2, c2) = r.bounds();
        if s1 == s2 {
            Value::Ref(Area::new(s1, r1, c1, r2, c2))
        } else {
            Value::Refs(Arc::new(
                (s1..=s2).map(|s| Area::new(s, r1, c1, r2, c2)).collect(),
            ))
        }
    }

    fn lookup_var(&self, name: &str) -> Option<Value> {
        self.scopes
            .iter()
            .rev()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.clone())
    }

    fn eval_name(&mut self, sheet: Option<&SheetRef>, name: &str) -> Value {
        if sheet.is_none() {
            if let Some(v) = self.lookup_var(name) {
                return v;
            }
        }
        let scope = match sheet {
            Some(s) => match self.provider.sheet_index(&s.first) {
                Some(i) => i,
                None => return Value::Error(ErrorKind::Ref),
            },
            None => self.sheet,
        };
        let Some(def) = self.provider.defined_name(scope, name) else {
            // A bare table name designates the table's data rows (ECMA-376 Part 1, §18.5.1.2).
            if sheet.is_none() && self.provider.table(name).is_some() {
                let s = StructRef {
                    table: Some(name.to_owned()),
                    items: Vec::new(),
                };
                return self.eval_struct(&s).unwrap_or_else(Value::Error);
            }
            return Value::Error(ErrorKind::Name);
        };
        if self.depth > MAX_DEPTH {
            return Value::Error(ErrorKind::Calc);
        }
        self.depth += 1;
        // Names are evaluated without the caller's LET bindings.
        let saved = std::mem::take(&mut self.scopes);
        let v = self.eval(&def);
        self.scopes = saved;
        self.depth -= 1;
        v
    }

    fn eval_struct(&self, s: &StructRef) -> R {
        struct_area(self.provider, s, self.sheet, self.row, self.col).map(Value::Ref)
    }
}

/// Resolve a structured reference written in cell (`sheet`, `row`, `col`) to its area
/// (ECMA-376 Part 1, §18.5.1 tables; `[#This Row]` / `@` selects the formula's row).
pub(crate) fn struct_area(
    provider: &dyn CellProvider,
    s: &StructRef,
    sheet: usize,
    row: u32,
    col: u32,
) -> Result<Area, ErrorKind> {
    {
        let table: TableInfo = match &s.table {
            Some(name) => provider.table(name).ok_or(ErrorKind::Ref)?,
            None => provider.table_at(sheet, row, col).ok_or(ErrorKind::Ref)?,
        };
        let mut col_lo = table.first_col;
        let mut col_hi = table.last_col();
        let mut cols_given = false;
        let mut rows: Option<(u32, u32)> = None;
        let mut add_rows = |lo: u32, hi: u32| {
            rows = Some(match rows {
                Some((a, b)) => (a.min(lo), b.max(hi)),
                None => (lo, hi),
            });
        };
        for item in &s.items {
            match item {
                StructItem::All => add_rows(table.first_row, table.last_row),
                StructItem::Data => add_rows(table.data_first_row(), table.data_last_row()),
                StructItem::Headers => {
                    if !table.header_row {
                        return Err(ErrorKind::Ref);
                    }
                    add_rows(table.first_row, table.first_row);
                }
                StructItem::Totals => {
                    if !table.totals_row {
                        return Err(ErrorKind::Ref);
                    }
                    add_rows(table.last_row, table.last_row);
                }
                StructItem::ThisRow => {
                    if row < table.data_first_row() || row > table.data_last_row() {
                        return Err(ErrorKind::Value);
                    }
                    add_rows(row, row);
                }
                StructItem::Column(c) => {
                    let i = table.column_index(c).ok_or(ErrorKind::Ref)?;
                    if cols_given {
                        col_lo = col_lo.min(i);
                        col_hi = col_hi.max(i);
                    } else {
                        (col_lo, col_hi) = (i, i);
                        cols_given = true;
                    }
                }
                StructItem::ColumnRange(a, b) => {
                    let i = table.column_index(a).ok_or(ErrorKind::Ref)?;
                    let j = table.column_index(b).ok_or(ErrorKind::Ref)?;
                    let (lo, hi) = (i.min(j), i.max(j));
                    if cols_given {
                        col_lo = col_lo.min(lo);
                        col_hi = col_hi.max(hi);
                    } else {
                        (col_lo, col_hi) = (lo, hi);
                        cols_given = true;
                    }
                }
            }
        }
        let (r1, r2) = rows.unwrap_or((table.data_first_row(), table.data_last_row()));
        if r1 > r2 {
            return Err(ErrorKind::Ref);
        }
        Ok(Area::new(table.sheet, r1, col_lo, r2, col_hi))
    }
}

impl Ctx<'_> {
    fn eval_unary(&mut self, op: UnaryOp, inner: &Expr) -> Value {
        let v = self.eval(inner);
        match op {
            UnaryOp::Plus => v,
            UnaryOp::ImplicitIntersection => self.implicit_intersection(v),
            UnaryOp::SpillRange => match v {
                Value::Ref(a) if a.is_single() => {
                    // Reading the anchor first lets a recalculation compute it before us.
                    let _ = self.provider.cell_value(a.sheet, a.r1, a.c1);
                    self.provider
                        .spill_area(a.sheet, a.r1, a.c1)
                        .map_or(Value::Error(ErrorKind::Ref), Value::Ref)
                }
                Value::Error(e) => Value::Error(e),
                _ => Value::Error(ErrorKind::Ref),
            },
            UnaryOp::Neg | UnaryOp::Percent => {
                let v = self.deref(v);
                self.map_scalar(&v, |ctx, x| match ctx.num(x) {
                    Ok(n) => Value::number(if op == UnaryOp::Neg { -n } else { n / 100.0 }),
                    Err(e) => Value::Error(e),
                })
            }
        }
    }

    /// `@` operator: pick the cell of a range in the formula's row or column.
    pub(crate) fn implicit_intersection(&self, v: Value) -> Value {
        match v {
            Value::Ref(a) => {
                if a.is_single() {
                    return Value::Ref(a);
                }
                if a.c1 == a.c2 && a.sheet == self.sheet && self.row >= a.r1 && self.row <= a.r2 {
                    return Value::Ref(Area::cell(a.sheet, self.row, a.c1));
                }
                if a.r1 == a.r2 && a.sheet == self.sheet && self.col >= a.c1 && self.col <= a.c2 {
                    return Value::Ref(Area::cell(a.sheet, a.r1, self.col));
                }
                Value::Error(ErrorKind::Value)
            }
            Value::Array(a) => a.get(0, 0).clone(),
            Value::Refs(_) => Value::Error(ErrorKind::Value),
            v => v,
        }
    }

    fn eval_binary(&mut self, op: BinaryOp, a: &Expr, b: &Expr) -> Value {
        match op {
            BinaryOp::Range | BinaryOp::Union | BinaryOp::Intersect => {
                let x = self.eval(a);
                let y = self.eval(b);
                ref_op(op, &x, &y)
            }
            _ => {
                let x = self.eval(a);
                let x = self.deref(x);
                let y = self.eval(b);
                let y = self.deref(y);
                self.broadcast2(&x, &y, |ctx, x, y| ctx.scalar_binary(op, x, y))
            }
        }
    }

    fn scalar_binary(&self, op: BinaryOp, x: &Value, y: &Value) -> Value {
        if let Value::Error(e) = x {
            return Value::Error(*e);
        }
        if let Value::Error(e) = y {
            return Value::Error(*e);
        }
        match op {
            BinaryOp::Concat => match (self.text(x), self.text(y)) {
                (Ok(a), Ok(b)) => Value::Text(a + &b),
                (Err(e), _) | (_, Err(e)) => Value::Error(e),
            },
            BinaryOp::Eq
            | BinaryOp::Ne
            | BinaryOp::Lt
            | BinaryOp::Le
            | BinaryOp::Gt
            | BinaryOp::Ge => {
                let ord = compare_values(x, y);
                Value::Bool(match op {
                    BinaryOp::Eq => ord.is_eq(),
                    BinaryOp::Ne => ord.is_ne(),
                    BinaryOp::Lt => ord.is_lt(),
                    BinaryOp::Le => ord.is_le(),
                    BinaryOp::Gt => ord.is_gt(),
                    _ => ord.is_ge(),
                })
            }
            _ => {
                let (a, b) = match (self.num(x), self.num(y)) {
                    (Ok(a), Ok(b)) => (a, b),
                    (Err(e), _) | (_, Err(e)) => return Value::Error(e),
                };
                match op {
                    BinaryOp::Add => Value::number(a + b),
                    BinaryOp::Sub => Value::number(a - b),
                    BinaryOp::Mul => Value::number(a * b),
                    BinaryOp::Div => {
                        if b == 0.0 {
                            Value::Error(ErrorKind::Div0)
                        } else {
                            Value::number(a / b)
                        }
                    }
                    _ => power(a, b),
                }
            }
        }
    }

    // ------------------------------------------------------------ helpers

    /// Apply `f` to each element of an array, or to a scalar.
    pub(crate) fn map_scalar(
        &mut self,
        v: &Value,
        f: impl Fn(&mut Self, &Value) -> Value,
    ) -> Value {
        match v {
            Value::Array(a) => {
                let data = a.data().iter().map(|x| f(self, x)).collect();
                Value::from(Array::new(a.rows(), a.cols(), data))
            }
            x => f(self, x),
        }
    }

    /// Element-wise application with broadcasting of single rows/columns;
    /// positions outside both operands yield `#N/A`.
    pub(crate) fn broadcast2(
        &mut self,
        x: &Value,
        y: &Value,
        f: impl Fn(&mut Self, &Value, &Value) -> Value,
    ) -> Value {
        match (x, y) {
            (Value::Array(_), _) | (_, Value::Array(_)) => {
                let dims = |v: &Value| match v {
                    Value::Array(a) => (a.rows(), a.cols()),
                    _ => (1, 1),
                };
                let (xr, xc) = dims(x);
                let (yr, yc) = dims(y);
                let (rows, cols) = (xr.max(yr), xc.max(yc));
                let mut data = Vec::with_capacity(rows * cols);
                for r in 0..rows {
                    for c in 0..cols {
                        let a = elem(x, r, c);
                        let b = elem(y, r, c);
                        data.push(match (a, b) {
                            (Some(a), Some(b)) => f(self, a, b),
                            _ => Value::Error(ErrorKind::NA),
                        });
                    }
                }
                Value::from(Array::new(rows, cols, data))
            }
            _ => f(self, x, y),
        }
    }

    /// Dereference: single-cell reference → its value, multi-cell → array.
    pub(crate) fn deref(&self, v: Value) -> Value {
        match v {
            Value::Ref(a) => self.area_value(&a),
            Value::Refs(list) => {
                if list.len() == 1 {
                    self.area_value(&list[0])
                } else {
                    Value::Error(ErrorKind::Value)
                }
            }
            v => v,
        }
    }

    /// Value of an area: a scalar for one cell, an array otherwise.
    pub(crate) fn area_value(&self, a: &Area) -> Value {
        if a.is_single() {
            return self.provider.cell_value(a.sheet, a.r1, a.c1);
        }
        let a = self.clip(a);
        if a.size() >= AREA_CACHE_MIN {
            if let Some(arr) = self.provider.area_array(&a) {
                return Value::Array(arr);
            }
        }
        let mut data = Vec::with_capacity(a.size() as usize);
        for r in a.r1..=a.r2 {
            for c in a.c1..=a.c2 {
                data.push(self.provider.cell_value(a.sheet, r, c));
            }
        }
        Value::from(Array::new(a.rows() as usize, a.cols() as usize, data))
    }

    /// Clip very large areas (whole rows/columns) to the used extent.
    pub(crate) fn clip(&self, a: &Area) -> Area {
        if a.size() <= CLIP_THRESHOLD {
            return *a;
        }
        let (rows, cols) = self.provider.used_extent(a.sheet);
        let r2 = a.r2.min(rows.max(1) - 1).max(a.r1);
        let c2 = a.c2.min(cols.max(1) - 1).max(a.c1);
        Area { r2, c2, ..*a }
    }

    /// Value as an array (scalars become 1×1).
    pub(crate) fn to_array(&self, v: Value) -> Result<Arc<Array>, ErrorKind> {
        match self.deref(v) {
            Value::Array(a) => Ok(a),
            Value::Error(e) => Err(e),
            Value::Lambda(_) | Value::Refs(_) => Err(ErrorKind::Value),
            v => Ok(Arc::new(Array::new(1, 1, vec![v]))),
        }
    }

    /// Scalar value (dereferenced; arrays give their top-left element).
    pub(crate) fn scalar(&self, v: &Value) -> Value {
        match v {
            Value::Ref(a) => {
                if a.is_single() {
                    self.provider.cell_value(a.sheet, a.r1, a.c1)
                } else {
                    // Implicit intersection with the formula cell.
                    match self.implicit_intersection(Value::Ref(*a)) {
                        Value::Ref(c) => self.provider.cell_value(c.sheet, c.r1, c.c1),
                        other => other,
                    }
                }
            }
            Value::Refs(_) => Value::Error(ErrorKind::Value),
            Value::Array(a) => a.get(0, 0).clone(),
            v => v.clone(),
        }
    }

    /// Coerce to number (date strings use the context's date system).
    pub(crate) fn num(&self, v: &Value) -> Result<f64, ErrorKind> {
        match self.scalar(v) {
            Value::Text(s) => text_to_number(&s, self.date_system).ok_or(ErrorKind::Value),
            other => other.as_number(),
        }
    }

    /// Coerce to text.
    pub(crate) fn text(&self, v: &Value) -> Result<String, ErrorKind> {
        self.scalar(v).as_text()
    }

    /// Coerce to boolean.
    pub(crate) fn boolean(&self, v: &Value) -> Result<bool, ErrorKind> {
        self.scalar(v).as_bool()
    }

    /// Coerce to an integer by truncation.
    pub(crate) fn int(&self, v: &Value) -> Result<i64, ErrorKind> {
        let n = self.num(v)?;
        if n.abs() > 9.0e15 {
            return Err(ErrorKind::Num);
        }
        Ok(n.trunc() as i64)
    }

    /// Visit every value of an argument. `in_range` is true for values coming
    /// from references or arrays (where text and booleans are usually skipped).
    pub(crate) fn for_each_value(&self, v: &Value, f: &mut dyn FnMut(&Value, bool)) {
        match v {
            Value::Ref(a) => self.for_each_area(a, &mut |_, _, x| f(&x, true)),
            Value::Refs(list) => {
                for a in list.iter() {
                    self.for_each_area(a, &mut |_, _, x| f(&x, true));
                }
            }
            Value::Array(arr) => arr.data().iter().for_each(|x| f(x, true)),
            x => f(x, false),
        }
    }

    /// Visit every cell of an area (clipped) with its row/column.
    pub(crate) fn for_each_area(&self, a: &Area, f: &mut dyn FnMut(u32, u32, Value)) {
        let a = self.clip(a);
        for r in a.r1..=a.r2 {
            for c in a.c1..=a.c2 {
                f(r, c, self.provider.cell_value(a.sheet, r, c));
            }
        }
    }

    // ---------------------------------------------------------- functions

    fn call(&mut self, name: &str, args: &[Expr]) -> R {
        if let Some(Value::Lambda(lam)) = self.lookup_var(name) {
            let vals: Vec<Value> = args.iter().map(|a| self.eval(a)).collect();
            return self.invoke_lambda(&lam, vals);
        }
        let Some(spec) = functions::lookup(name) else {
            // A defined name holding a LAMBDA can be called like a function.
            if let Some(def) = self.provider.defined_name(self.sheet, name) {
                if let Value::Lambda(lam) = self.eval(&def) {
                    let vals: Vec<Value> = args.iter().map(|a| self.eval(a)).collect();
                    return self.invoke_lambda(&lam, vals);
                }
            }
            return Err(ErrorKind::Name);
        };
        if args.len() < spec.min || args.len() > spec.max {
            return Err(ErrorKind::Value);
        }
        if self.depth > MAX_DEPTH {
            return Err(ErrorKind::Calc);
        }
        self.depth += 1;
        let out = match spec.kind {
            FnKind::Lazy(f) => f(self, args),
            FnKind::Eager(f, lift) => {
                let vals: Vec<Value> = args.iter().map(|a| self.eval(a)).collect();
                self.apply_lifted(f, vals, lift)
            }
        };
        self.depth -= 1;
        out
    }

    /// Call `f`, broadcasting over array arguments in scalar positions.
    pub(crate) fn apply_lifted(
        &mut self,
        f: fn(&mut Ctx, &[Value]) -> R,
        mut vals: Vec<Value>,
        lift: Lift,
    ) -> R {
        let mut rows = 0usize;
        let mut cols = 0usize;
        for (i, v) in vals.iter_mut().enumerate() {
            if !lift.is_scalar(i) {
                continue;
            }
            let d = self.deref(std::mem::take(v));
            *v = match d {
                Value::Array(a) if a.rows() == 1 && a.cols() == 1 => a.get(0, 0).clone(),
                other => other,
            };
            if let Value::Array(a) = v {
                rows = rows.max(a.rows());
                cols = cols.max(a.cols());
            }
        }
        if rows == 0 {
            return f(self, &vals);
        }
        let mut data = Vec::with_capacity(rows * cols);
        let mut call_args = vals.clone();
        for r in 0..rows {
            for c in 0..cols {
                let mut missing = false;
                for (i, v) in vals.iter().enumerate() {
                    if lift.is_scalar(i) {
                        match elem(v, r, c) {
                            Some(x) => call_args[i] = x.clone(),
                            None => missing = true,
                        }
                    }
                }
                data.push(if missing {
                    Value::Error(ErrorKind::NA)
                } else {
                    match f(self, &call_args) {
                        Ok(Value::Array(a)) => a.get(0, 0).clone(),
                        Ok(v) => v,
                        Err(e) => Value::Error(e),
                    }
                });
            }
        }
        Ok(Value::from(Array::new(rows, cols, data)))
    }

    /// Invoke a `LAMBDA` value with evaluated arguments.
    pub(crate) fn invoke_lambda(&mut self, lam: &Lambda, args: Vec<Value>) -> R {
        if args.len() != lam.params.len() {
            return Err(ErrorKind::Value);
        }
        if self.depth > MAX_DEPTH {
            return Err(ErrorKind::Num);
        }
        self.depth += 1;
        let saved = std::mem::replace(&mut self.scopes, lam.env.clone());
        for (p, v) in lam.params.iter().zip(args) {
            self.scopes.push((p.clone(), v));
        }
        let v = self.eval(&lam.body);
        self.scopes = saved;
        self.depth -= 1;
        Ok(v)
    }

    /// `LET(name1, value1, ..., body)`.
    pub(crate) fn eval_let(&mut self, args: &[Expr]) -> R {
        if args.len() < 3 || args.len().is_multiple_of(2) {
            return Err(ErrorKind::Value);
        }
        let saved = self.scopes.len();
        for pair in args[..args.len() - 1].chunks(2) {
            let Expr::Name { sheet: None, name } = &pair[0] else {
                self.scopes.truncate(saved);
                return Err(ErrorKind::Value);
            };
            let v = self.eval(&pair[1]);
            self.scopes.push((name.to_uppercase(), v));
        }
        let v = self.eval(&args[args.len() - 1]);
        self.scopes.truncate(saved);
        Ok(v)
    }

    /// `LAMBDA(param1, ..., body)`.
    pub(crate) fn make_lambda(&mut self, args: &[Expr]) -> R {
        let Some((body, params)) = args.split_last() else {
            return Err(ErrorKind::Value);
        };
        let mut names = Vec::with_capacity(params.len());
        for p in params {
            let Expr::Name { sheet: None, name } = p else {
                return Err(ErrorKind::Value);
            };
            names.push(name.to_uppercase());
        }
        Ok(Value::Lambda(Arc::new(Lambda {
            params: names,
            body: body.clone(),
            env: self.scopes.clone(),
        })))
    }
}

/// Element (r, c) of an array with broadcasting, or the scalar itself.
pub(crate) fn elem(v: &Value, r: usize, c: usize) -> Option<&Value> {
    match v {
        Value::Array(a) => a.get_broadcast(r, c),
        x => Some(x),
    }
}

fn power(a: f64, b: f64) -> Value {
    if a == 0.0 && b == 0.0 {
        return Value::Error(ErrorKind::Num);
    }
    if a == 0.0 && b < 0.0 {
        return Value::Error(ErrorKind::Div0);
    }
    Value::number(a.powf(b))
}

fn areas_of(v: &Value) -> Result<Vec<Area>, ErrorKind> {
    match v {
        Value::Ref(a) => Ok(vec![*a]),
        Value::Refs(l) => Ok(l.as_ref().clone()),
        Value::Error(e) => Err(*e),
        _ => Err(ErrorKind::Value),
    }
}

/// Reference operators `:`, `,` and space (ECMA-376 Part 1, §18.17.2.2).
fn ref_op(op: BinaryOp, x: &Value, y: &Value) -> Value {
    let (xa, ya) = match (areas_of(x), areas_of(y)) {
        (Ok(a), Ok(b)) => (a, b),
        (Err(e), _) | (_, Err(e)) => return Value::Error(e),
    };
    match op {
        BinaryOp::Range => {
            let all: Vec<&Area> = xa.iter().chain(ya.iter()).collect();
            let sheet = all[0].sheet;
            if all.iter().any(|a| a.sheet != sheet) {
                return Value::Error(ErrorKind::Value);
            }
            let r1 = all.iter().map(|a| a.r1).min().unwrap_or(0);
            let c1 = all.iter().map(|a| a.c1).min().unwrap_or(0);
            let r2 = all.iter().map(|a| a.r2).max().unwrap_or(0);
            let c2 = all.iter().map(|a| a.c2).max().unwrap_or(0);
            Value::Ref(Area::new(sheet, r1, c1, r2, c2))
        }
        BinaryOp::Union => {
            let mut all = xa;
            all.extend(ya);
            Value::Refs(Arc::new(all))
        }
        _ => {
            let mut out = Vec::new();
            for a in &xa {
                for b in &ya {
                    if let Some(i) = a.intersect(b) {
                        out.push(i);
                    }
                }
            }
            match out.len() {
                0 => Value::Error(ErrorKind::Null),
                1 => Value::Ref(out[0]),
                _ => Value::Refs(Arc::new(out)),
            }
        }
    }
}

/// Resolve a reference expression's static areas (for dependency tracking).
pub(crate) fn static_areas(r: &Reference, provider: &dyn CellProvider, sheet: usize) -> Vec<Area> {
    let resolve = |name: &str| provider.sheet_index(name);
    let (s1, s2) = match &r.sheet {
        None => (sheet, sheet),
        Some(s) => match (resolve(&s.first), s.last.as_deref().map(resolve)) {
            (Some(a), None) => (a, a),
            (Some(a), Some(Some(b))) => (a.min(b), a.max(b)),
            _ => return Vec::new(),
        },
    };
    let (r1, c1, r2, c2) = r.bounds();
    let _ = RefKind::Cell;
    (s1..=s2).map(|s| Area::new(s, r1, c1, r2, c2)).collect()
}
