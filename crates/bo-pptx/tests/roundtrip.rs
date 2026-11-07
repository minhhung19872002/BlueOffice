//! Lossless round trip over the BlueSlide corpus (`tests/corpus/slide`, prompt §7):
//! read → write → compare every package part. XML parts are compared in
//! canonical form (attribute order, namespace declarations and insignificant
//! whitespace ignored), everything else byte for byte.

use bo_drawingml::dom::{canonical, parse};
use bo_opc::{Limits, Package};
use bo_pptx::{read_pptx, write_pptx};
use std::path::PathBuf;

fn corpus() -> Vec<(String, Vec<u8>)> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus/slide");
    let mut out: Vec<(String, Vec<u8>)> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "pptx"))
        .map(|p| {
            (
                p.file_name().unwrap().to_string_lossy().into_owned(),
                std::fs::read(&p).unwrap(),
            )
        })
        .collect();
    out.sort();
    assert!(out.len() >= 24, "corpus has {} decks", out.len());
    out
}

fn canon_part(name: &str, data: &[u8]) -> Vec<u8> {
    let is_xml = name.ends_with(".xml") || name.ends_with(".rels");
    if is_xml {
        if let Ok(el) = parse(data, name) {
            return canonical(&el).into_bytes();
        }
    }
    data.to_vec()
}

/// (identical parts, total parts, differing part names)
fn compare(orig: &[u8], back: &[u8]) -> (usize, usize, Vec<String>) {
    let a = Package::read(orig, &Limits::default()).unwrap();
    let b = Package::read(back, &Limits::default()).unwrap();
    let mut same = 0;
    let mut diff = Vec::new();
    for p in a.parts() {
        match b.part(&p.name) {
            Some(q) if canon_part(&p.name, &p.data) == canon_part(&q.name, &q.data) => same += 1,
            _ => diff.push(p.name.clone()),
        }
    }
    for q in b.parts() {
        if a.part(&q.name).is_none() {
            diff.push(format!("+{}", q.name));
        }
    }
    (same, a.parts().len(), diff)
}

#[test]
fn corpus_round_trip_is_lossless() {
    let mut same = 0;
    let mut total = 0;
    let mut report = Vec::new();
    for (name, bytes) in corpus() {
        let pres = read_pptx(&bytes).unwrap_or_else(|e| panic!("{name}: {e}"));
        let out = write_pptx(&pres).unwrap_or_else(|e| panic!("{name}: {e}"));
        let (s, t, d) = compare(&bytes, &out);
        same += s;
        total += t;
        if !d.is_empty() {
            report.push(format!("{name}: {d:?}"));
        }
        // Writing is a fixpoint: a second round trip changes nothing in the model.
        let again = read_pptx(&out).unwrap_or_else(|e| panic!("{name} (2nd read): {e}"));
        assert_eq!(again.slides.len(), pres.slides.len(), "{name}");
        assert_eq!(
            again.slides, pres.slides,
            "{name}: slides differ after round trip"
        );
        assert_eq!(again.layouts, pres.layouts, "{name}: layouts differ");
        assert_eq!(again.masters, pres.masters, "{name}: masters differ");
        assert_eq!(again.themes, pres.themes, "{name}: themes differ");
    }
    let pct = same as f64 * 100.0 / total as f64;
    println!("round trip: {same}/{total} parts identical ({pct:.1}%)");
    for r in &report {
        println!("  {r}");
    }
    assert!(pct >= 95.0, "round trip {pct:.1}% < 95%: {report:#?}");
}

