//! Evaluation of model expressions in a filter context (and row contexts).
//!
//! * **Filter context** ([`Context`]): filters on columns (a set of allowed or excluded
//!   values) and on rows of a table. A table's visible rows pass its own filters and, through
//!   every active many-to-one relationship, the filters of the tables it reaches (a filter on
//!   a dimension filters the facts, never the other way round). A row without a match on the
//!   one side relates to that table's blank row: it passes value filters that accept blank.
//! * **Row context**: the current row of a table while a calculated column, `FILTER` or an
//!   iterator (`SUMX`, …) evaluates its expression; `Table[Column]` reads it and `RELATED`
//!   follows relationships from it. A measure referenced in a row context sees that row as a
//!   filter (context transition), as does `CALCULATE`.
//! * `CALCULATE(expr, modifiers…)`: `ALL(column)` / `ALL(table)` remove filters,
//!   `USERELATIONSHIP(a, b)` activates a relationship, `FILTER(table, condition)` sets the rows
//!   of a table (evaluated in the outer context) and a condition over one column replaces the
//!   filters of that column (`Table[Column] = "x"`).

use super::expr::{Expr, Op, FUNCTIONS};
use super::{Key, ModelData};
use bo_model::sheet::{CellError, CellValue};
use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

/// A value of the language.
#[derive(Debug, Clone, PartialEq)]
pub enum V {
    /// Blank.
    Blank,
    /// A number.
    Num(f64),
    /// Text.
    Text(String),
    /// A boolean.
    Bool(bool),
    /// An error.
    Err(CellError),
}

impl V {
    /// The value of a cell.
    #[must_use]
    pub fn of(v: &CellValue) -> V {
        match v {
            CellValue::Empty => V::Blank,
            CellValue::Number(n) => V::Num(*n),
            CellValue::Bool(b) => V::Bool(*b),
            CellValue::Error(e) => V::Err(e.clone()),
            CellValue::Text(s) | CellValue::Date(s) => V::Text(s.clone()),
            other @ CellValue::RichText(_) => V::Text(other.as_text().unwrap_or_default()),
        }
    }

    /// As a cell value.
    #[must_use]
    pub fn into_cell(self) -> CellValue {
        match self {
            V::Blank => CellValue::Empty,
            V::Num(n) if n.is_finite() => CellValue::Number(n),
            V::Num(_) => CellValue::Error(CellError::Num),
            V::Text(s) => CellValue::Text(s),
            V::Bool(b) => CellValue::Bool(b),
            V::Err(e) => CellValue::Error(e),
        }
    }

    fn text(&self) -> String {
        match self {
            V::Blank => String::new(),
            V::Num(n) => crate::pivot::value_text(&CellValue::Number(*n)),
            V::Text(s) => s.clone(),
            V::Bool(b) => if *b { "TRUE" } else { "FALSE" }.into(),
            V::Err(e) => e.as_str().to_owned(),
        }
    }

    fn key(&self) -> Key {
        match self {
            V::Blank => Key::Empty,
            V::Num(n) => Key::of(&CellValue::Number(*n)),
            V::Text(s) => Key::Text(s.to_lowercase()),
            V::Bool(b) => Key::Bool(*b),
            V::Err(e) => Key::Err(e.as_str().to_owned()),
        }
    }
}

/// One filter of a context.
#[derive(Debug, Clone)]
pub enum Filter {
    /// Rows whose value in `col` is (or, `exclude`, is not) one of `keys`.
    Values {
        /// Table.
        table: usize,
        /// Column.
        col: usize,
        /// Value keys.
        keys: Rc<HashSet<Key>>,
        /// Keep the rows whose value is not in `keys`.
        exclude: bool,
    },
    /// Rows of `table` flagged in `rows`.
    Rows {
        /// Table.
        table: usize,
        /// One flag per row.
        rows: Rc<Vec<bool>>,
    },
    /// One row of `table` (context transition).
    Row {
        /// Table.
        table: usize,
        /// The row.
        row: usize,
    },
}

