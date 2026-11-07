//! PivotTable summaries (ECMA-376 Part 1, §18.10 PivotTables).
//!
//! Workbook `pivotCaches` (§18.2.22) → pivot cache definition parts
//! (§18.10.1.67 `pivotCacheDefinition`); worksheet relationships → pivot table
//! parts (§18.10.1.73 `pivotTableDefinition`). The parts themselves stay opaque
//! package parts written back byte-for-byte; the [`PivotCache`] / [`PivotTable`]
//! values produced here feed the field list and refresh. Parse failures are
//! tolerated (the pivot is simply not summarized).

use crate::chart::{parse_dom, Node};
use bo_model::sheet::{
    CellError, CellRange, CellValue, PivotCache, PivotCacheField, PivotDataField, PivotFilter,
    PivotFunction, PivotTable,
};
use bo_opc::{resolve_target, Package, TargetMode};

/// Upper bound on items / fields kept from one part (defence against huge parts).
const MAX_ITEMS: usize = 1_000_000;

/// A shared item (§18.10.1.90 `sharedItems` children: `s`, `n`, `b`, `e`, `d`, `m`).
fn item_value(n: &Node) -> Option<CellValue> {
    let v = n.attr("v");
    Some(match n.name.as_str() {
        "s" => CellValue::Text(v.unwrap_or_default().to_owned()),
        "n" => CellValue::Number(v.and_then(|v| v.trim().parse().ok()).unwrap_or(0.0)),
        "b" => CellValue::Bool(v.is_some_and(|v| v == "1" || v == "true")),
        "e" => CellValue::Error(CellError::parse(v.unwrap_or("#N/A"))),
        "d" => CellValue::Date(v.unwrap_or_default().to_owned()),
        "m" => CellValue::Empty,
        _ => return None,
    })
}

fn num(n: &Node, attr: &str) -> Option<u32> {
    n.attr(attr).and_then(|v| v.trim().parse().ok())
}

/// Fill `cache` from its definition part.
pub(crate) fn read_cache_definition(pkg: &Package, cache: &mut PivotCache) {
    let Some(p) = pkg.part(&cache.part) else {
        return;
    };
    let Some(dom) = parse_dom(&p.data, &cache.part) else {
        return;
    };
    if dom.name != "pivotCacheDefinition" {
        return;
    }
    // Records part through the definition's `r:id` (§18.10.1.67).
    if let Some(id) = dom.attr("id") {
        let rels = pkg.relationships(&cache.part).unwrap_or_default();
        cache.records_part = rels
            .iter()
            .find(|r| r.id == id && r.mode == TargetMode::Internal)
            .map(|r| resolve_target(&cache.part, &r.target));
    }
    if let Some(ws) = dom
        .child("cacheSource")
        .and_then(|s| s.child("worksheetSource"))
    {
        cache.source_sheet = ws.attr("sheet").map(str::to_owned);
        cache.source_ref = ws.attr("ref").map(str::to_owned);
        cache.source_name = ws.attr("name").map(str::to_owned);
    }
    if let Some(fields) = dom.child("cacheFields") {
        for f in fields.children.iter().filter(|c| c.name == "cacheField") {
            let items = f
                .child("sharedItems")
                .map(|s| {
                    s.children
                        .iter()
                        .take(MAX_ITEMS)
                        .filter_map(item_value)
                        .collect()
                })
                .unwrap_or_default();
            cache.fields.push(PivotCacheField {
                name: f.attr("name").unwrap_or_default().to_owned(),
                items,
            });
            if cache.fields.len() >= MAX_ITEMS {
                break;
            }
        }
    }
}

