//! Charts on slides (prompt §6 Phase 4 "chart (dùng chung bo-drawingml)").
//!
//! A chart is a graphic frame (§19.3.1.21) whose `graphicData` holds a
//! §21.2.2.27 `c:chart` reference to a chart part (§21.2.2.29 chartSpace). The
//! chart part keeps its data in caches (`c:strCache` / `c:numCache`) pointing at
//! an embedded workbook (§21.2.2.63 externalData → package relationship).
//!
//! Inserting writes the chart part with BlueSheet's chart writer
//! (`bo_xlsx::chart_write`) and an embedded `.xlsx` holding the same data.
//! Editing data, type, title or legend regenerates both from the chart summary
//! (styling of a chart read from a file is not kept once it is edited, like
//! BlueSheet); a chart that is not edited stays byte-for-byte.

use super::build;
use super::geom::{find, place_slide};
use super::ops::{node_at, Tx};
use super::{insert_top, next_shape_id, slide_size, EditError, SlideSel, R};
use bo_model::sheet::{CellValue, Chart, ChartKind, ChartPlot, ChartSeries, Workbook, Worksheet};
use bo_model::slide::{GraphicContent, Presentation, RawPart, Rel, ShapeNode};
use bo_opc::{Package, Relationship, TargetMode};
use serde::{Deserialize, Serialize};

/// Content type of an embedded workbook (ECMA-376 Part 1, §12.3.24 SpreadsheetML package).
const CT_XLSX: &str = "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet";
/// Relationship type of an embedded package (§15.2.11 / §21.2.2.63 externalData).
const REL_PACKAGE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/package";
const NS_C: &str = "http://schemas.openxmlformats.org/drawingml/2006/chart";
const NS_R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
/// Sheet holding the data in the embedded workbook.
const SHEET: &str = "Sheet1";
/// Size limits of the data grid.
const MAX_ROWS: usize = 1000;
const MAX_SERIES: usize = 50;

/// One data series of the chart grid.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct SeriesData {
    /// Series name (header of its column).
    pub name: String,
    /// One value per category; `None` = empty cell.
    pub values: Vec<Option<f64>>,
}

/// The chart grid: categories down, series across.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ChartData {
    /// Category labels, one per row.
    pub categories: Vec<String>,
    /// Series, one per column.
    pub series: Vec<SeriesData>,
}

/// The selected chart as seen by the UI.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChartOut {
    /// Shape id of the chart's graphic frame (`p:cNvPr/@id`).
    pub id: u32,
    /// `column` | `bar` | `line` | `area` | `pie` | `doughnut` | `other`.
    pub kind: String,
    /// Chart title text; `None` = no title (§21.2.2.210 title).
    pub title: Option<String>,
    /// `r` | `l` | `t` | `b` | `tr`; `None` = no legend.
    pub legend: Option<String>,
    /// Data grid of the chart series (from the cached category and value data).
    pub data: ChartData,
    /// Has an embedded workbook.
    pub embedded: bool,
}

/// Fields of the set command (absent = unchanged).
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChartSpec {
    /// New chart type: `column` | `bar` | `line` | `area` | `pie` | `doughnut`.
    #[serde(default)]
    pub kind: Option<String>,
    /// Empty = no title.
    #[serde(default)]
    pub title: Option<String>,
    /// `none` = no legend.
    #[serde(default)]
    pub legend: Option<String>,
    /// New data grid (rewrites the caches and the embedded workbook).
    #[serde(default)]
    pub data: Option<ChartData>,
    /// Chart template, style, colour variation, quick layout (CORE-CHART-007).
    #[serde(flatten)]
    pub design: crate::chart_design::DesignSpec,
}

/// The slide dialog's name of the chart's type (`other` for types the dialog does not offer).
pub(crate) fn kind_name(c: &Chart) -> &'static str {
    let name = bo_sheet::chart::type_name(c);
    if matches!(
        name,
        "column" | "bar" | "line" | "area" | "pie" | "doughnut"
    ) || MORE_KINDS.contains(&name)
    {
        name
    } else {
        "other"
    }
}

