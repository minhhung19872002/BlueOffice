//! Shape effects and WordArt (CORE-SHAPE-002): reflection (§20.1.8.50), soft edges
//! (§20.1.8.53), 3-D rotation (§20.1.5.1 scene3d camera), text warp (§20.1.9.19
//! prstTxWarp), text outline and text fill (§21.1.2.3.9 rPr `a:ln`, fills) of the selected
//! shapes — one undo step per command. Effects the command does not touch keep their XML.
//!
//! The preset values written (reflection sizes, the light rig of a 3-D rotation, gradient
//! stop positions) are BlueOffice's own choices.

use super::geom::{place_slide, EMU_PT};
use super::ops::Tx;
use super::{node_clone, refuse, selected, text, EditError, SlideSel, R};
use bo_drawingml::dom::key_for;
use bo_drawingml::effects::EFFECT_LST;
use bo_drawingml::geom::Seg;
use bo_drawingml::textwarp::{warp_names, TextWarp};
use bo_model::slide::{
    Color, Effect, Extra, Fill, GradientFill, GradientShade, GradientStop, Line, ShapeNode,
    TextBody, TextRun, XmlElement,
};
use serde::{Deserialize, Serialize};

/// Text fill of the selected shapes' text.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum TextFillSpec {
    /// Remove any text fill (back to the inherited colour).
    Auto,
    /// No fill (outline-only text).
    None,
    /// `RRGGBB`.
    Solid {
        /// Colour.
        color: String,
    },
    /// Two-colour linear gradient (`RRGGBB`), `angle` in degrees clockwise.
    Gradient {
        /// Start colour.
        from: String,
        /// End colour.
        to: String,
        /// Direction (degrees).
        #[serde(default)]
        angle: f64,
    },
}

/// One effect edit (`shapeEffects` command).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum EffectOp {
    /// Reflection: `size` = how far the fade reaches (fraction of the shape height,
    /// 0..=1), `distance` in points; `size: None` removes it.
    Reflection {
        /// Fade length as a fraction of the reflected height; `None` removes the effect.
        #[serde(default)]
        size: Option<f64>,
        /// Gap between the shape and its reflection (points).
        #[serde(default)]
        distance: f64,
    },
    /// Soft edges of `radius` points; `None` or 0 removes them.
    SoftEdge {
        /// Radius (points).
        #[serde(default)]
        radius: Option<f64>,
    },
    /// 3-D rotation: §20.1.10.47 ST_PresetCameraType name; `None` removes the scene.
    Rotation3d {
        /// Camera preset.
        #[serde(default)]
        preset: Option<String>,
    },
    /// Text warp: §20.1.10.76 ST_TextShapeType name; `None` or `textNoShape` removes it.
    TextWarp {
        /// Warp preset.
        #[serde(default)]
        preset: Option<String>,
    },
    /// Text outline `RRGGBB` (`None` removes it), width in points.
    TextOutline {
        /// Outline colour.
        #[serde(default)]
        color: Option<String>,
        /// Width (points).
        #[serde(default)]
        width: Option<f64>,
    },
    /// Text fill.
    TextFill {
        /// The fill.
        fill: TextFillSpec,
    },
    /// Gallery previews of every text warp (read-only: `warpPreviews` in the result).
    WarpPreviews,
}

/// A text warp gallery thumbnail: blocks standing for three lines of text, warped by
/// the preset, as an SVG path in a 120 × 72 box.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WarpPreview {
    /// §20.1.10.76 preset name.
    pub name: String,
    /// SVG path data.
    pub d: String,
}

/// Width and height of a warp preview box.
pub const PREVIEW_BOX: (f64, f64) = (120.0, 72.0);

/// Gallery previews of all preset text warps (sorted by name).
pub fn warp_previews() -> Vec<WarpPreview> {
    use std::fmt::Write as _;
    let (w, h) = PREVIEW_BOX;
    // Three lines of four "words" in a 100 × 60 block.
    let mut sample = Vec::new();
    for line in 0..3 {
        let y0 = f64::from(line) * 22.0;
        let mut x = 0.0;
        for (k, wd) in [22.0, 30.0, 16.0, 20.0].iter().enumerate() {
            let w = wd - 3.0 + f64::from(line == 1 && k == 2) * 2.0;
            sample.extend([
                Seg::Move(x, y0),
                Seg::Line(x + w, y0),
                Seg::Line(x + w, y0 + 16.0),
                Seg::Line(x, y0 + 16.0),
                Seg::Close,
            ]);
            x += wd + 4.0;
        }
    }
    let src = [0.0, 0.0, 104.0, 60.0];
    warp_names()
        .into_iter()
        .filter_map(|name| {
            let warp = TextWarp::new(name, &[], w - 8.0, h - 8.0)?;
            let mut d = String::new();
            for s in warp.warp_segs(&sample, src) {
                let _ = match s {
                    Seg::Move(x, y) => write!(d, "M{:.1} {:.1}", x + 4.0, y + 4.0),
                    Seg::Line(x, y) => write!(d, "L{:.1} {:.1}", x + 4.0, y + 4.0),
                    Seg::Close => write!(d, "Z"),
                    _ => Ok(()),
                };
            }
            Some(WarpPreview {
                name: name.to_owned(),
                d,
            })
        })
        .collect()
}

