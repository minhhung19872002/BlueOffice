//! Defined names of BlueSheet (ECMA-376 Part 1, §18.2.5 `definedName`): the Name
//! Manager list, New / Edit / Delete name, and Create from Selection (names taken from
//! the labels in the top row / left column / bottom row / right column of a range).
//! Each change is one undo step; formulas using the names are recalculated.
//!
//! * `nameList` → `[{ name, refersTo, scope, comment }]` (`scope`: sheet index or null);
//! * `defineName { name, refersTo, scope?, comment?, old? }` — adds a name, or replaces
//!   `old` (`{ name, scope }`, the name being edited);
//! * `deleteName { name, scope? }`;
//! * `createNames { sheet, range, top, left, bottom, right }` → `{ created: [names] }`.

use super::cmd::{Recalc, Rng};
use super::SheetEngine;
use crate::{EngineError, Result};
use bo_model::sheet::{CellRange, CellRef, DefinedName, Workbook};
use serde::Deserialize;
use serde_json::{json, Value};

fn bad(msg: impl Into<String>) -> EngineError {
    EngineError::Sheet(msg.into())
}

/// A name and its scope (`None`: the workbook).
#[derive(Debug, Clone, Deserialize)]
pub struct NameKey {
    name: String,
    #[serde(default)]
    scope: Option<usize>,
}

/// Defined-name commands.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "cmd", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum NameCommand {
    /// The names shown to the user.
    NameList,
    /// Add a name, or replace `old`.
    DefineName {
        name: String,
        /// Formula text the name stands for, `=` optional (`Sheet1!$A$1:$A$5`, `0,1`).
        refers_to: String,
        #[serde(default)]
        scope: Option<usize>,
        #[serde(default)]
        comment: Option<String>,
        #[serde(default)]
        old: Option<NameKey>,
    },
    /// Remove a name.
    DeleteName {
        name: String,
        #[serde(default)]
        scope: Option<usize>,
    },
    /// Names from the labels around a range.
    CreateNames {
        sheet: usize,
        range: Rng,
        #[serde(default)]
        top: bool,
        #[serde(default)]
        left: bool,
        #[serde(default)]
        bottom: bool,
        #[serde(default)]
        right: bool,
    },
}

impl NameCommand {
    /// Parses a JSON command when it is a defined-name command.
    pub fn parse(v: &Value) -> Option<Result<NameCommand>> {
        if !matches!(
            v.get("cmd")?.as_str()?,
            "nameList" | "defineName" | "deleteName" | "createNames"
        ) {
            return None;
        }
        Some(serde_json::from_value(v.clone()).map_err(|e| bad(format!("bad command: {e}"))))
    }
}

/// Whether `s` reads as a cell reference (`A1`, `xfd1048576`, `R1C1`, `R`, `C`), which a
/// name may not be.
fn is_reference(s: &str) -> bool {
    let up = s.to_ascii_uppercase();
    if up == "R" || up == "C" {
        return true;
    }
    let letters = up.chars().take_while(char::is_ascii_alphabetic).count();
    let rest = &up[letters..];
    if (1..=3).contains(&letters)
        && !rest.is_empty()
        && rest.chars().all(|c| c.is_ascii_digit())
        && bo_model::sheet::letters_to_col(&up[..letters]).is_some_and(|c| c < 16_384)
    {
        return true;
    }
    // R1C1 forms: R, R1, R1C, R1C1, RC1 …
    let b = up.as_bytes();
    if b.first() == Some(&b'R') {
        let mut i = 1;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        if i == b.len() {
            return true;
        }
        if b[i] == b'C' && b[i + 1..].iter().all(u8::is_ascii_digit) {
            return true;
        }
    }
    false
}

/// A valid defined name (§18.2.5: starts with a letter, `_` or `\`; letters, digits,
/// `_`, `.`, `\`, `?` after; not a cell reference; at most 255 characters).
pub fn valid_name(s: &str) -> bool {
    let mut chars = s.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    s.chars().count() <= 255
        && (first.is_alphabetic() || first == '_' || first == '\\')
        && chars.all(|c| c.is_alphanumeric() || matches!(c, '_' | '.' | '\\' | '?'))
        && !is_reference(s)
        && !s.eq_ignore_ascii_case("TRUE")
        && !s.eq_ignore_ascii_case("FALSE")
}

