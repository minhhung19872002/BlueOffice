//! 3D models on slides (CORE-3DMODEL-001): insert a glTF / GLB / OBJ / STL file (stored as
//! GLB, [MS-ODRAWXML] `am3d:model3d`), turn, zoom and pan its view, preset views, and keep
//! the rendered picture (`am3d:raster` and the `mc:Fallback` picture) in step with the view
//! so other software and every export show the same picture.

use super::build;
use super::geom::{find, place_slide};
use super::ops::{node_at, OpError, SlideOp, Tx};
use super::{
    insert_top, next_shape_id, slide_size, EditError, Outcome, SlideEditResult, SlideSel, R,
};
use crate::{EngineError, Result};
use bo_model::slide::{GraphicFrame, Presentation, RawPart, Rel, ShapeNode};
use bo_model3d::view::View3d;
use bo_model3d::Scene;
use bo_pptx::model3d as m3d;
use serde::{Deserialize, Serialize};

const EMU_PT: f64 = 12_700.0;
/// Resolution of the rendered picture.
const PICTURE_DPI: f64 = 192.0;
/// Longest side of the rendered picture (pixels).
const PICTURE_MAX: u32 = 1600;

/// Commands of the 3D model tools (JSON `cmd` starting with `model3d`).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "cmd", rename_all = "camelCase")]
pub enum Model3dCommand {
    /// The selected model → `model3d`.
    Model3dInfo,
    /// Geometry of the selected model for the interactive viewer → `model3dMesh`.
    Model3dMesh,
    /// Set the whole view (the viewer's result after a drag).
    Model3dView {
        /// New view.
        view: View3d,
    },
    /// Turn by `dx` degrees about the vertical axis and `dy` about the horizontal one.
    Model3dRotate {
        /// Degrees.
        dx: f64,
        /// Degrees.
        dy: f64,
    },
    /// A preset view (`front`, `back`, `left`, `right`, `top`, `bottom`, `aboveFrontLeft`…).
    Model3dPreset {
        /// Preset name.
        preset: String,
    },
    /// Back to the view of a new model (front, whole model in the frame).
    Model3dReset,
    /// Zoom by `factor` within the frame.
    Model3dZoom {
        /// > 1 larger.
        factor: f64,
    },
    /// Pan within the frame by (`dx`, `dy`) points.
    Model3dPan {
        /// Points.
        dx: f64,
        /// Points.
        dy: f64,
    },
    /// Render the picture again (after a resize).
    Model3dRefresh,
    /// Alternative text of the selected model (§20.1.2.2.8 cNvPr `descr`; empty removes it).
    Model3dAltText {
        /// Text.
        text: String,
    },
}

impl Model3dCommand {
    /// Parse a JSON command whose `cmd` starts with `model3d`.
    pub fn parse(v: &serde_json::Value) -> Option<Model3dCommand> {
        let cmd = v.get("cmd")?.as_str()?;
        if !cmd.starts_with("model3d") {
            return None;
        }
        serde_json::from_value(v.clone()).ok()
    }
}

/// The selected model.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Model3dOut {
    /// Shape id.
    pub id: u32,
    /// Shape name.
    pub name: String,
    /// Alternative text.
    pub descr: Option<String>,
    /// Frame `[x, y, w, h]` in slide points.
    pub rect: [f64; 4],
    /// Frame size in EMU (the units of the view).
    pub frame_emu: [f64; 2],
    /// GLB part (bytes via `EngineHost.getMedia`).
    pub part: Option<String>,
    /// The view.
    pub view: View3d,
    /// Triangles of the model (0 when the part cannot be read).
    pub triangles: usize,
}

/// Geometry for the web viewer: model-space arrays, little-endian, base64.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MeshOut {
    /// `f32 × 3` per vertex.
    pub positions: String,
    /// `f32 × 3` per vertex.
    pub normals: String,
    /// `u8 × 4` per vertex (sRGB; base colour × vertex colour × texture at the vertex).
    pub colors: String,
    /// `u32 × 3` per triangle.
    pub indices: String,
    /// Vertex count.
    pub vertices: usize,
    /// Triangle count.
    pub triangles: usize,
}

