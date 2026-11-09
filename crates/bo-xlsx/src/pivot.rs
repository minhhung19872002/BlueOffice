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
    CellError, CellRange, CellValue, PivotAxisItem, PivotCache, PivotCacheField,
    PivotConditionalFormat, PivotDataField, PivotFieldFilter, PivotFieldGroup, PivotFieldOptions,
    PivotFilter, PivotFilterType, PivotFunction, PivotGroupBy, PivotLayout, PivotRangeGroup,
    PivotShowAs, PivotSort, PivotStyleInfo, PivotTable,
};
use bo_opc::{resolve_target, Package, TargetMode};

/// Upper bound on items / fields kept from one part (defence against huge parts).
const MAX_ITEMS: usize = 1_000_000;

/// `reference/@field` of the values pseudo-field (−2 as `xsd:unsignedInt`).
const DATA_FIELD_REF: &str = "4294967294";

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

fn float(n: &Node, attr: &str) -> Option<f64> {
    n.attr(attr).and_then(|v| v.trim().parse().ok())
}

/// `xsd:boolean` attribute with its schema default.
fn flag(n: &Node, attr: &str, default: bool) -> bool {
    n.attr(attr).map_or(default, |v| v == "1" || v == "true")
}

/// Days from 1970-01-01 of a proleptic Gregorian date.
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// 1900-system serial of an `xsd:dateTime` (§18.17.4.1; dates from 1900-03-01).
pub(crate) fn iso_to_serial(s: &str) -> Option<f64> {
    let (date, time) = s.trim().split_once('T').unwrap_or((s.trim(), ""));
    let mut it = date.splitn(3, '-');
    let y: i64 = it.next()?.parse().ok()?;
    let m: i64 = it.next()?.parse().ok()?;
    let d: i64 = it.next()?.parse().ok()?;
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    let mut serial = (days_from_civil(y, m, d) - days_from_civil(1899, 12, 30)) as f64;
    let time = time.trim_end_matches('Z');
    if !time.is_empty() {
        let mut p = time.split(':');
        let h: f64 = p.next().and_then(|v| v.parse().ok()).unwrap_or(0.0);
        let mi: f64 = p.next().and_then(|v| v.parse().ok()).unwrap_or(0.0);
        let sec: f64 = p.next().and_then(|v| v.parse().ok()).unwrap_or(0.0);
        serial += (h * 3600.0 + mi * 60.0 + sec) / 86_400.0;
    }
    Some(serial)
}

/// `xsd:dateTime` of a 1900-system serial (inverse of [`iso_to_serial`]).
pub(crate) fn serial_to_iso(serial: f64) -> String {
    let days = serial.floor();
    let z = days as i64 + days_from_civil(1899, 12, 30) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    let secs = ((serial - days) * 86_400.0).round() as i64;
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}",
        secs / 3600,
        secs / 60 % 60,
        secs % 60
    )
}

/// Days between serial 0 of the 1900 and of the 1904 date system
/// (§18.17.4.1: 1904-01-01 is serial 1462 in the 1900 system).
pub(crate) const DATE1904_OFFSET: f64 = 1462.0;

/// Serial in the workbook's date system of an `xsd:dateTime` (§18.17.4).
pub(crate) fn iso_to_serial_in(s: &str, date1904: bool) -> Option<f64> {
    iso_to_serial(s).map(|v| if date1904 { v - DATE1904_OFFSET } else { v })
}

/// `xsd:dateTime` of a serial in the workbook's date system (§18.17.4).
pub(crate) fn serial_to_iso_in(serial: f64, date1904: bool) -> String {
    serial_to_iso(if date1904 {
        serial + DATE1904_OFFSET
    } else {
        serial
    })
}

