//! Chart design commands shared by BlueSheet and BlueSlide (CORE-CHART-007): chart styles,
//! colour variations, quick layouts and chart templates over the JSON bridge, the gallery data
//! of the Chart Design tab, and chart template packages (`.crtx`).
//!
//! A `.crtx` written here is an OPC package (ECMA-376 Part 2) whose main part, reached by the
//! package's officeDocument relationship, is a DrawingML chart part (Part 1, §21.2.2.29
//! `c:chartSpace`, content type `application/vnd.openxmlformats-officedocument.drawingml.chart+xml`)
//! holding the template's formatting on placeholder data. BlueOffice also stores the template as
//! JSON in a part of its own (relationship type [`REL_TEMPLATE_JSON`]); reading takes that part,
//! or else the formatting of the chart part, so chart template packages from elsewhere import too.

use bo_model::sheet::{Chart, ChartPlot, ChartSeries};
use bo_opc::{Limits, Package, Relationship, TargetMode};
use bo_sheet::chart::ColorFn;
use bo_sheet::chart_style::{self as cs, ChartTemplate, LayoutTexts};
use serde::Deserialize;
use serde_json::{json, Value};

/// Relationship type (package → JSON part) of BlueOffice's template description.
pub const REL_TEMPLATE_JSON: &str = "urn:bluestar:blueoffice:relationships:chart-template";
const CHART_PART: &str = "/chart/chart.xml";
const JSON_PART: &str = "/blueoffice/chartTemplate.json";

/// What a chart design command changes (absent = unchanged), applied in this order: template,
/// style, colours, layout.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct DesignSpec {
    /// Chart template to apply.
    pub template: Option<ChartTemplate>,
    /// Chart style id ([`cs::STYLES`]).
    pub style: Option<String>,
    /// Colour variation id ([`cs::PALETTES`]).
    pub colors: Option<String>,
    /// Quick layout id ([`cs::LAYOUTS`]).
    pub layout: Option<String>,
    /// Titles a layout or template adds (localised by the UI).
    pub texts: LayoutTexts,
    /// Options of the chart's elements (CORE-CHART-005: axes, gridlines, series, data labels,
    /// trend lines, error bars, …), applied last.
    pub elements: Option<Box<crate::chart_elements::ElementsPatch>>,
}

impl DesignSpec {
    /// Nothing to change.
    pub fn is_empty(&self) -> bool {
        self.template.is_none()
            && self.style.is_none()
            && self.colors.is_none()
            && self.layout.is_none()
            && self.elements.is_none()
    }
}

/// Applies `spec` to `c`; the error names the unknown preset.
pub fn apply(c: &mut Chart, spec: &DesignSpec, accents: &[u32; 6]) -> Result<(), String> {
    if let Some(t) = &spec.template {
        cs::apply_template(c, t, &spec.texts);
    }
    if let Some(id) = &spec.style {
        if !cs::apply_style(c, id) {
            return Err(format!("unknown chart style '{id}'"));
        }
    }
    if let Some(id) = &spec.colors {
        if !cs::apply_colors(c, id, accents) {
            return Err(format!("unknown chart colours '{id}'"));
        }
    }
    if let Some(id) = &spec.layout {
        if !cs::apply_layout(c, id, &spec.texts) {
            return Err(format!("unknown quick layout '{id}'"));
        }
    }
    if let Some(e) = &spec.elements {
        crate::chart_elements::apply(c, e)?;
    }
    c.dirty = true;
    Ok(())
}

/// Gallery of the Chart Design tab for chart `c`: a preview (SVG, drawn by the shared chart
/// renderer) per style and quick layout, the colours of each colour variation, and the chart's
/// current template.
pub fn gallery(c: &Chart, accents: &[u32; 6], w: f64, h: f64, color: &ColorFn<'_>) -> Value {
    let (w, h) = (w.clamp(40.0, 400.0), h.clamp(30.0, 300.0));
    let styles: Vec<Value> = cs::STYLES
        .iter()
        .map(|id| {
            let mut x = c.clone();
            cs::apply_style(&mut x, id);
            json!({ "id": id, "svg": cs::preview_svg(&x, w, h, color) })
        })
        .collect();
    let layouts: Vec<Value> = cs::LAYOUTS
        .iter()
        .map(|id| {
            let mut x = c.clone();
            cs::apply_layout(&mut x, id, &LayoutTexts::default());
            json!({ "id": id, "svg": cs::preview_svg(&x, w, h, color) })
        })
        .collect();
    let colors: Vec<Value> = cs::PALETTES
        .iter()
        .map(|id| {
            let v = cs::palette_colors(id, accents).unwrap_or_default();
            json!({
                "id": id,
                "colors": v.iter().map(|c| format!("#{c:06X}")).collect::<Vec<_>>(),
            })
        })
        .collect();
    json!({
        "styles": styles,
        "colors": colors,
        "layouts": layouts,
        "template": cs::save_template(c, ""),
    })
}

