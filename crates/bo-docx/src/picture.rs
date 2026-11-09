//! Picture formatting of `w:drawing` pictures (CORE-PIC-001): the adjustments of the
//! picture's `a:blip` (ECMA-376 Part 1, §20.1.8.13), its crop (§20.1.8.55 srcRect)
//! and crop shape (§20.1.9.18 prstGeom in `pic:spPr`, §20.2.2.6 pic). Read from the
//! drawing's source XML, and written back by patching that XML in place so
//! everything else in it is kept.

use bo_drawingml::dom::{parse, write_element, El};
use bo_drawingml::picture::{read_fx, write_fx};
use bo_model::picture::PictureFormat;
use bo_model::slide::{XmlElement, XmlNode};

/// Picture properties found in a drawing's XML.
#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct PicProps {
    /// `a:blip@r:embed`.
    pub embed: Option<String>,
    /// Adjustments and crop shape.
    pub format: PictureFormat,
    /// srcRect l, t, r, b.
    pub crop: Option<[i32; 4]>,
}

fn find_mut<'a>(el: &'a mut XmlElement, local: &str) -> Option<&'a mut XmlElement> {
    if el.local() == local {
        return Some(el);
    }
    for c in &mut el.children {
        if let XmlNode::Element(c) = c {
            if let Some(f) = find_mut(c, local) {
                return Some(f);
            }
        }
    }
    None
}

fn find<'a>(el: &'a XmlElement, local: &str) -> Option<&'a XmlElement> {
    if el.local() == local {
        return Some(el);
    }
    el.elements().find_map(|c| find(c, local))
}

fn child_mut<'a>(el: &'a mut XmlElement, local: &str) -> Option<&'a mut XmlElement> {
    el.children.iter_mut().find_map(|c| match c {
        XmlNode::Element(e) if e.local() == local => Some(e),
        _ => None,
    })
}

fn embed_of(blip: &XmlElement) -> Option<String> {
    blip.attrs
        .iter()
        .find(|(k, _)| k.ends_with(":embed"))
        .map(|(_, v)| v.clone())
}

fn props_of(pic: &XmlElement) -> PicProps {
    let mut p = PicProps::default();
    if let Some(fill) = pic.elements().find(|e| e.local() == "blipFill") {
        if let Some(blip) = fill.elements().find(|e| e.local() == "blip") {
            p.embed = embed_of(blip);
            let kids: Vec<XmlElement> = blip.elements().cloned().collect();
            p.format.fx = read_fx(&kids);
        }
        if let Some(r) = fill.elements().find(|e| e.local() == "srcRect") {
            let v = |n: &str| {
                r.attr(n)
                    .and_then(|x| x.trim().parse::<i32>().ok())
                    .unwrap_or(0)
            };
            let c = [v("l"), v("t"), v("r"), v("b")];
            p.crop = c.iter().any(|x| *x != 0).then_some(c);
        }
    }
    if let Some(geom) = pic
        .elements()
        .find(|e| e.local() == "spPr")
        .and_then(|sp| sp.elements().find(|e| e.local() == "prstGeom"))
    {
        p.format.shape = geom
            .attr("prst")
            .filter(|s| *s != "rect")
            .map(str::to_owned);
    }
    p
}

/// Picture properties of a `w:drawing` holding a `pic:pic`.
pub(crate) fn read(xml: &str) -> Option<PicProps> {
    let root = parse(xml.as_bytes(), "drawing").ok()?;
    find(&root, "pic").map(props_of)
}

