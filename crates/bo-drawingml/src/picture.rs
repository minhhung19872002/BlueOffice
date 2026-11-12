//! Picture adjustments in DrawingML markup (CORE-PIC-001): reading and writing the
//! colour transforms of `a:blip` (ECMA-376 Part 1, §20.1.8.13 blip) and the
//! [MS-ODRAWXML] picture effects of the `a14:imgProps` blip extension, shared by the
//! presentation, spreadsheet and word-processing crates.
//!
//! Writing is lossless: only the elements BlueOffice models are replaced; any other
//! blip child (e.g. `a:alphaInv`, artistic effects, `useLocalDpi`) stays as it was.

use crate::color::{is_color_element, parse_color, resolve, ColorContext};
use crate::dom::{attr_int, El};
use bo_model::picture::{BackgroundMark, BackgroundRemoval, ColorChange, PictureFx};
use bo_model::slide::{Color, XmlElement, XmlNode};

/// [MS-ODRAWXML] extension URI of `a14:imgProps` in `a:blip/a:extLst`.
pub const IMG_PROPS_EXT: &str = "{BEBA8EAE-BF5A-486C-A8C5-ECC9F3942E4B}";
/// [MS-ODRAWXML] namespace of the `a14:` elements.
pub const NS_A14: &str = "http://schemas.microsoft.com/office/drawing/2010/main";
/// [MS-ODRAWXML] extension URI of `asvg:svgBlip` (SVG picture next to its PNG fallback).
pub const SVG_BLIP_EXT: &str = "{96DAC541-7B7A-43D3-8B79-37D633B846F1}";

/// Blip children that [`write_fx`] owns (replaced on every write).
const OWNED: [&str; 6] = [
    "lum",
    "grayscl",
    "biLevel",
    "duotone",
    "clrChange",
    "alphaModFix",
];
/// `a14:imgEffect` children that [`write_fx`] owns.
const OWNED_A14: [&str; 3] = ["sharpenSoften", "saturation", "brightnessContrast"];

/// `0xRRGGBB` of a colour without a theme (scheme colours resolve to black).
pub fn plain_rgb(c: &Color) -> Option<u32> {
    Some(resolve(c, &ColorContext::default()).to_hex() & 0xFF_FFFF)
}

fn colors(el: &XmlElement, rgb: &dyn Fn(&Color) -> Option<u32>) -> Vec<u32> {
    el.elements()
        .filter(|e| is_color_element(&e.name))
        .filter_map(parse_color)
        .filter_map(|c| rgb(&c))
        .collect()
}

/// The `a14:imgEffect` elements of the blip's `a14:imgProps` extension.
fn img_effects(children: &[XmlElement]) -> Vec<&XmlElement> {
    let mut out = Vec::new();
    for ext in children
        .iter()
        .filter(|c| c.local() == "extLst")
        .flat_map(bo_model::slide::XmlElement::elements)
        .filter(|e| e.attr("uri") == Some(IMG_PROPS_EXT))
    {
        for layer in ext
            .elements()
            .filter(|e| e.local() == "imgProps")
            .flat_map(bo_model::slide::XmlElement::elements)
            .filter(|e| e.local() == "imgLayer")
        {
            out.extend(
                layer
                    .elements()
                    .filter(|e| e.local() == "imgEffect")
                    .flat_map(bo_model::slide::XmlElement::elements),
            );
        }
    }
    out
}

