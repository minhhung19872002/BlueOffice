//! Sparklines ([MS-XLSX] 2.6.x `CT_SparklineGroups` / `CT_SparklineGroup` / `CT_Sparklines` /
//! `CT_Sparkline`), stored in a worksheet `extLst` (ECMA-376 Part 1, §18.2.10 `extLst`) under
//! the ext with URI [`SPARKLINE_EXT_URI`].
//!
//! Reading moves that ext out of the raw `extLst` fragment into
//! [`Worksheet::sparkline_groups`](bo_model::sheet::Worksheet); the other exts stay verbatim.
//! Writing appends a regenerated ext (unknown group attributes such as `xr2:uid` kept).

use bo_model::sheet::{
    CellRef, RawXml, SheetColor, Sparkline, SparklineAxis, SparklineColors, SparklineEmpty,
    SparklineGroup, SparklineType,
};
use bo_opc::xml::{escape, XmlReader};

use crate::chart::{parse_dom, Node};
use crate::styles::write_color;
use crate::xml::{a, a_optf, a_raw, a_true, capture, for_children, lname, pbool, root};

/// [MS-XLSX] 2.2.4.x: URI of the worksheet ext holding `x14:sparklineGroups`.
pub const SPARKLINE_EXT_URI: &str = "{05C60535-1F16-4fd2-B633-F4F36F0B64E0}";
/// SpreadsheetML 2009 extensions namespace (`x14`).
const NS_X14: &str = "http://schemas.microsoft.com/office/spreadsheetml/2009/9/main";
/// Excel 2006 main namespace (`xm`: `xm:f`, `xm:sqref`).
const NS_XM: &str = "http://schemas.microsoft.com/office/excel/2006/main";

/// Attributes of `sparklineGroup` the model owns (by local name).
const KNOWN_ATTRS: &[&str] = &[
    "manualMax",
    "manualMin",
    "lineWeight",
    "type",
    "dateAxis",
    "displayEmptyCellsAs",
    "markers",
    "high",
    "low",
    "first",
    "last",
    "negative",
    "displayXAxis",
    "displayHidden",
    "minAxisType",
    "maxAxisType",
    "rightToLeft",
];

/// Colour children of `sparklineGroup` in schema order.
const COLORS: &[&str] = &[
    "colorSeries",
    "colorNegative",
    "colorAxis",
    "colorMarkers",
    "colorFirst",
    "colorLast",
    "colorHigh",
    "colorLow",
];

fn local(k: &str) -> &str {
    k.rsplit_once(':').map_or(k, |(_, l)| l)
}

/// Moves the sparkline groups out of the worksheet's raw `extLst` fragment.
/// Malformed fragments are left untouched.
pub(crate) fn extract(unknown: &mut Vec<RawXml>) -> Vec<SparklineGroup> {
    let mut groups = Vec::new();
    let mut drop = None;
    for (i, u) in unknown.iter_mut().enumerate() {
        if !u.xml.contains(SPARKLINE_EXT_URI) && !u.xml.to_ascii_uppercase().contains("05C60535") {
            continue;
        }
        let Some((rest, found)) = split_ext_lst(&u.xml) else {
            continue;
        };
        if found.is_empty() {
            continue;
        }
        for ext in &found {
            if let Some(dom) = parse_dom(ext.as_bytes(), "extLst") {
                read_ext(&dom, &mut groups);
            }
        }
        match rest {
            Some(xml) => u.xml = xml,
            None => drop = Some(i),
        }
        break;
    }
    if let Some(i) = drop {
        unknown.remove(i);
    }
    groups
}

/// Splits an `extLst` fragment into (fragment without the sparkline exts — `None` when nothing
/// is left —, the sparkline exts).
fn split_ext_lst(xml: &str) -> Option<(Option<String>, Vec<String>)> {
    let mut r = XmlReader::new(xml.as_bytes(), "extLst");
    let (e, empty) = root(&mut r).ok()?;
    if lname(&e) != "extLst" || empty {
        return None;
    }
    let start: &str = &e;
    let qname: String = {
        let n = e.name();
        let s: &str = n.as_ref();
        s.to_owned()
    };
    let mut keep = Vec::new();
    let mut found = Vec::new();
    for_children(&mut r, |r, c, cempty| {
        let is_spark = lname(c) == "ext"
            && bo_opc::xml::attr(c, b"uri")
                .is_some_and(|u| u.eq_ignore_ascii_case(SPARKLINE_EXT_URI));
        let raw = capture(r, c, cempty)?;
        if is_spark {
            found.push(raw);
        } else {
            keep.push(raw);
        }
        Ok(())
    })
    .ok()?;
    let rest = (!keep.is_empty()).then(|| format!("<{start}>{}</{qname}>", keep.concat()));
    Some((rest, found))
}

fn color(n: &Node) -> SheetColor {
    let num = |k: &str| n.attr(k).and_then(|v| v.trim().parse::<u32>().ok());
    SheetColor {
        rgb: n.attr("rgb").map(str::to_owned),
        theme: num("theme"),
        indexed: num("indexed"),
        tint: n
            .attr("tint")
            .and_then(|v| v.trim().parse::<f64>().ok())
            .filter(|v| v.is_finite()),
        auto: n.attr("auto").is_some_and(pbool),
    }
}

