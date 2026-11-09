//! Database functions (ECMA-376 Part 1, §18.17.7: DAVERAGE … DVARP).
//!
//! `database` is a range whose first row labels the fields; `field` is a label (case-insensitive)
//! or a 1-based column number; `criteria` is a range whose first row holds field labels and
//! whose other rows hold conditions — conditions in one row must all hold, any row may hold.
//! A blank condition matches every record.

use super::{f, num, shape, FnSpec};
use crate::criteria::Criterion;
use crate::eval::{Ctx, R};
use crate::value::{ErrorKind, Value};

pub(super) const FUNCTIONS: &[FnSpec] = &[
    f("DAVERAGE", 3, 3, daverage),
    f("DCOUNT", 2, 3, dcount),
    f("DCOUNTA", 2, 3, dcounta),
    f("DGET", 3, 3, dget),
    f("DMAX", 3, 3, dmax),
    f("DMIN", 3, 3, dmin),
    f("DPRODUCT", 3, 3, dproduct),
    f("DSTDEV", 3, 3, dstdev),
    f("DSTDEVP", 3, 3, dstdevp),
    f("DSUM", 3, 3, dsum),
    f("DVAR", 3, 3, dvar),
    f("DVARP", 3, 3, dvarp),
];

/// A range as rows of values.
fn rows(ctx: &Ctx, v: &Value) -> Vec<Vec<Value>> {
    let (r, c) = shape(ctx, v);
    let flat = super::flatten(ctx, v);
    (0..r).map(|i| flat[i * c..(i + 1) * c].to_vec()).collect()
}

fn label(v: &Value) -> String {
    match v {
        Value::Text(t) => t.trim().to_lowercase(),
        Value::Number(n) => crate::value::number_to_text(*n),
        Value::Bool(b) => if *b { "true" } else { "false" }.to_owned(),
        _ => String::new(),
    }
}

/// Index of the field named or numbered by `field` (`None` when the argument is omitted).
fn field_index(
    ctx: &Ctx,
    header: &[Value],
    field: Option<&Value>,
) -> Result<Option<usize>, ErrorKind> {
    let Some(field) = field else { return Ok(None) };
    match ctx.scalar(field) {
        Value::Empty => Ok(None),
        Value::Number(n) => {
            let i = n.trunc();
            if i < 1.0 || i > header.len() as f64 {
                return Err(ErrorKind::Value);
            }
            Ok(Some(i as usize - 1))
        }
        Value::Error(e) => Err(e),
        v => {
            let name = label(&v);
            header
                .iter()
                .position(|h| label(h) == name)
                .map(Some)
                .ok_or(ErrorKind::Value)
        }
    }
}

/// Records (rows below the header) matching the criteria.
fn matching(ctx: &Ctx, db: &[Vec<Value>], crit: &[Vec<Value>]) -> Result<Vec<usize>, ErrorKind> {
    let header = &db[0];
    let Some(crit_header) = crit.first() else {
        return Err(ErrorKind::Value);
    };
    // Each criteria column: the database column it tests (`None`: an unknown label).
    let cols: Vec<Option<usize>> = crit_header
        .iter()
        .map(|h| {
            let name = label(h);
            header
                .iter()
                .position(|d| !name.is_empty() && label(d) == name)
        })
        .collect();
    let conditions: Vec<Vec<(Option<usize>, Option<Criterion>)>> = crit[1..]
        .iter()
        .map(|row| {
            row.iter()
                .zip(&cols)
                .map(|(c, col)| {
                    let blank =
                        matches!(c, Value::Empty) || matches!(c, Value::Text(t) if t.is_empty());
                    (*col, (!blank).then(|| Criterion::parse(c, ctx.date_system)))
                })
                .collect()
        })
        .collect();
    let mut out = Vec::new();
    for (i, rec) in db.iter().enumerate().skip(1) {
        let hit = if conditions.is_empty() {
            true
        } else {
            conditions.iter().any(|row| {
                row.iter().all(|(col, cond)| match (col, cond) {
                    (_, None) => true,
                    (Some(c), Some(cond)) => cond.matches(&rec[*c]),
                    (None, Some(_)) => false,
                })
            })
        };
        if hit {
            out.push(i);
        }
    }
    Ok(out)
}

