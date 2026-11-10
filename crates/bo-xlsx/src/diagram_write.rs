//! SmartArt in sheet drawings (CORE-SMART-005): the parts of new and edited diagrams and their
//! graphic frames. A diagram is an `xdr:graphicFrame` (ECMA-376 Part 1, §20.5.2.16) in a
//! two-cell anchor (§20.5.2.33) whose `a:graphicData` holds `dgm:relIds` (§21.4.2.6) naming the
//! data model, layout, quick style and colours parts by relationships of the sheet drawing part;
//! the drawing part ([MS-ODRAWXML] `dsp:drawing`) is another relationship of it, named by the data
//! model. Diagrams read from the file and left unchanged are not touched (their parts stay
//! byte-for-byte as read).

use bo_model::diagram::{self, DiagramPart};
use bo_model::sheet::{SheetDiagram, Workbook, Worksheet};
use bo_opc::xml::escape;
use std::collections::HashMap;
use std::fmt::Write as _;

/// Diagrams of `ws` that are not in the file yet.
pub(crate) fn new_diagrams(ws: &Worksheet) -> Vec<&SheetDiagram> {
    ws.diagrams
        .iter()
        .filter(|d| d.xml.is_some() && d.rel_dm.is_empty())
        .collect()
}

/// Edited file diagrams: their parts are rewritten in place (relationships unchanged), and moved
/// ones get their anchor cells rewritten in the sheet drawing part.
pub(crate) fn write_edited(
    wb: &Workbook,
    ws: &Worksheet,
    overrides: &mut HashMap<String, Vec<u8>>,
) {
    let opaque = |name: &str| wb.opaque.parts.iter().find(|p| p.name == name);
    for d in ws.diagrams.iter().filter(|d| !d.rel_dm.is_empty()) {
        if let Some(x) = &d.xml {
            let drawing = (d.part != d.data).then_some(d.part.as_str());
            for (name, kind) in [
                (Some(d.data.as_str()), DiagramPart::Data),
                (Some(d.layout.as_str()), DiagramPart::Layout),
                (Some(d.style.as_str()), DiagramPart::Style),
                (Some(d.colors.as_str()), DiagramPart::Colors),
                (drawing, DiagramPart::Drawing),
            ] {
                if let Some(name) = name.filter(|n| opaque(n).is_some()) {
                    overrides.insert(name.to_owned(), x.get(kind).to_vec());
                }
            }
        }
        if d.moved {
            let Some(dpath) = ws
                .rels
                .iter()
                .find(|r| !r.external && r.rel_type.ends_with("/drawing"))
                .map(|r| r.target.clone())
            else {
                continue;
            };
            let text = overrides
                .get(&dpath)
                .cloned()
                .or_else(|| opaque(&dpath).map(|p| p.data.clone()))
                .and_then(|b| String::from_utf8(b).ok());
            if let Some(patched) = text
                .and_then(|t| crate::chart_write::sync_diagram_anchor(&t, &d.rel_dm, d.from, d.to))
            {
                overrides.insert(dpath, patched.into_bytes());
            }
        }
    }
}

