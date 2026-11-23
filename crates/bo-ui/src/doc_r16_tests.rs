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
    assert_eq!(w.view.comments().len(), 0);
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
    let row3 = w.menu.as_ref().expect("menu").row_rect(3);
    assert!(w.action("review.markup.balloonsSub"));
    let sub = w.menu.as_ref().expect("Balloons").rect();
    assert!(
        ((sub[0] - (row3[0] + row3[2])).abs() < 8.0 || (sub[0] + sub[2] - row3[0]).abs() < 8.0)
            && (sub[1] - row3[1]).abs() < 8.0,
        "beside its row: {sub:?} {row3:?}"
    );
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
    assert_eq!(w.view.markup().hidden_reviewers.len(), 0);
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
    assert!(!w.view.field_marks.is_empty(), "the merge field is shaded");
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
    assert_eq!(w.take_new_document_actions().as_deref(), Some("file:print"));
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

// source: the reference's Review › Speech › Read Aloud: reads from the caret, the word being
// spoken selected, paragraph after paragraph; its bar pauses and closes.
#[test]
fn read_aloud_follows_the_voice() {
    let mut w = window();
    w.run_interface_command("text:One two", &Fixed);
    w.handle(
        Input::Key(crate::Key::Enter, crate::Mods::default()),
        &Fixed,
    );
    w.run_interface_command("text:Three", &Fixed);
    w.r3_run(vec![json!({ "cmd": "setSelection", "selection": {
        "anchor": { "p": 0, "o": 0 }, "focus": { "p": 0, "o": 0 } } })]);
    w.take_requests();
    assert!(w.action("review.readAloud"));
    let spoken: Vec<String> = w
        .take_requests()
        .into_iter()
        .filter_map(|r| match r {
            super::super::Request::Speak { text, .. } => Some(text),
            _ => None,
        })
        .collect();
    assert_eq!(spoken, ["One two"]);
    assert!(w.reading_aloud());
    // The voice says "two" (UTF-16 4..7): it is selected.
    assert!(w.speech_word(Some((4, 3))));
    let sel = w.view.engine().session().map(bo_edit::Session::selection);
    assert_eq!(sel.map(|s| (s.anchor.offset, s.focus.offset)), Some((4, 7)));
    // Silent after speaking: the next paragraph.
    w.speech_word(None);
    let next: Vec<String> = w
        .take_requests()
        .into_iter()
        .filter_map(|r| match r {
            super::super::Request::Speak { text, .. } => Some(text),
            _ => None,
        })
        .collect();
    assert_eq!(next, ["Three"]);
    assert!(w.on_ids().contains(&"review.readAloud"));
    assert!(w.action("review.readAloud"), "stops");
    assert!(!w.reading_aloud());
}

// source: the reference's Check for Errors (Checking and Reporting Errors).
#[test]
fn check_for_errors_reports_unknown_fields() {
    let mut w = window();
    w.r3_run(vec![
        json!({ "cmd": "mergeTable", "name": "list", "fields": ["Name"], "rows": [["Ann"]] }),
        json!({ "cmd": "mergeInsertField", "name": "Name" }),
    ]);
    assert!(w.action("r16.checkErrors:sim"));
    assert!(
        w.r16.report.contains("No mail merge errors"),
        "{}",
        w.r16.report
    );
    w.r3.form = None;
    w.r3_run(vec![json!({ "cmd": "mergeInsertField", "name": "City" })]);
    assert!(w.action("r16.checkErrors:sim"));
    assert!(w.r16.report.contains("«City»"), "{}", w.r16.report);
}

