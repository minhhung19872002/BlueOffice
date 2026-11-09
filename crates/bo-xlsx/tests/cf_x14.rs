//! Conditional formatting extensions round trip (SHEET-CF-001): 2006 rules linked to
//! `x14:cfRule` by `x14:id`, rules kept only in `x14:conditionalFormattings`
//! ([MS-XLSX] x14 schema), byte stability of the rewritten sheet and of untouched rules.
// Test helpers may unwrap: a panic is the desired failure mode here.
#![allow(clippy::unwrap_used)]

use bo_model::sheet as m;
use bo_opc::{Limits, Package};
use bo_sheet::cf::{add_rule, CfIconSpec, CfRuleSpec, CfvoSpec, DataBarOptions, IconSetOptions};
use bo_xlsx::{read_xlsx, write_xlsx};

const SHEET: &str = "/xl/worksheets/sheet1.xml";

fn sheet_xml(bytes: &[u8]) -> String {
    let pkg = Package::read(bytes, &Limits::default()).unwrap();
    String::from_utf8(pkg.part(SHEET).unwrap().data.clone()).unwrap()
}

fn book() -> m::Workbook {
    let mut wb = m::Workbook::new();
    for (r, v) in [-8.0, -2.0, 0.0, 3.0, 9.0].iter().enumerate() {
        wb.sheets[0].set_cell(r as u32, 0, m::Cell::new(m::CellValue::Number(*v)));
        wb.sheets[0].set_cell(r as u32, 1, m::Cell::new(m::CellValue::Number(*v)));
    }
    wb
}

