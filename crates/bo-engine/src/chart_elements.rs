//! Chart element options over the JSON bridge (CORE-CHART-005): what the "Format chart" pane of
//! BlueSheet, BlueSlide and BlueDoc reads ([`info`]) and changes ([`apply`], the `elements` field
//! of the shared chart design command, [`crate::chart_design::DesignSpec`]).
//!
//! The options are those of the DrawingML chart elements (ECMA-376 Part 1, §21.2.2): titles and
//! legend with their overlay, axes (`c:scaling`, units, `c:crosses`, `c:dispUnits`, `c:numFmt`,
//! tick marks, `c:tickLblPos`, gridlines, `c:dateAx`), series (`c:spPr`, `c:marker`, `c:smooth`,
//! `c:explosion`, the secondary axis), `c:gapWidth` / `c:overlap`, data labels (`c:dLbls`,
//! `c:dLbl`), `c:trendline`, `c:errBars`, `c:dTable`, `c:dropLines`, `c:hiLowLines`,
//! `c:upDownBars`. In a patch an absent field keeps its value and `null` returns it to automatic.

// A patch field has three states: absent (unchanged), `null` (back to automatic) and a value.
#![allow(clippy::option_option)]

use bo_model::chart_style::DataLabels;
use bo_model::sheet::{
    AxisOptions, Chart, ChartColor, ChartKind, ChartPlot, ErrorBars, Marker, Trendline,
};
use serde::{Deserialize, Deserializer};
use serde_json::{json, Value};

/// `null` → `Some(None)` (back to automatic); an absent field stays `None` (unchanged).
fn nullable<'de, T, D>(d: D) -> Result<Option<Option<T>>, D::Error>
where
    T: Deserialize<'de>,
    D: Deserializer<'de>,
{
    Option::<T>::deserialize(d).map(Some)
}

type Num = Option<Option<f64>>;
type Text = Option<Option<String>>;

/// Chart title (§21.2.2.210 title).
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct TitlePatch {
    /// Title text; empty removes the title.
    pub text: Option<String>,
    /// overlay.
    pub overlay: Option<bool>,
}

/// Legend (§21.2.2.93 legend).
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LegendPatch {
    /// `r`, `l`, `t`, `b`, `tr`, or `none`.
    pub pos: Option<String>,
    /// overlay.
    pub overlay: Option<bool>,
}

/// Chart area / plot area (§21.2.2.197 spPr).
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct AreaPatch {
    /// Fill colour (`#RRGGBB` or a theme colour name).
    #[serde(deserialize_with = "nullable")]
    pub fill: Text,
    /// Border colour (chart area).
    #[serde(deserialize_with = "nullable")]
    pub line: Text,
    /// §21.2.2.156 roundedCorners (chart area).
    pub rounded: Option<bool>,
}

/// One axis.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct AxisPatch {
    /// §21.2.2.111 min.
    #[serde(deserialize_with = "nullable")]
    pub min: Num,
    /// §21.2.2.106 max.
    #[serde(deserialize_with = "nullable")]
    pub max: Num,
    /// majorUnit.
    #[serde(deserialize_with = "nullable")]
    pub major_unit: Num,
    /// minorUnit.
    #[serde(deserialize_with = "nullable")]
    pub minor_unit: Num,
    /// §21.2.2.100 logBase (2..=1000).
    #[serde(deserialize_with = "nullable")]
    pub log_base: Num,
    /// orientation `maxMin`.
    pub reverse: Option<bool>,
    /// §21.2.2.33 crosses: `min` / `max` (`null` = automatic).
    #[serde(deserialize_with = "nullable")]
    pub crosses: Text,
    /// crossesAt.
    #[serde(deserialize_with = "nullable")]
    pub crosses_at: Num,
    /// builtInUnit.
    #[serde(deserialize_with = "nullable")]
    pub disp_unit: Text,
    /// dispUnitsLbl.
    pub disp_unit_label: Option<bool>,
    /// numFmt format code.
    #[serde(deserialize_with = "nullable")]
    pub num_fmt: Text,
    /// majorTickMark (`in`, `cross`, `none`; `null` / `out` = automatic).
    #[serde(deserialize_with = "nullable")]
    pub major_tick: Text,
    /// minorTickMark (`in`, `out`, `cross`; `null` / `none` = automatic).
    #[serde(deserialize_with = "nullable")]
    pub minor_tick: Text,
    /// tickLblPos (`high`, `low`, `none`; `null` / `nextTo` = automatic).
    #[serde(deserialize_with = "nullable")]
    pub label_pos: Text,
    /// §21.2.2.100 majorGridlines.
    pub major_grid: Option<bool>,
    /// minorGridlines.
    pub minor_grid: Option<bool>,
    /// delete.
    pub deleted: Option<bool>,
    /// baseTimeUnit of a date axis (`days`, `months`, `years`).
    #[serde(deserialize_with = "nullable")]
    pub base_unit: Text,
    /// Axis title; empty or `null` removes it.
    #[serde(deserialize_with = "nullable")]
    pub title: Text,
    /// Category axis only: `category` (§21.2.2.25 catAx) or `date` (§21.2.2.39 dateAx).
    pub kind: Option<String>,
}

/// The axes of the chart.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct AxesPatch {
    /// Category axis (X axis of a scatter chart).
    pub cat: Option<AxisPatch>,
    /// Primary value axis.
    pub val: Option<AxisPatch>,
    /// Secondary value axis.
    pub val2: Option<AxisPatch>,
}

