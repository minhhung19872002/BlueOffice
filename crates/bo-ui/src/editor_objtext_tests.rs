//! The Format pane on objects selected with Edit Object: the text object's style, the
//! controls offered, whole-object changes, the Split and Effect sections.
use crate::editor_tests::{key, texts, window, Fixed, Rec};
use crate::editor_window::EditorWindow;
use crate::Input;

fn select_first(w: &mut EditorWindow) {
    let b = {
        let v = w.view_mut().expect("view");
        let objs = v.doc_mut().page_objects(0).expect("objects");
        objs[0].bbox
    };
    let v = w.view().expect("view");
    let r = v
        .doc_rect_to_window(0, {
            let (x, y) = (b[0] + 3.0, f32::midpoint(b[1], b[3]));
            [x, y, x, y]
        })
        .expect("on screen");
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
}

fn pane_cmds(w: &mut EditorWindow) -> Vec<&'static str> {
    let mut rec = Rec::default();
    w.paint(&mut rec);
    w.pane.hits.iter().map(|h| h.1).collect()
}

fn press_cmd(w: &mut EditorWindow, cmd: &str) {
    let r = w
        .pane
        .hits
        .iter()
        .find(|h| h.1 == cmd)
        .map(|h| h.0)
        .expect("button shown");
    let (x, y) = (r[0] + r[2] / 2.0, r[1] + r[3] / 2.0);
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
fn text_object_shows_its_style_and_offers_the_reference_controls() {
    let mut w = window(1);
    // Round 8: the Format pane starts collapsed; these tests use it opened.
    w.format_open8 = true;
    w.run("ed.editObject", &Fixed);
    select_first(&mut w);
    assert!(w.object_text_active());
    let (s, _) = w.object_text_style().expect("style of the object");
    assert!((s.size - 24.0).abs() < 0.1, "{}", s.size);
    assert_eq!(s.family, "Helvetica");
    assert!(!s.bold);
    let cmds = pane_cmds(&mut w);
    for on in [
        "fmt.font",
        "fmt.size",
        "fmt.color",
        "fmt.bold",
        "fmt.italic",
        "fmt.scale",
        "fmt.charSp",
    ] {
        assert!(cmds.contains(&on), "{on} offered: {cmds:?}");
    }
    for off in [
        "fmt.underline",
        "fmt.alignCenter",
        "fmt.lineSp",
        "fmt.wordSp",
        "fmt.bullets",
    ] {
        assert!(!cmds.contains(&off), "{off} grey: {cmds:?}");
    }
    // Split: a one-line object can only be converted to shapes.
    assert!(cmds.contains(&"obj.toShapes"));
    assert!(!cmds.contains(&"obj.mergeText") && !cmds.contains(&"obj.splitText"));
}

#[test]
fn bold_and_size_apply_to_the_whole_object_in_one_undo_step() {
    let mut w = window(1);
    // Round 8: the Format pane starts collapsed; these tests use it opened.
    w.format_open8 = true;
    w.run("ed.editObject", &Fixed);
    select_first(&mut w);
    pane_cmds(&mut w);
    press_cmd(&mut w, "fmt.bold");
    // Still selected, now bold.
    let (s, _) = w.object_text_style().expect("still selected");
    assert!(s.bold);
    w.run("fmt.size.36", &Fixed);
    let (s, _) = w.object_text_style().expect("still selected");
    assert!((s.size - 36.0).abs() < 0.1, "{}", s.size);
    assert_eq!(texts(&mut w), vec!["Page 1".to_owned()]);
    key(&mut w, crate::Key::Letter('z'), true, false);
    select_first(&mut w);
    let (s, _) = w.object_text_style().expect("selected");
    assert!(s.bold && (s.size - 24.0).abs() < 0.1, "{s:?}");
}

#[test]
fn graphics_show_effect_and_take_an_opacity() {
    let mut w = window(1);
    // Round 8: the Format pane starts collapsed; these tests use it opened.
    w.format_open8 = true;
    w.run("ed.editObject", &Fixed);
    select_first(&mut w);
    pane_cmds(&mut w);
    press_cmd(&mut w, "obj.toShapes");
    key(&mut w, crate::Key::Letter('a'), true, false);
    let cmds = pane_cmds(&mut w);
    assert!(
        cmds.contains(&"obj.opacity") && cmds.contains(&"obj.shear"),
        "{cmds:?}"
    );
    // The opacity button opens its menu.
    press_cmd(&mut w, "obj.opacity");
    let mut rec = Rec::default();
    w.paint(&mut rec);
    assert!(rec.texts.iter().any(|t| t == "50%"), "{:?}", rec.texts);
    key(&mut w, crate::Key::Escape, false, false);
    w.run("obj.opacity.50", &Fixed);
    let v = w.view_mut().expect("view");
    let objs = v.doc_mut().page_objects(0).expect("objects");
    let i = objs
        .iter()
        .position(|o| o.kind == bo_pdfread::ObjectKind::Path)
        .expect("path");
    assert!((v.doc_mut().object_opacity(0, i) - 0.5).abs() < 1e-3);
}

#[test]
fn convert_to_shapes_replaces_the_text() {
    let mut w = window(1);
    // Round 8: the Format pane starts collapsed; these tests use it opened.
    w.format_open8 = true;
    w.run("ed.editObject", &Fixed);
    select_first(&mut w);
    pane_cmds(&mut w);
    press_cmd(&mut w, "obj.toShapes");
    assert_eq!(texts(&mut w), vec![String::new()]);
    let v = w.view_mut().expect("view");
    let objs = v.doc_mut().page_objects(0).expect("objects");
    assert!(objs.iter().any(|o| o.kind == bo_pdfread::ObjectKind::Path));
}
