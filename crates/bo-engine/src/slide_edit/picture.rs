//! BlueSlide picture tools (CORE-PIC-001): colour adjustments, crop (free, to an
//! aspect ratio, to a shape, fill, fit), compress, change and reset pictures,
//! SVG pictures and their conversion to shapes. Every edit is one undoable
//! transaction; the original picture parts are kept until a compress or change
//! replaces them.
//!
//! Commands (`cmd`): `pictureInfo`, `picturePreviews`, `pictureFx`, `pictureCrop`,
//! `pictureCropAspect`, `pictureCropShape`, `pictureFill`, `pictureFit`,
//! `pictureCompress`, `pictureChange`, `pictureReset`, `pictureToShapes`,
//! `pictureInsertSvg`.

use super::build;
use super::geom::{node_xfrm, place_slide};
use super::ops::{node_at, OpError, SlideOp, Tx};
use super::{
    insert_top, next_shape_id, selected, slide_size, EditError, Outcome, SlideEditResult, SlideSel,
    R,
};
use crate::picture_tools::{self as pt, CompressReport, PictureInfo, EMU_PER_PX};
use crate::{EngineError, Result};
use bo_drawingml::picture::{read_fx, svg_blip_embed, write_fx, SVG_BLIP_EXT};
use bo_drawingml::svg::{parse_svg, SvgDoc};
use bo_model::picture::{aspect_crop, aspect_ratio, fill_crop, fit_size, PictureFx, PCT};
use bo_model::slide::{
    Color, ColorMod, CustomGeometry, Fill, Geometry, GroupShape, Line, NonVisual, Picture,
    Presentation, RawPart, Rel, Shape, ShapeNode, ShapeProps, Xfrm, XmlElement, XmlNode,
};

pub use crate::picture_tools::PictureCommand;

const EMU_PT: f64 = 12_700.0;

/// Paths of the selected pictures.
fn picture_paths(pres: &Presentation, sel: &SlideSel) -> Vec<Vec<usize>> {
    let placed = place_slide(pres, sel.slide);
    selected(&placed, &sel.ids)
        .into_iter()
        .filter(|p| {
            matches!(
                node_at(pres, sel.slide, &p.path),
                Some(ShapeNode::Picture(_))
            )
        })
        .map(|p| p.path.clone())
        .collect()
}

/// Part name of relationship `rid` of a slide.
pub(super) fn rel_part(pres: &Presentation, slide: usize, rid: &str) -> Option<String> {
    let s = pres.slides.get(slide)?;
    s.info
        .rels
        .iter()
        .find(|r| r.id == rid && !r.external)
        .map(|r| bo_opc::resolve_target(&s.info.part_name, &r.target))
}

fn picture_at(pres: &Presentation, slide: usize, path: &[usize]) -> R<Picture> {
    match node_at(pres, slide, path) {
        Some(ShapeNode::Picture(p)) => Ok((**p).clone()),
        _ => Err(EditError::Refused("noPicture")),
    }
}

fn image_of(pres: &Presentation, slide: usize, pic: &Picture) -> Option<(String, Vec<u8>)> {
    let part = rel_part(pres, slide, pic.blip.embed.as_deref()?)?;
    let data = pres.part_data(&part)?.to_vec();
    Some((part, data))
}

/// The picture's frame (inherited from its placeholder when it has none).
fn frame(pres: &Presentation, slide: usize, pic: &Picture) -> Xfrm {
    pic.props
        .xfrm
        .clone()
        .or_else(|| node_xfrm(pres, slide, &ShapeNode::Picture(Box::new(pic.clone()))))
        .unwrap_or_default()
}

fn put(tx: &mut Tx<'_>, slide: usize, path: &[usize], pic: Picture) -> R<()> {
    tx.replace_node(slide, path, ShapeNode::Picture(Box::new(pic)))?;
    Ok(())
}

/// Pixel size of a picture part.
fn pixels(data: &[u8]) -> Option<(f64, f64)> {
    crate::edit::image_size(data).map(|(w, h)| (f64::from(w), f64::from(h)))
}

