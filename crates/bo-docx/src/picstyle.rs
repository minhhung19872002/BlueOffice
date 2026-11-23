//! Picture styles of `pic:pic` (BlueDoc round 19): Picture Border (ECMA-376 Part 1,
//! §20.1.2.2.24 `ln`), Picture Effects (§20.1.8.26 `effectLst`, §20.1.5.9 `scene3d`,
//! §20.1.5.12 `sp3d`) in `pic:spPr`, and the artistic effect ([MS-ODRAWXML] 2.3.1) of the
//! blip's `a14:imgProps` extension. Read from the drawing XML; written by replacing only
//! the elements whose modelled value changed, so anything BlueOffice does not model in
//! them stays as it was.

use bo_drawingml::color::{find_color, resolve, ColorContext};
use bo_drawingml::dom::{attr_int, El};
use bo_drawingml::picture::{IMG_PROPS_EXT, NS_A14};
use bo_model::picture::PictureFormat;
use bo_model::picture_style::{
    PicArtistic, PicBevel, PicEffects, PicGlow, PicLine, PicReflection, PicShadow, ARTISTIC,
};
use bo_model::slide::{XmlElement, XmlNode};

/// Colour `0xRRGGBB` and opacity (1/1000 %) of the first colour child of `el`.
fn color_of(el: &XmlElement) -> Option<(u32, i32)> {
    let c = find_color(el)?;
    let rgba = resolve(&c, &ColorContext::default());
    let a = i32::from(rgba.to_u8()[3]);
    Some((rgba.to_hex() & 0xFF_FFFF, (a * 100_000 + 127) / 255))
}

fn kid<'a>(el: &'a XmlElement, local: &str) -> Option<&'a XmlElement> {
    el.elements().find(|e| e.local() == local)
}

/// §20.1.2.2.24 ln of `spPr` as a Picture Border (a solid line only).
fn read_line(sp: &XmlElement) -> Option<PicLine> {
    let ln = kid(sp, "ln")?;
    if kid(ln, "noFill").is_some() {
        return None;
    }
    let (color, _) = kid(ln, "solidFill").and_then(color_of)?;
    Some(PicLine {
        color,
        width: attr_int(ln, "w").unwrap_or(9525),
        dash: kid(ln, "prstDash")
            .and_then(|d| d.attr("val"))
            .filter(|v| *v != "solid")
            .map(str::to_owned),
    })
}

fn read_shadow(e: &XmlElement, inner: bool) -> PicShadow {
    let (color, alpha) = color_of(e).unwrap_or((0, 100_000));
    PicShadow {
        inner,
        blur: attr_int(e, "blurRad").unwrap_or(0),
        dist: attr_int(e, "dist").unwrap_or(0),
        dir: attr_int(e, "dir").unwrap_or(0),
        color,
        alpha,
        sx: if inner {
            0
        } else {
            attr_int(e, "sx").unwrap_or(100_000)
        },
        sy: if inner {
            0
        } else {
            attr_int(e, "sy").unwrap_or(100_000)
        },
    }
}

/// The effects of `spPr`.
fn read_effects(sp: &XmlElement) -> Option<PicEffects> {
    let mut e = PicEffects::default();
    if let Some(list) = kid(sp, "effectLst") {
        for c in list.elements() {
            match c.local() {
                "outerShdw" => e.shadow = Some(read_shadow(c, false)),
                "innerShdw" => e.shadow = Some(read_shadow(c, true)),
                "glow" => {
                    let (color, alpha) = color_of(c).unwrap_or((0, 100_000));
                    e.glow = Some(PicGlow {
                        rad: attr_int(c, "rad").unwrap_or(0),
                        color,
                        alpha,
                    });
                }
                "reflection" => {
                    e.reflection = Some(PicReflection {
                        blur: attr_int(c, "blurRad").unwrap_or(0),
                        st_a: attr_int(c, "stA").unwrap_or(100_000),
                        end_a: attr_int(c, "endA").unwrap_or(0),
                        end_pos: attr_int(c, "endPos").unwrap_or(100_000),
                        dist: attr_int(c, "dist").unwrap_or(0),
                    });
                }
                "softEdge" => e.soft_edge = attr_int(c, "rad"),
                _ => {}
            }
        }
    }
    e.camera = kid(sp, "scene3d")
        .and_then(|s| kid(s, "camera"))
        .and_then(|c| c.attr("prst"))
        .filter(|p| *p != "orthographicFront")
        .map(str::to_owned);
    e.bevel = kid(sp, "sp3d")
        .and_then(|s| kid(s, "bevelT"))
        .map(|b| PicBevel {
            prst: b.attr("prst").unwrap_or("circle").to_owned(),
            w: attr_int(b, "w").unwrap_or(76_200),
            h: attr_int(b, "h").unwrap_or(76_200),
        });
    (!e.is_empty()).then_some(e)
}

