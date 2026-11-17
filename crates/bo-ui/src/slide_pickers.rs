//! Drop-down pickers of BlueSlide's ribbon (N6, round 4), drawn like the reference
//! presentation app's: the Insert Table grid, the shapes gallery (preset outlines in
//! sections), the icon library, symbols, quick styles and colour palettes.
use crate::theme::Theme;
use crate::{Painter, TextStyle};
use bo_render::{PathSeg, Rgba};

/// What a picker offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PickKind {
    /// Rows × columns of a new table.
    Table,
    /// Preset shapes.
    Shapes,
    /// Icons of the library.
    Icons,
    /// Symbols to type.
    Symbols,
    /// Shape quick styles (theme accents).
    Styles,
    /// Shape fill colour.
    Fill,
    /// Shape outline colour.
    Outline,
    /// Animation effects by class (Entrance, Emphasis, Exit, Motion Paths): the
    /// Animations tab's gallery opened in full (round 17).
    Effects,
}

/// A gallery section: heading key, class and effects (engine id, name key).
pub type EffectSection = (
    &'static str,
    &'static str,
    &'static [(&'static str, &'static str)],
);

/// The animation gallery by section (heading key, class, effects as engine id and `slideFx`
/// name key), in the reference's order and names; effects the engine's catalog lacks
/// (Color Pulse, Desaturate, …) are left out, and the catalog's other effects are in the
/// More … Effects dialogs.
pub const EFFECT_SECTIONS: [EffectSection; 5] = [
    ("slideNative.fxNone", "none", &[("none", "none")]),
    (
        "slideNative.fxEntrance",
        "entr",
        &[
            ("appear", "appear"),
            ("fade", "fade"),
            ("flyIn", "flyIn"),
            ("float", "floatIn"),
            ("split", "split"),
            ("wipe", "wipe"),
            ("circle", "shape"),
            ("wheel", "wheel"),
            ("randomBars", "randomBars"),
            ("growTurn", "growTurn"),
            ("zoom", "zoom"),
            ("swivel", "swivel"),
            ("bounce", "bounce"),
        ],
    ),
    (
        "slideNative.fxEmphasis",
        "emph",
        &[
            ("pulse", "pulse"),
            ("colorPulse", "colorPulse"),
            ("teeter", "teeter"),
            ("spin", "spin"),
            ("growShrink", "growShrink"),
            ("desaturate", "desaturate"),
            ("darken", "darken"),
            ("lighten", "lighten"),
            ("transparency", "transparency"),
            ("objectColor", "objectColor"),
            ("complementary", "complementary"),
            ("lineColor", "lineColor"),
            ("fillColor", "fillColor"),
            ("brushColor", "brushColor"),
            ("fontColor", "fontColor"),
            ("underline", "underline"),
            ("boldFlash", "boldFlash"),
            ("boldReveal", "boldReveal"),
            ("waveText", "waveText"),
        ],
    ),
    (
        "slideNative.fxExit",
        "exit",
        &[
            ("appear", "disappear"),
            ("fade", "fade"),
            ("flyIn", "flyOut"),
            ("float", "floatOut"),
            ("split", "split"),
            ("wipe", "wipe"),
            ("circle", "shape"),
            ("wheel", "wheel"),
            ("randomBars", "randomBars"),
            ("growTurn", "shrinkTurn"),
            ("zoom", "zoom"),
            ("swivel", "swivel"),
            ("bounce", "bounce"),
        ],
    ),
    (
        "slideNative.fxPaths",
        "path",
        &[
            ("lineDown", "lines"),
            ("arcDown", "arcs"),
            ("turnDown", "turns"),
            ("circlePath", "shapes"),
            ("loop", "loops"),
            ("custom", "customPath"),
        ],
    ),
];

/// Rows under the gallery: the More … Effects dialogs (`slideNative` keys); OLE Action Verbs
/// is greyed.
pub const EFFECT_MORE: [(&str, &str); 5] = [
    ("moreEntrance", "entr"),
    ("moreEmphasis", "emph"),
    ("moreExit", "exit"),
    ("morePaths", "path"),
    ("oleVerbs", ""),
];

/// Columns of the effects gallery (reference: 5 tiles of 124 × 84 px at 125 %).
pub const EFFECT_COLS: usize = 5;

/// Colour of an effect class's star (entrance green, emphasis amber, exit red, path blue).
#[must_use]
pub fn class_color(class: &str) -> Rgba {
    match class {
        // The reference's meaning of the classes: muted green entrance, golden emphasis,
        // red exit.
        "emph" => Rgba(0xe8, 0xb4, 0x4a, 255),
        "exit" => Rgba(0xd0, 0x53, 0x3f, 255),
        "path" => Rgba(0x5b, 0x8f, 0xd0, 255),
        "none" => Rgba(0xa6, 0xa6, 0xa6, 255),
        _ => Rgba(0x5a, 0x9e, 0x8c, 255),
    }
}

/// A five-pointed star path centred on (`cx`, `cy`), outer radius `r`, turned `rot` radians.
fn star_path(cx: f32, cy: f32, r: f32, rot: f32) -> Vec<PathSeg> {
    let mut v = Vec::with_capacity(11);
    for k in 0..10 {
        let a = std::f32::consts::PI * (k as f32) / 5.0 - std::f32::consts::FRAC_PI_2 + rot;
        let rr = if k % 2 == 0 { r } else { r * 0.45 };
        let (x, y) = (cx + rr * a.cos(), cy + rr * a.sin());
        v.push(if k == 0 {
            PathSeg::M(x, y)
        } else {
            PathSeg::L(x, y)
        });
    }
    v.push(PathSeg::Z);
    v
}

