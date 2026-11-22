//! BlueDoc parity round 14: Index and Tables (Table of Figures, Table of Authorities, tab
//! switching), Mark Citation, the Table of Authorities ribbon group, Tabs › Default tab stops
//! and Paragraph › Collapsed by default.
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

/// "Smith v. Jones" / "two", the caret in the first paragraph.
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
        text: "Smith v. Jones".into(),
    });
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

/// The document model as JSON (field instructions are searchable in it).
fn model_json(w: &DocWindow) -> String {
    serde_json::to_string(doc(w)).expect("json")
}

// source: the reference's References tab (captured round 14): a Table of Authorities group
// with Mark Citation, Insert Table of Authorities and Update Table; Index › Update Index.
#[test]
fn references_tab_has_the_authorities_group() {
    let mut w = window();
    assert!(w.run_interface_command("tab:references", &Fixed));
    for id in [
        "references.markCitation",
        "references.insertToa",
        "references.updateToa",
        "references.updateIndex",
    ] {
        assert!(
            w.ribbon
                .rect_of(crate::ribbon::RibbonHit::Command(id))
                .is_some(),
            "{id} missing from the References tab"
        );
        assert!(crate::commands::enabled(id, w.view.format()), "{id} dimmed");
        assert_ne!(
            w.strings.get(&format!("commands.{id}")),
            format!("commands.{id}")
        );
        assert!(w.icons.has(id), "{id} has no icon");
    }
}

