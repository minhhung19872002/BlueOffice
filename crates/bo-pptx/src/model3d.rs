//! 3D models on slides (CORE-3DMODEL-001): [MS-ODRAWXML] `am3d:model3d`
//! (namespace `http://schemas.microsoft.com/office/drawing/2017/model3d`).
//!
//! A model is a graphic frame (ECMA-376 Part 1, §19.3.1.21) whose `graphicData` holds
//! `am3d:model3d` (CT_Model3D: `spPr`, `camera`, `trans`, `attrSrcUrl`?, `raster`?, `extLst`?,
//! `objViewport` | `winViewport`, `ambientLight`?, lights*) with `r:embed` pointing at the
//! GLB part (`model/gltf-binary`). Producers wrap the frame in `mc:AlternateContent`
//! (ECMA-376 Part 3, §10.2.1) with a `Requires="am3d"` choice and a picture fallback.
//! The reader keeps the wrapper in [`GraphicFrame::alt`] and models the frame itself, so
//! the editor moves, resizes and names it like any frame; the writer puts it back.

use bo_drawingml::props::{parse_xfrm, write_xfrm};
use bo_model::slide::{GraphicContent, GraphicFrame, NonVisual, ShapeNode, XmlElement, XmlNode};
use bo_model3d::view::View3d;

/// [MS-ODRAWXML] model3d namespace (also the `graphicData@uri`).
pub const NS_AM3D: &str = "http://schemas.microsoft.com/office/drawing/2017/model3d";
/// Relationship type of `am3d:model3d@r:embed`.
pub const REL_MODEL3D: &str = "http://schemas.microsoft.com/office/2017/06/relationships/model3d";
/// Content type of a GLB part.
pub const CT_GLB: &str = "model/gltf-binary";
/// `raster@rName` written by BlueOffice.
pub const RENDERER: &str = "BlueOffice";
const MC_NS: &str = "http://schemas.openxmlformats.org/markup-compatibility/2006";

/// Is this frame a 3D model?
pub fn is_model3d(f: &GraphicFrame) -> bool {
    f.uri == NS_AM3D
}

/// The model frame of a shape-tree node.
pub fn node_model(n: &ShapeNode) -> Option<&GraphicFrame> {
    match n {
        ShapeNode::Frame(f) if is_model3d(f) => Some(f),
        _ => None,
    }
}

/// `am3d:model3d` of a frame.
pub fn model_el(f: &GraphicFrame) -> Option<&XmlElement> {
    match &f.content {
        GraphicContent::Other(gd) => gd.elements().find(|e| e.local() == "model3d"),
        GraphicContent::Table(_) => None,
    }
}

fn model_el_mut(f: &mut GraphicFrame) -> Option<&mut XmlElement> {
    match &mut f.content {
        GraphicContent::Other(gd) => gd.children.iter_mut().find_map(|c| match c {
            XmlNode::Element(e) if e.local() == "model3d" => Some(e),
            _ => None,
        }),
        GraphicContent::Table(_) => None,
    }
}

fn rid_attr(el: &XmlElement, local: &str) -> Option<String> {
    el.attrs
        .iter()
        .find(|(k, _)| k.ends_with(&format!(":{local}")) && !k.starts_with("xmlns"))
        .map(|(_, v)| v.clone())
}

/// Relationship id of the GLB part (`model3d@r:embed`, AG_Blob).
pub fn model_rid(f: &GraphicFrame) -> Option<String> {
    model_el(f).and_then(|m| rid_attr(m, "embed"))
}

/// Relationship id of the rendered picture (`raster/blip@r:embed`).
pub fn raster_rid(f: &GraphicFrame) -> Option<String> {
    model_el(f)
        .and_then(|m| m.elements().find(|e| e.local() == "raster"))
        .and_then(|r| r.elements().find(|e| e.local() == "blip"))
        .and_then(|b| rid_attr(b, "embed"))
}

/// The fallback `p:pic` of the wrapper (first picture in `mc:Fallback`).
pub fn fallback_pic(alt: &XmlElement) -> Option<&XmlElement> {
    alt.elements()
        .find(|c| c.local() == "Fallback")
        .and_then(|fb| fb.elements().find(|e| e.local() == "pic"))
}

/// Relationship id of the fallback picture's blip.
pub fn fallback_rid(f: &GraphicFrame) -> Option<String> {
    let pic = fallback_pic(f.alt.as_deref()?)?;
    let bf = pic.elements().find(|e| e.local() == "blipFill")?;
    let blip = bf.elements().find(|e| e.local() == "blip")?;
    rid_attr(blip, "embed")
}