fn shape_of(pic: &Picture) -> Option<&str> {
    match &pic.props.geometry {
        Some(Geometry::Preset { name, .. }) => Some(name.as_str()),
        _ => None,
    }
}

fn info_of(pres: &Presentation, slide: usize, pic: &Picture) -> Option<PictureInfo> {
    let (part, data) = image_of(pres, slide, pic)?;
    let x = frame(pres, slide, pic);
    let (cx, cy) = x.ext.unwrap_or((0, 0));
    let mut i = pt::info(
        &part,
        &data,
        read_fx(&pic.blip.blip_children),
        pic.blip.src_rect.filter(|c| c.iter().any(|v| *v != 0)),
        shape_of(pic),
        (cx as f64 / EMU_PT, cy as f64 / EMU_PT),
    );
    i.svg = svg_blip_embed(&pic.blip.blip_children).is_some();
    Some(i)
}

/// New crop with the frame resized so the picture keeps its scale.
fn recrop(x: &mut Xfrm, old: Option<[i32; 4]>, new: [i32; 4]) {
    let [ol, ot, or, ob] = old.unwrap_or([0; 4]).map(f64::from);
    let [l, t, r, b] = new.map(f64::from);
    let (cx, cy) = x.ext.unwrap_or((0, 0));
    let (ox, oy) = x.off.unwrap_or((0, 0));
    let uw = cx as f64 / ((PCT - ol - or) / PCT).max(1e-3);
    let uh = cy as f64 / ((PCT - ot - ob) / PCT).max(1e-3);
    x.ext = Some((
        (uw * (PCT - l - r) / PCT).round().max(1.0) as i64,
        (uh * (PCT - t - b) / PCT).round().max(1.0) as i64,
    ));
    x.off = Some((
        ox + (uw * (l - ol) / PCT).round() as i64,
        oy + (uh * (t - ot) / PCT).round() as i64,
    ));
}

/// Is `name` still the target of a relationship anywhere in the deck?
fn part_referenced(pres: &Presentation, name: &str) -> bool {
    let hit = |part: &str, rels: &[Rel]| {
        rels.iter()
            .any(|r| !r.external && bo_opc::resolve_target(part, &r.target) == name)
    };
    let file = name.rsplit('/').next().unwrap_or(name);
    pres.slides
        .iter()
        .any(|s| hit(&s.info.part_name, &s.info.rels))
        || pres
            .layouts
            .iter()
            .any(|s| hit(&s.info.part_name, &s.info.rels))
        || pres
            .masters
            .iter()
            .any(|s| hit(&s.info.part_name, &s.info.rels))
        || pres.parts.iter().any(|p| {
            p.name.ends_with(".rels")
                && std::str::from_utf8(&p.data).is_ok_and(|t| t.contains(file))
        })
}

/// Add a picture part; returns its name.
pub(super) fn add_part(tx: &mut Tx<'_>, data: Vec<u8>, ext: &str, mime: &str) -> R<String> {
    let part = build::unique_part(tx.pres, "/ppt/media/image", ext);
    let at = tx.pres.parts.len();
    tx.op(SlideOp::Parts {
        index: at,
        remove: vec![],
        insert: vec![RawPart {
            name: part.clone(),
            content_type: Some(mime.into()),
            data,
        }],
    })?;
    Ok(part)
}

/// Point relationship `rid` of `slide` at `part`; drops the old part when unused.
pub(super) fn retarget(tx: &mut Tx<'_>, slide: usize, rid: &str, part: &str) -> R<()> {
    let old = rel_part(tx.pres, slide, rid);
    let target = format!("../media/{}", part.rsplit('/').next().unwrap_or_default());
    let rid = rid.to_owned();
    tx.fields(slide, |f| {
        if let Some(r) = f.rels.iter_mut().find(|r| r.id == rid) {
            r.target = target;
        }
    })?;
    if let Some(old) = old {
        drop_if_unused(tx, &old)?;
    }
    Ok(())
}

