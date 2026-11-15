//! Slide show playback for BlueSlide's native window (N6, rounds 2–3): slide transitions and
//! the effects of the main sequence as the engine describes them (`slide_edit::show`:
//! transition kind, variant and duration; show layers; each effect's click step, timing and
//! behaviours read from its §19.5 time nodes). Drawing composes the stored slide and layer
//! images with [`Painter::page_part`] (offset, scale, clip, opacity, turn), so a frame costs a
//! few bitmap draws and keeps the display's frame rate.
//!
//! Round 3: spin (§19.5.5 animRot), the §19.5.3 filters beyond wipe ([`Reveal`]), effects
//! built by paragraph (the layer's pieces, `LayerOut::parts`) and by word / letter (§19.5
//! iterate, `LayerOut::text`), the matching transitions, and Morph ([MS-PPTX] p159:morph:
//! matched shapes move, scale, turn and change outline; the others fade).
use crate::slide_reveal::Reveal;
use crate::Painter;
use bo_engine::slide_edit::morph::MorphOut;
use bo_engine::slide_edit::show::{AnimOut, ShowOut, TransitionOut};
use bo_pptx::anim_play::TrackKind;
use bo_render::{PageList, PathSeg, Rgba};

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
    /// A filter uncovering the shape and how far it is.
    pub reveal: Option<(Reveal, f64)>,
    /// Turn about the shape's centre, degrees clockwise.
    pub rot: f64,
}

impl Look {
    const SHOWN: Look = Look {
        visible: true,
        opacity: 1.0,
        dx: 0.0,
        dy: 0.0,
        sx: 1.0,
        sy: 1.0,
        reveal: None,
        rot: 0.0,
    };
    const HIDDEN: Look = Look {
        visible: false,
        ..Look::SHOWN
    };

    fn plain(&self) -> bool {
        (self.sx - 1.0).abs() < 1e-6
            && (self.sy - 1.0).abs() < 1e-6
            && self.reveal.is_none()
            && self.rot.abs() < 1e-3
    }
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

/// How effect `a` leaves its shape `ms` after it started, from `before`; slide size in
/// points and the box the effect moves (`pivot`, points) give the shape's centre and size as
/// fractions (`#ppt_x` …).
pub fn effect_look(
    a: &AnimOut,
    ms: f64,
    before: Look,
    slide: (f64, f64),
    pivot: Option<[f64; 4]>,
) -> Look {
    let mut look = before;
    let base = pivot
        .or(a.rect)
        .map_or((0.5, 0.5, 0.0, 0.0), |[x, y, w, h]| {
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
    look.reveal = None;
    look.rot = 0.0;
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
                    "r" | "style.rotation" => look.rot = v,
                    _ => {}
                }
            }
            TrackKind::Filter { filter, entering } => {
                let q = if *entering { p } else { 1.0 - p };
                match Reveal::from_filter(filter) {
                    Some(r) => look.reveal = Some((r, q)),
                    // Fades and filters without a shape play as a fade.
                    None => look.opacity *= q as f32,
                }
            }
            TrackKind::Motion { points } => {
                let (x, y) = along(points, p);
                look.dx += x;
                look.dy += y;
            }
            TrackKind::Rotate { by } => look.rot += by * p,
            TrackKind::Scale { from, to } => {
                look.sx *= from.0 + (to.0 - from.0) * p;
                look.sy *= from.1 + (to.1 - from.1) * p;
            }
            _ => {}
        }
    }
    look
}

/// One effect as it applies to a piece: its start and end (ms after its click).
struct Timed<'a> {
    a: &'a AnimOut,
    start: f64,
    end: f64,
}

