//! Links to other workbooks (ECMA-376 Part 1, §18.14; SHEET-FORMULA-001). Reference
//! behaviour (public documentation of the Edit Links dialog of spreadsheet applications):
//! a workbook opened without its sources calculates with the values saved for the links;
//! Update Values reads the source again; Change Source points the link at another file
//! (the formulas keep their references, now to that file); Break Link turns every formula
//! using the link into its current value and removes the link. Expected values worked by
//! hand from the data below.
// Tests may unwrap: a panic is a test failure.
#![allow(clippy::unwrap_used)]

use base64::Engine as _;
use bo_engine::SheetEngine;
use bo_model::sheet::{
    Cell, CellValue, DefinedName, ExternalLink, OpaquePart, OpaqueRel, RawXml, Workbook, Worksheet,
};
use serde_json::{json, Value};

const PART: &str = "/xl/externalLinks/externalLink1.xml";

/// A workbook linking `Giá.xlsx` (sheet `Bảng giá`: B2 = 100, B3 = 250 saved) and a second
/// link `Kho.xlsx` (sheet `S`: A1 = 7) that formulas number `[2]`.
fn linked() -> Vec<u8> {
    let mut wb = Workbook::new();
    wb.sheets.clear();
    let mut ws = Worksheet::new("Tính");
    ws.set_cell(
        0,
        0,
        Cell::new(CellValue::Number(200.0)).with_formula("'[1]Bảng giá'!B2*2"),
    );
    ws.set_cell(
        1,
        0,
        Cell::new(CellValue::Number(350.0)).with_formula("SUM('[1]Bảng giá'!B2:B3)"),
    );
    ws.set_cell(
        2,
        0,
        Cell::new(CellValue::Number(7.0)).with_formula("[2]S!A1"),
    );
    ws.set_cell(3, 0, Cell::new(CellValue::Number(3.0)).with_formula("1+2"));
    // A dynamic array (cell metadata XLDAPR) and a legacy array formula next to the links.
    for (at, text, range, dynamic) in [
        ((0, 4), "SEQUENCE(2)", "E1:E2", true),
        ((0, 5), "A1:A2*2", "F1:F3", false),
    ] {
        let mut c = Cell::new(CellValue::Number(1.0)).with_formula(text);
        if let Some(f) = c.formula.as_mut() {
            f.kind = bo_model::sheet::FormulaKind::Array {
                range: range.into(),
            };
            f.dynamic = dynamic;
        }
        ws.set_cell(at.0, at.1, c);
    }
    wb.sheets.push(ws);
    wb.defined_names.push(DefinedName {
        name: "Giá_gốc".into(),
        formula: "'[1]Bảng giá'!$B$2".into(),
        ..DefinedName::default()
    });
    let links = [
        (
            PART,
            "rId90",
            "Gi%C3%A1.xlsx",
            "Bảng giá",
            vec![(1, 1, 100.0), (2, 1, 250.0)],
        ),
        (
            "/xl/externalLinks/externalLink2.xml",
            "rId91",
            "Kho.xlsx",
            "S",
            vec![(0, 0, 7.0)],
        ),
    ];
    let mut refs = String::from("<externalReferences>");
    for (part, rid, target, sheet, cells) in links {
        wb.external_links.push(ExternalLink {
            part: part.into(),
            rel_id: rid.into(),
            target: target.into(),
            sheets: vec![sheet.into()],
            cells: vec![cells
                .into_iter()
                .map(|(r, c, v)| (r, c, CellValue::Number(v)))
                .collect()],
            names: Vec::new(),
        });
        wb.opaque.parts.push(OpaquePart {
            name: part.into(),
            content_type: Some(
                "application/vnd.openxmlformats-officedocument.spreadsheetml.externalLink+xml"
                    .into(),
            ),
            data: Vec::new(),
        });
        wb.opaque.workbook_rels.push(OpaqueRel {
            id: rid.into(),
            rel_type: bo_xlsx::extlink::REL_EXTERNAL_LINK.into(),
            target: part.into(),
            external: false,
        });
        refs.push_str(&format!(
            "<externalReference xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\" r:id=\"{rid}\"/>"
        ));
    }
    refs.push_str("</externalReferences>");
    wb.unknown.push(RawXml {
        after: "externalReferences".into(),
        xml: refs,
    });
    assert!(bo_xlsx::extlink::store(&mut wb, 0));
    assert!(bo_xlsx::extlink::store(&mut wb, 1));
    bo_xlsx::write_xlsx(&wb).unwrap()
}

/// The source workbook with new prices (B2 = 120, B3 = 300) and a sheet the link lacks.
fn source() -> String {
    base64::engine::general_purpose::STANDARD.encode(source_bytes())
}

