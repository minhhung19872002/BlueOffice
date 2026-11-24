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
    custom: Option<&bo_raster::codecs::Rgba>,
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
    // Change Icon › Browse…: the file's own icon is put in place after rendering.
    if custom.is_none() {
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
    let png = match custom {
        Some(icon) => {
            let mut out = bo_raster::decode_png_rgba(&png)?;
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            blit(
                &mut out,
                icon,
                (ix * SCALE) as u32,
                0,
                (ICON * SCALE) as u32,
            );
            bo_raster::rgba_to_png(out)?
        }
        None => png,
    };
    Some((png, (f64::from(w), f64::from(h))))
}

/// Draw `src` scaled into the `size` × `size` square at (`x`, `y`) of `dst`, blended by its
/// alpha (nearest pixel).
fn blit(
    dst: &mut bo_raster::codecs::Rgba,
    src: &bo_raster::codecs::Rgba,
    x: u32,
    y: u32,
    size: u32,
) {
    if src.width == 0 || src.height == 0 {
        return;
    }
    for dy in 0..size {
        for dx in 0..size {
            let (tx, ty) = (x + dx, y + dy);
            if tx >= dst.width || ty >= dst.height {
                continue;
            }
            let sx = (u64::from(dx) * u64::from(src.width) / u64::from(size)) as usize;
            let sy = (u64::from(dy) * u64::from(src.height) / u64::from(size)) as usize;
            let si = (sy * src.width as usize + sx) * 4;
            let di = (ty as usize * dst.width as usize + tx as usize) * 4;
            let (Some(s), true) = (src.pixels.get(si..si + 4), di + 4 <= dst.pixels.len()) else {
                continue;
            };
            let a = u32::from(s[3]);
            for (c, sv) in s.iter().take(3).enumerate() {
                let d = u32::from(dst.pixels[di + c]);
                dst.pixels[di + c] = ((u32::from(*sv) * a + d * (255 - a)) / 255) as u8;
            }
            let da = u32::from(dst.pixels[di + 3]);
            dst.pixels[di + 3] = (a + da * (255 - a) / 255).min(255) as u8;
        }
    }
}

fn u16_at(b: &[u8], i: usize) -> Option<u16> {
    Some(u16::from_le_bytes(b.get(i..i + 2)?.try_into().ok()?))
}

fn u32_at(b: &[u8], i: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(i..i + 4)?.try_into().ok()?))
}

/// One icon image: a PNG, or a DIB whose height counts the AND mask too (halved first).
fn icon_image(data: &[u8]) -> Option<bo_raster::codecs::Rgba> {
    if data.starts_with(b"\x89PNG") {
        return bo_raster::decode_png_rgba(data);
    }
    let mut dib = data.to_vec();
    let h = i32::from_le_bytes(dib.get(8..12)?.try_into().ok()?);
    let half = (h / 2).to_le_bytes();
    dib.get_mut(8..12)?.copy_from_slice(&half);
    bo_raster::codecs::decode_dib(&dib)
}

/// The largest icon of an `.ico` file, a PNG picture, or the largest `RT_ICON` resource of
/// a Windows program / library (`.exe`, `.dll`: the PE resource directory is only read,
/// nothing is loaded or run). `None` when there is none.
pub fn decode_icon_file(b: &[u8]) -> Option<bo_raster::codecs::Rgba> {
    if b.starts_with(b"\x89PNG") {
        return bo_raster::decode_png_rgba(b);
    }
    if b.len() > 6 && u16_at(b, 0)? == 0 && u16_at(b, 2)? == 1 {
        // ICONDIR: entries of 16 bytes; the largest by bytes.
        let n = usize::from(u16_at(b, 4)?);
        let best = (0..n)
            .filter_map(|i| {
                let e = 6 + i * 16;
                Some((u32_at(b, e + 8)? as usize, u32_at(b, e + 12)? as usize))
            })
            .max_by_key(|(size, _)| *size)?;
        return icon_image(b.get(best.1..best.1.checked_add(best.0)?)?);
    }
    if b.starts_with(b"MZ") {
        return pe_icon(b);
    }
    None
}

