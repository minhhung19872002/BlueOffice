//! AutoFilter drop-down of BlueSheet (round 16): the menu a filter button in a header cell
//! opens, laid out like the reference spreadsheet's — sort items, "Clear Filter From",
//! Text / Number / Date Filters submenu, a search box, the value check list headed by
//! (Select All), OK and Cancel — plus the Custom AutoFilter and Top 10 forms. Choosing
//! values sets the engine's column criterion (`setFilter`); the status bar then reads
//! "x of y records found" as the reference does.
use super::form_impl::{Event, Form, FormKind, Kind};
use super::SheetWindow;
use crate::popup::{Choice, Menu, MenuItem};
use crate::{Input, Key, Painter, TextMeasure, TextStyle};
use bo_render::{PathSeg, Rgba};
use serde_json::{json, Value};

const BG: Rgba = Rgba(0xff, 0xff, 0xff, 255);
const EDGE: Rgba = Rgba(0xc6, 0xc6, 0xc6, 255);
const HOT: Rgba = Rgba(0xe5, 0xf1, 0xfb, 255);
const HOT_EDGE: Rgba = Rgba(0xcc, 0xe4, 0xf7, 255);
const INK: Rgba = Rgba(0x26, 0x26, 0x26, 255);
const DIM: Rgba = Rgba(0xa6, 0xa6, 0xa6, 255);
const BOX_EDGE: Rgba = Rgba(0xab, 0xab, 0xab, 255);
const CHECK: Rgba = Rgba(0x33, 0x33, 0x33, 255);
const SEP: Rgba = Rgba(0xe1, 0xe1, 0xe1, 255);
const BUTTON: Rgba = Rgba(0xfd, 0xfd, 0xfd, 255);
const BUTTON_EDGE: Rgba = Rgba(0xd0, 0xd0, 0xd0, 255);
const DEFAULT_EDGE: Rgba = Rgba(0x00, 0x78, 0xd4, 255);

/// Geometry in points of display scale (device pixels = value × scale).
const W: f32 = 240.0;
const ITEM_H: f32 = 22.0;
const PAD: f32 = 3.0;
const SEP_H: f32 = 7.0;
const ROW_H: f32 = 17.0;
const LIST_H: f32 = 154.0;
const SEARCH_H: f32 = 22.0;
const BUTTON_W: f32 = 70.0;
const BUTTON_H: f32 = 22.0;
/// Left edge of item text, search box and list (after the icon column).
const TEXT_X: f32 = 30.0;

/// What the column holds, which names the submenu (Text / Number / Date Filters).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ColumnKind {
    Text,
    Number,
    Date,
}

/// One line of the value list.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Line {
    /// Displayed value (`""`: blanks) or the year / month of a date group.
    pub text: String,
    pub checked: bool,
    /// Date group: year, and month (1–12) for a month line.
    pub year: Option<i32>,
    pub month: Option<u32>,
}

/// The open drop-down.
#[derive(Debug, Clone)]
pub(super) struct FilterDrop {
    /// Absolute column of the AutoFilter range.
    pub col: u32,
    /// The header cell's text ("Clear Filter From \"…\"").
    pub header: String,
    pub kind: ColumnKind,
    /// The column has a criterion (Clear Filter is enabled, the button shows a funnel).
    pub active: bool,
    pub lines: Vec<Line>,
    pub search: String,
    /// First list row shown.
    pub top: usize,
    /// Hovered menu item (0–5) or list row.
    pub(super) hover: Option<Hot>,
    /// Top-left corner (device pixels) and scale.
    pub origin: (f32, f32),
    pub k: f32,
    /// A PivotTable field button's menu: More Sort Options… then Label / Value Filters
    /// submenus (no colour items).
    pub pivot: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Hot {
    Item(usize),
    Row(usize),
    Ok,
    Cancel,
}

/// Menu items above the search box.
const ITEMS: usize = 6;

impl FilterDrop {
    fn d(&self, v: f32) -> f32 {
        (v * self.k).round()
    }

    /// Top of menu item `i` (points from the drop-down's top), a separator after item 2.
    fn item_y(i: usize) -> f32 {
        PAD + i as f32 * ITEM_H + if i >= 3 { SEP_H } else { 0.0 }
    }

    fn search_y() -> f32 {
        Self::item_y(ITEMS) + 4.0
    }

    fn list_y() -> f32 {
        Self::search_y() + SEARCH_H + 6.0
    }

    fn buttons_y() -> f32 {
        Self::list_y() + LIST_H + 10.0
    }

    /// Height in points.
    fn height() -> f32 {
        Self::buttons_y() + BUTTON_H + 10.0
    }

    /// Device rectangle.
    pub fn rect(&self) -> [f32; 4] {
        [
            self.origin.0,
            self.origin.1,
            self.d(W),
            self.d(Self::height()),
        ]
    }

    /// Lines shown: those matching the search, after the (Select All) line.
    pub fn shown(&self) -> Vec<usize> {
        let q = self.search.to_lowercase();
        self.lines
            .iter()
            .enumerate()
            .filter(|(_, l)| q.is_empty() || l.text.to_lowercase().contains(&q))
            .map(|(i, _)| i)
            .collect()
    }

    fn rows_visible() -> usize {
        ((LIST_H - 4.0) / ROW_H).floor() as usize
    }

    fn button_x(ok: bool) -> f32 {
        let right = W - 10.0 - if ok { BUTTON_W + 6.0 } else { 0.0 };
        right - BUTTON_W
    }

