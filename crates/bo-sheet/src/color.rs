//! Colour resolution for spreadsheet colour references (ECMA-376 Part 1,
//! §18.3.1.15 / §18.8.19 `CT_Color`): `rgb`, `theme` + `tint`, `indexed`, `auto`.

use bo_model::sheet::{SheetColor, ThemeColors};
use bo_render::Rgba;

/// Default indexed colour palette (ECMA-376 Part 1, §18.8.27 `indexedColors`,
/// default values). Entries 64 and 65 are the system foreground/background.
#[allow(clippy::unreadable_literal)]
const INDEXED: [u32; 64] = [
    0x000000, 0xFFFFFF, 0xFF0000, 0x00FF00, 0x0000FF, 0xFFFF00, 0xFF00FF, 0x00FFFF, // 0-7
    0x000000, 0xFFFFFF, 0xFF0000, 0x00FF00, 0x0000FF, 0xFFFF00, 0xFF00FF, 0x00FFFF, // 8-15
    0x800000, 0x008000, 0x000080, 0x808000, 0x800080, 0x008080, 0xC0C0C0, 0x808080, // 16-23
    0x9999FF, 0x993366, 0xFFFFCC, 0xCCFFFF, 0x660066, 0xFF8080, 0x0066CC, 0xCCCCFF, // 24-31
    0x000080, 0xFF00FF, 0xFFFF00, 0x00FFFF, 0x800080, 0x800000, 0x008080, 0x0000FF, // 32-39
    0x00CCFF, 0xCCFFFF, 0xCCFFCC, 0xFFFF99, 0x99CCFF, 0xFF99CC, 0xCC99FF, 0xFFCC99, // 40-47
    0x3366FF, 0x33CCCC, 0x99CC00, 0xFFCC00, 0xFF9900, 0xFF6600, 0x666699, 0x969696, // 48-55
    0x003366, 0x339966, 0x003300, 0x333300, 0x993300, 0x993366, 0x333399, 0x333333, // 56-63
];

/// Theme colours used when a workbook has no theme part (BlueOffice defaults).
#[allow(clippy::unreadable_literal)]
const DEFAULT_THEME: [u32; 12] = [
    0x000000, 0xFFFFFF, 0x1F2A44, 0xE7E6E6, 0x2F6FB5, 0xE07B39, 0x9E9E9E, 0xF2B632, 0x4BA3DB,
    0x5AA454, 0x0B57D0, 0x7B1FA2,
];

fn split(v: u32) -> (u8, u8, u8) {
    ((v >> 16) as u8, (v >> 8) as u8, v as u8)
}

/// RGB of an indexed colour (64 = system foreground → black, 65 = system background → white).
pub fn indexed_rgb(i: u32) -> (u8, u8, u8) {
    match i {
        0..=63 => split(INDEXED[i as usize]),
        65 => (255, 255, 255),
        _ => (0, 0, 0),
    }
}

/// Parse `RRGGBB` or `AARRGGBB` (an optional `#` is ignored).
pub fn parse_hex(s: &str) -> Option<(u8, u8, u8)> {
    let s = s.trim().trim_start_matches('#');
    let s = if s.len() == 8 { &s[2..] } else { s };
    if s.len() != 6 || !s.is_ascii() {
        return None;
    }
    let v = u32::from_str_radix(s, 16).ok()?;
    Some(split(v))
}

/// Theme colour by `theme` index. SpreadsheetML theme indexes swap the
/// light/dark pairs relative to `clrScheme` order: 0 = lt1, 1 = dk1, 2 = lt2,
/// 3 = dk2, 4–9 = accent1–6, 10 = hlink, 11 = folHlink.
pub fn theme_rgb(index: u32, theme: Option<&ThemeColors>) -> (u8, u8, u8) {
    let slot = match index {
        0 => 1,
        1 => 0,
        2 => 3,
        3 => 2,
        n => n as usize,
    };
    theme
        .and_then(|t| t.colors.get(slot))
        .and_then(|s| parse_hex(s))
        .unwrap_or_else(|| split(DEFAULT_THEME.get(slot).copied().unwrap_or(0)))
}

/// Apply a tint (−1..1) by scaling HSL luminance (§18.3.1.15 `tint`).
pub fn apply_tint((r, g, b): (u8, u8, u8), tint: f64) -> (u8, u8, u8) {
    if tint == 0.0 {
        return (r, g, b);
    }
    let (h, s, l) = rgb_to_hsl(r, g, b);
    let l = if tint < 0.0 {
        l * (1.0 + tint)
    } else {
        l * (1.0 - tint) + tint
    };
    hsl_to_rgb(h, s, l.clamp(0.0, 1.0))
}

fn rgb_to_hsl(r: u8, g: u8, b: u8) -> (f64, f64, f64) {
    let (r, g, b) = (
        f64::from(r) / 255.0,
        f64::from(g) / 255.0,
        f64::from(b) / 255.0,
    );
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = f64::midpoint(max, min);
    if max == min {
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
    } / 6.0;
    (h, s, l)
}

