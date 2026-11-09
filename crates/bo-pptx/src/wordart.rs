//! WordArt in shapes (CORE-SHAPE-002): text warps (§20.1.9.19 `bodyPr/prstTxWarp`) and
//! glyph fills / outlines beyond a plain colour (§21.1.2.3.9 rPr `a:ln`, `a:gradFill`,
//! `a:pattFill`, `a:blipFill`), applied to the commands [`crate::text_layout`] drew in the
//! text box's local coordinates.
//!
//! * A warp turns every glyph run, underline and strike line into a path, measures the
//!   ink box of all of them and maps it through [`bo_drawingml::textwarp::TextWarp`]
//!   evaluated for the text box (shape text rectangle minus the bodyPr insets).
//! * A run with a WordArt fill is painted over the box of all such runs (one gradient
//!   across the text, BlueOffice's choice), its outline stroked on top.

use crate::text_layout::{RunFx, TextFx};
use bo_drawingml::geom::Seg;
use bo_drawingml::textwarp::TextWarp;
use bo_model::slide::{BodyProps, Guide};
use bo_render::{Command, Paint, PathSeg};
use bo_text::{FaceId, FontManager};

const EMU_PT: f64 = 12_700.0;

/// The preset warp of a body (`prst` and its avLst guides); `None` without one or for
/// `textNoShape`.
pub(crate) fn warp_of(bp: &BodyProps) -> Option<(String, Vec<Guide>)> {
    let el = bp
        .extra
        .iter()
        .map(|e| &e.xml)
        .find(|x| x.name == "a:prstTxWarp")?;
    let prst = el.attr("prst")?;
    if prst == "textNoShape" {
        return None;
    }
    // §20.1.9.5 avLst / §20.1.9.11 gd.
    let av = el
        .child("a:avLst")
        .map(|l| {
            l.elements()
                .filter(|g| g.name == "a:gd")
                .filter_map(|g| {
                    Some(Guide {
                        name: g.attr("name")?.to_owned(),
                        fmla: g.attr("fmla")?.to_owned(),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    Some((prst.to_owned(), av))
}

/// The text box `[x0, y0, x1, y1]`: rectangle `[l, t, r, b]` minus the §21.1.2.1.1 insets
/// (defaults 7.2 pt left/right, 3.6 pt top/bottom).
pub(crate) fn inner_box(rect: [f64; 4], bp: &BodyProps) -> [f64; 4] {
    let ins = |v: Option<i64>, d: f64| v.map_or(d, |e| e as f64 / EMU_PT);
    let x0 = rect[0] + ins(bp.l_ins, 7.2);
    let y0 = rect[1] + ins(bp.t_ins, 3.6);
    let x1 = (rect[2] - ins(bp.r_ins, 7.2)).max(x0 + 1.0);
    let y1 = (rect[3] - ins(bp.b_ins, 3.6)).max(y0 + 1.0);
    [x0, y0, x1, y1]
}

fn to_seg(s: &PathSeg) -> Seg {
    let f = f64::from;
    match *s {
        PathSeg::M(x, y) => Seg::Move(f(x), f(y)),
        PathSeg::L(x, y) => Seg::Line(f(x), f(y)),
        PathSeg::Q(a, b, x, y) => Seg::Quad(f(a), f(b), f(x), f(y)),
        PathSeg::C(a, b, c, d, x, y) => Seg::Cubic(f(a), f(b), f(c), f(d), f(x), f(y)),
        PathSeg::Z => Seg::Close,
    }
}

fn to_path_seg(s: &Seg) -> PathSeg {
    #[allow(clippy::cast_possible_truncation)]
    let f = |v: f64| v as f32;
    match *s {
        Seg::Move(x, y) => PathSeg::M(f(x), f(y)),
        Seg::Line(x, y) => PathSeg::L(f(x), f(y)),
        Seg::Quad(a, b, x, y) => PathSeg::Q(f(a), f(b), f(x), f(y)),
        Seg::Cubic(a, b, c, d, x, y) => PathSeg::C(f(a), f(b), f(c), f(d), f(x), f(y)),
        Seg::Close => PathSeg::Z,
    }
}

/// Box `[x0, y0, x1, y1]` of path points (control points included).
pub(crate) fn bounds(segs: &[PathSeg], acc: &mut Option<[f64; 4]>) {
    let mut add = |x: f32, y: f32| {
        let (x, y) = (f64::from(x), f64::from(y));
        let b = acc.get_or_insert([x, y, x, y]);
        b[0] = b[0].min(x);
        b[1] = b[1].min(y);
        b[2] = b[2].max(x);
        b[3] = b[3].max(y);
    };
    for s in segs {
        match *s {
            PathSeg::M(x, y) | PathSeg::L(x, y) => add(x, y),
            PathSeg::Q(a, b, x, y) => {
                add(a, b);
                add(x, y);
            }
            PathSeg::C(a, b, c, d, x, y) => {
                add(a, b);
                add(c, d);
                add(x, y);
            }
            PathSeg::Z => {}
        }
    }
}

/// Turn glyph runs and text lines into paths (same index), so a warp can bend them.
fn outline_text(cmds: &mut [Command], fonts: &FontManager) {
    for c in cmds.iter_mut() {
        let path = match c {
            Command::Glyphs {
                face,
                size,
                x,
                y,
                color,
                glyphs,
                ..
            } => Command::Path {
                segs: crate::text_layout::glyph_path(
                    fonts,
                    FaceId(*face),
                    f64::from(*size),
                    (f64::from(*x), f64::from(*y)),
                    glyphs,
                ),
                fill: Some(Paint::Solid { color: *color }),
                stroke: None,
            },
            Command::Line {
                x1,
                y1,
                x2,
                y2,
                width,
                color,
                ..
            } => {
                // A text line (underline / strike) as a thin quadrilateral.
                let (dx, dy) = (*x2 - *x1, *y2 - *y1);
                let n = dx.hypot(dy).max(1e-6);
                let (ox, oy) = (-dy / n * *width / 2.0, dx / n * *width / 2.0);
                Command::Path {
                    segs: vec![
                        PathSeg::M(*x1 + ox, *y1 + oy),
                        PathSeg::L(*x2 + ox, *y2 + oy),
                        PathSeg::L(*x2 - ox, *y2 - oy),
                        PathSeg::L(*x1 - ox, *y1 - oy),
                        PathSeg::Z,
                    ],
                    fill: Some(Paint::Solid { color: *color }),
                    stroke: None,
                }
            }
            _ => continue,
        };
        *c = path;
    }
}

/// Warp the text commands `cmds` (local coordinates) with preset `warp` laid over `bx`
/// (`[x0, y0, x1, y1]`). Commands keep their indices. Returns `false` when the preset
/// does not evaluate (the text stays flat).
pub(crate) fn apply_warp(
    cmds: &mut [Command],
    fonts: &FontManager,
    bx: [f64; 4],
    (name, av): &(String, Vec<Guide>),
) -> bool {
    let Some(w) = TextWarp::new(name, av, bx[2] - bx[0], bx[3] - bx[1]) else {
        return false;
    };
    outline_text(cmds, fonts);
    let mut ink = None;
    for c in cmds.iter() {
        if let Command::Path { segs, .. } = c {
            bounds(segs, &mut ink);
        }
    }
    let Some(src) = ink else { return false };
    for c in cmds.iter_mut() {
        if let Command::Path { segs, .. } = c {
            let local: Vec<Seg> = segs.iter().map(to_seg).collect();
            *segs = w
                .warp_segs(&local, src)
                .iter()
                .map(|s| {
                    to_path_seg(&match *s {
                        Seg::Move(x, y) => Seg::Move(x + bx[0], y + bx[1]),
                        Seg::Line(x, y) => Seg::Line(x + bx[0], y + bx[1]),
                        other => other,
                    })
                })
                .collect();
        }
    }
    true
}

/// Paint the WordArt runs: `fill(fx, segs, ink box, out)` paints a non-solid fill over the
/// box of all WordArt runs, `stroke(line)` makes the outline stroke.
pub(crate) fn finish(
    cmds: Vec<Command>,
    fx: &[TextFx],
    mut fill: impl FnMut(&RunFx, &[PathSeg], [f64; 4], &mut Vec<Command>),
    stroke: impl Fn(&bo_model::slide::Line) -> Option<bo_render::Stroke>,
) -> Vec<Command> {
    if fx.is_empty() {
        return cmds;
    }
    let mut ink = None;
    for f in fx {
        if let Some(Command::Path { segs, .. }) = cmds.get(f.index) {
            bounds(segs, &mut ink);
        }
    }
    let ink = ink.unwrap_or([0.0, 0.0, 1.0, 1.0]);
    let mut out = Vec::with_capacity(cmds.len() + fx.len());
    let mut next = fx.iter().peekable();
    for (i, c) in cmds.into_iter().enumerate() {
        let Some(f) = next.next_if(|f| f.index == i) else {
            out.push(c);
            continue;
        };
        let Command::Path {
            segs, fill: solid, ..
        } = c
        else {
            out.push(c);
            continue;
        };
        if f.fx.fill.is_some() {
            fill(&f.fx, &segs, ink, &mut out);
        } else if solid.is_some() {
            out.push(Command::Path {
                segs: segs.clone(),
                fill: solid,
                stroke: None,
            });
        }
        if let Some(s) = f.fx.line.as_ref().and_then(&stroke) {
            out.push(Command::Path {
                segs,
                fill: None,
                stroke: Some(s),
            });
        }
    }
    out
}
