//! Forms, comment editing and redaction.
use super::*;
use crate::tests::build_pdf;

/// A page with a text field, a check box, a two-button radio group and a combo box.
fn form_pdf() -> Vec<u8> {
    let on = "<< /Type /XObject /Subtype /Form /BBox [0 0 12 12] /Length 16 >>\nstream\n0 g 2 2 8 8 re f\nendstream";
    let off =
        "<< /Type /XObject /Subtype /Form /BBox [0 0 12 12] /Length 0 >>\nstream\n\nendstream";
    let content = "BT /F1 12 Tf 20 250 Td (Name:) Tj ET";
    let objs = [
        "<< /Type /Catalog /Pages 2 0 R /AcroForm << /Fields [5 0 R 6 0 R 7 0 R 10 0 R] /DA (/Helv 0 Tf 0 g) >> >>".to_owned(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_owned(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 300] /Contents 4 0 R \
         /Resources << /Font << /F1 11 0 R >> >> /Annots [5 0 R 6 0 R 8 0 R 9 0 R 10 0 R] >>"
            .to_owned(),
        format!("<< /Length {} >>\nstream\n{content}\nendstream", content.len()),
        "<< /FT /Tx /T (name) /Subtype /Widget /Rect [70 240 250 262] /DA (/Helv 12 Tf 0 g) /P 3 0 R >>".to_owned(),
        "<< /FT /Btn /T (agree) /Subtype /Widget /Rect [20 200 32 212] /V /Off /AS /Off \
         /AP << /N << /Yes 12 0 R /Off 13 0 R >> >> /P 3 0 R >>"
            .to_owned(),
        "<< /FT /Btn /Ff 49152 /T (size) /V /Off /Kids [8 0 R 9 0 R] >>".to_owned(),
        "<< /Parent 7 0 R /Subtype /Widget /Rect [20 160 32 172] /AS /Off \
         /AP << /N << /S 12 0 R /Off 13 0 R >> >> /P 3 0 R >>"
            .to_owned(),
        "<< /Parent 7 0 R /Subtype /Widget /Rect [60 160 72 172] /AS /Off \
         /AP << /N << /L 12 0 R /Off 13 0 R >> >> /P 3 0 R >>"
            .to_owned(),
        "<< /FT /Ch /Ff 131072 /T (city) /Subtype /Widget /Rect [20 100 150 120] \
         /Opt [(Ha Noi) <FEFF004800751EBF> [(sg) (Sai Gon)]] /DA (/Helv 0 Tf 0 g) /P 3 0 R >>"
            .to_owned(),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_owned(),
        on.to_owned(),
        off.to_owned(),
    ];
    let refs: Vec<&str> = objs.iter().map(String::as_str).collect();
    build_pdf(&refs)
}

fn open(bytes: Vec<u8>) -> PdfDocument {
    PdfDocument::open(bytes, standard_fonts()).expect("opens")
}

fn text(doc: &mut PdfDocument) -> String {
    let p = doc.render(0, RenderOptions::default()).expect("renders");
    TextLayer::new(p.chars).text()
}

#[test]
fn fields_fill_reset_and_flatten() {
    let mut doc = open(form_pdf());
    let f = doc.form_fields();
    let kinds: Vec<(String, FieldKind)> = f.iter().map(|x| (x.name.clone(), x.kind)).collect();
    assert_eq!(
        kinds,
        [
            ("name".into(), FieldKind::Text),
            ("agree".into(), FieldKind::CheckBox),
            ("size".into(), FieldKind::Radio),
            ("size".into(), FieldKind::Radio),
            ("city".into(), FieldKind::Combo),
        ]
    );
    assert_eq!(f[4].options, ["Ha Noi", "Huế", "Sai Gon"]);
    // Box in page points: y down from the 300 pt page top.
    assert_eq!(f[0].rect, [70.0, 38.0, 250.0, 60.0]);
    doc.set_field_text(&f[0], "Nguyễn Văn A").expect("fills");
    doc.set_field_checked(&f[1], true).expect("checks");
    doc.set_field_checked(&f[3], true).expect("chooses");
    doc.set_field_text(&f[4], "Huế").expect("chooses");
    let g = doc.form_fields();
    assert_eq!(g[0].value, "Nguyễn Văn A");
    assert!(g[1].checked);
    assert!(!g[2].checked && g[3].checked);
    assert_eq!(g[3].value, "L");
    assert_eq!(g[4].value, "Huế");
    // The appearances draw: the typed text is on the page.
    let t = text(&mut doc);
    assert!(t.contains("Nguyễn Văn A") && t.contains("Huế"), "{t}");
    // Saved incrementally and read back.
    let mut again = open(doc.save().expect("saves"));
    assert_eq!(again.form_fields()[0].value, "Nguyễn Văn A");
    assert!(again.form_fields()[1].checked);
    // Choosing the other radio button turns this one off.
    let g = again.form_fields();
    again.set_field_checked(&g[2], true).expect("chooses");
    let g = again.form_fields();
    assert!(g[2].checked && !g[3].checked);
    // Reset clears everything; undo brings it back.
    again.reset_form().expect("resets");
    let g = again.form_fields();
    assert!(g.iter().all(|x| !x.checked) && g[0].value.is_empty());
    assert!(again.undo());
    assert_eq!(again.form_fields()[0].value, "Nguyễn Văn A");
    // Flatten: no fields, the text stays on the page as content.
    assert_eq!(again.flatten_form().expect("flattens"), 5);
    assert_eq!(again.form_fields().len(), 0);
    let t = text(&mut again);
    assert!(t.contains("Nguyễn Văn A"), "{t}");
    let mut flat = open(again.save().expect("saves"));
    assert_eq!(flat.form_fields().len(), 0);
    assert!(text(&mut flat).contains("Huế"));
    // Read-only fields refuse values.
    let mut ro = f[0].clone();
    ro.read_only = true;
    assert!(doc.set_field_text(&ro, "x").is_err());
}

#[test]
fn edit_and_delete_comments() {
    let mut doc = open(form_pdf());
    doc.add_annotation(&NewAnnot {
        kind: AnnotKind::TextBox,
        page: 0,
        rects: vec![[150.0, 200.0, 280.0, 240.0]],
        color: [80, 80, 80],
        author: "A".into(),
        contents: "first".into(),
    })
    .expect("adds");
    doc.add_annotation(&NewAnnot {
        kind: AnnotKind::Note,
        page: 0,
        rects: vec![[10.0, 10.0, 30.0, 30.0]],
        color: [255, 214, 0],
        author: "A".into(),
        contents: "note".into(),
    })
    .expect("adds");
    let list = doc.annotations();
    assert_eq!(list.len(), 2);
    doc.set_annotation_contents(&list[0], "Sửa lại")
        .expect("edits");
    assert_eq!(doc.annotations()[0].contents, "Sửa lại");
    assert!(text(&mut doc).contains("Sửa lại"));
    doc.delete_annotation(&list[1]).expect("deletes");
    let mut again = open(doc.save().expect("saves"));
    let list = again.annotations();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].contents, "Sửa lại");
    assert!(again.undo() || !again.can_undo());
}

