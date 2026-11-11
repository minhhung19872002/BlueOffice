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
pub(crate) const NS_X14: &str = "http://schemas.microsoft.com/office/spreadsheetml/2009/9/main";
pub(crate) const NS_X15: &str = "http://schemas.microsoft.com/office/spreadsheetml/2010/11/main";
const NS_MAIN: &str = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";
pub(crate) const NS_MC: &str = "http://schemas.openxmlformats.org/markup-compatibility/2006";
/// Extension of a worksheet listing its table slicers ([MS-XLSX] `x14:slicerList`).
const EXT_SHEET_TABLE_SLICERS: &str = "{3A4CF648-6AED-40f4-86FF-DC5316D8AED3}";
/// Extension of a worksheet listing its pivot table slicers ([MS-XLSX] `x14:slicerList`).
const EXT_SHEET_SLICERS: &str = "{A8765BA9-456A-4dab-B4F3-ACF838C121DE}";
/// Extension of the workbook listing table slicer caches (`x15:slicerCaches`).
pub(crate) const EXT_WB_TABLE_SLICER_CACHES: &str = "{46F421CA-312F-682f-3DD2-61675219B42D}";
/// Extension of the workbook listing pivot table slicer caches (`x14:slicerCaches`).
const EXT_WB_SLICER_CACHES: &str = "{BBE1A952-AA13-448e-AADC-164F8A28A991}";
/// Extension of a slicer cache naming its table column (`x15:tableSlicerCache`).
const EXT_TABLE_SLICER_CACHE: &str = "{2F2917AC-EB37-4324-AD4E-5DD8C200BD13}";

pub(crate) fn esc(s: &str) -> String {
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

/// Where the pivot tables of the workbook live, for the `pivotTables` of slicer and timeline
/// caches: `pivotTable/@tabId` is the `sheetId` of the pivot table's sheet.
pub(crate) struct PivotRefs<'a> {
    /// The workbook.
    pub wb: &'a bo_model::sheet::Workbook,
    /// `sheetId` written for every sheet.
    pub sheet_ids: &'a [u32],
}

impl PivotRefs<'_> {
    /// `<pivotTables>` naming `pivots` (the ones that still exist).
    pub(crate) fn pivot_tables_xml(&self, prefix: &str, pivots: &[String]) -> String {
        let mut out = format!("<{prefix}pivotTables>");
        for name in pivots {
            let at = self
                .wb
                .sheets
                .iter()
                .position(|s| s.pivots.iter().any(|p| &p.name == name));
            if let Some(si) = at {
                let tab = self
                    .sheet_ids
                    .get(si)
                    .copied()
                    .unwrap_or(self.wb.sheets[si].sheet_id);
                out.push_str(&format!(
                    "<{prefix}pivotTable tabId=\"{tab}\" name=\"{}\"/>",
                    esc(name)
                ));
            }
        }
        out.push_str(&format!("</{prefix}pivotTables>"));
        out
    }

    /// The [MS-XLSX] `x14:pivotCacheDefinition/@pivotCacheId` of the cache of the first of
    /// `pivots` that exists.
    pub(crate) fn ext_cache_id(&self, pivots: &[String]) -> Option<u32> {
        let cache_id = self
            .wb
            .sheets
            .iter()
            .flat_map(|s| &s.pivots)
            .find(|p| pivots.iter().any(|n| n == &p.name))?
            .cache_id;
        self.wb
            .pivot_caches
            .iter()
            .find(|c| c.cache_id == cache_id)?
            .ext_id
    }
}

/// The `slicerCacheDefinition` of a new pivot table slicer ([MS-XLSX] `CT_SlicerCacheDefinition`:
/// the connected pivot tables, then the items of the cache field with their selection).
fn pivot_cache_xml(
    s: &Slicer,
    field: &str,
    pivots: &[String],
    items: &[(u32, bool)],
    refs: &PivotRefs<'_>,
) -> String {
    let mut out = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<slicerCacheDefinition xmlns=\"{NS_X14}\" xmlns:mc=\"{NS_MC}\" mc:Ignorable=\"x\" xmlns:x=\"{NS_MAIN}\" name=\"{}\" sourceName=\"{}\">",
        esc(&s.cache),
        esc(field)
    );
    out.push_str(&refs.pivot_tables_xml("", pivots));
    out.push_str(&format!(
        "<data><tabular pivotCacheId=\"{}\">",
        refs.ext_cache_id(pivots).unwrap_or(0)
    ));
    if !items.is_empty() {
        // Nothing chosen is stored as every item selected.
        let all = items.iter().all(|(_, sel)| !sel);
        out.push_str(&format!("<items count=\"{}\">", items.len()));
        for (x, sel) in items {
            out.push_str(&format!("<i x=\"{x}\""));
            if *sel || all {
                out.push_str(" s=\"1\"");
            }
            out.push_str("/>");
        }
        out.push_str("</items>");
    }
    out.push_str("</tabular></data></slicerCacheDefinition>");
    out
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

