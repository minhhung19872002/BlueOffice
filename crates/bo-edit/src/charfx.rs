//! Advanced character formatting: effects (ECMA-376 Part 1, §17.3.2.13 emboss,
//! §17.3.2.18 imprint, §17.3.2.23 outline, §17.3.2.31 shadow), spacing / position /
//! scale / kerning (§17.3.2.35, §17.3.2.24, §17.3.2.43, §17.3.2.19), character border
//! and shading (§17.3.2.4, §17.3.2.32), two lines in one (§17.3.2.10), the [MS-DOCX]
//! w14 text effects and OpenType typography, and phonetic guides (§17.3.3.25 ruby).
//!
//! The UI talks in simple units (points, degrees, percent, `RRGGBB`); this module
//! converts them to the model.

use crate::{body_paragraphs, para, para_at, Pos, Result, Selection, Session};
use bo_model::run_fx::{
    EastAsianLayout, FxColor, FxFill, GradientStop, TextFill, TextGlow, TextOutline,
    TextReflection, TextShadow,
};
use bo_model::{Border, Color, HalfPoints, Raw, RunContent, RunProps, Twips};
use serde::{Deserialize, Deserializer, Serialize};

const EMU_PER_PT: f64 = 12_700.0;

fn emu(pt: f32) -> i64 {
    (f64::from(pt.max(0.0)) * EMU_PER_PT).round() as i64
}

fn pt(emu: i64) -> f32 {
    (emu as f64 / EMU_PER_PT) as f32
}

fn color(hex: &str) -> Color {
    Color::from_hex(hex.trim_start_matches('#')).unwrap_or(Color::BLACK)
}

/// Opacity (thousandths of a percent) from a transparency in percent.
fn opacity(transparency: u32) -> u32 {
    100_000 - transparency.min(100) * 1000
}

fn transparency(c: &FxColor) -> u32 {
    (100_000 - c.alpha.min(100_000)).div_ceil(1000)
}

/// Glow: colour, radius in points, transparency in percent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GlowSpec {
    /// `RRGGBB`.
    pub color: String,
    /// Radius, points.
    pub radius: f32,
    /// Transparency, percent.
    pub transparency: u32,
}

/// Soft shadow: colour, blur and distance in points, direction in degrees.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShadowSpec {
    /// `RRGGBB`.
    pub color: String,
    /// Blur radius, points.
    pub blur: f32,
    /// Distance, points.
    pub distance: f32,
    /// Direction, degrees clockwise from the right.
    pub angle: f32,
    /// Transparency, percent.
    pub transparency: u32,
}

/// Reflection: size (share of the text height, percent), start transparency, gap.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReflectionSpec {
    /// Fade length, percent of the text height.
    pub size: u32,
    /// Transparency at the start, percent.
    pub transparency: u32,
    /// Gap below the text, points.
    pub distance: f32,
}

/// Text outline: colour and width in points.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutlineSpec {
    /// `RRGGBB`.
    pub color: String,
    /// Width, points.
    pub width: f32,
}

/// Two-colour linear gradient fill.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GradientSpec {
    /// Start colour `RRGGBB`.
    pub from: String,
    /// End colour `RRGGBB`.
    pub to: String,
    /// Direction, degrees clockwise from left→right.
    pub angle: f32,
}

/// Character border.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BorderSpec {
    /// §17.18.2 `ST_Border` (`single`, `double`, `dotted`, …).
    pub style: String,
    /// Width in eighths of a point.
    pub size: u32,
    /// `RRGGBB`; `None` = automatic.
    pub color: Option<String>,
}

