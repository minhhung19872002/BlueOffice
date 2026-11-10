//! BlueSlide zooms (SLIDE-ZOOM-001): slide / section / summary zooms ([MS-PPTX] §2.8–§2.11)
//! inserted, saved as `mc:AlternateContent` (zoom choice + picture fallback), reopened with
//! their targets and options, previews rendered again when the target changes, unknown markup
//! kept, and the slide show links (target slides, return).
// Tests may unwrap: a panic is a test failure.
#![allow(clippy::unwrap_used)]

use base64::Engine as _;
use bo_engine::Engine;
use bo_opc::{Limits, Package};
use serde_json::{json, Value};
use std::path::PathBuf;

fn deck(name: &str) -> Vec<u8> {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus/slide")
        .join(name);
    std::fs::read(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

fn open_bytes(bytes: &[u8]) -> Engine {
    let mut e = Engine::with_bundled_fonts();
    e.open(bytes, "zoom.pptx").unwrap();
    e
}

fn run(e: &mut Engine, cmd: &Value) -> Value {
    let out = e.slide_command_json(&cmd.to_string()).unwrap();
    let r: Value = serde_json::from_str(&out).unwrap();
    assert!(r.get("error").is_none(), "{cmd}: {:?}", r.get("error"));
    r
}

/// Save; with `BO_ZOOM_OUT=<dir>` the file is also written there (OpenXmlValidator).
fn save(e: &Engine, name: &str) -> Vec<u8> {
    let bytes = e.save_pptx().unwrap();
    if let Ok(dir) = std::env::var("BO_ZOOM_OUT") {
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(PathBuf::from(dir).join(name), &bytes).unwrap();
    }
    bytes
}

fn part(bytes: &[u8], name: &str) -> String {
    let pkg = Package::read(bytes, &Limits::default()).unwrap();
    String::from_utf8(pkg.part(name).unwrap().data.clone()).unwrap()
}

fn links(e: &mut Engine, slide: usize) -> Vec<Value> {
    let r = run(e, &json!({ "cmd": "show", "slide": slide }));
    r["show"]["links"].as_array().cloned().unwrap_or_default()
}

fn zoom_links(e: &mut Engine, slide: usize) -> Vec<Value> {
    links(e, slide)
        .into_iter()
        .filter(|l| l["action"]["kind"] == "zoom")
        .collect()
}

#[test]
fn slide_zoom_round_trips_with_options_and_show_link() {
    let mut e = open_bytes(&deck("slide-edit-text.pptx"));
    run(&mut e, &json!({ "cmd": "goTo", "slide": 0 }));
    let r = run(&mut e, &json!({ "cmd": "zoomInsertSlides", "slides": [2] }));
    assert_eq!(r["selection"][0]["kind"], "zoom");
    let z = &r["zoom"];
    assert_eq!(z["kind"], "slide");
    assert_eq!(z["objects"][0]["slides"], json!([2]));
    assert_eq!(z["objects"][0]["returnToParent"], true);
    assert_eq!(z["objects"][0]["cover"], false);
    assert_eq!(z["objects"][0]["transitionMs"], Value::Null);

    // Options: no return, zoom transition 1.5 s, no zoom background.
    let r = run(
        &mut e,
        &json!({ "cmd": "zoomOptions", "returnToParent": false, "transition": true,
                 "durationMs": 1500, "showBg": false }),
    );
    let o = &r["zoom"]["objects"][0];
    assert_eq!(o["returnToParent"], false);
    assert_eq!(o["transitionMs"], 1500);
    assert_eq!(o["showBg"], false);

    let l = zoom_links(&mut e, 0);
    assert_eq!(l.len(), 1);
    assert_eq!(l[0]["action"]["slides"], json!([2]));
    assert_eq!(l[0]["action"]["return"], false);
    assert_eq!(l[0]["action"]["transitionMs"], 1500);

    let bytes = save(&e, "slide-zoom-slide.pptx");
    let xml = part(&bytes, "/ppt/slides/slide1.xml");
    assert!(xml.contains("<mc:AlternateContent"), "{xml}");
    assert!(xml.contains(r#"Requires="pslz""#));
    assert!(xml.contains(
        r#"<a:graphicData uri="http://schemas.microsoft.com/office/powerpoint/2016/slidezoom">"#
    ));
    assert!(xml.contains("<pslz:sldZmObj sldId="));
    assert!(xml.contains(r#"returnToParent="0""#));
    assert!(xml.contains(r#"transitionDur="1500""#));
    assert!(xml.contains(r#"showBg="0""#));
    assert!(xml.contains("ppaction://hlinksldjump"));
    let rels = part(&bytes, "/ppt/slides/_rels/slide1.xml.rels");
    assert!(rels.contains("relationships/slide\""), "{rels}");

    // Reopened: same zoom, same link; saved again byte for byte.
    let mut e2 = open_bytes(&bytes);
    let l = zoom_links(&mut e2, 0);
    assert_eq!(l.len(), 1);
    assert_eq!(l[0]["action"]["slides"], json!([2]));
    let again = e2.save_pptx().unwrap();
    assert_eq!(part(&again, "/ppt/slides/slide1.xml"), xml);

    // Undo the options: return is back.
    run(&mut e, &json!({ "cmd": "undo" }));
    let l = zoom_links(&mut e, 0);
    assert_eq!(l[0]["action"]["return"], true);
}

#[test]
fn summary_zoom_creates_slide_sections_and_links() {
    let mut e = open_bytes(&deck("slide-edit-text.pptx"));
    run(&mut e, &json!({ "cmd": "goTo", "slide": 0 }));
    let r = run(
        &mut e,
        &json!({ "cmd": "zoomInsertSummary", "slides": [1, 3],
                 "names": { "title": "Tổng quan", "summary": "Phần tổng quan",
                            "section": "Phần {n}", "first": "Mở đầu" } }),
    );
    // A new summary slide at index 1.
    assert_eq!(r["info"]["pageCount"], 5);
    assert_eq!(r["slide"], 1);
    let z = &r["zoom"];
    assert_eq!(z["kind"], "summary");
    let objs = z["objects"].as_array().unwrap();
    assert_eq!(objs.len(), 2);
    // Sections: Mở đầu [0], summary [1], [2, 3], [4].
    assert_eq!(objs[0]["slides"], json!([2, 3]));
    assert_eq!(objs[1]["slides"], json!([4]));
    let secs = r["sections"].as_array().unwrap();
    let names: Vec<&str> = secs.iter().map(|s| s["name"].as_str().unwrap()).collect();
    assert_eq!(names[0], "Mở đầu");
    assert_eq!(names[1], "Phần tổng quan");
    assert_eq!(secs.len(), 4);
    assert_eq!(secs[1]["count"], 1);

    // Two links over the summary slide, each its own box, coming back.
    let l = zoom_links(&mut e, 1);
    assert_eq!(l.len(), 2);
    assert_eq!(l[0]["action"]["slides"], json!([2, 3]));
    assert_eq!(l[0]["action"]["return"], true);
    let (a, b) = (&l[0]["rect"], &l[1]["rect"]);
    assert!(a[0].as_f64().unwrap() + a[2].as_f64().unwrap() <= b[0].as_f64().unwrap() + 0.01);

    let bytes = save(&e, "slide-zoom-summary.pptx");
    let xml = part(&bytes, "/ppt/slides/slide5.xml");
    assert!(xml.contains("<psuz:summaryZm>"), "{xml}");
    assert!(xml.contains("<psuz:gridLayout/>"));
    assert!(xml.contains("<p:grpSp>"));
    let pres = part(&bytes, "/ppt/presentation.xml");
    assert!(pres.contains("Phần tổng quan"));

    // Undo: the slide and sections go away in one step.
    let r = run(&mut e, &json!({ "cmd": "undo" }));
    assert_eq!(r["sections"].as_array().map_or(0, Vec::len), 0);
    assert_eq!(r["info"]["pageCount"], 4);

    // Reopened: the same two links.
    let mut e2 = open_bytes(&bytes);
    let l = zoom_links(&mut e2, 1);
    assert_eq!(l.len(), 2);
    assert_eq!(l[1]["action"]["slides"], json!([4]));
}

#[test]
fn section_zoom_and_preview_follows_target() {
    let mut e = open_bytes(&deck("slide-edit-text.pptx"));
    run(
        &mut e,
        &json!({ "cmd": "addSection", "name": "B", "slide": 2, "firstName": "A" }),
    );
    run(&mut e, &json!({ "cmd": "goTo", "slide": 0 }));
    let r = run(
        &mut e,
        &json!({ "cmd": "zoomInsertSections", "sections": [1] }),
    );
    let id = r["selection"][0]["id"].clone();
    assert_eq!(r["zoom"]["kind"], "section");
    assert_eq!(r["zoom"]["objects"][0]["slides"], json!([2, 3]));
    assert_eq!(r["zoom"]["objects"][0]["section"], "B");
    let bytes = save(&e, "slide-zoom-section.pptx");
    let image_of = |bytes: &[u8]| -> Vec<u8> {
        let xml = part(bytes, "/ppt/slides/slide1.xml");
        let rels = part(bytes, "/ppt/slides/_rels/slide1.xml.rels");
        let at = xml
            .find("<p166:blip")
            .or_else(|| xml.find("p166:blipFill"))
            .unwrap();
        let rid = &xml[at..];
        let s = rid.find("r:embed=\"").unwrap() + 9;
        let rid = &rid[s..s + rid[s..].find('"').unwrap()];
        let r = rels.find(&format!("Id=\"{rid}\"")).unwrap();
        let t = rels[r..].find("Target=\"").unwrap() + r + 8;
        let target = &rels[t..t + rels[t..].find('"').unwrap()];
        let name = format!("/ppt/media/{}", target.rsplit('/').next().unwrap());
        let pkg = Package::read(bytes, &Limits::default()).unwrap();
        pkg.part(&name).unwrap().data.clone()
    };
    let before = image_of(&bytes);
    assert!(before.starts_with(b"\x89PNG"));

    // Change the section's first slide: the preview is rendered again.
    run(&mut e, &json!({ "cmd": "goTo", "slide": 2 }));
    run(
        &mut e,
        &json!({ "cmd": "insertShape", "preset": "rect",
                 "rect": { "x": 50, "y": 50, "w": 300, "h": 200 } }),
    );
    let after = image_of(&e.save_pptx().unwrap());
    assert_ne!(before, after);
    // One undo removes the shape and puts the old preview back.
    run(&mut e, &json!({ "cmd": "undo" }));
    assert_eq!(image_of(&e.save_pptx().unwrap()), before);

    // A custom image, then back to the preview.
    run(&mut e, &json!({ "cmd": "goTo", "slide": 0 }));
    run(&mut e, &json!({ "cmd": "select", "ids": [id] }));
    let png = e.render_png(1, 0.1).unwrap();
    let data = base64::engine::general_purpose::STANDARD.encode(&png);
    let r = run(&mut e, &json!({ "cmd": "zoomImage", "data": data }));
    assert_eq!(r["zoom"]["objects"][0]["cover"], true);
    let saved = e.save_pptx().unwrap();
    assert_eq!(image_of(&saved), png);
    assert!(part(&saved, "/ppt/slides/slide1.xml").contains(r#"imageType="cover""#));
    let r = run(&mut e, &json!({ "cmd": "zoomResetImage" }));
    assert_eq!(r["zoom"]["objects"][0]["cover"], false);
    assert_eq!(image_of(&e.save_pptx().unwrap()), before);

    // Reopened: opening changes nothing, the first edit of the target renders the preview.
    let mut e2 = open_bytes(&bytes);
    run(&mut e2, &json!({ "cmd": "state" }));
    assert_eq!(image_of(&e2.save_pptx().unwrap()), before);
    run(&mut e2, &json!({ "cmd": "goTo", "slide": 2 }));
    run(
        &mut e2,
        &json!({ "cmd": "insertShape", "preset": "ellipse",
                 "rect": { "x": 50, "y": 50, "w": 300, "h": 200 } }),
    );
    assert_ne!(image_of(&e2.save_pptx().unwrap()), before);
}

/// Hand-written zoom markup (another producer's prefixes, an extLst, a fixed summary
/// layout) reads, renders and writes back unchanged; moving it moves the fallback too.
#[test]
fn foreign_markup_is_kept() {
    let mut e = open_bytes(&deck("slide-edit-text.pptx"));
    run(&mut e, &json!({ "cmd": "goTo", "slide": 0 }));
    let r = run(
        &mut e,
        &json!({ "cmd": "zoomInsertSlides", "slides": [1, 2] }),
    );
    let first = r["selection"][0]["id"].clone();
    let bytes = save(&e, "slide-zoom-two.pptx");
    let xml = part(&bytes, "/ppt/slides/slide1.xml");
    assert_eq!(xml.matches("<pslz:sldZm>").count(), 2);
    let mut e2 = open_bytes(&bytes);
    let l = zoom_links(&mut e2, 0);
    assert_eq!(l.len(), 2);
    assert_eq!(l[0]["action"]["slides"], json!([1]));
    assert_eq!(l[1]["action"]["slides"], json!([2]));
    // Moving one zoom: its fallback picture follows.
    run(&mut e2, &json!({ "cmd": "goTo", "slide": 0 }));
    run(&mut e2, &json!({ "cmd": "select", "ids": [first] }));
    run(
        &mut e2,
        &json!({ "cmd": "transform", "transform": { "kind": "move", "dx": 10, "dy": 20 } }),
    );
    let moved = part(&e2.save_pptx().unwrap(), "/ppt/slides/slide1.xml");
    assert_ne!(moved, xml);
    assert_eq!(moved.matches("<pslz:sldZm>").count(), 2);
}
