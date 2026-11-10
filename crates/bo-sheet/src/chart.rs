//! Charts on worksheets: live data from the referenced ranges (falling back to
//! the cached values of the chart part, ECMA-376 Part 1, §21.2.2.120 `numCache`)
//! and drawing into the display list.
//!
//! Supported plot groups (§21.2.2 `c:plotArea` children): bar/column (clustered,
//! stacked, percent-stacked), line, area, pie, doughnut and scatter; several
//! groups in one chart are drawn as a combo on shared axes. The palette and the
//! layout are BlueSheet's own.

use crate::cells::{abs_ref, resolve_ref};
use crate::numfmt::Locale;
use crate::render::format_cell;
use bo_model::sheet::{
    CellRange, CellRef, CellValue, Chart, ChartColor, ChartKind, ChartPlot, ChartSeries, Workbook,
};
use bo_render::{Command, Paint, PathSeg, Rgba, Stroke};

mod look;
mod three_d;

use look::Look;

/// Series colours (BlueSheet palette).
pub const PALETTE: [Rgba; 8] = [
    Rgba(0x2E, 0x6D, 0xB4, 255),
    Rgba(0xF2, 0x8C, 0x28, 255),
    Rgba(0x2F, 0xA3, 0x6B, 255),
    Rgba(0xD6, 0x45, 0x45, 255),
    Rgba(0x7E, 0x57, 0xC2, 255),
    Rgba(0xC9, 0xA2, 0x27, 255),
    Rgba(0x1F, 0xA7, 0xA8, 255),
    Rgba(0x6B, 0x72, 0x80, 255),
];

const GRID: Rgba = Rgba(0xE1, 0xE4, 0xE8, 255);
const AXIS: Rgba = Rgba(0x8A, 0x90, 0x99, 255);
const TEXT: Rgba = Rgba(0x3C, 0x40, 0x43, 255);
const FRAME: Rgba = Rgba(0xC8, 0xCD, 0xD3, 255);

/// Maximum number of points read from a range.
const MAX_POINTS: usize = 4_000;

/// Text alignment of a label inside its box.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    /// Flush with the left edge of the box.
    Left,
    /// Centred horizontally in the box.
    Center,
    /// Flush with the right edge of the box.
    Right,
}

/// Draws `text` into the box `[l, t, r, b]` (points) with a font size.
pub type LabelFn<'a> = dyn FnMut(&str, [f64; 4], Align, f64, Rgba, &mut Vec<Command>) + 'a;

/// Values of a range, row-major (`None` for non-numeric cells).
fn range_cells(wb: &Workbook, sheet: usize, r: CellRange) -> Vec<Option<&bo_model::sheet::Cell>> {
    let Some(ws) = wb.sheets.get(sheet) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for row in r.first.row..=r.last.row {
        for col in r.first.col..=r.last.col {
            out.push(ws.cell(row, col));
            if out.len() >= MAX_POINTS {
                return out;
            }
        }
    }
    out
}

/// Live data of a series: (name, categories, values). References that do not
/// resolve keep the cached data of the chart part.
pub fn series_data(
    wb: &Workbook,
    sheet: usize,
    s: &ChartSeries,
    locale: &Locale,
) -> (Option<String>, Vec<String>, Vec<Option<f64>>) {
    let text = |c: Option<&bo_model::sheet::Cell>| {
        c.map(|c| format_cell(wb, c, locale).text)
            .unwrap_or_default()
    };
    let name = s
        .name_ref
        .as_deref()
        .and_then(|f| resolve_ref(wb, sheet, f))
        .map(|(si, r)| text(range_cells(wb, si, r).first().copied().flatten()))
        .or_else(|| s.name.clone());
    let cats = s
        .cat_ref
        .as_deref()
        .and_then(|f| resolve_ref(wb, sheet, f))
        .map_or_else(
            || s.categories.clone(),
            |(si, r)| range_cells(wb, si, r).into_iter().map(text).collect(),
        );
    let vals = s
        .val_ref
        .as_deref()
        .and_then(|f| resolve_ref(wb, sheet, f))
        .map_or_else(
            || s.values.clone(),
            |(si, r)| {
                range_cells(wb, si, r)
                    .into_iter()
                    .map(|c| match c.map(|c| &c.value) {
                        Some(CellValue::Number(n)) => Some(*n),
                        _ => None,
                    })
                    .collect()
            },
        );
    (name, cats, vals)
}

/// Refreshes the cached values of every chart from its ranges (before saving).
pub fn refresh_caches(wb: &mut Workbook, locale: &Locale) {
    for si in 0..wb.sheets.len() {
        let updated: Vec<Chart> = wb.sheets[si]
            .charts
            .iter()
            .map(|c| {
                let mut c = c.clone();
                for p in &mut c.plots {
                    for s in &mut p.series {
                        let (name, cats, vals) = series_data(wb, si, s, locale);
                        s.name = name;
                        s.categories = cats;
                        s.values = vals;
                    }
                }
                c
            })
            .collect();
        wb.sheets[si].charts = updated;
    }
}

/// The chart type names accepted by [`new_chart`].
pub const CHART_TYPES: &[&str] = &[
    "column",
    "stackedColumn",
    "percentColumn",
    "bar",
    "stackedBar",
    "percentBar",
    "line",
    "stackedLine",
    "percentLine",
    "area",
    "stackedArea",
    "percentArea",
    "pie",
    "explodedPie",
    "doughnut",
    "pieOfPie",
    "barOfPie",
    "scatter",
    "bubble",
    "radar",
    "radarMarker",
    "radarFilled",
    "stock",
    "stockUpDown",
    "surface",
    "surfaceWireframe",
    "waterfall",
    "treemap",
    "sunburst",
    "histogram",
    "pareto",
    "boxWhisker",
    "funnel",
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
    "surfaceWireframe3D",
    "combo",
];

/// The [`CHART_TYPES`] name of a chart (several groups: `combo`).
#[must_use]
pub fn type_name(c: &Chart) -> &'static str {
    if c.plots.len() > 1 {
        return "combo";
    }
    let Some(p) = c.plots.first() else {
        return "column";
    };
    let g = p.grouping.as_deref();
    let horizontal = p.bar_dir.as_deref() == Some("bar");
    // 3-D groups (§21.2.2.15 bar3DChart, …; bar shape §21.2.2.177).
    if p.three_d {
        return match p.kind {
            ChartKind::Bar => match (horizontal, g, p.style.as_deref()) {
                (true, Some("stacked"), _) => "stackedBar3D",
                (true, Some("percentStacked"), _) => "percentBar3D",
                (true, _, _) => "bar3D",
                (false, Some("stacked"), _) => "stackedColumn3D",
                (false, Some("percentStacked"), _) => "percentColumn3D",
                (false, _, Some("cylinder")) => "cylinder3D",
                (false, _, Some("cone" | "coneToMax")) => "cone3D",
                (false, _, Some("pyramid" | "pyramidToMax")) => "pyramid3D",
                (false, Some("standard"), _) => "column3D",
                _ => "clusteredColumn3D",
            },
            ChartKind::Line => "line3D",
            ChartKind::Area => match g {
                Some("stacked") => "stackedArea3D",
                Some("percentStacked") => "percentArea3D",
                _ => "area3D",
            },
            ChartKind::Pie if p.series.iter().any(|s| s.explosion > 0) => "explodedPie3D",
            ChartKind::Pie => "pie3D",
            ChartKind::Surface if p.flag => "surfaceWireframe3D",
            ChartKind::Surface => "surface3D",
            _ => "column",
        };
    }
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
        ChartKind::Surface if p.flag => "surfaceWireframe",
        ChartKind::Surface => "surface",
        ChartKind::Scatter => "scatter",
        ChartKind::Bubble => "bubble",
        ChartKind::Waterfall => "waterfall",
        ChartKind::Treemap => "treemap",
        ChartKind::Sunburst => "sunburst",
        ChartKind::Histogram => "histogram",
        ChartKind::Pareto => "pareto",
        ChartKind::BoxWhisker => "boxWhisker",
        ChartKind::Funnel => "funnel",
        ChartKind::Other(_) => "column",
    }
}

/// The plot of a chart type name of [`CHART_TYPES`] (unknown names: clustered columns).
#[must_use]
pub fn plot_for(kind: &str) -> ChartPlot {
    // 3-D types: `column3D` stands the series one behind the other (§21.2.2.76 grouping standard),
    // `clusteredColumn3D` and the shapes (§21.2.2.177) cluster them; the others are their 2-D
    // type drawn in 3-D.
    if let Some(base) = kind.strip_suffix("3D") {
        let (base, standard, shape) = match base {
            "column" => ("column", true, None),
            "clusteredColumn" => ("column", false, None),
            "cylinder" | "cone" | "pyramid" => ("column", false, Some(base)),
            other => (other, false, None),
        };
        let mut p = plot_for(base);
        p.three_d = true;
        if standard {
            p.grouping = Some("standard".into());
        }
        if let Some(shape) = shape {
            p.style = Some(shape.into());
        }
        return p;
    }
    let (k, dir, grouping, style, flag) = match kind {
        "bar" => (ChartKind::Bar, Some("bar"), Some("clustered"), None, false),
        "stackedColumn" => (ChartKind::Bar, Some("col"), Some("stacked"), None, false),
        "percentColumn" => (
            ChartKind::Bar,
            Some("col"),
            Some("percentStacked"),
            None,
            false,
        ),
        "stackedBar" => (ChartKind::Bar, Some("bar"), Some("stacked"), None, false),
        "percentBar" => (
            ChartKind::Bar,
            Some("bar"),
            Some("percentStacked"),
            None,
            false,
        ),
        "line" => (ChartKind::Line, None, Some("standard"), None, false),
        "stackedLine" => (ChartKind::Line, None, Some("stacked"), None, false),
        "percentLine" => (ChartKind::Line, None, Some("percentStacked"), None, false),
        "area" => (ChartKind::Area, None, Some("standard"), None, false),
        "stackedArea" => (ChartKind::Area, None, Some("stacked"), None, false),
        "percentArea" => (ChartKind::Area, None, Some("percentStacked"), None, false),
        "pie" | "explodedPie" => (ChartKind::Pie, None, None, None, false),
        "doughnut" => (ChartKind::Doughnut, None, None, None, false),
        "pieOfPie" => (ChartKind::OfPie, None, None, Some("pie"), false),
        "barOfPie" => (ChartKind::OfPie, None, None, Some("bar"), false),
        "scatter" => (ChartKind::Scatter, None, None, None, false),
        "bubble" => (ChartKind::Bubble, None, None, None, false),
        "radar" => (ChartKind::Radar, None, None, Some("standard"), false),
        "radarMarker" => (ChartKind::Radar, None, None, Some("marker"), false),
        "radarFilled" => (ChartKind::Radar, None, None, Some("filled"), false),
        "stock" => (ChartKind::Stock, None, None, None, false),
        "stockUpDown" => (ChartKind::Stock, None, None, None, true),
        "surface" => (ChartKind::Surface, None, None, None, false),
        "surfaceWireframe" => (ChartKind::Surface, None, None, None, true),
        "waterfall" => (ChartKind::Waterfall, None, None, None, false),
        "treemap" => (ChartKind::Treemap, None, None, None, false),
        "sunburst" => (ChartKind::Sunburst, None, None, None, false),
        "histogram" => (ChartKind::Histogram, None, None, None, false),
        "pareto" => (ChartKind::Pareto, None, None, None, false),
        "boxWhisker" => (ChartKind::BoxWhisker, None, None, Some("exclusive"), false),
        "funnel" => (ChartKind::Funnel, None, None, None, false),
        _ => (ChartKind::Bar, Some("col"), Some("clustered"), None, false),
    };
    ChartPlot {
        vary_colors: matches!(k, ChartKind::Pie | ChartKind::Doughnut | ChartKind::OfPie),
        kind: k,
        bar_dir: dir.map(str::to_owned),
        grouping: grouping.map(str::to_owned),
        style: style.map(str::to_owned),
        flag,
        series: Vec::new(),
        ..ChartPlot::default()
    }
}

/// Series of a new chart of `kind`: bubble series from consecutive (value, size) series, stock charts
/// at most four series (open, high, low, close), an exploded pie's slices pushed out by a quarter of
/// the radius.
fn shape_series(kind: &str, plot: &mut ChartPlot) {
    match kind {
        "bubble" => {
            let all = std::mem::take(&mut plot.series);
            let mut it = all.into_iter();
            while let Some(mut y) = it.next() {
                if let Some(size) = it.next() {
                    y.size_ref = size.val_ref;
                    y.sizes = size.values;
                }
                plot.series.push(y);
            }
        }
        "stock" | "stockUpDown" => plot.series.truncate(4),
        // One series: waterfall, funnel, pareto, histogram, treemap and sunburst.
        "waterfall" | "funnel" | "pareto" | "histogram" | "treemap" | "sunburst" => {
            plot.series.truncate(1);
        }
        "explodedPie" => {
            for s in &mut plot.series {
                s.explosion = 25;
            }
        }
        _ => {}
    }
}

/// Changes the chart type, keeping its series (combo: first series as
/// columns, the others as lines).
pub fn set_chart_type(chart: &mut Chart, kind: &str) {
    let series: Vec<ChartSeries> = chart.plots.drain(..).flat_map(|p| p.series).collect();
    if kind == "combo" && series.len() > 1 {
        let mut bars = plot_for("column");
        let mut lines = plot_for("line");
        let mut it = series.into_iter();
        bars.series.extend(it.next());
        lines.series.extend(it);
        chart.plots = vec![bars, lines];
    } else {
        let mut p = plot_for(if kind == "combo" { "column" } else { kind });
        p.series = series;
        // A type change never drops data: only the exploded pie look is applied.
        if matches!(kind, "explodedPie" | "explodedPie3D") {
            shape_series("explodedPie", &mut p);
        }
        chart.plots = vec![p];
    }
    view_for_type(chart);
    chart.dirty = true;
}

/// A 3-D chart gets the view of a new 3-D chart (§21.2.2.228 view3D) unless it has one; a flat chart
/// drops it.
pub fn view_for_type(chart: &mut Chart) {
    let pie = chart.plots.iter().all(|p| p.kind == ChartKind::Pie);
    if chart.plots.iter().any(|p| p.three_d) {
        if chart.axes.view3d.is_none() {
            chart.axes.view3d = Some(bo_model::sheet::View3D::new_chart(pie));
        }
    } else {
        chart.axes.view3d = None;
    }
}

/// A chart for the data in `range` of sheet `sheet`: the first row holds series
/// names and the first column categories when they are text (columns are series).
pub fn new_chart(wb: &Workbook, sheet: usize, range: CellRange, kind: &str) -> Option<Chart> {
    let ws = wb.sheets.get(sheet)?;
    let is_text = |r: u32, c: u32| {
        ws.cell(r, c)
            .is_some_and(|c| matches!(c.value, CellValue::Text(_) | CellValue::RichText(_)))
    };
    let header =
        range.rows() > 1 && (range.first.col..=range.last.col).any(|c| is_text(range.first.row, c));
    // Text or dates in the first column are the categories (a date axis for dates).
    let cat_col = range.cols() > 1
        && (range.first.row + u32::from(header)..=range.last.row)
            .all(|r| crate::chart_recommend::is_label(wb, ws, r, range.first.col));
    let r0 = range.first.row + u32::from(header);
    // Treemap / sunburst: every leading text column is a level of the categories.
    let text_cols = if matches!(kind, "treemap" | "sunburst") && cat_col {
        (range.first.col..=range.last.col)
            .take_while(|&c| {
                (r0..=range.last.row).all(|r| is_text(r, c) || ws.cell(r, c).is_none())
            })
            .count() as u32
    } else {
        u32::from(cat_col)
    };
    let text_cols = text_cols.min(range.cols().saturating_sub(1));
    let c0 = range.first.col + text_cols.max(u32::from(cat_col));
    if r0 > range.last.row || c0 > range.last.col {
        return None;
    }
    let name = &ws.name;
    let cat_ref = cat_col.then(|| {
        abs_ref(
            name,
            CellRange::new(
                CellRef::new(r0, range.first.col),
                CellRef::new(range.last.row, range.first.col + text_cols.max(1) - 1),
            ),
        )
    });
    let mut plot = plot_for(if kind == "combo" { "column" } else { kind });
    let first_plot_kind = kind;
    for c in c0..=range.last.col {
        plot.series.push(ChartSeries {
            name_ref: header
                .then(|| abs_ref(name, CellRange::single(CellRef::new(range.first.row, c)))),
            cat_ref: cat_ref.clone(),
            val_ref: Some(abs_ref(
                name,
                CellRange::new(CellRef::new(r0, c), CellRef::new(range.last.row, c)),
            )),
            ..ChartSeries::default()
        });
    }
    // Bubble data: X in the first column (when it holds numbers), then (Y, size) pairs;
    // scatter data: X in the first column, one Y series per other column.
    let xy = match first_plot_kind {
        "bubble" => plot.series.len() >= 3,
        "scatter" => plot.series.len() >= 2,
        _ => false,
    };
    if xy && !cat_col {
        let x = plot.series.remove(0);
        for s in &mut plot.series {
            s.cat_ref.clone_from(&x.val_ref);
        }
    }
    shape_series(
        first_plot_kind
            .strip_suffix("3D")
            .unwrap_or(first_plot_kind),
        &mut plot,
    );
    let mut chart = Chart {
        from: CellRef::new(range.first.row, range.last.col + 2),
        to: Some(CellRef::new(range.first.row + 15, range.last.col + 10)),
        plots: vec![plot],
        legend: Some("r".into()),
        dirty: true,
        ..Chart::default()
    };
    if kind == "combo" {
        set_chart_type(&mut chart, "combo");
    }
    view_for_type(&mut chart);
    let mut data = chart.clone();
    refresh_one(wb, sheet, &mut data);
    Some(data)
}

