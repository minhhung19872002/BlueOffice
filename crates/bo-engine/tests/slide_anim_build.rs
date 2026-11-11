//! BlueSlide builds and effect options (SLIDE-ANIM-004): text by paragraph and outline level,
//! charts by series / category / element, SmartArt node by node; after-animation (dim, hide,
//! hide on next click), rewind and effect sounds — set through the JSON commands, played in the
//! slide show, saved and read back.
// Tests may unwrap: a panic is a test failure.
#![allow(clippy::unwrap_used)]

use bo_engine::slide_edit::{SlideCommand, SlideEditResult};
use bo_engine::Engine;
use bo_model::slide::{AnimAfter, AnimTarget, AnimTrigger, Build, GraphicSub};
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

fn timing(e: &Engine, slide: usize) -> bo_model::slide::Timing {
    e.presentation().unwrap().slides[slide]
        .timing
        .clone()
        .unwrap()
}

/// The effects without what saving fills in (preset ids written for the effect).
fn norm(t: &bo_model::slide::Timing) -> Vec<bo_model::slide::Animation> {
    t.animations
        .iter()
        .cloned()
        .map(|mut a| {
            a.preset_id = None;
            a.preset_subtype = None;
            a.source = None;
            a
        })
        .collect()
}

fn paragraphs(e: &Engine, slide: usize) -> Vec<(u32, u32, AnimTrigger)> {
    timing(e, slide)
        .animations
        .iter()
        .filter_map(|a| match a.opts.target {
            Some(AnimTarget::Paragraphs { start, end }) => Some((start, end, a.trigger)),
            _ => None,
        })
        .collect()
}

