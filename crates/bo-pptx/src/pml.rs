//! PresentationML shape tree and slide-like part bodies — ECMA-376 Part 1, §19.3.1.

use bo_drawingml::color::{find_color, write_color};
use bo_drawingml::dom::{attr_int, walk, AttrBag, El, Kids};
use bo_drawingml::props::{
    is_fill, parse_blip_fill, parse_fill, parse_sp_pr, parse_style, parse_xfrm, write_blip_fill,
    write_fill, write_sp_pr, write_style, write_xfrm,
};
use bo_drawingml::table::{parse_table, write_table};
use bo_drawingml::text::{parse_text_body, write_text_body};
use bo_model::slide::{
    Background, ColorMapOverride, CommonSlideData, Connector, Extra, GraphicContent, GraphicFrame,
    GroupShape, NonVisual, Picture, Placeholder, Shape, ShapeNode, ShapeProps, XmlElement,
};

/// `graphicData@uri` of DrawingML tables (§21.1.3).
pub const URI_TABLE: &str = "http://schemas.openxmlformats.org/drawingml/2006/table";

const SP: &[&str] = &["p:nvSpPr", "p:spPr", "p:style", "p:txBody", "p:extLst"];
const PIC: &[&str] = &["p:nvPicPr", "p:blipFill", "p:spPr", "p:style", "p:extLst"];
const CXN: &[&str] = &["p:nvCxnSpPr", "p:spPr", "p:style", "p:extLst"];
const FRAME: &[&str] = &["p:nvGraphicFramePr", "p:xfrm", "a:graphic", "p:extLst"];
const CNV_PR: &[&str] = &["a:hlinkClick", "a:hlinkHover", "a:extLst"];
const NV_PR: &[&str] = &[
    "p:ph",
    "a:audioCd",
    "a:wavAudioFile",
    "a:audioFile",
    "a:videoFile",
    "a:quickTimeFile",
    "p:custDataLst",
    "p:extLst",
];
const CSLD: &[&str] = &[
    "p:bg",
    "p:spTree",
    "p:custDataLst",
    "p:controls",
    "p:extLst",
];
const BG_PR: &[&str] = &[
    "a:noFill",
    "a:solidFill",
    "a:gradFill",
    "a:blipFill",
    "a:pattFill",
    "a:grpFill",
    "a:effectLst",
    "a:effectDag",
    "a:extLst",
];

/// §19.3.1.34 nvSpPr (and the other `nv*Pr` containers).
fn parse_nv(el: &XmlElement) -> NonVisual {
    let mut nv = NonVisual::default();
    for c in el.elements() {
        match c.name.as_str() {
            // §19.3.1.12 cNvPr
            "p:cNvPr" => {
                let mut bag = AttrBag::of(c);
                nv.id = bag.int("id").unwrap_or(0);
                nv.name = bag.take("name").unwrap_or_default();
                nv.descr = bag.take("descr");
                nv.hidden = bag.bool("hidden");
                nv.cnv_attrs = bag.rest();
                nv.cnv_extra = walk(c, CNV_PR, |_| false);
            }
            // §19.3.1.33 nvPr
            "p:nvPr" => {
                nv.nv_attrs = c.attrs.clone();
                nv.nv_extra = walk(c, NV_PR, |x| {
                    if x.name != "p:ph" {
                        return false;
                    }
                    let mut bag = AttrBag::of(x);
                    // §19.3.1.36 ph
                    let ph = Placeholder {
                        kind: bag.take("type"),
                        orient: bag.take("orient"),
                        sz: bag.take("sz"),
                        idx: bag.int("idx"),
                        has_custom_prompt: bag.bool("hasCustomPrompt"),
                        extra: walk(x, &["p:extLst"], |_| false),
                    };
                    if !bag.rest().is_empty() {
                        return false;
                    }
                    nv.placeholder = Some(ph);
                    true
                });
            }
            _ => nv.drawing_props = Some(c.clone()),
        }
    }
    nv
}

