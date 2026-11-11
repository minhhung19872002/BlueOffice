//! SmartArt in BlueSlide (`docs/parity/BACKLOG.md` CORE-SMART-003/004): insert a diagram from the
//! BlueOffice layout library, read and change its text outline and layout (the text pane), and convert it
//! to ordinary shapes. A diagram is written as the parts of ECMA-376 Part 1, §21.4 (data model, layout
//! definition, quick style, colours) plus the drawing part ([MS-ODRAWXML] `dsp:drawing`) laid out by
//! `bo_pptx::diagram_layout`, so other applications show the same picture.
//!
//! A node may have a picture (shown by the picture layouts): a picture fill (§20.1.8.14 blipFill) in
//! the point's `dgm:spPr` and in its shape of the drawing part, whose `a:blip@r:embed` names a
//! relationship of the data model part and of the drawing part to the image part. A diagram of
//! another application keeps its own quick style and colours parts (`colors` / `style` = `file`)
//! and is laid out with them until BlueOffice ones are chosen.

use super::build;
use super::chart::put_part;
use super::geom::{find, place_slide};
use super::ops::{node_at, SlideOp, Tx};
use super::{insert_top, next_shape_id, slide_size, EditError, SlideSel, R};
use crate::smartart::{self as shared, Pictures};
use bo_model::slide::{
    Fill as ShapeFill, GraphicContent, GroupShape, NonVisual, Presentation, Rel, ShapeNode,
    ShapeProps, Xfrm,
};
use bo_pptx::diagram_layout::{self as dl, Fill, Layout, Look, Palette};
use serde::{Deserialize, Serialize};

const CT: &str = "application/vnd.openxmlformats-officedocument.drawingml.";
const CT_DRAWING: &str = "application/vnd.ms-office.drawingml.diagramDrawing+xml";
const REL_DRAWING: &str = "http://schemas.microsoft.com/office/2007/relationships/diagramDrawing";
const NS_DGM: &str = "http://schemas.openxmlformats.org/drawingml/2006/diagram";
const NS_R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";

/// One line of the text pane: the text of a node and its level (0 = top).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SmartItem {
    /// Node text.
    pub text: String,
    /// Outline level.
    #[serde(default)]
    pub level: usize,
    /// The node's picture. Read: the id of the picture the node has. In a command: that id (the
    /// picture is kept), a `data:image/…;base64,…` URL of a PNG or JPEG picture (a new picture),
    /// or nothing (no picture).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub picture: Option<String>,
}

/// The selected SmartArt as the text pane shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SmartArtOut {
    /// Shape id of the graphic frame.
    pub id: u32,
    /// Layout of the BlueOffice library (`bo_pptx::diagram_layout::Layout` name: `list`,
    /// `processChevron`, …); a diagram of another producer reports the first layout of its category.
    pub layout: String,
    /// Text outline.
    pub items: Vec<SmartItem>,
    /// Colours: `colorful` | `blue` | `green` | `orange` | `purple` | `gray`, or `file`: the
    /// diagram's own colour definition part.
    pub colors: String,
    /// Look: `fill` | `outline`, or `file`: the diagram's own quick style part.
    pub style: String,
}

/// A node of a SmartArt under a point of the slide (editing its text on the canvas).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SmartNodeOut {
    /// Shape id of the graphic frame.
    pub id: u32,
    /// Index of the node in the text outline.
    pub node: usize,
    /// The node's text.
    pub text: String,
    /// Box of the shape showing the text, in slide points.
    pub x: f64,
    /// Top.
    pub y: f64,
    /// Width.
    pub w: f64,
    /// Height.
    pub h: f64,
}

/// Colours / look of a command (`None` = keep `base`).
pub(crate) fn look(
    base: Look,
    colors: Option<&str>,
    style: Option<&str>,
) -> std::result::Result<Look, &'static str> {
    Ok(Look {
        palette: match colors {
            Some(c) => Palette::parse(c).ok_or("badSmartArt")?,
            None => base.palette,
        },
        fill: match style {
            Some(s) => Fill::parse(s).ok_or("badSmartArt")?,
            None => base.fill,
        },
    })
}

