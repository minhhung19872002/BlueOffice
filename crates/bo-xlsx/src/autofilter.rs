//! Worksheet AutoFilter (ECMA-376 Part 1, §18.3.1.2 `autoFilter`, §18.3.2 filter elements).
//!
//! Criteria the model represents (`filters` with plain values, `customFilters`,
//! `top10`) are parsed; anything else (`dynamicFilter`, `colorFilter`,
//! `iconFilter`, `dateGroupItem`, extra attributes) is kept as verbatim XML so the
//! element round-trips losslessly.

use crate::xml::{
    a, a_opt, a_raw, a_true, attrs, capture, emit_ordered, fmt_f64, for_children, lname,
    push_unknown, root, take, take_bool, take_f64, take_u32,
};
use crate::Result;
use bo_model::sheet::{
    AutoFilter, CellRange, ColumnFilter, CustomFilter, DateGroup, FilterColumn, RawXml,
};
use bo_opc::xml::XmlReader;
use quick_xml::events::BytesStart;

/// Child sequence of `autoFilter` (§18.3.1.2 CT_AutoFilter).
const AUTOFILTER_SCHEMA: &[&str] = &["filterColumn", "sortState", "extLst"];

/// Read an `autoFilter` element whose start tag `e` was just consumed.
pub(crate) fn read_auto_filter(
    r: &mut XmlReader<'_>,
    e: &BytesStart<'_>,
    empty: bool,
) -> Result<AutoFilter> {
    let mut at = attrs(e);
    let mut af = AutoFilter {
        range: take(&mut at, "ref")
            .and_then(|s| CellRange::parse(&s))
            .unwrap_or_default(),
        attrs: at,
        ..AutoFilter::default()
    };
    if empty {
        return Ok(af);
    }
    let mut prev = String::new();
    let mut unknown: Vec<RawXml> = Vec::new();
    for_children(r, |r, e, empty| {
        let name = lname(e);
        let xml = capture(r, e, empty)?;
        if name == "filterColumn" {
            if let Some(fc) = parse_filter_column(&xml) {
                af.columns.push(fc);
                prev = name;
                return Ok(());
            }
            // Malformed column (no colId): kept verbatim.
        }
        push_unknown(&mut unknown, &mut prev, AUTOFILTER_SCHEMA, &name, xml);
        Ok(())
    })?;
    af.unknown = unknown;
    Ok(af)
}

/// §18.3.2.7 filterColumn, from its captured XML. `None` for a malformed column
/// (no `colId`, unknown attributes).
fn parse_filter_column(xml: &str) -> Option<FilterColumn> {
    let mut r = XmlReader::new(xml.as_bytes(), "filterColumn");
    let (e, empty) = root(&mut r).ok()?;
    let mut at = attrs(&e);
    let col_id = take_u32(&mut at, "colId")?;
    let mut fc = FilterColumn {
        col_id,
        hidden_button: take_bool(&mut at, "hiddenButton").unwrap_or(false),
        show_button: take_bool(&mut at, "showButton"),
        filter: ColumnFilter::None,
    };
    if !at.is_empty() {
        return None;
    }
    if !empty {
        let mut raws: Vec<String> = Vec::new();
        for_children(&mut r, |r, e, empty| {
            raws.push(capture(r, e, empty)?);
            Ok(())
        })
        .ok()?;
        fc.filter = match raws.as_slice() {
            [] => ColumnFilter::None,
            [one] => parse_criterion(one).unwrap_or_else(|| ColumnFilter::Raw(one.clone())),
            many => ColumnFilter::Raw(many.concat()),
        };
    }
    Some(fc)
}