// ---------------------------------------------------------------------------
// Pivot charts (§21.2.2.142 `c:pivotSource`)
// ---------------------------------------------------------------------------

/// `c:pivotSource/c:name` of pivot `name` on sheet `sheet_name`.
pub fn pivot_source_name(sheet_name: &str, name: &str) -> String {
    format!("{}!{name}", crate::cells::quote_sheet(sheet_name))
}

/// Text of a report cell (number formats applied).
fn cell_text(wb: &Workbook, sheet: usize, row: u32, col: u32) -> String {
    wb.sheets
        .get(sheet)
        .and_then(|ws| ws.cell(row, col))
        .map(|c| format_cell(wb, c, &Locale::default()).text)
        .unwrap_or_default()
}

/// The series of a pivot chart from the report of pivot `index` of `sheet`
/// (§18.10.1.73: `location`, `firstDataRow` / `firstDataCol`, `rowItems` /
/// `colItems`): categories are the innermost row items (outer captions joined
/// with " / "), one series per innermost column item and value field;
/// subtotals and grand totals are left out. Ranges are referenced when the
/// points are contiguous cells, otherwise the data are literals.
pub fn pivot_series(wb: &Workbook, sheet: usize, index: usize) -> Option<Vec<ChartSeries>> {
    let ws = wb.sheets.get(sheet)?;
    let pt = ws.pivots.get(index)?;
    let cache = wb.pivot_caches.iter().find(|c| c.cache_id == pt.cache_id)?;
    let r_fields = pt.rows.len();
    let c_fields = pt.cols.len();
    let k = pt.values.len();
    let dor = usize::from(pt.data_on_rows && k > 1);
    let top = pt.location.first.row + pt.first_data_row;
    let left = pt.location.first.col + pt.first_data_col;
    let label_col = |l: usize| {
        if pt.layout == bo_model::sheet::PivotLayout::Compact {
            pt.location.first.col
        } else {
            pt.location.first.col + l as u32
        }
    };
    // Categories: (sheet row, caption, value index) of the innermost lines.
    let mut cur: Vec<String> = vec![String::new(); r_fields];
    let mut cats: Vec<(u32, String, usize)> = Vec::new();
    for (i, it) in pt.row_items.iter().enumerate() {
        let row = top + i as u32;
        let depth = it.items.len().min(r_fields);
        if it.kind.is_empty() {
            if pt.layout == bo_model::sheet::PivotLayout::Compact {
                if it.items.len() <= r_fields && depth > 0 {
                    let t = cell_text(wb, sheet, row, label_col(0));
                    if !t.is_empty() {
                        cur[depth - 1] = t;
                    }
                }
            } else {
                for (l, slot) in cur.iter_mut().enumerate().take(depth) {
                    let t = cell_text(wb, sheet, row, label_col(l));
                    if !t.is_empty() {
                        *slot = t;
                    }
                }
            }
        }
        let leaf = it.kind.is_empty() && it.items.len() == r_fields + dor;
        if leaf {
            let caption = if r_fields == 0 {
                String::new()
            } else {
                cur.join(" / ")
            };
            let vi = if dor == 1 {
                it.items.last().copied().unwrap_or(0) as usize
            } else {
                0
            };
            cats.push((row, caption, vi));
        }
    }
    let field_caption = |f: u32, x: u32| -> String {
        cache
            .fields
            .get(f as usize)
            .and_then(|fl| crate::pivot::axis_items(fl).get(x as usize))
            .map(crate::pivot::value_text)
            .unwrap_or_default()
    };
    let value_names: Vec<String> = pt
        .values
        .iter()
        .map(|d| crate::pivot::value_name(cache, d))
        .collect();
    // Columns: (sheet col, caption, value index) of the innermost entries.
    let mut cols: Vec<(u32, String, usize)> = Vec::new();
    for (j, it) in pt.col_items.iter().enumerate() {
        if !it.kind.is_empty() {
            continue;
        }
        let col = left + j as u32;
        let mut parts: Vec<String> = it
            .items
            .iter()
            .take(c_fields)
            .enumerate()
            .map(|(l, x)| field_caption(pt.cols[l], *x))
            .collect();
        let vi = if dor == 0 && k > 1 {
            it.items.get(c_fields).copied().unwrap_or(0) as usize
        } else {
            0
        };
        if dor == 0 && (k > 1 || c_fields == 0) {
            parts.push(value_names.get(vi).cloned().unwrap_or_default());
        }
        cols.push((col, parts.join(" - "), vi));
    }
    let contiguous = |rows: &[u32]| rows.windows(2).all(|w| w[1] == w[0] + 1);
    let name = &ws.name;
    let mut out = Vec::new();
    let value_sets: Vec<usize> = if dor == 1 { (0..k).collect() } else { vec![0] };
    for (col, caption, _) in &cols {
        for &vi in &value_sets {
            let points: Vec<&(u32, String, usize)> = cats
                .iter()
                .filter(|(_, _, v)| dor == 0 || *v == vi)
                .collect();
            if points.is_empty() {
                continue;
            }
            let rows: Vec<u32> = points.iter().map(|p| p.0).collect();
            let joined = contiguous(&rows) && r_fields <= 1;
            let range = |c: u32| {
                CellRange::new(
                    CellRef::new(rows[0], c),
                    CellRef::new(rows[rows.len() - 1], c),
                )
            };
            let values: Vec<Option<f64>> = rows
                .iter()
                .map(|r| match ws.cell(*r, *col).map(|c| &c.value) {
                    Some(CellValue::Number(n)) => Some(*n),
                    _ => None,
                })
                .collect();
            let mut series_name = caption.clone();
            if dor == 1 {
                let v = value_names.get(vi).cloned().unwrap_or_default();
                series_name = if c_fields == 0 {
                    v
                } else {
                    format!("{caption} - {v}")
                };
            }
            out.push(ChartSeries {
                name: Some(series_name),
                name_ref: None,
                cat_ref: (joined && r_fields == 1).then(|| abs_ref(name, range(label_col(0)))),
                val_ref: joined.then(|| abs_ref(name, range(*col))),
                categories: points.iter().map(|p| p.1.clone()).collect(),
                values,
                ..ChartSeries::default()
            });
        }
    }
    Some(out)
}

/// Re-point every chart plotting pivot `name` of sheet `sheet` at its current
/// report (after a refresh, filter or layout change). Charts read from a file
/// get their chart part regenerated.
pub fn sync_pivot_charts(wb: &mut Workbook, sheet: usize) {
    let Some(ws) = wb.sheets.get(sheet) else {
        return;
    };
    let sheet_name = ws.name.to_lowercase();
    let names: Vec<(usize, String)> = ws
        .pivots
        .iter()
        .enumerate()
        .map(|(i, p)| (i, p.name.to_lowercase()))
        .collect();
    for si in 0..wb.sheets.len() {
        for ci in 0..wb.sheets[si].charts.len() {
            let Some((s, n)) = wb.sheets[si].charts[ci].pivot_ref() else {
                continue;
            };
            if s.to_lowercase() != sheet_name {
                continue;
            }
            let Some((pi, _)) = names.iter().find(|(_, x)| *x == n.to_lowercase()) else {
                continue;
            };
            let Some(series) = pivot_series(wb, sheet, *pi) else {
                continue;
            };
            let chart = &mut wb.sheets[si].charts[ci];
            let kept: Vec<ChartPlot> = chart.plots.clone();
            // One plot (or the first two of a combo) keep their type.
            let mut plots = if kept.is_empty() {
                vec![plot_for("column")]
            } else {
                kept
            };
            for p in &mut plots {
                p.series.clear();
            }
            if plots.len() > 1 {
                let mut it = series.into_iter();
                plots[0].series.extend(it.next());
                plots[1].series.extend(it);
                plots.truncate(2);
            } else {
                plots[0].series = series;
            }
            chart.plots = plots;
            chart.dirty = true;
        }
    }
}

/// A new pivot chart plotting pivot `index` of `sheet` (`kind` as for
/// [`new_chart`]), placed to the right of the report.
pub fn new_pivot_chart(wb: &Workbook, sheet: usize, index: usize, kind: &str) -> Option<Chart> {
    let ws = wb.sheets.get(sheet)?;
    let pt = ws.pivots.get(index)?;
    let area = crate::pivot::occupied(pt);
    let mut chart = Chart {
        from: CellRef::new(area.first.row, area.last.col + 2),
        to: Some(CellRef::new(area.first.row + 15, area.last.col + 10)),
        plots: vec![plot_for(if kind == "combo" { "column" } else { kind })],
        legend: Some("r".into()),
        dirty: true,
        pivot_source: Some(pivot_source_name(&ws.name, &pt.name)),
        ..Chart::default()
    };
    if kind == "combo" {
        let mut bars = plot_for("column");
        let lines = plot_for("line");
        bars.series.clear();
        chart.plots = vec![bars, lines];
    }
    chart.plots[0].series = pivot_series(wb, sheet, index)?;
    if kind == "combo" {
        let series: Vec<ChartSeries> = chart
            .plots
            .iter_mut()
            .flat_map(|p| p.series.drain(..))
            .collect();
        let mut it = series.into_iter();
        chart.plots[0].series.extend(it.next());
        chart.plots[1].series.extend(it);
    }
    Some(chart)
}

/// Cells covered by a one-cell or absolute anchor (§20.5.2.24 / §20.5.2.1):
/// `from` / `to` from `pos` / `ext` (EMU, 12 700 per point), the corners
/// snapped to the nearest cell edge.
pub fn resolve_anchor(chart: &mut Chart, g: &crate::geometry::SheetGeometry) {
    use bo_model::sheet::ChartAnchor;
    let pt = |emu: i64| emu as f64 / 12_700.0;
    let nearest_col = |x: f64| {
        let c = g.hit_test(x.max(0.0), 0.0).col;
        if x - g.col_x(c) > g.col_width(c) / 2.0 {
            c + 1
        } else {
            c
        }
    };
    let nearest_row = |y: f64| {
        let r = g.hit_test(0.0, y.max(0.0)).row;
        if y - g.row_y(r) > g.row_height(r) / 2.0 {
            r + 1
        } else {
            r
        }
    };
    let (x0, y0) = match (chart.anchor, chart.pos) {
        (ChartAnchor::Absolute, Some((x, y))) => {
            let (x, y) = (pt(x), pt(y));
            let c = g.hit_test(x.max(0.0), y.max(0.0));
            chart.from = c;
            (x, y)
        }
        (ChartAnchor::OneCell, _) => (g.col_x(chart.from.col), g.row_y(chart.from.row)),
        _ => return,
    };
    if let Some((cx, cy)) = chart.ext {
        let to = CellRef::new(
            nearest_row(y0 + pt(cy)).max(chart.from.row + 1),
            nearest_col(x0 + pt(cx)).max(chart.from.col + 1),
        );
        chart.to = Some(to);
    }
}

/// The pivot table a chart plots: `(sheet, index)`.
pub fn chart_pivot(wb: &Workbook, chart: &Chart) -> Option<(usize, usize)> {
    let (s, n) = chart.pivot_ref()?;
    let si = wb
        .sheets
        .iter()
        .position(|w| w.name.to_lowercase() == s.to_lowercase())?;
    let pi = wb.sheets[si]
        .pivots
        .iter()
        .position(|p| p.name.to_lowercase() == n.to_lowercase())?;
    Some((si, pi))
}

fn refresh_one(wb: &Workbook, sheet: usize, chart: &mut Chart) {
    for p in &mut chart.plots {
        let hierarchical = matches!(p.kind, ChartKind::Treemap | ChartKind::Sunburst);
        for s in &mut p.series {
            let (name, cats, vals) = series_data(wb, sheet, s, &Locale::EN_US);
            s.name = name;
            s.values = vals;
            // A category range of several columns holds the levels (outer column first).
            let levels = s
                .cat_ref
                .as_deref()
                .and_then(|f| resolve_ref(wb, sheet, f))
                .filter(|(_, r)| r.cols() > 1)
                .map(|(si, r)| {
                    (r.first.col..=r.last.col)
                        .rev()
                        .map(|c| {
                            (r.first.row..=r.last.row)
                                .map(|row| {
                                    wb.sheets[si]
                                        .cell(row, c)
                                        .map(|cell| format_cell(wb, cell, &Locale::EN_US).text)
                                        .unwrap_or_default()
                                })
                                .collect::<Vec<String>>()
                        })
                        .collect::<Vec<_>>()
                });
            match levels {
                Some(l) if hierarchical => {
                    s.categories = l.first().cloned().unwrap_or_default();
                    s.levels = l;
                }
                _ => s.categories = cats,
            }
        }
    }
}

fn rect(x: f64, y: f64, w: f64, h: f64, c: Rgba) -> Command {
    Command::FillRect {
        x: x as f32,
        y: y as f32,
        w: w.max(0.0) as f32,
        h: h.max(0.0) as f32,
        color: c,
    }
}

fn polyline(pts: &[(f64, f64)], c: Rgba, width: f64) -> Command {
    Command::Path {
        segs: pts
            .iter()
            .enumerate()
            .map(|(i, (x, y))| {
                if i == 0 {
                    PathSeg::M(*x as f32, *y as f32)
                } else {
                    PathSeg::L(*x as f32, *y as f32)
                }
            })
            .collect(),
        fill: None,
        stroke: Some(Stroke {
            width: width as f32,
            color: c,
            dash: vec![],
            cap: "round".into(),
            join: "round".into(),
        }),
    }
}

fn hline(x1: f64, x2: f64, y: f64, c: Rgba, w: f64) -> Command {
    Command::Line {
        x1: x1 as f32,
        y1: y as f32,
        x2: x2 as f32,
        y2: y as f32,
        width: w as f32,
        color: c,
        dash: None,
    }
}

fn vline(x: f64, y1: f64, y2: f64, c: Rgba, w: f64) -> Command {
    Command::Line {
        x1: x as f32,
        y1: y1 as f32,
        x2: x as f32,
        y2: y2 as f32,
        width: w as f32,
        color: c,
        dash: None,
    }
}

/// "Nice" axis step (1, 2, 5 × 10ⁿ).
fn nice_step(range: f64, ticks: f64) -> f64 {
    if range <= 0.0 || !range.is_finite() {
        return 1.0;
    }
    let raw = range / ticks;
    let mag = 10f64.powf(raw.log10().floor());
    let norm = raw / mag;
    let k = if norm <= 1.0 {
        1.0
    } else if norm <= 2.0 {
        2.0
    } else if norm <= 5.0 {
        5.0
    } else {
        10.0
    };
    k * mag
}

fn fmt_num(v: f64, locale: &Locale) -> String {
    let s = if (v - v.round()).abs() < 1e-9 {
        format!("{}", v.round() as i64)
    } else {
        let s = format!("{v:.2}");
        s.trim_end_matches('0').trim_end_matches('.').to_owned()
    };
    if locale.decimal == ',' {
        s.replace('.', ",")
    } else {
        s
    }
}

/// Data of one plotted series.
struct Ser {
    name: String,
    cats: Vec<String>,
    vals: Vec<Option<f64>>,
    color: Rgba,
    /// Colour of each point (pies, varied colours, `c:dPt`).
    pts: Vec<Rgba>,
    /// Bubble sizes.
    sizes: Vec<Option<f64>>,
    /// Pie explosion (percent of the radius).
    explosion: u32,
    /// Hierarchical labels (innermost level first).
    levels: Vec<Vec<String>>,
    /// Waterfall totals.
    subtotals: Vec<usize>,
    /// Trend lines (§21.2.2.211).
    trendlines: Vec<bo_model::sheet::Trendline>,
    /// Error bars (§21.2.2.58).
    err_bars: Option<bo_model::sheet::ErrorBars>,
}

