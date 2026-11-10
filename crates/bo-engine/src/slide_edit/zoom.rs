//! Slide, section and summary zooms (SLIDE-ZOOM-001, [MS-PPTX] §2.8–§2.11; markup in
//! [`bo_pptx::zoom`]).
//!
//! - Insert a slide zoom (one per picked slide) or a section zoom (one per picked section)
//!   on the current slide.
//! - Insert a summary zoom: a new summary slide before the first picked slide, in a section
//!   of its own, with one zoom per picked slide's section (sections are created where a
//!   picked slide does not start one yet).
//! - Options of the selected zoom: return to the zoom, zoom transition and its duration,
//!   zoom background, a custom image or the slide preview.
//! - Thumbnails are rendered by the engine renderer; [`SlideEditor::zoom_sync`] renders the
//!   preview again, in the same undo step, after a command changed a target slide.
//! - In the slide show a zoom is a link ([`show_links`]): it plays the target slide or
//!   section and, with "return", comes back to the zoom's slide.

use super::geom::{find, place_slide, Placed};
use super::ops::{node_at, OpError, SlideOp, Tx};
use super::present::{LinkAction, LinkOut};
use super::{
    build, insert_top, next_shape_id, sections, slide_size, EditError, Outcome, SlideEditResult,
    SlideEditor, SlideSel, R,
};
use crate::{EngineError, Result};
use bo_model::slide::{Presentation, RawPart, Rel, ShapeNode};
use bo_pptx::zoom::{self as z, NewObject, PropsChange, ZoomKind, ZoomProps, ZoomTarget};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

const EMU_PT: f64 = 12_700.0;
/// Width of a rendered thumbnail (pixels).
const THUMB_PX: f32 = 480.0;

/// Names the summary zoom gives to what it creates (UI language, from the host).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SummaryNames {
    /// Title of the summary slide.
    #[serde(default)]
    pub title: String,
    /// Name of the summary slide's section.
    #[serde(default)]
    pub summary: String,
    /// Name of a new section when its first slide has no title; `{n}` = its number.
    #[serde(default)]
    pub section: String,
    /// Name of the section of the slides before the first section.
    #[serde(default)]
    pub first: String,
}

/// Commands of the zoom tools (JSON `cmd` starting with `zoom`).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "cmd", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum ZoomCommand {
    /// The selected zoom → `zoom`.
    ZoomInfo,
    /// Insert a slide zoom for each of `slides` (0-based) on the current slide.
    ZoomInsertSlides {
        /// Target slides.
        slides: Vec<usize>,
    },
    /// Insert a section zoom for each of `sections` (0-based) on the current slide.
    ZoomInsertSections {
        /// Target sections.
        sections: Vec<usize>,
    },
    /// Create a summary slide with a summary zoom over the sections starting at `slides`.
    ZoomInsertSummary {
        /// First slides of the sections (0-based).
        slides: Vec<usize>,
        /// Names of the new slide and sections.
        #[serde(default)]
        names: SummaryNames,
    },
    /// Options of the selected zoom(s) (absent fields stay).
    ZoomOptions {
        /// Come back to the zoom after the target.
        #[serde(default)]
        return_to_parent: Option<bool>,
        /// Zoom transition on (with `duration_ms`) or off (the target's own transition).
        #[serde(default)]
        transition: Option<bool>,
        /// Duration of the zoom transition (ms).
        #[serde(default)]
        duration_ms: Option<u32>,
        /// Use the target's background.
        #[serde(default)]
        show_bg: Option<bool>,
    },
    /// A custom image (`imageType="cover"`): PNG / JPEG / GIF bytes, base64.
    ZoomImage {
        /// Base64 image bytes.
        data: String,
    },
    /// Back to the slide preview (`imageType="preview"`), rendered again.
    ZoomResetImage,
    /// Render the previews of the selected zooms (all zooms of the slide when none is
    /// selected) again.
    ZoomRefresh,
}

impl ZoomCommand {
    /// Parse a JSON command whose `cmd` starts with `zoom`.
    pub fn parse(v: &serde_json::Value) -> Option<ZoomCommand> {
        let cmd = v.get("cmd")?.as_str()?;
        if !cmd.starts_with("zoom") {
            return None;
        }
        serde_json::from_value(v.clone()).ok()
    }
}

/// One object of the selected zoom.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ZoomObjectOut {
    /// Target slides (0-based, playing order); empty when the target is gone.
    pub slides: Vec<usize>,
    /// Name of the target section (section and summary zooms).
    pub section: Option<String>,
    /// `zmPr` attributes.
    #[serde(flatten)]
    pub props: ZoomProps,
}

/// The selected zoom.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ZoomOut {
    /// Shape id.
    pub id: u32,
    /// Kind of zoom.
    pub kind: ZoomKind,
    /// Its objects.
    pub objects: Vec<ZoomObjectOut>,
}

/// Is the node a zoom?
pub fn is_zoom(n: &ShapeNode) -> bool {
    z::node_zoom(n).is_some()
}

