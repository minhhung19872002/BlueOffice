//! Reveal patterns of the slide show (N6, BlueSlide round 3): the §19.5.3 animEffect filters
//! (`blinds(horizontal)`, `checkerboard(across)`, `circle(in)`, `dissolve`, `wheel(4)`, …)
//! and the ECMA-376 §19.3.1 transitions that uncover the new slide the same way (blinds,
//! checker, circle, diamond, plus, wedge, wheel, dissolve, strips, random bars, comb, split).
//!
//! A pattern at progress `q` (0: nothing shown, 1: all) is a list of rectangles in fractions
//! of the box being revealed, so any backend draws it with clipped bitmap draws.

/// A way of uncovering a box.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Reveal {
    /// From one side: `l` (left edge first), `r`, `u` (bottom edge first, moving up), `d`.
    Wipe(char),
    /// Bands opening from their top (or left) edge.
    Blinds {
        /// Vertical bands.
        vert: bool,
    },
    /// Staggered squares filling across (or down).
    Checker {
        /// Down instead of across.
        down: bool,
    },
    /// Thin bars in random order.
    RandomBars {
        /// Vertical bars.
        vert: bool,
    },
    /// Small cells in random order.
    Dissolve,
    /// From the middle line out, or from both edges in.
    Barn {
        /// The opening line is vertical (the box opens left and right).
        vert: bool,
        /// From the middle out.
        out: bool,
    },
    /// A centred rectangle growing (out) or the frame closing (in).
    Box {
        /// Growing from the centre.
        out: bool,
    },
    /// A centred circle.
    Circle {
        /// Growing from the centre.
        out: bool,
    },
    /// A centred diamond.
    Diamond {
        /// Growing from the centre.
        out: bool,
    },
    /// A centred cross.
    Plus {
        /// Growing from the centre.
        out: bool,
    },
    /// A saw-tooth edge from a corner (`lu`: from the bottom right towards the top left, as
    /// §19.3.1 strips `dir`).
    Strips {
        /// Moving left.
        left: bool,
        /// Moving up.
        up: bool,
    },
    /// A wedge opening both ways from the top.
    Wedge,
    /// Clock hands sweeping (`n` spokes).
    Wheel(u8),
    /// Interleaved strips from both sides.
    Comb {
        /// Vertical strips.
        vert: bool,
    },
}

impl Reveal {
    /// The pattern of an animEffect filter (`wipe(down)`, `blinds(horizontal)`, …).
    #[must_use]
    pub fn from_filter(filter: &str) -> Option<Reveal> {
        let (name, sub) = match filter.split_once('(') {
            Some((n, s)) => (n, s.trim_end_matches(')')),
            None => (filter, ""),
        };
        let vert = sub.contains("vertical") || sub == "down";
        Some(match name {
            // wipe(down) reveals from the top edge down.
            "wipe" => Reveal::Wipe(match sub {
                "up" => 'u',
                "left" => 'l',
                "right" => 'r',
                _ => 'd',
            }),
            "blinds" => Reveal::Blinds { vert },
            "checkerboard" => Reveal::Checker {
                down: sub == "down",
            },
            "randombar" => Reveal::RandomBars { vert },
            "dissolve" => Reveal::Dissolve,
            "barn" => Reveal::Barn {
                vert: sub.contains("Vertical"),
                out: sub.starts_with("out"),
            },
            "box" => Reveal::Box { out: sub != "in" },
            "circle" => Reveal::Circle { out: sub != "in" },
            "diamond" => Reveal::Diamond { out: sub != "in" },
            "plus" => Reveal::Plus { out: sub != "in" },
            "strips" => Reveal::Strips {
                left: sub.contains("Left"),
                up: sub.starts_with("up"),
            },
            "wedge" => Reveal::Wedge,
            "wheel" => Reveal::Wheel(sub.parse().unwrap_or(1)),
            // slide(fromBottom) peeks in from that side.
            "slide" => Reveal::Wipe(match sub {
                "fromTop" => 'd',
                "fromLeft" => 'r',
                "fromRight" => 'l',
                _ => 'u',
            }),
            _ => return None,
        })
    }

