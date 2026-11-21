//! Non-modal tool windows of BlueSheet (round 52):
//!
//! * Review › Check Accessibility as the reference's right-hand task pane (measured at 125 %
//!   on the reference, `show/sheet-60`): a 400 px pane right of the grid's scroll bar, the
//!   title with a close button, a status card ("Keep going!" / "Good to go!"), the rules in
//!   categories (Color and Contrast, Media and Illustrations, Tables, Document Structure,
//!   Document Access) as white cards of 56 px rows — a green check when the rule passes, the
//!   number of issues when it does not — and a details view per issue (‹ › to step, the
//!   reason, a fix button). The checks run again after every change (the pane "keeps
//!   running"); the status bar shows "Accessibility: Investigate / Good to go".
//! * Formulas › Watch Window as the reference's floating tool window (556 × 239 px): Add
//!   Watch… / Delete Watch, the Book / Sheet / Name / Cell / Value / Formula list (engine
//!   `watchValues`; the watched cells are kept in the workbook through `setCellWatches`).
use super::SheetWindow;
use crate::{Input, Painter, TextMeasure, TextStyle};
use bo_render::{PathSeg, Rgba};
use serde_json::{json, Value};

const PANE_BG: Rgba = Rgba(0xE6, 0xE6, 0xE6, 255);
const PANE_EDGE: Rgba = Rgba(0xBF, 0xBF, 0xBF, 255);
const CARD_EDGE: Rgba = Rgba(0xE0, 0xE0, 0xE0, 255);
const WHITE: Rgba = Rgba(255, 255, 255, 255);
const INK: Rgba = Rgba(0x24, 0x24, 0x24, 255);
const PASSED: Rgba = Rgba(0x99, 0x99, 0x99, 255);
const GREEN: Rgba = Rgba(0x0E, 0x70, 0x0E, 255);
const DESC: Rgba = Rgba(0x61, 0x61, 0x61, 255);
const BLACK: Rgba = Rgba(0, 0, 0, 255);

/// Categories of the accessibility rules (label key, fallback).
const CATS: [(&str, &str); 5] = [
    ("sheetAcc.catColor", "Color and Contrast"),
    ("sheetAcc.catMedia", "Media and Illustrations"),
    ("sheetAcc.catTables", "Tables"),
    ("sheetAcc.catStructure", "Document Structure"),
    ("sheetAcc.catAccess", "Document Access"),
];

/// Rules: category, id (keys `sheetAcc.<id>`, `sheetAcc.<id>Why`, `sheetAcc.<id>Fix`),
/// title, reason and fix fallbacks.
/// Index in [`RULES`] of the default sheet name rule (`out[6]` of `acc_check`).
const DEFAULT_NAME_RULE: usize = 6;

const RULES: [(usize, &str, &str, &str, &str); 8] = [
    (
        0,
        "contrast",
        "Hard-to-read text contrast",
        "Text with enough contrast against its background is easier to read for everyone.",
        "",
    ),
    (
        0,
        "red",
        "Avoid red formatting",
        "Red text or fill alone does not tell readers who cannot see colours what it means.",
        "",
    ),
    (
        1,
        "altText",
        "Missing alt text",
        "Alternative text tells readers who cannot see a chart or picture what it shows.",
        "Select Object",
    ),
    (
        2,
        "tableHeader",
        "Missing table header",
        "A header row names the columns so that readers can follow the table.",
        "Show Header Row",
    ),
    (
        2,
        "merged",
        "Use of merged cells",
        "Merged cells in a table make it hard to move through the table with a keyboard or a screen reader.",
        "Unmerge Cells",
    ),
    (
        3,
        "linkText",
        "Unclear hyperlink text",
        "Link text that says where it goes is clearer than a bare web address.",
        "Select Cell",
    ),
    (
        3,
        "sheetName",
        "Default sheet name",
        "Meaningful sheet names give context and make moving between sheets easier for everyone.",
        "Rename Sheet",
    ),
    (
        4,
        "restricted",
        "Restricted access",
        "Restrictions on a workbook can keep assistive technology from reading it.",
        "",
    ),
];

/// One issue: its sheet, the cell or range it is about, and what to show.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct AccIssue {
    pub sheet: usize,
    pub range: Option<(u32, u32, u32, u32)>,
    /// Table name (header / merged-in-table issues) or chart index.
    pub item: Option<String>,
}

/// The open accessibility pane.
#[derive(Debug, Clone, Default)]
pub(super) struct AccPane {
    /// Issues of each rule (index of [`RULES`]).
    pub issues: Vec<Vec<AccIssue>>,
    /// Details view: rule and issue shown.
    pub detail: Option<(usize, usize)>,
    /// Content scrolled up by this many reference pixels.
    pub scroll: f32,
    /// Measured text widths (text, style size, width).
    widths: Vec<(String, u32, f32)>,
    /// Content height (reference pixels) of the list view.
    content: f32,
}

/// The open Watch Window: watched cells and the rows shown.
#[derive(Debug, Clone, Default)]
pub(super) struct WatchWin {
    pub rows: Vec<[String; 6]>,
    pub sel: Option<usize>,
    /// Top-left corner (device pixels), dragged by its title.
    pub pos: Option<(f32, f32)>,
    drag: Option<(f32, f32)>,
}

/// Pane width (reference pixels at 125 %, the left edge line included).
const PANE_W: f32 = 400.0;
/// Watch Window size (reference pixels at 125 %).
const WATCH: (f32, f32) = (556.0, 239.0);
/// Watch list columns: left edges (reference pixels from the list's left).
const WATCH_COLS: [f32; 6] = [0.0, 67.0, 134.0, 201.0, 268.0, 403.0];

impl AccPane {
    fn total(&self) -> usize {
        self.issues.iter().map(Vec::len).sum()
    }