/// One effect's tile picture (own drawings in BlueSlide's family): the shaded star of its
/// class with a mark of what the effect does — rays (Appear), a fading half (Fade, and
/// Transparency), an arrow (Fly In), a lift (Float In), a split, a wipe, a shape cut-out,
/// spokes (Wheel), bars, a small star turning (Grow & Turn), corner ticks (Zoom), a turning
/// arc (Swivel, Spin), hops (Bounce), a halo (Pulse), a tilt (Teeter), a grown outline
/// (Grow/Shrink), a second colour (Fill Color).
pub fn paint_effect_icon(p: &mut dyn Painter, id: &str, class: &str, cx: f32, cy: f32, r: f32) {
    let base = class_color(class);
    let white = Rgba(255, 255, 255, 255);
    let dark = Rgba(0x40, 0x40, 0x40, 255);
    let line = (r / 9.0).max(1.0).round();
    let star_r = if matches!(id, "flyIn" | "float" | "growTurn" | "bounce") {
        r * 0.82
    } else {
        r
    };
    match id {
        "teeter" => {
            p.path(&star_path(cx, cy, r, 0.28), Some(base), Some((dark, 1.0)));
        }
        "growTurn" => {
            crate::slide_ribbon::paint_star(p, cx + r * 0.2, cy - r * 0.1, star_r, base, true);
            p.path(
                &star_path(cx - r * 0.8, cy + r * 0.6, r * 0.38, 0.4),
                Some(base),
                Some((dark, 1.0)),
            );
        }
        "growShrink" => {
            crate::slide_ribbon::paint_star(p, cx - r * 0.25, cy + r * 0.2, r * 0.7, base, true);
            p.path(
                &star_path(cx + r * 0.25, cy - r * 0.15, r * 0.95, 0.0),
                None,
                Some((dark, 1.0)),
            );
        }
        _ => crate::slide_ribbon::paint_star(
            p,
            cx,
            if id == "float" { cy - r * 0.15 } else { cy },
            star_r,
            base,
            true,
        ),
    }
    let seg = |pts: &[(f32, f32)]| -> Vec<PathSeg> {
        pts.iter()
            .enumerate()
            .map(|(i, (a, b))| {
                let (x, y) = (cx + a * r, cy + b * r);
                if i == 0 {
                    PathSeg::M(x, y)
                } else {
                    PathSeg::L(x, y)
                }
            })
            .collect()
    };
    match id {
        "appear" => {
            for (a, b, c, d) in [
                (-1.25, -0.9, -1.0, -0.7),
                (1.25, -0.9, 1.0, -0.7),
                (-1.2, 0.6, -0.95, 0.45),
                (1.2, 0.6, 0.95, 0.45),
                (0.0, -1.35, 0.0, -1.1),
            ] {
                p.path(&seg(&[(a, b), (c, d)]), None, Some((base, line)));
            }
        }
        "fade" | "transparency" => {
            p.push_clip(cx, cy - r, r + 1.0, 2.0 * r);
            p.path(
                &star_path(cx, cy, r, 0.0),
                Some(Rgba(255, 255, 255, 150)),
                None,
            );
            p.pop_clip();
        }
        "flyIn" => {
            p.path(&seg(&[(0.0, 1.35), (0.0, 0.75)]), None, Some((dark, line)));
            p.path(
                &seg(&[(-0.18, 0.95), (0.0, 0.72), (0.18, 0.95)]),
                None,
                Some((dark, line)),
            );
        }
        "float" => {
            for x in [-0.35, 0.0, 0.35] {
                p.path(&seg(&[(x, 1.25), (x, 0.95)]), None, Some((base, line)));
            }
        }
        "split" => {
            p.fill_rect((cx - line / 2.0).round(), cy - r, line, 2.0 * r, white);
        }
        "wipe" => {
            p.push_clip(cx - r, cy + r * 0.1, 2.0 * r, r);
            p.path(
                &star_path(cx, cy, r, 0.0),
                Some(Rgba(255, 255, 255, 150)),
                None,
            );
            p.pop_clip();
        }
        "circle" => {
            let k = (r * 0.36).round();
            p.fill_rect(cx - k / 2.0, cy - k / 2.0 + r * 0.08, k, k, white);
        }
        "wheel" => {
            for (a, b) in [(0.0, -0.75), (0.7, 0.2), (-0.7, 0.2)] {
                p.path(&seg(&[(0.0, 0.08), (a, b)]), None, Some((white, line)));
            }
        }
        "randomBars" => {
            for y in [-0.3, 0.05, 0.4] {
                p.fill_rect(cx - r * 0.7, (cy + y * r).round(), 1.4 * r, line, white);
            }
        }
        "zoom" => {
            for (sx, sy) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
                p.path(
                    &seg(&[(sx * 1.3, sy * 1.1), (sx * 1.0, sy * 0.85)]),
                    None,
                    Some((dark, line)),
                );
            }
        }
        "swivel" | "spin" => {
            let arc: Vec<(f32, f32)> = (0..=10)
                .map(|i| {
                    let a = -2.6 + 2.2 * i as f32 / 10.0;
                    (1.25 * a.cos(), 1.15 * a.sin())
                })
                .collect();
            p.path(&seg(&arc), None, Some((dark, line)));
            let (ex, ey) = arc[arc.len() - 1];
            p.path(
                &seg(&[(ex - 0.25, ey - 0.05), (ex, ey), (ex - 0.05, ey + 0.25)]),
                None,
                Some((dark, line)),
            );
        }
        "bounce" => {
            for (x, y) in [(-1.3, 0.2), (-1.15, -0.25), (-0.95, 0.15), (-1.25, 0.95)] {
                let d = line.max(2.0);
                p.fill_rect(cx + x * r, cy + y * r, d, d, dark);
            }
        }
        "pulse" => {
            p.path(
                &star_path(cx, cy, r * 1.25, 0.0),
                None,
                Some((Rgba(base.0, base.1, base.2, 140), line)),
            );
        }
        "fillColor" | "objectColor" | "complementary" | "desaturate" | "darken" => {
            let c = match id {
                "objectColor" => Rgba(0x70, 0x30, 0xa0, 255),
                "desaturate" => Rgba(0xa6, 0xa6, 0xa6, 255),
                "darken" => Rgba(0x6e, 0x55, 0x2a, 255),
                _ => Rgba(0x44, 0x80, 0xc0, 255),
            };
            // A second colour on one half (left for fills, right for the colour changes).
            let x0 = if id == "fillColor" { cx - r } else { cx };
            p.push_clip(x0, cy - r, r + 1.0, 2.0 * r);
            p.path(&star_path(cx, cy, r, 0.0), Some(c), None);
            p.pop_clip();
        }
        "lighten" => {
            p.push_clip(cx, cy - r, r + 1.0, 2.0 * r);
            p.path(
                &star_path(cx, cy, r, 0.0),
                Some(Rgba(255, 245, 220, 200)),
                None,
            );
            p.pop_clip();
        }
        "colorPulse" => {
            p.path(
                &star_path(cx, cy, r * 1.25, 0.0),
                None,
                Some((Rgba(0x44, 0x80, 0xc0, 200), line)),
            );
        }
        "lineColor" => {
            p.path(
                &star_path(cx, cy, r, 0.0),
                None,
                Some((Rgba(0x44, 0x80, 0xc0, 255), line * 2.0)),
            );
        }
        "brushColor" | "fontColor" => {
            let c = if id == "brushColor" {
                Rgba(0x70, 0x30, 0xa0, 255)
            } else {
                Rgba(0xd0, 0x30, 0x30, 255)
            };
            // A letter on the star and the new colour under it.
            let t = (r * 0.9).round();
            p.path(
                &seg(&[(-0.3, 0.35), (0.0, -0.35), (0.3, 0.35)]),
                None,
                Some((dark, line)),
            );
            p.path(&seg(&[(-0.18, 0.1), (0.18, 0.1)]), None, Some((dark, line)));
            p.fill_rect(
                cx - t * 0.6,
                cy + r * 0.75,
                t * 1.2,
                (line * 2.0).max(2.0),
                c,
            );
        }
        "underline" => {
            p.fill_rect(cx - r * 0.8, (cy + r * 1.05).round(), r * 1.6, line, dark);
        }
        "boldFlash" | "boldReveal" => {
            // A bold B on the star (flash: rays beside it).
            p.path(
                &seg(&[(-0.2, -0.35), (-0.2, 0.35)]),
                None,
                Some((white, line * 2.0)),
            );
            p.path(
                &seg(&[
                    (-0.2, -0.35),
                    (0.15, -0.3),
                    (0.15, -0.05),
                    (-0.2, 0.0),
                    (0.2, 0.05),
                    (0.2, 0.3),
                    (-0.2, 0.35),
                ]),
                None,
                Some((white, line * 1.5)),
            );
            if id == "boldFlash" {
                for (a, b, c, d) in [(-1.25, -0.9, -1.0, -0.7), (1.25, -0.9, 1.0, -0.7)] {
                    p.path(&seg(&[(a, b), (c, d)]), None, Some((base, line)));
                }
            }
        }
        "waveText" => {
            let pts: Vec<(f32, f32)> = (0..=12)
                .map(|i| {
                    let t = i as f32 / 12.0;
                    (
                        t * 1.8 - 0.9,
                        1.15 + (t * std::f32::consts::TAU).sin() * 0.12,
                    )
                })
                .collect();
            p.path(&seg(&pts), None, Some((dark, line)));
        }
        _ => {}
    }
}

