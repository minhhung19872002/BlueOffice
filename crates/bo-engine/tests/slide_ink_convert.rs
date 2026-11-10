//! BlueSlide ink conversion (CORE-INK-002): strokes drawn as ink become
//! DrawingML preset shapes (ECMA-376 Part 1, §20.1.9.18 prstGeom) in one undo
//! step, recognised strokes are converted while drawing in the automatic mode,
//! and recognised handwriting replaces ink with a text box or an equation
//! ([MS-ODRAWXML] a14:m). The saved decks reopen with the same shapes; with
//! `BO_INK_OUT=<dir>` they are written there for `bo-cli validate`.
// Tests may unwrap: a panic is a test failure.
#![allow(clippy::unwrap_used)]

use bo_engine::Engine;
use bo_opc::{Limits, Package};
use std::fmt::Write as _;
use std::path::PathBuf;

fn engine() -> Engine {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus/slide/slide-edit-text.pptx");
    let mut e = Engine::with_bundled_fonts();
    e.open(&std::fs::read(p).unwrap(), "ink.pptx").unwrap();
    run(&mut e, r#"{"cmd":"goTo","slide":0}"#);
    e
}

fn run(e: &mut Engine, json: &str) -> serde_json::Value {
    let out = e.slide_command_json(json).unwrap();
    let r: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert!(r.get("error").is_none(), "{json}: {:?}", r.get("error"));
    r
}

fn save(e: &Engine, name: &str) -> Vec<u8> {
    let bytes = e.save_pptx().unwrap();
    if let Ok(dir) = std::env::var("BO_INK_OUT") {
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(PathBuf::from(dir).join(name), &bytes).unwrap();
    }
    bytes
}

fn slide_xml(bytes: &[u8]) -> String {
    let pkg = Package::read(bytes, &Limits::default()).unwrap();
    String::from_utf8(pkg.part("/ppt/slides/slide1.xml").unwrap().data.clone()).unwrap()
}

fn stroke_json(pts: &[(f64, f64)], color: &str, width: f64) -> String {
    let mut s = String::from("[");
    for (i, (x, y)) in pts.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        write!(s, "[{x:.2},{y:.2}]").unwrap();
    }
    s.push(']');
    format!(r#"{{"points":{s},"color":"{color}","width":{width}}}"#)
}

fn add(e: &mut Engine, pts: &[(f64, f64)], extra: &str) -> serde_json::Value {
    let cmd = format!(
        r#"{{"cmd":"addInk","stroke":{}{extra}}}"#,
        stroke_json(pts, "1F5FBF", 3.0)
    );
    run(e, &cmd)
}

/// A rough rectangle: corners with a little wobble, one point per 5 pt.
fn rough_rect(x: f64, y: f64, w: f64, h: f64) -> Vec<(f64, f64)> {
    let c = [
        (x, y),
        (x + w, y + 2.0),
        (x + w - 1.0, y + h),
        (x + 1.0, y + h - 1.0),
        (x + 2.0, y + 3.0),
    ];
    let mut out = Vec::new();
    for i in 0..4 {
        let (a, b) = (c[i], c[i + 1]);
        let n = ((b.0 - a.0).hypot(b.1 - a.1) / 5.0).ceil() as usize;
        for s in 0..n {
            let t = s as f64 / n as f64;
            let wob = if s % 2 == 0 { 0.8 } else { -0.8 };
            out.push((a.0 + t * (b.0 - a.0) + wob, a.1 + t * (b.1 - a.1) - wob));
        }
    }
    out.push(c[4]);
    out
}

fn circle(cx: f64, cy: f64, r: f64) -> Vec<(f64, f64)> {
    (0..=72)
        .map(|i| {
            let a = f64::from(i) * 5.0f64.to_radians();
            let rr = r + if i % 3 == 0 { 1.0 } else { -0.5 };
            (cx + rr * a.cos(), cy + rr * a.sin())
        })
        .collect()
}

fn seg(a: (f64, f64), b: (f64, f64), n: usize) -> Vec<(f64, f64)> {
    (0..=n)
        .map(|i| {
            let t = i as f64 / n as f64;
            (a.0 + t * (b.0 - a.0), a.1 + t * (b.1 - a.1))
        })
        .collect()
}

fn ink_count(e: &mut Engine) -> usize {
    run(e, r#"{"cmd":"inkInfo"}"#)["ink"]["objects"]
        .as_array()
        .unwrap()
        .len()
}

#[test]
fn ink_becomes_shapes_in_one_undo_step() {
    let mut e = engine();
    let n0 = ink_count(&mut e);
    add(&mut e, &rough_rect(60.0, 60.0, 200.0, 110.0), "");
    add(&mut e, &circle(420.0, 140.0, 60.0), "");
    // An arrow: a shaft and two head strokes at its right end.
    add(&mut e, &seg((80.0, 300.0), (320.0, 300.0), 40), "");
    add(&mut e, &seg((320.0, 300.0), (295.0, 285.0), 6), "");
    add(&mut e, &seg((320.0, 300.0), (295.0, 315.0), 6), "");
    // A scribble that stays ink.
    let scribble: Vec<(f64, f64)> = (0..40)
        .map(|i| {
            (
                450.0 + f64::from(i) * 3.0,
                320.0 + if i % 2 == 0 { 15.0 } else { -15.0 } + f64::from(i % 7),
            )
        })
        .collect();
    add(&mut e, &scribble, "");
    assert_eq!(ink_count(&mut e), n0 + 6);

    let r = run(&mut e, r#"{"cmd":"inkToShapes"}"#);
    let conv = &r["ink"]["converted"];
    let kinds: Vec<&str> = conv["shapes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["kind"].as_str().unwrap())
        .collect();
    assert_eq!(kinds, ["rectangle", "circle", "arrow"]);
    assert_eq!(conv["kept"], 1);
    assert_eq!(r["ink"]["objects"].as_array().unwrap().len(), n0 + 1);
    // The new shapes are selected.
    assert_eq!(r["selection"].as_array().unwrap().len(), 3);

    let bytes = save(&e, "ink-shapes.pptx");
    let xml = slide_xml(&bytes);
    assert!(xml.contains(r#"<a:prstGeom prst="rect">"#));
    assert!(xml.contains(r#"<a:prstGeom prst="ellipse">"#));
    assert!(
        xml.contains(r#"<a:prstGeom prst="line">"#)
            && xml.contains(r#"<a:tailEnd type="triangle"/>"#)
    );
    // The ink's colour and width: 3 pt = 38100 EMU, 1F5FBF.
    assert!(
        xml.matches(r#"<a:ln w="38100" cap="rnd"><a:solidFill><a:srgbClr val="1F5FBF"/>"#)
            .count()
            >= 3
    );
    // Only the scribble's ink part is left in the package.
    let pkg = Package::read(&bytes, &Limits::default()).unwrap();
    let inks = pkg
        .parts()
        .iter()
        .filter(|p| p.name.starts_with("/ppt/ink/"))
        .count();
    assert_eq!(inks, n0 + 1);
    // Reopened: the same shapes.
    let mut back = Engine::with_bundled_fonts();
    back.open(&bytes, "back.pptx").unwrap();
    assert_eq!(slide_xml(&back.save_pptx().unwrap()), xml);

    // One undo step brings back all the ink, redo converts again.
    run(&mut e, r#"{"cmd":"undo"}"#);
    assert_eq!(ink_count(&mut e), n0 + 6);
    run(&mut e, r#"{"cmd":"redo"}"#);
    assert_eq!(ink_count(&mut e), n0 + 1);
}

#[test]
fn only_the_chosen_ink_is_converted() {
    let mut e = engine();
    let a = add(&mut e, &rough_rect(60.0, 60.0, 200.0, 110.0), "");
    let id_a = a["ink"]["objects"].as_array().unwrap().last().unwrap()["id"]
        .as_u64()
        .unwrap();
    add(&mut e, &circle(420.0, 140.0, 60.0), "");
    let n = ink_count(&mut e);
    let r = run(
        &mut e,
        &format!(r#"{{"cmd":"inkToShapes","ids":[{id_a}]}}"#),
    );
    assert_eq!(r["ink"]["converted"]["shapes"][0]["kind"], "rectangle");
    assert_eq!(ink_count(&mut e), n - 1);
    // Nothing recognisable: nothing changes, no undo step.
    let s = add(&mut e, &seg((10.0, 10.0), (12.0, 11.0), 2), "");
    let tiny = s["ink"]["objects"].as_array().unwrap().last().unwrap()["id"]
        .as_u64()
        .unwrap();
    let r = run(
        &mut e,
        &format!(r#"{{"cmd":"inkToShapes","ids":[{tiny}]}}"#),
    );
    assert_eq!(r["ink"]["converted"]["shapes"].as_array().unwrap().len(), 0);
    assert_eq!(r["ink"]["converted"]["kept"], 1);
    run(&mut e, r#"{"cmd":"undo"}"#);
    // The undo took back the tiny stroke itself.
    assert_eq!(ink_count(&mut e), n - 1);
}

#[test]
fn automatic_mode_converts_while_drawing() {
    let mut e = engine();
    let n0 = ink_count(&mut e);
    let r = add(
        &mut e,
        &rough_rect(60.0, 60.0, 200.0, 110.0),
        r#","autoShape":true"#,
    );
    assert_eq!(r["ink"]["converted"]["shapes"][0]["kind"], "rectangle");
    assert_eq!(ink_count(&mut e), n0);
    // A line, then its two head strokes join it as an arrow.
    let r = add(
        &mut e,
        &seg((80.0, 300.0), (320.0, 300.0), 40),
        r#","autoShape":true"#,
    );
    let line = &r["ink"]["converted"]["shapes"][0];
    assert_eq!(line["kind"], "line");
    let id = line["id"].as_u64().unwrap();
    for head in [
        seg((320.0, 300.0), (295.0, 285.0), 6),
        seg((320.0, 300.0), (295.0, 315.0), 6),
    ] {
        let r = add(
            &mut e,
            &head,
            &format!(r#","autoShape":true,"attachTo":{id}"#),
        );
        assert_eq!(r["ink"]["converted"]["shapes"][0]["kind"], "arrow");
        assert_eq!(r["ink"]["converted"]["shapes"][0]["id"], id);
    }
    assert_eq!(ink_count(&mut e), n0);
    let xml = slide_xml(&save(&e, "ink-auto.pptx"));
    assert_eq!(xml.matches("<a:tailEnd").count(), 1);
    // Unrecognised strokes stay ink.
    let scribble: Vec<(f64, f64)> = (0..30)
        .map(|i| (400.0 + f64::from(i) * 4.0, 100.0 + f64::from(i % 5) * 9.0))
        .collect();
    let r = add(&mut e, &scribble, r#","autoShape":true"#);
    assert!(r["ink"]["converted"].is_null());
    assert_eq!(ink_count(&mut e), n0 + 1);
    // Undo steps: the scribble, the first head (the second one only went into
    // it and changed nothing), then the line.
    for _ in 0..2 {
        run(&mut e, r#"{"cmd":"undo"}"#);
    }
    let xml = slide_xml(&e.save_pptx().unwrap());
    assert!(xml.contains(r#"<a:prstGeom prst="line">"#) && !xml.contains("<a:tailEnd"));
    run(&mut e, r#"{"cmd":"undo"}"#);
    assert!(!slide_xml(&e.save_pptx().unwrap()).contains(r#"<a:prstGeom prst="line">"#));
}

#[test]
fn handwriting_becomes_text_or_an_equation() {
    let mut e = engine();
    let n0 = ink_count(&mut e);
    let a = add(&mut e, &seg((100.0, 100.0), (180.0, 140.0), 10), "");
    let id = a["ink"]["objects"].as_array().unwrap().last().unwrap()["id"]
        .as_u64()
        .unwrap();
    let r = run(
        &mut e,
        &format!(r#"{{"cmd":"inkToText","ids":[{id}],"text":"Xin chào"}}"#),
    );
    assert_eq!(r["ink"]["converted"]["shapes"][0]["kind"], "text");
    assert_eq!(ink_count(&mut e), n0);
    let xml = slide_xml(&save(&e, "ink-text.pptx"));
    assert!(xml.contains("<a:t>Xin chào</a:t>"));
    // 40 pt of ink → 30 pt text in the ink's colour.
    assert!(xml.contains(r#"sz="3000""#) && xml.contains(r#"<a:srgbClr val="1F5FBF"/>"#));

    add(&mut e, &seg((100.0, 300.0), (220.0, 340.0), 10), "");
    let r = run(
        &mut e,
        r#"{"cmd":"inkToMath","text":"\\frac{a}{b}","latex":true}"#,
    );
    assert_eq!(r["ink"]["converted"]["shapes"][0]["kind"], "math");
    let bytes = save(&e, "ink-math.pptx");
    let xml = slide_xml(&bytes);
    assert!(xml.contains("<a14:m>") && xml.contains("<m:f>"), "{xml}");
    // The fallback holds the linear form.
    assert!(xml.contains("<a:t>a/b</a:t>"));
    // Drawn as an equation.
    let mut back = Engine::with_bundled_fonts();
    back.open(&bytes, "m.pptx").unwrap();
    let page = back.page_json(0).unwrap();
    assert!(!page.contains("a/b"), "the fallback is not drawn");
    // UnicodeMath too; no text is refused.
    add(&mut e, &seg((300.0, 300.0), (360.0, 340.0), 10), "");
    run(&mut e, r#"{"cmd":"inkToMath","text":"x^2"}"#);
    add(&mut e, &seg((300.0, 100.0), (360.0, 140.0), 10), "");
    let out = e
        .slide_command_json(r#"{"cmd":"inkToText","text":"  "}"#)
        .unwrap();
    assert!(out.contains("\"error\""));
}
