//! BlueSlide views and sections (SLIDE-VIEW-001) through the JSON bridge:
//! outline ↔ slides, notes page geometry from the notes master, notes and
//! handout master views, sections; undo and lossless saving.
//!
//! With `BO_VIEWS_OUT=<dir>` the written files are kept there (for
//! OpenXmlValidator: `bo-cli validate <dir> --openxml require`).
// Tests may unwrap: a panic is a test failure.
#![allow(clippy::unwrap_used)]

use bo_engine::Engine;
use bo_model::slide::{Presentation, ShapeNode};
use bo_pptx::build::{
    build_pptx, DeckSpec, LayoutKind, ParaSpec, Placeholder, ShapeSpec, SlideSpec,
};
use serde_json::{json, Value};
use std::path::PathBuf;

fn keep(name: &str, bytes: &[u8]) {
    if let Ok(dir) = std::env::var("BO_VIEWS_OUT") {
        let _ = std::fs::create_dir_all(&dir);
        std::fs::write(PathBuf::from(dir).join(name), bytes).unwrap();
    }
}

fn shape(name: &str, p: Placeholder, paras: Vec<ParaSpec>) -> ShapeSpec {
    ShapeSpec {
        name: name.into(),
        x: 838_200,
        y: 365_125,
        cx: 10_515_600,
        cy: 1_325_563,
        placeholder: Some(p),
        text: paras,
        ..ShapeSpec::default()
    }
}

fn para(text: &str, level: u8) -> ParaSpec {
    ParaSpec {
        level,
        ..ParaSpec::plain(text)
    }
}

/// 16:9 deck: title slide; "Kế hoạch" with three bullets (levels 0, 1, 1) and
/// notes; "Kết luận" with one bullet. The notes master of `build_pptx` puts the
/// slide image at (685 800, 1 143 000) 5 486 400 × 3 086 100 EMU and the notes
/// body at (685 800, 4 400 550) 5 486 400 × 3 600 450 EMU on a 6 858 000 ×
/// 9 144 000 EMU notes page.
fn deck_bytes() -> Vec<u8> {
    build_pptx(&DeckSpec {
        width: 12_192_000,
        height: 6_858_000,
        title: None,
        slides: vec![
            SlideSpec {
                layout: LayoutKind::Title,
                shapes: vec![
                    shape("Title", Placeholder::CenterTitle, vec![para("Mở đầu", 0)]),
                    shape("Sub", Placeholder::SubTitle, vec![para("Phụ đề", 0)]),
                ],
                ..SlideSpec::default()
            },
            SlideSpec {
                layout: LayoutKind::TitleContent,
                shapes: vec![
                    shape("Title", Placeholder::Title, vec![para("Kế hoạch", 0)]),
                    shape(
                        "Body",
                        Placeholder::Body,
                        vec![
                            para("Mục tiêu", 0),
                            para("Doanh thu", 1),
                            para("Chi phí", 1),
                        ],
                    ),
                ],
                notes: vec!["Ghi chú hai".into()],
                ..SlideSpec::default()
            },
            SlideSpec {
                layout: LayoutKind::TitleContent,
                shapes: vec![
                    shape("Title", Placeholder::Title, vec![para("Kết luận", 0)]),
                    shape("Body", Placeholder::Body, vec![para("Cảm ơn", 0)]),
                ],
                ..SlideSpec::default()
            },
        ],
    })
    .unwrap()
}

fn open_bytes(b: &[u8]) -> Engine {
    let mut e = Engine::with_bundled_fonts();
    e.open(b, "deck.pptx").unwrap();
    e
}

fn open() -> Engine {
    open_bytes(&deck_bytes())
}

fn run(e: &mut Engine, v: &Value) -> Value {
    let s = e.slide_command_json(&v.to_string()).unwrap();
    let r: Value = serde_json::from_str(&s).unwrap();
    assert!(r.get("error").is_none(), "{v}: {:?}", r.get("error"));
    r
}

fn error_of(e: &mut Engine, v: &Value) -> Option<String> {
    let s = e.slide_command_json(&v.to_string()).unwrap();
    let r: Value = serde_json::from_str(&s).unwrap();
    r.get("error").and_then(Value::as_str).map(str::to_owned)
}

