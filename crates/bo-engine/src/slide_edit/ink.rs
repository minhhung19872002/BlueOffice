//! BlueSlide ink (CORE-INK-001): strokes drawn with the Draw tab are stored as
//! InkML content parts ([MS-ODRAWXML] §1.3.2, §2.1.4; ECMA-376 Part 1,
//! §19.3.1.14 contentPart) — one ink object per stroke, each an undoable step —
//! and erased stroke by stroke or point by point. Ink read from a file is
//! edited in place: only the traces that change are rewritten, everything else
//! in the part is kept.
//!
//! Commands (`cmd`): `inkInfo` (ink of the current slide in slide points, for
//! the eraser preview and ink replay; with `backdrop`, the slide drawn without
//! its ink), `addInk`, `eraseInk`.

use super::build;
use super::geom::EMU_PT;
use super::ops::{OpError, Tx};
use super::{next_shape_id, Outcome, SlideEditResult, SlideSel, R};
use crate::{EngineError, Result};
use bo_drawingml::ink::{tips, InkDoc, NewStroke, Placement, Tip, Trace, CONTENT_TYPE, REL_TYPE};
use bo_model::slide::{Presentation, Rel, ShapeNode};
use serde::{Deserialize, Serialize};

/// Ink commands.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "cmd", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum InkCommand {
    /// Ink objects of the current slide → `ink`.
    InkInfo {
        /// Also the slide drawn without ink (`ink.backdrop`, for replay).
        #[serde(default)]
        backdrop: bool,
    },
    /// Add one stroke as a new ink object on top of the current slide.
    AddInk {
        /// The stroke (slide points).
        stroke: InkStrokeIn,
        /// Base name of the object (default "Ink").
        #[serde(default)]
        name: String,
    },
    /// Erase along a path of the current slide (one undo step).
    EraseInk {
        /// Eraser path (slide points).
        points: Vec<[f64; 2]>,
        /// Eraser radius in points.
        radius: f64,
        /// Whole strokes or only the points under the eraser.
        #[serde(default)]
        mode: EraseMode,
    },
}

/// Command names of [`InkCommand`].
pub const COMMANDS: &[&str] = &["inkInfo", "addInk", "eraseInk"];

impl InkCommand {
    /// Parses a JSON command when it is an ink command.
    pub fn parse(v: &serde_json::Value) -> Option<InkCommand> {
        let c = v.get("cmd")?.as_str()?;
        if !COMMANDS.contains(&c) {
            return None;
        }
        serde_json::from_value(v.clone()).ok()
    }
}

/// A stroke drawn in the editor.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InkStrokeIn {
    /// Points `[x, y]` (slide points).
    pub points: Vec<[f64; 2]>,
    /// Pen pressure 0..=1 per point (pen input only).
    #[serde(default)]
    pub pressure: Option<Vec<f64>>,
    /// Milliseconds since the stroke started, per point.
    #[serde(default)]
    pub times: Option<Vec<f64>>,
    /// `RRGGBB`.
    pub color: String,
    /// Pen width in points.
    pub width: f64,
    /// Highlighter.
    #[serde(default)]
    pub highlighter: bool,
}

/// What the eraser removes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EraseMode {
    /// Every stroke it touches.
    #[default]
    Stroke,
    /// The samples under it (strokes are split).
    Point,
}

/// Ink of the current slide.
#[derive(Debug, Clone, PartialEq, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct InkOut {
    /// Ink objects in z-order.
    pub objects: Vec<InkObjectOut>,
    /// The slide drawn without ink (`inkInfo` with `backdrop`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backdrop: Option<bo_render::PageList>,
}

/// One ink object (content part).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InkObjectOut {
    /// `cNvPr@id`.
    pub id: u32,
    /// `cNvPr@name`.
    pub name: String,
    /// Frame `[x, y, w, h]` (points).
    pub rect: [f64; 4],
    /// Traces in document order.
    pub strokes: Vec<InkStrokeOut>,
}

/// One trace in slide points.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InkStrokeOut {
    /// Samples `[x, y, width]` (width after pressure).
    pub points: Vec<[f64; 3]>,
    /// Time of each sample in ms (channel `T`), when recorded.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub times: Option<Vec<f64>>,
    /// `RRGGBB`.
    pub color: String,
    /// Opacity 0..=1.
    pub opacity: f64,
    /// Highlighter (`rasterOp` `maskPen`).
    pub highlighter: bool,
    /// Rectangular tip.
    pub rect_tip: bool,
}

