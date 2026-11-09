//! Slicers ([MS-XLSX] 2.1.x slicer parts): `x14:slicers` (slicers part of a worksheet),
//! `x14:slicerCacheDefinition` (slicer cache part of the workbook, pivot field items in
//! `tabular`, or `x15:tableSlicerCache` for a table column) and the drawing frame whose
//! `sle:slicer` places a slicer on the sheet.
//!
//! The parts stay verbatim in the package; a changed selection rewrites the `s` flags of the
//! cache's `tabular/items/i` elements.

use std::collections::HashMap;

use bo_model::sheet::{CellRef, Slicer, SlicerSource, Table};
use bo_opc::{resolve_target, Package, Relationship, TargetMode};

use crate::chart::{anchor_cell, parse_dom, Node};

/// `http://schemas.microsoft.com/office/2007/relationships/slicer` (worksheet → slicers).
pub const REL_SLICER: &str = "http://schemas.microsoft.com/office/2007/relationships/slicer";
/// `…/slicerCache` (workbook → slicer cache definition).
pub const REL_SLICER_CACHE: &str =
    "http://schemas.microsoft.com/office/2007/relationships/slicerCache";
/// Content type of a slicers part.
pub const CT_SLICER: &str = "application/vnd.ms-excel.slicer+xml";
/// Content type of a slicer cache part.
pub const CT_SLICER_CACHE: &str = "application/vnd.ms-excel.slicerCache+xml";

/// A slicer cache definition as read.
struct Cache {
    part: String,
    source: SlicerSource,
}

fn descendants<'a>(n: &'a Node, name: &str, out: &mut Vec<&'a Node>) {
    for c in &n.children {
        if c.name == name {
            out.push(c);
        }
        descendants(c, name, out);
    }
}

/// Slicer caches of the workbook by name.
fn caches(pkg: &Package, wb_part: &str, tables: &[&Table]) -> HashMap<String, Cache> {
    let mut out = HashMap::new();
    for rel in pkg.relationships(wb_part).unwrap_or_default() {
        if rel.mode != TargetMode::Internal || rel.rel_type != REL_SLICER_CACHE {
            continue;
        }
        let part = resolve_target(wb_part, &rel.target);
        let Some(dom) = pkg.part(&part).and_then(|p| parse_dom(&p.data, &part)) else {
            continue;
        };
        if dom.name != "slicerCacheDefinition" {
            continue;
        }
        let name = dom.attr("name").unwrap_or_default().to_owned();
        let field = dom.attr("sourceName").unwrap_or_default().to_owned();
        let mut table = Vec::new();
        descendants(&dom, "tableSlicerCache", &mut table);
        let source = if let Some(t) = table.first() {
            let table_id = t.attr("tableId").and_then(|v| v.parse().ok()).unwrap_or(0);
            let column = t.attr("column").and_then(|v| v.parse().ok()).unwrap_or(1);
            let shown = tables
                .iter()
                .find(|t| t.id == table_id)
                .and_then(|t| table_shown(t, column));
            SlicerSource::Table {
                table_id,
                column,
                shown,
            }
        } else {
            let pivots = dom
                .child("pivotTables")
                .map(|p| {
                    p.children
                        .iter()
                        .filter_map(|t| t.attr("name").map(str::to_owned))
                        .collect()
                })
                .unwrap_or_default();
            let tabular = dom.child("data").and_then(|d| d.child("tabular"));
            let cache_id = tabular
                .and_then(|t| t.attr("pivotCacheId"))
                .and_then(|v| v.parse().ok())
                .unwrap_or(0);
            let items = tabular
                .and_then(|t| t.child("items"))
                .map(|items| {
                    items
                        .children
                        .iter()
                        .filter(|i| i.name == "i")
                        .map(|i| {
                            (
                                i.attr("x").and_then(|v| v.parse().ok()).unwrap_or(0),
                                matches!(i.attr("s"), Some("1" | "true")),
                            )
                        })
                        .collect()
                })
                .unwrap_or_default();
            SlicerSource::Pivot {
                field,
                pivots,
                cache_id,
                items,
            }
        };
        out.insert(name, Cache { part, source });
    }
    out
}

