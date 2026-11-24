//! Shape Format fills and effects of `wps:wsp` (BlueDoc round 20, `bo_model::shape_style`):
//! read from and written to `wps:spPr` — §20.1.8.33 gradFill (§20.1.8.37 gs, §20.1.8.41 lin,
//! §20.1.8.46 path with §20.1.8.31 fillToRect), §20.1.8.14 blipFill (§20.1.8.56 stretch /
//! §20.1.8.58 tile, §20.1.8.4 alphaModFix), §20.1.2.3.1 alpha of solid fills and lines,
//! §20.1.8.26 effectLst / §20.1.5.9 scene3d / §20.1.5.12 sp3d (as Picture Effects).

use bo_drawingml::color::{resolve, ColorContext};
use bo_drawingml::dom::attr_int;
use bo_model::shape_style::{ShapeGradient, ShapePicture, ShapeStop, ShapeStyle};
use bo_model::slide::{Fill, GradientShade, XmlElement};

/// The exact :alpha (1/1000 %) of the colour in fill element ill (a child of parent).
fn xml_alpha(parent: Option<&XmlElement>, fill: &str) -> Option<i32> {
    let f = parent?.elements().find(|e| e.local() == fill)?;
    let c = f.elements().next()?;
    let a = c.elements().find(|e| e.local() == "alpha")?;
    attr_int::<i32>(a, "val")
}

/// Opacity (1/1000 %) of an 8-bit alpha.
fn alpha_of(a: u8) -> i32 {
    (i32::from(a) * 100_000 + 127) / 255
}

/// The style of a shape: `sp` is its `spPr` element, `fill` / `line_fill` the parsed fills.
/// A picture fill's `media` is its relationship id (`r:embed`) for the caller to resolve.
pub(crate) fn read(
    sp: Option<&XmlElement>,
    fill: Option<&Fill>,
    line_fill: Option<&Fill>,
    ctx: &ColorContext<'_>,
) -> Option<ShapeStyle> {
    let mut s = ShapeStyle::default();
    match fill {
        Some(Fill::Solid(c)) => {
            let a = resolve(c, ctx).to_u8()[3];
            if a < 255 {
                s.fill_alpha = Some(xml_alpha(sp, "solidFill").unwrap_or_else(|| alpha_of(a)));
            }
        }
        Some(Fill::Gradient(g)) if g.stops.len() >= 2 => {
            let (kind, angle, focus) = match &g.shade {
                Some(GradientShade::Path { path, fill_to }) => {
                    (path.clone(), 0, fill_to.unwrap_or([50_000; 4]))
                }
                Some(GradientShade::Linear { ang, .. }) => ("linear".to_owned(), *ang, [0; 4]),
                None => ("linear".to_owned(), 5_400_000, [0; 4]),
            };
            let mut stops: Vec<ShapeStop> = g
                .stops
                .iter()
                .map(|st| {
                    let rgba = resolve(&st.color, ctx);
                    ShapeStop {
                        pos: st.pos,
                        color: rgba.to_hex() & 0xFF_FFFF,
                        alpha: alpha_of(rgba.to_u8()[3]),
                    }
                })
                .collect();
            stops.sort_by_key(|st| st.pos);
            s.gradient = Some(ShapeGradient {
                kind,
                angle,
                focus,
                stops,
            });
        }
        Some(Fill::Picture(b)) => {
            let rid = b.embed.clone()?;
            let alpha = b
                .blip_children
                .iter()
                .find(|c| c.local() == "alphaModFix")
                .and_then(|c| attr_int::<i32>(c, "amt"))
                .unwrap_or(100_000);
            let scale = b
                .tile
                .as_ref()
                .and_then(|t| attr_int::<i32>(t, "sx"))
                .unwrap_or(100_000);
            s.picture = Some(ShapePicture {
                media: rid,
                tile: b.tile.is_some(),
                scale,
                alpha,
            });
        }
        _ => {}
    }
    if let Some(Fill::Solid(c)) = line_fill {
        let a = resolve(c, ctx).to_u8()[3];
        if a < 255 {
            let ln = sp.and_then(|sp| sp.elements().find(|e| e.local() == "ln"));
            s.line_alpha = Some(xml_alpha(ln, "solidFill").unwrap_or_else(|| alpha_of(a)));
        }
    }
    // §20.1.8.33 / §20.1.8.14 rotWithShape="0": the fill stays upright (round 22).
    if s.gradient.is_some() || s.picture.is_some() {
        let off = sp
            .and_then(|sp| {
                sp.elements()
                    .find(|e| matches!(e.local(), "gradFill" | "blipFill"))
            })
            .and_then(|e| e.attr("rotWithShape"))
            .is_some_and(|v| v == "0" || v == "false");
        if off {
            s.rot_with_shape = Some(false);
        }
    }
    s.effects = sp.and_then(crate::picstyle::read_shape_effects);
    (!s.is_empty()).then_some(s)
}

