//! [MS-DOCX] Word 2010 run extensions (`w14:` children of `w:rPr`): text effects
//! (§2.6.x `CT_Glow`, `CT_Shadow`, `CT_Reflection`, `CT_TextOutlineEffect`,
//! `CT_FillTextEffect`) and OpenType typography (`CT_Ligatures`, `CT_NumForm`,
//! `CT_NumSpacing`, `CT_StylisticSets`, `CT_OnOff` cntxtAlts).
//!
//! Effects read from a file keep their element verbatim (`xml`) and are written back
//! unchanged; effects set by an edit are generated from the model.

use bo_drawingml::color::{resolve, ColorContext};
use bo_model::run_fx::{
    FxColor, FxFill, GradientStop, TextEffects, TextFill, TextGlow, TextOutline, TextReflection,
    TextShadow,
};
use bo_model::slide::{ColorKind, ColorMod, Theme, XmlElement};
use bo_model::Color;
use std::cell::RefCell;
use std::fmt::Write as _;
use std::sync::Arc;

/// Namespace of the Word 2010 extensions ([MS-DOCX] §2.1).
pub(crate) const W14_NS: &str = "http://schemas.microsoft.com/office/word/2010/wordml";

thread_local! {
    /// Theme of the document being read (resolves `w14:schemeClr`).
    static THEME: RefCell<Option<Arc<Theme>>> = const { RefCell::new(None) };
}

/// Set (or clear) the theme used to resolve scheme colours while reading.
pub(crate) fn set_theme(theme: Option<Arc<Theme>>) {
    THEME.with(|t| *t.borrow_mut() = theme);
}

fn local(name: &str) -> &str {
    name.rsplit(':').next().unwrap_or(name)
}

fn attr<'e>(e: &'e XmlElement, name: &str) -> Option<&'e str> {
    e.attrs
        .iter()
        .find(|(k, _)| local(k) == name && !k.starts_with("xmlns"))
        .map(|(_, v)| v.as_str())
}

fn int(e: &XmlElement, name: &str) -> Option<i64> {
    attr(e, name).and_then(|v| v.trim().parse::<i64>().ok())
}

/// [MS-DOCX] `EG_ColorChoice` (`w14:srgbClr`, `w14:schemeClr`) with its transforms
/// (`CT_ColorTransforms`). `w14:alpha` is the transparency of the colour.
fn read_color(el: &XmlElement) -> Option<FxColor> {
    let (kind, scheme) = match local(&el.name) {
        "srgbClr" => (
            ColorKind::Srgb(u32::from_str_radix(attr(el, "val")?.trim(), 16).ok()?),
            None,
        ),
        "schemeClr" => {
            let v = attr(el, "val")?.to_owned();
            (ColorKind::Scheme(v.clone()), Some(v))
        }
        _ => return None,
    };
    let mut alpha = 100_000u32;
    let mut mods = Vec::new();
    for m in el.elements() {
        let val = int(m, "val").map(|v| v as i32);
        if local(&m.name) == "alpha" {
            alpha = 100_000u32.saturating_sub(val.unwrap_or(0).clamp(0, 100_000) as u32);
        } else {
            mods.push(ColorMod {
                name: local(&m.name).to_owned(),
                val,
            });
        }
    }
    let c = bo_model::slide::Color { kind, mods };
    let rgba = THEME.with(|t| {
        let t = t.borrow();
        resolve(
            &c,
            &ColorContext {
                theme: t.as_deref(),
                clr_map: &[],
                ph_clr: None,
            },
        )
    });
    let [r, g, b, _] = rgba.to_u8();
    Some(FxColor {
        rgb: Color { r, g, b },
        scheme,
        alpha,
    })
}

fn first_color(el: &XmlElement) -> Option<FxColor> {
    el.elements().find_map(read_color)
}

/// [MS-DOCX] `EG_FillProperties`: `w14:noFill`, `w14:solidFill`, `w14:gradFill`.
fn read_fill(el: &XmlElement) -> Option<FxFill> {
    for f in el.elements() {
        match local(&f.name) {
            "noFill" => return Some(FxFill::None),
            "solidFill" => {
                return Some(FxFill::Solid {
                    color: first_color(f)?,
                })
            }
            "gradFill" => {
                let mut stops = Vec::new();
                let mut angle = 0;
                let mut path = false;
                for g in f.elements() {
                    match local(&g.name) {
                        "gsLst" => {
                            for gs in g.elements() {
                                if let Some(color) = first_color(gs) {
                                    stops.push(GradientStop {
                                        pos: int(gs, "pos").unwrap_or(0).clamp(0, 100_000) as u32,
                                        color,
                                    });
                                }
                            }
                        }
                        "lin" => angle = int(g, "ang").unwrap_or(0) as i32,
                        "path" => path = true,
                        _ => {}
                    }
                }
                stops.sort_by_key(|s| s.pos);
                if stops.is_empty() {
                    return None;
                }
                return Some(FxFill::Gradient { stops, angle, path });
            }
            _ => {}
        }
    }
    None
}

