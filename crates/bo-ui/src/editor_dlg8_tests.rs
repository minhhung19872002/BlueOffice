//! BlueEditor round 8: the classic dialogs (Header & Footer, Watermark, Background, Bates,
//! Password Protection, Insert, Extract, Split).
use crate::editor_dlg8::{page_groups, ranges_label, FormWhat};
use crate::editor_form8::Val;
use crate::editor_tests::{window, Fixed};
use crate::editor_window::EditorWindow;
use crate::reader_window::ReaderRequest;
use crate::{Input, Key, Mods};

fn paint(w: &mut EditorWindow) -> crate::editor_tests::Rec {
    let mut p = crate::editor_tests::Rec::default();
    w.paint(&mut p);
    p
}

fn press(w: &mut EditorWindow, r: [f32; 4]) {
    w.handle(
        Input::Press {
            x: r[0] + 3.0,
            y: r[1] + 3.0,
            count: 1,
            shift: false,
        },
        &Fixed,
    );
    paint(w);
}

/// Click control `id` (radio `k`) of the top dialog.
fn click(w: &mut EditorWindow, id: &str, k: usize) {
    paint(w);
    let r = {
        let f = &w.form8.last().expect("dialog").0;
        let i = f.index_of(id, k).unwrap_or_else(|| panic!("no {id}"));
        f.rect_of(i).unwrap_or_else(|| panic!("{id} not usable"))
    };
    press(w, r);
}

fn set(w: &mut EditorWindow, id: &'static str, v: Val) {
    w.form8.last_mut().expect("dialog").0.set(id, v);
}

fn enter(w: &mut EditorWindow) {
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
}

fn texts(w: &mut EditorWindow) -> Vec<String> {
    paint(w).texts
}

/// Drawing commands of page 0 (marks add some).
fn cmds(w: &mut EditorWindow) -> usize {
    let v = w.view_mut().expect("view");
    v.doc_mut()
        .render(0, bo_pdfread::RenderOptions::default())
        .expect("renders")
        .list
        .commands
        .len()
}

#[test]
fn page_lists_and_labels() {
    assert_eq!(page_groups("1, 3-5, 9", 6), vec![vec![0], vec![2, 3, 4]]);
    assert_eq!(page_groups("6-4", 6), vec![vec![3, 4, 5]]);
    assert_eq!(page_groups("x, 2", 6), vec![vec![1]]);
    assert_eq!(ranges_label(&[0, 1, 2, 4]), "1-3,5");
}

#[test]
fn header_footer_dialog_has_the_reference_layout_and_writes_the_marks() {
    let mut w = window(3);
    w.run("org.headerFooter", &Fixed);
    // The button opens Add / Remove All, as the reference's.
    w.run("org8.add.headerFooter", &Fixed);
    let t = texts(&mut w);
    for l in [
        "Add Headers and Footers",
        "Saved Settings:",
        "Margin (Inches)",
        "Left Header Text",
        "Center Footer Text",
        "Macros:",
        "Preview Page:",
    ] {
        assert!(t.iter().any(|x| x == l), "{l}: {t:?}");
    }
    let f = &w.form8.last().expect("dialog").0;
    // 1004 x 820 at 125 %.
    assert!((f.frame[2] - 1004.0).abs() < 1.0 && (f.frame[3] - 820.0).abs() < 1.0);
    // Focus the centre footer box, insert the page-number macro from the list.
    click(&mut w, "fc", 0);
    w.handle(Input::Text("Page ".into()), &Fixed);
    click(&mut w, "insert", 0);
    assert_eq!(
        w.form8.last().expect("dialog").0.text("fc"),
        "Page <<page>>"
    );
    // Margin spin: up arrow adds 0.1 in.
    paint(&mut w);
    let r = {
        let f = &w.form8.last().expect("dialog").0;
        f.arrow_of(f.index_of("top", 0).expect("top"), true)
            .expect("arrow")
    };
    press(&mut w, r);
    assert_eq!(w.form8.last().expect("dialog").0.text("top"), "0.60");
    let before = cmds(&mut w);
    enter(&mut w);
    assert!(w.form8.is_empty());
    let after = cmds(&mut w);
    assert!(after > before, "footer written");
    // Remove All takes them away again.
    w.run("org8.remove.headerFooter", &Fixed);
    assert_eq!(cmds(&mut w), before);
}

