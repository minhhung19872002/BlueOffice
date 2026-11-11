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
            out.push(crate::parse_node(&e));
        }
    }
    Some(out)
}

/// Shapes of a diagram in a `w`×`h` EMU frame from either its drawing part or, for a SmartArt saved
/// without one, its data model part (laid out by [`crate::diagram_layout`]).
pub fn diagram_shapes(xml: &[u8], part: &str, w: i64, h: i64) -> Option<Vec<ShapeNode>> {
    drawing_shapes(xml, part).or_else(|| crate::diagram_layout::synth(xml, part, None, w, h))
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
    use bo_model::slide::{CommonSlideData, Presentation, Slide, SlideSize};
    let shapes = diagram_shapes(xml, part, w, h)?;
    if shapes.is_empty() || w <= 0 || h <= 0 {
        return None;
    }
    let n = shapes.len();
    let pres = Presentation {
        slide_size: SlideSize {
            cx: w,
            cy: h,
            kind: None,
        },
        slides: vec![Slide {
            data: CommonSlideData {
                shapes,
                ..CommonSlideData::default()
            },
            ..Slide::default()
        }],
        ..Presentation::default()
    };
    let opts = crate::RenderOptions {
        layer: Some(crate::ShapeRange {
            start: 0,
            end: n,
            backdrop: false,
        }),
        ..crate::RenderOptions::default()
    };
    let mut images = std::collections::BTreeMap::new();
    crate::render_with(&pres, 0, fonts, &mut images, &opts)
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