/// Values listed in the table's `autoFilter` for column `column` (1-based id), or `None` when
/// the column is not filtered.
#[must_use]
pub fn table_shown(t: &Table, column: u32) -> Option<Vec<String>> {
    let xml = t.auto_filter.as_ref()?;
    let index = t.columns.iter().position(|c| c.id == column)?;
    let dom = parse_dom(xml.as_bytes(), "/autoFilter")?;
    let root = if dom.name == "autoFilter" {
        &dom
    } else {
        dom.child("autoFilter")?
    };
    let fc = root
        .children
        .iter()
        .find(|c| c.name == "filterColumn" && c.attr("colId") == Some(&index.to_string()))?;
    let filters = fc.child("filters")?;
    Some(
        filters
            .children
            .iter()
            .filter(|f| f.name == "filter")
            .filter_map(|f| f.attr("val").map(str::to_owned))
            .collect(),
    )
}

/// Drawing anchors of slicer frames by slicer name.
fn anchors(
    pkg: &Package,
    sheet_part: &str,
    rels: &[Relationship],
) -> HashMap<String, (CellRef, Option<CellRef>)> {
    let mut out = HashMap::new();
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
        for anchor in &dom.children {
            let mut found = Vec::new();
            descendants(anchor, "slicer", &mut found);
            for s in found {
                if let Some(name) = s.attr("name") {
                    let from = anchor.child("from").map(anchor_cell).unwrap_or_default();
                    let to = anchor.child("to").map(anchor_cell);
                    out.insert(name.to_owned(), (from, to));
                }
            }
        }
    }
    out
}

/// The slicers of a worksheet.
pub(crate) fn read_slicers(
    pkg: &Package,
    wb_part: &str,
    sheet_part: &str,
    rels: &[Relationship],
    tables: &[&Table],
) -> Vec<Slicer> {
    let parts: Vec<String> = rels
        .iter()
        .filter(|r| r.mode == TargetMode::Internal && r.rel_type == REL_SLICER)
        .map(|r| resolve_target(sheet_part, &r.target))
        .collect();
    if parts.is_empty() {
        return Vec::new();
    }
    let caches = caches(pkg, wb_part, tables);
    let places = anchors(pkg, sheet_part, rels);
    let mut out = Vec::new();
    for part in parts {
        let Some(dom) = pkg.part(&part).and_then(|p| parse_dom(&p.data, &part)) else {
            continue;
        };
        for s in dom.children.iter().filter(|c| c.name == "slicer") {
            let name = s.attr("name").unwrap_or_default().to_owned();
            let cache_name = s.attr("cache").unwrap_or_default().to_owned();
            let Some(cache) = caches.get(&cache_name) else {
                continue;
            };
            let (from, to) = places.get(&name).copied().unwrap_or_default();
            out.push(Slicer {
                caption: s.attr("caption").unwrap_or(&name).to_owned(),
                name,
                cache: cache_name,
                source: cache.source.clone(),
                columns: s
                    .attr("columnCount")
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(1),
                style: s.attr("style").map(str::to_owned),
                row_height: s.attr("rowHeight").and_then(|v| v.parse().ok()),
                from,
                to,
                cache_part: cache.part.clone(),
                part: part.clone(),
                dirty: false,
            });
        }
    }
    out
}

