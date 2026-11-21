//! Format Background, BlueSlide round 22: gradient backgrounds with explicit stops
//! (positions, transparency, path kinds) and picture / texture backgrounds (tiled or
//! stretched with offsets, transparency), saved and read back.

#![allow(clippy::unwrap_used)]

use base64::Engine as _;
use bo_engine::Engine;
use bo_model::slide::{Background, Fill, GradientShade, Presentation};
use bo_pptx::build::{build_pptx, DeckSpec, LayoutKind, SlideSpec};
use bo_render::Command;
use serde_json::{json, Value};

fn open() -> Engine {
    let deck = build_pptx(&DeckSpec {
        width: 12_192_000,
        height: 6_858_000,
        title: None,
        slides: vec![
            SlideSpec {
                layout: LayoutKind::Title,
                ..SlideSpec::default()
            },
            SlideSpec {
                layout: LayoutKind::TitleContent,
                ..SlideSpec::default()
            },
        ],
    })
    .unwrap();
    let mut e = Engine::with_bundled_fonts();
    e.open(&deck, "deck.pptx").unwrap();
    e
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

fn reopen(e: &Engine) -> Engine {
    let bytes = e.save_pptx().unwrap();
    let mut again = Engine::with_bundled_fonts();
    again.open(&bytes, "bg.pptx").unwrap();
    again
}

/// A 4 × 2 PNG, base64.
fn png_b64() -> String {
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, 4, 2);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut w = enc.write_header().unwrap();
        w.write_image_data(&[200u8; 4 * 2 * 4]).unwrap();
    }
    base64::engine::general_purpose::STANDARD.encode(out)
}

#[test]
fn gradient_stops_with_positions_transparency_and_paths() {
    let mut e = open();
    run(
        &mut e,
        &json!({"cmd":"setBackgroundGradient","stops":[
            {"pos":100,"color":"4472C4"},
            {"pos":0,"color":"FFFFFF","transparency":40},
            {"pos":35,"color":"DEEAF6"}],"angle":45}),
    );
    let again = reopen(&e);
    match &pres(&again).slides[0].data.background {
        Some(Background::Props {
            fill: Fill::Gradient(g),
            ..
        }) => {
            // Sorted by position; the transparent stop carries alpha 60 %.
            let pos: Vec<i32> = g.stops.iter().map(|s| s.pos).collect();
            assert_eq!(pos, [0, 35_000, 100_000]);
            let alpha = g.stops[0].color.mods.iter().find(|m| m.name == "alpha");
            assert_eq!(alpha.and_then(|m| m.val), Some(60_000));
            assert!(matches!(
                g.shade,
                Some(GradientShade::Linear { ang: 2_700_000, .. })
            ));
        }
        other => panic!("{other:?}"),
    }
    // A radial (circle path) gradient from the top-left corner, on every slide.
    run(
        &mut e,
        &json!({"cmd":"setBackgroundGradient","stops":[
            {"pos":0,"color":"FFFFFF"},{"pos":100,"color":"000000"}],
            "path":"circle","focus":[0,0,100,100],"all":true}),
    );
    for s in &pres(&e).slides {
        match &s.data.background {
            Some(Background::Props {
                fill: Fill::Gradient(g),
                ..
            }) => assert_eq!(
                g.shade,
                Some(GradientShade::Path {
                    path: "circle".into(),
                    fill_to: Some([0, 0, 100_000, 100_000]),
                })
            ),
            other => panic!("{other:?}"),
        }
    }
    assert_eq!(
        error_of(
            &mut e,
            &json!({"cmd":"setBackgroundGradient","stops":[{"pos":0,"color":"FFFFFF"}]})
        )
        .as_deref(),
        Some("badStops")
    );
    assert!(error_of(
        &mut e,
        &json!({"cmd":"setBackgroundGradient","stops":[
            {"pos":0,"color":"FFFFFF"},{"pos":100,"color":"000000"}],"path":"star"})
    )
    .is_some());
}

#[test]
fn picture_background_tiles_stretches_and_reuses_its_part() {
    let mut e = open();
    let parts = |e: &Engine| {
        pres(e)
            .parts
            .iter()
            .filter(|p| p.name.starts_with("/ppt/media/"))
            .count()
    };
    let before = parts(&e);
    run(
        &mut e,
        &json!({"cmd":"setBackgroundPicture","data":png_b64(),
            "tile":{"tx":3,"ty":0,"sx":1000,"sy":1000,"algn":"ctr","flip":"x"},"transparency":25}),
    );
    assert_eq!(parts(&e), before + 1);
    // The slide's picture is drawn tile by tile (4 × 2 px at 1000 %: 30 × 15 pt; 960 × 540 pt
    // need 33 × 36 tiles).
    let images = e
        .refresh_slide(0)
        .unwrap()
        .commands
        .iter()
        .filter(|c| matches!(c, Command::Image { .. }))
        .count();
    assert!(images > 1000, "{images}");
    // Options only: the same picture stretched with offsets, then on every slide.
    run(
        &mut e,
        &json!({"cmd":"setBackgroundPicture","offsets":[10,0,-10,5],"all":true}),
    );
    assert_eq!(parts(&e), before + 1);
    let again = reopen(&e);
    for s in &pres(&again).slides {
        match &s.data.background {
            Some(Background::Props {
                fill: Fill::Picture(b),
                ..
            }) => {
                assert!(b.stretch && b.tile.is_none());
                assert_eq!(b.fill_rect, Some(Box::new([10_000, 0, -10_000, 5_000])));
                assert!(b.embed.is_some());
            }
            other => panic!("{other:?}"),
        }
    }
    // A new picture replaces the old one, whose part goes when nothing uses it.
    run(
        &mut e,
        &json!({"cmd":"setBackgroundPicture","data":png_b64(),"all":true}),
    );
    assert_eq!(parts(&e), before + 1);
    // Undo brings the stretched picture back.
    run(&mut e, &json!({"cmd":"undo"}));
    assert!(matches!(
        &pres(&e).slides[0].data.background,
        Some(Background::Props { fill: Fill::Picture(b), .. }) if b.fill_rect.is_some()
    ));
    // No picture yet on a slide without one, bad data.
    run(
        &mut e,
        &json!({"cmd":"setBackground","color":null,"all":true}),
    );
    assert_eq!(
        error_of(&mut e, &json!({"cmd":"setBackgroundPicture"})).as_deref(),
        Some("noPicture")
    );
    assert_eq!(
        error_of(
            &mut e,
            &json!({"cmd":"setBackgroundPicture","data":"bm90IGFuIGltYWdl"})
        )
        .as_deref(),
        Some("badImage")
    );
}
