//! Round 11 (BlueEditor): field properties read back as written, tab order, arranging.
use super::*;
use crate::standard_fonts;
use crate::tests::build_pdf;

fn fields_pdf() -> Vec<u8> {
    build_pdf(&[
        "<< /Type /Catalog /Pages 2 0 R /AcroForm << /Fields [5 0 R 6 0 R 7 0 R] /DA (/Helv 0 Tf 0 g) >> >>",
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 300] /Annots [6 0 R 5 0 R 7 0 R] >>",
        "<< >>",
        "<< /FT /Tx /T (Name) /Subtype /Widget /Rect [20 250 140 270] /V (Ann) /DA (/Helv 12 Tf 0 0 1 rg) /F 4 /P 3 0 R >>",
        "<< /FT /Btn /T (Agree) /Subtype /Widget /Rect [20 200 34 214] /AS /Off /AP << /N << /Yes 4 0 R /Off 4 0 R >> >> /P 3 0 R >>",
        "<< /FT /Ch /Ff 131072 /T (City) /Opt [(Hue) (Hanoi)] /Subtype /Widget /Rect [160 250 280 270] /P 3 0 R >>",
    ])
}

fn open() -> PdfDocument {
    PdfDocument::open(fields_pdf(), standard_fonts()).expect("opens")
}

fn field(doc: &PdfDocument, name: &str) -> FormField {
    doc.form_fields()
        .into_iter()
        .find(|f| f.name == name)
        .expect("field")
}

#[test]
fn reads_the_defaults_of_a_text_field() {
    let doc = open();
    let p = doc.field_props(&field(&doc, "Name"));
    assert_eq!(p.name, "Name");
    assert_eq!(p.font, "Helvetica");
    assert!((p.font_size - 12.0).abs() < 1e-3);
    assert_eq!(p.text_color, [0.0, 0.0, 1.0]);
    assert_eq!(p.rect, [20.0, 250.0, 140.0, 270.0]);
    assert_eq!(p.visibility, 0);
    assert!(p.scroll && p.spell_check && !p.multiline);
    assert_eq!(p.format, FieldFormat::None);
}

#[test]
fn every_tab_round_trips() {
    let mut doc = open();
    let f = field(&doc, "Name");
    let mut p = doc.field_props(&f);
    p.name = "Amount".into();
    p.tooltip = "How much".into();
    p.visibility = 2;
    p.rotation = 90;
    p.required = true;
    p.locked = true;
    p.line_style = 1;
    p.thickness = 2;
    p.border_color = Some([1.0, 0.0, 0.0]);
    p.fill_color = Some([1.0, 1.0, 0.0]);
    p.font = "Courier".into();
    p.font_size = 9.0;
    p.rect = [30.0, 240.0, 150.0, 262.0];
    p.align = 2;
    p.default_value = "0".into();
    p.max_len = 8;
    p.comb = true;
    p.multiline = true;
    p.spell_check = false;
    p.format = FieldFormat::Number {
        decimals: 2,
        sep: 0,
        currency: "$".into(),
        parens: true,
        red: false,
    };
    p.validate = FieldValidate::Range(Some(0.0), Some(100.0));
    p.calculate = FieldCalc::Simple("SUM".into(), vec!["a".into(), "b".into()]);
    p.actions = vec![
        FieldAction {
            trigger: "U".into(),
            kind: FieldActionKind::WebLink("https://example.com".into()),
        },
        FieldAction {
            trigger: "Fo".into(),
            kind: FieldActionKind::JavaScript("app.beep(0);".into()),
        },
    ];
    doc.set_field_props(&f, &p).expect("written");
    let f2 = field(&doc, "Amount");
    let q = doc.field_props(&f2);
    assert_eq!(q, p);
    // One undo step brings the old field back.
    assert!(doc.undo());
    assert_eq!(doc.field_props(&field(&doc, "Name")).tooltip, "");
}

