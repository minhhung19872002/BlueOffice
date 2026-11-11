//! Groups of shapes in sheet drawings (CORE-SMART-004 "Convert to shapes"): an `xdr:grpSp`
//! (ECMA-376 Part 1, §20.5.2.17) in a two-cell anchor (§20.5.2.33) holding `xdr:sp` shapes
//! (§20.5.2.29: `nvSpPr`, `spPr`, `style`, `txBody`) and nested groups. Shape properties and text
//! bodies are DrawingML (§20.1, §21.1.2), as in a diagram's drawing part.
//!
//! In the model a group is its `xdr:grpSp` element as text
//! ([`bo_model::sheet::SheetShapeGroup::grp_sp`]) whose picture fills name their image part. A
//! group of preset shapes with text is read from the sheet drawing (and stays there as read);
//! a new group is written as a new anchor with image relationships of the drawing part.

use crate::chart::{anchor_cell, parse_dom, Node};
use bo_model::sheet::{SheetShapeGroup, Worksheet};
use bo_opc::xml::escape;
use bo_opc::{resolve_target, Package, Relationship, TargetMode};
use std::fmt::Write as _;

const R_NS: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const REL_IMAGE: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/image";

/// Elements whose children are in the spreadsheet drawing namespace; everything deeper is
/// DrawingML (`a:`).
fn xdr_parent(name: &str) -> bool {
    matches!(name, "grpSp" | "sp" | "nvSpPr" | "nvGrpSpPr")
}

/// Serialize `n` (an element of an `xdr:grpSp`, itself in the `xdr` namespace when `xdr`);
/// `false` when it holds something a group of preset shapes with text does not (a picture, a
/// connector, a graphic frame, a custom geometry).
fn write(n: &Node, xdr: bool, blip: &dyn Fn(&str) -> Option<String>, out: &mut String) -> bool {
    if xdr {
        match n.name.as_str() {
            "grpSp" => {
                if n.children
                    .iter()
                    .any(|c| !matches!(c.name.as_str(), "nvGrpSpPr" | "grpSpPr" | "sp" | "grpSp"))
                {
                    return false;
                }
            }
            "sp" => {
                let preset = n
                    .child("spPr")
                    .is_some_and(|p| p.child("prstGeom").is_some());
                if !preset
                    || n.children
                        .iter()
                        .any(|c| !matches!(c.name.as_str(), "nvSpPr" | "spPr" | "style" | "txBody"))
                {
                    return false;
                }
            }
            _ => {}
        }
    }
    let prefix = if xdr { "xdr" } else { "a" };
    let _ = write!(out, "<{prefix}:{}", n.name);
    for (k, v) in &n.attrs {
        if k == "xmlns" || k.starts_with("xmlns:") {
            continue;
        }
        // §20.1.8.13 blip: the picture is named by its image part.
        if n.name == "blip" && (k == "r:embed" || k == "r:link") {
            if let Some(part) = blip(v) {
                let _ = write!(out, " {k}=\"{}\"", escape(&part));
            }
            continue;
        }
        let _ = write!(out, " {k}=\"{}\"", escape(v));
    }
    if n.children.is_empty() && n.text.is_empty() {
        out.push_str("/>");
        return true;
    }
    out.push('>');
    if n.children.is_empty() {
        out.push_str(&escape(&n.text));
    }
    let inner = xdr && xdr_parent(&n.name);
    for c in &n.children {
        if !write(c, inner, blip, out) {
            return false;
        }
    }
    let _ = write!(out, "</{prefix}:{}>", n.name);
    true
}

/// Groups of preset shapes with text in the drawing of a worksheet (other groups stay opaque).
pub(crate) fn read_groups(
    pkg: &Package,
    sheet_part: &str,
    rels: &[Relationship],
) -> Vec<SheetShapeGroup> {
    let mut out = Vec::new();
    for rel in rels {
        if rel.mode != TargetMode::Internal || !rel.rel_type.ends_with("/drawing") {
            continue;
        }
        let drawing = resolve_target(sheet_part, &rel.target);
        let Some(dom) = pkg
            .part(&drawing)
            .and_then(|p| parse_dom(&p.data, &drawing))
        else {
            continue;
        };
        let drels = pkg.relationships(&drawing).unwrap_or_default();
        let blip = |id: &str| {
            drels
                .iter()
                .find(|r| r.id == id && r.mode == TargetMode::Internal)
                .map(|r| resolve_target(&drawing, &r.target))
                .filter(|p| pkg.part(p).is_some())
        };
        for (i, anchor) in dom.children.iter().enumerate() {
            let Some(grp) = anchor.child("grpSp") else {
                continue;
            };
            let mut xml = String::new();
            if !write(grp, true, &blip, &mut xml) {
                continue;
            }
            out.push(SheetShapeGroup {
                from: anchor.child("from").map(anchor_cell).unwrap_or_default(),
                to: anchor.child("to").map(anchor_cell),
                // Unique in the workbook: the drawing part and the anchor's place in it.
                key: format!("{drawing}#group{i}"),
                grp_sp: xml,
                images: Vec::new(),
                in_file: true,
            });
        }
    }
    out
}

