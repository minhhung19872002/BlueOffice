//! Slot geometry of equations for the caret inside a math zone (CORE-MATH-010, ECMA-376 Part 1, §22.1).
// Integration tests may unwrap.
#![allow(clippy::unwrap_used)]

use bo_layout::{equation_geometry, layout_document, EquationGeometry, LayoutCache};
use bo_model::{
    Block, Document, HalfPoints, ParaProps, Paragraph, Raw, Run, RunContent, RunProps, Section,
};
use bo_text::FontManager;

const NS: &str = r#"xmlns:m="http://schemas.openxmlformats.org/officeDocument/2006/math""#;

fn fonts() -> FontManager {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/fonts");
    let mut fm = FontManager::new();
    fm.add_fonts_from_dir(&dir);
    fm
}

/// A document whose only paragraph is "x " and the equation `xml`.
fn doc(xml: &str) -> Document {
    let p = Paragraph::new(
        ParaProps::default(),
        vec![
            Run::text("x ", RunProps::default()),
            Run {
                props: RunProps::default(),
                content: vec![RunContent::Markup(Raw::new(xml, 0))],
            },
        ],
    );
    Document {
        default_run: RunProps {
            font: Some("Times New Roman".into()),
            size: Some(HalfPoints(24)),
            ..Default::default()
        },
        sections: vec![Section::new(vec![Block::Paragraph(p)])],
        ..Default::default()
    }
}

fn geometry(xml: &str) -> (EquationGeometry, f32) {
    let d = doc(xml);
    let mut fm = fonts();
    let layout = layout_document(&d, &mut fm, &mut LayoutCache::new()).unwrap();
    let Some(Block::Paragraph(p)) = d.sections[0].blocks.first() else {
        panic!("no paragraph")
    };
    let g = equation_geometry(&d, &mut fm, p, 2, xml).unwrap();
    // The equation is an object of the line from offset 2 to 5 (U+FFFC): its width as laid out.
    let line = &layout.pages[0].lines[0];
    let x = |o: u32| line.carets.iter().find(|c| c.0 == o).unwrap().1;
    (g, x(5) - x(2))
}

/// Geometry rounded to 0.1 pt for the snapshot.
fn rounded(g: &EquationGeometry) -> serde_json::Value {
    let r = |v: f32| (f64::from(v) * 10.0).round() / 10.0;
    serde_json::json!(g
        .slots
        .iter()
        .map(|s| serde_json::json!({
            "x": r(s.x),
            "w": r(s.w),
            "top": r(s.top),
            "bottom": r(s.bottom),
            "stops": s.stops.iter().map(|v| r(*v)).collect::<Vec<_>>(),
        }))
        .collect::<Vec<_>>())
}

// source: §22.1.2.36 f (numerator above the fraction bar, denominator below), §22.1.2.98 r.
#[test]
fn caret_stops_inside_a_fraction() {
    let xml = format!(
        "<m:oMath {NS}><m:f><m:num><m:r><m:t>a</m:t></m:r></m:num><m:den><m:r><m:t>b+c</m:t></m:r></m:den></m:f></m:oMath>"
    );
    let (g, w) = geometry(&xml);
    // The geometry is the zone as the paragraph lays it out.
    assert!((g.w - w).abs() < 0.01, "{} vs {w}", g.w);
    // Zone, numerator, denominator; one stop per offset.
    assert_eq!(g.slots.len(), 3);
    assert_eq!(g.slots[0].stops.len(), 2);
    assert_eq!(g.slots[1].stops.len(), 2);
    assert_eq!(g.slots[2].stops.len(), 4);
    let (num, den) = (&g.slots[1], &g.slots[2]);
    assert!(num.bottom < den.top, "numerator above the denominator");
    assert!(num.bottom < 0.0 && den.top > -g.asc);
    for s in &g.slots {
        assert!(s.stops.windows(2).all(|p| p[0] <= p[1]), "{:?}", s.stops);
    }
    insta::assert_json_snapshot!(rounded(&g));
}

// source: §22.1.2.60 m / §22.1.2.69 mr (cells row by row), §22.1.2.24 d around it.
#[test]
fn caret_stops_inside_a_matrix() {
    let cell = |t: &str| {
        if t.is_empty() {
            "<m:e/>".to_owned()
        } else {
            format!("<m:e><m:r><m:t>{t}</m:t></m:r></m:e>")
        }
    };
    let xml = format!(
        "<m:oMath {NS}><m:d><m:e><m:m><m:mr>{}{}</m:mr><m:mr>{}{}</m:mr></m:m></m:e></m:d></m:oMath>",
        cell("1"),
        cell("22"),
        cell("3"),
        cell("")
    );
    let (g, w) = geometry(&xml);
    assert!((g.w - w).abs() < 0.01, "{} vs {w}", g.w);
    // Zone, the bracket's part, four cells.
    assert_eq!(g.slots.len(), 6);
    let (c11, c12, c21, c22) = (&g.slots[2], &g.slots[3], &g.slots[4], &g.slots[5]);
    assert!(c11.x < c12.x && c21.x < c22.x, "columns left to right");
    assert!(c11.bottom <= c21.top + 0.01, "rows top to bottom");
    // The empty cell keeps a placeholder's width and has one stop.
    assert!(c22.w > 0.0);
    assert_eq!(c22.stops.len(), 1);
    insta::assert_json_snapshot!(rounded(&g));
}
