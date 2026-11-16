//! BlueEditor round 9: the Edit Object selection as the reference draws it, side handles.
use crate::editor_rot8::SEL_BLUE;
use crate::editor_tests::{window, Fixed, Rec};
use crate::editor_window::EditorWindow;
use crate::{Input, Key, Mods};

fn paint(w: &mut EditorWindow) -> Rec {
    let mut p = Rec::default();
    w.paint(&mut p);
    p
}

fn select_all_objects(w: &mut EditorWindow) {
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
}

fn text_box(w: &mut EditorWindow) -> ([f32; 4], [f32; 4]) {
    let v = w.view_mut().expect("view");
    let objs = v.doc_mut().page_objects(0).expect("objects");
    let b = objs
        .iter()
        .find(|o| o.kind == bo_pdfread::ObjectKind::Text)
        .map(|o| o.bbox)
        .expect("text");
    (b, v.doc_rect_to_window(0, b).expect("shown"))
}

#[test]
fn edit_object_selection_has_eight_solid_blue_handles() {
    let mut w = window(1);
    select_all_objects(&mut w);
    let p = paint(&mut w);
    let squares: Vec<[f32; 4]> = p
        .rects
        .iter()
        .filter(|(r, c)| *c == SEL_BLUE && (r[2] - 6.0).abs() < 0.01 && (r[3] - 6.0).abs() < 0.01)
        .map(|(r, _)| *r)
        .collect();
    assert_eq!(squares.len(), 8, "{squares:?}");
    // No white inner squares any more (the reference's handles are solid).
    assert!(!p
        .rects
        .iter()
        .any(|(r, c)| *c == crate::Rgba(255, 255, 255, 255) && (r[2] - 5.0).abs() < 0.01));
    // The rotation stalk is dashed in the same blue.
    assert!(p
        .rects
        .iter()
        .any(|(r, c)| *c == SEL_BLUE && (r[2] - 1.0).abs() < 0.01 && r[3] > 10.0));
}

#[test]
fn a_side_handle_stretches_one_direction() {
    let mut w = window(1);
    select_all_objects(&mut w);
    paint(&mut w);
    let (before, r) = text_box(&mut w);
    let (x, y) = (r[0] + r[2], r[1] + r[3] / 2.0);
    w.handle(
        Input::Press {
            x,
            y,
            count: 1,
            shift: false,
        },
        &Fixed,
    );
    w.handle(
        Input::Drag {
            x: x + 60.0,
            y: y + 30.0,
        },
        &Fixed,
    );
    w.handle(
        Input::Release {
            x: x + 60.0,
            y: y + 30.0,
        },
        &Fixed,
    );
    paint(&mut w);
    let (after, _) = text_box(&mut w);
    let (bw, bh) = (before[2] - before[0], before[3] - before[1]);
    let (aw, ah) = (after[2] - after[0], after[3] - after[1]);
    assert!(aw > bw * 1.2, "{before:?} -> {after:?}");
    assert!(
        (ah - bh).abs() < 0.5,
        "height kept: {before:?} -> {after:?}"
    );
    assert!((after[0] - before[0]).abs() < 0.5, "left side stays");
}

