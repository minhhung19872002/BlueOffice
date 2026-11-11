//! Extended charts ([MS-ODRAWXML] 2.24, namespace `http://schemas.microsoft.com/office/drawing/2014/chartex`):
//! waterfall, treemap, sunburst, histogram, pareto, box & whisker and funnel. A `cx:chartSpace` keeps its
//! data in `cx:chartData/cx:data` (string and numeric dimensions with cached points) and its series in
//! `cx:chart/cx:plotArea/cx:plotAreaRegion/cx:series@layoutId`.

use crate::chart::Node;
use crate::chart_format as cf;
use bo_model::chart_style::{ChartFormat, DataLabels};
use bo_model::sheet::{Chart, ChartKind, ChartPlot, ChartSeries};
use bo_opc::xml::escape;
use std::fmt::Write;

/// Chartex namespace.
pub const CX_NS: &str = "http://schemas.microsoft.com/office/drawing/2014/chartex";
/// Content type of a chartex part.
pub const CT_CHARTEX: &str = "application/vnd.ms-office.chartex+xml";
/// Relationship type from a drawing to a chartex part.
pub const REL_CHARTEX: &str = "http://schemas.microsoft.com/office/2014/relationships/chartEx";
const A_NS: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
const R_NS: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";

/// A data dimension of `cx:data`: its kind (`cat`, `val`, `size` …), formula and levels of points.
struct Dim {
    ty: String,
    f: Option<String>,
    /// Levels, each `ptCount` long (missing points `None`).
    levels: Vec<Vec<Option<String>>>,
}

fn dims(data: &Node) -> Vec<Dim> {
    data.children
        .iter()
        .filter(|d| d.name == "strDim" || d.name == "numDim")
        .map(|d| Dim {
            ty: d.attr("type").unwrap_or("val").to_owned(),
            f: d.child("f")
                .map(|f| f.text.trim().to_owned())
                .filter(|f| !f.is_empty()),
            levels: d
                .children
                .iter()
                .filter(|l| l.name == "lvl")
                .map(|l| {
                    let n = l
                        .attr("ptCount")
                        .and_then(|v| v.trim().parse::<usize>().ok())
                        .unwrap_or(0)
                        .min(100_000);
                    let mut v = vec![None; n];
                    for p in l.children.iter().filter(|p| p.name == "pt") {
                        if let Some(i) = p.attr("idx").and_then(|i| i.trim().parse::<usize>().ok())
                        {
                            if i < n {
                                v[i] = Some(p.text.clone());
                            }
                        }
                    }
                    v
                })
                .collect(),
        })
        .collect()
}

fn text_of(n: &Node) -> String {
    let mut out = n.text.clone();
    for c in &n.children {
        out.push_str(&text_of(c));
    }
    out
}

