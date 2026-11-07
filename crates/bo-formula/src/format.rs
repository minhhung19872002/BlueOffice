//! Formatter: [`Expr`] → formula text (ECMA-376 Part 1, §18.17.2 formula grammar).
//!
//! Parentheses are emitted only where precedence requires them, so
//! `parse(format(e)) == e` for every parsed expression.

use crate::ast::{
    BinaryOp, Expr, StructItem, StructRef, UnaryOp, PERCENT_PRECEDENCE, PREFIX_PRECEDENCE,
};
use crate::parser::{ParseOptions, RefStyle};
use crate::reference::{col_to_letters, letters_to_col, CellAddr, RefKind, Reference, SheetRef};

/// Format an expression in A1 style (without the leading `=`).
pub fn format(expr: &Expr) -> String {
    format_with(expr, &ParseOptions::default())
}

/// Format an expression with the given reference style and origin cell.
pub fn format_with(expr: &Expr, opts: &ParseOptions) -> String {
    let mut out = String::new();
    write_expr(&mut out, expr, opts);
    out
}

fn precedence(e: &Expr) -> u8 {
    match e {
        Expr::Binary(op, ..) => op.precedence(),
        Expr::Unary(UnaryOp::Percent, _) => PERCENT_PRECEDENCE,
        Expr::Unary(..) => PREFIX_PRECEDENCE,
        _ => 100,
    }
}

fn write_child(out: &mut String, e: &Expr, opts: &ParseOptions, paren: bool) {
    if paren {
        out.push('(');
        write_expr(out, e, opts);
        out.push(')');
    } else {
        write_expr(out, e, opts);
    }
}

fn write_number(out: &mut String, n: f64) {
    if n != 0.0 && (n.abs() >= 1e15 || n.abs() < 1e-5) {
        out.push_str(&format!("{n:e}"));
    } else {
        out.push_str(&format!("{n}"));
    }
}

fn write_string(out: &mut String, s: &str) {
    out.push('"');
    out.push_str(&s.replace('"', "\"\""));
    out.push('"');
}

fn write_expr(out: &mut String, e: &Expr, opts: &ParseOptions) {
    match e {
        Expr::Number(n) => write_number(out, *n),
        Expr::Text(s) => write_string(out, s),
        Expr::Bool(b) => out.push_str(if *b { "TRUE" } else { "FALSE" }),
        Expr::Error(err) => out.push_str(err.as_str()),
        Expr::Array(rows) => {
            out.push('{');
            for (i, row) in rows.iter().enumerate() {
                if i > 0 {
                    out.push(';');
                }
                for (j, el) in row.iter().enumerate() {
                    if j > 0 {
                        out.push(',');
                    }
                    write_expr(out, el, opts);
                }
            }
            out.push('}');
        }
        Expr::Ref(r) => out.push_str(&format_reference(r, opts)),
        Expr::Name { sheet, name } => {
            if let Some(s) = sheet {
                write_sheet(out, s);
            }
            out.push_str(name);
        }
        Expr::Structured(s) => write_struct(out, s),
        Expr::Unary(op, inner) => {
            let ip = precedence(inner);
            match op {
                UnaryOp::Percent => {
                    // Parenthesize anything binding looser than `%`, including prefix ops
                    // (`(-1)%` must not become `-1%`, which parses as `-(1%)`).
                    write_child(out, inner, opts, ip <= PREFIX_PRECEDENCE);
                    out.push('%');
                }
                UnaryOp::Neg | UnaryOp::Plus | UnaryOp::ImplicitIntersection => {
                    out.push(match op {
                        UnaryOp::Neg => '-',
                        UnaryOp::Plus => '+',
                        _ => '@',
                    });
                    let need = if *op == UnaryOp::ImplicitIntersection {
                        ip <= PERCENT_PRECEDENCE
                    } else {
                        ip < PREFIX_PRECEDENCE
                    };
                    write_child(out, inner, opts, need);
                }
            }
        }
        Expr::Binary(op, a, b) => {
            let p = op.precedence();
            // Union is always parenthesized: a bare `,` would read as an argument separator.
            let wrap = *op == BinaryOp::Union;
            if wrap {
                out.push('(');
            }
            write_child(out, a, opts, precedence(a) < p);
            out.push_str(op.symbol());
            // Left-associative: an equal-precedence right operand needs parentheses.
            // A prefix operator on the right binds its whole tail, so it is wrapped too
            // when the outer operator binds tighter than the prefix.
            let bp = precedence(b);
            let rhs_paren = bp <= p
                || (matches!(**b, Expr::Unary(UnaryOp::Neg | UnaryOp::Plus, _))
                    && p > PREFIX_PRECEDENCE);
            write_child(out, b, opts, rhs_paren);
            if wrap {
                out.push(')');
            }
        }
        Expr::Call { name, args } => {
            out.push_str(name);
            write_args(out, args, opts);
        }
        Expr::Invoke { callee, args } => {
            let paren = !matches!(**callee, Expr::Call { .. } | Expr::Invoke { .. });
            write_child(out, callee, opts, paren);
            write_args(out, args, opts);
        }
        Expr::Missing => {}
    }
}

fn write_args(out: &mut String, args: &[Expr], opts: &ParseOptions) {
    out.push('(');
    for (i, a) in args.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        write_expr(out, a, opts);
    }
    out.push(')');
}