/// Advanced character formatting at the selection (Font dialog state).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CharAdvanced {
    /// §17.3.2.13 emboss.
    pub emboss: bool,
    /// §17.3.2.18 imprint.
    pub imprint: bool,
    /// §17.3.2.23 outline.
    pub outline: bool,
    /// §17.3.2.31 shadow.
    pub shadow: bool,
    /// §17.3.2.41 vanish (hidden text).
    #[serde(default)]
    pub hidden: bool,
    /// §17.3.2.35 spacing, twips (0 = normal).
    pub spacing: i32,
    /// §17.3.2.24 position, half-points (positive = raised).
    pub position: i32,
    /// §17.3.2.43 w, percent.
    pub scale: u32,
    /// §17.3.2.19 kern threshold, half-points (`None` = no kerning).
    pub kern: Option<u32>,
    /// w14:glow.
    pub glow: Option<GlowSpec>,
    /// w14:shadow.
    pub soft_shadow: Option<ShadowSpec>,
    /// w14:reflection.
    pub reflection: Option<ReflectionSpec>,
    /// w14:textOutline (solid).
    pub text_outline: Option<OutlineSpec>,
    /// w14:textFill gradient.
    pub gradient: Option<GradientSpec>,
    /// w14:ligatures (`""` = not set).
    pub ligatures: String,
    /// w14:numForm (`""` = not set).
    pub num_form: String,
    /// w14:numSpacing (`""` = not set).
    pub num_spacing: String,
    /// Stylistic set 1–20 (0 = none).
    pub stylistic_set: u8,
    /// w14:cntxtAlts.
    pub contextual_alternates: bool,
    /// §17.3.2.4 bdr.
    pub border: Option<BorderSpec>,
    /// §17.3.2.32 shd fill `RRGGBB`.
    pub shading: Option<String>,
    /// §17.3.2.10 combine.
    pub combine: bool,
    /// §17.18.8 `ST_CombineBrackets` (`""` = none).
    pub combine_brackets: String,
}

fn fx_hex(c: &FxColor) -> String {
    c.rgb.to_hex()
}

impl CharAdvanced {
    /// State of effective run properties.
    #[must_use]
    pub fn of(r: &RunProps) -> CharAdvanced {
        let fx = r.effects.as_deref().cloned().unwrap_or_default();
        let solid = |f: &FxFill| match f {
            FxFill::Solid { color } => Some(color.clone()),
            FxFill::Gradient { stops, .. } => stops.first().map(|s| s.color.clone()),
            FxFill::None => None,
        };
        CharAdvanced {
            emboss: r.emboss.unwrap_or(false),
            imprint: r.imprint.unwrap_or(false),
            outline: r.outline.unwrap_or(false),
            shadow: r.shadow.unwrap_or(false),
            hidden: r.hidden.unwrap_or(false),
            spacing: r.spacing.map_or(0, |t| t.0),
            position: r.position.unwrap_or(0),
            scale: r.scale.unwrap_or(100),
            kern: r.kern.map(|k| k.0).filter(|k| *k > 0),
            glow: fx.glow.as_ref().map(|g| GlowSpec {
                color: fx_hex(&g.color),
                radius: pt(g.radius),
                transparency: transparency(&g.color),
            }),
            soft_shadow: fx.shadow.as_ref().map(|s| ShadowSpec {
                color: fx_hex(&s.color),
                blur: pt(s.blur),
                distance: pt(s.dist),
                angle: s.dir as f32 / 60_000.0,
                transparency: transparency(&s.color),
            }),
            reflection: fx.reflection.as_ref().map(|x| ReflectionSpec {
                size: x.end_pos / 1000,
                transparency: 100 - (x.start_alpha / 1000).min(100),
                distance: pt(x.dist),
            }),
            text_outline: fx.outline.as_ref().and_then(|o| {
                solid(&o.fill).map(|c| OutlineSpec {
                    color: fx_hex(&c),
                    width: pt(o.width),
                })
            }),
            gradient: fx.fill.as_ref().and_then(|f| match &f.fill {
                FxFill::Gradient { stops, angle, .. } if stops.len() >= 2 => Some(GradientSpec {
                    from: fx_hex(&stops[0].color),
                    to: fx_hex(&stops[stops.len() - 1].color),
                    angle: *angle as f32 / 60_000.0,
                }),
                _ => None,
            }),
            ligatures: fx.ligatures.clone().unwrap_or_default(),
            num_form: fx.num_form.clone().unwrap_or_default(),
            num_spacing: fx.num_spacing.clone().unwrap_or_default(),
            stylistic_set: fx
                .stylistic_sets
                .as_ref()
                .and_then(|s| s.first().copied())
                .unwrap_or(0),
            contextual_alternates: fx.contextual_alternates.unwrap_or(false),
            border: r
                .border
                .as_ref()
                .filter(|b| b.is_visible())
                .map(|b| BorderSpec {
                    style: b.style.clone(),
                    size: b.size,
                    color: b.color.map(Color::to_hex),
                }),
            shading: r.shading.map(Color::to_hex),
            combine: r
                .east_asian
                .as_ref()
                .is_some_and(|e| e.combine == Some(true)),
            combine_brackets: r
                .east_asian
                .as_ref()
                .and_then(|e| e.combine_brackets.clone())
                .unwrap_or_default(),
        }
    }
}

