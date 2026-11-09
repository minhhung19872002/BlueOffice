//! DOC-FMT-001 through the JSON bridge: advanced character formatting from the Font
//! dialog (ECMA-376 Part 1, §17.3.2 effects, spacing, position, scale, border; [MS-DOCX]
//! w14 text effects and typography; §17.3.2.10 two lines in one) and phonetic guides
//! (§17.3.3.25 ruby); the display list carries the effect drawing commands; saved as
//! .docx and read back. `BO_CHAR_FX_OUT=<file.docx>` also writes the saved file (for
//! the OpenXmlValidator check: `bo-cli validate <file> --openxml require`).
// Tests may unwrap: a panic is a test failure.
#![allow(clippy::unwrap_used)]

use bo_engine::Engine;
use bo_model::{Block, RunContent};
use serde_json::Value;

fn cmd(e: &mut Engine, json: &str) -> Value {
    let r: Value = serde_json::from_str(&e.command_json(json).unwrap()).unwrap();
    assert!(r.get("error").is_none(), "{json}: {:?}", r.get("error"));
    r
}

fn ops(e: &Engine) -> Vec<Value> {
    let page: Value = serde_json::from_str(&e.page_json(0).unwrap()).unwrap();
    page["commands"].as_array().unwrap().clone()
}

fn count(ops: &[Value], op: &str) -> usize {
    ops.iter().filter(|c| c["op"] == op).count()
}

#[test]
fn effects_through_the_bridge_and_into_the_display_list() {
    let mut e = Engine::with_bundled_fonts();
    e.new_document("a.docx").unwrap();
    cmd(&mut e, r#"{"cmd":"insertText","text":"Hiệu ứng"}"#);
    let plain = ops(&e);
    assert_eq!(count(&plain, "path"), 0);
    assert_eq!(count(&plain, "glyphs"), 1);
    cmd(&mut e, r#"{"cmd":"selectAll"}"#);
    // §17.3.2.23 outline: the glyph run stays (transparent, still text) and the
    // outlines are stroked over it; §17.3.2.31 shadow: one filled copy behind.
    let r = cmd(
        &mut e,
        r#"{"cmd":"formatText","format":{"kind":"advanced","patch":{"outline":true,"shadow":true}}}"#,
    );
    assert_eq!(r["format"]["advanced"]["outline"], true);
    let c = ops(&e);
    let glyphs = c.iter().position(|c| c["op"] == "glyphs").unwrap();
    assert_eq!(c[glyphs]["color"][3], 0, "hollow: transparent glyph run");
    let paths: Vec<usize> = c
        .iter()
        .enumerate()
        .filter(|(_, c)| c["op"] == "path")
        .map(|(i, _)| i)
        .collect();
    assert_eq!(paths.len(), 2);
    assert!(
        paths[0] < glyphs && c[paths[0]]["fill"].is_object(),
        "shadow first"
    );
    assert!(
        paths[1] > glyphs && c[paths[1]]["stroke"].is_object(),
        "outline over"
    );
    // w14 glow + soft shadow: two blurred paths behind; gradient fill: a path with a
    // linear paint over the (transparent) glyphs.
    cmd(
        &mut e,
        r#"{"cmd":"formatText","format":{"kind":"advanced","patch":{"outline":false,"shadow":false,
            "glow":{"color":"FFC000","radius":6,"transparency":40},
            "softShadow":{"color":"000000","blur":3,"distance":2,"angle":45,"transparency":50},
            "gradient":{"from":"C00000","to":"1F3A68","angle":0},
            "textOutline":{"color":"000000","width":0.5},
            "ligatures":"standard","scale":150,"position":6,"spacing":20,
            "border":{"style":"single","size":4,"color":null},"shading":"FFFF00"}}}"#,
    );
    let c = ops(&e);
    assert_eq!(count(&c, "blurPath"), 2);
    assert!(c
        .iter()
        .any(|c| c["op"] == "path" && c["fill"]["type"] == "linear"));
    // §17.3.2.43 w 150 %: the glyph run is scaled horizontally.
    let t = c.iter().find(|c| c["op"] == "pushTransform").unwrap();
    assert!((t["m"][0].as_f64().unwrap() - 1.5).abs() < 1e-6);
    // Ruby: insert, then it is reported and can be replaced.
    cmd(&mut e, r#"{"cmd":"move","motion":"docEnd"}"#);
    let r = cmd(
        &mut e,
        r#"{"cmd":"insertRuby","ruby":{"base":"Huế","guide":"cố đô","align":"center","size":10,"raise":20,"baseSize":24,"lang":"vi-VN"}}"#,
    );
    assert_eq!(r["ruby"]["guide"], "cố đô");
    let r = cmd(
        &mut e,
        r#"{"cmd":"insertRuby","replace":true,"ruby":{"base":"Huế","guide":"Thừa Thiên","align":"left","size":10,"raise":20,"baseSize":24,"lang":"vi-VN"}}"#,
    );
    assert_eq!(r["ruby"]["guide"], "Thừa Thiên");
    // Saved and read back.
    let bytes = e.save_docx().unwrap();
    if let Ok(path) = std::env::var("BO_CHAR_FX_OUT") {
        std::fs::write(path, &bytes).unwrap();
    }
    let d = bo_docx::read_docx(&bytes).unwrap();
    let Block::Paragraph(p) = &d.sections[0].blocks[0] else {
        panic!("paragraph")
    };
    let rp = &p.runs[0].props;
    assert_eq!(
        (rp.scale, rp.position, rp.outline),
        (Some(150), Some(6), None)
    );
    let fx = rp.effects.as_deref().unwrap();
    assert_eq!(fx.glow.as_ref().unwrap().radius, 76_200);
    assert_eq!(fx.ligatures.as_deref(), Some("standard"));
    assert!(fx.fill.is_some() && fx.outline.is_some() && fx.shadow.is_some());
    assert!(rp.border.is_some() && rp.shading.is_some());
    let rubies = p
        .runs
        .iter()
        .flat_map(|r| &r.content)
        .filter(|c| matches!(c, RunContent::Opaque(x) if bo_model::index::is_ruby_xml(&x.xml)))
        .count();
    assert_eq!(rubies, 1);
}
