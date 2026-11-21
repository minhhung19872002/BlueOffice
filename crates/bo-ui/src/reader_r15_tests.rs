//! Round 15: keys measured on the reference reader (posted key messages on a scratch copy).
use crate::reader_tests::{pdf, Fixed};
use crate::reader_view::{Fit, ZOOM_STEPS};
use crate::reader_window::ReaderWindow;
use crate::{Input, Key, Mods};
use bo_pdfread::PdfDocument;

fn window() -> ReaderWindow {
    let doc = PdfDocument::open(pdf(12), bo_pdfread::standard_fonts()).expect("pdf");
    let mut w = ReaderWindow::new("en-US");
    w.handle(Input::Scale(1.25), &Fixed);
    w.handle(
        Input::Resize {
            width: 1500.0,
            height: 1102.0,
        },
        &Fixed,
    );
    w.open(doc, "test.pdf", None);
    w
}

fn press(w: &mut ReaderWindow, k: Key, ctrl: bool, shift: bool, alt: bool) {
    w.handle(Input::Key(k, Mods { shift, ctrl, alt }), &Fixed);
}

fn scroll_y(w: &ReaderWindow) -> f32 {
    w.view().expect("view").scroll_state().0 .1
}

#[test]
fn page_down_scrolls_the_view_less_41_px() {
    let mut w = window();
    let h = w.view().expect("view").frame()[3];
    let y0 = scroll_y(&w);
    press(&mut w, Key::PageDown, false, false, false);
    assert!(
        (scroll_y(&w) - y0 - (h - 41.0)).abs() < 0.6,
        "{}",
        scroll_y(&w)
    );
    w.handle(Input::Text(" ".into()), &Fixed);
    assert!((scroll_y(&w) - y0 - 2.0 * (h - 41.0)).abs() < 1.0);
    press(&mut w, Key::PageUp, false, false, false);
    assert!((scroll_y(&w) - y0 - (h - 41.0)).abs() < 0.6);
}

#[test]
fn zoom_ladder_has_300_1200_and_the_fit_stops() {
    assert!(ZOOM_STEPS.contains(&3.0) && ZOOM_STEPS.contains(&12.0));
    let mut w = window();
    press(&mut w, Key::Letter('2'), true, false, false);
    let width = w.view().expect("view").zoom();
    press(&mut w, Key::Letter('1'), true, false, false);
    // 100 % → 125 % → fit width → 150 %.
    press(&mut w, Key::Letter('+'), true, false, false);
    assert!((w.view().expect("view").zoom() - 1.25).abs() < 1e-4);
    press(&mut w, Key::Letter('+'), true, false, false);
    let z = w.view().expect("view").zoom();
    assert!((z - width).abs() < 1e-4, "{z} vs {width}");
    press(&mut w, Key::Letter('+'), true, false, false);
    assert!((w.view().expect("view").zoom() - 1.5).abs() < 1e-4);
    // 200 % → 300 % → 400 %.
    w.view_mut().expect("view").set_zoom(2.0);
    w.view_mut().expect("view").step_zoom(1);
    assert!((w.view().expect("view").zoom() - 3.0).abs() < 1e-4);
}

#[test]
fn ctrl_3_fits_the_visible_content() {
    let mut w = window();
    press(&mut w, Key::Letter('2'), true, false, false);
    let width = w.view().expect("view").zoom();
    press(&mut w, Key::Letter('3'), true, false, false);
    let v = w.view().expect("view");
    // The page's text spans much less than its 612 pt: the zoom grows by the same ratio.
    assert!(v.zoom() > width * 1.3, "{} vs {width}", v.zoom());
    assert_eq!(v.fit(), Fit::None);
}

#[test]
fn alt_arrows_step_through_the_views() {
    let mut w = window();
    press(&mut w, Key::Letter('2'), true, false, false);
    press(&mut w, Key::Home, true, false, false);
    let top = scroll_y(&w);
    press(&mut w, Key::Right, false, false, false);
    press(&mut w, Key::Down, false, false, false);
    let p2 = scroll_y(&w);
    press(&mut w, Key::End, true, false, false);
    assert!(w.enabled("view.prev"));
    assert!(!w.enabled("view.next"));
    press(&mut w, Key::Left, false, false, true);
    assert!((scroll_y(&w) - p2).abs() < 0.6);
    press(&mut w, Key::Left, false, false, true);
    press(&mut w, Key::Left, false, false, true);
    assert!((scroll_y(&w) - top).abs() < 0.6);
    assert!(w.enabled("view.next"));
    press(&mut w, Key::Right, false, false, true);
    press(&mut w, Key::Right, false, false, true);
    assert!((scroll_y(&w) - p2).abs() < 0.6);
    // A zoom change is a view too.
    let z = w.view().expect("view").zoom();
    press(&mut w, Key::Letter('1'), true, false, false);
    press(&mut w, Key::Left, false, false, true);
    assert!((w.view().expect("view").zoom() - z).abs() < 1e-4);
    assert_eq!(w.view().expect("view").fit(), Fit::Width);
}