/// Adjustments of a blip from its children; colours resolved by `rgb`.
pub fn read_fx_with(children: &[XmlElement], rgb: &dyn Fn(&Color) -> Option<u32>) -> PictureFx {
    let mut fx = PictureFx::default();
    for c in children {
        match c.local() {
            // §20.1.8.42 lum
            "lum" => {
                fx.bright = attr_int(c, "bright").unwrap_or(0);
                fx.contrast = attr_int(c, "contrast").unwrap_or(0);
            }
            // §20.1.8.34 grayscl
            "grayscl" => fx.grayscale = true,
            // §20.1.8.11 biLevel
            "biLevel" => fx.bilevel = Some(attr_int(c, "thresh").unwrap_or(50_000)),
            // §20.1.8.23 duotone (exactly two colours)
            "duotone" => {
                if let [a, b] = colors(c, rgb)[..] {
                    fx.duotone = Some([a, b]);
                }
            }
            // §20.1.8.16 clrChange (§20.1.8.17 clrFrom, §20.1.8.18 clrTo)
            "clrChange" => {
                let from = c.elements().find(|e| e.local() == "clrFrom");
                let to = c.elements().find(|e| e.local() == "clrTo");
                if let (Some(f), Some(t)) = (from, to) {
                    let alpha_of = |el: &XmlElement| {
                        el.elements()
                            .filter(|e| is_color_element(&e.name))
                            .flat_map(bo_model::slide::XmlElement::elements)
                            .find(|m| m.local() == "alpha")
                            .and_then(|m| attr_int(m, "val"))
                            .unwrap_or(100_000)
                    };
                    if let (Some(&fc), Some(&tc)) = (colors(f, rgb).first(), colors(t, rgb).first())
                    {
                        fx.color_change = Some(ColorChange {
                            from: fc,
                            to: tc,
                            alpha: alpha_of(t),
                        });
                    }
                }
            }
            // §20.1.8.4 alphaModFix
            "alphaModFix" => fx.alpha = Some(attr_int(c, "amt").unwrap_or(100_000)),
            _ => {}
        }
    }
    for e in img_effects(children) {
        match e.local() {
            "sharpenSoften" => fx.sharpen = attr_int(e, "amount").unwrap_or(0),
            "saturation" => fx.saturation = attr_int(e, "sat"),
            // Brightness/contrast stored as a picture effect (when no a:lum).
            "brightnessContrast" if fx.bright == 0 && fx.contrast == 0 => {
                fx.bright = attr_int(e, "bright").unwrap_or(0);
                fx.contrast = attr_int(e, "contrast").unwrap_or(0);
            }
            _ => {}
        }
    }
    fx.clamped()
}

/// Adjustments of a blip from its children (colours without a theme).
pub fn read_fx(children: &[XmlElement]) -> PictureFx {
    read_fx_with(children, &plain_rgb)
}

fn srgb(c: u32) -> XmlElement {
    El::new("a:srgbClr")
        .a("val", format!("{:06X}", c & 0xFF_FFFF))
        .done()
}

/// The modelled transform elements for `fx` (in BlueOffice's write order).
pub fn fx_elements(fx: &PictureFx) -> Vec<XmlElement> {
    let fx = fx.clamped();
    let mut out = Vec::new();
    if let Some(a) = fx.alpha.filter(|a| *a < 100_000) {
        // §20.1.8.4 alphaModFix
        out.push(El::new("a:alphaModFix").a("amt", a).done());
    }
    if let Some(cc) = fx.color_change {
        // §20.1.8.16 clrChange; clrTo carries the §20.1.2.3.1 alpha of the new colour.
        let mut to = El::new("a:srgbClr").a("val", format!("{:06X}", cc.to));
        if cc.alpha < 100_000 {
            to = to.child(El::new("a:alpha").a("val", cc.alpha).done());
        }
        out.push(
            El::new("a:clrChange")
                .child(El::new("a:clrFrom").child(srgb(cc.from)).done())
                .child(El::new("a:clrTo").child(to.done()).done())
                .done(),
        );
    }
    if let Some(t) = fx.bilevel {
        // §20.1.8.11 biLevel
        out.push(El::new("a:biLevel").a("thresh", t).done());
    }
    if let Some([a, b]) = fx.duotone {
        // §20.1.8.23 duotone
        out.push(El::new("a:duotone").child(srgb(a)).child(srgb(b)).done());
    }
    if fx.grayscale {
        // §20.1.8.34 grayscl
        out.push(El::new("a:grayscl").done());
    }
    if fx.bright != 0 || fx.contrast != 0 {
        // §20.1.8.42 lum
        let mut l = El::new("a:lum");
        if fx.bright != 0 {
            l = l.a("bright", fx.bright);
        }
        if fx.contrast != 0 {
            l = l.a("contrast", fx.contrast);
        }
        out.push(l.done());
    }
    out
}

/// `a14:imgEffect` elements for the [MS-ODRAWXML] effects of `fx`.
fn a14_effects(fx: &PictureFx) -> Vec<XmlElement> {
    let mut out = Vec::new();
    if fx.sharpen != 0 {
        out.push(
            El::new("a14:imgEffect")
                .child(El::new("a14:sharpenSoften").a("amount", fx.sharpen).done())
                .done(),
        );
    }
    if let Some(s) = fx.saturation.filter(|s| *s != 100_000) {
        out.push(
            El::new("a14:imgEffect")
                .child(El::new("a14:saturation").a("sat", s).done())
                .done(),
        );
    }
    out
}

