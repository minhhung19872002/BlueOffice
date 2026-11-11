//! SmartArt drawing fallback.
//!
//! A SmartArt graphic frame (`a:graphicData@uri` = DrawingML diagram, ECMA-376
//! Part 1, §21.4.2.6 `dgm:relIds`) points at the diagram data, layout, style and
//! colour parts. Laying out a diagram from its data model is not implemented;
//! instead the pre-rendered drawing part ([MS-ODRAWXML] §2.3.2 `dsp:drawing`,
//! shapes in DrawingML) is drawn. The drawing part is found through the data
//! model's `dsp:dataModelExt@relId` ([MS-ODRAWXML] dsp namespace), a relationship of the
//! part that holds the graphic frame (slide, sheet drawing, document).

use bo_model::slide::{ShapeNode, XmlElement, XmlNode};

/// `graphicData@uri` of a SmartArt diagram (ECMA-376 Part 1, §21.4).
pub const URI_DIAGRAM: &str = "http://schemas.openxmlformats.org/drawingml/2006/diagram";

/// Relationship type of the drawing fallback part ([MS-ODRAWXML] diagramDrawing).
pub const REL_DIAGRAM_DRAWING: &str =
    "http://schemas.microsoft.com/office/2007/relationships/diagramDrawing";

/// `dsp:` names become PresentationML names understood by the shape parser.
fn rename(el: &mut XmlElement) {
    if let Some(local) = el.name.strip_prefix("dsp:") {
        el.name = format!("p:{local}");
    }
    for c in &mut el.children {
        if let XmlNode::Element(e) = c {
            rename(e);
        }
    }
}

fn find<'a>(el: &'a XmlElement, local: &str) -> Option<&'a XmlElement> {
    if el.local() == local {
        return Some(el);
    }
    el.children.iter().find_map(|c| match c {
        XmlNode::Element(e) => find(e, local),
        XmlNode::Text(_) => None,
    })
}

/// Relationship id of the diagram data part (`dgm:relIds@r:dm`, §21.4.2.6).
pub fn data_rel_id(graphic_data: &XmlElement) -> Option<&str> {
    graphic_data
        .elements()
        .find(|c| c.local() == "relIds")
        .and_then(|c| c.attr("r:dm"))
}

/// Relationship id of the layout definition part (`dgm:relIds@r:lo`, §21.4.2.6).
pub fn layout_rel_id(graphic_data: &XmlElement) -> Option<&str> {
    graphic_data
        .elements()
        .find(|c| c.local() == "relIds")
        .and_then(|c| c.attr("r:lo"))
}

/// Relationship id of the drawing part named by a diagram data part
/// (`dsp:dataModelExt@relId`, [MS-ODRAWXML]), if any.
pub fn drawing_rel_id(data_xml: &[u8], part: &str) -> Option<String> {
    let root = bo_drawingml::dom::parse(data_xml, part).ok()?;
    find(&root, "dataModelExt")
        .and_then(|e| e.attr("relId"))
        .map(str::to_owned)
}

/// Top-level shapes of a diagram drawing part (`dsp:drawing/dsp:spTree`),
/// coordinates in EMU relative to the graphic frame.
pub fn drawing_shapes(xml: &[u8], part: &str) -> Option<Vec<ShapeNode>> {
    drawing_shapes_with(xml, part, None)
}

/// The absolute name of the image part behind a relationship id of a diagram part.
pub type Resolve<'a> = &'a dyn Fn(&str) -> Option<String>;

/// Pictures of a diagram part (`a:blip@r:embed` / `@r:link`, ECMA-376 Part 1, §20.1.8.13) name
/// relationships of that part, not of the part holding the frame: each id is replaced by the
/// absolute name of its image part, which the renderer takes as it is.
fn resolve_blips(el: &mut XmlElement, resolve: Resolve<'_>) {
    if el.local() == "blip" {
        for (k, v) in &mut el.attrs {
            if k == "r:embed" || k == "r:link" {
                // An id that does not resolve names nothing (never a relationship of the frame's part).
                *v = resolve(v).unwrap_or_default();
            }
        }
    }
    for c in &mut el.children {
        if let XmlNode::Element(e) = c {
            resolve_blips(e, resolve);
        }
    }
}

