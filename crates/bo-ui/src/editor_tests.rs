//! Headless tests of the BlueEditor window: page grid, page commands, undo, title bar.
use crate::editor_window::{EditorWindow, EDITOR_TITLE};
use crate::reader_view::{PdfStore, THUMB_KEY};
use crate::reader_window::ReaderRequest;
use crate::{Input, Key, Mods, Painter, TextMeasure, TextStyle};
use bo_pdfread::PdfDocument;
use bo_render::{PageList, PathSeg, Rgba};
use std::collections::HashMap;
use std::sync::Arc;

pub(crate) struct Fixed;

impl TextMeasure for Fixed {
    fn measure(&self, text: &str, style: TextStyle) -> (f32, f32) {
        (
            text.chars().count() as f32 * style.size * 0.5,
            style.size * 1.3,
        )
    }
}

#[derive(Default)]
pub(crate) struct Store {
    pub keys: HashMap<usize, (f32, f32, f32)>,
}

impl PdfStore for Store {
    fn update(
        &mut self,
        key: usize,
        list: &PageList,
        _: &bo_text::FontManager,
        _: &HashMap<String, Arc<bo_pdfread::Image>>,
        scale: f32,
    ) -> Result<(), String> {
        self.keys.insert(key, (list.width, list.height, scale));
        Ok(())
    }
    fn remove(&mut self, key: usize) {
        self.keys.remove(&key);
    }
}

#[derive(Default)]
pub(crate) struct Rec {
    pub pages: Vec<(usize, i32, i32)>,
    pub texts: Vec<String>,
    pub rects: Vec<([f32; 4], Rgba)>,
}

impl Painter for Rec {
    fn clear(&mut self, _: Rgba) {}
    fn fill_rect(&mut self, x: f32, y: f32, w: f32, h: f32, c: Rgba) {
        self.rects.push(([x, y, w, h], c));
    }
    fn page(&mut self, index: usize, x: i32, y: i32) {
        self.pages.push((index, x, y));
    }
    fn text(&mut self, text: &str, _: f32, _: f32, _: TextStyle, _: Rgba) {
        self.texts.push(text.to_owned());
    }
    fn path(&mut self, _: &[PathSeg], _: Option<Rgba>, _: Option<(Rgba, f32)>) {}
    fn push_clip(&mut self, _: f32, _: f32, _: f32, _: f32) {}
    fn pop_clip(&mut self) {}
}

/// A PDF with `pages` Letter pages, each showing "Page N".
pub(crate) fn pdf(pages: usize) -> Vec<u8> {
    let mut objs: Vec<String> = vec![
        "<< /Type /Catalog /Pages 2 0 R >>".into(),
        String::new(),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".into(),
    ];
    let mut kids = Vec::new();
    for i in 0..pages {
        let content = format!("BT /F1 24 Tf 72 700 Td (Page {}) Tj ET", i + 1);
        let page_id = objs.len() + 1;
        objs.push(format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents {} 0 R \
             /Resources << /Font << /F1 3 0 R >> >> >>",
            page_id + 1
        ));
        objs.push(format!(
            "<< /Length {} >>\nstream\n{content}\nendstream",
            content.len()
        ));
        kids.push(format!("{page_id} 0 R"));
    }
    objs[1] = format!(
        "<< /Type /Pages /Kids [{}] /Count {pages} >>",
        kids.join(" ")
    );
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