/// Line of the gridlines (§21.2.2.100 majorGridlines spPr).
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct GridPatch {
    /// Line colour.
    #[serde(deserialize_with = "nullable")]
    pub color: Text,
    /// Line width in points.
    #[serde(deserialize_with = "nullable")]
    pub width: Num,
}

/// Data table (§21.2.2.54 dTable).
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct TablePatch {
    /// Show the table.
    pub show: Option<bool>,
    /// showKeys.
    pub keys: Option<bool>,
}

/// Options of a chart group (`c:barChart`, `c:lineChart`, …).
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct GroupPatch {
    /// Index of the group; absent = every group the option applies to.
    pub plot: Option<usize>,
    /// §21.2.2.75 gapWidth (0..=500; the chart's bar groups share it).
    #[serde(deserialize_with = "nullable")]
    pub gap_width: Option<Option<u32>>,
    /// overlap (−100..=100).
    #[serde(deserialize_with = "nullable")]
    pub overlap: Option<Option<i32>>,
    /// §21.2.2.53 dropLines.
    pub drop_lines: Option<bool>,
    /// §21.2.2.80 hiLowLines.
    pub hi_low_lines: Option<bool>,
    /// §21.2.2.218 upDownBars.
    pub up_down_bars: Option<bool>,
}

/// The data label of one point.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct PointLabelPatch {
    /// Zero-based point.
    pub point: usize,
    /// Its label; `null` removes the point's own label (it follows the series again).
    pub labels: Option<DataLabels>,
}

/// One series (index across the chart's groups, in order).
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SeriesPatch {
    /// Zero-based series.
    pub index: usize,
    /// Fill colour.
    #[serde(deserialize_with = "nullable")]
    pub fill: Text,
    /// Line colour.
    #[serde(deserialize_with = "nullable")]
    pub line: Text,
    /// Line width in points (0.25..=12).
    #[serde(deserialize_with = "nullable")]
    pub line_width: Num,
    /// §21.2.2.106 marker.
    #[serde(deserialize_with = "nullable")]
    pub marker: Option<Option<Marker>>,
    /// smooth.
    pub smooth: Option<bool>,
    /// §21.2.2.65 explosion (percent, pies).
    pub explosion: Option<u32>,
    /// Plot the series on the secondary value axis.
    pub secondary: Option<bool>,
    /// §21.2.2.49 dLbls of the series; `null` = the chart's.
    #[serde(deserialize_with = "nullable")]
    pub labels: Option<Option<DataLabels>>,
    /// dLbl of single points.
    pub point_labels: Vec<PointLabelPatch>,
    /// §21.2.2.211 trendline: the series' trend lines (the whole list).
    pub trendlines: Option<Vec<Trendline>>,
    /// §21.2.2.58 errBars; `null` removes them.
    #[serde(deserialize_with = "nullable")]
    pub error_bars: Option<Option<ErrorBars>>,
}

/// A change to the elements of a chart.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ElementsPatch {
    /// Chart title.
    pub title: Option<TitlePatch>,
    /// Legend.
    pub legend: Option<LegendPatch>,
    /// Chart area.
    pub chart_area: Option<AreaPatch>,
    /// Plot area.
    pub plot_area: Option<AreaPatch>,
    /// Axes.
    pub axes: Option<AxesPatch>,
    /// Gridline format.
    pub gridlines: Option<GridPatch>,
    /// Data table.
    pub data_table: Option<TablePatch>,
    /// Chart groups.
    pub groups: Vec<GroupPatch>,
    /// Series.
    pub series: Vec<SeriesPatch>,
    /// Data labels of every series; `null` removes them.
    #[serde(deserialize_with = "nullable")]
    pub labels: Option<Option<DataLabels>>,
}

fn color_text(c: Option<&ChartColor>) -> Value {
    match c {
        Some(ChartColor::Rgb(v)) => json!(format!("#{:06X}", v & 0xFF_FFFF)),
        Some(ChartColor::Scheme(n)) => json!(n),
        None => Value::Null,
    }
}

/// Theme colour names a patch may use (ST_SchemeColorVal).
const SCHEME: &[&str] = &[
    "accent1", "accent2", "accent3", "accent4", "accent5", "accent6", "tx1", "tx2", "bg1", "bg2",
    "dk1", "dk2", "lt1", "lt2", "hlink", "folHlink",
];

/// A colour of a patch: `#RRGGBB` (or `RRGGBB`), else a theme colour name (`accent1`, `tx1`, …).
fn parse_color(s: &str) -> Result<ChartColor, String> {
    let t = s.trim();
    let hex = t.strip_prefix('#').unwrap_or(t);
    if hex.len() == 6 && hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return u32::from_str_radix(hex, 16)
            .map(ChartColor::Rgb)
            .map_err(|_| format!("bad colour '{s}'"));
    }
    if SCHEME.contains(&t) {
        Ok(ChartColor::Scheme(t.to_owned()))
    } else {
        Err(format!("bad colour '{s}'"))
    }
}

fn set_color(slot: &mut Option<ChartColor>, v: &Text) -> Result<(), String> {
    match v {
        Some(Some(s)) => *slot = Some(parse_color(s)?),
        Some(None) => *slot = None,
        None => {}
    }
    Ok(())
}