/// [`drawing_shapes`] with the part's pictures resolved through `resolve` (the relationships of
/// the drawing part).
pub fn drawing_shapes_with(
    xml: &[u8],
    part: &str,
    resolve: Option<Resolve<'_>>,
) -> Option<Vec<ShapeNode>> {
    let root = bo_drawingml::dom::parse(xml, part).ok()?;
    if root.local() != "drawing" {
        return None;
    }
    let tree = find(&root, "spTree")?;
    let mut out = Vec::new();
    for e in tree.elements() {
        if matches!(e.local(), "sp" | "grpSp" | "cxnSp" | "pic") {
            let mut e = e.clone();
            rename(&mut e);
            if let Some(r) = resolve {
                resolve_blips(&mut e, r);
            }
            out.push(crate::parse_node(&e));
        }
    }
    Some(out)
}

/// The media of a shape group (`bo_model::diagram::group_name`) for a frame of `w` × `h` EMU
/// laid out for `child` EMU: the shape tree `drawing` (a `dsp:drawing`) with that child space on
/// its group properties (§20.1.7.5 xfrm: `a:chOff` / `a:chExt`) and its picture fills naming
/// their image parts (`resolve`: the image part of a relationship id of the drawing part).
#[must_use]
pub fn group_tree(drawing: &[u8], child: (i64, i64), resolve: Resolve<'_>) -> Option<Vec<u8>> {
    let mut root = bo_drawingml::dom::parse(drawing, "/group.xml").ok()?;
    if root.local() != "drawing" {
        return None;
    }
    let tree = root.children.iter_mut().find_map(|c| match c {
        XmlNode::Element(e) if e.local() == "spTree" => Some(e),
        _ => None,
    })?;
    resolve_blips(tree, resolve);
    let (cx, cy) = child;
    let mut pr = XmlElement::new("dsp:grpSpPr");
    let mut xfrm = XmlElement::new("a:xfrm");
    for (name, a, b, x, y) in [
        ("a:off", "x", "y", 0, 0),
        ("a:ext", "cx", "cy", cx, cy),
        ("a:chOff", "x", "y", 0, 0),
        ("a:chExt", "cx", "cy", cx, cy),
    ] {
        let mut e = XmlElement::new(name);
        e.attrs = vec![(a.into(), x.to_string()), (b.into(), y.to_string())];
        xfrm.children.push(XmlNode::Element(e));
    }
    pr.children.push(XmlNode::Element(xfrm));
    let mut placed = false;
    for c in &mut tree.children {
        if let XmlNode::Element(e) = c {
            if e.local() == "grpSpPr" {
                *e = pr.clone();
                placed = true;
            }
        }
    }
    if !placed {
        tree.children.insert(0, XmlNode::Element(pr));
    }
    if !root.attrs.iter().any(|(k, _)| k == "xmlns:r") {
        root.attrs.push((
            "xmlns:r".into(),
            "http://schemas.openxmlformats.org/officeDocument/2006/relationships".into(),
        ));
    }
    Some(bo_drawingml::dom::to_part_xml(&root))
}