pub(super) fn drop_if_unused(tx: &mut Tx<'_>, old: &str) -> R<()> {
    if part_referenced(tx.pres, old) {
        return Ok(());
    }
    if let Some(i) = tx.pres.parts.iter().position(|p| p.name == old) {
        let p = tx.pres.parts[i].clone();
        tx.op(SlideOp::Parts {
            index: i,
            remove: vec![p],
            insert: vec![],
        })?;
    }
    Ok(())
}

/// New relationship of `slide` to `part`; returns its id.
fn add_rel(tx: &mut Tx<'_>, slide: usize, part: &str) -> R<String> {
    let s = tx.pres.slides.get(slide).ok_or(OpError::NoSlide(slide))?;
    let rid = (1..=s.info.rels.len() + 1)
        .map(|n| format!("rId{n}"))
        .find(|id| s.info.rels.iter().all(|r| &r.id != id))
        .unwrap_or_default();
    let target = format!("../media/{}", part.rsplit('/').next().unwrap_or_default());
    let id = rid.clone();
    tx.fields(slide, |f| {
        f.rels.push(Rel {
            id,
            rel_type: format!("{}image", bo_pptx::RELATIONSHIP_NS),
            target,
            external: false,
        });
    })?;
    Ok(rid)
}

/// `a:extLst` entry referencing the SVG picture ([MS-ODRAWXML] `asvg:svgBlip`).
fn svg_ext(rid: &str) -> XmlElement {
    let mut blip = XmlElement::new("asvg:svgBlip");
    blip.attrs = vec![
        (
            "xmlns:asvg".into(),
            "http://schemas.microsoft.com/office/drawing/2016/SVG/main".into(),
        ),
        ("r:embed".into(), rid.into()),
    ];
    let mut ext = XmlElement::new("a:ext");
    ext.attrs = vec![("uri".into(), SVG_BLIP_EXT.into())];
    ext.children.push(XmlNode::Element(blip));
    let mut list = XmlElement::new("a:extLst");
    list.children.push(XmlNode::Element(ext));
    list
}

fn solid(c: u32, opacity: f64) -> Fill {
    let mut color = Color::srgb(c);
    if opacity < 1.0 {
        // §20.1.2.3.1 alpha
        color.mods.push(ColorMod {
            name: "alpha".into(),
            val: Some((opacity * PCT).round() as i32),
        });
    }
    Fill::Solid(color)
}

/// Shapes of an SVG document placed in the frame `x` (a group scaled to the frame).
fn svg_group(doc: &SvgDoc, x: &Xfrm, mut next: u32, name: &str) -> Option<ShapeNode> {
    let k = EMU_PER_PX;
    let mut children = Vec::new();
    for s in &doc.shapes {
        let Some([x0, y0, x1, y1]) = s.bounds() else {
            continue;
        };
        let (w, h) = ((x1 - x0).max(1e-3), (y1 - y0).max(1e-3));
        let path = s.geom_path(x0, y0, w, h, k);
        let id = next;
        next += 1;
        children.push(ShapeNode::Shape(Box::new(Shape {
            nv: NonVisual {
                id,
                name: format!("Freeform {id}"),
                ..NonVisual::default()
            },
            props: ShapeProps {
                xfrm: Some(Xfrm {
                    off: Some(((x0 * k).round() as i64, (y0 * k).round() as i64)),
                    ext: Some((
                        (w * k).round().max(1.0) as i64,
                        (h * k).round().max(1.0) as i64,
                    )),
                    ..Xfrm::default()
                }),
                // §20.1.9.8 custGeom with one §20.1.9.15 path.
                geometry: Some(Geometry::Custom(CustomGeometry {
                    paths: vec![path],
                    ..CustomGeometry::default()
                })),
                fill: Some(s.fill.map_or(Fill::None, |c| solid(c, s.opacity))),
                // §20.1.2.2.24 ln
                line: Some(match s.stroke {
                    Some((c, width)) => Line {
                        w: Some((width * k).round() as i64),
                        fill: Some(solid(c, s.opacity)),
                        ..Line::default()
                    },
                    None => Line {
                        fill: Some(Fill::None),
                        ..Line::default()
                    },
                }),
                ..ShapeProps::default()
            },
            ..Shape::default()
        })));
    }
    if children.is_empty() {
        return None;
    }
    let ch = (
        (doc.width * k).round() as i64,
        (doc.height * k).round() as i64,
    );
    Some(ShapeNode::Group(Box::new(GroupShape {
        nv: NonVisual {
            id: next,
            name: name.to_owned(),
            ..NonVisual::default()
        },
        props: ShapeProps {
            // §20.1.7.5 group xfrm: the SVG's coordinate space mapped onto the frame.
            xfrm: Some(Xfrm {
                off: x.off,
                ext: x.ext,
                ch_off: Some((0, 0)),
                ch_ext: Some(ch),
                rot: x.rot,
                flip_h: x.flip_h,
                flip_v: x.flip_v,
            }),
            ..ShapeProps::default()
        },
        children,
        extra: vec![],
    })))
}