/// Slides a target plays, in order: the slide, or the slides of the section.
pub fn target_slides(pres: &Presentation, t: &ZoomTarget) -> Vec<usize> {
    match t {
        ZoomTarget::Slide { sld_id } => pres
            .slides
            .iter()
            .position(|s| s.slide_id == *sld_id && *sld_id != 0)
            .into_iter()
            .collect(),
        ZoomTarget::Section { id } => sections::sections_out(pres)
            .into_iter()
            .find(|s| s.id.eq_ignore_ascii_case(id))
            .map(|s| (s.first..s.first + s.count).collect())
            .unwrap_or_default(),
    }
}

fn section_name(pres: &Presentation, t: &ZoomTarget) -> Option<String> {
    let ZoomTarget::Section { id } = t else {
        return None;
    };
    sections::sections_out(pres)
        .into_iter()
        .find(|s| s.id.eq_ignore_ascii_case(id))
        .map(|s| s.name)
}

/// Info of zoom `id` on `slide`.
pub fn zoom_out(pres: &Presentation, slide: usize, id: u32) -> Option<ZoomOut> {
    let placed = place_slide(pres, slide);
    let p = find(&placed, id)?;
    let f = node_at(pres, slide, &p.path).and_then(z::node_zoom)?;
    Some(ZoomOut {
        id,
        kind: z::zoom_kind(f)?,
        objects: z::zoom_objects(f)
            .into_iter()
            .map(|o| ZoomObjectOut {
                slides: target_slides(pres, &o.target),
                section: section_name(pres, &o.target),
                props: o.props,
            })
            .collect(),
    })
}

