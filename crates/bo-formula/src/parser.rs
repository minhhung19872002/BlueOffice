//! Formula parser: tokens → [`Expr`] (ECMA-376 Part 1, §18.17.2 formula grammar).
//!
//! A Pratt parser with the operator precedence of §18.17.2.2: reference operators
//! (`:`, space, `,`) bind tightest, then prefix negation, `%`, `^`, `*` `/`, `+` `-`,
//! `&` and finally comparisons. All binary operators are left-associative.

use std::fmt;

use crate::ast::{
    BinaryOp, Expr, UnaryOp, PERCENT_PRECEDENCE, PREFIX_PRECEDENCE, SPILL_PRECEDENCE,
};
use crate::lexer::{Lexer, Tok, Token};
use crate::reference::{RefKind, Reference};

/// Reference notation of formula text.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RefStyle {
    /// `A1`, `$B$2`.
    #[default]
    A1,
    /// `R1C1`, `R[-1]C[2]` (relative to the origin cell).
    R1C1,
}

/// Options controlling parsing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ParseOptions {
    /// Reference style of the input.
    pub style: RefStyle,
    /// Row of the cell holding the formula (needed for R1C1 relative references).
    pub origin_row: u32,
    /// Column of the cell holding the formula.
    pub origin_col: u32,
}

/// A syntax error with byte position.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseError {
    /// Byte offset in the input.
    pub pos: usize,
    /// Human-readable description.
    pub message: String,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "formula syntax error at {}: {}", self.pos, self.message)
    }
}

impl std::error::Error for ParseError {}

/// Parse an A1-style formula (a leading `=` is optional).
pub fn parse(formula: &str) -> Result<Expr, ParseError> {
    parse_with(formula, &ParseOptions::default())
}

/// Parse a formula with explicit options.
pub fn parse_with(formula: &str, opts: &ParseOptions) -> Result<Expr, ParseError> {
    let trimmed = formula.trim_start();
    let body = trimmed.strip_prefix('=').unwrap_or(trimmed);
    let offset = formula.len() - body.len();
    let tokens = Lexer::new(body, opts).tokenize().map_err(|mut e| {
        e.pos += offset;
        e
    })?;
    let mut p = Parser {
        tokens,
        pos: 0,
        end: body.len(),
    };
    let e = p.expr(0, true).map_err(|mut e| {
        e.pos += offset;
        e
    })?;
    if p.pos < p.tokens.len() {
        return Err(ParseError {
            pos: p.tokens[p.pos].pos + offset,
            message: "unexpected token".into(),
        });
    }
    Ok(e)
}

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
    end: usize,
}

fn starts_operand(t: &Tok) -> bool {
    matches!(
        t,
        Tok::Number(_)
            | Tok::Text(_)
            | Tok::Bool(_)
            | Tok::Error(_)
            | Tok::Ref(_)
            | Tok::Name(..)
            | Tok::Func(_)
            | Tok::Struct(_)
            | Tok::LParen
            | Tok::LBrace
    )
}

impl Parser {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn err<T>(&self, msg: &str) -> Result<T, ParseError> {
        Err(ParseError {
            pos: self.peek().map_or(self.end, |t| t.pos),
            message: msg.to_owned(),
        })
    }

    fn expect(&mut self, want: &Tok, msg: &str) -> Result<(), ParseError> {
        if self.peek().map(|t| &t.tok) == Some(want) {
            self.pos += 1;
            Ok(())
        } else {
            self.err(msg)
        }
    }