/// A motion path tile: the path's own outline (dotted) from a green start to a red end
/// (own drawing).
pub fn paint_path_tile(p: &mut dyn Painter, id: &str, cx: f32, cy: f32, r: f32) {
    let pts: Vec<(f32, f32)> = match id {
        "lineDown" => vec![(0.0, -1.0), (0.0, 1.0)],
        "lineUp" => vec![(0.0, 1.0), (0.0, -1.0)],
        "lineLeft" => vec![(1.0, 0.0), (-1.0, 0.0)],
        "lineRight" => vec![(-1.0, 0.0), (1.0, 0.0)],
        "arcDown" | "arcUp" => {
            let s = if id == "arcDown" { 1.0 } else { -1.0 };
            (0..=12)
                .map(|i| {
                    let a = std::f32::consts::PI * i as f32 / 12.0;
                    (-a.cos(), s * a.sin() * 0.8)
                })
                .collect()
        }
        "turnDown" => vec![
            (-1.0, -0.6),
            (0.2, -0.6),
            (0.6, -0.4),
            (0.7, 0.0),
            (0.7, 1.0),
        ],
        "circlePath" | "loop" => (0..=24)
            .map(|i| {
                let a = std::f32::consts::TAU * i as f32 / 24.0;
                (a.sin() * 0.9, -a.cos() * 0.9)
            })
            .collect(),
        "squarePath" => vec![
            (-0.8, -0.8),
            (0.8, -0.8),
            (0.8, 0.8),
            (-0.8, 0.8),
            (-0.8, -0.8),
        ],
        "diamondPath" => vec![
            (0.0, -1.0),
            (1.0, 0.0),
            (0.0, 1.0),
            (-1.0, 0.0),
            (0.0, -1.0),
        ],
        "triangle" => vec![(0.0, -0.9), (0.9, 0.8), (-0.9, 0.8), (0.0, -0.9)],
        _ => (0..=16)
            .map(|i| {
                let t = i as f32 / 16.0;
                (t * 2.0 - 1.0, (t * std::f32::consts::TAU).sin() * 0.45)
            })
            .collect(),
    };
    let at = |(x, y): (f32, f32)| (cx + x * r, cy + y * r);
    // Dots along the path, every few pixels.
    let mut prev = at(pts[0]);
    let mut carry = 0.0f32;
    let step = (r / 4.0).max(2.0);
    for q in pts.iter().skip(1) {
        let next = at(*q);
        let len = (next.0 - prev.0).hypot(next.1 - prev.1);
        let mut d = carry;
        while d < len {
            let t = d / len.max(1e-3);
            let (x, y) = (
                prev.0 + (next.0 - prev.0) * t,
                prev.1 + (next.1 - prev.1) * t,
            );
            p.fill_rect(
                x.round() - 1.0,
                y.round() - 1.0,
                2.0,
                2.0,
                Rgba(0x40, 0x40, 0x40, 255),
            );
            d += step;
        }
        carry = d - len;
        prev = next;
    }
    let (sx, sy) = at(pts[0]);
    let (ex, ey) = at(pts[pts.len() - 1]);
    let dot = (r / 5.0).max(3.0).round();
    p.fill_rect(
        sx - dot / 2.0,
        sy - dot / 2.0,
        dot,
        dot,
        Rgba(0x3f, 0xa3, 0x4d, 255),
    );
    p.fill_rect(
        ex - dot / 2.0,
        ey - dot / 2.0,
        dot,
        dot,
        Rgba(0xd2, 0x4a, 0x3c, 255),
    );
}

