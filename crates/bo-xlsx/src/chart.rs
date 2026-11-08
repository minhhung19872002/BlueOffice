//! Read-only chart summary: worksheet → drawing (ECMA-376 Part 1, §20.5
//! SpreadsheetML Drawing) → chart part (§21.2 DrawingML Charts, `c:chartSpace`).
//!
//! Drawing and chart parts are **not** consumed: they stay opaque package parts and
//! are written back byte-for-byte. The [`Chart`] values produced here only feed
//! rendering. Parse failures are tolerated (the chart is simply not summarized).

use crate::xml::{attrs, lname};
use bo_model::sheet::{
    CellRef, Chart, ChartAnchor, ChartKind, ChartPlot, ChartSeries, RawAttr, SheetDiagram,
};
use bo_opc::xml::{unescape, XmlReader};
use bo_opc::{resolve_target, Package, Relationship, TargetMode};
use quick_xml::events::Event;

/// Maximum number of elements kept in a mini DOM (defence against huge parts).
const MAX_NODES: usize = 200_000;

/// A tiny element tree (local names only), enough for chart summaries.
#[derive(Debug, Default)]
pub(crate) struct Node {
    pub(crate) name: String,
    pub(crate) attrs: Vec<RawAttr>,
    pub(crate) children: Vec<Node>,
    pub(crate) text: String,
}

impl Node {
    pub(crate) fn child(&self, name: &str) -> Option<&Node> {
        self.children.iter().find(|c| c.name == name)
    }

    fn path(&self, names: &[&str]) -> Option<&Node> {
        names.iter().try_fold(self, |n, name| n.child(name))
    }

    pub(crate) fn attr(&self, local: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(k, _)| k.rsplit_once(':').map_or(k.as_str(), |(_, l)| l) == local)
            .map(|(_, v)| v.as_str())
    }

    /// `val` attribute of child `name` (the common `<c:x val="…"/>` pattern).
    fn val_of(&self, name: &str) -> Option<String> {
        self.child(name)
            .and_then(|c| c.attr("val"))
            .map(str::to_owned)
    }

    fn descendants<'a>(&'a self, name: &str, out: &mut Vec<&'a Node>) {
        for c in &self.children {
            if c.name == name {
                out.push(c);
            }
            c.descendants(name, out);
        }
    }

    /// Concatenated text of all descendants named `name` (e.g. `a:t` runs).
    fn texts(&self, name: &str) -> String {
        let mut v = Vec::new();
        self.descendants(name, &mut v);
        v.iter().map(|n| n.text.as_str()).collect()
    }
}

