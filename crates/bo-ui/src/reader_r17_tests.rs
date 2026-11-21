//! Round 17: character-boundary drags with edge scrolling, cascading context menus, Arrange ›
//! Center and the Format pane for a selected note.
use super::{boundary17, span17};
use crate::reader_tests::{pdf, Fixed};
use crate::reader_window::{ReaderWindow, Tool};
use crate::{Input, Key, Mods};
use bo_pdfread::{AnnotKind, PdfDocument};

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

fn key(w: &mut ReaderWindow, k: Key) {
    w.handle(Input::Key(k, Mods::default()), &Fixed);
}

/// Window point of a fraction `f` across character `i` of page 0.
fn char_point(w: &mut ReaderWindow, i: usize, f: f32) -> (f32, f32) {
    let v = w.view().expect("view");
    let r = v.page_rect(0).expect("page");
    let s = v.px_per_pt();
    let layer = w.tabs[0].view.text(0).expect("text").clone();
    let c = &layer.chars()[i];
    (
        r[0] + (c.rect[0] + (c.rect[2] - c.rect[0]) * f) * s,
        r[1] + f32::midpoint(c.rect[1], c.rect[3]) * s,
    )
}

#[test]
fn boundaries_round_to_the_nearest_side_of_a_letter() {
    let mut w = window();
    let layer = w.tabs[0].view.text(0).expect("text").clone();
    let c = layer.chars()[2].rect;
    let y = f32::midpoint(c[1], c[3]);
    let at = |f: f32| boundary17(&layer, c[0] + (c[2] - c[0]) * f, y);
    assert_eq!(at(0.3), Some(2));
    assert_eq!(at(0.7), Some(3));
    assert_eq!(span17(0, 2, 5).map(|s| (s.a, s.b)), Some((2, 4)));
    assert_eq!(span17(0, 5, 2).map(|s| (s.a, s.b)), Some((2, 4)));
    assert!(span17(0, 3, 3).is_none());
}

#[test]
fn a_drag_selects_between_boundaries_like_the_reference() {
    // "Page 1 hello world": press in the right half of 'a', release in the left half of 'h'.
    let mut w = window();
    w.run_command("tool.select", &Fixed);
    let (x0, y0) = char_point(&mut w, 1, 0.75);
    let (x1, y1) = char_point(&mut w, 7, 0.25);
    w.handle(
        Input::Press {
            x: x0,
            y: y0,
            count: 1,
            shift: false,
        },
        &Fixed,
    );
    w.handle(Input::Drag { x: x1, y: y1 }, &Fixed);
    w.handle(Input::Release { x: x1, y: y1 }, &Fixed);
    let text = w.view().expect("view").selected_text().expect("selection");
    assert_eq!(text.trim_end(), "ge 1");
    // Back to the start: the anchor's letter stays out, the boundary is kept.
    let (x2, y2) = char_point(&mut w, 0, 0.2);
    w.handle(
        Input::Press {
            x: x0,
            y: y0,
            count: 1,
            shift: false,
        },
        &Fixed,
    );
    w.handle(Input::Drag { x: x2, y: y2 }, &Fixed);
    let text = w.view().expect("view").selected_text().expect("selection");
    assert_eq!(text, "Pa");
    // A drag that comes back to where it began selects nothing.
    w.handle(Input::Drag { x: x0, y: y0 }, &Fixed);
    assert!(w.view().expect("view").selection().is_none());
    w.handle(Input::Release { x: x0, y: y0 }, &Fixed);
}

#[test]
fn dragging_past_the_bottom_keeps_scrolling_until_release() {
    let mut w = window();
    w.run_command("tool.select", &Fixed);
    let (x0, y0) = char_point(&mut w, 1, 0.75);
    w.handle(
        Input::Press {
            x: x0,
            y: y0,
            count: 1,
            shift: false,
        },
        &Fixed,
    );
    let f = w.view().expect("view").frame();
    let below = f[1] + f[3] + 30.0;
    w.handle(Input::Drag { x: x0, y: below }, &Fixed);
    assert!(w.edge_scroll17());
    let y = w.view().expect("view").scroll_state().0 .1;
    assert!(w.edge_step17(100.0));
    let moved = w.view().expect("view").scroll_state().0 .1 - y;
    // 100 px/s at the edge plus 16.5 px/s for each of the 30 px: 59.5 px in 100 ms.
    assert!((moved - 59.5).abs() < 0.6, "{moved}");
    w.handle(Input::Release { x: x0, y: below }, &Fixed);
    assert!(!w.edge_scroll17());
    assert!(!w.edge_step17(100.0));
}

