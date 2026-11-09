//! Conditional formatting evaluation (ECMA-376 Part 1, §18.3.1.18
//! `conditionalFormatting`, §18.3.1.10 `cfRule`) and rule creation.
//!
//! Rules are applied in `priority` order (lower first, §18.3.1.10 `priority`);
//! a property set by an earlier rule is not overridden by a later one and
//! `stopIfTrue` ends evaluation for that cell. Differential formats come from
//! `dxfs` (§18.8.15); colour scales (§18.3.1.16), data bars (§18.3.1.28) and icon
//! sets (§18.3.1.49) are computed from the numeric values of the rule's range
//! with `cfvo` thresholds (§18.3.1.11).

use crate::cells::{to_formula_value, ModelCells};
use crate::color::resolve_color;
use bo_formula::value::compare_values;
use bo_formula::{parse, Value};
use bo_model::sheet::{
    CellRange, CellRef, CfRule, Cfvo, ColorScale, ConditionalFormat, DataBar, DataBarX14, Dxf,
    Fill, Font, IconSet, PatternFill, SheetColor, Workbook, Worksheet,
};
use bo_render::Rgba;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::ops::Range;

/// A data bar to draw in a cell (§18.3.1.28 `dataBar`, [MS-XLSX] x14 `CT_DataBar`).
/// Positions are fractions of the cell's inner width, measured from its left edge.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BarDraw {
    /// Left end of the bar (0.0–1.0).
    pub start: f64,
    /// Right end of the bar (0.0–1.0, ≥ `start`).
    pub end: f64,
    /// Bar fill colour (negative colour for negative values).
    pub color: Rgba,
    /// Bar border colour (`border="1"`).
    pub border: Option<Rgba>,
    /// Fill fades from `color` at the bar's origin towards its far end.
    pub gradient: bool,
    /// The bar grows leftwards from its origin (negative value or right-to-left).
    pub leftward: bool,
    /// Axis position (0.0–1.0) drawn for ranges with negative and positive values.
    pub axis: Option<f64>,
    /// Axis colour.
    pub axis_color: Rgba,
}

/// Default data bar colour (BlueSheet's own choice).
pub const BAR_BLUE: Rgba = Rgba(0x4A, 0x7B, 0xB7, 255);
/// Default negative bar colour (BlueSheet's own choice).
pub const BAR_RED: Rgba = Rgba(0xD6, 0x45, 0x45, 255);

/// Geometry of one data bar for value `x` with thresholds `lo`..`hi`.
///
/// `axis` (`axisPosition`): `none` scales every value linearly from `lo` (left edge) to
/// `hi`; `automatic` puts the axis where zero falls between `lo` and `hi` (left / right
/// edge when every threshold has the same sign); `middle` puts it at the centre and
/// scales both sides by the larger magnitude. On each side of the axis a non-zero value
/// takes `min_len + (max_len − min_len) · |x| / |extreme|` of that side's width; a zero
/// value draws no bar. Returns `(start, end, axis, leftward)`.
#[must_use]
pub fn bar_geometry(
    x: f64,
    lo: f64,
    hi: f64,
    min_len: f64,
    max_len: f64,
    axis: &str,
) -> (f64, f64, Option<f64>, bool) {
    let (lo, hi) = if hi < lo { (hi, lo) } else { (lo, hi) };
    let len = |t: f64| min_len + (max_len - min_len) * t.clamp(0.0, 1.0);
    let linear = || {
        let t = if hi > lo { (x - lo) / (hi - lo) } else { 1.0 };
        (0.0, len(t), None, false)
    };
    let sided = |a: f64, pos_extreme: f64, neg_extreme: f64| {
        if x > 0.0 && pos_extreme > 0.0 {
            let w = (1.0 - a) * len(x / pos_extreme);
            (a, a + w, Some(a), false)
        } else if x < 0.0 && neg_extreme < 0.0 {
            let w = a * len(x / neg_extreme);
            (a - w, a, Some(a), true)
        } else {
            (a, a, Some(a), false)
        }
    };
    match axis {
        "none" => linear(),
        "middle" => {
            let m = lo.abs().max(hi.abs());
            sided(0.5, m, -m)
        }
        _ => {
            if lo >= 0.0 {
                linear()
            } else if hi <= 0.0 {
                // Every threshold negative: bars grow leftwards from the right edge.
                let t = if hi > lo { (hi - x) / (hi - lo) } else { 1.0 };
                (1.0 - len(t), 1.0, None, true)
            } else {
                sided(-lo / (hi - lo), hi, lo)
            }
        }
    }
}

/// An icon of an icon set: family, position (0 = lowest) and set size.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IconDraw {
    /// Icon set name (§18.18.42 `ST_IconSetType`), e.g. `3TrafficLights1`.
    pub set: String,
    /// Zero-based position of the icon in the set (0 = lowest band).
    pub index: usize,
    /// Number of icons in the set (3, 4 or 5).
    pub count: usize,
}

/// Formatting that conditional formats add to a cell.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CellFormat {
    /// Background fill from the matching rule's `dxf` (§18.8.17).
    pub fill: Option<Rgba>,
    /// Font colour from the matching rule's `dxf`.
    pub font_color: Option<Rgba>,
    /// Bold override (`b`), `None` = unchanged.
    pub bold: Option<bool>,
    /// Italic override (`i`), `None` = unchanged.
    pub italic: Option<bool>,
    /// Strikethrough override (`strike`), `None` = unchanged.
    pub strike: Option<bool>,
    /// Underline override (`u`), `None` = unchanged.
    pub underline: Option<bool>,
    /// Data bar to draw behind the value (§18.3.1.28).
    pub bar: Option<BarDraw>,
    /// Icon to draw before the value (§18.3.1.49 `iconSet`).
    pub icon: Option<IconDraw>,
    /// `showValue="0"` of a data bar / icon set.
    pub hide_value: bool,
}

impl CellFormat {
    fn merge_from(&mut self, o: &CellFormat) {
        self.fill = self.fill.or(o.fill);
        self.font_color = self.font_color.or(o.font_color);
        self.bold = self.bold.or(o.bold);
        self.italic = self.italic.or(o.italic);
        self.strike = self.strike.or(o.strike);
        self.underline = self.underline.or(o.underline);
        if self.bar.is_none() {
            self.bar = o.bar;
        }
        if self.icon.is_none() {
            self.icon.clone_from(&o.icon);
        }
        self.hide_value |= o.hide_value;
    }
}

fn dxf_format(dxf: &Dxf, wb: &Workbook) -> CellFormat {
    let theme = wb.theme.as_ref();
    let mut out = CellFormat::default();
    if let Some(f) = &dxf.font {
        out.font_color = f.color.as_ref().and_then(|c| resolve_color(c, theme));
        out.bold = f.bold.then_some(true);
        out.italic = f.italic.then_some(true);
        out.strike = f.strike.then_some(true);
        out.underline = f.underline.as_ref().map(|u| u != "none");
    }
    // §18.8.14: a dxf solid fill keeps its colour in bgColor (fgColor is a fallback).
    out.fill = match &dxf.fill {
        Some(Fill::Pattern(p)) if p.pattern.as_deref() != Some("none") => {
            p.bg.as_ref()
                .or(p.fg.as_ref())
                .and_then(|c| resolve_color(c, theme))
        }
        Some(Fill::Gradient(g)) => g.stops.first().and_then(|(_, c)| resolve_color(c, theme)),
        _ => None,
    };
    out
}

/// Numbers of the rule's ranges, sorted ascending.
fn numbers(ws: &Worksheet, sqref: &[CellRange]) -> Vec<f64> {
    let mut v = Vec::new();
    for_each_cell(ws, sqref, |_, _, val| {
        if let Value::Number(n) = val {
            v.push(n);
        }
    });
    v.sort_by(f64::total_cmp);
    v
}

