//! Formula abstract syntax tree (ECMA-376 Part 1, §18.17.2 formula grammar).

use crate::reference::{Reference, SheetRef};
use crate::value::ErrorKind;

/// Prefix and postfix operators.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum UnaryOp {
    /// Prefix `-`.
    Neg,
    /// Prefix `+` (no-op).
    Plus,
    /// Postfix `%` (divide by 100).
    Percent,
    /// Prefix `@` (implicit intersection).
    ImplicitIntersection,
}

/// Binary operators, including the reference operators.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BinaryOp {
    /// `+`
    Add,
    /// `-`
    Sub,
    /// `*`
    Mul,
    /// `/`
    Div,
    /// `^`
    Pow,
    /// `&`
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
    /// `:` range operator.
    Range,
    /// `,` union operator (only inside parentheses).
    Union,
    /// ` ` (space) intersection operator.
    Intersect,
}

impl BinaryOp {
    /// Operator spelling.
    pub fn symbol(self) -> &'static str {
        match self {
            BinaryOp::Add => "+",
            BinaryOp::Sub => "-",
            BinaryOp::Mul => "*",
            BinaryOp::Div => "/",
            BinaryOp::Pow => "^",
            BinaryOp::Concat => "&",
            BinaryOp::Eq => "=",
            BinaryOp::Ne => "<>",
            BinaryOp::Lt => "<",
            BinaryOp::Le => "<=",
            BinaryOp::Gt => ">",
            BinaryOp::Ge => ">=",
            BinaryOp::Range => ":",
            BinaryOp::Union => ",",
            BinaryOp::Intersect => " ",
        }
    }

    /// Binding power; higher binds tighter (ECMA-376 Part 1, §18.17.2.2 operator precedence:
    /// `:`, space, `,`, negation, `%`, `^`, `*` `/`, `+` `-`, `&`, comparisons).
    pub fn precedence(self) -> u8 {
        match self {
            BinaryOp::Eq
            | BinaryOp::Ne
            | BinaryOp::Lt
            | BinaryOp::Le
            | BinaryOp::Gt
            | BinaryOp::Ge => 1,
            BinaryOp::Concat => 2,
            BinaryOp::Add | BinaryOp::Sub => 3,
            BinaryOp::Mul | BinaryOp::Div => 4,
            BinaryOp::Pow => 5,
            BinaryOp::Union => 8,
            BinaryOp::Intersect => 9,
            BinaryOp::Range => 10,
        }
    }
}

/// Precedence of prefix negation / plus.
pub(crate) const PREFIX_PRECEDENCE: u8 = 6;
/// Precedence of postfix percent.
pub(crate) const PERCENT_PRECEDENCE: u8 = 7;

/// One item of a structured reference specifier list.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum StructItem {
    /// `[#All]`
    All,
    /// `[#Data]`
    Data,
    /// `[#Headers]`
    Headers,
    /// `[#Totals]`
    Totals,
    /// `[#This Row]` or `@`
    ThisRow,
    /// `[Column]`
    Column(String),
    /// `[Col1]:[Col2]`
    ColumnRange(String, String),
}

/// A structured (table) reference such as `Table1[[#This Row],[Amount]]`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct StructRef {
    /// Table name; `None` for the in-table shorthand `[@Col]`.
    pub table: Option<String>,
    /// Specifiers; empty means `[#Data]` of the whole table.
    pub items: Vec<StructItem>,
}

/// A formula expression.
#[derive(Clone, Debug, PartialEq)]
pub enum Expr {
    /// Numeric constant.
    Number(f64),
    /// String constant.
    Text(String),
    /// Boolean constant.
    Bool(bool),
    /// Error constant.
    Error(ErrorKind),
    /// Array constant `{1,2;3,4}` (rows of constants).
    Array(Vec<Vec<Expr>>),
    /// Cell/range reference.
    Ref(Reference),
    /// Defined name (optionally sheet-qualified) or `LET`/`LAMBDA` variable.
    Name {
        /// Sheet qualifier for sheet-scoped names.
        sheet: Option<SheetRef>,
        /// Name as written.
        name: String,
    },
    /// Structured table reference.
    Structured(StructRef),
    /// Unary operator application.
    Unary(UnaryOp, Box<Expr>),
    /// Binary operator application.
    Binary(BinaryOp, Box<Expr>, Box<Expr>),
    /// Function call; `name` is upper-cased without `_xlfn.` prefixes.
    Call {
        /// Function name.
        name: String,
        /// Arguments (may contain [`Expr::Missing`]).
        args: Vec<Expr>,
    },
    /// Invocation of a computed function value, e.g. `LAMBDA(x,x+1)(2)`.
    Invoke {
        /// Expression producing a `LAMBDA`.
        callee: Box<Expr>,
        /// Arguments.
        args: Vec<Expr>,
    },
    /// Omitted function argument, e.g. the second argument of `F(1,,3)`.
    Missing,
}

impl Expr {
    /// Visit this expression and all sub-expressions (pre-order).
    pub fn walk(&self, f: &mut dyn FnMut(&Expr)) {
        f(self);
        match self {
            Expr::Array(rows) => rows.iter().flatten().for_each(|e| e.walk(f)),
            Expr::Unary(_, e) => e.walk(f),
            Expr::Binary(_, a, b) => {
                a.walk(f);
                b.walk(f);
            }
            Expr::Call { args, .. } => {
                for e in args {
                    e.walk(f);
                }
            }
            Expr::Invoke { callee, args } => {
                callee.walk(f);
                for e in args {
                    e.walk(f);
                }
            }
            _ => {}
        }
    }

    /// Rebuild the expression bottom-up, replacing nodes with `f`'s result.
    #[must_use]
    pub fn map(&self, f: &mut dyn FnMut(Expr) -> Expr) -> Expr {
        let rebuilt = match self {
            Expr::Array(rows) => Expr::Array(
                rows.iter()
                    .map(|r| r.iter().map(|e| e.map(f)).collect())
                    .collect(),
            ),
            Expr::Unary(op, e) => Expr::Unary(*op, Box::new(e.map(f))),
            Expr::Binary(op, a, b) => Expr::Binary(*op, Box::new(a.map(f)), Box::new(b.map(f))),
            Expr::Call { name, args } => Expr::Call {
                name: name.clone(),
                args: args.iter().map(|e| e.map(f)).collect(),
            },
            Expr::Invoke { callee, args } => Expr::Invoke {
                callee: Box::new(callee.map(f)),
                args: args.iter().map(|e| e.map(f)).collect(),
            },
            other => other.clone(),
        };
        f(rebuilt)
    }
}