/// An error code of the shared SmartArt helpers as a refusal.
fn refusal(code: &str) -> EditError {
    EditError::Refused(if code == "badPicture" {
        "badPicture"
    } else {
        "badSmartArt"
    })
}

/// Is this node a SmartArt frame?
pub fn is_smartart(n: &ShapeNode) -> bool {
    matches!(n, ShapeNode::Frame(f) if f.uri == bo_pptx::diagram::URI_DIAGRAM)
}

/// Part names of a slide's SmartArt frame: (data, layout, quick style, colours, drawing — if any),
/// the frame size (EMU) and the node pictures (relationships of the data model part).
struct Parts {
    data: String,
    layout: Option<String>,
    style: Option<String>,
    colors: Option<String>,
    drawing: Option<String>,
    ext: (i64, i64),
    pictures: Pictures,
}

/// Image relationships (id, part name) of diagram part `part`.
fn pictures_of(pres: &Presentation, part: &str) -> Pictures {
    pres.part_data(&bo_opc::rels_part_name(part))
        .map(|x| bo_pptx::diagram::image_rels(x, part))
        .unwrap_or_default()
}

fn parts_of(pres: &Presentation, slide: usize, n: &ShapeNode) -> Option<Parts> {
    let ShapeNode::Frame(f) = n else { return None };
    if f.uri != bo_pptx::diagram::URI_DIAGRAM {
        return None;
    }
    let GraphicContent::Other(gd) = &f.content else {
        return None;
    };
    let s = pres.slides.get(slide)?;
    let target = |rid: &str| {
        s.info
            .rels
            .iter()
            .find(|r| r.id == rid && !r.external)
            .map(|r| bo_opc::resolve_target(&s.info.part_name, &r.target))
    };
    let data = target(bo_pptx::diagram::data_rel_id(gd)?)?;
    let layout = bo_pptx::diagram::layout_rel_id(gd).and_then(target);
    // `dgm:relIds@r:qs` / `@r:cs` (§21.4.2.6): the quick style and colours parts.
    let rel_ids = gd.elements().find(|c| c.local() == "relIds");
    let style = rel_ids.and_then(|c| c.attr("r:qs")).and_then(target);
    let colors = rel_ids.and_then(|c| c.attr("r:cs")).and_then(target);
    let drawing = pres
        .part_data(&data)
        .and_then(|d| bo_pptx::diagram::drawing_rel_id(d, &data))
        .and_then(|rid| target(&rid));
    Some(Parts {
        pictures: pictures_of(pres, &data),
        data,
        layout,
        style,
        colors,
        drawing,
        ext: f.xfrm.ext.unwrap_or((0, 0)),
    })
}

impl Parts {
    /// The diagram's own quick style and colours.
    fn defs(&self, pres: &Presentation) -> dl::Defs {
        let get = |n: &Option<String>| n.as_deref().and_then(|n| pres.part_data(n));
        dl::Defs::parse(get(&self.style), get(&self.colors))
    }
}

/// The SmartArt of shape `id` on a slide.
pub fn smartart_out(pres: &Presentation, slide: usize, id: u32) -> Option<SmartArtOut> {
    let placed = place_slide(pres, slide);
    let p = find(&placed, id)?;
    let n = node_at(pres, slide, &p.path)?;
    let parts = parts_of(pres, slide, n)?;
    let (nodes, lo) = dl::parse_data_model(pres.part_data(&parts.data)?, &parts.data)?;
    let layout = parts.layout.as_deref().and_then(|l| pres.part_data(l));
    let lk = dl::look_of(pres.part_data(&parts.data)?, &parts.data);
    Some(SmartArtOut {
        colors: lk.palette.name().to_owned(),
        style: lk.fill.name().to_owned(),
        id,
        layout: dl::layout_of(lo.as_deref(), layout).name().to_owned(),
        items: shared::items_of(&nodes),
    })
}

