//! Picture Format › Picture Styles of a document picture (BlueDoc round 19, docs/ui/REPORT.md
//! "Vòng 109"): Picture Border, Picture Effects (shadow, reflection, glow, soft edges, bevel,
//! 3-D rotation, presets), Artistic Effects and the Quick Styles gallery. One undo step per
//! command; the model is `bo_model::picture_style`, written to .docx by `bo-docx`.
//!
//! JSON commands (through `Engine::command_json`):
//! `{"cmd":"pictureBorder","color":"RRGGBB"|"none","weight":pt,"dash":"solid"|…}`,
//! `{"cmd":"pictureEffect","kind":"shadow"|"reflection"|"glow"|"softEdge"|"bevel"|"rotation"|"preset","preset":"<name>"|"none"}`,
//! `{"cmd":"pictureArtistic","name":"artisticBlur"|…|"none"}`,
//! `{"cmd":"pictureQuickStyle","index":n}`.
//!
//! The preset values are BlueOffice's own choices.

use crate::edit::EditResult;
use crate::picture_tools::PictureCommand;
use crate::{EngineError, Result};
use bo_model::picture::PictureFormat;
use bo_model::picture_style::{
    PicArtistic, PicBevel, PicEffects, PicGlow, PicLine, PicReflection, PicShadow, ARTISTIC,
};
use bo_model::Image;

const EMU_PT: f64 = 12_700.0;

#[allow(clippy::cast_possible_truncation)]
fn emu(pt: f64) -> i64 {
    (pt * EMU_PT).round() as i64
}

/// Shadow presets: (name, inner, direction in degrees, distance pt, blur pt).
pub const SHADOWS: [(&str, bool, f64, f64, f64); 18] = [
    ("outerBottomRight", false, 45.0, 3.0, 4.0),
    ("outerBottom", false, 90.0, 3.0, 4.0),
    ("outerBottomLeft", false, 135.0, 3.0, 4.0),
    ("outerRight", false, 0.0, 3.0, 4.0),
    ("outerCenter", false, 0.0, 0.0, 5.0),
    ("outerLeft", false, 180.0, 3.0, 4.0),
    ("outerTopRight", false, 315.0, 3.0, 4.0),
    ("outerTop", false, 270.0, 3.0, 4.0),
    ("outerTopLeft", false, 225.0, 3.0, 4.0),
    ("innerTopLeft", true, 45.0, 4.0, 5.0),
    ("innerTop", true, 90.0, 4.0, 5.0),
    ("innerTopRight", true, 135.0, 4.0, 5.0),
    ("innerLeft", true, 0.0, 4.0, 5.0),
    ("innerCenter", true, 0.0, 0.0, 9.0),
    ("innerRight", true, 180.0, 4.0, 5.0),
    ("innerBottomLeft", true, 315.0, 4.0, 5.0),
    ("innerBottom", true, 270.0, 4.0, 5.0),
    ("innerBottomRight", true, 225.0, 4.0, 5.0),
];

/// Reflection presets: (name, end position %, distance pt).
pub const REFLECTIONS: [(&str, i32, f64); 9] = [
    ("tightTouching", 35, 0.0),
    ("halfTouching", 55, 0.0),
    ("fullTouching", 90, 0.0),
    ("tight4", 35, 4.0),
    ("half4", 55, 4.0),
    ("full4", 90, 4.0),
    ("tight8", 35, 8.0),
    ("half8", 55, 8.0),
    ("full8", 90, 8.0),
];

