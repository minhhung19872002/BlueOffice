//! More lookup & reference functions (ECMA-376 Part 1, §18.17.7 and [MS-XLSX] future
//! functions): array shaping (`EXPAND`, `TOCOL`, `TOROW`, `WRAPROWS`, `WRAPCOLS`,
//! `TRIMRANGE`), grouping (`GROUPBY`, `PIVOTBY`), `HYPERLINK`, and the functions that depend on
//! data the formula engine does not hold (`GETPIVOTDATA`, `IMAGE`, `RTD`).

use std::cmp::Ordering;
use std::sync::Arc;

use super::{f, flatten, opt_bool, opt_num, shape, FnSpec, VAR};
use crate::eval::{Ctx, R};
use crate::value::{compare_values, Array, ErrorKind, ImageValue, Lambda, Value};

pub(super) const FUNCTIONS: &[FnSpec] = &[
    f("EXPAND", 2, 4, expand),
    f("TOCOL", 1, 3, tocol),
    f("TOROW", 1, 3, torow),
    f("WRAPROWS", 2, 3, wraprows),
    f("WRAPCOLS", 2, 3, wrapcols),
    f("TRIMRANGE", 1, 3, trimrange),
    f("GROUPBY", 3, 8, groupby),
    f("PIVOTBY", 4, 11, pivotby),
    f("HYPERLINK", 1, 2, hyperlink),
    f("GETPIVOTDATA", 2, VAR, getpivotdata).volatile(),
    f("IMAGE", 1, 5, image),
    f("RTD", 3, VAR, rtd),
];

fn array_value(rows: usize, cols: usize, data: Vec<Value>) -> R {
    if rows == 0 || cols == 0 {
        return Err(ErrorKind::Calc);
    }
    Ok(Value::Array(Array::new(rows, cols, data).into()))
}

/// Rows of an argument (range or array).
fn grid(ctx: &Ctx, v: &Value) -> Result<(usize, usize, Vec<Value>), ErrorKind> {
    if let Value::Error(e) = v {
        return Err(*e);
    }
    let (r, c) = shape(ctx, v);
    Ok((r, c, flatten(ctx, v)))
}

// ------------------------------------------------------------------ shaping

fn pad_arg(args: &[Value], i: usize) -> Value {
    match args.get(i) {
        None | Some(Value::Empty) => Value::Error(ErrorKind::NA),
        Some(v) => v.clone(),
    }
}

fn expand(ctx: &mut Ctx, args: &[Value]) -> R {
    let (r, c, data) = grid(ctx, &args[0])?;
    let rows = match args.get(1) {
        None | Some(Value::Empty) => r as f64,
        Some(v) => ctx.num(v)?.trunc(),
    };
    let cols = match args.get(2) {
        None | Some(Value::Empty) => c as f64,
        Some(v) => ctx.num(v)?.trunc(),
    };
    if rows < r as f64 || cols < c as f64 {
        return Err(ErrorKind::Value);
    }
    if rows * cols > 1.0e8 {
        return Err(ErrorKind::Num);
    }
    let pad = ctx.scalar(&pad_arg(args, 3));
    let (rows, cols) = (rows as usize, cols as usize);
    let out = (0..rows * cols)
        .map(|i| {
            let (y, x) = (i / cols, i % cols);
            if y < r && x < c {
                data[y * c + x].clone()
            } else {
                pad.clone()
            }
        })
        .collect();
    array_value(rows, cols, out)
}