fn one_of(v: &str, allowed: &[&str], what: &str) -> Result<(), String> {
    if allowed.contains(&v) {
        Ok(())
    } else {
        Err(format!("bad {what} '{v}'"))
    }
}

/// An enumerated option: `null` and the value the writer emits for "automatic" clear it.
fn set_choice(
    slot: &mut Option<String>,
    v: &Text,
    allowed: &[&str],
    auto: &str,
    what: &str,
) -> Result<(), String> {
    match v {
        Some(Some(s)) if s == auto => *slot = None,
        Some(Some(s)) => {
            one_of(s, allowed, what)?;
            *slot = Some(s.clone());
        }
        Some(None) => *slot = None,
        None => {}
    }
    Ok(())
}

fn set_num(
    slot: &mut Option<f64>,
    v: Num,
    ok: impl Fn(f64) -> bool,
    what: &str,
) -> Result<(), String> {
    match v {
        Some(Some(x)) if x.is_finite() && ok(x) => *slot = Some(x),
        Some(Some(x)) => return Err(format!("bad {what} '{x}'")),
        Some(None) => *slot = None,
        None => {}
    }
    Ok(())
}

fn horizontal(c: &Chart) -> bool {
    !c.plots.is_empty()
        && c.plots
            .iter()
            .all(|p| p.kind == ChartKind::Bar && p.bar_dir.as_deref() == Some("bar"))
}

fn scatter(c: &Chart) -> bool {
    !c.plots.is_empty()
        && c.plots
            .iter()
            .all(|p| matches!(p.kind, ChartKind::Scatter | ChartKind::Bubble))
}

/// The chart has category / value axes the options apply to (2-D bar, line, area, scatter, bubble
/// and stock groups).
fn has_axes(c: &Chart) -> bool {
    !c.plots.is_empty()
        && c.plots.iter().all(|p| {
            !p.three_d
                && matches!(
                    p.kind,
                    ChartKind::Bar
                        | ChartKind::Line
                        | ChartKind::Area
                        | ChartKind::Scatter
                        | ChartKind::Bubble
                        | ChartKind::Stock
                )
        })
}

fn pie(c: &Chart) -> bool {
    c.plots.first().is_some_and(|p| {
        matches!(
            p.kind,
            ChartKind::Pie | ChartKind::Doughnut | ChartKind::OfPie
        )
    })
}

fn takes_trendline(p: &ChartPlot) -> bool {
    !p.three_d
        && matches!(
            p.kind,
            ChartKind::Bar
                | ChartKind::Line
                | ChartKind::Area
                | ChartKind::Scatter
                | ChartKind::Bubble
        )
}

const TICKS: &[&str] = &["cross", "in", "none", "out"];
const UNITS: &[&str] = &[
    "hundreds",
    "thousands",
    "tenThousands",
    "hundredThousands",
    "millions",
    "tenMillions",
    "hundredMillions",
    "billions",
    "trillions",
];
const LABEL_POS: &[&str] = &[
    "bestFit", "b", "ctr", "inBase", "inEnd", "l", "outEnd", "r", "t",
];
const SYMBOLS: &[&str] = &[
    "circle", "dash", "diamond", "dot", "none", "plus", "square", "star", "triangle", "x",
];

fn check_labels(d: &DataLabels) -> Result<DataLabels, String> {
    let mut d = d.clone();
    if let Some(p) = &d.pos {
        one_of(p, LABEL_POS, "label position")?;
    }
    d.num_fmt = d.num_fmt.filter(|f| !f.trim().is_empty());
    d.separator = d.separator.filter(|s| !s.is_empty());
    if d.num_fmt.as_ref().is_some_and(|f| f.len() > 255)
        || d.separator.as_ref().is_some_and(|s| s.chars().count() > 16)
    {
        return Err("bad data label".into());
    }
    Ok(d)
}

