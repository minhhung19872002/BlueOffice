//! Sparklines ([MS-XLSX] `x14:sparklineGroup`): small line, column and win/loss charts drawn
//! inside a cell, and the edits behind Insert › Sparklines and the sparkline tab.
//!
//! Geometry (BlueSheet's own layout, not taken from another product): the plot box is the cell
//! rectangle inset by [`PAD`] points on every side.
//! * Line: `n` points spread evenly from the left to the right edge of the box (a single point
//!   sits in the middle); with a date axis, by date. `y` maps the axis minimum to the bottom and
//!   the maximum to the top; values beyond custom bounds are clamped to the box.
//! * Column: `n` equal slots, each bar [`BAR`] of its slot wide and centred, drawn from the
//!   baseline (zero, clamped to the axis range) to the value. Automatic bounds of column
//!   sparklines include zero so every bar grows from zero.
//! * Win/loss (`stacked`): positive values fill the upper half of the box, negative values the
//!   lower half, zero draws nothing; the axis bounds do not apply.
//!
//! Point highlighting precedence: first, last, high, low, negative, then the plain series /
//! marker colour.

use crate::cells::{quote_sheet, resolve_ref};
use crate::color::resolve_color;
use crate::geometry::Rect;
use bo_model::sheet::{
    CellRange, CellRef, CellValue, SheetColor, Sparkline, SparklineAxis, SparklineColors,
    SparklineEmpty, SparklineGroup, SparklineType, ThemeColors, Workbook, Worksheet,
};
use bo_render::{Command, Paint, PathSeg, Rgba, Stroke};
use serde::Deserialize;

/// Inset of the plot box inside the cell (points).
pub const PAD: f64 = 2.0;
/// Bar width as a fraction of its slot (column and win/loss).
pub const BAR: f64 = 0.8;
/// Default line weight (points; [MS-XLSX] `lineWeight` default).
pub const DEFAULT_WEIGHT: f64 = 0.75;
/// Width of the horizontal axis line (points).
pub const AXIS_WEIGHT: f64 = 0.75;

/// BlueSheet's default sparkline colours (`RRGGBB`), written into new groups.
pub mod defaults {
    /// Series (line / bars).
    pub const SERIES: &str = "2E6DB4";
    /// Negative points.
    pub const NEGATIVE: &str = "D64545";
    /// Horizontal axis.
    pub const AXIS: &str = "000000";
    /// Line markers.
    pub const MARKERS: &str = "1F4E8C";
    /// First and last points.
    pub const ENDS: &str = "F28C28";
    /// Highest point.
    pub const HIGH: &str = "2FA36B";
    /// Lowest point.
    pub const LOW: &str = "D64545";
}

fn hex(h: &str) -> Rgba {
    let v = u32::from_str_radix(h, 16).unwrap_or(0);
    Rgba((v >> 16) as u8, (v >> 8) as u8, v as u8, 255)
}

/// The colours a group draws with (theme colours resolved, defaults filled in).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Palette {
    /// Series.
    pub series: Rgba,
    /// Negative points.
    pub negative: Rgba,
    /// Axis.
    pub axis: Rgba,
    /// Markers.
    pub markers: Rgba,
    /// First point.
    pub first: Rgba,
    /// Last point.
    pub last: Rgba,
    /// Highest point.
    pub high: Rgba,
    /// Lowest point.
    pub low: Rgba,
}

impl Palette {
    /// Colours of `c` with `theme`.
    pub fn of(c: &SparklineColors, theme: Option<&ThemeColors>) -> Palette {
        let pick = |c: &Option<SheetColor>, d: &str| {
            c.as_ref()
                .and_then(|c| resolve_color(c, theme))
                .unwrap_or_else(|| hex(d))
        };
        Palette {
            series: pick(&c.series, defaults::SERIES),
            negative: pick(&c.negative, defaults::NEGATIVE),
            axis: pick(&c.axis, defaults::AXIS),
            markers: pick(&c.markers, defaults::MARKERS),
            first: pick(&c.first, defaults::ENDS),
            last: pick(&c.last, defaults::ENDS),
            high: pick(&c.high, defaults::HIGH),
            low: pick(&c.low, defaults::LOW),
        }
    }
}

/// Default colours of a new group.
pub fn default_colors() -> SparklineColors {
    let c = |h: &str| Some(SheetColor::rgb(h));
    SparklineColors {
        series: c(defaults::SERIES),
        negative: c(defaults::NEGATIVE),
        axis: c(defaults::AXIS),
        markers: c(defaults::MARKERS),
        first: c(defaults::ENDS),
        last: c(defaults::ENDS),
        high: c(defaults::HIGH),
        low: c(defaults::LOW),
    }
}

// ---------------------------------------------------------------------------
// Data
// ---------------------------------------------------------------------------

fn hidden(ws: &Worksheet, at: CellRef) -> bool {
    ws.rows.get(&at.row).is_some_and(|r| r.hidden) || ws.col_info(at.col).is_some_and(|c| c.hidden)
}

/// Numbers of a range in row-major order (`None` = empty or not a number); rows and columns
/// that are hidden are left out unless `include_hidden`.
fn range_numbers(
    wb: &Workbook,
    sheet: usize,
    text: &str,
    include_hidden: bool,
) -> Option<Vec<Option<f64>>> {
    let (si, r) = resolve_ref(wb, sheet, text)?;
    let ws = wb.sheets.get(si)?;
    // Bounded by the size of the plot: a sparkline over a whole column is not useful.
    let count = u64::from(r.last.row - r.first.row + 1) * u64::from(r.last.col - r.first.col + 1);
    if count > 100_000 {
        return None;
    }
    let mut out = Vec::new();
    for row in r.first.row..=r.last.row {
        for col in r.first.col..=r.last.col {
            let at = CellRef::new(row, col);
            if !include_hidden && hidden(ws, at) {
                continue;
            }
            out.push(match ws.cell(row, col).map(|c| &c.value) {
                Some(CellValue::Number(n)) if n.is_finite() => Some(*n),
                _ => None,
            });
        }
    }
    Some(out)
}

