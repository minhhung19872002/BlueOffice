//! Step-by-step evaluation of a formula (the "evaluate formula" auditing tool).
//!
//! The formula is reduced one sub-expression at a time, innermost first and
//! left to right, the order in which operands are evaluated (ECMA-376 Part 1,
//! §18.17.2 operators, §18.17.7 functions). Each reduced part is replaced by a
//! variable bound to its value, so later steps are evaluated by the ordinary
//! evaluator with exactly the same semantics (a reference stays a reference
//! for the function receiving it). Only the branch an `IF`, `IFERROR`, `IFNA`
//! or `CHOOSE` takes is stepped into; `LET`/`LAMBDA` helpers are one step.
//!
//! Normal evaluation is untouched: tracing is a separate walk over a copy of
//! the expression, run only on request.

use std::sync::atomic::AtomicU64;

use crate::ast::{BinaryOp, Expr, UnaryOp};
use crate::eval::{Ctx, EvalConfig};
use crate::format::{format, format_spans};
use crate::provider::CellProvider;
use crate::reference::{CellAddr, RefKind, Reference, SheetRef};
use crate::value::{Area, ErrorKind, Value};
use crate::workbook::CellId;

/// Upper bound on recorded steps (very long formulas).
const MAX_STEPS: usize = 2000;
/// Upper bound on array elements written into a step's text.
const MAX_ARRAY_SHOWN: usize = 64;
/// First character of the variables standing in for evaluated parts (never
/// produced by the parser).
const MARK: char = '\u{1}';

/// Functions evaluated as a whole: their arguments use local variables.
const OPAQUE: &[&str] = &[
    "LET",
    "LAMBDA",
    "MAP",
    "REDUCE",
    "SCAN",
    "BYROW",
    "BYCOL",
    "MAKEARRAY",
    "ISOMITTED",
];

/// One step: the formula text split around the part evaluated next.
#[derive(Clone, Debug, PartialEq)]
pub struct TraceStep {
    /// Text before the part (formula without the leading `=`).
    pub before: String,
    /// The part evaluated by this step (shown underlined).
    pub part: String,
    /// Text after the part.
    pub after: String,
    /// Value of the part (single-cell references are dereferenced).
    pub value: Value,
    /// The value as written into the next step's text.
    pub shown: String,
    /// The part is a reference to this cell, which holds a formula
    /// ("step in" evaluates that formula).
    pub cell: Option<CellId>,
}

/// Result of [`trace_formula`].
#[derive(Clone, Debug, PartialEq)]
pub struct FormulaTrace {
    /// Steps in evaluation order.
    pub steps: Vec<TraceStep>,
    /// Final value of the formula (as stored in the cell).
    pub result: Value,
    /// The final value as text.
    pub shown: String,
    /// The step limit was reached; the result is still exact.
    pub truncated: bool,
}

/// Evaluate `expr` as the formula of cell (`sheet`, `row`, `col`) step by step.
pub fn trace_formula(
    expr: &Expr,
    provider: &dyn CellProvider,
    sheet: usize,
    row: u32,
    col: u32,
    config: &EvalConfig,
) -> FormulaTrace {
    let rng = AtomicU64::new(0x9E37_79B9_7F4A_7C15);
    let mut ctx = Ctx::new(provider, sheet, row, col, config, &rng);
    let mut work = expr.clone();
    // Text shown for each bound variable, by index.
    let mut shown: Vec<String> = Vec::new();
    let mut steps = Vec::new();
    let mut truncated = false;
    while let Some(path) = next(&work, &mut ctx) {
        if steps.len() >= MAX_STEPS {
            truncated = true;
            break;
        }
        let node = get(&work, &path).clone();
        let v = ctx.eval(&node);
        let silent = match (&v, &node) {
            (Value::Ref(a), _) => !a.is_single(),
            (Value::Refs(_), _) => true,
            (_, Expr::Unary(UnaryOp::Neg | UnaryOp::Plus, inner)) => {
                matches!(**inner, Expr::Number(_))
            }
            _ => false,
        };
        let text = if silent {
            match &v {
                Value::Ref(_) | Value::Refs(_) => format(&display(&node, &shown)),
                other => render(other, &ctx),
            }
        } else {
            render(&v, &ctx)
        };
        if !silent {
            let (full, spans) = format_spans(&display(&work, &shown));
            let (s, e) = spans
                .get(preorder(&work, &path))
                .copied()
                .unwrap_or((0, full.len()));
            let cell = match (&node, &v) {
                (Expr::Ref(_) | Expr::Name { .. }, Value::Ref(a)) if a.is_single() => provider
                    .cell_formula(a.sheet, a.r1, a.c1)
                    .map(|_| CellId::new(a.sheet, a.r1, a.c1)),
                _ => None,
            };
            steps.push(TraceStep {
                before: full.get(..s).unwrap_or_default().to_owned(),
                part: full.get(s..e).unwrap_or_default().to_owned(),
                after: full.get(e..).unwrap_or_default().to_owned(),
                value: ctx.deref(v.clone()),
                shown: text.clone(),
                cell,
            });
        }
        let name = format!("{MARK}{}", shown.len());
        shown.push(text);
        ctx.bind(name.clone(), v);
        replace_at(&mut work, &path, Expr::Name { sheet: None, name });
    }
    let raw = ctx.eval(expr);
    let result = ctx.finalize(raw);
    let shown = render(&result, &ctx);
    FormulaTrace {
        steps,
        result,
        shown,
        truncated,
    }
}