#[test]
fn redaction_removes_glyphs_under_the_box_only() {
    let content = "BT /F1 12 Tf 20 250 Td [(Secret) -250 (code 1234)] TJ ET \
                   0 0 1 rg 20 100 50 50 re f 1 0 0 rg 200 100 50 50 re f";
    let objs = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_owned(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_owned(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 300] /Contents 4 0 R \
         /Resources << /Font << /F1 5 0 R >> >> >>"
            .to_owned(),
        format!(
            "<< /Length {} >>\nstream\n{content}\nendstream",
            content.len()
        ),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_owned(),
    ];
    let refs: Vec<&str> = objs.iter().map(String::as_str).collect();
    let mut doc = open(build_pdf(&refs));
    let before = doc
        .render(0, RenderOptions::default())
        .expect("renders")
        .chars;
    let digits: Vec<&TextChar> = before
        .iter()
        .filter(|c| c.text.chars().all(|c| c.is_ascii_digit()))
        .collect();
    let (x0, x1) = (digits[0].rect[0], digits[3].rect[2]);
    let y = digits[0].rect;
    // Black out the digits and the blue square.
    let n = doc
        .apply_redactions(
            0,
            &[[x0 - 0.5, y[1], x1 + 0.5, y[3]], [25.0, 160.0, 40.0, 190.0]],
        )
        .expect("redacts");
    assert_eq!(n, 2);
    let after = doc.render(0, RenderOptions::default()).expect("renders");
    let t = TextLayer::new(after.chars.clone()).text();
    assert!(
        t.contains("Secret") && t.contains("code") && !t.contains("1234"),
        "{t}"
    );
    // The kept glyphs did not move.
    let code_before = before.iter().find(|c| c.text == "c").expect("c").rect;
    let code_after = after.chars.iter().find(|c| c.text == "c").expect("c").rect;
    assert!((code_before[0] - code_after[0]).abs() < 0.01);
    // The blue square is gone, the red one stays; black boxes cover the areas.
    let fills: Vec<bo_render::Rgba> = after
        .list
        .commands
        .iter()
        .filter_map(|c| match c {
            bo_render::Command::Path {
                fill: Some(bo_render::Paint::Solid { color }),
                ..
            } => Some(*color),
            _ => None,
        })
        .collect();
    assert!(!fills.contains(&bo_render::Rgba(0, 0, 255, 255)));
    assert!(fills.contains(&bo_render::Rgba(255, 0, 0, 255)));
    assert_eq!(
        fills
            .iter()
            .filter(|c| **c == bo_render::Rgba(0, 0, 0, 255))
            .count(),
        2
    );
    // Saved: the digits are not in the file at all.
    let saved = doc
        .save_full(SaveOptions {
            compress: false,
            ..SaveOptions::default()
        })
        .expect("saves");
    assert!(!saved.windows(4).any(|w| w == b"1234"));
}

