//! SmartArt in BlueDoc: the diagram's drawing part ([MS-ODRAWXML] §2.3.2 `dsp:drawing`, shapes in
//! DrawingML, ECMA-376 Part 1 §20.1) — or, without one, its data model laid out by
//! `bo_pptx::diagram_layout` — is drawn with the BlueSlide shape renderer and shown as a picture.
//! The pictures live in [`SmartArtImages`], next to the document: the parts themselves stay in the
//! document's media as XML (kept verbatim when read, written as they are for new / edited
//! diagrams, CORE-SMART-005), so undo and save see the real parts.

use bo_model::diagram;
use bo_model::slide::ShapeNode;
use bo_model::Document;
use bo_text::FontManager;
use std::collections::BTreeMap;
use std::sync::Arc;

/// Shapes of a diagram drawing part.
pub fn drawing_shapes(xml: &[u8]) -> Option<Vec<ShapeNode>> {
    bo_pptx::diagram::drawing_shapes(xml, "/word/diagrams/drawing.xml")
}

/// Render a diagram drawing (`w`×`h` EMU) to PNG at 2 px per point; a data model part (SmartArt saved
/// without its drawing) is laid out first (`bo_pptx::diagram_layout`).
pub fn render_png(xml: &[u8], w: i64, h: i64, fonts: &mut FontManager) -> Option<Vec<u8>> {
    render_png_with(
        xml,
        &bo_pptx::diagram::DiagramContext::default(),
        &bo_pptx::diagram::DiagramAssets::default(),
        (w, h),
        fonts,
    )
}

/// [`render_png`] with the diagram's pictures, definitions and the document theme.
pub fn render_png_with(
    xml: &[u8],
    cx: &bo_pptx::diagram::DiagramContext<'_>,
    assets: &bo_pptx::diagram::DiagramAssets<'_>,
    (w, h): (i64, i64),
    fonts: &mut FontManager,
) -> Option<Vec<u8>> {
    let shapes =
        bo_pptx::diagram::diagram_shapes_with(xml, "/word/diagrams/drawing.xml", cx, w, h)?;
    if shapes.is_empty() || w <= 0 || h <= 0 {
        return None;
    }
    let pres = bo_pptx::diagram::shapes_presentation(shapes, assets, (w, h));
    let mut images = BTreeMap::new();
    let page = bo_pptx::render_slide(&pres, 0, fonts, &mut images).ok()?;
    bo_raster::render_page_png_with(&page, fonts, 2.0, &images).ok()
}

/// Draw the shape group of frame `img` (its media `bytes`, `bo_model::diagram::group_name`): its
/// shapes scaled to the frame, pictures from the document's media.
fn render_group(
    doc: &Document,
    img: &bo_model::Image,
    bytes: &[u8],
    theme: Option<&bo_model::slide::Theme>,
    fonts: &mut FontManager,
) -> Option<Vec<u8>> {
    let size = (img.width.0, img.height.0);
    if size.0 <= 0 || size.1 <= 0 {
        return None;
    }
    let shapes = bo_pptx::diagram::group_shapes(bytes, &img.media, size)?;
    let image = |name: &str| doc.media.get(name).map(|b| b.as_ref().clone());
    let assets = bo_pptx::diagram::DiagramAssets {
        theme,
        image: Some(&image),
    };
    let pres = bo_pptx::diagram::shapes_presentation(shapes, &assets, size);
    let mut images = BTreeMap::new();
    let page = bo_pptx::render_slide(&pres, 0, fonts, &mut images).ok()?;
    bo_raster::render_page_png_with(&page, fonts, 2.0, &images).ok()
}

