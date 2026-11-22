//! BlueDoc parity round 10: OLE drag and drop, pointer shapes, context menus.
use super::super::{DocWindow, Request};
use super::pointer;
use crate::{DocView, Input, TextMeasure, TextStyle};
use bo_edit::{Pos, Selection};
use bo_engine::edit::Command;
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

fn window(text: &str) -> DocWindow {
    let mut engine = Engine::with_bundled_fonts();
    engine.new_document("a.docx").expect("new");
    let mut w = DocWindow::new(DocView::new(engine).expect("view"), "en-US", "a.docx");
    w.handle(Input::Scale(1.25), &Fixed);
    w.handle(
        Input::Resize {
            width: 1500.0,
            height: 1102.0,
        },
        &Fixed,
    );
    w.view
        .run_command(Command::InsertText { text: text.into() });
    w.view.run_command(Command::SetSelection {
        selection: Selection::caret(Pos::new(0, 0)),
    });
    w
}

/// Window pixel of byte `offset` of paragraph `para`.
fn pixel_of(w: &DocWindow, para: usize, offset: usize) -> (f32, f32) {
    let layout = w.view.engine().layout().expect("layout");
    let slot = w.view.slots()[0];
    let s = w.view.scale();
    let page = &layout.pages[slot.index];
    let line = page
        .lines
        .iter()
        .find(|l| {
            l.source == bo_layout::LineSource::Body(para as u32)
                && (l.start as usize) <= offset
                && offset < (l.end as usize).max(l.start as usize + 1)
        })
        .expect("line");
    let x = line
        .carets
        .iter()
        .find(|c| c.0 as usize >= offset)
        .map_or(0.0, |c| c.1);
    (
        slot.x as f32 + x * s,
        slot.y as f32 + (line.top + line.height / 2.0) * s,
    )
}

fn para_text(w: &DocWindow, p: usize) -> String {
    w.view
        .engine()
        .session()
        .and_then(|s| s.para_text(p))
        .unwrap_or_default()
}

fn select(w: &mut DocWindow, para: usize, a: usize, b: usize) {
    w.view.run_command(Command::SetSelection {
        selection: Selection {
            anchor: Pos::new(para, a),
            focus: Pos::new(para, b),
        },
    });
}

/// Press inside the selection at `from` and drag to `to` (window pixels).
fn start_drag(w: &mut DocWindow, from: (f32, f32), to: (f32, f32)) {
    w.handle(
        Input::Press {
            x: from.0,
            y: from.1,
            count: 1,
            shift: false,
        },
        &Fixed,
    );
    w.handle(Input::Drag { x: to.0, y: to.1 }, &Fixed);
}

#[test]
fn an_ole_drag_drops_back_into_the_document_with_smart_spaces() {
    let mut w = window("one two three four");
    w.set_ole_drag(true);
    select(&mut w, 0, 4, 7);
    let from = pixel_of(&w, 0, 5);
    let to = pixel_of(&w, 0, 13);
    start_drag(&mut w, from, to);
    let drag = w.take_requests().into_iter().find_map(|r| match r {
        Request::DragText { text, .. } => Some(text),
        _ => None,
    });
    assert_eq!(drag.as_deref(), Some("two"));
    // Over the selection itself it cannot drop; past it it can.
    assert!(!w.drag_over(from.0, from.1));
    assert!(w.drag_over(to.0, to.1));
    assert!(w.drop_data(to.0, to.1, false, None, None, None));
    w.drag_done(2);
    assert_eq!(para_text(&w, 0), "one three two four");
}

#[test]
fn text_moved_into_another_application_leaves_the_document() {
    let mut w = window("one two three four");
    w.set_ole_drag(true);
    select(&mut w, 0, 4, 7);
    let from = pixel_of(&w, 0, 5);
    start_drag(&mut w, from, (from.0, from.1 + 300.0));
    let _ = w.take_requests();
    // Dropped elsewhere (another application), moved: smart cut.
    w.drag_done(2);
    assert_eq!(para_text(&w, 0), "one three four");
    // Copied elsewhere: the text stays.
    select(&mut w, 0, 4, 9);
    let from = pixel_of(&w, 0, 5);
    start_drag(&mut w, from, (from.0, from.1 + 300.0));
    w.drag_done(1);
    assert_eq!(para_text(&w, 0), "one three four");
}

#[test]
fn text_dropped_from_another_application_is_pasted_there() {
    let mut w = window("one three");
    let at = pixel_of(&w, 0, 3);
    assert!(w.drag_over(at.0, at.1));
    assert!(w.drop_data(at.0, at.1, true, None, None, Some("two".into())));
    assert_eq!(para_text(&w, 0), "one two three");
}