/// The artistic effect among the blip's `a14:imgEffect`s.
fn read_artistic(blip: &XmlElement) -> Option<PicArtistic> {
    blip.elements()
        .filter(|c| c.local() == "extLst")
        .flat_map(XmlElement::elements)
        .filter(|e| e.attr("uri") == Some(IMG_PROPS_EXT))
        .flat_map(XmlElement::elements)
        .flat_map(XmlElement::elements)
        .filter(|l| l.local() == "imgLayer")
        .flat_map(XmlElement::elements)
        .filter(|e| e.local() == "imgEffect")
        .flat_map(XmlElement::elements)
        .find(|e| ARTISTIC.iter().any(|a| a.0 == e.local()))
        .map(|e| PicArtistic {
            name: e.local().to_owned(),
            attrs: e.attrs.clone(),
            rendered: None,
        })
}

/// Read the picture style of `pic` into `format`.
pub(crate) fn read(pic: &XmlElement, format: &mut PictureFormat) {
    if let Some(sp) = kid(pic, "spPr") {
        format.line = read_line(sp);
        format.effects = read_effects(sp).map(Box::new);
    }
    format.artistic = kid(pic, "blipFill")
        .and_then(|f| kid(f, "blip"))
        .and_then(read_artistic);
}

fn srgb(color: u32, alpha: i32) -> XmlElement {
    let mut c = El::new("a:srgbClr").a("val", format!("{:06X}", color & 0xFF_FFFF));
    if alpha < 100_000 {
        c = c.child(El::new("a:alpha").a("val", alpha.max(0)).done());
    }
    c.done()
}

/// §20.1.2.2.24 ln of a Picture Border.
fn line_el(l: &PicLine) -> XmlElement {
    let mut ln = El::new("a:ln")
        .a("w", l.width.max(0))
        .child(El::new("a:solidFill").child(srgb(l.color, 100_000)).done());
    if let Some(d) = &l.dash {
        ln = ln.child(El::new("a:prstDash").a("val", d).done());
    }
    ln.done()
}

/// §20.1.8.26 effectLst (children in the schema's order: blur, fillOverlay, glow,
/// innerShdw, outerShdw, prstShdw, reflection, softEdge).
fn effect_list(e: &PicEffects) -> Option<XmlElement> {
    let mut list = El::new("a:effectLst");
    let mut any = false;
    if let Some(g) = &e.glow {
        list = list.child(
            El::new("a:glow")
                .a("rad", g.rad)
                .child(srgb(g.color, g.alpha))
                .done(),
        );
        any = true;
    }
    if let Some(s) = &e.shadow {
        let el = if s.inner {
            El::new("a:innerShdw")
                .a("blurRad", s.blur)
                .a("dist", s.dist)
                .a("dir", s.dir)
        } else {
            let mut o = El::new("a:outerShdw")
                .a("blurRad", s.blur)
                .a("dist", s.dist)
                .a("dir", s.dir);
            if s.sx != 0 && s.sx != 100_000 {
                o = o.a("sx", s.sx);
            }
            if s.sy != 0 && s.sy != 100_000 {
                o = o.a("sy", s.sy);
            }
            o.a("algn", "tl").a("rotWithShape", "0")
        };
        list = list.child(el.child(srgb(s.color, s.alpha)).done());
        any = true;
    }
    if let Some(r) = &e.reflection {
        list = list.child(
            El::new("a:reflection")
                .a("blurRad", r.blur)
                .a("stA", r.st_a)
                .a("endA", r.end_a)
                .a("endPos", r.end_pos)
                .a("dist", r.dist)
                .a("dir", 5_400_000)
                .a("sy", -100_000)
                .a("algn", "bl")
                .a("rotWithShape", "0")
                .done(),
        );
        any = true;
    }
    if let Some(rad) = e.soft_edge.filter(|r| *r > 0) {
        list = list.child(El::new("a:softEdge").a("rad", rad).done());
        any = true;
    }
    any.then(|| list.done())
}

/// §20.1.5.9 scene3d (camera + a light rig, required by the schema).
fn scene_el(camera: &str) -> XmlElement {
    El::new("a:scene3d")
        .child(El::new("a:camera").a("prst", camera).done())
        .child(
            El::new("a:lightRig")
                .a("rig", "threePt")
                .a("dir", "t")
                .done(),
        )
        .done()
}