/// Resolves chart colours: a colour of the file (`Some`) or the default colour of series / point `i`
/// (`None`). Hosts with a theme resolve `ChartColor::Scheme` through it.
pub type ColorFn<'a> = dyn Fn(Option<&ChartColor>, usize) -> Rgba + 'a;

/// BlueSheet's colours: sRGB as is, theme accents `accentN` and the defaults from [`PALETTE`].
pub fn default_color(c: Option<&ChartColor>, i: usize) -> Rgba {
    match c {
        Some(ChartColor::Rgb(v)) => Rgba((v >> 16) as u8, (v >> 8) as u8, *v as u8, 255),
        Some(ChartColor::Scheme(name)) => match name
            .strip_prefix("accent")
            .and_then(|n| n.parse::<usize>().ok())
        {
            Some(n) if n >= 1 => PALETTE[(n - 1) % PALETTE.len()],
            _ if matches!(name.as_str(), "bg1" | "lt1") => Rgba(255, 255, 255, 255),
            _ => TEXT,
        },
        None => PALETTE[i % PALETTE.len()],
    }
}

/// Is a plot drawn as a pie (one series, a wedge per point)?
fn pie_like(k: &ChartKind) -> bool {
    matches!(k, ChartKind::Pie | ChartKind::Doughnut | ChartKind::OfPie)
}

/// Draws `chart` into a `w × h` box at the origin (points) with BlueSheet's colours.
#[allow(clippy::too_many_arguments)]
pub fn draw_chart(
    wb: &Workbook,
    sheet: usize,
    chart: &Chart,
    w: f64,
    h: f64,
    locale: &Locale,
    label: &mut LabelFn<'_>,
    out: &mut Vec<Command>,
) {
    draw_chart_with(
        wb,
        sheet,
        chart,
        w,
        h,
        locale,
        &default_color,
        true,
        label,
        out,
    );
}

/// Draws `chart` into a `w × h` box at the origin (points), colours resolved by `color`; `framed`
/// paints the white chart area with its border (sheets), otherwise the chart sits on its page (slides,
/// documents). The chart's formatting (CORE-CHART-007: palette, series treatment, areas, gridlines,
/// text colour, data labels) is applied.
#[allow(clippy::too_many_arguments)]
pub fn draw_chart_with(
    wb: &Workbook,
    sheet: usize,
    chart: &Chart,
    w: f64,
    h: f64,
    locale: &Locale,
    color: &ColorFn<'_>,
    framed: bool,
    label: &mut LabelFn<'_>,
    out: &mut Vec<Command>,
) {
    let start = out.len();
    let fmt = &chart.format;
    // Series without a colour of their own take the format's palette.
    let pal = |c: Option<&ChartColor>, i: usize| -> Rgba {
        match (c, fmt.palette.len()) {
            (None, n) if n > 0 => color(Some(&fmt.palette[i % n]), i),
            _ => color(c, i),
        }
    };
    let look = Look::new(fmt, &pal);
    let ink = look.text;
    let mut text = |t: &str, r: [f64; 4], a: Align, size: f64, c: Rgba, out: &mut Vec<Command>| {
        let c = if c == TEXT { ink.unwrap_or(c) } else { c };
        label(t, r, a, size, c, out);
    };
    draw_body(
        wb, sheet, chart, w, h, locale, &pal, framed, &look, &mut text, out,
    );
    if let Some(cmds) = out.get_mut(start..) {
        look.recolor(cmds);
    }
}

#[allow(clippy::too_many_lines, clippy::too_many_arguments)]
fn draw_body(
    wb: &Workbook,
    sheet: usize,
    chart: &Chart,
    w: f64,
    h: f64,
    locale: &Locale,
    color: &ColorFn<'_>,
    framed: bool,
    look: &Look,
    label: &mut LabelFn<'_>,
    out: &mut Vec<Command>,
) {
    // §21.2.2.29 chart space spPr: the chart area's fill and border.
    if framed || look.chart_fill.is_some() {
        let bg = look.chart_fill.unwrap_or(Rgba(255, 255, 255, 255));
        out.push(rect(0.0, 0.0, w, h, bg));
    }
    if framed || look.chart_line.is_some() {
        let fc = look.chart_line.unwrap_or(FRAME);
        for c in [
            hline(0.0, w, 0.25, fc, 0.5),
            hline(0.0, w, h - 0.25, fc, 0.5),
            vline(0.25, 0.0, h, fc, 0.5),
            vline(w - 0.25, 0.0, h, fc, 0.5),
        ] {
            out.push(c);
        }
    }
    // Sheet charts sit among cells; slide and document charts are read from further away.
    let font = look.font.unwrap_or(if framed {
        (h / 24.0).clamp(6.0, 11.0)
    } else {
        (h / 22.0).clamp(6.0, 14.0)
    });
    let mut top = 6.0;
    if let Some(t) = chart.title.as_deref().filter(|t| !t.is_empty()) {
        let th = font * 1.9;
        label(
            t,
            [0.0, top, w, top + th],
            Align::Center,
            font * 1.35,
            TEXT,
            out,
        );
        top += th + 2.0;
    }
    // Series with colours (index across all plots).
    let mut idx = 0usize;
    let mut plots: Vec<(&ChartPlot, Vec<Ser>)> = Vec::new();
    for p in &chart.plots {
        let mut v = Vec::new();
        let single = p.series.len() == 1;
        for s in &p.series {
            let (name, cats, vals) = series_data(wb, sheet, s, locale);
            let own = if p.kind == ChartKind::Line {
                s.line.as_ref().or(s.fill.as_ref())
            } else {
                s.fill.as_ref().or(s.line.as_ref())
            };
            let c = color(own, idx);
            // Points: their own fill, else one colour each for pies / varied colours, else the series'.
            let vary = (pie_like(&p.kind) && (p.vary_colors || own.is_none()))
                || (p.vary_colors && single);
            let pts = (0..vals.len())
                .map(|i| match s.point_fills.iter().find(|(k, _)| *k == i) {
                    Some((_, pc)) => color(Some(pc), i),
                    None if vary => color(None, i),
                    None => c,
                })
                .collect();
            // Hierarchical charts label points with their innermost level (the live range spans
            // several columns).
            let cats = match s.levels.first() {
                Some(inner) if matches!(p.kind, ChartKind::Treemap | ChartKind::Sunburst) => {
                    inner.clone()
                }
                _ => cats,
            };
            v.push(Ser {
                name: name.unwrap_or_else(|| format!("{}", idx + 1)),
                cats,
                vals,
                color: c,
                pts,
                sizes: s.sizes.clone(),
                explosion: s.explosion,
                levels: s.levels.clone(),
                subtotals: s.subtotals.clone(),
                trendlines: s.trendlines.clone(),
                err_bars: s.err_bars.clone(),
            });
            idx += 1;
        }
        plots.push((p, v));
    }
    let pie = plots.first().is_some_and(|(p, _)| pie_like(&p.kind));
    let surface = plots
        .first()
        .is_some_and(|(p, _)| p.kind == ChartKind::Surface);
    let bands = if surface {
        plots
            .first()
            .map(|(_, s)| surface_bands(s))
            .unwrap_or_default()
    } else {
        Vec::new()
    };
    let ex = plots
        .first()
        .filter(|(p, _)| p.kind.is_ex())
        .map(|(p, s)| (*p, s));
    // Legend (right or bottom).
    let entries: Vec<(String, Rgba)> = if let Some((p, ss)) = ex {
        ex_entries(p, ss, locale, color)
    } else if pie {
        plots
            .first()
            .and_then(|(_, s)| s.first())
            .map(|s| {
                s.cats
                    .iter()
                    .enumerate()
                    .map(|(i, c)| (c.clone(), s.pts.get(i).copied().unwrap_or(s.color)))
                    .collect()
            })
            .unwrap_or_default()
    } else if surface {
        bands
            .iter()
            .enumerate()
            .map(|(i, (a, b))| {
                (
                    format!("{}–{}", fmt_num(*a, locale), fmt_num(*b, locale)),
                    color(None, i),
                )
            })
            .collect()
    } else {
        plots
            .iter()
            .flat_map(|(_, s)| s.iter().map(|s| (s.name.clone(), s.color)))
            .collect()
    };
    let mut right = w - 6.0;
    let mut bottom = h - 6.0;
    if let Some(pos) = chart.legend.as_deref() {
        if !entries.is_empty() {
            let row = font * 1.6;
            if pos == "b" || pos == "t" {
                let n = entries.len().max(1) as f64;
                let cell = ((w - 12.0) / n).min(110.0);
                let y = if pos == "b" { h - row - 4.0 } else { top };
                let mut x = (w - cell * n) / 2.0;
                for (name, c) in &entries {
                    out.push(rect(x, y + row * 0.3, font * 0.8, font * 0.8, *c));
                    label(
                        name,
                        [x + font * 1.1, y, x + cell, y + row],
                        Align::Left,
                        font,
                        TEXT,
                        out,
                    );
                    x += cell;
                }
                if pos == "b" {
                    bottom = y - 2.0;
                } else {
                    top += row + 2.0;
                }
            } else {
                let lw = (w * 0.24).clamp(40.0, 140.0);
                let total = row * entries.len() as f64;
                let mut y = top + ((bottom - top) - total).max(0.0) / 2.0;
                let x = w - lw;
                for (name, c) in &entries {
                    out.push(rect(x, y + row * 0.3, font * 0.8, font * 0.8, *c));
                    label(
                        name,
                        [x + font * 1.1, y, w - 2.0, y + row],
                        Align::Left,
                        font,
                        TEXT,
                        out,
                    );
                    y += row;
                }
                right = w - lw - 6.0;
            }
        }
    }
    // 3-D groups (§21.2.2.15 bar3DChart, …) are drawn as solids.
    let solid = plots.first().filter(|(p, _)| {
        p.three_d
            && matches!(
                p.kind,
                ChartKind::Bar
                    | ChartKind::Line
                    | ChartKind::Area
                    | ChartKind::Pie
                    | ChartKind::Surface
            )
    });
    if let Some((p, ss)) = solid.filter(|_| pie || surface) {
        let view = chart
            .axes
            .view3d
            .clone()
            .unwrap_or_else(|| bo_model::sheet::View3D::new_chart(pie));
        three_d::draw(
            p,
            ss,
            &view,
            [6.0, top, right, bottom],
            font,
            locale,
            color,
            label,
            out,
        );
        return;
    }
    if let Some((p, ss)) = ex {
        draw_ex(
            p,
            ss,
            [6.0, top, right, bottom],
            font,
            locale,
            color,
            label,
            out,
        );
        return;
    }
    if pie {
        if let Some((p, s)) = plots.first() {
            let area = [6.0, top, right, bottom];
            if p.kind == ChartKind::OfPie {
                draw_of_pie(s.first(), p.style.as_deref() == Some("bar"), area, out);
            } else {
                draw_pie(
                    s.first(),
                    p.kind == ChartKind::Doughnut,
                    area,
                    (look, font, locale),
                    label,
                    out,
                );
            }
        }
        return;
    }
    if let Some((p, ss)) = plots.first().filter(|(p, _)| p.kind == ChartKind::Radar) {
        draw_radar(p, ss, [6.0, top, right, bottom], font, locale, label, out);
        return;
    }
    if surface {
        if let Some((_, ss)) = plots.first() {
            draw_surface(
                ss,
                &bands,
                [6.0, top, right, bottom],
                font,
                color,
                label,
                out,
            );
        }
        return;
    }
    // Axis titles.
    let mut left = 6.0;
    if let Some(t) = chart.x_title.as_deref().filter(|t| !t.is_empty()) {
        let th = font * 1.6;
        label(
            t,
            [left, bottom - th, right, bottom],
            Align::Center,
            font,
            TEXT,
            out,
        );
        bottom -= th;
    }
    if let Some(t) = chart.y_title.as_deref().filter(|t| !t.is_empty()) {
        let th = font * 1.6;
        let (cx, cy) = (left + th / 2.0, f64::midpoint(top, bottom));
        out.push(Command::PushTransform {
            m: [0.0, -1.0, 1.0, 0.0, cx as f32, cy as f32],
        });
        let half = (bottom - top) / 2.0;
        label(
            t,
            [-half, -th / 2.0, half, th / 2.0],
            Align::Center,
            font,
            TEXT,
            out,
        );
        out.push(Command::PopTransform);
        left += th;
    }
    if let Some((p, ss)) = solid {
        let view = chart
            .axes
            .view3d
            .clone()
            .unwrap_or_else(|| bo_model::sheet::View3D::new_chart(false));
        three_d::draw(
            p,
            ss,
            &view,
            [left, top, right, bottom],
            font,
            locale,
            color,
            label,
            out,
        );
        return;
    }
    // §21.2.2.54 dTable: a table of the values under the plot.
    let table_rows = if chart.axes.data_table {
        plots.iter().map(|(_, s)| s.len()).sum::<usize>() + 1
    } else {
        0
    };
    let table_h = table_rows as f64 * font * 1.6;
    let plot_bottom = bottom - table_h;
    let geo = draw_xy(
        &plots,
        [left, top, right, plot_bottom],
        font,
        locale,
        &chart.axes,
        look,
        label,
        out,
    );
    if let Some((pl, pr, n)) = geo.filter(|_| table_rows > 0) {
        data_table(
            &plots,
            [left, plot_bottom, right, bottom],
            (pl, pr, n),
            font,
            locale,
            label,
            out,
        );
    }
}

/// §21.2.2.54 dTable: one row per series under the categories, with grid lines.
#[allow(clippy::too_many_arguments)]
fn data_table(
    plots: &[(&ChartPlot, Vec<Ser>)],
    [l, t, _r, b]: [f64; 4],
    (pl, pr, n): (f64, f64, usize),
    font: f64,
    locale: &Locale,
    label: &mut LabelFn<'_>,
    out: &mut Vec<Command>,
) {
    let row = font * 1.6;
    let slot = (pr - pl) / n.max(1) as f64;
    let series: Vec<&Ser> = plots.iter().flat_map(|(_, s)| s.iter()).collect();
    let mut y = t + row;
    out.push(hline(l, pr, y, GRID, 0.5));
    for s in series {
        out.push(rect(
            l + 2.0,
            y + row * 0.3,
            font * 0.8,
            font * 0.8,
            s.color,
        ));
        label(
            &s.name,
            [l + font * 1.1, y, pl - 2.0, y + row],
            Align::Left,
            font,
            TEXT,
            out,
        );
        for i in 0..n {
            if let Some(v) = s.vals.get(i).copied().flatten() {
                let x = pl + i as f64 * slot;
                label(
                    &fmt_num(v, locale),
                    [x, y, x + slot, y + row],
                    Align::Center,
                    font,
                    TEXT,
                    out,
                );
            }
        }
        y += row;
        out.push(hline(l, pr, y.min(b), GRID, 0.5));
    }
    for i in 0..=n {
        out.push(vline(pl + i as f64 * slot, t, y.min(b), GRID, 0.5));
    }
}