fn is_owned_effect(e: &XmlElement) -> bool {
    e.local() == "imgEffect"
        && e.elements()
            .next()
            .is_some_and(|c| OWNED_A14.contains(&c.local()))
}

/// Replace the modelled adjustments among the blip children `children` by `fx`;
/// every other child is kept in place. The colour transforms go before `a:extLst`
/// (CT_Blip: a choice of transforms, then extLst); sharpen/soften and saturation go
/// into the `a14:imgProps` extension, created or updated as needed.
pub fn write_fx(children: &mut Vec<XmlElement>, fx: &PictureFx) {
    children.retain(|c| !OWNED.contains(&c.local()));
    let at = children
        .iter()
        .position(|c| c.local() == "extLst")
        .unwrap_or(children.len());
    for (i, e) in fx_elements(fx).into_iter().enumerate() {
        children.insert(at + i, e);
    }
    // a14:imgProps: drop the owned effects, then add the new ones to the first layer.
    let new_effects = a14_effects(&fx.clamped());
    let li = children.iter().position(|c| c.local() == "extLst");
    if let Some(li) = li {
        let list = &mut children[li];
        for ext in &mut list.children {
            let XmlNode::Element(ext) = ext else { continue };
            if ext.attr("uri") != Some(IMG_PROPS_EXT) {
                continue;
            }
            for props in &mut ext.children {
                let XmlNode::Element(props) = props else {
                    continue;
                };
                for layer in &mut props.children {
                    let XmlNode::Element(layer) = layer else {
                        continue;
                    };
                    layer.children.retain(|n| match n {
                        XmlNode::Element(e) => !is_owned_effect(e),
                        XmlNode::Text(_) => true,
                    });
                }
                props.children.retain(|n| match n {
                    XmlNode::Element(e) => {
                        e.local() != "imgLayer"
                            || e.elements().next().is_some()
                            || e.attrs.iter().any(|(k, _)| k.ends_with("embed"))
                    }
                    XmlNode::Text(_) => true,
                });
            }
        }
    }
    if !new_effects.is_empty() {
        let li = li.unwrap_or_else(|| {
            children.push(El::new("a:extLst").done());
            children.len() - 1
        });
        let list = &mut children[li];
        let has = list
            .elements()
            .any(|e| e.attr("uri") == Some(IMG_PROPS_EXT));
        if !has {
            // [MS-ODRAWXML] imgProps extension with one picture layer.
            let ext = El::new("a:ext")
                .a("uri", IMG_PROPS_EXT)
                .child(
                    El::new("a14:imgProps")
                        .a("xmlns:a14", NS_A14)
                        .child(El::new("a14:imgLayer").done())
                        .done(),
                )
                .done();
            list.children.insert(0, XmlNode::Element(ext));
        }
        for ext in &mut list.children {
            let XmlNode::Element(ext) = ext else { continue };
            if ext.attr("uri") != Some(IMG_PROPS_EXT) {
                continue;
            }
            let Some(XmlNode::Element(props)) = ext
                .children
                .iter_mut()
                .find(|n| matches!(n, XmlNode::Element(e) if e.local() == "imgProps"))
            else {
                continue;
            };
            let prefix = props.name.split(':').next().unwrap_or("a14").to_owned();
            if !props.elements().any(|e| e.local() == "imgLayer") {
                props
                    .children
                    .push(XmlNode::Element(XmlElement::new(&format!(
                        "{prefix}:imgLayer"
                    ))));
            }
            if let Some(XmlNode::Element(layer)) = props
                .children
                .iter_mut()
                .find(|n| matches!(n, XmlNode::Element(e) if e.local() == "imgLayer"))
            {
                for mut e in new_effects.clone() {
                    rename_prefix(&mut e, &prefix);
                    layer.children.push(XmlNode::Element(e));
                }
            }
            break;
        }
    }
    // Drop an emptied imgProps extension and an emptied extLst.
    if let Some(li) = children.iter().position(|c| c.local() == "extLst") {
        let list = &mut children[li];
        list.children.retain(|n| match n {
            XmlNode::Element(e) if e.attr("uri") == Some(IMG_PROPS_EXT) => e
                .elements()
                .flat_map(bo_model::slide::XmlElement::elements)
                .any(|l| {
                    l.elements().next().is_some()
                        || l.attrs.iter().any(|(k, _)| k.ends_with("embed"))
                }),
            _ => true,
        });
        if list.elements().next().is_none() {
            children.remove(li);
        }
    }
}