#[test]
fn watermark_dialog_options_and_ok() {
    let mut w = window(2);
    w.run("org8.add.watermark", &Fixed);
    let t = texts(&mut w);
    for l in [
        "Add Watermark",
        "Source",
        "Appearance",
        "Position",
        "Rotation:",
    ] {
        assert!(t.iter().any(|x| x == l), "{l}: {t:?}");
    }
    // Custom rotation's box is off until Custom is chosen.
    {
        let f = &w.form8.last().expect("dialog").0;
        assert!(f.rect_of(f.index_of("angle", 0).expect("angle")).is_none());
    }
    click(&mut w, "rot", 3);
    {
        let f = &w.form8.last().expect("dialog").0;
        assert!(f.rect_of(f.index_of("angle", 0).expect("angle")).is_some());
    }
    // OK with no text does nothing; with text it writes the watermark behind the page.
    set(&mut w, "text", Val::Text("DRAFT".into()));
    click(&mut w, "loc", 0);
    let before = cmds(&mut w);
    click(&mut w, "ok", 0);
    assert!(w.form8.is_empty());
    assert!(cmds(&mut w) > before);
}

#[test]
fn background_and_bates_dialogs() {
    let mut w = window(2);
    w.run("org8.add.background", &Fixed);
    assert!(texts(&mut w).iter().any(|x| x == "Add Background"));
    // The colour box cycles; opacity from the slider's box.
    click(&mut w, "color", 0);
    set(&mut w, "opacity", Val::Text("60%".into()));
    let before = cmds(&mut w);
    enter(&mut w);
    assert!(cmds(&mut w) > before);
    // Bates: the file list first (this document), then the numbering.
    w.run("org8.add.bates", &Fixed);
    let t = texts(&mut w);
    assert!(t.iter().any(|x| x == "Arrange Your Files"), "{t:?}");
    assert!(t.iter().any(|x| x == "document"), "{t:?}");
    enter(&mut w);
    assert!(matches!(
        w.form8.last().map(|f| &f.1),
        Some(FormWhat::BatesOptions)
    ));
    set(&mut w, "prefix", Val::Text("ACME".into()));
    enter(&mut w);
    assert!(w.form8.is_empty());
}

#[test]
fn password_dialog_needs_a_choice_and_matching_passwords() {
    let mut w = window(1);
    w.run("prot.password", &Fixed);
    let t = texts(&mut w);
    for l in [
        "Password Protection",
        "Document Open Settings",
        "Encrypt Settings",
        "128-bit AES",
    ] {
        assert!(t.iter().any(|x| x == l), "{l}: {t:?}");
    }
    // OK is off until a password is asked for.
    enter(&mut w);
    assert_eq!(w.form8.len(), 1);
    click(&mut w, "openOn", 0);
    click(&mut w, "open", 0);
    w.handle(Input::Text("abc".into()), &Fixed);
    set(&mut w, "open2", Val::Text("abd".into()));
    enter(&mut w);
    assert_eq!(w.form8.len(), 1, "passwords differ: stays open");
    set(&mut w, "open2", Val::Text("abc".into()));
    // Permissions sub-dialog keeps the values typed so far.
    click(&mut w, "restrictOn", 0);
    set(&mut w, "owner", Val::Text("own".into()));
    set(&mut w, "owner2", Val::Text("own".into()));
    click(&mut w, "perm", 0);
    assert_eq!(w.form8.len(), 2);
    click(&mut w, "print", 0);
    enter(&mut w);
    assert_eq!(w.form8.len(), 1);
    assert!(!w.perm8.0);
    assert_eq!(w.form8.last().expect("dialog").0.text("owner"), "own");
    enter(&mut w);
    assert!(w.form8.is_empty());
    let bytes = w.view_mut().expect("view").doc_mut().save().expect("saves");
    assert!(bo_pdfread::PdfDocument::needs_password(&bytes));
}