#[allow(clippy::too_many_lines)]
fn apply_axis(c: &mut Chart, which: &str, p: &AxisPatch) -> Result<(), String> {
    let (hor, xy) = (horizontal(c), scatter(c));
    // Scale: the primary value axis keeps it in the chart's own fields.
    {
        let (mut min, mut max, mut log) = match which {
            "val" => (c.axes.min, c.axes.max, c.axes.log_base),
            "cat" => (c.axes.cat.min, c.axes.cat.max, c.axes.cat.log_base),
            _ => (c.axes.val2.min, c.axes.val2.max, c.axes.val2.log_base),
        };
        set_num(&mut min, p.min, |_| true, "minimum")?;
        set_num(&mut max, p.max, |_| true, "maximum")?;
        set_num(
            &mut log,
            p.log_base,
            |b| (2.0..=1000.0).contains(&b),
            "logarithm base",
        )?;
        if let (Some(a), Some(z)) = (min, max) {
            if a >= z {
                return Err("the minimum must be below the maximum".into());
            }
        }
        if log.is_some() && (min.is_some_and(|v| v <= 0.0) || max.is_some_and(|v| v <= 0.0)) {
            return Err("a logarithmic axis needs positive bounds".into());
        }
        if which == "cat" && !xy && (min.is_some() || max.is_some() || log.is_some()) {
            return Err("a category axis has no scale".into());
        }
        match which {
            "val" => (c.axes.min, c.axes.max, c.axes.log_base) = (min, max, log),
            "cat" => (c.axes.cat.min, c.axes.cat.max, c.axes.cat.log_base) = (min, max, log),
            _ => (c.axes.val2.min, c.axes.val2.max, c.axes.val2.log_base) = (min, max, log),
        }
    }
    if let Some(k) = &p.kind {
        if which != "cat" || xy {
            return Err("only a category axis can be a date axis".into());
        }
        one_of(k, &["category", "date"], "axis type")?;
        c.axes.date = k == "date";
    }
    if let Some(on) = p.major_grid {
        match which {
            "val" => c.format.hide_val_grid = !on,
            "cat" => c.format.cat_grid = on,
            _ => return Err("the secondary axis has no gridlines".into()),
        }
    }
    if let Some(t) = &p.title {
        let t = t
            .clone()
            .map(|t| t.trim().to_owned())
            .filter(|t| !t.is_empty());
        match (which, hor) {
            ("cat", false) | ("val", true) => c.x_title = t,
            ("cat", true) | ("val", false) => c.y_title = t,
            _ => c.axes.val2.title = t,
        }
    }
    let date = c.axes.date;
    let o: &mut AxisOptions = match which {
        "val" => &mut c.axes.val,
        "cat" => &mut c.axes.cat,
        _ => &mut c.axes.val2,
    };
    set_num(&mut o.major_unit, p.major_unit, |v| v > 0.0, "major unit")?;
    set_num(&mut o.minor_unit, p.minor_unit, |v| v > 0.0, "minor unit")?;
    set_num(&mut o.crosses_at, p.crosses_at, |_| true, "crossing value")?;
    if let Some(r) = p.reverse {
        o.reverse = r;
    }
    if let Some(d) = p.deleted {
        o.deleted = d;
    }
    if let Some(g) = p.minor_grid {
        if which == "val2" {
            return Err("the secondary axis has no gridlines".into());
        }
        o.minor_grid = g;
    }
    // The secondary value axis crosses at the maximum by default, the others at zero.
    let auto_cross = if which == "val2" { "max" } else { "autoZero" };
    set_choice(
        &mut o.crosses,
        &p.crosses,
        &["min", "max", "autoZero"],
        auto_cross,
        "crossing",
    )?;
    if o.crosses_at.is_some() && p.crosses_at.is_some() {
        o.crosses = None;
    } else if p.crosses.is_some() {
        o.crosses_at = None;
    }
    if which == "cat" && !xy {
        if p.disp_unit.as_ref().is_some_and(Option::is_some) {
            return Err("a category axis has no display units".into());
        }
    } else {
        set_choice(
            &mut o.disp_unit,
            &p.disp_unit,
            UNITS,
            "none",
            "display unit",
        )?;
        if let Some(l) = p.disp_unit_label {
            o.disp_unit_label = l;
        }
        if o.disp_unit.is_none() {
            o.disp_unit_label = false;
        }
    }
    if let Some(f) = &p.num_fmt {
        let f = f.clone().filter(|f| !f.trim().is_empty());
        if f.as_ref().is_some_and(|f| f.len() > 255) {
            return Err("bad number format".into());
        }
        o.num_fmt = f;
    }
    set_choice(&mut o.major_tick, &p.major_tick, TICKS, "out", "tick mark")?;
    set_choice(&mut o.minor_tick, &p.minor_tick, TICKS, "none", "tick mark")?;
    set_choice(
        &mut o.label_pos,
        &p.label_pos,
        &["high", "low", "nextTo", "none"],
        "nextTo",
        "label position",
    )?;
    if which == "cat" && date {
        set_choice(
            &mut o.base_unit,
            &p.base_unit,
            &["days", "months", "years"],
            "days",
            "base unit",
        )?;
    } else {
        if p.base_unit.as_ref().is_some_and(Option::is_some) {
            return Err("only a date axis has a base unit".into());
        }
        o.base_unit = None;
    }
    Ok(())
}

/// The (group, series) position of series `index` counted across the groups.
fn locate(c: &Chart, index: usize) -> Option<(usize, usize)> {
    let mut n = 0;
    for (pi, p) in c.plots.iter().enumerate() {
        if index < n + p.series.len() {
            return Some((pi, index - n));
        }
        n += p.series.len();
    }
    None
}

/// Series without a colour of their own get the colour they are written with now (the chart's
/// palette, else BlueOffice's), so that moving a series between groups does not recolour it.
fn pin_colors(c: &mut Chart) {
    let palette = c.format.palette.clone();
    let mut i = 0usize;
    for p in &mut c.plots {
        for s in &mut p.series {
            if s.fill.is_none() && s.line.is_none() {
                s.fill = Some(match palette.len() {
                    0 => {
                        let bo_render::Rgba(r, g, b, _) =
                            bo_sheet::chart::PALETTE[i % bo_sheet::chart::PALETTE.len()];
                        ChartColor::Rgb(u32::from(r) << 16 | u32::from(g) << 8 | u32::from(b))
                    }
                    n => palette[i % n].clone(),
                });
            }
            i += 1;
        }
    }
}