#[test]
fn f4_hides_the_navigation_column() {
    let mut w = window();
    let before = w.view().expect("view").frame();
    press(&mut w, Key::F(4), false, false, false);
    let after = w.view().expect("view").frame();
    assert!(after[0] < before[0] - 30.0 && after[2] > before[2] + 30.0);
    assert!(w.control_rect("nav.bookmarks").is_none());
    press(&mut w, Key::F(4), false, false, false);
    assert_eq!(w.view().expect("view").frame(), before);
}

#[test]
fn ctrl_g_edits_the_page_box() {
    let mut w = window();
    press(&mut w, Key::Letter('g'), true, false, false);
    w.handle(Input::Text("7".into()), &Fixed);
    press(&mut w, Key::Enter, false, false, false);
    assert_eq!(w.view().expect("view").current_page(), 6);
    // Escape leaves without moving.
    press(&mut w, Key::Letter('g'), true, false, false);
    w.handle(Input::Text("2".into()), &Fixed);
    press(&mut w, Key::Escape, false, false, false);
    assert_eq!(w.view().expect("view").current_page(), 6);
}

#[test]
fn ctrl_f_search_box_marks_only_the_current_match() {
    let mut w = window();
    press(&mut w, Key::Letter('f'), true, false, false);
    w.handle(Input::Text("hello".into()), &Fixed);
    press(&mut w, Key::Enter, false, false, false);
    for _ in 0..50 {
        w.handle(Input::Tick, &Fixed);
    }
    let v = w.view().expect("view");
    assert!(v.hits().len() >= 12, "{}", v.hits().len());
    assert_eq!(v.current_hit(), Some(0));
    assert_eq!(w.hit_colour15(true), Some(super::FIND_CURRENT));
    assert_eq!(w.hit_colour15(false), None);
    press(&mut w, Key::Enter, false, false, false);
    assert_eq!(w.view().expect("view").current_hit(), Some(1));
    press(&mut w, Key::Enter, false, true, false);
    assert_eq!(w.view().expect("view").current_hit(), Some(0));
    // Escape closes the box and its marks.
    press(&mut w, Key::Escape, false, false, false);
    assert_eq!(w.view().expect("view").hits().len(), 0);
}

#[test]
fn zoom_box_text_like_the_reference() {
    use super::super::zoom_label15;
    assert_eq!(zoom_label15(1.0), "100%");
    assert_eq!(zoom_label15(0.125), "12.50%");
    assert_eq!(zoom_label15(1.351), "135.10%");
}

#[test]
fn ctrl_3_scrolls_the_content_left_edge_to_the_margin() {
    let mut w = window();
    press(&mut w, Key::Letter('3'), true, false, false);
    let v = w.view().expect("view");
    let i = v.current_page();
    let r = v.page_rect(i).expect("page");
    let f = v.frame();
    // The pen starts at 72 pt, the ink of "P" one side bearing (about 1.8 pt) later: the ink
    // shows 7.5 px inside the view.
    let x = r[0] + 72.0 * v.px_per_pt() - f[0];
    let bearing = 3.0 * v.px_per_pt();
    assert!(
        x < 7.5 && x > 7.5 - bearing,
        "text at {x}, page {r:?}, frame {f:?}"
    );
}

/// Fit Visible zoom of real files (`BO_FV_PDFS`, `;`-separated) at the reference's fit width
/// frame (1394 px wide at 125 %): compare with the reference's zoom box.
#[test]
#[ignore = "needs BO_FV_PDFS"]
fn fit_visible_of_files() {
    let Ok(list) = std::env::var("BO_FV_PDFS") else {
        return;
    };
    for path in list.split(';') {
        let bytes = std::fs::read(path).expect("read");
        let doc = PdfDocument::open(bytes, bo_pdfread::standard_fonts()).expect("pdf");
        let mut v = crate::reader_view::ReaderView::new(doc);
        v.set_frame([76.0, 245.0, 1394.0, 814.0], 1.25);
        v.set_fit(Fit::Width);
        let w = v.zoom();
        v.fit_visible();
        println!(
            "{path}: width {:.2} % visible {:.2} %",
            w * 100.0,
            v.zoom() * 100.0
        );
    }
}
