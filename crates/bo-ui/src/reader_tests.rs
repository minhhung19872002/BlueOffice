//! Headless tests of the BlueReader window and view.
use crate::reader_view::{Fit, Layout, PdfStore, THUMB_KEY};
use crate::reader_window::{ReaderRequest, ReaderWindow};
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
pub(crate) struct Rec {
    pages: Vec<(usize, i32, i32)>,
    pub(crate) texts: Vec<String>,
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
pub(crate) fn pdf(pages: usize) -> Vec<u8> {
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
fn search_shows_the_first_hit_then_reads_the_rest_on_ticks() {
    let mut w = window(30);
    if let Some(v) = w.view_mut() {
        v.go_to(20, None);
    }
    let v = w.view_mut().expect("view");
    let start = v.current_page();
    assert!(start >= 19, "{start}");
    // Page 21 has "Page 21 hello world": the first hit is on the current page, shown before
    // pages 1–20 are read.
    v.find_start("hello", false, false);
    assert_eq!(v.current_hit(), Some(0));
    assert_eq!(v.hits()[0].page, start);
    assert!(v.searching());
    // Ticks read the rest; hits stay in page order and the current hit keeps its page.
    let mut ticks = 0;
    while v.search_step(std::time::Duration::ZERO) {
        ticks += 1;
    }
    assert!(ticks > 0);
    assert!(!v.searching());
    assert_eq!(v.hits().len(), 30);
    assert!(v.hits().windows(2).all(|h| h[0].page < h[1].page));
    let cur = v.current_hit().expect("current");
    assert_eq!(v.hits()[cur].page, start);
    // The window's status shows k / n.
    w.handle(Input::Tick, &Fixed);
    // "Page 3 hello" only once; a query without hits.
    let v = w.view_mut().expect("view");
    assert_eq!(v.find("Page 3 hello", false, false), 1);
    assert_eq!(v.find("nothing", false, false), 0);
    assert!(v.current_hit().is_none());
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
    assert!(w.print_dialog().is_some_and(crate::PrintDialog::has_view));
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
            color: Rgba(25, 25, 25, 255),
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
    // AutoScroll: every frame moves the document by the speed times the time since the last
    // (Down: faster); frames keep coming while it runs; Escape stops.
    w.run_command("view.autoScroll", &Fixed);
    for _ in 0..6 {
        key(&mut w, Key::Down, false, false);
    }
    let y0 = w.view().expect("view").scroll_state().0 .1;
    let mut store = Store::default();
    assert!(w.prepare(&mut store));
    std::thread::sleep(std::time::Duration::from_millis(150));
    assert!(w.prepare(&mut store));
    let y1 = w.view().expect("view").scroll_state().0 .1;
    assert!(y1 > y0 + 5.0, "{y0} → {y1}");
    key(&mut w, Key::Escape, false, false);
    w.prepare(&mut store);
    assert!(!w.prepare(&mut store));
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

#[test]
fn text_viewer_split_and_read_aloud() {
    let mut w = window(3);
    // Text Viewer: every page's text as plain lines, no page images.
    w.run_command("view.textViewer", &Fixed);
    let mut p = Rec::default();
    w.paint(&mut p);
    for n in 1..=3 {
        let line = format!("Page {n} hello world");
        assert!(p.texts.contains(&line), "{line} in {:?}", p.texts);
    }
    assert!(p.pages.iter().all(|(k, ..)| *k >= THUMB_KEY));
    w.run_command("view.textViewer", &Fixed);
    // Horizontal split: two panes the same width, one above the other; the second keeps its
    // own scroll and shows pages too.
    let whole = w.view().expect("view").frame();
    w.run_command("split.horizontal", &Fixed);
    let top = w.view().expect("view").frame();
    assert_eq!(top[2], whole[2]);
    assert!(top[3] < whole[3] / 2.0 + 1.0);
    let mut store = Store::default();
    w.prepare(&mut store);
    let mut p = Rec::default();
    w.paint(&mut p);
    let shown = p.pages.iter().filter(|(k, ..)| *k == 0).count();
    assert_eq!(shown, 2, "page 1 in both panes: {:?}", p.pages);
    // The wheel over the lower pane moves the view there.
    let below = (top[0] + 20.0, top[1] + top[3] + 40.0);
    w.handle(
        Input::Move {
            x: below.0,
            y: below.1,
        },
        &Fixed,
    );
    w.handle(Input::Wheel(-120.0), &Fixed);
    assert!(w.view().expect("view").frame()[1] > top[1] + top[3]);
    // Choosing it again removes the split.
    w.run_command("split.horizontal", &Fixed);
    assert_eq!(w.view().expect("view").frame(), whole);
    w.run_command("split.spreadsheet", &Fixed);
    let q = w.view().expect("view").frame();
    assert!(q[2] < whole[2] / 2.0 + 1.0 && q[3] < whole[3] / 2.0 + 1.0);
    w.run_command("split.spreadsheet", &Fixed);
    // Read aloud asks the host to speak the page's text.
    w.take_requests();
    w.run_command("read.page", &Fixed);
    w.run_command("read.stop", &Fixed);
    let req = w.take_requests();
    assert!(matches!(&req[..], [
        ReaderRequest::Speak { text, rate: 0 },
        ReaderRequest::SpeechControl(0)
    ] if text.contains("Page 1 hello world")));
}

#[test]
fn word_count_and_reverse_view() {
    let mut w = window(4);
    w.run_command("view.wordCount", &Fixed);
    let mut p = Rec::default();
    w.paint(&mut p);
    // "Page N hello world" on 4 pages.
    assert!(p.texts.iter().any(|t| t == "Words: 16"), "{:?}", p.texts);
    key(&mut w, Key::Enter, false, false);
    w.run_command("view.facing", &Fixed);
    w.run_command("view.cover", &Fixed);
    let v = w.view().expect("view");
    let (a, b) = (v.page_rect(0).expect("p0"), v.page_rect(1).expect("p1"));
    assert!(a[0] < b[0]);
    w.run_command("view.reverse", &Fixed);
    let v = w.view().expect("view");
    let (a, b) = (v.page_rect(0).expect("p0"), v.page_rect(1).expect("p1"));
    assert!(a[0] > b[0], "{a:?} {b:?}");
}

/// A PDF of the objects (object 1 the catalog).
fn raw_pdf(objs: &[&str]) -> Vec<u8> {
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

#[test]
fn form_fill_area_highlight_eraser_and_comment_order() {
    let pdf = raw_pdf(&[
        "<< /Type /Catalog /Pages 2 0 R /AcroForm << /Fields [4 0 R 5 0 R] /DA (/Helv 0 Tf 0 g) >> >>",
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Annots [4 0 R 5 0 R] >>",
        "<< /FT /Tx /T (name) /Subtype /Widget /Rect [72 700 300 724] /DA (/Helv 12 Tf 0 g) /P 3 0 R >>",
        "<< /FT /Btn /T (agree) /Subtype /Widget /Rect [72 650 90 668] /V /Off /AS /Off          /AP << /N << /Yes 6 0 R /Off 7 0 R >> >> /P 3 0 R >>",
        "<< /Type /XObject /Subtype /Form /BBox [0 0 12 12] /Length 16 >>
stream
0 g 2 2 8 8 re f
endstream",
        "<< /Type /XObject /Subtype /Form /BBox [0 0 12 12] /Length 0 >>
stream

endstream",
    ]);
    let doc = PdfDocument::open(pdf, bo_pdfread::standard_fonts()).expect("pdf");
    let mut w = ReaderWindow::new("en-US");
    w.handle(Input::Scale(1.25), &Fixed);
    w.handle(
        Input::Resize {
            width: 1500.0,
            height: 1100.0,
        },
        &Fixed,
    );
    w.open(doc, "form.pdf", None);
    w.run_command("form.highlight", &Fixed);
    // A click on the text field with the Hand tool asks for its value.
    let v = w.view().expect("view");
    let r = v
        .doc_rect_to_window(0, [80.0, 70.0, 90.0, 80.0])
        .expect("field on screen");
    press(&mut w, r[0] + 2.0, r[1] + 2.0, 1);
    w.handle(Input::Text("Lan".into()), &Fixed);
    key(&mut w, Key::Enter, false, false);
    let fields = w.view().expect("view").doc().form_fields();
    assert_eq!(
        fields
            .iter()
            .find(|f| f.name == "name")
            .map(|f| f.value.as_str()),
        Some("Lan")
    );
    // The check box toggles.
    let v = w.view().expect("view");
    let r = v
        .doc_rect_to_window(0, [75.0, 126.0, 80.0, 131.0])
        .expect("box on screen");
    press(&mut w, r[0] + 1.0, r[1] + 1.0, 1);
    let fields = w.view().expect("view").doc().form_fields();
    assert!(fields.iter().any(|f| f.name == "agree" && f.checked));
    // Reset Form empties the text field again.
    w.run_command("form.reset", &Fixed);
    let fields = w.view().expect("view").doc().form_fields();
    assert!(fields
        .iter()
        .any(|f| f.name == "name" && f.value.is_empty()));
    // Area Highlight: a dragged box becomes a highlight.
    w.run_command("comment.areaHighlight", &Fixed);
    let v = w.view().expect("view");
    let a = v
        .doc_rect_to_window(0, [100.0, 300.0, 300.0, 400.0])
        .expect("area");
    drag(&mut w, (a[0], a[1]), (a[0] + a[2], a[1] + a[3]));
    let comments = w.view().expect("view").doc().annotations();
    assert!(comments
        .iter()
        .any(|c| c.kind == bo_pdfread::AnnotKind::Highlight));
    w.run_command("comment.areaHighlight", &Fixed);
    // Eraser: a pencil stroke under the pointer goes away.
    w.view_mut()
        .expect("view")
        .doc_mut()
        .add_drawing(
            0,
            &bo_pdfread::Shape::Ink(vec![vec![[100.0, 200.0], [200.0, 220.0]]]),
            &bo_pdfread::DrawStyle::default(),
            "me",
            "",
        )
        .expect("ink");
    w.run_command("comment.eraser", &Fixed);
    let v = w.view().expect("view");
    let e = v
        .doc_rect_to_window(0, [150.0, 210.0, 151.0, 211.0])
        .expect("ink");
    press(&mut w, e[0], e[1], 1);
    let kinds: Vec<_> = w
        .view()
        .expect("view")
        .doc()
        .annotations()
        .iter()
        .map(|c| c.kind)
        .collect();
    assert!(!kinds.contains(&bo_pdfread::AnnotKind::Ink), "{kinds:?}");
    // The comment list's filter keeps one kind.
    w.run_command("comments.filter0", &Fixed);
    let shown = w.comments_shown(w.view().expect("view").doc().comment_threads());
    assert!(shown
        .iter()
        .all(|t| t.comment.kind == bo_pdfread::AnnotKind::Highlight));
}

#[test]
fn search_and_highlight_marks_every_match() {
    let mut w = window(2);
    w.run_command("comment.searchHighlight", &Fixed);
    w.handle(Input::Text("hello".into()), &Fixed);
    key(&mut w, Key::Enter, false, false);
    let comments = w.view().expect("view").doc().annotations();
    let pages: Vec<usize> = comments
        .iter()
        .filter(|c| c.kind == bo_pdfread::AnnotKind::Highlight)
        .map(|c| c.page)
        .collect();
    assert_eq!(pages, vec![0, 1]);
}

#[test]
fn round5_fill_sign_comment_rows_and_exchange() {
    let mut w = window(2);
    // A document without a form or a signature: Reset Form and Validate grey, the rest on.
    assert_eq!(w.tabs5_enabled("form.reset"), Some(false));
    assert_eq!(w.tabs5_enabled("prot.validate"), Some(false));
    assert_eq!(w.tabs5_enabled("prot.sign"), None);
    assert!(!ReaderWindow::tabs4_disabled("comment.summarize"));
    assert!(!ReaderWindow::tabs4_disabled("prot.fillSign"));
    // Comment tab (round 12, as the reference at 1518 px): Summarize / Import / Export as large
    // buttons in a row, then FDF via Email / Manage Comments / Keep Tool Selected as a column
    // of labelled rows 29 px apart.
    w.set_ribbon_tab(1, &Fixed);
    let s = w.control_rect("comment.summarize").expect("summarize");
    let i = w.control_rect("comment.import").expect("import");
    let e = w.control_rect("menu.commentExport").expect("export");
    assert!(s[0] < i[0] && i[0] < e[0]);
    assert_eq!((s[1], i[1]), (e[1], e[1]));
    let f = w.control_rect("comment.fdfMail").expect("fdf");
    let o = w.control_rect("menu.commentOptions").expect("manage");
    let k = w.control_rect("comment.keep").expect("keep");
    assert!(f[0] > e[0] + e[2] && (f[0], o[0]) == (k[0], k[0]));
    assert!((o[1] - f[1] - 29.0).abs() < 0.6, "{f:?} {o:?}");
    assert!(w.control_rect("panel.comments").is_none());
    // Fill & Sign: a contextual tab; marks become comments.
    w.run_command("prot.fillSign", &Fixed);
    assert!(w.control_rect("fill.check").is_some());
    w.run_command("fill.check", &Fixed);
    let r = w.view().expect("view").page_rect(0).expect("page");
    let (x, y) = (r[0] + 200.0, r[1] + 300.0);
    press(&mut w, x, y, 1);
    w.handle(Input::Release { x, y }, &Fixed);
    w.run_command("fill.cross", &Fixed);
    press(&mut w, x + 60.0, y, 1);
    w.handle(Input::Release { x: x + 60.0, y }, &Fixed);
    let kinds: Vec<_> = w
        .view()
        .expect("view")
        .doc()
        .annotations()
        .iter()
        .map(|a| a.kind)
        .collect();
    assert_eq!(
        kinds,
        vec![
            bo_pdfread::AnnotKind::Ink,
            bo_pdfread::AnnotKind::Line,
            bo_pdfread::AnnotKind::Line
        ]
    );
    w.run_command("fill.close", &Fixed);
    assert!(w.control_rect("fill.check").is_none());
    // Summary opens as a document, export offers a file, import adds the comments back.
    w.take_requests();
    w.run_command("comment.summarize", &Fixed);
    w.run_command("comment.exportXfdf", &Fixed);
    let req = w.take_requests();
    assert!(
        matches!(&req[0], ReaderRequest::OpenDocument { name, .. } if name == "test_summary.pdf")
    );
    let ReaderRequest::SaveBytes { name, bytes } = &req[1] else {
        panic!("{req:?}");
    };
    assert_eq!(name, "test.xfdf");
    let mut w2 = window(2);
    assert!(w2.file_picked5("comments", std::path::Path::new("a.xfdf"), bytes));
    assert_eq!(w2.view().expect("view").doc().annotations().len(), 3);
    // File: a click on the page asks for the file, which is attached there.
    w2.run_command("comment.file", &Fixed);
    press(&mut w2, x, y, 1);
    assert!(matches!(
        w2.take_requests().first(),
        Some(ReaderRequest::PickFile { purpose, .. }) if purpose == "attach"
    ));
    assert!(w2.file_picked5("attach", std::path::Path::new("notes.txt"), b"hello"));
    assert_eq!(w2.view().expect("view").doc().annotations().len(), 4);
}

#[test]
fn round5_zoom_presets_wheel_and_link_warning() {
    let mut w = window(3);
    // Ctrl + wheel walks the reference's presets.
    w.run_command("zoom.set.100", &Fixed);
    w.handle(Input::Zoom(1.0), &Fixed);
    assert!((w.view().expect("view").zoom() - 1.25).abs() < 1e-4);
    w.handle(Input::Zoom(-2.0), &Fixed);
    assert!((w.view().expect("view").zoom() - 0.75).abs() < 1e-4);
    w.run_command("zoom.set.6400", &Fixed);
    assert!((w.view().expect("view").zoom() - 64.0).abs() < 1e-3);
    // No /PageMode: no bookmarks panel at first.
    assert_eq!(w.panel(), None);
    // A web link asks first; Allow opens it.
    let mut w = window(1);
    w.confirm_url("https://example.com/".into(), &Fixed);
    assert_eq!(w.take_requests().len(), 0);
    key(&mut w, Key::Enter, false, false);
    assert!(matches!(
        w.take_requests().first(),
        Some(ReaderRequest::OpenUrl(u)) if u == "https://example.com/"
    ));
}

#[test]
fn round5_properties_dialog() {
    let mut w = window(2);
    w.run_command("file.properties", &Fixed);
    let mut p = Rec::default();
    w.paint(&mut p);
    assert!(p.texts.iter().any(|t| t == "Document Properties"));
    assert!(p.texts.iter().any(|t| t == "test.pdf"));
    assert!(p.texts.iter().any(|t| t == "1.7"));
    // Fonts tab lists Helvetica, not embedded.
    key(&mut w, Key::Tab, false, false);
    key(&mut w, Key::Tab, false, false);
    let mut p = Rec::default();
    w.paint(&mut p);
    assert!(p.texts.iter().any(|t| t == "Helvetica"), "{:?}", p.texts);
    assert!(p.texts.iter().any(|t| t.contains("Not embedded")));
    key(&mut w, Key::Escape, false, false);
    let mut p = Rec::default();
    w.paint(&mut p);
    assert!(!p.texts.iter().any(|t| t == "Document Properties"));
    // File opens the File page (backstage) with Open first, then Properties.
    let file = w.tab_boxes()[0];
    press(&mut w, file[0] + 4.0, file[1] + 4.0, 1);
    let mut p = Rec::default();
    w.paint(&mut p);
    assert!(p.texts.iter().any(|t| t == "Recent Documents"));
    assert!(p.texts.iter().any(|t| t == "Revert"));
    key(&mut w, Key::Escape, false, false);
    let mut p = Rec::default();
    w.paint(&mut p);
    assert!(!p.texts.iter().any(|t| t == "Recent Documents"));
}