fn hsl_to_rgb(h: f64, s: f64, l: f64) -> (u8, u8, u8) {
    let to = |v: f64| (v * 255.0).round().clamp(0.0, 255.0) as u8;
    if s == 0.0 {
        return (to(l), to(l), to(l));
    }
    let q = if l < 0.5 {
        l * (1.0 + s)
    } else {
        l + s - l * s
    };
    let p = 2.0 * l - q;
    let hue = |mut t: f64| {
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
    (to(hue(h + 1.0 / 3.0)), to(hue(h)), to(hue(h - 1.0 / 3.0)))
}

/// Resolve a colour reference. `None` for `auto` (the caller picks the context default).
pub fn resolve_color(c: &SheetColor, theme: Option<&ThemeColors>) -> Option<Rgba> {
    let base = if let Some(rgb) = c.rgb.as_deref().and_then(parse_hex) {
        rgb
    } else if let Some(t) = c.theme {
        theme_rgb(t, theme)
    } else {
        let i = c.indexed?;
        if i == 64 && c.tint.is_none() {
            return None; // system foreground: context dependent
        }
        indexed_rgb(i)
    };
    let (r, g, b) = apply_tint(base, c.tint.unwrap_or(0.0));
    Some(Rgba(r, g, b, 255))
}

/// Fill colour of a cell (§18.8.32 patternFill foreground, §18.8.24 first gradient
/// stop); `None` when the cell has no fill.
pub fn cell_fill_rgb(
    wb: &bo_model::sheet::Workbook,
    cell: &bo_model::sheet::Cell,
) -> Option<(u8, u8, u8)> {
    use bo_model::sheet::Fill;
    let xf = wb.styles.xf(cell.style)?;
    let theme = wb.theme.as_ref();
    let c = match wb.styles.fills.get(xf.fill_id as usize)? {
        Fill::Pattern(pf) => match pf.pattern.as_deref() {
            None | Some("none" | "gray125") => return None,
            _ => pf
                .fg
                .as_ref()
                .and_then(|c| resolve_color(c, theme))
                .unwrap_or(Rgba(0, 0, 0, 255)),
        },
        Fill::Gradient(g) => g.stops.first().and_then(|(_, c)| resolve_color(c, theme))?,
    };
    Some((c.0, c.1, c.2))
}

/// Font colour of a cell (§18.8.22 font/color); `None` = automatic.
pub fn cell_font_rgb(
    wb: &bo_model::sheet::Workbook,
    cell: &bo_model::sheet::Cell,
) -> Option<(u8, u8, u8)> {
    let xf = wb.styles.xf(cell.style)?;
    let font = wb.styles.fonts.get(xf.font_id as usize)?;
    let c = resolve_color(font.color.as_ref()?, wb.theme.as_ref())?;
    Some((c.0, c.1, c.2))
}

/// `RRGGBB` of a colour.
pub fn hex((r, g, b): (u8, u8, u8)) -> String {
    format!("{r:02X}{g:02X}{b:02X}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgb_theme_indexed() {
        let c = SheetColor::rgb("C00000");
        assert_eq!(resolve_color(&c, None), Some(Rgba(0xC0, 0, 0, 255)));
        let theme = ThemeColors {
            colors: vec![
                "000000".into(),
                "FFFFFF".into(),
                "111111".into(),
                "EEEEEE".into(),
                "1565C0".into(),
            ],
        };
        let t = |i| SheetColor {
            theme: Some(i),
            ..SheetColor::default()
        };
        assert_eq!(
            resolve_color(&t(0), Some(&theme)),
            Some(Rgba(255, 255, 255, 255))
        );
        assert_eq!(resolve_color(&t(1), Some(&theme)), Some(Rgba(0, 0, 0, 255)));
        assert_eq!(
            resolve_color(&t(4), Some(&theme)),
            Some(Rgba(0x15, 0x65, 0xC0, 255))
        );
        let idx = SheetColor {
            indexed: Some(10),
            ..SheetColor::default()
        };
        assert_eq!(resolve_color(&idx, None), Some(Rgba(255, 0, 0, 255)));
        assert_eq!(
            resolve_color(
                &SheetColor {
                    auto: true,
                    ..SheetColor::default()
                },
                None
            ),
            None
        );
    }

    #[test]
    fn tint() {
        // tint +1 → white, −1 → black, 0.5 lightens halfway.
        assert_eq!(apply_tint((0x40, 0x80, 0xC0), 1.0), (255, 255, 255));
        assert_eq!(apply_tint((0x40, 0x80, 0xC0), -1.0), (0, 0, 0));
        assert_eq!(apply_tint((0, 0, 0), 0.5), (128, 128, 128));
        assert_eq!(apply_tint((255, 0, 0), -0.5), (128, 0, 0));
    }
}
