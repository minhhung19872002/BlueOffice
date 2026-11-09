//! Vector icons for conditional-format icon sets (ECMA-376 Part 1, §18.18.42
//! `ST_IconSetType`, plus the [MS-XLSX] x14 sets `3Stars`, `3Triangles`, `5Boxes`).
//! The shapes and colours are BlueSheet's own simple designs (arrows, discs, a
//! housing for the framed lights, signs, marks, flags, signal bars, pies, stars,
//! triangles and box grids); icon 0 is always the lowest band.

use bo_render::{Command, Paint, PathSeg, Rgba, Stroke};

const RED: Rgba = Rgba(0xD6, 0x45, 0x45, 255);
const ORANGE: Rgba = Rgba(0xF2, 0x8C, 0x28, 255);
const YELLOW: Rgba = Rgba(0xE0, 0xB4, 0x1F, 255);
const LIME: Rgba = Rgba(0x8B, 0xC3, 0x4A, 255);
const GREEN: Rgba = Rgba(0x2F, 0xA3, 0x6B, 255);
const GRAY: Rgba = Rgba(0x8A, 0x90, 0x99, 255);
const DARK: Rgba = Rgba(0x3A, 0x3F, 0x47, 255);
const LIGHT: Rgba = Rgba(0xD9, 0xDD, 0xE2, 255);
const BLUE: Rgba = Rgba(0x2E, 0x6D, 0xB4, 255);
const WHITE: Rgba = Rgba(255, 255, 255, 255);

/// Every icon set name BlueSheet draws (§18.18.42 and the x14 additions).
pub const ICON_SETS: [&str; 20] = [
    "3Arrows",
    "3ArrowsGray",
    "3Flags",
    "3TrafficLights1",
    "3TrafficLights2",
    "3Signs",
    "3Symbols",
    "3Symbols2",
    "3Stars",
    "3Triangles",
    "4Arrows",
    "4ArrowsGray",
    "4RedToBlack",
    "4Rating",
    "4TrafficLights",
    "5Arrows",
    "5ArrowsGray",
    "5Rating",
    "5Quarters",
    "5Boxes",
];

/// Number of icons of a set: the leading digit of its name (3, 4 or 5); 0 for `NoIcons`.
#[must_use]
pub fn icon_count(set: &str) -> usize {
    set.chars()
        .next()
        .and_then(|c| c.to_digit(10))
        .map_or(if set == "NoIcons" { 0 } else { 3 }, |d| d as usize)
        .clamp(0, 5)
}

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

