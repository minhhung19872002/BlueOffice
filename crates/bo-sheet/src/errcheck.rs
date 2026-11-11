//! Background error checking of a sheet (the "error checking" auditing tool).
//!
//! Each rule flags cells worth a second look; the user can ignore a finding,
//! which is saved in the sheet as an `ignoredError` (ECMA-376 Part 1,
//! §18.3.1.51 `ignoredErrors`, §18.3.1.50 `ignoredError`). The rules and their
//! wording are BlueSheet's own:
//!
//! * `evalError` — a formula whose result is an error value;
//! * `inconsistentFormula` — a formula that differs (in R1C1 form) from the
//!   identical formulas on both sides of it, in its row or its column;
//! * `numberAsText` — text that reads as a number;
//! * `emptyRef` — a formula reading a single cell that is empty;
//! * `unlockedFormula` — a formula in a cell not locked for protection
//!   (§18.8.33 `protection/@locked`).

use bo_formula::{format_with, parse_with, ParseOptions, RefStyle};
use bo_model::sheet::{CellRange, CellRef, CellValue, RawXml, Workbook, Worksheet};
use serde::{Deserialize, Serialize};

use crate::calcopts::{element, escape};

/// An error-checking rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ErrorRule {
    /// A formula evaluating to an error.
    EvalError,
    /// A formula unlike its neighbours.
    InconsistentFormula,
    /// A number stored as text.
    NumberAsText,
    /// A formula reading an empty cell.
    EmptyRef,
    /// A formula in an unlocked cell.
    UnlockedFormula,
}

impl ErrorRule {
    /// Every rule, in checking order.
    pub const ALL: [ErrorRule; 5] = [
        ErrorRule::EvalError,
        ErrorRule::InconsistentFormula,
        ErrorRule::NumberAsText,
        ErrorRule::EmptyRef,
        ErrorRule::UnlockedFormula,
    ];

    /// The `ignoredError` attribute recording this rule (§18.3.1.50).
    pub fn attribute(self) -> &'static str {
        match self {
            ErrorRule::EvalError => "evalError",
            ErrorRule::InconsistentFormula => "formula",
            ErrorRule::NumberAsText => "numberStoredAsText",
            ErrorRule::EmptyRef => "emptyCellReference",
            ErrorRule::UnlockedFormula => "unlockedFormula",
        }
    }
}

/// Which rules are on (all by default).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
#[allow(clippy::struct_excessive_bools)]
pub struct ErrorRules {
    /// [`ErrorRule::EvalError`].
    pub eval_error: bool,
    /// [`ErrorRule::InconsistentFormula`].
    pub inconsistent_formula: bool,
    /// [`ErrorRule::NumberAsText`].
    pub number_as_text: bool,
    /// [`ErrorRule::EmptyRef`].
    pub empty_ref: bool,
    /// [`ErrorRule::UnlockedFormula`].
    pub unlocked_formula: bool,
}

impl Default for ErrorRules {
    fn default() -> Self {
        ErrorRules {
            eval_error: true,
            inconsistent_formula: true,
            number_as_text: true,
            empty_ref: true,
            unlocked_formula: true,
        }
    }
}

impl ErrorRules {
    /// Whether `rule` is on.
    pub fn on(&self, rule: ErrorRule) -> bool {
        match rule {
            ErrorRule::EvalError => self.eval_error,
            ErrorRule::InconsistentFormula => self.inconsistent_formula,
            ErrorRule::NumberAsText => self.number_as_text,
            ErrorRule::EmptyRef => self.empty_ref,
            ErrorRule::UnlockedFormula => self.unlocked_formula,
        }
    }
}

/// One flagged cell.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Finding {
    /// Row (0-based).
    pub row: u32,
    /// Column (0-based).
    pub col: u32,
    /// The rule that flagged the cell.
    pub rule: ErrorRule,
    /// `inconsistentFormula`: the neighbours' formula written for this cell
    /// (A1, without `=`), offered as the fix.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,
    /// `numberAsText`: the number the text reads as.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub number: Option<f64>,
    /// `emptyRef`: the empty cell (A1, sheet-qualified when elsewhere).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub empty: Option<String>,
}