fn source_bytes() -> Vec<u8> {
    let mut wb = Workbook::new();
    wb.sheets.clear();
    let mut ws = Worksheet::new("Bảng giá");
    ws.set_a1("B2", CellValue::Number(120.0));
    ws.set_a1("B3", CellValue::Number(300.0));
    ws.set_a1("Z99", CellValue::Number(1.0));
    wb.sheets.push(ws);
    bo_xlsx::write_xlsx(&wb).unwrap()
}

fn value(e: &SheetEngine, a1: &str) -> CellValue {
    e.workbook().unwrap().sheets[0]
        .cell_a1(a1)
        .map_or(CellValue::Empty, |c| c.value.clone())
}

fn formula(e: &SheetEngine, a1: &str) -> Option<String> {
    e.workbook().unwrap().sheets[0]
        .cell_a1(a1)
        .and_then(|c| c.formula.as_ref())
        .map(|f| f.text.clone())
}

fn exec(e: &mut SheetEngine, cmd: &Value) -> Value {
    serde_json::from_str(&e.exec_json(&cmd.to_string()).unwrap()).unwrap()
}

fn opened() -> SheetEngine {
    let mut e = SheetEngine::new();
    e.open(&linked(), "a.xlsx").unwrap();
    e
}

#[test]
fn saved_link_values_are_read_and_used() {
    let mut e = opened();
    let wb = e.workbook().unwrap();
    assert_eq!(wb.external_links.len(), 2);
    assert_eq!(wb.external_links[0].sheets, ["Bảng giá"]);
    assert_eq!(
        wb.external_links[0].cells[0],
        [
            (1, 1, CellValue::Number(100.0)),
            (2, 1, CellValue::Number(250.0))
        ]
    );
    let list = exec(&mut e, &json!({"cmd": "externalLinks"}));
    assert_eq!(list["links"][0]["source"], "Giá.xlsx");
    assert_eq!(list["links"][0]["formulas"], 3);
    assert_eq!(list["links"][1]["source"], "Kho.xlsx");
    // A full recalculation uses the saved values (not #REF!).
    exec(&mut e, &json!({"cmd": "calculate"}));
    assert_eq!(value(&e, "A1"), CellValue::Number(200.0));
    assert_eq!(value(&e, "A2"), CellValue::Number(350.0));
    assert_eq!(value(&e, "A3"), CellValue::Number(7.0));
    // A new formula over the link calculates from them too.
    e.set_inputs(0, &[(4, 0, "='[1]Bảng giá'!B3+1".into())])
        .unwrap();
    assert_eq!(value(&e, "A5"), CellValue::Number(251.0));
    // The formula bar names the linked file; typed back that way it is the same link.
    assert_eq!(e.cell(0, 0, 0).unwrap().input, "='[Giá.xlsx]Bảng giá'!B2*2");
    e.set_inputs(
        0,
        &[(5, 0, "='[giá.xlsx]Bảng giá'!B2+'[Kho.xlsx]S'!A1".into())],
    )
    .unwrap();
    assert_eq!(value(&e, "A6"), CellValue::Number(107.0));
    assert_eq!(
        formula(&e, "A6").as_deref(),
        Some("'[1]Bảng giá'!B2+'[2]S'!A1")
    );
    // Saved untouched, the link part is the same bytes.
    let out = e.save("xlsx", 0).unwrap();
    let mut again = SheetEngine::new();
    again.open(&out, "a.xlsx").unwrap();
    assert_eq!(
        again.workbook().unwrap().external_links,
        e.workbook().unwrap().external_links
    );
}