/// Parse a part into a [`Node`] tree. Depth is bounded by [`XmlReader`].
pub(crate) fn parse_dom(data: &[u8], part: &str) -> Option<Node> {
    let mut r = XmlReader::new(data, part);
    let mut stack: Vec<Node> = vec![Node::default()];
    let mut count = 0usize;
    loop {
        match r.read().ok()? {
            Event::Start(e) => {
                count += 1;
                if count > MAX_NODES {
                    return None;
                }
                stack.push(Node {
                    name: lname(&e),
                    attrs: attrs(&e),
                    ..Node::default()
                });
            }
            Event::Empty(e) => {
                count += 1;
                if count > MAX_NODES {
                    return None;
                }
                let n = Node {
                    name: lname(&e),
                    attrs: attrs(&e),
                    ..Node::default()
                };
                stack.last_mut()?.children.push(n);
            }
            Event::End(_) => {
                let n = stack.pop()?;
                stack.last_mut()?.children.push(n);
            }
            Event::Text(t) => {
                if let Some(n) = stack.last_mut() {
                    n.text.push_str(&unescape(&t));
                }
            }
            Event::GeneralRef(g) => {
                if let Some(n) = stack.last_mut() {
                    let t: &str = &g;
                    n.text.push_str(&unescape(&format!("&{t};")));
                }
            }
            Event::CData(c) => {
                if let Some(n) = stack.last_mut() {
                    n.text.push_str(&c);
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    let mut doc = stack.pop()?;
    doc.children.pop()
}

/// Charts of a worksheet, following its `drawing` relationships.
pub(crate) fn read_charts(pkg: &Package, sheet_part: &str, rels: &[Relationship]) -> Vec<Chart> {
    let mut charts = Vec::new();
    for rel in rels {
        if rel.mode != TargetMode::Internal || !rel.rel_type.ends_with("/drawing") {
            continue;
        }
        let drawing = resolve_target(sheet_part, &rel.target);
        let Some(p) = pkg.part(&drawing) else {
            continue;
        };
        let Some(dom) = parse_dom(&p.data, &drawing) else {
            continue;
        };
        let drels = pkg.relationships(&drawing).unwrap_or_default();
        for anchor in &dom.children {
            // §20.5.2.33 twoCellAnchor, §20.5.2.24 oneCellAnchor, §20.5.2.1 absoluteAnchor
            let from = anchor.child("from").map(anchor_cell).unwrap_or_default();
            let to = anchor.child("to").map(anchor_cell);
            let kind = match anchor.name.as_str() {
                "oneCellAnchor" => ChartAnchor::OneCell,
                "absoluteAnchor" => ChartAnchor::Absolute,
                _ => ChartAnchor::TwoCell,
            };
            let emu = |n: &Node, a: &str, b: &str| {
                let v = |k: &str| n.attr(k).and_then(|v| v.trim().parse::<i64>().ok());
                Some((v(a)?, v(b)?))
            };
            // §20.5.2.25 pos, §20.5.2.14 ext (EMU).
            let pos = anchor.child("pos").and_then(|p| emu(p, "x", "y"));
            let ext = anchor.child("ext").and_then(|e| emu(e, "cx", "cy"));
            let mut refs = Vec::new();
            anchor.descendants("chart", &mut refs);
            for c in refs {
                let Some(id) = c.attr("id") else { continue };
                let Some(crel) = drels.iter().find(|r| r.id == id) else {
                    continue;
                };
                let part = resolve_target(&drawing, &crel.target);
                if let Some(cp) = pkg.part(&part) {
                    if let Some(mut chart) = parse_chart_space(&cp.data, &part) {
                        chart.from = from;
                        chart.to = to;
                        chart.anchor = kind;
                        if kind != ChartAnchor::TwoCell {
                            chart.pos = pos;
                            chart.ext = ext;
                        }
                        charts.push(chart);
                    }
                }
            }
        }
    }
    charts
}

/// SmartArt frames of a worksheet's drawings (§20.5.2.16 `xdr:graphicFrame`
/// whose `a:graphicData` holds `dgm:relIds`, §21.4.2.6), resolved to their
/// drawing fallback part: data part (`r:dm`) → `dsp:dataModelExt@relId`
/// ([MS-ODRAWXML]), a relationship of the sheet drawing part.
pub(crate) fn read_diagrams(
    pkg: &Package,
    sheet_part: &str,
    rels: &[Relationship],
) -> Vec<SheetDiagram> {
    const URI_DIAGRAM: &str = "http://schemas.openxmlformats.org/drawingml/2006/diagram";
    let mut out = Vec::new();
    for rel in rels {
        if rel.mode != TargetMode::Internal || !rel.rel_type.ends_with("/drawing") {
            continue;
        }
        let drawing = resolve_target(sheet_part, &rel.target);
        let Some(dom) = pkg
            .part(&drawing)
            .and_then(|p| parse_dom(&p.data, &drawing))
        else {
            continue;
        };
        let drels = pkg.relationships(&drawing).unwrap_or_default();
        let target = |id: &str| {
            drels
                .iter()
                .find(|r| r.id == id && r.mode == TargetMode::Internal)
                .map(|r| resolve_target(&drawing, &r.target))
        };
        for anchor in &dom.children {
            let Some(frame) = anchor.child("graphicFrame") else {
                continue;
            };
            let Some(gd) = frame.path(&["graphic", "graphicData"]) else {
                continue;
            };
            if gd.attr("uri") != Some(URI_DIAGRAM) {
                continue;
            }
            let Some(data_part) = gd
                .child("relIds")
                .and_then(|r| r.attr("dm"))
                .and_then(target)
            else {
                continue;
            };
            let data = pkg
                .part(&data_part)
                .and_then(|p| parse_dom(&p.data, &data_part));
            let mut ext = Vec::new();
            if let Some(d) = &data {
                d.descendants("dataModelExt", &mut ext);
            }
            // The drawing part, or the data model itself (laid out when drawn) when there is none.
            let Some(part) = ext
                .first()
                .and_then(|e| e.attr("relId"))
                .and_then(target)
                .filter(|p| pkg.part(p).is_some())
                .or_else(|| data.is_some().then(|| data_part.clone()))
            else {
                continue;
            };
            out.push(SheetDiagram {
                from: anchor.child("from").map(anchor_cell).unwrap_or_default(),
                to: anchor.child("to").map(anchor_cell),
                part,
                name: frame
                    .path(&["nvGraphicFramePr", "cNvPr"])
                    .and_then(|c| c.attr("name"))
                    .unwrap_or_default()
                    .to_owned(),
            });
        }
    }
    out
}

/// §20.5.2.15 from / §20.5.2.32 to (`xdr:col`, `xdr:row`, zero-based).
fn anchor_cell(n: &Node) -> CellRef {
    let num = |name: &str| {
        n.child(name)
            .and_then(|c| c.text.trim().parse::<u32>().ok())
            .unwrap_or(0)
    };
    CellRef::new(num("row"), num("col"))
}

/// Parse a `c:chartSpace` part (§21.2.2.29).
pub(crate) fn parse_chart_space(data: &[u8], part: &str) -> Option<Chart> {
    let dom = parse_dom(data, part)?;
    if dom.name != "chartSpace" {
        return None;
    }
    // [MS-ODRAWXML] 2.24 chartex: data in cx:chartData.
    if dom.child("chartData").is_some() {
        return crate::chartex::parse_chartex(&dom, part);
    }
    let chart = dom.child("chart")?;
    let mut out = Chart {
        part: part.to_owned(),
        ..Chart::default()
    };
    // §21.2.2.210 title: rich text or a cell reference with cached text.
    let deleted = chart.val_of("autoTitleDeleted").is_some_and(|v| v == "1");
    if let Some(t) = chart.child("title") {
        let rich = t.path(&["tx", "rich"]).map(|r| r.texts("t"));
        let cached = t.path(&["tx", "strRef"]).map(|s| s.texts("v"));
        out.title = rich.or(cached).filter(|s| !s.is_empty());
    } else if deleted {
        out.title = None;
    }
    // §21.2.2.140 plotArea
    if let Some(pa) = chart.child("plotArea") {
        for g in pa.children.iter().filter(|c| c.name.ends_with("Chart")) {
            out.plots.push(parse_plot(g));
        }
        // §21.2.2.25 catAx / §21.2.2.226 valAx titles (the first value axis is
        // vertical unless the plot is a horizontal bar chart).
        let title_of = |ax: &Node| {
            ax.child("title")
                .and_then(|t| t.path(&["tx", "rich"]).map(|r| r.texts("t")))
                .filter(|s| !s.is_empty())
        };
        let horizontal = out
            .plots
            .first()
            .is_some_and(|p| p.bar_dir.as_deref() == Some("bar"));
        let axes: Vec<&Node> = pa
            .children
            .iter()
            .filter(|c| matches!(c.name.as_str(), "catAx" | "valAx" | "dateAx"))
            .collect();
        let cat = axes
            .iter()
            .find(|a| a.name != "valAx")
            .or_else(|| axes.first())
            .and_then(|a| title_of(a));
        let val = axes
            .iter()
            .rfind(|a| a.name == "valAx")
            .and_then(|a| title_of(a));
        (out.x_title, out.y_title) = if horizontal { (val, cat) } else { (cat, val) };
    }
    // §21.2.2.93 legend
    if let Some(l) = chart.child("legend") {
        out.legend = Some(l.val_of("legendPos").unwrap_or_else(|| "r".into()));
    }
    // §21.2.2.142 pivotSource (pivot charts) and the c14:pivotOptions field buttons.
    if let Some(ps) = dom.child("pivotSource") {
        out.pivot_source = ps
            .child("name")
            .map(|n| n.text.trim().to_owned())
            .filter(|n| !n.is_empty());
        out.pivot_fmt_id = ps
            .val_of("fmtId")
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or(0);
        let mut opts = Vec::new();
        dom.descendants("pivotOptions", &mut opts);
        out.pivot_buttons = opts.first().is_some_and(|o| {
            o.children
                .iter()
                .any(|c| c.attr("val").is_none_or(|v| v == "1" || v == "true"))
        });
    }
    Some(out)
}

fn parse_plot(g: &Node) -> ChartPlot {
    let kind = match g.name.as_str() {
        "barChart" | "bar3DChart" => ChartKind::Bar,
        "lineChart" | "line3DChart" => ChartKind::Line,
        "areaChart" | "area3DChart" => ChartKind::Area,
        "pieChart" | "pie3DChart" => ChartKind::Pie,
        "doughnutChart" => ChartKind::Doughnut,
        "scatterChart" => ChartKind::Scatter,
        "radarChart" => ChartKind::Radar,
        "bubbleChart" => ChartKind::Bubble,
        "stockChart" => ChartKind::Stock,
        "surfaceChart" | "surface3DChart" => ChartKind::Surface,
        "ofPieChart" => ChartKind::OfPie,
        other => ChartKind::Other(other.to_owned()),
    };
    let on = |name: &str| {
        g.child(name)
            .is_some_and(|c| c.attr("val").is_none_or(|v| v == "1" || v == "true"))
    };
    ChartPlot {
        vary_colors: on("varyColors"),
        style: g.val_of("radarStyle").or_else(|| g.val_of("ofPieType")),
        // §21.2.2.218 upDownBars (stock), §21.2.2.230 wireframe (surface).
        flag: g.child("upDownBars").is_some() || on("wireframe"),
        kind,
        bar_dir: g.val_of("barDir"),
        grouping: g.val_of("grouping"),
        series: g
            .children
            .iter()
            .filter(|c| c.name == "ser")
            .map(parse_series)
            .collect(),
    }
}

/// The chart part `data` with each series reference (`c:f`, §21.2.2.215) that
/// differs from `chart` replaced — e.g. after rows were inserted above the
/// data; `None` when nothing changed. Formatting and everything else stay as
/// read (lossless, ADR-0005).
pub(crate) fn patch_refs(data: &[u8], part: &str, chart: &Chart) -> Option<Vec<u8>> {
    const OPEN: char = '\u{E000}';
    const CLOSE: char = '\u{E001}';
    let orig = parse_chart_space(data, part)?;
    let refs = |c: &Chart| -> Vec<Option<String>> {
        c.plots
            .iter()
            .flat_map(|p| &p.series)
            .flat_map(|s| [s.name_ref.clone(), s.cat_ref.clone(), s.val_ref.clone()])
            .collect()
    };
    let (old, new) = (refs(&orig), refs(chart));
    if old.len() != new.len() {
        return None;
    }
    let mut pairs: Vec<(String, String)> = Vec::new();
    for (o, n) in old.into_iter().zip(new) {
        if let (Some(o), Some(n)) = (o, n) {
            if o != n && !pairs.iter().any(|(a, _)| *a == o) {
                pairs.push((o, n));
            }
        }
    }
    if pairs.is_empty() {
        return None;
    }
    let mut xml = String::from_utf8(data.to_vec()).ok()?;
    // Replace through placeholders so that a new text equal to another old
    // one is not replaced twice.
    for (i, (o, _)) in pairs.iter().enumerate() {
        let from = format!("f>{}</", xml_text(o));
        xml = xml.replace(&from, &format!("f>{OPEN}{i}{CLOSE}</"));
    }
    for (i, (_, n)) in pairs.iter().enumerate() {
        xml = xml.replace(
            &format!("f>{OPEN}{i}{CLOSE}</"),
            &format!("f>{}</", xml_text(n)),
        );
    }
    Some(xml.into_bytes())
}

/// Text content escaped for XML (§2.4 of XML 1.0: `&`, `<`, `>`).
fn xml_text(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// §21.2.2.150 ser (and the scatter / bubble variants).
fn parse_series(s: &Node) -> ChartSeries {
    let mut out = ChartSeries::default();
    if let Some(tx) = s.child("tx") {
        if let Some(sr) = tx.child("strRef") {
            out.name_ref = sr.child("f").map(|f| f.text.clone());
            out.name = Some(sr.texts("v"));
        } else if let Some(v) = tx.child("v") {
            out.name = Some(v.text.clone());
        }
    }
    if let Some(cat) = s.child("cat").or_else(|| s.child("xVal")) {
        let (f, pts) = data_ref(cat);
        out.cat_ref = f;
        out.categories = pts.into_iter().map(Option::unwrap_or_default).collect();
    }
    if let Some(val) = s.child("val").or_else(|| s.child("yVal")) {
        let (f, pts) = data_ref(val);
        out.val_ref = f;
        out.values = pts
            .into_iter()
            .map(|p| p.and_then(|v| v.trim().parse::<f64>().ok()))
            .collect();
    }
    // §21.2.2.22 bubbleSize.
    if let Some(sz) = s.child("bubbleSize") {
        let (f, pts) = data_ref(sz);
        out.size_ref = f;
        out.sizes = pts
            .into_iter()
            .map(|p| p.and_then(|v| v.trim().parse::<f64>().ok()))
            .collect();
    }
    if let Some(sp) = s.child("spPr") {
        out.fill = sp.child("solidFill").and_then(color_of);
        out.line = sp
            .child("ln")
            .and_then(|l| l.child("solidFill"))
            .and_then(color_of);
    }
    // §21.2.2.52 dPt: per-point fills.
    for d in s.children.iter().filter(|c| c.name == "dPt") {
        let idx = d.val_of("idx").and_then(|v| v.trim().parse::<usize>().ok());
        let fill = d
            .child("spPr")
            .and_then(|sp| sp.child("solidFill"))
            .and_then(color_of);
        if let (Some(i), Some(c)) = (idx, fill) {
            out.point_fills.push((i, c));
        }
    }
    out.explosion = s
        .val_of("explosion")
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(0);
    out
}

/// The colour of a `a:solidFill` (§20.1.8.54): `a:srgbClr` or `a:schemeClr` (modifiers ignored).
fn color_of(fill: &Node) -> Option<bo_model::sheet::ChartColor> {
    use bo_model::sheet::ChartColor;
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

/// Formula and cached points of a `numRef` / `strRef` / `multiLvlStrRef` /
/// literal (`numLit` / `strLit`) data source (§21.2.2.121, §21.2.2.200).
fn data_ref(n: &Node) -> (Option<String>, Vec<Option<String>>) {
    let Some(src) = n.children.first() else {
        return (None, Vec::new());
    };
    let f = src.child("f").map(|f| f.text.clone());
    let cache = ["numCache", "strCache", "multiLvlStrCache"]
        .iter()
        .find_map(|c| src.child(c))
        .unwrap_or(src);
    let cache = if cache.name == "multiLvlStrCache" {
        cache.child("lvl").unwrap_or(cache)
    } else {
        cache
    };
    let count = cache
        .val_of("ptCount")
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(0)
        .min(MAX_NODES);
    let pts: Vec<&Node> = cache.children.iter().filter(|c| c.name == "pt").collect();
    let mut out = vec![None; count];
    for p in pts {
        let idx = p.attr("idx").and_then(|v| v.parse::<usize>().ok());
        let v = p.child("v").map(|v| v.text.clone());
        match idx {
            Some(i) if i < out.len() => out[i] = v,
            Some(i) if i < MAX_NODES => {
                out.resize(i + 1, None);
                out[i] = v;
            }
            _ => {}
        }
    }
    (f, out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const CHART: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<c:chartSpace xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
<c:chart><c:title><c:tx><c:rich><a:bodyPr/><a:p><a:r><a:t>Doanh thu </a:t></a:r><a:r><a:t>2024</a:t></a:r></a:p></c:rich></c:tx></c:title>
<c:plotArea><c:layout/>
<c:barChart><c:barDir val="col"/><c:grouping val="clustered"/>
<c:ser><c:idx val="0"/><c:tx><c:strRef><c:f>Sheet1!$B$1</c:f><c:strCache><c:ptCount val="1"/><c:pt idx="0"><c:v>Q1</c:v></c:pt></c:strCache></c:strRef></c:tx>
<c:cat><c:strRef><c:f>Sheet1!$A$2:$A$4</c:f><c:strCache><c:ptCount val="3"/><c:pt idx="0"><c:v>HN</c:v></c:pt><c:pt idx="2"><c:v>ĐN</c:v></c:pt></c:strCache></c:strRef></c:cat>
<c:val><c:numRef><c:f>Sheet1!$B$2:$B$4</c:f><c:numCache><c:formatCode>General</c:formatCode><c:ptCount val="3"/><c:pt idx="0"><c:v>10</c:v></c:pt><c:pt idx="1"><c:v>2.5</c:v></c:pt></c:numCache></c:numRef></c:val>
</c:ser></c:barChart>
<c:lineChart><c:grouping val="standard"/><c:ser><c:idx val="1"/><c:tx><c:v>Lit</c:v></c:tx><c:val><c:numLit><c:ptCount val="2"/><c:pt idx="0"><c:v>1</c:v></c:pt><c:pt idx="1"><c:v>2</c:v></c:pt></c:numLit></c:val></c:ser></c:lineChart>
</c:plotArea><c:legend><c:legendPos val="b"/></c:legend></c:chart></c:chartSpace>"#;

    #[test]
    fn parses_combo_chart() {
        let c = parse_chart_space(CHART.as_bytes(), "/xl/charts/chart1.xml").unwrap();
        assert_eq!(c.title.as_deref(), Some("Doanh thu 2024"));
        assert_eq!(c.legend.as_deref(), Some("b"));
        assert_eq!(c.plots.len(), 2);
        let bar = &c.plots[0];
        assert_eq!(bar.kind, ChartKind::Bar);
        assert_eq!(bar.bar_dir.as_deref(), Some("col"));
        let s = &bar.series[0];
        assert_eq!(s.name.as_deref(), Some("Q1"));
        assert_eq!(s.name_ref.as_deref(), Some("Sheet1!$B$1"));
        assert_eq!(s.cat_ref.as_deref(), Some("Sheet1!$A$2:$A$4"));
        assert_eq!(s.categories, vec!["HN", "", "ĐN"]);
        assert_eq!(s.values, vec![Some(10.0), Some(2.5), None]);
        assert_eq!(c.plots[1].kind, ChartKind::Line);
        assert_eq!(c.plots[1].series[0].name.as_deref(), Some("Lit"));
        assert_eq!(c.plots[1].series[0].values, vec![Some(1.0), Some(2.0)]);
    }

    #[test]
    fn garbage_is_tolerated() {
        assert!(parse_chart_space(b"<x/>", "p").is_none());
        assert!(parse_chart_space(b"<c:chartSpace><c:chart>", "p").is_none());
        assert!(parse_chart_space(b"\xff\xfe", "p").is_none());
    }
}
