//! Chart writer: `c:chartSpace` parts (ECMA-376 Part 1, §21.2 DrawingML Charts)
//! and their SpreadsheetML drawing anchors (§20.5).
//!
//! Charts read from a file stay opaque unless edited ([`Chart::dirty`]); edited
//! charts get a regenerated chart part under the same name, and new charts
//! (empty [`Chart::part`]) are added to the sheet's drawing (created when the
//! sheet has none).

use crate::chart_format as cf;
use crate::xml::fmt_f64;
use bo_model::chart_style::ChartFormat;
use bo_model::sheet::{
    CellRef, Chart, ChartAnchor, ChartColor, ChartKind, ChartPlot, ChartSeries, Workbook,
};
use bo_opc::xml::escape;
use std::fmt::Write as _;

/// Content type of a chart part (§21.2).
pub const CT_CHART: &str = "application/vnd.openxmlformats-officedocument.drawingml.chart+xml";
/// Content type of a SpreadsheetML drawing part (§20.5).
pub const CT_DRAWING: &str = "application/vnd.openxmlformats-officedocument.drawing+xml";
/// Relationship type sheet → drawing.
pub const REL_DRAWING: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/drawing";
/// Relationship type drawing → chart.
pub const REL_CHART: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/chart";

const C_NS: &str = "http://schemas.openxmlformats.org/drawingml/2006/chart";
const A_NS: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
const R_NS: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const XDR_NS: &str = "http://schemas.openxmlformats.org/drawingml/2006/spreadsheetDrawing";
const C14_NS: &str = "http://schemas.microsoft.com/office/drawing/2007/8/2/chart";
/// `ext/@uri` of `c14:pivotOptions` ([MS-ODRAWXML] chartSpace extensions).
pub(crate) const EXT_PIVOT_OPTIONS: &str = "{781A3756-C4B2-4CAC-9D66-4F8BD8637D16}";

/// Series colours of generated charts (BlueSheet palette, `RRGGBB`).
const PALETTE: [&str; 8] = [
    "2E6DB4", "F28C28", "2FA36B", "D64545", "7E57C2", "C9A227", "1FA7A8", "6B7280",
];

const CAT_AX: u32 = 50_010;
const VAL_AX: u32 = 50_020;
const SER_AX: u32 = 50_030;

/// `<a:p>` rich text body of a title (§21.2.2.156 rich); a bold run (§21.1.2.3.9 rPr `b`).
fn rich(out: &mut String, text: &str, bold: bool) {
    let rpr = if bold {
        "<a:rPr lang=\"en-US\" b=\"1\"/>"
    } else {
        ""
    };
    let _ = write!(
        out,
        "<c:tx><c:rich><a:bodyPr/><a:lstStyle/><a:p><a:r>{rpr}<a:t>{}</a:t></a:r></a:p></c:rich></c:tx>",
        escape(text)
    );
}

/// §21.2.2.210 title.
fn title(out: &mut String, text: &str, bold: bool) {
    out.push_str("<c:title>");
    rich(out, text, bold);
    out.push_str("<c:overlay val=\"0\"/></c:title>");
}

/// Colour of series (or pie point) `i` without a colour of its own: the format's palette, else
/// BlueSheet's.
fn auto_color(fmt: &ChartFormat, i: usize) -> ChartColor {
    match fmt.palette.len() {
        0 => ChartColor::Rgb(u32::from_str_radix(PALETTE[i % PALETTE.len()], 16).unwrap_or(0)),
        n => fmt.palette[i % n].clone(),
    }
}

/// §21.2.2.200 strRef with a cache.
fn str_ref(out: &mut String, f: Option<&str>, pts: &[String]) {
    match f {
        Some(f) => {
            let _ = write!(out, "<c:strRef><c:f>{}</c:f>", escape(f));
            str_cache(out, "c:strCache", pts);
            out.push_str("</c:strRef>");
        }
        None => str_cache(out, "c:strLit", pts),
    }
}

fn str_cache(out: &mut String, tag: &str, pts: &[String]) {
    let _ = write!(out, "<{tag}><c:ptCount val=\"{}\"/>", pts.len());
    for (i, p) in pts.iter().enumerate() {
        let _ = write!(out, "<c:pt idx=\"{i}\"><c:v>{}</c:v></c:pt>", escape(p));
    }
    let _ = write!(out, "</{tag}>");
}

/// §21.2.2.121 numRef with a cache (missing points are left out, §21.2.2.150).
fn num_ref(out: &mut String, f: Option<&str>, pts: &[Option<f64>]) {
    let cache = |out: &mut String, tag: &str| {
        let _ = write!(
            out,
            "<{tag}><c:formatCode>General</c:formatCode><c:ptCount val=\"{}\"/>",
            pts.len()
        );
        for (i, p) in pts.iter().enumerate() {
            if let Some(v) = p {
                let _ = write!(out, "<c:pt idx=\"{i}\"><c:v>{}</c:v></c:pt>", fmt_f64(*v));
            }
        }
        let _ = write!(out, "</{tag}>");
    };
    match f {
        Some(f) => {
            let _ = write!(out, "<c:numRef><c:f>{}</c:f>", escape(f));
            cache(out, "c:numCache");
            out.push_str("</c:numRef>");
        }
        None => cache(out, "c:numLit"),
    }
}

/// The `a:solidFill` colour value of a model colour (sRGB hex or a theme colour element).
fn fill_xml(c: &bo_model::sheet::ChartColor) -> String {
    match c {
        bo_model::sheet::ChartColor::Rgb(v) => {
            format!("<a:srgbClr val=\"{:06X}\"/>", v & 0xFF_FFFF)
        }
        bo_model::sheet::ChartColor::Scheme(n) => format!("<a:schemeClr val=\"{}\"/>", escape(n)),
    }
}

