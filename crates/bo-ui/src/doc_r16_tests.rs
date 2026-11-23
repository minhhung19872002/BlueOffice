//! BlueDoc parity round 16: the Review tab laid out as the reference's (collapsed groups
//! with their drop-down, the markup box, Delete ▾, Show Markup ▸ sub-menus).
use super::super::DocWindow;
use crate::popup::Choice;
use crate::ribbon::RibbonHit;
use crate::{DocView, Input, TextMeasure, TextStyle};
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

/// Two comments by two reviewers.
fn commented() -> DocWindow {
    let mut w = window();
    w.run_interface_command("text:Hello world again", &Fixed);
    for (a, b, who) in [(0, 5, "Ann"), (6, 11, "Bob")] {
        w.r3_run(vec![
            json!({ "cmd": "setAuthor", "name": who }),
            json!({ "cmd": "setSelection", "selection": {
                "anchor": { "p": 0, "o": a }, "focus": { "p": 0, "o": b } } }),
            json!({ "cmd": "addComment", "text": format!("note by {who}") }),
        ]);
    }
    w.handle(Input::Tick, &Fixed);
    w
}

fn enter(w: &mut DocWindow) {
    let out =
        w.r3.form
            .as_mut()
            .expect("form")
            .input(&Input::Key(crate::Key::Enter, crate::Mods::default()));
    w.form_out(out);
}

fn click(w: &mut DocWindow, r: [f32; 4]) {
    let (x, y) = (r[0] + r[2] / 2.0, r[1] + r[3] / 2.0);
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
}

// source: the reference's Review tab at 1500 px, 125 % (captured round 16): Comments and
// Tracking are one drop-down button each; the rest of the tab fits without scrolling.
#[test]
fn review_tab_fits_with_collapsed_groups() {
    let mut w = window();
    w.run_interface_command("tab:review", &Fixed);
    w.handle(Input::Tick, &Fixed);
    assert!(
        w.ribbon.rect_of(RibbonHit::ScrollRight).is_none(),
        "no scrolling"
    );
    // Collapsed or not depends on the measured widths; the commands are reachable.
    for (group, inner) in [
        ("review.commentsGroup", "review.showComments"),
        ("review.trackingGroup", "review.acceptMenu"),
    ] {
        let r = |id| w.ribbon.rect_of(RibbonHit::Command(id));
        assert!(r(group).is_some() || r(inner).is_some(), "{group}");
    }
    for id in [
        "review.thesaurus",
        "review.markupMode",
        "review.compareMenu",
        "review.restrictEditing",
    ] {
        assert!(w.ribbon.rect_of(RibbonHit::Command(id)).is_some(), "{id}");
    }
}

// source: the reference's collapsed Comments group opens a panel holding the whole group.
#[test]
fn collapsed_group_shows_its_commands_and_runs_them() {
    let mut w = commented();
    w.run_interface_command("tab:review", &Fixed);
    w.handle(Input::Tick, &Fixed);
    let b = w
        .ribbon
        .rect_of(RibbonHit::Command("review.commentsGroup"))
        .expect("button");
    click(&mut w, b);
    let pop = w.r16.pop.as_ref().expect("drop-down open");
    let show = pop
        .rect_of("review.showComments")
        .expect("Show Comments in it");
    assert!(show[1] >= b[1] + b[3], "under the button");
    click(&mut w, show);
    assert!(w.r16.pop.is_none(), "closes after a command");
    assert!(w.comments_pane.is_some(), "Show Comments ran");
    // Delete ▾ in the drop-down opens its menu.
    click(&mut w, b);
    let del = w
        .r16
        .pop
        .as_ref()
        .and_then(|p| p.rect_of("review.deleteMenu"));
    click(&mut w, del.expect("Delete"));
    let items = w.menu.as_ref().expect("Delete menu").items().len();
    assert_eq!(items, 3);
}

// source: the reference's Delete ▾ › Delete All Comments Shown / in Document.
#[test]
fn delete_all_comments_shown_keeps_hidden_reviewers() {
    let mut w = commented();
    assert_eq!(w.view.comments().len(), 2);
    w.r3_run(vec![
        json!({ "cmd": "setMarkup", "hiddenReviewers": ["Bob"] }),
    ]);
    assert!(w.action("review.deleteShown"));
    let left: Vec<String> = w.view.comments().iter().map(|c| c.author.clone()).collect();
    assert_eq!(left, ["Bob"]);
    assert!(w.action("review.deleteAll"));
    assert!(w.view.comments().is_empty());
    // One undo step brings the shown deletion back... the last one first.
    w.action("edit.undo");
    assert_eq!(w.view.comments().len(), 1);
}

// source: the reference's Show Markup ▾ › Balloons ▸ / Specific People ▸.
#[test]
fn show_markup_balloons_and_people() {
    let mut w = commented();
    w.run_interface_command("tab:review", &Fixed);
    w.handle(Input::Tick, &Fixed);
    assert!(w.action("review.showMarkup"));
    let n = w.menu.as_ref().expect("menu").items().len();
    assert_eq!(n, 5);
    assert!(w.action("review.markup.balloonsSub"));
    let rows = w.menu.as_ref().expect("Balloons").items();
    assert_eq!(rows.len(), 3);
    assert!(
        rows[2].checked,
        "comments and formatting in balloons by default"
    );
    assert!(w.action("review.balloons.inline"));
    assert!(w.view.markup().inline);
    assert!(w.action("review.balloons.revisions"));
    assert!(w.view.markup().deletion_balloons && !w.view.markup().inline);
    // Specific People: All Reviewers, Ann, Bob.
    assert!(w.action("review.markup.peopleSub"));
    let people: Vec<String> = w
        .menu
        .as_ref()
        .expect("people")
        .items()
        .iter()
        .map(|i| i.label.clone())
        .collect();
    assert_eq!(people, ["All Reviewers", "Ann", "Bob"]);
    let Some(Choice::Command(bob)) = w.menu.as_ref().map(|m| m.items()[2].choice.clone()) else {
        panic!("command row");
    };
    assert!(w.action(&bob));
    assert_eq!(w.view.markup().hidden_reviewers, ["Bob"]);
    assert!(w.action("review.people.all"));
    assert!(w.view.markup().hidden_reviewers.is_empty());
    assert!(w.action("review.people.all"));
    assert_eq!(w.view.markup().hidden_reviewers, ["Ann", "Bob"]);
}