fn rel_ids(s: &bo_model::slide::Slide, n: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut k = 1;
    while out.len() < n {
        let id = format!("rId{k}");
        if s.info.rels.iter().all(|r| r.id != id) {
            out.push(id);
        }
        k += 1;
    }
    out
}

/// Remove a preserved part, if it is there.
fn remove_part(tx: &mut Tx<'_>, name: &str) -> R<()> {
    if let Some(i) = tx
        .pres
        .parts
        .iter()
        .position(|p| p.name.eq_ignore_ascii_case(name))
    {
        let old = tx.pres.parts[i].clone();
        tx.op(SlideOp::Parts {
            index: i,
            remove: vec![old],
            insert: vec![],
        })?;
    }
    Ok(())
}

/// Write the image relationships of the data model part and the drawing part of a diagram (one
/// list for both; no relationships part when there is no picture), then drop the image parts
/// (`before`) that nothing names any more.
fn put_pictures(tx: &mut Tx<'_>, parts: &[&str], pictures: &Pictures, before: &Pictures) -> R<()> {
    for part in parts {
        let name = bo_opc::rels_part_name(part);
        if pictures.is_empty() {
            remove_part(tx, &name)?;
        } else {
            put_part(
                tx,
                &name,
                None,
                bo_pptx::diagram::image_rels_xml(part, pictures),
            )?;
        }
    }
    for (_, image) in before {
        if pictures.iter().all(|(_, n)| n != image) {
            super::picture::drop_if_unused(tx, image)?;
        }
    }
    Ok(())
}

/// The nodes of a command's items, new pictures stored as image parts of the presentation.
fn nodes_of(
    tx: &mut Tx<'_>,
    items: &[SmartItem],
    old: &Pictures,
) -> R<(Vec<dl::DiagramNode>, Pictures)> {
    let mut store = |pic: shared::NewPicture| {
        super::picture::add_part(tx, pic.data, pic.ext, pic.mime).map_err(|e| e.to_string())
    };
    shared::nodes_of(items, old, &mut store).map_err(|e| refusal(&e))
}

