//! BlueSlide ink (CORE-INK-001): strokes added and erased through the JSON
//! commands, saved as InkML content parts ([MS-ODRAWXML] §2.1.4) and reopened;
//! a hand-written deck with an ink content part is drawn, edited and kept.
// Tests may unwrap: a panic is a test failure.
#![allow(clippy::unwrap_used)]

use bo_engine::Engine;
use bo_opc::{Limits, Package, Relationship, TargetMode};
use std::path::PathBuf;

const CM: f64 = 72.0 / 2.54;

fn deck(name: &str) -> Vec<u8> {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus/slide")
        .join(name);
    std::fs::read(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

fn open_bytes(bytes: &[u8]) -> Engine {
    let mut e = Engine::with_bundled_fonts();
    e.open(bytes, "ink.pptx").unwrap();
    e
}

fn run(e: &mut Engine, json: &str) -> serde_json::Value {
    let out = e.slide_command_json(json).unwrap();
    let r: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert!(r.get("error").is_none(), "{json}: {:?}", r.get("error"));
    r
}

/// The `ink` answer as JSON.
fn ink(e: &mut Engine, json: &str) -> serde_json::Value {
    let out = e.slide_command_json(json).unwrap();
    let r: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert!(r.get("error").is_none(), "{json}: {:?}", r.get("error"));
    r["ink"].clone()
}

/// Save; with `BO_INK_OUT=<dir>` the file is also written there (OpenXmlValidator runs).
fn save(e: &Engine, name: &str) -> Vec<u8> {
    let bytes = e.save_pptx().unwrap();
    if let Ok(dir) = std::env::var("BO_INK_OUT") {
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(PathBuf::from(dir).join(name), &bytes).unwrap();
    }
    bytes
}

fn close(a: f64, b: f64, tol: f64) -> bool {
    (a - b).abs() <= tol
}

fn num(v: &serde_json::Value) -> f64 {
    v.as_f64().unwrap()
}

#[test]
fn drawn_strokes_are_ink_parts() {
    let mut e = open_bytes(&deck("slide-edit-text.pptx"));
    run(&mut e, r#"{"cmd":"goTo","slide":0}"#);
    let before = ink(&mut e, r#"{"cmd":"inkInfo"}"#);
    let n0 = before["objects"].as_array().unwrap().len();
    // 11 points 10 pt apart along y = 100, half pressure, 10 ms apart.
    let pts: Vec<String> = (0..=10)
        .map(|i| format!("[{},100]", 100 + 10 * i))
        .collect();
    let f: Vec<&str> = (0..=10).map(|_| "0.5").collect();
    let t: Vec<String> = (0..=10).map(|i| (10 * i).to_string()).collect();
    let add = format!(
        r#"{{"cmd":"addInk","stroke":{{"points":[{}],"pressure":[{}],"times":[{}],"color":"1f77b4","width":4}}}}"#,
        pts.join(","),
        f.join(","),
        t.join(",")
    );
    let r = ink(&mut e, &add);
    let objs = r["objects"].as_array().unwrap();
    assert_eq!(objs.len(), n0 + 1);
    let o = &objs[n0];
    // The frame is the ink's box: samples 100..200 × 100 plus the 2 pt margin
    // (half the 4 pt width); 1 pt = 2540/72 raw units, rounded to 0.014 pt.
    let rect = o["rect"].as_array().unwrap();
    assert!(close(num(&rect[0]), 98.0, 0.02) && close(num(&rect[1]), 98.0, 0.02));
    assert!(close(num(&rect[2]), 104.0, 0.03) && close(num(&rect[3]), 4.0, 0.03));
    let s = &o["strokes"][0];
    assert_eq!(s["color"], "1F77B4");
    assert_eq!(s["points"].as_array().unwrap().len(), 11);
    // Width 4 · (0.5 + 16384/32767) ≈ 4.0.
    assert!(close(num(&s["points"][0][2]), 4.0, 1e-3));
    assert_eq!(s["times"][10], 100.0);
    // A second stroke gets its own id.
    let r2 = ink(
        &mut e,
        r#"{"cmd":"addInk","stroke":{"points":[[50,300],[90,340]],"color":"FFE600","width":12,"highlighter":true}}"#,
    );
    let objs2 = r2["objects"].as_array().unwrap();
    assert_eq!(objs2.len(), n0 + 2);
    assert_ne!(objs2[n0]["id"], objs2[n0 + 1]["id"]);
    assert_eq!(objs2[n0 + 1]["strokes"][0]["highlighter"], true);
    assert!(close(
        num(&objs2[n0 + 1]["strokes"][0]["opacity"]),
        0.5,
        1e-9
    ));
    // Each stroke is one undo step.
    run(&mut e, r#"{"cmd":"undo"}"#);
    assert_eq!(
        ink(&mut e, r#"{"cmd":"inkInfo"}"#)["objects"]
            .as_array()
            .unwrap()
            .len(),
        n0 + 1
    );
    run(&mut e, r#"{"cmd":"redo"}"#);

    // Saved: an inkml part with its content type and a customXml relationship.
    let bytes = save(&e, "ink-drawn.pptx");
    let pkg = Package::read(&bytes, &Limits::default()).unwrap();
    let ct = pkg.content_types().unwrap();
    let ink_parts: Vec<String> = pkg
        .parts()
        .iter()
        .filter(|p| p.name.starts_with("/ppt/ink/"))
        .map(|p| p.name.clone())
        .collect();
    assert_eq!(ink_parts.len(), 2);
    for p in &ink_parts {
        assert_eq!(ct.get(p), Some("application/inkml+xml"));
    }
    let slide_part = "/ppt/slides/slide1.xml";
    let rels = pkg.relationships(slide_part).unwrap();
    assert_eq!(
        rels.iter()
            .filter(|r| r.rel_type
                == "http://schemas.openxmlformats.org/officeDocument/2006/relationships/customXml")
            .count(),
        2
    );
    let xml = String::from_utf8(pkg.part(slide_part).unwrap().data.clone()).unwrap();
    assert!(xml.contains("<p:contentPart"));
    assert!(xml.contains(r#"Requires="p14""#));
    assert!(xml.contains("<mc:Fallback><p:sp>"));

    // Reopened: the same ink.
    let mut back = open_bytes(&bytes);
    run(&mut back, r#"{"cmd":"goTo","slide":0}"#);
    let again = ink(&mut back, r#"{"cmd":"inkInfo"}"#);
    assert_eq!(again["objects"], r2["objects"]);

    // Point eraser across x = 150 (radius 3 + half width 2): the sample at 150
    // goes, the stroke splits into 100..140 and 160..200 — one undo step.
    let r = ink(
        &mut back,
        r#"{"cmd":"eraseInk","points":[[150,90],[150,110]],"radius":3,"mode":"point"}"#,
    );
    let st = r["objects"][n0]["strokes"].as_array().unwrap();
    assert_eq!(st.len(), 2);
    assert_eq!(st[0]["points"].as_array().unwrap().len(), 5);
    assert_eq!(st[1]["points"].as_array().unwrap().len(), 5);
    // The ink stays in place.
    assert!(close(num(&st[1]["points"][0][0]), 160.0, 0.02));
    run(&mut back, r#"{"cmd":"undo"}"#);
    let un = ink(&mut back, r#"{"cmd":"inkInfo"}"#);
    assert_eq!(un["objects"], r2["objects"]);

    // Stroke eraser: the touched stroke and its object, part and relationship go.
    let r = ink(
        &mut back,
        r#"{"cmd":"eraseInk","points":[[70,320]],"radius":2}"#,
    );
    assert_eq!(r["objects"].as_array().unwrap().len(), n0 + 1);
    let bytes = save(&back, "ink-erased.pptx");
    let pkg = Package::read(&bytes, &Limits::default()).unwrap();
    assert_eq!(
        pkg.parts()
            .iter()
            .filter(|p| p.name.starts_with("/ppt/ink/"))
            .count(),
        1
    );
}

/// Hand-written InkML (the bo-drawingml fixture's trace layout): pen 0.1 cm red,
/// highlighter 2 × 4 mm yellow with transparency 51, an EMMA annotation.
const INK: &str = r##"<?xml version="1.0" encoding="UTF-8"?>
<inkml:ink xmlns:inkml="http://www.w3.org/2003/InkML" xmlns:emma="http://www.w3.org/2003/04/emma">
<inkml:definitions>
<inkml:context xml:id="ctx0"><inkml:inkSource xml:id="src0"><inkml:traceFormat>
<inkml:channel name="X" type="integer" units="cm"/><inkml:channel name="Y" type="integer" units="cm"/>
<inkml:channel name="F" type="integer" max="1023" units="dev"/><inkml:channel name="T" type="integer" units="dev"/>
</inkml:traceFormat><inkml:channelProperties>
<inkml:channelProperty channel="X" name="resolution" value="1000" units="1/cm"/>
<inkml:channelProperty channel="Y" name="resolution" value="1000" units="1/cm"/>
</inkml:channelProperties></inkml:inkSource></inkml:context>
<inkml:brush xml:id="pen"><inkml:brushProperty name="width" value="0.1" units="cm"/><inkml:brushProperty name="height" value="0.1" units="cm"/><inkml:brushProperty name="color" value="#FF0000"/></inkml:brush>
<inkml:brush xml:id="hl"><inkml:brushProperty name="width" value="2" units="mm"/><inkml:brushProperty name="height" value="4" units="mm"/><inkml:brushProperty name="color" value="#FFFF00"/><inkml:brushProperty name="tip" value="rectangle"/><inkml:brushProperty name="rasterOp" value="maskPen"/><inkml:brushProperty name="transparency" value="51"/></inkml:brush>
</inkml:definitions>
<inkml:trace xml:id="t1" contextRef="#ctx0" brushRef="#pen">1000 2000 0 0, '254 '0 1023 10, "0 '508 '0 '10, 0 0 0 0</inkml:trace>
<inkml:traceGroup brushRef="#hl" contextRef="#ctx0">
<inkml:annotationXML><emma:emma version="1.0"><emma:interpretation id="{00000000-0000-0000-0000-000000000000}" emma:mode="ink"/></emma:emma></inkml:annotationXML>
<inkml:trace xml:id="t2">0 0 512 100,1000-500 512 110</inkml:trace>
</inkml:traceGroup>
</inkml:ink>"##;

/// The deck with the ink placed at (1 cm, 1 cm) in its natural size: samples
/// x 0..1.762 cm, y −0.5..2.508 cm plus margins 0.1 cm (half the widest brush
/// width, 2 mm) and 0.2 cm (half the tallest, 4 mm): 1.962 × 3.408 cm =
/// 706 320 × 1 226 880 EMU.
fn deck_with_ink() -> Vec<u8> {
    let mut pkg = Package::read(&deck("slide-edit-text.pptx"), &Limits::default()).unwrap();
    let slide = "/ppt/slides/slide1.xml";
    pkg.add_part(
        "/ppt/ink/ink1.xml",
        "application/inkml+xml",
        INK.as_bytes().to_vec(),
    )
    .unwrap();
    let mut rels = pkg.relationships(slide).unwrap();
    rels.push(Relationship {
        id: "rIdInk1".into(),
        rel_type: "http://schemas.openxmlformats.org/officeDocument/2006/relationships/customXml"
            .into(),
        target: "../ink/ink1.xml".into(),
        mode: TargetMode::Internal,
    });
    pkg.set_relationships(slide, &rels);
    let ac = concat!(
        r#"<mc:AlternateContent xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006">"#,
        r#"<mc:Choice xmlns:p14="http://schemas.microsoft.com/office/powerpoint/2010/main" Requires="p14">"#,
        r#"<p:contentPart p14:bwMode="auto" r:id="rIdInk1"><p14:nvContentPartPr><p14:cNvPr id="900" name="Hand ink"/>"#,
        r#"<p14:cNvContentPartPr/><p14:nvPr/></p14:nvContentPartPr><p14:xfrm><a:off x="360000" y="360000"/>"#,
        r#"<a:ext cx="706320" cy="1226880"/></p14:xfrm></p:contentPart></mc:Choice>"#,
        r#"<mc:Fallback><p:sp><p:nvSpPr><p:cNvPr id="900" name="Hand ink"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr>"#,
        r#"<p:spPr><a:xfrm><a:off x="360000" y="360000"/><a:ext cx="706320" cy="1226880"/></a:xfrm>"#,
        r#"<a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr></p:sp></mc:Fallback></mc:AlternateContent>"#
    );
    let xml = String::from_utf8(pkg.part(slide).unwrap().data.clone()).unwrap();
    let xml = xml.replacen("</p:spTree>", &format!("{ac}</p:spTree>"), 1);
    pkg.put_part(slide, xml.into_bytes());
    pkg.write().unwrap()
}

#[test]
fn ink_from_a_file_is_drawn_edited_and_kept() {
    let bytes = deck_with_ink();
    let mut e = open_bytes(&bytes);
    run(&mut e, r#"{"cmd":"goTo","slide":0}"#);
    let r = ink(&mut e, r#"{"cmd":"inkInfo","backdrop":true}"#);
    let objs = r["objects"].as_array().unwrap();
    assert_eq!(objs.len(), 1);
    assert_eq!(objs[0]["id"], 900);
    assert_eq!(objs[0]["name"], "Hand ink");
    let st = objs[0]["strokes"].as_array().unwrap();
    assert_eq!(st.len(), 2);
    // First sample (1 cm, 2 cm) → frame (1 − 0 + 0.1, 2 + 0.5 + 0.2) cm, plus the
    // frame origin (1 cm, 1 cm): (2.1, 3.7) cm. Pressure 0 → half the 0.1 cm width.
    let p0 = &st[0]["points"][0];
    assert!(close(num(&p0[0]), 2.1 * CM, 1e-6) && close(num(&p0[1]), 3.7 * CM, 1e-6));
    assert!(close(num(&p0[2]), 0.05 * CM, 1e-6));
    assert_eq!(st[0]["times"][2], 20.0);
    assert_eq!(st[1]["highlighter"], true);
    assert_eq!(st[1]["rectTip"], true);
    assert!(close(num(&st[1]["opacity"]), 0.4, 1e-9));
    // The backdrop leaves the ink out; the slide itself draws it as two filled paths.
    let backdrop = r["backdrop"]["commands"].as_array().unwrap().len();
    let page: serde_json::Value = serde_json::from_str(&e.slide_json(0).unwrap()).unwrap();
    let fills: Vec<serde_json::Value> = page["commands"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["op"] == "path" && c["fill"]["type"] == "solid")
        .map(|c| c["fill"]["color"].clone())
        .collect();
    assert!(fills.contains(&serde_json::json!([255, 0, 0, 255])));
    // 0.4 · 255 = 102.
    assert!(fills.contains(&serde_json::json!([255, 255, 0, 102])));
    assert_eq!(
        page["commands"].as_array().unwrap().len(),
        backdrop + 2,
        "the fallback is not drawn"
    );

    // Saved without edits: the ink part is kept byte for byte.
    let saved = save(&e, "ink-file.pptx");
    let pkg = Package::read(&saved, &Limits::default()).unwrap();
    assert_eq!(pkg.part("/ppt/ink/ink1.xml").unwrap().data, INK.as_bytes());

    // Erase the highlighter stroke (through its second sample at (2.0, 1.2) cm
    // + frame offset): the pen trace and the EMMA annotation stay.
    let x = (1.0 + 0.1 + 1.0) * CM;
    let y = (-0.5 + 0.5 + 0.2 + 1.0) * CM;
    let r = ink(
        &mut e,
        &format!(r#"{{"cmd":"eraseInk","points":[[{x},{y}]],"radius":1}}"#),
    );
    let st = r["objects"][0]["strokes"].as_array().unwrap();
    assert_eq!(st.len(), 1);
    assert_eq!(st[0]["color"], "FF0000");
    // The pen ink did not move.
    assert!(close(num(&st[0]["points"][0][0]), 2.1 * CM, 1e-3));
    assert!(close(num(&st[0]["points"][0][1]), 3.7 * CM, 1e-3));
    let saved = save(&e, "ink-file-edited.pptx");
    let pkg = Package::read(&saved, &Limits::default()).unwrap();
    let part = String::from_utf8(pkg.part("/ppt/ink/ink1.xml").unwrap().data.clone()).unwrap();
    assert!(part.contains("emma:interpretation"));
    assert!(part.contains(r##"<inkml:trace xml:id="t1" contextRef="#ctx0" brushRef="#pen">"##));
    assert!(!part.contains(r#"xml:id="t2""#));
}
