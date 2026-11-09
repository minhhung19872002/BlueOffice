//! Character effects of WordprocessingML runs that go beyond plain formatting:
//! the [MS-DOCX] Word 2010 text effects and OpenType features (`w14:glow`, `w14:shadow`,
//! `w14:reflection`, `w14:textOutline`, `w14:textFill`, `w14:ligatures`, `w14:numForm`,
//! `w14:numSpacing`, `w14:stylisticSets`, `w14:cntxtAlts`) and the East Asian layout of
//! a run (ECMA-376 Part 1, §17.3.2.10 eastAsianLayout).
//!
//! Each effect read from a file keeps its source element (`xml`): the writer emits it
//! verbatim while the effect is unchanged (an edit replaces the whole effect and drops
//! `xml`), so effects round-trip losslessly even where the model is a subset
//! (colour transforms, 3-D scene, …).

use crate::write::Color;
use serde::{Deserialize, Serialize};

/// A DrawingML colour as used by the w14 effects ([MS-DOCX] §2.6 `CT_SolidColorFillProperties`):
/// an RGB value (resolved from `w14:schemeClr` through the theme when possible), the scheme
/// name it came from and its opacity.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FxColor {
    /// Colour value.
    pub rgb: Color,
    /// `w14:schemeClr/@w14:val` (`accent1`, `tx1`, …) when the colour is a theme colour.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scheme: Option<String>,
    /// Opacity in thousandths of a percent (`100000` = opaque; `w14:alpha` is the transparency).
    pub alpha: u32,
}

impl FxColor {
    /// Opaque RGB colour.
    #[must_use]
    pub fn rgb(rgb: Color) -> FxColor {
        FxColor {
            rgb,
            scheme: None,
            alpha: 100_000,
        }
    }

    /// Opacity in 0..=1.
    #[must_use]
    pub fn opacity(&self) -> f32 {
        (self.alpha.min(100_000) as f32) / 100_000.0
    }
}

/// A gradient stop: position in thousandths of a percent (0..=100000) and colour.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GradientStop {
    /// `w14:gs/@w14:pos`.
    pub pos: u32,
    /// Stop colour.
    pub color: FxColor,
}

/// Fill of text or of its outline ([MS-DOCX] `CT_FillTextEffect`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum FxFill {
    /// `w14:noFill`.
    None,
    /// `w14:solidFill`.
    Solid {
        /// Fill colour.
        color: FxColor,
    },
    /// `w14:gradFill`: linear (`w14:lin/@w14:ang`, 60 000ths of a degree) or path gradient.
    Gradient {
        /// Colour stops in increasing position.
        stops: Vec<GradientStop>,
        /// Direction of a linear gradient, 60 000ths of a degree clockwise from left→right.
        angle: i32,
        /// `w14:path` (radial-like) instead of `w14:lin`.
        #[serde(default)]
        path: bool,
    },
}

/// `w14:glow` ([MS-DOCX] `CT_Glow`): a blurred halo around the glyphs.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextGlow {
    /// `w14:rad`, EMU.
    pub radius: i64,
    /// Glow colour.
    pub color: FxColor,
    /// Source element, written verbatim while unchanged.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub xml: Option<String>,
}

/// `w14:shadow` ([MS-DOCX] `CT_Shadow`): a blurred, offset copy of the glyphs behind them.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextShadow {
    /// `w14:blurRad`, EMU.
    pub blur: i64,
    /// `w14:dist`, EMU.
    pub dist: i64,
    /// `w14:dir`, 60 000ths of a degree clockwise from the positive x axis.
    pub dir: i32,
    /// Shadow colour.
    pub color: FxColor,
    /// Source element, written verbatim while unchanged.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub xml: Option<String>,
}

/// `w14:reflection` ([MS-DOCX] `CT_Reflection`): a fading mirror image under the text.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextReflection {
    /// `w14:blurRad`, EMU.
    pub blur: i64,
    /// `w14:stA`: opacity at the start, thousandths of a percent.
    pub start_alpha: u32,
    /// `w14:endA`: opacity at the end.
    pub end_alpha: u32,
    /// `w14:endPos`: where the fade ends, thousandths of a percent of the text height.
    pub end_pos: u32,
    /// `w14:dist`, EMU gap between the text and its reflection.
    pub dist: i64,
    /// Source element, written verbatim while unchanged.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub xml: Option<String>,
}

/// `w14:textOutline` ([MS-DOCX] `CT_TextOutlineEffect`): the stroke of the glyph outlines.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextOutline {
    /// `w14:w`, EMU.
    pub width: i64,
    /// Stroke paint (`None` fill = no outline).
    pub fill: FxFill,
    /// Source element, written verbatim while unchanged.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub xml: Option<String>,
}

/// `w14:textFill` ([MS-DOCX] `CT_FillTextEffect`): the paint of the glyph interiors.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextFill {
    /// Interior paint.
    pub fill: FxFill,
    /// Source element, written verbatim while unchanged.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub xml: Option<String>,
}

/// The [MS-DOCX] text effects and OpenType features of a run. `None` = inherit.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextEffects {
    /// `w14:glow`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub glow: Option<TextGlow>,
    /// `w14:shadow`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shadow: Option<TextShadow>,
    /// `w14:reflection`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reflection: Option<TextReflection>,
    /// `w14:textOutline`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outline: Option<TextOutline>,
    /// `w14:textFill`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill: Option<TextFill>,
    /// `w14:ligatures/@w14:val` ([MS-DOCX] `ST_Ligatures`: `none`, `standard`, `contextual`,
    /// `historical`, `discretional` and their combinations such as `standardContextual`, `all`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ligatures: Option<String>,
    /// `w14:numForm/@w14:val` (`default`, `lining`, `oldStyle`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub num_form: Option<String>,
    /// `w14:numSpacing/@w14:val` (`default`, `proportional`, `tabular`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub num_spacing: Option<String>,
    /// `w14:stylisticSets`: the enabled `w14:styleSet/@w14:id` numbers (1–20).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stylistic_sets: Option<Vec<u8>>,
    /// `w14:cntxtAlts/@w14:val`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contextual_alternates: Option<bool>,
}