/// The largest `RT_ICON` (type 3) data of a PE file's resource directory.
fn pe_icon(b: &[u8]) -> Option<bo_raster::codecs::Rgba> {
    let pe = u32_at(b, 0x3C)? as usize;
    if b.get(pe..pe + 4)? != b"PE\0\0" {
        return None;
    }
    let coff = pe + 4;
    let sections = usize::from(u16_at(b, coff + 2)?);
    let opt = coff + 20;
    let opt_size = usize::from(u16_at(b, coff + 16)?);
    let dirs = match u16_at(b, opt)? {
        0x10b => opt + 96,
        0x20b => opt + 112,
        _ => return None,
    };
    let res_rva = u32_at(b, dirs + 2 * 8)?;
    let sec = opt + opt_size;
    let to_off = |rva: u32| -> Option<usize> {
        (0..sections.min(96)).find_map(|i| {
            let s = sec + i * 40;
            let (vsize, va, raw) = (u32_at(b, s + 8)?, u32_at(b, s + 12)?, u32_at(b, s + 20)?);
            (rva >= va && rva < va.checked_add(vsize.max(u32_at(b, s + 16)?))?)
                .then(|| (raw + (rva - va)) as usize)
        })
    };
    let root = to_off(res_rva)?;
    // (id or name, offset with the subdirectory bit) of a resource directory's entries.
    let entries = |dir: usize| -> Vec<(u32, u32)> {
        let named = usize::from(u16_at(b, dir + 12).unwrap_or(0));
        let ids = usize::from(u16_at(b, dir + 14).unwrap_or(0));
        (0..(named + ids).min(4096))
            .filter_map(|i| {
                let e = dir + 16 + i * 8;
                Some((u32_at(b, e)?, u32_at(b, e + 4)?))
            })
            .collect()
    };
    let sub = |off: u32| (off & 0x8000_0000 != 0).then(|| root + (off & 0x7FFF_FFFF) as usize);
    let icons = entries(root)
        .into_iter()
        .find(|(id, _)| *id == 3)
        .and_then(|(_, off)| sub(off))?;
    let mut best: Option<(u32, u32)> = None;
    for (_, off) in entries(icons) {
        let Some(langs) = sub(off) else { continue };
        for (_, data) in entries(langs) {
            if data & 0x8000_0000 != 0 {
                continue;
            }
            let d = root + data as usize;
            let (Some(rva), Some(size)) = (u32_at(b, d), u32_at(b, d + 4)) else {
                continue;
            };
            if best.is_none_or(|(_, s)| size > s) {
                best = Some((rva, size));
            }
        }
    }
    let (rva, size) = best?;
    let start = to_off(rva)?;
    icon_image(b.get(start..start.checked_add(size as usize)?)?)
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
        let (png, (w, h)) = icon_png(&mut fonts, "Worksheet", 0, None).unwrap();
        assert!(png.starts_with(b"\x89PNG"));
        assert!((40.0..80.0).contains(&w), "{w}");
        assert!(h > 40.0 && h < 60.0, "{h}");
        let (_, (w2, _)) =
            icon_png(&mut fonts, "A much longer caption for the object", 1, None).unwrap();
        // A picture file as the icon (Change Icon › Browse…).
        let red = bo_raster::codecs::Rgba {
            width: 2,
            height: 2,
            pixels: [255u8, 0, 0, 255].repeat(4),
        };
        let file = bo_raster::rgba_to_png(red).unwrap();
        let icon = decode_icon_file(&file).unwrap();
        let (png, _) = icon_png(&mut fonts, "Data", 0, Some(&icon)).unwrap();
        let back = bo_raster::decode_png_rgba(&png).unwrap();
        let mid = ((60 * back.width + back.width / 2) * 4) as usize;
        assert_eq!(&back.pixels[mid..mid + 3], &[255, 0, 0]);
        assert!(decode_icon_file(b"MZ not a program").is_none());
        assert!(w2 > w);
    }
}
