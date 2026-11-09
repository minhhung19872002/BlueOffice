//! Built-in function registry (ECMA-376 Part 1, §18.17.7 predefined function definitions).

// Every built-in shares the registry signature `fn(&mut Ctx, &[Value]) -> R`,
// so infallible functions still return `Result` and take `&mut Ctx`.
#![allow(clippy::unnecessary_wraps, clippy::trivially_copy_pass_by_ref)]

use std::collections::HashMap;
use std::sync::OnceLock;

use crate::ast::Expr;
use crate::eval::{Ctx, R};
use crate::value::{ErrorKind, Value};

mod datetime;
mod engineering;
mod financial;
mod info;
mod logical;
mod lookup;
mod math;
mod stats;
mod text;

/// Which arguments are scalar (array arguments in those positions are broadcast).
#[derive(Clone, Copy, Debug)]
pub(crate) enum Lift {
    /// No broadcasting; the function handles arrays/references itself.
    None,
    /// Every argument is scalar.
    All,
    /// Bit `i` set → argument `i` is scalar.
    Mask(u32),
}

impl Lift {
    pub(crate) fn is_scalar(self, i: usize) -> bool {
        match self {
            Lift::None => false,
            Lift::All => true,
            Lift::Mask(m) => i < 32 && m & (1 << i) != 0,
        }
    }
}

/// Eager function: receives evaluated arguments (references not dereferenced).
pub(crate) type EagerFn = fn(&mut Ctx, &[Value]) -> R;
/// Lazy function: receives unevaluated argument expressions.
pub(crate) type LazyFn = fn(&mut Ctx, &[Expr]) -> R;

#[derive(Clone, Copy)]
pub(crate) enum FnKind {
    Eager(EagerFn, Lift),
    Lazy(LazyFn),
}

/// A built-in function definition.
#[derive(Clone, Copy)]
pub(crate) struct FnSpec {
    pub name: &'static str,
    pub min: usize,
    pub max: usize,
    pub kind: FnKind,
    pub volatile: bool,
}

/// Maximum argument count for variadic functions.
pub(crate) const VAR: usize = 255;

/// Eager function without broadcasting.
pub(crate) const fn f(name: &'static str, min: usize, max: usize, func: EagerFn) -> FnSpec {
    FnSpec {
        name,
        min,
        max,
        kind: FnKind::Eager(func, Lift::None),
        volatile: false,
    }
}

/// Eager function whose arguments are all scalar (broadcast over arrays).
pub(crate) const fn s(name: &'static str, min: usize, max: usize, func: EagerFn) -> FnSpec {
    FnSpec {
        name,
        min,
        max,
        kind: FnKind::Eager(func, Lift::All),
        volatile: false,
    }
}

/// Eager function with a scalar-argument mask.
pub(crate) const fn m(
    name: &'static str,
    min: usize,
    max: usize,
    mask: u32,
    func: EagerFn,
) -> FnSpec {
    FnSpec {
        name,
        min,
        max,
        kind: FnKind::Eager(func, Lift::Mask(mask)),
        volatile: false,
    }
}

/// Lazy function.
pub(crate) const fn l(name: &'static str, min: usize, max: usize, func: LazyFn) -> FnSpec {
    FnSpec {
        name,
        min,
        max,
        kind: FnKind::Lazy(func),
        volatile: false,
    }
}

impl FnSpec {
    /// Mark as volatile (always recalculated).
    pub(crate) const fn volatile(mut self) -> Self {
        self.volatile = true;
        self
    }
}

fn registry() -> &'static HashMap<&'static str, FnSpec> {
    static REG: OnceLock<HashMap<&'static str, FnSpec>> = OnceLock::new();
    REG.get_or_init(|| {
        let mut map = HashMap::new();
        for group in [
            math::FUNCTIONS,
            logical::FUNCTIONS,
            lookup::FUNCTIONS,
            text::FUNCTIONS,
            datetime::FUNCTIONS,
            stats::FUNCTIONS,
            financial::FUNCTIONS,
            info::FUNCTIONS,
            engineering::FUNCTIONS,
        ] {
            for spec in group {
                map.insert(spec.name, *spec);
            }
        }
        map
    })
}

/// Look up a built-in function by upper-case name.
pub(crate) fn lookup(name: &str) -> Option<FnSpec> {
    registry().get(name).copied()
}

/// Names of all built-in functions, sorted.
pub fn function_names() -> Vec<&'static str> {
    let mut v: Vec<&'static str> = registry().keys().copied().collect();
    v.sort_unstable();
    v
}

/// Whether a built-in function is volatile (`NOW`, `RAND`, `OFFSET`, ...).
pub fn is_volatile_function(name: &str) -> bool {
    lookup(&name.to_uppercase()).is_some_and(|s| s.volatile)
}

// ------------------------------------------------------------ shared helpers

/// Optional numeric argument: missing/empty → `default`.
pub(crate) fn opt_num(ctx: &Ctx, args: &[Value], i: usize, default: f64) -> Result<f64, ErrorKind> {
    match args.get(i) {
        None | Some(Value::Empty) => Ok(default),
        Some(v) => ctx.num(v),
    }
}