/// Insert an SVG picture: the SVG part, a PNG fallback drawn from its shapes and a
/// `p:pic` whose blip embeds the PNG with the SVG in the `asvg:svgBlip` extension.
fn insert_svg(tx: &mut Tx<'_>, sel: &mut SlideSel, svg: Vec<u8>, name: &str) -> R<()> {
    let doc = parse_svg(&svg).ok_or(EditError::Refused("badImage"))?;
    let png = pt::svg_png(&doc, 1024.0).ok_or(EditError::Refused("badImage"))?;
    let (sw, sh) = slide_size(tx.pres);
    let (mut w, mut h) = (doc.width * 0.75, doc.height * 0.75);
    let k = (sw * 0.8 / w).min(sh * 0.8 / h).min(1.0);
    w *= k;
    h *= k;
    let slide = sel.slide;
    let png_part = add_part(tx, png, "png", "image/png")?;
    let svg_part = build::unique_part(tx.pres, "/ppt/media/image", "svg");
    let at = tx.pres.parts.len();
    tx.op(SlideOp::Parts {
        index: at,
        remove: vec![],
        insert: vec![RawPart {
            name: svg_part.clone(),
            content_type: Some("image/svg+xml".into()),
            data: svg,
        }],
    })?;
    let rid_png = add_rel(tx, slide, &png_part)?;
    let rid_svg = add_rel(tx, slide, &svg_part)?;
    let id = next_shape_id(tx.pres, slide);
    let mut node = build::picture(id, &rid_png, name, (sw - w) / 2.0, (sh - h) / 2.0, w, h);
    if let Some(ShapeNode::Picture(p)) = node.as_mut() {
        p.blip.blip_children.push(svg_ext(&rid_svg));
    }
    insert_top(tx, sel, node)
}