/// `Some(None)` for JSON `null` (remove), `Some(Some(v))` for a value; absent = `None`.
#[allow(clippy::option_option)]
fn double<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    d: D,
) -> std::result::Result<Option<Option<T>>, D::Error> {
    Option::<T>::deserialize(d).map(Some)
}

/// Changes made by the Font dialog: absent fields stay as they are; for the effects
/// `null` removes them (hence `Option<Option<_>>`: absent / remove / set).
#[allow(clippy::option_option)]
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CharAdvancedPatch {
    /// §17.3.2.13 emboss (turning it on turns imprint and outline off).
    pub emboss: Option<bool>,
    /// §17.3.2.18 imprint (turning it on turns emboss and outline off).
    pub imprint: Option<bool>,
    /// §17.3.2.23 outline.
    pub outline: Option<bool>,
    /// §17.3.2.31 shadow.
    pub shadow: Option<bool>,
    /// §17.3.2.41 vanish: hidden text.
    pub hidden: Option<bool>,
    /// Spacing in twips (0 removes it).
    pub spacing: Option<i32>,
    /// Position in half-points (0 removes it).
    pub position: Option<i32>,
    /// Scale in percent (100 removes it).
    pub scale: Option<u32>,
    /// Kerning threshold in half-points (`null` = no kerning).
    #[serde(deserialize_with = "double")]
    pub kern: Option<Option<u32>>,
    /// Glow.
    #[serde(deserialize_with = "double")]
    pub glow: Option<Option<GlowSpec>>,
    /// Soft shadow.
    #[serde(deserialize_with = "double")]
    pub soft_shadow: Option<Option<ShadowSpec>>,
    /// Reflection.
    #[serde(deserialize_with = "double")]
    pub reflection: Option<Option<ReflectionSpec>>,
    /// Text outline.
    #[serde(deserialize_with = "double")]
    pub text_outline: Option<Option<OutlineSpec>>,
    /// Gradient fill.
    #[serde(deserialize_with = "double")]
    pub gradient: Option<Option<GradientSpec>>,
    /// Ligatures (`""` removes the setting).
    pub ligatures: Option<String>,
    /// Number form (`""` removes the setting).
    pub num_form: Option<String>,
    /// Number spacing (`""` removes the setting).
    pub num_spacing: Option<String>,
    /// Stylistic set (0 removes it).
    pub stylistic_set: Option<u8>,
    /// Contextual alternates.
    pub contextual_alternates: Option<bool>,
    /// Character border.
    #[serde(deserialize_with = "double")]
    pub border: Option<Option<BorderSpec>>,
    /// Character shading fill.
    #[serde(deserialize_with = "double")]
    pub shading: Option<Option<String>>,
    /// Two lines in one.
    pub combine: Option<bool>,
    /// Brackets of combined text (`""` = none).
    pub combine_brackets: Option<String>,
}

fn opt_str(v: &str) -> Option<String> {
    (!v.is_empty()).then(|| v.to_owned())
}

