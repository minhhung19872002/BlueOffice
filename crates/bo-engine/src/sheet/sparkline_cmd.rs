//! Sparkline commands of BlueSheet (SHEET-SPARK-001, [MS-XLSX] `x14:sparklineGroup`): list the
//! groups of a sheet, insert, change settings, group / ungroup and clear. Every change is one
//! undo step through [`SheetEngine::edit`].

use super::cmd::{Recalc, Rng};
use super::SheetEngine;
use crate::{EngineError, Result};
use bo_model::sheet::{SparklineGroup, SparklineType};
use bo_render::Rgba;
use bo_sheet::sparkline::{self as sp, Palette, SparklinePatch};
use serde_json::{json, Value};

fn bad(msg: impl Into<String>) -> EngineError {
    EngineError::Sheet(msg.into())
}

fn hex(c: Rgba) -> String {
    format!("#{:02X}{:02X}{:02X}", c.0, c.1, c.2)
}

fn group_json(index: usize, g: &SparklineGroup, p: &Palette) -> Value {
    let mut v = serde_json::to_value(g).unwrap_or(Value::Null);
    if let Value::Object(o) = &mut v {
        o.remove("attrs");
        o.insert("index".into(), json!(index));
        o.insert(
            "colorsHex".into(),
            json!({
                "series": hex(p.series),
                "negative": hex(p.negative),
                "axis": hex(p.axis),
                "markers": hex(p.markers),
                "first": hex(p.first),
                "last": hex(p.last),
                "high": hex(p.high),
                "low": hex(p.low),
            }),
        );
        o.insert(
            "lineWeight".into(),
            json!(g.line_weight.unwrap_or(sp::DEFAULT_WEIGHT)),
        );
    }
    v
}

impl SheetEngine {
    /// The sparkline groups of `sheet`.
    pub(super) fn sparkline_list(&self, sheet: usize) -> Result<Value> {
        let wb = self.wb()?;
        let ws = self.sheet(sheet)?;
        let list: Vec<Value> = ws
            .sparkline_groups
            .iter()
            .enumerate()
            .map(|(i, g)| group_json(i, g, &Palette::of(&g.colors, wb.theme.as_ref())))
            .collect();
        Ok(Value::Array(list))
    }

    /// Inserts a group of `kind` sparklines over `data` into `location`.
    pub(super) fn sparkline_insert(
        &mut self,
        sheet: usize,
        kind: SparklineType,
        data: &str,
        location: Rng,
    ) -> Result<Value> {
        self.edit(sheet, Recalc::No, |wb, _| {
            let i = sp::insert(wb, sheet, kind, data, location.range()).map_err(bad)?;
            Ok(json!({ "index": i }))
        })
    }

    /// Changes the settings of group `group`.
    pub(super) fn sparkline_edit(
        &mut self,
        sheet: usize,
        group: usize,
        patch: &SparklinePatch,
    ) -> Result<Value> {
        if patch == &SparklinePatch::default() {
            return Err(bad("nothing to change"));
        }
        self.edit(sheet, Recalc::No, |wb, _| {
            sp::edit(wb, sheet, group, patch).map_err(bad)?;
            Ok(json!({ "ok": true }))
        })
    }

    /// Groups the sparklines in `range`.
    pub(super) fn sparkline_group(&mut self, sheet: usize, range: Rng) -> Result<Value> {
        self.edit(sheet, Recalc::No, |wb, _| {
            let i = sp::group(wb, sheet, range.range()).map_err(bad)?;
            Ok(json!({ "index": i }))
        })
    }

    /// Ungroups the groups with a sparkline in `range`.
    pub(super) fn sparkline_ungroup(&mut self, sheet: usize, range: Rng) -> Result<Value> {
        self.edit(sheet, Recalc::No, |wb, _| {
            let n = sp::ungroup(wb, sheet, range.range()).map_err(bad)?;
            Ok(json!({ "groups": n }))
        })
    }

    /// Clears the sparklines (or their whole groups) in `range`.
    pub(super) fn sparkline_clear(
        &mut self,
        sheet: usize,
        range: Rng,
        whole_groups: bool,
    ) -> Result<Value> {
        self.edit(sheet, Recalc::No, |wb, _| {
            let n = sp::clear(wb, sheet, range.range(), whole_groups).map_err(bad)?;
            if n == 0 {
                return Err(bad("no sparklines in the selection"));
            }
            Ok(json!({ "removed": n }))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::super::SheetEngine;
    use serde_json::Value;

    fn run(e: &mut SheetEngine, cmd: &str) -> Value {
        serde_json::from_str(&e.exec_json(cmd).unwrap()).unwrap()
    }

    #[test]
    fn insert_edit_undo_and_save() {
        let mut e = SheetEngine::new();
        e.new_workbook("Book").unwrap();
        for (c, v) in ["1", "3", "2"].iter().enumerate() {
            e.set_input(0, 0, c as u32, v).unwrap();
        }
        let r = run(
            &mut e,
            r#"{"cmd":"sparklineInsert","sheet":0,"type":"line","data":"A1:C1","location":{"r0":0,"c0":3,"r1":0,"c1":3}}"#,
        );
        assert_eq!(r["index"], 0);
        let list = run(&mut e, r#"{"cmd":"sparklines","sheet":0}"#);
        assert_eq!(list[0]["type"], "line");
        assert_eq!(list[0]["sparklines"][0]["formula"], "Sheet1!A1:C1");
        assert_eq!(list[0]["colorsHex"]["series"], "#2E6DB4");
        run(
            &mut e,
            r##"{"cmd":"sparklineEdit","sheet":0,"group":0,"type":"column","high":true,"colorHigh":"#112233"}"##,
        );
        let list = run(&mut e, r#"{"cmd":"sparklines","sheet":0}"#);
        assert_eq!(list[0]["type"], "column");
        assert_eq!(list[0]["high"], true);
        assert_eq!(list[0]["colorsHex"]["high"], "#112233");
        // Saved and read back.
        let bytes = e.save("xlsx", 0).unwrap();
        let wb = bo_xlsx::read_xlsx(&bytes).unwrap();
        assert_eq!(wb.sheets[0].sparkline_groups.len(), 1);
        assert!(wb.sheets[0].sparkline_groups[0].high);
        // Undo the edit, then the insert.
        e.undo().unwrap();
        let list = run(&mut e, r#"{"cmd":"sparklines","sheet":0}"#);
        assert_eq!(list[0]["type"], "line");
        e.undo().unwrap();
        assert_eq!(
            run(&mut e, r#"{"cmd":"sparklines","sheet":0}"#),
            Value::Array(vec![])
        );
        // Bad input is refused without an undo step.
        assert!(e
            .exec_json(r#"{"cmd":"sparklineInsert","sheet":0,"type":"stacked","data":"nope","location":{"r0":0,"c0":3,"r1":0,"c1":3}}"#)
            .is_err());
    }
}