/// Draw the diagram of frame `img` (its media part `bytes`) with what its other parts give: the
/// pictures of its nodes, and — for a data model BlueOffice lays out — its layout definition and
/// its own quick style and colours.
fn render_frame(
    doc: &Document,
    img: &bo_model::Image,
    bytes: &[u8],
    theme: Option<&bo_model::slide::Theme>,
    pkg: &mut Option<Option<bo_opc::Package>>,
    fonts: &mut FontManager,
) -> Option<Vec<u8>> {
    let size = (img.width.0, img.height.0);
    // A file diagram's other parts are in the source package: opened once for all diagrams.
    let pkg = img
        .src
        .is_some()
        .then(|| {
            pkg.get_or_insert_with(|| bo_docx::diagram::source_package(doc))
                .as_ref()
        })
        .flatten();
    let Some(src) = bo_docx::diagram::diagram_source_in(doc, img, pkg) else {
        return render_png(bytes, size.0, size.1, fonts);
    };
    // The media part is the drawing part, or (without one) the data model part.
    let is_data =
        diagram::split_name(&img.media).is_some_and(|(k, _, _)| k == diagram::DiagramPart::Data);
    let rels = if is_data {
        &src.pictures
    } else {
        &src.drawing_pictures
    };
    let resolve = |rid: &str| {
        rels.iter()
            .find(|(id, _)| id == rid)
            .map(|(_, name)| name.clone())
    };
    let image = |name: &str| {
        src.images
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, b)| b.as_ref().clone())
    };
    let defs = bo_pptx::diagram_layout::Defs::parse(
        src.style.as_ref().map(|s| s.1.as_slice()),
        src.colors.as_ref().map(|s| s.1.as_slice()),
    );
    let cx = bo_pptx::diagram::DiagramContext {
        resolve: Some(&resolve),
        layout: src.layout.as_ref().map(|l| l.1.as_slice()),
        defs: Some(&defs),
    };
    let assets = bo_pptx::diagram::DiagramAssets {
        theme,
        image: Some(&image),
    };
    render_png_with(bytes, &cx, &assets, size, fonts)
}

/// One rendered diagram or chart: the part bytes it was drawn from, the PNG and, for a chart, its
/// vector drawing (page of the frame's size).
#[derive(Debug, Clone, Default)]
struct Rendered {
    source: Arc<Vec<u8>>,
    size: (i64, i64),
    png: Vec<u8>,
    vector: Option<bo_render::PageList>,
}

/// Is a media part one this cache draws (a SmartArt diagram part or a chart part)?
fn drawn(name: &str, bytes: &[u8]) -> bool {
    (diagram::split_name(name).is_some() && diagram::is_diagram_xml(bytes))
        || (bo_model::chart_part::is_chart_name(name) && bo_model::chart_part::is_chart_xml(bytes))
}

/// Pictures of the SmartArt diagrams and charts of the open document, by media part name.
#[derive(Debug, Clone, Default)]
pub struct SmartArtImages {
    map: BTreeMap<String, Rendered>,
    /// Diagram media (name, bytes address) the pictures were drawn for.
    key: Vec<(String, usize)>,
}

impl SmartArtImages {
    /// The picture of diagram part `name`.
    pub fn get(&self, name: &str) -> Option<&[u8]> {
        self.map.get(name).map(|r| r.png.as_slice())
    }

    /// The vector drawing of chart part `name` (a page of the frame's size).
    pub fn vector(&self, name: &str) -> Option<&bo_render::PageList> {
        self.map.get(name).and_then(|r| r.vector.as_ref())
    }

    /// Replace the pictures of charts in `list` by their vector drawing, scaled to the picture box.
    pub fn vectorize(&self, list: &mut bo_render::PageList) {
        use bo_render::Command;
        if !list
            .commands
            .iter()
            .any(|c| matches!(c, Command::Image { image, .. } if self.vector(image).is_some()))
        {
            return;
        }
        let mut out = Vec::with_capacity(list.commands.len());
        for c in std::mem::take(&mut list.commands) {
            match &c {
                Command::Image {
                    x,
                    y,
                    w,
                    h,
                    image,
                    crop: None,
                    rotation,
                } if *rotation == 0.0 => {
                    if let Some(v) = self
                        .vector(image)
                        .filter(|v| v.width > 0.0 && v.height > 0.0)
                    {
                        out.push(Command::PushClip {
                            x: *x,
                            y: *y,
                            w: *w,
                            h: *h,
                        });
                        out.push(Command::PushTransform {
                            m: [w / v.width, 0.0, 0.0, h / v.height, *x, *y],
                        });
                        out.extend(v.commands.iter().cloned());
                        out.push(Command::PopTransform);
                        out.push(Command::PopClip);
                        continue;
                    }
                    out.push(c);
                }
                _ => out.push(c),
            }
        }
        list.commands = out;
    }