/// Values in row-major (or column-major) order, skipping blanks (ignore 1), errors (2) or both
/// (3).
fn linear(ctx: &Ctx, args: &[Value]) -> Result<Vec<Value>, ErrorKind> {
    let (r, c, data) = grid(ctx, &args[0])?;
    let ignore = opt_num(ctx, args, 1, 0.0)?.trunc();
    if !(0.0..=3.0).contains(&ignore) {
        return Err(ErrorKind::Value);
    }
    let by_col = opt_bool(ctx, args, 2, false)?;
    let order: Vec<usize> = if by_col {
        (0..c)
            .flat_map(|x| (0..r).map(move |y| y * c + x))
            .collect()
    } else {
        (0..r * c).collect()
    };
    let ig = ignore as u8;
    Ok(order
        .into_iter()
        .map(|i| data[i].clone())
        .filter(|v| match v {
            Value::Empty => ig & 1 == 0,
            Value::Error(_) => ig & 2 == 0,
            _ => true,
        })
        .collect())
}

fn tocol(ctx: &mut Ctx, args: &[Value]) -> R {
    let v = linear(ctx, args)?;
    let n = v.len();
    array_value(n, 1, v)
}

fn torow(ctx: &mut Ctx, args: &[Value]) -> R {
    let v = linear(ctx, args)?;
    let n = v.len();
    array_value(1, n, v)
}

fn wrap(ctx: &Ctx, args: &[Value], by_rows: bool) -> R {
    let (r, c, data) = grid(ctx, &args[0])?;
    if r != 1 && c != 1 {
        return Err(ErrorKind::Value);
    }
    let count = ctx.num(&args[1])?.trunc();
    if count < 1.0 {
        return Err(ErrorKind::Num);
    }
    let count = count as usize;
    let pad = ctx.scalar(&pad_arg(args, 2));
    let n = data.len();
    let lines = n.div_ceil(count);
    let at = |i: usize| data.get(i).cloned().unwrap_or_else(|| pad.clone());
    if by_rows {
        array_value(lines, count, (0..lines * count).map(at).collect())
    } else {
        // Column-major: element i goes to row i % count, column i / count.
        let out = (0..count * lines)
            .map(|k| {
                let (y, x) = (k / lines, k % lines);
                at(x * count + y)
            })
            .collect();
        array_value(count, lines, out)
    }
}

fn wraprows(ctx: &mut Ctx, args: &[Value]) -> R {
    wrap(ctx, args, true)
}

fn wrapcols(ctx: &mut Ctx, args: &[Value]) -> R {
    wrap(ctx, args, false)
}

fn trimrange(ctx: &mut Ctx, args: &[Value]) -> R {
    let Value::Ref(area) = &args[0] else {
        return match &args[0] {
            Value::Error(e) => Err(*e),
            _ => Err(ErrorKind::Value),
        };
    };
    let rows_mode = opt_num(ctx, args, 1, 3.0)?.trunc();
    let cols_mode = opt_num(ctx, args, 2, 3.0)?.trunc();
    if !(0.0..=3.0).contains(&rows_mode) || !(0.0..=3.0).contains(&cols_mode) {
        return Err(ErrorKind::Value);
    }
    ctx.trim_area(area, rows_mode as u8, cols_mode as u8)
        .map(Value::Ref)
        .ok_or(ErrorKind::Ref)
}

// ------------------------------------------------------------------ GROUPBY / PIVOTBY

fn lambda_arg(v: &Value) -> Result<Arc<Lambda>, ErrorKind> {
    match v {
        Value::Lambda(l) => Ok(l.clone()),
        Value::Error(e) => Err(*e),
        _ => Err(ErrorKind::Value),
    }
}

/// Apply the aggregation to a column of values.
fn aggregate(ctx: &mut Ctx, lam: &Lambda, values: Vec<Value>) -> Value {
    let n = values.len();
    let arg = if n == 0 {
        Value::Empty
    } else {
        Value::Array(Array::new(n, 1, values).into())
    };
    match ctx.invoke_lambda(lam, vec![arg]) {
        Ok(v) => match ctx.deref(v) {
            Value::Array(a) if a.rows() == 1 && a.cols() == 1 => a.get(0, 0).clone(),
            Value::Array(_) => Value::Error(ErrorKind::Calc),
            v => v,
        },
        Err(e) => Value::Error(e),
    }
}