/// A label turned into a name: spaces and other characters become `_`, a name that
/// would start with a digit or read as a reference gets a leading `_`.
fn name_from_label(label: &str) -> Option<String> {
    let t = label.trim();
    if t.is_empty() {
        return None;
    }
    let mut s: String = t
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || matches!(c, '_' | '.' | '\\' | '?') {
                c
            } else {
                '_'
            }
        })
        .collect();
    if !s.starts_with(|c: char| c.is_alphabetic() || c == '_' || c == '\\') || is_reference(&s) {
        s.insert(0, '_');
    }
    valid_name(&s).then_some(s)
}

fn scope_id(scope: Option<usize>) -> Option<u32> {
    scope.and_then(|s| u32::try_from(s).ok())
}

fn find(wb: &Workbook, name: &str, scope: Option<usize>) -> Option<usize> {
    let id = scope_id(scope);
    wb.defined_names
        .iter()
        .position(|d| d.local_sheet == id && d.name.eq_ignore_ascii_case(name))
}

fn comment_of(d: &DefinedName) -> Option<&str> {
    d.attrs
        .iter()
        .find(|(k, _)| k == "comment")
        .map(|(_, v)| v.as_str())
}

/// Sets `name` (scope `scope`) to `formula`, replacing a name of the same scope.
fn put(
    wb: &mut Workbook,
    name: &str,
    scope: Option<usize>,
    formula: String,
    comment: Option<&str>,
) {
    let mut attrs: Vec<(String, String)> = Vec::new();
    if let Some(i) = find(wb, name, scope) {
        attrs = wb.defined_names.remove(i).attrs;
    }
    attrs.retain(|(k, _)| k != "comment");
    if let Some(c) = comment.filter(|c| !c.is_empty()) {
        attrs.push(("comment".into(), c.to_owned()));
    }
    wb.defined_names.push(DefinedName {
        name: name.to_owned(),
        formula,
        local_sheet: scope_id(scope),
        hidden: false,
        attrs,
    });
}

impl SheetEngine {
    /// Runs a defined-name command.
    pub(super) fn name_command(&mut self, cmd: NameCommand) -> Result<Value> {
        match cmd {
            NameCommand::NameList => {
                let wb = self.wb()?;
                let mut list: Vec<Value> = wb
                    .defined_names
                    .iter()
                    .filter(|d| !d.hidden && !d.name.starts_with("_xlnm."))
                    .map(|d| {
                        json!({
                            "name": d.name,
                            "refersTo": format!("={}", d.formula),
                            "scope": d.local_sheet,
                            "comment": comment_of(d),
                        })
                    })
                    .collect();
                list.sort_by_key(|v| v["name"].as_str().unwrap_or_default().to_lowercase());
                Ok(Value::Array(list))
            }
            NameCommand::DefineName {
                name,
                refers_to,
                scope,
                comment,
                old,
            } => {
                self.writable()?;
                let name = name.trim().to_owned();
                if !valid_name(&name) {
                    return Err(bad("invalid name"));
                }
                let n = self.wb()?.sheets.len();
                if scope.is_some_and(|s| s >= n) {
                    return Err(EngineError::PageOutOfRange(scope.unwrap_or_default()));
                }
                let body = refers_to.trim().trim_start_matches('=').trim().to_owned();
                let locale = self.locale;
                let formula = bo_formula::localized_input(&body, locale.decimal).unwrap_or(body);
                if formula.is_empty() || bo_formula::parse(&formula).is_err() {
                    return Err(bad("invalid formula"));
                }
                let same = old
                    .as_ref()
                    .is_some_and(|o| o.name.eq_ignore_ascii_case(&name) && o.scope == scope);
                if !same && find(self.wb()?, &name, scope).is_some() {
                    return Err(bad("name exists"));
                }
                self.edit(0, Recalc::All, |wb, _| {
                    if let Some(o) = &old {
                        if let Some(i) = find(wb, &o.name, o.scope) {
                            let prev = wb.defined_names.remove(i);
                            if comment.is_none() {
                                put(wb, &name, scope, formula, comment_of(&prev));
                                return Ok(json!({ "name": name }));
                            }
                        }
                    }
                    put(wb, &name, scope, formula, comment.as_deref());
                    Ok(json!({ "name": name }))
                })
            }
            NameCommand::DeleteName { name, scope } => {
                self.writable()?;
                if find(self.wb()?, &name, scope).is_none() {
                    return Ok(json!({ "removed": false }));
                }
                self.edit(0, Recalc::All, |wb, _| {
                    if let Some(i) = find(wb, &name, scope) {
                        wb.defined_names.remove(i);
                    }
                    Ok(json!({ "removed": true }))
                })
            }
            NameCommand::CreateNames {
                sheet,
                range,
                top,
                left,
                bottom,
                right,
            } => {
                self.writable()?;
                let created =
                    self.names_from_labels(sheet, range.range(), [top, left, bottom, right])?;
                if created.is_empty() {
                    return Ok(json!({ "created": [] }));
                }
                let names: Vec<String> = created.iter().map(|(n, _)| n.clone()).collect();
                self.edit(sheet, Recalc::All, |wb, _| {
                    for (name, formula) in created {
                        put(wb, &name, None, formula, None);
                    }
                    Ok(json!({ "created": names }))
                })
            }
        }
    }