    /// Parse an expression whose operators all bind tighter than `min`.
    fn expr(&mut self, min: u8, allow_union: bool) -> Result<Expr, ParseError> {
        let mut lhs = self.prefix(allow_union)?;
        while let Some(tok) = self.peek() {
            let op = match &tok.tok {
                Tok::Colon => Some(BinaryOp::Range),
                Tok::Comma if allow_union => Some(BinaryOp::Union),
                Tok::Op(op) => Some(*op),
                Tok::Hash => {
                    if SPILL_PRECEDENCE < min {
                        break;
                    }
                    self.pos += 1;
                    lhs = Expr::Unary(UnaryOp::SpillRange, Box::new(lhs));
                    continue;
                }
                Tok::Percent => {
                    if PERCENT_PRECEDENCE < min {
                        break;
                    }
                    self.pos += 1;
                    lhs = Expr::Unary(UnaryOp::Percent, Box::new(lhs));
                    continue;
                }
                Tok::LParen
                    if !tok.space_before
                        && matches!(lhs, Expr::Call { .. } | Expr::Invoke { .. }) =>
                {
                    // Invocation of a function value: LAMBDA(x,x+1)(2).
                    self.pos += 1;
                    let args = self.args()?;
                    lhs = Expr::Invoke {
                        callee: Box::new(lhs),
                        args,
                    };
                    continue;
                }
                t if tok.space_before && starts_operand(t) => Some(BinaryOp::Intersect),
                _ => None,
            };
            let Some(op) = op else { break };
            let prec = op.precedence();
            if prec < min {
                break;
            }
            if op != BinaryOp::Intersect {
                self.pos += 1;
            }
            let rhs = self.expr(prec + 1, allow_union)?;
            lhs = combine(op, lhs, rhs);
        }
        Ok(lhs)
    }

    fn prefix(&mut self, allow_union: bool) -> Result<Expr, ParseError> {
        let Some(tok) = self.peek().cloned() else {
            return self.err("unexpected end of formula");
        };
        self.pos += 1;
        Ok(match tok.tok {
            Tok::Number(n) => Expr::Number(n),
            Tok::Text(s) => Expr::Text(s),
            Tok::Bool(b) => Expr::Bool(b),
            Tok::Error(e) => Expr::Error(e),
            Tok::Ref(r) => Expr::Ref(r),
            Tok::Name(sheet, name) => Expr::Name { sheet, name },
            Tok::Struct(s) => Expr::Structured(s),
            Tok::Func(name) => {
                self.expect(&Tok::LParen, "expected '('")?;
                let args = self.args()?;
                Expr::Call { name, args }
            }
            Tok::LParen => {
                let e = self.expr(0, true)?;
                self.expect(&Tok::RParen, "expected ')'")?;
                e
            }
            Tok::LBrace => self.array()?,
            Tok::Op(BinaryOp::Sub) => {
                let e = self.expr(PREFIX_PRECEDENCE, allow_union)?;
                Expr::Unary(UnaryOp::Neg, Box::new(e))
            }
            Tok::Op(BinaryOp::Add) => {
                let e = self.expr(PREFIX_PRECEDENCE, allow_union)?;
                Expr::Unary(UnaryOp::Plus, Box::new(e))
            }
            Tok::At => {
                let e = self.expr(PERCENT_PRECEDENCE + 1, allow_union)?;
                Expr::Unary(UnaryOp::ImplicitIntersection, Box::new(e))
            }
            _ => {
                self.pos -= 1;
                return self.err("unexpected token");
            }
        })
    }

    /// Arguments after `(` up to and including `)`.
    fn args(&mut self) -> Result<Vec<Expr>, ParseError> {
        let mut args = Vec::new();
        if self.peek().map(|t| &t.tok) == Some(&Tok::RParen) {
            self.pos += 1;
            return Ok(args);
        }
        loop {
            match self.peek().map(|t| &t.tok) {
                Some(Tok::Comma | Tok::RParen) => args.push(Expr::Missing),
                Some(_) => args.push(self.expr(0, false)?),
                None => return self.err("expected ')'"),
            }
            match self.peek().map(|t| &t.tok) {
                Some(Tok::Comma) => self.pos += 1,
                Some(Tok::RParen) => {
                    self.pos += 1;
                    return Ok(args);
                }
                _ => return self.err("expected ',' or ')'"),
            }
        }
    }