fn rename_prefix(e: &mut XmlElement, prefix: &str) {
    if let Some((_, local)) = e.name.split_once(':') {
        e.name = format!("{prefix}:{local}");
    }
    for c in &mut e.children {
        if let XmlNode::Element(c) = c {
            rename_prefix(c, prefix);
        }
    }
}

/// The `a14:imgProps` element of the blip's extension list.
fn img_props_mut(children: &mut [XmlElement]) -> Option<&mut XmlElement> {
    let list = children.iter_mut().find(|c| c.local() == "extLst")?;
    list.children.iter_mut().find_map(|n| match n {
        XmlNode::Element(ext) if ext.attr("uri") == Some(IMG_PROPS_EXT) => {
            ext.children.iter_mut().find_map(|p| match p {
                XmlNode::Element(p) if p.local() == "imgProps" => Some(p),
                _ => None,
            })
        }
        _ => None,
    })
}

fn embed_attr(e: &XmlElement) -> Option<String> {
    e.attrs
        .iter()
        .find(|(k, _)| k.ends_with(":embed"))
        .map(|(_, v)| v.clone())
}

fn pct(e: &XmlElement, name: &str) -> i32 {
    attr_int(e, name).unwrap_or(0).clamp(0, 100_000)
}

/// Background removal of a blip (CORE-PIC-002): the relationship id of the original
/// picture ([MS-ODRAWXML] `a14:imgLayer@r:embed`) and the [MS-ODRAWXML]
/// `a14:backgroundRemoval` effect of that layer, when present.
pub fn read_background_removal(
    children: &[XmlElement],
) -> Option<(Option<String>, BackgroundRemoval)> {
    for ext in children
        .iter()
        .filter(|c| c.local() == "extLst")
        .flat_map(XmlElement::elements)
        .filter(|e| e.attr("uri") == Some(IMG_PROPS_EXT))
    {
        for layer in ext
            .elements()
            .filter(|e| e.local() == "imgProps")
            .flat_map(XmlElement::elements)
            .filter(|e| e.local() == "imgLayer")
        {
            let Some(br) = layer
                .elements()
                .filter(|e| e.local() == "imgEffect")
                .flat_map(XmlElement::elements)
                .find(|e| e.local() == "backgroundRemoval")
            else {
                continue;
            };
            let mut v = BackgroundRemoval {
                rect: [pct(br, "l"), pct(br, "t"), pct(br, "r"), pct(br, "b")],
                marks: Vec::new(),
            };
            for m in br.elements() {
                let keep = match m.local() {
                    "foregroundMark" => true,
                    "backgroundMark" => false,
                    _ => continue,
                };
                v.marks.push(BackgroundMark {
                    keep,
                    line: [pct(m, "x1"), pct(m, "y1"), pct(m, "x2"), pct(m, "y2")],
                });
            }
            return Some((embed_attr(layer), v));
        }
    }
    None
}

fn is_removal_effect(n: &XmlNode) -> bool {
    matches!(n, XmlNode::Element(e) if e.local() == "imgEffect"
        && e.elements().next().is_some_and(|c| c.local() == "backgroundRemoval"))
}