    fn width(&self, s: &str, size: f32) -> f32 {
        self.widths
            .iter()
            .find(|(t, z, _)| t == s && *z == size.to_bits())
            .map_or(s.chars().count() as f32 * size * 0.5, |w| w.2)
    }
}

impl SheetWindow {
    /// Reference pixels at 125 % → device pixels.
    fn d(&self, v: f32) -> f32 {
        v * self.scale / 1.25
    }

    /// Width the docked pane takes from the grid (0 when closed).
    pub(super) fn pane_w(&self) -> f32 {
        if self.acc.is_some() {
            self.d(PANE_W).round()
        } else if self.pivot.is_some() {
            self.d(super::pivotpane_impl::PIVOT_W).round()
        } else {
            0.0
        }
    }

    /// The docked pane: left, top, width, height.
    pub(super) fn pane_rect(&self) -> [f32; 4] {
        let w = self.pane_w();
        let top = self.grid_top();
        let bottom = self.size.1 - self.status_h();
        [self.size.0 - w, top, w, (bottom - top).max(0.0)]
    }

    fn style(&self, ref_px: f32, bold: bool) -> TextStyle {
        TextStyle {
            size: self.d(ref_px),
            bold,
        }
    }

    // ------------------------------------------------------------ accessibility checks

    /// Run the accessibility checks on the workbook.
    pub(super) fn acc_check(&self) -> Vec<Vec<AccIssue>> {
        let mut out = vec![Vec::new(); RULES.len()];
        let Some(wb) = self.view.engine().workbook() else {
            return out;
        };
        let default_name = |n: &str| {
            let l = n.to_lowercase();
            ["sheet", "trang tính"].iter().any(|p| {
                l.strip_prefix(p)
                    .is_some_and(|d| d.trim().parse::<u32>().is_ok())
            })
        };
        let rng =
            |r: &bo_model::sheet::CellRange| (r.first.row, r.first.col, r.last.row, r.last.col);
        for (si, ws) in wb.sheets.iter().enumerate() {
            for (ci, ch) in ws.charts.iter().enumerate() {
                if ch.title.as_deref().unwrap_or("").is_empty() {
                    out[2].push(AccIssue {
                        sheet: si,
                        range: None,
                        item: Some(ci.to_string()),
                    });
                }
            }
            for tb in &ws.tables {
                if tb.header_row_count == Some(0) {
                    out[3].push(AccIssue {
                        sheet: si,
                        range: Some(rng(&tb.range)),
                        item: Some(tb.display_name.clone()),
                    });
                }
                // Merged cells inside a table (as the reference flags them).
                let t = rng(&tb.range);
                for m in &ws.merges {
                    let m = rng(m);
                    if m.0 <= t.2 && m.2 >= t.0 && m.1 <= t.3 && m.3 >= t.1 {
                        out[4].push(AccIssue {
                            sheet: si,
                            range: Some(m),
                            item: Some(tb.display_name.clone()),
                        });
                    }
                }
            }
            for h in &ws.hyperlinks {
                // A link whose text is its bare address.
                let Some(target) = h.target.as_deref() else {
                    continue;
                };
                let shown = self
                    .view
                    .engine()
                    .cell(si, h.range.first.row, h.range.first.col)
                    .map(|c| c.display)
                    .unwrap_or_default();
                let bare = |s: &str| {
                    s.trim()
                        .trim_start_matches("https://")
                        .trim_start_matches("http://")
                        .trim_end_matches('/')
                        .to_lowercase()
                };
                if !shown.is_empty() && bare(&shown) == bare(target) {
                    out[5].push(AccIssue {
                        sheet: si,
                        range: Some(rng(&h.range)),
                        item: None,
                    });
                }
            }
            if default_name(&ws.name) && !ws.cells.is_empty() {
                out[6].push(AccIssue {
                    sheet: si,
                    range: None,
                    item: Some(ws.name.clone()),
                });
            }
        }
        out
    }

    /// Issues in the whole workbook (status bar).
    pub(super) fn acc_issue_count(&self) -> usize {
        // Default sheet names are a tip in the pane only: the reference's status bar stays
        // "Good to go" on a workbook whose sheets keep their default names (round 61).
        self.acc_check()
            .iter()
            .enumerate()
            .filter(|(rule, _)| *rule != DEFAULT_NAME_RULE)
            .map(|(_, v)| v.len())
            .sum()
    }

    /// Review › Check Accessibility: open (or close) the pane.
    pub(super) fn toggle_accessibility_pane(&mut self) -> bool {
        if self.acc.is_some() {
            self.acc = None;
        } else {
            self.acc = Some(AccPane {
                issues: self.acc_check(),
                ..AccPane::default()
            });
        }
        self.pane_changed = true;
        true
    }

    /// After input: run the checks again, measure the pane's texts.
    pub(super) fn layout_panes(&mut self, m: &dyn TextMeasure) {
        if self.acc.is_some() {
            let issues = self.acc_check();
            let texts = self.acc_texts(&issues);
            if let Some(p) = self.acc.as_mut() {
                // A details view whose issue was fixed goes back to the list.
                if let Some((r, i)) = p.detail {
                    if issues.get(r).is_none_or(Vec::is_empty) {
                        p.detail = None;
                    } else if i >= issues[r].len() {
                        p.detail = Some((r, issues[r].len() - 1));
                    }
                }
                p.issues = issues;
            }
            let mut widths = Vec::new();
            for (t, size, bold) in texts {
                let st = self.style(size, bold);
                if !widths
                    .iter()
                    .any(|(s, z, _): &(String, u32, f32)| *s == t && *z == st.size.to_bits())
                {
                    let w = m.measure(&t, st).0;
                    widths.push((t, st.size.to_bits(), w));
                }
            }
            let content = Self::acc_content_height();
            if let Some(p) = self.acc.as_mut() {
                p.widths = widths;
                p.content = content;
            }
        }
        if self.watch.is_some() {
            self.watch_refresh();
        }
    }