/// Moves series `index` to (or off) the secondary value axis: into a group of its kind on that
/// axis, created beside its own when there is none.
fn set_secondary(c: &mut Chart, index: usize, on: bool) -> Result<(), String> {
    let (pi, si) = locate(c, index).ok_or("no such series")?;
    if c.plots[pi].secondary == on {
        return Ok(());
    }
    if !has_axes(c) || c.plots[pi].kind == ChartKind::Stock {
        return Err("this chart type has no secondary axis".into());
    }
    let primary_left = c
        .plots
        .iter()
        .enumerate()
        .filter(|(i, p)| !p.secondary || *i == pi)
        .map(|(i, p)| p.series.len() - usize::from(i == pi && !p.secondary))
        .sum::<usize>();
    if on && primary_left == 0 {
        return Err("the chart needs a series on the primary axis".into());
    }
    let same = |a: &ChartPlot, b: &ChartPlot| {
        a.kind == b.kind && a.bar_dir == b.bar_dir && a.grouping == b.grouping && !a.three_d
    };
    let src = c.plots[pi].clone();
    let dst = c
        .plots
        .iter()
        .enumerate()
        .position(|(i, p)| i != pi && p.secondary == on && same(p, &src));
    match dst {
        // A group of its kind is on that axis already: the series joins it.
        Some(di) => {
            pin_colors(c);
            let s = c.plots[pi].series.remove(si);
            c.plots[di].series.push(s);
            if c.plots[pi].series.is_empty() {
                c.plots.remove(pi);
            }
        }
        None if src.series.len() == 1 => c.plots[pi].secondary = on,
        None => {
            pin_colors(c);
            let s = c.plots[pi].series.remove(si);
            let mut group = src;
            group.series = vec![s];
            group.secondary = on;
            c.plots.insert(pi + 1, group);
        }
    }
    Ok(())
}

fn clean_trendline(t: &Trendline, n_points: usize) -> Result<Trendline, String> {
    let mut t = t.clone();
    one_of(
        &t.kind,
        &["linear", "exp", "log", "poly", "power", "movingAvg"],
        "trend line type",
    )?;
    t.order = match t.kind.as_str() {
        "poly" => t.order.clamp(2, 6),
        // A moving average needs fewer points than the series has.
        "movingAvg" => t.order.clamp(2, (n_points.max(3) - 1) as u32),
        _ => 0,
    };
    let pos = |v: Option<f64>| v.filter(|v| v.is_finite() && *v > 0.0);
    t.forward = pos(t.forward);
    t.backward = pos(t.backward);
    t.intercept = t
        .intercept
        .filter(|v| v.is_finite() && matches!(t.kind.as_str(), "linear" | "poly" | "exp"));
    if t.kind == "exp" && t.intercept.is_some_and(|v| v <= 0.0) {
        return Err("an exponential trend line needs a positive intercept".into());
    }
    if t.kind == "movingAvg" {
        (t.forward, t.backward, t.intercept) = (None, None, None);
        (t.disp_eq, t.disp_r2) = (false, false);
    }
    t.name = t
        .name
        .map(|n| n.trim().to_owned())
        .filter(|n| !n.is_empty());
    Ok(t)
}

fn clean_err_bars(e: &ErrorBars, xy: bool) -> Result<ErrorBars, String> {
    let mut e = e.clone();
    one_of(&e.kind, &["both", "plus", "minus"], "error bar direction")?;
    one_of(
        &e.value_type,
        &["fixedVal", "percentage", "stdDev", "stdErr", "cust"],
        "error amount",
    )?;
    if !e.value.is_finite() || e.value < 0.0 {
        return Err("bad error amount".into());
    }
    e.dir = e.dir.filter(|d| xy && d == "x");
    let clean = |f: Option<String>| {
        f.map(|f| f.trim().trim_start_matches('=').to_owned())
            .filter(|f| !f.is_empty())
    };
    e.plus_ref = clean(e.plus_ref);
    e.minus_ref = clean(e.minus_ref);
    if e.value_type == "cust" {
        let empty = |f: &Option<String>, v: &[Option<f64>]| f.is_none() && v.is_empty();
        if empty(&e.plus_ref, &e.plus) && empty(&e.minus_ref, &e.minus) {
            return Err("custom error bars need their amounts".into());
        }
        for v in e.plus.iter_mut().chain(e.minus.iter_mut()) {
            *v = v.filter(|x| x.is_finite());
        }
    } else {
        e.plus.clear();
        e.minus.clear();
        e.plus_ref = None;
        e.minus_ref = None;
    }
    Ok(e)
}

