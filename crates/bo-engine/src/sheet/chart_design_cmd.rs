//! Chart design commands of BlueSheet (CORE-CHART-007): gallery, style / colours / quick layout /
//! template, `.crtx` export and import. Edits go through [`SheetEngine::edit`] (undoable).

use super::cmd::Recalc;
use super::SheetEngine;
use crate::chart_design::{self, DesignSpec};
use crate::{EngineError, Result};
use base64::Engine as _;
use bo_sheet::chart_style::{self as cs, ChartTemplate};
use serde_json::{json, Value};

fn bad(msg: impl Into<String>) -> EngineError {
    EngineError::Sheet(msg.into())
}

impl SheetEngine {
    /// Gallery data of chart `index` on `sheet` (previews `w × h` points).
    pub(super) fn chart_gallery(
        &self,
        sheet: usize,
        index: usize,
        w: f64,
        h: f64,
    ) -> Result<Value> {
        let wb = self.wb()?;
        let c = self
            .sheet(sheet)?
            .charts
            .get(index)
            .ok_or_else(|| bad("no such chart"))?;
        let live = self.live_chart(sheet, c);
        Ok(chart_design::gallery(
            &live,
            &cs::theme_accents(wb),
            w,
            h,
            &bo_sheet::chart::default_color,
        ))
    }

    /// Chart `c` of `sheet` with the live data of its references (names, categories, values), as
    /// drawn on the sheet.
    pub(super) fn live_chart(
        &self,
        sheet: usize,
        c: &bo_model::sheet::Chart,
    ) -> bo_model::sheet::Chart {
        let mut live = c.clone();
        let Ok(wb) = self.wb() else { return live };
        for (p, src) in live.plots.iter_mut().zip(&c.plots) {
            for (s, from) in p.series.iter_mut().zip(&src.series) {
                let (name, cats, vals) =
                    bo_sheet::chart::series_data(wb, sheet, from, &self.locale);
                if name.is_some() {
                    s.name = name;
                }
                if !vals.is_empty() {
                    s.categories = cats;
                    s.values = vals;
                }
            }
        }
        live
    }

    /// Applies a chart design `spec` to chart `index` on `sheet`.
    pub(super) fn chart_design(
        &mut self,
        sheet: usize,
        index: usize,
        spec: &DesignSpec,
    ) -> Result<Value> {
        if spec.is_empty() {
            return Err(bad("nothing to change"));
        }
        let accents = cs::theme_accents(self.wb()?);
        self.edit(sheet, Recalc::No, |wb, _| {
            let c = wb.sheets[sheet]
                .charts
                .get_mut(index)
                .ok_or_else(|| bad("no such chart"))?;
            chart_design::apply(c, spec, &accents).map_err(bad)?;
            Ok(json!({ "ok": true }))
        })
    }
}

/// `.crtx` bytes (base64) of template `t`.
pub(super) fn template_export(t: &ChartTemplate) -> Result<Value> {
    let bytes = chart_design::template_to_crtx(t)?;
    Ok(json!({
        "base64": base64::engine::general_purpose::STANDARD.encode(bytes),
        "name": format!("{}.crtx", if t.name.is_empty() { "chart" } else { &t.name }),
    }))
}

/// The template of a `.crtx` file (base64).
pub(super) fn template_import(b64: &str, name: &str) -> Result<Value> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(b64.trim())
        .map_err(|_| bad("not a chart template"))?;
    let name = name.trim().trim_end_matches(".crtx");
    let t = chart_design::template_from_crtx(&bytes, name)
        .ok_or_else(|| bad("not a chart template"))?;
    Ok(serde_json::to_value(t).unwrap_or(Value::Null))
}
