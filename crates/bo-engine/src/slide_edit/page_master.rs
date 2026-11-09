//! Notes Master and Handout Master views, and the Notes Page view's geometry
//! (SLIDE-VIEW-001).
//!
//! The notes master (ECMA-376 Part 1, §19.3.1.27 notesMaster) and the handout
//! master (§19.3.1.24 handoutMaster) stay raw package parts in the model. While
//! one of their views is open, the deck's slides are set aside (like the slide
//! master view, `master.rs`) and the slide list holds a single *view slide*
//! made of the master's `cSld` (§19.3.1.16) at the notes page size
//! (§19.2.1.23 notesSz). A helper slide master built from the part's colour
//! map (§19.3.1.6 clrMap), theme and notes text style (§19.3.1.28 notesStyle)
//! lets every editing command of the normal view work on it unchanged, with
//! the view's own undo history. Closing the view writes the edited `cSld`,
//! `p:hf` (§19.3.1.25) and relationships back into the part and folds the whole
//! session into one undoable step; an untouched master stays byte for byte.
//! A deck without the master gets a new one only when the session changed it.

use super::build;
use super::geom::{Rect, EMU_PT};
use super::ops::{PresFields, SlideOp, Tx};
use super::{EditError, Outcome, SlideEditor, SlideSel, Txn, R};
use crate::slide_export::handout;
use crate::slide_export::masters;
use bo_model::slide::{
    CommonSlideData, Extra, PartInfo, Presentation, RawPart, Rel, ShapeNode, Slide, SlideLayout,
    SlideMaster, SlideSize, TextStyles, XmlElement, XmlNode,
};
use serde::{Deserialize, Serialize};

/// Which master the view edits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PageKind {
    /// §19.3.1.27 notesMaster.
    Notes,
    /// §19.3.1.24 handoutMaster.
    Handout,
}

const NOTES_CT: &str =
    "application/vnd.openxmlformats-officedocument.presentationml.notesMaster+xml";
const HANDOUT_CT: &str =
    "application/vnd.openxmlformats-officedocument.presentationml.handoutMaster+xml";
const RELS_CT: &str = "application/vnd.openxmlformats-package.relationships+xml";

/// Default notes page (§19.2.1.23 notesSz: 7.5 × 10 in, portrait).
const DEFAULT_NOTES: (i64, i64) = (6_858_000, 9_144_000);

const NS: &str = concat!(
    r#"xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" "#,
    r#"xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" "#,
    r#"xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main""#
);

/// A master created for a deck that had none (added when the session changed it).
struct NewMaster {
    /// The master part, its `.rels` part and its theme part.
    parts: Vec<RawPart>,
    /// Relationship from the presentation part.
    rel: Rel,
    /// `p:notesMasterIdLst` / `p:handoutMasterIdLst` (§19.2.1.18 / §19.2.1.15).
    id_list: XmlElement,
}

/// What the normal view had while a notes / handout master view is open.
pub(crate) struct PageView {
    kind: PageKind,
    slides: Vec<Slide>,
    sel: SlideSel,
    undo: Vec<Txn>,
    redo: Vec<Txn>,
    /// Presentation fields and parts when the view opened.
    entry: PresFields,
    entry_parts: Vec<RawPart>,
    /// Part name of the master.
    part: String,
    /// Root element of the master as read (or as built).
    root: XmlElement,
    /// Relationships of the master as read.
    rels: Vec<Rel>,
    /// The view slide as it was built (detects changes without rewriting the part).
    view: Slide,
    created: Option<NewMaster>,
    /// The deck had unsaved changes when the view opened.
    modified: bool,
    /// View history state last saved (0: none).
    saved: u64,
    /// Slides per handout page shown by the handout master view (preview only:
    /// the handout master part has no such setting).
    per_page: u8,
}

impl PageView {
    pub(crate) fn modified(&self, state: u64) -> bool {
        self.modified || state != self.saved
    }
}

/// The view's pane: what the ribbon toggles show.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PageMasterOut {
    /// Which master.
    pub kind: PageKind,
    /// Header placeholder present (`hdr`).
    pub header: bool,
    /// Date placeholder present (`dt`).
    pub date: bool,
    /// Footer placeholder present (`ftr`).
    pub footer: bool,
    /// Page number placeholder present (`sldNum`).
    pub number: bool,
    /// Slide image placeholder present (`sldImg`, notes master).
    pub slide_image: bool,
    /// Notes body placeholder present (`body`, notes master).
    pub body: bool,
    /// The page is portrait.
    pub portrait: bool,
    /// Slides per page of the handout preview.
    pub per_page: u8,
    /// Where the slides go on a handout page (points), for the preview.
    pub slots: Vec<Rect>,
}