/// An ink object found on a slide.
struct Obj {
    /// Index in the spTree.
    index: usize,
    id: u32,
    name: String,
    rid: String,
    part: String,
    doc: InkDoc,
    traces: Vec<Trace>,
    place: Placement,
    /// Frame origin (points).
    off: (f64, f64),
    ext: (f64, f64),
}

impl Obj {
    fn slide_tips(&self, t: &Trace) -> Vec<bo_drawingml::ink::TipAt> {
        tips(t, &self.place)
            .into_iter()
            .map(|mut p| {
                p.x += self.off.0;
                p.y += self.off.1;
                p
            })
            .collect()
    }
}

/// Top-level ink objects of a slide (ink inside groups is drawn, not edited).
fn objects(pres: &Presentation, slide: usize) -> Vec<Obj> {
    let Some(s) = pres.slides.get(slide) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (index, n) in s.data.shapes.iter().enumerate() {
        let Some(cp) = bo_pptx::ink::node_content_part(n) else {
            continue;
        };
        let Some(rid) = bo_pptx::ink::content_part_rid(cp) else {
            continue;
        };
        let Some(part) = s
            .info
            .rels
            .iter()
            .find(|r| r.id == rid && !r.external)
            .map(|r| bo_opc::resolve_target(&s.info.part_name, &r.target))
        else {
            continue;
        };
        let Some(doc) = pres.part_data(&part).and_then(InkDoc::parse) else {
            continue;
        };
        let traces = doc.traces();
        let (off, ext) = match bo_pptx::ink::content_part_xfrm(cp) {
            Some(x) => {
                let (ox, oy) = x.off.unwrap_or((0, 0));
                let (cx, cy) = x.ext.unwrap_or((0, 0));
                (
                    (ox as f64 / EMU_PT, oy as f64 / EMU_PT),
                    (cx as f64 / EMU_PT, cy as f64 / EMU_PT),
                )
            }
            None => match Placement::natural(&traces) {
                Some((p, size)) => ((p.min.0 - p.margin.0, p.min.1 - p.margin.1), size),
                None => ((0.0, 0.0), (0.0, 0.0)),
            },
        };
        let Some(place) = Placement::fit(&traces, ext.0, ext.1) else {
            continue;
        };
        let (id, name) = bo_pptx::ink::content_part_nv(cp).unwrap_or((0, String::new()));
        out.push(Obj {
            index,
            id,
            name,
            rid: rid.to_owned(),
            part,
            doc,
            traces,
            place,
            off,
            ext,
        });
    }
    out
}

/// `ink` answer for a slide.
pub fn ink_out(pres: &Presentation, slide: usize) -> InkOut {
    let objects = objects(pres, slide)
        .iter()
        .map(|o| InkObjectOut {
            id: o.id,
            name: o.name.clone(),
            rect: [o.off.0, o.off.1, o.ext.0, o.ext.1],
            strokes: o
                .traces
                .iter()
                .map(|t| {
                    let tips = o.slide_tips(t);
                    let times: Option<Vec<f64>> = t.samples.iter().map(|s| s.t).collect();
                    InkStrokeOut {
                        points: tips
                            .iter()
                            .map(|p| [p.x, p.y, 2.0 * p.rx.max(p.ry)])
                            .collect(),
                        times,
                        color: t.brush.hex(),
                        opacity: t.brush.opacity(),
                        highlighter: t.brush.is_highlighter(),
                        rect_tip: t.brush.tip == Tip::Rectangle,
                    }
                })
                .collect(),
        })
        .collect();
    InkOut {
        objects,
        backdrop: None,
    }
}

fn emu(pt: f64) -> i64 {
    (pt * EMU_PT).round() as i64
}

