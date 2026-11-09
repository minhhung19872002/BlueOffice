//! Signature lines (CORE-SIGLINE-001; ECMA-376 Part 4, §19.2.2.30 signatureline):
//! drawn from their attributes rather than from the stored picture, so the same
//! rendition serves new lines, lines read from other producers (whose picture may
//! be a metafile) and the signed state found in the package signatures.
//!
//! Layout (BlueOffice's own design): a cross and a rule across the lower part of
//! the box; the suggested signer and their title under the rule; once signed, the
//! typed name (or the signature picture) above the rule and the signing date in
//! the top-right corner. A line whose signature failed verification is framed in
//! red.

use crate::drawing::{layer_of, Drawing};
use crate::types::{LineSource, Page, PageDecor, PageItem, PathCmd, ShapePath};
use bo_model::{Color, Image, SignatureLine};
use bo_text::{FontManager, PathCmd as OutlineCmd};

const INK: Color = Color {
    r: 0x26,
    g: 0x26,
    b: 0x26,
};
const MUTED: Color = Color {
    r: 0x59,
    g: 0x59,
    b: 0x59,
};
const SIGNED_INK: Color = Color {
    r: 0x1F,
    g: 0x3A,
    b: 0x93,
};
const INVALID: Color = Color {
    r: 0xC0,
    g: 0x1C,
    b: 0x28,
};

/// Glyph outlines of `text` with its top-left corner at (`x`, `top`), `size`
/// points high (ascender to descender), shrunk to fit `max_w`.
#[allow(clippy::too_many_arguments)]
fn text_outline(
    fonts: &mut FontManager,
    text: &str,
    family: &str,
    italic: bool,
    x: f32,
    top: f32,
    size: f32,
    max_w: f32,
) -> Vec<PathCmd> {
    let text = text.replace(['\r', '\n'], " ");
    if text.trim().is_empty() || size <= 0.0 || max_w <= 0.0 {
        return Vec::new();
    }
    let Some(face) = fonts
        .resolve(family, false, italic)
        .or_else(|| fonts.resolve("Calibri", false, italic))
    else {
        return Vec::new();
    };
    let glyphs = fonts.shape(face, &text);
    let info = fonts.info(face);
    let (asc, desc) = (f32::from(info.ascender), f32::from(info.descender));
    let advance: f32 = glyphs.iter().map(|g| g.x_advance as f32).sum();
    if advance <= 0.0 || asc - desc <= 0.0 {
        return Vec::new();
    }
    let mut s = size / (asc - desc);
    if advance * s > max_w {
        s = max_w / advance;
    }
    let mut segs = Vec::new();
    let mut pen = 0.0f32;
    for g in glyphs.iter() {
        let ox = pen + g.x_offset as f32;
        let oy = g.y_offset as f32;
        // Font units, y up → box points, y down.
        let p = |px: f32, py: f32| (x + (ox + px) * s, top + (asc - oy - py) * s);
        for c in fonts.outline(face, g.glyph_id) {
            segs.push(match c {
                OutlineCmd::MoveTo(a, b) => {
                    let (a, b) = p(a, b);
                    PathCmd::M(a, b)
                }
                OutlineCmd::LineTo(a, b) => {
                    let (a, b) = p(a, b);
                    PathCmd::L(a, b)
                }
                OutlineCmd::QuadTo(a, b, c, d) => {
                    let ((a, b), (c, d)) = (p(a, b), p(c, d));
                    PathCmd::Q(a, b, c, d)
                }
                OutlineCmd::CubicTo(a, b, c, d, e, f) => {
                    let ((a, b), (c, d), (e, f)) = (p(a, b), p(c, d), p(e, f));
                    PathCmd::C(a, b, c, d, e, f)
                }
                OutlineCmd::Close => PathCmd::Z,
            });
        }
        pen += g.x_advance as f32;
    }
    segs
}

fn stroke(segs: Vec<PathCmd>, width: f32, color: Color) -> ShapePath {
    ShapePath {
        segs,
        fill: None,
        stroke: Some((width, color, None)),
    }
}

fn filled(segs: Vec<PathCmd>, color: Color) -> Option<ShapePath> {
    (!segs.is_empty()).then_some(ShapePath {
        segs,
        fill: Some(color),
        stroke: None,
    })
}