    /// The pattern of a transition kind and variant that uncovers the new slide in place.
    #[must_use]
    pub fn from_transition(kind: &str, dir: Option<&str>) -> Option<Reveal> {
        let dir = dir.unwrap_or("");
        let vert = dir == "vert";
        Some(match kind {
            "blinds" => Reveal::Blinds { vert },
            "checker" => Reveal::Checker { down: vert },
            "randomBar" => Reveal::RandomBars { vert },
            "dissolve" => Reveal::Dissolve,
            "circle" => Reveal::Circle { out: true },
            "diamond" => Reveal::Diamond { out: true },
            "plus" => Reveal::Plus { out: true },
            "wedge" => Reveal::Wedge,
            "wheel" | "wheelReverse" => Reveal::Wheel(dir.parse().unwrap_or(4)),
            "strips" => Reveal::Strips {
                left: dir.starts_with('l'),
                up: dir.ends_with('u'),
            },
            "comb" => Reveal::Comb { vert },
            _ => return None,
        })
    }

    /// Rectangles (fractions `[x, y, w, h]` of the box) shown at progress `q`.
    #[must_use]
    pub fn parts(self, q: f64) -> Vec<[f32; 4]> {
        let q = q.clamp(0.0, 1.0);
        if q >= 1.0 {
            return vec![[0.0, 0.0, 1.0, 1.0]];
        }
        if q <= 0.0 {
            return Vec::new();
        }
        let qf = q as f32;
        match self {
            Reveal::Wipe(side) => vec![match side {
                'u' => [0.0, 1.0 - qf, 1.0, qf],
                'l' => [1.0 - qf, 0.0, qf, 1.0],
                'r' => [0.0, 0.0, qf, 1.0],
                _ => [0.0, 0.0, 1.0, qf],
            }],
            Reveal::Blinds { vert } => {
                let n = 6;
                let k = 1.0 / n as f32;
                (0..n)
                    .map(|i| {
                        let a = i as f32 * k;
                        if vert {
                            [a, 0.0, k * qf, 1.0]
                        } else {
                            [0.0, a, 1.0, k * qf]
                        }
                    })
                    .collect()
            }
            Reveal::Comb { vert } => {
                let n = 12;
                let k = 1.0 / n as f32;
                (0..n)
                    .map(|i| {
                        let a = i as f32 * k;
                        let from_start = i % 2 == 0;
                        let o = if from_start { 0.0 } else { 1.0 - qf };
                        if vert {
                            [a, o, k, qf]
                        } else {
                            [o, a, qf, k]
                        }
                    })
                    .collect()
            }
            Reveal::Checker { down } => {
                // Rows of squares, every other row half a square behind.
                let (cols, rows) = (8, 6);
                let (cw, ch) = (1.0 / cols as f32, 1.0 / rows as f32);
                let mut out = Vec::new();
                for r in 0..rows {
                    // Every square fills in the first half; odd rows start half way.
                    let lag = if r % 2 == 1 { 0.5 } else { 0.0 };
                    let shown = (2.0 * q - lag).clamp(0.0, 1.0) as f32;
                    if shown <= 0.0 {
                        continue;
                    }
                    for c in 0..cols {
                        let (a, b) = (c as f32 * cw, r as f32 * ch);
                        out.push(if down {
                            [b, a, ch, cw * shown]
                        } else {
                            [a, b, cw * shown, ch]
                        });
                    }
                }
                out
            }
            Reveal::RandomBars { vert } => {
                let n = 64;
                let k = 1.0 / n as f32;
                let shown = (q * f64::from(n)).ceil() as u32;
                let rank = ranks(n);
                let on: Vec<bool> = rank.iter().map(|r| *r < shown).collect();
                runs_1d(&on)
                    .into_iter()
                    .map(|(a, len)| {
                        let (a, l) = (a as f32 * k, len as f32 * k);
                        if vert {
                            [a, 0.0, l, 1.0]
                        } else {
                            [0.0, a, 1.0, l]
                        }
                    })
                    .collect()
            }
            Reveal::Dissolve => {
                let (cols, rows) = (48u32, 27u32);
                let n = cols * rows;
                let shown = (q * f64::from(n)).ceil() as u32;
                let rank = ranks(n);
                scan(cols, rows, |c, r| rank[(r * cols + c) as usize] < shown)
            }
            Reveal::Barn { vert, out } => {
                let half = qf / 2.0;
                match (vert, out) {
                    (true, true) => vec![[0.5 - half, 0.0, qf, 1.0]],
                    (false, true) => vec![[0.0, 0.5 - half, 1.0, qf]],
                    (true, false) => vec![[0.0, 0.0, half, 1.0], [1.0 - half, 0.0, half, 1.0]],
                    (false, false) => vec![[0.0, 0.0, 1.0, half], [0.0, 1.0 - half, 1.0, half]],
                }
            }
            Reveal::Box { out: true } => {
                vec![[0.5 - qf / 2.0, 0.5 - qf / 2.0, qf, qf]]
            }
            Reveal::Box { out: false } => {
                let e = qf / 2.0;
                vec![
                    [0.0, 0.0, 1.0, e],
                    [0.0, 1.0 - e, 1.0, e],
                    [0.0, e, e, 1.0 - 2.0 * e],
                    [1.0 - e, e, e, 1.0 - 2.0 * e],
                ]
            }
            Reveal::Circle { out } => {
                // Radius reaching the corners at the end.
                let full = std::f64::consts::FRAC_1_SQRT_2;
                let rad = if out { q * full } else { (1.0 - q) * full };
                scan(96, 96, |c, r| {
                    let (x, y) = cell(c, r, 96, 96);
                    let inside = (x - 0.5).hypot(y - 0.5) <= rad;
                    inside == out
                })
            }
            Reveal::Diamond { out } => {
                let rad = if out { q } else { 1.0 - q };
                scan(96, 96, |c, r| {
                    let (x, y) = cell(c, r, 96, 96);
                    let inside = (x - 0.5).abs() + (y - 0.5).abs() <= rad;
                    inside == out
                })
            }
            Reveal::Plus { out } => {
                let rad = if out { q / 2.0 } else { (1.0 - q) / 2.0 };
                scan(96, 96, |c, r| {
                    let (x, y) = cell(c, r, 96, 96);
                    let inside = (x - 0.5).abs() <= rad || (y - 0.5).abs() <= rad;
                    inside == out
                })
            }
            Reveal::Strips { left, up } => scan(96, 96, |c, r| {
                let (x, y) = cell(c, r, 96, 96);
                let x = if left { 1.0 - x } else { x };
                let y = if up { 1.0 - y } else { y };
                f64::midpoint(x, y) <= q
            }),
            Reveal::Wedge => scan(96, 96, |c, r| {
                let (x, y) = cell(c, r, 96, 96);
                // Angle from straight up, both ways.
                let a = (x - 0.5).atan2(0.5 - y).abs();
                a <= q * std::f64::consts::PI
            }),
            Reveal::Wheel(spokes) => {
                let n = f64::from(spokes.max(1));
                let sector = std::f64::consts::TAU / n;
                scan(96, 96, |c, r| {
                    let (x, y) = cell(c, r, 96, 96);
                    let a = (x - 0.5).atan2(0.5 - y).rem_euclid(std::f64::consts::TAU);
                    a % sector <= q * sector
                })
            }
        }
    }
}