/// Glow sizes (pt) and colours of the Glow menu.
pub const GLOW_SIZES: [f64; 4] = [5.0, 8.0, 11.0, 18.0];
/// Glow colours (accent colours of the default theme).
pub const GLOW_COLORS: [u32; 6] = [
    0x44_72C4, 0xED_7D31, 0xA5_A5A5, 0xFF_C000, 0x5B_9BD5, 0x70_AD47,
];
/// Soft edge radii (pt).
pub const SOFT_EDGES: [f64; 6] = [1.0, 2.5, 5.0, 10.0, 25.0, 50.0];
/// Bevel presets (§20.1.10.9 ST_BevelPresetType).
pub const BEVELS: [&str; 12] = [
    "circle",
    "relaxedInset",
    "cross",
    "coolSlant",
    "angle",
    "softRound",
    "convex",
    "slope",
    "divot",
    "riblet",
    "hardEdge",
    "artDeco",
];
/// 3-D Rotation presets (§20.1.10.47 ST_PresetCameraType): parallel, perspective, oblique.
pub const ROTATIONS: [&str; 22] = [
    "isometricOffAxis1Right",
    "isometricOffAxis2Left",
    "isometricOffAxis1Left",
    "isometricOffAxis2Right",
    "isometricLeftDown",
    "isometricRightUp",
    "isometricTopUp",
    "isometricBottomDown",
    "perspectiveFront",
    "perspectiveLeft",
    "perspectiveRight",
    "perspectiveBelow",
    "perspectiveAbove",
    "perspectiveRelaxedModerately",
    "perspectiveRelaxed",
    "perspectiveContrastingLeftFacing",
    "perspectiveContrastingRightFacing",
    "perspectiveHeroicExtremeLeftFacing",
    "obliqueTopLeft",
    "obliqueTopRight",
    "obliqueBottomLeft",
    "obliqueBottomRight",
];

/// Round 21 (the Format Shape / Format Picture panes): `custom:key=value;…` settings
/// of an effect instead of a preset name (numbers in pt, %, degrees; colours `RRGGBB`).
fn custom(preset: &str) -> Option<Vec<(&str, &str)>> {
    let body = preset.strip_prefix("custom:")?;
    Some(
        body.split(';')
            .filter_map(|kv| kv.split_once('='))
            .map(|(k, v)| (k.trim(), v.trim()))
            .collect(),
    )
}

/// A number of `custom` settings.
fn num(kv: &[(&str, &str)], key: &str, default: f64) -> f64 {
    kv.iter()
        .find(|(k, _)| *k == key)
        .and_then(|(_, v)| v.parse::<f64>().ok())
        .filter(|v| v.is_finite())
        .unwrap_or(default)
}

/// A colour of `custom` settings.
fn hex(kv: &[(&str, &str)], key: &str, default: u32) -> u32 {
    kv.iter()
        .find(|(k, _)| *k == key)
        .and_then(|(_, v)| u32::from_str_radix(v, 16).ok())
        .map_or(default, |c| c & 0xFF_FFFF)
}

/// Percent → 1/1000 %.
#[allow(clippy::cast_possible_truncation)]
fn pct(v: f64) -> i32 {
    (v.clamp(0.0, 100.0) * 1000.0).round() as i32
}

/// §20.1.8.49 outerShdw / §20.1.8.40 innerShdw from `custom` settings: `inner` (0/1),
/// `color`, `transparency` %, `size` %, `blur` pt, `angle` degrees, `distance` pt.
#[allow(clippy::cast_possible_truncation)]
fn custom_shadow(kv: &[(&str, &str)]) -> PicShadow {
    let inner = num(kv, "inner", 0.0) != 0.0;
    let size = pct(num(kv, "size", 100.0).clamp(1.0, 200.0) / 2.0) * 2;
    PicShadow {
        inner,
        blur: emu(num(kv, "blur", 4.0).clamp(0.0, 100.0)),
        dist: emu(num(kv, "distance", 3.0).clamp(0.0, 200.0)),
        dir: (num(kv, "angle", 45.0).rem_euclid(360.0) * 60_000.0).round() as i32,
        color: hex(kv, "color", 0),
        alpha: pct(100.0 - num(kv, "transparency", 60.0)),
        sx: if inner { 0 } else { size },
        sy: if inner { 0 } else { size },
    }
}

fn shadow(name: &str) -> Option<PicShadow> {
    if let Some(kv) = custom(name) {
        return Some(custom_shadow(&kv));
    }
    let (_, inner, dir, dist, blur) = SHADOWS.iter().find(|s| s.0 == name)?;
    #[allow(clippy::cast_possible_truncation)]
    Some(PicShadow {
        inner: *inner,
        blur: emu(*blur),
        dist: emu(*dist),
        dir: (dir * 60_000.0).round() as i32,
        color: 0,
        alpha: if *inner { 50_000 } else { 40_000 },
        sx: if *inner { 0 } else { 100_000 },
        sy: if *inner { 0 } else { 100_000 },
    })
}