/// `fieldGroup` (§18.10.1.27) with `rangePr` (§18.10.1.77), `discretePr`
/// (§18.10.1.26) and `groupItems` (§18.10.1.37). `startDate` / `endDate` are
/// read into serials of the workbook's date system.
fn read_group(g: &Node, date1904: bool) -> PivotFieldGroup {
    let range = g.child("rangePr").map(|r| {
        let dates = r.attr("startDate").is_some() || r.attr("endDate").is_some();
        let bound = |n: &str, d: &str| {
            if dates {
                r.attr(d).and_then(|v| iso_to_serial_in(v, date1904))
            } else {
                float(r, n)
            }
        };
        PivotRangeGroup {
            group_by: PivotGroupBy::parse(r.attr("groupBy").unwrap_or("range")),
            auto_start: flag(r, "autoStart", true),
            auto_end: flag(r, "autoEnd", true),
            start: bound("startNum", "startDate").unwrap_or(0.0),
            end: bound("endNum", "endDate").unwrap_or(0.0),
            interval: float(r, "groupInterval").unwrap_or(1.0),
            dates,
        }
    });
    PivotFieldGroup {
        par: num(g, "par"),
        base: num(g, "base"),
        range,
        discrete: g
            .child("discretePr")
            .map(|d| {
                d.children
                    .iter()
                    .filter(|c| c.name == "x")
                    .take(MAX_ITEMS)
                    .map(|x| num(x, "v").unwrap_or(0))
                    .collect()
            })
            .unwrap_or_default(),
        items: g
            .child("groupItems")
            .map(|s| {
                s.children
                    .iter()
                    .take(MAX_ITEMS)
                    .filter_map(item_value)
                    .collect()
            })
            .unwrap_or_default(),
    }
}

/// Fill `cache` from its definition part.
pub(crate) fn read_cache_definition(pkg: &Package, cache: &mut PivotCache, date1904: bool) {
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
                group: f.child("fieldGroup").map(|g| read_group(g, date1904)),
                formula: f.attr("formula").map(str::to_owned),
                derived: !flag(f, "databaseField", true),
                num_fmt_id: num(f, "numFmtId").unwrap_or(0),
            });
            if cache.fields.len() >= MAX_ITEMS {
                break;
            }
        }
    }
    // §18.10.1.9 calculatedItems: the item is the cache item its pivotArea
    // (§18.3.1.68) references.
    if let Some(list) = dom.child("calculatedItems") {
        for ci in list
            .children
            .iter()
            .filter(|c| c.name == "calculatedItem")
            .take(MAX_ITEMS)
        {
            let Some(reference) = ci
                .child("pivotArea")
                .and_then(|p| p.child("references"))
                .and_then(|r| r.child("reference"))
            else {
                continue;
            };
            let Some(field) = num(reference, "field").or_else(|| num(ci, "field")) else {
                continue;
            };
            let Some(x) = reference.child("x").map(|x| num(x, "v").unwrap_or(0)) else {
                continue;
            };
            let Some(name) = cache
                .fields
                .get(field as usize)
                .and_then(|f| f.items.get(x as usize))
                .and_then(CellValue::as_text)
            else {
                continue;
            };
            let formula = ci.attr("formula").unwrap_or_default();
            cache.calculated_items.push(bo_model::sheet::PivotCalcItem {
                field,
                name,
                formula: formula.strip_prefix('=').unwrap_or(formula).to_owned(),
            });
        }
    }
}

/// Items of a field as an axis shows them: its group captions when grouped,
/// otherwise its shared items.
fn axis_values(cache: Option<&PivotCache>, field: u32) -> &[CellValue] {
    cache
        .and_then(|c| c.fields.get(field as usize))
        .map_or(&[], |f| match &f.group {
            Some(g) if g.range.is_some() || !g.discrete.is_empty() => g.items.as_slice(),
            _ => f.items.as_slice(),
        })
}

/// `rowItems` / `colItems` lines (§18.10.1.44 `i`).
fn read_axis_items(n: Option<&Node>) -> Vec<PivotAxisItem> {
    n.map(|n| {
        n.children
            .iter()
            .filter(|c| c.name == "i")
            .take(MAX_ITEMS)
            .map(|i| PivotAxisItem {
                kind: i
                    .attr("t")
                    .filter(|t| *t != "data")
                    .unwrap_or_default()
                    .to_owned(),
                repeat: num(i, "r").unwrap_or(0),
                data_index: num(i, "i").unwrap_or(0),
                items: i
                    .children
                    .iter()
                    .filter(|x| x.name == "x")
                    .map(|x| num(x, "v").unwrap_or(0))
                    .collect(),
            })
            .collect()
    })
    .unwrap_or_default()
}

/// An `ext` child of `extLst` by local element name.
fn ext_child<'a>(n: &'a Node, name: &str) -> Option<&'a Node> {
    n.child("extLst")?
        .children
        .iter()
        .filter(|e| e.name == "ext")
        .find_map(|e| e.child(name))
}