/// Parse one `w14:` child of `w:rPr` into `fx`; `false` when it is not modelled
/// (the caller keeps it verbatim).
pub(crate) fn read_w14(xml: &str, fx: &mut TextEffects) -> bool {
    let wrapped = format!("<x xmlns:w14=\"{W14_NS}\">{xml}</x>");
    let Ok(root) = bo_drawingml::dom::parse(wrapped.as_bytes(), "/rPr") else {
        return false;
    };
    let Some(el) = root.elements().next() else {
        return false;
    };
    let src = Some(xml.to_owned());
    match local(&el.name) {
        "glow" => {
            let Some(color) = first_color(el) else {
                return false;
            };
            fx.glow = Some(TextGlow {
                radius: int(el, "rad").unwrap_or(0).max(0),
                color,
                xml: src,
            });
        }
        "shadow" => {
            let Some(color) = first_color(el) else {
                return false;
            };
            fx.shadow = Some(TextShadow {
                blur: int(el, "blurRad").unwrap_or(0).max(0),
                dist: int(el, "dist").unwrap_or(0).max(0),
                dir: int(el, "dir").unwrap_or(0) as i32,
                color,
                xml: src,
            });
        }
        "reflection" => {
            fx.reflection = Some(TextReflection {
                blur: int(el, "blurRad").unwrap_or(0).max(0),
                start_alpha: int(el, "stA").unwrap_or(100_000).clamp(0, 100_000) as u32,
                end_alpha: int(el, "endA").unwrap_or(0).clamp(0, 100_000) as u32,
                end_pos: int(el, "endPos").unwrap_or(100_000).clamp(0, 100_000) as u32,
                dist: int(el, "dist").unwrap_or(0).max(0),
                xml: src,
            });
        }
        "textOutline" => {
            fx.outline = Some(TextOutline {
                width: int(el, "w").unwrap_or(0).max(0),
                fill: read_fill(el).unwrap_or(FxFill::None),
                xml: src,
            });
        }
        "textFill" => {
            let Some(fill) = read_fill(el) else {
                return false;
            };
            fx.fill = Some(TextFill { fill, xml: src });
        }
        "ligatures" => fx.ligatures = attr(el, "val").map(str::to_owned),
        "numForm" => fx.num_form = attr(el, "val").map(str::to_owned),
        "numSpacing" => fx.num_spacing = attr(el, "val").map(str::to_owned),
        "stylisticSets" => {
            let mut ids = Vec::new();
            for s in el.elements().filter(|s| local(&s.name) == "styleSet") {
                let on = attr(s, "val").is_none_or(|v| !matches!(v, "0" | "false" | "off"));
                if let Some(id) = int(s, "id").filter(|i| (1..=20).contains(i)) {
                    if on {
                        ids.push(id as u8);
                    }
                }
            }
            fx.stylistic_sets = Some(ids);
        }
        "cntxtAlts" => {
            fx.contextual_alternates =
                Some(attr(el, "val").is_none_or(|v| !matches!(v, "0" | "false" | "off")));
        }
        _ => return false,
    }
    true
}

fn color_xml(c: &FxColor) -> String {
    let mut s = match &c.scheme {
        Some(n) => format!("<w14:schemeClr w14:val=\"{}\"", bo_opc::xml::escape(n)),
        None => format!("<w14:srgbClr w14:val=\"{}\"", c.rgb.to_hex()),
    };
    let tag = if c.scheme.is_some() {
        "schemeClr"
    } else {
        "srgbClr"
    };
    if c.alpha < 100_000 {
        let _ = write!(
            s,
            "><w14:alpha w14:val=\"{}\"/></w14:{tag}>",
            100_000 - c.alpha
        );
    } else {
        s.push_str("/>");
    }
    s
}