/// Preset shapes of the gallery by section (string key of the heading, presets); presets
/// the engine does not know are left out when the picker opens.
pub const SHAPE_SECTIONS: [(&str, &[&str]); 8] = [
    (
        "slideNative.shapesLines",
        &[
            "line",
            "straightConnector1",
            "bentConnector3",
            "curvedConnector3",
        ],
    ),
    (
        "slideNative.shapesRect",
        &[
            "rect",
            "roundRect",
            "snip1Rect",
            "snip2SameRect",
            "snipRoundRect",
            "round1Rect",
            "round2SameRect",
            "round2DiagRect",
        ],
    ),
    (
        "slideNative.shapesBasic",
        &[
            "ellipse",
            "triangle",
            "rtTriangle",
            "parallelogram",
            "trapezoid",
            "diamond",
            "pentagon",
            "hexagon",
            "heptagon",
            "octagon",
            "decagon",
            "dodecagon",
            "pie",
            "chord",
            "teardrop",
            "frame",
            "halfFrame",
            "corner",
            "diagStripe",
            "plus",
            "plaque",
            "can",
            "cube",
            "bevel",
            "donut",
            "noSmoking",
            "blockArc",
            "foldedCorner",
            "smileyFace",
            "heart",
            "lightningBolt",
            "sun",
            "moon",
            "cloud",
            "arc",
            "bracketPair",
            "bracePair",
            "leftBracket",
            "rightBracket",
            "leftBrace",
            "rightBrace",
        ],
    ),
    (
        "slideNative.shapesArrows",
        &[
            "rightArrow",
            "leftArrow",
            "upArrow",
            "downArrow",
            "leftRightArrow",
            "upDownArrow",
            "quadArrow",
            "leftRightUpArrow",
            "bentArrow",
            "uturnArrow",
            "leftUpArrow",
            "bentUpArrow",
            "curvedRightArrow",
            "curvedLeftArrow",
            "curvedUpArrow",
            "curvedDownArrow",
            "stripedRightArrow",
            "notchedRightArrow",
            "homePlate",
            "chevron",
            "rightArrowCallout",
            "downArrowCallout",
            "leftArrowCallout",
            "upArrowCallout",
            "circularArrow",
        ],
    ),
    (
        "slideNative.shapesEquation",
        &[
            "mathPlus",
            "mathMinus",
            "mathMultiply",
            "mathDivide",
            "mathEqual",
            "mathNotEqual",
        ],
    ),
    (
        "slideNative.shapesFlow",
        &[
            "flowChartProcess",
            "flowChartAlternateProcess",
            "flowChartDecision",
            "flowChartInputOutput",
            "flowChartPredefinedProcess",
            "flowChartInternalStorage",
            "flowChartDocument",
            "flowChartMultidocument",
            "flowChartTerminator",
            "flowChartPreparation",
            "flowChartManualInput",
            "flowChartManualOperation",
            "flowChartConnector",
            "flowChartOffpageConnector",
            "flowChartPunchedCard",
            "flowChartPunchedTape",
            "flowChartSummingJunction",
            "flowChartOr",
            "flowChartCollate",
            "flowChartSort",
            "flowChartExtract",
            "flowChartMerge",
            "flowChartOnlineStorage",
            "flowChartDelay",
            "flowChartMagneticTape",
            "flowChartMagneticDisk",
            "flowChartMagneticDrum",
            "flowChartDisplay",
        ],
    ),
    (
        "slideNative.shapesStars",
        &[
            "irregularSeal1",
            "irregularSeal2",
            "star4",
            "star5",
            "star6",
            "star7",
            "star8",
            "star10",
            "star12",
            "star16",
            "star24",
            "star32",
            "ribbon2",
            "ribbon",
            "ellipseRibbon2",
            "ellipseRibbon",
            "verticalScroll",
            "horizontalScroll",
            "wave",
            "doubleWave",
        ],
    ),
    (
        "slideNative.shapesCallouts",
        &[
            "wedgeRectCallout",
            "wedgeRoundRectCallout",
            "wedgeEllipseCallout",
            "cloudCallout",
            "borderCallout1",
            "borderCallout2",
            "borderCallout3",
            "accentCallout1",
            "callout1",
            "callout2",
        ],
    ),
];

/// Symbols of the Symbols picker (the reference's recently used and common ones).
pub const SYMBOLS: [&str; 48] = [
    "©", "®", "™", "§", "¶", "†", "‡", "•", "…", "–", "—", "‰", "€", "£", "¥", "¢", "°", "±", "×",
    "÷", "≠", "≈", "≤", "≥", "∞", "√", "∑", "∏", "∫", "∂", "∆", "π", "α", "β", "γ", "δ", "λ", "μ",
    "Ω", "←", "↑", "→", "↓", "↔", "✓", "✗", "★", "♥",
];

/// Standard colours of the palettes (the reference's row under the theme colours).
pub const STANDARD: [u32; 10] = [
    0xC0_0000, 0xFF_0000, 0xFF_C000, 0xFF_FF00, 0x92_D050, 0x00_B050, 0x00_B0F0, 0x00_70C0,
    0x00_2060, 0x70_30A0,
];

/// One choosable cell.
#[derive(Debug, Clone)]
struct Cell {
    r: [f32; 4],
    /// What choosing it runs (`table:3x2`, `shape:rect`, `fill:FF0000`, …).
    value: String,
    /// A wide row with a label instead of a picture ("No Fill").
    label: Option<String>,
}

/// One shape of an icon: segments, fill, stroke (colour, width in grid units).
pub type SvgPart = (Vec<PathSeg>, Option<Rgba>, Option<(Rgba, f32)>);

/// An icon of the library, its drawing in the 24-unit grid.
#[derive(Debug, Clone)]
pub struct LibIcon {
    /// Library id.
    pub id: String,
    /// SVG source.
    pub svg: String,
    /// Paths to draw: segments, fill, stroke (colour, width in grid units).
    pub parts: Vec<SvgPart>,
}

/// An open picker anchored under a ribbon control.
pub struct Picker {
    /// What it offers.
    pub kind: PickKind,
    frame: [f32; 4],
    cells: Vec<Cell>,
    /// Section headings (text, rectangle).
    heads: Vec<(String, [f32; 4])>,
    hover: Option<usize>,
    scroll: f32,
    content_h: f32,
    scale: f32,
    /// Heading of the table grid ("Insert Table", "3x2 Table").
    title: String,
    /// Colours of the quick styles / theme palette (accents 1–6, then dark 1, light 1).
    accents: Vec<Rgba>,
    /// `{{c}}x{{r}} Table` pattern.
    size_label: String,
}

/// Table grid size of the reference's Insert Table picker.
pub const TABLE_COLS: usize = 10;
/// Rows of the grid.
pub const TABLE_ROWS: usize = 8;

fn px(v: f32, scale: f32) -> f32 {
    (v * scale).round()
}

/// `RRGGBB` of a colour.
pub fn hex(c: Rgba) -> String {
    format!("{:02X}{:02X}{:02X}", c.0, c.1, c.2)
}

/// A colour from `0xRRGGBB`.
pub fn rgb(v: u32) -> Rgba {
    Rgba((v >> 16) as u8, (v >> 8) as u8, v as u8, 255)
}

/// Mix `c` with white (`t` > 0) or black (`t` < 0), as theme tints and shades.
pub fn tint(c: Rgba, t: f32) -> Rgba {
    let f = |v: u8| {
        let v = f32::from(v);
        (if t >= 0.0 {
            v + (255.0 - v) * t
        } else {
            v * (1.0 + t)
        })
        .round() as u8
    };
    Rgba(f(c.0), f(c.1), f(c.2), 255)
}

/// Tints / shades of the theme palette rows (the reference: light 80/60/40 %, dark 25/50 %).
const ROW_TINTS: [f32; 5] = [0.8, 0.6, 0.4, -0.25, -0.5];