/// Chart types of the slide chart dialog beyond the basic six (`bo_sheet::chart` names).
const MORE_KINDS: &[&str] = &[
    "stackedColumn",
    "percentColumn",
    "stackedBar",
    "percentBar",
    "stackedLine",
    "percentLine",
    "stackedArea",
    "percentArea",
    "explodedPie",
    "pieOfPie",
    "barOfPie",
    "radar",
    "radarMarker",
    "radarFilled",
    "stock",
    "stockUpDown",
    "stockVolume",
    "stockVolumeUpDown",
    "surface",
    "column3D",
    "clusteredColumn3D",
    "stackedColumn3D",
    "percentColumn3D",
    "cylinder3D",
    "cone3D",
    "pyramid3D",
    "bar3D",
    "stackedBar3D",
    "percentBar3D",
    "line3D",
    "area3D",
    "stackedArea3D",
    "percentArea3D",
    "pie3D",
    "explodedPie3D",
    "surface3D",
    // Chartex layouts ([MS-ODRAWXML] 2.24), stored in a `cx:chartSpace` part.
    "waterfall",
    "treemap",
    "sunburst",
    "histogram",
    "pareto",
    "boxWhisker",
    "funnel",
];

/// Is the chart a chartex chart (`cx:chartSpace` part, frame in `mc:AlternateContent`)?
pub(crate) fn is_ex(c: &Chart) -> bool {
    c.plots.first().is_some_and(|p| p.kind.is_ex())
}

/// An exploded pie pushes its slices out by a quarter of the radius (§21.2.2.62 explosion).
fn explode(kind: &str, plot: &mut ChartPlot) {
    if kind.starts_with("explodedPie") {
        for s in &mut plot.series {
            s.explosion = 25;
        }
    }
}

/// Plot of a kind name (§21.2.2.16 barChart barDir, §21.2.2.97 lineChart, …).
fn plot_of(kind: &str, grouping: Option<String>) -> R<ChartPlot> {
    let (k, dir) = match kind {
        "column" => (ChartKind::Bar, Some("col")),
        "bar" => (ChartKind::Bar, Some("bar")),
        "line" => (ChartKind::Line, None),
        "area" => (ChartKind::Area, None),
        "pie" => (ChartKind::Pie, None),
        "doughnut" => (ChartKind::Doughnut, None),
        other if MORE_KINDS.contains(&other) => {
            return Ok(bo_sheet::chart::plot_for(other));
        }
        _ => return Err(EditError::Refused("badChart")),
    };
    // Bar groupings (§21.2.2.76) differ from line/area ones: keep only what fits.
    let grouping = grouping.filter(|g| match k {
        ChartKind::Bar => matches!(g.as_str(), "clustered" | "stacked" | "percentStacked"),
        ChartKind::Line | ChartKind::Area => {
            matches!(g.as_str(), "standard" | "stacked" | "percentStacked")
        }
        _ => false,
    });
    Ok(ChartPlot {
        kind: k,
        bar_dir: dir.map(str::to_owned),
        grouping,
        series: Vec::new(),
        ..ChartPlot::default()
    })
}

/// Plot groups of a kind name holding the grid's series (a volume stock chart: volume columns plus
/// a stock group on the secondary axis, `bo_sheet::chart::arrange`).
pub(crate) fn plots_of(kind: &str, grouping: Option<String>, d: &ChartData) -> R<Vec<ChartPlot>> {
    let mut plot = plot_of(kind, grouping)?;
    if matches!(kind, "stockVolume" | "stockVolumeUpDown") {
        return Ok(bo_sheet::chart::arrange(kind, series_of(d)));
    }
    plot.series = series_of(d);
    explode(kind, &mut plot);
    Ok(vec![plot])
}

