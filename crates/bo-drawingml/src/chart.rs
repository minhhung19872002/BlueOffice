//! DrawingML charts (ECMA-376 Part 1, §21.2): a read-only view of chart
//! parts used for rendering. The part itself is preserved verbatim by the
//! package readers; only the cached values (`c:strCache` / `c:numCache`) are
//! read, so no spreadsheet evaluation is needed.

use crate::dom::{self, attr_int};
use crate::props::{is_fill, parse_fill};
use bo_model::slide::{Fill, XmlElement};

/// `graphicData@uri` of charts (§21.2.2.27 chart reference).
pub const URI_CHART: &str = "http://schemas.openxmlformats.org/drawingml/2006/chart";

/// Supported plot kinds.
#[derive(Debug, Clone, PartialEq)]
pub enum ChartKind {
    /// §21.2.2.16 barChart (also bar3DChart): `horizontal` = barDir `bar`.
    Bar {
        horizontal: bool,
        /// §21.2.2.76 grouping: `clustered` | `stacked` | `percentStacked`.
        grouping: String,
        /// §21.2.2.75 gapWidth, percent of a bar width.
        gap_width: f64,
    },
    /// §21.2.2.97 lineChart (also line3DChart).
    Line,
    /// §21.2.2.141 pieChart / pie3DChart / §21.2.2.50 doughnutChart.
    Pie { doughnut: bool },
    /// §21.2.2.5 areaChart.
    Area,
    /// Anything else, by element name (drawn as an empty plot frame).
    Unsupported(String),
}

/// One data series (§21.2.2.168 ser).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Series {
    pub name: String,
    pub categories: Vec<String>,
    pub values: Vec<Option<f64>>,
    /// Series fill (`c:spPr`).
    pub fill: Option<Fill>,
    /// Line colour for line charts (`c:spPr/a:ln`).
    pub line_fill: Option<Fill>,
    /// Data point overrides (§21.2.2.52 dPt): (index, fill).
    pub point_fills: Vec<(usize, Fill)>,
}

/// A parsed chart.
#[derive(Debug, Clone, PartialEq)]
pub struct Chart {
    pub title: Option<String>,
    pub kind: ChartKind,
    pub series: Vec<Series>,
    pub legend: bool,
    /// §21.2.2.47 c:varyColors (pie slices coloured per point).
    pub vary_colors: bool,
}

fn child_local<'a>(el: &'a XmlElement, name: &str) -> Option<&'a XmlElement> {
    el.elements().find(|c| c.local() == name)
}

fn all_text(el: &XmlElement, out: &mut String) {
    for c in el.elements() {
        if c.local() == "t" {
            out.push_str(&c.text());
        } else {
            all_text(c, out);
        }
    }
}

/// Points of a §21.2.2.172 strCache / §21.2.2.120 numCache, by `idx`.
fn cache_points(el: &XmlElement) -> Vec<(usize, String)> {
    let cache = el.elements().find_map(|r| match r.local() {
        "strRef" | "numRef" | "multiLvlStrRef" => r
            .elements()
            .find(|c| matches!(c.local(), "strCache" | "numCache" | "multiLvlStrCache")),
        "strLit" | "numLit" => Some(r),
        _ => None,
    });
    let Some(cache) = cache else {
        return Vec::new();
    };
    // multiLvlStrCache: use the first level.
    let cache = child_local(cache, "lvl").unwrap_or(cache);
    cache
        .elements()
        .filter(|p| p.local() == "pt")
        .filter_map(|p| {
            let idx = attr_int::<usize>(p, "idx")?;
            let v = child_local(p, "v").map(XmlElement::text)?;
            Some((idx, v))
        })
        .collect()
}

fn sp_fill(sp: &XmlElement) -> Option<Fill> {
    sp.elements()
        .find(|c| is_fill(&c.name))
        .and_then(parse_fill)
}

fn parse_series(ser: &XmlElement) -> Series {
    let mut s = Series::default();
    if let Some(tx) = child_local(ser, "tx") {
        if let Some((_, v)) = cache_points(tx).into_iter().next() {
            s.name = v;
        } else if let Some(v) = child_local(tx, "v") {
            s.name = v.text();
        }
    }
    let vals = child_local(ser, "val").or_else(|| child_local(ser, "yVal"));
    let cats = child_local(ser, "cat").or_else(|| child_local(ser, "xVal"));
    let vpts = vals.map(cache_points).unwrap_or_default();
    let cpts = cats.map(cache_points).unwrap_or_default();
    let n = vpts
        .iter()
        .map(|(i, _)| i + 1)
        .chain(cpts.iter().map(|(i, _)| i + 1))
        .max()
        .unwrap_or(0)
        .min(10_000);
    s.values = vec![None; n];
    for (i, v) in vpts {
        if i < n {
            s.values[i] = v.trim().parse().ok();
        }
    }
    s.categories = vec![String::new(); n];
    for (i, v) in cpts {
        if i < n {
            s.categories[i] = v;
        }
    }
    if let Some(sp) = child_local(ser, "spPr") {
        s.fill = sp_fill(sp);
        s.line_fill = child_local(sp, "ln").and_then(sp_fill);
    }
    for dpt in ser.elements().filter(|c| c.local() == "dPt") {
        let idx = child_local(dpt, "idx").and_then(|i| attr_int::<usize>(i, "val"));
        let fill = child_local(dpt, "spPr").and_then(sp_fill);
        if let (Some(i), Some(f)) = (idx, fill) {
            s.point_fills.push((i, f));
        }
    }
    s
}

