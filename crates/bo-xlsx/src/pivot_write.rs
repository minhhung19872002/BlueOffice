//! PivotTable writer (ECMA-376 Part 1, §18.10): pivot cache definition
//! (§18.10.1.67), pivot cache records (§18.10.1.68) and pivot table definition
//! (§18.10.1.73) parts for new and refreshed pivots.
//!
//! The generated parts describe the report BlueSheet writes into the sheet
//! (layout, subtotals, grand totals, `rowItems` / `colItems`, filters, "show
//! values as", style); `refreshOnLoad` asks consumers to rebuild their own
//! layout from the cache when the file is opened. Settings that only exist in
//! the Office 2010 schema (`fillDownLabels`, the newer "show values as"
//! calculations) are written as `x14` extensions ([MS-XLSX] `CT_PivotFieldExtension`, `CT_DataField` x14).

use crate::pivot::serial_to_iso_in;
use crate::styles::SML_NS;
use crate::xml::{a, fmt_f64};
use bo_model::sheet::{
    CellValue, PivotAxisItem, PivotCache, PivotCacheField, PivotFieldFilter, PivotFieldGroup,
    PivotFilterType, PivotGroupBy, PivotItemKey, PivotLayout, PivotShowAs, PivotSort, PivotTable,
};
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
const X14_NS: &str = "http://schemas.microsoft.com/office/spreadsheetml/2009/9/main";
/// `ext/@uri` of `x14:pivotField` and `x14:dataField` ([MS-XLSX] extLst extension URIs).
const EXT_PIVOT_FIELD: &str = "{2946ED86-A175-432a-8AC1-64E0C546D7DE}";
const EXT_DATA_FIELD: &str = "{E15A36E0-9728-4e99-A89B-3F7291B0FE68}";
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
        CellValue::Date(d) => {
            let _ = write!(out, "<d v=\"{}\"/>", escape(d));
        }
        other => {
            let t = other.as_text().unwrap_or_default();
            let _ = write!(out, "<s v=\"{}\"/>", escape(&t));
        }
    }
}

/// Items of a date-grouped base field as dates (`d`, §18.10.1.21); serials are
/// in the workbook's date system (§18.17.4.1).
fn as_dates(items: &[CellValue], date1904: bool) -> Vec<CellValue> {
    items
        .iter()
        .map(|v| match v {
            CellValue::Number(n) => CellValue::Date(serial_to_iso_in(*n, date1904)),
            o => o.clone(),
        })
        .collect()
}

