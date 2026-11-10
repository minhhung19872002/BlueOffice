//! Zoom objects on slides (SLIDE-ZOOM-001): slide, section and summary zooms
//! ([MS-PPTX] §2.8–§2.11, schemas §5.7–§5.10).
//!
//! A zoom is a graphic frame (ECMA-376 Part 1, §19.3.1.21) whose `a:graphicData@uri` is the
//! namespace of its kind and holds one container:
//!
//! - `pslz:sldZm` (CT_SlideZoom, `http://schemas.microsoft.com/office/powerpoint/2016/slidezoom`)
//!   with one `pslz:sldZmObj@sldId` (and optional `cId`);
//! - `psez:sectionZm` (CT_SectionZoom, `…/2016/sectionzoom`) with one
//!   `psez:sectionZmObj@sectionId`;
//! - `psuz:summaryZm` (CT_SummaryZoom, `…/2016/summaryzoom`) with any number of
//!   `psuz:summaryZmObj@sectionId`, then `gridLayout` | `fixedLayout`.
//!
//! Every object carries `zmPr` (p166:CT_ZoomObjectProperties, `…/2016/6/main`): `id`,
//! `returnToParent` (default true), `imageType` (`preview` | `cover`), `transitionDur`
//! (p14:ST_UniversalTimeOffset; absent = the destination's own transition), `showBg`, and the
//! children `p166:blipFill` (the image) and `p166:spPr` (box and outline).
//!
//! Producers wrap the frame in `mc:AlternateContent` (ECMA-376 Part 3, §10.2.1) with a picture
//! fallback (a group of pictures for a summary zoom) carrying `ppaction://hlinksldjump`
//! hyperlinks. As for 3D models, the reader keeps the wrapper in [`GraphicFrame::alt`] and
//! models the frame; the writer puts the frame back, moving the fallback with it.

use bo_drawingml::props::{parse_xfrm, write_xfrm};
use bo_model::slide::{GraphicContent, GraphicFrame, ShapeNode, Xfrm, XmlElement, XmlNode};

/// [MS-PPTX] §5.9 slide zoom namespace (also the `graphicData@uri`).
pub const NS_SLIDE_ZOOM: &str = "http://schemas.microsoft.com/office/powerpoint/2016/slidezoom";
/// [MS-PPTX] §5.8 section zoom namespace.
pub const NS_SECTION_ZOOM: &str = "http://schemas.microsoft.com/office/powerpoint/2016/sectionzoom";
/// [MS-PPTX] §5.10 summary zoom namespace.
pub const NS_SUMMARY_ZOOM: &str = "http://schemas.microsoft.com/office/powerpoint/2016/summaryzoom";
/// [MS-PPTX] §5.7 namespace of CT_ZoomObjectProperties.
pub const NS_P166: &str = "http://schemas.microsoft.com/office/powerpoint/2016/6/main";
const MC_NS: &str = "http://schemas.openxmlformats.org/markup-compatibility/2006";
/// `a:hlinkClick@action` of the fallback pictures (jump to the hyperlink's slide).
pub const ACTION_SLIDE_JUMP: &str = "ppaction://hlinksldjump";

/// Kind of zoom.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ZoomKind {
    /// Slide zoom (`pslz:sldZm`).
    Slide,
    /// Section zoom (`psez:sectionZm`).
    Section,
    /// Summary zoom (`psuz:summaryZm`).
    Summary,
}

impl ZoomKind {
    /// Namespace (and `graphicData@uri`) of the kind.
    pub fn ns(self) -> &'static str {
        match self {
            ZoomKind::Slide => NS_SLIDE_ZOOM,
            ZoomKind::Section => NS_SECTION_ZOOM,
            ZoomKind::Summary => NS_SUMMARY_ZOOM,
        }
    }
    fn prefix(self) -> &'static str {
        match self {
            ZoomKind::Slide => "pslz",
            ZoomKind::Section => "psez",
            ZoomKind::Summary => "psuz",
        }
    }
    fn container(self) -> &'static str {
        match self {
            ZoomKind::Slide => "sldZm",
            ZoomKind::Section => "sectionZm",
            ZoomKind::Summary => "summaryZm",
        }
    }
    fn object(self) -> &'static str {
        match self {
            ZoomKind::Slide => "sldZmObj",
            ZoomKind::Section => "sectionZmObj",
            ZoomKind::Summary => "summaryZmObj",
        }
    }
}

/// Kind of a `graphicData@uri`.
pub fn kind_of_uri(uri: &str) -> Option<ZoomKind> {
    [ZoomKind::Slide, ZoomKind::Section, ZoomKind::Summary]
        .into_iter()
        .find(|k| k.ns() == uri)
}

/// Kind of zoom of a frame (`None`: not a zoom).
pub fn zoom_kind(f: &GraphicFrame) -> Option<ZoomKind> {
    kind_of_uri(&f.uri)
}

/// Is this frame a zoom?
pub fn is_zoom(f: &GraphicFrame) -> bool {
    zoom_kind(f).is_some()
}

/// The zoom frame of a shape-tree node.
pub fn node_zoom(n: &ShapeNode) -> Option<&GraphicFrame> {
    match n {
        ShapeNode::Frame(f) if is_zoom(f) => Some(f),
        _ => None,
    }
}

/// What a zoom object leads to.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ZoomTarget {
    /// A slide (`sldZmObj@sldId`, the `p:sldId@id` of presentation.xml).
    Slide {
        /// `p:sldId@id`.
        #[serde(rename = "sldId")]
        sld_id: u32,
    },
    /// A section (`sectionId`, a [MS-PPTX] CT_Section `id`).
    Section {
        /// Section GUID in braces.
        id: String,
    },
}