    /// Texts the pane measures: (text, reference px size, bold).
    fn acc_texts(&self, issues: &[Vec<AccIssue>]) -> Vec<(String, f32, bool)> {
        let mut out = Vec::new();
        let total: usize = issues.iter().map(Vec::len).sum();
        let (head, msg) = self.acc_status_texts(total);
        out.push((head, 28.0 / 1.1, true));
        for w in msg.split_whitespace() {
            out.push((w.to_owned(), 20.0 / 1.1, false));
        }
        out.push((" ".to_owned(), 20.0 / 1.1, false));
        for v in issues {
            if !v.is_empty() {
                out.push((v.len().to_string(), 21.8 / 1.1, false));
            }
        }
        out.push((self.t("sheetAcc.title", "Accessibility"), 30.0 / 1.1, false));
        out
    }

    fn acc_status_texts(&self, total: usize) -> (String, String) {
        if total == 0 {
            (
                self.t("sheetAcc.good", "Good to go!"),
                self.t(
                    "sheetAcc.goodMsg",
                    "No accessibility issues found. People with disabilities should not have difficulty reading this workbook.",
                ),
            )
        } else {
            (
                self.t("sheetAcc.keepGoing", "Keep going!"),
                self.t(
                    "sheetAcc.keepGoingMsg",
                    "Fix the remaining issues in the document to make it accessible to everyone.",
                ),
            )
        }
    }

    /// Height of the list view's content (reference pixels from the pane's top).
    fn acc_content_height() -> f32 {
        let mut y = 252.0;
        for (ci, _) in CATS.iter().enumerate() {
            let n = RULES.iter().filter(|r| r.0 == ci).count() as f32;
            y += 38.0 + 33.0 + n * 56.0;
        }
        y + 30.0
    }

    // ------------------------------------------------------------ input

    /// Pointer / wheel input over the pane or the Watch Window: `Some(redraw)` when taken.
    pub(super) fn pane_input(&mut self, input: &Input, text: &dyn TextMeasure) -> Option<bool> {
        if let Some(r) = self.watch_input(input, text) {
            return Some(r);
        }
        if self.acc.is_none() && self.pivot.is_none() {
            return None;
        }
        let r = self.pane_rect();
        let inside = |x: f32, y: f32| x >= r[0] && x < r[0] + r[2] && y >= r[1] && y < r[1] + r[3];
        match *input {
            Input::Move { x, y } => {
                self.pane_ptr = inside(x, y);
                self.pane_ptr.then_some(false)
            }
            Input::Wheel(_) if self.pane_ptr && self.acc.is_none() => Some(false),
            Input::Wheel(d) if self.pane_ptr => {
                let max = {
                    let p = self.acc.as_ref()?;
                    (p.content - r[3] / (self.scale / 1.25)).max(0.0)
                };
                if let Some(p) = self.acc.as_mut() {
                    p.scroll = (p.scroll - d * 56.0).clamp(0.0, max);
                }
                Some(true)
            }
            Input::Press { x, y, .. } if inside(x, y) && self.acc_bar_press(x, y) => Some(true),
            Input::Press { x, y, .. } if inside(x, y) => {
                let lx = (x - r[0]) / (self.scale / 1.25);
                let ly = (y - r[1]) / (self.scale / 1.25);
                if self.acc.is_none() {
                    return Some(self.pivot_press(lx, ly));
                }
                Some(self.acc_press(lx, ly, text))
            }
            // A field dragged out of the PivotTable Fields pane: inside, it moves; dropped
            // outside, it is taken out of the report (as the reference does).
            Input::Release { x, y } if self.acc.is_none() && self.pivot_drag.is_some() => {
                if inside(x, y) {
                    let lx = (x - r[0]) / (self.scale / 1.25);
                    let ly = (y - r[1]) / (self.scale / 1.25);
                    Some(self.pivot_release(lx, ly))
                } else {
                    let (from, f, ..) = self.pivot_drag.take()?;
                    Some(self.pivot_item_command("remove", from, f))
                }
            }
            Input::Drag { .. } if self.acc.is_none() && self.pivot_drag.is_some() => Some(false),
            Input::Drag { x, y } | Input::Release { x, y } if inside(x, y) => Some(false),
            _ => None,
        }
    }

    /// A press at (lx, ly) reference pixels from the pane's top-left corner.
    fn acc_press(&mut self, lx: f32, ly: f32, text: &dyn TextMeasure) -> bool {
        // Close button.
        if (355.0..390.0).contains(&lx) && (10.0..46.0).contains(&ly) {
            self.acc = None;
            self.pane_changed = true;
            return true;
        }
        let Some(p) = self.acc.clone() else {
            return false;
        };
        if let Some((rule, i)) = p.detail {
            let n = p.issues.get(rule).map_or(0, Vec::len);
            if (82.0..122.0).contains(&ly) {
                if lx < 50.0 {
                    self.acc_set_detail(None);
                } else if (300.0..340.0).contains(&lx) && i > 0 {
                    self.acc_set_detail(Some((rule, i - 1)));
                } else if lx >= 340.0 && i + 1 < n {
                    self.acc_set_detail(Some((rule, i + 1)));
                }
                return true;
            }
            // The fix button.
            if (41.0..361.0).contains(&lx) && (343.0..391.0).contains(&ly) {
                self.acc_fix(rule, i, text);
                return true;
            }
            return false;
        }
        // Rows of the cards.
        let y = ly + p.scroll;
        let mut top = 252.0;
        for (ci, _) in CATS.iter().enumerate() {
            top += 38.0 + 33.0;
            for (ri, r) in RULES.iter().enumerate().filter(|(_, r)| r.0 == ci) {
                let _ = r;
                if (17.0..362.0).contains(&lx) && y >= top && y < top + 56.0 {
                    if p.issues.get(ri).is_some_and(|v| !v.is_empty()) {
                        self.acc_set_detail(Some((ri, 0)));
                    }
                    return true;
                }
                top += 56.0;
            }
        }
        false
    }

