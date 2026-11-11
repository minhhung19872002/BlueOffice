//! Morph ([MS-PPTX] `p159:morph` transition): the shapes of the previous slide that are found again on
//! the next one move, resize, turn and change colour into their new state; the others fade out or in.
//!
//! Shapes are matched, top-level shape against top-level shape, in this order:
//! 1. names starting with `!!` that are equal (the author asks for the pair);
//! 2. equal names and the same kind (a duplicated slide keeps its names);
//! 3. the same kind and the same text (non-empty);
//! 4. the same kind and preset geometry (or the same picture), the nearest first.
//!
//! [`interpolate`] gives the state at a time of the transition (what the slide show plays).

use super::geom::{place_slide, BoxR};
use bo_model::slide::{ColorKind, Fill, Geometry, Presentation, ShapeNode};
use serde::Serialize;

/// What Morph knows of a top-level shape.
#[derive(Debug, Clone, PartialEq)]
pub struct ShapeState {
    /// Index among the slide's top-level shapes.
    pub index: usize,
    /// `sp` | `pic` | `grpSp` | `cxnSp` | `graphicFrame` | `other`.
    pub kind: &'static str,
    /// `cNvPr@name`.
    pub name: String,
    /// Text of the shape (shapes only).
    pub text: String,
    /// Preset geometry name, or the picture's relationship target for pictures.
    pub look: String,
    /// Visual box (centre, size, rotation).
    pub b: BoxR,
    /// Solid sRGB fill, if any.
    pub fill: Option<[u8; 3]>,
}

fn kind_of(n: &ShapeNode) -> &'static str {
    match n {
        ShapeNode::Shape(_) => "sp",
        ShapeNode::Picture(_) => "pic",
        ShapeNode::Group(_) => "grpSp",
        ShapeNode::Connector(_) => "cxnSp",
        ShapeNode::Frame(_) => "graphicFrame",
        ShapeNode::Other(_) => "other",
    }
}

/// The top-level shapes of a slide.
pub fn states(pres: &Presentation, slide: usize) -> Vec<ShapeState> {
    let Some(s) = pres.slides.get(slide) else {
        return Vec::new();
    };
    let placed = place_slide(pres, slide);
    s.data
        .shapes
        .iter()
        .enumerate()
        .filter_map(|(index, n)| {
            let p = placed.iter().find(|p| p.path.first() == Some(&index))?;
            let (text, look, fill) = match n {
                ShapeNode::Shape(sp) => (
                    sp.text
                        .as_ref()
                        .map(super::text::body_text)
                        .unwrap_or_default(),
                    match &sp.props.geometry {
                        Some(Geometry::Preset { name, .. }) => name.clone(),
                        Some(Geometry::Custom(_)) => "custom".into(),
                        None => String::new(),
                    },
                    match &sp.props.fill {
                        Some(Fill::Solid(c)) => match c.kind {
                            ColorKind::Srgb(v) => Some([(v >> 16) as u8, (v >> 8) as u8, v as u8]),
                            _ => None,
                        },
                        _ => None,
                    },
                ),
                // The same picture: the same image part (relationship ids differ between slides).
                ShapeNode::Picture(pic) => (
                    String::new(),
                    pic.blip
                        .embed
                        .as_ref()
                        .and_then(|id| s.info.rels.iter().find(|r| &r.id == id))
                        .map(|r| r.target.clone())
                        .unwrap_or_default(),
                    None,
                ),
                _ => (String::new(), String::new(), None),
            };
            Some(ShapeState {
                index,
                kind: kind_of(n),
                name: n.nv().map(|nv| nv.name.clone()).unwrap_or_default(),
                text,
                look,
                b: p.b,
                fill,
            })
        })
        .collect()
}

/// Pairs (index in `from`, index in `to`) of matched shapes.
#[must_use]
pub fn match_shapes(from: &[ShapeState], to: &[ShapeState]) -> Vec<(usize, usize)> {
    let mut used_a = vec![false; from.len()];
    let mut used_b = vec![false; to.len()];
    let mut out = Vec::new();
    let mut pass = |ok: &dyn Fn(&ShapeState, &ShapeState) -> bool,
                    nearest: bool,
                    out: &mut Vec<(usize, usize)>| {
        for (i, a) in from.iter().enumerate() {
            if used_a[i] {
                continue;
            }
            let best = to
                .iter()
                .enumerate()
                .filter(|(j, b)| !used_b[*j] && ok(a, b))
                .min_by(|(_, x), (_, y)| {
                    let d = |b: &ShapeState| {
                        if nearest {
                            (a.b.cx - b.b.cx).hypot(a.b.cy - b.b.cy)
                        } else {
                            0.0
                        }
                    };
                    d(x).total_cmp(&d(y))
                })
                .map(|(j, _)| j);
            if let Some(j) = best {
                used_a[i] = true;
                used_b[j] = true;
                out.push((i, j));
            }
        }
    };
    pass(
        &|a, b| a.name.starts_with("!!") && a.name == b.name,
        false,
        &mut out,
    );
    pass(
        &|a, b| !a.name.is_empty() && a.name == b.name && a.kind == b.kind,
        false,
        &mut out,
    );
    pass(
        &|a, b| a.kind == b.kind && !a.text.trim().is_empty() && a.text == b.text,
        true,
        &mut out,
    );
    pass(
        &|a, b| a.kind == b.kind && !a.look.is_empty() && a.look == b.look && a.kind != "other",
        true,
        &mut out,
    );
    out.sort_unstable();
    out
}