fn pres(e: &Engine) -> &Presentation {
    e.presentation().unwrap()
}

fn part(bytes: &[u8], name: &str) -> Option<Vec<u8>> {
    let pkg = bo_opc::Package::read(bytes, &bo_opc::Limits::default()).unwrap();
    pkg.part(name).map(|p| p.data.clone())
}

fn part_text(bytes: &[u8], name: &str) -> String {
    String::from_utf8(part(bytes, name).unwrap_or_default()).unwrap()
}

/// (title, [(text, level)]) of every slide.
fn outline(e: &mut Engine) -> Vec<(String, Vec<(String, u64)>)> {
    let r = run(e, &json!({ "cmd": "outline" }));
    r["outline"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| {
            (
                s["title"].as_str().unwrap().to_owned(),
                s["bullets"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|b| {
                        (
                            b["text"].as_str().unwrap().to_owned(),
                            b["level"].as_u64().unwrap(),
                        )
                    })
                    .collect(),
            )
        })
        .collect()
}

fn o(title: &str, bullets: &[(&str, u64)]) -> (String, Vec<(String, u64)>) {
    (
        title.to_owned(),
        bullets.iter().map(|(t, l)| ((*t).to_owned(), *l)).collect(),
    )
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 0.01
}

// ---------------------------------------------------------------------------
// Outline
// ---------------------------------------------------------------------------

#[test]
fn outline_lists_titles_and_body_levels() {
    let mut e = open();
    assert_eq!(
        outline(&mut e),
        vec![
            o("Mở đầu", &[("Phụ đề", 0)]),
            o(
                "Kế hoạch",
                &[("Mục tiêu", 0), ("Doanh thu", 1), ("Chi phí", 1)]
            ),
            o("Kết luận", &[("Cảm ơn", 0)]),
        ]
    );
}

#[test]
fn promote_bullet_to_title_makes_a_new_slide_in_one_undo_step() {
    let mut e = open();
    // Tab on "Doanh thu" (level 1 → 2); Shift+Tab twice on "Chi phí"
    // (level 1 → 0 → title): a new slide after "Kế hoạch".
    let r = run(
        &mut e,
        &json!({ "cmd": "applyOutline", "current": 2, "slides": [
            { "slide": 0, "title": "Mở đầu", "bullets": [{ "text": "Phụ đề", "level": 0 }] },
            { "slide": 1, "title": "Kế hoạch", "bullets": [
                { "text": "Mục tiêu", "level": 0 }, { "text": "Doanh thu", "level": 2 }] },
            { "title": "Chi phí", "bullets": [] },
            { "slide": 2, "title": "Kết luận", "bullets": [{ "text": "Cảm ơn", "level": 0 }] },
        ]}),
    );
    assert_eq!(r["slide"], 2);
    assert_eq!(r["info"]["pageCount"], 4);
    assert_eq!(
        outline(&mut e),
        vec![
            o("Mở đầu", &[("Phụ đề", 0)]),
            o("Kế hoạch", &[("Mục tiêu", 0), ("Doanh thu", 2)]),
            // New slide on the content layout: its empty body is one empty bullet.
            o("Chi phí", &[("", 0)]),
            o("Kết luận", &[("Cảm ơn", 0)]),
        ]
    );
    // The new slide is on the "title and content" layout of slide 2.
    let p = pres(&e);
    assert_eq!(p.slides[2].layout, p.slides[1].layout);
    // §21.1.2.2.7 pPr@lvl = 2 on the demoted paragraph.
    let ShapeNode::Shape(body) = &p.slides[1].data.shapes[1] else {
        panic!("body")
    };
    let lv: Vec<Option<u8>> = body
        .text
        .as_ref()
        .unwrap()
        .paragraphs
        .iter()
        .map(|p| p.props.as_ref().and_then(|x| x.lvl))
        .collect();
    assert_eq!(lv[1], Some(2));
    assert_eq!(lv.len(), 2);
    // One undo step restores the deck.
    run(&mut e, &json!({ "cmd": "undo" }));
    assert_eq!(pres(&e).slides.len(), 3);
    assert_eq!(outline(&mut e)[1].1.len(), 3);
    run(&mut e, &json!({ "cmd": "redo" }));
    assert_eq!(outline(&mut e)[2].0, "Chi phí");
}

