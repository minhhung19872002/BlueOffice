//! Lookup & reference functions and dynamic-array helpers (ECMA-376 Part 1, §18.17.7).

use std::cmp::Ordering;
use std::sync::Arc;

use super::{f, l, opt_bool, opt_num, FnSpec, VAR};
use crate::ast::Expr;
use crate::criteria::{has_wildcards, wildcard_match};
use crate::eval::{Ctx, R};
use crate::parser::{parse_with, ParseOptions, RefStyle};
use crate::reference::{col_to_letters, CellAddr, MAX_COLS, MAX_ROWS};
use crate::value::{compare_values, Area, Array, ErrorKind, Value};

pub(super) const FUNCTIONS: &[FnSpec] = &[
    f("VLOOKUP", 3, 4, vlookup),
    f("HLOOKUP", 3, 4, hlookup),
    f("LOOKUP", 2, 3, lookup),
    f("INDEX", 2, 4, index),
    f("MATCH", 2, 3, match_fn),
    f("XLOOKUP", 3, 6, xlookup),
    f("XMATCH", 2, 4, xmatch),
    l("CHOOSE", 2, VAR, choose),
    f("OFFSET", 3, 5, offset).volatile(),
    f("INDIRECT", 1, 2, indirect).volatile(),
    f("ROW", 0, 1, row),
    f("ROWS", 1, 1, rows),
    f("COLUMN", 0, 1, column),
    f("COLUMNS", 1, 1, columns),
    super::s("ADDRESS", 2, 5, address),
    f("AREAS", 1, 1, areas),
    f("FILTER", 2, 3, filter),
    f("SORT", 1, 4, sort),
    f("SORTBY", 2, VAR, sortby),
    f("UNIQUE", 1, 3, unique),
    f("TRANSPOSE", 1, 1, transpose),
    f("HSTACK", 1, VAR, hstack),
    f("VSTACK", 1, VAR, vstack),
    f("CHOOSECOLS", 2, VAR, choosecols),
    f("CHOOSEROWS", 2, VAR, chooserows),
    f("TAKE", 2, 3, take),
    f("DROP", 2, 3, drop),
];

// ------------------------------------------------------------ matching core

/// Type class used to restrict approximate matches to comparable values.
fn class(v: &Value) -> u8 {
    match v {
        Value::Number(_) => 0,
        Value::Text(_) => 1,
        Value::Bool(_) => 2,
        Value::Empty => 3,
        _ => 4,
    }
}

fn same_value(a: &Value, b: &Value) -> bool {
    class(a) == class(b) && compare_values(a, b) == Ordering::Equal
}

/// Exact match with optional wildcards for text lookup values.
fn exact_eq(needle: &Value, v: &Value, wildcards: bool) -> bool {
    match (needle, v) {
        (Value::Text(p), Value::Text(t)) if wildcards && has_wildcards(p) => wildcard_match(p, t),
        _ => same_value(needle, v),
    }
}

/// Position of an exact match.
fn find_exact(needle: &Value, vals: &[Value], wildcards: bool, reverse: bool) -> Option<usize> {
    if reverse {
        vals.iter().rposition(|v| exact_eq(needle, v, wildcards))
    } else {
        vals.iter().position(|v| exact_eq(needle, v, wildcards))
    }
}

/// Approximate match in ascending data: last value ≤ needle (same type class),
/// stopping at the first larger value.
fn find_le_sorted(needle: &Value, vals: &[Value]) -> Option<usize> {
    let mut best = None;
    for (i, v) in vals.iter().enumerate() {
        if class(v) != class(needle) {
            continue;
        }
        match compare_values(v, needle) {
            Ordering::Greater => break,
            _ => best = Some(i),
        }
    }
    best
}

/// Approximate match in descending data: last value ≥ needle.
fn find_ge_sorted(needle: &Value, vals: &[Value]) -> Option<usize> {
    let mut best = None;
    for (i, v) in vals.iter().enumerate() {
        if class(v) != class(needle) {
            continue;
        }
        match compare_values(v, needle) {
            Ordering::Less => break,
            _ => best = Some(i),
        }
    }
    best
}

