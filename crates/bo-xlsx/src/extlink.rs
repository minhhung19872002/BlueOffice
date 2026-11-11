//! Links to other workbooks (ECMA-376 Part 1, §18.14 External Workbook References).
//!
//! The workbook lists its links in `externalReferences` (§18.2.9); each one is an
//! external link part (§12.3.7) whose `externalBook` (§18.14.7) names the linked file through
//! an external relationship and keeps the sheet names (§18.14.13), defined names
//! (§18.14.5) and the cell values last read from it (`sheetDataSet`, §18.14.10). Formulas
//! refer to link `n` (1-based, in `externalReferences` order) as `[n]Sheet!A1`.
//!
//! Reading summarizes the parts into [`Workbook::external_links`]; the parts themselves stay
//! opaque (written back byte for byte) until [`store`] rewrites one from the summary (new
//! values, another source) or [`remove`] drops it (the link is broken).

use crate::xmlpatch::index;
use bo_model::sheet::{CellError, CellRef, CellValue, ExternalLink, OpaquePart, RawXml, Workbook};
use bo_opc::xml::escape;

/// Relationship type of an external link part (§12.3.7).
pub const REL_EXTERNAL_LINK: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/externalLink";
/// Relationship type of the linked workbook (`externalBook/@r:id`, §18.14.7).
pub const REL_EXTERNAL_PATH: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/externalLinkPath";

const SML_NS: &str = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";
const R_NS: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const PKG_REL_NS: &str = "http://schemas.openxmlformats.org/package/2006/relationships";

/// `r:id` of every `externalReference` (§18.2.8) of the workbook's `externalReferences`
/// fragment, in order.
fn reference_ids(xml: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = xml;
    while let Some(i) = rest.find('<') {
        rest = &rest[i + 1..];
        let end = rest.find('>').unwrap_or(rest.len());
        let tag = &rest[..end];
        let name = tag.split_whitespace().next().unwrap_or("");
        let local = name.rsplit(':').next().unwrap_or(name);
        if local == "externalReference" {
            // The relationship id attribute (`r:id` with any prefix).
            for part in tag.split_whitespace().skip(1) {
                if let Some((k, v)) = part.split_once('=') {
                    if k.rsplit(':').next() == Some("id") && k.contains(':') {
                        let v = v
                            .trim_end_matches('/')
                            .trim_matches(|c| c == '"' || c == '\'');
                        out.push(v.to_owned());
                    }
                }
            }
        }
        rest = &rest[end..];
    }
    out
}

fn is_references(x: &RawXml) -> bool {
    let t = x.xml.trim_start();
    let name = t
        .strip_prefix('<')
        .and_then(|t| {
            t.split(|c: char| c.is_whitespace() || c == '>' || c == '/')
                .next()
        })
        .unwrap_or("");
    name.rsplit(':').next() == Some("externalReferences")
}

fn rels_name(part: &str) -> String {
    bo_opc::rels_part_name(part)
}

/// Target of relationship `id` in the (opaque) relationships part of `part`.
fn rel_target(parts: &[OpaquePart], part: &str, id: &str) -> Option<String> {
    let name = rels_name(part);
    let data = &parts.iter().find(|p| p.name == name)?.data;
    let els = index(data, &name)?;
    els.iter()
        .find(|e| e.local == "Relationship" && e.attr("Id").as_deref() == Some(id))
        .and_then(|e| e.attr("Target"))
}

fn cell_value(t: Option<&str>, v: &str) -> CellValue {
    match t {
        Some("s" | "str" | "inlineStr") => CellValue::Text(v.to_owned()),
        Some("b") => CellValue::Bool(v.trim() == "1" || v.trim().eq_ignore_ascii_case("true")),
        Some("e") => CellValue::Error(CellError::parse(v.trim())),
        _ => v
            .trim()
            .parse::<f64>()
            .map_or_else(|_| CellValue::Text(v.to_owned()), CellValue::Number),
    }
}

