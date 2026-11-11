//! BlueSlide commands added for everyday behaviour of presentation programs (pass 2 of
//! docs/parity/office-behaviour-slide.md): shape boxes for an immediate selection, superscript
//! and subscript, the font dialog's attributes, the format painter and the header and footer of
//! slides — through the JSON commands, saved, reopened and undone.
// Tests may unwrap: a panic is a test failure.
#![allow(clippy::unwrap_used)]

use bo_engine::slide_edit::{SlideCommand, SlideEditResult};
use bo_engine::Engine;
use bo_model::slide::{Fill, ShapeNode, TextRun};
use std::path::PathBuf;

const DECK: &str = "slide-text-bullets.pptx";

fn open() -> Engine {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus/slide")
        .join(DECK);
    let mut e = Engine::with_bundled_fonts();
    e.open(&std::fs::read(&p).unwrap(), DECK).unwrap();
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

fn reopen(e: &Engine) -> Engine {
    let bytes = e.save_pptx().unwrap();
    let mut back = Engine::with_bundled_fonts();
    back.open(&bytes, DECK).unwrap();
    back
}

fn json<T: serde::Serialize>(v: &T) -> serde_json::Value {
    serde_json::to_value(v).unwrap()
}

/// Kinds of the header / footer placeholders of slide `i`.
fn hf_kinds(e: &Engine, i: usize) -> Vec<String> {
    let p = e.presentation().unwrap();
    p.slides[i]
        .data
        .shapes
        .iter()
        .filter_map(|n| n.nv()?.placeholder.as_ref()?.kind.clone())
        .filter(|k| k == "dt" || k == "ftr" || k == "sldNum")
        .collect()
}

#[test]
fn shape_boxes_match_the_engine_hit_test() {
    let mut e = open();
    let r = run(&mut e, r#"{"cmd":"shapeBoxes"}"#);
    let boxes = json(&r)["shapeBoxes"].clone();
    assert_eq!(boxes["tolerance"], 3.0);
    let shapes = boxes["shapes"].as_array().unwrap();
    // Slide 1: title and subtitle, bottom to top; their boxes are the selection boxes.
    assert_eq!(shapes.len(), 2);
    let all = run(&mut e, r#"{"cmd":"selectAll"}"#);
    for s in shapes {
        let id = s["id"].as_u64().unwrap();
        let sel = all
            .selection
            .iter()
            .find(|x| u64::from(x.id) == id)
            .unwrap();
        assert_eq!(s["boxes"][0], json(&sel.rect));
        // A click at the centre of the box hits that shape (the topmost one there).
        let (cx, cy) = (sel.rect.cx, sel.rect.cy);
        let hit = run(&mut e, &format!(r#"{{"cmd":"hitTest","x":{cx},"y":{cy}}}"#));
        assert_eq!(hit.hits.unwrap(), vec![u32::try_from(id).unwrap()]);
    }
    // A query: the selection stays.
    let r = run(&mut e, r#"{"cmd":"shapeBoxes"}"#);
    assert_eq!(r.selection.len(), 2);
}

#[test]
fn superscript_subscript_caps_spacing_and_double_strike() {
    let mut e = open();
    // The title of slide 1 (108,81 720×162): select "Báo" (3 characters).
    run(&mut e, r#"{"cmd":"click","x":400,"y":160}"#);
    run(&mut e, r#"{"cmd":"textSelect","start":0,"end":3}"#);
    let r = run(
        &mut e,
        r#"{"cmd":"formatText","format":{"kind":"baseline","value":30000}}"#,
    );
    assert_eq!(r.format.baseline, 30000);
    let r = run(
        &mut e,
        r#"{"cmd":"formatText","format":{"kind":"baseline","value":-25000}}"#,
    );
    assert_eq!(r.format.baseline, -25000);
    run(
        &mut e,
        r#"{"cmd":"formatText","format":{"kind":"caps","value":"small"}}"#,
    );
    run(
        &mut e,
        r#"{"cmd":"formatText","format":{"kind":"spacing","points":3}}"#,
    );
    let r = run(
        &mut e,
        r#"{"cmd":"formatText","format":{"kind":"doubleStrike"}}"#,
    );
    assert_eq!(r.format.caps.as_deref(), Some("small"));
    assert_eq!(r.format.spacing, Some(3.0));
    assert!(r.format.double_strike);
    // Saved as rPr@baseline / @cap / @spc / @strike (§21.1.2.3.9).
    let back = reopen(&e);
    let p = back.presentation().unwrap();
    let ShapeNode::Shape(title) = &p.slides[0].data.shapes[0] else {
        panic!("title");
    };
    let TextRun::Run { props, text } = &title.text.as_ref().unwrap().paragraphs[0].runs[0] else {
        panic!("run");
    };
    assert_eq!(text, "Báo");
    assert_eq!(props.baseline, Some(-25000));
    assert_eq!(props.cap.as_deref(), Some("small"));
    assert_eq!(props.spc, Some(300));
    assert_eq!(props.strike.as_deref(), Some("dblStrike"));
    // Ctrl+Space: the character formatting goes, the text takes the placeholder's again.
    let r = run(
        &mut e,
        r#"{"cmd":"formatText","format":{"kind":"clearChar"}}"#,
    );
    assert_eq!(r.format.baseline, 0);
    assert_eq!(r.format.caps, None);
    assert_eq!(r.format.spacing, None);
    assert!(!r.format.double_strike);
    run(&mut e, r#"{"cmd":"undo"}"#);
    // 0 puts the text back on the line.
    let r = run(
        &mut e,
        r#"{"cmd":"formatText","format":{"kind":"baseline","value":0}}"#,
    );
    assert_eq!(r.format.baseline, 0);
}

#[test]
fn format_painter_copies_text_and_shape_formatting() {
    let mut e = open();
    // Text: bold + 40 pt on "Báo", painted onto "kinh".
    run(&mut e, r#"{"cmd":"click","x":400,"y":160}"#);
    run(&mut e, r#"{"cmd":"textSelect","start":0,"end":3}"#);
    run(
        &mut e,
        r#"{"cmd":"formatText","format":{"kind":"bold","value":true}}"#,
    );
    run(
        &mut e,
        r#"{"cmd":"formatText","format":{"kind":"size","points":40}}"#,
    );
    run(&mut e, r#"{"cmd":"copyFormat"}"#);
    // "Báo cáo kết quả kinh doanh": "kinh" is 16..20.
    run(&mut e, r#"{"cmd":"textSelect","start":16,"end":20}"#);
    let r = run(&mut e, r#"{"cmd":"pasteFormat"}"#);
    assert!(r.format.bold);
    assert_eq!(r.format.size, Some(40.0));
    // One undo step takes it back.
    let r = run(&mut e, r#"{"cmd":"undo"}"#);
    assert!(!r.format.bold || r.format.size != Some(40.0));
    run(&mut e, r#"{"cmd":"textExit"}"#);

    // Shapes: a red rectangle painted onto a new ellipse keeps the ellipse's geometry.
    run(&mut e, r#"{"cmd":"insertShape","preset":"rect"}"#);
    run(&mut e, r#"{"cmd":"setFill","color":"FF0000"}"#);
    run(&mut e, r#"{"cmd":"copyFormat"}"#);
    let r = run(
        &mut e,
        r#"{"cmd":"insertShape","preset":"ellipse","rect":{"x":10,"y":10,"w":50,"h":40}}"#,
    );
    let id = r.selection[0].id;
    run(&mut e, r#"{"cmd":"pasteFormat"}"#);
    let p = e.presentation().unwrap();
    let painted = p.slides[0]
        .data
        .shapes
        .iter()
        .find(|n| n.nv().is_some_and(|nv| nv.id == id))
        .unwrap();
    let ShapeNode::Shape(s) = painted else {
        panic!("shape");
    };
    assert!(matches!(&s.props.fill, Some(Fill::Solid(_))));
    // The geometry (§20.1.9.18 prstGeom) is the ellipse's own.
    assert!(format!("{:?}", s.props.geometry).contains("ellipse"));
    // Nothing taken yet in a fresh editor: refused.
    let mut fresh = open();
    run(&mut fresh, r#"{"cmd":"click","x":400,"y":160}"#);
    assert_eq!(
        try_run(&mut fresh, r#"{"cmd":"pasteFormat"}"#)
            .error
            .as_deref(),
        Some("noFormat")
    );
}

#[test]
fn header_and_footer_on_slides() {
    let mut e = open();
    let r = run(&mut e, r#"{"cmd":"headerFooterInfo"}"#);
    let info = json(&r)["headerFooter"].clone();
    assert_eq!(info["slideNumber"], false);
    assert!(info.get("footer").is_none());

    // Apply to all, not on the title slide (slide 1 uses the title slide layout).
    run(
        &mut e,
        r#"{"cmd":"headerFooter","all":true,"spec":{"slideNumber":true,"footer":"Bluestar","date":{"text":"11/11/2026","auto":false},"notOnTitle":true}}"#,
    );
    assert!(hf_kinds(&e, 0).is_empty());
    for i in 1..3 {
        let mut k = hf_kinds(&e, i);
        k.sort();
        assert_eq!(k, ["dt", "ftr", "sldNum"]);
    }
    run(&mut e, r#"{"cmd":"goTo","slide":1}"#);
    let info = json(&run(&mut e, r#"{"cmd":"headerFooterInfo"}"#))["headerFooter"].clone();
    assert_eq!(info["slideNumber"], true);
    assert_eq!(info["footer"], "Bluestar");
    assert_eq!(info["date"]["text"], "11/11/2026");
    assert_eq!(info["date"]["auto"], false);
    assert_eq!(info["notOnTitle"], true);

    // Saved and reopened: the slide number is a `slidenum` field (§21.1.2.2.4).
    let back = reopen(&e);
    let p = back.presentation().unwrap();
    let num = p.slides[2]
        .data
        .shapes
        .iter()
        .find_map(|n| match n {
            ShapeNode::Shape(s)
                if s.nv.placeholder.as_ref().and_then(|ph| ph.kind.as_deref())
                    == Some("sldNum") =>
            {
                Some(s.text.clone().unwrap())
            }
            _ => None,
        })
        .unwrap();
    assert!(matches!(
        &num.paragraphs[0].runs[0],
        TextRun::Field { kind: Some(k), .. } if k == "slidenum"
    ));

    // Applied to the current slide only, without the footer: the footer goes, one undo step.
    run(
        &mut e,
        r#"{"cmd":"headerFooter","spec":{"slideNumber":true}}"#,
    );
    assert_eq!(hf_kinds(&e, 1), ["sldNum"]);
    assert_eq!(hf_kinds(&e, 2).len(), 3);
    run(&mut e, r#"{"cmd":"undo"}"#);
    assert_eq!(hf_kinds(&e, 1).len(), 3);
}