    fn hit(&self, x: f32, y: f32) -> Option<Hot> {
        let (lx, ly) = ((x - self.origin.0) / self.k, (y - self.origin.1) / self.k);
        if !(0.0..W).contains(&lx) || !(0.0..Self::height()).contains(&ly) {
            return None;
        }
        for i in 0..ITEMS {
            let y0 = Self::item_y(i);
            if ly >= y0 && ly < y0 + ITEM_H {
                return Some(Hot::Item(i));
            }
        }
        let ly0 = Self::list_y() + 2.0;
        if (TEXT_X..W - 8.0).contains(&lx) && (ly0..Self::list_y() + LIST_H - 2.0).contains(&ly) {
            return Some(Hot::Row(self.top + ((ly - ly0) / ROW_H).floor() as usize));
        }
        let by = Self::buttons_y();
        if ly >= by && ly < by + BUTTON_H {
            if lx >= Self::button_x(true) && lx < Self::button_x(true) + BUTTON_W {
                return Some(Hot::Ok);
            }
            if lx >= Self::button_x(false) && lx < Self::button_x(false) + BUTTON_W {
                return Some(Hot::Cancel);
            }
        }
        None
    }

    /// Whether every shown line is checked (the (Select All) box).
    fn all_checked(&self) -> Option<bool> {
        let shown = self.shown();
        let on = shown.iter().filter(|i| self.lines[**i].checked).count();
        if on == shown.len() {
            Some(true)
        } else if on == 0 {
            Some(false)
        } else {
            None
        }
    }

    /// Toggle list row `row` (0 = Select All).
    pub fn toggle(&mut self, row: usize) {
        let shown = self.shown();
        if row == 0 {
            let to = self.all_checked() != Some(true);
            for i in shown {
                self.lines[i].checked = to;
            }
            return;
        }
        let Some(&i) = shown.get(row - 1) else {
            return;
        };
        let to = !self.lines[i].checked;
        self.lines[i].checked = to;
        // A year line carries its months; a month line updates its year.
        if let (Some(y), None) = (self.lines[i].year, self.lines[i].month) {
            for l in &mut self.lines {
                if l.year == Some(y) && l.month.is_some() {
                    l.checked = to;
                }
            }
        } else if let (Some(y), Some(_)) = (self.lines[i].year, self.lines[i].month) {
            let any = self
                .lines
                .iter()
                .any(|l| l.year == Some(y) && l.month.is_some() && l.checked);
            if let Some(yl) = self
                .lines
                .iter_mut()
                .find(|l| l.year == Some(y) && l.month.is_none())
            {
                yl.checked = any;
            }
        }
    }

    /// The criterion OK applies: `None` keeps every row.
    pub fn criterion(&self) -> Option<Value> {
        let shown = self.shown();
        let searching = !self.search.is_empty();
        let keep = |i: usize| self.lines[i].checked && (!searching || shown.contains(&i));
        if (0..self.lines.len()).all(keep) {
            return None;
        }
        if self.kind == ColumnKind::Date {
            let mut dates = Vec::new();
            let mut values = Vec::new();
            for (i, l) in self.lines.iter().enumerate() {
                match (l.year, l.month) {
                    (Some(y), None) => {
                        let months: Vec<&Line> = self
                            .lines
                            .iter()
                            .filter(|m| m.year == Some(y) && m.month.is_some())
                            .collect();
                        if months.iter().all(|m| m.checked) && keep(i) {
                            dates.push(json!({ "year": y }));
                        }
                    }
                    (Some(y), Some(m)) => {
                        let year_all = self
                            .lines
                            .iter()
                            .filter(|o| o.year == Some(y) && o.month.is_some())
                            .all(|o| o.checked);
                        if keep(i) && !year_all {
                            dates.push(json!({ "year": y, "month": m }));
                        }
                    }
                    _ => {
                        if keep(i) {
                            values.push(Value::from(l.text.clone()));
                        }
                    }
                }
            }
            return Some(json!({ "values": values, "dates": dates }));
        }
        let values: Vec<Value> = (0..self.lines.len())
            .filter(|i| keep(*i))
            .map(|i| Value::from(self.lines[i].text.clone()))
            .collect();
        Some(json!({ "values": values }))
    }

