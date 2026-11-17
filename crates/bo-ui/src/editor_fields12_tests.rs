//! BlueEditor round 12: the field context menu's rows and clipboard, Create Multiple Copies,
//! Duplicate, new-field defaults and the rights-management message.
use crate::editor_dlg8::FormWhat;
use crate::editor_fields12::Dlg12;
use crate::editor_tests::{Fixed, Rec};
use crate::editor_window::EditorWindow;
use crate::Input;
use bo_pdfread::PdfDocument;

/// `pages` pages, the first with a text field and a combo box.
fn form_window(pages: usize) -> EditorWindow {
    let mut objs = vec![
        "<< /Type /Catalog /Pages 2 0 R /AcroForm << /Fields [4 0 R 5 0 R] >> >>".to_owned(),
        format!(
            "<< /Type /Pages /Kids [3 0 R{}] /Count {pages} >>",
            (1..pages).fold(String::new(), |mut s, i| {
                s.push_str(&format!(" {} 0 R", 5 + i));
                s
            })
        ),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Annots [4 0 R 5 0 R] >>".to_owned(),
        "<< /FT /Tx /T (name) /Subtype /Widget /Rect [100 700 300 720] /DA (/Helv 12 Tf 0 g) /P 3 0 R >>".to_owned(),
        "<< /FT /Ch /Ff 131072 /T (city) /Opt [(Hue) (Hanoi)] /Subtype /Widget /Rect [350 700 500 720] /P 3 0 R >>".to_owned(),
    ];
    for _ in 1..pages {
        objs.push("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] >>".to_owned());
    }
    let mut out = b"%PDF-1.7\n".to_vec();
    let mut offsets = Vec::new();
    for (i, o) in objs.iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n{o}\nendobj\n", i + 1).as_bytes());
    }
    let xref = out.len();
    out.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", objs.len() + 1).as_bytes());
    for off in offsets {
        out.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            objs.len() + 1
        )
        .as_bytes(),
    );
    let doc = PdfDocument::open(out, bo_pdfread::standard_fonts()).expect("pdf");
    let mut w = EditorWindow::new("en-US");
    w.handle(Input::Scale(1.25), &Fixed);
    w.handle(
        Input::Resize {
            width: 1500.0,
            height: 1100.0,
        },
        &Fixed,
    );
    w.open(doc, "form.pdf", None);
    w.run("fd.text", &Fixed);
    w
}

fn at(w: &EditorWindow, x: f32, y: f32) -> (f32, f32) {
    let r = w
        .view()
        .and_then(|v| v.doc_rect_to_window(0, [x, y, x, y]))
        .expect("on screen");
    (r[0], r[1])
}

fn select_name(w: &mut EditorWindow) {
    let p = at(w, 150.0, 82.0);
    w.handle(
        Input::Press {
            x: p.0,
            y: p.1,
            count: 1,
            shift: false,
        },
        &Fixed,
    );
    w.handle(Input::Release { x: p.0, y: p.1 }, &Fixed);
    assert_eq!(w.f11.selected.len(), 1);
}

fn names(w: &EditorWindow) -> Vec<String> {
    let mut v: Vec<String> = w
        .view()
        .expect("view")
        .doc()
        .form_fields()
        .into_iter()
        .map(|f| f.name)
        .collect();
    v.sort();
    v
}

#[test]
fn context_menu_has_the_reference_rows() {
    let mut w = form_window(1);
    let p = at(&w, 150.0, 82.0);
    w.handle(Input::Context { x: p.0, y: p.1 }, &Fixed);
    let mut rec = Rec::default();
    w.paint(&mut rec);
    for label in [
        "Properties…",
        "Cut",
        "Copy",
        "Paste",
        "Delete",
        "Align",
        "Center",
        "Distribute",
        "Size",
        "Create Multiple Copies…",
        "Set Tab Order",
        "Duplicate…",
        "Use Current Properties as New Defaults",
    ] {
        assert!(
            rec.texts.iter().any(|t| t == label),
            "{label}: {:?}",
            rec.texts
        );
    }
    assert!(!rec.texts.iter().any(|t| t == "Highlight Fields"));
}