/// Visits the existing cells of `sqref` (clipped to the used range).
fn for_each_cell(ws: &Worksheet, sqref: &[CellRange], mut f: impl FnMut(u32, u32, Value)) {
    let Some(used) = ws.used_range() else { return };
    for r in sqref {
        let r0 = r.first.row.max(used.first.row);
        let r1 = r.last.row.min(used.last.row);
        if r0 > r1 {
            continue;
        }
        for row in r0..=r1 {
            for (&(rr, cc), cell) in ws.cells.range((row, r.first.col)..=(row, r.last.col)) {
                f(rr, cc, to_formula_value(&cell.value));
            }
        }
    }
}

fn percentile(sorted: &[f64], p: f64) -> Option<f64> {
    if sorted.is_empty() {
        return None;
    }
    // PERCENTILE.INC interpolation.
    let k = (p / 100.0).clamp(0.0, 1.0) * (sorted.len() - 1) as f64;
    let lo = k.floor() as usize;
    let hi = k.ceil() as usize;
    let a = sorted[lo.min(sorted.len() - 1)];
    let b = sorted[hi.min(sorted.len() - 1)];
    Some(a + (b - a) * (k - lo as f64))
}

/// Threshold of a `cfvo` (§18.18.13 `ST_CfvoType`).
fn cfvo_value(c: &Cfvo, sorted: &[f64], m: &ModelCells<'_>, sheet: usize) -> Option<f64> {
    let min = *sorted.first()?;
    let max = *sorted.last()?;
    let num = |m: &ModelCells<'_>| -> Option<f64> {
        let v = c.val.as_deref()?;
        v.trim()
            .parse::<f64>()
            .ok()
            .or_else(|| m.eval_at(sheet, v, (0, 0), 0, 0).first().as_number().ok())
    };
    match c.kind.as_str() {
        "min" => Some(min),
        "max" => Some(max),
        "autoMin" => Some(min.min(0.0)),
        "autoMax" => Some(max.max(0.0)),
        "percent" => num(m).map(|p| min + (max - min) * p / 100.0),
        "percentile" => num(m).and_then(|p| percentile(sorted, p)),
        _ => num(m),
    }
}

fn lerp(a: Rgba, b: Rgba, t: f64) -> Rgba {
    let t = t.clamp(0.0, 1.0);
    let ch = |x: u8, y: u8| (f64::from(x) + (f64::from(y) - f64::from(x)) * t).round() as u8;
    Rgba(ch(a.0, b.0), ch(a.1, b.1), ch(a.2, b.2), 255)
}

fn truthy(v: &Value) -> bool {
    match v.first() {
        Value::Bool(b) => *b,
        Value::Number(n) => *n != 0.0,
        _ => false,
    }
}

/// Rule context computed once per rule.
struct Prepared<'r> {
    rule: &'r CfRule,
    sqref: &'r [CellRange],
    origin: (u32, u32),
    exprs: Vec<bo_formula::Expr>,
    sorted: Vec<f64>,
    thresholds: Vec<Option<f64>>,
    average: Option<(f64, f64)>,
    counts: HashMap<String, usize>,
}

fn attr<'a>(r: &'a CfRule, name: &str) -> Option<&'a str> {
    r.attrs
        .iter()
        .find(|(k, _)| k == name)
        .map(|(_, v)| v.as_str())
}

fn key_of(v: &Value) -> Option<String> {
    match v {
        Value::Empty => None,
        Value::Number(n) => Some(format!("n{n}")),
        Value::Text(t) => Some(format!("t{}", t.to_lowercase())),
        Value::Bool(b) => Some(format!("b{b}")),
        other => Some(format!("e{}", other.display())),
    }
}

impl<'r> Prepared<'r> {
    fn new(
        rule: &'r CfRule,
        cf: &'r ConditionalFormat,
        ws: &Worksheet,
        m: &ModelCells<'_>,
        sheet: usize,
    ) -> Self {
        let origin = cf
            .sqref
            .first()
            .map_or((0, 0), |r| (r.first.row, r.first.col));
        let needs_numbers = matches!(
            rule.kind.as_str(),
            "colorScale" | "dataBar" | "iconSet" | "top10" | "aboveAverage"
        );
        let sorted = if needs_numbers {
            numbers(ws, &cf.sqref)
        } else {
            Vec::new()
        };
        let cfvos: &[Cfvo] = match (&rule.color_scale, &rule.data_bar, &rule.icon_set) {
            (Some(c), _, _) if rule.kind == "colorScale" => &c.cfvos,
            // [MS-XLSX] x14:dataBar thresholds (autoMin / autoMax) take precedence.
            (_, Some(d), _) if rule.kind == "dataBar" => match &d.x14 {
                Some(x) if x.cfvos.len() == 2 => &x.cfvos,
                _ => &d.cfvos,
            },
            (_, _, Some(i)) if rule.kind == "iconSet" => &i.cfvos,
            _ => &[],
        };
        let thresholds = cfvos
            .iter()
            .map(|c| cfvo_value(c, &sorted, m, sheet))
            .collect();
        let average = (rule.kind == "aboveAverage" && !sorted.is_empty()).then(|| {
            let n = sorted.len() as f64;
            let mean = sorted.iter().sum::<f64>() / n;
            let var = sorted.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / n;
            (mean, var.sqrt())
        });
        let mut counts = HashMap::new();
        if matches!(rule.kind.as_str(), "duplicateValues" | "uniqueValues") {
            for_each_cell(ws, &cf.sqref, |_, _, v| {
                if let Some(k) = key_of(&v) {
                    *counts.entry(k).or_insert(0) += 1;
                }
            });
        }
        Prepared {
            rule,
            sqref: &cf.sqref,
            origin,
            exprs: rule.formulas.iter().filter_map(|f| parse(f).ok()).collect(),
            sorted,
            thresholds,
            average,
            counts,
        }
    }

    fn eval(&self, m: &ModelCells<'_>, sheet: usize, i: usize, row: u32, col: u32) -> Value {
        self.exprs.get(i).map_or(Value::Empty, |e| {
            m.eval_expr(sheet, e, self.origin, row, col)
        })
    }