/// Field headers: 0 none, 1 present but hidden, 2 none but generated, 3 present and shown;
/// omitted → present and shown when the first value row is text above numbers.
fn headers(
    ctx: &Ctx,
    arg: Option<&Value>,
    values: &[Vec<Value>],
) -> Result<(bool, bool), ErrorKind> {
    match arg {
        None | Some(Value::Empty) => {
            let first_text = values
                .first()
                .is_some_and(|r| r.iter().all(|v| matches!(v, Value::Text(_))));
            let then_number = values
                .get(1)
                .is_some_and(|r| r.iter().any(|v| matches!(v, Value::Number(_))));
            let auto = first_text && then_number;
            Ok((auto, auto))
        }
        Some(v) => match ctx.num(v)?.trunc() as i64 {
            0 => Ok((false, false)),
            1 => Ok((true, false)),
            2 => Ok((false, true)),
            3 => Ok((true, true)),
            _ => Err(ErrorKind::Value),
        },
    }
}

fn rows_of(ctx: &Ctx, v: &Value) -> Result<Vec<Vec<Value>>, ErrorKind> {
    let (r, c, data) = grid(ctx, v)?;
    Ok((0..r).map(|i| data[i * c..(i + 1) * c].to_vec()).collect())
}

fn key_cmp(a: &[Value], b: &[Value]) -> Ordering {
    for (x, y) in a.iter().zip(b) {
        let o = compare_values(x, y);
        if o != Ordering::Equal {
            return o;
        }
    }
    Ordering::Equal
}

/// Distinct keys in ascending order.
fn distinct(keys: &[Vec<Value>]) -> Vec<Vec<Value>> {
    let mut out: Vec<Vec<Value>> = Vec::new();
    for k in keys {
        if !out.iter().any(|o| key_cmp(o, k) == Ordering::Equal) {
            out.push(k.clone());
        }
    }
    out.sort_by(|a, b| key_cmp(a, b));
    out
}

fn filter_mask(ctx: &Ctx, arg: Option<&Value>, n: usize) -> Result<Vec<bool>, ErrorKind> {
    match arg {
        None | Some(Value::Empty) => Ok(vec![true; n]),
        Some(v) => {
            let f = flatten(ctx, v);
            if f.len() != n {
                return Err(ErrorKind::Value);
            }
            f.iter()
                .map(|x| match x {
                    Value::Bool(b) => Ok(*b),
                    Value::Number(n) => Ok(*n != 0.0),
                    Value::Error(e) => Err(*e),
                    _ => Ok(false),
                })
                .collect()
        }
    }
}

fn total_label() -> Value {
    Value::Text("Total".to_owned())
}

/// Key levels that get subtotals for a `total_depth` argument: `±2` subtotals the first
/// key column (and each further step one more), never the last (its groups are the rows).
fn subtotal_levels(depth: f64, keys: usize) -> usize {
    let d = depth.abs() as usize;
    d.saturating_sub(1).min(keys.saturating_sub(1))
}

/// A `sort_order` argument: an output column (key columns first, then values), negative
/// for descending; `0` keeps the keys ascending.
#[derive(Clone, Copy)]
struct SortSpec {
    /// 0-based output column.
    col: Option<usize>,
    descending: bool,
    keys: usize,
}

impl SortSpec {
    fn new(sort: f64, keys: usize) -> Self {
        SortSpec {
            col: (sort != 0.0).then(|| (sort.abs() as usize).saturating_sub(1)),
            descending: sort < 0.0,
            keys,
        }
    }
}

/// One output line of a nested grouping: a group (`prefix` has every key) or the subtotal
/// of the groups sharing the shorter `prefix`, with the source rows it aggregates.
struct Line {
    prefix: Vec<Value>,
    rows: Vec<usize>,
}

