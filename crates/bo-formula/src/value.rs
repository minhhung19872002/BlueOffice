//! Formula values, error values and implicit type coercion.
//!
//! ECMA-376 Part 1, §18.17.2 (formula syntax: constants, error values) and
//! §18.17.3 / §18.17.7 (evaluation rules used by the predefined functions).

use std::cmp::Ordering;
use std::fmt;
use std::sync::Arc;

use crate::ast::Expr;

/// A spreadsheet error value (ECMA-376 Part 1, §18.17.2 error-constant).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ErrorKind {
    /// `#NULL!` — empty intersection.
    Null,
    /// `#DIV/0!` — division by zero.
    Div0,
    /// `#VALUE!` — wrong operand type.
    Value,
    /// `#REF!` — invalid reference.
    Ref,
    /// `#NAME?` — unknown name or function.
    Name,
    /// `#NUM!` — invalid numeric result or argument.
    Num,
    /// `#N/A` — value not available.
    NA,
    /// `#GETTING_DATA` — asynchronous value not ready.
    GettingData,
    /// `#SPILL!` — a dynamic array result cannot be placed.
    Spill,
    /// `#CALC!` — calculation error (empty array, circular reference, ...).
    Calc,
}

impl ErrorKind {
    /// All error kinds, in a stable order.
    pub const ALL: [ErrorKind; 10] = [
        ErrorKind::Null,
        ErrorKind::Div0,
        ErrorKind::Value,
        ErrorKind::Ref,
        ErrorKind::Name,
        ErrorKind::Num,
        ErrorKind::NA,
        ErrorKind::GettingData,
        ErrorKind::Spill,
        ErrorKind::Calc,
    ];

    /// The literal spelling of the error, e.g. `#DIV/0!`.
    pub fn as_str(self) -> &'static str {
        match self {
            ErrorKind::Null => "#NULL!",
            ErrorKind::Div0 => "#DIV/0!",
            ErrorKind::Value => "#VALUE!",
            ErrorKind::Ref => "#REF!",
            ErrorKind::Name => "#NAME?",
            ErrorKind::Num => "#NUM!",
            ErrorKind::NA => "#N/A",
            ErrorKind::GettingData => "#GETTING_DATA",
            ErrorKind::Spill => "#SPILL!",
            ErrorKind::Calc => "#CALC!",
        }
    }

    /// Parse an error literal (case-insensitive).
    pub fn parse(s: &str) -> Option<Self> {
        ErrorKind::ALL
            .into_iter()
            .find(|e| e.as_str().eq_ignore_ascii_case(s))
    }

    /// The number returned by `ERROR.TYPE` (ECMA-376 Part 1, §18.17.7 ERROR.TYPE).
    pub fn type_code(self) -> f64 {
        match self {
            ErrorKind::Null => 1.0,
            ErrorKind::Div0 => 2.0,
            ErrorKind::Value => 3.0,
            ErrorKind::Ref => 4.0,
            ErrorKind::Name => 5.0,
            ErrorKind::Num => 6.0,
            ErrorKind::NA => 7.0,
            ErrorKind::GettingData => 8.0,
            ErrorKind::Spill => 9.0,
            ErrorKind::Calc => 14.0,
        }
    }
}

impl fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A resolved rectangular cell area (0-based, inclusive bounds).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Area {
    /// Sheet index.
    pub sheet: usize,
    /// First row.
    pub r1: u32,
    /// First column.
    pub c1: u32,
    /// Last row (inclusive).
    pub r2: u32,
    /// Last column (inclusive).
    pub c2: u32,
}

impl Area {
    /// An area covering a single cell.
    pub fn cell(sheet: usize, row: u32, col: u32) -> Self {
        Area {
            sheet,
            r1: row,
            c1: col,
            r2: row,
            c2: col,
        }
    }

    /// Build a normalized area from two corners.
    pub fn new(sheet: usize, r1: u32, c1: u32, r2: u32, c2: u32) -> Self {
        Area {
            sheet,
            r1: r1.min(r2),
            c1: c1.min(c2),
            r2: r1.max(r2),
            c2: c1.max(c2),
        }
    }

    /// Number of rows.
    pub fn rows(&self) -> u32 {
        self.r2 - self.r1 + 1
    }

    /// Number of columns.
    pub fn cols(&self) -> u32 {
        self.c2 - self.c1 + 1
    }

    /// Number of cells.
    pub fn size(&self) -> u64 {
        u64::from(self.rows()) * u64::from(self.cols())
    }

