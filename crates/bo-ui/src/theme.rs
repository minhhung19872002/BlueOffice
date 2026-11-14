//! Colours shared with the React interface: parsed from `packages/ui/src/tokens/tokens.json`,
//! which `tokens.ts` generates (a test keeps it in sync).
use bo_render::Rgba;
use serde::Deserialize;

const TOKENS: &str = include_str!("../../../packages/ui/src/tokens/tokens.json");

#[derive(Deserialize)]
struct Tokens {
    themes: std::collections::HashMap<String, std::collections::HashMap<String, String>>,
    accents: std::collections::HashMap<
        String,
        std::collections::HashMap<String, std::collections::HashMap<String, String>>,
    >,
}

/// Parse `#rgb`, `#rrggbb` or `rgba(r, g, b, a)`.
pub fn parse_color(s: &str) -> Option<Rgba> {
    let s = s.trim();
    if let Some(hex) = s.strip_prefix('#') {
        let v = |i: usize, n: usize| u8::from_str_radix(hex.get(i..i + n)?, 16).ok();
        return match hex.len() {
            6 => Some(Rgba(v(0, 2)?, v(2, 2)?, v(4, 2)?, 255)),
            3 => Some(Rgba(v(0, 1)? * 17, v(1, 1)? * 17, v(2, 1)? * 17, 255)),
            _ => None,
        };
    }
    let inner = s.strip_prefix("rgba(").or_else(|| s.strip_prefix("rgb("))?;
    let parts: Vec<f32> = inner
        .trim_end_matches(')')
        .split(',')
        .map(|p| p.trim().parse().ok())
        .collect::<Option<_>>()?;
    let alpha = parts.get(3).copied().unwrap_or(1.0);
    Some(Rgba(
        *parts.first()? as u8,
        *parts.get(1)? as u8,
        *parts.get(2)? as u8,
        (alpha.clamp(0.0, 1.0) * 255.0).round() as u8,
    ))
}

const RIBBON_LIGHT: Rgba = Rgba(0xf3, 0xf3, 0xf3, 255);

/// Interface colours of one theme and one app accent.
#[derive(Debug, Clone)]
pub struct Theme {
    /// Window background.
    pub bg: Rgba,
    /// Ribbon and panel surface.
    pub surface: Rgba,
    /// Body of the ribbon under the tab strip: a neutral light gray so white fields and
    /// galleries stand out, as on desktop office ribbons.
    pub ribbon: Rgba,
    /// Lines between groups and around controls.
    pub border: Rgba,
    /// Body text.
    pub text: Rgba,
    /// Secondary text (group labels, status bar).
    pub text_muted: Rgba,
    /// Hover tint.
    pub hover: Rgba,
    /// Pressed / toggled tint.
    pub active: Rgba,
    /// Focus ring.
    pub focus: Rgba,
    /// Background behind the pages.
    pub canvas: Rgba,
    /// App accent (selected tab underline, app mark).
    pub accent: Rgba,
    /// Filled accent (File button) and the text on it.
    pub accent_strong: Rgba,
    /// Text on `accent_strong`.
    pub accent_on: Rgba,
    /// Accent-coloured text.
    pub accent_text: Rgba,
}

impl Theme {
    /// The light theme with the accent of `app` (`doc`, `sheet` or `slide`).
    pub fn light(app: &str) -> Self {
        Self::load("light", app).unwrap_or_else(Self::fallback)
    }

    fn load(theme: &str, app: &str) -> Option<Self> {
        let t: Tokens = serde_json::from_str(TOKENS).ok()?;
        let c = t.themes.get(theme)?;
        let a = t.accents.get(theme)?.get(app)?;
        let get = |k: &str| c.get(k).and_then(|v| parse_color(v));
        let acc = |k: &str| a.get(k).and_then(|v| parse_color(v));
        Some(Self {
            bg: get("bg")?,
            surface: get("surface")?,
            ribbon: get("ribbon").unwrap_or(RIBBON_LIGHT),
            border: get("border")?,
            text: get("text")?,
            text_muted: get("text-muted")?,
            hover: get("hover")?,
            active: get("active")?,
            focus: get("focus")?,
            canvas: get("canvas-bg")?,
            accent: acc("base")?,
            accent_strong: acc("strong")?,
            accent_on: acc("on")?,
            accent_text: acc("text")?,
        })
    }

    fn fallback() -> Self {
        Self {
            bg: Rgba(0xee, 0xf1, 0xf5, 255),
            surface: Rgba(255, 255, 255, 255),
            ribbon: RIBBON_LIGHT,
            border: Rgba(0xd3, 0xd9, 0xe2, 255),
            text: Rgba(0x1b, 0x24, 0x30, 255),
            text_muted: Rgba(0x4f, 0x5a, 0x6a, 255),
            hover: Rgba(27, 36, 48, 15),
            active: Rgba(27, 36, 48, 31),
            focus: Rgba(0x1f, 0x6f, 0xeb, 255),
            canvas: Rgba(0xe3, 0xe7, 0xee, 255),
            accent: Rgba(0x4f, 0x46, 0xe5, 255),
            accent_strong: Rgba(0x4f, 0x46, 0xe5, 255),
            accent_on: Rgba(255, 255, 255, 255),
            accent_text: Rgba(0x43, 0x38, 0xca, 255),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_parse_into_the_light_doc_theme() {
        let t = Theme::load("light", "doc").expect("tokens.json");
        assert_eq!(t.surface, Rgba(255, 255, 255, 255));
        assert_eq!(t.accent, Rgba(0x4f, 0x46, 0xe5, 255));
        assert_eq!(t.hover, Rgba(27, 36, 48, 15));
        assert_eq!(parse_color("#fff"), Some(Rgba(255, 255, 255, 255)));
    }
}
