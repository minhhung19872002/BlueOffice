//! Conditional Formatting menu of BlueSheet (round 27), laid out as the reference's (measured at
//! 125 %: 274 × 401 px, five 57 px entries with large pictures and bold labels, then New Rule…,
//! Clear Rules ▸, Manage Rules… at 35 px): cascading submenus Highlight Cells Rules, Top/Bottom
//! Rules (lists), Data Bars, Color Scales, Icon Sets (tile galleries), Clear Rules; the rule
//! dialogs (Greater Than…, Top 10 Items…, …), New Formatting Rule and the Conditional Formatting
//! Rules Manager. Pictures are BlueOffice's own drawings. Engine: `addCf`, `listCf`, `editCf`,
//! `clearCf`.

use super::form_impl::{Event, Form, FormKind, Kind};
use super::SheetWindow;
use crate::{Input, Key, Painter, TextStyle};
use bo_render::{PathSeg, Rgba};
use serde_json::{json, Value};

const BG: Rgba = Rgba(0xff, 0xff, 0xff, 255);
const EDGE: Rgba = Rgba(0xc6, 0xc6, 0xc6, 255);
const HOT: Rgba = Rgba(0xe5, 0xf1, 0xfb, 255);
const HOT_EDGE: Rgba = Rgba(0xcc, 0xe4, 0xf7, 255);
const INK: Rgba = Rgba(0x26, 0x26, 0x26, 255);
const SEP: Rgba = Rgba(0xe1, 0xe1, 0xe1, 255);
const HEAD_BG: Rgba = Rgba(0xeb, 0xeb, 0xeb, 255);
const HEAD_INK: Rgba = Rgba(0x5e, 0x5e, 0x5e, 255);
const GRID: Rgba = Rgba(0x9b, 0x9b, 0x9b, 255);
const ACCENT: Rgba = Rgba(0xf3, 0xd5, 0x8b, 255);

/// Formats offered by the rule dialogs: (fill, font colour) as the reference's presets.
pub(super) const PRESETS: [(&str, Option<&str>, Option<&str>); 5] = [
    (
        "Light Red Fill with Dark Red Text",
        Some("FFC7CE"),
        Some("9C0006"),
    ),
    (
        "Yellow Fill with Dark Yellow Text",
        Some("FFEB9C"),
        Some("9C5700"),
    ),
    (
        "Green Fill with Dark Green Text",
        Some("C6EFCE"),
        Some("006100"),
    ),
    ("Light Red Fill", Some("FFC7CE"), None),
    ("Red Text", None, Some("9C0006")),
];

/// Data bar colours (gradient and solid rows).
const BARS: [&str; 6] = ["638EC6", "63C384", "FF555A", "FFB628", "008AEF", "D6007B"];

/// Colour scales in the reference's order: (min, mid, max).
const SCALES: [(&str, Option<&str>, &str); 12] = [
    ("F8696B", Some("FFEB84"), "63BE7B"),
    ("63BE7B", Some("FFEB84"), "F8696B"),
    ("F8696B", Some("FCFCFF"), "63BE7B"),
    ("63BE7B", Some("FCFCFF"), "F8696B"),
    ("F8696B", Some("FCFCFF"), "5A8AC6"),
    ("5A8AC6", Some("FCFCFF"), "F8696B"),
    ("FCFCFF", None, "F8696B"),
    ("F8696B", None, "FCFCFF"),
    ("FCFCFF", None, "63BE7B"),
    ("63BE7B", None, "FCFCFF"),
    ("FFEF9C", None, "63BE7B"),
    ("63BE7B", None, "FFEF9C"),
];

/// Icon sets by section (§18.18.42 `ST_IconSetType`).
const ICON_SECTIONS: [(&str, &str, &[&str]); 4] = [
    (
        "sheetCf.directional",
        "Directional",
        &[
            "3Arrows",
            "3ArrowsGray",
            "4Arrows",
            "4ArrowsGray",
            "5Arrows",
            "5ArrowsGray",
            "3Triangles",
        ],
    ),
    (
        "sheetCf.shapes",
        "Shapes",
        &[
            "3TrafficLights1",
            "3TrafficLights2",
            "3Signs",
            "4TrafficLights",
            "4RedToBlack",
        ],
    ),
    (
        "sheetCf.indicators",
        "Indicators",
        &["3Symbols", "3Symbols2", "3Flags"],
    ),
    (
        "sheetCf.ratings",
        "Ratings",
        &["3Stars", "4Rating", "5Quarters", "5Rating", "5Boxes"],
    ),
];

/// Entries of the main menu: (id, label key, fallback, big, has a submenu).
const MAIN: [(&str, &str, &str, bool, bool); 8] = [
    (
        "highlight",
        "sheetCf.highlight",
        "Highlight Cells Rules",
        true,
        true,
    ),
    (
        "topBottom",
        "sheetCf.topBottom",
        "Top/Bottom Rules",
        true,
        true,
    ),
    ("dataBars", "sheetCf.dataBars", "Data Bars", true, true),
    (
        "colorScales",
        "sheetCf.colorScales",
        "Color Scales",
        true,
        true,
    ),
    ("iconSets", "sheetCf.iconSets", "Icon Sets", true, true),
    ("newRule", "sheetCf.newRule", "New Rule...", false, false),
    ("clear", "sheetCf.clearRules", "Clear Rules", false, true),
    ("manage", "sheetCf.manage", "Manage Rules...", false, false),
];

/// Items of the list submenus: (choice, label key, fallback).
fn list_items(sub: &str) -> Vec<(&'static str, &'static str, &'static str)> {
    match sub {
        "highlight" => vec![
            ("cfd.greaterThan", "sheetCf.greaterThan", "Greater Than..."),
            ("cfd.lessThan", "sheetCf.lessThan", "Less Than..."),
            ("cfd.between", "sheetCf.between", "Between..."),
            ("cfd.equal", "sheetCf.equalTo", "Equal To..."),
            ("cfd.text", "sheetCf.textContains", "Text that Contains..."),
            ("cfd.date", "sheetCf.dateOccurring", "A Date Occurring..."),
            (
                "cfd.duplicate",
                "sheetCf.duplicateValues",
                "Duplicate Values...",
            ),
            ("cfd.newRule", "sheetCf.moreRules", "More Rules..."),
        ],
        "topBottom" => vec![
            ("cfd.top", "sheetCf.top10", "Top 10 Items..."),
            ("cfd.topPct", "sheetCf.top10Pct", "Top 10%..."),
            ("cfd.bottom", "sheetCf.bottom10", "Bottom 10 Items..."),
            ("cfd.bottomPct", "sheetCf.bottom10Pct", "Bottom 10%..."),
            ("cfd.above", "sheetCf.aboveAverage", "Above Average..."),
            ("cfd.below", "sheetCf.belowAverage", "Below Average..."),
            ("cfd.newRule", "sheetCf.moreRules", "More Rules..."),
        ],
        "clear" => vec![
            (
                "cf.clearSel",
                "sheetCf.clearSelection",
                "Clear Rules from Selected Cells",
            ),
            (
                "cf.clearSheet",
                "sheetCf.clearSheet",
                "Clear Rules from Entire Sheet",
            ),
            (
                "cf.clearTable",
                "sheetCf.clearTable",
                "Clear Rules from This Table",
            ),
            (
                "cf.clearPivot",
                "sheetCf.clearPivot",
                "Clear Rules from This PivotTable",
            ),
        ],
        _ => Vec::new(),
    }
}