#[test]
fn demoting_a_title_merges_the_slide_into_the_previous_one() {
    let mut e = open();
    run(
        &mut e,
        &json!({ "cmd": "applyOutline", "slides": [
            { "slide": 0, "title": "Mở đầu", "bullets": [{ "text": "Phụ đề", "level": 0 }] },
            { "slide": 1, "title": "Kế hoạch", "bullets": [
                { "text": "Mục tiêu", "level": 0 }, { "text": "Doanh thu", "level": 1 },
                { "text": "Chi phí", "level": 1 }, { "text": "Kết luận", "level": 0 },
                { "text": "Cảm ơn", "level": 1 }] },
        ]}),
    );
    assert_eq!(
        outline(&mut e),
        vec![
            o("Mở đầu", &[("Phụ đề", 0)]),
            o(
                "Kế hoạch",
                &[
                    ("Mục tiêu", 0),
                    ("Doanh thu", 1),
                    ("Chi phí", 1),
                    ("Kết luận", 0),
                    ("Cảm ơn", 1)
                ]
            ),
        ]
    );
    // Existing slides cannot be reordered by the outline.
    let err = error_of(
        &mut e,
        &json!({ "cmd": "applyOutline", "slides": [
            { "slide": 1, "title": "B" }, { "slide": 0, "title": "A" }] }),
    );
    assert_eq!(err.as_deref(), Some("outlineOrder"));
}

#[test]
fn new_slide_from_outline_round_trips() {
    let mut e = open();
    let mut slides: Vec<Value> = outline(&mut e)
        .iter()
        .enumerate()
        .map(|(i, (t, b))| {
            json!({ "slide": i, "title": t, "bullets": b.iter()
                .map(|(x, l)| json!({ "text": x, "level": l })).collect::<Vec<_>>() })
        })
        .collect();
    // Ctrl+Enter at the end: a new slide with a title and two bullets.
    slides.push(json!({ "title": "Phụ lục", "bullets": [
        { "text": "Bảng số liệu", "level": 0 }, { "text": "Nguồn", "level": 1 }] }));
    // Edited title text of slide 1 keeps the run formatting around the change.
    slides[0]["title"] = json!("Mở đầu mới");
    run(&mut e, &json!({ "cmd": "applyOutline", "slides": slides }));
    let want = vec![
        o("Mở đầu mới", &[("Phụ đề", 0)]),
        o(
            "Kế hoạch",
            &[("Mục tiêu", 0), ("Doanh thu", 1), ("Chi phí", 1)],
        ),
        o("Kết luận", &[("Cảm ơn", 0)]),
        o("Phụ lục", &[("Bảng số liệu", 0), ("Nguồn", 1)]),
    ];
    assert_eq!(outline(&mut e), want);
    let bytes = e.save_pptx().unwrap();
    keep("views-outline.pptx", &bytes);
    let mut again = open_bytes(&bytes);
    assert_eq!(outline(&mut again), want);
}

// ---------------------------------------------------------------------------
// Notes page
// ---------------------------------------------------------------------------