    /// Whether the area is a single cell.
    pub fn is_single(&self) -> bool {
        self.r1 == self.r2 && self.c1 == self.c2
    }

    /// Whether the area contains the cell.
    pub fn contains(&self, sheet: usize, row: u32, col: u32) -> bool {
        self.sheet == sheet && row >= self.r1 && row <= self.r2 && col >= self.c1 && col <= self.c2
    }

    /// Intersection of two areas, if any.
    pub fn intersect(&self, other: &Area) -> Option<Area> {
        if self.sheet != other.sheet {
            return None;
        }
        let r1 = self.r1.max(other.r1);
        let r2 = self.r2.min(other.r2);
        let c1 = self.c1.max(other.c1);
        let c2 = self.c2.min(other.c2);
        (r1 <= r2 && c1 <= c2).then_some(Area {
            sheet: self.sheet,
            r1,
            c1,
            r2,
            c2,
        })
    }
}

/// A 2-D array of values stored row-major.
#[derive(Clone, Debug, PartialEq)]
pub struct Array {
    rows: usize,
    cols: usize,
    data: Vec<Value>,
}

impl Array {
    /// Create an array; `data.len()` must equal `rows * cols` and both dimensions must be > 0.
    pub fn new(rows: usize, cols: usize, data: Vec<Value>) -> Self {
        assert!(
            rows > 0 && cols > 0 && data.len() == rows * cols,
            "bad array shape"
        );
        Array { rows, cols, data }
    }

    /// Create an array filled with one value.
    pub fn filled(rows: usize, cols: usize, v: &Value) -> Self {
        Array::new(rows, cols, vec![v.clone(); rows * cols])
    }

    /// Create an array from rows; returns `None` if empty or ragged.
    pub fn from_rows(rows: Vec<Vec<Value>>) -> Option<Self> {
        let r = rows.len();
        let c = rows.first()?.len();
        if c == 0 || rows.iter().any(|row| row.len() != c) {
            return None;
        }
        Some(Array::new(r, c, rows.into_iter().flatten().collect()))
    }

    /// A single-column array.
    pub fn column(values: Vec<Value>) -> Option<Self> {
        if values.is_empty() {
            return None;
        }
        Some(Array::new(values.len(), 1, values))
    }

    /// A single-row array.
    pub fn row(values: Vec<Value>) -> Option<Self> {
        if values.is_empty() {
            return None;
        }
        Some(Array::new(1, values.len(), values))
    }

    /// Row count.
    pub fn rows(&self) -> usize {
        self.rows
    }

    /// Column count.
    pub fn cols(&self) -> usize {
        self.cols
    }

    /// Element at (row, col); panics when out of bounds.
    pub fn get(&self, row: usize, col: usize) -> &Value {
        &self.data[row * self.cols + col]
    }

    /// Element with broadcasting of single rows/columns, `None` when out of range.
    pub fn get_broadcast(&self, row: usize, col: usize) -> Option<&Value> {
        let r = if self.rows == 1 { 0 } else { row };
        let c = if self.cols == 1 { 0 } else { col };
        (r < self.rows && c < self.cols).then(|| &self.data[r * self.cols + c])
    }

    /// Row-major elements.
    pub fn data(&self) -> &[Value] {
        &self.data
    }

    /// Consume into row-major elements.
    pub fn into_data(self) -> Vec<Value> {
        self.data
    }

    /// One row as a slice.
    pub fn row_slice(&self, row: usize) -> &[Value] {
        &self.data[row * self.cols..(row + 1) * self.cols]
    }

    /// Transposed copy.
    #[must_use]
    pub fn transpose(&self) -> Array {
        let mut data = Vec::with_capacity(self.data.len());
        for c in 0..self.cols {
            for r in 0..self.rows {
                data.push(self.get(r, c).clone());
            }
        }
        Array::new(self.cols, self.rows, data)
    }
}

/// A user-defined function value created by `LAMBDA`.
#[derive(Clone, Debug)]
pub struct Lambda {
    /// Parameter names (upper-cased).
    pub params: Vec<String>,
    /// Body expression.
    pub body: Expr,
    /// Captured `LET`/`LAMBDA` bindings.
    pub(crate) env: Vec<(String, Value)>,
}