/// Whether a node needs no evaluation step.
fn is_done(e: &Expr) -> bool {
    match e {
        Expr::Number(_)
        | Expr::Text(_)
        | Expr::Bool(_)
        | Expr::Error(_)
        | Expr::Missing
        | Expr::Array(_) => true,
        Expr::Name { sheet: None, name } => name.starts_with(MARK),
        _ => false,
    }
}

/// Path (child indexes) of the next part to evaluate, or `None` when done.
fn next(e: &Expr, ctx: &mut Ctx) -> Option<Vec<usize>> {
    if is_done(e) {
        return None;
    }
    let first = |children: &[&Expr], ctx: &mut Ctx| -> Option<Vec<usize>> {
        children.iter().enumerate().find_map(|(i, c)| {
            next(c, ctx).map(|mut p| {
                p.insert(0, i);
                p
            })
        })
    };
    let inner = match e {
        // Reference operators work on the references themselves: one step.
        Expr::Unary(UnaryOp::SpillRange, _) => None,
        Expr::Binary(op, ..) if op.precedence() >= BinaryOp::Union.precedence() => None,
        Expr::Unary(_, a) => first(&[a], ctx),
        Expr::Binary(_, a, b) => first(&[a, b], ctx),
        Expr::Call { name, args } => {
            if OPAQUE.contains(&name.as_str()) {
                return Some(Vec::new());
            }
            let order = arg_order(name, args, ctx);
            order.into_iter().find_map(|i| {
                next(&args[i], ctx).map(|mut p| {
                    p.insert(0, i);
                    p
                })
            })
        }
        _ => None,
    };
    Some(inner.unwrap_or_default())
}

/// Arguments of a call that are evaluated, in order. The condition / value /
/// index of `IF`, `IFERROR`, `IFNA` and `CHOOSE` comes first; once it is known
/// only the argument the function uses follows.
fn arg_order(name: &str, args: &[Expr], ctx: &mut Ctx) -> Vec<usize> {
    let all = || (0..args.len()).collect::<Vec<_>>();
    let Some(head) = args.first() else {
        return Vec::new();
    };
    if !matches!(name, "IF" | "IFERROR" | "IFNA" | "CHOOSE") {
        return all();
    }
    if !is_done(head) {
        return vec![0];
    }
    let v = ctx.eval(head);
    let v = ctx.deref(v);
    if matches!(v, Value::Array(_)) {
        // Array conditions use every argument.
        return all();
    }
    let pick = match name {
        "IF" => match ctx.boolean(&v) {
            Ok(true) => Some(1),
            Ok(false) => Some(2),
            Err(_) => None,
        },
        "IFERROR" => matches!(v, Value::Error(_)).then_some(1),
        "IFNA" => matches!(v, Value::Error(ErrorKind::NA)).then_some(1),
        _ => ctx
            .int(&v)
            .ok()
            .and_then(|n| usize::try_from(n).ok())
            .filter(|&n| n >= 1),
    };
    pick.filter(|&i| i < args.len()).into_iter().collect()
}