/// Closest match without sort assumption (XLOOKUP/XMATCH modes -1 / 1).
fn find_nearest(needle: &Value, vals: &[Value], larger: bool, reverse: bool) -> Option<usize> {
    if let Some(i) = find_exact(needle, vals, false, reverse) {
        return Some(i);
    }
    let mut best: Option<usize> = None;
    let order: Box<dyn Iterator<Item = usize>> = if reverse {
        Box::new((0..vals.len()).rev())
    } else {
        Box::new(0..vals.len())
    };
    for i in order {
        let v = &vals[i];
        if class(v) != class(needle) {
            continue;
        }
        let ord = compare_values(v, needle);
        let ok = if larger { ord == Ordering::Greater } else { ord == Ordering::Less };
        if !ok {
            continue;
        }
        let better = match best {
            None => true,
            Some(b) => {
                let c = compare_values(v, &vals[b]);
                if larger {
                    c == Ordering::Less
                } else {
                    c == Ordering::Greater
                }
            }
        };
        if better {
            best = Some(i);
        }
    }
    best
}

fn lookup_value(ctx: &Ctx, v: &Value) -> Result<Value, ErrorKind> {
    match ctx.scalar(v) {
        Value::Error(e) => Err(e),
        v => Ok(v),
    }
}

/// A 2-D source (reference or array) with element access.
struct Grid {
    area: Option<Area>,
    arr: Arc<Array>,
}

impl Grid {
    fn new(ctx: &Ctx, v: &Value) -> Result<Grid, ErrorKind> {
        let area = match v {
            Value::Ref(a) => Some(ctx.clip(a)),
            Value::Refs(_) => return Err(ErrorKind::Value),
            Value::Error(e) => return Err(*e),
            _ => None,
        };
        let arr = match area {
            Some(a) => ctx.to_array(Value::Ref(a))?,
            None => ctx.to_array(v.clone())?,
        };
        Ok(Grid { area, arr })
    }

    fn rows(&self) -> usize {
        self.arr.rows()
    }

    fn cols(&self) -> usize {
        self.arr.cols()
    }

    fn column(&self, c: usize) -> Vec<Value> {
        (0..self.rows()).map(|r| self.arr.get(r, c).clone()).collect()
    }

    fn row(&self, r: usize) -> Vec<Value> {
        self.arr.row_slice(r).to_vec()
    }

    /// Sub-block as a reference (when backed by one) or an array.
    fn block(&self, r1: usize, c1: usize, r2: usize, c2: usize) -> Value {
        if let Some(a) = self.area {
            return Value::Ref(Area::new(
                a.sheet,
                a.r1 + r1 as u32,
                a.c1 + c1 as u32,
                a.r1 + r2 as u32,
                a.c1 + c2 as u32,
            ));
        }
        let mut data = Vec::new();
        for r in r1..=r2 {
            for c in c1..=c2 {
                data.push(self.arr.get(r, c).clone());
            }
        }
        if data.len() == 1 {
            return data.pop().unwrap_or_default();
        }
        Array::new(r2 - r1 + 1, c2 - c1 + 1, data).into()
    }

    /// Values of a 1-D vector, or `None` when 2-D.
    fn vector(&self) -> Option<Vec<Value>> {
        if self.cols() == 1 {
            Some(self.column(0))
        } else if self.rows() == 1 {
            Some(self.row(0))
        } else {
            None
        }
    }
}

// ------------------------------------------------------------ classic lookups

fn v_or_h_lookup(ctx: &Ctx, args: &[Value], vertical: bool) -> R {
    let needle = lookup_value(ctx, &args[0])?;
    let grid = Grid::new(ctx, &args[1])?;
    let idx = ctx.num(&args[2])?.trunc();
    let approx = super::opt_bool(ctx, args, 3, true)?;
    if idx < 1.0 {
        return Err(ErrorKind::Value);
    }
    let idx = idx as usize - 1;
    let (keys, limit) = if vertical {
        (grid.column(0), grid.cols())
    } else {
        (grid.row(0), grid.rows())
    };
    if idx >= limit {
        return Err(ErrorKind::Ref);
    }
    let pos = if approx {
        find_le_sorted(&needle, &keys)
    } else {
        find_exact(&needle, &keys, true, false)
    }
    .ok_or(ErrorKind::NA)?;
    Ok(if vertical {
        grid.arr.get(pos, idx).clone()
    } else {
        grid.arr.get(idx, pos).clone()
    })
}