    /// Forget every picture (another document is opened).
    pub fn clear(&mut self) {
        *self = SmartArtImages::default();
    }

    /// Draw the diagrams of `doc` whose parts changed since the last call.
    pub fn refresh(&mut self, doc: &Document, fonts: &mut FontManager) {
        let key: Vec<(String, usize)> = doc
            .media
            .iter()
            .filter(|(n, b)| drawn(n, b))
            .map(|(n, b)| (n.clone(), Arc::as_ptr(b) as usize))
            .collect();
        if key == self.key {
            return;
        }
        self.key = key;
        let mut next = BTreeMap::new();
        // The document theme gives the chart colours and the scheme colours of diagrams; read
        // once, when a chart or diagram is drawn.
        let mut theme: Option<Option<bo_model::slide::Theme>> = None;
        // The source package, opened when the first file diagram is drawn.
        let mut pkg: Option<Option<bo_opc::Package>> = None;
        // The frame of each diagram part (the first that shows it).
        let mut frames: BTreeMap<&str, &bo_model::Image> = BTreeMap::new();
        for img in crate::media::pictures(doc) {
            frames.entry(img.media.as_str()).or_insert(img);
        }
        for (name, (w, h)) in crate::media::sizes(doc) {
            let Some(bytes) = doc.media.get(&name) else {
                continue;
            };
            if !drawn(&name, bytes) {
                continue;
            }
            match self.map.remove(&name) {
                Some(r) if Arc::ptr_eq(&r.source, bytes) && r.size == (w, h) => {
                    next.insert(name, r);
                }
                _ if diagram::is_diagram_xml(bytes) => {
                    let theme = theme.get_or_insert_with(|| crate::doc_chart::doc_theme(doc));
                    let png = match frames.get(name.as_str()) {
                        Some(img) if diagram::is_group_name(&name) => {
                            render_group(doc, img, bytes, theme.as_ref(), fonts)
                        }
                        Some(img) => render_frame(doc, img, bytes, theme.as_ref(), &mut pkg, fonts),
                        None => render_png(bytes, w, h, fonts),
                    };
                    if let Some(png) = png {
                        next.insert(
                            name,
                            Rendered {
                                source: bytes.clone(),
                                size: (w, h),
                                png,
                                vector: None,
                            },
                        );
                    }
                }
                _ => {
                    let theme = theme.get_or_insert_with(|| crate::doc_chart::doc_theme(doc));
                    let Some(page) = crate::doc_chart::render(theme.as_ref(), bytes, w, h, fonts)
                    else {
                        continue;
                    };
                    let png = bo_raster::render_page_png_with(
                        &page,
                        fonts,
                        2.0,
                        &BTreeMap::<String, Vec<u8>>::new(),
                    )
                    .unwrap_or_default();
                    next.insert(
                        name,
                        Rendered {
                            source: bytes.clone(),
                            size: (w, h),
                            png,
                            vector: Some(page),
                        },
                    );
                }
            }
        }
        self.map = next;
    }
}