/// Relationship ids of the pictures showing the view (raster and fallback), deduplicated.
pub fn picture_rids(f: &GraphicFrame) -> Vec<String> {
    let mut v: Vec<String> = raster_rid(f).into_iter().chain(fallback_rid(f)).collect();
    v.dedup();
    v
}

fn num(el: &XmlElement, a: &str) -> Option<f64> {
    el.attr(a)
        .and_then(|v| v.trim().parse::<f64>().ok())
        .filter(|x| x.is_finite())
}

fn ratio(el: Option<&XmlElement>) -> Option<f64> {
    let el = el?;
    let (n, d) = (num(el, "n")?, num(el, "d")?);
    (d != 0.0).then(|| n / d)
}

fn child<'a>(el: &'a XmlElement, local: &str) -> Option<&'a XmlElement> {
    el.elements().find(|e| e.local() == local)
}

fn point(el: Option<&XmlElement>, keys: [&str; 3], or: [f64; 3]) -> [f64; 3] {
    match el {
        Some(e) => [
            num(e, keys[0]).unwrap_or(0.0),
            num(e, keys[1]).unwrap_or(0.0),
            num(e, keys[2]).unwrap_or(0.0),
        ],
        None => or,
    }
}

/// The view stored in a model frame (defaults for missing parts).
pub fn read_view(f: &GraphicFrame) -> View3d {
    let mut v = View3d::default();
    let Some(m) = model_el(f) else { return v };
    if let Some(cam) = child(m, "camera") {
        v.camera_pos = point(child(cam, "pos"), ["x", "y", "z"], v.camera_pos);
        v.camera_up = point(child(cam, "up"), ["dx", "dy", "dz"], v.camera_up);
        v.look_at = point(child(cam, "lookAt"), ["x", "y", "z"], v.look_at);
        if let Some(p) = child(cam, "perspective") {
            v.fov = num(p, "fov").or(v.fov);
        } else if let Some(o) = child(cam, "orthographic") {
            v.fov = None;
            v.ortho_size = ratio(child(o, "sz")).unwrap_or(v.ortho_size);
        }
    }
    if let Some(t) = child(m, "trans") {
        v.meter_per_unit = ratio(child(t, "meterPerModelUnit"))
            .filter(|x| *x > 0.0)
            .unwrap_or(1.0);
        v.pre_trans = point(child(t, "preTrans"), ["dx", "dy", "dz"], [0.0; 3]);
        if let Some(s) = child(t, "scale") {
            v.scale = [
                ratio(child(s, "sx")).unwrap_or(1.0),
                ratio(child(s, "sy")).unwrap_or(1.0),
                ratio(child(s, "sz")).unwrap_or(1.0),
            ];
        }
        v.rot = point(child(t, "rot"), ["ax", "ay", "az"], [0.0; 3]);
        v.post_trans = point(child(t, "postTrans"), ["dx", "dy", "dz"], [0.0; 3]);
    }
    v.viewport = child(m, "objViewport").and_then(|o| num(o, "viewportSz"));
    v
}

fn i(x: f64) -> String {
    format!("{}", x.round() as i64)
}

/// `n` / `d` for a positive ratio, `d` a power of ten keeping ~7 significant digits.
fn ratio_attrs(x: f64) -> (String, String) {
    let x = if x.is_finite() { x.abs() } else { 1.0 };
    let mut d: u64 = 1_000_000;
    while x * (d as f64) < 1_000_000.0 && d < 1_000_000_000_000_000 {
        d *= 10;
    }
    while x * (d as f64) > 1e15 && d > 1 {
        d /= 10;
    }
    (format!("{}", (x * d as f64).round() as u64), d.to_string())
}

fn signed_ratio(x: f64) -> (String, String) {
    let x = if x.is_finite() { x } else { 1.0 };
    (format!("{}", (x * 1e6).round() as i64), "1000000".into())
}

