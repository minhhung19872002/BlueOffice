//! Chart formatting (CORE-CHART-007): writing and reading the shape and text properties of a
//! `c:chartSpace` that make up a chart style, colour variation and quick layout
//! ([`bo_model::chart_style::ChartFormat`]).
//!
//! Elements (ECMA-376 Part 1): §21.2.2.197 `c:spPr` (→ §20.1.2.2.35 CT_ShapeProperties: fill,
//! §20.1.2.2.24 `a:ln`, §20.1.8.25 `a:effectLst`), §21.2.2.216 `c:txPr` (→ §21.1.2.3.2 `a:defRPr`),
//! §21.2.2.100 `c:majorGridlines`, §21.2.2.49 `c:dLbls`, §21.2.2.156 `c:roundedCorners`.
//!
//! Chart style and colour parts ([MS-ODRAWXML] 2.3 `cs:chartStyle` / `cs:colorStyle`, namespace
//! `http://schemas.microsoft.com/office/drawing/2012/chartStyle`) are related from the chart part
//! and stay in the package untouched: they only describe how an application formats elements
//! added later, while the formatting written here is explicit on every element and takes
//! precedence ([MS-ODRAWXML] 2.3.1).

use crate::chart::Node;
use bo_model::chart_style::{ChartFormat, DataLabels, SeriesFill};
use bo_model::sheet::{ChartColor, ChartKind};
use bo_opc::xml::escape;
use std::fmt::Write as _;

/// Default series line width (2.25 pt) and outline width (0.75 pt) in EMU.
pub(crate) const LINE_W: u32 = 28_575;
const OUTLINE_W: u32 = 9_525;

/// A colour element (§20.1.2.3.32 srgbClr / §20.1.2.3.29 schemeClr) with colour transforms
/// (§20.1.2.3 lumMod, lumOff, alpha…) as children.
pub(crate) fn color_el(c: &ChartColor, mods: &str) -> String {
    let (tag, val) = match c {
        ChartColor::Rgb(v) => ("srgbClr", format!("{:06X}", v & 0xFF_FFFF)),
        ChartColor::Scheme(n) => ("schemeClr", escape(n)),
    };
    if mods.is_empty() {
        format!("<a:{tag} val=\"{val}\"/>")
    } else {
        format!("<a:{tag} val=\"{val}\">{mods}</a:{tag}>")
    }
}

fn solid(c: &ChartColor) -> String {
    format!("<a:solidFill>{}</a:solidFill>", color_el(c, ""))
}

/// §20.1.2.2.24 ln with a solid colour.
fn ln(c: &ChartColor, w: u32) -> String {
    format!("<a:ln w=\"{w}\">{}</a:ln>", solid(c))
}

/// §20.1.8.49 outerShdw: a soft drop shadow below the shape.
const SHADOW: &str = "<a:effectLst><a:outerShdw blurRad=\"38100\" dist=\"19050\" dir=\"5400000\" algn=\"t\" rotWithShape=\"0\"><a:srgbClr val=\"000000\"><a:alpha val=\"35000\"/></a:srgbClr></a:outerShdw></a:effectLst>";

/// The `c:spPr` of a series (or data point) in colour `c` with the treatment of `fmt`.
pub(crate) fn series_sp_pr(c: &ChartColor, fmt: &ChartFormat, line: bool) -> String {
    let mut out = String::from("<c:spPr>");
    if line {
        let w = fmt.line_w.unwrap_or(LINE_W);
        let _ = write!(
            out,
            "<a:ln w=\"{w}\" cap=\"rnd\">{}<a:round/></a:ln>",
            solid(c)
        );
    } else {
        let ow = if fmt.outline_w == 0 {
            OUTLINE_W
        } else {
            fmt.outline_w
        };
        match fmt.series_fill {
            SeriesFill::Solid => out.push_str(&solid(c)),
            // §20.1.8.33 gradFill: §20.1.8.37 gsLst of two §20.1.8.36 gs stops, §20.1.8.41 lin.
            SeriesFill::Gradient => {
                let _ = write!(
                    out,
                    "<a:gradFill rotWithShape=\"1\"><a:gsLst><a:gs pos=\"0\">{}</a:gs><a:gs pos=\"100000\">{}</a:gs></a:gsLst><a:lin ang=\"5400000\" scaled=\"0\"/></a:gradFill>",
                    color_el(c, "<a:lumMod val=\"60000\"/><a:lumOff val=\"40000\"/>"),
                    color_el(c, "")
                );
            }
            SeriesFill::Soft => {
                let _ = write!(
                    out,
                    "<a:solidFill>{}</a:solidFill>",
                    color_el(c, "<a:alpha val=\"65000\"/>")
                );
            }
            SeriesFill::Outline => out.push_str("<a:noFill/>"),
        }
        match (&fmt.series_fill, &fmt.outline) {
            (SeriesFill::Outline, _) => out.push_str(&ln(c, ow.max(19_050))),
            (_, Some(o)) => out.push_str(&ln(o, ow)),
            _ => {}
        }
    }
    if fmt.shadow {
        out.push_str(SHADOW);
    }
    out.push_str("</c:spPr>");
    out
}

