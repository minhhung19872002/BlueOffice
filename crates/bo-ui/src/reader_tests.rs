//! Headless tests of the BlueReader window and view.
use crate::reader_view::{Fit, Layout, PdfStore, THUMB_KEY};
use crate::reader_window::{ReaderRequest, ReaderWindow};
use crate::{Input, Key, Mods, Painter, TextMeasure, TextStyle};
use bo_pdfread::PdfDocument;
use bo_render::{PageList, PathSeg, Rgba};
use std::collections::HashMap;
use std::sync::Arc;

struct Fixed;

impl TextMeasure for Fixed {
    fn measure(&self, text: &str, style: TextStyle) -> (f32, f32) {
        (
            text.chars().count() as f32 * style.size * 0.5,
            style.size * 1.3,
        )
    }
}

#[derive(Default)]
struct Store {
    keys: HashMap<usize, (f32, f32, f32)>,
    updates: usize,
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
        self.updates += 1;
        self.keys.insert(key, (list.width, list.height, scale));
        Ok(())
    }
    fn remove(&mut self, key: usize) {
        self.keys.remove(&key);
    }
}

#[derive(Default)]
struct Rec {
    pages: Vec<(usize, i32, i32)>,
    texts: Vec<String>,
}

impl Painter for Rec {
    fn clear(&mut self, _: Rgba) {}
    fn fill_rect(&mut self, _: f32, _: f32, _: f32, _: f32, _: Rgba) {}
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

/// A PDF with `pages` Letter pages, each showing "Page N hello world".
fn pdf(pages: usize) -> Vec<u8> {
    let mut objs: Vec<String> = vec![
        "<< /Type /Catalog /Pages 2 0 R >>".into(),
        String::new(),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".into(),
    ];
    let mut kids = Vec::new();
    for i in 0..pages {
        let content = format!("BT /F1 24 Tf 72 700 Td (Page {} hello world) Tj ET", i + 1);
        let page_id = objs.len() + 1;
        let content_id = page_id + 1;
        objs.push(format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents {content_id} 0 R \
             /Resources << /Font << /F1 3 0 R >> >> >>"
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

fn window(pages: usize) -> ReaderWindow {
    let doc = PdfDocument::open(pdf(pages), bo_pdfread::standard_fonts()).expect("pdf");
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

fn key(w: &mut ReaderWindow, k: Key, ctrl: bool, shift: bool) {
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

#[test]
fn opens_in_continuous_fit_width_and_draws_visible_pages() {
    let mut w = window(5);
    let v = w.view().expect("view");
    assert_eq!(v.layout(), Layout::Continuous);
    assert_eq!(v.fit(), Fit::Width);
    let f = v.frame();
    let r = v.page_rect(0).expect("page 0");
    // The page fills the width less the margins.
    assert!(r[2] > f[2] - 40.0 && r[2] <= f[2], "{r:?} in {f:?}");
    let mut store = Store::default();
    w.prepare(&mut store);
    assert!(store.keys.contains_key(&0));
    let mut p = Rec::default();
    w.paint(&mut p);
    assert!(p.pages.iter().any(|(k, ..)| *k == 0));
    assert!(p.texts.iter().any(|t| t == "1 / 5"), "{:?}", p.texts);
    assert_eq!(w.title(), "test.pdf - BlueReader");
}

#[test]
fn page_navigation_layouts_and_zoom() {
    let mut w = window(5);
    key(&mut w, Key::End, false, false);
    assert_eq!(w.view().map(crate::ReaderView::current_page), Some(4));
    key(&mut w, Key::Home, false, false);
    assert_eq!(w.view().map(crate::ReaderView::current_page), Some(0));
    key(&mut w, Key::Letter('+'), true, false);
    let z = w.view().map_or(0.0, crate::ReaderView::zoom);
    assert!(w.view().is_some_and(|v| v.fit() == Fit::None));
    key(&mut w, Key::Letter('1'), true, false);
    assert!((w.view().map_or(0.0, crate::ReaderView::zoom) - 1.0).abs() < 1e-6);
    assert!(z > 1.0);
    key(&mut w, Key::Letter('0'), true, false);
    let v = w.view().expect("view");
    assert_eq!(v.fit(), Fit::Page);
    let r = v.page_rect(0).expect("page");
    assert!(r[3] <= v.frame()[3], "the whole page fits");
    // Facing layout puts pages 2 and 3 side by side after the cover page.
    if let Some(v) = w.view_mut() {
        v.set_layout(Layout::ContinuousFacing);
        let a = v.page_rect(1).expect("p2");
        let b = v.page_rect(2).expect("p3");
        assert!((a[1] - b[1]).abs() < 1.0 && b[0] > a[0] + a[2]);
        v.set_layout(Layout::Single);
        v.go_to(3, None);
        assert_eq!(v.visible_pages(), vec![3]);
        // Scrolling past the end of a single page turns to the next one.
        for _ in 0..40 {
            v.scroll_by(0.0, 200.0);
        }
        assert_eq!(v.current_page(), 4);
    }
}

#[test]
fn search_selection_and_copy() {
    let mut w = window(3);
    let n = w.view_mut().map_or(0, |v| v.find("HELLO", false, false));
    assert_eq!(n, 3);
    let v = w.view().expect("view");
    assert_eq!(v.current_hit(), Some(0));
    assert_eq!(v.selected_text().as_deref(), Some("hello"));
    key(&mut w, Key::Letter('c'), true, false);
    let reqs = w.take_requests();
    assert!(
        reqs.contains(&ReaderRequest::Copy("hello".into())),
        "{reqs:?}"
    );
    if let Some(v) = w.view_mut() {
        v.next_hit(true);
        assert_eq!(v.current_hit(), Some(1));
        assert_eq!(v.selection().map(|s| s.page), Some(1));
    }
}

#[test]
fn drag_selects_text_and_highlight_adds_a_comment() {
    let mut w = window(1);
    let (r, s) = {
        let v = w.view().expect("view");
        (v.page_rect(0).expect("page"), v.px_per_pt())
    };
    // "Page 1 hello world" at 72 pt from the left, baseline 92 pt from the top (24 pt text).
    let y = r[1] + 85.0 * s;
    let x0 = r[0] + 74.0 * s;
    let x1 = r[0] + 200.0 * s;
    if let Some(v) = w.view_mut() {
        v.find("x", false, false); // renders the text layer
    }
    w.handle(
        Input::Press {
            x: x0,
            y,
            count: 1,
            shift: false,
        },
        &Fixed,
    );
    w.handle(Input::Drag { x: x1, y }, &Fixed);
    w.handle(Input::Release { x: x1, y }, &Fixed);
    let text = w
        .view()
        .and_then(crate::ReaderView::selected_text)
        .unwrap_or_default();
    assert!(text.starts_with("Page 1"), "{text:?}");
    // Highlight tool: the next drag becomes a highlight annotation.
    if let Some(v) = w.view_mut() {
        v.set_selection(None);
    }
    w.handle(Input::Key(Key::Escape, Mods::default()), &Fixed);
    w.set_tool(crate::reader_window::Tool::Markup(
        bo_pdfread::AnnotKind::Highlight,
    ));
    w.handle(
        Input::Press {
            x: x0,
            y,
            count: 1,
            shift: false,
        },
        &Fixed,
    );
    w.handle(Input::Drag { x: x1, y }, &Fixed);
    w.handle(Input::Release { x: x1, y }, &Fixed);
    let v = w.view().expect("view");
    assert!(v.modified());
    let list = v.doc().annotations();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].kind, bo_pdfread::AnnotKind::Highlight);
    assert!(list[0].contents.starts_with("Page 1"));
}

#[test]
fn thumbnails_use_their_own_keys() {
    let mut w = window(4);
    w.handle(Input::Key(Key::Escape, Mods::default()), &Fixed);
    w.toggle_panel(crate::reader_window::Panel::Pages);
    let mut store = Store::default();
    for _ in 0..4 {
        w.prepare(&mut store);
    }
    assert!(store.keys.keys().any(|k| *k >= THUMB_KEY));
    let thumb = store.keys.get(&THUMB_KEY).expect("first thumbnail");
    assert!(thumb.2 < 0.5, "thumbnails are small: {thumb:?}");
}

#[test]
fn page_is_centred_in_the_frame() {
    // Opened before the window knows its size and scale, like the host does.
    let doc = PdfDocument::open(pdf(5), bo_pdfread::standard_fonts()).expect("pdf");
    let mut w = ReaderWindow::new("en-US");
    w.open(doc, "test.pdf", None);
    w.handle(Input::Scale(1.25), &Fixed);
    w.handle(
        Input::Resize {
            width: 1500.0,
            height: 1100.0,
        },
        &Fixed,
    );
    let v = w.view().expect("view");
    let f = v.frame();
    let r = v.page_rect(0).expect("page");
    let left = r[0] - f[0];
    let right = f[0] + f[2] - (r[0] + r[2]);
    assert!(
        (left - right).abs() <= 2.0 && left >= 0.0,
        "frame {f:?} page {r:?}"
    );
    // The view starts at the top: the 7.5 px margin above the first page shows.
    assert!((r[1] - f[1] - 8.0).abs() < 1.0, "frame {f:?} page {r:?}");
}

#[test]
fn read_mode_and_full_screen_flip_pages() {
    let mut w = window(4);
    let before = w.view().expect("view").frame();
    // Read Mode (Ctrl+H): the document starts under the title bar.
    key(&mut w, Key::Letter('h'), true, false);
    let read = w.view().expect("view").frame();
    assert!(
        read[1] < before[1] && read[3] > before[3],
        "{read:?} vs {before:?}"
    );
    key(&mut w, Key::Letter('h'), true, false);
    assert_eq!(w.view().expect("view").frame(), before);
    // Full Screen (F11): the whole window, one page fitted, flipped by keys and clicks.
    key(&mut w, Key::F(11), false, false);
    assert!(w
        .take_requests()
        .contains(&ReaderRequest::Window(crate::Request::FullScreen(true))));
    let v = w.view().expect("view");
    assert_eq!(v.frame(), [0.0, 0.0, 1500.0, 1100.0]);
    assert_eq!((v.layout(), v.fit()), (Layout::Single, Fit::Page));
    key(&mut w, Key::Right, false, false);
    w.handle(
        Input::Press {
            x: 700.0,
            y: 500.0,
            count: 1,
            shift: false,
        },
        &Fixed,
    );
    assert_eq!(w.view().expect("view").current_page(), 2);
    key(&mut w, Key::PageUp, false, false);
    assert_eq!(w.view().expect("view").current_page(), 1);
    key(&mut w, Key::Escape, false, false);
    let v = w.view().expect("view");
    assert_eq!((v.layout(), v.fit()), (Layout::Continuous, Fit::Width));
    assert_eq!(v.frame(), before);
    assert!(w
        .take_requests()
        .contains(&ReaderRequest::Window(crate::Request::FullScreen(false))));
}

#[test]
fn print_dialog_returns_the_job() {
    let mut w = window(3);
    key(&mut w, Key::Letter('p'), true, false);
    assert!(w.take_requests().contains(&ReaderRequest::Print));
    w.open_print(vec!["Printer A".into()], Some("Printer A"), &Fixed);
    assert!(w.print_open());
    key(&mut w, Key::Enter, false, false);
    assert!(!w.print_open());
    let reqs = w.take_requests();
    assert!(reqs.iter().any(|r| matches!(
        r,
        ReaderRequest::PrintPages(s) if s.printer == "Printer A" && s.pages == vec![0, 1, 2]
    )));
}

#[test]
fn select_move_recolour_and_delete_a_comment() {
    let mut w = window(1);
    w.view_mut()
        .expect("view")
        .doc_mut()
        .add_annotation(&bo_pdfread::NewAnnot {
            kind: bo_pdfread::AnnotKind::Square,
            page: 0,
            rects: vec![[100.0, 100.0, 200.0, 160.0]],
            color: [255, 0, 0],
            author: "T".into(),
            contents: "box".into(),
        })
        .expect("adds");
    let (r, s) = {
        let v = w.view().expect("view");
        (v.page_rect(0).expect("page"), v.px_per_pt())
    };
    let (x, y) = (r[0] + 150.0 * s, r[1] + 130.0 * s);
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
            x: x + 20.0 * s,
            y: y + 10.0 * s,
        },
        &Fixed,
    );
    w.handle(
        Input::Release {
            x: x + 20.0 * s,
            y: y + 10.0 * s,
        },
        &Fixed,
    );
    let a = w.view().expect("view").doc().annotations().remove(0);
    assert!(
        (a.rect[0] - 120.0).abs() < 0.5 && (a.rect[1] - 110.0).abs() < 0.5,
        "{a:?}"
    );
    // Context menu: colour; then Delete removes it.
    w.handle(
        Input::Context {
            x: x + 20.0 * s,
            y: y + 10.0 * s,
        },
        &Fixed,
    );
    w.run_command("comment.color.3", &Fixed);
    assert_eq!(
        w.view().expect("view").doc().annotations()[0].color,
        [30, 136, 229]
    );
    key(&mut w, Key::Escape, false, false);
    key(&mut w, Key::Delete, false, false);
    assert_eq!(w.view().expect("view").doc().annotations().len(), 0);
}

#[test]
fn huge_zoom_draws_only_the_part_in_view() {
    let mut w = window(1);
    w.view_mut().expect("view").set_zoom(16.0);
    let mut s = Store::default();
    w.prepare(&mut s);
    let (pw, ph, sc) = s.keys[&0];
    // 1600 %: the whole page would be about 16 000 px wide; only the view's part is drawn.
    assert!(
        pw * sc <= 8192.0 && pw < 612.0 && ph < 792.0,
        "{pw} x {ph} at {sc}"
    );
    w.view_mut().expect("view").scroll_by(0.0, 6000.0);
    w.prepare(&mut s);
    assert!(w.view().expect("view").image_offset(0).1 > 0.0);
}

/// Records the first command of every page list drawn.
#[derive(Default)]
struct FirstCommand(HashMap<usize, Option<bo_render::Command>>);

impl PdfStore for FirstCommand {
    fn update(
        &mut self,
        key: usize,
        list: &PageList,
        _: &bo_text::FontManager,
        _: &HashMap<String, Arc<bo_pdfread::Image>>,
        _: f32,
    ) -> Result<(), String> {
        self.0.insert(key, list.commands.first().cloned());
        Ok(())
    }
    fn remove(&mut self, key: usize) {
        self.0.remove(&key);
    }
}

#[test]
fn night_mode_reflow_autoscroll_cover_and_rulers() {
    let mut w = window(4);
    // Night Mode: pages are drawn again on a black sheet.
    w.run_command("view.night", &Fixed);
    let mut store = FirstCommand::default();
    w.prepare(&mut store);
    assert!(matches!(
        store.0.get(&0),
        Some(Some(bo_render::Command::FillRect {
            color: Rgba(0, 0, 0, 255),
            ..
        }))
    ));
    w.run_command("view.night", &Fixed);
    // Reflow: the page's text is drawn as lines in the window's font.
    w.run_command("view.reflow", &Fixed);
    w.handle(Input::Tick, &Fixed);
    let mut p = Rec::default();
    w.paint(&mut p);
    assert!(
        p.texts.iter().any(|t| t == "Page 1 hello world"),
        "{:?}",
        p.texts
    );
    assert!(p.pages.iter().all(|(k, ..)| *k >= THUMB_KEY));
    // Page Down moves on to the next page's text (one short page per screen).
    key(&mut w, Key::PageDown, false, false);
    w.handle(Input::Tick, &Fixed);
    let mut p = Rec::default();
    w.paint(&mut p);
    assert!(
        p.texts.iter().any(|t| t == "Page 2 hello world"),
        "{:?}",
        p.texts
    );
    w.run_command("view.reflow", &Fixed);
    // AutoScroll: ticks move the document down; Escape stops.
    w.run_command("view.autoScroll", &Fixed);
    let y0 = w.view().expect("view").scroll_state().0 .1;
    for _ in 0..5 {
        assert!(w.handle(Input::Tick, &Fixed));
    }
    let y1 = w.view().expect("view").scroll_state().0 .1;
    assert!(y1 > y0 + 10.0, "{y0} → {y1}");
    key(&mut w, Key::Escape, false, false);
    assert!(!w.handle(Input::Tick, &Fixed));
    // Separate cover page in facing layouts.
    w.run_command("view.facing", &Fixed);
    let cover = w.view().expect("view").cover();
    w.run_command("view.cover", &Fixed);
    assert_eq!(w.view().expect("view").cover(), !cover);
    // Rulers take 16 px (125 %: 20) along the top and the left.
    let before = w.view().expect("view").frame();
    w.run_command("view.rulers", &Fixed);
    let after = w.view().expect("view").frame();
    assert_eq!((after[0] - before[0], after[1] - before[1]), (20.0, 20.0));
    w.run_command("view.grid", &Fixed);
    let mut p = Rec::default();
    w.paint(&mut p);
    assert!(p.texts.iter().any(|t| t == "1"));
}

fn press(w: &mut ReaderWindow, x: f32, y: f32, count: u8) {
    w.handle(
        Input::Press {
            x,
            y,
            count,
            shift: false,
        },
        &Fixed,
    );
}

fn drag(w: &mut ReaderWindow, from: (f32, f32), to: (f32, f32)) {
    press(w, from.0, from.1, 1);
    w.handle(Input::Drag { x: to.0, y: to.1 }, &Fixed);
    w.handle(Input::Release { x: to.0, y: to.1 }, &Fixed);
}

#[test]
fn drawing_tools_stamp_measure_resize_and_properties() {
    use bo_pdfread::AnnotKind as K;
    let mut w = window(2);
    let r = w.view().expect("view").page_rect(0).expect("page");
    let at = |fx: f32, fy: f32| (r[0] + r[2] * fx, r[1] + r[3] * fy);
    // Pencil: one stroke, one ink comment.
    w.run_command("draw.pencil", &Fixed);
    assert!(w.draw_tool().is_some());
    press(&mut w, at(0.2, 0.05).0, at(0.2, 0.05).1, 1);
    for k in 1..10 {
        let (x, y) = at(0.2 + k as f32 * 0.02, 0.05 + (k % 2) as f32 * 0.01);
        w.handle(Input::Drag { x, y }, &Fixed);
    }
    w.handle(Input::Release { x: 0.0, y: 0.0 }, &Fixed);
    // Arrow, oval and a stamp from the menu.
    w.run_command("draw.arrow", &Fixed);
    drag(&mut w, at(0.1, 0.12), at(0.4, 0.15));
    w.run_command("draw.oval", &Fixed);
    drag(&mut w, at(0.5, 0.12), at(0.7, 0.18));
    w.run_command("draw.stamp.Draft", &Fixed);
    press(&mut w, at(0.5, 0.25).0, at(0.5, 0.25).1, 1);
    w.handle(Input::Release { x: 0.0, y: 0.0 }, &Fixed);
    // Area: three clicks and a double click.
    w.run_command("draw.area", &Fixed);
    for (fx, fy) in [(0.1, 0.3), (0.3, 0.3), (0.3, 0.35)] {
        let (x, y) = at(fx, fy);
        press(&mut w, x, y, 1);
        w.handle(Input::Release { x, y }, &Fixed);
    }
    let (x, y) = at(0.1, 0.35);
    press(&mut w, x, y, 1);
    press(&mut w, x, y, 2);
    // Typewriter asks for its text.
    w.run_command("draw.typewriter", &Fixed);
    press(&mut w, at(0.1, 0.4).0, at(0.1, 0.4).1, 1);
    w.handle(Input::Release { x: 0.0, y: 0.0 }, &Fixed);
    w.handle(Input::Text("Typed".into()), &Fixed);
    key(&mut w, Key::Enter, false, false);
    let doc = w.view().expect("view").doc();
    let annots = doc.annotations();
    let kinds: Vec<_> = annots.iter().map(|a| a.kind).collect();
    assert_eq!(
        kinds,
        vec![K::Ink, K::Line, K::Circle, K::Stamp, K::Line, K::TextBox],
        "{annots:?}"
    );
    assert!(matches!(
        doc.drawing(&annots[4]).map(|d| d.0),
        Some(bo_pdfread::Shape::Area(v)) if v.len() == 4
    ));
    assert!(annots[4].contents.ends_with("sq in"));
    assert_eq!(annots[5].contents, "Typed");
    // Escape leaves the tool; select the oval and drag its bottom-right handle.
    key(&mut w, Key::Escape, false, false);
    assert!(w.draw_tool().is_none());
    w.run_command("tool.hand", &Fixed);
    let oval = annots[2].clone();
    let wr = w
        .view()
        .expect("view")
        .doc_rect_to_window(0, oval.rect)
        .expect("rect");
    press(&mut w, wr[0] + wr[2] / 2.0, wr[1] + wr[3] / 2.0, 1);
    w.handle(
        Input::Release {
            x: wr[0] + wr[2] / 2.0,
            y: wr[1] + wr[3] / 2.0,
        },
        &Fixed,
    );
    let (hx, hy) = (wr[0] + wr[2], wr[1] + wr[3]);
    drag(&mut w, (hx, hy), (hx + 60.0, hy + 40.0));
    let grown = w.view().expect("view").doc().annotations()[2].clone();
    assert!(grown.rect[2] > oval.rect[2] + 20.0, "{oval:?} → {grown:?}");
    // Properties: thickness 3 pt for the selected oval.
    w.run_command("draw.props", &Fixed);
    let mut p = Rec::default();
    w.paint(&mut p);
    assert!(p.texts.iter().any(|t| t == "3 pt"));
}