pub(crate) fn data_of(c: &Chart) -> ChartData {
    let series: Vec<&ChartSeries> = c.plots.iter().flat_map(|p| &p.series).collect();
    let categories = series
        .iter()
        .map(|s| &s.categories)
        .max_by_key(|c| c.len())
        .cloned()
        .unwrap_or_default();
    // Histograms keep no categories: the points are numbered so that no value is dropped.
    let n = series.iter().map(|s| s.values.len()).max().unwrap_or(0);
    let mut categories = categories;
    for i in categories.len()..n {
        categories.push((i + 1).to_string());
    }
    ChartData {
        series: series
            .iter()
            .map(|s| SeriesData {
                name: s.name.clone().unwrap_or_default(),
                values: s.values.clone(),
            })
            .collect(),
        categories,
    }
}

/// Column letters of a zero-based column (`0` → `A`).
fn col_name(mut c: usize) -> String {
    let mut s = Vec::new();
    loop {
        s.push(b'A' + (c % 26) as u8);
        if c < 26 {
            break;
        }
        c = c / 26 - 1;
    }
    s.reverse();
    String::from_utf8(s).unwrap_or_default()
}

/// Normalise grid sizes (every series as long as the categories, limits).
pub(crate) fn clean(mut d: ChartData) -> R<ChartData> {
    if d.series.is_empty() || d.series.len() > MAX_SERIES || d.categories.len() > MAX_ROWS {
        return Err(EditError::Refused("badChartData"));
    }
    let n = d.categories.len().max(1);
    if d.categories.is_empty() {
        d.categories.push(String::new());
    }
    for s in &mut d.series {
        s.values.resize(n, None);
        for v in &mut s.values {
            if v.is_some_and(|x| !x.is_finite()) {
                *v = None;
            }
        }
    }
    Ok(d)
}

/// Series of the grid with references into the embedded sheet.
pub(crate) fn series_of(d: &ChartData) -> Vec<ChartSeries> {
    let last = d.categories.len() + 1;
    d.series
        .iter()
        .enumerate()
        .map(|(j, s)| {
            let col = col_name(j + 1);
            ChartSeries {
                name: Some(s.name.clone()),
                name_ref: Some(format!("{SHEET}!${col}$1")),
                cat_ref: Some(format!("{SHEET}!$A$2:$A${last}")),
                val_ref: Some(format!("{SHEET}!${col}$2:${col}${last}")),
                categories: d.categories.clone(),
                values: s.values.clone(),
                ..ChartSeries::default()
            }
        })
        .collect()
}

/// The embedded workbook holding the grid (`Sheet1`: categories in column A,
/// series names in row 1).
pub(crate) fn workbook(d: &ChartData) -> R<Vec<u8>> {
    let mut wb = Workbook::new();
    let mut ws = Worksheet::new(SHEET);
    for (i, c) in d.categories.iter().enumerate() {
        ws.set_cell(
            i as u32 + 1,
            0,
            bo_model::sheet::Cell::new(CellValue::Text(c.clone())),
        );
    }
    for (j, s) in d.series.iter().enumerate() {
        let col = j as u32 + 1;
        ws.set_cell(
            0,
            col,
            bo_model::sheet::Cell::new(CellValue::Text(s.name.clone())),
        );
        for (i, v) in s.values.iter().enumerate() {
            if let Some(v) = v {
                ws.set_cell(
                    i as u32 + 1,
                    col,
                    bo_model::sheet::Cell::new(CellValue::Number(*v)),
                );
            }
        }
    }
    wb.sheets = vec![ws];
    bo_xlsx::write_xlsx(&wb).map_err(|_| EditError::Refused("badChartData"))
}

/// Chart part XML with the §21.2.2.63 externalData reference to `rid`.
pub(crate) fn chart_xml(c: &Chart, rid: &str) -> Vec<u8> {
    if is_ex(c) {
        // [MS-ODRAWXML] chartex `cx:chartData/cx:externalData`.
        return bo_xlsx::chartex::chartex_space_xml_with(c, Some(rid)).into_bytes();
    }
    bo_xlsx::chart_write::chart_space_xml_with(c, Some(rid)).into_bytes()
}

