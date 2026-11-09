//! Chart styles, colour variations, quick layouts and chart templates (CORE-CHART-007) through
//! the JSON commands of BlueSheet and BlueSlide: apply, undo, save, reopen.
//!
//! With `BO_CHART_OUT=<dir>` the written files are kept there (for OpenXmlValidator:
//! `bo-cli validate <dir> --openxml require`).
// Tests may unwrap: a panic is a test failure.
#![allow(clippy::unwrap_used)]

use bo_engine::sheet::SheetEngine;
use bo_engine::slide_edit::{SlideCommand, SlideEditResult};
use bo_engine::Engine;
use bo_model::chart_style::SeriesFill;
use bo_model::sheet::ChartColor;
use serde_json::{json, Value};
use std::path::PathBuf;

fn keep(name: &str, bytes: &[u8]) {
    if let Ok(dir) = std::env::var("BO_CHART_OUT") {
        let _ = std::fs::create_dir_all(&dir);
        std::fs::write(PathBuf::from(dir).join(name), bytes).unwrap();
    }
}

fn exec(e: &mut SheetEngine, v: &Value) -> Value {
    serde_json::from_str(&e.exec_json(&v.to_string()).unwrap()).unwrap()
}

fn sheet_with_chart() -> SheetEngine {
    let mut e = SheetEngine::with_bundled_fonts();
    e.new_workbook("Book1.xlsx").unwrap();
    let data = [
        ["Tỉnh", "2024", "2025"],
        ["HN", "10", "12"],
        ["HUE", "5", "4"],
        ["DN", "7", "9"],
    ];
    for (r, row) in data.iter().enumerate() {
        for (c, v) in row.iter().enumerate() {
            e.set_input(0, r as u32, c as u32, v).unwrap();
        }
    }
    exec(
        &mut e,
        &json!({ "cmd": "insertChart", "sheet": 0,
            "range": { "r0": 0, "c0": 0, "r1": 3, "c1": 2 }, "kind": "column" }),
    );
    e
}

fn reopen(e: &mut SheetEngine, name: &str) -> SheetEngine {
    let bytes = e.save("xlsx", 0).unwrap();
    keep(name, &bytes);
    let mut back = SheetEngine::with_bundled_fonts();
    back.open(&bytes, name).unwrap();
    back
}

#[test]
fn sheet_chart_style_colours_layout_round_trip() {
    let mut e = sheet_with_chart();
    let g = exec(
        &mut e,
        &json!({ "cmd": "chartGallery", "sheet": 0, "index": 0, "width": 120, "height": 80 }),
    );
    assert_eq!(g["styles"].as_array().unwrap().len(), 12);
    assert_eq!(g["colors"].as_array().unwrap().len(), 8);
    assert_eq!(g["layouts"].as_array().unwrap().len(), 6);
    assert!(g["styles"][0]["svg"].as_str().unwrap().starts_with("<svg"));
    exec(
        &mut e,
        &json!({ "cmd": "chartDesign", "sheet": 0, "index": 0, "style": "gradient",
            "colors": "lotus", "layout": "layout3",
            "texts": { "xTitle": "Tỉnh", "yTitle": "Tỷ đồng" } }),
    );
    let c = e.workbook().unwrap().sheets[0].charts[0].clone();
    assert_eq!(c.format.series_fill, SeriesFill::Gradient);
    assert_eq!(c.format.palette[0], ChartColor::Rgb(0xB2_3A6F));
    assert_eq!(c.x_title.as_deref(), Some("Tỉnh"));
    // Undo restores the chart as it was.
    assert!(e.undo().unwrap());
    let undone = &e.workbook().unwrap().sheets[0].charts[0];
    assert_eq!(undone.format.series_fill, SeriesFill::Solid);
    assert_eq!(undone.x_title, None);
    assert!(e.redo().unwrap());
    let back = reopen(&mut e, "chart-design-sheet.xlsx");
    let r = &back.workbook().unwrap().sheets[0].charts[0];
    assert_eq!(r.format.series_fill, SeriesFill::Gradient);
    assert_eq!(r.format.grid_color, c.format.grid_color);
    assert!(r.format.title_bold);
    assert_eq!(r.x_title.as_deref(), Some("Tỉnh"));
    assert_eq!(r.legend.as_deref(), Some("b"));
    // The palette is written on the series.
    assert_eq!(r.plots[0].series[1].fill, Some(ChartColor::Rgb(0xE9_849B)));
    let bad = json!({ "cmd": "chartDesign", "sheet": 0, "index": 0, "style": "nope" });
    assert!(e.exec_json(&bad.to_string()).is_err());
    let none = json!({ "cmd": "chartGallery", "sheet": 0, "index": 3 });
    assert!(e.exec_json(&none.to_string()).is_err());
}