/// A formula or cell value.
#[derive(Clone, Debug, Default)]
pub enum Value {
    /// Empty cell / missing argument.
    #[default]
    Empty,
    /// IEEE-754 double.
    Number(f64),
    /// Text.
    Text(String),
    /// Boolean.
    Bool(bool),
    /// Error value.
    Error(ErrorKind),
    /// 2-D array (dynamic array result or array constant).
    Array(Arc<Array>),
    /// Reference to a single area.
    Ref(Area),
    /// Reference to several areas (union, 3-D reference).
    Refs(Arc<Vec<Area>>),
    /// A `LAMBDA` function value.
    Lambda(Arc<Lambda>),
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Empty, Value::Empty) => true,
            (Value::Number(a), Value::Number(b)) => a == b,
            (Value::Text(a), Value::Text(b)) => a == b,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Error(a), Value::Error(b)) => a == b,
            (Value::Array(a), Value::Array(b)) => a == b,
            (Value::Ref(a), Value::Ref(b)) => a == b,
            (Value::Refs(a), Value::Refs(b)) => a == b,
            (Value::Lambda(a), Value::Lambda(b)) => Arc::ptr_eq(a, b),
            _ => false,
        }
    }
}

impl From<f64> for Value {
    fn from(n: f64) -> Self {
        Value::number(n)
    }
}

impl From<bool> for Value {
    fn from(b: bool) -> Self {
        Value::Bool(b)
    }
}

impl From<&str> for Value {
    fn from(s: &str) -> Self {
        Value::Text(s.to_owned())
    }
}

impl From<String> for Value {
    fn from(s: String) -> Self {
        Value::Text(s)
    }
}

impl From<ErrorKind> for Value {
    fn from(e: ErrorKind) -> Self {
        Value::Error(e)
    }
}

impl From<Array> for Value {
    fn from(a: Array) -> Self {
        Value::Array(Arc::new(a))
    }
}

impl<E: Into<Value>> From<Result<f64, E>> for Value {
    fn from(r: Result<f64, E>) -> Self {
        match r {
            Ok(n) => Value::number(n),
            Err(e) => e.into(),
        }
    }
}

impl Value {
    /// A number value; NaN and infinities become `#NUM!`.
    pub fn number(n: f64) -> Value {
        if n.is_finite() {
            // Normalize negative zero.
            Value::Number(if n == 0.0 { 0.0 } else { n })
        } else {
            Value::Error(ErrorKind::Num)
        }
    }

    /// Whether this is an error value.
    pub fn is_error(&self) -> bool {
        matches!(self, Value::Error(_))
    }

    /// The error kind, if this is an error.
    pub fn error(&self) -> Option<ErrorKind> {
        match self {
            Value::Error(e) => Some(*e),
            _ => None,
        }
    }

    /// Whether this is an array.
    pub fn is_array(&self) -> bool {
        matches!(self, Value::Array(_))
    }

    /// The top-left element of an array, or the value itself.
    pub fn first(&self) -> &Value {
        match self {
            Value::Array(a) => a.get(0, 0),
            v => v,
        }
    }

    /// Coerce a scalar to a number (1900 date system for date strings).
    ///
    /// Empty → 0, booleans → 1/0, numeric text is parsed, other text → `#VALUE!`.
    pub fn as_number(&self) -> Result<f64, ErrorKind> {
        match self.first() {
            Value::Empty => Ok(0.0),
            Value::Number(n) => Ok(*n),
            Value::Bool(b) => Ok(if *b { 1.0 } else { 0.0 }),
            Value::Text(s) => {
                text_to_number(s, crate::date::DateSystem::Date1900).ok_or(ErrorKind::Value)
            }
            Value::Error(e) => Err(*e),
            _ => Err(ErrorKind::Value),
        }
    }

    /// Coerce a scalar to text (numbers use the General format).
    pub fn as_text(&self) -> Result<String, ErrorKind> {
        match self.first() {
            Value::Empty => Ok(String::new()),
            Value::Number(n) => Ok(number_to_text(*n)),
            Value::Bool(b) => Ok(if *b { "TRUE" } else { "FALSE" }.to_owned()),
            Value::Text(s) => Ok(s.clone()),
            Value::Error(e) => Err(*e),
            _ => Err(ErrorKind::Value),
        }
    }

    /// Coerce a scalar to a boolean.
    pub fn as_bool(&self) -> Result<bool, ErrorKind> {
        match self.first() {
            Value::Empty => Ok(false),
            Value::Number(n) => Ok(*n != 0.0),
            Value::Bool(b) => Ok(*b),
            Value::Text(s) => {
                if s.eq_ignore_ascii_case("TRUE") {
                    Ok(true)
                } else if s.eq_ignore_ascii_case("FALSE") {
                    Ok(false)
                } else {
                    Err(ErrorKind::Value)
                }
            }
            Value::Error(e) => Err(*e),
            _ => Err(ErrorKind::Value),
        }
    }