impl Picker {
    /// Open a picker of `kind` under `anchor` inside a window of `size`.
    ///
    /// `labels`: headings (kind-specific: table title and size pattern; shape section names;
    /// "No Fill" / "No Outline" and palette headings), `accents`: the theme's accent colours
    /// then dark 1 and light 1, `icons`: the icon library (icon picker only).
    #[allow(clippy::too_many_lines)] // one layout per kind
    pub fn open(
        kind: PickKind,
        anchor: [f32; 4],
        size: (f32, f32),
        scale: f32,
        labels: &[String],
        accents: &[Rgba],
        icons: &[LibIcon],
    ) -> Self {
        let pad = px(6.0, scale);
        let label = |i: usize| labels.get(i).cloned().unwrap_or_default();
        let mut cells = Vec::new();
        let mut heads = Vec::new();
        let (w, content_h) = match kind {
            PickKind::Table => {
                // Reference at 125 %: 10 × 8 squares of 19 px on a 21 px pitch under the
                // heading.
                let pitch = px(16.8, scale);
                let head = px(28.0, scale);
                for r in 0..TABLE_ROWS {
                    for c in 0..TABLE_COLS {
                        cells.push(Cell {
                            r: [
                                pad + c as f32 * pitch,
                                head + r as f32 * pitch,
                                pitch - px(2.0, scale),
                                pitch - px(2.0, scale),
                            ],
                            value: format!("table:{}x{}", c + 1, r + 1),
                            label: None,
                        });
                    }
                }
                (
                    2.0 * pad + TABLE_COLS as f32 * pitch,
                    head + TABLE_ROWS as f32 * pitch + pad,
                )
            }
            PickKind::Shapes | PickKind::Icons | PickKind::Symbols => {
                let (cols, cell) = match kind {
                    PickKind::Shapes => (12, px(22.4, scale)),
                    PickKind::Icons => (8, px(40.0, scale)),
                    _ => (12, px(28.0, scale)),
                };
                let sections: Vec<(String, Vec<String>)> = match kind {
                    PickKind::Shapes => SHAPE_SECTIONS
                        .iter()
                        .enumerate()
                        .map(|(i, (_, presets))| {
                            (
                                label(i),
                                presets
                                    .iter()
                                    .filter(|p| bo_drawingml::preset(p).is_some())
                                    .map(|p| format!("shape:{p}"))
                                    .collect(),
                            )
                        })
                        .collect(),
                    PickKind::Icons => vec![(
                        label(0),
                        icons.iter().map(|i| format!("icon:{}", i.id)).collect(),
                    )],
                    _ => vec![(
                        label(0),
                        SYMBOLS.iter().map(|s| format!("sym:{s}")).collect(),
                    )],
                };
                let head_h = px(24.0, scale);
                let mut y = 0.0;
                for (name, values) in sections {
                    if values.is_empty() {
                        continue;
                    }
                    heads.push((name, [0.0, y, 0.0, head_h]));
                    y += head_h + px(2.0, scale);
                    for (i, value) in values.into_iter().enumerate() {
                        let (c, r) = (i % cols, i / cols);
                        cells.push(Cell {
                            r: [pad + c as f32 * cell, y + r as f32 * cell, cell, cell],
                            value,
                            label: None,
                        });
                    }
                    let rows = cells.last().map_or(0.0, |c| c.r[1] + c.r[3] - y);
                    y += rows + px(4.0, scale);
                }
                (2.0 * pad + cols as f32 * cell, y + pad)
            }
            PickKind::Styles => {
                // Six theme accents across, four styles down (outline, solid, light fill
                // with accent outline, intense), as the reference's quick styles.
                let (cw, ch) = (px(60.0, scale), px(44.0, scale));
                let head = px(24.0, scale);
                heads.push((label(0), [0.0, 0.0, 0.0, head]));
                for r in 0..4 {
                    for c in 0..6 {
                        cells.push(Cell {
                            r: [pad + c as f32 * cw, head + r as f32 * ch, cw, ch],
                            value: format!("style:{c}:{r}"),
                            label: None,
                        });
                    }
                }
                (2.0 * pad + 6.0 * cw, head + 4.0 * ch + pad)
            }
            PickKind::Effects => {
                // Section headings, then labelled tiles of 5 across (labels: the section
                // headings, then each effect's name in section order).
                let (cw, ch) = (px(99.2, scale), px(67.2, scale));
                let head_h = px(24.0, scale);
                let mut y = 0.0;
                let mut n = EFFECT_SECTIONS.len();
                for (si, (_, class, ids)) in EFFECT_SECTIONS.iter().enumerate() {
                    heads.push((label(si), [0.0, y, 0.0, head_h]));
                    y += head_h + px(2.0, scale);
                    for (i, (id, _)) in ids.iter().enumerate() {
                        let (c, r) = (i % EFFECT_COLS, i / EFFECT_COLS);
                        cells.push(Cell {
                            r: [pad + c as f32 * cw, y + r as f32 * ch, cw, ch],
                            value: format!("afx:{id}:{class}"),
                            label: Some(label(n)),
                        });
                        n += 1;
                    }
                    let rows = ids.len().div_ceil(EFFECT_COLS);
                    y += rows as f32 * ch + px(4.0, scale);
                }
                // The More … Effects rows (labels after the effects' names).
                let rh = px(28.8, scale);
                let w = 2.0 * pad + EFFECT_COLS as f32 * cw;
                for (_, class) in EFFECT_MORE {
                    cells.push(Cell {
                        r: [pad, y, w - 2.0 * pad, rh],
                        value: format!("afxmore:{class}"),
                        label: Some(label(n)),
                    });
                    n += 1;
                    y += rh;
                }
                (w, y + pad)
            }
            PickKind::Fill | PickKind::Outline => {
                // Theme colours: dark 1, light 1, six accents; tints / shades under them;
                // standard colours; "No Fill" (reference layout at 125 %: 10 columns).
                let sw = px(17.6, scale);
                let gap = px(4.0, scale);
                let head = px(24.0, scale);
                let mut y = 0.0;
                heads.push((label(1), [0.0, y, 0.0, head]));
                y += head;
                let base: Vec<Rgba> = {
                    let dark = accents.get(6).copied().unwrap_or(Rgba(0, 0, 0, 255));
                    let light = accents.get(7).copied().unwrap_or(Rgba(255, 255, 255, 255));
                    let mut v = vec![light, dark];
                    v.extend(accents.iter().take(6).copied());
                    v.push(rgb(0xA5_A5A5));
                    v.push(rgb(0x44_546A));
                    v
                };
                for (c, col) in base.iter().enumerate() {
                    cells.push(Cell {
                        r: [pad + c as f32 * (sw + gap), y, sw, sw],
                        value: format!("color:{}", hex(*col)),
                        label: None,
                    });
                }
                y += sw + px(4.0, scale);
                for t in ROW_TINTS {
                    for (c, col) in base.iter().enumerate() {
                        let v = if *col == Rgba(255, 255, 255, 255) {
                            tint(*col, -t.abs() * 0.6)
                        } else {
                            tint(*col, t)
                        };
                        cells.push(Cell {
                            r: [pad + c as f32 * (sw + gap), y, sw, sw],
                            value: format!("color:{}", hex(v)),
                            label: None,
                        });
                    }
                    y += sw;
                }
                y += px(6.0, scale);
                heads.push((label(2), [0.0, y, 0.0, head]));
                y += head;
                for (c, v) in STANDARD.iter().enumerate() {
                    cells.push(Cell {
                        r: [pad + c as f32 * (sw + gap), y, sw, sw],
                        value: format!("color:{}", hex(rgb(*v))),
                        label: None,
                    });
                }
                y += sw + px(8.0, scale);
                let w = 2.0 * pad + 10.0 * (sw + gap) - gap;
                cells.push(Cell {
                    r: [pad, y, w - 2.0 * pad, px(26.0, scale)],
                    value: "color:".into(),
                    label: Some(label(0)),
                });
                y += px(26.0, scale);
                (w, y + pad)
            }
        };
        let w = w.max(px(120.0, scale));
        let margin = px(4.0, scale);
        let x = anchor[0].clamp(margin, (size.0 - w - margin).max(margin));
        let y = anchor[1] + anchor[3] + px(2.0, scale);
        let h = content_h.min((size.1 - y - margin).max(px(80.0, scale)));
        for (_, r) in &mut heads {
            r[2] = w;
        }
        Self {
            kind,
            frame: [x.round(), y.round(), w.round(), h.round()],
            cells,
            heads,
            hover: None,
            scroll: 0.0,
            content_h,
            scale,
            title: label(0),
            accents: accents.to_vec(),
            size_label: label(1),
        }
    }