impl Filter {
    /// The table filtered.
    #[must_use]
    pub fn table(&self) -> usize {
        match self {
            Filter::Values { table, .. }
            | Filter::Rows { table, .. }
            | Filter::Row { table, .. } => *table,
        }
    }
}

/// A filter context.
#[derive(Debug, Clone, Default)]
pub struct Context {
    /// Filters (all must pass).
    pub filters: Vec<Filter>,
    /// Relationships activated by `USERELATIONSHIP`.
    pub use_rels: Vec<usize>,
}

impl Context {
    /// Keep the rows of `table` whose `col` is one of `values`.
    pub fn keep(&mut self, table: usize, col: usize, values: &[CellValue]) {
        self.filters.push(Filter::Values {
            table,
            col,
            keys: Rc::new(values.iter().map(Key::of).collect()),
            exclude: false,
        });
    }

    /// Drop the rows of `table` whose `col` is one of `values`.
    pub fn drop_values(&mut self, table: usize, col: usize, values: &[CellValue]) {
        self.filters.push(Filter::Values {
            table,
            col,
            keys: Rc::new(values.iter().map(Key::of).collect()),
            exclude: true,
        });
    }
}

/// Maximum nesting of measures (circular references end in `#REF!`... once).
const MAX_DEPTH: usize = 64;

/// Evaluates expressions over a loaded model.
pub struct Evaluator<'a> {
    md: &'a ModelData,
    /// Measures being evaluated (circular references).
    stack: RefCell<Vec<usize>>,
}

type Rows<'r> = &'r [(usize, usize)];

fn num_err() -> V {
    V::Err(CellError::Value)
}

fn truthy(v: &V) -> Result<bool, V> {
    match v {
        V::Blank => Ok(false),
        V::Num(n) => Ok(*n != 0.0),
        V::Bool(b) => Ok(*b),
        V::Text(s) => match s.to_uppercase().as_str() {
            "TRUE" => Ok(true),
            "FALSE" | "" => Ok(false),
            _ => Err(num_err()),
        },
        V::Err(_) => Err(v.clone()),
    }
}

fn to_num(v: &V) -> Result<f64, V> {
    match v {
        V::Blank => Ok(0.0),
        V::Num(n) => Ok(*n),
        V::Bool(b) => Ok(f64::from(u8::from(*b))),
        V::Text(s) => s.trim().parse::<f64>().map_err(|_| num_err()),
        V::Err(_) => Err(v.clone()),
    }
}

macro_rules! tri {
    ($e:expr) => {
        match $e {
            Ok(x) => x,
            Err(v) => return v,
        }
    };
}