/// Parse a captured criterion element; `None` when the model cannot represent it exactly.
fn parse_criterion(xml: &str) -> Option<ColumnFilter> {
    let mut r = XmlReader::new(xml.as_bytes(), "filterColumn");
    let (e, empty) = root(&mut r).ok()?;
    let mut at = attrs(&e);
    // Prefixed (extension) elements are kept raw.
    if e.name().as_ref().contains(':') {
        return None;
    }
    match lname(&e).as_str() {
        // §18.3.2.8 filters / §18.3.2.6 filter
        "filters" => {
            let blank = take_bool(&mut at, "blank").unwrap_or(false);
            if !at.is_empty() {
                return None;
            }
            let mut values = Vec::new();
            let mut groups = Vec::new();
            let mut ok = true;
            if !empty {
                for_children(&mut r, |_, c, _| {
                    let mut ca = attrs(c);
                    match lname(c).as_str() {
                        "filter" => match take(&mut ca, "val") {
                            Some(v) if ca.is_empty() => values.push(v),
                            _ => ok = false,
                        },
                        // §18.3.2.4 dateGroupItem
                        "dateGroupItem" => {
                            let g = DateGroup {
                                year: take_u32(&mut ca, "year").unwrap_or(0),
                                month: take_u32(&mut ca, "month"),
                                day: take_u32(&mut ca, "day"),
                                hour: take_u32(&mut ca, "hour"),
                                minute: take_u32(&mut ca, "minute"),
                                second: take_u32(&mut ca, "second"),
                                grouping: take(&mut ca, "dateTimeGrouping").unwrap_or_default(),
                            };
                            if ca.is_empty() && !g.grouping.is_empty() {
                                groups.push(g);
                            } else {
                                ok = false;
                            }
                        }
                        _ => ok = false,
                    }
                    Ok(())
                })
                .ok()?;
            }
            if !ok {
                return None;
            }
            Some(if groups.is_empty() {
                ColumnFilter::Values { values, blank }
            } else {
                ColumnFilter::DateGroups {
                    values,
                    blank,
                    groups,
                }
            })
        }
        // §18.3.2.5 dynamicFilter
        "dynamicFilter" if empty => {
            let kind = take(&mut at, "type")?;
            let val = take_f64(&mut at, "val");
            let max_val = take_f64(&mut at, "maxVal");
            at.is_empty()
                .then_some(ColumnFilter::Dynamic { kind, val, max_val })
        }
        // §18.3.2.1 colorFilter
        "colorFilter" if empty => {
            let dxf_id = take_u32(&mut at, "dxfId");
            let cell_color = take_bool(&mut at, "cellColor").unwrap_or(true);
            at.is_empty()
                .then_some(ColumnFilter::Color { dxf_id, cell_color })
        }
        // §18.3.2.3 customFilters / §18.3.2.2 customFilter
        "customFilters" => {
            let and = take_bool(&mut at, "and").unwrap_or(false);
            if !at.is_empty() {
                return None;
            }
            let mut conditions = Vec::new();
            let mut ok = true;
            if !empty {
                for_children(&mut r, |_, c, _| {
                    let mut ca = attrs(c);
                    let operator = take(&mut ca, "operator");
                    let val = take(&mut ca, "val").unwrap_or_default();
                    if lname(c) == "customFilter" && ca.is_empty() {
                        conditions.push(CustomFilter { operator, val });
                    } else {
                        ok = false;
                    }
                    Ok(())
                })
                .ok()?;
            }
            ok.then_some(ColumnFilter::Custom { and, conditions })
        }
        // §18.3.2.10 top10
        "top10" if empty => {
            let top = take_bool(&mut at, "top").unwrap_or(true);
            let percent = take_bool(&mut at, "percent").unwrap_or(false);
            let val = take_f64(&mut at, "val")?;
            let filter_val = take_f64(&mut at, "filterVal");
            at.is_empty().then_some(ColumnFilter::Top10 {
                top,
                percent,
                val,
                filter_val,
            })
        }
        _ => None,
    }
}

/// Write an `autoFilter` element.
pub(crate) fn write_auto_filter(out: &mut String, af: &AutoFilter) {
    out.push_str("<autoFilter");
    a(out, "ref", &af.range.to_string());
    a_raw(out, &af.attrs);
    if af.columns.is_empty() && af.unknown.is_empty() {
        out.push_str("/>");
        return;
    }
    out.push('>');
    emit_ordered(out, AUTOFILTER_SCHEMA, &af.unknown, |slot, out| {
        if slot == "filterColumn" {
            for fc in &af.columns {
                write_filter_column(out, fc);
            }
        }
    });
    out.push_str("</autoFilter>");
}

