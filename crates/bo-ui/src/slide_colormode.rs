//! Grayscale and Black and White views of BlueSlide (N6, round 5): the slide's display list
//! recoloured before it is drawn, as the reference presentation app's View > Color /
//! Grayscale / Black and White modes show a deck for printing on a monochrome printer. The
//! document never changes.
//!
//! - Grayscale: every colour becomes its luminance (ITU-R BT.601 weights); pictures are drawn
//!   through a grayscale derived image id (`media#fx:…`, CORE-PIC-001).
//! - Black and White: text and lines black, fills white (light fills) or black (very dark
//!   fills), gradients white, pictures grayscale.
use bo_model::picture::{split_fx_id, PictureFx};
use bo_render::{Command, PageList, Paint, Rgba};

/// How slides are drawn in the editing area.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ColorMode {
    /// As they are.
    #[default]
    Color,
    /// Shades of grey.
    Grayscale,
    /// Black and white.
    BlackWhite,
}

fn luma(c: Rgba) -> u8 {
    (0.299 * f32::from(c.0) + 0.587 * f32::from(c.1) + 0.114 * f32::from(c.2)).round() as u8
}

fn grey(c: Rgba) -> Rgba {
    let l = luma(c);
    Rgba(l, l, l, c.3)
}

const BLACK: Rgba = Rgba(0, 0, 0, 255);
const WHITE: Rgba = Rgba(255, 255, 255, 255);

/// A fill in Black and White: white unless the colour is nearly black.
fn bw_fill(c: Rgba) -> Rgba {
    let v = if luma(c) < 40 { BLACK } else { WHITE };
    Rgba(v.0, v.1, v.2, c.3)
}

fn ink(c: Rgba) -> Rgba {
    Rgba(0, 0, 0, c.3)
}

fn paint(p: &Paint, mode: ColorMode) -> Paint {
    let map = |c: Rgba| match mode {
        ColorMode::Color => c,
        ColorMode::Grayscale => grey(c),
        ColorMode::BlackWhite => bw_fill(c),
    };
    match p {
        Paint::Solid { color } => Paint::Solid { color: map(*color) },
        Paint::Linear {
            x1,
            y1,
            x2,
            y2,
            stops,
        } => {
            if mode == ColorMode::BlackWhite {
                return Paint::Solid { color: WHITE };
            }
            let mut stops = stops.clone();
            for s in &mut stops {
                s.color = map(s.color);
            }
            Paint::Linear {
                x1: *x1,
                y1: *y1,
                x2: *x2,
                y2: *y2,
                stops,
            }
        }
        Paint::Radial { cx, cy, r, stops } => {
            if mode == ColorMode::BlackWhite {
                return Paint::Solid { color: WHITE };
            }
            let mut stops = stops.clone();
            for s in &mut stops {
                s.color = map(s.color);
            }
            Paint::Radial {
                cx: *cx,
                cy: *cy,
                r: *r,
                stops,
            }
        }
    }
}

/// A picture id drawn in grey.
fn grey_image(id: &str) -> String {
    let (base, fx) = split_fx_id(id).map_or((id, PictureFx::default()), |(b, f)| (b, f));
    PictureFx {
        grayscale: true,
        ..fx
    }
    .derived_id(base)
}

fn command(c: &Command, mode: ColorMode) -> Command {
    let line = |col: Rgba| match mode {
        ColorMode::Color => col,
        ColorMode::Grayscale => grey(col),
        ColorMode::BlackWhite => ink(col),
    };
    let mut c = c.clone();
    match &mut c {
        Command::FillRect { color, .. } => {
            *color = match mode {
                ColorMode::BlackWhite => bw_fill(*color),
                _ => line(*color),
            };
        }
        Command::Line { color, .. } | Command::Glyphs { color, .. } => *color = line(*color),
        Command::BlurPath { color, .. } => {
            // Shadows and glows go in Black and White.
            *color = match mode {
                ColorMode::BlackWhite => Rgba(0, 0, 0, 0),
                _ => line(*color),
            };
        }
        Command::Image { image, .. } => *image = grey_image(image),
        Command::Path { fill, stroke, .. } => {
            if let Some(f) = fill.as_mut() {
                *f = paint(f, mode);
            }
            if let Some(s) = stroke.as_mut() {
                s.color = line(s.color);
            }
            // Filled shapes keep their edge in Black and White (white on white otherwise).
            if mode == ColorMode::BlackWhite && fill.is_some() && stroke.is_none() {
                *stroke = Some(bo_render::Stroke {
                    width: 0.75,
                    color: BLACK,
                    dash: Vec::new(),
                    cap: "butt".into(),
                    join: "miter".into(),
                });
            }
        }
        Command::PushGroup { mask: Some(m), .. } => {
            m.commands = m.commands.iter().map(|c| command(c, mode)).collect();
        }
        _ => {}
    }
    c
}

/// `list` drawn in `mode`.
pub fn recolor(list: &PageList, mode: ColorMode) -> PageList {
    if mode == ColorMode::Color {
        return list.clone();
    }
    PageList {
        width: list.width,
        height: list.height,
        commands: list.commands.iter().map(|c| command(c, mode)).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn list() -> PageList {
        PageList {
            width: 100.0,
            height: 100.0,
            commands: vec![
                Command::FillRect {
                    x: 0.0,
                    y: 0.0,
                    w: 10.0,
                    h: 10.0,
                    color: Rgba(0x44, 0x72, 0xc4, 255),
                },
                Command::Image {
                    x: 0.0,
                    y: 0.0,
                    w: 10.0,
                    h: 10.0,
                    image: "/ppt/media/image1.png".into(),
                    crop: None,
                    rotation: 0.0,
                },
            ],
        }
    }

    #[test]
    fn grayscale_greys_colours_and_pictures() {
        let g = recolor(&list(), ColorMode::Grayscale);
        let Command::FillRect { color, .. } = &g.commands[0] else {
            panic!("rect");
        };
        assert_eq!(color.0, color.1);
        assert_eq!(color.1, color.2);
        let Command::Image { image, .. } = &g.commands[1] else {
            panic!("image");
        };
        let (base, fx) = split_fx_id(image).expect("derived");
        assert_eq!(base, "/ppt/media/image1.png");
        assert!(fx.grayscale);
        assert_eq!(recolor(&list(), ColorMode::Color), list());
    }

    #[test]
    fn black_and_white_makes_fills_white() {
        let b = recolor(&list(), ColorMode::BlackWhite);
        let Command::FillRect { color, .. } = &b.commands[0] else {
            panic!("rect");
        };
        assert_eq!(*color, WHITE);
    }
}