/// Lines of a grouping with subtotals for the first `sub_levels` key columns, placed
/// before (`top`) or after their groups. At each level the groups are ordered by their
/// key, or by their aggregated value when `order` names a value column (ties by key).
#[allow(clippy::too_many_arguments)]
fn hierarchy(
    ctx: &mut Ctx,
    keys_rows: &[Vec<Value>],
    rows: &[usize],
    nk: usize,
    sub_levels: usize,
    top: bool,
    order: SortSpec,
    agg: &dyn Fn(&mut Ctx, &[usize]) -> Vec<Value>,
) -> Vec<Line> {
    fn level(
        ctx: &mut Ctx,
        keys_rows: &[Vec<Value>],
        rows: &[usize],
        depth: usize,
        nk: usize,
        sub_levels: usize,
        top: bool,
        order: SortSpec,
        agg: &dyn Fn(&mut Ctx, &[usize]) -> Vec<Value>,
        out: &mut Vec<Line>,
    ) {
        // Children: distinct values of key column `depth` among `rows`.
        let mut children: Vec<(Vec<Value>, Vec<usize>)> = Vec::new();
        for &i in rows {
            let prefix = &keys_rows[i][..=depth];
            match children
                .iter_mut()
                .find(|(p, _)| key_cmp(p, prefix) == Ordering::Equal)
            {
                Some((_, sel)) => sel.push(i),
                None => children.push((prefix.to_vec(), vec![i])),
            }
        }
        let by_key = |a: &[Value], b: &[Value]| compare_values(&a[depth], &b[depth]);
        let mut keyed: Vec<(Option<Value>, Vec<Value>, Vec<usize>)> = children
            .into_iter()
            .map(|(p, sel)| {
                let v = match order.col {
                    Some(c) if c >= order.keys => agg(ctx, &sel).get(c - order.keys).cloned(),
                    _ => None,
                };
                (v, p, sel)
            })
            .collect();
        keyed.sort_by(|a, b| {
            let o = match (&a.0, &b.0, order.col) {
                (Some(x), Some(y), _) => compare_values(x, y).then_with(|| by_key(&a.1, &b.1)),
                (_, _, Some(c)) if c == depth => by_key(&a.1, &b.1),
                _ => return by_key(&a.1, &b.1),
            };
            if order.descending {
                o.reverse()
            } else {
                o
            }
        });
        for (_, prefix, sel) in keyed {
            let subtotal = depth < sub_levels;
            if depth + 1 == nk {
                out.push(Line { prefix, rows: sel });
                continue;
            }
            let sub = Line {
                prefix: prefix.clone(),
                rows: sel.clone(),
            };
            if subtotal && top {
                out.push(sub);
                level(
                    ctx,
                    keys_rows,
                    &sel,
                    depth + 1,
                    nk,
                    sub_levels,
                    top,
                    order,
                    agg,
                    out,
                );
            } else {
                level(
                    ctx,
                    keys_rows,
                    &sel,
                    depth + 1,
                    nk,
                    sub_levels,
                    top,
                    order,
                    agg,
                    out,
                );
                if subtotal {
                    out.push(sub);
                }
            }
        }
    }
    let mut out = Vec::new();
    level(
        ctx, keys_rows, rows, 0, nk, sub_levels, top, order, agg, &mut out,
    );
    out
}