    /// The pane's scroll bar: track (arrows included) and thumb, device pixels; `None` when
    /// the list fits.
    pub(super) fn acc_bar(&self) -> Option<([f32; 4], [f32; 4])> {
        let pane = self.acc.as_ref().filter(|p| p.detail.is_none())?;
        let r = self.pane_rect();
        let k = self.scale / 1.25;
        let view = r[3] / k - 60.0;
        let max = pane.content - 60.0 - view;
        if max <= 0.0 {
            return None;
        }
        let w = (19.0 * k).round();
        let track = [
            (r[0] + 371.0 * k).round(),
            (r[1] + 77.0 * k).round(),
            w,
            (r[3] - 89.0 * k).round(),
        ];
        let room = track[3] - 2.0 * w;
        let len = (room * view / (pane.content - 60.0))
            .max(20.0 * k)
            .min(room);
        let y = track[1] + w + (room - len) * (pane.scroll / max).clamp(0.0, 1.0);
        Some((track, [track[0], y.round(), w, len.round()]))
    }

    /// A press on the pane's scroll bar: arrows move a row, the track pages.
    fn acc_bar_press(&mut self, x: f32, y: f32) -> bool {
        let Some((t, th)) = self.acc_bar() else {
            return false;
        };
        if x < t[0] || x >= t[0] + t[2] || y < t[1] || y >= t[1] + t[3] {
            return false;
        }
        let k = self.scale / 1.25;
        let page = (self.pane_rect()[3] / k - 120.0).max(56.0);
        let step = if y < t[1] + t[2] {
            -56.0
        } else if y >= t[1] + t[3] - t[2] {
            56.0
        } else if y < th[1] {
            -page
        } else if y >= th[1] + th[3] {
            page
        } else {
            0.0
        };
        let view = self.pane_rect()[3] / k - 60.0;
        if let Some(p) = self.acc.as_mut() {
            let max = (p.content - 60.0 - view).max(0.0);
            p.scroll = (p.scroll + step).clamp(0.0, max);
        }
        true
    }

    /// Scroll the pane's list to `px` reference pixels (scripted captures).
    pub(super) fn pane_scroll_to(&mut self, px: f32) {
        if let Some(p) = self.acc.as_mut() {
            p.scroll = px.clamp(0.0, p.content.max(0.0));
        }
    }

    /// Open the details of rule `rule`'s first issue (scripted captures).
    pub(super) fn pane_detail(&mut self, rule: usize) {
        if self
            .acc
            .as_ref()
            .and_then(|p| p.issues.get(rule))
            .is_some_and(|v| !v.is_empty())
        {
            self.acc_set_detail(Some((rule, 0)));
        }
    }

    /// Show the details of an issue (and select what it is about) or the list.
    fn acc_set_detail(&mut self, d: Option<(usize, usize)>) {
        if let Some(p) = self.acc.as_mut() {
            p.detail = d;
        }
        let Some((rule, i)) = d else {
            return;
        };
        let Some(issue) = self
            .acc
            .as_ref()
            .and_then(|p| p.issues.get(rule))
            .and_then(|v| v.get(i))
            .cloned()
        else {
            return;
        };
        if issue.sheet != self.view.sheet() {
            self.view.set_sheet(issue.sheet);
        }
        if let Some((r0, c0, r1, c1)) = issue.range {
            self.view.select((r0, c0), (r1, c1), (r0, c0));
            self.view.scroll_to_cell(r0, c0);
        }
    }

    /// The fix button of a details view.
    fn acc_fix(&mut self, rule: usize, i: usize, text: &dyn TextMeasure) {
        let Some(issue) = self
            .acc
            .as_ref()
            .and_then(|p| p.issues.get(rule))
            .and_then(|v| v.get(i))
            .cloned()
        else {
            return;
        };
        match RULES[rule].1 {
            "sheetName" => {
                self.view.set_sheet(issue.sheet);
                let _ = self.tabs_action("tab.rename", text);
            }
            "merged" => {
                if let Some((r0, c0, r1, c1)) = issue.range {
                    self.view.run(json!({
                        "cmd": "unmerge", "sheet": issue.sheet,
                        "range": { "r0": r0, "c0": c0, "r1": r1, "c1": c1 },
                    }));
                }
            }
            "tableHeader" => {
                self.acc_set_detail(Some((rule, i)));
                let _ = self.table_action("sheet.tblHeaderRow", text);
            }
            _ => self.acc_set_detail(Some((rule, i))),
        }
    }

    // ------------------------------------------------------------ painting

