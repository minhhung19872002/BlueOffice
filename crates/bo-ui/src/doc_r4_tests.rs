//! BlueDoc parity round 4: Insert / Layout / View galleries and menus, posted clicks.
use super::super::DocWindow;
use crate::ribbon::RibbonHit;
use crate::{DocView, Input, TextMeasure, TextStyle};
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
    w
}

fn center(r: [f32; 4]) -> (f32, f32) {
    (r[0] + r[2] / 2.0, r[1] + r[3] / 2.0)
}

/// A click as a measurement script posts it: move, the pointer leaves (it is elsewhere),
/// press, release.
fn posted_click(w: &mut DocWindow, (x, y): (f32, f32)) {
    w.handle(Input::Move { x, y }, &Fixed);
    w.handle(Input::Leave, &Fixed);
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
    w.handle(Input::Tick, &Fixed);
}

#[test]
fn a_posted_click_on_a_tab_selects_that_tab() {
    let mut w = window();
    for i in [1, 3, 6] {
        let r = w.ribbon.rect_of(RibbonHit::Tab(i)).expect("tab");
        posted_click(&mut w, center(r));
        assert_eq!(w.ribbon.active_tab(), i);
    }
}

fn run(w: &mut DocWindow, id: &str) -> bool {
    let r = w.run_interface_command(id, &Fixed);
    w.handle(Input::Tick, &Fixed);
    r
}

fn body_text(w: &DocWindow) -> String {
    w.view
        .engine()
        .document()
        .map(bo_model::Document::plain_text)
        .unwrap_or_default()
}

#[test]
fn insert_galleries_open_and_their_tiles_run() {
    let mut w = window();
    run(&mut w, "tab:insert");
    for g in crate::doc_r4_menus::GALLERIES {
        if g.starts_with("layout.") {
            continue;
        }
        assert!(run(&mut w, g), "{g}");
        assert!(w.r3.tiles.is_some(), "{g}");
        w.r3.tiles = None;
    }
    assert!(run(&mut w, "insert.coverPage.horizon"));
    assert!(
        body_text(&w).contains("DOCUMENT TITLE"),
        "{}",
        body_text(&w)
    );
    assert!(run(&mut w, "insert.equation.pythagoras"));
    assert!(run(&mut w, "insert.chart.column"));
    assert!(run(&mut w, "insert.smartArt.process"));
    assert!(run(&mut w, "insert.quickParts.author"));
    assert!(run(&mut w, "insert.blankPage"));
}

#[test]
fn layout_galleries_mark_the_current_setup() {
    let mut w = window();
    run(&mut w, "tab:layout");
    assert!(run(&mut w, "layout.orientation"));
    assert_eq!(
        w.r3.tiles
            .as_ref()
            .and_then(|t| t.current.clone())
            .as_deref(),
        Some("layout.orientation.portrait")
    );
    w.r3.tiles = None;
    assert!(run(&mut w, "layout.orientation.landscape"));
    let sec = &w.view.format().section;
    assert!(sec.width > sec.height);
    assert!(run(&mut w, "layout.columns.left"));
    assert_eq!(w.view.format().section.columns, 2);
}

#[test]
fn view_toggles_flip() {
    let mut w = window();
    assert!(run(&mut w, "view.gridlines"));
    assert!(w.r4.gridlines);
    assert!(run(&mut w, "view.focus"));
    assert!(w.r4.focus);
}

#[test]
fn date_and_time_lists_the_formats_and_zoom_sets_the_view() {
    assert_eq!(super::civil(0), [1970, 1, 1, 4, 0, 0, 0]);
    // 2026-11-16 16:30:47 UTC, a Monday.
    let now = super::civil(1_794_846_647);
    assert_eq!(now, [2026, 11, 16, 1, 16, 30, 47]);
    let f = |p: &str| crate::doc_forms::format_date(p, now);
    assert_eq!(f("dddd, MMMM d, yyyy"), "Monday, November 16, 2026");
    assert_eq!(f("d-MMM-yy"), "16-Nov-26");
    assert_eq!(f("M/d/yyyy h:mm am/pm"), "11/16/2026 4:30 PM");
    assert_eq!(f("HH:mm:ss"), "16:30:47");
    let mut w = window();
    assert!(run(&mut w, "insert.dateTime"));
    assert_eq!(
        w.r3_form_kind(),
        Some(crate::doc_forms::DocFormKind::DateTime)
    );
    assert!(run(&mut w, "view.zoomTo.200"));
    assert!((w.view.zoom() - 2.0).abs() < 0.01);
}

#[test]
fn layout_paragraph_boxes_step_the_indent_and_spacing() {
    let mut w = window();
    run(&mut w, "tab:layout");
    let id = crate::doc_table::PARA_SPINS[2];
    let r = w.ribbon.rect_of(RibbonHit::Command(id)).expect("box");
    let b = crate::doc_table::para_rect(r, w.scale);
    // The up arrow: 6 pt more space before.
    let up = (b[0] + b[2] - 3.0, b[1] + 3.0);
    let before = w.view.format().space_before;
    posted_click(&mut w, up);
    assert_eq!(w.view.format().space_before, before + 120);
}
