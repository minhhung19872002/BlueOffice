//! SmartArt in BlueSlide (`docs/parity/BACKLOG.md` CORE-SMART-003/004): insert a diagram from the
//! BlueOffice layout library, read and change its text outline and layout (the text pane), and convert it
//! to ordinary shapes. A diagram is written as the parts of ECMA-376 Part 1, §21.4 (data model, layout
//! definition, quick style, colours) plus the drawing part ([MS-ODRAWXML] `dsp:drawing`) laid out by
//! `bo_pptx::diagram_layout`, so other applications show the same picture.

use super::build;
use super::chart::put_part;
use super::geom::{find, place_slide};
use super::ops::{node_at, Tx};
use super::{insert_top, next_shape_id, slide_size, EditError, SlideSel, R};
use bo_model::slide::{
    GraphicContent, GroupShape, NonVisual, Presentation, Rel, ShapeNode, ShapeProps, Xfrm,
};
use bo_pptx::diagram_layout::{self as dl, Category, Fill, Look, Palette};
use serde::{Deserialize, Serialize};

const CT: &str = "application/vnd.openxmlformats-officedocument.drawingml.";
const CT_DRAWING: &str = "application/vnd.ms-office.drawingml.diagramDrawing+xml";
const REL_DRAWING: &str = "http://schemas.microsoft.com/office/2007/relationships/diagramDrawing";
const NS_DGM: &str = "http://schemas.openxmlformats.org/drawingml/2006/diagram";
const NS_R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";

/// One line of the text pane: the text of a node and its level (0 = top).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SmartItem {
    /// Node text.
    pub text: String,
    /// Outline level.
    #[serde(default)]
    pub level: usize,
}

/// The selected SmartArt as the text pane shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SmartArtOut {
    /// Shape id of the graphic frame.
    pub id: u32,
    /// Layout category (`list`, `process`, `cycle`, `hierarchy`, `matrix`, `pyramid`, `relationship`).
    pub layout: String,
    /// Text outline.
    pub items: Vec<SmartItem>,
    /// Colours: `colorful` | `blue` | `green` | `orange` | `purple` | `gray`.
    pub colors: String,
    /// Look: `fill` | `outline`.
    pub style: String,
}

/// Colours / look of a command (`None` = keep `base`).
fn look(base: Look, colors: Option<&str>, style: Option<&str>) -> R<Look> {
    Ok(Look {
        palette: match colors {
            Some(c) => Palette::parse(c).ok_or(EditError::Refused("badSmartArt"))?,
            None => base.palette,
        },
        fill: match style {
            Some(s) => Fill::parse(s).ok_or(EditError::Refused("badSmartArt"))?,
            None => base.fill,
        },
    })
}

/// Is this node a SmartArt frame?
pub fn is_smartart(n: &ShapeNode) -> bool {
    matches!(n, ShapeNode::Frame(f) if f.uri == bo_pptx::diagram::URI_DIAGRAM)
}

fn outline(items: &[SmartItem]) -> Vec<(String, usize)> {
    items
        .iter()
        .filter(|i| !i.text.trim().is_empty())
        .map(|i| (i.text.trim().to_owned(), i.level))
        .collect()
}