impl CharAdvancedPatch {
    /// Apply to the direct properties of a run.
    #[allow(clippy::too_many_lines)]
    pub fn apply(&self, r: &mut RunProps) {
        let flag = |v: bool| v.then_some(true);
        if let Some(v) = self.emboss {
            r.emboss = flag(v);
            if v {
                r.imprint = None;
                r.outline = None;
            }
        }
        if let Some(v) = self.imprint {
            r.imprint = flag(v);
            if v {
                r.emboss = None;
                r.outline = None;
            }
        }
        if let Some(v) = self.outline {
            r.outline = flag(v);
        }
        if let Some(v) = self.shadow {
            r.shadow = flag(v);
        }
        if let Some(v) = self.hidden {
            r.hidden = flag(v);
        }
        if let Some(v) = self.spacing {
            r.spacing = (v != 0).then_some(Twips(v.clamp(-31_680, 31_680)));
        }
        if let Some(v) = self.position {
            r.position = (v != 0).then_some(v.clamp(-3168, 3168));
        }
        if let Some(v) = self.scale {
            r.scale = (v != 100).then_some(v.clamp(1, 600));
        }
        if let Some(k) = self.kern {
            r.kern = k.map(|k| HalfPoints(k.clamp(1, 3276)));
        }
        let mut fx = r.effects.take().map(|b| *b).unwrap_or_default();
        if let Some(g) = &self.glow {
            fx.glow = g.as_ref().map(|g| TextGlow {
                radius: emu(g.radius),
                color: FxColor {
                    rgb: color(&g.color),
                    scheme: None,
                    alpha: opacity(g.transparency),
                },
                xml: None,
            });
        }
        if let Some(s) = &self.soft_shadow {
            fx.shadow = s.as_ref().map(|s| TextShadow {
                blur: emu(s.blur),
                dist: emu(s.distance),
                dir: ((f64::from(s.angle).rem_euclid(360.0)) * 60_000.0).round() as i32,
                color: FxColor {
                    rgb: color(&s.color),
                    scheme: None,
                    alpha: opacity(s.transparency),
                },
                xml: None,
            });
        }
        if let Some(x) = &self.reflection {
            fx.reflection = x.as_ref().map(|x| TextReflection {
                blur: 0,
                start_alpha: opacity(x.transparency),
                end_alpha: 0,
                end_pos: x.size.min(100) * 1000,
                dist: emu(x.distance),
                xml: None,
            });
        }
        if let Some(o) = &self.text_outline {
            fx.outline = o.as_ref().map(|o| TextOutline {
                width: emu(o.width),
                fill: FxFill::Solid {
                    color: FxColor::rgb(color(&o.color)),
                },
                xml: None,
            });
        }
        if let Some(g) = &self.gradient {
            fx.fill = g.as_ref().map(|g| TextFill {
                fill: FxFill::Gradient {
                    stops: vec![
                        GradientStop {
                            pos: 0,
                            color: FxColor::rgb(color(&g.from)),
                        },
                        GradientStop {
                            pos: 100_000,
                            color: FxColor::rgb(color(&g.to)),
                        },
                    ],
                    angle: ((f64::from(g.angle).rem_euclid(360.0)) * 60_000.0).round() as i32,
                    path: false,
                },
                xml: None,
            });
        }
        if let Some(v) = &self.ligatures {
            fx.ligatures = opt_str(v);
        }
        if let Some(v) = &self.num_form {
            fx.num_form = opt_str(v);
        }
        if let Some(v) = &self.num_spacing {
            fx.num_spacing = opt_str(v);
        }
        if let Some(n) = self.stylistic_set {
            fx.stylistic_sets = (1..=20).contains(&n).then(|| vec![n]);
        }
        if let Some(c) = self.contextual_alternates {
            fx.contextual_alternates = c.then_some(true);
        }
        r.effects = (!fx.is_empty()).then(|| Box::new(fx));
        if let Some(b) = &self.border {
            r.border = b.as_ref().map(|b| Border {
                style: b.style.clone(),
                size: b.size.clamp(2, 96),
                color: b.color.as_deref().and_then(Color::from_hex),
                space: 0,
            });
        }
        if let Some(s) = &self.shading {
            r.shading = s.as_deref().and_then(Color::from_hex);
            r.shading_pattern = None;
        }
        if self.combine.is_some() || self.combine_brackets.is_some() {
            let mut ea: EastAsianLayout = r.east_asian.take().unwrap_or_default();
            if let Some(c) = self.combine {
                ea.combine = c.then_some(true);
            }
            if let Some(b) = &self.combine_brackets {
                ea.combine_brackets = opt_str(b).filter(|b| b != "none");
            }
            if ea.combine.is_none() {
                ea.combine_brackets = None;
            }
            r.east_asian = (ea != EastAsianLayout::default()).then_some(ea);
        }
    }
}

