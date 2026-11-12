//! Sheet view types and custom views (SHEET-VIEW-001): `sheetView/@view`,
//! `@showFormulas` and the per-view zoom attributes (ECMA-376 Part 1, §18.3.1.87),
//! `customSheetViews` (§18.3.1.26) and `customWorkbookViews` (§18.2.3) survive a
//! write / read / write cycle and are written in schema order.
// Tests may unwrap: a panic is a test failure.
#![allow(clippy::unwrap_used)]

use bo_model::sheet::{
    AutoFilter, CellRange, CellValue, CustomSheetView, CustomWorkbookView, PageMargins, PageSetup,
    Pane, PaneState, SheetView, SheetViewType, Workbook,
};
use bo_opc::{Limits, Package};
use bo_xlsx::{read_xlsx, write_xlsx};

fn part(bytes: &[u8], name: &str) -> String {
    let pkg = Package::read(bytes, &Limits::default()).unwrap();
    String::from_utf8(pkg.part(name).unwrap().data.clone()).unwrap()
}

fn model() -> Workbook {
    let mut wb = Workbook::new();
    let ws = &mut wb.sheets[0];
    ws.sheet_id = 1;
    ws.set_a1("A1", CellValue::Number(1.0));
    let mut view = SheetView::default();
    view.set_view_type(SheetViewType::PageBreakPreview);
    view.set_show_formulas(true);
    view.zoom_scale = Some(60);
    view.set_attr("zoomScaleNormal", Some("90".into()));
    view.set_attr("zoomScaleSheetLayoutView", Some("60".into()));
    ws.views.push(view);
    ws.custom_views.push(CustomSheetView {
        guid: "{11111111-2222-4333-8444-555555555555}".into(),
        scale: Some(75),
        show_formulas: true,
        show_grid_lines: Some(false),
        fit_to_page: true,
        print_area: true,
        filter: true,
        show_auto_filter: true,
        hidden_rows: true,
        session_hidden_rows: Some(vec![(4, 5)]),
        view: Some("pageLayout".into()),
        top_left_cell: Some("B3".into()),
        pane: Some(Pane {
            x_split: 1.0,
            y_split: 2.0,
            top_left_cell: Some("B3".into()),
            active_pane: Some("bottomRight".into()),
            state: PaneState::Frozen,
        }),
        selection: Some(vec![
            ("pane".into(), "bottomRight".into()),
            ("activeCell".into(), "C4".into()),
            ("sqref".into(), "C4".into()),
        ]),
        page_setup: Some(PageSetup {
            margins: Some(PageMargins::default()),
            orientation: Some("landscape".into()),
            scale: Some(80),
            has_page_setup: true,
            print_grid_lines: true,
            has_print_options: true,
            odd_header: Some("&CView".into()),
            row_breaks: vec![9],
            col_breaks: vec![3],
            ..PageSetup::default()
        }),
        auto_filter: Some(AutoFilter {
            range: CellRange::parse("A1:C9").unwrap(),
            ..AutoFilter::default()
        }),
        ..CustomSheetView::default()
    });
    wb.custom_views.push(CustomWorkbookView {
        name: "Báo cáo in".into(),
        guid: "{11111111-2222-4333-8444-555555555555}".into(),
        active_sheet_id: 1,
        window_width: 1000,
        window_height: 600,
        include_print_settings: true,
        include_hidden_row_col: false,
        ..CustomWorkbookView::default()
    });
    wb
}

#[test]
fn sheet_view_type_and_custom_views_round_trip() {
    let wb = model();
    let bytes = write_xlsx(&wb).unwrap();
    let sheet = part(&bytes, "/xl/worksheets/sheet1.xml");
    // §18.3.1.87 attributes are written on the sheet view.
    assert!(sheet.contains("view=\"pageBreakPreview\""), "{sheet}");
    assert!(sheet.contains("showFormulas=\"1\""));
    assert!(sheet.contains("zoomScaleSheetLayoutView=\"60\""));
    // CT_Worksheet order: sheetData … autoFilter? … customSheetViews before mergeCells / pageMargins.
    let csv = sheet.find("<customSheetViews>").unwrap();
    assert!(sheet.find("</sheetData>").unwrap() < csv);
    assert!(csv < sheet.find("<pageMargins").unwrap_or(usize::MAX));
    // CT_CustomSheetView child order: pane, selection, rowBreaks, colBreaks,
    // pageMargins, printOptions, pageSetup, headerFooter, autoFilter.
    let view = &sheet[csv..sheet.find("</customSheetViews>").unwrap()];
    let order = [
        "<pane",
        "<selection",
        "<rowBreaks",
        "<colBreaks",
        "<pageMargins",
        "<printOptions",
        "<pageSetup",
        "<headerFooter",
        "<autoFilter",
    ];
    let pos: Vec<usize> = order.iter().map(|t| view.find(t).unwrap()).collect();
    assert!(pos.windows(2).all(|w| w[0] < w[1]), "{view}");
    // The break before row 11 (index 9 = last row of the page) is `brk id="10"`.
    assert!(view.contains("<brk id=\"10\""));

    let book = part(&bytes, "/xl/workbook.xml");
    // §18.2.27 order: definedNames? calcPr? … customWorkbookViews after sheets.
    let cw = book.find("<customWorkbookViews>").unwrap();
    assert!(book.find("</sheets>").unwrap() < cw);
    assert!(book.contains("includeHiddenRowCol=\"0\""));
    assert!(!book.contains("includePrintSettings"));

    let back = read_xlsx(&bytes).unwrap();
    let ws = &back.sheets[0];
    assert_eq!(ws.views[0].view_type(), SheetViewType::PageBreakPreview);
    assert!(ws.views[0].show_formulas());
    assert_eq!(ws.views[0].zoom_of(SheetViewType::Normal), Some(90));
    assert_eq!(ws.views[0].zoom_of(SheetViewType::PageLayout), None);
    assert_eq!(ws.custom_views, wb.sheets[0].custom_views);
    assert_eq!(back.custom_views, wb.custom_views);
    // Written again byte for byte.
    let again = write_xlsx(&back).unwrap();
    assert_eq!(part(&again, "/xl/worksheets/sheet1.xml"), sheet);
    assert_eq!(part(&again, "/xl/workbook.xml"), book);
}