fn axis(v: Option<&str>) -> SparklineAxis {
    match v {
        Some("group") => SparklineAxis::Group,
        Some("custom") => SparklineAxis::Custom,
        _ => SparklineAxis::Individual,
    }
}

fn read_ext(ext: &Node, out: &mut Vec<SparklineGroup>) {
    // ext → x14:sparklineGroups → x14:sparklineGroup*
    let Some(list) = ext.child("sparklineGroups") else {
        return;
    };
    for g in list.children.iter().filter(|c| c.name == "sparklineGroup") {
        out.push(read_group(g));
    }
}

fn read_group(g: &Node) -> SparklineGroup {
    let flag = |k: &str| g.attr(k).is_some_and(pbool);
    let num = |k: &str| {
        g.attr(k)
            .and_then(|v| v.trim().parse::<f64>().ok())
            .filter(|v| v.is_finite())
    };
    let mut colors = SparklineColors::default();
    let mut date_range = None;
    let mut sparklines = Vec::new();
    for c in &g.children {
        let col = || Some(color(c));
        match c.name.as_str() {
            "colorSeries" => colors.series = col(),
            "colorNegative" => colors.negative = col(),
            "colorAxis" => colors.axis = col(),
            "colorMarkers" => colors.markers = col(),
            "colorFirst" => colors.first = col(),
            "colorLast" => colors.last = col(),
            "colorHigh" => colors.high = col(),
            "colorLow" => colors.low = col(),
            // xm:f — the date range of a date axis.
            "f" => date_range = Some(c.text.trim().to_owned()).filter(|t| !t.is_empty()),
            "sparklines" => {
                for s in c.children.iter().filter(|s| s.name == "sparkline") {
                    let formula = s
                        .child("f")
                        .map(|f| f.text.trim().to_owned())
                        .filter(|t| !t.is_empty());
                    let Some(location) = s.child("sqref").and_then(|q| CellRef::parse(&q.text))
                    else {
                        continue;
                    };
                    sparklines.push(Sparkline { formula, location });
                }
            }
            _ => {}
        }
    }
    SparklineGroup {
        kind: match g.attr("type") {
            Some("column") => SparklineType::Column,
            Some("stacked") => SparklineType::Stacked,
            _ => SparklineType::Line,
        },
        sparklines,
        empty_as: match g.attr("displayEmptyCellsAs") {
            Some("gap") => SparklineEmpty::Gap,
            Some("span") => SparklineEmpty::Span,
            _ => SparklineEmpty::Zero,
        },
        markers: flag("markers"),
        high: flag("high"),
        low: flag("low"),
        first: flag("first"),
        last: flag("last"),
        negative: flag("negative"),
        display_x_axis: flag("displayXAxis"),
        display_hidden: flag("displayHidden"),
        right_to_left: flag("rightToLeft"),
        min_axis: axis(g.attr("minAxisType")),
        max_axis: axis(g.attr("maxAxisType")),
        manual_min: num("manualMin"),
        manual_max: num("manualMax"),
        line_weight: num("lineWeight"),
        date_axis: flag("dateAxis"),
        date_range,
        colors,
        attrs: g
            .attrs
            .iter()
            .filter(|(k, _)| !KNOWN_ATTRS.contains(&local(k)) && !k.starts_with("xmlns"))
            .cloned()
            .collect(),
    }
}

fn axis_name(a: SparklineAxis) -> Option<&'static str> {
    match a {
        SparklineAxis::Individual => None,
        SparklineAxis::Group => Some("group"),
        SparklineAxis::Custom => Some("custom"),
    }
}

/// The worksheet ext holding `groups` (empty string when there are none).
pub(crate) fn ext_xml(groups: &[SparklineGroup]) -> String {
    let groups: Vec<&SparklineGroup> = groups.iter().filter(|g| !g.sparklines.is_empty()).collect();
    if groups.is_empty() {
        return String::new();
    }
    let mut out = format!(
        r#"<ext uri="{SPARKLINE_EXT_URI}" xmlns:x14="{NS_X14}"><x14:sparklineGroups xmlns:xm="{NS_XM}">"#
    );
    for g in groups {
        write_group(&mut out, g);
    }
    out.push_str("</x14:sparklineGroups></ext>");
    out
}