/// A drawing anchor holding a frame only newer consumers know (`choice`, with the namespace
/// declaration and prefix its `mc:Choice` requires) and a plain rectangle with `label` for the
/// others (`mc:AlternateContent`, ECMA-376 Part 3; §20.5.2.33 twoCellAnchor).
#[allow(clippy::too_many_arguments)]
pub(crate) fn frame_anchor_xml(
    from: CellRef,
    to: CellRef,
    id: u32,
    name: &str,
    requires: (&str, &str),
    uri: &str,
    choice: &str,
    label: &str,
) -> String {
    let cell = |tag: &str, c: CellRef| {
        format!(
            "<xdr:{tag}><xdr:col>{}</xdr:col><xdr:colOff>0</xdr:colOff><xdr:row>{}</xdr:row><xdr:rowOff>0</xdr:rowOff></xdr:{tag}>",
            c.col, c.row
        )
    };
    let name = esc(name);
    let (prefix, ns) = requires;
    format!(
        "<xdr:twoCellAnchor editAs=\"oneCell\">{}{}<mc:AlternateContent xmlns:mc=\"{NS_MC}\"><mc:Choice xmlns:{prefix}=\"{ns}\" Requires=\"{prefix}\"><xdr:graphicFrame macro=\"\"><xdr:nvGraphicFramePr><xdr:cNvPr id=\"{id}\" name=\"{name}\"/><xdr:cNvGraphicFramePr/></xdr:nvGraphicFramePr><xdr:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"0\" cy=\"0\"/></xdr:xfrm><a:graphic><a:graphicData uri=\"{uri}\">{choice}</a:graphicData></a:graphic></xdr:graphicFrame></mc:Choice><mc:Fallback><xdr:sp macro=\"\" textlink=\"\"><xdr:nvSpPr><xdr:cNvPr id=\"{}\" name=\"{name}\"/><xdr:cNvSpPr><a:spLocks noTextEdit=\"1\"/></xdr:cNvSpPr></xdr:nvSpPr><xdr:spPr><a:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"1828800\" cy=\"2524125\"/></a:xfrm><a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom><a:solidFill><a:srgbClr val=\"FFFFFF\"/></a:solidFill><a:ln w=\"1\"><a:solidFill><a:srgbClr val=\"808080\"/></a:solidFill></a:ln></xdr:spPr><xdr:txBody><a:bodyPr vertOverflow=\"clip\" horzOverflow=\"clip\"/><a:lstStyle/><a:p><a:r><a:rPr lang=\"vi-VN\" sz=\"1100\"/><a:t>{label}: {name}</a:t></a:r></a:p></xdr:txBody></xdr:sp></mc:Fallback></mc:AlternateContent><xdr:clientData/></xdr:twoCellAnchor>",
        cell("from", from),
        cell("to", to),
        id + 1
    )
}

/// The drawing anchor of a slicer frame: `sle:slicer` for consumers that know slicers, a
/// plain rectangle naming the slicer for the others ([MS-ODRAWXML]).
fn anchor_xml(s: &Slicer, id: u32) -> String {
    let to =
        s.to.unwrap_or(CellRef::new(s.from.row + 10, s.from.col + 2));
    frame_anchor_xml(
        s.from,
        to,
        id,
        &s.name,
        ("a14", "http://schemas.microsoft.com/office/drawing/2010/main"),
        "http://schemas.microsoft.com/office/drawing/2010/slicer",
        &format!(
            "<sle:slicer xmlns:sle=\"http://schemas.microsoft.com/office/drawing/2010/slicer\" name=\"{}\"/>",
            esc(&s.name)
        ),
        "Slicer",
    )
}