fn vlookup(ctx: &mut Ctx, args: &[Value]) -> R {
    v_or_h_lookup(ctx, args, true)
}

fn hlookup(ctx: &mut Ctx, args: &[Value]) -> R {
    v_or_h_lookup(ctx, args, false)
}

fn lookup(ctx: &mut Ctx, args: &[Value]) -> R {
    let needle = lookup_value(ctx, &args[0])?;
    let grid = Grid::new(ctx, &args[1])?;
    let (keys, results) = match args.get(2) {
        Some(r) => {
            let keys = grid.vector().ok_or(ErrorKind::NA)?;
            let rg = Grid::new(ctx, r)?;
            (keys, rg.vector().ok_or(ErrorKind::NA)?)
        }
        None => {
            if grid.cols() > grid.rows() {
                (grid.row(0), grid.row(grid.rows() - 1))
            } else {
                (grid.column(0), grid.column(grid.cols() - 1))
            }
        }
    };
    let pos = find_le_sorted(&needle, &keys).ok_or(ErrorKind::NA)?;
    results.get(pos).cloned().ok_or(ErrorKind::NA)
}

fn match_fn(ctx: &mut Ctx, args: &[Value]) -> R {
    let needle = lookup_value(ctx, &args[0])?;
    let grid = Grid::new(ctx, &args[1])?;
    let kind = opt_num(ctx, args, 2, 1.0)?;
    let vals = grid.vector().ok_or(ErrorKind::NA)?;
    let pos = if kind == 0.0 {
        find_exact(&needle, &vals, true, false)
    } else if kind > 0.0 {
        find_le_sorted(&needle, &vals)
    } else {
        find_ge_sorted(&needle, &vals)
    };
    pos.map(|p| Value::Number((p + 1) as f64)).ok_or(ErrorKind::NA)
}

/// XLOOKUP/XMATCH search over a vector.
fn x_find(needle: &Value, vals: &[Value], match_mode: i64, search_mode: i64) -> Result<Option<usize>, ErrorKind> {
    let reverse = match search_mode {
        1 | 2 => false,
        -1 | -2 => true,
        _ => return Err(ErrorKind::Value),
    };
    Ok(match match_mode {
        0 => find_exact(needle, vals, false, reverse),
        2 => find_exact(needle, vals, true, reverse),
        -1 => find_nearest(needle, vals, false, reverse),
        1 => find_nearest(needle, vals, true, reverse),
        _ => return Err(ErrorKind::Value),
    })
}

fn xlookup(ctx: &mut Ctx, args: &[Value]) -> R {
    let needle = lookup_value(ctx, &args[0])?;
    let keys_grid = Grid::new(ctx, &args[1])?;
    let ret = Grid::new(ctx, &args[2])?;
    let match_mode = opt_num(ctx, args, 4, 0.0)?.trunc() as i64;
    let search_mode = opt_num(ctx, args, 5, 1.0)?.trunc() as i64;
    let vertical = keys_grid.cols() == 1;
    let keys = keys_grid.vector().ok_or(ErrorKind::Value)?;
    if (vertical && ret.rows() != keys.len()) || (!vertical && ret.cols() != keys.len()) {
        return Err(ErrorKind::Value);
    }
    match x_find(&needle, &keys, match_mode, search_mode)? {
        Some(p) => Ok(if vertical {
            ret.block(p, 0, p, ret.cols() - 1)
        } else {
            ret.block(0, p, ret.rows() - 1, p)
        }),
        None => match args.get(3) {
            Some(v) if !matches!(v, Value::Empty) => Ok(v.clone()),
            _ => Err(ErrorKind::NA),
        },
    }
}

fn xmatch(ctx: &mut Ctx, args: &[Value]) -> R {
    let needle = lookup_value(ctx, &args[0])?;
    let grid = Grid::new(ctx, &args[1])?;
    let match_mode = opt_num(ctx, args, 2, 0.0)?.trunc() as i64;
    let search_mode = opt_num(ctx, args, 3, 1.0)?.trunc() as i64;
    let vals = grid.vector().ok_or(ErrorKind::Value)?;
    x_find(&needle, &vals, match_mode, search_mode)?
        .map(|p| Value::Number((p + 1) as f64))
        .ok_or(ErrorKind::NA)
}

