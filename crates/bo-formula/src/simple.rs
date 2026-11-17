//! Plain arithmetic formulas compiled to a short postfix program.
//!
//! Most formulas of large sheets are like `=A2*2+$D$1`: numbers, single cells of the
//! formula's own sheet and `+ - * /`, unary minus and percent (ECMA-376 Part 1,
//! §18.17.2.2 operators). Recalculating 100 000 of them through the general evaluator
//! (reference values, dereferencing through the cell provider, coercions) costs far more
//! than the arithmetic, so such formulas also get a [`Program`] run on plain `f64`s.
//!
//! The program gives up — and the general evaluator then decides — whenever its result
//! could differ: a cell holding text, an error, an array or a value not computed yet, a
//! division by zero, or an intermediate result that is not finite (`#NUM!`).

use crate::ast::{BinaryOp, Expr, UnaryOp};
use crate::reference::RefKind;

/// Deepest operand stack a program may need.
const MAX_STACK: usize = 16;
/// Longest program kept (longer formulas use the general evaluator).
const MAX_OPS: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Op {
    Num(f64),
    /// A cell of the formula's sheet: `(row, column)`.
    Cell(u32, u32),
    Add,
    Sub,
    Mul,
    Div,
    Neg,
    Percent,
}

/// A compiled arithmetic formula.
#[derive(Debug)]
pub(crate) struct Program(Box<[Op]>);

impl Program {
    /// The program of `expr`, or `None` when it is not plain arithmetic.
    pub(crate) fn compile(expr: &Expr) -> Option<Program> {
        let mut ops = Vec::new();
        let (mut depth, mut max) = (0usize, 0usize);
        emit(expr, &mut ops, &mut depth, &mut max)?;
        (depth == 1 && max <= MAX_STACK && ops.len() <= MAX_OPS)
            .then(|| Program(ops.into_boxed_slice()))
    }

    /// Runs the program; `read(row, column)` gives a cell's number or `None` to give up.
    pub(crate) fn run(&self, read: impl Fn(u32, u32) -> Option<f64>) -> Option<f64> {
        let mut st = [0f64; MAX_STACK];
        let mut n = 0usize;
        for op in &*self.0 {
            match *op {
                Op::Num(x) => {
                    st[n] = x;
                    n += 1;
                }
                Op::Cell(r, c) => {
                    st[n] = read(r, c)?;
                    n += 1;
                }
                Op::Neg => st[n - 1] = -st[n - 1],
                Op::Percent => st[n - 1] /= 100.0,
                Op::Add | Op::Sub | Op::Mul | Op::Div => {
                    n -= 1;
                    let b = st[n];
                    let a = st[n - 1];
                    let v = match *op {
                        Op::Add => a + b,
                        Op::Sub => a - b,
                        Op::Mul => a * b,
                        _ if b == 0.0 => return None,
                        _ => a / b,
                    };
                    if !v.is_finite() {
                        return None;
                    }
                    st[n - 1] = v;
                }
            }
        }
        // Negative zero shows as 0 (`Value::number`).
        Some(if st[0] == 0.0 { 0.0 } else { st[0] })
    }
}

fn emit(e: &Expr, ops: &mut Vec<Op>, depth: &mut usize, max: &mut usize) -> Option<()> {
    if ops.len() > MAX_OPS {
        return None;
    }
    match e {
        Expr::Number(x) => {
            ops.push(Op::Num(*x));
            *depth += 1;
        }
        Expr::Ref(r) if r.sheet.is_none() => {
            let RefKind::Cell(a) = r.kind else {
                return None;
            };
            ops.push(Op::Cell(a.row, a.col));
            *depth += 1;
        }
        Expr::Unary(UnaryOp::Plus, inner) => emit(inner, ops, depth, max)?,
        Expr::Unary(op @ (UnaryOp::Neg | UnaryOp::Percent), inner) => {
            emit(inner, ops, depth, max)?;
            ops.push(if *op == UnaryOp::Neg {
                Op::Neg
            } else {
                Op::Percent
            });
        }
        Expr::Binary(
            op @ (BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div),
            a,
            b,
        ) => {
            emit(a, ops, depth, max)?;
            emit(b, ops, depth, max)?;
            ops.push(match op {
                BinaryOp::Add => Op::Add,
                BinaryOp::Sub => Op::Sub,
                BinaryOp::Mul => Op::Mul,
                _ => Op::Div,
            });
            *depth -= 1;
        }
        _ => return None,
    }
    *max = (*max).max(*depth);
    Some(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse;

    fn prog(f: &str) -> Option<Program> {
        Program::compile(&parse(f).expect("parses"))
    }

    #[test]
    fn compiles_plain_arithmetic_only() {
        assert!(prog("=A1*2+$D$1").is_some());
        assert!(prog("=-(A1+B2)%/3").is_some());
        assert!(prog("=SUM(A1:A3)").is_none());
        assert!(prog("=A1:A3*2").is_none());
        assert!(prog("=Sheet2!A1+1").is_none());
        assert!(prog("=A1^2").is_none());
        assert!(prog("=\"a\"&A1").is_none());
        assert!(prog("=A1>2").is_none());
    }

    #[test]
    fn runs_and_gives_up_where_the_evaluator_decides() {
        let cells = |r: u32, c: u32| match (r, c) {
            (0, 0) => Some(3.0),
            (0, 3) => Some(1.0),
            (1, 0) => Some(0.0),
            _ => None,
        };
        assert_eq!(prog("=A1*2+$D$1").and_then(|p| p.run(cells)), Some(7.0));
        assert_eq!(prog("=-(A1+D1)%").and_then(|p| p.run(cells)), Some(-0.04));
        assert_eq!(prog("=-A2").and_then(|p| p.run(cells)), Some(0.0));
        // Division by zero, a cell given up on, overflow.
        assert_eq!(prog("=A1/A2").and_then(|p| p.run(cells)), None);
        assert_eq!(prog("=A1+C9").and_then(|p| p.run(cells)), None);
        assert_eq!(prog("=1E308*10-1").and_then(|p| p.run(cells)), None);
    }
}