/// Relationships of a part stored as a preserved `.rels` part.
fn read_rels(pres: &Presentation, part: &str) -> Vec<Relationship> {
    let name = bo_opc::rels_part_name(part);
    let Some(data) = pres.part_data(&name) else {
        return Vec::new();
    };
    let mut pkg = Package::new();
    pkg.put_part(&name, data.to_vec());
    pkg.relationships(part).unwrap_or_default()
}

fn rels_bytes(part: &str, rels: &[Relationship]) -> Vec<u8> {
    let mut pkg = Package::new();
    pkg.set_relationships(part, rels);
    pkg.part(&bo_opc::rels_part_name(part))
        .map(|p| p.data.clone())
        .unwrap_or_default()
}

/// Add or replace a preserved part.
pub(super) fn put_part(
    tx: &mut Tx<'_>,
    name: &str,
    content_type: Option<&str>,
    data: Vec<u8>,
) -> R<()> {
    let part = RawPart {
        name: name.to_owned(),
        content_type: content_type.map(str::to_owned),
        data,
    };
    let at = tx
        .pres
        .parts
        .iter()
        .position(|p| p.name.eq_ignore_ascii_case(name));
    let op = match at {
        Some(i) => {
            let old = tx.pres.parts[i].clone();
            if old.data == part.data && old.content_type == part.content_type {
                return Ok(());
            }
            super::ops::SlideOp::Parts {
                index: i,
                remove: vec![old],
                insert: vec![part],
            }
        }
        None => super::ops::SlideOp::Parts {
            index: tx.pres.parts.len(),
            remove: vec![],
            insert: vec![part],
        },
    };
    tx.op(op)?;
    Ok(())
}

/// Chart part of a graphic frame on a slide.
pub(super) fn chart_part_of(pres: &Presentation, slide: usize, n: &ShapeNode) -> Option<String> {
    let ShapeNode::Frame(f) = n else { return None };
    if f.uri != bo_drawingml::chart::URI_CHART && f.uri != bo_xlsx::chartex::CX_NS {
        return None;
    }
    let GraphicContent::Other(gd) = &f.content else {
        return None;
    };
    let rid = bo_drawingml::chart::chart_rel_id(gd)?;
    let s = pres.slides.get(slide)?;
    let rel = s.info.rels.iter().find(|r| r.id == rid && !r.external)?;
    Some(bo_opc::resolve_target(&s.info.part_name, &rel.target))
}

/// Is this node a chart frame?
pub fn is_chart(n: &ShapeNode) -> bool {
    matches!(n, ShapeNode::Frame(f)
        if f.uri == bo_drawingml::chart::URI_CHART || f.uri == bo_xlsx::chartex::CX_NS)
}

/// The chart of shape `id` on a slide.
pub fn chart_out(pres: &Presentation, slide: usize, id: u32) -> Option<ChartOut> {
    let placed = place_slide(pres, slide);
    let p = find(&placed, id)?;
    let n = node_at(pres, slide, &p.path)?;
    let part = chart_part_of(pres, slide, n)?;
    let c = bo_xlsx::chart_write::parse_chart_part(pres.part_data(&part)?, &part)?;
    let embedded = read_rels(pres, &part)
        .iter()
        .any(|r| r.rel_type == REL_PACKAGE);
    Some(ChartOut {
        id,
        kind: kind_name(&c).to_owned(),
        title: c.title.clone(),
        legend: c.legend.clone(),
        data: data_of(&c),
        embedded,
    })
}

