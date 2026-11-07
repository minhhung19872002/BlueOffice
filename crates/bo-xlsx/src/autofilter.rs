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
use bo_model::sheet::{AutoFilter, CellRange, ColumnFilter, CustomFilter, FilterColumn, RawXml};
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
            let mut ok = true;
            if !empty {
                for_children(&mut r, |_, c, _| {
                    let mut ca = attrs(c);
                    match (lname(c).as_str(), take(&mut ca, "val")) {
                        ("filter", Some(v)) if ca.is_empty() => values.push(v),
                        _ => ok = false,
                    }
                    Ok(())
                })
                .ok()?;
            }
            ok.then_some(ColumnFilter::Values { values, blank })
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
        assert!(
            matches!(af.columns[3].filter, ColumnFilter::Raw(ref s) if s.contains("dynamicFilter"))
        );
        assert_eq!(af.unknown.len(), 1);
        assert_eq!(out, xml);
    }

    #[test]
    fn date_group_items_stay_raw() {
        let xml = r#"<autoFilter ref="A1:A5"><filterColumn colId="0"><filters><dateGroupItem year="2024" dateTimeGrouping="year"/></filters></filterColumn></autoFilter>"#;
        let (af, out) = rt(xml);
        assert!(matches!(af.columns[0].filter, ColumnFilter::Raw(_)));
        assert_eq!(out, xml);
        let (_, out) = rt(r#"<autoFilter ref="A1:B2"/>"#);
        assert_eq!(out, r#"<autoFilter ref="A1:B2"/>"#);
    }
}