fn index(ctx: &mut Ctx, args: &[Value]) -> R {
    let source = match &args[0] {
        Value::Refs(list) => {
            let n = opt_num(ctx, args, 3, 1.0)?.trunc();
            if n < 1.0 || n as usize > list.len() {
                return Err(ErrorKind::Ref);
            }
            Value::Ref(list[n as usize - 1])
        }
        v => v.clone(),
    };
    let grid = Grid::new(ctx, &source)?;
    let r = opt_num(ctx, args, 1, 0.0)?.trunc();
    let c = opt_num(ctx, args, 2, 0.0)?.trunc();
    if r < 0.0 || c < 0.0 {
        return Err(ErrorKind::Value);
    }
    let (r, c) = (r as usize, c as usize);
    let (rows, cols) = (grid.rows(), grid.cols());
    // A single index into a 1-D vector selects along its length.
    let (r, c) = if args.len() == 2 && rows == 1 && cols > 1 { (1, r) } else { (r, c) };
    if r > rows || c > cols {
        return Err(ErrorKind::Ref);
    }
    let (r1, r2) = if r == 0 { (0, rows - 1) } else { (r - 1, r - 1) };
    let (c1, c2) = if c == 0 { (0, cols - 1) } else { (c - 1, c - 1) };
    Ok(grid.block(r1, c1, r2, c2))
}

fn choose(ctx: &mut Ctx, args: &[Expr]) -> R {
    let i = ctx.eval(&args[0]);
    let i = ctx.deref(i);
    if let Value::Array(a) = &i {
        let options: Vec<Value> = args[1..]
            .iter()
            .map(|e| {
                let v = ctx.eval(e);
                ctx.deref(v)
            })
            .collect();
        let data = a
            .data()
            .iter()
            .enumerate()
            .map(|(k, x)| match ctx.num(x) {
                Ok(n) if n >= 1.0 && (n as usize) <= options.len() => {
                    crate::eval::elem(&options[n as usize - 1], k / a.cols(), k % a.cols())
                        .cloned()
                        .unwrap_or(Value::Error(ErrorKind::NA))
                }
                Ok(_) => Value::Error(ErrorKind::Value),
                Err(e) => Value::Error(e),
            })
            .collect();
        return Ok(Array::new(a.rows(), a.cols(), data).into());
    }
    let n = ctx.num(&i)?.trunc();
    if n < 1.0 || n as usize >= args.len() {
        return Err(ErrorKind::Value);
    }
    Ok(ctx.eval(&args[n as usize]))
}

fn offset(ctx: &mut Ctx, args: &[Value]) -> R {
    let Value::Ref(a) = &args[0] else {
        return Err(args[0].error().unwrap_or(ErrorKind::Value));
    };
    let dr = ctx.num(&args[1])?.trunc() as i64;
    let dc = ctx.num(&args[2])?.trunc() as i64;
    let h = match args.get(3) {
        None | Some(Value::Empty) => i64::from(a.rows()),
        Some(v) => ctx.num(v)?.trunc() as i64,
    };
    let w = match args.get(4) {
        None | Some(Value::Empty) => i64::from(a.cols()),
        Some(v) => ctx.num(v)?.trunc() as i64,
    };
    if h == 0 || w == 0 {
        return Err(ErrorKind::Ref);
    }
    let r1 = i64::from(a.r1) + dr;
    let c1 = i64::from(a.c1) + dc;
    // Negative height/width extend upward/leftward.
    let (r1, r2) = if h > 0 { (r1, r1 + h - 1) } else { (r1 + h + 1, r1) };
    let (c1, c2) = if w > 0 { (c1, c1 + w - 1) } else { (c1 + w + 1, c1) };
    if r1 < 0 || c1 < 0 || r2 >= i64::from(MAX_ROWS) || c2 >= i64::from(MAX_COLS) {
        return Err(ErrorKind::Ref);
    }
    Ok(Value::Ref(Area::new(a.sheet, r1 as u32, c1 as u32, r2 as u32, c2 as u32)))
}

