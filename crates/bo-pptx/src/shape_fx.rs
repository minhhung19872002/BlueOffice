//! Reflection and soft edges of slide shapes (CORE-SHAPE-002), drawn with plain display-list
//! commands (clip, opacity, transform) so the canvas, PNG and PDF backends need nothing new.
//! Geometry and ramps: [`bo_drawingml::effects`].

use bo_drawingml::effects::{soft_edge_radius, Reflection, REFLECTION_BANDS};
use bo_model::slide::Effect;
use bo_render::{Command, PathSeg};

/// §20.1.8.50 reflection in an effect list.
pub(crate) fn reflection_in(list: &[Effect]) -> Option<Reflection> {
    list.iter().find_map(|e| match e {
        Effect::Other(x) if x.name == "a:reflection" => Some(Reflection::parse(x)),
        _ => None,
    })
}

/// §20.1.8.53 softEdge radius (points) in an effect list.
pub(crate) fn soft_edge_in(list: &[Effect]) -> Option<f64> {
    list.iter()
        .find_map(|e| match e {
            Effect::Other(x) if x.name == "a:softEdge" => Some(soft_edge_radius(x)),
            _ => None,
        })
        .filter(|r| *r > 0.0)
}

/// Box `[x0, y0, x1, y1]` of points.
pub(crate) fn box_of(pts: impl IntoIterator<Item = (f64, f64)>) -> Option<[f64; 4]> {
    let mut b: Option<[f64; 4]> = None;
    for (x, y) in pts {
        let r = b.get_or_insert([x, y, x, y]);
        r[0] = r[0].min(x);
        r[1] = r[1].min(y);
        r[2] = r[2].max(x);
        r[3] = r[3].max(y);
    }
    b
}

/// Draw `content` (page coordinates) again as the reflection `r` of a shape whose page box
/// is `bbox`: one copy per fade band, clipped to the band and faded to its opacity.
pub(crate) fn draw_reflection(
    r: &Reflection,
    bbox: [f64; 4],
    content: &[Command],
    out: &mut Vec<Command>,
) {
    if content.is_empty() {
        return;
    }
    let m = r.transform(bbox);
    let corners = [
        (bbox[0], bbox[1]),
        (bbox[2], bbox[1]),
        (bbox[2], bbox[3]),
        (bbox[0], bbox[3]),
    ];
    let Some(rb) = box_of(
        corners
            .iter()
            .map(|&(x, y)| (m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5])),
    ) else {
        return;
    };
    // Room for outlines, shadows and glows beyond the box across the fade.
    let pad = 24.0 + r.blur * 2.0;
    #[allow(clippy::cast_possible_truncation)]
    let f = |v: f64| v as f32;
    for (poly, alpha) in r.bands(rb, REFLECTION_BANDS, pad) {
        let mut segs: Vec<PathSeg> = poly.iter().map(|&(x, y)| PathSeg::L(f(x), f(y))).collect();
        if let Some(PathSeg::L(x, y)) = segs.first().copied() {
            segs[0] = PathSeg::M(x, y);
        }
        segs.push(PathSeg::Z);
        out.push(Command::PushClipPath { segs });
        out.push(Command::PushOpacity { alpha: f(alpha) });
        out.push(Command::PushTransform { m: m.map(f) });
        out.extend(content.iter().cloned());
        out.push(Command::PopTransform);
        out.push(Command::PopOpacity);
        out.push(Command::PopClip);
    }
}
