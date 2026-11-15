//! Headless tests of the Edit tab tools, spelling, bookmarks, command search, the reader
//! tool hand-off and Optimize in BlueEditor.
use crate::dialog::DialogButton;
use crate::editor_actions::Ask2;
use crate::editor_tests::{window, Fixed, Rec};
use crate::editor_tools::{BoxTool, ToolAsk};
use crate::{Input, Key, Mods};
use bo_pdfread::LinkTarget;

fn drag(w: &mut crate::EditorWindow, page: usize, r: [f32; 4]) {
    let q = w
        .view()
        .and_then(|v| v.doc_rect_to_window(page, r))
        .expect("page");
    w.handle(
        Input::Press {
            x: q[0],
            y: q[1],
            count: 1,
            shift: false,
        },
        &Fixed,
    );
    w.handle(
        Input::Drag {
            x: q[0] + q[2],
            y: q[1] + q[3],
        },
        &Fixed,
    );
    w.handle(
        Input::Release {
            x: q[0] + q[2],
            y: q[1] + q[3],
        },
        &Fixed,
    );
}

fn answer(w: &mut crate::EditorWindow, values: &[&str]) {
    let Some((_, what)) = w.dialog2.take() else {
        panic!("no dialog");
    };
    let v: Vec<String> = values.iter().map(|s| (*s).to_owned()).collect();
    let Ask2::Tools(t) = what else {
        panic!("not a tools dialog: {what:?}");
    };
    w.tools_answer(t, DialogButton::Ok, &v);
}

fn links(w: &mut crate::EditorWindow, page: usize) -> Vec<bo_pdfread::Link> {
    let v = w.view_mut().expect("view");
    v.doc_mut()
        .render(page, bo_pdfread::RenderOptions::default())
        .expect("renders")
        .links
}

#[test]
fn link_cross_reference_and_shapes() {
    let mut w = window(3);
    w.run("ed.link", &Fixed);
    assert_eq!(w.tools.tool, Some(BoxTool::Link));
    drag(&mut w, 0, [100.0, 100.0, 200.0, 120.0]);
    answer(&mut w, &["bluestar.vn", "1"]);
    w.run("ed.crossRef", &Fixed);
    drag(&mut w, 0, [100.0, 150.0, 200.0, 170.0]);
    answer(&mut w, &["3"]);
    let l = links(&mut w, 0);
    assert_eq!(l.len(), 2, "{l:?}");
    assert_eq!(l[0].target, LinkTarget::Uri("http://bluestar.vn".into()));
    assert_eq!(
        l[1].target,
        LinkTarget::Page {
            index: 2,
            top: None
        }
    );
    assert!((l[0].rect[0] - 100.0).abs() < 1.0 && (l[0].rect[3] - 120.0).abs() < 1.0);
    // Add Shapes: an ellipse path in the page content.
    w.run("ed.shape.ellipse", &Fixed);
    drag(&mut w, 0, [300.0, 250.0, 400.0, 300.0]);
    let objs = w
        .view_mut()
        .expect("v")
        .doc_mut()
        .page_objects(0)
        .expect("objs");
    assert!(objs.iter().any(|o| o.kind == bo_pdfread::ObjectKind::Path));
    // A reader tool on the ribbon turns the box tool off.
    w.run("ed.link", &Fixed);
    assert!(w.tools.tool.is_some());
    w.run("tool.highlight", &Fixed);
    assert!(w.tools.tool.is_none());
}

#[test]
fn reader_comment_tool_hands_off_editor_tools() {
    let mut w = window(1);
    w.run("cm.rect", &Fixed);
    assert!(w.comment.tool.is_some());
    // Pick the reader's highlight by clicking its ribbon button (Comment tab).
    w.show_tab(4, &Fixed);
    let r = w.r.control_rect("tool.highlight").expect("button");
    for i in [
        Input::Press {
            x: r[0] + 5.0,
            y: r[1] + 5.0,
            count: 1,
            shift: false,
        },
        Input::Release {
            x: r[0] + 5.0,
            y: r[1] + 5.0,
        },
    ] {
        w.handle(i, &Fixed);
    }
    assert!(w.comment.tool.is_none(), "the editor's comment tool is off");
}