    /// Whether a window position is over the picker.
    pub fn contains(&self, x: f32, y: f32) -> bool {
        crate::spin_box::inside(self.frame, x, y)
    }

    fn abs(&self, r: [f32; 4]) -> [f32; 4] {
        [
            self.frame[0] + r[0],
            self.frame[1] + r[1] - self.scroll,
            r[2],
            r[3],
        ]
    }

    fn index_at(&self, x: f32, y: f32) -> Option<usize> {
        if !self.contains(x, y) {
            return None;
        }
        self.cells
            .iter()
            .position(|c| crate::spin_box::inside(self.abs(c.r), x, y))
    }

    /// Pointer moved: `true` when the highlighted cell changed.
    pub fn hover(&mut self, x: f32, y: f32) -> bool {
        let h = self.index_at(x, y);
        std::mem::replace(&mut self.hover, h) != h
    }

    /// Press: the value of the cell under the pointer.
    pub fn press(&self, x: f32, y: f32) -> Option<String> {
        self.index_at(x, y).map(|i| self.cells[i].value.clone())
    }

    /// Scroll a long picker (icons) by `dy` device px.
    pub fn wheel(&mut self, dy: f32) -> bool {
        let max = (self.content_h - self.frame[3]).max(0.0);
        let before = self.scroll;
        self.scroll = (self.scroll + dy).clamp(0.0, max).round();
        before != self.scroll
    }

    /// Window position of the centre of the cell offering `value` (automation, tests).
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn cell_center(&self, value: &str) -> Option<(f32, f32)> {
        let c = self.cells.iter().find(|c| c.value == value)?;
        let [x, y, w, h] = self.abs(c.r);
        Some((x + w / 2.0, y + h / 2.0))
    }

    /// The highlighted table size (columns, rows).
    pub fn table_size(&self) -> Option<(usize, usize)> {
        let i = self.hover.filter(|_| self.kind == PickKind::Table)?;
        Some((i % TABLE_COLS + 1, i / TABLE_COLS + 1))
    }

    /// Paint the picker: a white panel with a grey frame and a soft shadow.
    #[allow(clippy::too_many_lines)] // one look per kind
    pub fn paint(&self, p: &mut dyn Painter, t: &Theme, icons: &[LibIcon]) {
        let text = estimate;
        let s = self.scale;
        let [x, y, w, h] = self.frame;
        for (k, a) in [(3.0, 18u8), (2.0, 26), (1.0, 34)] {
            let o = (k * s).round();
            p.fill_rect(
                x - o + 2.0 * s,
                y - o + 3.0 * s,
                w + 2.0 * o,
                h + 2.0 * o,
                Rgba(0, 0, 0, a),
            );
        }
        p.fill_rect(x, y, w, h, Rgba(0xc6, 0xc6, 0xc6, 255));
        p.fill_rect(x + 1.0, y + 1.0, w - 2.0, h - 2.0, Rgba(255, 255, 255, 255));
        p.push_clip(x + 1.0, y + 1.0, w - 2.0, h - 2.0);
        let style = TextStyle {
            size: px(12.0, s),
            bold: false,
        };
        let bold = TextStyle {
            bold: true,
            ..style
        };
        let line = style.size * 1.33;
        if self.kind == PickKind::Table {
            let title = match self.table_size() {
                Some((c, r)) => self
                    .size_label
                    .replace("{{c}}", &c.to_string())
                    .replace("{{r}}", &r.to_string()),
                None => self.title.clone(),
            };
            // Heading band (reference: light grey 243 with bold text).
            p.fill_rect(
                x + 1.0,
                y + 1.0,
                w - 2.0,
                px(24.0, s),
                Rgba(0xf3, 0xf3, 0xf3, 255),
            );
            p.text(
                &title,
                x + px(8.0, s),
                y + 1.0 + (px(24.0, s) - line) / 2.0,
                bold,
                t.text,
            );
            let hc = self.table_size();
            for (i, c) in self.cells.iter().enumerate() {
                let [cx, cy, cw, ch] = self.abs(c.r);
                let on = hc.is_some_and(|(hc, hr)| i % TABLE_COLS < hc && i / TABLE_COLS < hr);
                let (edge, fill) = if on {
                    (Rgba(0xd2, 0x47, 0x26, 255), Rgba(0xfc, 0xe4, 0xd6, 255))
                } else {
                    (Rgba(0x9a, 0x9a, 0x9a, 255), Rgba(255, 255, 255, 255))
                };
                p.fill_rect(cx, cy, cw, ch, edge);
                p.fill_rect(cx + 1.0, cy + 1.0, cw - 2.0, ch - 2.0, fill);
            }
            p.pop_clip();
            return;
        }
        for (name, r) in &self.heads {
            let [hx, hy, hw, hh] = self.abs(*r);
            p.fill_rect(hx + 1.0, hy, hw - 2.0, hh, Rgba(0xf3, 0xf3, 0xf3, 255));
            p.text(name, hx + px(8.0, s), hy + (hh - line) / 2.0, bold, t.text);
        }
        let ink = Rgba(0x40, 0x40, 0x40, 255);
        for (i, c) in self.cells.iter().enumerate() {
            let [cx, cy, cw, ch] = self.abs(c.r);
            if cy + ch < y || cy > y + h {
                continue;
            }
            let hovered = self.hover == Some(i);
            if hovered {
                p.fill_rect(cx, cy, cw, ch, Rgba(0xd2, 0x47, 0x26, 255));
                p.fill_rect(
                    cx + 1.0,
                    cy + 1.0,
                    cw - 2.0,
                    ch - 2.0,
                    Rgba(0xfc, 0xe4, 0xd6, 255),
                );
            }
            if let (Some(label), Some(more)) = (&c.label, c.value.strip_prefix("afxmore:")) {
                // A small star in the class colour (an outline for paths), then the name;
                // OLE Action Verbs greyed.
                let r = px(7.0, s);
                let (mx, my) = (cx + px(12.0, s), (cy + ch / 2.0).round());
                if !more.is_empty() {
                    crate::slide_ribbon::paint_star(p, mx, my, r, class_color(more), true);
                }
                p.text(
                    label,
                    cx + px(28.0, s),
                    cy + (ch - line) / 2.0,
                    style,
                    if more.is_empty() {
                        Rgba(0xa6, 0xa6, 0xa6, 255)
                    } else {
                        t.text
                    },
                );
                continue;
            }
            if let (Some(label), Some(fx)) = (&c.label, c.value.strip_prefix("afx:")) {
                let (id, class) = fx.split_once(':').unwrap_or((fx, "entr"));
                let r = px(17.0, s);
                let (mx, my) = ((cx + cw / 2.0).round(), cy + px(6.0, s) + r);
                if class == "path" {
                    paint_path_tile(p, id, mx, my, r);
                } else {
                    paint_effect_icon(p, id, class, mx, my, r);
                }
                let small = TextStyle {
                    size: px(12.0, s),
                    bold: false,
                };
                let tw = text(label, small).min(cw - px(4.0, s));
                p.push_clip(cx + 1.0, cy, cw - 2.0, ch);
                p.text(
                    label,
                    (cx + (cw - tw) / 2.0).round(),
                    cy + ch - small.size * 1.33 - px(4.0, s),
                    small,
                    t.text,
                );
                p.pop_clip();
                continue;
            }
            if let Some(label) = &c.label {
                p.text(
                    label,
                    cx + px(28.0, s),
                    cy + (ch - line) / 2.0,
                    style,
                    t.text,
                );
                continue;
            }
            let (kind, v) = c.value.split_once(':').unwrap_or(("", ""));
            match kind {
                "shape" => {
                    let k = (cw * 0.72).round();
                    paint_preset(p, v, cx + (cw - k) / 2.0, cy + (ch - k) / 2.0, k, k, ink);
                }
                "icon" => {
                    if let Some(icon) = icons.iter().find(|i| i.id == v) {
                        let k = (cw * 0.7).round();
                        paint_icon(p, icon, cx + (cw - k) / 2.0, cy + (ch - k) / 2.0, k);
                    }
                }
                "sym" => {
                    let big = TextStyle {
                        size: px(16.0, s),
                        bold: false,
                    };
                    let tw = text(v, big);
                    p.text(
                        v,
                        (cx + (cw - tw) / 2.0).round(),
                        cy + (ch - big.size * 1.33) / 2.0,
                        big,
                        t.text,
                    );
                }
                "style" => {
                    let mut it = v.split(':').filter_map(|n| n.parse::<usize>().ok());
                    let (a, r) = (it.next().unwrap_or(0), it.next().unwrap_or(0));
                    let (fill, line_c, ink2) = quick_style(&self.accents, a, r);
                    let m = px(6.0, s);
                    let (bx, by, bw, bh) = (cx + m, cy + m, cw - 2.0 * m, ch - 2.0 * m);
                    p.fill_rect(bx, by, bw, bh, line_c);
                    p.fill_rect(bx + 1.0, by + 1.0, bw - 2.0, bh - 2.0, fill);
                    let tw = text("Abc", style);
                    p.text(
                        "Abc",
                        (bx + (bw - tw) / 2.0).round(),
                        by + (bh - line) / 2.0,
                        style,
                        ink2,
                    );
                }
                "color" => {
                    if let Some(col) = u32::from_str_radix(v, 16).ok().map(rgb) {
                        let edge = if hovered {
                            Rgba(0xd2, 0x47, 0x26, 255)
                        } else {
                            Rgba(0xd0, 0xd0, 0xd0, 255)
                        };
                        p.fill_rect(cx, cy, cw, ch, edge);
                        p.fill_rect(cx + 1.0, cy + 1.0, cw - 2.0, ch - 2.0, col);
                    }
                }
                _ => {}
            }
        }
        p.pop_clip();
    }
}