/// The values of sparkline `s` of group `g` (empty cells as `None`).
pub fn values(wb: &Workbook, sheet: usize, g: &SparklineGroup, s: &Sparkline) -> Vec<Option<f64>> {
    s.formula
        .as_deref()
        .and_then(|f| range_numbers(wb, sheet, f, g.display_hidden))
        .unwrap_or_default()
}

/// Dates of the group's date axis, when it has one.
pub fn dates(wb: &Workbook, sheet: usize, g: &SparklineGroup) -> Option<Vec<Option<f64>>> {
    if !g.date_axis {
        return None;
    }
    range_numbers(wb, sheet, g.date_range.as_deref()?, g.display_hidden)
}

/// Values with the empty-cell rule applied: `zero` turns empty cells into 0.
fn effective(g: &SparklineGroup, v: &[Option<f64>]) -> Vec<Option<f64>> {
    match g.empty_as {
        SparklineEmpty::Zero => v.iter().map(|x| Some(x.unwrap_or(0.0))).collect(),
        _ => v.to_vec(),
    }
}

fn min_max(v: &[Option<f64>]) -> Option<(f64, f64)> {
    v.iter().flatten().fold(None, |acc, &x| match acc {
        None => Some((x, x)),
        Some((lo, hi)) => Some((lo.min(x), hi.max(x))),
    })
}

/// Vertical axis bounds of sparkline `index` of group `g` on `sheet`.
pub fn axis_bounds(wb: &Workbook, sheet: usize, g: &SparklineGroup, index: usize) -> (f64, f64) {
    let own = g
        .sparklines
        .get(index)
        .map(|s| effective(g, &values(wb, sheet, g, s)))
        .unwrap_or_default();
    let all: Vec<Option<f64>> =
        if g.min_axis == SparklineAxis::Group || g.max_axis == SparklineAxis::Group {
            g.sparklines
                .iter()
                .flat_map(|s| effective(g, &values(wb, sheet, g, s)))
                .collect()
        } else {
            Vec::new()
        };
    bounds(g, &own, &all)
}

/// Axis bounds from the sparkline's own values and the whole group's values.
pub fn bounds(g: &SparklineGroup, own: &[Option<f64>], group: &[Option<f64>]) -> (f64, f64) {
    let zero = g.kind == SparklineType::Column;
    let auto = |v: &[Option<f64>]| {
        let (lo, hi) = min_max(v).unwrap_or((0.0, 0.0));
        if zero {
            (lo.min(0.0), hi.max(0.0))
        } else {
            (lo, hi)
        }
    };
    let (own_lo, own_hi) = auto(own);
    let (grp_lo, grp_hi) = auto(group);
    let lo = match g.min_axis {
        SparklineAxis::Individual => own_lo,
        SparklineAxis::Group => grp_lo,
        SparklineAxis::Custom => g.manual_min.unwrap_or(own_lo),
    };
    let hi = match g.max_axis {
        SparklineAxis::Individual => own_hi,
        SparklineAxis::Group => grp_hi,
        SparklineAxis::Custom => g.manual_max.unwrap_or(own_hi),
    };
    (lo, hi)
}

// ---------------------------------------------------------------------------
// Drawing
// ---------------------------------------------------------------------------

fn fill_rect(out: &mut Vec<Command>, x: f64, y: f64, w: f64, h: f64, color: Rgba) {
    if w > 0.0 && h > 0.0 {
        out.push(Command::FillRect {
            x: x as f32,
            y: y as f32,
            w: w as f32,
            h: h as f32,
            color,
        });
    }
}

fn stroke(color: Rgba, width: f64) -> Stroke {
    Stroke {
        width: width as f32,
        color,
        dash: Vec::new(),
        cap: "round".into(),
        join: "round".into(),
    }
}

/// Radius of a line marker for a line of `weight` points.
pub fn marker_radius(weight: f64) -> f64 {
    1.5 + weight / 2.0
}

fn marker(out: &mut Vec<Command>, cx: f64, cy: f64, r: f64, color: Rgba) {
    let k = 0.552_284_75 * r;
    let (cx, cy, r, k) = (cx as f32, cy as f32, r as f32, k as f32);
    out.push(Command::Path {
        segs: vec![
            PathSeg::M(cx + r, cy),
            PathSeg::C(cx + r, cy + k, cx + k, cy + r, cx, cy + r),
            PathSeg::C(cx - k, cy + r, cx - r, cy + k, cx - r, cy),
            PathSeg::C(cx - r, cy - k, cx - k, cy - r, cx, cy - r),
            PathSeg::C(cx + k, cy - r, cx + r, cy - k, cx + r, cy),
            PathSeg::Z,
        ],
        fill: Some(Paint::Solid { color }),
        stroke: None,
    });
}

/// Highlight colour of point `i` (value `v`), if a flag of the group marks it.
fn highlight(
    g: &SparklineGroup,
    p: &Palette,
    i: usize,
    v: f64,
    ends: (Option<usize>, Option<usize>),
    data: (f64, f64),
) -> Option<Rgba> {
    if g.first && ends.0 == Some(i) {
        Some(p.first)
    } else if g.last && ends.1 == Some(i) {
        Some(p.last)
    } else if g.high && v == data.1 {
        Some(p.high)
    } else if g.low && v == data.0 {
        Some(p.low)
    } else if g.negative && v < 0.0 {
        Some(p.negative)
    } else {
        None
    }
}

