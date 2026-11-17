//! Round 14: View tab placement, icons in the Select / Assistant menus.
use crate::reader_tests::{pdf, Fixed};
use crate::reader_window::ReaderWindow;
use crate::{Input, PdfStore};
use bo_pdfread::PdfDocument;
use bo_render::PageList;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// Keys held, like the host's page bitmaps.
#[derive(Default)]
struct Store {
    keys: HashSet<usize>,
}

impl PdfStore for Store {
    fn update(
        &mut self,
        key: usize,
        _: &PageList,
        _: &bo_text::FontManager,
        _: &HashMap<String, Arc<bo_pdfread::Image>>,
        _: f32,
    ) -> Result<(), String> {
        self.keys.insert(key);
        Ok(())
    }
    fn remove(&mut self, key: usize) {
        self.keys.remove(&key);
    }
}

fn window() -> ReaderWindow {
    let doc = PdfDocument::open(pdf(3), bo_pdfread::standard_fonts()).expect("pdf");
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

#[test]
fn view_tab_buttons_move_like_the_reference() {
    let mut w = window();
    let home_select = w.control_rect("tool.select").expect("select");
    w.set_ribbon_tab(2, &Fixed);
    let sel = w.control_rect("tool.select").expect("select");
    // Select is 4 px wider on the View tab only (measured at 125 %).
    assert!((sel[2] - home_select[2] - 4.0).abs() < 0.6, "{sel:?}");
    let read = w.control_rect("view.readMode").expect("read mode");
    let reflow = w.control_rect("view.reflow").expect("reflow");
    assert!(read[0] > sel[0] + sel[2]);
    assert!((reflow[0] - (read[0] + read[2]) - 2.0).abs() < 0.6);
    // The Home tab keeps its measured places.
    w.set_ribbon_tab(0, &Fixed);
    assert_eq!(w.control_rect("tool.select"), Some(home_select));
}

#[test]
fn select_and_assistant_menus_have_icons() {
    let mut w = window();
    let r = w.control_rect("tool.select").expect("select");
    w.handle(
        Input::Press {
            x: r[0] + r[2] / 2.0,
            y: r[1] + r[3] - 10.0,
            count: 1,
            shift: false,
        },
        &Fixed,
    );
    w.handle(
        Input::Release {
            x: r[0] + r[2] / 2.0,
            y: r[1] + r[3] - 10.0,
        },
        &Fixed,
    );
    assert!(w.modal_open());
    assert!(w.menu_has_icons14());
    w.close_menu();
    w.set_ribbon_tab(2, &Fixed);
    w.run_command("menu.assistant", &Fixed);
    assert!(w.menu_has_icons14());
}

#[test]
fn closing_a_tab_frees_its_page_images() {
    let mut w = window();
    let mut store = Store::default();
    for _ in 0..50 {
        if !w.prepare(&mut store) {
            break;
        }
    }
    let first: HashSet<usize> = store.keys.clone();
    assert!(!first.is_empty());
    let doc = PdfDocument::open(pdf(3), bo_pdfread::standard_fonts()).expect("pdf");
    w.open(doc, "second.pdf", None);
    for _ in 0..50 {
        if !w.prepare(&mut store) {
            break;
        }
    }
    assert!(store.keys.len() > first.len());
    w.run_command("file.close", &Fixed);
    assert_eq!(w.tab_count(), 1);
    w.prepare(&mut store);
    // Only the first tab's images are left.
    assert!(
        store.keys.iter().all(|k| first.contains(k)),
        "{:?}",
        store.keys
    );
    w.run_command("file.close", &Fixed);
    w.prepare(&mut store);
    assert!(store.keys.is_empty(), "{:?}", store.keys);
}

#[test]
fn one_page_documents_grey_the_multi_page_layouts() {
    let doc = PdfDocument::open(pdf(1), bo_pdfread::standard_fonts()).expect("pdf");
    let mut w = window();
    w.open(doc, "one.pdf", None);
    w.set_ribbon_tab(2, &Fixed);
    let grey = [
        "view.reverse",
        "view.continuous",
        "view.facing",
        "view.contFacing",
    ];
    for id in grey {
        assert!(!w.is_enabled(id), "{id}");
    }
    for id in ["view.single", "view.cover", "menu.split", "view.readMode"] {
        assert!(w.is_enabled(id), "{id}");
    }
    // The 3-page tab offers them all.
    w.run_command("file.close", &Fixed);
    for id in grey {
        assert!(w.is_enabled(id), "{id}");
    }
}

#[test]
fn the_document_tab_close_button_is_exposed_and_invokable() {
    let mut w = window();
    let items = w.accessibility12();
    assert!(items.iter().any(|a| a.key == "doc:close" && a.enabled));
    assert!(w.invoke12("doc:close", &Fixed));
    assert_eq!(w.tab_count(), 0);
    assert!(!w.invoke12("doc:close", &Fixed));
}

#[test]
fn a_new_fitted_zoom_rebuilds_the_page_lists() {
    // NoZoom notes and hairlines depend on the zoom: lists drawn for the first window size
    // must not be reused after it changes.
    let mut w = window();
    w.run_command("zoom.page", &Fixed);
    let mut store = Store::default();
    for _ in 0..20 {
        if !w.prepare(&mut store) {
            break;
        }
    }
    assert!(w.view().expect("view").cached_lists() > 0);
    w.handle(
        Input::Resize {
            width: 1100.0,
            height: 760.0,
        },
        &Fixed,
    );
    w.prepare(&mut store);
    let z = w.view().expect("view").zoom();
    assert!(w.view().expect("view").cached_lists() > 0);
    // The rebuilt lists were made at the new zoom: a second resize back changes it again.
    w.handle(
        Input::Resize {
            width: 1518.0,
            height: 1036.0,
        },
        &Fixed,
    );
    assert!((w.view().expect("view").zoom() - z).abs() > 1e-3);
    assert_eq!(w.view().expect("view").cached_lists(), 0);
}
