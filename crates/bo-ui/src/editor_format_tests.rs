//! Headless tests of paragraph typing and the Format tab of BlueEditor.
use crate::editor_format::TAB_FORMAT;
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

fn tabs(w: &EditorWindow) -> usize {
    w.r.profile.as_ref().map_or(0, |p| p.tabs.len())
}

#[test]
fn format_tab_appears_while_typing() {
    let mut w = window(1);
    assert_eq!(tabs(&w), TAB_FORMAT);
    start_typing(&mut w);
    assert_eq!(tabs(&w), TAB_FORMAT + 1);
    assert_eq!(w.r.ribbon_tab(), TAB_FORMAT);
    let mut rec = Rec::default();
    w.paint(&mut rec);
    assert!(rec.texts.iter().any(|t| t == "Format"));
    assert!(rec.texts.iter().any(|t| t == "Bold"));
    // Select "Page" and make it bold, then the whole line italic and red.
    key(&mut w, Key::Home, false, false);
    for _ in 0..4 {
        key(&mut w, Key::Right, false, true);
    }
    w.run("fmt.bold", &Fixed);
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
    assert_eq!(tabs(&w), TAB_FORMAT);
    assert_eq!(w.r.ribbon_tab(), 0);
    assert_eq!(texts(&mut w), ["Page 1 A\nB"]);
    // One undo step for the whole session.
    key(&mut w, Key::Letter('z'), true, false);
    assert_eq!(texts(&mut w), ["Page 1"]);
}

#[test]
fn typing_rewrites_only_the_paragraph() {
    let mut w = window(1);
    start_typing(&mut w);
    key(&mut w, Key::End, false, false);
    for c in "abcdefghij".chars() {
        w.handle(Input::Text(c.to_string()), &Fixed);
    }
    key(&mut w, Key::Escape, false, false);
    assert_eq!(texts(&mut w), ["Page 1abcdefghij"]);
}
