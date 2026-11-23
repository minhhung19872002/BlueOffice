//! Shape Format of a document shape, round 20 (docs/ui/REPORT.md "Vòng 110"): Shape Fill ›
//! Gradient / Texture / Picture, transparency of the fill and the line, Shape Effects
//! (shadow, reflection, glow, soft edges, bevel, 3-D rotation, presets — the Picture
//! Effects of round 19 on a shape) and the WordArt Styles of the shape's text (Text Fill,
//! Text Outline, Text Effects: [MS-DOCX] w14 run effects). One undo step per command; the
//! model is `bo_model::shape_style` (written to .docx by `bo-docx`), the text effects are
//! the runs' `bo_edit::charfx` effects.
//!
//! JSON commands (through `Engine::command_json`):
//! `{"cmd":"shapeGradient","variation":"light"|"dark","index":0..8}` (the gallery: the fill
//! colour lighter / darker in nine directions), or with `"kind":"linear"|"circle"|"rect"|
//! "shape","angle":deg,"stops":[{"pos":0..100,"color":"RRGGBB","alpha":0..100}]`, or
//! `"variation":"none"` (back to the solid colour);
//! `{"cmd":"shapePictureFill","data":"<base64>","tile":bool,"scale":percent,"alpha":percent}`;
//! `{"cmd":"shapeTransparency","fill":percent,"line":percent}`;
//! `{"cmd":"shapeEffect","kind":…,"preset":…}` (as `pictureEffect`);
//! `{"cmd":"shapeTextFill","color":"RRGGBB"|"none","gradient":[from,to,angle]?}`;
//! `{"cmd":"shapeTextOutline","color":"RRGGBB"|"none","weight":pt?}`;
//! `{"cmd":"shapeTextEffect","kind":"shadow"|"reflection"|"glow"|"none","preset":…}`;
//! `{"cmd":"wordArtStyle","index":n}` (BlueOffice's own WordArt quick styles).

use crate::edit::EditResult;
use crate::picture_tools::PictureCommand;
use crate::{EngineError, Result};
use bo_edit::charfx::{
    CharAdvancedPatch, GlowSpec, GradientSpec, OutlineSpec, ReflectionSpec, ShadowSpec,
};
use bo_model::shape_style::{ShapeGradient, ShapePicture, ShapeStop};
use bo_model::{Block, Color, Image};
use serde_json::Value;

/// A Shape Format command of round 20.
#[derive(Debug, Clone, PartialEq)]
pub enum ShapeStyleCommand {
    /// Gradient fill.
    Gradient(Value),
    /// Picture / texture fill: bytes, tiled, tile scale %, opacity %.
    Picture(Vec<u8>, bool, f64, f64),
    /// Transparency % of the fill and of the line.
    Transparency(Option<f64>, Option<f64>),
    /// Shape Effects kind, preset.
    Effect(String, String),
    /// Text Fill: colour (`None` = no fill), or a two-colour gradient.
    TextFill(Option<String>, Option<(String, String, f32)>),
    /// Text Outline: colour (`None` = no outline), weight pt.
    TextOutline(Option<String>, Option<f32>),
    /// Text Effects kind, preset.
    TextEffect(String, String),
    /// WordArt quick style.
    WordArt(usize),
    /// Text › Align Text: "t", "ctr" or "b" (§21.1.2.1.1 bodyPr anchor).
    Anchor(String),
}