#[test]
fn chart_style_parts_are_kept() {
    // [MS-ODRAWXML] chartStyle / colors parts related from the chart part stay in the package
    // when a BlueOffice style is applied (the explicit spPr takes precedence).
    let mut e = sheet_with_chart();
    let bytes = e.save("xlsx", 0).unwrap();
    let mut p = bo_opc::Package::read(&bytes, &bo_opc::Limits::default()).unwrap();
    let chart = p
        .parts()
        .iter()
        .find(|x| x.name.starts_with("/xl/charts/chart"))
        .unwrap()
        .name
        .clone();
    let style = "/xl/charts/style1.xml";
    p.add_part(
        style,
        "application/vnd.ms-office.chartstyle+xml",
        br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><cs:chartStyle xmlns:cs="http://schemas.microsoft.com/office/drawing/2012/chartStyle" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" id="201"/>"#.to_vec(),
    )
    .unwrap();
    let mut rels = p.relationships(&chart).unwrap();
    rels.push(bo_opc::Relationship {
        id: "rIdStyle".into(),
        rel_type: "http://schemas.microsoft.com/office/2011/relationships/chartStyle".into(),
        target: "style1.xml".into(),
        mode: bo_opc::TargetMode::Internal,
    });
    p.set_relationships(&chart, &rels);
    let mut e = SheetEngine::with_bundled_fonts();
    e.open(&p.write().unwrap(), "styled.xlsx").unwrap();
    exec(
        &mut e,
        &json!({ "cmd": "chartDesign", "sheet": 0, "index": 0, "style": "soft" }),
    );
    let out = e.save("xlsx", 0).unwrap();
    let q = bo_opc::Package::read(&out, &bo_opc::Limits::default()).unwrap();
    assert!(q.part(style).is_some());
    assert!(q
        .relationships(&chart)
        .unwrap()
        .iter()
        .any(|r| r.id == "rIdStyle"));
    // (Not kept for the validator: the injected style part is a stub.)
    let mut back = SheetEngine::with_bundled_fonts();
    back.open(&out, "back.xlsx").unwrap();
    assert_eq!(
        back.workbook().unwrap().sheets[0].charts[0]
            .format
            .series_fill,
        SeriesFill::Soft
    );
}

