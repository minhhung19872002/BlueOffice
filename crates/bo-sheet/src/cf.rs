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
    CellRange, CellRef, CfRule, Cfvo, ColorScale, ConditionalFormat, DataBar, Dxf, Fill, Font,
    IconSet, PatternFill, SheetColor, Workbook, Worksheet,
};
use bo_render::Rgba;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::ops::Range;

/// A data bar to draw in a cell: fraction of the cell width and colour.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BarDraw {
    /// Bar length as a fraction of the cell width (0.0–1.0, §18.3.1.28 `dataBar`).
    pub fraction: f64,
    /// Bar fill colour.
    pub color: Rgba,
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
            (_, Some(d), _) if rule.kind == "dataBar" => &d.cfvos,
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
                let t = if hi > lo { (x - lo) / (hi - lo) } else { 1.0 };
                let min_len = f64::from(db.min_length.unwrap_or(10)) / 100.0;
                let max_len = f64::from(db.max_length.unwrap_or(90)) / 100.0;
                let color = db
                    .color
                    .as_ref()
                    .and_then(|c| resolve_color(c, theme))
                    .unwrap_or(Rgba(0x4A, 0x7B, 0xB7, 255));
                Some(CellFormat {
                    bar: Some(BarDraw {
                        fraction: min_len + (max_len - min_len) * t.clamp(0.0, 1.0),
                        color,
                    }),
                    hide_value: db.show_value == Some(false),
                    ..CellFormat::default()
                })
            }
            "iconSet" => {
                let is = self.rule.icon_set.as_ref()?;
                let set = is
                    .icon_set
                    .clone()
                    .unwrap_or_else(|| "3TrafficLights1".into());
                let count = set
                    .chars()
                    .next()
                    .and_then(|c| c.to_digit(10))
                    .map_or(3, |d| d as usize)
                    .clamp(3, 5);
                let mut index = 0;
                for (i, c) in is.cfvos.iter().enumerate().skip(1).take(count - 1) {
                    let Some(t) = th.get(i).copied().flatten() else {
                        continue;
                    };
                    let pass = if c.gte == Some(false) { x > t } else { x >= t };
                    if pass {
                        index = i;
                    }
                }
                if is.reverse == Some(true) {
                    index = count - 1 - index;
                }
                Some(CellFormat {
                    icon: Some(IconDraw { set, index, count }),
                    hide_value: is.show_value == Some(false),
                    ..CellFormat::default()
                })
            }
            _ => None,
        }
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
    /// §18.3.1.28 `dataBar` with the default min/max value objects.
    DataBar {
        /// Bar colour `RRGGBB`.
        color: String,
    },
    /// §18.3.1.49 `iconSet` with percent thresholds.
    IconSet {
        /// Icon set name (§18.18.42 `ST_IconSetType`), e.g. `3Arrows`.
        set: String,
        /// Reverse the icon order (`reverse` attribute).
        #[serde(default)]
        reverse: bool,
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

/// `"text"` literal for formulas.
fn quoted(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "\"\""))
}

/// Adds a rule on `range` with the highest priority. Returns `false` when
/// the sheet does not exist or the spec is unusable.
pub fn add_rule(wb: &mut Workbook, sheet: usize, range: CellRange, spec: &CfRuleSpec) -> bool {
    if sheet >= wb.sheets.len() {
        return false;
    }
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
                return false;
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
                return false;
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
        CfRuleSpec::DataBar { color } => {
            rule.kind = "dataBar".into();
            rule.data_bar = Some(DataBar {
                cfvos: vec![cfvo("min", None), cfvo("max", None)],
                color: Some(rgb(color)),
                ..DataBar::default()
            });
        }
        CfRuleSpec::IconSet { set, reverse } => {
            let count = set
                .chars()
                .next()
                .and_then(|c| c.to_digit(10))
                .unwrap_or(3)
                .clamp(3, 5);
            rule.kind = "iconSet".into();
            rule.icon_set = Some(IconSet {
                icon_set: Some(set.clone()),
                cfvos: (0..count)
                    .map(|i| cfvo("percent", Some(&(i * 100 / count).to_string())))
                    .collect(),
                reverse: reverse.then_some(true),
                ..IconSet::default()
            });
        }
    }
    let ws = &mut wb.sheets[sheet];
    for cf in &mut ws.conditional_formats {
        for r in &mut cf.rules {
            r.priority += 1;
        }
    }
    ws.conditional_formats.push(ConditionalFormat {
        sqref: vec![range],
        pivot: false,
        rules: vec![rule],
    });
    true
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
            },
        );
        let m = evaluate(&wb, 0, 0..5, 0..1, None);
        assert!((m[&(0, 0)].bar.unwrap().fraction - 0.1).abs() < 1e-9);
        assert!((m[&(4, 0)].bar.unwrap().fraction - 0.9).abs() < 1e-9);
        wb.sheets[0].conditional_formats.clear();
        add_rule(
            &mut wb,
            0,
            rng("A1:A5"),
            &CfRuleSpec::IconSet {
                set: "3Arrows".into(),
                reverse: false,
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
}
