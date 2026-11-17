//! BlueDoc parity round 7: contextual tabs (Table Design pen and Borders, Picture Format,
//! Header & Footer, Equation), the in-card comment editor, formatting marks by kind, Protect /
//! Inspect.
use super::super::DocWindow;
use crate::doc_forms::DocFormKind;
use crate::doc_tools;
use crate::{DocView, Input, Key, Mods, TextMeasure, TextStyle};
use bo_engine::Engine;
use serde_json::json;

struct Fixed;
impl TextMeasure for Fixed {
    fn measure(&self, text: &str, style: TextStyle) -> (f32, f32) {
        (
            text.chars().count() as f32 * style.size * 0.55,
            style.size * 1.33,
        )
    }
}

fn window_with(engine: Engine) -> DocWindow {
    let mut w = DocWindow::new(DocView::new(engine).expect("view"), "en-US", "a.docx");
    w.handle(Input::Scale(1.25), &Fixed);
    w.handle(
        Input::Resize {
            width: 1500.0,
            height: 1102.0,
        },
        &Fixed,
    );
    w
}

fn window() -> DocWindow {
    let mut engine = Engine::with_bundled_fonts();
    engine.new_document("a.docx").expect("new");
    window_with(engine)
}

fn run(w: &mut DocWindow, id: &str) -> bool {
    w.run_interface_command(id, &Fixed)
}

fn png() -> Vec<u8> {
    let mut out = Vec::new();
    {
        let mut e = png::Encoder::new(&mut out, 8, 6);
        e.set_color(png::ColorType::Rgba);
        e.set_depth(png::BitDepth::Eight);
        let mut wr = e.write_header().expect("header");
        wr.write_image_data(&[90; 8 * 6 * 4]).expect("png");
    }
    out
}

fn table_borders(w: &DocWindow) -> bo_model::Borders {
    let doc = w.view.engine().document().expect("doc");
    doc.sections[0]
        .blocks
        .iter()
        .find_map(|b| match b {
            bo_model::Block::Table(t) => t.props.borders.clone(),
            _ => None,
        })
        .expect("table borders")
}

#[test]
fn table_design_pen_and_borders_menu_apply_to_the_table() {
    let mut w = window();
    assert!(run(&mut w, "insert.table.3x3"));
    w.handle(Input::Tick, &Fixed);
    assert!(w
        .ribbon
        .contextual_labels()
        .contains(&doc_tools::TABLE_DESIGN));
    run(&mut w, "tab:tableDesign");
    for id in [
        "table.borderStyles",
        "table.penStyle",
        "table.penWeight",
        "table.penColor",
        "table.borders",
        "table.borderPainter",
    ] {
        assert!(
            w.ribbon
                .rect_of(crate::ribbon::RibbonHit::Command(id))
                .is_some(),
            "{id} on Table Design"
        );
    }
    // Double, 1½ pt, red pen; then Inside Horizontal Border.
    assert!(run(&mut w, "table.pen.style.5"));
    assert!(run(&mut w, "table.pen.weight.4"));
    assert!(run(&mut w, "table.pen.color.2"));
    assert!(run(&mut w, "table.border.insideH"));
    let b = table_borders(&w);
    let h = b.inside_h.expect("inside h");
    assert_eq!((h.style.as_str(), h.size), ("double", 12));
    assert_eq!(
        h.color.map(bo_model::Color::to_hex),
        Some("FF0000".to_owned())
    );
    // Border Painter toggles and shows pressed.
    assert!(run(&mut w, "table.borderPainter"));
    assert!(w.r7_on_ids().contains(&"table.borderPainter"));
    w.handle(Input::Key(Key::Escape, Mods::default()), &Fixed);
    assert!(!w.r7.painter);
}

