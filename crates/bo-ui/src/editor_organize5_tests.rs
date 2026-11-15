//! Page grid, round 5: Ctrl+click, thumbnail zoom, page clipboard, pages dragged onto another
//! document's tab.
use crate::editor_tests::{key, pdf, texts, window, Fixed, Rec};
use crate::editor_window::{EditorWindow, TAB_ORGANIZE};
use crate::{Input, Key, Mods};
use bo_pdfread::PdfDocument;

fn centre(r: [f32; 4]) -> (f32, f32) {
    (r[0] + r[2] / 2.0, r[1] + r[3] / 2.0)
}

fn click(w: &mut EditorWindow, x: f32, y: f32, ctrl: bool) {
    w.set_pointer_mods(Mods {
        ctrl,
        ..Mods::default()
    });
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
    w.set_pointer_mods(Mods::default());
}

#[test]
fn ctrl_click_zoom_and_page_clipboard() {
    let mut w = window(4);
    w.show_tab(TAB_ORGANIZE, &Fixed);
    let (x, y) = centre(w.grid_cell(0).expect("cell"));
    click(&mut w, x, y, false);
    let (x, y) = centre(w.grid_cell(2).expect("cell"));
    click(&mut w, x, y, true);
    assert_eq!(w.selected_pages(), [0, 2]);
    click(&mut w, x, y, true);
    assert_eq!(w.selected_pages(), [0]);
    // Ctrl+wheel over the grid makes the thumbnails larger.
    let before = w.grid_cell(0).expect("cell");
    w.handle(Input::Move { x, y }, &Fixed);
    w.handle(Input::Zoom(3.0), &Fixed);
    let after = w.grid_cell(0).expect("cell");
    assert!(after[2] > before[2] * 1.2, "{before:?} {after:?}");
    // The slider is painted and sets the size.
    let mut rec = Rec::default();
    w.paint(&mut rec);
    let s = w.org.slider;
    w.handle(
        Input::Press {
            x: s[0] + s[2],
            y: s[1],
            count: 1,
            shift: false,
        },
        &Fixed,
    );
    assert!((w.org.zoom() - crate::editor_organize::ZOOM_MAX).abs() < 0.01);
    w.handle(Input::Drag { x: s[0], y: s[1] }, &Fixed);
    w.handle(Input::Release { x: s[0], y: s[1] }, &Fixed);
    assert!((w.org.zoom() - crate::editor_organize::ZOOM_MIN).abs() < 0.01);
    assert!(!w.org.sliding);
    w.org.set_zoom(1.0);
    // Ctrl+X cuts page 1, Ctrl+V puts it after the selected page.
    key(&mut w, Key::Letter('x'), true, false);
    assert_eq!(texts(&mut w), ["Page 2", "Page 3", "Page 4"]);
    let (x, y) = centre(w.grid_cell(1).expect("cell"));
    click(&mut w, x, y, false);
    key(&mut w, Key::Letter('v'), true, false);
    assert_eq!(texts(&mut w), ["Page 2", "Page 3", "Page 1", "Page 4"]);
    assert_eq!(w.selected_pages(), [2]);
    key(&mut w, Key::Letter('z'), true, false);
    assert_eq!(texts(&mut w), ["Page 2", "Page 3", "Page 4"]);
    // Ctrl+C keeps the pages.
    key(&mut w, Key::Letter('c'), true, false);
    assert_eq!(texts(&mut w).len(), 3);
}

#[test]
fn pages_dropped_on_another_tab_are_copied_there() {
    let mut w = window(1);
    let doc = PdfDocument::open(pdf(3), bo_pdfread::standard_fonts()).expect("pdf");
    w.open(doc, "second.pdf", None);
    w.show_tab(TAB_ORGANIZE, &Fixed);
    let mut rec = Rec::default();
    w.paint(&mut rec);
    // The first document's tab.
    let mut tab = None;
    'find: for y in (100..400).step_by(3) {
        for x in (0..1500).step_by(5) {
            if w.r.doc_tab_at(x as f32, y as f32) == Some(0) {
                tab = Some((x as f32 + 10.0, y as f32 + 3.0));
                break 'find;
            }
        }
    }
    let (tx, ty) = tab.expect("first tab");
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
    w.handle(Input::Drag { x: tx, y: ty + 40.0 }, &Fixed);
    w.handle(Input::Drag { x: tx, y: ty }, &Fixed);
    w.handle(Input::Release { x: tx, y: ty }, &Fixed);
    // This document is unchanged; the first one gained "Page 2".
    assert_eq!(texts(&mut w), ["Page 1", "Page 2", "Page 3"]);
    let first = w.r.tab_view_mut(0).expect("first");
    let t: Vec<String> = (0..first.page_count())
        .map(|i| first.text(i).map(bo_pdfread::TextLayer::text).unwrap_or_default())
        .collect();
    assert_eq!(t, ["Page 1", "Page 2"]);
}
