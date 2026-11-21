//! BlueDoc parity round 6: File screen, Options, Reviewing Pane, Track Changes split button,
//! Accept / Reject menus.
use super::super::{DocWindow, Request};
use crate::doc_backstage6::BsPage;
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

fn run(w: &mut DocWindow, id: &str) -> bool {
    w.run_interface_command(id, &Fixed)
}

#[test]
fn file_tab_opens_the_file_screen_and_escape_leaves() {
    let mut w = window();
    assert!(run(&mut w, "file:print"));
    assert_eq!(w.r6.bs.as_ref().map(|b| b.page), Some(BsPage::Print));
    // The print preview asks for an image of the first page.
    w.handle(Input::Blink, &Fixed);
    assert!(w
        .r6
        .bs
        .as_ref()
        .and_then(crate::doc_backstage6::DocBackstage::preview_rect)
        .is_some());
    w.handle(
        Input::Key(crate::Key::Escape, crate::Mods::default()),
        &Fixed,
    );
    assert!(w.r6.bs.is_none());
}

#[test]
fn options_store_preferences_and_apply_them() {
    let mut w = window();
    w.set_prefs("userName=An Nguyen\nquick=file.save,file.print\nshowMarks=1");
    assert!(w.marks, "formatting marks on from the preferences");
    assert!(run(&mut w, "options:8"));
    assert!(w.r6.options.is_some());
    w.handle(
        Input::Key(crate::Key::Escape, crate::Mods::default()),
        &Fixed,
    );
    assert!(w.r6.options.is_none());
    w.r6_store_prefs(crate::doc_forms::options6::DocPrefs {
        check_spelling: false,
        ..w.r6.prefs.clone()
    });
    assert!(w
        .take_prefs()
        .is_some_and(|t| t.contains("checkSpelling=0")));
    assert!(w.requests.contains(&Request::SavePrefs));
}

#[test]
fn templates_fill_a_new_document() {
    let mut w = window();
    assert!(w.apply_template("report"));
    let doc = w.view.engine().document().expect("doc");
    let text: String = bo_model::index::body_paragraphs(doc)
        .iter()
        .filter_map(|p| bo_model::index::para_at(doc, p))
        .map(bo_model::Paragraph::plain_text)
        .collect();
    assert!(text.contains("Report Title"), "{text}");
}

#[test]
fn reviewing_pane_lists_revisions_and_takes_the_left_edge() {
    let mut w = window();
    run(&mut w, "review.trackChanges");
    run(&mut w, "text:Hello");
    run(&mut w, "review.reviewingPane.vertical");
    w.handle(Input::Blink, &Fixed);
    let pane = w.r6.review.as_ref().expect("pane");
    assert_eq!(pane.rows.len(), 1);
    assert_eq!(pane.rows[0].what, "Added");
    assert!(w.nav_w() > 0.0, "the document moves right of the pane");
    run(&mut w, "review.reviewingPane.vertical");
    assert!(w.r6.review.is_none());
}

#[test]
fn track_changes_split_button_and_lock_tracking() {
    let mut w = window();
    assert!(crate::catalog::is_menu("review.trackChanges"));
    run(&mut w, "tab:review");
    let r = w
        .ribbon
        .rect_of(RibbonHit::Command("review.trackChanges"))
        .expect("button");
    // Lower part: the menu (Track Changes / Lock Tracking).
    let (x, y) = (r[0] + r[2] / 2.0, r[1] + r[3] * 0.85);
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
    assert!(w.menu.is_some(), "menu opens from the label");
    assert!(!w.view.track_changes(), "the label does not toggle");
    w.menu = None;
    run(&mut w, "review.lockTracking");
    assert!(w.view.track_changes());
    assert!(w.view.tracking_locked());
    // Locked: tracking cannot be turned off.
    run(&mut w, "review.trackChanges");
    assert!(w.view.track_changes());
    run(&mut w, "review.lockTracking");
    assert!(!w.view.tracking_locked());
}

#[test]
fn accept_all_and_stop_tracking() {
    let mut w = window();
    run(&mut w, "review.trackChanges");
    run(&mut w, "text:Hello");
    assert_eq!(w.view.revisions().len(), 1);
    run(&mut w, "review.acceptAllStop");
    assert_eq!(w.view.revisions().len(), 0);
    assert!(!w.view.track_changes());
}

#[test]
fn all_markup_opens_the_markup_area_and_moves_pages_left() {
    let mut w = window();
    run(&mut w, "text:Hello world");
    let x0 = w.view.slots()[0].x;
    run(&mut w, "select:world");
    run(&mut w, r#"engine:{"cmd":"addComment","text":"Check"}"#);
    w.handle(Input::Blink, &Fixed);
    assert!(w.view.markup_px() > 0.0, "balloons need the markup area");
    assert!(w.view.slots()[0].x < x0, "the page moves left of the area");
}