fn reflection(name: &str) -> Option<PicReflection> {
    // Round 21: `transparency` % (of the start), `size` % (end position), `blur` pt,
    // `distance` pt.
    if let Some(kv) = custom(name) {
        return Some(PicReflection {
            blur: emu(num(&kv, "blur", 0.5).clamp(0.0, 100.0)),
            st_a: pct(100.0 - num(&kv, "transparency", 48.0)),
            end_a: 300,
            end_pos: pct(num(&kv, "size", 55.0)),
            dist: emu(num(&kv, "distance", 0.0).clamp(0.0, 100.0)),
        });
    }
    let (_, end, dist) = REFLECTIONS.iter().find(|r| r.0 == name)?;
    Some(PicReflection {
        blur: emu(0.5),
        st_a: 52_000,
        end_a: 300,
        end_pos: end * 1000,
        dist: emu(*dist),
    })
}

/// `size:RRGGBB` → glow.
fn glow(preset: &str) -> Option<PicGlow> {
    // Round 21: `size` pt, `color`, `transparency` %.
    if let Some(kv) = custom(preset) {
        return Some(PicGlow {
            rad: emu(num(&kv, "size", 8.0).clamp(0.0, 150.0)),
            color: hex(&kv, "color", 0x44_72C4),
            alpha: pct(100.0 - num(&kv, "transparency", 60.0)),
        });
    }
    let (size, color) = preset.split_once(':')?;
    Some(PicGlow {
        rad: emu(size.parse::<f64>().ok()?.clamp(0.0, 150.0)),
        color: u32::from_str_radix(color, 16).ok()? & 0xFF_FFFF,
        alpha: 40_000,
    })
}

/// Picture Effects › Preset: (shadow, reflection, glow, soft edge, bevel, rotation).
type Preset = (
    Option<&'static str>,
    Option<&'static str>,
    Option<&'static str>,
    Option<f64>,
    Option<&'static str>,
    Option<&'static str>,
);
/// The presets of the Preset menu.
pub const PRESETS: [Preset; 12] = [
    (Some("outerBottomRight"), None, None, None, None, None),
    (Some("outerBottom"), None, None, None, Some("circle"), None),
    (None, None, None, None, Some("circle"), None),
    (None, None, None, None, Some("relaxedInset"), None),
    (
        Some("outerCenter"),
        None,
        None,
        None,
        Some("softRound"),
        None,
    ),
    (None, Some("tightTouching"), None, None, None, None),
    (None, None, Some("5:4472C4"), None, None, None),
    (None, None, None, Some(5.0), None, None),
    (
        Some("outerBottomRight"),
        None,
        None,
        None,
        Some("angle"),
        Some("perspectiveRelaxedModerately"),
    ),
    (
        None,
        Some("halfTouching"),
        None,
        None,
        Some("circle"),
        Some("perspectiveFront"),
    ),
    (Some("innerCenter"), None, None, None, None, None),
    (
        Some("outerBottomRight"),
        Some("tight4"),
        None,
        None,
        Some("convex"),
        Some("isometricOffAxis1Right"),
    ),
];

/// Quick Styles: (border colour, border pt, shadow, reflection, soft edge pt, bevel,
/// rotation, crop shape) — BlueOffice's own frames.
type Quick = (
    Option<u32>,
    f64,
    Option<&'static str>,
    Option<&'static str>,
    Option<f64>,
    Option<&'static str>,
    Option<&'static str>,
    Option<&'static str>,
);
/// The Quick Styles gallery.
pub const QUICK_STYLES: [Quick; 16] = [
    (
        Some(0xFF_FFFF),
        8.0,
        Some("outerBottomRight"),
        None,
        None,
        None,
        None,
        None,
    ),
    (Some(0x00_0000), 6.0, None, None, None, None, None, None),
    (None, 0.0, Some("outerCenter"), None, None, None, None, None),
    (None, 0.0, None, None, Some(10.0), None, None, None),
    (
        None,
        0.0,
        None,
        None,
        Some(25.0),
        None,
        None,
        Some("ellipse"),
    ),
    (
        None,
        0.0,
        Some("outerBottom"),
        Some("tight4"),
        None,
        None,
        None,
        Some("roundRect"),
    ),
    (
        Some(0xFF_FFFF),
        5.0,
        Some("outerBottomRight"),
        None,
        None,
        None,
        Some("perspectiveRelaxedModerately"),
        None,
    ),
    (
        None,
        0.0,
        None,
        Some("halfTouching"),
        None,
        None,
        None,
        None,
    ),
    (
        Some(0xBF_BFBF),
        3.0,
        None,
        None,
        None,
        Some("circle"),
        None,
        None,
    ),
    (
        Some(0x40_4040),
        4.5,
        Some("outerBottomRight"),
        None,
        None,
        Some("relaxedInset"),
        None,
        None,
    ),
    (
        Some(0xFF_FFFF),
        6.0,
        Some("outerCenter"),
        None,
        None,
        None,
        None,
        Some("ellipse"),
    ),
    (
        None,
        0.0,
        Some("outerBottomRight"),
        None,
        None,
        None,
        Some("perspectiveLeft"),
        None,
    ),
    (
        None,
        0.0,
        Some("outerBottomRight"),
        None,
        None,
        None,
        Some("perspectiveRight"),
        None,
    ),
    (
        Some(0xFF_FFFF),
        4.0,
        None,
        Some("tight4"),
        None,
        None,
        Some("perspectiveFront"),
        Some("roundRect"),
    ),
    (
        Some(0x00_0000),
        1.0,
        Some("innerCenter"),
        None,
        None,
        None,
        None,
        None,
    ),
    (
        None,
        0.0,
        None,
        None,
        Some(5.0),
        Some("softRound"),
        None,
        Some("roundRect"),
    ),
];