/// The summary of external link part `part` (§18.14.7 `externalBook`); `None` for a part
/// that links no workbook (DDE / OLE links, §18.14.4 / §18.14.11).
fn read_part(parts: &[OpaquePart], part: &str, rel_id: &str) -> Option<ExternalLink> {
    let data = &parts.iter().find(|p| p.name == part)?.data;
    let els = index(data, part)?;
    let book = els.iter().position(|e| e.local == "externalBook")?;
    let book_rel = els[book].attr("id").unwrap_or_default();
    let target = rel_target(parts, part, &book_rel).unwrap_or_default();
    let mut link = ExternalLink {
        part: part.to_owned(),
        rel_id: rel_id.to_owned(),
        target,
        ..ExternalLink::default()
    };
    link.sheets = els
        .iter()
        .filter(|e| e.local == "sheetName")
        .map(|e| e.attr("val").unwrap_or_default())
        .collect();
    link.cells = vec![Vec::new(); link.sheets.len()];
    link.names = els
        .iter()
        .filter(|e| e.local == "definedName")
        .filter_map(|e| Some((e.attr("name")?, e.attr("refersTo").unwrap_or_default())))
        .collect();
    let parent_of = |i: usize, local: &str| -> Option<usize> {
        let mut p = els[i].parent;
        while let Some(j) = p {
            if els[j].local == local {
                return Some(j);
            }
            p = els[j].parent;
        }
        None
    };
    for (i, e) in els.iter().enumerate().filter(|(_, e)| e.local == "cell") {
        let Some(sd) = parent_of(i, "sheetData") else {
            continue;
        };
        let Some(sheet) = els[sd]
            .attr("sheetId")
            .and_then(|s| s.trim().parse::<usize>().ok())
        else {
            continue;
        };
        let Some(at) = e.attr("r").and_then(|r| CellRef::parse(&r)) else {
            continue;
        };
        let v = els
            .iter()
            .find(|x| x.parent == Some(i) && x.local == "v")
            .map(|x| x.text.clone());
        let Some(v) = v else { continue };
        if let Some(list) = link.cells.get_mut(sheet) {
            list.push((at.row, at.col, cell_value(e.attr("t").as_deref(), &v)));
        }
    }
    Some(link)
}

/// After reading: the summary of every external workbook link, in `externalReferences`
/// order. A link that is not to a workbook keeps its place with an empty summary (the
/// numbering of the others stays right).
pub(crate) fn resolve(wb: &mut Workbook) {
    let Some(refs) = wb.unknown.iter().find(|x| is_references(x)) else {
        return;
    };
    let ids = reference_ids(&refs.xml);
    wb.external_links = ids
        .iter()
        .map(|id| {
            wb.opaque
                .workbook_rels
                .iter()
                .find(|r| &r.id == id && !r.external)
                .and_then(|r| read_part(&wb.opaque.parts, &r.target, id))
                .unwrap_or_else(|| ExternalLink {
                    rel_id: id.clone(),
                    ..ExternalLink::default()
                })
        })
        .collect();
}

/// The external link part of `link` (§18.14.7) and its relationships part.
fn part_xml(link: &ExternalLink) -> (String, String) {
    let mut s = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
         <externalLink xmlns=\"{SML_NS}\" xmlns:r=\"{R_NS}\"><externalBook r:id=\"rId1\">"
    );
    if !link.sheets.is_empty() {
        s.push_str("<sheetNames>");
        for n in &link.sheets {
            s.push_str(&format!("<sheetName val=\"{}\"/>", escape(n)));
        }
        s.push_str("</sheetNames>");
    }
    if !link.names.is_empty() {
        s.push_str("<definedNames>");
        for (n, r) in &link.names {
            s.push_str(&format!(
                "<definedName name=\"{}\" refersTo=\"{}\"/>",
                escape(n),
                escape(r)
            ));
        }
        s.push_str("</definedNames>");
    }
    if link.cells.iter().any(|c| !c.is_empty()) {
        s.push_str("<sheetDataSet>");
        for (si, cells) in link.cells.iter().enumerate() {
            if cells.is_empty() {
                continue;
            }
            let mut sorted: Vec<&(u32, u32, CellValue)> = cells.iter().collect();
            sorted.sort_by_key(|(r, c, _)| (*r, *c));
            s.push_str(&format!("<sheetData sheetId=\"{si}\">"));
            let mut row: Option<u32> = None;
            for (r, c, v) in sorted {
                if row != Some(*r) {
                    if row.is_some() {
                        s.push_str("</row>");
                    }
                    s.push_str(&format!("<row r=\"{}\">", r + 1));
                    row = Some(*r);
                }
                let at = CellRef { row: *r, col: *c }.to_string();
                // §18.14.1 cell: `t` is ST_CellType (§18.18.11).
                let (t, text) = match v {
                    CellValue::Number(n) => ("n", n.to_string()),
                    CellValue::Bool(b) => ("b", if *b { "1" } else { "0" }.to_owned()),
                    CellValue::Error(e) => ("e", e.as_str().to_owned()),
                    CellValue::Empty => continue,
                    other => ("str", other.as_text().unwrap_or_default()),
                };
                s.push_str(&format!(
                    "<cell r=\"{at}\" t=\"{t}\"><v>{}</v></cell>",
                    escape(&text)
                ));
            }
            if row.is_some() {
                s.push_str("</row>");
            }
            s.push_str("</sheetData>");
        }
        s.push_str("</sheetDataSet>");
    }
    s.push_str("</externalBook></externalLink>");
    let rels = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
         <Relationships xmlns=\"{PKG_REL_NS}\"><Relationship Id=\"rId1\" \
         Type=\"{REL_EXTERNAL_PATH}\" Target=\"{}\" TargetMode=\"External\"/></Relationships>",
        escape(&link.target)
    );
    (s, rels)
}