/// Insert a SmartArt on the current slide (`layout`: a layout name; `items`: the text outline).
pub fn cmd_insert_smartart(
    tx: &mut Tx<'_>,
    sel: &mut SlideSel,
    layout: &str,
    items: &[SmartItem],
    (colors, style): (Option<&str>, Option<&str>),
    rect: Option<super::geom::Rect>,
) -> R<()> {
    let cat = Layout::parse(layout).ok_or(EditError::Refused("badSmartArt"))?;
    let lk = look(Look::default(), colors, style).map_err(EditError::Refused)?;
    if items.iter().all(|i| i.text.trim().is_empty()) {
        return Err(EditError::Refused("badSmartArt"));
    }
    let (nodes, pictures) = nodes_of(tx, items, &Vec::new())?;
    let slide = sel.slide;
    let (sw, sh) = slide_size(tx.pres);
    let r = rect.unwrap_or(super::geom::Rect {
        x: sw * 0.1,
        y: sh * 0.2,
        w: sw * 0.8,
        h: sh * 0.65,
    });
    let emu = |pt: f64| (pt * super::geom::EMU_PT).round() as i64;
    let (cx, cy) = (emu(r.w.max(1.0)), emu(r.h.max(1.0)));
    // Five new parts and their relationships from the slide.
    let mut names: Vec<String> = Vec::new();
    for prefix in ["data", "layout", "quickStyle", "colors", "drawing"] {
        let n =
            build::unique_part_except(tx.pres, &format!("/ppt/diagrams/{prefix}"), "xml", &names);
        names.push(n);
    }
    let s = tx
        .pres
        .slides
        .get(slide)
        .ok_or(EditError::Refused("noSlide"))?;
    let rids = rel_ids(s, 5);
    let p = {
        let pres = &*tx.pres;
        let size = |rid: &str| picture_size(pres, &pictures, rid);
        let styling = dl::Styling {
            look: lk,
            defs: None,
            picture_size: Some(&size),
        };
        dl::build_parts_with(&nodes, cat, &styling, (cx, cy), &rids[4])
    };
    let types = [
        (format!("{CT}diagramData+xml"), p.data),
        (format!("{CT}diagramLayout+xml"), p.layout),
        (format!("{CT}diagramStyle+xml"), p.style),
        (format!("{CT}diagramColors+xml"), p.colors),
        (CT_DRAWING.to_owned(), p.drawing),
    ];
    for (name, (ct, bytes)) in names.iter().zip(types) {
        put_part(tx, name, Some(&ct), bytes)?;
    }
    put_pictures(tx, &[&names[0], &names[4]], &pictures, &Vec::new())?;
    let rel_types = [
        format!("{}diagramData", bo_pptx::RELATIONSHIP_NS),
        format!("{}diagramLayout", bo_pptx::RELATIONSHIP_NS),
        format!("{}diagramQuickStyle", bo_pptx::RELATIONSHIP_NS),
        format!("{}diagramColors", bo_pptx::RELATIONSHIP_NS),
        REL_DRAWING.to_owned(),
    ];
    tx.fields(slide, |f| {
        for ((id, ty), name) in rids.iter().zip(&rel_types).zip(&names) {
            f.rels.push(Rel {
                id: id.clone(),
                rel_type: ty.clone(),
                target: format!(
                    "../diagrams/{}",
                    name.rsplit('/').next().unwrap_or_default()
                ),
                external: false,
            });
        }
    })?;
    let id = next_shape_id(tx.pres, slide);
    let xml = format!(
        concat!(
            r#"<p:graphicFrame><p:nvGraphicFramePr><p:cNvPr id="{id}" name="Diagram {id}"/><p:cNvGraphicFramePr/><p:nvPr/></p:nvGraphicFramePr>"#,
            r#"<p:xfrm><a:off x="{x}" y="{y}"/><a:ext cx="{cx}" cy="{cy}"/></p:xfrm>"#,
            r#"<a:graphic><a:graphicData uri="{uri}"><dgm:relIds xmlns:dgm="{dgm}" xmlns:r="{r}" r:dm="{dm}" r:lo="{lo}" r:qs="{qs}" r:cs="{cs}"/></a:graphicData></a:graphic></p:graphicFrame>"#
        ),
        id = id,
        x = emu(r.x),
        y = emu(r.y),
        cx = cx,
        cy = cy,
        uri = bo_pptx::diagram::URI_DIAGRAM,
        dgm = NS_DGM,
        r = NS_R,
        dm = rids[0],
        lo = rids[1],
        qs = rids[2],
        cs = rids[3],
    );
    insert_top(tx, sel, build::node(&xml))
}

/// Pixel size of the picture behind relationship `rid` of a diagram.
fn picture_size(pres: &Presentation, pictures: &Pictures, rid: &str) -> Option<(u32, u32)> {
    let (_, name) = pictures.iter().find(|(id, _)| id == rid)?;
    crate::edit::image_size(pres.part_data(name)?)
}

fn frame_id(sel: &SlideSel, id: Option<u32>) -> R<u32> {
    match (id, sel.ids.as_slice()) {
        (Some(i), _) => Ok(i),
        (None, [i]) => Ok(*i),
        _ => Err(EditError::Refused("noSmartArt")),
    }
}

