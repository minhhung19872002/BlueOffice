//! Insert › Object › Display as icon (round 22): the picture an embedded object shows
//! when it is drawn as an icon (`o:OLEObject DrawAspect="Icon"`, [MS-OI29500] / ECMA-376
//! Part 4 VML `o:OLEObject`) — BlueOffice's own workbook icon with the caption below it.

use bo_render::{Command, Glyph, PageList, Paint, PathSeg, Rgba, Stroke};
use bo_text::FontManager;

/// Icon box (points), the caption's size and the gap between them.
const ICON: f32 = 32.0;
const CAPTION_PT: f32 = 9.0;
const GAP: f32 = 3.0;
/// Pixels per point of the stored picture.
const SCALE: f32 = 4.0;
/// Longest caption kept (characters), like the reference's caption box.
pub const CAPTION_MAX: usize = 60;

/// Default caption of a new embedded workbook.
pub const DEFAULT_CAPTION: &str = "Worksheet";

/// BlueSheet's own accent (the selection green of its interface).
const ACCENT: Rgba = Rgba(0x21, 0x73, 0x46, 255);

fn stroke(width: f32, color: Rgba) -> Stroke {
    Stroke {
        width,
        color,
        dash: Vec::new(),
        cap: "butt".into(),
        join: "miter".into(),
    }
}

/// Number of icon drawings offered by Change Icon (workbook grid, chart).
pub const ICON_STYLES: u8 = 2;

/// The icon picture (PNG) and its size in points for `caption`; `style` picks the drawing
/// inside the page (0 = cells, 1 = a column chart).
pub fn icon_png(
    fonts: &mut FontManager,
    caption: &str,
    style: u8,
) -> Option<(Vec<u8>, (f64, f64))> {
    let caption: String = caption.chars().take(CAPTION_MAX).collect();
    let face = ["Segoe UI", "Calibri", "Carlito", "Liberation Sans"]
        .iter()
        .find_map(|f| fonts.resolve(f, false, false))?;
    let (glyphs, width) = {
        let upem = f32::from(fonts.face(face)?.units_per_em().max(1));
        let k = CAPTION_PT / upem;
        let mut x = 0.0;
        let mut out = Vec::new();
        for (i, c) in caption.char_indices() {
            let g = fonts.glyph_of(face, c);
            out.push(Glyph(g, x, 0.0, u32::try_from(i).unwrap_or(0)));
            x += f32::from(fonts.glyph_extents(face, g).advance) * k;
        }
        (out, x)
    };
    let w = (width + 4.0).max(ICON + 8.0).ceil();
    let h = ICON + GAP + CAPTION_PT * 1.4;
    let ix = ((w - ICON) / 2.0).round();
    let mut commands = Vec::new();
    // A sheet of paper with a folded corner (own drawing)…
    let fold = 8.0;
    let (l, t, r, b) = (ix + 4.0, 0.5, ix + ICON - 4.0, ICON - 0.5);
    commands.push(Command::Path {
        segs: vec![
            PathSeg::M(l, t),
            PathSeg::L(r - fold, t),
            PathSeg::L(r, t + fold),
            PathSeg::L(r, b),
            PathSeg::L(l, b),
            PathSeg::Z,
        ],
        fill: Some(Paint::Solid {
            color: Rgba(255, 255, 255, 255),
        }),
        stroke: Some(stroke(1.0, Rgba(0x80, 0x80, 0x80, 255))),
    });
    commands.push(Command::Path {
        segs: vec![
            PathSeg::M(r - fold, t),
            PathSeg::L(r - fold, t + fold),
            PathSeg::L(r, t + fold),
        ],
        fill: None,
        stroke: Some(stroke(1.0, Rgba(0x80, 0x80, 0x80, 255))),
    });
    if style % ICON_STYLES == 1 {
        // …holding three accent columns on a base line (a chart).
        let base = b - 5.0;
        let cw = (r - l - 8.0) / 4.0;
        for (n, frac) in [0.45_f32, 0.8, 0.6].into_iter().enumerate() {
            let hgt = (base - t - 10.0) * frac;
            commands.push(Command::FillRect {
                x: l + 4.0 + cw * (n as f32 * 1.3),
                y: base - hgt,
                w: cw,
                h: hgt,
                color: ACCENT,
            });
        }
        commands.push(Command::Line {
            x1: l + 3.0,
            y1: base,
            x2: r - 3.0,
            y2: base,
            width: 0.75,
            color: Rgba(0x60, 0x60, 0x60, 255),
            dash: None,
        });
    } else {
        // …holding a small grid of cells with an accent header row.
        let (gx, gy, cw, ch) = (l + 3.0, t + 11.0, (r - l - 6.0) / 3.0, 4.0);
        commands.push(Command::FillRect {
            x: gx,
            y: gy,
            w: cw * 3.0,
            h: ch,
            color: ACCENT,
        });
        let grid = stroke(0.75, ACCENT);
        for i in 0..=4 {
            let y = gy + ch * i as f32;
            commands.push(Command::Line {
                x1: gx,
                y1: y,
                x2: gx + cw * 3.0,
                y2: y,
                width: grid.width,
                color: ACCENT,
                dash: None,
            });
        }
        for j in 0..=3 {
            let x = gx + cw * j as f32;
            commands.push(Command::Line {
                x1: x,
                y1: gy,
                x2: x,
                y2: gy + ch * 4.0,
                width: grid.width,
                color: ACCENT,
                dash: None,
            });
        }
    }
    commands.push(Command::Glyphs {
        face: face.0,
        size: CAPTION_PT,
        x: ((w - width) / 2.0).max(0.0),
        y: ICON + GAP + CAPTION_PT,
        color: Rgba(0, 0, 0, 255),
        text: caption,
        glyphs,
    });
    let page = PageList {
        width: w,
        height: h,
        commands,
    };
    let png = bo_raster::render_page_png(&page, fonts, SCALE).ok()?;
    Some((png, (f64::from(w), f64::from(h))))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icon_picture_holds_the_caption_width() {
        let mut fonts = FontManager::new();
        for f in crate::bundled::FONTS {
            fonts.add_font(f.to_vec());
        }
        let (png, (w, h)) = icon_png(&mut fonts, "Worksheet", 0).unwrap();
        assert!(png.starts_with(b"\x89PNG"));
        assert!((40.0..80.0).contains(&w), "{w}");
        assert!(h > 40.0 && h < 60.0, "{h}");
        let (_, (w2, _)) = icon_png(&mut fonts, "A much longer caption for the object", 1).unwrap();
        assert!(w2 > w);
    }
}
