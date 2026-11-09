//! Information functions (ECMA-376 Part 1, §18.17.7).

use super::{f, l, s, FnSpec};
use crate::ast::Expr;
use crate::eval::{Ctx, R};
use crate::reference::CellAddr;
use crate::value::{ErrorKind, Value};

pub(super) const FUNCTIONS: &[FnSpec] = &[
    s("ISBLANK", 1, 1, isblank),
    s("ISNUMBER", 1, 1, isnumber),
    s("ISTEXT", 1, 1, istext),
    s("ISNONTEXT", 1, 1, isnontext),
    s("ISERROR", 1, 1, iserror),
    s("ISERR", 1, 1, iserr),
    s("ISNA", 1, 1, isna),
    s("ISLOGICAL", 1, 1, islogical),
    f("ISREF", 1, 1, isref),
    f("ISFORMULA", 1, 1, isformula),
    s("ISEVEN", 1, 1, iseven),
    s("ISODD", 1, 1, isodd),
    f("TYPE", 1, 1, type_fn),
    s("N", 1, 1, n),
    f("NA", 0, 0, na),
    f("CELL", 1, 2, cell),
    s("ERROR.TYPE", 1, 1, error_type),
    f("SHEET", 0, 1, sheet),
    f("SHEETS", 0, 1, sheets),
    f("FORMULATEXT", 1, 1, formulatext),
    s("INFO", 1, 1, info),
    l("ISOMITTED", 1, 1, isomitted),
];

/// `INFO(type)`: facts about the operating environment (§18.17.7 INFO). BlueOffice reports
/// itself; the working directory is not exposed.
fn info(ctx: &mut Ctx, args: &[Value]) -> R {
    let kind = ctx.text(&args[0])?.to_lowercase();
    Ok(match kind.as_str() {
        "directory" => Value::Text(String::new()),
        "numfile" => Value::number(ctx.provider.sheet_count() as f64),
        "origin" => Value::Text("$A:$A$1".to_owned()),
        "osversion" => Value::Text("BlueOffice".to_owned()),
        "recalc" => Value::Text("Automatic".to_owned()),
        "release" => Value::Text(env!("CARGO_PKG_VERSION").to_owned()),
        "system" => Value::Text("pcdos".to_owned()),
        _ => return Err(ErrorKind::Value),
    })
}

/// `ISOMITTED(parameter)`: whether a `LAMBDA` parameter was left out of the call.
fn isomitted(ctx: &mut Ctx, args: &[Expr]) -> R {
    match &args[0] {
        Expr::Name { sheet: None, name } => Ok(Value::Bool(ctx.is_omitted(name))),
        Expr::Missing => Ok(Value::Bool(true)),
        _ => Ok(Value::Bool(false)),
    }
}

fn isblank(_ctx: &mut Ctx, args: &[Value]) -> R {
    Ok(Value::Bool(matches!(args[0], Value::Empty)))
}

fn isnumber(_ctx: &mut Ctx, args: &[Value]) -> R {
    Ok(Value::Bool(matches!(args[0], Value::Number(_))))
}

fn istext(_ctx: &mut Ctx, args: &[Value]) -> R {
    Ok(Value::Bool(matches!(args[0], Value::Text(_))))
}

fn isnontext(_ctx: &mut Ctx, args: &[Value]) -> R {
    Ok(Value::Bool(!matches!(args[0], Value::Text(_))))
}

fn iserror(_ctx: &mut Ctx, args: &[Value]) -> R {
    Ok(Value::Bool(matches!(args[0], Value::Error(_))))
}

fn iserr(_ctx: &mut Ctx, args: &[Value]) -> R {
    Ok(Value::Bool(
        matches!(args[0], Value::Error(e) if e != ErrorKind::NA),
    ))
}

fn isna(_ctx: &mut Ctx, args: &[Value]) -> R {
    Ok(Value::Bool(matches!(args[0], Value::Error(ErrorKind::NA))))
}

fn islogical(_ctx: &mut Ctx, args: &[Value]) -> R {
    Ok(Value::Bool(matches!(args[0], Value::Bool(_))))
}

fn isref(_ctx: &mut Ctx, args: &[Value]) -> R {
    Ok(Value::Bool(matches!(
        args[0],
        Value::Ref(_) | Value::Refs(_)
    )))
}

fn isformula(ctx: &mut Ctx, args: &[Value]) -> R {
    match &args[0] {
        Value::Ref(a) => Ok(Value::Bool(
            ctx.provider.cell_formula(a.sheet, a.r1, a.c1).is_some(),
        )),
        Value::Error(e) => Err(*e),
        _ => Err(ErrorKind::Value),
    }
}

fn parity(ctx: &Ctx, v: &Value) -> Result<bool, ErrorKind> {
    let n = match v {
        Value::Bool(_) => return Err(ErrorKind::Value),
        other => ctx.num(other)?,
    };
    Ok(n.abs().trunc() % 2.0 == 0.0)
}