fn write_group(out: &mut String, g: &SparklineGroup) {
    out.push_str("<x14:sparklineGroup");
    // Attribute order of CT_SparklineGroup.
    a_optf(out, "manualMax", g.manual_max);
    a_optf(out, "manualMin", g.manual_min);
    a_optf(out, "lineWeight", g.line_weight);
    match g.kind {
        SparklineType::Line => {}
        SparklineType::Column => a(out, "type", "column"),
        SparklineType::Stacked => a(out, "type", "stacked"),
    }
    a_true(out, "dateAxis", g.date_axis);
    a(
        out,
        "displayEmptyCellsAs",
        match g.empty_as {
            SparklineEmpty::Gap => "gap",
            SparklineEmpty::Zero => "zero",
            SparklineEmpty::Span => "span",
        },
    );
    a_true(out, "markers", g.markers);
    a_true(out, "high", g.high);
    a_true(out, "low", g.low);
    a_true(out, "first", g.first);
    a_true(out, "last", g.last);
    a_true(out, "negative", g.negative);
    a_true(out, "displayXAxis", g.display_x_axis);
    a_true(out, "displayHidden", g.display_hidden);
    if let Some(v) = axis_name(g.min_axis) {
        a(out, "minAxisType", v);
    }
    if let Some(v) = axis_name(g.max_axis) {
        a(out, "maxAxisType", v);
    }
    a_true(out, "rightToLeft", g.right_to_left);
    a_raw(out, &g.attrs);
    out.push('>');
    let c = &g.colors;
    for (name, col) in COLORS.iter().zip([
        &c.series,
        &c.negative,
        &c.axis,
        &c.markers,
        &c.first,
        &c.last,
        &c.high,
        &c.low,
    ]) {
        if let Some(col) = col {
            write_color(out, &format!("x14:{name}"), col);
        }
    }
    if let Some(f) = &g.date_range {
        out.push_str("<xm:f>");
        out.push_str(&escape(f));
        out.push_str("</xm:f>");
    }
    out.push_str("<x14:sparklines>");
    for s in &g.sparklines {
        out.push_str("<x14:sparkline>");
        if let Some(f) = &s.formula {
            out.push_str("<xm:f>");
            out.push_str(&escape(f));
            out.push_str("</xm:f>");
        }
        out.push_str("<xm:sqref>");
        out.push_str(&s.location.to_string());
        out.push_str("</xm:sqref></x14:sparkline>");
    }
    out.push_str("</x14:sparklines></x14:sparklineGroup>");
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXT: &str = r#"<ext uri="{05C60535-1F16-4fd2-B633-F4F36F0B64E0}" xmlns:x14="http://schemas.microsoft.com/office/spreadsheetml/2009/9/main"><x14:sparklineGroups xmlns:xm="http://schemas.microsoft.com/office/excel/2006/main"><x14:sparklineGroup manualMin="-2" lineWeight="1.5" type="column" displayEmptyCellsAs="gap" high="1" negative="1" minAxisType="custom" maxAxisType="group" xr2:uid="{AB}"><x14:colorSeries rgb="FF376092"/><x14:colorNegative theme="5" tint="-0.25"/><x14:sparklines><x14:sparkline><xm:f>Sheet1!A1:D1</xm:f><xm:sqref>E1</xm:sqref></x14:sparkline><x14:sparkline><xm:f>Sheet1!A2:D2</xm:f><xm:sqref>E2</xm:sqref></x14:sparkline></x14:sparklines></x14:sparklineGroup></x14:sparklineGroups></ext>"#;

    #[test]
    fn extracts_groups_and_keeps_other_exts() {
        let other = r#"<ext uri="{CCE6A557-97BC-4b89-ADB6-D9C93CAAB3DF}"><x14:dataValidations count="0"/></ext>"#;
        let mut unknown = vec![RawXml {
            after: "extLst".into(),
            xml: format!("<extLst>{other}{EXT}</extLst>"),
        }];
        let g = extract(&mut unknown);
        assert_eq!(g.len(), 1);
        assert_eq!(unknown[0].xml, format!("<extLst>{other}</extLst>"));
        let g = &g[0];
        assert_eq!(g.kind, SparklineType::Column);
        assert_eq!(g.empty_as, SparklineEmpty::Gap);
        assert!(g.high && g.negative && !g.low && !g.markers);
        assert_eq!(g.min_axis, SparklineAxis::Custom);
        assert_eq!(g.max_axis, SparklineAxis::Group);
        assert_eq!(g.manual_min, Some(-2.0));
        assert_eq!(g.line_weight, Some(1.5));
        assert_eq!(
            g.colors.series.as_ref().unwrap().rgb.as_deref(),
            Some("FF376092")
        );
        assert_eq!(g.colors.negative.as_ref().unwrap().theme, Some(5));
        assert_eq!(g.colors.negative.as_ref().unwrap().tint, Some(-0.25));
        assert_eq!(g.attrs, vec![("xr2:uid".to_owned(), "{AB}".to_owned())]);
        assert_eq!(g.sparklines.len(), 2);
        assert_eq!(g.sparklines[1].formula.as_deref(), Some("Sheet1!A2:D2"));
        assert_eq!(g.sparklines[1].location, CellRef::new(1, 4));
        // Written back: the same group.
        let xml = ext_xml(std::slice::from_ref(g));
        let mut again = vec![RawXml {
            after: "extLst".into(),
            xml: format!("<extLst>{xml}</extLst>"),
        }];
        let g2 = extract(&mut again);
        assert!(again.is_empty(), "nothing else left in extLst");
        assert_eq!(&g2[0], g);
    }

    #[test]
    fn no_groups_no_ext() {
        assert_eq!(ext_xml(&[]), "");
        assert_eq!(ext_xml(&[SparklineGroup::default()]), "");
    }
}