/// One `c:ser` (§21.2.2.150 and the per-type variants: CT_BarSer, CT_LineSer, CT_PieSer, CT_AreaSer,
/// CT_ScatterSer, CT_RadarSer, CT_BubbleSer, CT_SurfaceSer; children in schema order).
#[allow(clippy::too_many_lines)]
fn series(out: &mut String, kind: &ChartKind, s: &ChartSeries, idx: usize, fmt: &ChartFormat) {
    let color = s
        .fill
        .as_ref()
        .or(s.line.as_ref())
        .cloned()
        .unwrap_or_else(|| auto_color(fmt, idx));
    let _ = write!(out, "<c:ser><c:idx val=\"{idx}\"/><c:order val=\"{idx}\"/>");
    if s.name.is_some() || s.name_ref.is_some() {
        match &s.name_ref {
            Some(f) => {
                out.push_str("<c:tx>");
                str_ref(out, Some(f), &[s.name.clone().unwrap_or_default()]);
                out.push_str("</c:tx>");
            }
            None => {
                let _ = write!(
                    out,
                    "<c:tx><c:v>{}</c:v></c:tx>",
                    escape(s.name.as_deref().unwrap_or(""))
                );
            }
        }
    }
    let pie = matches!(
        kind,
        ChartKind::Pie | ChartKind::Doughnut | ChartKind::OfPie
    );
    let line_like = matches!(
        kind,
        ChartKind::Line | ChartKind::Scatter | ChartKind::Radar
    );
    if *kind == ChartKind::Stock {
        // Stock series draw only their high–low lines and ticks (§21.2.2.198).
        out.push_str("<c:spPr><a:ln w=\"19050\"><a:noFill/></a:ln></c:spPr><c:marker><c:symbol val=\"none\"/></c:marker>");
    } else if !pie {
        out.push_str(&cf::series_sp_pr(&color, fmt, line_like));
    }
    match kind {
        ChartKind::Bar | ChartKind::Bubble => out.push_str("<c:invertIfNegative val=\"0\"/>"),
        ChartKind::Line | ChartKind::Radar => out.push_str(&cf::marker(&color, fmt)),
        ChartKind::Scatter => {
            out.push_str("<c:marker><c:symbol val=\"circle\"/><c:size val=\"5\"/></c:marker>");
        }
        _ => {}
    }
    if pie && s.explosion > 0 {
        // §21.2.2.65 explosion.
        let _ = write!(out, "<c:explosion val=\"{}\"/>", s.explosion);
    }
    if pie {
        // Per-point colours (§21.2.2.52 dPt): the file's or the palette.
        for i in 0..s.values.len() {
            let fill = s
                .point_fills
                .iter()
                .find(|(k, _)| *k == i)
                .map_or_else(|| auto_color(fmt, i), |(_, c)| c.clone());
            let _ = write!(
                out,
                "<c:dPt><c:idx val=\"{i}\"/><c:bubble3D val=\"0\"/>{}</c:dPt>",
                cf::series_sp_pr(&fill, fmt, false)
            );
        }
    } else {
        for (i, c) in &s.point_fills {
            let _ = write!(
                out,
                "<c:dPt><c:idx val=\"{i}\"/><c:spPr><a:solidFill>{}</a:solidFill></c:spPr></c:dPt>",
                fill_xml(c)
            );
        }
    }
    // §21.2.2.211 trendline*, §21.2.2.58 errBars (after dPt / dLbls, before the data).
    if matches!(
        kind,
        ChartKind::Bar | ChartKind::Line | ChartKind::Area | ChartKind::Scatter | ChartKind::Bubble
    ) {
        for t in &s.trendlines {
            let ty = match t.kind.as_str() {
                k @ ("exp" | "log" | "poly" | "power" | "movingAvg") => k,
                _ => "linear",
            };
            let _ = write!(out, "<c:trendline><c:trendlineType val=\"{ty}\"/>");
            match ty {
                "poly" => {
                    let _ = write!(out, "<c:order val=\"{}\"/>", t.order.clamp(2, 6));
                }
                "movingAvg" => {
                    let _ = write!(out, "<c:period val=\"{}\"/>", t.order.max(2));
                }
                _ => {}
            }
            let _ = write!(
                out,
                "<c:dispRSqr val=\"{}\"/><c:dispEq val=\"{}\"/></c:trendline>",
                u8::from(t.disp_r2),
                u8::from(t.disp_eq)
            );
        }
        if let Some(e) = &s.err_bars {
            let ty = match e.kind.as_str() {
                k @ ("plus" | "minus") => k,
                _ => "both",
            };
            let vt = match e.value_type.as_str() {
                k @ ("percentage" | "stdDev" | "stdErr") => k,
                _ => "fixedVal",
            };
            // CT_ErrBars: errDir (not for bar / line / area: only XY series take a direction).
            out.push_str("<c:errBars>");
            if matches!(kind, ChartKind::Scatter | ChartKind::Bubble) {
                out.push_str("<c:errDir val=\"y\"/>");
            }
            let _ = write!(
                out,
                "<c:errBarType val=\"{ty}\"/><c:errValType val=\"{vt}\"/><c:noEndCap val=\"0\"/><c:val val=\"{}\"/></c:errBars>",
                e.value
            );
        }
    }
    if matches!(kind, ChartKind::Scatter | ChartKind::Bubble) {
        let xs: Option<Vec<Option<f64>>> = s
            .categories
            .iter()
            .map(|c| c.trim().parse::<f64>().ok().map(Some))
            .collect();
        if !s.categories.is_empty() || s.cat_ref.is_some() {
            out.push_str("<c:xVal>");
            match xs {
                Some(xs) if !xs.is_empty() => num_ref(out, s.cat_ref.as_deref(), &xs),
                _ => str_ref(out, s.cat_ref.as_deref(), &s.categories),
            }
            out.push_str("</c:xVal>");
        }
        out.push_str("<c:yVal>");
        num_ref(out, s.val_ref.as_deref(), &s.values);
        out.push_str("</c:yVal>");
        if *kind == ChartKind::Bubble {
            // §21.2.2.22 bubbleSize (a size of 1 for each point when none was given).
            out.push_str("<c:bubbleSize>");
            if s.sizes.is_empty() && s.size_ref.is_none() {
                num_ref(out, None, &vec![Some(1.0); s.values.len()]);
            } else {
                num_ref(out, s.size_ref.as_deref(), &s.sizes);
            }
            out.push_str("</c:bubbleSize><c:bubble3D val=\"0\"/></c:ser>");
        } else {
            out.push_str("<c:smooth val=\"0\"/></c:ser>");
        }
        return;
    }
    if !s.categories.is_empty() || s.cat_ref.is_some() {
        out.push_str("<c:cat>");
        str_ref(out, s.cat_ref.as_deref(), &s.categories);
        out.push_str("</c:cat>");
    }
    out.push_str("<c:val>");
    num_ref(out, s.val_ref.as_deref(), &s.values);
    out.push_str("</c:val>");
    if matches!(kind, ChartKind::Line | ChartKind::Stock) {
        out.push_str("<c:smooth val=\"0\"/>");
    }
    out.push_str("</c:ser>");
}

fn axis_ids(out: &mut String) {
    let _ = write!(out, "<c:axId val=\"{CAT_AX}\"/><c:axId val=\"{VAL_AX}\"/>");
}

/// Does a 3-D group need a series axis (§21.2.2.175 serAx)? Line and surface groups always have one,
/// bar and area groups when their series stand one behind the other (`standard` grouping).
fn needs_ser_ax(p: &ChartPlot) -> bool {
    p.three_d
        && match p.kind {
            ChartKind::Line | ChartKind::Surface => true,
            ChartKind::Bar | ChartKind::Area => p.grouping.as_deref() == Some("standard"),
            _ => false,
        }
}

/// A 3-D group: §21.2.2.15 bar3DChart, §21.2.2.96 line3DChart, §21.2.2.4 area3DChart,
/// §21.2.2.140 pie3DChart, §21.2.2.202 surface3DChart. `None` for kinds without a 3-D form.
fn plot_3d(out: &mut String, p: &ChartPlot, sers: &str, gap: u32) -> Option<bool> {
    let ids = |out: &mut String| {
        axis_ids(out);
        if needs_ser_ax(p) {
            let _ = write!(out, "<c:axId val=\"{SER_AX}\"/>");
        }
    };
    let g = p.grouping.as_deref();
    match p.kind {
        ChartKind::Bar => {
            let dir = if p.bar_dir.as_deref() == Some("bar") {
                "bar"
            } else {
                "col"
            };
            let g = g.unwrap_or("clustered");
            let _ = write!(
                out,
                "<c:bar3DChart><c:barDir val=\"{dir}\"/><c:grouping val=\"{g}\"/><c:varyColors val=\"0\"/>{sers}<c:gapWidth val=\"{gap}\"/>"
            );
            if let Some(shape) = p.style.as_deref().filter(|s| {
                matches!(
                    *s,
                    "box" | "cylinder" | "cone" | "coneToMax" | "pyramid" | "pyramidToMax"
                )
            }) {
                let _ = write!(out, "<c:shape val=\"{shape}\"/>");
            }
            ids(out);
            out.push_str("</c:bar3DChart>");
        }
        ChartKind::Line | ChartKind::Area => {
            let tag = if p.kind == ChartKind::Line {
                "line3DChart"
            } else {
                "area3DChart"
            };
            let g = if p.kind == ChartKind::Line {
                "standard"
            } else {
                g.unwrap_or("standard")
            };
            let _ = write!(
                out,
                "<c:{tag}><c:grouping val=\"{g}\"/><c:varyColors val=\"0\"/>{sers}"
            );
            if p.drop_lines {
                out.push_str("<c:dropLines/>");
            }
            ids(out);
            let _ = write!(out, "</c:{tag}>");
        }
        ChartKind::Pie => {
            let _ = write!(
                out,
                "<c:pie3DChart><c:varyColors val=\"1\"/>{sers}</c:pie3DChart>"
            );
            return Some(false);
        }
        ChartKind::Surface => {
            let _ = write!(
                out,
                "<c:surface3DChart><c:wireframe val=\"{}\"/>{sers}",
                u8::from(p.flag)
            );
            ids(out);
            out.push_str("</c:surface3DChart>");
        }
        _ => return None,
    }
    Some(true)
}

