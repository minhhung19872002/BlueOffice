//! Closing a changed document asks first (Yes / No / Cancel), as the reference editor.
use crate::dialog::DialogButton;
use crate::editor_close::Closing;
use crate::editor_tests::{key, window, Fixed};
use crate::reader_window::ReaderRequest;
use crate::Key;
use crate::Request;

fn change(w: &mut crate::editor_window::EditorWindow) {
    w.view_mut()
        .expect("view")
        .doc_mut()
        .rotate_pages(&[0], 1)
        .expect("rotates");
}

fn asked(w: &crate::editor_window::EditorWindow) -> Option<Closing> {
    w.close_box.as_ref().map(|b| b.what)
}

#[test]
fn unchanged_tab_closes_without_a_question() {
    let mut w = window(2);
    assert_eq!(w.r.tab_count(), 1);
    w.run("file.close", &Fixed);
    assert_eq!(w.r.tab_count(), 0);
    assert!(asked(&w).is_none());
    assert!(w.close_requested());
}

#[test]
fn changed_tab_asks_and_cancel_keeps_it() {
    let mut w = window(2);
    change(&mut w);
    key(&mut w, Key::Letter('w'), true, false);
    assert_eq!(asked(&w), Some(Closing::Tab(0)));
    assert_eq!(w.r.tab_count(), 1);
    w.close_box = None;
    w.close_answer(Closing::Tab(0), DialogButton::Cancel);
    assert_eq!(w.r.tab_count(), 1);
    // No: closed without saving.
    w.run("file.close", &Fixed);
    assert_eq!(asked(&w), Some(Closing::Tab(0)));
    w.close_box = None;
    w.close_answer(Closing::Tab(0), DialogButton::Extra(3));
    assert_eq!(w.r.tab_count(), 0);
}

#[test]
fn yes_saves_then_closes_the_tab() {
    let mut w = window(2);
    change(&mut w);
    w.run("file.close", &Fixed);
    w.close_box = None;
    w.close_answer(Closing::Tab(0), DialogButton::Ok);
    assert!(matches!(&w.take_requests()[..], [ReaderRequest::Save]));
    // The host saves: the document is no longer changed, the tab closes on the next frame.
    let _ = w.view_mut().expect("view").doc_mut().save().expect("saves");
    assert!(w.close_pending());
    assert_eq!(w.r.tab_count(), 0);
}

#[test]
fn window_close_asks_for_each_changed_document() {
    let mut w = window(2);
    change(&mut w);
    assert!(!w.close_requested());
    assert_eq!(asked(&w), Some(Closing::Window(0)));
    w.close_box = None;
    w.close_answer(Closing::Window(0), DialogButton::Extra(3));
    assert!(matches!(
        &w.take_requests()[..],
        [ReaderRequest::Window(Request::Close)]
    ));
    assert!(w.close_requested());
}

/// Select opens the reference's two-entry menu and turns the select tool on.
#[test]
fn select_has_the_reference_menu() {
    let mut w = window(1);
    w.run("ed.selectMenu", &Fixed);
    assert_eq!(w.r.current_tool(), crate::reader_window::Tool::Select);
    let mut p = crate::editor_tests::Rec::default();
    w.paint(&mut p);
    for label in ["Select Text and Image", "Select Annotation"] {
        assert!(p.texts.iter().any(|t| t == label), "{label}: {:?}", p.texts);
    }
    assert!(!p.texts.iter().any(|t| t == "Select Area"));
}

/// The question is the reference's message box: Yes / No / Cancel pills, keys answer it.
#[test]
fn close_question_box_buttons_and_keys() {
    let mut w = window(2);
    change(&mut w);
    w.run("file.close", &Fixed);
    let mut p = crate::editor_tests::Rec::default();
    w.paint(&mut p);
    for label in ["Yes", "No", "Cancel", "BlueEditor"] {
        assert!(p.texts.iter().any(|t| t == label), "{label}: {:?}", p.texts);
    }
    assert!(p
        .texts
        .iter()
        .any(|t| t == "Do you want to save changes to 'test.pdf' before closing?"));
    // Other input does not reach the document while it is shown; Escape cancels.
    key(&mut w, Key::Delete, false, false);
    assert!(asked(&w).is_some());
    key(&mut w, Key::Escape, false, false);
    assert!(asked(&w).is_none());
    assert_eq!(w.r.tab_count(), 1);
    // Clicking No closes without saving.
    w.run("file.close", &Fixed);
    let mut p = crate::editor_tests::Rec::default();
    w.paint(&mut p);
    let no = w.close_box.as_ref().map(|b| b.button(1)).expect("box");
    w.handle(
        crate::Input::Press {
            x: no[0] + 4.0,
            y: no[1] + 4.0,
            count: 1,
            shift: false,
        },
        &Fixed,
    );
    assert_eq!(w.r.tab_count(), 0);
}