    /// Whether a dxf-type rule matches the cell.
    #[allow(clippy::too_many_lines)]
    fn matches(&self, m: &ModelCells<'_>, sheet: usize, row: u32, col: u32, v: &Value) -> bool {
        let r = self.rule;
        let text = || match v {
            Value::Empty => String::new(),
            Value::Text(t) => t.clone(),
            other => other.display(),
        };
        let needle = || r.text.clone().unwrap_or_default().to_lowercase();
        match r.kind.as_str() {
            "cellIs" => {
                let cell = match v {
                    Value::Empty => Value::Number(0.0),
                    other => other.clone(),
                };
                let a = self.eval(m, sheet, 0, row, col);
                let cmp = |x: &Value| compare_values(&cell, x.first());
                match r.operator.as_deref().unwrap_or("equal") {
                    "lessThan" => cmp(&a) == Ordering::Less,
                    "lessThanOrEqual" => cmp(&a) != Ordering::Greater,
                    "greaterThan" => cmp(&a) == Ordering::Greater,
                    "greaterThanOrEqual" => cmp(&a) != Ordering::Less,
                    "notEqual" => cmp(&a) != Ordering::Equal,
                    "between" | "notBetween" => {
                        let b = self.eval(m, sheet, 1, row, col);
                        let (lo, hi) = if compare_values(a.first(), b.first()) == Ordering::Greater
                        {
                            (b, a)
                        } else {
                            (a, b)
                        };
                        let inside = cmp(&lo) != Ordering::Less && cmp(&hi) != Ordering::Greater;
                        inside == (r.operator.as_deref() == Some("between"))
                    }
                    _ => cmp(&a) == Ordering::Equal,
                }
            }
            "expression" => truthy(&self.eval(m, sheet, 0, row, col)),
            "containsText" => text().to_lowercase().contains(&needle()),
            "notContainsText" => !text().to_lowercase().contains(&needle()),
            "beginsWith" => text().to_lowercase().starts_with(&needle()),
            "endsWith" => text().to_lowercase().ends_with(&needle()),
            "containsBlanks" => text().trim().is_empty(),
            "notContainsBlanks" => !text().trim().is_empty(),
            "containsErrors" => v.is_error(),
            "notContainsErrors" => !v.is_error(),
            "duplicateValues" | "uniqueValues" => {
                let n = key_of(v)
                    .and_then(|k| self.counts.get(&k).copied())
                    .unwrap_or(0);
                if r.kind == "duplicateValues" {
                    n > 1
                } else {
                    n == 1
                }
            }
            "top10" => {
                let Value::Number(x) = v else { return false };
                let rank: f64 = attr(r, "rank").and_then(|s| s.parse().ok()).unwrap_or(10.0);
                let bottom = attr(r, "bottom") == Some("1");
                let percent = attr(r, "percent") == Some("1");
                let n = self.sorted.len();
                if n == 0 {
                    return false;
                }
                let k = if percent {
                    ((n as f64) * rank / 100.0).floor().max(1.0) as usize
                } else {
                    rank.max(1.0) as usize
                }
                .min(n);
                if bottom {
                    *x <= self.sorted[k - 1]
                } else {
                    *x >= self.sorted[n - k]
                }
            }
            "aboveAverage" => {
                let (Value::Number(x), Some((mean, sd))) = (v, self.average) else {
                    return false;
                };
                let above = attr(r, "aboveAverage") != Some("0");
                let equal = attr(r, "equalAverage") == Some("1");
                let std: f64 = attr(r, "stdDev")
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0.0);
                let limit = if above {
                    mean + std * sd
                } else {
                    mean - std * sd
                };
                match (above, equal) {
                    (true, true) => *x >= limit,
                    (true, false) => *x > limit,
                    (false, true) => *x <= limit,
                    (false, false) => *x < limit,
                }
            }
            _ => false,
        }
    }

    /// Format of a scale / bar / icon rule for a numeric cell.
    fn graded(&self, wb: &Workbook, x: f64) -> Option<CellFormat> {
        let theme = wb.theme.as_ref();
        let th = &self.thresholds;
        match self.rule.kind.as_str() {
            "colorScale" => {
                let cs = self.rule.color_scale.as_ref()?;
                let colors: Vec<Rgba> = cs
                    .colors
                    .iter()
                    .map(|c| resolve_color(c, theme).unwrap_or(Rgba(255, 255, 255, 255)))
                    .collect();
                let pts: Vec<f64> = th.iter().map(|t| t.unwrap_or(0.0)).collect();
                if colors.len() < 2 || pts.len() < colors.len() {
                    return None;
                }
                let fill = if x <= pts[0] {
                    colors[0]
                } else if x >= pts[colors.len() - 1] {
                    colors[colors.len() - 1]
                } else {
                    let i = (1..colors.len()).find(|&i| x <= pts[i]).unwrap_or(1);
                    let span = pts[i] - pts[i - 1];
                    let t = if span > 0.0 {
                        (x - pts[i - 1]) / span
                    } else {
                        1.0
                    };
                    lerp(colors[i - 1], colors[i], t)
                };
                Some(CellFormat {
                    fill: Some(fill),
                    ..CellFormat::default()
                })
            }
            "dataBar" => {
                let db = self.rule.data_bar.as_ref()?;
                let lo = th.first().copied().flatten()?;
                let hi = th.get(1).copied().flatten()?;
                Some(CellFormat {
                    bar: Some(data_bar_draw(db, lo, hi, x, theme)),
                    hide_value: db.show_value == Some(false),
                    ..CellFormat::default()
                })
            }
            "iconSet" => {
                let is = self.rule.icon_set.as_ref()?;
                let index = icon_index(is, th, x);
                let icon = if is.custom && !is.icons.is_empty() {
                    // [MS-XLSX] x14:cfIcon: the icon of each band is chosen explicitly.
                    is.icons.get(index).and_then(|ic| {
                        (ic.set != "NoIcons").then(|| IconDraw {
                            set: ic.set.clone(),
                            index: ic.id as usize,
                            count: crate::icons::icon_count(&ic.set),
                        })
                    })
                } else {
                    let set = is
                        .icon_set
                        .clone()
                        .unwrap_or_else(|| "3TrafficLights1".into());
                    let count = crate::icons::icon_count(&set);
                    let index = if is.reverse == Some(true) {
                        count.saturating_sub(1).saturating_sub(index)
                    } else {
                        index
                    };
                    (set != "NoIcons").then_some(IconDraw { set, index, count })
                };
                Some(CellFormat {
                    icon,
                    hide_value: is.show_value == Some(false),
                    ..CellFormat::default()
                })
            }
            _ => None,
        }
    }
}

/// Band of `x` in an icon set (0 = lowest): the highest threshold `i ≥ 1` that `x`
/// reaches (`≥`, or `>` when `gte="0"`, §18.3.1.11); the first `cfvo` is the floor.
#[must_use]
pub fn icon_index(is: &IconSet, thresholds: &[Option<f64>], x: f64) -> usize {
    let count = crate::icons::icon_count(is.icon_set.as_deref().unwrap_or("3TrafficLights1"))
        .max(is.icons.len())
        .min(is.cfvos.len().max(1));
    let mut index = 0;
    for (i, c) in is
        .cfvos
        .iter()
        .enumerate()
        .skip(1)
        .take(count.saturating_sub(1))
    {
        let Some(t) = thresholds.get(i).copied().flatten() else {
            continue;
        };
        let pass = if c.gte == Some(false) { x > t } else { x >= t };
        if pass {
            index = i;
        }
    }
    index
}

fn darker(c: Rgba) -> Rgba {
    let d = |v: u8| (f64::from(v) * 0.75).round() as u8;
    Rgba(d(c.0), d(c.1), d(c.2), 255)
}

/// The bar of value `x` for a data bar rule whose thresholds evaluate to `lo`, `hi`.
fn data_bar_draw(
    db: &DataBar,
    lo: f64,
    hi: f64,
    x: f64,
    theme: Option<&bo_model::sheet::ThemeColors>,
) -> BarDraw {
    let col = |c: &Option<SheetColor>| c.as_ref().and_then(|c| resolve_color(c, theme));
    let ext = db.x14.as_ref();
    let pct = |v: Option<u32>, d: u32| f64::from(v.unwrap_or(d).min(100)) / 100.0;
    let min_len = pct(ext.and_then(|e| e.min_length).or(db.min_length), 10);
    let max_len = pct(ext.and_then(|e| e.max_length).or(db.max_length), 90);
    // A 2006 data bar has no axis (§18.3.1.28); x14 defaults to `automatic`.
    let axis = ext.map_or("none", |e| {
        e.axis_position.as_deref().unwrap_or("automatic")
    });
    let (mut start, mut end, mut axis_at, mut leftward) =
        bar_geometry(x, lo, hi, min_len, max_len, axis);
    if ext.and_then(|e| e.direction.as_deref()) == Some("rightToLeft") {
        (start, end) = (1.0 - end, 1.0 - start);
        axis_at = axis_at.map(|a| 1.0 - a);
        leftward = !leftward;
    }
    let pos = ext
        .and_then(|e| col(&e.fill_color))
        .or_else(|| col(&db.color))
        .unwrap_or(BAR_BLUE);
    let negative = x < 0.0 && ext.is_some();
    let color = match ext {
        Some(e) if negative && !e.negative_same_color => {
            col(&e.negative_fill_color).unwrap_or(BAR_RED)
        }
        _ => pos,
    };
    let border = ext.filter(|e| e.border).map(|e| {
        let pos_border = col(&e.border_color).unwrap_or_else(|| darker(pos));
        // negativeBarBorderColorSameAsPositive defaults to true.
        if negative && e.negative_same_border == Some(false) {
            col(&e.negative_border_color).unwrap_or_else(|| darker(color))
        } else {
            pos_border
        }
    });
    BarDraw {
        start,
        end,
        color,
        border,
        gradient: ext.is_none_or(|e| e.gradient != Some(false)),
        leftward,
        axis: axis_at.filter(|a| *a > 0.0 && *a < 1.0),
        axis_color: ext
            .and_then(|e| col(&e.axis_color))
            .unwrap_or(Rgba(0x20, 0x20, 0x20, 255)),
    }
}