/// Groups of `ws` that are not in the file yet.
pub(crate) fn new_groups(ws: &Worksheet) -> Vec<&SheetShapeGroup> {
    ws.shape_groups.iter().filter(|g| !g.in_file).collect()
}

/// Content type of an image part by its extension.
pub(crate) fn image_type(name: &str) -> &'static str {
    match name
        .rsplit_once('.')
        .map(|(_, e)| e.to_ascii_lowercase())
        .as_deref()
    {
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        _ => "image/png",
    }
}

/// `grp_sp` with every picture fill's part name replaced by a relationship id of the drawing
/// part (`rel`: part name → id) and its `cNvPr@id`s (§20.5.2.8) numbered from `next`.
fn finish(grp_sp: &str, next: &mut u32, rel: &mut dyn FnMut(&str) -> String) -> String {
    let mut out = String::with_capacity(grp_sp.len() + 64);
    let mut rest = grp_sp;
    let mut pictures = false;
    loop {
        let embed = rest.find(" r:embed=\"/");
        let id = rest.find("<xdr:cNvPr id=\"");
        match (embed, id) {
            (Some(e), i) if i.is_none_or(|i| e < i) => {
                let start = e + " r:embed=\"".len();
                let end = rest[start..].find('"').map_or(rest.len(), |k| start + k);
                out.push_str(&rest[..start]);
                out.push_str(&rel(&rest[start..end]));
                pictures = true;
                rest = &rest[end..];
            }
            (_, Some(i)) => {
                let start = i + "<xdr:cNvPr id=\"".len();
                let end = rest[start..].find('"').map_or(rest.len(), |k| start + k);
                out.push_str(&rest[..start]);
                let _ = write!(out, "{next}");
                *next += 1;
                rest = &rest[end..];
            }
            _ => break,
        }
    }
    out.push_str(rest);
    if pictures {
        // The relationship namespace may be undeclared in the drawing part.
        out = out.replacen("<xdr:grpSp>", &format!("<xdr:grpSp xmlns:r=\"{R_NS}\">"), 1);
    }
    out
}

