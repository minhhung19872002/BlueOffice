//! Design › Page Color › Fill Effects over the JSON bridge: gradient and picture page fills
//! (the VML page background, ECMA-376 Part 4 §19.1.2.5) drawn on every page, one undo step,
//! saved and reopened; a plain page colour removes the effect.
// Tests may unwrap: a panic is a test failure.
#![allow(clippy::unwrap_used)]

use bo_engine::Engine;
use bo_render::{Command, Paint};
use serde_json::Value;

fn cmd(e: &mut Engine, json: &str) -> Value {
    let r: Value = serde_json::from_str(&e.command_json(json).unwrap()).unwrap();
    assert!(r.get("error").is_none(), "{json}: {:?}", r.get("error"));
    r
}

fn reopen(e: &Engine) -> Engine {
    let bytes = e.save_docx().unwrap();
    let mut f = Engine::with_bundled_fonts();
    f.open(&bytes, "x.docx").unwrap();
    f
}

/// A 2×2 PNG (base64).
const PNG_B64: &str = "iVBORw0KGgoAAAANSUhEUgAAAAIAAAACCAIAAAD91JpzAAAAEElEQVR4nGP4zwAE/xkgFAAb8gP91pbyKwAAAABJRU5ErkJggg==";

#[test]
fn gradient_fill_is_drawn_saved_and_undone() {
    let mut e = Engine::with_bundled_fonts();
    e.new_document("a.docx").unwrap();
    cmd(
        &mut e,
        r#"{"cmd":"insertText","text":"Nền trang chuyển màu."}"#,
    );
    let r = cmd(
        &mut e,
        r#"{"cmd":"setPageFill","color":"2070C0","fill":{"kind":"gradient","color2":{"r":255,"g":224,"b":96},"focus":100,"angle":-90}}"#,
    );
    assert_eq!(r["design"]["pageColor"], "2070C0");
    assert_eq!(r["design"]["pageFill"]["kind"], "gradient");
    let p = e.page(0).unwrap();
    let grad = p.commands.iter().find_map(|c| match c {
        Command::Path {
            fill: Some(Paint::Linear { x1, x2, stops, .. }),
            ..
        } => Some((*x1, *x2, stops.clone())),
        _ => None,
    });
    let (x1, x2, stops) = grad.expect("gradient path");
    assert!(
        x1.abs() < 0.1 && (x2 - p.width).abs() < 0.1,
        "left to right"
    );
    assert_eq!(stops.first().unwrap().color.2, 0xC0);
    let back = reopen(&e);
    let d = &back.document().unwrap().design;
    assert_eq!(d.background_fill.as_ref().unwrap().angle, -90);
    assert!(d.display_background_shape);
    cmd(&mut e, r#"{"cmd":"undo"}"#);
    let r = cmd(&mut e, r#"{"cmd":"design"}"#);
    assert!(r["design"]["pageFill"].is_null());
    // A plain colour replaces the effect.
    cmd(
        &mut e,
        r#"{"cmd":"setPageFill","color":"FFFFFF","fill":{"kind":"gradientRadial","focus":100,"focusPosition":[500,500]}}"#,
    );
    cmd(&mut e, r#"{"cmd":"setPageColor","color":"FFF2CC"}"#);
    let r = cmd(&mut e, r#"{"cmd":"design"}"#);
    assert!(r["design"]["pageFill"].is_null());
}

#[test]
fn picture_fill_adds_media_and_tiles() {
    let mut e = Engine::with_bundled_fonts();
    e.new_document("a.docx").unwrap();
    cmd(&mut e, r#"{"cmd":"insertText","text":"Kết cấu."}"#);
    cmd(
        &mut e,
        &format!(
            r#"{{"cmd":"setPageFill","color":"FFFFFF","fill":{{"kind":"tile","title":"Ảnh"}},"picture":"{PNG_B64}"}}"#
        ),
    );
    let d = e.document().unwrap();
    let img = d
        .design
        .background_fill
        .as_ref()
        .unwrap()
        .image
        .clone()
        .unwrap();
    assert!(d.media.contains_key(&img), "{img}");
    let p = e.page(0).unwrap();
    let tiles = p
        .commands
        .iter()
        .filter(|c| matches!(c, Command::Image { image, .. } if *image == img))
        .count();
    // 2 px at 96 dpi = 1.5 pt tiles, grown to keep at most 4096 a page.
    assert!((1000..=4096).contains(&tiles), "{tiles}");
    let back = reopen(&e);
    let d = back.document().unwrap();
    let f = d.design.background_fill.as_ref().unwrap();
    assert!(d.media.contains_key(f.image.as_deref().unwrap()));
}

/// An 8 × 8 1-bit BMP: black where `rows` has a bit set (bit 7 = left), white elsewhere.
fn bmp(rows: [u8; 8]) -> Vec<u8> {
    let mut b = b"BM".to_vec();
    let size: u32 = 14 + 40 + 8 + 8 * 4;
    b.extend_from_slice(&size.to_le_bytes());
    b.extend_from_slice(&[0; 4]);
    b.extend_from_slice(&(14u32 + 40 + 8).to_le_bytes());
    b.extend_from_slice(&40u32.to_le_bytes());
    b.extend_from_slice(&8i32.to_le_bytes());
    b.extend_from_slice(&8i32.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&[0; 24]);
    // Palette: 0 = white, 1 = black.
    b.extend_from_slice(&[255, 255, 255, 0, 0, 0, 0, 0]);
    for r in rows.iter().rev() {
        b.extend_from_slice(&[*r, 0, 0, 0]);
    }
    b
}

#[test]
fn pattern_fill_keeps_its_bitmap_and_draws_the_screen_pattern() {
    use base64::Engine as _;
    let mut e = Engine::with_bundled_fonts();
    e.new_document("a.docx").unwrap();
    cmd(&mut e, r#"{"cmd":"insertText","text":"Mẫu."}"#);
    let rows = [0x80, 0x40, 0x20, 0x10, 0x08, 0x04, 0x02, 0x01];
    let b64 = base64::engine::general_purpose::STANDARD.encode(bmp(rows));
    cmd(
        &mut e,
        &format!(
            r#"{{"cmd":"setPageFill","color":"2040C0","fill":{{"kind":"pattern","color2":{{"r":224,"g":240,"b":255}}}},"picture":"{b64}"}}"#
        ),
    );
    let d = e.document().unwrap();
    let img = d
        .design
        .background_fill
        .as_ref()
        .unwrap()
        .image
        .clone()
        .unwrap();
    assert!(img.ends_with(".bmp"), "{img}");
    let p = e.page(0).unwrap();
    let tile = p.commands.iter().find_map(|c| match c {
        Command::FillPattern { tile, fg, .. } => Some((*tile, *fg)),
        _ => None,
    });
    let (tile, fg) = tile.expect("pattern");
    assert_eq!(tile, rows);
    assert_eq!((fg.0, fg.1, fg.2), (0x20, 0x40, 0xC0));
}