#[test]
fn insert_blank_pages_where_asked() {
    let mut w = window(3);
    w.run("org.insertBlank", &Fixed);
    assert!(texts(&mut w).iter().any(|x| x == "Insert Blank Page"));
    click(&mut w, "where", 2);
    set(&mut w, "count", Val::Text("2".into()));
    enter(&mut w);
    assert_eq!(w.view().expect("view").page_count(), 5);
    // Insert › From File asks where the picked file's pages go.
    let other = crate::editor_tests::pdf(2);
    w.file_picked("insert", std::path::Path::new("other.pdf"), &other);
    assert!(texts(&mut w).iter().any(|x| x == "Insert Pages"));
    click(&mut w, "where", 3);
    enter(&mut w);
    assert_eq!(w.view().expect("view").page_count(), 7);
}

#[test]
fn extract_pages_modes() {
    let mut w = window(6);
    w.run("org.extract", &Fixed);
    assert!(texts(&mut w).iter().any(|x| x == "Extract Pages"));
    click(&mut w, "range", 1);
    set(&mut w, "pages", Val::Text("1-2,5".into()));
    click(&mut w, "mode", 1);
    click(&mut w, "save", 0);
    enter(&mut w);
    let req = w.take_requests();
    let files = req
        .iter()
        .find_map(|r| match r {
            ReaderRequest::SaveMany(f) => Some(f.len()),
            _ => None,
        })
        .expect("several files");
    assert_eq!(files, 2);
    // Not saved now: each part opens as a new tab; delete after extraction.
    w.run("org.extract", &Fixed);
    click(&mut w, "range", 1);
    set(&mut w, "pages", Val::Text("6".into()));
    click(&mut w, "delete", 0);
    enter(&mut w);
    let req = w.take_requests();
    assert!(req
        .iter()
        .any(|r| matches!(r, ReaderRequest::OpenDocument { .. })));
    assert_eq!(w.view().expect("view").page_count(), 5);
}

#[test]
fn split_document_ways() {
    let mut w = window(6);
    w.run("org.split", &Fixed);
    assert!(texts(&mut w).iter().any(|x| x == "Split Document"));
    set(&mut w, "pages", Val::Text("4".into()));
    let f = w.form8.last().expect("dialog").0.clone();
    assert_eq!(w.split_groups(&f).len(), 2);
    click(&mut w, "by", 2);
    set(&mut w, "range", Val::Text("1, 2-3, 4-6".into()));
    let f = w.form8.last().expect("dialog").0.clone();
    assert_eq!(w.split_groups(&f), vec![vec![0], vec![1, 2], vec![3, 4, 5]]);
    enter(&mut w);
    let req = w.take_requests();
    assert!(req
        .iter()
        .any(|r| matches!(r, ReaderRequest::SaveMany(f) if f.len() == 3)));
}