/// Click Edit Text into the first paragraph of page 0, at its first character.
fn start_typing(w: &mut EditorWindow) {
    w.run("ed.editText", &Fixed);
    let v = w.view_mut().expect("view");
    let objs = v.doc_mut().page_objects(0).expect("objects");
    let b = bo_pdfread::text_blocks(&objs).remove(0);
    let r = v.doc_rect_to_window(0, b.bbox).expect("on screen");
    let (x, y) = (r[0] + 3.0, r[1] + r[3] / 2.0);
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

#[test]
fn selected_text_is_white_on_the_reference_band_in_the_page_image() {
    use crate::editor_selhi9::{mark_list, MARK_BG, MARK_FG};
    let mut w = window(1);
    start_typing(&mut w);
    crate::editor_tests::key(&mut w, Key::Home, false, false);
    for _ in 0..4 {
        crate::editor_tests::key(&mut w, Key::Right, false, true);
    }
    w.sync_text_mark9();
    let (page, rects) = w
        .view()
        .and_then(|v| v.text_mark9.clone())
        .expect("selection handed to the view");
    assert_eq!((page, rects.len()), (0, 1));
    // The page's list gets the band and the text again in white inside it.
    let list = {
        let v = w.view_mut().expect("view");
        v.doc_mut()
            .render(0, bo_pdfread::RenderOptions::default())
            .expect("renders")
            .list
    };
    let n = list.commands.len();
    let marked = mark_list(list, &rects);
    let added = &marked.commands[n..];
    assert!(matches!(added[0], bo_render::Command::FillRect { color, .. } if color == MARK_BG));
    assert!(added
        .iter()
        .any(|c| matches!(c, bo_render::Command::Glyphs { color, .. } if *color == MARK_FG)));
    assert!(matches!(added.last(), Some(bo_render::Command::PopClip)));
    // No selection: nothing drawn in (the mark goes away).
    crate::editor_tests::key(&mut w, Key::Right, false, false);
    w.sync_text_mark9();
    assert!(w.view().expect("view").text_mark9.is_none());
}

fn block_text(w: &EditorWindow) -> String {
    let t = w.edit.typing.as_ref().expect("typing");
    t.block.as_ref().expect("block").text.iter().collect()
}

#[test]
fn undo_while_typing_takes_one_character_like_the_reference() {
    use crate::editor_tests::key;
    let mut w = window(1);
    start_typing(&mut w);
    key(&mut w, Key::End, false, false);
    for s in [" ", "x", "y", "z"] {
        w.handle(Input::Text(s.into()), &Fixed);
    }
    assert_eq!(block_text(&w), "Page 1 xyz");
    key(&mut w, Key::Letter('z'), true, false);
    assert_eq!(block_text(&w), "Page 1 xy");
    key(&mut w, Key::Backspace, false, false);
    key(&mut w, Key::Backspace, false, false);
    assert_eq!(block_text(&w), "Page 1 ");
    key(&mut w, Key::Letter('z'), true, false);
    assert_eq!(block_text(&w), "Page 1 x");
    // Redo puts the deleted character back off again.
    key(&mut w, Key::Letter('y'), true, false);
    assert_eq!(block_text(&w), "Page 1 ");
}

fn organize(pages: usize) -> EditorWindow {
    let mut w = window(pages);
    w.show_tab(crate::editor_window::TAB_ORGANIZE, &Fixed);
    paint(&mut w);
    w
}

#[test]
fn insert_menu_has_the_reference_rows() {
    let mut w = organize(2);
    w.run("org.insertMenu", &Fixed);
    let t = paint(&mut w).texts;
    for l in [
        "From File",
        "From Folder",
        "From Open Files",
        "From Web Page",
        "From Scanner",
        "From Clipboard",
        "Blank Page",
        "Into File",
    ] {
        assert!(t.iter().any(|x| x == l), "{l}: {t:?}");
    }
    let (rows, seps) = crate::editor_org9::insert_rows();
    assert_eq!((rows.len(), seps), (8, vec![7]));
}

fn page_count(w: &mut EditorWindow) -> usize {
    w.view().expect("view").page_count()
}

#[test]
fn insert_from_open_files_and_from_the_clipboard() {
    let mut w = organize(2);
    let other =
        bo_pdfread::PdfDocument::open(crate::editor_tests::pdf(3), bo_pdfread::standard_fonts())
            .expect("pdf");
    w.open(other, "other.pdf", None);
    w.r.select_tab(0);
    assert_eq!(page_count(&mut w), 2);
    w.run("org9.insertTab.1", &Fixed);
    assert_eq!(page_count(&mut w), 5);
    // From Clipboard: the host is asked for the picture, which becomes a page.
    w.run("org9.insertClipboard", &Fixed);
    let req = w.take_requests();
    assert!(
        matches!(&req[..], [crate::reader_window::ReaderRequest::Command(c)] if c == "host.clipboardImage"),
        "{req:?}"
    );
    let mut dib = vec![0_u8; 40];
    dib[0] = 40;
    dib[4] = 2;
    dib[8] = 1;
    dib[12] = 1;
    dib[14] = 32;
    dib.extend_from_slice(&[0, 0, 255, 255, 0, 255, 0, 255]);
    w.file_picked("clipboardImage", std::path::Path::new("c.bmp"), &dib);
    assert_eq!(page_count(&mut w), 6);
    // No new tab was opened for it (From Clipboard of the Convert tab does that).
    assert_eq!(w.take_requests().len(), 0);
}

#[test]
fn update_replaces_the_watermark_with_the_last_values() {
    use crate::editor_form8::Val;
    let mut w = organize(1);
    // Drawing commands of page 0 (the watermark adds some).
    let marks = |w: &mut EditorWindow| {
        let v = w.view_mut().expect("view");
        v.doc_mut()
            .render(0, bo_pdfread::RenderOptions::default())
            .expect("renders")
            .list
            .commands
            .len()
    };
    let none = marks(&mut w);
    w.run("org.watermark", &Fixed);
    assert!(paint(&mut w).texts.iter().any(|t| t == "Update"));
    w.run("org8.add.watermark", &Fixed);
    w.form8
        .last_mut()
        .expect("dialog")
        .0
        .set("text", Val::Text("DRAFT".into()));
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    let one = marks(&mut w);
    assert!(one > none);
    // Update: the dialog comes back with "DRAFT"; changed to FINAL it replaces the mark.
    w.run("org9.update.watermark", &Fixed);
    assert_eq!(w.form8.last().expect("dialog").0.text("text"), "DRAFT");
    w.form8
        .last_mut()
        .expect("dialog")
        .0
        .set("text", Val::Text("FINAL".into()));
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    assert_eq!(marks(&mut w), one, "replaced, not added");
}

#[test]
fn combine_files_lists_ranges_reorders_and_makes_a_new_document() {
    use crate::editor_form8::Val;
    use crate::reader_window::ReaderRequest;
    let mut w = window(3);
    w.run("conv.combine", &Fixed);
    assert_eq!(w.combine9.len(), 1, "the open document comes first");
    let t = paint(&mut w).texts;
    for l in [
        "Combine Files",
        "Add Files...",
        "Move Up",
        "Page Range",
        "ALL",
    ] {
        assert!(t.iter().any(|x| x == l), "{l}: {t:?}");
    }
    // Add a 2-page file; it is selected.
    w.file_picked(
        "combine9",
        std::path::Path::new("two.pdf"),
        &crate::editor_tests::pdf(2),
    );
    assert_eq!(w.combine9.len(), 2);
    assert_eq!(w.form8.last().expect("dialog").0.pick("row"), 1);
    // Its range: page 2 only; then it moves up to the top.
    w.form8
        .last_mut()
        .expect("dialog")
        .0
        .set("c9range", Val::Text("2".into()));
    w.combine9_button("c9set");
    assert_eq!(w.combine9[1].range, "2");
    w.combine9_button("c9up");
    assert_eq!(w.combine9[0].name, "two.pdf");
    assert_eq!(w.form8.last().expect("dialog").0.pick("row"), 0);
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    let req = w.take_requests();
    let Some(ReaderRequest::OpenDocument { bytes, .. }) = req.into_iter().next() else {
        panic!("no new document");
    };
    let mut doc = bo_pdfread::PdfDocument::open(bytes, bo_pdfread::standard_fonts()).expect("pdf");
    assert_eq!(doc.page_count(), 4);
    let first = doc
        .render(0, bo_pdfread::RenderOptions::default())
        .expect("renders")
        .list;
    assert!(first
        .commands
        .iter()
        .any(|c| matches!(c, bo_render::Command::Glyphs { text, .. } if text.contains("Page 2"))));
    assert!(w.combine9.is_empty());
    assert!(w.form8.is_empty());
}

#[test]
fn set_page_dimensions_has_the_reference_layout_and_removes_white_margins() {
    use crate::editor_form8::Val;
    let mut w = organize(2);
    w.run("org.crop", &Fixed);
    let t = paint(&mut w).texts;
    for l in [
        "Set Page Dimensions",
        "Margin Controls",
        "Constrain Proportions",
        "Remove White Margins",
        "Set To Zero",
        "Change Page Size",
        "Page Range",
        "Show All Boxes",
        "Units:",
    ] {
        assert!(t.iter().any(|x| x == l), "{l}: {t:?}");
    }
    assert!(
        t.iter()
            .any(|x| x.starts_with("Cropped page size: 8.50 × 11.00")),
        "{t:?}"
    );
    let f = &w.form8.last().expect("dialog").0;
    assert!((f.frame[2] - 843.0).abs() < 1.0 && (f.frame[3] - 673.0).abs() < 1.0);
    // Remove White Margins on page 1: the page shrinks to its text.
    w.form8
        .last_mut()
        .expect("dialog")
        .0
        .set("white", Val::Pick(1));
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    let d = w.view().expect("view").doc();
    let (a, b) = (
        d.page_info(0).expect("p").width,
        d.page_info(1).expect("p").width,
    );
    assert!(a < 200.0 && (b - 612.0).abs() < 0.1, "{a} {b}");
}