fn write_filter_column(out: &mut String, fc: &FilterColumn) {
    out.push_str("<filterColumn");
    a(out, "colId", &fc.col_id.to_string());
    a_true(out, "hiddenButton", fc.hidden_button);
    if let Some(b) = fc.show_button {
        a(out, "showButton", if b { "1" } else { "0" });
    }
    if fc.filter == ColumnFilter::None {
        out.push_str("/>");
        return;
    }
    out.push('>');
    match &fc.filter {
        ColumnFilter::None => {}
        ColumnFilter::Values { values, blank } => {
            out.push_str("<filters");
            a_true(out, "blank", *blank);
            if values.is_empty() {
                out.push_str("/>");
            } else {
                out.push('>');
                for v in values {
                    out.push_str("<filter");
                    a(out, "val", v);
                    out.push_str("/>");
                }
                out.push_str("</filters>");
            }
        }
        ColumnFilter::Custom { and, conditions } => {
            out.push_str("<customFilters");
            a_true(out, "and", *and);
            out.push('>');
            for c in conditions {
                out.push_str("<customFilter");
                a_opt(out, "operator", c.operator.as_ref());
                a(out, "val", &c.val);
                out.push_str("/>");
            }
            out.push_str("</customFilters>");
        }
        ColumnFilter::Top10 {
            top,
            percent,
            val,
            filter_val,
        } => {
            out.push_str("<top10");
            if !*top {
                a(out, "top", "0");
            }
            a_true(out, "percent", *percent);
            a(out, "val", &fmt_f64(*val));
            if let Some(fv) = filter_val {
                a(out, "filterVal", &fmt_f64(*fv));
            }
            out.push_str("/>");
        }
        ColumnFilter::DateGroups {
            values,
            blank,
            groups,
        } => {
            out.push_str("<filters");
            a_true(out, "blank", *blank);
            out.push('>');
            for v in values {
                out.push_str("<filter");
                a(out, "val", v);
                out.push_str("/>");
            }
            for g in groups {
                out.push_str("<dateGroupItem");
                a(out, "year", &g.year.to_string());
                a_opt(out, "month", g.month.as_ref());
                a_opt(out, "day", g.day.as_ref());
                a_opt(out, "hour", g.hour.as_ref());
                a_opt(out, "minute", g.minute.as_ref());
                a_opt(out, "second", g.second.as_ref());
                a(out, "dateTimeGrouping", &g.grouping);
                out.push_str("/>");
            }
            out.push_str("</filters>");
        }
        ColumnFilter::Dynamic { kind, val, max_val } => {
            out.push_str("<dynamicFilter");
            a(out, "type", kind);
            if let Some(v) = val {
                a(out, "val", &fmt_f64(*v));
            }
            if let Some(v) = max_val {
                a(out, "maxVal", &fmt_f64(*v));
            }
            out.push_str("/>");
        }
        ColumnFilter::Color { dxf_id, cell_color } => {
            out.push_str("<colorFilter");
            a_opt(out, "dxfId", dxf_id.as_ref());
            if !*cell_color {
                a(out, "cellColor", "0");
            }
            out.push_str("/>");
        }
        ColumnFilter::Raw(xml) => out.push_str(xml),
    }
    out.push_str("</filterColumn>");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rt(xml: &str) -> (AutoFilter, String) {
        let mut r = XmlReader::new(xml.as_bytes(), "t");
        let (e, empty) = root(&mut r).unwrap();
        let af = read_auto_filter(&mut r, &e, empty).unwrap();
        let mut out = String::new();
        write_auto_filter(&mut out, &af);
        (af, out)
    }

    #[test]
    fn values_custom_top10_and_raw() {
        let xml = r#"<autoFilter ref="A1:D10"><filterColumn colId="0"><filters blank="1"><filter val="Hà Nội"/><filter val="5"/></filters></filterColumn><filterColumn colId="1"><customFilters and="1"><customFilter operator="greaterThan" val="10"/><customFilter operator="lessThanOrEqual" val="20"/></customFilters></filterColumn><filterColumn colId="2"><top10 percent="1" val="10" filterVal="45"/></filterColumn><filterColumn colId="3"><dynamicFilter type="aboveAverage" val="3.5"/></filterColumn><sortState ref="A2:D10"><sortCondition ref="A2:A10"/></sortState></autoFilter>"#;
        let (af, out) = rt(xml);
        assert_eq!(af.range, CellRange::parse("A1:D10").unwrap());
        assert_eq!(
            af.columns[0].filter,
            ColumnFilter::Values {
                values: vec!["Hà Nội".into(), "5".into()],
                blank: true
            }
        );
        assert!(
            matches!(af.columns[1].filter, ColumnFilter::Custom { and: true, ref conditions } if conditions.len() == 2)
        );
        assert_eq!(
            af.columns[2].filter,
            ColumnFilter::Top10 {
                top: true,
                percent: true,
                val: 10.0,
                filter_val: Some(45.0)
            }
        );
        assert_eq!(
            af.columns[3].filter,
            ColumnFilter::Dynamic {
                kind: "aboveAverage".into(),
                val: Some(3.5),
                max_val: None
            }
        );
        assert_eq!(af.unknown.len(), 1);
        assert_eq!(out, xml);
    }

    #[test]
    fn date_groups_dynamic_and_colour_filters() {
        let xml = r#"<autoFilter ref="A1:C5"><filterColumn colId="0"><filters blank="1"><filter val="x"/><dateGroupItem year="2024" month="3" dateTimeGrouping="month"/><dateGroupItem year="2025" dateTimeGrouping="year"/></filters></filterColumn><filterColumn colId="1"><colorFilter dxfId="2"/></filterColumn><filterColumn colId="2"><dynamicFilter type="thisMonth"/></filterColumn></autoFilter>"#;
        let (af, out) = rt(xml);
        match &af.columns[0].filter {
            ColumnFilter::DateGroups {
                values,
                blank,
                groups,
            } => {
                assert_eq!((values.len(), *blank, groups.len()), (1, true, 2));
                assert_eq!((groups[0].year, groups[0].month), (2024, Some(3)));
                assert_eq!(groups[1].grouping, "year");
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(
            af.columns[1].filter,
            ColumnFilter::Color {
                dxf_id: Some(2),
                cell_color: true
            }
        );
        assert!(
            matches!(&af.columns[2].filter, ColumnFilter::Dynamic { kind, .. } if kind == "thisMonth")
        );
        assert_eq!(out, xml);
        // ISO values are not modelled: kept raw.
        let raw = r#"<autoFilter ref="A1:A5"><filterColumn colId="0"><dynamicFilter type="aboveAverage" valIso="2024-01-01T00:00:00"/></filterColumn></autoFilter>"#;
        let (af, out) = rt(raw);
        assert!(matches!(af.columns[0].filter, ColumnFilter::Raw(_)));
        assert_eq!(out, raw);
        let (_, out) = rt(r#"<autoFilter ref="A1:B2"/>"#);
        assert_eq!(out, r#"<autoFilter ref="A1:B2"/>"#);
    }
}