#[allow(clippy::too_many_lines)]
fn apply_series(c: &mut Chart, p: &SeriesPatch) -> Result<(), String> {
    // The secondary axis first: it may move the series to another group.
    let mut index = p.index;
    if let Some(on) = p.secondary {
        let name = locate(c, index)
            .map(|(pi, si)| c.plots[pi].series[si].clone())
            .ok_or("no such series")?;
        set_secondary(c, index, on)?;
        // The series may have a new place in the order.
        index = c
            .plots
            .iter()
            .flat_map(|p| &p.series)
            .position(|s| {
                s.val_ref == name.val_ref && s.name == name.name && s.values == name.values
            })
            .unwrap_or(index);
    }
    let (pi, si) = locate(c, index).ok_or("no such series")?;
    let plot = c.plots[pi].clone();
    let xy = matches!(plot.kind, ChartKind::Scatter | ChartKind::Bubble);
    let line_like = matches!(
        plot.kind,
        ChartKind::Line | ChartKind::Scatter | ChartKind::Radar
    );
    let s = &mut c.plots[pi].series[si];
    set_color(&mut s.fill, &p.fill)?;
    set_color(&mut s.line, &p.line)?;
    match p.line_width {
        Some(Some(w)) if (0.25..=12.0).contains(&w) && line_like => {
            s.line_w = Some((w * 12_700.0).round() as u32);
        }
        Some(Some(_)) => return Err("bad line width".into()),
        Some(None) => s.line_w = None,
        None => {}
    }
    match &p.marker {
        Some(Some(m)) => {
            if !line_like {
                return Err("this chart type has no markers".into());
            }
            one_of(&m.symbol, SYMBOLS, "marker")?;
            s.marker = Some(Marker {
                symbol: m.symbol.clone(),
                size: m.size.map(|v| v.clamp(2, 72)),
            });
        }
        Some(None) => s.marker = None,
        None => {}
    }
    if let Some(on) = p.smooth {
        if on && !(plot.kind == ChartKind::Line && !plot.three_d || plot.kind == ChartKind::Scatter)
        {
            return Err("only lines can be smoothed".into());
        }
        s.smooth = on;
    }
    if let Some(e) = p.explosion {
        if !matches!(
            plot.kind,
            ChartKind::Pie | ChartKind::Doughnut | ChartKind::OfPie
        ) {
            return Err("only pie slices can be pulled out".into());
        }
        s.explosion = e.min(400);
    }
    match &p.labels {
        Some(Some(d)) => s.labels = Some(check_labels(d)?),
        Some(None) => s.labels = None,
        None => {}
    }
    for pl in &p.point_labels {
        if pl.point >= s.values.len().max(1) {
            return Err("no such data point".into());
        }
        s.point_labels.retain(|(i, _)| *i != pl.point);
        if let Some(d) = &pl.labels {
            s.point_labels.push((pl.point, check_labels(d)?));
        }
        s.point_labels.sort_by_key(|(i, _)| *i);
    }
    if let Some(list) = &p.trendlines {
        if !list.is_empty() && !takes_trendline(&plot) {
            return Err("this chart type has no trend lines".into());
        }
        if list.len() > 8 {
            return Err("too many trend lines".into());
        }
        let n = s.values.len();
        s.trendlines = list
            .iter()
            .map(|t| clean_trendline(t, n))
            .collect::<Result<_, _>>()?;
    }
    match &p.error_bars {
        Some(Some(e)) => {
            if !takes_trendline(&plot) {
                return Err("this chart type has no error bars".into());
            }
            s.err_bars = Some(clean_err_bars(e, xy)?);
        }
        Some(None) => s.err_bars = None,
        None => {}
    }
    Ok(())
}

fn apply_group(c: &mut Chart, g: &GroupPatch) -> Result<(), String> {
    if let Some(i) = g.plot {
        if i >= c.plots.len() {
            return Err("no such chart group".into());
        }
    }
    match g.gap_width {
        Some(Some(w)) if w <= 500 => c.format.gap_width = Some(w).filter(|w| *w != 150),
        Some(Some(_)) => return Err("bad gap width".into()),
        Some(None) => c.format.gap_width = None,
        None => {}
    }
    let mut touched = false;
    let wanted = g.overlap.is_some()
        || g.drop_lines.is_some()
        || g.hi_low_lines.is_some()
        || g.up_down_bars.is_some();
    for (i, p) in c.plots.iter_mut().enumerate() {
        if g.plot.is_some_and(|k| k != i) || p.three_d {
            continue;
        }
        let stacked = matches!(p.grouping.as_deref(), Some("stacked" | "percentStacked"));
        if let (Some(o), ChartKind::Bar) = (g.overlap, &p.kind) {
            if o.is_some_and(|v| !(-100..=100).contains(&v)) {
                return Err("bad overlap".into());
            }
            // The default of the grouping is not kept.
            p.overlap = o.filter(|v| *v != if stacked { 100 } else { 0 });
            touched = true;
        }
        if let (Some(on), ChartKind::Line | ChartKind::Area) = (g.drop_lines, &p.kind) {
            p.drop_lines = on;
            touched = true;
        }
        if let (Some(on), ChartKind::Line) = (g.hi_low_lines, &p.kind) {
            p.hi_low_lines = on;
            touched = true;
        }
        if let (Some(on), ChartKind::Line | ChartKind::Stock) = (g.up_down_bars, &p.kind) {
            p.flag = on;
            touched = true;
        }
    }
    if wanted && !touched {
        return Err("this chart type does not have that element".into());
    }
    Ok(())
}