/// Drawing commands of one sparkline in the cell rectangle `cell` (points).
/// `values` are the raw values (empty = `None`), `dates` the date-axis positions,
/// `axis` the vertical bounds (see [`axis_bounds`]).
pub fn draw(
    cell: Rect,
    g: &SparklineGroup,
    values: &[Option<f64>],
    dates: Option<&[Option<f64>]>,
    axis: (f64, f64),
    palette: &Palette,
) -> Vec<Command> {
    let mut out = Vec::new();
    let (x0, y0) = (cell.x + PAD, cell.y + PAD);
    let (w, h) = (cell.w - 2.0 * PAD, cell.h - 2.0 * PAD);
    let vals = effective(g, values);
    let n = vals.len();
    if w <= 0.0 || h <= 0.0 || n == 0 {
        return out;
    }
    let Some(data) = min_max(&vals) else {
        return out;
    };
    let first = vals.iter().position(Option::is_some);
    let last = vals.iter().rposition(Option::is_some);
    let mirror = |x: f64| {
        if g.right_to_left {
            x0 + w - (x - x0)
        } else {
            x
        }
    };
    let (lo, hi) = axis;
    let y_of = |v: f64| {
        if hi > lo {
            let t = ((v - lo) / (hi - lo)).clamp(0.0, 1.0);
            y0 + h - t * h
        } else {
            y0 + h / 2.0
        }
    };
    match g.kind {
        SparklineType::Line => {
            // x positions: evenly spread, or by date.
            let xs: Vec<Option<f64>> = match dates.and_then(|d| {
                let (dlo, dhi) = min_max(d)?;
                Some((d, dlo, dhi))
            }) {
                Some((d, dlo, dhi)) => (0..n)
                    .map(|i| {
                        let t = d.get(i).copied().flatten()?;
                        Some(if dhi > dlo {
                            x0 + w * (t - dlo) / (dhi - dlo)
                        } else {
                            x0 + w / 2.0
                        })
                    })
                    .collect(),
                None => (0..n)
                    .map(|i| {
                        Some(if n == 1 {
                            x0 + w / 2.0
                        } else {
                            x0 + w * i as f64 / (n - 1) as f64
                        })
                    })
                    .collect(),
            };
            let pts: Vec<Option<(f64, f64)>> = vals
                .iter()
                .zip(&xs)
                .map(|(v, x)| Some((mirror((*x)?), y_of((*v)?))))
                .collect();
            if g.display_x_axis && lo <= 0.0 && hi >= 0.0 && hi > lo {
                let y = y_of(0.0);
                axis_line(&mut out, x0, x0 + w, y, palette.axis);
            }
            let weight = g.line_weight.unwrap_or(DEFAULT_WEIGHT);
            let mut segs = Vec::new();
            let mut pen = false;
            let mut count = 0;
            for p in &pts {
                match p {
                    Some((x, y)) => {
                        let (x, y) = (*x as f32, *y as f32);
                        segs.push(if pen {
                            PathSeg::L(x, y)
                        } else {
                            PathSeg::M(x, y)
                        });
                        pen = true;
                        count += 1;
                    }
                    // `gap` breaks the line; `span` connects over the empty cell.
                    None => pen = pen && g.empty_as == SparklineEmpty::Span,
                }
            }
            if count > 1 {
                out.push(Command::Path {
                    segs,
                    fill: None,
                    stroke: Some(stroke(palette.series, weight)),
                });
            }
            let r = marker_radius(weight);
            for (i, (p, v)) in pts.iter().zip(&vals).enumerate() {
                let (Some((x, y)), Some(v)) = (p, v) else {
                    continue;
                };
                let color = highlight(g, palette, i, *v, (first, last), data)
                    .or(g.markers.then_some(palette.markers))
                    .or((count == 1).then_some(palette.series));
                if let Some(c) = color {
                    marker(&mut out, *x, *y, r, c);
                }
            }
        }
        SparklineType::Column | SparklineType::Stacked => {
            let slot = w / n as f64;
            let bw = slot * BAR;
            let mid = y0 + h / 2.0;
            let base = if g.kind == SparklineType::Column {
                y_of(0.0_f64.clamp(lo.min(hi), hi.max(lo)))
            } else {
                mid
            };
            if g.display_x_axis
                && (g.kind == SparklineType::Stacked || (lo <= 0.0 && hi >= 0.0 && hi > lo))
            {
                axis_line(&mut out, x0, x0 + w, base, palette.axis);
            }
            for (i, v) in vals.iter().enumerate() {
                let Some(v) = *v else { continue };
                let left = x0 + i as f64 * slot + (slot - bw) / 2.0;
                let left = if g.right_to_left {
                    mirror(left + bw)
                } else {
                    left
                };
                let (top, bottom) = if g.kind == SparklineType::Column {
                    if hi <= lo {
                        continue;
                    }
                    let y = y_of(v);
                    (y.min(base), y.max(base))
                } else if v > 0.0 {
                    (y0, mid)
                } else if v < 0.0 {
                    (mid, y0 + h)
                } else {
                    continue;
                };
                let color =
                    highlight(g, palette, i, v, (first, last), data).unwrap_or(palette.series);
                fill_rect(&mut out, left, top, bw, bottom - top, color);
            }
        }
    }
    out
}

fn axis_line(out: &mut Vec<Command>, x1: f64, x2: f64, y: f64, color: Rgba) {
    out.push(Command::Line {
        x1: x1 as f32,
        y1: y as f32,
        x2: x2 as f32,
        y2: y as f32,
        width: AXIS_WEIGHT as f32,
        color,
        dash: None,
    });
}

/// Sparkline located at `at` on `ws`: (group, index in the group).
pub fn at(ws: &Worksheet, at: CellRef) -> Option<(usize, usize)> {
    ws.sparkline_groups.iter().enumerate().find_map(|(gi, g)| {
        g.sparklines
            .iter()
            .position(|s| s.location == at)
            .map(|si| (gi, si))
    })
}

/// Drawing commands of sparkline `index` of group `group` on `sheet`, in `cell` (points).
pub fn render(wb: &Workbook, sheet: usize, group: usize, index: usize, cell: Rect) -> Vec<Command> {
    let Some(g) = wb
        .sheets
        .get(sheet)
        .and_then(|ws| ws.sparkline_groups.get(group))
    else {
        return Vec::new();
    };
    let Some(s) = g.sparklines.get(index) else {
        return Vec::new();
    };
    let v = values(wb, sheet, g, s);
    let d = dates(wb, sheet, g);
    let axis = axis_bounds(wb, sheet, g, index);
    draw(
        cell,
        g,
        &v,
        d.as_deref(),
        axis,
        &Palette::of(&g.colors, wb.theme.as_ref()),
    )
}