/// The zooms of `slide` as show links: one per object, its box in points.
pub fn show_links(pres: &Presentation, slide: usize, placed: &[Placed]) -> Vec<LinkOut> {
    let mut flat: Vec<&Placed> = Vec::new();
    super::connect::walk(placed, &mut |p| flat.push(p));
    let mut out = Vec::new();
    for p in flat {
        let Some(f) = node_at(pres, slide, &p.path).and_then(z::node_zoom) else {
            continue;
        };
        let b = p.b.bounds();
        let summary = z::zoom_kind(f) == Some(ZoomKind::Summary);
        let (fx, fy) = f.xfrm.off.unwrap_or((0, 0));
        for o in z::zoom_objects(f) {
            let slides = target_slides(pres, &o.target);
            if slides.is_empty() {
                continue;
            }
            let rect = match (summary, o.rect) {
                // Summary objects: their own boxes, placed like the frame.
                (true, Some(r)) => [
                    b.x + (r[0] - fx) as f64 / EMU_PT,
                    b.y + (r[1] - fy) as f64 / EMU_PT,
                    r[2] as f64 / EMU_PT,
                    r[3] as f64 / EMU_PT,
                ],
                _ => [b.x, b.y, b.w, b.h],
            };
            out.push(LinkOut {
                shape_id: p.id,
                rect,
                action: LinkAction::Zoom {
                    slides,
                    ret: o.props.return_to_parent,
                    transition_ms: o.props.transition_ms,
                },
                on: super::action::Trigger::Click,
                text: false,
                tooltip: None,
                highlight: false,
                sound: None,
                safety: super::action::Safety::Internal,
                key: format!("zoom:{slide}:{}:{}", p.id, out.len()),
                history: false,
            });
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Thumbnails
// ---------------------------------------------------------------------------

/// PNG of slide `i` (the engine renderer, [`THUMB_PX`] wide).
fn thumbnail(pres: &Presentation, i: usize, fonts: &mut bo_text::FontManager) -> Option<Vec<u8>> {
    let mut images = BTreeMap::new();
    let page = bo_pptx::render_slide(pres, i, fonts, &mut images).ok()?;
    if page.width <= 0.0 {
        return None;
    }
    bo_raster::render_page_png_with(&page, fonts, THUMB_PX / page.width, &images).ok()
}

/// Add an image under a name derived from its bytes (hosts cache pictures by part name).
fn image_part(tx: &mut Tx<'_>, data: Vec<u8>, ext: &str, mime: &str) -> R<String> {
    let hash = data.iter().fold(0xcbf2_9ce4_8422_2325u64, |h, &b| {
        (h ^ u64::from(b)).wrapping_mul(0x100_0000_01b3)
    });
    let name = format!("/ppt/media/zoom-{hash:016x}.{ext}");
    if !tx
        .pres
        .parts
        .iter()
        .any(|p| p.name.eq_ignore_ascii_case(&name))
    {
        let at = tx.pres.parts.len();
        tx.op(SlideOp::Parts {
            index: at,
            remove: vec![],
            insert: vec![RawPart {
                name: name.clone(),
                content_type: Some(mime.into()),
                data,
            }],
        })?;
    }
    Ok(name)
}

fn file_of(part: &str) -> &str {
    part.rsplit('/').next().unwrap_or_default()
}

fn new_rid(rels: &[Rel], taken: &[String]) -> String {
    (1..=rels.len() + taken.len() + 1)
        .map(|n| format!("rId{n}"))
        .find(|id| rels.iter().all(|r| &r.id != id) && !taken.contains(id))
        .unwrap_or_default()
}

/// Slide used for the preview of a target: the slide, or the section's first slide.
fn preview_slide(pres: &Presentation, t: &ZoomTarget) -> Option<usize> {
    target_slides(pres, t).first().copied()
}

/// Fingerprint of what a preview shows (the slide's content and layout).
fn fingerprint(pres: &Presentation, slide: usize) -> u64 {
    let Some(s) = pres.slides.get(slide) else {
        return 0;
    };
    let bytes = serde_json::to_vec(&(&s.data, s.layout, &s.clr_map_ovr)).unwrap_or_default();
    bytes.iter().fold(0xcbf2_9ce4_8422_2325u64, |h, &b| {
        (h ^ u64::from(b)).wrapping_mul(0x100_0000_01b3)
    })
}

// ---------------------------------------------------------------------------
// Insert
// ---------------------------------------------------------------------------

/// What a new object needs: its target, box (EMU) and preview slide.
struct Planned {
    target: ZoomTarget,
    rect: [i64; 4],
    preview: usize,
    name: String,
}

/// Rels and parts for the objects of a new zoom on `slide`: (image rid, link rid) each.
fn add_object_rels(
    tx: &mut Tx<'_>,
    slide: usize,
    plan: &[Planned],
    fonts: &mut bo_text::FontManager,
) -> R<Vec<(String, String)>> {
    let mut out = Vec::new();
    let mut taken: Vec<String> = Vec::new();
    let mut rels: Vec<Rel> = Vec::new();
    for p in plan {
        let png = thumbnail(tx.pres, p.preview, fonts).ok_or(EditError::Refused("render"))?;
        let part = image_part(tx, png, "png", "image/png")?;
        let s = tx.pres.slides.get(slide).ok_or(OpError::NoSlide(slide))?;
        let image = new_rid(&s.info.rels, &taken);
        taken.push(image.clone());
        let link = new_rid(&s.info.rels, &taken);
        taken.push(link.clone());
        let target = tx
            .pres
            .slides
            .get(p.preview)
            .map(|t| file_of(&t.info.part_name).to_owned())
            .unwrap_or_default();
        rels.push(Rel {
            id: image.clone(),
            rel_type: format!("{}image", bo_pptx::RELATIONSHIP_NS),
            target: format!("../media/{}", file_of(&part)),
            external: false,
        });
        let link = if target.is_empty() {
            String::new()
        } else {
            // ECMA-376 Part 1, §21.1.2.3.5: the fallback picture's hyperlink to the slide.
            rels.push(Rel {
                id: link.clone(),
                rel_type: format!("{}slide", bo_pptx::RELATIONSHIP_NS),
                target,
                external: false,
            });
            link
        };
        out.push((image, link));
    }
    tx.fields(slide, |f| f.rels.extend(rels))?;
    Ok(out)
}

fn zoom_props(tx: &Tx<'_>, seed: &str) -> ZoomProps {
    let taken = all_zoom_ids(tx.pres);
    let taken: Vec<&str> = taken.iter().map(String::as_str).collect();
    ZoomProps {
        id: sections::new_id(&taken, seed),
        ..ZoomProps::default()
    }
}

fn all_zoom_ids(pres: &Presentation) -> Vec<String> {
    let mut v = Vec::new();
    for s in &pres.slides {
        bo_pptx::visit(&s.data.shapes, &mut |n| {
            if let Some(f) = z::node_zoom(n) {
                v.extend(z::zoom_objects(f).into_iter().map(|o| o.props.id));
            }
        });
    }
    v
}

/// Build and insert one zoom frame on `slide` from `plan`.
fn insert_zoom(
    tx: &mut Tx<'_>,
    sel: &mut SlideSel,
    kind: ZoomKind,
    name: &str,
    rect: [i64; 4],
    plan: &[Planned],
    fonts: &mut bo_text::FontManager,
) -> R<u32> {
    let slide = sel.slide;
    let rids = add_object_rels(tx, slide, plan, fonts)?;
    let id = next_shape_id(tx.pres, slide);
    let mut objects = Vec::new();
    for (k, (p, (image, link))) in plan.iter().zip(rids).enumerate() {
        let mut props = zoom_props(tx, &format!("{}\u{0}{id}\u{0}{k}", p.name));
        // Ids handed out in this command are taken too.
        if objects
            .iter()
            .any(|o: &NewObject| o.props.id.eq_ignore_ascii_case(&props.id))
        {
            props.id = sections::new_id(&[props.id.as_str()], &format!("{k}"));
        }
        objects.push(NewObject {
            target: p.target.clone(),
            props,
            rect: p.rect,
            image_rid: image,
            link_rid: link,
            name: p.name.clone(),
        });
    }
    let fallback_ids: Vec<u32> = (1..=objects.len() as u32).map(|k| id + k).collect();
    let xml = z::node_xml(kind, id, name, rect, &objects, &fallback_ids);
    insert_top(tx, sel, build::node(&xml))?;
    Ok(id)
}

/// Boxes (points) of `n` thumbnails side by side in the middle of the slide.
fn row_boxes(sw: f64, sh: f64, n: usize) -> Vec<[f64; 4]> {
    let n = n.max(1) as f64;
    let mut w = sw / 4.0;
    let gap = w / 8.0;
    if n * w + (n - 1.0) * gap > sw * 0.9 {
        w = (sw * 0.9 - (n - 1.0) * gap) / n;
    }
    let h = w * sh / sw;
    let total = n * w + (n - 1.0) * gap;
    let x0 = (sw - total) / 2.0;
    let y = (sh - h) / 2.0;
    (0..n as usize)
        .map(|i| [x0 + i as f64 * (w + gap), y, w, h])
        .collect()
}

/// Grid of `n` thumbnails below the title area (summary zoom).
pub fn grid_boxes(sw: f64, sh: f64, n: usize) -> Vec<[f64; 4]> {
    let n = n.max(1);
    let cols = (n as f64).sqrt().ceil().max(1.0) as usize;
    let cols = cols.min(n);
    let rows = n.div_ceil(cols);
    let (ax, ay, aw, ah) = (sw * 0.06, sh * 0.27, sw * 0.88, sh * 0.66);
    let gap = aw * 0.03;
    let cw = (aw - gap * (cols as f64 - 1.0)) / cols as f64;
    let ch = (ah - gap * (rows as f64 - 1.0)) / rows as f64;
    let (w, h) = if cw * sh / sw <= ch {
        (cw, cw * sh / sw)
    } else {
        (ch * sw / sh, ch)
    };
    let total_h = rows as f64 * h + (rows as f64 - 1.0) * gap;
    let y0 = ay + (ah - total_h) / 2.0;
    (0..n)
        .map(|i| {
            let (r, c) = (i / cols, i % cols);
            // The last row is centred.
            let in_row = if r == rows - 1 { n - r * cols } else { cols };
            let row_w = in_row as f64 * w + (in_row as f64 - 1.0) * gap;
            let x0 = ax + (aw - row_w) / 2.0;
            [x0 + c as f64 * (w + gap), y0 + r as f64 * (h + gap), w, h]
        })
        .collect()
}

fn emu_box(b: [f64; 4]) -> [i64; 4] {
    b.map(|v| (v * EMU_PT).round() as i64)
}

fn bounds_of(boxes: &[[i64; 4]]) -> [i64; 4] {
    let x0 = boxes.iter().map(|b| b[0]).min().unwrap_or(0);
    let y0 = boxes.iter().map(|b| b[1]).min().unwrap_or(0);
    let x1 = boxes.iter().map(|b| b[0] + b[2]).max().unwrap_or(1);
    let y1 = boxes.iter().map(|b| b[1] + b[3]).max().unwrap_or(1);
    [x0, y0, x1 - x0, y1 - y0]
}

/// Insert a slide zoom (`Slide`) or section zoom (`Section`) for each target.
pub fn cmd_insert(
    tx: &mut Tx<'_>,
    sel: &mut SlideSel,
    targets: Vec<ZoomTarget>,
    fonts: &mut bo_text::FontManager,
) -> R<()> {
    if targets.is_empty() {
        return Err(EditError::Refused("noTarget"));
    }
    let (sw, sh) = slide_size(tx.pres);
    let boxes = row_boxes(sw, sh, targets.len());
    let mut ids = Vec::new();
    for (t, b) in targets.into_iter().zip(boxes) {
        let preview = preview_slide(tx.pres, &t).ok_or(EditError::Refused("noTarget"))?;
        let (kind, name) = match &t {
            ZoomTarget::Slide { .. } => (ZoomKind::Slide, format!("Slide Zoom {}", preview + 1)),
            ZoomTarget::Section { .. } => (
                ZoomKind::Section,
                section_name(tx.pres, &t).unwrap_or_else(|| "Section Zoom".into()),
            ),
        };
        let rect = emu_box(b);
        let plan = [Planned {
            target: t,
            rect,
            preview,
            name: name.clone(),
        }];
        ids.push(insert_zoom(tx, sel, kind, &name, rect, &plan, fonts)?);
    }
    sel.ids = ids;
    Ok(())
}

/// Plain text of slide `i`'s title placeholder.
fn slide_title(pres: &Presentation, i: usize) -> Option<String> {
    let s = pres.slides.get(i)?;
    s.data.shapes.iter().find_map(|n| match n {
        ShapeNode::Shape(sp)
            if sp
                .nv
                .placeholder
                .as_ref()
                .is_some_and(|p| matches!(p.kind.as_deref(), Some("title" | "ctrTitle"))) =>
        {
            let t = super::clip::plain_text(std::slice::from_ref(n));
            let t = t.split_whitespace().collect::<Vec<_>>().join(" ");
            (!t.is_empty()).then_some(t)
        }
        _ => None,
    })
}

/// Layout of the summary slide: "title only" of the master of slide `near`, else a layout
/// like a new slide's.
fn summary_layout(pres: &Presentation, near: usize) -> Option<usize> {
    let master = pres
        .slides
        .get(near)
        .and_then(|s| pres.layouts.get(s.layout))
        .map(|l| l.master);
    pres.layouts
        .iter()
        .position(|l| Some(l.master) == master && l.kind.as_deref() == Some("titleOnly"))
        .or_else(|| super::layout_for_new(pres, near.saturating_sub(1)))
}

/// Create the summary slide (before the first of `slides`) with a summary zoom over the
/// sections starting at `slides`, creating those sections where needed.
pub fn cmd_insert_summary(
    tx: &mut Tx<'_>,
    sel: &mut SlideSel,
    slides: &[usize],
    names: &SummaryNames,
    fonts: &mut bo_text::FontManager,
) -> R<()> {
    let n = tx.pres.slides.len();
    let mut picked: Vec<usize> = slides.iter().copied().filter(|&s| s < n).collect();
    picked.sort_unstable();
    picked.dedup();
    let Some(&at) = picked.first() else {
        return Err(EditError::Refused("noTarget"));
    };
    let pick_names: Vec<Option<String>> = picked.iter().map(|&i| slide_title(tx.pres, i)).collect();
    sections::assign_ids(tx)?;
    // The summary slide.
    let layout = summary_layout(tx.pres, at).ok_or(EditError::Refused("noLayout"))?;
    let mut slide = build::slide_from_layout(tx.pres, layout);
    // Only the title: the zooms take the rest of the slide.
    slide.data.shapes.retain(|n| match n {
        ShapeNode::Shape(sp) => sp
            .nv
            .placeholder
            .as_ref()
            .is_none_or(|p| matches!(p.kind.as_deref(), Some("title" | "ctrTitle"))),
        _ => true,
    });
    for node in &mut slide.data.shapes {
        if let ShapeNode::Shape(sp) = node {
            let title = sp
                .nv
                .placeholder
                .as_ref()
                .is_some_and(|p| matches!(p.kind.as_deref(), Some("title" | "ctrTitle")));
            if title && !names.title.is_empty() {
                if let Some(tb) = sp.text.as_mut() {
                    super::text::set_text(tb, &names.title);
                }
            }
        }
    }
    tx.op(SlideOp::Slides {
        index: at,
        remove: vec![],
        insert: vec![slide],
    })?;
    sections::assign_ids(tx)?;
    let picked: Vec<usize> = picked.iter().map(|i| i + 1).collect();
    let summary_name = if names.summary.is_empty() {
        "Summary Section"
    } else {
        names.summary.as_str()
    };
    let first = if names.first.is_empty() {
        "Default Section"
    } else {
        names.first.as_str()
    };
    sections::cmd_add(tx, sel, summary_name, Some(at), first)?;
    for (k, &s) in picked.iter().enumerate() {
        let starts = sections::sections_out(tx.pres)
            .iter()
            .any(|x| x.first == s && x.count > 0);
        if !starts {
            let name = pick_names[k].clone().unwrap_or_else(|| {
                let t = if names.section.is_empty() {
                    "Section {n}"
                } else {
                    names.section.as_str()
                };
                t.replace("{n}", &(k + 1).to_string())
            });
            sections::cmd_add(tx, sel, &name, Some(s), first)?;
        }
    }
    let secs = sections::sections_out(tx.pres);
    let (sw, sh) = slide_size(tx.pres);
    let boxes: Vec<[i64; 4]> = grid_boxes(sw, sh, picked.len())
        .into_iter()
        .map(emu_box)
        .collect();
    let mut plan = Vec::new();
    for (&s, b) in picked.iter().zip(&boxes) {
        let sec = secs
            .iter()
            .find(|x| x.first == s && x.count > 0)
            .ok_or(EditError::Refused("noSection"))?;
        plan.push(Planned {
            target: ZoomTarget::Section { id: sec.id.clone() },
            rect: *b,
            preview: s,
            name: sec.name.clone(),
        });
    }
    *sel = SlideSel {
        slide: at,
        ids: vec![],
    };
    let rect = bounds_of(&boxes);
    let id = insert_zoom(
        tx,
        sel,
        ZoomKind::Summary,
        if names.title.is_empty() {
            "Summary Zoom"
        } else {
            names.title.as_str()
        },
        rect,
        &plan,
        fonts,
    )?;
    sel.ids = vec![id];
    Ok(())
}

// ---------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------

/// The zoom frames among the selected shapes (all zooms of the slide with `all_if_none`
/// and nothing selected): their paths.
fn selected_zooms(pres: &Presentation, sel: &SlideSel, all_if_none: bool) -> Vec<Vec<usize>> {
    let placed = place_slide(pres, sel.slide);
    let mut out = Vec::new();
    if sel.ids.is_empty() && all_if_none {
        super::connect::walk(&placed, &mut |p| {
            if node_at(pres, sel.slide, &p.path).is_some_and(is_zoom) {
                out.push(p.path.clone());
            }
        });
        return out;
    }
    for id in &sel.ids {
        if let Some(p) = find(&placed, *id) {
            if node_at(pres, sel.slide, &p.path).is_some_and(is_zoom) {
                out.push(p.path.clone());
            }
        }
    }
    out
}

fn edit_frame(
    tx: &mut Tx<'_>,
    slide: usize,
    path: &[usize],
    f: impl FnOnce(&mut bo_model::slide::GraphicFrame),
) -> R<()> {
    let Some(ShapeNode::Frame(fr)) = node_at(tx.pres, slide, path) else {
        return Err(EditError::Refused("noZoom"));
    };
    let mut fr = (**fr).clone();
    f(&mut fr);
    tx.replace_node(slide, path, ShapeNode::Frame(Box::new(fr)))?;
    Ok(())
}

/// Default zoom transition (ms) when it is switched on without a duration.
pub const DEFAULT_TRANSITION_MS: u32 = 1000;

/// Change the options of the selected zooms.
pub fn cmd_options(tx: &mut Tx<'_>, sel: &SlideSel, ch: &ZoomCommand) -> R<()> {
    let ZoomCommand::ZoomOptions {
        return_to_parent,
        transition,
        duration_ms,
        show_bg,
    } = ch
    else {
        return Ok(());
    };
    let paths = selected_zooms(tx.pres, sel, false);
    if paths.is_empty() {
        return Err(EditError::Refused("noZoom"));
    }
    for path in paths {
        let cur = match node_at(tx.pres, sel.slide, &path).and_then(z::node_zoom) {
            Some(f) => z::zoom_objects(f),
            None => continue,
        };
        edit_frame(tx, sel.slide, &path, |f| {
            for (o, old) in z::object_elements_mut(f).into_iter().zip(&cur) {
                let dur = duration_ms.map(|d| d.clamp(1, 60_000));
                let transition_ms = match (transition, dur) {
                    (Some(false), _) => Some(None),
                    (Some(true), d) => Some(Some(
                        d.or(old.props.transition_ms)
                            .unwrap_or(DEFAULT_TRANSITION_MS),
                    )),
                    // A new duration switches the zoom transition on.
                    (None, Some(d)) => Some(Some(d)),
                    (None, None) => None,
                };
                z::set_props(
                    o,
                    &PropsChange {
                        return_to_parent: *return_to_parent,
                        cover: None,
                        transition_ms,
                        show_bg: *show_bg,
                    },
                );
            }
        })?;
    }
    Ok(())
}

/// Point object `k` of the zoom at `path` (choice and fallback picture) at image `part`.
fn set_object_image(
    tx: &mut Tx<'_>,
    slide: usize,
    path: &[usize],
    k: usize,
    part: &str,
    cover: Option<bool>,
) -> R<()> {
    let Some(f) = node_at(tx.pres, slide, path).and_then(z::node_zoom) else {
        return Err(EditError::Refused("noZoom"));
    };
    let obj = z::zoom_objects(f).into_iter().nth(k);
    let fallback = f
        .alt
        .as_deref()
        .map(z::fallback_rids)
        .unwrap_or_default()
        .into_iter()
        .filter(|r| {
            tx.pres.slides[slide]
                .info
                .rels
                .iter()
                .any(|x| &x.id == r && x.rel_type.ends_with("/image"))
        })
        .nth(k);
    let rid = if let Some(r) = obj.and_then(|o| o.image_rid) {
        r
    } else {
        {
            let r = new_rid(&tx.pres.slides[slide].info.rels, &[]);
            let target = format!("../media/{}", file_of(part));
            let id = r.clone();
            tx.fields(slide, |f| {
                f.rels.push(Rel {
                    id,
                    rel_type: format!("{}image", bo_pptx::RELATIONSHIP_NS),
                    target,
                    external: false,
                });
            })?;
            r
        }
    };
    super::picture::retarget(tx, slide, &rid, part)?;
    let fb_rid = fallback.filter(|r| *r != rid);
    if let Some(r) = &fb_rid {
        super::picture::retarget(tx, slide, r, part)?;
    }
    edit_frame(tx, slide, path, |f| {
        if let Some(o) = z::object_elements_mut(f).into_iter().nth(k) {
            z::set_image_rid(o, &rid);
            if cover.is_some() {
                z::set_props(
                    o,
                    &PropsChange {
                        cover,
                        ..PropsChange::default()
                    },
                );
            }
        }
        if fb_rid.is_none() {
            if let Some(alt) = f.alt.as_deref_mut() {
                z::fallback_set_image(alt, k, &rid);
            }
        }
    })
}

/// A custom image for the selected zooms (every object).
pub fn cmd_image(tx: &mut Tx<'_>, sel: &SlideSel, bytes: Vec<u8>) -> R<()> {
    let (ext, mime) = if bytes.starts_with(b"\x89PNG") {
        ("png", "image/png")
    } else if bytes.starts_with(&[0xFF, 0xD8]) {
        ("jpeg", "image/jpeg")
    } else if bytes.starts_with(b"GIF8") {
        ("gif", "image/gif")
    } else {
        return Err(EditError::Refused("badImage"));
    };
    let paths = selected_zooms(tx.pres, sel, false);
    if paths.is_empty() {
        return Err(EditError::Refused("noZoom"));
    }
    let part = image_part(tx, bytes, ext, mime)?;
    for path in paths {
        let n = node_at(tx.pres, sel.slide, &path)
            .and_then(z::node_zoom)
            .map_or(0, |f| z::zoom_objects(f).len());
        for k in 0..n {
            set_object_image(tx, sel.slide, &path, k, &part, Some(true))?;
        }
    }
    Ok(())
}

/// Render the preview of object `k` of the zoom at `path` again (and switch it back to the
/// preview with `reset`). Nothing changes when the picture is the same.
fn refresh_object(
    tx: &mut Tx<'_>,
    slide: usize,
    path: &[usize],
    k: usize,
    reset: bool,
    fonts: &mut bo_text::FontManager,
) -> R<()> {
    let Some(o) = node_at(tx.pres, slide, path)
        .and_then(z::node_zoom)
        .and_then(|f| z::zoom_objects(f).into_iter().nth(k))
    else {
        return Ok(());
    };
    if o.props.cover && !reset {
        return Ok(());
    }
    let Some(preview) = preview_slide(tx.pres, &o.target) else {
        return Ok(());
    };
    let png = thumbnail(tx.pres, preview, fonts).ok_or(EditError::Refused("render"))?;
    let current = o
        .image_rid
        .as_deref()
        .and_then(|r| super::picture::rel_part(tx.pres, slide, r))
        .and_then(|p| tx.pres.part_data(&p).map(<[u8]>::to_vec));
    if current.as_deref() == Some(png.as_slice()) && !o.props.cover {
        return Ok(());
    }
    let part = image_part(tx, png, "png", "image/png")?;
    set_object_image(tx, slide, path, k, &part, o.props.cover.then_some(false))
}

/// Render the previews of the selected zooms again (`reset`: also custom images go back to
/// the preview).
pub fn cmd_refresh(
    tx: &mut Tx<'_>,
    sel: &SlideSel,
    reset: bool,
    fonts: &mut bo_text::FontManager,
) -> R<()> {
    let paths = selected_zooms(tx.pres, sel, !reset);
    if paths.is_empty() {
        return Err(EditError::Refused("noZoom"));
    }
    for path in paths {
        let n = node_at(tx.pres, sel.slide, &path)
            .and_then(z::node_zoom)
            .map_or(0, |f| z::zoom_objects(f).len());
        for k in 0..n {
            refresh_object(tx, sel.slide, &path, k, reset, fonts)?;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Keeping previews up to date
// ---------------------------------------------------------------------------

/// (slide, path, object index, key, preview slide) of every preview zoom object.
fn preview_objects(pres: &Presentation) -> Vec<(usize, Vec<usize>, usize, String, usize)> {
    let mut out = Vec::new();
    for (i, s) in pres.slides.iter().enumerate() {
        let mut any = false;
        bo_pptx::visit(&s.data.shapes, &mut |n| any |= is_zoom(n));
        if !any {
            continue;
        }
        let placed = place_slide(pres, i);
        super::connect::walk(&placed, &mut |p| {
            let Some(f) = node_at(pres, i, &p.path).and_then(z::node_zoom) else {
                return;
            };
            for (k, o) in z::zoom_objects(f).into_iter().enumerate() {
                if o.props.cover {
                    continue;
                }
                if let Some(t) = preview_slide(pres, &o.target) {
                    let key = format!("{}#{}#{k}#{}", s.info.part_name, p.id, o.props.id);
                    out.push((i, p.path.clone(), k, key, t));
                }
            }
        });
    }
    out
}

impl SlideEditor {
    /// After a command that changed something: render again the previews of zooms whose
    /// target slide changed (same undo step). Previews are compared with what the target
    /// looked like when this session first saw the zoom, so opening a deck changes nothing.
    pub(super) fn zoom_sync(
        &mut self,
        pres: &mut Presentation,
        out: &mut Outcome,
        mark: (usize, usize, usize),
        history: bool,
        fonts: &mut bo_text::FontManager,
    ) {
        if history {
            // Undo / redo put previews back with the edits: look again from here.
            self.zoom_seen.clear();
            return;
        }
        if self.read_only {
            return;
        }
        let objs = preview_objects(pres);
        if objs.is_empty() {
            return;
        }
        if self.history_mark() == mark {
            // Nothing changed (the first `state` after opening, selections…): remember the
            // zooms not seen yet, so the first edit of a target is noticed.
            for (_, _, _, key, t) in objs {
                self.zoom_seen
                    .entry(key)
                    .or_insert_with(|| fingerprint(pres, t));
            }
            return;
        }
        let mut stale = Vec::new();
        for (slide, path, k, key, t) in objs {
            let fp = fingerprint(pres, t);
            match self.zoom_seen.insert(key, fp) {
                Some(old) if old != fp => stale.push((slide, path, k)),
                _ => {}
            }
        }
        if stale.is_empty() {
            return;
        }
        let mut tx = Tx::new(pres);
        let mut ok = true;
        for (slide, path, k) in &stale {
            if refresh_object(&mut tx, *slide, path, *k, false, fonts).is_err() {
                ok = false;
                break;
            }
        }
        if !ok {
            tx.rollback();
            return;
        }
        let ops = std::mem::take(&mut tx.ops);
        out.note(&ops);
        for (slide, _, _) in &stale {
            out.touched.insert(*slide);
        }
        if let Some(last) = self.undo.last_mut() {
            last.ops.extend(ops);
        }
    }

    /// Remember how the targets of every zoom look now (after inserting zooms).
    fn zoom_seen_now(&mut self, pres: &Presentation) {
        for (_, _, _, key, t) in preview_objects(pres) {
            let fp = fingerprint(pres, t);
            self.zoom_seen.insert(key, fp);
        }
    }
}

fn b64_decode(s: &str) -> Option<Vec<u8>> {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD
        .decode(s.trim().as_bytes())
        .ok()
}

impl crate::Engine {
    /// Run a zoom command.
    pub fn slide_zoom_command(&mut self, cmd: &ZoomCommand) -> Result<SlideEditResult> {
        let _salt = build::salt_names(
            self.slide_collab
                .as_ref()
                .map(|c| build::salt_of(c.client_id())),
        );
        let mut out = Outcome::default();
        {
            let pres = self.pres.as_mut().ok_or(EngineError::NoDocument)?;
            let fonts = &mut self.fonts;
            let ed = &mut self.slides;
            ed.clamp(pres);
            ed.text = None;
            let mark = ed.history_mark();
            match cmd {
                ZoomCommand::ZoomInfo => {}
                ZoomCommand::ZoomInsertSlides { slides } => {
                    let targets: Vec<ZoomTarget> = slides
                        .iter()
                        .filter_map(|&i| pres.slides.get(i))
                        .map(|s| s.slide_id)
                        .collect::<Vec<_>>()
                        .into_iter()
                        .map(|sld_id| ZoomTarget::Slide { sld_id })
                        .collect();
                    ed.mutate(pres, &mut out, |tx, sel| {
                        // New slides get their `p:sldId@id` first.
                        sections::assign_ids(tx)?;
                        let targets: Vec<ZoomTarget> = if targets
                            .iter()
                            .any(|t| matches!(t, ZoomTarget::Slide { sld_id: 0 }))
                        {
                            slides
                                .iter()
                                .filter_map(|&i| tx.pres.slides.get(i))
                                .map(|s| ZoomTarget::Slide { sld_id: s.slide_id })
                                .collect()
                        } else {
                            targets
                        };
                        cmd_insert(tx, sel, targets, fonts)
                    });
                }
                ZoomCommand::ZoomInsertSections { sections: picked } => {
                    let secs = sections::sections_out(pres);
                    let targets: Vec<ZoomTarget> = picked
                        .iter()
                        .filter_map(|&i| secs.get(i))
                        .filter(|s| s.count > 0)
                        .map(|s| ZoomTarget::Section { id: s.id.clone() })
                        .collect();
                    ed.mutate(pres, &mut out, |tx, sel| {
                        cmd_insert(tx, sel, targets, fonts)
                    });
                }
                ZoomCommand::ZoomInsertSummary { slides, names } => {
                    ed.mutate(pres, &mut out, |tx, sel| {
                        cmd_insert_summary(tx, sel, slides, names, fonts)
                    });
                }
                ZoomCommand::ZoomOptions { .. } => {
                    ed.mutate(pres, &mut out, |tx, sel| cmd_options(tx, sel, cmd));
                }
                ZoomCommand::ZoomImage { data } => match b64_decode(data) {
                    Some(bytes) => ed.mutate(pres, &mut out, |tx, sel| cmd_image(tx, sel, bytes)),
                    None => out.error = Some("badImage".into()),
                },
                ZoomCommand::ZoomResetImage => {
                    ed.mutate(pres, &mut out, |tx, sel| cmd_refresh(tx, sel, true, fonts));
                }
                ZoomCommand::ZoomRefresh => {
                    ed.mutate(pres, &mut out, |tx, sel| cmd_refresh(tx, sel, false, fonts));
                }
            }
            ed.sections_sync(pres, mark, false);
            if ed.history_mark() != mark {
                ed.zoom_seen_now(pres);
            }
            ed.clamp(pres);
        }
        let mut r = self.finish_slide_command(out)?;
        r.collab = self.slide_collab_after_edit();
        Ok(r)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid_fits_the_area() {
        let (sw, sh) = (960.0, 540.0);
        for n in 1..=9 {
            let g = grid_boxes(sw, sh, n);
            assert_eq!(g.len(), n);
            for b in &g {
                assert!(b[0] >= 0.0 && b[1] >= sh * 0.26 && b[0] + b[2] <= sw);
                assert!(b[1] + b[3] <= sh * 0.94);
                // Slide aspect.
                assert!((b[2] / b[3] - sw / sh).abs() < 1e-9);
            }
        }
        // Four in a 2 × 2 grid.
        let g = grid_boxes(sw, sh, 4);
        assert!((g[0][1] - g[1][1]).abs() < 1e-9 && g[2][1] > g[0][1]);
    }

    #[test]
    fn row_is_centred() {
        let r = row_boxes(960.0, 540.0, 2);
        let left = r[0][0];
        let right = 960.0 - (r[1][0] + r[1][2]);
        assert!((left - right).abs() < 1e-9);
    }
}