fn outlined(segs: Vec<PathSeg>, c: Rgba) -> Command {
    Command::Path {
        segs,
        fill: Some(Paint::Solid { color: c }),
        stroke: Some(Stroke {
            width: 0.5,
            color: Rgba(0, 0, 0, 70),
            dash: vec![],
            cap: "butt".into(),
            join: "round".into(),
        }),
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

fn line(x1: f64, y1: f64, x2: f64, y2: f64, w: f64, c: Rgba) -> Command {
    Command::Line {
        x1: x1 as f32,
        y1: y1 as f32,
        x2: x2 as f32,
        y2: y2 as f32,
        width: w as f32,
        color: c,
        dash: None,
    }
}

/// Cross (worst), exclamation (middle) or check (best) mark centred at (`cx`, `cy`).
#[allow(clippy::too_many_arguments)]
fn mark(out: &mut Vec<Command>, index: usize, n: usize, cx: f64, cy: f64, m: f64, w: f64, c: Rgba) {
    if index == 0 {
        out.push(line(cx - m, cy - m, cx + m, cy + m, w, c));
        out.push(line(cx - m, cy + m, cx + m, cy - m, w, c));
    } else if index + 1 >= n {
        out.push(line(cx - m, cy, cx - m * 0.2, cy + m * 0.8, w, c));
        out.push(line(cx - m * 0.2, cy + m * 0.8, cx + m, cy - m * 0.7, w, c));
    } else {
        out.push(line(cx, cy - m, cx, cy + m * 0.25, w, c));
        out.push(line(cx, cy + m * 0.6, cx, cy + m, w, c));
    }
}

/// Commands drawing icon `index` (0 = lowest) of an icon set of `count`
/// icons, in the `s × s` square at (`x`, `y`).
#[allow(clippy::too_many_lines)]
pub fn icon(set: &str, index: usize, count: usize, x: f64, y: f64, s: f64) -> Vec<Command> {
    let (cx, cy, r) = (x + s / 2.0, y + s / 2.0, s / 2.0);
    let n = count.max(2);
    let index = index.min(n - 1);
    let mut out = Vec::new();
    if set.contains("Arrows") {
        // Lowest points down, highest up, the others in between.
        let angle = 180.0 - 180.0 * index as f64 / (n - 1) as f64;
        let c = if set.contains("Gray") {
            GRAY
        } else {
            ramp(index, n)
        };
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
    } else if set.contains("Rating") {
        // Four signal bars of rising height; 5Rating fills `index` bars (0–4),
        // 4Rating `index + 1` (1–4).
        let bars = 4;
        let filled = if n >= 5 { index } else { index + 1 };
        let w = s / (bars as f64 * 1.5);
        for b in 0..bars {
            let hgt = s * (0.3 + 0.7 * (b + 1) as f64 / bars as f64);
            let bx = x + b as f64 * w * 1.5;
            let by = y + s - hgt;
            let c = if b < filled { BLUE } else { LIGHT };
            out.push(fill(
                poly(&[(bx, by), (bx + w, by), (bx + w, y + s), (bx, y + s)]),
                c,
            ));
        }
    } else if set.contains("Boxes") {
        // 2 × 2 grid of squares; `index` of them filled (0–4), top-left first.
        let g = s * 0.1;
        let w = (s - 3.0 * g) / 2.0;
        for b in 0..4 {
            let bx = x + g + (b % 2) as f64 * (w + g);
            let by = y + g + (b / 2) as f64 * (w + g);
            let c = if b < index { BLUE } else { LIGHT };
            out.push(fill(
                poly(&[(bx, by), (bx + w, by), (bx + w, by + w), (bx, by + w)]),
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
    } else if set.contains("Signs") {
        // Diamond (worst), upward triangle (middle), disc (best).
        let c = ramp(index, 3.min(n));
        let h = r * 0.9;
        let segs = match index {
            0 => poly(&[(cx, cy - h), (cx + h, cy), (cx, cy + h), (cx - h, cy)]),
            i if i + 1 >= n => circle(cx, cy, r * 0.82),
            _ => poly(&[(cx, cy - h), (cx + h, cy + h * 0.8), (cx - h, cy + h * 0.8)]),
        };
        out.push(outlined(segs, c));
    } else if set.contains("Symbols2") {
        // Coloured marks without a disc.
        mark(
            &mut out,
            index,
            n,
            cx,
            cy,
            r * 0.7,
            s * 0.16,
            ramp(index, 3.min(n)),
        );
    } else if set.contains("Symbols") {
        out.push(outlined(circle(cx, cy, r * 0.88), ramp(index, 3.min(n))));
        mark(&mut out, index, n, cx, cy, r * 0.45, s * 0.12, WHITE);
    } else if set == "3TrafficLights2" {
        // A dark housing around the light.
        let h = r * 0.98;
        out.push(fill(
            poly(&[
                (cx - h, cy - h),
                (cx + h, cy - h),
                (cx + h, cy + h),
                (cx - h, cy + h),
            ]),
            DARK,
        ));
        out.push(fill(circle(cx, cy, r * 0.68), ramp(index, 3)));
    } else {
        // Traffic lights and red-to-black: coloured discs.
        let c = if set.contains("RedToBlack") {
            let t = index as f64 / (n - 1) as f64;
            let ch = |a: u8, b: u8| (f64::from(a) + (f64::from(b) - f64::from(a)) * t) as u8;
            Rgba(ch(0x20, RED.0), ch(0x20, RED.1), ch(0x20, RED.2), 255)
        } else if set == "4TrafficLights" {
            [DARK, RED, YELLOW, GREEN][index.min(3)]
        } else {
            ramp(index, n)
        };
        out.push(outlined(circle(cx, cy, r * 0.85), c));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fill_of(c: &Command) -> Option<Rgba> {
        match c {
            Command::Path {
                fill: Some(Paint::Solid { color }),
                ..
            } => Some(*color),
            _ => None,
        }
    }

    #[test]
    fn every_family_draws() {
        for set in ICON_SETS {
            let n = icon_count(set);
            assert!((3..=5).contains(&n), "{set}");
            for i in 0..n {
                assert!(!icon(set, i, n, 0.0, 0.0, 10.0).is_empty(), "{set} {i}");
            }
        }
        assert_eq!(icon_count("NoIcons"), 0);
        // Arrows: lowest is red, highest green.
        assert_eq!(
            fill_of(&icon("3Arrows", 0, 3, 0.0, 0.0, 10.0)[0]),
            Some(RED)
        );
        assert_eq!(
            fill_of(&icon("3Arrows", 2, 3, 0.0, 0.0, 10.0)[0]),
            Some(GREEN)
        );
        // Boxes: icon k fills k of the four squares.
        let filled = |i| {
            icon("5Boxes", i, 5, 0.0, 0.0, 10.0)
                .iter()
                .filter(|c| fill_of(c) == Some(BLUE))
                .count()
        };
        assert_eq!((0..5).map(filled).collect::<Vec<_>>(), vec![0, 1, 2, 3, 4]);
        // 4Rating starts at one bar, 5Rating at none.
        let bars = |set: &str, i, n| {
            icon(set, i, n, 0.0, 0.0, 10.0)
                .iter()
                .filter(|c| fill_of(c) == Some(BLUE))
                .count()
        };
        assert_eq!(bars("4Rating", 0, 4), 1);
        assert_eq!(bars("5Rating", 0, 5), 0);
        assert_eq!(bars("5Rating", 4, 5), 4);
        // The framed lights draw a housing first.
        assert_eq!(
            fill_of(&icon("3TrafficLights2", 1, 3, 0.0, 0.0, 10.0)[0]),
            Some(DARK)
        );
    }
}