    /// The docked pane.
    pub(super) fn paint_acc_pane(&self, p: &mut dyn Painter) {
        let Some(pane) = &self.acc else {
            return;
        };
        let r = self.pane_rect();
        let k = self.scale / 1.25;
        let at = |x: f32, y: f32| ((r[0] + x * k).round(), (r[1] + y * k).round());
        p.fill_rect(r[0], r[1], r[2], r[3], PANE_BG);
        p.fill_rect(r[0], r[1], 1.0, r[3], PANE_EDGE);
        p.push_clip(r[0], r[1], r[2], r[3]);
        // Title and close button.
        let title = self.t("sheetAcc.title", "Accessibility");
        let ts = self.style(30.0 / 1.1, false);
        let (tx, ty) = at(16.0, 12.0);
        p.text(&title, tx, ty, ts, INK);
        let tw = pane.width(&title, ts.size);
        // Chevron after the title (the pane's menu, as the reference).
        let (cx, cy) = (tx + tw + 12.0 * k, r[1] + 26.0 * k);
        p.path(
            &[
                PathSeg::M(cx, cy),
                PathSeg::L(cx + 6.0 * k, cy + 6.0 * k),
                PathSeg::L(cx + 12.0 * k, cy),
            ],
            None,
            Some((INK, k.max(1.0))),
        );
        let (xx, xy) = at(366.0, 22.0);
        let s = 11.0 * k;
        p.path(
            &[
                PathSeg::M(xx, xy),
                PathSeg::L(xx + s, xy + s),
                PathSeg::M(xx + s, xy),
                PathSeg::L(xx, xy + s),
            ],
            None,
            Some((INK, k.max(1.0))),
        );
        if let Some((rule, i)) = pane.detail {
            self.paint_acc_detail(p, pane, rule, i, &at);
        } else {
            p.push_clip(r[0], r[1] + 60.0 * k, r[2], r[3] - 60.0 * k);
            self.paint_acc_list(p, pane, &|x, y| at(x, y - pane.scroll));
            p.pop_clip();
            // The reference's classic scroll bar on the pane's right edge (measured: x 371,
            // 19 px wide, from y 77 to 12 px above the bottom): arrow boxes and a thumb
            // framed #949494 on a white track.
            if let Some((track, thumb)) = self.acc_bar() {
                let edge = Rgba(0x94, 0x94, 0x94, 255);
                let [x, y0, w, h] = track;
                p.fill_rect(x, y0, w, h, edge);
                p.fill_rect(x + 1.0, y0 + 1.0, w - 2.0, h - 2.0, WHITE);
                p.fill_rect(x, thumb[1], w, thumb[3], edge);
                p.fill_rect(x + 2.0, thumb[1] + 1.0, w - 4.0, thumb[3] - 2.0, WHITE);
                let a = 3.5 * k;
                let cx = x + w / 2.0;
                for (cy, up) in [(y0 + w / 2.0, true), (y0 + h - w / 2.0, false)] {
                    let d = if up { -a } else { a };
                    p.path(
                        &[
                            PathSeg::M(cx - a, cy - d / 2.0),
                            PathSeg::L(cx + a, cy - d / 2.0),
                            PathSeg::L(cx, cy + d / 2.0),
                            PathSeg::Z,
                        ],
                        Some(INK),
                        None,
                    );
                }
            }
        }
        p.pop_clip();
    }

    fn paint_acc_list(
        &self,
        p: &mut dyn Painter,
        pane: &AccPane,
        at: &dyn Fn(f32, f32) -> (f32, f32),
    ) {
        let k = self.scale / 1.25;
        let total = pane.total();
        // Status card: 2 px black frame, the heading in green, the message centred.
        let (x0, y0) = at(17.0, 77.0);
        let (w, h) = ((345.0 * k).round(), (175.0 * k).round());
        p.fill_rect(x0, y0, w, h, BLACK);
        let b = (2.0 * k).round().max(1.0);
        p.fill_rect(x0 + b, y0 + b, w - 2.0 * b, h - 2.0 * b, WHITE);
        let (head, msg) = self.acc_status_texts(total);
        let hs = self.style(28.0 / 1.1, true);
        let hw = pane.width(&head, hs.size);
        p.text(
            &head,
            (x0 + (w - hw) / 2.0).round(),
            y0 + 26.0 * k,
            hs,
            GREEN,
        );
        let ms = self.style(20.0 / 1.1, false);
        let space = pane.width(" ", ms.size);
        let mut lines: Vec<(String, f32)> = Vec::new();
        let max = w - 50.0 * k;
        for word in msg.split_whitespace() {
            let ww = pane.width(word, ms.size);
            match lines.last_mut() {
                Some((l, lw)) if *lw + space + ww <= max => {
                    l.push(' ');
                    l.push_str(word);
                    *lw += space + ww;
                }
                _ => lines.push((word.to_owned(), ww)),
            }
        }
        for (i, (l, lw)) in lines.iter().enumerate() {
            p.text(
                l,
                (x0 + (w - lw) / 2.0).round(),
                (y0 + 74.0 * k + i as f32 * 24.0 * k).round(),
                ms,
                INK,
            );
        }
        // Categories.
        let heading = self.style(21.0 / 1.1, true);
        let item = self.style(21.8 / 1.1, false);
        let mut top = 252.0;
        for (ci, (key, fallback)) in CATS.iter().enumerate() {
            let (hx, hy) = at(17.0, top + 31.0);
            p.text(&self.t(key, fallback), hx, hy, heading, INK);
            top += 38.0 + 33.0;
            let rules: Vec<usize> = (0..RULES.len()).filter(|i| RULES[*i].0 == ci).collect();
            let (cx, cy) = at(17.0, top);
            let ch = (rules.len() as f32 * 56.0 * k).round();
            p.fill_rect(cx, cy, w, ch, CARD_EDGE);
            p.fill_rect(cx + 1.0, cy + 1.0, w - 2.0, ch - 2.0, WHITE);
            for (n, ri) in rules.iter().enumerate() {
                let (_, ry) = at(0.0, top);
                if n > 0 {
                    p.fill_rect(cx, ry, w, 1.0, CARD_EDGE);
                }
                let (_, id, title, ..) = RULES[*ri];
                let count = pane.issues.get(*ri).map_or(0, Vec::len);
                let (ix, iy) = at(38.0, top + 15.0);
                p.text(
                    &self.t(&format!("sheetAcc.{id}"), title),
                    ix,
                    iy,
                    item,
                    if count > 0 { INK } else { PASSED },
                );
                if count > 0 {
                    let c = count.to_string();
                    let (rx, _) = at(342.0, 0.0);
                    p.text(&c, (rx - pane.width(&c, item.size)).round(), iy, item, INK);
                } else {
                    let (gx, gy) = at(332.0, top + 28.0);
                    paint_check(p, gx, gy, 10.0 * k);
                }
                top += 56.0;
            }
        }
    }