/// One plot group (§21.2.2.16 barChart, §21.2.2.97 lineChart, …).
fn plot(out: &mut String, p: &ChartPlot, first_idx: usize, fmt: &ChartFormat) -> bool {
    let grouping = p.grouping.as_deref();
    let mut sers = String::new();
    for (i, s) in p.series.iter().enumerate() {
        series(&mut sers, &p.kind, s, first_idx + i, fmt);
    }
    // §21.2.2.49 dLbls follows the series in every chart group that has it.
    sers.push_str(&cf::data_labels(fmt, &p.kind, grouping));
    let gap = fmt.gap_width.unwrap_or(150).min(500);
    if p.three_d {
        if let Some(axes) = plot_3d(out, p, &sers, gap) {
            return axes;
        }
    }
    match p.kind {
        ChartKind::Bar => {
            let dir = if p.bar_dir.as_deref() == Some("bar") {
                "bar"
            } else {
                "col"
            };
            let g = grouping.unwrap_or("clustered");
            let _ = write!(
                out,
                "<c:barChart><c:barDir val=\"{dir}\"/><c:grouping val=\"{g}\"/><c:varyColors val=\"0\"/>{sers}<c:gapWidth val=\"{gap}\"/>"
            );
            if g != "clustered" && g != "standard" {
                out.push_str("<c:overlap val=\"100\"/>");
            }
            axis_ids(out);
            out.push_str("</c:barChart>");
        }
        ChartKind::Line | ChartKind::Area => {
            let tag = if p.kind == ChartKind::Line {
                "lineChart"
            } else {
                "areaChart"
            };
            let g = grouping.unwrap_or("standard");
            let _ = write!(
                out,
                "<c:{tag}><c:grouping val=\"{g}\"/><c:varyColors val=\"0\"/>{sers}"
            );
            if p.drop_lines {
                out.push_str("<c:dropLines/>");
            }
            if p.kind == ChartKind::Line {
                if p.hi_low_lines {
                    out.push_str("<c:hiLowLines/>");
                }
                if p.flag {
                    out.push_str("<c:upDownBars><c:gapWidth val=\"150\"/><c:upBars/><c:downBars/></c:upDownBars>");
                }
                out.push_str("<c:marker val=\"1\"/>");
            }
            axis_ids(out);
            let _ = write!(out, "</c:{tag}>");
        }
        ChartKind::Pie => {
            let _ = write!(
                out,
                "<c:pieChart><c:varyColors val=\"1\"/>{sers}<c:firstSliceAng val=\"0\"/></c:pieChart>"
            );
            return false;
        }
        ChartKind::Doughnut => {
            let _ = write!(
                out,
                "<c:doughnutChart><c:varyColors val=\"1\"/>{sers}<c:firstSliceAng val=\"0\"/><c:holeSize val=\"50\"/></c:doughnutChart>"
            );
            return false;
        }
        ChartKind::Scatter => {
            let _ = write!(
                out,
                "<c:scatterChart><c:scatterStyle val=\"lineMarker\"/><c:varyColors val=\"0\"/>{sers}"
            );
            axis_ids(out);
            out.push_str("</c:scatterChart>");
        }
        // §21.2.2.153 radarChart (radarStyle standard | marker | filled).
        ChartKind::Radar => {
            let style = p
                .style
                .as_deref()
                .filter(|st| matches!(*st, "marker" | "filled"))
                .unwrap_or("standard");
            let _ = write!(
                out,
                "<c:radarChart><c:radarStyle val=\"{style}\"/><c:varyColors val=\"0\"/>{sers}"
            );
            axis_ids(out);
            out.push_str("</c:radarChart>");
        }
        // §21.2.2.20 bubbleChart.
        ChartKind::Bubble => {
            let _ = write!(
                out,
                "<c:bubbleChart><c:varyColors val=\"0\"/>{sers}<c:bubbleScale val=\"100\"/><c:showNegBubbles val=\"0\"/>"
            );
            axis_ids(out);
            out.push_str("</c:bubbleChart>");
        }
        // §21.2.2.198 stockChart with high-low lines (§21.2.2.80) and up/down bars (§21.2.2.218).
        ChartKind::Stock => {
            let _ = write!(out, "<c:stockChart>{sers}<c:hiLowLines/>");
            if p.flag {
                out.push_str("<c:upDownBars><c:gapWidth val=\"150\"/><c:upBars/><c:downBars/></c:upDownBars>");
            }
            axis_ids(out);
            out.push_str("</c:stockChart>");
        }
        // §21.2.2.203 surfaceChart (wireframe §21.2.2.230).
        ChartKind::Surface => {
            let _ = write!(
                out,
                "<c:surfaceChart><c:wireframe val=\"{}\"/>{sers}",
                u8::from(p.flag)
            );
            axis_ids(out);
            out.push_str("</c:surfaceChart>");
        }
        // §21.2.2.126 ofPieChart (ofPieType pie | bar).
        ChartKind::OfPie => {
            let ty = if p.style.as_deref() == Some("bar") {
                "bar"
            } else {
                "pie"
            };
            let _ = write!(
                out,
                "<c:ofPieChart><c:ofPieType val=\"{ty}\"/><c:varyColors val=\"1\"/>{sers}<c:gapWidth val=\"100\"/><c:secondPieSize val=\"75\"/><c:serLines/></c:ofPieChart>"
            );
            return false;
        }
        // Other kinds (and chartex kinds, written in their own part) as clustered columns.
        _ => {
            let _ = write!(
                out,
                "<c:barChart><c:barDir val=\"col\"/><c:grouping val=\"clustered\"/><c:varyColors val=\"0\"/>{sers}<c:gapWidth val=\"150\"/>"
            );
            axis_ids(out);
            out.push_str("</c:barChart>");
        }
    }
    true
}

/// §21.2.2.25 catAx / §21.2.2.226 valAx.
#[allow(clippy::too_many_arguments)]
fn axis(
    out: &mut String,
    tag: &str,
    id: u32,
    cross: u32,
    pos: &str,
    title_text: Option<&str>,
    grid: &str,
    scale: Option<&bo_model::sheet::ChartAxes>,
    sp_pr: &str,
) {
    // §21.2.2.160 scaling: logBase?, orientation, max?, min?.
    let _ = write!(out, "<c:{tag}><c:axId val=\"{id}\"/><c:scaling>");
    if let Some(b) = scale.and_then(|a| a.log_base).filter(|b| *b >= 2.0) {
        let _ = write!(out, "<c:logBase val=\"{b}\"/>");
    }
    out.push_str("<c:orientation val=\"minMax\"/>");
    if let Some(v) = scale.and_then(|a| a.max) {
        let _ = write!(out, "<c:max val=\"{v}\"/>");
    }
    if let Some(v) = scale.and_then(|a| a.min) {
        let _ = write!(out, "<c:min val=\"{v}\"/>");
    }
    let _ = write!(
        out,
        "</c:scaling><c:delete val=\"0\"/><c:axPos val=\"{pos}\"/>"
    );
    // §21.2.2.100 majorGridlines.
    out.push_str(grid);
    if let Some(t) = title_text.filter(|t| !t.is_empty()) {
        title(out, t, false);
    }
    if tag == "valAx" {
        out.push_str("<c:numFmt formatCode=\"General\" sourceLinked=\"1\"/>");
    }
    let _ = write!(
        out,
        "<c:majorTickMark val=\"out\"/><c:minorTickMark val=\"none\"/><c:tickLblPos val=\"nextTo\"/>{sp_pr}<c:crossAx val=\"{cross}\"/><c:crosses val=\"autoZero\"/>"
    );
    match tag {
        "valAx" => out.push_str("<c:crossBetween val=\"between\"/>"),
        // §21.2.2.39 dateAx: no label alignment; the base time unit.
        "dateAx" => out.push_str(
            "<c:auto val=\"1\"/><c:lblOffset val=\"100\"/><c:baseTimeUnit val=\"days\"/>",
        ),
        _ => out.push_str("<c:auto val=\"1\"/><c:lblAlgn val=\"ctr\"/><c:lblOffset val=\"100\"/>"),
    }
    let _ = write!(out, "</c:{tag}>");
}

/// Summary of a `c:chartSpace` part (§21.2.2.29), e.g. a chart embedded in a
/// presentation; `None` when the part is not a chart.
pub fn parse_chart_part(data: &[u8], part: &str) -> Option<Chart> {
    crate::chart::parse_chart_space(data, part)
}

/// The `c:chartSpace` part of `chart` (§21.2.2.29).
pub fn chart_space_xml(chart: &Chart) -> String {
    chart_space_xml_with(chart, None)
}