    /// Array constant after `{` (ECMA-376 Part 1, §18.17.2 array-constant).
    fn array(&mut self) -> Result<Expr, ParseError> {
        let mut rows: Vec<Vec<Expr>> = vec![Vec::new()];
        loop {
            let el = self.array_element()?;
            if let Some(last) = rows.last_mut() {
                last.push(el);
            }
            match self.peek().map(|t| &t.tok) {
                Some(Tok::Comma) => self.pos += 1,
                Some(Tok::Semi) => {
                    self.pos += 1;
                    rows.push(Vec::new());
                }
                Some(Tok::RBrace) => {
                    self.pos += 1;
                    break;
                }
                _ => return self.err("expected ',', ';' or '}'"),
            }
        }
        let width = rows[0].len();
        if rows.iter().any(|r| r.len() != width) {
            return self.err("array constant rows must have equal length");
        }
        Ok(Expr::Array(rows))
    }

    fn array_element(&mut self) -> Result<Expr, ParseError> {
        let Some(tok) = self.peek().cloned() else {
            return self.err("unterminated array constant");
        };
        self.pos += 1;
        match tok.tok {
            Tok::Number(n) => Ok(Expr::Number(n)),
            Tok::Text(s) => Ok(Expr::Text(s)),
            Tok::Bool(b) => Ok(Expr::Bool(b)),
            Tok::Error(e) => Ok(Expr::Error(e)),
            Tok::Op(op @ (BinaryOp::Sub | BinaryOp::Add)) => match self.peek().map(|t| &t.tok) {
                Some(Tok::Number(n)) => {
                    let n = if op == BinaryOp::Sub { -*n } else { *n };
                    self.pos += 1;
                    Ok(Expr::Number(n))
                }
                _ => self.err("expected number in array constant"),
            },
            _ => {
                self.pos -= 1;
                self.err("invalid array constant element")
            }
        }
    }
}

/// Build a binary node, folding `A1:B2`-style ranges written with separate
/// operands (e.g. `A1:Sheet1!B2` is not folded) into a single reference.
fn combine(op: BinaryOp, lhs: Expr, rhs: Expr) -> Expr {
    if op == BinaryOp::Range {
        if let (Expr::Ref(a), Expr::Ref(b)) = (&lhs, &rhs) {
            if b.sheet.is_none() {
                if let (RefKind::Cell(x), RefKind::Cell(y)) = (a.kind, b.kind) {
                    return Expr::Ref(Reference {
                        sheet: a.sheet.clone(),
                        kind: RefKind::Range(x, y),
                    });
                }
            }
        }
    }
    Expr::Binary(op, Box::new(lhs), Box::new(rhs))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reference::CellAddr;

    #[test]
    fn precedence() {
        // -2^2 parses as (-2)^2 (negation binds tighter than ^).
        let e = parse("=-2^2").unwrap();
        assert!(matches!(e, Expr::Binary(BinaryOp::Pow, ..)));
        let e = parse("1+2*3").unwrap();
        assert!(matches!(e, Expr::Binary(BinaryOp::Add, ..)));
    }

    #[test]
    fn refs() {
        let e = parse("Sheet1!$A$1:B2").unwrap();
        let Expr::Ref(r) = e else { panic!() };
        assert_eq!(r.sheet.unwrap().first, "Sheet1");
        assert_eq!(
            r.kind,
            RefKind::Range(CellAddr::absolute(0, 0), CellAddr::new(1, 1))
        );
        assert!(parse("LOG10(100)").is_ok());
        assert!(matches!(
            parse("A1 B1").unwrap(),
            Expr::Binary(BinaryOp::Intersect, ..)
        ));
        assert!(matches!(
            parse("(A1,B1)").unwrap(),
            Expr::Binary(BinaryOp::Union, ..)
        ));
    }

    #[test]
    fn errors() {
        assert!(parse("1+").is_err());
        assert!(parse("SUM(1").is_err());
        assert!(parse("{1,2;3}").is_err());
        assert!(parse("\"abc").is_err());
    }
}