/// `am3d:camera` markup (CT_Model3DCamera), without extLst.
fn camera_xml(v: &View3d) -> String {
    let proj = if let Some(f) = v.fov {
        format!(
            r#"<am3d:perspective fov="{}"/>"#,
            (f.round() as i64).clamp(0, 10_800_000)
        )
    } else {
        let (n, d) = ratio_attrs(v.ortho_size);
        format!(r#"<am3d:orthographic><am3d:sz n="{n}" d="{d}"/></am3d:orthographic>"#)
    };
    format!(
        concat!(
            r#"<am3d:camera><am3d:pos x="{}" y="{}" z="{}"/><am3d:up dx="{}" dy="{}" dz="{}"/>"#,
            r#"<am3d:lookAt x="{}" y="{}" z="{}"/>{}</am3d:camera>"#
        ),
        i(v.camera_pos[0]),
        i(v.camera_pos[1]),
        i(v.camera_pos[2]),
        i(v.camera_up[0]),
        i(v.camera_up[1]),
        i(v.camera_up[2]),
        i(v.look_at[0]),
        i(v.look_at[1]),
        i(v.look_at[2]),
        proj
    )
}

/// `am3d:trans` markup (CT_Model3DTransform), without extLst.
fn trans_xml(v: &View3d) -> String {
    let (mn, md) = ratio_attrs(v.meter_per_unit);
    let s: Vec<(String, String)> = v.scale.iter().map(|x| signed_ratio(*x)).collect();
    let ang = |a: f64| (a.round() as i64).rem_euclid(21_600_000);
    format!(
        concat!(
            r#"<am3d:trans><am3d:meterPerModelUnit n="{mn}" d="{md}"/>"#,
            r#"<am3d:preTrans dx="{p0}" dy="{p1}" dz="{p2}"/>"#,
            r#"<am3d:scale><am3d:sx n="{s0}" d="{d0}"/><am3d:sy n="{s1}" d="{d1}"/><am3d:sz n="{s2}" d="{d2}"/></am3d:scale>"#,
            r#"<am3d:rot ax="{ax}" ay="{ay}" az="{az}"/>"#,
            r#"<am3d:postTrans dx="{q0}" dy="{q1}" dz="{q2}"/></am3d:trans>"#
        ),
        mn = mn,
        md = md,
        p0 = i(v.pre_trans[0]),
        p1 = i(v.pre_trans[1]),
        p2 = i(v.pre_trans[2]),
        s0 = s[0].0,
        d0 = s[0].1,
        s1 = s[1].0,
        d1 = s[1].1,
        s2 = s[2].0,
        d2 = s[2].1,
        ax = ang(v.rot[0]),
        ay = ang(v.rot[1]),
        az = ang(v.rot[2]),
        q0 = i(v.post_trans[0]),
        q1 = i(v.post_trans[1]),
        q2 = i(v.post_trans[2]),
    )
}

fn viewport_xml(v: &View3d) -> String {
    match v.viewport {
        Some(s) => format!(r#"<am3d:objViewport viewportSz="{}"/>"#, i(s.max(0.0))),
        None => "<am3d:winViewport/>".into(),
    }
}

/// Parse an element snippet that uses the `am3d` and `a` prefixes.
fn snippet(xml: &str) -> Option<XmlElement> {
    let wrapped = format!(
        r#"<w xmlns:am3d="{NS_AM3D}" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">{xml}</w>"#
    );
    let w = bo_drawingml::dom::parse(wrapped.as_bytes(), "/model3d").ok()?;
    let first = w.elements().next().cloned();
    first
}

/// Replace the element `local` of `m` by `new`, keeping the old element's `extLst`
/// and its prefix (`am3d:`, or the prefix the file used).
fn replace_child(m: &mut XmlElement, local: &str, alt_local: Option<&str>, mut new: XmlElement) {
    let prefix = m.name.split_once(':').map(|(p, _)| p.to_owned());
    let rename = |e: &mut XmlElement| {
        fn go(e: &mut XmlElement, p: &str) {
            if let Some(rest) = e.name.strip_prefix("am3d:") {
                e.name = format!("{p}:{rest}");
            }
            for c in &mut e.children {
                if let XmlNode::Element(c) = c {
                    go(c, p);
                }
            }
        }
        if let Some(p) = &prefix {
            go(e, p);
        }
    };
    rename(&mut new);
    let pos = m.children.iter().position(
        |c| matches!(c, XmlNode::Element(e) if e.local() == local || Some(e.local()) == alt_local),
    );
    if let Some(pos) = pos {
        if let XmlNode::Element(old) = &m.children[pos] {
            if let Some(ext) = old.elements().find(|e| e.local() == "extLst") {
                new.children.push(XmlNode::Element(ext.clone()));
            }
        }
        m.children[pos] = XmlNode::Element(new);
    }
}

/// Write a view into a model frame (`camera`, `trans` and the viewport are replaced in
/// place, their `extLst` kept).
pub fn write_view(f: &mut GraphicFrame, v: &View3d) {
    let Some(m) = model_el_mut(f) else { return };
    if let Some(c) = snippet(&camera_xml(v)) {
        replace_child(m, "camera", None, c);
    }
    if let Some(t) = snippet(&trans_xml(v)) {
        replace_child(m, "trans", None, t);
    }
    if let Some(vp) = snippet(&viewport_xml(v)) {
        replace_child(m, "objViewport", Some("winViewport"), vp);
    }
}

/// Markup of a new model: `mc:AlternateContent` with the `am3d` choice (graphic frame
/// holding `am3d:model3d`) and a `p:pic` fallback showing `image_rid`.
#[allow(clippy::too_many_arguments)]
pub fn node_xml(
    id: u32,
    name: &str,
    descr: &str,
    off: (i64, i64),
    ext: (i64, i64),
    model_rid: &str,
    image_rid: &str,
    view: &View3d,
) -> String {
    let xfrm = |tag: &str| {
        format!(
            r#"<{tag}><a:off x="{}" y="{}"/><a:ext cx="{}" cy="{}"/></{tag}>"#,
            off.0,
            off.1,
            ext.0.max(1),
            ext.1.max(1)
        )
    };
    let nv = format!(
        r#"<p:cNvPr id="{id}" name="{}"{}/>"#,
        esc(name),
        if descr.is_empty() {
            String::new()
        } else {
            format!(r#" descr="{}""#, esc(descr))
        }
    );
    let light = |x: i64, y: i64, z: i64, lum: u32| {
        format!(
            concat!(
                r#"<am3d:dirLight angularRad="0"><am3d:clr><a:scrgbClr r="100000" g="100000" b="100000"/></am3d:clr>"#,
                r#"<am3d:illuminance n="{lum}" d="1000000"/><am3d:pos x="{x}" y="{y}" z="{z}"/>"#,
                r#"<am3d:lookAt x="0" y="0" z="0"/></am3d:dirLight>"#
            ),
            lum = lum,
            x = x,
            y = y,
            z = z
        )
    };
    format!(
        concat!(
            // ECMA-376 Part 3, §10.2.1 AlternateContent / §10.2.2 Choice@Requires / §10.2.3 Fallback.
            r#"<mc:AlternateContent xmlns:mc="{mc}"><mc:Choice xmlns:am3d="{am3d}" Requires="am3d">"#,
            // ECMA-376 Part 1, §19.3.1.21 graphicFrame.
            r#"<p:graphicFrame><p:nvGraphicFramePr>{nv}<p:cNvGraphicFramePr><a:graphicFrameLocks noGrp="1" noChangeAspect="1"/></p:cNvGraphicFramePr><p:nvPr/></p:nvGraphicFramePr>"#,
            "{pxfrm}",
            r#"<a:graphic><a:graphicData uri="{am3d}"><am3d:model3d r:embed="{model}">"#,
            // [MS-ODRAWXML] CT_Model3D sequence.
            r#"<am3d:spPr>{axfrm}<a:prstGeom prst="rect"><a:avLst/></a:prstGeom></am3d:spPr>"#,
            "{camera}{trans}",
            r#"<am3d:raster rName="{renderer}" rVer="1"><am3d:blip r:embed="{image}"/></am3d:raster>"#,
            "{viewport}",
            r#"<am3d:ambientLight><am3d:clr><a:scrgbClr r="50000" g="50000" b="50000"/></am3d:clr><am3d:illuminance n="500000" d="1000000"/></am3d:ambientLight>"#,
            "{l1}{l2}",
            r#"</am3d:model3d></a:graphicData></a:graphic></p:graphicFrame></mc:Choice>"#,
            // Fallback picture (§19.3.1.37 pic) for consumers without am3d.
            r#"<mc:Fallback><p:pic><p:nvPicPr>{nv}<p:cNvPicPr><a:picLocks noGrp="1" noChangeAspect="1"/></p:cNvPicPr><p:nvPr/></p:nvPicPr>"#,
            r#"<p:blipFill><a:blip r:embed="{image}"/><a:stretch><a:fillRect/></a:stretch></p:blipFill>"#,
            r#"<p:spPr>{axfrm}<a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr></p:pic></mc:Fallback></mc:AlternateContent>"#
        ),
        mc = MC_NS,
        am3d = NS_AM3D,
        nv = nv,
        pxfrm = xfrm("p:xfrm"),
        axfrm = xfrm("a:xfrm"),
        model = esc(model_rid),
        image = esc(image_rid),
        camera = camera_xml(view),
        trans = trans_xml(view),
        renderer = RENDERER,
        viewport = viewport_xml(view),
        l1 = light(-20_000_000, 30_000_000, 60_000_000, 1_000_000),
        l2 = light(30_000_000, -10_000_000, 40_000_000, 400_000),
    )
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Is `el` an `mc:AlternateContent` whose choice holds a 3D model frame? Gives that frame.
pub fn alternate_model_frame(el: &XmlElement) -> Option<&XmlElement> {
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
        .filter(|f| {
            f.elements()
                .find(|g| g.name == "a:graphic")
                .and_then(|g| g.child("a:graphicData"))
                .and_then(|d| d.attr("uri"))
                == Some(NS_AM3D)
        })
}

pub(crate) fn set_xfrm(el: &mut XmlElement, x: &bo_model::slide::Xfrm) {
    // The picture's spPr / a:xfrm (or am3d:spPr / a:xfrm), replaced in place.
    for c in &mut el.children {
        if let XmlNode::Element(sp) = c {
            if sp.local() == "spPr" {
                let new = write_xfrm(x, "a:xfrm");
                if let Some(pos) = sp
                    .children
                    .iter()
                    .position(|n| matches!(n, XmlNode::Element(e) if e.name == "a:xfrm"))
                {
                    sp.children[pos] = XmlNode::Element(new);
                } else {
                    sp.children.insert(0, XmlNode::Element(new));
                }
            }
        }
    }
}

pub(crate) fn set_cnv(el: &mut XmlElement, nv: &NonVisual) {
    fn go(e: &mut XmlElement, nv: &NonVisual) -> bool {
        if e.local() == "cNvPr" {
            e.attrs.retain(|(k, _)| k != "name" && k != "descr");
            e.attrs.push(("name".into(), nv.name.clone()));
            if let Some(d) = &nv.descr {
                e.attrs.push(("descr".into(), d.clone()));
            }
            return true;
        }
        e.children.iter_mut().any(|c| match c {
            XmlNode::Element(c) => go(c, nv),
            XmlNode::Text(_) => false,
        })
    }
    go(el, nv);
}

/// The wrapper to write for a model frame: `alt` with its choice frame replaced by
/// `frame_xml` (the frame as modelled). When the frame moved, was resized or renamed since
/// it was read, the fallback picture (and `am3d:spPr`) follow, so other software shows the
/// same box; unchanged files are written back as read.
pub fn write_alternate(f: &GraphicFrame, alt: &XmlElement, frame_xml: &XmlElement) -> XmlElement {
    let (moved, renamed) = changes(f, alt);
    let mut out = alt.clone();
    for c in &mut out.children {
        let XmlNode::Element(c) = c else { continue };
        match c.local() {
            "Choice" if alternate_model_frame_in(c) => {
                for n in &mut c.children {
                    if matches!(n, XmlNode::Element(e) if e.name == "p:graphicFrame") {
                        *n = XmlNode::Element(frame_xml.clone());
                    }
                }
            }
            "Fallback" => {
                for n in &mut c.children {
                    if let XmlNode::Element(e) = n {
                        if e.local() == "pic" {
                            if moved {
                                let mut x = parse_xfrm_of_pic(e).unwrap_or_default();
                                x.off = f.xfrm.off;
                                x.ext = f.xfrm.ext;
                                x.rot = f.xfrm.rot;
                                x.flip_h = f.xfrm.flip_h;
                                x.flip_v = f.xfrm.flip_v;
                                set_xfrm(e, &x);
                            }
                            if renamed {
                                set_cnv(e, &f.nv);
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
    out
}

/// (moved or resized, renamed or new alt text) since the wrapper was read.
fn changes(f: &GraphicFrame, alt: &XmlElement) -> (bool, bool) {
    match alternate_model_frame(alt).map(crate::parse_node) {
        Some(ShapeNode::Frame(o)) => (
            o.xfrm != f.xfrm,
            o.nv.name != f.nv.name || o.nv.descr != f.nv.descr,
        ),
        _ => (true, true),
    }
}

/// Has the frame moved or been resized since the wrapper was read?
pub fn moved(f: &GraphicFrame, alt: &XmlElement) -> bool {
    changes(f, alt).0
}

fn alternate_model_frame_in(choice: &XmlElement) -> bool {
    choice.elements().any(|e| {
        e.name == "p:graphicFrame"
            && e.elements()
                .find(|g| g.name == "a:graphic")
                .and_then(|g| g.child("a:graphicData"))
                .and_then(|d| d.attr("uri"))
                == Some(NS_AM3D)
    })
}

pub(crate) fn parse_xfrm_of_pic(pic: &XmlElement) -> Option<bo_model::slide::Xfrm> {
    let sp = pic.elements().find(|e| e.local() == "spPr")?;
    sp.elements().find(|e| e.name == "a:xfrm").map(parse_xfrm)
}

/// Keep `am3d:spPr/a:xfrm` equal to the frame's box (what producers write).
pub fn sync_model_xfrm(f: &mut GraphicFrame) {
    let x = bo_model::slide::Xfrm {
        off: f.xfrm.off,
        ext: f.xfrm.ext,
        ..Default::default()
    };
    if let Some(m) = model_el_mut(f) {
        set_xfrm(m, &x);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NS: &str = r#"xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships""#;

    fn parse(xml: &str) -> XmlElement {
        let w = format!("<w {NS}>{xml}</w>");
        let el = bo_drawingml::dom::parse(w.as_bytes(), "/t").unwrap();
        let first = el.elements().next().unwrap().clone();
        first
    }

    fn canon(e: &XmlElement) -> String {
        bo_drawingml::dom::canonical(e)
    }

    /// Hand-written per the [MS-ODRAWXML] schema: orthographic camera, window viewport,
    /// a point light and an extLst the reader does not model.
    const HAND: &str = r#"<mc:AlternateContent xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"><mc:Choice xmlns:am3d="http://schemas.microsoft.com/office/drawing/2017/model3d" Requires="am3d"><p:graphicFrame><p:nvGraphicFramePr><p:cNvPr id="7" name="Model" descr="A box"/><p:cNvGraphicFramePr/><p:nvPr/></p:nvGraphicFramePr><p:xfrm><a:off x="100" y="200"/><a:ext cx="3000" cy="2000"/></p:xfrm><a:graphic><a:graphicData uri="http://schemas.microsoft.com/office/drawing/2017/model3d"><am3d:model3d r:embed="rId2"><am3d:spPr><a:xfrm><a:off x="100" y="200"/><a:ext cx="3000" cy="2000"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></am3d:spPr><am3d:camera><am3d:pos x="0" y="0" z="5000000"/><am3d:up dx="0" dy="1" dz="0"/><am3d:lookAt x="0" y="0" z="0"/><am3d:orthographic><am3d:sz n="2000000" d="1"/></am3d:orthographic><am3d:extLst><a:ext uri="{X}"/></am3d:extLst></am3d:camera><am3d:trans><am3d:meterPerModelUnit n="1" d="2"/><am3d:rot ax="60000" ay="120000" az="0"/></am3d:trans><am3d:winViewport/><am3d:ptLight rad="0"><am3d:clr><a:srgbClr val="FFFFFF"/></am3d:clr><am3d:intensity n="1" d="1"/><am3d:pos x="1" y="2" z="3"/></am3d:ptLight></am3d:model3d></a:graphicData></a:graphic></p:graphicFrame></mc:Choice><mc:Fallback><p:pic><p:nvPicPr><p:cNvPr id="7" name="Model" descr="A box"/><p:cNvPicPr/><p:nvPr/></p:nvPicPr><p:blipFill><a:blip r:embed="rId3"/><a:stretch><a:fillRect/></a:stretch></p:blipFill><p:spPr><a:xfrm><a:off x="100" y="200"/><a:ext cx="3000" cy="2000"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr></p:pic></mc:Fallback></mc:AlternateContent>"#;

    #[test]
    fn reads_a_model_frame_and_writes_it_back_unchanged() {
        let el = parse(HAND);
        let node = crate::parse_node(&el);
        let f = node_model(&node).expect("model frame");
        assert_eq!(f.nv.id, 7);
        assert_eq!(f.nv.descr.as_deref(), Some("A box"));
        assert_eq!(f.xfrm.ext, Some((3000, 2000)));
        assert_eq!(model_rid(f).as_deref(), Some("rId2"));
        assert_eq!(fallback_rid(f).as_deref(), Some("rId3"));
        assert_eq!(raster_rid(f), None);
        let v = read_view(f);
        assert_eq!(v.fov, None);
        assert_eq!(v.ortho_size, 2_000_000.0);
        assert_eq!(v.meter_per_unit, 0.5);
        assert_eq!(v.rot, [60_000.0, 120_000.0, 0.0]);
        assert_eq!(v.viewport, None);
        assert_eq!(canon(&crate::write_node(&node)), canon(&el));
    }

    #[test]
    fn a_moved_frame_moves_its_fallback_and_a_new_view_keeps_ext_lists() {
        let el = parse(HAND);
        let mut node = crate::parse_node(&el);
        let ShapeNode::Frame(f) = &mut node else {
            panic!()
        };
        f.xfrm.off = Some((500, 600));
        f.nv.descr = Some("Turned".into());
        let mut v = read_view(f);
        v.orbit(30.0, 0.0);
        v.fov = Some(1_800_000.0);
        v.viewport = Some(4000.0);
        write_view(f, &v);
        let out = crate::write_node(&node);
        let back = crate::parse_node(&out);
        let g = node_model(&back).unwrap();
        // A perspective camera does not store the orthographic size.
        let r = read_view(g);
        assert_eq!(
            View3d {
                ortho_size: v.ortho_size,
                ..r
            },
            v
        );
        let xml = canon(&out);
        assert!(xml.contains(r#"<a:ext uri="{X}"/>"#), "{xml}");
        assert!(xml.contains("am3d:objViewport"));
        assert!(!xml.contains("am3d:winViewport"));
        assert!(xml.contains("am3d:ptLight"));
        // Fallback picture and am3d:spPr follow the frame.
        let pic = fallback_pic(g.alt.as_deref().unwrap()).unwrap();
        assert_eq!(parse_xfrm_of_pic(pic).unwrap().off, Some((500, 600)));
        let m = model_el(g).unwrap();
        assert_eq!(parse_xfrm_of_pic(m).unwrap().off, Some((500, 600)));
        assert!(canon(pic).contains(r#"descr="Turned""#));
    }

    #[test]
    fn new_markup_round_trips() {
        let mut v = View3d::fit([-1.0; 3], [1.0; 3]);
        v.viewport = Some(3_000_000.0);
        v.rot = [1_200_000.0, 2_100_000.0, 0.0];
        let xml = node_xml(
            5,
            "3D Model 5",
            "cube",
            (10, 20),
            (3000, 4000),
            "rId1",
            "rId2",
            &v,
        );
        let el = parse(&xml);
        let node = crate::parse_node(&el);
        let f = node_model(&node).expect("model");
        let r = read_view(f);
        assert!((r.meter_per_unit - v.meter_per_unit).abs() < 1e-9);
        assert_eq!(r.rot, v.rot);
        assert_eq!(r.viewport, v.viewport);
        assert_eq!(picture_rids(f), vec!["rId2".to_owned()]);
        assert_eq!(canon(&crate::write_node(&node)), canon(&el));
    }

    /// Children of `el` (local names) against a sequence of (names, min, max) particles.
    fn sequence(el: &XmlElement, seq: &[(&[&str], usize, usize)]) -> Result<(), String> {
        let kids: Vec<&str> = el.elements().map(XmlElement::local).collect();
        let mut i = 0;
        for (names, min, max) in seq {
            let mut n = 0;
            while i < kids.len() && n < *max && names.contains(&kids[i]) {
                i += 1;
                n += 1;
            }
            if n < *min {
                return Err(format!(
                    "{}: expected {names:?} at {i} in {kids:?}",
                    el.name
                ));
            }
        }
        if i == kids.len() {
            Ok(())
        } else {
            Err(format!("{}: unexpected {:?} in {kids:?}", el.name, kids[i]))
        }
    }

    fn need(el: &XmlElement, attrs: &[&str]) -> Result<(), String> {
        for a in attrs {
            if el.attr(a).is_none() {
                return Err(format!("{} lacks @{a}", el.name));
            }
        }
        Ok(())
    }

    /// [MS-ODRAWXML] model3d schema (CT_Model3D and its parts): sequences, choices and
    /// required attributes. (The SDK validator treats graphicData content as lax.)
    fn check_schema(m: &XmlElement) -> Result<(), String> {
        const U: usize = usize::MAX;
        sequence(
            m,
            &[
                (&["spPr"], 1, 1),
                (&["camera"], 1, 1),
                (&["trans"], 1, 1),
                (&["attrSrcUrl"], 0, 1),
                (&["raster"], 0, 1),
                (&["extLst"], 0, 1),
                (&["objViewport", "winViewport"], 1, 1),
                (&["ambientLight"], 0, 1),
                (&["ptLight", "spotLight", "dirLight", "unkLight"], 0, U),
            ],
        )?;
        if rid_attr(m, "embed").is_none() {
            return Err("model3d lacks r:embed".into());
        }
        for c in m.elements() {
            match c.local() {
                "camera" => {
                    sequence(
                        c,
                        &[
                            (&["pos"], 1, 1),
                            (&["up"], 1, 1),
                            (&["lookAt"], 1, 1),
                            (&["orthographic", "perspective"], 1, 1),
                            (&["extLst"], 0, 1),
                        ],
                    )?;
                    if let Some(p) = child(c, "perspective") {
                        need(p, &["fov"])?;
                        let f = num(p, "fov").unwrap_or(-1.0);
                        if !(0.0..=10_800_000.0).contains(&f) {
                            return Err(format!("fov {f}"));
                        }
                    }
                    if let Some(o) = child(c, "orthographic") {
                        sequence(o, &[(&["sz"], 1, 1), (&["extLst"], 0, 1)])?;
                    }
                }
                "trans" => {
                    sequence(
                        c,
                        &[
                            (&["meterPerModelUnit"], 0, 1),
                            (&["preTrans"], 0, 1),
                            (&["scale"], 0, 1),
                            (&["rot"], 0, 1),
                            (&["postTrans"], 0, 1),
                            (&["extLst"], 0, 1),
                        ],
                    )?;
                    if let Some(s) = child(c, "scale") {
                        sequence(s, &[(&["sx"], 1, 1), (&["sy"], 1, 1), (&["sz"], 1, 1)])?;
                    }
                    if let Some(r) = child(c, "meterPerModelUnit") {
                        need(r, &["n", "d"])?;
                    }
                }
                "raster" => need(c, &["rName", "rVer"])?,
                "objViewport" => need(c, &["viewportSz"])?,
                "ambientLight" => sequence(
                    c,
                    &[
                        (&["clr"], 1, 1),
                        (&["illuminance"], 1, 1),
                        (&["extLst"], 0, 1),
                    ],
                )?,
                "ptLight" => {
                    need(c, &["rad"])?;
                    sequence(
                        c,
                        &[
                            (&["clr"], 1, 1),
                            (&["intensity"], 1, 1),
                            (&["pos"], 1, 1),
                            (&["extLst"], 0, 1),
                        ],
                    )?;
                }
                "spotLight" => {
                    need(c, &["rad", "spotAng"])?;
                    sequence(
                        c,
                        &[
                            (&["clr"], 1, 1),
                            (&["intensity"], 1, 1),
                            (&["pos"], 1, 1),
                            (&["lookAt"], 1, 1),
                            (&["extLst"], 0, 1),
                        ],
                    )?;
                }
                "dirLight" => {
                    need(c, &["angularRad"])?;
                    sequence(
                        c,
                        &[
                            (&["clr"], 1, 1),
                            (&["illuminance"], 1, 1),
                            (&["pos"], 1, 1),
                            (&["lookAt"], 1, 1),
                            (&["extLst"], 0, 1),
                        ],
                    )?;
                }
                _ => {}
            }
        }
        Ok(())
    }

    #[test]
    fn written_markup_follows_the_schema() {
        let v = View3d::fit([-1.0; 3], [1.0; 3]);
        let el = parse(&node_xml(5, "M", "", (0, 0), (10, 10), "rId1", "rId2", &v));
        let node = crate::parse_node(&el);
        check_schema(model_el(node_model(&node).unwrap()).unwrap()).unwrap();
        // A view written over the hand-written markup (orthographic → perspective, window →
        // object viewport) stays valid.
        let mut node = crate::parse_node(&parse(HAND));
        let ShapeNode::Frame(f) = &mut node else {
            panic!()
        };
        check_schema(model_el(f).unwrap()).unwrap();
        let mut v = read_view(f);
        v.fov = Some(1_800_000.0);
        v.viewport = Some(5000.0);
        write_view(f, &v);
        check_schema(model_el(f).unwrap()).unwrap();
        v.fov = None;
        v.viewport = None;
        write_view(f, &v);
        check_schema(model_el(f).unwrap()).unwrap();
        // A broken one is caught.
        let mut bad = model_el(f).unwrap().clone();
        bad.children
            .retain(|c| !matches!(c, XmlNode::Element(e) if e.local() == "winViewport"));
        assert!(check_schema(&bad).is_err());
    }

    #[test]
    fn corpus_models_follow_the_schema() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus/slide");
        let mut seen = 0;
        for name in [
            "slide-model3d-cube.pptx",
            "slide-model3d-scenes.pptx",
            "slide-model3d-handwritten.pptx",
        ] {
            let pres = crate::read_pptx(&std::fs::read(dir.join(name)).unwrap()).unwrap();
            for s in &pres.slides {
                crate::visit(&s.data.shapes, &mut |n| {
                    if let Some(f) = node_model(n) {
                        check_schema(model_el(f).unwrap())
                            .unwrap_or_else(|e| panic!("{name}: {e}"));
                        assert!(fallback_rid(f).is_some(), "{name}: fallback picture");
                        seen += 1;
                    }
                });
            }
        }
        // 1 + 3 + 2 (one inside a group).
        assert_eq!(seen, 6);
    }
}
