//! PivotTable writer (ECMA-376 Part 1, §18.10): pivot cache definition
//! (§18.10.1.67), pivot cache records (§18.10.1.68) and pivot table definition
//! (§18.10.1.73) parts for new and refreshed pivots.
//!
//! The generated parts describe a tabular report without subtotals that matches
//! the cells BlueSheet writes into the sheet; `refreshOnLoad` asks consumers to
//! rebuild their own layout from the cache when the file is opened.

use crate::styles::SML_NS;
use crate::xml::{a, fmt_f64};
use bo_model::sheet::{CellValue, PivotCache, PivotItemKey, PivotTable};
use bo_opc::xml::escape;
use std::collections::HashMap;
use std::fmt::Write as _;

/// Pivot table part content type (§12.3.11).
pub const CT_PIVOT_TABLE: &str =
    "application/vnd.openxmlformats-officedocument.spreadsheetml.pivotTable+xml";
/// Pivot cache definition part content type (§12.3.12).
pub const CT_PIVOT_CACHE_DEFINITION: &str =
    "application/vnd.openxmlformats-officedocument.spreadsheetml.pivotCacheDefinition+xml";
/// Pivot cache records part content type (§12.3.13).
pub const CT_PIVOT_CACHE_RECORDS: &str =
    "application/vnd.openxmlformats-officedocument.spreadsheetml.pivotCacheRecords+xml";
/// Relationship types (§12.3.11–13).
pub const REL_PIVOT_TABLE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/pivotTable";
pub const REL_PIVOT_CACHE_DEFINITION: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/pivotCacheDefinition";
pub const REL_PIVOT_CACHE_RECORDS: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/pivotCacheRecords";

const R_NS: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const XML_DECL: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n";

/// Index of each shared item of a field by grouping key.
fn item_index(items: &[CellValue]) -> HashMap<PivotItemKey, usize> {
    let mut m = HashMap::with_capacity(items.len());
    for (i, v) in items.iter().enumerate() {
        m.entry(PivotItemKey::of(v)).or_insert(i);
    }
    m
}

/// One shared item / inline record value (§18.10.1.90 children).
fn item_xml(out: &mut String, v: &CellValue) {
    match v {
        CellValue::Empty => out.push_str("<m/>"),
        CellValue::Number(n) => {
            let _ = write!(out, "<n v=\"{}\"/>", fmt_f64(*n));
        }
        CellValue::Bool(b) => {
            let _ = write!(out, "<b v=\"{}\"/>", u8::from(*b));
        }
        CellValue::Error(e) => {
            let _ = write!(out, "<e v=\"{}\"/>", escape(e.as_str()));
        }
        other => {
            let t = match other {
                CellValue::Date(d) => d.clone(),
                o => o.as_text().unwrap_or_default(),
            };
            let _ = write!(out, "<s v=\"{}\"/>", escape(&t));
        }
    }
}

/// `sharedItems` (§18.10.1.90) with the type flags of its items.
fn shared_items_xml(out: &mut String, items: &[CellValue]) {
    let mut numbers = 0usize;
    let mut integers = true;
    let mut text = false;
    let mut blank = false;
    let mut other = false;
    let (mut min, mut max) = (f64::INFINITY, f64::NEG_INFINITY);
    for v in items {
        match v {
            CellValue::Number(n) => {
                numbers += 1;
                integers &= n.fract() == 0.0;
                min = min.min(*n);
                max = max.max(*n);
            }
            CellValue::Empty => blank = true,
            CellValue::Bool(_) | CellValue::Error(_) => other = true,
            _ => text = true,
        }
    }
    out.push_str("<sharedItems");
    if numbers > 0 {
        // Numbers only (plus blanks): not "semi-mixed" unless text is present.
        if !text && !other {
            a(out, "containsSemiMixedTypes", if blank { "1" } else { "0" });
            a(out, "containsString", "0");
        }
        if text || other {
            a(out, "containsMixedTypes", "1");
        }
        a(out, "containsNumber", "1");
        if integers {
            a(out, "containsInteger", "1");
        }
        a(out, "minValue", &fmt_f64(min));
        a(out, "maxValue", &fmt_f64(max));
    } else if !text && other {
        a(out, "containsString", "0");
    }
    if blank {
        a(out, "containsBlank", "1");
    }
    if items.is_empty() {
        out.push_str("/>");
        return;
    }
    let _ = write!(out, " count=\"{}\">", items.len());
    for v in items {
        item_xml(out, v);
    }
    out.push_str("</sharedItems>");
}