/// Whether a sheet name must be quoted in a reference.
fn needs_quotes(name: &str) -> bool {
    if name.is_empty() {
        return true;
    }
    let first = name.chars().next().unwrap_or(' ');
    if first.is_ascii_digit() || first == '.' {
        return true;
    }
    if !name
        .chars()
        .all(|c| c.is_alphanumeric() || c == '_' || c == '.')
    {
        return true;
    }
    // Names that look like cell references (A1, R1C1) or booleans need quoting.
    let up = name.to_ascii_uppercase();
    if up == "TRUE" || up == "FALSE" || up == "R" || up == "C" {
        return true;
    }
    let letters: String = up.chars().take_while(char::is_ascii_alphabetic).collect();
    let rest = &up[letters.len()..];
    if !letters.is_empty()
        && letters_to_col(&letters).is_some()
        && !rest.is_empty()
        && rest.chars().all(|c| c.is_ascii_digit())
    {
        return true;
    }
    up.starts_with('R')
        && up[1..].chars().any(|c| c == 'C')
        && up.chars().skip(1).all(|c| c.is_ascii_digit() || c == 'C')
}

fn write_sheet(out: &mut String, s: &SheetRef) {
    let quote = needs_quotes(&s.first) || s.last.as_deref().is_some_and(needs_quotes);
    let mut text = s.first.clone();
    if let Some(l) = &s.last {
        text.push(':');
        text.push_str(l);
    }
    if quote {
        out.push('\'');
        out.push_str(&text.replace('\'', "''"));
        out.push('\'');
    } else {
        out.push_str(&text);
    }
    out.push('!');
}

fn r1c1_part(out: &mut String, letter: char, v: u32, abs: bool, origin: u32) {
    out.push(letter);
    if abs {
        out.push_str(&(v + 1).to_string());
    } else {
        let off = i64::from(v) - i64::from(origin);
        if off != 0 {
            out.push_str(&format!("[{off}]"));
        }
    }
}

fn write_cell(out: &mut String, a: &CellAddr, opts: &ParseOptions) {
    match opts.style {
        RefStyle::A1 => out.push_str(&a.to_a1()),
        RefStyle::R1C1 => {
            r1c1_part(out, 'R', a.row, a.row_abs, opts.origin_row);
            r1c1_part(out, 'C', a.col, a.col_abs, opts.origin_col);
        }
    }
}

/// Format a reference (with sheet qualifier) in the given style.
pub fn format_reference(r: &Reference, opts: &ParseOptions) -> String {
    let mut out = String::new();
    if let Some(s) = &r.sheet {
        write_sheet(&mut out, s);
    }
    match r.kind {
        RefKind::Cell(a) => write_cell(&mut out, &a, opts),
        RefKind::Range(a, b) => {
            write_cell(&mut out, &a, opts);
            out.push(':');
            write_cell(&mut out, &b, opts);
        }
        RefKind::Cols {
            first,
            last,
            first_abs,
            last_abs,
        } => match opts.style {
            RefStyle::A1 => {
                for (i, (c, abs)) in [(first, first_abs), (last, last_abs)]
                    .into_iter()
                    .enumerate()
                {
                    if i > 0 {
                        out.push(':');
                    }
                    if abs {
                        out.push('$');
                    }
                    out.push_str(&col_to_letters(c));
                }
            }
            RefStyle::R1C1 => {
                r1c1_part(&mut out, 'C', first, first_abs, opts.origin_col);
                if (first, first_abs) != (last, last_abs) {
                    out.push(':');
                    r1c1_part(&mut out, 'C', last, last_abs, opts.origin_col);
                }
            }
        },
        RefKind::Rows {
            first,
            last,
            first_abs,
            last_abs,
        } => match opts.style {
            RefStyle::A1 => {
                for (i, (r, abs)) in [(first, first_abs), (last, last_abs)]
                    .into_iter()
                    .enumerate()
                {
                    if i > 0 {
                        out.push(':');
                    }
                    if abs {
                        out.push('$');
                    }
                    out.push_str(&(r + 1).to_string());
                }
            }
            RefStyle::R1C1 => {
                r1c1_part(&mut out, 'R', first, first_abs, opts.origin_row);
                if (first, first_abs) != (last, last_abs) {
                    out.push(':');
                    r1c1_part(&mut out, 'R', last, last_abs, opts.origin_row);
                }
            }
        },
    }
    out
}

fn escape_column(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        if matches!(c, '[' | ']' | '#' | '\'') {
            out.push('\'');
        }
        out.push(c);
    }
    out
}

fn item_text(i: &StructItem) -> String {
    match i {
        StructItem::All => "[#All]".into(),
        StructItem::Data => "[#Data]".into(),
        StructItem::Headers => "[#Headers]".into(),
        StructItem::Totals => "[#Totals]".into(),
        StructItem::ThisRow => "[#This Row]".into(),
        StructItem::Column(c) => format!("[{}]", escape_column(c)),
        StructItem::ColumnRange(a, b) => {
            format!("[{}]:[{}]", escape_column(a), escape_column(b))
        }
    }
}

fn write_struct(out: &mut String, s: &StructRef) {
    if let Some(t) = &s.table {
        out.push_str(t);
    }
    match s.items.as_slice() {
        [] => out.push_str("[]"),
        [StructItem::Column(c)] => {
            out.push('[');
            out.push_str(&escape_column(c));
            out.push(']');
        }
        items => {
            out.push('[');
            let parts: Vec<String> = items.iter().map(item_text).collect();
            out.push_str(&parts.join(","));
            out.push(']');
        }
    }
}
