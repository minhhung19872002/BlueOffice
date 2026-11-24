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
    // The title bar's Ribbon Display Options button: Show Tabs.
    let r = w.r24_title_rect().expect("button");
    assert_eq!(
        w.nc_hit(r[0] + 4.0, r[1] + 4.0),
        crate::titlebar::NcHit::Client
    );
    click(&mut w, (r[0] + r[2] / 2.0, r[1] + r[3] / 2.0));
    let rows: Vec<String> = w
        .menu
        .as_ref()
        .expect("menu")
        .items()
        .iter()
        .map(|i| i.label.clone())
        .collect();
    assert_eq!(rows, ["Show Tabs", "Show Tabs and Commands"]);
    assert!(w.action("r24.ribbon:tabs"));
    assert!(w.ribbon.collapsed());
    // The ribbon's right-click menu has Collapse the Ribbon.
    assert!(w.action("r24.ribbon:toggle"));
    assert!(!w.ribbon.collapsed());
    // ^ at the ribbon's bottom right collapses it.
    w.handle(Input::Tick, &Fixed);
    let r = w.r24_display_rect().expect("collapse button");
    click(&mut w, (r[0] + r[2] / 2.0, r[1] + r[3] / 2.0));
    assert!(w.ribbon.collapsed());
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
    let mut rows = w.r24_ctx_rows(0);
    rows.extend(w.r24_ctx_rows(1));
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
    assert_eq!(w.r24_ctx_rows(0)[0].0, "Search \"Second\"");
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

/// The drop cap frame of the document (its letter is a paragraph of its own).
fn first_frame(w: &DocWindow) -> Option<bo_model::FrameProps> {
    let s = w.view.engine().session().expect("session");
    (0..s.paragraph_count())
        .find_map(|i| bo_model::index::para_by_id(s.doc(), i).and_then(|p| p.props.frame))
}

// Insert › Drop Cap › Drop Cap Options…: position, lines, distance (§17.3.1.11 framePr).
#[test]
fn drop_cap_options_dialog() {
    use crate::doc_forms::DocFormKind;
    let mut w = window(Ctx::Text);
    let rows = crate::catalog::menu("insert.dropCap");
    assert!(rows.iter().any(|e| e.id == "r24.dropCapOptions"));
    assert!(w.action("r24.dropCapOptions"));
    let f = w.r3.form.as_mut().expect("Drop Cap");
    assert_eq!(f.kind, DocFormKind::DropCap24);
    assert!(!f.enabled_for_test("lines"), "greyed with None");
    f.r24_set_pos("drop");
    assert!(f.enabled_for_test("lines"));
    f.set_text("lines", "2");
    f.set_text("distance", "6 pt");
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    assert!(w.r3.form.is_none());
    let frame = first_frame(&w).expect("frame");
    assert_eq!(frame.drop_cap, bo_model::DropCap::Drop);
    assert_eq!(frame.h_space.0, 120);
    // Opening again shows what the paragraph has.
    assert!(w.action("r24.dropCapOptions"));
    let f = w.r3.form.as_ref().expect("Drop Cap");
    assert_eq!(f.r24_pos(), "drop");
}

// Layout › Hyphenation › Hyphenation Options…: automatic, CAPS, zone, limit.
#[test]
fn hyphenation_options_dialog() {
    let mut w = window(Ctx::Text);
    let rows = crate::catalog::menu("layout.hyphenation");
    assert!(rows.iter().any(|e| e.id == "r24.hyphOptions"));
    assert!(rows.iter().any(|e| e.id == "r24.hyphManual"));
    assert!(w.action("r24.hyphOptions"));
    let f = w.r3.form.as_mut().expect("Hyphenation");
    f.set_on("auto", true);
    f.set_on("caps", false);
    f.set_text("zone", "9 pt");
    f.set_sel("limit", Some(2));
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    let h = w
        .view
        .engine()
        .session()
        .map(|s| s.doc().hyphenation)
        .expect("hyphenation");
    assert!(h.auto && h.no_caps);
    assert_eq!(h.zone.map(|z| z.0), Some(180));
    assert_eq!(h.consecutive_limit, Some(2));
}

// Layout › Hyphenation › Manual: each line-start word that can be hyphenated, Yes puts an
// optional hyphen (§17.3.3.29) at the chosen point.
#[test]
fn manual_hyphenation() {
    use crate::doc_forms::DocFormKind;
    let mut w = window(Ctx::Text);
    w.view.run_command(Command::SelectAll);
    let long = "information ".repeat(40);
    w.view.run_command(Command::InsertText { text: long });
    w.handle(Input::Tick, &Fixed);
    assert!(w.action("r24.hyphManual"));
    let f = w.r3.form.as_mut().expect("Manual Hyphenation");
    assert_eq!(f.kind, DocFormKind::ManualHyph24);
    let shown = f.text("at");
    assert!(shown.contains('-'), "{shown}");
    let out = f.click_for_test("yes");
    assert!(w.form_out(out));
    let text = w
        .view
        .engine()
        .session()
        .and_then(|s| s.para_text(0))
        .unwrap_or_default();
    assert!(text.contains('\u{ad}'), "an optional hyphen");
    // Cancel ends it.
    if let Some(f) = w.r3.form.as_mut() {
        let out = f.click_for_test("cancel");
        w.form_out(out);
    }
}