fn bevel(name: &str) -> Option<PicBevel> {
    // Round 21: `type` (a preset), `width` / `height` pt.
    if let Some(kv) = custom(name) {
        let prst = kv
            .iter()
            .find(|(k, _)| *k == "type")
            .map(|(_, v)| *v)
            .filter(|v| BEVELS.contains(v))?;
        return Some(PicBevel {
            prst: prst.to_owned(),
            w: emu(num(&kv, "width", 6.0).clamp(0.0, 1584.0)),
            h: emu(num(&kv, "height", 6.0).clamp(0.0, 1584.0)),
        });
    }
    BEVELS.contains(&name).then(|| PicBevel {
        prst: name.to_owned(),
        w: emu(6.0),
        h: emu(6.0),
    })
}

/// A picture style command.
#[derive(Debug, Clone, PartialEq)]
pub enum PicStyleCommand {
    /// Picture Border: colour (`None` keeps, `Some(None)` = no line), weight pt, dash.
    Border(Option<Option<u32>>, Option<f64>, Option<String>),
    /// Picture Effects: kind and preset (`none` clears it).
    Effect(String, String),
    /// Artistic Effects (`none` clears it).
    Artistic(String),
    /// Quick Styles entry.
    Quick(usize),
}

impl PicStyleCommand {
    /// Read the JSON command (see the module docs); `None` when it is not one.
    pub fn parse(v: &serde_json::Value) -> Option<PicStyleCommand> {
        let s = |k: &str| {
            v.get(k)
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
        };
        Some(match v.get("cmd")?.as_str()? {
            "pictureBorder" => PicStyleCommand::Border(
                s("color").map(|c| {
                    if c == "none" {
                        None
                    } else {
                        u32::from_str_radix(&c, 16).ok()
                    }
                }),
                v.get("weight").and_then(serde_json::Value::as_f64),
                s("dash"),
            ),
            "pictureEffect" => PicStyleCommand::Effect(s("kind")?, s("preset")?),
            "pictureArtistic" => PicStyleCommand::Artistic(s("name")?),
            "pictureQuickStyle" => {
                PicStyleCommand::Quick(usize::try_from(v.get("index")?.as_u64()?).ok()?)
            }
            _ => return None,
        })
    }
}

fn effects_mut(f: &mut PictureFormat) -> &mut PicEffects {
    f.effects.get_or_insert_with(Box::default)
}

fn tidy(f: &mut PictureFormat) {
    if f.effects.as_deref().is_some_and(PicEffects::is_empty) {
        f.effects = None;
    }
}