// ---------------------------------------------------------------------------
// Editing
// ---------------------------------------------------------------------------

/// Changes to a group's settings; absent fields stay as they are. Colours are `RRGGBB`.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SparklinePatch {
    /// Sparkline type.
    #[serde(rename = "type")]
    pub kind: Option<SparklineType>,
    /// Markers on every point.
    pub markers: Option<bool>,
    /// Highlight the highest point.
    pub high: Option<bool>,
    /// Highlight the lowest point.
    pub low: Option<bool>,
    /// Highlight the first point.
    pub first: Option<bool>,
    /// Highlight the last point.
    pub last: Option<bool>,
    /// Highlight negative points.
    pub negative: Option<bool>,
    /// Draw the horizontal axis.
    pub display_x_axis: Option<bool>,
    /// Include hidden rows and columns.
    pub display_hidden: Option<bool>,
    /// Right-to-left plotting.
    pub right_to_left: Option<bool>,
    /// Empty cells.
    pub empty_as: Option<SparklineEmpty>,
    /// Minimum of the vertical axis.
    pub min_axis: Option<SparklineAxis>,
    /// Maximum of the vertical axis.
    pub max_axis: Option<SparklineAxis>,
    /// Custom minimum (used with a `custom` minimum axis).
    pub manual_min: Option<f64>,
    /// Custom maximum (used with a `custom` maximum axis).
    pub manual_max: Option<f64>,
    /// Line weight in points.
    pub line_weight: Option<f64>,
    /// Series colour.
    pub color_series: Option<String>,
    /// Negative-point colour.
    pub color_negative: Option<String>,
    /// Axis colour.
    pub color_axis: Option<String>,
    /// Marker colour.
    pub color_markers: Option<String>,
    /// First-point colour.
    pub color_first: Option<String>,
    /// Last-point colour.
    pub color_last: Option<String>,
    /// Highest-point colour.
    pub color_high: Option<String>,
    /// Lowest-point colour.
    pub color_low: Option<String>,
}

fn color_of(h: &str) -> Result<SheetColor, String> {
    let h = h.trim().trim_start_matches('#');
    if h.len() == 6 && h.chars().all(|c| c.is_ascii_hexdigit()) {
        Ok(SheetColor::rgb(&h.to_ascii_uppercase()))
    } else {
        Err(format!("invalid colour: {h}"))
    }
}

/// Applies `p` to group `g`.
pub fn apply(g: &mut SparklineGroup, p: &SparklinePatch) -> Result<(), String> {
    let set = |dst: &mut bool, v: Option<bool>| {
        if let Some(v) = v {
            *dst = v;
        }
    };
    if let Some(k) = p.kind {
        g.kind = k;
    }
    set(&mut g.markers, p.markers);
    set(&mut g.high, p.high);
    set(&mut g.low, p.low);
    set(&mut g.first, p.first);
    set(&mut g.last, p.last);
    set(&mut g.negative, p.negative);
    set(&mut g.display_x_axis, p.display_x_axis);
    set(&mut g.display_hidden, p.display_hidden);
    set(&mut g.right_to_left, p.right_to_left);
    if let Some(e) = p.empty_as {
        g.empty_as = e;
    }
    if let Some(a) = p.min_axis {
        g.min_axis = a;
    }
    if let Some(a) = p.max_axis {
        g.max_axis = a;
    }
    if let Some(v) = p.manual_min.filter(|v| v.is_finite()) {
        g.manual_min = Some(v);
    }
    if let Some(v) = p.manual_max.filter(|v| v.is_finite()) {
        g.manual_max = Some(v);
    }
    if let (SparklineAxis::Custom, SparklineAxis::Custom, Some(lo), Some(hi)) =
        (g.min_axis, g.max_axis, g.manual_min, g.manual_max)
    {
        if lo >= hi {
            return Err("the axis minimum must be less than the maximum".into());
        }
    }
    if let Some(w) = p.line_weight {
        if !(w.is_finite() && (0.0..=1584.0).contains(&w)) {
            return Err("invalid line weight".into());
        }
        g.line_weight = Some(w);
    }
    let c = &mut g.colors;
    for (dst, src) in [
        (&mut c.series, &p.color_series),
        (&mut c.negative, &p.color_negative),
        (&mut c.axis, &p.color_axis),
        (&mut c.markers, &p.color_markers),
        (&mut c.first, &p.color_first),
        (&mut c.last, &p.color_last),
        (&mut c.high, &p.color_high),
        (&mut c.low, &p.color_low),
    ] {
        if let Some(h) = src {
            *dst = Some(color_of(h)?);
        }
    }
    Ok(())
}

/// Removes the sparklines located in `cells` and drops groups left empty.
fn remove_at(ws: &mut Worksheet, cells: impl Fn(CellRef) -> bool) {
    for g in &mut ws.sparkline_groups {
        g.sparklines.retain(|s| !cells(s.location));
    }
    ws.sparkline_groups.retain(|g| !g.sparklines.is_empty());
}

/// Inserts a group of `kind` sparklines over `data` (a range, optionally sheet-qualified)
/// into the cells of `location` (one row or one column). With several locations the data is
/// split by rows when its row count matches the number of locations (columns for a single-row
/// location are tried first), else by columns. Existing sparklines at those cells are
/// replaced. Returns the new group's index.
pub fn insert(
    wb: &mut Workbook,
    sheet: usize,
    kind: SparklineType,
    data: &str,
    location: CellRange,
) -> Result<usize, String> {
    let sparklines = layout(wb, sheet, data, location, None)?;
    let ws = &mut wb.sheets[sheet];
    remove_at(ws, |c| location.contains(c));
    ws.sparkline_groups.push(SparklineGroup {
        kind,
        sparklines,
        empty_as: SparklineEmpty::Gap,
        line_weight: Some(DEFAULT_WEIGHT),
        colors: default_colors(),
        ..SparklineGroup::default()
    });
    Ok(ws.sparkline_groups.len() - 1)
}