/// §20.1.5.12 sp3d with a top bevel.
fn sp3d_el(b: &PicBevel) -> XmlElement {
    El::new("a:sp3d")
        .child(
            El::new("a:bevelT")
                .a("w", b.w)
                .a("h", b.h)
                .a("prst", &b.prst)
                .done(),
        )
        .done()
}

/// The effects of a shape's `spPr` (BlueDoc round 20: Shape Effects use the Picture Effects
/// model).
pub(crate) fn read_shape_effects(sp: &XmlElement) -> Option<PicEffects> {
    read_effects(sp)
}

/// `effectLst`, `scene3d` and `sp3d` of Shape Effects `fx` (written after `ln`).
pub(crate) fn effects_xml(fx: &PicEffects) -> String {
    sp_pr_xml(&PictureFormat {
        effects: Some(Box::new(fx.clone())),
        ..PictureFormat::default()
    })
}

/// `spPr` children of a new picture after its geometry.
pub(crate) fn sp_pr_xml(format: &PictureFormat) -> String {
    let mut out = String::new();
    let mut put = |e: &XmlElement| bo_drawingml::dom::write_element(e, &mut out);
    if let Some(l) = &format.line {
        put(&line_el(l));
    }
    let fx = format.effects.as_deref();
    if let Some(list) = fx.and_then(effect_list) {
        put(&list);
    }
    // A bevel needs a scene: the default front camera when there is no 3-D rotation.
    let camera = fx.and_then(|e| e.camera.as_deref());
    if camera.is_some() || fx.is_some_and(|e| e.bevel.is_some()) {
        put(&scene_el(camera.unwrap_or("orthographicFront")));
    }
    if let Some(b) = fx.and_then(|e| e.bevel.as_ref()) {
        put(&sp3d_el(b));
    }
    out
}

/// Order of `spPr` children (CT_ShapeProperties).
const SP_ORDER: [&str; 13] = [
    "xfrm",
    "custGeom",
    "prstGeom",
    "noFill",
    "solidFill",
    "gradFill",
    "blipFill",
    "pattFill",
    "grpFill",
    "ln",
    "effectLst",
    "scene3d",
    "sp3d",
];

fn rank(local: &str) -> usize {
    // effectDag stands where effectLst does.
    let local = if local == "effectDag" {
        "effectLst"
    } else {
        local
    };
    SP_ORDER
        .iter()
        .position(|n| *n == local)
        .unwrap_or(SP_ORDER.len())
}

/// Put `el` (or nothing) in place of `sp`'s child named `local`, in schema order.
fn replace(sp: &mut XmlElement, local: &str, el: Option<XmlElement>) {
    let at = sp
        .children
        .iter()
        .position(|c| matches!(c, XmlNode::Element(e) if e.local() == local));
    if let Some(i) = at {
        sp.children.remove(i);
    }
    let Some(el) = el else { return };
    let r = rank(local);
    let pos = at.unwrap_or_else(|| {
        sp.children
            .iter()
            .position(|c| matches!(c, XmlNode::Element(e) if rank(e.local()) > r))
            .unwrap_or(sp.children.len())
    });
    sp.children.insert(pos, XmlNode::Element(el));
}

fn child_mut<'a>(el: &'a mut XmlElement, local: &str) -> Option<&'a mut XmlElement> {
    el.children.iter_mut().find_map(|c| match c {
        XmlNode::Element(e) if e.local() == local => Some(e),
        _ => None,
    })
}

/// Write the picture style of `format` into `pic` where it differs from what is there.
pub(crate) fn patch(pic: &mut XmlElement, format: &PictureFormat, layer_embed: Option<&str>) {
    let mut was = PictureFormat::default();
    read(pic, &mut was);
    if let Some(sp) = child_mut(pic, "spPr") {
        if was.line != format.line {
            replace(sp, "ln", format.line.as_ref().map(line_el));
        }
        let (old, new) = (was.effects.as_deref(), format.effects.as_deref());
        let list = |e: Option<&PicEffects>| {
            e.map(|e| {
                (
                    e.shadow.clone(),
                    e.glow.clone(),
                    e.reflection.clone(),
                    e.soft_edge,
                )
            })
        };
        if list(old) != list(new) {
            replace(sp, "effectLst", new.and_then(effect_list));
        }
        let cam = |e: Option<&PicEffects>| e.and_then(|e| e.camera.clone());
        let bev = |e: Option<&PicEffects>| e.and_then(|e| e.bevel.clone());
        if cam(old) != cam(new) || (bev(old).is_none() != bev(new).is_none()) {
            let need = cam(new).is_some() || bev(new).is_some();
            replace(
                sp,
                "scene3d",
                need.then(|| scene_el(cam(new).as_deref().unwrap_or("orthographicFront"))),
            );
        }
        if bev(old) != bev(new) {
            replace(sp, "sp3d", bev(new).as_ref().map(sp3d_el));
        }
    }
    // The rendered copy is not in the markup's effect: compare without it.
    let bare = |a: Option<&PicArtistic>| {
        a.map(|a| PicArtistic {
            rendered: None,
            ..a.clone()
        })
    };
    if bare(was.artistic.as_ref()) != bare(format.artistic.as_ref()) || layer_embed.is_some() {
        if let Some(blip) = child_mut(pic, "blipFill").and_then(|f| child_mut(f, "blip")) {
            write_artistic(blip, format.artistic.as_ref(), layer_embed);
        }
    }
}