    /// Display string as the spreadsheet would show it in General format.
    pub fn display(&self) -> String {
        match self.first() {
            Value::Error(e) => e.as_str().to_owned(),
            Value::Ref(_) | Value::Refs(_) => "#VALUE!".to_owned(),
            Value::Lambda(_) => "#CALC!".to_owned(),
            v => v.as_text().unwrap_or_default(),
        }
    }
}

/// Round to 15 significant digits, the precision used for comparisons.
pub fn round_sig15(x: f64) -> f64 {
    if x == 0.0 || !x.is_finite() {
        return x;
    }
    format!("{x:.14e}").parse().unwrap_or(x)
}

/// Equality of two numbers at 15 significant digits.
pub fn num_eq(a: f64, b: f64) -> bool {
    if a == b {
        return true;
    }
    // Two numbers with the same 15-significant-digit rounding differ by less than
    // one unit in the 15th digit, i.e. by less than 1e-14 of the larger magnitude;
    // anything further apart is unequal without the (slow) decimal rounding.
    let scale = a.abs().max(b.abs());
    if (a - b).abs() >= 1e-14 * scale || a.is_nan() || b.is_nan() {
        return false;
    }
    round_sig15(a) == round_sig15(b)
}

/// Rank of a scalar type in sort/compare order: numbers < text < booleans.
fn type_rank(v: &Value) -> u8 {
    match v {
        Value::Number(_) | Value::Empty => 0,
        Value::Text(_) => 1,
        Value::Bool(_) => 2,
        _ => 3,
    }
}

/// Compare two text values case-insensitively.
pub fn cmp_text(a: &str, b: &str) -> Ordering {
    let la = a.to_lowercase();
    let lb = b.to_lowercase();
    la.cmp(&lb)
}

/// Compare two non-error scalars with spreadsheet semantics.
///
/// Empty compares as 0 against numbers, `""` against text and `FALSE` against booleans.
/// Numbers compare at 15 significant digits; text is case-insensitive.
pub fn compare_values(a: &Value, b: &Value) -> Ordering {
    let a = a.first();
    let b = b.first();
    let (a, b) = match (a, b) {
        (Value::Empty, Value::Text(_)) => (&Value::Text(String::new()), b),
        (Value::Text(_), Value::Empty) => (a, &Value::Text(String::new())),
        (Value::Empty, Value::Bool(_)) => (&Value::Bool(false), b),
        (Value::Bool(_), Value::Empty) => (a, &Value::Bool(false)),
        _ => (a, b),
    };
    match (a, b) {
        (Value::Text(x), Value::Text(y)) => cmp_text(x, y),
        (Value::Bool(x), Value::Bool(y)) => x.cmp(y),
        (x @ (Value::Number(_) | Value::Empty), y @ (Value::Number(_) | Value::Empty)) => {
            let x = x.as_number().unwrap_or(0.0);
            let y = y.as_number().unwrap_or(0.0);
            if num_eq(x, y) {
                Ordering::Equal
            } else {
                x.partial_cmp(&y).unwrap_or(Ordering::Equal)
            }
        }
        (x, y) => type_rank(x).cmp(&type_rank(y)),
    }
}

/// Parse plain numeric text: optional sign, digits with optional thousands
/// separators, decimal point, exponent and a trailing `%`.
pub fn parse_number_text(s: &str) -> Option<f64> {
    let t = s.trim();
    if t.is_empty() {
        return None;
    }
    let (t, pct) = match t.strip_suffix('%') {
        Some(rest) => (rest.trim_end(), true),
        None => (t, false),
    };
    let (neg, body) = match t.as_bytes().first() {
        Some(b'-') => (true, &t[1..]),
        Some(b'+') => (false, &t[1..]),
        _ => (false, t),
    };
    let body = body.trim_start();
    // Parenthesized negatives "(5)" are accepted as -5.
    let (neg, body) = if body.starts_with('(') && body.ends_with(')') && body.len() > 2 {
        (!neg, &body[1..body.len() - 1])
    } else {
        (neg, body)
    };
    if body.is_empty()
        || !body
            .bytes()
            .next()
            .is_some_and(|c| c.is_ascii_digit() || c == b'.')
    {
        return None;
    }
    let mut cleaned = String::with_capacity(body.len());
    let mut seen_dot = false;
    let mut seen_exp = false;
    for (i, c) in body.char_indices() {
        match c {
            '0'..='9' => cleaned.push(c),
            ',' if !seen_dot && !seen_exp => {
                // Thousands separator must be followed by exactly 3 digits.
                let rest = &body[i + 1..];
                let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
                if digits != 3 {
                    return None;
                }
            }
            '.' if !seen_dot && !seen_exp => {
                seen_dot = true;
                cleaned.push('.');
            }
            'e' | 'E' if !seen_exp => {
                seen_exp = true;
                cleaned.push('e');
            }
            '+' | '-' if cleaned.ends_with('e') => cleaned.push(c),
            _ => return None,
        }
    }
    let mut n: f64 = cleaned.parse().ok()?;
    if neg {
        n = -n;
    }
    if pct {
        n /= 100.0;
    }
    n.is_finite().then_some(n)
}

