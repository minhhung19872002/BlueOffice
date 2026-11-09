//! More lookup & reference functions (ECMA-376 Part 1, §18.17.7 and [MS-XLSX] future
//! functions): array shaping (`EXPAND`, `TOCOL`, `TOROW`, `WRAPROWS`, `WRAPCOLS`,
//! `TRIMRANGE`), grouping (`GROUPBY`, `PIVOTBY`), `HYPERLINK`, and the functions that depend on
//! data the formula engine does not hold (`GETPIVOTDATA`, `IMAGE`, `RTD`).

use std::cmp::Ordering;
use std::sync::Arc;

use super::{f, flatten, opt_bool, opt_num, shape, FnSpec, VAR};
use crate::eval::{Ctx, R};
use crate::value::{compare_values, Array, ErrorKind, Lambda, Value};

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
    f("GETPIVOTDATA", 2, VAR, getpivotdata),
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
    let mut body: Vec<Vec<Value>> = Vec::new();
    for g in &groups {
        let mut row = g.clone();
        row.extend(agg(ctx, &|i| key_cmp(&keys_rows[i], g) == Ordering::Equal));
        body.push(row);
    }
    // Sort by the given output column; negative descends.
    if sort != 0.0 {
        let col = (sort.abs() as usize).saturating_sub(1);
        if col >= nk + nv {
            return Err(ErrorKind::Value);
        }
        body.sort_by(|a, b| {
            let o = compare_values(&a[col], &b[col]);
            if sort < 0.0 {
                o.reverse()
            } else {
                o
            }
        });
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
    let col_total = opt_num(ctx, args, 7, 1.0)?.trunc();
    let mask = filter_mask(ctx, args.get(9), value_rows.len())?;
    let start = usize::from(has_headers);
    let rows: Vec<usize> = (start..value_rows.len()).filter(|&i| mask[i]).collect();
    if value_rows[0].len() != 1 {
        // One value field at a time.
        return Err(ErrorKind::Value);
    }
    let rk: Vec<Vec<Value>> = rows.iter().map(|&i| row_keys[i].clone()).collect();
    let ck: Vec<Vec<Value>> = rows.iter().map(|&i| col_keys[i].clone()).collect();
    let rgroups = distinct(&rk);
    let cgroups = distinct(&ck);
    let nrk = row_keys[0].len();
    let nck = col_keys[0].len();
    let cell = |ctx: &mut Ctx, r: Option<&Vec<Value>>, c: Option<&Vec<Value>>| -> Value {
        let vals: Vec<Value> = rows
            .iter()
            .filter(|&&i| r.is_none_or(|g| key_cmp(&row_keys[i], g) == Ordering::Equal))
            .filter(|&&i| c.is_none_or(|g| key_cmp(&col_keys[i], g) == Ordering::Equal))
            .map(|&i| value_rows[i][0].clone())
            .collect();
        if vals.is_empty() {
            return Value::Empty;
        }
        aggregate(ctx, &lam, vals)
    };
    let width = nrk + cgroups.len() + usize::from(col_total != 0.0);
    let mut out: Vec<Vec<Value>> = Vec::new();
    // Header rows: one per column field.
    for level in 0..nck {
        let mut h = vec![Value::Empty; nrk];
        h.extend(cgroups.iter().map(|g| g[level].clone()));
        if col_total != 0.0 {
            h.push(if level == 0 {
                total_label()
            } else {
                Value::Empty
            });
        }
        out.push(h);
    }
    let row_line = |ctx: &mut Ctx, key: Option<&Vec<Value>>| -> Vec<Value> {
        let mut line = if let Some(k) = key {
            k.clone()
        } else {
            let mut l = vec![total_label()];
            l.extend(std::iter::repeat_n(Value::Empty, nrk - 1));
            l
        };
        for g in &cgroups {
            line.push(cell(ctx, key, Some(g)));
        }
        if col_total != 0.0 {
            line.push(cell(ctx, key, None));
        }
        line
    };
    for g in &rgroups {
        out.push(row_line(ctx, Some(g)));
    }
    if row_total != 0.0 {
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

/// `GETPIVOTDATA`: the formula engine holds no pivot-table caches; a reference that is not
/// inside a pivot table is `#REF!` (§18.17.7 GETPIVOTDATA).
fn getpivotdata(_ctx: &mut Ctx, args: &[Value]) -> R {
    match &args[1] {
        Value::Error(e) => Err(*e),
        _ => Err(ErrorKind::Ref),
    }
}

/// `IMAGE(source, [alt_text], …)`: pictures are not fetched by the formula engine; the cell
/// shows the alternative text (or the source).
fn image(ctx: &mut Ctx, args: &[Value]) -> R {
    let source = ctx.text(&args[0])?;
    if source.is_empty() {
        return Err(ErrorKind::Value);
    }
    match args.get(1) {
        Some(v) if !matches!(v, Value::Empty) => Ok(Value::Text(ctx.text(v)?)),
        _ => Ok(Value::Text(source)),
    }
}

/// `RTD(prog_id, server, topic, …)`: no real-time data server is available → `#N/A`.
fn rtd(_ctx: &mut Ctx, _args: &[Value]) -> R {
    Err(ErrorKind::NA)
}
