//! Headless tests of paragraph typing and the Format tab of BlueEditor.
use crate::editor_format::PANE_W;
use crate::editor_tests::{key, texts, window, Fixed, Rec};
use crate::editor_window::EditorWindow;
use crate::{Input, Key};

/// Click Edit Text into the first paragraph of page 0, at its first character.
fn start_typing(w: &mut EditorWindow) {
    w.run("ed.editText", &Fixed);
    let v = w.view_mut().expect("view");
    let objs = v.doc_mut().page_objects(0).expect("objects");
    let b = bo_pdfread::text_blocks(&objs).remove(0);
    let r = v.doc_rect_to_window(0, b.bbox).expect("on screen");
    let (x, y) = (r[0] + 3.0, r[1] + r[3] / 2.0);
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

fn hit(w: &EditorWindow, cmd: &str) -> (f32, f32) {
    let r = w.pane.hits.iter().find(|h| h.1 == cmd).expect("button").0;
    (r[0] + r[2] / 2.0, r[1] + r[3] / 2.0)
}

fn press(w: &mut EditorWindow, (x, y): (f32, f32)) {
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

#[test]
fn format_pane_follows_edit_text() {
    let mut w = window(1);
    assert_eq!(w.r.right_pane_rect()[2], 0.0);
    let before = w.view().map(crate::ReaderView::frame).expect("view");
    // Round 8: Edit Text shows the pane collapsed to the reference's 18 px strip (the page
    // keeps its size); a click on the strip opens it.
    w.run("ed.editText", &Fixed);
    let strip = w.r.right_pane_rect();
    assert!((strip[2] - 18.0).abs() < 1.0, "{strip:?}");
    w.handle(
        Input::Press {
            x: strip[0] + 5.0,
            y: strip[1] + 100.0,
            count: 1,
            shift: false,
        },
        &Fixed,
    );
    assert!(w.format_open8);
    w.run("ed.editText", &Fixed);
    start_typing(&mut w);
    let pane = w.r.right_pane_rect();
    assert!((pane[2] - PANE_W * 1.25).abs() < 1.0, "{pane:?}");
    // The document makes room for it.
    let after = w.view().map(crate::ReaderView::frame).expect("view");
    assert!(after[2] < before[2] - 400.0, "{before:?} {after:?}");
    let mut rec = Rec::default();
    w.paint(&mut rec);
    assert!(rec.texts.iter().any(|t| t == "Format"));
    assert!(rec.texts.iter().any(|t| t == "Text Style"));
    assert!(rec.texts.iter().any(|t| t == "24"), "{:?}", rec.texts);
    // Select "Page" and make it bold with the pane's button.
    key(&mut w, Key::Home, false, false);
    for _ in 0..4 {
        key(&mut w, Key::Right, false, true);
    }
    let at = hit(&w, "fmt.bold");
    press(&mut w, at);
    {
        let t = w.edit.typing.as_ref().expect("typing");
        let b = t.block.as_ref().expect("block");
        assert!(b.styles[b.style_of[0]].bold);
        assert!(!b.styles[b.style_of[5]].bold);
    }
    key(&mut w, Key::End, false, false);
    w.run("fmt.italic", &Fixed);
    w.run("fmt.color.2", &Fixed);
    w.run("fmt.size.30", &Fixed);
    {
        let t = w.edit.typing.as_ref().expect("typing");
        let b = t.block.as_ref().expect("block");
        assert!(b.style_of.iter().all(|&k| b.styles[k].italic));
        assert!(b
            .style_of
            .iter()
            .all(|&k| b.styles[k].color == [0xC0, 0, 0]));
        assert!(b
            .style_of
            .iter()
            .all(|&k| (b.styles[k].size - 30.0).abs() < 0.01));
    }
    let st = w.format_status().unwrap_or_default();
    assert!(st.contains("30 pt"), "{st}");
    // Enter breaks the line, typing continues on the next one.
    w.handle(Input::Text(" A".into()), &Fixed);
    key(&mut w, Key::Enter, false, false);
    w.handle(Input::Text("B".into()), &Fixed);
    {
        let t = w.edit.typing.as_ref().expect("typing");
        let b = t.block.as_ref().expect("block");
        assert_eq!(b.lines, 2);
        assert_eq!(b.text.iter().collect::<String>(), "Page 1 A\nB");
    }
    // Up goes back to the first line.
    key(&mut w, Key::Up, false, false);
    assert!(w.edit.typing.as_ref().is_some_and(|t| t.caret <= 8));
    key(&mut w, Key::Escape, false, false);
    assert_eq!(texts(&mut w), ["Page 1 A\nB"]);
    // One undo step for the whole session.
    key(&mut w, Key::Letter('z'), true, false);
    assert_eq!(texts(&mut w), ["Page 1"]);
    // The close box hides the pane until Edit Text is turned on again.
    let at = hit(&w, "fmt.close");
    press(&mut w, at);
    assert_eq!(w.r.right_pane_rect()[2], 0.0);
    w.run("ed.editText", &Fixed);
    w.run("ed.editText", &Fixed);
    assert!(w.r.right_pane_rect()[2] > 0.0);
}

#[test]
fn typing_rewrites_only_the_paragraph() {
    let mut w = window(1);
    // Round 8: the Format pane starts collapsed; these tests use it opened.
    w.format_open8 = true;
    start_typing(&mut w);
    key(&mut w, Key::End, false, false);
    for c in "abcdefghij".chars() {
        w.handle(Input::Text(c.to_string()), &Fixed);
    }
    key(&mut w, Key::Escape, false, false);
    assert_eq!(texts(&mut w), ["Page 1abcdefghij"]);
}

#[test]
fn pane_rows_spacing_lists_and_replace() {
    let mut w = window(1);
    // Round 8: the Format pane starts collapsed; these tests use it opened.
    w.format_open8 = true;
    start_typing(&mut w);
    // Edit Text switches to the Edit tab.
    assert_eq!(w.r.ribbon_tab(), 2);
    let mut rec = Rec::default();
    w.paint(&mut rec);
    assert!(rec.texts.iter().any(|t| t == "Replace"), "{:?}", rec.texts);
    for cmd in [
        "fmt.underline",
        "fmt.strike",
        "fmt.super",
        "fmt.indent",
        "fmt.bullets",
        "fmt.numbering",
        "fmt.lineSp",
        "fmt.scale",
        "fmt.charSp",
        "fmt.wordSp",
    ] {
        assert!(w.pane.hits.iter().any(|h| h.1 == cmd), "{cmd}");
    }
    w.run("fmt.underline", &Fixed);
    w.run("fmt.super", &Fixed);
    w.run("fmt.scale.150", &Fixed);
    w.run("fmt.tc.1", &Fixed);
    {
        let b = w.edit.block().expect("block");
        let s = &b.styles[b.style_of[0]];
        assert!(s.underline && s.script == 1);
        assert!((s.scale() - 150.0).abs() < 0.01);
        assert!((s.char_spacing() - 1.0).abs() < 0.01);
    }
    w.run("fmt.list.b0", &Fixed);
    w.run("fmt.pitch.30", &Fixed);
    {
        let b = w.edit.block().expect("block");
        assert_eq!(b.text.iter().collect::<String>(), "\u{2022} Page 1");
        assert!((b.pitch - 30.0).abs() < 0.01);
    }
    w.run("fmt.list.n0", &Fixed);
    assert_eq!(
        w.edit.block().map(|b| b.text.iter().collect::<String>()),
        Some("1. Page 1".into())
    );
    w.run("fmt.list.none", &Fixed);
    key(&mut w, Key::Escape, false, false);
    assert_eq!(texts(&mut w), ["Page 1"]);
    // Search & Replace changes the text in its paragraph.
    w.run("ed.searchReplace", &Fixed);
    assert!(w.dialog2.is_some());
    w.dialog2 = None;
    w.answer2(
        crate::editor_actions::Ask2::Replace,
        &["Page".into(), "Trang".into()],
    );
    assert_eq!(texts(&mut w), ["Trang 1"]);
}