/// Fill, outline and text colour of quick style (`accent`, `row`): row 0 colored outline
/// on white, 1 colored fill, 2 light fill with colored outline, 3 intense (darker fill).
pub fn quick_style(accents: &[Rgba], accent: usize, row: usize) -> (Rgba, Rgba, Rgba) {
    let a = accents
        .get(accent)
        .copied()
        .unwrap_or(Rgba(0x44, 0x72, 0xc4, 255));
    let white = Rgba(255, 255, 255, 255);
    match row {
        0 => (white, a, Rgba(0, 0, 0, 255)),
        1 => (a, tint(a, -0.25), white),
        2 => (tint(a, 0.8), a, Rgba(0, 0, 0, 255)),
        _ => (tint(a, -0.25), tint(a, -0.5), white),
    }
}

fn segs_of(path: &[bo_drawingml::Seg], x: f32, y: f32, k: f32) -> Vec<PathSeg> {
    use bo_drawingml::Seg;
    let f = |v: f64| v as f32 * k;
    path.iter()
        .map(|s| match *s {
            Seg::Move(a, b) => PathSeg::M(x + f(a), y + f(b)),
            Seg::Line(a, b) => PathSeg::L(x + f(a), y + f(b)),
            Seg::Quad(a, b, c, d) => PathSeg::Q(x + f(a), y + f(b), x + f(c), y + f(d)),
            Seg::Cubic(a, b, c, d, e, g) => {
                PathSeg::C(x + f(a), y + f(b), x + f(c), y + f(d), x + f(e), y + f(g))
            }
            Seg::Close => PathSeg::Z,
        })
        .collect()
}

/// Draw preset `name` as an outline in `[x, y, w, h]` (gallery pictures, own rendering of
/// the ECMA-376 §20.1.9.18 presets).
pub fn paint_preset(p: &mut dyn Painter, name: &str, x: f32, y: f32, w: f32, h: f32, ink: Rgba) {
    // Evaluate in a 100-unit box so adjust values keep their proportions.
    let Some(Ok(e)) = bo_drawingml::geom::eval_preset(name, &[], 100.0, 100.0) else {
        return;
    };
    let k = w.min(h) / 100.0;
    for path in &e.paths {
        if !path.stroke && path.fill == bo_drawingml::FillMode::None {
            continue;
        }
        let segs = segs_of(&path.segs, x, y, k);
        if path.stroke {
            p.path(&segs, None, Some((ink, 1.0)));
        }
    }
}

/// Draw a library icon in a `size` square at (`x`, `y`).
pub fn paint_icon(p: &mut dyn Painter, icon: &LibIcon, x: f32, y: f32, size: f32) {
    let k = size / 24.0;
    for (segs, fill, stroke) in &icon.parts {
        let moved: Vec<PathSeg> = segs
            .iter()
            .map(|s| match *s {
                PathSeg::M(a, b) => PathSeg::M(x + a * k, y + b * k),
                PathSeg::L(a, b) => PathSeg::L(x + a * k, y + b * k),
                PathSeg::Q(a, b, c, d) => PathSeg::Q(x + a * k, y + b * k, x + c * k, y + d * k),
                PathSeg::C(a, b, c, d, e, f) => PathSeg::C(
                    x + a * k,
                    y + b * k,
                    x + c * k,
                    y + d * k,
                    x + e * k,
                    y + f * k,
                ),
                PathSeg::Z => PathSeg::Z,
            })
            .collect();
        p.path(&moved, *fill, stroke.map(|(c, w)| (c, (w * k).max(1.0))));
    }
}