#[test]
fn xfdf_fdf_and_csv_round_trip() {
    let mut doc = open(form_pdf());
    let fields = doc.form_fields();
    let by = |n: &str| fields.iter().find(|f| f.name == n).cloned().expect("field");
    doc.set_field_text(&by("name"), "Ann").expect("fills");
    doc.set_field_checked(&by("agree"), true).expect("checks");
    let large = fields
        .iter()
        .find(|f| f.name == "size" && f.on_state.as_deref() == Some(&b"L"[..]))
        .cloned()
        .expect("radio");
    doc.set_field_checked(&large, true).expect("chooses");
    let note = NewAnnot {
        kind: AnnotKind::Note,
        page: 0,
        rects: vec![[100.0, 100.0, 120.0, 120.0]],
        color: [255, 200, 0],
        author: "me".into(),
        contents: "see <this>".into(),
    };
    doc.add_annotation(&note).expect("adds");
    let xfdf = doc.to_xfdf(true, true, "form.pdf");
    let fdf = doc.fields_fdf();
    let csv = doc.fields_csv();
    assert!(xfdf.contains("<text page=\"0\"") && xfdf.contains("see &lt;this&gt;"));
    assert!(csv.starts_with("name,agree,size,city"), "{csv}");
    for data in [xfdf.as_bytes(), &fdf[..], csv.as_bytes()] {
        let mut fresh = open(form_pdf());
        let rep = fresh.import_data(data, "x").expect("imports");
        assert_eq!(rep.fields, 4, "{}", String::from_utf8_lossy(data));
        let v = fresh.field_values();
        assert_eq!(v[0], ("name".to_owned(), "Ann".to_owned()));
        assert_eq!(v[1], ("agree".to_owned(), "Yes".to_owned()));
        assert_eq!(v[2], ("size".to_owned(), "L".to_owned()));
        // One undo step takes it all back.
        assert!(fresh.undo());
        assert_eq!(fresh.field_values()[0].1, "");
    }
    let mut fresh = open(form_pdf());
    let rep = fresh.import_data(xfdf.as_bytes(), "x").expect("imports");
    assert_eq!(rep.comments, 1);
    let a = fresh.annotations();
    let n = a.iter().find(|a| a.kind == AnnotKind::Note).expect("note");
    assert_eq!(n.contents, "see <this>");
    assert_eq!(n.author, "me");
    let orig = doc.annotations();
    let o = orig
        .iter()
        .find(|a| a.kind == AnnotKind::Note)
        .expect("note");
    assert!((n.rect[0] - o.rect[0]).abs() < 0.5 && (n.rect[1] - o.rect[1]).abs() < 0.5);
}
#[test]
fn hidden_data_calculation_order_tooltips_and_buttons() {
    let mut doc = open(form_pdf());
    let note = NewAnnot {
        kind: AnnotKind::Note,
        page: 0,
        rects: vec![[100.0, 100.0, 120.0, 120.0]],
        color: [255, 200, 0],
        author: "me".into(),
        contents: "x".into(),
    };
    doc.add_annotation(&note).expect("adds");
    let before = doc.hidden_counts();
    assert_eq!(before.comments, 1);
    assert_eq!(before.forms, 5);
    // Calculation order and tooltips.
    let n = doc
        .set_calculation_order(&["city".into(), "name".into(), "nope".into()])
        .expect("orders");
    assert_eq!(n, 2);
    assert_eq!(doc.calculation_order(), ["city", "name"]);
    let name = doc
        .form_fields()
        .into_iter()
        .find(|f| f.name == "name")
        .expect("field");
    doc.set_field_tooltip(&name, "Your name").expect("tip");
    assert_eq!(doc.field_tooltip(&name), "Your name");
    // Hide and show a field, a reset button action.
    assert_eq!(doc.hide_fields(&["name".into()], true).expect("hides"), 1);
    let agree = doc
        .form_fields()
        .into_iter()
        .find(|f| f.name == "agree")
        .expect("field");
    doc.set_button_action(&agree, &ButtonAction::Hide(vec!["name".into()], false))
        .expect("action");
    assert_eq!(
        doc.button_action(&agree),
        Some(ButtonAction::Hide(vec!["name".into()], false))
    );
    // Remove comments only: the fields stay; then everything.
    doc.remove_hidden_data(HiddenData {
        comments: true,
        ..HiddenData::default()
    })
    .expect("removes");
    assert_eq!(doc.annotations(), []);
    assert_eq!(doc.form_fields().len(), 5);
    doc.remove_hidden_data(HiddenData::all()).expect("removes");
    assert_eq!(doc.form_fields(), []);
    let mut again = open(doc.save().expect("saves"));
    assert_eq!(again.hidden_counts().forms, 0);
    assert!(text(&mut again).contains("Name:"));
    // One step each.
    assert!(doc.undo());
    assert_eq!(doc.form_fields().len(), 5);
}