/// `zmPr` attributes (p166:CT_ZoomObjectProperties) with their schema defaults.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ZoomProps {
    /// `id` (GUID).
    pub id: String,
    /// `returnToParent` (default true): come back to the zoom's slide afterwards.
    pub return_to_parent: bool,
    /// `imageType="cover"`: a custom image instead of the slide preview.
    pub cover: bool,
    /// `transitionDur` in ms; `None`: the destination slide's own transition.
    pub transition_ms: Option<u32>,
    /// `showBg` (default true): the zoom uses the destination's background.
    pub show_bg: bool,
}

impl Default for ZoomProps {
    fn default() -> Self {
        ZoomProps {
            id: String::new(),
            return_to_parent: true,
            cover: false,
            transition_ms: None,
            show_bg: true,
        }
    }
}

/// One zoom object (one target).
#[derive(Debug, Clone, PartialEq)]
pub struct ZoomObject {
    /// Target.
    pub target: ZoomTarget,
    /// `zmPr` attributes.
    pub props: ZoomProps,
    /// `zmPr/spPr/a:xfrm` box `[x, y, cx, cy]` in EMU (slide coordinates).
    pub rect: Option<[i64; 4]>,
    /// `zmPr/blipFill/a:blip@r:embed`.
    pub image_rid: Option<String>,
    /// Summary objects: `title` / `descr` (alt text).
    pub title: Option<String>,
}

fn rid_attr(el: &XmlElement, local: &str) -> Option<String> {
    el.attrs
        .iter()
        .find(|(k, _)| k.ends_with(&format!(":{local}")) && !k.starts_with("xmlns"))
        .map(|(_, v)| v.clone())
}

fn child<'a>(el: &'a XmlElement, local: &str) -> Option<&'a XmlElement> {
    el.elements().find(|e| e.local() == local)
}

fn child_mut<'a>(el: &'a mut XmlElement, local: &str) -> Option<&'a mut XmlElement> {
    el.children.iter_mut().find_map(|c| match c {
        XmlNode::Element(e) if e.local() == local => Some(e),
        _ => None,
    })
}

fn xml_bool(v: &str) -> Option<bool> {
    match v.trim() {
        "1" | "true" => Some(true),
        "0" | "false" => Some(false),
        _ => None,
    }
}

/// [MS-PPTX] §2.3.4.6 ST_UniversalTimeOffset in ms: a number, then `h`, `min`, `s`, `ms`,
/// `µs`, `ns` or nothing (ms).
pub fn parse_time_offset(s: &str) -> Option<f64> {
    let s = s.trim();
    let digits = s
        .find(|c: char| !(c.is_ascii_digit() || c == '.'))
        .unwrap_or(s.len());
    let (num, unit) = s.split_at(digits);
    let v: f64 = num.parse().ok()?;
    let k = match unit {
        "" | "ms" => 1.0,
        "h" => 3_600_000.0,
        "min" => 60_000.0,
        "s" => 1000.0,
        "µs" => 0.001,
        "ns" => 0.000_001,
        _ => return None,
    };
    Some(v * k).filter(|t| t.is_finite() && *t >= 0.0)
}

fn container(f: &GraphicFrame) -> Option<&XmlElement> {
    let kind = zoom_kind(f)?;
    match &f.content {
        GraphicContent::Other(gd) => child(gd, kind.container()),
        GraphicContent::Table(_) => None,
    }
}

fn container_mut(f: &mut GraphicFrame) -> Option<&mut XmlElement> {
    let kind = zoom_kind(f)?;
    match &mut f.content {
        GraphicContent::Other(gd) => child_mut(gd, kind.container()),
        GraphicContent::Table(_) => None,
    }
}

fn read_props(zm: &XmlElement) -> ZoomProps {
    let d = ZoomProps::default();
    ZoomProps {
        id: zm.attr("id").unwrap_or_default().to_owned(),
        return_to_parent: zm
            .attr("returnToParent")
            .and_then(xml_bool)
            .unwrap_or(d.return_to_parent),
        cover: zm.attr("imageType") == Some("cover"),
        transition_ms: zm
            .attr("transitionDur")
            .and_then(parse_time_offset)
            .map(|t| t.round().min(f64::from(u32::MAX)) as u32),
        show_bg: zm.attr("showBg").and_then(xml_bool).unwrap_or(d.show_bg),
    }
}

fn read_object(kind: ZoomKind, o: &XmlElement) -> Option<ZoomObject> {
    let target = match kind {
        ZoomKind::Slide => ZoomTarget::Slide {
            sld_id: o.attr("sldId")?.trim().parse().ok()?,
        },
        ZoomKind::Section | ZoomKind::Summary => ZoomTarget::Section {
            id: o.attr("sectionId")?.to_owned(),
        },
    };
    let zm = child(o, "zmPr");
    let rect = zm
        .and_then(|z| child(z, "spPr"))
        .and_then(|sp| sp.elements().find(|e| e.local() == "xfrm"))
        .map(parse_xfrm)
        .and_then(|x| {
            let (ox, oy) = x.off?;
            let (cx, cy) = x.ext?;
            Some([ox, oy, cx, cy])
        });
    let image_rid = zm
        .and_then(|z| child(z, "blipFill"))
        .and_then(|b| child(b, "blip"))
        .and_then(|b| rid_attr(b, "embed"));
    Some(ZoomObject {
        target,
        props: zm.map(read_props).unwrap_or_default(),
        rect,
        image_rid,
        title: o.attr("title").filter(|t| !t.is_empty()).map(str::to_owned),
    })
}