/// §21.2.2.106 marker of a line / radar series.
pub(crate) fn marker(c: &ChartColor, fmt: &ChartFormat) -> String {
    if fmt.markers {
        format!(
            "<c:marker><c:symbol val=\"circle\"/><c:size val=\"6\"/><c:spPr>{}{}</c:spPr></c:marker>",
            solid(c),
            ln(&ChartColor::Rgb(0xFF_FFFF), OUTLINE_W)
        )
    } else {
        "<c:marker><c:symbol val=\"none\"/></c:marker>".to_owned()
    }
}

/// §21.2.2.49 dLbls of a chart group (placed right after its `c:ser` elements). `dLblPos` is only
/// written where §21.2.2.48 allows it for the group.
pub(crate) fn data_labels(fmt: &ChartFormat, kind: &ChartKind, grouping: Option<&str>) -> String {
    let Some(d) = fmt.data_labels.as_ref().filter(|d| d.any()) else {
        return String::new();
    };
    if *kind == ChartKind::Surface {
        return String::new();
    }
    let pie = matches!(
        kind,
        ChartKind::Pie | ChartKind::Doughnut | ChartKind::OfPie
    );
    let stacked = matches!(grouping, Some("stacked" | "percentStacked"));
    let pos = d.pos.as_deref().filter(|p| match kind {
        ChartKind::Pie | ChartKind::OfPie => {
            matches!(*p, "bestFit" | "ctr" | "inEnd" | "outEnd")
        }
        ChartKind::Bar if stacked => matches!(*p, "ctr" | "inEnd" | "inBase"),
        ChartKind::Bar => matches!(*p, "ctr" | "inEnd" | "inBase" | "outEnd"),
        ChartKind::Line | ChartKind::Scatter => matches!(*p, "t" | "b" | "l" | "r" | "ctr"),
        _ => false,
    });
    let mut out = String::from("<c:dLbls>");
    if let Some(p) = pos {
        let _ = write!(out, "<c:dLblPos val=\"{p}\"/>");
    }
    let b = |v: bool| u8::from(v);
    let _ = write!(
        out,
        "<c:showLegendKey val=\"0\"/><c:showVal val=\"{}\"/><c:showCatName val=\"{}\"/><c:showSerName val=\"{}\"/><c:showPercent val=\"{}\"/><c:showBubbleSize val=\"0\"/>",
        b(d.value),
        b(d.category),
        b(d.series),
        b(d.percent && pie)
    );
    if pie {
        out.push_str("<c:showLeaderLines val=\"1\"/>");
    }
    out.push_str("</c:dLbls>");
    out
}

/// §21.2.2.100 majorGridlines (with its line when the format colours it).
pub(crate) fn gridlines(fmt: &ChartFormat) -> String {
    match &fmt.grid_color {
        Some(c) => format!(
            "<c:majorGridlines><c:spPr>{}</c:spPr></c:majorGridlines>",
            ln(c, fmt.grid_w.unwrap_or(OUTLINE_W))
        ),
        None => "<c:majorGridlines/>".to_owned(),
    }
}