/// Optional boolean argument.
pub(crate) fn opt_bool(
    ctx: &Ctx,
    args: &[Value],
    i: usize,
    default: bool,
) -> Result<bool, ErrorKind> {
    match args.get(i) {
        None | Some(Value::Empty) => Ok(default),
        Some(v) => ctx.boolean(v),
    }
}

/// Optional text argument.
pub(crate) fn opt_text(
    ctx: &Ctx,
    args: &[Value],
    i: usize,
    default: &str,
) -> Result<String, ErrorKind> {
    match args.get(i) {
        None | Some(Value::Empty) => Ok(default.to_owned()),
        Some(v) => ctx.text(v),
    }
}

/// How numbers are gathered from aggregate arguments.
#[allow(clippy::struct_excessive_bools)] // independent option flags
#[derive(Clone, Copy, Default)]
pub(crate) struct Gather {
    /// "A" functions: text in ranges counts as 0, booleans as 1/0.
    pub a_mode: bool,
    /// Skip error values instead of propagating them.
    pub skip_errors: bool,
    /// Skip hidden rows.
    pub skip_hidden: bool,
    /// Skip cells holding `SUBTOTAL`/`AGGREGATE` formulas.
    pub skip_nested: bool,
}

/// Collect numbers from aggregate arguments (`SUM`, `AVERAGE`, `MAX`, ...).
///
/// Values from references/arrays: numbers only (text/booleans/blank ignored)
/// unless `a_mode`. Direct arguments: coerced, non-numeric text → `#VALUE!`.
pub(crate) fn gather(ctx: &Ctx, args: &[Value], opt: Gather) -> Result<Vec<f64>, ErrorKind> {
    let mut out = Vec::new();
    let mut err: Option<ErrorKind> = None;
    let mut push = |v: &Value, in_range: bool| {
        if err.is_some() {
            return;
        }
        match v {
            Value::Number(n) => out.push(*n),
            Value::Error(e) => {
                if !opt.skip_errors {
                    err = Some(*e);
                }
            }
            Value::Bool(b) => {
                if !in_range || opt.a_mode {
                    out.push(if *b { 1.0 } else { 0.0 });
                }
            }
            Value::Text(s) => {
                if in_range {
                    if opt.a_mode {
                        out.push(0.0);
                    }
                } else {
                    match crate::value::text_to_number(s, ctx.date_system) {
                        Some(n) => out.push(n),
                        None => err = Some(ErrorKind::Value),
                    }
                }
            }
            Value::Empty if !in_range => {
                out.push(0.0);
            }
            _ => {}
        }
    };
    for a in args {
        match a {
            Value::Ref(_) | Value::Refs(_) if opt.skip_hidden || opt.skip_nested => {
                let areas: Vec<crate::value::Area> = match a {
                    Value::Ref(x) => vec![*x],
                    Value::Refs(l) => l.as_ref().clone(),
                    _ => Vec::new(),
                };
                for area in &areas {
                    ctx.for_each_area(area, &mut |r, c, v| {
                        if opt.skip_hidden && ctx.provider.is_row_hidden(area.sheet, r) {
                            return;
                        }
                        if opt.skip_nested && is_nested_subtotal(ctx, area.sheet, r, c) {
                            return;
                        }
                        push(&v, true);
                    });
                }
            }
            _ => ctx.for_each_value(a, &mut push),
        }
    }
    match err {
        Some(e) => Err(e),
        None => Ok(out),
    }
}

fn is_nested_subtotal(ctx: &Ctx, sheet: usize, r: u32, c: u32) -> bool {
    ctx.provider.cell_formula(sheet, r, c).is_some_and(|f| {
        let u = f.to_uppercase();
        u.contains("SUBTOTAL(") || u.contains("AGGREGATE(")
    })
}

/// Collect numbers with default options.
pub(crate) fn numbers(ctx: &Ctx, args: &[Value]) -> Result<Vec<f64>, ErrorKind> {
    gather(ctx, args, Gather::default())
}

/// All values of an argument (reference, array or scalar) in row-major order.
pub(crate) fn flatten(ctx: &Ctx, v: &Value) -> Vec<Value> {
    let mut out = Vec::new();
    ctx.for_each_value(v, &mut |x, _| out.push(x.clone()));
    out
}

/// Shape (rows, cols) of an argument.
pub(crate) fn shape(ctx: &Ctx, v: &Value) -> (usize, usize) {
    match v {
        Value::Ref(a) => {
            let a = ctx.clip(a);
            (a.rows() as usize, a.cols() as usize)
        }
        Value::Array(a) => (a.rows(), a.cols()),
        _ => (1, 1),
    }
}

/// Numbers from a range for paired statistics: keeps positions, `None` for non-numbers.
pub(crate) fn numeric_cells(ctx: &Ctx, v: &Value) -> Result<Vec<Option<f64>>, ErrorKind> {
    let mut out = Vec::new();
    for x in flatten(ctx, v) {
        match x {
            Value::Number(n) => out.push(Some(n)),
            Value::Error(e) => return Err(e),
            _ => out.push(None),
        }
    }
    Ok(out)
}

/// Return a number value, mapping non-finite results to `#NUM!`.
pub(crate) fn num(n: f64) -> R {
    if n.is_finite() {
        Ok(Value::number(n))
    } else {
        Err(ErrorKind::Num)
    }
}