/// Draws the axes and plots of a category / XY chart; returns the plot's left and right edges and the
/// number of categories (for the data table), `None` when there is nothing to draw.
#[allow(clippy::too_many_lines, clippy::too_many_arguments)]
fn draw_xy(
    plots: &[(&ChartPlot, Vec<Ser>)],
    [l, t, r, b]: [f64; 4],
    font: f64,
    locale: &Locale,
    axes: &bo_model::sheet::ChartAxes,
    look: &Look,
    label: &mut LabelFn<'_>,
    out: &mut Vec<Command>,
) -> Option<(f64, f64, usize)> {
    let scatter = plots
        .iter()
        .all(|(p, _)| matches!(p.kind, ChartKind::Scatter | ChartKind::Bubble));
    let horizontal = plots
        .iter()
        .all(|(p, _)| p.kind == ChartKind::Bar && p.bar_dir.as_deref() == Some("bar"));
    let stacked_of =
        |p: &ChartPlot| matches!(p.grouping.as_deref(), Some("stacked" | "percentStacked"));
    let percent_of = |p: &ChartPlot| p.grouping.as_deref() == Some("percentStacked");
    let ncat = plots
        .iter()
        .flat_map(|(_, s)| s.iter().map(|s| s.vals.len()))
        .max()
        .unwrap_or(0);
    if ncat == 0 {
        return None;
    }
    let val = |s: &Ser, i: usize| s.vals.get(i).copied().flatten().unwrap_or(0.0);
    // Value range over the plots of the primary axis (the secondary axis has its own, below).
    let (mut vmin, mut vmax) = (0.0_f64, 0.0_f64);
    for (p, ss) in plots.iter().filter(|(p, _)| !p.secondary) {
        for i in 0..ncat {
            if stacked_of(p) {
                let total: f64 = ss.iter().map(|s| val(s, i).abs()).sum();
                let (mut pos, mut neg) = (0.0, 0.0);
                for s in ss {
                    let v = if percent_of(p) && total > 0.0 {
                        val(s, i) / total * 100.0
                    } else {
                        val(s, i)
                    };
                    if v >= 0.0 {
                        pos += v;
                    } else {
                        neg += v;
                    }
                }
                vmax = vmax.max(pos);
                vmin = vmin.min(neg);
            } else {
                for s in ss {
                    vmax = vmax.max(val(s, i));
                    vmin = vmin.min(val(s, i));
                }
            }
        }
    }
    // Error bars and trend lines widen the range a little.
    for (_, ss) in plots.iter().filter(|(p, _)| !p.secondary) {
        for s in ss {
            if let Some(e) = &s.err_bars {
                let vs: Vec<f64> = s.vals.iter().flatten().copied().collect();
                for v in &vs {
                    let d = err_amount(e, *v, &vs);
                    vmax = vmax.max(v + d);
                    vmin = vmin.min(v - d);
                }
            }
        }
    }
    if vmax <= vmin {
        vmax = vmin + 1.0;
    }
    // §21.2.2.100 logBase: the axis runs over powers of the base (values ≤ 0 are not shown).
    let log = axes.log_base.filter(|b| *b > 1.0);
    let positive_min = plots
        .iter()
        .flat_map(|(_, s)| s.iter().flat_map(|s| s.vals.iter().flatten().copied()))
        .filter(|v| *v > 0.0)
        .fold(f64::MAX, f64::min);
    let tf = |v: f64| match log {
        Some(base) => v.max(1e-300).log(base),
        None => v,
    };
    let untf = |t: f64| match log {
        Some(base) => base.powf(t),
        None => t,
    };
    let (vmin, vmax, step) = if log.is_some() {
        let lo = if positive_min < f64::MAX {
            tf(positive_min).floor()
        } else {
            0.0
        };
        let hi = tf(vmax.max(untf(lo + 1.0))).ceil();
        (lo, hi.max(lo + 1.0), 1.0)
    } else {
        let step = nice_step(vmax - vmin, 5.0);
        (
            axes.min.unwrap_or((vmin / step).floor() * step),
            axes.max.unwrap_or((vmax / step).ceil() * step),
            step,
        )
    };
    let (vmin, vmax) = if vmax > vmin {
        (vmin, vmax)
    } else {
        (vmin, vmin + step)
    };
    // Secondary value axis (§21.2.2.226 valAx crossing at the right).
    let secondary = plots.iter().any(|(p, _)| p.secondary);
    let (smin, smax, sstep) = {
        let (mut lo, mut hi) = (0.0_f64, 0.0_f64);
        for (_, ss) in plots.iter().filter(|(p, _)| p.secondary) {
            for s in ss {
                for v in s.vals.iter().flatten() {
                    lo = lo.min(*v);
                    hi = hi.max(*v);
                }
            }
        }
        if hi <= lo {
            hi = lo + 1.0;
        }
        let st = nice_step(hi - lo, 5.0);
        ((lo / st).floor() * st, (hi / st).ceil() * st, st)
    };
    // Scatter X range.
    let xs_of = |s: &Ser| -> Vec<f64> {
        (0..s.vals.len())
            .map(|i| {
                s.cats
                    .get(i)
                    .and_then(|c| c.replace(',', ".").trim().parse::<f64>().ok())
                    .unwrap_or((i + 1) as f64)
            })
            .collect()
    };
    let (mut xmin, mut xmax) = (f64::MAX, f64::MIN);
    if scatter {
        for (_, ss) in plots {
            for s in ss {
                for x in xs_of(s) {
                    xmin = xmin.min(x);
                    xmax = xmax.max(x);
                }
            }
        }
        if xmax <= xmin {
            xmax = xmin + 1.0;
        }
    }
    let xstep = nice_step(xmax - xmin, 5.0);
    let (xmin, xmax) = if scatter {
        (
            (xmin / xstep).floor() * xstep,
            (xmax / xstep).ceil() * xstep,
        )
    } else {
        (0.0, 1.0)
    };
    let label_w = font * 4.0;
    let label_h = font * 1.6;
    let right_w = if secondary && !horizontal {
        label_w
    } else {
        0.0
    };
    let (pl, pt, pr, pb) = if horizontal {
        (l + label_w * 1.4, t, r, b - label_h)
    } else {
        (l + label_w, t + font * 0.5, r - right_w, b - label_h)
    };
    let (pw, ph) = ((pr - pl).max(1.0), (pb - pt).max(1.0));
    // On a log axis values at or below the axis minimum (bars from zero) start at the bottom.
    let vpos1 = |v: f64| {
        let f = (tf(v) - vmin) / (vmax - vmin);
        if log.is_some() {
            f.max(0.0)
        } else {
            f
        }
    };
    let vpos2 = |v: f64| (v - smin) / (smax - smin);
    let vpos = vpos1;
    // §21.2.2.145 plot area spPr.
    if let Some(pf) = look.plot_fill {
        out.push(rect(pl, pt, pw, ph, pf));
    }
    // Gridlines + value labels (on a log axis, at each power of the base).
    let mut tv = vmin;
    let mut guard = 0;
    while tv <= vmax + step * 0.5 && guard < 60 {
        let v = untf(tv);
        let f = (tv - vmin) / (vmax - vmin);
        let text = fmt_num(v, locale);
        if horizontal {
            let x = pl + f * pw;
            if look.val_grid {
                out.push(vline(x, pt, pb, GRID, 0.5));
            }
            label(
                &text,
                [x - label_w / 2.0, pb, x + label_w / 2.0, b],
                Align::Center,
                font,
                TEXT,
                out,
            );
        } else {
            let y = pb - f * ph;
            if look.val_grid {
                out.push(hline(pl, pr, y, GRID, 0.5));
            }
            label(
                &text,
                [l, y - label_h / 2.0, pl - 3.0, y + label_h / 2.0],
                Align::Right,
                font,
                TEXT,
                out,
            );
        }
        tv += step;
        guard += 1;
    }
    if secondary && !horizontal {
        let mut v = smin;
        let mut guard = 0;
        while v <= smax + sstep * 0.5 && guard < 60 {
            let y = pb - vpos2(v) * ph;
            label(
                &fmt_num(v, locale),
                [pr + 3.0, y - label_h / 2.0, r, y + label_h / 2.0],
                Align::Left,
                font,
                TEXT,
                out,
            );
            v += sstep;
            guard += 1;
        }
    }
    let zero = if log.is_some() {
        0.0
    } else {
        vpos(0.0_f64.clamp(vmin, vmax))
    };
    if horizontal {
        out.push(vline(pl + zero * pw, pt, pb, AXIS, 0.75));
    } else {
        out.push(hline(pl, pr, pb - zero * ph, AXIS, 0.75));
    }
    let slot = if horizontal { ph } else { pw } / ncat as f64;
    // §21.2.2.100 majorGridlines of the category axis: between the categories (at the X steps of
    // a scatter chart).
    if look.cat_grid {
        if scatter {
            let mut x = xmin;
            let mut guard = 0;
            while x <= xmax + xstep * 0.5 && guard < 60 {
                out.push(vline(
                    pl + (x - xmin) / (xmax - xmin) * pw,
                    pt,
                    pb,
                    GRID,
                    0.5,
                ));
                x += xstep;
                guard += 1;
            }
        } else {
            for i in 0..=ncat {
                let o = i as f64 * slot;
                out.push(if horizontal {
                    hline(pl, pr, pb - o, GRID, 0.5)
                } else {
                    vline(pl + o, pt, pb, GRID, 0.5)
                });
            }
        }
    }
    if scatter {
        let mut x = xmin;
        let mut guard = 0;
        while x <= xmax + xstep * 0.5 && guard < 60 {
            let px = pl + (x - xmin) / (xmax - xmin) * pw;
            label(
                &fmt_num(x, locale),
                [px - label_w / 2.0, pb + 1.0, px + label_w / 2.0, b],
                Align::Center,
                font,
                TEXT,
                out,
            );
            x += xstep;
            guard += 1;
        }
    } else {
        let cats: Vec<String> = (0..ncat)
            .map(|i| {
                plots
                    .iter()
                    .flat_map(|(_, s)| s.iter())
                    .find_map(|s| s.cats.get(i).filter(|c| !c.is_empty()).cloned())
                    .unwrap_or_else(|| (i + 1).to_string())
            })
            // §21.2.2.39 dateAx: serial-number categories shown as dates (dd/mm/yyyy).
            .map(
                |c| match c.trim().parse::<f64>().ok().filter(|_| axes.date) {
                    Some(serial) => crate::numfmt::serial_to_datetime(serial, false, 0)
                        .map_or(c.clone(), |d| {
                            format!("{:02}/{:02}/{}", d.day, d.month, d.year)
                        }),
                    None => c,
                },
            )
            .collect();
        // Thin labels out when they would overlap.
        let every = ((label_w * 0.8) / slot).ceil().max(1.0) as usize;
        for (i, c) in cats.iter().enumerate() {
            if horizontal {
                let y = pb - i as f64 * slot - slot;
                label(c, [l, y, pl - 3.0, y + slot], Align::Right, font, TEXT, out);
            } else if i % every == 0 {
                let x = pl + i as f64 * slot;
                label(
                    c,
                    [
                        x - slot * (every as f64 - 1.0) / 2.0,
                        pb + 1.0,
                        x + slot * (every as f64 + 1.0) / 2.0,
                        b,
                    ],
                    Align::Center,
                    font,
                    TEXT,
                    out,
                );
            }
        }
    }
    // Bars first, then areas, lines and scatter on top.
    let mut order: Vec<&(&ChartPlot, Vec<Ser>)> = plots.iter().collect();
    order.sort_by_key(|(p, _)| match p.kind {
        ChartKind::Area => 0,
        ChartKind::Bar => 1,
        _ => 2,
    });
    let nbar_groups = 1.0;
    for (p, ss) in order {
        let vpos = |v: f64| if p.secondary { vpos2(v) } else { vpos1(v) };
        match p.kind {
            ChartKind::Bar => {
                let stacked = stacked_of(p);
                let gap = look.gap / nbar_groups;
                let group = slot / (1.0 + gap);
                let bar = if stacked {
                    group
                } else {
                    group / ss.len().max(1) as f64
                };
                for i in 0..ncat {
                    let total: f64 = ss.iter().map(|s| val(s, i).abs()).sum();
                    let (mut pos, mut neg) = (0.0, 0.0);
                    for (si, s) in ss.iter().enumerate() {
                        let mut v = val(s, i);
                        if percent_of(p) && total > 0.0 {
                            v = v / total * 100.0;
                        }
                        let (from, to) = if stacked {
                            let base = if v >= 0.0 { pos } else { neg };
                            if v >= 0.0 {
                                pos += v;
                            } else {
                                neg += v;
                            }
                            (base, base + v)
                        } else {
                            (0.0, v)
                        };
                        let (a, z) = (vpos(from), vpos(to));
                        let (lo, hi) = (a.min(z), a.max(z));
                        let off = i as f64 * slot
                            + (slot - group) / 2.0
                            + if stacked { 0.0 } else { si as f64 * bar };
                        let c = s.pts.get(i).copied().unwrap_or(s.color);
                        let bx = if horizontal {
                            [pl + lo * pw, pb - off - bar, (hi - lo) * pw, bar]
                        } else {
                            [pl + off, pb - hi * ph, bar, (hi - lo) * ph]
                        };
                        look.bar(bx, c, out);
                        // §21.2.2.49 data labels: at the outer end of a clustered bar, centred in
                        // a stacked one.
                        let total = if percent_of(p) { Some(v / 100.0) } else { None };
                        let cat = s.cats.get(i).map_or("", String::as_str);
                        if let Some(text) = look.label_text(&s.name, cat, val(s, i), total, locale)
                        {
                            let centre = stacked
                                || look.labels.as_ref().and_then(|d| d.pos.as_deref())
                                    == Some("ctr");
                            let (x, y) = match (horizontal, centre) {
                                (_, true) => (bx[0] + bx[2] / 2.0, bx[1] + bx[3] / 2.0),
                                (true, false) => (bx[0] + bx[2] + font * 1.2, bx[1] + bx[3] / 2.0),
                                (false, false) => (bx[0] + bx[2] / 2.0, bx[1] - font * 0.8),
                            };
                            let ink = if centre {
                                Rgba(255, 255, 255, 255)
                            } else {
                                TEXT
                            };
                            Look::put_label(&text, x, y, font, ink, label, out);
                        }
                    }
                }
            }
            ChartKind::Line | ChartKind::Area => {
                let stacked = stacked_of(p);
                // 100 % stacked: each category's values as a share of its total.
                let totals: Vec<f64> = (0..ncat)
                    .map(|i| ss.iter().map(|s| val(s, i).abs()).sum())
                    .collect();
                let share = |s: &Ser, i: usize| {
                    if percent_of(p) && totals[i] > 0.0 {
                        val(s, i) / totals[i] * 100.0
                    } else {
                        val(s, i)
                    }
                };
                let mut base = vec![0.0; ncat];
                for s in ss {
                    let pts: Vec<(f64, f64)> = (0..ncat)
                        .map(|i| {
                            let v = share(s, i) + if stacked { base[i] } else { 0.0 };
                            (pl + (i as f64 + 0.5) * slot, pb - vpos(v) * ph)
                        })
                        .collect();
                    if p.kind == ChartKind::Area {
                        let mut segs = Vec::with_capacity(ncat * 2 + 2);
                        for (i, (x, y)) in pts.iter().enumerate() {
                            segs.push(if i == 0 {
                                PathSeg::M(*x as f32, *y as f32)
                            } else {
                                PathSeg::L(*x as f32, *y as f32)
                            });
                        }
                        for i in (0..ncat).rev() {
                            let y = pb
                                - vpos(if stacked {
                                    base[i]
                                } else {
                                    0.0_f64.clamp(vmin, vmax)
                                }) * ph;
                            segs.push(PathSeg::L(pts[i].0 as f32, y as f32));
                        }
                        segs.push(PathSeg::Z);
                        let Rgba(r, g, bl, _) = s.color;
                        if look.fill == bo_model::chart_style::SeriesFill::Solid
                            && look.outline.is_none()
                            && !look.shadow
                        {
                            out.push(Command::Path {
                                segs,
                                fill: Some(Paint::Solid {
                                    color: Rgba(r, g, bl, 200),
                                }),
                                stroke: None,
                            });
                        } else {
                            look.shape(segs, [pl, pt, pr, pb], s.color, out);
                        }
                    } else {
                        out.push(polyline(&pts, s.color, look.line_w));
                        look.markers(&pts, s.color, out);
                    }
                    if look.labels.is_some() {
                        for (i, (x, y)) in pts.iter().enumerate() {
                            let cat = s.cats.get(i).map_or("", String::as_str);
                            if let Some(text) =
                                look.label_text(&s.name, cat, share(s, i), None, locale)
                            {
                                Look::put_label(&text, *x, y - font * 0.9, font, TEXT, label, out);
                            }
                        }
                    }
                    if stacked {
                        for (i, b) in base.iter_mut().enumerate() {
                            *b += share(s, i);
                        }
                    }
                }
            }
            ChartKind::Scatter => {
                for s in ss {
                    let xs = xs_of(s);
                    for (i, x) in xs.iter().enumerate() {
                        let Some(y) = s.vals.get(i).copied().flatten() else {
                            continue;
                        };
                        let px = pl + (x - xmin) / (xmax - xmin) * pw;
                        let py = pb - vpos(y) * ph;
                        out.push(rect(px - 2.5, py - 2.5, 5.0, 5.0, s.color));
                    }
                }
            }
            // §21.2.2.20 bubbleChart: circles whose area follows the bubble size (bubbleScale 100 %:
            // the largest bubble a quarter of the plot's smaller side across).
            ChartKind::Bubble => {
                let max = ss
                    .iter()
                    .flat_map(|s| s.sizes.iter().flatten())
                    .fold(0.0_f64, |m, v| m.max(v.abs()));
                let rmax = pw.min(ph) * 0.125;
                for s in ss {
                    let xs = xs_of(s);
                    for (i, x) in xs.iter().enumerate() {
                        let Some(y) = s.vals.get(i).copied().flatten() else {
                            continue;
                        };
                        let size = s.sizes.get(i).copied().flatten().unwrap_or(1.0).abs();
                        let r = if max > 0.0 {
                            (size / max).sqrt() * rmax
                        } else {
                            rmax / 2.0
                        };
                        let px = pl + (x - xmin) / (xmax - xmin) * pw;
                        let py = pb - vpos(y) * ph;
                        let Rgba(cr, cg, cb, _) = s.pts.get(i).copied().unwrap_or(s.color);
                        out.push(Command::Path {
                            segs: circle(px, py, r.max(1.0)),
                            fill: Some(Paint::Solid {
                                color: Rgba(cr, cg, cb, 190),
                            }),
                            stroke: Some(thin(Rgba(255, 255, 255, 255))),
                        });
                    }
                }
            }
            // §21.2.2.198 stockChart: high–low line per category, close tick to the right, open tick to
            // the left; with up/down bars (§21.2.2.218) a box between open and close (white up, dark
            // down). Series in order (open,) high, low, close.
            ChartKind::Stock => {
                let (open, high, low, close) = match ss.len() {
                    n if n >= 4 => (Some(&ss[0]), &ss[1], &ss[2], &ss[3]),
                    3 => (None, &ss[0], &ss[1], &ss[2]),
                    _ => continue,
                };
                let tick = slot * 0.18;
                for i in 0..ncat {
                    let x = pl + (i as f64 + 0.5) * slot;
                    let y = |s: &Ser| s.vals.get(i).copied().flatten().map(|v| pb - vpos(v) * ph);
                    if let (Some(hy), Some(ly)) = (y(high), y(low)) {
                        out.push(vline(x, hy, ly, TEXT, 1.0));
                    }
                    let (oy, cy) = (open.and_then(y), y(close));
                    if p.flag {
                        if let (Some(oy), Some(cy)) = (oy, cy) {
                            let up = cy < oy;
                            let (top, h) = (oy.min(cy), (oy - cy).abs().max(0.75));
                            let fill = if up { Rgba(255, 255, 255, 255) } else { TEXT };
                            out.push(rect(x - tick, top, tick * 2.0, h, fill));
                            out.push(polyline(
                                &[
                                    (x - tick, top),
                                    (x + tick, top),
                                    (x + tick, top + h),
                                    (x - tick, top + h),
                                    (x - tick, top),
                                ],
                                TEXT,
                                0.75,
                            ));
                            continue;
                        }
                    }
                    if let Some(cy) = cy {
                        out.push(hline(x, x + tick, cy, TEXT, 1.0));
                    }
                    if let Some(oy) = oy {
                        out.push(hline(x - tick, x, oy, TEXT, 1.0));
                    }
                }
            }
            _ => {}
        }
    }
    // Components (§21.2.2): drop lines, high-low lines, up/down bars, error bars, trend lines.
    for (p, ss) in plots {
        let vpos = |v: f64| if p.secondary { vpos2(v) } else { vpos1(v) };
        let x_of = |i: usize| pl + (i as f64 + 0.5) * slot;
        let cat_like =
            matches!(p.kind, ChartKind::Line | ChartKind::Area | ChartKind::Bar) && !horizontal;
        if cat_like && p.drop_lines {
            for s in ss {
                for (i, v) in s.vals.iter().enumerate() {
                    if let Some(v) = v {
                        out.push(vline(x_of(i), pb - vpos(*v) * ph, pb, AXIS, 0.5));
                    }
                }
            }
        }
        if p.kind == ChartKind::Line && (p.hi_low_lines || p.flag) && ss.len() > 1 {
            for i in 0..ncat {
                let vs: Vec<f64> = ss
                    .iter()
                    .filter_map(|s| s.vals.get(i).copied().flatten())
                    .collect();
                let (Some(lo), Some(hi)) = (
                    vs.iter().copied().reduce(f64::min),
                    vs.iter().copied().reduce(f64::max),
                ) else {
                    continue;
                };
                if p.hi_low_lines {
                    out.push(vline(
                        x_of(i),
                        pb - vpos(hi) * ph,
                        pb - vpos(lo) * ph,
                        TEXT,
                        0.75,
                    ));
                }
                // §21.2.2.218 upDownBars between the first and the last series.
                if p.flag {
                    if let (Some(first), Some(last)) = (
                        ss.first().and_then(|s| s.vals.get(i).copied().flatten()),
                        ss.last().and_then(|s| s.vals.get(i).copied().flatten()),
                    ) {
                        let (y0, y1) = (pb - vpos(first) * ph, pb - vpos(last) * ph);
                        let fill = if last >= first {
                            Rgba(255, 255, 255, 255)
                        } else {
                            TEXT
                        };
                        let w = slot * 0.3;
                        let (top, h) = (y0.min(y1), (y0 - y1).abs().max(0.75));
                        out.push(rect(x_of(i) - w / 2.0, top, w, h, fill));
                        out.push(polyline(
                            &[
                                (x_of(i) - w / 2.0, top),
                                (x_of(i) + w / 2.0, top),
                                (x_of(i) + w / 2.0, top + h),
                                (x_of(i) - w / 2.0, top + h),
                                (x_of(i) - w / 2.0, top),
                            ],
                            TEXT,
                            0.75,
                        ));
                    }
                }
            }
        }
        let scatter_plot = matches!(p.kind, ChartKind::Scatter | ChartKind::Bubble);
        for (si, s) in ss.iter().enumerate() {
            // X positions of the points (category centre; the bar's own centre for clustered bars).
            let bar_x = |i: usize| -> f64 {
                if p.kind == ChartKind::Bar && !stacked_of(p) {
                    let group = slot / (1.0 + look.gap);
                    let bar = group / ss.len().max(1) as f64;
                    pl + i as f64 * slot + (slot - group) / 2.0 + si as f64 * bar + bar / 2.0
                } else {
                    x_of(i)
                }
            };
            let xs = xs_of(s);
            let px_of = |i: usize| -> f64 {
                if scatter_plot {
                    pl + (xs[i] - xmin) / (xmax - xmin) * pw
                } else {
                    bar_x(i)
                }
            };
            if let Some(e) = &s.err_bars {
                if !horizontal {
                    let vs: Vec<f64> = s.vals.iter().flatten().copied().collect();
                    let mean = if vs.is_empty() {
                        0.0
                    } else {
                        vs.iter().sum::<f64>() / vs.len() as f64
                    };
                    for (i, v) in s.vals.iter().enumerate() {
                        let Some(v) = v else { continue };
                        // Standard-deviation bars sit around the mean of the series.
                        let centre = if e.value_type == "stdDev" { mean } else { *v };
                        let d = err_amount(e, *v, &vs);
                        let (hi, lo) = match e.kind.as_str() {
                            "plus" => (centre + d, centre),
                            "minus" => (centre, centre - d),
                            _ => (centre + d, centre - d),
                        };
                        let x = px_of(i);
                        let (yh, yl) = (pb - vpos(hi) * ph, pb - vpos(lo) * ph);
                        out.push(vline(x, yh, yl, TEXT, 0.75));
                        let cap = (slot * 0.08).clamp(2.0, 6.0);
                        if e.kind != "minus" {
                            out.push(hline(x - cap, x + cap, yh, TEXT, 0.75));
                        }
                        if e.kind != "plus" {
                            out.push(hline(x - cap, x + cap, yl, TEXT, 0.75));
                        }
                    }
                }
            }
            for tl in &s.trendlines {
                if horizontal {
                    break;
                }
                let pts: Vec<(f64, f64)> = s
                    .vals
                    .iter()
                    .enumerate()
                    .filter_map(|(i, v)| {
                        v.map(|v| (if scatter_plot { xs[i] } else { (i + 1) as f64 }, v))
                    })
                    .collect();
                let Some(fit) = Fit::new(tl, &pts) else {
                    continue;
                };
                let (x0, x1) = pts.iter().fold((f64::MAX, f64::MIN), |(a, z), (x, _)| {
                    (a.min(*x), z.max(*x))
                });
                let to_px = |x: f64| {
                    if scatter_plot {
                        pl + (x - xmin) / (xmax - xmin) * pw
                    } else {
                        pl + (x - 0.5) * slot
                    }
                };
                let curve: Vec<(f64, f64)> = if tl.kind == "movingAvg" {
                    Fit::moving(&pts, tl.order.max(2) as usize)
                        .into_iter()
                        .map(|(x, y)| (to_px(x), pb - vpos(y) * ph))
                        .collect()
                } else {
                    (0..=60)
                        .map(|k| x0 + (x1 - x0) * f64::from(k) / 60.0)
                        .filter_map(|x| {
                            fit.at(x)
                                .filter(|y| y.is_finite())
                                .map(|y| (to_px(x), pb - vpos(y).clamp(-0.2, 1.2) * ph))
                        })
                        .collect()
                };
                let Rgba(cr, cg, cb, _) = s.color;
                out.push(Command::Path {
                    segs: curve
                        .iter()
                        .enumerate()
                        .map(|(k, (x, y))| {
                            if k == 0 {
                                PathSeg::M(*x as f32, *y as f32)
                            } else {
                                PathSeg::L(*x as f32, *y as f32)
                            }
                        })
                        .collect(),
                    fill: None,
                    stroke: Some(Stroke {
                        width: 1.25,
                        color: Rgba(cr, cg, cb, 255),
                        dash: vec![4.0, 2.5],
                        cap: "butt".into(),
                        join: "round".into(),
                    }),
                });
                // §21.2.2.51 dispEq / §21.2.2.50 dispRSqr near the end of the line.
                let mut lines = Vec::new();
                // A moving average has no equation or R².
                if tl.disp_eq && tl.kind != "movingAvg" {
                    lines.push(fit.equation(locale));
                }
                if tl.disp_r2 && tl.kind != "movingAvg" {
                    lines.push(format!("R² = {}", fmt_dec(fit.r2(&pts), 4, locale)));
                }
                if let Some((x, y)) = curve.last() {
                    // One block of labels per series, stacked upwards so that they do not overlap.
                    let lift = font * 1.4 * (lines.len() * si) as f64;
                    for (k, text) in lines.iter().enumerate() {
                        let yy = y - lift - font * 1.4 * (lines.len() - k) as f64;
                        label(
                            text,
                            [x - font * 12.0, yy, *x, yy + font * 1.4],
                            Align::Right,
                            font,
                            TEXT,
                            out,
                        );
                    }
                }
            }
        }
    }
    Some((pl, pr, ncat))
}

