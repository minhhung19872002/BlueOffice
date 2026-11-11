//! BlueSlide transitions, animations, slide show layers, charts and media
//! (Phase 4): edit through the JSON commands, save, reopen.
// Tests may unwrap: a panic is a test failure.
#![allow(clippy::unwrap_used)]

use bo_engine::slide_edit::geom::place_slide;
use bo_engine::slide_edit::{SlideCommand, SlideEditResult};
use bo_engine::Engine;
use bo_model::slide::{AnimClass, AnimEffect, AnimTrigger, ShapeNode, TransitionKind};
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

fn reopen(e: &Engine) -> Engine {
    let bytes = e.save_pptx().unwrap();
    let mut back = Engine::with_bundled_fonts();
    back.open(&bytes, "back.pptx").unwrap();
    back
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
fn transitions_are_set_saved_and_undone() {
    let mut e = open("slide-text-bullets.pptx");
    let n = e.slide_count();
    let r = run(
        &mut e,
        r#"{"cmd":"setTransition","transition":{"kind":"push","dir":"u","durMs":900}}"#,
    );
    let t = r.transition.as_ref().unwrap();
    assert_eq!(
        (t.kind.as_str(), t.dir.as_deref(), t.dur_ms),
        ("push", Some("u"), 900)
    );
    assert!(t.advance_on_click && t.known);
    // Every slide: wipe; then the timing only (advance after 2 s).
    run(
        &mut e,
        r#"{"cmd":"setTransition","all":true,"transition":{"kind":"wipe","dir":"l"}}"#,
    );
    run(
        &mut e,
        r#"{"cmd":"setTransition","slides":[0],"transition":{"kind":"keep","advanceAfterMs":2000}}"#,
    );
    let back = reopen(&e);
    let p = back.presentation().unwrap();
    assert_eq!(p.slides.len(), n);
    for (i, s) in p.slides.iter().enumerate() {
        let t = s.transition.as_ref().unwrap();
        assert_eq!(t.kind, Some(TransitionKind::Wipe), "slide {i}");
        assert_eq!(t.dir.as_deref(), Some("l"));
    }
    assert_eq!(
        p.slides[0].transition.as_ref().unwrap().advance_after_ms,
        Some(2000)
    );
    assert_eq!(p.slides[0].transition.as_ref().unwrap().dur_ms, Some(900));
    // Fade has no direction (§19.3.1.20): an invalid one falls back to the default variant
    // (smoothly, not through black); "none" removes the transition.
    let r = run(
        &mut e,
        r#"{"cmd":"setTransition","transition":{"kind":"fade","dir":"l"}}"#,
    );
    let t = r.transition.as_ref().unwrap();
    assert_eq!(t.dir.as_deref(), Some("smooth"));
    assert_eq!(t.dirs, vec!["smooth", "black"]);
    let r = run(
        &mut e,
        r#"{"cmd":"setTransition","transition":{"kind":"none"}}"#,
    );
    assert!(r.transition.is_none());
    let r = try_run(
        &mut e,
        r#"{"cmd":"setTransition","transition":{"kind":"notAnEffect"}}"#,
    );
    assert_eq!(r.error.as_deref(), Some("badTransition"));
    for _ in 0..5 {
        run(&mut e, r#"{"cmd":"undo"}"#);
    }
    assert!(e.presentation().unwrap().slides[0].transition.is_none());
}

#[test]
fn animations_are_added_edited_reordered_and_saved() {
    let mut e = open("slide-shapes-presets.pptx");
    let ids = free_ids(&e, 0);
    let (a, b, c) = (ids[0], ids[1], ids[2]);
    run(&mut e, &format!(r#"{{"cmd":"select","ids":[{a}]}}"#));
    run(&mut e, r#"{"cmd":"addAnimation","effect":"fade"}"#);
    run(&mut e, &format!(r#"{{"cmd":"select","ids":[{b},{c}]}}"#));
    let r = run(
        &mut e,
        r#"{"cmd":"addAnimation","effect":"flyIn","direction":"l","trigger":"afterPrevious","durMs":700}"#,
    );
    assert_eq!(r.animations.len(), 3);
    assert_eq!(r.animations[1].trigger, "afterPrevious");
    assert_eq!(r.animations[2].trigger, "withPrevious");
    assert_eq!(r.animations[1].direction.as_deref(), Some("l"));
    // Timeline: fade 0–500 on click 1, fly-ins after it (500–1200).
    assert_eq!(
        (
            r.animations[1].click,
            r.animations[1].start_ms,
            r.animations[1].end_ms
        ),
        (1, 500, 1200)
    );
    assert_eq!(r.animations[2].start_ms, 500);
    assert!(r.animations[0].rect.is_some());
    run(
        &mut e,
        r#"{"cmd":"setAnimation","index":2,"effect":"wipe","direction":"t","trigger":"onClick","delayMs":250}"#,
    );
    run(
        &mut e,
        r#"{"cmd":"setAnimation","index":0,"class":"exit","effect":"appear"}"#,
    );
    let r = run(&mut e, r#"{"cmd":"moveAnimation","from":2,"to":0}"#);
    assert_eq!(r.animations[0].effect, "wipe");
    assert_eq!(r.animations[0].shape_id, c);
    let back = reopen(&e);
    let t = back.presentation().unwrap().slides[0]
        .timing
        .clone()
        .unwrap();
    let got: Vec<_> = t
        .animations
        .iter()
        .map(|x| {
            (
                x.shape_id,
                x.class.clone(),
                x.effect.clone(),
                x.trigger,
                x.delay_ms,
            )
        })
        .collect();
    assert_eq!(
        got,
        vec![
            (
                c,
                AnimClass::Entrance,
                AnimEffect::Wipe,
                AnimTrigger::OnClick,
                250
            ),
            (
                a,
                AnimClass::Exit,
                AnimEffect::Appear,
                AnimTrigger::OnClick,
                0
            ),
            (
                b,
                AnimClass::Entrance,
                AnimEffect::FlyIn,
                AnimTrigger::AfterPrevious,
                0
            ),
        ]
    );
    let r = run(&mut e, r#"{"cmd":"removeAnimation","index":1}"#);
    assert_eq!(r.animations.len(), 2);
    let r = try_run(&mut e, r#"{"cmd":"removeAnimation","index":9}"#);
    assert_eq!(r.error.as_deref(), Some("noAnimation"));
    // Undo everything: the slide has no timing again.
    for _ in 0..6 {
        run(&mut e, r#"{"cmd":"undo"}"#);
    }
    let t = e.presentation().unwrap().slides[0].timing.clone();
    assert!(t.is_none_or(|t| t.animations.is_empty()));
}

#[test]
fn show_splits_the_slide_into_layers_for_animated_shapes() {
    let mut e = open("slide-anim-fly-wipe.pptx");
    let r = run(&mut e, r#"{"cmd":"show","slide":0}"#);
    let show = r.show.unwrap();
    assert!(show.animations.len() >= 2);
    assert!(show.layers[0].backdrop);
    assert!(show.layers.iter().skip(1).all(|l| !l.backdrop));
    for a in &show.animations {
        let l = a.layer.expect("animated shape has a layer");
        assert_eq!(show.layers[l].ids, vec![a.shape_id]);
    }
    assert_eq!(
        show.clicks,
        show.animations.iter().map(|a| a.click).max().unwrap_or(0)
    );
    assert!(show.clicks >= 1);
    // The layers together draw what the slide draws.
    let full = e.slide(0).unwrap().commands.len();
    let parts: usize = show.layers.iter().map(|l| l.page.commands.len()).sum();
    assert_eq!(parts, full);
    // A slide without effects is one layer; a bad index is refused.
    let mut plain = open("slide-text-bullets.pptx");
    let s = run(&mut plain, r#"{"cmd":"show","slide":0}"#).show.unwrap();
    assert_eq!(s.layers.len(), 1);
    assert_eq!(s.clicks, 0);
    let r = try_run(&mut plain, r#"{"cmd":"show","slide":99}"#);
    assert_eq!(r.error.as_deref(), Some("noSlide"));
    // Transitions of the corpus deck come through with their durations.
    let mut t = open("slide-anim-transitions.pptx");
    let s = run(&mut t, r#"{"cmd":"show","slide":2}"#).show.unwrap();
    let tr = s.transition.unwrap();
    assert_eq!((tr.kind.as_str(), tr.dur_ms), ("random", 1000));
}

#[test]
fn unknown_effects_are_kept_when_the_sequence_is_edited() {
    // The corpus deck written by producers: effects BlueSlide does not model
    // are kept as read when another effect changes.
    let mut e = open("slide-anim-fade-appear.pptx");
    let before = e.presentation().unwrap().slides[0].timing.clone().unwrap();
    let n = before.animations.len();
    assert!(n > 0);
    run(&mut e, r#"{"cmd":"setAnimation","index":0,"durMs":1234}"#);
    let back = reopen(&e);
    let t = back.presentation().unwrap().slides[0]
        .timing
        .clone()
        .unwrap();
    assert_eq!(t.animations.len(), n);
    assert_eq!(t.animations[0].dur_ms.max(1234), t.animations[0].dur_ms);
    for (x, y) in before.animations.iter().zip(&t.animations).skip(1) {
        assert_eq!(
            (x.shape_id, &x.class, &x.effect),
            (y.shape_id, &y.class, &y.effect)
        );
    }
}

#[test]
fn charts_are_inserted_with_an_embedded_workbook_and_edited() {
    let mut e = open("slide-text-bullets.pptx");
    let r = run(
        &mut e,
        r#"{"cmd":"insertChart","kind":"column","title":"Doanh thu","data":{"categories":["Q1","Q2","Q3"],"series":[{"name":"2025","values":[1,2,3]},{"name":"2026","values":[2,null,4]}]}}"#,
    );
    assert_eq!(r.selection[0].kind, "chart");
    let c = r.chart.unwrap();
    assert_eq!(
        (c.kind.as_str(), c.title.as_deref()),
        ("column", Some("Doanh thu"))
    );
    assert_eq!(c.data.categories, ["Q1", "Q2", "Q3"]);
    assert_eq!(c.data.series[1].values, vec![Some(2.0), None, Some(4.0)]);
    assert!(c.embedded);
    let id = c.id;
    // The chart draws (bars) on the slide.
    let slide = r.slide;
    let page = e.slide(slide).unwrap().to_json();
    assert!(page.contains("Doanh thu") || page.contains("glyphs"));
    // Change type, title, legend and the grid.
    let r = run(
        &mut e,
        r#"{"cmd":"setChart","kind":"line","title":"Lợi nhuận","legend":"r","data":{"categories":["T1","T2","T3","T4"],"series":[{"name":"A","values":[4,3,2,1]}]}}"#,
    );
    let c = r.chart.unwrap();
    assert_eq!(
        (c.kind.as_str(), c.title.as_deref(), c.legend.as_deref()),
        ("line", Some("Lợi nhuận"), Some("r"))
    );
    assert_eq!(c.data.series.len(), 1);
    assert_eq!(c.data.categories.len(), 4);
    let r = run(
        &mut e,
        r#"{"cmd":"setChart","kind":"pie","title":"","legend":"none"}"#,
    );
    let c = r.chart.unwrap();
    assert_eq!(
        (c.kind.as_str(), c.title.clone(), c.legend.clone()),
        ("pie", None, None)
    );
    // Saved: chart part, its relationships and a readable workbook with the grid.
    let back = reopen(&e);
    let p = back.presentation().unwrap();
    let chart_part = p
        .parts
        .iter()
        .find(|x| x.name.starts_with("/ppt/charts/chart"))
        .unwrap();
    assert_eq!(
        chart_part.content_type.as_deref(),
        Some("application/vnd.openxmlformats-officedocument.drawingml.chart+xml")
    );
    let xml = String::from_utf8_lossy(&chart_part.data);
    assert!(
        xml.contains("<c:pieChart>") && xml.contains("c:externalData"),
        "{xml}"
    );
    let book = p
        .parts
        .iter()
        .find(|x| x.name.starts_with("/ppt/embeddings/") && x.name.ends_with(".xlsx"))
        .unwrap();
    let wb = bo_xlsx::read_xlsx(&book.data).unwrap();
    let ws = &wb.sheets[0];
    assert_eq!(
        ws.cell_a1("A2").and_then(|c| c.value.as_text()).as_deref(),
        Some("T1")
    );
    assert_eq!(
        ws.cell_a1("B1").and_then(|c| c.value.as_text()).as_deref(),
        Some("A")
    );
    // Reopened: the chart is still editable.
    let mut back = back;
    run(&mut back, &format!(r#"{{"cmd":"goTo","slide":{slide}}}"#));
    let r = run(&mut back, &format!(r#"{{"cmd":"select","ids":[{id}]}}"#));
    assert_eq!(
        r.chart.unwrap().data.series[0].values,
        vec![Some(4.0), Some(3.0), Some(2.0), Some(1.0)]
    );
    // Undo back to no chart.
    for _ in 0..3 {
        run(&mut e, r#"{"cmd":"undo"}"#);
    }
    assert!(!e
        .presentation()
        .unwrap()
        .parts
        .iter()
        .any(|x| x.name.starts_with("/ppt/charts/")));
}

#[test]
fn existing_charts_of_the_corpus_are_editable() {
    for name in [
        "slide-charts-column.pptx",
        "slide-charts-line.pptx",
        "slide-charts-pie.pptx",
        "slide-charts-bar-stacked.pptx",
    ] {
        let mut e = open(name);
        let p = e.presentation().unwrap();
        let id = place_slide(p, 0)
            .iter()
            .find(|pl| matches!(&p.slides[0].data.shapes[pl.path[0]], ShapeNode::Frame(_)))
            .map_or_else(|| panic!("{name}: no chart"), |pl| pl.id);
        let r = run(&mut e, &format!(r#"{{"cmd":"select","ids":[{id}]}}"#));
        let mut c = r.chart.unwrap_or_else(|| panic!("{name}: not a chart"));
        assert!(!c.data.series.is_empty(), "{name}");
        c.data.series[0].values[0] = Some(42.0);
        let data = serde_json::json!({
            "categories": c.data.categories,
            "series": c.data.series.iter().map(|s| serde_json::json!({"name": s.name, "values": s.values})).collect::<Vec<_>>(),
        });
        let r = run(
            &mut e,
            &format!(r#"{{"cmd":"setChart","title":"Đã sửa","data":{data}}}"#),
        );
        let out = r.chart.unwrap();
        assert_eq!(out.data.series[0].values[0], Some(42.0), "{name}");
        assert_eq!(out.kind, c.kind, "{name}");
        assert!(out.embedded, "{name}");
        let back = reopen(&e);
        assert!(bo_pptx::read_pptx(&back.save_pptx().unwrap()).is_ok());
    }
}

#[test]
fn media_clips_are_listed_and_inserted() {
    // Embedded video, embedded audio, linked video.
    let e = open("slide-media-video.pptx");
    let mut v = e;
    let r = run(&mut v, r#"{"cmd":"state"}"#);
    assert_eq!(r.media.len(), 1);
    let m = &r.media[0];
    assert_eq!((m.kind, m.mime.as_deref()), ("video", Some("video/mp4")));
    let part = m.part.clone().unwrap();
    assert!(v.media(&part).is_some_and(|b| b.len() > 8));
    let mut l = open("slide-media-linked.pptx");
    let r = run(&mut l, r#"{"cmd":"state"}"#);
    let linked = r.media.iter().find(|m| m.kind == "video").unwrap();
    assert!(linked.part.is_none());
    assert_eq!(
        linked.link.as_deref(),
        Some("https://example.invalid/clip.mp4")
    );
    assert!(r
        .media
        .iter()
        .any(|m| m.kind == "audio" && m.part.is_some()));

    // Insert a clip: media part, poster picture, three relationships.
    let mut e = open("slide-text-bullets.pptx");
    let mut mp4 = b"\x00\x00\x00\x18ftypmp42\x00\x00\x00\x00mp42isom".to_vec();
    mp4.extend(std::iter::repeat_n(0u8, 32));
    let r = e.slide_insert_picture(mp4.clone(), "phim.mp4").unwrap();
    assert!(r.error.is_none(), "{:?}", r.error);
    assert_eq!(r.media.len(), 1);
    assert_eq!(r.media[0].kind, "video");
    let r = e
        .slide_insert_picture(b"ID3\x04\x00\x00".to_vec(), "nhac.mp3")
        .unwrap();
    assert_eq!(r.media.len(), 2);
    let r = e.slide_insert_picture(vec![1, 2, 3], "x.xyz").unwrap();
    assert!(r.error.is_some());
    let back = reopen(&e);
    let p = back.presentation().unwrap();
    let s = &p.slides[0];
    for t in ["video", "audio", "image"] {
        assert!(
            s.info
                .rels
                .iter()
                .any(|r| r.rel_type.ends_with(&format!("/{t}"))),
            "{t}"
        );
    }
    let media = p.parts.iter().find(|x| x.data == mp4).unwrap();
    assert_eq!(media.content_type.as_deref(), Some("video/mp4"));
    let mut back = back;
    let r = run(&mut back, r#"{"cmd":"state"}"#);
    assert_eq!(r.media.len(), 2);
    assert_eq!(r.media[1].mime.as_deref(), Some("audio/mpeg"));
}

// source: ECMA-376 Part 1, §21.4 (dataModel, layoutDef, relIds) and [MS-ODRAWXML] dsp:drawing —
// CORE-SMART-003 / 004.
#[test]
fn smartart_inserted_edited_converted_and_saved() {
    let mut e = open("slide-text-bullets.pptx");
    let r = run(
        &mut e,
        r#"{"cmd":"insertSmartArt","layout":"process","items":[{"text":"Tiếp nhận"},{"text":"Xử lý"},{"text":"Trả kết quả"}]}"#,
    );
    assert_eq!(r.selection[0].kind, "smartart");
    let sa = r.smartart.clone().unwrap();
    assert_eq!(sa.layout, "process");
    let texts: Vec<&str> = sa.items.iter().map(|i| i.text.as_str()).collect();
    assert_eq!(texts, ["Tiếp nhận", "Xử lý", "Trả kết quả"]);
    // Drawn from its drawing part: the step texts are on the slide.
    let page = e.slide(r.slide).unwrap().to_json();
    assert!(page.contains("Xử"), "drawing shown");
    // Text pane: add a sub-item, switch to a hierarchy.
    let r = run(
        &mut e,
        r#"{"cmd":"setSmartArt","layout":"hierarchy","items":[{"text":"Giám đốc"},{"text":"Kỹ thuật","level":1},{"text":"Kinh doanh","level":1}]}"#,
    );
    let sa = r.smartart.clone().unwrap();
    assert_eq!(sa.layout, "hierarchy");
    assert_eq!(sa.items[1].level, 1);
    assert_eq!(
        (sa.colors.as_str(), sa.style.as_str()),
        ("colorful", "fill")
    );
    // Colours and look.
    let r = run(
        &mut e,
        r#"{"cmd":"setSmartArt","colors":"green","style":"outline"}"#,
    );
    let sa2 = r.smartart.clone().unwrap();
    assert_eq!(
        (sa2.colors.as_str(), sa2.style.as_str()),
        ("green", "outline")
    );
    assert_eq!(sa2.items, sa.items, "outline kept");
    assert!(
        try_run(&mut e, r#"{"cmd":"setSmartArt","colors":"rainbow"}"#)
            .error
            .is_some()
    );
    // Saved with all diagram parts and relationships; reopened, it reads back the same.
    let back = reopen(&e);
    let p = back.presentation().unwrap();
    for (prefix, ct) in [
        ("/ppt/diagrams/data", "drawingml.diagramData+xml"),
        ("/ppt/diagrams/layout", "drawingml.diagramLayout+xml"),
        ("/ppt/diagrams/quickStyle", "drawingml.diagramStyle+xml"),
        ("/ppt/diagrams/colors", "drawingml.diagramColors+xml"),
        ("/ppt/diagrams/drawing", "diagramDrawing+xml"),
    ] {
        let part = p.parts.iter().find(|x| x.name.starts_with(prefix)).unwrap();
        assert!(
            part.content_type
                .as_deref()
                .unwrap_or_default()
                .ends_with(ct),
            "{prefix}"
        );
    }
    let mut back = back;
    let rels = &back.presentation().unwrap().slides[r.slide].info.rels;
    assert!(rels.iter().any(|r| r.rel_type.ends_with("/diagramData")));
    assert!(rels.iter().any(|r| r.rel_type.ends_with("/diagramDrawing")));
    let r2 = run(
        &mut back,
        &format!(
            r#"{{"cmd":"select","slide":{},"ids":[{}]}}"#,
            r.slide, sa.id
        ),
    );
    assert_eq!(r2.smartart.unwrap().items.len(), 3);
    // Convert to shapes: a group of ordinary shapes with the same id.
    let r = run(&mut e, r#"{"cmd":"smartArtToShapes"}"#);
    assert_eq!(r.selection[0].kind, "group");
    assert!(r.smartart.is_none());
    assert!(e.slide(r.slide).unwrap().to_json().contains("Giám"));
    // Undo brings the SmartArt back.
    let r = run(&mut e, r#"{"cmd":"undo"}"#);
    assert_eq!(r.smartart.unwrap().layout, "hierarchy");
    // Unknown layouts and empty outlines are refused.
    let r = try_run(
        &mut e,
        r#"{"cmd":"insertSmartArt","layout":"spiral","items":[{"text":"x"}]}"#,
    );
    assert!(r.error.is_some());
    let r = try_run(
        &mut e,
        r#"{"cmd":"insertSmartArt","layout":"list","items":[{"text":"  "}]}"#,
    );
    assert!(r.error.is_some());
}

// source: ECMA-376 Part 1, §21.2.2.153 radarChart / §21.2.2.126 ofPieChart / §21.2.2.198 stockChart —
// CORE-CHART-002 types inserted on a slide.
#[test]
fn new_chart_types_on_slides() {
    let mut e = open("slide-text-bullets.pptx");
    for (kind, element) in [
        (
            "radarFilled",
            "<c:radarChart><c:radarStyle val=\"filled\"/>",
        ),
        ("barOfPie", "<c:ofPieType val=\"bar\"/>"),
        ("percentColumn", "<c:grouping val=\"percentStacked\"/>"),
        ("stockUpDown", "<c:upDownBars>"),
        // CORE-CHART-003: 3-D groups with their view (§21.2.2.228) and series axis (§21.2.2.175).
        ("column3D", "<c:serAx>"),
        ("cylinder3D", "<c:shape val=\"cylinder\"/>"),
        ("pie3D", "<c:view3D><c:rotX val=\"30\"/>"),
        ("explodedPie3D", "<c:explosion val=\"25\"/>"),
        (
            "percentArea3D",
            "<c:area3DChart><c:grouping val=\"percentStacked\"/>",
        ),
    ] {
        let data = if kind.starts_with("stock") {
            r#"{"categories":["N1","N2","N3"],"series":[{"name":"O","values":[10,11,12]},{"name":"H","values":[12,13,14]},{"name":"L","values":[9,10,11]},{"name":"C","values":[11,12,11]}]}"#
        } else {
            r#"{"categories":["A","B","C","D"],"series":[{"name":"S1","values":[4,3,2,5]},{"name":"S2","values":[1,2,3,2]}]}"#
        };
        let r = run(
            &mut e,
            &format!(r#"{{"cmd":"insertChart","kind":"{kind}","data":{data}}}"#),
        );
        assert_eq!(r.chart.as_ref().unwrap().kind, kind, "{kind}");
        let page = e.slide(r.slide).unwrap().to_json();
        assert!(
            page.contains("\"path\"") || page.contains("Path"),
            "{kind} drawn"
        );
        let p = e.presentation().unwrap();
        let xml = p
            .parts
            .iter()
            .rev()
            .find(|x| x.name.starts_with("/ppt/charts/chart"))
            .map(|x| String::from_utf8_lossy(&x.data).into_owned())
            .unwrap();
        assert!(xml.contains(element), "{kind}: {xml}");
    }
}

// source: [MS-PPTX] p159:morph — a duplicated slide keeps its shape names: every shape is a pair,
// the moved one goes from its old box to its new one; a new shape fades in.
#[test]
fn morph_pairs_shapes_of_consecutive_slides() {
    let mut e = open("slide-shapes-presets.pptx");
    let n = e.presentation().unwrap().slides[0].data.shapes.len();
    run(&mut e, r#"{"cmd":"duplicateSlide","index":0}"#);
    let id = free_ids(&e, 1)[0];
    run(&mut e, &format!(r#"{{"cmd":"select","ids":[{id}]}}"#));
    run(
        &mut e,
        r#"{"cmd":"transform","transform":{"kind":"move","dx":120,"dy":-30},"snap":false}"#,
    );
    run(&mut e, r#"{"cmd":"insertShape","preset":"star5"}"#);
    let r = run(
        &mut e,
        r#"{"cmd":"setTransition","transition":{"kind":"morph","dir":"byWord"}}"#,
    );
    let t = r.transition.as_ref().unwrap();
    assert_eq!(
        (t.kind.as_str(), t.dir.as_deref()),
        ("morph", Some("byWord"))
    );
    let show = run(&mut e, r#"{"cmd":"show","slide":1}"#).show.unwrap();
    let m = show.morph.expect("morph");
    assert_eq!(m.option, "byWord");
    // By word: the shapes pair as objects (without their text), their words as pairs of their own
    // (found unchanged on the duplicated slide: none fade).
    let words = m.pairs.iter().filter(|p| p.clip.is_some()).count();
    assert!(words > 0);
    assert!(m.gone_text.is_empty() && m.new_text.is_empty());
    let shapes: Vec<_> = m.pairs.iter().filter(|p| p.clip.is_none()).collect();
    assert_eq!(shapes.len(), n);
    assert_eq!(m.gone.len(), 0);
    assert_eq!(m.new.len(), 1);
    let moved: Vec<_> = shapes
        .iter()
        .filter(|p| {
            (p.to[0] - p.from[0] - 120.0).abs() < 0.01 && (p.to[1] - p.from[1] + 30.0).abs() < 0.01
        })
        .collect();
    assert_eq!(moved.len(), 1);
    assert_eq!(shapes.iter().filter(|p| p.from == p.to).count(), n - 1);
    // The first slide has no morph; saved and read back, the transition stays a morph.
    assert!(run(&mut e, r#"{"cmd":"show","slide":0}"#)
        .show
        .unwrap()
        .morph
        .is_none());
    let back = reopen(&e);
    let t = back.presentation().unwrap().slides[1]
        .transition
        .clone()
        .unwrap();
    assert_eq!(t.kind, Some(TransitionKind::Other("p159:morph".into())));
    assert_eq!(t.dir.as_deref(), Some("byWord"));
}

// source: ECMA-376 Part 1, §19.5 — catalog effects of every class are written as behaviours,
// reported with what they do (the played tracks) and read back as themselves.
#[test]
fn entrance_emphasis_exit_and_path_effects() {
    let mut e = open("slide-shapes-presets.pptx");
    let ids = free_ids(&e, 0);
    let add = |e: &mut Engine, id: u32, spec: &str| {
        run(e, &format!(r#"{{"cmd":"select","ids":[{id}]}}"#));
        run(e, &format!(r#"{{"cmd":"addAnimation",{spec}}}"#))
    };
    add(&mut e, ids[0], r#""effect":"zoom","direction":"out""#);
    add(
        &mut e,
        ids[1],
        r#""effect":"spin","direction":"ccw","trigger":"afterPrevious","durMs":1200"#,
    );
    add(&mut e, ids[2], r#""effect":"lineRight""#);
    let r = add(
        &mut e,
        ids[3],
        r#""effect":"checkerboard","class":"exit","direction":"down""#,
    );
    let a = &r.animations;
    assert_eq!(
        a.iter()
            .map(|x| (x.effect.as_str(), x.class))
            .collect::<Vec<_>>(),
        vec![
            ("zoom", "entr"),
            ("spin", "emph"),
            ("lineRight", "path"),
            ("checkerboard", "exit")
        ]
    );
    assert_eq!(a[0].dirs, vec!["in", "out"]);
    assert_eq!(a[1].direction.as_deref(), Some("ccw"));
    // What they do: zoom scales and fades in, spin turns −360° over 1.2 s, the path goes right.
    let kinds = |i: usize| {
        a[i].play
            .tracks
            .iter()
            .map(|t| {
                serde_json::to_value(t).unwrap()["kind"]
                    .as_str()
                    .unwrap()
                    .to_owned()
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(kinds(0), vec!["set", "anim", "anim", "filter"]);
    assert_eq!(kinds(1), vec!["rotate"]);
    assert!((a[1].play.dur_ms - 1200.0).abs() < 1e-9);
    assert_eq!(kinds(2), vec!["motion"]);
    assert_eq!(kinds(3), vec!["filter", "set"]);
    // Saved and read back: the same effects, variants and classes.
    let back = reopen(&e);
    let anims = &back.presentation().unwrap().slides[0]
        .timing
        .as_ref()
        .unwrap()
        .animations;
    let got: Vec<(String, Option<String>)> = anims
        .iter()
        .map(|x| (format!("{:?}", x.effect), x.direction.clone()))
        .collect();
    assert_eq!(
        got,
        vec![
            ("Named(\"zoom\")".into(), Some("out".into())),
            ("Named(\"spin\")".into(), Some("ccw".into())),
            ("Named(\"lineRight\")".into(), None),
            ("Named(\"checkerboard\")".into(), Some("down".into())),
        ]
    );
    // Changing the effect of an emphasis to an entrance brings the shape in again.
    let r = run(
        &mut e,
        r#"{"cmd":"setAnimation","index":1,"effect":"bounce"}"#,
    );
    assert_eq!(
        (r.animations[1].effect.as_str(), r.animations[1].class),
        ("bounce", "entr")
    );
    let r = try_run(&mut e, r#"{"cmd":"addAnimation","effect":"nope"}"#);
    assert_eq!(r.error.as_deref(), Some("badEffect"));
}

// source: ECMA-376 Part 1, §19.5.29 cTn@repeatCount, §19.5.65 seq nodeType="interactiveSeq" (a
// click on the trigger shape starts it) — repeats, triggers and the animation painter.
#[test]
fn repeats_triggers_and_the_animation_painter() {
    let mut e = open("slide-shapes-presets.pptx");
    let ids = free_ids(&e, 0);
    let (a, b, c, d) = (ids[0], ids[1], ids[2], ids[3]);
    run(&mut e, &format!(r#"{{"cmd":"select","ids":[{a}]}}"#));
    run(
        &mut e,
        r#"{"cmd":"addAnimation","effect":"spin","durMs":400,"repeat":3}"#,
    );
    run(
        &mut e,
        r#"{"cmd":"addAnimation","effect":"pulse","repeat":0,"trigger":"afterPrevious"}"#,
    );
    // Shape b appears when shape c is clicked.
    run(&mut e, &format!(r#"{{"cmd":"select","ids":[{b}]}}"#));
    let r = run(
        &mut e,
        &format!(r#"{{"cmd":"addAnimation","effect":"zoom","triggerShape":{c}}}"#),
    );
    assert_eq!(r.animations[2].trigger_shape, Some(c));
    assert_eq!(r.animations[0].repeat, 3);
    assert_eq!(r.animations[1].repeat, 0);
    // Three runs of 400 ms; the trigger's effect is not a click step of the slide.
    assert_eq!(
        (r.animations[0].start_ms, r.animations[0].end_ms),
        (0, 1200)
    );
    let show = run(&mut e, r#"{"cmd":"show","slide":0}"#).show.unwrap();
    assert_eq!(show.clicks, 1);
    assert_eq!(show.triggers.len(), 1);
    assert_eq!(show.triggers[0].shape_id, c);
    // Painter: shape d gets shape a's two effects.
    run(&mut e, &format!(r#"{{"cmd":"select","ids":[{d}]}}"#));
    let r = run(&mut e, &format!(r#"{{"cmd":"copyAnimations","from":{a}}}"#));
    assert_eq!(r.animations.len(), 5);
    assert_eq!(
        r.animations[3..]
            .iter()
            .map(|x| (x.shape_id, x.effect.as_str(), x.repeat))
            .collect::<Vec<_>>(),
        vec![(d, "spin", 3), (d, "pulse", 0)]
    );
    // Saved and read back.
    let back = reopen(&e);
    let t = back.presentation().unwrap().slides[0]
        .timing
        .clone()
        .unwrap();
    let got: Vec<(u32, Option<u32>, Option<u32>)> = t
        .animations
        .iter()
        .map(|x| (x.shape_id, x.repeat, x.trigger_shape))
        .collect();
    assert_eq!(
        got,
        vec![
            (a, Some(3000), None),
            (a, Some(u32::MAX), None),
            (d, Some(3000), None),
            (d, Some(u32::MAX), None),
            (b, None, Some(c)),
        ]
    );
    let r = try_run(&mut e, &format!(r#"{{"cmd":"copyAnimations","from":{c}}}"#));
    assert_eq!(r.error.as_deref(), Some("noAnimation"));
}