/// Checks sheet `sheet`. `engine` gives the formula precedents (the
/// `emptyRef` rule is skipped without it); `number` reads text as a number
/// the way typed input is read. Findings are in row-major order, ignored ones
/// (§18.3.1.51) left out.
pub fn check(
    wb: &Workbook,
    sheet: usize,
    rules: &ErrorRules,
    engine: Option<&bo_formula::Workbook>,
    number: &dyn Fn(&str) -> Option<f64>,
) -> Vec<Finding> {
    let Some(ws) = wb.sheets.get(sheet) else {
        return Vec::new();
    };
    let ignored = ignored(ws);
    let mut keys: Vec<(u32, u32)> = ws.cells.keys().copied().collect();
    keys.sort_unstable();
    let mut out = Vec::new();
    let mut r1c1 = std::collections::HashMap::new();
    for (row, col) in keys {
        let Some(cell) = ws.cells.get(&(row, col)) else {
            continue;
        };
        let at = CellRef::new(row, col);
        let skip = |rule: ErrorRule| {
            !rules.on(rule)
                || ignored
                    .iter()
                    .any(|(r, list)| r.contains(at) && list.contains(&rule))
        };
        let finding = |rule| Finding {
            row,
            col,
            rule,
            suggestion: None,
            number: None,
            empty: None,
        };
        let formula = cell.formula.as_ref().filter(|f| !f.text.is_empty());
        if formula.is_some() {
            if !skip(ErrorRule::EvalError) && matches!(cell.value, CellValue::Error(_)) {
                out.push(finding(ErrorRule::EvalError));
            }
            if !skip(ErrorRule::InconsistentFormula) {
                if let Some(s) = inconsistent(ws, row, col, &mut r1c1) {
                    out.push(Finding {
                        suggestion: Some(s),
                        ..finding(ErrorRule::InconsistentFormula)
                    });
                }
            }
            if !skip(ErrorRule::EmptyRef) {
                if let Some(e) = engine.and_then(|e| empty_precedent(wb, e, sheet, row, col)) {
                    out.push(Finding {
                        empty: Some(e),
                        ..finding(ErrorRule::EmptyRef)
                    });
                }
            }
            let unlocked = wb
                .styles
                .xf(cell.style)
                .and_then(|x| x.protection.as_ref())
                .and_then(|p| p.locked)
                == Some(false);
            if unlocked && !skip(ErrorRule::UnlockedFormula) {
                out.push(finding(ErrorRule::UnlockedFormula));
            }
        } else if cell.formula.is_none() && !skip(ErrorRule::NumberAsText) {
            if let CellValue::Text(t) = &cell.value {
                if let Some(n) = number(t) {
                    out.push(Finding {
                        number: Some(n),
                        ..finding(ErrorRule::NumberAsText)
                    });
                }
            }
        }
    }
    out
}

/// R1C1 text of the formula in a cell (cached).
fn r1c1_of(
    ws: &Worksheet,
    row: u32,
    col: u32,
    cache: &mut std::collections::HashMap<(u32, u32), Option<String>>,
) -> Option<String> {
    cache
        .entry((row, col))
        .or_insert_with(|| {
            let f = ws.cell(row, col)?.formula.as_ref()?;
            if f.text.is_empty() {
                return None;
            }
            let opts = ParseOptions {
                style: RefStyle::A1,
                origin_row: row,
                origin_col: col,
            };
            let expr = parse_with(&f.text, &opts).ok()?;
            Some(format_with(
                &expr,
                &ParseOptions {
                    style: RefStyle::R1C1,
                    ..opts
                },
            ))
        })
        .clone()
}