#[test]
fn picture_format_has_the_reference_groups_and_alt_text_sets_the_description() {
    let mut engine = Engine::with_bundled_fonts();
    engine.new_document("a.docx").expect("new");
    engine.insert_image(png(), "a.png").expect("picture");
    let mut w = window_with(engine);
    w.handle(Input::Tick, &Fixed);
    assert!(w
        .ribbon
        .contextual_labels()
        .contains(&doc_tools::PICTURE_FORMAT));
    w.ribbon.select_tab(doc_tools::PICTURE_FORMAT);
    w.relayout(&Fixed);
    for id in [
        "picture.removeBackground",
        "picture.corrections",
        "picture.color",
        "picture.compress",
        "picture.change",
        "picture.altText",
        "picture.position",
        "picture.crop",
        "picture.height",
    ] {
        assert!(
            w.ribbon
                .rect_of(crate::ribbon::RibbonHit::Command(id))
                .is_some(),
            "{id} on Picture Format"
        );
    }
    // The Corrections gallery opens under its button and closes with Escape.
    assert!(run(&mut w, "picture.corrections"));
    assert!(w.r7.gallery.is_some());
    w.handle(Input::Key(Key::Escape, Mods::default()), &Fixed);
    assert!(w.r7.gallery.is_none());
    // Alt Text: the dialog, then the description is the picture's.
    assert!(run(&mut w, "picture.altText"));
    assert_eq!(w.r3_form_kind(), Some(DocFormKind::AltText));
    if let Some(f) = w.r3.form.as_mut() {
        f.set_text("alt", "A sunny hill");
    }
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    let desc = w
        .view
        .engine()
        .session()
        .and_then(bo_edit::Session::tools_picture)
        .map(|i| i.description);
    assert_eq!(desc.as_deref(), Some("A sunny hill"));
    // The height box steps the picture by 0.1 unit.
    let before = w.r7_box_value("picture.height").expect("height");
    assert!(w.r7_set_box("picture.height", before + 144));
    let after = w.r7_box_value("picture.height").expect("height");
    assert!((after - before - 144).abs() <= 2, "{before} → {after}");
}

#[test]
fn header_footer_tab_shows_the_reference_groups() {
    let mut w = window();
    assert!(w
        .view
        .run_json(&json!({ "cmd": "editHeaderFooter", "page": 0, "header": true }))
        .is_some());
    w.handle(Input::Tick, &Fixed);
    assert_eq!(w.ribbon.active_label(), doc_tools::HEADER_FOOTER);
    for id in [
        "insert.header",
        "insert.dateTime",
        "hf.documentInfo",
        "hf.previous",
        "hf.next",
        "hf.showDocumentText",
        "hf.headerPos",
        "hf.close",
    ] {
        assert!(
            w.ribbon
                .rect_of(crate::ribbon::RibbonHit::Command(id))
                .is_some(),
            "{id} on Header & Footer"
        );
    }
    // Header from Top steps the section's header distance.
    let before = w.view.format().section.header;
    assert!(w.r7_set_box("hf.headerPos", before + 144));
    assert_eq!(w.view.format().section.header, before + 144);
}

#[test]
fn equation_tab_shows_with_an_equation_and_structures_insert_math() {
    let mut w = window();
    assert!(run(&mut w, "eq.fraction.stacked"));
    let eq = w
        .view
        .engine()
        .document()
        .is_some_and(|d| format!("{d:?}").contains("oMath"));
    assert!(eq, "a structure outside an equation inserts one");
    assert!(run(&mut w, "eq.latex"));
    assert!(w.r7_on_ids().contains(&"eq.latex"));
    // Select the equation: the Equation tab appears.
    w.view.run_json(&json!({ "cmd": "selectAll" }));
    w.handle(Input::Tick, &Fixed);
    assert!(w
        .ribbon
        .contextual_labels()
        .contains(&crate::doc_tabs7::EQUATION));
}