#[test]
fn written_rules_round_trip_byte_stable() {
    let mut wb = book();
    let auto = |k: &str| CfvoSpec {
        kind: k.into(),
        value: None,
        gte: true,
    };
    assert!(add_rule(
        &mut wb,
        0,
        m::CellRange::parse("A1:A5").unwrap(),
        &CfRuleSpec::DataBar {
            color: "2E6DB4".into(),
            options: Some(DataBarOptions {
                min: auto("autoMin"),
                max: auto("autoMax"),
                min_length: 0,
                max_length: 100,
                gradient: false,
                border: true,
                border_color: Some("1F3F66".into()),
                negative_color: Some("8E3B8E".into()),
                negative_border_color: Some("5A1E5A".into()),
                negative_same_color: false,
                negative_same_border: false,
                axis_position: "middle".into(),
                axis_color: Some("7F7F7F".into()),
                direction: "rightToLeft".into(),
                show_value: true,
            }),
        }
    ));
    assert!(add_rule(
        &mut wb,
        0,
        m::CellRange::parse("B1:B5").unwrap(),
        &CfRuleSpec::IconSet {
            set: "3Stars".into(),
            reverse: false,
            options: Some(IconSetOptions {
                thresholds: vec![
                    CfvoSpec {
                        kind: "num".into(),
                        value: Some("0".into()),
                        gte: false,
                    },
                    CfvoSpec {
                        kind: "formula".into(),
                        value: Some("$A$5/2".into()),
                        gte: true,
                    },
                ],
                show_value: false,
                icons: vec![
                    CfIconSpec {
                        set: "NoIcons".into(),
                        id: 0,
                    },
                    CfIconSpec {
                        set: "3Flags".into(),
                        id: 1,
                    },
                    CfIconSpec {
                        set: "3Stars".into(),
                        id: 2,
                    },
                ],
            }),
        }
    ));
    wb.sheets[0].conditional_formats[1].rules[0].stop_if_true = true;
    let first = write_xlsx(&wb).unwrap();
    let xml = sheet_xml(&first);
    // The data bar is a 2006 rule linked to its x14 part; the star set lives only in x14.
    assert!(xml.contains("<x14:id>"));
    assert!(xml.contains(r#"axisPosition="middle""#));
    assert!(xml.contains(r#"<x14:cfIcon iconSet="NoIcons" iconId="0"/>"#));
    assert_eq!(xml.matches("<conditionalFormatting ").count(), 1);
    let back = read_xlsx(&first).unwrap();
    let cfs = &back.sheets[0].conditional_formats;
    assert_eq!(cfs.len(), 2);
    let db = cfs[0].rules[0].data_bar.as_ref().unwrap();
    let x = db.x14.as_ref().unwrap();
    assert_eq!(x.axis_position.as_deref(), Some("middle"));
    assert_eq!(x.direction.as_deref(), Some("rightToLeft"));
    assert_eq!(x.negative_same_border, Some(false));
    assert_eq!(x.cfvos[0].kind, "autoMin");
    assert_eq!(db.cfvos[0].kind, "min", "2006 part has no autoMin");
    assert!(cfs[0].rules[0].ext_id.is_some());
    assert!(cfs[1].ext);
    let is = cfs[1].rules[0].icon_set.as_ref().unwrap();
    assert!(is.custom && cfs[1].rules[0].stop_if_true);
    assert_eq!(is.cfvos[1].gte, Some(false));
    assert_eq!(is.cfvos[2].val.as_deref(), Some("$A$5/2"));
    assert_eq!(is.icons[1].set, "3Flags");
    // Writing what was read gives the same sheet, and reading it again the same model.
    let second = write_xlsx(&back).unwrap();
    assert_eq!(sheet_xml(&second), xml);
    assert_eq!(read_xlsx(&second).unwrap().sheets[0], back.sheets[0]);
}

#[test]
fn untouched_rules_and_other_extensions_stay_verbatim() {
    let base = write_xlsx(&book()).unwrap();
    let cell_is = r#"<conditionalFormatting sqref="A1:A5"><cfRule type="cellIs" dxfId="0" priority="1" operator="greaterThan"><formula>3</formula></cfRule></conditionalFormatting>"#;
    let other_ext = r#"<ext uri="{CCE6A557-97BC-4b89-ADB6-D9C93CAAB3DF}" xmlns:x14="http://schemas.microsoft.com/office/spreadsheetml/2009/9/main"><x14:dataValidations count="1" xmlns:xm="http://schemas.microsoft.com/office/excel/2006/main"><x14:dataValidation type="list" allowBlank="1"><x14:formula1><xm:f>$C$1:$C$3</xm:f></x14:formula1><xm:sqref>B1:B5</xm:sqref></x14:dataValidation></x14:dataValidations></ext>"#;
    let src = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheetData><row r="1"><c r="A1"><v>-8</v></c></row><row r="2"><c r="A2"><v>4</v></c></row></sheetData>{cell_is}<conditionalFormatting sqref="B1:B5"><cfRule type="dataBar" priority="2"><dataBar><cfvo type="min"/><cfvo type="max"/><color rgb="FF638EC6"/></dataBar><extLst><ext uri="{{B025F937-C7B1-47D3-B67F-A62EFF666E3E}}" xmlns:x14="http://schemas.microsoft.com/office/spreadsheetml/2009/9/main"><x14:id>{{AAAAAAAA-0000-0000-0000-000000000001}}</x14:id></ext></extLst></cfRule></conditionalFormatting><extLst><ext uri="{{78C0D931-6437-407d-A8EE-F0AAD7539E65}}" xmlns:x14="http://schemas.microsoft.com/office/spreadsheetml/2009/9/main"><x14:conditionalFormattings><x14:conditionalFormatting xmlns:xm="http://schemas.microsoft.com/office/excel/2006/main"><x14:cfRule type="dataBar" id="{{AAAAAAAA-0000-0000-0000-000000000001}}"><x14:dataBar minLength="0" maxLength="100" gradient="0" axisPosition="middle"><x14:cfvo type="autoMin"/><x14:cfvo type="autoMax"/><x14:fillColor rgb="FF638EC6"/><x14:negativeFillColor rgb="FFFF0000"/><x14:axisColor rgb="FF000000"/></x14:dataBar></x14:cfRule><xm:sqref>B1:B5</xm:sqref></x14:conditionalFormatting></x14:conditionalFormattings></ext>{other_ext}</extLst></worksheet>"#
    );
    let mut pkg = Package::read(&base, &Limits::default()).unwrap();
    pkg.put_part(SHEET, src.into_bytes());
    let input = pkg.write().unwrap();
    let wb = read_xlsx(&input).unwrap();
    let ws = &wb.sheets[0];
    let x = ws.conditional_formats[1].rules[0]
        .data_bar
        .as_ref()
        .unwrap()
        .x14
        .as_ref()
        .unwrap();
    assert_eq!(x.gradient, Some(false));
    assert_eq!(
        x.negative_fill_color.as_ref().unwrap().rgb.as_deref(),
        Some("FFFF0000")
    );
    let out = sheet_xml(&write_xlsx(&wb).unwrap());
    // The untouched 2006 rule and the other extension are written exactly as read.
    assert!(out.contains(cell_is), "{out}");
    assert!(out.contains(other_ext), "{out}");
    // The linked pair keeps its id; the CF extension comes first in extLst.
    assert!(out.contains("<x14:id>{AAAAAAAA-0000-0000-0000-000000000001}</x14:id>"));
    let cf_ext = out.find("{78C0D931-6437-407d-A8EE-F0AAD7539E65}").unwrap();
    let dv_ext = out.find("{CCE6A557-97BC-4b89-ADB6-D9C93CAAB3DF}").unwrap();
    assert!(cf_ext < dv_ext);
    // Second generation is identical.
    let again = sheet_xml(&write_xlsx(&read_xlsx(&write_xlsx(&wb).unwrap()).unwrap()).unwrap());
    assert_eq!(again, out);
}