impl crate::Engine {
    /// Run a picture tools command on the selected pictures of the current slide.
    pub fn slide_picture_command(&mut self, cmd: PictureCommand) -> Result<SlideEditResult> {
        let mut out = Outcome::default();
        let mut previews = Vec::new();
        let mut report = None;
        let mut fresh: Vec<String> = Vec::new();
        {
            let pres = self.pres.as_mut().ok_or(EngineError::NoDocument)?;
            let ed = &mut self.slides;
            ed.clamp(pres);
            ed.text = None;
            ed.table = None;
            let slide = ed.sel.slide;
            match cmd {
                PictureCommand::PictureInfo => {}
                PictureCommand::PicturePreviews { presets, size } => {
                    let paths = picture_paths(pres, &ed.sel);
                    if let Some(pic) = paths.first().and_then(|p| picture_at(pres, slide, p).ok()) {
                        if let Some((_, data)) = image_of(pres, slide, &pic) {
                            previews = pt::previews(&data, &presets, size.unwrap_or(96));
                        }
                    }
                }
                PictureCommand::PictureFx { fx } => ed.mutate(pres, &mut out, |tx, sel| {
                    let paths = picture_paths(tx.pres, sel);
                    if paths.is_empty() {
                        return Err(EditError::Refused("noPicture"));
                    }
                    for p in paths {
                        let mut pic = picture_at(tx.pres, sel.slide, &p)?;
                        write_fx(&mut pic.blip.blip_children, &fx);
                        put(tx, sel.slide, &p, pic)?;
                    }
                    Ok(())
                }),
                PictureCommand::PictureCrop { crop } => ed.mutate(pres, &mut out, |tx, sel| {
                    let crop = crop.map(|v| v.clamp(-PCT as i32, PCT as i32));
                    if crop[0] + crop[2] >= PCT as i32 || crop[1] + crop[3] >= PCT as i32 {
                        return Err(EditError::Refused("badCrop"));
                    }
                    for p in picture_paths(tx.pres, sel) {
                        let mut pic = picture_at(tx.pres, sel.slide, &p)?;
                        let mut x = frame(tx.pres, sel.slide, &pic);
                        recrop(&mut x, pic.blip.src_rect, crop);
                        pic.props.xfrm = Some(x);
                        pic.blip.src_rect = crop.iter().any(|v| *v != 0).then_some(crop);
                        put(tx, sel.slide, &p, pic)?;
                    }
                    Ok(())
                }),
                PictureCommand::PictureCropAspect { ratio } => {
                    ed.mutate(pres, &mut out, |tx, sel| {
                        let (rw, rh) = aspect_ratio(&ratio).ok_or(EditError::Refused("badRatio"))?;
                        for p in picture_paths(tx.pres, sel) {
                            let mut pic = picture_at(tx.pres, sel.slide, &p)?;
                            let Some((iw, ih)) =
                                image_of(tx.pres, sel.slide, &pic).and_then(|(_, d)| pixels(&d))
                            else {
                                continue;
                            };
                            let crop = aspect_crop(iw, ih, rw, rh);
                            let mut x = frame(tx.pres, sel.slide, &pic);
                            // Keep the width; the height follows the ratio.
                            let (cx, _) = x.ext.unwrap_or((0, 0));
                            x.ext = Some((cx, (cx as f64 * rh / rw).round() as i64));
                            pic.props.xfrm = Some(x);
                            pic.blip.src_rect = crop.iter().any(|v| *v != 0).then_some(crop);
                            put(tx, sel.slide, &p, pic)?;
                        }
                        Ok(())
                    });
                }
                PictureCommand::PictureCropShape { preset } => {
                    ed.mutate(pres, &mut out, |tx, sel| {
                        if bo_drawingml::preset(&preset).is_none() {
                            return Err(EditError::Refused("badShape"));
                        }
                        for p in picture_paths(tx.pres, sel) {
                            let mut pic = picture_at(tx.pres, sel.slide, &p)?;
                            // §20.1.9.18 prstGeom of the picture's spPr clips it.
                            pic.props.geometry = Some(Geometry::Preset {
                                name: preset.clone(),
                                av: vec![],
                            });
                            put(tx, sel.slide, &p, pic)?;
                        }
                        Ok(())
                    });
                }
                PictureCommand::PictureFill | PictureCommand::PictureFit => {
                    let fill = matches!(cmd, PictureCommand::PictureFill);
                    ed.mutate(pres, &mut out, |tx, sel| {
                        for p in picture_paths(tx.pres, sel) {
                            let mut pic = picture_at(tx.pres, sel.slide, &p)?;
                            let Some((iw, ih)) =
                                image_of(tx.pres, sel.slide, &pic).and_then(|(_, d)| pixels(&d))
                            else {
                                continue;
                            };
                            let mut x = frame(tx.pres, sel.slide, &pic);
                            let (cx, cy) = x.ext.unwrap_or((0, 0));
                            let (fw, fh) = (cx as f64, cy as f64);
                            if fill {
                                let c = fill_crop(iw, ih, fw, fh);
                                pic.blip.src_rect = c.iter().any(|v| *v != 0).then_some(c);
                            } else {
                                let (w, h) = fit_size(iw, ih, fw, fh);
                                let (ox, oy) = x.off.unwrap_or((0, 0));
                                x.off = Some((
                                    ox + ((fw - w) / 2.0).round() as i64,
                                    oy + ((fh - h) / 2.0).round() as i64,
                                ));
                                x.ext = Some((w.round() as i64, h.round() as i64));
                                pic.blip.src_rect = None;
                            }
                            pic.props.xfrm = Some(x);
                            put(tx, sel.slide, &p, pic)?;
                        }
                        Ok(())
                    });
                }
                PictureCommand::PictureCompress(spec) => {
                    let mut rep = CompressReport::default();
                    ed.mutate(pres, &mut out, |tx, sel| {
                        let targets: Vec<(usize, Vec<usize>)> = if spec.all {
                            (0..tx.pres.slides.len())
                                .flat_map(|s| all_pictures(tx.pres, s).into_iter().map(move |p| (s, p)))
                                .collect()
                        } else {
                            picture_paths(tx.pres, sel)
                                .into_iter()
                                .map(|p| (sel.slide, p))
                                .collect()
                        };
                        if targets.is_empty() {
                            return Err(EditError::Refused("noPicture"));
                        }
                        let opts = spec.options();
                        for (s, p) in targets {
                            let mut pic = picture_at(tx.pres, s, &p)?;
                            let Some(rid) = pic.blip.embed.clone() else { continue };
                            let Some((_, data)) = image_of(tx.pres, s, &pic) else { continue };
                            let x = frame(tx.pres, s, &pic);
                            let (cx, cy) = x.ext.unwrap_or((0, 0));
                            let Some(c) = bo_raster::picfx::compress(
                                &data,
                                pic.blip.src_rect,
                                cx as f64 / EMU_PT,
                                cy as f64 / EMU_PT,
                                &opts,
                            ) else {
                                continue;
                            };
                            rep.pictures += 1;
                            rep.before += data.len();
                            rep.after += c.bytes.len();
                            let (ext, mime) = if c.jpeg {
                                ("jpeg", "image/jpeg")
                            } else {
                                ("png", "image/png")
                            };
                            let part = add_part(tx, c.bytes, ext, mime)?;
                            fresh.push(part.clone());
                            retarget(tx, s, &rid, &part)?;
                            if c.crop_applied {
                                pic.blip.src_rect = None;
                            }
                            put(tx, s, &p, pic)?;
                        }
                        Ok(())
                    });
                    report = Some(rep);
                }
                PictureCommand::PictureChange { data, name } => {
                    ed.mutate(pres, &mut out, |tx, sel| {
                        let bytes = pt::decode_data(&data).ok_or(EditError::Refused("badImage"))?;
                        let paths = picture_paths(tx.pres, sel);
                        let Some(p) = paths.first() else {
                            return Err(EditError::Refused("noPicture"));
                        };
                        let mut pic = picture_at(tx.pres, sel.slide, p)?;
                        let rid = pic.blip.embed.clone().ok_or(EditError::Refused("noPicture"))?;
                        let svg = pt::is_svg(&bytes).then(|| bytes.clone());
                        let (img, ext, mime) = match &svg {
                            Some(s) => {
                                let doc = parse_svg(s).ok_or(EditError::Refused("badImage"))?;
                                (
                                    pt::svg_png(&doc, 1024.0).ok_or(EditError::Refused("badImage"))?,
                                    "png",
                                    "image/png",
                                )
                            }
                            None => pt::drawable(bytes).ok_or(EditError::Refused("badImage"))?,
                        };
                        let (iw, ih) = pixels(&img).ok_or(EditError::Refused("badImage"))?;
                        let part = add_part(tx, img, ext, mime)?;
                        fresh.push(part.clone());
                        // The relationship may be shared: give the picture its own.
                        let new_rid = add_rel(tx, sel.slide, &part)?;
                        pic.blip.embed = Some(new_rid);
                        // Drop a previous SVG version, add the new one.
                        pic.blip.blip_children.retain(|c| svg_blip_embed(std::slice::from_ref(c)).is_none());
                        if let Some(s) = svg {
                            let svg_part = build::unique_part(tx.pres, "/ppt/media/image", "svg");
                            let at = tx.pres.parts.len();
                            tx.op(SlideOp::Parts {
                                index: at,
                                remove: vec![],
                                insert: vec![RawPart {
                                    name: svg_part.clone(),
                                    content_type: Some("image/svg+xml".into()),
                                    data: s,
                                }],
                            })?;
                            let r = add_rel(tx, sel.slide, &svg_part)?;
                            pic.blip.blip_children.push(svg_ext(&r));
                        }
                        // Same frame, no distortion: crop the new picture to fill it.
                        let x = frame(tx.pres, sel.slide, &pic);
                        let (cx, cy) = x.ext.unwrap_or((0, 0));
                        let c = fill_crop(iw, ih, cx as f64, cy as f64);
                        pic.blip.src_rect = c.iter().any(|v| *v != 0).then_some(c);
                        if !name.is_empty() {
                            pic.nv.descr = Some(name.clone());
                        }
                        put(tx, sel.slide, p, pic)?;
                        // The old relationship when nothing uses it any more.
                        unlink_if_unused(tx, sel.slide, &rid)?;
                        Ok(())
                    });
                }
                PictureCommand::PictureReset { size } => ed.mutate(pres, &mut out, |tx, sel| {
                    for p in picture_paths(tx.pres, sel) {
                        let mut pic = picture_at(tx.pres, sel.slide, &p)?;
                        write_fx(&mut pic.blip.blip_children, &PictureFx::default());
                        if size {
                            let px = image_of(tx.pres, sel.slide, &pic).and_then(|(_, d)| pixels(&d));
                            let mut x = frame(tx.pres, sel.slide, &pic);
                            if let Some((iw, ih)) = px {
                                // 96 pixels per inch.
                                x.ext = Some(((iw * EMU_PER_PX).round() as i64, (ih * EMU_PER_PX).round() as i64));
                            }
                            pic.props.xfrm = Some(x);
                        } else if pic.blip.src_rect.is_some() {
                            let mut x = frame(tx.pres, sel.slide, &pic);
                            recrop(&mut x, pic.blip.src_rect, [0; 4]);
                            pic.props.xfrm = Some(x);
                        }
                        pic.blip.src_rect = None;
                        if matches!(&pic.props.geometry, Some(Geometry::Preset { name, .. }) if name != "rect") {
                            pic.props.geometry = Some(Geometry::Preset {
                                name: "rect".into(),
                                av: vec![],
                            });
                        }
                        put(tx, sel.slide, &p, pic)?;
                    }
                    Ok(())
                }),
                PictureCommand::PictureToShapes => ed.mutate(pres, &mut out, |tx, sel| {
                    let paths = picture_paths(tx.pres, sel);
                    let Some(p) = paths.first() else {
                        return Err(EditError::Refused("noPicture"));
                    };
                    let pic = picture_at(tx.pres, sel.slide, p)?;
                    let rid = svg_blip_embed(&pic.blip.blip_children).ok_or(EditError::Refused("notSvg"))?;
                    let part = rel_part(tx.pres, sel.slide, &rid).ok_or(EditError::Refused("notSvg"))?;
                    let doc = tx
                        .pres
                        .part_data(&part)
                        .and_then(parse_svg)
                        .ok_or(EditError::Refused("notSvg"))?;
                    let x = frame(tx.pres, sel.slide, &pic);
                    let next = next_shape_id(tx.pres, sel.slide);
                    let group = svg_group(&doc, &x, next, &pic.nv.name).ok_or(EditError::Refused("notSvg"))?;
                    let gid = group.nv().map_or(0, |nv| nv.id);
                    tx.replace_node(sel.slide, p, group)?;
                    sel.ids = vec![gid];
                    Ok(())
                }),
                PictureCommand::PictureInsertSvg { data, name } => {
                    ed.mutate(pres, &mut out, |tx, sel| {
                        let bytes = pt::decode_data(&data).ok_or(EditError::Refused("badImage"))?;
                        insert_svg(tx, sel, bytes, if name.is_empty() { "SVG" } else { &name })
                    });
                }
            }
        }
        // New picture parts may reuse a name dropped by an undo: draw them afresh.
        for f in &fresh {
            self.images.remove(f);
        }
        let mut r = self.finish_slide_command(out)?;
        let pres = self.pres.as_ref().ok_or(EngineError::NoDocument)?;
        let slide = self.slides.sel.slide;
        let mut info = picture_paths(pres, &self.slides.sel)
            .first()
            .and_then(|p| picture_at(pres, slide, p).ok())
            .and_then(|pic| info_of(pres, slide, &pic));
        if let Some(i) = info.as_mut() {
            i.previews = previews;
            i.compressed = report;
        } else if report.is_some() {
            info = Some(PictureInfo {
                compressed: report,
                ..PictureInfo::default()
            });
        }
        r.picture = info.map(Box::new);
        r.collab = self.slide_collab_after_edit();
        Ok(r)
    }
}

