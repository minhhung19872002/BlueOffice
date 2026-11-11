//! Timelines ([MS-XLSX] timeline parts): `x15:timelines` (timelines part of a worksheet),
//! `x15:timelineCacheDefinition` (timeline cache part of the workbook: the pivot date field,
//! the connected pivot tables and `state/selection` dates) and the drawing frame whose
//! `tsle:timeslicer` places a timeline on the sheet.
//!
//! The parts stay verbatim in the package; a changed selection rewrites the cache's
//! `selection` element.

use std::collections::HashMap;

use bo_model::sheet::{CellRef, Timeline};
use bo_opc::{resolve_target, Package, Relationship, TargetMode};

use crate::chart::{anchor_cell, parse_dom, Node};

/// Worksheet → timelines part.
pub const REL_TIMELINE: &str = "http://schemas.microsoft.com/office/2011/relationships/timeline";
/// Workbook → timeline cache part.
pub const REL_TIMELINE_CACHE: &str =
    "http://schemas.microsoft.com/office/2011/relationships/timelineCache";

fn descendants<'a>(n: &'a Node, name: &str, out: &mut Vec<&'a Node>) {
    for c in &n.children {
        if c.name == name {
            out.push(c);
        }
        descendants(c, name, out);
    }
}

struct Cache {
    part: String,
    field: String,
    pivots: Vec<String>,
    start: Option<String>,
    end: Option<String>,
}