#[test]
fn format_categories_round_trip_and_show_examples() {
    let mut doc = open();
    for fmt in [
        FieldFormat::Percent {
            decimals: 1,
            sep: 2,
        },
        FieldFormat::Date("mm/dd/yyyy".into()),
        FieldFormat::Time(1),
        FieldFormat::Special(2, String::new()),
        FieldFormat::Special(4, "999-AAA".into()),
        FieldFormat::Custom {
            format: "event.value = 1;".into(),
            keystroke: String::new(),
        },
    ] {
        let f = field(&doc, "Name");
        let mut p = doc.field_props(&f);
        p.format = fmt.clone();
        doc.set_field_props(&f, &p).expect("written");
        assert_eq!(doc.field_props(&field(&doc, "Name")).format, fmt);
    }
    assert_eq!(
        format_example(&FieldFormat::Number {
            decimals: 2,
            sep: 0,
            currency: String::new(),
            parens: false,
            red: false
        }),
        "4,123.03"
    );
    assert_eq!(
        format_example(&FieldFormat::Percent {
            decimals: 2,
            sep: 0
        }),
        "412,303.45%"
    );
    assert_eq!(format_example(&FieldFormat::Date("m/d".into())), "11/17");
}

#[test]
fn choice_and_check_options() {
    let mut doc = open();
    let f = field(&doc, "City");
    let mut p = doc.field_props(&f);
    assert_eq!(p.options, vec!["Hue", "Hanoi"]);
    p.options.push("Da Nang".into());
    p.sort = true;
    p.editable = true;
    doc.set_field_props(&f, &p).expect("written");
    let g = field(&doc, "City");
    assert_eq!(g.options, vec!["Da Nang", "Hanoi", "Hue"]);
    assert_eq!(g.kind, FieldKind::Combo);
    assert!(doc.field_props(&g).editable);

    let c = field(&doc, "Agree");
    let mut p = doc.field_props(&c);
    p.export_value = "On".into();
    p.default_on = true;
    doc.set_field_props(&c, &p).expect("written");
    let q = doc.field_props(&field(&doc, "Agree"));
    assert_eq!(q.export_value, "On");
    assert!(q.default_on);
}

#[test]
fn tab_order_by_rows_columns_and_manually() {
    let mut doc = open();
    assert_eq!(doc.tab_order(0), "");
    doc.set_tab_order(0, Some("R")).expect("set");
    assert_eq!(doc.tab_order(0), "R");
    let names = |d: &PdfDocument| -> Vec<String> {
        d.form_tab_order().into_iter().map(|f| f.name).collect()
    };
    assert_eq!(names(&doc), vec!["Name", "City", "Agree"]);
    doc.set_tab_order(0, Some("C")).expect("set");
    assert_eq!(names(&doc), vec!["Name", "Agree", "City"]);
    let order = vec![
        field(&doc, "City"),
        field(&doc, "Agree"),
        field(&doc, "Name"),
    ];
    doc.set_field_order(0, &order).expect("set");
    assert_eq!(doc.tab_order(0), "");
    assert_eq!(names(&doc), vec!["City", "Agree", "Name"]);
}

#[test]
fn moves_and_sizes_several_fields_in_one_step() {
    let mut doc = open();
    let a = field(&doc, "Name");
    let b = field(&doc, "City");
    doc.set_field_rects(&[
        (a.clone(), [10.0, 250.0, 130.0, 270.0]),
        (b.clone(), [10.0, 220.0, 130.0, 240.0]),
    ])
    .expect("moved");
    assert_eq!(
        doc.field_rect(&field(&doc, "Name")),
        [10.0, 250.0, 130.0, 270.0]
    );
    assert_eq!(
        doc.field_rect(&field(&doc, "City")),
        [10.0, 220.0, 130.0, 240.0]
    );
    assert!(doc.undo());
    assert_eq!(
        doc.field_rect(&field(&doc, "City")),
        [160.0, 250.0, 280.0, 270.0]
    );
}

#[test]
fn deletes_a_field_and_undo_brings_it_back() {
    let mut doc = open();
    let f = field(&doc, "City");
    doc.delete_fields(&[f]).expect("deleted");
    assert!(doc.form_fields().iter().all(|f| f.name != "City"));
    assert_eq!(doc.form_fields().len(), 2);
    assert!(doc.undo());
    assert_eq!(doc.form_fields().len(), 3);
}
