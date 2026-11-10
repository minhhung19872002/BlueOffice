//! The BlueOffice model expression language: lexer and parser
//! (`docs/spec-notes/sheet-datamodel.md`, "Biểu thức").
//!
//! ```text
//! expr    := or
//! or      := and ("||" and)*
//! and     := cmp ("&&" cmp)*
//! cmp     := concat (("=" | "<>" | "<" | "<=" | ">" | ">=") concat)*
//! concat  := add ("&" add)*
//! add     := mul (("+" | "-") mul)*
//! mul     := unary (("*" | "/") unary)*
//! unary   := ("-" | "+") unary | primary
//! primary := number | string | TRUE | FALSE | "(" expr ")"
//!          | name "(" [expr ("," expr)*] ")"       function call
//!          | table "[" column "]"                  column reference
//!          | "[" name "]"                          measure (or a column of the row's table)
//!          | table                                 table reference
//! table   := identifier | "'" quoted name "'"
//! ```

use std::fmt;

/// A binary operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    /// `+`
    Add,
    /// `-`
    Sub,
    /// `*`
    Mul,
    /// `/`
    Div,
    /// `&` (text concatenation)
    Concat,
    /// `=`
    Eq,
    /// `<>`
    Ne,
    /// `<`
    Lt,
    /// `<=`
    Le,
    /// `>`
    Gt,
    /// `>=`
    Ge,
    /// `&&`
    And,
    /// `||`
    Or,
}

/// A parsed expression.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    /// A number literal.
    Num(f64),
    /// A text literal.
    Str(String),
    /// `TRUE` / `FALSE`.
    Bool(bool),
    /// `Table[Column]`.
    Col {
        /// Table name.
        table: String,
        /// Column name.
        column: String,
    },
    /// `[Name]`: a measure, or a column of the table of the current row.
    Bracket(String),
    /// A bare table name (`COUNTROWS(Sales)`).
    Table(String),
    /// Unary minus.
    Neg(Box<Expr>),
    /// A binary operation.
    Bin(Op, Box<Expr>, Box<Expr>),
    /// A function call; the name is upper-cased.
    Call(String, Vec<Expr>),
}

/// A syntax error with its character range in the source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    /// What is wrong (English, shown with the localized prefix by the host).
    pub message: String,
    /// First character (0-based, in `char`s).
    pub start: usize,
    /// One past the last character.
    pub end: usize,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} (at {})", self.message, self.start + 1)
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Num(f64),
    Str(String),
    Ident(String),
    Quoted(String),
    Bracket(String),
    LParen,
    RParen,
    Comma,
    Op(Op),
}

struct Lexed {
    tok: Tok,
    start: usize,
    end: usize,
}

fn err(message: impl Into<String>, start: usize, end: usize) -> ParseError {
    ParseError {
        message: message.into(),
        start,
        end,
    }
}