fn caches(pkg: &Package, wb_part: &str) -> HashMap<String, Cache> {
    let mut out = HashMap::new();
    for rel in pkg.relationships(wb_part).unwrap_or_default() {
        if rel.mode != TargetMode::Internal || rel.rel_type != REL_TIMELINE_CACHE {
            continue;
        }
        let part = resolve_target(wb_part, &rel.target);
        let Some(dom) = pkg.part(&part).and_then(|p| parse_dom(&p.data, &part)) else {
            continue;
        };
        if dom.name != "timelineCacheDefinition" {
            continue;
        }
        let pivots = dom
            .child("pivotTables")
            .map(|p| {
                p.children
                    .iter()
                    .filter_map(|t| t.attr("name").map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default();
        let sel = dom.child("state").and_then(|s| s.child("selection"));
        out.insert(
            dom.attr("name").unwrap_or_default().to_owned(),
            Cache {
                part,
                field: dom.attr("sourceName").unwrap_or_default().to_owned(),
                pivots,
                start: sel.and_then(|s| s.attr("startDate")).map(str::to_owned),
                end: sel.and_then(|s| s.attr("endDate")).map(str::to_owned),
            },
        );
    }
    out
}

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
            descendants(anchor, "timeslicer", &mut found);
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

/// The timelines of a worksheet.
pub(crate) fn read_timelines(
    pkg: &Package,
    wb_part: &str,
    sheet_part: &str,
    rels: &[Relationship],
) -> Vec<Timeline> {
    let parts: Vec<String> = rels
        .iter()
        .filter(|r| r.mode == TargetMode::Internal && r.rel_type == REL_TIMELINE)
        .map(|r| resolve_target(sheet_part, &r.target))
        .collect();
    if parts.is_empty() {
        return Vec::new();
    }
    let caches = caches(pkg, wb_part);
    let places = anchors(pkg, sheet_part, rels);
    let mut out = Vec::new();
    for part in parts {
        let Some(dom) = pkg.part(&part).and_then(|p| parse_dom(&p.data, &part)) else {
            continue;
        };
        for t in dom.children.iter().filter(|c| c.name == "timeline") {
            let name = t.attr("name").unwrap_or_default().to_owned();
            let cache_name = t.attr("cache").unwrap_or_default().to_owned();
            let Some(cache) = caches.get(&cache_name) else {
                continue;
            };
            let (from, to) = places.get(&name).copied().unwrap_or_default();
            out.push(Timeline {
                caption: t.attr("caption").unwrap_or(&name).to_owned(),
                name,
                cache: cache_name,
                field: cache.field.clone(),
                pivots: cache.pivots.clone(),
                level: t.attr("level").and_then(|v| v.parse().ok()).unwrap_or(2),
                start: cache.start.clone(),
                end: cache.end.clone(),
                from,
                to,
                cache_part: cache.part.clone(),
                part: part.clone(),
                dirty: false,
                style: t.attr("style").map(str::to_owned),
            });
        }
    }
    out
}

/// The cache part with its `state/selection` dates set (or the selection removed) and the
/// state's `filterType` following it (`dateBetween` with a selection, `unknown` without).
#[must_use]
pub fn patch_selection(xml: &str, start: Option<&str>, end: Option<&str>) -> Option<String> {
    let out = patch_selection_element(xml, start, end)?;
    let state = out.find("state")?;
    let tag_start = out[..state].rfind('<')?;
    let tag_end = tag_start + out[tag_start..].find('>')? + 1;
    let kind = if start.is_some() && end.is_some() {
        "dateBetween"
    } else {
        "unknown"
    };
    let tag = crate::slicer::set_tag_attr(&out[tag_start..tag_end], "filterType", Some(kind));
    Some(format!("{}{tag}{}", &out[..tag_start], &out[tag_end..]))
}

fn patch_selection_element(xml: &str, start: Option<&str>, end: Option<&str>) -> Option<String> {
    let state = xml.find("state")?;
    let state_tag = xml[..state].rfind('<')?;
    let prefix = &xml[state_tag + 1..state];
    let sel_open = format!("<{prefix}selection");
    let new_sel = match (start, end) {
        (Some(a), Some(b)) => format!("<{prefix}selection startDate=\"{a}\" endDate=\"{b}\"/>"),
        _ => String::new(),
    };
    if let Some(k) = xml[state..].find(&sel_open).map(|k| state + k) {
        // Replace the existing selection element (self-closing).
        let end_k = k + xml[k..].find("/>")? + 2;
        return Some(format!("{}{new_sel}{}", &xml[..k], &xml[end_k..]));
    }
    // No selection yet: insert it as the first child of `state`.
    let body = state + xml[state..].find('>')?;
    if xml.as_bytes().get(body - 1) == Some(&b'/') {
        // `<state …/>`: expand it.
        let close = format!("</{prefix}state>");
        return Some(format!(
            "{}>{new_sel}{close}{}",
            &xml[..body - 1],
            &xml[body + 1..]
        ));
    }
    Some(format!("{}{new_sel}{}", &xml[..=body], &xml[body + 1..]))
}

// ------------------------------------------------------------------ writing new timelines

/// Content type of a timelines part.
pub const CT_TIMELINE: &str = "application/vnd.ms-excel.timeline+xml";
/// Content type of a timeline cache part.
pub const CT_TIMELINE_CACHE: &str = "application/vnd.ms-excel.timelineCache+xml";
/// Extension of a worksheet listing its timelines parts ([MS-XLSX] `x15:timelineRefs`).
const EXT_SHEET_TIMELINES: &str = "{7E03D99C-DC04-49d9-9315-930204A7B6E9}";
/// Extension of the workbook listing timeline caches ([MS-XLSX] `x15:timelineCacheRefs`).
const EXT_WB_TIMELINE_CACHES: &str = "{D0CA8CA8-9F24-4464-BF8E-62219DCF47F9}";
/// Namespace of the drawing frame of a timeline ([MS-ODRAWXML] `tsle:timeslicer`).
const NS_TSLE: &str = "http://schemas.microsoft.com/office/drawing/2012/timeslicer";

use crate::slicer::{esc, ExtItem, PivotRefs, NS_X15};

/// First and last date (ISO date-times) of the timeline's field in its pivot cache: the
/// `bounds` of the timeline cache's state.
fn bounds(refs: &PivotRefs<'_>, t: &Timeline) -> Option<(String, String)> {
    let wb = refs.wb;
    let cache_id = wb
        .sheets
        .iter()
        .flat_map(|s| &s.pivots)
        .find(|p| t.pivots.iter().any(|n| n == &p.name))?
        .cache_id;
    let cache = wb.pivot_caches.iter().find(|c| c.cache_id == cache_id)?;
    let fi = cache
        .fields
        .iter()
        .position(|f| f.name.eq_ignore_ascii_case(&t.field))?;
    let serial = |v: &bo_model::sheet::CellValue| match v {
        bo_model::sheet::CellValue::Number(n) => Some(n.floor()),
        bo_model::sheet::CellValue::Date(d) => crate::pivot::iso_to_serial_in(d, wb.date1904),
        _ => None,
    };
    let mut days: Vec<f64> = cache.fields[fi].items.iter().filter_map(serial).collect();
    if days.is_empty() {
        days = cache
            .records
            .iter()
            .filter_map(|r| r.get(fi).and_then(serial))
            .collect();
    }
    let lo = days.iter().copied().fold(f64::INFINITY, f64::min);
    let hi = days.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    if !lo.is_finite() || !hi.is_finite() {
        return None;
    }
    let iso = |n: f64| {
        let s = crate::pivot::serial_to_iso_in(n, wb.date1904);
        // Date-times of the timeline parts carry seconds and no fraction.
        format!("{}T00:00:00", s.get(..10).unwrap_or(&s))
    };
    Some((iso(lo), iso(hi + 1.0)))
}

/// The `timelineCacheDefinition` of a new timeline ([MS-XLSX] `CT_TimelineCacheDefinition`:
/// the connected pivot tables, then the state with the selection and the bounds).
fn cache_xml(t: &Timeline, refs: &PivotRefs<'_>) -> String {
    let mut out = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<timelineCacheDefinition xmlns=\"{NS_X15}\" name=\"{}\" sourceName=\"{}\">",
        esc(&t.cache),
        esc(&t.field)
    );
    out.push_str(&refs.pivot_tables_xml("", &t.pivots));
    let selection = t.start.as_ref().zip(t.end.as_ref());
    out.push_str(&format!(
        "<state minimalRefreshVersion=\"6\" lastRefreshVersion=\"6\" pivotCacheId=\"{}\" filterType=\"{}\">",
        refs.ext_cache_id(&t.pivots).unwrap_or(0),
        if selection.is_some() {
            "dateBetween"
        } else {
            "unknown"
        }
    ));
    if let Some((a, b)) = selection {
        out.push_str(&format!(
            "<selection startDate=\"{}\" endDate=\"{}\"/>",
            esc(a),
            esc(b)
        ));
    }
    if let Some((a, b)) = bounds(refs, t) {
        out.push_str(&format!("<bounds startDate=\"{a}\" endDate=\"{b}\"/>"));
    }
    out.push_str("</state></timelineCacheDefinition>");
    out
}

/// The `timelines` part of a sheet's new timelines ([MS-XLSX] `CT_Timeline`).
fn timelines_xml(list: &[&Timeline]) -> String {
    let mut out = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<timelines xmlns=\"{NS_X15}\">"
    );
    for t in list {
        out.push_str(&format!(
            "<timeline name=\"{}\" cache=\"{}\" caption=\"{}\" level=\"{}\" selectionLevel=\"{}\"",
            esc(&t.name),
            esc(&t.cache),
            esc(&t.caption),
            t.level.min(3),
            t.level.min(3)
        ));
        if let Some(st) = &t.style {
            out.push_str(&format!(" style=\"{}\"", esc(st)));
        }
        out.push_str("/>");
    }
    out.push_str("</timelines>");
    out
}

/// What a sheet's new timelines add to the package.
pub(crate) struct NewTimelines {
    /// Drawing anchors of the frames.
    pub anchors: String,
    /// The worksheet `extLst` entry listing the timelines part.
    pub sheet_ext: ExtItem,
    /// New timeline cache parts (for the workbook relationships and extension).
    pub caches: Vec<String>,
}

/// Parts of the new timelines of `ws` (timelines without a part): one timeline cache part per
/// timeline, one timelines part for the sheet and the sheet relationship to it.
pub(crate) fn write_new(
    ws: &bo_model::sheet::Worksheet,
    refs: &PivotRefs<'_>,
    sheet_path: &str,
    fresh: &mut dyn FnMut(&str) -> String,
    alloc_rel: &mut dyn FnMut() -> String,
    rels: &mut Vec<Relationship>,
    out_parts: &mut Vec<(String, Option<String>, Vec<u8>)>,
) -> Option<NewTimelines> {
    let new: Vec<&Timeline> = ws.timelines.iter().filter(|t| t.part.is_empty()).collect();
    if new.is_empty() {
        return None;
    }
    let mut caches = Vec::new();
    for t in &new {
        let path = fresh("/xl/timelineCaches/timelineCache");
        out_parts.push((
            path.clone(),
            Some(CT_TIMELINE_CACHE.into()),
            cache_xml(t, refs).into_bytes(),
        ));
        caches.push(path);
    }
    let tpath = fresh("/xl/timelines/timeline");
    out_parts.push((
        tpath.clone(),
        Some(CT_TIMELINE.into()),
        timelines_xml(&new).into_bytes(),
    ));
    let rid = alloc_rel();
    rels.push(Relationship {
        id: rid.clone(),
        rel_type: REL_TIMELINE.into(),
        target: crate::write::relative_target(sheet_path, &tpath),
        mode: TargetMode::Internal,
    });
    let anchors = new
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let to = t.to.unwrap_or(CellRef::new(t.from.row + 7, t.from.col + 6));
            crate::slicer::frame_anchor_xml(
                t.from,
                to,
                6000 + 2 * i as u32,
                &t.name,
                ("tsle", NS_TSLE),
                NS_TSLE,
                &format!("<tsle:timeslicer name=\"{}\"/>", esc(&t.name)),
                "Timeline",
            )
        })
        .collect();
    let item = format!("<x15:timelineRef r:id=\"{rid}\"/>");
    Some(NewTimelines {
        anchors,
        sheet_ext: ExtItem {
            uri: EXT_SHEET_TIMELINES,
            list_close: "</x15:timelineRefs>",
            ext: format!(
                "<ext uri=\"{EXT_SHEET_TIMELINES}\" xmlns:x15=\"{NS_X15}\"><x15:timelineRefs>{item}</x15:timelineRefs></ext>"
            ),
            item,
        },
        caches,
    })
}