/// The `c:chartSpace` part of `chart` (§21.2.2.29) with an optional §21.2.2.63 externalData
/// reference to relationship `external` (an embedded workbook holding the data).
#[allow(clippy::too_many_lines)]
pub fn chart_space_xml_with(chart: &Chart, external: Option<&str>) -> String {
    let fmt = &chart.format;
    let mut out = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n");
    let _ = write!(
        out,
        "<c:chartSpace xmlns:c=\"{C_NS}\" xmlns:a=\"{A_NS}\" xmlns:r=\"{R_NS}\"><c:roundedCorners val=\"{}\"/>",
        u8::from(fmt.rounded)
    );
    // §21.2.2.142 pivotSource: name of the pivot table and its chart format id (§21.2.2.73).
    if let Some(src) = chart.pivot_source.as_deref().filter(|s| !s.is_empty()) {
        let _ = write!(
            out,
            "<c:pivotSource><c:name>{}</c:name><c:fmtId val=\"{}\"/></c:pivotSource>",
            escape(src),
            chart.pivot_fmt_id
        );
    }
    out.push_str("<c:chart>");
    match chart.title.as_deref().filter(|t| !t.is_empty()) {
        Some(t) => {
            title(&mut out, t, fmt.title_bold);
            out.push_str("<c:autoTitleDeleted val=\"0\"/>");
        }
        None => out.push_str("<c:autoTitleDeleted val=\"1\"/>"),
    }
    if chart.pivot_source.is_some() {
        // §21.2.2.144 pivotFmts: one format per series (§21.2.2.143 pivotFmt).
        let n: usize = chart.plots.iter().map(|p| p.series.len()).sum();
        out.push_str("<c:pivotFmts>");
        for i in 0..n.max(1) {
            let _ = write!(
                out,
                "<c:pivotFmt><c:idx val=\"{i}\"/><c:spPr><a:solidFill><a:srgbClr val=\"{}\"/></a:solidFill></c:spPr><c:marker><c:symbol val=\"none\"/></c:marker></c:pivotFmt>",
                PALETTE[i % PALETTE.len()]
            );
        }
        out.push_str("</c:pivotFmts>");
    }
    // §21.2.2.228 view3D (then §21.2.2.69 floor, §21.2.2.187 sideWall, §21.2.2.11 backWall, left to
    // the application defaults).
    if chart.plots.iter().any(|p| p.three_d) {
        let pie = chart.plots.iter().all(|p| p.kind == ChartKind::Pie);
        let v = chart
            .axes
            .view3d
            .clone()
            .unwrap_or_else(|| bo_model::sheet::View3D::new_chart(pie));
        let _ = write!(
            out,
            "<c:view3D><c:rotX val=\"{}\"/>",
            v.rot_x.clamp(-90, 90)
        );
        if let Some(h) = v.h_percent {
            let _ = write!(out, "<c:hPercent val=\"{}\"/>", h.clamp(5, 500));
        }
        let _ = write!(
            out,
            "<c:rotY val=\"{}\"/><c:depthPercent val=\"{}\"/><c:rAngAx val=\"{}\"/><c:perspective val=\"{}\"/></c:view3D>",
            v.rot_y.min(360),
            v.depth_percent.clamp(20, 2000),
            u8::from(v.r_ang_ax),
            v.perspective.min(240)
        );
    }
    out.push_str("<c:plotArea><c:layout/>");
    let mut axes = false;
    let mut idx = 0;
    for p in &chart.plots {
        axes |= plot(&mut out, p, idx, fmt);
        idx += p.series.len();
    }
    if chart.plots.is_empty() {
        // §21.2.2.145: a plot area needs a chart group.
        out.push_str("<c:barChart><c:barDir val=\"col\"/><c:grouping val=\"clustered\"/><c:varyColors val=\"0\"/>");
        axis_ids(&mut out);
        out.push_str("</c:barChart>");
        axes = true;
    }
    if axes {
        let scatter = chart
            .plots
            .iter()
            .all(|p| matches!(p.kind, ChartKind::Scatter | ChartKind::Bubble));
        let horizontal = chart
            .plots
            .iter()
            .all(|p| p.kind == ChartKind::Bar && p.bar_dir.as_deref() == Some("bar"));
        // The category axis title is the horizontal one unless bars are horizontal.
        let (cat_title, val_title) = if horizontal {
            (chart.y_title.as_deref(), chart.x_title.as_deref())
        } else {
            (chart.x_title.as_deref(), chart.y_title.as_deref())
        };
        let (cat_pos, val_pos) = if horizontal { ("l", "b") } else { ("b", "l") };
        let cat_tag = match (scatter, chart.axes.date) {
            (true, _) => "valAx",
            (false, true) => "dateAx",
            _ => "catAx",
        };
        let grid = cf::gridlines(fmt);
        let ax_sp = cf::axis_sp_pr(fmt);
        axis(
            &mut out,
            cat_tag,
            CAT_AX,
            VAL_AX,
            cat_pos,
            cat_title,
            if fmt.cat_grid { &grid } else { "" },
            None,
            &ax_sp,
        );
        axis(
            &mut out,
            "valAx",
            VAL_AX,
            CAT_AX,
            val_pos,
            val_title,
            if fmt.hide_val_grid { "" } else { &grid },
            Some(&chart.axes),
            &ax_sp,
        );
        // §21.2.2.175 serAx: the depth axis of a 3-D chart whose series stand one behind the other.
        if chart.plots.iter().any(needs_ser_ax) {
            let _ = write!(
                out,
                "<c:serAx><c:axId val=\"{SER_AX}\"/><c:scaling><c:orientation val=\"minMax\"/></c:scaling><c:delete val=\"0\"/><c:axPos val=\"b\"/><c:majorTickMark val=\"out\"/><c:minorTickMark val=\"none\"/><c:tickLblPos val=\"nextTo\"/><c:crossAx val=\"{VAL_AX}\"/><c:crosses val=\"autoZero\"/></c:serAx>"
            );
        }
        // §21.2.2.54 dTable.
        if chart.axes.data_table {
            out.push_str("<c:dTable><c:showHorzBorder val=\"1\"/><c:showVertBorder val=\"1\"/><c:showOutline val=\"1\"/><c:showKeys val=\"1\"/></c:dTable>");
        }
    }
    out.push_str(&cf::plot_sp_pr(fmt));
    out.push_str("</c:plotArea>");
    if let Some(pos) = &chart.legend {
        let pos = match pos.as_str() {
            "l" | "t" | "b" | "tr" => pos.as_str(),
            _ => "r",
        };
        let _ = write!(
            out,
            "<c:legend><c:legendPos val=\"{pos}\"/><c:overlay val=\"0\"/></c:legend>"
        );
    }
    out.push_str("<c:plotVisOnly val=\"1\"/><c:dispBlanksAs val=\"gap\"/></c:chart>");
    // §21.2.2.29: spPr, txPr and externalData follow the chart, before extLst.
    out.push_str(&cf::space_props(fmt));
    if let Some(rid) = external {
        let _ = write!(
            out,
            "<c:externalData r:id=\"{}\"><c:autoUpdate val=\"0\"/></c:externalData>",
            escape(rid)
        );
    }
    if chart.pivot_source.is_some() {
        // Field buttons of a pivot chart ([MS-ODRAWXML] c14:pivotOptions, chartSpace extension).
        let v = u8::from(chart.pivot_buttons);
        let _ = write!(
            out,
            "<c:extLst><c:ext uri=\"{EXT_PIVOT_OPTIONS}\" xmlns:c14=\"{C14_NS}\"><c14:pivotOptions><c14:dropZoneFilter val=\"{v}\"/><c14:dropZoneCategories val=\"{v}\"/><c14:dropZoneData val=\"{v}\"/><c14:dropZoneSeries val=\"{v}\"/><c14:dropZonesVisible val=\"{v}\"/></c14:pivotOptions></c:ext></c:extLst>"
        );
    }
    out.push_str("</c:chartSpace>");
    out
}