/// Applies `patch` to `c`. The error says what the chart cannot take; the chart is unchanged then.
pub fn apply(c: &mut Chart, patch: &ElementsPatch) -> Result<(), String> {
    if c.plots.first().is_some_and(|p| p.kind.is_ex()) {
        return Err("this chart type has no element options".into());
    }
    let mut x = c.clone();
    if let Some(t) = &patch.title {
        if let Some(text) = &t.text {
            x.title = Some(text.trim().to_owned()).filter(|t| !t.is_empty());
        }
        if let Some(o) = t.overlay {
            x.title_overlay = o;
        }
    }
    if let Some(l) = &patch.legend {
        if let Some(pos) = &l.pos {
            one_of(pos, &["r", "l", "t", "b", "tr", "none"], "legend position")?;
            x.legend = (pos != "none").then(|| pos.clone());
        }
        if let Some(o) = l.overlay {
            x.legend_overlay = o;
        }
    }
    if let Some(a) = &patch.chart_area {
        set_color(&mut x.format.chart_fill, &a.fill)?;
        set_color(&mut x.format.chart_line, &a.line)?;
        if let Some(r) = a.rounded {
            x.format.rounded = r;
        }
    }
    if let Some(a) = &patch.plot_area {
        set_color(&mut x.format.plot_fill, &a.fill)?;
    }
    if let Some(g) = &patch.gridlines {
        set_color(&mut x.format.grid_color, &g.color)?;
        match g.width {
            Some(Some(w)) if (0.25..=6.0).contains(&w) => {
                x.format.grid_w = Some((w * 12_700.0).round() as u32);
            }
            Some(Some(_)) => return Err("bad line width".into()),
            Some(None) => x.format.grid_w = None,
            None => {}
        }
    }
    if let Some(a) = &patch.axes {
        if !has_axes(&x) {
            return Err("this chart type has no axis options".into());
        }
        for (which, p) in [("cat", &a.cat), ("val", &a.val), ("val2", &a.val2)] {
            if let Some(p) = p {
                if which == "val2" && !x.plots.iter().any(|p| p.secondary) {
                    return Err("the chart has no secondary axis".into());
                }
                apply_axis(&mut x, which, p)?;
            }
        }
    }
    if let Some(t) = &patch.data_table {
        if !has_axes(&x) || scatter(&x) {
            return Err("this chart type has no data table".into());
        }
        if let Some(on) = t.show {
            x.axes.data_table = on;
        }
        if let Some(k) = t.keys {
            x.axes.table_no_keys = !k;
        }
    }
    for g in &patch.groups {
        apply_group(&mut x, g)?;
    }
    match &patch.labels {
        Some(Some(d)) => {
            let d = check_labels(d)?;
            // The chart's labels replace those of the series.
            for s in x.plots.iter_mut().flat_map(|p| &mut p.series) {
                s.labels = None;
                s.point_labels.clear();
            }
            x.format.data_labels = d.any().then_some(d);
        }
        Some(None) => {
            for s in x.plots.iter_mut().flat_map(|p| &mut p.series) {
                s.labels = None;
                s.point_labels.clear();
            }
            x.format.data_labels = None;
        }
        None => {}
    }
    for s in &patch.series {
        apply_series(&mut x, s)?;
    }
    // A chart without a group on the secondary axis has no secondary axis options.
    if !x.plots.iter().any(|p| p.secondary) {
        x.axes.val2 = AxisOptions::default();
    }
    x.dirty = true;
    *c = x;
    Ok(())
}

fn kind_name(k: &ChartKind) -> &'static str {
    match k {
        ChartKind::Bar => "bar",
        ChartKind::Line => "line",
        ChartKind::Area => "area",
        ChartKind::Pie => "pie",
        ChartKind::Doughnut => "doughnut",
        ChartKind::Scatter => "scatter",
        ChartKind::Radar => "radar",
        ChartKind::Bubble => "bubble",
        ChartKind::Stock => "stock",
        ChartKind::Surface => "surface",
        ChartKind::OfPie => "ofPie",
        _ => "other",
    }
}

fn axis_info(c: &Chart, which: &str) -> Value {
    let (hor, xy) = (horizontal(c), scatter(c));
    let o = match which {
        "val" => &c.axes.val,
        "cat" => &c.axes.cat,
        _ => &c.axes.val2,
    };
    let mut v = serde_json::to_value(o).unwrap_or_else(|_| json!({}));
    let (min, max, log) = match which {
        "val" => (c.axes.min, c.axes.max, c.axes.log_base),
        _ => (o.min, o.max, o.log_base),
    };
    let title = match (which, hor) {
        ("cat", false) | ("val", true) => c.x_title.clone(),
        ("cat", true) | ("val", false) => c.y_title.clone(),
        _ => c.axes.val2.title.clone(),
    };
    let kind = match which {
        "cat" if xy => "value",
        "cat" if c.axes.date => "date",
        "cat" => "category",
        _ => "value",
    };
    if let Some(m) = v.as_object_mut() {
        m.insert("min".into(), json!(min));
        m.insert("max".into(), json!(max));
        m.insert("logBase".into(), json!(log));
        m.insert("title".into(), json!(title));
        m.insert("kind".into(), json!(kind));
        m.insert(
            "majorGrid".into(),
            json!(match which {
                "val" => !c.format.hide_val_grid,
                "cat" => c.format.cat_grid,
                _ => false,
            }),
        );
    }
    v
}

fn trendline_info(t: &Trendline) -> Value {
    json!({
        "kind": t.kind,
        "order": t.order,
        "dispEq": t.disp_eq,
        "dispR2": t.disp_r2,
        "name": t.name,
        "forward": t.forward,
        "backward": t.backward,
        "intercept": t.intercept,
    })
}

fn err_bars_info(e: &ErrorBars) -> Value {
    json!({
        "kind": e.kind,
        "valueType": e.value_type,
        "value": e.value,
        "noEndCap": e.no_end_cap,
        "dir": e.dir,
        "plus": e.plus,
        "minus": e.minus,
        "plusRef": e.plus_ref,
        "minusRef": e.minus_ref,
    })
}