/// Remove relationship `rid` of `slide` when no picture of the slide uses it.
fn unlink_if_unused(tx: &mut Tx<'_>, slide: usize, rid: &str) -> R<()> {
    let Some(s) = tx.pres.slides.get(slide) else {
        return Ok(());
    };
    let mut used = false;
    bo_pptx::visit(&s.data.shapes, &mut |n| {
        if let ShapeNode::Picture(p) = n {
            used |= p.blip.embed.as_deref() == Some(rid)
                || svg_blip_embed(&p.blip.blip_children).as_deref() == Some(rid);
        }
        if let ShapeNode::Shape(sh) = n {
            if let Some(Fill::Picture(b)) = &sh.props.fill {
                used |= b.embed.as_deref() == Some(rid);
            }
        }
    });
    // Other markup of the slide (background, opaque shapes, media) may refer to it too.
    let quoted = format!("{rid:?}");
    let mut xml_uses = format!("{:?}{:?}", s.data.background, s.data.extra).contains(&quoted);
    bo_pptx::visit(&s.data.shapes, &mut |n| {
        if matches!(n, ShapeNode::Other(_) | ShapeNode::Frame(_))
            || matches!(n, ShapeNode::Picture(p) if !p.nv.nv_extra.is_empty())
        {
            xml_uses |= format!("{n:?}").contains(&quoted);
        }
    });
    if used || xml_uses {
        return Ok(());
    }
    let old = rel_part(tx.pres, slide, rid);
    let rid = rid.to_owned();
    tx.fields(slide, |f| f.rels.retain(|r| r.id != rid))?;
    if let Some(old) = old {
        drop_if_unused(tx, &old)?;
    }
    Ok(())
}

/// Index paths of every picture of a slide (groups included).
fn all_pictures(pres: &Presentation, slide: usize) -> Vec<Vec<usize>> {
    fn walk(nodes: &[ShapeNode], path: &mut Vec<usize>, out: &mut Vec<Vec<usize>>) {
        for (i, n) in nodes.iter().enumerate() {
            path.push(i);
            match n {
                ShapeNode::Picture(_) => out.push(path.clone()),
                ShapeNode::Group(g) => walk(&g.children, path, out),
                _ => {}
            }
            path.pop();
        }
    }
    let mut out = Vec::new();
    if let Some(s) = pres.slides.get(slide) {
        walk(&s.data.shapes, &mut Vec::new(), &mut out);
    }
    out
}
