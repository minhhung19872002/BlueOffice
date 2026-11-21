//! BlueReader round 15: the content box of a page for Fit Visible (Ctrl+3).
//!
//! Measured on the reference with four pages (text from 72 to 156 pt, 150 to 380 pt, a stroked
//! rectangle from 50 to 560 pt, the 12-page test document): its Fit Visible zoom is the fit
//! width zoom × page width / content width, where the content width is the INK width of the
//! glyphs (outline boxes, not advances: 82.2 pt for an 83.75 pt advance box) and of painted
//! paths with half their stroke.
use bo_render::{Command, PageList, PathSeg};
use bo_text::{FaceId, FontManager};

type Affine = [f32; 6];

fn concat(local: &Affine, outer: &Affine) -> Affine {
    let [a, b, c, d, e, f] = *local;
    let [oa, ob, oc, od, oe, of] = *outer;
    [
        a * oa + b * oc,
        a * ob + b * od,
        c * oa + d * oc,
        c * ob + d * od,
        e * oa + f * oc + oe,
        e * ob + f * od + of,
    ]
}

#[derive(Default)]
struct Box2(Option<[f32; 4]>);

impl Box2 {
    fn add(&mut self, x: f32, y: f32, m: &Affine) {
        let (px, py) = (m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5]);
        if !(px.is_finite() && py.is_finite()) {
            return;
        }
        let b = self.0.get_or_insert([px, py, px, py]);
        b[0] = b[0].min(px);
        b[1] = b[1].min(py);
        b[2] = b[2].max(px);
        b[3] = b[3].max(py);
    }

    fn rect(&mut self, [l, t, r, b]: [f32; 4], m: &Affine) {
        for (x, y) in [(l, t), (r, t), (l, b), (r, b)] {
            self.add(x, y, m);
        }
    }
}

/// Ink box (left, top, right, bottom in page points) of what page `list` paints, or `None`
/// for an empty page. A fill covering almost the whole page (a page background) is left out.
pub(crate) fn content_box(list: &PageList, fonts: &FontManager) -> Option<[f32; 4]> {
    const IDENTITY: Affine = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];
    let mut m = IDENTITY;
    let mut stack: Vec<Affine> = Vec::new();
    let mut out = Box2::default();
    let page_area = (list.width * list.height).max(1.0);
    for c in &list.commands {
        match c {
            Command::PushTransform { m: local } => {
                stack.push(m);
                m = concat(local, &m);
            }
            Command::PopTransform => m = stack.pop().unwrap_or(IDENTITY),
            Command::FillRect { x, y, w, h, .. } => {
                if (w * h).abs() < page_area * 0.95 {
                    out.rect([*x, *y, x + w, y + h], &m);
                }
            }
            Command::Line {
                x1,
                y1,
                x2,
                y2,
                width,
                ..
            } => {
                let p = width / 2.0;
                out.rect(
                    [
                        x1.min(*x2) - p,
                        y1.min(*y2) - p,
                        x1.max(*x2) + p,
                        y1.max(*y2) + p,
                    ],
                    &m,
                );
            }
            Command::Image { x, y, w, h, .. } => out.rect([*x, *y, x + w, y + h], &m),
            Command::Path { segs, fill, stroke } => {
                let pad = stroke.as_ref().map_or(0.0, |s| s.width / 2.0);
                let mut path = Box2::default();
                for seg in segs {
                    match *seg {
                        PathSeg::M(x, y) | PathSeg::L(x, y) => path.add(x, y, &IDENTITY),
                        PathSeg::Q(a, b, x, y) => {
                            path.add(a, b, &IDENTITY);
                            path.add(x, y, &IDENTITY);
                        }
                        PathSeg::C(a, b, c, d, x, y) => {
                            path.add(a, b, &IDENTITY);
                            path.add(c, d, &IDENTITY);
                            path.add(x, y, &IDENTITY);
                        }
                        PathSeg::Z => {}
                    }
                }
                if let Some([l, t, r, b]) = path.0 {
                    let big = ((r - l) * (b - t)).abs() >= page_area * 0.95;
                    if !(big && stroke.is_none() && fill.is_some()) {
                        out.rect([l - pad, t - pad, r + pad, b + pad], &m);
                    }
                }
            }
            Command::Glyphs {
                face,
                size,
                x,
                y,
                glyphs,
                ..
            } => {
                let id = FaceId(*face);
                let upem = fonts
                    .face(id)
                    .map_or(1000.0, |f| f32::from(f.units_per_em()).max(1.0));
                let k = size / upem;
                for g in glyphs {
                    let e = fonts.glyph_extents(id, g.0);
                    if e.x_max <= e.x_min {
                        continue;
                    }
                    let (gx, gy) = (x + g.1, y + g.2);
                    out.rect(
                        [
                            gx + f32::from(e.x_min) * k,
                            gy - f32::from(e.y_max) * k,
                            gx + f32::from(e.x_max) * k,
                            gy - f32::from(e.y_min) * k,
                        ],
                        &m,
                    );
                }
            }
            _ => {}
        }
    }
    out.0.filter(|b| b[2] > b[0])
}