/// Freeform fallback of an ink object (§19.3.1.43 sp, §20.1.9.8 custGeom):
/// one path per trace, in the first trace's colour and nominal width.
fn fallback_xml(id: u32, name: &str, strokes: &[Vec<(f64, f64)>], first: &Trace) -> Option<String> {
    let pts = || strokes.iter().flatten();
    let (mut x0, mut y0) = *pts().next()?;
    let (mut x1, mut y1) = (x0, y0);
    for &(x, y) in pts() {
        x0 = x0.min(x);
        y0 = y0.min(y);
        x1 = x1.max(x);
        y1 = y1.max(y);
    }
    let (cx, cy) = (emu(x1 - x0).max(1), emu(y1 - y0).max(1));
    let mut paths = String::new();
    for s in strokes {
        let Some(&(fx, fy)) = s.first() else { continue };
        let mut d = format!(
            r#"<a:moveTo><a:pt x="{}" y="{}"/></a:moveTo>"#,
            emu(fx - x0),
            emu(fy - y0)
        );
        // A dot is a segment to itself so the round cap shows.
        let rest: &[(f64, f64)] = if s.len() == 1 { s } else { &s[1..] };
        for &(x, y) in rest {
            d.push_str(&format!(
                r#"<a:lnTo><a:pt x="{}" y="{}"/></a:lnTo>"#,
                emu(x - x0),
                emu(y - y0)
            ));
        }
        paths.push_str(&format!(
            r#"<a:path w="{cx}" h="{cy}" fill="none">{d}</a:path>"#
        ));
    }
    // §20.1.2.3.1 alpha for translucent ink.
    let alpha = if first.brush.opacity() < 1.0 {
        format!(
            r#"<a:alpha val="{}"/>"#,
            (first.brush.opacity() * 100_000.0).round() as i64
        )
    } else {
        String::new()
    };
    Some(format!(
        concat!(
            r#"<p:sp><p:nvSpPr><p:cNvPr id="{id}" name="{name}"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr>"#,
            r#"<p:spPr><a:xfrm><a:off x="{x}" y="{y}"/><a:ext cx="{cx}" cy="{cy}"/></a:xfrm>"#,
            r#"<a:custGeom><a:avLst/><a:gdLst/><a:ahLst/><a:cxnLst/><a:rect l="l" t="t" r="r" b="b"/>"#,
            r#"<a:pathLst>{paths}</a:pathLst></a:custGeom>"#,
            r#"<a:noFill/><a:ln w="{w}" cap="rnd"><a:solidFill><a:srgbClr val="{color}">{alpha}</a:srgbClr></a:solidFill>"#,
            r#"<a:round/></a:ln></p:spPr></p:sp>"#
        ),
        id = id,
        name = build::esc(name),
        x = emu(x0),
        y = emu(y0),
        cx = cx,
        cy = cy,
        paths = paths,
        w = emu(first.brush.width.clamp(0.25, 200.0)),
        color = first.brush.hex(),
        alpha = alpha,
    ))
}

fn add_rel(tx: &mut Tx<'_>, slide: usize, part: &str) -> R<String> {
    let s = tx.pres.slides.get(slide).ok_or(OpError::NoSlide(slide))?;
    let rid = (1..=s.info.rels.len() + 1)
        .map(|n| format!("rId{n}"))
        .find(|id| s.info.rels.iter().all(|r| &r.id != id))
        .unwrap_or_default();
    let target = format!("../ink/{}", part.rsplit('/').next().unwrap_or_default());
    let id = rid.clone();
    tx.fields(slide, |f| {
        f.rels.push(Rel {
            id,
            rel_type: REL_TYPE.into(),
            target,
            external: false,
        });
    })?;
    Ok(rid)
}

/// Add one stroke as a new ink object.
pub fn cmd_add(tx: &mut Tx<'_>, sel: &mut SlideSel, s: &InkStrokeIn, name: &str) -> R<()> {
    let slide = sel.slide;
    if slide >= tx.pres.slides.len() {
        return Err(OpError::NoSlide(slide).into());
    }
    let ok = |v: f64| v.is_finite() && v.abs() < 1e7;
    if s.points.is_empty()
        || s.points.len() > 100_000
        || !s.points.iter().all(|p| ok(p[0]) && ok(p[1]))
        || !ok(s.width)
    {
        return Err(super::EditError::Refused("badInk"));
    }
    let stroke = NewStroke {
        points: s.points.clone(),
        pressure: s.pressure.clone(),
        times: s.times.clone(),
        color: s.color.clone(),
        width: s.width.clamp(0.1, 200.0),
        highlighter: s.highlighter,
    };
    let doc = InkDoc::from_strokes(&[stroke]);
    let traces = doc.traces();
    let (place, size) = Placement::natural(&traces).ok_or(super::EditError::Refused("badInk"))?;
    // Written in slide points, so the frame is the ink's own box.
    let off = (place.min.0 - place.margin.0, place.min.1 - place.margin.1);
    let part = build::unique_part(tx.pres, "/ppt/ink/ink", "xml");
    super::chart::put_part(tx, &part, Some(CONTENT_TYPE), doc.to_bytes())?;
    let rid = add_rel(tx, slide, &part)?;
    let id = next_shape_id(tx.pres, slide);
    let base = if name.trim().is_empty() {
        "Ink"
    } else {
        name.trim()
    };
    let name = format!("{base} {id}");
    let strokes: Vec<Vec<(f64, f64)>> = traces
        .iter()
        .map(|t| t.samples.iter().map(|p| (p.x, p.y)).collect())
        .collect();
    let fallback =
        fallback_xml(id, &name, &strokes, &traces[0]).ok_or(super::EditError::Refused("badInk"))?;
    let xml = bo_pptx::ink::ink_node_xml(
        id,
        &name,
        &rid,
        (emu(off.0), emu(off.1)),
        (emu(size.0), emu(size.1)),
        &fallback,
    );
    let node = build::node(&xml).ok_or(super::EditError::Refused("build"))?;
    let at = tx.pres.slides[slide].data.shapes.len();
    tx.insert_nodes(slide, &[], at, vec![node])?;
    sel.ids.clear();
    Ok(())
}

