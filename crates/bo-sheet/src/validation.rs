//! Data validation (ECMA-376 Part 1, §18.3.1.32 `dataValidation`): in-cell
//! dropdown lists and checking user input against the rule.
//!
//! * `type` (§18.18.21 `ST_DataValidationType`): `whole`, `decimal`, `list`,
//!   `date`, `time`, `textLength`, `custom`;
//! * `operator` (§18.18.20 `ST_DataValidationOperator`, default `between`);
//! * `errorStyle` (§18.18.18): `stop` rejects the input, `warning` and
//!   `information` only report it.

use crate::cells::{resolve_ref, ModelCells};
use crate::numfmt::Locale;
use crate::render::format_cell;
use bo_formula::Value;
use bo_model::sheet::{CellRange, CellRef, CellValue, DataValidation, Workbook};
use serde::{Deserialize, Serialize};

/// Maximum number of items offered by a list dropdown.
pub const MAX_LIST_ITEMS: usize = 1_000;

/// Why an input was refused (or only flagged, for non-`stop` styles).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Rejection {
    pub title: String,
    pub message: String,
    /// `stop` / `warning` / `information`.
    pub style: String,
}

impl Rejection {
    /// Whether the input must be refused.
    pub fn blocks(&self) -> bool {
        self.style == "stop"
    }
}

/// The validation rule covering a cell (the last matching one wins, as
/// later rules replace earlier ones when edited).
pub fn rule_at(wb: &Workbook, sheet: usize, row: u32, col: u32) -> Option<&DataValidation> {
    let at = CellRef::new(row, col);
    wb.sheets
        .get(sheet)?
        .data_validations
        .iter()
        .rev()
        .find(|dv| dv.sqref.iter().any(|r| r.contains(at)))
}

/// Splits an inline list `"a,b,c"` (§18.3.1.32: the formula is a string literal).
fn inline_items(f: &str) -> Option<Vec<String>> {
    let t = f.trim();
    let inner = t.strip_prefix('"')?.strip_suffix('"')?;
    Some(
        inner
            .replace("\"\"", "\"")
            .split(',')
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty())
            .collect(),
    )
}

/// Items of a `list` rule: inline values or the displayed text of the source range.
pub fn list_items(
    wb: &Workbook,
    sheet: usize,
    dv: &DataValidation,
    locale: &Locale,
) -> Vec<String> {
    let Some(f) = dv.formula1.as_deref() else {
        return Vec::new();
    };
    if let Some(items) = inline_items(f) {
        return items.into_iter().take(MAX_LIST_ITEMS).collect();
    }
    let Some((si, range)) = resolve_ref(wb, sheet, f) else {
        return Vec::new();
    };
    let Some(ws) = wb.sheets.get(si) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    'outer: for r in range.first.row..=range.last.row {
        for c in range.first.col..=range.last.col {
            if let Some(cell) = ws.cell(r, c) {
                let t = format_cell(wb, cell, locale).text;
                if !t.is_empty() {
                    out.push(t);
                }
            }
            if out.len() >= MAX_LIST_ITEMS {
                break 'outer;
            }
        }
    }
    out
}

/// Whether the cell shows a dropdown arrow (`showDropDown` = 1 *hides* it).
pub fn has_dropdown(dv: &DataValidation) -> bool {
    dv.kind.as_deref() == Some("list") && !dv.show_drop_down
}