/// The neighbours' formula for (`row`, `col`) when the formulas on both sides
/// (left and right, or above and below) agree and this one does not.
fn inconsistent(
    ws: &Worksheet,
    row: u32,
    col: u32,
    cache: &mut std::collections::HashMap<(u32, u32), Option<String>>,
) -> Option<String> {
    let mine = r1c1_of(ws, row, col, cache)?;
    let pairs = [
        (row.checked_sub(1).map(|r| (r, col)), Some((row + 1, col))),
        (col.checked_sub(1).map(|c| (row, c)), Some((row, col + 1))),
    ];
    for (a, b) in pairs {
        let (Some(a), Some(b)) = (a, b) else {
            continue;
        };
        let (Some(fa), Some(fb)) = (r1c1_of(ws, a.0, a.1, cache), r1c1_of(ws, b.0, b.1, cache))
        else {
            continue;
        };
        if fa == fb && fa != mine {
            let opts = ParseOptions {
                style: RefStyle::R1C1,
                origin_row: row,
                origin_col: col,
            };
            let expr = parse_with(&fa, &opts).ok()?;
            return Some(format_with(
                &expr,
                &ParseOptions {
                    style: RefStyle::A1,
                    ..opts
                },
            ));
        }
    }
    None
}

/// First single-cell precedent of a formula that is empty (A1 text).
fn empty_precedent(
    wb: &Workbook,
    engine: &bo_formula::Workbook,
    sheet: usize,
    row: u32,
    col: u32,
) -> Option<String> {
    engine
        .direct_precedents(sheet, row, col)
        .into_iter()
        .filter(bo_formula::Area::is_single)
        .find(|a| {
            wb.sheets.get(a.sheet).is_some_and(|ws| {
                ws.cell(a.r1, a.c1)
                    .is_none_or(|c| c.formula.is_none() && matches!(c.value, CellValue::Empty))
            })
        })
        .map(|a| {
            let cell = CellRef::new(a.r1, a.c1).to_string();
            if a.sheet == sheet {
                cell
            } else {
                let name = wb.sheets.get(a.sheet).map_or("", |s| s.name.as_str());
                format!("{}!{cell}", quote_sheet(name))
            }
        })
}

fn quote_sheet(name: &str) -> String {
    if name.chars().all(|c| c.is_alphanumeric() || c == '_') {
        name.to_owned()
    } else {
        format!("'{}'", name.replace('\'', "''"))
    }
}

fn is_ignored_errors(x: &RawXml) -> bool {
    element(&x.xml).is_some_and(|(n, _)| n.rsplit(':').next() == Some("ignoredErrors"))
}

/// Ranges and rules ignored on a sheet (§18.3.1.51 `ignoredErrors`).
pub fn ignored(ws: &Worksheet) -> Vec<(CellRange, Vec<ErrorRule>)> {
    let mut out = Vec::new();
    for x in ws.unknown.iter().filter(|x| is_ignored_errors(x)) {
        for chunk in x.xml.split('<').skip(1) {
            let name_end = chunk
                .find(|c: char| c.is_whitespace() || c == '/' || c == '>')
                .unwrap_or(chunk.len());
            if chunk[..name_end].rsplit(':').next() != Some("ignoredError") {
                continue;
            }
            let Some((_, attrs)) = element(&format!("<{chunk}")) else {
                continue;
            };
            let on = |a: &str| {
                attrs
                    .iter()
                    .any(|(k, v)| k == a && (v == "1" || v == "true"))
            };
            let rules: Vec<ErrorRule> = ErrorRule::ALL
                .into_iter()
                .filter(|r| on(r.attribute()))
                .collect();
            if rules.is_empty() {
                continue;
            }
            let sqref = attrs
                .iter()
                .find(|(k, _)| k == "sqref")
                .map(|(_, v)| v.as_str())
                .unwrap_or_default();
            for r in CellRange::parse_sqref(sqref) {
                out.push((r, rules.clone()));
            }
        }
    }
    out
}

