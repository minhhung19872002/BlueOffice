//! Slide show playback for BlueSlide's native window (N6, round 2): slide transitions and
//! the effects of the main sequence as the engine describes them (`slide_edit::show`:
//! transition kind, variant and duration; show layers; each effect's click step, timing and
//! behaviours read from its §19.5 time nodes). Drawing composes the stored slide and layer
//! images with [`Painter::page_part`] (offset, scale, clip, opacity), so a frame costs a few
//! bitmap draws and keeps the display's frame rate.
use crate::Painter;
use bo_engine::slide_edit::show::{AnimOut, ShowOut, TransitionOut};
use bo_pptx::anim_play::TrackKind;
use bo_render::Rgba;

/// How a layer looks at an instant.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Look {
    /// Shown at all.
    pub visible: bool,
    /// Opacity 0–1.
    pub opacity: f32,
    /// Offset (slide widths, heights).
    pub dx: f64,
    /// Vertical offset (slide heights).
    pub dy: f64,
    /// Scale of the shape about its centre (x, y).
    pub sx: f64,
    /// Vertical scale.
    pub sy: f64,
    /// Wipe: the side the shape is revealed from (`d` top to bottom, `u`, `l`, `r`) and the
    /// revealed fraction.
    pub wipe: Option<(char, f64)>,
}

impl Look {
    const SHOWN: Look = Look {
        visible: true,
        opacity: 1.0,
        dx: 0.0,
        dy: 0.0,
        sx: 1.0,
        sy: 1.0,
        wipe: None,
    };
    const HIDDEN: Look = Look {
        visible: false,
        ..Look::SHOWN
    };
}

/// Linear interpolation through keyframes (time 0..1, value).
fn keyframes(keys: &[(f64, f64)], t: f64, discrete: bool) -> Option<f64> {
    let first = keys.first()?;
    if t <= first.0 {
        return Some(first.1);
    }
    for w in keys.windows(2) {
        let (a, b) = (w[0], w[1]);
        if t <= b.0 {
            if discrete {
                return Some(a.1);
            }
            let k = if b.0 > a.0 {
                (t - a.0) / (b.0 - a.0)
            } else {
                1.0
            };
            return Some(a.1 + (b.1 - a.1) * k);
        }
    }
    keys.last().map(|k| k.1)
}

/// Point along a polyline at fraction `t` of its length.
fn along(points: &[(f64, f64)], t: f64) -> (f64, f64) {
    let total: f64 = points
        .windows(2)
        .map(|w| (w[1].0 - w[0].0).hypot(w[1].1 - w[0].1))
        .sum();
    if points.len() < 2 || total <= 0.0 {
        return points.first().copied().unwrap_or((0.0, 0.0));
    }
    let mut left = t.clamp(0.0, 1.0) * total;
    for w in points.windows(2) {
        let d = (w[1].0 - w[0].0).hypot(w[1].1 - w[0].1);
        if left <= d && d > 0.0 {
            let k = left / d;
            return (
                w[0].0 + (w[1].0 - w[0].0) * k,
                w[0].1 + (w[1].1 - w[0].1) * k,
            );
        }
        left -= d;
    }
    points.last().copied().unwrap_or((0.0, 0.0))
}

/// Direction letter of a wipe filter (`wipe(down)` reveals from the top).
fn wipe_side(filter: &str) -> Option<char> {
    let sub = filter.strip_prefix("wipe(")?.trim_end_matches(')');
    Some(match sub {
        "up" => 'u',
        "left" => 'l',
        "right" => 'r',
        _ => 'd',
    })
}

