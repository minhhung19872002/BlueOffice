//! Charts follow inserted / deleted rows and columns: anchors (§20.5.2.33
//! twoCellAnchor) and series references (§21.2.2.215 `f`), in the model and in
//! the saved chart part, whose other content is kept byte for byte.
#![allow(clippy::unwrap_used)]

use bo_engine::SheetEngine;

fn corpus(name: &str) -> Vec<u8> {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus/sheet")
        .join(name);
    std::fs::read(p).unwrap()
}

fn chart_xml(xlsx: &[u8]) -> String {
    let pkg = bo_opc::Package::read(xlsx, &bo_opc::Limits::default()).unwrap();
    String::from_utf8(pkg.part("/xl/charts/chart1.xml").unwrap().data.clone()).unwrap()
}

/// The chart XML without the texts of its `c:f` elements.
fn without_refs(xml: &str) -> String {
    let mut out = String::new();
    let mut rest = xml;
    while let Some(i) = rest.find("<c:f>") {
        out.push_str(&rest[..i + 5]);
        let j = rest[i..].find("</c:f>").unwrap();
        rest = &rest[i + j..];
    }
    out.push_str(rest);
    out
}

fn refs(e: &SheetEngine) -> Vec<String> {
    e.workbook().unwrap().sheets[0].charts[0]
        .plots
        .iter()
        .flat_map(|p| &p.series)
        .flat_map(|s| [s.name_ref.clone(), s.cat_ref.clone(), s.val_ref.clone()])
        .flatten()
        .collect()
}

#[test]
fn inserted_rows_move_the_chart_and_its_references() {
    let original = corpus("charts-1.xlsx");
    let mut e = SheetEngine::with_bundled_fonts();
    e.open(&original, "charts-1.xlsx").unwrap();
    let from = e.workbook().unwrap().sheets[0].charts[0].from;
    assert_eq!(refs(&e)[1], "'Biểu đồ'!$A$2:$A$4");
    e.structural(0, true, 0, 2, false).unwrap();
    let ch = e.workbook().unwrap().sheets[0].charts[0].clone();
    assert_eq!(ch.from.row, from.row + 2);
    assert_eq!(
        refs(&e),
        [
            "'Biểu đồ'!$B$3",
            "'Biểu đồ'!$A$4:$A$6",
            "'Biểu đồ'!$B$4:$B$6",
            "'Biểu đồ'!$C$3",
            "'Biểu đồ'!$A$4:$A$6",
            "'Biểu đồ'!$C$4:$C$6",
        ]
    );
    // Saved: the chart part has the new references and is otherwise unchanged.
    let saved = e.save("xlsx", 0).unwrap();
    let (before, after) = (chart_xml(&original), chart_xml(&saved));
    assert!(after.contains("<c:f>'Biểu đồ'!$B$4:$B$6</c:f>"), "{after}");
    assert!(!after.contains("$B$2:$B$4"));
    assert_eq!(without_refs(&before), without_refs(&after));
    // The drawing part: only the anchor rows changed (offsets kept).
    let drawing = |x: &[u8]| {
        let pkg = bo_opc::Package::read(x, &bo_opc::Limits::default()).unwrap();
        let d =
            String::from_utf8(pkg.part("/xl/drawings/drawing1.xml").unwrap().data.clone()).unwrap();
        let mut out = String::new();
        let mut rest = d.as_str();
        while let Some(i) = rest.find("<xdr:row>") {
            out.push_str(&rest[..i + 9]);
            let j = rest[i..].find("</xdr:row>").unwrap();
            rest = &rest[i + j..];
        }
        out.push_str(rest);
        (out, d)
    };
    let ((d0, raw0), (d1, raw1)) = (drawing(&original), drawing(&saved));
    assert_eq!(d0, d1);
    assert_ne!(raw0, raw1);
    if let Ok(dir) = std::env::var("BO_WRITE_CHART_SAMPLE") {
        std::fs::write(
            std::path::Path::new(&dir).join("charts-1-rows-inserted.xlsx"),
            &saved,
        )
        .unwrap();
    }
    let mut back = SheetEngine::with_bundled_fonts();
    back.open(&saved, "x.xlsx").unwrap();
    assert_eq!(refs(&back), refs(&e));
    assert_eq!(back.workbook().unwrap().sheets[0].charts[0].from, ch.from);
}

#[test]
fn deleted_columns_move_the_chart_left() {
    let mut e = SheetEngine::with_bundled_fonts();
    e.open(&corpus("charts-1.xlsx"), "charts-1.xlsx").unwrap();
    let from = e.workbook().unwrap().sheets[0].charts[0].from;
    // A column right of the data, left of the chart.
    e.structural(0, false, 3, 1, true).unwrap();
    let ch = &e.workbook().unwrap().sheets[0].charts[0];
    assert_eq!(ch.from.col, from.col - 1);
    assert_eq!(refs(&e)[2], "'Biểu đồ'!$B$2:$B$4");
}
