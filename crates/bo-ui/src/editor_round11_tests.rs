//! BlueEditor round 11: field mode (select, move, context menu, tab order) and the tabbed
//! field Properties dialog.
use crate::editor_dlg8::FormWhat;
use crate::editor_form8::{Ctl, Val};
use crate::editor_tests::{Fixed, Rec};
use crate::editor_window::EditorWindow;
use crate::Input;
use bo_pdfread::{FieldFormat, FieldKind, PdfDocument};

/// A page with a text field, a check box and a combo box.
fn form_window() -> EditorWindow {
    let on = "<< /Type /XObject /Subtype /Form /BBox [0 0 12 12] /Length 16 >>\nstream\n0 g 2 2 8 8 re f\nendstream";
    let objs = [
        "<< /Type /Catalog /Pages 2 0 R /AcroForm << /Fields [4 0 R 5 0 R 7 0 R] >> >>".to_owned(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_owned(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Annots [4 0 R 5 0 R 7 0 R] >>"
            .to_owned(),
        "<< /FT /Tx /T (name) /Subtype /Widget /Rect [100 700 300 720] /DA (/Helv 12 Tf 0 g) /P 3 0 R >>".to_owned(),
        "<< /FT /Btn /T (ok) /Subtype /Widget /Rect [100 650 112 662] /AS /Off \
         /AP << /N << /Yes 6 0 R /Off 6 0 R >> >> /P 3 0 R >>"
            .to_owned(),
        on.to_owned(),
        "<< /FT /Ch /Ff 131072 /T (city) /Opt [(Hue) (Hanoi)] /Subtype /Widget /Rect [350 700 500 720] /P 3 0 R >>".to_owned(),
    ];
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
    w
}

/// Window point of page point (x, y) (y down from the top).
fn at(w: &EditorWindow, x: f32, y: f32) -> (f32, f32) {
    let r = w
        .view()
        .and_then(|v| v.doc_rect_to_window(0, [x, y, x, y]))
        .expect("on screen");
    (r[0], r[1])
}

fn press(w: &mut EditorWindow, p: (f32, f32), count: u8, shift: bool) {
    w.handle(
        Input::Press {
            x: p.0,
            y: p.1,
            count,
            shift,
        },
        &Fixed,
    );
    w.handle(Input::Release { x: p.0, y: p.1 }, &Fixed);
}

fn field_mode(w: &mut EditorWindow) {
    w.run("fd.text", &Fixed);
    assert!(w.design.tool.is_some());
}

#[test]
fn field_mode_selects_fields_and_double_click_opens_properties() {
    let mut w = form_window();
    field_mode(&mut w);
    let name = at(&w, 150.0, 82.0);
    press(&mut w, name, 1, false);
    assert_eq!(w.f11.selected.len(), 1);
    assert_eq!(w.f11.selected[0].name, "name");
    // Shift adds the combo box; a click on the page clears.
    let q = at(&w, 400.0, 82.0);
    press(&mut w, q, 1, true);
    assert_eq!(w.f11.selected.len(), 2);
    let mut rec = Rec::default();
    w.paint(&mut rec);
    // No new field was drawn by those clicks.
    assert_eq!(w.view().expect("view").doc().form_fields().len(), 3);
    press(&mut w, name, 2, false);
    let (f, what) = w.form8.last().expect("dialog");
    assert!(matches!(what, FormWhat::FieldProps11));
    assert_eq!(f.title, "Text Field Properties");
    assert_eq!(f.size, (660.0, 452.0));
    assert_eq!(f.text("name"), "name");
    // Eight tabs as the reference's text field.
    let tabs = f
        .ctls
        .iter()
        .filter(|c| matches!(c, Ctl::Tab { .. }))
        .count();
    assert_eq!(tabs, 8);
}

#[test]
fn every_page_has_the_reference_size_and_close_applies() {
    let mut w = form_window();
    field_mode(&mut w);
    let q = at(&w, 150.0, 82.0);
    press(&mut w, q, 2, false);
    let sizes = [
        (660.0, 452.0),
        (678.0, 615.0),
        (834.0, 495.0),
        (628.0, 667.0),
        (625.0, 755.0),
        (675.0, 540.0),
        (660.0, 752.0),
        (678.0, 690.0),
    ];
    for (k, size) in sizes.iter().enumerate() {
        if let Some((f, _)) = w.form8.last_mut() {
            f.set("tab", Val::Pick(k));
        }
        assert!(w.fprops11_button("tab"));
        assert_eq!(w.form8.last().expect("dialog").0.size, *size, "page {k}");
    }
    // Change values on several pages, then Close writes them as one step.
    if let Some((f, _)) = w.form8.last_mut() {
        f.set("name", Val::Text("total".into()));
        f.set("tooltip", Val::Text("Sum".into()));
        f.set("required", Val::Pick(1));
        f.set("fcolor", Val::Color([255, 255, 0]));
        f.set("thick", Val::Pick(1));
        f.set("bcolor", Val::Color([0, 0, 0]));
        f.set("fcat", Val::Pick(1));
        f.set("dec", Val::Pick(2));
        f.set("calc", Val::Pick(1));
        f.set("cfields", Val::Text("a, b".into()));
    }
    assert!(w.fprops11_button("close"));
    assert!(w.form8.is_empty());
    let doc = w.view().expect("view").doc();
    let f = doc
        .form_fields()
        .into_iter()
        .find(|f| f.name == "total")
        .expect("renamed");
    let p = doc.field_props(&f);
    assert_eq!(p.tooltip, "Sum");
    assert!(p.required);
    assert_eq!(p.fill_color, Some([1.0, 1.0, 0.0]));
    assert!(matches!(p.format, FieldFormat::Number { decimals: 2, .. }));
    assert!(w.view_mut().expect("view").doc_mut().undo());
}

#[test]
fn position_page_moves_and_sizes_in_the_chosen_unit() {
    let mut w = form_window();
    field_mode(&mut w);
    let q = at(&w, 150.0, 82.0);
    press(&mut w, q, 2, false);
    if let Some((f, _)) = w.form8.last_mut() {
        f.set("tab", Val::Pick(2));
    }
    w.fprops11_button("tab");
    let f = &w.form8.last().expect("dialog").0;
    // Inches by default, as the reference: left 100 pt = 1.39 in.
    assert_eq!(f.text("pl"), "1.39");
    assert_eq!(f.text("pw"), "2.78");
    // Width 3 in keeps the left edge.
    if let Some((f, _)) = w.form8.last_mut() {
        f.set("pw", Val::Text("3".into()));
    }
    w.fprops11_after_input(1);
    assert!((w.f11.rect[2] - (100.0 + 216.0)).abs() < 0.01);
    assert_eq!(w.form8.last().expect("dialog").0.text("pr"), "4.39");
    // Points.
    if let Some((f, _)) = w.form8.last_mut() {
        f.set("units", Val::Pick(0));
    }
    w.fprops11_after_input(1);
    assert_eq!(w.form8.last().expect("dialog").0.text("pl"), "100");
    w.fprops11_button("close");
    let doc = w.view().expect("view").doc();
    let f = doc
        .form_fields()
        .into_iter()
        .find(|f| f.name == "name")
        .expect("f");
    assert_eq!(doc.field_rect(&f), [100.0, 700.0, 316.0, 720.0]);
}

#[test]
fn actions_page_adds_moves_and_deletes_actions() {
    let mut w = form_window();
    field_mode(&mut w);
    let q = at(&w, 150.0, 82.0);
    press(&mut w, q, 2, false);
    if let Some((f, _)) = w.form8.last_mut() {
        f.set("tab", Val::Pick(7));
        f.set("act", Val::Pick(3));
    }
    w.fprops11_button("tab");
    assert!(w.fprops11_button("addAct"));
    assert!(matches!(w.form8.last(), Some((_, FormWhat::Action11(..)))));
    if let Some((f, _)) = w.form8.last_mut() {
        f.set("value", Val::Text("https://example.com".into()));
    }
    assert!(w.fprops11_button("ok"));
    assert_eq!(w.f11.actions.len(), 1);
    // A JavaScript on focus.
    if let Some((f, _)) = w.form8.last_mut() {
        f.set("trig", Val::Pick(4));
        f.set("act", Val::Pick(6));
    }
    w.fprops11_button("addAct");
    if let Some((f, _)) = w.form8.last_mut() {
        f.set("value", Val::Text("app.beep(0);".into()));
    }
    w.fprops11_button("ok");
    assert_eq!(w.f11.actions.len(), 2);
    let rows = w
        .form8
        .last()
        .expect("dialog")
        .0
        .ctls
        .iter()
        .find_map(|c| match c {
            Ctl::ListBox {
                id: "alist", items, ..
            } => Some(items.clone()),
            _ => None,
        })
        .expect("list");
    assert_eq!(rows.len(), 2);
    assert!(rows[0].contains("example.com"));
    if let Some((f, _)) = w.form8.last_mut() {
        f.set("alist", Val::Pick(2));
    }
    w.fprops11_button("actUp");
    assert_eq!(w.f11.actions[0].trigger, "Fo");
    w.fprops11_button("actDelete");
    assert_eq!(w.f11.actions.len(), 1);
    w.fprops11_button("close");
    let doc = w.view().expect("view").doc();
    let f = doc
        .form_fields()
        .into_iter()
        .find(|f| f.name == "name")
        .expect("f");
    assert_eq!(doc.field_props(&f).actions.len(), 1);
}

#[test]
fn context_menu_aligns_sizes_and_orders_tabs() {
    let mut w = form_window();
    field_mode(&mut w);
    let q = at(&w, 150.0, 82.0);
    press(&mut w, q, 1, false);
    let q = at(&w, 400.0, 82.0);
    press(&mut w, q, 1, true);
    // Match width: the combo box takes the text field's 200 pt.
    assert!(w.fields11_command("fd11.sizeW"));
    let doc = w.view().expect("view").doc();
    let city = doc
        .form_fields()
        .into_iter()
        .find(|f| f.name == "city")
        .expect("f");
    assert_eq!(doc.field_rect(&city), [350.0, 700.0, 550.0, 720.0]);
    // Align left.
    assert!(w.fields11_command("fd11.alignLeft"));
    let doc = w.view().expect("view").doc();
    let city = doc
        .form_fields()
        .into_iter()
        .find(|f| f.name == "city")
        .expect("f");
    assert!((doc.field_rect(&city)[0] - 100.0).abs() < 0.01);
    // Tab order by columns, then by hand: combo, then text field.
    assert!(w.fields11_command("fd11.tabColumn"));
    assert_eq!(w.view().expect("view").doc().tab_order(0), "C");
    assert!(w.fields11_command("fd11.tabManual"));
    let q = at(&w, 106.0, 136.0);
    press(&mut w, q, 1, false);
    let q = at(&w, 150.0, 82.0);
    press(&mut w, q, 1, false);
    w.handle(
        Input::Key(crate::Key::Enter, crate::Mods::default()),
        &Fixed,
    );
    let order: Vec<String> = w
        .view()
        .expect("view")
        .doc()
        .form_tab_order()
        .into_iter()
        .map(|f| f.name)
        .collect();
    // The combo box now lies over the text field (same box): the top one is clicked.
    assert_eq!(order[..2], ["ok".to_owned(), "city".to_owned()]);
    // Delete removes the selection.
    let q = at(&w, 106.0, 136.0);
    press(&mut w, q, 1, false);
    assert!(w.fields11_command("fd11.delete"));
    assert_eq!(w.view().expect("view").doc().form_fields().len(), 2);
    // The context menu of a field selects it.
    let p = at(&w, 150.0, 82.0);
    w.handle(Input::Context { x: p.0, y: p.1 }, &Fixed);
    assert_eq!(w.f11.selected.len(), 1);
}

#[test]
fn kinds_have_their_pages() {
    use crate::editor_fields11::{pages, Page};
    assert_eq!(pages(FieldKind::Text).len(), 8);
    assert_eq!(pages(FieldKind::Combo).len(), 8);
    assert_eq!(pages(FieldKind::CheckBox).len(), 5);
    assert!(!pages(FieldKind::Signature).contains(&Page::Options));
}

#[test]
fn moving_a_field_by_dragging() {
    let mut w = form_window();
    field_mode(&mut w);
    let a = at(&w, 150.0, 82.0);
    let b = at(&w, 170.0, 92.0);
    w.handle(
        Input::Press {
            x: a.0,
            y: a.1,
            count: 1,
            shift: false,
        },
        &Fixed,
    );
    w.handle(Input::Drag { x: b.0, y: b.1 }, &Fixed);
    w.handle(Input::Release { x: b.0, y: b.1 }, &Fixed);
    let doc = w.view().expect("view").doc();
    let f = doc
        .form_fields()
        .into_iter()
        .find(|f| f.name == "name")
        .expect("f");
    let r = doc.field_rect(&f);
    assert!(
        (r[0] - 120.0).abs() < 0.5 && (r[1] - 690.0).abs() < 0.5,
        "{r:?}"
    );
}
