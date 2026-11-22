//! BlueDoc parity round 11: Find and Replace › Format ▾ dialogs and Special ▾.
use super::super::DocWindow;
use crate::doc_find::FindAction;
use crate::doc_forms::{DocFormKind, FormOut};
use crate::nav_pane::NavTab;
use crate::{DocView, Input, Key, Mods, TextMeasure, TextStyle};
use bo_edit::{ParaFormat, Pos, Selection};
use bo_engine::edit::Command;
use bo_engine::Engine;
use bo_model::Alignment;

struct Fixed;
impl TextMeasure for Fixed {
    fn measure(&self, text: &str, style: TextStyle) -> (f32, f32) {
        (
            text.chars().count() as f32 * style.size * 0.55,
            style.size * 1.33,
        )
    }
}

/// "plain fox" then a centred "centre fox".
fn window() -> DocWindow {
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
    w.view.run_command(Command::InsertText {
        text: "plain fox".into(),
    });
    w.view.run_command(Command::SplitParagraph);
    w.view.run_command(Command::InsertText {
        text: "centre fox".into(),
    });
    w.view.run_command(Command::FormatParagraph {
        format: ParaFormat::Align {
            align: Alignment::Center,
        },
    });
    w.view.run_command(Command::SetSelection {
        selection: Selection::caret(Pos::new(0, 0)),
    });
    w
}

fn ok(w: &mut DocWindow) -> bool {
    let out =
        w.r3.form
            .as_mut()
            .expect("form")
            .input(&Input::Key(Key::Enter, Mods::default()));
    assert!(matches!(out, FormOut::FindFormat(..)), "{out:?}");
    w.form_out(out)
}

// source: the reference's Format ▾ (Font…, Paragraph…, Tabs…, Language…, Frame…, Style…,
// Highlight) opens "Find Paragraph" etc. over Find and Replace; a paragraph format found
// selects only the text of paragraphs with it (captured round 11).
#[test]
fn format_paragraph_finds_centred_text() {
    let mut w = window();
    assert!(w.action("edit.replace"));
    let fd = w.find_dialog.as_mut().expect("dialog");
    assert_eq!(fd.format_entry(1), FindAction::FormatDialog(1));
    assert!(w.find_dialog_action(FindAction::FormatDialog(1)));
    let f = w.r3.form.as_mut().expect("Find Paragraph");
    assert_eq!(f.kind, DocFormKind::FindParagraph);
    f.set_sel("align", Some(2)); // Centered
    assert!(ok(&mut w));
    assert!(w.r3.form.is_none());
    let fd = w.find_dialog.as_mut().expect("dialog");
    assert_eq!(fd.fmt.summary(), "Centered");
    assert!(fd.find_next_enabled());
    // "fox" with the format: only the centred paragraph's.
    fd.text("fox");
    assert!(w.find_dialog_action(FindAction::FindNext));
    let (a, b) = w.view.engine().session().expect("s").selection().ordered();
    assert_eq!((a.para, a.offset, b.offset), (1, 7, 10));
}

// source: the reference's Replace with formatting: Format ▾ with Replace with focused sets
// the replacement's format ("Format:" under Replace with); an empty Replace with keeps the
// text found and only formats it.
#[test]
fn replace_with_a_font_format() {
    let mut w = window();
    assert!(w.action("edit.replace"));
    let fd = w.find_dialog.as_mut().expect("dialog");
    fd.text("fox");
    // Tab moves the focus to Replace with.
    assert_eq!(fd.key(Key::Tab, Mods::default()), FindAction::Redraw);
    assert!(w.find_dialog_action(FindAction::FormatDialog(0)));
    let f = w.r3.form.as_mut().expect("Find Font");
    assert_eq!(f.kind, DocFormKind::FindFont);
    f.set_text("fstyle", "Bold");
    f.set_tri("strike", Some(true));
    assert!(ok(&mut w));
    let fd = w.find_dialog.as_ref().expect("dialog");
    assert!(fd.fmt.is_empty());
    assert_eq!(fd.rfmt.summary(), "Font: Bold, Strikethrough");
    assert!(w.find_dialog_action(FindAction::ReplaceAll));
    let doc = w.view.engine().document().expect("doc");
    let bold: Vec<String> = bo_model::index::body_paragraphs(doc)
        .iter()
        .filter_map(|p| bo_model::index::para_at(doc, p))
        .flat_map(|p| p.runs.iter())
        .filter(|r| r.props.bold == Some(true) && r.props.strike == Some(true))
        .map(|r| {
            r.content
                .iter()
                .filter_map(|c| match c {
                    bo_model::RunContent::Text(t) => Some(t.as_str()),
                    _ => None,
                })
                .collect()
        })
        .collect();
    assert_eq!(bold, ["fox", "fox"]);
    assert_eq!(
        w.view
            .engine()
            .session()
            .expect("s")
            .para_text(0)
            .as_deref(),
        Some("plain fox")
    );
}

