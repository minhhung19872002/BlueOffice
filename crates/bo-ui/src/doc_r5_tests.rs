//! BlueDoc parity round 5: disabled icons, cover pages, Mailings rules, Match Fields, Find
//! Recipient.
use super::super::DocWindow;
use crate::doc_forms::DocFormKind;
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

pub(super) fn window() -> DocWindow {
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

/// A window whose document has recipients (FirstName, LastName, City).
fn merge_window() -> DocWindow {
    let mut w = window();
    w.view.run_json(&json!({
        "cmd": "mergeTable", "name": "people",
        "fields": ["FirstName", "LastName", "City"],
        "rows": [["An", "Nguyen", "Hanoi"], ["Binh", "Tran", "Hue"], ["Chi", "Le", "Danang"]],
    }));
    w
}

fn body_text(w: &DocWindow) -> String {
    let doc = w.view.engine().document().expect("doc");
    bo_model::index::body_paragraphs(doc)
        .iter()
        .filter_map(|p| bo_model::index::para_at(doc, p))
        .map(bo_model::Paragraph::plain_text)
        .collect::<Vec<_>>()
        .join("\n")
}

fn type_into(w: &mut DocWindow, text: &str) {
    w.handle(Input::Text(text.to_owned()), &Fixed);
}

fn press_key(w: &mut DocWindow, k: Key) {
    w.handle(Input::Key(k, Mods::default()), &Fixed);
}

#[test]
fn the_rules_menu_lists_every_rule_in_the_reference_order() {
    let ids: Vec<&str> = crate::doc_r3_menus::menu("mailings.rules")
        .iter()
        .map(|e| e.id)
        .collect();
    assert_eq!(
        ids,
        [
            "mailings.rule.ask",
            "mailings.rule.fillIn",
            "mailings.rule.if",
            "mailings.rule.mergeRec",
            "mailings.rule.mergeSeq",
            "mailings.rule.next",
            "mailings.rule.nextIf",
            "mailings.rule.set",
            "mailings.rule.skipIf",
        ]
    );
}

#[test]
fn rule_dialogs_insert_their_fields() {
    let mut w = merge_window();
    // Ask: OK waits for a bookmark name.
    assert!(w.action("mailings.rule.ask"));
    assert_eq!(
        w.r3.form.as_ref().map(|f| f.kind),
        Some(DocFormKind::AskRule)
    );
    let f = w.r3.form.as_mut().expect("form");
    f.set_text("bookmark", "Greeting");
    f.set_text("prompt", "How to greet?");
    f.set_text("default", "Hello");
    press_key(&mut w, Key::Enter);
    assert!(w.r3.form.is_none(), "closed by OK");
    let instr = fields(&w);
    assert!(
        instr.iter().any(|i| i.starts_with("ASK Greeting")),
        "{instr:?}"
    );
    // Skip Record If: City Equal to Hue.
    assert!(w.action("mailings.rule.skipIf"));
    let f = w.r3.form.as_mut().expect("form");
    f.set_sel("field", Some(2));
    f.set_text("value", "Hue");
    press_key(&mut w, Key::Enter);
    let instr = fields(&w);
    assert!(instr.iter().any(|i| i.contains("SKIPIF")), "{instr:?}");
    // If…Then…Else with Is blank: no value compared.
    assert!(w.action("mailings.rule.if"));
    let f = w.r3.form.as_mut().expect("form");
    f.set_sel("cmp", Some(6));
    f.set_text("then", "none");
    f.set_text("else", "some");
    press_key(&mut w, Key::Enter);
    assert!(fields(&w).iter().any(|i| i.starts_with("IF")));
    // Set Bookmark and Next Record If open their own dialogs.
    assert!(w.action("mailings.rule.set"));
    assert_eq!(
        w.r3.form.as_ref().map(|f| f.kind),
        Some(DocFormKind::SetRule)
    );
    press_key(&mut w, Key::Escape);
    assert!(w.action("mailings.rule.nextIf"));
    assert_eq!(
        w.r3.form.as_ref().map(|f| f.kind),
        Some(DocFormKind::NextIfRule)
    );
}

/// Field instructions of the document (simple and complex fields).
fn fields(w: &DocWindow) -> Vec<String> {
    let doc = w.view.engine().document().expect("doc");
    let mut out = Vec::new();
    let mut complex = String::new();
    let mut open = 0;
    for path in bo_model::index::body_paragraphs(doc) {
        let Some(p) = bo_model::index::para_at(doc, &path) else {
            continue;
        };
        for r in &p.runs {
            for c in &r.content {
                match c {
                    bo_model::RunContent::Field { instr, .. } => out.push(instr.trim().to_owned()),
                    bo_model::RunContent::Opaque(raw) => {
                        let x: &str = &raw.xml;
                        if x.contains("fldCharType=\"begin\"") {
                            open += 1;
                        } else if x.contains("fldCharType=\"end\"") {
                            open -= 1;
                            if open == 0 {
                                out.push(complex.trim().to_owned());
                                complex.clear();
                            }
                        } else if x.contains("instrText") {
                            let s = x.split('>').nth(1).unwrap_or("");
                            complex.push_str(s.split('<').next().unwrap_or(""));
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    out
}

#[test]
fn match_fields_guesses_columns_and_drives_the_address_block() {
    let mut w = merge_window();
    assert!(w.action("mailings.matchFields"));
    let f = w.r3.form.as_mut().expect("form");
    // First Name (row 2) and Last Name (row 4) are matched by their letters; City too.
    assert_eq!(f.text("m2"), "FirstName");
    assert_eq!(f.text("m4"), "LastName");
    assert_eq!(f.text("m11"), "City");
    assert_eq!(f.sel("m0"), Some(0), "(not matched)");
    press_key(&mut w, Key::Enter);
    assert_eq!(
        w.r5.match_fields.get("City").map(String::as_str),
        Some("City")
    );
    assert!(w.action("mailings.addressBlock"));
    let instr = fields(&w);
    assert!(instr.iter().any(|i| i.contains("FirstName")), "{instr:?}");
    assert!(instr.iter().any(|i| i.contains("City")), "{instr:?}");
}

#[test]
fn find_recipient_previews_the_matching_record() {
    let mut w = merge_window();
    assert!(w.action("mailings.findRecipient"));
    {
        let f = w.r3.form.as_mut().expect("form");
        f.set_sel("field", Some(2));
        f.set_text("what", "dan");
    }
    press_key(&mut w, Key::Enter);
    assert_eq!(w.merge_record, Some(2), "Danang is the third record");
    assert!(w.r3.form.is_some(), "Find in Field stays open");
    let _ = body_text(&w);
}

#[test]
fn a_cover_page_can_be_removed_from_the_gallery() {
    let mut w = window();
    assert!(!w.has_cover_page());
    assert!(w.action("insert.coverPage.horizon"));
    assert!(w.has_cover_page());
    assert!(w.action("insert.coverPage.remove"));
    assert!(!w.has_cover_page());
}

fn para_texts(w: &DocWindow) -> Vec<String> {
    let s = w.view.engine().session().expect("session");
    (0..s.paragraph_count())
        .map(|i| s.para_text(i).unwrap_or_default())
        .collect()
}

#[test]
fn typing_in_outline_view_edits_items_and_enter_adds_one_at_the_same_level() {
    let mut w = window();
    w.view
        .run_json(&json!({ "cmd": "insertText", "text": "Intro" }));
    w.view
        .run_json(&json!({ "cmd": "setOutlineLevel", "para": 0, "level": 0 }));
    assert!(w.action("view.outline"));
    w.handle(Input::Tick, &Fixed);
    assert_eq!(
        w.r3.outline
            .as_ref()
            .and_then(crate::doc_outline::OutlineView::caret),
        Some((0, 5))
    );
    press_key(&mut w, Key::Enter);
    type_into(&mut w, "Next");
    assert_eq!(para_texts(&w), vec!["Intro", "Next"]);
    // The new item is a heading of the same level (its own outline row).
    let items = w.view.outline();
    assert_eq!(items.len(), 2);
    assert_eq!(items[1].entry.level, Some(0));
    assert_eq!(
        w.r3.outline
            .as_ref()
            .and_then(crate::doc_outline::OutlineView::caret),
        Some((1, 4))
    );
    // Backspace and the arrows edit inside the item.
    press_key(&mut w, Key::Backspace);
    press_key(&mut w, Key::Home);
    type_into(&mut w, "A ");
    assert_eq!(para_texts(&w), vec!["Intro", "A Nex"]);
    // Tab demotes the item.
    press_key(&mut w, Key::Tab);
    assert_eq!(w.view.outline()[1].entry.level, Some(1));
}

#[test]
fn split_shows_two_panes_that_swap_on_a_click_and_a_double_click_on_the_bar_removes_it() {
    let mut w = window();
    for _ in 0..40 {
        w.view
            .run_json(&json!({ "cmd": "insertText", "text": "Line of text" }));
        w.view.run_json(&json!({ "cmd": "splitParagraph" }));
    }
    let (.., whole) = w.view.scroll_info();
    assert!(w.action("view.split"));
    w.handle(Input::Tick, &Fixed);
    assert_eq!(w.strings.get("commands.view.split"), "Remove Split");
    let s = w.r5.split.expect("split");
    let (.., pane) = w.view.scroll_info();
    assert!(
        pane < whole * 0.6,
        "the main view takes the bottom half: {pane} of {whole}"
    );
    // The lower pane continues where the upper one ends, as the window was split.
    let (main, ..) = w.view.scroll_info();
    let top = w.view.secondary_scroll().expect("second pane");
    assert!(
        (main - top - (s.bar[1] + s.bar[3] - s.area[1])).abs() < 1.5,
        "{main} {top}"
    );
    // The bar can be dragged up.
    let bar = s.bar;
    let (bx, by) = (bar[0] + 40.0, bar[1] + bar[3] / 2.0);
    w.handle(
        Input::Press {
            x: bx,
            y: by,
            count: 1,
            shift: false,
        },
        &Fixed,
    );
    w.handle(
        Input::Drag {
            x: bx,
            y: by - 100.0,
        },
        &Fixed,
    );
    w.handle(
        Input::Release {
            x: bx,
            y: by - 100.0,
        },
        &Fixed,
    );
    let s2 = w.r5.split.expect("split");
    assert!(s2.bar[1] < bar[1] - 50.0, "{:?} → {:?}", bar, s2.bar);
    // A click in the top pane makes it the main view.
    w.handle(
        Input::Press {
            x: bx + 300.0,
            y: s2.area[1] + 30.0,
            count: 1,
            shift: false,
        },
        &Fixed,
    );
    w.handle(
        Input::Release {
            x: bx + 300.0,
            y: s2.area[1] + 30.0,
        },
        &Fixed,
    );
    assert!(w.r5.split.expect("split").active_top);
    // Double click on the bar: one pane again.
    let bar = w.r5.split.expect("split").bar;
    w.handle(
        Input::Press {
            x: bx,
            y: bar[1] + 1.0,
            count: 2,
            shift: false,
        },
        &Fixed,
    );
    assert!(w.r5.split.is_none());
    assert!(w.view.secondary_scroll().is_none());
    assert_eq!(w.strings.get("commands.view.split"), "Split");
}

#[test]
fn arrange_all_asks_the_host() {
    let mut w = window();
    w.action("view.arrangeAll");
    assert!(w.take_requests().contains(&super::Request::ArrangeAll));
}

#[test]
fn reveal_formatting_and_style_inspector_show_the_caret_formatting() {
    let mut w = window();
    w.view
        .run_json(&json!({ "cmd": "insertText", "text": "Some text" }));
    // Shift+F1: Reveal Formatting.
    w.handle(
        Input::Key(
            Key::F(1),
            Mods {
                shift: true,
                ..Mods::default()
            },
        ),
        &Fixed,
    );
    let (inspector, pane) = w.r5.info.as_ref().expect("pane");
    assert!(!inspector);
    assert!(pane.sections.iter().any(|s| s.header == "Font"));
    assert!(pane.rect[2] > 0.0, "laid out");
    // The document area makes room for it.
    assert!(w.doc_right() <= w.size.0 - pane.rect[2] + 0.5);
    // Style Inspector from the Styles pane button.
    assert!(w.action("styles.inspector"));
    let (inspector, pane) = w.r5.info.as_ref().expect("pane");
    assert!(*inspector);
    assert_eq!(pane.sections.len(), 4);
    assert_eq!(pane.sections[0].lines, vec!["Normal".to_owned()]);
    // Bold text shows as direct character formatting, cleared by its button.
    w.view.select_range(0, 0, 4);
    assert!(w.action("format.bold"));
    w.view.select_range(0, 2, 2);
    w.handle(Input::Tick, &Fixed);
    let pane = &w.r5.info.as_ref().expect("pane").1;
    assert!(
        pane.sections[3].lines.iter().any(|l| l.contains("Bold")),
        "{:?}",
        pane.sections[3].lines
    );
    assert!(w.action("styles.inspector"), "toggles off");
    assert!(w.r5.info.is_none());
}

#[test]
fn manage_styles_lists_sorts_and_sets_the_defaults() {
    let mut w = window();
    assert!(w.action("styles.manage"));
    let f = w.r3.form.as_mut().expect("form");
    assert_eq!(f.kind, DocFormKind::ManageStyles);
    assert!(f.ids.iter().any(|i| i == "Normal"));
    // Latent built-in styles are listed (Hide Until Used), names as the interface shows them.
    let items = f.items_for_test("list");
    assert!(
        items
            .iter()
            .any(|i| i.contains("Heading 6") && i.contains("(Hide Until Used)")),
        "{items:?}"
    );
    assert!(items.iter().any(|i| i.contains("Default Paragraph Font")));
    assert!(!items.iter().any(|i| i.contains("heading 1")));
    // The selected style's description.
    // (round 13: drawn wrapped in the bordered box `msDesc`).
    let desc = f
        .words
        .iter()
        .find(|w| w.0 == "msDesc")
        .map(|w| w.1.clone())
        .unwrap_or_default();
    assert!(desc.starts_with("Font:"), "{desc}");
    // Alphabetical order.
    f.set_sel("sort", Some(1));
    f.click_for_test("none");
    // Set Defaults (round 13: the document defaults, which Normal inherits): Arial 14 pt.
    f.set_tab(3);
    f.set_text("font", "Arial");
    f.set_text("size", "14");
    press_key(&mut w, Key::Enter);
    assert!(w.r3.form.is_none());
    let normal = w
        .view
        .all_styles()
        .into_iter()
        .find(|s| s.id == "Normal")
        .expect("Normal");
    assert_eq!(normal.preview.font, "Arial");
    assert!(
        (normal.preview.size - 14.0).abs() < 0.01,
        "{}",
        normal.preview.size
    );
}

#[test]
fn bring_forward_and_send_backward_name_their_wrap_rows_as_the_reference() {
    let mut w = window();
    assert!(w.run_interface_command("tab:layout", &Fixed));
    for (menu, want) in [
        ("layout.bringForward", "Bring in Front of Text"),
        ("layout.sendBackward", "Send Behind Text"),
    ] {
        w.r4_action(menu);
        let labels: Vec<String> = w
            .menu
            .as_ref()
            .map(|m| m.items().iter().map(|i| i.label.clone()).collect())
            .unwrap_or_default();
        assert!(labels.iter().any(|l| l == want), "{menu}: {labels:?}");
        w.menu = None;
    }
}

#[test]
fn a_disabled_coloured_icon_keeps_a_grey_drawing() {
    use crate::icons::{IconColors, Icons};
    struct Fills(Vec<bo_render::Rgba>);
    impl crate::Painter for Fills {
        fn clear(&mut self, _: bo_render::Rgba) {}
        fn fill_rect(&mut self, _: f32, _: f32, _: f32, _: f32, _: bo_render::Rgba) {}
        fn page(&mut self, _: usize, _: i32, _: i32) {}
        fn text(&mut self, _: &str, _: f32, _: f32, _: crate::TextStyle, _: bo_render::Rgba) {}
        fn path(
            &mut self,
            _: &[bo_render::PathSeg],
            fill: Option<bo_render::Rgba>,
            stroke: Option<(bo_render::Rgba, f32)>,
        ) {
            if let Some(c) = fill.or(stroke.map(|s| s.0)) {
                self.0.push(c);
            }
        }
        fn push_clip(&mut self, _: f32, _: f32, _: f32, _: f32) {}
        fn pop_clip(&mut self) {}
    }
    let mut icons = Icons::load();
    icons.add_solid(
        "test.coloured",
        &[(
            "M3 3h18v18h-18Z",
            Some(bo_render::Rgba(0x2b, 0x6a, 0xb8, 255)),
        )],
    );
    let colors = IconColors {
        ink: bo_render::Rgba(0xc8, 0xc8, 0xc8, 255),
        accent: bo_render::Rgba(0, 0, 255, 255),
        on: bo_render::Rgba(255, 255, 255, 255),
        tinted: false,
    };
    // Other apps: disabled coloured parts are left out.
    let mut p = Fills(Vec::new());
    icons.draw(&mut p, "test.coloured", 0.0, 0.0, 20.0, colors);
    assert_eq!(p.0, Vec::new());
    // BlueDoc: drawn in a light neutral grey.
    icons.set_grey_disabled(true);
    let mut p = Fills(Vec::new());
    icons.draw(&mut p, "test.coloured", 0.0, 0.0, 20.0, colors);
    assert_eq!(p.0.len(), 1);
    let c = p.0[0];
    assert!(c.0 == c.1 && c.1 == c.2, "neutral grey: {c:?}");
    assert!((150..=235).contains(&c.0), "{c:?}");
}

#[test]
fn mailings_commands_dim_by_merge_state_as_the_reference() {
    let mut w = window();
    assert!(w.run_interface_command("tab:mailings", &Fixed));
    w.handle(Input::Tick, &Fixed);
    // No data source: only Envelopes, Labels, Start Mail Merge, Select Recipients lit.
    for id in [
        "mailings.editRecipients",
        "mailings.addressBlock",
        "mailings.insertField",
        "mailings.rules",
        "mailings.matchFields",
        "mailings.preview",
        "mailings.firstRecord",
        "mailings.checkErrors",
        "mailings.finish",
    ] {
        assert!(w.ribbon.extra_dimmed(id), "{id} dim without data");
    }
    for id in [
        "mailings.envelopes",
        "mailings.labels",
        "mailings.startMerge",
        "mailings.recipients",
    ] {
        assert!(!w.ribbon.extra_dimmed(id), "{id} lit");
    }
    assert!(!w.action("mailings.rules"), "a dimmed command does nothing");
    // Select Recipients is a menu button (its label carries the mark).
    assert!(crate::catalog::is_menu("mailings.recipients"));
    assert!(w.action("mailings.recipients"));
    assert_eq!(w.menu.as_ref().map(|m| m.items().len()), Some(3));
    w.menu = None;
    assert_eq!(w.r5.record_box, None);
    // Data source, no merge field: Highlight, Preview Results, Find Recipient, Update Labels dim.
    w.view.run_json(&json!({
        "cmd": "mergeTable", "name": "people",
        "fields": ["FirstName"], "rows": [["An"], ["Binh"]],
    }));
    w.handle(Input::Tick, &Fixed);
    for id in [
        "mailings.highlightFields",
        "mailings.preview",
        "mailings.findRecipient",
        "mailings.updateLabels",
    ] {
        assert!(w.ribbon.extra_dimmed(id), "{id} dim without fields");
    }
    for id in [
        "mailings.rules",
        "mailings.addressBlock",
        "mailings.nextRecord",
        "mailings.finish",
    ] {
        assert!(!w.ribbon.extra_dimmed(id), "{id} lit with data");
    }
    assert_eq!(w.r5.record_box, Some(0), "the record box shows 1");
    // A merge field: Preview Results, Highlight and Find Recipient light up.
    w.view
        .run_json(&json!({ "cmd": "mergeInsertField", "name": "FirstName" }));
    w.handle(Input::Tick, &Fixed);
    for id in [
        "mailings.highlightFields",
        "mailings.preview",
        "mailings.findRecipient",
    ] {
        assert!(!w.ribbon.extra_dimmed(id), "{id} lit with a field");
    }
    assert!(
        w.ribbon.extra_dimmed("mailings.updateLabels"),
        "letters: no labels"
    );
}

#[test]
fn modify_from_manage_styles_changes_the_style() {
    let mut w = window();
    assert!(w.action("styles.modify:Normal"));
    let f = w.r3.form.as_mut().expect("form");
    assert_eq!(f.kind, DocFormKind::NewStyle);
    assert_eq!(f.modify.as_deref(), Some("Normal"));
    // Round 13: the reference's Bold toggle of the Formatting row.
    assert!(f.r13_canvas_for_test("sBold"));
    press_key(&mut w, Key::Enter);
    let normal = w
        .view
        .all_styles()
        .into_iter()
        .find(|s| s.id == "Normal")
        .expect("Normal");
    assert!(normal.preview.bold);
    assert_eq!(super::ui_style_name("heading 1"), "Heading 1");
    assert_eq!(super::ui_style_name("toc 2"), "TOC 2");
}