/// The chart of a `cx:chartSpace` (`dom` its root).
pub(crate) fn parse_chartex(dom: &Node, raw: &[u8], part: &str) -> Option<Chart> {
    let raw = std::str::from_utf8(raw).ok();
    let data: Vec<(String, Vec<Dim>)> = dom
        .child("chartData")?
        .children
        .iter()
        .filter(|d| d.name == "data")
        .map(|d| (d.attr("id").unwrap_or("0").to_owned(), dims(d)))
        .collect();
    let chart = dom.child("chart")?;
    let mut out = Chart {
        part: part.to_owned(),
        ..Chart::default()
    };
    out.title = chart
        .child("title")
        .and_then(|t| t.child("tx"))
        .map(text_of)
        .map(|t| t.trim().to_owned())
        .filter(|t| !t.is_empty());
    out.legend = chart
        .child("legend")
        .map(|l| l.attr("pos").unwrap_or("t").to_owned());
    let region = chart.child("plotArea")?.child("plotAreaRegion")?;
    let series: Vec<&Node> = region
        .children
        .iter()
        .filter(|s| s.name == "series")
        .collect();
    let pareto = series
        .iter()
        .any(|s| s.attr("layoutId") == Some("paretoLine"));
    let mut plot = ChartPlot::default();
    let mut labels = None;
    for s in series {
        let layout = s.attr("layoutId").unwrap_or("");
        let kind = match layout {
            "waterfall" => ChartKind::Waterfall,
            "treemap" => ChartKind::Treemap,
            "sunburst" => ChartKind::Sunburst,
            "funnel" => ChartKind::Funnel,
            "regionMap" => ChartKind::RegionMap,
            "boxWhisker" => ChartKind::BoxWhisker,
            "clusteredColumn" if pareto => ChartKind::Pareto,
            "clusteredColumn" => ChartKind::Histogram,
            // The pareto line is drawn from its owner series.
            "paretoLine" => continue,
            other => ChartKind::Other(other.to_owned()),
        };
        plot.kind = kind;
        let mut cs = ChartSeries {
            name: s
                .child("tx")
                .map(text_of)
                .map(|t| t.trim().to_owned())
                .filter(|t| !t.is_empty()),
            ..ChartSeries::default()
        };
        // spPr / dataPt: the series' and points' own colours; dataLabels: what the labels show.
        cs.fill = s.child("spPr").and_then(cf::fill_color);
        cs.point_fills = s
            .children
            .iter()
            .filter(|d| d.name == "dataPt")
            .filter_map(|d| {
                let k = d.attr("idx")?.trim().parse().ok()?;
                Some((k, d.child("spPr").and_then(cf::fill_color)?))
            })
            .collect();
        if plot.kind != ChartKind::RegionMap && labels.is_none() {
            labels = labels_of(s);
        }
        let id = s.child("dataId").and_then(|d| d.attr("val")).unwrap_or("0");
        let mut map =
            (plot.kind == ChartKind::RegionMap).then(|| crate::chartex_map::parse(s, raw));
        if let Some((_, ds)) = data.iter().find(|(i, _)| i == id) {
            for d in ds {
                match d.ty.as_str() {
                    // A map of categories: `cx:strDim type="colorStr"`.
                    "colorStr" => {
                        if let Some(m) = map.as_mut() {
                            m.categorical = true;
                            m.text_values = d
                                .levels
                                .first()
                                .map(|l| l.iter().map(|p| p.clone().unwrap_or_default()).collect())
                                .unwrap_or_default();
                            // No numbers: one missing value per point.
                            cs.values = vec![None; m.text_values.len()];
                        }
                        cs.val_ref.clone_from(&d.f);
                    }
                    "cat" => {
                        cs.cat_ref.clone_from(&d.f);
                        cs.levels = d
                            .levels
                            .iter()
                            .map(|l| l.iter().map(|p| p.clone().unwrap_or_default()).collect())
                            .collect();
                        cs.categories = cs.levels.first().cloned().unwrap_or_default();
                    }
                    "val" | "size" | "y" | "colorVal" => {
                        cs.val_ref.clone_from(&d.f);
                        cs.values = d
                            .levels
                            .first()
                            .map(|l| {
                                l.iter()
                                    .map(|p| {
                                        p.as_deref().and_then(|v| v.trim().parse::<f64>().ok())
                                    })
                                    .collect()
                            })
                            .unwrap_or_default();
                    }
                    _ => {}
                }
            }
        }
        if let Some(lp) = s.child("layoutPr") {
            if let Some(st) = lp.child("subtotals") {
                cs.subtotals = st
                    .children
                    .iter()
                    .filter(|i| i.name == "idx")
                    .filter_map(|i| i.attr("val").and_then(|v| v.trim().parse().ok()))
                    .collect();
            }
            // Histogram bins: `count:n` or `size:w` (automatic when absent).
            if let Some(b) = lp.child("binning") {
                // The bin count / size is the element's text (an attribute in some writers).
                let value = |n: &Node| {
                    let t = n.text.trim();
                    if t.is_empty() {
                        n.attr("val").map(|v| v.trim().to_owned())
                    } else {
                        Some(t.to_owned())
                    }
                };
                plot.style = b
                    .child("binCount")
                    .and_then(value)
                    .map(|v| format!("count:{v}"))
                    .or_else(|| {
                        b.child("binSize")
                            .and_then(value)
                            .map(|v| format!("size:{v}"))
                    });
            }
            if let Some(q) = lp
                .child("statistics")
                .and_then(|st| st.attr("quartileMethod"))
            {
                plot.style = Some(q.to_owned());
            }
        }
        plot.series.push(cs);
        if let Some(m) = map {
            out.map.get_or_insert(m);
        }
    }
    out.plots = vec![plot];
    // Chart space and plot area shape / text properties (the same elements as a c:chartSpace).
    out.format = cf::read_format(dom, chart);
    out.format.data_labels = labels;
    Some(out)
}