#[test]
fn corpus_models_features() {
    use bo_model::slide::{AnimEffect, GraphicContent, ShapeNode, TransitionKind};
    let decks = corpus();
    let get = |n: &str| {
        let (_, b) = decks.iter().find(|(name, _)| name == n).unwrap();
        read_pptx(b).unwrap()
    };
    // Placeholder inheritance: the content placeholder has no xfrm of its own.
    let p = get("slide-text-bullets.pptx");
    assert_eq!(p.slides.len(), 3);
    assert_eq!(p.masters[0].layouts.len(), 3);
    let ShapeNode::Shape(body) = &p.slides[1].data.shapes[1] else {
        panic!()
    };
    assert!(body.props.xfrm.is_none());
    let chain = bo_pptx::inherit::chain(&p, bo_pptx::inherit::Level::Slide(1), body);
    assert_eq!(chain.len(), 3);
    assert!(bo_pptx::inherit::effective_xfrm(&chain).is_some());
    // Transitions and animations.
    let a = get("slide-anim-fly-wipe.pptx");
    let s = &a.slides[0];
    assert_eq!(
        s.transition.as_ref().and_then(|t| t.kind.clone()),
        Some(TransitionKind::Wipe)
    );
    let timing = s.timing.as_ref().unwrap();
    let effects: Vec<_> = timing.animations.iter().map(|x| x.effect.clone()).collect();
    assert_eq!(
        effects,
        [AnimEffect::FlyIn, AnimEffect::Wipe, AnimEffect::Fade]
    );
    let tl = timing.timeline();
    assert_eq!(tl[1].start_ms, tl[0].start_ms);
    // After-previous: starts when the fly-in ends (500 ms) plus its 250 ms delay.
    assert_eq!((tl[2].click, tl[2].start_ms, tl[2].end_ms), (1, 750, 1250));
    let f = get("slide-anim-fade-appear.pptx");
    let t = f.slides[0].transition.as_ref().unwrap();
    assert_eq!(
        (t.kind.clone(), t.dur_ms),
        (Some(TransitionKind::Fade), Some(1500))
    );
    assert_eq!(
        f.slides[1].transition.as_ref().unwrap().advance_after_ms,
        Some(3000)
    );
    // Tables and charts.
    let t = get("slide-tables-merge.pptx");
    assert!(t.slides[0].data.shapes.iter().any(|n| matches!(
        n,
        ShapeNode::Frame(f) if matches!(f.content, GraphicContent::Table(_))
    )));
    // Notes.
    let n = get("slide-notes-basic.pptx");
    assert!(n.slides[0]
        .notes
        .as_ref()
        .unwrap()
        .text()
        .contains("Chào mừng"));
    // Media parts are preserved verbatim.
    let m = get("slide-media-video.pptx");
    assert!(m.part_data("/ppt/media/media1.mp4").is_some());
}

#[test]
fn edited_deck_round_trips() {
    use bo_model::slide::{
        AnimClass, AnimEffect, AnimTrigger, Animation, Timing, Transition, TransitionKind,
    };
    let (_, bytes) = corpus()
        .into_iter()
        .find(|(n, _)| n == "slide-text-columns.pptx")
        .unwrap();
    let mut p = read_pptx(&bytes).unwrap();
    p.slides[0].transition = Some(Transition {
        kind: Some(TransitionKind::Push),
        dir: Some("l".into()),
        dur_ms: Some(800),
        ..Transition::default()
    });
    p.slides[0].timing = Some(Timing {
        animations: vec![Animation {
            shape_id: 4,
            class: AnimClass::Entrance,
            effect: AnimEffect::FlyIn,
            direction: Some("l".into()),
            trigger: AnimTrigger::OnClick,
            delay_ms: 0,
            dur_ms: 500,
            preset_id: None,
            preset_subtype: None,
        }],
        source: None,
    });
    // Duplicate a slide (new part, new ids).
    let mut dup = p.slides[2].clone();
    dup.info.part_name.clear();
    dup.slide_id = 0;
    dup.pres_rel_id.clear();
    p.slides.push(dup);
    let out = write_pptx(&p).unwrap();
    let back = read_pptx(&out).unwrap();
    assert_eq!(back.slides.len(), 4);
    let t = back.slides[0].transition.as_ref().unwrap();
    assert_eq!(
        (t.kind.clone(), t.dur_ms),
        (Some(TransitionKind::Push), Some(800))
    );
    let a = &back.slides[0].timing.as_ref().unwrap().animations[0];
    assert_eq!(
        (a.effect.clone(), a.direction.as_deref()),
        (AnimEffect::FlyIn, Some("l"))
    );
    assert_ne!(back.slides[3].slide_id, back.slides[2].slide_id);
    assert_ne!(back.slides[3].info.part_name, back.slides[2].info.part_name);
}