/// The drawing XML with the picture's blip embed, adjustments, crop and shape
/// replaced (`None` when the XML holds no picture).
pub(crate) fn patch(
    xml: &str,
    embed: Option<&str>,
    format: &PictureFormat,
    crop: Option<[i32; 4]>,
) -> Option<String> {
    let mut root = parse(xml.as_bytes(), "drawing").ok()?;
    let pic = find_mut(&mut root, "pic")?;
    let prefix = pic
        .name
        .split_once(':')
        .map_or("pic", |(p, _)| p)
        .to_owned();
    {
        let fill = child_mut(pic, "blipFill")?;
        let blip = child_mut(fill, "blip")?;
        if let Some(e) = embed {
            if let Some(a) = blip.attrs.iter_mut().find(|(k, _)| k.ends_with(":embed")) {
                a.1 = e.to_owned();
            }
        }
        let mut kids: Vec<XmlElement> = blip.elements().cloned().collect();
        write_fx(&mut kids, &format.fx);
        blip.children = kids.into_iter().map(XmlNode::Element).collect();
        // §20.1.8.55 srcRect follows the blip (CT_BlipFillProperties).
        fill.children
            .retain(|c| !matches!(c, XmlNode::Element(e) if e.local() == "srcRect"));
        if let Some([l, t, r, b]) = crop.filter(|c| c.iter().any(|v| *v != 0)) {
            let at = fill
                .children
                .iter()
                .position(|c| matches!(c, XmlNode::Element(e) if e.local() == "blip"))
                .map_or(0, |i| i + 1);
            let mut s = El::new("a:srcRect");
            for (k, v) in [("l", l), ("t", t), ("r", r), ("b", b)] {
                if v != 0 {
                    s = s.a(k, v);
                }
            }
            fill.children.insert(at, XmlNode::Element(s.done()));
        }
    }
    if let Some(sp) = child_mut(pic, "spPr") {
        let want = format.shape.as_deref().unwrap_or("rect");
        if let Some(g) = child_mut(sp, "prstGeom") {
            if g.attr("prst") != Some(want) {
                if let Some(a) = g.attrs.iter_mut().find(|(k, _)| k == "prst") {
                    a.1 = want.to_owned();
                }
                // Adjust values belong to the old shape.
                for c in &mut g.children {
                    if let XmlNode::Element(av) = c {
                        av.children.clear();
                    }
                }
            }
        }
    }
    let _ = prefix;
    let mut out = String::new();
    write_element(&root, &mut out);
    Some(out)
}

/// Children of `a:blip` for a new picture (written by the generator).
pub(crate) fn blip_children_xml(format: &PictureFormat) -> String {
    let mut kids = Vec::new();
    write_fx(&mut kids, &format.fx);
    let mut out = String::new();
    for k in &kids {
        write_element(k, &mut out);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use bo_model::picture::PictureFx;

    const XML: &str = r#"<w:drawing><wp:inline><wp:extent cx="100" cy="100"/><a:graphic xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/picture"><pic:pic xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture"><pic:nvPicPr><pic:cNvPr id="1" name="P"/><pic:cNvPicPr/></pic:nvPicPr><pic:blipFill><a:blip r:embed="rId5"><a:extLst><a:ext uri="{28A0092B-C50C-407E-A947-70E740481C1C}"><a14:useLocalDpi val="0"/></a:ext></a:extLst></a:blip><a:stretch><a:fillRect/></a:stretch></pic:blipFill><pic:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="100" cy="100"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></pic:spPr></pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing>"#;

    #[test]
    fn patch_then_read_back() {
        let f = PictureFormat {
            fx: PictureFx {
                bright: 20_000,
                grayscale: true,
                ..PictureFx::default()
            },
            shape: Some("ellipse".into()),
        };
        let out = patch(XML, Some("rId9"), &f, Some([10_000, 0, 10_000, 0])).unwrap();
        assert!(
            out.contains(r#"<a:blip r:embed="rId9"><a:grayscl/><a:lum bright="20000"/><a:extLst>"#),
            "{out}"
        );
        assert!(out.contains(r#"</a:blip><a:srcRect l="10000" r="10000"/><a:stretch>"#));
        assert!(out.contains(r#"<a:prstGeom prst="ellipse"><a:avLst/></a:prstGeom>"#));
        // useLocalDpi stays.
        assert!(out.contains("a14:useLocalDpi"));
        let p = read(&out).unwrap();
        assert_eq!(p.embed.as_deref(), Some("rId9"));
        assert_eq!(p.format, f);
        assert_eq!(p.crop, Some([10_000, 0, 10_000, 0]));
        // Reset: back to the source markup.
        let back = patch(&out, Some("rId5"), &PictureFormat::default(), None).unwrap();
        assert_eq!(read(&back).unwrap(), read(XML).unwrap());
    }
}