/// Conditional formatting of the cells in `rows` × `cols` of sheet `sheet`
/// (positions without a cell included, so fills of empty cells show).
pub fn evaluate(
    wb: &Workbook,
    sheet: usize,
    rows: Range<u32>,
    cols: Range<u32>,
    now: Option<f64>,
) -> HashMap<(u32, u32), CellFormat> {
    let mut out: HashMap<(u32, u32), CellFormat> = HashMap::new();
    let Some(ws) = wb.sheets.get(sheet) else {
        return out;
    };
    if ws.conditional_formats.is_empty() || rows.is_empty() || cols.is_empty() {
        return out;
    }
    let m = ModelCells::new(wb, now);
    let mut rules: Vec<(&CfRule, &ConditionalFormat)> = ws
        .conditional_formats
        .iter()
        .flat_map(|cf| cf.rules.iter().map(move |r| (r, cf)))
        .collect();
    rules.sort_by_key(|(r, _)| r.priority);
    let mut stopped: HashSet<(u32, u32)> = HashSet::new();
    for (rule, cf) in rules {
        let visible: Vec<CellRange> = cf
            .sqref
            .iter()
            .filter_map(|r| {
                let r0 = r.first.row.max(rows.start);
                let r1 = r.last.row.min(rows.end - 1);
                let c0 = r.first.col.max(cols.start);
                let c1 = r.last.col.min(cols.end - 1);
                (r0 <= r1 && c0 <= c1)
                    .then(|| CellRange::new(CellRef::new(r0, c0), CellRef::new(r1, c1)))
            })
            .collect();
        if visible.is_empty() {
            continue;
        }
        let p = Prepared::new(rule, cf, ws, &m, sheet);
        let dxf = rule
            .dxf_id
            .and_then(|i| wb.styles.dxfs.get(i as usize))
            .map(|d| dxf_format(d, wb));
        // Cells already formatted by this rule (sqref ranges may overlap).
        let mut done: HashSet<(u32, u32)> = HashSet::new();
        for vr in &visible {
            for row in vr.first.row..=vr.last.row {
                for col in vr.first.col..=vr.last.col {
                    if stopped.contains(&(row, col)) || !done.insert((row, col)) {
                        continue;
                    }
                    let v = ws
                        .cell(row, col)
                        .map_or(Value::Empty, |c| to_formula_value(&c.value));
                    let fmt = match rule.kind.as_str() {
                        "colorScale" | "dataBar" | "iconSet" => match v {
                            Value::Number(x) => p.graded(wb, x),
                            _ => None,
                        },
                        _ => (p.matches(&m, sheet, row, col, &v))
                            .then(|| dxf.clone().unwrap_or_default()),
                    };
                    if let Some(f) = fmt {
                        out.entry((row, col)).or_default().merge_from(&f);
                        if rule.stop_if_true {
                            stopped.insert((row, col));
                        }
                    }
                }
            }
        }
        let _ = p.sqref;
    }
    out
}

/// Formatting of a highlight rule created from the UI.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CfStyle {
    /// Fill `RRGGBB`.
    pub fill: Option<String>,
    /// Font colour `RRGGBB`.
    pub font_color: Option<String>,
    /// Bold text (`dxf/font/b`).
    pub bold: bool,
    /// Italic text (`dxf/font/i`).
    pub italic: bool,
}

/// A threshold of a data bar or icon set (§18.3.1.11 `cfvo`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CfvoSpec {
    /// `type` (§18.18.13 `ST_CfvoType`): num, percent, percentile, formula, min, max,
    /// and for data bars autoMin / autoMax ([MS-XLSX] x14).
    pub kind: String,
    /// Number, percent or formula of the threshold (unused for min / max).
    #[serde(default)]
    pub value: Option<String>,
    /// `gte`: the band starts at values ≥ the threshold (false: >).
    #[serde(default = "yes")]
    pub gte: bool,
}

fn yes() -> bool {
    true
}

fn hundred() -> u32 {
    100
}

fn automatic() -> String {
    "automatic".into()
}

fn context() -> String {
    "context".into()
}

/// Options of a data bar ([MS-XLSX] x14 `CT_DataBar`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DataBarOptions {
    /// Lower threshold (`autoMin` = the lowest value, or 0 when it is positive).
    pub min: CfvoSpec,
    /// Upper threshold (`autoMax` = the highest value, or 0 when it is negative).
    pub max: CfvoSpec,
    /// `minLength` in percent.
    #[serde(default)]
    pub min_length: u32,
    /// `maxLength` in percent.
    #[serde(default = "hundred")]
    pub max_length: u32,
    /// `gradient`: fade the fill (false = solid fill).
    #[serde(default = "yes")]
    pub gradient: bool,
    /// `border`: outline the bar.
    #[serde(default)]
    pub border: bool,
    /// `borderColor` `RRGGBB`.
    #[serde(default)]
    pub border_color: Option<String>,
    /// `negativeFillColor` `RRGGBB`.
    #[serde(default)]
    pub negative_color: Option<String>,
    /// `negativeBorderColor` `RRGGBB`.
    #[serde(default)]
    pub negative_border_color: Option<String>,
    /// `negativeBarColorSameAsPositive`.
    #[serde(default)]
    pub negative_same_color: bool,
    /// `negativeBarBorderColorSameAsPositive`.
    #[serde(default = "yes")]
    pub negative_same_border: bool,
    /// `axisPosition`: automatic, middle or none.
    #[serde(default = "automatic")]
    pub axis_position: String,
    /// `axisColor` `RRGGBB`.
    #[serde(default)]
    pub axis_color: Option<String>,
    /// `direction`: context, leftToRight or rightToLeft.
    #[serde(default = "context")]
    pub direction: String,
    /// `showValue`: false shows the bar only.
    #[serde(default = "yes")]
    pub show_value: bool,
}

/// An icon chosen for one band of a custom icon set ([MS-XLSX] `x14:cfIcon`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CfIconSpec {
    /// Icon set the icon comes from (`NoIcons` = no icon).
    pub set: String,
    /// Index of the icon in that set (0 = the set's lowest icon).
    pub id: u32,
}

/// Options of an icon set (§18.3.1.49 `iconSet`, [MS-XLSX] x14 `CT_IconSet`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IconSetOptions {
    /// Thresholds of the bands above the lowest, in ascending order (count − 1).
    pub thresholds: Vec<CfvoSpec>,
    /// `showValue`: false shows the icon only.
    #[serde(default = "yes")]
    pub show_value: bool,
    /// Custom icon of each band, lowest band first; empty = the set's own icons.
    #[serde(default)]
    pub icons: Vec<CfIconSpec>,
}