/// `c:spPr` of an axis (its line).
pub(crate) fn axis_sp_pr(fmt: &ChartFormat) -> String {
    fmt.axis_line.as_ref().map_or_else(String::new, |c| {
        format!("<c:spPr>{}</c:spPr>", ln(c, OUTLINE_W))
    })
}

/// `c:spPr` of the plot area (§21.2.2.145).
pub(crate) fn plot_sp_pr(fmt: &ChartFormat) -> String {
    fmt.plot_fill
        .as_ref()
        .map_or_else(String::new, |c| format!("<c:spPr>{}</c:spPr>", solid(c)))
}

/// `c:spPr` and `c:txPr` of the chart space (§21.2.2.29: after `c:chart`).
pub(crate) fn space_props(fmt: &ChartFormat) -> String {
    let mut out = String::new();
    if fmt.chart_fill.is_some() || fmt.chart_line.is_some() {
        out.push_str("<c:spPr>");
        if let Some(c) = &fmt.chart_fill {
            out.push_str(&solid(c));
        }
        if let Some(c) = &fmt.chart_line {
            out.push_str(&ln(c, OUTLINE_W));
        }
        out.push_str("</c:spPr>");
    }
    if fmt.text_color.is_some() || fmt.font_size.is_some() {
        // §21.1.2.3.2 defRPr: sz in hundredths of a point (100..=400000).
        out.push_str("<c:txPr><a:bodyPr/><a:lstStyle/><a:p><a:pPr><a:defRPr");
        if let Some(sz) = fmt.font_size {
            let _ = write!(out, " sz=\"{}\"", sz.clamp(100, 400_000));
        }
        out.push('>');
        if let Some(c) = &fmt.text_color {
            out.push_str(&solid(c));
        }
        out.push_str("</a:defRPr></a:pPr><a:endParaRPr lang=\"en-US\"/></a:p></c:txPr>");
    }
    out
}

// ---------------------------------------------------------------------------
// Reading
// ---------------------------------------------------------------------------

/// Colour of a fill element: §20.1.8.54 solidFill, or the last stop of a §20.1.8.33 gradFill.
pub(crate) fn fill_color(sp: &Node) -> Option<ChartColor> {
    if let Some(f) = sp.child("solidFill") {
        return color_of(f);
    }
    let g = sp.child("gradFill")?.child("gsLst")?;
    g.children
        .iter()
        .rfind(|c| c.name == "gs")
        .and_then(color_of)
}

/// `a:srgbClr` / `a:schemeClr` child of a fill or stop (transforms ignored).
pub(crate) fn color_of(fill: &Node) -> Option<ChartColor> {
    if let Some(c) = fill.child("srgbClr") {
        return c
            .attr("val")
            .and_then(|v| u32::from_str_radix(v.trim(), 16).ok())
            .map(ChartColor::Rgb);
    }
    fill.child("schemeClr")
        .and_then(|c| c.attr("val"))
        .map(|v| ChartColor::Scheme(v.to_owned()))
}

fn ln_of(sp: &Node) -> Option<(ChartColor, Option<u32>)> {
    let l = sp.child("ln")?;
    let c = l.child("solidFill").and_then(color_of)?;
    Some((c, l.attr("w").and_then(|w| w.trim().parse().ok())))
}

fn flag(n: &Node, name: &str) -> bool {
    n.child(name)
        .and_then(|c| c.attr("val"))
        .is_some_and(|v| v == "1" || v == "true")
}

fn labels_of(d: &Node) -> Option<DataLabels> {
    let l = DataLabels {
        value: flag(d, "showVal"),
        percent: flag(d, "showPercent"),
        category: flag(d, "showCatName"),
        series: flag(d, "showSerName"),
        pos: d
            .child("dLblPos")
            .and_then(|p| p.attr("val"))
            .map(str::to_owned),
    };
    l.any().then_some(l)
}