/// The shapes of a shape group's media ([`group_tree`]) for a frame of `w` × `h` EMU: one group
/// whose child space is the size the shapes were laid out for, so a resized frame scales them.
#[must_use]
pub fn group_shapes(xml: &[u8], part: &str, (w, h): (i64, i64)) -> Option<Vec<ShapeNode>> {
    use bo_model::slide::{GroupShape, ShapeProps, Xfrm};
    let root = bo_drawingml::dom::parse(xml, part).ok()?;
    let tree = find(&root, "spTree")?;
    let child = tree
        .elements()
        .find(|e| e.local() == "grpSpPr")
        .and_then(|g| find(g, "chExt"))
        .and_then(|e| {
            Some((
                e.attr("cx")?.parse::<i64>().ok()?,
                e.attr("cy")?.parse::<i64>().ok()?,
            ))
        })
        .filter(|(cw, ch)| *cw > 0 && *ch > 0)
        .unwrap_or((w, h));
    let children = drawing_shapes(xml, part)?;
    Some(vec![ShapeNode::Group(Box::new(GroupShape {
        props: ShapeProps {
            xfrm: Some(Xfrm {
                off: Some((0, 0)),
                ext: Some((w, h)),
                ch_off: Some((0, 0)),
                ch_ext: Some(child),
                ..Xfrm::default()
            }),
            ..ShapeProps::default()
        },
        children,
        ..GroupShape::default()
    }))])
}

/// The shapes of a sheet's shape group (`xdr:grpSp`, ECMA-376 Part 1, §20.5.2.17: its shapes are
/// DrawingML as a slide's are) fitted to a frame of `w` × `h` EMU; picture fills name their
/// image part.
#[must_use]
pub fn sheet_group_shapes(grp_sp: &str, (w, h): (i64, i64)) -> Option<Vec<ShapeNode>> {
    fn to_p(el: &mut XmlElement) {
        if let Some(local) = el.name.strip_prefix("xdr:") {
            el.name = format!("p:{local}");
        }
        for c in &mut el.children {
            if let XmlNode::Element(e) = c {
                to_p(e);
            }
        }
    }
    let mut root = bo_drawingml::dom::parse(grp_sp.as_bytes(), "/group.xml").ok()?;
    if root.local() != "grpSp" {
        return None;
    }
    to_p(&mut root);
    let mut node = crate::parse_node(&root);
    let ShapeNode::Group(g) = &mut node else {
        return None;
    };
    let mut x = g.props.xfrm.clone().unwrap_or_default();
    // The anchor gives the place and size; the child space stays the group's own.
    let child = x.ch_ext.or(x.ext).unwrap_or((w, h));
    x.ch_off = Some(x.ch_off.or(x.off).unwrap_or((0, 0)));
    x.ch_ext = Some(child);
    x.off = Some((0, 0));
    x.ext = Some((w, h));
    g.props.xfrm = Some(x);
    Some(vec![node])
}

/// Draw `shapes` (EMU, in a `w` × `h` box) as vector commands; the images they show are added
/// to `images` (by part name).
pub fn render_shapes(
    shapes: Vec<ShapeNode>,
    assets: &DiagramAssets<'_>,
    (w, h): (i64, i64),
    fonts: &mut bo_text::FontManager,
    images: &mut std::collections::BTreeMap<String, Vec<u8>>,
) -> Option<Vec<bo_render::Command>> {
    if shapes.is_empty() || w <= 0 || h <= 0 {
        return None;
    }
    let n = shapes.len();
    let pres = shapes_presentation(shapes, assets, (w, h));
    let opts = crate::RenderOptions {
        layer: Some(crate::ShapeRange {
            start: 0,
            end: n,
            backdrop: false,
        }),
        ..crate::RenderOptions::default()
    };
    crate::render_with(&pres, 0, fonts, images, &opts)
        .ok()
        .map(|p| p.commands)
}

/// Relationship type of an image part (ECMA-376 Part 1, §15.2.14).
pub const REL_IMAGE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/image";

/// The image relationships (id, absolute part name) of a diagram part `part` from its
/// relationships part `rels_xml`.
#[must_use]
pub fn image_rels(rels_xml: &[u8], part: &str) -> Vec<(String, String)> {
    bo_opc::parse_relationships(rels_xml, &bo_opc::rels_part_name(part))
        .unwrap_or_default()
        .into_iter()
        .filter(|r| r.rel_type == REL_IMAGE && r.mode == bo_opc::TargetMode::Internal)
        .map(|r| (r.id, bo_opc::resolve_target(part, &r.target)))
        .collect()
}