/// Error amount of a point (§21.2.2.59 errValType).
fn err_amount(e: &bo_model::sheet::ErrorBars, v: f64, all: &[f64]) -> f64 {
    let n = all.len() as f64;
    let mean = if n > 0.0 {
        all.iter().sum::<f64>() / n
    } else {
        0.0
    };
    // Sample standard deviation (n − 1).
    let sd = if n > 1.0 {
        (all.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (n - 1.0)).sqrt()
    } else {
        0.0
    };
    match e.value_type.as_str() {
        "percentage" => v.abs() * e.value / 100.0,
        "stdDev" => e.value * sd,
        "stdErr" => {
            if n > 0.0 {
                sd / n.sqrt()
            } else {
                0.0
            }
        }
        _ => e.value,
    }
}

/// A number with at most `digits` decimals (trailing zeros dropped), in the locale.
fn fmt_dec(v: f64, digits: usize, locale: &Locale) -> String {
    let s = format!("{v:.digits$}");
    let s = if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.').to_owned()
    } else {
        s
    };
    if locale.decimal == ',' {
        s.replace('.', ",")
    } else {
        s
    }
}

/// A fitted trend line (§21.2.2.212 trendlineType).
struct Fit {
    kind: String,
    /// Coefficients: linear / log / exp / power (a, b); polynomial c0..cn.
    c: Vec<f64>,
}

impl Fit {
    fn new(t: &bo_model::sheet::Trendline, pts: &[(f64, f64)]) -> Option<Fit> {
        if pts.len() < 2 {
            return None;
        }
        // Linear fit of (g(x), h(y)) by the LINEST solver: returns (intercept, slope).
        let line = |pts: &[(f64, f64)]| -> Option<(f64, f64)> {
            let x: Vec<Vec<f64>> = pts.iter().map(|(x, _)| vec![*x]).collect();
            let y: Vec<f64> = pts.iter().map(|(_, y)| *y).collect();
            let (m, b) = bo_formula::linear_fit(&x, &y)?;
            Some((b, m[0]))
        };
        let c = match t.kind.as_str() {
            "exp" => {
                let q: Vec<(f64, f64)> = pts
                    .iter()
                    .filter(|(_, y)| *y > 0.0)
                    .map(|(x, y)| (*x, y.ln()))
                    .collect();
                let (a, b) = line(&q)?;
                vec![a.exp(), b]
            }
            "log" => {
                let q: Vec<(f64, f64)> = pts
                    .iter()
                    .filter(|(x, _)| *x > 0.0)
                    .map(|(x, y)| (x.ln(), *y))
                    .collect();
                let (a, b) = line(&q)?;
                vec![a, b]
            }
            "power" => {
                let q: Vec<(f64, f64)> = pts
                    .iter()
                    .filter(|(x, y)| *x > 0.0 && *y > 0.0)
                    .map(|(x, y)| (x.ln(), y.ln()))
                    .collect();
                let (a, b) = line(&q)?;
                vec![a.exp(), b]
            }
            "poly" => {
                let k = (t.order.clamp(2, 6) as usize).min(pts.len() - 1);
                // Polynomial: least squares on the powers x, x², …, xᵏ (LINEST with x^{1,2,…}).
                let x: Vec<Vec<f64>> = pts
                    .iter()
                    .map(|(x, _)| (1..=k).map(|i| x.powi(i as i32)).collect())
                    .collect();
                let y: Vec<f64> = pts.iter().map(|(_, y)| *y).collect();
                let (m, b) = bo_formula::linear_fit(&x, &y)?;
                std::iter::once(b).chain(m).collect()
            }
            "movingAvg" => Vec::new(),
            _ => {
                let (a, b) = line(pts)?;
                vec![a, b]
            }
        };
        Some(Fit {
            kind: t.kind.clone(),
            c,
        })
    }

    fn at(&self, x: f64) -> Option<f64> {
        let c = &self.c;
        Some(match self.kind.as_str() {
            "exp" => c[0] * (c[1] * x).exp(),
            "log" if x > 0.0 => c[0] + c[1] * x.ln(),
            "power" if x > 0.0 => c[0] * x.powf(c[1]),
            // No value: log / power at x ≤ 0, moving averages (drawn point by point).
            "log" | "power" | "movingAvg" => return None,
            "poly" => c
                .iter()
                .enumerate()
                .map(|(i, k)| k * x.powi(i as i32))
                .sum(),
            _ => c[0] + c[1] * x,
        })
    }

    /// Moving average over `period` points, at each point from the `period`-th.
    fn moving(pts: &[(f64, f64)], period: usize) -> Vec<(f64, f64)> {
        pts.windows(period)
            .map(|w| {
                (
                    w[w.len() - 1].0,
                    w.iter().map(|(_, y)| y).sum::<f64>() / w.len() as f64,
                )
            })
            .collect()
    }

    /// Coefficient of determination of the fit on `pts` (in the fitted space for exp / power, as the
    /// spreadsheet reports it).
    fn r2(&self, pts: &[(f64, f64)]) -> f64 {
        let tr = |y: f64| {
            if matches!(self.kind.as_str(), "exp" | "power") {
                y.ln()
            } else {
                y
            }
        };
        let data: Vec<(f64, f64)> = pts
            .iter()
            .filter_map(|(x, y)| self.at(*x).map(|f| (tr(*y), tr(f))))
            .filter(|(y, f)| y.is_finite() && f.is_finite())
            .collect();
        let n = data.len() as f64;
        if n < 2.0 {
            return 0.0;
        }
        let mean = data.iter().map(|(y, _)| y).sum::<f64>() / n;
        let ss_tot: f64 = data.iter().map(|(y, _)| (y - mean).powi(2)).sum();
        let ss_res: f64 = data.iter().map(|(y, f)| (y - f).powi(2)).sum();
        if ss_tot <= 0.0 {
            1.0
        } else {
            1.0 - ss_res / ss_tot
        }
    }

    /// The equation shown on the chart (`y = 2x + 1`, `y = 3e^{0,5x}` written `y = 3e^0,5x` …).
    fn equation(&self, locale: &Locale) -> String {
        let n = |v: f64| fmt_dec(v, 4, locale);
        let signed = |v: f64| {
            if v < 0.0 {
                format!(" − {}", n(-v))
            } else {
                format!(" + {}", n(v))
            }
        };
        let c = &self.c;
        match self.kind.as_str() {
            "exp" => format!("y = {}e^{}x", n(c[0]), n(c[1])),
            "log" => format!("y = {}ln(x){}", n(c[1]), signed(c[0])),
            "power" => format!("y = {}x^{}", n(c[0]), n(c[1])),
            "poly" => {
                let mut s = String::from("y =");
                for (i, k) in c.iter().enumerate().rev() {
                    let term = match i {
                        0 => String::new(),
                        1 => "x".to_owned(),
                        _ => format!("x^{i}"),
                    };
                    if i == c.len() - 1 {
                        s.push_str(&format!(" {}{term}", n(*k)));
                    } else {
                        s.push_str(&format!("{}{term}", signed(*k)));
                    }
                }
                s
            }
            "movingAvg" => String::new(),
            _ => format!("y = {}x{}", n(c[1]), signed(c[0])),
        }
    }
}