/// Checks `value` (already parsed from the user input) for the cell.
pub fn check(
    wb: &Workbook,
    sheet: usize,
    row: u32,
    col: u32,
    value: &CellValue,
    locale: &Locale,
    now: Option<f64>,
) -> Result<(), Rejection> {
    let Some(dv) = rule_at(wb, sheet, row, col) else {
        return Ok(());
    };
    let kind = dv.kind.as_deref().unwrap_or("none");
    if kind == "none" {
        return Ok(());
    }
    let blank = match value {
        CellValue::Empty => true,
        CellValue::Text(t) => t.is_empty(),
        _ => false,
    };
    if blank {
        return if dv.allow_blank {
            Ok(())
        } else {
            Err(reject(dv))
        };
    }
    let m = ModelCells::new(wb, now);
    let origin = dv
        .sqref
        .first()
        .map_or((row, col), |r| (r.first.row, r.first.col));
    // Thresholds are written for the top-left cell of the rule.
    let shift = |f: Option<&str>| -> Option<f64> {
        let f = f?;
        if let Ok(n) = f.trim().parse::<f64>() {
            return Some(n);
        }
        m.eval_at(sheet, f, origin, row, col)
            .first()
            .as_number()
            .ok()
    };
    let ok = match kind {
        "list" => {
            let text = match value {
                CellValue::Number(_) | CellValue::Bool(_) => {
                    format_cell(wb, &bo_model::sheet::Cell::new(value.clone()), locale).text
                }
                other => other.as_text().unwrap_or_default(),
            };
            let items = list_items(wb, sheet, dv, locale);
            items
                .iter()
                .any(|i| i.to_lowercase() == text.to_lowercase())
        }
        "custom" => {
            // Evaluate with the candidate value in place of the cell.
            let mut tmp = wb.clone();
            if let Some(ws) = tmp.sheets.get_mut(sheet) {
                ws.cells.entry((row, col)).or_default().value = value.clone();
            }
            let m2 = ModelCells::new(&tmp, now);
            let f = dv.formula1.as_deref().unwrap_or("TRUE");
            matches!(
                m2.eval_at(sheet, f, origin, row, col).first(),
                Value::Bool(true)
            ) || matches!(m2.eval_at(sheet, f, origin, row, col).first(), Value::Number(n) if *n != 0.0)
        }
        _ => {
            let x = match (kind, value) {
                ("textLength", v) => v
                    .as_text()
                    .or_else(|| match v {
                        CellValue::Number(n) => Some(format!("{n}")),
                        _ => None,
                    })
                    .map(|t| t.chars().count() as f64),
                (_, CellValue::Number(n)) => Some(*n),
                _ => None,
            };
            match x {
                None => false,
                Some(x) if kind == "whole" && x.fract() != 0.0 => false,
                Some(x) => {
                    let a = shift(dv.formula1.as_deref());
                    let b = shift(dv.formula2.as_deref());
                    compare(dv.operator.as_deref().unwrap_or("between"), x, a, b)
                }
            }
        }
    };
    if ok {
        Ok(())
    } else {
        Err(reject(dv))
    }
}

fn compare(op: &str, x: f64, a: Option<f64>, b: Option<f64>) -> bool {
    let Some(a) = a else { return true };
    match op {
        "between" | "notBetween" => {
            let b = b.unwrap_or(a);
            let (lo, hi) = (a.min(b), a.max(b));
            let inside = x >= lo && x <= hi;
            inside == (op == "between")
        }
        "equal" => (x - a).abs() < 1e-12,
        "notEqual" => (x - a).abs() >= 1e-12,
        "greaterThan" => x > a,
        "greaterThanOrEqual" => x >= a,
        "lessThan" => x < a,
        "lessThanOrEqual" => x <= a,
        _ => true,
    }
}

fn reject(dv: &DataValidation) -> Rejection {
    Rejection {
        title: dv.error_title.clone().unwrap_or_default(),
        message: dv.error.clone().unwrap_or_default(),
        style: dv.error_style.clone().unwrap_or_else(|| "stop".into()),
    }
}

/// A validation rule created from the UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationSpec {
    /// `list`, `whole`, `decimal`, `date`, `time`, `textLength`, `custom` or `none` (remove).
    pub kind: String,
    #[serde(default)]
    pub operator: Option<String>,
    /// List source (`a,b,c` or `=$A$1:$A$5`) or first threshold.
    #[serde(default)]
    pub formula1: String,
    #[serde(default)]
    pub formula2: Option<String>,
    #[serde(default = "yes")]
    pub allow_blank: bool,
    #[serde(default)]
    pub error_title: Option<String>,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub error_style: Option<String>,
    #[serde(default)]
    pub prompt: Option<String>,
}

