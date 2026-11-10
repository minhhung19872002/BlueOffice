//! Recorded slide shows (SLIDE-REC-001): narration, camera, ink with timing and recorded clicks
//! saved through `saveRecording`, written to the package (ECMA-376 Part 1, §19.5 audio
//! `isNarration`, cMediaNode `showWhenStopped`, §19.3.1.50 transition `advTm`, [MS-ODRAWXML]
//! InkML content parts), reopened, undone and cleared; rehearsed timings keep `advClick`.
// Tests may unwrap: a panic is a test failure.
#![allow(clippy::unwrap_used)]

use base64::Engine as _;
use bo_engine::Engine;
use bo_opc::{Limits, Package};
use std::path::PathBuf;

fn deck(name: &str) -> Vec<u8> {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus/slide")
        .join(name);
    std::fs::read(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

fn open_bytes(bytes: &[u8], name: &str) -> Engine {
    let mut e = Engine::with_bundled_fonts();
    e.open(bytes, name).unwrap();
    e
}

fn call(e: &mut Engine, json: &str) -> serde_json::Value {
    serde_json::from_str(&e.slide_command_json(json).unwrap()).unwrap()
}

fn run(e: &mut Engine, json: &str) -> serde_json::Value {
    let r = call(e, json);
    assert!(r.get("error").is_none(), "{json}: {:?}", r.get("error"));
    r
}

/// Save; with `BO_REC_OUT=<dir>` the file is also written there (OpenXmlValidator runs).
fn save(e: &Engine, name: &str) -> Vec<u8> {
    let bytes = e.save_pptx().unwrap();
    if let Ok(dir) = std::env::var("BO_REC_OUT") {
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(PathBuf::from(dir).join(name), &bytes).unwrap();
    }
    bytes
}

fn part_text(bytes: &[u8], name: &str) -> String {
    let pkg = Package::read(bytes, &Limits::default()).unwrap();
    String::from_utf8(pkg.part(name).unwrap().data.clone()).unwrap()
}

fn part_names(bytes: &[u8]) -> Vec<String> {
    let pkg = Package::read(bytes, &Limits::default()).unwrap();
    pkg.parts().iter().map(|p| p.name.clone()).collect()
}

fn b64(b: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(b)
}

/// A short WAV (16-bit mono, 8 kHz, 0.1 s of silence).
fn wav() -> Vec<u8> {
    let data = vec![0u8; 1600];
    let mut v = Vec::new();
    v.extend(b"RIFF");
    v.extend(&(36 + data.len() as u32).to_le_bytes());
    v.extend(b"WAVEfmt ");
    v.extend(&16u32.to_le_bytes());
    v.extend(&1u16.to_le_bytes());
    v.extend(&1u16.to_le_bytes());
    v.extend(&8000u32.to_le_bytes());
    v.extend(&16000u32.to_le_bytes());
    v.extend(&2u16.to_le_bytes());
    v.extend(&16u16.to_le_bytes());
    v.extend(b"data");
    v.extend(&(data.len() as u32).to_le_bytes());
    v.extend(data);
    v
}

fn mp4() -> Vec<u8> {
    let mut v = b"\x00\x00\x00\x18ftypmp42\x00\x00\x00\x00mp42isom".to_vec();
    v.extend(std::iter::repeat_n(0u8, 32));
    v
}

fn recording(slide: usize, clicks: &str, camera: bool) -> String {
    let cam = if camera {
        format!(
            r#","camera":{{"data":"{}","mime":"video/mp4","corner":"topRight","aspect":1.7777}}"#,
            b64(&mp4())
        )
    } else {
        String::new()
    };
    format!(
        concat!(
            r#"{{"slide":{slide},"durationMs":8000,"clicks":[{clicks}],"#,
            r#""ink":[{{"points":[[100,100],[140,120],[180,100]],"times":[3000,3050,3100],"color":"D7261E","width":2}}],"#,
            r#""narration":{{"data":"{wav}","mime":"audio/wav"}}{cam}}}"#
        ),
        slide = slide,
        clicks = clicks,
        wav = b64(&wav()),
        cam = cam,
    )
}

fn save_cmd(slides: &[String]) -> String {
    format!(
        r#"{{"cmd":"saveRecording","slides":[{}],"names":{{"narration":"Lời thuyết minh","camera":"Camera","ink":"Mực"}}}}"#,
        slides.join(",")
    )
}

fn rec_slide(r: &serde_json::Value, slide: u64) -> serde_json::Value {
    r["recording"]["slides"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["slide"] == slide)
        .cloned()
        .unwrap_or(serde_json::Value::Null)
}

/// (click, startMs) of the effects of `shape` on the current slide.
fn effect_at(r: &serde_json::Value, shape: u64) -> Vec<(u64, u64)> {
    r["animations"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|a| a["shapeId"] == shape)
        .map(|a| (a["click"].as_u64().unwrap(), a["startMs"].as_u64().unwrap()))
        .collect()
}

#[test]
fn recording_is_saved_reopened_undone_and_cleared() {
    let mut e = open_bytes(&deck("slide-edit-animations.pptx"), "anim.pptx");
    run(&mut e, r#"{"cmd":"goTo","slide":0}"#);
    let before = run(&mut e, r#"{"cmd":"recordingInfo"}"#);
    assert!(rec_slide(&before, 0).is_null());
    let parts_before = part_names(&save(&e, "rec-before.pptx")).len();

    // Click 1 at 2 s, click 2 (exit of shape 15) at 6 s; click 3 (wipe of 14) not reached.
    let r = run(&mut e, &save_cmd(&[recording(0, "2000,6000", true)]));
    let s = rec_slide(&r, 0);
    assert!(s["narration"].is_u64() && s["camera"].is_u64(), "{s}");
    assert_eq!(s["ink"].as_array().unwrap().len(), 1);
    assert_eq!(s["timingMs"], 8000);
    let ink_id = s["ink"][0].as_u64().unwrap();
    let narr = s["narration"].as_u64().unwrap();
    let cam = s["camera"].as_u64().unwrap();

    // The effects run on one timeline: recorded clicks at their times, the ink at 3 s.
    let r = run(&mut e, r#"{"cmd":"state"}"#);
    assert_eq!(effect_at(&r, 10), vec![(0, 2000)]);
    assert_eq!(effect_at(&r, 11), vec![(0, 2000)]);
    assert_eq!(effect_at(&r, 12), vec![(0, 3200)]);
    assert_eq!(effect_at(&r, 15), vec![(0, 6000)]);
    assert_eq!(effect_at(&r, ink_id), vec![(0, 3000)]);
    assert_eq!(effect_at(&r, 14), vec![(1, 300)]);
    // Clips: recorded, start with the slide; the narration icon is hidden in the show.
    let media = r["media"].as_array().unwrap();
    let n = media.iter().find(|m| m["id"] == narr).unwrap();
    assert_eq!(n["recorded"], "narration");
    assert_eq!(n["kind"], "audio");
    assert_eq!(n["mime"], "audio/wav");
    assert_eq!(n["playback"]["start"], "auto");
    assert_eq!(n["playback"]["hideWhenStopped"], true);
    let c = media.iter().find(|m| m["id"] == cam).unwrap();
    assert_eq!(c["recorded"], "camera");
    assert_eq!(c["playback"]["start"], "auto");
    // Top right corner, a quarter of the slide wide, 16:9.
    let rect = c["rect"].as_array().unwrap();
    let w = r["info"]["pages"][0]["width"].as_f64().unwrap();
    assert!((rect[2].as_f64().unwrap() - w * 0.25).abs() < 0.01);
    assert!((rect[0].as_f64().unwrap() + rect[2].as_f64().unwrap() - (w - 12.0)).abs() < 0.01);
    assert!((rect[1].as_f64().unwrap() - 12.0).abs() < 0.01);
    // The show setup plays narrations and uses timings (§19.2.1.30).
    assert_eq!(r["presenting"]["setup"]["narration"], true);
    assert_eq!(r["presenting"]["setup"]["useTimings"], true);

    let bytes = save(&e, "rec-saved.pptx");
    let xml = part_text(&bytes, "/ppt/slides/slide1.xml");
    assert!(xml.contains(r#"isNarration="1""#), "{xml}");
    assert!(xml.contains(r#"showWhenStopped="0""#));
    assert!(xml.contains(r#"advTm="8000""#));
    assert!(xml.contains("<p:video"));
    assert!(xml.contains("playFrom(0.0)"));
    assert!(xml.contains(bo_engine::slide_edit::record::REC_EXT_URI));
    let pres = part_text(&bytes, "/ppt/presentation.xml");
    assert!(
        pres.contains(r#"showNarration="1""#) || {
            let props = part_text(&bytes, "/ppt/presProps.xml");
            props.contains(r#"showNarration="1""#)
        }
    );
    let names = part_names(&bytes);
    assert!(names.iter().any(|n| n.starts_with("/ppt/ink/")));
    assert!(names
        .iter()
        .any(|n| n.starts_with("/ppt/media/") && n.ends_with(".wav")));
    assert!(names
        .iter()
        .any(|n| n.starts_with("/ppt/media/") && n.ends_with(".mp4")));

    // Reopened: the same recording, ink times kept (InkML channel T).
    let mut back = open_bytes(&bytes, "rec-saved.pptx");
    run(&mut back, r#"{"cmd":"goTo","slide":0}"#);
    let r = run(&mut back, r#"{"cmd":"recordingInfo"}"#);
    let s2 = rec_slide(&r, 0);
    assert_eq!(s2["narration"].as_u64(), Some(narr));
    assert_eq!(s2["camera"].as_u64(), Some(cam));
    assert_eq!(s2["ink"][0].as_u64(), Some(ink_id));
    assert_eq!(s2["timingMs"], 8000);
    let ink = run(&mut back, r#"{"cmd":"inkInfo"}"#)["ink"].clone();
    let o = ink["objects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|o| o["id"].as_u64() == Some(ink_id))
        .unwrap()
        .clone();
    assert_eq!(o["strokes"][0]["times"][0], 3000.0);
    assert_eq!(o["strokes"][0]["times"][2], 3100.0);
    let st = run(&mut back, r#"{"cmd":"state"}"#);
    assert_eq!(effect_at(&st, ink_id), vec![(0, 3000)]);
    assert_eq!(effect_at(&st, 15), vec![(0, 6000)]);
    // Unchanged, the reopened deck saves the same slide.
    let again = back.save_pptx().unwrap();
    assert_eq!(part_text(&again, "/ppt/slides/slide1.xml"), xml);

    // Recording the slide again replaces its recording.
    let r = run(&mut back, &save_cmd(&[recording(0, "", false)]));
    let s3 = rec_slide(&r, 0);
    assert!(s3["camera"].is_null());
    assert_eq!(s3["ink"].as_array().unwrap().len(), 1);
    let st = run(&mut back, r#"{"cmd":"state"}"#);
    assert_eq!(
        st["media"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|m| m["recorded"] == "narration")
            .count(),
        1
    );

    // Clear the recording of the slide: clips, ink, their parts and effects go; timing stays.
    let r = run(
        &mut back,
        r#"{"cmd":"clearRecording","scope":"current","what":"recording"}"#,
    );
    let s4 = rec_slide(&r, 0);
    assert!(s4["narration"].is_null() && s4["ink"].as_array().unwrap().is_empty());
    assert_eq!(s4["timingMs"], 8000);
    let st = run(&mut back, r#"{"cmd":"state"}"#);
    assert_eq!(st["media"].as_array().map(Vec::len), Some(0));
    let cleared = save(&back, "rec-cleared.pptx");
    let names = part_names(&cleared);
    assert!(!names.iter().any(|n| n.starts_with("/ppt/ink/")));
    assert!(
        !names.iter().any(|n| n.starts_with("/ppt/media/")),
        "{names:?}"
    );
    assert_eq!(names.len(), parts_before);
    let xml = part_text(&cleared, "/ppt/slides/slide1.xml");
    assert!(!xml.contains("isNarration"));
    // Then the timings of every slide.
    run(
        &mut back,
        r#"{"cmd":"clearRecording","scope":"all","what":"timings"}"#,
    );
    let r = run(&mut back, r#"{"cmd":"recordingInfo"}"#);
    assert!(rec_slide(&r, 0).is_null());
    // Nothing left to clear.
    let r = call(
        &mut back,
        r#"{"cmd":"clearRecording","scope":"all","what":"all"}"#,
    );
    assert_eq!(r["error"], "noRecording");

    // Saving the recording is one undo step.
    run(&mut e, r#"{"cmd":"undo"}"#);
    let r = run(&mut e, r#"{"cmd":"recordingInfo"}"#);
    assert!(rec_slide(&r, 0).is_null());
    let st = run(&mut e, r#"{"cmd":"state"}"#);
    assert_eq!(effect_at(&st, 10), vec![(1, 0)]);
    assert_eq!(part_names(&e.save_pptx().unwrap()).len(), parts_before);
}

#[test]
fn several_slides_and_bad_input() {
    let mut e = open_bytes(&deck("slide-text-bullets.pptx"), "bullets.pptx");
    let count = run(&mut e, r#"{"cmd":"state"}"#)["info"]["pageCount"]
        .as_u64()
        .unwrap();
    assert!(count >= 2);
    let r = run(
        &mut e,
        &save_cmd(&[recording(0, "", false), recording(1, "", true)]),
    );
    assert!(rec_slide(&r, 0)["narration"].is_u64());
    assert!(rec_slide(&r, 1)["camera"].is_u64());
    save(&e, "rec-two-slides.pptx");
    // Clear everything on all slides.
    run(
        &mut e,
        r#"{"cmd":"clearRecording","scope":"all","what":"all"}"#,
    );
    let r = run(&mut e, r#"{"cmd":"recordingInfo"}"#);
    assert_eq!(r["recording"]["slides"].as_array().map(Vec::len), Some(0));

    // Refused: a video as narration, ink without times, a slide out of range, nothing.
    let bad_mime = format!(
        r#"{{"cmd":"saveRecording","slides":[{{"slide":0,"narration":{{"data":"{}","mime":"video/mp4"}}}}]}}"#,
        b64(&mp4())
    );
    assert_eq!(call(&mut e, &bad_mime)["error"], "badMedia");
    let no_times = r#"{"cmd":"saveRecording","slides":[{"slide":0,"ink":[{"points":[[1,1],[2,2]],"color":"000000","width":2}]}]}"#;
    assert_eq!(call(&mut e, no_times)["error"], "badInk");
    let far = r#"{"cmd":"saveRecording","slides":[{"slide":99,"durationMs":10}]}"#;
    assert!(call(&mut e, far)["error"].is_string());
    assert_eq!(
        call(&mut e, r#"{"cmd":"saveRecording","slides":[]}"#)["error"],
        "noRecording"
    );
    // A refused recording changes nothing.
    let r = run(&mut e, r#"{"cmd":"recordingInfo"}"#);
    assert_eq!(r["recording"]["slides"].as_array().map(Vec::len), Some(0));
}

// source: ECMA-376 Part 1, §19.3.1.50 transition — advTm written in ms, advClick kept.
#[test]
fn rehearsed_timings_keep_advance_on_click() {
    let mut e = open_bytes(&deck("slide-text-bullets.pptx"), "bullets.pptx");
    run(&mut e, r#"{"cmd":"goTo","slide":1}"#);
    run(
        &mut e,
        r#"{"cmd":"setTransition","transition":{"kind":"fade","advanceOnClick":false}}"#,
    );
    run(
        &mut e,
        r#"{"cmd":"setTimings","timings":[{"slide":0,"ms":4200},{"slide":1,"ms":12345}],"useTimings":true}"#,
    );
    let bytes = save(&e, "rec-rehearsed.pptx");
    let s1 = part_text(&bytes, "/ppt/slides/slide1.xml");
    let s2 = part_text(&bytes, "/ppt/slides/slide2.xml");
    assert!(s1.contains(r#"advTm="4200""#), "{s1}");
    assert!(
        s2.contains(r#"advTm="12345""#) && s2.contains(r#"advClick="0""#),
        "{s2}"
    );
    let mut back = open_bytes(&bytes, "rehearsed.pptx");
    run(&mut back, r#"{"cmd":"goTo","slide":1}"#);
    let r = run(&mut back, r#"{"cmd":"recordingInfo"}"#);
    assert_eq!(rec_slide(&r, 0)["timingMs"], 4200);
    assert_eq!(rec_slide(&r, 1)["timingMs"], 12345);
    let st = run(&mut back, r#"{"cmd":"state"}"#);
    assert_eq!(st["transition"]["advanceOnClick"], false);
    assert_eq!(st["transition"]["advanceAfterMs"], 12345);
}

// Corpus decks written by `bo-cli sample-slides` (SLIDE-REC-001) read back with their
// recordings; their XML is checked by OpenXmlValidator (`pnpm fidelity`).
#[test]
fn corpus_recordings_read_back() {
    let mut e = open_bytes(&deck("slide-rec-narration.pptx"), "n.pptx");
    let r = run(&mut e, r#"{"cmd":"recordingInfo"}"#);
    assert!(rec_slide(&r, 0)["narration"].is_u64());
    assert_eq!(rec_slide(&r, 0)["timingMs"], 6500);
    assert_eq!(rec_slide(&r, 1)["timingMs"], 4200);

    let mut e = open_bytes(&deck("slide-rec-camera-ink.pptx"), "c.pptx");
    let r = run(&mut e, r#"{"cmd":"recordingInfo"}"#);
    let s = rec_slide(&r, 0);
    assert!(s["camera"].is_u64() && s["narration"].is_u64(), "{s}");
    assert_eq!(s["ink"].as_array().unwrap().len(), 2);
    let st = run(&mut e, r#"{"cmd":"state"}"#);
    // The first effect played on the recorded click at 1.5 s, the second still on click.
    assert_eq!(effect_at(&st, 10), vec![(0, 1500)]);
    assert_eq!(effect_at(&st, 11), vec![(1, 0)]);
    let ink = s["ink"].as_array().unwrap();
    assert_eq!(effect_at(&st, ink[0].as_u64().unwrap()), vec![(0, 2500)]);
    assert_eq!(effect_at(&st, ink[1].as_u64().unwrap()), vec![(0, 5200)]);

    let mut e = open_bytes(&deck("slide-rec-timings.pptx"), "t.pptx");
    let r = run(&mut e, r#"{"cmd":"recordingInfo"}"#);
    assert_eq!(rec_slide(&r, 0)["timingMs"], 3200);
    assert_eq!(rec_slide(&r, 1)["timingMs"], 7450);
    assert!(rec_slide(&r, 1)["narration"].is_null());
}