fn groupby(ctx: &mut Ctx, args: &[Value]) -> R {
    let keys_rows = rows_of(ctx, &args[0])?;
    let value_rows = rows_of(ctx, &args[1])?;
    if keys_rows.len() != value_rows.len() || keys_rows.is_empty() {
        return Err(ErrorKind::Value);
    }
    let lam = lambda_arg(&args[2])?;
    let (has_headers, show_headers) = headers(ctx, args.get(3), &value_rows)?;
    let depth = opt_num(ctx, args, 4, 1.0)?.trunc();
    let sort = opt_num(ctx, args, 5, 1.0)?.trunc();
    let start = usize::from(has_headers);
    let mask = filter_mask(ctx, args.get(6), keys_rows.len())?;
    let rows: Vec<usize> = (start..keys_rows.len()).filter(|&i| mask[i]).collect();
    let nk = keys_rows[0].len();
    let nv = value_rows[0].len();
    let keys: Vec<Vec<Value>> = rows.iter().map(|&i| keys_rows[i].clone()).collect();
    let groups = distinct(&keys);
    let agg = |ctx: &mut Ctx, pick: &dyn Fn(usize) -> bool| -> Vec<Value> {
        (0..nv)
            .map(|v| {
                let col: Vec<Value> = rows
                    .iter()
                    .filter(|&&i| pick(i))
                    .map(|&i| value_rows[i][v].clone())
                    .collect();
                aggregate(ctx, &lam, col)
            })
            .collect()
    };
    let sort_col = (sort.abs() as usize).saturating_sub(1);
    if sort != 0.0 && sort_col >= nk + nv {
        return Err(ErrorKind::Value);
    }
    let sub_levels = subtotal_levels(depth, nk);
    let mut body: Vec<Vec<Value>> = Vec::new();
    if sub_levels > 0 {
        // Subtotals: groups nest by key column; each level is sorted on its own.
        let agg_rows = |ctx: &mut Ctx, sel: &[usize]| -> Vec<Value> {
            (0..nv)
                .map(|v| {
                    let col: Vec<Value> = sel.iter().map(|&i| value_rows[i][v].clone()).collect();
                    aggregate(ctx, &lam, col)
                })
                .collect()
        };
        let order = SortSpec::new(sort, nk);
        let lines = hierarchy(
            ctx,
            &keys_rows,
            &rows,
            nk,
            sub_levels,
            depth < 0.0,
            order,
            &agg_rows,
        );
        for line in lines {
            let mut row = line.prefix.clone();
            row.resize(nk, Value::Empty);
            row.extend(agg_rows(ctx, &line.rows));
            body.push(row);
        }
    } else {
        for g in &groups {
            let mut row = g.clone();
            row.extend(agg(ctx, &|i| key_cmp(&keys_rows[i], g) == Ordering::Equal));
            body.push(row);
        }
        // Sort by the given output column; negative descends.
        if sort != 0.0 {
            body.sort_by(|a, b| {
                let o = compare_values(&a[sort_col], &b[sort_col]);
                if sort < 0.0 {
                    o.reverse()
                } else {
                    o
                }
            });
        }
    }
    let mut out: Vec<Vec<Value>> = Vec::new();
    if show_headers {
        let mut h: Vec<Value> = if has_headers {
            keys_rows[0].clone()
        } else {
            (1..=nk)
                .map(|i| Value::Text(format!("Row Field {i}")))
                .collect()
        };
        if has_headers {
            h.extend(value_rows[0].iter().cloned());
        } else {
            h.extend((1..=nv).map(|i| Value::Text(format!("Value {i}"))));
        }
        out.push(h);
    }
    let mut total = vec![total_label()];
    total.extend(std::iter::repeat_n(Value::Empty, nk - 1));
    total.extend(agg(ctx, &|_| true));
    if depth < 0.0 {
        out.push(total.clone());
    }
    out.extend(body);
    if depth > 0.0 {
        out.push(total);
    }
    let cols = nk + nv;
    let r = out.len();
    array_value(r, cols, out.into_iter().flatten().collect())
}