/// One `xdr:twoCellAnchor` (§20.5.2.33) holding a chart frame.
pub fn anchor_xml(chart: &Chart, shape_id: u32, rel_id: &str) -> String {
    let to = chart.to.unwrap_or(bo_model::sheet::CellRef::new(
        chart.from.row.saturating_add(15),
        chart.from.col.saturating_add(8),
    ));
    let mut out = String::new();
    let _ = write!(
        out,
        "<xdr:twoCellAnchor editAs=\"oneCell\"><xdr:from><xdr:col>{}</xdr:col><xdr:colOff>0</xdr:colOff><xdr:row>{}</xdr:row><xdr:rowOff>0</xdr:rowOff></xdr:from><xdr:to><xdr:col>{}</xdr:col><xdr:colOff>0</xdr:colOff><xdr:row>{}</xdr:row><xdr:rowOff>0</xdr:rowOff></xdr:to>",
        chart.from.col, chart.from.row, to.col, to.row
    );
    if chart.plots.first().is_some_and(|p| p.kind.is_ex()) {
        // A chartex frame is an mc:AlternateContent choice (ECMA-376 Part 3, §10.2.1) for the
        // applications that know the chartex namespaces; others show the fallback text box.
        let cx = crate::chartex::CX_NS;
        let (req, req_ns) = if chart
            .plots
            .first()
            .is_some_and(|p| p.kind == ChartKind::Funnel)
        {
            (
                "cx2",
                "http://schemas.microsoft.com/office/drawing/2015/10/21/chartex",
            )
        } else {
            (
                "cx1",
                "http://schemas.microsoft.com/office/drawing/2015/9/8/chartex",
            )
        };
        let _ = write!(
            out,
            "<mc:AlternateContent xmlns:mc=\"http://schemas.openxmlformats.org/markup-compatibility/2006\"><mc:Choice xmlns:{req}=\"{req_ns}\" Requires=\"{req}\"><xdr:graphicFrame macro=\"\"><xdr:nvGraphicFramePr><xdr:cNvPr id=\"{shape_id}\" name=\"Chart {shape_id}\"/><xdr:cNvGraphicFramePr/></xdr:nvGraphicFramePr><xdr:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"0\" cy=\"0\"/></xdr:xfrm><a:graphic><a:graphicData uri=\"{cx}\"><cx:chart xmlns:cx=\"{cx}\" xmlns:r=\"{R_NS}\" r:id=\"{}\"/></a:graphicData></a:graphic></xdr:graphicFrame></mc:Choice><mc:Fallback><xdr:sp macro=\"\" textlink=\"\"><xdr:nvSpPr><xdr:cNvPr id=\"{shape_id}\" name=\"Chart {shape_id}\"/><xdr:cNvSpPr><a:spLocks noTextEdit=\"1\"/></xdr:cNvSpPr></xdr:nvSpPr><xdr:spPr><a:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"0\" cy=\"0\"/></a:xfrm><a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom></xdr:spPr><xdr:txBody><a:bodyPr vertOverflow=\"clip\" horzOverflow=\"clip\"/><a:lstStyle/><a:p><a:r><a:rPr lang=\"vi-VN\" sz=\"1100\"/><a:t>{}</a:t></a:r></a:p></xdr:txBody></xdr:sp></mc:Fallback></mc:AlternateContent><xdr:clientData/></xdr:twoCellAnchor>",
            escape(rel_id),
            escape("Biểu đồ mở rộng (chartex): cần ứng dụng hỗ trợ để hiển thị.")
        );
        return out;
    }
    let _ = write!(
        out,
        "<xdr:graphicFrame macro=\"\"><xdr:nvGraphicFramePr><xdr:cNvPr id=\"{shape_id}\" name=\"Chart {shape_id}\"/><xdr:cNvGraphicFramePr/></xdr:nvGraphicFramePr><xdr:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"0\" cy=\"0\"/></xdr:xfrm><a:graphic><a:graphicData uri=\"{C_NS}\"><c:chart xmlns:c=\"{C_NS}\" xmlns:r=\"{R_NS}\" r:id=\"{}\"/></a:graphicData></a:graphic></xdr:graphicFrame><xdr:clientData/></xdr:twoCellAnchor>",
        escape(rel_id)
    );
    out
}

/// A new drawing part with `anchors`.
pub fn drawing_xml(anchors: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<xdr:wsDr xmlns:xdr=\"{XDR_NS}\" xmlns:a=\"{A_NS}\">{anchors}</xdr:wsDr>"
    )
}

/// Inserts `anchors` before the closing tag of an existing drawing part.
pub fn append_to_drawing(data: &[u8], anchors: &str) -> Option<Vec<u8>> {
    let text = std::str::from_utf8(data).ok()?;
    let close = text.rfind("</")?;
    let mut out = String::with_capacity(text.len() + anchors.len());
    out.push_str(&text[..close]);
    out.push_str(anchors);
    out.push_str(&text[close..]);
    Some(out.into_bytes())
}

/// Adds relationships to an existing `.rels` part (or creates one); returns
/// the part and the ids given to `targets`.
pub fn add_relationships(
    existing: Option<&[u8]>,
    targets: &[(String, bool)],
) -> (Vec<u8>, Vec<String>) {
    let base = existing
        .and_then(|d| std::str::from_utf8(d).ok())
        .map_or_else(
            || {
                "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"></Relationships>".to_owned()
            },
            str::to_owned,
        );
    let mut ids = Vec::new();
    let mut add = String::new();
    let mut n = 1;
    for (t, ex) in targets {
        let id = loop {
            let id = format!("rIdC{n}");
            n += 1;
            if !base.contains(&format!("\"{id}\"")) {
                break id;
            }
        };
        // A chartex part has its own relationship type ([MS-ODRAWXML] 2.1.x chartEx).
        let ty = if *ex {
            crate::chartex::REL_CHARTEX
        } else {
            REL_CHART
        };
        let _ = write!(
            add,
            "<Relationship Id=\"{id}\" Type=\"{ty}\" Target=\"{}\"/>",
            escape(t)
        );
        ids.push(id);
    }
    let out = match base.rfind("</Relationships>") {
        Some(i) => format!("{}{add}{}", &base[..i], &base[i..]),
        None => base.replace("/>", &format!(">{add}</Relationships>")),
    };
    (out.into_bytes(), ids)
}

/// The drawing part and relationship id anchoring chart `index` of `sheet`
/// (a chart read from the file).
fn chart_anchor(wb: &Workbook, sheet: usize, index: usize) -> Option<(String, String)> {
    let ws = wb.sheets.get(sheet)?;
    let chart = ws.charts.get(index)?;
    if chart.part.is_empty() {
        return None;
    }
    for rel in ws
        .rels
        .iter()
        .filter(|r| !r.external && r.rel_type.ends_with("/drawing"))
    {
        let rels_name = bo_opc::rels_part_name(&rel.target);
        let Some(rels) = wb.opaque.parts.iter().find(|p| p.name == rels_name) else {
            continue;
        };
        let text = std::str::from_utf8(&rels.data).ok()?;
        for (id, target) in relationship_targets(text) {
            if bo_opc::resolve_target(&rel.target, &target) == chart.part {
                return Some((rel.target.clone(), id));
            }
        }
    }
    None
}

/// `(Id, Target)` of every `Relationship` element of a `.rels` part.
pub(crate) fn relationship_targets(text: &str) -> Vec<(String, String)> {
    let attr = |el: &str, name: &str| -> Option<String> {
        let at = el.find(&format!(" {name}=\""))? + name.len() + 3;
        let end = el[at..].find('"')? + at;
        Some(bo_opc::xml::unescape(&el[at..end]))
    };
    text.split("<Relationship ")
        .skip(1)
        .filter_map(|el| {
            let el = &format!(" {}", el.split('>').next().unwrap_or(""));
            Some((attr(el, "Id")?, attr(el, "Target")?))
        })
        .collect()
}

/// Byte range of the drawing anchor (§20.5.2.1 absoluteAnchor, §20.5.2.24
/// oneCellAnchor, §20.5.2.33 twoCellAnchor) holding the chart with `rel_id`,
/// with the anchor's qualified name.
fn anchor_span(text: &str, rel_id: &str) -> Option<(usize, usize, String)> {
    anchor_span_of(text, &format!(":id=\"{rel_id}\""))
}

/// Byte range and qualified name of the drawing anchor holding the first `needle`.
pub(crate) fn anchor_span_of(text: &str, needle: &str) -> Option<(usize, usize, String)> {
    anchor_span_at(text, text.find(needle)?)
}

/// Start of the first element `<local …>` (any prefix) whose `name` attribute is `name`.
fn named_element(text: &str, local: &str, name: &str) -> Option<usize> {
    let attr = format!(" name=\"{}\"", escape(name));
    let mut from = 0;
    while let Some(p) = text[from..].find('<') {
        let at = from + p;
        from = at + 1;
        let tag_end = text[at..].find('>').map_or(text.len(), |e| at + e);
        let tag = &text[at + 1..tag_end];
        let qname = tag
            .split(|c: char| c.is_whitespace() || c == '/')
            .next()
            .unwrap_or_default();
        let l = qname.rsplit_once(':').map_or(qname, |(_, l)| l);
        if l == local && tag.contains(&attr) {
            return Some(at);
        }
    }
    None
}