/// The workbook `extLst` entry listing a new timeline cache by relationship id.
pub(crate) fn workbook_ext(rid: &str) -> ExtItem {
    let item = format!("<x15:timelineCacheRef r:id=\"{rid}\"/>");
    ExtItem {
        uri: EXT_WB_TIMELINE_CACHES,
        list_close: "</x15:timelineCacheRefs>",
        ext: format!(
            "<ext uri=\"{EXT_WB_TIMELINE_CACHES}\" xmlns:x15=\"{NS_X15}\"><x15:timelineCacheRefs>{item}</x15:timelineCacheRefs></ext>"
        ),
        item,
    }
}

// ------------------------------------------------------------------ editing file parts

/// Remove timeline `index` of sheet `sheet`. A timeline read from the file loses its drawing
/// frame and its entry in the timelines part; the timelines part and the timeline cache go
/// with their relationships and `extLst` entries when nothing uses them any more. `false`
/// when there is no such timeline.
pub fn remove_timeline(wb: &mut bo_model::sheet::Workbook, sheet: usize, index: usize) -> bool {
    use crate::slicer::{
        count_elements, drop_sheet_part, drop_workbook_part, named_span, opaque_text, remove_frame,
        set_opaque_text,
    };
    let Some(ws) = wb.sheets.get_mut(sheet) else {
        return false;
    };
    if index >= ws.timelines.len() {
        return false;
    }
    let t = ws.timelines.remove(index);
    let cache_used = wb
        .sheets
        .iter()
        .flat_map(|x| &x.timelines)
        .any(|o| o.cache == t.cache);
    if !cache_used {
        wb.defined_names.retain(|d| d.name != t.cache);
    }
    if t.part.is_empty() {
        return true;
    }
    remove_frame(wb, sheet, "timeslicer", &t.name);
    if let Some(mut text) = opaque_text(wb, &t.part) {
        if let Some((a, b)) = named_span(&text, "timeline", &t.name) {
            text.replace_range(a..b, "");
        }
        if count_elements(&text, "timeline") == 0 {
            drop_sheet_part(wb, sheet, &t.part);
        } else {
            set_opaque_text(wb, &t.part, text);
        }
    }
    if !cache_used && !t.cache_part.is_empty() {
        drop_workbook_part(wb, &t.cache_part);
    }
    true
}