/// How effect `a` leaves its shape `ms` after it started, from `before`; slide size in
/// points gives the shape's centre and size as fractions (`#ppt_x` …).
pub fn effect_look(a: &AnimOut, ms: f64, before: Look, slide: (f64, f64)) -> Look {
    let mut look = before;
    let base = a.rect.map_or((0.5, 0.5, 0.0, 0.0), |[x, y, w, h]| {
        (
            (x + w / 2.0) / slide.0,
            (y + h / 2.0) / slide.1,
            w / slide.0,
            h / slide.1,
        )
    });
    look.opacity = 1.0;
    look.dx = 0.0;
    look.dy = 0.0;
    look.sx = 1.0;
    look.sy = 1.0;
    look.wipe = None;
    for tr in &a.play.tracks {
        let Some(p) = tr.progress(ms) else {
            continue;
        };
        match &tr.kind {
            TrackKind::Set { attr, value } if attr == "style.visibility" => {
                look.visible = value == "visible";
            }
            TrackKind::Anim {
                attr,
                keys,
                discrete,
            } => {
                let Some(v) = keyframes(keys, p, *discrete) else {
                    continue;
                };
                match attr.as_str() {
                    "ppt_x" => look.dx = v - base.0,
                    "ppt_y" => look.dy = v - base.1,
                    "ppt_w" if base.2 > 0.0 => look.sx = v / base.2,
                    "ppt_h" if base.3 > 0.0 => look.sy = v / base.3,
                    "style.opacity" => look.opacity = v.clamp(0.0, 1.0) as f32,
                    _ => {}
                }
            }
            TrackKind::Filter { filter, entering } => {
                let q = if *entering { p } else { 1.0 - p };
                match wipe_side(filter) {
                    Some(side) => look.wipe = Some((side, q)),
                    // Other filters (blinds, dissolve, …) play as a fade.
                    None => look.opacity *= q as f32,
                }
            }
            TrackKind::Motion { points } => {
                let (x, y) = along(points, p);
                look.dx += x;
                look.dy += y;
            }
            TrackKind::Scale { from, to } => {
                look.sx *= from.0 + (to.0 - from.0) * p;
                look.sy *= from.1 + (to.1 - from.1) * p;
            }
            _ => {}
        }
    }
    look
}

/// The look of every layer at click `step`, `elapsed` ms into that step's effects
/// (`f64::INFINITY` once they are done).
pub fn layer_looks(show: &ShowOut, step: usize, elapsed: f64) -> Vec<Look> {
    let slide = (f64::from(show.width), f64::from(show.height));
    (0..show.layers.len())
        .map(|li| {
            let mine: Vec<&AnimOut> = show
                .animations
                .iter()
                .filter(|a| a.layer == Some(li) && a.trigger_shape.is_none())
                .collect();
            // A shape whose first effect brings it in starts hidden.
            let mut look = match mine.first() {
                Some(a) if a.class == "entr" => Look::HIDDEN,
                _ => Look::SHOWN,
            };
            for a in mine {
                let done = a.click < step || (a.click == step && elapsed >= f64::from(a.end_ms));
                let running = a.click == step && elapsed >= f64::from(a.start_ms) && !done;
                if done {
                    look = match a.class {
                        "exit" if !a.rewind => Look::HIDDEN,
                        "entr" | "emph" | "exit" => Look::SHOWN,
                        _ => {
                            // Motion paths stay where they end.
                            let end = effect_look(a, f64::INFINITY, look, slide);
                            Look {
                                visible: look.visible,
                                ..end
                            }
                        }
                    };
                } else if running {
                    look = effect_look(a, elapsed - f64::from(a.start_ms), look, slide);
                } else {
                    break;
                }
            }
            look
        })
        .collect()
}

/// End of the effects of click `step` (ms after the click).
pub fn step_end(show: &ShowOut, step: usize) -> f64 {
    show.animations
        .iter()
        .filter(|a| a.click == step && a.trigger_shape.is_none())
        .map(|a| f64::from(a.end_ms))
        .fold(0.0, f64::max)
}