pub(crate) fn window(pages: usize) -> EditorWindow {
    let doc = PdfDocument::open(pdf(pages), bo_pdfread::standard_fonts()).expect("pdf");
    let mut w = EditorWindow::new("en-US");
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

pub(crate) fn key(w: &mut EditorWindow, k: Key, ctrl: bool, shift: bool) {
    w.handle(
        Input::Key(
            k,
            Mods {
                ctrl,
                shift,
                alt: false,
            },
        ),
        &Fixed,
    );
}

/// Text of every page, in order.
pub(crate) fn texts(w: &mut EditorWindow) -> Vec<String> {
    let v = w.view_mut().expect("view");
    (0..v.page_count())
        .map(|i| {
            v.text(i)
                .map(bo_pdfread::TextLayer::text)
                .unwrap_or_default()
        })
        .collect()
}

fn centre(r: [f32; 4]) -> (f32, f32) {
    (r[0] + r[2] / 2.0, r[1] + r[3] / 2.0)
}

#[test]
fn page_grid_select_delete_undo_and_drag() {
    let mut w = window(5);
    w.show_tab(crate::editor_window::TAB_ORGANIZE, &Fixed);
    assert!(w.organize_on());
    // Thumbnails of the grid are drawn, not the document pages.
    let mut store = Store::default();
    for _ in 0..4 {
        w.prepare(&mut store);
    }
    assert!((0..5).all(|i| store.keys.contains_key(&(THUMB_KEY + i))));
    let mut rec = Rec::default();
    w.paint(&mut rec);
    assert_eq!(
        rec.pages.iter().filter(|p| p.0 >= THUMB_KEY).count(),
        5,
        "{:?}",
        rec.pages
    );
    assert!(rec.texts.iter().any(|t| t == "3"));
    // Click page 2, Shift+click page 3, delete both.
    let (x, y) = centre(w.grid_cell(1).expect("cell"));
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
    let (x3, y3) = centre(w.grid_cell(2).expect("cell"));
    w.handle(
        Input::Press {
            x: x3,
            y: y3,
            count: 1,
            shift: true,
        },
        &Fixed,
    );
    w.handle(Input::Release { x: x3, y: y3 }, &Fixed);
    assert_eq!(w.selected_pages(), [1, 2]);
    key(&mut w, Key::Delete, false, false);
    assert_eq!(texts(&mut w), ["Page 1", "Page 4", "Page 5"]);
    assert!(w.view().is_some_and(crate::ReaderView::modified));
    key(&mut w, Key::Letter('z'), true, false);
    assert_eq!(
        texts(&mut w),
        ["Page 1", "Page 2", "Page 3", "Page 4", "Page 5"]
    );
    key(&mut w, Key::Letter('y'), true, false);
    assert_eq!(texts(&mut w), ["Page 1", "Page 4", "Page 5"]);
    key(&mut w, Key::Letter('z'), true, false);
    // Drag page 1 after page 3.
    let (x, y) = centre(w.grid_cell(0).expect("cell"));
    w.handle(
        Input::Press {
            x,
            y,
            count: 1,
            shift: false,
        },
        &Fixed,
    );
    let c = w.grid_cell(2).expect("cell");
    let (tx, ty) = (c[0] + c[2] * 0.8, c[1] + c[3] / 2.0);
    w.handle(Input::Drag { x: tx, y: ty }, &Fixed);
    w.handle(Input::Release { x: tx, y: ty }, &Fixed);
    assert_eq!(
        texts(&mut w),
        ["Page 2", "Page 3", "Page 1", "Page 4", "Page 5"]
    );
    assert_eq!(w.selected_pages(), [2]);
    // Double-click opens the page in the document view.
    let (x, y) = centre(w.grid_cell(4).expect("cell"));
    w.handle(
        Input::Press {
            x,
            y,
            count: 2,
            shift: false,
        },
        &Fixed,
    );
    w.handle(Input::Release { x, y }, &Fixed);
    assert!(!w.organize_on());
}

#[test]
fn page_commands_make_files_and_requests() {
    let mut w = window(4);
    w.show_tab(crate::editor_window::TAB_ORGANIZE, &Fixed);
    key(&mut w, Key::Letter('a'), true, false);
    w.run("org.reverse", &Fixed);
    assert_eq!(texts(&mut w), ["Page 4", "Page 3", "Page 2", "Page 1"]);
    key(&mut w, Key::Home, false, false);
    w.run("org.rotateRight", &Fixed);
    assert_eq!(
        w.view()
            .and_then(|v| v.doc().page_info(0))
            .map(|p| p.rotate),
        Some(90)
    );
    // Extract Pages (round 8: the reference's dialog; not saved at once: a new tab).
    w.run("org.extract", &Fixed);
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    let req = w.take_requests();
    let Some(ReaderRequest::OpenDocument { name, bytes }) = req.first() else {
        panic!("{req:?}");
    };
    assert_eq!(name, "document-1.pdf");
    let part = PdfDocument::open(bytes.clone(), bo_pdfread::standard_fonts()).expect("opens");
    assert_eq!(part.page_count(), 1);
    // Split asks how many pages per file, then writes the parts.
    w.run("org.split", &Fixed);
    w.handle(Input::Key(Key::Backspace, Mods::default()), &Fixed);
    w.handle(Input::Text("3".into()), &Fixed);
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    let req = w.take_requests();
    let Some(ReaderRequest::SaveMany(files)) = req.first() else {
        panic!("{req:?}");
    };
    assert_eq!(files.len(), 2);
    assert_eq!(files[1].0, "document-part2.pdf");
    // Insert from a file: the host picks it and hands the bytes back.
    w.run("org.insertFile", &Fixed);
    let req = w.take_requests();
    assert!(matches!(&req[..], [ReaderRequest::PickFile { purpose, .. }] if purpose == "insert"));
    w.file_picked("insert", std::path::Path::new("two.pdf"), &pdf(2));
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    assert_eq!(
        texts(&mut w),
        ["Page 4", "Page 1", "Page 2", "Page 3", "Page 2", "Page 1"]
    );
    w.run("org.insertBlank", &Fixed);
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    assert_eq!(w.view().map(crate::ReaderView::page_count), Some(7));
    // Merging appends.
    w.file_picked("merge", std::path::Path::new("one.pdf"), &pdf(1));
    assert_eq!(w.view().map(crate::ReaderView::page_count), Some(8));
    // Everything is saved as one rewrite and reads back the same.
    let bytes = w.view_mut().expect("view").doc_mut().save().expect("saves");
    let mut again = PdfDocument::open(bytes, bo_pdfread::standard_fonts()).expect("opens");
    assert_eq!(again.page_count(), 8);
    let p = again
        .render(0, bo_pdfread::RenderOptions::default())
        .expect("renders");
    assert_eq!(bo_pdfread::TextLayer::new(p.chars).text(), "Page 4");
}

#[test]
fn title_bar_band_and_undo_buttons() {
    let mut w = window(2);
    assert_eq!(w.title(), "test.pdf - BlueEditor");
    let mut rec = Rec::default();
    w.paint(&mut rec);
    // The title bar is our own colour, full width.
    assert!(rec
        .rects
        .iter()
        .any(|(r, c)| *c == EDITOR_TITLE && r[0] == 0.0 && r[1] == 0.0 && r[2] == 1500.0));
    w.run("org.rotateLeft", &Fixed);
    assert!(w.view().is_some_and(|v| v.doc().can_undo()));
    w.run("edit.undo", &Fixed);
    assert_eq!(
        w.view()
            .and_then(|v| v.doc().page_info(0))
            .map(|p| p.rotate),
        Some(0)
    );
    assert!(w.view().is_some_and(|v| v.doc().can_redo()));
}

/// Window point at the centre of the first text line of page 0.
fn line_centre(w: &mut EditorWindow) -> (f32, f32) {
    let v = w.view_mut().expect("view");
    let objs = v.doc_mut().page_objects(0).expect("objects");
    let line = bo_pdfread::text_lines(&objs)
        .into_iter()
        .next()
        .expect("line");
    let r = v.doc_rect_to_window(0, line.bbox).expect("on screen");
    // Inside the first character so the caret goes before or after it.
    (r[0] + 3.0, r[1] + r[3] / 2.0)
}

#[test]
fn edit_text_in_place_and_undo() {
    let mut w = window(1);
    w.run("ed.editText", &Fixed);
    assert_eq!(w.edit_tool(), Some(crate::EditTool::Text));
    let (x, y) = line_centre(&mut w);
    w.handle(Input::Move { x, y }, &Fixed);
    assert_eq!(w.cursor(), 4);
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
    // The caret shows while typing.
    assert!(w.caret().is_some());
    key(&mut w, Key::End, false, false);
    w.handle(Input::Text("!".into()), &Fixed);
    w.handle(Input::Text(" ổn".into()), &Fixed);
    assert_eq!(texts(&mut w), ["Page 1! ổn"]);
    key(&mut w, Key::Home, false, false);
    key(&mut w, Key::Delete, false, false);
    assert_eq!(texts(&mut w), ["age 1! ổn"]);
    key(&mut w, Key::Escape, false, false);
    assert!(w.caret().is_none());
    // The whole typing session undoes in one step.
    key(&mut w, Key::Letter('z'), true, false);
    assert_eq!(texts(&mut w), ["Page 1"]);
    let mut rec = Rec::default();
    w.paint(&mut rec);
}

#[test]
fn edit_object_moves_and_deletes() {
    let mut w = window(1);
    w.run("ed.editObject", &Fixed);
    let (x, y) = line_centre(&mut w);
    w.handle(
        Input::Press {
            x,
            y,
            count: 1,
            shift: false,
        },
        &Fixed,
    );
    let s = w.view().map_or(1.0, crate::ReaderView::scale);
    // Drag 50 document points to the right and 20 down.
    let (tx, ty) = (x + 50.0 * s, y + 20.0 * s);
    w.handle(Input::Drag { x: tx, y: ty }, &Fixed);
    let mut rec = Rec::default();
    w.paint(&mut rec);
    w.handle(Input::Release { x: tx, y: ty }, &Fixed);
    let v = w.view_mut().expect("view");
    let objs = v.doc_mut().page_objects(0).expect("objects");
    let line = bo_pdfread::text_lines(&objs).remove(0);
    assert_eq!(line.text, "Page 1");
    assert!(
        (line.bbox[0] - (72.0 + 50.0)).abs() < 0.5,
        "{:?}",
        line.bbox
    );
    // Select it again and delete it.
    let (x, y) = line_centre(&mut w);
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
    key(&mut w, Key::Delete, false, false);
    assert_eq!(texts(&mut w), [""]);
    key(&mut w, Key::Letter('z'), true, false);
    key(&mut w, Key::Letter('z'), true, false);
    assert_eq!(texts(&mut w), ["Page 1"]);
}

#[test]
fn add_text_and_image() {
    let mut w = window(1);
    w.run("ed.addText", &Fixed);
    let v = w.view().expect("view");
    let r = v
        .doc_rect_to_window(0, [100.0, 300.0, 100.0, 300.0])
        .expect("page");
    w.handle(
        Input::Press {
            x: r[0],
            y: r[1],
            count: 1,
            shift: false,
        },
        &Fixed,
    );
    w.handle(Input::Release { x: r[0], y: r[1] }, &Fixed);
    w.handle(Input::Text("Ghi chú".into()), &Fixed);
    key(&mut w, Key::Escape, false, false);
    let t = texts(&mut w).remove(0);
    assert!(t.contains("Ghi chú"), "{t}");
    // A picture from a file goes in the middle of the page.
    let mut png_bytes = Vec::new();
    {
        let mut e = png_encoder(&mut png_bytes);
        e.write_image_data(&[255, 0, 0, 255, 0, 0, 255, 255, 0, 255, 0, 255, 9, 9, 9, 128])
            .expect("png");
    }
    w.run("ed.addImage", &Fixed);
    assert!(
        matches!(&w.take_requests()[..], [ReaderRequest::PickFile { purpose, .. }] if purpose == "addImage")
    );
    w.file_picked("addImage", std::path::Path::new("a.png"), &png_bytes);
    let v = w.view_mut().expect("view");
    let objs = v.doc_mut().page_objects(0).expect("objects");
    let img = objs
        .iter()
        .find(|o| o.kind == bo_pdfread::ObjectKind::Image)
        .expect("image");
    // 2 × 2 px at 96 dpi is 1.5 pt, centred on the 612 × 792 page.
    assert!(
        (img.bbox[0] - 305.25).abs() < 0.1 && (img.bbox[1] - 395.25).abs() < 0.1,
        "{:?}",
        img.bbox
    );
}

/// A 2 × 2 PNG.
pub(crate) fn png2x2() -> Vec<u8> {
    let mut out = Vec::new();
    {
        let mut e = png_encoder(&mut out);
        e.write_image_data(&[9; 16]).expect("png");
    }
    out
}

fn png_encoder(out: &mut Vec<u8>) -> png::Writer<&mut Vec<u8>> {
    let mut e = png::Encoder::new(out, 2, 2);
    e.set_color(png::ColorType::Rgba);
    e.set_depth(png::BitDepth::Eight);
    e.write_header().expect("header")
}