/// Centre of cell (`c`, `r`) of a `cols` × `rows` grid, as fractions.
fn cell(c: u32, r: u32, cols: u32, rows: u32) -> (f64, f64) {
    (
        (f64::from(c) + 0.5) / f64::from(cols),
        (f64::from(r) + 0.5) / f64::from(rows),
    )
}

/// A fixed shuffle of `0..n`: element `i` is the position at which item `i` shows.
fn ranks(n: u32) -> Vec<u32> {
    let hash = |i: u32| {
        let mut h = i.wrapping_mul(0x9E37_79B9) ^ (i >> 3);
        h ^= h >> 15;
        h = h.wrapping_mul(0x85EB_CA6B);
        h ^ (h >> 13)
    };
    let mut order: Vec<u32> = (0..n).collect();
    order.sort_by_key(|i| (hash(*i), *i));
    let mut rank = vec![0; n as usize];
    for (pos, i) in order.into_iter().enumerate() {
        rank[i as usize] = pos as u32;
    }
    rank
}

/// Runs of `true` in a row: (start, length).
fn runs_1d(on: &[bool]) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut start = None;
    for (i, v) in on.iter().chain(std::iter::once(&false)).enumerate() {
        match (start, *v) {
            (None, true) => start = Some(i),
            (Some(s), false) => {
                out.push((s, i - s));
                start = None;
            }
            _ => {}
        }
    }
    out
}