    /// Paint the drop-down; `labels` are the menu item texts, list header and button labels.
    #[allow(clippy::too_many_lines)]
    pub fn paint(&self, p: &mut dyn Painter, labels: &Labels) {
        let [x, y, w, h] = self.rect();
        let s = self.k;
        for (o, a) in [(3.0, 18u8), (2.0, 26), (1.0, 34)] {
            let o = (o * s).round();
            p.fill_rect(
                x - o + 2.0 * s,
                y - o + 3.0 * s,
                w + 2.0 * o,
                h + 2.0 * o,
                Rgba(0, 0, 0, a),
            );
        }
        p.fill_rect(x, y, w, h, EDGE);
        p.fill_rect(x + 1.0, y + 1.0, w - 2.0, h - 2.0, BG);
        let style = TextStyle {
            size: (12.0 * s).round(),
            bold: false,
        };
        let line_h = style.size * 1.33;
        for i in 0..ITEMS {
            let iy = y + self.d(Self::item_y(i));
            let ih = self.d(ITEM_H);
            let enabled = labels.enabled[i];
            if self.hover == Some(Hot::Item(i)) && enabled {
                p.fill_rect(x + self.d(PAD), iy, w - 2.0 * self.d(PAD), ih, HOT_EDGE);
                p.fill_rect(
                    x + self.d(PAD) + 1.0,
                    iy + 1.0,
                    w - 2.0 * self.d(PAD) - 2.0,
                    ih - 2.0,
                    HOT,
                );
            }
            let ink = if enabled { INK } else { DIM };
            p.text(
                &labels.items[i],
                x + self.d(TEXT_X),
                (iy + (ih - line_h) / 2.0).round(),
                style,
                ink,
            );
            if matches!(i, 2 | 4 | 5) && !(self.pivot && i == 2) {
                // Submenu arrow.
                let (ax, ay, a) = (x + w - self.d(14.0), iy + ih / 2.0, self.d(3.5));
                p.path(
                    &[
                        PathSeg::M(ax, ay - a),
                        PathSeg::L(ax + a, ay),
                        PathSeg::L(ax, ay + a),
                        PathSeg::Z,
                    ],
                    Some(ink),
                    None,
                );
            }
            self.paint_icon(
                p,
                i,
                x + self.d(8.0),
                iy + (ih - self.d(16.0)) / 2.0,
                enabled,
            );
        }
        // Separator after the sort items.
        p.fill_rect(
            x + self.d(TEXT_X),
            y + self.d(Self::item_y(3) - SEP_H / 2.0).round(),
            w - self.d(TEXT_X + 4.0),
            1.0,
            SEP,
        );
        // Search box with a magnifier.
        let (sx, sy, sw, sh) = (
            x + self.d(TEXT_X),
            y + self.d(Self::search_y()),
            self.d(W - TEXT_X - 8.0),
            self.d(SEARCH_H),
        );
        p.fill_rect(sx, sy, sw, sh, BOX_EDGE);
        p.fill_rect(sx + 1.0, sy + 1.0, sw - 2.0, sh - 2.0, BG);
        let (q, qc) = if self.search.is_empty() {
            (labels.search.as_str(), DIM)
        } else {
            (self.search.as_str(), INK)
        };
        p.push_clip(sx + 1.0, sy, sw - self.d(22.0), sh);
        p.text(
            q,
            sx + self.d(4.0),
            (sy + (sh - line_h) / 2.0).round(),
            style,
            qc,
        );
        p.pop_clip();
        let (mx, my, r) = (
            sx + sw - self.d(12.0),
            sy + sh / 2.0 - self.d(1.5),
            self.d(4.0),
        );
        let ring: Vec<PathSeg> = (0..=16)
            .map(|i| {
                let a = i as f32 / 16.0 * std::f32::consts::TAU;
                let (px, py) = (mx + r * a.cos(), my + r * a.sin());
                if i == 0 {
                    PathSeg::M(px, py)
                } else {
                    PathSeg::L(px, py)
                }
            })
            .collect();
        p.path(&ring, None, Some((INK, 1.0)));
        p.path(
            &[
                PathSeg::M(mx - r * 0.7, my + r * 0.7),
                PathSeg::L(mx - r * 1.8, my + r * 1.8),
            ],
            None,
            Some((INK, 1.5)),
        );
        // Value list.
        let (lx, ly, lw, lh) = (
            x + self.d(TEXT_X),
            y + self.d(Self::list_y()),
            self.d(W - TEXT_X - 8.0),
            self.d(LIST_H),
        );
        p.fill_rect(lx, ly, lw, lh, BOX_EDGE);
        p.fill_rect(lx + 1.0, ly + 1.0, lw - 2.0, lh - 2.0, BG);
        p.push_clip(lx + 1.0, ly + 1.0, lw - 2.0, lh - 2.0);
        let shown = self.shown();
        let rh = self.d(ROW_H);
        let cb = self.d(11.0);
        for row in self.top..(self.top + Self::rows_visible()).min(shown.len() + 1) {
            let ry = ly + self.d(2.0) + (row - self.top) as f32 * rh;
            let (text, state, indent) = if row == 0 {
                let label = if self.search.is_empty() {
                    &labels.select_all
                } else {
                    &labels.select_results
                };
                (label.clone(), self.all_checked(), 0.0)
            } else {
                let l = &self.lines[shown[row - 1]];
                let text = if l.text.is_empty() && l.year.is_none() {
                    labels.blanks.clone()
                } else {
                    l.text.clone()
                };
                let indent = match (l.year, l.month) {
                    (Some(_), Some(_)) => 28.0,
                    _ => 14.0,
                };
                (text, Some(l.checked), indent)
            };
            if self.hover == Some(Hot::Row(row)) {
                p.fill_rect(lx + 1.0, ry, lw - 2.0, rh, HOT);
            }
            let bx = lx + self.d(4.0 + indent);
            let by = (ry + (rh - cb) / 2.0).round();
            p.fill_rect(bx, by, cb, cb, CHECK);
            p.fill_rect(bx + 1.0, by + 1.0, cb - 2.0, cb - 2.0, BG);
            match state {
                Some(true) => p.path(
                    &[
                        PathSeg::M(bx + cb * 0.2, by + cb * 0.5),
                        PathSeg::L(bx + cb * 0.42, by + cb * 0.72),
                        PathSeg::L(bx + cb * 0.8, by + cb * 0.28),
                    ],
                    None,
                    Some((CHECK, (1.4 * s).max(1.5))),
                ),
                None => p.fill_rect(
                    bx + self.d(3.0),
                    by + self.d(3.0),
                    cb - 2.0 * self.d(3.0),
                    cb - 2.0 * self.d(3.0),
                    CHECK,
                ),
                Some(false) => {}
            }
            p.text(
                &text,
                bx + cb + self.d(5.0),
                (ry + (rh - line_h) / 2.0).round(),
                style,
                INK,
            );
        }
        p.pop_clip();
        // OK / Cancel.
        for ok in [true, false] {
            let bx = x + self.d(Self::button_x(ok));
            let by = y + self.d(Self::buttons_y());
            let (bw, bh) = (self.d(BUTTON_W), self.d(BUTTON_H));
            let hot = self.hover == Some(if ok { Hot::Ok } else { Hot::Cancel });
            p.fill_rect(bx, by, bw, bh, if ok { DEFAULT_EDGE } else { BUTTON_EDGE });
            p.fill_rect(
                bx + 1.0,
                by + 1.0,
                bw - 2.0,
                bh - 2.0,
                if hot { HOT } else { BUTTON },
            );
            let label = if ok { &labels.ok } else { &labels.cancel };
            let tw = label.chars().count() as f32 * style.size * 0.5;
            p.text(
                label,
                (bx + (bw - tw) / 2.0).round(),
                (by + (bh - line_h) / 2.0).round(),
                style,
                INK,
            );
        }
        // Size grip: three dots in the corner.
        for (gx, gy) in [(4.0, 4.0), (4.0, 8.0), (8.0, 4.0)] {
            p.fill_rect(
                x + w - self.d(gx) - 2.0,
                y + h - self.d(gy) - 2.0,
                (s * 1.6).round(),
                (s * 1.6).round(),
                DIM,
            );
        }
    }