impl<'a> Evaluator<'a> {
    /// An evaluator over `md`.
    #[must_use]
    pub fn new(md: &'a ModelData) -> Evaluator<'a> {
        Evaluator {
            md,
            stack: RefCell::new(Vec::new()),
        }
    }

    /// The value of measure `mi` in `ctx`.
    #[must_use]
    pub fn measure(&self, mi: usize, ctx: &Context) -> V {
        let Some(parsed) = self.md.measures.get(mi) else {
            return V::Err(CellError::Name);
        };
        let Ok(e) = parsed else {
            return V::Err(CellError::Name);
        };
        {
            let mut st = self.stack.borrow_mut();
            if st.contains(&mi) || st.len() >= MAX_DEPTH {
                return V::Err(CellError::Ref);
            }
            st.push(mi);
        }
        let v = self.eval(e, ctx, &[]);
        self.stack.borrow_mut().pop();
        v
    }

    /// The value of `e` in `ctx` (no row context).
    #[must_use]
    pub fn value(&self, e: &Expr, ctx: &Context) -> V {
        self.eval(e, ctx, &[])
    }

    /// The value of `e` for row `row` of table `t` (a calculated column).
    #[must_use]
    pub fn row_value(&self, e: &Expr, t: usize, row: usize) -> V {
        self.eval(e, &Context::default(), &[(t, row)])
    }

    // -- visibility --------------------------------------------------------

    /// Rows of `u` passing its own filters, and whether its blank row passes them.
    fn own_mask(&self, u: usize, ctx: &Context) -> (Vec<bool>, bool) {
        let rows = &self.md.tables[u].rows;
        let mut mask = vec![true; rows.len()];
        let mut blank_ok = true;
        for f in ctx.filters.iter().filter(|f| f.table() == u) {
            match f {
                Filter::Values {
                    col, keys, exclude, ..
                } => {
                    blank_ok &= keys.contains(&Key::Empty) != *exclude;
                    for (m, row) in mask.iter_mut().zip(rows) {
                        if *m {
                            let k = row.get(*col).map_or(Key::Empty, Key::of);
                            *m = keys.contains(&k) != *exclude;
                        }
                    }
                }
                Filter::Rows { rows: flags, .. } => {
                    blank_ok = false;
                    for (i, m) in mask.iter_mut().enumerate() {
                        *m = *m && flags.get(i).copied().unwrap_or(false);
                    }
                }
                Filter::Row { row, .. } => {
                    blank_ok = false;
                    for (i, m) in mask.iter_mut().enumerate() {
                        *m = *m && i == *row;
                    }
                }
            }
        }
        (mask, blank_ok)
    }

    /// Visible rows of table `t` in `ctx` (`None`: every row).
    #[must_use]
    pub fn visible(&self, t: usize, ctx: &Context) -> Option<Vec<bool>> {
        let filtered: HashSet<usize> = ctx.filters.iter().map(Filter::table).collect();
        if filtered.is_empty() {
            return None;
        }
        let reach = self.md.reachable(t, &ctx.use_rels);
        if !reach.iter().any(|(u, _)| filtered.contains(u)) {
            return None;
        }
        let n = self.md.tables[t].rows.len();
        let mut vis = vec![true; n];
        for (u, path) in &reach {
            if !filtered.contains(u) {
                continue;
            }
            let (mask, blank_ok) = self.own_mask(*u, ctx);
            if path.is_empty() {
                for (v, m) in vis.iter_mut().zip(&mask) {
                    *v = *v && *m;
                }
                continue;
            }
            for (r, v) in vis.iter_mut().enumerate() {
                if !*v {
                    continue;
                }
                let mut cur = Some(r as u32);
                for ri in path {
                    cur = cur.and_then(|x| {
                        self.md.rels[*ri]
                            .as_ref()
                            .and_then(|rel| rel.map.get(x as usize).copied().flatten())
                    });
                }
                *v = match cur {
                    Some(x) => mask.get(x as usize).copied().unwrap_or(false),
                    None => blank_ok,
                };
            }
        }
        Some(vis)
    }

    fn rows_of(&self, t: usize, ctx: &Context) -> Vec<usize> {
        match self.visible(t, ctx) {
            None => (0..self.md.tables[t].rows.len()).collect(),
            Some(v) => v
                .iter()
                .enumerate()
                .filter_map(|(i, x)| x.then_some(i))
                .collect(),
        }
    }

    // -- evaluation --------------------------------------------------------

    fn col(&self, table: &str, column: &str) -> Result<(usize, usize), V> {
        self.md.column(table, column).ok_or(V::Err(CellError::Name))
    }

    fn cell(&self, t: usize, r: usize, c: usize) -> V {
        self.md.tables[t]
            .rows
            .get(r)
            .and_then(|row| row.get(c))
            .map_or(V::Blank, V::of)
    }

    /// The filter context with the row contexts turned into filters.
    fn transition(ctx: &Context, rows: Rows<'_>) -> Context {
        let mut c = ctx.clone();
        for &(t, r) in rows {
            c.filters.retain(|f| f.table() != t);
            c.filters.push(Filter::Row { table: t, row: r });
        }
        c
    }

    fn eval(&self, e: &Expr, ctx: &Context, rows: Rows<'_>) -> V {
        match e {
            Expr::Num(n) => V::Num(*n),
            Expr::Str(s) => V::Text(s.clone()),
            Expr::Bool(b) => V::Bool(*b),
            Expr::Col { table, column } => {
                let (t, c) = tri!(self.col(table, column));
                match rows.iter().rev().find(|(rt, _)| *rt == t) {
                    Some(&(_, r)) => self.cell(t, r, c),
                    // A column needs a row context (or an aggregation).
                    None => V::Err(CellError::Value),
                }
            }
            Expr::Bracket(name) => {
                if let Some(mi) = self.md.def.measure(name) {
                    let c = if rows.is_empty() {
                        ctx.clone()
                    } else {
                        Self::transition(ctx, rows)
                    };
                    return self.measure(mi, &c);
                }
                if let Some(&(t, r)) = rows.last() {
                    if let Some(c) = self.md.tables[t].column(name) {
                        return self.cell(t, r, c);
                    }
                }
                V::Err(CellError::Name)
            }
            Expr::Table(_) => V::Err(CellError::Value),
            Expr::Neg(a) => match self.eval(a, ctx, rows) {
                V::Blank => V::Blank,
                v => V::Num(-tri!(to_num(&v))),
            },
            Expr::Bin(op, a, b) => {
                let x = self.eval(a, ctx, rows);
                match op {
                    Op::And => {
                        if !tri!(truthy(&x)) {
                            return V::Bool(false);
                        }
                        return V::Bool(tri!(truthy(&self.eval(b, ctx, rows))));
                    }
                    Op::Or => {
                        if tri!(truthy(&x)) {
                            return V::Bool(true);
                        }
                        return V::Bool(tri!(truthy(&self.eval(b, ctx, rows))));
                    }
                    _ => {}
                }
                let y = self.eval(b, ctx, rows);
                binary(*op, &x, &y)
            }
            Expr::Call(name, args) => self.call(name, args, ctx, rows),
        }
    }

    /// A table expression: (table, flags of its rows).
    fn table_expr(&self, e: &Expr, ctx: &Context, rows: Rows<'_>) -> Result<(usize, Vec<bool>), V> {
        match e {
            Expr::Table(name) => {
                let t = self.md.table(name).ok_or(V::Err(CellError::Name))?;
                let n = self.md.tables[t].rows.len();
                Ok((t, self.visible(t, ctx).unwrap_or_else(|| vec![true; n])))
            }
            Expr::Call(f, args) if f == "ALL" && args.len() == 1 => match &args[0] {
                Expr::Table(name) => {
                    let t = self.md.table(name).ok_or(V::Err(CellError::Name))?;
                    Ok((t, vec![true; self.md.tables[t].rows.len()]))
                }
                _ => Err(V::Err(CellError::Value)),
            },
            Expr::Call(f, args) if f == "FILTER" && args.len() == 2 => {
                let (t, mut mask) = self.table_expr(&args[0], ctx, rows)?;
                let mut inner: Vec<(usize, usize)> = rows.to_vec();
                inner.push((t, 0));
                for (r, m) in mask.iter_mut().enumerate() {
                    if !*m {
                        continue;
                    }
                    if let Some(last) = inner.last_mut() {
                        last.1 = r;
                    }
                    *m = truthy(&self.eval(&args[1], ctx, &inner))?;
                }
                Ok((t, mask))
            }
            _ => Err(V::Err(CellError::Value)),
        }
    }

    fn aggregate(&self, f: &str, arg: &Expr, ctx: &Context) -> V {
        let Expr::Col { table, column } = arg else {
            return V::Err(CellError::Value);
        };
        let (t, c) = tri!(self.col(table, column));
        let rows = self.rows_of(t, ctx);
        let data = &self.md.tables[t].rows;
        let values = rows
            .iter()
            .map(|r| data[*r].get(c).unwrap_or(&CellValue::Empty));
        match f {
            "COUNTA" => {
                let n = values.filter(|v| !matches!(v, CellValue::Empty)).count();
                if n == 0 {
                    V::Blank
                } else {
                    V::Num(n as f64)
                }
            }
            "DISTINCTCOUNT" => {
                let n = values.map(Key::of).collect::<HashSet<_>>().len();
                if n == 0 {
                    V::Blank
                } else {
                    V::Num(n as f64)
                }
            }
            _ => {
                let mut nums: Vec<f64> = Vec::new();
                for v in values {
                    match v {
                        CellValue::Number(n) => nums.push(*n),
                        CellValue::Error(e) if f != "COUNT" => return V::Err(e.clone()),
                        _ => {}
                    }
                }
                numeric(f, &nums)
            }
        }
    }

    #[allow(clippy::too_many_lines)]
    fn call(&self, f: &str, args: &[Expr], ctx: &Context, rows: Rows<'_>) -> V {
        let Some(&(_, lo, hi)) = FUNCTIONS.iter().find(|(n, _, _)| *n == f) else {
            return V::Err(CellError::Name);
        };
        if args.len() < lo || args.len() > hi {
            return V::Err(CellError::Value);
        }
        match f {
            "SUM" | "AVERAGE" | "MIN" | "MAX" | "COUNT" | "COUNTA" | "DISTINCTCOUNT" => {
                // Aggregations see the filter context (a row context becomes one first only
                // through CALCULATE or a measure, as in the documented semantics).
                self.aggregate(f, &args[0], ctx)
            }
            "COUNTROWS" => {
                let (_, mask) = tri!(self.table_expr(&args[0], ctx, rows));
                let n = mask.iter().filter(|x| **x).count();
                if n == 0 {
                    V::Blank
                } else {
                    V::Num(n as f64)
                }
            }
            "SUMX" | "AVERAGEX" | "MINX" | "MAXX" => {
                let (t, mask) = tri!(self.table_expr(&args[0], ctx, rows));
                let mut inner: Vec<(usize, usize)> = rows.to_vec();
                inner.push((t, 0));
                let mut nums = Vec::new();
                for (r, m) in mask.iter().enumerate() {
                    if !*m {
                        continue;
                    }
                    if let Some(last) = inner.last_mut() {
                        last.1 = r;
                    }
                    match self.eval(&args[1], ctx, &inner) {
                        V::Blank => {}
                        V::Err(e) => return V::Err(e),
                        v => nums.push(tri!(to_num(&v))),
                    }
                }
                let base = match f {
                    "SUMX" => "SUM",
                    "AVERAGEX" => "AVERAGE",
                    "MINX" => "MIN",
                    _ => "MAX",
                };
                numeric(base, &nums)
            }
            "DIVIDE" => {
                let a = self.eval(&args[0], ctx, rows);
                let b = self.eval(&args[1], ctx, rows);
                let alt = || args.get(2).map_or(V::Blank, |x| self.eval(x, ctx, rows));
                if let V::Err(_) = a {
                    return a;
                }
                if let V::Err(_) = b {
                    return b;
                }
                let d = tri!(to_num(&b));
                if matches!(b, V::Blank) || d == 0.0 {
                    return alt();
                }
                if matches!(a, V::Blank) {
                    return V::Blank;
                }
                V::Num(tri!(to_num(&a)) / d)
            }
            "CALCULATE" => self.calculate(args, ctx, rows),
            "FILTER" | "ALL" | "USERELATIONSHIP" => V::Err(CellError::Value),
            "RELATED" => {
                let Expr::Col { table, column } = &args[0] else {
                    return V::Err(CellError::Value);
                };
                let (u, c) = tri!(self.col(table, column));
                for &(t, r) in rows.iter().rev() {
                    if t == u {
                        return self.cell(t, r, c);
                    }
                    let reach = self.md.reachable(t, &ctx.use_rels);
                    if let Some((_, path)) = reach.iter().find(|(x, _)| *x == u) {
                        let mut cur = Some(r as u32);
                        for ri in path {
                            cur = cur.and_then(|x| {
                                self.md.rels[*ri]
                                    .as_ref()
                                    .and_then(|rel| rel.map.get(x as usize).copied().flatten())
                            });
                        }
                        return cur.map_or(V::Blank, |x| self.cell(u, x as usize, c));
                    }
                }
                V::Err(CellError::Value)
            }
            "IF" => {
                let c = tri!(truthy(&self.eval(&args[0], ctx, rows)));
                if c {
                    self.eval(&args[1], ctx, rows)
                } else {
                    args.get(2).map_or(V::Blank, |x| self.eval(x, ctx, rows))
                }
            }
            "AND" | "OR" => {
                let a = tri!(truthy(&self.eval(&args[0], ctx, rows)));
                if f == "AND" && !a {
                    return V::Bool(false);
                }
                if f == "OR" && a {
                    return V::Bool(true);
                }
                V::Bool(tri!(truthy(&self.eval(&args[1], ctx, rows))))
            }
            "NOT" => V::Bool(!tri!(truthy(&self.eval(&args[0], ctx, rows)))),
            "ISBLANK" => V::Bool(matches!(self.eval(&args[0], ctx, rows), V::Blank)),
            "BLANK" => V::Blank,
            "ABS" => match self.eval(&args[0], ctx, rows) {
                V::Blank => V::Blank,
                v => V::Num(tri!(to_num(&v)).abs()),
            },
            "ROUND" => {
                let v = self.eval(&args[0], ctx, rows);
                if matches!(v, V::Blank) {
                    return V::Blank;
                }
                let x = tri!(to_num(&v));
                let d = tri!(to_num(&self.eval(&args[1], ctx, rows)));
                let p = 10f64.powi(d.trunc() as i32);
                V::Num((x * p).round() / p)
            }
            _ => V::Err(CellError::Name),
        }
    }

    fn calculate(&self, args: &[Expr], ctx: &Context, rows: Rows<'_>) -> V {
        let outer = if rows.is_empty() {
            ctx.clone()
        } else {
            Self::transition(ctx, rows)
        };
        let mut new = outer.clone();
        // Removals and relationship choices first.
        for m in &args[1..] {
            let Expr::Call(f, a) = m else { continue };
            match (f.as_str(), a.as_slice()) {
                ("ALL", [Expr::Col { table, column }]) => {
                    let (t, c) = tri!(self.col(table, column));
                    new.filters
                        .retain(|x| !matches!(x, Filter::Values { table, col, .. } if *table == t && *col == c));
                }
                ("ALL", [Expr::Table(name)]) => {
                    let t = tri!(self.md.table(name).ok_or(V::Err(CellError::Name)));
                    // The table and the tables it reaches (its expanded table).
                    let reach: Vec<usize> = self
                        .md
                        .reachable(t, &new.use_rels)
                        .into_iter()
                        .map(|(u, _)| u)
                        .collect();
                    new.filters.retain(|x| !reach.contains(&x.table()));
                }
                (
                    "USERELATIONSHIP",
                    [Expr::Col {
                        table: t1,
                        column: c1,
                    }, Expr::Col {
                        table: t2,
                        column: c2,
                    }],
                ) => {
                    let a = tri!(self.col(t1, c1));
                    let b = tri!(self.col(t2, c2));
                    let found = self.md.rels.iter().position(|r| {
                        r.as_ref().is_some_and(|r| {
                            (r.from == a && r.to == b) || (r.from == b && r.to == a)
                        })
                    });
                    match found {
                        Some(ri) => new.use_rels.push(ri),
                        None => return V::Err(CellError::Value),
                    }
                }
                ("ALL" | "USERELATIONSHIP", _) => return V::Err(CellError::Value),
                _ => {}
            }
        }
        for m in &args[1..] {
            match m {
                Expr::Call(f, _) if f == "ALL" || f == "USERELATIONSHIP" => {}
                Expr::Call(f, _) if f == "FILTER" => {
                    let (t, mask) = tri!(self.table_expr(m, &outer, &[]));
                    new.filters.retain(|x| x.table() != t);
                    new.filters.push(Filter::Rows {
                        table: t,
                        rows: Rc::new(mask),
                    });
                }
                cond => {
                    let cols = cond.columns();
                    let Some(&(tn, cn)) = cols.first() else {
                        return V::Err(CellError::Value);
                    };
                    let (t, c) = tri!(self.col(tn, cn));
                    let mut resolved = Vec::with_capacity(cols.len());
                    for (a, b) in &cols {
                        resolved.push(tri!(self.col(a, b)));
                    }
                    if resolved.iter().any(|(x, _)| *x != t) {
                        // A condition must use the columns of one table.
                        return V::Err(CellError::Value);
                    }
                    let n = self.md.tables[t].rows.len();
                    let single = resolved.iter().all(|x| *x == (t, c));
                    let empty = Context::default();
                    let mut mask = vec![false; n];
                    for (r, m) in mask.iter_mut().enumerate() {
                        *m = tri!(truthy(&self.eval(cond, &empty, &[(t, r)])));
                    }
                    if single {
                        let keys: HashSet<Key> = mask
                            .iter()
                            .enumerate()
                            .filter(|(_, m)| **m)
                            .map(|(r, _)| self.cell(t, r, c).key())
                            .collect();
                        new.filters.retain(
                            |x| !matches!(x, Filter::Values { table, col, .. } if *table == t && *col == c),
                        );
                        new.filters.push(Filter::Values {
                            table: t,
                            col: c,
                            keys: Rc::new(keys),
                            exclude: false,
                        });
                    } else {
                        new.filters.retain(|x| x.table() != t);
                        new.filters.push(Filter::Rows {
                            table: t,
                            rows: Rc::new(mask),
                        });
                    }
                }
            }
        }
        self.eval(&args[0], &new, &[])
    }
}

fn numeric(f: &str, nums: &[f64]) -> V {
    if nums.is_empty() {
        return V::Blank;
    }
    match f {
        "SUM" => V::Num(nums.iter().sum()),
        "AVERAGE" => V::Num(nums.iter().sum::<f64>() / nums.len() as f64),
        "MIN" => V::Num(nums.iter().copied().fold(f64::INFINITY, f64::min)),
        "MAX" => V::Num(nums.iter().copied().fold(f64::NEG_INFINITY, f64::max)),
        "COUNT" => V::Num(nums.len() as f64),
        _ => V::Err(CellError::Name),
    }
}

fn compare(x: &V, y: &V) -> Result<std::cmp::Ordering, V> {
    use std::cmp::Ordering;
    let ord = |a: f64, b: f64| a.partial_cmp(&b).unwrap_or(Ordering::Equal);
    match (x, y) {
        (V::Err(_), _) => Err(x.clone()),
        (_, V::Err(_)) => Err(y.clone()),
        (V::Text(a), V::Text(b)) => Ok(a.to_lowercase().cmp(&b.to_lowercase())),
        (V::Text(a), V::Blank) => Ok(a.as_str().cmp("")),
        (V::Blank, V::Text(b)) => Ok("".cmp(b.as_str())),
        (V::Text(_), _) | (_, V::Text(_)) => Err(V::Err(CellError::Value)),
        _ => Ok(ord(to_num(x)?, to_num(y)?)),
    }
}

fn binary(op: Op, x: &V, y: &V) -> V {
    use std::cmp::Ordering as O;
    match op {
        Op::Concat => {
            if let V::Err(_) = x {
                return x.clone();
            }
            if let V::Err(_) = y {
                return y.clone();
            }
            V::Text(format!("{}{}", x.text(), y.text()))
        }
        Op::Eq | Op::Ne | Op::Lt | Op::Le | Op::Gt | Op::Ge => {
            let o = tri!(compare(x, y));
            V::Bool(match op {
                Op::Eq => o == O::Equal,
                Op::Ne => o != O::Equal,
                Op::Lt => o == O::Less,
                Op::Le => o != O::Greater,
                Op::Gt => o == O::Greater,
                _ => o != O::Less,
            })
        }
        _ => {
            if let V::Err(_) = x {
                return x.clone();
            }
            if let V::Err(_) = y {
                return y.clone();
            }
            let (bx, by) = (matches!(x, V::Blank), matches!(y, V::Blank));
            if bx && by {
                return V::Blank;
            }
            let a = tri!(to_num(x));
            let b = tri!(to_num(y));
            match op {
                Op::Add => V::Num(a + b),
                Op::Sub => V::Num(a - b),
                Op::Mul => {
                    if bx || by {
                        V::Blank
                    } else {
                        V::Num(a * b)
                    }
                }
                _ => {
                    if bx {
                        V::Blank
                    } else if b == 0.0 {
                        V::Err(CellError::Div0)
                    } else {
                        V::Num(a / b)
                    }
                }
            }
        }
    }
}

/// A problem found by [`check`]: message and character range (`0..0` when not tied to a
/// position).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckError {
    /// What is wrong.
    pub message: String,
    /// First character.
    pub start: usize,
    /// One past the last character.
    pub end: usize,
}

