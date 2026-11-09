//! What-if scenarios (ECMA-376 Part 1, §18.3.1 `scenarios` / `scenario` / `inputCells`):
//! hand-written SpreadsheetML is read into the model, written in schema order and read back.
// Tests may unwrap: a panic is a test failure.
#![allow(clippy::unwrap_used)]

use bo_model::sheet::{CellRange, CellRef, CellValue, Scenario, ScenarioCell, Scenarios, Workbook};
use bo_opc::{Limits, Package};
use bo_xlsx::{read_xlsx, write_xlsx};

fn sheet_part(bytes: &[u8]) -> String {
    let pkg = Package::read(bytes, &Limits::default()).unwrap();
    String::from_utf8(pkg.part("/xl/worksheets/sheet1.xml").unwrap().data.clone()).unwrap()
}

fn model() -> Workbook {
    let mut wb = Workbook::new();
    if wb.sheets.is_empty() {
        wb.sheets.push(bo_model::sheet::Worksheet::new("Sheet1"));
    }
    let ws = &mut wb.sheets[0];
    ws.set_a1("B1", CellValue::Number(10.0));
    ws.set_a1("B2", CellValue::Number(100.0));
    ws.set_cell(1, 2, bo_model::sheet::Cell::default().with_formula("B1*B2"));
    let cell = |a1: &str, v: &str| ScenarioCell {
        cell: CellRef::parse(a1).unwrap(),
        value: v.into(),
        ..ScenarioCell::default()
    };
    ws.scenarios = Some(Scenarios {
        current: Some(1),
        show: Some(0),
        sqref: CellRange::parse_sqref("C2 D1:D3"),
        items: vec![
            Scenario {
                name: "Thấp & \"xấu\"".into(),
                locked: true,
                hidden: false,
                user: Some("Lan".into()),
                comment: Some("dòng 1\ndòng 2".into()),
                cells: vec![cell("B1", "8"), cell("B2", "50")],
            },
            Scenario {
                name: "Cao".into(),
                locked: false,
                hidden: true,
                user: None,
                comment: None,
                cells: vec![ScenarioCell {
                    num_fmt_id: Some(4),
                    ..cell("B1", "12.5")
                }],
            },
            // No changing cell: not written (the schema needs one).
            Scenario {
                name: "Trống".into(),
                ..Scenario::default()
            },
        ],
    });
    // Sheet protection comes before scenarios, autoFilter after (CT_Worksheet order).
    ws.unknown.push(bo_model::sheet::RawXml {
        after: "sheetProtection".into(),
        xml: r#"<sheetProtection sheet="1" scenarios="1"/>"#.into(),
    });
    ws.auto_filter = Some(bo_model::sheet::AutoFilter {
        range: CellRange::parse("A1:C2").unwrap(),
        ..bo_model::sheet::AutoFilter::default()
    });
    wb
}

#[test]
fn scenarios_round_trip_in_schema_order() {
    let wb = model();
    let bytes = write_xlsx(&wb).unwrap();
    let xml = sheet_part(&bytes);
    let p = |s: &str| {
        xml.find(s)
            .unwrap_or_else(|| panic!("{s} missing in {xml}"))
    };
    assert!(p("<sheetProtection") < p("<scenarios"));
    assert!(p("<scenarios") < p("<autoFilter"));
    assert!(xml.contains(r#"<scenarios current="1" show="0" sqref="C2 D1:D3">"#));
    assert!(xml.contains(
        r#"<scenario name="Thấp &amp; &quot;xấu&quot;" locked="1" count="2" user="Lan""#
    ));
    assert!(xml.contains(r#"<inputCells r="B1" val="8"/><inputCells r="B2" val="50"/>"#));
    assert!(xml.contains(r#"<scenario name="Cao" hidden="1" count="1">"#));
    assert!(xml.contains(r#"<inputCells r="B1" val="12.5" numFmtId="4"/>"#));
    assert!(!xml.contains("Trống"));

    let back = read_xlsx(&bytes).unwrap();
    let mut expected = wb.sheets[0].scenarios.clone().unwrap();
    expected.items.pop();
    assert_eq!(back.sheets[0].scenarios.as_ref(), Some(&expected));
    // Nothing of the element is left over as unknown XML.
    assert!(back.sheets[0]
        .unknown
        .iter()
        .all(|u| !u.xml.contains("scenario") || u.xml.starts_with("<sheetProtection")));
    // A second save is identical.
    assert_eq!(sheet_part(&write_xlsx(&back).unwrap()), xml);

    if let Ok(dir) = std::env::var("BO_SCENARIO_OUT") {
        std::fs::write(std::path::Path::new(&dir).join("scenarios.xlsx"), &bytes).unwrap();
    }
}

#[test]
fn reads_attribute_defaults_and_absolute_refs() {
    let wb = model();
    let bytes = write_xlsx(&wb).unwrap();
    let mut pkg = Package::read(&bytes, &Limits::default()).unwrap();
    let xml = sheet_part(&bytes);
    let start = xml.find("<scenarios").unwrap();
    let end = xml.find("</scenarios>").unwrap() + "</scenarios>".len();
    let hand = r#"<scenarios><scenario name="A" locked="true" count="1"><inputCells r="$C$4" val="x" deleted="1" undone="1"/></scenario></scenarios>"#;
    let new_xml = format!("{}{hand}{}", &xml[..start], &xml[end..]);
    pkg.add_part(
        "/xl/worksheets/sheet1.xml",
        "application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml",
        new_xml.into_bytes(),
    )
    .unwrap();
    let back = read_xlsx(&pkg.write().unwrap()).unwrap();
    let s = back.sheets[0].scenarios.as_ref().unwrap();
    assert_eq!(s.current, None);
    assert_eq!(s.sqref, Vec::<CellRange>::new());
    assert_eq!(s.items.len(), 1);
    let a = &s.items[0];
    assert!(a.locked && !a.hidden);
    assert_eq!(a.user, None);
    assert_eq!(
        a.cells,
        [ScenarioCell {
            cell: CellRef::parse("C4").unwrap(),
            value: "x".into(),
            deleted: true,
            undone: true,
            num_fmt_id: None,
        }]
    );
}
