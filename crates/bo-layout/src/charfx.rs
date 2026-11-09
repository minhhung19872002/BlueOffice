//! Character effects carried by fragments to the renderer: the ECMA-376 legacy
//! effects (§17.3.2.23 outline, §17.3.2.31 shadow, §17.3.2.13 emboss, §17.3.2.18
//! imprint), the horizontal scale (§17.3.2.43 w) and the [MS-DOCX] w14 text effects
//! (glow, soft shadow, reflection, text outline, text fill).

use bo_model::run_fx::{FxColor, FxFill, TextEffects};
use bo_model::{Color, RunProps};
use serde::Serialize;

/// One EMU in points (ECMA-376 Part 1, §20.1.2.1: 12 700 EMU per point).
const EMU_PER_PT: f32 = 12_700.0;

/// A paint resolved for drawing: colour + opacity (0..=1).
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct FxRgba {
    /// Colour.
    pub color: Color,
    /// Opacity in 0..=1.
    pub alpha: f32,
}

impl From<&FxColor> for FxRgba {
    fn from(c: &FxColor) -> Self {
        FxRgba {
            color: c.rgb,
            alpha: c.opacity(),
        }
    }
}

/// Paint of glyph interiors or outlines.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum FxPaint {
    /// Nothing drawn.
    None,
    /// Uniform colour.
    Solid(FxRgba),
    /// Linear gradient across the fragment box; `angle` in degrees clockwise from
    /// left→right; stops (offset 0..=1, paint).
    Linear {
        /// Direction in degrees.
        angle: f32,
        /// Stops in increasing offset.
        stops: Vec<(f32, FxRgba)>,
    },
    /// Radial gradient from the fragment box centre.
    Radial {
        /// Stops from the centre outwards.
        stops: Vec<(f32, FxRgba)>,
    },
}

impl FxPaint {
    fn from_fill(f: &FxFill) -> FxPaint {
        match f {
            FxFill::None => FxPaint::None,
            FxFill::Solid { color } => FxPaint::Solid(color.into()),
            FxFill::Gradient { stops, angle, path } => {
                let stops: Vec<(f32, FxRgba)> = stops
                    .iter()
                    .map(|s| (s.pos as f32 / 100_000.0, (&s.color).into()))
                    .collect();
                if *path {
                    FxPaint::Radial { stops }
                } else {
                    FxPaint::Linear {
                        angle: *angle as f32 / 60_000.0,
                        stops,
                    }
                }
            }
        }
    }

    /// A single representative colour (first stop of a gradient).
    #[must_use]
    pub fn first(&self) -> Option<FxRgba> {
        match self {
            FxPaint::None => None,
            FxPaint::Solid(c) => Some(*c),
            FxPaint::Linear { stops, .. } | FxPaint::Radial { stops } => stops.first().map(|s| s.1),
        }
    }
}

/// Soft shadow ([MS-DOCX] w14:shadow), points.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct FxShadow {
    /// Offset x.
    pub dx: f32,
    /// Offset y (down).
    pub dy: f32,
    /// Blur radius.
    pub blur: f32,
    /// Shadow paint.
    pub paint: FxRgba,
}

/// Reflection ([MS-DOCX] w14:reflection).
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct FxReflection {
    /// Gap below the text, points.
    pub dist: f32,
    /// Opacity at the start (0..=1).
    pub start_alpha: f32,
    /// Opacity at the end (0..=1).
    pub end_alpha: f32,
    /// Fade length as a share of the text height (0..=1).
    pub end_pos: f32,
}

/// Effects of a fragment's glyphs (all off by default).
#[derive(Debug, Clone, PartialEq, Serialize, Default)]
pub struct TextFx {
    /// §17.3.2.23 outline: hollow glyphs (stroked in the text colour).
    pub hollow: bool,
    /// §17.3.2.31 shadow.
    pub shadow: bool,
    /// §17.3.2.13 emboss.
    pub emboss: bool,
    /// §17.3.2.18 imprint.
    pub imprint: bool,
    /// Glow: radius (points) and paint.
    pub glow: Option<(f32, FxRgba)>,
    /// Soft shadow.
    pub soft_shadow: Option<FxShadow>,
    /// Reflection.
    pub reflection: Option<FxReflection>,
    /// Glyph outline stroke: width (points) and paint.
    pub stroke: Option<(f32, FxPaint)>,
    /// Glyph interior paint replacing the text colour.
    pub fill: Option<FxPaint>,
    /// §17.3.2.43 w: horizontal glyph scale (1 = 100 %).
    pub scale_x: f32,
}