impl ShapeStyleCommand {
    /// Read the JSON command (see the module docs); `None` when it is not one.
    pub fn parse(v: &Value) -> Option<ShapeStyleCommand> {
        let s = |k: &str| v.get(k).and_then(Value::as_str).map(str::to_owned);
        let f = |k: &str| v.get(k).and_then(Value::as_f64);
        Some(match v.get("cmd")?.as_str()? {
            "shapeGradient" => ShapeStyleCommand::Gradient(v.clone()),
            "shapePictureFill" => {
                use base64::Engine as _;
                let data = base64::engine::general_purpose::STANDARD
                    .decode(s("data")?.trim())
                    .ok()?;
                ShapeStyleCommand::Picture(
                    data,
                    v.get("tile").and_then(Value::as_bool).unwrap_or(false),
                    f("scale").unwrap_or(100.0),
                    f("alpha").unwrap_or(100.0),
                )
            }
            "shapeTransparency" => ShapeStyleCommand::Transparency(f("fill"), f("line")),
            "shapeEffect" => ShapeStyleCommand::Effect(s("kind")?, s("preset")?),
            "shapeTextFill" => {
                let grad = v.get("gradient").and_then(Value::as_array).and_then(|a| {
                    Some((
                        a.first()?.as_str()?.to_owned(),
                        a.get(1)?.as_str()?.to_owned(),
                        a.get(2).and_then(Value::as_f64).unwrap_or(90.0) as f32,
                    ))
                });
                ShapeStyleCommand::TextFill(s("color").filter(|c| c != "none"), grad)
            }
            "shapeTextOutline" => ShapeStyleCommand::TextOutline(
                s("color").filter(|c| c != "none"),
                f("weight").map(|w| w as f32),
            ),
            "shapeTextEffect" => ShapeStyleCommand::TextEffect(
                s("kind")?,
                s("preset").unwrap_or_else(|| "none".into()),
            ),
            "shapeTextAnchor" => ShapeStyleCommand::Anchor(s("anchor")?),
            "wordArtStyle" => {
                ShapeStyleCommand::WordArt(usize::try_from(v.get("index")?.as_u64()?).ok()?)
            }
            _ => return None,
        })
    }
}

fn rgb_u32(c: Color) -> u32 {
    (u32::from(c.r) << 16) | (u32::from(c.g) << 8) | u32::from(c.b)
}

/// `c` mixed towards white (`t` > 0) or black (`t` < 0) by |t|.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub fn tone(c: u32, t: f64) -> u32 {
    let ch = |s: u32| {
        let v = f64::from((c >> s) & 255);
        let out = if t >= 0.0 {
            v + (255.0 - v) * t
        } else {
            v * (1.0 + t)
        };
        (out.round().clamp(0.0, 255.0) as u32) << s
    };
    ch(16) | ch(8) | ch(0)
}

/// The nine directions of the Gradient gallery: (kind, angle in degrees) — top-left to
/// bottom-right, down, top-right to bottom-left, right, from the centre, left, bottom-left
/// to top-right, up, bottom-right to top-left.
pub const DIRECTIONS: [(&str, f64); 9] = [
    ("linear", 45.0),
    ("linear", 90.0),
    ("linear", 135.0),
    ("linear", 0.0),
    ("circle", 0.0),
    ("linear", 180.0),
    ("linear", 315.0),
    ("linear", 270.0),
    ("linear", 225.0),
];

/// The gallery gradient `index` (0..9) of base colour `base`: lighter or darker variation.
pub fn variation(base: u32, dark: bool, index: usize) -> ShapeGradient {
    let (kind, angle) = DIRECTIONS[index.min(DIRECTIONS.len() - 1)];
    let (a, b) = if dark {
        (base, tone(base, -0.45))
    } else {
        (tone(base, 0.6), base)
    };
    #[allow(clippy::cast_possible_truncation)]
    ShapeGradient {
        kind: kind.to_owned(),
        angle: (angle * 60_000.0).round() as i32,
        focus: if kind == "linear" {
            [0; 4]
        } else {
            [50_000; 4]
        },
        stops: vec![
            ShapeStop {
                pos: 0,
                color: a,
                alpha: 100_000,
            },
            ShapeStop {
                pos: 100_000,
                color: b,
                alpha: 100_000,
            },
        ],
    }
}