/// Put a note on page 0, select it and open its context menu.
fn note_menu(w: &mut ReaderWindow) -> (f32, f32) {
    w.run_command("tool.note", &Fixed);
    let r = w.view().expect("view").page_rect(0).expect("page");
    let (x, y) = (r[0] + 300.0, r[1] + 500.0);
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
    w.handle(Input::Text("Hi".into()), &Fixed);
    key(w, Key::Enter);
    w.run_command("tool.hand", &Fixed);
    let a = w
        .view()
        .expect("view")
        .doc()
        .annotations()
        .into_iter()
        .find(|a| a.kind == AnnotKind::Note)
        .expect("note");
    let s = w.view().expect("view").px_per_pt();
    let (cx, cy) = (
        r[0] + f32::midpoint(a.rect[0], a.rect[2]) * s,
        r[1] + f32::midpoint(a.rect[1], a.rect[3]) * s,
    );
    w.handle(Input::Context { x: cx, y: cy }, &Fixed);
    (cx, cy)
}

#[test]
fn the_comment_menu_cascades_like_the_reference() {
    let mut w = window();
    note_menu(&mut w);
    let labels = w.menu_labels9();
    let at = |l: &str| labels.iter().position(|x| x == l).expect(l);
    assert_eq!(at("Align"), 5);
    assert_eq!(at("Center"), 6);
    assert_eq!(at("Distribute"), 7);
    assert_eq!(at("Set Status"), 8);
    let dims = w.menu_dims9();
    assert!(dims.contains(&"Align".to_owned()) && dims.contains(&"Distribute".to_owned()));
    assert!(!dims.contains(&"Center".to_owned()));
    // Down to Center, then → opens its cascade with the first row highlighted.
    for _ in 0..7 {
        key(&mut w, Key::Down);
    }
    key(&mut w, Key::Right);
    let sub = w.sub_labels17();
    let names: Vec<&str> = sub.iter().map(|s| s.0.as_str()).collect();
    assert_eq!(names, ["Vertically", "Horizontally", "Both"]);
    // ← closes only the cascade; Down to Set Status and hovering it opens the seven states.
    key(&mut w, Key::Left);
    assert!(w.sub_labels17().is_empty());
    assert!(w.modal_open());
    let row = w.menu.as_ref().expect("menu").0.row_rect(8);
    w.handle(
        Input::Move {
            x: row[0] + 20.0,
            y: row[1] + row[3] / 2.0,
        },
        &Fixed,
    );
    let sub = w.sub_labels17();
    let names: Vec<&str> = sub.iter().map(|s| s.0.as_str()).collect();
    assert_eq!(
        names,
        [
            "None",
            "Accepted",
            "Rejected",
            "Cancelled",
            "Completed",
            "Deferred",
            "Future"
        ]
    );
    assert!(sub[0].2, "None is ticked");
    // The cascade sits at the menu's right edge (1 px overlap), its top 2 px above the row.
    let (mr, sr) = (
        w.menu.as_ref().expect("menu").0.rect(),
        w.sub_rect17().expect("cascade"),
    );
    assert!((sr[0] - (mr[0] + mr[2] - 1.0)).abs() < 0.5 || sr[0] + sr[2] <= mr[0] + 1.0);
    assert!((sr[1] - (row[1] - 2.0)).abs() < 0.5, "{sr:?} {row:?}");
    // Choosing Deferred sets the review state and closes both menus.
    let r = w.sub_row17(5).expect("Deferred row");
    let (x, y) = (r[0] + 20.0, r[1] + r[3] / 2.0);
    w.handle(Input::Move { x, y }, &Fixed);
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
    assert!(!w.modal_open());
    let a = w.extra.selected.clone().expect("selected");
    let state = w.view().expect("view").doc().comment_state(&a);
    assert_eq!(state.review.as_deref(), Some("Deferred"));
}