fn write_nv(nv: &NonVisual, name: &str, default_cnv: &str) -> XmlElement {
    let mut k = Kids::new(CNV_PR);
    k.extras(&nv.cnv_extra);
    let cnv = k.build(
        El::new("p:cNvPr")
            .a("id", nv.id)
            .a("name", &nv.name)
            .opt("descr", nv.descr.as_ref())
            .optb("hidden", nv.hidden)
            .rest(&nv.cnv_attrs),
    );
    let drawing = nv
        .drawing_props
        .clone()
        .unwrap_or_else(|| XmlElement::new(default_cnv));
    let mut k = Kids::new(NV_PR);
    if let Some(ph) = &nv.placeholder {
        let mut pk = Kids::new(&["p:extLst"]);
        pk.extras(&ph.extra);
        k.push(
            pk.build(
                El::new("p:ph")
                    .opt("type", ph.kind.as_ref())
                    .opt("orient", ph.orient.as_ref())
                    .opt("sz", ph.sz.as_ref())
                    .opt("idx", ph.idx)
                    .optb("hasCustomPrompt", ph.has_custom_prompt),
            ),
        );
    }
    k.extras(&nv.nv_extra);
    let nvpr = k.build(El::new("p:nvPr").rest(&nv.nv_attrs));
    El::new(name).child(cnv).child(drawing).child(nvpr).done()
}

/// Parse one shape-tree element.
pub fn parse_node(el: &XmlElement) -> ShapeNode {
    match el.name.as_str() {
        // §19.3.1.43 sp
        "p:sp" => {
            let mut s = Shape {
                attrs: el.attrs.clone(),
                ..Shape::default()
            };
            s.extra = walk(el, SP, |c| {
                match c.name.as_str() {
                    "p:nvSpPr" => s.nv = parse_nv(c),
                    "p:spPr" => s.props = parse_sp_pr(c),
                    "p:style" => s.style = Some(parse_style(c)),
                    "p:txBody" => s.text = Some(parse_text_body(c)),
                    _ => return false,
                }
                true
            });
            ShapeNode::Shape(Box::new(s))
        }
        // §19.3.1.37 pic
        "p:pic" if el.attrs.is_empty() => {
            let mut p = Picture::default();
            p.extra = walk(el, PIC, |c| {
                match c.name.as_str() {
                    "p:nvPicPr" => p.nv = parse_nv(c),
                    "p:blipFill" => p.blip = parse_blip_fill(c),
                    "p:spPr" => p.props = parse_sp_pr(c),
                    "p:style" => p.style = Some(parse_style(c)),
                    _ => return false,
                }
                true
            });
            ShapeNode::Picture(Box::new(p))
        }
        // §19.3.1.22 grpSp
        "p:grpSp" if el.attrs.is_empty() => {
            let mut g = GroupShape::default();
            g.extra = walk_tree(el, &mut g.nv, &mut g.props, &mut g.children);
            ShapeNode::Group(Box::new(g))
        }
        // §19.3.1.19 cxnSp
        "p:cxnSp" if el.attrs.is_empty() => {
            let mut c = Connector::default();
            c.extra = walk(el, CXN, |x| {
                match x.name.as_str() {
                    "p:nvCxnSpPr" => c.nv = parse_nv(x),
                    "p:spPr" => c.props = parse_sp_pr(x),
                    "p:style" => c.style = Some(parse_style(x)),
                    _ => return false,
                }
                true
            });
            ShapeNode::Connector(Box::new(c))
        }
        // §19.3.1.21 graphicFrame
        "p:graphicFrame" if el.attrs.is_empty() => {
            let mut f = GraphicFrame {
                nv: NonVisual::default(),
                xfrm: bo_model::slide::Xfrm::default(),
                uri: String::new(),
                content: GraphicContent::Other(XmlElement::new("a:graphicData")),
                extra: Vec::new(),
                alt: None,
            };
            let mut ok = true;
            f.extra = walk(el, FRAME, |c| {
                match c.name.as_str() {
                    "p:nvGraphicFramePr" => f.nv = parse_nv(c),
                    "p:xfrm" => f.xfrm = parse_xfrm(c),
                    // §20.1.2.2.16 graphic / §20.1.2.2.17 graphicData
                    "a:graphic" => {
                        let Some(gd) = c.child("a:graphicData") else {
                            ok = false;
                            return false;
                        };
                        if c.elements().count() != 1 || !c.attrs.is_empty() {
                            ok = false;
                        }
                        f.uri = gd.attr("uri").unwrap_or_default().to_owned();
                        let tbl = gd.child("a:tbl");
                        f.content = match tbl {
                            Some(t) if f.uri == URI_TABLE && gd.elements().count() == 1 => {
                                GraphicContent::Table(parse_table(t))
                            }
                            _ => GraphicContent::Other(gd.clone()),
                        };
                    }
                    _ => return false,
                }
                true
            });
            if ok {
                ShapeNode::Frame(Box::new(f))
            } else {
                ShapeNode::Other(el.clone())
            }
        }
        // A 3D model ([MS-ODRAWXML] am3d) in mc:AlternateContent: the frame of its choice,
        // the wrapper kept for writing (ECMA-376 Part 3, §10.2.1).
        _ if crate::model3d::alternate_model_frame(el).is_some() => {
            match crate::model3d::alternate_model_frame(el).map(parse_node) {
                Some(ShapeNode::Frame(mut f)) => {
                    f.alt = Some(Box::new(el.clone()));
                    ShapeNode::Frame(f)
                }
                _ => ShapeNode::Other(el.clone()),
            }
        }
        // A zoom ([MS-PPTX] pslz / psez / psuz) in mc:AlternateContent: the same.
        _ if crate::zoom::alternate_zoom_frame(el).is_some() => {
            match crate::zoom::alternate_zoom_frame(el).map(parse_node) {
                Some(ShapeNode::Frame(mut f)) => {
                    f.alt = Some(Box::new(el.clone()));
                    ShapeNode::Frame(f)
                }
                _ => ShapeNode::Other(el.clone()),
            }
        }
        _ => ShapeNode::Other(el.clone()),
    }
}