/// Set (or, with `None`, remove) the background removal of a blip: the picture layer
/// referencing the original picture `rid` with its `a14:backgroundRemoval` effect
/// ([MS-ODRAWXML] CT_PictureLayer / CT_PictureEffectBackgroundRemoval; the foreground
/// marks precede the background marks). Every other child is kept.
pub fn write_background_removal(
    children: &mut Vec<XmlElement>,
    value: Option<(&str, &BackgroundRemoval)>,
) {
    // Drop the previous removal and the layer's original-picture reference.
    if let Some(props) = img_props_mut(children) {
        for layer in &mut props.children {
            let XmlNode::Element(layer) = layer else {
                continue;
            };
            if layer.local() != "imgLayer" || !layer.children.iter().any(is_removal_effect) {
                continue;
            }
            layer.children.retain(|n| !is_removal_effect(n));
            layer.attrs.retain(|(k, _)| !k.ends_with(":embed"));
        }
        props.children.retain(|n| match n {
            XmlNode::Element(e) => {
                e.local() != "imgLayer"
                    || e.elements().next().is_some()
                    || e.attrs.iter().any(|(k, _)| k.ends_with("embed"))
            }
            XmlNode::Text(_) => true,
        });
    }
    if let Some((rid, v)) = value {
        let li = children
            .iter()
            .position(|c| c.local() == "extLst")
            .unwrap_or_else(|| {
                children.push(El::new("a:extLst").done());
                children.len() - 1
            });
        if img_props_mut(children).is_none() {
            let ext = El::new("a:ext")
                .a("uri", IMG_PROPS_EXT)
                .child(El::new("a14:imgProps").a("xmlns:a14", NS_A14).done())
                .done();
            children[li].children.insert(0, XmlNode::Element(ext));
        }
        if let Some(props) = img_props_mut(children) {
            let prefix = props.name.split(':').next().unwrap_or("a14").to_owned();
            let at = props
                .children
                .iter()
                .position(|n| matches!(n, XmlNode::Element(e) if e.local() == "imgLayer"));
            let at = at.unwrap_or_else(|| {
                props
                    .children
                    .push(XmlNode::Element(XmlElement::new(&format!(
                        "{prefix}:imgLayer"
                    ))));
                props.children.len() - 1
            });
            if let XmlNode::Element(layer) = &mut props.children[at] {
                layer.attrs.retain(|(k, _)| !k.ends_with(":embed"));
                layer.attrs.push(("r:embed".into(), rid.to_owned()));
                let c = |x: i32| x.clamp(0, 100_000);
                let [l, t, r, b] = v.rect;
                let mut br = El::new(&format!("{prefix}:backgroundRemoval"))
                    .a("t", c(t))
                    .a("b", c(b))
                    .a("l", c(l))
                    .a("r", c(r));
                for keep in [true, false] {
                    let name = if keep {
                        "foregroundMark"
                    } else {
                        "backgroundMark"
                    };
                    for m in v.marks.iter().filter(|m| m.keep == keep) {
                        let [x1, y1, x2, y2] = m.line;
                        br = br.child(
                            El::new(&format!("{prefix}:{name}"))
                                .a("x1", c(x1))
                                .a("y1", c(y1))
                                .a("x2", c(x2))
                                .a("y2", c(y2))
                                .done(),
                        );
                    }
                }
                let effect = El::new(&format!("{prefix}:imgEffect"))
                    .child(br.done())
                    .done();
                layer.children.insert(0, XmlNode::Element(effect));
            }
        }
    }
    // Drop an emptied imgProps extension and an emptied extLst.
    if let Some(li) = children.iter().position(|c| c.local() == "extLst") {
        let list = &mut children[li];
        list.children.retain(|n| match n {
            XmlNode::Element(e) if e.attr("uri") == Some(IMG_PROPS_EXT) => {
                e.elements().flat_map(XmlElement::elements).next().is_some()
            }
            _ => true,
        });
        if list.elements().next().is_none() {
            children.remove(li);
        }
    }
}

/// Relationship id of the SVG picture of a blip ([MS-ODRAWXML] `asvg:svgBlip@r:embed`
/// in the blip's extension list), if any.
pub fn svg_blip_embed(children: &[XmlElement]) -> Option<String> {
    children
        .iter()
        .filter(|c| c.local() == "extLst")
        .flat_map(bo_model::slide::XmlElement::elements)
        .filter(|e| e.attr("uri") == Some(SVG_BLIP_EXT))
        .flat_map(bo_model::slide::XmlElement::elements)
        .find(|e| e.local() == "svgBlip")
        .and_then(|e| {
            e.attrs
                .iter()
                .find(|(k, _)| k.ends_with(":embed"))
                .map(|(_, v)| v.clone())
        })
}

/// [MS-ODRAWXML] namespace of `asvg:svgBlip`.
pub const NS_ASVG: &str = "http://schemas.microsoft.com/office/drawing/2016/SVG/main";