/// Records that `rule` is ignored for one cell (§18.3.1.50 `ignoredError`).
pub fn ignore(ws: &mut Worksheet, row: u32, col: u32, rule: ErrorRule) {
    let item = format!(
        "<ignoredError sqref=\"{}\" {}=\"1\"/>",
        escape(&CellRef::new(row, col).to_string()),
        rule.attribute()
    );
    if let Some(x) = ws.unknown.iter_mut().find(|x| is_ignored_errors(x)) {
        if let Some(i) = x.xml.rfind("</") {
            x.xml.insert_str(i, &item);
        } else if let Some(i) = x.xml.rfind("/>") {
            // `<ignoredErrors/>` with no children.
            let open = x.xml[..i].trim_end().to_owned();
            let name = element(&x.xml).map_or_else(|| "ignoredErrors".to_owned(), |(n, _)| n);
            x.xml = format!("{open}>{item}</{name}>");
        }
        return;
    }
    ws.unknown.push(RawXml {
        after: "ignoredErrors".into(),
        xml: format!("<ignoredErrors>{item}</ignoredErrors>"),
    });
}

fn is_cell_watches(x: &RawXml) -> bool {
    element(&x.xml).is_some_and(|(n, _)| n.rsplit(':').next() == Some("cellWatches"))
}

/// Cells of the watch window saved with a sheet (ECMA-376 Part 1, `cellWatches` /
/// `cellWatch/@r` of CT_Worksheet), in file order.
pub fn cell_watches(ws: &Worksheet) -> Vec<CellRef> {
    let mut out = Vec::new();
    for x in ws.unknown.iter().filter(|x| is_cell_watches(x)) {
        for chunk in x.xml.split('<').skip(1) {
            let name_end = chunk
                .find(|c: char| c.is_whitespace() || c == '/' || c == '>')
                .unwrap_or(chunk.len());
            if chunk[..name_end].rsplit(':').next() != Some("cellWatch") {
                continue;
            }
            let Some((_, attrs)) = element(&format!("<{chunk}")) else {
                continue;
            };
            if let Some(c) = attrs
                .iter()
                .find(|(k, _)| k == "r")
                .and_then(|(_, v)| CellRef::parse(v))
            {
                out.push(c);
            }
        }
    }
    out
}

/// Replaces the watched cells saved with a sheet (`cellWatches`; none: removed).
/// Returns whether anything changed.
pub fn set_cell_watches(ws: &mut Worksheet, cells: &[CellRef]) -> bool {
    if cell_watches(ws) == cells {
        return false;
    }
    ws.unknown.retain(|x| !is_cell_watches(x));
    if !cells.is_empty() {
        let mut items = String::new();
        for c in cells {
            items.push_str("<cellWatch r=\"");
            items.push_str(&escape(&c.to_string()));
            items.push_str("\"/>");
        }
        ws.unknown.push(RawXml {
            after: "cellWatches".into(),
            xml: format!("<cellWatches>{items}</cellWatches>"),
        });
    }
    true
}

/// Forgets every ignored error of a sheet.
pub fn reset_ignored(ws: &mut Worksheet) -> bool {
    let before = ws.unknown.len();
    ws.unknown.retain(|x| !is_ignored_errors(x));
    ws.unknown.len() != before
}

#[cfg(test)]
mod tests {
    use super::*;
    use bo_model::sheet::{Cell, CellProtection};

    fn num(s: &str) -> Option<f64> {
        s.trim().parse().ok()
    }

