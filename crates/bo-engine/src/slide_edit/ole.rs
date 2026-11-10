//! BlueSlide embedded and linked workbooks (CORE-EMBED-001): the [`OleCommand`]s on
//! slides. An object is a graphic frame (§19.3.1.21) holding a `p:oleObj` (ISO/IEC
//! 29500-1, §19.3.2.4) with `p:embed` (§19.3.2.2, the package relationship `r:id`) or
//! `p:link` (§19.3.2.3, an external relationship) and the preview picture `p:pic`.
//! Objects read from files stay byte-for-byte until edited.

use super::build;
use super::geom::{find, place_slide};
use super::ops::{node_at, SlideOp, Tx};
use super::{insert_top, next_shape_id, slide_size, EditError, Outcome, SlideSel, R};
use crate::ole::{self, Built, LinkOut, OleCommand, OleInfo};
use crate::{EngineError, Result};
use bo_model::ole::{CT_XLSX, PROG_ID_XLSX, REL_OLE_OBJECT, REL_PACKAGE};
use bo_model::slide::{GraphicContent, Presentation, RawPart, Rel, ShapeNode, XmlElement};
use bo_pptx::ole::{ole_ref, OleRef, URI_OLE};

const EMU_PT: f64 = 12_700.0;

fn emu(pt: f64) -> i64 {
    (pt * EMU_PT).round() as i64
}

fn esc(s: &str) -> String {
    bo_opc::xml::escape(s)
}

/// Is this node an OLE object frame?
pub fn is_ole(n: &ShapeNode) -> bool {
    matches!(n, ShapeNode::Frame(f) if f.uri == URI_OLE)
}

/// An object frame on a slide: its path, `graphicData`, frame size (points) and object data.
struct Found {
    path: Vec<usize>,
    gd: XmlElement,
    size: (f64, f64),
    r: OleRef,
}

fn found(pres: &Presentation, slide: usize, id: u32) -> Option<Found> {
    let placed = place_slide(pres, slide);
    let p = find(&placed, id)?;
    let ShapeNode::Frame(f) = node_at(pres, slide, &p.path)? else {
        return None;
    };
    if f.uri != URI_OLE {
        return None;
    }
    let GraphicContent::Other(gd) = &f.content else {
        return None;
    };
    let (cx, cy) = f.xfrm.ext.unwrap_or((0, 0));
    Some(Found {
        path: p.path.clone(),
        gd: gd.clone(),
        size: (cx as f64 / EMU_PT, cy as f64 / EMU_PT),
        r: ole_ref(gd)?,
    })
}

fn rel<'a>(pres: &'a Presentation, slide: usize, rid: &str) -> Option<&'a Rel> {
    pres.slides
        .get(slide)?
        .info
        .rels
        .iter()
        .find(|r| r.id == rid)
}

/// Embedded part of an object.
fn package_part(pres: &Presentation, slide: usize, r: &OleRef) -> Option<String> {
    let s = pres.slides.get(slide)?;
    let rel = rel(pres, slide, &r.rid).filter(|x| !x.external)?;
    Some(bo_opc::resolve_target(&s.info.part_name, &rel.target))
}

fn new_rid(pres: &Presentation, slide: usize, also: &[String]) -> String {
    let rels = pres
        .slides
        .get(slide)
        .map(|s| s.info.rels.as_slice())
        .unwrap_or_default();
    (1..=u32::MAX)
        .map(|n| format!("rId{n}"))
        .find(|id| rels.iter().all(|r| &r.id != id) && !also.contains(id))
        .unwrap_or_default()
}

fn add_part(tx: &mut Tx<'_>, name: &str, ct: &str, data: Vec<u8>) -> R<()> {
    let at = tx.pres.parts.len();
    tx.op(SlideOp::Parts {
        index: at,
        remove: vec![],
        insert: vec![RawPart {
            name: name.to_owned(),
            content_type: Some(ct.to_owned()),
            data,
        }],
    })?;
    Ok(())
}

