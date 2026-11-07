//! BlueSlide editing, second part (Phase 4): rich text inside shapes, the
//! clipboard, marquee selection, adjust handles, connectors, table cells and
//! ungrouping flipped groups.

use bo_engine::slide_edit::{SlideCommand, SlideEditResult};
use bo_engine::Engine;
use bo_model::slide::{Autofit, Bullet, ShapeNode, Spacing, TextRun};
use std::path::PathBuf;

fn deck(name: &str) -> Vec<u8> {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus/slide")
        .join(name);
    std::fs::read(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

fn open(name: &str) -> Engine {
    let mut e = Engine::with_bundled_fonts();
    e.open(&deck(name), name).unwrap();
    e
}

fn try_run(e: &mut Engine, json: &str) -> SlideEditResult {
    let cmd: SlideCommand =
        serde_json::from_str(json).unwrap_or_else(|err| panic!("{json}: {err}"));
    e.slide_command(cmd).unwrap()
}

fn run(e: &mut Engine, json: &str) -> SlideEditResult {
    let r = try_run(e, json);
    assert!(r.error.is_none(), "{json}: {:?}", r.error);
    r
}

fn shape(e: &Engine, slide: usize, id: u32) -> &bo_model::slide::Shape {
    let p = e.presentation().unwrap();
    find_shape(&p.slides[slide].data.shapes, id).expect("shape")
}

fn find_shape(nodes: &[ShapeNode], id: u32) -> Option<&bo_model::slide::Shape> {
    for n in nodes {
        match n {
            ShapeNode::Shape(s) if s.nv.id == id => return Some(s),
            ShapeNode::Group(g) => {
                if let Some(s) = find_shape(&g.children, id) {
                    return Some(s);
                }
            }
            _ => {}
        }
    }
    None
}

fn text_box(e: &mut Engine, text: &str) -> u32 {
    let r = run(
        e,
        &format!(
            r#"{{"cmd":"insertTextBox","text":{},"rect":{{"x":100,"y":100,"w":300,"h":40}}}}"#,
            serde_json::json!(text)
        ),
    );
    r.selection[0].id
}

#[test]
fn bold_part_of_a_shape_text() {
    let mut e = open("slide-shapes-presets.pptx");
    let id = text_box(&mut e, "Xin chào Việt Nam");
    // Select "chào" (UTF-16 offsets 4..8) and make it bold.
    let r = run(
        &mut e,
        &format!(r#"{{"cmd":"textSelect","id":{id},"start":4,"end":8}}"#),
    );
    let te = r.text_edit.expect("text editing state");
    assert_eq!((te.start, te.end), (4, 8));
    assert!(te.caret.is_none());
    assert_eq!(te.rects.len(), 1, "one highlighted line");
    let r = run(&mut e, r#"{"cmd":"formatText","format":{"kind":"bold"}}"#);
    assert!(r.format.bold, "state at the selection");
    let s = shape(&e, 0, id);
    let runs: Vec<(String, Option<bool>)> = s.text.as_ref().unwrap().paragraphs[0]
        .runs
        .iter()
        .filter_map(|r| match r {
            TextRun::Run { props, text } => Some((text.clone(), props.b)),
            _ => None,
        })
        .collect();
    assert_eq!(
        runs,
        vec![
            ("Xin ".to_owned(), None),
            ("chào".to_owned(), Some(true)),
            (" Việt Nam".to_owned(), None)
        ]
    );
    // Caret in the plain part: not bold; italic + strike + font + highlight on a range.
    let r = run(
        &mut e,
        &format!(r#"{{"cmd":"textSelect","id":{id},"start":1,"end":1}}"#),
    );
    assert!(!r.format.bold);
    assert!(r.text_edit.unwrap().caret.is_some());
    for f in [
        r#"{"kind":"italic"}"#,
        r#"{"kind":"strike"}"#,
        r#"{"kind":"font","typeface":"Arial"}"#,
        r#"{"kind":"highlight","hex":"FFFF00"}"#,
        r#"{"kind":"color","hex":"C00000"}"#,
        r#"{"kind":"size","points":28}"#,
    ] {
        run(
            &mut e,
            &format!(r#"{{"cmd":"formatText","format":{f},"range":[9,13]}}"#),
        );
    }
    let r = run(
        &mut e,
        &format!(r#"{{"cmd":"textSelect","id":{id},"start":9,"end":13}}"#),
    );
    let f = &r.format;
    assert!(f.italic && f.strike && !f.bold);
    assert_eq!(f.font.as_deref(), Some("Arial"));
    assert_eq!(f.highlight.as_deref(), Some("FFFF00"));
    assert_eq!(f.color.as_deref(), Some("C00000"));
    assert_eq!(f.size, Some(28.0));
    // Saved and reopened: the runs survive.
    let bytes = e.save_pptx().unwrap();
    let mut e2 = Engine::with_bundled_fonts();
    e2.open(&bytes, "x.pptx").unwrap();
    let s2 = shape(&e2, 0, id);
    assert_eq!(s2.text, shape(&e, 0, id).text);
    // Undo restores the plain text runs.
    for _ in 0..7 {
        run(&mut e, r#"{"cmd":"undo"}"#);
    }
    let s = shape(&e, 0, id);
    assert_eq!(s.text.as_ref().unwrap().paragraphs[0].runs.len(), 1);
}

#[test]
fn paragraph_formats_levels_and_spacing() {
    let mut e = open("slide-shapes-presets.pptx");
    let id = text_box(&mut e, "Một\nHai\nBa");
    // Caret in "Hai": bullets, level up (Tab), centre, line spacing 1.5.
    run(
        &mut e,
        &format!(r#"{{"cmd":"textSelect","id":{id},"start":5,"end":5}}"#),
    );
    for f in [
        r#"{"kind":"bullets"}"#,
        r#"{"kind":"level","delta":1}"#,
        r#"{"kind":"align","align":"center"}"#,
        r#"{"kind":"lineSpacing","value":1.5}"#,
    ] {
        run(&mut e, &format!(r#"{{"cmd":"formatText","format":{f}}}"#));
    }
    let r = run(&mut e, r#"{"cmd":"state"}"#);
    assert!(r.format.bullets);
    assert_eq!(r.format.level, 1);
    assert_eq!(r.format.line_spacing, Some(1.5));
    let s = shape(&e, 0, id);
    let ps = &s.text.as_ref().unwrap().paragraphs;
    assert!(ps[0].props.is_none() || ps[0].props.as_ref().unwrap().bullet.is_none());
    let pp = ps[1].props.as_ref().unwrap();
    assert_eq!(pp.bullet, Some(Bullet::Char("•".into())));
    assert_eq!(pp.lvl, Some(1));
    assert_eq!(pp.mar_l, Some(2 * 342_900));
    assert_eq!(pp.algn.as_deref(), Some("ctr"));
    assert_eq!(pp.ln_spc, Some(Spacing::Percent(150_000)));
    // Numbering over the two last paragraphs; Shift+Tab back to level 0.
    run(
        &mut e,
        r#"{"cmd":"formatText","format":{"kind":"numbering"},"range":[4,10]}"#,
    );
    run(
        &mut e,
        r#"{"cmd":"formatText","format":{"kind":"level","delta":-1},"range":[4,10]}"#,
    );
    let s = shape(&e, 0, id);
    let ps = &s.text.as_ref().unwrap().paragraphs;
    for p in &ps[1..] {
        let pp = p.props.as_ref().unwrap();
        assert!(matches!(pp.bullet, Some(Bullet::AutoNum { .. })), "{pp:?}");
        assert_eq!(pp.lvl, None);
    }
}

#[test]
fn typing_merges_into_one_undo_step_and_text_box_grows() {
    let mut e = open("slide-shapes-presets.pptx");
    let id = text_box(&mut e, "");
    let h0 = run(&mut e, r#"{"cmd":"state"}"#).selection[0].rect.h;
    let mut text = String::new();
    for word in ["Dòng một", "\nDòng hai", "\nDòng ba", "\nDòng bốn"] {
        text.push_str(word);
        let n = text.encode_utf16().count();
        let r = run(
            &mut e,
            &format!(
                r#"{{"cmd":"setText","id":{id},"text":{},"merge":true,"sel":[{n},{n}]}}"#,
                serde_json::json!(text)
            ),
        );
        let te = r.text_edit.expect("editing");
        assert_eq!((te.start, te.end), (n, n));
        assert!(te.caret.is_some());
    }
    // spAutoFit: the box grew with four lines.
    let r = run(&mut e, r#"{"cmd":"state"}"#);
    assert!(
        r.selection[0].rect.h > h0 * 2.0,
        "{} vs {h0}",
        r.selection[0].rect.h
    );
    // The caret sits on the last line, below the first.
    let te = r.text_edit.unwrap();
    let c = te.caret.unwrap();
    assert!(c[1] > te.rect.cy, "{c:?}");
    // Clicking the first line puts the caret there.
    let top = te.rect.cy - te.rect.h / 2.0 + 10.0;
    let r = run(
        &mut e,
        &format!(
            r#"{{"cmd":"textHit","x":{},"y":{top}}}"#,
            te.rect.cx - te.rect.w / 2.0 + 2.0
        ),
    );
    assert_eq!(r.text_edit.unwrap().start, 0);
    // One undo removes all the typing (and the growth).
    run(&mut e, r#"{"cmd":"undo"}"#);
    let s = shape(&e, 0, id);
    assert_eq!(s.text.as_ref().unwrap().text(), "");
    let r = run(&mut e, r#"{"cmd":"textExit"}"#);
    assert!(r.text_edit.is_none());
    assert!((r.selection[0].rect.h - h0).abs() < 0.01);
}

#[test]
fn shrink_on_overflow_records_font_scale() {
    let mut e = open("slide-shapes-presets.pptx");
    let id = text_box(&mut e, "");
    // Turn the text box into a fixed box that shrinks text on overflow.
    {
        let p = e.presentation_mut().unwrap();
        for n in &mut p.slides[0].data.shapes {
            if let ShapeNode::Shape(s) = n {
                if s.nv.id == id {
                    s.text.as_mut().unwrap().body_pr.autofit = Some(Autofit::Normal {
                        font_scale: None,
                        ln_spc_reduction: None,
                    });
                }
            }
        }
    }
    let long = "Văn bản rất dài ".repeat(30);
    run(
        &mut e,
        &format!(
            r#"{{"cmd":"setText","id":{id},"text":{}}}"#,
            serde_json::json!(long)
        ),
    );
    let s = shape(&e, 0, id);
    match &s.text.as_ref().unwrap().body_pr.autofit {
        Some(Autofit::Normal {
            font_scale: Some(f),
            ..
        }) => assert!(*f < 100_000),
        other => panic!("{other:?}"),
    }
}