/// Wedge from `a0` sweeping `sw` radians clockwise from 12 o'clock, as cubic Béziers.
fn wedge(cx: f64, cy: f64, r: f64, inner: f64, a0: f64, sw: f64) -> Vec<PathSeg> {
    let pt = |rad: f64, a: f64| (cx + rad * a.sin(), cy - rad * a.cos());
    let arc = |segs: &mut Vec<PathSeg>, rad: f64, from: f64, sweep: f64| {
        let n = (sweep.abs() / std::f64::consts::FRAC_PI_2).ceil().max(1.0) as usize;
        let d = sweep / n as f64;
        let k = 4.0 / 3.0 * (d / 4.0).tan() * rad;
        for i in 0..n {
            let s = from + d * i as f64;
            let e = s + d;
            let p0 = pt(rad, s);
            let p3 = pt(rad, e);
            let c1 = (p0.0 + k * s.cos(), p0.1 + k * s.sin());
            let c2 = (p3.0 - k * e.cos(), p3.1 - k * e.sin());
            segs.push(PathSeg::C(
                c1.0 as f32,
                c1.1 as f32,
                c2.0 as f32,
                c2.1 as f32,
                p3.0 as f32,
                p3.1 as f32,
            ));
        }
    };
    let mut segs = Vec::new();
    if inner > 0.0 {
        let s = pt(r, a0);
        segs.push(PathSeg::M(s.0 as f32, s.1 as f32));
        arc(&mut segs, r, a0, sw);
        let e = pt(inner, a0 + sw);
        segs.push(PathSeg::L(e.0 as f32, e.1 as f32));
        arc(&mut segs, inner, a0 + sw, -sw);
    } else {
        segs.push(PathSeg::M(cx as f32, cy as f32));
        let s = pt(r, a0);
        segs.push(PathSeg::L(s.0 as f32, s.1 as f32));
        arc(&mut segs, r, a0, sw);
    }
    segs.push(PathSeg::Z);
    segs
}

/// A pie without formatting or labels (the parts of a pie of pie).
fn draw_pie_plain(s: Option<&Ser>, doughnut: bool, area: [f64; 4], out: &mut Vec<Command>) {
    let look = Look::new(
        &bo_model::chart_style::ChartFormat::default(),
        &default_color,
    );
    draw_pie(
        s,
        doughnut,
        area,
        (&look, 10.0, &Locale::default()),
        &mut |_, _, _, _, _, _| {},
        out,
    );
}

fn draw_pie(
    s: Option<&Ser>,
    doughnut: bool,
    [l, t, r, b]: [f64; 4],
    (look, font, locale): (&Look, f64, &Locale),
    label: &mut LabelFn<'_>,
    out: &mut Vec<Command>,
) {
    let Some(s) = s else { return };
    let vals: Vec<f64> = s.vals.iter().map(|v| v.unwrap_or(0.0).max(0.0)).collect();
    let total: f64 = vals.iter().sum();
    if total <= 0.0 {
        return;
    }
    // §21.2.2.65 explosion: slices pushed out by a share of the radius (the pie shrinks to fit).
    let ex = f64::from(s.explosion.min(400)) / 100.0;
    let rad = (((r - l).min(b - t) / 2.0 - 2.0) / (1.0 + ex)).max(1.0);
    let (cx, cy) = (f64::midpoint(l, r), f64::midpoint(t, b));
    let inner = if doughnut { rad * 0.5 } else { 0.0 };
    let mut a = 0.0;
    for (i, v) in vals.iter().enumerate() {
        let sw = v / total * std::f64::consts::TAU;
        if sw <= 0.0 {
            continue;
        }
        let mid = a + sw / 2.0;
        let (ox, oy) = (rad * ex * mid.sin(), -rad * ex * mid.cos());
        let pc = s.pts.get(i).copied().unwrap_or(PALETTE[i % PALETTE.len()]);
        if look.fill == bo_model::chart_style::SeriesFill::Solid
            && look.outline.is_none()
            && !look.shadow
        {
            out.push(Command::Path {
                segs: wedge(cx + ox, cy + oy, rad, inner, a, sw),
                fill: Some(Paint::Solid { color: pc }),
                stroke: Some(Stroke {
                    width: 0.75,
                    color: Rgba(255, 255, 255, 255),
                    dash: vec![],
                    cap: "butt".into(),
                    join: "round".into(),
                }),
            });
        } else {
            look.shape(
                wedge(cx + ox, cy + oy, rad, inner, a, sw),
                [cx - rad, cy - rad, cx + rad, cy + rad],
                pc,
                out,
            );
        }
        // §21.2.2.49 data labels inside the slice.
        let cat = s.cats.get(i).map_or("", String::as_str);
        if let Some(text) = look.label_text(&s.name, cat, *v, Some(v / total), locale) {
            let lr = if doughnut { rad * 0.75 } else { rad * 0.62 };
            let (lx, ly) = (cx + ox + lr * mid.sin(), cy + oy - lr * mid.cos());
            Look::put_label(&text, lx, ly, font, Rgba(255, 255, 255, 255), label, out);
        }
        a += sw;
    }
}

/// Legend entries of a chartex chart.
fn ex_entries(
    p: &ChartPlot,
    ss: &[Ser],
    locale: &Locale,
    color: &ColorFn<'_>,
) -> Vec<(String, Rgba)> {
    let vi = locale.lcid == 0x42A;
    match p.kind {
        ChartKind::Waterfall => [
            (if vi { "Tăng" } else { "Increase" }, 0),
            (if vi { "Giảm" } else { "Decrease" }, 1),
            (if vi { "Tổng" } else { "Total" }, 2),
        ]
        .iter()
        .map(|(t, i)| ((*t).to_owned(), color(None, *i)))
        .collect(),
        ChartKind::Treemap | ChartKind::Sunburst => ss
            .first()
            .map(|s| {
                top_groups(s)
                    .into_iter()
                    .enumerate()
                    .map(|(i, (g, _))| (g, color(None, i)))
                    .collect()
            })
            .unwrap_or_default(),
        _ => ss.iter().map(|s| (s.name.clone(), s.color)).collect(),
    }
}

/// Top-level groups of a hierarchical series (outermost level), in order, with their point indices.
fn top_groups(s: &Ser) -> Vec<(String, Vec<usize>)> {
    let top = s.levels.last().unwrap_or(&s.cats);
    let mut out: Vec<(String, Vec<usize>)> = Vec::new();
    for (i, g) in top.iter().enumerate() {
        match out.iter_mut().find(|(n, _)| n == g) {
            Some((_, v)) => v.push(i),
            None => out.push((g.clone(), vec![i])),
        }
    }
    out
}

/// Value axis of a plot area: gridlines and labels from `vmin` to `vmax`; returns the inner plot rect.
#[allow(clippy::too_many_arguments)]
fn value_axis(
    [l, t, r, b]: [f64; 4],
    (vmin, vmax): (f64, f64),
    font: f64,
    locale: &Locale,
    percent: bool,
    label: &mut LabelFn<'_>,
    out: &mut Vec<Command>,
) -> ([f64; 4], f64, f64) {
    let step = nice_step((vmax - vmin).max(1e-9), 5.0);
    let (vmin, vmax) = (
        (vmin / step).floor() * step,
        ((vmax / step).ceil() * step).max(vmin + step),
    );
    let label_w = font * 4.0;
    let (pl, pt, pr, pb) = (l + label_w, t + font * 0.5, r, b - font * 1.6);
    let ph = (pb - pt).max(1.0);
    let mut v = vmin;
    let mut guard = 0;
    while v <= vmax + step * 0.5 && guard < 60 {
        let y = pb - (v - vmin) / (vmax - vmin) * ph;
        out.push(hline(pl, pr, y, GRID, 0.5));
        let text = if percent {
            format!("{}%", fmt_num(v, locale))
        } else {
            fmt_num(v, locale)
        };
        label(
            &text,
            [l, y - font, pl - 3.0, y + font],
            Align::Right,
            font,
            TEXT,
            out,
        );
        v += step;
        guard += 1;
    }
    ([pl, pt, pr, pb], vmin, vmax)
}

/// Category labels under `n` slots of a plot rect.
fn cat_labels(
    cats: &[String],
    [pl, _, pr, pb]: [f64; 4],
    b: f64,
    font: f64,
    label: &mut LabelFn<'_>,
    out: &mut Vec<Command>,
) {
    let n = cats.len().max(1);
    let slot = (pr - pl) / n as f64;
    let every = ((font * 3.2) / slot).ceil().max(1.0) as usize;
    for (i, c) in cats.iter().enumerate() {
        if i % every == 0 {
            let x = pl + i as f64 * slot;
            label(
                c,
                [x - slot * 0.5, pb + 1.0, x + slot * 1.5, b],
                Align::Center,
                font,
                TEXT,
                out,
            );
        }
    }
}

/// Histogram bins of `vals`: `count:n`, `size:w` or automatic (Scott's normal reference rule).
fn bins(vals: &[f64], style: Option<&str>) -> Vec<(f64, f64, usize)> {
    let (Some(lo), Some(hi)) = (
        vals.iter().copied().reduce(f64::min),
        vals.iter().copied().reduce(f64::max),
    ) else {
        return Vec::new();
    };
    let n = vals.len() as f64;
    let width = match style {
        Some(st) if st.starts_with("count:") => {
            let k: f64 = st[6..].trim().parse().unwrap_or(5.0);
            (hi - lo).max(1e-9) / k.max(1.0)
        }
        Some(st) if st.starts_with("size:") => st[5..].trim().parse().unwrap_or(1.0_f64).max(1e-9),
        _ => {
            let mean = vals.iter().sum::<f64>() / n;
            let sd = (vals.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / n.max(1.0)).sqrt();
            if sd > 0.0 {
                3.5 * sd / n.cbrt()
            } else {
                1.0
            }
        }
    };
    let k = (((hi - lo) / width).ceil() as usize).clamp(1, 200);
    let mut out: Vec<(f64, f64, usize)> = (0..k)
        .map(|i| (lo + width * i as f64, lo + width * (i + 1) as f64, 0))
        .collect();
    for v in vals {
        // Intervals closed on the right: (a, b]; the first one also holds its lower bound.
        let i = (((v - lo) / width).ceil() as usize)
            .saturating_sub(1)
            .min(k - 1);
        out[i].2 += 1;
    }
    out
}

/// Quartiles (exclusive or inclusive method), as used by box & whisker charts.
fn quartiles(sorted: &[f64], inclusive: bool) -> (f64, f64, f64) {
    let q = |p: f64| -> f64 {
        let n = sorted.len() as f64;
        let h = if inclusive {
            (n - 1.0) * p
        } else {
            (n + 1.0) * p - 1.0
        };
        let h = h.clamp(0.0, n - 1.0);
        let (i, f) = (h.floor() as usize, h - h.floor());
        let a = sorted[i];
        let b = sorted[(i + 1).min(sorted.len() - 1)];
        a + (b - a) * f
    };
    (q(0.25), q(0.5), q(0.75))
}

/// Squarified treemap layout (Bruls, Huizing, van Wijk) of `values` in `rect`.
fn squarify(values: &[f64], [x, y, w, h]: [f64; 4]) -> Vec<[f64; 4]> {
    let total: f64 = values.iter().sum();
    if total <= 0.0 || values.is_empty() {
        return vec![[x, y, 0.0, 0.0]; values.len()];
    }
    let scale = w * h / total;
    let areas: Vec<f64> = values.iter().map(|v| v.max(0.0) * scale).collect();
    let mut out = vec![[0.0; 4]; values.len()];
    let (mut rx, mut ry, mut rw, mut rh) = (x, y, w, h);
    let mut i = 0;
    while i < areas.len() {
        let side = rw.min(rh).max(1e-9);
        let worst = |row: &[f64]| {
            let s: f64 = row.iter().sum();
            let (mx, mn) = row
                .iter()
                .fold((0.0_f64, f64::MAX), |(a, b), v| (a.max(*v), b.min(*v)));
            if s <= 0.0 || mn <= 0.0 {
                f64::MAX
            } else {
                (side * side * mx / (s * s)).max(s * s / (side * side * mn))
            }
        };
        let mut j = i + 1;
        while j < areas.len() && worst(&areas[i..=j]) <= worst(&areas[i..j]) {
            j += 1;
        }
        let row = &areas[i..j];
        let s: f64 = row.iter().sum();
        let thick = s / side;
        let mut off = 0.0;
        for (k, a) in row.iter().enumerate() {
            let len = if thick > 0.0 { a / thick } else { 0.0 };
            out[i + k] = if rw >= rh {
                [rx, ry + off, thick, len]
            } else {
                [rx + off, ry, len, thick]
            };
            off += len;
        }
        if rw >= rh {
            rx += thick;
            rw -= thick;
        } else {
            ry += thick;
            rh -= thick;
        }
        i = j;
    }
    out
}