/// Fold the effects of a piece into its look at click `step`, `elapsed` ms in.
fn fold(
    fx: &[Timed<'_>],
    step: usize,
    elapsed: f64,
    slide: (f64, f64),
    pivot: Option<[f64; 4]>,
) -> Look {
    // A piece whose first effect brings it in starts hidden.
    let mut look = match fx.first() {
        Some(t) if t.a.class == "entr" => Look::HIDDEN,
        _ => Look::SHOWN,
    };
    for t in fx {
        let a = t.a;
        let done = a.click < step || (a.click == step && elapsed >= t.end);
        let running = a.click == step && elapsed >= t.start && !done;
        if done {
            look = match a.class {
                "exit" if !a.rewind => Look::HIDDEN,
                "entr" | "emph" | "exit" => Look {
                    visible: true,
                    ..Look::SHOWN
                },
                _ => {
                    // Motion paths stay where they end.
                    let end = effect_look(a, f64::INFINITY, look, slide, pivot);
                    Look {
                        visible: look.visible,
                        ..end
                    }
                }
            };
        } else if running {
            look = effect_look(a, elapsed - t.start, look, slide, pivot);
        } else {
            break;
        }
    }
    look
}

/// Which images a layer is drawn from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayerMode {
    /// The layer's own drawing.
    Whole,
    /// Its body ([`LayerOut::parts`] page 0) and its pieces (paragraphs, chart elements).
    Parts,
    /// Its drawing without text and its letters (effects by word or letter).
    Text,
}

/// How the show draws layer `li` of `show`.
#[must_use]
pub fn layer_mode(show: &ShowOut, li: usize) -> LayerMode {
    let Some(layer) = show.layers.get(li) else {
        return LayerMode::Whole;
    };
    let mine = || {
        show.animations
            .iter()
            .filter(move |a| a.layer == Some(li) && a.trigger_shape.is_none())
    };
    if layer.text.is_some() && mine().any(|a| matches!(a.text_by.as_deref(), Some("wd" | "lt"))) {
        return LayerMode::Text;
    }
    if layer.parts.is_some() && mine().any(|a| !a.parts.is_empty()) {
        return LayerMode::Parts;
    }
    LayerMode::Whole
}

/// The drawings of layer `li` the show keeps as images: the base (layer, body) and, for
/// pieces, all pieces in one drawing (each is shown through its own box).
#[must_use]
pub fn layer_images(show: &ShowOut, li: usize) -> (PageList, Option<PageList>) {
    let layer = &show.layers[li];
    match layer_mode(show, li) {
        LayerMode::Whole => (layer.page.clone(), None),
        LayerMode::Parts => {
            let parts = layer.parts.as_ref().map_or(&[][..], |p| p.pages.as_slice());
            let body = parts.first().cloned().unwrap_or_else(|| layer.page.clone());
            (body, merge(parts.iter().skip(1)))
        }
        LayerMode::Text => {
            let Some(split) = layer.text.as_ref() else {
                return (layer.page.clone(), None);
            };
            (
                split.body.clone(),
                merge(split.pieces.iter().map(|p| &p.page)),
            )
        }
    }
}

/// Several drawings of the same page size as one.
pub fn merge<'a>(lists: impl Iterator<Item = &'a PageList>) -> Option<PageList> {
    let mut out: Option<PageList> = None;
    for l in lists {
        match out.as_mut() {
            Some(o) => o.commands.extend(l.commands.iter().cloned()),
            None => out = Some(l.clone()),
        }
    }
    out
}

/// What a piece is drawn from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PieceImage {
    /// The layer's base image (its drawing, or its body).
    Base(usize),
    /// The layer's pieces image.
    Pieces(usize),
}

/// One thing the show draws for the slide's layers.
#[derive(Debug, Clone, PartialEq)]
pub struct Piece {
    /// Image it comes from.
    pub image: PieceImage,
    /// Its box on the slide (points): the image is shown through it; `None` = the whole image.
    pub rect: Option<[f64; 4]>,
    /// The box effects scale and turn it about (points).
    pub pivot: Option<[f64; 4]>,
    /// How it looks now.
    pub look: Look,
}

