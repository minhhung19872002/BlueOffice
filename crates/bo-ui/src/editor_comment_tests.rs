//! Headless tests of the editor's comment tools.
use crate::editor_tests::{window, Fixed, Rec};
use crate::editor_window::EditorWindow;
use crate::reader_window::ReaderRequest;
use crate::{Input, Key, Mods};
use bo_pdfread::AnnotKind;

fn at(w: &EditorWindow, x: f32, y: f32) -> (f32, f32) {
    let r = w
        .view()
        .and_then(|v| v.doc_rect_to_window(0, [x, y, x, y]))
        .expect("page");
    (r[0], r[1])
}

fn press(w: &mut EditorWindow, (x, y): (f32, f32), count: u8) {
    w.handle(
        Input::Press {
            x,
            y,
            count,
            shift: false,
        },
        &Fixed,
    );
    w.handle(Input::Release { x, y }, &Fixed);
}

fn drag(w: &mut EditorWindow, a: (f32, f32), b: (f32, f32)) {
    w.handle(
        Input::Press {
            x: a.0,
            y: a.1,
            count: 1,
            shift: false,
        },
        &Fixed,
    );
    w.handle(Input::Drag { x: b.0, y: b.1 }, &Fixed);
    let mut rec = Rec::default();
    w.paint(&mut rec);
    w.handle(Input::Release { x: b.0, y: b.1 }, &Fixed);
}

fn kinds(w: &EditorWindow) -> Vec<AnnotKind> {
    w.view()
        .map(|v| v.doc().annotations().iter().map(|a| a.kind).collect())
        .unwrap_or_default()
}

#[test]
fn comment_tools_add_comments() {
    let mut w = window(1);
    w.run("cm.rect", &Fixed);
    // The Format pane shows the comment properties.
    assert!(w.r.right_pane_rect()[2] > 0.0);
    w.run("fmt.cOpacity.50", &Fixed);
    w.run("fmt.cWidth.4", &Fixed);
    w.run("fmt.cColor.5", &Fixed);
    let mut rec = Rec::default();
    w.paint(&mut rec);
    assert!(
        rec.texts.iter().any(|t| t == "Appearance"),
        "{:?}",
        rec.texts
    );
    assert!(rec.texts.iter().any(|t| t == "50%"));
    let (a, b) = (at(&w, 100.0, 100.0), at(&w, 200.0, 160.0));
    drag(&mut w, a, b);
    assert_eq!(kinds(&w), [AnnotKind::Square]);
    let annot = w.view().expect("view").doc().annotations().remove(0);
    assert_eq!(annot.color, [0x1F, 0x4E, 0xA8]);
    w.run("cm.arrow", &Fixed);
    let (a, b) = (at(&w, 100.0, 300.0), at(&w, 250.0, 330.0));
    drag(&mut w, a, b);
    w.run("cm.pencil", &Fixed);
    let (a, b) = (at(&w, 300.0, 300.0), at(&w, 320.0, 340.0));
    w.handle(
        Input::Press {
            x: a.0,
            y: a.1,
            count: 1,
            shift: false,
        },
        &Fixed,
    );
    w.handle(
        Input::Drag {
            x: a.0 + 10.0,
            y: a.1 + 5.0,
        },
        &Fixed,
    );
    w.handle(Input::Drag { x: b.0, y: b.1 }, &Fixed);
    w.handle(Input::Release { x: b.0, y: b.1 }, &Fixed);
    // A cloud: three corners, a double click to finish.
    w.run("cm.cloud", &Fixed);
    for p in [(100.0, 380.0), (250.0, 380.0), (180.0, 450.0)] {
        let q = at(&w, p.0, p.1);
        press(&mut w, q, 1);
    }
    let q = at(&w, 180.0, 450.0);
    press(&mut w, q, 2);
    w.run("cm.stamp.1", &Fixed);
    let q = at(&w, 400.0, 100.0);
    press(&mut w, q, 1);
    assert_eq!(
        kinds(&w),
        [
            AnnotKind::Square,
            AnnotKind::Line,
            AnnotKind::Ink,
            AnnotKind::Line,
            AnnotKind::Stamp
        ]
    );
    // Typewriter text is typed in place (Enter: new line, Backspace) and becomes a comment
    // when a click or Escape ends it; nothing typed, no comment.
    w.run("cm.typewriter", &Fixed);
    let q = at(&w, 72.0, 470.0);
    press(&mut w, q, 1);
    assert!(w.dialog2.is_none() && w.tw.is_some());
    let before = kinds(&w).len();
    for s in ["Ghi chú", "x"] {
        w.handle(Input::Text(s.into()), &Fixed);
    }
    w.handle(Input::Key(Key::Backspace, Mods::default()), &Fixed);
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    w.handle(Input::Text("nhanh".into()), &Fixed);
    // Shortcuts do not reach the document while typing.
    w.handle(Input::Key(Key::Delete, Mods::default()), &Fixed);
    assert_eq!(
        w.tw.as_ref().map(|d| d.text.as_str()),
        Some(
            "Ghi chú
nhanh"
        )
    );
    assert_eq!(kinds(&w).len(), before);
    w.handle(Input::Key(Key::Escape, Mods::default()), &Fixed);
    assert!(w.tw.is_none());
    assert_eq!(kinds(&w).last(), Some(&AnnotKind::TextBox));
    let n = kinds(&w).len();
    let q = at(&w, 300.0, 470.0);
    press(&mut w, q, 1);
    press(&mut w, q, 1);
    assert_eq!(kinds(&w).len(), n, "an empty typewriter adds nothing");
    w.handle(Input::Key(Key::Escape, Mods::default()), &Fixed);
    // A file attachment asks for the file, then goes where it was clicked.
    w.run("cm.file", &Fixed);
    let q = at(&w, 500.0, 470.0);
    press(&mut w, q, 1);
    assert!(
        matches!(&w.take_requests()[..], [ReaderRequest::PickFile { purpose, .. }] if purpose == "attach")
    );
    w.file_picked("attach", std::path::Path::new("notes.txt"), b"hello");
    assert_eq!(kinds(&w).last(), Some(&AnnotKind::Other));
    let saved = w.view_mut().expect("view").doc_mut().save().expect("saves");
    assert!(String::from_utf8_lossy(&saved).contains("/CA 0.5"));
    // Hand ends the tool.
    w.run("tool.hand", &Fixed);
    assert!(w.comment.tool.is_none());
}
