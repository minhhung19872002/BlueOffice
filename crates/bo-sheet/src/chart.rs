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
    CellRange, CellRef, CellValue, Chart, ChartKind, ChartPlot, ChartSeries, Workbook,
};
use bo_render::{Command, Paint, PathSeg, Rgba, Stroke};

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
    Left,
    Center,
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
    "bar",
    "line",
    "area",
    "pie",
    "doughnut",
    "scatter",
    "combo",
];

fn plot_for(kind: &str) -> ChartPlot {
    let (k, dir, grouping) = match kind {
        "bar" => (ChartKind::Bar, Some("bar"), Some("clustered")),
        "stackedColumn" => (ChartKind::Bar, Some("col"), Some("stacked")),
        "line" => (ChartKind::Line, None, Some("standard")),
        "area" => (ChartKind::Area, None, Some("standard")),
        "pie" => (ChartKind::Pie, None, None),
        "doughnut" => (ChartKind::Doughnut, None, None),
        "scatter" => (ChartKind::Scatter, None, None),
        _ => (ChartKind::Bar, Some("col"), Some("clustered")),
    };
    ChartPlot {
        kind: k,
        bar_dir: dir.map(str::to_owned),
        grouping: grouping.map(str::to_owned),
        series: Vec::new(),
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
        chart.plots = vec![p];
    }
    chart.dirty = true;
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
    let cat_col = range.cols() > 1
        && (range.first.row + u32::from(header)..=range.last.row)
            .all(|r| is_text(r, range.first.col) || ws.cell(r, range.first.col).is_none());
    let r0 = range.first.row + u32::from(header);
    let c0 = range.first.col + u32::from(cat_col);
    if r0 > range.last.row || c0 > range.last.col {
        return None;
    }
    let name = &ws.name;
    let cat_ref = cat_col.then(|| {
        abs_ref(
            name,
            CellRange::new(
                CellRef::new(r0, range.first.col),
                CellRef::new(range.last.row, range.first.col),
            ),
        )
    });
    let mut plot = plot_for(if kind == "combo" { "column" } else { kind });
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
    let mut data = chart.clone();
    refresh_one(wb, sheet, &mut data);
    Some(data)
}