fn str_dim(out: &mut String, f: Option<&str>, levels: &[Vec<String>]) {
    str_dim_of(out, "cat", f, levels);
}

fn str_dim_of(out: &mut String, ty: &str, f: Option<&str>, levels: &[Vec<String>]) {
    let _ = write!(out, "<cx:strDim type=\"{ty}\">");
    if let Some(f) = f {
        let _ = write!(out, "<cx:f>{}</cx:f>", escape(f));
    }
    for l in levels {
        let _ = write!(out, "<cx:lvl ptCount=\"{}\">", l.len());
        for (i, v) in l.iter().enumerate() {
            let _ = write!(out, "<cx:pt idx=\"{i}\">{}</cx:pt>", escape(v));
        }
        out.push_str("</cx:lvl>");
    }
    out.push_str("</cx:strDim>");
}

fn num_dim(out: &mut String, ty: &str, f: Option<&str>, vals: &[Option<f64>]) {
    let _ = write!(out, "<cx:numDim type=\"{ty}\">");
    if let Some(f) = f {
        let _ = write!(out, "<cx:f>{}</cx:f>", escape(f));
    }
    let _ = write!(
        out,
        "<cx:lvl ptCount=\"{}\" formatCode=\"General\">",
        vals.len()
    );
    for (i, v) in vals.iter().enumerate() {
        if let Some(v) = v {
            let _ = write!(out, "<cx:pt idx=\"{i}\">{v}</cx:pt>");
        }
    }
    out.push_str("</cx:lvl></cx:numDim>");
}

fn text_data(out: &mut String, tag: &str, text: &str) {
    let _ = write!(
        out,
        "<cx:{tag}><cx:txData><cx:v>{}</cx:v></cx:txData></cx:{tag}>",
        escape(text)
    );
}

/// The `mc:Choice@Requires` prefix and namespace a chartex frame of `kind` needs ([MS-ODRAWXML]
/// namespaces: funnels came with the 2015/10/21 chartex namespace, region maps with the 2016/5/10
/// one, the other layouts with the 2015/9/8 one).
#[must_use]
pub fn requires(kind: Option<&ChartKind>) -> (&'static str, &'static str) {
    match kind {
        Some(ChartKind::Funnel) => (
            "cx2",
            "http://schemas.microsoft.com/office/drawing/2015/10/21/chartex",
        ),
        Some(ChartKind::RegionMap) => (
            "cx4",
            "http://schemas.microsoft.com/office/drawing/2016/5/10/chartex",
        ),
        _ => (
            "cx1",
            "http://schemas.microsoft.com/office/drawing/2015/9/8/chartex",
        ),
    }
}

/// The `cx:chartSpace` part of a chartex `chart` ([MS-ODRAWXML] 2.24; children in schema order).
#[must_use]
pub fn chartex_space_xml(chart: &Chart) -> String {
    chartex_space_xml_with(chart, None)
}