/// Write chart `c` (its data `d`) to `part` and its embedded workbook.
fn store(tx: &mut Tx<'_>, part: &str, c: &Chart, d: &ChartData) -> R<()> {
    let mut rels = read_rels(tx.pres, part);
    let existing = rels
        .iter()
        .find(|r| r.rel_type == REL_PACKAGE && r.mode == TargetMode::Internal)
        .map(|r| bo_opc::resolve_target(part, &r.target));
    let book = if let Some(b) = existing {
        b
    } else {
        let name = build::unique_part(tx.pres, "/ppt/embeddings/workbook", "xlsx");
        let id = (1..=rels.len() + 1)
            .map(|n| format!("rId{n}"))
            .find(|id| rels.iter().all(|r| &r.id != id))
            .unwrap_or_default();
        rels.push(Relationship {
            id,
            rel_type: REL_PACKAGE.into(),
            target: format!(
                "../embeddings/{}",
                name.rsplit('/').next().unwrap_or_default()
            ),
            mode: TargetMode::Internal,
        });
        name
    };
    let rid = rels
        .iter()
        .find(|r| r.rel_type == REL_PACKAGE)
        .map(|r| r.id.clone())
        .unwrap_or_default();
    put_part(tx, &book, Some(CT_XLSX), workbook(d)?)?;
    put_part(
        tx,
        &bo_opc::rels_part_name(part),
        None,
        rels_bytes(part, &rels),
    )?;
    let ct = if is_ex(c) {
        bo_xlsx::chartex::CT_CHARTEX
    } else {
        bo_xlsx::chart_write::CT_CHART
    };
    put_part(tx, part, Some(ct), chart_xml(c, &rid))
}

pub(crate) fn default_data() -> ChartData {
    ChartData {
        categories: ["1", "2", "3", "4"].map(String::from).to_vec(),
        series: vec![
            SeriesData {
                name: "A".into(),
                values: vec![Some(4.3), Some(2.5), Some(3.5), Some(4.5)],
            },
            SeriesData {
                name: "B".into(),
                values: vec![Some(2.4), Some(4.4), Some(1.8), Some(2.8)],
            },
        ],
    }
}

/// A chart to insert.
pub struct NewChart<'a> {
    /// Chart type: `column` | `bar` | `line` | `area` | `pie` | `doughnut`.
    pub kind: &'a str,
    /// Chart title; `None` = no title.
    pub title: Option<String>,
    /// Category/series grid; `None` = the sample grid.
    pub data: Option<ChartData>,
    /// Legend position `r` | `l` | `t` | `b` | `tr` | `none`; `None` = bottom (right for pies).
    pub legend: Option<String>,
    /// Frame of the chart in points; `None` = the default frame.
    pub rect: Option<super::geom::Rect>,
    /// Chart template to format the new chart with.
    pub template: Option<bo_sheet::chart_style::ChartTemplate>,
    /// Titles the template adds.
    pub texts: bo_sheet::chart_style::LayoutTexts,
}

/// The theme accents of the deck (§20.1.6.2 clrScheme `accent1`…`accent6` of the first theme).
pub(crate) fn theme_accents(pres: &Presentation) -> [u32; 6] {
    accents_of(pres.themes.first())
}

/// The accents of a theme (§20.1.6.2 clrScheme `accent1`…`accent6`; BlueOffice's defaults for the
/// slots the theme does not give as RGB).
pub(crate) fn accents_of(theme: Option<&bo_model::slide::Theme>) -> [u32; 6] {
    let mut out = bo_sheet::chart_style::DEFAULT_ACCENTS;
    if let Some(t) = theme {
        for (i, slot) in out.iter_mut().enumerate() {
            if let Some(bo_model::slide::ColorKind::Srgb(v)) =
                t.scheme_color(&format!("accent{}", i + 1)).map(|c| &c.kind)
            {
                *slot = *v;
            }
        }
    }
    out
}