/// The cache part with its `tabular/items/i` selection flags set from `items` (by shared item
/// index); other attributes and content are kept.
#[must_use]
pub fn patch_cache_items(xml: &str, items: &[(u32, bool)]) -> Option<String> {
    let tab = xml.find("tabular")?;
    let open = tab + xml[tab..].find("items")?;
    // Start of the `<…items` tag and the end of its element.
    let tag_start = xml[..open].rfind('<')?;
    let prefix = &xml[tag_start + 1..open];
    let close_tag = format!("</{prefix}items>");
    let end = open + xml[open..].find(&close_tag)? + close_tag.len();
    let selected: HashMap<u32, bool> = items.iter().copied().collect();
    let body_start = open + xml[open..].find('>')? + 1;
    let mut out = String::with_capacity(xml.len());
    out.push_str(&xml[..body_start]);
    // Rewrite each `<{prefix}i …/>` keeping x and nd, setting s.
    let mut rest = &xml[body_start..end - close_tag.len()];
    let open_i = format!("<{prefix}i ");
    while let Some(k) = rest.find(&open_i) {
        out.push_str(&rest[..k]);
        let tag_end = k + rest[k..].find("/>")? + 2;
        let tag = &rest[k..tag_end];
        let attr = |name: &str| {
            let key = format!(" {name}=\"");
            tag.find(&key).and_then(|p| {
                let v = &tag[p + key.len()..];
                v.find('"').map(|q| v[..q].to_owned())
            })
        };
        let x = attr("x").unwrap_or_default();
        let nd = attr("nd");
        let s = x
            .parse::<u32>()
            .ok()
            .and_then(|x| selected.get(&x).copied())
            .unwrap_or(false);
        out.push_str(&format!("<{prefix}i x=\"{x}\""));
        if s {
            out.push_str(" s=\"1\"");
        }
        if let Some(nd) = nd {
            out.push_str(&format!(" nd=\"{nd}\""));
        }
        out.push_str("/>");
        rest = &rest[tag_end..];
    }
    out.push_str(rest);
    out.push_str(&close_tag);
    out.push_str(&xml[end..]);
    Some(out)
}

// ------------------------------------------------------------------ writing new slicers

/// Namespaces of the slicer parts.
const NS_X14: &str = "http://schemas.microsoft.com/office/spreadsheetml/2009/9/main";
const NS_X15: &str = "http://schemas.microsoft.com/office/spreadsheetml/2010/11/main";
const NS_MAIN: &str = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";
const NS_MC: &str = "http://schemas.openxmlformats.org/markup-compatibility/2006";
/// Extension of a worksheet listing its table slicers ([MS-XLSX] `x14:slicerList`).
const EXT_SHEET_TABLE_SLICERS: &str = "{3A4CF648-6AED-40f4-86FF-DC5316D8AED3}";
/// Extension of the workbook listing table slicer caches (`x15:slicerCaches`).
pub(crate) const EXT_WB_TABLE_SLICER_CACHES: &str = "{46F421CA-312F-682f-3DD2-61675219B42D}";
/// Extension of a slicer cache naming its table column (`x15:tableSlicerCache`).
const EXT_TABLE_SLICER_CACHE: &str = "{2F2917AC-EB37-4324-AD4E-5DD8C200BD13}";

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// The `slicerCacheDefinition` of a new table slicer.
fn table_cache_xml(s: &Slicer, table_id: u32, column: u32, source_name: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<slicerCacheDefinition xmlns=\"{NS_X14}\" xmlns:mc=\"{NS_MC}\" mc:Ignorable=\"x\" xmlns:x=\"{NS_MAIN}\" name=\"{}\" sourceName=\"{}\"><extLst><x:ext uri=\"{EXT_TABLE_SLICER_CACHE}\" xmlns:x15=\"{NS_X15}\"><x15:tableSlicerCache tableId=\"{table_id}\" column=\"{column}\"/></x:ext></extLst></slicerCacheDefinition>",
        esc(&s.cache),
        esc(source_name)
    )
}

/// The `slicers` part of a sheet's new slicers.
fn slicers_xml(list: &[&Slicer]) -> String {
    let mut out = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<slicers xmlns=\"{NS_X14}\" xmlns:mc=\"{NS_MC}\" mc:Ignorable=\"x\" xmlns:x=\"{NS_MAIN}\">"
    );
    for s in list {
        out.push_str(&format!(
            "<slicer name=\"{}\" cache=\"{}\" caption=\"{}\"",
            esc(&s.name),
            esc(&s.cache),
            esc(&s.caption)
        ));
        if s.columns > 1 {
            out.push_str(&format!(" columnCount=\"{}\"", s.columns));
        }
        if let Some(st) = &s.style {
            out.push_str(&format!(" style=\"{}\"", esc(st)));
        }
        out.push_str(&format!(
            " rowHeight=\"{}\"/>",
            s.row_height.unwrap_or(241_300)
        ));
    }
    out.push_str("</slicers>");
    out
}