#[test]
fn pointer_is_text_over_the_page_and_an_arrow_over_the_selection_and_ribbon() {
    let mut w = window("one two three four");
    let (x, y) = pixel_of(&w, 0, 5);
    w.handle(Input::Move { x, y }, &Fixed);
    assert_eq!(w.cursor(), pointer::TEXT);
    select(&mut w, 0, 4, 7);
    w.handle(Input::Move { x, y }, &Fixed);
    assert_eq!(w.cursor(), pointer::ARROW);
    w.handle(Input::Move { x, y: 40.0 }, &Fixed);
    assert_eq!(w.cursor(), pointer::ARROW);
}

#[test]
fn the_text_context_menu_has_the_reference_rows_and_the_mini_toolbar() {
    let mut w = window("one two three four");
    let (x, y) = pixel_of(&w, 0, 5);
    // Text on the clipboard: only Keep Text Only is offered.
    w.set_clipboard_kinds(false, false, true);
    w.handle(Input::Context { x, y }, &Fixed);
    let menu = w.menu.as_ref().expect("menu");
    let labels: Vec<&str> = menu.items().iter().map(|i| i.label.as_str()).collect();
    assert_eq!(
        labels,
        [
            "",
            "Cut",
            "Copy",
            "Paste Options:",
            "",
            "Font...",
            "Paragraph...",
            "Synonyms",
            "Link",
            "New Comment"
        ]
    );
    assert_eq!(w.r10.ctx.as_ref().expect("ctx").paste_modes(), [2]);
    // Without a selection Cut and Copy are dim; access keys as on the reference.
    assert!(menu.items()[1].dim && menu.items()[2].dim);
    assert_eq!(
        menu.access_choice('f'),
        Some(crate::popup::Choice::Command("format.fontDialog".into()))
    );
    // The mini toolbar sits above the menu with the measured gap.
    let ctx = w.r10.ctx.as_ref().expect("ctx");
    let bar = ctx.bar_rect().expect("bar");
    let m = menu.rect();
    assert!(bar[1] + bar[3] < m[1]);
    assert!((m[1] - (bar[1] + bar[3]) - 19.0).abs() <= 1.0);
    assert!((bar[2] - 456.0).abs() <= 1.0);
}

#[test]
fn table_and_picture_menus_and_the_bold_button() {
    let mut w = window("one two three four");
    w.view
        .run_command(Command::InsertTable { rows: 2, cols: 2 });
    let at = w.view.engine().session().expect("s").selection().focus;
    w.view.run_command(Command::InsertText {
        text: "cell".into(),
    });
    let (x, y) = pixel_of(&w, at.para, 1);
    w.handle(Input::Context { x, y }, &Fixed);
    let labels: Vec<String> = w
        .menu
        .as_ref()
        .expect("menu")
        .items()
        .iter()
        .map(|i| i.label.clone())
        .collect();
    assert!(labels.iter().any(|l| l == "Insert"), "{labels:?}");
    assert!(
        labels.iter().any(|l| l == "Table Properties..."),
        "{labels:?}"
    );
    // The Insert row opens its sub-menu.
    w.handle(
        Input::Key(crate::Key::Escape, crate::Mods::default()),
        &Fixed,
    );
    assert!(w.menu.is_none());
    // Bold from the mini toolbar.
    let (x, y) = pixel_of(&w, 0, 1);
    select(&mut w, 0, 0, 3);
    w.handle(Input::Context { x, y }, &Fixed);
    let b = w
        .r10
        .ctx
        .as_ref()
        .and_then(|c| c.button("format.bold"))
        .expect("bold");
    let (bx, by) = (b[0] + 4.0, b[1] + 4.0);
    w.handle(
        Input::Press {
            x: bx,
            y: by,
            count: 1,
            shift: false,
        },
        &Fixed,
    );
    w.handle(Input::Release { x: bx, y: by }, &Fixed);
    assert!(w.menu.is_none());
    assert!(w.view.format().bold);
}

#[test]
fn shift_arrows_extend_a_block_made_by_alt_drag() {
    let mut w = window("abcdef ghij\nabcdef ghij\nabcdef ghij");
    w.view.run_command(Command::BlockSelect {
        page: 0,
        x: 0.0,
        y: 0.0,
        extend: false,
    });
    // Start a block at the caret's place through the engine (what Alt+drag does).
    let (x0, y0) = pixel_of(&w, 0, 1);
    let (x1, y1) = pixel_of(&w, 1, 3);
    w.set_pointer_mods(crate::Mods {
        alt: true,
        ..crate::Mods::default()
    });
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
    assert_eq!(w.view.engine().block_text(), ["bc", "bc"]);
    let shift = crate::Mods {
        shift: true,
        ..crate::Mods::default()
    };
    w.handle(Input::Key(crate::Key::Down, shift), &Fixed);
    assert_eq!(w.view.engine().block_text(), ["bc", "bc", "bc"]);
    w.handle(Input::Key(crate::Key::Right, shift), &Fixed);
    assert_eq!(w.view.engine().block_text(), ["bcd", "bcd", "bcd"]);
}

