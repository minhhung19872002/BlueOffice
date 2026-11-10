//! Show Changes of BlueSheet (CORE-VERSION-001): the open workbook compared
//! with another version of it ([`bo_sheet::diff`]). A query: nothing changes.

use super::SheetEngine;
use crate::{EngineError, Result};
use serde_json::{json, Value};

/// At most this many changes are listed (`truncated` tells the rest was cut).
pub const MAX_CHANGES: usize = 5_000;

/// `{"cmd":"compareWorkbook","name":"v3.xlsx","base64":"…"}` — the open
/// workbook is the new side, the given file the old one →
/// `{ changes: [SheetChange], total, truncated }`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompareCommand {
    /// File name of the other version (picks the reader by extension).
    pub name: String,
    /// Its bytes, base64.
    pub base64: String,
}

impl CompareCommand {
    /// Recognises the command.
    pub fn parse(v: &Value) -> Option<CompareCommand> {
        if v.get("cmd")?.as_str()? != "compareWorkbook" {
            return None;
        }
        Some(CompareCommand {
            name: v.get("name")?.as_str()?.to_owned(),
            base64: v.get("base64")?.as_str()?.to_owned(),
        })
    }
}

impl SheetEngine {
    /// Runs [`CompareCommand`].
    pub fn compare_workbook(&self, c: &CompareCommand) -> Result<Value> {
        let new = self.wb()?;
        let bytes = crate::merge::base64_decode(&c.base64)
            .ok_or_else(|| EngineError::Sheet("base64".into()))?;
        let mut other = SheetEngine::new();
        other.open(&bytes, &c.name)?;
        let old = other.wb()?;
        let changes = bo_sheet::diff::diff_workbooks(old, new);
        let total = changes.len();
        let shown: Vec<_> = changes.into_iter().take(MAX_CHANGES).collect();
        Ok(json!({ "changes": shown, "total": total, "truncated": total > MAX_CHANGES }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn b64(bytes: &[u8]) -> String {
        use base64::Engine as _;
        base64::engine::general_purpose::STANDARD.encode(bytes)
    }

    #[test]
    fn compares_the_open_workbook_with_another_version() {
        let mut e = SheetEngine::new();
        e.new_workbook("a.xlsx").expect("new");
        let put = |e: &mut SheetEngine, cells: &[(u32, u32, &str)]| {
            let v: Vec<(u32, u32, String)> = cells
                .iter()
                .map(|&(r, c, t)| (r, c, t.to_owned()))
                .collect();
            e.set_inputs(0, &v).expect("inputs");
        };
        put(
            &mut e,
            &[
                (0, 0, "Item"),
                (0, 1, "Qty"),
                (1, 0, "Pen"),
                (1, 1, "3"),
                (2, 0, "Ink"),
                (2, 1, "5"),
            ],
        );
        let old = e.save("xlsx", 0).expect("save");
        // Insert a row above "Ink" and change the quantity of "Pen".
        e.structural(0, true, 2, 1, false).expect("insert");
        put(&mut e, &[(2, 0, "Cap"), (2, 1, "4"), (1, 1, "6")]);
        let r: Value = serde_json::from_str(
            &e.exec_json(&format!(
                r#"{{"cmd":"compareWorkbook","name":"old.xlsx","base64":"{}"}}"#,
                b64(&old)
            ))
            .expect("compare"),
        )
        .expect("json");
        let kinds: Vec<(&str, &str)> = r["changes"]
            .as_array()
            .expect("changes")
            .iter()
            .map(|c| {
                (
                    c["kind"].as_str().unwrap_or(""),
                    c["address"].as_str().unwrap_or(""),
                )
            })
            .collect();
        assert_eq!(kinds, vec![("rowsInserted", "3"), ("valueChanged", "B2")]);
        assert_eq!(r["total"], 2);
        let bad = e.exec_json(r#"{"cmd":"compareWorkbook","name":"x.xlsx","base64":"bm9wZQ=="}"#);
        assert!(bad.is_err());
    }
}