/// The drawing anchor of a slicer frame: `sle:slicer` for consumers that know slicers, a
/// plain rectangle naming the slicer for the others (`mc:AlternateContent`, [MS-ODRAWXML]).
fn anchor_xml(s: &Slicer, id: u32) -> String {
    let to =
        s.to.unwrap_or(CellRef::new(s.from.row + 10, s.from.col + 2));
    let cell = |tag: &str, c: CellRef| {
        format!(
            "<xdr:{tag}><xdr:col>{}</xdr:col><xdr:colOff>0</xdr:colOff><xdr:row>{}</xdr:row><xdr:rowOff>0</xdr:rowOff></xdr:{tag}>",
            c.col, c.row
        )
    };
    let name = esc(&s.name);
    format!(
        "<xdr:twoCellAnchor editAs=\"oneCell\">{}{}<mc:AlternateContent xmlns:mc=\"{NS_MC}\"><mc:Choice xmlns:a14=\"http://schemas.microsoft.com/office/drawing/2010/main\" Requires=\"a14\"><xdr:graphicFrame macro=\"\"><xdr:nvGraphicFramePr><xdr:cNvPr id=\"{id}\" name=\"{name}\"/><xdr:cNvGraphicFramePr/></xdr:nvGraphicFramePr><xdr:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"0\" cy=\"0\"/></xdr:xfrm><a:graphic><a:graphicData uri=\"http://schemas.microsoft.com/office/drawing/2010/slicer\"><sle:slicer xmlns:sle=\"http://schemas.microsoft.com/office/drawing/2010/slicer\" name=\"{name}\"/></a:graphicData></a:graphic></xdr:graphicFrame></mc:Choice><mc:Fallback><xdr:sp macro=\"\" textlink=\"\"><xdr:nvSpPr><xdr:cNvPr id=\"{}\" name=\"{name}\"/><xdr:cNvSpPr><a:spLocks noTextEdit=\"1\"/></xdr:cNvSpPr></xdr:nvSpPr><xdr:spPr><a:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"1828800\" cy=\"2524125\"/></a:xfrm><a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom><a:solidFill><a:srgbClr val=\"FFFFFF\"/></a:solidFill><a:ln w=\"1\"><a:solidFill><a:srgbClr val=\"808080\"/></a:solidFill></a:ln></xdr:spPr><xdr:txBody><a:bodyPr vertOverflow=\"clip\" horzOverflow=\"clip\"/><a:lstStyle/><a:p><a:r><a:rPr lang=\"vi-VN\" sz=\"1100\"/><a:t>Slicer: {name}</a:t></a:r></a:p></xdr:txBody></xdr:sp></mc:Fallback></mc:AlternateContent><xdr:clientData/></xdr:twoCellAnchor>",
        cell("from", s.from),
        cell("to", to),
        id + 1
    )
}

/// What a sheet's new slicers add to the package.
pub(crate) struct NewSlicers {
    /// Drawing anchors of the frames.
    pub anchors: String,
    /// The worksheet `extLst` extension listing the slicers part.
    pub sheet_ext: String,
    /// New slicer cache parts (for the workbook relationships and extension).
    pub caches: Vec<String>,
}

