//! Round 11: comment tools, Fill & Sign, forms, Read Aloud word highlight, ribbon names.
use super::{decode_sigs, encode_sigs, GripMode, Mark11, SavedSig, FIELD_TINT};
use crate::reader_tests::{pdf, Fixed};
use crate::reader_window::{ReaderRequest, ReaderWindow, Tool};
use crate::{Input, Key, Mods};
use bo_pdfread::{AnnotKind, FieldKind, PdfDocument, SignatureInk};

fn build(objs: &[String]) -> Vec<u8> {
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
    out
}

/// A Letter page with two amounts, a total (SUM) and a check box.
fn form_pdf() -> Vec<u8> {
    let on = "<< /Type /XObject /Subtype /Form /BBox [0 0 12 12] /Length 16 >>\nstream\n0 g 2 2 8 8 re f\nendstream";
    build(&[
        "<< /Type /Catalog /Pages 2 0 R /AcroForm << /Fields [4 0 R 5 0 R 6 0 R 7 0 R] /CO [6 0 R] >> >>".into(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".into(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Annots [4 0 R 5 0 R 6 0 R 7 0 R] >>".into(),
        "<< /FT /Tx /T (a) /Subtype /Widget /Rect [100 700 200 720] /V (2) /P 3 0 R >>".into(),
        "<< /FT /Tx /T (b) /Subtype /Widget /Rect [100 660 200 680] /P 3 0 R >>".into(),
        "<< /FT /Tx /T (total) /Ff 1 /Subtype /Widget /Rect [100 620 200 640] /P 3 0 R \
         /AA << /C << /S /JavaScript /JS (AFSimple_Calculate\\(\"SUM\", new Array \\(\"a\", \"b\"\\)\\);) >> >> >>"
            .into(),
        "<< /FT /Btn /T (ok) /Subtype /Widget /Rect [100 580 112 592] /V /Off /AS /Off /AP << /N << /Yes 8 0 R >> >> /P 3 0 R >>".into(),
        on.into(),
    ])
}

fn window_with(bytes: Vec<u8>) -> ReaderWindow {
    let doc = PdfDocument::open(bytes, bo_pdfread::standard_fonts()).expect("pdf");
    let mut w = ReaderWindow::new("en-US");
    w.handle(Input::Scale(1.25), &Fixed);
    w.handle(
        Input::Resize {
            width: 1500.0,
            height: 1100.0,
        },
        &Fixed,
    );
    w.open(doc, "test.pdf", None);
    w
}

/// Window point of a page point (top-down) on page 1.
fn at(w: &ReaderWindow, x: f32, y: f32) -> (f32, f32) {
    let v = w.view().expect("view");
    let r = v.page_rect(0).expect("page");
    let s = v.px_per_pt();
    (r[0] + x * s, r[1] + y * s)
}

fn click(w: &mut ReaderWindow, (x, y): (f32, f32)) {
    w.handle(
        Input::Press {
            x,
            y,
            count: 1,
            shift: false,
        },
        &Fixed,
    );
    w.handle(Input::Release { x, y }, &Fixed);
}

fn key(w: &mut ReaderWindow, k: Key, shift: bool) {
    w.handle(
        Input::Key(
            k,
            Mods {
                shift,
                ctrl: false,
                alt: false,
            },
        ),
        &Fixed,
    );
}

fn value(w: &ReaderWindow, name: &str) -> String {
    w.view()
        .expect("view")
        .doc()
        .form_fields()
        .into_iter()
        .find(|f| f.name == name)
        .map(|f| f.value)
        .unwrap_or_default()
}

#[test]
fn saved_signatures_survive_their_text_form() {
    let sigs = vec![
        SavedSig {
            ink: SignatureInk::Typed {
                text: "Ann | Lee\\x".into(),
                face: 14,
            },
            initials: false,
            color: [0, 0, 255],
            keep_size: false,
            session: false,
            password: None,
        },
        SavedSig {
            ink: SignatureInk::Strokes(vec![vec![[0.0, 0.5], [1.0, 0.25]], vec![[0.5, 0.5]]]),
            initials: true,
            color: [255, 0, 0],
            keep_size: false,
            session: false,
            password: None,
        },
        SavedSig {
            ink: SignatureInk::Image(vec![1, 2, 3, 250]),
            initials: false,
            color: [0, 0, 0],
            keep_size: false,
            session: false,
            password: None,
        },
    ];
    assert_eq!(decode_sigs(&encode_sigs(&sigs)), sigs);
    // Round 11 Options: keep size survives, discarded signatures are not written; a typed
    // signature in a system font is kept as shapes.
    let more = vec![
        SavedSig {
            ink: SignatureInk::Image(vec![9, 9]),
            initials: true,
            color: [0, 0, 0],
            keep_size: true,
            session: false,
            password: None,
        },
        SavedSig {
            ink: SignatureInk::Shape {
                segs: vec![
                    bo_render::PathSeg::M(0.0, 0.5),
                    bo_render::PathSeg::C(0.25, 0.0, 0.75, 1.0, 1.0, 0.5),
                    bo_render::PathSeg::Z,
                ],
                aspect: 3.5,
            },
            initials: false,
            color: [0, 0, 255],
            keep_size: false,
            session: false,
            password: None,
        },
        SavedSig {
            ink: SignatureInk::Strokes(vec![vec![[0.0, 0.0]]]),
            initials: false,
            color: [0, 0, 0],
            keep_size: false,
            session: true,
            password: None,
        },
    ];
    assert_eq!(decode_sigs(&encode_sigs(&more)), more[..2].to_vec());
    assert_eq!(decode_sigs("garbage\nX|0|000000|y").len(), 0);
}

#[test]
fn fields_are_tinted_typed_in_place_tabbed_and_calculated() {
    let mut w = window_with(form_pdf());
    // Highlight Fields is on at open: the page is drawn with the measured tint.
    assert!(w.tabs4_highlight());
    assert_eq!(w.view().expect("view").field_tint(), Some(FIELD_TINT));
    w.run_command("form.highlight", &Fixed);
    assert_eq!(w.view().expect("view").field_tint(), None);
    w.run_command("form.highlight", &Fixed);
    // Click field b (page top-down y 112..132) and type: the text shows in the field.
    let p = at(&w, 150.0, 122.0);
    click(&mut w, p);
    assert!(w.eleven.edit.as_ref().is_some_and(|e| e.f.name == "b"));
    w.handle(Input::Text("40".into()), &Fixed);
    assert_eq!(w.eleven.edit.as_ref().map(|e| e.text.as_str()), Some("40"));
    // Tab writes it, runs the calculation (total = 2 + 40) and moves on: total is read-only,
    // so the check box is next; Shift+Tab goes back to b with its text selected.
    key(&mut w, Key::Tab, false);
    assert_eq!(value(&w, "b"), "40");
    assert_eq!(value(&w, "total"), "42");
    assert_eq!(w.eleven.focus.as_ref().map(|f| f.name.as_str()), Some("ok"));
    // Space turns the check box on.
    w.handle(Input::Text(" ".into()), &Fixed);
    let ok = w
        .view()
        .expect("view")
        .doc()
        .form_fields()
        .into_iter()
        .find(|f| f.name == "ok")
        .expect("ok");
    assert_eq!(ok.kind, FieldKind::CheckBox);
    assert!(ok.checked);
    key(&mut w, Key::Tab, true);
    let e = w.eleven.edit.clone().expect("editing b");
    assert!(e.all && e.f.name == "b");
    // Typing replaces the selection; Escape drops what was typed.
    w.handle(Input::Text("7".into()), &Fixed);
    key(&mut w, Key::Escape, false);
    assert_eq!(value(&w, "b"), "40");
    // One undo takes the value and its calculation back together.
    assert!(w.view_mut().expect("view").doc_mut().undo());
    assert!(w.view_mut().expect("view").doc_mut().undo());
    assert_eq!(value(&w, "b"), "");
    assert_eq!(value(&w, "total"), "");
}

#[test]
fn fill_and_sign_places_marks_text_and_a_typed_signature() {
    let mut w = window_with(pdf(1));
    w.run_command("prot.fillSign", &Fixed);
    // The reference's tab: gallery, Manage Signatures, Keep Tool Selected, Add Text,
    // Predefined Text, the five marks, Close; the gallery is 506 px wide at 125 %.
    let ids: Vec<&str> = w.accessible_names11().into_iter().map(|c| c.0).collect();
    for id in [
        "fill.gallery",
        "fill.manage",
        "fill.keep",
        "fill.text",
        "menu.fillPredef",
        "fill.check",
        "fill.line",
        "fill.cross",
        "fill.box",
        "fill.dot",
        "fill.close",
    ] {
        assert!(ids.contains(&id), "{id}");
    }
    let g = w.control_rect("fill.gallery").expect("gallery");
    assert!((g[2] - 506.0).abs() < 1.5, "{g:?}");
    // A box mark: one Fill & Sign comment, selected with its corner handle.
    w.run_command("fill.box", &Fixed);
    let p = at(&w, 300.0, 150.0);
    click(&mut w, p);
    let doc = w.view().expect("view").doc();
    let a = doc.annotations();
    assert_eq!(a.len(), 1);
    assert!(doc.is_fill_sign(&a[0]));
    let sel = w.extra.selected.clone().expect("selected");
    assert_eq!(w.grip_mode11(&sel), GripMode::Corner);
    // Add Text typed in place, written when Escape ends it.
    w.run_command("fill.text", &Fixed);
    assert_eq!(w.eleven.mark, Some(Mark11::Text));
    let p = at(&w, 100.0, 200.0);
    click(&mut w, p);
    assert!(w.eleven.typing.is_some(), "typing");
    w.handle(Input::Text("Hello fill".into()), &Fixed);
    assert_eq!(
        w.eleven.typing.as_ref().map(|t| t.2.as_str()),
        Some("Hello fill")
    );
    key(&mut w, Key::Escape, false);
    let doc = w.view().expect("view").doc();
    let a = doc.annotations();
    assert_eq!(a.len(), 2);
    assert_eq!(a[1].contents, "Hello fill");
    assert!(doc.is_fill_sign(&a[1]));
    // Create a typed signature, place it with a click (the tool then goes back).
    w.run_command("fill.newSig", &Fixed);
    assert!(w.eleven.dialog.is_some());
    w.handle(Input::Text("Ann Lee".into()), &Fixed);
    key(&mut w, Key::Enter, false);
    assert!(w.eleven.dialog.is_none());
    assert_eq!(w.eleven.placing, Some(0));
    let reqs = w.take_requests();
    assert!(reqs.iter().any(
        |r| matches!(r, ReaderRequest::Command(c) if c.starts_with("host.sigs11:T|0|000000|2|Ann Lee"))
    ));
    let p = at(&w, 300.0, 250.0);
    click(&mut w, p);
    assert_eq!(w.eleven.placing, None);
    let doc = w.view().expect("view").doc();
    let a = doc.annotations();
    assert_eq!(a.len(), 3);
    assert_eq!(a[2].kind, AnnotKind::Stamp);
    assert!(doc.is_fill_sign(&a[2]));
    // Centred on the click, 24 pt high.
    let r = a[2].rect;
    assert!((f32::midpoint(r[1], r[3]) - 250.0).abs() < 0.5 && (r[3] - r[1] - 24.0).abs() < 0.5);
    // Flatten on save draws them into the page.
    w.run_command("fill.flattenSave", &Fixed);
    w.run_command("file.save", &Fixed);
    assert_eq!(w.view().expect("view").doc().annotations().len(), 0);
    assert!(w.take_requests().contains(&ReaderRequest::Save));
}

#[test]
fn shapes_are_selected_once_drawn_and_notes_hit_by_their_icon() {
    let mut w = window_with(pdf(1));
    w.set_tool(Tool::Rectangle);
    let (a, b) = (at(&w, 100.0, 100.0), at(&w, 200.0, 160.0));
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
    let ann = doc.annotations();
    assert_eq!(ann.len(), 1);
    assert_eq!(ann[0].kind, AnnotKind::Square);
    let (_, style, _) = doc.drawing(&ann[0]).expect("drawing");
    assert!((style.width - 2.0).abs() < 1e-3, "2 pt like the reference");
    assert_eq!(
        w.extra.selected.as_ref().map(|s| s.kind),
        Some(AnnotKind::Square)
    );
    // A sticky note with NoZoom (/F 28): hit by its 16 px icon, not the whole /Rect.
    let note = build(&[
        "<< /Type /Catalog /Pages 2 0 R >>".into(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".into(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Annots [4 0 R] >>".into(),
        "<< /Type /Annot /Subtype /Text /Rect [100 600 160 660] /F 28 /Contents (n) >>".into(),
    ]);
    let w = window_with(note);
    let v = w.view().expect("view");
    let k = 1.0 / (v.zoom() * 96.0 / 72.0);
    let inside_icon = at(&w, 101.0, 133.0);
    let outside_icon = at(&w, 100.0 + 60.0 * k + 6.0, 180.0);
    assert!(w.comment_at(inside_icon.0, inside_icon.1).is_some());
    assert!(w.comment_at(outside_icon.0, outside_icon.1).is_none());
    let a = w.comment_at(inside_icon.0, inside_icon.1).expect("note");
    assert_eq!(w.grip_mode11(&a), GripMode::Frame);
}

#[test]
fn read_aloud_highlights_the_spoken_word() {
    let mut w = window_with(pdf(2));
    w.run_command("read.page", &Fixed);
    assert!(w.reading11());
    // "Page 1 hello world": "hello" is UTF-16 offset 7, 5 long.
    assert!(w.speech_word11(7, 5));
    let (page, a, b) = w.eleven.word.expect("word");
    let text = w
        .view_mut()
        .expect("view")
        .text(page)
        .expect("text")
        .slice(a, b);
    assert_eq!((page, text.as_str()), (0, "hello"));
    assert!(!w.speech_word11(7, 5), "same word: no new frame");
    w.run_command("read.stop", &Fixed);
    assert!(w.eleven.word.is_none());
}

#[test]
fn every_ribbon_command_has_a_name() {
    let mut w = window_with(form_pdf());
    w.run_command("prot.fillSign", &Fixed);
    let mut unnamed = Vec::new();
    for tab in 0..9 {
        w.set_ribbon_tab(tab, &Fixed);
        for (id, name, _) in w.accessible_names11() {
            if name.trim().is_empty() {
                unnamed.push(format!("{tab}:{id}"));
            }
        }
    }
    unnamed.sort();
    unnamed.dedup();
    assert!(unnamed.is_empty(), "{unnamed:?}");
}

#[test]
fn reflow_reads_with_the_keyboard() {
    let mut w = window_with(pdf(3));
    w.run_command("view.reflow", &Fixed);
    w.handle(Input::Move { x: 700.0, y: 600.0 }, &Fixed);
    assert_eq!(w.reflow_page11(), Some(0));
    key(&mut w, Key::Right, false);
    assert_eq!(w.reflow_page11(), Some(1));
    key(&mut w, Key::End, false);
    assert_eq!(w.reflow_page11(), Some(2));
    key(&mut w, Key::Left, false);
    assert_eq!(w.reflow_page11(), Some(1));
    key(&mut w, Key::Home, false);
    assert_eq!(w.reflow_page11(), Some(0));
}

#[test]
fn fill_and_sign_keeps_the_status_bar_in_place() {
    let mut w = window_with(form_pdf());
    let ids = [
        "page.next",
        "page.last",
        "view.single",
        "zoom.out",
        "zoom.in",
    ];
    let before: Vec<_> = ids.iter().map(|id| w.control_rect(id)).collect();
    w.run_command("prot.fillSign", &Fixed);
    assert!(w.control_rect("fill.gallery").is_some());
    let after: Vec<_> = ids.iter().map(|id| w.control_rect(id)).collect();
    assert_eq!(before, after);
}

#[test]
fn comb_field_apply_all_and_keep_tool_follow_the_reference() {
    let mut w = window_with(pdf(1));
    w.run_command("prot.fillSign", &Fixed);
    // Grey at rest, as the reference.
    assert!(!w.enabled("fill.keep"));
    assert!(!w.enabled("fill.applyAll"));
    // Comb Field: one character per cell, Enter ends it.
    w.run_command("fill.comb", &Fixed);
    assert!(w.enabled("fill.keep"));
    let p = at(&w, 100.0, 200.0);
    click(&mut w, p);
    w.handle(Input::Text("AB C".into()), &Fixed);
    key(&mut w, Key::Enter, false);
    assert!(w.eleven.typing.is_none());
    let a = w.view().expect("view").doc().annotations();
    let texts: Vec<&str> = a.iter().map(|x| x.contents.as_str()).collect();
    assert_eq!(texts, ["A", "B", "C"]);
    // 14 pt apart; the space keeps its cell.
    assert!((a[1].rect[0] - a[0].rect[0] - 14.0).abs() < 0.5, "{a:?}");
    assert!((a[2].rect[0] - a[0].rect[0] - 42.0).abs() < 0.5, "{a:?}");
    // A signature placed: Apply All Signatures turns on and flattens the Fill & Sign items.
    w.run_command("fill.newSig", &Fixed);
    w.handle(Input::Text("Ann".into()), &Fixed);
    key(&mut w, Key::Enter, false);
    let p = at(&w, 300.0, 250.0);
    click(&mut w, p);
    assert!(w.enabled("fill.applyAll"));
    w.run_command("fill.applyAll", &Fixed);
    assert_eq!(w.view().expect("view").doc().annotations().len(), 0);
    assert!(!w.enabled("fill.applyAll"));
}

#[test]
fn create_signature_paste_options_and_system_fonts() {
    let mut w = window_with(pdf(1));
    w.run_command("prot.fillSign", &Fixed);
    w.run_command("fill.newSig", &Fixed);
    assert!(w
        .take_requests()
        .contains(&ReaderRequest::Command("host.sigfonts11".into())));
    // A handwriting font of the system comes first and becomes the default.
    let font = include_bytes!("../../../assets/fonts/Caladea-Italic.ttf").to_vec();
    let mut b = b"Script Test\0".to_vec();
    b.extend_from_slice(&font);
    assert!(w.file_picked11("sigfont11", &b));
    assert_eq!(w.faces11()[0], "Script Test");
    w.handle(Input::Text("Ann".into()), &Fixed);
    // Options: discard on close keeps it out of the saved file.
    let d = w.eleven.dialog.as_mut().expect("dialog");
    d.discard = true;
    key(&mut w, Key::Enter, false);
    let s = w.eleven.sigs[0].clone();
    assert!(s.session);
    assert!(matches!(s.ink, SignatureInk::Shape { .. }));
    assert!(!w
        .take_requests()
        .iter()
        .any(|r| matches!(r, ReaderRequest::Command(c) if c.starts_with("host.sigs11:S"))));
    // Paste page asks the host for the clipboard picture.
    w.run_command("fill.newSig", &Fixed);
    w.eleven.dialog.as_mut().expect("dialog").tab = 3;
    let parts = w.dlg_parts11();
    click(&mut w, (parts.browse[0] + 4.0, parts.browse[1] + 4.0));
    assert!(w
        .take_requests()
        .contains(&ReaderRequest::Command("host.sigpaste11".into())));
}

#[test]
fn tab_into_a_filled_field_selects_its_value() {
    // As the reference: entering a text field with Tab selects its whole value (typing replaces
    // it); a click places the caret.
    let mut w = window_with(form_pdf());
    // Click field b, then Shift+Tab: field "a" (value "2") with its value selected.
    let p = at(&w, 150.0, 122.0);
    click(&mut w, p);
    assert!(w.eleven.edit.as_ref().is_some_and(|e| !e.all));
    key(&mut w, Key::Tab, true);
    let e = w.eleven.edit.clone().expect("editing a");
    assert_eq!(e.f.name, "a");
    assert!(e.all && e.text == "2", "{e:?}");
    w.handle(Input::Text("9".into()), &Fixed);
    assert_eq!(w.eleven.edit.as_ref().map(|e| e.text.as_str()), Some("9"));
    // A click into a filled field: caret, nothing selected.
    let p = at(&w, 150.0, 82.0);
    click(&mut w, p);
    let e = w.eleven.edit.clone().expect("editing by click");
    assert!(!e.all);
}