// source: the reference's Find Language list ends with "(no language)"; Find Style lists the
// styles after "(no style)"; Highlight cycles Highlight / Not Highlight / any.
#[test]
fn language_style_and_highlight_parts() {
    let mut w = window();
    assert!(w.action("edit.replace"));
    assert!(w.find_dialog_action(FindAction::FormatDialog(3)));
    let f = w.r3.form.as_mut().expect("Find Language");
    let n = f.items_for_test("list").len();
    assert_eq!(f.items_for_test("list")[n - 1], "(no language)");
    f.set_sel("list", Some(n - 1));
    assert!(ok(&mut w));
    assert!(w.find_dialog_action(FindAction::FormatDialog(5)));
    let f = w.r3.form.as_mut().expect("Find Style");
    assert_eq!(f.kind, DocFormKind::FindStyle);
    let items = f.items_for_test("styles");
    let normal = items
        .iter()
        .position(|s| s.ends_with("Normal"))
        .expect("Normal");
    f.set_sel("styles", Some(normal));
    assert!(ok(&mut w));
    let fd = w.find_dialog.as_mut().expect("dialog");
    assert_eq!(fd.fmt.summary(), "(no language), Style: Normal");
    assert_eq!(fd.format_entry(6), FindAction::Redraw);
    assert!(fd.fmt.summary().ends_with(", Highlight"));
    fd.format_entry(6);
    assert!(fd.fmt.summary().ends_with(", Not Highlight"));
    fd.format_entry(6);
    assert_eq!(fd.fmt.summary(), "(no language), Style: Normal");
    let j = fd.format_json().expect("json");
    assert_eq!(j["lang"], "none");
    assert_eq!(j["style"], "Normal");
}

/// Headings Introduction (1), Background (2), Method (1), Data (2), Results (1) with body
/// paragraphs, as the reference capture of round 11.
fn headings_window() -> DocWindow {
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
    let rows: [(&str, Option<u8>); 10] = [
        ("Introduction", Some(0)),
        ("Intro text.", None),
        ("Background", Some(1)),
        ("Background text.", None),
        ("Method", Some(0)),
        ("Data", Some(1)),
        ("Data text.", None),
        ("Results", Some(0)),
        ("Final words.", None),
        ("End.", None),
    ];
    for (i, (text, level)) in rows.iter().enumerate() {
        if i > 0 {
            w.view.run_command(Command::SplitParagraph);
        }
        w.view.run_command(Command::InsertText {
            text: (*text).into(),
        });
        w.view.run_command(Command::SetOutlineLevel {
            para: Some(i),
            level: *level,
        });
    }
    assert!(w.action("edit.find"));
    w.nav.as_mut().expect("nav").show(NavTab::Headings);
    w.refresh_nav();
    w
}

fn heading_texts(w: &DocWindow) -> Vec<String> {
    let nav = w.nav.as_ref().expect("nav");
    nav.visible()
        .iter()
        .map(|&i| nav.headings[i].text.clone())
        .collect()
}

// source: the reference's navigation pane (round 11 capture): headings with subheadings
// carry a triangle that collapses them; Collapse All / Expand All; Show Heading Levels.
#[test]
fn headings_collapse_and_levels() {
    let mut w = headings_window();
    assert_eq!(
        heading_texts(&w),
        ["Introduction", "Background", "Method", "Data", "Results"]
    );
    let nav = w.nav.as_ref().expect("nav");
    assert!(nav.has_children(0) && !nav.has_children(1) && !nav.has_children(4));
    assert!(w.action("nav11.collapseAll"));
    assert_eq!(heading_texts(&w), ["Introduction", "Method", "Results"]);
    assert!(w.action("nav11.expandAll"));
    assert!(w.action("nav11.level.1"));
    assert_eq!(heading_texts(&w), ["Introduction", "Method", "Results"]);
    assert!(w.action("nav11.level.9"));
    assert_eq!(heading_texts(&w).len(), 5);
}