/// A slide-safe design spec: chart templates of types a slide chart part cannot hold (chartex)
/// keep the chart's type.
pub(crate) fn slide_spec(
    spec: &crate::chart_design::DesignSpec,
    c: &Chart,
) -> crate::chart_design::DesignSpec {
    let mut spec = spec.clone();
    if let Some(t) = spec.template.as_mut() {
        if bo_sheet::chart::plot_for(&t.kind).kind.is_ex() != is_ex(c) || t.kind == "combo" {
            bo_sheet::chart::type_name(c).clone_into(&mut t.kind);
        }
    }
    spec
}

/// Gallery of chart `id` on `slide` (previews drawn with the deck's theme accents).
pub fn chart_gallery(
    pres: &Presentation,
    slide: usize,
    id: u32,
    w: f64,
    h: f64,
) -> Option<serde_json::Value> {
    let placed = place_slide(pres, slide);
    let p = find(&placed, id)?;
    let n = node_at(pres, slide, &p.path)?;
    let part = chart_part_of(pres, slide, n)?;
    let c = bo_xlsx::chart_write::parse_chart_part(pres.part_data(&part)?, &part)?;
    Some(gallery_for(&c, &theme_accents(pres), w, h))
}

/// Gallery of chart `c` with previews drawn in the theme `accents` (shared with BlueDoc).
pub(crate) fn gallery_for(c: &Chart, accents: &[u32; 6], w: f64, h: f64) -> serde_json::Value {
    let color = |c: Option<&bo_model::sheet::ChartColor>, i: usize| -> bo_render::Rgba {
        use bo_model::sheet::ChartColor;
        let rgb = match c {
            Some(ChartColor::Rgb(v)) => *v,
            Some(ChartColor::Scheme(name)) => match name
                .strip_prefix("accent")
                .and_then(|k| k.parse::<usize>().ok())
            {
                Some(k) if (1..=6).contains(&k) => accents[k - 1],
                _ => return bo_sheet::chart::default_color(c, i),
            },
            None => accents[i % 6],
        };
        bo_render::Rgba((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8, 255)
    };
    crate::chart_design::gallery(c, accents, w, h, &color)
}

pub(crate) fn legend_of(l: &str) -> R<Option<String>> {
    match l {
        "none" => Ok(None),
        "r" | "l" | "t" | "b" | "tr" => Ok(Some(l.to_owned())),
        _ => Err(EditError::Refused("badChart")),
    }
}

/// A new chart of type `kind` with its data grid (the sample grid by default), legend (bottom,
/// right for pies, `none` = no legend), title and chart template; shared with BlueDoc.
pub(crate) fn new_chart(
    kind: &str,
    title: Option<String>,
    legend: Option<&str>,
    data: Option<ChartData>,
    template: Option<bo_sheet::chart_style::ChartTemplate>,
    texts: bo_sheet::chart_style::LayoutTexts,
    accents: &[u32; 6],
) -> R<(Chart, ChartData)> {
    let d = clean(data.unwrap_or_else(default_data))?;
    let plots = plots_of(kind, None, &d)?;
    let legend = match legend {
        Some(l) => legend_of(l)?,
        None => Some(
            if kind == "pie" || kind == "doughnut" {
                "r"
            } else {
                "b"
            }
            .into(),
        ),
    };
    let mut c = Chart {
        title: title.filter(|t| !t.is_empty()),
        plots,
        legend,
        ..Chart::default()
    };
    bo_sheet::chart::view_for_type(&mut c);
    if let Some(t) = template {
        let design = crate::chart_design::DesignSpec {
            template: Some(t),
            texts,
            ..crate::chart_design::DesignSpec::default()
        };
        let design = slide_spec(&design, &c);
        crate::chart_design::apply(&mut c, &design, accents)
            .map_err(|_| EditError::Refused("badChart"))?;
    }
    Ok((c, d))
}

/// Insert a chart on the current slide.
pub fn cmd_insert_chart(tx: &mut Tx<'_>, sel: &mut SlideSel, spec: NewChart<'_>) -> R<()> {
    let rect = spec.rect;
    let (c, d) = new_chart(
        spec.kind,
        spec.title,
        spec.legend.as_deref(),
        spec.data,
        spec.template,
        spec.texts,
        &theme_accents(tx.pres),
    )?;
    let slide = sel.slide;
    let (sw, sh) = slide_size(tx.pres);
    let r = rect.unwrap_or(super::geom::Rect {
        x: sw * 0.15,
        y: sh * 0.15,
        w: sw * 0.7,
        h: sh * 0.7,
    });
    let ex = is_ex(&c);
    let part = build::unique_part(
        tx.pres,
        if ex {
            "/ppt/charts/chartEx"
        } else {
            "/ppt/charts/chart"
        },
        "xml",
    );
    store(tx, &part, &c, &d)?;
    let s = tx
        .pres
        .slides
        .get(slide)
        .ok_or(EditError::Refused("noSlide"))?;
    let rid = (1..=s.info.rels.len() + 1)
        .map(|n| format!("rId{n}"))
        .find(|id| s.info.rels.iter().all(|r| &r.id != id))
        .unwrap_or_default();
    let id = next_shape_id(tx.pres, slide);
    let emu = |pt: f64| (pt * super::geom::EMU_PT).round() as i64;
    let xml = if ex {
        // [MS-ODRAWXML] 2.24: the frame in an mc:AlternateContent choice with a text fallback.
        bo_pptx::chartex_frame::new_frame_xml(
            id,
            &format!("Chart {id}"),
            (emu(r.x), emu(r.y), emu(r.w.max(1.0)), emu(r.h.max(1.0))),
            &rid,
            bo_xlsx::chartex::requires(c.plots.first().map(|p| &p.kind)),
            "Biểu đồ mở rộng (chartex): cần ứng dụng hỗ trợ để hiển thị.",
        )
    } else {
        format!(
            concat!(
                r#"<p:graphicFrame><p:nvGraphicFramePr><p:cNvPr id="{id}" name="Chart {id}"/><p:cNvGraphicFramePr/><p:nvPr/></p:nvGraphicFramePr>"#,
                r#"<p:xfrm><a:off x="{x}" y="{y}"/><a:ext cx="{cx}" cy="{cy}"/></p:xfrm>"#,
                r#"<a:graphic><a:graphicData uri="{uri}"><c:chart xmlns:c="{c}" xmlns:r="{r}" r:id="{rid}"/></a:graphicData></a:graphic></p:graphicFrame>"#
            ),
            id = id,
            x = emu(r.x),
            y = emu(r.y),
            cx = emu(r.w.max(1.0)),
            cy = emu(r.h.max(1.0)),
            uri = bo_drawingml::chart::URI_CHART,
            c = NS_C,
            r = NS_R,
            rid = rid,
        )
    };
    let target = format!("../charts/{}", part.rsplit('/').next().unwrap_or_default());
    let rel_type = if ex {
        bo_xlsx::chartex::REL_CHARTEX.to_owned()
    } else {
        format!("{}chart", bo_pptx::RELATIONSHIP_NS)
    };
    tx.fields(slide, |f| {
        f.rels.push(Rel {
            id: rid.clone(),
            rel_type: rel_type.clone(),
            target,
            external: false,
        });
    })?;
    insert_top(tx, sel, build::node(&xml))
}

/// Chart `c` changed by `spec` (type, title, legend, data, design) with its data grid; shared
/// with BlueDoc. A formatting-only change keeps the plots and their colours.
pub(crate) fn edit_chart(
    mut c: Chart,
    spec: &ChartSpec,
    accents: &[u32; 6],
) -> R<(Chart, ChartData)> {
    let d = clean(spec.data.clone().unwrap_or_else(|| data_of(&c)))?;
    // Formatting only (CORE-CHART-007): the chart keeps its plots and their own colours.
    if spec.kind.is_none() && spec.title.is_none() && spec.legend.is_none() && spec.data.is_none() {
        if spec.design.is_empty() {
            return Err(EditError::Refused("noChange"));
        }
        let design = slide_spec(&spec.design, &c);
        crate::chart_design::apply(&mut c, &design, accents)
            .map_err(|_| EditError::Refused("badChart"))?;
        // The data is rewritten into the embedded workbook: the references follow it.
        let refs = series_of(&d);
        for (s, r) in c.plots.iter_mut().flat_map(|p| &mut p.series).zip(refs) {
            s.name_ref = r.name_ref;
            s.cat_ref = r.cat_ref;
            s.val_ref = r.val_ref;
        }
        return Ok((c, d));
    }
    let kind = spec
        .kind
        .clone()
        .unwrap_or_else(|| kind_name(&c).to_owned());
    let kind = if kind == "other" {
        "column".to_owned()
    } else {
        kind
    };
    // A chart part holds one family: classic (c:) or chartex (cx:) types, not a change between them.
    if bo_sheet::chart::plot_for(&kind).kind.is_ex() != is_ex(&c) && kind_name(&c) != kind {
        return Err(EditError::Refused("badChart"));
    }
    let grouping = c.plots.first().and_then(|p| p.grouping.clone());
    c.plots = plots_of(&kind, grouping, &d)?;
    bo_sheet::chart::view_for_type(&mut c);
    if let Some(t) = &spec.title {
        c.title = Some(t.clone()).filter(|t| !t.is_empty());
    }
    if let Some(l) = &spec.legend {
        c.legend = legend_of(l)?;
    }
    if !spec.design.is_empty() {
        let design = slide_spec(&spec.design, &c);
        crate::chart_design::apply(&mut c, &design, accents)
            .map_err(|_| EditError::Refused("badChart"))?;
    }
    c.dirty = true;
    Ok((c, d))
}

/// Change the type, title, legend or data of chart `id` (default: the selected one).
pub fn cmd_set_chart(tx: &mut Tx<'_>, sel: &SlideSel, id: Option<u32>, spec: &ChartSpec) -> R<()> {
    let id = match (id, sel.ids.as_slice()) {
        (Some(i), _) => i,
        (None, [i]) => *i,
        _ => return Err(EditError::Refused("noChart")),
    };
    let placed = place_slide(tx.pres, sel.slide);
    let p = find(&placed, id).ok_or(EditError::Refused("noChart"))?;
    let n = node_at(tx.pres, sel.slide, &p.path).ok_or(EditError::Refused("noChart"))?;
    let part = chart_part_of(tx.pres, sel.slide, n).ok_or(EditError::Refused("noChart"))?;
    let data = tx
        .pres
        .part_data(&part)
        .ok_or(EditError::Refused("noChart"))?;
    let c =
        bo_xlsx::chart_write::parse_chart_part(data, &part).ok_or(EditError::Refused("noChart"))?;
    let (c, d) = edit_chart(c, spec, &theme_accents(tx.pres))?;
    store(tx, &part, &c, &d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn column_names() {
        assert_eq!(col_name(0), "A");
        assert_eq!(col_name(25), "Z");
        assert_eq!(col_name(26), "AA");
        assert_eq!(col_name(27), "AB");
        assert_eq!(col_name(701), "ZZ");
        assert_eq!(col_name(702), "AAA");
    }

    #[test]
    fn grid_is_normalised() {
        let d = clean(ChartData {
            categories: vec!["a".into(), "b".into()],
            series: vec![SeriesData {
                name: "s".into(),
                values: vec![Some(1.0), Some(f64::NAN), Some(3.0)],
            }],
        })
        .unwrap();
        assert_eq!(d.series[0].values, vec![Some(1.0), None]);
        assert!(clean(ChartData::default()).is_err());
        let s = series_of(&d);
        assert_eq!(s[0].val_ref.as_deref(), Some("Sheet1!$B$2:$B$3"));
        assert_eq!(s[0].cat_ref.as_deref(), Some("Sheet1!$A$2:$A$3"));
    }
}