/// The `pivotCacheDefinition` part; `records_rid` is the relationship id of the
/// records part.
pub fn cache_definition_xml(cache: &PivotCache, records_rid: &str) -> String {
    let mut out = String::from(XML_DECL);
    out.push_str("<pivotCacheDefinition");
    a(&mut out, "xmlns", SML_NS);
    a(&mut out, "xmlns:r", R_NS);
    a(&mut out, "r:id", records_rid);
    a(&mut out, "refreshOnLoad", "1");
    a(&mut out, "createdVersion", "3");
    a(&mut out, "refreshedVersion", "3");
    a(&mut out, "minRefreshableVersion", "3");
    a(&mut out, "recordCount", &cache.records.len().to_string());
    out.push_str("><cacheSource type=\"worksheet\"><worksheetSource");
    if let Some(r) = &cache.source_ref {
        a(&mut out, "ref", r);
    }
    if let Some(n) = &cache.source_name {
        a(&mut out, "name", n);
    }
    if let Some(s) = &cache.source_sheet {
        a(&mut out, "sheet", s);
    }
    let _ = write!(
        out,
        "/></cacheSource><cacheFields count=\"{}\">",
        cache.fields.len()
    );
    for f in &cache.fields {
        out.push_str("<cacheField");
        a(&mut out, "name", &f.name);
        a(&mut out, "numFmtId", "0");
        out.push('>');
        shared_items_xml(&mut out, &f.items);
        out.push_str("</cacheField>");
    }
    out.push_str("</cacheFields></pivotCacheDefinition>");
    out
}

/// The `pivotCacheRecords` part: every value is an index into its field's
/// shared items (`x`), or inline when the field has none.
pub fn cache_records_xml(cache: &PivotCache) -> String {
    let index: Vec<HashMap<PivotItemKey, usize>> =
        cache.fields.iter().map(|f| item_index(&f.items)).collect();
    let mut out = String::from(XML_DECL);
    out.push_str("<pivotCacheRecords");
    a(&mut out, "xmlns", SML_NS);
    a(&mut out, "xmlns:r", R_NS);
    let _ = write!(out, " count=\"{}\">", cache.records.len());
    for rec in &cache.records {
        out.push_str("<r>");
        for (i, f) in cache.fields.iter().enumerate() {
            let v = rec.get(i).unwrap_or(&CellValue::Empty);
            match index.get(i).and_then(|m| m.get(&PivotItemKey::of(v))) {
                Some(x) if !f.items.is_empty() => {
                    let _ = write!(out, "<x v=\"{x}\"/>");
                }
                _ => item_xml(&mut out, v),
            }
        }
        out.push_str("</r>");
    }
    out.push_str("</pivotCacheRecords>");
    out
}

/// Items of an axis / page field in display order (§18.10.1.45 `items`);
/// `hidden` items carry `h="1"`.
fn field_items(out: &mut String, count: usize, hidden: &dyn Fn(usize) -> bool) {
    let _ = write!(out, "<items count=\"{count}\">");
    for x in 0..count {
        if hidden(x) {
            let _ = write!(out, "<item h=\"1\" x=\"{x}\"/>");
        } else {
            let _ = write!(out, "<item x=\"{x}\"/>");
        }
    }
    out.push_str("</items>");
}