fn el(name: &str, attrs: &[(&str, String)]) -> XmlElement {
    let mut e = XmlElement::new(name);
    e.attrs = attrs
        .iter()
        .map(|(k, v)| ((*k).to_owned(), v.clone()))
        .collect();
    e
}

fn push_child(parent: &mut XmlElement, child: XmlElement) {
    parent
        .children
        .push(bo_model::slide::XmlNode::Element(child));
}

/// Replace (or remove, `new = None`) effect `name` in an effect list, keeping the
/// §20.1.8.26 CT_EffectList order.
fn set_effect(list: &mut Vec<Effect>, name: &str, new: Option<XmlElement>) {
    let name_of = |e: &Effect| match e {
        Effect::OuterShadow(_) => "a:outerShdw".to_owned(),
        Effect::Glow(_) => "a:glow".to_owned(),
        Effect::Other(x) => x.name.clone(),
    };
    list.retain(|e| name_of(e) != name);
    if let Some(x) = new {
        let rank = |n: &str| {
            EFFECT_LST
                .iter()
                .position(|s| *s == n)
                .unwrap_or(usize::MAX)
        };
        let me = rank(name);
        let at = list
            .iter()
            .position(|e| rank(&name_of(e)) > me)
            .unwrap_or(list.len());
        list.insert(at, Effect::Other(x));
    }
}

/// Replace (or remove) a `spPr` / `bodyPr` child kept in `extra`, keyed by `schema`.
fn set_extra(extra: &mut Vec<Extra>, schema: &[&str], name: &str, new: Option<XmlElement>) {
    extra.retain(|e| e.xml.name != name);
    if let Some(xml) = new {
        extra.push(Extra {
            key: key_for(schema, name, 0),
            xml,
        });
        extra.sort_by_key(|e| e.key);
    }
}

fn reflection_el(size: f64, distance: f64) -> XmlElement {
    let size = size.clamp(0.05, 1.0);
    // §20.1.8.50: fade from half opacity to almost clear over `size` of the copy, mirrored
    // (sy −100 %) about the bottom edge.
    el(
        "a:reflection",
        &[
            ("blurRad", "6350".into()),
            ("stA", "50000".into()),
            ("endA", "300".into()),
            ("endPos", format!("{}", (size * 100_000.0).round() as i64)),
            (
                "dist",
                format!("{}", (distance.clamp(0.0, 200.0) * EMU_PT).round() as i64),
            ),
            ("dir", "5400000".into()),
            ("sy", "-100000".into()),
            ("algn", "bl".into()),
            ("rotWithShape", "0".into()),
        ],
    )
}

fn hex_color(h: &str) -> R<Color> {
    text::parse_hex(h)
        .map(Color::srgb)
        .ok_or(EditError::Refused("badColor"))
}

/// The ST_PresetCameraType names the 3-D rotation gallery offers (others are accepted too).
pub fn camera_known(name: &str) -> bool {
    // §20.1.10.47: every value starts with one of these families.
    [
        "legacy",
        "orthographic",
        "isometric",
        "oblique",
        "perspective",
    ]
    .iter()
    .any(|p| name.starts_with(p))
        && name.chars().all(|c| c.is_ascii_alphanumeric())
}

/// Every run property of a text body (runs, breaks, fields, end of paragraph).
fn each_rpr(tb: &mut TextBody, f: &mut impl FnMut(&mut bo_model::slide::RunProps)) {
    for p in &mut tb.paragraphs {
        for r in &mut p.runs {
            match r {
                TextRun::Run { props, .. }
                | TextRun::Break { props }
                | TextRun::Field { props, .. } => f(props),
                TextRun::Other(_) => {}
            }
        }
        f(p.end_props.get_or_insert_with(Default::default));
    }
}

