//! Page items of a picture with a picture style (BlueDoc round 19): the effect images
//! (outer shadow, glow, reflection) around the picture, the picture itself with its
//! in-place effects (artistic effect, inner shadow, bevel, soft edges, 3-D rotation) and the
//! Picture Border (ECMA-376 Part 1, §20.1.2.2.24 ln) — see `bo_model::picture_style`.
//! Without a style the picture is one image item, as before.

use crate::types::{Layer, PageItem};
use bo_model::picture::FX_MARK;
use bo_model::picture_style::frac;
use bo_model::{Color, Image};

const EMU_PT: f64 = 12_700.0;

fn pt(emu: i64) -> f64 {
    emu as f64 / EMU_PT
}

fn push_token(id: &mut String, tok: &str) {
    if id.contains(FX_MARK) {
        id.push(';');
    } else {
        id.push_str(FX_MARK);
    }
    id.push_str(tok);
}

fn hex(c: u32) -> String {
    format!("{:06X}", c & 0xFF_FFFF)
}

/// The items of picture `img` drawn into the box (`x`, `y`, `w`, `h`) points.
#[allow(clippy::too_many_arguments, clippy::cast_possible_truncation)]
pub(crate) fn picture_items(
    img: &Image,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    layer: Layer,
    alpha: f32,
    z: u32,
) -> Vec<PageItem> {
    let media = img.display_id();
    let crop = crate::blocks::crop_of(img);
    let rotation = crate::blocks::rotation_of(img);
    let item =
        |x: f32, y: f32, w: f32, h: f32, media: String, crop: Option<[f32; 4]>| PageItem::Image {
            x,
            y,
            w,
            h,
            media,
            crop,
            rotation,
            layer,
            alpha,
            z,
        };
    let Some(f) = img.format.as_deref() else {
        return vec![item(x, y, w, h, media, crop)];
    };
    let fx = f.effects.as_deref().filter(|e| !e.is_empty());
    if fx.is_none() && f.line.is_none() && f.artistic.is_none() {
        return vec![item(x, y, w, h, media, crop)];
    }
    let (wf, hf) = (f64::from(w.max(0.01)), f64::from(h.max(0.01)));
    let crop_tok = crop.map(|c| {
        let c = c.map(|v| frac(f64::from(v)));
        format!("crop={},{},{},{}", c[0], c[1], c[2], c[3])
    });
    // The visible picture as the source of effect images.
    let mut base = media.clone();
    if let Some(t) = &crop_tok {
        push_token(&mut base, t);
    }
    let mut out = Vec::new();
    if let Some(e) = fx {
        if let Some(s) = e.shadow.as_ref().filter(|s| !s.inner) {
            let blur = pt(s.blur);
            let (sin, cos) = (f64::from(s.dir) / 60_000.0).to_radians().sin_cos();
            let (dx, dy) = (pt(s.dist) * cos, pt(s.dist) * sin);
            let (sx, sy) = (
                f64::from(if s.sx == 0 { 100_000 } else { s.sx }) / 100_000.0,
                f64::from(if s.sy == 0 { 100_000 } else { s.sy }) / 100_000.0,
            );
            let mut id = base.clone();
            push_token(
                &mut id,
                &format!(
                    "shadow={},{},{},{},{},{}",
                    hex(s.color),
                    s.alpha,
                    frac(blur / wf),
                    frac(blur / hf),
                    frac(blur / wf),
                    frac(blur / hf)
                ),
            );
            let (sw, sh) = (wf * sx, hf * sy);
            let (cx, cy) = (f64::from(x) + wf / 2.0 + dx, f64::from(y) + hf / 2.0 + dy);
            // §20.1.8.49 algn (default b): scaled about the bottom edge.
            let cy = cy + (hf - sh) / 2.0;
            out.push(item(
                (cx - sw / 2.0 - blur * sx) as f32,
                (cy - sh / 2.0 - blur * sy) as f32,
                (sw + 2.0 * blur * sx) as f32,
                (sh + 2.0 * blur * sy) as f32,
                id,
                None,
            ));
        }
        if let Some(g) = &e.glow {
            let r = pt(g.rad);
            let mut id = base.clone();
            push_token(
                &mut id,
                &format!(
                    "glow={},{},{},{}",
                    hex(g.color),
                    g.alpha,
                    frac(r / wf),
                    frac(r / hf)
                ),
            );
            out.push(item(
                (f64::from(x) - r) as f32,
                (f64::from(y) - r) as f32,
                (wf + 2.0 * r) as f32,
                (hf + 2.0 * r) as f32,
                id,
                None,
            ));
        }
    }
    // The picture with its in-place effects.
    let mut main_tokens: Vec<String> = Vec::new();
    if let Some(a) = &f.artistic {
        main_tokens.push(format!("art={}@{}", a.name, a.amount()));
    }
    if let Some(e) = fx {
        if let Some(s) = e.shadow.as_ref().filter(|s| s.inner) {
            let blur = pt(s.blur);
            let (sin, cos) = (f64::from(s.dir) / 60_000.0).to_radians().sin_cos();
            let (dx, dy) = (pt(s.dist) * cos, pt(s.dist) * sin);
            main_tokens.push(format!(
                "ish={},{},{},{},{},{}",
                hex(s.color),
                s.alpha,
                frac(blur / wf),
                frac(blur / hf),
                frac(dx / wf),
                frac(dy / hf)
            ));
        }
        if let Some(b) = &e.bevel {
            main_tokens.push(format!(
                "bevel={},{}",
                frac(pt(b.w) / wf),
                frac(pt(b.h.max(b.w)) / hf)
            ));
        }
        if let Some(r) = e.soft_edge.filter(|r| *r > 0) {
            main_tokens.push(format!("soft={},{}", frac(pt(r) / wf), frac(pt(r) / hf)));
        }
        if let Some(c) = &e.camera {
            main_tokens.push(format!("cam={c}"));
        }
    }
    let (main_id, main_crop) = if main_tokens.is_empty() {
        (media.clone(), crop)
    } else {
        let mut id = base.clone();
        for t in &main_tokens {
            push_token(&mut id, t);
        }
        (id, None)
    };
    if let Some(r) = fx.and_then(|e| e.reflection.as_ref()) {
        let mut id = main_id.clone();
        if main_crop.is_some() {
            id.clone_from(&base);
        }
        let blur = pt(r.blur);
        push_token(
            &mut id,
            &format!(
                "refl={},{},{},{},{}",
                r.st_a,
                r.end_a,
                r.end_pos,
                frac(blur / wf),
                frac(blur / hf)
            ),
        );
        out.push(item(x, y + h + pt(r.dist) as f32, w, h, id, None));
    }
    out.push(item(x, y, w, h, main_id, main_crop));
    if let Some(l) = &f.line {
        let width = (pt(l.width) as f32).max(0.25);
        let color = Color {
            r: ((l.color >> 16) & 255) as u8,
            g: ((l.color >> 8) & 255) as u8,
            b: (l.color & 255) as u8,
        };
        let style = match l.dash.as_deref() {
            Some("sysDot" | "dot") => "dotted",
            Some(_) => "dashed",
            None => "single",
        };
        // Centred on the box edge (§20.1.2.2.24 algn ctr), turned with the picture.
        let (cx, cy) = (x + w / 2.0, y + h / 2.0);
        let (s, c) = rotation.to_radians().sin_cos();
        let turn = |px: f32, py: f32| {
            let (dx, dy) = (px - cx, py - cy);
            (cx + dx * c - dy * s, cy + dx * s + dy * c)
        };
        let pts = [
            turn(x, y),
            turn(x + w, y),
            turn(x + w, y + h),
            turn(x, y + h),
        ];
        for i in 0..4 {
            let (a, b) = (pts[i], pts[(i + 1) % 4]);
            // Each side runs on by half the width so the corners are square.
            let len = ((b.0 - a.0).hypot(b.1 - a.1)).max(1e-3);
            let (ux, uy) = (
                (b.0 - a.0) / len * width / 2.0,
                (b.1 - a.1) / len * width / 2.0,
            );
            out.push(PageItem::Rule {
                x1: a.0 - ux,
                y1: a.1 - uy,
                x2: b.0 + ux,
                y2: b.1 + uy,
                width,
                color,
                style: style.to_owned(),
            });
        }
    }
    out
}