/// Parts of the new table slicers of `ws` (slicers without a part): one slicer cache part per
/// slicer, one slicers part for the sheet, the sheet relationship to it.
pub(crate) fn write_new(
    ws: &bo_model::sheet::Worksheet,
    all_tables: &[&Table],
    sheet_path: &str,
    fresh: &mut dyn FnMut(&str) -> String,
    alloc_rel: &mut dyn FnMut() -> String,
    rels: &mut Vec<Relationship>,
    out_parts: &mut Vec<(String, Option<String>, Vec<u8>)>,
) -> Option<NewSlicers> {
    let new: Vec<&Slicer> = ws
        .slicers
        .iter()
        .filter(|s| s.part.is_empty() && matches!(s.source, SlicerSource::Table { .. }))
        .collect();
    if new.is_empty() {
        return None;
    }
    let mut caches = Vec::new();
    for s in &new {
        let SlicerSource::Table {
            table_id, column, ..
        } = s.source
        else {
            continue;
        };
        let source_name = all_tables
            .iter()
            .find(|t| t.id == table_id)
            .and_then(|t| t.columns.iter().find(|c| c.id == column))
            .map_or_else(|| s.caption.clone(), |c| c.name.clone());
        let path = fresh("/xl/slicerCaches/slicerCache");
        out_parts.push((
            path.clone(),
            Some(CT_SLICER_CACHE.into()),
            table_cache_xml(s, table_id, column, &source_name).into_bytes(),
        ));
        caches.push(path);
    }
    let spath = fresh("/xl/slicers/slicer");
    out_parts.push((
        spath.clone(),
        Some(CT_SLICER.into()),
        slicers_xml(&new).into_bytes(),
    ));
    let rid = alloc_rel();
    rels.push(Relationship {
        id: rid.clone(),
        rel_type: REL_SLICER.into(),
        target: crate::write::relative_target(sheet_path, &spath),
        mode: TargetMode::Internal,
    });
    let anchors = new
        .iter()
        .enumerate()
        .map(|(i, s)| anchor_xml(s, 5000 + 2 * i as u32))
        .collect();
    let sheet_ext = format!(
        "<ext uri=\"{EXT_SHEET_TABLE_SLICERS}\" xmlns:x14=\"{NS_X14}\"><x14:slicerList><x14:slicer r:id=\"{rid}\"/></x14:slicerList></ext>"
    );
    Some(NewSlicers {
        anchors,
        sheet_ext,
        caches,
    })
}

/// The workbook `extLst` extension listing new table slicer caches by relationship id.
pub(crate) fn workbook_ext(rids: &[String]) -> String {
    let mut items = String::new();
    for r in rids {
        items.push_str("<x14:slicerCache r:id=\"");
        items.push_str(r);
        items.push_str("\"/>");
    }
    format!(
        "<ext uri=\"{EXT_WB_TABLE_SLICER_CACHES}\" xmlns:x15=\"{NS_X15}\"><x15:slicerCaches xmlns:x14=\"{NS_X14}\">{items}</x15:slicerCaches></ext>"
    )
}

/// `unknown` with `ext` added to its `extLst` (created when missing).
pub(crate) fn with_ext(
    unknown: &[bo_model::sheet::RawXml],
    ext: &str,
) -> Vec<bo_model::sheet::RawXml> {
    let mut out = unknown.to_vec();
    // The worksheet's own `extLst` (anchored at its schema slot), not one nested in a fragment.
    let own = out
        .iter()
        .position(|u| u.after == "extLst" && u.xml.trim_start().starts_with("<extLst"))
        .or_else(|| out.iter().position(|u| u.xml.contains("<extLst")));
    if let Some(x) = own.map(|i| &mut out[i]) {
        if let Some(k) = x.xml.rfind("</extLst>") {
            x.xml.insert_str(k, ext);
            return out;
        }
    }
    out.push(bo_model::sheet::RawXml {
        after: "extLst".into(),
        xml: format!("<extLst>{ext}</extLst>"),
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patches_selection_flags() {
        let xml = r#"<slicerCacheDefinition name="S"><data><tabular pivotCacheId="1"><items count="3"><i x="0" s="1"/><i x="1" s="1"/><i x="2" s="1" nd="1"/></items></tabular></data></slicerCacheDefinition>"#;
        let out = patch_cache_items(xml, &[(0, true), (1, false), (2, true)]).unwrap();
        assert_eq!(
            out,
            r#"<slicerCacheDefinition name="S"><data><tabular pivotCacheId="1"><items count="3"><i x="0" s="1"/><i x="1"/><i x="2" s="1" nd="1"/></items></tabular></data></slicerCacheDefinition>"#
        );
        let pre = r#"<x14:slicerCacheDefinition><x14:data><x14:tabular><x14:items count="1"><x14:i x="0"/></x14:items></x14:tabular></x14:data></x14:slicerCacheDefinition>"#;
        assert!(patch_cache_items(pre, &[(0, true)])
            .unwrap()
            .contains(r#"<x14:i x="0" s="1"/>"#));
    }
}
