//! Printing settings (SHEET-PRINT-001): `printOptions` (§18.3.1.70),
//! `pageMargins` (§18.3.1.62), `pageSetup` attributes (§18.3.1.63) and every
//! `headerFooter` child (§18.3.1.46) survive a write / read / write cycle.
// Tests may unwrap: a panic is a test failure.
#![allow(clippy::unwrap_used)]

use bo_model::sheet::{CellValue, PageMargins, Workbook};
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
    ws.set_a1("A1", CellValue::Number(1.0));
    ws.props.fit_to_page = Some(true);
    ws.props.present = true;
    ws.print_titles = Some("Sheet1!$1:$1,Sheet1!$A:$A".into());
    ws.print_area = Some("Sheet1!$A$1:$B$4,Sheet1!$D$1:$E$4".into());
    let p = &mut ws.page_setup;
    p.print_headings = true;
    p.print_grid_lines = true;
    p.horizontal_centered = true;
    p.vertical_centered = true;
    p.margins = Some(PageMargins {
        header: 0.4,
        footer: 0.5,
        ..PageMargins::default()
    });
    p.fit_to_width = Some(2);
    p.fit_to_height = Some(0);
    p.attrs = vec![
        ("pageOrder".into(), "overThenDown".into()),
        ("firstPageNumber".into(), "3".into()),
        ("useFirstPageNumber".into(), "1".into()),
        ("blackAndWhite".into(), "1".into()),
        ("draft".into(), "1".into()),
        ("cellComments".into(), "atEnd".into()),
        ("errors".into(), "dash".into()),
    ];
    p.odd_header = Some("&L&\"Arial,Bold\"&14A & B&&C&R&D &T".into());
    p.odd_footer = Some("&CTrang &P / &N".into());
    p.even_header = Some("&R&A".into());
    p.even_footer = Some("&L&F".into());
    p.first_header = Some("&CBìa".into());
    p.first_footer = Some("&C&Z&F".into());
    p.different_odd_even = true;
    p.different_first = true;
    p.header_footer_attrs = vec![("scaleWithDoc".into(), "0".into())];
    p.row_breaks = vec![1];
    p.col_breaks = vec![0];
    wb
}

#[test]
fn print_settings_round_trip() {
    let wb = model();
    let bytes = write_xlsx(&wb).unwrap();
    if let Ok(out) = std::env::var("BO_PAGE_SETUP_OUT") {
        std::fs::write(out, &bytes).unwrap();
    }
    let xml = sheet_part(&bytes);
    // CT_HeaderFooter children in schema order, with the switches as attributes.
    let hf = &xml[xml.find("<headerFooter").unwrap()..xml.find("</headerFooter>").unwrap()];
    assert!(hf.contains("differentOddEven=\"1\"") && hf.contains("differentFirst=\"1\""));
    let order = [
        "oddHeader",
        "oddFooter",
        "evenHeader",
        "evenFooter",
        "firstHeader",
        "firstFooter",
    ]
    .map(|n| hf.find(&format!("<{n}>")).unwrap());
    assert!(order.windows(2).all(|w| w[0] < w[1]), "{hf}");
    // `&` is escaped in XML.
    assert!(hf.contains("A &amp; B&amp;&amp;C"));
    let back = read_xlsx(&bytes).unwrap();
    let (a, b) = (&wb.sheets[0], &back.sheets[0]);
    assert_eq!(a.page_setup.odd_header, b.page_setup.odd_header);
    assert_eq!(a.page_setup.first_footer, b.page_setup.first_footer);
    assert_eq!(a.page_setup.even_header, b.page_setup.even_header);
    assert!(b.page_setup.different_odd_even && b.page_setup.different_first);
    assert!(b.page_setup.header_footer_raw.is_none());
    assert_eq!(
        b.page_setup.header_footer_attrs,
        a.page_setup.header_footer_attrs
    );
    for k in [
        "pageOrder",
        "firstPageNumber",
        "useFirstPageNumber",
        "blackAndWhite",
        "draft",
        "cellComments",
        "errors",
    ] {
        let get = |p: &bo_model::sheet::PageSetup| p.attrs.iter().find(|(n, _)| n == k).cloned();
        assert_eq!(get(&a.page_setup), get(&b.page_setup), "{k}");
    }
    assert_eq!(b.page_setup.margins, a.page_setup.margins);
    assert_eq!(
        (b.page_setup.fit_to_width, b.page_setup.fit_to_height),
        (Some(2), Some(0))
    );
    assert_eq!(b.props.fit_to_page, Some(true));
    assert!(
        b.page_setup.print_headings
            && b.page_setup.horizontal_centered
            && b.page_setup.vertical_centered
    );
    assert_eq!(b.print_area, a.print_area);
    assert_eq!(b.print_titles, a.print_titles);
    assert_eq!(
        (
            b.page_setup.row_breaks.clone(),
            b.page_setup.col_breaks.clone()
        ),
        (vec![1], vec![0])
    );
    // Writing again gives the same sheet XML.
    assert_eq!(sheet_part(&write_xlsx(&back).unwrap()), xml);
}