impl TextFx {
    /// Effects of effective run properties; `None` when the glyphs are drawn plainly.
    #[must_use]
    pub fn of(rp: &RunProps) -> Option<TextFx> {
        let fx = rp.effects.as_deref();
        let t = TextFx {
            hollow: rp.outline.unwrap_or(false),
            shadow: rp.shadow.unwrap_or(false),
            emboss: rp.emboss.unwrap_or(false),
            imprint: rp.imprint.unwrap_or(false),
            glow: fx
                .and_then(|f| f.glow.as_ref())
                .filter(|g| g.radius > 0)
                .map(|g| (g.radius as f32 / EMU_PER_PT, (&g.color).into())),
            soft_shadow: fx.and_then(|f| f.shadow.as_ref()).map(|s| {
                // w14:dir: 60 000ths of a degree clockwise from +x (y down).
                let a = (s.dir as f32 / 60_000.0).to_radians();
                let d = s.dist as f32 / EMU_PER_PT;
                FxShadow {
                    dx: d * a.cos(),
                    dy: d * a.sin(),
                    blur: s.blur as f32 / EMU_PER_PT,
                    paint: (&s.color).into(),
                }
            }),
            reflection: fx
                .and_then(|f| f.reflection.as_ref())
                .map(|r| FxReflection {
                    dist: r.dist as f32 / EMU_PER_PT,
                    start_alpha: r.start_alpha as f32 / 100_000.0,
                    end_alpha: r.end_alpha as f32 / 100_000.0,
                    end_pos: r.end_pos as f32 / 100_000.0,
                }),
            stroke: fx
                .and_then(|f| f.outline.as_ref())
                .filter(|o| o.fill != FxFill::None)
                .map(|o| {
                    // A zero width is the thinnest visible line.
                    let w = (o.width as f32 / EMU_PER_PT).max(0.25);
                    (w, FxPaint::from_fill(&o.fill))
                }),
            fill: fx
                .and_then(|f| f.fill.as_ref())
                .map(|f| FxPaint::from_fill(&f.fill)),
            scale_x: rp.scale.map_or(1.0, |w| w.clamp(1, 600) as f32 / 100.0),
        };
        (t != TextFx {
            scale_x: 1.0,
            ..TextFx::default()
        })
        .then_some(t)
    }

    /// True when the glyphs themselves are not painted with the plain text colour
    /// (the renderer then keeps them only as invisible text and paints outlines).
    #[must_use]
    pub fn replaces_fill(&self) -> bool {
        self.hollow
            || self
                .fill
                .as_ref()
                .is_some_and(|f| !matches!(f, FxPaint::Solid(_)))
    }
}

/// OpenType features of effective run properties (see
/// [`TextEffects::opentype_features`]).
#[must_use]
pub fn features_of(rp: &RunProps) -> Vec<([u8; 4], u32)> {
    rp.effects
        .as_deref()
        .map(TextEffects::opentype_features)
        .unwrap_or_default()
}

/// Effective character shading colour (§17.3.2.32 shd): the fill (auto = white)
/// mixed with the pattern colour (auto = black) by the pattern's coverage.
#[must_use]
pub fn shading_color(rp: &RunProps) -> Option<Color> {
    let Some(pat) = rp.shading_pattern.as_ref().filter(|p| p.coverage() > 0.0) else {
        return rp.shading;
    };
    let fill = rp.shading.unwrap_or(Color::WHITE);
    let fg = pat.color.unwrap_or(Color::BLACK);
    let k = pat.coverage();
    let mix = |a: u8, b: u8| (f32::from(a) * (1.0 - k) + f32::from(b) * k).round() as u8;
    Some(Color {
        r: mix(fill.r, fg.r),
        g: mix(fill.g, fg.g),
        b: mix(fill.b, fg.b),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use bo_model::run_fx::{ShadingPattern, TextGlow, TextShadow};

    #[test]
    fn plain_runs_have_no_effects() {
        assert_eq!(TextFx::of(&RunProps::default()), None);
        let rp = RunProps {
            scale: Some(150),
            ..RunProps::default()
        };
        assert!((TextFx::of(&rp).unwrap().scale_x - 1.5).abs() < 1e-6);
    }

    #[test]
    fn w14_effects_in_points() {
        let red = FxColor::rgb(Color { r: 255, g: 0, b: 0 });
        let rp = RunProps {
            effects: Some(Box::new(TextEffects {
                glow: Some(TextGlow {
                    radius: 127_000, // 10 pt
                    color: red.clone(),
                    xml: None,
                }),
                shadow: Some(TextShadow {
                    blur: 25_400,
                    dist: 38_100,   // 3 pt
                    dir: 5_400_000, // 90°: straight down
                    color: red,
                    xml: None,
                }),
                ..TextEffects::default()
            })),
            ..RunProps::default()
        };
        let fx = TextFx::of(&rp).unwrap();
        assert!((fx.glow.unwrap().0 - 10.0).abs() < 1e-4);
        let s = fx.soft_shadow.unwrap();
        assert!(s.dx.abs() < 1e-4 && (s.dy - 3.0).abs() < 1e-4 && (s.blur - 2.0).abs() < 1e-4);
    }

    #[test]
    fn pattern_shading_mix() {
        // pct25 black on white: 75 % of 255 = 191.25 → 191.
        let rp = RunProps {
            shading: Some(Color::WHITE),
            shading_pattern: Some(ShadingPattern {
                val: "pct25".into(),
                color: None,
            }),
            ..RunProps::default()
        };
        assert_eq!(
            shading_color(&rp),
            Some(Color {
                r: 191,
                g: 191,
                b: 191
            })
        );
    }
}
