//! DrawingML colours — ECMA-376 Part 1, §20.1.2.3 (Color Models and Transforms).
//!
//! Reading/writing of the six colour models and resolution to RGBA, applying
//! the colour transforms in document order.

use crate::dom::{attr_int, El};
use bo_model::slide::{Color, ColorKind, ColorMod, Theme, XmlElement};

/// Resolved colour, components in 0..=1 (sRGB, non-premultiplied).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rgba {
    /// Red, 0..=1.
    pub r: f64,
    /// Green, 0..=1.
    pub g: f64,
    /// Blue, 0..=1.
    pub b: f64,
    /// Alpha (opacity), 0..=1.
    pub a: f64,
}

impl Rgba {
    /// From `0xRRGGBB`.
    pub fn from_hex(v: u32) -> Self {
        Rgba {
            r: f64::from((v >> 16) & 0xFF) / 255.0,
            g: f64::from((v >> 8) & 0xFF) / 255.0,
            b: f64::from(v & 0xFF) / 255.0,
            a: 1.0,
        }
    }
    /// 8-bit components `[r, g, b, a]`.
    pub fn to_u8(self) -> [u8; 4] {
        let q = |v: f64| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
        [q(self.r), q(self.g), q(self.b), q(self.a)]
    }
    /// `0xRRGGBB` (alpha dropped).
    pub fn to_hex(self) -> u32 {
        let [r, g, b, _] = self.to_u8();
        (u32::from(r) << 16) | (u32::from(g) << 8) | u32::from(b)
    }
}

/// Is `name` (qualified) one of the colour choice elements (EG_ColorChoice)?
pub fn is_color_element(name: &str) -> bool {
    matches!(
        name,
        "a:srgbClr" | "a:scrgbClr" | "a:hslClr" | "a:schemeClr" | "a:sysClr" | "a:prstClr"
    )
}

/// Parse a colour choice element.
pub fn parse_color(el: &XmlElement) -> Option<Color> {
    let kind = match el.name.as_str() {
        // §20.1.2.3.32 srgbClr
        "a:srgbClr" => ColorKind::Srgb(u32::from_str_radix(el.attr("val")?.trim(), 16).ok()?),
        // §20.1.2.3.30 scrgbClr
        "a:scrgbClr" => ColorKind::ScRgb(
            attr_int(el, "r").unwrap_or(0),
            attr_int(el, "g").unwrap_or(0),
            attr_int(el, "b").unwrap_or(0),
        ),
        // §20.1.2.3.13 hslClr
        "a:hslClr" => ColorKind::Hsl(
            attr_int(el, "hue").unwrap_or(0),
            attr_int(el, "sat").unwrap_or(0),
            attr_int(el, "lum").unwrap_or(0),
        ),
        // §20.1.2.3.29 schemeClr
        "a:schemeClr" => ColorKind::Scheme(el.attr("val")?.to_owned()),
        // §20.1.2.3.33 sysClr
        "a:sysClr" => ColorKind::System {
            val: el.attr("val")?.to_owned(),
            last: el
                .attr("lastClr")
                .and_then(|v| u32::from_str_radix(v.trim(), 16).ok()),
        },
        // §20.1.2.3.22 prstClr
        "a:prstClr" => ColorKind::Preset(el.attr("val")?.to_owned()),
        _ => return None,
    };
    let mods = el
        .elements()
        .map(|m| ColorMod {
            name: m.local().to_owned(),
            val: attr_int(m, "val"),
        })
        .collect();
    Some(Color { kind, mods })
}

/// First colour choice child of `parent`.
pub fn find_color(parent: &XmlElement) -> Option<Color> {
    parent
        .elements()
        .find(|e| is_color_element(&e.name))
        .and_then(parse_color)
}

/// Serialize a colour.
pub fn write_color(c: &Color) -> XmlElement {
    let el = match &c.kind {
        ColorKind::Srgb(v) => El::new("a:srgbClr").a("val", format!("{v:06X}")),
        ColorKind::ScRgb(r, g, b) => El::new("a:scrgbClr").a("r", r).a("g", g).a("b", b),
        ColorKind::Hsl(h, s, l) => El::new("a:hslClr").a("hue", h).a("sat", s).a("lum", l),
        ColorKind::Scheme(v) => El::new("a:schemeClr").a("val", v),
        ColorKind::System { val, last } => El::new("a:sysClr")
            .a("val", val)
            .opt("lastClr", last.map(|v| format!("{v:06X}"))),
        ColorKind::Preset(v) => El::new("a:prstClr").a("val", v),
    };
    let mut el = el;
    for m in &c.mods {
        el = el.child(El::new(&format!("a:{}", m.name)).opt("val", m.val).done());
    }
    el.done()
}