/// An effect on the whole of its shape: its own start and end.
fn whole(a: &AnimOut) -> Timed<'_> {
    Timed {
        a,
        start: f64::from(a.start_ms),
        end: f64::from(a.end_ms),
    }
}

/// Every piece of the slide at click `step`, `elapsed` ms into that step's effects
/// (`f64::INFINITY` once they are done), bottom to top.
#[must_use]
pub fn pieces(show: &ShowOut, step: usize, elapsed: f64) -> Vec<Piece> {
    let slide = (f64::from(show.width), f64::from(show.height));
    let mut out = Vec::new();
    for (li, layer) in show.layers.iter().enumerate() {
        let mine: Vec<&AnimOut> = show
            .animations
            .iter()
            .filter(|a| a.layer == Some(li) && a.trigger_shape.is_none())
            .collect();
        let pivot = mine.iter().find_map(|a| a.rect);
        match layer_mode(show, li) {
            LayerMode::Whole => {
                let fx: Vec<Timed<'_>> = mine.iter().copied().map(whole).collect();
                out.push(Piece {
                    image: PieceImage::Base(li),
                    rect: None,
                    pivot,
                    look: fold(&fx, step, elapsed, slide, pivot),
                });
            }
            LayerMode::Parts => {
                let Some(parts) = layer.parts.as_ref() else {
                    continue;
                };
                for (j, rect) in parts.rects.iter().enumerate() {
                    let fx: Vec<Timed<'_>> = mine
                        .iter()
                        .filter(|a| a.parts.is_empty() || a.parts.contains(&j))
                        .copied()
                        .map(whole)
                        .collect();
                    if j == 0 {
                        out.push(Piece {
                            image: PieceImage::Base(li),
                            rect: None,
                            pivot,
                            look: fold(&fx, step, elapsed, slide, pivot),
                        });
                        continue;
                    }
                    let Some(rect) = *rect else { continue };
                    // Effects on the piece alone move it about its own box.
                    let own = fx.iter().all(|t| !t.a.parts.is_empty());
                    let pv = if own { Some(rect) } else { pivot };
                    out.push(Piece {
                        image: PieceImage::Pieces(li),
                        rect: Some(rect),
                        pivot: pv,
                        look: fold(&fx, step, elapsed, slide, pv),
                    });
                }
            }
            LayerMode::Text => {
                let Some(split) = layer.text.as_ref() else {
                    continue;
                };
                let fx: Vec<Timed<'_>> = mine.iter().copied().map(whole).collect();
                out.push(Piece {
                    image: PieceImage::Base(li),
                    rect: None,
                    pivot,
                    look: fold(&fx, step, elapsed, slide, pivot),
                });
                let letters = split.pieces.len().max(1);
                let words = split.words.max(1);
                for piece in &split.pieces {
                    let fx: Vec<Timed<'_>> = mine
                        .iter()
                        .map(|a| {
                            let (n, idx) = match a.text_by.as_deref() {
                                Some("lt") => (letters, piece.letter),
                                Some("wd") => (words, piece.word),
                                _ => return whole(a),
                            };
                            let idx = if a.text_backwards {
                                n.saturating_sub(1 + idx)
                            } else {
                                idx
                            };
                            let step_ms = f64::from(a.text_step_ms.unwrap_or(0));
                            let start = f64::from(a.start_ms) + idx as f64 * step_ms;
                            Timed {
                                a,
                                start,
                                end: (start + f64::from(a.dur_ms)).min(f64::from(a.end_ms)),
                            }
                        })
                        .collect();
                    out.push(Piece {
                        image: PieceImage::Pieces(li),
                        rect: Some(piece.rect),
                        pivot: Some(piece.rect),
                        look: fold(&fx, step, elapsed, slide, Some(piece.rect)),
                    });
                }
            }
        }
    }
    out
}