impl TextEffects {
    /// Nothing is set.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        *self == TextEffects::default()
    }

    /// Overlay `self` on `base`, effect by effect.
    #[must_use]
    pub fn inherit(&self, base: &TextEffects) -> TextEffects {
        TextEffects {
            glow: self.glow.clone().or_else(|| base.glow.clone()),
            shadow: self.shadow.clone().or_else(|| base.shadow.clone()),
            reflection: self.reflection.clone().or_else(|| base.reflection.clone()),
            outline: self.outline.clone().or_else(|| base.outline.clone()),
            fill: self.fill.clone().or_else(|| base.fill.clone()),
            ligatures: self.ligatures.clone().or_else(|| base.ligatures.clone()),
            num_form: self.num_form.clone().or_else(|| base.num_form.clone()),
            num_spacing: self
                .num_spacing
                .clone()
                .or_else(|| base.num_spacing.clone()),
            stylistic_sets: self
                .stylistic_sets
                .clone()
                .or_else(|| base.stylistic_sets.clone()),
            contextual_alternates: self.contextual_alternates.or(base.contextual_alternates),
        }
    }

    /// OpenType feature settings implied by the typography properties, as
    /// (tag, value) pairs for the shaper (OpenType feature registry: `liga`, `clig`,
    /// `hlig`, `dlig`, `calt`, `lnum`, `onum`, `pnum`, `tnum`, `ss01`–`ss20`). Properties
    /// left unset add nothing, so the shaper's defaults apply.
    #[must_use]
    pub fn opentype_features(&self) -> Vec<([u8; 4], u32)> {
        let mut v = Vec::new();
        if let Some(l) = &self.ligatures {
            // [MS-DOCX] ST_Ligatures: a combination of the four classes, or none / all.
            let all = l == "all";
            let has = |w: &str| all || l.to_ascii_lowercase().contains(w);
            v.push((*b"liga", u32::from(has("standard"))));
            v.push((*b"clig", u32::from(has("contextual"))));
            v.push((*b"hlig", u32::from(has("historical"))));
            v.push((*b"dlig", u32::from(has("discretional"))));
        }
        match self.num_form.as_deref() {
            Some("lining") => v.push((*b"lnum", 1)),
            Some("oldStyle") => v.push((*b"onum", 1)),
            _ => {}
        }
        match self.num_spacing.as_deref() {
            Some("proportional") => v.push((*b"pnum", 1)),
            Some("tabular") => v.push((*b"tnum", 1)),
            _ => {}
        }
        if let Some(sets) = &self.stylistic_sets {
            for &n in sets.iter().filter(|n| (1..=20).contains(*n)) {
                let tag = [b's', b's', b'0' + n / 10, b'0' + n % 10];
                v.push((tag, 1));
            }
        }
        if let Some(c) = self.contextual_alternates {
            v.push((*b"calt", u32::from(c)));
        }
        v
    }
}

/// East Asian typography of a run (ECMA-376 Part 1, §17.3.2.10 eastAsianLayout).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EastAsianLayout {
    /// `w:id`: runs with the same id form one combined unit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<i64>,
    /// `w:combine`: two lines in one (the text set in two half-height lines).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub combine: Option<bool>,
    /// `w:combineBrackets` (§17.18.8 `ST_CombineBrackets`: `none`, `round`, `square`,
    /// `angle`, `curly`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub combine_brackets: Option<String>,
    /// `w:vert`: horizontal text in vertical text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vert: Option<bool>,
    /// `w:vertCompress`: compress the rotated text to the line height.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vert_compress: Option<bool>,
}

impl EastAsianLayout {
    /// The bracket pair drawn around combined text (§17.18.8).
    #[must_use]
    pub fn brackets(&self) -> Option<(char, char)> {
        match self.combine_brackets.as_deref() {
            Some("round") => Some(('(', ')')),
            Some("square") => Some(('[', ']')),
            Some("angle") => Some(('<', '>')),
            Some("curly") => Some(('{', '}')),
            _ => None,
        }
    }
}

/// Character shading pattern beyond the fill (ECMA-376 Part 1, §17.3.2.32 shd: `w:val`
/// pattern and `w:color` pattern colour). The fill itself is `RunProps::shading`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShadingPattern {
    /// `w:val` (§17.18.78 `ST_Shd`: `clear`, `solid`, `pct10`, …).
    pub val: String,
    /// `w:color`; `None` = auto.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<Color>,
}

impl ShadingPattern {
    /// Share of the pattern colour in the mix (0..=1) for the percentage patterns of
    /// §17.18.78 (`pct5` … `pct95`, `solid`); line patterns count as a light mix.
    #[must_use]
    pub fn coverage(&self) -> f32 {
        match self.val.as_str() {
            "clear" | "nil" => 0.0,
            "solid" => 1.0,
            v => v
                .strip_prefix("pct")
                .and_then(|p| p.parse::<f32>().ok())
                .map_or(0.25, |p| (p / 100.0).clamp(0.0, 1.0)),
        }
    }
}