/// Sets (`Some(rid)`) or removes the SVG version of a blip ([MS-ODRAWXML] `asvg:svgBlip`
/// in an `a:ext` of the blip's `a:extLst`, §20.1.2.2.15 extLst); other extensions stay.
pub fn write_svg_blip(children: &mut Vec<XmlElement>, rid: Option<&str>) {
    for list in children.iter_mut().filter(|c| c.local() == "extLst") {
        list.children
            .retain(|n| !matches!(n, XmlNode::Element(e) if e.attr("uri") == Some(SVG_BLIP_EXT)));
    }
    children.retain(|c| c.local() != "extLst" || c.elements().next().is_some());
    let Some(rid) = rid else {
        return;
    };
    let li = children
        .iter()
        .position(|c| c.local() == "extLst")
        .unwrap_or_else(|| {
            children.push(El::new("a:extLst").done());
            children.len() - 1
        });
    let ext = El::new("a:ext")
        .a("uri", SVG_BLIP_EXT)
        .child(
            El::new("asvg:svgBlip")
                .a("xmlns:asvg", NS_ASVG)
                .a("r:embed", rid)
                .done(),
        )
        .done();
    children[li].children.push(XmlNode::Element(ext));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dom::{parse, write_element};

    fn blip(xml: &str) -> XmlElement {
        parse(xml.as_bytes(), "t").unwrap()
    }

    fn children(el: &XmlElement) -> Vec<XmlElement> {
        el.elements().cloned().collect()
    }

    #[test]
    fn reads_transforms() {
        let b = blip(
            r#"<a:blip xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="r"><a:alphaModFix amt="60000"/><a:duotone><a:prstClr val="black"/><a:srgbClr val="D9C3A5"/></a:duotone><a:lum bright="20000" contrast="-40000"/><a:extLst><a:ext uri="{BEBA8EAE-BF5A-486C-A8C5-ECC9F3942E4B}"><a14:imgProps xmlns:a14="http://schemas.microsoft.com/office/drawing/2010/main"><a14:imgLayer r:embed="rId9"><a14:imgEffect><a14:sharpenSoften amount="25000"/></a14:imgEffect><a14:imgEffect><a14:saturation sat="0"/></a14:imgEffect></a14:imgLayer></a14:imgProps></a:ext></a:extLst></a:blip>"#,
        );
        let fx = read_fx(&children(&b));
        assert_eq!(fx.bright, 20_000);
        assert_eq!(fx.contrast, -40_000);
        assert_eq!(fx.alpha, Some(60_000));
        assert_eq!(fx.duotone, Some([0x00_0000, 0xD9_C3A5]));
        assert_eq!(fx.sharpen, 25_000);
        assert_eq!(fx.saturation, Some(0));
    }

    #[test]
    fn writes_and_keeps_unknown_children() {
        let b = blip(
            r#"<a:blip xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><a:alphaInv/><a:lum bright="10000"/><a:extLst><a:ext uri="{28A0092B-C50C-407E-A947-70E740481C1C}"><a14:useLocalDpi xmlns:a14="http://schemas.microsoft.com/office/drawing/2010/main" val="0"/></a:ext></a:extLst></a:blip>"#,
        );
        let mut c = children(&b);
        let fx = PictureFx {
            grayscale: true,
            contrast: 40_000,
            sharpen: -50_000,
            ..PictureFx::default()
        };
        write_fx(&mut c, &fx);
        let mut s = String::new();
        for e in &c {
            write_element(e, &mut s);
        }
        assert_eq!(
            s,
            "<a:alphaInv/><a:grayscl/><a:lum contrast=\"40000\"/><a:extLst><a:ext uri=\"{BEBA8EAE-BF5A-486C-A8C5-ECC9F3942E4B}\"><a14:imgProps xmlns:a14=\"http://schemas.microsoft.com/office/drawing/2010/main\"><a14:imgLayer><a14:imgEffect><a14:sharpenSoften amount=\"-50000\"/></a14:imgEffect></a14:imgLayer></a14:imgProps></a:ext><a:ext uri=\"{28A0092B-C50C-407E-A947-70E740481C1C}\"><a14:useLocalDpi xmlns:a14=\"http://schemas.microsoft.com/office/drawing/2010/main\" val=\"0\"/></a:ext></a:extLst>"
        );
        assert_eq!(read_fx(&c), fx);
        // Reset: back to the unknown children only.
        write_fx(&mut c, &PictureFx::default());
        let mut s = String::new();
        for e in &c {
            write_element(e, &mut s);
        }
        assert_eq!(
            s,
            "<a:alphaInv/><a:extLst><a:ext uri=\"{28A0092B-C50C-407E-A947-70E740481C1C}\"><a14:useLocalDpi xmlns:a14=\"http://schemas.microsoft.com/office/drawing/2010/main\" val=\"0\"/></a:ext></a:extLst>"
        );
    }

    #[test]
    fn background_removal_round_trip() {
        let b = blip(
            r#"<a:blip xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><a:lum bright="10000"/><a:extLst><a:ext uri="{28A0092B-C50C-407E-A947-70E740481C1C}"><a14:useLocalDpi xmlns:a14="http://schemas.microsoft.com/office/drawing/2010/main" val="0"/></a:ext></a:extLst></a:blip>"#,
        );
        let mut c = children(&b);
        assert_eq!(read_background_removal(&c), None);
        let v = BackgroundRemoval {
            rect: [5000, 6000, 95_000, 94_000],
            marks: vec![
                BackgroundMark {
                    keep: false,
                    line: [1000, 2000, 3000, 4000],
                },
                BackgroundMark {
                    keep: true,
                    line: [50_000, 50_000, 60_000, 50_000],
                },
            ],
        };
        write_background_removal(&mut c, Some(("rId7", &v)));
        // Sharpen goes into the same layer, before or after: both are kept.
        write_fx(
            &mut c,
            &PictureFx {
                bright: 10_000,
                sharpen: 20_000,
                ..PictureFx::default()
            },
        );
        let mut s = String::new();
        for e in &c {
            write_element(e, &mut s);
        }
        assert!(
            s.contains(r#"<a14:imgLayer r:embed="rId7"><a14:imgEffect><a14:backgroundRemoval t="6000" b="94000" l="5000" r="95000"><a14:foregroundMark x1="50000" y1="50000" x2="60000" y2="50000"/><a14:backgroundMark x1="1000" y1="2000" x2="3000" y2="4000"/></a14:backgroundRemoval></a14:imgEffect><a14:imgEffect><a14:sharpenSoften amount="20000"/></a14:imgEffect></a14:imgLayer>"#),
            "{s}"
        );
        let (rid, back) = read_background_removal(&c).unwrap();
        assert_eq!(rid.as_deref(), Some("rId7"));
        // Foreground marks first (schema order).
        assert_eq!(back.rect, v.rect);
        assert_eq!(back.marks, vec![v.marks[1], v.marks[0]]);
        assert_eq!(read_fx(&c).sharpen, 20_000);
        // Removing: the layer keeps the sharpen effect, loses the original reference.
        write_background_removal(&mut c, None);
        assert_eq!(read_background_removal(&c), None);
        assert_eq!(read_fx(&c).sharpen, 20_000);
        write_fx(&mut c, &PictureFx::default());
        let mut s = String::new();
        for e in &c {
            write_element(e, &mut s);
        }
        assert_eq!(
            s,
            "<a:extLst><a:ext uri=\"{28A0092B-C50C-407E-A947-70E740481C1C}\"><a14:useLocalDpi xmlns:a14=\"http://schemas.microsoft.com/office/drawing/2010/main\" val=\"0\"/></a:ext></a:extLst>"
        );
    }

    #[test]
    fn colour_change_round_trip() {
        let fx = PictureFx {
            color_change: Some(ColorChange {
                from: 0xFF_FFFF,
                to: 0xFF_FFFF,
                alpha: 0,
            }),
            bilevel: Some(50_000),
            alpha: Some(40_000),
            ..PictureFx::default()
        };
        let mut c = Vec::new();
        write_fx(&mut c, &fx);
        assert_eq!(read_fx(&c), fx);
    }

    #[test]
    fn svg_blip_set_and_removed() {
        let mut kids = vec![];
        write_svg_blip(&mut kids, Some("rId7"));
        assert_eq!(svg_blip_embed(&kids).as_deref(), Some("rId7"));
        // Next to the a14 picture effects in the same extension list.
        write_fx(
            &mut kids,
            &PictureFx {
                sharpen: 20_000,
                ..PictureFx::default()
            },
        );
        write_svg_blip(&mut kids, Some("rId8"));
        assert_eq!(kids.iter().filter(|k| k.local() == "extLst").count(), 1);
        assert_eq!(svg_blip_embed(&kids).as_deref(), Some("rId8"));
        let mut xml = String::new();
        for k in &kids {
            write_element(k, &mut xml);
        }
        assert_eq!(xml.matches("svgBlip").count(), 1, "{xml}");
        assert!(xml.contains("imgProps"), "{xml}");
        write_svg_blip(&mut kids, None);
        assert!(svg_blip_embed(&kids).is_none());
        assert!(kids.iter().any(|k| k.local() == "extLst"), "a14 kept");
    }
}
