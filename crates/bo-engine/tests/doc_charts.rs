//! Charts in BlueDoc (CORE-CHART-007 / CORE-CHART-004): insert, edit data, change type, styles,
//! wrapping, undo, save + reopen, chartex in `mc:AlternateContent`, vector drawing on the page.
// Tests may unwrap: a panic is a test failure.
#![allow(clippy::unwrap_used)]

use bo_engine::Engine;
use bo_opc::{Limits, Package};
use serde_json::{json, Value};

fn engine() -> Engine {
    let mut e = Engine::with_bundled_fonts();
    e.open("Báo cáo doanh thu".as_bytes(), "new.txt").unwrap();
    e
}

#[allow(clippy::needless_pass_by_value)]
fn run(e: &mut Engine, v: Value) -> Value {
    serde_json::from_str(&e.command_json(&v.to_string()).unwrap()).unwrap()
}

fn grid(values: [[f64; 2]; 3]) -> Value {
    json!({
        "categories": ["Q1", "Q2", "Q3"],
        "series": [
            {"name": "Bắc", "values": values.iter().map(|r| r[0]).collect::<Vec<_>>()},
            {"name": "Nam", "values": values.iter().map(|r| r[1]).collect::<Vec<_>>()},
        ]
    })
}

fn part(pkg: &Package, name: &str) -> String {
    String::from_utf8(pkg.part(name).unwrap().data.clone()).unwrap()
}

/// Insert a chart and put the caret after it (a new chart stays selected: the next insertion
/// would replace it).
fn insert(e: &mut Engine, v: Value) -> Value {
    let r = run(e, v);
    let f = r["selection"]["focus"].clone();
    run(
        e,
        json!({"cmd": "setSelection", "selection": {"anchor": f, "focus": f}}),
    );
    r
}

/// Select the first chart of the body (a caret before it selects it).
fn select_chart(e: &mut Engine) {
    let doc = e.document().unwrap();
    let paths = bo_model::index::body_paragraphs(doc);
    for (id, path) in paths.iter().enumerate() {
        let p = bo_model::index::para_at(doc, path).unwrap();
        let mut off = 0;
        for (_, item) in bo_edit::para::flatten(p) {
            if matches!(&item, bo_model::RunContent::Image(i) if bo_model::chart_part::is_chart_name(&i.media))
            {
                run(
                    e,
                    json!({"cmd": "setSelection", "selection": {"anchor": {"p": id, "o": off}, "focus": {"p": id, "o": off + 1}}}),
                );
                return;
            }
            off += bo_edit::para::atom_len(&item);
        }
    }
    panic!("no chart");
}