fn b64(bytes: &[u8]) -> String {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

fn slide_rel_part(pres: &Presentation, slide: usize, rid: &str) -> Option<String> {
    let s = pres.slides.get(slide)?;
    let rel = s.info.rels.iter().find(|r| r.id == rid)?;
    super::clip::rel_part(&s.info.part_name, rel)
}

/// GLB part of a model frame.
fn glb_part(pres: &Presentation, slide: usize, f: &GraphicFrame) -> Option<String> {
    slide_rel_part(pres, slide, &m3d::model_rid(f)?)
}

/// The model's scene, read from its GLB part.
fn scene_of(pres: &Presentation, slide: usize, f: &GraphicFrame) -> Option<Scene> {
    let part = glb_part(pres, slide, f)?;
    bo_model3d::gltf::parse_glb(pres.part_data(&part)?).ok()
}

/// The model frames among the selected shapes: (path, frame).
fn selected_models(pres: &Presentation, sel: &SlideSel) -> Vec<(Vec<usize>, GraphicFrame)> {
    let placed = place_slide(pres, sel.slide);
    sel.ids
        .iter()
        .filter_map(|id| find(&placed, *id))
        .filter_map(|p| match node_at(pres, sel.slide, &p.path) {
            Some(ShapeNode::Frame(f)) if m3d::is_model3d(f) => {
                Some((p.path.clone(), (**f).clone()))
            }
            _ => None,
        })
        .collect()
}

/// Info of model `id` on `slide`.
pub fn model_out(pres: &Presentation, slide: usize, id: u32) -> Option<Model3dOut> {
    let placed = place_slide(pres, slide);
    let p = find(&placed, id)?;
    let Some(ShapeNode::Frame(f)) = node_at(pres, slide, &p.path) else {
        return None;
    };
    if !m3d::is_model3d(f) {
        return None;
    }
    let r = p.b.bounds();
    let (cx, cy) = f.xfrm.ext.unwrap_or((0, 0));
    Some(Model3dOut {
        id,
        name: f.nv.name.clone(),
        descr: f.nv.descr.clone(),
        rect: [r.x, r.y, r.w, r.h],
        frame_emu: [cx as f64, cy as f64],
        part: glb_part(pres, slide, f),
        view: m3d::read_view(f),
        triangles: scene_of(pres, slide, f).map_or(0, |s| s.triangles()),
    })
}

/// Is the node a 3D model?
pub fn is_model(n: &ShapeNode) -> bool {
    m3d::node_model(n).is_some()
}

/// The picture of a view, PNG.
fn picture(scene: &Scene, view: &View3d, w: f64, h: f64) -> Vec<u8> {
    let (pw, ph) = bo_model3d::raster::picture_size(w, h, PICTURE_DPI, PICTURE_MAX);
    bo_model3d::raster::render_png(scene, view, w, h, pw, ph)
}

/// Write `view` into the model at `path`, render its picture again and point the raster /
/// fallback relationships at the new picture (a new part: pictures are cached by name).
fn apply_view(tx: &mut Tx<'_>, slide: usize, path: &[usize], view: &View3d) -> R<()> {
    let Some(ShapeNode::Frame(f)) = node_at(tx.pres, slide, path) else {
        return Err(EditError::Refused("noModel"));
    };
    let mut f = (**f).clone();
    let scene = scene_of(tx.pres, slide, &f).ok_or(EditError::Refused("badModel"))?;
    m3d::write_view(&mut f, view);
    let (w, h) = f.xfrm.ext.unwrap_or((0, 0));
    let png = picture(&scene, view, w as f64, h as f64);
    if png.is_empty() {
        return Err(EditError::Refused("badModel"));
    }
    let rids = m3d::picture_rids(&f);
    tx.replace_node(slide, path, ShapeNode::Frame(Box::new(f)))?;
    if !rids.is_empty() {
        let part = view_part(tx, png)?;
        for rid in rids {
            super::picture::retarget(tx, slide, &rid, &part)?;
        }
    }
    Ok(())
}

/// Add a rendered picture under a name derived from its bytes: hosts cache pictures by
/// part name, so a name is never reused for other bytes (a dropped picture's name may come
/// back only with the same picture).
fn view_part(tx: &mut Tx<'_>, png: Vec<u8>) -> R<String> {
    let hash = png.iter().fold(0xcbf2_9ce4_8422_2325u64, |h, &b| {
        (h ^ u64::from(b)).wrapping_mul(0x100_0000_01b3)
    });
    let name = format!("/ppt/media/model3d-view-{hash:016x}.png");
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
                content_type: Some("image/png".into()),
                data: png,
            }],
        })?;
    }
    Ok(name)
}