/// A rule created by the "add rule" dialog.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum CfRuleSpec {
    /// §18.18.12 `cellIs` with an operator (§18.18.15 `ST_ConditionalFormattingOperator`).
    CellIs {
        /// Operator name, e.g. `greaterThan`, `between`.
        operator: String,
        /// First operand formula (or constant).
        formula1: String,
        /// Second operand formula, for `between` / `notBetween`.
        #[serde(default)]
        formula2: Option<String>,
        /// Formatting applied to matching cells.
        style: CfStyle,
    },
    /// Formula evaluated for each cell, relative to the top-left cell.
    Expression {
        /// Boolean formula; the rule applies where it is true.
        formula: String,
        /// Formatting applied to matching cells.
        style: CfStyle,
    },
    /// §18.18.12 `containsText`: the cell text contains `text` (case-insensitive).
    ContainsText {
        /// Substring searched for.
        text: String,
        /// Formatting applied to matching cells.
        style: CfStyle,
    },
    /// §18.18.12 `top10`: the highest (or lowest) ranked values.
    Top10 {
        /// Number of items, or percentage when `percent` is set.
        rank: u32,
        /// Select the lowest values instead of the highest.
        #[serde(default)]
        bottom: bool,
        /// `rank` is a percentage of the cells in the range.
        #[serde(default)]
        percent: bool,
        /// Formatting applied to matching cells.
        style: CfStyle,
    },
    /// §18.18.12 `duplicateValues`: values that occur more than once in the range.
    DuplicateValues {
        /// Formatting applied to the duplicated cells.
        style: CfStyle,
    },
    /// 2- or 3-colour scale (min [, 50th percentile], max).
    ColorScale {
        /// Colour (`RRGGBB`) of the lowest value.
        min_color: String,
        /// Colour (`RRGGBB`) of the 50th percentile; `None` = 2-colour scale.
        #[serde(default)]
        mid_color: Option<String>,
        /// Colour (`RRGGBB`) of the highest value.
        max_color: String,
    },
    /// §18.3.1.28 `dataBar` with its x14 extension (negative bars, axis, border).
    DataBar {
        /// Bar colour `RRGGBB`.
        color: String,
        /// Thresholds, lengths, negative bars and axis; `None` = defaults
        /// (autoMin / autoMax, automatic axis, gradient fill).
        #[serde(default)]
        options: Option<DataBarOptions>,
    },
    /// §18.3.1.49 `iconSet`.
    IconSet {
        /// Icon set name (§18.18.42 `ST_IconSetType` or an x14 set), e.g. `3Arrows`.
        set: String,
        /// Reverse the icon order (`reverse` attribute).
        #[serde(default)]
        reverse: bool,
        /// Thresholds, icon-only display and custom icons; `None` = equal percent bands.
        #[serde(default)]
        options: Option<IconSetOptions>,
    },
}

fn rgb(hex: &str) -> SheetColor {
    SheetColor::rgb(&hex.trim_start_matches('#').to_ascii_uppercase())
}

fn dxf_of(s: &CfStyle) -> Dxf {
    let color = |h: &Option<String>| h.as_deref().filter(|h| h.len() >= 6).map(rgb);
    let font = (s.bold || s.italic || s.font_color.is_some()).then(|| Font {
        bold: s.bold,
        italic: s.italic,
        color: color(&s.font_color),
        ..Font::default()
    });
    let fill = color(&s.fill).map(|c| {
        Fill::Pattern(PatternFill {
            pattern: Some("solid".into()),
            fg: Some(c.clone()),
            bg: Some(c),
        })
    });
    Dxf {
        font,
        fill,
        ..Dxf::default()
    }
}

fn cfvo(kind: &str, val: Option<&str>) -> Cfvo {
    Cfvo {
        kind: kind.into(),
        val: val.map(str::to_owned),
        gte: None,
    }
}

fn cfvo_of(s: &CfvoSpec) -> Cfvo {
    let fixed = matches!(s.kind.as_str(), "min" | "max" | "autoMin" | "autoMax");
    Cfvo {
        kind: s.kind.clone(),
        val: if fixed {
            None
        } else {
            Some(
                s.value
                    .as_deref()
                    .unwrap_or("0")
                    .trim()
                    .trim_start_matches('=')
                    .to_owned(),
            )
        },
        gte: (!s.gte).then_some(false),
    }
}

/// `"text"` literal for formulas.
fn quoted(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "\"\""))
}

/// Builds the model rule of `spec` for `range` (dxfs are added to the workbook).
/// The priority is left at 1. `None` when the spec is unusable.
#[allow(clippy::too_many_lines)]
pub fn build_rule(wb: &mut Workbook, range: CellRange, spec: &CfRuleSpec) -> Option<CfRule> {
    let mut add_dxf = |s: &CfStyle| -> u32 {
        wb.styles.dxfs.push(dxf_of(s));
        u32::try_from(wb.styles.dxfs.len() - 1).unwrap_or(0)
    };
    let top_left = range.first.to_string();
    let mut rule = CfRule {
        priority: 1,
        ..CfRule::default()
    };
    match spec {
        CfRuleSpec::CellIs {
            operator,
            formula1,
            formula2,
            style,
        } => {
            if formula1.trim().is_empty() {
                return None;
            }
            rule.kind = "cellIs".into();
            rule.operator = Some(operator.clone());
            rule.dxf_id = Some(add_dxf(style));
            rule.formulas
                .push(formula1.trim().trim_start_matches('=').to_owned());
            if let Some(f2) = formula2.as_deref().filter(|f| !f.trim().is_empty()) {
                rule.formulas
                    .push(f2.trim().trim_start_matches('=').to_owned());
            }
        }
        CfRuleSpec::Expression { formula, style } => {
            let f = formula.trim().trim_start_matches('=');
            if f.is_empty() || parse(f).is_err() {
                return None;
            }
            rule.kind = "expression".into();
            rule.dxf_id = Some(add_dxf(style));
            rule.formulas.push(f.to_owned());
        }
        CfRuleSpec::ContainsText { text, style } => {
            rule.kind = "containsText".into();
            rule.operator = Some("containsText".into());
            rule.text = Some(text.clone());
            rule.dxf_id = Some(add_dxf(style));
            rule.formulas
                .push(format!("NOT(ISERROR(SEARCH({},{top_left})))", quoted(text)));
        }
        CfRuleSpec::Top10 {
            rank,
            bottom,
            percent,
            style,
        } => {
            rule.kind = "top10".into();
            rule.dxf_id = Some(add_dxf(style));
            if *percent {
                rule.attrs.push(("percent".into(), "1".into()));
            }
            if *bottom {
                rule.attrs.push(("bottom".into(), "1".into()));
            }
            rule.attrs.push(("rank".into(), (*rank).max(1).to_string()));
        }
        CfRuleSpec::DuplicateValues { style } => {
            rule.kind = "duplicateValues".into();
            rule.dxf_id = Some(add_dxf(style));
        }
        CfRuleSpec::ColorScale {
            min_color,
            mid_color,
            max_color,
        } => {
            rule.kind = "colorScale".into();
            let mut cs = ColorScale::default();
            cs.cfvos.push(cfvo("min", None));
            cs.colors.push(rgb(min_color));
            if let Some(mid) = mid_color {
                cs.cfvos.push(cfvo("percentile", Some("50")));
                cs.colors.push(rgb(mid));
            }
            cs.cfvos.push(cfvo("max", None));
            cs.colors.push(rgb(max_color));
            rule.color_scale = Some(cs);
        }
        CfRuleSpec::DataBar { color, options } => {
            rule.kind = "dataBar".into();
            rule.data_bar = Some(data_bar_of(color, options.as_ref()));
        }
        CfRuleSpec::IconSet {
            set,
            reverse,
            options,
        } => {
            let count = crate::icons::icon_count(set);
            if count < 3 {
                return None;
            }
            rule.kind = "iconSet".into();
            let mut is = IconSet {
                icon_set: Some(set.clone()),
                cfvos: (0..count)
                    .map(|i| cfvo("percent", Some(&(i * 100 / count).to_string())))
                    .collect(),
                reverse: reverse.then_some(true),
                ..IconSet::default()
            };
            if let Some(o) = options {
                if o.thresholds.len() + 1 != count {
                    return None;
                }
                is.cfvos = std::iter::once(cfvo("percent", Some("0")))
                    .chain(o.thresholds.iter().map(cfvo_of))
                    .collect();
                is.show_value = (!o.show_value).then_some(false);
                if !o.icons.is_empty() {
                    if o.icons.len() != count {
                        return None;
                    }
                    is.custom = true;
                    is.icons = o
                        .icons
                        .iter()
                        .map(|i| bo_model::sheet::CfIcon {
                            set: i.set.clone(),
                            id: i.id,
                        })
                        .collect();
                }
            }
            rule.icon_set = Some(is);
        }
    }
    Some(rule)
}