#[test]
fn new_comment_is_written_in_a_card_and_posted() {
    let mut w = window();
    w.view.handle(Input::Text("Hello world".into()));
    w.view.run_json(&json!({ "cmd": "selectAll" }));
    assert!(run(&mut w, "review.newComment"));
    assert!(w.comments_pane.as_ref().is_some_and(|c| c.draft.is_some()));
    w.handle(Input::Text("Check this".into()), &Fixed);
    let ctrl = Mods {
        ctrl: true,
        ..Mods::default()
    };
    w.handle(Input::Key(Key::Enter, ctrl), &Fixed);
    let comments = w.view.comments();
    assert_eq!(comments.len(), 1);
    assert_eq!(comments[0].text, "Check this");
    assert!(w.comments_pane.as_ref().is_some_and(|c| c.draft.is_none()));
    // Escape drops a draft without posting.
    assert!(run(&mut w, "review.newComment"));
    w.handle(Input::Text("Never mind".into()), &Fixed);
    w.handle(Input::Key(Key::Escape, Mods::default()), &Fixed);
    assert_eq!(w.view.comments().len(), 1);
}

#[test]
fn formatting_mark_kinds_follow_the_options() {
    let mut w = window();
    w.view.handle(Input::Text("a\tb c".into()));
    // Only tab characters.
    w.set_prefs("markKinds=100000");
    let marks = w.view.engine().formatting_marks(0);
    assert!(marks
        .iter()
        .all(|m| m.kind == bo_engine::views::FormatMarkKind::Tab));
    assert_eq!(marks.len(), 1);
    // Paragraph marks and spaces.
    w.set_prefs("markKinds=011000");
    let kinds: Vec<_> = w
        .view
        .engine()
        .formatting_marks(0)
        .into_iter()
        .map(|m| m.kind)
        .collect();
    assert!(kinds.contains(&bo_engine::views::FormatMarkKind::Space));
    assert!(kinds.contains(&bo_engine::views::FormatMarkKind::Paragraph));
    assert!(!kinds.contains(&bo_engine::views::FormatMarkKind::Tab));
    // The ¶ button shows every kind.
    assert!(run(&mut w, "view.formattingMarks"));
    // Tab, space and the paragraph mark.
    assert_eq!(w.view.engine().formatting_marks(0).len(), 3);
}

#[test]
fn protect_and_inspect_menus_run_the_engine() {
    let mut w = window();
    w.view.handle(Input::Text("Hello".into()));
    w.view.run_json(&json!({ "cmd": "selectAll" }));
    w.view
        .run_json(&json!({ "cmd": "addComment", "text": "Note" }));
    // Encrypt with Password: the next save is encrypted.
    assert!(run(&mut w, "file.encrypt"));
    assert_eq!(w.r3_form_kind(), Some(DocFormKind::Encrypt));
    if let Some(f) = w.r3.form.as_mut() {
        f.set_text("pw", "secret");
    }
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    assert!(w.view.engine().save_password_set());
    // Always Open Read-Only toggles the recommendation.
    assert!(run(&mut w, "file.readOnlyRecommended"));
    let sec = w
        .view
        .run_json(&json!({ "cmd": "security" }))
        .expect("security");
    assert_eq!(sec["security"]["readOnlyRecommended"], json!(true));
    // Inspect Document finds the comment; Remove All takes it away.
    assert!(run(&mut w, "file.inspect"));
    assert!(w.r7_inspect(&["comments".to_owned()], None));
    assert_eq!(w.view.comments().len(), 1);
    assert!(w.r7_inspect(&[], Some("comments")));
    assert_eq!(w.view.comments().len(), 0);
    // Check Accessibility opens the pane.
    assert!(run(&mut w, "review.checkAccessibility"));
    assert!(w.r7.a11y.is_some());
    assert!(w.r7_pane_w() > 0.0);
    // Check Compatibility lists features.
    assert!(run(&mut w, "file.compatibility"));
    assert_eq!(w.r3_form_kind(), Some(DocFormKind::Compatibility));
}