/// Change the text outline and/or the layout of SmartArt `id` (default: the selected one); the data
/// model, layout definition and drawing part are rewritten.
pub fn cmd_set_smartart(
    tx: &mut Tx<'_>,
    sel: &SlideSel,
    id: Option<u32>,
    layout: Option<&str>,
    items: Option<&[SmartItem]>,
    (colors, style): (Option<&str>, Option<&str>),
) -> R<()> {
    let id = frame_id(sel, id)?;
    let placed = place_slide(tx.pres, sel.slide);
    let p = find(&placed, id).ok_or(EditError::Refused("noSmartArt"))?;
    let n = node_at(tx.pres, sel.slide, &p.path).ok_or(EditError::Refused("noSmartArt"))?;
    let parts = parts_of(tx.pres, sel.slide, n).ok_or(EditError::Refused("noSmartArt"))?;
    let data = tx
        .pres
        .part_data(&parts.data)
        .ok_or(EditError::Refused("noSmartArt"))?;
    let (old_nodes, lo) =
        dl::parse_data_model(data, &parts.data).ok_or(EditError::Refused("noSmartArt"))?;
    let old_cat = dl::layout_of(
        lo.as_deref(),
        parts.layout.as_deref().and_then(|l| tx.pres.part_data(l)),
    );
    let cat = match layout {
        Some(l) => Layout::parse(l).ok_or(EditError::Refused("badSmartArt"))?,
        None => old_cat,
    };
    // The drawing relationship keeps its id; without a drawing part the data model names none.
    let drawing_rel = bo_pptx::diagram::drawing_rel_id(data, &parts.data).unwrap_or_default();
    let lk = look(dl::look_of(data, &parts.data), colors, style).map_err(EditError::Refused)?;
    let defs = parts.defs(tx.pres);
    let (nodes, pictures) = if let Some(i) = items {
        nodes_of(tx, i, &parts.pictures)?
    } else {
        let used = shared::used_pictures(&old_nodes, &parts.pictures);
        (old_nodes, used)
    };
    if nodes.is_empty() {
        return Err(EditError::Refused("badSmartArt"));
    }
    let built = {
        let pres = &*tx.pres;
        let size = |rid: &str| picture_size(pres, &pictures, rid);
        let styling = dl::Styling {
            look: lk,
            defs: Some(&defs),
            picture_size: Some(&size),
        };
        dl::build_parts_with(&nodes, cat, &styling, parts.ext, &drawing_rel)
    };
    put_part(
        tx,
        &parts.data,
        Some(&format!("{CT}diagramData+xml")),
        built.data,
    )?;
    // The definition parts name the same layout, quick style and colours as the data model's
    // document point (`prSet@loTypeId` / `@qsTypeId` / `@csTypeId`); unchanged parts stay as they are
    // (a diagram's own quick style and colours are kept while its look is `file`).
    for (name, ct, bytes) in [
        (&parts.layout, "diagramLayout+xml", built.layout),
        (&parts.style, "diagramStyle+xml", built.style),
        (&parts.colors, "diagramColors+xml", built.colors),
    ] {
        if let Some(n) = name {
            put_part(tx, n, Some(&format!("{CT}{ct}")), bytes)?;
        }
    }
    let mut with_rels = vec![parts.data.as_str()];
    if let Some(d) = &parts.drawing {
        put_part(tx, d, Some(CT_DRAWING), built.drawing)?;
        with_rels.push(d.as_str());
    }
    put_pictures(tx, &with_rels, &pictures, &parts.pictures)
}

/// Change the text of node `node` (index in the text outline) of SmartArt `id`: the edit of a node
/// on the canvas, one undo step.
pub fn cmd_set_smartart_node_text(
    tx: &mut Tx<'_>,
    sel: &SlideSel,
    id: Option<u32>,
    node: usize,
    text: &str,
) -> R<()> {
    let id = frame_id(sel, id)?;
    let mut items = smartart_out(tx.pres, sel.slide, id)
        .ok_or(EditError::Refused("noSmartArt"))?
        .items;
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let item = items
        .get_mut(node)
        .ok_or(EditError::Refused("badSmartArt"))?;
    if text.is_empty() {
        return Err(EditError::Refused("badSmartArt"));
    }
    item.text = text;
    cmd_set_smartart(tx, sel, Some(id), None, Some(&items), (None, None))
}