#[test]
fn notes_page_geometry_comes_from_the_notes_master() {
    let mut e = open();
    let r = run(&mut e, &json!({ "cmd": "notesPages" }));
    let pages = r["notesPages"].as_array().unwrap();
    assert_eq!(pages.len(), 3);
    let p = &pages[1];
    // notesSz 6 858 000 × 9 144 000 EMU = 540 × 720 pt (12 700 EMU per pt).
    assert!(close(p["width"].as_f64().unwrap(), 540.0));
    assert!(close(p["height"].as_f64().unwrap(), 720.0));
    // sldImg: 685 800 / 12 700 = 54, 1 143 000 / 12 700 = 90, 432 × 243 pt;
    // 432 / 243 = 16 / 9, the slide's aspect, so the slide fills the box.
    let img = &p["image"];
    assert!(close(img["x"].as_f64().unwrap(), 54.0));
    assert!(close(img["y"].as_f64().unwrap(), 90.0));
    assert!(close(img["w"].as_f64().unwrap(), 432.0));
    assert!(close(img["h"].as_f64().unwrap(), 243.0));
    // body: y = 4 400 550 / 12 700 = 346.5, h = 3 600 450 / 12 700 = 283.5.
    let body = &p["body"];
    assert!(close(body["x"].as_f64().unwrap(), 54.0));
    assert!(close(body["y"].as_f64().unwrap(), 346.5));
    assert!(close(body["w"].as_f64().unwrap(), 432.0));
    assert!(close(body["h"].as_f64().unwrap(), 283.5));
    // notesStyle lvl1pPr defRPr sz="1200" → 12 pt.
    assert!(close(p["size"].as_f64().unwrap(), 12.0));
    assert_eq!(p["text"], "Ghi chú hai");
    assert_eq!(pages[0]["text"], "");
    // Editing the notes (the notes page's text box) is the undoable setNotes.
    run(
        &mut e,
        &json!({ "cmd": "setNotes", "slide": 0, "text": "Lời mở đầu" }),
    );
    let r = run(&mut e, &json!({ "cmd": "notesPages" }));
    assert_eq!(r["notesPages"][0]["text"], "Lời mở đầu");
    run(&mut e, &json!({ "cmd": "undo" }));
    let r = run(&mut e, &json!({ "cmd": "notesPages" }));
    assert_eq!(r["notesPages"][0]["text"], "");
}

// ---------------------------------------------------------------------------
// Notes master and handout master views
// ---------------------------------------------------------------------------

const NOTES_MASTER: &str = "/ppt/notesMasters/notesMaster1.xml";