// Insert › Quick Parts: AutoText ▸, Building Blocks Organizer…, Save Selection to Quick
// Part Gallery….
#[test]
fn quick_parts_autotext_and_organizer() {
    use crate::doc_forms::DocFormKind;
    let mut w = window(Ctx::Text);
    let rows = crate::doc_r4_menus::menu("insert.quickParts");
    for id in ["r24.autoText", "r24.organizer", "r24.saveQuickPart"] {
        assert!(rows.iter().any(|e| e.id == id), "{id}");
    }
    // Save the selection as AutoText, then it is in AutoText ▸.
    assert!(w.action("r24.saveAutoText"));
    let f = w.r3.form.as_mut().expect("Create New Building Block");
    assert_eq!(f.kind, DocFormKind::NewBlock);
    f.set_text("name", "mine");
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    assert!(w.action("r24.autoText"));
    let labels: Vec<String> = w
        .menu
        .as_ref()
        .expect("AutoText menu")
        .items()
        .iter()
        .map(|i| i.label.clone())
        .collect();
    assert!(labels.iter().any(|l| l == "mine"), "{labels:?}");
    // The organizer lists it with the library's blocks; Insert puts it in.
    assert!(w.action("r24.organizer"));
    let f = w.r3.form.as_mut().expect("Organizer");
    assert_eq!(f.kind, DocFormKind::Organizer24);
    let items = f.items_for_test("list");
    assert!(items.len() > 5);
    let i = items
        .iter()
        .position(|r| r.starts_with("mine\t"))
        .expect("mine listed");
    f.set_sel("list", Some(i));
    let before = w
        .view
        .engine()
        .session()
        .map(bo_edit::Session::paragraph_count);
    let out = w.r3.form.as_mut().expect("form").click_for_test("insert");
    assert!(w.form_out(out));
    assert!(w.view.modified());
    let _ = before;
    // Delete takes it out of the list.
    assert!(w.action("r24.organizer"));
    let f = w.r3.form.as_mut().expect("Organizer");
    let items = f.items_for_test("list");
    let i = items
        .iter()
        .position(|r| r.starts_with("mine\t"))
        .expect("mine");
    f.set_sel("list", Some(i));
    let out = f.click_for_test("delete");
    w.form_out(out);
    let f = w.r3.form.as_ref().expect("Organizer again");
    assert!(!f
        .items_for_test("list")
        .iter()
        .any(|r| r.starts_with("mine\t")));
}

// Shape Format › Shape Styles › More: the Theme Styles gallery (7 colours × 6 intensities)
// restyles the selected shape.
#[test]
fn shape_styles_more_gallery() {
    let mut w = window(Ctx::Shape);
    w.ribbon.select_tab(crate::doc_tabs7::SHAPE_FORMAT);
    w.relayout(&Fixed);
    assert!(w.action("shape.styleMore"));
    let g = w.r22.gallery.as_ref().expect("gallery");
    let cells = g.cells();
    assert_eq!(cells.len(), 42);
    let cmd = cells[8].2.to_owned();
    assert!(cmd.starts_with("r24.shapeStyle:4472C4:"), "{cmd}");
    assert!(w.action(&cmd));
    assert!(w.r22.gallery.is_none());
    assert!(w.view.modified());
}

// Font › Advanced: OpenType features are greyed for a document in an older layout mode
// (the reference's Compatibility Mode), on for current documents.
#[test]
fn opentype_features_greyed_in_compatibility_mode() {
    use crate::doc_forms::DocFormKind;
    let mut w = window(Ctx::Text);
    let mut c = w.form_context();
    c.extra["r23"] = crate::doc_forms::r23::extra(w.view.format());
    c.extra["r23"]["compat"] = serde_json::json!(12);
    let f = crate::doc_forms::open(DocFormKind::Font23, &w.strings, &c);
    assert!(!f.enabled_for_test("lig"));
    assert!(!f.enabled_for_test("ctxAlts"));
    c.extra["r23"]["compat"] = serde_json::json!(15);
    let f = crate::doc_forms::open(DocFormKind::Font23, &w.strings, &c);
    assert!(f.enabled_for_test("lig"));
}

// The second inventory sample: Next Footnote moves to the note in the caret's own
// paragraph; View Alt Text… of the selected text box opens the Alt Text dialog.
#[test]
fn rich_sample_notes_and_text_box_alt_text() {
    use super::super::r23::inv_tests::RICH;
    RICH.with(|r| r.set(true));
    let mut w = window(Ctx::Text);
    let mut b = window(Ctx::Shape);
    RICH.with(|r| r.set(false));
    let before = w.view.engine().session().map(bo_edit::Session::selection);
    assert!(w.action("references.note.nextFoot"));
    let after = w.view.engine().session().map(bo_edit::Session::selection);
    assert_ne!(before, after);
    let s = b.view.engine().session().expect("session");
    let sel = s.selection();
    let text = s.para_text(sel.anchor.para).unwrap_or_default();
    let shape = s.tools_shape().is_some();
    let ctx = b.context.shape;
    assert!(
        b.action("picture.altText"),
        "{sel:?} {text:?} {shape} {ctx}"
    );
    assert!(b.r3.form.is_some());
    // The same through the shape's context menu (as the inventory opens it).
    RICH.with(|r| r.set(true));
    let mut c = window(Ctx::Shape);
    RICH.with(|r| r.set(false));
    let (x, y) = super::super::r23::inv_tests::context_point(&c, Ctx::Shape);
    c.handle(Input::Context { x, y }, &Fixed);
    let s = c.view.engine().session().expect("session");
    let sel = s.selection();
    let text = s.para_text(sel.anchor.para).unwrap_or_default();
    let story = c.view.story().is_some();
    let paras: Vec<String> = (0..s.paragraph_count())
        .map(|i| s.para_text(i).unwrap_or_default())
        .collect();
    assert!(
        c.action("picture.altText"),
        "{sel:?} {text:?} story {story} {paras:?}"
    );
}