    #[test]
    fn findings_on_a_hand_built_sheet() {
        let mut wb = Workbook::new();
        // Unlocked style for E1.
        let mut xf = wb.styles.xf(0).cloned().unwrap_or_default();
        xf.protection = Some(CellProtection {
            locked: Some(false),
            hidden: None,
        });
        xf.apply_protection = Some(true);
        wb.styles.cell_xfs.push(xf);
        let unlocked = (wb.styles.cell_xfs.len() - 1) as u32;
        let ws = &mut wb.sheets[0];
        // A1:A3 numbers, B1:B3 = A*2 except B2 = A2*3 (inconsistent).
        for r in 0..3 {
            ws.set_cell(r, 0, Cell::new(CellValue::Number(f64::from(r + 1))));
        }
        ws.set_cell(0, 1, Cell::default().with_formula("A1*2"));
        ws.set_cell(1, 1, Cell::default().with_formula("A2*3"));
        ws.set_cell(2, 1, Cell::default().with_formula("A3*2"));
        // C1 = 1/0 (error), C2 = "42" as text, D1 reads empty Z9.
        ws.set_cell(0, 2, Cell::default().with_formula("1/0"));
        ws.set_cell(1, 2, Cell::new(CellValue::Text("42".into())));
        ws.set_cell(0, 3, Cell::default().with_formula("Z9+1"));
        let mut e1 = Cell::default().with_formula("A1+1");
        e1.style = unlocked;
        ws.set_cell(0, 4, e1);
        crate::live::LiveCalc::new().rebuild(&mut wb);
        let fw = crate::formula::load(&wb, None);
        let got: Vec<(u32, u32, ErrorRule)> =
            check(&wb, 0, &ErrorRules::default(), Some(&fw), &num)
                .into_iter()
                .map(|f| (f.row, f.col, f.rule))
                .collect();
        assert_eq!(
            got,
            vec![
                (0, 2, ErrorRule::EvalError),
                (0, 3, ErrorRule::EmptyRef),
                (0, 4, ErrorRule::UnlockedFormula),
                (1, 1, ErrorRule::InconsistentFormula),
                (1, 2, ErrorRule::NumberAsText),
            ]
        );
        let all = check(&wb, 0, &ErrorRules::default(), Some(&fw), &num);
        assert_eq!(all[1].empty.as_deref(), Some("Z9"));
        assert_eq!(all[3].suggestion.as_deref(), Some("A2*2"));
        assert_eq!(all[4].number, Some(42.0));
        // Turning a rule off, then ignoring one finding (saved as ignoredError).
        let rules = ErrorRules {
            unlocked_formula: false,
            ..ErrorRules::default()
        };
        assert_eq!(check(&wb, 0, &rules, Some(&fw), &num).len(), 4);
        ignore(&mut wb.sheets[0], 1, 2, ErrorRule::NumberAsText);
        ignore(&mut wb.sheets[0], 0, 2, ErrorRule::EvalError);
        assert_eq!(
            wb.sheets[0].unknown[0].xml,
            "<ignoredErrors><ignoredError sqref=\"C2\" numberStoredAsText=\"1\"/>\
             <ignoredError sqref=\"C1\" evalError=\"1\"/></ignoredErrors>"
        );
        let left: Vec<ErrorRule> = check(&wb, 0, &rules, Some(&fw), &num)
            .into_iter()
            .map(|f| f.rule)
            .collect();
        assert_eq!(
            left,
            vec![ErrorRule::EmptyRef, ErrorRule::InconsistentFormula]
        );
        assert!(reset_ignored(&mut wb.sheets[0]));
        assert_eq!(check(&wb, 0, &rules, Some(&fw), &num).len(), 4);
    }

    #[test]
    fn ignored_errors_from_a_file_are_read() {
        let mut ws = Worksheet::new("S");
        ws.unknown.push(RawXml {
            after: "ignoredErrors".into(),
            xml: r#"<ignoredErrors><ignoredError sqref="A1:C5 E1" numberStoredAsText="1" formula="1"/></ignoredErrors>"#.into(),
        });
        let got = ignored(&ws);
        assert_eq!(got.len(), 2);
        assert_eq!(
            got[0].1,
            vec![ErrorRule::InconsistentFormula, ErrorRule::NumberAsText]
        );
        assert!(got[1].0.contains(CellRef::new(0, 4)));
    }
}