fn lex(src: &str) -> Result<Vec<Lexed>, ParseError> {
    let chars: Vec<char> = src.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let start = i;
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        let tok = match c {
            '(' => {
                i += 1;
                Tok::LParen
            }
            ')' => {
                i += 1;
                Tok::RParen
            }
            ',' | ';' => {
                i += 1;
                Tok::Comma
            }
            '+' => {
                i += 1;
                Tok::Op(Op::Add)
            }
            '-' => {
                i += 1;
                Tok::Op(Op::Sub)
            }
            '*' => {
                i += 1;
                Tok::Op(Op::Mul)
            }
            '/' => {
                i += 1;
                Tok::Op(Op::Div)
            }
            '=' => {
                i += if chars.get(i + 1) == Some(&'=') { 2 } else { 1 };
                Tok::Op(Op::Eq)
            }
            '&' => {
                if chars.get(i + 1) == Some(&'&') {
                    i += 2;
                    Tok::Op(Op::And)
                } else {
                    i += 1;
                    Tok::Op(Op::Concat)
                }
            }
            '|' => {
                if chars.get(i + 1) == Some(&'|') {
                    i += 2;
                    Tok::Op(Op::Or)
                } else {
                    return Err(err("expected ||", i, i + 1));
                }
            }
            '<' => match chars.get(i + 1) {
                Some('=') => {
                    i += 2;
                    Tok::Op(Op::Le)
                }
                Some('>') => {
                    i += 2;
                    Tok::Op(Op::Ne)
                }
                _ => {
                    i += 1;
                    Tok::Op(Op::Lt)
                }
            },
            '>' => {
                if chars.get(i + 1) == Some(&'=') {
                    i += 2;
                    Tok::Op(Op::Ge)
                } else {
                    i += 1;
                    Tok::Op(Op::Gt)
                }
            }
            '"' => {
                let mut s = String::new();
                i += 1;
                loop {
                    match chars.get(i) {
                        None => return Err(err("unterminated text", start, chars.len())),
                        Some('"') if chars.get(i + 1) == Some(&'"') => {
                            s.push('"');
                            i += 2;
                        }
                        Some('"') => {
                            i += 1;
                            break;
                        }
                        Some(ch) => {
                            s.push(*ch);
                            i += 1;
                        }
                    }
                }
                Tok::Str(s)
            }
            '\'' => {
                let mut s = String::new();
                i += 1;
                loop {
                    match chars.get(i) {
                        None => return Err(err("unterminated table name", start, chars.len())),
                        Some('\'') if chars.get(i + 1) == Some(&'\'') => {
                            s.push('\'');
                            i += 2;
                        }
                        Some('\'') => {
                            i += 1;
                            break;
                        }
                        Some(ch) => {
                            s.push(*ch);
                            i += 1;
                        }
                    }
                }
                Tok::Quoted(s)
            }
            '[' => {
                let mut s = String::new();
                i += 1;
                loop {
                    match chars.get(i) {
                        None => return Err(err("missing ]", start, chars.len())),
                        Some(']') if chars.get(i + 1) == Some(&']') => {
                            s.push(']');
                            i += 2;
                        }
                        Some(']') => {
                            i += 1;
                            break;
                        }
                        Some(ch) => {
                            s.push(*ch);
                            i += 1;
                        }
                    }
                }
                if s.trim().is_empty() {
                    return Err(err("empty name in [ ]", start, i));
                }
                Tok::Bracket(s.trim().to_owned())
            }
            c if c.is_ascii_digit()
                || (c == '.' && chars.get(i + 1).is_some_and(char::is_ascii_digit)) =>
            {
                while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                    i += 1;
                }
                if i < chars.len() && (chars[i] == 'e' || chars[i] == 'E') {
                    let save = i;
                    i += 1;
                    if i < chars.len() && (chars[i] == '+' || chars[i] == '-') {
                        i += 1;
                    }
                    if i < chars.len() && chars[i].is_ascii_digit() {
                        while i < chars.len() && chars[i].is_ascii_digit() {
                            i += 1;
                        }
                    } else {
                        i = save;
                    }
                }
                let text: String = chars[start..i].iter().collect();
                let n = text
                    .parse::<f64>()
                    .map_err(|_| err(format!("bad number {text}"), start, i))?;
                Tok::Num(n)
            }
            c if c.is_alphabetic() || c == '_' => {
                while i < chars.len()
                    && (chars[i].is_alphanumeric() || chars[i] == '_' || chars[i] == '.')
                {
                    i += 1;
                }
                Tok::Ident(chars[start..i].iter().collect())
            }
            other => return Err(err(format!("unexpected character {other}"), i, i + 1)),
        };
        out.push(Lexed { tok, start, end: i });
    }
    Ok(out)
}