/// Draws a chartex chart ([MS-ODRAWXML] 2.24 layouts) in `area`.
#[allow(clippy::too_many_lines, clippy::too_many_arguments)]
fn draw_ex(
    p: &ChartPlot,
    ss: &[Ser],
    [l, t, r, b]: [f64; 4],
    font: f64,
    locale: &Locale,
    color: &ColorFn<'_>,
    label: &mut LabelFn<'_>,
    out: &mut Vec<Command>,
) {
    let Some(s) = ss.first() else { return };
    let vals: Vec<f64> = s.vals.iter().map(|v| v.unwrap_or(0.0)).collect();
    let n = vals.len();
    match p.kind {
        // Waterfall: running total; totals from zero; increases / decreases / totals coloured.
        ChartKind::Waterfall => {
            let mut run = 0.0;
            let mut bars = Vec::with_capacity(n);
            for (i, v) in vals.iter().enumerate() {
                if s.subtotals.contains(&i) {
                    bars.push((0.0, *v, 2));
                    run = *v;
                } else {
                    let from = run;
                    run += v;
                    bars.push((from, run, usize::from(*v < 0.0)));
                }
            }
            let vmin = bars.iter().fold(0.0_f64, |m, (a, z, _)| m.min(a.min(*z)));
            let vmax = bars.iter().fold(0.0_f64, |m, (a, z, _)| m.max(a.max(*z)));
            let (pa, vmin, vmax) =
                value_axis([l, t, r, b], (vmin, vmax), font, locale, false, label, out);
            let [pl, pt, pr, pb] = pa;
            let ph = pb - pt;
            let y = |v: f64| pb - (v - vmin) / (vmax - vmin) * ph;
            let slot = (pr - pl) / n.max(1) as f64;
            for (i, (a, z, k)) in bars.iter().enumerate() {
                let x = pl + i as f64 * slot + slot * 0.2;
                let (top, bot) = (y(a.max(*z)), y(a.min(*z)));
                out.push(rect(
                    x,
                    top,
                    slot * 0.6,
                    (bot - top).max(0.5),
                    color(None, *k),
                ));
                // Connector to the next bar at the running level.
                if i + 1 < n {
                    out.push(hline(x + slot * 0.6, x + slot, y(*z), AXIS, 0.5));
                }
            }
            out.push(hline(pl, pr, y(0.0_f64.clamp(vmin, vmax)), AXIS, 0.75));
            cat_labels(&s.cats, pa, b, font, label, out);
        }
        // Funnel: centred bars, widest at the top, the value on each bar.
        ChartKind::Funnel => {
            let max = vals.iter().fold(0.0_f64, |m, v| m.max(v.abs()));
            let label_w = font * 6.0;
            let (pl, pr) = (l + label_w, r);
            let slot = (b - t) / n.max(1) as f64;
            let mid = f64::midpoint(pl, pr);
            for (i, v) in vals.iter().enumerate() {
                let w = if max > 0.0 {
                    v.abs() / max * (pr - pl)
                } else {
                    0.0
                };
                let y = t + i as f64 * slot;
                out.push(rect(
                    mid - w / 2.0,
                    y + slot * 0.08,
                    w,
                    slot * 0.84,
                    s.color,
                ));
                label(
                    &fmt_num(*v, locale),
                    [mid - w / 2.0, y, mid + w / 2.0, y + slot],
                    Align::Center,
                    font,
                    Rgba(255, 255, 255, 255),
                    out,
                );
                if let Some(c) = s.cats.get(i) {
                    label(c, [l, y, pl - 3.0, y + slot], Align::Right, font, TEXT, out);
                }
            }
        }
        // Histogram (and pareto without categories): binned counts as adjacent columns.
        ChartKind::Histogram | ChartKind::Pareto => {
            let (cats, heights): (Vec<String>, Vec<f64>) =
                if p.kind == ChartKind::Pareto && !s.cats.iter().all(String::is_empty) {
                    // Categories sorted by value, largest first.
                    let mut pairs: Vec<(String, f64)> =
                        s.cats.iter().cloned().zip(vals.iter().copied()).collect();
                    pairs.sort_by(|a, b| b.1.total_cmp(&a.1));
                    pairs.into_iter().unzip()
                } else {
                    let bs = bins(&vals, p.style.as_deref());
                    (
                        bs.iter()
                            .map(|(a, z, _)| {
                                format!("({}, {}]", fmt_num(*a, locale), fmt_num(*z, locale))
                            })
                            .collect(),
                        bs.iter().map(|(_, _, c)| *c as f64).collect(),
                    )
                };
            let right = if p.kind == ChartKind::Pareto {
                r - font * 3.0
            } else {
                r
            };
            let vmax = heights.iter().fold(0.0_f64, |m, v| m.max(*v));
            let (pa, vmin, vmax) = value_axis(
                [l, t, right, b],
                (0.0, vmax),
                font,
                locale,
                false,
                label,
                out,
            );
            let [pl, pt, pr, pb] = pa;
            let ph = pb - pt;
            let k = heights.len().max(1);
            let slot = (pr - pl) / k as f64;
            for (i, h) in heights.iter().enumerate() {
                let top = pb - (h - vmin) / (vmax - vmin) * ph;
                out.push(rect(
                    pl + i as f64 * slot + 0.5,
                    top,
                    slot - 1.0,
                    pb - top,
                    s.color,
                ));
            }
            cat_labels(&cats, pa, b, font, label, out);
            if p.kind == ChartKind::Pareto {
                // Cumulative share on a 0–100 % axis at the right.
                let total: f64 = heights.iter().sum();
                let mut acc = 0.0;
                let pts: Vec<(f64, f64)> = heights
                    .iter()
                    .enumerate()
                    .map(|(i, h)| {
                        acc += h;
                        let f = if total > 0.0 { acc / total } else { 0.0 };
                        (pl + (i as f64 + 0.5) * slot, pb - f * ph)
                    })
                    .collect();
                out.push(polyline(&pts, color(None, 1), 2.0));
                for k in 0..=4 {
                    let f = f64::from(k) / 4.0;
                    let y = pb - f * ph;
                    label(
                        &format!("{}%", k * 25),
                        [pr + 2.0, y - font, r, y + font],
                        Align::Left,
                        font,
                        TEXT,
                        out,
                    );
                }
            }
        }
        // Box & whisker: per category and series, quartile box, median line, mean mark, whiskers to the
        // last values within 1.5 × IQR, outliers as dots.
        ChartKind::BoxWhisker => {
            let inclusive = p.style.as_deref() == Some("inclusive");
            let mut groups: Vec<String> = Vec::new();
            for s in ss {
                for c in &s.cats {
                    if !groups.contains(c) {
                        groups.push(c.clone());
                    }
                }
            }
            if groups.is_empty() {
                groups.push(String::new());
            }
            let all: Vec<f64> = ss
                .iter()
                .flat_map(|s| s.vals.iter().flatten().copied())
                .collect();
            let (Some(lo), Some(hi)) = (
                all.iter().copied().reduce(f64::min),
                all.iter().copied().reduce(f64::max),
            ) else {
                return;
            };
            let (pa, vmin, vmax) = value_axis(
                [l, t, r, b],
                (lo.min(0.0), hi),
                font,
                locale,
                false,
                label,
                out,
            );
            let [pl, pt, pr, pb] = pa;
            let ph = pb - pt;
            let y = |v: f64| pb - (v - vmin) / (vmax - vmin) * ph;
            let slot = (pr - pl) / groups.len() as f64;
            let sub = slot * 0.8 / ss.len().max(1) as f64;
            for (gi, g) in groups.iter().enumerate() {
                for (si, s) in ss.iter().enumerate() {
                    let mut v: Vec<f64> = s
                        .cats
                        .iter()
                        .zip(&s.vals)
                        .filter(|(c, _)| *c == g || g.is_empty())
                        .filter_map(|(_, v)| *v)
                        .collect();
                    if v.is_empty() && g.is_empty() {
                        v = s.vals.iter().flatten().copied().collect();
                    }
                    if v.is_empty() {
                        continue;
                    }
                    v.sort_by(f64::total_cmp);
                    let (q1, med, q3) = quartiles(&v, inclusive);
                    let iqr = q3 - q1;
                    let (wl, wh) = (
                        v.iter()
                            .copied()
                            .find(|x| *x >= q1 - 1.5 * iqr)
                            .unwrap_or(q1),
                        v.iter()
                            .rev()
                            .copied()
                            .find(|x| *x <= q3 + 1.5 * iqr)
                            .unwrap_or(q3),
                    );
                    let mean = v.iter().sum::<f64>() / v.len() as f64;
                    let x0 = pl + gi as f64 * slot + slot * 0.1 + si as f64 * sub;
                    let (bx, bw) = (x0 + sub * 0.15, sub * 0.7);
                    let cx = bx + bw / 2.0;
                    let c = s.color;
                    out.push(vline(cx, y(wh), y(q3), c, 1.0));
                    out.push(vline(cx, y(q1), y(wl), c, 1.0));
                    out.push(hline(cx - bw * 0.25, cx + bw * 0.25, y(wh), c, 1.0));
                    out.push(hline(cx - bw * 0.25, cx + bw * 0.25, y(wl), c, 1.0));
                    let Rgba(cr, cg, cb, _) = c;
                    out.push(rect(
                        bx,
                        y(q3),
                        bw,
                        (y(q1) - y(q3)).max(0.5),
                        Rgba(cr, cg, cb, 90),
                    ));
                    out.push(polyline(
                        &[
                            (bx, y(q3)),
                            (bx + bw, y(q3)),
                            (bx + bw, y(q1)),
                            (bx, y(q1)),
                            (bx, y(q3)),
                        ],
                        c,
                        1.0,
                    ));
                    out.push(hline(bx, bx + bw, y(med), c, 1.5));
                    // Mean as an ×.
                    let (mx, my, d) = (cx, y(mean), bw * 0.12);
                    out.push(polyline(&[(mx - d, my - d), (mx + d, my + d)], c, 1.0));
                    out.push(polyline(&[(mx - d, my + d), (mx + d, my - d)], c, 1.0));
                    for o in v.iter().filter(|x| **x < wl || **x > wh) {
                        out.push(Command::Path {
                            segs: circle(cx, y(*o), 2.0),
                            fill: None,
                            stroke: Some(thin(c)),
                        });
                    }
                }
            }
            cat_labels(&groups, pa, b, font, label, out);
        }
        // Treemap: top-level groups, then their leaves, as squarified rectangles; leaf names inside.
        ChartKind::Treemap => {
            let groups = top_groups(s);
            let sums: Vec<f64> = groups
                .iter()
                .map(|(_, ix)| ix.iter().map(|i| vals[*i].max(0.0)).sum())
                .collect();
            let cells = squarify(&sums, [l, t, r - l, b - t]);
            for (gi, ((_, ix), cell)) in groups.iter().zip(&cells).enumerate() {
                let leaf_vals: Vec<f64> = ix.iter().map(|i| vals[*i].max(0.0)).collect();
                let leaves = squarify(&leaf_vals, *cell);
                let c = color(None, gi);
                for (k, lr) in ix.iter().zip(&leaves) {
                    let [x, y, w, h] = *lr;
                    out.push(rect(x, y, w, h, c));
                    out.push(polyline(
                        &[(x, y), (x + w, y), (x + w, y + h), (x, y + h), (x, y)],
                        Rgba(255, 255, 255, 255),
                        1.0,
                    ));
                    if w > font * 2.5 && h > font * 1.6 {
                        let name = s.cats.get(*k).cloned().unwrap_or_default();
                        label(
                            &name,
                            [x + 2.0, y + 1.0, x + w - 2.0, y + font * 1.6],
                            Align::Left,
                            font,
                            Rgba(255, 255, 255, 255),
                            out,
                        );
                    }
                }
            }
        }
        // Sunburst: one ring per level (outermost level at the centre), angles by leaf sizes.
        ChartKind::Sunburst => {
            let levels: Vec<&Vec<String>> = if s.levels.is_empty() {
                vec![&s.cats]
            } else {
                s.levels.iter().collect()
            };
            let depth = levels.len();
            let total: f64 = vals.iter().map(|v| v.max(0.0)).sum();
            if total <= 0.0 {
                return;
            }
            let (cx, cy) = (f64::midpoint(l, r), f64::midpoint(t, b));
            let rad = ((r - l).min(b - t) / 2.0 - 2.0).max(4.0);
            let ring = rad / depth as f64;
            let groups = top_groups(s);
            let group_of = |i: usize| {
                groups
                    .iter()
                    .position(|(_, ix)| ix.contains(&i))
                    .unwrap_or(0)
            };
            // Ring k (0 = centre) shows level depth-1-k; consecutive leaves with the same label up to that
            // level form one segment.
            for k in 0..depth {
                let lv = depth - 1 - k;
                let mut a = 0.0;
                let mut i = 0;
                while i < n {
                    let key = |j: usize| -> Vec<&str> {
                        (lv..depth)
                            .map(|d| levels[d].get(j).map_or("", String::as_str))
                            .collect()
                    };
                    let mut j = i + 1;
                    while j < n && key(j) == key(i) {
                        j += 1;
                    }
                    let v: f64 = vals[i..j].iter().map(|v| v.max(0.0)).sum();
                    let sw = v / total * std::f64::consts::TAU;
                    if sw > 0.0 {
                        let Rgba(cr, cg, cb, _) = color(None, group_of(i));
                        let alpha = (255 - k as u32 * 40).max(110) as u8;
                        out.push(Command::Path {
                            segs: wedge(cx, cy, ring * (k + 1) as f64, ring * k as f64, a, sw),
                            fill: Some(Paint::Solid {
                                color: Rgba(cr, cg, cb, alpha),
                            }),
                            stroke: Some(thin(Rgba(255, 255, 255, 255))),
                        });
                        if sw > 0.25 {
                            let mid = a + sw / 2.0;
                            let rr = ring * (k as f64 + 0.5);
                            let (x, y) = (cx + rr * mid.sin(), cy - rr * mid.cos());
                            let name = levels[lv].get(i).cloned().unwrap_or_default();
                            label(
                                &name,
                                [x - ring, y - font, x + ring, y + font],
                                Align::Center,
                                font * 0.9,
                                Rgba(255, 255, 255, 255),
                                out,
                            );
                        }
                    }
                    a += sw;
                    i = j;
                }
            }
        }
        _ => {}
    }
}

fn thin(c: Rgba) -> Stroke {
    Stroke {
        width: 0.75,
        color: c,
        dash: vec![],
        cap: "butt".into(),
        join: "round".into(),
    }
}

/// A circle as four cubic Béziers.
fn circle(cx: f64, cy: f64, r: f64) -> Vec<PathSeg> {
    const K: f64 = 0.552_284_75;
    let k = K * r;
    let f = |v: f64| v as f32;
    vec![
        PathSeg::M(f(cx), f(cy - r)),
        PathSeg::C(f(cx + k), f(cy - r), f(cx + r), f(cy - k), f(cx + r), f(cy)),
        PathSeg::C(f(cx + r), f(cy + k), f(cx + k), f(cy + r), f(cx), f(cy + r)),
        PathSeg::C(f(cx - k), f(cy + r), f(cx - r), f(cy + k), f(cx - r), f(cy)),
        PathSeg::C(f(cx - r), f(cy - k), f(cx - k), f(cy - r), f(cx), f(cy - r)),
        PathSeg::Z,
    ]
}

/// §21.2.2.126 ofPieChart: the main pie with the last points gathered into one slice, and those
/// points as a second pie or a stacked bar beside it (split: the last third of the points).
fn draw_of_pie(s: Option<&Ser>, bar: bool, [l, t, r, b]: [f64; 4], out: &mut Vec<Command>) {
    let Some(s) = s else { return };
    let n = s.vals.len();
    if n < 3 {
        draw_pie_plain(Some(s), false, [l, t, r, b], out);
        return;
    }
    let k = n.div_ceil(3);
    let vals: Vec<f64> = s.vals.iter().map(|v| v.unwrap_or(0.0).max(0.0)).collect();
    let other: f64 = vals[n - k..].iter().sum();
    let other_color = Rgba(0xA0, 0xA6, 0xAE, 255);
    let main = Ser {
        name: s.name.clone(),
        cats: Vec::new(),
        vals: vals[..n - k]
            .iter()
            .copied()
            .chain([other])
            .map(Some)
            .collect(),
        color: s.color,
        pts: s.pts[..(n - k).min(s.pts.len())]
            .iter()
            .copied()
            .chain([other_color])
            .collect(),
        sizes: Vec::new(),
        explosion: 0,
        levels: Vec::new(),
        subtotals: Vec::new(),
        trendlines: Vec::new(),
        err_bars: None,
    };
    let split = l + (r - l) * 0.6;
    draw_pie_plain(Some(&main), false, [l, t, split, b], out);
    let total: f64 = vals[n - k..].iter().sum();
    if total <= 0.0 {
        return;
    }
    let (cy, h) = (f64::midpoint(t, b), (b - t) * 0.6);
    let (sx0, sx1) = (split + (r - split) * 0.25, r - 4.0);
    // Connector lines from the gathered slice to the second plot.
    let main_r = ((split - l).min(b - t) / 2.0 - 2.0).max(1.0);
    let edge = f64::midpoint(l, split) + main_r * 0.9;
    out.push(polyline(
        &[(edge, cy - main_r * 0.3), (sx0, cy - h / 2.0)],
        AXIS,
        0.75,
    ));
    out.push(polyline(
        &[(edge, cy + main_r * 0.3), (sx0, cy + h / 2.0)],
        AXIS,
        0.75,
    ));
    let colors: Vec<Rgba> = (n - k..n)
        .map(|i| s.pts.get(i).copied().unwrap_or(PALETTE[i % PALETTE.len()]))
        .collect();
    if bar {
        let bw = ((sx1 - sx0) * 0.5).min(h * 0.4);
        let x = f64::midpoint(sx0, sx1) - bw / 2.0;
        let mut y = cy - h / 2.0;
        for (v, c) in vals[n - k..].iter().zip(&colors) {
            let hh = v / total * h;
            out.push(rect(x, y, bw, hh, *c));
            y += hh;
        }
    } else {
        let sub = Ser {
            name: String::new(),
            cats: Vec::new(),
            vals: vals[n - k..].iter().copied().map(Some).collect(),
            color: s.color,
            pts: colors,
            sizes: Vec::new(),
            explosion: 0,
            levels: Vec::new(),
            subtotals: Vec::new(),
            trendlines: Vec::new(),
            err_bars: None,
        };
        draw_pie_plain(
            Some(&sub),
            false,
            [sx0, cy - h / 2.0, sx1, cy + h / 2.0],
            out,
        );
    }
}