#[test]
fn web_links_bookmarks_and_articles() {
    let mut w = window(2);
    // The sample pages have no addresses: nothing is created.
    w.run("ed.webLinks.create", &Fixed);
    assert_eq!(links(&mut w, 0).len(), 0);
    // Bookmarks: add two, add a child, rename, move, delete.
    w.run("ed.bookmark", &Fixed);
    answer(&mut w, &["Một"]);
    w.view_mut().expect("v").go_to(1, None);
    w.run("ed.bm.add", &Fixed);
    answer(&mut w, &["Hai"]);
    let titles = |w: &crate::EditorWindow| -> Vec<String> {
        w.view()
            .expect("v")
            .doc()
            .outline()
            .iter()
            .map(|i| i.title.clone())
            .collect()
    };
    assert_eq!(titles(&w), ["Một", "Hai"]);
    assert_eq!(w.r.selected_outline, Some(vec![1]));
    w.run("ed.bm.up", &Fixed);
    assert_eq!(titles(&w), ["Hai", "Một"]);
    w.run("ed.bm.child", &Fixed);
    answer(&mut w, &["Con"]);
    assert_eq!(
        w.view().expect("v").doc().outline()[0].children[0].title,
        "Con"
    );
    w.run("ed.bm.rename", &Fixed);
    answer(&mut w, &["Hai (sửa)"]);
    assert_eq!(titles(&w), ["Hai (sửa)", "Một"]);
    w.run("ed.bm.delete", &Fixed);
    assert_eq!(titles(&w), ["Một"]);
    // Article: two boxes, Enter, title.
    w.run("ed.articleBox", &Fixed);
    assert_eq!(w.tools.tool, Some(BoxTool::Article));
    w.view_mut().expect("v").go_to(0, None);
    drag(&mut w, 0, [200.0, 150.0, 400.0, 300.0]);
    drag(&mut w, 0, [200.0, 320.0, 400.0, 450.0]);
    assert_eq!(w.tools.beads.len(), 2);
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    answer(&mut w, &["Bài viết"]);
    assert_eq!(
        w.view().expect("v").doc().articles(),
        vec![("Bài viết".to_owned(), 2)]
    );
}

#[test]
fn spelling_finds_and_changes_words() {
    let mut w = window(1);
    // The sample page says "Page 1": write a misspelt word.
    let v = w.view_mut().expect("v");
    v.doc_mut()
        .add_text(0, [72.0, 300.0], "Thiss sentense", 12.0, [0, 0, 0])
        .expect("text");
    w.run("ed.spell", &Fixed);
    let words: Vec<String> = w.tools.spell.iter().map(|m| m.word.clone()).collect();
    assert_eq!(words, ["Thiss", "sentense"]);
    let Some((d, Ask2::Tools(ToolAsk::Spell))) = w.dialog2.take() else {
        panic!("no spelling dialog");
    };
    assert_eq!(d.values()[0], "Thiss");
    // Change the first to "This", ignore the second.
    w.tools_answer(
        ToolAsk::Spell,
        DialogButton::Ok,
        &["Thiss".into(), "This".into()],
    );
    let Some((_, Ask2::Tools(ToolAsk::Spell))) = w.dialog2.take() else {
        panic!("no second word");
    };
    w.tools_answer(
        ToolAsk::Spell,
        DialogButton::FindNext,
        &["sentense".into(), "sentence".into()],
    );
    assert!(w.dialog2.is_none());
    let objs = w
        .view_mut()
        .expect("v")
        .doc_mut()
        .page_objects(0)
        .expect("o");
    let text: String = bo_pdfread::text_blocks(&objs)
        .iter()
        .map(|b| b.text.clone())
        .collect::<Vec<_>>()
        .join("|");
    assert!(text.contains("This sentense"), "{text}");
}

#[test]
fn command_search_runs_a_command() {
    let mut w = window(1);
    let b = w.search_box();
    w.handle(
        Input::Press {
            x: b[0] + 10.0,
            y: b[1] + 5.0,
            count: 1,
            shift: false,
        },
        &Fixed,
    );
    assert!(w.tools.search_on);
    w.handle(Input::Text("cross".into()), &Fixed);
    let m = w.matches();
    assert_eq!(m.first().map(|x| x.0), Some("ed.crossRef"), "{m:?}");
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    assert_eq!(w.tools.tool, Some(BoxTool::CrossRef));
    let mut rec = Rec::default();
    w.paint(&mut rec);
    assert!(
        rec.texts.iter().any(|t| t == "Search commands"),
        "{:?}",
        rec.texts
    );
}

#[test]
fn recognize_fields_and_new_field_kinds() {
    let mut w = window(1);
    let v = w.view_mut().expect("v");
    v.doc_mut()
        .add_text(0, [72.0, 300.0], "Name: ________________", 12.0, [0, 0, 0])
        .expect("text");
    w.run("form.recognize", &Fixed);
    let fields = w.view().expect("v").doc().form_fields();
    assert_eq!(fields.len(), 1, "{fields:?}");
    assert_eq!(fields[0].name, "Name");
    for (cmd, ft) in [
        ("fd.signature", "/FT /Sig"),
        ("fd.barcode", "Barcode1"),
        ("fd.image", "buttonImportIcon"),
    ] {
        w.run(cmd, &Fixed);
        assert!(w.design.tool.is_some(), "{cmd} on");
        drag(&mut w, 0, [300.0, 150.0, 450.0, 200.0]);
        let Some((d, what)) = w.dialog2.take() else {
            panic!("no properties dialog for {cmd}");
        };
        w.answer2(what, &d.values());
        let saved = w.view_mut().expect("v").doc_mut().save().expect("saves");
        assert!(String::from_utf8_lossy(&saved).contains(ft), "{cmd}");
        w.run(cmd, &Fixed);
    }
    assert_eq!(w.view().expect("v").doc().form_fields().len(), 4);
}

#[test]
fn optimize_saves_a_smaller_copy() {
    let mut w = window(2);
    w.run("file.optimize", &Fixed);
    answer(&mut w, &["150", "75"]);
    let saves: Vec<_> = w
        .take_requests()
        .into_iter()
        .filter(|r| matches!(r, crate::ReaderRequest::SaveBytes { .. }))
        .collect();
    assert_eq!(saves.len(), 1);
}