/// Apply `cmd` to a picture's format.
fn apply(cmd: &PicStyleCommand, f: &mut PictureFormat) -> std::result::Result<(), String> {
    match cmd {
        PicStyleCommand::Border(color, weight, dash) => {
            if *color == Some(None) {
                f.line = None;
                return Ok(());
            }
            let mut l = f.line.clone().unwrap_or(PicLine {
                color: 0,
                width: 9525,
                dash: None,
            });
            if let Some(Some(c)) = color {
                l.color = *c & 0xFF_FFFF;
            }
            if let Some(w) = weight {
                l.width = emu(w.clamp(0.25, 1584.0));
            }
            if let Some(d) = dash {
                l.dash = (d != "solid").then(|| d.clone());
            }
            f.line = Some(l);
        }
        PicStyleCommand::Effect(kind, preset) => {
            let none = preset == "none";
            let e = effects_mut(f);
            match kind.as_str() {
                "shadow" => {
                    e.shadow = if none {
                        None
                    } else {
                        Some(shadow(preset).ok_or("badPreset")?)
                    }
                }
                "reflection" => {
                    e.reflection = if none {
                        None
                    } else {
                        Some(reflection(preset).ok_or("badPreset")?)
                    };
                }
                "glow" => {
                    e.glow = if none {
                        None
                    } else {
                        Some(glow(preset).ok_or("badPreset")?)
                    }
                }
                "softEdge" => {
                    e.soft_edge = if none {
                        None
                    } else {
                        Some(emu(preset
                            .parse::<f64>()
                            .map_err(|_| "badPreset")?
                            .clamp(0.0, 300.0)))
                    };
                }
                "bevel" => {
                    e.bevel = if none {
                        None
                    } else {
                        Some(bevel(preset).ok_or("badPreset")?)
                    }
                }
                "rotation" => {
                    e.camera = if none {
                        None
                    } else if ROTATIONS.contains(&preset.as_str()) {
                        Some(preset.clone())
                    } else {
                        return Err("badPreset".into());
                    };
                }
                "preset" => {
                    *e = PicEffects::default();
                    if !none {
                        let i: usize = preset.parse().map_err(|_| "badPreset")?;
                        let (sh, rf, gl, se, bv, rot) = *PRESETS.get(i).ok_or("badPreset")?;
                        e.shadow = sh.and_then(shadow);
                        e.reflection = rf.and_then(reflection);
                        e.glow = gl.and_then(glow);
                        e.soft_edge = se.map(emu);
                        e.bevel = bv.and_then(bevel);
                        e.camera = rot.map(str::to_owned);
                    }
                }
                _ => return Err("badKind".into()),
            }
            tidy(f);
        }
        PicStyleCommand::Artistic(name) => {
            f.artistic = if name == "none" {
                None
            } else {
                Some(PicArtistic::preset(name).ok_or("badEffect")?)
            };
        }
        PicStyleCommand::Quick(i) => {
            let (color, width, sh, rf, se, bv, rot, shape) =
                *QUICK_STYLES.get(*i).ok_or("badPreset")?;
            f.line = color.map(|c| PicLine {
                color: c,
                width: emu(width),
                dash: None,
            });
            f.effects = Some(Box::new(PicEffects {
                shadow: sh.and_then(shadow),
                reflection: rf.and_then(reflection),
                soft_edge: se.map(emu),
                bevel: bv.and_then(bevel),
                camera: rot.map(str::to_owned),
                glow: None,
            }));
            f.shape = shape.map(str::to_owned);
            tidy(f);
        }
    }
    Ok(())
}

/// Shape Effects (round 20): Picture Effects `kind` = `preset` applied to a shape's effects.
pub(crate) fn apply_effect(
    kind: &str,
    preset: &str,
    fx: Option<PicEffects>,
) -> std::result::Result<Option<PicEffects>, String> {
    let mut f = PictureFormat {
        effects: fx.map(Box::new),
        ..PictureFormat::default()
    };
    apply(
        &PicStyleCommand::Effect(kind.to_owned(), preset.to_owned()),
        &mut f,
    )?;
    Ok(f.effects.map(|b| *b).filter(|e| !e.is_empty()))
}

/// The artistic effect names, in the gallery's order.
pub fn artistic_names() -> impl Iterator<Item = &'static str> {
    ARTISTIC.iter().map(|a| a.0)
}