/// Change the view of every selected model with `f`.
fn edit_views(
    tx: &mut Tx<'_>,
    sel: &SlideSel,
    mut f: impl FnMut(&mut View3d, (f64, f64), &Scene),
) -> R<()> {
    let models = selected_models(tx.pres, sel);
    if models.is_empty() {
        return Err(EditError::Refused("noModel"));
    }
    for (path, frame) in models {
        let scene = scene_of(tx.pres, sel.slide, &frame).ok_or(EditError::Refused("badModel"))?;
        let mut v = m3d::read_view(&frame);
        let (w, h) = frame.xfrm.ext.unwrap_or((0, 0));
        f(&mut v, (w as f64, h as f64), &scene);
        apply_view(tx, sel.slide, &path, &v)?;
    }
    Ok(())
}

/// The view of a new model in a frame `w × h` EMU: front, centred, the whole model shown.
fn reset_view(scene: &Scene, w: f64, h: f64) -> View3d {
    let (lo, hi) = scene.bounds().unwrap_or(([-0.5; 3], [0.5; 3]));
    let mut v = View3d::fit(lo, hi);
    v.frame_for(scene, w, h);
    v
}

/// Render again the pictures of selected models whose frame changed shape (after a
/// resize): the picture keeps the frame's aspect.
pub fn refresh_resized(tx: &mut Tx<'_>, sel: &SlideSel) -> R<()> {
    for (path, frame) in selected_models(tx.pres, sel) {
        let (w, h) = frame.xfrm.ext.unwrap_or((0, 0));
        let Some(rid) = m3d::picture_rids(&frame).into_iter().next() else {
            continue;
        };
        let size = slide_rel_part(tx.pres, sel.slide, &rid)
            .and_then(|p| tx.pres.part_data(&p).and_then(crate::edit::image_size));
        let stale = size.is_none_or(|(pw, ph)| {
            let want =
                bo_model3d::raster::picture_size(w as f64, h as f64, PICTURE_DPI, PICTURE_MAX);
            (f64::from(pw) - f64::from(want.0)).abs() > 1.5
                || (f64::from(ph) - f64::from(want.1)).abs() > 1.5
        });
        if stale && scene_of(tx.pres, sel.slide, &frame).is_some() {
            apply_view(tx, sel.slide, &path, &m3d::read_view(&frame))?;
        }
    }
    Ok(())
}

fn new_rid(rels: &[Rel], taken: &[String]) -> String {
    (1..=rels.len() + taken.len() + 1)
        .map(|n| format!("rId{n}"))
        .find(|id| rels.iter().all(|r| &r.id != id) && !taken.contains(id))
        .unwrap_or_default()
}