/// The sparklines of `data` shown in `location` (see [`insert`]); `by_rows` forces how a
/// data range that fits both ways is split ("switch row / column"): `prefer_rows`.
fn layout(
    wb: &Workbook,
    sheet: usize,
    data: &str,
    location: CellRange,
    prefer_rows: Option<bool>,
) -> Result<Vec<Sparkline>, String> {
    let (dsheet, dr) =
        resolve_ref(wb, sheet, data).ok_or_else(|| "invalid data range".to_owned())?;
    let dname = quote_sheet(&wb.sheets.get(dsheet).ok_or("no such sheet")?.name);
    if sheet >= wb.sheets.len() {
        return Err("no such sheet".into());
    }
    let (lrows, lcols) = (
        location.last.row - location.first.row + 1,
        location.last.col - location.first.col + 1,
    );
    if lrows > 1 && lcols > 1 {
        return Err("the location must be one row or one column".into());
    }
    let n = lrows.max(lcols);
    let (drows, dcols) = (
        dr.last.row - dr.first.row + 1,
        dr.last.col - dr.first.col + 1,
    );
    let range_text = |r: CellRange| {
        if r.first == r.last {
            format!("{dname}!{}", r.first)
        } else {
            format!("{dname}!{}:{}", r.first, r.last)
        }
    };
    let locations: Vec<CellRef> = (0..n)
        .map(|i| {
            if lrows > 1 {
                CellRef::new(location.first.row + i, location.first.col)
            } else {
                CellRef::new(location.first.row, location.first.col + i)
            }
        })
        .collect();
    let by_rows = |i: u32| {
        CellRange::new(
            CellRef::new(dr.first.row + i, dr.first.col),
            CellRef::new(dr.first.row + i, dr.last.col),
        )
    };
    let by_cols = |i: u32| {
        CellRange::new(
            CellRef::new(dr.first.row, dr.first.col + i),
            CellRef::new(dr.last.row, dr.first.col + i),
        )
    };
    let ranges: Vec<CellRange> = if n == 1 {
        if drows > 1 && dcols > 1 {
            return Err("the data must be one row or one column for a single sparkline".into());
        }
        vec![dr]
    } else {
        let rows_fit = drows == n;
        let cols_fit = dcols == n;
        // A location row pairs with data columns first, a location column with data rows.
        let use_rows = match prefer_rows {
            Some(r) if (r && rows_fit) || (!r && cols_fit) => r,
            _ if lrows > 1 => rows_fit,
            _ => !cols_fit && rows_fit,
        };
        if use_rows {
            (0..n).map(by_rows).collect()
        } else if cols_fit {
            (0..n).map(by_cols).collect()
        } else {
            return Err("the data does not match the location range".into());
        }
    };
    Ok(locations
        .iter()
        .zip(&ranges)
        .map(|(loc, r)| Sparkline {
            formula: Some(range_text(*r)),
            location: *loc,
        })
        .collect())
}

/// "Edit group location and data": group `group` shows `data` in `location` instead, with
/// its settings kept; sparklines of other groups in `location` are removed.
pub fn set_data(
    wb: &mut Workbook,
    sheet: usize,
    group: usize,
    data: &str,
    location: CellRange,
) -> Result<(), String> {
    let sparklines = layout(wb, sheet, data, location, None)?;
    let ws = wb.sheets.get_mut(sheet).ok_or("no such sheet")?;
    if group >= ws.sparkline_groups.len() {
        return Err("no such sparkline group".into());
    }
    for (i, g) in ws.sparkline_groups.iter_mut().enumerate() {
        if i == group {
            g.sparklines = sparklines.clone();
        } else {
            g.sparklines.retain(|s| !location.contains(s.location));
        }
    }
    ws.sparkline_groups.retain(|g| !g.sparklines.is_empty());
    Ok(())
}

/// "Edit single sparkline's data": the sparkline in cell `at` reads `data` (one row or
/// one column, optionally sheet-qualified).
pub fn set_single_data(
    wb: &mut Workbook,
    sheet: usize,
    at: CellRef,
    data: &str,
) -> Result<(), String> {
    let one = layout(wb, sheet, data, CellRange::single(at), None)?;
    let ws = wb.sheets.get_mut(sheet).ok_or("no such sheet")?;
    let (g, i) = self::at(ws, at).ok_or("no sparkline in the cell")?;
    ws.sparkline_groups[g].sparklines[i].formula = one.into_iter().next().and_then(|s| s.formula);
    Ok(())
}

