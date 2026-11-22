//! BlueDoc parity round 12: Borders and Shading, Table Positioning, table alt text.
use super::super::DocWindow;
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

fn enter(w: &mut DocWindow) {
    let out =
        w.r3.form
            .as_mut()
            .expect("form")
            .input(&Input::Key(Key::Enter, Mods::default()));
    assert!(matches!(out, FormOut::Run(..)), "{out:?}");
    w.form_out(out);
}

// source: the reference's Borders and Shading (captured round 12): Setting None / Box /
// Shadow / 3-D / Custom for a paragraph; the edge buttons toggle single edges (Custom).
#[test]
fn paragraph_box_then_custom_edges() {
    let mut w = window();
    assert!(w.action("format.bordersShading"));
    w.handle(Input::Tick, &Fixed);
    let f = w.r3.form.as_mut().expect("Borders and Shading");
    assert_eq!(f.kind, DocFormKind::BordersShading);
    assert_eq!(f.items_for_test("bApply"), ["Paragraph"]);
    f.canvas_for_test("bSet1", 10.0, 10.0);
    assert!((0..4).all(|i| f.bs_edge_for_test(0, i).is_some()));
    assert!(f.bs_edge_for_test(0, 4).is_none());
    // One paragraph: top and bottom beside the diagram, left and right under it (the line
    // between paragraphs only for a selection over several).
    let buttons = f.bs_buttons_for_test(0);
    assert_eq!(buttons.len(), 4);
    let (_, left) = *buttons.iter().find(|b| b.0 == 1).expect("left edge button");
    f.canvas_for_test("bPreview", left[0] + 5.0, left[1] + 5.0);
    assert!(f.bs_edge_for_test(0, 1).is_none(), "left edge toggled off");
    enter(&mut w);
    let b = w.view.format().para_borders.clone().expect("borders");
    assert!(b.top.is_some() && b.bottom.is_some() && b.right.is_some());
    assert!(b.left.is_none());
    // Back in the dialog the paragraph's edges show and the setting is Custom.
    w.action("format.bordersShading");
    let f = w.r3.form.as_mut().expect("again");
    assert!(f.bs_edge_for_test(0, 0).is_some() && f.bs_edge_for_test(0, 1).is_none());
}

// source: the reference's Table Properties › Borders and Shading… for a table: Apply to Cell /
// Table, settings None / Box / All / Grid / Custom; Shading › Fill.
#[test]
fn table_grid_and_shading_from_table_properties() {
    let mut w = window();
    w.view
        .run_command(Command::InsertTable { rows: 2, cols: 2 });
    assert!(w.action(crate::doc_table::DIALOGS[0]));
    let f = w.r3.form.as_mut().expect("Table Properties");
    let out = f.click_for_test("borders");
    assert_eq!(out, FormOut::Stash(DocFormKind::BordersShading));
    w.form_out(out);
    let f = w.r3.form.as_mut().expect("Borders and Shading");
    assert_eq!(f.items_for_test("bApply"), ["Paragraph", "Cell", "Table"]);
    f.set_sel("bApply", Some(2));
    f.changed_for_test("bApply");
    f.canvas_for_test("bSet3", 10.0, 10.0);
    assert_eq!(
        f.bs_edge_for_test(0, 4).map(|e| e.1),
        Some(4),
        "grid inside ½ pt"
    );
    assert_eq!(f.bs_buttons_for_test(0).len(), 6);
    f.set_tab(2);
    enter(&mut w);
    // Back to Table Properties, still open.
    assert_eq!(
        w.r3.form.as_ref().map(|f| f.kind),
        Some(DocFormKind::TableProps)
    );
    let t = w.view.format().table.clone().expect("table");
    let b = t.borders.expect("table borders");
    assert!(b.inside_h.is_some_and(|l| l.style == "single"));
}

// source: the reference's Table Properties › Alt Text (Title, Description) and Table
// Positioning (Horizontal / Vertical position and relative to, distances, Move with text,
// Allow overlap).
#[test]
fn alt_text_and_positioning() {
    let mut w = window();
    w.view
        .run_command(Command::InsertTable { rows: 2, cols: 2 });
    assert!(w.action(crate::doc_table::DIALOGS[0]));
    let f = w.r3.form.as_mut().expect("Table Properties");
    f.set_text("aTitle", "Sales");
    f.set_text("aDesc", "By region");
    // Around, then Positioning….
    f.canvas_for_test("wrAround", 5.0, 5.0);
    let out = f.click_for_test("positioning");
    assert_eq!(out, FormOut::Stash(DocFormKind::TablePosition));
    w.form_out(out);
    let f = w.r3.form.as_mut().expect("Table Positioning");
    assert_eq!(f.items_for_test("hPick")[1], "Center");
    f.set_sel("hPick", Some(1));
    f.changed_for_test("hPick");
    assert_eq!(f.text("hPos"), "Center");
    f.set_sel("vRel", Some(1));
    f.changed_for_test("vRel");
    assert!(!f.on("move"), "Page is not Move with text");
    f.set_text("vPos", "1\"");
    f.set_on("overlap", false);
    enter(&mut w);
    assert!(
        w.view
            .format()
            .table
            .as_ref()
            .is_some_and(|t| t.float.is_some()),
        "positioned: {:?}",
        w.view.format().table
    );
    let f = w.r3.form.as_mut().expect("back to Table Properties");
    assert_eq!(f.kind, DocFormKind::TableProps);
    enter(&mut w);
    let t = w.view.format().table.clone().expect("table");
    assert_eq!(
        (t.caption.as_str(), t.description.as_str()),
        ("Sales", "By region")
    );
    let fl = t.float.expect("floating");
    assert_eq!(fl.x_align.as_deref(), Some("center"));
    assert_eq!(fl.vert_anchor.as_deref(), Some("page"));
    assert_eq!(fl.y.0, 1440);
    assert!(!t.overlap);
}

// source: the reference's Insert › Table › Quick Tables (captured round 12): a "Built-In"
// gallery of table designs; choosing one inserts it at the caret. The designs are our own.
#[test]
fn quick_tables_gallery_inserts_a_design() {
    let mut w = window();
    assert!(w.r12_open_quick([100.0, 200.0, 280.0, 36.0]));
    let q = w.r12.quick.as_ref().expect("gallery");
    let names: Vec<&str> = q.entries.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(names.len(), 6, "{names:?}");
    assert!(names.contains(&"Lịch tháng"));
    // Inside the second entry (band 38, entries 200 px at scale 1.25).
    let (x, y) = (480.0, 200.0 + 38.0 + 200.0 + 60.0);
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
    assert!(w.r12.quick.is_none());
    let doc = w.view.engine().document().expect("document");
    let tables = doc.sections[0]
        .blocks
        .iter()
        .filter(|b| matches!(b, bo_model::Block::Table(_)))
        .count();
    assert_eq!(tables, 1, "the design is in the document");
}