/// The look of every layer at click `step` (each layer as a whole).
#[cfg(test)]
#[must_use]
pub fn layer_looks(show: &ShowOut, step: usize, elapsed: f64) -> Vec<Look> {
    let slide = (f64::from(show.width), f64::from(show.height));
    (0..show.layers.len())
        .map(|li| {
            let fx: Vec<Timed<'_>> = show
                .animations
                .iter()
                .filter(|a| a.layer == Some(li) && a.trigger_shape.is_none())
                .map(|a| Timed {
                    a,
                    start: f64::from(a.start_ms),
                    end: f64::from(a.end_ms),
                })
                .collect();
            fold(&fx, step, elapsed, slide, None)
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
    /// Draw the whole slide into `dest` turned `degrees` clockwise about its centre (images
    /// that cannot turn draw it straight).
    fn draw_turned(&self, p: &mut dyn Painter, dest: [f32; 4], opacity: f32, degrees: f32) {
        let _ = degrees;
        self.draw(p, dest, WHOLE, opacity);
    }
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

/// The whole of a box, as a part.
pub const WHOLE: [f32; 4] = [0.0, 0.0, 1.0, 1.0];

/// Draw a transition `t` of the way (0..1) from `from` (`None`: black) to `to` in `r`.
#[allow(clippy::too_many_lines)] // one arm per transition family
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
        "r" | "rBlack" => (1.0, 0.0),
        "u" => (0.0, -1.0),
        "d" => (0.0, 1.0),
        "lu" => (-1.0, -1.0),
        "ru" => (1.0, -1.0),
        "ld" => (-1.0, 1.0),
        "rd" => (1.0, 1.0),
        _ => (-1.0, 0.0),
    };
    let tf = t as f32;
    let kind = match tr.kind.as_str() {
        // Effects drawn like a simpler one.
        "random" | "glitter" | "shred" | "honeycomb" | "vortex" => "dissolve",
        "ripple" => "circle",
        other => other,
    };
    if let Some(rev) = Reveal::from_transition(kind, tr.dir.as_deref()) {
        old(p, r, WHOLE, 1.0);
        for part in rev.parts(t) {
            to.draw(p, r, part, 1.0);
        }
        return;
    }
    match kind {
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
        "pull" | "reveal" => {
            let e = ease(t) as f32;
            if dir.ends_with("Black") {
                p.fill_rect(x, y, w, h, black);
                to.draw(p, r, WHOLE, (2.0 * tf - 1.0).clamp(0.0, 1.0));
            } else {
                to.draw(p, r, WHOLE, 1.0);
            }
            p.push_clip(x, y, w, h);
            let fade = if kind == "reveal" { 1.0 - tf } else { 1.0 };
            old(p, [x + ux * w * e, y + uy * h * e, w, h], WHOLE, fade);
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
        "newsflash" => {
            // The new slide spins in twice as it grows from the centre.
            p.fill_rect(x, y, w, h, black);
            old(p, r, WHOLE, 1.0 - tf);
            let e = ease(t) as f32;
            let (zw, zh) = (w * e, h * e);
            p.push_clip(x, y, w, h);
            to.draw_turned(
                p,
                [x + (w - zw) / 2.0, y + (h - zh) / 2.0, zw, zh],
                1.0,
                720.0 * (1.0 - e),
            );
            p.pop_clip();
        }
        "flip" | "switch" | "gallery" | "cube" | "rotate" | "box" | "orbit" | "doors"
        | "window" | "ferris" | "conveyor" | "pan" | "warp" | "flythrough" | "fallOver"
        | "drape" | "curtains" | "airplane" | "origami" | "peelOff" | "pageCurlDouble"
        | "pageCurlSingle" | "crush" | "fracture" | "prestige" | "wind" => {
            // Turning effects: the old slide narrows to its middle line, the new one opens
            // from it (a flat view of the turn).
            p.fill_rect(x, y, w, h, black);
            let e = ease(t) as f32;
            if e < 0.5 {
                let k = 1.0 - 2.0 * e;
                old(p, [x + w * (1.0 - k) / 2.0, y, w * k, h], WHOLE, 1.0);
            } else {
                let k = 2.0 * e - 1.0;
                to.draw(p, [x + w * (1.0 - k) / 2.0, y, w * k, h], WHOLE, 1.0);
            }
        }
        "flash" => {
            // Through a white flash.
            let white = Rgba(255, 255, 255, 255);
            if tf < 0.5 {
                old(p, r, WHOLE, 1.0);
                p.fill_rect(x, y, w, h, Rgba(255, 255, 255, (510.0 * tf) as u8));
            } else {
                to.draw(p, r, WHOLE, 1.0);
                p.fill_rect(
                    x,
                    y,
                    w,
                    h,
                    Rgba(white.0, white.1, white.2, (510.0 * (1.0 - tf)) as u8),
                );
            }
        }
        _ if dir == "black" => {
            p.fill_rect(x, y, w, h, black);
            if tf < 0.5 {
                old(p, r, WHOLE, 1.0 - 2.0 * tf);
            } else {
                to.draw(p, r, WHOLE, 2.0 * tf - 1.0);
            }
        }
        // Fade, Morph without matched shapes, and every other effect played as a fade.
        _ => {
            old(p, r, WHOLE, 1.0);
            to.draw(p, r, WHOLE, tf);
        }
    }
}

/// Draw piece `pc` of a layer image (`key`, full slide, `size` device pixels) into slide
/// rectangle `r` of a slide `slide` points big.
pub fn paint_piece(
    p: &mut dyn Painter,
    key: usize,
    size: (f32, f32),
    r: [f32; 4],
    pc: &Piece,
    slide: (f64, f64),
    opacity: f32,
) {
    let look = &pc.look;
    if !look.visible || look.opacity <= 0.0 {
        return;
    }
    let o = look.opacity * opacity;
    let [x, y, w, h] = r;
    let (ox, oy) = ((look.dx as f32) * w, (look.dy as f32) * h);
    // The box shown: the piece's, else the moved shape's (a little larger for outlines and
    // shadows); a plain whole layer is drawn as it is.
    let pad = if pc.rect.is_some() { 0.004 } else { 0.02 };
    let Some([bx, by, bw, bh]) = pc.rect.or(pc.pivot.filter(|_| !look.plain())) else {
        draw_part(p, key, size, [x + ox, y + oy, w, h], WHOLE, o);
        return;
    };
    let mut part = [
        ((bx / slide.0) as f32 - pad).max(0.0),
        ((by / slide.1) as f32 - pad).max(0.0),
        0.0,
        0.0,
    ];
    part[2] = (((bx + bw) / slide.0) as f32 + pad).min(1.0) - part[0];
    part[3] = (((by + bh) / slide.1) as f32 + pad).min(1.0) - part[1];
    if part[2] <= 0.0 || part[3] <= 0.0 {
        return;
    }
    // Scale and turn about the pivot's centre.
    let [px, py, pw, ph] = pc.pivot.unwrap_or([bx, by, bw, bh]);
    let (cx, cy) = (
        x + ((px + pw / 2.0) / slide.0) as f32 * w + ox,
        y + ((py + ph / 2.0) / slide.1) as f32 * h + oy,
    );
    let (sx, sy) = (look.sx.max(0.0) as f32, look.sy.max(0.0) as f32);
    let subs = match look.reveal {
        Some((rev, q)) => rev.parts(q),
        None => vec![WHOLE],
    };
    for sub in subs {
        let part = [
            part[0] + sub[0] * part[2],
            part[1] + sub[1] * part[3],
            sub[2] * part[2],
            sub[3] * part[3],
        ];
        let src = [
            part[0] * size.0,
            part[1] * size.1,
            part[2] * size.0,
            part[3] * size.1,
        ];
        let dx0 = x + part[0] * w + ox;
        let dy0 = y + part[1] * h + oy;
        let dest = [
            cx + (dx0 - cx) * sx,
            cy + (dy0 - cy) * sy,
            part[2] * w * sx,
            part[3] * h * sy,
        ];
        if look.rot.abs() < 1e-3 {
            p.page_part(key, src, dest, o);
        } else {
            p.page_part_rotated(key, src, dest, o, look.rot as f32);
        }
    }
}

/// Image keys of a Morph's drawings, from a base key: backdrops, matched shapes, gone, new.
pub mod morph_keys {
    /// Previous slide's backdrop.
    pub const FROM_BACKDROP: usize = 0;
    /// This slide's backdrop.
    pub const TO_BACKDROP: usize = 1;
    /// Matched shapes as on the previous slide.
    pub const FROM_PAIRS: usize = 2;
    /// Matched shapes as on this slide.
    pub const TO_PAIRS: usize = 3;
    /// Shapes of the previous slide only.
    pub const GONE: usize = 4;
    /// Shapes of this slide only.
    pub const NEW: usize = 5;
}

/// The drawings a Morph keeps as images (index = [`morph_keys`] offset).
#[must_use]
pub fn morph_images(m: &MorphOut) -> Vec<Option<PageList>> {
    vec![
        Some(m.from_backdrop.clone()),
        Some(m.to_backdrop.clone()),
        merge(m.pairs.iter().map(|p| &p.from_page)),
        merge(m.pairs.iter().map(|p| &p.to_page)),
        merge(m.gone.iter().chain(m.gone_text.iter().map(|g| &g.page))),
        merge(m.new.iter().chain(m.new_text.iter().map(|g| &g.page))),
    ]
}

fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

/// Axis-aligned bounds of a box `[cx, cy, w, h, rot]`.
fn bounds(b: [f64; 5]) -> [f64; 4] {
    let (s, c) = b[4].to_radians().sin_cos();
    let (hw, hh) = (b[2] / 2.0, b[3] / 2.0);
    let ex = (hw * c).abs() + (hh * s).abs();
    let ey = (hw * s).abs() + (hh * c).abs();
    [b[0] - ex, b[1] - ey, 2.0 * ex, 2.0 * ey]
}

/// Draw a Morph `t` of the way (0..1) in slide rectangle `r` from images at `base` +
/// [`morph_keys`] (`size` device pixels for a slide `slide` points big): the backdrops and
/// lone shapes cross-fade, matched shapes move, scale and turn from their old box to the
/// new one (outlines change point by point).
#[allow(clippy::too_many_arguments)]
pub fn paint_morph(
    p: &mut dyn Painter,
    m: &MorphOut,
    base: usize,
    size: (f32, f32),
    r: [f32; 4],
    slide: (f64, f64),
    t: f64,
    has: &dyn Fn(usize) -> bool,
) {
    use morph_keys as k;
    let e = ease(t);
    let ef = e as f32;
    let whole = |p: &mut dyn Painter, off: usize, o: f32| {
        if has(base + off) && o > 0.0 {
            draw_part(p, base + off, size, r, WHOLE, o);
        }
    };
    whole(p, k::FROM_BACKDROP, 1.0);
    whole(p, k::TO_BACKDROP, ef);
    whole(p, k::GONE, 1.0 - ef);
    let (kx, ky) = (f64::from(r[2]) / slide.0, f64::from(r[3]) / slide.1);
    let (ix, iy) = (f64::from(size.0) / slide.0, f64::from(size.1) / slide.1);
    for pair in &m.pairs {
        let now: [f64; 5] = std::array::from_fn(|i| lerp(pair.from[i], pair.to[i], e));
        if let Some(o) = &pair.outline {
            paint_outline(p, o, e, r, (kx, ky));
        }
        // The drawing of each end shown through its bounds, moved and scaled to now.
        for (end, off, opacity) in [
            (pair.from, k::FROM_PAIRS, 1.0f32),
            (pair.to, k::TO_PAIRS, ef),
        ] {
            if !has(base + off) || opacity <= 0.0 {
                continue;
            }
            let b = match pair.clip {
                Some((a, z)) => {
                    if off == k::FROM_PAIRS {
                        a
                    } else {
                        z
                    }
                }
                None => bounds(end),
            };
            let pad = 2.0;
            let b = [b[0] - pad, b[1] - pad, b[2] + 2.0 * pad, b[3] + 2.0 * pad];
            let src = [
                (b[0] * ix) as f32,
                (b[1] * iy) as f32,
                (b[2] * ix) as f32,
                (b[3] * iy) as f32,
            ];
            let (sx, sy) = (
                if end[2] > 0.0 { now[2] / end[2] } else { 1.0 },
                if end[3] > 0.0 { now[3] / end[3] } else { 1.0 },
            );
            // Box centre offset from the shape centre, scaled.
            let (bcx, bcy) = (b[0] + b[2] / 2.0 - end[0], b[1] + b[3] / 2.0 - end[1]);
            let (cx, cy) = (now[0] + bcx * sx, now[1] + bcy * sy);
            let (dw, dh) = (b[2] * sx, b[3] * sy);
            let dest = [
                r[0] + ((cx - dw / 2.0) * kx) as f32,
                r[1] + ((cy - dh / 2.0) * ky) as f32,
                (dw * kx) as f32,
                (dh * ky) as f32,
            ];
            let turn = (now[4] - end[4]) as f32;
            if turn.abs() < 1e-3 {
                p.page_part(base + off, src, dest, opacity);
            } else {
                p.page_part_rotated(base + off, src, dest, opacity, turn);
            }
        }
    }
    whole(p, k::NEW, ef);
}

/// A shape's outline part way between its two forms.
fn paint_outline(
    p: &mut dyn Painter,
    o: &bo_engine::slide_edit::morph_outline::MorphOutline,
    e: f64,
    r: [f32; 4],
    k: (f64, f64),
) {
    let rgba = |c: [u8; 4]| Rgba(c[0], c[1], c[2], c[3]);
    for (a, b) in o.from.iter().zip(&o.to) {
        let mut segs = Vec::new();
        for (ra, rb) in a.rings.iter().zip(&b.rings) {
            for (i, (pa, pb)) in ra.iter().zip(rb).enumerate() {
                let x = r[0] + (lerp(pa[0], pb[0], e) * k.0) as f32;
                let y = r[1] + (lerp(pa[1], pb[1], e) * k.1) as f32;
                segs.push(if i == 0 {
                    PathSeg::M(x, y)
                } else {
                    PathSeg::L(x, y)
                });
            }
            segs.push(PathSeg::Z);
        }
        let mix = |x: Option<[u8; 4]>, y: Option<[u8; 4]>| match (x, y) {
            (Some(x), Some(y)) => Some(std::array::from_fn(|i| {
                lerp(f64::from(x[i]), f64::from(y[i]), e).round() as u8
            })),
            (x, y) => {
                if e < 0.5 {
                    x
                } else {
                    y
                }
            }
        };
        let fill = mix(a.fill, b.fill).map(rgba);
        let stroke = match (a.stroke, b.stroke) {
            (Some((ca, wa)), Some((cb, wb))) => {
                mix(Some(ca), Some(cb)).map(|c| (rgba(c), (lerp(wa, wb, e) * k.0).max(1.0) as f32))
            }
            (x, y) => {
                (if e < 0.5 { x } else { y }).map(|(c, w)| (rgba(c), (w * k.0).max(1.0) as f32))
            }
        };
        p.path(&segs, fill, stroke);
    }
}