/// A gradient from the command's fields.
#[allow(clippy::cast_possible_truncation)]
fn gradient_of(v: &Value, base: u32) -> std::result::Result<Option<ShapeGradient>, String> {
    match v.get("variation").and_then(Value::as_str) {
        Some("none") => return Ok(None),
        Some(var @ ("light" | "dark")) => {
            let i = v.get("index").and_then(Value::as_u64).unwrap_or(0) as usize;
            if i >= DIRECTIONS.len() {
                return Err("badPreset".into());
            }
            return Ok(Some(variation(base, var == "dark", i)));
        }
        Some(_) => return Err("badPreset".into()),
        None => {}
    }
    let stops: Vec<ShapeStop> = v
        .get("stops")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|s| {
                    Some(ShapeStop {
                        pos: (s.get("pos")?.as_f64()?.clamp(0.0, 100.0) * 1000.0).round() as i32,
                        color: u32::from_str_radix(s.get("color")?.as_str()?, 16).ok()?,
                        alpha: (s
                            .get("alpha")
                            .and_then(Value::as_f64)
                            .unwrap_or(100.0)
                            .clamp(0.0, 100.0)
                            * 1000.0)
                            .round() as i32,
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    if stops.len() < 2 {
        return Err("badGradient".into());
    }
    let kind = v
        .get("kind")
        .and_then(Value::as_str)
        .unwrap_or("linear")
        .to_owned();
    if !matches!(kind.as_str(), "linear" | "circle" | "rect" | "shape") {
        return Err("badGradient".into());
    }
    let angle = v.get("angle").and_then(Value::as_f64).unwrap_or(90.0);
    let mut stops = stops;
    stops.sort_by_key(|s| s.pos);
    // `focus`: fillToRect l, t, r, b in percent (path gradients; default the centre).
    let focus = v.get("focus").and_then(Value::as_array).and_then(|a| {
        let n: Vec<i32> = a
            .iter()
            .filter_map(Value::as_f64)
            .map(|x| (x.clamp(-100.0, 200.0) * 1000.0).round() as i32)
            .collect();
        <[i32; 4]>::try_from(n).ok()
    });
    Ok(Some(ShapeGradient {
        focus: if kind == "linear" {
            [0; 4]
        } else {
            focus.unwrap_or([50_000; 4])
        },
        kind,
        angle: (angle.rem_euclid(360.0) * 60_000.0).round() as i32,
        stops,
    }))
}

/// Picture extension from its first bytes.
fn picture_ext(b: &[u8]) -> Option<&'static str> {
    if b.starts_with(b"\x89PNG") {
        Some("png")
    } else if b.starts_with(&[0xFF, 0xD8]) {
        Some("jpeg")
    } else if b.starts_with(b"GIF8") {
        Some("gif")
    } else if b.starts_with(b"BM") {
        Some("bmp")
    } else {
        None
    }
}

/// The WordArt quick styles (BlueOffice's own): text fill, outline (colour, pt), shadow,
/// glow, reflection, gradient (from, to).
#[allow(clippy::type_complexity)]
pub const WORDART: [(
    Option<&str>,
    Option<(&str, f32)>,
    bool,
    Option<&str>,
    bool,
    Option<(&str, &str)>,
); 12] = [
    (Some("000000"), None, false, None, false, None),
    (Some("4472C4"), None, true, None, false, None),
    (
        Some("ED7D31"),
        Some(("FFFFFF", 0.5)),
        true,
        None,
        false,
        None,
    ),
    (
        Some("FFFFFF"),
        Some(("4472C4", 1.0)),
        true,
        None,
        false,
        None,
    ),
    (
        Some("FFC000"),
        Some(("C55A11", 0.75)),
        false,
        None,
        false,
        None,
    ),
    (Some("000000"), None, false, None, true, None),
    (
        Some("FFFFFF"),
        Some(("000000", 0.5)),
        true,
        None,
        false,
        None,
    ),
    (None, Some(("4472C4", 1.0)), false, None, false, None),
    (Some("4472C4"), None, false, Some("FFC000"), false, None),
    (Some("A5A5A5"), None, true, None, true, None),
    (None, None, false, None, false, Some(("ED7D31", "FFC000"))),
    (None, None, true, None, false, Some(("4472C4", "70AD47"))),
];

/// Text shadows of Text Effects › Shadow: (name, blur pt, distance pt, angle °, transparency %).
pub const TEXT_SHADOWS: [(&str, f32, f32, f32, u32); 6] = [
    ("offsetBottomRight", 4.0, 3.0, 45.0, 60),
    ("offsetBottom", 4.0, 3.0, 90.0, 60),
    ("offsetRight", 4.0, 3.0, 0.0, 60),
    ("offsetCenter", 5.0, 0.0, 0.0, 60),
    ("offsetTopLeft", 4.0, 3.0, 225.0, 60),
    ("perspective", 6.0, 8.0, 60.0, 70),
];

/// Text reflections: (name, size %, transparency %, distance pt).
pub const TEXT_REFLECTIONS: [(&str, u32, u32, f32); 3] = [
    ("tight", 25, 50, 0.0),
    ("half", 50, 50, 1.0),
    ("full", 90, 50, 2.0),
];

/// Text glows: (radius pt, colour, transparency %).
pub const TEXT_GLOWS: [(f32, &str, u32); 6] = [
    (5.0, "4472C4", 40),
    (5.0, "ED7D31", 40),
    (5.0, "FFC000", 40),
    (8.0, "70AD47", 40),
    (8.0, "5B9BD5", 40),
    (11.0, "A5A5A5", 40),
];

/// Apply `f` to every run of `blocks` (paragraphs and table cells).
fn each_run(blocks: &mut [Block], f: &dyn Fn(&mut bo_model::RunProps)) {
    for b in blocks {
        match b {
            Block::Paragraph(p) => {
                for r in &mut p.runs {
                    f(&mut r.props);
                }
                // The paragraph mark too: text typed later takes the style.
                f(p.props.mark_run.get_or_insert_with(Box::default));
            }
            Block::Table(t) => {
                for row in &mut t.rows {
                    for cell in &mut row.cells {
                        each_run(&mut cell.blocks, f);
                    }
                }
            }
            _ => {}
        }
    }
}

/// The run patch of a text command.
#[allow(clippy::cast_possible_truncation)]
fn text_patch(
    cmd: &ShapeStyleCommand,
) -> std::result::Result<(CharAdvancedPatch, Option<Option<Color>>), String> {
    let mut p = CharAdvancedPatch::default();
    let mut color: Option<Option<Color>> = None;
    match cmd {
        ShapeStyleCommand::TextFill(c, grad) => {
            if let Some((from, to, angle)) = grad {
                p.gradient = Some(Some(GradientSpec {
                    from: from.clone(),
                    to: to.clone(),
                    angle: *angle,
                }));
            } else {
                p.gradient = Some(None);
                color = Some(match c {
                    Some(c) => Some(Color::from_hex(c).ok_or("badColor")?),
                    // No Fill: white text with an outline, as hollow text reads.
                    None => Some(Color::WHITE),
                });
            }
        }
        ShapeStyleCommand::TextOutline(c, w) => {
            p.text_outline = Some(c.as_ref().map(|c| OutlineSpec {
                color: c.clone(),
                width: w.unwrap_or(0.75).clamp(0.25, 20.0),
            }));
        }
        ShapeStyleCommand::TextEffect(kind, preset) => {
            let none = preset == "none";
            match kind.as_str() {
                "shadow" => {
                    p.soft_shadow = Some(if none {
                        None
                    } else {
                        let (_, blur, distance, angle, transparency) = *TEXT_SHADOWS
                            .iter()
                            .find(|s| s.0 == preset)
                            .ok_or("badPreset")?;
                        Some(ShadowSpec {
                            color: "000000".into(),
                            blur,
                            distance,
                            angle,
                            transparency,
                        })
                    });
                }
                "reflection" => {
                    p.reflection = Some(if none {
                        None
                    } else {
                        let (_, size, transparency, distance) = *TEXT_REFLECTIONS
                            .iter()
                            .find(|s| s.0 == preset)
                            .ok_or("badPreset")?;
                        Some(ReflectionSpec {
                            size,
                            transparency,
                            distance,
                        })
                    });
                }
                "glow" => {
                    p.glow = Some(if none {
                        None
                    } else {
                        let i: usize = preset.parse().map_err(|_| "badPreset")?;
                        let (radius, c, transparency) = *TEXT_GLOWS.get(i).ok_or("badPreset")?;
                        Some(GlowSpec {
                            color: c.into(),
                            radius,
                            transparency,
                        })
                    });
                }
                "none" => {
                    p.soft_shadow = Some(None);
                    p.reflection = Some(None);
                    p.glow = Some(None);
                }
                _ => return Err("badKind".into()),
            }
        }
        ShapeStyleCommand::WordArt(i) => {
            let (fill, outline, shadow, glow, refl, grad) = *WORDART.get(*i).ok_or("badPreset")?;
            color = Some(fill.and_then(Color::from_hex).or(Some(Color::WHITE)));
            p.text_outline = Some(outline.map(|(c, w)| OutlineSpec {
                color: c.into(),
                width: w,
            }));
            p.soft_shadow = Some(shadow.then(|| ShadowSpec {
                color: "000000".into(),
                blur: 4.0,
                distance: 3.0,
                angle: 45.0,
                transparency: 60,
            }));
            p.glow = Some(glow.map(|c| GlowSpec {
                color: c.into(),
                radius: 5.0,
                transparency: 40,
            }));
            p.reflection = Some(refl.then(|| ReflectionSpec {
                size: 25,
                transparency: 50,
                distance: 0.0,
            }));
            p.gradient = Some(grad.map(|(from, to)| GradientSpec {
                from: from.into(),
                to: to.into(),
                angle: 90.0,
            }));
            if fill.is_none()
                && grad.is_none()
                && p.text_outline.as_ref().is_some_and(Option::is_none)
            {
                p.text_outline = Some(Some(OutlineSpec {
                    color: "000000".into(),
                    width: 0.75,
                }));
            }
        }
        _ => {}
    }
    Ok((p, color))
}

/// Apply `cmd` to shape `img`; new media parts go into `parts`.
#[allow(clippy::cast_possible_truncation)]
fn apply(
    cmd: &ShapeStyleCommand,
    img: &mut Image,
    picture_name: Option<String>,
) -> std::result::Result<(), String> {
    let Some(sh) = img.shape.as_deref_mut() else {
        return Err("notShape".into());
    };
    let mut st = sh.style.as_deref().cloned().unwrap_or_default();
    let base = sh.fill.map_or(0x44_72C4, rgb_u32);
    let pct = |v: f64| ((100.0 - v.clamp(0.0, 100.0)) * 1000.0).round() as i32;
    match cmd {
        ShapeStyleCommand::Gradient(v) => {
            st.gradient = gradient_of(v, base)?;
            if let Some(g) = &st.gradient {
                st.picture = None;
                // The solid fallback: the first stop.
                sh.fill = g.stops.first().map(|s| Color {
                    r: ((s.color >> 16) & 255) as u8,
                    g: ((s.color >> 8) & 255) as u8,
                    b: (s.color & 255) as u8,
                });
            }
        }
        ShapeStyleCommand::Picture(_, tile, scale, alpha) => {
            let media = picture_name.ok_or("badPicture")?;
            st.gradient = None;
            st.picture = Some(ShapePicture {
                media,
                tile: *tile,
                scale: (scale.clamp(1.0, 1000.0) * 1000.0).round() as i32,
                alpha: pct(100.0 - alpha.clamp(0.0, 100.0)),
            });
            if sh.fill.is_none() {
                sh.fill = Some(Color::WHITE);
            }
        }
        ShapeStyleCommand::Transparency(fill, line) => {
            if let Some(f) = fill {
                st.fill_alpha = Some(pct(*f)).filter(|a| *a < 100_000);
                if let Some(p) = st.picture.as_mut() {
                    p.alpha = pct(*f);
                }
                if let Some(g) = st.gradient.as_mut() {
                    for s in &mut g.stops {
                        s.alpha = pct(*f);
                    }
                }
            }
            if let Some(l) = line {
                st.line_alpha = Some(pct(*l)).filter(|a| *a < 100_000);
            }
        }
        ShapeStyleCommand::Anchor(a) => {
            sh.anchor = match a.as_str() {
                "t" => bo_model::TextAnchor::Top,
                "ctr" => bo_model::TextAnchor::Center,
                "b" => bo_model::TextAnchor::Bottom,
                _ => return Err("badAnchor".into()),
            };
        }
        ShapeStyleCommand::Effect(kind, preset) => {
            st.effects = crate::doc_picstyle::apply_effect(kind, preset, st.effects.take())?;
        }
        _ => {
            let (patch, color) = text_patch(cmd)?;
            let Some(blocks) = sh.text.as_mut() else {
                return Err("noText".into());
            };
            each_run(blocks, &|r| {
                patch.apply(r);
                if let Some(c) = color {
                    r.color = c;
                    r.theme_color = None;
                }
            });
        }
    }
    sh.style = (!st.is_empty()).then(|| Box::new(st));
    Ok(())
}

impl crate::Engine {
    /// Run a round-20 Shape Format command on the document shape at the selection.
    pub fn doc_shape_style(&mut self, cmd: &ShapeStyleCommand) -> Result<EditResult> {
        if self.pres.is_some() {
            return Err(EngineError::BadCommand("not a document".into()));
        }
        let read_only = self.sec.read_only;
        let s = self.session.as_mut().ok_or(EngineError::NoDocument)?;
        let error = if read_only {
            Some(crate::security::READ_ONLY.to_owned())
        } else if s.tools_shape().is_none() {
            Some("noShape".to_owned())
        } else {
            let parts: Vec<(String, std::sync::Arc<Vec<u8>>)> = match cmd {
                ShapeStyleCommand::Picture(data, ..) => match picture_ext(data) {
                    Some(ext) => vec![(s.new_media_name(ext), std::sync::Arc::new(data.clone()))],
                    None => Vec::new(),
                },
                _ => Vec::new(),
            };
            let name = parts.first().map(|p| p.0.clone());
            // Check first so that a failed command leaves no undo step.
            let mut probe = s.tools_shape().ok_or(EngineError::NoDocument)?;
            match apply(cmd, &mut probe, name.clone()) {
                Err(e) => Some(e),
                Ok(()) => {
                    let mut failed = None;
                    let r = s.update_shape_parts(parts, |img| {
                        if let Err(e) = apply(cmd, img, name) {
                            failed = Some(e);
                        }
                    });
                    failed.or_else(|| r.err().map(|e| e.to_string()))
                }
            }
        };
        let mut r = self.doc_picture_command(PictureCommand::PictureInfo)?;
        if error.is_some() {
            r.error = error;
        }
        Ok(r)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Engine;
    use bo_model::shape_style::ShapeStyle;
    use serde_json::json;

    /// A document with one rectangle, selected.
    fn engine_with_shape() -> Engine {
        let mut e = Engine::with_bundled_fonts();
        e.new_document("s.docx").expect("doc");
        for json in [
            r#"{"cmd":"insertShape","preset":"rect","width":144,"height":72}"#,
            r#"{"cmd":"move","motion":"left","extend":true}"#,
        ] {
            e.command_json(json).expect("command");
        }
        e
    }

    /// The first shape's style of a saved document read back.
    fn saved_style(
        e: &mut Engine,
    ) -> (
        Option<ShapeStyle>,
        std::collections::BTreeMap<String, std::sync::Arc<Vec<u8>>>,
    ) {
        let bytes = e.save_docx().expect("save");
        let mut back = Engine::with_bundled_fonts();
        back.open(&bytes, "s.docx").expect("open");
        let d = back.document().expect("document");
        let st = d.sections[0].blocks.iter().find_map(|b| match b {
            Block::Paragraph(p) => p.runs.iter().find_map(|r| {
                r.content.iter().find_map(|c| match c {
                    bo_model::RunContent::Image(i) => {
                        i.shape.as_deref().and_then(|s| s.style.as_deref().cloned())
                    }
                    _ => None,
                })
            }),
            _ => None,
        });
        (st, d.media.clone())
    }

    fn shape(e: &Engine) -> Option<Image> {
        e.session().and_then(bo_edit::Session::tools_shape)
    }

    fn run(e: &mut Engine, v: &Value) -> Value {
        let out = e.command_json(&v.to_string()).expect("command");
        serde_json::from_str(&out).unwrap_or(Value::Null)
    }

    #[test]
    fn gradients_pictures_effects_survive_docx() {
        let mut e = engine_with_shape();
        assert!(shape(&e).is_some(), "the shape is selected");
        run(
            &mut e,
            &json!({"cmd": "shapeGradient", "variation": "dark", "index": 4}),
        );
        let st = shape(&e)
            .and_then(|i| i.shape)
            .and_then(|s| s.style)
            .expect("style");
        let g = st.gradient.as_ref().expect("gradient");
        assert_eq!(g.kind, "circle");
        run(
            &mut e,
            &json!({"cmd": "shapeTransparency", "fill": 40, "line": 25}),
        );
        run(
            &mut e,
            &json!({"cmd": "shapeEffect", "kind": "shadow", "preset": "outerBottomRight"}),
        );
        run(
            &mut e,
            &json!({"cmd": "shapeEffect", "kind": "glow", "preset": "8:4472C4"}),
        );
        let before = shape(&e)
            .and_then(|i| i.shape)
            .and_then(|s| s.style)
            .expect("style");
        assert_eq!(before.line_alpha, Some(75_000));
        assert!(before
            .effects
            .as_ref()
            .is_some_and(|f| f.shadow.is_some() && f.glow.is_some()));
        // Saved and read back.
        let after = saved_style(&mut e).0.expect("styled shape read back");
        assert_eq!(after.gradient, before.gradient);
        assert_eq!(after.line_alpha, before.line_alpha);
        assert_eq!(
            after.effects.as_ref().and_then(|f| f.glow.clone()),
            before.effects.as_ref().and_then(|f| f.glow.clone())
        );
        // Undo goes back step by step.
        e.command(crate::edit::Command::Undo).expect("undo");
        let st = shape(&e)
            .and_then(|i| i.shape)
            .and_then(|s| s.style)
            .expect("style");
        assert!(st.effects.as_ref().is_some_and(|f| f.glow.is_none()));
    }

    #[test]
    fn picture_and_texture_fills_add_media() {
        let mut e = engine_with_shape();
        assert!(shape(&e).is_some(), "the shape is selected");
        let png = bo_raster::rgba_to_png(bo_raster::codecs::Rgba {
            width: 2,
            height: 2,
            pixels: vec![255; 16],
        })
        .expect("png");
        use base64::Engine as _;
        let data = base64::engine::general_purpose::STANDARD.encode(&png);
        run(
            &mut e,
            &json!({"cmd": "shapePictureFill", "data": data, "tile": true, "scale": 50}),
        );
        let st = shape(&e)
            .and_then(|i| i.shape)
            .and_then(|s| s.style)
            .expect("style");
        let p = st.picture.expect("picture");
        assert!(p.tile);
        assert_eq!(p.scale, 50_000);
        assert!(e.session().expect("s").doc().media.contains_key(&p.media));
        let (st, media) = saved_style(&mut e);
        let pic = st.and_then(|s| s.picture).expect("picture fill read back");
        assert!(pic.tile);
        assert!(media.contains_key(&pic.media));
    }

    #[test]
    fn variations_and_tones() {
        assert_eq!(tone(0x80_8080, 0.0), 0x80_8080);
        assert_eq!(tone(0x00_0000, 1.0), 0xFF_FFFF);
        assert_eq!(tone(0xFF_FFFF, -1.0), 0);
        let g = variation(0x44_72C4, false, 1);
        assert_eq!(g.angle, 5_400_000);
        assert_eq!(g.stops[1].color, 0x44_72C4);
        assert!(ShapeStyleCommand::parse(&json!({"cmd": "wordArtStyle", "index": 3})).is_some());
        assert!(ShapeStyleCommand::parse(&json!({"cmd": "pictureBorder"})).is_none());
    }
}