fn fill_xml(f: &FxFill) -> String {
    match f {
        FxFill::None => "<w14:noFill/>".into(),
        FxFill::Solid { color } => format!("<w14:solidFill>{}</w14:solidFill>", color_xml(color)),
        FxFill::Gradient { stops, angle, path } => {
            let mut s = String::from("<w14:gradFill><w14:gsLst>");
            for g in stops {
                let _ = write!(
                    s,
                    "<w14:gs w14:pos=\"{}\">{}</w14:gs>",
                    g.pos.min(100_000),
                    color_xml(&g.color)
                );
            }
            s.push_str("</w14:gsLst>");
            if *path {
                s.push_str("<w14:path w14:path=\"circle\"/>");
            } else {
                let _ = write!(
                    s,
                    "<w14:lin w14:ang=\"{}\" w14:scaled=\"0\"/>",
                    angle.rem_euclid(21_600_000)
                );
            }
            s.push_str("</w14:gradFill>");
            s
        }
    }
}

/// Verbatim source of an effect, made self-contained (declares the w14 prefix, so it
/// can move into a package whose root does not).
fn verbatim(xml: &str) -> String {
    if xml.contains("xmlns:w14") {
        return xml.to_owned();
    }
    let t = xml.trim_start();
    let end = t
        .find(|c: char| c.is_whitespace() || c == '>' || c == '/')
        .unwrap_or(t.len());
    format!("{} xmlns:w14=\"{W14_NS}\"{}", &t[..end], &t[end..])
}

fn open(tag: &str) -> String {
    format!("<w14:{tag} xmlns:w14=\"{W14_NS}\"")
}