/// The node of SmartArt `id` (default: the selected one) at slide point `at`, or — with `node` —
/// that node: its text and the box showing it.
pub fn smartart_node_at(
    pres: &Presentation,
    sel: &SlideSel,
    id: Option<u32>,
    at: Option<(f64, f64)>,
    node: Option<usize>,
) -> Option<SmartNodeOut> {
    let id = frame_id(sel, id).ok()?;
    let placed = place_slide(pres, sel.slide);
    let p = find(&placed, id)?;
    let n = node_at(pres, sel.slide, &p.path)?;
    let parts = parts_of(pres, sel.slide, n)?;
    let (ew, eh) = parts.ext;
    if ew <= 0 || eh <= 0 || p.b.w <= 0.0 || p.b.h <= 0.0 {
        return None;
    }
    let data = pres.part_data(&parts.data)?;
    let boxes = shared::node_boxes(
        (data, &parts.data),
        parts.layout.as_deref().and_then(|l| pres.part_data(l)),
        parts
            .drawing
            .as_deref()
            .and_then(|d| Some((pres.part_data(d)?, d))),
        parts.ext,
    );
    let (left, top) = (p.b.cx - p.b.w / 2.0, p.b.cy - p.b.h / 2.0);
    let hit = match (node, at) {
        (Some(k), _) => shared::text_box_of(&boxes, k)?,
        (None, Some((x, y))) => {
            // Into the frame's own axes (a turned frame), then to EMU of the drawing.
            let (dx, dy) = (x - p.b.cx, y - p.b.cy);
            let a = -p.b.rot.to_radians();
            let (lx, ly) = (
                dx * a.cos() - dy * a.sin() + p.b.w / 2.0,
                dx * a.sin() + dy * a.cos() + p.b.h / 2.0,
            );
            shared::node_at(
                &boxes,
                (lx / p.b.w * ew as f64) as i64,
                (ly / p.b.h * eh as f64) as i64,
            )?
        }
        (None, None) => return None,
    };
    let (nodes, _) = dl::parse_data_model(data, &parts.data)?;
    let text = dl::outline_of(&nodes).get(hit.node)?.0.clone();
    let (bx, by, bw, bh) = hit.rect;
    Some(SmartNodeOut {
        id,
        node: hit.node,
        text,
        x: left + bx as f64 / ew as f64 * p.b.w,
        y: top + by as f64 / eh as f64 * p.b.h,
        w: bw as f64 / ew as f64 * p.b.w,
        h: bh as f64 / eh as f64 * p.b.h,
    })
}

