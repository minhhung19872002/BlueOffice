//! BlueDoc parity round 24: Show Tabs Only, the highlighter pen, diagonal cell borders,
//! context menu Search / Read Aloud / Translate, Sort Text by Number, underline colour.
use super::super::r23::inv_tests::{window, Ctx};
use super::super::{DocWindow, Request};
use crate::ribbon::RibbonHit;
use crate::{Input, Key, Mods, TextMeasure, TextStyle};
use bo_edit::{Pos, Selection};
use bo_engine::edit::Command;

struct Fixed;
impl TextMeasure for Fixed {
    fn measure(&self, text: &str, style: TextStyle) -> (f32, f32) {
        (
            text.chars().count() as f32 * style.size * 0.55,
            style.size * 1.33,
        )
    }
}

fn click_n(w: &mut DocWindow, (x, y): (f32, f32), count: u8) {
    w.handle(Input::Move { x, y }, &Fixed);
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

fn click(w: &mut DocWindow, at: (f32, f32)) {
    click_n(w, at, 1);
}

fn tab_center(w: &DocWindow, i: usize) -> (f32, f32) {
    let (x, y) = (0..w.size.0 as i32)
        .step_by(4)
        .flat_map(|x| (0..200).step_by(4).map(move |y| (x as f32, y as f32)))
        .find(|&(x, y)| w.ribbon.hit(x, y) == Some(RibbonHit::Tab(i)))
        .expect("tab shown");
    (x + 6.0, y + 6.0)
}

fn ctrl_f1(w: &mut DocWindow) {
    w.handle(
        Input::Key(
            Key::F(1),
            Mods {
                ctrl: true,
                ..Mods::default()
            },
        ),
        &Fixed,
    );
    w.handle(Input::Tick, &Fixed);
}

// Ctrl+F1 collapses the ribbon to its tabs: the document moves up, the groups are gone; a
// tab click shows them over the document (which does not move) until a command runs.
#[test]
fn collapse_the_ribbon_and_peek() {
    let mut w = window(Ctx::Text);
    let full_top = w.view_top();
    assert!(w
        .ribbon
        .rect_of(RibbonHit::Command("format.bold"))
        .is_some());
    ctrl_f1(&mut w);
    assert!(w.ribbon.collapsed());
    let top = w.view_top();
    assert!(top < full_top - 60.0, "{top} vs {full_top}");
    let b = w
        .ribbon
        .rect_of(RibbonHit::Command("format.bold"))
        .expect("placed");
    assert_eq!(w.ribbon.hit(b[0] + 4.0, b[1] + b[3] / 2.0), None);
    // A tab click: the groups over the document.
    let home = tab_center(&w, 0);
    click(&mut w, home);
    w.handle(Input::Tick, &Fixed);
    assert!(w.ribbon.peek());
    assert!(
        (w.view_top() - top).abs() < 0.5,
        "the document does not move"
    );
    let b = w
        .ribbon
        .rect_of(RibbonHit::Command("format.bold"))
        .expect("placed");
    assert_eq!(
        w.ribbon.hit(b[0] + 4.0, b[1] + b[3] / 2.0),
        Some(RibbonHit::Command("format.bold"))
    );
    // Bold runs; the ribbon hides again.
    click(&mut w, (b[0] + 4.0, b[1] + b[3] / 2.0));
    w.handle(Input::Tick, &Fixed);
    assert!(w.view.format().bold);
    assert!(!w.ribbon.peek());
    // Esc hides a peeking ribbon; a double click on a tab shows the ribbon again.
    click(&mut w, home);
    assert!(w.ribbon.peek());
    w.handle(Input::Key(Key::Escape, Mods::default()), &Fixed);
    assert!(!w.ribbon.peek());
    click_n(&mut w, home, 2);
    w.handle(Input::Tick, &Fixed);
    assert!(!w.ribbon.collapsed());
    assert!((w.view_top() - full_top).abs() < 0.5);
    // The display options button: Show tabs only.
    let r = w.r24_display_rect().expect("button");
    click(&mut w, (r[0] + r[2] / 2.0, r[1] + r[3] / 2.0));
    let rows: Vec<String> = w
        .menu
        .as_ref()
        .expect("menu")
        .items()
        .iter()
        .map(|i| i.label.clone())
        .collect();
    assert_eq!(rows, ["Show tabs only", "Always show Ribbon"]);
    assert!(w.action("r24.ribbon:tabs"));
    assert!(w.ribbon.collapsed());
    // The ribbon's right-click menu has Collapse the Ribbon.
    assert!(w.action("r24.ribbon:toggle"));
    assert!(!w.ribbon.collapsed());
}

// Text Highlight Color with no selection turns the highlighter pen on: a drag highlights
// the text it covers; Esc ends it; Stop Highlighting is greyed while it is off.
#[test]
fn highlighter_pen() {
    let mut w = window(Ctx::Text);
    w.view.run_command(Command::SetSelection {
        selection: Selection::caret(Pos::new(1, 0)),
    });
    assert!(w.action("format.highlight"));
    assert!(w.r24.pen);
    // Drag over the first paragraph's first word.
    let layout = w.view.engine().layout().expect("layout");
    let slot = w.view.slots()[0];
    let s = w.view.scale();
    let line = &layout.pages[slot.index].lines[0];
    let y = slot.y as f32 + (line.top + line.height / 2.0) * s;
    let x0 = slot.x as f32 + line.carets.get(0).map_or(0.0, |c| c.1) * s + 1.0;
    let x1 = slot.x as f32 + line.carets.get(5).map_or(30.0, |c| c.1) * s;
    w.handle(Input::Move { x: x0, y }, &Fixed);
    assert_eq!(w.cursor(), super::super::r10::pointer::PEN);
    w.handle(
        Input::Press {
            x: x0,
            y,
            count: 1,
            shift: false,
        },
        &Fixed,
    );
    w.handle(Input::Drag { x: x1, y }, &Fixed);
    w.handle(Input::Release { x: x1, y }, &Fixed);
    w.view.run_command(Command::SetSelection {
        selection: Selection {
            anchor: Pos::new(0, 1),
            focus: Pos::new(0, 3),
        },
    });
    assert_eq!(w.view.format().highlight.as_deref(), Some("FFFF00"));
    assert!(w.r24.pen, "the pen stays on");
    w.handle(Input::Key(Key::Escape, Mods::default()), &Fixed);
    assert!(!w.r24.pen);
    // A colour picked with no selection turns the pen on with that colour.
    w.view.run_command(Command::SetSelection {
        selection: Selection::caret(Pos::new(1, 0)),
    });
    assert!(w.action("r23.hl:00FF00"));
    assert!(w.r24.pen);
    assert_eq!(w.r23.highlight.as_deref(), Some("00FF00"));
    assert!(w.action("r23.hl:stop"));
    assert!(!w.r24.pen);
}

// Borders ▾ › Diagonal Down / Up Border in a table cell (§17.4.80 tl2br, §17.4.79 tr2bl).
#[test]
fn diagonal_cell_borders() {
    let mut w = window(Ctx::Table);
    assert!(w.action("format.paraBorders"));
    let m = w.menu.as_ref().expect("borders menu");
    let row = m
        .items()
        .iter()
        .find(|i| i.label == "Diagonal Down Border")
        .expect("row");
    assert!(!row.dim);
    assert!(w.action("r24.diag:down"));
    let t = w.view.format().table.clone().expect("table");
    let d = t.cell_diagonals.expect("diagonals");
    assert!(d[0].is_some() && d[1].is_none());
    assert!(w.action("r24.diag:up"));
    assert!(w.action("r24.diag:down"));
    let d = w
        .view
        .format()
        .table
        .clone()
        .and_then(|t| t.cell_diagonals)
        .expect("diagonals");
    assert!(d[0].is_none() && d[1].is_some());
    // Outside a table the rows are greyed.
    let mut w = window(Ctx::Text);
    assert!(w.action("format.paraBorders"));
    assert!(w
        .menu
        .as_ref()
        .expect("menu")
        .items()
        .iter()
        .filter(|i| i.label.starts_with("Diagonal"))
        .all(|i| i.dim));
}

// The text context menu: Search "…", Read Aloud, Translate.
#[test]
fn context_search_read_aloud_translate() {
    let mut w = window(Ctx::Text);
    let rows = w.r24_ctx_rows();
    let labels: Vec<&str> = rows.iter().map(|r| r.0.as_str()).collect();
    assert_eq!(labels, ["Search \"wrold\"", "Read Aloud", "Translate"]);
    assert!(w.action("r24.search"));
    assert!(w
        .take_requests()
        .iter()
        .any(|r| matches!(r, Request::OpenUrl(u) if u.ends_with("search?q=wrold"))));
    assert!(w.action("r24.translate"));
    assert!(w
        .take_requests()
        .iter()
        .any(|r| matches!(r, Request::OpenUrl(u) if u.contains("text=wrold"))));
    // The caret's word when nothing is selected.
    w.view.run_command(Command::SetSelection {
        selection: Selection::caret(Pos::new(1, 2)),
    });
    assert_eq!(w.r24_ctx_rows()[0].0, "Search \"Second\"");
}

// Sort Text › Type: Number.
#[test]
fn sort_text_by_number() {
    let mut w = window(Ctx::Text);
    w.view.run_command(Command::SelectAll);
    w.view.run_command(Command::InsertText {
        text: "b 10\na 9\nc 100".into(),
    });
    w.view.run_command(Command::SelectAll);
    assert!(w.action("r23.sort:asc:number"));
    let s = w.view.engine().session().expect("session");
    let t: Vec<String> = (0..3).map(|i| s.para_text(i).unwrap_or_default()).collect();
    assert_eq!(t, ["a 9", "b 10", "c 100"]);
}

// Font dialog › Underline color: enabled with an underline, sent as the underline's colour.
#[test]
fn underline_color_in_font_dialog() {
    let mut w = window(Ctx::Text);
    assert!(w.action("format.fontDialog"));
    let f = w.r3.form.as_mut().expect("font dialog");
    assert!(!f.enabled_for_test("ulColor"));
    f.set_sel("underline", Some(2));
    f.r23_changed("underline");
    assert!(f.enabled_for_test("ulColor"));
    // Red is the second standard colour.
    f.set_sel("ulColor", Some(2));
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    assert!(w.r3.form.is_none());
    let a = &w.view.format().advanced;
    assert_eq!(a.underline, "single");
    assert_eq!(a.underline_color.as_deref(), Some("FF0000"));
}