/// The 2006 `dataBar` and its x14 extension for a bar of `color` with `options`.
fn data_bar_of(color: &str, options: Option<&DataBarOptions>) -> DataBar {
    let auto = CfvoSpec {
        kind: "autoMin".into(),
        value: None,
        gte: true,
    };
    let o = options.cloned().unwrap_or(DataBarOptions {
        min: auto.clone(),
        max: CfvoSpec {
            kind: "autoMax".into(),
            ..auto
        },
        min_length: 0,
        max_length: 100,
        gradient: true,
        border: false,
        border_color: None,
        negative_color: None,
        negative_border_color: None,
        negative_same_color: false,
        negative_same_border: true,
        axis_position: automatic(),
        axis_color: None,
        direction: context(),
        show_value: true,
    });
    // §18.18.13: the 2006 schema has no autoMin / autoMax.
    let legacy = |c: Cfvo| match c.kind.as_str() {
        "autoMin" => cfvo("min", None),
        "autoMax" => cfvo("max", None),
        _ => c,
    };
    let hex = |h: &Option<String>| h.as_deref().filter(|h| h.len() >= 6).map(rgb);
    let (min, max) = (cfvo_of(&o.min), cfvo_of(&o.max));
    DataBar {
        cfvos: vec![legacy(min.clone()), legacy(max.clone())],
        color: Some(rgb(color)),
        min_length: None,
        max_length: None,
        show_value: (!o.show_value).then_some(false),
        x14: Some(DataBarX14 {
            min_length: Some(o.min_length.min(100)),
            max_length: Some(o.max_length.clamp(o.min_length.min(100), 100)),
            border: o.border,
            gradient: (!o.gradient).then_some(false),
            direction: (o.direction != "context").then(|| o.direction.clone()),
            negative_same_color: o.negative_same_color,
            negative_same_border: (!o.negative_same_border).then_some(false),
            axis_position: (o.axis_position != "automatic").then(|| o.axis_position.clone()),
            cfvos: vec![min, max],
            fill_color: Some(rgb(color)),
            border_color: if o.border {
                hex(&o.border_color).or_else(|| Some(rgb(color)))
            } else {
                None
            },
            negative_fill_color: if o.negative_same_color {
                None
            } else {
                Some(hex(&o.negative_color).unwrap_or_else(|| rgb("D64545")))
            },
            negative_border_color: if o.border && !o.negative_same_border {
                hex(&o.negative_border_color).or_else(|| Some(rgb("D64545")))
            } else {
                None
            },
            axis_color: (o.axis_position != "none")
                .then(|| hex(&o.axis_color).unwrap_or_else(|| rgb("202020"))),
        }),
    }
}

/// Adds a rule on `range` with the highest priority. Returns `false` when
/// the sheet does not exist or the spec is unusable.
pub fn add_rule(wb: &mut Workbook, sheet: usize, range: CellRange, spec: &CfRuleSpec) -> bool {
    add_rule_in(wb, sheet, vec![range], spec, false).is_some()
}

/// Adds a rule on `sqref` with the highest priority (priority 1, the others shift
/// down); returns its (block, rule) position.
pub fn add_rule_in(
    wb: &mut Workbook,
    sheet: usize,
    sqref: Vec<CellRange>,
    spec: &CfRuleSpec,
    pivot: bool,
) -> Option<(usize, usize)> {
    if sheet >= wb.sheets.len() {
        return None;
    }
    let first = *sqref.first()?;
    let rule = build_rule(wb, first, spec)?;
    let ws = &mut wb.sheets[sheet];
    for cf in &mut ws.conditional_formats {
        for r in &mut cf.rules {
            r.priority += 1;
        }
    }
    for pt in &mut ws.pivots {
        for c in &mut pt.conditional_formats {
            c.priority += 1;
        }
    }
    let ext = rule.needs_ext();
    ws.conditional_formats.push(ConditionalFormat {
        sqref,
        pivot,
        rules: vec![rule],
        ext,
    });
    Some((ws.conditional_formats.len() - 1, 0))
}

