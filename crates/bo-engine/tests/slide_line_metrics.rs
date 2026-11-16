//! Slide text lines use the Windows line metrics (`OS/2.usWinAscent` / `usWinDescent`), as
//! the desktop presentation app lays them out (REPORT-slide round 14): with the installed
//! Calibri, whose `hhea` ascender (0.75 em) is far below its Windows ascent (0.952 em), the
//! first baseline of a top-anchored text box sits `inset + winAscent` below its top.
//!
//! Needs the system Calibri (`C:\Windows\Fonts\calibri.ttf`); skipped where it is absent
//! (bundled Carlito has `hhea` = Windows metrics, so it cannot tell the two apart).
// Tests may unwrap: a panic is a test failure.
#![allow(clippy::unwrap_used)]

use bo_engine::Engine;
use bo_pptx::build::{build_pptx, DeckSpec, ParaSpec, RunSpec, ShapeSpec, SlideSpec};
use bo_render::Command;

/// A table of a TrueType font (`tag` → its bytes).
fn table<'a>(font: &'a [u8], tag: &[u8; 4]) -> Option<&'a [u8]> {
    let n = usize::from(u16::from_be_bytes([*font.get(4)?, *font.get(5)?]));
    (0..n).find_map(|i| {
        let rec = font.get(12 + 16 * i..28 + 16 * i)?;
        if &rec[..4] != tag {
            return None;
        }
        let off = u32::from_be_bytes([rec[8], rec[9], rec[10], rec[11]]) as usize;
        let len = u32::from_be_bytes([rec[12], rec[13], rec[14], rec[15]]) as usize;
        font.get(off..off + len)
    })
}

fn u16_at(t: &[u8], at: usize) -> u16 {
    u16::from_be_bytes([t[at], t[at + 1]])
}

/// First baseline (points) of the run reading `word` on slide 0.
fn baseline(e: &Engine, word: &str) -> f32 {
    e.slide(0)
        .unwrap()
        .commands
        .iter()
        .find_map(|c| match c {
            Command::Glyphs { y, text, .. } if text.contains(word) => Some(*y),
            _ => None,
        })
        .unwrap()
}

#[test]
fn system_calibri_first_baseline_follows_the_windows_ascent() {
    let Ok(font) = std::fs::read(r"C:\Windows\Fonts\calibri.ttf") else {
        eprintln!("no system Calibri: skipped");
        return;
    };
    let upem = f32::from(u16_at(table(&font, b"head").unwrap(), 18));
    let win_ascent = f32::from(u16_at(table(&font, b"OS/2").unwrap(), 74));
    let hhea_ascent = f32::from(u16_at(table(&font, b"hhea").unwrap(), 4) as i16);
    assert!(
        win_ascent - hhea_ascent > 0.1 * upem,
        "the two metrics differ"
    );
    let run = |text: &str, size: u32| RunSpec {
        text: text.into(),
        size: Some(size),
        font: Some("Calibri".into()),
        ..RunSpec::default()
    };
    let shape = |name: &str, y: i64, text: &str, size: u32| ShapeSpec {
        name: name.into(),
        x: 914_400,
        y,
        cx: 6_096_000,
        cy: 1_371_600,
        anchor: Some("t".into()),
        text: vec![ParaSpec {
            runs: vec![run(text, size)],
            ..ParaSpec::default()
        }],
        ..ShapeSpec::default()
    };
    let deck = build_pptx(&DeckSpec {
        width: 12_192_000,
        height: 6_858_000,
        title: None,
        slides: vec![SlideSpec {
            shapes: vec![
                shape("A", 914_400, "Small", 2400),
                shape("B", 2_743_200, "Large", 5400),
            ],
            ..SlideSpec::default()
        }],
    })
    .unwrap();
    let mut e = Engine::new();
    e.add_font(font);
    e.open(&deck, "metrics.pptx").unwrap();
    // Top inset 3.6 pt (§21.1.2.1.1 bodyPr tIns 45 720 EMU), then the Windows ascent (runs
    // are placed in the shape's own coordinates, from its top-left corner).
    for (word, size) in [("Small", 24.0), ("Large", 54.0)] {
        let want = 3.6 + win_ascent / upem * size;
        let got = baseline(&e, word);
        assert!(
            (got - want).abs() < 0.5,
            "{word}: baseline {got}, Windows metrics {want}, hhea {}",
            3.6 + hhea_ascent / upem * size
        );
    }
}