/// The w14 children of `w:rPr` for `fx`: (qualified name, XML).
pub(crate) fn w14_items(fx: &TextEffects) -> Vec<(String, String)> {
    let mut v = Vec::new();
    let mut push = |tag: &str, xml: String| v.push((format!("w14:{tag}"), xml));
    if let Some(g) = &fx.glow {
        push(
            "glow",
            g.xml.as_deref().map_or_else(
                || {
                    format!(
                        "{} w14:rad=\"{}\">{}</w14:glow>",
                        open("glow"),
                        g.radius.max(0),
                        color_xml(&g.color)
                    )
                },
                verbatim,
            ),
        );
    }
    if let Some(sh) = &fx.shadow {
        push(
            "shadow",
            sh.xml.as_deref().map_or_else(
                || {
                    format!(
                        "{} w14:blurRad=\"{}\" w14:dist=\"{}\" w14:dir=\"{}\" w14:sx=\"100000\" w14:sy=\"100000\" w14:kx=\"0\" w14:ky=\"0\" w14:algn=\"tl\">{}</w14:shadow>",
                        open("shadow"),
                        sh.blur.max(0),
                        sh.dist.max(0),
                        sh.dir.rem_euclid(21_600_000),
                        color_xml(&sh.color)
                    )
                },
                verbatim,
            ),
        );
    }
    if let Some(r) = &fx.reflection {
        push(
            "reflection",
            r.xml.as_deref().map_or_else(
                || {
                    format!(
                        "{} w14:blurRad=\"{}\" w14:stA=\"{}\" w14:stPos=\"0\" w14:endA=\"{}\" w14:endPos=\"{}\" w14:dist=\"{}\" w14:dir=\"5400000\" w14:fadeDir=\"5400000\" w14:sx=\"100000\" w14:sy=\"-100000\" w14:kx=\"0\" w14:ky=\"0\" w14:algn=\"bl\"/>",
                        open("reflection"),
                        r.blur.max(0),
                        r.start_alpha.min(100_000),
                        r.end_alpha.min(100_000),
                        r.end_pos.min(100_000),
                        r.dist.max(0)
                    )
                },
                verbatim,
            ),
        );
    }
    if let Some(o) = &fx.outline {
        push(
            "textOutline",
            o.xml.as_deref().map_or_else(
                || {
                    format!(
                        "{} w14:w=\"{}\" w14:cap=\"flat\" w14:cmpd=\"sng\" w14:algn=\"ctr\">{}<w14:prstDash w14:val=\"solid\"/><w14:round/></w14:textOutline>",
                        open("textOutline"),
                        o.width.max(0),
                        fill_xml(&o.fill)
                    )
                },
                verbatim,
            ),
        );
    }
    if let Some(f) = &fx.fill {
        push(
            "textFill",
            f.xml.as_deref().map_or_else(
                || format!("{}>{}</w14:textFill>", open("textFill"), fill_xml(&f.fill)),
                verbatim,
            ),
        );
    }
    let simple =
        |tag: &str, val: &str| format!("{} w14:val=\"{}\"/>", open(tag), bo_opc::xml::escape(val));
    if let Some(l) = &fx.ligatures {
        push("ligatures", simple("ligatures", l));
    }
    if let Some(n) = &fx.num_form {
        push("numForm", simple("numForm", n));
    }
    if let Some(n) = &fx.num_spacing {
        push("numSpacing", simple("numSpacing", n));
    }
    if let Some(sets) = &fx.stylistic_sets {
        let mut s = format!("{}>", open("stylisticSets"));
        for id in sets.iter().filter(|i| (1..=20).contains(*i)) {
            let _ = write!(s, "<w14:styleSet w14:id=\"{id}\"/>");
        }
        s.push_str("</w14:stylisticSets>");
        push("stylisticSets", s);
    }
    if let Some(c) = fx.contextual_alternates {
        push("cntxtAlts", simple("cntxtAlts", if c { "1" } else { "0" }));
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effects_parse_and_regenerate() {
        let mut fx = TextEffects::default();
        let glow = "<w14:glow w14:rad=\"63500\"><w14:srgbClr w14:val=\"FF0000\"><w14:alpha w14:val=\"40000\"/></w14:srgbClr></w14:glow>";
        assert!(read_w14(glow, &mut fx));
        let g = fx.glow.clone().unwrap();
        assert_eq!(g.radius, 63_500);
        assert_eq!(g.color.rgb, Color { r: 255, g: 0, b: 0 });
        // w14:alpha is the transparency: 40 % transparent = 60 % opaque.
        assert_eq!(g.color.alpha, 60_000);
        let fill = "<w14:textFill><w14:gradFill><w14:gsLst><w14:gs w14:pos=\"100000\"><w14:srgbClr w14:val=\"0000FF\"/></w14:gs><w14:gs w14:pos=\"0\"><w14:srgbClr w14:val=\"FF0000\"/></w14:gs></w14:gsLst><w14:lin w14:ang=\"5400000\" w14:scaled=\"0\"/></w14:gradFill></w14:textFill>";
        assert!(read_w14(fill, &mut fx));
        match &fx.fill.as_ref().unwrap().fill {
            FxFill::Gradient { stops, angle, path } => {
                assert_eq!(stops.len(), 2);
                assert_eq!(stops[0].pos, 0); // sorted by position
                assert_eq!(*angle, 5_400_000);
                assert!(!path);
            }
            f => panic!("{f:?}"),
        }
        assert!(read_w14(
            "<w14:ligatures w14:val=\"standardContextual\"/>",
            &mut fx
        ));
        assert!(read_w14(
            "<w14:stylisticSets><w14:styleSet w14:id=\"3\"/><w14:styleSet w14:id=\"5\" w14:val=\"0\"/></w14:stylisticSets>",
            &mut fx
        ));
        assert_eq!(fx.stylistic_sets, Some(vec![3]));
        assert!(!read_w14("<w14:scene3d/>", &mut fx));
        // Read effects are written back verbatim (plus the namespace declaration).
        let items = w14_items(&fx);
        assert!(items[0].1.starts_with("<w14:glow xmlns:w14="));
        assert!(items[0].1.ends_with("w14:rad=\"63500\"><w14:srgbClr w14:val=\"FF0000\"><w14:alpha w14:val=\"40000\"/></w14:srgbClr></w14:glow>"));
        // Generated effects parse back to the same model.
        let mut gen = fx.clone();
        gen.glow.as_mut().unwrap().xml = None;
        gen.fill.as_mut().unwrap().xml = None;
        let mut back = TextEffects::default();
        for (_, x) in w14_items(&gen) {
            assert!(read_w14(&x, &mut back), "{x}");
        }
        let strip = |mut f: TextEffects| {
            if let Some(g) = f.glow.as_mut() {
                g.xml = None;
            }
            if let Some(g) = f.fill.as_mut() {
                g.xml = None;
            }
            f
        };
        assert_eq!(strip(back), strip(gen));
    }

    #[test]
    fn opentype_features_from_properties() {
        let fx = TextEffects {
            ligatures: Some("none".into()),
            num_form: Some("oldStyle".into()),
            num_spacing: Some("tabular".into()),
            stylistic_sets: Some(vec![2, 12]),
            contextual_alternates: Some(false),
            ..TextEffects::default()
        };
        let f = fx.opentype_features();
        assert!(f.contains(&(*b"liga", 0)) && f.contains(&(*b"clig", 0)));
        assert!(f.contains(&(*b"onum", 1)) && f.contains(&(*b"tnum", 1)));
        assert!(f.contains(&(*b"ss02", 1)) && f.contains(&(*b"ss12", 1)));
        assert!(f.contains(&(*b"calt", 0)));
        let all = TextEffects {
            ligatures: Some("all".into()),
            ..TextEffects::default()
        };
        assert!(all.opentype_features().iter().all(|(_, v)| *v == 1));
    }
}
