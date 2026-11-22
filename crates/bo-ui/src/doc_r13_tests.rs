//! BlueDoc parity round 13: Page Setup, Paragraph, Manage Styles › Set Defaults, Border and
//! Shading Options.
use super::super::DocWindow;
use crate::doc_forms::r13::show_len;
use crate::doc_forms::{DocFormKind, FormOut};
use crate::{DocView, Input, Key, Mods, TextMeasure, TextStyle};
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

/// "one" / "two", the caret in "one".
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
    w.view
        .run_command(Command::InsertText { text: "one".into() });
    w.view.run_command(Command::SplitParagraph);
    w.view
        .run_command(Command::InsertText { text: "two".into() });
    w.view.run_command(Command::SetSelection {
        selection: Selection::caret(Pos::new(0, 1)),
    });
    w
}

/// OK of the open dialog (Enter on its default button).
fn enter(w: &mut DocWindow) {
    let out =
        w.r3.form
            .as_mut()
            .expect("form")
            .input(&Input::Key(Key::Enter, Mods::default()));
    assert!(matches!(out, FormOut::Run(..)), "{out:?}");
    w.form_out(out);
}

fn doc(w: &DocWindow) -> &bo_model::Document {
    w.view.engine().document().expect("document")
}

// source: the reference's Page Setup (captured round 13): Margins › Gutter, Layout ›
// Vertical alignment, Margins › Multiple pages: Mirror margins (§17.6.11, §17.6.23,
// §17.15.1.57).
#[test]
fn page_setup_sets_gutter_vertical_alignment_and_mirror_margins() {
    let mut w = window();
    assert!(w.action("layout.pageSetup"));
    let inches = w.units.inches;
    let f = w.r3.form.as_mut().expect("Page Setup");
    assert_eq!(f.kind, DocFormKind::PageSetup);
    f.set_text("gutter", &show_len(720, inches));
    f.set_sel("vAlign", Some(1));
    f.set_sel("multi", Some(1));
    enter(&mut w);
    assert!(w.r3.form.is_none());
    let d = doc(&w);
    assert_eq!(d.sections[0].layout.gutter.0, 720);
    assert_eq!(d.sections[0].layout.v_align, bo_model::PageVAlign::Center);
    assert!(d.pages.mirror_margins);
    // One undo step.
    w.view.run_command(Command::Undo);
    assert!(doc(&w).sections[0].layout.is_default());
    assert!(!doc(&w).pages.mirror_margins);
}

// source: the reference's Paragraph dialog (captured round 13): Indentation › Left and
// Spacing › After set the caret's paragraph.
#[test]
fn paragraph_dialog_sets_indent_and_spacing() {
    let mut w = window();
    assert!(w.action("format.paragraphSettings"));
    let inches = w.units.inches;
    let f = w.r3.form.as_mut().expect("Paragraph");
    assert_eq!(f.kind, DocFormKind::Paragraph);
    f.set_text("inside", &show_len(720, inches));
    f.set_text("after", "12 pt");
    enter(&mut w);
    let Some(bo_model::Block::Paragraph(p)) = doc(&w).sections[0].blocks.first() else {
        panic!("paragraph");
    };
    assert_eq!(p.props.indent_left, Some(bo_model::Twips(720)));
    assert_eq!(p.props.space_after, Some(bo_model::Twips(240)));
}

// source: the reference's Manage Styles › Set Defaults (captured round 13): Size and
// Paragraph Spacing › After go into the document defaults (§17.7.5.1 docDefaults).
#[test]
fn manage_styles_set_defaults_sets_doc_defaults() {
    let mut w = window();
    assert!(w.action("styles.manage"));
    let f = w.r3.form.as_mut().expect("Manage Styles");
    assert_eq!(f.kind, DocFormKind::ManageStyles);
    f.set_text("size", "14");
    f.set_text("after", "6 pt");
    enter(&mut w);
    let d = doc(&w);
    assert_eq!(d.default_run.size, Some(bo_model::HalfPoints(28)));
    assert_eq!(d.default_para.space_after, Some(bo_model::Twips(120)));
}