/// Context needed to resolve scheme colours.
#[derive(Debug, Clone, Copy, Default)]
pub struct ColorContext<'a> {
    /// Theme supplying the colour scheme, if any.
    pub theme: Option<&'a Theme>,
    /// §19.3.1.6 clrMap (`bg1`→`lt1`, `tx1`→`dk1`, …).
    pub clr_map: &'a [(String, String)],
    /// Placeholder colour (`phClr`) from a style reference.
    pub ph_clr: Option<Rgba>,
}

/// Resolve a colour to RGBA, applying its transforms.
pub fn resolve(c: &Color, ctx: &ColorContext<'_>) -> Rgba {
    resolve_depth(c, ctx, 0)
}

fn resolve_depth(c: &Color, ctx: &ColorContext<'_>, depth: u8) -> Rgba {
    let base = match &c.kind {
        ColorKind::Srgb(v) => Rgba::from_hex(*v),
        ColorKind::ScRgb(r, g, b) => Rgba {
            r: lin_to_srgb(f64::from(*r) / 100_000.0),
            g: lin_to_srgb(f64::from(*g) / 100_000.0),
            b: lin_to_srgb(f64::from(*b) / 100_000.0),
            a: 1.0,
        },
        ColorKind::Hsl(h, s, l) => {
            let (r, g, b) = hsl_to_rgb(
                f64::from(*h) / 60_000.0,
                f64::from(*s) / 100_000.0,
                f64::from(*l) / 100_000.0,
            );
            Rgba { r, g, b, a: 1.0 }
        }
        ColorKind::Scheme(name) => scheme(name, ctx, depth),
        ColorKind::System { val, last } => {
            Rgba::from_hex(last.unwrap_or_else(|| system_color(val)))
        }
        ColorKind::Preset(name) => Rgba::from_hex(preset_color(name).unwrap_or(0)),
    };
    apply_mods(base, &c.mods)
}

fn scheme(name: &str, ctx: &ColorContext<'_>, depth: u8) -> Rgba {
    if name == "phClr" {
        return ctx.ph_clr.unwrap_or(Rgba::from_hex(0));
    }
    // §19.3.1.6 clrMap maps the logical names bg1/tx1/bg2/tx2 to theme slots.
    let mapped = ctx
        .clr_map
        .iter()
        .find(|(k, _)| k == name)
        .map(|(_, v)| v.as_str());
    let slot = match (mapped, name) {
        (Some(m), _) => m,
        (None, "bg1") => "lt1",
        (None, "tx1") => "dk1",
        (None, "bg2") => "lt2",
        (None, "tx2") => "dk2",
        (None, n) => n,
    };
    match ctx.theme.and_then(|t| t.scheme_color(slot)) {
        Some(c) if depth < 4 => resolve_depth(c, ctx, depth + 1),
        _ => Rgba::from_hex(if slot.starts_with("lt") { 0xFF_FFFF } else { 0 }),
    }
}