/// The objects of a zoom frame in document order (none for other frames).
pub fn zoom_objects(f: &GraphicFrame) -> Vec<ZoomObject> {
    let (Some(kind), Some(c)) = (zoom_kind(f), container(f)) else {
        return Vec::new();
    };
    c.elements()
        .filter(|e| e.local() == kind.object())
        .filter_map(|o| read_object(kind, o))
        .collect()
}

/// The objects of a zoom frame as pictures in the frame's own coordinates (EMU from the
/// frame's top-left corner): image from `p166:blipFill`, outline and geometry from
/// `p166:spPr`. A slide or section zoom's picture fills the frame.
pub fn object_pictures(f: &GraphicFrame) -> Vec<ShapeNode> {
    let (Some(kind), Some(c)) = (zoom_kind(f), container(f)) else {
        return Vec::new();
    };
    let (fx, fy) = f.xfrm.off.unwrap_or((0, 0));
    let (fw, fh) = f.xfrm.ext.unwrap_or((0, 0));
    let mut out = Vec::new();
    for o in c.elements().filter(|e| e.local() == kind.object()) {
        let Some(zm) = child(o, "zmPr") else { continue };
        let blip = child(zm, "blipFill")
            .map(bo_drawingml::props::parse_blip_fill)
            .unwrap_or_default();
        let mut props = child(zm, "spPr")
            .map(bo_drawingml::props::parse_sp_pr)
            .unwrap_or_default();
        let r = match kind {
            ZoomKind::Summary => read_object(kind, o)
                .and_then(|z| z.rect)
                .map(|r| [r[0] - fx, r[1] - fy, r[2], r[3]]),
            _ => Some([0, 0, fw, fh]),
        };
        let Some(r) = r else { continue };
        let mut x = props.xfrm.unwrap_or_default();
        x.off = Some((r[0], r[1]));
        x.ext = Some((r[2], r[3]));
        x.rot = None;
        props.xfrm = Some(x);
        out.push(ShapeNode::Picture(Box::new(bo_model::slide::Picture {
            blip,
            props,
            ..Default::default()
        })));
    }
    out
}

/// Mutable object elements of a zoom frame (`sldZmObj` / `sectionZmObj` / `summaryZmObj`).
pub fn object_elements_mut(f: &mut GraphicFrame) -> Vec<&mut XmlElement> {
    let Some(kind) = zoom_kind(f) else {
        return Vec::new();
    };
    let Some(c) = container_mut(f) else {
        return Vec::new();
    };
    c.children
        .iter_mut()
        .filter_map(|n| match n {
            XmlNode::Element(e) if e.local() == kind.object() => Some(e),
            _ => None,
        })
        .collect()
}

fn set_attr(e: &mut XmlElement, k: &str, v: Option<String>) {
    let at = e.attrs.iter().position(|(n, _)| n == k);
    match (at, v) {
        (Some(i), Some(v)) => e.attrs[i].1 = v,
        (Some(i), None) => {
            e.attrs.remove(i);
        }
        (None, Some(v)) => e.attrs.push((k.to_owned(), v)),
        (None, None) => {}
    }
}

/// Changes to `zmPr` attributes (`None` fields stay).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PropsChange {
    /// `returnToParent`.
    pub return_to_parent: Option<bool>,
    /// `imageType`: true = `cover`.
    pub cover: Option<bool>,
    /// `transitionDur`: `Some(None)` removes it (destination's transition).
    pub transition_ms: Option<Option<u32>>,
    /// `showBg`.
    pub show_bg: Option<bool>,
}

/// Apply `ch` to the `zmPr` of object element `obj`; attributes left at their schema default
/// are written out only when they were present (unknown attributes stay as read).
pub fn set_props(obj: &mut XmlElement, ch: &PropsChange) {
    let Some(zm) = child_mut(obj, "zmPr") else {
        return;
    };
    let b = |v: bool| if v { "1".to_owned() } else { "0".to_owned() };
    if let Some(r) = ch.return_to_parent {
        let keep = zm.attr("returnToParent").is_some();
        set_attr(zm, "returnToParent", (!r || keep).then(|| b(r)));
    }
    if let Some(c) = ch.cover {
        let keep = zm.attr("imageType").is_some();
        set_attr(
            zm,
            "imageType",
            (c || keep).then(|| if c { "cover" } else { "preview" }.to_owned()),
        );
    }
    if let Some(t) = ch.transition_ms {
        set_attr(zm, "transitionDur", t.map(|ms| ms.to_string()));
    }
    if let Some(s) = ch.show_bg {
        let keep = zm.attr("showBg").is_some();
        set_attr(zm, "showBg", (!s || keep).then(|| b(s)));
    }
}

/// Point `zmPr/blipFill/a:blip@r:embed` of object element `obj` at `rid`.
pub fn set_image_rid(obj: &mut XmlElement, rid: &str) {
    let Some(blip) = child_mut(obj, "zmPr")
        .and_then(|z| child_mut(z, "blipFill"))
        .and_then(|b| child_mut(b, "blip"))
    else {
        return;
    };
    let key = blip
        .attrs
        .iter()
        .find(|(k, _)| k.ends_with(":embed") && !k.starts_with("xmlns"))
        .map_or_else(|| "r:embed".to_owned(), |(k, _)| k.clone());
    set_attr(blip, &key, Some(rid.to_owned()));
}