/// Rewrites the external link part of link `index` (0-based) from its summary (after its
/// values were read again or its source changed). Returns `false` when the link has no part.
pub fn store(wb: &mut Workbook, index: usize) -> bool {
    let Some(link) = wb.external_links.get(index) else {
        return false;
    };
    if link.part.is_empty() {
        return false;
    }
    let (xml, rels) = part_xml(link);
    let part = link.part.clone();
    let rels_part = rels_name(&part);
    let Some(p) = wb.opaque.parts.iter_mut().find(|p| p.name == part) else {
        return false;
    };
    p.data = xml.into_bytes();
    match wb.opaque.parts.iter_mut().find(|p| p.name == rels_part) {
        Some(r) => r.data = rels.into_bytes(),
        None => wb.opaque.parts.push(OpaquePart {
            name: rels_part,
            content_type: None,
            data: rels.into_bytes(),
        }),
    }
    true
}

/// Removes link `index` (0-based): its part, relationships and `externalReference`. The
/// formulas must not refer to it any more (the caller replaced them by their values and
/// renumbered the links after it).
pub fn remove(wb: &mut Workbook, index: usize) {
    if index >= wb.external_links.len() {
        return;
    }
    let link = wb.external_links.remove(index);
    if !link.part.is_empty() {
        let rels = rels_name(&link.part);
        wb.opaque
            .parts
            .retain(|p| p.name != link.part && p.name != rels);
    }
    wb.opaque.workbook_rels.retain(|r| r.id != link.rel_id);
    let ids: Vec<String> = wb.external_links.iter().map(|l| l.rel_id.clone()).collect();
    if ids.is_empty() {
        wb.unknown.retain(|x| !is_references(x));
        return;
    }
    if let Some(x) = wb.unknown.iter_mut().find(|x| is_references(x)) {
        // §18.2.9 externalReferences, §18.2.8 externalReference.
        let mut s = String::from("<externalReferences>");
        for id in ids {
            s.push_str(&format!(
                "<externalReference xmlns:r=\"{R_NS}\" r:id=\"{}\"/>",
                escape(&id)
            ));
        }
        s.push_str("</externalReferences>");
        x.xml = s;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reference_ids_in_order() {
        let xml = r#"<externalReferences><externalReference r:id="rId4"/><x:externalReference x:id="rId7" /></externalReferences>"#;
        assert_eq!(reference_ids(xml), ["rId4", "rId7"]);
    }

    #[test]
    fn part_round_trip() {
        let link = ExternalLink {
            part: "/xl/externalLinks/externalLink1.xml".into(),
            rel_id: "rId9".into(),
            target: "file:///C:/Data/Giá%20bán.xlsx".into(),
            sheets: vec!["Bán hàng".into(), "Kho".into()],
            cells: vec![
                vec![
                    (0, 0, CellValue::Number(12.5)),
                    (0, 1, CellValue::Text("a<b".into())),
                    (2, 0, CellValue::Bool(true)),
                ],
                vec![(1, 1, CellValue::Error(CellError::Div0))],
            ],
            names: vec![("Tỷ_giá".into(), "='Bán hàng'!$A$1".into())],
        };
        let (xml, rels) = part_xml(&link);
        let parts = vec![
            OpaquePart {
                name: link.part.clone(),
                content_type: None,
                data: xml.into_bytes(),
            },
            OpaquePart {
                name: rels_name(&link.part),
                content_type: None,
                data: rels.into_bytes(),
            },
        ];
        assert_eq!(read_part(&parts, &link.part, "rId9"), Some(link));
    }
}