fn indirect(ctx: &mut Ctx, args: &[Value]) -> R {
    let text = ctx.text(&args[0])?;
    let a1 = opt_bool(ctx, args, 1, true)?;
    let opts = ParseOptions {
        style: if a1 { RefStyle::A1 } else { RefStyle::R1C1 },
        origin_row: ctx.row,
        origin_col: ctx.col,
    };
    let expr = parse_with(text.trim(), &opts).map_err(|_| ErrorKind::Ref)?;
    if !matches!(expr, Expr::Ref(_) | Expr::Name { .. } | Expr::Binary(..) | Expr::Structured(_)) {
        return Err(ErrorKind::Ref);
    }
    match ctx.eval(&expr) {
        v @ (Value::Ref(_) | Value::Refs(_)) => Ok(v),
        Value::Error(e) => Err(e),
        _ => Err(ErrorKind::Ref),
    }
}

fn ref_area(v: &Value) -> Result<Area, ErrorKind> {
    match v {
        Value::Ref(a) => Ok(*a),
        Value::Refs(l) => l.first().copied().ok_or(ErrorKind::Ref),
        Value::Error(e) => Err(*e),
        _ => Err(ErrorKind::Value),
    }
}

fn row(ctx: &mut Ctx, args: &[Value]) -> R {
    let a = match args.first() {
        None | Some(Value::Empty) => return Ok(Value::Number(f64::from(ctx.row + 1))),
        Some(v) => ctx.clip(&ref_area(v)?),
    };
    if a.r1 == a.r2 {
        return Ok(Value::Number(f64::from(a.r1 + 1)));
    }
    let data = (a.r1..=a.r2).map(|r| Value::Number(f64::from(r + 1))).collect();
    Ok(Array::new(a.rows() as usize, 1, data).into())
}

fn column(ctx: &mut Ctx, args: &[Value]) -> R {
    let a = match args.first() {
        None | Some(Value::Empty) => return Ok(Value::Number(f64::from(ctx.col + 1))),
        Some(v) => ctx.clip(&ref_area(v)?),
    };
    if a.c1 == a.c2 {
        return Ok(Value::Number(f64::from(a.c1 + 1)));
    }
    let data = (a.c1..=a.c2).map(|c| Value::Number(f64::from(c + 1))).collect();
    Ok(Array::new(1, a.cols() as usize, data).into())
}

fn rows(_ctx: &mut Ctx, args: &[Value]) -> R {
    match &args[0] {
        Value::Ref(a) => Ok(Value::Number(f64::from(a.rows()))),
        Value::Array(a) => Ok(Value::Number(a.rows() as f64)),
        Value::Error(e) => Err(*e),
        Value::Refs(_) => Err(ErrorKind::Ref),
        _ => Ok(Value::Number(1.0)),
    }
}

fn columns(_ctx: &mut Ctx, args: &[Value]) -> R {
    match &args[0] {
        Value::Ref(a) => Ok(Value::Number(f64::from(a.cols()))),
        Value::Array(a) => Ok(Value::Number(a.cols() as f64)),
        Value::Error(e) => Err(*e),
        Value::Refs(_) => Err(ErrorKind::Ref),
        _ => Ok(Value::Number(1.0)),
    }
}

fn quote_sheet(name: &str) -> String {
    if name.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '.')
        && !name.starts_with(|c: char| c.is_ascii_digit())
    {
        name.to_owned()
    } else {
        format!("'{}'", name.replace('\'', "''"))
    }
}

fn address(ctx: &mut Ctx, args: &[Value]) -> R {
    let r = ctx.num(&args[0])?.trunc();
    let c = ctx.num(&args[1])?.trunc();
    let abs = opt_num(ctx, args, 2, 1.0)?.trunc() as i64;
    let a1 = opt_bool(ctx, args, 3, true)?;
    if r < 1.0 || c < 1.0 || r > f64::from(MAX_ROWS) || c > f64::from(MAX_COLS) || !(1..=4).contains(&abs) {
        return Err(ErrorKind::Value);
    }
    let (row_abs, col_abs) = match abs {
        1 => (true, true),
        2 => (true, false),
        3 => (false, true),
        _ => (false, false),
    };
    let mut out = String::new();
    if let Some(v) = args.get(4) {
        if !matches!(v, Value::Empty) {
            let sheet = ctx.text(v)?;
            out.push_str(&quote_sheet(&sheet));
            out.push('!');
        }
    }
    let (r, c) = (r as u32 - 1, c as u32 - 1);
    if a1 {
        out.push_str(
            &CellAddr {
                row: r,
                col: c,
                row_abs,
                col_abs,
            }
            .to_a1(),
        );
    } else {
        out.push('R');
        if row_abs {
            out.push_str(&(r + 1).to_string());
        } else {
            out.push_str(&format!("[{}]", r + 1));
        }
        out.push('C');
        if col_abs {
            out.push_str(&(c + 1).to_string());
        } else {
            out.push_str(&format!("[{}]", c + 1));
        }
    }
    let _ = col_to_letters;
    Ok(Value::Text(out))
}

