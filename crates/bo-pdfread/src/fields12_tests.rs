//! Round 12 (BlueEditor): copying, pasting, multiplying and duplicating form fields.
use super::*;
use crate::standard_fonts;
use crate::tests::build_pdf;

fn open(pages: usize) -> PdfDocument {
    let kids: Vec<String> = (0..pages).map(|i| format!("{} 0 R", 8 + i)).collect();
    let pages_obj = format!(
        "<< /Type /Pages /Kids [3 0 R {}] /Count {} >>",
        kids.join(" "),
        pages + 1
    );
    let mut objs = vec![
        "<< /Type /Catalog /Pages 2 0 R /AcroForm << /Fields [5 0 R 6 0 R] /DA (/Helv 0 Tf 0 g) >> >>".to_owned(),
        pages_obj,
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 300] /Annots [5 0 R 6 0 R] >>".to_owned(),
        "<< /Length 0 >>stream\n\nendstream".to_owned(),
        "<< /FT /Tx /T (Name) /Subtype /Widget /Rect [20 250 140 270] /V (Ann) /DA (/Helv 12 Tf 0 g) /F 4 /P 3 0 R /AP << /N 4 0 R >> /AA << /K 7 0 R /Fo 7 0 R >> >>".to_owned(),
        "<< /FT /Ch /Ff 131072 /T (City) /Opt [(Hue) (Hanoi)] /Subtype /Widget /Rect [160 250 280 270] /P 3 0 R >>".to_owned(),
        "<< /S /JavaScript /JS (1) >>".to_owned(),
    ];
    for _ in 0..pages {
        objs.push("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 300] >>".to_owned());
    }
    let refs: Vec<&str> = objs.iter().map(String::as_str).collect();
    PdfDocument::open(build_pdf(&refs), standard_fonts()).expect("opens")
}

fn field(doc: &PdfDocument, name: &str) -> FormField {
    doc.form_fields()
        .into_iter()
        .find(|f| f.name == name)
        .expect("field")
}

fn names(doc: &PdfDocument) -> Vec<String> {
    let mut v: Vec<String> = doc.form_fields().into_iter().map(|f| f.name).collect();
    v.sort();
    v
}

#[test]
fn copy_and_paste_makes_a_new_field_with_its_own_objects() {
    let mut doc = open(0);
    let clip = doc.copy_field(&field(&doc, "Name")).expect("clip");
    assert_eq!(clip.rect, [20.0, 250.0, 140.0, 270.0]);
    let names_new = doc
        .paste_fields(0, std::slice::from_ref(&clip), 10.0, -40.0)
        .expect("paste");
    assert_eq!(names_new, ["Name_1"]);
    let f = field(&doc, "Name_1");
    assert_eq!(doc.field_rect(&f), [30.0, 210.0, 150.0, 230.0]);
    assert_eq!(f.value, "Ann");
    // Its appearance and action are copies, not the original objects.
    let a = doc.doc.get_dictionary(f.widget).expect("w").clone();
    let o = doc
        .doc
        .get_dictionary(field(&doc, "Name").widget)
        .expect("w")
        .clone();
    assert_ne!(a.get(b"AP").ok(), o.get(b"AP").ok());
    // Pasting again takes the next free number; one undo step each.
    doc.paste_fields(0, &[clip], 0.0, -80.0).expect("paste");
    assert_eq!(names(&doc), ["City", "Name", "Name_1", "Name_2"]);
    assert!(doc.undo());
    assert_eq!(names(&doc), ["City", "Name", "Name_1"]);
    // Saved and opened again, the copy is a field.
    let bytes = doc.save().expect("save");
    let again = PdfDocument::open(bytes, standard_fonts()).expect("opens");
    assert_eq!(names(&again), ["City", "Name", "Name_1"]);
}

#[test]
fn multiple_copies_make_a_grid_with_new_or_same_names() {
    let mut doc = open(0);
    let f = field(&doc, "City");
    let n = doc
        .field_copies(&[f], 2, 2, (2.88, 2.88), (0.0, 0.0), false)
        .expect("copies");
    assert_eq!(n, 3);
    assert_eq!(names(&doc), ["City", "City_1", "City_2", "City_3", "Name"]);
    let below = field(&doc, "City_2");
    let r = doc.field_rect(&below);
    assert!((r[3] - (250.0 - 2.88)).abs() < 0.01, "{r:?}");
    assert!(doc.undo());
    assert_eq!(names(&doc), ["City", "Name"]);
    // Same name: widgets of one field (one value).
    let f = field(&doc, "Name");
    let n = doc
        .field_copies(&[f], 1, 3, (0.0, 0.0), (0.0, 0.0), true)
        .expect("copies");
    assert_eq!(n, 2);
    let all: Vec<FormField> = doc
        .form_fields()
        .into_iter()
        .filter(|f| f.name == "Name")
        .collect();
    assert_eq!(all.len(), 3);
    assert!(all
        .iter()
        .all(|f| f.same_field(&all[0]) && f.value == "Ann"));
    // The keystroke script went to the field, the focus one stayed on the widgets.
    let parent = doc.doc.get_dictionary(all[0].field).expect("field");
    assert!(parent
        .get(b"AA")
        .expect("aa")
        .as_dict()
        .expect("d")
        .has(b"K"));
    assert!(doc.undo());
    assert_eq!(names(&doc), ["City", "Name"]);
}

#[test]
fn duplicate_puts_the_same_field_on_other_pages() {
    let mut doc = open(2);
    let f = field(&doc, "Name");
    let n = doc.duplicate_fields(&[f], &[0, 1, 2]).expect("dup");
    assert_eq!(n, 2);
    let pages: Vec<usize> = doc
        .form_fields()
        .into_iter()
        .filter(|f| f.name == "Name")
        .map(|f| f.page)
        .collect();
    assert_eq!(pages, [0, 1, 2]);
    let mut doc2 = PdfDocument::open(doc.save().expect("save"), standard_fonts()).expect("o");
    let f = field(&doc2, "Name");
    doc2.set_field_text(&f, "Bo").expect("set");
    assert!(doc2
        .form_fields()
        .into_iter()
        .filter(|f| f.name == "Name")
        .all(|f| f.value == "Bo"));
    assert!(doc.undo());
    assert_eq!(doc.form_fields().len(), 2);
}