/// Insert model file `bytes` (`name`: `.glb`, `.gltf`, `.obj`, `.stl`) on the current
/// slide, centred, half the slide at most.
pub fn cmd_insert_model(tx: &mut Tx<'_>, sel: &mut SlideSel, bytes: &[u8], name: &str) -> R<()> {
    let (scene, glb) = bo_model3d::load(bytes, name).map_err(|_| EditError::Refused("badModel"))?;
    if scene.triangles() == 0 {
        return Err(EditError::Refused("badModel"));
    }
    let slide = sel.slide;
    let (sw, sh) = slide_size(tx.pres);
    let (lo, hi) = scene.bounds().ok_or(EditError::Refused("badModel"))?;
    let mut view = View3d::fit(lo, hi);
    let (w, h) = view.frame_for(&scene, sw * 0.5 * EMU_PT, sh * 0.6 * EMU_PT);
    let png = picture(&scene, &view, w, h);
    let off = (
        ((sw * EMU_PT - w) / 2.0).round() as i64,
        ((sh * EMU_PT - h) / 2.0).round() as i64,
    );
    let model_part = build::unique_part(tx.pres, "/ppt/media/model3d", "glb");
    let s = tx.pres.slides.get(slide).ok_or(OpError::NoSlide(slide))?;
    let model_rid = new_rid(&s.info.rels, &[]);
    let image_rid = new_rid(&s.info.rels, std::slice::from_ref(&model_rid));
    let id = next_shape_id(tx.pres, slide);
    let stem = name
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(name)
        .rsplit_once('.')
        .map_or(name, |(s, _)| s);
    let xml = m3d::node_xml(
        id,
        &format!("3D Model {id}"),
        stem,
        off,
        (w.round() as i64, h.round() as i64),
        &model_rid,
        &image_rid,
        &view,
    );
    let node = build::node(&xml);
    let at = tx.pres.parts.len();
    tx.op(SlideOp::Parts {
        index: at,
        remove: vec![],
        insert: vec![RawPart {
            name: model_part.clone(),
            content_type: Some(m3d::CT_GLB.into()),
            data: glb,
        }],
    })?;
    let image_part = view_part(tx, png)?;
    let file = |p: &str| format!("../media/{}", p.rsplit('/').next().unwrap_or_default());
    tx.fields(slide, |f| {
        f.rels.push(Rel {
            id: model_rid.clone(),
            rel_type: m3d::REL_MODEL3D.into(),
            target: file(&model_part),
            external: false,
        });
        f.rels.push(Rel {
            id: image_rid.clone(),
            rel_type: format!("{}image", bo_pptx::RELATIONSHIP_NS),
            target: file(&image_part),
            external: false,
        });
    })?;
    insert_top(tx, sel, node)
}