#[test]
fn sheet_template_save_apply_and_crtx() {
    let mut e = sheet_with_chart();
    exec(
        &mut e,
        &json!({ "cmd": "chartDesign", "sheet": 0, "index": 0, "style": "night",
            "colors": "terrace", "layout": "layout2" }),
    );
    exec(
        &mut e,
        &json!({ "cmd": "editChart", "sheet": 0, "index": 0, "kind": "bar" }),
    );
    let g = exec(
        &mut e,
        &json!({ "cmd": "chartGallery", "sheet": 0, "index": 0 }),
    );
    let mut template = g["template"].clone();
    template["name"] = json!("Đêm");
    // A second chart takes the template's type and formatting.
    exec(
        &mut e,
        &json!({ "cmd": "insertChart", "sheet": 0,
            "range": { "r0": 0, "c0": 0, "r1": 3, "c1": 1 }, "kind": "line" }),
    );
    exec(
        &mut e,
        &json!({ "cmd": "chartDesign", "sheet": 0, "index": 1, "template": template }),
    );
    // And a chart inserted with the template.
    exec(
        &mut e,
        &json!({ "cmd": "insertChart", "sheet": 0,
            "range": { "r0": 0, "c0": 0, "r1": 3, "c1": 2 }, "kind": "pie", "template": template }),
    );
    let charts = e.workbook().unwrap().sheets[0].charts.clone();
    assert_eq!(bo_sheet::chart::type_name(&charts[1]), "bar");
    assert_eq!(bo_sheet::chart::type_name(&charts[2]), "bar");
    assert_eq!(charts[1].format, charts[2].format);
    assert_eq!(charts[1].format.chart_fill, charts[0].format.chart_fill);
    assert_eq!(charts[1].format.data_labels, charts[0].format.data_labels);
    // The second chart keeps its own data (one series).
    assert_eq!(charts[1].plots[0].series.len(), 1);
    // .crtx export / import.
    let x = exec(
        &mut e,
        &json!({ "cmd": "chartTemplateExport", "template": template }),
    );
    assert_eq!(x["name"], "Đêm.crtx");
    let b64 = x["base64"].as_str().unwrap().to_owned();
    {
        use base64::Engine as _;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(&b64)
            .unwrap();
        keep("chart-design-template.crtx", &bytes);
    }
    let t = exec(
        &mut e,
        &json!({ "cmd": "chartTemplateImport", "base64": b64, "name": "x.crtx" }),
    );
    assert_eq!(t, template);
    let back = reopen(&mut e, "chart-design-templates.xlsx");
    let charts_back = &back.workbook().unwrap().sheets[0].charts;
    assert_eq!(charts_back.len(), 3);
    assert_eq!(
        charts_back[2].format.chart_fill,
        Some(ChartColor::Rgb(0x1E_2530))
    );
}

fn slide_run(e: &mut Engine, v: &Value) -> SlideEditResult {
    let cmd: SlideCommand = serde_json::from_value(v.clone()).unwrap();
    let r = e.slide_command(cmd).unwrap();
    assert!(r.error.is_none(), "{v}: {:?}", r.error);
    r
}

fn slide_engine() -> Engine {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus/slide/slide-text-bullets.pptx");
    let mut e = Engine::with_bundled_fonts();
    e.open(&std::fs::read(p).unwrap(), "deck.pptx").unwrap();
    e
}

fn chart_of(e: &Engine) -> bo_model::sheet::Chart {
    let p = e.presentation().unwrap();
    let part = p
        .parts
        .iter()
        .find(|x| x.name.starts_with("/ppt/charts/chart"))
        .unwrap();
    bo_xlsx::chart_write::parse_chart_part(&part.data, &part.name).unwrap()
}

#[test]
fn slide_chart_design_and_template() {
    let mut e = slide_engine();
    slide_run(
        &mut e,
        &json!({ "cmd": "insertChart", "kind": "column", "title": "Doanh thu",
            "data": { "categories": ["Q1", "Q2", "Q3"],
                "series": [{ "name": "A", "values": [1, 2, 3] }, { "name": "B", "values": [3, 2, 1] }] } }),
    );
    let r = slide_run(
        &mut e,
        &json!({ "cmd": "chartGallery", "width": 100, "height": 70 }),
    );
    let g = r.chart_gallery.unwrap();
    assert_eq!(g["styles"].as_array().unwrap().len(), 12);
    // Colourful variation: the deck theme's accent1.
    let accent1 = match &e.presentation().unwrap().themes[0]
        .scheme_color("accent1")
        .unwrap()
        .kind
    {
        bo_model::slide::ColorKind::Srgb(v) => *v,
        _ => bo_sheet::chart_style::DEFAULT_ACCENTS[0],
    };
    assert_eq!(g["colors"][0]["colors"][0], format!("#{accent1:06X}"));
    slide_run(
        &mut e,
        &json!({ "cmd": "setChart", "style": "outline", "colors": "colorful", "layout": "layout6" }),
    );
    let c = chart_of(&e);
    assert_eq!(c.format.series_fill, SeriesFill::Outline);
    assert_eq!(c.plots[0].series[0].line, Some(ChartColor::Rgb(accent1)));
    assert!(c
        .format
        .data_labels
        .as_ref()
        .is_some_and(|d| d.category && d.value));
    assert_eq!(c.legend, None);
    // Undo, redo.
    slide_run(&mut e, &json!({ "cmd": "undo" }));
    assert_eq!(chart_of(&e).format.series_fill, SeriesFill::Solid);
    slide_run(&mut e, &json!({ "cmd": "redo" }));
    // Saved and reopened.
    let bytes = e.save_pptx().unwrap();
    keep("chart-design-slide.pptx", &bytes);
    let mut back = Engine::with_bundled_fonts();
    back.open(&bytes, "back.pptx").unwrap();
    let c2 = chart_of(&back);
    assert_eq!(c2.format, c.format);
    // A template from the slide chart applied on a new chart.
    let template = g["template"].clone();
    slide_run(
        &mut back,
        &json!({ "cmd": "insertChart", "kind": "line", "template": template }),
    );
    let r = slide_run(&mut back, &json!({ "cmd": "state" }));
    assert_eq!(r.chart.unwrap().kind, "column");
}