/// Convert text to a number the way arithmetic coercion does: plain numbers,
/// percentages, then date/time strings.
pub fn text_to_number(s: &str, system: crate::date::DateSystem) -> Option<f64> {
    parse_number_text(s).or_else(|| crate::date::parse_datetime_text(s, system))
}

/// Format a number in the General format used for text conversion
/// (up to 15 significant digits, scientific notation for very large/small).
pub fn number_to_text(x: f64) -> String {
    if x == 0.0 {
        return "0".to_owned();
    }
    if !x.is_finite() {
        return "#NUM!".to_owned();
    }
    let s = format!("{:.14e}", x.abs());
    let (mant, exp) = s.split_once('e').unwrap_or((&s, "0"));
    let exp: i32 = exp.parse().unwrap_or(0);
    let digits: String = mant.chars().filter(char::is_ascii_digit).collect();
    let digits = digits.trim_end_matches('0');
    let digits = if digits.is_empty() { "0" } else { digits };
    let sign = if x < 0.0 { "-" } else { "" };
    if (-10..15).contains(&exp) {
        let mut out = String::from(sign);
        if exp < 0 {
            out.push_str("0.");
            for _ in 0..(-exp - 1) {
                out.push('0');
            }
            out.push_str(digits);
        } else {
            let int_len = (exp + 1) as usize;
            if digits.len() <= int_len {
                out.push_str(digits);
                for _ in digits.len()..int_len {
                    out.push('0');
                }
            } else {
                out.push_str(&digits[..int_len]);
                out.push('.');
                out.push_str(&digits[int_len..]);
            }
        }
        out
    } else {
        let mut out = String::from(sign);
        out.push_str(&digits[..1]);
        if digits.len() > 1 {
            out.push('.');
            out.push_str(&digits[1..]);
        }
        out.push('E');
        out.push(if exp < 0 { '-' } else { '+' });
        out.push_str(&format!("{:02}", exp.abs()));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn general_number_text() {
        assert_eq!(number_to_text(1.0), "1");
        assert_eq!(number_to_text(-2.5), "-2.5");
        assert_eq!(number_to_text(1.0 / 3.0), "0.333333333333333");
        assert_eq!(number_to_text(0.1 + 0.2), "0.3");
        assert_eq!(number_to_text(1e20), "1E+20");
        assert_eq!(number_to_text(123_456.0), "123456");
        assert_eq!(number_to_text(0.001), "0.001");
    }

    #[test]
    fn numeric_text() {
        assert_eq!(parse_number_text(" 12 "), Some(12.0));
        assert_eq!(parse_number_text("1,234.5"), Some(1234.5));
        assert_eq!(parse_number_text("50%"), Some(0.5));
        assert_eq!(parse_number_text("1e3"), Some(1000.0));
        assert_eq!(parse_number_text("-.5"), Some(-0.5));
        assert_eq!(parse_number_text("abc"), None);
        assert_eq!(parse_number_text("1,23"), None);
    }

    #[test]
    fn compare_rules() {
        assert_eq!(
            compare_values(&"abc".into(), &"ABC".into()),
            Ordering::Equal
        );
        assert_eq!(compare_values(&1.0.into(), &"a".into()), Ordering::Less);
        assert_eq!(compare_values(&"z".into(), &true.into()), Ordering::Less);
        assert_eq!(compare_values(&Value::Empty, &0.0.into()), Ordering::Equal);
        assert!(num_eq(0.1 + 0.2, 0.3));
    }
}
