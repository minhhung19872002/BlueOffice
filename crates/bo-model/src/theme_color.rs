//! Theme colours of runs (ECMA-376 Part 1, §17.3.2.6 color: `w:themeColor`,
//! `w:themeTint`, `w:themeShade`) resolved against the document theme's colour
//! scheme (DrawingML §20.1.6.2 clrScheme) — so a theme change recolours the text.

use crate::write::Color;
use serde::{Deserialize, Serialize};

/// A run colour taken from the theme (§17.3.2.6 color attributes).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeColorRef {
    /// `w:themeColor` (§17.18.97 `ST_ThemeColor`: `accent1`, `text1`, `dark2`, …).
    pub name: String,
    /// `w:themeTint` (§17.18.38 `ST_UcharHexNumber`): lighten towards white.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tint: Option<u8>,
    /// `w:themeShade`: darken towards black.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shade: Option<u8>,
}

/// The colour scheme slot (§20.1.6.2: `dk1` … `folHlink`) of an `ST_ThemeColor` value,
/// with the default colour scheme mapping (§17.15.1.20 clrSchemeMapping:
/// background1 → light1, text1 → dark1, background2 → light2, text2 → dark2).
#[must_use]
pub fn scheme_slot(name: &str) -> Option<&'static str> {
    Some(match name {
        "dark1" | "text1" => "dk1",
        "light1" | "background1" => "lt1",
        "dark2" | "text2" => "dk2",
        "light2" | "background2" => "lt2",
        "accent1" => "accent1",
        "accent2" => "accent2",
        "accent3" => "accent3",
        "accent4" => "accent4",
        "accent5" => "accent5",
        "accent6" => "accent6",
        "hyperlink" => "hlink",
        "followedHyperlink" => "folHlink",
        _ => return None,
    })
}

impl ThemeColorRef {
    /// The colour in a theme whose scheme is `slots` (`(slot, sRGB)` pairs), `None`
    /// when the slot is missing (`none` included).
    #[must_use]
    pub fn resolve(&self, slots: &[(String, Color)]) -> Option<Color> {
        let slot = scheme_slot(&self.name)?;
        let base = slots.iter().find(|(k, _)| k == slot)?.1;
        Some(self.apply(base))
    }

    /// `base` with the tint / shade applied on its HSL luminance: a tint `t` (of 255)
    /// gives L·t + (1 − t), a shade `s` gives L·s.
    #[must_use]
    pub fn apply(&self, base: Color) -> Color {
        if self.tint.is_none() && self.shade.is_none() {
            return base;
        }
        let (h, s, mut l) = rgb_to_hsl(base);
        if let Some(t) = self.tint {
            let t = f64::from(t) / 255.0;
            l = l * t + (1.0 - t);
        }
        if let Some(sh) = self.shade {
            l *= f64::from(sh) / 255.0;
        }
        hsl_to_rgb(h, s, l.clamp(0.0, 1.0))
    }
}

fn rgb_to_hsl(c: Color) -> (f64, f64, f64) {
    let (r, g, b) = (
        f64::from(c.r) / 255.0,
        f64::from(c.g) / 255.0,
        f64::from(c.b) / 255.0,
    );
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = f64::midpoint(max, min);
    let d = max - min;
    if d.abs() < f64::EPSILON {
        return (0.0, 0.0, l);
    }
    let s = if l > 0.5 {
        d / (2.0 - max - min)
    } else {
        d / (max + min)
    };
    let h = if (max - r).abs() < f64::EPSILON {
        (g - b) / d + if g < b { 6.0 } else { 0.0 }
    } else if (max - g).abs() < f64::EPSILON {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    };
    (h / 6.0, s, l)
}

fn hsl_to_rgb(h: f64, s: f64, l: f64) -> Color {
    fn hue(p: f64, q: f64, mut t: f64) -> f64 {
        if t < 0.0 {
            t += 1.0;
        }
        if t > 1.0 {
            t -= 1.0;
        }
        if t < 1.0 / 6.0 {
            p + (q - p) * 6.0 * t
        } else if t < 0.5 {
            q
        } else if t < 2.0 / 3.0 {
            p + (q - p) * (2.0 / 3.0 - t) * 6.0
        } else {
            p
        }
    }
    let (r, g, b) = if s.abs() < f64::EPSILON {
        (l, l, l)
    } else {
        let q = if l < 0.5 {
            l * (1.0 + s)
        } else {
            l + s - l * s
        };
        let p = 2.0 * l - q;
        (
            hue(p, q, h + 1.0 / 3.0),
            hue(p, q, h),
            hue(p, q, h - 1.0 / 3.0),
        )
    };
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let byte = |v: f64| (v * 255.0).round().clamp(0.0, 255.0) as u8;
    Color {
        r: byte(r),
        g: byte(g),
        b: byte(b),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slots() -> Vec<(String, Color)> {
        vec![
            ("dk1".into(), Color { r: 0, g: 0, b: 0 }),
            ("lt1".into(), Color::WHITE),
            (
                "accent1".into(),
                Color {
                    r: 0x44,
                    g: 0x72,
                    b: 0xC4,
                },
            ),
        ]
    }

    #[test]
    fn slots_and_mapping() {
        let r = |n: &str| {
            ThemeColorRef {
                name: n.into(),
                ..ThemeColorRef::default()
            }
            .resolve(&slots())
        };
        assert_eq!(r("accent1"), Color::from_hex("4472C4"));
        assert_eq!(r("text1"), Some(Color::BLACK));
        assert_eq!(r("background1"), Some(Color::WHITE));
        assert_eq!(r("accent2"), None);
        assert_eq!(r("none"), None);
    }

    #[test]
    fn tint_and_shade_on_luminance() {
        // Black with a half tint: L = 0·0.5 + 0.5 → mid grey.
        let c = ThemeColorRef {
            name: "text1".into(),
            tint: Some(0x80),
            shade: None,
        };
        assert_eq!(c.resolve(&slots()), Color::from_hex("7F7F7F"));
        // White shaded to 0xBF (75 %): L = 0.749 → light grey.
        let c = ThemeColorRef {
            name: "background1".into(),
            tint: None,
            shade: Some(0xBF),
        };
        assert_eq!(c.resolve(&slots()), Color::from_hex("BFBFBF"));
        // accent1 shaded keeps its hue, gets darker.
        let c = ThemeColorRef {
            name: "accent1".into(),
            tint: None,
            shade: Some(0x80),
        }
        .resolve(&slots())
        .unwrap();
        assert!(c.b > c.r && u32::from(c.b) < 0xC4);
    }
}
