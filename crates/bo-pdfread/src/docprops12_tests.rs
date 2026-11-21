//! Round 12 (BlueEditor): document properties written and read back.
use super::*;
use crate::standard_fonts;
use crate::tests::build_pdf;

fn open() -> PdfDocument {
    PdfDocument::open(
        build_pdf(&[
            "<< /Type /Catalog /Pages 2 0 R >>",
            "<< /Type /Pages /Kids [3 0 R 4 0 R 5 0 R] /Count 3 >>",
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 300] >>",
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 300] >>",
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 300] >>",
        ]),
        standard_fonts(),
    )
    .expect("opens")
}

fn reopen(doc: &mut PdfDocument) -> PdfDocument {
    PdfDocument::open(doc.save().expect("save"), standard_fonts()).expect("opens")
}

#[test]
fn initial_view_round_trips() {
    let mut doc = open();
    assert_eq!(doc.initial_view(), InitialView::default());
    let v = InitialView {
        navigation: 1,
        layout: 4,
        magnification: 3,
        page: 2,
        display_title: true,
        center_window: true,
        hide_toolbars: true,
        ..InitialView::default()
    };
    doc.set_initial_view(&v).expect("set");
    let again = reopen(&mut doc);
    assert_eq!(again.initial_view(), v);
    assert_eq!(again.page_mode().as_deref(), Some("UseOutlines"));
    // Full screen keeps the panel as the non-full-screen mode; a zoom of 150 %.
    let v2 = InitialView {
        navigation: 2,
        full_screen: true,
        magnification: 6 + 5,
        ..InitialView::default()
    };
    doc.set_initial_view(&v2).expect("set");
    assert_eq!(reopen(&mut doc).initial_view(), v2);
    // Undo goes back to the first view.
    assert!(doc.undo());
    assert_eq!(doc.initial_view(), v);
}

#[test]
fn info_entries_and_custom_keys() {
    let mut doc = open();
    doc.set_info("Title", "Báo cáo").expect("title");
    doc.set_info("Project", "BlueOffice").expect("custom");
    assert!(doc.set_info("bad name", "x").is_err());
    let again = reopen(&mut doc);
    assert_eq!(again.doc_info().title, "Báo cáo");
    assert_eq!(
        again.custom_info(),
        [("Project".to_owned(), "BlueOffice".to_owned())]
    );
    doc.set_info("Title", "").expect("clear");
    assert_eq!(doc.doc_info().title, "");
}

#[test]
fn print_presets_round_trip() {
    let mut doc = open();
    let p = PrintPresets {
        scaling: 1,
        duplex: 2,
        copies: 3,
        pages: "1,2-3".into(),
        tray_by_size: true,
        binding: 1,
        language: "vi-VN".into(),
    };
    doc.set_print_presets(&p).expect("set");
    assert_eq!(reopen(&mut doc).print_presets(), p);
    let bad = PrintPresets {
        pages: "4-9".into(),
        ..PrintPresets::default()
    };
    assert!(doc.set_print_presets(&bad).is_err());
    doc.set_print_presets(&PrintPresets::default())
        .expect("clear");
    assert_eq!(doc.print_presets(), PrintPresets::default());
}