fn areas(_ctx: &mut Ctx, args: &[Value]) -> R {
    match &args[0] {
        Value::Ref(_) => Ok(Value::Number(1.0)),
        Value::Refs(l) => Ok(Value::Number(l.len() as f64)),
        Value::Error(e) => Err(*e),
        _ => Err(ErrorKind::Value),
    }
}

// ------------------------------------------------------------ dynamic arrays

fn array_arg(ctx: &Ctx, v: &Value) -> Result<Arc<Array>, ErrorKind> {
    ctx.to_array(v.clone())
}

fn rows_of(a: &Array) -> Vec<Vec<Value>> {
    (0..a.rows()).map(|r| a.row_slice(r).to_vec()).collect()
}

fn from_rows(rows: Vec<Vec<Value>>) -> R {
    Array::from_rows(rows).map(Value::from).ok_or(ErrorKind::Calc)
}

fn filter(ctx: &mut Ctx, args: &[Value]) -> R {
    let a = array_arg(ctx, &args[0])?;
    let inc = array_arg(ctx, &args[1])?;
    let keep = |v: &Value| -> Result<bool, ErrorKind> {
        match v {
            Value::Error(e) => Err(*e),
            Value::Text(_) => Err(ErrorKind::Value),
            v => v.as_bool(),
        }
    };
    let out: Vec<Vec<Value>> = if inc.cols() == 1 && inc.rows() == a.rows() {
        let mut out = Vec::new();
        for r in 0..a.rows() {
            if keep(inc.get(r, 0))? {
                out.push(a.row_slice(r).to_vec());
            }
        }
        out
    } else if inc.rows() == 1 && inc.cols() == a.cols() {
        let cols: Vec<usize> = (0..a.cols())
            .filter_map(|c| match keep(inc.get(0, c)) {
                Ok(true) => Some(Ok(c)),
                Ok(false) => None,
                Err(e) => Some(Err(e)),
            })
            .collect::<Result<_, _>>()?;
        if cols.is_empty() {
            Vec::new()
        } else {
            (0..a.rows())
                .map(|r| cols.iter().map(|&c| a.get(r, c).clone()).collect())
                .collect()
        }
    } else {
        return Err(ErrorKind::Value);
    };
    if out.is_empty() {
        return match args.get(2) {
            Some(v) if !matches!(v, Value::Empty) => Ok(ctx.deref(v.clone())),
            _ => Err(ErrorKind::Calc),
        };
    }
    from_rows(out)
}

/// Sort order of values: numbers < text < booleans < errors < blanks.
fn sort_cmp(a: &Value, b: &Value) -> Ordering {
    let rank = |v: &Value| match v {
        Value::Number(_) => 0,
        Value::Text(_) => 1,
        Value::Bool(_) => 2,
        Value::Error(_) => 3,
        _ => 4,
    };
    match rank(a).cmp(&rank(b)) {
        Ordering::Equal if rank(a) < 3 => compare_values(a, b),
        o => o,
    }
}

fn sort_keyed(rows: &mut [(Vec<Value>, Vec<Value>)], orders: &[bool]) {
    rows.sort_by(|x, y| {
        for (k, asc) in orders.iter().enumerate() {
            let o = sort_cmp(&x.0[k], &y.0[k]);
            if o != Ordering::Equal {
                // Blanks/errors keep their place at the end regardless of direction.
                return if *asc { o } else { o.reverse() };
            }
        }
        Ordering::Equal
    });
}