    /// Sort and funnel icons of the menu items (own drawings).
    fn paint_icon(&self, p: &mut dyn Painter, i: usize, x: f32, y: f32, enabled: bool) {
        let ink = if enabled { INK } else { DIM };
        let s = self.k;
        let small = TextStyle {
            size: (7.5 * s).round(),
            bold: true,
        };
        match i {
            0 | 1 => {
                let (top, bottom) = if i == 0 { ("A", "Z") } else { ("Z", "A") };
                p.text(top, x, y - self.d(1.0), small, Rgba(0x1e, 0x6e, 0xc8, 255));
                p.text(bottom, x, y + self.d(7.0), small, ink);
                let ax = x + self.d(11.0);
                p.fill_rect(
                    ax,
                    y + self.d(1.0),
                    1.0f32.max(s.round()),
                    self.d(12.0),
                    ink,
                );
                p.path(
                    &[
                        PathSeg::M(ax - self.d(3.0), y + self.d(10.0)),
                        PathSeg::L(ax + self.d(0.5), y + self.d(14.0)),
                        PathSeg::L(ax + self.d(4.0), y + self.d(10.0)),
                        PathSeg::Z,
                    ],
                    Some(ink),
                    None,
                );
            }
            3 | 4 if !(self.pivot && i == 4) => {
                // Funnel (Clear Filter adds a red cross).
                let (fx, fy) = (x + self.d(1.0), y + self.d(2.0));
                p.path(
                    &[
                        PathSeg::M(fx, fy),
                        PathSeg::L(fx + self.d(12.0), fy),
                        PathSeg::L(fx + self.d(7.5), fy + self.d(6.0)),
                        PathSeg::L(fx + self.d(7.5), fy + self.d(12.0)),
                        PathSeg::L(fx + self.d(4.5), fy + self.d(10.0)),
                        PathSeg::L(fx + self.d(4.5), fy + self.d(6.0)),
                        PathSeg::Z,
                    ],
                    None,
                    Some((ink, 1.0)),
                );
                if i == 3 {
                    let c = if enabled {
                        Rgba(0xd1, 0x34, 0x38, 255)
                    } else {
                        DIM
                    };
                    let (cx, cy, a) = (fx + self.d(11.0), fy + self.d(10.0), self.d(3.0));
                    p.path(
                        &[PathSeg::M(cx - a, cy - a), PathSeg::L(cx + a, cy + a)],
                        None,
                        Some((c, 1.5)),
                    );
                    p.path(
                        &[PathSeg::M(cx + a, cy - a), PathSeg::L(cx - a, cy + a)],
                        None,
                        Some((c, 1.5)),
                    );
                }
            }
            _ => {}
        }
    }
}

/// Texts of the drop-down (translated once when it opens).
#[derive(Debug, Clone, Default)]
pub(super) struct Labels {
    pub items: Vec<String>,
    pub enabled: Vec<bool>,
    pub search: String,
    pub select_all: String,
    pub select_results: String,
    pub blanks: String,
    pub ok: String,
    pub cancel: String,
}

/// Custom AutoFilter operators: label key, fallback, engine operator, value wildcard pattern.
const OPS: [(&str, &str, &str, &str); 12] = [
    ("sheetFilter.opEquals", "equals", "equal", "{}"),
    (
        "sheetFilter.opNotEquals",
        "does not equal",
        "notEqual",
        "{}",
    ),
    (
        "sheetFilter.opGreater",
        "is greater than",
        "greaterThan",
        "{}",
    ),
    (
        "sheetFilter.opGreaterEq",
        "is greater than or equal to",
        "greaterThanOrEqual",
        "{}",
    ),
    ("sheetFilter.opLess", "is less than", "lessThan", "{}"),
    (
        "sheetFilter.opLessEq",
        "is less than or equal to",
        "lessThanOrEqual",
        "{}",
    ),
    ("sheetFilter.opBegins", "begins with", "equal", "{}*"),
    (
        "sheetFilter.opNotBegins",
        "does not begin with",
        "notEqual",
        "{}*",
    ),
    ("sheetFilter.opEnds", "ends with", "equal", "*{}"),
    (
        "sheetFilter.opNotEnds",
        "does not end with",
        "notEqual",
        "*{}",
    ),
    ("sheetFilter.opContains", "contains", "equal", "*{}*"),
    (
        "sheetFilter.opNotContains",
        "does not contain",
        "notEqual",
        "*{}*",
    ),
];

/// Submenu entries per column kind: choice id suffix, label key, fallback.
fn submenu(kind: ColumnKind) -> Vec<(&'static str, &'static str, &'static str)> {
    match kind {
        ColumnKind::Text => vec![
            ("op.0", "sheetFilter.equals", "Equals..."),
            ("op.1", "sheetFilter.notEquals", "Does Not Equal..."),
            ("op.6", "sheetFilter.begins", "Begins With..."),
            ("op.8", "sheetFilter.ends", "Ends With..."),
            ("op.10", "sheetFilter.contains", "Contains..."),
            ("op.11", "sheetFilter.notContains", "Does Not Contain..."),
            ("op.custom", "sheetFilter.custom", "Custom Filter..."),
        ],
        ColumnKind::Number => vec![
            ("op.0", "sheetFilter.equals", "Equals..."),
            ("op.1", "sheetFilter.notEquals", "Does Not Equal..."),
            ("op.2", "sheetFilter.greater", "Greater Than..."),
            (
                "op.3",
                "sheetFilter.greaterEq",
                "Greater Than Or Equal To...",
            ),
            ("op.4", "sheetFilter.less", "Less Than..."),
            ("op.5", "sheetFilter.lessEq", "Less Than Or Equal To..."),
            ("op.between", "sheetFilter.between", "Between..."),
            ("top10", "sheetFilter.top10", "Top 10..."),
            ("dyn.aboveAverage", "sheetFilter.above", "Above Average"),
            ("dyn.belowAverage", "sheetFilter.below", "Below Average"),
            ("op.custom", "sheetFilter.custom", "Custom Filter..."),
        ],
        ColumnKind::Date => vec![
            ("op.0", "sheetFilter.equals", "Equals..."),
            ("op.4", "sheetFilter.before", "Before..."),
            ("op.2", "sheetFilter.after", "After..."),
            ("op.between", "sheetFilter.between", "Between..."),
            ("dyn.tomorrow", "sheetFilter.tomorrow", "Tomorrow"),
            ("dyn.today", "sheetFilter.today", "Today"),
            ("dyn.yesterday", "sheetFilter.yesterday", "Yesterday"),
            ("dyn.nextWeek", "sheetFilter.nextWeek", "Next Week"),
            ("dyn.thisWeek", "sheetFilter.thisWeek", "This Week"),
            ("dyn.lastWeek", "sheetFilter.lastWeek", "Last Week"),
            ("dyn.nextMonth", "sheetFilter.nextMonth", "Next Month"),
            ("dyn.thisMonth", "sheetFilter.thisMonth", "This Month"),
            ("dyn.lastMonth", "sheetFilter.lastMonth", "Last Month"),
            ("dyn.thisYear", "sheetFilter.thisYear", "This Year"),
            ("dyn.yearToDate", "sheetFilter.ytd", "Year to Date"),
            ("op.custom", "sheetFilter.custom", "Custom Filter..."),
        ],
    }
}

const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

impl SheetWindow {
    /// The AutoFilter range `[r0, c0, r1, c1]` of the current sheet.
    pub(super) fn auto_filter(&self) -> Option<[u32; 4]> {
        let info = self.view.engine().info().ok()?;
        info.sheets.get(self.view.sheet())?.auto_filter
    }

