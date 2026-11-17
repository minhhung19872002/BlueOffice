//! Round 11: tab order, simple calculations, NoZoom, Fill & Sign signatures and flattening.
use super::*;
use crate::tests::build_pdf;
use crate::{standard_fonts, RenderOptions};

/// Three amounts, a total (SUM) and an average (AVG) in `/CO`, a radio group and a page with
/// `/Tabs` given by `tabs`.
fn calc_pdf(tabs: &str) -> Vec<u8> {
    let on = "<< /Type /XObject /Subtype /Form /BBox [0 0 12 12] /Length 16 >>\nstream\n0 g 2 2 8 8 re f\nendstream";
    let objs = [
        "<< /Type /Catalog /Pages 2 0 R /AcroForm << /Fields [5 0 R 6 0 R 7 0 R 8 0 R 9 0 R 10 0 R] /CO [8 0 R 9 0 R] /DA (/Helv 0 Tf 0 g) >> >>".to_owned(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_owned(),
        format!("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 300] {tabs} /Annots [7 0 R 5 0 R 6 0 R 8 0 R 9 0 R 11 0 R 12 0 R] >>"),
        "<< >>".to_owned(),
        "<< /FT /Tx /T (a) /Subtype /Widget /Rect [20 250 100 270] /V (2) /P 3 0 R >>".to_owned(),
        "<< /FT /Tx /T (b) /Subtype /Widget /Rect [120 250 200 270] /V (3) /P 3 0 R >>".to_owned(),
        "<< /FT /Tx /T (c) /Subtype /Widget /Rect [20 200 100 220] /P 3 0 R >>".to_owned(),
        "<< /FT /Tx /T (total) /Ff 1 /Subtype /Widget /Rect [20 150 100 170] /P 3 0 R \
         /AA << /C << /S /JavaScript /JS (AFSimple_Calculate\\(\"SUM\", new Array \\(\"a\", \"b\", \"c\"\\)\\);) >> >> >>"
            .to_owned(),
        "<< /FT /Tx /T (avg) /Subtype /Widget /Rect [120 150 200 170] /P 3 0 R \
         /AA << /C << /S /JavaScript /JS (AFSimple_Calculate\\(\"AVG\", \"a, b, c\"\\);) >> >> >>"
            .to_owned(),
        "<< /FT /Btn /Ff 49152 /T (pick) /V /Off /Kids [11 0 R 12 0 R] >>".to_owned(),
        "<< /Parent 10 0 R /Subtype /Widget /Rect [20 100 32 112] /AS /Off /AP << /N << /X 13 0 R >> >> /P 3 0 R >>".to_owned(),
        "<< /Parent 10 0 R /Subtype /Widget /Rect [60 100 72 112] /AS /Y /AP << /N << /Y 13 0 R >> >> /P 3 0 R >>".to_owned(),
        on.to_owned(),
    ];
    let refs: Vec<&str> = objs.iter().map(String::as_str).collect();
    build_pdf(&refs)
}

fn open(bytes: Vec<u8>) -> PdfDocument {
    PdfDocument::open(bytes, standard_fonts()).expect("opens")
}

fn value(doc: &PdfDocument, name: &str) -> String {
    doc.form_fields()
        .into_iter()
        .find(|f| f.name == name)
        .map(|f| f.value)
        .unwrap_or_default()
}

#[test]
fn simple_calculations_follow_the_calculation_order() {
    assert_eq!(
        parse_simple_calculate("AFSimple_Calculate(\"PRD\", new Array (\"x\", 'y'));"),
        Some(("PRD".into(), vec!["x".into(), "y".into()]))
    );
    assert_eq!(parse_simple_calculate("event.value = 1;"), None);
    assert_eq!(calc_text(5.0), "5");
    assert_eq!(calc_text(5.0 / 3.0), "1.666667");
    assert_eq!(calc_text(2.5), "2.5");
    let mut doc = open(calc_pdf(""));
    let c = doc.calculations();
    assert_eq!(c.len(), 2);
    assert_eq!(c[0].target, "total");
    assert_eq!(c[1].sources, ["a", "b", "c"]);
    assert_eq!(doc.calculate_fields().expect("calc"), 2);
    // Read-only to the user, written by the calculation; empty fields count as 0 (as the
    // reference's AVG 5 / 3).
    assert_eq!(value(&doc, "total"), "5");
    assert_eq!(value(&doc, "avg"), "1.666667");
    let c_field = doc
        .form_fields()
        .into_iter()
        .find(|f| f.name == "c")
        .expect("c");
    doc.set_field_text(&c_field, "10").expect("set");
    doc.calculate_fields().expect("calc");
    assert_eq!(value(&doc, "total"), "15");
    assert_eq!(value(&doc, "avg"), "5");
    // Nothing changes the second time.
    assert_eq!(doc.calculate_fields().expect("calc"), 0);
    // Saved and opened again, the values stay.
    let bytes = doc.save().expect("saves");
    assert_eq!(value(&open(bytes), "total"), "15");
}

#[test]
fn tab_order_follows_annots_rows_or_columns() {
    let names = |doc: &PdfDocument| -> Vec<String> {
        doc.form_tab_order().into_iter().map(|f| f.name).collect()
    };
    // Unspecified: the /Annots order (c first); read-only total skipped; one stop for the
    // radio group, its chosen button.
    let doc = open(calc_pdf(""));
    assert_eq!(names(&doc), ["c", "a", "b", "avg", "pick"]);
    let pick = doc
        .form_tab_order()
        .into_iter()
        .find(|f| f.name == "pick")
        .expect("pick");
    assert!(pick.checked, "the chosen radio button is the stop");
    // Rows: top to bottom, then left to right.
    assert_eq!(
        names(&open(calc_pdf("/Tabs /R"))),
        ["a", "b", "c", "avg", "pick"]
    );
    // Columns: left to right, then top to bottom.
    assert_eq!(
        names(&open(calc_pdf("/Tabs /C"))),
        ["a", "c", "pick", "b", "avg"]
    );
}

fn blank() -> PdfDocument {
    open(build_pdf(&[
        "<< /Type /Catalog /Pages 2 0 R >>",
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 200] >>",
    ]))
}

