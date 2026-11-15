//! Edit Object behaviour: nudging, Shift-constrained and snapped moves, Ctrl+drag copies,
//! marquee selection, the Arrange section and their undo steps.
#![allow(clippy::manual_midpoint)]
use crate::editor_tests::{key, texts, window, Fixed, Rec};
use crate::editor_window::EditorWindow;
use crate::{Input, Key, Mods};

/// Boxes of the text lines of page 0 (page points).
fn lines(w: &mut EditorWindow) -> Vec<[f32; 4]> {
    let v = w.view_mut().expect("view");
    let objs = v.doc_mut().page_objects(0).expect("objects");
    bo_pdfread::text_lines(&objs)
        .into_iter()
        .map(|l| l.bbox)
        .collect()
}

fn to_window(w: &EditorWindow, x: f32, y: f32) -> (f32, f32) {
    let v = w.view().expect("view");
    let r = v.doc_rect_to_window(0, [x, y, x, y]).expect("on screen");
    (r[0], r[1])
}

fn press(w: &mut EditorWindow, x: f32, y: f32) {
    w.handle(
        Input::Press {
            x,
            y,
            count: 1,
            shift: false,
        },
        &Fixed,
    );
}

/// Drag from document point a to b with `mods` held.
fn drag(w: &mut EditorWindow, a: (f32, f32), b: (f32, f32), mods: Mods) {
    w.set_pointer_mods(mods);
    let (x0, y0) = to_window(w, a.0, a.1);
    let (x1, y1) = to_window(w, b.0, b.1);
    press(w, x0, y0);
    w.handle(
        Input::Drag {
            x: (x0 + x1) / 2.0,
            y: (y0 + y1) / 2.0,
        },
        &Fixed,
    );
    w.handle(Input::Drag { x: x1, y: y1 }, &Fixed);
    let mut rec = Rec::default();
    w.paint(&mut rec);
    w.handle(Input::Release { x: x1, y: y1 }, &Fixed);
    w.set_pointer_mods(Mods::default());
}

fn select_line(w: &mut EditorWindow) -> [f32; 4] {
    let b = lines(w)[0];
    let (x, y) = to_window(w, b[0] + 3.0, (b[1] + b[3]) / 2.0);
    press(w, x, y);
    w.handle(Input::Release { x, y }, &Fixed);
    b
}

#[test]
fn arrows_nudge_one_point_and_shift_ten_each_one_undo_step() {
    let mut w = window(1);
    w.run("ed.editObject", &Fixed);
    let b = select_line(&mut w);
    key(&mut w, Key::Right, false, false);
    key(&mut w, Key::Right, false, false);
    key(&mut w, Key::Down, false, true);
    let n = lines(&mut w)[0];
    assert!((n[0] - (b[0] + 2.0)).abs() < 0.1, "{n:?} {b:?}");
    assert!((n[1] - (b[1] + 10.0)).abs() < 0.1, "{n:?} {b:?}");
    // The line stays selected through the nudges.
    assert_eq!(w.edit.selected.len(), 1);
    key(&mut w, Key::Letter('z'), true, false);
    let u = lines(&mut w)[0];
    assert!((u[1] - b[1]).abs() < 0.1 && (u[0] - (b[0] + 2.0)).abs() < 0.1);
}

#[test]
fn shift_drag_keeps_the_line_and_ctrl_drag_copies() {
    let mut w = window(1);
    w.run("ed.editObject", &Fixed);
    let b = select_line(&mut w);
    let a = (b[0] + 3.0, (b[1] + b[3]) / 2.0);
    // 40 pt across, 15 down with Shift: only across.
    drag(
        &mut w,
        a,
        (a.0 + 40.0, a.1 + 15.0),
        Mods {
            shift: true,
            ..Mods::default()
        },
    );
    let n = lines(&mut w)[0];
    assert!((n[1] - b[1]).abs() < 0.1, "{n:?} {b:?}");
    assert!((n[0] - (b[0] + 40.0)).abs() < 0.1, "{n:?} {b:?}");
    // Ctrl+drag 100 pt down: a copy, the original stays.
    let a = (n[0] + 3.0, (n[1] + n[3]) / 2.0);
    drag(
        &mut w,
        a,
        (a.0, a.1 + 100.0),
        Mods {
            ctrl: true,
            ..Mods::default()
        },
    );
    let all = lines(&mut w);
    assert_eq!(all.len(), 2, "{all:?}");
    assert_eq!(texts(&mut w)[0].matches("Page 1").count(), 2);
    // The copy is selected; one undo removes it.
    assert_eq!(w.edit.selected.len(), 1);
    key(&mut w, Key::Letter('z'), true, false);
    assert_eq!(lines(&mut w).len(), 1);
}

#[test]
fn a_move_snaps_to_the_page_centre_with_a_guide() {
    let mut w = window(1);
    w.run("ed.editObject", &Fixed);
    let b = select_line(&mut w);
    let a = (b[0] + 3.0, (b[1] + b[3]) / 2.0);
    // Put the line's centre 2 pt left of the page's centre (306).
    let dx = 306.0 - 2.0 - (b[0] + b[2]) / 2.0;
    w.set_pointer_mods(Mods::default());
    let (x0, y0) = to_window(&w, a.0, a.1);
    let (x1, y1) = to_window(&w, a.0 + dx, a.1);
    press(&mut w, x0, y0);
    w.handle(Input::Drag { x: x1, y: y1 }, &Fixed);
    assert_ne!(w.edit.guides.len(), 0);
    w.handle(Input::Release { x: x1, y: y1 }, &Fixed);
    assert_eq!(w.edit.guides.len(), 0);
    let n = lines(&mut w)[0];
    assert!(((n[0] + n[2]) / 2.0 - 306.0).abs() < 0.2, "{n:?}");
}

#[test]
fn marquee_selects_and_arrange_centres_on_the_page() {
    let mut w = window(1);
    w.run("ed.editObject", &Fixed);
    let b = lines(&mut w)[0];
    // From empty space above-left of the line to its middle.
    drag(
        &mut w,
        (b[0] - 20.0, b[1] - 20.0),
        (b[0] + 10.0, b[1] + 5.0),
        Mods::default(),
    );
    assert_eq!(w.edit.selected.len(), 1);
    // The Format pane shows the Arrange section; Alignment needs two objects.
    let mut rec = Rec::default();
    w.paint(&mut rec);
    assert!(rec.texts.iter().any(|t| t == "Arrange"), "{:?}", rec.texts);
    assert!(rec.texts.iter().any(|t| t == "Page Center"));
    w.run("obj.alignLeft", &Fixed);
    assert!((lines(&mut w)[0][0] - b[0]).abs() < 0.1);
    w.run("obj.centerBoth", &Fixed);
    let n = lines(&mut w)[0];
    assert!(((n[0] + n[2]) / 2.0 - 306.0).abs() < 0.2, "{n:?}");
    assert!(((n[1] + n[3]) / 2.0 - 396.0).abs() < 0.2, "{n:?}");
    w.run("obj.rotateRight", &Fixed);
    let r = lines(&mut w);
    assert_ne!(r.len(), 0);
    key(&mut w, Key::Letter('z'), true, false);
    key(&mut w, Key::Letter('z'), true, false);
    assert!((lines(&mut w)[0][0] - b[0]).abs() < 0.1);
    // Ctrl+A selects every object; the context menu offers the Arrange commands.
    key(&mut w, Key::Letter('a'), true, false);
    assert_eq!(w.edit.selected.len(), 1);
    let (x, y) = to_window(&w, b[0], b[1]);
    w.handle(Input::Context { x, y }, &Fixed);
}