/// The elements of chart `c` and their options, as the "Format chart" pane shows them. `null`
/// for chart types without element options (chartex charts).
#[must_use]
pub fn info(c: &Chart) -> Value {
    if c.plots.is_empty() || c.plots.first().is_some_and(|p| p.kind.is_ex()) {
        return Value::Null;
    }
    let axes = has_axes(c);
    let secondary = c.plots.iter().any(|p| p.secondary);
    let fmt = &c.format;
    let mut index = 0usize;
    let mut series = Vec::new();
    let mut groups = Vec::new();
    for (pi, p) in c.plots.iter().enumerate() {
        let stacked = matches!(p.grouping.as_deref(), Some("stacked" | "percentStacked"));
        let line_like = matches!(
            p.kind,
            ChartKind::Line | ChartKind::Scatter | ChartKind::Radar
        );
        groups.push(json!({
            "plot": pi,
            "kind": kind_name(&p.kind),
            "threeD": p.three_d,
            "stacked": stacked,
            "secondary": p.secondary,
            "gapWidth": fmt.gap_width.unwrap_or(150),
            "overlap": p.overlap.unwrap_or(if stacked { 100 } else { 0 }),
            "dropLines": p.drop_lines,
            "hiLowLines": p.hi_low_lines || p.kind == ChartKind::Stock,
            "upDownBars": matches!(p.kind, ChartKind::Line | ChartKind::Stock) && p.flag,
        }));
        for s in &p.series {
            // What the series is drawn with: its own marker, else the chart format's.
            let marker = s.marker.clone().or_else(|| match p.kind {
                ChartKind::Line | ChartKind::Radar => Some(Marker {
                    symbol: if fmt.markers { "circle" } else { "none" }.into(),
                    size: fmt.markers.then_some(6),
                }),
                ChartKind::Scatter => Some(Marker {
                    symbol: "circle".into(),
                    size: Some(5),
                }),
                _ => None,
            });
            series.push(json!({
                "index": index,
                "plot": pi,
                "kind": kind_name(&p.kind),
                "name": s.name.clone().unwrap_or_default(),
                "points": s.values.len(),
                "categories": s.categories.iter().take(200).collect::<Vec<_>>(),
                "secondary": p.secondary,
                "fill": color_text(s.fill.as_ref()),
                "line": color_text(s.line.as_ref()),
                "lineWidth": line_like.then(|| {
                    f64::from(s.line_w.or(fmt.line_w).unwrap_or(28_575)) / 12_700.0
                }),
                "marker": marker,
                "smooth": s.smooth,
                "explosion": s.explosion,
                "labels": s.labels,
                "pointLabels": s.point_labels.iter().map(|(i, d)| json!({ "point": i, "labels": d })).collect::<Vec<_>>(),
                "trendlines": s.trendlines.iter().map(trendline_info).collect::<Vec<_>>(),
                "errorBars": s.err_bars.as_ref().map(err_bars_info),
                "canTrend": takes_trendline(p),
                "canSecondary": axes && p.kind != ChartKind::Stock,
            }));
            index += 1;
        }
    }
    json!({
        "has": {
            "axes": axes,
            "scatter": scatter(c),
            "horizontal": horizontal(c),
            "pie": pie(c),
            "secondary": secondary,
            "dataTable": axes && !scatter(c),
            "bars": c.plots.iter().any(|p| p.kind == ChartKind::Bar && !p.three_d),
            "lines": c.plots.iter().any(|p| p.kind == ChartKind::Line && !p.three_d),
            "dropLines": c.plots.iter().any(|p| matches!(p.kind, ChartKind::Line | ChartKind::Area) && !p.three_d),
            "upDownBars": c.plots.iter().any(|p| matches!(p.kind, ChartKind::Line | ChartKind::Stock) && !p.three_d),
        },
        "title": { "text": c.title.clone().unwrap_or_default(), "overlay": c.title_overlay },
        "legend": {
            "pos": c.legend.clone().unwrap_or_else(|| "none".into()),
            "overlay": c.legend_overlay,
        },
        "chartArea": {
            "fill": color_text(fmt.chart_fill.as_ref()),
            "line": color_text(fmt.chart_line.as_ref()),
            "rounded": fmt.rounded,
        },
        "plotArea": { "fill": color_text(fmt.plot_fill.as_ref()) },
        "gridlines": {
            "color": color_text(fmt.grid_color.as_ref()),
            "width": fmt.grid_w.map(|w| f64::from(w) / 12_700.0),
        },
        "axes": if axes {
            json!({
                "cat": axis_info(c, "cat"),
                "val": axis_info(c, "val"),
                "val2": if secondary { axis_info(c, "val2") } else { Value::Null },
            })
        } else {
            Value::Null
        },
        "dataTable": { "show": c.axes.data_table, "keys": !c.axes.table_no_keys },
        "groups": groups,
        "series": series,
        "labels": fmt.data_labels,
    })
}

/// Per-series element options (trend lines, error bars, labels, marker, line) carried from the
/// series `old` to those of `c` by position — for edits that rebuild the series from a data grid.
pub fn carry_series_options(c: &mut Chart, old: &[bo_model::sheet::ChartSeries]) {
    for (s, o) in c.plots.iter_mut().flat_map(|p| &mut p.series).zip(old) {
        s.trendlines.clone_from(&o.trendlines);
        s.err_bars.clone_from(&o.err_bars);
        s.labels.clone_from(&o.labels);
        s.point_labels = o
            .point_labels
            .iter()
            .filter(|(i, _)| *i < s.values.len())
            .cloned()
            .collect();
        s.marker.clone_from(&o.marker);
        s.line_w = o.line_w;
        s.smooth = o.smooth;
    }
}
