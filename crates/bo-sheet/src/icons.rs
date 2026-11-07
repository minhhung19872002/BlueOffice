//! Vector icons for conditional-format icon sets (ECMA-376 Part 1, §18.18.42
//! `ST_IconSetType`). The shapes and colours are BlueSheet's own simple
//! designs: arrows, circles, flags, bars, quarters, stars and triangles.

use bo_render::{Command, Paint, PathSeg, Rgba, Stroke};

const RED: Rgba = Rgba(0xD6, 0x45, 0x45, 255);
const ORANGE: Rgba = Rgba(0xF2, 0x8C, 0x28, 255);
const YELLOW: Rgba = Rgba(0xE0, 0xB4, 0x1F, 255);
const LIME: Rgba = Rgba(0x8B, 0xC3, 0x4A, 255);
const GREEN: Rgba = Rgba(0x2F, 0xA3, 0x6B, 255);
const GRAY: Rgba = Rgba(0x8A, 0x90, 0x99, 255);
const LIGHT: Rgba = Rgba(0xD9, 0xDD, 0xE2, 255);
const BLUE: Rgba = Rgba(0x2E, 0x6D, 0xB4, 255);

/// Colour of position `i` of `n` on a bad → good ramp.
fn ramp(i: usize, n: usize) -> Rgba {
    let ramp: &[Rgba] = match n {
        3 => &[RED, YELLOW, GREEN],
        4 => &[RED, ORANGE, LIME, GREEN],
        _ => &[RED, ORANGE, YELLOW, LIME, GREEN],
    };
    ramp[i.min(ramp.len() - 1)]
}

fn fill(segs: Vec<PathSeg>, c: Rgba) -> Command {
    Command::Path {
        segs,
        fill: Some(Paint::Solid { color: c }),
        stroke: None,
    }
}

fn p(x: f64, y: f64) -> (f32, f32) {
    (x as f32, y as f32)
}

fn poly(pts: &[(f64, f64)]) -> Vec<PathSeg> {
    let mut v: Vec<PathSeg> = pts
        .iter()
        .enumerate()
        .map(|(i, &(x, y))| {
            let (x, y) = p(x, y);
            if i == 0 {
                PathSeg::M(x, y)
            } else {
                PathSeg::L(x, y)
            }
        })
        .collect();
    v.push(PathSeg::Z);
    v
}

/// Circle as four cubic Béziers.
fn circle(cx: f64, cy: f64, r: f64) -> Vec<PathSeg> {
    let k = 0.552_284_75 * r;
    let (cx, cy, r, k) = (cx as f32, cy as f32, r as f32, k as f32);
    vec![
        PathSeg::M(cx + r, cy),
        PathSeg::C(cx + r, cy + k, cx + k, cy + r, cx, cy + r),
        PathSeg::C(cx - k, cy + r, cx - r, cy + k, cx - r, cy),
        PathSeg::C(cx - r, cy - k, cx - k, cy - r, cx, cy - r),
        PathSeg::C(cx + k, cy - r, cx + r, cy - k, cx + r, cy),
        PathSeg::Z,
    ]
}

/// Arrow pointing at `angle` degrees (0 = up, 90 = right, 180 = down).
fn arrow(cx: f64, cy: f64, s: f64, angle: f64, c: Rgba) -> Command {
    let h = s / 2.0;
    // Up arrow, then rotated.
    let pts = [
        (0.0, -h),
        (h * 0.9, 0.0),
        (h * 0.35, 0.0),
        (h * 0.35, h),
        (-h * 0.35, h),
        (-h * 0.35, 0.0),
        (-h * 0.9, 0.0),
    ];
    let (sin, cos) = angle.to_radians().sin_cos();
    let rot: Vec<(f64, f64)> = pts
        .iter()
        .map(|&(x, y)| (cx + x * cos - y * sin, cy + x * sin + y * cos))
        .collect();
    fill(poly(&rot), c)
}