/// `pivotField` settings (§18.10.1.69).
fn read_field_options(f: &Node, values: &[CellValue]) -> PivotFieldOptions {
    let sort_by = f.child("autoSortScope").and_then(|s| {
        let refs = s.child("pivotArea")?.child("references")?;
        refs.children
            .iter()
            .filter(|r| r.name == "reference")
            .find(|r| r.attr("field") == Some(DATA_FIELD_REF))
            .and_then(|r| r.child("x"))
            .map(|x| num(x, "v").unwrap_or(0))
    });
    let hidden = f
        .child("items")
        .map(|i| {
            i.children
                .iter()
                .filter(|c| c.name == "item" && flag(c, "h", false) && c.attr("t").is_none())
                .filter_map(|c| values.get(num(c, "x")? as usize).cloned())
                .collect()
        })
        .unwrap_or_default();
    PivotFieldOptions {
        subtotal: flag(f, "defaultSubtotal", true),
        subtotal_top: flag(f, "subtotalTop", true),
        blank_row: flag(f, "insertBlankRow", false),
        repeat_labels: ext_child(f, "pivotField").is_some_and(|x| flag(x, "fillDownLabels", false)),
        sort: PivotSort::parse(f.attr("sortType").unwrap_or("manual")),
        sort_by,
        hidden,
    }
}