#[test]
fn hidden_layers_are_cut_from_the_content() {
    let content = "/OC /oc1 BDC BT /F1 12 Tf 20 250 Td (Secret) Tj ET EMC BT /F1 12 Tf 20 200 Td (Shown) Tj ET";
    let objs = [
        "<< /Type /Catalog /Pages 2 0 R /OCProperties << /OCGs [5 0 R] /D << /OFF [5 0 R] >> >> >>"
            .to_owned(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_owned(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 300] /Contents 4 0 R \
         /Resources << /Font << /F1 6 0 R >> /Properties << /oc1 5 0 R >> >> >>"
            .to_owned(),
        format!(
            "<< /Length {} >>\nstream\n{content}\nendstream",
            content.len()
        ),
        "<< /Type /OCG /Name (Hidden) >>".to_owned(),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_owned(),
    ];
    let refs: Vec<&str> = objs.iter().map(String::as_str).collect();
    let mut doc = open(build_pdf(&refs));
    assert_eq!(doc.hidden_counts().hidden_layers, 1);
    doc.remove_hidden_data(HiddenData {
        hidden_layers: true,
        ..HiddenData::default()
    })
    .expect("removes");
    let mut again = open(doc.save().expect("saves"));
    let t = text(&mut again);
    assert!(t.contains("Shown") && !t.contains("Secret"), "{t}");
    assert_eq!(again.hidden_counts().hidden_layers, 0);
}