struct Parser {
    toks: Vec<Lexed>,
    pos: usize,
    len: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos).map(|t| &t.tok)
    }

    fn span(&self) -> (usize, usize) {
        self.toks
            .get(self.pos)
            .map_or((self.len, self.len), |t| (t.start, t.end))
    }

    fn eat_op(&mut self, ops: &[Op]) -> Option<Op> {
        if let Some(Tok::Op(o)) = self.peek() {
            if ops.contains(o) {
                let o = *o;
                self.pos += 1;
                return Some(o);
            }
        }
        None
    }

    fn binary(
        &mut self,
        ops: &[Op],
        next: fn(&mut Parser) -> Result<Expr, ParseError>,
    ) -> Result<Expr, ParseError> {
        let mut left = next(self)?;
        while let Some(op) = self.eat_op(ops) {
            let right = next(self)?;
            left = Expr::Bin(op, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn or(&mut self) -> Result<Expr, ParseError> {
        self.binary(&[Op::Or], Parser::and)
    }

    fn and(&mut self) -> Result<Expr, ParseError> {
        self.binary(&[Op::And], Parser::cmp)
    }

    fn cmp(&mut self) -> Result<Expr, ParseError> {
        self.binary(
            &[Op::Eq, Op::Ne, Op::Lt, Op::Le, Op::Gt, Op::Ge],
            Parser::concat,
        )
    }

    fn concat(&mut self) -> Result<Expr, ParseError> {
        self.binary(&[Op::Concat], Parser::add)
    }

    fn add(&mut self) -> Result<Expr, ParseError> {
        self.binary(&[Op::Add, Op::Sub], Parser::mul)
    }

    fn mul(&mut self) -> Result<Expr, ParseError> {
        self.binary(&[Op::Mul, Op::Div], Parser::unary)
    }

    fn unary(&mut self) -> Result<Expr, ParseError> {
        match self.eat_op(&[Op::Sub, Op::Add]) {
            Some(Op::Sub) => Ok(Expr::Neg(Box::new(self.unary()?))),
            Some(_) => self.unary(),
            None => self.primary(),
        }
    }

    fn table_suffix(&mut self, table: String) -> Expr {
        if let Some(Tok::Bracket(c)) = self.peek() {
            let column = c.clone();
            self.pos += 1;
            return Expr::Col { table, column };
        }
        Expr::Table(table)
    }

    fn primary(&mut self) -> Result<Expr, ParseError> {
        let (start, end) = self.span();
        let Some(t) = self.toks.get(self.pos).map(|t| t.tok.clone()) else {
            return Err(err("unexpected end of the expression", start, end));
        };
        self.pos += 1;
        match t {
            Tok::Num(n) => Ok(Expr::Num(n)),
            Tok::Str(s) => Ok(Expr::Str(s)),
            Tok::Bracket(s) => Ok(Expr::Bracket(s)),
            Tok::Quoted(s) => Ok(self.table_suffix(s)),
            Tok::LParen => {
                let e = self.or()?;
                if self.peek() != Some(&Tok::RParen) {
                    let (s, e2) = self.span();
                    return Err(err("missing )", s, e2));
                }
                self.pos += 1;
                Ok(e)
            }
            Tok::Ident(name) => {
                if self.peek() == Some(&Tok::LParen) {
                    self.pos += 1;
                    let mut args = Vec::new();
                    if self.peek() == Some(&Tok::RParen) {
                        self.pos += 1;
                    } else {
                        loop {
                            args.push(self.or()?);
                            match self.peek() {
                                Some(Tok::Comma) => self.pos += 1,
                                Some(Tok::RParen) => {
                                    self.pos += 1;
                                    break;
                                }
                                _ => {
                                    let (s, e2) = self.span();
                                    return Err(err("expected , or )", s, e2));
                                }
                            }
                        }
                    }
                    return Ok(Expr::Call(name.to_uppercase(), args));
                }
                match name.to_uppercase().as_str() {
                    "TRUE" if !matches!(self.peek(), Some(Tok::Bracket(_))) => {
                        return Ok(Expr::Bool(true));
                    }
                    "FALSE" if !matches!(self.peek(), Some(Tok::Bracket(_))) => {
                        return Ok(Expr::Bool(false));
                    }
                    _ => {}
                }
                Ok(self.table_suffix(name))
            }
            Tok::RParen | Tok::Comma | Tok::Op(_) => Err(err("expected a value", start, end)),
        }
    }
}

/// Parse an expression (a leading `=` is allowed).
///
/// # Errors
/// The first syntax error with its position.
pub fn parse(src: &str) -> Result<Expr, ParseError> {
    let trimmed = src.trim_start();
    let offset = src.chars().count() - trimmed.chars().count();
    let body = trimmed.strip_prefix('=').unwrap_or(trimmed);
    let shift = offset + usize::from(body.len() != trimmed.len());
    let shifted = |mut e: ParseError| {
        e.start += shift;
        e.end += shift;
        e
    };
    let toks = lex(body).map_err(shifted)?;
    let len = body.chars().count();
    if toks.is_empty() {
        return Err(shifted(err("empty expression", 0, len)));
    }
    let mut p = Parser { toks, pos: 0, len };
    let e = p.or().map_err(shifted)?;
    if p.pos < p.toks.len() {
        let (s, e2) = p.span();
        return Err(shifted(err("unexpected text after the expression", s, e2)));
    }
    Ok(e)
}

impl Expr {
    /// Every `Table[Column]` the expression names (with duplicates).
    pub fn columns(&self) -> Vec<(&str, &str)> {
        let mut out = Vec::new();
        self.walk(&mut |e| {
            if let Expr::Col { table, column } = e {
                out.push((table.as_str(), column.as_str()));
            }
        });
        out
    }

    /// Every table the expression names (column tables and bare tables).
    pub fn tables(&self) -> Vec<&str> {
        let mut out: Vec<&str> = Vec::new();
        self.walk(&mut |e| match e {
            Expr::Col { table, .. } | Expr::Table(table) => out.push(table),
            _ => {}
        });
        out
    }

    /// Every `[Name]` of the expression.
    pub fn brackets(&self) -> Vec<&str> {
        let mut out = Vec::new();
        self.walk(&mut |e| {
            if let Expr::Bracket(n) = e {
                out.push(n.as_str());
            }
        });
        out
    }

    fn walk<'a>(&'a self, f: &mut dyn FnMut(&'a Expr)) {
        f(self);
        match self {
            Expr::Neg(a) => a.walk(f),
            Expr::Bin(_, a, b) => {
                a.walk(f);
                b.walk(f);
            }
            Expr::Call(_, args) => {
                for a in args {
                    a.walk(f);
                }
            }
            _ => {}
        }
    }
}

/// Functions of the language with their argument counts (min, max).
pub const FUNCTIONS: &[(&str, usize, usize)] = &[
    ("SUM", 1, 1),
    ("AVERAGE", 1, 1),
    ("MIN", 1, 1),
    ("MAX", 1, 1),
    ("COUNT", 1, 1),
    ("COUNTA", 1, 1),
    ("COUNTROWS", 1, 1),
    ("DISTINCTCOUNT", 1, 1),
    ("SUMX", 2, 2),
    ("AVERAGEX", 2, 2),
    ("MINX", 2, 2),
    ("MAXX", 2, 2),
    ("DIVIDE", 2, 3),
    ("CALCULATE", 1, 64),
    ("FILTER", 2, 2),
    ("ALL", 1, 1),
    ("USERELATIONSHIP", 2, 2),
    ("RELATED", 1, 1),
    ("IF", 2, 3),
    ("AND", 2, 2),
    ("OR", 2, 2),
    ("NOT", 1, 1),
    ("ISBLANK", 1, 1),
    ("BLANK", 0, 0),
    ("ABS", 1, 1),
    ("ROUND", 2, 2),
];