/// A label / value / top-N filter (§18.10.1.35 `filter`).
fn read_field_filter(f: &Node) -> Option<PivotFieldFilter> {
    let kind = PivotFilterType::parse(f.attr("type")?)?;
    let col = f.child("autoFilter").and_then(|a| a.child("filterColumn"));
    let top10 = col.and_then(|c| c.child("top10"));
    let custom: Vec<&str> = col
        .and_then(|c| c.child("customFilters"))
        .map(|c| {
            c.children
                .iter()
                .filter(|x| x.name == "customFilter")
                .filter_map(|x| x.attr("val"))
                .collect()
        })
        .unwrap_or_default();
    let value = |attr: &str, i: usize| {
        f.attr(attr)
            .map(str::to_owned)
            .or_else(|| (!kind.is_caption()).then(|| custom.get(i).map(|s| (*s).to_owned()))?)
            .unwrap_or_default()
    };
    Some(PivotFieldFilter {
        field: num(f, "fld")?,
        kind,
        measure: num(f, "iMeasureFld").unwrap_or(0),
        value1: value("stringValue1", 0),
        value2: value("stringValue2", 1),
        top: top10.is_none_or(|t| flag(t, "top", true)),
        count: top10.and_then(|t| float(t, "val")).unwrap_or(0.0),
    })
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
        data_on_rows: flag(&dom, "dataOnRows", false),
        data_caption: dom.attr("dataCaption").unwrap_or("Values").to_owned(),
        row_grand_totals: flag(&dom, "rowGrandTotals", true),
        col_grand_totals: flag(&dom, "colGrandTotals", true),
        row_items: read_axis_items(dom.child("rowItems")),
        col_items: read_axis_items(dom.child("colItems")),
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
            // x14:dataField/@pivotShowAs overrides showDataAs ([MS-XLSX] `x14:dataField`).
            let show = ext_child(d, "dataField")
                .and_then(|x| x.attr("pivotShowAs"))
                .or_else(|| d.attr("showDataAs"))
                .unwrap_or("normal");
            pt.values.push(PivotDataField {
                field: num(d, "fld").unwrap_or(0),
                function: PivotFunction::parse(d.attr("subtotal").unwrap_or("sum")),
                name: d.attr("name").map(str::to_owned),
                show_as: PivotShowAs::parse(show),
                base_field: num(d, "baseField").unwrap_or(0),
                base_item: num(d, "baseItem").unwrap_or(0),
                num_fmt_id: num(d, "numFmtId"),
            });
        }
    }
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
    let page: Vec<u32> = dom
        .child("pageFields")
        .map(|pf| {
            pf.children
                .iter()
                .filter(|c| c.name == "pageField")
                .filter_map(|c| num(c, "fld"))
                .collect()
        })
        .unwrap_or_default();
    // Field settings; report filters keep their hidden items as the selection.
    for (i, f) in pivot_fields.iter().enumerate() {
        let i = i as u32;
        let mut o = read_field_options(f, axis_values(cache, i));
        if page.contains(&i) {
            o.hidden.clear();
        }
        if o != PivotFieldOptions::default() {
            *pt.field_mut(i) = o;
        }
    }
    // Layout (§18.10.1.73 `compact` / `outline`, §18.10.1.69 per field): the
    // first row field decides, else the table attributes.
    let (compact, outline) = match pt.rows.first().and_then(|f| pivot_fields.get(*f as usize)) {
        Some(f) => (flag(f, "compact", true), flag(f, "outline", true)),
        None => (flag(&dom, "compact", true), flag(&dom, "outline", false)),
    };
    pt.layout = if compact && outline {
        PivotLayout::Compact
    } else if outline {
        PivotLayout::Outline
    } else if compact && pt.rows.is_empty() {
        PivotLayout::Compact
    } else {
        PivotLayout::Tabular
    };
    // Report filters: selection from `pageField/@item` or hidden items.
    if let Some(pf) = dom.child("pageFields") {
        for f in pf.children.iter().filter(|c| c.name == "pageField") {
            let Some(field) = num(f, "fld") else { continue };
            let items: Vec<&Node> = pivot_fields
                .get(field as usize)
                .and_then(|p| p.child("items"))
                .map(|i| i.children.iter().filter(|c| c.name == "item").collect())
                .unwrap_or_default();
            let shared = axis_values(cache, field);
            let value_of = |it: &Node| {
                let x = num(it, "x")? as usize;
                shared.get(x).cloned()
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
    if let Some(fl) = dom.child("filters") {
        pt.field_filters = fl
            .children
            .iter()
            .filter(|c| c.name == "filter")
            .filter_map(read_field_filter)
            .collect();
    }
    // §18.10.1.16 conditionalFormats / §18.10.1.15 conditionalFormat
    if let Some(cfs) = dom.child("conditionalFormats") {
        pt.conditional_formats = cfs
            .children
            .iter()
            .filter(|c| c.name == "conditionalFormat")
            .map(|c| PivotConditionalFormat {
                priority: c
                    .attr("priority")
                    .and_then(|v| v.trim().parse().ok())
                    .unwrap_or(1),
                scope: c.attr("scope").map(str::to_owned),
                kind: c.attr("type").map(str::to_owned),
                areas: c
                    .child("pivotAreas")
                    .map(|a| {
                        a.children
                            .iter()
                            .filter(|p| p.name == "pivotArea")
                            .map(node_xml)
                            .collect()
                    })
                    .unwrap_or_default(),
            })
            .collect();
    }
    pt.style = dom.child("pivotTableStyleInfo").map(|s| PivotStyleInfo {
        name: s.attr("name").unwrap_or_default().to_owned(),
        show_row_headers: flag(s, "showRowHeaders", false),
        show_col_headers: flag(s, "showColHeaders", false),
        show_row_stripes: flag(s, "showRowStripes", false),
        show_col_stripes: flag(s, "showColStripes", false),
        show_last_column: flag(s, "showLastColumn", false),
    });
    Some(pt)
}

/// A pivot table element (default SpreadsheetML namespace) serialized back to XML.
fn node_xml(n: &Node) -> String {
    let mut out = String::new();
    write_node(&mut out, n);
    out
}

fn write_node(out: &mut String, n: &Node) {
    out.push('<');
    out.push_str(&n.name);
    for (k, v) in &n.attrs {
        crate::xml::a(out, k, v);
    }
    if n.children.is_empty() && n.text.is_empty() {
        out.push_str("/>");
        return;
    }
    out.push('>');
    out.push_str(&bo_opc::xml::escape(&n.text));
    for c in &n.children {
        write_node(out, c);
    }
    out.push_str("</");
    out.push_str(&n.name);
    out.push('>');
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iso_dates_round_trip() {
        // 2024-01-15 is serial 45306 (1900 system); 1900-03-01 is 61.
        assert_eq!(iso_to_serial("2024-01-15T00:00:00"), Some(45306.0));
        assert_eq!(iso_to_serial("1900-03-01"), Some(61.0));
        assert_eq!(serial_to_iso(45306.5), "2024-01-15T12:00:00");
        assert_eq!(iso_to_serial(&serial_to_iso(45657.0)), Some(45657.0));
        assert_eq!(iso_to_serial("2024-13-01"), None);
    }
}