/// A phonetic guide (§17.3.3.25 ruby) as edited by the UI.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RubySpec {
    /// Base text (§17.3.3.27 rubyBase).
    pub base: String,
    /// Guide text (§17.3.3.24 rt).
    pub guide: String,
    /// §17.3.3.26 rubyAlign (`center`, `distributeLetter`, `distributeSpace`, `left`, `right`).
    pub align: String,
    /// Guide size, half-points (§17.3.3.11 hps).
    pub size: u32,
    /// Distance of the guide baseline above the base baseline, half-points (§17.3.3.12 hpsRaise).
    pub raise: u32,
    /// Base text size, half-points (§17.3.3.13 hpsBaseText).
    pub base_size: u32,
    /// Language (§17.3.3.15 lid).
    pub lang: String,
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn unesc(s: &str) -> String {
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

impl RubySpec {
    /// The `w:ruby` element (CT_Ruby: rubyPr with all its required children, rt, rubyBase).
    #[must_use]
    pub fn to_xml(&self) -> String {
        let align = match self.align.as_str() {
            a @ ("center" | "distributeLetter" | "distributeSpace" | "left" | "right"
            | "rightVertical") => a,
            _ => "center",
        };
        let (hps, raise, base) = (
            self.size.clamp(2, 3276),
            self.raise.min(3276),
            self.base_size.clamp(2, 3276),
        );
        let lang = if self.lang.trim().is_empty() {
            "vi-VN"
        } else {
            self.lang.trim()
        };
        format!(
            "<w:ruby><w:rubyPr><w:rubyAlign w:val=\"{align}\"/><w:hps w:val=\"{hps}\"/><w:hpsRaise w:val=\"{raise}\"/><w:hpsBaseText w:val=\"{base}\"/><w:lid w:val=\"{}\"/></w:rubyPr><w:rt><w:r><w:rPr><w:sz w:val=\"{hps}\"/></w:rPr><w:t xml:space=\"preserve\">{}</w:t></w:r></w:rt><w:rubyBase><w:r><w:rPr><w:sz w:val=\"{base}\"/></w:rPr><w:t xml:space=\"preserve\">{}</w:t></w:r></w:rubyBase></w:ruby>",
            esc(lang),
            esc(&self.guide),
            esc(&self.base)
        )
    }

    /// Read a `w:ruby` element written by any producer (prefix `w`).
    #[must_use]
    pub fn parse(xml: &str) -> Option<RubySpec> {
        let attr = |el: &str| -> Option<String> {
            let re = regex::Regex::new(&format!(r#"<w:{el}\b[^>]*\bw:val="([^"]*)""#)).ok()?;
            re.captures(xml).map(|c| unesc(&c[1]))
        };
        let num = |el: &str| attr(el).and_then(|v| v.trim().parse::<u32>().ok());
        let texts = |el: &str| -> Option<String> {
            let block = regex::Regex::new(&format!(r"(?s)<w:{el}>(.*?)</w:{el}>")).ok()?;
            let inner = block.captures(xml)?.get(1)?.as_str().to_owned();
            let t = regex::Regex::new(r"(?s)<w:t(?:\s[^>]*)?>(.*?)</w:t>").ok()?;
            Some(t.captures_iter(&inner).map(|c| unesc(&c[1])).collect())
        };
        Some(RubySpec {
            base: texts("rubyBase")?,
            guide: texts("rt")?,
            align: attr("rubyAlign").unwrap_or_else(|| "center".into()),
            size: num("hps").unwrap_or(10),
            raise: num("hpsRaise").unwrap_or(20),
            base_size: num("hpsBaseText").unwrap_or(20),
            lang: attr("lid").unwrap_or_default(),
        })
    }
}

impl Session {
    /// Insert a phonetic guide in place of the selection; the new object is selected.
    pub fn insert_ruby(&mut self, spec: &RubySpec) -> Result<Selection> {
        let ruby = RunContent::Opaque(Raw::new(spec.to_xml(), 0));
        self.edit(|tx, sel| {
            let (a, b) = sel.ordered();
            let at = tx.delete_range(a, b)?;
            let (_, _, atoms) = tx.atoms(at.para)?;
            let props = para::props_at(&atoms, at.offset);
            let end = tx.insert_atoms(at, vec![(props, ruby.clone())])?;
            Ok(Selection {
                anchor: at,
                focus: end,
            })
        })
    }

    /// The phonetic guide the selection covers exactly (or the one just before a
    /// collapsed caret): its range and its properties.
    #[must_use]
    pub fn selected_ruby(&self) -> Option<(Selection, RubySpec)> {
        let (a, b) = self.selection.ordered();
        if a.para != b.para {
            return None;
        }
        let paths = body_paragraphs(&self.doc);
        let p = para_at(&self.doc, paths.get(a.para)?)?;
        let mut off = 0;
        for (_, c) in para::flatten(p) {
            let n = para::atom_len(&c);
            if let RunContent::Opaque(raw) = &c {
                if bo_model::index::is_ruby_xml(&raw.xml) {
                    let covers = a.offset == off && b.offset == off + n;
                    let after = a == b && a.offset == off + n;
                    if covers || after {
                        return Some((
                            Selection {
                                anchor: Pos::new(a.para, off),
                                focus: Pos::new(a.para, off + n),
                            },
                            RubySpec::parse(&raw.xml)?,
                        ));
                    }
                }
            }
            off += n;
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ruby_xml_round_trip() {
        let spec = RubySpec {
            base: "Hà <Nội>".into(),
            guide: "ha noi".into(),
            align: "center".into(),
            size: 10,
            raise: 18,
            base_size: 20,
            lang: "vi-VN".into(),
        };
        let xml = spec.to_xml();
        assert!(bo_model::index::is_ruby_xml(&xml));
        assert_eq!(RubySpec::parse(&xml), Some(spec));
    }

    #[test]
    fn patch_units() {
        let mut r = RunProps::default();
        let patch: CharAdvancedPatch = serde_json::from_str(
            r#"{"emboss":true,"scale":150,"position":-6,"spacing":40,
                 "glow":{"color":"FF0000","radius":5,"transparency":40},
                 "ligatures":"standard","stylisticSet":3,
                 "border":{"style":"single","size":8,"color":null},
                 "combine":true,"combineBrackets":"round"}"#,
        )
        .unwrap();
        patch.apply(&mut r);
        assert_eq!(r.emboss, Some(true));
        assert_eq!(
            (r.scale, r.position, r.spacing),
            (Some(150), Some(-6), Some(Twips(40)))
        );
        let fx = r.effects.as_deref().unwrap();
        let g = fx.glow.as_ref().unwrap();
        // 5 pt = 63 500 EMU; 40 % transparent = 60 000 thousandths of a percent opaque.
        assert_eq!((g.radius, g.color.alpha), (63_500, 60_000));
        assert_eq!(fx.stylistic_sets, Some(vec![3]));
        assert_eq!(r.border.as_ref().map(|b| b.size), Some(8));
        let ea = r.east_asian.as_ref().unwrap();
        assert_eq!(
            (ea.combine, ea.combine_brackets.as_deref()),
            (Some(true), Some("round"))
        );
        let state = CharAdvanced::of(&r);
        assert_eq!(state.glow.as_ref().map(|g| g.transparency), Some(40));
        assert!((state.glow.as_ref().unwrap().radius - 5.0).abs() < 1e-4);
        // Imprint turns emboss off; null removes the glow.
        let off: CharAdvancedPatch =
            serde_json::from_str(r#"{"imprint":true,"glow":null}"#).unwrap();
        off.apply(&mut r);
        assert_eq!((r.emboss, r.imprint), (None, Some(true)));
        assert!(r.effects.as_ref().unwrap().glow.is_none());
        // Absent fields are left alone.
        assert_eq!(r.scale, Some(150));
    }
}