/// An entry to add to a list inside an `extLst` extension: the extension's `uri`, the end tag
/// of its list element, the entry, and the whole extension to add when there is none yet.
pub(crate) struct ExtItem {
    /// `ext/@uri`.
    pub uri: &'static str,
    /// End tag of the list element (`</x14:slicerList>`).
    pub list_close: &'static str,
    /// The entry (`<x14:slicer r:id="…"/>`).
    pub item: String,
    /// The extension holding only this entry.
    pub ext: String,
}

/// What a sheet's new slicers add to the package.
pub(crate) struct NewSlicers {
    /// Drawing anchors of the frames.
    pub anchors: String,
    /// The worksheet `extLst` entries listing the slicers parts.
    pub sheet_exts: Vec<ExtItem>,
    /// New slicer cache parts (for the workbook relationships and extension) and whether
    /// each one is a table slicer cache.
    pub caches: Vec<(String, bool)>,
}

/// Parts of the new slicers of `ws` (slicers without a part): one slicer cache part per
/// slicer, one slicers part for the sheet's table slicers and one for its pivot table
/// slicers, the sheet relationships to them.
#[allow(clippy::too_many_arguments)]
pub(crate) fn write_new(
    ws: &bo_model::sheet::Worksheet,
    all_tables: &[&Table],
    refs: &PivotRefs<'_>,
    sheet_path: &str,
    fresh: &mut dyn FnMut(&str) -> String,
    alloc_rel: &mut dyn FnMut() -> String,
    rels: &mut Vec<Relationship>,
    out_parts: &mut Vec<(String, Option<String>, Vec<u8>)>,
) -> Option<NewSlicers> {
    let new: Vec<&Slicer> = ws.slicers.iter().filter(|s| s.part.is_empty()).collect();
    if new.is_empty() {
        return None;
    }
    let mut caches = Vec::new();
    for s in &new {
        let (xml, table) = match &s.source {
            SlicerSource::Table {
                table_id, column, ..
            } => {
                let source_name = all_tables
                    .iter()
                    .find(|t| t.id == *table_id)
                    .and_then(|t| t.columns.iter().find(|c| c.id == *column))
                    .map_or_else(|| s.caption.clone(), |c| c.name.clone());
                (table_cache_xml(s, *table_id, *column, &source_name), true)
            }
            SlicerSource::Pivot {
                field,
                pivots,
                items,
                ..
            } => (pivot_cache_xml(s, field, pivots, items, refs), false),
        };
        let path = fresh("/xl/slicerCaches/slicerCache");
        out_parts.push((path.clone(), Some(CT_SLICER_CACHE.into()), xml.into_bytes()));
        caches.push((path, table));
    }
    let mut sheet_exts = Vec::new();
    for table in [true, false] {
        let group: Vec<&Slicer> = new
            .iter()
            .copied()
            .filter(|s| matches!(s.source, SlicerSource::Table { .. }) == table)
            .collect();
        if group.is_empty() {
            continue;
        }
        let spath = fresh("/xl/slicers/slicer");
        out_parts.push((
            spath.clone(),
            Some(CT_SLICER.into()),
            slicers_xml(&group).into_bytes(),
        ));
        let rid = alloc_rel();
        rels.push(Relationship {
            id: rid.clone(),
            rel_type: REL_SLICER.into(),
            target: crate::write::relative_target(sheet_path, &spath),
            mode: TargetMode::Internal,
        });
        let uri = if table {
            EXT_SHEET_TABLE_SLICERS
        } else {
            EXT_SHEET_SLICERS
        };
        let item = format!("<x14:slicer r:id=\"{rid}\"/>");
        sheet_exts.push(ExtItem {
            uri,
            list_close: "</x14:slicerList>",
            ext: format!(
                "<ext uri=\"{uri}\" xmlns:x14=\"{NS_X14}\"><x14:slicerList>{item}</x14:slicerList></ext>"
            ),
            item,
        });
    }
    let anchors = new
        .iter()
        .enumerate()
        .map(|(i, s)| anchor_xml(s, 5000 + 2 * i as u32))
        .collect();
    Some(NewSlicers {
        anchors,
        sheet_exts,
        caches,
    })
}