fn sort(ctx: &mut Ctx, args: &[Value]) -> R {
    let a = array_arg(ctx, &args[0])?;
    let by_col = opt_bool(ctx, args, 3, false)?;
    let a = if by_col { Arc::new(a.transpose()) } else { a };
    let idx: Vec<usize> = match args.get(1) {
        None | Some(Value::Empty) => vec![1],
        Some(v) => super::flatten(ctx, v)
            .iter()
            .map(|x| ctx.num(x).map(|n| n.trunc() as usize))
            .collect::<Result<_, _>>()?,
    };
    let orders: Vec<bool> = match args.get(2) {
        None | Some(Value::Empty) => vec![true; idx.len()],
        Some(v) => super::flatten(ctx, v)
            .iter()
            .map(|x| match ctx.num(x)?.trunc() as i64 {
                1 => Ok(true),
                -1 => Ok(false),
                _ => Err(ErrorKind::Value),
            })
            .collect::<Result<_, _>>()?,
    };
    if idx.iter().any(|i| *i < 1 || *i > a.cols()) || (orders.len() != idx.len() && orders.len() != 1) {
        return Err(ErrorKind::Value);
    }
    let orders: Vec<bool> = if orders.len() == idx.len() {
        orders
    } else {
        vec![orders[0]; idx.len()]
    };
    let mut keyed: Vec<(Vec<Value>, Vec<Value>)> = rows_of(&a)
        .into_iter()
        .map(|r| (idx.iter().map(|i| r[i - 1].clone()).collect(), r))
        .collect();
    sort_keyed(&mut keyed, &orders);
    let out = Array::from_rows(keyed.into_iter().map(|(_, r)| r).collect()).ok_or(ErrorKind::Calc)?;
    Ok(if by_col { out.transpose() } else { out }.into())
}

fn sortby(ctx: &mut Ctx, args: &[Value]) -> R {
    let a = array_arg(ctx, &args[0])?;
    let mut keys: Vec<Arc<Array>> = Vec::new();
    let mut orders = Vec::new();
    let mut i = 1;
    while i < args.len() {
        keys.push(array_arg(ctx, &args[i])?);
        let asc = match args.get(i + 1) {
            None | Some(Value::Empty) => true,
            Some(v) => match ctx.num(v)?.trunc() as i64 {
                1 => true,
                -1 => false,
                _ => return Err(ErrorKind::Value),
            },
        };
        orders.push(asc);
        i += 2;
    }
    let by_col = keys.first().is_some_and(|k| k.rows() == 1 && k.cols() == a.cols() && a.rows() != k.cols());
    let a = if by_col { Arc::new(a.transpose()) } else { a };
    let n = a.rows();
    let key_vals: Vec<Vec<Value>> = keys
        .iter()
        .map(|k| {
            let v: Vec<Value> = k.data().to_vec();
            if v.len() == n {
                Ok(v)
            } else {
                Err(ErrorKind::Value)
            }
        })
        .collect::<Result<_, _>>()?;
    let mut keyed: Vec<(Vec<Value>, Vec<Value>)> = rows_of(&a)
        .into_iter()
        .enumerate()
        .map(|(r, row)| (key_vals.iter().map(|k| k[r].clone()).collect(), row))
        .collect();
    sort_keyed(&mut keyed, &orders);
    let out = Array::from_rows(keyed.into_iter().map(|(_, r)| r).collect()).ok_or(ErrorKind::Calc)?;
    Ok(if by_col { out.transpose() } else { out }.into())
}

fn rows_equal(a: &[Value], b: &[Value]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| same_value(x, y))
}

fn unique(ctx: &mut Ctx, args: &[Value]) -> R {
    let a = array_arg(ctx, &args[0])?;
    let by_col = opt_bool(ctx, args, 1, false)?;
    let once = opt_bool(ctx, args, 2, false)?;
    let a = if by_col { Arc::new(a.transpose()) } else { a };
    let rows = rows_of(&a);
    let mut out: Vec<Vec<Value>> = Vec::new();
    for (i, r) in rows.iter().enumerate() {
        let count = rows.iter().filter(|o| rows_equal(o, r)).count();
        let first = rows.iter().position(|o| rows_equal(o, r)) == Some(i);
        if first && (!once || count == 1) {
            out.push(r.clone());
        }
    }
    if out.is_empty() {
        return Err(ErrorKind::Calc);
    }
    let arr = Array::from_rows(out).ok_or(ErrorKind::Calc)?;
    Ok(if by_col { arr.transpose() } else { arr }.into())
}