/// Add the artistic effect to the children of a new picture's blip.
pub(crate) fn add_artistic(
    kids: &mut Vec<XmlElement>,
    a: Option<&PicArtistic>,
    layer_embed: Option<&str>,
) {
    if a.is_none() {
        return;
    }
    let mut blip = XmlElement::new("a:blip");
    blip.children = kids.drain(..).map(XmlNode::Element).collect();
    write_artistic(&mut blip, a, layer_embed);
    kids.extend(blip.children.into_iter().filter_map(|n| match n {
        XmlNode::Element(e) => Some(e),
        XmlNode::Text(_) => None,
    }));
}

/// `a14:imgEffect` with the artistic effect, as XML text for a new picture's blip.
#[cfg(test)]
pub(crate) fn artistic_xml(a: Option<&PicArtistic>) -> String {
    let Some(a) = a else { return String::new() };
    let mut blip = XmlElement::new("a:blip");
    write_artistic(&mut blip, Some(a), None);
    let mut out = String::new();
    for c in blip.elements() {
        bo_drawingml::dom::write_element(c, &mut out);
    }
    out
}

/// Replace the artistic effect of a blip (one per picture, the first effect of its layer).
/// `layer_embed`: relationship id of the original picture when the blip shows the rendered
/// copy ([MS-ODRAWXML] `a14:imgLayer@r:embed`).
fn write_artistic(blip: &mut XmlElement, a: Option<&PicArtistic>, layer_embed: Option<&str>) {
    let is_art = |n: &XmlNode| {
        matches!(n, XmlNode::Element(e) if e.local() == "imgEffect"
            && e.elements().next().is_some_and(|c| ARTISTIC.iter().any(|a| a.0 == c.local())))
    };
    // Drop the old one.
    for ext in blip
        .children
        .iter_mut()
        .filter_map(|n| match n {
            XmlNode::Element(e) if e.local() == "extLst" => Some(e),
            _ => None,
        })
        .flat_map(|l| l.children.iter_mut())
    {
        let XmlNode::Element(ext) = ext else { continue };
        if ext.attr("uri") != Some(IMG_PROPS_EXT) {
            continue;
        }
        for props in &mut ext.children {
            if let XmlNode::Element(props) = props {
                for layer in &mut props.children {
                    if let XmlNode::Element(layer) = layer {
                        layer.children.retain(|n| !is_art(n));
                        // Without an effect the layer's original is no longer the picture's.
                        if a.is_none() && layer.elements().next().is_none() {
                            layer.attrs.retain(|(k, _)| !k.ends_with("embed"));
                        }
                    }
                }
            }
        }
    }
    let Some(a) = a else { return };
    // Find or make extLst › ext › a14:imgProps › a14:imgLayer.
    if !blip.elements().any(|e| e.local() == "extLst") {
        blip.children
            .push(XmlNode::Element(El::new("a:extLst").done()));
    }
    let Some(list) = child_mut(blip, "extLst") else {
        return;
    };
    if !list
        .elements()
        .any(|e| e.attr("uri") == Some(IMG_PROPS_EXT))
    {
        list.children.insert(
            0,
            XmlNode::Element(
                El::new("a:ext")
                    .a("uri", IMG_PROPS_EXT)
                    .child(
                        El::new("a14:imgProps")
                            .a("xmlns:a14", NS_A14)
                            .child(El::new("a14:imgLayer").done())
                            .done(),
                    )
                    .done(),
            ),
        );
    }
    let Some(ext) = list.children.iter_mut().find_map(|n| match n {
        XmlNode::Element(e) if e.attr("uri") == Some(IMG_PROPS_EXT) => Some(e),
        _ => None,
    }) else {
        return;
    };
    let Some(props) = child_mut(ext, "imgProps") else {
        return;
    };
    let prefix = props.name.split(':').next().unwrap_or("a14").to_owned();
    if child_mut(props, "imgLayer").is_none() {
        props
            .children
            .push(XmlNode::Element(XmlElement::new(&format!(
                "{prefix}:imgLayer"
            ))));
    }
    if let Some(layer) = child_mut(props, "imgLayer") {
        let effect = El::new(&format!("{prefix}:imgEffect"))
            .child(
                El::new(&format!("{prefix}:{}", a.name))
                    .rest(&a.attrs)
                    .done(),
            )
            .done();
        // [MS-ODRAWXML]: the artistic effect comes first in the layer.
        layer.children.insert(0, XmlNode::Element(effect));
        if let Some(rid) = layer_embed {
            layer.attrs.retain(|(k, _)| !k.ends_with("embed"));
            layer.attrs.push(("r:embed".into(), rid.to_owned()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bo_drawingml::dom::{parse, write_element};

    const PIC: &str = r#"<pic:pic xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><pic:blipFill><a:blip r:embed="rId5"/><a:stretch><a:fillRect/></a:stretch></pic:blipFill><pic:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="100" cy="100"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></pic:spPr></pic:pic>"#;

    fn styled() -> PictureFormat {
        PictureFormat {
            line: Some(PicLine {
                color: 0x00_70C0,
                width: 38_100,
                dash: Some("dash".into()),
            }),
            effects: Some(Box::new(PicEffects {
                shadow: Some(PicShadow {
                    blur: 50_800,
                    dist: 38_100,
                    dir: 2_700_000,
                    color: 0,
                    alpha: 40_000,
                    sx: 100_000,
                    sy: 100_000,
                    inner: false,
                }),
                glow: Some(PicGlow {
                    rad: 63_500,
                    color: 0xFF_C000,
                    alpha: 60_000,
                }),
                reflection: Some(PicReflection {
                    blur: 6_350,
                    st_a: 52_000,
                    end_a: 300,
                    end_pos: 35_000,
                    dist: 0,
                }),
                soft_edge: Some(63_500),
                bevel: Some(PicBevel {
                    prst: "circle".into(),
                    w: 76_200,
                    h: 76_200,
                }),
                camera: Some("perspectiveRelaxed".into()),
            })),
            artistic: PicArtistic::preset("artisticPencilGrayscale"),
            ..PictureFormat::default()
        }
    }

    #[test]
    fn style_round_trips_and_keeps_order() {
        let mut pic = parse(PIC.as_bytes(), "pic").expect("xml");
        let f = styled();
        patch(&mut pic, &f, None);
        let mut back = PictureFormat::default();
        read(&pic, &mut back);
        assert_eq!(back.line, f.line);
        assert_eq!(back.effects, f.effects);
        assert_eq!(back.artistic, f.artistic);
        let mut xml = String::new();
        write_element(&pic, &mut xml);
        let order: Vec<usize> = ["prstGeom", "a:ln", "effectLst", "scene3d", "sp3d"]
            .iter()
            .map(|n| xml.find(n).expect(n))
            .collect();
        assert!(order.windows(2).all(|w| w[0] < w[1]), "{xml}");
        assert!(
            xml.contains("a14:artisticPencilGrayscale pencilSize=\"50\""),
            "{xml}"
        );
        // Clearing removes everything again.
        patch(&mut pic, &PictureFormat::default(), None);
        let mut xml = String::new();
        write_element(&pic, &mut xml);
        for n in ["a:ln", "effectLst", "scene3d", "sp3d", "imgEffect"] {
            assert!(!xml.contains(n), "{n}: {xml}");
        }
    }

    #[test]
    fn new_picture_markup_reads_back() {
        let f = styled();
        let xml = format!(
            r#"<pic:pic xmlns:pic="p" xmlns:a="a"><pic:blipFill><a:blip>{}</a:blip></pic:blipFill><pic:spPr><a:prstGeom prst="rect"/>{}</pic:spPr></pic:pic>"#,
            artistic_xml(f.artistic.as_ref()),
            sp_pr_xml(&f)
        );
        let pic = parse(xml.as_bytes(), "pic").expect("xml");
        let mut back = PictureFormat::default();
        read(&pic, &mut back);
        assert_eq!(
            (back.line, back.effects, back.artistic),
            (f.line, f.effects, f.artistic)
        );
    }
}