/// The workbook `extLst` entry listing a new slicer cache by relationship id (`table`: a table
/// slicer cache, listed in `x15:slicerCaches`; otherwise `x14:slicerCaches`).
pub(crate) fn workbook_ext(rid: &str, table: bool) -> ExtItem {
    let item = format!("<x14:slicerCache r:id=\"{rid}\"/>");
    if table {
        ExtItem {
            uri: EXT_WB_TABLE_SLICER_CACHES,
            list_close: "</x15:slicerCaches>",
            ext: format!(
                "<ext uri=\"{EXT_WB_TABLE_SLICER_CACHES}\" xmlns:x15=\"{NS_X15}\"><x15:slicerCaches xmlns:x14=\"{NS_X14}\">{item}</x15:slicerCaches></ext>"
            ),
            item,
        }
    } else {
        ExtItem {
            uri: EXT_WB_SLICER_CACHES,
            list_close: "</x14:slicerCaches>",
            ext: format!(
                "<ext uri=\"{EXT_WB_SLICER_CACHES}\" xmlns:x14=\"{NS_X14}\"><x14:slicerCaches>{item}</x14:slicerCaches></ext>"
            ),
            item,
        }
    }
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

/// `unknown` with `item` listed in the `extLst` extension `item.uri`: added to the extension's
/// list when the element has one already, as a new extension otherwise.
pub(crate) fn with_ext_item(
    unknown: &[bo_model::sheet::RawXml],
    item: &ExtItem,
) -> Vec<bo_model::sheet::RawXml> {
    let mut out = unknown.to_vec();
    for u in &mut out {
        let Some(at) = u.xml.find(item.uri) else {
            continue;
        };
        // The list must end inside this extension.
        let ext_end = u.xml[at..].find("ext>").map_or(u.xml.len(), |e| at + e);
        if let Some(k) = u.xml[at..ext_end].find(item.list_close) {
            u.xml.insert_str(at + k, &item.item);
            return out;
        }
    }
    with_ext(unknown, &item.ext)
}

// ------------------------------------------------------------------ editing file parts

/// Start tags of `text`: `(position of '<', qualified name, position after the tag's '>')`.
fn start_tags(text: &str) -> impl Iterator<Item = (usize, &str, usize)> {
    let mut from = 0;
    std::iter::from_fn(move || loop {
        let at = from + text[from..].find('<')?;
        let end = text[at..].find('>').map_or(text.len(), |e| at + e + 1);
        from = at + 1;
        let tag = text[at + 1..end].trim_end_matches('>');
        if tag.starts_with(['/', '?', '!']) {
            continue;
        }
        let qname = tag
            .split(|c: char| c.is_whitespace() || c == '/')
            .next()
            .unwrap_or_default();
        return Some((at, qname, end));
    })
}

fn local(qname: &str) -> &str {
    qname.rsplit_once(':').map_or(qname, |(_, l)| l)
}

/// Byte range of the element `<local name="name" …>` (any prefix) of `text`, end tag included.
pub(crate) fn named_span(text: &str, elem: &str, name: &str) -> Option<(usize, usize)> {
    let attr = format!(" name=\"{}\"", esc(name));
    let (at, qname, end) = start_tags(text)
        .find(|(at, q, end)| local(q) == elem && text[*at..*end].contains(&attr))?;
    if text[..end].ends_with("/>") {
        return Some((at, end));
    }
    let close = format!("</{qname}>");
    let k = text[end..].find(&close)?;
    Some((at, end + k + close.len()))
}

/// Number of elements `<local …>` (any prefix) in `text`.
pub(crate) fn count_elements(text: &str, elem: &str) -> usize {
    start_tags(text)
        .filter(|(_, q, _)| local(q) == elem)
        .count()
}

/// The start tag `tag` (`<x a="1"/>` or `<x a="1">`) with attribute `attr` set to `value`
/// (added before the end of the tag when missing) or removed (`None`).
pub(crate) fn set_tag_attr(tag: &str, attr: &str, value: Option<&str>) -> String {
    let key = format!(" {attr}=\"");
    if let Some(a) = tag.find(&key) {
        let v0 = a + key.len();
        let v1 = tag[v0..].find('"').map_or(tag.len(), |q| v0 + q);
        return match value {
            Some(v) => format!("{}{}{}", &tag[..v0], esc(v), &tag[v1..]),
            None => format!("{}{}", &tag[..a], &tag[(v1 + 1).min(tag.len())..]),
        };
    }
    let Some(v) = value else {
        return tag.to_owned();
    };
    let cut = if tag.ends_with("/>") {
        tag.len() - 2
    } else {
        tag.len().saturating_sub(1)
    };
    format!("{}{key}{}\"{}", &tag[..cut], esc(v), &tag[cut..])
}

/// Part `part` of the package kept as read, as text.
pub(crate) fn opaque_text(wb: &bo_model::sheet::Workbook, part: &str) -> Option<String> {
    wb.opaque
        .parts
        .iter()
        .find(|p| p.name == part)
        .and_then(|p| String::from_utf8(p.data.clone()).ok())
}

pub(crate) fn set_opaque_text(wb: &mut bo_model::sheet::Workbook, part: &str, text: String) {
    if let Some(p) = wb.opaque.parts.iter_mut().find(|p| p.name == part) {
        p.data = text.into_bytes();
    }
}

/// Set attributes of the element `<elem name="name">` of part `part` (`None` removes one).
pub(crate) fn patch_named_attrs(
    wb: &mut bo_model::sheet::Workbook,
    part: &str,
    elem: &str,
    name: &str,
    attrs: &[(&str, Option<String>)],
) {
    let Some(mut text) = opaque_text(wb, part) else {
        return;
    };
    let Some((at, _)) = named_span(&text, elem, name) else {
        return;
    };
    let Some(end) = text[at..].find('>').map(|e| at + e + 1) else {
        return;
    };
    let mut tag = text[at..end].to_owned();
    for (a, v) in attrs {
        tag = set_tag_attr(&tag, a, v.as_deref());
    }
    text.replace_range(at..end, &tag);
    set_opaque_text(wb, part, text);
}

/// Replace the `pivotTables` element of cache part `part` with the list naming `pivots`.
pub(crate) fn patch_pivot_tables(
    wb: &mut bo_model::sheet::Workbook,
    part: &str,
    pivots: &[String],
) {
    let Some(mut text) = opaque_text(wb, part) else {
        return;
    };
    let Some((at, qname, _)) = start_tags(&text).find(|(_, q, _)| local(q) == "pivotTables") else {
        return;
    };
    let prefix = qname
        .rsplit_once(':')
        .map_or(String::new(), |(p, _)| format!("{p}:"));
    let close = format!("</{qname}>");
    let Some(end) = text[at..].find(&close).map(|e| at + e + close.len()) else {
        return;
    };
    let ids: Vec<u32> = wb.sheets.iter().map(|s| s.sheet_id).collect();
    let xml = PivotRefs {
        wb,
        sheet_ids: &ids,
    }
    .pivot_tables_xml(&prefix, pivots);
    text.replace_range(at..end, &xml);
    set_opaque_text(wb, part, text);
}

/// Remove the drawing anchor of the frame `<elem name="name">` (`slicer`, `timeslicer`) of
/// sheet `sheet` (§20.5.2.33).
pub(crate) fn remove_frame(
    wb: &mut bo_model::sheet::Workbook,
    sheet: usize,
    elem: &str,
    name: &str,
) {
    let drawings: Vec<String> = wb.sheets[sheet]
        .rels
        .iter()
        .filter(|r| !r.external && r.rel_type.ends_with("/drawing"))
        .map(|r| r.target.clone())
        .collect();
    for d in drawings {
        let Some(mut text) = opaque_text(wb, &d) else {
            continue;
        };
        let Some((at, _)) = named_span(&text, elem, name) else {
            continue;
        };
        if let Some((s, e, _)) = crate::chart_write::anchor_span_at(&text, at) {
            text.replace_range(s..e, "");
            set_opaque_text(wb, &d, text);
            return;
        }
    }
}

/// Remove the `extLst` entry `<… r:id="rid"/>` from `unknown`; an extension left with an empty
/// list goes too, and so does an `extLst` left without extensions.
pub(crate) fn drop_ext_ref(unknown: &mut Vec<bo_model::sheet::RawXml>, rid: &str) {
    let needle = format!(":id=\"{rid}\"");
    for u in unknown.iter_mut() {
        if !u.xml.contains("extLst") {
            continue;
        }
        let Some(k) = u.xml.find(&needle) else {
            continue;
        };
        let (Some(s), Some(e)) = (u.xml[..k].rfind('<'), u.xml[k..].find("/>")) else {
            continue;
        };
        u.xml.replace_range(s..k + e + 2, "");
        // The extension the entry was in: gone when it lists nothing any more.
        let ext_at = start_tags(&u.xml[..s])
            .filter(|(_, q, _)| local(q) == "ext")
            .last()
            .map(|(at, q, _)| (at, q.to_owned()));
        if let Some((at, q)) = ext_at {
            let close = format!("</{q}>");
            if let Some(end) = u.xml[at..].find(&close).map(|x| at + x + close.len()) {
                if !u.xml[at..end].contains(":id=\"") {
                    u.xml.replace_range(at..end, "");
                }
            }
        }
        break;
    }
    unknown.retain(|u| {
        let t: String = u.xml.split_whitespace().collect();
        t != "<extLst></extLst>" && t != "<extLst/>"
    });
}

/// Remove part `part` from the package with its relationship from sheet `sheet` and the
/// sheet's `extLst` entry naming that relationship.
pub(crate) fn drop_sheet_part(wb: &mut bo_model::sheet::Workbook, sheet: usize, part: &str) {
    let ws = &mut wb.sheets[sheet];
    if let Some(i) = ws.rels.iter().position(|r| !r.external && r.target == part) {
        let id = ws.rels.remove(i).id;
        drop_ext_ref(&mut ws.unknown, &id);
    }
    drop_part(wb, part);
}

/// Remove part `part` from the package with its relationship from the workbook and the
/// workbook's `extLst` entry naming that relationship.
pub(crate) fn drop_workbook_part(wb: &mut bo_model::sheet::Workbook, part: &str) {
    let rels = &mut wb.opaque.workbook_rels;
    if let Some(i) = rels.iter().position(|r| !r.external && r.target == part) {
        let id = rels.remove(i).id;
        drop_ext_ref(&mut wb.unknown, &id);
    }
    drop_part(wb, part);
}

fn drop_part(wb: &mut bo_model::sheet::Workbook, part: &str) {
    let rels = bo_opc::rels_part_name(part);
    wb.opaque.parts.retain(|p| p.name != part && p.name != rels);
}

/// Remove slicer `index` of sheet `sheet`. A slicer read from the file loses its drawing frame
/// and its entry in the slicers part; the slicers part and the slicer cache go with their
/// relationships and `extLst` entries when nothing uses them any more. The defined name of
/// the cache goes with the cache. `false` when there is no such slicer.
pub fn remove_slicer(wb: &mut bo_model::sheet::Workbook, sheet: usize, index: usize) -> bool {
    let Some(ws) = wb.sheets.get_mut(sheet) else {
        return false;
    };
    if index >= ws.slicers.len() {
        return false;
    }
    let s = ws.slicers.remove(index);
    let cache_used = wb
        .sheets
        .iter()
        .flat_map(|x| &x.slicers)
        .any(|o| o.cache == s.cache);
    if !cache_used {
        wb.defined_names.retain(|d| d.name != s.cache);
    }
    if s.part.is_empty() {
        return true;
    }
    remove_frame(wb, sheet, "slicer", &s.name);
    if let Some(mut text) = opaque_text(wb, &s.part) {
        if let Some((a, b)) = named_span(&text, "slicer", &s.name) {
            text.replace_range(a..b, "");
        }
        if count_elements(&text, "slicer") == 0 {
            drop_sheet_part(wb, sheet, &s.part);
        } else {
            set_opaque_text(wb, &s.part, text);
        }
    }
    if !cache_used && !s.cache_part.is_empty() {
        drop_workbook_part(wb, &s.cache_part);
    }
    true
}

/// Write the caption, column count, style and connected pivot tables of slicer `index` of
/// sheet `sheet` into the parts it was read from (a new slicer is written whole on save).
pub fn patch_slicer(wb: &mut bo_model::sheet::Workbook, sheet: usize, index: usize) {
    let Some(s) = wb
        .sheets
        .get(sheet)
        .and_then(|ws| ws.slicers.get(index))
        .cloned()
    else {
        return;
    };
    if s.part.is_empty() {
        return;
    }
    patch_named_attrs(
        wb,
        &s.part,
        "slicer",
        &s.name,
        &[
            ("caption", Some(s.caption.clone())),
            (
                "columnCount",
                (s.columns > 1).then(|| s.columns.to_string()),
            ),
            ("style", s.style.clone()),
        ],
    );
    if let SlicerSource::Pivot { pivots, .. } = &s.source {
        patch_pivot_tables(wb, &s.cache_part, pivots);
    }
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
