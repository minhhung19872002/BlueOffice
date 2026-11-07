//! Logical functions, `LET` and `LAMBDA` helpers (ECMA-376 Part 1, §18.17.7).

use std::sync::Arc;

use super::{f, l, s, FnSpec, VAR};
use crate::ast::Expr;
use crate::eval::{elem, Ctx, R};
use crate::value::{compare_values, Array, ErrorKind, Lambda, Value};

pub(super) const FUNCTIONS: &[FnSpec] = &[
    l("IF", 1, 3, if_fn),
    l("IFS", 2, VAR, ifs),
    f("AND", 1, VAR, and),
    f("OR", 1, VAR, or),
    f("XOR", 1, VAR, xor),
    s("NOT", 1, 1, not),
    l("IFERROR", 2, 2, iferror),
    l("IFNA", 2, 2, ifna),
    l("SWITCH", 3, VAR, switch),
    f("TRUE", 0, 0, true_fn),
    f("FALSE", 0, 0, false_fn),
    l("LET", 3, VAR, let_fn),
    l("LAMBDA", 1, VAR, lambda),
    f("MAP", 2, VAR, map),
    f("REDUCE", 3, 3, reduce),
    f("SCAN", 3, 3, scan),
    f("BYROW", 2, 2, byrow),
    f("BYCOL", 2, 2, bycol),
    f("MAKEARRAY", 3, 3, makearray),
];

fn if_fn(ctx: &mut Ctx, args: &[Expr]) -> R {
    let cond = ctx.eval(&args[0]);
    let cond = ctx.deref(cond);
    let branch = |ctx: &mut Ctx, i: usize| -> Value {
        match args.get(i) {
            None => Value::Bool(false),
            Some(Expr::Missing) => Value::Number(0.0),
            Some(e) => ctx.eval(e),
        }
    };
    if let Value::Array(a) = &cond {
        let t = branch(ctx, 1);
        let t = ctx.deref(t);
        let e = branch(ctx, 2);
        let e = ctx.deref(e);
        let rows = a.rows();
        let cols = a.cols();
        let mut data = Vec::with_capacity(rows * cols);
        for r in 0..rows {
            for c in 0..cols {
                let v = match ctx.boolean(a.get(r, c)) {
                    Ok(true) => elem(&t, r, c).cloned(),
                    Ok(false) => elem(&e, r, c).cloned(),
                    Err(err) => Some(Value::Error(err)),
                };
                data.push(v.unwrap_or(Value::Error(ErrorKind::NA)));
            }
        }
        return Ok(Array::new(rows, cols, data).into());
    }
    if ctx.boolean(&cond)? {
        Ok(branch(ctx, 1))
    } else {
        Ok(branch(ctx, 2))
    }
}

fn ifs(ctx: &mut Ctx, args: &[Expr]) -> R {
    if !args.len().is_multiple_of(2) {
        return Err(ErrorKind::Value);
    }
    for pair in args.chunks(2) {
        let c = ctx.eval(&pair[0]);
        if ctx.boolean(&c)? {
            return Ok(ctx.eval(&pair[1]));
        }
    }
    Err(ErrorKind::NA)
}

/// Booleans from logical-function arguments (ranges: numbers/booleans only).
fn logicals(ctx: &Ctx, args: &[Value]) -> Result<Vec<bool>, ErrorKind> {
    let mut out = Vec::new();
    let mut err = None;
    for a in args {
        ctx.for_each_value(a, &mut |v, in_range| {
            if err.is_some() {
                return;
            }
            match v {
                Value::Bool(b) => out.push(*b),
                Value::Number(n) => out.push(*n != 0.0),
                Value::Error(e) => err = Some(*e),
                Value::Text(_) if !in_range => match v.as_bool() {
                    Ok(b) => out.push(b),
                    Err(e) => err = Some(e),
                },
                _ => {}
            }
        });
    }
    if let Some(e) = err {
        return Err(e);
    }
    if out.is_empty() {
        return Err(ErrorKind::Value);
    }
    Ok(out)
}

fn and(ctx: &mut Ctx, args: &[Value]) -> R {
    Ok(Value::Bool(logicals(ctx, args)?.iter().all(|b| *b)))
}