/// Write the caption, level, style and connected pivot tables of timeline `index` of sheet
/// `sheet` into the parts it was read from (a new timeline is written whole on save).
pub fn patch_timeline(wb: &mut bo_model::sheet::Workbook, sheet: usize, index: usize) {
    let Some(t) = wb
        .sheets
        .get(sheet)
        .and_then(|ws| ws.timelines.get(index))
        .cloned()
    else {
        return;
    };
    if t.part.is_empty() {
        return;
    }
    let level = t.level.min(3).to_string();
    crate::slicer::patch_named_attrs(
        wb,
        &t.part,
        "timeline",
        &t.name,
        &[
            ("caption", Some(t.caption.clone())),
            ("level", Some(level.clone())),
            ("selectionLevel", Some(level)),
            ("style", t.style.clone()),
        ],
    );
    crate::slicer::patch_pivot_tables(wb, &t.cache_part, &t.pivots);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selection_patch() {
        let xml = r#"<timelineCacheDefinition><state filterType="dateBetween"><selection startDate="2023-01-01T00:00:00" endDate="2023-03-31T00:00:00"/><bounds startDate="2023-01-01T00:00:00" endDate="2025-01-01T00:00:00"/></state></timelineCacheDefinition>"#;
        let out = patch_selection(
            xml,
            Some("2024-01-01T00:00:00"),
            Some("2024-12-31T00:00:00"),
        )
        .unwrap();
        assert!(out.contains(
            r#"<selection startDate="2024-01-01T00:00:00" endDate="2024-12-31T00:00:00"/><bounds"#
        ));
        let cleared = patch_selection(xml, None, None).unwrap();
        assert!(!cleared.contains("<selection"));
        assert!(cleared.contains("<state filterType=\"unknown\"><bounds"));
        let empty = r#"<x15:timelineCacheDefinition><x15:state minimalRefreshVersion="6"/></x15:timelineCacheDefinition>"#;
        let out = patch_selection(
            empty,
            Some("2024-01-01T00:00:00"),
            Some("2024-01-31T00:00:00"),
        )
        .unwrap();
        assert_eq!(
            out,
            r#"<x15:timelineCacheDefinition><x15:state minimalRefreshVersion="6" filterType="dateBetween"><x15:selection startDate="2024-01-01T00:00:00" endDate="2024-01-31T00:00:00"/></x15:state></x15:timelineCacheDefinition>"#
        );
    }
}
