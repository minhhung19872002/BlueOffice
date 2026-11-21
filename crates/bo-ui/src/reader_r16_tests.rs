//! Round 16: Shift+Space, Ctrl+Shift+N and the note Format pane, triple click, auto-scroll and
//! the page text for UI Automation.
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

fn scroll_y(w: &ReaderWindow) -> f32 {
    w.view().expect("view").scroll_state().0 .1
}

fn key(w: &mut ReaderWindow, k: Key, ctrl: bool, shift: bool) {
    w.handle(
        Input::Key(
            k,
            Mods {
                shift,
                ctrl,
                alt: false,
            },
        ),
        &Fixed,
    );
}

#[test]
fn shift_space_pages_up_and_space_down() {
    let mut w = window();
    let h = w.view().expect("view").frame()[3];
    w.handle(Input::Text(" ".into()), &Fixed);
    w.handle(Input::Text(" ".into()), &Fixed);
    let y2 = scroll_y(&w);
    // The host reports Shift+Space on key down, then the space text arrives.
    key(&mut w, Key::Letter(' '), false, true);
    w.handle(Input::Text(" ".into()), &Fixed);
    assert!(
        (y2 - scroll_y(&w) - (h - 41.0)).abs() < 0.6,
        "{}",
        scroll_y(&w)
    );
    // A plain space after it pages down again (the Shift state is not kept).
    w.handle(Input::Text(" ".into()), &Fixed);
    assert!((scroll_y(&w) - y2).abs() < 0.6);
    // Another key between them cancels it.
    key(&mut w, Key::Letter(' '), false, true);
    key(&mut w, Key::Down, false, false);
    let y = scroll_y(&w);
    w.handle(Input::Text(" ".into()), &Fixed);
    assert!(scroll_y(&w) > y);
}

#[test]
fn ctrl_shift_n_picks_the_note_tool_with_its_format_pane() {
    let mut w = window();
    let width = w.view().expect("view").frame()[2];
    key(&mut w, Key::Letter('n'), true, true);
    assert_eq!(w.tool, Tool::Note);
    let pane = w.right_pane_rect();
    // 476 px at 125 %; the document narrows by as much.
    assert!((pane[2] - 476.0).abs() < 1.0, "{pane:?}");
    // The scroll bar meets the pane: the document narrows by 476 px less its 8 px margin.
    assert!((width - w.view().expect("view").frame()[2] - 468.0).abs() < 1.5);
    // Pick the key icon, red, 50 % opacity, then place a note.
    let (x0, y0) = (pane[0], pane[1]);
    let px = |v: f32| (v * 1.25_f32).round();
    let click = |w: &mut ReaderWindow, x: f32, y: f32| {
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
    };
    click(&mut w, x0 + px(198.4), y0 + px(232.8));
    click(&mut w, x0 + px(300.0), y0 + px(313.6));
    click(&mut w, x0 + px(130.4) + px(124.8) / 2.0, y0 + px(395.2));
    assert_eq!(w.r16.style.icon, 9);
    assert_eq!(w.r16.style.colour, [255, 0, 0]);
    assert!((w.r16.style.opacity as i32 - 50).abs() <= 1);
    let page = w.view().expect("view").page_rect(0).expect("page");
    click(&mut w, page[0] + 100.0, page[1] + 100.0);
    // The note's text dialog: type and confirm.
    w.handle(Input::Text("Hi".into()), &Fixed);
    key(&mut w, Key::Enter, false, false);
    let doc = w.view().expect("view").doc();
    let a = doc
        .annotations()
        .into_iter()
        .find(|a| a.kind == AnnotKind::Note)
        .expect("note");
    assert_eq!(a.color, [255, 0, 0]);
    let (icon, alpha) = doc.note_style(&a).expect("style");
    assert_eq!(icon, "Key");
    assert!((alpha - 0.5).abs() < 0.02, "{alpha}");
    // × closes the pane; picking another tool hides it too.
    click(&mut w, x0 + px(84.8), y0 + px(17.6));
    assert!(w.right_pane_rect()[2] <= 0.0);
    key(&mut w, Key::Letter('n'), true, true);
    assert!(w.right_pane_rect()[2] > 0.0);
    w.run_command("tool.hand", &Fixed);
    w.handle(Input::Tick, &Fixed);
    assert!(w.right_pane_rect()[2] <= 0.0);
}

#[test]
fn triple_click_selects_the_line() {
    let mut w = window();
    w.run_command("tool.select", &Fixed);
    // Find a character on the first page and click three times on it.
    let v = w.view().expect("view");
    let r = v.page_rect(0).expect("page");
    let s = v.px_per_pt();
    let _ = v;
    let layer = w.tabs[0].view.text(0).expect("text").clone();
    let c = &layer.chars()[3];
    let (x, y) = (
        r[0] + f32::midpoint(c.rect[0], c.rect[2]) * s,
        r[1] + f32::midpoint(c.rect[1], c.rect[3]) * s,
    );
    for count in 1..=3 {
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
    let text = w.view().expect("view").selected_text().expect("selection");
    let first_line = layer
        .text()
        .lines()
        .next()
        .unwrap_or_default()
        .trim()
        .to_owned();
    assert_eq!(text.trim(), first_line);
}

#[test]
fn auto_scroll_has_a_dead_zone_then_follows_the_distance() {
    let mut w = window();
    let y0 = scroll_y(&w);
    assert!(!w.autoscroll_step16(100.0, 100.0), "not started");
    assert!(w.set_autoscroll16(Some((700.0, 600.0))));
    assert!(!w.autoscroll_step16(20.0, 100.0));
    assert!(w.autoscroll_step16(128.0, 100.0));
    // 100 px past the dead zone for 0.1 s: about 165 px.
    assert!((scroll_y(&w) - y0 - 165.0).abs() < 2.0, "{}", scroll_y(&w));
    assert!(w.set_autoscroll16(None));
}

#[test]
fn page_text_and_focus_for_assistive_technology() {
    let mut w = window();
    assert_eq!(w.access_focus16().as_deref(), Some("doc"));
    let t = w.access_text16().expect("text");
    assert_ne!(t.text, "");
    assert_eq!(t.text.chars().count(), t.boxes.len());
    assert_eq!(t.pages.first().map(|p| p.0), Some(0));
    // Every drawn character has a box inside the document area.
    let doc = w.doc_rect;
    let b = t.boxes.iter().flatten().next().expect("box");
    assert!(b[0] >= doc[0] && b[1] >= doc[1], "{b:?} {doc:?}");
    // A selection shows as offsets of the same text.
    w.tabs[0]
        .view
        .set_selection(Some(crate::reader_view::Selection {
            page: 0,
            a: 0,
            b: 4,
        }));
    let t = w.access_text16().expect("text");
    let (s, e) = t.selection.expect("selection");
    let sel: String = t.text.chars().skip(s).take(e - s).collect();
    assert_eq!(
        sel.trim_end(),
        w.view().expect("view").selected_text().expect("sel")
    );
    key(&mut w, Key::Letter('f'), true, false);
    assert_eq!(w.access_focus16().as_deref(), Some("edit:find"));
}