/// The relationships part of a diagram part in directory `<x>/diagrams` whose pictures
/// (id, absolute part name) are `images`; the data model part and the drawing part of a diagram
/// BlueOffice writes share one such list.
#[must_use]
pub fn image_rels_xml(part: &str, images: &[(String, String)]) -> Vec<u8> {
    let mut s = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">",
    );
    for (id, name) in images {
        s.push_str(&format!(
            "<Relationship Id=\"{}\" Type=\"{REL_IMAGE}\" Target=\"{}\"/>",
            bo_opc::xml::escape(id),
            bo_opc::xml::escape(&relative(part, name)),
        ));
    }
    s.push_str("</Relationships>");
    s.into_bytes()
}

/// `target` relative to the directory of part `from` (both absolute part names).
fn relative(from: &str, target: &str) -> String {
    let dir: Vec<&str> = from.split('/').filter(|s| !s.is_empty()).collect();
    let dir = &dir[..dir.len().saturating_sub(1)];
    let to: Vec<&str> = target.split('/').filter(|s| !s.is_empty()).collect();
    let common = dir.iter().zip(&to).take_while(|(a, b)| a == b).count();
    let mut out: Vec<&str> = vec![".."; dir.len() - common];
    out.extend(&to[common..]);
    out.join("/")
}

/// Shapes of a diagram in a `w`×`h` EMU frame from either its drawing part or, for a SmartArt saved
/// without one, its data model part (laid out by [`crate::diagram_layout`]).
pub fn diagram_shapes(xml: &[u8], part: &str, w: i64, h: i64) -> Option<Vec<ShapeNode>> {
    diagram_shapes_with(xml, part, &DiagramContext::default(), w, h)
}

/// What a diagram part is drawn with besides its own XML.
#[derive(Clone, Copy, Default)]
pub struct DiagramContext<'a> {
    /// The image part of a relationship id of the part.
    pub resolve: Option<Resolve<'a>>,
    /// The layout definition part (for a data model laid out by BlueOffice).
    pub layout: Option<&'a [u8]>,
    /// The diagram's own quick style and colours (for a data model laid out by BlueOffice).
    pub defs: Option<&'a crate::diagram_layout::Defs>,
}

/// [`diagram_shapes`] with the diagram's pictures and definitions.
pub fn diagram_shapes_with(
    xml: &[u8],
    part: &str,
    cx: &DiagramContext<'_>,
    w: i64,
    h: i64,
) -> Option<Vec<ShapeNode>> {
    drawing_shapes_with(xml, part, cx.resolve).or_else(|| {
        crate::diagram_layout::synth_with(xml, part, cx.layout, cx.defs, cx.resolve, w, h)
    })
}

/// Draw a diagram drawing part as vector commands in a `w`×`h` EMU box
/// (no background; the frame's colours come from the shapes themselves).
pub fn render_drawing(
    xml: &[u8],
    part: &str,
    w: i64,
    h: i64,
    fonts: &mut bo_text::FontManager,
) -> Option<Vec<bo_render::Command>> {
    let mut images = std::collections::BTreeMap::new();
    render_drawing_with(
        xml,
        part,
        &DiagramContext::default(),
        &DiagramAssets::default(),
        (w, h),
        fonts,
        &mut images,
    )
}

/// What the shapes of a diagram outside a presentation are drawn with: the theme its scheme
/// colours and style references resolve against, and its image parts.
#[derive(Clone, Copy, Default)]
pub struct DiagramAssets<'a> {
    /// The document's theme.
    pub theme: Option<&'a bo_model::slide::Theme>,
    /// Bytes of an image part by absolute name.
    pub image: Option<ImageBytes<'a>>,
}

/// Bytes of an image part by absolute name.
pub type ImageBytes<'a> = &'a dyn Fn(&str) -> Option<Vec<u8>>;

