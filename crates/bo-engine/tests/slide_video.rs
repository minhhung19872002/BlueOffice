//! Slide show export as video frames and animated GIF (SLIDE-EXPORT-001): the frame
//! generator is deterministic, follows the timeline and samples transitions and effects.
// Tests may unwrap: a panic is a test failure.
#![allow(clippy::unwrap_used)]

use bo_engine::slide_edit::video::VideoOptions;
use bo_engine::slide_edit::SlideCommand;
use bo_engine::Engine;
use std::path::PathBuf;

fn open(name: &str) -> Engine {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus/slide")
        .join(name);
    let mut e = Engine::with_bundled_fonts();
    e.open(&std::fs::read(&p).unwrap(), name).unwrap();
    e
}

fn run(e: &mut Engine, json: &str) {
    let cmd: SlideCommand = serde_json::from_str(json).unwrap();
    let r = e.slide_command(cmd).unwrap();
    assert!(r.error.is_none(), "{json}: {:?}", r.error);
}

fn opts(width: u32, fps: u32, secs: f64) -> VideoOptions {
    VideoOptions {
        width,
        fps,
        seconds_per_slide: secs,
        ..VideoOptions::default()
    }
}

#[test]
fn frames_are_deterministic_and_follow_the_timeline() {
    let mut e = open("slide-text-bullets.pptx");
    let n = e.slide_count();
    assert!(n >= 2);
    run(
        &mut e,
        r#"{"cmd":"setTransition","slides":[1],"transition":{"kind":"push","dir":"l","durMs":1000}}"#,
    );
    let info = e.video_begin(opts(320, 10, 1.0)).unwrap();
    assert_eq!((info.width, info.height % 2), (320, 0));
    // One second per slide, plus the second of the push into slide 2.
    assert_eq!(info.duration_ms, n as u64 * 1000 + 1000);
    assert_eq!(u64::from(info.frame_count), info.duration_ms * 10 / 1000);
    assert_eq!(info.slides[1].start_ms, 1000);
    assert_eq!(info.slides[1].transition_ms, 1000);
    let len = (info.width * info.height * 4) as usize;
    let f3 = e.video_frame(3).unwrap();
    assert_eq!(f3.len(), len);
    assert_eq!(e.video_frame(3).unwrap(), f3);
    // Transition: start = the outgoing slide, middle differs from both ends, end = the slide.
    let before = e.video_frame(9).unwrap();
    let start = e.video_frame(10).unwrap();
    let mid = e.video_frame(15).unwrap();
    let end = e.video_frame(20).unwrap();
    assert_eq!(start, before);
    assert_ne!(mid, start);
    assert_ne!(mid, end);
    assert_ne!(start, end);
    assert_eq!(e.video_frame(25).unwrap(), end);
    assert!(e.video_frame(info.frame_count).is_err());
    // A new export of the same deck gives the same pixels.
    e.video_end();
    let again = e.video_begin(opts(320, 10, 1.0)).unwrap();
    assert_eq!(again, info);
    assert_eq!(e.video_frame(15).unwrap(), mid);
    assert!(e.video_frame_png(0).unwrap().starts_with(b"\x89PNG"));
    e.video_end();
    assert!(e.video_frame(0).is_err());
}

#[test]
fn entrance_effects_are_sampled() {
    let mut e = open("slide-shapes-presets.pptx");
    let p = e.presentation().unwrap();
    let id = bo_engine::slide_edit::geom::place_slide(p, 0)[0].id;
    run(
        &mut e,
        &format!(r#"{{"cmd":"select","slide":0,"ids":[{id}]}}"#),
    );
    run(
        &mut e,
        r#"{"cmd":"addAnimation","effect":"fade","durMs":1000}"#,
    );
    let info = e.video_begin(opts(240, 10, 2.0)).unwrap();
    assert_eq!(info.slides[0].length_ms, 2000);
    let hidden = e.video_frame(0).unwrap();
    let half = e.video_frame(5).unwrap();
    let shown = e.video_frame(12).unwrap();
    assert_ne!(hidden, half);
    assert_ne!(half, shown);
    assert_eq!(e.video_frame(19).unwrap(), shown);
}

#[test]
fn the_show_exports_as_an_animated_gif() {
    let mut e = open("slide-text-bullets.pptx");
    run(
        &mut e,
        r#"{"cmd":"setTransition","slides":[1],"transition":{"kind":"fade","durMs":1000}}"#,
    );
    let o = VideoOptions {
        dither: false,
        ..opts(160, 5, 1.0)
    };
    let info = e.video_begin(o.clone()).unwrap();
    let first = e.video_frame(0).unwrap();
    let mid = e.video_frame(7).unwrap();
    e.video_end();
    let gif = e.export_show_gif(o).unwrap();
    let anim = bo_raster::gif::decode_animation(&gif).unwrap();
    assert_eq!((anim.width, anim.height), (info.width, info.height));
    assert_eq!(anim.repeat, Some(0));
    // Identical frames are merged; the delays add up to the show.
    let total: u64 = anim.frames.iter().map(|f| u64::from(f.delay_cs)).sum();
    assert_eq!(total, info.duration_ms / 10);
    assert!(anim.frames.len() < info.frame_count as usize);
    assert!(anim.frames.len() >= 3);
    let close = |a: &[u8], b: &[u8]| {
        let d: u64 = a
            .iter()
            .zip(b)
            .map(|(x, y)| u64::from(x.abs_diff(*y)))
            .sum();
        d as f64 / a.len() as f64
    };
    assert!(close(&anim.frames[0].rgba, &first) < 3.0);
    // The 1 s on slide 1 and the first fade frame (still slide 1) are one GIF frame.
    assert_eq!(anim.frames[0].delay_cs, 120);
    assert!(anim.frames.iter().any(|f| close(&f.rgba, &mid) < 3.0));
    assert!(anim.frames.iter().all(|f| f.palette.len() <= 256));
}
