//! Closing a changed document asks first (Yes / No / Cancel), as the reference editor.
use crate::dialog::DialogButton;
use crate::editor_actions::Ask2;
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
    match &w.dialog2 {
        Some((_, Ask2::Close(c))) => Some(*c),
        _ => None,
    }
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
    w.dialog2 = None;
    w.close_answer(Closing::Tab(0), DialogButton::Cancel);
    assert_eq!(w.r.tab_count(), 1);
    // No: closed without saving.
    w.run("file.close", &Fixed);
    assert_eq!(asked(&w), Some(Closing::Tab(0)));
    w.dialog2 = None;
    w.close_answer(Closing::Tab(0), DialogButton::Extra(3));
    assert_eq!(w.r.tab_count(), 0);
}

#[test]
fn yes_saves_then_closes_the_tab() {
    let mut w = window(2);
    change(&mut w);
    w.run("file.close", &Fixed);
    w.dialog2 = None;
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
    w.dialog2 = None;
    w.close_answer(Closing::Window(0), DialogButton::Extra(3));
    assert!(matches!(
        &w.take_requests()[..],
        [ReaderRequest::Window(Request::Close)]
    ));
    assert!(w.close_requested());
}