fn pivotby(ctx: &mut Ctx, args: &[Value]) -> R {
    let row_keys = rows_of(ctx, &args[0])?;
    let col_keys = rows_of(ctx, &args[1])?;
    let value_rows = rows_of(ctx, &args[2])?;
    if row_keys.len() != value_rows.len()
        || col_keys.len() != value_rows.len()
        || row_keys.is_empty()
    {
        return Err(ErrorKind::Value);
    }
    let lam = lambda_arg(&args[3])?;
    let (has_headers, _) = headers(ctx, args.get(4), &value_rows)?;
    let row_total = opt_num(ctx, args, 5, 1.0)?.trunc();
    let row_sort = opt_num(ctx, args, 6, 1.0)?.trunc();
    let col_total = opt_num(ctx, args, 7, 1.0)?.trunc();
    let col_sort = opt_num(ctx, args, 8, 1.0)?.trunc();
    let mask = filter_mask(ctx, args.get(9), value_rows.len())?;
    let start = usize::from(has_headers);
    let rows: Vec<usize> = (start..value_rows.len()).filter(|&i| mask[i]).collect();
    let nrk = row_keys[0].len();
    let nck = col_keys[0].len();
    let nv = value_rows[0].len();
    // Sort orders name key columns of their own axis (negative descends).
    let key_sort = |s: f64, keys: usize| -> Result<SortSpec, ErrorKind> {
        let spec = SortSpec::new(s, keys);
        if spec.col.is_some_and(|c| c >= keys) {
            return Err(ErrorKind::Value);
        }
        Ok(spec)
    };
    let (row_order, col_order) = (key_sort(row_sort, nrk)?, key_sort(col_sort, nck)?);
    let none = |_: &mut Ctx, _: &[usize]| -> Vec<Value> { Vec::new() };
    let row_lines = hierarchy(
        ctx,
        &row_keys,
        &rows,
        nrk,
        subtotal_levels(row_total, nrk),
        row_total < 0.0,
        row_order,
        &none,
    );
    let col_lines = hierarchy(
        ctx,
        &col_keys,
        &rows,
        nck,
        subtotal_levels(col_total, nck),
        col_total < 0.0,
        col_order,
        &none,
    );
    // Column groups in output order; `None` is the grand total.
    let mut columns: Vec<Option<&Line>> = col_lines.iter().map(Some).collect();
    if col_total < 0.0 {
        columns.insert(0, None);
    } else if col_total > 0.0 {
        columns.push(None);
    }
    let cell = |ctx: &mut Ctx, r: Option<&Line>, c: Option<&Line>, v: usize| -> Value {
        let vals: Vec<Value> = rows
            .iter()
            .filter(|i| r.is_none_or(|l| l.rows.contains(i)))
            .filter(|i| c.is_none_or(|l| l.rows.contains(i)))
            .map(|&i| value_rows[i][v].clone())
            .collect();
        if vals.is_empty() {
            return Value::Empty;
        }
        aggregate(ctx, &lam, vals)
    };
    let width = nrk + columns.len() * nv;
    let mut out: Vec<Vec<Value>> = Vec::new();
    // Header rows: one per column field, then the value names when there are several.
    for level in 0..nck {
        let mut h = vec![Value::Empty; nrk];
        for c in &columns {
            let label = match c {
                Some(l) => l.prefix.get(level).cloned().unwrap_or(Value::Empty),
                None if level == 0 => total_label(),
                None => Value::Empty,
            };
            h.extend(std::iter::repeat_n(label, nv));
        }
        out.push(h);
    }
    if nv > 1 {
        let mut h = vec![Value::Empty; nrk];
        for _ in &columns {
            h.extend((0..nv).map(|v| {
                if has_headers {
                    value_rows[0][v].clone()
                } else {
                    Value::Text(format!("Value {}", v + 1))
                }
            }));
        }
        out.push(h);
    }
    let row_line = |ctx: &mut Ctx, line: Option<&Line>| -> Vec<Value> {
        let mut out = match line {
            Some(l) => {
                let mut k = l.prefix.clone();
                k.resize(nrk, Value::Empty);
                k
            }
            None => {
                let mut l = vec![total_label()];
                l.extend(std::iter::repeat_n(Value::Empty, nrk - 1));
                l
            }
        };
        for c in &columns {
            for v in 0..nv {
                out.push(cell(ctx, line, *c, v));
            }
        }
        out
    };
    if row_total < 0.0 {
        out.push(row_line(ctx, None));
    }
    for l in &row_lines {
        out.push(row_line(ctx, Some(l)));
    }
    if row_total > 0.0 {
        out.push(row_line(ctx, None));
    }
    let r = out.len();
    array_value(r, width, out.into_iter().flatten().collect())
}