/// Add the relationships of the new diagrams `ds` to the `.rels` part `rels` of the drawing part
/// `dpath`, push their parts to `out_parts` and return the new `.rels` bytes and the anchors.
pub(crate) fn append_new(
    rels: Vec<u8>,
    dpath: &str,
    ds: &[&SheetDiagram],
    first_shape_id: u32,
    out_parts: &mut Vec<(String, Option<String>, Vec<u8>)>,
) -> (Vec<u8>, String) {
    let mut base = String::from_utf8(rels).unwrap_or_default();
    let mut add = String::new();
    let mut anchors = String::new();
    let mut n = 1;
    for (i, d) in ds.iter().enumerate() {
        let Some(x) = &d.xml else { continue };
        let names = [
            d.data.clone(),
            d.layout.clone(),
            d.style.clone(),
            d.colors.clone(),
            d.part.clone(),
        ];
        let mut ids = Vec::with_capacity(5);
        for (kind, name) in DiagramPart::ALL.iter().zip(&names) {
            let id = loop {
                let id = format!("rIdG{n}");
                n += 1;
                if !base.contains(&format!("\"{id}\"")) && !add.contains(&format!("\"{id}\"")) {
                    break id;
                }
            };
            let _ = write!(
                add,
                "<Relationship Id=\"{id}\" Type=\"{}\" Target=\"{}\"/>",
                kind.rel_type(),
                escape(&crate::write::relative_target(dpath, name))
            );
            ids.push(id);
        }
        for (kind, name) in DiagramPart::ALL.iter().zip(&names) {
            let bytes = if *kind == DiagramPart::Data {
                diagram::set_drawing_rel(&x.data, &ids[4])
            } else {
                x.get(*kind).to_vec()
            };
            out_parts.push((name.clone(), Some(kind.content_type()), bytes));
        }
        anchors.push_str(&anchor_xml(d, first_shape_id + i as u32, &ids));
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

/// §20.5.2.33 twoCellAnchor holding the §20.5.2.16 graphicFrame of a diagram (`ids`: dm, lo, qs,
/// cs relationship ids).
fn anchor_xml(d: &SheetDiagram, shape_id: u32, ids: &[String]) -> String {
    let to = d.to.unwrap_or(bo_model::sheet::CellRef::new(
        d.from.row.saturating_add(10),
        d.from.col.saturating_add(6),
    ));
    let name = if d.name.is_empty() {
        format!("SmartArt {shape_id}")
    } else {
        d.name.clone()
    };
    format!(
        "<xdr:twoCellAnchor editAs=\"oneCell\"><xdr:from><xdr:col>{}</xdr:col><xdr:colOff>0</xdr:colOff><xdr:row>{}</xdr:row><xdr:rowOff>0</xdr:rowOff></xdr:from><xdr:to><xdr:col>{}</xdr:col><xdr:colOff>0</xdr:colOff><xdr:row>{}</xdr:row><xdr:rowOff>0</xdr:rowOff></xdr:to><xdr:graphicFrame macro=\"\"><xdr:nvGraphicFramePr><xdr:cNvPr id=\"{shape_id}\" name=\"{}\"/><xdr:cNvGraphicFramePr/></xdr:nvGraphicFramePr><xdr:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"0\" cy=\"0\"/></xdr:xfrm><a:graphic><a:graphicData uri=\"{}\">{}</a:graphicData></a:graphic></xdr:graphicFrame><xdr:clientData/></xdr:twoCellAnchor>",
        d.from.col,
        d.from.row,
        to.col,
        to.row,
        escape(&name),
        diagram::URI_DIAGRAM,
        diagram::rel_ids_xml(&ids[0], &ids[1], &ids[2], &ids[3]),
    )
}

/// Remove diagram `index` of sheet `sheet`. A file diagram loses its anchor in the sheet drawing,
/// the drawing's relationships to its parts and the parts no other diagram uses. `false` when
/// there is no such diagram.
pub fn remove_diagram(wb: &mut Workbook, sheet: usize, index: usize) -> bool {
    let Some(ws) = wb.sheets.get_mut(sheet) else {
        return false;
    };
    if index >= ws.diagrams.len() {
        return false;
    }
    let d = ws.diagrams.remove(index);
    if d.rel_dm.is_empty() {
        return true;
    }
    let Some(dpath) = ws
        .rels
        .iter()
        .find(|r| !r.external && r.rel_type.ends_with("/drawing"))
        .map(|r| r.target.clone())
    else {
        return true;
    };
    let still_used: Vec<String> = wb
        .sheets
        .iter()
        .flat_map(|s| &s.diagrams)
        .flat_map(|o| [&o.part, &o.data, &o.layout, &o.style, &o.colors])
        .cloned()
        .collect();
    let gone: Vec<String> = [d.part, d.data, d.layout, d.style, d.colors]
        .into_iter()
        .filter(|n| !n.is_empty() && !still_used.contains(n))
        .collect();
    // Sheet drawing: drop the anchor holding the frame (§20.5.2.33).
    if let Some(p) = wb.opaque.parts.iter_mut().find(|p| p.name == dpath) {
        if let Ok(text) = std::str::from_utf8(&p.data) {
            if let Some((s, e, _)) =
                crate::chart_write::anchor_span_of(text, &format!(":dm=\"{}\"", d.rel_dm))
            {
                let mut t = text.to_owned();
                t.replace_range(s..e, "");
                p.data = t.into_bytes();
            }
        }
    }
    // Its relationships to the removed parts.
    let rels_name = bo_opc::rels_part_name(&dpath);
    if let Some(p) = wb.opaque.parts.iter_mut().find(|p| p.name == rels_name) {
        if let Ok(text) = std::str::from_utf8(&p.data) {
            let mut t = text.to_owned();
            for (id, target) in crate::chart_write::relationship_targets(text) {
                if !gone.contains(&bo_opc::resolve_target(&dpath, &target)) {
                    continue;
                }
                if let Some(at) = t.find(&format!("Id=\"{id}\"")) {
                    if let (Some(s), Some(e)) = (t[..at].rfind('<'), t[at..].find("/>")) {
                        t.replace_range(s..at + e + 2, "");
                    }
                }
            }
            p.data = t.into_bytes();
        }
    }
    wb.opaque.parts.retain(|p| {
        !gone.contains(&p.name) && !gone.iter().any(|g| p.name == bo_opc::rels_part_name(g))
    });
    true
}