#[test]
fn copy_paste_and_cut_fields() {
    let mut w = form_window(1);
    select_name(&mut w);
    w.handle(
        Input::Key(
            crate::Key::Letter('c'),
            crate::Mods {
                ctrl: true,
                ..crate::Mods::default()
            },
        ),
        &Fixed,
    );
    assert_eq!(w.f12.clip.len(), 1);
    // Paste from the page's context menu: the copy's top-left lands where it opened.
    let q = at(&w, 100.0, 300.0);
    w.handle(Input::Context { x: q.0, y: q.1 }, &Fixed);
    assert!(w.fields12_command("fd12.paste"));
    assert_eq!(names(&w), ["city", "name", "name_1"]);
    let doc = w.view().expect("view").doc();
    let f = doc
        .form_fields()
        .into_iter()
        .find(|f| f.name == "name_1")
        .expect("f");
    let r = doc.field_rect(&f);
    assert!(
        (r[0] - 100.0).abs() < 1.0 && (r[3] - 492.0).abs() < 1.0,
        "{r:?}"
    );
    // The pasted field is selected; Cut removes it and keeps it on the clipboard.
    assert_eq!(w.f11.selected[0].name, "name_1");
    assert!(w.fields12_command("fd12.cut"));
    assert_eq!(names(&w), ["city", "name"]);
    assert_eq!(w.f12.clip[0].name, "name_1");
}

#[test]
fn multiple_copies_dialog_makes_a_grid() {
    let mut w = form_window(1);
    select_name(&mut w);
    assert!(w.fields12_command("fd12.copies"));
    let (f, what) = w.form8.last().expect("dialog");
    assert_eq!(f.size, (545.0, 585.0));
    assert!(matches!(what, FormWhat::R12(Dlg12::Copies)));
    // The preview draws the copies' boxes.
    let mut rec = Rec::default();
    w.paint(&mut rec);
    assert!(w.r12_button("right12"));
    assert!((w.f12.shift.0 - 1.0).abs() < 1e-3);
    w.handle(
        Input::Key(crate::Key::Enter, crate::Mods::default()),
        &Fixed,
    );
    assert_eq!(names(&w), ["city", "name", "name_1", "name_2", "name_3"]);
    assert!(w.form8.is_empty());
}

#[test]
fn duplicate_to_pages_and_defaults_for_new_fields() {
    let mut w = form_window(3);
    select_name(&mut w);
    assert!(w.fields12_command("fd12.duplicate"));
    w.handle(
        Input::Key(crate::Key::Enter, crate::Mods::default()),
        &Fixed,
    );
    let pages: Vec<usize> = w
        .view()
        .expect("view")
        .doc()
        .form_fields()
        .into_iter()
        .filter(|f| f.name == "name")
        .map(|f| f.page)
        .collect();
    assert_eq!(pages, [0, 1, 2]);
    // Defaults: the text field's look goes to the next text field drawn.
    select_name(&mut w);
    let f = w.f11.selected[0].clone();
    let mut p = w.view().expect("view").doc().field_props(&f);
    p.fill_color = Some([1.0, 1.0, 0.0]);
    w.doc_edit(|d| d.set_field_props(&f, &p)).expect("props");
    select_name(&mut w);
    assert!(w.fields12_command("fd12.defaults"));
    w.doc_edit(|d| {
        d.add_field(&bo_pdfread::NewField {
            kind: bo_pdfread::NewFieldKind::Text,
            page: 0,
            rect: [100.0, 400.0, 200.0, 420.0],
            name: "fresh".into(),
            tooltip: String::new(),
            options: Vec::new(),
            required: false,
            read_only: false,
        })
    })
    .expect("add");
    w.apply_defaults12(0, "fresh");
    let doc = w.view().expect("view").doc();
    let fresh = doc
        .form_fields()
        .into_iter()
        .find(|f| f.name == "fresh")
        .expect("fresh");
    assert_eq!(doc.field_props(&fresh).fill_color, Some([1.0, 1.0, 0.0]));
}

#[test]
fn rights_management_row_opens_a_message() {
    let mut w = form_window(1);
    w.run("prot.drm12", &Fixed);
    assert!(matches!(
        w.form8.last(),
        Some((_, FormWhat::R12(Dlg12::Rms)))
    ));
}

/// A closed tab's page images leave the store on the next frame (memory back after closing).
#[test]
fn closing_a_tab_releases_its_page_images() {
    let mut w = crate::editor_tests::window(3);
    let mut store = crate::editor_tests::Store::default();
    while w.prepare(&mut store) {}
    assert!(!store.keys.is_empty());
    w.r.close_tab_now(0);
    w.prepare(&mut store);
    assert!(store.keys.is_empty(), "{:?}", store.keys.keys());
}