// source: ECMA-376 Part 1, §19.5 bldP (build `p`, bldLvl, rev, animBg) — slide 2 of the bullets
// deck has paragraphs at levels 1 2 2 3 1 2 4 5: by 1st level two steps (0–3, 4–7), by 2nd level
// five (0, 1, 2–3, 4, 5–7); the show plays each step on its own piece of the text.
#[test]
fn text_builds_paragraph_by_paragraph() {
    let mut e = open("slide-text-bullets.pptx");
    run(&mut e, r#"{"cmd":"goTo","slide":1}"#);
    run(&mut e, r#"{"cmd":"select","ids":[3]}"#);
    run(
        &mut e,
        r#"{"cmd":"addAnimation","effect":"fade","durMs":400}"#,
    );
    let r = run(&mut e, r#"{"cmd":"setAnimation","index":0,"build":"p1"}"#);
    let c = AnimTrigger::OnClick;
    assert_eq!(paragraphs(&e, 1), vec![(0, 3, c), (4, 7, c)]);
    let a = &r.animations;
    let b = a[0].build.as_ref().unwrap();
    assert_eq!(
        (b.kind, b.value.as_str(), b.head, b.levels),
        ("text", "p1", true, 5)
    );
    assert!(!a[1].build.as_ref().unwrap().head);
    assert_eq!(a[1].target.as_ref().unwrap().start, Some(4));
    // By 2nd level, in reverse order.
    run(
        &mut e,
        r#"{"cmd":"setAnimation","index":1,"build":"p2","buildReverse":true}"#,
    );
    assert_eq!(
        paragraphs(&e, 1),
        vec![(5, 7, c), (4, 4, c), (2, 3, c), (1, 1, c), (0, 0, c)]
    );
    let t = timing(&e, 1);
    assert!(matches!(&t.builds[..], [Build::Paragraph(p)]
        if p.build == "p" && p.level == 2 && p.reverse && !p.anim_bg));
    // The show: the layer is cut into the body (the placeholder without its text) and the five
    // groups of paragraphs; each effect plays on its own piece, five clicks.
    let s = run(&mut e, r#"{"cmd":"show","slide":1}"#).show.unwrap();
    assert_eq!(s.clicks, 5);
    let li = s.animations[0].layer.unwrap();
    let parts = s.layers[li].parts.as_ref().expect("pieces");
    assert_eq!(parts.pages.len(), 6);
    let mut seen: Vec<usize> = s.animations.iter().flat_map(|a| a.parts.clone()).collect();
    seen.sort_unstable();
    assert_eq!(seen, vec![1, 2, 3, 4, 5]);
    // Every piece draws text, and its box is inside the shape's: "5–7" lower than "0".
    let glyphs = |p: &bo_render::PageList| {
        p.commands
            .iter()
            .filter(|c| matches!(c, bo_render::Command::Glyphs { .. }))
            .count()
    };
    assert_eq!(glyphs(&parts.pages[0]), 0, "the body has no text");
    for p in &parts.pages[1..] {
        assert!(glyphs(p) > 0);
    }
    let top = |i: usize| s.animations[i].rect.unwrap()[1];
    assert!(top(0) > top(4), "{} {}", top(0), top(4));
    // The pieces together draw the shape: as many glyph runs as the whole layer.
    let whole = run(
        &mut open("slide-text-bullets.pptx"),
        r#"{"cmd":"show","slide":1}"#,
    )
    .show
    .unwrap();
    let all: usize = whole.layers.iter().map(|l| glyphs(&l.page)).sum();
    let split: usize = s
        .layers
        .iter()
        .map(|l| match &l.parts {
            Some(p) => p.pages.iter().map(glyphs).sum(),
            None => glyphs(&l.page),
        })
        .sum();
    assert_eq!(split, all);
    // With the shape first (animBg) and all at once.
    run(
        &mut e,
        r#"{"cmd":"setAnimation","index":0,"build":"allAtOnce","buildBg":true,"buildReverse":false}"#,
    );
    let t = timing(&e, 1);
    assert_eq!(t.animations[0].opts.target, Some(AnimTarget::Background));
    assert_eq!(t.animations.len(), 1 + 8);
    assert!(t.animations[1..]
        .iter()
        .all(|a| a.trigger == AnimTrigger::WithPrevious));
    let s = run(&mut e, r#"{"cmd":"show","slide":1}"#).show.unwrap();
    assert_eq!(s.clicks, 1);
    assert_eq!(
        s.animations[0].parts,
        vec![0],
        "the background step plays on the body"
    );
    // Saved and read back; editing the effect changes the whole build.
    let mut back = reopen(&e);
    let key = |t: &bo_model::slide::Timing| {
        t.animations
            .iter()
            .map(|a| (a.shape_id, a.opts.clone(), a.trigger, a.effect.clone()))
            .collect::<Vec<_>>()
    };
    assert_eq!(key(&timing(&back, 1)), key(&timing(&e, 1)));
    assert_eq!(timing(&back, 1).builds, timing(&e, 1).builds);
    run(&mut back, r#"{"cmd":"goTo","slide":1}"#);
    run(
        &mut back,
        r#"{"cmd":"setAnimation","index":3,"effect":"wipe","durMs":900}"#,
    );
    let t = timing(&back, 1);
    assert!(t
        .animations
        .iter()
        .all(|a| a.effect == bo_model::slide::AnimEffect::Wipe && a.dur_ms == 900));
    // Back to one object: one effect, the build says so.
    run(
        &mut back,
        r#"{"cmd":"setAnimation","index":4,"build":"whole"}"#,
    );
    let t = timing(&back, 1);
    assert_eq!(t.animations.len(), 1);
    assert_eq!(t.animations[0].opts.target, None);
    let r = run(&mut back, r#"{"cmd":"show","slide":1}"#);
    assert!(r.show.unwrap().layers.iter().all(|l| l.parts.is_none()));
    // A picture cannot be built; a bad value is refused.
    let r = try_run(
        &mut back,
        r#"{"cmd":"setAnimation","index":0,"build":"p9"}"#,
    );
    assert_eq!(r.error.as_deref(), Some("badBuild"));
}

// source: DrawingML §20.1.2.2 bldChart (by series with the chart background first) and bldDgm
// (one by one); §19.5 graphicEl targets (a:chart seriesIdx / bldStep, a:dgm id) — written, read
// back and played on pieces of the chart / SmartArt.
#[test]
fn charts_and_smartart_build_by_element() {
    let mut e = open("slide-text-bullets.pptx");
    let chart_slide = run(&mut e, r#"{"cmd":"addSlide"}"#).slide;
    run(
        &mut e,
        r#"{"cmd":"insertChart","kind":"column","data":{"categories":["Q1","Q2","Q3"],"series":[{"name":"A","values":[3,5,4]},{"name":"B","values":[4,6,2]}]},"rect":{"x":40,"y":40,"w":400,"h":300}}"#,
    );
    run(
        &mut e,
        r#"{"cmd":"addAnimation","effect":"wipe","direction":"b"}"#,
    );
    let r = run(
        &mut e,
        r#"{"cmd":"setAnimation","index":0,"build":"series","buildBg":true}"#,
    );
    assert_eq!(r.animations.len(), 3);
    let b = r.animations[0].build.as_ref().unwrap();
    assert_eq!((b.kind, b.value.as_str(), b.bg), ("chart", "series", true));
    assert_eq!(r.animations[2].target.as_ref().unwrap().series, Some(1));
    let s = run(
        &mut e,
        &format!(r#"{{"cmd":"show","slide":{chart_slide}}}"#),
    )
    .show
    .unwrap();
    assert_eq!(s.clicks, 3);
    let li = s.animations[1].layer.unwrap();
    let parts = s.layers[li].parts.as_ref().expect("chart pieces");
    // The body (axes, legend) and one piece per series, three bars each.
    assert_eq!(parts.pages.len(), 3);
    let bars = |p: &bo_render::PageList| {
        p.commands
            .iter()
            .filter(|c| {
                matches!(
                    c,
                    bo_render::Command::FillRect { .. } | bo_render::Command::Path { .. }
                )
            })
            .count()
    };
    assert_eq!(bars(&parts.pages[1]), 3);
    assert_eq!(bars(&parts.pages[2]), 3);
    assert_eq!(s.animations[0].parts, vec![0]);
    assert_eq!(s.animations[2].parts, vec![2]);
    // By element in category: 6 steps after the background.
    run(
        &mut e,
        r#"{"cmd":"setAnimation","index":0,"build":"categoryEl"}"#,
    );
    assert_eq!(timing(&e, chart_slide).animations.len(), 7);

    // SmartArt one by one.
    let art_slide = run(&mut e, r#"{"cmd":"addSlide"}"#).slide;
    run(
        &mut e,
        r#"{"cmd":"insertSmartArt","layout":"process","items":[{"text":"Tiếp nhận"},{"text":"Xử lý"},{"text":"Trả kết quả"}]}"#,
    );
    run(&mut e, r#"{"cmd":"addAnimation","effect":"fade"}"#);
    let r = run(&mut e, r#"{"cmd":"setAnimation","index":0,"build":"one"}"#);
    assert_eq!(r.animations.len(), 3);
    assert_eq!(r.animations[1].target.as_ref().unwrap().node, Some(2));
    let s = run(&mut e, &format!(r#"{{"cmd":"show","slide":{art_slide}}}"#))
        .show
        .unwrap();
    assert_eq!(s.clicks, 3);
    let li = s.animations[0].layer.unwrap();
    let parts = s.layers[li].parts.as_ref().expect("SmartArt pieces");
    assert_eq!(parts.pages.len(), 4, "the arrows stay in the body");
    run(
        &mut e,
        r#"{"cmd":"setAnimation","index":0,"build":"lvlAtOnce","buildReverse":true}"#,
    );
    let t = timing(&e, art_slide);
    assert!(matches!(t.builds.last(), Some(Build::Graphic(g))
        if matches!(&g.sub, Some(GraphicSub::Diagram { build, reverse }) if build == "lvlAtOnce" && *reverse)));
    // All three nodes are on level 1: one click.
    assert_eq!(
        t.animations
            .iter()
            .filter(|a| a.trigger == AnimTrigger::OnClick)
            .count(),
        1
    );
    // Saved and read back.
    let back = reopen(&e);
    for s in [chart_slide, art_slide] {
        let (a, b) = (timing(&e, s), timing(&back, s));
        assert_eq!(norm(&a), norm(&b));
        assert_eq!(a.builds, b.builds);
    }
}

// source: §19.5.29 cTn@afterEffect behaviours (dim to a colour, hide), the after-effect node of
// "hide on next click", cTn@fill `remove` (rewind) and the effect sound (§19.5 subTnLst → audio →
// sndTgt) — set, shown to the pane and the show, saved and read back.
#[test]
fn after_animation_rewind_and_sound() {
    use base64::Engine as _;
    let mut e = open("slide-text-bullets.pptx");
    run(&mut e, r#"{"cmd":"goTo","slide":1}"#);
    run(&mut e, r#"{"cmd":"select","ids":[3]}"#);
    run(&mut e, r#"{"cmd":"addAnimation","effect":"fade"}"#);
    run(&mut e, r#"{"cmd":"setAnimation","index":0,"build":"p1"}"#);
    // Dim each paragraph once done (the whole build), rewind the second one.
    let r = run(
        &mut e,
        r#"{"cmd":"setAnimation","index":0,"after":"dim","dimColor":"A0A0A0"}"#,
    );
    assert!(r
        .animations
        .iter()
        .all(|a| a.after == Some("dim") && a.dim_color.as_deref() == Some("A0A0A0")));
    let t = timing(&e, 1);
    assert_eq!(
        t.animations[1].opts.after,
        Some(AnimAfter::Dim {
            rgb: 0xA0_A0_A0,
            attrs: vec!["style.color".into()]
        })
    );
    // The show draws each dimmed piece in its dim colour.
    let s = run(&mut e, r#"{"cmd":"show","slide":1}"#).show.unwrap();
    let tint = s.animations[0].tint.as_ref().expect("dim drawing");
    assert!(tint.commands.iter().any(|c| matches!(c,
        bo_render::Command::Glyphs { color, .. } if (color.0, color.1, color.2) == (0xA0, 0xA0, 0xA0))));
    // Hide on the next click: one more click.
    run(
        &mut e,
        r#"{"cmd":"setAnimation","index":1,"after":"hideOnClick"}"#,
    );
    let s = run(&mut e, r#"{"cmd":"show","slide":1}"#).show.unwrap();
    assert_eq!(s.clicks, 3);
    assert!(s.animations.iter().all(|a| a.after == Some("hideOnClick")));
    // Hide once done, rewind, a sound.
    let shape_slide = run(&mut e, r#"{"cmd":"addSlide"}"#).slide;
    run(
        &mut e,
        r#"{"cmd":"insertShape","preset":"rect","rect":{"x":100,"y":100,"w":100,"h":80}}"#,
    );
    run(&mut e, r#"{"cmd":"addAnimation","effect":"flyIn"}"#);
    let wav = {
        // A tiny PCM WAV (RIFF header, 2 samples).
        let mut w = b"RIFF".to_vec();
        w.extend_from_slice(&40u32.to_le_bytes());
        w.extend_from_slice(b"WAVEfmt ");
        w.extend_from_slice(&16u32.to_le_bytes());
        w.extend_from_slice(&[1, 0, 1, 0, 0x22, 0x56, 0, 0, 0x44, 0xAC, 0, 0, 2, 0, 16, 0]);
        w.extend_from_slice(b"data");
        w.extend_from_slice(&4u32.to_le_bytes());
        w.extend_from_slice(&[0, 0, 0, 0]);
        base64::engine::general_purpose::STANDARD.encode(w)
    };
    let r = run(
        &mut e,
        &format!(
            r#"{{"cmd":"setAnimation","index":0,"after":"hide","rewind":true,"soundData":"{wav}","soundName":"chime.wav"}}"#
        ),
    );
    let a = &r.animations[0];
    assert_eq!((a.after, a.rewind), (Some("hide"), true));
    let snd = a.sound.as_ref().unwrap();
    assert_eq!(snd.name, "chime.wav");
    let part = snd.part.clone().unwrap();
    assert!(e.presentation().unwrap().part_data(&part).is_some());
    let back = reopen(&e);
    let (x, y) = (timing(&e, shape_slide), timing(&back, shape_slide));
    assert_eq!(norm(&x), norm(&y));
    assert!(y.animations[0].opts.rewind);
    assert_eq!(y.animations[0].opts.after, Some(AnimAfter::Hide));
    assert_eq!(
        timing(&back, 1).animations[1].opts.after,
        Some(AnimAfter::HideOnClick)
    );
    // Options off again.
    let mut back = back;
    run(
        &mut back,
        &format!(r#"{{"cmd":"goTo","slide":{shape_slide}}}"#),
    );
    let r = run(
        &mut back,
        r#"{"cmd":"setAnimation","index":0,"after":"none","rewind":false,"sound":"none"}"#,
    );
    let a = &r.animations[0];
    assert!(a.after.is_none() && !a.rewind && a.sound.is_none());
    let r = try_run(
        &mut back,
        r#"{"cmd":"setAnimation","index":0,"after":"later"}"#,
    );
    assert_eq!(r.error.as_deref(), Some("badAfter"));
    let r = try_run(
        &mut back,
        r#"{"cmd":"setAnimation","index":0,"after":"dim","dimColor":"xyz"}"#,
    );
    assert_eq!(r.error.as_deref(), Some("badColor"));
}