/// Replace SmartArt `id` (default: the selected one) by a group of ordinary shapes drawn as it shows.
pub fn cmd_smartart_to_shapes(tx: &mut Tx<'_>, sel: &mut SlideSel, id: Option<u32>) -> R<()> {
    let id = frame_id(sel, id)?;
    let placed = place_slide(tx.pres, sel.slide);
    let p = find(&placed, id).ok_or(EditError::Refused("noSmartArt"))?;
    let n = node_at(tx.pres, sel.slide, &p.path).ok_or(EditError::Refused("noSmartArt"))?;
    let ShapeNode::Frame(f) = n else {
        return Err(EditError::Refused("noSmartArt"));
    };
    let (name, xfrm) = (f.nv.name.clone(), f.xfrm.clone());
    let parts = parts_of(tx.pres, sel.slide, n).ok_or(EditError::Refused("noSmartArt"))?;
    let (w, h) = parts.ext;
    let resolve_in = |rels: &Pictures, rid: &str| {
        rels.iter()
            .find(|(i, _)| i == rid)
            .map(|(_, name)| name.clone())
    };
    let shapes = parts
        .drawing
        .as_deref()
        .and_then(|d| {
            let rels = pictures_of(tx.pres, d);
            bo_pptx::diagram::drawing_shapes_with(
                tx.pres.part_data(d)?,
                d,
                Some(&|rid: &str| resolve_in(&rels, rid)),
            )
        })
        .or_else(|| {
            let data = tx.pres.part_data(&parts.data)?;
            let layout = parts.layout.as_deref().and_then(|l| tx.pres.part_data(l));
            let defs = parts.defs(tx.pres);
            dl::synth_with(
                data,
                &parts.data,
                layout,
                Some(&defs),
                Some(&|rid: &str| resolve_in(&parts.pictures, rid)),
                w,
                h,
            )
        })
        .ok_or(EditError::Refused("noSmartArt"))?;
    let mut next = next_shape_id(tx.pres, sel.slide);
    let mut children = shapes;
    // Pictures of the diagram parts become pictures of the slide.
    let mut slide_rels: Vec<(String, String)> = Vec::new();
    for c in &mut children {
        renumber(c, &mut next);
        relink(tx, sel.slide, c, &mut slide_rels)?;
    }
    let group = ShapeNode::Group(Box::new(GroupShape {
        nv: NonVisual {
            id,
            name,
            ..NonVisual::default()
        },
        props: ShapeProps {
            xfrm: Some(Xfrm {
                off: xfrm.off,
                ext: xfrm.ext,
                ch_off: Some((0, 0)),
                ch_ext: xfrm.ext,
                rot: xfrm.rot,
                flip_h: xfrm.flip_h,
                flip_v: xfrm.flip_v,
            }),
            ..ShapeProps::default()
        },
        children,
        extra: Vec::new(),
    }));
    tx.replace_node(sel.slide, &p.path, group)
        .map_err(|_| EditError::Refused("noSmartArt"))?;
    sel.ids = vec![id];
    Ok(())
}

/// Picture fills naming an image part (`/…`, as `bo_pptx::diagram::drawing_shapes_with` leaves
/// them) get a relationship of the slide (`done`: part name → relationship id).
fn relink(
    tx: &mut Tx<'_>,
    slide: usize,
    n: &mut ShapeNode,
    done: &mut Vec<(String, String)>,
) -> R<()> {
    fn link(
        tx: &mut Tx<'_>,
        slide: usize,
        embed: &mut Option<String>,
        done: &mut Vec<(String, String)>,
    ) -> R<()> {
        let Some(part) = embed.clone().filter(|e| e.starts_with('/')) else {
            // An id that named nothing: no picture.
            if embed.as_deref() == Some("") {
                *embed = None;
            }
            return Ok(());
        };
        let rid = if let Some((_, rid)) = done.iter().find(|(p, _)| *p == part) {
            rid.clone()
        } else {
            let rid = super::picture::add_rel(tx, slide, &part)?;
            done.push((part, rid.clone()));
            rid
        };
        *embed = Some(rid);
        Ok(())
    }
    match n {
        ShapeNode::Shape(s) => {
            if let Some(ShapeFill::Picture(b)) = &mut s.props.fill {
                link(tx, slide, &mut b.embed, done)?;
            }
        }
        ShapeNode::Picture(p) => link(tx, slide, &mut p.blip.embed, done)?,
        ShapeNode::Group(g) => {
            for c in &mut g.children {
                relink(tx, slide, c, done)?;
            }
        }
        _ => {}
    }
    Ok(())
}

/// Give a node and its children fresh shape ids.
fn renumber(n: &mut ShapeNode, next: &mut u32) {
    let nv = match n {
        ShapeNode::Shape(s) => Some(&mut s.nv),
        ShapeNode::Picture(p) => Some(&mut p.nv),
        ShapeNode::Connector(c) => Some(&mut c.nv),
        ShapeNode::Frame(f) => Some(&mut f.nv),
        ShapeNode::Group(g) => {
            for c in &mut g.children {
                renumber(c, next);
            }
            Some(&mut g.nv)
        }
        ShapeNode::Other(_) => None,
    };
    if let Some(nv) = nv {
        nv.id = *next;
        if nv.name.is_empty() {
            nv.name = format!("Shape {next}");
        }
        *next += 1;
    }
}
