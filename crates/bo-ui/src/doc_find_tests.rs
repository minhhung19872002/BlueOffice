//! BlueDoc Find and Replace dialog (classic layout): tabs, history, search options.
use super::*;
use crate::doc_find;
use bo_engine::Engine;

struct Fixed;
impl TextMeasure for Fixed {
    fn measure(&self, text: &str, style: TextStyle) -> (f32, f32) {
        (
            text.chars().count() as f32 * style.size * 0.55,
            style.size * 1.33,
        )
    }
}

fn window() -> DocWindow {
    let mut engine = Engine::with_bundled_fonts();
    engine.new_document("a.docx").unwrap();
    let mut w = DocWindow::new(DocView::new(engine).unwrap(), "en-US", "a.docx");
    w.handle(
        Input::Resize {
            width: 1800.0,
            height: 900.0,
        },
        &Fixed,
    );
    w
}

fn press(w: &mut DocWindow, (x, y): (f32, f32)) {
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
}

fn ctrl(c: char) -> Input {
    Input::Key(
        Key::Letter(c),
        Mods {
            ctrl: true,
            ..Mods::default()
        },
    )
}

fn doc_text(w: &DocWindow) -> String {
    format!("{:?}", w.view.engine().document())
}

#[test]
fn replace_tab_finds_replaces_and_keeps_history() {
    let mut w = window();
    w.handle(Input::Text("Apple apple pie".into()), &Fixed);
    w.handle(ctrl('h'), &Fixed);
    assert_eq!(w.find_dialog.as_ref().map(|d| d.tab), Some(1));
    w.handle(Input::Text("apple".into()), &Fixed);
    w.handle(Input::Key(Key::Tab, Mods::default()), &Fixed);
    w.handle(Input::Text("pear".into()), &Fixed);
    // Enter is Find Next (the default button): two matches without Match case.
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    let msg = w.find_dialog.as_ref().map(|d| d.message.clone()).unwrap();
    assert!(msg.contains("1 of 2"), "{msg}");
    // More >> then Match case: one match.
    let more = w
        .find_dialog
        .as_ref()
        .and_then(|d| d.button_center(6))
        .expect("More");
    press(&mut w, more);
    assert!(w.find_dialog.as_ref().is_some_and(|d| d.more));
    let mc = w
        .find_dialog
        .as_ref()
        .and_then(FindDialog::match_case_center)
        .expect("Match case");
    press(&mut w, mc);
    assert!(w.find_dialog.as_ref().is_some_and(|d| d.match_case));
    let all = w
        .find_dialog
        .as_ref()
        .and_then(|d| d.button_center(9))
        .expect("Replace All");
    press(&mut w, all);
    let msg = w.find_dialog.as_ref().map(|d| d.message.clone()).unwrap();
    assert!(msg.starts_with('1'), "{msg}");
    let text = doc_text(&w);
    assert!(
        text.contains("pear") && text.contains("Apple"),
        "only the lower-case one"
    );
    assert!(!text.contains("apple"));
    // The searches are remembered for the ▾ list, most recent first.
    assert_eq!(w.find_history.first().map(String::as_str), Some("pear"));
    assert!(w.find_history.contains(&"apple".to_owned()));
    w.handle(Input::Key(Key::Escape, Mods::default()), &Fixed);
    assert!(w.find_dialog.is_none());
    // Reopened: the history drop-down fills Find what.
    w.handle(ctrl('h'), &Fixed);
    let drop = w.find_dialog.as_ref().map(FindDialog::drop_center).unwrap();
    press(&mut w, drop);
    let first = w
        .find_dialog
        .as_ref()
        .and_then(FindDialog::first_item)
        .expect("history list");
    press(&mut w, first);
    assert_eq!(
        w.find_dialog.as_ref().map(FindDialog::query).as_deref(),
        Some("pear")
    );
}

#[test]
fn tabs_switch_and_go_to_moves_to_a_page() {
    let mut w = window();
    for _ in 0..3 {
        w.view.run_command(Command::InsertPageBreak);
    }
    w.handle(ctrl('g'), &Fixed);
    assert_eq!(w.find_dialog.as_ref().map(|d| d.tab), Some(2));
    w.view.scroll_to(0.0);
    w.handle(Input::Text("3".into()), &Fixed);
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    let (scroll, _, _) = w.view.scroll_info();
    assert!(scroll > 0.0, "scrolled to page 3");
    // The tab strip and Ctrl+PageUp change tab.
    let find = w.find_dialog.as_ref().map(|d| d.tab_center(0)).unwrap();
    press(&mut w, find);
    assert_eq!(w.find_dialog.as_ref().map(|d| d.tab), Some(0));
    w.handle(
        Input::Key(
            Key::PageUp,
            Mods {
                ctrl: true,
                ..Mods::default()
            },
        ),
        &Fixed,
    );
    assert_eq!(w.find_dialog.as_ref().map(|d| d.tab), Some(2));
    let close = w
        .find_dialog
        .as_ref()
        .and_then(|d| d.button_center(12))
        .expect("Close");
    press(&mut w, close);
    assert!(w.find_dialog.is_none());
    assert_eq!(doc_find::HISTORY, 20);
}