/// The drawing of signature line `line` in the box of `img` (anchor offset `at`).
pub(crate) fn signature_line_drawing(
    fonts: &mut FontManager,
    img: &Image,
    line: &SignatureLine,
    at: u32,
) -> Drawing {
    let (w, h) = (img.width.to_pt().max(1.0), img.height.to_pt().max(1.0));
    let pad = (w * 0.04).min(8.0);
    let rule_y = h * 0.58;
    let caption = (h * 0.13).clamp(5.0, 11.0);
    let mut paths = Vec::new();
    // The cross marking where to sign, sitting on the rule.
    let xs = (rule_y * 0.3).min(14.0);
    let (x0, y0) = (pad, rule_y - xs - 3.0);
    paths.push(stroke(
        vec![
            PathCmd::M(x0, y0),
            PathCmd::L(x0 + xs, y0 + xs),
            PathCmd::M(x0 + xs, y0),
            PathCmd::L(x0, y0 + xs),
        ],
        1.25,
        INK,
    ));
    // The rule.
    paths.push(stroke(
        vec![PathCmd::M(pad, rule_y), PathCmd::L(w - pad, rule_y)],
        0.75,
        INK,
    ));
    // Suggested signer and title under the rule.
    let mut y = rule_y + caption * 0.35;
    for (text, color) in [
        (&line.suggested_signer, INK),
        (&line.suggested_signer2, MUTED),
    ] {
        if !text.is_empty() {
            paths.extend(filled(
                text_outline(
                    fonts,
                    text,
                    "Calibri",
                    false,
                    pad,
                    y,
                    caption,
                    w - 2.0 * pad,
                ),
                color,
            ));
            y += caption * 1.1;
        }
    }
    let mut items = Vec::new();
    if let Some(signed) = &line.signed {
        let invalid = signed.valid == Some(false);
        let ink = if invalid { INVALID } else { SIGNED_INK };
        let left = x0 + xs + 6.0;
        let avail_w = (w - pad - left).max(1.0);
        let top = (h * 0.12).max(2.0);
        let avail_h = (rule_y - 3.0 - top).max(1.0);
        if signed.image.is_some() {
            items.push(PageItem::Image {
                x: left,
                y: top,
                w: avail_w,
                h: avail_h,
                media: SignatureLine::image_key(&line.id),
                crop: None,
                rotation: 0.0,
                layer: layer_of(img),
                alpha: 1.0,
            });
        } else {
            let size = avail_h.min(28.0);
            paths.extend(filled(
                text_outline(
                    fonts,
                    &signed.text,
                    "Times New Roman",
                    true,
                    left,
                    rule_y - 3.0 - size,
                    size,
                    avail_w,
                ),
                ink,
            ));
        }
        if line.show_sign_date && !signed.date.is_empty() {
            let size = caption * 0.9;
            let date_w = w * 0.4;
            paths.extend(filled(
                text_outline(
                    fonts,
                    &signed.date,
                    "Calibri",
                    false,
                    w - pad - date_w,
                    2.0,
                    size,
                    date_w,
                ),
                MUTED,
            ));
        }
        if invalid {
            paths.push(stroke(
                vec![
                    PathCmd::M(0.5, 0.5),
                    PathCmd::L(w - 0.5, 0.5),
                    PathCmd::L(w - 0.5, h - 0.5),
                    PathCmd::L(0.5, h - 0.5),
                    PathCmd::Z,
                ],
                1.0,
                INVALID,
            ));
        }
    }
    let mut out = Drawing::default();
    out.items.push(PageItem::Shape {
        x: 0.0,
        y: 0.0,
        w,
        h,
        paths,
        source: LineSource::TextBox {
            para: 0,
            at,
            sub: 0,
        },
        layer: layer_of(img),
        alpha: 1.0,
    });
    out.items.extend(items);
    out
}

/// A page exactly the size of signature line `img` showing it: the picture of
/// the line stored in the document, and the line pictures of a signature
/// ([MS-OFFCRYPTO] idValidSigLnImg / idInvalidSigLnImg), rendered by the hosts.
#[must_use]
pub fn signature_line_page(img: &Image, line: &SignatureLine, fonts: &mut FontManager) -> Page {
    let d = signature_line_drawing(fonts, img, line, 0);
    Page {
        width: img.width.to_pt().max(1.0),
        height: img.height.to_pt().max(1.0),
        lines: Vec::new(),
        items: d.items,
        number: 1,
        section: 0,
        decor: PageDecor::default(),
    }
}
