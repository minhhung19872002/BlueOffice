//! Colour modes of a print job: colour, greyscale, pure black and white.
//!
//! * Greyscale: every colour becomes its luma `Y = 0.299 R + 0.587 G + 0.114 B`
//!   (ITU-R BT.601 weights), rounded; alpha is kept. Pictures get the
//!   greyscale adjustment of ECMA-376 Part 1, §20.1.8.34 `grayscl` through a
//!   derived image id (the PDF writer resolves it).
//! * Black and white (our own choice of mapping): text, lines and outlines
//!   black; fills white; shapes that were only filled get a black hairline so
//!   they stay visible; pictures go through §20.1.8.11 `biLevel` at 50 %;
//!   soft shadows and glows are left out.

use bo_model::picture::{split_fx_id, PictureFx};
use bo_render::{Command, Paint, Rgba, Stroke};
use serde::Deserialize;

/// Colour mode of a print job.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ColorMode {
    /// As the document is.
    #[default]
    Color,
    /// Shades of grey.
    Grayscale,
    /// Pure black and white.
    BlackWhite,
}

/// BT.601 luma of a colour, alpha kept.
pub fn gray(c: Rgba) -> Rgba {
    let y = (0.299 * f64::from(c.0) + 0.587 * f64::from(c.1) + 0.114 * f64::from(c.2))
        .round()
        .clamp(0.0, 255.0);
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let g = y as u8;
    Rgba(g, g, g, c.3)
}

const BLACK: Rgba = Rgba(0, 0, 0, 255);

/// Image id with `edit` applied to its adjustments.
fn image_with(id: &str, edit: impl FnOnce(&mut PictureFx)) -> String {
    let (base, mut fx) = split_fx_id(id).unwrap_or((id, PictureFx::default()));
    edit(&mut fx);
    fx.derived_id(base)
}

fn paint_map(p: &mut Paint, f: impl Fn(Rgba) -> Rgba) {
    match p {
        Paint::Solid { color } => *color = f(*color),
        Paint::Linear { stops, .. } | Paint::Radial { stops, .. } => {
            for s in stops {
                s.color = f(s.color);
            }
        }
    }
}

/// Greyscale every colour of `commands`.
pub fn grayscale(commands: &mut [Command]) {
    for c in commands {
        match c {
            Command::FillRect { color, .. }
            | Command::Line { color, .. }
            | Command::Glyphs { color, .. }
            | Command::BlurPath { color, .. } => *color = gray(*color),
            Command::Path { fill, stroke, .. } => {
                if let Some(p) = fill {
                    paint_map(p, gray);
                }
                if let Some(s) = stroke {
                    s.color = gray(s.color);
                }
            }
            Command::Image { image, .. } => {
                *image = image_with(image, |fx| fx.grayscale = true);
            }
            _ => {}
        }
    }
}

/// Pure black and white.
pub fn black_white(commands: &mut Vec<Command>) {
    commands.retain(|c| !matches!(c, Command::BlurPath { .. }));
    for c in commands.iter_mut() {
        match c {
            Command::FillRect { color, .. } => *color = Rgba(255, 255, 255, color.3),
            Command::Line { color, .. } | Command::Glyphs { color, .. } => {
                *color = Rgba(0, 0, 0, color.3);
            }
            Command::Path { fill, stroke, .. } => {
                let had_stroke = stroke.is_some();
                if let Some(p) = fill.as_mut() {
                    paint_map(p, |c| Rgba(255, 255, 255, c.3));
                    if !had_stroke {
                        *stroke = Some(Stroke {
                            width: 0.5,
                            color: BLACK,
                            dash: Vec::new(),
                            cap: "butt".into(),
                            join: "miter".into(),
                        });
                    }
                }
                if let Some(s) = stroke {
                    s.color = Rgba(0, 0, 0, s.color.3);
                }
            }
            Command::Image { image, .. } => {
                // §20.1.8.11 biLevel@thresh in 1000ths of a percent: 50 %.
                *image = image_with(image, |fx| fx.bilevel = Some(50_000));
            }
            _ => {}
        }
    }
}

/// Apply `mode` to a page's commands.
pub fn apply(mode: ColorMode, commands: &mut Vec<Command>) {
    match mode {
        ColorMode::Color => {}
        ColorMode::Grayscale => grayscale(commands),
        ColorMode::BlackWhite => black_white(commands),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bo_render::{GradientStop, PathSeg};

    #[test]
    fn luma_by_hand() {
        // Red: 0.299 × 255 = 76.245 → 76; green 149.685 → 150; blue 29.07 → 29.
        assert_eq!(gray(Rgba(255, 0, 0, 255)), Rgba(76, 76, 76, 255));
        assert_eq!(gray(Rgba(0, 255, 0, 10)), Rgba(150, 150, 150, 10));
        assert_eq!(gray(Rgba(0, 0, 255, 255)), Rgba(29, 29, 29, 255));
        assert_eq!(gray(Rgba(255, 255, 255, 255)), Rgba(255, 255, 255, 255));
    }

    fn sample() -> Vec<Command> {
        vec![
            Command::FillRect {
                x: 0.0,
                y: 0.0,
                w: 1.0,
                h: 1.0,
                color: Rgba(255, 0, 0, 255),
            },
            Command::Path {
                segs: vec![PathSeg::M(0.0, 0.0), PathSeg::L(1.0, 1.0)],
                fill: Some(Paint::Linear {
                    x1: 0.0,
                    y1: 0.0,
                    x2: 1.0,
                    y2: 0.0,
                    stops: vec![GradientStop {
                        offset: 0.0,
                        color: Rgba(0, 0, 255, 255),
                    }],
                }),
                stroke: None,
            },
            Command::Image {
                x: 0.0,
                y: 0.0,
                w: 1.0,
                h: 1.0,
                image: "/word/media/a.png".into(),
                crop: None,
                rotation: 0.0,
            },
            Command::BlurPath {
                segs: Vec::new(),
                color: Rgba(1, 2, 3, 255),
                radius: 2.0,
                spread: 0.0,
            },
        ]
    }

    #[test]
    fn greyscale_maps_every_colour_and_pictures() {
        let mut c = sample();
        grayscale(&mut c);
        assert!(matches!(c[0], Command::FillRect { color, .. } if color == Rgba(76, 76, 76, 255)));
        let Command::Path {
            fill: Some(Paint::Linear { stops, .. }),
            ..
        } = &c[1]
        else {
            panic!("path")
        };
        assert_eq!(stops[0].color, Rgba(29, 29, 29, 255));
        let Command::Image { image, .. } = &c[2] else {
            panic!("image")
        };
        let (base, fx) = split_fx_id(image).unwrap();
        assert_eq!(base, "/word/media/a.png");
        assert!(fx.grayscale);
        assert_eq!(c.len(), 4);
    }

    #[test]
    fn black_and_white() {
        let mut c = sample();
        black_white(&mut c);
        // The blur is dropped.
        assert_eq!(c.len(), 3);
        assert!(
            matches!(c[0], Command::FillRect { color, .. } if color == Rgba(255, 255, 255, 255))
        );
        let Command::Path {
            fill: Some(Paint::Linear { stops, .. }),
            stroke: Some(s),
            ..
        } = &c[1]
        else {
            panic!("path")
        };
        assert_eq!(stops[0].color, Rgba(255, 255, 255, 255));
        assert_eq!(s.color, BLACK);
        let Command::Image { image, .. } = &c[2] else {
            panic!("image")
        };
        assert_eq!(split_fx_id(image).unwrap().1.bilevel, Some(50_000));
    }
}