#[test]
fn update_values_and_change_source() {
    let mut e = opened();
    let r = exec(
        &mut e,
        &json!({"cmd": "externalLinkUpdate", "index": 0, "base64": source(), "name": "Giá.xlsx"}),
    );
    assert_eq!(r["missing"], json!([]));
    // B2 120 → A1 240; B2+B3 = 420. Only the cells the formulas read are kept (not Z99).
    assert_eq!(value(&e, "A1"), CellValue::Number(240.0));
    assert_eq!(value(&e, "A2"), CellValue::Number(420.0));
    assert_eq!(e.workbook().unwrap().external_links[0].cells[0].len(), 2);
    // One undo step.
    e.undo().unwrap();
    assert_eq!(value(&e, "A1"), CellValue::Number(200.0));
    // Change Source: the link names the new file and reads its values.
    exec(
        &mut e,
        &json!({"cmd": "externalLinkUpdate", "index": 0, "base64": source(), "name": "Giá 2026.xlsx", "changeSource": true}),
    );
    assert_eq!(
        e.workbook().unwrap().external_links[0].target,
        "Giá 2026.xlsx"
    );
    assert_eq!(value(&e, "A2"), CellValue::Number(420.0));
    // The new target and values survive saving.
    let out = e.save("xlsx", 0).unwrap();
    let mut again = SheetEngine::new();
    again.open(&out, "a.xlsx").unwrap();
    let link = &again.workbook().unwrap().external_links[0];
    assert_eq!(link.target, "Giá 2026.xlsx");
    assert_eq!(link.cells[0][0], (1, 1, CellValue::Number(120.0)));
    // A source without the linked sheet: reported, values kept.
    let mut other = Workbook::new();
    other.sheets[0].name = "Khác".into();
    let b64 =
        base64::engine::general_purpose::STANDARD.encode(bo_xlsx::write_xlsx(&other).unwrap());
    let r = exec(
        &mut e,
        &json!({"cmd": "externalLinkUpdate", "index": 0, "base64": b64, "name": "x.xlsx"}),
    );
    assert_eq!(r["missing"], json!(["Bảng giá"]));
    assert_eq!(value(&e, "A1"), CellValue::Number(240.0));
}

#[test]
fn break_link_keeps_values_and_renumbers() {
    let mut e = opened();
    let r = exec(&mut e, &json!({"cmd": "externalLinkBreak", "index": 0}));
    assert_eq!(r["converted"], 2);
    // The formulas using link 1 are their values now; the one using link 2 is `[1]` now.
    assert_eq!(formula(&e, "A1"), None);
    assert_eq!(value(&e, "A1"), CellValue::Number(200.0));
    assert_eq!(value(&e, "A2"), CellValue::Number(350.0));
    assert_eq!(formula(&e, "A3").as_deref(), Some("'[1]S'!A1"));
    assert_eq!(value(&e, "A3"), CellValue::Number(7.0));
    assert_eq!(formula(&e, "A4").as_deref(), Some("1+2"));
    let wb = e.workbook().unwrap();
    assert_eq!(wb.external_links.len(), 1);
    assert_eq!(wb.defined_names[0].formula, "100");
    // Saved: one link part left, the formulas still right.
    let out = e.save("xlsx", 0).unwrap();
    let mut again = SheetEngine::new();
    again.open(&out, "a.xlsx").unwrap();
    let wb = again.workbook().unwrap();
    assert_eq!(wb.external_links.len(), 1);
    assert_eq!(wb.external_links[0].target, "Kho.xlsx");
    exec(&mut again, &json!({"cmd": "calculate"}));
    assert_eq!(value(&again, "A3"), CellValue::Number(7.0));
    // Undo brings the link back.
    e.undo().unwrap();
    assert_eq!(e.workbook().unwrap().external_links.len(), 2);
    assert_eq!(formula(&e, "A1").as_deref(), Some("'[1]Bảng giá'!B2*2"));
}

/// The corpus pair `tests/corpus/sheet/external-links.xlsx` (linking workbook) and
/// `external-links-source.xlsx` (its source with new prices), regenerated with
/// `BO_WRITE_CORPUS=1`; the committed linking workbook opens with its saved link values.
#[test]
fn corpus_files() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus/sheet");
    let files = [
        ("external-links.xlsx", linked()),
        ("external-links-source.xlsx", source_bytes()),
    ];
    for (name, bytes) in &files {
        if std::env::var_os("BO_WRITE_CORPUS").is_some() {
            std::fs::write(dir.join(name), bytes).unwrap();
        }
    }
    let Ok(bytes) = std::fs::read(dir.join("external-links.xlsx")) else {
        return;
    };
    let mut e = SheetEngine::new();
    e.open(&bytes, "external-links.xlsx").unwrap();
    exec(&mut e, &json!({"cmd": "calculate"}));
    assert_eq!(value(&e, "A1"), CellValue::Number(200.0));
    assert_eq!(value(&e, "A3"), CellValue::Number(7.0));
    // E1:E2 spills 1, 2; F1:F3 = A1:A2*2 = 400, 700, #N/A.
    assert_eq!(value(&e, "E2"), CellValue::Number(2.0));
    assert_eq!(value(&e, "F2"), CellValue::Number(700.0));
    assert_eq!(
        value(&e, "F3"),
        CellValue::Error(bo_model::sheet::CellError::NA)
    );
    let ws = &e.workbook().unwrap().sheets[0];
    assert!(ws.cell_a1("E1").unwrap().formula.as_ref().unwrap().dynamic);
    assert!(!ws.cell_a1("F1").unwrap().formula.as_ref().unwrap().dynamic);
}
