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

#[cfg(test)]
mod tests {
    use super::*;

    const DSP: &str = "http://schemas.microsoft.com/office/drawing/2008/diagram";

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