/// Walk a group-like container (spTree / grpSp): nv + props + ordered shapes.
fn walk_tree(
    el: &XmlElement,
    nv: &mut NonVisual,
    props: &mut ShapeProps,
    children: &mut Vec<ShapeNode>,
) -> Vec<Extra> {
    let mut extra = Vec::new();
    let mut seen_shape = false;
    for c in el.elements() {
        match c.name.as_str() {
            "p:nvGrpSpPr" => *nv = parse_nv(c),
            "p:grpSpPr" => *props = parse_sp_pr(c),
            "p:extLst" if seen_shape || children.is_empty() => extra.push(Extra {
                key: 6,
                xml: c.clone(),
            }),
            _ => {
                seen_shape = true;
                children.push(parse_node(c));
            }
        }
    }
    extra
}

fn write_tree(
    name: &str,
    nv: &NonVisual,
    props: &ShapeProps,
    children: &[ShapeNode],
    extra: &[Extra],
) -> XmlElement {
    let mut e = El::new(name)
        .child(write_nv(nv, "p:nvGrpSpPr", "p:cNvGrpSpPr"))
        .child(write_sp_pr(props, "p:grpSpPr"));
    for c in children {
        e = e.child(write_node(c));
    }
    for x in extra {
        e = e.child(x.xml.clone());
    }
    e.done()
}