fn child(e: &Expr, i: usize) -> Option<&Expr> {
    match e {
        Expr::Unary(_, a) => (i == 0).then_some(&**a),
        Expr::Binary(_, a, b) => match i {
            0 => Some(a),
            1 => Some(b),
            _ => None,
        },
        Expr::Call { args, .. } => args.get(i),
        _ => None,
    }
}

/// Replace the node at `path` with `new`.
fn replace_at(e: &mut Expr, path: &[usize], new: Expr) {
    let Some((&i, rest)) = path.split_first() else {
        *e = new;
        return;
    };
    match e {
        Expr::Unary(_, a) if i == 0 => replace_at(a, rest, new),
        Expr::Binary(_, a, _) if i == 0 => replace_at(a, rest, new),
        Expr::Binary(_, _, b) if i == 1 => replace_at(b, rest, new),
        Expr::Call { args, .. } => {
            if let Some(c) = args.get_mut(i) {
                replace_at(c, rest, new);
            }
        }
        _ => {}
    }
}

fn get<'e>(mut e: &'e Expr, path: &[usize]) -> &'e Expr {
    for &i in path {
        match child(e, i) {
            Some(c) => e = c,
            None => break,
        }
    }
    e
}

/// Number of nodes of a subtree (the order of [`Expr::walk`]).
fn size(e: &Expr) -> usize {
    let mut n = 0;
    e.walk(&mut |_| n += 1);
    n
}

/// Pre-order index of the node at `path`.
fn preorder(mut e: &Expr, path: &[usize]) -> usize {
    let mut ix = 0;
    for &i in path {
        ix += 1;
        for j in 0..i {
            if let Some(c) = child(e, j) {
                ix += size(c);
            }
        }
        match child(e, i) {
            Some(c) => e = c,
            None => break,
        }
    }
    ix
}

/// The expression with every evaluated part written as its value.
fn display(e: &Expr, shown: &[String]) -> Expr {
    e.map(&mut |x| match x {
        Expr::Name { sheet: None, name } if name.starts_with(MARK) => {
            let text = name[MARK.len_utf8()..]
                .parse::<usize>()
                .ok()
                .and_then(|i| shown.get(i))
                .cloned()
                .unwrap_or_default();
            Expr::Name {
                sheet: None,
                name: text,
            }
        }
        other => other,
    })
}

/// A value written as a formula constant.
fn render(v: &Value, ctx: &Ctx) -> String {
    match v {
        Value::Ref(a) if a.is_single() => {
            render(&ctx.provider.cell_value(a.sheet, a.r1, a.c1), ctx)
        }
        Value::Ref(a) => area_text(a, ctx),
        Value::Refs(list) => list
            .iter()
            .map(|a| area_text(a, ctx))
            .collect::<Vec<_>>()
            .join(","),
        Value::Empty => "\"\"".to_owned(),
        Value::Number(n) => format(&Expr::Number(*n)),
        Value::Text(s) => format(&Expr::Text(s.clone())),
        Value::Bool(b) => format(&Expr::Bool(*b)),
        Value::Error(e) => e.as_str().to_owned(),
        Value::Array(a) => {
            let mut out = String::from("{");
            let mut n = 0;
            'rows: for r in 0..a.rows() {
                if r > 0 {
                    out.push(';');
                }
                for c in 0..a.cols() {
                    if c > 0 {
                        out.push(',');
                    }
                    n += 1;
                    if n > MAX_ARRAY_SHOWN {
                        out.push('…');
                        break 'rows;
                    }
                    out.push_str(&render(a.get(r, c), ctx));
                }
            }
            out.push('}');
            out
        }
        Value::Lambda(_) => "LAMBDA".to_owned(),
        Value::Image(i) => format!("IMAGE(\"{}\")", i.source),
    }
}

/// A1 text of an area, sheet-qualified when not on the formula's sheet.
fn area_text(a: &Area, ctx: &Ctx) -> String {
    let sheet = (a.sheet != ctx.sheet)
        .then(|| ctx.provider.sheet_name(a.sheet).map(SheetRef::single))
        .flatten();
    let kind = if a.is_single() {
        RefKind::Cell(CellAddr::new(a.r1, a.c1))
    } else {
        RefKind::Range(CellAddr::new(a.r1, a.c1), CellAddr::new(a.r2, a.c2))
    };
    format(&Expr::Ref(Reference { sheet, kind }))
}