/// What an expression is written for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A measure (filter context).
    Measure,
    /// A calculated column of the given table (row context).
    Column(usize),
}

/// Check `src` against the model: syntax, tables, columns, functions and their argument
/// counts, measures (a measure may not use itself, `self_measure`).
///
/// # Errors
/// The first problem.
pub fn check(
    md: &ModelData,
    src: &str,
    kind: Kind,
    self_measure: Option<&str>,
) -> Result<Expr, CheckError> {
    let e = super::expr::parse(src).map_err(|p| CheckError {
        message: p.message,
        start: p.start,
        end: p.end,
    })?;
    let at = |message: String| CheckError {
        message,
        start: 0,
        end: 0,
    };
    let mut problem: Option<CheckError> = None;
    let mut visit = |x: &Expr| {
        if problem.is_some() {
            return;
        }
        match x {
            Expr::Col { table, column } => {
                if md.table(table).is_none() {
                    problem = Some(at(format!("unknown table {table}")));
                } else if md.column(table, column).is_none() {
                    problem = Some(at(format!("unknown column {table}[{column}]")));
                }
            }
            Expr::Table(t) => {
                if md.table(t).is_none() {
                    problem = Some(at(format!("unknown table {t}")));
                }
            }
            Expr::Bracket(n) => {
                if self_measure.is_some_and(|s| s.eq_ignore_ascii_case(n)) {
                    problem = Some(at(format!("measure [{n}] refers to itself")));
                    return;
                }
                let col = match kind {
                    Kind::Column(t) => md.tables[t].column(n).is_some(),
                    Kind::Measure => false,
                };
                if md.def.measure(n).is_none() && !col {
                    problem = Some(at(format!("unknown measure or column [{n}]")));
                }
            }
            Expr::Call(name, args) => match FUNCTIONS.iter().find(|(f, _, _)| f == name) {
                None => problem = Some(at(format!("unknown function {name}"))),
                Some((_, lo, hi)) if args.len() < *lo || args.len() > *hi => {
                    problem = Some(at(format!("wrong number of arguments for {name}")));
                }
                _ => {}
            },
            _ => {}
        }
    };
    walk(&e, &mut visit);
    if let Some(p) = problem {
        return Err(p);
    }
    // Indirect self reference through other measures.
    if let Some(me) = self_measure {
        let mut seen: Vec<String> = Vec::new();
        let mut todo: Vec<String> = e.brackets().iter().map(|s| s.to_lowercase()).collect();
        while let Some(n) = todo.pop() {
            if n == me.to_lowercase() {
                return Err(at(format!("measure [{me}] refers to itself")));
            }
            if seen.contains(&n) {
                continue;
            }
            seen.push(n.clone());
            if let Some(mi) = md.def.measure(&n) {
                if let Some(Ok(x)) = md.measures.get(mi) {
                    todo.extend(x.brackets().iter().map(|s| s.to_lowercase()));
                }
            }
        }
    }
    Ok(e)
}

fn walk(e: &Expr, f: &mut dyn FnMut(&Expr)) {
    f(e);
    match e {
        Expr::Neg(a) => walk(a, f),
        Expr::Bin(_, a, b) => {
            walk(a, f);
            walk(b, f);
        }
        Expr::Call(_, args) => {
            for a in args {
                walk(a, f);
            }
        }
        _ => {}
    }
}