fn yes() -> bool {
    true
}

fn subtract(r: CellRange, cut: CellRange) -> Vec<CellRange> {
    let disjoint = r.last.row < cut.first.row
        || cut.last.row < r.first.row
        || r.last.col < cut.first.col
        || cut.last.col < r.first.col;
    if disjoint {
        return vec![r];
    }
    let mut out = Vec::new();
    let mk = |r0, c0, r1, c1| CellRange::new(CellRef::new(r0, c0), CellRef::new(r1, c1));
    if r.first.row < cut.first.row {
        out.push(mk(r.first.row, r.first.col, cut.first.row - 1, r.last.col));
    }
    if r.last.row > cut.last.row {
        out.push(mk(cut.last.row + 1, r.first.col, r.last.row, r.last.col));
    }
    let (m0, m1) = (r.first.row.max(cut.first.row), r.last.row.min(cut.last.row));
    if r.first.col < cut.first.col {
        out.push(mk(m0, r.first.col, m1, cut.first.col - 1));
    }
    if r.last.col > cut.last.col {
        out.push(mk(m0, cut.last.col + 1, m1, r.last.col));
    }
    out
}

/// Sets (or with kind `none` removes) the validation of `range`; other rules
/// lose that area.
pub fn set_rule(wb: &mut Workbook, sheet: usize, range: CellRange, spec: &ValidationSpec) -> bool {
    let Some(ws) = wb.sheets.get_mut(sheet) else {
        return false;
    };
    for dv in &mut ws.data_validations {
        dv.sqref = dv.sqref.iter().flat_map(|r| subtract(*r, range)).collect();
    }
    ws.data_validations.retain(|dv| !dv.sqref.is_empty());
    if spec.kind == "none" {
        return true;
    }
    let f1 = spec.formula1.trim();
    let formula1 = if spec.kind == "list" && !f1.starts_with('=') {
        // Inline list → quoted literal (§18.3.1.32).
        format!("\"{}\"", f1.replace('"', "\"\""))
    } else {
        f1.trim_start_matches('=').to_owned()
    };
    if formula1.is_empty() || formula1 == "\"\"" {
        return false;
    }
    let needs_op = !matches!(spec.kind.as_str(), "list" | "custom");
    ws.data_validations.push(DataValidation {
        sqref: vec![range],
        kind: Some(spec.kind.clone()),
        operator: if needs_op {
            spec.operator.clone().filter(|o| o != "between")
        } else {
            None
        },
        allow_blank: spec.allow_blank,
        show_input_message: spec.prompt.is_some(),
        show_error_message: true,
        error_style: spec.error_style.clone().filter(|s| s != "stop"),
        error_title: spec.error_title.clone().filter(|s| !s.is_empty()),
        error: spec.error.clone().filter(|s| !s.is_empty()),
        prompt: spec.prompt.clone().filter(|s| !s.is_empty()),
        formula1: Some(formula1),
        formula2: spec
            .formula2
            .as_deref()
            .map(|f| f.trim().trim_start_matches('=').to_owned())
            .filter(|f| !f.is_empty()),
        ..DataValidation::default()
    });
    true
}