/// [`chartex_space_xml`] with a `cx:externalData` reference (relationship `external`) to an
/// embedded workbook holding the data (first child of `cx:chartData`).
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn chartex_space_xml_with(chart: &Chart, external: Option<&str>) -> String {
    let mut out = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n");
    let _ = write!(
        out,
        "<cx:chartSpace xmlns:a=\"{A_NS}\" xmlns:r=\"{R_NS}\" xmlns:cx=\"{CX_NS}\"><cx:chartData>"
    );
    if let Some(rid) = external {
        let _ = write!(out, "<cx:externalData r:id=\"{}\"/>", escape(rid));
    }
    let plot = chart.plots.first();
    let kind = plot.map_or(ChartKind::Waterfall, |p| p.kind.clone());
    let series: Vec<&ChartSeries> = plot.map(|p| p.series.iter().collect()).unwrap_or_default();
    for (i, s) in series.iter().enumerate() {
        let _ = write!(out, "<cx:data id=\"{i}\">");
        let hierarchical = matches!(kind, ChartKind::Treemap | ChartKind::Sunburst);
        // Histograms bin raw values: no categories.
        if kind != ChartKind::Histogram {
            let levels: Vec<Vec<String>> = if s.levels.is_empty() {
                vec![s.categories.clone()]
            } else {
                s.levels.clone()
            };
            str_dim(&mut out, s.cat_ref.as_deref(), &levels);
        }
        match (&kind, chart.map.as_ref()) {
            // [MS-ODRAWXML] chartex ST_StringDimensionType colorStr: a map of categories.
            (ChartKind::RegionMap, Some(m)) if m.categorical => {
                str_dim_of(
                    &mut out,
                    "colorStr",
                    s.val_ref.as_deref(),
                    std::slice::from_ref(&m.text_values),
                );
            }
            // ST_NumericDimensionType colorVal: the values of a map's colour scale.
            (ChartKind::RegionMap, _) => {
                num_dim(&mut out, "colorVal", s.val_ref.as_deref(), &s.values);
            }
            _ => num_dim(
                &mut out,
                if hierarchical { "size" } else { "val" },
                s.val_ref.as_deref(),
                &s.values,
            ),
        }
        out.push_str("</cx:data>");
    }
    if series.is_empty() {
        out.push_str("<cx:data id=\"0\"><cx:numDim type=\"val\"><cx:lvl ptCount=\"0\" formatCode=\"General\"/></cx:numDim></cx:data>");
    }
    out.push_str("</cx:chartData><cx:chart>");
    if let Some(t) = chart.title.as_deref().filter(|t| !t.is_empty()) {
        out.push_str("<cx:title pos=\"t\" align=\"ctr\" overlay=\"0\">");
        let _ = write!(
            out,
            "<cx:tx><cx:txData><cx:v>{}</cx:v></cx:txData></cx:tx>",
            escape(t)
        );
        out.push_str("</cx:title>");
    }
    out.push_str("<cx:plotArea><cx:plotAreaRegion>");
    let layout = match kind {
        ChartKind::Treemap => "treemap",
        ChartKind::Sunburst => "sunburst",
        ChartKind::Funnel => "funnel",
        ChartKind::BoxWhisker => "boxWhisker",
        ChartKind::Histogram | ChartKind::Pareto => "clusteredColumn",
        ChartKind::RegionMap => "regionMap",
        _ => "waterfall",
    };
    let map_opts = chart.map.clone().unwrap_or_default();
    for (i, s) in series.iter().enumerate() {
        let _ = write!(
            out,
            "<cx:series layoutId=\"{layout}\" uniqueId=\"{{B1E50FF3-0000-4000-8000-{i:012X}}}\">"
        );
        if let Some(n) = s.name.as_deref().filter(|n| !n.is_empty()) {
            text_data(&mut out, "tx", n);
        }
        // CT_Series: tx, spPr, valueColors, valueColorPositions, dataPt*, dataLabels, dataId.
        let fmt = &chart.format;
        let own = s.fill.clone().or_else(|| match fmt.palette.len() {
            0 => None,
            n => fmt.palette.get(i % n).cloned(),
        });
        if let Some(c) = own.filter(|_| kind != ChartKind::RegionMap) {
            out.push_str(&cf::cx_sp_pr(&cf::series_sp_pr(&c, fmt, false)));
        }
        if kind == ChartKind::RegionMap {
            if !map_opts.categorical {
                crate::chartex_map::write_scale(&mut out, &map_opts.scale);
            }
            crate::chartex_map::write_labels(&mut out, &map_opts);
        } else {
            // dataPt: the colour of one point.
            for (k, c) in &s.point_fills {
                let _ = write!(
                    out,
                    "<cx:dataPt idx=\"{k}\">{}</cx:dataPt>",
                    cf::cx_sp_pr(&cf::series_sp_pr(c, fmt, false))
                );
            }
            data_labels(&mut out, fmt, &kind);
        }
        let _ = write!(out, "<cx:dataId val=\"{i}\"/>");
        match kind {
            ChartKind::RegionMap => crate::chartex_map::write_layout(&mut out, &map_opts),
            ChartKind::Waterfall => {
                out.push_str("<cx:layoutPr><cx:visibility connectorLines=\"1\"/>");
                if !s.subtotals.is_empty() {
                    out.push_str("<cx:subtotals>");
                    for k in &s.subtotals {
                        let _ = write!(out, "<cx:idx val=\"{k}\"/>");
                    }
                    out.push_str("</cx:subtotals>");
                }
                out.push_str("</cx:layoutPr>");
            }
            ChartKind::Treemap => {
                out.push_str(
                    "<cx:layoutPr><cx:parentLabelLayout val=\"overlapping\"/></cx:layoutPr>",
                );
            }
            ChartKind::BoxWhisker => {
                let q = plot
                    .and_then(|p| p.style.as_deref())
                    .filter(|q| *q == "inclusive")
                    .unwrap_or("exclusive");
                let _ = write!(
                    out,
                    "<cx:layoutPr><cx:visibility meanLine=\"0\" meanMarker=\"1\" nonoutliers=\"0\" outliers=\"1\"/><cx:statistics quartileMethod=\"{q}\"/></cx:layoutPr>"
                );
            }
            ChartKind::Histogram => {
                out.push_str("<cx:layoutPr><cx:binning intervalClosed=\"r\">");
                match plot.and_then(|p| p.style.as_deref()) {
                    Some(st) if st.starts_with("count:") => {
                        let _ = write!(out, "<cx:binCount>{}</cx:binCount>", &st[6..]);
                    }
                    Some(st) if st.starts_with("size:") => {
                        let _ = write!(out, "<cx:binSize>{}</cx:binSize>", &st[5..]);
                    }
                    _ => {}
                }
                out.push_str("</cx:binning></cx:layoutPr>");
            }
            ChartKind::Pareto => out.push_str("<cx:layoutPr><cx:aggregation/></cx:layoutPr>"),
            _ => {}
        }
        if matches!(kind, ChartKind::Pareto) {
            out.push_str("<cx:axisId>0</cx:axisId><cx:axisId>1</cx:axisId>");
        }
        out.push_str("</cx:series>");
        if kind == ChartKind::Pareto {
            let _ = write!(
                out,
                "<cx:series layoutId=\"paretoLine\" ownerIdx=\"{i}\" uniqueId=\"{{B1E50FF4-0000-4000-8000-{i:012X}}}\"><cx:axisId>2</cx:axisId></cx:series>"
            );
        }
    }
    out.push_str("</cx:plotAreaRegion>");
    match kind {
        ChartKind::Treemap | ChartKind::Sunburst | ChartKind::RegionMap => {}
        ChartKind::Funnel => {
            out.push_str(
                "<cx:axis id=\"0\"><cx:catScaling gapWidth=\"0.06\"/><cx:tickLabels/></cx:axis>",
            );
        }
        ChartKind::Pareto => {
            out.push_str(
                "<cx:axis id=\"0\"><cx:catScaling gapWidth=\"0\"/><cx:tickLabels/></cx:axis>",
            );
            out.push_str(
                "<cx:axis id=\"1\"><cx:valScaling/><cx:majorGridlines/><cx:tickLabels/></cx:axis>",
            );
            out.push_str("<cx:axis id=\"2\"><cx:valScaling max=\"1\" min=\"0\"/><cx:units unit=\"percentage\"/><cx:tickLabels/></cx:axis>");
        }
        _ => {
            let gap = if kind == ChartKind::Histogram {
                "0"
            } else {
                "0.5"
            };
            let _ = write!(
                out,
                "<cx:axis id=\"0\"><cx:catScaling gapWidth=\"{gap}\"/><cx:tickLabels/></cx:axis><cx:axis id=\"1\"><cx:valScaling/><cx:majorGridlines/><cx:tickLabels/></cx:axis>"
            );
        }
    }
    // CT_PlotArea: plotAreaRegion, axis*, spPr.
    let plot_sp = cf::plot_sp_pr(&chart.format);
    if !plot_sp.is_empty() {
        out.push_str(&cf::cx_sp_pr(&plot_sp));
    }
    out.push_str("</cx:plotArea>");
    if let Some(pos) = &chart.legend {
        let pos = match pos.as_str() {
            "l" | "r" | "b" | "t" => pos.as_str(),
            _ => "t",
        };
        let _ = write!(
            out,
            "<cx:legend pos=\"{pos}\" align=\"ctr\" overlay=\"0\"/>"
        );
    }
    out.push_str("</cx:chart>");
    // CT_ChartSpace: chartData, chart, spPr, txPr.
    out.push_str(&cf::space_props_in(&chart.format, "cx"));
    out.push_str("</cx:chartSpace>");
    out
}