/// Geometry of a notes page (points of the notes page, §19.2.1.23 notesSz).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NotesPageOut {
    /// Page width.
    pub width: f64,
    /// Page height.
    pub height: f64,
    /// The slide picture: the `sldImg` box of the notes slide or the notes
    /// master, the slide fitted in it keeping its aspect ratio.
    pub image: Rect,
    /// The notes text box (`body` placeholder).
    pub body: Rect,
    /// Speaker notes.
    pub text: String,
    /// Notes text size (§19.3.1.28 notesStyle level 1), points.
    pub size: f64,
}

fn rect(f: handout::Frame) -> Rect {
    Rect {
        x: f64::from(f.x),
        y: f64::from(f.y),
        w: f64::from(f.w),
        h: f64::from(f.h),
    }
}

fn notes_size(pres: &Presentation) -> (i64, i64) {
    if pres.notes_size.cx > 0 && pres.notes_size.cy > 0 {
        (pres.notes_size.cx, pres.notes_size.cy)
    } else {
        DEFAULT_NOTES
    }
}

/// Notes pages of every slide: the same boxes the notes page export uses.
pub fn notes_pages(pres: &Presentation) -> Vec<NotesPageOut> {
    let (nw, nh) = notes_size(pres);
    let (pw, ph) = (nw as f32 / EMU_PT as f32, nh as f32 / EMU_PT as f32);
    let (sw, sh) = (
        pres.slide_size.cx as f32 / EMU_PT as f32,
        pres.slide_size.cy as f32 / EMU_PT as f32,
    );
    let master = masters::notes_master(pres);
    pres.slides
        .iter()
        .map(|s| {
            let own = s
                .notes
                .as_ref()
                .map(masters::notes_boxes)
                .unwrap_or_default();
            let img = own
                .slide_image
                .or(master.as_ref().and_then(|m| m.slide_image))
                .unwrap_or_else(|| handout::default_notes_image(pw, ph));
            let body = own
                .body
                .or(master.as_ref().and_then(|m| m.body))
                .unwrap_or_else(|| handout::default_notes_body(pw, ph, &img));
            NotesPageOut {
                width: f64::from(pw),
                height: f64::from(ph),
                image: rect(handout::fit(&img, sw / sh.max(1.0))),
                body: rect(body),
                text: s
                    .notes
                    .as_ref()
                    .map(bo_model::slide::Notes::text)
                    .unwrap_or_default(),
                size: f64::from(
                    master
                        .as_ref()
                        .and_then(|m| m.notes_size_pt)
                        .unwrap_or(12.0),
                ),
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Building and locating the master parts
// ---------------------------------------------------------------------------

fn part_of_type<'a>(pres: &'a Presentation, ct: &str) -> Option<&'a RawPart> {
    pres.parts
        .iter()
        .find(|p| p.content_type.as_deref() == Some(ct))
}

fn rels_name(part: &str) -> String {
    bo_opc::rels_part_name(part)
}

fn parse_rels(data: &[u8], name: &str) -> Vec<Rel> {
    bo_opc::parse_relationships(data, name)
        .unwrap_or_default()
        .into_iter()
        .map(|r| Rel {
            id: r.id,
            rel_type: r.rel_type,
            target: r.target,
            external: r.mode == bo_opc::TargetMode::External,
        })
        .collect()
}

/// Relationships part (ECMA-376 Part 2, §9.3.2.1).
fn rels_xml(rels: &[Rel]) -> Vec<u8> {
    use bo_opc::xml::escape;
    let mut s = String::from(concat!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>
",
        "<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">"
    ));
    for r in rels {
        s.push_str(&format!(
            "<Relationship Id=\"{}\" Type=\"{}\" Target=\"{}\"{}/>",
            escape(&r.id),
            escape(&r.rel_type),
            escape(&r.target),
            if r.external {
                " TargetMode=\"External\""
            } else {
                ""
            }
        ));
    }
    s.push_str("</Relationships>");
    s.into_bytes()
}

/// Header / date / footer / page number placeholder (§19.3.1.36 ph) in a box (EMU).
fn hf_shape_xml(id: u32, kind: &str, idx: u32, (x, y, w, h): (i64, i64, i64, i64)) -> String {
    let (name, algn, anchor) = match kind {
        "hdr" => ("Header Placeholder", "l", "t"),
        "dt" => ("Date Placeholder", "r", "t"),
        "ftr" => ("Footer Placeholder", "l", "b"),
        _ => ("Slide Number Placeholder", "r", "b"),
    };
    let idx_attr = if kind == "hdr" {
        String::new()
    } else {
        format!(r#" idx="{idx}""#)
    };
    // §21.1.2.2.4 fld type="slidenum": the page number.
    let para = if kind == "sldNum" {
        format!(
            concat!(
                r#"<a:p><a:fld id="{{B6F15528-21DE-4FAA-801E-634DDDAF4B2B}}" type="slidenum">"#,
                r#"<a:rPr lang="{lang}"/><a:t>#</a:t></a:fld><a:endParaRPr lang="{lang}"/></a:p>"#
            ),
            lang = build::LANG
        )
    } else {
        format!(r#"<a:p><a:endParaRPr lang="{}"/></a:p>"#, build::LANG)
    };
    format!(
        concat!(
            r#"<p:sp><p:nvSpPr><p:cNvPr id="{id}" name="{name} {n}"/><p:cNvSpPr><a:spLocks noGrp="1"/></p:cNvSpPr>"#,
            r#"<p:nvPr><p:ph type="{kind}" sz="quarter"{idx}/></p:nvPr></p:nvSpPr>"#,
            r#"<p:spPr><a:xfrm><a:off x="{x}" y="{y}"/><a:ext cx="{w}" cy="{h}"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr>"#,
            r#"<p:txBody><a:bodyPr vert="horz" lIns="91440" tIns="45720" rIns="91440" bIns="45720" anchor="{anchor}"/>"#,
            r#"<a:lstStyle><a:lvl1pPr algn="{algn}"><a:defRPr sz="1200"/></a:lvl1pPr></a:lstStyle>{para}</p:txBody></p:sp>"#
        ),
        id = id,
        name = name,
        n = id.saturating_sub(1),
        kind = kind,
        idx = idx_attr,
        x = x,
        y = y,
        w = w,
        h = h,
        anchor = anchor,
        algn = algn,
        para = para,
    )
}

/// Our default box of a header / footer item on a `w` × `h` page (EMU): the
/// four corners, 42 % of the width, 5 % of the height.
fn hf_box(kind: &str, w: i64, h: i64) -> (i64, i64, i64, i64) {
    let bw = w * 42 / 100;
    let bh = h / 20;
    let x = if matches!(kind, "hdr" | "ftr") {
        0
    } else {
        w - bw
    };
    let y = if matches!(kind, "hdr" | "dt") {
        0
    } else {
        h - bh
    };
    (x, y, bw, bh)
}

/// Slide image and notes body boxes of a new notes master (EMU), as
/// `build::new_notes_master` places them.
fn notes_boxes(pres: &Presentation, w: i64, h: i64) -> [(i64, i64, i64, i64); 2] {
    let sw = pres.slide_size.cx.max(1) as f64;
    let sh = pres.slide_size.cy.max(1) as f64;
    let iw = (w as f64 * 0.8) as i64;
    let ih = (iw as f64 * sh / sw) as i64;
    let mx = w / 10;
    let iy = h / 12;
    let by = iy + ih + h / 24;
    [(mx, iy, iw, ih), (mx, by, w - 2 * mx, h - by - h / 12)]
}

fn body_shape_xml(id: u32, kind: &str, (x, y, w, h): (i64, i64, i64, i64)) -> String {
    if kind == "sldImg" {
        format!(
            concat!(
                r#"<p:sp><p:nvSpPr><p:cNvPr id="{id}" name="Slide Image Placeholder {n}"/><p:cNvSpPr><a:spLocks noGrp="1" noRot="1" noChangeAspect="1"/></p:cNvSpPr>"#,
                r#"<p:nvPr><p:ph type="sldImg" idx="2"/></p:nvPr></p:nvSpPr><p:spPr><a:xfrm><a:off x="{x}" y="{y}"/><a:ext cx="{w}" cy="{h}"/></a:xfrm>"#,
                r#"<a:prstGeom prst="rect"><a:avLst/></a:prstGeom><a:noFill/><a:ln w="12700"><a:solidFill><a:schemeClr val="tx1"/></a:solidFill></a:ln></p:spPr></p:sp>"#
            ),
            id = id,
            n = id.saturating_sub(1),
            x = x,
            y = y,
            w = w,
            h = h,
        )
    } else {
        format!(
            concat!(
                r#"<p:sp><p:nvSpPr><p:cNvPr id="{id}" name="Notes Placeholder {n}"/><p:cNvSpPr><a:spLocks noGrp="1"/></p:cNvSpPr>"#,
                r#"<p:nvPr><p:ph type="body" sz="quarter" idx="3"/></p:nvPr></p:nvSpPr><p:spPr><a:xfrm><a:off x="{x}" y="{y}"/><a:ext cx="{w}" cy="{h}"/></a:xfrm>"#,
                r#"<a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr><p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:endParaRPr lang="{lang}"/></a:p></p:txBody></p:sp>"#
            ),
            id = id,
            n = id.saturating_sub(1),
            x = x,
            y = y,
            w = w,
            h = h,
            lang = build::LANG,
        )
    }
}

fn presentation_rid(pres: &Presentation) -> String {
    let mut n = 1;
    loop {
        let id = format!("rId{n}");
        let taken = pres.info.rels.iter().any(|r| r.id == id)
            || pres.slides.iter().any(|s| s.pres_rel_id == id)
            || pres.masters.iter().any(|m| m.pres_rel_id == id);
        if !taken {
            return id;
        }
        n += 1;
    }
}

/// A handout master with header, date, footer and page number (§19.3.1.24),
/// its theme (a copy of the first slide theme) and the relationships.
fn new_handout_master(pres: &Presentation) -> Option<NewMaster> {
    let master = build::unique_part(pres, "/ppt/handoutMasters/handoutMaster", "xml");
    let theme_name = build::unique_part(pres, "/ppt/theme/theme", "xml");
    let theme_bytes = bo_drawingml::theme::write_theme(pres.themes.first()?);
    let (w, h) = notes_size(pres);
    let shapes: String = [("hdr", 0), ("dt", 1), ("ftr", 2), ("sldNum", 3)]
        .iter()
        .enumerate()
        .map(|(i, (k, idx))| hf_shape_xml(i as u32 + 2, k, *idx, hf_box(k, w, h)))
        .collect();
    let xml = format!(
        concat!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#,
            r#"<p:handoutMaster {ns}><p:cSld><p:bg><p:bgRef idx="1001"><a:schemeClr val="bg1"/></p:bgRef></p:bg><p:spTree>"#,
            r#"<p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/>"#,
            r#"{shapes}</p:spTree></p:cSld>"#,
            r#"<p:clrMap bg1="lt1" tx1="dk1" bg2="lt2" tx2="dk2" accent1="accent1" accent2="accent2" accent3="accent3" "#,
            r#"accent4="accent4" accent5="accent5" accent6="accent6" hlink="hlink" folHlink="folHlink"/></p:handoutMaster>"#
        ),
        ns = NS,
        shapes = shapes,
    );
    let theme_target = format!("../theme/{}", theme_name.rsplit('/').next()?);
    let rels = vec![Rel {
        id: "rId1".into(),
        rel_type: format!("{}theme", bo_pptx::RELATIONSHIP_NS),
        target: theme_target,
        external: false,
    }];
    let rid = presentation_rid(pres);
    let id_list = bo_drawingml::dom::parse(
        format!(
            r#"<p:handoutMasterIdLst {NS}><p:handoutMasterId r:id="{rid}"/></p:handoutMasterIdLst>"#
        )
        .as_bytes(),
        "presentation",
    )
    .ok()?;
    let mut id_list = id_list;
    id_list.attrs.clear();
    Some(NewMaster {
        parts: vec![
            RawPart {
                name: master.clone(),
                content_type: Some(HANDOUT_CT.into()),
                data: xml.into_bytes(),
            },
            RawPart {
                name: rels_name(&master),
                content_type: Some(RELS_CT.into()),
                data: rels_xml(&rels),
            },
            RawPart {
                name: theme_name,
                content_type: Some(
                    "application/vnd.openxmlformats-officedocument.theme+xml".into(),
                ),
                data: theme_bytes,
            },
        ],
        rel: Rel {
            id: rid,
            rel_type: format!("{}handoutMaster", bo_pptx::RELATIONSHIP_NS),
            target: master.trim_start_matches("/ppt/").to_owned(),
            external: false,
        },
        id_list,
    })
}

fn new_notes_master(pres: &Presentation) -> Option<NewMaster> {
    let nm = build::new_notes_master(pres)?;
    Some(NewMaster {
        parts: nm.parts,
        rel: nm.rel,
        id_list: nm.id_list,
    })
}

fn child_index(root: &XmlElement, local: &str) -> Option<usize> {
    root.children
        .iter()
        .position(|c| matches!(c, XmlNode::Element(e) if e.local() == local))
}

/// The master root with `data` as its `cSld` and `hf` (§19.3.1.25) after `clrMap`.
fn rebuild_root(root: &XmlElement, data: &CommonSlideData, hf: Option<&XmlElement>) -> XmlElement {
    let mut r = root.clone();
    let csld = XmlNode::Element(bo_pptx::write_csld(data));
    match child_index(&r, "cSld") {
        Some(i) => r.children[i] = csld,
        None => r.children.insert(0, csld),
    }
    if let Some(i) = child_index(&r, "hf") {
        r.children.remove(i);
    }
    if let Some(hf) = hf {
        let at = child_index(&r, "clrMap").map_or(r.children.len(), |i| i + 1);
        r.children.insert(at, XmlNode::Element(hf.clone()));
    }
    r
}

/// `p:hf` stashed in the view slide's extras.
fn hf_of(s: &Slide) -> Option<&XmlElement> {
    s.extra.iter().map(|x| &x.xml).find(|x| x.local() == "hf")
}

// ---------------------------------------------------------------------------
// Opening and closing the view
// ---------------------------------------------------------------------------

/// Text styles of the helper master: the notes master's `notesStyle` as body style.
fn text_styles(pres: &Presentation, root: &XmlElement, kind: PageKind) -> TextStyles {
    let base = pres
        .masters
        .first()
        .map(|m| m.text_styles.clone())
        .unwrap_or_default();
    let notes = root
        .elements()
        .find(|e| e.local() == "notesStyle")
        .map(bo_drawingml::text::parse_list_style);
    TextStyles {
        title: None,
        body: if kind == PageKind::Notes {
            notes.or(base.body)
        } else {
            base.body
        },
        other: base.other,
        extra: Vec::new(),
    }
}

fn enter(ed: &mut SlideEditor, pres: &mut Presentation, kind: PageKind, out: &mut Outcome) {
    if ed.page.is_some() || ed.master.is_some() {
        out.error = Some("masterView".into());
        return;
    }
    if ed.read_only {
        out.error = Some(crate::security::READ_ONLY.into());
        return;
    }
    if pres.themes.is_empty() {
        out.error = Some("noMaster".into());
        return;
    }
    let ct = match kind {
        PageKind::Notes => NOTES_CT,
        PageKind::Handout => HANDOUT_CT,
    };
    let (part, data, rels, created) = if let Some(p) = part_of_type(pres, ct) {
        let rn = rels_name(&p.name);
        let rels = pres
            .part_data(&rn)
            .map(|d| parse_rels(d, &rn))
            .unwrap_or_default();
        (p.name.clone(), p.data.clone(), rels, None)
    } else {
        let built = match kind {
            PageKind::Notes => new_notes_master(pres),
            PageKind::Handout => new_handout_master(pres),
        };
        let Some(nm) = built else {
            out.error = Some("build".into());
            return;
        };
        let name = nm.parts[0].name.clone();
        let data = nm.parts[0].data.clone();
        let rels = parse_rels(&nm.parts[1].data, &nm.parts[1].name);
        (name, data, rels, Some(nm))
    };
    let Ok(root) = bo_drawingml::dom::parse(&data, &part) else {
        out.error = Some("badPart".into());
        return;
    };
    let entry = PresFields::of(pres);
    let entry_parts = pres.parts.clone();
    // The master's theme (§20.1.6.9), for its colours and fonts.
    let theme = rels
        .iter()
        .find(|r| r.rel_type.ends_with("/theme") && !r.external)
        .map(|r| bo_opc::resolve_target(&part, &r.target))
        .and_then(|t| {
            pres.part_data(&t)
                .map(<[u8]>::to_vec)
                .or_else(|| {
                    created
                        .as_ref()
                        .and_then(|c| c.parts.iter().find(|p| p.name.eq_ignore_ascii_case(&t)))
                        .map(|p| p.data.clone())
                })
                .and_then(|d| bo_drawingml::theme::parse_theme(&d, &t).ok())
        });
    let theme = match theme {
        Some(t) => {
            pres.themes.push(t);
            pres.themes.len() - 1
        }
        None => pres.masters.first().map_or(0, |m| m.theme),
    };
    let clr_map = root
        .elements()
        .find(|e| e.local() == "clrMap")
        .map(|e| e.attrs.clone())
        .unwrap_or_default();
    let styles = text_styles(pres, &root, kind);
    pres.masters.push(SlideMaster {
        data: CommonSlideData {
            tree_nv: bo_model::slide::NonVisual {
                id: 1,
                ..Default::default()
            },
            ..CommonSlideData::default()
        },
        clr_map,
        theme,
        text_styles: styles,
        ..SlideMaster::default()
    });
    pres.layouts.push(SlideLayout {
        master: pres.masters.len() - 1,
        show_master_sp: Some(false),
        ..SlideLayout::default()
    });
    let data = root
        .elements()
        .find(|e| e.local() == "cSld")
        .map(bo_pptx::parse_csld)
        .unwrap_or_default();
    let view = Slide {
        info: PartInfo {
            part_name: part.clone(),
            rels: rels.clone(),
            root_attrs: root.attrs.clone(),
        },
        data,
        layout: pres.layouts.len() - 1,
        extra: root
            .elements()
            .find(|e| e.local() == "hf")
            .map(|h| {
                vec![Extra {
                    key: 0,
                    xml: h.clone(),
                }]
            })
            .unwrap_or_default(),
        ..Slide::default()
    };
    let (w, h) = notes_size(pres);
    pres.slide_size = SlideSize {
        cx: w,
        cy: h,
        kind: None,
    };
    let modified = ed.state_id() != ed.saved;
    let slides = std::mem::replace(&mut pres.slides, vec![view.clone()]);
    let sel = std::mem::take(&mut ed.sel);
    ed.page = Some(PageView {
        kind,
        slides,
        sel,
        undo: std::mem::take(&mut ed.undo),
        redo: std::mem::take(&mut ed.redo),
        entry,
        entry_parts,
        part,
        root,
        rels,
        view,
        created,
        modified,
        saved: 0,
        per_page: 6,
    });
    out.all = true;
}

/// The deck's parts and presentation fields with the edited master written back.
fn assembled(pres: &Presentation, pv: &PageView) -> (Vec<RawPart>, PresFields) {
    let mut parts = pres.parts.clone();
    let mut fields = pv.entry.clone();
    let Some(view) = pres.slides.first() else {
        return (pv.entry_parts.clone(), fields);
    };
    let root = rebuild_root(&pv.root, &view.data, hf_of(view));
    // Compared on the model: an untouched part is never rewritten.
    let changed_root = view.data != pv.view.data || hf_of(view) != hf_of(&pv.view);
    let changed_rels = view.info.rels != pv.rels;
    let size = SlideSize {
        cx: pres.slide_size.cx,
        cy: pres.slide_size.cy,
        kind: pv.entry.notes_size.kind.clone(),
    };
    let size_changed = (size.cx, size.cy) != notes_size_of(&pv.entry);
    if size_changed {
        fields.notes_size = size;
    }
    if !changed_root && !changed_rels && !size_changed {
        return (parts, fields);
    }
    let bytes = || bo_drawingml::dom::to_part_xml(&root);
    let rels_part = rels_name(&pv.part);
    if let Some(nm) = &pv.created {
        let mut new = nm.parts.clone();
        new[0].data = bytes();
        new[1].data = rels_xml(&view.info.rels);
        parts.extend(new);
        fields.rels.push(nm.rel.clone());
        fields
            .extra
            .push(bo_pptx::presentation_extra(nm.id_list.clone()));
    } else {
        if changed_root {
            if let Some(p) = parts.iter_mut().find(|p| p.name == pv.part) {
                p.data = bytes();
            }
        }
        if changed_rels {
            let data = rels_xml(&view.info.rels);
            if let Some(p) = parts
                .iter_mut()
                .find(|p| p.name.eq_ignore_ascii_case(&rels_part))
            {
                p.data = data;
            } else {
                parts.push(RawPart {
                    name: rels_part,
                    content_type: Some(RELS_CT.into()),
                    data,
                });
            }
        }
    }
    (parts, fields)
}

fn notes_size_of(f: &PresFields) -> (i64, i64) {
    if f.notes_size.cx > 0 && f.notes_size.cy > 0 {
        (f.notes_size.cx, f.notes_size.cy)
    } else {
        DEFAULT_NOTES
    }
}

fn restore(pres: &mut Presentation, pv: &PageView, parts: Vec<RawPart>, fields: &PresFields) {
    pres.slides.clone_from(&pv.slides);
    pres.parts = parts;
    pres.info.rels.clone_from(&fields.rels);
    pres.extra.clone_from(&fields.extra);
    pres.layouts.clone_from(&fields.layouts);
    pres.masters.clone_from(&fields.masters);
    pres.themes.clone_from(&fields.themes);
    pres.slide_size.clone_from(&fields.slide_size);
    pres.notes_size.clone_from(&fields.notes_size);
}

fn exit(ed: &mut SlideEditor, pres: &mut Presentation, out: &mut Outcome) {
    let Some(pv) = ed.page.take() else { return };
    let saved_now = !pv.modified(ed.state_id());
    let (parts, fields) = assembled(pres, &pv);
    restore(pres, &pv, parts.clone(), &fields);
    ed.sel = pv.sel;
    ed.undo = pv.undo;
    ed.redo = pv.redo;
    let mut ops = Vec::new();
    if parts != pv.entry_parts {
        ops.push(SlideOp::Parts {
            index: 0,
            remove: pv.entry_parts,
            insert: parts,
        });
    }
    if fields != pv.entry {
        ops.push(SlideOp::Pres {
            before: Box::new(pv.entry),
            after: Box::new(fields),
        });
    }
    if !ops.is_empty() {
        ed.next_txn += 1;
        ed.undo.push(Txn {
            id: ed.next_txn,
            ops,
            before: ed.sel.clone(),
            after: ed.sel.clone(),
            typing: None,
        });
        ed.redo.clear();
    }
    if saved_now {
        ed.saved = ed.state_id();
    }
    out.all = true;
}

/// The real presentation while a notes / handout master view is open (for saving).
pub(crate) fn assemble(pres: &Presentation, ed: &SlideEditor) -> Option<Presentation> {
    let pv = ed.page.as_ref()?;
    let (parts, fields) = assembled(pres, pv);
    let mut p = pres.clone();
    restore(&mut p, pv, parts, &fields);
    Some(p)
}

impl SlideEditor {
    /// A notes or handout master view is open.
    pub(crate) fn in_page_master_view(&self) -> bool {
        self.page.is_some()
    }

    /// Any master view (slide, notes or handout) is open.
    pub(crate) fn in_any_master_view(&self) -> bool {
        self.master.is_some() || self.page.is_some()
    }

    /// Unsaved changes (the deck's, or the open master view session's).
    pub(crate) fn is_modified(&self) -> bool {
        let state = self.state_id();
        if let Some(pv) = &self.page {
            return pv.modified(state);
        }
        match &self.master {
            Some(mv) => mv.modified(state),
            None => state != self.saved,
        }
    }

    /// A save while a notes / handout master view is open.
    pub(super) fn page_saved(&mut self) {
        let state = self.state_id();
        if let Some(pv) = &mut self.page {
            pv.modified = false;
            pv.saved = state;
        }
    }
}

// ---------------------------------------------------------------------------
// Commands inside the view
// ---------------------------------------------------------------------------

/// Placeholder kinds of an element toggle (§19.7.10 ST_PlaceholderType).
const ELEMENTS: [&str; 6] = ["hdr", "dt", "ftr", "sldNum", "sldImg", "body"];

fn ph_kind(n: &ShapeNode) -> Option<&str> {
    match n {
        ShapeNode::Shape(s) => s.nv.placeholder.as_ref().and_then(|p| p.kind.as_deref()),
        _ => None,
    }
}

/// `p:hf` attribute of a header / footer kind (§19.3.1.25).
fn hf_attr(kind: &str) -> Option<&'static str> {
    match kind {
        "hdr" => Some("hdr"),
        "dt" => Some("dt"),
        "ftr" => Some("ftr"),
        "sldNum" => Some("sldNum"),
        _ => None,
    }
}

/// Show (`true`) or remove (`false`) placeholders of the view slide.
fn set_elements(tx: &mut Tx<'_>, kind: PageKind, want: &[(&str, bool)]) -> R<()> {
    let old = tx
        .pres
        .slides
        .first()
        .cloned()
        .ok_or(EditError::Refused("noSlide"))?;
    let mut s = old.clone();
    let (w, h) = (tx.pres.slide_size.cx, tx.pres.slide_size.cy);
    for &(k, on) in want {
        if kind == PageKind::Handout && matches!(k, "sldImg" | "body") {
            continue;
        }
        let present = s.data.shapes.iter().any(|n| ph_kind(n) == Some(k));
        if !on {
            s.data.shapes.retain(|n| ph_kind(n) != Some(k));
            continue;
        }
        if !present {
            let mut id = s.data.tree_nv.id;
            bo_pptx::visit(&s.data.shapes, &mut |n| {
                if let Some(nv) = n.nv() {
                    id = id.max(nv.id);
                }
            });
            let id = id + 1;
            let xml = match k {
                "sldImg" | "body" => {
                    let b = notes_boxes(tx.pres, w, h);
                    body_shape_xml(id, k, if k == "sldImg" { b[0] } else { b[1] })
                }
                _ => {
                    let idx = ELEMENTS.iter().position(|e| *e == k).unwrap_or(0) as u32;
                    hf_shape_xml(id, k, idx, hf_box(k, w, h))
                }
            };
            let node = build::node(&xml).ok_or(EditError::Refused("build"))?;
            s.data.shapes.push(node);
        }
        // An item switched off by `p:hf` (§19.3.1.25, default on) is switched on again.
        if let (Some(attr), Some(hf)) = (
            hf_attr(k),
            s.extra.iter_mut().find(|x| x.xml.local() == "hf"),
        ) {
            hf.xml
                .attrs
                .retain(|(n, v)| !(n == attr && matches!(v.as_str(), "0" | "false")));
        }
    }
    if s != old {
        tx.op(SlideOp::Slides {
            index: 0,
            remove: vec![old],
            insert: vec![s],
        })?;
    }
    Ok(())
}

/// Turn the page portrait or landscape: swap the notes size and move the
/// placeholders proportionally (the slide image keeps its aspect ratio).
fn set_orientation(tx: &mut Tx<'_>, portrait: bool) -> R<()> {
    let (w, h) = (tx.pres.slide_size.cx, tx.pres.slide_size.cy);
    if (h >= w) == portrait {
        return Ok(());
    }
    let (nw, nh) = (h, w);
    let sx = nw as f64 / w.max(1) as f64;
    let sy = nh as f64 / h.max(1) as f64;
    let old = tx
        .pres
        .slides
        .first()
        .cloned()
        .ok_or(EditError::Refused("noSlide"))?;
    let mut s = old.clone();
    for n in &mut s.data.shapes {
        let keep_aspect = ph_kind(n) == Some("sldImg");
        let xfrm = match n {
            ShapeNode::Shape(x) => x.props.xfrm.as_mut(),
            ShapeNode::Picture(x) => x.props.xfrm.as_mut(),
            ShapeNode::Connector(x) => x.props.xfrm.as_mut(),
            ShapeNode::Group(g) => g.props.xfrm.as_mut(),
            _ => None,
        };
        let Some(x) = xfrm else { continue };
        let (Some((ox, oy)), Some((cx, cy))) = (x.off, x.ext) else {
            continue;
        };
        let (bx, by) = (ox as f64 * sx, oy as f64 * sy);
        let (bw, bh) = (cx as f64 * sx, cy as f64 * sy);
        if keep_aspect {
            let k = sx.min(sy);
            let (fw, fh) = (cx as f64 * k, cy as f64 * k);
            x.off = Some(((bx + (bw - fw) / 2.0) as i64, (by + (bh - fh) / 2.0) as i64));
            x.ext = Some((fw as i64, fh as i64));
        } else {
            x.off = Some((bx as i64, by as i64));
            x.ext = Some((bw as i64, bh as i64));
        }
    }
    let before = PresFields::of(tx.pres);
    let mut after = before.clone();
    after.slide_size = SlideSize {
        cx: nw,
        cy: nh,
        kind: None,
    };
    tx.op(SlideOp::Pres {
        before: Box::new(before),
        after: Box::new(after),
    })?;
    if s != old {
        tx.op(SlideOp::Slides {
            index: 0,
            remove: vec![old],
            insert: vec![s],
        })?;
    }
    Ok(())
}

/// Commands of the notes / handout master views (JSON bridge, ADR-0008).
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(tag = "cmd", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum PageMasterCommand {
    /// Open or close the notes / handout master view.
    PageMasterView {
        /// Which master (ignored when closing).
        #[serde(default = "notes_kind")]
        kind: PageKind,
        /// Open the view.
        on: bool,
    },
    /// Show or remove placeholders of the master.
    PageMasterElements {
        /// Header (`hdr`).
        #[serde(default)]
        header: Option<bool>,
        /// Date (`dt`).
        #[serde(default)]
        date: Option<bool>,
        /// Footer (`ftr`).
        #[serde(default)]
        footer: Option<bool>,
        /// Page number (`sldNum`).
        #[serde(default)]
        number: Option<bool>,
        /// Slide image (`sldImg`, notes master).
        #[serde(default)]
        slide_image: Option<bool>,
        /// Notes body (`body`, notes master).
        #[serde(default)]
        body: Option<bool>,
    },
    /// Portrait or landscape notes / handout pages (§19.2.1.23 notesSz).
    PageOrientation {
        /// Portrait.
        portrait: bool,
    },
    /// Slides per page of the handout preview (1, 2, 3, 4, 6 or 9).
    HandoutSlides {
        /// Slides per page.
        per_page: u8,
    },
}

fn notes_kind() -> PageKind {
    PageKind::Notes
}

/// Command names of [`PageMasterCommand`] (routing in the JSON bridge).
pub const COMMANDS: &[&str] = &[
    "pageMasterView",
    "pageMasterElements",
    "pageOrientation",
    "handoutSlides",
];

/// Run a page master command (the caller refuses it while collaborating).
pub(super) fn run(
    ed: &mut SlideEditor,
    pres: &mut Presentation,
    cmd: PageMasterCommand,
    out: &mut Outcome,
) {
    match cmd {
        PageMasterCommand::PageMasterView { kind, on: true } => enter(ed, pres, kind, out),
        PageMasterCommand::PageMasterView { on: false, .. } => exit(ed, pres, out),
        PageMasterCommand::HandoutSlides { per_page } => match &mut ed.page {
            Some(pv) if handout::grid(per_page).is_some() => pv.per_page = per_page,
            _ => out.error = Some("notMasterView".into()),
        },
        PageMasterCommand::PageMasterElements {
            header,
            date,
            footer,
            number,
            slide_image,
            body,
        } => {
            let Some(kind) = ed.page.as_ref().map(|p| p.kind) else {
                out.error = Some("notMasterView".into());
                return;
            };
            let want: Vec<(&str, bool)> = [
                ("hdr", header),
                ("dt", date),
                ("ftr", footer),
                ("sldNum", number),
                ("sldImg", slide_image),
                ("body", body),
            ]
            .into_iter()
            .filter_map(|(k, v)| v.map(|v| (k, v)))
            .collect();
            ed.mutate(pres, out, |tx, _| set_elements(tx, kind, &want));
            out.all = true;
        }
        PageMasterCommand::PageOrientation { portrait } => {
            if ed.page.is_none() {
                out.error = Some("notMasterView".into());
                return;
            }
            ed.mutate(pres, out, |tx, _| set_orientation(tx, portrait));
            out.all = true;
        }
    }
}

/// The view's pane, while a notes / handout master view is open.
pub(crate) fn view_out(pres: &Presentation, ed: &SlideEditor) -> Option<PageMasterOut> {
    let pv = ed.page.as_ref()?;
    let s = pres.slides.first()?;
    let has = |k: &str| s.data.shapes.iter().any(|n| ph_kind(n) == Some(k));
    let (w, h) = (
        pres.slide_size.cx as f32 / EMU_PT as f32,
        pres.slide_size.cy as f32 / EMU_PT as f32,
    );
    let slots = if pv.kind == PageKind::Handout {
        let e = &pv.entry.slide_size;
        let aspect = e.cx as f32 / (e.cy.max(1) as f32);
        handout::layout(pv.per_page, w, h, aspect)
            .map(|l| l.frames.into_iter().map(rect).collect())
            .unwrap_or_default()
    } else {
        Vec::new()
    };
    Some(PageMasterOut {
        kind: pv.kind,
        header: has("hdr"),
        date: has("dt"),
        footer: has("ftr"),
        number: has("sldNum"),
        slide_image: has("sldImg"),
        body: has("body"),
        portrait: h >= w,
        per_page: pv.per_page,
        slots,
    })
}