/// Placeholder data of the chart in a template package.
fn sample_chart() -> Chart {
    let cats: Vec<String> = ["1", "2", "3", "4"]
        .iter()
        .map(|s| (*s).to_owned())
        .collect();
    let series = |n: usize, v: [f64; 4]| ChartSeries {
        name: Some(format!("{n}")),
        categories: cats.clone(),
        values: v.iter().map(|x| Some(*x)).collect(),
        ..ChartSeries::default()
    };
    Chart {
        title: Some("1".into()),
        legend: Some("r".into()),
        plots: vec![ChartPlot {
            kind: bo_model::sheet::ChartKind::Bar,
            bar_dir: Some("col".into()),
            series: vec![
                series(1, [4.3, 2.5, 3.5, 4.5]),
                series(2, [2.4, 4.4, 1.8, 2.8]),
                series(3, [2.0, 2.0, 3.0, 5.0]),
            ],
            ..ChartPlot::default()
        }],
        ..Chart::default()
    }
}

/// The `.crtx` package of template `t`.
pub fn template_to_crtx(t: &ChartTemplate) -> crate::Result<Vec<u8>> {
    let mut c = sample_chart();
    cs::apply_template(&mut c, t, &LayoutTexts::default());
    let mut p = Package::new();
    let err = |e: bo_opc::OpcError| crate::EngineError::Sheet(e.to_string());
    p.add_part(
        CHART_PART,
        bo_xlsx::chart_write::CT_CHART,
        bo_xlsx::chart_write::chart_space_xml(&c).into_bytes(),
    )
    .map_err(err)?;
    p.add_part(
        JSON_PART,
        "application/json",
        serde_json::to_vec_pretty(t).unwrap_or_default(),
    )
    .map_err(err)?;
    p.set_relationships(
        "/",
        &[
            Relationship {
                id: "rId1".into(),
                rel_type: bo_opc::REL_OFFICE_DOCUMENT.into(),
                target: CHART_PART.trim_start_matches('/').into(),
                mode: TargetMode::Internal,
            },
            Relationship {
                id: "rId2".into(),
                rel_type: REL_TEMPLATE_JSON.into(),
                target: JSON_PART.trim_start_matches('/').into(),
                mode: TargetMode::Internal,
            },
        ],
    );
    p.write().map_err(err)
}

/// The template of a `.crtx` package (named `name` when the package does not name it); `None`
/// when the package holds no chart.
pub fn template_from_crtx(bytes: &[u8], name: &str) -> Option<ChartTemplate> {
    let p = Package::read(bytes, &Limits::default()).ok()?;
    if let Some(part) = p.related_part("/", REL_TEMPLATE_JSON).ok().flatten() {
        if let Some(mut t) = p
            .part(&part)
            .and_then(|d| serde_json::from_slice::<ChartTemplate>(&d.data).ok())
        {
            if t.name.is_empty() {
                name.clone_into(&mut t.name);
            }
            return Some(t);
        }
    }
    let main = p
        .related_part("/", bo_opc::REL_OFFICE_DOCUMENT)
        .ok()
        .flatten()
        .or_else(|| {
            let ct = p.content_types().ok()?;
            p.parts()
                .iter()
                .find(|x| ct.get(&x.name) == Some(bo_xlsx::chart_write::CT_CHART))
                .map(|x| x.name.clone())
        })?;
    let c = bo_xlsx::chart_write::parse_chart_part(&p.part(&main)?.data, &main)?;
    Some(cs::save_template(&c, name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crtx_round_trip() {
        let mut c = sample_chart();
        cs::apply_style(&mut c, "paper");
        cs::apply_colors(&mut c, "terrace", &cs::DEFAULT_ACCENTS);
        cs::apply_layout(&mut c, "layout3", &LayoutTexts::default());
        let t = cs::save_template(&c, "Giấy");
        let bytes = template_to_crtx(&t).unwrap();
        assert_eq!(template_from_crtx(&bytes, "x"), Some(t.clone()));
        // Without BlueOffice's JSON part the chart part's formatting is read.
        let mut p = Package::read(&bytes, &Limits::default()).unwrap();
        p.remove_part(JSON_PART);
        p.set_relationships(
            "/",
            &[Relationship {
                id: "rId1".into(),
                rel_type: bo_opc::REL_OFFICE_DOCUMENT.into(),
                target: "chart/chart.xml".into(),
                mode: TargetMode::Internal,
            }],
        );
        let other = template_from_crtx(&p.write().unwrap(), "Khác").unwrap();
        assert_eq!(other.name, "Khác");
        assert_eq!(other.kind, "column");
        assert_eq!(other.format.chart_fill, t.format.chart_fill);
        assert_eq!(other.format.palette, t.format.palette[..3].to_vec());
        assert!(other.x_title && other.y_title);
        assert!(template_from_crtx(b"not a zip", "x").is_none());
    }

    #[test]
    fn spec_errors() {
        let mut c = sample_chart();
        let bad = DesignSpec {
            style: Some("nope".into()),
            ..DesignSpec::default()
        };
        assert!(apply(&mut c, &bad, &cs::DEFAULT_ACCENTS).is_err());
        let g = gallery(
            &c,
            &cs::DEFAULT_ACCENTS,
            96.0,
            64.0,
            &bo_sheet::chart::default_color,
        );
        assert_eq!(g["styles"].as_array().map(Vec::len), Some(cs::STYLES.len()));
        assert_eq!(
            g["layouts"].as_array().map(Vec::len),
            Some(cs::LAYOUTS.len())
        );
        assert_eq!(g["colors"][0]["colors"][0], "#2E6DB4");
    }
}