/// Ease in and out (smoothstep), as moving transitions start and stop softly.
fn ease(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// A running transition into a slide.
#[derive(Debug, Clone, PartialEq)]
pub struct Transition {
    /// Engine kind (`fade`, `push`, …).
    pub kind: String,
    /// Variant (direction, …).
    pub dir: Option<String>,
    /// Duration, ms.
    pub dur_ms: f64,
}

impl Transition {
    /// The transition of a slide, if it animates.
    pub fn of(t: Option<&TransitionOut>) -> Option<Transition> {
        let t = t?;
        if matches!(t.kind.as_str(), "none" | "cut") || t.dur_ms == 0 {
            return None;
        }
        Some(Transition {
            kind: t.kind.clone(),
            dir: t.dir.clone(),
            dur_ms: f64::from(t.dur_ms.max(1)),
        })
    }
}

/// Something that draws one slide's state into a rectangle (offset, clip, opacity).
pub trait SlideImage {
    /// Draw the slide into `dest` (device pixels) at `opacity`, showing only the `part` of
    /// it (fractions of the slide: x, y, w, h).
    fn draw(&self, p: &mut dyn Painter, dest: [f32; 4], part: [f32; 4], opacity: f32);
}

/// Draw `part` of an image `key` of a slide whose image is `size` device pixels into `dest`.
pub fn draw_part(
    p: &mut dyn Painter,
    key: usize,
    size: (f32, f32),
    dest: [f32; 4],
    part: [f32; 4],
    opacity: f32,
) {
    let src = [
        part[0] * size.0,
        part[1] * size.1,
        part[2] * size.0,
        part[3] * size.1,
    ];
    let d = [
        dest[0] + part[0] * dest[2],
        dest[1] + part[1] * dest[3],
        part[2] * dest[2],
        part[3] * dest[3],
    ];
    p.page_part(key, src, d, opacity);
}

const WHOLE: [f32; 4] = [0.0, 0.0, 1.0, 1.0];

/// Draw a transition `t` of the way (0..1) from `from` (`None`: black) to `to` in `r`.
pub fn paint_transition(
    p: &mut dyn Painter,
    tr: &Transition,
    t: f64,
    r: [f32; 4],
    from: Option<&dyn SlideImage>,
    to: &dyn SlideImage,
) {
    let [x, y, w, h] = r;
    let black = Rgba(0, 0, 0, 255);
    let old = |p: &mut dyn Painter, dest: [f32; 4], part: [f32; 4], o: f32| match from {
        Some(f) => f.draw(p, dest, part, o),
        None => p.fill_rect(
            dest[0] + part[0] * dest[2],
            dest[1] + part[1] * dest[3],
            part[2] * dest[2],
            part[3] * dest[3],
            black,
        ),
    };
    let dir = tr.dir.as_deref().unwrap_or("");
    // Unit vector the moving content travels along (`l`: towards the left, …).
    let (ux, uy) = match dir {
        "r" => (1.0, 0.0),
        "u" => (0.0, -1.0),
        "d" => (0.0, 1.0),
        "lu" => (-1.0, -1.0),
        "ru" => (1.0, -1.0),
        "ld" => (-1.0, 1.0),
        "rd" => (1.0, 1.0),
        _ => (-1.0, 0.0),
    };
    let tf = t as f32;
    match tr.kind.as_str() {
        "push" => {
            let e = ease(t) as f32;
            let (ox, oy) = (ux * w * e, uy * h * e);
            p.push_clip(x, y, w, h);
            old(p, [x + ox, y + oy, w, h], WHOLE, 1.0);
            to.draw(p, [x + ox - ux * w, y + oy - uy * h, w, h], WHOLE, 1.0);
            p.pop_clip();
        }
        "cover" => {
            let e = 1.0 - ease(t) as f32;
            old(p, r, WHOLE, 1.0);
            p.push_clip(x, y, w, h);
            to.draw(p, [x - ux * w * e, y - uy * h * e, w, h], WHOLE, 1.0);
            p.pop_clip();
        }
        "pull" => {
            let e = ease(t) as f32;
            to.draw(p, r, WHOLE, 1.0);
            p.push_clip(x, y, w, h);
            old(p, [x + ux * w * e, y + uy * h * e, w, h], WHOLE, 1.0);
            p.pop_clip();
        }
        "wipe" => {
            old(p, r, WHOLE, 1.0);
            // The new slide is revealed from the side opposite to the motion.
            let part = match dir {
                "r" => [0.0, 0.0, tf, 1.0],
                "u" => [0.0, 1.0 - tf, 1.0, tf],
                "d" => [0.0, 0.0, 1.0, tf],
                _ => [1.0 - tf, 0.0, tf, 1.0],
            };
            to.draw(p, r, part, 1.0);
        }
        "split" => {
            old(p, r, WHOLE, 1.0);
            let out = !dir.ends_with("In") && dir != "in";
            let vert = dir.starts_with("vert");
            let k = if out { tf } else { 1.0 - tf };
            if out {
                let part = if vert {
                    [0.5 - k / 2.0, 0.0, k, 1.0]
                } else {
                    [0.0, 0.5 - k / 2.0, 1.0, k]
                };
                to.draw(p, r, part, 1.0);
            } else {
                // Closing in: the new slide comes from both edges towards the middle.
                let (a, b) = if vert {
                    (
                        [0.0, 0.0, (1.0 - k) / 2.0, 1.0],
                        [0.5 + k / 2.0, 0.0, (1.0 - k) / 2.0, 1.0],
                    )
                } else {
                    (
                        [0.0, 0.0, 1.0, (1.0 - k) / 2.0],
                        [0.0, 0.5 + k / 2.0, 1.0, (1.0 - k) / 2.0],
                    )
                };
                to.draw(p, r, a, 1.0);
                to.draw(p, r, b, 1.0);
            }
        }
        "zoom" => {
            let e = ease(t) as f32;
            old(p, r, WHOLE, 1.0);
            let k = if dir == "out" {
                1.6 - 0.6 * e
            } else {
                0.3 + 0.7 * e
            };
            let (zw, zh) = (w * k, h * k);
            p.push_clip(x, y, w, h);
            to.draw(
                p,
                [x + (w - zw) / 2.0, y + (h - zh) / 2.0, zw, zh],
                WHOLE,
                e,
            );
            p.pop_clip();
        }
        _ if dir == "black" => {
            p.fill_rect(x, y, w, h, black);
            if tf < 0.5 {
                old(p, r, WHOLE, 1.0 - 2.0 * tf);
            } else {
                to.draw(p, r, WHOLE, 2.0 * tf - 1.0);
            }
        }
        // Fade, and every other effect played as a fade.
        _ => {
            old(p, r, WHOLE, 1.0);
            to.draw(p, r, WHOLE, tf);
        }
    }
}

/// Draw a layer image (`key`, full slide, `size` device pixels) with `look` into slide
/// rectangle `r`; `rect` is the animated shape's box in points of a slide `slide` points big.
#[allow(clippy::too_many_arguments)]
pub fn paint_layer(
    p: &mut dyn Painter,
    key: usize,
    size: (f32, f32),
    r: [f32; 4],
    look: &Look,
    rect: Option<[f64; 4]>,
    slide: (f64, f64),
    opacity: f32,
) {
    if !look.visible || look.opacity <= 0.0 {
        return;
    }
    let o = look.opacity * opacity;
    let [x, y, w, h] = r;
    let (ox, oy) = ((look.dx as f32) * w, (look.dy as f32) * h);
    let plain = (look.sx - 1.0).abs() < 1e-6 && (look.sy - 1.0).abs() < 1e-6 && look.wipe.is_none();
    let Some([bx, by, bw, bh]) = rect.filter(|_| !plain) else {
        draw_part(p, key, size, [x + ox, y + oy, w, h], WHOLE, o);
        return;
    };
    // The shape's box (a little larger for outlines and shadows) as fractions of the slide.
    let pad = 0.02;
    let mut part = [
        ((bx / slide.0) as f32 - pad).max(0.0),
        ((by / slide.1) as f32 - pad).max(0.0),
        0.0,
        0.0,
    ];
    part[2] = (((bx + bw) / slide.0) as f32 + pad).min(1.0) - part[0];
    part[3] = (((by + bh) / slide.1) as f32 + pad).min(1.0) - part[1];
    if let Some((side, q)) = look.wipe {
        let q = q.clamp(0.0, 1.0) as f32;
        match side {
            'u' => {
                part[1] += part[3] * (1.0 - q);
                part[3] *= q;
            }
            'l' => {
                part[0] += part[2] * (1.0 - q);
                part[2] *= q;
            }
            'r' => part[2] *= q,
            _ => part[3] *= q,
        }
    }
    if part[2] <= 0.0 || part[3] <= 0.0 {
        return;
    }
    let src = [
        part[0] * size.0,
        part[1] * size.1,
        part[2] * size.0,
        part[3] * size.1,
    ];
    // Scale about the shape's centre.
    let (cx, cy) = (
        x + ((bx + bw / 2.0) / slide.0) as f32 * w + ox,
        y + ((by + bh / 2.0) / slide.1) as f32 * h + oy,
    );
    let (sx, sy) = (look.sx.max(0.0) as f32, look.sy.max(0.0) as f32);
    let dx0 = x + part[0] * w + ox;
    let dy0 = y + part[1] * h + oy;
    let dest = [
        cx + (dx0 - cx) * sx,
        cy + (dy0 - cy) * sy,
        part[2] * w * sx,
        part[3] * h * sy,
    ];
    p.page_part(key, src, dest, o);
}