/// "Switch row / column": the data of group `group` (the block its ranges cover) is split
/// the other way — by columns instead of rows, or the reverse — when it fits both ways.
pub fn switch_row_column(wb: &mut Workbook, sheet: usize, group: usize) -> Result<(), String> {
    let g = wb
        .sheets
        .get(sheet)
        .and_then(|ws| ws.sparkline_groups.get(group))
        .ok_or("no such sparkline group")?;
    let mut block: Option<(usize, CellRange)> = None;
    let mut rows_split = true;
    for s in &g.sparklines {
        let f = s.formula.as_deref().ok_or("a sparkline has no data")?;
        let (ds, r) = resolve_ref(wb, sheet, f).ok_or("invalid data range")?;
        if r.first.row != r.last.row {
            rows_split = false;
        }
        block = Some(match block {
            None => (ds, r),
            Some((bs, b)) if bs == ds => (
                bs,
                CellRange::new(
                    CellRef::new(b.first.row.min(r.first.row), b.first.col.min(r.first.col)),
                    CellRef::new(b.last.row.max(r.last.row), b.last.col.max(r.last.col)),
                ),
            ),
            Some(_) => return Err("the data is on several sheets".into()),
        });
    }
    let (ds, b) = block.ok_or("the group is empty")?;
    let locs: Vec<CellRef> = g.sparklines.iter().map(|s| s.location).collect();
    let loc = CellRange::new(
        CellRef::new(
            locs.iter().map(|c| c.row).min().unwrap_or(0),
            locs.iter().map(|c| c.col).min().unwrap_or(0),
        ),
        CellRef::new(
            locs.iter().map(|c| c.row).max().unwrap_or(0),
            locs.iter().map(|c| c.col).max().unwrap_or(0),
        ),
    );
    let n = u32::try_from(locs.len()).unwrap_or(u32::MAX);
    if b.rows() != n || b.cols() != n {
        return Err("the data does not fit the other way".into());
    }
    let name = quote_sheet(&wb.sheets.get(ds).ok_or("no such sheet")?.name);
    let data = format!("{name}!{}:{}", b.first, b.last);
    let sparklines = layout(wb, sheet, &data, loc, Some(!rows_split))?;
    wb.sheets[sheet].sparkline_groups[group].sparklines = sparklines;
    Ok(())
}

/// Changes the settings of group `group`.
pub fn edit(
    wb: &mut Workbook,
    sheet: usize,
    group: usize,
    patch: &SparklinePatch,
) -> Result<(), String> {
    let g = wb
        .sheets
        .get_mut(sheet)
        .and_then(|ws| ws.sparkline_groups.get_mut(group))
        .ok_or("no such sparkline group")?;
    let mut next = g.clone();
    apply(&mut next, patch)?;
    *g = next;
    Ok(())
}

/// Puts every sparkline located in `range` into one group, with the settings of the first
/// group involved. Returns the group's index.
pub fn group(wb: &mut Workbook, sheet: usize, range: CellRange) -> Result<usize, String> {
    let ws = wb.sheets.get_mut(sheet).ok_or("no such sheet")?;
    let first = ws
        .sparkline_groups
        .iter()
        .position(|g| g.sparklines.iter().any(|s| range.contains(s.location)))
        .ok_or("no sparklines in the selection")?;
    let mut merged = ws.sparkline_groups[first].clone();
    merged.sparklines = ws
        .sparkline_groups
        .iter()
        .flat_map(|g| g.sparklines.iter())
        .filter(|s| range.contains(s.location))
        .cloned()
        .collect();
    for (i, g) in ws.sparkline_groups.iter_mut().enumerate() {
        if i == first {
            continue;
        }
        g.sparklines.retain(|s| !range.contains(s.location));
    }
    ws.sparkline_groups[first] = merged;
    let mut index = first;
    let mut k = 0;
    ws.sparkline_groups.retain(|g| {
        let keep = !g.sparklines.is_empty();
        if !keep && k < first {
            index -= 1;
        }
        k += 1;
        keep
    });
    Ok(index)
}

/// Splits every group with a sparkline in `range` into one group per sparkline (same
/// settings). Returns the number of groups created.
pub fn ungroup(wb: &mut Workbook, sheet: usize, range: CellRange) -> Result<usize, String> {
    let ws = wb.sheets.get_mut(sheet).ok_or("no such sheet")?;
    let mut out = Vec::new();
    let mut made = 0;
    for g in std::mem::take(&mut ws.sparkline_groups) {
        if g.sparklines.len() > 1 && g.sparklines.iter().any(|s| range.contains(s.location)) {
            for s in &g.sparklines {
                let mut one = g.clone();
                one.sparklines = vec![s.clone()];
                out.push(one);
                made += 1;
            }
        } else {
            out.push(g);
        }
    }
    ws.sparkline_groups = out;
    if made == 0 {
        return Err("no sparkline group in the selection".into());
    }
    Ok(made)
}

