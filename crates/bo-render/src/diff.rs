//! Changed area between two display lists of the same page, for incremental repaints.
//!
//! The lists are compared command by command: the common prefix and suffix are unchanged, so
//! only the differing middle commands of both lists can alter pixels. Their bounds are
//! conservative (glyph boxes are generous em boxes, strokes include miters), and any state
//! command (clip, transform, opacity) in the middle makes the whole page dirty.
use crate::{Command, PageList, PathSeg};

/// What changed between two renders of a page.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Change {
    /// Identical lists: nothing to repaint.
    None,
    /// Only this box (left, top, right, bottom in page points) can differ.
    Region([f32; 4]),
    /// Repaint the whole page.
    Full,
}

impl Change {
    /// Device-pixel band (left, top, width, height) to repaint at `scale` pixels per point,
    /// padded for antialiasing and hinting; an empty band for [`Change::None`] and `None` for
    /// [`Change::Full`].
    pub fn device_band(self, scale: f32) -> Option<[u32; 4]> {
        match self {
            Change::None => Some([0, 0, 0, 0]),
            Change::Full => None,
            Change::Region([l, t, r, b]) => {
                // Hinting and ClearType reach a pixel or two past the outline box.
                const PAD: f32 = 3.0;
                let px = |v: f32| (v * scale).max(0.0);
                let (l, t) = (
                    (px(l) - PAD).max(0.0).floor(),
                    (px(t) - PAD).max(0.0).floor(),
                );
                let (r, b) = ((px(r) + PAD).ceil(), (px(b) + PAD).ceil());
                Some([
                    l as u32,
                    t as u32,
                    (r - l).max(0.0) as u32,
                    (b - t).max(0.0) as u32,
                ])
            }
        }
    }
}

type Affine = [f32; 6];
const IDENTITY: Affine = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];

/// `local` applied first, then `outer` (bo-render: x' = a·x + c·y + e, y' = b·x + d·y + f).
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
struct Bounds(Option<[f32; 4]>);

impl Bounds {
    fn add(&mut self, x: f32, y: f32, m: &Affine) {
        let (px, py) = (m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5]);
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

    fn path(&mut self, segs: &[PathSeg], pad: f32, m: &Affine) {
        // Control points bound every Bézier segment.
        for seg in segs {
            let points: &[(f32, f32)] = &match *seg {
                PathSeg::M(x, y) | PathSeg::L(x, y) => [(x, y), (x, y), (x, y)],
                PathSeg::Q(a, b, x, y) => [(a, b), (x, y), (x, y)],
                PathSeg::C(a, b, c, d, x, y) => [(a, b), (c, d), (x, y)],
                PathSeg::Z => continue,
            };
            for &(x, y) in points {
                self.rect([x - pad, y - pad, x + pad, y + pad], m);
            }
        }
    }
}

/// Add the area `command` paints; `false` for state commands, which make the page dirty.
fn paint_bounds(command: &Command, m: &Affine, out: &mut Bounds) -> bool {
    match command {
        Command::FillRect { x, y, w, h, .. } => out.rect([*x, *y, x + w, y + h], m),
        Command::Line {
            x1,
            y1,
            x2,
            y2,
            width,
            ..
        } => {
            let p = width.abs() / 2.0 + 1.0;
            out.rect(
                [
                    x1.min(*x2) - p,
                    y1.min(*y2) - p,
                    x1.max(*x2) + p,
                    y1.max(*y2) + p,
                ],
                m,
            );
        }
        Command::Glyphs {
            x, y, size, glyphs, ..
        } => {
            // Generous em box per glyph: stacked Vietnamese marks above, descenders, overhang.
            let s = size.abs();
            for g in glyphs {
                let (gx, gy) = (x + g.1, y + g.2);
                out.rect([gx - 0.5 * s, gy - 1.6 * s, gx + 1.5 * s, gy + 0.8 * s], m);
            }
        }
        Command::Image {
            x,
            y,
            w,
            h,
            rotation,
            ..
        } => {
            if *rotation == 0.0 {
                out.rect([*x, *y, x + w, y + h], m);
            } else {
                // Any rotation stays inside the circle through the corners.
                let (cx, cy) = (x + w / 2.0, y + h / 2.0);
                let r = (w * w + h * h).sqrt() / 2.0;
                out.rect([cx - r, cy - r, cx + r, cy + r], m);
            }
        }
        Command::Path { segs, stroke, .. } => {
            // Miter joins reach at most limit (10) × half width.
            let pad = stroke.as_ref().map_or(0.0, |s| s.width.abs() * 5.0) + 1.0;
            out.path(segs, pad, m);
        }
        Command::BlurPath {
            segs,
            radius,
            spread,
            ..
        } => out.path(segs, radius.abs() * 2.0 + spread.abs() + 2.0, m),
        Command::PushClip { .. }
        | Command::PushClipPath { .. }
        | Command::PopClip
        | Command::PushTransform { .. }
        | Command::PopTransform
        | Command::PushOpacity { .. }
        | Command::PopOpacity => return false,
    }
    true
}