/// Two paragraphs close together and one far below (Helvetica 12 pt, 15 pt leading).
fn paragraphs() -> EditorWindow {
    let content = "BT /F1 12 Tf 15 TL 72 700 Td (The first paragraph has two) Tj T* (lines of text.) Tj ET \
                   BT /F1 12 Tf 15 TL 72 662 Td (The second one follows) Tj T* (close below it.) Tj ET \
                   BT /F1 12 Tf 72 400 Td (A paragraph far below.) Tj ET";
    let bytes = crate::editor_round7_tests::build(&[
        "<< /Type /Catalog /Pages 2 0 R >>".into(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".into(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R \
         /Resources << /Font << /F1 5 0 R >> >> >>"
            .into(),
        format!(
            "<< /Length {} >>\nstream\n{content}\nendstream",
            content.len()
        ),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".into(),
    ]);
    let doc = bo_pdfread::PdfDocument::open(bytes, bo_pdfread::standard_fonts()).expect("pdf");
    let mut w = EditorWindow::new("en-US");
    w.handle(Input::Scale(1.25), &Fixed);
    w.handle(
        Input::Resize {
            width: 1500.0,
            height: 1100.0,
        },
        &Fixed,
    );
    w.open(doc, "paras.pdf", None);
    w
}

fn blocks(w: &mut EditorWindow) -> Vec<String> {
    let v = w.view_mut().expect("view");
    let objs = v.doc_mut().page_objects(0).expect("objects");
    bo_pdfread::text_blocks(&objs)
        .into_iter()
        .map(|b| b.text)
        .collect()
}

/// Click into paragraph `i` with Edit Text and put the caret at its start / end.
fn caret_in(w: &mut EditorWindow, i: usize, end: bool) {
    if w.edit_tool() != Some(crate::EditTool::Text) {
        w.run("ed.editText", &Fixed);
    }
    let (x, y) = {
        let v = w.view_mut().expect("view");
        let objs = v.doc_mut().page_objects(0).expect("objects");
        let b = bo_pdfread::text_blocks(&objs).remove(i);
        let r = v.doc_rect_to_window(0, b.bbox).expect("on screen");
        (r[0] + 3.0, r[1] + 3.0)
    };
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
    let k = if end { Key::End } else { Key::Home };
    w.handle(
        Input::Key(
            k,
            Mods {
                ctrl: true,
                shift: false,
                alt: false,
            },
        ),
        &Fixed,
    );
}

#[test]
fn backspace_and_delete_join_paragraphs_like_the_reference() {
    let mut w = paragraphs();
    assert_eq!(blocks(&mut w).len(), 3);
    // Backspace at the start of the first paragraph: nothing.
    caret_in(&mut w, 0, false);
    w.handle(Input::Key(Key::Backspace, Mods::default()), &Fixed);
    w.handle(Input::Key(Key::Escape, Mods::default()), &Fixed);
    assert_eq!(blocks(&mut w).len(), 3);
    // At the start of the second: it runs on after the first one's last character.
    caret_in(&mut w, 1, false);
    w.handle(Input::Key(Key::Backspace, Mods::default()), &Fixed);
    w.handle(Input::Text("X".into()), &Fixed);
    w.handle(Input::Key(Key::Escape, Mods::default()), &Fixed);
    let b = blocks(&mut w);
    assert_eq!(b.len(), 2, "{b:?}");
    let joined = b.concat().replace(" ", "");
    assert!(
        joined.contains("text.XThesecondonefollowsclosebelowit."),
        "{b:?}"
    );
    // Delete at the end: the far paragraph is not pulled up (too far below).
    caret_in(&mut w, 0, true);
    w.handle(Input::Key(Key::Delete, Mods::default()), &Fixed);
    w.handle(Input::Key(Key::Escape, Mods::default()), &Fixed);
    assert_eq!(blocks(&mut w).len(), 2);
}

#[test]
fn edit_object_extracts_the_selected_picture() {
    let png = crate::editor_tests::png2x2();
    let pdf = crate::editor_more::picture_pdf(&png).expect("pdf");
    let doc = bo_pdfread::PdfDocument::open(pdf, bo_pdfread::standard_fonts()).expect("pdf");
    let mut w = EditorWindow::new("en-US");
    w.handle(Input::Scale(1.25), &Fixed);
    w.handle(
        Input::Resize {
            width: 1500.0,
            height: 1100.0,
        },
        &Fixed,
    );
    w.open(doc, "pic.pdf", None);
    w.run("ed.editObject", &Fixed);
    w.handle(
        Input::Key(
            Key::Letter('a'),
            Mods {
                ctrl: true,
                shift: false,
                alt: false,
            },
        ),
        &Fixed,
    );
    assert_eq!(w.image_menu8().len(), 2);
    w.run("obj8.extractImage", &Fixed);
    let req = w.take_requests();
    let Some(ReaderRequest::SaveBytes { name, bytes }) = req.last() else {
        panic!("{req:?}");
    };
    assert!(name.ends_with("-p1-image.png"), "{name}");
    assert!(bytes.starts_with(b"\x89PNG"));
}