/// The shortest turn from `a` to `b` degrees.
fn turn(a: f64, b: f64) -> f64 {
    let d = (b - a).rem_euclid(360.0);
    if d > 180.0 {
        d - 360.0
    } else {
        d
    }
}

/// The state between `a` (t = 0) and `b` (t = 1): box and fill (linear; rotation the shortest way).
#[must_use]
pub fn interpolate(a: &ShapeState, b: &ShapeState, t: f64) -> (BoxR, Option<[u8; 3]>) {
    let lerp = |x: f64, y: f64| x + (y - x) * t;
    let r = BoxR {
        cx: lerp(a.b.cx, b.b.cx),
        cy: lerp(a.b.cy, b.b.cy),
        w: lerp(a.b.w, b.b.w),
        h: lerp(a.b.h, b.b.h),
        rot: a.b.rot + turn(a.b.rot, b.b.rot) * t,
    };
    let fill = match (a.fill, b.fill) {
        (Some(x), Some(y)) => {
            Some([0, 1, 2].map(|k| lerp(f64::from(x[k]), f64::from(y[k])).round() as u8))
        }
        (x, y) => {
            if t < 0.5 {
                x
            } else {
                y
            }
        }
    };
    (r, fill)
}

/// A matched pair as the slide show plays it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MorphPair {
    /// Box on the previous slide `[cx, cy, w, h, rot]` (points, degrees).
    pub from: [f64; 5],
    /// Box on this slide.
    pub to: [f64; 5],
    /// The shape drawn as on the previous slide.
    pub from_page: bo_render::PageList,
    /// The shape drawn as on this slide.
    pub to_page: bo_render::PageList,
    /// For a word or letter: where the two drawings are `[x, y, w, h]` (from, to), points.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clip: Option<([f64; 4], [f64; 4])>,
    /// The shape's outlines changing point by point (`from_page` / `to_page` then hold its text
    /// only); `None` = the two drawings cross-fade.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outline: Option<super::morph_outline::MorphOutline>,
}

/// Everything the slide show needs to morph into a slide.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MorphOut {
    /// Background and master / layout shapes of the previous slide.
    pub from_backdrop: bo_render::PageList,
    /// Background and master / layout shapes of this slide.
    pub to_backdrop: bo_render::PageList,
    /// Shapes found on both slides.
    pub pairs: Vec<MorphPair>,
    /// Shapes of the previous slide only (they fade out).
    pub gone: Vec<bo_render::PageList>,
    /// Shapes of this slide only (they fade in).
    pub new: Vec<bo_render::PageList>,
    /// Words / letters of the previous slide's matched shapes not found again (by word / by
    /// character): they fade out.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub gone_text: Vec<MorphPiece>,
    /// Words / letters new on this slide: they fade in.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub new_text: Vec<MorphPiece>,
    /// `byObject` | `byWord` | `byChar` ([MS-PPTX] morph@option).
    pub option: String,
}

/// Words or letters of two matched shapes ([`super::anim_text::grouped`]): those with the same
/// text, in reading order, move from their box to the new one; the others fade out (`gone`) or
/// in (`new`).
pub fn text_pairs(
    from: &[(String, bo_render::PageList, [f64; 4])],
    to: &[(String, bo_render::PageList, [f64; 4])],
    gone: &mut Vec<MorphPiece>,
    new: &mut Vec<MorphPiece>,
) -> Vec<MorphPair> {
    let ta: Vec<String> = from.iter().map(|p| p.0.clone()).collect();
    let tb: Vec<String> = to.iter().map(|p| p.0.clone()).collect();
    let pairs = super::anim_text::pair_by_text(&ta, &tb);
    let centre = |r: &[f64; 4]| [r[0] + r[2] / 2.0, r[1] + r[3] / 2.0, r[2], r[3], 0.0];
    let mut out = Vec::new();
    for &(i, j) in &pairs {
        out.push(MorphPair {
            from: centre(&from[i].2),
            to: centre(&to[j].2),
            from_page: from[i].1.clone(),
            to_page: to[j].1.clone(),
            clip: Some((from[i].2, to[j].2)),
            outline: None,
        });
    }
    for (i, p) in from.iter().enumerate() {
        if !pairs.iter().any(|q| q.0 == i) {
            gone.push(MorphPiece {
                page: p.1.clone(),
                rect: p.2,
            });
        }
    }
    for (j, p) in to.iter().enumerate() {
        if !pairs.iter().any(|q| q.1 == j) {
            new.push(MorphPiece {
                page: p.1.clone(),
                rect: p.2,
            });
        }
    }
    out
}