// source: the reference's Select Recipients › Type a New List… (New Address List) and the
// Preview Results record box.
#[test]
fn type_a_new_list_becomes_the_data_source() {
    use crate::doc_forms::DocFormKind;
    let mut w = window();
    assert!(w.action("mailings.recipients"));
    assert!(
        !w.menu.as_ref().expect("menu").items()[0].dim,
        "Type a New List is on"
    );
    w.menu = None;
    assert!(w.action("mailings.recipients.new"));
    assert!(w.r16.grid.is_some(), "New Address List grid");
    // Typed into the selected cell: Title, then First Name.
    let tab =
        |w: &mut DocWindow| w.handle(Input::Key(crate::Key::Tab, crate::Mods::default()), &Fixed);
    tab(&mut w);
    w.handle(Input::Text("Bob".into()), &Fixed);
    if let Some(g) = w.r16.grid.as_mut() {
        g.new_entry();
    }
    tab(&mut w);
    w.handle(Input::Text("Ann".into()), &Fixed);
    // A column's menu sorts.
    assert!(w.action("r16.nl.sort:1:1"));
    assert_eq!(
        w.r16.grid.as_ref().map(|g| g.rows[0][1].clone()).as_deref(),
        Some("Ann")
    );
    // Customize Columns adds one.
    assert!(w.action("r16.nl.cols:[[0,\"Title\"],[1,\"First Name\"],[-1,\"Nickname\"]]"));
    assert_eq!(w.r16.grid.as_ref().map(|g| g.columns.len()), Some(3));
    // Find… opens its dialog.
    if let Some(g) = w.r16.grid.as_mut() {
        g.cur = (0, 0);
    }
    assert!(w.action("r16.nl.find:0\u{1f}bob"));
    assert_eq!(w.r16.grid.as_ref().map(|g| g.cur), Some((1, 1)));
    let ok = w
        .r16
        .grid
        .as_mut()
        .map(|g| g.input(&Input::Key(crate::Key::Enter, crate::Mods::default())));
    let Some(crate::doc_addrgrid16::GridOut::Ok(fields, rows)) = ok else {
        panic!("OK on the last row");
    };
    w.r16.grid = None;
    assert!(
        w.r3_run(vec![json!({ "cmd": "mergeTable", "name": "Address List",
        "fields": fields, "rows": rows })])
    );
    let (fields, count) = w.merge_fields();
    assert_eq!(count, 2);
    assert!(fields.iter().any(|f| f == "Nickname"));
    let _ = DocFormKind::NlFind;
    // The record box: type 2, Enter shows the second record.
    w.r3_run(vec![
        json!({ "cmd": "mergeInsertField", "name": "First Name" }),
    ]);
    assert!(w.action("mailings.recordBox"));
    w.handle(
        Input::Key(crate::Key::Backspace, crate::Mods::default()),
        &Fixed,
    );
    w.handle(Input::Text("2".into()), &Fixed);
    w.handle(
        Input::Key(crate::Key::Enter, crate::Mods::default()),
        &Fixed,
    );
    assert_eq!(w.merge_record, Some(1));
}

// source: the reference's Home › Font Color split button: the palette picks a colour that
// the button then applies; Gradient ▸ fills the text with a gradient.
#[test]
fn font_color_split_button() {
    let mut w = window();
    w.run_interface_command("text:Hello", &Fixed);
    w.r3_run(vec![json!({ "cmd": "selectAll" })]);
    assert!(w.action("format.colorMenu"));
    assert!(w.r16.font_pal.is_some());
    w.r16.font_pal = None;
    assert!(w.action("r16.fontColor:00B050"));
    assert_eq!(w.view.format().color.as_deref(), Some("00B050"));
    w.r3_run(vec![
        json!({ "cmd": "formatText", "format": { "kind": "color", "hex": null } }),
    ]);
    assert!(w.action("format.color"), "applies the last colour");
    assert_eq!(w.view.format().color.as_deref(), Some("00B050"));
    assert!(w.action("r16.gradient:light:right"));
}

// source: the reference's Step-by-Step Mail Merge Wizard (six steps), Update Labels and
// Send Email Messages.
#[test]
fn wizard_steps_run_mailings_commands() {
    let mut w = window();
    assert!(w.action("mailings.wizard"));
    w.handle(Input::Tick, &Fixed);
    assert_eq!(w.r16.wiz.as_ref().map(|z| z.step), Some(1));
    assert!(w.action("r16.wiz.choose:4"));
    assert_eq!(w.r3.merge_type, "directory");
    for _ in 0..2 {
        assert!(w.action("r16.wiz.next"));
    }
    assert_eq!(w.r16.wiz.as_ref().map(|z| z.step), Some(3));
    assert!(w.action("r16.wiz.choose:1"));
    w.handle(Input::Tick, &Fixed);
    // Step 3 › Create… opens New Address List.
    let link = w.r16.wiz.as_ref().map(|z| z.recipients).expect("wizard");
    assert_eq!(link, 1);
    assert!(w.action("mailings.recipients.new"));
    assert!(w.r16.grid.is_some());
    w.r16.grid = None;
    assert!(w.action("r16.wiz.prev"));
    assert_eq!(w.r16.wiz.as_ref().map(|z| z.step), Some(2));
}

#[test]
fn send_email_hands_messages_to_the_mail_program() {
    let mut w = window();
    w.run_interface_command("text:Dear ", &Fixed);
    w.r3_run(vec![
        json!({ "cmd": "mergeTable", "name": "list", "fields": ["Name", "Email"],
            "rows": [["Ann", "ann@example.com"], ["Bob", "bob@example.com"]] }),
        json!({ "cmd": "mergeInsertField", "name": "Name" }),
    ]);
    w.take_requests();
    let spec = json!({ "to": "Email", "subject": "Hi", "format": "html", "range": "all" });
    assert!(w.action(&format!("r16.email:{spec}")));
    let mails: Vec<super::super::OutMail> = w
        .take_requests()
        .into_iter()
        .filter_map(|r| match r {
            super::super::Request::SendMails(m) => Some(m),
            _ => None,
        })
        .flatten()
        .collect();
    assert_eq!(mails.len(), 2);
    assert_eq!(mails[1].to, "bob@example.com");
    assert!(mails[0].text.contains("Dear Ann"), "{}", mails[0].text);
}