/// Distance from `p` to the segment `a`–`b`.
fn seg_dist(p: (f64, f64), a: (f64, f64), b: (f64, f64)) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len2 = dx * dx + dy * dy;
    let t = if len2 == 0.0 {
        0.0
    } else {
        (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / len2).clamp(0.0, 1.0)
    };
    (a.0 + t * dx - p.0).hypot(a.1 + t * dy - p.1)
}

/// Distance from `p` to a polyline (a single point when it has one).
fn path_dist(p: (f64, f64), path: &[(f64, f64)]) -> f64 {
    match path {
        [] => f64::INFINITY,
        [a] => (a.0 - p.0).hypot(a.1 - p.1),
        _ => path
            .windows(2)
            .map(|w| seg_dist(p, w[0], w[1]))
            .fold(f64::INFINITY, f64::min),
    }
}

/// Erase along `path`; true when ink changed.
pub fn cmd_erase(
    tx: &mut Tx<'_>,
    sel: &mut SlideSel,
    path: &[[f64; 2]],
    radius: f64,
    mode: EraseMode,
) -> R<()> {
    let slide = sel.slide;
    let path: Vec<(f64, f64)> = path
        .iter()
        .filter(|p| p[0].is_finite() && p[1].is_finite())
        .map(|p| (p[0], p[1]))
        .collect();
    let radius = if radius.is_finite() {
        radius.clamp(0.0, 500.0)
    } else {
        0.0
    };
    if path.is_empty() {
        return Ok(());
    }
    for o in objects(tx.pres, slide).into_iter().rev() {
        let mut doc = o.doc.clone();
        let mut changed = false;
        match mode {
            EraseMode::Stroke => {
                let hit: Vec<usize> = o
                    .traces
                    .iter()
                    .enumerate()
                    .filter(|(_, t)| {
                        let tips = o.slide_tips(t);
                        let line: Vec<(f64, f64)> = tips.iter().map(|p| (p.x, p.y)).collect();
                        let half = tips.iter().map(|p| p.rx.max(p.ry)).fold(0.0, f64::max);
                        tips.iter()
                            .any(|p| path_dist((p.x, p.y), &path) <= radius + p.rx.max(p.ry))
                            || path.iter().any(|&q| path_dist(q, &line) <= radius + half)
                    })
                    .map(|(i, _)| i)
                    .collect();
                if !hit.is_empty() {
                    doc.remove_traces(&hit);
                    changed = true;
                }
            }
            EraseMode::Point => {
                // Later traces first: splitting one shifts the indices after it.
                for (i, t) in o.traces.iter().enumerate().rev() {
                    let tips = o.slide_tips(t);
                    let keep: Vec<bool> = tips
                        .iter()
                        .map(|p| path_dist((p.x, p.y), &path) > radius + p.rx.max(p.ry))
                        .collect();
                    if keep.iter().all(|k| *k) {
                        continue;
                    }
                    let mut runs = Vec::new();
                    let mut start = None;
                    for (j, k) in keep.iter().chain(std::iter::once(&false)).enumerate() {
                        match (k, start) {
                            (true, None) => start = Some(j),
                            (false, Some(s)) => {
                                // A single sample left over is dropped (no dots).
                                if j - s >= 2 {
                                    runs.push(s..j);
                                }
                                start = None;
                            }
                            _ => {}
                        }
                    }
                    doc.split_trace(i, &runs);
                    changed = true;
                }
            }
        }
        if !changed {
            continue;
        }
        let traces = doc.traces();
        if traces.iter().all(|t| t.samples.is_empty()) {
            // Nothing left: the object, its relationship and its part go.
            tx.remove_node(slide, &[o.index])?;
            let rid = o.rid.clone();
            tx.fields(slide, |f| f.rels.retain(|r| r.id != rid))?;
            super::picture::drop_if_unused(tx, &o.part)?;
            continue;
        }
        super::chart::put_part(tx, &o.part, Some(CONTENT_TYPE), doc.to_bytes())?;
        // Keep the remaining ink where it was: same scale, frame shrunk to it.
        let Some((np, nsize)) = Placement::natural(&traces) else {
            continue;
        };
        let s = o.place.scale;
        let off = (
            o.off.0 + (np.min.0 - np.margin.0 - o.place.min.0 + o.place.margin.0) * s.0,
            o.off.1 + (np.min.1 - np.margin.1 - o.place.min.1 + o.place.margin.1) * s.1,
        );
        let ext = (nsize.0 * s.0, nsize.1 * s.1);
        let Some(ShapeNode::Other(mut el)) =
            tx.pres.slides[slide].data.shapes.get(o.index).cloned()
        else {
            continue;
        };
        bo_pptx::ink::set_content_part_frame(
            &mut el,
            (emu(off.0), emu(off.1)),
            (emu(ext.0).max(1), emu(ext.1).max(1)),
        );
        // Fresh fallback for the remaining strokes.
        let fit = Placement::fit(&traces, ext.0, ext.1).unwrap_or(np);
        let strokes: Vec<Vec<(f64, f64)>> = traces
            .iter()
            .map(|t| {
                t.samples
                    .iter()
                    .map(|p| {
                        let (x, y) = fit.map(p.x, p.y);
                        (x + off.0, y + off.1)
                    })
                    .collect()
            })
            .collect();
        if let Some(fb) = fallback_xml(o.id, &o.name, &strokes, &traces[0])
            .and_then(|x| bo_drawingml::dom::parse(x.as_bytes(), "/new").ok())
        {
            for c in &mut el.children {
                if let bo_model::slide::XmlNode::Element(c) = c {
                    if c.local() == "Fallback" {
                        c.children = vec![bo_model::slide::XmlNode::Element(fb.clone())];
                    }
                }
            }
        }
        tx.replace_node(slide, &[o.index], ShapeNode::Other(el))?;
    }
    sel.ids.clear();
    Ok(())
}