/// Apply colour transforms (§20.1.2.3.*) in order.
pub fn apply_mods(mut c: Rgba, mods: &[ColorMod]) -> Rgba {
    for m in mods {
        let v = f64::from(m.val.unwrap_or(0)) / 100_000.0;
        match m.name.as_str() {
            // §20.1.2.3.1 alpha, .2 alphaMod, .3 alphaOff
            "alpha" => c.a = v,
            "alphaMod" => c.a *= v,
            "alphaOff" => c.a += v,
            // §20.1.2.3.19-21 lum, lumMod, lumOff; .27-29 sat*; .14-16 hue*
            "lum" | "lumMod" | "lumOff" | "sat" | "satMod" | "satOff" | "hue" | "hueMod"
            | "hueOff" => {
                let (mut h, mut s, mut l) = rgb_to_hsl(c.r, c.g, c.b);
                let deg = f64::from(m.val.unwrap_or(0)) / 60_000.0;
                match m.name.as_str() {
                    "lum" => l = v,
                    "lumMod" => l *= v,
                    "lumOff" => l += v,
                    "sat" => s = v,
                    "satMod" => s *= v,
                    "satOff" => s += v,
                    "hue" => h = deg,
                    "hueMod" => h *= v,
                    _ => h += deg,
                }
                let (r, g, b) =
                    hsl_to_rgb(h.rem_euclid(360.0), s.clamp(0.0, 1.0), l.clamp(0.0, 1.0));
                c = Rgba { r, g, b, a: c.a };
            }
            // §20.1.2.3.34 tint / §20.1.2.3.31 shade, computed in linear RGB.
            "tint" | "shade" => {
                let f = |x: f64| {
                    let lin = srgb_to_lin(x);
                    let out = if m.name == "tint" {
                        lin * v + (1.0 - v)
                    } else {
                        lin * v
                    };
                    lin_to_srgb(out.clamp(0.0, 1.0))
                };
                c = Rgba {
                    r: f(c.r),
                    g: f(c.g),
                    b: f(c.b),
                    a: c.a,
                };
            }
            // §20.1.2.3.7 comp: complement (hue + 180°).
            "comp" => {
                let (h, s, l) = rgb_to_hsl(c.r, c.g, c.b);
                let (r, g, b) = hsl_to_rgb((h + 180.0).rem_euclid(360.0), s, l);
                c = Rgba { r, g, b, a: c.a };
            }
            // §20.1.2.3.17 inv
            "inv" => {
                c = Rgba {
                    r: 1.0 - c.r,
                    g: 1.0 - c.g,
                    b: 1.0 - c.b,
                    a: c.a,
                };
            }
            // §20.1.2.3.9 gray
            "gray" => {
                let y = 0.3 * c.r + 0.59 * c.g + 0.11 * c.b;
                c = Rgba {
                    r: y,
                    g: y,
                    b: y,
                    a: c.a,
                };
            }
            // §20.1.2.3.8 gamma / §20.1.2.3.18 invGamma
            "gamma" => {
                c = Rgba {
                    r: lin_to_srgb(c.r),
                    g: lin_to_srgb(c.g),
                    b: lin_to_srgb(c.b),
                    a: c.a,
                };
            }
            "invGamma" => {
                c = Rgba {
                    r: srgb_to_lin(c.r),
                    g: srgb_to_lin(c.g),
                    b: srgb_to_lin(c.b),
                    a: c.a,
                };
            }
            // §20.1.2.3.23-25 red/redMod/redOff (and green/blue), on linear components.
            n @ ("red" | "redMod" | "redOff" | "green" | "greenMod" | "greenOff" | "blue"
            | "blueMod" | "blueOff") => {
                let comp = match &n[..1] {
                    "r" => &mut c.r,
                    "g" => &mut c.g,
                    _ => &mut c.b,
                };
                let lin = srgb_to_lin(*comp);
                let out = if n.ends_with("Mod") {
                    lin * v
                } else if n.ends_with("Off") {
                    lin + v
                } else {
                    v
                };
                *comp = lin_to_srgb(out.clamp(0.0, 1.0));
            }
            _ => {}
        }
        c.r = c.r.clamp(0.0, 1.0);
        c.g = c.g.clamp(0.0, 1.0);
        c.b = c.b.clamp(0.0, 1.0);
        c.a = c.a.clamp(0.0, 1.0);
    }
    c
}