/// Serialize one shape-tree element.
pub fn write_node(n: &ShapeNode) -> XmlElement {
    match n {
        ShapeNode::Shape(s) => {
            let mut k = Kids::new(SP);
            k.push(write_nv(&s.nv, "p:nvSpPr", "p:cNvSpPr"));
            k.push(write_sp_pr(&s.props, "p:spPr"));
            k.push_opt(s.style.as_ref().map(|x| write_style(x, "p:style")));
            k.push_opt(s.text.as_ref().map(|t| write_text_body(t, "p:txBody")));
            k.extras(&s.extra);
            k.build(El::new("p:sp").rest(&s.attrs))
        }
        ShapeNode::Picture(p) => {
            let mut k = Kids::new(PIC);
            k.push(write_nv(&p.nv, "p:nvPicPr", "p:cNvPicPr"));
            k.push(write_blip_fill(&p.blip, "p:blipFill"));
            k.push(write_sp_pr(&p.props, "p:spPr"));
            k.push_opt(p.style.as_ref().map(|x| write_style(x, "p:style")));
            k.extras(&p.extra);
            k.build(El::new("p:pic"))
        }
        ShapeNode::Group(g) => write_tree("p:grpSp", &g.nv, &g.props, &g.children, &g.extra),
        ShapeNode::Connector(c) => {
            let mut k = Kids::new(CXN);
            k.push(write_nv(&c.nv, "p:nvCxnSpPr", "p:cNvCxnSpPr"));
            k.push(write_sp_pr(&c.props, "p:spPr"));
            k.push_opt(c.style.as_ref().map(|x| write_style(x, "p:style")));
            k.extras(&c.extra);
            k.build(El::new("p:cxnSp"))
        }
        ShapeNode::Frame(f) if f.alt.is_some() => {
            let Some(alt) = f.alt.as_deref() else {
                return XmlElement::new("mc:AlternateContent");
            };
            let mut plain = f.clone();
            plain.alt = None;
            if crate::zoom::is_zoom(f) {
                if let Some(old) = crate::zoom::moved_from(f, alt) {
                    crate::zoom::sync_rects(&mut plain, &old);
                }
                let el = write_node(&ShapeNode::Frame(plain));
                return crate::zoom::write_alternate(f, alt, &el);
            }
            if crate::model3d::moved(f, alt) {
                crate::model3d::sync_model_xfrm(&mut plain);
            }
            let el = write_node(&ShapeNode::Frame(plain));
            crate::model3d::write_alternate(f, alt, &el)
        }
        ShapeNode::Frame(f) => {
            let gd = match &f.content {
                GraphicContent::Table(t) => El::new("a:graphicData")
                    .a("uri", &f.uri)
                    .child(write_table(t))
                    .done(),
                GraphicContent::Other(x) => x.clone(),
            };
            let mut k = Kids::new(FRAME);
            k.push(write_nv(&f.nv, "p:nvGraphicFramePr", "p:cNvGraphicFramePr"));
            k.push(write_xfrm(&f.xfrm, "p:xfrm"));
            k.push(El::new("a:graphic").child(gd).done());
            k.extras(&f.extra);
            k.build(El::new("p:graphicFrame"))
        }
        ShapeNode::Other(x) => x.clone(),
    }
}

/// §19.3.1.16 cSld.
pub fn parse_csld(el: &XmlElement) -> CommonSlideData {
    let mut d = CommonSlideData {
        name: el.attr("name").map(str::to_owned),
        ..CommonSlideData::default()
    };
    d.extra = walk(el, CSLD, |c| {
        match c.name.as_str() {
            "p:bg" => {
                if let Some(bp) = c.child("p:bgPr") {
                    let mut fill = None;
                    let extra = walk(bp, BG_PR, |x| {
                        if is_fill(&x.name) && fill.is_none() {
                            fill = parse_fill(x);
                            return fill.is_some();
                        }
                        false
                    });
                    let Some(fill) = fill else {
                        return false;
                    };
                    d.background = Some(Background::Props {
                        fill,
                        shade_to_title: bp
                            .attr("shadeToTitle")
                            .and_then(bo_drawingml::dom::parse_bool),
                        extra,
                    });
                } else if let Some(br) = c.child("p:bgRef") {
                    d.background = Some(Background::Ref {
                        idx: attr_int(br, "idx").unwrap_or(0),
                        color: find_color(br),
                    });
                } else {
                    return false;
                }
            }
            "p:spTree" => {
                d.tree_extra = walk_tree(c, &mut d.tree_nv, &mut d.tree_props, &mut d.shapes);
            }
            _ => return false,
        }
        true
    });
    d
}