// Borders and Shading › Options…: the distances from the text come back to the dialog.
#[test]
fn border_options_set_the_distances() {
    let mut w = window();
    assert!(w.action("format.bordersShading"));
    w.form_out(FormOut::Stash(DocFormKind::BorderOptions));
    let f = w.r3.form.as_mut().expect("Border and Shading Options");
    assert_eq!(f.kind, DocFormKind::BorderOptions);
    f.set_text("boTop", "6 pt");
    f.set_text("boRight", "40 pt");
    enter(&mut w);
    let f = w.r3.form.as_ref().expect("Borders and Shading again");
    assert_eq!(f.kind, DocFormKind::BordersShading);
    // §17.3.4: at most 31 points.
    assert_eq!(f.bs_space()[0], 6);
    assert_eq!(f.bs_space()[3], 31);
}

// Borders and Shading › Shadow and Shading › Patterns reach the model (§17.18.2 w:shadow,
// §17.3.1.31 shd w:val / w:color) through the commands the dialog's OK runs.
#[test]
fn shadow_edges_and_shading_pattern_commands() {
    let mut w = window();
    let pen = serde_json::json!({ "style": "single", "size": 4, "color": null, "shadow": true });
    assert!(w
        .view
        .run_json(&serde_json::json!({ "cmd": "formatParagraph", "format": {
            "kind": "setBorders", "top": pen, "left": pen, "bottom": pen, "right": pen,
            "between": null, "space": [1, 4, 1, 4] } }))
        .is_some());
    assert!(w
        .view
        .run_json(&serde_json::json!({ "cmd": "formatParagraph", "format": {
            "kind": "shading", "hex": "FFFF00", "pattern": { "val": "pct25", "color": "FF0000" } } }))
        .is_some());
    let Some(bo_model::Block::Paragraph(p)) = doc(&w).sections[0].blocks.first() else {
        panic!("paragraph");
    };
    let b = p.props.borders.as_ref().expect("borders");
    assert!(b.top.as_ref().is_some_and(|e| e.shadow && !e.frame));
    let sp = p.props.shading_pattern.as_ref().expect("pattern");
    assert_eq!(sp.val, "pct25");
    assert_eq!(
        sp.color.map(bo_model::Color::to_hex).as_deref(),
        Some("FF0000")
    );
}

// Styles pane › Options…: All styles, Alphabetical (the reference's Style Pane Options).
#[test]
fn style_pane_options_show_all_alphabetically() {
    let mut w = window();
    assert!(w.action("view.stylesPane"));
    let before = w.r3.styles_pane.as_ref().expect("pane").styles.len();
    assert!(w.action(r#"r13.paneOptions:{"show":3,"sort":1,"template":false}"#));
    let pane = w.r3.styles_pane.as_ref().expect("pane");
    assert!(pane.styles.len() >= before);
    let names: Vec<String> = pane.styles.iter().map(|s| s.name.to_lowercase()).collect();
    let mut sorted = names.clone();
    sorted.sort();
    assert_eq!(names, sorted);
    // The dialog opens with the pane's choices.
    assert!(w.action("styles.paneOptions"));
    let f = w.r3.form.as_ref().expect("Style Pane Options");
    assert_eq!(f.kind, DocFormKind::PaneOptions);
    assert_eq!(f.sel("show"), Some(3));
    assert_eq!(f.sel("sort"), Some(1));
}

// Page Setup › Line Numbers… (§17.6.8 lnNumType): Count by 5, restart each section.
#[test]
fn line_numbers_dialog_sets_the_section() {
    let mut w = window();
    assert!(w.action("layout.pageSetup"));
    w.form_out(FormOut::Stash(DocFormKind::LineNumbers));
    let f = w.r3.form.as_mut().expect("Line Numbers");
    assert_eq!(f.kind, DocFormKind::LineNumbers);
    f.set_on("lnOn", true);
    f.set_text("lnBy", "5");
    f.set_text("lnStart", "3");
    f.set_radio("lnSection");
    enter(&mut w);
    assert_eq!(
        w.r3.form.as_ref().map(|f| f.kind),
        Some(DocFormKind::PageSetup)
    );
    let ln = doc(&w).sections[0].line_numbers.expect("line numbers");
    assert_eq!((ln.count_by, ln.start), (5, 2));
    assert_eq!(ln.restart, bo_model::LineNumberRestart::NewSection);
}
