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
            });
        }
    }
    out
}

/// The cache part with its `state/selection` dates set (or the selection removed).
#[must_use]
pub fn patch_selection(xml: &str, start: Option<&str>, end: Option<&str>) -> Option<String> {
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
        assert!(cleared.contains("<bounds"));
        let empty = r#"<x15:timelineCacheDefinition><x15:state minimalRefreshVersion="6"/></x15:timelineCacheDefinition>"#;
        let out = patch_selection(
            empty,
            Some("2024-01-01T00:00:00"),
            Some("2024-01-31T00:00:00"),
        )
        .unwrap();
        assert_eq!(
            out,
            r#"<x15:timelineCacheDefinition><x15:state minimalRefreshVersion="6"><x15:selection startDate="2024-01-01T00:00:00" endDate="2024-01-31T00:00:00"/></x15:state></x15:timelineCacheDefinition>"#
        );
    }
}