/// A one-slide presentation of `w`×`h` EMU holding `shapes` (a diagram drawn outside BlueSlide),
/// with the image parts the shapes name.
#[must_use]
pub fn shapes_presentation(
    shapes: Vec<ShapeNode>,
    assets: &DiagramAssets<'_>,
    (w, h): (i64, i64),
) -> bo_model::slide::Presentation {
    use bo_model::slide::{
        CommonSlideData, Presentation, RawPart, Slide, SlideLayout, SlideMaster, SlideSize,
    };
    fn blips(n: &ShapeNode, out: &mut Vec<String>) {
        let mut fill = |p: &bo_model::slide::ShapeProps| {
            if let Some(bo_model::slide::Fill::Picture(b)) = &p.fill {
                out.extend(b.embed.clone());
            }
        };
        match n {
            ShapeNode::Shape(s) => fill(&s.props),
            ShapeNode::Picture(p) => out.extend(p.blip.embed.clone()),
            ShapeNode::Group(g) => {
                for c in &g.children {
                    blips(c, out);
                }
            }
            _ => {}
        }
    }
    let mut names = Vec::new();
    for s in &shapes {
        blips(s, &mut names);
    }
    names.sort();
    names.dedup();
    let parts = names
        .into_iter()
        .filter(|n| n.starts_with('/'))
        .filter_map(|n| {
            let data = assets.image.and_then(|f| f(&n))?;
            Some(RawPart {
                name: n,
                content_type: None,
                data,
            })
        })
        .collect();
    Presentation {
        slide_size: SlideSize {
            cx: w,
            cy: h,
            kind: None,
        },
        // The theme is reached through a master and a layout (both empty).
        themes: assets.theme.cloned().into_iter().collect(),
        masters: assets
            .theme
            .map(|_| SlideMaster::default())
            .into_iter()
            .collect(),
        layouts: assets
            .theme
            .map(|_| SlideLayout::default())
            .into_iter()
            .collect(),
        slides: vec![Slide {
            data: CommonSlideData {
                shapes,
                ..CommonSlideData::default()
            },
            ..Slide::default()
        }],
        parts,
        ..Presentation::default()
    }
}

/// [`render_drawing`] with the diagram's pictures, definitions and theme; the images the drawing
/// shows are added to `images` (by part name).
pub fn render_drawing_with(
    xml: &[u8],
    part: &str,
    cx: &DiagramContext<'_>,
    assets: &DiagramAssets<'_>,
    (w, h): (i64, i64),
    fonts: &mut bo_text::FontManager,
    images: &mut std::collections::BTreeMap<String, Vec<u8>>,
) -> Option<Vec<bo_render::Command>> {
    let shapes = diagram_shapes_with(xml, part, cx, w, h)?;
    if shapes.is_empty() || w <= 0 || h <= 0 {
        return None;
    }
    let n = shapes.len();
    let pres = shapes_presentation(shapes, assets, (w, h));
    let opts = crate::RenderOptions {
        layer: Some(crate::ShapeRange {
            start: 0,
            end: n,
            backdrop: false,
        }),
        ..crate::RenderOptions::default()
    };
    crate::render_with(&pres, 0, fonts, images, &opts)
        .ok()
        .map(|p| p.commands)
}

/// The nodes of a SmartArt and the node each drawn shape belongs to, for building it node by node
/// in a slide show (DrawingML §20.1.2.2 bldDgm; §19.5 graphicEl → dgm targets).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DiagramBuild {
    /// Content points (§21.4.3.8 pt of type `node` / `asst`) in reading order (depth first by
    /// §21.4.3.1 cxn `srcOrd`): model id and level (1 = under the document point).
    pub nodes: Vec<(String, u32)>,
    /// For each top-level shape of the drawing part: the index in `nodes` of its node; `None` =
    /// part of the background.
    pub shapes: Vec<Option<usize>>,
}

fn text_of_el(el: &XmlElement) -> String {
    let mut out = String::new();
    for c in &el.children {
        match c {
            XmlNode::Text(t) if el.local() == "t" => out.push_str(t),
            XmlNode::Element(e) => out.push_str(&text_of_el(e)),
            XmlNode::Text(_) => {}
        }
    }
    out
}

