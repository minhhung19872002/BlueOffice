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
fn combine_files_tab_lists_ranges_reorders_and_makes_a_new_document() {
    use crate::editor_form8::Val;
    use crate::reader_window::ReaderRequest;
    let mut w = window(3);
    w.run("conv.combine", &Fixed);
    assert_eq!(w.combine9.len(), 1, "the open document comes first");
    // A contextual ribbon tab and a document tab, as the reference.
    assert_eq!(w.r.ribbon_tab(), crate::editor_window::TABS.len());
    assert!(w.combine9_shown());
    let t = paint(&mut w).texts;
    for l in [
        "Combine Files",
        "Add",
        "Move",
        "Page Range",
        "Bookmark for File",
        "Warnings/Errors",
        "ALL",
        "Combine",
        "Cancel",
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
    assert_eq!(w.comb9.as_ref().expect("tab").row, 1);
    // Its range: page 2 only (Enter asks for it); then it moves up to the top.
    paint(&mut w);
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    w.form8
        .last_mut()
        .expect("page range")
        .0
        .set("c9range", Val::Text("2".into()));
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    assert_eq!(w.combine9[1].range, "2");
    w.run("comb9.up", &Fixed);
    assert_eq!(w.combine9[0].name, "two.pdf");
    assert_eq!(w.comb9.as_ref().expect("tab").row, 0);
    w.run("comb9.combine", &Fixed);
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
    assert!(w.comb9.is_none());
    assert!(w.r.ribbon_tab() < crate::editor_window::TABS.len());
}

#[test]
fn set_page_dimensions_has_the_reference_layout_and_removes_white_margins() {
    use crate::editor_form8::Val;
    let mut w = organize(2);
    w.run("org.crop", &Fixed);
    // The Crop Pages tool shows the page; a click opens the dialog.
    assert!(!w.organize_on());
    crate::editor_tools_tests::drag(&mut w, 0, [100.0, 100.0, 101.0, 101.0]);
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

#[test]
fn delete_pages_dialog_deletes_the_range_and_its_even_pages() {
    use crate::editor_form8::Val;
    let mut w = organize(5);
    w.run("org.delete", &Fixed);
    let t = paint(&mut w).texts;
    for l in [
        "Delete Pages",
        "Page Range",
        "Page:",
        "Delete:",
        "Show Preview",
    ] {
        assert!(t.iter().any(|x| x == l), "{l}: {t:?}");
    }
    let f = &w.form8.last().expect("dialog").0;
    assert!((f.frame[2] - 453.0).abs() < 1.0 && (f.frame[3] - 364.0).abs() < 1.0);
    assert_eq!(f.text("pages"), "1,");
    {
        let f = &mut w.form8.last_mut().expect("dialog").0;
        f.set("pages", Val::Text("2-5".into()));
        f.set("subset", Val::Pick(1));
    }
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    assert!(w.form8.is_empty());
    // Pages 2 and 4 went.
    assert_eq!(page_count(&mut w), 3);
    // The Delete key in the page grid still deletes at once.
    w.handle(Input::Key(Key::Delete, Mods::default()), &Fixed);
    assert!(w.form8.is_empty());
    assert_eq!(page_count(&mut w), 2);
}

#[test]
fn swap_two_pages_has_the_reference_layout() {
    let mut w = organize(3);
    w.run("org.swap", &Fixed);
    let t = paint(&mut w).texts;
    for l in ["Swap Two Pages", "Swap:", "For:", "of", "3"] {
        assert!(t.iter().any(|x| x == l), "{l}: {t:?}");
    }
    let f = &w.form8.last().expect("dialog").0;
    assert!((f.frame[2] - 419.0).abs() < 1.0 && (f.frame[3] - 178.0).abs() < 1.0);
    assert_eq!((f.text("a"), f.text("b")), ("1".into(), "1".into()));
}

#[test]
fn replace_pages_dialog_takes_every_page_of_the_file_by_default() {
    let mut w = organize(3);
    w.run("org.replace", &Fixed);
    w.take_requests();
    w.file_picked(
        "replacePages",
        std::path::Path::new("src.pdf"),
        &crate::editor_tests::pdf(2),
    );
    let t = paint(&mut w).texts;
    for l in [
        "Replace Pages",
        "Select pages to replace",
        "Select replacement pages",
        "src.pdf",
        "Browse...",
    ] {
        assert!(t.iter().any(|x| x == l), "{l}: {t:?}");
    }
    let f = &w.form8.last().expect("dialog").0;
    assert!((f.frame[2] - 763.0).abs() < 1.0 && (f.frame[3] - 311.0).abs() < 1.0);
    // Browse asks for another file.
    w.pages9_button("replace9browse");
    assert!(matches!(
        &w.take_requests()[..],
        [crate::reader_window::ReaderRequest::PickFile { purpose, .. }] if purpose == "replacePages9"
    ));
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    // Page 1 gave way to both pages of the file.
    assert_eq!(page_count(&mut w), 4);
}

#[test]
fn rotate_pages_dialog_turns_the_even_pages_of_a_range() {
    use crate::editor_form8::Val;
    let mut w = organize(4);
    w.run("conv9.rotateMulti", &Fixed);
    let t = paint(&mut w).texts;
    for l in [
        "Rotate Pages",
        "Direction",
        "Clockwise 90 degrees",
        "Counterclockwise 90 degrees",
        "180 degrees",
        "Landscape and portrait pages",
    ] {
        assert!(t.iter().any(|x| x == l), "{l}: {t:?}");
    }
    {
        let f = &mut w.form8.last_mut().expect("dialog").0;
        assert!((f.frame[2] - 481.0).abs() < 1.0 && (f.frame[3] - 469.0).abs() < 1.0);
        f.set("dir", Val::Pick(1));
        f.set("subset", Val::Pick(1));
    }
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    let rot: Vec<u16> = {
        let d = w.view().expect("view").doc();
        (0..4).map(|i| d.page_info(i).expect("p").rotate).collect()
    };
    assert_eq!(rot, [0, 270, 0, 270]);
}

#[test]
fn convert_menus_have_the_reference_rows_and_write_tiff_bmp_rtf() {
    use crate::editor_conv9::{encode_bmp, encode_tiff, menu_rows, to_rtf};
    let office: Vec<&str> = menu_rows("conv.officeMenu")
        .expect("rows")
        .iter()
        .map(|r| r.0)
        .collect();
    assert_eq!(office.len(), 6);
    assert_eq!(menu_rows("conv.imageMenu").expect("rows").len(), 5);
    assert_eq!(menu_rows("conv.otherMenu").expect("rows").len(), 4);
    // 2 × 1 pixels: red, half-transparent blue over white.
    let px = [255, 0, 0, 255, 0, 0, 255, 128];
    let bmp = encode_bmp(&px, 2, 1);
    assert_eq!(&bmp[..2], b"BM");
    assert_eq!(bmp.len(), 54 + 8);
    assert_eq!(&bmp[54..57], &[0, 0, 255]);
    let back = bo_raster::codecs::decode(&bmp).expect("bmp decodes");
    assert_eq!((back.width, back.height), (2, 1));
    let tif = encode_tiff(&px, 2, 1);
    let img = bo_raster::codecs::decode(&tif).expect("tiff decodes");
    assert_eq!((img.width, img.height), (2, 1));
    assert_eq!(&img.pixels[..4], &[255, 0, 0, 255]);
    let rtf = to_rtf("Hà {x}\n\u{c}Page 2\n");
    assert!(rtf.starts_with(r"{\rtf1"));
    assert!(rtf.contains(r"H\u224?") && rtf.contains(r"\{x\}") && rtf.contains(r"\page"));
    // The menu opens on the ribbon button.
    let mut w = window(1);
    w.show_tab(1, &Fixed);
    paint(&mut w);
    w.run("conv.imageMenu", &Fixed);
    let t = paint(&mut w).texts;
    for l in ["PNG...", "JPEG...", "JPEG2000...", "TIFF...", "BMP..."] {
        assert!(t.iter().any(|x| x == l), "{l}: {t:?}");
    }
}

#[test]
fn crop_tool_box_fills_the_margins_and_change_page_size_resizes() {
    use crate::editor_form8::Val;
    let mut w = organize(2);
    w.run("org.crop", &Fixed);
    // A box 1 inch in from the left and top, 3 from the right, 4 from the bottom.
    crate::editor_tools_tests::drag(&mut w, 0, [72.0, 72.0, 396.0, 504.0]);
    {
        let f = &w.form8.last().expect("Set Page Dimensions").0;
        let v: Vec<String> = ["left", "top", "right", "bottom"]
            .iter()
            .map(|s| f.text(s))
            .collect();
        assert_eq!(v, ["1.00", "1.00", "3.00", "4.00"]);
        assert_eq!(f.pick("size"), 0);
    }
    // Fixed Size A4 for page 1 only, no cropping.
    {
        let f = &mut w.form8.last_mut().expect("dialog").0;
        for s in ["left", "top", "right", "bottom"] {
            f.set(s, Val::Text("0".into()));
        }
        f.set("fixed", Val::Pick(1));
    }
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    let d = w.view().expect("view").doc();
    let p = d.page_info(0).expect("p");
    assert!((p.width - 595.28).abs() < 0.5 && (p.height - 841.89).abs() < 0.5);
    assert!((d.page_info(1).expect("p").width - 612.0).abs() < 0.1);
}