/// Geometry of a scene for the viewer.
pub fn mesh_out(scene: &Scene) -> MeshOut {
    let mut pos = Vec::new();
    let mut nor = Vec::new();
    let mut col = Vec::new();
    let mut idx = Vec::new();
    let mut base = 0u32;
    let to8 = |c: f32| {
        let c = c.clamp(0.0, 1.0);
        let s = if c <= 0.003_130_8 {
            c * 12.92
        } else {
            1.055 * c.powf(1.0 / 2.4) - 0.055
        };
        (s * 255.0).round() as u8
    };
    let from8 = |c: f32| {
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    for p in &scene.prims {
        let m = scene.material(p);
        let tex = m.texture.and_then(|t| scene.textures.get(t));
        for (i, v) in p.positions.iter().enumerate() {
            pos.extend(v.iter().flat_map(|x| x.to_le_bytes()));
            let n = p.normals.get(i).copied().unwrap_or([0.0, 0.0, 1.0]);
            nor.extend(n.iter().flat_map(|x| x.to_le_bytes()));
            let vc = p.colors.get(i).copied().unwrap_or([1.0; 4]);
            let mut c = [
                m.base[0] * vc[0],
                m.base[1] * vc[1],
                m.base[2] * vc[2],
                m.base[3] * vc[3],
            ];
            if let (Some(t), Some(uv)) = (tex, p.uvs.get(i)) {
                let s = t.sample(uv[0], uv[1]);
                for k in 0..3 {
                    c[k] *= from8(s[k]);
                }
            }
            col.extend_from_slice(&[
                to8(c[0]),
                to8(c[1]),
                to8(c[2]),
                (c[3].clamp(0.0, 1.0) * 255.0) as u8,
            ]);
        }
        idx.extend(p.indices.iter().flat_map(|i| (i + base).to_le_bytes()));
        base += p.positions.len() as u32;
    }
    MeshOut {
        positions: b64(&pos),
        normals: b64(&nor),
        colors: b64(&col),
        indices: b64(&idx),
        vertices: base as usize,
        triangles: scene.triangles(),
    }
}

impl crate::Engine {
    /// Run a 3D model command on the selected model(s).
    pub fn slide_model3d_command(&mut self, cmd: Model3dCommand) -> Result<SlideEditResult> {
        let _salt = build::salt_names(
            self.slide_collab
                .as_ref()
                .map(|c| build::salt_of(c.client_id())),
        );
        let mut out = Outcome::default();
        let mut mesh = None;
        {
            let pres = self.pres.as_mut().ok_or(EngineError::NoDocument)?;
            let ed = &mut self.slides;
            ed.clamp(pres);
            ed.text = None;
            match cmd {
                Model3dCommand::Model3dInfo => {}
                Model3dCommand::Model3dMesh => {
                    let sel = ed.sel.clone();
                    mesh = selected_models(pres, &sel)
                        .first()
                        .and_then(|(_, f)| scene_of(pres, sel.slide, f))
                        .map(|s| mesh_out(&s));
                    if mesh.is_none() {
                        out.error = Some("noModel".into());
                    }
                }
                Model3dCommand::Model3dView { view } => ed.mutate(pres, &mut out, |tx, sel| {
                    let ok = view
                        .camera_pos
                        .iter()
                        .chain(&view.look_at)
                        .chain(&view.rot)
                        .all(|x| x.is_finite())
                        && view.meter_per_unit.is_finite()
                        && view.meter_per_unit > 0.0;
                    if !ok {
                        return Err(EditError::Refused("badView"));
                    }
                    edit_views(tx, sel, |v, _, _| *v = view.clone())
                }),
                Model3dCommand::Model3dRotate { dx, dy } => ed.mutate(pres, &mut out, |tx, sel| {
                    let (dx, dy) = (finite(dx), finite(dy));
                    edit_views(tx, sel, |v, _, _| v.orbit(dx, dy))
                }),
                Model3dCommand::Model3dPreset { preset } => ed.mutate(pres, &mut out, |tx, sel| {
                    let rot = View3d::preset(&preset).ok_or(EditError::Refused("badPreset"))?;
                    edit_views(tx, sel, |v, _, _| {
                        v.rot = rot.map(|d| d * bo_model3d::view::ANGLE);
                    })
                }),
                Model3dCommand::Model3dReset => ed.mutate(pres, &mut out, |tx, sel| {
                    edit_views(tx, sel, |v, (w, h), scene| *v = reset_view(scene, w, h))
                }),
                Model3dCommand::Model3dZoom { factor } => ed.mutate(pres, &mut out, |tx, sel| {
                    let f = if factor.is_finite() {
                        factor.clamp(0.05, 20.0)
                    } else {
                        1.0
                    };
                    edit_views(tx, sel, |v, frame, _| v.zoom(f, frame))
                }),
                Model3dCommand::Model3dPan { dx, dy } => ed.mutate(pres, &mut out, |tx, sel| {
                    let (dx, dy) = (finite(dx) * EMU_PT, finite(dy) * EMU_PT);
                    edit_views(tx, sel, |v, frame, _| v.pan(dx, dy, frame))
                }),
                Model3dCommand::Model3dRefresh => {
                    ed.mutate(pres, &mut out, |tx, sel| edit_views(tx, sel, |_, _, _| {}));
                }
                Model3dCommand::Model3dAltText { text } => ed.mutate(pres, &mut out, |tx, sel| {
                    let models = selected_models(tx.pres, sel);
                    if models.is_empty() {
                        return Err(EditError::Refused("noModel"));
                    }
                    let t = text.trim();
                    for (path, mut f) in models {
                        f.nv.descr = (!t.is_empty()).then(|| t.to_owned());
                        tx.replace_node(sel.slide, &path, ShapeNode::Frame(Box::new(f)))?;
                    }
                    Ok(())
                }),
            }
            ed.clamp(pres);
        }
        let mut r = self.finish_slide_command(out)?;
        r.model3d_mesh = mesh;
        r.collab = self.slide_collab_after_edit();
        Ok(r)
    }
}

fn finite(x: f64) -> f64 {
    if x.is_finite() {
        x.clamp(-1e6, 1e6)
    } else {
        0.0
    }
}