// ------------------------------------------------------------------ links and external data

/// `HYPERLINK(link, [friendly])`: the cell shows the friendly name, else the link.
fn hyperlink(ctx: &mut Ctx, args: &[Value]) -> R {
    match args.get(1) {
        None | Some(Value::Empty) => Ok(Value::Text(ctx.text(&args[0])?)),
        Some(v) => Ok(ctx.scalar(v)),
    }
}

/// Whether pivot item caption `item` is the item `wanted` names: numbers (dates) by value,
/// text without regard to case, a number given as text by its value.
fn pivot_item_matches(item: &Value, wanted: &Value) -> bool {
    match (item, wanted) {
        (Value::Number(a), Value::Number(b)) => (a - b).abs() <= 1e-9 * a.abs().max(1.0),
        (Value::Text(a), Value::Text(b)) => a.to_lowercase() == b.to_lowercase(),
        (Value::Bool(a), Value::Bool(b)) => a == b,
        (Value::Number(a), Value::Text(b)) | (Value::Text(b), Value::Number(a)) => {
            b.trim().parse::<f64>().is_ok_and(|x| x == *a)
        }
        _ => false,
    }
}

/// `GETPIVOTDATA(data_field, pivot_table, [field1, item1], …)` (ECMA-376 Part 1, §18.17.7
/// GETPIVOTDATA): the value the pivot table report containing `pivot_table` shows for the
/// data field and items. Fields left out total over their items, so the named row (column)
/// fields must be the outermost ones; naming none gives the grand total. `#REF!` when the
/// reference is not in a report, a name or item is unknown, or the report does not show the
/// combination (hidden subtotals or grand totals, filtered items).
fn getpivotdata(ctx: &mut Ctx, args: &[Value]) -> R {
    let area = match &args[1] {
        Value::Ref(a) => *a,
        Value::Refs(l) => *l.first().ok_or(ErrorKind::Ref)?,
        Value::Error(e) => return Err(*e),
        _ => return Err(ErrorKind::Ref),
    };
    let name_of = |ctx: &Ctx, v: &Value| -> Result<String, ErrorKind> {
        match ctx.scalar(v) {
            Value::Error(e) => Err(e),
            v => ctx.text(&v),
        }
    };
    let data_field = name_of(ctx, &args[0])?;
    let pivot = ctx
        .provider
        .pivot_at(area.sheet, area.r1, area.c1)
        .ok_or(ErrorKind::Ref)?;
    let lower = data_field.trim().to_lowercase();
    let data = pivot
        .data_fields
        .iter()
        .position(|names| names.iter().any(|n| n.trim().to_lowercase() == lower))
        .ok_or(ErrorKind::Ref)?;
    let pairs = &args[2..];
    if pairs.len() % 2 != 0 {
        return Err(ErrorKind::Ref);
    }
    let mut row_sel: Vec<Option<u32>> = vec![None; pivot.row_fields.len()];
    let mut col_sel: Vec<Option<u32>> = vec![None; pivot.col_fields.len()];
    for pair in pairs.chunks(2) {
        let field = name_of(ctx, &pair[0])?.trim().to_lowercase();
        let wanted = match ctx.scalar(&pair[1]) {
            Value::Error(e) => return Err(e),
            v => v,
        };
        let mut found = false;
        for (fields, sel) in [
            (&pivot.row_fields, &mut row_sel),
            (&pivot.col_fields, &mut col_sel),
        ] {
            if let Some(fi) = fields
                .iter()
                .position(|f| f.name.trim().to_lowercase() == field)
            {
                let item = fields[fi]
                    .items
                    .iter()
                    .position(|it| pivot_item_matches(it, &wanted))
                    .ok_or(ErrorKind::Ref)?;
                sel[fi] = Some(item as u32);
                found = true;
                break;
            }
        }
        if !found {
            return Err(ErrorKind::Ref);
        }
    }
    // The named fields of an axis must be its outermost ones (a report shows no total
    // over an outer field for one inner item).
    let path = |sel: &[Option<u32>]| -> Result<Vec<u32>, ErrorKind> {
        let n = sel.iter().take_while(|s| s.is_some()).count();
        if sel[n..].iter().any(Option::is_some) {
            return Err(ErrorKind::Ref);
        }
        Ok(sel[..n].iter().flatten().copied().collect())
    };
    let (rp, cp) = (path(&row_sel)?, path(&col_sel)?);
    let cell = pivot
        .cells
        .iter()
        .find(|c| c.data == data && c.row_path == rp && c.col_path == cp)
        .ok_or(ErrorKind::Ref)?;
    match ctx
        .provider
        .cell_value(pivot.area.sheet, cell.row, cell.col)
    {
        Value::Error(e) => Err(e),
        v => Ok(v),
    }
}