/// `p:pic` preview of an object (§19.3.1.37) filling the frame.
fn pic_xml(rid: &str, cx: i64, cy: i64) -> String {
    format!(
        concat!(
            r#"<p:pic><p:nvPicPr><p:cNvPr id="0" name=""/><p:cNvPicPr/><p:nvPr/></p:nvPicPr>"#,
            r#"<p:blipFill><a:blip r:embed="{rid}"/><a:stretch><a:fillRect/></a:stretch></p:blipFill>"#,
            r#"<p:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="{cx}" cy="{cy}"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr></p:pic>"#
        ),
        rid = esc(rid),
        cx = cx,
        cy = cy
    )
}

/// The object frame (§19.3.1.21 with a §19.3.2.4 oleObj).
fn frame_xml(
    id: u32,
    (x, y, w, h): (f64, f64, f64, f64),
    obj_rid: &str,
    link: Option<bool>,
    img_rid: &str,
) -> String {
    let child = match link {
        // §19.3.2.3 link@updateAutomatic.
        Some(auto) => format!(r#"<p:link updateAutomatic="{}"/>"#, u8::from(auto)),
        None => "<p:embed/>".to_owned(),
    };
    format!(
        concat!(
            r#"<p:graphicFrame><p:nvGraphicFramePr><p:cNvPr id="{id}" name="Object {id}"/>"#,
            r#"<p:cNvGraphicFramePr><a:graphicFrameLocks noChangeAspect="1"/></p:cNvGraphicFramePr><p:nvPr/></p:nvGraphicFramePr>"#,
            r#"<p:xfrm><a:off x="{x}" y="{y}"/><a:ext cx="{cx}" cy="{cy}"/></p:xfrm>"#,
            r#"<a:graphic><a:graphicData uri="{uri}"><p:oleObj r:id="{orid}" imgW="{cx}" imgH="{cy}" progId="{prog}">{child}{pic}</p:oleObj></a:graphicData></a:graphic></p:graphicFrame>"#
        ),
        id = id,
        x = emu(x),
        y = emu(y),
        cx = emu(w.max(1.0)),
        cy = emu(h.max(1.0)),
        uri = URI_OLE,
        orid = esc(obj_rid),
        prog = PROG_ID_XLSX,
        child = child,
        pic = pic_xml(img_rid, emu(w.max(1.0)), emu(h.max(1.0))),
    )
}

/// Insert a built object in the middle of the slide.
fn cmd_insert(
    tx: &mut Tx<'_>,
    sel: &mut SlideSel,
    b: Built,
    link: Option<(String, bool)>,
) -> R<()> {
    let slide = sel.slide;
    tx.pres
        .slides
        .get(slide)
        .ok_or(EditError::Refused("noSlide"))?;
    let (sw, sh) = slide_size(tx.pres);
    let (w, h) = ole::fit(b.size, sw * 0.8, sh * 0.8);
    let png = build::unique_part(tx.pres, "/ppt/media/image", "png");
    let img_rid = new_rid(tx.pres, slide, &[]);
    let obj_rid = new_rid(tx.pres, slide, std::slice::from_ref(&img_rid));
    add_part(tx, &png, "image/png", b.png)?;
    let obj_rel = if let Some((path, _)) = &link {
        Rel {
            id: obj_rid.clone(),
            rel_type: REL_OLE_OBJECT.into(),
            target: file_uri(path),
            external: true,
        }
    } else {
        let book = build::unique_part(tx.pres, "/ppt/embeddings/Workbook", "xlsx");
        add_part(tx, &book, CT_XLSX, b.xlsx)?;
        Rel {
            id: obj_rid.clone(),
            rel_type: REL_PACKAGE.into(),
            target: format!(
                "../embeddings/{}",
                book.rsplit('/').next().unwrap_or_default()
            ),
            external: false,
        }
    };
    let img_rel = Rel {
        id: img_rid.clone(),
        rel_type: format!("{}image", bo_pptx::RELATIONSHIP_NS),
        target: format!("../media/{}", png.rsplit('/').next().unwrap_or_default()),
        external: false,
    };
    tx.fields(slide, |f| {
        f.rels.push(img_rel);
        f.rels.push(obj_rel);
    })?;
    let id = next_shape_id(tx.pres, slide);
    let xml = frame_xml(
        id,
        ((sw - w) / 2.0, (sh - h) / 2.0, w, h),
        &obj_rid,
        link.map(|(_, auto)| auto),
        &img_rid,
    );
    insert_top(tx, sel, build::node(&xml))
}

/// A file path as an external relationship target URI (ECMA-376 Part 2, §9.3.2).
fn file_uri(path: &str) -> String {
    if path.contains("://") {
        return path.to_owned();
    }
    let p = path.replace('\\', "/").replace(' ', "%20");
    if p.starts_with('/') {
        format!("file://{p}")
    } else {
        format!("file:///{p}")
    }
}

/// Give the frame at `path` a new preview `png` (and size in points), keeping the rest.
fn set_preview(tx: &mut Tx<'_>, slide: usize, f: &Found, png: Vec<u8>, size: (f64, f64)) -> R<()> {
    let part = build::unique_part(tx.pres, "/ppt/media/image", "png");
    add_part(tx, &part, "image/png", png)?;
    let rid = new_rid(tx.pres, slide, &[]);
    let rel = Rel {
        id: rid.clone(),
        rel_type: format!("{}image", bo_pptx::RELATIONSHIP_NS),
        target: format!("../media/{}", part.rsplit('/').next().unwrap_or_default()),
        external: false,
    };
    tx.fields(slide, |x| x.rels.push(rel))?;
    let Some(ShapeNode::Frame(old)) = node_at(tx.pres, slide, &f.path).cloned() else {
        return Err(EditError::Refused("noObject"));
    };
    let mut frame = *old;
    let (cx, cy) = (emu(size.0.max(1.0)), emu(size.1.max(1.0)));
    frame.xfrm.ext = Some((cx, cy));
    // A plain p:oleObj (no AlternateContent) with the new picture; imgW / imgH follow
    // the picture ([MS-OI29500] 2.1.1134 b, c).
    let mut o = bo_pptx::ole::ole_obj(&f.gd)
        .cloned()
        .ok_or(EditError::Refused("noObject"))?;
    for (k, v) in &mut o.attrs {
        match k.as_str() {
            "imgW" => *v = cx.to_string(),
            "imgH" => *v = cy.to_string(),
            "spid" => v.clear(),
            _ => {}
        }
    }
    o.attrs.retain(|(k, v)| !(k == "spid" && v.is_empty()));
    o.children
        .retain(|c| !matches!(c, bo_model::slide::XmlNode::Element(e) if e.local() == "pic"));
    let pic = super::build::parse(&pic_xml(&rid, cx, cy)).ok_or(EditError::Refused("build"))?;
    o.children.push(bo_model::slide::XmlNode::Element(pic));
    let mut gd = f.gd.clone();
    gd.children = vec![bo_model::slide::XmlNode::Element(o)];
    frame.content = GraphicContent::Other(gd);
    tx.replace_node(slide, &f.path, ShapeNode::Frame(Box::new(frame)))?;
    Ok(())
}

/// Slide edit of an object command, prepared before the transaction.
enum Plan {
    None,
    Insert(Built, Option<(String, bool)>),
    Reembed(Built),
    Links(Vec<(usize, u32, Built)>),
}

impl crate::Engine {
    fn slide_ole_target(&self) -> Option<(usize, Found)> {
        let pres = self.pres.as_ref()?;
        let sel = &self.slides.sel;
        match sel.ids.as_slice() {
            [id] => found(pres, sel.slide, *id).map(|f| (sel.slide, f)),
            _ => None,
        }
    }

    fn slide_ole_info(&mut self, slide: usize, f: &Found, cells: bool, data: bool) -> OleInfo {
        let Some(pres) = self.pres.as_ref() else {
            return OleInfo::default();
        };
        let link =
            f.r.link
                .then(|| rel(pres, slide, &f.r.rid).map(|r| r.target.clone()))
                .flatten();
        let package = package_part(pres, slide, &f.r);
        let bytes = package
            .as_deref()
            .and_then(|p| pres.part_data(p))
            .map(<[u8]>::to_vec);
        let workbook = f.r.prog_id.starts_with("Excel.Sheet")
            || package
                .as_deref()
                .is_some_and(|p| p.to_ascii_lowercase().ends_with(".xlsx"));
        let mut i = OleInfo {
            prog_id: f.r.prog_id.clone(),
            workbook,
            linked: f.r.link,
            source: link.as_deref().map(ole::source_path),
            auto_update: f.r.auto_update,
            width: f.size.0,
            height: f.size.1,
            ..OleInfo::default()
        };
        if let (false, true, Some(b)) = (f.r.link, workbook, bytes) {
            if let Ok((sheet, area, inputs)) = ole::area_of(&mut self.fonts, &b) {
                i.sheet = Some(sheet);
                i.range = Some(area.to_string());
                i.cells = cells.then_some(inputs);
            }
            i.data = data.then(|| ole::encode(&b));
        }
        i
    }

    fn slide_links(&self) -> Vec<(usize, u32, LinkOut)> {
        let mut out = Vec::new();
        let Some(pres) = self.pres.as_ref() else {
            return out;
        };
        for (si, s) in pres.slides.iter().enumerate() {
            bo_pptx::visit(&s.data.shapes, &mut |n| {
                let ShapeNode::Frame(fr) = n else { return };
                let GraphicContent::Other(gd) = &fr.content else {
                    return;
                };
                if fr.uri != URI_OLE {
                    return;
                }
                let Some(r) = ole_ref(gd).filter(|r| r.link) else {
                    return;
                };
                if let Some(rel) = s.info.rels.iter().find(|x| x.id == r.rid) {
                    out.push((
                        si,
                        fr.nv.id,
                        LinkOut {
                            source: rel.target.clone(),
                            path: ole::source_path(&rel.target),
                            item: String::new(),
                            count: 1,
                        },
                    ));
                }
            });
        }
        out
    }

    /// Run an embedded-object command on the deck; the result is the slide edit result
    /// with `ole` (the selected object), `links` and `count` (objects updated).
    pub fn slide_ole_command(&mut self, cmd: OleCommand) -> Result<serde_json::Value> {
        if self.pres.is_none() {
            return Err(EngineError::NoDocument);
        }
        let mut out = Outcome::default();
        let target = self.slide_ole_target();
        let mut want = (false, false);
        let mut links = None;
        let mut count = None;
        // Workbook work (fonts) happens before the slide transaction.
        let plan: std::result::Result<Plan, String> = match cmd {
            OleCommand::OleInfo { cells, data } => {
                want = (cells, data);
                Ok(Plan::None)
            }
            OleCommand::OleLinks => {
                let mut v: Vec<LinkOut> = Vec::new();
                for (_, _, l) in self.slide_links() {
                    match v.iter_mut().find(|x| x.source == l.source) {
                        Some(x) => x.count += 1,
                        None => v.push(l),
                    }
                }
                links = Some(v);
                Ok(Plan::None)
            }
            OleCommand::OleInsert {
                data,
                range,
                cells,
                link,
                auto_update,
            } => {
                let src = data.as_deref().map(ole::decode).transpose()?;
                let link = link.filter(|l| !l.trim().is_empty());
                if link.is_some() && src.is_none() {
                    Err("noData".into())
                } else {
                    ole::build(
                        &mut self.fonts,
                        src.as_deref(),
                        range.as_deref(),
                        cells.as_deref(),
                    )
                    .map(|b| Plan::Insert(b, link.map(|l| (l, auto_update))))
                    .map_err(|e| e.to_string())
                }
            }
            OleCommand::OleEdit { cells } => {
                self.slide_old_package(target.as_ref()).and_then(|old| {
                    ole::build(&mut self.fonts, Some(&old), None, Some(&cells))
                        .map(Plan::Reembed)
                        .map_err(|e| e.to_string())
                })
            }
            OleCommand::OleSetData { data } => {
                let bytes = ole::decode(&data)?;
                self.slide_old_package(target.as_ref()).and_then(|_| {
                    ole::build(&mut self.fonts, Some(&bytes), None, None)
                        .map(Plan::Reembed)
                        .map_err(|e| e.to_string())
                })
            }
            OleCommand::OleUpdateLinks { sources } => {
                let mut v = Vec::new();
                let mut err = None;
                let all = self.slide_links();
                for s in sources {
                    let bytes = ole::decode(&s.data)?;
                    let hits: Vec<(usize, u32)> = all
                        .iter()
                        .filter(|(_, _, l)| l.source == s.source || l.path == s.source)
                        .map(|(si, id, _)| (*si, *id))
                        .collect();
                    if hits.is_empty() {
                        continue;
                    }
                    match ole::build(&mut self.fonts, Some(&bytes), None, None) {
                        Ok(b) => v.extend(hits.into_iter().map(|(si, id)| (si, id, b.clone()))),
                        Err(e) => err = Some(e.to_string()),
                    }
                }
                count = Some(v.len());
                match err {
                    Some(e) if v.is_empty() => Err(e),
                    _ => Ok(Plan::Links(v)),
                }
            }
        };
        match plan {
            Err(e) => out.error = Some(e),
            Ok(Plan::None) => {}
            Ok(plan) => {
                let pres = self.pres.as_mut().ok_or(EngineError::NoDocument)?;
                let ed = &mut self.slides;
                ed.clamp(pres);
                ed.text = None;
                ed.table = None;
                let target = target.as_ref();
                ed.mutate(pres, &mut out, |tx, sel| match plan {
                    Plan::Insert(b, link) => cmd_insert(tx, sel, b, link),
                    Plan::Reembed(b) => {
                        let (slide, f) = target.ok_or(EditError::Refused("noObject"))?;
                        let part = package_part(tx.pres, *slide, &f.r)
                            .ok_or(EditError::Refused("noObject"))?;
                        let size = ole::edited_size(f.size, b.old_size, b.size);
                        super::chart::put_part(tx, &part, Some(CT_XLSX), b.xlsx)?;
                        set_preview(tx, *slide, f, b.png, size)
                    }
                    Plan::Links(v) => {
                        for (slide, id, b) in v {
                            let Some(f) = found(tx.pres, slide, id) else {
                                continue;
                            };
                            let size = ole::edited_size(f.size, None, b.size);
                            set_preview(tx, slide, &f, b.png, size)?;
                        }
                        Ok(())
                    }
                    Plan::None => Ok(()),
                });
            }
        }
        let mut r = self.finish_slide_command(out)?;
        r.collab = self.slide_collab_after_edit();
        let mut v = serde_json::to_value(&r).unwrap_or_default();
        let info = self
            .slide_ole_target()
            .map(|(s, f)| self.slide_ole_info(s, &f, want.0, want.1));
        if let Some(o) = v.as_object_mut() {
            if let Some(i) = info {
                o.insert("ole".into(), serde_json::to_value(i).unwrap_or_default());
            }
            if let Some(l) = links {
                o.insert("links".into(), serde_json::to_value(l).unwrap_or_default());
            }
            if let Some(c) = count {
                o.insert("count".into(), c.into());
            }
        }
        Ok(v)
    }

    /// The embedded workbook of the selected object.
    fn slide_old_package(
        &self,
        target: Option<&(usize, Found)>,
    ) -> std::result::Result<Vec<u8>, String> {
        let (slide, f) = target.ok_or("noObject")?;
        if f.r.link {
            return Err("linkedObject".into());
        }
        let pres = self.pres.as_ref().ok_or("noDocument")?;
        let part = package_part(pres, *slide, &f.r).ok_or("noObject")?;
        if !part.to_ascii_lowercase().ends_with(".xlsx")
            && !f.r.prog_id.starts_with("Excel.Sheet.12")
        {
            return Err("notWorkbook".into());
        }
        pres.part_data(&part)
            .map(<[u8]>::to_vec)
            .ok_or_else(|| "noObject".into())
    }
}
