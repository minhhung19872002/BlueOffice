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
}

fn kind_name(c: &Chart) -> &'static str {
    let Some(p) = c.plots.first() else {
        return "column";
    };
    let g = p.grouping.as_deref();
    let horizontal = p.bar_dir.as_deref() == Some("bar");
    match p.kind {
        ChartKind::Bar => match (horizontal, g) {
            (false, Some("stacked")) => "stackedColumn",
            (false, Some("percentStacked")) => "percentColumn",
            (true, Some("stacked")) => "stackedBar",
            (true, Some("percentStacked")) => "percentBar",
            (true, _) => "bar",
            _ => "column",
        },
        ChartKind::Line => match g {
            Some("stacked") => "stackedLine",
            Some("percentStacked") => "percentLine",
            _ => "line",
        },
        ChartKind::Area => match g {
            Some("stacked") => "stackedArea",
            Some("percentStacked") => "percentArea",
            _ => "area",
        },
        ChartKind::Pie if p.series.iter().any(|s| s.explosion > 0) => "explodedPie",
        ChartKind::Pie => "pie",
        ChartKind::Doughnut => "doughnut",
        ChartKind::OfPie if p.style.as_deref() == Some("bar") => "barOfPie",
        ChartKind::OfPie => "pieOfPie",
        ChartKind::Radar => match p.style.as_deref() {
            Some("marker") => "radarMarker",
            Some("filled") => "radarFilled",
            _ => "radar",
        },
        ChartKind::Stock if p.flag => "stockUpDown",
        ChartKind::Stock => "stock",
        ChartKind::Surface => "surface",
        _ => "other",
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
    "surface",
];

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

fn data_of(c: &Chart) -> ChartData {
    let series: Vec<&ChartSeries> = c.plots.iter().flat_map(|p| &p.series).collect();
    let categories = series
        .iter()
        .map(|s| &s.categories)
        .max_by_key(|c| c.len())
        .cloned()
        .unwrap_or_default();
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
fn clean(mut d: ChartData) -> R<ChartData> {
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
fn series_of(d: &ChartData) -> Vec<ChartSeries> {
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
fn workbook(d: &ChartData) -> R<Vec<u8>> {
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
fn chart_xml(c: &Chart, rid: &str) -> Vec<u8> {
    let xml = bo_xlsx::chart_write::chart_space_xml(c);
    let ext = format!(
        r#"</c:chart><c:externalData r:id="{}"><c:autoUpdate val="0"/></c:externalData></c:chartSpace>"#,
        build::esc(rid)
    );
    xml.replacen("</c:chart></c:chartSpace>", &ext, 1)
        .into_bytes()
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
fn chart_part_of(pres: &Presentation, slide: usize, n: &ShapeNode) -> Option<String> {
    let ShapeNode::Frame(f) = n else { return None };
    if f.uri != bo_drawingml::chart::URI_CHART {
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
    matches!(n, ShapeNode::Frame(f) if f.uri == bo_drawingml::chart::URI_CHART)
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
    put_part(
        tx,
        part,
        Some(bo_xlsx::chart_write::CT_CHART),
        chart_xml(c, &rid),
    )
}

fn default_data() -> ChartData {
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
}

fn legend_of(l: &str) -> R<Option<String>> {
    match l {
        "none" => Ok(None),
        "r" | "l" | "t" | "b" | "tr" => Ok(Some(l.to_owned())),
        _ => Err(EditError::Refused("badChart")),
    }
}

/// Insert a chart on the current slide.
pub fn cmd_insert_chart(tx: &mut Tx<'_>, sel: &mut SlideSel, spec: NewChart<'_>) -> R<()> {
    let kind = spec.kind;
    let d = clean(spec.data.unwrap_or_else(default_data))?;
    let mut plot = plot_of(kind, None)?;
    plot.series = series_of(&d);
    let legend = match spec.legend.as_deref() {
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
    let rect = spec.rect;
    let c = Chart {
        title: spec.title.filter(|t| !t.is_empty()),
        plots: vec![plot],
        legend,
        ..Chart::default()
    };
    let slide = sel.slide;
    let (sw, sh) = slide_size(tx.pres);
    let r = rect.unwrap_or(super::geom::Rect {
        x: sw * 0.15,
        y: sh * 0.15,
        w: sw * 0.7,
        h: sh * 0.7,
    });
    let part = build::unique_part(tx.pres, "/ppt/charts/chart", "xml");
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
    let xml = format!(
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
    );
    let target = format!("../charts/{}", part.rsplit('/').next().unwrap_or_default());
    tx.fields(slide, |f| {
        f.rels.push(Rel {
            id: rid.clone(),
            rel_type: format!("{}chart", bo_pptx::RELATIONSHIP_NS),
            target,
            external: false,
        });
    })?;
    insert_top(tx, sel, build::node(&xml))
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
    let mut c =
        bo_xlsx::chart_write::parse_chart_part(data, &part).ok_or(EditError::Refused("noChart"))?;
    let d = clean(spec.data.clone().unwrap_or_else(|| data_of(&c)))?;
    let kind = spec
        .kind
        .clone()
        .unwrap_or_else(|| kind_name(&c).to_owned());
    let kind = if kind == "other" {
        "column".to_owned()
    } else {
        kind
    };
    let grouping = c.plots.first().and_then(|p| p.grouping.clone());
    let mut plot = plot_of(&kind, grouping)?;
    plot.series = series_of(&d);
    c.plots = vec![plot];
    if let Some(t) = &spec.title {
        c.title = Some(t.clone()).filter(|t| !t.is_empty());
    }
    if let Some(l) = &spec.legend {
        c.legend = legend_of(l)?;
    }
    c.dirty = true;
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