/// The values of `field` in the matching records.
fn selected(ctx: &Ctx, args: &[Value]) -> Result<(Vec<Value>, usize), ErrorKind> {
    let db = rows(ctx, &args[0]);
    if db.is_empty() {
        return Err(ErrorKind::Value);
    }
    let crit_arg = args.get(2).unwrap_or(&Value::Empty);
    let crit = rows(ctx, crit_arg);
    let field = field_index(ctx, &db[0], args.get(1))?;
    let hits = matching(ctx, &db, &crit)?;
    let n = hits.len();
    let values = match field {
        Some(c) => hits.iter().map(|&i| db[i][c].clone()).collect(),
        None => Vec::new(),
    };
    Ok((values, n))
}

fn numbers(values: &[Value]) -> Result<Vec<f64>, ErrorKind> {
    let mut out = Vec::new();
    for v in values {
        match v {
            Value::Number(n) => out.push(*n),
            Value::Error(e) => return Err(*e),
            _ => {}
        }
    }
    Ok(out)
}

fn field_numbers(ctx: &Ctx, args: &[Value]) -> Result<Vec<f64>, ErrorKind> {
    numbers(&selected(ctx, args)?.0)
}

fn daverage(ctx: &mut Ctx, args: &[Value]) -> R {
    let v = field_numbers(ctx, args)?;
    if v.is_empty() {
        return Err(ErrorKind::Div0);
    }
    num(v.iter().sum::<f64>() / v.len() as f64)
}

fn dcount(ctx: &mut Ctx, args: &[Value]) -> R {
    let (values, records) = selected(ctx, args)?;
    let field_given = !matches!(
        args.get(1).map(|v| ctx.scalar(v)),
        None | Some(Value::Empty)
    );
    if !field_given {
        return Ok(Value::number(records as f64));
    }
    Ok(Value::number(
        values
            .iter()
            .filter(|v| matches!(v, Value::Number(_)))
            .count() as f64,
    ))
}

fn dcounta(ctx: &mut Ctx, args: &[Value]) -> R {
    let (values, records) = selected(ctx, args)?;
    let field_given = !matches!(
        args.get(1).map(|v| ctx.scalar(v)),
        None | Some(Value::Empty)
    );
    if !field_given {
        return Ok(Value::number(records as f64));
    }
    Ok(Value::number(
        values.iter().filter(|v| !matches!(v, Value::Empty)).count() as f64,
    ))
}

fn dget(ctx: &mut Ctx, args: &[Value]) -> R {
    let (values, _) = selected(ctx, args)?;
    match values.as_slice() {
        [] => Err(ErrorKind::Value),
        [v] => Ok(v.clone()),
        _ => Err(ErrorKind::Num),
    }
}

fn dmax(ctx: &mut Ctx, args: &[Value]) -> R {
    let v = field_numbers(ctx, args)?;
    num(if v.is_empty() {
        0.0
    } else {
        v.iter().copied().fold(f64::MIN, f64::max)
    })
}

fn dmin(ctx: &mut Ctx, args: &[Value]) -> R {
    let v = field_numbers(ctx, args)?;
    num(if v.is_empty() {
        0.0
    } else {
        v.iter().copied().fold(f64::MAX, f64::min)
    })
}

fn dproduct(ctx: &mut Ctx, args: &[Value]) -> R {
    let v = field_numbers(ctx, args)?;
    num(if v.is_empty() {
        0.0
    } else {
        v.iter().product()
    })
}

fn dsum(ctx: &mut Ctx, args: &[Value]) -> R {
    num(field_numbers(ctx, args)?.iter().sum())
}

fn variance(v: &[f64], sample: bool) -> Result<f64, ErrorKind> {
    let n = v.len() as f64;
    let d = if sample { n - 1.0 } else { n };
    if d < 1.0 {
        return Err(ErrorKind::Div0);
    }
    let m = v.iter().sum::<f64>() / n;
    Ok(v.iter().map(|x| (x - m) * (x - m)).sum::<f64>() / d)
}

fn dstdev(ctx: &mut Ctx, args: &[Value]) -> R {
    num(variance(&field_numbers(ctx, args)?, true)?.sqrt())
}

fn dstdevp(ctx: &mut Ctx, args: &[Value]) -> R {
    num(variance(&field_numbers(ctx, args)?, false)?.sqrt())
}

fn dvar(ctx: &mut Ctx, args: &[Value]) -> R {
    num(variance(&field_numbers(ctx, args)?, true)?)
}

fn dvarp(ctx: &mut Ctx, args: &[Value]) -> R {
    num(variance(&field_numbers(ctx, args)?, false)?)
}