fn star(cx: f64, cy: f64, r: f64) -> Vec<PathSeg> {
    let pts: Vec<(f64, f64)> = (0..10)
        .map(|i| {
            let a = std::f64::consts::PI / 5.0 * f64::from(i) - std::f64::consts::FRAC_PI_2;
            let rr = if i % 2 == 0 { r } else { r * 0.45 };
            (cx + rr * a.cos(), cy + rr * a.sin())
        })
        .collect();
    poly(&pts)
}

/// Commands drawing icon `index` (0 = lowest) of an icon set of `count`
/// icons, in the `s × s` square at (`x`, `y`).
pub fn icon(set: &str, index: usize, count: usize, x: f64, y: f64, s: f64) -> Vec<Command> {
    let (cx, cy, r) = (x + s / 2.0, y + s / 2.0, s / 2.0);
    let n = count.max(2);
    let gray = set.contains("Gray");
    let mut out = Vec::new();
    if set.contains("Arrows") {
        // Lowest points down, highest up, the others in between.
        let angle = 180.0 - 180.0 * index as f64 / (n - 1) as f64;
        let c = if gray { GRAY } else { ramp(index, n) };
        out.push(arrow(cx, cy, s * 0.95, angle, c));
    } else if set.contains("Triangles") {
        let c = ramp(index, 3.min(n));
        let h = s * 0.42;
        let pts = match index {
            0 => vec![
                (cx - h, cy - h * 0.6),
                (cx + h, cy - h * 0.6),
                (cx, cy + h * 0.8),
            ],
            i if i + 1 >= n => vec![
                (cx - h, cy + h * 0.6),
                (cx + h, cy + h * 0.6),
                (cx, cy - h * 0.8),
            ],
            _ => vec![
                (cx - h, cy - h * 0.25),
                (cx + h, cy - h * 0.25),
                (cx + h, cy + h * 0.25),
                (cx - h, cy + h * 0.25),
            ],
        };
        out.push(fill(poly(&pts), c));
    } else if set.contains("Flags") {
        let c = ramp(index, n);
        out.push(fill(
            poly(&[
                (x + s * 0.2, y + s * 0.1),
                (x + s * 0.3, y + s * 0.1),
                (x + s * 0.3, y + s * 0.95),
                (x + s * 0.2, y + s * 0.95),
            ]),
            GRAY,
        ));
        out.push(fill(
            poly(&[
                (x + s * 0.3, y + s * 0.1),
                (x + s * 0.9, y + s * 0.32),
                (x + s * 0.3, y + s * 0.55),
            ]),
            c,
        ));
    } else if set.contains("Rating") || set.contains("Boxes") {
        // `count` bars; the first `index + 1` are filled (index 0 = one bar).
        let bars = if set.contains("Boxes") {
            4
        } else {
            n.saturating_sub(1).max(3)
        };
        let filled = index * bars / (n - 1);
        let w = s / (bars as f64 * 1.5);
        for b in 0..bars {
            let hgt = s * (0.3 + 0.7 * (b + 1) as f64 / bars as f64);
            let bx = x + b as f64 * w * 1.5;
            let by = y + s - hgt;
            let c = if b < filled.max(usize::from(index > 0)) {
                BLUE
            } else {
                LIGHT
            };
            out.push(fill(
                poly(&[(bx, by), (bx + w, by), (bx + w, y + s), (bx, y + s)]),
                c,
            ));
        }
    } else if set.contains("Quarters") {
        out.push(fill(circle(cx, cy, r * 0.9), LIGHT));
        let frac = index as f64 / (n - 1) as f64;
        if frac >= 1.0 {
            out.push(fill(circle(cx, cy, r * 0.9), GRAY));
        } else if frac > 0.0 {
            let a = std::f64::consts::TAU * frac;
            let rr = r * 0.9;
            let mut segs = vec![
                PathSeg::M(cx as f32, cy as f32),
                PathSeg::L(cx as f32, (cy - rr) as f32),
            ];
            let steps = 16;
            for k in 1..=steps {
                let t = a * f64::from(k) / f64::from(steps);
                segs.push(PathSeg::L(
                    (cx + rr * t.sin()) as f32,
                    (cy - rr * t.cos()) as f32,
                ));
            }
            segs.push(PathSeg::Z);
            out.push(fill(segs, GRAY));
        }
    } else if set.contains("Stars") {
        out.push(fill(star(cx, cy, r * 0.95), LIGHT));
        match index {
            0 => {}
            i if i + 1 >= n => out.push(fill(star(cx, cy, r * 0.95), YELLOW)),
            _ => {
                out.push(Command::PushClip {
                    x: x as f32,
                    y: y as f32,
                    w: (s / 2.0) as f32,
                    h: s as f32,
                });
                out.push(fill(star(cx, cy, r * 0.95), YELLOW));
                out.push(Command::PopClip);
            }
        }
    } else {
        // Traffic lights, signs, symbols, red-to-black: coloured discs.
        let c = if set.contains("RedToBlack") {
            let t = index as f64 / (n - 1) as f64;
            let ch = |a: u8, b: u8| (f64::from(a) + (f64::from(b) - f64::from(a)) * t) as u8;
            Rgba(ch(0x20, RED.0), ch(0x20, RED.1), ch(0x20, RED.2), 255)
        } else {
            ramp(index, n)
        };
        out.push(Command::Path {
            segs: circle(cx, cy, r * 0.85),
            fill: Some(Paint::Solid { color: c }),
            stroke: Some(Stroke {
                width: 0.5,
                color: Rgba(0, 0, 0, 60),
                dash: vec![],
                cap: "butt".into(),
                join: "round".into(),
            }),
        });
        if set.contains("Symbols") {
            // A light mark in the disc: check (best), bar (middle), cross (worst).
            let w = Rgba(255, 255, 255, 255);
            let m = r * 0.45;
            let line = |x1: f64, y1: f64, x2: f64, y2: f64| Command::Line {
                x1: x1 as f32,
                y1: y1 as f32,
                x2: x2 as f32,
                y2: y2 as f32,
                width: (s * 0.12) as f32,
                color: w,
                dash: None,
            };
            if index == 0 {
                out.push(line(cx - m, cy - m, cx + m, cy + m));
                out.push(line(cx - m, cy + m, cx + m, cy - m));
            } else if index + 1 >= n {
                out.push(line(cx - m, cy, cx - m * 0.2, cy + m * 0.8));
                out.push(line(cx - m * 0.2, cy + m * 0.8, cx + m, cy - m * 0.7));
            } else {
                out.push(line(cx - m, cy, cx + m, cy));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_family_draws() {
        for set in [
            "3Arrows",
            "3ArrowsGray",
            "3Flags",
            "3TrafficLights1",
            "3TrafficLights2",
            "3Signs",
            "3Symbols",
            "3Symbols2",
            "4Arrows",
            "4ArrowsGray",
            "4RedToBlack",
            "4Rating",
            "4TrafficLights",
            "5Arrows",
            "5ArrowsGray",
            "5Rating",
            "5Quarters",
            "3Stars",
            "3Triangles",
            "5Boxes",
        ] {
            let n = usize::from(set.as_bytes()[0] - b'0');
            for i in 0..n {
                assert!(!icon(set, i, n, 0.0, 0.0, 10.0).is_empty(), "{set} {i}");
            }
        }
        // Arrows: lowest is red, highest green.
        let lo = icon("3Arrows", 0, 3, 0.0, 0.0, 10.0);
        assert!(
            matches!(&lo[0], Command::Path { fill: Some(Paint::Solid { color }), .. } if *color == RED)
        );
        let hi = icon("3Arrows", 2, 3, 0.0, 0.0, 10.0);
        assert!(
            matches!(&hi[0], Command::Path { fill: Some(Paint::Solid { color }), .. } if *color == GREEN)
        );
    }
}