/// The formatting of a parsed `c:chartSpace` (`dom`) with its `c:chart` element.
pub(crate) fn read_format(dom: &Node, chart: &Node) -> ChartFormat {
    let mut f = ChartFormat {
        rounded: flag(dom, "roundedCorners"),
        ..ChartFormat::default()
    };
    if let Some(sp) = dom.child("spPr") {
        f.chart_fill = sp.child("solidFill").and_then(color_of);
        f.chart_line = ln_of(sp).map(|(c, _)| c);
    }
    if let Some(d) = dom
        .child("txPr")
        .and_then(|t| t.child("p"))
        .and_then(|p| p.child("pPr"))
        .and_then(|p| p.child("defRPr"))
    {
        f.text_color = d.child("solidFill").and_then(color_of);
        f.font_size = d.attr("sz").and_then(|s| s.trim().parse().ok());
    }
    if let Some(t) = chart.child("title") {
        let mut runs = Vec::new();
        collect(t, "rPr", &mut runs);
        collect(t, "defRPr", &mut runs);
        f.title_bold = runs
            .iter()
            .any(|r| r.attr("b").is_some_and(|b| b == "1" || b == "true"));
    }
    let Some(pa) = chart.child("plotArea") else {
        return f;
    };
    f.plot_fill = pa
        .child("spPr")
        .and_then(|sp| sp.child("solidFill"))
        .and_then(color_of);
    let axes: Vec<&Node> = pa
        .children
        .iter()
        .filter(|c| matches!(c.name.as_str(), "catAx" | "valAx" | "dateAx"))
        .collect();
    let vals: Vec<&&Node> = axes.iter().filter(|a| a.name == "valAx").collect();
    // Scatter / bubble charts: two value axes, the first being X.
    let (cat, val) = match axes.iter().find(|a| a.name != "valAx") {
        Some(c) => (Some(*c), vals.first().map(|v| **v)),
        None if vals.len() >= 2 => (Some(*vals[0]), Some(*vals[1])),
        None => (None, vals.first().map(|v| **v)),
    };
    if let Some(v) = val {
        let g = v.child("majorGridlines");
        f.hide_val_grid = g.is_none();
        if let Some((c, w)) = g.and_then(|g| g.child("spPr")).and_then(ln_of) {
            f.grid_color = Some(c);
            f.grid_w = w;
        }
    }
    if let Some(c) = cat {
        f.cat_grid = c.child("majorGridlines").is_some();
        f.axis_line = c.child("spPr").and_then(ln_of).map(|(c, _)| c);
    }
    let Some(g) = pa.children.iter().find(|c| c.name.ends_with("Chart")) else {
        return f;
    };
    let line = g.name.starts_with("line") || g.name == "scatterChart" || g.name == "radarChart";
    if g.name.starts_with("bar") {
        f.gap_width = g
            .child("gapWidth")
            .and_then(|w| w.attr("val"))
            .and_then(|v| v.trim().parse().ok())
            .filter(|w| *w != 150);
    }
    let ser = g.children.iter().find(|c| c.name == "ser");
    f.data_labels = g
        .child("dLbls")
        .or_else(|| ser.and_then(|s| s.child("dLbls")))
        .and_then(labels_of);
    let Some(s) = ser else { return f };
    // Pies keep their treatment on the data points.
    let sp = s.child("spPr").or_else(|| {
        s.children
            .iter()
            .find(|c| c.name == "dPt")
            .and_then(|d| d.child("spPr"))
    });
    if let Some(sp) = sp {
        f.shadow = sp
            .child("effectLst")
            .is_some_and(|e| e.child("outerShdw").is_some());
        if line {
            f.line_w = sp
                .child("ln")
                .and_then(|l| l.attr("w"))
                .and_then(|w| w.trim().parse().ok());
        } else {
            let alpha = sp
                .child("solidFill")
                .and_then(|s| s.children.first())
                .is_some_and(|c| c.child("alpha").is_some());
            f.series_fill = if sp.child("gradFill").is_some() {
                SeriesFill::Gradient
            } else if alpha {
                SeriesFill::Soft
            } else if sp.child("noFill").is_some() && sp.child("ln").is_some() {
                SeriesFill::Outline
            } else {
                SeriesFill::Solid
            };
            if let Some((c, w)) = ln_of(sp) {
                if f.series_fill != SeriesFill::Outline {
                    f.outline = Some(c);
                }
                f.outline_w = w.unwrap_or(0);
            }
        }
    }
    if line {
        f.markers = s
            .child("marker")
            .and_then(|m| m.child("symbol"))
            .and_then(|y| y.attr("val"))
            .is_some_and(|v| v != "none");
    }
    f
}