/// sRGB → linear (IEC 61966-2-1).
pub fn srgb_to_lin(v: f64) -> f64 {
    if v <= 0.040_45 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

/// Linear → sRGB (IEC 61966-2-1).
pub fn lin_to_srgb(v: f64) -> f64 {
    if v <= 0.003_130_8 {
        v * 12.92
    } else {
        1.055 * v.powf(1.0 / 2.4) - 0.055
    }
}

/// RGB (0..1) → HSL (hue degrees, s, l in 0..1).
pub fn rgb_to_hsl(r: f64, g: f64, b: f64) -> (f64, f64, f64) {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = f64::midpoint(max, min);
    if (max - min).abs() < 1e-12 {
        return (0.0, 0.0, l);
    }
    let d = max - min;
    let s = if l > 0.5 {
        d / (2.0 - max - min)
    } else {
        d / (max + min)
    };
    let h = if max == r {
        (g - b) / d + if g < b { 6.0 } else { 0.0 }
    } else if max == g {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    };
    (h * 60.0, s, l)
}

/// HSL → RGB.
pub fn hsl_to_rgb(h: f64, s: f64, l: f64) -> (f64, f64, f64) {
    if s <= 0.0 {
        return (l, l, l);
    }
    let q = if l < 0.5 {
        l * (1.0 + s)
    } else {
        l + s - l * s
    };
    let p = 2.0 * l - q;
    let hk = h / 360.0;
    let t = |mut t: f64| {
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
    };
    (t(hk + 1.0 / 3.0), t(hk), t(hk - 1.0 / 3.0))
}

/// Default values of §20.1.10.58 ST_SystemColorVal when `lastClr` is missing.
fn system_color(val: &str) -> u32 {
    match val {
        "window" | "highlightText" | "btnHighlight" | "3dLight" | "menu" | "infoBk" => 0xFF_FFFF,
        "btnFace" | "menuBar" | "scrollBar" => 0xF0_F0F0,
        "btnShadow" | "grayText" | "3dDkShadow" => 0xA0_A0A0,
        "highlight" | "hotLight" | "activeCaption" => 0x00_78D7,
        _ => 0,
    }
}

/// §20.1.10.47 ST_PresetColorVal (subset; values per the named-colour table).
pub fn preset_color(name: &str) -> Option<u32> {
    Some(match name {
        "black" => 0x00_0000,
        "white" => 0xFF_FFFF,
        "red" => 0xFF_0000,
        "green" => 0x00_8000,
        "lime" => 0x00_FF00,
        "blue" => 0x00_00FF,
        "yellow" => 0xFF_FF00,
        "cyan" | "aqua" => 0x00_FFFF,
        "magenta" | "fuchsia" => 0xFF_00FF,
        "gray" | "grey" => 0x80_8080,
        "silver" => 0xC0_C0C0,
        "maroon" => 0x80_0000,
        "olive" => 0x80_8000,
        "navy" => 0x00_0080,
        "purple" => 0x80_0080,
        "teal" => 0x00_8080,
        "orange" => 0xFF_A500,
        "pink" => 0xFF_C0CB,
        "brown" => 0xA5_2A2A,
        "gold" => 0xFF_D700,
        "darkGray" | "darkGrey" | "dkGray" | "dkGrey" => 0xA9_A9A9,
        "lightGray" | "lightGrey" | "ltGray" | "ltGrey" => 0xD3_D3D3,
        "darkBlue" | "dkBlue" => 0x00_008B,
        "darkRed" | "dkRed" => 0x8B_0000,
        "darkGreen" | "dkGreen" => 0x00_6400,
        "lightBlue" | "ltBlue" => 0xAD_D8E6,
        "lightGreen" | "ltGreen" => 0x90_EE90,
        "lightYellow" | "ltYellow" => 0xFF_FFE0,
        "indigo" => 0x4B_0082,
        "violet" => 0xEE_82EE,
        "coral" => 0xFF_7F50,
        "tomato" => 0xFF_6347,
        "skyBlue" => 0x87_CEEB,
        "steelBlue" => 0x46_82B4,
        "tan" => 0xD2_B48C,
        "beige" => 0xF5_F5DC,
        "ivory" => 0xFF_FFF0,
        "khaki" => 0xF0_E68C,
        "salmon" => 0xFA_8072,
        "crimson" => 0xDC_143C,
        "turquoise" => 0x40_E0D0,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use bo_model::slide::ColorScheme;

    fn m(name: &str, val: i32) -> ColorMod {
        ColorMod {
            name: name.into(),
            val: Some(val),
        }
    }

    fn theme() -> Theme {
        Theme {
            colors: ColorScheme {
                name: "t".into(),
                colors: vec![
                    ("dk1".into(), Color::srgb(0x00_0000)),
                    ("lt1".into(), Color::srgb(0xFF_FFFF)),
                    ("dk2".into(), Color::srgb(0x44_546A)),
                    ("lt2".into(), Color::srgb(0xE7_E6E6)),
                    ("accent1".into(), Color::srgb(0x44_72C4)),
                ],
                extra: vec![],
            },
            ..Theme::default()
        }
    }

    #[test]
    fn parse_write_roundtrip() {
        let el = crate::dom::parse(
            br#"<a:schemeClr val="accent1"><a:lumMod val="75000"/><a:alpha val="50000"/></a:schemeClr>"#,
            "/t",
        )
        .unwrap();
        let c = parse_color(&el).unwrap();
        assert_eq!(c.kind, ColorKind::Scheme("accent1".into()));
        assert_eq!(c.mods.len(), 2);
        assert_eq!(write_color(&c), el);
    }

    #[test]
    fn scheme_through_clr_map() {
        let t = theme();
        let map = vec![("tx1".to_string(), "lt1".to_string())];
        let ctx = ColorContext {
            theme: Some(&t),
            clr_map: &map,
            ph_clr: None,
        };
        // tx1 remapped to lt1 = white; bg2 default-mapped to lt2.
        assert_eq!(resolve(&Color::scheme("tx1"), &ctx).to_hex(), 0xFF_FFFF);
        assert_eq!(resolve(&Color::scheme("bg2"), &ctx).to_hex(), 0xE7_E6E6);
        assert_eq!(resolve(&Color::scheme("accent1"), &ctx).to_hex(), 0x44_72C4);
    }

    #[test]
    fn lum_mod_off() {
        // §20.1.2.3.20/21: lumMod 75000 multiplies HSL luminance by 0.75.
        // 0x4472C4: HSL l = (0xC4 + 0x44)/2/255 = 0.5176; ×0.75 = 0.3882.
        let c = Color {
            kind: ColorKind::Srgb(0x44_72C4),
            mods: vec![m("lumMod", 75_000)],
        };
        let r = resolve(&c, &ColorContext::default());
        let (_, _, l) = rgb_to_hsl(r.r, r.g, r.b);
        assert!((l - 0.388_2).abs() < 0.005, "{l}");
        assert_eq!(r.to_hex(), 0x2F_5597);
        // lumMod 20000 + lumOff 80000 on white: l = 1·0.2 + 0.8 = 1.0.
        let c = Color {
            kind: ColorKind::Srgb(0xFF_FFFF),
            mods: vec![m("lumMod", 20_000), m("lumOff", 80_000)],
        };
        assert_eq!(resolve(&c, &ColorContext::default()).to_hex(), 0xFF_FFFF);
        // Black with lumOff 50000 → mid gray (l = 0.5, s = 0).
        let c = Color {
            kind: ColorKind::Srgb(0),
            mods: vec![m("lumOff", 50_000)],
        };
        assert_eq!(resolve(&c, &ColorContext::default()).to_hex(), 0x80_8080);
    }

    #[test]
    fn tint_shade_alpha() {
        // shade 50%: linear 1.0 × 0.5 = 0.5 → sRGB 0.7354 → 188.
        let c = Color {
            kind: ColorKind::Srgb(0xFF_0000),
            mods: vec![m("shade", 50_000), m("alpha", 40_000)],
        };
        let r = resolve(&c, &ColorContext::default()).to_u8();
        assert_eq!(r, [188, 0, 0, 102]);
        // tint 0% → white; tint 100% → unchanged.
        let c = Color {
            kind: ColorKind::Srgb(0x12_3456),
            mods: vec![m("tint", 0)],
        };
        assert_eq!(resolve(&c, &ColorContext::default()).to_hex(), 0xFF_FFFF);
        let c = Color {
            kind: ColorKind::Srgb(0x12_3456),
            mods: vec![m("tint", 100_000)],
        };
        assert_eq!(resolve(&c, &ColorContext::default()).to_hex(), 0x12_3456);
    }

    #[test]
    fn other_models() {
        let ctx = ColorContext::default();
        let sys = Color {
            kind: ColorKind::System {
                val: "windowText".into(),
                last: Some(0x11_2233),
            },
            mods: vec![],
        };
        assert_eq!(resolve(&sys, &ctx).to_hex(), 0x11_2233);
        let prst = Color {
            kind: ColorKind::Preset("orange".into()),
            mods: vec![],
        };
        assert_eq!(resolve(&prst, &ctx).to_hex(), 0xFF_A500);
        let hsl = Color {
            kind: ColorKind::Hsl(120 * 60_000, 100_000, 50_000),
            mods: vec![],
        };
        assert_eq!(resolve(&hsl, &ctx).to_hex(), 0x00_FF00);
        let sc = Color {
            kind: ColorKind::ScRgb(100_000, 0, 0),
            mods: vec![m("comp", 0)],
        };
        assert_eq!(resolve(&sc, &ctx).to_hex(), 0x00_FFFF);
        let ph = ColorContext {
            ph_clr: Some(Rgba::from_hex(0x00_00FF)),
            ..ColorContext::default()
        };
        assert_eq!(resolve(&Color::scheme("phClr"), &ph).to_hex(), 0x00_00FF);
    }
}