/// Apply an effect edit to the selected shapes.
pub fn cmd_effects(tx: &mut Tx<'_>, sel: &SlideSel, op: &EffectOp) -> R<()> {
    let placed = place_slide(tx.pres, sel.slide);
    let paths: Vec<Vec<usize>> = selected(&placed, &sel.ids)
        .iter()
        .map(|p| p.path.clone())
        .collect();
    if paths.is_empty() {
        return refuse("noSelection");
    }
    // Validate once.
    let outline_color = match op {
        EffectOp::TextOutline { color: Some(c), .. } => Some(hex_color(c)?),
        _ => None,
    };
    let text_fill = match op {
        EffectOp::TextFill { fill } => Some(match fill {
            TextFillSpec::Auto => None,
            TextFillSpec::None => Some(Fill::None),
            TextFillSpec::Solid { color } => Some(Fill::Solid(hex_color(color)?)),
            TextFillSpec::Gradient { from, to, angle } => {
                let ang = (angle.rem_euclid(360.0) * 60_000.0).round() as i32;
                Some(Fill::Gradient(GradientFill {
                    stops: vec![
                        GradientStop {
                            pos: 0,
                            color: hex_color(from)?,
                        },
                        GradientStop {
                            pos: 100_000,
                            color: hex_color(to)?,
                        },
                    ],
                    // §20.1.8.41 lin.
                    shade: Some(GradientShade::Linear {
                        ang,
                        scaled: Some(false),
                    }),
                    rot_with_shape: Some(true),
                    ..GradientFill::default()
                }))
            }
        }),
        _ => None,
    };
    match op {
        EffectOp::Rotation3d { preset: Some(p) } if !camera_known(p) => return refuse("badPreset"),
        EffectOp::TextWarp { preset: Some(p) }
            if p != "textNoShape" && bo_drawingml::textwarp::warp_preset(p).is_none() =>
        {
            return refuse("badPreset")
        }
        _ => {}
    }
    let theme =
        bo_pptx::inherit::theme_of(tx.pres, bo_pptx::inherit::Level::Slide(sel.slide)).cloned();
    let mut changed = false;
    for p in paths {
        let mut n = node_clone(tx.pres, sel.slide, &p)?;
        let ShapeNode::Shape(s) = &mut n else {
            continue;
        };
        match op {
            EffectOp::Reflection { .. } | EffectOp::SoftEdge { .. } => {
                // An effect list of its own replaces the style's (§20.1.4.2.8 effectRef):
                // start from the style's effects so they stay.
                let list = s.props.effects.get_or_insert_with(|| {
                    let idx = s.style.as_ref().map_or(0, |st| st.effect_ref.idx) as usize;
                    theme
                        .as_ref()
                        .and_then(|t| t.formats.effect_styles.get(idx.wrapping_sub(1)))
                        .and_then(|es| es.child("a:effectLst"))
                        .map(bo_drawingml::props::parse_effects)
                        .unwrap_or_default()
                });
                match op {
                    EffectOp::Reflection { size, distance } => set_effect(
                        list,
                        "a:reflection",
                        size.filter(|v| *v > 0.0)
                            .map(|v| reflection_el(v, *distance)),
                    ),
                    EffectOp::SoftEdge { radius } => set_effect(
                        list,
                        "a:softEdge",
                        radius.filter(|r| *r > 0.0).map(|r| {
                            el(
                                "a:softEdge",
                                &[("rad", format!("{}", (r.min(400.0) * EMU_PT).round() as i64))],
                            )
                        }),
                    ),
                    _ => {}
                }
            }
            EffectOp::Rotation3d { preset } => {
                let schema = bo_drawingml::props::SP_PR;
                let scene = preset.as_ref().map(|p| {
                    // §20.1.5.1 scene3d: camera + light rig (both required by the schema).
                    let mut sc = XmlElement::new("a:scene3d");
                    push_child(&mut sc, el("a:camera", &[("prst", p.clone())]));
                    push_child(
                        &mut sc,
                        el(
                            "a:lightRig",
                            &[("rig", "threePt".into()), ("dir", "t".into())],
                        ),
                    );
                    sc
                });
                set_extra(&mut s.props.extra, schema, "a:scene3d", scene);
            }
            EffectOp::TextWarp { preset } => {
                let Some(tb) = s.text.as_mut() else { continue };
                let warp = preset.as_ref().filter(|p| *p != "textNoShape").map(|p| {
                    // §20.1.9.19 prstTxWarp with an empty avLst (preset defaults).
                    let mut w = el("a:prstTxWarp", &[("prst", p.clone())]);
                    push_child(&mut w, XmlElement::new("a:avLst"));
                    w
                });
                set_extra(
                    &mut tb.body_pr.extra,
                    bo_drawingml::text::BODY_PR,
                    "a:prstTxWarp",
                    warp,
                );
            }
            EffectOp::TextOutline { width, .. } => {
                let Some(tb) = s.text.as_mut() else { continue };
                each_rpr(tb, &mut |rp| match &outline_color {
                    Some(c) => {
                        let l = rp.line.get_or_insert_with(Line::default);
                        l.fill = Some(Fill::Solid(c.clone()));
                        if let Some(w) = width {
                            l.w = Some((w.clamp(0.0, 20.0) * EMU_PT).round() as i64);
                        } else if l.w.is_none() {
                            l.w = Some(9_525);
                        }
                    }
                    None => rp.line = None,
                });
            }
            EffectOp::TextFill { .. } => {
                let Some(tb) = s.text.as_mut() else { continue };
                let f = text_fill.clone().flatten();
                each_rpr(tb, &mut |rp| rp.fill.clone_from(&f));
            }
            EffectOp::WarpPreviews => {}
        }
        tx.replace_node(sel.slide, &p, n)?;
        changed = true;
    }
    if !changed {
        return refuse("noSelection");
    }
    Ok(())
}