    fn paint_acc_detail(
        &self,
        p: &mut dyn Painter,
        pane: &AccPane,
        rule: usize,
        i: usize,
        at: &dyn Fn(f32, f32) -> (f32, f32),
    ) {
        let k = self.scale / 1.25;
        let n = pane.issues.get(rule).map_or(0, Vec::len);
        let (cat, id, title, why, fix) = RULES[rule];
        // ← "1 of 2 - Document Structure" ‹ ›
        let (ax, ay) = at(16.0, 102.0);
        let a = 9.0 * k;
        p.path(
            &[
                PathSeg::M(ax + 2.0 * a, ay),
                PathSeg::L(ax, ay),
                PathSeg::M(ax + a * 0.8, ay - a * 0.8),
                PathSeg::L(ax, ay),
                PathSeg::L(ax + a * 0.8, ay + a * 0.8),
            ],
            None,
            Some((INK, k.max(1.0))),
        );
        let nav = self.style(20.0 / 1.1, false);
        let label = format!(
            "{} {} {} - {}",
            i + 1,
            self.t("sheetAcc.of", "of"),
            n,
            self.t(CATS[cat].0, CATS[cat].1)
        );
        let (lx, ly) = at(56.0, 89.0);
        p.text(&label, lx, ly, nav, INK);
        for (x, left) in [(322.0, true), (362.0, false)] {
            let (px, py) = at(x, 102.0);
            let s = 6.0 * k;
            let (dx, ink) = (if left { s } else { -s }, INK);
            p.path(
                &[
                    PathSeg::M(px + dx, py - s),
                    PathSeg::L(px, py),
                    PathSeg::L(px + dx, py + s),
                ],
                None,
                Some((ink, k.max(1.0))),
            );
        }
        // The card.
        let (x0, y0) = at(21.0, 141.0);
        let w = (360.0 * k).round();
        p.fill_rect(x0, y0, w, (298.0 * k).round(), WHITE);
        let hs = self.style(26.0 / 1.1, true);
        p.text(
            &self.t(&format!("sheetAcc.{id}"), title),
            x0 + 20.0 * k,
            y0 + 16.0 * k,
            hs,
            INK,
        );
        let ds = self.style(20.0 / 1.1, false);
        let reason = self.t(&format!("sheetAcc.{id}Why"), why);
        for (n, l) in wrap_chars(&reason, 36).iter().enumerate() {
            p.text(
                l,
                x0 + 20.0 * k,
                y0 + (58.0 + n as f32 * 24.0) * k,
                ds,
                DESC,
            );
        }
        if !fix.is_empty() {
            p.text(
                &self.t("sheetAcc.resolve", "To resolve try"),
                x0 + 20.0 * k,
                y0 + 162.0 * k,
                ds,
                DESC,
            );
            let (bx, by) = at(41.0, 343.0);
            let (bw, bh) = ((320.0 * k).round(), (48.0 * k).round());
            p.fill_rect(bx, by, bw, bh, BLACK);
            let b = (2.0 * k).round().max(1.0);
            p.fill_rect(
                bx + b,
                by + b,
                bw - 2.0 * b,
                bh - 2.0 * b,
                Rgba(0xF5, 0xF5, 0xF5, 255),
            );
            p.text(
                &self.t(&format!("sheetAcc.{id}Fix"), fix),
                bx + 50.0 * k,
                by + 12.0 * k,
                ds,
                INK,
            );
            // Own pencil mark before the label.
            let (ix, iy) = (bx + 20.0 * k, by + 32.0 * k);
            p.path(
                &[
                    PathSeg::M(ix, iy),
                    PathSeg::L(ix + 14.0 * k, iy - 14.0 * k),
                    PathSeg::L(ix + 17.0 * k, iy - 11.0 * k),
                    PathSeg::L(ix + 3.0 * k, iy + 3.0 * k),
                    PathSeg::Z,
                ],
                None,
                Some((INK, k.max(1.0))),
            );
        }
    }

    /// Status bar item: "Accessibility: Investigate" / "Good to go" (x, style).
    pub(super) fn acc_status_text(&self) -> String {
        if self.acc_issues_cached > 0 {
            self.t("sheetAcc.statusInvestigate", "Accessibility: Investigate")
        } else {
            self.t("sheetAcc.statusGood", "Accessibility: Good to go")
        }
    }

    // ------------------------------------------------------------ Watch Window

    /// Formulas › Watch Window: show or hide it (the watched cells come from the workbook).
    pub(super) fn toggle_watch_window(&mut self) -> bool {
        if self.watch.is_some() {
            self.watch = None;
        } else {
            self.watch = Some(WatchWin::default());
            self.watch_refresh();
        }
        true
    }

    fn watched_cells(&mut self) -> Vec<Value> {
        self.view
            .query(json!({ "cmd": "cellWatches" }))
            .and_then(|v| v.as_array().cloned())
            .unwrap_or_default()
    }

    /// Read the watched cells' rows again.
    pub(super) fn watch_refresh(&mut self) {
        let cells = self.watched_cells();
        let rows = self
            .view
            .query(json!({ "cmd": "watchValues", "cells": cells }))
            .and_then(|v| v.as_array().cloned())
            .unwrap_or_default();
        let s = |v: &Value, k: &str| v.get(k).and_then(Value::as_str).unwrap_or("").to_owned();
        let rows: Vec<[String; 6]> = rows
            .iter()
            .map(|r| {
                [
                    s(r, "book"),
                    s(r, "sheet"),
                    s(r, "name"),
                    s(r, "cell"),
                    s(r, "value"),
                    s(r, "formula"),
                ]
            })
            .collect();
        if let Some(w) = self.watch.as_mut() {
            if w.sel.is_some_and(|i| i >= rows.len()) {
                w.sel = None;
            }
            w.rows = rows;
        }
    }