impl bo_render::ImageSource for SmartArtImages {
    fn image(&self, id: &str) -> Option<&[u8]> {
        self.get(id)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    // source: corpus smartart-04-datamodel.docx (data model only, ECMA-376 Part 1 §21.4.3) — CORE-SMART-002.
    #[test]
    fn data_model_smartart_is_laid_out_and_drawn() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/corpus/doc/smartart-04-datamodel.docx"
        );
        let doc = bo_docx::read_docx(&std::fs::read(path).unwrap()).unwrap();
        // The picture of the frame is the data model part, kept verbatim.
        let (name, bytes) = doc
            .media
            .iter()
            .find(|(n, _)| n.ends_with("data1.xml"))
            .map(|(n, b)| (n.clone(), b.clone()))
            .unwrap();
        assert!(String::from_utf8_lossy(&bytes).contains("dgm:dataModel"));
        let mut fonts = bo_text::FontManager::new();
        for f in crate::bundled::FONTS {
            fonts.add_font(f.to_vec());
        }
        let mut images = super::SmartArtImages::default();
        images.refresh(&doc, &mut fonts);
        let png = images.get(&name).unwrap();
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
        // The document's media keep the part as XML; so does the saved file.
        assert_eq!(doc.media[&name].as_slice(), bytes.as_slice());
        let saved = bo_docx::write_docx(&doc).unwrap();
        let back = bo_docx::read_docx(&saved).unwrap();
        assert_eq!(back.media[&name].as_slice(), bytes.as_slice());
    }
}

// ---------------------------------------------------------------------------------------------
// Shared by BlueSlide, BlueDoc and BlueSheet: the items of the text pane with their pictures, the
// layout previews of the dialog and which node a point of a diagram is on.
// ---------------------------------------------------------------------------------------------

use crate::slide_edit::smartart::SmartItem;
use bo_pptx::diagram_layout::{self as dl, DiagramNode, Layout, Look};

/// Pictures of a diagram: (relationship id in its data model and drawing parts, image part name).
pub type Pictures = Vec<(String, String)>;

/// A picture given by a command as a `data:` URL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewPicture {
    /// Image bytes.
    pub data: Vec<u8>,
    /// File extension of its part (`png` | `jpeg`).
    pub ext: &'static str,
    /// Content type.
    pub mime: &'static str,
}

/// The PNG or JPEG picture of a `data:…;base64,…` URL (the format is read from the bytes).
#[must_use]
pub fn decode_picture(url: &str) -> Option<NewPicture> {
    use base64::Engine as _;
    let (head, b64) = url.strip_prefix("data:")?.split_once(',')?;
    if !head.ends_with(";base64") {
        return None;
    }
    let data = base64::engine::general_purpose::STANDARD
        .decode(b64.trim())
        .ok()?;
    let (ext, mime) = match bo_render::image::sniff(&data) {
        bo_render::image::Format::Png => ("png", "image/png"),
        bo_render::image::Format::Jpeg => ("jpeg", "image/jpeg"),
        bo_render::image::Format::Other => return None,
    };
    crate::edit::image_size(&data)?;
    Some(NewPicture { data, ext, mime })
}

/// The nodes of the items of a command (blank lines are dropped) and the pictures they use. An
/// item's `picture` is the relationship id of a picture the diagram has (`old`; kept), or a
/// `data:` URL of a new PNG / JPEG picture, which `store` adds to the file (returning its part
/// name); anything else is refused (`badPicture`).
pub fn nodes_of(
    items: &[SmartItem],
    old: &Pictures,
    store: &mut dyn FnMut(NewPicture) -> std::result::Result<String, String>,
) -> std::result::Result<(Vec<DiagramNode>, Pictures), String> {
    let kept: Vec<&SmartItem> = items.iter().filter(|i| !i.text.trim().is_empty()).collect();
    let lines: Vec<(String, usize)> = kept
        .iter()
        .map(|i| (i.text.trim().to_owned(), i.level))
        .collect();
    let mut pictures: Pictures = Vec::new();
    let mut ids = Vec::with_capacity(kept.len());
    let mut next = 1;
    for item in &kept {
        let id = match item.picture.as_deref() {
            None | Some("") => None,
            Some(url) if url.starts_with("data:") => {
                let pic = decode_picture(url).ok_or("badPicture")?;
                let name = store(pic)?;
                let id = loop {
                    let id = format!("rIdImg{next}");
                    next += 1;
                    if old.iter().chain(&pictures).all(|(i, _)| *i != id) {
                        break id;
                    }
                };
                pictures.push((id.clone(), name));
                Some(id)
            }
            Some(rid) => {
                let (id, name) = old.iter().find(|(i, _)| i == rid).ok_or("badPicture")?;
                if pictures.iter().all(|(i, _)| i != id) {
                    pictures.push((id.clone(), name.clone()));
                }
                Some(id.clone())
            }
        };
        ids.push(id);
    }
    let mut nodes = dl::nodes_from_outline(&lines);
    dl::set_outline_pictures(&mut nodes, &ids);
    Ok((nodes, pictures))
}