impl crate::Engine {
    /// Run a picture style command on the document picture at the selection.
    pub fn doc_picture_style(&mut self, cmd: &PicStyleCommand) -> Result<EditResult> {
        if self.pres.is_some() {
            return Err(EngineError::BadCommand("not a document".into()));
        }
        let read_only = self.sec.read_only;
        let s = self.session.as_mut().ok_or(EngineError::NoDocument)?;
        let error = if read_only {
            Some(crate::security::READ_ONLY.to_owned())
        } else {
            match s.tools_picture() {
                None => Some("noPicture".to_owned()),
                Some(img) if img.shape.is_some() => Some("notPicture".to_owned()),
                Some(img) => {
                    let mut f = img.format.as_deref().cloned().unwrap_or_default();
                    let applied = apply(cmd, &mut f).map(|()| {
                        // §20.4.2.6 effectExtent: the room the effects now take.
                        f.effect_extent = f
                            .effects
                            .as_deref()
                            .map(|e| {
                                bo_model::picture_style::effect_extent(e, img.width.0, img.height.0)
                            })
                            .filter(|e| *e != [0; 4]);
                    });
                    // An artistic effect keeps a copy drawn with it (written as the blip with
                    // the original in a14:imgLayer, as the reference does).
                    let new_art = matches!(cmd, PicStyleCommand::Artistic(_));
                    let rendered = f.artistic.as_ref().filter(|_| new_art).and_then(|a| {
                        let bytes = s.doc().media.get(&img.media)?;
                        let id = format!(
                            "{}{}art={}@{}",
                            img.media,
                            bo_model::picture::FX_MARK,
                            a.name,
                            a.amount()
                        );
                        bo_raster::picfx::derive_id_png(bytes, &id)
                    });
                    let original = img.media.clone();
                    if let Err(e) = applied {
                        Some(e)
                    } else {
                        {
                            let parts: Vec<(String, std::sync::Arc<Vec<u8>>)> = rendered
                                .map(|png| (s.new_media_name("png"), std::sync::Arc::new(png)))
                                .into_iter()
                                .collect();
                            if new_art {
                                let name = parts.first().map(|p| p.0.clone());
                                if let Some(a) = f.artistic.as_mut() {
                                    a.rendered = name;
                                }
                            }
                            s.update_picture_parts(parts, |img: &mut Image| {
                                // The picture stays the original.
                                img.media = original;
                                img.format = (!f.is_plain()).then(|| Box::new(f));
                            })
                            .err()
                            .map(|e| e.to_string())
                        }
                    }
                }
            }
        };
        // Relayout and the picture tools state, as every picture command answers.
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

    fn engine_with_picture() -> Engine {
        let mut e = Engine::with_bundled_fonts();
        e.new_document("p.docx").expect("doc");
        let png = bo_raster::rgba_to_png(bo_raster::codecs::Rgba {
            width: 8,
            height: 8,
            pixels: vec![200; 8 * 8 * 4],
        })
        .expect("png");
        e.insert_image(png, "a.png").expect("insert");
        e.command_json(r#"{"cmd":"move","motion":"left","extend":true}"#)
            .expect("select");
        e
    }

    fn picture(e: &Engine) -> Image {
        e.document()
            .and_then(|d| {
                d.sections[0].blocks.iter().find_map(|b| match b {
                    bo_model::Block::Paragraph(p) => p.runs.iter().find_map(|r| {
                        r.content.iter().find_map(|c| match c {
                            bo_model::RunContent::Image(i) => Some((**i).clone()),
                            _ => None,
                        })
                    }),
                    _ => None,
                })
            })
            .expect("picture")
    }

    #[test]
    fn styles_apply_undo_and_survive_docx() {
        let mut e = engine_with_picture();
        for json in [
            r#"{"cmd":"pictureBorder","color":"0070C0","weight":3,"dash":"dash"}"#,
            r#"{"cmd":"pictureEffect","kind":"shadow","preset":"outerBottomRight"}"#,
            r#"{"cmd":"pictureEffect","kind":"reflection","preset":"half4"}"#,
            r#"{"cmd":"pictureEffect","kind":"glow","preset":"8:ED7D31"}"#,
            r#"{"cmd":"pictureEffect","kind":"softEdge","preset":"5"}"#,
            r#"{"cmd":"pictureEffect","kind":"bevel","preset":"circle"}"#,
            r#"{"cmd":"pictureEffect","kind":"rotation","preset":"perspectiveLeft"}"#,
            r#"{"cmd":"pictureArtistic","name":"artisticBlur"}"#,
        ] {
            let out = e.command_json(json).expect("run");
            assert!(!out.contains("\"error\""), "{json}: {out}");
        }
        let img = picture(&e);
        let f = img.format.as_deref().expect("format");
        assert_eq!(
            f.line.as_ref().map(|l| (l.color, l.width)),
            Some((0x70C0, 38_100))
        );
        let fx = f.effects.as_deref().expect("effects");
        assert_eq!(fx.camera.as_deref(), Some("perspectiveLeft"));
        assert_eq!(fx.glow.as_ref().map(|g| g.color), Some(0xED_7D31));
        assert!(img.display_id().contains("#fx:") || f.artistic.is_some());
        let bytes = e.save_docx().expect("save");
        let mut back = Engine::with_bundled_fonts();
        back.open(&bytes, "p.docx").expect("open");
        let b = picture(&back);
        let bf = b.format.as_deref().expect("format back");
        assert_eq!(bf.line, f.line);
        assert_eq!(bf.effects, f.effects);
        let art = |a: Option<&PicArtistic>| a.map(|a| (a.name.clone(), a.attrs.clone()));
        assert_eq!(art(bf.artistic.as_ref()), art(f.artistic.as_ref()));
        // The reference's way: the blip is the copy drawn with the effect, the picture
        // (a14:imgLayer) the original — read back as the original with its copy.
        assert!(bf
            .artistic
            .as_ref()
            .and_then(|a| a.rendered.as_ref())
            .is_some());
        assert_ne!(
            bf.artistic.as_ref().and_then(|a| a.rendered.clone()),
            Some(b.media.clone())
        );
        assert_eq!(bf.effect_extent, f.effect_extent);
        assert!(f.effect_extent.is_some_and(|e| e[3] > 0));
        // The page shows the effect images (shadow, glow, reflection) and the border.
        let pages = &back.layout().expect("layout").pages;
        let ids: Vec<String> = pages[0]
            .items
            .iter()
            .filter_map(|i| match i {
                bo_layout::PageItem::Image { media, .. } => Some(media.clone()),
                _ => None,
            })
            .collect();
        for t in [
            "shadow=",
            "glow=",
            "refl=",
            "art=artisticBlur",
            "soft=",
            "cam=",
        ] {
            assert!(ids.iter().any(|m| m.contains(t)), "{t}: {ids:?}");
        }
        // The effect images resolve to pixels.
        for id in &ids {
            assert!(back.media_resolved(id).is_some(), "{id}");
        }
        // One undo step per command.
        e.command_json(r#"{"cmd":"undo"}"#).expect("undo");
        assert!(picture(&e)
            .format
            .as_deref()
            .is_some_and(|f| f.artistic.is_none()));
    }

    #[test]
    fn custom_effect_settings() {
        let sh = shadow("custom:color=FF0000;transparency=25;size=110;blur=6;angle=90;distance=5")
            .expect("shadow");
        assert_eq!(sh.color, 0xFF_0000);
        assert_eq!(sh.alpha, 75_000);
        assert_eq!(sh.sx, 110_000);
        assert_eq!(sh.dir, 90 * 60_000);
        assert_eq!(sh.dist, emu(5.0));
        let inner = shadow("custom:inner=1").expect("inner");
        assert!(inner.inner);
        let rf = reflection("custom:transparency=50;size=40;distance=2").expect("refl");
        assert_eq!((rf.st_a, rf.end_pos, rf.dist), (50_000, 40_000, emu(2.0)));
        let gl = glow("custom:size=10;color=00FF00;transparency=30").expect("glow");
        assert_eq!((gl.rad, gl.color, gl.alpha), (emu(10.0), 0xFF00, 70_000));
        let bv = bevel("custom:type=circle;width=8;height=3").expect("bevel");
        assert_eq!((bv.w, bv.h), (emu(8.0), emu(3.0)));
        assert!(bevel("custom:type=nope").is_none());
    }

    #[test]
    fn quick_styles_presets_and_clearing() {
        let mut e = engine_with_picture();
        for i in 0..QUICK_STYLES.len() {
            let out = e
                .command_json(&format!(r#"{{"cmd":"pictureQuickStyle","index":{i}}}"#))
                .expect("quick");
            assert!(!out.contains("\"error\""), "{i}: {out}");
        }
        for i in 0..PRESETS.len() {
            e.command_json(&format!(
                r#"{{"cmd":"pictureEffect","kind":"preset","preset":"{i}"}}"#
            ))
            .expect("preset");
        }
        e.command_json(r#"{"cmd":"pictureEffect","kind":"preset","preset":"none"}"#)
            .expect("none");
        e.command_json(r#"{"cmd":"pictureBorder","color":"none"}"#)
            .expect("none");
        let f = picture(&e).format.map(|f| *f).unwrap_or_default();
        assert!(f.effects.is_none() && f.line.is_none());
        let bad = e
            .command_json(r#"{"cmd":"pictureEffect","kind":"bevel","preset":"zzz"}"#)
            .expect("bad");
        assert!(bad.contains("badPreset"), "{bad}");
    }
}