// source: ECMA-376 Part 1, §21.2.2.27 chart, §21.2.2.29 chartSpace, §21.2.2.63 externalData,
// §20.4.2.8 inline; default size 15 × 9 cm = 5 400 000 × 3 240 000 EMU (360 000 EMU per cm).
#[test]
fn insert_edit_change_type_undo_and_save() {
    let mut e = engine();
    let r = run(
        &mut e,
        json!({"cmd": "insertChart", "kind": "column", "title": "Doanh thu", "data": grid([[10.0, 20.0], [30.0, 40.0], [50.0, 60.0]])}),
    );
    assert!(r["error"].is_null(), "{r}");
    assert_eq!(r["format"]["chart"], true);
    let c = &r["chart"];
    assert_eq!(c["kind"], "column");
    assert_eq!(c["title"], "Doanh thu");
    assert_eq!(c["wrap"], "inline");
    assert_eq!(c["embedded"], true);
    assert!((c["width"].as_f64().unwrap() - 5_400_000.0 / 12_700.0).abs() < 1e-6);
    assert!((c["height"].as_f64().unwrap() - 3_240_000.0 / 12_700.0).abs() < 1e-6);
    assert_eq!(c["data"]["series"][1]["values"][2], 60.0);
    let doc = e.document().unwrap();
    assert!(doc.media.contains_key("/word/charts/chart1.xml"));
    assert!(doc.media.contains_key("/word/embeddings/chart1.xlsx"));
    // The chart is drawn as vectors on the page (no picture command for it).
    let page = e.page(0).unwrap();
    assert!(!page
        .commands
        .iter()
        .any(|c| matches!(c, bo_render::Command::Image { image, .. } if image.contains("chart"))));
    assert!(page
        .commands
        .iter()
        .any(|c| matches!(c, bo_render::Command::PushTransform { .. })));
    // Its media part shows as a PNG picture (exports).
    assert_eq!(
        &e.media("/word/charts/chart1.xml").unwrap()[..4],
        b"\x89PNG"
    );

    // Edit two values in the data grid, then change the type: one undo step each.
    let r = run(
        &mut e,
        json!({"cmd": "setChart", "data": grid([[10.0, 20.0], [35.5, 40.0], [50.0, 77.0]])}),
    );
    assert!(r["error"].is_null(), "{r}");
    assert_eq!(r["chart"]["data"]["series"][0]["values"][1], 35.5);
    let r = run(&mut e, json!({"cmd": "setChart", "kind": "line"}));
    assert!(r["error"].is_null(), "{r}");
    assert_eq!(r["chart"]["kind"], "line");
    assert_eq!(r["chart"]["data"]["series"][1]["values"][2], 77.0);
    run(&mut e, json!({"cmd": "undo"}));
    assert_eq!(e.doc_chart().unwrap().kind, "column");
    run(&mut e, json!({"cmd": "undo"}));
    assert_eq!(e.doc_chart().unwrap().data.series[0].values[1], Some(30.0));
    run(&mut e, json!({"cmd": "redo"}));
    run(&mut e, json!({"cmd": "redo"}));
    assert_eq!(e.doc_chart().unwrap().kind, "line");

    // Saved: chart part, workbook, relationships and content types (§21.2.2.63, §15.2.11).
    let bytes = e.save_docx().unwrap();
    let pkg = Package::read(&bytes, &Limits::default()).unwrap();
    let ct = pkg.content_types().unwrap();
    assert_eq!(
        ct.get("/word/charts/chart1.xml"),
        Some(bo_model::chart_part::CT_CHART)
    );
    let rels = pkg.relationships("/word/document.xml").unwrap();
    let rel = rels
        .iter()
        .find(|r| r.rel_type == bo_model::chart_part::REL_CHART)
        .unwrap();
    assert_eq!(
        bo_opc::resolve_target("/word/document.xml", &rel.target),
        "/word/charts/chart1.xml"
    );
    let main = part(&pkg, "/word/document.xml");
    assert!(main.contains(&format!("r:id=\"{}\"", rel.id)));
    assert!(main.contains("<wp:inline"));
    assert!(main.contains(r#"<wp:extent cx="5400000" cy="3240000"/>"#));
    let chart_rels = pkg.relationships("/word/charts/chart1.xml").unwrap();
    assert_eq!(chart_rels.len(), 1);
    assert_eq!(chart_rels[0].rel_type, bo_model::ole::REL_PACKAGE);
    let book = bo_opc::resolve_target("/word/charts/chart1.xml", &chart_rels[0].target);
    assert_eq!(book, "/word/embeddings/chart1.xlsx");
    // The embedded workbook holds the edited values.
    let wb = bo_xlsx::read_xlsx(&pkg.part(&book).unwrap().data).unwrap();
    let cell = |r: u32, c: u32| wb.sheets[0].cells.get(&(r, c)).map(|c| c.value.clone());
    assert_eq!(cell(2, 1), Some(bo_model::sheet::CellValue::Number(35.5)));
    assert_eq!(cell(3, 2), Some(bo_model::sheet::CellValue::Number(77.0)));
    let xml = part(&pkg, "/word/charts/chart1.xml");
    assert!(xml.contains("<c:lineChart>"), "{xml}");
    assert!(xml.contains("<c:v>35.5</c:v>") && xml.contains("<c:v>77</c:v>"));
    assert!(xml.contains(r#"<c:externalData r:id="rId1">"#));

    // Reopened: the same chart, unchanged parts written back verbatim.
    let mut back = engine();
    back.open(&bytes, "back.docx").unwrap();
    select_chart(&mut back);
    let info = back.doc_chart().unwrap();
    assert_eq!(info.kind, "line");
    assert_eq!(info.title.as_deref(), Some("Doanh thu"));
    assert_eq!(info.data.series[1].values[2], Some(77.0));
    assert!(info.embedded);
    let again = back.save_docx().unwrap();
    let pkg2 = Package::read(&again, &Limits::default()).unwrap();
    assert_eq!(part(&pkg2, "/word/charts/chart1.xml"), xml);
    // Editing the file's chart writes new parts beside the file's.
    let r = run(&mut back, json!({"cmd": "setChart", "kind": "bar"}));
    assert!(r["error"].is_null(), "{r}");
    assert_eq!(r["chart"]["part"], "/word/charts/chart2.xml");
    let saved = back.save_docx().unwrap();
    let pkg3 = Package::read(&saved, &Limits::default()).unwrap();
    assert!(part(&pkg3, "/word/charts/chart2.xml").contains(r#"<c:barDir val="bar"/>"#));
    assert!(pkg3.part("/word/embeddings/chart2.xlsx").is_some());
}

// source: ECMA-376 Part 1, §20.4.2.3 anchor (behindDoc), §20.4.2.17 wrapSquare, §20.4.2.20
// wrapTight, wrapNone.
#[test]
fn wrapping_inline_square_tight_behind_in_front() {
    let mut e = engine();
    run(&mut e, json!({"cmd": "insertChart", "kind": "pie"}));
    for (wrap, markup) in [
        ("square", "<wp:wrapSquare"),
        ("tight", "<wp:wrapTight"),
        ("behind", "behindDoc=\"1\""),
        ("inFront", "<wp:wrapNone/>"),
        ("inline", "<wp:inline"),
    ] {
        let r = run(&mut e, json!({"cmd": "setChart", "wrap": wrap}));
        assert!(r["error"].is_null(), "{wrap}: {r}");
        assert_eq!(r["chart"]["wrap"], wrap);
        let pkg = Package::read(&e.save_docx().unwrap(), &Limits::default()).unwrap();
        let main = part(&pkg, "/word/document.xml");
        assert!(main.contains(markup), "{wrap}: {main}");
        if wrap == "inFront" {
            assert!(main.contains("behindDoc=\"0\""), "{main}");
        }
        // A reopened file keeps the wrapping.
        let mut back = engine();
        back.open(&pkg.write().unwrap(), "w.docx").unwrap();
        select_chart(&mut back);
        assert_eq!(back.doc_chart().unwrap().wrap, wrap);
    }
    let r = run(&mut e, json!({"cmd": "setChart", "wrap": "around"}));
    assert_eq!(r["error"], "badChart");
    // Floating at insertion, with a size.
    let r = run(
        &mut e,
        json!({"cmd": "insertChart", "kind": "doughnut", "wrap": "square", "width": 200, "height": 120}),
    );
    assert_eq!(r["chart"]["wrap"], "square");
    assert_eq!(r["chart"]["width"], 200.0);
}

// source: [MS-ODRAWXML] 2.24 chartex (cx:chart in an mc:AlternateContent choice, ECMA-376 Part 3,
// §10.2.1); funnels require the 2015/10/21 chartex namespace.
#[test]
fn chartex_types_are_written_in_alternate_content() {
    let mut e = engine();
    for kind in [
        "waterfall",
        "treemap",
        "sunburst",
        "histogram",
        "pareto",
        "boxWhisker",
        "funnel",
    ] {
        let r = insert(&mut e, json!({"cmd": "insertChart", "kind": kind}));
        assert!(r["error"].is_null(), "{kind}: {r}");
        assert_eq!(r["chart"]["kind"], kind);
    }
    // A chartex chart becomes a classic one and back: a part of the other family each time
    // (§21.2.2.29 chartSpace / [MS-ODRAWXML] 2.24), the data kept; undo goes back.
    let r = run(&mut e, json!({"cmd": "setChart", "kind": "column"}));
    assert!(r["error"].is_null(), "{r}");
    assert_eq!(r["chart"]["kind"], "column");
    let data = r["chart"]["data"].clone();
    let r = run(&mut e, json!({"cmd": "setChart", "kind": "funnel"}));
    assert!(r["error"].is_null(), "{r}");
    assert_eq!(r["chart"]["kind"], "funnel");
    assert_eq!(r["chart"]["data"], data);
    run(&mut e, json!({"cmd": "undo"}));
    assert_eq!(e.doc_chart().unwrap().kind, "column");
    run(&mut e, json!({"cmd": "redo"}));
    assert_eq!(e.doc_chart().unwrap().kind, "funnel");
    let r = run(&mut e, json!({"cmd": "setChart", "title": "Phễu"}));
    assert!(r["error"].is_null(), "{r}");
    let bytes = e.save_docx().unwrap();
    let pkg = Package::read(&bytes, &Limits::default()).unwrap();
    let ct = pkg.content_types().unwrap();
    let main = part(&pkg, "/word/document.xml");
    assert_eq!(main.matches("<mc:AlternateContent").count(), 7);
    assert!(main.contains(r#"Requires="cx2""#) && main.contains(r#"Requires="cx1""#));
    assert!(main.contains("<mc:Fallback><w:t"));
    let rels = pkg.relationships("/word/document.xml").unwrap();
    let ex: Vec<_> = rels
        .iter()
        .filter(|r| r.rel_type == bo_model::chart_part::REL_CHARTEX)
        .collect();
    assert_eq!(ex.len(), 7);
    for r in ex {
        let name = bo_opc::resolve_target("/word/document.xml", &r.target);
        assert_eq!(ct.get(&name), Some(bo_model::chart_part::CT_CHARTEX));
        assert!(part(&pkg, &name).contains("<cx:chartSpace"));
        assert_eq!(pkg.relationships(&name).unwrap().len(), 1);
    }
    // Reopened: the choice's chart is read back.
    let mut back = engine();
    back.open(&bytes, "cx.docx").unwrap();
    select_chart(&mut back);
    assert_eq!(back.doc_chart().unwrap().kind, "waterfall");
    let page = back.page(0).unwrap();
    assert!(page
        .commands
        .iter()
        .any(|c| matches!(c, bo_render::Command::PushTransform { .. })));
}

// source: CORE-CHART-007 presets (bo_sheet::chart_style STYLES / PALETTES / LAYOUTS) applied by
// the shared chart design commands.
#[test]
fn styles_colours_layouts_gallery_and_refusals() {
    let mut e = engine();
    let r = run(&mut e, json!({"cmd": "setChart", "kind": "line"}));
    assert_eq!(r["error"], "noChart");
    let r = run(&mut e, json!({"cmd": "insertChart", "kind": "nope"}));
    assert_eq!(r["error"], "badChart");
    run(&mut e, json!({"cmd": "insertChart", "kind": "column"}));
    let r = run(
        &mut e,
        json!({"cmd": "chartGallery", "width": 80, "height": 50}),
    );
    let g = &r["chartGallery"];
    assert!(g["styles"].as_array().unwrap().len() >= 10, "{g}");
    assert!(g["styles"][0]["svg"].as_str().unwrap().starts_with("<svg"));
    let style = g["styles"][3]["id"].as_str().unwrap().to_owned();
    let colors = g["colors"][5]["id"].as_str().unwrap().to_owned();
    let layout = g["layouts"][1]["id"].as_str().unwrap().to_owned();
    let before = e.save_docx().unwrap();
    for spec in [
        json!({"cmd": "setChart", "style": style}),
        json!({"cmd": "setChart", "colors": colors}),
        json!({"cmd": "setChart", "layout": layout, "texts": {"title": "Tiêu đề", "xTitle": "X", "yTitle": "Y"}}),
    ] {
        let r = run(&mut e, spec.clone());
        assert!(r["error"].is_null(), "{spec}: {r}");
    }
    let after = e.save_docx().unwrap();
    let chart = |b: &[u8]| {
        part(
            &Package::read(b, &Limits::default()).unwrap(),
            "/word/charts/chart1.xml",
        )
    };
    assert_ne!(chart(&before), chart(&after));
    // Title, legend; a template from the gallery applied to another chart.
    let r = run(
        &mut e,
        json!({"cmd": "setChart", "title": "Mới", "legend": "r"}),
    );
    assert_eq!(r["chart"]["title"], "Mới");
    assert_eq!(r["chart"]["legend"], "r");
    let template = run(&mut e, json!({"cmd": "chartGallery"}))["chartGallery"]["template"].clone();
    let r = run(
        &mut e,
        json!({"cmd": "insertChart", "kind": "bar", "template": template}),
    );
    assert!(r["error"].is_null(), "{r}");
    let r = run(&mut e, json!({"cmd": "setChart", "style": "nope"}));
    assert_eq!(r["error"], "badChart");
    let r = run(&mut e, json!({"cmd": "setChart"}));
    assert_eq!(r["error"], "noChange");
}

// source: the chart types of BlueSlide's chart dialog (slide_edit::chart MORE_KINDS), written as
// ECMA-376 Part 1, §21.2.2.29 chart parts.
#[test]
fn every_slide_chart_type_inserts_in_a_document() {
    let mut e = engine();
    for kind in [
        "column",
        "bar",
        "line",
        "area",
        "pie",
        "doughnut",
        "stackedColumn",
        "percentColumn",
        "stackedBar",
        "percentBar",
        "stackedLine",
        "percentLine",
        "stackedArea",
        "percentArea",
        "explodedPie",
        "pieOfPie",
        "barOfPie",
        "radar",
        "radarMarker",
        "radarFilled",
        "stock",
        "stockUpDown",
        "stockVolume",
        "stockVolumeUpDown",
        "surface",
        "column3D",
        "clusteredColumn3D",
        "stackedColumn3D",
        "percentColumn3D",
        "cylinder3D",
        "cone3D",
        "pyramid3D",
        "bar3D",
        "stackedBar3D",
        "percentBar3D",
        "line3D",
        "area3D",
        "stackedArea3D",
        "percentArea3D",
        "pie3D",
        "explodedPie3D",
        "surface3D",
    ] {
        let r = insert(&mut e, json!({"cmd": "insertChart", "kind": kind}));
        assert!(r["error"].is_null(), "{kind}: {r}");
        assert_eq!(r["chart"]["kind"], kind, "{kind}");
    }
    let bytes = e.save_docx().unwrap();
    let pkg = Package::read(&bytes, &Limits::default()).unwrap();
    let n = pkg
        .parts()
        .iter()
        .filter(|p| p.name.starts_with("/word/charts/chart") && p.name.ends_with(".xml"))
        .count();
    assert_eq!(n, 42);
}

/// One line per command of a page, coordinates rounded to 0.1 pt (text runs with their string).
fn summary(cmds: &[bo_render::Command]) -> String {
    use bo_render::{Command, Paint, PathSeg};
    use std::fmt::Write as _;
    let rgba = |c: bo_render::Rgba| format!("#{:02x}{:02x}{:02x}{:02x}", c.0, c.1, c.2, c.3);
    let mut s = String::new();
    let mut depth = 0usize;
    for c in cmds {
        if matches!(
            c,
            Command::PopClip | Command::PopTransform | Command::PopOpacity
        ) {
            depth = depth.saturating_sub(1);
        }
        s.push_str(&"  ".repeat(depth));
        match c {
            Command::FillRect { x, y, w, h, color } => {
                let _ = writeln!(s, "rect {x:.1},{y:.1} {w:.1}x{h:.1} {}", rgba(*color));
            }
            Command::Line { x1, y1, x2, y2, .. } => {
                let _ = writeln!(s, "line {x1:.1},{y1:.1} → {x2:.1},{y2:.1}");
            }
            Command::Glyphs {
                size, x, y, text, ..
            } => {
                let _ = writeln!(s, "text {size:.1}pt {x:.1},{y:.1} {text:?}");
            }
            Command::PushClip { x, y, w, h } => {
                let _ = writeln!(s, "clip {x:.1},{y:.1} {w:.1}x{h:.1}");
            }
            Command::Image {
                x, y, w, h, image, ..
            } => {
                let _ = writeln!(s, "image {image} {x:.1},{y:.1} {w:.1}x{h:.1}");
            }
            Command::Path { segs, fill, .. } => {
                let mut b = [f32::MAX, f32::MAX, f32::MIN, f32::MIN];
                for seg in segs {
                    if let PathSeg::M(x, y) | PathSeg::L(x, y) = *seg {
                        b = [b[0].min(x), b[1].min(y), b[2].max(x), b[3].max(y)];
                    }
                }
                let f = match fill {
                    Some(Paint::Solid { color }) => rgba(*color),
                    Some(_) => "gradient".into(),
                    None => "none".into(),
                };
                let _ = writeln!(
                    s,
                    "path n={} [{:.1} {:.1} {:.1} {:.1}] fill={f}",
                    segs.len(),
                    b[0],
                    b[1],
                    b[2],
                    b[3]
                );
            }
            Command::PushTransform { m } => {
                let _ = writeln!(
                    s,
                    "transform [{:.3} {:.3} {:.3} {:.3} {:.1} {:.1}]",
                    m[0], m[1], m[2], m[3], m[4], m[5]
                );
            }
            Command::PopTransform => s.push_str("pop transform\n"),
            Command::PopClip => s.push_str("pop clip\n"),
            other => {
                let d = format!("{other:?}");
                let _ = writeln!(s, "{}", d.split([' ', '{']).next().unwrap_or(""));
            }
        }
        if matches!(
            c,
            Command::PushClip { .. }
                | Command::PushTransform { .. }
                | Command::PushClipPath { .. }
                | Command::PushOpacity { .. }
        ) {
            depth += 1;
        }
    }
    s
}

// source: corpus doc-chart-{1,2,3}.docx (written by BlueDoc's chart commands, crates/bo-cli/src/
// doc_charts.rs): the display list of each page, its charts drawn as vectors in their frames.
#[test]
fn doc_chart_page_snapshots() {
    for name in ["doc-chart-1", "doc-chart-2", "doc-chart-3"] {
        let path = format!(
            "{}/../../tests/corpus/doc/{name}.docx",
            env!("CARGO_MANIFEST_DIR")
        );
        let mut e = Engine::with_bundled_fonts();
        let info = e
            .open(&std::fs::read(path).unwrap(), &format!("{name}.docx"))
            .unwrap();
        for i in 0..info.pages.len() {
            let page = e.page(i).unwrap();
            assert!(
                !page.commands.iter().any(|c| matches!(c,
                    bo_render::Command::Image { image, .. } if image.contains("/charts/"))),
                "{name}: a chart left as a picture"
            );
            insta::assert_snapshot!(format!("{name}-{}", i + 1), summary(&page.commands));
        }
    }
}