fn refresh_one(wb: &Workbook, sheet: usize, chart: &mut Chart) {
    for p in &mut chart.plots {
        for s in &mut p.series {
            let (name, cats, vals) = series_data(wb, sheet, s, &Locale::EN_US);
            s.name = name;
            s.categories = cats;
            s.values = vals;
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
}

/// Draws `chart` into a `w × h` box at the origin (points).
#[allow(clippy::too_many_lines, clippy::too_many_arguments)]
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
    out.push(rect(0.0, 0.0, w, h, Rgba(255, 255, 255, 255)));
    for c in [
        hline(0.0, w, 0.25, FRAME, 0.5),
        hline(0.0, w, h - 0.25, FRAME, 0.5),
        vline(0.25, 0.0, h, FRAME, 0.5),
        vline(w - 0.25, 0.0, h, FRAME, 0.5),
    ] {
        out.push(c);
    }
    let font = (h / 24.0).clamp(6.0, 11.0);
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
        for s in &p.series {
            let (name, cats, vals) = series_data(wb, sheet, s, locale);
            v.push(Ser {
                name: name.unwrap_or_else(|| format!("{}", idx + 1)),
                cats,
                vals,
                color: PALETTE[idx % PALETTE.len()],
            });
            idx += 1;
        }
        plots.push((p, v));
    }
    let pie = plots
        .first()
        .is_some_and(|(p, _)| matches!(p.kind, ChartKind::Pie | ChartKind::Doughnut));
    // Legend (right or bottom).
    let entries: Vec<(String, Rgba)> = if pie {
        plots
            .first()
            .and_then(|(_, s)| s.first())
            .map(|s| {
                s.cats
                    .iter()
                    .enumerate()
                    .map(|(i, c)| (c.clone(), PALETTE[i % PALETTE.len()]))
                    .collect()
            })
            .unwrap_or_default()
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
    if pie {
        if let Some((p, s)) = plots.first() {
            draw_pie(
                s.first(),
                p.kind == ChartKind::Doughnut,
                [6.0, top, right, bottom],
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
    draw_xy(&plots, [left, top, right, bottom], font, locale, label, out);
}

#[allow(clippy::too_many_lines)]
fn draw_xy(
    plots: &[(&ChartPlot, Vec<Ser>)],
    [l, t, r, b]: [f64; 4],
    font: f64,
    locale: &Locale,
    label: &mut LabelFn<'_>,
    out: &mut Vec<Command>,
) {
    let scatter = plots.iter().all(|(p, _)| p.kind == ChartKind::Scatter);
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
        return;
    }
    let val = |s: &Ser, i: usize| s.vals.get(i).copied().flatten().unwrap_or(0.0);
    // Value range over all plots.
    let (mut vmin, mut vmax) = (0.0_f64, 0.0_f64);
    for (p, ss) in plots {
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
    if vmax <= vmin {
        vmax = vmin + 1.0;
    }
    let step = nice_step(vmax - vmin, 5.0);
    let vmax = (vmax / step).ceil() * step;
    let vmin = (vmin / step).floor() * step;
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
    let (pl, pt, pr, pb) = if horizontal {
        (l + label_w * 1.4, t, r, b - label_h)
    } else {
        (l + label_w, t + font * 0.5, r, b - label_h)
    };
    let (pw, ph) = ((pr - pl).max(1.0), (pb - pt).max(1.0));
    let vpos = |v: f64| (v - vmin) / (vmax - vmin);
    // Gridlines + value labels.
    let mut v = vmin;
    let mut guard = 0;
    while v <= vmax + step * 0.5 && guard < 60 {
        let f = vpos(v);
        let text = fmt_num(v, locale);
        if horizontal {
            let x = pl + f * pw;
            out.push(vline(x, pt, pb, GRID, 0.5));
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
            out.push(hline(pl, pr, y, GRID, 0.5));
            label(
                &text,
                [l, y - label_h / 2.0, pl - 3.0, y + label_h / 2.0],
                Align::Right,
                font,
                TEXT,
                out,
            );
        }
        v += step;
        guard += 1;
    }
    let zero = vpos(0.0_f64.clamp(vmin, vmax));
    if horizontal {
        out.push(vline(pl + zero * pw, pt, pb, AXIS, 0.75));
    } else {
        out.push(hline(pl, pr, pb - zero * ph, AXIS, 0.75));
    }
    let slot = if horizontal { ph } else { pw } / ncat as f64;
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
        match p.kind {
            ChartKind::Bar => {
                let stacked = stacked_of(p);
                let gap = 1.5 / nbar_groups;
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
                        if horizontal {
                            out.push(rect(
                                pl + lo * pw,
                                pb - off - bar,
                                (hi - lo) * pw,
                                bar,
                                s.color,
                            ));
                        } else {
                            out.push(rect(pl + off, pb - hi * ph, bar, (hi - lo) * ph, s.color));
                        }
                    }
                }
            }
            ChartKind::Line | ChartKind::Area => {
                let stacked = stacked_of(p);
                let mut base = vec![0.0; ncat];
                for s in ss {
                    let pts: Vec<(f64, f64)> = (0..ncat)
                        .map(|i| {
                            let v = val(s, i) + if stacked { base[i] } else { 0.0 };
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
                        out.push(Command::Path {
                            segs,
                            fill: Some(Paint::Solid {
                                color: Rgba(r, g, bl, 200),
                            }),
                            stroke: None,
                        });
                    } else {
                        out.push(polyline(&pts, s.color, 2.0));
                    }
                    if stacked {
                        for (i, b) in base.iter_mut().enumerate() {
                            *b += val(s, i);
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
            _ => {}
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

fn draw_pie(s: Option<&Ser>, doughnut: bool, [l, t, r, b]: [f64; 4], out: &mut Vec<Command>) {
    let Some(s) = s else { return };
    let vals: Vec<f64> = s.vals.iter().map(|v| v.unwrap_or(0.0).max(0.0)).collect();
    let total: f64 = vals.iter().sum();
    if total <= 0.0 {
        return;
    }
    let rad = ((r - l).min(b - t) / 2.0 - 2.0).max(1.0);
    let (cx, cy) = (f64::midpoint(l, r), f64::midpoint(t, b));
    let inner = if doughnut { rad * 0.5 } else { 0.0 };
    let mut a = 0.0;
    for (i, v) in vals.iter().enumerate() {
        let sw = v / total * std::f64::consts::TAU;
        if sw <= 0.0 {
            continue;
        }
        out.push(Command::Path {
            segs: wedge(cx, cy, rad, inner, a, sw),
            fill: Some(Paint::Solid {
                color: PALETTE[i % PALETTE.len()],
            }),
            stroke: Some(Stroke {
                width: 0.75,
                color: Rgba(255, 255, 255, 255),
                dash: vec![],
                cap: "butt".into(),
                join: "round".into(),
            }),
        });
        a += sw;
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
}