/// Longest `source` URL `IMAGE` accepts (characters; Microsoft's published `IMAGE`
/// function documentation gives 255).
pub const IMAGE_MAX_URL: usize = 255;

/// `IMAGE(source, [alt_text], [sizing], [height], [width])` ([MS-XLSX] future
/// function; result type: §2.3.6.1.11 web image rich value).
///
/// The formula engine only checks the arguments and returns a
/// [`Value::Image`] request; the host fetches the picture asynchronously through
/// its platform layer and the sheet shows it (or `#CONNECT!` / `#BLOCKED!` when the
/// fetch fails — those come from the host, not from this function).
///
/// `#VALUE!` (as the published function documentation describes): `source` empty,
/// not `https://`, longer than [`IMAGE_MAX_URL`] or not text; `alt_text` not text;
/// `sizing` not 0–3; `height` / `width` given with sizing 0–2; sizing 3 with both
/// `height` and `width` missing or below 1.
fn image(ctx: &mut Ctx, args: &[Value]) -> R {
    let text_arg = |v: &Value| -> Result<String, ErrorKind> {
        match ctx.scalar(v) {
            Value::Text(s) => Ok(s),
            Value::Error(e) => Err(e),
            _ => Err(ErrorKind::Value),
        }
    };
    let source = text_arg(&args[0])?.trim().to_owned();
    let lower = source.to_ascii_lowercase();
    if !lower.starts_with("https://")
        || source.len() <= "https://".len()
        || source.chars().count() > IMAGE_MAX_URL
    {
        return Err(ErrorKind::Value);
    }
    let alt_text = match args.get(1) {
        None | Some(Value::Empty) => None,
        Some(v) => Some(text_arg(v)?).filter(|s| !s.is_empty()),
    };
    let sizing = opt_num(ctx, args, 2, 0.0)?.trunc();
    if !(0.0..=3.0).contains(&sizing) {
        return Err(ErrorKind::Value);
    }
    let dim = |i: usize| -> Result<Option<f64>, ErrorKind> {
        match args.get(i) {
            None | Some(Value::Empty) => Ok(None),
            Some(v) => ctx.num(v).map(Some),
        }
    };
    let (height, width) = (dim(3)?, dim(4)?);
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let sizing = sizing as u8;
    let (height, width) = if sizing == 3 {
        let ok = |d: Option<f64>| d.filter(|x| *x >= 1.0);
        let (h, w) = (ok(height), ok(width));
        if h.is_none() && w.is_none() {
            return Err(ErrorKind::Value);
        }
        (h, w)
    } else {
        if height.is_some() || width.is_some() {
            return Err(ErrorKind::Value);
        }
        (None, None)
    };
    Ok(Value::Image(Arc::new(ImageValue {
        source,
        alt_text,
        sizing,
        height,
        width,
    })))
}

/// `RTD(prog_id, server, topic, …)`: no real-time data server is available → `#N/A`.
fn rtd(_ctx: &mut Ctx, _args: &[Value]) -> R {
    Err(ErrorKind::NA)
}