/// The pictures (of `all`) the nodes use.
#[must_use]
pub fn used_pictures(nodes: &[DiagramNode], all: &Pictures) -> Pictures {
    let used = dl::outline_pictures(nodes);
    all.iter()
        .filter(|(id, _)| used.iter().flatten().any(|u| u == id))
        .cloned()
        .collect()
}

/// The text pane's items of the nodes: one per node in reading order, with its picture.
#[must_use]
pub fn items_of(nodes: &[DiagramNode]) -> Vec<SmartItem> {
    dl::outline_of(nodes)
        .into_iter()
        .zip(dl::outline_pictures(nodes))
        .map(|((text, level), picture)| SmartItem {
            text,
            level,
            picture,
        })
        .collect()
}

/// The items with the text of node `node` replaced (editing a node on the canvas).
#[must_use]
pub fn with_node_text(nodes: &[DiagramNode], node: usize, text: &str) -> Option<Vec<SmartItem>> {
    let mut items = items_of(nodes);
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.is_empty() {
        return None;
    }
    items.get_mut(node)?.text = text;
    Some(items)
}

/// Sample nodes a layout's preview is drawn with (no text: the thumbnail is too small to read).
fn sample_nodes(layout: Layout) -> Vec<DiagramNode> {
    let leaf = DiagramNode::default;
    let with = |n: usize| DiagramNode {
        children: (0..n).map(|_| leaf()).collect(),
        ..DiagramNode::default()
    };
    match layout {
        Layout::Hierarchy | Layout::HierarchyHorizontal => vec![DiagramNode {
            children: vec![with(2), leaf(), with(1)],
            ..DiagramNode::default()
        }],
        Layout::ListVertical => vec![with(1), with(1), with(1)],
        Layout::CycleRadial | Layout::MatrixCentered | Layout::Cycle => {
            (0..5).map(|_| leaf()).collect()
        }
        Layout::Pyramid
        | Layout::PyramidInverted
        | Layout::Relationship
        | Layout::RelationshipTarget
        | Layout::PictureStrip => (0..3).map(|_| leaf()).collect(),
        _ => (0..4).map(|_| leaf()).collect(),
    }
}

/// Thumbnails of every layout of the library drawn with sample nodes in `look`, `w` × `h` points:
/// `[{ "layout": name, "svg": document }]` (the dialog's layout previews).
pub fn layout_previews(look: Look, w: f64, h: f64, fonts: &mut FontManager) -> serde_json::Value {
    let (w, h) = (w.clamp(40.0, 400.0), h.clamp(30.0, 300.0));
    let (we, he) = ((w * 12_700.0) as i64, (h * 12_700.0) as i64);
    // A margin of 4 % around the drawing.
    let (mx, my) = (we / 25, he / 25);
    let list: Vec<serde_json::Value> = Layout::ALL
        .iter()
        .map(|layout| {
            let xml = dl::drawing_xml(
                &sample_nodes(*layout),
                *layout,
                look,
                we - 2 * mx,
                he - 2 * my,
            );
            let cmds = bo_pptx::diagram::render_drawing(
                xml.as_bytes(),
                "/preview.xml",
                we - 2 * mx,
                he - 2 * my,
                fonts,
            )
            .unwrap_or_default();
            let mut all = vec![bo_render::Command::PushTransform {
                m: [
                    1.0,
                    0.0,
                    0.0,
                    1.0,
                    (mx as f64 / 12_700.0) as f32,
                    (my as f64 / 12_700.0) as f32,
                ],
            }];
            all.extend(cmds);
            all.push(bo_render::Command::PopTransform);
            serde_json::json!({
                "layout": layout.name(),
                "svg": bo_sheet::chart_style::commands_svg(&all, w, h),
            })
        })
        .collect();
    serde_json::Value::Array(list)
}