fn set_obj_xfrm(obj: &mut XmlElement, r: [i64; 4]) {
    let Some(sp) = child_mut(obj, "zmPr").and_then(|z| child_mut(z, "spPr")) else {
        return;
    };
    let mut x = sp
        .elements()
        .find(|e| e.local() == "xfrm")
        .map(parse_xfrm)
        .unwrap_or_default();
    x.off = Some((r[0], r[1]));
    x.ext = Some((r[2].max(1), r[3].max(1)));
    let new = write_xfrm(&x, "a:xfrm");
    match sp
        .children
        .iter()
        .position(|n| matches!(n, XmlNode::Element(e) if e.local() == "xfrm"))
    {
        Some(p) => sp.children[p] = XmlNode::Element(new),
        None => sp.children.insert(0, XmlNode::Element(new)),
    }
}

/// Is `el` an `mc:AlternateContent` whose choice holds a zoom frame? Gives that frame.
pub fn alternate_zoom_frame(el: &XmlElement) -> Option<&XmlElement> {
    if el.local() != "AlternateContent" {
        return None;
    }
    el.elements()
        .filter(|c| c.local() == "Choice")
        .find_map(|c| {
            let mut frames = c.elements().filter(|e| e.name == "p:graphicFrame");
            let f = frames.next()?;
            (frames.next().is_none() && c.elements().count() == 1).then_some(f)
        })
        .filter(|f| frame_uri(f).and_then(kind_of_uri).is_some())
}

fn frame_uri(f: &XmlElement) -> Option<&str> {
    f.elements()
        .find(|g| g.name == "a:graphic")
        .and_then(|g| g.child("a:graphicData"))
        .and_then(|d| d.attr("uri"))
}

fn box_of(x: &Xfrm) -> Option<[f64; 4]> {
    let (ox, oy) = x.off?;
    let (cx, cy) = x.ext?;
    Some([ox as f64, oy as f64, cx as f64, cy as f64])
}

/// Map `r` from box `from` to box `to` (both `[x, y, cx, cy]`).
fn map_rect(r: [i64; 4], from: [f64; 4], to: [f64; 4]) -> [i64; 4] {
    let sx = if from[2] > 0.0 { to[2] / from[2] } else { 1.0 };
    let sy = if from[3] > 0.0 { to[3] / from[3] } else { 1.0 };
    [
        (to[0] + (r[0] as f64 - from[0]) * sx).round() as i64,
        (to[1] + (r[1] as f64 - from[1]) * sy).round() as i64,
        (r[2] as f64 * sx).round() as i64,
        (r[3] as f64 * sy).round() as i64,
    ]
}

/// Keep the object boxes (`zmPr/spPr/a:xfrm`) in the frame after it moved or was resized
/// from `old`: a slide or section zoom's object is the frame's box, a summary zoom's objects
/// move and scale with the frame.
pub fn sync_rects(f: &mut GraphicFrame, old: &Xfrm) {
    let Some(new) = box_of(&f.xfrm) else { return };
    let kind = zoom_kind(f);
    let from = box_of(old);
    for o in object_elements_mut(f) {
        let r = match (kind, from) {
            (Some(ZoomKind::Summary), Some(from)) => {
                let Some(cur) = read_object(ZoomKind::Summary, o).and_then(|z| z.rect) else {
                    continue;
                };
                map_rect(cur, from, new)
            }
            _ => [new[0] as i64, new[1] as i64, new[2] as i64, new[3] as i64],
        };
        set_obj_xfrm(o, r);
    }
}

/// The frame as read from its wrapper (to tell whether it moved).
fn read_frame(alt: &XmlElement) -> Option<GraphicFrame> {
    match alternate_zoom_frame(alt).map(crate::parse_node) {
        Some(ShapeNode::Frame(f)) => Some(*f),
        _ => None,
    }
}

/// Has the frame moved or been resized since the wrapper was read? Gives the old box.
pub fn moved_from(f: &GraphicFrame, alt: &XmlElement) -> Option<Xfrm> {
    match read_frame(alt) {
        Some(o) if o.xfrm != f.xfrm => Some(o.xfrm),
        Some(_) => None,
        None => Some(Xfrm::default()),
    }
}

fn set_group_xfrm(g: &mut XmlElement, f: &GraphicFrame) {
    let Some(sp) = child_mut(g, "grpSpPr") else {
        return;
    };
    let mut x = sp
        .elements()
        .find(|e| e.local() == "xfrm")
        .map(parse_xfrm)
        .unwrap_or_default();
    x.off = f.xfrm.off;
    x.ext = f.xfrm.ext;
    if x.ch_off.is_none() || x.ch_ext.is_none() {
        x.ch_off = f.xfrm.off;
        x.ch_ext = f.xfrm.ext;
    }
    let new = write_xfrm(&x, "a:xfrm");
    match sp
        .children
        .iter()
        .position(|n| matches!(n, XmlNode::Element(e) if e.local() == "xfrm"))
    {
        Some(p) => sp.children[p] = XmlNode::Element(new),
        None => sp.children.insert(0, XmlNode::Element(new)),
    }
}

