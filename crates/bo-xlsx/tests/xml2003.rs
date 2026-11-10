//! XML Spreadsheet 2003 reader against the hand-written fixtures in
//! `tests/corpus/sheet/xml2003-*.xml`: expected cell values, formulas (R1C1 → A1),
//! styles, merged cells, sizes, names — and mutations of them that must not panic.
#![allow(clippy::unwrap_used)]

use bo_model::sheet::{
    CellError, CellRange, CellValue, Fill, FormulaKind, SheetColor, SheetState, Workbook,
};
use bo_xlsx::{is_xml_spreadsheet, read_xlsx, read_xml_spreadsheet, write_xlsx};
use proptest::prelude::*;
use std::path::PathBuf;

fn fixture(name: &str) -> Vec<u8> {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus/sheet")
        .join(name);
    std::fs::read(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

fn text(s: &str) -> CellValue {
    CellValue::Text(s.into())
}

fn check_budget(wb: &Workbook) {
    assert_eq!(
        wb.sheets
            .iter()
            .map(|s| s.name.as_str())
            .collect::<Vec<_>>(),
        ["Ngân sách", "Tham_so", "Ẩn"]
    );
    assert!(!wb.date1904);
    let s = &wb.sheets[0];
    let v = |a1: &str| {
        s.cell_a1(a1)
            .unwrap_or_else(|| panic!("{a1} missing"))
            .value
            .clone()
    };
    let f = |a1: &str| {
        s.cell_a1(a1)
            .unwrap()
            .formula
            .as_ref()
            .map(|f| f.text.clone())
    };
    // Values and types.
    assert_eq!(v("A1"), text("Khoản mục"));
    assert_eq!(v("A2"), text("Lương & thưởng"));
    assert_eq!(v("B2"), CellValue::Number(120_000_000.0));
    assert_eq!(v("C2"), CellValue::Number(118_500_000.5));
    assert_eq!(v("A4"), text("Tiếp thị (mới)"));
    assert_eq!(v("B5"), CellValue::Number(165_000_000.0));
    assert_eq!(v("A7"), text("Ghi chú gộp ô"));
    assert_eq!(v("D7"), text("xoay"));
    assert_eq!(v("D8"), CellValue::Number(16_500_000.0));
    // Formulas: R1C1 → A1, relative to the cell.
    assert_eq!(f("D2").as_deref(), Some("C2/B2"));
    assert_eq!(f("D4").as_deref(), Some("C4/B4"));
    assert_eq!(f("B5").as_deref(), Some("SUM(B2:B4)"));
    assert_eq!(f("C5").as_deref(), Some("SUM($C$2:$C$4)"));
    assert_eq!(f("D5").as_deref(), Some("IF(B5=0,0,C5/B5)"));
    assert_eq!(f("D8").as_deref(), Some("Thue_suat*$B$5"));
    assert_eq!(f("B2"), None);
    // Merged cells and the hyperlink.
    assert_eq!(s.merges, vec![CellRange::parse("A7:C8").unwrap()]);
    assert_eq!(s.hyperlinks.len(), 1);
    assert_eq!(s.hyperlinks[0].range, CellRange::parse("A7").unwrap());
    assert_eq!(
        s.hyperlinks[0].target.as_deref(),
        Some("https://example.test/bao-cao")
    );
    // Column widths (points → characters) and row heights (points).
    let col = |c: u32| s.col_info(c).unwrap();
    assert!((col(0).width.unwrap() - 22.14).abs() < 0.01);
    assert!((col(1).width.unwrap() - 13.57).abs() < 0.01);
    assert_eq!(col(1).width, col(2).width);
    assert!(s.col_info(3).is_none_or(|c| c.width.is_none()));
    assert!(col(4).hidden);
    assert_eq!(s.rows[&0].height, Some(30.0));
    assert!(s.rows[&0].custom_height);
    assert!(s.rows[&7].hidden);
    assert_eq!(s.format.default_row_height, Some(13.5));
    // Styles.
    let st = &wb.styles;
    assert_eq!(st.fonts[0].name.as_deref(), Some("Arial"));
    assert_eq!(st.fonts[0].size, Some(10.0));
    let xf = |a1: &str| st.xf(s.cell_a1(a1).unwrap().style).unwrap().clone();
    let head = xf("A1");
    let hf = &st.fonts[head.font_id as usize];
    assert!(hf.bold && !hf.italic);
    assert_eq!(hf.size, Some(12.0));
    assert_eq!(hf.color, Some(SheetColor::rgb("FFFFFF")));
    assert_eq!(st.fills[head.fill_id as usize], Fill::solid("1F4E79"));
    let hb = &st.borders[head.border_id as usize];
    assert_eq!(hb.bottom.style.as_deref(), Some("medium"));
    assert_eq!(hb.bottom.color, Some(SheetColor::rgb("1F4E79")));
    assert_eq!(hb.top.style.as_deref(), Some("thin"));
    assert_eq!(hb.left.style.as_deref(), Some("dashed"));
    assert_eq!(hb.right.style.as_deref(), Some("double"));
    let ha = head.alignment.unwrap();
    assert_eq!(ha.horizontal.as_deref(), Some("center"));
    assert_eq!(ha.vertical.as_deref(), Some("center"));
    assert!(ha.wrap_text);
    assert_eq!(st.custom_num_fmt(xf("B2").num_fmt_id), Some("#,##0.00"));
    // The child style inherits the parent's number format and adds its own font and fill.
    let total = xf("B5");
    assert_eq!(st.custom_num_fmt(total.num_fmt_id), Some("#,##0.00"));
    let tf = &st.fonts[total.font_id as usize];
    assert!(tf.bold && tf.italic);
    assert_eq!(tf.underline.as_deref(), Some("single"));
    assert_eq!(st.fills[total.fill_id as usize], Fill::solid("FFF2CC"));
    let pct = xf("D2");
    assert_eq!(st.custom_num_fmt(pct.num_fmt_id), Some("0.00%"));
    let pa = pct.alignment.unwrap();
    assert_eq!(
        (pa.horizontal.as_deref(), pa.indent),
        (Some("right"), Some(1))
    );
    let rot = xf("D7");
    assert_eq!(rot.alignment.unwrap().text_rotation, Some(135));
    let rf = &st.fonts[rot.font_id as usize];
    assert!(rf.strike);
    assert_eq!(rf.name.as_deref(), Some("Times New Roman"));
    assert_eq!(rf.vert_align.as_deref(), Some("superscript"));
    // The row style reaches a cell without a style of its own.
    assert_eq!(st.fills[xf("A7").fill_id as usize], Fill::solid("E2EFDA"));
    // An unstyled cell uses the default format.
    assert_eq!(s.cell_a1("A2").unwrap().style, 0);
    // Second sheet: date, boolean, error, text that looks like a number.
    let p = &wb.sheets[1];
    assert_eq!(p.cell_a1("B2").unwrap().value, CellValue::Number(0.1));
    assert_eq!(p.cell_a1("B3").unwrap().value, CellValue::Number(45_366.0));
    assert_eq!(
        st.custom_num_fmt(st.xf(p.cell_a1("B3").unwrap().style).unwrap().num_fmt_id),
        Some("m/d/yyyy")
    );
    assert_eq!(p.cell_a1("B4").unwrap().value, CellValue::Bool(true));
    assert_eq!(
        p.cell_a1("B5").unwrap().value,
        CellValue::Error(CellError::parse("#DIV/0!"))
    );
    assert_eq!(
        p.cell_a1("B5").unwrap().formula.as_ref().unwrap().text,
        "1/0"
    );
    assert_eq!(p.cell_a1("B6").unwrap().value, text("00123"));
    assert_eq!(wb.sheets[2].state, SheetState::Hidden);
    assert_eq!(wb.sheets[0].state, SheetState::Visible);
    // Names: workbook scope, hidden, sheet scope; print areas are not names.
    let name = |n: &str| wb.defined_names.iter().find(|d| d.name == n).unwrap();
    assert_eq!(name("Thue_suat").formula, "Tham_so!$B$2");
    assert_eq!(name("Thue_suat").local_sheet, None);
    assert_eq!(name("Bang_chi").formula, "'Ngân sách'!$A$2:$C$4");
    assert!(name("Bang_chi").hidden);
    assert_eq!(name("Tong_cuc_bo").formula, "'Ngân sách'!$B$5");
    assert_eq!(name("Tong_cuc_bo").local_sheet, Some(0));
    assert_eq!(wb.defined_names.len(), 3);
}

#[test]
fn budget_fixture_reads_as_expected() {
    let bytes = fixture("xml2003-1-budget.xml");
    assert!(is_xml_spreadsheet(&bytes));
    let wb = read_xml_spreadsheet(&bytes).unwrap();
    check_budget(&wb);
}

#[test]
fn budget_fixture_survives_xlsx() {
    let wb = read_xml_spreadsheet(&fixture("xml2003-1-budget.xml")).unwrap();
    let xlsx = write_xlsx(&wb).unwrap();
    let back = read_xlsx(&xlsx).unwrap();
    check_budget(&back);
    // Keep the converted workbook for `bo-cli validate` (OpenXmlValidator).
    if let Ok(dir) = std::env::var("BO_XML2003_OUT") {
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(PathBuf::from(dir).join("xml2003-1-budget.xlsx"), xlsx).unwrap();
    }
}

#[test]
fn dates_1904_fixture_reads_as_expected() {
    let bytes = fixture("xml2003-2-dates1904.xml");
    assert!(is_xml_spreadsheet(&bytes));
    let wb = read_xml_spreadsheet(&bytes).unwrap();
    assert!(wb.date1904);
    // A repeated sheet name is made unique.
    assert_eq!(wb.sheets[0].name, "Lịch");
    assert_eq!(wb.sheets[1].name, "Lịch (2)");
    let s = &wb.sheets[0];
    assert_eq!(s.cell_a1("B3").unwrap().value, CellValue::Number(1.5));
    assert_eq!(s.cell_a1("C3").unwrap().value, CellValue::Number(42_428.0));
    // The last column exists; the cell after it does not.
    assert_eq!(s.cell_a1("XFD3").unwrap().value, CellValue::Number(1.0));
    assert_eq!(s.cells.keys().filter(|(r, _)| *r == 2).count(), 3);
    // `ss:Span` on a row skips rows; the array formula keeps its range.
    let a7 = s.cell_a1("A7").unwrap();
    assert_eq!(a7.value, CellValue::Number(2.5));
    let f = a7.formula.as_ref().unwrap();
    assert_eq!(f.text, "B3+1");
    assert_eq!(
        f.kind,
        FormulaKind::Array {
            range: "A7:A8".into()
        }
    );
    // Table style for cells without their own; a patterned fill from the child style.
    let st = &wb.styles;
    let b3 = st.xf(s.cell_a1("B3").unwrap().style).unwrap();
    assert_eq!(
        st.custom_num_fmt(b3.num_fmt_id),
        Some("yyyy\\-mm\\-dd\\ hh:mm")
    );
    let c3 = st.xf(s.cell_a1("C3").unwrap().style).unwrap();
    assert_eq!(
        st.custom_num_fmt(c3.num_fmt_id),
        Some("yyyy\\-mm\\-dd\\ hh:mm")
    );
    let Fill::Pattern(p) = &st.fills[c3.fill_id as usize] else {
        panic!("pattern fill")
    };
    assert_eq!(p.pattern.as_deref(), Some("mediumGray"));
    assert_eq!(p.fg, Some(SheetColor::rgb("FF0000")));
    assert_eq!(p.bg, Some(SheetColor::rgb("C0C0C0")));
    let xlsx = write_xlsx(&wb).unwrap();
    assert!(read_xlsx(&xlsx).unwrap().date1904);
    if let Ok(dir) = std::env::var("BO_XML2003_OUT") {
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(PathBuf::from(dir).join("xml2003-2-dates1904.xlsx"), xlsx).unwrap();
    }
}

#[test]
fn hostile_declarations_are_bounded() {
    let wrap = |body: &str| {
        format!(
            "<Workbook xmlns=\"urn:schemas-microsoft-com:office:spreadsheet\" xmlns:ss=\"urn:schemas-microsoft-com:office:spreadsheet\"><Worksheet ss:Name=\"a\"><Table>{body}</Table></Worksheet></Workbook>"
        )
    };
    // Indexes, spans and merges far outside the grid.
    let wb = read_xml_spreadsheet(
        wrap("<Column ss:Index=\"4000000000\" ss:Span=\"4000000000\" ss:Width=\"1e308\"/><Row ss:Index=\"4294967295\" ss:Span=\"4294967295\"><Cell ss:Index=\"4294967295\" ss:MergeAcross=\"4294967295\" ss:MergeDown=\"4294967295\"><Data ss:Type=\"Number\">1e999</Data></Cell></Row><Row ss:Index=\"0\"><Cell ss:Index=\"0\" ss:MergeAcross=\"99999999\" ss:Formula=\"=R[-99999999]C[-99999999]\"><Data ss:Type=\"DateTime\">9999-99-99T99:99:99</Data></Cell></Row>").as_bytes(),
    )
    .unwrap();
    assert!(wb.sheets[0].cells.len() <= 1);
    // A style that is its own ancestor.
    let cyc = "<Workbook xmlns=\"urn:schemas-microsoft-com:office:spreadsheet\" xmlns:ss=\"urn:schemas-microsoft-com:office:spreadsheet\"><Styles><Style ss:ID=\"a\" ss:Parent=\"b\"/><Style ss:ID=\"b\" ss:Parent=\"a\"><Font ss:Bold=\"1\"/></Style></Styles><Worksheet><Table><Row><Cell ss:StyleID=\"a\"><Data ss:Type=\"String\">x</Data></Cell></Row></Table></Worksheet></Workbook>";
    let wb = read_xml_spreadsheet(cyc.as_bytes()).unwrap();
    let xf = wb
        .styles
        .xf(wb.sheets[0].cell_a1("A1").unwrap().style)
        .unwrap();
    assert!(wb.styles.fonts[xf.font_id as usize].bold);
    assert_eq!(wb.sheets[0].name, "Sheet1");
    // Unterminated and deeply nested documents are errors, not panics.
    assert!(read_xml_spreadsheet(wrap("<Row><Cell><Data>").as_bytes()).is_err());
    let deep = wrap(&"<Row>".repeat(2000));
    assert!(read_xml_spreadsheet(deep.as_bytes()).is_err());
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(150))]

    #[test]
    fn mutated_fixtures_never_panic(which in 0usize..2, edits in proptest::collection::vec((any::<u32>(), 0u8..5, 0usize..8), 1..8)) {
        const INJECT: [&str; 8] = [
            "<Row ss:Index=\"1048577\">", "<Cell ss:MergeAcross=\"-1\" ss:Formula=\"=R[1]C[\">",
            "</Table>", "<Style ss:ID=\"Default\" ss:Parent=\"Default\">", "&#0;", "<Data ss:Type=\"DateTime\">",
            "ss:StyleID=\"nope\"", "<NamedRange ss:Name=\"\" ss:RefersTo=\"=#REF!\"/>",
        ];
        let mut data = fixture(["xml2003-1-budget.xml", "xml2003-2-dates1904.xml"][which]);
        for (at, kind, tok) in edits {
            let i = at as usize % data.len().max(1);
            match kind {
                0 => data.truncate(i),
                1 => { if i < data.len() { data[i] ^= 0x21; } }
                2 => { data.splice(i..i, INJECT[tok].bytes()); }
                3 => { let end = (i + 60).min(data.len()); data.drain(i..end); }
                _ => { let end = (i + 200).min(data.len()); let chunk = data[i..end].to_vec(); data.splice(i..i, chunk); }
            }
        }
        let _ = is_xml_spreadsheet(&data);
        if let Ok(wb) = read_xml_spreadsheet(&data) {
            // Whatever was read can be saved.
            prop_assert!(write_xlsx(&wb).is_ok());
        }
    }
}
