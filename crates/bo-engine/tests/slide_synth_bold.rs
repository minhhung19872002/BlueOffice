//! Simulated bold on slides (REPORT-slide round 16): a bold run in a family without a bold
//! face (the installed Calibri Light, the default heading font) is drawn emboldened on
//! advances 0.982 of the regular ones, as the desktop presentation app lays it out — its
//! titles then fit on the line where the regular advances would wrap.
//!
//! Needs the system Calibri Light (`C:\Windows\Fonts\calibril.ttf`); skipped where absent.
// Tests may unwrap: a panic is a test failure.
#![allow(clippy::unwrap_used)]

use bo_engine::Engine;
use bo_pptx::build::{build_pptx, DeckSpec, ParaSpec, RunSpec, ShapeSpec, SlideSpec};
use bo_render::Command;

/// Glyph runs of slide 0 reading `word`: (x of the last glyph, run x).
fn runs(e: &Engine, word: &str) -> Vec<(f32, f32)> {
    e.slide(0)
        .unwrap()
        .commands
        .iter()
        .filter_map(|c| match c {
            Command::Glyphs {
                x, text, glyphs, ..
            } if text.contains(word) => Some((glyphs.last().unwrap().1, *x)),
            _ => None,
        })
        .collect()
}

#[test]
fn simulated_bold_is_narrower_and_smeared() {
    let Ok(font) = std::fs::read(r"C:\Windows\Fonts\calibril.ttf") else {
        eprintln!("no system Calibri Light: skipped");
        return;
    };
    let shape = |name: &str, y: i64, text: &str, bold: bool| ShapeSpec {
        name: name.into(),
        x: 914_400,
        y,
        cx: 9_144_000,
        cy: 1_371_600,
        anchor: Some("t".into()),
        text: vec![ParaSpec {
            runs: vec![RunSpec {
                text: text.into(),
                size: Some(6000),
                bold,
                font: Some("Calibri Light".into()),
                ..RunSpec::default()
            }],
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
                shape("R", 914_400, "nnnnnnnnnnR", false),
                shape("B", 2_743_200, "nnnnnnnnnnB", true),
            ],
            ..SlideSpec::default()
        }],
    })
    .unwrap();
    let mut e = Engine::new();
    e.add_font(font);
    e.open(&deck, "bold.pptx").unwrap();
    let regular = runs(&e, "nnnnnnnnnnR");
    let bold = runs(&e, "nnnnnnnnnnB");
    assert_eq!(regular.len(), 1);
    // Drawn twice, the copy smeared right by 0.032 em.
    assert_eq!(bold.len(), 2, "{bold:?}");
    assert!(((bold[0].1 - bold[1].1).abs() - 60.0 * 0.032).abs() < 0.01);
    // Ten `n` at 60 pt: 31.125 pt each regular, 30.625 bold on the reference.
    let (r, b) = (regular[0].0 / 10.0, bold[0].0 / 10.0);
    assert!((r - 31.17).abs() < 0.1, "regular {r}");
    assert!((b - 30.61).abs() < 0.1, "bold {b}");
}