// source: the reference's Mark Citation dialog (captured round 14): the selected text fills
// the short and long citations; Mark adds a §17.16.5.62 TA field and the dialog stays open.
#[test]
fn mark_citation_marks_the_selection_and_stays_open() {
    let mut w = window();
    w.view.run_command(Command::SetSelection {
        selection: Selection {
            anchor: Pos::new(0, 0),
            focus: Pos::new(0, 14),
        },
    });
    assert!(w.action("references.markCitation"));
    let f = w.r3.form.as_mut().expect("Mark Citation");
    assert_eq!(f.kind, DocFormKind::MarkCitation);
    assert_eq!(f.text("short"), "Smith v. Jones");
    assert_eq!(f.text("long"), "Smith v. Jones");
    let out = f.input(&Input::Key(Key::Enter, Mods::default()));
    assert!(matches!(out, FormOut::Run(_, false)), "{out:?}");
    w.form_out(out);
    assert!(w.r3.form.is_some(), "Mark Citation stays open");
    assert!(model_json(&w).contains(r#"TA \\l \"Smith v. Jones\""#));
}

// source: the reference's Index and Tables › Table of Authorities (captured round 14).
#[test]
fn table_of_authorities_tab_inserts_a_toa_field() {
    let mut w = window();
    assert!(w.action("references.insertToa"));
    let f = w.r3.form.as_mut().expect("Index and Tables");
    assert_eq!(f.kind, DocFormKind::Toa);
    f.set_sel("category", Some(2));
    enter(&mut w);
    let j = model_json(&w);
    assert!(j.contains(r#"TOA \\h \\c \"2\" \\p"#), "{j}");
}

// source: the reference's Index and Tables › Table of Figures (captured round 14): TOC \c
// with the caption label.
#[test]
fn table_of_figures_dialog_inserts_a_figures_table() {
    let mut w = window();
    assert!(w.action("references.tableOfFigures"));
    let f = w.r3.form.as_mut().expect("Index and Tables");
    assert_eq!(f.kind, DocFormKind::Tof);
    f.set_sel("label", Some(3));
    enter(&mut w);
    assert!(model_json(&w).contains(r#"TOC \\h \\z \\c \"Table\""#));
}

// Picking a tab of Index and Tables opens that tab's dialog in place.
#[test]
fn index_and_tables_tabs_switch_dialogs() {
    let mut w = window();
    assert!(w.action("references.toc.custom"));
    let f = w.r3.form.as_mut().expect("Index and Tables");
    assert_eq!(f.kind, DocFormKind::Toc);
    assert_eq!(f.pick_tab_for_test(1), None);
    let out = f.pick_tab_for_test(3).expect("switch");
    assert_eq!(out, FormOut::Open(DocFormKind::Toa));
    w.form_out(out);
    let f = w.r3.form.as_mut().expect("Index and Tables");
    assert_eq!(f.kind, DocFormKind::Toa);
    let out = f.pick_tab_for_test(0).expect("switch");
    w.form_out(out);
    assert_eq!(w.r3.form.as_ref().map(|f| f.kind), Some(DocFormKind::Index));
}

// Tabs › Default tab stops sets the document's §17.15.1.25 defaultTabStop (one undo step).
#[test]
fn tabs_dialog_sets_the_default_tab_stop() {
    let mut w = window();
    assert!(w.action("format.tabs"));
    let inches = w.units.inches;
    let f = w.r3.form.as_mut().expect("Tabs");
    assert_eq!(f.kind, DocFormKind::Tabs);
    f.set_text("defTab", &show_len(567, inches));
    enter(&mut w);
    let t = doc(&w).default_tab_stop.expect("set").0;
    assert!((t - 567).abs() <= 2, "{t}");
    w.view.run_command(Command::Undo);
    assert_eq!(doc(&w).default_tab_stop, None);
}

// Paragraph › Collapsed by default ([MS-DOCX] w15:collapsed) for an outline level.
#[test]
fn paragraph_dialog_sets_collapsed_by_default() {
    let mut w = window();
    assert!(w.action("format.paragraphSettings"));
    let f = w.r3.form.as_mut().expect("Paragraph");
    f.set_sel("outline", Some(1));
    f.set_tri("collapsed", Some(true));
    enter(&mut w);
    let Some(bo_model::Block::Paragraph(p)) = doc(&w).sections[0].blocks.first() else {
        panic!("paragraph");
    };
    assert_eq!(p.props.collapsed, Some(true));
    assert_eq!(p.props.outline_level, Some(0));
}

// source: the reference's Design › Watermark › Custom Watermark… (Printed Watermark,
// captured round 14): a text watermark set from the dialog, then removed with No watermark.
#[test]
fn printed_watermark_dialog_sets_and_removes_a_text_watermark() {
    let mut w = window();
    assert!(w.action("design.watermarkCustom"));
    let f = w.r3.form.as_mut().expect("Printed Watermark");
    assert_eq!(f.kind, DocFormKind::Watermark);
    f.choose("wmText");
    f.set_text("wmTextBox", "BẢN NHÁP");
    f.choose("horizontal");
    enter(&mut w);
    let wm = w
        .view
        .engine()
        .session()
        .and_then(bo_edit::Session::watermark)
        .expect("watermark");
    assert_eq!(wm.text.as_deref(), Some("BẢN NHÁP"));
    assert!(!wm.diagonal);
    // Opened again it shows the watermark; No watermark removes it.
    assert!(w.action("design.watermarkCustom"));
    let f = w.r3.form.as_mut().expect("Printed Watermark");
    assert_eq!(f.text("wmTextBox"), "BẢN NHÁP");
    f.choose("wmNone");
    enter(&mut w);
    assert!(w
        .view
        .engine()
        .session()
        .and_then(bo_edit::Session::watermark)
        .is_none());
}

// Design › Watermark and Page Color open their galleries under the buttons; Escape closes.
#[test]
fn design_watermark_and_page_color_open_galleries() {
    let mut w = window();
    assert!(w.run_interface_command("tab:design", &Fixed));
    assert!(w.action("design.watermark"));
    assert!(w.r14.wm.is_some() && w.menu.is_none());
    w.handle(Input::Key(Key::Escape, Mods::default()), &Fixed);
    assert!(w.r14.wm.is_none());
    assert!(w.action("design.pageColor"));
    assert!(w.r14.pal.is_some());
    w.handle(Input::Key(Key::Escape, Mods::default()), &Fixed);
    assert!(w.r14.pal.is_none());
}