// source: the reference's heading menu (UI Automation, round 11): Promote / Demote / Move
// Down / New Heading After / Select Heading and Content / Delete act on the heading with its
// content.
#[test]
fn heading_menu_commands() {
    let mut w = headings_window();
    // Right-click "Background".
    assert!(w.r11_heading_menu(1));
    assert!(w.menu.is_some());
    assert!(w.action("nav11.promote"));
    assert_eq!(w.nav.as_ref().expect("nav").headings[1].level, 0);
    assert!(w.r11_heading_menu(1));
    assert!(w.action("nav11.demote"));
    // Move "Method" (with Data and its text) below "Results" and its content.
    assert!(w.r11_heading_menu(2));
    assert!(w.action("nav11.moveDown"));
    assert_eq!(
        heading_texts(&w),
        ["Introduction", "Background", "Results", "Method", "Data"]
    );
    // Select "Introduction" and its content: paragraphs 0–3 with the last mark.
    assert!(w.r11_heading_menu(0));
    assert!(w.action("nav11.select"));
    let (a, b) = w.view.engine().session().expect("s").selection().ordered();
    assert_eq!((a.para, a.offset, b.para, b.offset), (0, 0, 4, 0));
    // New Heading After "Introduction": an empty level-1 heading after its content.
    assert!(w.r11_heading_menu(0));
    assert!(w.action("nav11.newAfter"));
    let nav = w.nav.as_ref().expect("nav");
    assert_eq!((nav.headings[2].level, nav.headings[2].para), (0, 4));
    // Delete "Results" with its content.
    let i = heading_texts(&w)
        .iter()
        .position(|t| t == "Results")
        .expect("Results");
    assert!(w.r11_heading_menu(i));
    assert!(w.action("nav11.delete"));
    assert!(!heading_texts(&w).iter().any(|t| t == "Results"));
}

// source: dragging a heading in the reference's pane moves it with its content.
#[test]
fn drag_a_heading_moves_its_section() {
    let mut w = headings_window();
    let nav = w.nav.as_mut().expect("nav");
    // Drag "Results" (row 4) before "Method" (row 2).
    nav.drag = Some((4, None));
    let y = nav.row_y_for_test(2);
    nav.drag_over(y);
    assert_eq!(nav.drag, Some((4, Some(2))));
    w.r11.heading_drag = true;
    assert!(w.handle(Input::Release { x: 50.0, y }, &Fixed));
    assert_eq!(
        heading_texts(&w),
        ["Introduction", "Background", "Results", "Method", "Data"]
    );
}

// source: the reference's pane searches as the user types (Results tab, first match selected).
#[test]
fn search_as_you_type() {
    let mut w = headings_window();
    w.nav.as_mut().expect("nav").focus = true;
    assert!(w.handle(Input::Text("tex".into()), &Fixed));
    let nav = w.nav.as_ref().expect("nav");
    assert_eq!(nav.tab, NavTab::Results);
    assert_eq!(nav.results.len(), 3);
    assert!(w.handle(Input::Key(Key::Backspace, Mods::default()), &Fixed));
    assert_eq!(w.nav.as_ref().expect("nav").query, "te");
}

// source: the reference's Table Properties (captured round 11): five tabs; Row › Repeat as
// header row and Table › Preferred width act on the caret's table.
#[test]
fn table_properties_tabs_and_ok() {
    let mut w = window();
    w.view
        .run_command(Command::InsertTable { rows: 2, cols: 2 });
    assert!(w.action("table.properties"));
    let f = w.r3.form.as_mut().expect("Table Properties");
    assert_eq!(f.kind, DocFormKind::TableProps);
    f.set_on("rHeader", true);
    f.set_on("tPref", true);
    f.set_text("tWidth", "4\"");
    let out = f.input(&Input::Key(Key::Enter, Mods::default()));
    assert!(matches!(out, FormOut::Run(..)), "{out:?}");
    assert!(w.form_out(out));
    let t = w.view.format().table.clone().expect("table");
    assert!(t.header_row);
    assert_eq!(t.width, 5760);
}