/// Byte range and qualified name of the drawing anchor holding byte `id_at`.
fn anchor_span_at(text: &str, id_at: usize) -> Option<(usize, usize, String)> {
    let head = &text[..id_at];
    let (start, qname) = ["twoCellAnchor", "oneCellAnchor", "absoluteAnchor"]
        .iter()
        .filter_map(|n| {
            let mut best = None;
            let mut from = 0;
            while let Some(p) = head[from..].find(n) {
                let at = from + p;
                // `<n` or `<prefix:n` followed by a space or `>`.
                let lt = head[..at].rfind('<')?;
                let q = &head[lt + 1..at + n.len()];
                let next = head[at + n.len()..].chars().next();
                if !q.starts_with('/')
                    && q.chars()
                        .all(|c| c.is_alphanumeric() || c == ':' || c == '_')
                    && matches!(next, Some(' ' | '>'))
                {
                    best = Some((lt, q.to_owned()));
                }
                from = at + n.len();
            }
            best
        })
        .max_by_key(|(lt, _)| *lt)?;
    let close = format!("</{qname}>");
    let end = text[id_at..].find(&close)? + id_at + close.len();
    Some((start, end, qname))
}

/// Element `<p:name>…</p:name>` (prefix taken from `qname`) of an anchor:
/// `(start, end)` byte range.
fn element_span(text: &str, prefix: &str, name: &str) -> Option<(usize, usize)> {
    let open = format!("<{prefix}{name}>");
    let close = format!("</{prefix}{name}>");
    let s = text.find(&open)?;
    let e = text[s..].find(&close)? + s + close.len();
    Some((s, e))
}

/// A drawing frame located by an element's `name`: (element local name, its `name`, text
/// the anchor must hold, cells).
type Frame<'a> = (
    &'static str,
    &'a str,
    Option<&'static str>,
    CellRef,
    Option<CellRef>,
);

/// The drawing anchors of the file charts of `sheet` whose cells no longer
/// match the model (rows or columns inserted / deleted, §20.5.2.15 from,
/// §20.5.2.32 to): only the `col` / `row` values change, offsets and the rest
/// of the drawing part stay as read. Patched drawings go to `overrides`.
pub(crate) fn sync_anchors(
    wb: &Workbook,
    sheet: usize,
    overrides: &mut std::collections::HashMap<String, Vec<u8>>,
) {
    let Some(ws) = wb.sheets.get(sheet) else {
        return;
    };
    for (i, ch) in ws.charts.iter().enumerate() {
        if ch.part.is_empty() || ch.dirty || ch.anchor == ChartAnchor::Absolute {
            continue;
        }
        let Some((drawing, rel_id)) = chart_anchor(wb, sheet, i) else {
            continue;
        };
        let text = match overrides.get(&drawing) {
            Some(d) => String::from_utf8(d.clone()).ok(),
            None => wb
                .opaque
                .parts
                .iter()
                .find(|p| p.name == drawing)
                .and_then(|p| String::from_utf8(p.data.clone()).ok()),
        };
        let Some(text) = text else { continue };
        let to = (ch.anchor == ChartAnchor::TwoCell)
            .then_some(ch.to)
            .flatten();
        if let Some(patched) = sync_anchor(&text, &rel_id, ch.from, to) {
            overrides.insert(drawing, patched.into_bytes());
        }
    }
    // Slicer / timeline frames ([MS-XLSX] `sle:slicer`, `tsle:timeslicer` by `name`) and
    // SmartArt frames (§20.5.2.16 graphicFrame `cNvPr/@name` with `dgm:relIds`) read from
    // the file: their drawing anchors follow the model cells.
    let mut frames: Vec<Frame<'_>> = Vec::new();
    for s in ws.slicers.iter().filter(|s| !s.part.is_empty()) {
        frames.push(("slicer", &s.name, None, s.from, s.to));
    }
    for t in ws.timelines.iter().filter(|t| !t.part.is_empty()) {
        frames.push(("timeslicer", &t.name, None, t.from, t.to));
    }
    for d in ws.diagrams.iter().filter(|d| !d.name.is_empty()) {
        frames.push(("cNvPr", &d.name, Some("relIds"), d.from, d.to));
    }
    if frames.is_empty() {
        return;
    }
    for r in ws
        .rels
        .iter()
        .filter(|r| !r.external && r.rel_type.ends_with("/drawing"))
    {
        let current = overrides.get(&r.target).cloned().or_else(|| {
            wb.opaque
                .parts
                .iter()
                .find(|p| p.name == r.target)
                .map(|p| p.data.clone())
        });
        let Some(mut text) = current.and_then(|d| String::from_utf8(d).ok()) else {
            continue;
        };
        let mut changed = false;
        for (local, name, must, from, to) in &frames {
            let Some(at) = named_element(&text, local, name) else {
                continue;
            };
            let Some((s, e, _)) = anchor_span_at(&text, at) else {
                continue;
            };
            if must.is_some_and(|m| !text[s..e].contains(m)) {
                continue;
            }
            if let Some(p) = sync_anchor_at(&text, at, *from, *to) {
                text = p;
                changed = true;
            }
        }
        if changed {
            overrides.insert(r.target.clone(), text.into_bytes());
        }
    }
}

/// `text` with the `from` (and `to`) cell of the anchor of `rel_id` set to the
/// given cells; `None` when they already match.
fn sync_anchor(text: &str, rel_id: &str, from: CellRef, to: Option<CellRef>) -> Option<String> {
    sync_anchor_at(text, text.find(&format!(":id=\"{rel_id}\""))?, from, to)
}

/// `text` with the cells of the SmartArt frame whose `dgm:relIds@r:dm` is `rel_dm` (§21.4.2.6)
/// set to `from` / `to`; `None` when they already match.
pub(crate) fn sync_diagram_anchor(
    text: &str,
    rel_dm: &str,
    from: CellRef,
    to: Option<CellRef>,
) -> Option<String> {
    sync_anchor_at(text, text.find(&format!(":dm=\"{rel_dm}\""))?, from, to)
}

/// [`sync_anchor`] for the anchor holding byte `at`.
fn sync_anchor_at(text: &str, at: usize, from: CellRef, to: Option<CellRef>) -> Option<String> {
    let (s, e, qname) = anchor_span_at(text, at)?;
    let prefix = qname
        .rsplit_once(':')
        .map_or(String::new(), |(p, _)| format!("{p}:"));
    let mut anchor = text[s..e].to_owned();
    let mut changed = false;
    for (tag, cell) in [("from", Some(from)), ("to", to)] {
        let Some(cell) = cell else { continue };
        let Some((a, b)) = element_span(&anchor, &prefix, tag) else {
            continue;
        };
        let mut el = anchor[a..b].to_owned();
        for (child, v) in [("col", cell.col), ("row", cell.row)] {
            let Some((ca, cb)) = element_span(&el, &prefix, child) else {
                continue;
            };
            let open = format!("<{prefix}{child}>");
            let close = format!("</{prefix}{child}>");
            let cur = el[ca + open.len()..cb - close.len()].trim();
            if cur.parse::<u32>().ok() != Some(v) {
                el.replace_range(ca..cb, &format!("{open}{v}{close}"));
                changed = true;
            }
        }
        anchor.replace_range(a..b, &el);
    }
    changed.then(|| format!("{}{anchor}{}", &text[..s], &text[e..]))
}

/// Replace the empty element `<p:name …/>` of an anchor by `with`.
fn replace_empty(anchor: &mut String, prefix: &str, name: &str, with: &str) -> bool {
    let open = format!("<{prefix}{name} ");
    let Some(s) = anchor.find(&open) else {
        return false;
    };
    let Some(e) = anchor[s..].find("/>").map(|e| s + e + 2) else {
        return false;
    };
    anchor.replace_range(s..e, with);
    true
}