fn or(ctx: &mut Ctx, args: &[Value]) -> R {
    Ok(Value::Bool(logicals(ctx, args)?.iter().any(|b| *b)))
}

fn xor(ctx: &mut Ctx, args: &[Value]) -> R {
    Ok(Value::Bool(
        logicals(ctx, args)?.iter().filter(|b| **b).count() % 2 == 1,
    ))
}

fn not(ctx: &mut Ctx, args: &[Value]) -> R {
    Ok(Value::Bool(!ctx.boolean(&args[0])?))
}

fn on_error(ctx: &mut Ctx, args: &[Expr], only_na: bool) -> R {
    let v = ctx.eval(&args[0]);
    let v = ctx.deref(v);
    let hit = |x: &Value| match x {
        Value::Error(e) => !only_na || *e == ErrorKind::NA,
        _ => false,
    };
    match &v {
        Value::Array(a) => {
            if !a.data().iter().any(hit) {
                return Ok(v);
            }
            let fallback = ctx.eval(&args[1]);
            let fallback = ctx.deref(fallback);
            let data = a
                .data()
                .iter()
                .enumerate()
                .map(|(i, x)| {
                    if hit(x) {
                        elem(&fallback, i / a.cols(), i % a.cols())
                            .cloned()
                            .unwrap_or(Value::Error(ErrorKind::NA))
                    } else {
                        x.clone()
                    }
                })
                .collect();
            Ok(Array::new(a.rows(), a.cols(), data).into())
        }
        x if hit(x) => match &args[1] {
            Expr::Missing => Ok(Value::Number(0.0)),
            e => Ok(ctx.eval(e)),
        },
        Value::Empty => Ok(Value::Number(0.0)),
        _ => Ok(v),
    }
}

fn iferror(ctx: &mut Ctx, args: &[Expr]) -> R {
    on_error(ctx, args, false)
}

fn ifna(ctx: &mut Ctx, args: &[Expr]) -> R {
    on_error(ctx, args, true)
}

fn switch(ctx: &mut Ctx, args: &[Expr]) -> R {
    let v = ctx.eval(&args[0]);
    let v = ctx.scalar(&v);
    if let Value::Error(e) = v {
        return Err(e);
    }
    let rest = &args[1..];
    let pairs = rest.len() / 2;
    for i in 0..pairs {
        let candidate = ctx.eval(&rest[2 * i]);
        let candidate = ctx.scalar(&candidate);
        if let Value::Error(e) = candidate {
            return Err(e);
        }
        let same_type = std::mem::discriminant(&candidate) == std::mem::discriminant(&v);
        if same_type && compare_values(&candidate, &v).is_eq() {
            return Ok(ctx.eval(&rest[2 * i + 1]));
        }
    }
    if rest.len() % 2 == 1 {
        return Ok(ctx.eval(&rest[rest.len() - 1]));
    }
    Err(ErrorKind::NA)
}

fn true_fn(_ctx: &mut Ctx, _args: &[Value]) -> R {
    Ok(Value::Bool(true))
}

fn false_fn(_ctx: &mut Ctx, _args: &[Value]) -> R {
    Ok(Value::Bool(false))
}

fn let_fn(ctx: &mut Ctx, args: &[Expr]) -> R {
    ctx.eval_let(args)
}

fn lambda(ctx: &mut Ctx, args: &[Expr]) -> R {
    ctx.make_lambda(args)
}

fn as_lambda(v: &Value) -> Result<Arc<Lambda>, ErrorKind> {
    match v {
        Value::Lambda(l) => Ok(l.clone()),
        Value::Error(e) => Err(*e),
        _ => Err(ErrorKind::Value),
    }
}

fn scalar_result(ctx: &Ctx, v: Value) -> Value {
    match ctx.deref(v) {
        Value::Array(a) if a.rows() == 1 && a.cols() == 1 => a.get(0, 0).clone(),
        Value::Array(_) => Value::Error(ErrorKind::Calc),
        Value::Empty => Value::Number(0.0),
        v => v,
    }
}