/// Chartex `ST_DataLabelPos` values.
const LABEL_POS: [&str; 9] = [
    "bestFit", "b", "ctr", "inBase", "inEnd", "l", "outEnd", "r", "t",
];

/// `cx:dataLabels` of a series (CT_DataLabels: `pos` attribute, `cx:visibility` with the
/// seriesName / categoryName / value switches) from the chart's data labels.
fn data_labels(out: &mut String, fmt: &ChartFormat, kind: &ChartKind) {
    let Some(d) = fmt.data_labels.as_ref().filter(|d| d.any()) else {
        return;
    };
    out.push_str("<cx:dataLabels");
    // Hierarchical layouts place their labels inside the shapes: no position.
    if !matches!(kind, ChartKind::Treemap | ChartKind::Sunburst) {
        if let Some(p) = d.pos.as_deref().filter(|p| LABEL_POS.contains(p)) {
            let _ = write!(out, " pos=\"{p}\"");
        }
    }
    let b = |v: bool| u8::from(v);
    let _ = write!(
        out,
        "><cx:visibility seriesName=\"{}\" categoryName=\"{}\" value=\"{}\"/></cx:dataLabels>",
        b(d.series),
        b(d.category),
        b(d.value || d.percent)
    );
}

/// The `cx:dataLabels` of a series as the chart's data labels (`None`: no labels shown).
fn labels_of(s: &Node) -> Option<DataLabels> {
    let d = s.child("dataLabels")?;
    let on = |name: &str| {
        d.child("visibility")
            .and_then(|v| v.attr(name))
            .is_some_and(|v| v == "1" || v == "true")
    };
    let l = DataLabels {
        value: on("value"),
        category: on("categoryName"),
        series: on("seriesName"),
        percent: false,
        pos: d.attr("pos").map(str::to_owned),
    };
    l.any().then_some(l)
}