    /// Add Watch: the selected cells (each one) are watched.
    pub(super) fn watch_add_selection(&mut self) {
        let mut cells = self.watched_cells();
        let sheet = self.view.sheet();
        let (r0, c0, r1, c1) = self.view.range_bounds();
        for r in r0..=r1.min(r0 + 63) {
            for c in c0..=c1.min(c0 + 63) {
                let cell = json!({ "sheet": sheet, "row": r, "col": c });
                if !cells.contains(&cell) {
                    cells.push(cell);
                }
            }
        }
        let _ = self
            .view
            .query(json!({ "cmd": "setCellWatches", "cells": cells }));
        self.watch_refresh();
    }

    fn watch_delete(&mut self) {
        let Some(i) = self.watch.as_ref().and_then(|w| w.sel) else {
            return;
        };
        let mut cells = self.watched_cells();
        if i < cells.len() {
            cells.remove(i);
        }
        let _ = self
            .view
            .query(json!({ "cmd": "setCellWatches", "cells": cells }));
        if let Some(w) = self.watch.as_mut() {
            w.sel = None;
        }
        self.watch_refresh();
    }

    /// The Watch Window's rectangle (device pixels).
    pub(super) fn watch_rect(&self) -> Option<[f32; 4]> {
        let w = self.watch.as_ref()?;
        let (ww, wh) = (self.d(WATCH.0).round(), self.d(WATCH.1).round());
        let (x, y) = w.pos.unwrap_or_else(|| {
            let g = self.view.grid_frame();
            (
                (g[0] + g[2] - ww - self.d(40.0)).max(0.0).round(),
                (g[1] + self.d(60.0)).round(),
            )
        });
        Some([x, y, ww, wh])
    }

    fn watch_input(&mut self, input: &Input, _text: &dyn TextMeasure) -> Option<bool> {
        let r = self.watch_rect()?;
        let k = self.scale / 1.25;
        let inside = |x: f32, y: f32| x >= r[0] && x < r[0] + r[2] && y >= r[1] && y < r[1] + r[3];
        match *input {
            Input::Press { x, y, .. } if inside(x, y) => {
                let (lx, ly) = ((x - r[0]) / k, (y - r[1]) / k);
                if ly < 48.0 && lx > WATCH.0 - 32.0 {
                    self.watch = None;
                } else if ly < 48.0 {
                    if let Some(w) = self.watch.as_mut() {
                        w.drag = Some((x - r[0], y - r[1]));
                    }
                } else if (52.0..80.0).contains(&ly) && (8.0..128.0).contains(&lx) {
                    self.watch_add_selection();
                } else if (52.0..80.0).contains(&ly) && (128.0..256.0).contains(&lx) {
                    self.watch_delete();
                } else if ly >= 107.0 {
                    let i = ((ly - 107.0) / 21.0) as usize;
                    if let Some(w) = self.watch.as_mut() {
                        w.sel = (i < w.rows.len()).then_some(i);
                    }
                }
                Some(true)
            }
            Input::Drag { x, y } => {
                let off = self.watch.as_ref()?.drag?;
                if let Some(w) = self.watch.as_mut() {
                    w.pos = Some(((x - off.0).round(), (y - off.1).round()));
                }
                Some(true)
            }
            Input::Release { .. } if self.watch.as_ref()?.drag.is_some() => {
                if let Some(w) = self.watch.as_mut() {
                    w.drag = None;
                }
                Some(true)
            }
            Input::Move { x, y } | Input::Release { x, y } if inside(x, y) => Some(false),
            _ => None,
        }
    }

    /// The Watch Window over the grid.
    pub(super) fn paint_watch(&self, p: &mut dyn Painter) {
        let (Some(w), Some(r)) = (self.watch.as_ref(), self.watch_rect()) else {
            return;
        };
        let k = self.scale / 1.25;
        let at = |x: f32, y: f32| ((r[0] + x * k).round(), (r[1] + y * k).round());
        // Frame #C6C6C6, face #E6E6E6, a soft shadow.
        p.fill_rect(r[0] + 3.0, r[1] + 3.0, r[2], r[3], Rgba(0, 0, 0, 40));
        p.fill_rect(r[0], r[1], r[2], r[3], Rgba(0xC6, 0xC6, 0xC6, 255));
        p.fill_rect(r[0] + 1.0, r[1] + 1.0, r[2] - 2.0, r[3] - 2.0, PANE_BG);
        let title = self.style(30.0 / 1.1, false);
        let (tx, ty) = at(14.0, 8.0);
        p.text(
            &self.t("sheetFn.watchTitle", "Watch Window"),
            tx,
            ty,
            title,
            INK,
        );
        // ˅ and × at the top right.
        let s = 6.0 * k;
        let (vx, vy) = at(WATCH.0 - 48.0, 16.0);
        p.path(
            &[
                PathSeg::M(vx, vy),
                PathSeg::L(vx + s, vy + s),
                PathSeg::L(vx + 2.0 * s, vy),
            ],
            None,
            Some((INK, k.max(1.0))),
        );
        let (xx, xy) = at(WATCH.0 - 24.0, 12.0);
        let s = 10.0 * k;
        p.path(
            &[
                PathSeg::M(xx, xy),
                PathSeg::L(xx + s, xy + s),
                PathSeg::M(xx + s, xy),
                PathSeg::L(xx, xy + s),
            ],
            None,
            Some((INK, k.max(1.0))),
        );
        // Toolbar: Add Watch… (an own eye-and-plus mark), Delete Watch (dim without a row).
        let ui = self.style(14.0 / 1.1 * 1.0, false);
        let ui = TextStyle {
            size: self.d(16.0 / 1.1),
            ..ui
        };
        let (ax, ay) = at(36.0, 55.0);
        p.text(&self.t("sheetFn.addWatch", "Add Watch..."), ax, ay, ui, INK);
        let (ix, iy) = at(14.0, 58.0);
        paint_watch_mark(p, ix, iy, k, false);
        let dim = w.sel.is_none();
        let (dx, dy) = at(158.0, 55.0);
        p.text(
            &self.t("sheetFn.deleteWatch", "Delete Watch"),
            dx,
            dy,
            ui,
            if dim { PASSED } else { INK },
        );
        let (jx, jy) = at(134.0, 58.0);
        paint_watch_mark(p, jx, jy, k, true);
        // The list: #646464 frame, white, a header row, rows 21 px.
        let (lx, ly) = at(8.0, 84.0);
        let (lw, lh) = ((540.0 * k).round(), (148.0 * k).round());
        p.fill_rect(lx, ly, lw, lh, Rgba(0x64, 0x64, 0x64, 255));
        p.fill_rect(lx + 1.0, ly + 1.0, lw - 2.0, lh - 2.0, WHITE);
        let cols = [
            ("sheetFn.colBook", "Book"),
            ("sheetFn.colSheet", "Sheet"),
            ("sheetFn.colName", "Name"),
            ("sheetFn.colCell", "Cell"),
            ("sheetFn.colValue", "Value"),
            ("sheetFn.colFormula", "Formula"),
        ];
        let list = TextStyle {
            size: self.d(14.5 / 1.1),
            bold: false,
        };
        for (i, (key, d)) in cols.iter().enumerate() {
            let (cx, cy) = at(8.0 + WATCH_COLS[i], 88.0);
            if i > 0 {
                p.fill_rect(cx - 1.0, ly + 1.0, 1.0, (22.0 * k).round(), CARD_EDGE);
            }
            p.text(&self.t(key, d), cx + 6.0 * k, cy, list, INK);
        }
        p.push_clip(lx + 1.0, ly + 1.0, lw - 2.0, lh - 2.0);
        for (n, row) in w.rows.iter().enumerate() {
            let (_, ry) = at(0.0, 107.0 + n as f32 * 21.0);
            if w.sel == Some(n) {
                p.fill_rect(
                    lx + 1.0,
                    ry,
                    lw - 2.0,
                    (21.0 * k).round(),
                    Rgba(0xCC, 0xE8, 0xFF, 255),
                );
            }
            for (i, cell) in row.iter().enumerate() {
                let (cx, _) = at(8.0 + WATCH_COLS[i], 0.0);
                let width = WATCH_COLS.get(i + 1).map_or(540.0, |n| *n) - WATCH_COLS[i];
                let shown = clip_chars(cell, ((width - 8.0) / 7.5) as usize);
                p.text(&shown, cx + 6.0 * k, ry + 1.0 * k, list, INK);
            }
        }
        p.pop_clip();
    }
}