/// Rectangles covering the cells of a grid where `on` holds, merged along each row.
fn scan(cols: u32, rows: u32, on: impl Fn(u32, u32) -> bool) -> Vec<[f32; 4]> {
    let (cw, rh) = (1.0 / cols as f32, 1.0 / rows as f32);
    let mut out = Vec::new();
    for r in 0..rows {
        let row: Vec<bool> = (0..cols).map(|c| on(c, r)).collect();
        for (a, len) in runs_1d(&row) {
            out.push([a as f32 * cw, r as f32 * rh, len as f32 * cw, rh]);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area(parts: &[[f32; 4]]) -> f32 {
        parts.iter().map(|p| p[2] * p[3]).sum()
    }

    #[test]
    fn filters_and_transitions_map_to_patterns() {
        assert_eq!(
            Reveal::from_filter("blinds(horizontal)"),
            Some(Reveal::Blinds { vert: false })
        );
        assert_eq!(
            Reveal::from_filter("circle(in)"),
            Some(Reveal::Circle { out: false })
        );
        assert_eq!(Reveal::from_filter("wheel(4)"), Some(Reveal::Wheel(4)));
        assert_eq!(Reveal::from_filter("wipe(down)"), Some(Reveal::Wipe('d')));
        assert_eq!(Reveal::from_filter("fade"), None);
        assert_eq!(
            Reveal::from_transition("checker", Some("vert")),
            Some(Reveal::Checker { down: true })
        );
        assert_eq!(Reveal::from_transition("push", None), None);
    }

    #[test]
    fn every_pattern_grows_from_nothing_to_everything() {
        let all = [
            Reveal::Wipe('l'),
            Reveal::Blinds { vert: true },
            Reveal::Checker { down: false },
            Reveal::RandomBars { vert: false },
            Reveal::Dissolve,
            Reveal::Barn {
                vert: true,
                out: false,
            },
            Reveal::Box { out: false },
            Reveal::Circle { out: true },
            Reveal::Diamond { out: false },
            Reveal::Plus { out: true },
            Reveal::Strips {
                left: true,
                up: false,
            },
            Reveal::Wedge,
            Reveal::Wheel(2),
            Reveal::Comb { vert: false },
        ];
        for r in all {
            assert!(r.parts(0.0).is_empty(), "{r:?}");
            assert!((area(&r.parts(1.0)) - 1.0).abs() < 1e-4, "{r:?}");
            let (a, b) = (area(&r.parts(0.3)), area(&r.parts(0.7)));
            assert!(a > 0.0 && a < b && b < 1.0 + 1e-4, "{r:?}: {a} {b}");
            for p in r.parts(0.5) {
                assert!(
                    p[0] >= -1e-4 && p[1] >= -1e-4 && p[0] + p[2] <= 1.0 + 1e-4,
                    "{r:?}"
                );
            }
        }
    }

    #[test]
    fn circle_covers_about_its_area() {
        let a = area(&Reveal::Circle { out: true }.parts(0.5));
        // Radius 0.354 of the box: π r² ≈ 0.39.
        assert!((a - 0.39).abs() < 0.03, "{a}");
    }
}