/// The wrapper to write for a zoom frame: `alt` with its choice frame replaced by
/// `frame_xml`. When the frame moved or was resized since it was read, the fallback
/// picture (or group of pictures) follows; when it was renamed, the fallback's name too.
/// Untouched zooms are written back as read.
pub fn write_alternate(f: &GraphicFrame, alt: &XmlElement, frame_xml: &XmlElement) -> XmlElement {
    let old = read_frame(alt);
    let moved = old.as_ref().is_none_or(|o| o.xfrm != f.xfrm);
    let renamed = old
        .as_ref()
        .is_none_or(|o| o.nv.name != f.nv.name || o.nv.descr != f.nv.descr);
    let mut out = alt.clone();
    for c in &mut out.children {
        let XmlNode::Element(c) = c else { continue };
        match c.local() {
            "Choice"
                if c.elements()
                    .any(|e| e.name == "p:graphicFrame" && frame_uri(e) == Some(&f.uri)) =>
            {
                for n in &mut c.children {
                    if matches!(n, XmlNode::Element(e) if e.name == "p:graphicFrame") {
                        *n = XmlNode::Element(frame_xml.clone());
                    }
                }
            }
            "Fallback" => {
                for n in &mut c.children {
                    let XmlNode::Element(e) = n else { continue };
                    match e.local() {
                        "pic" => {
                            if moved {
                                let mut x =
                                    crate::model3d::parse_xfrm_of_pic(e).unwrap_or_default();
                                x.off = f.xfrm.off;
                                x.ext = f.xfrm.ext;
                                x.rot = f.xfrm.rot;
                                x.flip_h = f.xfrm.flip_h;
                                x.flip_v = f.xfrm.flip_v;
                                crate::model3d::set_xfrm(e, &x);
                            }
                            if renamed {
                                crate::model3d::set_cnv(e, &f.nv);
                            }
                        }
                        "grpSp" => {
                            if moved {
                                set_group_xfrm(e, f);
                            }
                            if renamed {
                                if let Some(nv) = child_mut(e, "nvGrpSpPr") {
                                    crate::model3d::set_cnv(nv, &f.nv);
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    out
}

/// The fallback pictures of a wrapper in order (the picture, or the group's pictures).
fn fallback_pics_mut(alt: &mut XmlElement) -> Vec<&mut XmlElement> {
    let Some(fb) = child_mut(alt, "Fallback") else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for n in &mut fb.children {
        let XmlNode::Element(e) = n else { continue };
        match e.local() {
            "pic" => out.push(e),
            "grpSp" => {
                for k in &mut e.children {
                    if let XmlNode::Element(p) = k {
                        if p.local() == "pic" {
                            out.push(p);
                        }
                    }
                }
            }
            _ => {}
        }
    }
    out
}

/// Point the blip of fallback picture `index` at `rid` (after a new thumbnail or image).
pub fn fallback_set_image(alt: &mut XmlElement, index: usize, rid: &str) {
    let Some(pic) = fallback_pics_mut(alt).into_iter().nth(index) else {
        return;
    };
    let Some(blip) = child_mut(pic, "blipFill").and_then(|b| child_mut(b, "blip")) else {
        return;
    };
    let key = blip
        .attrs
        .iter()
        .find(|(k, _)| k.ends_with(":embed") && !k.starts_with("xmlns"))
        .map_or_else(|| "r:embed".to_owned(), |(k, _)| k.clone());
    set_attr(blip, &key, Some(rid.to_owned()));
}

/// Relationship ids used by the fallback pictures (blips and hyperlinks).
pub fn fallback_rids(alt: &XmlElement) -> Vec<String> {
    fn go(e: &XmlElement, out: &mut Vec<String>) {
        if matches!(e.local(), "blip" | "hlinkClick") {
            for (k, v) in &e.attrs {
                if (k.ends_with(":embed") || k.ends_with(":id"))
                    && !k.starts_with("xmlns")
                    && !v.is_empty()
                {
                    out.push(v.clone());
                }
            }
        }
        for c in e.elements() {
            go(c, out);
        }
    }
    let mut out = Vec::new();
    if let Some(fb) = alt.elements().find(|c| c.local() == "Fallback") {
        go(fb, &mut out);
    }
    out
}

// ---------------------------------------------------------------------------
// New zooms
// ---------------------------------------------------------------------------

/// One object of a new zoom.
#[derive(Debug, Clone, PartialEq)]
pub struct NewObject {
    /// Target.
    pub target: ZoomTarget,
    /// `zmPr` attributes.
    pub props: ZoomProps,
    /// Box `[x, y, cx, cy]` (EMU).
    pub rect: [i64; 4],
    /// Relationship of the image.
    pub image_rid: String,
    /// Relationship of the fallback hyperlink (a slide part); empty = none.
    pub link_rid: String,
    /// Name of the fallback picture (and summary object title).
    pub name: String,
}

fn esc(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            c if (c as u32) < 0x20 && !matches!(c, '\t' | '\n' | '\r') => {}
            c => o.push(c),
        }
    }
    o
}

fn xfrm_xml(tag: &str, r: [i64; 4]) -> String {
    format!(
        r#"<{tag}><a:off x="{}" y="{}"/><a:ext cx="{}" cy="{}"/></{tag}>"#,
        r[0],
        r[1],
        r[2].max(1),
        r[3].max(1)
    )
}

/// Thin grey outline of the zoom images (BlueOffice's own choice).
const OUTLINE: &str =
    r#"<a:ln w="3175"><a:solidFill><a:srgbClr val="BFBFBF"/></a:solidFill></a:ln>"#;

fn zm_pr_xml(o: &NewObject) -> String {
    let p = &o.props;
    let mut attrs = format!(r#" id="{}""#, esc(&p.id));
    if !p.return_to_parent {
        attrs.push_str(r#" returnToParent="0""#);
    }
    if p.cover {
        attrs.push_str(r#" imageType="cover""#);
    }
    if let Some(t) = p.transition_ms {
        attrs.push_str(&format!(r#" transitionDur="{t}""#));
    }
    if !p.show_bg {
        attrs.push_str(r#" showBg="0""#);
    }
    format!(
        concat!(
            // [MS-PPTX] §2.8.3.1 CT_ZoomObjectProperties: blipFill, spPr.
            r#"<p166:zmPr xmlns:p166="{p166}"{attrs}>"#,
            r#"<p166:blipFill><a:blip r:embed="{rid}"/><a:stretch><a:fillRect/></a:stretch></p166:blipFill>"#,
            r#"<p166:spPr>{xfrm}<a:prstGeom prst="rect"><a:avLst/></a:prstGeom>{ln}</p166:spPr></p166:zmPr>"#
        ),
        p166 = NS_P166,
        attrs = attrs,
        rid = esc(&o.image_rid),
        xfrm = xfrm_xml("a:xfrm", o.rect),
        ln = OUTLINE,
    )
}

fn object_xml(kind: ZoomKind, o: &NewObject) -> String {
    let pfx = kind.prefix();
    let target = match &o.target {
        ZoomTarget::Slide { sld_id } => format!(r#" sldId="{sld_id}""#),
        ZoomTarget::Section { id } => format!(r#" sectionId="{}""#, esc(id)),
    };
    let title = if kind == ZoomKind::Summary && !o.name.is_empty() {
        format!(r#" title="{}""#, esc(&o.name))
    } else {
        String::new()
    };
    format!(
        "<{pfx}:{obj}{target}{title}>{zm}</{pfx}:{obj}>",
        obj = kind.object(),
        zm = zm_pr_xml(o),
    )
}

fn fallback_pic(id: u32, o: &NewObject) -> String {
    let link = if o.link_rid.is_empty() {
        String::new()
    } else {
        // ECMA-376 Part 1, §21.1.2.3.5 hlinkClick to the target slide.
        format!(
            r#"<a:hlinkClick r:id="{}" action="{ACTION_SLIDE_JUMP}"/>"#,
            esc(&o.link_rid)
        )
    };
    let open = if link.is_empty() {
        format!(r#"<p:cNvPr id="{id}" name="{}"/>"#, esc(&o.name))
    } else {
        format!(
            r#"<p:cNvPr id="{id}" name="{}">{link}</p:cNvPr>"#,
            esc(&o.name)
        )
    };
    format!(
        concat!(
            // ECMA-376 Part 1, §19.3.1.37 pic.
            r#"<p:pic><p:nvPicPr>{open}<p:cNvPicPr><a:picLocks noGrp="1" noRot="1" noChangeAspect="1"/></p:cNvPicPr><p:nvPr/></p:nvPicPr>"#,
            r#"<p:blipFill><a:blip r:embed="{rid}"/><a:stretch><a:fillRect/></a:stretch></p:blipFill>"#,
            r#"<p:spPr>{xfrm}<a:prstGeom prst="rect"><a:avLst/></a:prstGeom>{ln}</p:spPr></p:pic>"#
        ),
        open = open,
        rid = esc(&o.image_rid),
        xfrm = xfrm_xml("a:xfrm", o.rect),
        ln = OUTLINE,
    )
}

/// Markup of a new zoom: `mc:AlternateContent` with the zoom's choice (a graphic frame
/// `id` / `name` over `rect`) and the fallback picture(s). `fallback_ids` gives the ids of
/// the group's pictures of a summary zoom (one per object).
pub fn node_xml(
    kind: ZoomKind,
    id: u32,
    name: &str,
    rect: [i64; 4],
    objects: &[NewObject],
    fallback_ids: &[u32],
) -> String {
    let pfx = kind.prefix();
    let ns = kind.ns();
    let objs: String = objects.iter().map(|o| object_xml(kind, o)).collect();
    // CT_SummaryZoom: the objects, then the layout choice.
    let layout = if kind == ZoomKind::Summary {
        format!("<{pfx}:gridLayout/>")
    } else {
        String::new()
    };
    let fallback = match kind {
        ZoomKind::Summary => {
            let pics: String = objects
                .iter()
                .enumerate()
                .map(|(i, o)| fallback_pic(fallback_ids.get(i).copied().unwrap_or(id), o))
                .collect();
            format!(
                concat!(
                    // ECMA-376 Part 1, §19.3.1.22 grpSp.
                    r#"<p:grpSp><p:nvGrpSpPr><p:cNvPr id="{id}" name="{name}"/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>"#,
                    r#"<p:grpSpPr><a:xfrm><a:off x="{x}" y="{y}"/><a:ext cx="{cx}" cy="{cy}"/>"#,
                    r#"<a:chOff x="{x}" y="{y}"/><a:chExt cx="{cx}" cy="{cy}"/></a:xfrm></p:grpSpPr>{pics}</p:grpSp>"#
                ),
                id = id,
                name = esc(name),
                x = rect[0],
                y = rect[1],
                cx = rect[2].max(1),
                cy = rect[3].max(1),
                pics = pics,
            )
        }
        _ => objects
            .first()
            .map(|o| {
                let mut o = o.clone();
                o.name = name.to_owned();
                fallback_pic(id, &o)
            })
            .unwrap_or_default(),
    };
    format!(
        concat!(
            // ECMA-376 Part 3, §10.2.1 AlternateContent / §10.2.2 Choice@Requires / §10.2.3 Fallback.
            r#"<mc:AlternateContent xmlns:mc="{mc}"><mc:Choice xmlns:{pfx}="{ns}" Requires="{pfx}">"#,
            // ECMA-376 Part 1, §19.3.1.21 graphicFrame.
            r#"<p:graphicFrame><p:nvGraphicFramePr><p:cNvPr id="{id}" name="{name}"/><p:cNvGraphicFramePr><a:graphicFrameLocks noGrp="1" noChangeAspect="1"/></p:cNvGraphicFramePr><p:nvPr/></p:nvGraphicFramePr>"#,
            "{pxfrm}",
            r#"<a:graphic><a:graphicData uri="{ns}"><{pfx}:{cont}>{objs}{layout}</{pfx}:{cont}></a:graphicData></a:graphic></p:graphicFrame></mc:Choice>"#,
            r#"<mc:Fallback>{fallback}</mc:Fallback></mc:AlternateContent>"#
        ),
        mc = MC_NS,
        pfx = pfx,
        ns = ns,
        id = id,
        name = esc(name),
        pxfrm = xfrm_xml("p:xfrm", rect),
        cont = kind.container(),
        objs = objs,
        layout = layout,
        fallback = fallback,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(xml: &str) -> XmlElement {
        let wrapped = format!(
            r#"<p:spTree xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">{xml}</p:spTree>"#
        );
        let root = bo_drawingml::dom::parse(wrapped.as_bytes(), "/t").expect("xml");
        let first = root.elements().next().expect("child").clone();
        first
    }

    fn obj(target: ZoomTarget, rect: [i64; 4]) -> NewObject {
        NewObject {
            target,
            props: ZoomProps {
                id: "{11111111-2222-4333-8444-555555555555}".into(),
                ..ZoomProps::default()
            },
            rect,
            image_rid: "rId5".into(),
            link_rid: "rId6".into(),
            name: "Phần 1".into(),
        }
    }

    #[test]
    fn slide_zoom_reads_back() {
        let xml = node_xml(
            ZoomKind::Slide,
            4,
            "Thu phóng trang chiếu 3",
            [100, 200, 3000, 2000],
            &[obj(
                ZoomTarget::Slide { sld_id: 257 },
                [100, 200, 3000, 2000],
            )],
            &[],
        );
        let el = parse(&xml);
        assert!(alternate_zoom_frame(&el).is_some());
        let ShapeNode::Frame(f) = crate::parse_node(&el) else {
            panic!("frame")
        };
        assert_eq!(zoom_kind(&f), Some(ZoomKind::Slide));
        assert!(f.alt.is_some());
        let o = zoom_objects(&f);
        assert_eq!(o.len(), 1);
        assert_eq!(o[0].target, ZoomTarget::Slide { sld_id: 257 });
        assert_eq!(o[0].rect, Some([100, 200, 3000, 2000]));
        assert_eq!(o[0].image_rid.as_deref(), Some("rId5"));
        assert!(o[0].props.return_to_parent);
        assert_eq!(o[0].props.transition_ms, None);
        // Unchanged: written back as read.
        let back = crate::write_node(&ShapeNode::Frame(f.clone()));
        assert_eq!(back, el);
    }

    #[test]
    fn props_change_and_defaults() {
        let xml = node_xml(
            ZoomKind::Section,
            4,
            "z",
            [0, 0, 10, 10],
            &[obj(
                ZoomTarget::Section { id: "{A}".into() },
                [0, 0, 10, 10],
            )],
            &[],
        );
        let ShapeNode::Frame(mut f) = crate::parse_node(&parse(&xml)) else {
            panic!("frame")
        };
        for o in object_elements_mut(&mut f) {
            set_props(
                o,
                &PropsChange {
                    return_to_parent: Some(false),
                    cover: Some(true),
                    transition_ms: Some(Some(1500)),
                    show_bg: Some(false),
                },
            );
            set_image_rid(o, "rId9");
        }
        let p = &zoom_objects(&f)[0];
        assert!(!p.props.return_to_parent && p.props.cover && !p.props.show_bg);
        assert_eq!(p.props.transition_ms, Some(1500));
        assert_eq!(p.image_rid.as_deref(), Some("rId9"));
        for o in object_elements_mut(&mut f) {
            set_props(
                o,
                &PropsChange {
                    transition_ms: Some(None),
                    ..PropsChange::default()
                },
            );
        }
        assert_eq!(zoom_objects(&f)[0].props.transition_ms, None);
    }

    /// Markup written by another producer: its own prefixes, `cId`, an `extLst`, a fixed
    /// summary layout with offset / scale factors, a fallback with extra attributes.
    #[test]
    fn foreign_markup_round_trips_and_moves() {
        let xml = concat!(
            r#"<mc:AlternateContent xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006">"#,
            r#"<mc:Choice xmlns:sz="http://schemas.microsoft.com/office/powerpoint/2016/summaryzoom" Requires="sz">"#,
            r#"<p:graphicFrame><p:nvGraphicFramePr><p:cNvPr id="9" name="Tóm tắt" title="x"/><p:cNvGraphicFramePr/><p:nvPr/></p:nvGraphicFramePr>"#,
            r#"<p:xfrm><a:off x="100" y="100"/><a:ext cx="1000" cy="500"/></p:xfrm>"#,
            r#"<a:graphic><a:graphicData uri="http://schemas.microsoft.com/office/powerpoint/2016/summaryzoom"><sz:summaryZm>"#,
            r#"<sz:summaryZmObj sectionId="{S1}" title="Một" descr="d" offsetFactorX="1000" scaleFactorY="90000">"#,
            r#"<q:zmPr xmlns:q="http://schemas.microsoft.com/office/powerpoint/2016/6/main" id="{Z1}" returnToParent="0" transitionDur="2s" imageType="cover">"#,
            r#"<q:blipFill><a:blip r:embed="rId7"/><a:stretch><a:fillRect/></a:stretch></q:blipFill>"#,
            r#"<q:spPr><a:xfrm><a:off x="100" y="100"/><a:ext cx="500" cy="500"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></q:spPr></q:zmPr>"#,
            r#"<sz:extLst><p:ext uri="{X}"><y:z xmlns:y="urn:y" v="1"/></p:ext></sz:extLst></sz:summaryZmObj>"#,
            r#"<sz:fixedLayout/><sz:extLst><p:ext uri="{L}"/></sz:extLst></sz:summaryZm></a:graphicData></a:graphic></p:graphicFrame></mc:Choice>"#,
            r#"<mc:Fallback><p:grpSp><p:nvGrpSpPr><p:cNvPr id="9" name="Tóm tắt"/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>"#,
            r#"<p:grpSpPr><a:xfrm><a:off x="100" y="100"/><a:ext cx="1000" cy="500"/><a:chOff x="100" y="100"/><a:chExt cx="1000" cy="500"/></a:xfrm></p:grpSpPr>"#,
            r#"<p:pic><p:nvPicPr><p:cNvPr id="10" name="Một"><a:hlinkClick r:id="rId8" action="ppaction://hlinksldjump"/></p:cNvPr><p:cNvPicPr/><p:nvPr/></p:nvPicPr>"#,
            r#"<p:blipFill><a:blip r:embed="rId7"/></p:blipFill><p:spPr><a:xfrm><a:off x="100" y="100"/><a:ext cx="500" cy="500"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr></p:pic>"#,
            r#"</p:grpSp></mc:Fallback></mc:AlternateContent>"#
        );
        let el = parse(xml);
        let ShapeNode::Frame(f) = crate::parse_node(&el) else {
            panic!("frame")
        };
        assert_eq!(zoom_kind(&f), Some(ZoomKind::Summary));
        let o = zoom_objects(&f);
        assert_eq!(o.len(), 1);
        assert_eq!(o[0].target, ZoomTarget::Section { id: "{S1}".into() });
        assert!(!o[0].props.return_to_parent && o[0].props.cover);
        assert_eq!(o[0].props.transition_ms, Some(2000));
        assert_eq!(o[0].title.as_deref(), Some("Một"));
        assert_eq!(object_pictures(&f).len(), 1);
        // Untouched: exactly as read.
        assert_eq!(crate::write_node(&ShapeNode::Frame(f.clone())), el);
        // Moved: the object box follows, extension lists and attributes stay.
        let mut moved = f.clone();
        moved.xfrm.off = Some((300, 200));
        let out = crate::write_node(&ShapeNode::Frame(moved));
        let ShapeNode::Frame(back) = crate::parse_node(&out) else {
            panic!("frame")
        };
        assert_eq!(zoom_objects(&back)[0].rect, Some([300, 200, 500, 500]));
        let text = format!("{out:?}");
        for keep in [
            "{X}",
            "{L}",
            "urn:y",
            "offsetFactorX",
            "fixedLayout",
            "title",
        ] {
            assert!(text.contains(keep), "{keep}");
        }
    }

    #[test]
    fn time_offsets() {
        assert_eq!(parse_time_offset("1000"), Some(1000.0));
        assert_eq!(parse_time_offset("1.5s"), Some(1500.0));
        assert_eq!(parse_time_offset("x"), None);
    }

    #[test]
    fn summary_moves_with_its_objects_and_fallback_group() {
        let xml = node_xml(
            ZoomKind::Summary,
            4,
            "Tổng quan",
            [1000, 1000, 2000, 1000],
            &[
                obj(
                    ZoomTarget::Section { id: "{A}".into() },
                    [1000, 1000, 900, 1000],
                ),
                obj(
                    ZoomTarget::Section { id: "{B}".into() },
                    [2100, 1000, 900, 1000],
                ),
            ],
            &[5, 6],
        );
        let ShapeNode::Frame(mut f) = crate::parse_node(&parse(&xml)) else {
            panic!("frame")
        };
        let alt = f.alt.clone().expect("alt");
        assert_eq!(fallback_rids(&alt), vec!["rId6", "rId5", "rId6", "rId5"]);
        f.xfrm.off = Some((0, 0));
        f.xfrm.ext = Some((4000, 2000));
        let mut plain = (*f).clone();
        plain.alt = None;
        let old = moved_from(&f, &alt).expect("moved");
        sync_rects(&mut plain, &old);
        let o = zoom_objects(&plain);
        assert_eq!(o[0].rect, Some([0, 0, 1800, 2000]));
        assert_eq!(o[1].rect, Some([2200, 0, 1800, 2000]));
        let el = crate::write_node(&ShapeNode::Frame(f));
        let fb = el.elements().find(|c| c.local() == "Fallback").expect("fb");
        let g = fb.elements().next().expect("grp");
        let x = g
            .child("p:grpSpPr")
            .and_then(|s| s.child("a:xfrm"))
            .map(parse_xfrm)
            .expect("xfrm");
        assert_eq!(x.off, Some((0, 0)));
        assert_eq!(x.ext, Some((4000, 2000)));
        // Children keep their coordinates: the group maps them.
        assert_eq!(x.ch_off, Some((1000, 1000)));
    }
}