fn collect<'a>(n: &'a Node, name: &str, out: &mut Vec<&'a Node>) {
    for c in &n.children {
        if c.name == name {
            out.push(c);
        }
        collect(c, name, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bo_model::sheet::{Chart, ChartPlot, ChartSeries};

    fn chart(kind: ChartKind, fmt: ChartFormat) -> Chart {
        Chart {
            title: Some("T".into()),
            legend: Some("b".into()),
            plots: vec![ChartPlot {
                kind,
                bar_dir: Some("col".into()),
                series: vec![
                    ChartSeries {
                        name: Some("A".into()),
                        categories: vec!["x".into(), "y".into()],
                        values: vec![Some(1.0), Some(2.0)],
                        ..ChartSeries::default()
                    },
                    ChartSeries {
                        name: Some("B".into()),
                        categories: vec!["x".into(), "y".into()],
                        values: vec![Some(3.0), Some(4.0)],
                        ..ChartSeries::default()
                    },
                ],
                ..ChartPlot::default()
            }],
            format: fmt,
            ..Chart::default()
        }
    }

    fn full() -> ChartFormat {
        ChartFormat {
            palette: vec![ChartColor::Rgb(0x11_2233), ChartColor::Rgb(0x44_5566)],
            series_fill: SeriesFill::Gradient,
            outline: Some(ChartColor::Rgb(0xFF_FFFF)),
            outline_w: 12_700,
            shadow: true,
            gap_width: Some(80),
            chart_fill: Some(ChartColor::Rgb(0x10_2030)),
            chart_line: Some(ChartColor::Rgb(0x20_3040)),
            plot_fill: Some(ChartColor::Rgb(0x30_4050)),
            grid_color: Some(ChartColor::Rgb(0x40_5060)),
            grid_w: Some(6_350),
            axis_line: Some(ChartColor::Rgb(0x50_6070)),
            text_color: Some(ChartColor::Rgb(0xEE_EEEE)),
            font_size: Some(1_100),
            title_bold: true,
            rounded: true,
            hide_val_grid: false,
            cat_grid: true,
            data_labels: Some(DataLabels {
                value: true,
                pos: Some("outEnd".into()),
                ..DataLabels::default()
            }),
            ..ChartFormat::default()
        }
    }

    #[test]
    fn format_round_trips_through_the_chart_part() {
        let c = chart(ChartKind::Bar, full());
        let xml = crate::chart_write::chart_space_xml(&c);
        // Palette colours are written on the series (§21.2.2.150 ser spPr).
        assert!(xml.contains("<a:srgbClr val=\"112233\">"), "{xml}");
        assert!(xml.contains("<a:ln w=\"12700\"><a:solidFill><a:srgbClr val=\"FFFFFF\"/>"));
        assert!(xml.contains("<c:gapWidth val=\"80\"/>"));
        assert!(xml.contains("<c:dLblPos val=\"outEnd\"/>"));
        let back = crate::chart::parse_chart_space(xml.as_bytes(), "/c.xml").unwrap();
        let want = ChartFormat {
            palette: vec![],
            ..full()
        };
        assert_eq!(back.format, want);
        assert_eq!(
            back.plots[0].series[1].fill,
            Some(ChartColor::Rgb(0x44_5566))
        );
    }

    #[test]
    fn line_treatment_and_hidden_grid() {
        let fmt = ChartFormat {
            line_w: Some(38_100),
            markers: true,
            hide_val_grid: true,
            series_fill: SeriesFill::Solid,
            ..ChartFormat::default()
        };
        let c = chart(ChartKind::Line, fmt.clone());
        let xml = crate::chart_write::chart_space_xml(&c);
        assert!(!xml.contains("majorGridlines"));
        assert!(xml.contains("<c:symbol val=\"circle\"/>"));
        let back = crate::chart::parse_chart_space(xml.as_bytes(), "/c.xml").unwrap();
        assert_eq!(back.format, fmt);
    }
}