/// The icon library (`packages/ui/src/iconLibrary/data.json`: our own drawings, shared with
/// the web editor), parsed once for the picker.
pub fn load_library() -> Vec<LibIcon> {
    const DATA: &str = include_str!("../../../packages/ui/src/iconLibrary/data.json");
    let Ok(v) = serde_json::from_str::<serde_json::Value>(DATA) else {
        return Vec::new();
    };
    let order: Vec<String> = v
        .pointer("/manifest/icons")
        .and_then(|i| i.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|i| i.get("id").and_then(|s| s.as_str()).map(str::to_owned))
                .collect()
        })
        .unwrap_or_default();
    let Some(svg) = v.get("svg").and_then(|s| s.as_object()) else {
        return Vec::new();
    };
    order
        .into_iter()
        .filter_map(|id| {
            let src = svg.get(&id)?.as_str()?.to_owned();
            let parts = parse_svg(&src);
            Some(LibIcon {
                id,
                svg: src,
                parts,
            })
        })
        .collect()
}

fn attr<'a>(el: &'a str, name: &str) -> Option<&'a str> {
    let key = format!(" {name}=\"");
    let i = el.find(&key)? + key.len();
    let j = el[i..].find('"')? + i;
    Some(&el[i..j])
}

fn color_attr(el: &str, name: &str) -> Option<Rgba> {
    let v = attr(el, name)?;
    let h = v.strip_prefix('#')?;
    u32::from_str_radix(h, 16).ok().map(rgb)
}

fn num(el: &str, name: &str) -> f32 {
    attr(el, name).and_then(|v| v.parse().ok()).unwrap_or(0.0)
}

fn ellipse_segs(cx: f32, cy: f32, rx: f32, ry: f32) -> Vec<PathSeg> {
    // Four cubic quarter arcs (kappa 0.5523).
    let (kx, ky) = (rx * 0.552_3, ry * 0.552_3);
    vec![
        PathSeg::M(cx + rx, cy),
        PathSeg::C(cx + rx, cy + ky, cx + kx, cy + ry, cx, cy + ry),
        PathSeg::C(cx - kx, cy + ry, cx - rx, cy + ky, cx - rx, cy),
        PathSeg::C(cx - rx, cy - ky, cx - kx, cy - ry, cx, cy - ry),
        PathSeg::C(cx + kx, cy - ry, cx + rx, cy - ky, cx + rx, cy),
        PathSeg::Z,
    ]
}

/// Shapes of a library SVG (`path`, `circle`, `ellipse`, `rect` with fill / stroke).
pub fn parse_svg(src: &str) -> Vec<SvgPart> {
    let mut out = Vec::new();
    for chunk in src.split('<').skip(1) {
        let name: String = chunk
            .chars()
            .take_while(char::is_ascii_alphabetic)
            .collect();
        let el = chunk.split('>').next().unwrap_or("");
        let segs = match name.as_str() {
            "path" => attr(el, "d").map(crate::icons::parse_path),
            "circle" => {
                let r = num(el, "r");
                Some(ellipse_segs(num(el, "cx"), num(el, "cy"), r, r))
            }
            "ellipse" => Some(ellipse_segs(
                num(el, "cx"),
                num(el, "cy"),
                num(el, "rx"),
                num(el, "ry"),
            )),
            "rect" => {
                let (x, y, w, h) = (
                    num(el, "x"),
                    num(el, "y"),
                    num(el, "width"),
                    num(el, "height"),
                );
                Some(vec![
                    PathSeg::M(x, y),
                    PathSeg::L(x + w, y),
                    PathSeg::L(x + w, y + h),
                    PathSeg::L(x, y + h),
                    PathSeg::Z,
                ])
            }
            _ => None,
        };
        let Some(segs) = segs else { continue };
        let fill = color_attr(el, "fill");
        let stroke = color_attr(el, "stroke").map(|c| {
            let w = attr(el, "stroke-width")
                .and_then(|v| v.parse().ok())
                .unwrap_or(1.0);
            (c, w)
        });
        out.push((segs, fill, stroke));
    }
    out
}

/// Width of interface text estimated from its size (pickers centre short labels).
pub fn estimate(s: &str, style: TextStyle) -> f32 {
    s.chars().count() as f32 * style.size * 0.58
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_grid_reports_the_hovered_size() {
        let mut p = Picker::open(
            PickKind::Table,
            [100.0, 50.0, 40.0, 60.0],
            (1500.0, 1000.0),
            1.25,
            &["Insert Table".into(), "{{c}}x{{r}} Table".into()],
            &[],
            &[],
        );
        assert_eq!(p.cells.len(), TABLE_COLS * TABLE_ROWS);
        // Third column, second row.
        let c = &p.cells[TABLE_COLS + 2];
        let [x, y, w, h] = p.abs(c.r);
        p.hover(x + w / 2.0, y + h / 2.0);
        assert_eq!(p.table_size(), Some((3, 2)));
        assert_eq!(
            p.press(x + w / 2.0, y + h / 2.0).as_deref(),
            Some("table:3x2")
        );
    }

    #[test]
    fn shape_gallery_offers_known_presets_only() {
        let p = Picker::open(
            PickKind::Shapes,
            [0.0, 0.0, 10.0, 10.0],
            (1500.0, 2000.0),
            1.0,
            &[],
            &[],
            &[],
        );
        assert!(p.cells.iter().any(|c| c.value == "shape:rect"));
        assert!(p.cells.iter().any(|c| c.value == "shape:star5"));
        assert!(p
            .cells
            .iter()
            .all(|c| bo_drawingml::preset(c.value.trim_start_matches("shape:")).is_some()));
    }

    #[test]
    fn icon_library_parses() {
        let lib = load_library();
        assert!(lib.len() > 100);
        assert!(
            lib.iter().all(|i| !i.parts.is_empty()),
            "every icon draws something"
        );
    }

    #[test]
    fn palette_has_theme_standard_and_none() {
        let p = Picker::open(
            PickKind::Fill,
            [0.0, 0.0, 10.0, 10.0],
            (1500.0, 2000.0),
            1.0,
            &[
                "No Fill".into(),
                "Theme Colors".into(),
                "Standard Colors".into(),
            ],
            &[rgb(0x44_72C4); 8],
            &[],
        );
        assert_eq!(p.cells.len(), 10 * 6 + 10 + 1);
        assert!(p.cells.iter().any(|c| c.value == "color:FF0000"));
        assert_eq!(p.cells.last().map(|c| c.value.as_str()), Some("color:"));
    }
}