#[test]
fn every_style_and_layout_on_many_types_round_trips() {
    let mut e = sheet_with_chart();
    let kinds = [
        "line",
        "pie",
        "doughnut",
        "area",
        "stackedBar",
        "scatter",
        "radarMarker",
        "bubble",
        "column3D",
        "pie3D",
        "explodedPie",
        "barOfPie",
        "percentColumn",
        "surface",
    ];
    let styles = bo_sheet::chart_style::STYLES;
    let layouts = bo_sheet::chart_style::LAYOUTS;
    let palettes = bo_sheet::chart_style::PALETTES;
    let mut n = 1;
    for (i, kind) in kinds.iter().enumerate() {
        for k in 0..2 {
            exec(
                &mut e,
                &json!({ "cmd": "insertChart", "sheet": 0,
                    "range": { "r0": 0, "c0": 0, "r1": 3, "c1": 2 }, "kind": kind }),
            );
            let j = i * 2 + k;
            exec(
                &mut e,
                &json!({ "cmd": "chartDesign", "sheet": 0, "index": n,
                    "style": styles[j % styles.len()],
                    "colors": palettes[j % palettes.len()],
                    "layout": layouts[j % layouts.len()],
                    "texts": { "title": "T", "xTitle": "X", "yTitle": "Y" } }),
            );
            n += 1;
        }
    }
    let before: Vec<_> = e.workbook().unwrap().sheets[0].charts.clone();
    let mut back = reopen(&mut e, "chart-design-all.xlsx");
    let after = back.workbook().unwrap().sheets[0].charts.clone();
    assert_eq!(after.len(), before.len());
    for (i, (c, b)) in after.iter().zip(&before).enumerate() {
        let kind = bo_sheet::chart::type_name(c);
        // What every chart type keeps.
        assert_eq!(
            c.format.chart_fill, b.format.chart_fill,
            "chart {i} ({kind})"
        );
        assert_eq!(
            c.format.text_color, b.format.text_color,
            "chart {i} ({kind})"
        );
        if b.title.is_some() {
            // A bold title is a property of the title's text.
            assert_eq!(
                c.format.title_bold, b.format.title_bold,
                "chart {i} ({kind})"
            );
        }
        assert_eq!(c.legend, b.legend, "chart {i} ({kind})");
        assert_eq!(c.title, b.title, "chart {i} ({kind})");
    }
    // Regenerating the parts from what was read changes nothing (the formatting is stable).
    for i in 0..after.len() {
        exec(
            &mut back,
            &json!({ "cmd": "editChart", "sheet": 0, "index": i }),
        );
    }
    let again = reopen(&mut back, "chart-design-all-2.xlsx");
    let twice = &again.workbook().unwrap().sheets[0].charts;
    for (i, (a, b)) in twice.iter().zip(&after).enumerate() {
        assert_eq!(a.format, b.format, "chart {i}");
    }
}
