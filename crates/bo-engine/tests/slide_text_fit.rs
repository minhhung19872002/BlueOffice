//! BlueSlide text as the reference presentation app lays it out: long words break inside
//! the box, auto-fit follows resizing and new font sizes, custom placeholder prompts.
// Tests may unwrap: a panic is a test failure.
#![allow(clippy::unwrap_used)]

use bo_engine::slide_edit::{SlideCommand, SlideEditResult};
use bo_engine::Engine;
use bo_model::slide::{Autofit, ShapeNode};
use std::path::PathBuf;

fn open(name: &str) -> Engine {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus/slide")
        .join(name);
    let bytes = std::fs::read(&p).unwrap();
    let mut e = Engine::with_bundled_fonts();
    e.open(&bytes, name).unwrap();
    e
}

fn run(e: &mut Engine, json: &str) -> SlideEditResult {
    let cmd: SlideCommand =
        serde_json::from_str(json).unwrap_or_else(|err| panic!("{json}: {err}"));
    let r = e.slide_command(cmd).unwrap();
    assert!(r.error.is_none(), "{json}: {:?}", r.error);
    r
}

fn text_box(e: &mut Engine, text: &str, w: f64) -> (u32, f64) {
    let r = run(
        e,
        &format!(
            r#"{{"cmd":"insertTextBox","text":{},"rect":{{"x":100,"y":100,"w":{w},"h":40}}}}"#,
            serde_json::json!(text)
        ),
    );
    (r.selection[0].id, r.selection[0].rect.h)
}

fn set_text(e: &mut Engine, id: u32, text: &str) -> f64 {
    run(
        e,
        &format!(
            r#"{{"cmd":"setText","id":{id},"text":{}}}"#,
            serde_json::json!(text)
        ),
    );
    run(e, r#"{"cmd":"state"}"#).selection[0].rect.h
}

#[test]
fn a_word_wider_than_the_box_breaks_between_characters() {
    let mut e = open("slide-shapes-presets.pptx");
    let (id, _) = text_box(&mut e, "", 200.0);
    let one = set_text(&mut e, id, "Hello");
    // The reference (200 pt box, 18 pt Calibri) breaks this word into two lines and puts
    // "end" on a third: the box (spAutoFit) is three lines high.
    let long = set_text(
        &mut e,
        id,
        "Supercalifragilisticexpialidociousxyzabcdefghij end",
    );
    let line = one - 7.2; // one line plus the top and bottom insets (3.6 pt each)
    let lines = ((long - 7.2) / line).round();
    assert!((lines - 3.0).abs() < 0.01, "{long} vs one line {one}");
    // One very long word alone wraps too (it used to run out of the box on one line).
    let alone = set_text(&mut e, id, &"abcdefghij".repeat(8));
    assert!(alone > one * 2.0, "{alone} vs {one}");
}

#[test]
fn resizing_a_text_box_fits_its_height_to_the_wrapped_text() {
    let mut e = open("slide-shapes-presets.pptx");
    let (id, _) = text_box(&mut e, "", 300.0);
    let h = set_text(&mut e, id, &"word ".repeat(30));
    // Narrower: more lines, so the box (spAutoFit) grows on its own.
    run(
        &mut e,
        r#"{"cmd":"transform","transform":{"kind":"resize","handle":"e","dx":-150,"dy":0},"snap":false}"#,
    );
    let r = run(&mut e, r#"{"cmd":"state"}"#);
    assert!(
        r.selection[0].rect.h > h * 1.5,
        "{} vs {h}",
        r.selection[0].rect.h
    );
    // A bigger font grows it again.
    let h2 = r.selection[0].rect.h;
    run(
        &mut e,
        r#"{"cmd":"formatText","format":{"kind":"size","points":32}}"#,
    );
    let r = run(&mut e, r#"{"cmd":"state"}"#);
    assert!(
        r.selection[0].rect.h > h2 * 1.3,
        "{} vs {h2}",
        r.selection[0].rect.h
    );
}

#[test]
fn shrink_on_overflow_follows_a_resize() {
    let mut e = open("slide-shapes-presets.pptx");
    let (id, _) = text_box(&mut e, "", 300.0);
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
    set_text(&mut e, id, &"text ".repeat(12));
    let scale = |e: &Engine| {
        let p = e.presentation().unwrap();
        p.slides[0]
            .data
            .shapes
            .iter()
            .find_map(|n| match n {
                ShapeNode::Shape(s) if s.nv.id == id => s.text.as_ref()?.body_pr.autofit.clone(),
                _ => None,
            })
            .unwrap()
    };
    // Make the box much shorter and narrower: the text now overflows, so it shrinks.
    run(
        &mut e,
        r#"{"cmd":"transform","transform":{"kind":"resize","handle":"se","dx":-200,"dy":-20},"snap":false}"#,
    );
    match scale(&e) {
        Autofit::Normal {
            font_scale: Some(f),
            ..
        } => assert!(f < 100_000, "{f}"),
        other => panic!("{other:?}"),
    }
}

#[test]
fn custom_placeholder_prompt_comes_from_the_layout() {
    let mut e = open("slide-edit-slides.pptx");
    let r = run(&mut e, r#"{"cmd":"addSlide","layout":1}"#);
    assert_ne!(r.placeholders.len(), 0);
    // No custom prompt: nothing reported (the window shows its default prompts).
    assert!(
        r.placeholder_prompts.is_empty(),
        "{:?}",
        r.placeholder_prompts
    );
    let slide = r.slide;
    {
        let p = e.presentation_mut().unwrap();
        let layout = p.slides[slide].layout;
        for n in &mut p.layouts[layout].data.shapes {
            if let ShapeNode::Shape(s) = n {
                let Some(ph) = s.nv.placeholder.as_mut() else {
                    continue;
                };
                if matches!(ph.kind.as_deref(), Some("title")) {
                    ph.has_custom_prompt = Some(true);
                    let tb = s.text.get_or_insert_with(Default::default);
                    bo_engine::slide_edit::text::set_text(tb, "Type the agenda title");
                }
            }
        }
    }
    let r = run(&mut e, r#"{"cmd":"state"}"#);
    let i = r
        .placeholder_kinds
        .iter()
        .position(|k| k == "title")
        .expect("title placeholder");
    assert_eq!(r.placeholder_prompts[i], "Type the agenda title");
    // Other placeholders keep the default prompt.
    for (j, p) in r.placeholder_prompts.iter().enumerate() {
        if j != i {
            assert!(p.is_empty(), "{p}");
        }
    }
}