fn map(ctx: &mut Ctx, args: &[Value]) -> R {
    let (lam, arrays) = args.split_last().ok_or(ErrorKind::Value)?;
    let lam = as_lambda(lam)?;
    let arrays: Vec<Arc<Array>> = arrays
        .iter()
        .map(|a| ctx.to_array(a.clone()))
        .collect::<Result<_, _>>()?;
    let rows = arrays.iter().map(|a| a.rows()).max().unwrap_or(1);
    let cols = arrays.iter().map(|a| a.cols()).max().unwrap_or(1);
    let mut data = Vec::with_capacity(rows * cols);
    for r in 0..rows {
        for c in 0..cols {
            let call: Option<Vec<Value>> = arrays
                .iter()
                .map(|a| a.get_broadcast(r, c).cloned())
                .collect();
            data.push(match call {
                Some(vals) => match ctx.invoke_lambda(&lam, vals) {
                    Ok(v) => scalar_result(ctx, v),
                    Err(e) => Value::Error(e),
                },
                None => Value::Error(ErrorKind::NA),
            });
        }
    }
    Ok(Array::new(rows, cols, data).into())
}

fn reduce(ctx: &mut Ctx, args: &[Value]) -> R {
    let lam = as_lambda(&args[2])?;
    let arr = ctx.to_array(args[1].clone())?;
    let mut acc = ctx.deref(args[0].clone());
    for v in arr.data() {
        acc = ctx.invoke_lambda(&lam, vec![acc, v.clone()])?;
        acc = ctx.deref(acc);
    }
    Ok(acc)
}

fn scan(ctx: &mut Ctx, args: &[Value]) -> R {
    let lam = as_lambda(&args[2])?;
    let arr = ctx.to_array(args[1].clone())?;
    let mut acc = ctx.deref(args[0].clone());
    let mut data = Vec::with_capacity(arr.data().len());
    for v in arr.data() {
        acc = ctx.invoke_lambda(&lam, vec![acc, v.clone()])?;
        acc = scalar_result(ctx, acc);
        data.push(acc.clone());
    }
    Ok(Array::new(arr.rows(), arr.cols(), data).into())
}

fn byrow(ctx: &mut Ctx, args: &[Value]) -> R {
    let lam = as_lambda(&args[1])?;
    let arr = ctx.to_array(args[0].clone())?;
    let mut data = Vec::with_capacity(arr.rows());
    for r in 0..arr.rows() {
        let row = Array::new(1, arr.cols(), arr.row_slice(r).to_vec());
        let v = ctx.invoke_lambda(&lam, vec![row.into()])?;
        data.push(scalar_result(ctx, v));
    }
    Ok(Array::new(arr.rows(), 1, data).into())
}

fn bycol(ctx: &mut Ctx, args: &[Value]) -> R {
    let lam = as_lambda(&args[1])?;
    let arr = ctx.to_array(args[0].clone())?;
    let mut data = Vec::with_capacity(arr.cols());
    for c in 0..arr.cols() {
        let col: Vec<Value> = (0..arr.rows()).map(|r| arr.get(r, c).clone()).collect();
        let v = ctx.invoke_lambda(&lam, vec![Array::new(arr.rows(), 1, col).into()])?;
        data.push(scalar_result(ctx, v));
    }
    Ok(Array::new(1, arr.cols(), data).into())
}

fn makearray(ctx: &mut Ctx, args: &[Value]) -> R {
    let rows = ctx.int(&args[0])?;
    let cols = ctx.int(&args[1])?;
    if rows < 1 || cols < 1 {
        return Err(ErrorKind::Value);
    }
    if rows * cols > 1 << 24 {
        return Err(ErrorKind::Num);
    }
    let lam = as_lambda(&args[2])?;
    let (rows, cols) = (rows as usize, cols as usize);
    let mut data = Vec::with_capacity(rows * cols);
    for r in 0..rows {
        for c in 0..cols {
            let v = ctx.invoke_lambda(
                &lam,
                vec![Value::Number((r + 1) as f64), Value::Number((c + 1) as f64)],
            )?;
            data.push(scalar_result(ctx, v));
        }
    }
    Ok(Array::new(rows, cols, data).into())
}