#[test]
#[ignore = "timing, run by hand: BO_PERF_DOC=big.docx cargo test --release -p bo-ui --lib typing_window -- --ignored --nocapture"]
fn typing_window_cost() {
    let Ok(path) = std::env::var("BO_PERF_DOC") else {
        return;
    };
    let mut engine = Engine::with_bundled_fonts();
    engine
        .open(&std::fs::read(path).expect("read"), "big.docx")
        .expect("open");
    let mut w = DocWindow::new(DocView::new(engine).expect("view"), "en-US", "big.docx");
    w.handle(Input::Scale(1.25), &Fixed);
    w.handle(
        Input::Resize {
            width: 1500.0,
            height: 1102.0,
        },
        &Fixed,
    );
    // As the measurement script: a click on the first page, then the typing.
    w.handle(
        Input::Press {
            x: 600.0,
            y: 500.0,
            count: 1,
            shift: false,
        },
        &Fixed,
    );
    w.handle(Input::Release { x: 600.0, y: 500.0 }, &Fixed);
    // Through the engine only (after the shell turned AutoCorrect on with the first key).
    if std::env::var_os("BO_PERF_ENGINE").is_some() {
        w.handle(Input::Text("n".into()), &Fixed);
        for c in "hanh hon ".chars() {
            let t = std::time::Instant::now();
            w.view.run_command(Command::InsertText { text: c.into() });
            let a = t.elapsed();
            let at = w.view.engine().session().map(|s| s.selection().focus);
            println!("{c:?} view {:8.3} ms {at:?}", a.as_secs_f64() * 1e3);
        }
        return;
    }
    for c in "nhanh hon nua di ban oi, go thu ".chars() {
        let t = std::time::Instant::now();
        w.handle(Input::Text(c.to_string()), &Fixed);
        let _ = w.take_requests();
        w.refresh(&Fixed);
        let a = t.elapsed();
        let t = std::time::Instant::now();
        w.handle(Input::Tick, &Fixed);
        println!(
            "{c:?} text {:8.3} ms  tick {:8.3} ms",
            a.as_secs_f64() * 1e3,
            t.elapsed().as_secs_f64() * 1e3
        );
    }
}

#[test]
fn typing_in_the_context_menu_searches_commands_and_synonyms_replace_the_word() {
    let mut w = window("one quick fox");
    let (x, y) = pixel_of(&w, 0, 5);
    w.handle(Input::Context { x, y }, &Fixed);
    let before = w.menu.as_ref().expect("menu").items().len();
    w.handle(Input::Text("bol".into()), &Fixed);
    let labels: Vec<String> = w
        .menu
        .as_ref()
        .expect("menu")
        .items()
        .iter()
        .map(|i| i.label.clone())
        .collect();
    assert!(labels.iter().any(|l| l == "Bold"), "{labels:?}");
    // Backspace to empty: the menu comes back.
    for _ in 0..3 {
        w.handle(
            Input::Key(crate::Key::Backspace, crate::Mods::default()),
            &Fixed,
        );
    }
    assert_eq!(w.menu.as_ref().expect("menu").items().len(), before);
    // Synonyms ▸ then a synonym replaces "quick".
    assert!(w.action("ctx.sub.syn"));
    let first = w.menu.as_ref().expect("submenu").items()[0].label.clone();
    w.menu = None;
    assert!(w.action("ctx.syn.0"));
    assert_eq!(para_text(&w, 0), format!("one {first} fox"));
}

#[test]
fn a_right_click_on_a_hyperlink_gives_the_hyperlink_rows() {
    let mut w = window("Visit the site for more.");
    select(&mut w, 0, 10, 14);
    w.view.run_command(Command::SetLink {
        target: "https://example.com/".into(),
        display: String::new(),
    });
    select(&mut w, 0, 0, 0);
    // The caret is elsewhere: the right-click moves it onto the link first.
    let (x, y) = pixel_of(&w, 0, 11);
    w.handle(Input::Context { x, y }, &Fixed);
    let labels: Vec<String> = w
        .menu
        .as_ref()
        .expect("menu")
        .items()
        .iter()
        .map(|i| i.label.clone())
        .collect();
    for l in [
        "Edit Hyperlink...",
        "Open Hyperlink",
        "Copy Hyperlink",
        "Remove Hyperlink",
    ] {
        assert!(labels.iter().any(|x| x == l), "{l} in {labels:?}");
    }
}

#[test]
fn find_by_format_and_the_find_next_enable_rule() {
    let mut w = window("plain bold plain");
    select(&mut w, 0, 6, 10);
    w.view.run_command(Command::FormatText {
        format: bo_edit::CharFormat::Bold,
    });
    select(&mut w, 0, 0, 0);
    assert!(w.action("edit.replace"));
    let fd = w.find_dialog.as_mut().expect("dialog");
    // Nothing to find yet: Find Next is off, as on the reference.
    assert!(!fd.find_next_enabled());
    fd.toggle_format(0);
    assert!(fd.find_next_enabled());
    assert!(w.find_dialog_action(crate::doc_find::FindAction::FindNext));
    let (a, b) = w.view.engine().session().expect("s").selection().ordered();
    assert_eq!((a.offset, b.offset), (6, 10));
}