/// The area of the page that can differ between `old` and `new`.
pub fn changed_region(old: &PageList, new: &PageList) -> Change {
    if old.width != new.width || old.height != new.height {
        return Change::Full;
    }
    let (a, b) = (&old.commands, &new.commands);
    let prefix = a.iter().zip(b).take_while(|(x, y)| x == y).count();
    if prefix == a.len() && prefix == b.len() {
        return Change::None;
    }
    let suffix = a[prefix..]
        .iter()
        .rev()
        .zip(b[prefix..].iter().rev())
        .take_while(|(x, y)| x == y)
        .count();
    // Transform in force after the shared prefix (same in both lists).
    let mut stack = Vec::new();
    let mut m = IDENTITY;
    for command in &a[..prefix] {
        match command {
            Command::PushTransform { m: local } => {
                stack.push(m);
                m = concat(local, &m);
            }
            Command::PopTransform => m = stack.pop().unwrap_or(IDENTITY),
            _ => {}
        }
    }
    let mut bounds = Bounds::default();
    for command in a[prefix..a.len() - suffix]
        .iter()
        .chain(&b[prefix..b.len() - suffix])
    {
        if !paint_bounds(command, &m, &mut bounds) {
            return Change::Full;
        }
    }
    match bounds.0 {
        Some(r) if r.iter().all(|v| v.is_finite()) => Change::Region(r),
        Some(_) => Change::Full,
        // Only empty commands changed (e.g. a glyph run's source text).
        None => Change::None,
    }
}

#[cfg(test)]
mod tests {
    use super::{changed_region, Change};
    use crate::{Command, Glyph, PageList, Rgba};

    fn run(y: f32, text: &str) -> Command {
        Command::Glyphs {
            face: 0,
            size: 10.0,
            x: 50.0,
            y,
            color: Rgba(0, 0, 0, 255),
            text: text.into(),
            glyphs: text
                .chars()
                .enumerate()
                .map(|(i, _)| Glyph(1, i as f32 * 5.0, 0.0, 0))
                .collect(),
        }
    }

    fn page(commands: Vec<Command>) -> PageList {
        PageList {
            width: 600.0,
            height: 800.0,
            commands,
        }
    }

    #[test]
    fn identical_pages_need_no_repaint() {
        let p = page(vec![run(100.0, "ab"), run(120.0, "cd")]);
        assert_eq!(changed_region(&p, &p.clone()), Change::None);
    }

    #[test]
    fn typing_dirties_only_that_line() {
        let old = page(vec![run(100.0, "ab"), run(120.0, "cd"), run(140.0, "ef")]);
        let new = page(vec![run(100.0, "ab"), run(120.0, "cxd"), run(140.0, "ef")]);
        let Change::Region([_, top, _, bottom]) = changed_region(&old, &new) else {
            panic!("expected a region");
        };
        assert!(top > 100.0 && top < 120.0, "{top}");
        assert!(bottom > 120.0 && bottom < 140.0, "{bottom}");
    }

    #[test]
    fn transforms_and_state_changes() {
        let wrap = |c: Command| {
            page(vec![
                Command::PushTransform {
                    m: [1.0, 0.0, 0.0, 1.0, 0.0, 300.0],
                },
                c,
                Command::PopTransform,
            ])
        };
        let Change::Region([_, top, ..]) =
            changed_region(&wrap(run(10.0, "a")), &wrap(run(10.0, "b")))
        else {
            panic!("expected a region");
        };
        assert!(top > 290.0, "{top}");
        let clipped = page(vec![Command::PushClip {
            x: 0.0,
            y: 0.0,
            w: 1.0,
            h: 1.0,
        }]);
        assert_eq!(changed_region(&page(vec![]), &clipped), Change::Full);
        let mut taller = page(vec![]);
        taller.height = 900.0;
        assert_eq!(changed_region(&page(vec![]), &taller), Change::Full);
    }
}