/// Parse a pivot table part (§18.10.1.73). `caches` resolves report filter
/// selections to item values.
pub(crate) fn read_pivot_table(
    pkg: &Package,
    part: &str,
    caches: &[PivotCache],
) -> Option<PivotTable> {
    let p = pkg.part(part)?;
    let dom = parse_dom(&p.data, part)?;
    if dom.name != "pivotTableDefinition" {
        return None;
    }
    let loc = dom.child("location")?;
    let mut pt = PivotTable {
        part: part.to_owned(),
        name: dom.attr("name").unwrap_or_default().to_owned(),
        cache_id: num(&dom, "cacheId").unwrap_or(0),
        location: CellRange::parse(loc.attr("ref")?)?,
        first_header_row: num(loc, "firstHeaderRow").unwrap_or(0),
        first_data_row: num(loc, "firstDataRow").unwrap_or(0),
        first_data_col: num(loc, "firstDataCol").unwrap_or(0),
        data_on_rows: dom
            .attr("dataOnRows")
            .is_some_and(|v| v == "1" || v == "true"),
        data_caption: dom.attr("dataCaption").unwrap_or("Values").to_owned(),
        ..PivotTable::default()
    };
    let axis = |name: &str| -> Vec<u32> {
        dom.child(name)
            .map(|f| {
                f.children
                    .iter()
                    .filter(|c| c.name == "field")
                    .filter_map(|c| c.attr("x").and_then(|v| v.trim().parse::<i64>().ok()))
                    // x = -2 is the values pseudo-field (§18.10.1 `rowFields` / `colFields`).
                    .filter_map(|x| u32::try_from(x).ok())
                    .collect()
            })
            .unwrap_or_default()
    };
    pt.rows = axis("rowFields");
    pt.cols = axis("colFields");
    if let Some(df) = dom.child("dataFields") {
        for d in df.children.iter().filter(|c| c.name == "dataField") {
            pt.values.push(PivotDataField {
                field: num(d, "fld").unwrap_or(0),
                function: PivotFunction::parse(d.attr("subtotal").unwrap_or("sum")),
                name: d.attr("name").map(str::to_owned),
            });
        }
    }
    // Report filters: selection from `pageField/@item` or hidden items.
    let cache = caches.iter().find(|c| c.cache_id == pt.cache_id);
    let pivot_fields: Vec<&Node> = dom
        .child("pivotFields")
        .map(|p| {
            p.children
                .iter()
                .filter(|c| c.name == "pivotField")
                .collect()
        })
        .unwrap_or_default();
    if let Some(pf) = dom.child("pageFields") {
        for f in pf.children.iter().filter(|c| c.name == "pageField") {
            let Some(field) = num(f, "fld") else { continue };
            let items: Vec<&Node> = pivot_fields
                .get(field as usize)
                .and_then(|p| p.child("items"))
                .map(|i| i.children.iter().filter(|c| c.name == "item").collect())
                .unwrap_or_default();
            let shared = cache.and_then(|c| c.fields.get(field as usize));
            let value_of = |it: &Node| {
                let x = num(it, "x")? as usize;
                shared.and_then(|s| s.items.get(x)).cloned()
            };
            let selected: Vec<CellValue> = if let Some(i) = num(f, "item") {
                items
                    .get(i as usize)
                    .and_then(|it| value_of(it))
                    .into_iter()
                    .collect()
            } else if items.iter().any(|it| it.attr("h") == Some("1")) {
                items
                    .iter()
                    .filter(|it| it.attr("h") != Some("1") && it.attr("t").is_none())
                    .filter_map(|it| value_of(it))
                    .collect()
            } else {
                Vec::new()
            };
            pt.filters.push(PivotFilter { field, selected });
        }
    }
    Some(pt)
}

/// Pivot tables of a worksheet, following its `pivotTable` relationships.
pub(crate) fn read_sheet_pivots(
    pkg: &Package,
    sheet_part: &str,
    rels: &[bo_opc::Relationship],
    caches: &[PivotCache],
) -> Vec<PivotTable> {
    rels.iter()
        .filter(|r| r.mode == TargetMode::Internal && r.rel_type.ends_with("/pivotTable"))
        .filter_map(|r| read_pivot_table(pkg, &resolve_target(sheet_part, &r.target), caches))
        .collect()
}