/// Move / resize chart `index` of `sheet` to the cell anchors `from`..`to`
/// (zero-based; `to` is the cell holding the bottom-right corner). Charts read
/// from the file get their drawing anchor rewritten: §20.5.2.15 from /
/// §20.5.2.32 to of a two-cell anchor; `from` and §20.5.2.14 ext of a one-cell
/// anchor; §20.5.2.25 pos and ext of an absolute anchor. `emu` is the target
/// rectangle `(x, y, cx, cy)` in EMU, needed by one-cell and absolute anchors.
/// Returns `false` when the chart cannot be moved.
pub fn move_chart(
    wb: &mut Workbook,
    sheet: usize,
    index: usize,
    from: CellRef,
    to: CellRef,
    emu: Option<(i64, i64, i64, i64)>,
) -> bool {
    let Some(chart) = wb.sheets.get(sheet).and_then(|s| s.charts.get(index)) else {
        return false;
    };
    if !chart.part.is_empty() {
        let Some((drawing, rel_id)) = chart_anchor(wb, sheet, index) else {
            return false;
        };
        let Some(part) = wb.opaque.parts.iter_mut().find(|p| p.name == drawing) else {
            return false;
        };
        let Ok(text) = std::str::from_utf8(&part.data) else {
            return false;
        };
        let Some((s, e, qname)) = anchor_span(text, &rel_id) else {
            return false;
        };
        let prefix = qname
            .rsplit_once(':')
            .map_or(String::new(), |(p, _)| format!("{p}:"));
        let mut anchor = text[s..e].to_owned();
        let cell = |tag: &str, at: CellRef| {
            format!(
                "<{prefix}{tag}><{prefix}col>{}</{prefix}col><{prefix}colOff>0</{prefix}colOff><{prefix}row>{}</{prefix}row><{prefix}rowOff>0</{prefix}rowOff></{prefix}{tag}>",
                at.col, at.row
            )
        };
        let absolute = qname.ends_with("absoluteAnchor");
        let one_cell = qname.ends_with("oneCellAnchor");
        if absolute || one_cell {
            let Some((x, y, cx, cy)) = emu else {
                return false;
            };
            let ext = format!("<{prefix}ext cx=\"{}\" cy=\"{}\"/>", cx.max(1), cy.max(1));
            if !replace_empty(&mut anchor, &prefix, "ext", &ext) {
                return false;
            }
            if absolute {
                let pos = format!("<{prefix}pos x=\"{}\" y=\"{}\"/>", x.max(0), y.max(0));
                if !replace_empty(&mut anchor, &prefix, "pos", &pos) {
                    return false;
                }
            }
        }
        if !absolute {
            if let Some((a, b)) = element_span(&anchor, &prefix, "from") {
                anchor.replace_range(a..b, &cell("from", from));
            }
        }
        if qname.ends_with("twoCellAnchor") {
            if let Some((a, b)) = element_span(&anchor, &prefix, "to") {
                anchor.replace_range(a..b, &cell("to", to));
            }
        }
        let mut out = String::with_capacity(text.len());
        out.push_str(&text[..s]);
        out.push_str(&anchor);
        out.push_str(&text[e..]);
        part.data = out.into_bytes();
    }
    let chart = &mut wb.sheets[sheet].charts[index];
    chart.from = from;
    chart.to = Some(to);
    if let Some((x, y, cx, cy)) = emu {
        if chart.anchor == ChartAnchor::Absolute {
            chart.pos = Some((x.max(0), y.max(0)));
        }
        if chart.anchor != ChartAnchor::TwoCell {
            chart.ext = Some((cx.max(1), cy.max(1)));
        }
    }
    true
}

/// Delete chart `index` of `sheet`. For a chart read from the file its drawing
/// anchor and relationship are removed, together with the chart part and the
/// parts only it referenced (style / colour parts). Returns `false` when not found.
pub fn remove_chart(wb: &mut Workbook, sheet: usize, index: usize) -> bool {
    let Some(chart) = wb.sheets.get(sheet).and_then(|s| s.charts.get(index)) else {
        return false;
    };
    if !chart.part.is_empty() {
        let part_name = chart.part.clone();
        let Some((drawing, rel_id)) = chart_anchor(wb, sheet, index) else {
            return false;
        };
        // Drawing: drop the anchor.
        if let Some(p) = wb.opaque.parts.iter_mut().find(|p| p.name == drawing) {
            if let Ok(text) = std::str::from_utf8(&p.data) {
                if let Some((s, e, _)) = anchor_span(text, &rel_id) {
                    let mut t = text.to_owned();
                    t.replace_range(s..e, "");
                    p.data = t.into_bytes();
                }
            }
        }
        // Drawing relationships: drop the chart relationship.
        let rels_name = bo_opc::rels_part_name(&drawing);
        if let Some(p) = wb.opaque.parts.iter_mut().find(|p| p.name == rels_name) {
            if let Ok(text) = std::str::from_utf8(&p.data) {
                let mut t = text.to_owned();
                if let Some(at) = t.find(&format!("Id=\"{rel_id}\"")) {
                    if let (Some(s), Some(e)) = (t[..at].rfind('<'), t[at..].find("/>")) {
                        t.replace_range(s..at + e + 2, "");
                    }
                }
                p.data = t.into_bytes();
            }
        }
        // The chart part, its relationships and the parts only they reference.
        let chart_rels = bo_opc::rels_part_name(&part_name);
        let owned: Vec<String> = wb
            .opaque
            .parts
            .iter()
            .find(|p| p.name == chart_rels)
            .and_then(|p| std::str::from_utf8(&p.data).ok().map(str::to_owned))
            .map(|t| {
                relationship_targets(&t)
                    .into_iter()
                    .map(|(_, target)| bo_opc::resolve_target(&part_name, &target))
                    .filter(|target| target.contains("/charts/"))
                    .collect()
            })
            .unwrap_or_default();
        wb.opaque
            .parts
            .retain(|p| p.name != part_name && p.name != chart_rels && !owned.contains(&p.name));
    }
    wb.sheets[sheet].charts.remove(index);
    true
}
#[cfg(test)]
mod tests {
    use super::*;
    use bo_model::sheet::CellRef;

    // source: [MS-XLSX] sle:slicer / tsle:timeslicer frames; ECMA-376 Part 1, §20.5.2.33
    #[test]
    fn slicer_timeline_and_smartart_frames_follow_their_cells() {
        use bo_model::sheet::{OpaquePart, OpaqueRel, SheetDiagram, Slicer, Timeline};
        let m = |c: u32, r: u32| {
            format!("<xdr:col>{c}</xdr:col><xdr:colOff>7</xdr:colOff><xdr:row>{r}</xdr:row><xdr:rowOff>9</xdr:rowOff>")
        };
        let anchor = |inner: &str| {
            format!(
                "<xdr:twoCellAnchor editAs=\"oneCell\"><xdr:from>{}</xdr:from><xdr:to>{}</xdr:to>{inner}<xdr:clientData/></xdr:twoCellAnchor>",
                m(1, 2),
                m(3, 8)
            )
        };
        let drawing = format!(
            "<xdr:wsDr xmlns:xdr=\"x\">{}{}{}</xdr:wsDr>",
            anchor("<mc:AlternateContent><mc:Choice Requires=\"a14\"><xdr:graphicFrame><xdr:nvGraphicFramePr><xdr:cNvPr id=\"2\" name=\"Tỉnh\"/></xdr:nvGraphicFramePr><a:graphic><a:graphicData><sle:slicer xmlns:sle=\"s\" name=\"Tỉnh\"/></a:graphicData></a:graphic></xdr:graphicFrame></mc:Choice></mc:AlternateContent>"),
            anchor("<xdr:graphicFrame><a:graphic><a:graphicData><tsle:timeslicer xmlns:tsle=\"t\" name=\"Ngày\"/></a:graphicData></a:graphic></xdr:graphicFrame>"),
            anchor("<xdr:graphicFrame><xdr:nvGraphicFramePr><xdr:cNvPr id=\"4\" name=\"Diagram 1\"/></xdr:nvGraphicFramePr><a:graphic><a:graphicData><dgm:relIds r:dm=\"rId1\"/></a:graphicData></a:graphic></xdr:graphicFrame>")
        );
        let mut wb = Workbook::new();
        wb.opaque.parts.push(OpaquePart {
            name: "/xl/drawings/drawing1.xml".into(),
            content_type: None,
            data: drawing.into_bytes(),
        });
        let ws = &mut wb.sheets[0];
        ws.rels.push(OpaqueRel {
            id: "rId1".into(),
            rel_type: REL_DRAWING.into(),
            target: "/xl/drawings/drawing1.xml".into(),
            external: false,
        });
        // Two rows inserted above (slicer), a column before (timeline), unchanged (diagram).
        ws.slicers.push(Slicer {
            name: "Tỉnh".into(),
            part: "/xl/slicers/slicer1.xml".into(),
            from: CellRef::new(4, 1),
            to: Some(CellRef::new(10, 3)),
            ..Slicer::default()
        });
        ws.timelines.push(Timeline {
            name: "Ngày".into(),
            part: "/xl/timelines/timeline1.xml".into(),
            from: CellRef::new(2, 2),
            to: Some(CellRef::new(8, 4)),
            ..Timeline::default()
        });
        ws.diagrams.push(SheetDiagram {
            name: "Diagram 1".into(),
            from: CellRef::new(2, 1),
            to: Some(CellRef::new(8, 3)),
            ..SheetDiagram::default()
        });
        let mut overrides = std::collections::HashMap::new();
        sync_anchors(&wb, 0, &mut overrides);
        let out = String::from_utf8(overrides["/xl/drawings/drawing1.xml"].clone()).unwrap();
        let anchors: Vec<&str> = out.split("<xdr:twoCellAnchor").skip(1).collect();
        assert!(anchors[0].contains(&m(1, 4)) && anchors[0].contains(&m(3, 10)));
        assert!(anchors[1].contains(&m(2, 2)) && anchors[1].contains(&m(4, 8)));
        assert!(anchors[2].contains(&m(1, 2)) && anchors[2].contains(&m(3, 8)));
    }