/// A tile of a gallery submenu.
#[derive(Debug, Clone, PartialEq)]
enum Tile {
    Bar { color: &'static str, gradient: bool },
    Scale(usize),
    Icons(&'static str),
}

/// Gallery submenus: sections (caption key, fallback, tiles, columns).
fn gallery(sub: &str) -> Vec<(&'static str, &'static str, Vec<Tile>, usize)> {
    match sub {
        "dataBars" => vec![
            (
                "sheetCf.gradientFill",
                "Gradient Fill",
                BARS.iter()
                    .map(|c| Tile::Bar {
                        color: *c,
                        gradient: true,
                    })
                    .collect(),
                3,
            ),
            (
                "sheetCf.solidFill",
                "Solid Fill",
                BARS.iter()
                    .map(|c| Tile::Bar {
                        color: *c,
                        gradient: false,
                    })
                    .collect(),
                3,
            ),
        ],
        "colorScales" => vec![("", "", (0..12).map(Tile::Scale).collect(), 4)],
        "iconSets" => ICON_SECTIONS
            .iter()
            .map(|(k, d, sets)| (*k, *d, sets.iter().map(|s| Tile::Icons(*s)).collect(), 2))
            .collect(),
        _ => Vec::new(),
    }
}

/// The open Conditional Formatting menu (reference pixels × `k`).
#[derive(Debug, Clone)]
pub(super) struct CfMenu {
    /// Top-left of the main menu (device px).
    origin: (f32, f32),
    /// Device px per reference px (125 % = 1).
    k: f32,
    hover: Option<usize>,
    /// Main entry whose submenu is open.
    open: Option<usize>,
    /// Hovered submenu entry: list item or (section, tile) flattened; `usize::MAX` = More Rules.
    sub_hover: Option<usize>,
    /// Window size (device px), to keep the submenu inside.
    view: (f32, f32),
}

const MAIN_W: f32 = 274.0;
const BIG_H: f32 = 57.0;
const SMALL_H: f32 = 35.0;
const SEP_H: f32 = 3.0;
const LIST_H: f32 = 40.0;
const SUB_LIST_W: f32 = 300.0;
const HEAD_H: f32 = 26.0;
const MORE_H: f32 = 32.0;

impl CfMenu {
    fn d(&self, v: f32) -> f32 {
        (v * self.k).round()
    }

    /// Top of main entry `i` and its height (reference px from the menu's top).
    fn row(i: usize) -> (f32, f32) {
        let mut y = 2.0;
        for (j, e) in MAIN.iter().enumerate() {
            if j == 2 || j == 5 {
                y += SEP_H;
            }
            let h = if e.3 { BIG_H } else { SMALL_H };
            if j == i {
                return (y, h);
            }
            y += h;
        }
        (y, SMALL_H)
    }

    fn main_h() -> f32 {
        let (y, h) = Self::row(MAIN.len() - 1);
        y + h + 2.0
    }

    /// Main menu rectangle (device px).
    fn main_rect(&self) -> [f32; 4] {
        [
            self.origin.0,
            self.origin.1,
            self.d(MAIN_W),
            self.d(Self::main_h()),
        ]
    }

    /// Size of the open submenu (reference px).
    fn sub_size(&self) -> (f32, f32) {
        let Some(i) = self.open else {
            return (0.0, 0.0);
        };
        let id = MAIN[i].0;
        let items = list_items(id);
        if !items.is_empty() {
            let more = items.iter().filter(|i| i.0 == "cfd.newRule").count() as f32;
            return (SUB_LIST_W, 4.0 + items.len() as f32 * LIST_H + more * SEP_H);
        }
        let (tw, th) = Self::tile_size(id);
        let mut h = 4.0;
        let mut cols = 1;
        for (cap, _, tiles, c) in gallery(id) {
            cols = c;
            if !cap.is_empty() {
                h += HEAD_H;
            }
            h += tiles.len().div_ceil(c) as f32 * (th + 6.0) + 4.0;
        }
        (8.0 + cols as f32 * (tw + 6.0), h + SEP_H + MORE_H + 2.0)
    }

    fn tile_size(id: &str) -> (f32, f32) {
        match id {
            "iconSets" => (110.0, 26.0),
            _ => (44.0, 44.0),
        }
    }

    /// Submenu rectangle (device px).
    fn sub_rect(&self) -> Option<[f32; 4]> {
        let i = self.open?;
        let (w, h) = self.sub_size();
        let (w, h) = (self.d(w), self.d(h));
        let [mx, my, mw, _] = self.main_rect();
        let (top, _) = Self::row(i);
        let mut x = mx + mw - 2.0;
        if x + w > self.view.0 {
            x = (mx - w + 2.0).max(0.0);
        }
        let y = (my + self.d(top) - 2.0).min((self.view.1 - h).max(0.0));
        Some([x, y, w, h])
    }

    /// Rectangles of the open submenu's entries (device px) with their choices.
    fn sub_entries(&self) -> Vec<([f32; 4], String)> {
        let (Some(i), Some([x, y, w, _])) = (self.open, self.sub_rect()) else {
            return Vec::new();
        };
        let id = MAIN[i].0;
        let mut out = Vec::new();
        let items = list_items(id);
        if !items.is_empty() {
            let mut ty = y + self.d(2.0);
            for (choice, _, _) in items {
                if choice == "cfd.newRule" {
                    ty += self.d(SEP_H);
                }
                out.push(([x + 2.0, ty, w - 4.0, self.d(LIST_H)], choice.to_owned()));
                ty += self.d(LIST_H);
            }
            return out;
        }
        let (tw, th) = Self::tile_size(id);
        let mut ty = y + self.d(4.0);
        for (cap, _, tiles, cols) in gallery(id) {
            if !cap.is_empty() {
                ty += self.d(HEAD_H);
            }
            for (k, t) in tiles.iter().enumerate() {
                let (r, c) = (k / cols, k % cols);
                let rect = [
                    x + self.d(6.0 + c as f32 * (tw + 6.0)),
                    ty + self.d(r as f32 * (th + 6.0)),
                    self.d(tw),
                    self.d(th),
                ];
                let choice = match t {
                    Tile::Bar { color, gradient } => {
                        format!("cfb.{}.{color}", if *gradient { "g" } else { "s" })
                    }
                    Tile::Scale(s) => format!("cfs.{s}"),
                    Tile::Icons(set) => format!("cfi.{set}"),
                };
                out.push((rect, choice));
            }
            ty += self.d(tiles.len().div_ceil(cols) as f32 * (th + 6.0) + 4.0);
        }
        ty += self.d(SEP_H);
        out.push((
            [x + 2.0, ty, w - 4.0, self.d(MORE_H)],
            "cfd.newRule".to_owned(),
        ));
        out
    }

    fn main_at(&self, x: f32, y: f32) -> Option<usize> {
        let [mx, my, mw, mh] = self.main_rect();
        if x < mx || x >= mx + mw || y < my || y >= my + mh {
            return None;
        }
        (0..MAIN.len()).find(|&i| {
            let (t, h) = Self::row(i);
            y >= my + self.d(t) && y < my + self.d(t + h)
        })
    }

    fn sub_at(&self, x: f32, y: f32) -> Option<usize> {
        self.sub_entries()
            .iter()
            .position(|(r, _)| x >= r[0] && x < r[0] + r[2] && y >= r[1] && y < r[1] + r[3])
    }

    fn inside(&self, x: f32, y: f32) -> bool {
        let [mx, my, mw, mh] = self.main_rect();
        let in_main = x >= mx && x < mx + mw && y >= my && y < my + mh;
        let in_sub = self
            .sub_rect()
            .is_some_and(|[sx, sy, sw, sh]| x >= sx && x < sx + sw && y >= sy && y < sy + sh);
        in_main || in_sub
    }
}

impl SheetWindow {
    /// Open the Conditional Formatting menu under its ribbon button.
    pub(super) fn open_cf_menu(&mut self, anchor: [f32; 4]) -> bool {
        let k = self.scale / 1.25;
        let w = (MAIN_W * k).round();
        self.cf_menu = Some(CfMenu {
            origin: (
                anchor[0].min(self.size.0 - w - 2.0).max(0.0).round(),
                (anchor[1] + anchor[3] + 1.0).round(),
            ),
            k,
            hover: None,
            open: None,
            sub_hover: None,
            view: self.size,
        });
        true
    }

    /// Input while the menu is open: `Some(redraw)` when it took the input.
    pub(super) fn cf_menu_input(
        &mut self,
        input: &Input,
        text: &dyn crate::TextMeasure,
    ) -> Option<bool> {
        let m = self.cf_menu.as_mut()?;
        Some(match *input {
            Input::Move { x, y } => {
                let before = (m.hover, m.open, m.sub_hover);
                if let Some(s) = m.sub_at(x, y) {
                    m.sub_hover = Some(s);
                } else if let Some(i) = m.main_at(x, y) {
                    m.hover = Some(i);
                    m.sub_hover = None;
                    // Pointing at an entry with a submenu opens it (others close it).
                    m.open = MAIN[i].4.then_some(i);
                } else {
                    m.sub_hover = None;
                }
                before != (m.hover, m.open, m.sub_hover)
            }
            Input::Press { x, y, .. } => {
                if !m.inside(x, y) {
                    self.cf_menu = None;
                    self.ribbon.clear_pointer();
                    return Some(true);
                }
                if let Some(s) = m.sub_at(x, y) {
                    let choice = m.sub_entries().get(s).map(|(_, c)| c.clone());
                    self.cf_menu = None;
                    self.ribbon.clear_pointer();
                    if let Some(c) = choice {
                        self.cf_choose(&c, text);
                    }
                    return Some(true);
                }
                if let Some(i) = m.main_at(x, y) {
                    if MAIN[i].4 {
                        m.open = Some(i);
                        return Some(true);
                    }
                    self.cf_menu = None;
                    self.ribbon.clear_pointer();
                    let id = MAIN[i].0;
                    self.cf_choose(&format!("cfd.{id}"), text);
                }
                true
            }
            Input::Key(Key::Escape, _) => {
                if m.open.is_some() {
                    m.open = None;
                    m.sub_hover = None;
                } else {
                    self.cf_menu = None;
                }
                true
            }
            Input::Release { .. } | Input::Drag { .. } | Input::Wheel(_) => true,
            _ => return None,
        })
    }

    /// Run a choice of the menu.
    pub(super) fn cf_choose(&mut self, c: &str, text: &dyn crate::TextMeasure) {
        let range = self.view.rng();
        let run = |w: &mut SheetWindow, rule: Value| {
            let ok = w
                .view
                .run(json!({ "cmd": "addCf", "range": range, "rule": rule }));
            if !ok {
                w.status = w.view.take_error().unwrap_or_default();
            }
        };
        if let Some(rest) = c.strip_prefix("cfb.") {
            let (kind, color) = rest.split_once('.').unwrap_or(("g", rest));
            let rule = if kind == "s" {
                json!({ "kind": "dataBar", "color": color, "options": {
                    "min": { "kind": "autoMin" }, "max": { "kind": "autoMax" },
                    "gradient": false, "border": false } })
            } else {
                json!({ "kind": "dataBar", "color": color })
            };
            run(self, rule);
        } else if let Some(i) = c.strip_prefix("cfs.").and_then(|s| s.parse::<usize>().ok()) {
            if let Some((min, mid, max)) = SCALES.get(i) {
                run(
                    self,
                    json!({ "kind": "colorScale", "minColor": min, "midColor": mid, "maxColor": max }),
                );
            }
        } else if let Some(set) = c.strip_prefix("cfi.") {
            run(self, json!({ "kind": "iconSet", "set": set }));
        } else if c == "cf.clearTable" {
            if let Some((_, t)) = self.table_at_active() {
                let range = t.get("range").cloned().unwrap_or(Value::Null);
                self.view
                    .run(json!({ "cmd": "clearCf", "range": range.clone() }));
            }
        } else if c == "cf.clearPivot" {
            // Pivot tables keep their own rules; the selection's are cleared.
            self.view
                .run(json!({ "cmd": "clearCf", "range": range.clone() }));
        } else if c.starts_with("cf.") {
            self.action(c, text);
        } else if let Some(d) = c.strip_prefix("cfd.") {
            self.open_cf_dialog(d);
        }
    }

    /// Paint the open menu and its submenu.
    pub(super) fn paint_cf_menu(&self, p: &mut dyn Painter) {
        let Some(m) = &self.cf_menu else { return };
        let k = m.k;
        let shadow = |p: &mut dyn Painter, [x, y, w, h]: [f32; 4]| {
            for (o, a) in [(3.0, 18u8), (2.0, 26), (1.0, 34)] {
                let o = (o * k).round();
                p.fill_rect(
                    x - o + 2.0,
                    y - o + 3.0,
                    w + 2.0 * o,
                    h + 2.0 * o,
                    Rgba(0, 0, 0, a),
                );
            }
            p.fill_rect(x, y, w, h, EDGE);
            p.fill_rect(x + 1.0, y + 1.0, w - 2.0, h - 2.0, BG);
        };
        let r = m.main_rect();
        shadow(p, r);
        let bold = TextStyle {
            size: (13.0 * self.scale).round(),
            bold: true,
        };
        let plain = TextStyle {
            size: (12.0 * self.scale).round(),
            bold: false,
        };
        for (i, (id, key, fallback, big, sub)) in MAIN.iter().enumerate() {
            let (t, h) = CfMenu::row(i);
            let (y, h) = (r[1] + m.d(t), m.d(h));
            if i == 2 || i == 5 {
                p.fill_rect(r[0] + m.d(46.0), y - m.d(2.0), r[2] - m.d(50.0), 1.0, SEP);
            }
            if m.hover == Some(i) || m.open == Some(i) {
                p.fill_rect(r[0] + 2.0, y, r[2] - 4.0, h, HOT_EDGE);
                p.fill_rect(r[0] + 3.0, y + 1.0, r[2] - 6.0, h - 2.0, HOT);
            }
            let style = if *big { bold } else { plain };
            let icon = if *big { m.d(40.0) } else { m.d(20.0) };
            paint_main_icon(p, id, r[0] + m.d(8.0), y + (h - icon) / 2.0, icon);
            let label = self.t(key, fallback);
            let tx = r[0] + if *big { m.d(62.0) } else { m.d(46.0) };
            p.text(
                &label,
                tx,
                (y + (h - style.size * 1.33) / 2.0).round(),
                style,
                INK,
            );
            if *sub {
                arrow_right(p, r[0] + r[2] - m.d(20.0), y + h / 2.0, m.d(4.0), INK);
            }
        }
        let Some(i) = m.open else { return };
        let Some(sr) = m.sub_rect() else { return };
        shadow(p, sr);
        let id = MAIN[i].0;
        let entries = m.sub_entries();
        let items = list_items(id);
        if !items.is_empty() {
            for (n, ((rect, choice), (_, key, fallback))) in
                entries.iter().zip(items.iter()).enumerate()
            {
                if choice == "cfd.newRule" {
                    p.fill_rect(
                        rect[0] + m.d(46.0),
                        rect[1] - m.d(2.0),
                        rect[2] - m.d(50.0),
                        1.0,
                        SEP,
                    );
                }
                if m.sub_hover == Some(n) {
                    p.fill_rect(rect[0], rect[1], rect[2], rect[3], HOT_EDGE);
                    p.fill_rect(
                        rect[0] + 1.0,
                        rect[1] + 1.0,
                        rect[2] - 2.0,
                        rect[3] - 2.0,
                        HOT,
                    );
                }
                let s = m.d(30.0);
                paint_rule_icon(
                    p,
                    choice,
                    rect[0] + m.d(6.0),
                    rect[1] + (rect[3] - s) / 2.0,
                    s,
                );
                let dim =
                    choice == "cf.clearPivot" || (choice == "cf.clearTable" && !self.table_tab);
                p.text(
                    &self.t(key, fallback),
                    rect[0] + m.d(46.0),
                    (rect[1] + (rect[3] - plain.size * 1.33) / 2.0).round(),
                    plain,
                    if dim {
                        Rgba(0xa6, 0xa6, 0xa6, 255)
                    } else {
                        INK
                    },
                );
            }
            return;
        }
        // Galleries: captions, tiles, More Rules….
        let (_, th) = CfMenu::tile_size(id);
        let mut ty = sr[1] + m.d(4.0);
        let caption = TextStyle {
            size: (12.0 * self.scale).round(),
            bold: true,
        };
        for (cap, fallback, tiles, cols) in gallery(id) {
            if !cap.is_empty() {
                p.fill_rect(sr[0] + 1.0, ty, sr[2] - 2.0, m.d(HEAD_H), HEAD_BG);
                p.text(
                    &self.t(cap, fallback),
                    sr[0] + m.d(8.0),
                    (ty + (m.d(HEAD_H) - caption.size * 1.33) / 2.0).round(),
                    caption,
                    HEAD_INK,
                );
                ty += m.d(HEAD_H);
            }
            ty += m.d(tiles.len().div_ceil(cols) as f32 * (th + 6.0) + 4.0);
        }
        for (n, (rect, choice)) in entries.iter().enumerate() {
            if choice == "cfd.newRule" {
                p.fill_rect(sr[0] + 2.0, rect[1] - m.d(2.0), sr[2] - 4.0, 1.0, SEP);
                if m.sub_hover == Some(n) {
                    p.fill_rect(rect[0], rect[1], rect[2], rect[3], HOT);
                }
                p.text(
                    &self.t("sheetCf.moreRules", "More Rules..."),
                    rect[0] + m.d(40.0),
                    (rect[1] + (rect[3] - plain.size * 1.33) / 2.0).round(),
                    plain,
                    INK,
                );
                continue;
            }
            if m.sub_hover == Some(n) {
                p.fill_rect(
                    rect[0] - 3.0,
                    rect[1] - 3.0,
                    rect[2] + 6.0,
                    rect[3] + 6.0,
                    Rgba(0x88, 0x8a, 0x86, 255),
                );
                p.fill_rect(
                    rect[0] - 2.0,
                    rect[1] - 2.0,
                    rect[2] + 4.0,
                    rect[3] + 4.0,
                    Rgba(0xd2, 0xd0, 0xce, 255),
                );
            }
            paint_tile(p, *rect, choice);
        }
    }
}

fn arrow_right(p: &mut dyn Painter, x: f32, cy: f32, s: f32, c: Rgba) {
    p.path(
        &[
            PathSeg::M(x, cy - s),
            PathSeg::L(x + s, cy),
            PathSeg::L(x, cy + s),
            PathSeg::Z,
        ],
        Some(c),
        None,
    );
}

fn hex(s: &str) -> Rgba {
    let v = u32::from_str_radix(s, 16).unwrap_or(0);
    Rgba((v >> 16) as u8, (v >> 8) as u8, v as u8, 255)
}

fn mix(a: Rgba, b: Rgba, t: f32) -> Rgba {
    let m = |x: u8, y: u8| (f32::from(x) + (f32::from(y) - f32::from(x)) * t).round() as u8;
    Rgba(m(a.0, b.0), m(a.1, b.1), m(a.2, b.2), 255)
}

/// A small grid (the cells a rule formats): `rows` × `cols` in a `s` square.
fn mini_grid(p: &mut dyn Painter, x: f32, y: f32, w: f32, h: f32, rows: u32, cols: u32) {
    p.fill_rect(x, y, w, h, GRID);
    p.fill_rect(x + 1.0, y + 1.0, w - 2.0, h - 2.0, BG);
    for r in 1..rows {
        p.fill_rect(
            x + 1.0,
            (y + h * r as f32 / rows as f32).round(),
            w - 2.0,
            1.0,
            GRID,
        );
    }
    for c in 1..cols {
        p.fill_rect(
            (x + w * c as f32 / cols as f32).round(),
            y + 1.0,
            1.0,
            h - 2.0,
            GRID,
        );
    }
}

/// Own pictures of the main menu entries (`s` square at x, y).
fn paint_main_icon(p: &mut dyn Painter, id: &str, x: f32, y: f32, s: f32) {
    let (x, y, s) = (x.round(), y.round(), s.round());
    let u = s / 40.0;
    match id {
        "highlight" | "topBottom" => {
            mini_grid(p, x, y, s, s, 4, 2);
            p.fill_rect(
                x + 1.0,
                (y + s / 4.0).round() + 1.0,
                (s / 2.0).round() - 1.0,
                (s / 4.0).round() - 1.0,
                ACCENT,
            );
            let c = Rgba(0x44, 0x44, 0x44, 255);
            let (bx, by) = (x + 20.0 * u, y + 24.0 * u);
            if id == "highlight" {
                // A "greater than" mark in the lower right cell.
                p.path(
                    &[
                        PathSeg::M(bx + 4.0 * u, by + 4.0 * u),
                        PathSeg::L(bx + 12.0 * u, by + 8.0 * u),
                        PathSeg::L(bx + 4.0 * u, by + 12.0 * u),
                    ],
                    None,
                    Some((c, 1.2 * u.max(1.0))),
                );
            } else {
                // An up arrow over "10".
                p.path(
                    &[
                        PathSeg::M(bx + 8.0 * u, by + 14.0 * u),
                        PathSeg::L(bx + 8.0 * u, by + 2.0 * u),
                        PathSeg::M(bx + 4.0 * u, by + 6.0 * u),
                        PathSeg::L(bx + 8.0 * u, by + 2.0 * u),
                        PathSeg::L(bx + 12.0 * u, by + 6.0 * u),
                    ],
                    None,
                    Some((c, 1.2 * u.max(1.0))),
                );
            }
        }
        "dataBars" => {
            mini_grid(p, x, y, s, s, 5, 1);
            for (r, len) in [0.7f32, 0.45, 0.85, 0.3, 0.6].iter().enumerate() {
                let ry = (y + 2.0 + r as f32 * (s - 2.0) / 5.0).round();
                p.fill_rect(
                    x + 3.0 * u,
                    ry + 2.0 * u,
                    ((s - 6.0 * u) * len).round(),
                    ((s - 2.0) / 5.0 - 4.0 * u).round().max(1.0),
                    hex("4F81BD"),
                );
            }
        }
        "colorScales" => {
            mini_grid(p, x, y, s, s, 5, 1);
            for (r, c) in ["63BE7B", "B5D57F", "FFEB84", "FCAA78", "F8696B"]
                .iter()
                .enumerate()
            {
                let ry = (y + 1.0 + r as f32 * (s - 2.0) / 5.0).round();
                let ry2 = (y + 1.0 + (r + 1) as f32 * (s - 2.0) / 5.0).round();
                p.fill_rect(x + 1.0, ry + 1.0, s - 2.0, ry2 - ry - 1.0, hex(c));
            }
        }
        "iconSets" => {
            mini_grid(p, x, y, s, s, 4, 1);
            for (r, c) in ["63BE7B", "FFC000", "F8696B", "63BE7B"].iter().enumerate() {
                let cy = y + (r as f32 + 0.5) * s / 4.0;
                dot(p, x + 8.0 * u, cy, 3.0 * u, hex(c));
                p.fill_rect(x + 15.0 * u, cy.round(), 18.0 * u, 1.0, GRID);
            }
        }
        _ => {
            // Small entries: a 2 × 2 grid with a tinted cell (Clear Rules adds an eraser).
            let (w, h) = (s, (s * 0.8).round());
            mini_grid(p, x, y + (s - h) / 2.0, w, h, 2, 2);
            p.fill_rect(
                x + 1.0,
                y + (s - h) / 2.0 + 1.0,
                (w / 2.0).round() - 1.0,
                (h / 2.0).round() - 1.0,
                ACCENT,
            );
            if id == "clear" {
                p.fill_rect(
                    x + w * 0.55,
                    y + s * 0.55,
                    w * 0.45,
                    s * 0.3,
                    Rgba(0xe8, 0x6a, 0x8f, 255),
                );
            }
        }
    }
}

fn dot(p: &mut dyn Painter, cx: f32, cy: f32, r: f32, c: Rgba) {
    let k = r * 0.552;
    p.path(
        &[
            PathSeg::M(cx + r, cy),
            PathSeg::C(cx + r, cy + k, cx + k, cy + r, cx, cy + r),
            PathSeg::C(cx - k, cy + r, cx - r, cy + k, cx - r, cy),
            PathSeg::C(cx - r, cy - k, cx - k, cy - r, cx, cy - r),
            PathSeg::C(cx + k, cy - r, cx + r, cy - k, cx + r, cy),
            PathSeg::Z,
        ],
        Some(c),
        None,
    );
}

/// Pictures of the list submenu entries: a small grid with the rule's highlighted cells.
fn paint_rule_icon(p: &mut dyn Painter, choice: &str, x: f32, y: f32, s: f32) {
    let (x, y, s) = (x.round(), y.round(), s.round());
    mini_grid(p, x, y, s, s, 3, 2);
    let cell = |p: &mut dyn Painter, r: u32, c: u32, color: Rgba| {
        let (cw, ch) = ((s / 2.0).round(), (s / 3.0).round());
        p.fill_rect(
            x + 1.0 + c as f32 * cw,
            y + 1.0 + r as f32 * ch,
            cw - 1.0,
            ch - 1.0,
            color,
        );
    };
    let pick: &[(u32, u32)] = match choice {
        "cfd.greaterThan" | "cfd.top" | "cfd.topPct" | "cfd.above" => &[(0, 0), (0, 1)],
        "cfd.lessThan" | "cfd.bottom" | "cfd.bottomPct" | "cfd.below" => &[(2, 0), (2, 1)],
        "cfd.between" => &[(1, 0), (1, 1)],
        "cfd.equal" | "cfd.text" | "cfd.date" => &[(1, 1)],
        "cfd.duplicate" => &[(0, 0), (2, 0)],
        "cf.clearSel" | "cf.clearSheet" | "cf.clearTable" | "cf.clearPivot" => &[],
        _ => &[(0, 1)],
    };
    for &(r, c) in pick {
        cell(p, r, c, ACCENT);
    }
    if choice.starts_with("cf.clear") {
        p.fill_rect(
            x + s * 0.5,
            y + s * 0.55,
            s * 0.5,
            s * 0.3,
            Rgba(0xe8, 0x6a, 0x8f, 255),
        );
    }
}

/// A gallery tile: a data bar, a colour scale or an icon set preview.
fn paint_tile(p: &mut dyn Painter, r: [f32; 4], choice: &str) {
    let [x, y, w, h] = r;
    if let Some(rest) = choice.strip_prefix("cfb.") {
        let (kind, color) = rest.split_once('.').unwrap_or(("g", rest));
        let c = hex(color);
        mini_grid(p, x, y, w, h, 4, 1);
        for (row, len) in [0.85f32, 0.35, 0.6, 0.2].iter().enumerate() {
            let ry = (y + 1.0 + row as f32 * (h - 2.0) / 4.0 + 2.0).round();
            let bh = ((h - 2.0) / 4.0 - 4.0).round().max(1.0);
            let bw = ((w - 6.0) * len).round();
            if kind == "g" {
                // Fades to white towards the far end.
                let steps = bw.max(1.0) as u32;
                for sx in 0..steps {
                    let t = sx as f32 / steps as f32;
                    p.fill_rect(x + 3.0 + sx as f32, ry, 1.0, bh, mix(c, BG, t * 0.85));
                }
                p.fill_rect(x + 3.0, ry, bw, 1.0, c);
                p.fill_rect(x + 3.0, ry + bh - 1.0, bw, 1.0, c);
            } else {
                p.fill_rect(x + 3.0, ry, bw, bh, c);
            }
        }
        return;
    }
    if let Some((min, mid, max)) = choice
        .strip_prefix("cfs.")
        .and_then(|s| s.parse::<usize>().ok())
        .and_then(|i| SCALES.get(i))
    {
        let (a, b) = (hex(max), hex(min));
        for row in 0..4 {
            let t = row as f32 / 3.0;
            let c = match mid {
                Some(m) if t <= 0.5 => mix(a, hex(m), t * 2.0),
                Some(m) => mix(hex(m), b, (t - 0.5) * 2.0),
                None => mix(a, b, t),
            };
            let ry = (y + row as f32 * h / 4.0).round();
            let ry2 = (y + (row + 1) as f32 * h / 4.0).round();
            p.fill_rect(x, ry, w, ry2 - ry, c);
        }
        mini_grid_lines(p, x, y, w, h, 4);
        return;
    }
    if let Some(set) = choice.strip_prefix("cfi.") {
        paint_icon_set(p, set, x, y, w, h);
    }
}

fn mini_grid_lines(p: &mut dyn Painter, x: f32, y: f32, w: f32, h: f32, rows: u32) {
    for r in 0..=rows {
        let ry = (y + h * r as f32 / rows as f32).round().min(y + h - 1.0);
        p.fill_rect(x, ry, w, 1.0, Rgba(0xff, 0xff, 0xff, 255));
    }
}

/// An icon set's icons in a row (own drawings: arrows, lights, signs, symbols, flags, stars,
/// bars, quarters, boxes), best first as the reference lists them.
fn paint_icon_set(p: &mut dyn Painter, set: &str, x: f32, y: f32, w: f32, h: f32) {
    let n: usize = set[..1].parse().unwrap_or(3);
    let green = hex("00B050");
    let yellow = hex("FFC000");
    let red = hex("FF0000");
    let grey = hex("808080");
    let step = w / n as f32;
    let s = (h * 0.62).min(step * 0.8);
    for i in 0..n {
        let cx = x + step * (i as f32 + 0.5);
        let cy = y + h / 2.0;
        let t = i as f32 / (n - 1).max(1) as f32;
        let traffic = if t < 0.34 {
            green
        } else if t < 0.67 {
            yellow
        } else {
            red
        };
        let up = 1.0 - 2.0 * t;
        match set {
            s3 if s3.contains("Arrows") || s3 == "3Triangles" => {
                let c = if set.ends_with("Gray") { grey } else { traffic };
                let a = s / 2.0;
                // Arrow pointing up (best), across, then down.
                let (dx, dy) = ((1.0 - up.abs()) * a, -up * a);
                let len = (dx * dx + dy * dy).sqrt().max(0.01);
                let (ux, uy) = (dx / len, dy / len);
                let (px, py) = (-uy, ux);
                let tip = (cx + ux * a, cy + uy * a);
                p.path(
                    &[
                        PathSeg::M(tip.0, tip.1),
                        PathSeg::L(cx + px * a * 0.7, cy + py * a * 0.7),
                        PathSeg::L(cx - px * a * 0.7, cy - py * a * 0.7),
                        PathSeg::Z,
                    ],
                    Some(c),
                    None,
                );
                if set != "3Triangles" {
                    p.path(
                        &[PathSeg::M(cx, cy), PathSeg::L(cx - ux * a, cy - uy * a)],
                        None,
                        Some((c, (s * 0.22).max(1.0))),
                    );
                }
            }
            "4RedToBlack" => dot(p, cx, cy, s / 2.0, mix(red, Rgba(0, 0, 0, 255), t)),
            "3Signs" => {
                let a = s / 2.0;
                p.path(
                    &[
                        PathSeg::M(cx, cy - a),
                        PathSeg::L(cx + a, cy),
                        PathSeg::L(cx, cy + a),
                        PathSeg::L(cx - a, cy),
                        PathSeg::Z,
                    ],
                    Some(traffic),
                    None,
                );
            }
            "3Symbols" | "3Symbols2" => {
                let filled = set == "3Symbols";
                if filled {
                    dot(p, cx, cy, s / 2.0, traffic);
                }
                let ink = if filled { BG } else { traffic };
                let a = s * 0.22;
                let stroke = Some((ink, (s * 0.14).max(1.0)));
                match i {
                    0 => p.path(
                        &[
                            PathSeg::M(cx - a, cy),
                            PathSeg::L(cx - a * 0.2, cy + a),
                            PathSeg::L(cx + a * 1.2, cy - a),
                        ],
                        None,
                        stroke,
                    ),
                    1 => p.path(
                        &[
                            PathSeg::M(cx, cy - a * 1.2),
                            PathSeg::L(cx, cy + a * 0.4),
                            PathSeg::M(cx, cy + a),
                            PathSeg::L(cx, cy + a * 1.2),
                        ],
                        None,
                        stroke,
                    ),
                    _ => p.path(
                        &[
                            PathSeg::M(cx - a, cy - a),
                            PathSeg::L(cx + a, cy + a),
                            PathSeg::M(cx + a, cy - a),
                            PathSeg::L(cx - a, cy + a),
                        ],
                        None,
                        stroke,
                    ),
                }
            }
            "3Flags" => {
                let a = s / 2.0;
                p.fill_rect(cx - a * 0.6, cy - a, 1.5, 2.0 * a, INK);
                p.path(
                    &[
                        PathSeg::M(cx - a * 0.5, cy - a),
                        PathSeg::L(cx + a, cy - a * 0.5),
                        PathSeg::L(cx - a * 0.5, cy),
                        PathSeg::Z,
                    ],
                    Some(traffic),
                    None,
                );
            }
            "3Stars" => {
                let fill = 1.0 - t;
                let a = s / 2.0;
                let star: Vec<PathSeg> = (0..10)
                    .map(|k| {
                        let ang =
                            std::f32::consts::PI * (k as f32 / 5.0) - std::f32::consts::FRAC_PI_2;
                        let rr = if k % 2 == 0 { a } else { a * 0.45 };
                        let (px, py) = (cx + rr * ang.cos(), cy + rr * ang.sin());
                        if k == 0 {
                            PathSeg::M(px, py)
                        } else {
                            PathSeg::L(px, py)
                        }
                    })
                    .chain([PathSeg::Z])
                    .collect();
                p.path(
                    &star,
                    Some(if fill > 0.6 {
                        yellow
                    } else if fill > 0.2 {
                        mix(yellow, BG, 0.5)
                    } else {
                        BG
                    }),
                    Some((yellow, 1.0)),
                );
            }
            "4Rating" | "5Rating" => {
                let bars = n - i;
                for b in 0..4 {
                    let bh = s * (b as f32 + 1.0) / 4.0;
                    let c = if b < bars {
                        hex("2E75B6")
                    } else {
                        hex("D9D9D9")
                    };
                    p.fill_rect(
                        cx - s / 2.0 + b as f32 * s / 4.0,
                        cy + s / 2.0 - bh,
                        (s / 4.0 - 1.0).max(1.0),
                        bh,
                        c,
                    );
                }
            }
            "5Quarters" => {
                dot(p, cx, cy, s / 2.0, BG);
                let a = s / 2.0;
                let part = 1.0 - t;
                if part > 0.99 {
                    dot(p, cx, cy, a, grey);
                } else if part > 0.01 {
                    let end = -std::f32::consts::FRAC_PI_2 + part * 2.0 * std::f32::consts::PI;
                    let mut seg = vec![PathSeg::M(cx, cy), PathSeg::L(cx, cy - a)];
                    for k in 1..=12 {
                        let ang = -std::f32::consts::FRAC_PI_2
                            + (end + std::f32::consts::FRAC_PI_2) * k as f32 / 12.0;
                        seg.push(PathSeg::L(cx + a * ang.cos(), cy + a * ang.sin()));
                    }
                    seg.push(PathSeg::Z);
                    p.path(&seg, Some(grey), None);
                }
                p.path(
                    &[
                        PathSeg::M(cx + a, cy),
                        PathSeg::C(cx + a, cy + a * 0.55, cx + a * 0.55, cy + a, cx, cy + a),
                        PathSeg::C(cx - a * 0.55, cy + a, cx - a, cy + a * 0.55, cx - a, cy),
                        PathSeg::C(cx - a, cy - a * 0.55, cx - a * 0.55, cy - a, cx, cy - a),
                        PathSeg::C(cx + a * 0.55, cy - a, cx + a, cy - a * 0.55, cx + a, cy),
                    ],
                    None,
                    Some((grey, 1.0)),
                );
            }
            "5Boxes" => {
                let filled = n - i - 1;
                for b in 0..4 {
                    let (bx, by) = (
                        cx - s / 2.0 + (b % 2) as f32 * s / 2.0,
                        cy - s / 2.0 + (b / 2) as f32 * s / 2.0,
                    );
                    let c = if b < filled {
                        hex("2E75B6")
                    } else {
                        hex("D9D9D9")
                    };
                    p.fill_rect(
                        bx,
                        by,
                        (s / 2.0 - 1.0).max(1.0),
                        (s / 2.0 - 1.0).max(1.0),
                        c,
                    );
                }
            }
            "4TrafficLights" => {
                let c = [green, yellow, red, Rgba(0x20, 0x20, 0x20, 255)][i.min(3)];
                dot(p, cx, cy, s / 2.0, c);
            }
            "3TrafficLights2" => {
                p.fill_rect(
                    cx - s / 2.0,
                    cy - s / 2.0,
                    s,
                    s,
                    Rgba(0x30, 0x30, 0x30, 255),
                );
                dot(p, cx, cy, s * 0.35, traffic);
            }
            _ => dot(p, cx, cy, s / 2.0, traffic),
        }
    }
}

// ---------------------------------------------------------------- rule dialogs

/// The rule a dialog adds (`which`: greaterThan, lessThan, between, equal, text, date,
/// duplicate, top, topPct, bottom, bottomPct, above, below).
impl SheetWindow {
    fn cf_presets(&self) -> Vec<String> {
        PRESETS
            .iter()
            .enumerate()
            .map(|(i, (l, _, _))| self.t(&format!("sheetCf.preset{i}"), l))
            .collect()
    }

    /// A number in the middle of the selection's values, as the reference proposes.
    fn cf_seed(&mut self) -> String {
        let s = self.view.stats();
        if s.numbers > 0 {
            let avg = s.sum / s.numbers as f64;
            let v = (avg * 100.0).round() / 100.0;
            return if v.fract() == 0.0 {
                format!("{}", v as i64)
            } else {
                format!("{v}")
            };
        }
        String::new()
    }

    /// Open the dialog of rule `which` (or New Rule / Manage Rules).
    pub(super) fn open_cf_dialog(&mut self, which: &str) {
        match which {
            "newRule" => return self.open_new_rule(None),
            "manage" => return self.open_cf_manager(),
            _ => {}
        }
        let seed = self.cf_seed();
        let presets = self.cf_presets();
        let t = |k: &str, d: &str| self.t(k, d);
        let (title, prompt) = match which {
            "greaterThan" => (
                t("sheetCf.dlgGreater", "Greater Than"),
                t("sheetCf.pGreater", "Format cells that are GREATER THAN:"),
            ),
            "lessThan" => (
                t("sheetCf.dlgLess", "Less Than"),
                t("sheetCf.pLess", "Format cells that are LESS THAN:"),
            ),
            "between" => (
                t("sheetCf.dlgBetween", "Between"),
                t("sheetCf.pBetween", "Format cells that are BETWEEN:"),
            ),
            "equal" => (
                t("sheetCf.dlgEqual", "Equal To"),
                t("sheetCf.pEqual", "Format cells that are EQUAL TO:"),
            ),
            "text" => (
                t("sheetCf.dlgText", "Text That Contains"),
                t("sheetCf.pText", "Format cells that contain the text:"),
            ),
            "date" => (
                t("sheetCf.dlgDate", "A Date Occurring"),
                t(
                    "sheetCf.pDate",
                    "Format cells that contain a date occurring:",
                ),
            ),
            "duplicate" => (
                t("sheetCf.dlgDuplicate", "Duplicate Values"),
                t("sheetCf.pDuplicate", "Format cells that contain:"),
            ),
            "top" | "topPct" => (
                t("sheetCf.dlgTop", "Top 10 Items"),
                t("sheetCf.pTop", "Format cells that rank in the TOP:"),
            ),
            "bottom" | "bottomPct" => (
                t("sheetCf.dlgBottom", "Bottom 10 Items"),
                t("sheetCf.pBottom", "Format cells that rank in the BOTTOM:"),
            ),
            "above" => (
                t("sheetCf.dlgAbove", "Above Average"),
                t("sheetCf.pAbove", "Format cells that are ABOVE AVERAGE:"),
            ),
            "below" => (
                t("sheetCf.dlgBelow", "Below Average"),
                t("sheetCf.pBelow", "Format cells that are BELOW AVERAGE:"),
            ),
            _ => return,
        };
        let title = if which.ends_with("Pct") {
            title.replace(" Items", "%")
        } else {
            title
        };
        let mut f = Form::new(&title, (500.0, 190.0), vec![]);
        f.add("prompt", None, [20.0, 54.0, 0.0, 0.0], Kind::Label(prompt));
        let edit = |s: &str| Kind::Edit {
            caret: s.chars().count(),
            value: s.chars().collect(),
        };
        let with_x;
        match which {
            "between" => {
                f.add("v1", None, [20.0, 80.0, 150.0, 25.0], edit(&seed));
                f.add(
                    "and",
                    None,
                    [178.0, 84.0, 0.0, 0.0],
                    Kind::Label(t("sheetCf.and", "and")),
                );
                f.add("v2", None, [212.0, 80.0, 150.0, 25.0], edit(&seed));
                with_x = 370.0;
            }
            "date" => {
                let items = DATES.iter().map(|(_, l, d)| t(l, d)).collect();
                f.add(
                    "v1",
                    None,
                    [20.0, 80.0, 200.0, 25.0],
                    Kind::Combo { items, sel: 0 },
                );
                with_x = 228.0;
            }
            "duplicate" => {
                let items = vec![
                    t("sheetCf.duplicate", "Duplicate"),
                    t("sheetCf.unique", "Unique"),
                ];
                f.add(
                    "v1",
                    None,
                    [20.0, 80.0, 150.0, 25.0],
                    Kind::Combo { items, sel: 0 },
                );
                with_x = 178.0;
            }
            "top" | "topPct" | "bottom" | "bottomPct" => {
                f.add(
                    "v1",
                    None,
                    [20.0, 80.0, 90.0, 25.0],
                    Kind::Spin {
                        value: 10,
                        min: 1,
                        max: 1000,
                    },
                );
                if which.ends_with("Pct") {
                    f.add(
                        "pct",
                        None,
                        [116.0, 84.0, 0.0, 0.0],
                        Kind::Label("%".into()),
                    );
                }
                with_x = 140.0;
            }
            "above" | "below" => {
                f.add(
                    "v1",
                    None,
                    [20.0, 84.0, 0.0, 0.0],
                    Kind::Label(t("sheetCf.forRange", "for the selected range")),
                );
                with_x = 200.0;
            }
            _ => {
                let v = if which == "text" { String::new() } else { seed };
                f.add("v1", None, [20.0, 80.0, 240.0, 25.0], edit(&v));
                with_x = 268.0;
            }
        }
        f.add(
            "with",
            None,
            [with_x, 84.0, 0.0, 0.0],
            Kind::Label(t("sheetCf.with", "with")),
        );
        f.add(
            "preset",
            None,
            [with_x + 36.0, 80.0, 464.0 - with_x - 36.0 + 16.0, 25.0],
            Kind::Combo {
                items: presets,
                sel: 0,
            },
        );
        f.ok_cancel(&t("common.ok", "OK"), &t("common.cancel", "Cancel"));
        f.focus("v1");
        self.form = Some(f);
        self.form_kind = FormKind::CfRule;
        self.cf_which = which.to_owned();
    }

    fn cf_style(&self, f: &Form, id: &str) -> Value {
        let (_, fill, font) = PRESETS[f.sel(id).unwrap_or(0).min(PRESETS.len() - 1)];
        json!({ "fill": fill, "fontColor": font })
    }

    /// The rule of the open simple rule dialog.
    fn cf_rule_of_form(&self) -> Option<Value> {
        let f = self.form.as_ref()?;
        let style = self.cf_style(f, "preset");
        let v1 = f.text("v1");
        let (r0, c0, r1, c1) = self.view.range_bounds();
        let a1 = |r: u32, c: u32| format!("{}{}", bo_model::sheet::col_to_letters(c), r + 1);
        let abs = |r: u32, c: u32| format!("${}${}", bo_model::sheet::col_to_letters(c), r + 1);
        let cell = a1(r0, c0);
        let all = format!("{}:{}", abs(r0, c0), abs(r1, c1));
        let q = |s: &str| {
            let s = s.trim();
            if s.parse::<f64>().is_ok() || s.starts_with('=') {
                s.trim_start_matches('=').to_owned()
            } else {
                format!("\"{}\"", s.replace('"', "\"\""))
            }
        };
        Some(match self.cf_which.as_str() {
            "greaterThan" | "lessThan" | "equal" => {
                let op = match self.cf_which.as_str() {
                    "greaterThan" => "greaterThan",
                    "lessThan" => "lessThan",
                    _ => "equal",
                };
                json!({ "kind": "cellIs", "operator": op, "formula1": q(&v1), "style": style })
            }
            "between" => json!({ "kind": "cellIs", "operator": "between",
                "formula1": q(&v1), "formula2": q(&f.text("v2")), "style": style }),
            "text" => json!({ "kind": "containsText", "text": v1, "style": style }),
            "date" => {
                let i = f.sel("v1").unwrap_or(0);
                let formula = DATES
                    .get(i)
                    .map_or(String::new(), |(fm, _, _)| fm.replace("A1", &cell));
                json!({ "kind": "expression", "formula": formula, "style": style })
            }
            "duplicate" => {
                if f.sel("v1") == Some(1) {
                    json!({ "kind": "expression", "formula": format!("COUNTIF({all},{cell})=1"), "style": style })
                } else {
                    json!({ "kind": "duplicateValues", "style": style })
                }
            }
            w @ ("top" | "topPct" | "bottom" | "bottomPct") => json!({ "kind": "top10",
                "rank": f.num("v1").max(1), "bottom": w.starts_with("bottom"),
                "percent": w.ends_with("Pct"), "style": style }),
            w @ ("above" | "below") => {
                let op = if w == "above" { ">" } else { "<" };
                json!({ "kind": "expression", "formula": format!("{cell}{op}AVERAGE({all})"), "style": style })
            }
            _ => return None,
        })
    }

    /// Events of the rule dialogs; `true` when handled.
    pub(super) fn cf_form_event(&mut self, kind: FormKind, ev: &Event) -> bool {
        match (kind, ev) {
            (FormKind::CfRule, Event::Ok) => {
                if let Some(rule) = self.cf_rule_of_form() {
                    let range = self.view.rng();
                    if self
                        .view
                        .run(json!({ "cmd": "addCf", "range": range, "rule": rule }))
                    {
                        self.form = None;
                    } else {
                        self.status = self.view.take_error().unwrap_or_default();
                    }
                }
            }
            (FormKind::CfRule | FormKind::CfNewRule, Event::Cancel) => {
                if let Some((f, k)) = self.form_stash.take() {
                    self.form = Some(f);
                    self.form_kind = k;
                } else {
                    self.form = None;
                }
            }
            (FormKind::CfNewRule, Event::Changed("type")) => self.cf_new_rule_show(),
            (FormKind::CfNewRule, Event::Ok) => self.cf_new_rule_ok(),
            (FormKind::CfManager, _) => return self.cf_manager_event(ev),
            _ => return false,
        }
        true
    }
}

/// A Date Occurring choices: (formula for cell A1, label key, fallback).
const DATES: [(&str, &str, &str); 10] = [
    ("FLOOR(A1,1)=TODAY()-1", "sheetCf.yesterday", "Yesterday"),
    ("FLOOR(A1,1)=TODAY()", "sheetCf.today", "Today"),
    ("FLOOR(A1,1)=TODAY()+1", "sheetCf.tomorrow", "Tomorrow"),
    ("AND(TODAY()-FLOOR(A1,1)<=6,FLOOR(A1,1)<=TODAY())", "sheetCf.last7", "In the last 7 days"),
    ("AND(TODAY()-ROUNDDOWN(A1,0)>=(WEEKDAY(TODAY())),TODAY()-ROUNDDOWN(A1,0)<(WEEKDAY(TODAY())+7))", "sheetCf.lastWeek", "Last week"),
    ("AND(TODAY()-ROUNDDOWN(A1,0)<=WEEKDAY(TODAY())-1,ROUNDDOWN(A1,0)-TODAY()<=7-WEEKDAY(TODAY()))", "sheetCf.thisWeek", "This week"),
    ("AND(ROUNDDOWN(A1,0)-TODAY()>(7-WEEKDAY(TODAY())),ROUNDDOWN(A1,0)-TODAY()<(15-WEEKDAY(TODAY())))", "sheetCf.nextWeek", "Next week"),
    ("AND(MONTH(A1)=MONTH(EDATE(TODAY(),0-1)),YEAR(A1)=YEAR(EDATE(TODAY(),0-1)))", "sheetCf.lastMonth", "Last month"),
    ("AND(MONTH(A1)=MONTH(TODAY()),YEAR(A1)=YEAR(TODAY()))", "sheetCf.thisMonth", "This month"),
    ("AND(MONTH(A1)=MONTH(EDATE(TODAY(),0+1)),YEAR(A1)=YEAR(EDATE(TODAY(),0+1)))", "sheetCf.nextMonth", "Next month"),
];

/// Rule types of New Formatting Rule, in the reference's order.
const RULE_TYPES: [(&str, &str); 6] = [
    (
        "sheetCf.typeValues",
        "Format all cells based on their values",
    ),
    ("sheetCf.typeContain", "Format only cells that contain"),
    (
        "sheetCf.typeRanked",
        "Format only top or bottom ranked values",
    ),
    (
        "sheetCf.typeAverage",
        "Format only values that are above or below average",
    ),
    (
        "sheetCf.typeUnique",
        "Format only unique or duplicate values",
    ),
    (
        "sheetCf.typeFormula",
        "Use a formula to determine which cells to format",
    ),
];

const OPERATORS: [(&str, &str); 8] = [
    ("between", "between"),
    ("notBetween", "not between"),
    ("equal", "equal to"),
    ("notEqual", "not equal to"),
    ("greaterThan", "greater than"),
    ("lessThan", "less than"),
    ("greaterThanOrEqual", "greater than or equal to"),
    ("lessThanOrEqual", "less than or equal to"),
];

impl SheetWindow {
    /// New Formatting Rule (or Edit Formatting Rule with `edit` = (block, rule, spec)).
    pub(super) fn open_new_rule(&mut self, edit: Option<(usize, usize, Value)>) {
        let t = |k: &str, d: &str| self.t(k, d);
        let title = if edit.is_some() {
            t("sheetCf.editRuleTitle", "Edit Formatting Rule")
        } else {
            t("sheetCf.newRuleTitle", "New Formatting Rule")
        };
        let mut f = Form::new(&title, (580.0, 470.0), vec![]);
        f.add(
            "typeLabel",
            None,
            [20.0, 54.0, 0.0, 0.0],
            Kind::Label(t("sheetCf.selectType", "Select a Rule Type:")),
        );
        f.add(
            "type",
            None,
            [20.0, 76.0, 540.0, 112.0],
            Kind::List {
                items: RULE_TYPES
                    .iter()
                    .map(|(k, d)| format!("\u{25B8} {}", t(k, d)))
                    .collect(),
                sel: Some(0),
                top: 0,
            },
        );
        f.add(
            "descLabel",
            None,
            [20.0, 200.0, 0.0, 0.0],
            Kind::Label(t("sheetCf.editDesc", "Edit the Rule Description:")),
        );
        // Values: format style.
        f.add(
            "style",
            None,
            [40.0, 230.0, 260.0, 25.0],
            Kind::Combo {
                items: vec![
                    t("sheetCf.scale2", "2-Color Scale"),
                    t("sheetCf.scale3", "3-Color Scale"),
                    t("sheetCf.dataBar", "Data Bar"),
                ],
                sel: 0,
            },
        );
        // Contain: operator and values.
        f.add(
            "op",
            None,
            [40.0, 230.0, 200.0, 25.0],
            Kind::Combo {
                items: OPERATORS.iter().map(|(_, l)| (*l).to_owned()).collect(),
                sel: 0,
            },
        );
        let edit_kind = |s: &str| Kind::Edit {
            caret: s.chars().count(),
            value: s.chars().collect(),
        };
        f.add("v1", None, [250.0, 230.0, 140.0, 25.0], edit_kind(""));
        f.add("v2", None, [400.0, 230.0, 140.0, 25.0], edit_kind(""));
        // Ranked.
        f.add(
            "tb",
            None,
            [40.0, 230.0, 120.0, 25.0],
            Kind::Combo {
                items: vec![
                    t("sheetCf.topWord", "Top"),
                    t("sheetCf.bottomWord", "Bottom"),
                ],
                sel: 0,
            },
        );
        f.add(
            "rank",
            None,
            [170.0, 230.0, 90.0, 25.0],
            Kind::Spin {
                value: 10,
                min: 1,
                max: 1000,
            },
        );
        f.add(
            "percent",
            None,
            [270.0, 232.0, 200.0, 20.0],
            Kind::Check {
                label: t("sheetCf.pctOfRange", "% of the selected range"),
                on: false,
            },
        );
        // Average.
        f.add(
            "avg",
            None,
            [40.0, 230.0, 200.0, 25.0],
            Kind::Combo {
                items: vec![t("sheetCf.above", "above"), t("sheetCf.below", "below")],
                sel: 0,
            },
        );
        // Unique.
        f.add(
            "dup",
            None,
            [40.0, 230.0, 200.0, 25.0],
            Kind::Combo {
                items: vec![
                    t("sheetCf.duplicate", "duplicate"),
                    t("sheetCf.unique", "unique"),
                ],
                sel: 0,
            },
        );
        // Formula.
        f.add("formula", None, [40.0, 230.0, 500.0, 25.0], edit_kind("="));
        // Format.
        f.add(
            "fmtLabel",
            None,
            [40.0, 290.0, 0.0, 0.0],
            Kind::Label(t("sheetCf.formatLabel", "Format:")),
        );
        let presets = self.cf_presets();
        f.add(
            "preset",
            None,
            [120.0, 286.0, 300.0, 25.0],
            Kind::Combo {
                items: presets,
                sel: 0,
            },
        );
        f.ok_cancel(&t("common.ok", "OK"), &t("common.cancel", "Cancel"));
        self.cf_edit = edit.as_ref().map(|(b, r, _)| (*b, *r));
        if let Some((_, _, spec)) = &edit {
            let kind = spec.get("kind").and_then(Value::as_str).unwrap_or_default();
            let ty = match kind {
                "colorScale" | "dataBar" | "iconSet" => 0,
                "cellIs" => 1,
                "top10" => 2,
                "duplicateValues" => 4,
                _ => 5,
            };
            f.set_sel("type", Some(ty));
            let s = |k: &str| {
                spec.get(k)
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned()
            };
            match kind {
                "cellIs" => {
                    let op = s("operator");
                    f.set_sel("op", OPERATORS.iter().position(|(o, _)| *o == op));
                    f.set_text("v1", &s("formula1"));
                    f.set_text("v2", &s("formula2"));
                }
                "top10" => {
                    f.set_sel(
                        "tb",
                        Some(usize::from(
                            spec.get("bottom").and_then(Value::as_bool).unwrap_or(false),
                        )),
                    );
                    f.set_num(
                        "rank",
                        spec.get("rank").and_then(Value::as_i64).unwrap_or(10) as i32,
                    );
                    f.set_on(
                        "percent",
                        spec.get("percent")
                            .and_then(Value::as_bool)
                            .unwrap_or(false),
                    );
                }
                "expression" => f.set_text("formula", &format!("={}", s("formula"))),
                "containsText" => {
                    f.set_sel("type", Some(5));
                    f.set_text(
                        "formula",
                        &format!("=ISNUMBER(SEARCH(\"{}\",A1))", s("text")),
                    );
                }
                "dataBar" => f.set_sel("style", Some(2)),
                "colorScale" => f.set_sel(
                    "style",
                    Some(if spec.get("midColor").is_some_and(|m| !m.is_null()) {
                        1
                    } else {
                        0
                    }),
                ),
                _ => {}
            }
            if let Some(st) = spec.get("style") {
                let fill = st.get("fill").and_then(Value::as_str);
                let font = st.get("fontColor").and_then(Value::as_str);
                if let Some(i) = PRESETS
                    .iter()
                    .position(|(_, a, b)| *a == fill && *b == font)
                {
                    f.set_sel("preset", Some(i));
                }
            }
        }
        if let Some(main) = self.form.take() {
            self.form_stash = Some((main, self.form_kind));
        }
        self.form = Some(f);
        self.form_kind = FormKind::CfNewRule;
        self.cf_new_rule_show();
    }

    /// Show the controls of the chosen rule type.
    fn cf_new_rule_show(&mut self) {
        let Some(f) = self.form.as_mut() else { return };
        let ty = f.sel("type").unwrap_or(0);
        let groups: [&[&str]; 6] = [
            &["style"],
            &["op", "v1", "v2"],
            &["tb", "rank", "percent"],
            &["avg"],
            &["dup"],
            &["formula"],
        ];
        for (i, ids) in groups.iter().enumerate() {
            for id in *ids {
                f.show(id, i == ty);
            }
        }
        let format = ty != 0;
        f.show("fmtLabel", format);
        f.show("preset", format);
    }

    fn cf_new_rule_ok(&mut self) {
        let Some(f) = self.form.as_ref() else { return };
        let style = self.cf_style(f, "preset");
        let (r0, c0, r1, c1) = self.view.range_bounds();
        let a1 = format!("{}{}", bo_model::sheet::col_to_letters(c0), r0 + 1);
        let abs = |r: u32, c: u32| format!("${}${}", bo_model::sheet::col_to_letters(c), r + 1);
        let all = format!("{}:{}", abs(r0, c0), abs(r1, c1));
        let strip = |s: String| s.trim().trim_start_matches('=').to_owned();
        let rule = match f.sel("type").unwrap_or(0) {
            0 => match f.sel("style").unwrap_or(0) {
                0 => json!({ "kind": "colorScale", "minColor": "F8696B", "maxColor": "63BE7B" }),
                1 => {
                    json!({ "kind": "colorScale", "minColor": "F8696B", "midColor": "FFEB84", "maxColor": "63BE7B" })
                }
                _ => json!({ "kind": "dataBar", "color": "638EC6" }),
            },
            1 => {
                let op = OPERATORS[f.sel("op").unwrap_or(0).min(OPERATORS.len() - 1)].0;
                let two = matches!(op, "between" | "notBetween");
                json!({ "kind": "cellIs", "operator": op, "formula1": strip(f.text("v1")),
                    "formula2": two.then(|| strip(f.text("v2"))), "style": style })
            }
            2 => {
                json!({ "kind": "top10", "rank": f.num("rank").max(1), "bottom": f.sel("tb") == Some(1),
                "percent": f.on("percent"), "style": style })
            }
            3 => {
                let op = if f.sel("avg") == Some(1) { "<" } else { ">" };
                json!({ "kind": "expression", "formula": format!("{a1}{op}AVERAGE({all})"), "style": style })
            }
            4 => {
                if f.sel("dup") == Some(1) {
                    json!({ "kind": "expression", "formula": format!("COUNTIF({all},{a1})=1"), "style": style })
                } else {
                    json!({ "kind": "duplicateValues", "style": style })
                }
            }
            _ => {
                json!({ "kind": "expression", "formula": strip(f.text("formula")), "style": style })
            }
        };
        let ok = if let Some((block, r)) = self.cf_edit.take() {
            self.view
                .run(json!({ "cmd": "editCf", "block": block, "rule": r,
                "op": { "op": "replace", "rule": rule } }))
        } else {
            let range = self.view.rng();
            self.view
                .run(json!({ "cmd": "addCf", "range": range, "rule": rule }))
        };
        if !ok {
            self.status = self.view.take_error().unwrap_or_default();
            return;
        }
        if let Some((f, k)) = self.form_stash.take() {
            self.form = Some(f);
            self.form_kind = k;
            if k == FormKind::CfManager {
                self.cf_manager_reload(None);
            }
        } else {
            self.form = None;
        }
    }

    // ------------------------------------------------------------ rules manager

    /// Conditional Formatting Rules Manager: the rules of the selection or the sheet in
    /// evaluation order (rule, format, applies to, Stop If True), New / Edit / Delete Rule and
    /// the order arrows.
    pub(super) fn open_cf_manager(&mut self) {
        let t = |k: &str, d: &str| self.t(k, d);
        let mut f = Form::new(
            &t(
                "sheetCf.managerTitle",
                "Conditional Formatting Rules Manager",
            ),
            (760.0, 420.0),
            vec![],
        );
        f.add(
            "showLabel",
            None,
            [20.0, 56.0, 0.0, 0.0],
            Kind::Label(t("sheetCf.showFor", "Show formatting rules for:")),
        );
        f.add(
            "scope",
            None,
            [220.0, 52.0, 220.0, 25.0],
            Kind::Combo {
                items: vec![
                    t("sheetCf.currentSelection", "Current Selection"),
                    t("sheetCf.thisSheet", "This Worksheet"),
                ],
                sel: 0,
            },
        );
        for (id, key, label, x, w) in [
            ("new", "sheetCf.newRuleBtn", "New Rule...", 20.0, 110.0),
            ("edit", "sheetCf.editRuleBtn", "Edit Rule...", 136.0, 110.0),
            (
                "delete",
                "sheetCf.deleteRuleBtn",
                "Delete Rule",
                252.0,
                110.0,
            ),
            ("up", "sheetChart.moveUp", "\u{25B2}", 380.0, 36.0),
            ("down", "sheetChart.moveDown", "\u{25BC}", 420.0, 36.0),
        ] {
            f.add(
                id,
                None,
                [x, 90.0, w, 28.0],
                Kind::Button {
                    label: t(key, label),
                    default: false,
                },
            );
        }
        f.add("rules", None, [20.0, 128.0, 720.0, 230.0], Kind::Canvas);
        f.ok_cancel(&t("common.ok", "OK"), &t("common.cancel", "Cancel"));
        self.form = Some(f);
        self.form_kind = FormKind::CfManager;
        self.cf_manager_reload(Some(0));
    }

    /// Read the rules shown by the manager (keeping `choose` selected).
    fn cf_manager_reload(&mut self, choose: Option<usize>) {
        let all = self
            .view
            .query(json!({ "cmd": "listCf" }))
            .and_then(|v| v.get("rules").and_then(Value::as_array).cloned())
            .unwrap_or_default();
        let sheet_scope = self.form.as_ref().and_then(|f| f.sel("scope")) == Some(1);
        let (r0, c0, r1, c1) = self.view.range_bounds();
        let overlaps = |range: &str| {
            range.split_whitespace().any(|part| {
                bo_model::sheet::CellRange::parse(&part.replace('$', ""))
                    .or_else(|| {
                        bo_model::sheet::CellRef::parse(part)
                            .map(bo_model::sheet::CellRange::single)
                    })
                    .is_some_and(|r| {
                        r.first.row <= r1
                            && r.last.row >= r0
                            && r.first.col <= c1
                            && r.last.col >= c0
                    })
            })
        };
        self.cf_rules = all
            .into_iter()
            .filter(|r| {
                sheet_scope || overlaps(r.get("range").and_then(Value::as_str).unwrap_or_default())
            })
            .collect();
        let n = self.cf_rules.len();
        self.cf_pick = choose
            .or(self.cf_pick)
            .filter(|i| *i < n)
            .or((n > 0).then_some(0));
    }

    /// A rule's description as the reference's list shows it.
    fn cf_describe(&self, r: &Value) -> String {
        let spec = r.get("spec").cloned().unwrap_or(Value::Null);
        let s = |k: &str| {
            spec.get(k)
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned()
        };
        match r.get("kind").and_then(Value::as_str).unwrap_or_default() {
            "cellIs" => {
                let op = s("operator");
                let word = OPERATORS
                    .iter()
                    .find(|(o, _)| *o == op)
                    .map_or(op.as_str(), |(_, l)| *l);
                let two = if s("formula2").is_empty() {
                    String::new()
                } else {
                    format!(" and {}", s("formula2"))
                };
                format!("Cell Value {word} {}{two}", s("formula1"))
            }
            "expression" => format!("Formula: ={}", s("formula")),
            "containsText" => format!("Cell value contains \"{}\"", s("text")),
            "top10" => {
                let bottom = spec.get("bottom").and_then(Value::as_bool).unwrap_or(false);
                let pct = if spec
                    .get("percent")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
                {
                    "%"
                } else {
                    ""
                };
                let rank = spec.get("rank").and_then(Value::as_u64).unwrap_or(10);
                format!("{} {rank}{pct}", if bottom { "Bottom" } else { "Top" })
            }
            "duplicateValues" => "Duplicate Values".into(),
            "colorScale" => "Graded Color Scale".into(),
            "dataBar" => "Data Bar".into(),
            "iconSet" => "Icon Set".into(),
            other => other.to_owned(),
        }
    }

    /// The manager's rule table, drawn into its canvas.
    pub(super) fn paint_cf_rules(&self, r: [f32; 4], p: &mut dyn Painter, k: f32) {
        let [x, y, w, h] = r;
        p.fill_rect(x, y, w, h, Rgba(0x82, 0x87, 0x90, 255));
        p.fill_rect(x + 1.0, y + 1.0, w - 2.0, h - 2.0, BG);
        let head = (24.0 * k).round();
        p.fill_rect(x + 1.0, y + 1.0, w - 2.0, head, Rgba(0xf3, 0xf3, 0xf3, 255));
        let style = TextStyle {
            size: (12.0 * self.scale).round(),
            bold: false,
        };
        let cols = [0.0, 0.42, 0.62, 0.86];
        let titles = [
            self.t("sheetCf.colRule", "Rule (applied in order shown)"),
            self.t("sheetCf.colFormat", "Format"),
            self.t("sheetCf.colApplies", "Applies to"),
            self.t("sheetCf.colStop", "Stop If True"),
        ];
        for (c, t) in cols.iter().zip(&titles) {
            p.text(
                t,
                x + w * c + (6.0 * k).round(),
                y + (4.0 * k).round(),
                style,
                INK,
            );
        }
        let row_h = (30.0 * k).round();
        for (i, rule) in self.cf_rules.iter().enumerate() {
            let ry = y + head + 1.0 + i as f32 * row_h;
            if ry + row_h > y + h {
                break;
            }
            if self.cf_pick == Some(i) {
                p.fill_rect(x + 1.0, ry, w - 2.0, row_h, Rgba(0xcc, 0xe8, 0xff, 255));
            }
            let ty = ry + (row_h - style.size * 1.33) / 2.0;
            p.text(
                &self.cf_describe(rule),
                x + (6.0 * k).round(),
                ty,
                style,
                INK,
            );
            // Format preview: the rule's fill and text colour on "AaBbCcYyZz".
            let spec = rule.get("spec").cloned().unwrap_or(Value::Null);
            let st = spec.get("style").cloned().unwrap_or(Value::Null);
            let fx = x + w * cols[1] + (4.0 * k).round();
            let fw = w * (cols[2] - cols[1]) - (10.0 * k).round();
            let fill = st.get("fill").and_then(Value::as_str).map(hex);
            let ink = st.get("fontColor").and_then(Value::as_str).map_or(INK, hex);
            match rule.get("kind").and_then(Value::as_str) {
                Some("colorScale") => {
                    let a = spec.get("minColor").and_then(Value::as_str).map_or(BG, hex);
                    let b = spec.get("maxColor").and_then(Value::as_str).map_or(BG, hex);
                    let m = spec.get("midColor").and_then(Value::as_str).map(hex);
                    let n = fw.max(1.0) as u32;
                    for s in 0..n {
                        let t = s as f32 / n as f32;
                        let c = match m {
                            Some(m) if t < 0.5 => mix(a, m, t * 2.0),
                            Some(m) => mix(m, b, (t - 0.5) * 2.0),
                            None => mix(a, b, t),
                        };
                        p.fill_rect(fx + s as f32, ry + 4.0, 1.0, row_h - 8.0, c);
                    }
                }
                Some("dataBar") => {
                    let c = spec.get("color").and_then(Value::as_str).map_or(BG, hex);
                    p.fill_rect(fx, ry + 4.0, fw * 0.7, row_h - 8.0, c);
                }
                Some("iconSet") => {
                    let set = spec
                        .get("set")
                        .and_then(Value::as_str)
                        .unwrap_or("3TrafficLights1")
                        .to_owned();
                    paint_icon_set(p, &set, fx, ry + 3.0, fw * 0.6, row_h - 6.0);
                }
                _ => {
                    if let Some(c) = fill {
                        p.fill_rect(fx, ry + 4.0, fw, row_h - 8.0, c);
                    }
                    p.text("AaBbCcYyZz", fx + (8.0 * k).round(), ty, style, ink);
                }
            }
            let range = rule
                .get("range")
                .and_then(Value::as_str)
                .unwrap_or_default();
            p.text(
                &format!("={range}"),
                x + w * cols[2] + (6.0 * k).round(),
                ty,
                style,
                INK,
            );
            let on = rule
                .get("stopIfTrue")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let b = (13.0 * k).round();
            super::form_impl::paint_check(
                p,
                [
                    x + w * cols[3] + (36.0 * k).round(),
                    (ry + (row_h - b) / 2.0).round(),
                    b,
                    b,
                ],
                on,
                true,
                false,
                self.scale,
            );
        }
    }

    fn cf_manager_event(&mut self, ev: &Event) -> bool {
        let pick = self.cf_pick;
        let rule = pick.and_then(|i| self.cf_rules.get(i).cloned());
        let at = |r: &Value| {
            (
                r.get("block").and_then(Value::as_u64).unwrap_or(0) as usize,
                r.get("rule").and_then(Value::as_u64).unwrap_or(0) as usize,
            )
        };
        match ev {
            Event::Ok | Event::Cancel => {
                self.form = None;
                self.cf_rules.clear();
            }
            Event::Changed("scope") => self.cf_manager_reload(None),
            Event::Clicked("new") => self.open_new_rule(None),
            Event::Clicked("edit") => {
                if let Some(r) = rule {
                    let (b, i) = at(&r);
                    let spec = r.get("spec").cloned().unwrap_or(Value::Null);
                    self.open_new_rule(Some((b, i, spec)));
                }
            }
            Event::Clicked(op @ ("delete" | "up" | "down")) => {
                if let Some(r) = rule {
                    let (b, i) = at(&r);
                    let name = match *op {
                        "delete" => "delete",
                        "up" => "moveUp",
                        _ => "moveDown",
                    };
                    if self.view.run(
                        json!({ "cmd": "editCf", "block": b, "rule": i, "op": { "op": name } }),
                    ) {
                        let next = pick.map(|p| match *op {
                            "up" => p.saturating_sub(1),
                            "down" => p + 1,
                            _ => p,
                        });
                        self.cf_manager_reload(next);
                    } else {
                        self.status = self.view.take_error().unwrap_or_default();
                    }
                }
            }
            Event::Canvas("rules", cx, cy) => {
                let head = 24.0;
                let row = ((cy - head - 1.0) / 30.0).floor();
                if row >= 0.0 && (row as usize) < self.cf_rules.len() {
                    let i = row as usize;
                    self.cf_pick = Some(i);
                    // The Stop If True column toggles.
                    let w = 720.0;
                    if *cx >= w * 0.86 {
                        let r = self.cf_rules[i].clone();
                        let (b, k) = at(&r);
                        let on = r
                            .get("stopIfTrue")
                            .and_then(Value::as_bool)
                            .unwrap_or(false);
                        self.view
                            .run(json!({ "cmd": "editCf", "block": b, "rule": k,
                            "op": { "op": "stopIfTrue", "value": !on } }));
                        self.cf_manager_reload(Some(i));
                    }
                }
            }
            _ => {}
        }
        true
    }
}