/// Add the image relationships of the new groups `groups` to the `.rels` part `rels` of the
/// drawing part `dpath`, push the image parts they add (those `exists` does not know) to
/// `out_parts` and return the new `.rels` bytes and the anchors.
pub(crate) fn append_new(
    rels: Vec<u8>,
    dpath: &str,
    groups: &[&SheetShapeGroup],
    first_shape_id: u32,
    exists: &dyn Fn(&str) -> bool,
    out_parts: &mut Vec<(String, Option<String>, Vec<u8>)>,
) -> (Vec<u8>, String) {
    let mut base = String::from_utf8(rels).unwrap_or_default();
    let mut add = String::new();
    let mut anchors = String::new();
    let mut next = first_shape_id;
    let mut ids: Vec<(String, String)> = Vec::new();
    let mut n = 1;
    for g in groups {
        let mut rel = |part: &str| {
            if let Some((_, id)) = ids.iter().find(|(p, _)| p == part) {
                return id.clone();
            }
            let id = loop {
                let id = format!("rIdP{n}");
                n += 1;
                if !base.contains(&format!("\"{id}\"")) {
                    break id;
                }
            };
            let _ = write!(
                add,
                "<Relationship Id=\"{id}\" Type=\"{REL_IMAGE}\" Target=\"{}\"/>",
                escape(&crate::write::relative_target(dpath, part))
            );
            ids.push((part.to_owned(), id.clone()));
            id
        };
        let grp = finish(&g.grp_sp, &mut next, &mut rel);
        let to = g.to.unwrap_or(bo_model::sheet::CellRef::new(
            g.from.row.saturating_add(10),
            g.from.col.saturating_add(6),
        ));
        let _ = write!(
            anchors,
            "<xdr:twoCellAnchor editAs=\"oneCell\"><xdr:from><xdr:col>{}</xdr:col><xdr:colOff>0</xdr:colOff><xdr:row>{}</xdr:row><xdr:rowOff>0</xdr:rowOff></xdr:from><xdr:to><xdr:col>{}</xdr:col><xdr:colOff>0</xdr:colOff><xdr:row>{}</xdr:row><xdr:rowOff>0</xdr:rowOff></xdr:to>{grp}<xdr:clientData/></xdr:twoCellAnchor>",
            g.from.col, g.from.row, to.col, to.row
        );
        for (name, bytes) in &g.images {
            if !exists(name) && out_parts.iter().all(|(p, _, _)| p != name) {
                out_parts.push((
                    name.clone(),
                    Some(image_type(name).to_owned()),
                    bytes.clone(),
                ));
            }
        }
    }
    if !add.is_empty() {
        base = match base.rfind("</Relationships>") {
            Some(i) => format!("{}{add}{}", &base[..i], &base[i..]),
            None => format!(
                "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">{add}</Relationships>"
            ),
        };
    }
    (base.into_bytes(), anchors)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    const GRP: &str = r#"<xdr:grpSp><xdr:nvGrpSpPr><xdr:cNvPr id="1" name="Group 1"/><xdr:cNvGrpSpPr/></xdr:nvGrpSpPr><xdr:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="4000" cy="2000"/><a:chOff x="0" y="0"/><a:chExt cx="4000" cy="2000"/></a:xfrm></xdr:grpSpPr><xdr:sp macro="" textlink=""><xdr:nvSpPr><xdr:cNvPr id="2" name="Shape 2"/><xdr:cNvSpPr/></xdr:nvSpPr><xdr:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="1000" cy="500"/></a:xfrm><a:prstGeom prst="ellipse"><a:avLst/></a:prstGeom><a:blipFill><a:blip r:embed="/xl/media/image4.png"/><a:stretch><a:fillRect/></a:stretch></a:blipFill></xdr:spPr><xdr:txBody><a:bodyPr anchor="ctr"/><a:lstStyle/><a:p><a:r><a:rPr lang="vi-VN" sz="1200"/><a:t>Một &amp; hai</a:t></a:r></a:p></xdr:txBody></xdr:sp></xdr:grpSp>"#;

    // source: ECMA-376 Part 1, §20.5.2.17 grpSp / §20.5.2.29 sp in a §20.5.2.33 twoCellAnchor;
    // §20.5.2.8 cNvPr@id; Part 2 relationships for the picture fill's image.
    #[test]
    fn a_new_group_is_an_anchor_with_image_relationships_and_fresh_ids() {
        let g = SheetShapeGroup {
            from: bo_model::sheet::CellRef::new(2, 1),
            to: Some(bo_model::sheet::CellRef::new(12, 7)),
            key: "/xl/shapegroups/drawing1.xml".into(),
            grp_sp: GRP.into(),
            images: vec![("/xl/media/image4.png".into(), vec![1, 2, 3])],
            in_file: false,
        };
        let mut parts = Vec::new();
        let (rels, anchors) = append_new(
            br#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdP1" Type="t" Target="x"/></Relationships>"#.to_vec(),
            "/xl/drawings/drawing1.xml",
            &[&g, &g],
            40,
            &|_| false,
            &mut parts,
        );
        let rels = String::from_utf8(rels).unwrap();
        // One relationship for the image both groups show; the id in use is skipped.
        assert_eq!(rels.matches("relationships/image").count(), 1);
        assert!(rels.contains(r#"<Relationship Id="rIdP2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="../media/image4.png"/>"#), "{rels}");
        assert_eq!(parts.len(), 1);
        assert_eq!(parts[0].1.as_deref(), Some("image/png"));
        assert_eq!(anchors.matches("<xdr:twoCellAnchor ").count(), 2);
        assert!(anchors.contains(
            "<xdr:from><xdr:col>1</xdr:col><xdr:colOff>0</xdr:colOff><xdr:row>2</xdr:row>"
        ));
        assert!(anchors.contains(&format!(
            r#"<xdr:grpSp xmlns:r="{R_NS}"><xdr:nvGrpSpPr><xdr:cNvPr id="40" name="Group 1"/>"#
        )));
        for id in 40..44 {
            assert_eq!(
                anchors.matches(&format!(r#"<xdr:cNvPr id="{id}""#)).count(),
                1
            );
        }
        assert_eq!(anchors.matches(r#"<a:blip r:embed="rIdP2"/>"#).count(), 2);
        // Read back from a drawing part: the same element, the picture by its part name.
        let drawing = format!(
            r#"<xdr:wsDr xmlns:xdr="http://schemas.openxmlformats.org/drawingml/2006/spreadsheetDrawing" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">{anchors}</xdr:wsDr>"#
        );
        let dom = parse_dom(drawing.as_bytes(), "/xl/drawings/drawing1.xml").unwrap();
        let grp = dom.children[0].child("grpSp").unwrap();
        let mut back = String::new();
        assert!(write(
            grp,
            true,
            &|id| (id == "rIdP2").then(|| "/xl/media/image4.png".to_owned()),
            &mut back
        ));
        assert_eq!(
            back,
            GRP.replace(r#"id="1""#, r#"id="40""#)
                .replace(r#"id="2""#, r#"id="41""#)
        );
        // A group with a picture element is not a group of preset shapes: left opaque.
        let pic = parse_dom(
            br#"<xdr:grpSp xmlns:xdr="x"><xdr:nvGrpSpPr/><xdr:grpSpPr/><xdr:pic/></xdr:grpSp>"#,
            "/d",
        )
        .unwrap();
        assert!(!write(&pic, true, &|_| None, &mut String::new()));
    }
}