    /// The filter range (header to last data row) of the table at (`row`, `col`) when the
    /// table shows filter buttons on a header row (§18.5.1.2 `table/autoFilter`) and the
    /// column's button is not hidden (§18.3.2.7 `hiddenButton`).
    pub(super) fn table_filter_at(&self, row: u32, col: u32) -> Option<[u32; 4]> {
        let ws = self
            .view
            .engine()
            .workbook()?
            .sheets
            .get(self.view.sheet())?;
        let t = ws.tables.iter().find(|t| {
            t.auto_filter.is_some()
                && t.header_row_count != Some(0)
                && t.range.contains(bo_model::sheet::CellRef::new(row, col))
        })?;
        let xml = t.auto_filter.as_deref().unwrap_or_default();
        let id = format!("colId=\"{}\"", col - t.range.first.col);
        if xml
            .split("<filterColumn")
            .skip(1)
            .find(|f| f.contains(&id))
            .is_some_and(|f| f.contains("hiddenButton=\"1\""))
        {
            return None;
        }
        let totals = u32::from(t.totals_row_count.unwrap_or(0) > 0);
        Some([
            t.range.first.row,
            t.range.first.col,
            t.range.last.row.saturating_sub(totals),
            t.range.last.col,
        ])
    }

    /// The filter button under (`x`, `y`): the column (and the table range when it is a
    /// table's header button), when the point is on the button part (right end) of an
    /// AutoFilter or table header cell.
    pub(super) fn filter_button_at(&self, x: f32, y: f32) -> Option<(u32, Option<[u32; 4]>)> {
        let (r, c) = self.view.cell_under(x, y)?;
        let table = match self.auto_filter() {
            Some([r0, c0, _, c1]) if r == r0 && c >= c0 && c <= c1 => None,
            _ => Some(self.table_filter_at(r, c).filter(|t| t[0] == r)?),
        };
        let [_, top, right, bottom] = self.view.cell_box(r, c)?;
        let size = (bottom - top - 3.0).clamp(self.px(8.0), self.px(16.0));
        (x >= right - size - 2.0).then_some((c, table))
    }

