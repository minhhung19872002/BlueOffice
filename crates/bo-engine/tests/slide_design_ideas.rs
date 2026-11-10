//! BlueSlide design suggestions (CORE-DESIGN-001): the content of a slide is analysed, the
//! suggestions come with thumbnails, applying one is one undo step that only moves / resizes /
//! crops the existing shapes (plus accent shapes), and the deck saves and reopens.
//!
//! With `BO_DESIGN_OUT=<dir>` the saved decks are kept there (for OpenXmlValidator:
//! `bo-cli validate <dir> --openxml require`).
// Tests may unwrap: a panic is a test failure.
#![allow(clippy::unwrap_used)]

use bo_engine::slide_edit::geom::place_slide;
use bo_engine::Engine;
use bo_model::slide::ShapeNode;
use serde_json::{json, Value};
use std::path::PathBuf;

fn deck(name: &str) -> Vec<u8> {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus/slide")
        .join(name);
    std::fs::read(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

fn open(bytes: &[u8]) -> Engine {
    let mut e = Engine::with_bundled_fonts();
    e.open(bytes, "design.pptx").unwrap();
    e
}

fn run(e: &mut Engine, cmd: &Value) -> Value {
    let out = e.slide_command_json(&cmd.to_string()).unwrap();
    let r: Value = serde_json::from_str(&out).unwrap();
    assert!(r.get("error").is_none(), "{cmd}: {:?}", r.get("error"));
    r
}

fn keep(name: &str, bytes: &[u8]) {
    if let Ok(dir) = std::env::var("BO_DESIGN_OUT") {
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(PathBuf::from(dir).join(name), bytes).unwrap();
    }
}

/// (id, name, text) of the slide's shapes other than accents.
fn content(e: &Engine, slide: usize) -> Vec<(u32, String, String)> {
    let p = e.presentation().unwrap();
    p.slides[slide]
        .data
        .shapes
        .iter()
        .filter_map(|n| {
            let nv = n.nv()?;
            if nv.name.starts_with("Design Accent") {
                return None;
            }
            let text = match n {
                ShapeNode::Shape(s) => s
                    .text
                    .as_ref()
                    .map(bo_model::slide::TextBody::text)
                    .unwrap_or_default(),
                _ => String::new(),
            };
            Some((nv.id, nv.name.clone(), text))
        })
        .collect()
}

fn accents(e: &Engine, slide: usize) -> usize {
    e.presentation().unwrap().slides[slide]
        .data
        .shapes
        .iter()
        .filter(|n| {
            n.nv()
                .is_some_and(|nv| nv.name.starts_with("Design Accent"))
        })
        .count()
}

/// Ideas of the current slide: (kind, ids).
fn ideas(e: &mut Engine) -> (String, Vec<String>) {
    let before = e.presentation().unwrap().clone();
    let r = run(e, &json!({ "cmd": "designIdeas", "width": 160 }));
    // Asking for ideas changes nothing (the previews are rolled back).
    assert_eq!(e.presentation().unwrap(), &before);
    assert_eq!(r["dirty"], json!([]));
    let d = &r["designIdeas"];
    let ideas = d["ideas"].as_array().unwrap();
    for i in ideas {
        // A thumbnail display list scaled to 160 points with drawing commands.
        let page = &i["page"];
        assert!(
            (page["width"].as_f64().unwrap() - 160.0).abs() < 0.5,
            "{page}"
        );
        assert_ne!(page["commands"], json!([]));
    }
    (
        d["kind"].as_str().unwrap_or_default().to_owned(),
        ideas
            .iter()
            .map(|i| i["id"].as_str().unwrap().to_owned())
            .collect(),
    )
}

fn inside_slide(e: &Engine, slide: usize) {
    let p = e.presentation().unwrap();
    let (sw, sh) = (
        p.slide_size.cx as f64 / 12_700.0,
        p.slide_size.cy as f64 / 12_700.0,
    );
    for pl in place_slide(p, slide) {
        let b = pl.b.bounds();
        assert!(
            b.x >= -0.5 && b.y >= -0.5 && b.right() <= sw + 0.5 && b.bottom() <= sh + 0.5,
            "shape {} at {b:?} outside {sw}×{sh}",
            pl.id
        );
    }
}

/// Every idea of slide `slide` of `name`: applied, inside the slide, the content unchanged,
/// one undo step back to the original.
fn check_deck(name: &str, slide: usize, kind: &str) -> Engine {
    let mut e = open(&deck(name));
    run(&mut e, &json!({ "cmd": "goTo", "slide": slide }));
    let (k, ids) = ideas(&mut e);
    assert_eq!(k, kind, "{name}");
    assert!((4..=8).contains(&ids.len()), "{name}: {ids:?}");
    let original = e.presentation().unwrap().slides[slide].clone();
    let before = content(&e, slide);
    for id in &ids {
        let r = run(&mut e, &json!({ "cmd": "designApply", "idea": id }));
        assert_eq!(r["canUndo"], true);
        assert_eq!(r["dirty"], json!([slide]), "{id}");
        // Same shapes, same text: only boxes change (and accents are added).
        assert_eq!(content(&e, slide), before, "{name} {id}");
        inside_slide(&e, slide);
        run(&mut e, &json!({ "cmd": "undo" }));
        assert_eq!(
            e.presentation().unwrap().slides[slide],
            original,
            "{name} {id}: one undo step restores the slide"
        );
    }
    e
}

#[test]
fn title_and_text_slide_gets_text_ideas() {
    let mut e = check_deck("slide-text-bullets.pptx", 1, "bullets");
    let (_, ids) = ideas(&mut e);
    assert!(ids.iter().all(|i| i.starts_with("bullets.")), "{ids:?}");
    assert_eq!(e.presentation().unwrap().slides.len(), 3);
}

#[test]
fn much_text_gets_long_text_ideas() {
    let mut e = open(&deck("slide-text-bullets.pptx"));
    run(&mut e, &json!({ "cmd": "goTo", "slide": 2 }));
    // The body placeholder (`ph idx=1`) gets a long paragraph text.
    let body = e.presentation().unwrap().slides[2]
        .data
        .shapes
        .iter()
        .find_map(|n| match n {
            ShapeNode::Shape(s) if s.nv.placeholder.as_ref().is_some_and(|p| p.idx == Some(1)) => {
                Some(s.nv.id)
            }
            _ => None,
        })
        .unwrap();
    let text = (1..=6)
        .map(|i| format!("Đoạn {i}: nội dung trình bày khá dài để kiểm tra bố cục chữ nhiều trên trang chiếu."))
        .collect::<Vec<_>>()
        .join("
");
    run(
        &mut e,
        &json!({ "cmd": "setText", "id": body, "text": text }),
    );
    let (kind, ids) = ideas(&mut e);
    assert_eq!(kind, "longText");
    assert!(ids.len() >= 4);
    let before = content(&e, 2);
    run(&mut e, &json!({ "cmd": "designApply", "idea": ids[1] }));
    assert_eq!(content(&e, 2), before);
    inside_slide(&e, 2);
}

#[test]
fn title_slide_gets_section_ideas() {
    let mut e = check_deck("slide-text-bullets.pptx", 0, "section");
    let (_, ids) = ideas(&mut e);
    assert!(ids.iter().all(|i| i.starts_with("section.")), "{ids:?}");
}

#[test]
fn table_and_chart_slides_get_visual_ideas() {
    check_deck("slide-tables-basic.pptx", 0, "visual");
    check_deck("slide-charts-column.pptx", 0, "visual");
}

#[test]
fn picture_slide_gets_picture_ideas_and_crops_without_distortion() {
    let mut e = check_deck("slide-images-crop.pptx", 0, "pictures");
    let (_, ids) = ideas(&mut e);
    run(&mut e, &json!({ "cmd": "designApply", "idea": ids[0] }));
    let p = e.presentation().unwrap();
    let placed = place_slide(p, 0);
    for n in &p.slides[0].data.shapes {
        if let ShapeNode::Picture(pic) = n {
            let b = placed.iter().find(|pl| pl.id == pic.nv.id).unwrap().b;
            assert!(b.w > 0.0 && b.h > 0.0);
            if let Some([l, t, r, bb]) = pic.blip.src_rect {
                assert!(l + r < 100_000 && t + bb < 100_000);
            }
        }
    }
}

#[test]
fn applying_another_idea_replaces_the_accents() {
    let mut e = open(&deck("slide-text-bullets.pptx"));
    let (_, ids) = ideas(&mut e);
    let with_accents: Vec<&String> = ids.iter().take(2).collect();
    run(
        &mut e,
        &json!({ "cmd": "designApply", "idea": with_accents[0] }),
    );
    let n1 = accents(&e, 0);
    assert!(n1 > 0);
    run(
        &mut e,
        &json!({ "cmd": "designApply", "idea": with_accents[1] }),
    );
    let n2 = accents(&e, 0);
    assert!(n2 > 0);
    // The second idea's accents only (not the first one's as well).
    let (_, again) = ideas(&mut e);
    assert_eq!(again, ids, "the accents do not count as content");
    let r = run(&mut e, &json!({ "cmd": "undo" }));
    assert_eq!(r["canUndo"], true);
    assert_eq!(accents(&e, 0), n1);
}

#[test]
fn unknown_idea_is_refused_and_the_deck_unchanged() {
    let mut e = open(&deck("slide-text-bullets.pptx"));
    let out = e
        .slide_command_json(&json!({ "cmd": "designApply", "idea": "nope" }).to_string())
        .unwrap();
    let r: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(r["error"], "noDesignIdea");
    assert_eq!(r["modified"], false);
}

#[test]
fn applied_idea_saves_and_reopens() {
    for (name, file) in [
        ("slide-text-bullets.pptx", "design-bullets.pptx"),
        ("slide-images-crop.pptx", "design-pictures.pptx"),
        ("slide-charts-column.pptx", "design-chart.pptx"),
    ] {
        let mut e = open(&deck(name));
        let (_, ids) = ideas(&mut e);
        // An idea with accents (the second one of every kind has some).
        run(&mut e, &json!({ "cmd": "designApply", "idea": ids[1] }));
        let accents_before = accents(&e, 0);
        let shapes = e.presentation().unwrap().slides[0].data.shapes.len();
        let boxes: Vec<_> = place_slide(e.presentation().unwrap(), 0)
            .into_iter()
            .map(|p| (p.id, p.b))
            .collect();
        let bytes = e.save_pptx().unwrap();
        keep(file, &bytes);
        let back = open(&bytes);
        let p = back.presentation().unwrap();
        assert_eq!(p.slides[0].data.shapes.len(), shapes, "{name}");
        assert_eq!(accents(&back, 0), accents_before, "{name}");
        let again: Vec<_> = place_slide(p, 0).into_iter().map(|p| (p.id, p.b)).collect();
        assert_eq!(again.len(), boxes.len());
        for ((ia, a), (ib, b)) in boxes.iter().zip(&again) {
            assert_eq!(ia, ib);
            // EMU rounding only.
            assert!(
                (a.cx - b.cx).abs() < 0.01 && (a.w - b.w).abs() < 0.01,
                "{name}: {a:?} {b:?}"
            );
        }
    }
}

#[test]
fn empty_slide_has_no_ideas() {
    let mut e = open(&deck("slide-text-bullets.pptx"));
    run(&mut e, &json!({ "cmd": "addSlide" }));
    run(&mut e, &json!({ "cmd": "selectAll" }));
    run(&mut e, &json!({ "cmd": "delete" }));
    let r = run(&mut e, &json!({ "cmd": "designIdeas" }));
    let d = &r["designIdeas"];
    assert!(d["kind"].is_null(), "{d}");
    assert_eq!(d["ideas"], json!([]));
    let out = e
        .slide_command_json(&json!({ "cmd": "designApply", "idea": "bullets.classic" }).to_string())
        .unwrap();
    let r: Value = serde_json::from_str(&out).unwrap();
    assert!(r["error"].is_string());
}
