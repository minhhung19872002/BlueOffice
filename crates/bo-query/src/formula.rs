//! The custom column step: a BlueSheet formula evaluated once per row (bo-formula), the
//! query table standing in as a table named `Query` on a sheet of its own: `[@Col]` is the
//! value of the row, `[Col]` the whole column (ECMA-376 Part 1, §18.17.6 structured
//! references), so `=[@Price]*[@Qty]` or `=[@Sales]/SUM([Sales])` work as in a sheet table.

use crate::value::{QTable, Val};
use crate::QueryError;
use bo_formula::provider::TableInfo;
use bo_formula::{CellProvider, EvalConfig, Value};

/// Name of the table formulas see.
pub const TABLE_NAME: &str = "Query";

struct Provider<'a> {
    t: &'a QTable,
    info: TableInfo,
}

impl CellProvider for Provider<'_> {
    fn sheet_count(&self) -> usize {
        1
    }

    fn sheet_name(&self, sheet: usize) -> Option<&str> {
        (sheet == 0).then_some(TABLE_NAME)
    }

    fn cell_value(&self, sheet: usize, row: u32, col: u32) -> Value {
        if sheet != 0 {
            return Value::Empty;
        }
        if row == 0 {
            return self
                .t
                .columns
                .get(col as usize)
                .map_or(Value::Empty, |c| Value::Text(c.clone()));
        }
        let Some(v) = self
            .t
            .rows
            .get(row as usize - 1)
            .and_then(|r| r.get(col as usize))
        else {
            return Value::Empty;
        };
        to_value(v)
    }

    fn table(&self, name: &str) -> Option<TableInfo> {
        name.eq_ignore_ascii_case(TABLE_NAME)
            .then(|| self.info.clone())
    }

    fn table_at(&self, _sheet: usize, _row: u32, _col: u32) -> Option<TableInfo> {
        Some(self.info.clone())
    }

    fn used_extent(&self, _sheet: usize) -> (u32, u32) {
        (self.t.rows.len() as u32 + 1, self.t.columns.len() as u32)
    }
}

fn to_value(v: &Val) -> Value {
    match v {
        Val::Null => Value::Empty,
        Val::Num(n) | Val::Date(n) => Value::Number(*n),
        Val::Text(s) => Value::Text(s.clone()),
        Val::Bool(b) => Value::Bool(*b),
        Val::Err(e) => Value::Error(error_kind(e)),
    }
}

fn error_kind(text: &str) -> bo_formula::ErrorKind {
    use bo_formula::ErrorKind as E;
    match text {
        "#DIV/0!" => E::Div0,
        "#N/A" => E::NA,
        "#REF!" => E::Ref,
        "#NAME?" => E::Name,
        "#NUM!" => E::Num,
        "#NULL!" => E::Null,
        _ => E::Value,
    }
}

fn from_value(v: &Value) -> Val {
    match v {
        Value::Empty => Val::Null,
        Value::Number(n) => Val::Num(*n),
        Value::Text(s) => Val::Text(s.clone()),
        Value::Bool(b) => Val::Bool(*b),
        Value::Error(e) => Val::Err(e.to_string()),
        Value::Array(a) => from_value(a.get(0, 0)),
        _ => Val::Err("#VALUE!".into()),
    }
}

/// The values of `formula` for every row of `t`.
pub fn column(t: &QTable, formula: &str) -> Result<Vec<Val>, QueryError> {
    let expr = bo_formula::parse(formula).map_err(|e| QueryError::Formula(e.to_string()))?;
    let n = t.rows.len() as u32;
    let info = TableInfo {
        name: TABLE_NAME.into(),
        sheet: 0,
        first_row: 0,
        last_row: n.max(1),
        first_col: 0,
        columns: t.columns.clone(),
        header_row: true,
        totals_row: false,
    };
    let p = Provider { t, info };
    let config = EvalConfig::default();
    let width = t.columns.len() as u32;
    Ok((0..n)
        .map(|r| {
            let v = bo_formula::evaluate(&expr, &p, 0, r + 1, width, &config);
            let v = match v {
                Value::Ref(a) => p.cell_value(0, a.r1, a.c1),
                other => other,
            };
            from_value(&v)
        })
        .collect())
}