    /// Open the drop-down of AutoFilter column `col` (of the table filter `table` when
    /// given).
    pub(super) fn open_filter(&mut self, col: u32, table: Option<[u32; 4]>) -> bool {
        let Some([r0, _, _, _]) = table.or_else(|| self.auto_filter()) else {
            return false;
        };
        self.filter_table = table;
        let Some(v) = self
            .view
            .query(json!({ "cmd": "filterValues", "col": col, "row": table.map(|t| t[0]) }))
        else {
            return false;
        };
        let header = self
            .view
            .engine()
            .cell(self.view.sheet(), r0, col)
            .ok()
            .map(|c| c.display)
            .unwrap_or_default();
        let active = v.get("kind").and_then(Value::as_str) != Some("none");
        let values: Vec<(String, bool)> = v
            .get("values")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .map(|e| {
                        (
                            e.get("text")
                                .and_then(Value::as_str)
                                .unwrap_or("")
                                .to_owned(),
                            e.get("checked").and_then(Value::as_bool).unwrap_or(true),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();
        let dates = v
            .get("dates")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let numeric = |s: &str| s.replace([',', ' '], "").parse::<f64>().is_ok();
        let kind = if !dates.is_empty() {
            ColumnKind::Date
        } else if values.iter().any(|(t, _)| !t.is_empty())
            && values
                .iter()
                .filter(|(t, _)| !t.is_empty())
                .all(|(t, _)| numeric(t))
        {
            ColumnKind::Number
        } else {
            ColumnKind::Text
        };
        let mut lines = Vec::new();
        if kind == ColumnKind::Date {
            for d in &dates {
                let year = d.get("year").and_then(Value::as_i64).unwrap_or(0) as i32;
                lines.push(Line {
                    text: year.to_string(),
                    checked: d.get("checked").and_then(Value::as_bool).unwrap_or(true),
                    year: Some(year),
                    month: None,
                });
                for m in d
                    .get("months")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                {
                    let month = m.get("month").and_then(Value::as_u64).unwrap_or(1) as u32;
                    let name = MONTHS[(month.clamp(1, 12) - 1) as usize];
                    lines.push(Line {
                        text: self.t(&format!("sheetFilter.month{month}"), name),
                        checked: m.get("checked").and_then(Value::as_bool).unwrap_or(true),
                        year: Some(year),
                        month: Some(month),
                    });
                }
            }
            // Blank cells stay a value line.
            if let Some((_, on)) = values.iter().find(|(t, _)| t.is_empty()) {
                lines.push(Line {
                    text: String::new(),
                    checked: *on,
                    year: None,
                    month: None,
                });
            }
        } else {
            lines = values
                .into_iter()
                .map(|(text, checked)| Line {
                    text,
                    checked,
                    year: None,
                    month: None,
                })
                .collect();
        }
        // Under the header cell, its left edge on the cell's left edge (as the reference),
        // kept in the window.
        let k = self.scale;
        let cell = self.view.cell_box(r0, col).unwrap_or([0.0; 4]);
        let w = (W * k).round();
        let h = (FilterDrop::height() * k).round();
        let x = cell[0]
            .max(self.px(2.0))
            .min(self.size.0 - w - self.px(2.0))
            .max(0.0);
        let mut y = cell[3] + 1.0;
        if y + h > self.size.1 {
            y = (self.size.1 - h).max(0.0);
        }
        self.filter = Some(FilterDrop {
            col,
            header,
            kind,
            active,
            lines,
            search: String::new(),
            top: 0,
            hover: None,
            origin: (x.round(), y.round()),
            k,
            pivot: false,
        });
        self.filter_pivot = None;
        self.filter_labels = self.filter_labels_now();
        true
    }

    fn filter_labels_now(&self) -> Labels {
        let Some(f) = &self.filter else {
            return Labels::default();
        };
        let sub = match f.kind {
            ColumnKind::Text => self.t("sheetFilter.textFilters", "Text Filters"),
            ColumnKind::Number => self.t("sheetFilter.numberFilters", "Number Filters"),
            ColumnKind::Date => self.t("sheetFilter.dateFilters", "Date Filters"),
        };
        let (asc, desc) = match f.kind {
            ColumnKind::Text => (
                self.t("sheetFilter.sortAZ", "Sort A to Z"),
                self.t("sheetFilter.sortZA", "Sort Z to A"),
            ),
            ColumnKind::Number => (
                self.t("sheetFilter.sortSmall", "Sort Smallest to Largest"),
                self.t("sheetFilter.sortLarge", "Sort Largest to Smallest"),
            ),
            ColumnKind::Date => (
                self.t("sheetFilter.sortOld", "Sort Oldest to Newest"),
                self.t("sheetFilter.sortNew", "Sort Newest to Oldest"),
            ),
        };
        Labels {
            items: vec![
                asc,
                desc,
                self.t("sheetFilter.sortColor", "Sort by Color"),
                self.t("sheetFilter.clearFrom", "Clear Filter From \"{0}\"")
                    .replace("{0}", &f.header),
                self.t("sheetFilter.filterColor", "Filter by Color"),
                sub,
            ],
            enabled: vec![true, true, false, f.active, false, true],
            search: self.t("sheetFilter.search", "Search"),
            select_all: self.t("sheetFilter.selectAll", "(Select All)"),
            select_results: self.t("sheetFilter.selectResults", "(Select All Search Results)"),
            blanks: self.t("sheetFilter.blanks", "(Blanks)"),
            ok: self.t("common.ok", "OK"),
            cancel: self.t("common.cancel", "Cancel"),
        }
    }

    /// Set the criterion of AutoFilter column `col` and report "x of y records found".
    #[allow(clippy::needless_pass_by_value)]
    pub(super) fn set_filter(&mut self, col: u32, filter: Option<Value>) -> bool {
        let row = self.filter_table.map(|t| t[0]);
        let Some(out) = self
            .view
            .query(json!({ "cmd": "setFilter", "col": col, "filter": filter, "row": row }))
        else {
            return false;
        };
        self.view.changed();
        let hidden = out.get("hidden").and_then(Value::as_u64).unwrap_or(0);
        let total = out.get("total").and_then(Value::as_u64).unwrap_or(0);
        self.status = self
            .t("sheetFilter.found", "{0} of {1} records found")
            .replace("{0}", &total.saturating_sub(hidden).to_string())
            .replace("{1}", &total.to_string());
        self.filter_status = true;
        true
    }

    fn filter_sort(&mut self, descending: bool, col: u32) -> bool {
        let Some([r0, c0, r1, c1]) = self.filter_table.or_else(|| self.auto_filter()) else {
            return false;
        };
        self.view.run(json!({
            "cmd": "sort", "range": { "r0": r0, "c0": c0, "r1": r1, "c1": c1 },
            "hasHeader": true, "keys": [{ "col": col, "descending": descending }],
        }))
    }

    /// Input while the drop-down is open: `Some(redraw)` when it took the input.
    pub(super) fn filter_input(&mut self, input: &Input, _text: &dyn TextMeasure) -> Option<bool> {
        let f = self.filter.as_mut()?;
        Some(match *input {
            Input::Move { x, y } => {
                let h = f.hit(x, y);
                let changed = f.hover != h;
                f.hover = h;
                changed
            }
            Input::Press { x, y, .. } => match f.hit(x, y) {
                None | Some(Hot::Cancel) => {
                    self.filter = None;
                    true
                }
                Some(Hot::Item(i)) => self.filter_item(i),
                Some(Hot::Row(r)) => {
                    f.toggle(r);
                    true
                }
                Some(Hot::Ok) => self.filter_ok(),
            },
            Input::Wheel(dy) => {
                let n = f.shown().len() + 1;
                let max = n.saturating_sub(FilterDrop::rows_visible());
                let step = if dy > 0.0 { 3i64 } else { -3 };
                let top = (f.top as i64 + step).clamp(0, max as i64) as usize;
                let changed = top != f.top;
                f.top = top;
                changed
            }
            Input::Text(ref s) => {
                f.search.push_str(s);
                f.top = 0;
                true
            }
            Input::Key(Key::Backspace, _) => {
                f.search.pop();
                f.top = 0;
                true
            }
            Input::Key(Key::Escape, _) => {
                self.filter = None;
                true
            }
            Input::Key(Key::Enter, _) => self.filter_ok(),
            Input::Key(..) | Input::Blink => true,
            Input::Drag { .. } | Input::Release { .. } | Input::Context { .. } => false,
            _ => return None,
        })
    }

    fn filter_ok(&mut self) -> bool {
        if self.filter_pivot.is_some() {
            return self.pivot_filter_ok();
        }
        let Some(f) = self.filter.take() else {
            return false;
        };
        let crit = f.criterion();
        self.set_filter(f.col, crit)
    }

    pub(super) fn filter_item(&mut self, i: usize) -> bool {
        if self.filter_pivot.is_some() {
            return self.pivot_filter_item(i);
        }
        let Some(f) = self.filter.clone() else {
            return false;
        };
        if !self.filter_labels.enabled.get(i).copied().unwrap_or(false) {
            return true;
        }
        match i {
            0 | 1 => {
                self.filter = None;
                self.filter_sort(i == 1, f.col)
            }
            3 => {
                self.filter = None;
                self.set_filter(f.col, None)
            }
            5 => {
                let items = submenu(f.kind)
                    .iter()
                    .map(|(id, key, fallback)| MenuItem {
                        label: self.t(key, fallback),
                        choice: Choice::Command(format!("filterop.{id}")),
                        checked: false,
                        dim: false,
                    })
                    .collect();
                let [x, y, w, _] = f.rect();
                let iy = y + (FilterDrop::item_y(5) * f.k).round();
                // To the right of the drop-down, its first item level with the hovered one.
                let anchor = [x + w - 2.0, iy - (30.0 * f.k).round(), 0.0, 0.0];
                self.menu = Some(Menu::open(items, anchor, self.size, self.scale));
                true
            }
            _ => true,
        }
    }

    /// A submenu choice (`filterop.<id>`): dynamic filters apply at once, others open the
    /// Custom AutoFilter or Top 10 form.
    pub(super) fn filter_choice(&mut self, id: &str) -> bool {
        let Some(f) = self.filter.take() else {
            return false;
        };
        if let Some(kind) = id.strip_prefix("dyn.") {
            return self.set_filter(f.col, Some(json!({ "dynamic": kind })));
        }
        if id == "top10" {
            return self.open_top10(f.col);
        }
        let (first, second) = match id {
            "op.between" => (3, Some(5)),
            "op.custom" => (0, None),
            op => (
                op.strip_prefix("op.")
                    .and_then(|n| n.parse::<usize>().ok())
                    .unwrap_or(0),
                None,
            ),
        };
        self.open_custom_filter(f.col, &f.header, first, second)
    }

    fn open_custom_filter(
        &mut self,
        col: u32,
        header: &str,
        op1: usize,
        op2: Option<usize>,
    ) -> bool {
        let t = |k: &str, d: &str| self.t(k, d);
        let mut form = Form::new(
            &t("sheetFilter.customTitle", "Custom AutoFilter"),
            (520.0, 300.0),
            vec![],
        );
        let ops: Vec<String> = OPS.iter().map(|(k, d, ..)| t(k, d)).collect();
        let mut blank_ops = vec![String::new()];
        blank_ops.extend(ops.iter().cloned());
        form.add(
            "show",
            None,
            [20.0, 54.0, 0.0, 0.0],
            Kind::Label(t("sheetFilter.showRows", "Show rows where:")),
        );
        form.add(
            "field",
            None,
            [32.0, 78.0, 0.0, 0.0],
            Kind::Label(header.to_owned()),
        );
        form.add(
            "op1",
            None,
            [32.0, 102.0, 200.0, 25.0],
            Kind::Combo {
                items: ops,
                sel: op1,
            },
        );
        form.add(
            "v1",
            None,
            [244.0, 102.0, 256.0, 25.0],
            Kind::Edit {
                value: Vec::new(),
                caret: 0,
            },
        );
        form.add(
            "and",
            None,
            [60.0, 138.0, 80.0, 20.0],
            Kind::Check {
                label: t("sheetFilter.and", "And"),
                on: true,
            },
        );
        form.add(
            "or",
            None,
            [150.0, 138.0, 80.0, 20.0],
            Kind::Check {
                label: t("sheetFilter.or", "Or"),
                on: false,
            },
        );
        form.add(
            "op2",
            None,
            [32.0, 170.0, 200.0, 25.0],
            Kind::Combo {
                items: blank_ops,
                sel: op2.map_or(0, |o| o + 1),
            },
        );
        form.add(
            "v2",
            None,
            [244.0, 170.0, 256.0, 25.0],
            Kind::Edit {
                value: Vec::new(),
                caret: 0,
            },
        );
        form.add(
            "hint",
            None,
            [20.0, 206.0, 0.0, 0.0],
            Kind::Note(t(
                "sheetFilter.hint",
                "Use ? to represent any single character. Use * to represent any series of characters.",
            )),
        );
        form.ok_cancel(&t("common.ok", "OK"), &t("common.cancel", "Cancel"));
        form.focus("v1");
        self.form = Some(form);
        self.form_kind = FormKind::CustomFilter;
        self.filter_col = col;
        true
    }

    fn open_top10(&mut self, col: u32) -> bool {
        let t = |k: &str, d: &str| self.t(k, d);
        let mut form = Form::new(
            &t("sheetFilter.top10Title", "Top 10 AutoFilter"),
            (440.0, 170.0),
            vec![],
        );
        form.add(
            "showLabel",
            None,
            [20.0, 54.0, 0.0, 0.0],
            Kind::Label(t("sheetFilter.show", "Show")),
        );
        form.add(
            "dir",
            None,
            [20.0, 76.0, 120.0, 25.0],
            Kind::Combo {
                items: vec![
                    t("sheetFilter.top", "Top"),
                    t("sheetFilter.bottom", "Bottom"),
                ],
                sel: 0,
            },
        );
        form.add(
            "n",
            None,
            [152.0, 76.0, 80.0, 25.0],
            Kind::Spin {
                value: 10,
                min: 1,
                max: 500,
            },
        );
        form.add(
            "unit",
            None,
            [244.0, 76.0, 176.0, 25.0],
            Kind::Combo {
                items: vec![
                    t("sheetFilter.items", "Items"),
                    t("sheetFilter.percent", "Percent"),
                ],
                sel: 0,
            },
        );
        form.ok_cancel(&t("common.ok", "OK"), &t("common.cancel", "Cancel"));
        self.form = Some(form);
        self.form_kind = FormKind::Top10;
        self.filter_col = col;
        true
    }

    /// Events of the Custom AutoFilter and Top 10 forms.
    pub(super) fn filter_form_event(&mut self, ev: &Event) -> bool {
        let Some(form) = self.form.as_mut() else {
            return false;
        };
        match ev {
            Event::Changed("and") => {
                let on = form.on("and");
                form.set_on("or", !on);
            }
            Event::Changed("or") => {
                let on = form.on("or");
                form.set_on("and", !on);
            }
            Event::Cancel => {
                self.form = None;
            }
            Event::Ok => {
                let col = self.filter_col;
                let filter = if self.form_kind == FormKind::Top10 {
                    json!({ "top10": {
                        "top": form.sel("dir") != Some(1),
                        "percent": form.sel("unit") == Some(1),
                        "val": f64::from(form.num("n")),
                    } })
                } else {
                    let cond = |op: usize, v: String| {
                        let (_, _, operator, pattern) = OPS[op.min(OPS.len() - 1)];
                        json!({ "operator": operator, "value": pattern.replace("{}", &v) })
                    };
                    let mut conditions = vec![cond(form.sel("op1").unwrap_or(0), form.text("v1"))];
                    if let Some(op2) = form.sel("op2").filter(|o| *o > 0) {
                        conditions.push(cond(op2 - 1, form.text("v2")));
                    }
                    json!({ "conditions": conditions, "and": form.on("and") })
                };
                self.form = None;
                return self.set_filter(col, Some(filter));
            }
            _ => {}
        }
        true
    }

    /// Paint the open drop-down.
    pub(super) fn paint_filter(&self, p: &mut dyn Painter) {
        if let Some(f) = &self.filter {
            f.paint(p, &self.filter_labels);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn drop(lines: &[(&str, bool)]) -> FilterDrop {
        FilterDrop {
            col: 1,
            header: "Region".into(),
            kind: ColumnKind::Text,
            active: false,
            lines: lines
                .iter()
                .map(|(t, c)| Line {
                    text: (*t).into(),
                    checked: *c,
                    year: None,
                    month: None,
                })
                .collect(),
            search: String::new(),
            top: 0,
            hover: None,
            origin: (0.0, 0.0),
            k: 1.0,
            pivot: false,
        }
    }

    #[test]
    fn select_all_and_search_build_the_criterion() {
        let mut d = drop(&[("Central", true), ("North", true), ("South", true)]);
        assert_eq!(d.criterion(), None);
        d.toggle(0);
        assert_eq!(d.all_checked(), Some(false));
        d.toggle(2);
        assert_eq!(d.all_checked(), None);
        assert_eq!(d.criterion(), Some(json!({ "values": ["North"] })));
        // Searching keeps only the matches that are checked.
        let mut d = drop(&[("Central", true), ("North", true), ("South", true)]);
        d.search = "th".into();
        assert_eq!(d.shown(), vec![1, 2]);
        assert_eq!(d.criterion(), Some(json!({ "values": ["North", "South"] })));
    }

    #[test]
    fn rows_and_buttons_are_hit_where_they_are_drawn() {
        let d = drop(&[("A", true), ("B", true)]);
        let y = FilterDrop::list_y() + 2.0 + ROW_H * 1.5;
        assert_eq!(d.hit(TEXT_X + 10.0, y), Some(Hot::Row(1)));
        assert_eq!(d.hit(10.0, FilterDrop::item_y(3) + 5.0), Some(Hot::Item(3)));
        let by = FilterDrop::buttons_y() + 5.0;
        assert_eq!(d.hit(FilterDrop::button_x(true) + 5.0, by), Some(Hot::Ok));
        assert_eq!(
            d.hit(FilterDrop::button_x(false) + 5.0, by),
            Some(Hot::Cancel)
        );
        assert_eq!(d.hit(W + 5.0, by), None);
    }

    #[test]
    fn date_lines_choose_years_and_months() {
        let mut d = drop(&[]);
        d.kind = ColumnKind::Date;
        for (text, year, month) in [
            ("2025", 2025, None),
            ("January", 2025, Some(1)),
            ("February", 2025, Some(2)),
        ] {
            d.lines.push(Line {
                text: text.into(),
                checked: true,
                year: Some(year),
                month,
            });
        }
        assert_eq!(d.criterion(), None);
        d.toggle(3); // February off
        assert_eq!(
            d.criterion(),
            Some(json!({ "values": [], "dates": [{ "year": 2025, "month": 1 }] }))
        );
        d.toggle(1); // the year off clears its months
        assert!(d.lines.iter().all(|l| !l.checked));
    }
}
