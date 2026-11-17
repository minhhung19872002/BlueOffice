//! Round 13: View ▸ Assistant ▸ Loupe and Magnifier.
use super::super::r12::Assist;
use crate::reader_tests::{pdf, Fixed};
use crate::reader_view::PREVIEW_KEY;
use crate::reader_window::ReaderWindow;
use crate::{Input, Key, Mods, Painter, PdfStore, TextStyle};
use bo_pdfread::PdfDocument;
use bo_render::{PageList, PathSeg, Rgba};
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Default)]
struct Store {
    keys: HashMap<usize, f32>,
}

impl PdfStore for Store {
    fn update(
        &mut self,
        key: usize,
        _: &PageList,
        _: &bo_text::FontManager,
        _: &HashMap<String, Arc<bo_pdfread::Image>>,
        scale: f32,
    ) -> Result<(), String> {
        self.keys.insert(key, scale);
        Ok(())
    }
    fn remove(&mut self, key: usize) {
        self.keys.remove(&key);
    }
}

#[derive(Default)]
struct Rec {
    pages: Vec<usize>,
    texts: Vec<String>,
}

impl Painter for Rec {
    fn clear(&mut self, _: Rgba) {}
    fn fill_rect(&mut self, _: f32, _: f32, _: f32, _: f32, _: Rgba) {}
    fn page(&mut self, index: usize, _: i32, _: i32) {
        self.pages.push(index);
    }
    fn text(&mut self, text: &str, _: f32, _: f32, _: TextStyle, _: Rgba) {
        self.texts.push(text.to_owned());
    }
    fn path(&mut self, _: &[PathSeg], _: Option<Rgba>, _: Option<(Rgba, f32)>) {}
    fn push_clip(&mut self, _: f32, _: f32, _: f32, _: f32) {}
    fn pop_clip(&mut self) {}
}

fn window() -> ReaderWindow {
    let doc = PdfDocument::open(pdf(2), bo_pdfread::standard_fonts()).expect("pdf");
    let mut w = ReaderWindow::new("en-US");
    w.handle(Input::Scale(1.25), &Fixed);
    w.handle(
        Input::Resize {
            width: 1518.0,
            height: 1036.0,
        },
        &Fixed,
    );
    w.open(doc, "test.pdf", None);
    w
}

fn press(w: &mut ReaderWindow, x: f32, y: f32) {
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
fn loupe_shows_the_clicked_place_at_its_own_zoom() {
    let mut w = window();
    let mut store = Store::default();
    w.prepare(&mut store);
    let items = w.assistant_items13();
    assert_eq!(items.len(), 3);
    assert!(items.iter().all(|i| !i.dim), "Loupe and Magnifier work now");
    w.run_command("assist.loupe", &Fixed);
    assert_eq!(w.assist12(), Some(Assist::Loupe));
    // The panel is up at once (title, Lock) but shows no page before a click.
    let mut p = Rec::default();
    w.paint(&mut p);
    assert!(p.texts.iter().any(|t| t == "Loupe Tool"));
    assert!(p.texts.iter().any(|t| t == "Lock"));
    let key = w.key_base().expect("tab") + PREVIEW_KEY + 1;
    assert!(!p.pages.contains(&key));
    // A click on the page (right of the panel) places the loupe there.
    let page = w.view().expect("view").page_rect(0).expect("page");
    let (x, y) = (page[0] + page[2] * 0.75, page[1] + page[3] * 0.2);
    press(&mut w, x, y);
    let place = w.twelve.thirteen.place.expect("placed");
    assert_eq!(place.0, 0);
    w.prepare(&mut store);
    // 200 % of actual size at 125 %: 1.25 × 96 / 72 × 2 device px per point.
    let s = store.keys.get(&key).copied().expect("loupe image");
    assert!((s - 1.25 * 96.0 / 72.0 * 2.0).abs() < 1e-3, "{s}");
    let mut p = Rec::default();
    w.paint(&mut p);
    assert!(p.pages.contains(&key));
    // + and − step through the zoom list; Lock keeps the place.
    w.run_command("loupe.in", &Fixed);
    assert!((w.twelve.thirteen.zoom - 300.0).abs() < 0.5);
    w.run_command("loupe.out", &Fixed);
    w.run_command("loupe.out", &Fixed);
    assert!((w.twelve.thirteen.zoom - 150.0).abs() < 0.5);
    w.run_command("loupe.zoom:800", &Fixed);
    assert!((w.twelve.thirteen.zoom - 800.0).abs() < 0.5);
    w.run_command("loupe.lock", &Fixed);
    press(&mut w, x, y + 100.0);
    assert_eq!(w.twelve.thirteen.place, Some(place));
    // Esc ends the tool.
    w.handle(Input::Key(Key::Escape, Mods::default()), &Fixed);
    assert_eq!(w.assist12(), None);
}

#[test]
fn magnifier_follows_the_pointer_over_the_page() {
    let mut w = window();
    let mut store = Store::default();
    w.prepare(&mut store);
    w.run_command("assist.magnifier", &Fixed);
    assert_eq!(w.assist12(), Some(Assist::Magnifier));
    let page = w.view().expect("view").page_rect(0).expect("page");
    let (x, y) = (page[0] + page[2] / 2.0, page[1] + page[3] / 3.0);
    w.handle(Input::Move { x, y }, &Fixed);
    assert_eq!(w.twelve.thirteen.pointer, Some((x, y)));
    w.prepare(&mut store);
    let key = w.key_base().expect("tab") + PREVIEW_KEY + 1;
    let s = store.keys.get(&key).copied().expect("lens image");
    let view = w.view().expect("view").px_per_pt();
    assert!((s - view * 2.25).abs() < 1e-3, "{s} {view}");
    let mut p = Rec::default();
    w.paint(&mut p);
    assert!(
        p.pages.iter().filter(|k| **k == key).count() > 50,
        "strips of the lens"
    );
    // Off the page area the lens goes away; another tool ends it.
    w.handle(Input::Move { x: 2.0, y: 2.0 }, &Fixed);
    assert_eq!(w.twelve.thirteen.pointer, None);
    w.run_command("tool.hand", &Fixed);
    assert_eq!(w.assist12(), None);
}