/// The `pivotTableDefinition` part of `pt` over `cache` (tabular layout,
/// no subtotals, grand totals on both axes).
pub fn pivot_table_xml(pt: &PivotTable, cache: &PivotCache) -> String {
    let mut out = String::from(XML_DECL);
    out.push_str("<pivotTableDefinition");
    a(&mut out, "xmlns", SML_NS);
    a(&mut out, "name", &pt.name);
    a(&mut out, "cacheId", &pt.cache_id.to_string());
    a(&mut out, "applyNumberFormats", "0");
    a(&mut out, "applyBorderFormats", "0");
    a(&mut out, "applyFontFormats", "0");
    a(&mut out, "applyPatternFormats", "0");
    a(&mut out, "applyAlignmentFormats", "0");
    a(&mut out, "applyWidthHeightFormats", "1");
    a(&mut out, "dataCaption", &pt.data_caption);
    if pt.data_on_rows {
        a(&mut out, "dataOnRows", "1");
    }
    a(&mut out, "updatedVersion", "3");
    a(&mut out, "minRefreshableVersion", "3");
    a(&mut out, "createdVersion", "3");
    a(&mut out, "useAutoFormatting", "1");
    a(&mut out, "itemPrintTitles", "1");
    a(&mut out, "indent", "0");
    a(&mut out, "compact", "0");
    a(&mut out, "compactData", "0");
    a(&mut out, "gridDropZones", "1");
    out.push('>');
    // §18.10.1.49 location
    out.push_str("<location");
    a(&mut out, "ref", &pt.location.to_string());
    a(&mut out, "firstHeaderRow", &pt.first_header_row.to_string());
    a(&mut out, "firstDataRow", &pt.first_data_row.to_string());
    a(&mut out, "firstDataCol", &pt.first_data_col.to_string());
    if !pt.filters.is_empty() {
        a(&mut out, "rowPageCount", &pt.filters.len().to_string());
        a(&mut out, "colPageCount", "1");
    }
    out.push_str("/>");
    // §18.10.1.76 pivotFields: one per cache field.
    let _ = write!(out, "<pivotFields count=\"{}\">", cache.fields.len());
    for (i, f) in cache.fields.iter().enumerate() {
        let i = i as u32;
        let axis = if pt.rows.contains(&i) {
            Some("axisRow")
        } else if pt.cols.contains(&i) {
            Some("axisCol")
        } else if pt.filters.iter().any(|p| p.field == i) {
            Some("axisPage")
        } else {
            None
        };
        out.push_str("<pivotField");
        if let Some(ax) = axis {
            a(&mut out, "axis", ax);
        }
        if pt.values.iter().any(|d| d.field == i) {
            a(&mut out, "dataField", "1");
        }
        a(&mut out, "compact", "0");
        a(&mut out, "outline", "0");
        let filter = pt.filters.iter().find(|p| p.field == i);
        let multi = filter.is_some_and(|p| p.selected.len() > 1);
        if multi {
            a(&mut out, "multipleItemSelectionAllowed", "1");
        }
        a(&mut out, "showAll", "0");
        a(&mut out, "defaultSubtotal", "0");
        if axis.is_some() && !f.items.is_empty() {
            out.push('>');
            let shown: Vec<PivotItemKey> = filter
                .filter(|_| multi)
                .map(|p| p.selected.iter().map(PivotItemKey::of).collect())
                .unwrap_or_default();
            let hidden =
                |x: usize| !shown.is_empty() && !shown.contains(&PivotItemKey::of(&f.items[x]));
            field_items(&mut out, f.items.len(), &hidden);
            out.push_str("</pivotField>");
        } else {
            out.push_str("/>");
        }
    }
    out.push_str("</pivotFields>");
    let data_axis = pt.values.len() > 1;
    let axis_fields = |out: &mut String, tag: &str, fields: &[u32], with_data: bool| {
        let n = fields.len() + usize::from(with_data);
        if n == 0 {
            return;
        }
        let _ = write!(out, "<{tag} count=\"{n}\">");
        for f in fields {
            let _ = write!(out, "<field x=\"{f}\"/>");
        }
        if with_data {
            out.push_str("<field x=\"-2\"/>");
        }
        let _ = write!(out, "</{tag}>");
    };
    axis_fields(
        &mut out,
        "rowFields",
        &pt.rows,
        data_axis && pt.data_on_rows,
    );
    axis_fields(
        &mut out,
        "colFields",
        &pt.cols,
        data_axis && !pt.data_on_rows,
    );
    if !pt.filters.is_empty() {
        let _ = write!(out, "<pageFields count=\"{}\">", pt.filters.len());
        for p in &pt.filters {
            out.push_str("<pageField");
            a(&mut out, "fld", &p.field.to_string());
            if let [one] = p.selected.as_slice() {
                let key = PivotItemKey::of(one);
                if let Some(x) = cache
                    .fields
                    .get(p.field as usize)
                    .and_then(|f| f.items.iter().position(|v| PivotItemKey::of(v) == key))
                {
                    a(&mut out, "item", &x.to_string());
                }
            }
            a(&mut out, "hier", "-1");
            out.push_str("/>");
        }
        out.push_str("</pageFields>");
    }
    if !pt.values.is_empty() {
        let _ = write!(out, "<dataFields count=\"{}\">", pt.values.len());
        for d in &pt.values {
            out.push_str("<dataField");
            if let Some(n) = &d.name {
                a(&mut out, "name", n);
            }
            a(&mut out, "fld", &d.field.to_string());
            if d.function != bo_model::sheet::PivotFunction::Sum {
                a(&mut out, "subtotal", d.function.as_str());
            }
            a(&mut out, "baseField", "0");
            a(&mut out, "baseItem", "0");
            out.push_str("/>");
        }
        out.push_str("</dataFields>");
    }
    out.push_str("</pivotTableDefinition>");
    out
}

/// A relationships part with one relationship `rId1` of `rel_type` to `target`.
pub fn single_rels_xml(rel_type: &str, target: &str) -> String {
    format!(
        "{XML_DECL}<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"{rel_type}\" Target=\"{}\"/></Relationships>",
        escape(target)
    )
}