#[test]
fn signatures_draw_flatten_and_mark_fill_sign() {
    let mut doc = blank();
    doc.add_signature_stamp(
        0,
        [20.0, 20.0, 140.0, 50.0],
        &SignatureInk::Typed {
            text: "Ann Lee".into(),
            face: 2,
        },
        [0, 0, 0],
        "me",
    )
    .expect("typed");
    doc.add_signature_stamp(
        0,
        [20.0, 80.0, 80.0, 110.0],
        &SignatureInk::Strokes(vec![
            vec![[0.0, 0.5], [0.5, 0.0], [1.0, 1.0]],
            vec![[0.3, 0.3]],
        ]),
        [0, 0, 255],
        "me",
    )
    .expect("drawn");
    let a = doc.annotations();
    assert_eq!(a.len(), 2);
    assert!(a.iter().all(|x| doc.is_fill_sign(x)));
    assert!((a[0].rect[0] - 20.0).abs() < 0.1 && (a[0].rect[3] - 50.0).abs() < 0.1);
    let page = doc.render(0, RenderOptions::default()).expect("renders");
    let text = crate::TextLayer::new(page.chars).text();
    assert!(text.contains("Ann Lee"), "{text}");
    // Flattened: no comments left, the text is in the page content.
    assert_eq!(doc.flatten_fill_sign().expect("flatten"), 2);
    assert_eq!(doc.annotations().len(), 0);
    let saved = doc.save().expect("saves");
    let mut again = open(saved);
    let page = again.render(0, RenderOptions::default()).expect("renders");
    assert!(crate::TextLayer::new(page.chars).text().contains("Ann Lee"));
    // One undo brings the comments back.
    assert!(doc.undo());
    assert_eq!(doc.annotations().len(), 2);
}

#[test]
fn no_zoom_flag_and_typed_width() {
    let mut doc = open(build_pdf(&[
        "<< /Type /Catalog /Pages 2 0 R >>",
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 200] /Annots [4 0 R 5 0 R] >>",
        "<< /Type /Annot /Subtype /Text /Rect [10 170 26 186] /F 28 /Contents (n) >>",
        "<< /Type /Annot /Subtype /Square /Rect [50 50 90 90] /F 4 >>",
    ]));
    let a = doc.annotations();
    assert!(doc.annotation_no_zoom(&a[0]));
    assert!(!doc.annotation_no_zoom(&a[1]));
    assert!(typed_width("Ann", 2) > 1.0 && typed_width("Ann", 2) < 2.5);
    // Marking the newest comment.
    assert_eq!(doc.annot_count(0), 2);
    doc.mark_fill_sign(0, 1).expect("mark");
    let a = doc.annotations();
    assert!(doc.is_fill_sign(&a[1]) && !doc.is_fill_sign(&a[0]));
}

#[test]
fn combo_boxes_get_a_drop_button_on_screen_only() {
    let mut doc = open(build_pdf(&[
        "<< /Type /Catalog /Pages 2 0 R /AcroForm << /Fields [4 0 R 5 0 R] >> >>",
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 200] /Annots [4 0 R 5 0 R] >>",
        "<< /FT /Ch /Ff 131072 /T (country) /Opt [(A) (B)] /V (A) /Subtype /Widget /Rect [20 150 240 170] /F 4 >>",
        "<< /FT /Ch /T (size) /Opt [(S) (M)] /Subtype /Widget /Rect [20 50 140 104] /F 4 >>",
    ]));
    let count = |doc: &mut PdfDocument, tint: bool, printing: bool| {
        let extra = crate::RenderExtra {
            field_tint: tint.then_some(bo_render::Rgba(221, 228, 255, 255)),
            printing,
            ..crate::RenderExtra::default()
        };
        doc.render_extra(0, RenderOptions::default(), extra)
            .expect("renders")
            .list
            .commands
            .len()
    };
    let plain = count(&mut doc, false, false);
    let tinted = count(&mut doc, true, false);
    // Two tints (combo and list box) and the combo's button (face, frame, arrow); the list box
    // gets no button.
    assert_eq!(tinted - plain, 2 + 3, "{plain} {tinted}");
    assert_eq!(count(&mut doc, true, true), plain);
}