    fn chart(kind: ChartKind) -> Chart {
        Chart {
            title: Some("Doanh thu & lãi".into()),
            x_title: Some("Tỉnh".into()),
            y_title: Some("Tỷ đồng".into()),
            legend: Some("b".into()),
            from: CellRef::new(1, 4),
            plots: vec![ChartPlot {
                kind,
                bar_dir: Some("col".into()),
                grouping: Some("clustered".into()),
                series: vec![ChartSeries {
                    name: Some("2025".into()),
                    name_ref: Some("Sheet1!$B$1".into()),
                    cat_ref: Some("Sheet1!$A$2:$A$3".into()),
                    val_ref: Some("Sheet1!$B$2:$B$3".into()),
                    categories: vec!["1".into(), "2".into()],
                    values: vec![Some(1.5), None],
                    ..ChartSeries::default()
                }],
                ..ChartPlot::default()
            }],
            ..Chart::default()
        }
    }

    #[test]
    fn writes_well_formed_chart_space() {
        for k in [
            ChartKind::Bar,
            ChartKind::Line,
            ChartKind::Area,
            ChartKind::Pie,
            ChartKind::Doughnut,
            ChartKind::Scatter,
        ] {
            let xml = chart_space_xml(&chart(k.clone()));
            assert!(crate::canonicalize_xml(xml.as_bytes()).is_ok(), "{k:?}");
            assert!(xml.contains("Doanh thu &amp; lãi"));
            let pie = matches!(k, ChartKind::Pie | ChartKind::Doughnut);
            assert_eq!(xml.contains("<c:valAx>"), !pie, "{k:?}");
        }
        let xml = chart_space_xml(&chart(ChartKind::Bar));
        assert!(xml.contains("<c:legendPos val=\"b\"/>"));
        assert!(xml.contains("<c:f>Sheet1!$B$2:$B$3</c:f>"));
        assert!(xml.contains("<c:pt idx=\"0\"><c:v>1.5</c:v></c:pt>"));
        assert!(xml.contains("<c:formatCode>General</c:formatCode><c:ptCount val=\"2\"/><c:pt idx=\"0\"><c:v>1.5</c:v></c:pt></c:numCache>"), "missing point left out");
    }

    #[test]
    fn drawings_and_relationships() {
        let a = anchor_xml(&chart(ChartKind::Bar), 2, "rId1");
        assert!(a.contains("<xdr:col>4</xdr:col>") && a.contains("<xdr:row>16</xdr:row>"));
        let d = drawing_xml(&a);
        let more = append_to_drawing(d.as_bytes(), "<x/>").unwrap();
        assert!(String::from_utf8(more)
            .unwrap()
            .ends_with("<x/></xdr:wsDr>"));
        let (rels, ids) = add_relationships(None, &[("../charts/chart1.xml".into(), false)]);
        assert_eq!(ids, vec!["rIdC1"]);
        let (rels2, ids2) =
            add_relationships(Some(&rels), &[("../charts/chart2.xml".into(), false)]);
        assert_eq!(ids2, vec!["rIdC2"]);
        let s = String::from_utf8(rels2).unwrap();
        assert!(s.contains("chart1.xml") && s.contains("chart2.xml"));
    }

    #[test]
    fn new_and_edited_charts_round_trip() {
        use bo_model::sheet::{Cell, CellValue, Workbook};
        let mut wb = Workbook::new();
        let ws = &mut wb.sheets[0];
        ws.set_cell(0, 1, Cell::new(CellValue::Text("2025".into())));
        ws.set_cell(1, 0, Cell::new(CellValue::Text("HN".into())));
        ws.set_cell(1, 1, Cell::new(CellValue::Number(1.5)));
        ws.charts.push(chart(ChartKind::Bar));
        let bytes = crate::write_xlsx(&wb).unwrap();
        let back = crate::read_xlsx(&bytes).unwrap();
        let c = &back.sheets[0].charts[0];
        assert_eq!(c.part, "/xl/charts/chart1.xml");
        assert_eq!(c.title.as_deref(), Some("Doanh thu & lãi"));
        assert_eq!(
            (c.x_title.as_deref(), c.y_title.as_deref()),
            (Some("Tỉnh"), Some("Tỷ đồng"))
        );
        assert_eq!(c.legend.as_deref(), Some("b"));
        assert_eq!(c.from, CellRef::new(1, 4));
        assert_eq!(
            c.plots[0].series[0].val_ref.as_deref(),
            Some("Sheet1!$B$2:$B$3")
        );
        // Edit the title and add a second chart to the existing drawing.
        let mut wb2 = back.clone();
        wb2.sheets[0].charts[0].title = Some("Mới".into());
        wb2.sheets[0].charts[0].dirty = true;
        let mut extra = chart(ChartKind::Line);
        extra.from = CellRef::new(20, 4);
        wb2.sheets[0].charts.push(extra);
        let again = crate::read_xlsx(&crate::write_xlsx(&wb2).unwrap()).unwrap();
        let cs = &again.sheets[0].charts;
        assert_eq!(cs.len(), 2);
        assert_eq!(cs[0].title.as_deref(), Some("Mới"));
        assert_eq!(cs[1].plots[0].kind, ChartKind::Line);
        assert_eq!(cs[1].from, CellRef::new(20, 4));
        // Untouched charts stay byte-for-byte.
        let third = crate::read_xlsx(&crate::write_xlsx(&again).unwrap()).unwrap();
        assert_eq!(third.sheets[0].charts, again.sheets[0].charts);
    }

    #[test]
    fn file_charts_move_and_delete() {
        use bo_model::sheet::{Cell, CellValue, Workbook};
        let mut wb = Workbook::new();
        wb.sheets[0].set_cell(1, 1, Cell::new(CellValue::Number(1.5)));
        wb.sheets[0].charts.push(chart(ChartKind::Bar));
        let mut second = chart(ChartKind::Line);
        second.from = CellRef::new(20, 4);
        wb.sheets[0].charts.push(second);
        let mut back = crate::read_xlsx(&crate::write_xlsx(&wb).unwrap()).unwrap();
        assert_eq!(back.sheets[0].charts.len(), 2);
        // Move + resize the first chart (a chart read from the file).
        assert!(move_chart(
            &mut back,
            0,
            0,
            CellRef::new(3, 2),
            CellRef::new(10, 7),
            None
        ));
        let moved = crate::read_xlsx(&crate::write_xlsx(&back).unwrap()).unwrap();
        let c = &moved.sheets[0].charts[0];
        assert_eq!(
            (c.from, c.to),
            (CellRef::new(3, 2), Some(CellRef::new(10, 7)))
        );
        assert_eq!(moved.sheets[0].charts[1].from, CellRef::new(20, 4));
        // Delete it: anchor, relationship and part go away.
        let mut del = moved.clone();
        let part = del.sheets[0].charts[0].part.clone();
        assert!(remove_chart(&mut del, 0, 0));
        let bytes = crate::write_xlsx(&del).unwrap();
        let again = crate::read_xlsx(&bytes).unwrap();
        assert_eq!(again.sheets[0].charts.len(), 1);
        assert_eq!(again.sheets[0].charts[0].plots[0].kind, ChartKind::Line);
        assert!(again.opaque.parts.iter().all(|p| p.name != part));
        let pkg = bo_opc::Package::read(&bytes, &bo_opc::Limits::default()).unwrap();
        assert!(pkg.part(&part).is_none());
        // New (unsaved) charts move and delete in the model only.
        let mut fresh = Workbook::new();
        fresh.sheets[0].charts.push(chart(ChartKind::Pie));
        assert!(move_chart(
            &mut fresh,
            0,
            0,
            CellRef::new(0, 0),
            CellRef::new(5, 5),
            None
        ));
        assert_eq!(fresh.sheets[0].charts[0].to, Some(CellRef::new(5, 5)));
        assert!(remove_chart(&mut fresh, 0, 0));
        assert_eq!(fresh.sheets[0].charts.len(), 0);
    }
}