/// `a:srgbClr` with an alpha below 100 %.
fn srgb(color: u32, alpha: i32) -> String {
    if alpha < 100_000 {
        format!(
            "<a:srgbClr val=\"{:06X}\"><a:alpha val=\"{}\"/></a:srgbClr>",
            color & 0xFF_FFFF,
            alpha.max(0)
        )
    } else {
        format!("<a:srgbClr val=\"{:06X}\"/>", color & 0xFF_FFFF)
    }
}

/// `rotWithShape` of a gradient / picture fill (round 22).
fn rot_attr(s: &ShapeStyle) -> &'static str {
    if s.rot_with_shape == Some(false) {
        "0"
    } else {
        "1"
    }
}

/// The fill of a styled shape (`None`: the plain solid / no fill applies). `rid` is the
/// relationship id of a picture fill's media.
pub(crate) fn fill_xml(
    s: &ShapeStyle,
    solid: Option<bo_model::Color>,
    rid: Option<&str>,
) -> Option<String> {
    if let Some(g) = &s.gradient {
        let mut x = format!("<a:gradFill rotWithShape=\"{}\"><a:gsLst>", rot_attr(s));
        for st in &g.stops {
            x.push_str(&format!(
                "<a:gs pos=\"{}\">{}</a:gs>",
                st.pos.clamp(0, 100_000),
                srgb(st.color, st.alpha)
            ));
        }
        x.push_str("</a:gsLst>");
        if g.kind == "linear" || g.kind.is_empty() {
            x.push_str(&format!(
                "<a:lin ang=\"{}\" scaled=\"1\"/>",
                g.angle.rem_euclid(21_600_000)
            ));
        } else {
            let [l, t, r, b] = g.focus;
            x.push_str(&format!(
                "<a:path path=\"{}\"><a:fillToRect l=\"{l}\" t=\"{t}\" r=\"{r}\" b=\"{b}\"/></a:path>",
                match g.kind.as_str() {
                    "rect" => "rect",
                    "shape" => "shape",
                    _ => "circle",
                }
            ));
        }
        x.push_str("</a:gradFill>");
        return Some(x);
    }
    if let (Some(p), Some(rid)) = (&s.picture, rid) {
        let amt = if p.alpha > 0 && p.alpha < 100_000 {
            format!("<a:alphaModFix amt=\"{}\"/>", p.alpha)
        } else {
            String::new()
        };
        let mode = if p.tile {
            let k = if p.scale > 0 { p.scale } else { 100_000 };
            format!("<a:tile tx=\"0\" ty=\"0\" sx=\"{k}\" sy=\"{k}\" flip=\"none\" algn=\"tl\"/>")
        } else {
            "<a:stretch><a:fillRect/></a:stretch>".to_owned()
        };
        let rot = rot_attr(s);
        return Some(format!(
            "<a:blipFill dpi=\"0\" rotWithShape=\"{rot}\"><a:blip r:embed=\"{rid}\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\">{amt}</a:blip><a:srcRect/>{mode}</a:blipFill>"
        ));
    }
    match (solid, s.fill_alpha) {
        (Some(c), Some(a)) if a < 100_000 => Some(format!(
            "<a:solidFill>{}</a:solidFill>",
            srgb(
                (u32::from(c.r) << 16) | (u32::from(c.g) << 8) | u32::from(c.b),
                a
            )
        )),
        _ => None,
    }
}

/// The line colour with its transparency.
pub(crate) fn line_color_xml(s: Option<&ShapeStyle>, c: bo_model::Color) -> String {
    let hex = (u32::from(c.r) << 16) | (u32::from(c.g) << 8) | u32::from(c.b);
    srgb(hex, s.and_then(|s| s.line_alpha).unwrap_or(100_000))
}

/// `effectLst` / `scene3d` / `sp3d` of the shape's effects.
pub(crate) fn effects_xml(s: Option<&ShapeStyle>) -> String {
    s.and_then(|s| s.effects.as_ref())
        .filter(|e| !e.is_empty())
        .map_or_else(String::new, crate::picstyle::effects_xml)
}