/// Content points under `id` by `parOf` links, depth first in order, with their level.
fn walk(
    id: &str,
    level: u32,
    links: &[(&str, u32, &str)],
    content: &dyn Fn(&str) -> bool,
    out: &mut Vec<(String, u32)>,
) {
    if level > 32 {
        return;
    }
    for l in links.iter().filter(|l| l.0 == id) {
        if content(l.2) && !out.iter().any(|(n, _)| n == l.2) {
            out.push((l.2.to_owned(), level));
            walk(l.2, level + 1, links, content, out);
        }
    }
}

/// Who builds with whom in a SmartArt: its data model part and its drawing part ([MS-ODRAWXML]
/// dsp:drawing). A shape's `modelId` names a node, a presentation point of one (`presOf`
/// connection, `prSet@presAssocID`) or a connection's transition point (`parTrans` / `sibTrans`,
/// built with the connection's destination node); shapes naming nothing known are matched to the
/// nodes by their text, in order.
#[must_use]
pub fn diagram_build(
    data_xml: &[u8],
    data_part: &str,
    drawing_xml: &[u8],
    drawing_part: &str,
) -> Option<DiagramBuild> {
    let data = bo_drawingml::dom::parse(data_xml, data_part).ok()?;
    let drawing = bo_drawingml::dom::parse(drawing_xml, drawing_part).ok()?;
    if data.local() != "dataModel" || drawing.local() != "drawing" {
        return None;
    }
    let kids = |el: &XmlElement, list: &str, item: &str| -> Vec<XmlElement> {
        el.elements()
            .find(|c| c.local() == list)
            .map(|l| {
                l.elements()
                    .filter(|c| c.local() == item)
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    };
    let pts = kids(&data, "ptLst", "pt");
    let cxns = kids(&data, "cxnLst", "cxn");
    let kind_of = |id: &str| {
        pts.iter()
            .find(|p| p.attr("modelId") == Some(id))
            .map(|p| p.attr("type").unwrap_or("node"))
    };
    // parOf connections (§21.4.7.15 ST_CxnType, default parOf): (parent, order, child).
    let mut links: Vec<(&str, u32, &str)> = cxns
        .iter()
        .filter(|c| c.attr("type").is_none_or(|t| t == "parOf"))
        .filter_map(|c| {
            Some((
                c.attr("srcId")?,
                c.attr("srcOrd").and_then(|o| o.parse().ok()).unwrap_or(0),
                c.attr("destId")?,
            ))
        })
        .collect();
    links.sort_by(|a, b| a.0.cmp(b.0).then(a.1.cmp(&b.1)));
    let content = |id: &str| matches!(kind_of(id), Some("node" | "asst"));
    let mut nodes: Vec<(String, u32)> = Vec::new();
    if let Some(doc) = pts.iter().find(|p| p.attr("type") == Some("doc")) {
        walk(
            doc.attr("modelId").unwrap_or_default(),
            1,
            &links,
            &content,
            &mut nodes,
        );
    }
    if nodes.is_empty() {
        for p in &pts {
            let id = p.attr("modelId").unwrap_or_default();
            if content(id) && !links.iter().any(|l| l.2 == id) {
                nodes.push((id.to_owned(), 1));
                walk(id, 2, &links, &content, &mut nodes);
            }
        }
    }
    let node_index = |id: &str| nodes.iter().position(|(n, _)| n == id);
    // The node a model id stands for (a few hops: presentation point → transition → connection).
    let resolve = |start: &str| -> Option<usize> {
        let mut id = start.to_owned();
        for _ in 0..4 {
            if let Some(i) = node_index(&id) {
                return Some(i);
            }
            let pt = pts.iter().find(|p| p.attr("modelId") == Some(id.as_str()));
            let next = match pt.map(|p| p.attr("type").unwrap_or("node")) {
                Some("pres") => pt
                    .and_then(|p| p.elements().find(|c| c.local() == "prSet"))
                    .and_then(|s| s.attr("presAssocID"))
                    .map(str::to_owned)
                    .or_else(|| {
                        cxns.iter()
                            .find(|c| {
                                c.attr("type") == Some("presOf")
                                    && c.attr("destId") == Some(id.as_str())
                            })
                            .and_then(|c| c.attr("srcId"))
                            .map(str::to_owned)
                    }),
                Some("parTrans" | "sibTrans") => pt
                    .and_then(|p| p.attr("cxnId"))
                    .and_then(|cx| cxns.iter().find(|c| c.attr("modelId") == Some(cx)))
                    .and_then(|c| c.attr("destId"))
                    .map(str::to_owned),
                _ => None,
            };
            id = next?;
        }
        None
    };
    let tree = find(&drawing, "spTree")?;
    let texts: Vec<String> = nodes
        .iter()
        .map(|(id, _)| {
            pts.iter()
                .find(|p| p.attr("modelId") == Some(id.as_str()))
                .and_then(|p| p.elements().find(|c| c.local() == "t"))
                .map(|t| text_of_el(t).trim().to_owned())
                .unwrap_or_default()
        })
        .collect();
    let mut used = vec![false; nodes.len()];
    let mut shapes = Vec::new();
    for e in tree
        .elements()
        .filter(|e| matches!(e.local(), "sp" | "grpSp" | "cxnSp" | "pic"))
    {
        let mut hit = e.attr("modelId").and_then(resolve);
        if hit.is_none() {
            let t = text_of_el(e).trim().to_owned();
            if !t.is_empty() {
                hit = (0..nodes.len()).find(|&i| !used[i] && texts[i] == t);
                if let Some(i) = hit {
                    used[i] = true;
                }
            }
        }
        shapes.push(hit);
    }
    Some(DiagramBuild { nodes, shapes })
}

/// The drawing part (name) and data part (name) of a SmartArt frame's graphic data, found through
/// the relationships `rels` of the part `from` holding the frame.
#[must_use]
pub fn frame_parts(
    graphic_data: &XmlElement,
    rels: &[bo_model::slide::Rel],
    from: &str,
    part_data: &dyn Fn(&str) -> Option<Vec<u8>>,
) -> Option<(String, String)> {
    let target = |rid: &str| {
        rels.iter()
            .find(|r| r.id == rid && !r.external)
            .map(|r| bo_opc::resolve_target(from, &r.target))
    };
    let data = target(data_rel_id(graphic_data)?)?;
    let drawing = drawing_rel_id(&part_data(&data)?, &data)
        .and_then(|rid| target(&rid))
        .or_else(|| {
            let mut it = rels
                .iter()
                .filter(|r| r.rel_type == REL_DIAGRAM_DRAWING && !r.external);
            match (it.next(), it.next()) {
                (Some(r), None) => Some(bo_opc::resolve_target(from, &r.target)),
                _ => None,
            }
        })?;
    Some((data, drawing))
}

#[cfg(test)]
mod tests {
    use super::*;

    const DSP: &str = "http://schemas.microsoft.com/office/drawing/2008/diagram";

    // source: §21.4.3 data model (pt types node / pres / sibTrans, cxn parOf / presOf with srcOrd,
    // prSet@presAssocID) — nodes in reading order with their levels; drawing shapes resolved to
    // their node through their modelId, else by their text.
    #[test]
    fn diagram_nodes_and_their_shapes() {
        let data = r#"<dgm:dataModel xmlns:dgm="d" xmlns:a="a"><dgm:ptLst><dgm:pt modelId="D" type="doc"/><dgm:pt modelId="B"><dgm:t><a:p><a:r><a:t>Hai</a:t></a:r></a:p></dgm:t></dgm:pt><dgm:pt modelId="A"><dgm:t><a:p><a:r><a:t>Một</a:t></a:r></a:p></dgm:t></dgm:pt><dgm:pt modelId="A1"><dgm:t><a:p><a:r><a:t>Một con</a:t></a:r></a:p></dgm:t></dgm:pt><dgm:pt modelId="PA" type="pres"><dgm:prSet presAssocID="A"/></dgm:pt><dgm:pt modelId="T" type="sibTrans" cxnId="C2"/><dgm:pt modelId="PT" type="pres"/></dgm:ptLst><dgm:cxnLst><dgm:cxn modelId="C1" srcId="D" destId="A" srcOrd="0"/><dgm:cxn modelId="C2" srcId="D" destId="B" srcOrd="1" sibTransId="T"/><dgm:cxn modelId="C3" srcId="A" destId="A1" srcOrd="0"/><dgm:cxn modelId="C4" type="presOf" srcId="T" destId="PT"/></dgm:cxnLst></dgm:dataModel>"#;
        let sp = |id: &str, text: &str| {
            format!(
                r#"<dsp:sp modelId="{id}"><dsp:txBody><a:p><a:r><a:t>{text}</a:t></a:r></a:p></dsp:txBody></dsp:sp>"#
            )
        };
        let drawing = format!(
            r#"<dsp:drawing xmlns:dsp="{DSP}" xmlns:a="a"><dsp:spTree><dsp:nvGrpSpPr/>{}{}{}{}{}</dsp:spTree></dsp:drawing>"#,
            sp("PA", ""),
            sp("PT", ""),
            sp("X1", "Hai"),
            sp("X2", "Một con"),
            sp("X3", "")
        );
        let b = diagram_build(data.as_bytes(), "/d", drawing.as_bytes(), "/w").unwrap();
        assert_eq!(
            b.nodes,
            vec![("A".into(), 1), ("A1".into(), 2), ("B".into(), 1)]
        );
        // PA → A (presAssocID); PT → T (presOf) → C2 → B; by text: B, A1; the last: background.
        assert_eq!(b.shapes, vec![Some(0), Some(2), Some(2), Some(1), None]);
    }

    #[test]
    fn drawing_part_shapes_and_ids() {
        let drawing = format!(
            r#"<dsp:drawing xmlns:dsp="{DSP}" xmlns:a="a"><dsp:spTree><dsp:nvGrpSpPr><dsp:cNvPr id="0" name=""/><dsp:cNvGrpSpPr/></dsp:nvGrpSpPr><dsp:grpSpPr/><dsp:sp modelId="{{1}}"><dsp:nvSpPr><dsp:cNvPr id="0" name=""/><dsp:cNvSpPr/></dsp:nvSpPr><dsp:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="100" cy="100"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></dsp:spPr><dsp:txBody><a:bodyPr/><a:lstStyle/><a:p><a:r><a:t>Bước 1</a:t></a:r></a:p></dsp:txBody></dsp:sp></dsp:spTree></dsp:drawing>"#
        );
        let shapes = drawing_shapes(drawing.as_bytes(), "/ppt/diagrams/drawing1.xml").unwrap();
        assert_eq!(shapes.len(), 1);
        let ShapeNode::Shape(s) = &shapes[0] else {
            panic!("shape")
        };
        assert!(s.text.is_some());
        let data = format!(
            r#"<dgm:dataModel xmlns:dgm="d" xmlns:a="a"><dgm:extLst><a:ext uri="x"><dsp:dataModelExt xmlns:dsp="{DSP}" relId="rId7" minVer="m"/></a:ext></dgm:extLst></dgm:dataModel>"#
        );
        assert_eq!(
            drawing_rel_id(data.as_bytes(), "/ppt/diagrams/data1.xml").as_deref(),
            Some("rId7")
        );
        assert!(drawing_shapes(b"<p:sld/>", "x").is_none());
        // A data model without a drawing part is laid out instead (CORE-SMART-002).
        let dm = crate::diagram_layout::tests::data_model(
            "urn:x/layout/process1",
            &[("A", None), ("B", None)],
        );
        let shapes = diagram_shapes(dm.as_bytes(), "/d.xml", 4_000_000, 1_000_000).unwrap();
        assert_eq!(shapes.len(), 3, "two steps and an arrow");
        assert!(drawing_shapes(b"not xml <", "x").is_none());
    }
}