/// §21.2.2.153 radarChart: one spoke per category, rings at the value steps; `marker` adds point
/// marks, `filled` fills each series' polygon.
#[allow(clippy::too_many_arguments)]
fn draw_radar(
    p: &ChartPlot,
    ss: &[Ser],
    [l, t, r, b]: [f64; 4],
    font: f64,
    locale: &Locale,
    label: &mut LabelFn<'_>,
    out: &mut Vec<Command>,
) {
    let ncat = ss.iter().map(|s| s.vals.len()).max().unwrap_or(0);
    if ncat < 3 {
        return;
    }
    let vmax = ss
        .iter()
        .flat_map(|s| s.vals.iter().flatten())
        .fold(0.0_f64, |m, v| m.max(*v));
    let vmin = ss
        .iter()
        .flat_map(|s| s.vals.iter().flatten())
        .fold(0.0_f64, |m, v| m.min(*v));
    let step = nice_step((vmax - vmin).max(1e-9), 4.0);
    let (vmin, vmax) = (
        (vmin / step).floor() * step,
        ((vmax / step).ceil() * step).max(vmin + step),
    );
    let (cx, cy) = (f64::midpoint(l, r), f64::midpoint(t, b));
    let rad = ((r - l).min(b - t) / 2.0 - font * 2.2).max(4.0);
    let ang =
        |i: usize| -std::f64::consts::FRAC_PI_2 + std::f64::consts::TAU * i as f64 / ncat as f64;
    let at = |i: usize, v: f64| {
        let k = (v - vmin) / (vmax - vmin) * rad;
        (cx + k * ang(i).cos(), cy + k * ang(i).sin())
    };
    // Rings and spokes.
    let mut v = vmin;
    let mut guard = 0;
    while v <= vmax + step * 0.5 && guard < 40 {
        let ring: Vec<(f64, f64)> = (0..=ncat).map(|i| at(i % ncat, v)).collect();
        out.push(polyline(&ring, GRID, 0.5));
        let (x, y) = at(0, v);
        label(
            &fmt_num(v, locale),
            [x + 2.0, y - font, x + font * 4.0, y],
            Align::Left,
            font * 0.9,
            TEXT,
            out,
        );
        v += step;
        guard += 1;
    }
    let cats: Vec<String> = (0..ncat)
        .map(|i| {
            ss.iter()
                .find_map(|s| s.cats.get(i).filter(|c| !c.is_empty()).cloned())
                .unwrap_or_else(|| (i + 1).to_string())
        })
        .collect();
    for (i, c) in cats.iter().enumerate() {
        out.push(polyline(&[(cx, cy), at(i, vmax)], AXIS, 0.5));
        let (x, y) = (
            cx + (rad + font * 1.2) * ang(i).cos(),
            cy + (rad + font * 1.2) * ang(i).sin(),
        );
        label(
            c,
            [x - font * 5.0, y - font, x + font * 5.0, y + font],
            Align::Center,
            font,
            TEXT,
            out,
        );
    }
    let style = p.style.as_deref().unwrap_or("standard");
    for s in ss {
        let pts: Vec<(f64, f64)> = (0..ncat)
            .map(|i| at(i, s.vals.get(i).copied().flatten().unwrap_or(vmin)))
            .collect();
        let mut closed = pts.clone();
        closed.push(pts[0]);
        if style == "filled" {
            let Rgba(cr, cg, cb, _) = s.color;
            let mut segs: Vec<PathSeg> = closed
                .iter()
                .enumerate()
                .map(|(i, (x, y))| {
                    if i == 0 {
                        PathSeg::M(*x as f32, *y as f32)
                    } else {
                        PathSeg::L(*x as f32, *y as f32)
                    }
                })
                .collect();
            segs.push(PathSeg::Z);
            out.push(Command::Path {
                segs,
                fill: Some(Paint::Solid {
                    color: Rgba(cr, cg, cb, 140),
                }),
                stroke: None,
            });
        }
        out.push(polyline(&closed, s.color, 1.75));
        if style == "marker" {
            for (x, y) in &pts {
                out.push(rect(x - 2.5, y - 2.5, 5.0, 5.0, s.color));
            }
        }
    }
}

/// Value bands of a surface chart: a nice step over the values (at most 8 bands).
fn surface_bands(ss: &[Ser]) -> Vec<(f64, f64)> {
    let vals: Vec<f64> = ss
        .iter()
        .flat_map(|s| s.vals.iter().flatten().copied())
        .collect();
    let (Some(lo), Some(hi)) = (
        vals.iter().copied().reduce(f64::min),
        vals.iter().copied().reduce(f64::max),
    ) else {
        return Vec::new();
    };
    let step = nice_step((hi - lo).max(1e-9), 5.0);
    let (lo, hi) = (
        (lo / step).floor() * step,
        ((hi / step).ceil() * step).max(lo + step),
    );
    let n = (((hi - lo) / step).round() as usize).clamp(1, 8);
    let step = (hi - lo) / n as f64;
    (0..n)
        .map(|i| (lo + step * i as f64, lo + step * (i + 1) as f64))
        .collect()
}

/// §21.2.2.203 surfaceChart, seen from above (as the contour view): categories across, series down,
/// each cell coloured by the value band of its value.
#[allow(clippy::too_many_arguments)]
fn draw_surface(
    ss: &[Ser],
    bands: &[(f64, f64)],
    [l, t, r, b]: [f64; 4],
    font: f64,
    color: &ColorFn<'_>,
    label: &mut LabelFn<'_>,
    out: &mut Vec<Command>,
) {
    let ncat = ss.iter().map(|s| s.vals.len()).max().unwrap_or(0);
    if ncat == 0 || ss.is_empty() || bands.is_empty() {
        return;
    }
    let (pl, pt, pr, pb) = (l + font * 5.0, t + font * 0.5, r, b - font * 1.8);
    let (cw, ch) = ((pr - pl) / ncat as f64, (pb - pt) / ss.len() as f64);
    let band = |v: f64| {
        bands
            .iter()
            .position(|(a, z)| v >= *a && v <= *z)
            .unwrap_or(if v < bands[0].0 { 0 } else { bands.len() - 1 })
    };
    for (si, s) in ss.iter().enumerate() {
        let y = pb - (si + 1) as f64 * ch;
        label(
            &s.name,
            [l, y, pl - 3.0, y + ch],
            Align::Right,
            font,
            TEXT,
            out,
        );
        for i in 0..ncat {
            if let Some(v) = s.vals.get(i).copied().flatten() {
                out.push(rect(
                    pl + i as f64 * cw,
                    y,
                    cw + 0.3,
                    ch + 0.3,
                    color(None, band(v)),
                ));
            }
        }
    }
    for i in 0..ncat {
        let c = ss
            .iter()
            .find_map(|s| s.cats.get(i).filter(|c| !c.is_empty()).cloned())
            .unwrap_or_else(|| (i + 1).to_string());
        let x = pl + i as f64 * cw;
        label(&c, [x, pb + 1.0, x + cw, b], Align::Center, font, TEXT, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bo_model::sheet::Cell;

    fn wb() -> Workbook {
        let mut wb = Workbook::new();
        let ws = &mut wb.sheets[0];
        ws.set_cell(0, 0, Cell::new(CellValue::Text("Tỉnh".into())));
        ws.set_cell(0, 1, Cell::new(CellValue::Text("2024".into())));
        ws.set_cell(0, 2, Cell::new(CellValue::Text("2025".into())));
        for (i, (n, a, b)) in [("HN", 10.0, 12.0), ("HUE", 5.0, 4.0), ("DN", 7.0, 9.0)]
            .iter()
            .enumerate()
        {
            let r = i as u32 + 1;
            ws.set_cell(r, 0, Cell::new(CellValue::Text((*n).into())));
            ws.set_cell(r, 1, Cell::new(CellValue::Number(*a)));
            ws.set_cell(r, 2, Cell::new(CellValue::Number(*b)));
        }
        wb
    }

    fn draw(wb: &Workbook, c: &Chart) -> (Vec<Command>, Vec<String>) {
        let mut labels = Vec::new();
        let mut out = Vec::new();
        draw_chart(
            wb,
            0,
            c,
            400.0,
            250.0,
            &Locale::VI_VN,
            &mut |t, _, _, _, _, _| labels.push(t.to_owned()),
            &mut out,
        );
        (out, labels)
    }

    #[test]
    fn new_chart_detects_headers_and_categories() {
        let wb = wb();
        let c = new_chart(&wb, 0, CellRange::parse("A1:C4").unwrap(), "column").unwrap();
        assert_eq!(c.plots[0].series.len(), 2);
        let s = &c.plots[0].series[0];
        assert_eq!(s.val_ref.as_deref(), Some("Sheet1!$B$2:$B$4"));
        assert_eq!(s.cat_ref.as_deref(), Some("Sheet1!$A$2:$A$4"));
        assert_eq!(s.name_ref.as_deref(), Some("Sheet1!$B$1"));
        assert_eq!(s.values, vec![Some(10.0), Some(5.0), Some(7.0)]);
        assert_eq!(c.from, CellRef::new(0, 4));
        assert!(c.dirty && c.part.is_empty());
        let (out, labels) = draw(&wb, &c);
        // 2 series × 3 categories of bars (+ background).
        let bars = out
            .iter()
            .filter(|c| matches!(c, Command::FillRect { color, .. } if *color == PALETTE[0] || *color == PALETTE[1]))
            .count();
        assert_eq!(bars, 6 + 2, "bars plus legend swatches");
        assert!(labels.contains(&"HUE".to_owned()) && labels.contains(&"2025".to_owned()));
    }

    #[test]
    fn all_types_draw_and_live_values_follow_cells() {
        let mut wb = wb();
        let mut c = new_chart(&wb, 0, CellRange::parse("A1:C4").unwrap(), "line").unwrap();
        c.title = Some("Doanh thu".into());
        c.x_title = Some("Tỉnh".into());
        c.y_title = Some("Tỷ".into());
        for kind in CHART_TYPES {
            set_chart_type(&mut c, kind);
            let (out, labels) = draw(&wb, &c);
            assert!(out.len() > 5, "{kind}");
            assert!(labels.contains(&"Doanh thu".to_owned()));
        }
        set_chart_type(&mut c, "combo");
        assert_eq!(c.plots.len(), 2);
        assert_eq!(c.plots[1].kind, ChartKind::Line);
        wb.sheets[0].set_cell(1, 1, Cell::new(CellValue::Number(99.0)));
        let (_, _, vals) = series_data(&wb, 0, &c.plots[0].series[0], &Locale::VI_VN);
        assert_eq!(vals[0], Some(99.0));
        wb.sheets[0].charts.push(c);
        refresh_caches(&mut wb, &Locale::VI_VN);
        assert_eq!(
            wb.sheets[0].charts[0].plots[0].series[0].values[0],
            Some(99.0)
        );
        assert_eq!(fmt_num(2.5, &Locale::VI_VN), "2,5");
        assert_eq!(nice_step(470.0, 5.0), 100.0);
    }

    fn tl(kind: &str, order: u32) -> bo_model::sheet::Trendline {
        bo_model::sheet::Trendline {
            kind: kind.into(),
            order,
            disp_eq: true,
            disp_r2: true,
        }
    }

    // source: data generated from the exact functions (y = 2x + 1, y = 2e^{0.5x}, y = 3 + 2 ln x,
    // y = 4x^{1.5}, y = x² − 2x + 3); ECMA-376 Part 1, §21.2.2.212 trendlineType.
    #[test]
    fn trend_lines_recover_their_functions() {
        let close = |a: f64, b: f64| (a - b).abs() < 1e-9;
        let pts =
            |f: &dyn Fn(f64) -> f64, xs: &[f64]| xs.iter().map(|x| (*x, f(*x))).collect::<Vec<_>>();
        let lin = pts(&|x| 2.0 * x + 1.0, &[1.0, 2.0, 3.0, 4.0]);
        let f = Fit::new(&tl("linear", 0), &lin).unwrap();
        assert!(close(f.c[0], 1.0) && close(f.c[1], 2.0), "{:?}", f.c);
        assert!(close(f.r2(&lin), 1.0));
        assert_eq!(f.equation(&Locale::VI_VN), "y = 2x + 1");
        let exp = pts(&|x| 2.0 * (0.5 * x).exp(), &[0.0, 1.0, 2.0, 3.0]);
        let f = Fit::new(&tl("exp", 0), &exp).unwrap();
        assert!(close(f.c[0], 2.0) && close(f.c[1], 0.5), "{:?}", f.c);
        assert_eq!(f.equation(&Locale::VI_VN), "y = 2e^0,5x");
        let log = pts(&|x: f64| 3.0 + 2.0 * x.ln(), &[1.0, 2.0, 4.0, 8.0]);
        let f = Fit::new(&tl("log", 0), &log).unwrap();
        assert!(close(f.c[0], 3.0) && close(f.c[1], 2.0), "{:?}", f.c);
        let pow = pts(&|x: f64| 4.0 * x.powf(1.5), &[1.0, 4.0, 9.0]);
        let f = Fit::new(&tl("power", 0), &pow).unwrap();
        assert!(
            (f.c[0] - 4.0).abs() < 1e-9 && (f.c[1] - 1.5).abs() < 1e-9,
            "{:?}",
            f.c
        );
        let poly = pts(&|x| x * x - 2.0 * x + 3.0, &[0.0, 1.0, 2.0, 3.0, 4.0]);
        let f = Fit::new(&tl("poly", 2), &poly).unwrap();
        assert!(
            close(f.c[0], 3.0) && close(f.c[1], -2.0) && close(f.c[2], 1.0),
            "{:?}",
            f.c
        );
        assert_eq!(f.equation(&Locale::EN_US), "y = 1x^2 − 2x + 3");
        assert!(Fit::new(&tl("movingAvg", 2), &lin).is_some());
        assert_eq!(
            Fit::moving(&lin, 2),
            vec![(2.0, 4.0), (3.0, 6.0), (4.0, 8.0)]
        );
        assert!(Fit::new(&tl("linear", 0), &lin[..1]).is_none());
    }

    // source: ECMA-376 Part 1, §21.2.2.59 errValType (fixed value, percentage, standard deviation of the
    // series, standard error); sample standard deviation of 2, 4, 4, 4, 5, 5, 7, 9 is √(32/7).
    #[test]
    fn error_bar_amounts() {
        let e = |t: &str, v: f64| bo_model::sheet::ErrorBars {
            kind: "both".into(),
            value_type: t.into(),
            value: v,
        };
        let all = [2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0];
        let sd = (32.0_f64 / 7.0).sqrt();
        assert!((err_amount(&e("fixedVal", 1.5), 9.0, &all) - 1.5).abs() < 1e-12);
        assert!((err_amount(&e("percentage", 10.0), 20.0, &all) - 2.0).abs() < 1e-12);
        assert!((err_amount(&e("stdDev", 2.0), 0.0, &all) - 2.0 * sd).abs() < 1e-12);
        assert!((err_amount(&e("stdErr", 0.0), 0.0, &all) - sd / 8.0_f64.sqrt()).abs() < 1e-12);
    }

    #[test]
    fn components_draw_labels_and_tables() {
        let wb = wb();
        let mut c = new_chart(&wb, 0, CellRange::parse("A1:C4").unwrap(), "line").unwrap();
        c.plots[0].series[0].trendlines.push(tl("linear", 0));
        c.plots[0].series[1].err_bars = Some(bo_model::sheet::ErrorBars {
            kind: "both".into(),
            value_type: "fixedVal".into(),
            value: 1.0,
        });
        c.axes.data_table = true;
        let (_, labels) = draw(&wb, &c);
        // Series HN 10, HUE 5, DN 7 (2024): the R² label is shown; the table repeats the values.
        assert!(labels.iter().any(|l| l.starts_with("R² = ")), "{labels:?}");
        assert!(labels.iter().any(|l| l.starts_with("y = ")), "{labels:?}");
        assert!(
            labels.iter().filter(|l| *l == "12").count() >= 1,
            "{labels:?}"
        );
        // Logarithmic value axis: labels at the powers of ten.
        let mut c2 = new_chart(&wb, 0, CellRange::parse("A1:C4").unwrap(), "column").unwrap();
        c2.axes.log_base = Some(10.0);
        let (_, labels) = draw(&wb, &c2);
        assert!(
            labels.contains(&"1".to_owned()) && labels.contains(&"10".to_owned()),
            "{labels:?}"
        );
        // Secondary axis: the second series' plot gets its own labels at the right.
        let mut c3 = new_chart(&wb, 0, CellRange::parse("A1:C4").unwrap(), "combo").unwrap();
        c3.plots[1].secondary = true;
        let (out, _) = draw(&wb, &c3);
        assert!(out.len() > 10);
    }

    #[test]
    fn three_d_chart_types() {
        let wb = wb();
        for kind in [
            "column3D",
            "cylinder3D",
            "bar3D",
            "line3D",
            "stackedArea3D",
            "pie3D",
            "surface3D",
        ] {
            let c = new_chart(&wb, 0, CellRange::parse("A1:C4").unwrap(), kind).unwrap();
            assert!(c.plots[0].three_d, "{kind}");
            let v = c.axes.view3d.clone().unwrap();
            // §21.2.2.228: a new pie is tilted 30°, the others seen 15° from above and turned 20°.
            assert_eq!(
                (v.rot_x, v.rot_y),
                if kind == "pie3D" { (30, 0) } else { (15, 20) },
                "{kind}"
            );
            let (out, labels) = draw(&wb, &c);
            assert!(out.len() > 8, "{kind}");
            if kind != "pie3D" {
                assert!(
                    labels.iter().any(|l| l == "HN" || l == "2024"),
                    "{kind}: {labels:?}"
                );
            }
        }
        let c = new_chart(&wb, 0, CellRange::parse("A1:C4").unwrap(), "column3D").unwrap();
        assert_eq!(c.plots[0].grouping.as_deref(), Some("standard"));
        let c = new_chart(&wb, 0, CellRange::parse("A1:C4").unwrap(), "pyramid3D").unwrap();
        assert_eq!(c.plots[0].style.as_deref(), Some("pyramid"));
        let mut c = c;
        set_chart_type(&mut c, "column");
        assert!(!c.plots[0].three_d && c.axes.view3d.is_none());
    }

    #[test]
    fn every_chart_type_is_named_back() {
        let wb = wb();
        for kind in CHART_TYPES {
            let c = new_chart(&wb, 0, CellRange::parse("A1:C4").unwrap(), kind).unwrap();
            assert_eq!(type_name(&c), *kind);
        }
    }
}