#[test]
fn custom_view_extension_children_are_kept() {
    let mut wb = model();
    wb.sheets[0].custom_views[0]
        .unknown
        .push(bo_model::sheet::RawXml {
            after: "autoFilter".into(),
            xml: "<extLst><ext uri=\"{00000000-0000-0000-0000-000000000000}\"/></extLst>".into(),
        });
    let bytes = write_xlsx(&wb).unwrap();
    let back = read_xlsx(&bytes).unwrap();
    assert_eq!(back.sheets[0].custom_views[0].unknown.len(), 1);
    assert_eq!(back.sheets[0].custom_views[0].unknown[0].after, "extLst");
}

#[test]
fn custom_view_hidden_rows_cols_and_print_area_travel_as_names() {
    let mut wb = model();
    wb.sheets[0].name = "Bán hàng, Q1".into();
    let cv = &mut wb.sheets[0].custom_views[0];
    cv.session_hidden_rows = Some(vec![(2, 4), (7, 7)]);
    cv.session_hidden_cols = Some(vec![(2, 3)]);
    cv.session_print_area = Some(Some("'Bán hàng, Q1'!$A$1:$D$20".into()));
    let bytes = write_xlsx(&wb).unwrap();
    let book = part(&bytes, "/xl/workbook.xml");
    let plain = book.replace("&apos;", "'");
    // Hidden sheet-scoped names keyed by the GUID (no braces, `_` for `-`).
    assert!(
        plain.contains(
            "<definedName name=\"Z_11111111_2222_4333_8444_555555555555_.wvu.Rows\" \
             localSheetId=\"0\" hidden=\"1\">'Bán hàng, Q1'!$3:$5,'Bán hàng, Q1'!$8:$8</definedName>"
        ),
        "{book}"
    );
    assert!(plain.contains("_.wvu.Cols\" localSheetId=\"0\" hidden=\"1\">'Bán hàng, Q1'!$C:$D<"));
    assert!(plain
        .contains("_.wvu.PrintArea\" localSheetId=\"0\" hidden=\"1\">'Bán hàng, Q1'!$A$1:$D$20<"));
    let back = read_xlsx(&bytes).unwrap();
    // Taken back into the view, not left as defined names.
    assert!(back.defined_names.is_empty(), "{:?}", back.defined_names);
    assert_eq!(back.sheets[0].custom_views, wb.sheets[0].custom_views);
    let again = write_xlsx(&back).unwrap();
    assert_eq!(part(&again, "/xl/workbook.xml"), book);
    // A name of an unknown view stays a defined name.
    let mut other = back.clone();
    other.sheets[0].custom_views[0].guid = "{99999999-2222-4333-8444-555555555555}".into();
    other.custom_views[0].guid = "{99999999-2222-4333-8444-555555555555}".into();
    other.defined_names.push(bo_model::sheet::DefinedName {
        name: "Z_11111111_2222_4333_8444_555555555555_.wvu.Rows".into(),
        formula: "'Bán hàng, Q1'!$3:$3".into(),
        local_sheet: Some(0),
        hidden: true,
        attrs: Vec::new(),
    });
    let back = read_xlsx(&write_xlsx(&other).unwrap()).unwrap();
    assert_eq!(back.defined_names.len(), 1);
}

/// `BO_DUMP_CUSTOM_VIEWS=<file>` writes the sample for `bo-cli validate --openxml require`.
#[test]
fn dump_for_validation() {
    if let Ok(p) = std::env::var("BO_DUMP_CUSTOM_VIEWS") {
        std::fs::write(p, write_xlsx(&model()).unwrap()).unwrap();
    }
}