fn iseven(ctx: &mut Ctx, args: &[Value]) -> R {
    Ok(Value::Bool(parity(ctx, &args[0])?))
}

fn isodd(ctx: &mut Ctx, args: &[Value]) -> R {
    Ok(Value::Bool(!parity(ctx, &args[0])?))
}

fn type_fn(ctx: &mut Ctx, args: &[Value]) -> R {
    let v = match &args[0] {
        Value::Ref(a) if !a.is_single() => return Ok(Value::Number(64.0)),
        Value::Refs(_) => return Ok(Value::Number(16.0)),
        other => ctx.deref(other.clone()),
    };
    Ok(Value::Number(match v {
        Value::Number(_) | Value::Empty => 1.0,
        Value::Text(_) => 2.0,
        Value::Bool(_) => 4.0,
        Value::Error(_) => 16.0,
        Value::Array(_) => 64.0,
        _ => 128.0,
    }))
}

fn n(_ctx: &mut Ctx, args: &[Value]) -> R {
    match &args[0] {
        Value::Number(n) => Ok(Value::Number(*n)),
        Value::Bool(b) => Ok(Value::Number(if *b { 1.0 } else { 0.0 })),
        Value::Error(e) => Err(*e),
        _ => Ok(Value::Number(0.0)),
    }
}

fn na(_ctx: &mut Ctx, _args: &[Value]) -> R {
    Err(ErrorKind::NA)
}

fn cell(ctx: &mut Ctx, args: &[Value]) -> R {
    let info = ctx.text(&args[0])?.to_lowercase();
    let area = match args.get(1) {
        Some(Value::Ref(a)) => *a,
        Some(Value::Error(e)) => return Err(*e),
        Some(Value::Empty) | None => crate::value::Area::cell(ctx.sheet, ctx.row, ctx.col),
        Some(_) => return Err(ErrorKind::Value),
    };
    let v = ctx.provider.cell_value(area.sheet, area.r1, area.c1);
    match info.as_str() {
        "address" => {
            let mut s = String::new();
            if area.sheet != ctx.sheet {
                if let Some(name) = ctx.provider.sheet_name(area.sheet) {
                    s.push('\'');
                    s.push_str(&name.replace('\'', "''"));
                    s.push_str("'!");
                }
            }
            s.push_str(&CellAddr::absolute(area.r1, area.c1).to_a1());
            Ok(Value::Text(s))
        }
        "row" => Ok(Value::Number(f64::from(area.r1 + 1))),
        "col" => Ok(Value::Number(f64::from(area.c1 + 1))),
        "contents" => Ok(match v {
            Value::Empty => Value::Number(0.0),
            v => v,
        }),
        "type" => Ok(Value::Text(
            match v {
                Value::Empty => "b",
                Value::Text(_) => "l",
                _ => "v",
            }
            .to_owned(),
        )),
        "sheet" => Ok(Value::Number((area.sheet + 1) as f64)),
        _ => Err(ErrorKind::Value),
    }
}

fn error_type(_ctx: &mut Ctx, args: &[Value]) -> R {
    match &args[0] {
        Value::Error(e) => Ok(Value::Number(e.type_code())),
        _ => Err(ErrorKind::NA),
    }
}

fn sheet(ctx: &mut Ctx, args: &[Value]) -> R {
    match args.first() {
        None | Some(Value::Empty) => Ok(Value::Number((ctx.sheet + 1) as f64)),
        Some(Value::Ref(a)) => Ok(Value::Number((a.sheet + 1) as f64)),
        Some(Value::Refs(l)) => Ok(Value::Number((l[0].sheet + 1) as f64)),
        Some(Value::Text(t)) => ctx
            .provider
            .sheet_index(t)
            .map(|i| Value::Number((i + 1) as f64))
            .ok_or(ErrorKind::NA),
        Some(Value::Error(e)) => Err(*e),
        Some(_) => Err(ErrorKind::Value),
    }
}

fn sheets(ctx: &mut Ctx, args: &[Value]) -> R {
    match args.first() {
        None | Some(Value::Empty) => Ok(Value::Number(ctx.provider.sheet_count() as f64)),
        Some(Value::Ref(_)) => Ok(Value::Number(1.0)),
        Some(Value::Refs(l)) => {
            let mut sheets: Vec<usize> = l.iter().map(|a| a.sheet).collect();
            sheets.sort_unstable();
            sheets.dedup();
            Ok(Value::Number(sheets.len() as f64))
        }
        Some(Value::Error(e)) => Err(*e),
        Some(_) => Err(ErrorKind::Value),
    }
}

fn formulatext(ctx: &mut Ctx, args: &[Value]) -> R {
    match &args[0] {
        Value::Ref(a) => ctx
            .provider
            .cell_formula(a.sheet, a.r1, a.c1)
            .map(|f| {
                Value::Text(if f.starts_with('=') {
                    f
                } else {
                    format!("={f}")
                })
            })
            .ok_or(ErrorKind::NA),
        Value::Error(e) => Err(*e),
        _ => Err(ErrorKind::Value),
    }
}