fn transpose(ctx: &mut Ctx, args: &[Value]) -> R {
    let a = array_arg(ctx, &args[0])?;
    Ok(a.transpose().into())
}

fn stack(ctx: &Ctx, args: &[Value], horizontal: bool) -> R {
    let arrays: Vec<Arc<Array>> = args.iter().map(|v| array_arg(ctx, v)).collect::<Result<_, _>>()?;
    let pad = Value::Error(ErrorKind::NA);
    if horizontal {
        let rows = arrays.iter().map(|a| a.rows()).max().unwrap_or(1);
        let out: Vec<Vec<Value>> = (0..rows)
            .map(|r| {
                arrays
                    .iter()
                    .flat_map(|a| (0..a.cols()).map(move |c| (a, c)))
                    .map(|(a, c)| if r < a.rows() { a.get(r, c).clone() } else { pad.clone() })
                    .collect()
            })
            .collect();
        from_rows(out)
    } else {
        let cols = arrays.iter().map(|a| a.cols()).max().unwrap_or(1);
        let mut out = Vec::new();
        for a in &arrays {
            for r in 0..a.rows() {
                let mut row = a.row_slice(r).to_vec();
                row.resize(cols, pad.clone());
                out.push(row);
            }
        }
        from_rows(out)
    }
}

fn hstack(ctx: &mut Ctx, args: &[Value]) -> R {
    stack(ctx, args, true)
}

fn vstack(ctx: &mut Ctx, args: &[Value]) -> R {
    stack(ctx, args, false)
}

/// Resolve 1-based (negative from end) indices.
fn pick_indices(ctx: &Ctx, args: &[Value], len: usize) -> Result<Vec<usize>, ErrorKind> {
    let mut out = Vec::new();
    for a in args {
        for v in super::flatten(ctx, a) {
            let n = ctx.num(&v)?.trunc() as i64;
            let i = if n > 0 { n - 1 } else { len as i64 + n };
            if n == 0 || i < 0 || i >= len as i64 {
                return Err(ErrorKind::Value);
            }
            out.push(i as usize);
        }
    }
    Ok(out)
}

fn choosecols(ctx: &mut Ctx, args: &[Value]) -> R {
    let a = array_arg(ctx, &args[0])?;
    let idx = pick_indices(ctx, &args[1..], a.cols())?;
    from_rows(
        (0..a.rows())
            .map(|r| idx.iter().map(|&c| a.get(r, c).clone()).collect())
            .collect(),
    )
}

fn chooserows(ctx: &mut Ctx, args: &[Value]) -> R {
    let a = array_arg(ctx, &args[0])?;
    let idx = pick_indices(ctx, &args[1..], a.rows())?;
    from_rows(idx.iter().map(|&r| a.row_slice(r).to_vec()).collect())
}

/// Row/column ranges kept by TAKE (`take = true`) or DROP.
fn span(n: Option<f64>, len: usize, take: bool) -> Result<(usize, usize), ErrorKind> {
    let Some(n) = n else {
        return Ok((0, len));
    };
    let k = (n.trunc().abs() as usize).min(len);
    let r = match (take, n >= 0.0) {
        (true, true) => (0, k),
        (true, false) => (len - k, len),
        (false, true) => (k, len),
        (false, false) => (0, len - k),
    };
    if r.0 >= r.1 {
        return Err(ErrorKind::Calc);
    }
    Ok(r)
}

fn take_drop(ctx: &Ctx, args: &[Value], take: bool) -> R {
    let a = array_arg(ctx, &args[0])?;
    let opt = |i: usize| -> Result<Option<f64>, ErrorKind> {
        match args.get(i) {
            None | Some(Value::Empty) => Ok(None),
            Some(v) => ctx.num(v).map(Some),
        }
    };
    let (r1, r2) = span(opt(1)?, a.rows(), take)?;
    let (c1, c2) = span(opt(2)?, a.cols(), take)?;
    from_rows(
        (r1..r2)
            .map(|r| (c1..c2).map(|c| a.get(r, c).clone()).collect())
            .collect(),
    )
}

fn take(ctx: &mut Ctx, args: &[Value]) -> R {
    take_drop(ctx, args, true)
}

fn drop(ctx: &mut Ctx, args: &[Value]) -> R {
    take_drop(ctx, args, false)
}
