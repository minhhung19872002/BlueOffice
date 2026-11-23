//! BlueDoc parity round 17: Insert / View commands that were placeholders.
use super::super::{DocWindow, Request};
use crate::doc_forms::DocFormKind;
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

fn select(w: &mut DocWindow, a: usize, b: usize) {
    w.r3_run(vec![json!({ "cmd": "setSelection", "selection": {
        "anchor": { "p": 0, "o": a }, "focus": { "p": 0, "o": b } } })]);
}

fn click(w: &mut DocWindow, id: &str) {
    let out = w.r3.form.as_mut().expect("form").click_for_test(id);
    w.form_out(out);
}

#[test]
fn bookmark_dialog_adds_goes_to_and_deletes() {
    let mut w = window();
    w.run_interface_command("text:Alpha beta gamma", &Fixed);
    select(&mut w, 6, 10);
    assert!(w.action("insert.bookmarkDialog"));
    let f = w.r3.form.as_mut().expect("bookmark");
    assert_eq!(f.kind, DocFormKind::Bookmark);
    f.set_text("name", "Beta");
    click(&mut w, "add");
    assert!(w.r3.form.is_none(), "Add closes the dialog");
    let marks = w.view.engine().doc_bookmarks();
    assert_eq!(marks.len(), 1);
    assert_eq!((marks[0].start.offset, marks[0].end.offset), (6, 10));
    // The dialog opens on the bookmark at the selection; Go To selects it again.
    select(&mut w, 0, 0);
    assert!(w.action("insert.bookmarkDialog"));
    assert_eq!(w.r3.form.as_ref().expect("form").text("name"), "Beta");
    click(&mut w, "goTo");
    let sel = w.view.engine().session().expect("s").selection();
    assert_eq!((sel.anchor.offset, sel.focus.offset), (6, 10));
    click(&mut w, "delete");
    assert_eq!(w.view.engine().doc_bookmarks(), Vec::new());
}

#[test]
fn excel_spreadsheet_and_object_dialog_embed_a_workbook() {
    let mut w = window();
    assert!(w.action("insert.table.excel"));
    let v = w.view.run_json(&json!({ "cmd": "oleInfo" })).expect("info");
    assert_eq!(v["ole"]["workbook"], true, "{v}");
    assert!(w.action("insert.object.object"));
    assert_eq!(
        w.r3.form.as_ref().map(|f| f.kind),
        Some(DocFormKind::InsertObject)
    );
    click(&mut w, "browse");
    assert!(w
        .take_requests()
        .iter()
        .any(|r| matches!(r, Request::PickFile { purpose, .. } if purpose == "object")));
}