// source: the reference's Track Changes Options (Review › Markup launcher, captured round
// 16) and its Advanced Options… dialog: OK changes the review display.
#[test]
fn track_changes_options_change_the_display() {
    use crate::doc_forms::DocFormKind;
    let mut w = commented();
    assert!(w.action("review.trackingOptions"));
    let f = w.r3.form.as_mut().expect("Track Changes Options");
    assert_eq!(f.kind, DocFormKind::TrackOptions);
    // Comments off, balloons show nothing (inline), Reviewing Pane vertical.
    f.set_on("comments", false);
    f.set_sel("balloons", Some(1));
    f.set_sel("pane", Some(1));
    enter(&mut w);
    assert!(w.r3.form.is_none());
    let m = w.view.markup().clone();
    assert!(!m.comments && m.inline);
    assert_eq!(w.r6.review.as_ref().map(|p| p.horizontal), Some(false));
    // Advanced: deletions double strikethrough in red, no connecting lines, 3" balloons.
    assert!(w.action("review.trackingOptions"));
    let out = w.r3.form.as_mut().expect("form").click_for_test("advanced");
    w.form_out(out);
    let f = w.r3.form.as_mut().expect("Advanced");
    assert_eq!(f.kind, DocFormKind::AdvTrackOptions);
    f.set_sel("del", Some(2));
    f.set_sel("delColor", Some(7));
    f.set_text("width", "3\"");
    f.set_on("connectors", false);
    enter(&mut w);
    let m = w.view.markup().clone();
    assert_eq!(m.del_mark, "doubleStrikethrough");
    assert_eq!(m.del_color, "FF0000");
    assert!(!m.connectors);
    assert!((m.balloon_width - 3.0).abs() < 0.01);
    // Back to Track Changes Options (kept under the Advanced dialog).
    assert_eq!(
        w.r3.form.as_ref().map(|f| f.kind),
        Some(DocFormKind::TrackOptions)
    );
}

// source: the reference's Finish & Merge ▾ › Edit Individual Documents… (Merge to New
// Document: All / Current record / From–To) and Highlight Merge Fields.
#[test]
fn finish_and_merge_opens_the_merged_document_apart() {
    use crate::doc_forms::DocFormKind;
    let mut w = window();
    w.run_interface_command("text:Dear ", &Fixed);
    w.r3_run(vec![
        json!({ "cmd": "mergeTable", "name": "list", "fields": ["Name"],
            "rows": [["Ann"], ["Bob"], ["Cy"]] }),
        json!({ "cmd": "mergeInsertField", "name": "Name" }),
    ]);
    w.handle(Input::Tick, &Fixed);
    assert!(w.action("mailings.highlightFields"));
    assert!(w.on_ids().contains(&"mailings.highlightFields"));
    assert!(w.action("mailings.finish"));
    assert_eq!(w.menu.as_ref().expect("menu").items().len(), 3);
    assert!(w.action("mailings.finish.edit"));
    let f = w.r3.form.as_mut().expect("Merge to New Document");
    assert_eq!(f.kind, DocFormKind::MergeNew);
    f.set_radio("from");
    f.set_text("fromN", "2");
    f.set_text("toN", "3");
    enter(&mut w);
    let bytes = w.take_new_document().expect("merged document");
    let mut e = Engine::with_bundled_fonts();
    e.open(&bytes, "merged.docx").expect("docx");
    let text = e
        .document()
        .map(bo_model::Document::plain_text)
        .unwrap_or_default();
    assert!(text.contains("Dear Bob") && text.contains("Dear Cy") && !text.contains("Ann"));
    assert!(w.take_new_document_actions().is_none());
    // Print Documents: the new window opens on its Print page.
    assert!(w.action("r16.merge:print:all"));
    assert_eq!(w.take_new_document_actions().as_deref(), Some("file.print"));
}

// source: the reference's Thesaurus pane (Review › Thesaurus, captured round 16).
#[test]
fn thesaurus_pane_inserts_a_synonym() {
    let mut w = window();
    w.run_interface_command("text:The quick fox", &Fixed);
    w.r3_run(vec![json!({ "cmd": "setSelection", "selection": {
        "anchor": { "p": 0, "o": 5 }, "focus": { "p": 0, "o": 5 } } })]);
    assert!(w.action("review.thesaurus"));
    w.handle(Input::Tick, &Fixed);
    let t = w.r16.thes.as_ref().expect("pane");
    assert_eq!(t.word, "quick");
    assert!(
        w.doc_right() < 1500.0 - 200.0,
        "the pane takes the right side"
    );
    w.r16.thes_word = Some("fast".to_owned());
    assert!(w.action("review.thes.insert"));
    let text = w.view.engine().session().and_then(|s| s.para_text(0));
    assert_eq!(text.as_deref(), Some("The fast fox"));
    assert!(w.action("review.thesaurus"), "toggles closed");
    assert!(w.r16.thes.is_none());
}
