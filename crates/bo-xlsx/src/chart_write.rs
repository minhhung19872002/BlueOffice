//! Chart writer: `c:chartSpace` parts (ECMA-376 Part 1, §21.2 DrawingML Charts)
//! and their SpreadsheetML drawing anchors (§20.5).
//!
//! Charts read from a file stay opaque unless edited ([`Chart::dirty`]); edited
//! charts get a regenerated chart part under the same name, and new charts
//! (empty [`Chart::part`]) are added to the sheet's drawing (created when the
//! sheet has none).

use crate::xml::fmt_f64;
use bo_model::sheet::{CellRef, Chart, ChartAnchor, ChartKind, ChartPlot, ChartSeries, Workbook};
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

/// `<a:p>` rich text body of a title (§21.2.2.156 rich).
fn rich(out: &mut String, text: &str) {
    let _ = write!(
        out,
        "<c:tx><c:rich><a:bodyPr/><a:lstStyle/><a:p><a:r><a:t>{}</a:t></a:r></a:p></c:rich></c:tx>",
        escape(text)
    );
}

/// §21.2.2.210 title.
fn title(out: &mut String, text: &str) {
    out.push_str("<c:title>");
    rich(out, text);
    out.push_str("<c:overlay val=\"0\"/></c:title>");
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

fn sp_pr(out: &mut String, color: &str, line: bool) {
    if line {
        let _ = write!(
            out,
            "<c:spPr><a:ln w=\"28575\" cap=\"rnd\"><a:solidFill><a:srgbClr val=\"{color}\"/></a:solidFill><a:round/></a:ln></c:spPr>"
        );
    } else {
        let _ = write!(
            out,
            "<c:spPr><a:solidFill><a:srgbClr val=\"{color}\"/></a:solidFill></c:spPr>"
        );
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
fn series(out: &mut String, kind: &ChartKind, s: &ChartSeries, idx: usize) {
    let color = PALETTE[idx % PALETTE.len()];
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
        match s.fill.as_ref().or(s.line.as_ref()) {
            Some(c) if line_like => {
                let _ = write!(
                    out,
                    "<c:spPr><a:ln w=\"28575\" cap=\"rnd\"><a:solidFill>{}</a:solidFill><a:round/></a:ln></c:spPr>",
                    fill_xml(c)
                );
            }
            Some(c) => {
                let _ = write!(
                    out,
                    "<c:spPr><a:solidFill>{}</a:solidFill></c:spPr>",
                    fill_xml(c)
                );
            }
            None => sp_pr(out, color, line_like),
        }
    }
    match kind {
        ChartKind::Bar | ChartKind::Bubble => out.push_str("<c:invertIfNegative val=\"0\"/>"),
        ChartKind::Line | ChartKind::Radar => {
            out.push_str("<c:marker><c:symbol val=\"none\"/></c:marker>");
        }
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
            let fill = s.point_fills.iter().find(|(k, _)| *k == i).map_or_else(
                || format!("<a:srgbClr val=\"{}\"/>", PALETTE[i % PALETTE.len()]),
                |(_, c)| fill_xml(c),
            );
            let _ = write!(
                out,
                "<c:dPt><c:idx val=\"{i}\"/><c:bubble3D val=\"0\"/><c:spPr><a:solidFill>{fill}</a:solidFill></c:spPr></c:dPt>"
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

/// One plot group (§21.2.2.16 barChart, §21.2.2.97 lineChart, …).
fn plot(out: &mut String, p: &ChartPlot, first_idx: usize) -> bool {
    let grouping = p.grouping.as_deref();
    let mut sers = String::new();
    for (i, s) in p.series.iter().enumerate() {
        series(&mut sers, &p.kind, s, first_idx + i);
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
                "<c:barChart><c:barDir val=\"{dir}\"/><c:grouping val=\"{g}\"/><c:varyColors val=\"0\"/>{sers}<c:gapWidth val=\"150\"/>"
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
            if p.kind == ChartKind::Line {
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
fn axis(
    out: &mut String,
    tag: &str,
    id: u32,
    cross: u32,
    pos: &str,
    title_text: Option<&str>,
    grid: bool,
) {
    let _ = write!(
        out,
        "<c:{tag}><c:axId val=\"{id}\"/><c:scaling><c:orientation val=\"minMax\"/></c:scaling><c:delete val=\"0\"/><c:axPos val=\"{pos}\"/>"
    );
    if grid {
        out.push_str("<c:majorGridlines/>");
    }
    if let Some(t) = title_text.filter(|t| !t.is_empty()) {
        title(out, t);
    }
    if tag == "valAx" {
        out.push_str("<c:numFmt formatCode=\"General\" sourceLinked=\"1\"/>");
    }
    let _ = write!(
        out,
        "<c:majorTickMark val=\"out\"/><c:minorTickMark val=\"none\"/><c:tickLblPos val=\"nextTo\"/><c:crossAx val=\"{cross}\"/><c:crosses val=\"autoZero\"/>"
    );
    if tag == "valAx" {
        out.push_str("<c:crossBetween val=\"between\"/>");
    } else {
        out.push_str("<c:auto val=\"1\"/><c:lblAlgn val=\"ctr\"/><c:lblOffset val=\"100\"/>");
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
    let mut out = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n");
    let _ = write!(
        out,
        "<c:chartSpace xmlns:c=\"{C_NS}\" xmlns:a=\"{A_NS}\" xmlns:r=\"{R_NS}\"><c:roundedCorners val=\"0\"/>"
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
            title(&mut out, t);
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
    out.push_str("<c:plotArea><c:layout/>");
    let mut axes = false;
    let mut idx = 0;
    for p in &chart.plots {
        axes |= plot(&mut out, p, idx);
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
        let cat_tag = if scatter { "valAx" } else { "catAx" };
        axis(&mut out, cat_tag, CAT_AX, VAL_AX, cat_pos, cat_title, false);
        axis(&mut out, "valAx", VAL_AX, CAT_AX, val_pos, val_title, true);
    }
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
fn relationship_targets(text: &str) -> Vec<(String, String)> {
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
    let needle = format!(":id=\"{rel_id}\"");
    let id_at = text.find(&needle)?;
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
}

/// `text` with the `from` (and `to`) cell of the anchor of `rel_id` set to the
/// given cells; `None` when they already match.
fn sync_anchor(text: &str, rel_id: &str, from: CellRef, to: Option<CellRef>) -> Option<String> {
    let (s, e, qname) = anchor_span(text, rel_id)?;
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