#[test]
fn text_from_file_inserts_the_other_document() {
    let mut other = Engine::with_bundled_fonts();
    other.new_document("b.docx").expect("new");
    other
        .command_json(r#"{"cmd":"insertText","text":"Imported words"}"#)
        .expect("text");
    let bytes = other.save_docx().expect("save");
    let mut w = window();
    assert!(!w.action("insert.object.textFromFile"));
    assert!(w
        .take_requests()
        .iter()
        .any(|r| matches!(r, Request::PickFile { purpose, .. } if purpose == "textFromFile")));
    assert!(w.file_picked("textFromFile", "C:\\x\\b.docx", &bytes));
    let text = w
        .view
        .engine()
        .session()
        .map(|s| {
            (0..4)
                .filter_map(|i| bo_model::index::para_by_id(s.doc(), i))
                .map(bo_model::index::para_index_text)
                .collect::<String>()
        })
        .unwrap_or_default();
    assert!(text.contains("Imported words"), "{text}");
}

#[test]
fn online_video_inserts_a_linked_frame() {
    let mut w = window();
    assert!(w.action("insert.onlineVideo"));
    let f = w.r3.form.as_mut().expect("video");
    f.set_text("url", "https://example.org/v/1");
    let out = f.r17_ok();
    w.form_out(out);
    assert!(w.r3.form.is_none());
    let doc = w.view.engine().document().expect("doc");
    assert_eq!(doc.media.len(), 1);
}

#[test]
fn macros_dialog_lists_nothing_without_a_project_and_never_runs() {
    let mut w = window();
    assert!(w.action("view.macros.view"));
    let f = w.r3.form.as_ref().expect("macros");
    assert_eq!(f.kind, DocFormKind::Macros);
    assert_eq!(f.items_for_test("macros"), Vec::<String>::new());
}

#[test]
fn switch_windows_lists_the_hosts_windows() {
    let mut w = window();
    assert!(!w.action("view.switchWindows"));
    assert!(w.take_requests().contains(&Request::ListWindows));
    assert!(w.show_windows(vec![
        (1, "a.docx".into(), true),
        (2, "b.docx".into(), false)
    ]));
    assert!(w.menu.is_some());
    assert!(!w.action("r17.window:2"));
    assert!(w.take_requests().contains(&Request::ActivateWindow(2)));
}

#[test]
fn picture_watermark_from_a_picked_file() {
    let mut w = window();
    assert!(w.action("design.watermarkCustom"));
    let png = super::video_frame().expect("png");
    assert!(w.file_picked("watermark", "C:\\pics\\logo.png", &png));
    let f = w.r3.form.as_mut().expect("watermark");
    assert_eq!(f.chosen("wm"), Some("wmPicture"));
    let out = f.input(&Input::Key(crate::Key::Enter, crate::Mods::default()));
    w.form_out(out);
    let wm = w
        .view
        .engine()
        .session()
        .and_then(bo_edit::Session::watermark)
        .expect("watermark");
    assert!(wm.picture);
    assert!(wm.washout);
}

#[test]
fn selection_saved_to_the_watermark_gallery() {
    let mut w = window();
    w.run_interface_command("text:PRIVATE copy", &Fixed);
    select(&mut w, 0, 7);
    assert!(w.r17_save_watermark());
    assert!(w
        .take_requests()
        .iter()
        .any(|r| matches!(r, Request::SaveWatermarks(j) if j.contains("PRIVATE"))));
    let (name, entries) = w.r17_wm_set().expect("General");
    assert_eq!(name, "General");
    assert_eq!(entries[0].text, "PRIVATE");
    let mut w2 = window();
    w2.load_watermarks(r#"["MINE"]"#);
    assert_eq!(w2.r17_wm_set().expect("loaded").1[0].text, "MINE");
}

#[test]
fn colors_custom_field_follows_a_drag() {
    let mut w = window();
    assert!(w.r3_open_form(DocFormKind::Colors));
    w.r3.form.as_mut().expect("colors").set_tab(1);
    w.handle(Input::Tick, &Fixed);
    let f = w.r3.form.as_mut().expect("colors");
    let (x0, y0) = f.point_of("spectrum").expect("field");
    let press = Input::Press {
        x: x0 + 2.0,
        y: y0 + 2.0,
        count: 1,
        shift: false,
    };
    f.input(&press);
    let first = f.text("hex");
    f.input(&Input::Drag {
        x: x0 + 120.0,
        y: y0 + 60.0,
    });
    assert_ne!(f.text("hex"), first, "the colour follows the drag");
    f.input(&Input::Release {
        x: x0 + 120.0,
        y: y0 + 60.0,
    });
}

#[test]
fn immersive_reader_tab_column_color_spacing_and_close() {
    let mut w = window();
    w.run_interface_command("text:Read this slowly", &Fixed);
    assert!(w.action("view.immersiveReader"));
    w.handle(Input::Tick, &Fixed);
    assert_eq!(w.ribbon.active_label(), "doc17.tabImmersive");
    let v = w.view.engine().view_layout();
    assert_eq!(v.mode, bo_engine::views::ViewMode::Immersive);
    assert!(v.immersive.spacing);
    assert!((v.immersive.column - 408.0).abs() < 0.1);
    assert!(w.r17_on_ids().contains(&"ir.textSpacing"));
    assert!(w.action("ir.column.0"));
    assert!(w.action("ir.color.1"));
    let v = w.view.engine().view_layout();
    assert!((v.immersive.column - 204.0).abs() < 0.1);
    assert_eq!(v.immersive.color, 1);
    assert!(w.action("ir.textSpacing"));
    assert!(!w.view.engine().view_layout().immersive.spacing);
    assert!(w.action("ir.focus.3"));
    w.run_interface_command("text: understanding information", &Fixed);
    assert!(w.action("ir.syllables"));
    w.handle(Input::Tick, &Fixed);
    assert!(w.r17_on_ids().contains(&"ir.syllables"));
    assert!(
        w.r17.syl.as_ref().is_some_and(|(_, s)| !s.is_empty()),
        "syllable breaks"
    );
    assert!(w.action("ir.close"));
    w.handle(Input::Tick, &Fixed);
    assert_eq!(
        w.view.engine().view_layout().mode,
        bo_engine::views::ViewMode::Print
    );
    assert!(!w.ribbon.contextual_labels().contains(&"doc17.tabImmersive"));
}

#[test]
fn style_format_numbering_gives_the_new_style_a_list() {
    let mut w = window();
    assert!(w.action("styles.newStyle"));
    assert!(w.action("r13.styleFmt.numbering"));
    assert_eq!(
        w.r3.form.as_ref().map(|f| f.kind),
        Some(DocFormKind::StyleNumbering)
    );
    let f = w.r3.form.as_mut().expect("numbering");
    f.set_sel("formats", Some(1));
    let out = f.r17_ok();
    w.form_out(out);
    let f = w.r3.form.as_ref().expect("style dialog back");
    assert_eq!(f.kind, DocFormKind::NewStyle);
    let spec = f.style_spec();
    assert!(spec["numId"].as_i64().unwrap_or(0) > 0, "{spec}");
}

#[test]
fn style_shortcut_key_assigned_and_applied() {
    let mut w = window();
    w.run_interface_command("text:Make me a heading", &Fixed);
    assert!(w.action("styles.modify:Heading1"));
    assert!(w.action("r13.styleFmt.shortcut"));
    assert_eq!(
        w.r3.form.as_ref().map(|f| f.kind),
        Some(DocFormKind::CustomizeKeyboard)
    );
    let f = w.r3.form.as_mut().expect("keyboard");
    let m = crate::Mods {
        ctrl: true,
        alt: true,
        shift: false,
    };
    f.input(&Input::Key(crate::Key::Letter('1'), m));
    assert_eq!(f.text("newKey"), "Ctrl+Alt+1");
    let out = f.r17_ok();
    w.form_out(out);
    assert!(w
        .take_requests()
        .iter()
        .any(|r| matches!(r, Request::SaveKeys(j) if j.contains("Ctrl+Alt+1"))));
    w.r3.form = None;
    w.r3.stash = None;
    w.handle(Input::Key(crate::Key::Letter('1'), m), &Fixed);
    assert_eq!(w.view.format().style.as_deref(), Some("Heading1"));
}

#[test]
fn style_text_effects_go_into_the_style() {
    let mut w = window();
    assert!(w.action("styles.newStyle"));
    assert!(w.action("r13.styleFmt.effects"));
    let f = w.r3.form.as_mut().expect("effects");
    assert_eq!(f.kind, DocFormKind::StyleEffects);
    f.set_on("fxGlow", true);
    let out = f.r17_ok();
    w.form_out(out);
    let spec = w.r3.form.as_ref().expect("style").style_spec();
    assert!(spec["advanced"]["glow"].is_object(), "{spec}");
    assert!(spec["advanced"]["textOutline"].is_null());
}

#[test]
fn style_frame_goes_into_the_style() {
    let mut w = window();
    assert!(w.action("styles.newStyle"));
    assert!(w.action("r13.styleFmt.frame"));
    let f = w.r3.form.as_mut().expect("frame");
    assert_eq!(f.kind, DocFormKind::StyleFrame);
    f.set_sel("wRule", Some(1));
    f.set_text("wAt", "2\"");
    let out = f.r17_ok();
    w.form_out(out);
    let spec = w.r3.form.as_ref().expect("style").style_spec();
    let x = spec["frameXml"].as_str().unwrap_or_default();
    assert!(
        x.contains("w:w=\"2880\"") && x.contains("w:wrap=\"around\""),
        "{x}"
    );
}

#[test]
fn draw_table_draws_a_table_then_splits_and_erases() {
    let mut w = window();
    assert!(w.action("insert.table.draw"));
    assert!(w.r17_on_ids().contains(&"table.drawTable"));
    let stroke = |w: &mut DocWindow, a: (f32, f32), b: (f32, f32), shift: bool| {
        w.handle(
            Input::Press {
                x: a.0,
                y: a.1,
                count: 1,
                shift,
            },
            &Fixed,
        );
        w.handle(Input::Drag { x: b.0, y: b.1 }, &Fixed);
        w.handle(Input::Release { x: b.0, y: b.1 }, &Fixed);
    };
    let caret = w.view.caret_rect().expect("caret");
    let (x0, y0) = (caret[0] + 4.0, caret[1] + 4.0);
    stroke(&mut w, (x0, y0), (x0 + 300.0, y0 + 80.0), false);
    let cols = |w: &DocWindow| w.view.format().table.as_ref().map(|t| t.cols);
    assert_eq!(cols(&w), Some(1), "a drawn rectangle is a table");
    let width = w
        .view
        .engine()
        .document()
        .and_then(|d| {
            d.sections[0].blocks.iter().find_map(|b| match b {
                bo_model::Block::Table(t) => t.grid.first().copied(),
                _ => None,
            })
        })
        .unwrap_or_default();
    let want = (300.0 / w.view.scale() * 20.0).round() as i32;
    assert!((width.0 - want).abs() <= 20, "{width:?} vs {want}");
    // Round 18: the row is as tall as the stroke.
    let height = w
        .view
        .engine()
        .document()
        .and_then(|d| {
            d.sections[0].blocks.iter().find_map(|b| match b {
                bo_model::Block::Table(t) => t.rows.first().and_then(|r| r.props.height),
                _ => None,
            })
        })
        .map(|h| h.0 .0);
    let want_h = (80.0 / w.view.scale() * 20.0).round() as i32;
    assert!(
        height.is_some_and(|h| (h - want_h).abs() <= 20),
        "{height:?} vs {want_h}"
    );
    // The pencil pointer over the page while drawing.
    w.handle(Input::Move { x: x0, y: y0 }, &Fixed);
    assert_eq!(w.cursor(), super::super::r10::pointer::PEN);
    let cell = w.view.caret_rect().expect("caret in the cell");
    stroke(
        &mut w,
        (cell[0] + 40.0, cell[1] + 2.0),
        (cell[0] + 40.0, cell[1] + 60.0),
        false,
    );
    assert_eq!(cols(&w), Some(2), "a vertical line splits the cell");
    assert!(w.action("table.eraser"));
    assert!(w.r17_on_ids().contains(&"table.eraser"));
    assert!(w.handle(
        Input::Key(crate::Key::Escape, crate::Mods::default()),
        &Fixed
    ));
    assert!(!w.r17_on_ids().contains(&"table.eraser"));
}