/// A word or letter fading out or in.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MorphPiece {
    /// The piece drawn alone.
    pub page: bo_render::PageList,
    /// Where it is drawn `[x, y, w, h]`, points.
    pub rect: [f64; 4],
}

/// The box as `[cx, cy, w, h, rot]`.
#[must_use]
pub fn box5(b: &BoxR) -> [f64; 5] {
    [b.cx, b.cy, b.w, b.h, b.rot]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn st(
        index: usize,
        kind: &'static str,
        name: &str,
        text: &str,
        look: &str,
        b: (f64, f64, f64, f64, f64),
    ) -> ShapeState {
        ShapeState {
            index,
            kind,
            name: name.into(),
            text: text.into(),
            look: look.into(),
            b: BoxR {
                cx: b.0,
                cy: b.1,
                w: b.2,
                h: b.3,
                rot: b.4,
            },
            fill: None,
        }
    }

    #[test]
    fn shapes_match_by_forced_name_name_text_and_look() {
        let from = vec![
            st(
                0,
                "sp",
                "!!logo",
                "",
                "ellipse",
                (10.0, 10.0, 5.0, 5.0, 0.0),
            ),
            st(
                1,
                "sp",
                "Title 1",
                "Xin chào",
                "rect",
                (100.0, 50.0, 300.0, 40.0, 0.0),
            ),
            st(
                2,
                "sp",
                "Box A",
                "Doanh thu",
                "rect",
                (50.0, 200.0, 80.0, 30.0, 0.0),
            ),
            st(
                3,
                "sp",
                "Star",
                "",
                "star5",
                (300.0, 300.0, 40.0, 40.0, 0.0),
            ),
            st(
                4,
                "pic",
                "Picture 9",
                "",
                "media/image1.png",
                (500.0, 300.0, 40.0, 40.0, 0.0),
            ),
            st(5, "sp", "Lone", "", "triangle", (0.0, 0.0, 1.0, 1.0, 0.0)),
        ];
        let to = vec![
            st(
                0,
                "sp",
                "Box B",
                "Doanh thu",
                "roundRect",
                (400.0, 200.0, 120.0, 60.0, 30.0),
            ),
            st(
                1,
                "sp",
                "!!logo",
                "",
                "rect",
                (900.0, 10.0, 50.0, 50.0, 0.0),
            ),
            st(
                2,
                "sp",
                "Title 1",
                "Tạm biệt",
                "rect",
                (100.0, 60.0, 300.0, 40.0, 0.0),
            ),
            // Two stars: the nearer one is the pair.
            st(
                3,
                "sp",
                "Star far",
                "",
                "star5",
                (900.0, 900.0, 40.0, 40.0, 0.0),
            ),
            st(
                4,
                "sp",
                "Star near",
                "",
                "star5",
                (310.0, 290.0, 40.0, 40.0, 0.0),
            ),
            st(
                5,
                "pic",
                "Picture 2",
                "",
                "media/image1.png",
                (520.0, 280.0, 60.0, 60.0, 0.0),
            ),
            st(6, "sp", "New", "", "triangle2", (0.0, 0.0, 1.0, 1.0, 0.0)),
        ];
        assert_eq!(
            match_shapes(&from, &to),
            vec![(0, 1), (1, 2), (2, 0), (3, 4), (4, 5)]
        );
    }

    // source: linear interpolation of the box; the rotation turns the short way (350° → 10° is +20°).
    #[test]
    fn states_interpolate() {
        let mut a = st(0, "sp", "a", "", "rect", (0.0, 0.0, 100.0, 50.0, 350.0));
        let mut b = st(0, "sp", "a", "", "rect", (200.0, 100.0, 300.0, 150.0, 10.0));
        a.fill = Some([255, 0, 0]);
        b.fill = Some([0, 0, 255]);
        let (r, f) = interpolate(&a, &b, 0.5);
        assert_eq!((r.cx, r.cy, r.w, r.h), (100.0, 50.0, 200.0, 100.0));
        assert!((r.rot - 360.0).abs() < 1e-9, "{}", r.rot);
        assert_eq!(f, Some([128, 0, 128]));
        let (r, _) = interpolate(&a, &b, 1.0);
        assert!((r.rot.rem_euclid(360.0) - 10.0).abs() < 1e-9);
        let (r, _) = interpolate(&a, &b, 0.0);
        assert_eq!((r.cx, r.w, r.rot), (0.0, 100.0, 350.0));
        assert_eq!(turn(10.0, 350.0), -20.0);
    }
}