#[test]
fn center_moves_the_note_to_the_middle_and_the_pane_shows() {
    let mut w = window();
    note_menu(&mut w);
    key(&mut w, Key::Escape);
    // A selected note shows the Format pane (Arrange › Page Center on).
    assert!(w.note_selected17());
    w.handle(Input::Tick, &Fixed);
    assert!(w.right_pane_rect()[2] > 0.0);
    assert!(w.command17("arrange17.center.both"));
    let a = w.extra.selected.clone().expect("still selected");
    let mid = |p: f32, q: f32| f32::midpoint(p, q);
    assert!(
        (mid(a.rect[0], a.rect[2]) - 306.0).abs() < 0.6,
        "{:?}",
        a.rect
    );
    assert!(
        (mid(a.rect[1], a.rect[3]) - 396.0).abs() < 0.6,
        "{:?}",
        a.rect
    );
    // Another tool hides the pane again.
    w.run_command("tool.select", &Fixed);
    w.extra.selected = None;
    w.handle(Input::Tick, &Fixed);
    assert_eq!(w.tool, Tool::Select);
    assert!(w.right_pane_rect()[2] <= 0.0);
}

#[test]
fn bookmark_menu_goes_and_prints_its_section() {
    use bo_pdfread::{LinkTarget, OutlineItem};
    let mut w = window();
    let mark = |title: &str, index: usize, children| OutlineItem {
        title: title.into(),
        target: Some(LinkTarget::Page {
            index,
            top: Some(0.0),
        }),
        open: false,
        children,
    };
    w.tabs[0].outline = vec![
        mark("One", 0, vec![mark("One.a", 2, Vec::new())]),
        mark("Two", 5, Vec::new()),
    ];
    w.selected_outline = Some(vec![0]);
    w.bookmark_menu17(200.0, 400.0, &Fixed);
    assert_eq!(
        w.menu_labels9(),
        [
            "Go to Bookmark",
            "Print Page(s)",
            "Print Section(s)",
            "Expand/Collapse All Bookmarks"
        ]
    );
    w.close_menu();
    // The section runs to the page before the next bookmark outside it.
    w.run_command("bm17.printSection", &Fixed);
    assert_eq!(w.r17.print_pages.as_deref(), Some("1 - 5"));
    w.selected_outline = Some(vec![1]);
    w.run_command("bm17.printSection", &Fixed);
    assert_eq!(w.r17.print_pages.as_deref(), Some("6 - 12"));
    w.run_command("bm17.printPages", &Fixed);
    assert_eq!(w.r17.print_pages.as_deref(), Some("6"));
    w.run_command("bm17.go", &Fixed);
    assert_eq!(w.view().expect("view").current_page(), 5);
    // Expand/Collapse All toggles.
    w.run_command("bm17.toggleAll", &Fixed);
    assert!(w.tabs[0].expanded.contains(&vec![0]));
    w.run_command("bm17.toggleAll", &Fixed);
    assert!(w.tabs[0].expanded.is_empty());
}

#[test]
fn menus_are_menu_items_for_assistive_technology() {
    use crate::AccessRole;
    let mut w = window();
    note_menu(&mut w);
    let items = w.accessibility17();
    let rows: Vec<_> = items
        .iter()
        .filter(|i| i.role == AccessRole::MenuItem)
        .collect();
    assert!(items.iter().any(|i| i.role == AccessRole::Menu));
    assert_eq!(rows[6].name, "Center");
    assert!(!rows[5].enabled, "Align is dim");
    // Invoking Center opens its cascade; Both centres the note and closes the menus.
    assert!(w.invoke17("menuitem:6", &Fixed));
    assert_eq!(w.sub_labels17().len(), 3);
    assert_eq!(w.access_focus17().as_deref(), Some("sub"));
    assert!(w.invoke17("subitem:2", &Fixed));
    assert!(!w.modal_open());
    let a = w.extra.selected.clone().expect("selected");
    assert!((f32::midpoint(a.rect[0], a.rect[2]) - 306.0).abs() < 0.6);
    // A dialog is a Window with its buttons.
    w.run_command("tool.note", &Fixed);
    let r = w.view().expect("view").page_rect(0).expect("page");
    w.handle(
        Input::Press {
            x: r[0] + 100.0,
            y: r[1] + 100.0,
            count: 1,
            shift: false,
        },
        &Fixed,
    );
    let items = w.accessibility17();
    assert!(items.iter().any(|i| i.role == AccessRole::Window));
    assert!(items.iter().any(|i| i.key.starts_with("dialogbtn:")));
    assert_eq!(w.access_focus17().as_deref(), Some("dialog"));
}