/// Part names of a slide's SmartArt frame: (data, layout, drawing — if any) and the frame size (EMU).
struct Parts {
    data: String,
    layout: Option<String>,
    drawing: Option<String>,
    ext: (i64, i64),
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
    let drawing = pres
        .part_data(&data)
        .and_then(|d| bo_pptx::diagram::drawing_rel_id(d, &data))
        .and_then(|rid| target(&rid));
    Some(Parts {
        data,
        layout,
        drawing,
        ext: f.xfrm.ext.unwrap_or((0, 0)),
    })
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
        layout: dl::category(lo.as_deref(), layout).name().to_owned(),
        items: dl::outline_of(&nodes)
            .into_iter()
            .map(|(text, level)| SmartItem { text, level })
            .collect(),
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

/// Insert a SmartArt on the current slide (`layout`: a category name; `items`: the text outline).
pub fn cmd_insert_smartart(
    tx: &mut Tx<'_>,
    sel: &mut SlideSel,
    layout: &str,
    items: &[SmartItem],
    (colors, style): (Option<&str>, Option<&str>),
    rect: Option<super::geom::Rect>,
) -> R<()> {
    let cat = Category::parse(layout).ok_or(EditError::Refused("badSmartArt"))?;
    let lk = look(Look::default(), colors, style)?;
    let nodes = dl::nodes_from_outline(&outline(items));
    if nodes.is_empty() {
        return Err(EditError::Refused("badSmartArt"));
    }
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
    let p = dl::build_parts(&nodes, cat, lk, (cx, cy), &rids[4]);
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
            r#"<p:graphicFrame><p:nvGraphicFramePr><p:cNvPr id="{id}" name="SmartArt {id}"/><p:cNvGraphicFramePr/><p:nvPr/></p:nvGraphicFramePr>"#,
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
    let old_cat = dl::category(
        lo.as_deref(),
        parts.layout.as_deref().and_then(|l| tx.pres.part_data(l)),
    );
    let cat = match layout {
        Some(l) => Category::parse(l).ok_or(EditError::Refused("badSmartArt"))?,
        None => old_cat,
    };
    let nodes = match items {
        Some(i) => dl::nodes_from_outline(&outline(i)),
        None => old_nodes,
    };
    if nodes.is_empty() {
        return Err(EditError::Refused("badSmartArt"));
    }
    // The drawing relationship keeps its id; without a drawing part the data model names none.
    let drawing_rel = tx
        .pres
        .part_data(&parts.data)
        .and_then(|d| bo_pptx::diagram::drawing_rel_id(d, &parts.data))
        .unwrap_or_default();
    let lk = look(dl::look_of(data, &parts.data), colors, style)?;
    let built = dl::build_parts(&nodes, cat, lk, parts.ext, &drawing_rel);
    put_part(
        tx,
        &parts.data,
        Some(&format!("{CT}diagramData+xml")),
        built.data,
    )?;
    if let Some(l) = &parts.layout {
        if layout.is_some() {
            put_part(tx, l, Some(&format!("{CT}diagramLayout+xml")), built.layout)?;
        }
    }
    if let Some(d) = &parts.drawing {
        put_part(tx, d, Some(CT_DRAWING), built.drawing)?;
    }
    Ok(())
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
    let parts = parts_of(tx.pres, sel.slide, n).ok_or(EditError::Refused("noSmartArt"))?;
    let (w, h) = parts.ext;
    let shapes = parts
        .drawing
        .as_deref()
        .and_then(|d| bo_pptx::diagram::drawing_shapes(tx.pres.part_data(d)?, d))
        .or_else(|| {
            let data = tx.pres.part_data(&parts.data)?;
            let layout = parts.layout.as_deref().and_then(|l| tx.pres.part_data(l));
            dl::synth(data, &parts.data, layout, w, h)
        })
        .ok_or(EditError::Refused("noSmartArt"))?;
    let mut next = next_shape_id(tx.pres, sel.slide);
    let mut children = shapes;
    for c in &mut children {
        renumber(c, &mut next);
    }
    let group = ShapeNode::Group(Box::new(GroupShape {
        nv: NonVisual {
            id,
            name: f.nv.name.clone(),
            ..NonVisual::default()
        },
        props: ShapeProps {
            xfrm: Some(Xfrm {
                off: f.xfrm.off,
                ext: f.xfrm.ext,
                ch_off: Some((0, 0)),
                ch_ext: f.xfrm.ext,
                rot: f.xfrm.rot,
                flip_h: f.xfrm.flip_h,
                flip_v: f.xfrm.flip_v,
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
