//! BlueSlide presenting (SLIDE-SHOW-001): custom shows, show setup, rehearsed
//! timings, show hyperlinks and kept ink — through the JSON commands, saved,
//! reopened and undone.
// Tests may unwrap: a panic is a test failure.
#![allow(clippy::unwrap_used)]

use bo_engine::slide_edit::geom::place_slide;
use bo_engine::slide_edit::present::{LinkAction, ShowSetup, ShowType, SlideRange};
use bo_engine::slide_edit::{SlideCommand, SlideEditResult};
use bo_engine::Engine;
use bo_model::slide::ShapeNode;
use bo_opc::{Limits, Package};
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

/// Save; with `BO_PRESENT_OUT=<dir>` the file is also written there (OpenXmlValidator runs).
fn save(e: &Engine, name: &str) -> Vec<u8> {
    let bytes = e.save_pptx().unwrap();
    if let Ok(dir) = std::env::var("BO_PRESENT_OUT") {
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(PathBuf::from(dir).join(name), &bytes).unwrap();
    }
    bytes
}

fn reopen(e: &Engine, name: &str) -> Engine {
    let bytes = save(e, name);
    let mut back = Engine::with_bundled_fonts();
    back.open(&bytes, name).unwrap();
    back
}

fn state(e: &mut Engine) -> SlideEditResult {
    run(e, r#"{"cmd":"state"}"#)
}

fn shows(e: &mut Engine) -> Vec<(u32, String, Vec<usize>)> {
    state(e)
        .presenting
        .unwrap()
        .custom_shows
        .into_iter()
        .map(|s| (s.id, s.name, s.slides))
        .collect()
}

fn part(bytes: &[u8], name: &str) -> Option<Vec<u8>> {
    let pkg = Package::read(bytes, &Limits::default()).unwrap();
    pkg.part(name).map(|p| p.data.clone())
}

fn part_text(bytes: &[u8], name: &str) -> String {
    String::from_utf8(part(bytes, name).unwrap()).unwrap()
}

/// Top-level shapes that are not placeholders.
fn free_ids(e: &Engine, slide: usize) -> Vec<u32> {
    let p = e.presentation().unwrap();
    place_slide(p, slide)
        .into_iter()
        .filter(|pl| match &p.slides[slide].data.shapes[pl.path[0]] {
            ShapeNode::Shape(s) => s.nv.placeholder.is_none(),
            ShapeNode::Other(_) => false,
            _ => true,
        })
        .map(|pl| pl.id)
        .collect()
}

#[test]
fn custom_shows_round_trip_with_their_playing_order() {
    let mut e = open("slide-text-bullets.pptx");
    assert_eq!(e.slide_count(), 3);
    assert_eq!(shows(&mut e), Vec::new());
    // Ids are ST_Index values, numbered from 0 in creation order.
    run(
        &mut e,
        r#"{"cmd":"setCustomShow","name":"Ngắn","slides":[2,0]}"#,
    );
    run(
        &mut e,
        r#"{"cmd":"setCustomShow","name":"Đủ","slides":[0,1,2,1]}"#,
    );
    assert_eq!(
        shows(&mut e),
        vec![
            (0, "Ngắn".to_owned(), vec![2, 0]),
            (1, "Đủ".to_owned(), vec![0, 1, 2, 1]),
        ]
    );
    // Same name (case-insensitive) as another show: refused.
    let r = try_run(
        &mut e,
        r#"{"cmd":"setCustomShow","name":"ngắn","slides":[1]}"#,
    );
    assert_eq!(r.error.as_deref(), Some("duplicateName"));
    let r = try_run(
        &mut e,
        r#"{"cmd":"setCustomShow","name":"  ","slides":[1]}"#,
    );
    assert_eq!(r.error.as_deref(), Some("emptyName"));
    let r = try_run(&mut e, r#"{"cmd":"setCustomShow","name":"X","slides":[9]}"#);
    assert_eq!(r.error.as_deref(), Some("noSlide"));
    // Edit show 0: new name and order.
    run(
        &mut e,
        r#"{"cmd":"setCustomShow","id":0,"name":"Rút gọn","slides":[1,2]}"#,
    );

    let bytes = save(&e, "present-custom-shows.pptx");
    let xml = part_text(&bytes, "/ppt/presentation.xml");
    // §19.2.1.26: custShowLst after notesSz/defaultTextStyle-ordered siblings, before extLst.
    let at = xml.find("<p:custShowLst>").unwrap();
    assert!(xml.find("<p:sldSz").unwrap() < at);
    if let Some(ext) = xml.find("<p:extLst>") {
        assert!(at < ext);
    }
    assert!(xml.contains(r#"<p:custShow name="Rút gọn" id="0">"#));

    let mut back = Engine::with_bundled_fonts();
    back.open(&bytes, "back.pptx").unwrap();
    assert_eq!(
        shows(&mut back),
        vec![
            (0, "Rút gọn".to_owned(), vec![1, 2]),
            (1, "Đủ".to_owned(), vec![0, 1, 2, 1]),
        ]
    );
    // Delete, then undo.
    run(&mut back, r#"{"cmd":"deleteCustomShow","id":1}"#);
    assert_eq!(shows(&mut back).len(), 1);
    run(&mut back, r#"{"cmd":"undo"}"#);
    assert_eq!(shows(&mut back).len(), 2);
    run(&mut back, r#"{"cmd":"deleteCustomShow","id":0}"#);
    run(&mut back, r#"{"cmd":"deleteCustomShow","id":1}"#);
    let bytes = save(&back, "present-no-shows.pptx");
    assert!(!part_text(&bytes, "/ppt/presentation.xml").contains("custShowLst"));
}

#[test]
fn custom_shows_follow_added_and_deleted_slides() {
    let mut e = open("slide-text-bullets.pptx");
    // A slide added in this session gets a relationship id the writer keeps.
    run(&mut e, r#"{"cmd":"addSlide","after":2}"#);
    assert_eq!(e.slide_count(), 4);
    run(
        &mut e,
        r#"{"cmd":"setCustomShow","name":"Mới","slides":[3,1]}"#,
    );
    let mut back = reopen(&e, "present-new-slide.pptx");
    assert_eq!(shows(&mut back), vec![(0, "Mới".to_owned(), vec![3, 1])]);
    // Deleting slide 2 (index 1): the show keeps the other slide, the file stays valid.
    run(&mut back, r#"{"cmd":"deleteSlide","index":1}"#);
    assert_eq!(shows(&mut back), vec![(0, "Mới".to_owned(), vec![2])]);
    let bytes = save(&back, "present-deleted-slide.pptx");
    let xml = part_text(&bytes, "/ppt/presentation.xml");
    assert_eq!(xml.matches("<p:sld r:id=").count(), 1);
    let mut again = Engine::with_bundled_fonts();
    again.open(&bytes, "again.pptx").unwrap();
    assert_eq!(shows(&mut again), vec![(0, "Mới".to_owned(), vec![2])]);
}

#[test]
fn show_setup_round_trips_and_keeps_other_parts() {
    let mut e = open("slide-text-bullets.pptx");
    let before = save(&e, "present-before.pptx");
    assert_eq!(
        state(&mut e).presenting.unwrap().setup,
        ShowSetup::default()
    );
    run(
        &mut e,
        r#"{"cmd":"setCustomShow","name":"A","slides":[0,2]}"#,
    );
    run(
        &mut e,
        r#"{"cmd":"setShowSetup","setup":{"showType":"kiosk","restartMs":60000,"loop":true,
            "narration":false,"animation":false,"useTimings":true,
            "range":{"kind":"custom","id":0},"penColor":"FF0000"}}"#,
    );
    let want = ShowSetup {
        show_type: ShowType::Kiosk,
        restart_ms: 60_000,
        looped: true,
        animation: false,
        range: SlideRange::Custom { id: 0 },
        pen_color: Some("FF0000".into()),
        ..ShowSetup::default()
    };
    assert_eq!(state(&mut e).presenting.unwrap().setup, want);
    let bytes = save(&e, "present-setup.pptx");
    let props = part_text(&bytes, "/ppt/presProps.xml");
    // §19.2.1.27: showPr before the extLst that was already there; defaults left out.
    assert!(props.contains(
        r#"<p:showPr loop="1" showAnimation="0"><p:kiosk restart="60000"/><p:custShow id="0"/><p:penClr><a:srgbClr val="FF0000"/></p:penClr></p:showPr><p:extLst>"#
    ), "{props}");
    // Slides, layouts, masters and theme are byte-stable.
    for name in [
        "/ppt/slides/slide1.xml",
        "/ppt/slides/slide2.xml",
        "/ppt/slides/slide3.xml",
        "/ppt/slideMasters/slideMaster1.xml",
        "/ppt/theme/theme1.xml",
        "/ppt/viewProps.xml",
    ] {
        assert_eq!(part(&before, name), part(&bytes, name), "{name}");
    }
    let mut back = Engine::with_bundled_fonts();
    back.open(&bytes, "back.pptx").unwrap();
    assert_eq!(state(&mut back).presenting.unwrap().setup, want);
    // Deleting the custom show the setup plays: every slide again.
    run(&mut back, r#"{"cmd":"deleteCustomShow","id":0}"#);
    assert_eq!(
        state(&mut back).presenting.unwrap().setup.range,
        SlideRange::All
    );
    // Browsed in a window, slides 2–3; undo goes back step by step.
    run(
        &mut back,
        r#"{"cmd":"setShowSetup","setup":{"showType":"browse","scrollbar":false,"range":{"kind":"range","from":2,"to":3}}}"#,
    );
    let s = state(&mut back).presenting.unwrap().setup;
    assert_eq!(
        (s.show_type, s.scrollbar, s.range),
        (
            ShowType::Browse,
            false,
            SlideRange::Range { from: 2, to: 3 }
        )
    );
    let r = try_run(
        &mut back,
        r#"{"cmd":"setShowSetup","setup":{"range":{"kind":"range","from":3,"to":2}}}"#,
    );
    assert_eq!(r.error.as_deref(), Some("badRange"));
    run(&mut back, r#"{"cmd":"undo"}"#);
    assert_eq!(
        state(&mut back).presenting.unwrap().setup.range,
        SlideRange::All
    );
}

#[test]
fn show_setup_creates_the_properties_part() {
    // A converted binary deck has no presentation properties part.
    let mut e = open("binary-ppt-01-shapes.ppt");
    run(
        &mut e,
        r#"{"cmd":"setShowSetup","setup":{"showType":"browse"}}"#,
    );
    let bytes = save(&e, "present-new-props.pptx");
    let props = part_text(&bytes, "/ppt/presProps.xml");
    assert!(
        props.contains("<p:showPr><p:browse/><p:sldAll/></p:showPr>"),
        "{props}"
    );
    let rels = part_text(&bytes, "/ppt/_rels/presentation.xml.rels");
    assert!(rels.contains("relationships/presProps"));
    let ct = part_text(&bytes, "/[Content_Types].xml");
    assert!(ct.contains("presentationml.presProps+xml"));
    let mut back = Engine::with_bundled_fonts();
    back.open(&bytes, "back.pptx").unwrap();
    assert_eq!(
        state(&mut back).presenting.unwrap().setup.show_type,
        ShowType::Browse
    );
}

#[test]
fn rehearsed_timings_are_written_as_adv_tm() {
    let mut e = open("slide-text-bullets.pptx");
    // Rehearsal: 4.5 s on slide 1, 12.25 s on slide 3; slide 2 not timed.
    run(
        &mut e,
        r#"{"cmd":"setTimings","timings":[{"slide":0,"ms":4500},{"slide":2,"ms":12250}],"useTimings":true}"#,
    );
    let p = state(&mut e).presenting.unwrap();
    assert_eq!(p.timings, vec![Some(4500), None, Some(12250)]);
    let bytes = save(&e, "present-timings.pptx");
    // §19.3.1.50: advTm in milliseconds.
    assert!(part_text(&bytes, "/ppt/slides/slide1.xml").contains(r#"advTm="4500""#));
    assert!(part_text(&bytes, "/ppt/slides/slide3.xml").contains(r#"advTm="12250""#));
    assert!(!part_text(&bytes, "/ppt/slides/slide2.xml").contains("advTm"));
    // The show plays the timing: advance after 4500 ms.
    let r = run(&mut e, r#"{"cmd":"show","slide":0}"#);
    assert_eq!(
        r.show.unwrap().transition.unwrap().advance_after_ms,
        Some(4500)
    );
    // One undo step for the whole rehearsal.
    run(&mut e, r#"{"cmd":"undo"}"#);
    assert_eq!(
        state(&mut e).presenting.unwrap().timings,
        vec![None, None, None]
    );
    let bytes = save(&e, "present-timings-undone.pptx");
    assert!(!part_text(&bytes, "/ppt/slides/slide1.xml").contains("transition"));
}

#[test]
fn hidden_slides_are_written_as_show_0_and_left_out_of_the_show() {
    let mut e = open("slide-text-bullets.pptx");
    // Hide the current slide (a toggle), then slide 3 by index.
    run(&mut e, r#"{"cmd":"goTo","slide":1}"#);
    run(&mut e, r#"{"cmd":"hideSlides"}"#);
    assert_eq!(state(&mut e).presenting.unwrap().hidden, vec![1]);
    run(&mut e, r#"{"cmd":"hideSlides","slides":[2],"hidden":true}"#);
    assert_eq!(state(&mut e).presenting.unwrap().hidden, vec![1, 2]);
    let bytes = save(&e, "present-hidden.pptx");
    // §19.3.1.38: sld@show="0" on the hidden slides only.
    assert!(!part_text(&bytes, "/ppt/slides/slide1.xml").contains(r#"show="0""#));
    assert!(part_text(&bytes, "/ppt/slides/slide2.xml").contains(r#"show="0""#));
    assert!(part_text(&bytes, "/ppt/slides/slide3.xml").contains(r#"show="0""#));
    let mut back = Engine::with_bundled_fonts();
    back.open(&bytes, "back.pptx").unwrap();
    assert_eq!(state(&mut back).presenting.unwrap().hidden, vec![1, 2]);
    // The same command on a hidden slide shows it again; each change is one undo step.
    run(&mut e, r#"{"cmd":"hideSlides"}"#);
    assert_eq!(state(&mut e).presenting.unwrap().hidden, vec![2]);
    run(&mut e, r#"{"cmd":"undo"}"#);
    assert_eq!(state(&mut e).presenting.unwrap().hidden, vec![1, 2]);
    run(&mut e, r#"{"cmd":"undo"}"#);
    run(&mut e, r#"{"cmd":"undo"}"#);
    assert!(state(&mut e).presenting.unwrap().hidden.is_empty());
    let r = try_run(&mut e, r#"{"cmd":"hideSlides","slides":[9]}"#);
    assert_eq!(r.error.as_deref(), Some("noSlide"));
}

#[test]
fn hyperlinks_start_custom_shows() {
    let mut e = open("slide-text-bullets.pptx");
    run(
        &mut e,
        r#"{"cmd":"setCustomShow","name":"Phụ lục","slides":[2]}"#,
    );
    let r = try_run(
        &mut e,
        r#"{"cmd":"setShowLink","action":{"kind":"customShow","id":0,"return":true}}"#,
    );
    assert_eq!(r.error.as_deref(), Some("noSelection"));
    let p = e.presentation().unwrap();
    let id = place_slide(p, 0)[0].id;
    run(&mut e, &format!(r#"{{"cmd":"select","ids":[{id}]}}"#));
    run(
        &mut e,
        r#"{"cmd":"setShowLink","action":{"kind":"customShow","id":0,"return":true}}"#,
    );
    let mut back = reopen(&e, "present-link.pptx");
    let show = run(&mut back, r#"{"cmd":"show","slide":0}"#).show.unwrap();
    assert_eq!(show.links.len(), 1);
    assert_eq!(show.links[0].shape_id, id);
    assert_eq!(
        show.links[0].action,
        LinkAction::CustomShow { id: 0, ret: true }
    );
    let bytes = save(&back, "present-link-2.pptx");
    assert!(part_text(&bytes, "/ppt/slides/slide1.xml").contains(
        r#"<a:hlinkClick r:id="" action="ppaction://customshow?id=0&amp;return=true"/>"#
    ));
    // Removing the link.
    run(&mut back, &format!(r#"{{"cmd":"select","ids":[{id}]}}"#));
    run(
        &mut back,
        r#"{"cmd":"setShowLink","action":{"kind":"none"}}"#,
    );
    let links = run(&mut back, r#"{"cmd":"show","slide":0}"#)
        .show
        .unwrap()
        .links;
    assert_eq!(links.len(), 0);
}

#[test]
fn kept_ink_becomes_freeform_shapes() {
    let mut e = open("slide-text-bullets.pptx");
    let n0 = free_ids(&e, 1).len();
    run(
        &mut e,
        r#"{"cmd":"keepInk","name":"Nét mực","ink":[{"slide":1,"strokes":[
            {"points":[[100,100],[150,120],[200,100]],"color":"C00000","width":2.5},
            {"points":[[50,300],[400,300]],"color":"FFFF00","width":12,"highlighter":true}]}]}"#,
    );
    assert_eq!(free_ids(&e, 1).len(), n0 + 2);
    let bytes = save(&e, "present-ink.pptx");
    let xml = part_text(&bytes, "/ppt/slides/slide2.xml");
    assert!(xml.contains("<a:custGeom>") && xml.contains(r#"<a:alpha val="50000"/>"#));
    let back = reopen(&e, "present-ink-2.pptx");
    assert_eq!(free_ids(&back, 1).len(), n0 + 2);
    run(&mut e, r#"{"cmd":"undo"}"#);
    assert_eq!(free_ids(&e, 1).len(), n0);
}