/// Write cSld.
pub fn write_csld(d: &CommonSlideData) -> XmlElement {
    let mut k = Kids::new(CSLD);
    match &d.background {
        Some(Background::Props {
            fill,
            shade_to_title,
            extra,
        }) => {
            let mut bk = Kids::new(BG_PR);
            bk.push(write_fill(fill));
            bk.extras(extra);
            let bp = bk.build(El::new("p:bgPr").optb("shadeToTitle", *shade_to_title));
            k.push(El::new("p:bg").child(bp).done());
        }
        Some(Background::Ref { idx, color }) => {
            let mut r = El::new("p:bgRef").a("idx", idx);
            if let Some(c) = color {
                r = r.child(write_color(c));
            }
            k.push(El::new("p:bg").child(r.done()).done());
        }
        None => {}
    }
    k.push(write_tree(
        "p:spTree",
        &d.tree_nv,
        &d.tree_props,
        &d.shapes,
        &d.tree_extra,
    ));
    k.extras(&d.extra);
    k.build(El::new("p:cSld").opt("name", d.name.as_ref()))
}

/// §19.3.1.7 clrMapOvr.
pub fn parse_clr_map_ovr(el: &XmlElement) -> Option<ColorMapOverride> {
    for c in el.elements() {
        match c.local() {
            "masterClrMapping" => return Some(ColorMapOverride::Master),
            "overrideClrMapping" => return Some(ColorMapOverride::Override(c.attrs.clone())),
            _ => {}
        }
    }
    None
}

/// Write clrMapOvr.
pub fn write_clr_map_ovr(m: &ColorMapOverride) -> XmlElement {
    let inner = match m {
        ColorMapOverride::Master => El::new("a:masterClrMapping").done(),
        ColorMapOverride::Override(map) => El::new("a:overrideClrMapping").rest(map).done(),
    };
    El::new("p:clrMapOvr").child(inner).done()
}

/// Iterate every node of a shape tree, depth first.
pub fn visit<'a>(nodes: &'a [ShapeNode], f: &mut impl FnMut(&'a ShapeNode)) {
    for n in nodes {
        f(n);
        if let ShapeNode::Group(g) = n {
            visit(&g.children, f);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bo_drawingml::dom::{canonical, parse};

    #[test]
    fn shape_tree_roundtrip() {
        let xml = r#"<p:cSld name="X"><p:bg><p:bgPr><a:solidFill><a:srgbClr val="FFFFFF"/></a:solidFill><a:effectLst/></p:bgPr></p:bg><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/><a:chOff x="0" y="0"/><a:chExt cx="0" cy="0"/></a:xfrm></p:grpSpPr><p:sp><p:nvSpPr><p:cNvPr id="2" name="Title 1"><a:extLst><a:ext uri="u"/></a:extLst></p:cNvPr><p:cNvSpPr><a:spLocks noGrp="1"/></p:cNvSpPr><p:nvPr><p:ph type="title"/></p:nvPr></p:nvSpPr><p:spPr/><p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:r><a:rPr lang="en-US"/><a:t>Hi</a:t></a:r></a:p></p:txBody></p:sp><mc:AlternateContent xmlns:mc="m"><mc:Choice Requires="x"/></mc:AlternateContent><p:grpSp><p:nvGrpSpPr><p:cNvPr id="3" name="G"/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/><p:cxnSp><p:nvCxnSpPr><p:cNvPr id="4" name="C"/><p:cNvCxnSpPr/><p:nvPr/></p:nvCxnSpPr><p:spPr><a:prstGeom prst="line"><a:avLst/></a:prstGeom></p:spPr></p:cxnSp></p:grpSp></p:spTree><p:extLst><p:ext uri="e"/></p:extLst></p:cSld>"#;
        let el = parse(xml.as_bytes(), "/t").unwrap();
        let d = parse_csld(&el);
        assert_eq!(d.shapes.len(), 3);
        assert!(matches!(d.shapes[1], ShapeNode::Other(_)));
        let ShapeNode::Shape(s) = &d.shapes[0] else {
            panic!()
        };
        assert_eq!(
            s.nv.placeholder.as_ref().unwrap().kind.as_deref(),
            Some("title")
        );
        assert_eq!(canonical(&write_csld(&d)), canonical(&el));
    }
}