/// `sharedItems` (§18.10.1.90) with the type flags of its items.
fn shared_items_xml(out: &mut String, items: &[CellValue]) {
    let mut numbers = 0usize;
    let mut dates = 0usize;
    let mut integers = true;
    let mut text = false;
    let mut blank = false;
    let mut other = false;
    let (mut min, mut max) = (f64::INFINITY, f64::NEG_INFINITY);
    let (mut min_d, mut max_d) = (String::new(), String::new());
    for v in items {
        match v {
            CellValue::Number(n) => {
                numbers += 1;
                integers &= n.fract() == 0.0;
                min = min.min(*n);
                max = max.max(*n);
            }
            CellValue::Date(d) => {
                dates += 1;
                if min_d.is_empty() || *d < min_d {
                    min_d.clone_from(d);
                }
                if max_d.is_empty() || *d > max_d {
                    max_d.clone_from(d);
                }
            }
            CellValue::Empty => blank = true,
            CellValue::Bool(_) | CellValue::Error(_) => other = true,
            _ => text = true,
        }
    }
    out.push_str("<sharedItems");
    if dates > 0 && numbers == 0 && !text && !other {
        // Dates only (plus blanks), §18.10.1.90 containsDate / containsNonDate.
        a(out, "containsSemiMixedTypes", if blank { "1" } else { "0" });
        a(out, "containsNonDate", "0");
        a(out, "containsDate", "1");
        a(out, "containsString", "0");
        a(out, "minDate", &min_d);
        a(out, "maxDate", &max_d);
    } else if numbers > 0 {
        // Numbers only (plus blanks): not "semi-mixed" unless text is present.
        if !text && !other && dates == 0 {
            a(out, "containsSemiMixedTypes", if blank { "1" } else { "0" });
            a(out, "containsString", "0");
        }
        if text || other || dates > 0 {
            a(out, "containsMixedTypes", "1");
        }
        if dates > 0 {
            a(out, "containsDate", "1");
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

/// `fieldGroup` (§18.10.1.27): `rangePr` (§18.10.1.77), `discretePr`
/// (§18.10.1.26), `groupItems` (§18.10.1.37).
fn field_group_xml(out: &mut String, g: &PivotFieldGroup, date1904: bool) {
    out.push_str("<fieldGroup");
    if let Some(p) = g.par {
        a(out, "par", &p.to_string());
    }
    if let Some(b) = g.base {
        a(out, "base", &b.to_string());
    }
    out.push('>');
    if let Some(r) = &g.range {
        out.push_str("<rangePr");
        if !r.auto_start {
            a(out, "autoStart", "0");
        }
        if !r.auto_end {
            a(out, "autoEnd", "0");
        }
        if r.group_by != PivotGroupBy::Range {
            a(out, "groupBy", r.group_by.as_str());
        }
        if r.dates {
            // xsd:dateTime: the same instant whatever the date system.
            a(out, "startDate", &serial_to_iso_in(r.start, date1904));
            a(out, "endDate", &serial_to_iso_in(r.end, date1904));
        } else {
            a(out, "startNum", &fmt_f64(r.start));
            a(out, "endNum", &fmt_f64(r.end));
        }
        if r.interval != 1.0 {
            a(out, "groupInterval", &fmt_f64(r.interval));
        }
        out.push_str("/>");
    }
    if !g.discrete.is_empty() {
        let _ = write!(out, "<discretePr count=\"{}\">", g.discrete.len());
        for x in &g.discrete {
            let _ = write!(out, "<x v=\"{x}\"/>");
        }
        out.push_str("</discretePr>");
    }
    if !g.items.is_empty() {
        let _ = write!(out, "<groupItems count=\"{}\">", g.items.len());
        for v in &g.items {
            item_xml(out, v);
        }
        out.push_str("</groupItems>");
    }
    out.push_str("</fieldGroup>");
}

/// Whether `f` (at index `i`) is a date-grouped base field.
fn date_base(f: &PivotCacheField, i: usize) -> bool {
    f.group.as_ref().is_some_and(|g| {
        g.base.is_none_or(|b| b as usize == i) && g.range.as_ref().is_some_and(|r| r.dates)
    })
}

/// The `pivotCacheDefinition` part; `records_rid` is the relationship id of the
/// records part. `date1904`: the workbook's date system (`workbookPr/@date1904`),
/// in which the serials of the cache are counted.
pub fn cache_definition_xml(cache: &PivotCache, records_rid: &str, date1904: bool) -> String {
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
    for (i, f) in cache.fields.iter().enumerate() {
        // §18.10.1.3 cacheField
        out.push_str("<cacheField");
        a(&mut out, "name", &f.name);
        a(&mut out, "numFmtId", &f.num_fmt_id.to_string());
        if let Some(formula) = &f.formula {
            a(&mut out, "formula", formula);
        }
        if f.derived {
            a(&mut out, "databaseField", "0");
        }
        out.push('>');
        if !f.derived {
            if date_base(f, i) {
                shared_items_xml(&mut out, &as_dates(&f.items, date1904));
            } else {
                shared_items_xml(&mut out, &f.items);
            }
        }
        if let Some(g) = &f.group {
            field_group_xml(&mut out, g, date1904);
        }
        out.push_str("</cacheField>");
    }
    out.push_str("</cacheFields>");
    calculated_items_xml(&mut out, cache);
    out.push_str("</pivotCacheDefinition>");
    out
}

/// `calculatedItems` (§18.10.1.9): each `calculatedItem` (§18.10.1.8) names
/// its item through a `pivotArea` (§18.3.1.68) referencing the cache item.
fn calculated_items_xml(out: &mut String, cache: &PivotCache) {
    let list: Vec<(u32, usize, &str)> = cache
        .calculated_items
        .iter()
        .filter_map(|ci| {
            let key = PivotItemKey::of(&CellValue::Text(ci.name.clone()));
            let x = cache
                .fields
                .get(ci.field as usize)?
                .items
                .iter()
                .position(|v| PivotItemKey::of(v) == key)?;
            Some((ci.field, x, ci.formula.as_str()))
        })
        .collect();
    if list.is_empty() {
        return;
    }
    let _ = write!(out, "<calculatedItems count=\"{}\">", list.len());
    for (field, x, formula) in list {
        out.push_str("<calculatedItem");
        a(out, "formula", formula);
        let _ = write!(
            out,
            "><pivotArea cacheIndex=\"1\" outline=\"0\" fieldPosition=\"0\"><references count=\"1\"><reference field=\"{field}\" count=\"1\"><x v=\"{x}\"/></reference></references></pivotArea></calculatedItem>"
        );
    }
    out.push_str("</calculatedItems>");
}

/// The `pivotCacheRecords` part: every value of a database field is an index
/// into its shared items (`x`), or inline when the field has none.
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
        for (i, f) in cache.fields.iter().enumerate().filter(|(_, f)| !f.derived) {
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

/// Items of a field as the axis shows them (group captions when grouped).
fn axis_values(f: &PivotCacheField) -> &[CellValue] {
    match &f.group {
        Some(g) if g.range.is_some() || !g.discrete.is_empty() => &g.items,
        _ => &f.items,
    }
}

/// `rowItems` / `colItems` (§18.10.1.84 / §18.10.1.10).
fn axis_items_xml(out: &mut String, tag: &str, items: &[PivotAxisItem]) {
    if items.is_empty() {
        return;
    }
    let _ = write!(out, "<{tag} count=\"{}\">", items.len());
    for it in items {
        out.push_str("<i");
        if !it.kind.is_empty() {
            a(out, "t", &it.kind);
        }
        if it.repeat > 0 {
            a(out, "r", &it.repeat.to_string());
        }
        if it.data_index > 0 {
            a(out, "i", &it.data_index.to_string());
        }
        if it.items.is_empty() {
            out.push_str("/>");
            continue;
        }
        out.push('>');
        for x in &it.items {
            if *x == 0 {
                out.push_str("<x/>");
            } else {
                let _ = write!(out, "<x v=\"{x}\"/>");
            }
        }
        out.push_str("</i>");
    }
    let _ = write!(out, "</{tag}>");
}

/// The AutoFilter criteria of a label / value / top-N filter (§18.10.1.35 →
/// §18.3.2 `filterColumn`).
fn filter_xml(out: &mut String, f: &PivotFieldFilter, id: usize) {
    use PivotFilterType as T;
    out.push_str("<filter");
    a(out, "fld", &f.field.to_string());
    a(out, "type", f.kind.as_str());
    a(out, "evalOrder", "-1");
    a(out, "id", &id.to_string());
    if !f.kind.is_caption() && !f.kind.is_date() {
        a(out, "iMeasureFld", &f.measure.to_string());
    }
    if f.kind.is_caption() {
        a(out, "stringValue1", &f.value1);
        if matches!(f.kind, T::CaptionBetween | T::CaptionNotBetween) {
            a(out, "stringValue2", &f.value2);
        }
    }
    out.push_str("><autoFilter ref=\"A1\"><filterColumn colId=\"0\">");
    let custom = |out: &mut String, and: bool, list: &[(&str, String)]| {
        out.push_str("<customFilters");
        if and {
            a(out, "and", "1");
        }
        out.push('>');
        for (op, val) in list {
            out.push_str("<customFilter");
            if !op.is_empty() {
                a(out, "operator", op);
            }
            a(out, "val", val);
            out.push_str("/>");
        }
        out.push_str("</customFilters>");
    };
    let (v1, v2) = (f.value1.clone(), f.value2.clone());
    match f.kind {
        T::Count | T::Percent | T::Sum => {
            out.push_str("<top10");
            if !f.top {
                a(out, "top", "0");
            }
            if f.kind == T::Percent {
                a(out, "percent", "1");
            }
            a(out, "val", &fmt_f64(f.count));
            a(out, "filterVal", &fmt_f64(f.count));
            out.push_str("/>");
        }
        // Date filters use serial dates (§18.3.2.2 customFilter).
        T::CaptionEqual | T::ValueEqual | T::DateEqual => custom(out, false, &[("", v1)]),
        T::CaptionNotEqual | T::ValueNotEqual | T::DateNotEqual => {
            custom(out, false, &[("notEqual", v1)]);
        }
        T::CaptionBeginsWith => custom(out, false, &[("", format!("{v1}*"))]),
        T::CaptionNotBeginsWith => custom(out, false, &[("notEqual", format!("{v1}*"))]),
        T::CaptionEndsWith => custom(out, false, &[("", format!("*{v1}"))]),
        T::CaptionNotEndsWith => custom(out, false, &[("notEqual", format!("*{v1}"))]),
        T::CaptionContains => custom(out, false, &[("", format!("*{v1}*"))]),
        T::CaptionNotContains => custom(out, false, &[("notEqual", format!("*{v1}*"))]),
        T::CaptionGreaterThan | T::ValueGreaterThan | T::DateNewerThan => {
            custom(out, false, &[("greaterThan", v1)]);
        }
        T::CaptionGreaterThanOrEqual | T::ValueGreaterThanOrEqual | T::DateNewerThanOrEqual => {
            custom(out, false, &[("greaterThanOrEqual", v1)]);
        }
        T::CaptionLessThan | T::ValueLessThan | T::DateOlderThan => {
            custom(out, false, &[("lessThan", v1)]);
        }
        T::CaptionLessThanOrEqual | T::ValueLessThanOrEqual | T::DateOlderThanOrEqual => {
            custom(out, false, &[("lessThanOrEqual", v1)]);
        }
        T::CaptionBetween | T::ValueBetween | T::DateBetween => custom(
            out,
            true,
            &[("greaterThanOrEqual", v1), ("lessThanOrEqual", v2)],
        ),
        T::CaptionNotBetween | T::ValueNotBetween | T::DateNotBetween => {
            custom(out, false, &[("lessThan", v1), ("greaterThan", v2)]);
        }
        // Relative periods, quarters and months as a dynamic filter
        // (§18.3.2.5 dynamicFilter, §18.18.26 ST_DynamicFilterType).
        _ => {
            out.push_str("<dynamicFilter");
            a(out, "type", f.kind.as_str());
            out.push_str("/>");
        }
    }
    out.push_str("</filterColumn></autoFilter></filter>");
}

/// The `pivotTableDefinition` part of `pt` over `cache`.
pub fn pivot_table_xml(pt: &PivotTable, cache: &PivotCache) -> String {
    let x14 =
        pt.values.iter().any(|d| d.show_as.is_x14()) || pt.fields.iter().any(|f| f.repeat_labels);
    let version = if x14 { "4" } else { "3" };
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
    if !pt.row_grand_totals {
        a(&mut out, "rowGrandTotals", "0");
    }
    if !pt.col_grand_totals {
        a(&mut out, "colGrandTotals", "0");
    }
    a(&mut out, "updatedVersion", version);
    a(&mut out, "minRefreshableVersion", "3");
    a(&mut out, "createdVersion", version);
    a(&mut out, "useAutoFormatting", "1");
    a(&mut out, "itemPrintTitles", "1");
    // Compact layout keeps the default indent of one character per level.
    if pt.layout != PivotLayout::Compact {
        a(&mut out, "indent", "0");
    }
    // §18.10.1.73 compact / compactData / outline / outlineData.
    match pt.layout {
        PivotLayout::Compact => {
            a(&mut out, "outline", "1");
            a(&mut out, "outlineData", "1");
        }
        PivotLayout::Outline => {
            a(&mut out, "compact", "0");
            a(&mut out, "compactData", "0");
            a(&mut out, "outline", "1");
            a(&mut out, "outlineData", "1");
        }
        PivotLayout::Tabular => {
            a(&mut out, "compact", "0");
            a(&mut out, "compactData", "0");
            a(&mut out, "gridDropZones", "1");
        }
    }
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
        let opts = pt.field(i);
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
        // §18.10.1.69 compact (default true) / outline (default true).
        match pt.layout {
            PivotLayout::Compact => {}
            PivotLayout::Outline => a(&mut out, "compact", "0"),
            PivotLayout::Tabular => {
                a(&mut out, "compact", "0");
                a(&mut out, "outline", "0");
            }
        }
        let filter = pt.filters.iter().find(|p| p.field == i);
        let multi = filter.is_some_and(|p| p.selected.len() > 1);
        if multi {
            a(&mut out, "multipleItemSelectionAllowed", "1");
        }
        a(&mut out, "showAll", "0");
        if opts.blank_row {
            a(&mut out, "insertBlankRow", "1");
        }
        if opts.sort != PivotSort::Manual {
            a(&mut out, "sortType", opts.sort.as_str());
        }
        if !opts.subtotal {
            a(&mut out, "defaultSubtotal", "0");
        }
        if !opts.subtotal_top {
            a(&mut out, "subtotalTop", "0");
        }
        let values = axis_values(f);
        let has_items = axis.is_some() && !values.is_empty();
        let has_sort = opts.sort_by.is_some();
        if !has_items && !has_sort && !opts.repeat_labels {
            out.push_str("/>");
            continue;
        }
        out.push('>');
        if has_items {
            let shown: Vec<PivotItemKey> = filter
                .filter(|_| multi)
                .map(|p| p.selected.iter().map(PivotItemKey::of).collect())
                .unwrap_or_default();
            let hidden: Vec<PivotItemKey> = if filter.is_some() {
                Vec::new()
            } else {
                opts.hidden.iter().map(PivotItemKey::of).collect()
            };
            let subtotal = opts.subtotal && axis != Some("axisPage");
            let n = values.len() + usize::from(subtotal);
            // §18.10.1.45 items / §18.10.1.43 item
            let _ = write!(out, "<items count=\"{n}\">");
            for (x, v) in values.iter().enumerate() {
                let key = PivotItemKey::of(v);
                // §18.10.1.45 item/@f: a calculated item.
                let calc = cache.calculated_items.iter().any(|c| {
                    c.field == i && PivotItemKey::of(&CellValue::Text(c.name.clone())) == key
                });
                let f_attr = if calc { " f=\"1\"" } else { "" };
                if (!shown.is_empty() && !shown.contains(&key)) || hidden.contains(&key) {
                    let _ = write!(out, "<item h=\"1\"{f_attr} x=\"{x}\"/>");
                } else {
                    let _ = write!(out, "<item{f_attr} x=\"{x}\"/>");
                }
            }
            if subtotal {
                out.push_str("<item t=\"default\"/>");
            }
            out.push_str("</items>");
        }
        if let Some(v) = opts.sort_by {
            // §18.10.1.6 autoSortScope → pivotArea (§18.3.1.68) on the data field.
            let _ = write!(
                out,
                "<autoSortScope><pivotArea dataOnly=\"0\" outline=\"0\" fieldPosition=\"0\"><references count=\"1\"><reference field=\"4294967294\" count=\"1\" selected=\"0\"><x v=\"{v}\"/></reference></references></pivotArea></autoSortScope>"
            );
        }
        if opts.repeat_labels {
            let _ = write!(
                out,
                "<extLst><ext uri=\"{EXT_PIVOT_FIELD}\" xmlns:x14=\"{X14_NS}\"><x14:pivotField fillDownLabels=\"1\"/></ext></extLst>"
            );
        }
        out.push_str("</pivotField>");
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
    axis_items_xml(&mut out, "rowItems", &pt.row_items);
    axis_fields(
        &mut out,
        "colFields",
        &pt.cols,
        data_axis && !pt.data_on_rows,
    );
    axis_items_xml(&mut out, "colItems", &pt.col_items);
    if !pt.filters.is_empty() {
        let _ = write!(out, "<pageFields count=\"{}\">", pt.filters.len());
        for p in &pt.filters {
            out.push_str("<pageField");
            a(&mut out, "fld", &p.field.to_string());
            if let [one] = p.selected.as_slice() {
                let key = PivotItemKey::of(one);
                if let Some(x) = cache.fields.get(p.field as usize).and_then(|f| {
                    axis_values(f)
                        .iter()
                        .position(|v| PivotItemKey::of(v) == key)
                }) {
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
            // §18.10.1.21 dataField
            out.push_str("<dataField");
            if let Some(n) = &d.name {
                a(&mut out, "name", n);
            }
            a(&mut out, "fld", &d.field.to_string());
            if d.function != bo_model::sheet::PivotFunction::Sum {
                a(&mut out, "subtotal", d.function.as_str());
            }
            if d.show_as != PivotShowAs::Normal && !d.show_as.is_x14() {
                a(&mut out, "showDataAs", d.show_as.as_str());
            }
            a(&mut out, "baseField", &d.base_field.to_string());
            a(&mut out, "baseItem", &d.base_item.to_string());
            if let Some(n) = d.num_fmt_id {
                a(&mut out, "numFmtId", &n.to_string());
            }
            if d.show_as.is_x14() {
                let _ = write!(
                    out,
                    "><extLst><ext uri=\"{EXT_DATA_FIELD}\" xmlns:x14=\"{X14_NS}\"><x14:dataField pivotShowAs=\"{}\"/></ext></extLst></dataField>",
                    d.show_as.as_str()
                );
            } else {
                out.push_str("/>");
            }
        }
        out.push_str("</dataFields>");
    }
    if let Some(s) = &pt.style {
        // §18.10.1 pivotTableStyleInfo
        out.push_str("<pivotTableStyleInfo");
        a(&mut out, "name", &s.name);
        let b = |v: bool| if v { "1" } else { "0" };
        a(&mut out, "showRowHeaders", b(s.show_row_headers));
        a(&mut out, "showColHeaders", b(s.show_col_headers));
        a(&mut out, "showRowStripes", b(s.show_row_stripes));
        a(&mut out, "showColStripes", b(s.show_col_stripes));
        a(&mut out, "showLastColumn", b(s.show_last_column));
        out.push_str("/>");
    }
    if !pt.field_filters.is_empty() {
        let _ = write!(out, "<filters count=\"{}\">", pt.field_filters.len());
        for (i, f) in pt.field_filters.iter().enumerate() {
            filter_xml(&mut out, f, i + 1);
        }
        out.push_str("</filters>");
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