    /// The names Create from Selection makes: `[top, left, bottom, right]` label sides.
    fn names_from_labels(
        &self,
        sheet: usize,
        r: CellRange,
        [top, left, bottom, right]: [bool; 4],
    ) -> Result<Vec<(String, String)>> {
        let wb = self.wb()?;
        let ws = self.sheet(sheet)?;
        let locale = self.locale;
        let label = |row: u32, col: u32| {
            ws.cell(row, col)
                .map(|c| bo_sheet::render::format_cell(wb, c, &locale).text)
                .unwrap_or_default()
        };
        // The cells the names cover: without the label rows / columns.
        let r0 = r.first.row + u32::from(top);
        let r1 = r.last.row.saturating_sub(u32::from(bottom));
        let c0 = r.first.col + u32::from(left);
        let c1 = r.last.col.saturating_sub(u32::from(right));
        if r0 > r1 || c0 > c1 {
            return Ok(Vec::new());
        }
        let mut out: Vec<(String, String)> = Vec::new();
        let mut add = |text: String, cells: CellRange| {
            if let Some(n) = name_from_label(&text) {
                let f = bo_sheet::cells::abs_ref(&ws.name, cells);
                out.retain(|(x, _)| !x.eq_ignore_ascii_case(&n));
                out.push((n, f));
            }
        };
        let cols = |c: u32| CellRange::new(CellRef::new(r0, c), CellRef::new(r1, c));
        let rows = |row: u32| CellRange::new(CellRef::new(row, c0), CellRef::new(row, c1));
        for c in c0..=c1.min(c0 + 16_383) {
            if top {
                add(label(r.first.row, c), cols(c));
            }
            if bottom {
                add(label(r.last.row, c), cols(c));
            }
        }
        for row in r0..=r1.min(r0 + 100_000) {
            if left {
                add(label(row, r.first.col), rows(row));
            }
            if right {
                add(label(row, r.last.col), rows(row));
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::{name_from_label, valid_name};
    use crate::sheet::SheetEngine;
    use bo_model::sheet::Workbook;
    use serde_json::{json, Value};

    fn run(e: &mut SheetEngine, v: &Value) -> Value {
        serde_json::from_str(&e.exec_json(&v.to_string()).unwrap()).unwrap()
    }

    #[test]
    fn define_use_edit_delete_and_create_names() {
        let mut e = SheetEngine::new();
        e.load(Workbook::new(), "t.xlsx").unwrap();
        // A1:B4 = Mặt hàng | Đơn giá / Bút 5 / Vở 12 / Thước 3.
        for (r, c, v) in [
            (0, 0, "Mặt hàng"),
            (0, 1, "Đơn giá"),
            (1, 0, "Bút"),
            (1, 1, "5"),
            (2, 0, "Vở"),
            (2, 1, "12"),
            (3, 0, "Thước"),
            (3, 1, "3"),
        ] {
            e.set_input(0, r, c, v).unwrap();
        }
        // A name typed with a vi-VN decimal comma, used by a formula.
        run(
            &mut e,
            &json!({ "cmd": "defineName", "name": "Thuế", "refersTo": "=0,1" }),
        );
        e.set_input(0, 5, 0, "=SUM(B2:B4)*(1+Thuế)").unwrap();
        // (5 + 12 + 3) × 1.1 = 22
        assert_eq!(e.cell(0, 5, 0).unwrap().display, "22");
        // Create from selection, top row: Đơn_giá = $B$2:$B$4.
        let made = run(
            &mut e,
            &json!({ "cmd": "createNames", "sheet": 0, "range": { "r0": 0, "c0": 0, "r1": 3, "c1": 1 }, "top": true }),
        );
        assert_eq!(made["created"], json!(["Mặt_hàng", "Đơn_giá"]));
        e.set_input(0, 6, 0, "=MAX(Đơn_giá)").unwrap();
        assert_eq!(e.cell(0, 6, 0).unwrap().display, "12");
        let list = run(&mut e, &json!({ "cmd": "nameList" }));
        let price = list
            .as_array()
            .unwrap()
            .iter()
            .find(|n| n["name"] == json!("Đơn_giá"))
            .unwrap();
        assert_eq!(price["refersTo"], json!("=Sheet1!$B$2:$B$4"));
        assert_eq!(price["scope"], Value::Null);
        // Edit: 0,1 → 0,2 keeps the name; the total follows (20 × 1.2 = 24).
        run(
            &mut e,
            &json!({ "cmd": "defineName", "name": "Thuế", "refersTo": "=0,2", "old": { "name": "Thuế" } }),
        );
        assert_eq!(e.cell(0, 5, 0).unwrap().display, "24");
        // Bad names and duplicates are refused.
        for bad in [
            json!({ "cmd": "defineName", "name": "A1", "refersTo": "=1" }),
            json!({ "cmd": "defineName", "name": "Thuế", "refersTo": "=1" }),
            json!({ "cmd": "defineName", "name": "x", "refersTo": "=1+" }),
        ] {
            assert!(e.exec_json(&bad.to_string()).is_err(), "{bad}");
        }
        // One undo step per change: undo the edit → 22 again.
        assert!(e.undo().unwrap());
        assert_eq!(e.cell(0, 5, 0).unwrap().display, "22");
        run(&mut e, &json!({ "cmd": "deleteName", "name": "Thuế" }));
        assert_eq!(e.cell(0, 5, 0).unwrap().display, "#NAME?");
    }

    #[test]
    fn names_follow_the_rules() {
        assert!(valid_name("Thuế_suất"));
        assert!(valid_name("_x"));
        assert!(valid_name("Doanh.thu"));
        assert!(!valid_name("A1"));
        assert!(!valid_name("xfd100"));
        assert!(!valid_name("R1C1"));
        assert!(!valid_name("r"));
        assert!(!valid_name("1abc"));
        assert!(!valid_name("có dấu cách"));
        assert!(!valid_name(""));
        // Four letters and a number is past the last column: a name.
        assert!(valid_name("ABCD1"));
    }

    #[test]
    fn labels_become_names() {
        assert_eq!(name_from_label("Đơn giá").as_deref(), Some("Đơn_giá"));
        assert_eq!(name_from_label("2025").as_deref(), Some("_2025"));
        assert_eq!(name_from_label("Q1").as_deref(), Some("_Q1"));
        assert_eq!(name_from_label("  "), None);
    }
}