fn val_of<'a>(el: &'a XmlElement, name: &str) -> Option<&'a str> {
    child_local(el, name).and_then(|c| c.attr("val"))
}

/// Parse a chart part (§21.2.2.29 chartSpace).
pub fn parse_chart(data: &[u8], part: &str) -> dom::DomResult<Chart> {
    let root = dom::parse(data, part)?;
    let chart = child_local(&root, "chart");
    let mut out = Chart {
        title: None,
        kind: ChartKind::Unsupported(String::new()),
        series: Vec::new(),
        legend: false,
        vary_colors: false,
    };
    let Some(chart) = chart else {
        return Ok(out);
    };
    // §21.2.2.210 title (an explicit title only; auto titles are not synthesised).
    if let Some(t) = child_local(chart, "title") {
        let mut s = String::new();
        all_text(t, &mut s);
        if !s.is_empty() {
            out.title = Some(s);
        }
    }
    out.legend = child_local(chart, "legend").is_some();
    let Some(plot) = child_local(chart, "plotArea") else {
        return Ok(out);
    };
    let Some(group) = plot.elements().find(|c| c.local().ends_with("Chart")) else {
        return Ok(out);
    };
    out.vary_colors = val_of(group, "varyColors").and_then(dom::parse_bool) == Some(true);
    out.kind = match group.local() {
        "barChart" | "bar3DChart" => ChartKind::Bar {
            horizontal: val_of(group, "barDir") == Some("bar"),
            grouping: val_of(group, "grouping").unwrap_or("clustered").to_owned(),
            gap_width: val_of(group, "gapWidth")
                .and_then(|v| v.trim_end_matches('%').parse().ok())
                .unwrap_or(150.0),
        },
        "lineChart" | "line3DChart" => ChartKind::Line,
        "pieChart" | "pie3DChart" | "ofPieChart" => ChartKind::Pie { doughnut: false },
        "doughnutChart" => ChartKind::Pie { doughnut: true },
        "areaChart" | "area3DChart" => ChartKind::Area,
        other => ChartKind::Unsupported(other.to_owned()),
    };
    out.series = group
        .elements()
        .filter(|c| c.local() == "ser")
        .map(parse_series)
        .collect();
    Ok(out)
}

/// The relationship id of the chart part referenced from `graphicData` (§21.2.2.27).
pub fn chart_rel_id(graphic_data: &XmlElement) -> Option<&str> {
    graphic_data
        .elements()
        .find(|c| c.local() == "chart")
        .and_then(|c| c.attr("r:id"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bar_chart_cache() {
        let xml = r#"<c:chartSpace xmlns:c="c" xmlns:a="a"><c:chart><c:title><c:tx><c:rich><a:p><a:r><a:t>Doanh thu</a:t></a:r></a:p></c:rich></c:tx></c:title><c:plotArea><c:barChart><c:barDir val="col"/><c:grouping val="clustered"/><c:varyColors val="0"/>
<c:ser><c:idx val="0"/><c:tx><c:strRef><c:f>S!B1</c:f><c:strCache><c:ptCount val="1"/><c:pt idx="0"><c:v>2025</c:v></c:pt></c:strCache></c:strRef></c:tx>
<c:spPr><a:solidFill><a:srgbClr val="FF0000"/></a:solidFill></c:spPr>
<c:cat><c:strRef><c:strCache><c:pt idx="0"><c:v>Q1</c:v></c:pt><c:pt idx="1"><c:v>Q2</c:v></c:pt></c:strCache></c:strRef></c:cat>
<c:val><c:numRef><c:numCache><c:pt idx="0"><c:v>1.5</c:v></c:pt><c:pt idx="1"><c:v>3</c:v></c:pt></c:numCache></c:numRef></c:val></c:ser>
<c:gapWidth val="100"/></c:barChart></c:plotArea><c:legend/></c:chart></c:chartSpace>"#;
        let c = parse_chart(xml.as_bytes(), "/c").unwrap();
        assert_eq!(c.title.as_deref(), Some("Doanh thu"));
        assert!(c.legend);
        assert_eq!(
            c.kind,
            ChartKind::Bar {
                horizontal: false,
                grouping: "clustered".into(),
                gap_width: 100.0
            }
        );
        let s = &c.series[0];
        assert_eq!(s.name, "2025");
        assert_eq!(s.categories, ["Q1", "Q2"]);
        assert_eq!(s.values, [Some(1.5), Some(3.0)]);
        assert!(matches!(s.fill, Some(Fill::Solid(_))));
    }
}