/// A green check in a filled circle centred at (cx, cy) of radius `r`.
fn paint_check(p: &mut dyn Painter, cx: f32, cy: f32, r: f32) {
    let k = r * 0.552_284_8;
    p.path(
        &[
            PathSeg::M(cx + r, cy),
            PathSeg::C(cx + r, cy + k, cx + k, cy + r, cx, cy + r),
            PathSeg::C(cx - k, cy + r, cx - r, cy + k, cx - r, cy),
            PathSeg::C(cx - r, cy - k, cx - k, cy - r, cx, cy - r),
            PathSeg::C(cx + k, cy - r, cx + r, cy - k, cx + r, cy),
            PathSeg::Z,
        ],
        Some(GREEN),
        None,
    );
    let s = r / 10.0;
    p.path(
        &[
            PathSeg::M(cx - 4.5 * s, cy),
            PathSeg::L(cx - 1.2 * s, cy + 3.2 * s),
            PathSeg::L(cx + 4.8 * s, cy - 3.4 * s),
        ],
        None,
        Some((WHITE, (2.0 * s).max(1.0))),
    );
}

/// Own Watch Window toolbar marks: an eye with a plus (add) or a cross (delete).
fn paint_watch_mark(p: &mut dyn Painter, x: f32, y: f32, k: f32, delete: bool) {
    let ink = if delete {
        PASSED
    } else {
        Rgba(0x1E, 0x6E, 0xC8, 255)
    };
    let (w, h) = (16.0 * k, 8.0 * k);
    let cy = y + 8.0 * k;
    p.path(
        &[
            PathSeg::M(x, cy),
            PathSeg::Q(x + w / 2.0, cy - h, x + w, cy),
            PathSeg::Q(x + w / 2.0, cy + h, x, cy),
            PathSeg::Z,
        ],
        None,
        Some((ink, k.max(1.0))),
    );
    p.fill_rect(x + w / 2.0 - 2.0 * k, cy - 2.0 * k, 4.0 * k, 4.0 * k, ink);
    let (mx, my) = (x + w + 2.0 * k, y + 12.0 * k);
    let s = 3.0 * k;
    if delete {
        p.path(
            &[
                PathSeg::M(mx - s, my - s),
                PathSeg::L(mx + s, my + s),
                PathSeg::M(mx + s, my - s),
                PathSeg::L(mx - s, my + s),
            ],
            None,
            Some((ink, k.max(1.0))),
        );
    } else {
        p.fill_rect(mx - s, my - k / 2.0, 2.0 * s, k.max(1.0), ink);
        p.fill_rect(mx - k / 2.0, my - s, k.max(1.0), 2.0 * s, ink);
    }
}

/// At most `n` characters (with an ellipsis).
fn clip_chars(s: &str, n: usize) -> String {
    if s.chars().count() <= n.max(1) {
        s.to_owned()
    } else {
        let mut out: String = s.chars().take(n.max(2) - 1).collect();
        out.push('…');
        out
    }
}

/// Words wrapped at `n` characters a line.
fn wrap_chars(s: &str, n: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut cur = String::new();
    for w in s.split_whitespace() {
        if !cur.is_empty() && cur.chars().count() + 1 + w.chars().count() > n {
            lines.push(std::mem::take(&mut cur));
        }
        if !cur.is_empty() {
            cur.push(' ');
        }
        cur.push_str(w);
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    lines
}