/// Value conversion used by callers that only have formula values.
pub fn cell_value_of(v: &Value) -> CellValue {
    match v {
        Value::Number(n) => CellValue::Number(*n),
        Value::Text(t) => CellValue::Text(t.clone()),
        Value::Bool(b) => CellValue::Bool(*b),
        _ => CellValue::Empty,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bo_model::sheet::Cell;

    fn rng(s: &str) -> CellRange {
        CellRange::parse(s).unwrap()
    }

    fn spec(kind: &str, f1: &str) -> ValidationSpec {
        ValidationSpec {
            kind: kind.into(),
            operator: None,
            formula1: f1.into(),
            formula2: None,
            allow_blank: true,
            error_title: Some("Sai".into()),
            error: Some("Giá trị không hợp lệ".into()),
            error_style: None,
            prompt: None,
        }
    }

    #[test]
    fn lists_inline_and_from_range() {
        let mut wb = Workbook::new();
        let vi = Locale::VI_VN;
        assert!(set_rule(
            &mut wb,
            0,
            rng("A1:A10"),
            &spec("list", "Hà Nội, Huế,Đà Nẵng")
        ));
        let dv = rule_at(&wb, 0, 3, 0).unwrap().clone();
        assert!(has_dropdown(&dv));
        assert_eq!(
            list_items(&wb, 0, &dv, &vi),
            vec!["Hà Nội", "Huế", "Đà Nẵng"]
        );
        let ok = |wb: &Workbook, v: &str| check(wb, 0, 3, 0, &CellValue::Text(v.into()), &vi, None);
        assert!(ok(&wb, "huế").is_ok());
        let err = ok(&wb, "Sài Gòn").unwrap_err();
        assert!(err.blocks());
        assert_eq!(err.message, "Giá trị không hợp lệ");
        assert!(ok(&wb, "").is_ok(), "blank allowed");
        // Range source.
        wb.sheets[0].set_cell(0, 5, Cell::new(CellValue::Number(1.5)));
        wb.sheets[0].set_cell(1, 5, Cell::new(CellValue::Text("x".into())));
        set_rule(&mut wb, 0, rng("B1"), &spec("list", "=$F$1:$F$3"));
        let dv = rule_at(&wb, 0, 0, 1).unwrap().clone();
        assert_eq!(list_items(&wb, 0, &dv, &vi), vec!["1,5", "x"]);
        assert!(check(&wb, 0, 0, 1, &CellValue::Number(1.5), &vi, None).is_ok());
        // Removing.
        set_rule(&mut wb, 0, rng("A1:A10"), &spec("none", ""));
        assert!(rule_at(&wb, 0, 3, 0).is_none());
        assert!(rule_at(&wb, 0, 0, 1).is_some());
    }

    #[test]
    fn numbers_lengths_and_custom() {
        let mut wb = Workbook::new();
        let vi = Locale::VI_VN;
        let mut s = spec("whole", "1");
        s.formula2 = Some("10".into());
        set_rule(&mut wb, 0, rng("A1:A5"), &s);
        let c = |wb: &Workbook, r, v: CellValue| check(wb, 0, r, 0, &v, &vi, None).is_ok();
        assert!(c(&wb, 0, CellValue::Number(5.0)));
        assert!(!c(&wb, 0, CellValue::Number(5.5)));
        assert!(!c(&wb, 0, CellValue::Number(11.0)));
        assert!(!c(&wb, 0, CellValue::Text("abc".into())));
        let mut s = spec("textLength", "3");
        s.operator = Some("lessThanOrEqual".into());
        s.error_style = Some("warning".into());
        set_rule(&mut wb, 0, rng("B1"), &s);
        let r = check(&wb, 0, 0, 1, &CellValue::Text("abcd".into()), &vi, None).unwrap_err();
        assert!(!r.blocks());
        // Custom: relative to the top-left cell.
        set_rule(&mut wb, 0, rng("C1:C3"), &spec("custom", "=C1>D1"));
        wb.sheets[0].set_cell(1, 3, Cell::new(CellValue::Number(5.0)));
        assert!(check(&wb, 0, 1, 2, &CellValue::Number(6.0), &vi, None).is_ok());
        assert!(check(&wb, 0, 1, 2, &CellValue::Number(4.0), &vi, None).is_err());
        assert_eq!(subtract(rng("A1:C3"), rng("B2")).len(), 4);
        assert_eq!(cell_value_of(&Value::Bool(true)), CellValue::Bool(true));
    }
}