/// The node at point (`x`, `y`) EMU of a diagram's frame among `boxes` (the topmost shape that
/// shows a node's text, else the topmost shape of a node).
#[must_use]
pub fn node_at(boxes: &[dl::NodeBox], x: i64, y: i64) -> Option<dl::NodeBox> {
    let inside = |b: &&dl::NodeBox| {
        let (bx, by, bw, bh) = b.rect;
        x >= bx && x <= bx + bw && y >= by && y <= by + bh
    };
    boxes
        .iter()
        .rev()
        .filter(inside)
        .find(|b| b.text)
        .or_else(|| boxes.iter().rev().find(inside))
        .copied()
}

/// The box that shows the text of node `node` (the place of the on-canvas editor).
#[must_use]
pub fn text_box_of(boxes: &[dl::NodeBox], node: usize) -> Option<dl::NodeBox> {
    boxes
        .iter()
        .rev()
        .find(|b| b.node == node && b.text)
        .or_else(|| boxes.iter().rev().find(|b| b.node == node))
        .copied()
}

/// The shapes of a diagram that belong to a node: those of BlueOffice's own layout, or — for a
/// drawing part of another application — its shapes matched to the nodes
/// (`bo_pptx::diagram::diagram_build`).
#[must_use]
pub fn node_boxes(
    data: (&[u8], &str),
    layout_xml: Option<&[u8]>,
    drawing: Option<(&[u8], &str)>,
    (w, h): (i64, i64),
) -> Vec<dl::NodeBox> {
    let Some((nodes, lo)) = dl::parse_data_model(data.0, data.1) else {
        return Vec::new();
    };
    if let Some(layout) = dl::own_layout(lo.as_deref(), layout_xml) {
        return dl::node_boxes(&nodes, layout, w, h);
    }
    let Some((dx, dpart)) = drawing else {
        return dl::node_boxes(&nodes, dl::layout_of(lo.as_deref(), layout_xml), w, h);
    };
    let (Some(build), Some(shapes)) = (
        bo_pptx::diagram::diagram_build(data.0, data.1, dx, dpart),
        bo_pptx::diagram::drawing_shapes(dx, dpart),
    ) else {
        return Vec::new();
    };
    shapes
        .iter()
        .zip(&build.shapes)
        .filter_map(|(s, node)| {
            let ShapeNode::Shape(s) = s else { return None };
            let x = s.props.xfrm.as_ref()?;
            let (off, ext) = (x.off?, x.ext?);
            Some(dl::NodeBox {
                node: (*node)?,
                rect: (off.0, off.1, ext.0, ext.1),
                text: s.text.as_ref().is_some_and(|t| !t.text().trim().is_empty()),
            })
        })
        .collect()
}