#[test]
fn notes_master_view_edits_placeholders_and_folds_into_one_step() {
    let original = deck_bytes();
    let mut e = open_bytes(&original);
    // Open and close without changes: nothing to undo, the part is unchanged.
    run(
        &mut e,
        &json!({ "cmd": "pageMasterView", "kind": "notes", "on": true }),
    );
    run(&mut e, &json!({ "cmd": "pageMasterView", "on": false }));
    let r = run(&mut e, &json!({ "cmd": "state" }));
    assert_eq!(r["canUndo"], false);
    let saved = e.save_pptx().unwrap();
    assert_eq!(
        part(&saved, NOTES_MASTER),
        part(&original, NOTES_MASTER),
        "untouched notes master is byte-stable"
    );

    let r = run(
        &mut e,
        &json!({ "cmd": "pageMasterView", "kind": "notes", "on": true }),
    );
    let pm = &r["pageMaster"];
    assert_eq!(pm["kind"], "notes");
    assert_eq!(pm["slideImage"], true);
    assert_eq!(pm["body"], true);
    assert_eq!(pm["header"], false);
    assert_eq!(pm["portrait"], true);
    // One page at the notes size (540 × 720 pt).
    assert_eq!(r["info"]["pageCount"], 1);
    assert!(close(
        r["info"]["pages"][0]["width"].as_f64().unwrap(),
        540.0
    ));
    assert!(close(
        r["info"]["pages"][0]["height"].as_f64().unwrap(),
        720.0
    ));
    // Slide commands do not apply here.
    assert_eq!(
        error_of(&mut e, &json!({ "cmd": "addSlide" })).as_deref(),
        Some("masterView")
    );
    assert_eq!(
        error_of(&mut e, &json!({ "cmd": "masterView", "on": true })).as_deref(),
        Some("masterView")
    );
    let r = run(
        &mut e,
        &json!({ "cmd": "pageMasterElements", "header": true, "body": false }),
    );
    assert_eq!(r["pageMaster"]["header"], true);
    assert_eq!(r["pageMaster"]["body"], false);
    // The view has its own history.
    let r = run(&mut e, &json!({ "cmd": "undo" }));
    assert_eq!(r["pageMaster"]["header"], false);
    assert_eq!(r["pageMaster"]["body"], true);
    run(&mut e, &json!({ "cmd": "redo" }));
    // Saving while the view is open writes the real deck.
    let during = e.save_pptx().unwrap();
    assert_eq!(open_bytes(&during).slide_count(), 3);
    let r = run(&mut e, &json!({ "cmd": "pageMasterView", "on": false }));
    assert_eq!(r["info"]["pageCount"], 3);
    assert!(r.get("pageMaster").is_none());
    let bytes = e.save_pptx().unwrap();
    keep("views-notes-master.pptx", &bytes);
    let xml = part_text(&bytes, NOTES_MASTER);
    assert!(xml.contains(r#"type="hdr""#), "{xml}");
    assert!(!xml.contains(r#"type="body""#), "{xml}");
    assert!(xml.contains("notesStyle"), "{xml}");
    // The whole session is one undo step.
    run(&mut e, &json!({ "cmd": "undo" }));
    let back = e.save_pptx().unwrap();
    assert_eq!(part(&back, NOTES_MASTER), part(&original, NOTES_MASTER));
}

#[test]
fn handout_master_is_created_edited_and_round_trips() {
    let mut e = open();
    assert!(part(
        &e.save_pptx().unwrap(),
        "/ppt/handoutMasters/handoutMaster1.xml"
    )
    .is_none());
    let r = run(
        &mut e,
        &json!({ "cmd": "pageMasterView", "kind": "handout", "on": true }),
    );
    let pm = &r["pageMaster"];
    assert_eq!(pm["kind"], "handout");
    for k in ["header", "date", "footer", "number"] {
        assert_eq!(pm[k], true, "{k}");
    }
    assert_eq!(pm["perPage"], 6);
    assert_eq!(pm["slots"].as_array().unwrap().len(), 6);
    // One slide per page: 540 × 720 pt page, 36 / 54 pt margins → a 468 ×
    // 612 pt cell; a 16:9 slide 468 wide is 263.25 high, centred:
    // y = 54 + (612 − 263.25) / 2 = 228.375.
    let r = run(&mut e, &json!({ "cmd": "handoutSlides", "perPage": 1 }));
    let s = &r["pageMaster"]["slots"][0];
    assert!(close(s["x"].as_f64().unwrap(), 36.0));
    assert!(close(s["y"].as_f64().unwrap(), 228.375));
    assert!(close(s["w"].as_f64().unwrap(), 468.0));
    assert!(close(s["h"].as_f64().unwrap(), 263.25));
    // Opening and closing without edits adds nothing.
    run(&mut e, &json!({ "cmd": "pageMasterView", "on": false }));
    assert_eq!(run(&mut e, &json!({ "cmd": "state" }))["canUndo"], false);
    assert!(part(
        &e.save_pptx().unwrap(),
        "/ppt/handoutMasters/handoutMaster1.xml"
    )
    .is_none());

    run(
        &mut e,
        &json!({ "cmd": "pageMasterView", "kind": "handout", "on": true }),
    );
    let r = run(
        &mut e,
        &json!({ "cmd": "pageMasterElements", "date": false }),
    );
    assert_eq!(r["pageMaster"]["date"], false);
    // Landscape: notesSz becomes 9 144 000 × 6 858 000; the header box
    // (0, 0, 2 880 360 × 457 200 EMU: 42 % × 5 % of the portrait page) scales
    // by 4/3 across and 3/4 down → 3 840 480 × 342 900; the footer's top
    // 9 144 000 − 457 200 = 8 686 800 → 6 515 100.
    let r = run(
        &mut e,
        &json!({ "cmd": "pageOrientation", "portrait": false }),
    );
    assert_eq!(r["pageMaster"]["portrait"], false);
    assert!(close(
        r["info"]["pages"][0]["width"].as_f64().unwrap(),
        720.0
    ));
    let p = pres(&e);
    let xfrm = |k: &str| {
        p.slides[0]
            .data
            .shapes
            .iter()
            .find_map(|n| match n {
                ShapeNode::Shape(s)
                    if s.nv.placeholder.as_ref().and_then(|x| x.kind.as_deref()) == Some(k) =>
                {
                    s.props.xfrm.clone()
                }
                _ => None,
            })
            .unwrap()
    };
    assert_eq!(xfrm("hdr").off, Some((0, 0)));
    assert_eq!(xfrm("hdr").ext, Some((3_840_480, 342_900)));
    assert_eq!(xfrm("ftr").off, Some((0, 6_515_100)));
    run(&mut e, &json!({ "cmd": "pageMasterView", "on": false }));
    let bytes = e.save_pptx().unwrap();
    keep("views-handout-master.pptx", &bytes);
    let xml = part_text(&bytes, "/ppt/handoutMasters/handoutMaster1.xml");
    assert!(xml.contains(r#"type="hdr""#));
    assert!(xml.contains(r#"type="sldNum""#));
    assert!(!xml.contains(r#"type="dt""#));
    let pres_xml = part_text(&bytes, "/ppt/presentation.xml");
    assert!(pres_xml.contains("handoutMasterIdLst"), "{pres_xml}");
    assert!(
        pres_xml.contains(r#"<p:notesSz cx="9144000" cy="6858000"/>"#),
        "{pres_xml}"
    );
    // Reopened: the handout master view shows the edited master.
    let mut again = open_bytes(&bytes);
    let r = run(
        &mut again,
        &json!({ "cmd": "pageMasterView", "kind": "handout", "on": true }),
    );
    assert_eq!(r["pageMaster"]["date"], false);
    assert_eq!(r["pageMaster"]["header"], true);
    assert_eq!(r["pageMaster"]["portrait"], false);
    // Undo of the session removes the new master again.
    run(&mut e, &json!({ "cmd": "undo" }));
    let back = e.save_pptx().unwrap();
    assert!(part(&back, "/ppt/handoutMasters/handoutMaster1.xml").is_none());
    assert!(part_text(&back, "/ppt/presentation.xml")
        .contains(r#"<p:notesSz cx="6858000" cy="9144000"/>"#));
}

// ---------------------------------------------------------------------------
// Sections
// ---------------------------------------------------------------------------

/// (name, first, count) of every section.
fn sections(r: &Value) -> Vec<(String, u64, u64)> {
    r.get("sections")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .map(|s| {
                    (
                        s["name"].as_str().unwrap().to_owned(),
                        s["first"].as_u64().unwrap(),
                        s["count"].as_u64().unwrap(),
                    )
                })
                .collect()
        })
        .unwrap_or_default()
}

fn sec(name: &str, first: u64, count: u64) -> (String, u64, u64) {
    (name.to_owned(), first, count)
}

fn titles(e: &mut Engine) -> Vec<String> {
    outline(e).into_iter().map(|(t, _)| t).collect()
}

#[test]
fn sections_add_rename_move_and_round_trip() {
    let mut e = open();
    let r = run(
        &mut e,
        &json!({ "cmd": "addSection", "name": "Phần B", "slide": 1, "firstName": "Phần A" }),
    );
    assert_eq!(sections(&r), vec![sec("Phần A", 0, 1), sec("Phần B", 1, 2)]);
    let r = run(
        &mut e,
        &json!({ "cmd": "renameSection", "index": 1, "name": "Phần hai" }),
    );
    assert_eq!(sections(&r)[1].0, "Phần hai");
    // Move "Phần hai" first: its two slides move with it.
    let r = run(
        &mut e,
        &json!({ "cmd": "moveSection", "index": 1, "to": 0 }),
    );
    assert_eq!(
        sections(&r),
        vec![sec("Phần hai", 0, 2), sec("Phần A", 2, 1)]
    );
    assert_eq!(r["slide"], 0);
    assert_eq!(titles(&mut e), vec!["Kế hoạch", "Kết luận", "Mở đầu"]);
    // A new slide after slide 1 joins its section.
    let r = run(&mut e, &json!({ "cmd": "addSlide", "after": 0 }));
    assert_eq!(
        sections(&r),
        vec![sec("Phần hai", 0, 3), sec("Phần A", 3, 1)]
    );
    // Dragging the last slide to the front lands it in the first section.
    let r = run(&mut e, &json!({ "cmd": "moveSlide", "from": 3, "to": 0 }));
    assert_eq!(
        sections(&r),
        vec![sec("Phần hai", 0, 4), sec("Phần A", 4, 0)]
    );
    // Save → reopen: the same sections; the ids in p14:sldIdLst are the slides'.
    let bytes = e.save_pptx().unwrap();
    keep("views-sections.pptx", &bytes);
    let xml = part_text(&bytes, "/ppt/presentation.xml");
    assert!(xml.contains("p14:sectionLst"), "{xml}");
    assert!(xml.contains(r#"name="Phần hai""#), "{xml}");
    let mut again = open_bytes(&bytes);
    let r = run(&mut again, &json!({ "cmd": "state" }));
    assert_eq!(
        sections(&r),
        vec![sec("Phần hai", 0, 4), sec("Phần A", 4, 0)]
    );
    let ids: Vec<u32> = pres(&again).slides.iter().map(|s| s.slide_id).collect();
    for id in &ids {
        assert!(xml.contains(&format!(r#"<p14:sldId id="{id}"/>"#)), "{id}");
    }
    // Untouched sections are saved byte for byte.
    let twice = again.save_pptx().unwrap();
    assert_eq!(
        part(&twice, "/ppt/presentation.xml"),
        part(&bytes, "/ppt/presentation.xml")
    );
    // Undo everything: no sections, the original order.
    for _ in 0..5 {
        run(&mut e, &json!({ "cmd": "undo" }));
    }
    let r = run(&mut e, &json!({ "cmd": "state" }));
    assert_eq!(sections(&r), Vec::<(String, u64, u64)>::new());
    assert_eq!(titles(&mut e), vec!["Mở đầu", "Kế hoạch", "Kết luận"]);
    assert!(!part_text(&e.save_pptx().unwrap(), "/ppt/presentation.xml").contains("sectionLst"));
}

#[test]
fn removing_sections() {
    let mut e = open();
    run(
        &mut e,
        &json!({ "cmd": "addSection", "name": "B", "slide": 1, "firstName": "A" }),
    );
    run(
        &mut e,
        &json!({ "cmd": "addSection", "name": "C", "slide": 2 }),
    );
    // Removing B: its slide joins A.
    let r = run(&mut e, &json!({ "cmd": "removeSection", "index": 1 }));
    assert_eq!(sections(&r), vec![sec("A", 0, 2), sec("C", 2, 1)]);
    // Removing C with its slides deletes "Kết luận".
    let r = run(
        &mut e,
        &json!({ "cmd": "removeSection", "index": 1, "slides": true }),
    );
    assert_eq!(sections(&r), vec![sec("A", 0, 2)]);
    assert_eq!(titles(&mut e), vec!["Mở đầu", "Kế hoạch"]);
    let r = run(&mut e, &json!({ "cmd": "removeAllSections" }));
    assert_eq!(sections(&r), Vec::<(String, u64, u64)>::new());
    assert_eq!(
        error_of(
            &mut e,
            &json!({ "cmd": "renameSection", "index": 0, "name": "x" })
        )
        .as_deref(),
        Some("noSection")
    );
}

/// The bridge routes these names before `SlideCommand`: none may be a variant
/// name (camelCase) of `SlideCommand`, nor appear twice.
#[test]
fn view_command_names_do_not_shadow_slide_commands() {
    let mut names: Vec<&str> = bo_engine::slide_edit::views::COMMANDS.to_vec();
    names.extend_from_slice(bo_engine::slide_edit::page_master::COMMANDS);
    names.extend_from_slice(bo_engine::slide_edit::master::COMMANDS);
    let src = include_str!("../src/slide_edit/mod.rs");
    let start = src.find("pub enum SlideCommand {").unwrap();
    let body = &src[start..];
    let body = &body[..body.find("\n}\n").unwrap()];
    let mut seen = 0;
    for line in body.lines() {
        let Some(rest) = line.strip_prefix("    ") else {
            continue;
        };
        let ident: String = rest
            .chars()
            .take_while(char::is_ascii_alphanumeric)
            .collect();
        if !ident.starts_with(|c: char| c.is_ascii_uppercase()) {
            continue;
        }
        seen += 1;
        let mut cs = ident.chars();
        let camel: String = cs
            .next()
            .map(|c| c.to_ascii_lowercase())
            .into_iter()
            .chain(cs)
            .collect();
        assert!(!names.contains(&camel.as_str()), "{camel} is shadowed");
    }
    assert!(seen > 50, "SlideCommand variants read: {seen}");
    let mut sorted = names.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.len(), names.len(), "duplicate command names");
}
