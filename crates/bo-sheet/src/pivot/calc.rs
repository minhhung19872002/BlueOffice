//! Calculated fields (ECMA-376 Part 1, §18.10.1.3 `cacheField/@formula`).
//!
//! A calculated field's formula names other cache fields, bare (`Revenue*0.1`)
//! or quoted when the name is not a plain identifier (`'Doanh thu'/'Số lượng'`).
//! As in the specification's model of calculated fields, the formula is
//! evaluated once per report cell over the **sums** of the named fields within
//! that cell (not record by record), with the operators and functions of
//! `bo-formula` (§18.17).

use bo_formula::{evaluate, parse, CellProvider, EvalConfig, Expr, Value};
use bo_model::sheet::{CellValue, PivotCacheField};
use std::collections::HashMap;

/// Placeholder name of a quoted field reference.
const PLACEHOLDER: &str = "_BOPF";

/// A compiled calculated field.
#[derive(Debug, Clone)]
pub(crate) struct CalcField {
    expr: Expr,
    /// Cache fields named by the formula, by lower-case name used in `expr`.
    pub(crate) refs: Vec<(String, u32)>,
}

/// Replace quoted field names by placeholders (`'Doanh thu'` → `_BOPF3`),
/// leaving string literals untouched.
fn unquote(formula: &str, fields: &[PivotCacheField]) -> Result<String, String> {
    let mut out = String::with_capacity(formula.len());
    let mut chars = formula.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' => {
                out.push(c);
                while let Some(d) = chars.next() {
                    out.push(d);
                    if d == '"' {
                        if chars.peek() == Some(&'"') {
                            out.push('"');
                            chars.next();
                        } else {
                            break;
                        }
                    }
                }
            }
            '\'' => {
                let mut name = String::new();
                loop {
                    match chars.next() {
                        Some('\'') if chars.peek() == Some(&'\'') => {
                            name.push('\'');
                            chars.next();
                        }
                        Some('\'') => break,
                        Some(d) => name.push(d),
                        None => return Err("unterminated field name".into()),
                    }
                }
                let i =
                    field_index(fields, &name).ok_or_else(|| format!("unknown field '{name}'"))?;
                out.push_str(&format!("{PLACEHOLDER}{i}"));
            }
            _ => out.push(c),
        }
    }
    Ok(out)
}

fn field_index(fields: &[PivotCacheField], name: &str) -> Option<usize> {
    let lower = name.trim().to_lowercase();
    fields.iter().position(|f| f.name.to_lowercase() == lower)
}

/// Compile `formula` (with or without `=`) against the cache `fields`. Every
/// name must be a database field of the cache.
pub(crate) fn compile(formula: &str, fields: &[PivotCacheField]) -> Result<CalcField, String> {
    let text = formula.trim();
    let text = text.strip_prefix('=').unwrap_or(text);
    if text.is_empty() {
        return Err("empty formula".into());
    }
    let replaced = unquote(text, fields)?;
    let expr = parse(&replaced).map_err(|e| format!("{e:?}"))?;
    let mut refs: Vec<(String, u32)> = Vec::new();
    let mut error = None;
    expr.walk(&mut |e| {
        if let Expr::Name { sheet: None, name } = e {
            let lower = name.to_lowercase();
            let idx = lower
                .strip_prefix(&PLACEHOLDER.to_lowercase())
                .and_then(|n| n.parse::<usize>().ok())
                .or_else(|| field_index(fields, name));
            match idx {
                Some(i) if fields.get(i).is_some_and(|f| !f.derived) => {
                    if !refs.iter().any(|(n, _)| *n == lower) {
                        refs.push((lower, i as u32));
                    }
                }
                _ => error = Some(format!("unknown field '{name}'")),
            }
        }
        if let Expr::Ref(_) = e {
            error = Some("cell references are not allowed in a calculated field".into());
        }
    });
    if let Some(e) = error {
        return Err(e);
    }
    Ok(CalcField { expr, refs })
}

/// Name lookups of one evaluation: field name → sum.
struct Sums<'a>(&'a HashMap<String, f64>);

impl CellProvider for Sums<'_> {
    fn sheet_count(&self) -> usize {
        0
    }

    fn sheet_name(&self, _sheet: usize) -> Option<&str> {
        None
    }

    fn cell_value(&self, _sheet: usize, _row: u32, _col: u32) -> Value {
        Value::Empty
    }

    fn defined_name(&self, _sheet: usize, name: &str) -> Option<Expr> {
        self.0.get(&name.to_lowercase()).map(|v| Expr::Number(*v))
    }
}

impl CalcField {
    /// Evaluate with the sum of each referenced field given by `sum`.
    pub(crate) fn eval(&self, sum: impl Fn(u32) -> f64) -> CellValue {
        let values: HashMap<String, f64> = self
            .refs
            .iter()
            .map(|(n, f)| (n.clone(), sum(*f)))
            .collect();
        let v = evaluate(&self.expr, &Sums(&values), 0, 0, 0, &EvalConfig::default());
        crate::formula::to_cell_value(&v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fields() -> Vec<PivotCacheField> {
        ["Vùng", "Doanh thu", "Qty"]
            .iter()
            .map(|n| PivotCacheField {
                name: (*n).into(),
                ..PivotCacheField::default()
            })
            .collect()
    }

    #[test]
    fn quoted_and_bare_names() {
        let c = compile("='Doanh thu'/qty", &fields()).unwrap();
        assert_eq!(c.refs.len(), 2);
        let sum = |f: u32| if f == 1 { 30.0 } else { 4.0 };
        assert_eq!(c.eval(sum), CellValue::Number(7.5));
        let c = compile("IF(Qty>0,'Doanh thu'*0.1,\"n/a 'x'\")", &fields()).unwrap();
        assert_eq!(c.eval(sum), CellValue::Number(3.0));
        assert_eq!(c.eval(|_| 0.0), CellValue::Text("n/a 'x'".into()));
        // #DIV/0! like a cell formula.
        let c = compile("'Doanh thu'/Qty", &fields()).unwrap();
        assert_eq!(
            c.eval(|f| if f == 2 { 0.0 } else { 1.0 }),
            CellValue::Error(bo_model::sheet::CellError::Div0)
        );
    }

    #[test]
    fn rejects_unknown_names_and_cells() {
        assert!(compile("Price*2", &fields()).is_err());
        assert!(compile("'Giá'*2", &fields()).is_err());
        assert!(compile("A1*2", &fields()).is_err());
        assert!(compile("", &fields()).is_err());
    }
}