/// The `xdr:grpSp` (ECMA-376 Part 1, §20.5.2.17) of a diagram drawing `drawing` laid out for
/// `child` EMU — "Convert to shapes" on a sheet: each `dsp:sp` becomes an `xdr:sp` (§20.5.2.29:
/// `nvSpPr`, `spPr`, `style`, `txBody`), nested groups stay groups; picture fills name their
/// image part (`resolve`: the image part of a relationship id of the drawing part).
#[must_use]
pub fn sheet_group_xml(
    drawing: &[u8],
    child: (i64, i64),
    resolve: bo_pptx::diagram::Resolve<'_>,
) -> Option<String> {
    use bo_model::slide::{XmlElement, XmlNode};
    fn renamed(el: &XmlElement, name: &str) -> String {
        let mut e = el.clone();
        e.name = name.to_owned();
        e.attrs
            .retain(|(k, _)| k != "xmlns" && !k.starts_with("xmlns:"));
        let mut s = String::new();
        bo_drawingml::dom::write_element(&e, &mut s);
        s
    }
    fn node(el: &XmlElement, n: &mut u32, out: &mut String) {
        use std::fmt::Write as _;
        *n += 1;
        let me = *n;
        let kid = |name: &str| el.elements().find(|c| c.local() == name);
        if el.local() == "grpSp" {
            let _ = write!(
                out,
                "<xdr:grpSp><xdr:nvGrpSpPr><xdr:cNvPr id=\"{me}\" name=\"Group {me}\"/><xdr:cNvGrpSpPr/></xdr:nvGrpSpPr>"
            );
            match kid("grpSpPr") {
                Some(p) => out.push_str(&renamed(p, "xdr:grpSpPr")),
                None => out.push_str("<xdr:grpSpPr/>"),
            }
            for c in el
                .elements()
                .filter(|c| matches!(c.local(), "sp" | "grpSp"))
            {
                node(c, n, out);
            }
            out.push_str("</xdr:grpSp>");
        } else {
            let _ = write!(
                out,
                "<xdr:sp macro=\"\" textlink=\"\"><xdr:nvSpPr><xdr:cNvPr id=\"{me}\" name=\"Shape {me}\"/><xdr:cNvSpPr/></xdr:nvSpPr>"
            );
            match kid("spPr") {
                Some(p) => out.push_str(&renamed(p, "xdr:spPr")),
                None => out.push_str("<xdr:spPr/>"),
            }
            if let Some(s) = kid("style") {
                out.push_str(&renamed(s, "xdr:style"));
            }
            if let Some(t) = kid("txBody") {
                out.push_str(&renamed(t, "xdr:txBody"));
            }
            out.push_str("</xdr:sp>");
        }
    }
    let tree = bo_pptx::diagram::group_tree(drawing, child, resolve)?;
    let root = bo_drawingml::dom::parse(&tree, "/group.xml").ok()?;
    let sp_tree = root.elements().find(|c| c.local() == "spTree")?;
    // The tree itself is the group: the same walk as a nested group.
    let mut as_group = sp_tree.clone();
    as_group.name = "dsp:grpSp".into();
    as_group.children.retain(|c| match c {
        XmlNode::Element(e) => e.local() != "nvGrpSpPr",
        XmlNode::Text(_) => false,
    });
    if !as_group
        .elements()
        .any(|c| matches!(c.local(), "sp" | "grpSp"))
    {
        return None;
    }
    let mut out = String::new();
    node(&as_group, &mut 0, &mut out);
    Some(out)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod shared_tests {
    use super::*;

    // A 1 × 1 PNG (ISO/IEC 15948: signature, IHDR, IDAT, IEND), base64.
    pub(crate) const PNG_1X1: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAIAAACQd1PeAAAADElEQVR42mPQK90GAAIuAVoZ84oiAAAAAElFTkSuQmCC";

    fn item(text: &str, level: usize, picture: Option<&str>) -> SmartItem {
        SmartItem {
            text: text.into(),
            level,
            picture: picture.map(str::to_owned),
        }
    }

    #[test]
    fn pictures_of_items_are_kept_added_and_refused() {
        let url = format!("data:image/png;base64,{PNG_1X1}");
        let pic = decode_picture(&url).unwrap();
        assert_eq!((pic.ext, pic.mime), ("png", "image/png"));
        assert!(decode_picture("data:text/plain;base64,QUJD").is_none());
        assert!(decode_picture("rId1").is_none());
        let old: Pictures = vec![
            ("rIdImg1".into(), "/m/image1.png".into()),
            ("rIdImg2".into(), "/m/image2.png".into()),
        ];
        let mut stored = 0;
        let mut store = |_p: NewPicture| {
            stored += 1;
            Ok(format!("/m/new{stored}.png"))
        };
        let items = [
            item("A", 0, Some("rIdImg2")),
            item("  ", 0, Some("rIdImg1")),
            item("B", 1, Some(&url)),
            item("C", 0, None),
        ];
        let (nodes, pictures) = nodes_of(&items, &old, &mut store).unwrap();
        // The blank line and its picture are gone; the new picture gets an id no old one has.
        assert_eq!(
            pictures,
            vec![
                ("rIdImg2".to_owned(), "/m/image2.png".to_owned()),
                ("rIdImg3".to_owned(), "/m/new1.png".to_owned()),
            ]
        );
        assert_eq!(nodes[0].picture.as_deref(), Some("rIdImg2"));
        assert_eq!(nodes[0].children[0].picture.as_deref(), Some("rIdImg3"));
        assert_eq!(nodes[1].picture, None);
        assert_eq!(items_of(&nodes)[1], item("B", 1, Some("rIdImg3")));
        assert_eq!(used_pictures(&nodes[1..], &pictures), Vec::new());
        // An id the diagram does not have, or a picture that is not PNG / JPEG.
        let bad = [item("A", 0, Some("rId77"))];
        assert_eq!(nodes_of(&bad, &old, &mut store).unwrap_err(), "badPicture");
        let bad = [item("A", 0, Some("data:image/png;base64,QUJD"))];
        assert_eq!(nodes_of(&bad, &old, &mut store).unwrap_err(), "badPicture");
        // Editing a node's text keeps the others and the pictures.
        let edited = with_node_text(&nodes, 1, " Mới \n lắm ").unwrap();
        assert_eq!(edited[1], item("Mới lắm", 1, Some("rIdImg3")));
        assert!(with_node_text(&nodes, 9, "x").is_none());
        assert!(with_node_text(&nodes, 0, "  ").is_none());
    }

    #[test]
    fn previews_and_node_hits() {
        let mut fonts = FontManager::new();
        let v = layout_previews(Look::default(), 96.0, 64.0, &mut fonts);
        let list = v.as_array().unwrap();
        assert_eq!(list.len(), Layout::ALL.len());
        for (p, layout) in list.iter().zip(Layout::ALL) {
            assert_eq!(p["layout"], layout.name());
            let svg = p["svg"].as_str().unwrap();
            assert!(svg.starts_with("<svg ") && svg.contains("viewBox=\"0 0 96 64\""));
            assert!(svg.contains("<path "), "{layout:?} draws shapes");
        }
        // The previews differ by layout and follow the colours.
        assert_ne!(list[0]["svg"], list[1]["svg"]);
        let gray = Look {
            palette: dl::Palette::Gray,
            ..Look::default()
        };
        assert_ne!(layout_previews(gray, 96.0, 64.0, &mut fonts), v);
        // Hit-testing a process of three steps in a 6 000 000 × 3 000 000 frame.
        let lines: Vec<(String, usize)> = ["A", "B", "C"]
            .iter()
            .map(|t| ((*t).to_owned(), 0))
            .collect();
        let nodes = dl::nodes_from_outline(&lines);
        let boxes = dl::node_boxes(&nodes, Layout::Process, 6_000_000, 3_000_000);
        let b = text_box_of(&boxes, 1).unwrap();
        let (cx, cy) = (b.rect.0 + b.rect.2 / 2, b.rect.1 + b.rect.3 / 2);
        assert_eq!(node_at(&boxes, cx, cy).map(|n| n.node), Some(1));
        assert_eq!(node_at(&boxes, cx, 10).map(|n| n.node), None);
        // The same through the parts BlueOffice writes.
        let p = dl::build_parts(
            &nodes,
            Layout::Process,
            Look::default(),
            (6_000_000, 3_000_000),
            "rId5",
        );
        let via = node_boxes(
            (&p.data, "/d.xml"),
            Some(&p.layout),
            Some((&p.drawing, "/w.xml")),
            (6_000_000, 3_000_000),
        );
        assert_eq!(via, boxes);
    }
}