/// Removes conditional formats whose ranges overlap `range`; returns how many.
pub fn clear_rules(wb: &mut Workbook, sheet: usize, range: CellRange) -> usize {
    let Some(ws) = wb.sheets.get_mut(sheet) else {
        return 0;
    };
    let before = ws.conditional_formats.len();
    ws.conditional_formats.retain(|cf| {
        !cf.sqref.iter().any(|r| {
            r.first.row <= range.last.row
                && range.first.row <= r.last.row
                && r.first.col <= range.last.col
                && range.first.col <= r.last.col
        })
    });
    before - ws.conditional_formats.len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use bo_model::sheet::{Cell, CellValue};

    fn rng(s: &str) -> CellRange {
        CellRange::parse(s).unwrap()
    }

    fn sheet() -> Workbook {
        let mut wb = Workbook::new();
        for (i, v) in [1.0, 5.0, 10.0, 15.0, 20.0].iter().enumerate() {
            wb.sheets[0].set_cell(i as u32, 0, Cell::new(CellValue::Number(*v)));
        }
        wb.sheets[0].set_cell(0, 1, Cell::new(CellValue::Text("Hà Nội".into())));
        wb.sheets[0].set_cell(1, 1, Cell::new(CellValue::Text("Huế".into())));
        wb
    }

    fn style(fill: &str) -> CfStyle {
        CfStyle {
            fill: Some(fill.into()),
            bold: true,
            ..CfStyle::default()
        }
    }

    #[test]
    fn cell_is_and_expression_rules() {
        let mut wb = sheet();
        assert!(add_rule(
            &mut wb,
            0,
            rng("A1:A5"),
            &CfRuleSpec::CellIs {
                operator: "greaterThan".into(),
                formula1: "9".into(),
                formula2: None,
                style: style("FF0000"),
            }
        ));
        let m = evaluate(&wb, 0, 0..10, 0..3, None);
        assert_eq!(
            m.get(&(2, 0)).and_then(|f| f.fill),
            Some(Rgba(255, 0, 0, 255))
        );
        assert_eq!(m.get(&(2, 0)).and_then(|f| f.bold), Some(true));
        assert!(!m.contains_key(&(1, 0)));
        // Expression on the whole row block with an absolute column (row highlight).
        assert!(add_rule(
            &mut wb,
            0,
            rng("A1:C5"),
            &CfRuleSpec::Expression {
                formula: "=$A1=5".into(),
                style: style("00FF00"),
            }
        ));
        let m = evaluate(&wb, 0, 0..10, 0..3, None);
        // New rule has priority over the older one; the empty C2 is filled too.
        assert_eq!(
            m.get(&(1, 2)).and_then(|f| f.fill),
            Some(Rgba(0, 255, 0, 255))
        );
        assert_eq!(
            m.get(&(2, 0)).and_then(|f| f.fill),
            Some(Rgba(255, 0, 0, 255))
        );
        assert!(!add_rule(
            &mut wb,
            0,
            rng("A1"),
            &CfRuleSpec::Expression {
                formula: "=(".into(),
                style: CfStyle::default()
            }
        ));
        // Between and text rules.
        add_rule(
            &mut wb,
            0,
            rng("B1:B5"),
            &CfRuleSpec::ContainsText {
                text: "nội".into(),
                style: style("0000FF"),
            },
        );
        let m = evaluate(&wb, 0, 0..10, 0..3, None);
        assert!(m
            .get(&(0, 1))
            .is_some_and(|f| f.fill == Some(Rgba(0, 0, 255, 255))));
        assert_ne!(
            m.get(&(1, 1)).and_then(|f| f.fill),
            Some(Rgba(0, 0, 255, 255))
        );
        assert_eq!(clear_rules(&mut wb, 0, rng("B1")), 2);
    }

    #[test]
    fn scales_bars_and_icons() {
        let mut wb = sheet();
        add_rule(
            &mut wb,
            0,
            rng("A1:A5"),
            &CfRuleSpec::ColorScale {
                min_color: "FFFFFF".into(),
                mid_color: None,
                max_color: "000000".into(),
            },
        );
        let m = evaluate(&wb, 0, 0..5, 0..1, None);
        assert_eq!(m[&(0, 0)].fill, Some(Rgba(255, 255, 255, 255)));
        assert_eq!(m[&(4, 0)].fill, Some(Rgba(0, 0, 0, 255)));
        // 10 is (10-1)/19 of the way.
        let g = m[&(2, 0)].fill.unwrap().0;
        assert_eq!(g, (255.0 - 255.0 * 9.0 / 19.0_f64).round() as u8);
        wb.sheets[0].conditional_formats.clear();
        add_rule(
            &mut wb,
            0,
            rng("A1:A5"),
            &CfRuleSpec::DataBar {
                color: "4A7BB7".into(),
                options: None,
            },
        );
        let m = evaluate(&wb, 0, 0..5, 0..1, None);
        // Defaults: autoMin = min(1, 0) = 0, autoMax = 20, lengths 0–100 %, no axis
        // (all values positive): 1 → 1/20, 20 → 1.
        let b0 = m[&(0, 0)].bar.unwrap();
        assert!((b0.start, b0.end) == (0.0, 0.05) && b0.axis.is_none());
        assert!((m[&(4, 0)].bar.unwrap().end - 1.0).abs() < 1e-9);
        // A 2006-only bar (no x14) uses min / max and the 10 % / 90 % defaults.
        wb.sheets[0].conditional_formats[0].rules[0]
            .data_bar
            .as_mut()
            .unwrap()
            .x14 = None;
        let m = evaluate(&wb, 0, 0..5, 0..1, None);
        assert!((m[&(0, 0)].bar.unwrap().end - 0.1).abs() < 1e-9);
        assert!((m[&(4, 0)].bar.unwrap().end - 0.9).abs() < 1e-9);
        wb.sheets[0].conditional_formats.clear();
        add_rule(
            &mut wb,
            0,
            rng("A1:A5"),
            &CfRuleSpec::IconSet {
                set: "3Arrows".into(),
                reverse: false,
                options: None,
            },
        );
        let m = evaluate(&wb, 0, 0..5, 0..1, None);
        let idx = |r| m[&(r, 0)].icon.as_ref().unwrap().index;
        assert_eq!((idx(0), idx(2), idx(4)), (0, 1, 2));
        assert_eq!(m[&(0, 0)].icon.as_ref().unwrap().count, 3);
    }

    #[test]
    fn top_duplicate_and_stop() {
        let mut wb = sheet();
        wb.sheets[0].set_cell(5, 0, Cell::new(CellValue::Number(20.0)));
        add_rule(
            &mut wb,
            0,
            rng("A1:A6"),
            &CfRuleSpec::DuplicateValues {
                style: style("111111"),
            },
        );
        add_rule(
            &mut wb,
            0,
            rng("A1:A6"),
            &CfRuleSpec::Top10 {
                rank: 2,
                bottom: true,
                percent: false,
                style: style("222222"),
            },
        );
        let m = evaluate(&wb, 0, 0..6, 0..1, None);
        assert_eq!(m[&(0, 0)].fill, Some(Rgba(0x22, 0x22, 0x22, 255)));
        assert_eq!(m[&(5, 0)].fill, Some(Rgba(0x11, 0x11, 0x11, 255)));
        assert!(!m.contains_key(&(2, 0)));
        wb.sheets[0].conditional_formats[1].rules[0].stop_if_true = true;
        let _ = evaluate(&wb, 0, 0..6, 0..1, None);
        assert_eq!(percentile(&[1.0, 2.0, 3.0], 50.0), Some(2.0));
    }

    fn close(a: (f64, f64, Option<f64>, bool), b: (f64, f64, Option<f64>, bool)) -> bool {
        (a.0 - b.0).abs() < 1e-9
            && (a.1 - b.1).abs() < 1e-9
            && a.3 == b.3
            && match (a.2, b.2) {
                (Some(x), Some(y)) => (x - y).abs() < 1e-9,
                (None, None) => true,
                _ => false,
            }
    }

    #[test]
    fn bar_geometry_by_hand() {
        // Values −10 … 30, lengths 0–100 %: zero sits at 10/40 = 25 % of the cell.
        let g = |x, axis| bar_geometry(x, -10.0, 30.0, 0.0, 1.0, axis);
        assert!(close(g(30.0, "automatic"), (0.25, 1.0, Some(0.25), false)));
        // 10 is a third of the positive extreme: 0.75 · 1/3 = 0.25 wide.
        assert!(close(g(10.0, "automatic"), (0.25, 0.5, Some(0.25), false)));
        assert!(close(g(-10.0, "automatic"), (0.0, 0.25, Some(0.25), true)));
        assert!(close(g(0.0, "automatic"), (0.25, 0.25, Some(0.25), false)));
        // Middle axis: both sides scaled by max(|−10|, |30|) = 30 over half the cell.
        assert!(close(g(30.0, "middle"), (0.5, 1.0, Some(0.5), false)));
        assert!(close(
            g(-10.0, "middle"),
            (0.5 - 0.5 / 3.0, 0.5, Some(0.5), true)
        ));
        // No axis: linear from −10 (left edge) to 30.
        assert!(close(g(10.0, "none"), (0.0, 0.5, None, false)));
        assert!(close(g(-10.0, "none"), (0.0, 0.0, None, false)));
        // Lengths 10–90 %: 30 takes 0.9 of the 0.75 positive side.
        let b = bar_geometry(30.0, -10.0, 30.0, 0.1, 0.9, "automatic");
        assert!(close(b, (0.25, 0.25 + 0.675, Some(0.25), false)));
        // All thresholds negative (−20 … −5): bars grow leftwards from the right edge;
        // −10 is a third of the way from −5 to −20.
        let n = |x| bar_geometry(x, -20.0, -5.0, 0.0, 1.0, "automatic");
        assert!(close(n(-5.0), (1.0, 1.0, None, true)));
        assert!(close(n(-20.0), (0.0, 1.0, None, true)));
        assert!(close(n(-10.0), (2.0 / 3.0, 1.0, None, true)));
    }

    fn numbers_sheet(vals: &[f64]) -> Workbook {
        let mut wb = Workbook::new();
        for (i, v) in vals.iter().enumerate() {
            wb.sheets[0].set_cell(i as u32, 0, Cell::new(CellValue::Number(*v)));
        }
        wb
    }

    #[test]
    fn negative_bars_colours_axis_and_direction() {
        let mut wb = numbers_sheet(&[-10.0, 0.0, 10.0, 30.0]);
        add_rule(
            &mut wb,
            0,
            rng("A1:A4"),
            &CfRuleSpec::DataBar {
                color: "4A7BB7".into(),
                options: None,
            },
        );
        let m = evaluate(&wb, 0, 0..4, 0..1, None);
        let neg = m[&(0, 0)].bar.unwrap();
        assert_eq!(neg.color, BAR_RED);
        assert!(neg.leftward && neg.gradient);
        assert_eq!((neg.start, neg.end), (0.0, 0.25));
        assert_eq!(neg.axis, Some(0.25));
        assert_eq!(m[&(3, 0)].bar.unwrap().color, Rgba(0x4A, 0x7B, 0xB7, 255));
        // Right to left, solid fill, border, negative bars in the positive colour.
        let mut o = match spec_of_first(&wb) {
            CfRuleSpec::DataBar {
                options: Some(o), ..
            } => o,
            other => panic!("{other:?}"),
        };
        o.direction = "rightToLeft".into();
        o.gradient = false;
        o.border = true;
        o.border_color = Some("000080".into());
        o.negative_same_color = true;
        wb.sheets[0].conditional_formats.clear();
        add_rule(
            &mut wb,
            0,
            rng("A1:A4"),
            &CfRuleSpec::DataBar {
                color: "4A7BB7".into(),
                options: Some(o),
            },
        );
        let m = evaluate(&wb, 0, 0..4, 0..1, None);
        let pos = m[&(3, 0)].bar.unwrap();
        // Mirrored: 30 spans 0 … 0.75, axis at 0.75.
        assert_eq!((pos.start, pos.end, pos.axis), (0.0, 0.75, Some(0.75)));
        assert!(pos.leftward && !pos.gradient);
        assert_eq!(pos.border, Some(Rgba(0, 0, 0x80, 255)));
        let neg = m[&(0, 0)].bar.unwrap();
        assert_eq!(neg.color, Rgba(0x4A, 0x7B, 0xB7, 255));
        assert_eq!((neg.start, neg.end), (0.75, 1.0));
        // Axis "none" draws no axis.
        let mut o = match spec_of_first(&wb) {
            CfRuleSpec::DataBar {
                options: Some(o), ..
            } => o,
            other => panic!("{other:?}"),
        };
        o.axis_position = "none".into();
        o.direction = "context".into();
        wb.sheets[0].conditional_formats.clear();
        add_rule(
            &mut wb,
            0,
            rng("A1:A4"),
            &CfRuleSpec::DataBar {
                color: "4A7BB7".into(),
                options: Some(o),
            },
        );
        let m = evaluate(&wb, 0, 0..4, 0..1, None);
        assert_eq!(m[&(2, 0)].bar.unwrap().axis, None);
        assert_eq!(m[&(2, 0)].bar.unwrap().end, 0.5);
    }

    fn spec_of_first(wb: &Workbook) -> CfRuleSpec {
        crate::cf_manage::spec_of(wb, &wb.sheets[0].conditional_formats[0].rules[0]).unwrap()
    }

    fn icons_with(wb: &mut Workbook, set: &str, reverse: bool, o: IconSetOptions) {
        wb.sheets[0].conditional_formats.clear();
        assert!(add_rule(
            wb,
            0,
            rng("A1:A10"),
            &CfRuleSpec::IconSet {
                set: set.into(),
                reverse,
                options: Some(o),
            },
        ));
    }

    fn th(kind: &str, value: &str, gte: bool) -> CfvoSpec {
        CfvoSpec {
            kind: kind.into(),
            value: Some(value.into()),
            gte,
        }
    }

    #[test]
    fn icon_thresholds_by_hand() {
        let vals: Vec<f64> = (1..=10).map(f64::from).collect();
        let mut wb = numbers_sheet(&vals);
        let idx = |wb: &Workbook| -> Vec<usize> {
            let m = evaluate(wb, 0, 0..10, 0..1, None);
            (0..10)
                .map(|r| m[&(r, 0)].icon.as_ref().map_or(99, |i| i.index))
                .collect()
        };
        // Numbers: ≥ 4 and > 7.
        icons_with(
            &mut wb,
            "3Arrows",
            false,
            IconSetOptions {
                thresholds: vec![th("num", "4", true), th("num", "7", false)],
                show_value: false,
                icons: vec![],
            },
        );
        assert_eq!(idx(&wb), vec![0, 0, 0, 1, 1, 1, 1, 2, 2, 2]);
        assert!(evaluate(&wb, 0, 0..1, 0..1, None)[&(0, 0)].hide_value);
        // Percent 50 of 1…10 = 1 + 9 · 0.5 = 5.5; percentile 80 (inclusive) of 1…10 =
        // 1 + 0.8 · 9 = 8.2 → 9 and 10 are top.
        icons_with(
            &mut wb,
            "3Flags",
            false,
            IconSetOptions {
                thresholds: vec![th("percent", "50", true), th("percentile", "80", true)],
                show_value: true,
                icons: vec![],
            },
        );
        assert_eq!(idx(&wb), vec![0, 0, 0, 0, 0, 1, 1, 1, 2, 2]);
        // Formula thresholds: =2*3 (≥ 6) and =SUM(9,0) (≥ 9); reversed icons.
        icons_with(
            &mut wb,
            "3Symbols",
            true,
            IconSetOptions {
                thresholds: vec![th("formula", "=2*3", true), th("formula", "SUM(9,0)", true)],
                show_value: true,
                icons: vec![],
            },
        );
        assert_eq!(idx(&wb), vec![2, 2, 2, 2, 2, 1, 1, 1, 0, 0]);
        // Five bands with ≥ vs > at the same number: 2 ≥ → band 1, 4 > → band 2 only above 4.
        icons_with(
            &mut wb,
            "5Boxes",
            false,
            IconSetOptions {
                thresholds: vec![
                    th("num", "2", true),
                    th("num", "4", false),
                    th("num", "6", true),
                    th("num", "8", false),
                ],
                show_value: true,
                icons: vec![],
            },
        );
        assert_eq!(idx(&wb), vec![0, 1, 1, 1, 2, 3, 3, 3, 4, 4]);
        assert!(wb.sheets[0].conditional_formats[0].ext);
        // Custom icons: lowest band a red flag, middle none, top a full star.
        icons_with(
            &mut wb,
            "3TrafficLights1",
            false,
            IconSetOptions {
                thresholds: vec![th("num", "4", true), th("num", "8", true)],
                show_value: true,
                icons: vec![
                    CfIconSpec {
                        set: "3Flags".into(),
                        id: 0,
                    },
                    CfIconSpec {
                        set: "NoIcons".into(),
                        id: 0,
                    },
                    CfIconSpec {
                        set: "3Stars".into(),
                        id: 2,
                    },
                ],
            },
        );
        let m = evaluate(&wb, 0, 0..10, 0..1, None);
        assert_eq!(
            m[&(0, 0)].icon,
            Some(IconDraw {
                set: "3Flags".into(),
                index: 0,
                count: 3
            })
        );
        assert_eq!(m[&(4, 0)].icon, None);
        assert_eq!(
            m[&(9, 0)].icon,
            Some(IconDraw {
                set: "3Stars".into(),
                index: 2,
                count: 3
            })
        );
        // Wrong number of thresholds is refused.
        assert!(!add_rule(
            &mut wb,
            0,
            rng("A1"),
            &CfRuleSpec::IconSet {
                set: "4Rating".into(),
                reverse: false,
                options: Some(IconSetOptions {
                    thresholds: vec![th("num", "1", true)],
                    show_value: true,
                    icons: vec![],
                }),
            },
        ));
    }

    #[test]
    fn stop_if_true_precedence() {
        let mut wb = numbers_sheet(&[3.0, 10.0]);
        let bold_green = CfStyle {
            fill: Some("00FF00".into()),
            bold: true,
            ..CfStyle::default()
        };
        // Lowest priority first: data bar (3), then "> 0" bold green (2), then "> 5" red (1).
        add_rule(
            &mut wb,
            0,
            rng("A1:A2"),
            &CfRuleSpec::DataBar {
                color: "4A7BB7".into(),
                options: None,
            },
        );
        add_rule(
            &mut wb,
            0,
            rng("A1:A2"),
            &CfRuleSpec::CellIs {
                operator: "greaterThan".into(),
                formula1: "0".into(),
                formula2: None,
                style: bold_green,
            },
        );
        add_rule(
            &mut wb,
            0,
            rng("A1:A2"),
            &CfRuleSpec::CellIs {
                operator: "greaterThan".into(),
                formula1: "5".into(),
                formula2: None,
                style: CfStyle {
                    fill: Some("FF0000".into()),
                    ..CfStyle::default()
                },
            },
        );
        // Without Stop If True every matching rule contributes what is still unset.
        let m = evaluate(&wb, 0, 0..2, 0..1, None);
        assert_eq!(m[&(1, 0)].fill, Some(Rgba(255, 0, 0, 255)));
        assert_eq!(m[&(1, 0)].bold, Some(true));
        assert!(m[&(1, 0)].bar.is_some());
        // Stop If True on "> 5": 10 keeps only the red fill; 3 is unaffected.
        let red = wb.sheets[0]
            .conditional_formats
            .iter()
            .position(|c| c.rules[0].priority == 1)
            .unwrap();
        wb.sheets[0].conditional_formats[red].rules[0].stop_if_true = true;
        let m = evaluate(&wb, 0, 0..2, 0..1, None);
        assert_eq!(m[&(1, 0)].fill, Some(Rgba(255, 0, 0, 255)));
        assert_eq!(m[&(1, 0)].bold, None);
        assert!(m[&(1, 0)].bar.is_none());
        assert_eq!(m[&(0, 0)].fill, Some(Rgba(0, 255, 0, 255)));
        assert!(m[&(0, 0)].bar.is_some());
    }
}