/// Removes the sparklines in `range`, or (with `whole_groups`) every group that has one
/// there. Returns the number of sparklines removed.
pub fn clear(
    wb: &mut Workbook,
    sheet: usize,
    range: CellRange,
    whole_groups: bool,
) -> Result<usize, String> {
    let ws = wb.sheets.get_mut(sheet).ok_or("no such sheet")?;
    let before: usize = ws.sparkline_groups.iter().map(|g| g.sparklines.len()).sum();
    if whole_groups {
        ws.sparkline_groups
            .retain(|g| !g.sparklines.iter().any(|s| range.contains(s.location)));
    } else {
        remove_at(ws, |c| range.contains(c));
    }
    let after: usize = ws.sparkline_groups.iter().map(|g| g.sparklines.len()).sum();
    Ok(before - after)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn group_of(kind: SparklineType) -> SparklineGroup {
        SparklineGroup {
            kind,
            ..SparklineGroup::default()
        }
    }

    fn pal() -> Palette {
        Palette::of(&SparklineColors::default(), None)
    }

    /// A 64 × 20 pt cell at the origin: plot box x 2..62 (w 60), y 2..18 (h 16).
    const CELL: Rect = Rect {
        x: 0.0,
        y: 0.0,
        w: 64.0,
        h: 20.0,
    };

    fn path_points(c: &Command) -> Vec<(f32, f32)> {
        match c {
            Command::Path { segs, .. } => segs
                .iter()
                .filter_map(|s| match s {
                    PathSeg::M(x, y) | PathSeg::L(x, y) => Some((*x, *y)),
                    _ => None,
                })
                .collect(),
            _ => Vec::new(),
        }
    }

    fn rects(cmds: &[Command]) -> Vec<(f32, f32, f32, f32, Rgba)> {
        cmds.iter()
            .filter_map(|c| match c {
                Command::FillRect { x, y, w, h, color } => Some((*x, *y, *w, *h, *color)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn line_points() {
        // [1, 3, 2], axis 1..3: x = 2, 32, 62; y = 18 (min), 2 (max), 10 (middle).
        let g = group_of(SparklineType::Line);
        let v = [Some(1.0), Some(3.0), Some(2.0)];
        let cmds = draw(CELL, &g, &v, None, bounds(&g, &v, &[]), &pal());
        assert_eq!(cmds.len(), 1, "a line, no markers");
        assert_eq!(
            path_points(&cmds[0]),
            vec![(2.0, 18.0), (32.0, 2.0), (62.0, 10.0)]
        );
        // Right to left mirrors x.
        let rtl = SparklineGroup {
            right_to_left: true,
            ..g.clone()
        };
        let cmds = draw(CELL, &rtl, &v, None, (1.0, 3.0), &pal());
        assert_eq!(
            path_points(&cmds[0]),
            vec![(62.0, 18.0), (32.0, 2.0), (2.0, 10.0)]
        );
    }

    #[test]
    fn line_empty_cells() {
        let v = [Some(1.0), None, Some(3.0)];
        // gap: two lone points → no line segment of 2+ points … only the first M and a second M.
        let gap = SparklineGroup {
            empty_as: SparklineEmpty::Gap,
            ..group_of(SparklineType::Line)
        };
        let cmds = draw(CELL, &gap, &v, None, (1.0, 3.0), &pal());
        match &cmds[0] {
            Command::Path { segs, .. } => {
                assert_eq!(segs, &vec![PathSeg::M(2.0, 18.0), PathSeg::M(62.0, 2.0)]);
            }
            other => panic!("{other:?}"),
        }
        // span: connected over the empty cell.
        let span = SparklineGroup {
            empty_as: SparklineEmpty::Span,
            ..gap.clone()
        };
        let cmds = draw(CELL, &span, &v, None, (1.0, 3.0), &pal());
        match &cmds[0] {
            Command::Path { segs, .. } => {
                assert_eq!(segs, &vec![PathSeg::M(2.0, 18.0), PathSeg::L(62.0, 2.0)]);
            }
            other => panic!("{other:?}"),
        }
        // zero: the empty cell is 0; axis 0..3 → y(0) = 18, y(1) = 18 − 16/3, y(3) = 2.
        let zero = group_of(SparklineType::Line);
        let b = bounds(&zero, &effective(&zero, &v), &[]);
        assert_eq!(b, (0.0, 3.0));
        let cmds = draw(CELL, &zero, &v, None, b, &pal());
        let p = path_points(&cmds[0]);
        assert_eq!(p[1], (32.0, 18.0));
        assert!((p[0].1 - (18.0 - 16.0 / 3.0) as f32).abs() < 1e-4);
        assert_eq!(p[2], (62.0, 2.0));
    }

    #[test]
    fn line_markers_and_highlights() {
        let g = SparklineGroup {
            high: true,
            low: true,
            ..group_of(SparklineType::Line)
        };
        let v = [Some(1.0), Some(3.0), Some(2.0)];
        let p = pal();
        let cmds = draw(CELL, &g, &v, None, (1.0, 3.0), &p);
        // Line + high marker at (32, 2) + low marker at (2, 18).
        assert_eq!(cmds.len(), 3);
        let fills: Vec<Rgba> = cmds[1..]
            .iter()
            .map(|c| match c {
                Command::Path {
                    fill: Some(Paint::Solid { color }),
                    segs,
                    ..
                } => {
                    // Circle starts at (cx + r, cy); r = 1.5 + 0.75 / 2 = 1.875.
                    if let PathSeg::M(x, y) = segs[0] {
                        assert!((x == 2.0 + 1.875 && y == 18.0) || (x == 32.0 + 1.875 && y == 2.0));
                    }
                    *color
                }
                _ => panic!(),
            })
            .collect();
        assert_eq!(fills, vec![p.low, p.high]);
        // markers: every point gets one.
        let all = SparklineGroup {
            markers: true,
            ..group_of(SparklineType::Line)
        };
        assert_eq!(draw(CELL, &all, &v, None, (1.0, 3.0), &p).len(), 4);
    }

    #[test]
    fn column_bars() {
        // [1, 3, 2]: slots of 20, bars 16 wide at x 4, 24, 44; axis 0..3 (zero included):
        // heights 16/3, 16, 32/3 from the bottom (y 18).
        let g = group_of(SparklineType::Column);
        let v = [Some(1.0), Some(3.0), Some(2.0)];
        let b = bounds(&g, &v, &[]);
        assert_eq!(b, (0.0, 3.0));
        let r = rects(&draw(CELL, &g, &v, None, b, &pal()));
        assert_eq!(r.len(), 3);
        let expect = [(4.0, 16.0 / 3.0), (24.0, 16.0), (44.0, 32.0 / 3.0)];
        for (got, (x, h)) in r.iter().zip(expect) {
            assert!((got.0 - x as f32).abs() < 1e-4);
            assert!((got.2 - 16.0).abs() < 1e-4);
            assert!((got.3 - h as f32).abs() < 1e-4);
            assert!((got.1 + got.3 - 18.0).abs() < 1e-4, "bars stand on y = 18");
        }
        // Negative values hang below zero: [2, −2] → axis −2..2, baseline y = 10.
        let neg = SparklineGroup {
            negative: true,
            ..g.clone()
        };
        let v = [Some(2.0), Some(-2.0)];
        let p = pal();
        let r = rects(&draw(CELL, &neg, &v, None, bounds(&neg, &v, &[]), &p));
        assert_eq!(r[0], (5.0, 2.0, 24.0, 8.0, p.series));
        assert_eq!(r[1], (35.0, 10.0, 24.0, 8.0, p.negative));
    }

    #[test]
    fn win_loss_bars() {
        // [1, −5, 0, 2]: slots of 15, bars 12 wide; positive → y 2..10, negative → 10..18.
        let g = group_of(SparklineType::Stacked);
        let v = [Some(1.0), Some(-5.0), Some(0.0), Some(2.0)];
        let p = pal();
        let r = rects(&draw(CELL, &g, &v, None, (-5.0, 2.0), &p));
        assert_eq!(
            r,
            vec![
                (3.5, 2.0, 12.0, 8.0, p.series),
                (18.5, 10.0, 12.0, 8.0, p.series),
                (48.5, 2.0, 12.0, 8.0, p.series),
            ]
        );
        // The axis runs through the middle.
        let ax = SparklineGroup {
            display_x_axis: true,
            ..g
        };
        let cmds = draw(CELL, &ax, &v, None, (-5.0, 2.0), &p);
        assert!(
            matches!(cmds[0], Command::Line { y1, x1, x2, .. } if y1 == 10.0 && x1 == 2.0 && x2 == 62.0)
        );
    }

    #[test]
    fn axis_bounds_individual_group_custom() {
        let mut wb = Workbook::new();
        let ws = &mut wb.sheets[0];
        for (c, v) in [1.0, 3.0, 2.0].iter().enumerate() {
            ws.set_cell(
                0,
                c as u32,
                bo_model::sheet::Cell::new(CellValue::Number(*v)),
            );
        }
        for (c, v) in [10.0, -4.0, 6.0].iter().enumerate() {
            ws.set_cell(
                1,
                c as u32,
                bo_model::sheet::Cell::new(CellValue::Number(*v)),
            );
        }
        let gi = insert(
            &mut wb,
            0,
            SparklineType::Line,
            "A1:C2",
            CellRange::parse("D1:D2").unwrap(),
        )
        .unwrap();
        let g = wb.sheets[0].sparkline_groups[gi].clone();
        assert_eq!(g.sparklines[0].formula.as_deref(), Some("Sheet1!A1:C1"));
        assert_eq!(g.sparklines[1].formula.as_deref(), Some("Sheet1!A2:C2"));
        assert_eq!(axis_bounds(&wb, 0, &g, 0), (1.0, 3.0));
        assert_eq!(axis_bounds(&wb, 0, &g, 1), (-4.0, 10.0));
        let grp = SparklineGroup {
            min_axis: SparklineAxis::Group,
            max_axis: SparklineAxis::Group,
            ..g.clone()
        };
        assert_eq!(axis_bounds(&wb, 0, &grp, 0), (-4.0, 10.0));
        let custom = SparklineGroup {
            min_axis: SparklineAxis::Custom,
            max_axis: SparklineAxis::Group,
            manual_min: Some(0.0),
            ..g.clone()
        };
        assert_eq!(axis_bounds(&wb, 0, &custom, 0), (0.0, 10.0));
        // Hidden column B is skipped unless displayHidden.
        wb.sheets[0].cols.push(bo_model::sheet::ColInfo {
            min: 1,
            max: 1,
            hidden: true,
            ..bo_model::sheet::ColInfo::default()
        });
        assert_eq!(
            values(&wb, 0, &g, &g.sparklines[0]),
            vec![Some(1.0), Some(2.0)]
        );
        let shown = SparklineGroup {
            display_hidden: true,
            ..g
        };
        assert_eq!(values(&wb, 0, &shown, &shown.sparklines[0]).len(), 3);
    }

    #[test]
    fn insert_group_ungroup_clear() {
        let mut wb = Workbook::new();
        let r = |s: &str| CellRange::parse(s).unwrap();
        // A row of locations pairs with data columns.
        insert(&mut wb, 0, SparklineType::Column, "A1:C4", r("A6:C6")).unwrap();
        let g = &wb.sheets[0].sparkline_groups[0];
        assert_eq!(g.sparklines.len(), 3);
        assert_eq!(g.sparklines[2].formula.as_deref(), Some("Sheet1!C1:C4"));
        assert_eq!(g.sparklines[2].location, CellRef::new(5, 2));
        assert!(insert(&mut wb, 0, SparklineType::Line, "A1:C4", r("A7:B7")).is_err());
        assert!(insert(&mut wb, 0, SparklineType::Line, "A1:C4", r("E1:F2")).is_err());
        // Replacing B6 with a new single sparkline takes it out of the first group.
        insert(&mut wb, 0, SparklineType::Line, "A1:A4", r("B6")).unwrap();
        assert_eq!(wb.sheets[0].sparkline_groups[0].sparklines.len(), 2);
        assert_eq!(at(&wb.sheets[0], CellRef::new(5, 1)), Some((1, 0)));
        // Group A6:C6 → one group with the first group's settings.
        let gi = group(&mut wb, 0, r("A6:C6")).unwrap();
        assert_eq!(wb.sheets[0].sparkline_groups.len(), 1);
        assert_eq!(gi, 0);
        assert_eq!(wb.sheets[0].sparkline_groups[0].kind, SparklineType::Column);
        assert_eq!(wb.sheets[0].sparkline_groups[0].sparklines.len(), 3);
        // Ungroup → three groups.
        assert_eq!(ungroup(&mut wb, 0, r("B6")).unwrap(), 3);
        assert_eq!(wb.sheets[0].sparkline_groups.len(), 3);
        edit(
            &mut wb,
            0,
            1,
            &SparklinePatch {
                kind: Some(SparklineType::Stacked),
                color_series: Some("#00ff00".into()),
                ..SparklinePatch::default()
            },
        )
        .unwrap();
        let g = &wb.sheets[0].sparkline_groups[1];
        assert_eq!(g.kind, SparklineType::Stacked);
        assert_eq!(
            g.colors.series.as_ref().unwrap().rgb.as_deref(),
            Some("FF00FF00")
        );
        assert!(edit(
            &mut wb,
            0,
            1,
            &SparklinePatch {
                color_low: Some("nope".into()),
                ..SparklinePatch::default()
            }
        )
        .is_err());
        // Clear one cell, then the rest.
        assert_eq!(clear(&mut wb, 0, r("A6"), false).unwrap(), 1);
        assert_eq!(clear(&mut wb, 0, r("A1:Z9"), false).unwrap(), 2);
        assert_eq!(wb.sheets[0].sparkline_groups, Vec::<SparklineGroup>::new());
    }
}