impl crate::Engine {
    /// Run an ink command.
    pub fn slide_ink_command(&mut self, cmd: InkCommand) -> Result<SlideEditResult> {
        let _salt = build::salt_names(
            self.slide_collab
                .as_ref()
                .map(|c| build::salt_of(c.client_id())),
        );
        let mut out = Outcome::default();
        let mut backdrop = false;
        {
            let pres = self.pres.as_mut().ok_or(EngineError::NoDocument)?;
            let ed = &mut self.slides;
            ed.clamp(pres);
            ed.text = None;
            ed.table = None;
            let blocked = !matches!(cmd, InkCommand::InkInfo { .. }) && ed.in_any_master_view();
            if blocked {
                out.error = Some("masterView".into());
            } else {
                match cmd {
                    InkCommand::InkInfo { backdrop: b } => backdrop = b,
                    InkCommand::AddInk { stroke, name } => {
                        ed.mutate(pres, &mut out, |tx, sel| cmd_add(tx, sel, &stroke, &name));
                    }
                    InkCommand::EraseInk {
                        points,
                        radius,
                        mode,
                    } => {
                        ed.mutate(pres, &mut out, |tx, sel| {
                            cmd_erase(tx, sel, &points, radius, mode)
                        });
                    }
                }
            }
            ed.clamp(pres);
        }
        let mut r = self.finish_slide_command(out)?;
        let pres = self.pres.as_ref().ok_or(EngineError::NoDocument)?;
        let slide = self.slides.sel.slide;
        let mut ink = ink_out(pres, slide);
        if backdrop {
            let opts = bo_pptx::RenderOptions {
                hide_ink: true,
                ..bo_pptx::RenderOptions::default()
            };
            ink.backdrop =
                bo_pptx::render_with(pres, slide, &mut self.fonts, &mut self.images, &opts).ok();
        }
        r.ink = Some(ink);
        r.collab = self.slide_collab_after_edit();
        Ok(r)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distances() {
        // (5, 3) above the segment (0,0)–(10,0): 3; beyond its end (13, 4): 5.
        assert_eq!(seg_dist((5.0, 3.0), (0.0, 0.0), (10.0, 0.0)), 3.0);
        assert_eq!(seg_dist((13.0, 4.0), (0.0, 0.0), (10.0, 0.0)), 5.0);
        assert_eq!(path_dist((3.0, 4.0), &[(0.0, 0.0)]), 5.0);
    }
}
