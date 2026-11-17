//! BlueSlide's Accessibility pane, round 19 (N6): Review › Check Accessibility and the status
//! bar's "Accessibility: …" item open the reference's right-hand task pane, measured at 125 %
//! (`show/slide-5x`): 400 px wide, the title with a chevron and ✕, a status card ("Keep
//! going!" in the app's accent / "Good to go!"), the switch "Show only issues on current
//! slide", then the rules in categories — Color and Contrast, Media and Illustrations,
//! Tables, Document Structure, Document Access — as white cards of 55 px rows (grey title
//! and a green check when the rule passes, the count when it does not), and a details view
//! per issue (← back, ‹ › to step, the reason, a fix button). The checks are the engine's
//! (`bo_engine::slide_edit::a11y_check`) and run again after every change.
use super::SlideWindow;
use crate::{Input, Painter, TextMeasure, TextStyle};
use bo_engine::slide_edit::a11y_check::{self, A11yIssue, RULES};
use bo_engine::slide_edit::SlideCommand;
use bo_render::{PathSeg, Rgba};

const PANE_BG: Rgba = Rgba(0xE6, 0xE6, 0xE6, 255);
const PANE_EDGE: Rgba = Rgba(0xBF, 0xBF, 0xBF, 255);
const CARD_EDGE: Rgba = Rgba(0xE0, 0xE0, 0xE0, 255);
const WHITE: Rgba = Rgba(255, 255, 255, 255);
const INK: Rgba = Rgba(0x24, 0x24, 0x24, 255);
const PASSED: Rgba = Rgba(0x99, 0x99, 0x99, 255);
const GREEN: Rgba = Rgba(0x0E, 0x70, 0x0E, 255);
const DESC: Rgba = Rgba(0x61, 0x61, 0x61, 255);
const BLACK: Rgba = Rgba(0, 0, 0, 255);

/// Pane width in reference pixels at 125 %.
pub(super) const PANE_W: f32 = 400.0;
/// Top of the first category heading (reference pixels from the pane's top).
const LIST_TOP: f32 = 335.0;
/// Row height of a rule (one line of text); each more line adds 31.
const ROW: f32 = 55.0;

/// Category names (key, fallback).
const CATS: [(&str, &str); 5] = [
    ("sheetAcc.catColor", "Color and Contrast"),
    ("sheetAcc.catMedia", "Media and Illustrations"),
    ("sheetAcc.catTables", "Tables"),
    ("sheetAcc.catStructure", "Document Structure"),
    ("sheetAcc.catAccess", "Document Access"),
];

/// Rule texts: id, title, reason, fix (fallbacks; keys `slideNative.acc.<id>`, `…Why`,
/// `…Fix`), in [`RULES`] order.
const TEXTS: [(&str, &str, &str, &str); 12] = [
    (
        "contrast",
        "Hard-to-read text contrast",
        "Text with enough contrast against its background is easier to read for everyone.",
        "Select Object",
    ),
    (
        "altText",
        "Missing alt text",
        "Alternative text tells readers who cannot see a picture, chart or diagram what it shows.",
        "Add a Description",
    ),
    (
        "captions",
        "Missing audio or video subtitles",
        "Captions let people who cannot hear a video follow what is said.",
        "Select Object",
    ),
    (
        "tableHeader",
        "Missing table header",
        "A header row names the columns so that readers can follow the table.",
        "Select Object",
    ),
    (
        "merged",
        "Use of merged or split cells",
        "Merged cells make a table hard to move through with a keyboard or a screen reader.",
        "Select Object",
    ),
    (
        "linkText",
        "Unclear hyperlink text",
        "Link text that says where it goes is clearer than a bare web address.",
        "Select Object",
    ),
    (
        "slideTitle",
        "Missing slide title",
        "Slide titles let people find their way through the presentation and move to a slide.",
        "Go to Slide",
    ),
    (
        "dupTitle",
        "Duplicate slide title",
        "Different titles tell slides apart for people who move through them by title.",
        "Go to Slide",
    ),
    (
        "readingOrder",
        "Check reading order",
        "A screen reader reads the objects of a slide in the order of the Selection Pane, not as they are placed.",
        "Go to Slide",
    ),
    (
        "defaultSection",
        "Default section name",
        "Meaningful section names give context and make moving through the slides easier.",
        "Go to Slide",
    ),
    (
        "dupSection",
        "Duplicate section name",
        "Different section names tell the parts of the presentation apart.",
        "Go to Slide",
    ),
    (
        "restricted",
        "Restricted access",
        "Restrictions on a presentation can keep assistive technology from reading it.",
        "",
    ),
];

/// State of the pane.
#[derive(Debug, Clone, Default)]
pub(super) struct A11yPane {
    /// Issues found by the last check (every slide).
    pub issues: Vec<A11yIssue>,
    /// Details view: rule (index of [`RULES`]) and issue shown.
    pub detail: Option<(usize, usize)>,
    /// List scrolled up by this many reference pixels.
    pub scroll: f32,
    /// "Show only issues on current slide".
    pub current_only: bool,
    /// Measured text widths: (text, size bits, width).
    widths: Vec<(String, u32, f32)>,
    /// The pointer is over the pane (wheel scrolls it).
    ptr: bool,
    /// Issue count by the status bar's last check, and the history key it was made at.
    pub status: Option<(u64, usize)>,
    /// When the deck changed since that check (the check waits for a pause in typing).
    stale: Option<std::time::Instant>,
}

impl A11yPane {
    fn width(&self, s: &str, size: f32) -> f32 {
        self.widths
            .iter()
            .find(|(t, z, _)| t == s && *z == size.to_bits())
            .map_or(s.chars().count() as f32 * size * 0.5, |w| w.2)
    }
}

/// Wrap `text` into lines no wider than `max` by the measured `width`.
fn wrap(text: &str, max: f32, width: &dyn Fn(&str) -> f32) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for w in text.split_whitespace() {
        match lines.last_mut() {
            Some(l) if width(&format!("{l} {w}")) <= max => {
                l.push(' ');
                l.push_str(w);
            }
            _ => lines.push(w.to_owned()),
        }
    }
    lines
}

impl SlideWindow {
    /// Reference pixels at 125 % → device pixels.
    fn a9(&self, v: f32) -> f32 {
        v * self.scale / 1.25
    }

    fn a9_style(&self, ref_px: f32, bold: bool) -> TextStyle {
        TextStyle {
            size: self.a9(ref_px),
            bold,
        }
    }

    /// Issues shown: every slide's, or the current slide's.
    pub(super) fn a9_shown(&self) -> Vec<&A11yIssue> {
        let cur = self.view.current();
        self.a11y9
            .issues
            .iter()
            .filter(|i| !self.a11y9.current_only || i.slide == Some(cur))
            .collect()
    }

    /// Issues of rule `r` shown.
    fn a9_rule(&self, r: usize) -> Vec<&A11yIssue> {
        self.a9_shown()
            .into_iter()
            .filter(|i| i.rule == RULES[r].0)
            .collect()
    }

    fn a9_title(&self, r: usize) -> String {
        let (id, title, ..) = TEXTS[r];
        self.t(&format!("slideNative.acc.{id}"), title)
    }

    /// Rule rows of a category with their height (reference px).
    fn a9_rows(&self, cat: usize) -> Vec<(usize, f32)> {
        let item = self.a9_style(21.8 / 1.1, false);
        let max = self.a9(273.0);
        (0..RULES.len())
            .filter(|r| RULES[*r].1 == cat)
            .map(|r| {
                let t = self.a9_title(r);
                let n = wrap(&t, max, &|s| self.a11y9.width(s, item.size)).len();
                (r, ROW + 31.0 * (n.max(1) - 1) as f32)
            })
            .collect()
    }

    fn a9_content(&self) -> f32 {
        let mut y = LIST_TOP;
        for c in 0..CATS.len() {
            y += 41.0 + self.a9_rows(c).iter().map(|r| r.1).sum::<f32>() + 30.0;
        }
        y
    }

    /// The status texts: heading and message.
    fn a9_status_texts(&self, total: usize) -> (String, String) {
        if total == 0 {
            (
                self.t("sheetAcc.good", "Good to go!"),
                self.t(
                    "slideNative.acc.goodMsg",
                    "No accessibility issues found. People with disabilities should not have difficulty reading this presentation.",
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

    /// After input: check again and measure the texts the pane draws.
    pub(super) fn a9_layout(&mut self, m: &dyn TextMeasure) {
        let issues = self
            .view
            .engine()
            .presentation()
            .map(a11y_check::check)
            .unwrap_or_default();
        self.a11y9.issues = issues;
        // A details view whose issue went away goes back to the list.
        if let Some((r, i)) = self.a11y9.detail {
            let n = self.a9_rule(r).len();
            self.a11y9.detail = match n {
                0 => None,
                n => Some((r, i.min(n - 1))),
            };
        }
        let total = self.a9_shown().len();
        let (head, msg) = self.a9_status_texts(total);
        let mut texts: Vec<(String, f32, bool)> = vec![
            (head, 28.0 / 1.1, true),
            (" ".into(), 20.0 / 1.1, false),
            (self.t("sheetAcc.title", "Accessibility"), 30.0 / 1.1, false),
        ];
        let mut words = |s: &str, size: f32| {
            // Prefixes of the words in order (for wrapping) and each count.
            let mut acc = String::new();
            for w in s.split_whitespace() {
                texts.push((w.to_owned(), size, false));
                if acc.is_empty() {
                    acc = w.to_owned();
                } else {
                    acc = format!("{acc} {w}");
                }
                texts.push((acc.clone(), size, false));
            }
        };
        words(&msg, 20.0 / 1.1);
        for (r, (id, _, why, _)) in TEXTS.iter().enumerate() {
            words(&self.a9_title(r), 21.8 / 1.1);
            words(
                &self.t(&format!("slideNative.acc.{id}Why"), why),
                20.0 / 1.1,
            );
        }
        for n in 1..=self.a11y9.issues.len().min(99) {
            texts.push((n.to_string(), 21.8 / 1.1, false));
        }
        let mut widths = Vec::with_capacity(texts.len());
        for (t, size, bold) in texts {
            let st = self.a9_style(size, bold);
            let key = st.size.to_bits();
            // Words wrap greedily, so only prefixes that can still fit are worth measuring;
            // all are short here.
            if !widths
                .iter()
                .any(|(s, z, _): &(String, u32, f32)| *s == t && *z == key)
            {
                let w = m.measure(&t, st).0;
                widths.push((t, key, w));
            }
        }
        self.a11y9.widths = widths;
    }

    /// Issue count for the status bar ("Accessibility: Investigate / Good to go"), checked
    /// again only when the history changes.
    pub(super) fn a9_status_count(&mut self) -> usize {
        let key = self.view.content_rev();
        if let Some((k, n)) = self.a11y9.status {
            if k == key {
                self.a11y9.stale = None;
                return n;
            }
            // Checking a 300-slide deck takes ~15 ms: not on every keystroke. The count is
            // brought up to date once the deck has stayed changed for 0.8 s (the caret's
            // blink brings the next input).
            let since = *self.a11y9.stale.get_or_insert_with(std::time::Instant::now);
            if since.elapsed() < std::time::Duration::from_millis(800) {
                return n;
            }
        }
        self.a11y9.stale = None;
        let n = self
            .view
            .engine()
            .presentation()
            .map_or(0, |p| a11y_check::check(p).len());
        self.a11y9.status = Some((key, n));
        n
    }

    /// The pane's rectangle (the side pane's place).
    fn a9_rect(&self) -> [f32; 4] {
        self.side.rect()
    }

    /// Pointer / wheel input over the pane: `Some(redraw)` when taken.
    pub(super) fn a9_input(&mut self, input: &Input) -> Option<bool> {
        if self.side.kind != super::selpane::SideKind::Accessibility {
            return None;
        }
        let r = self.a9_rect();
        let inside = |x: f32, y: f32| x >= r[0] && x < r[0] + r[2] && y >= r[1] && y < r[1] + r[3];
        let k = self.scale / 1.25;
        match *input {
            Input::Move { x, y } => {
                self.a11y9.ptr = inside(x, y);
                None
            }
            Input::Wheel(d) if self.a11y9.ptr => {
                let max = self.a9_max_scroll();
                self.a11y9.scroll = (self.a11y9.scroll - d * ROW).clamp(0.0, max);
                Some(true)
            }
            Input::Press { x, y, .. } if inside(x, y) && self.a9_bar_press(x, y) => {
                self.area_press = false;
                Some(true)
            }
            Input::Press { x, y, .. } if inside(x, y) => {
                self.area_press = false;
                let (lx, ly) = ((x - r[0]) / k, (y - r[1]) / k);
                Some(self.a9_press(lx, ly))
            }
            Input::Drag { x, y } | Input::Release { x, y } if inside(x, y) => Some(false),
            _ => None,
        }
    }

    /// Most the list scrolls (reference px).
    fn a9_max_scroll(&self) -> f32 {
        let k = self.scale / 1.25;
        (self.a9_content() + 20.0 - self.a9_rect()[3] / k).max(0.0)
    }

    /// The list's classic scroll bar on the pane's right edge (reference: x 371, 19 px
    /// wide, from y 77 to 12 px above the bottom): track and thumb in device px; `None`
    /// when the list fits or a details view shows.
    pub(super) fn a9_bar(&self) -> Option<([f32; 4], [f32; 4])> {
        if self.a11y9.detail.is_some() {
            return None;
        }
        let max = self.a9_max_scroll();
        if max <= 0.0 {
            return None;
        }
        let r = self.a9_rect();
        let k = self.scale / 1.25;
        let w = (19.0 * k).round();
        let track = [
            (r[0] + 371.0 * k).round(),
            (r[1] + 77.0 * k).round(),
            w,
            (r[3] - 89.0 * k).round(),
        ];
        let room = track[3] - 2.0 * w;
        let view = r[3] / k;
        let len = (room * view / (view + max)).max(20.0 * k).min(room);
        let y = track[1] + w + (room - len) * (self.a11y9.scroll / max).clamp(0.0, 1.0);
        Some((track, [track[0], y.round(), w, len.round()]))
    }

    /// A press on the scroll bar: arrows move a row, the track pages.
    fn a9_bar_press(&mut self, x: f32, y: f32) -> bool {
        let Some((t, th)) = self.a9_bar() else {
            return false;
        };
        if x < t[0] || x >= t[0] + t[2] || y < t[1] || y >= t[1] + t[3] {
            return false;
        }
        let k = self.scale / 1.25;
        let page = (self.a9_rect()[3] / k - 120.0).max(ROW);
        let step = if y < t[1] + t[2] {
            -ROW
        } else if y >= t[1] + t[3] - t[2] {
            ROW
        } else if y < th[1] {
            -page
        } else if y >= th[1] + th[3] {
            page
        } else {
            0.0
        };
        let max = self.a9_max_scroll();
        self.a11y9.scroll = (self.a11y9.scroll + step).clamp(0.0, max);
        true
    }

    /// A press at (lx, ly) reference pixels from the pane's top-left corner.
    fn a9_press(&mut self, lx: f32, ly: f32) -> bool {
        // ✕
        if (355.0..390.0).contains(&lx) && (10.0..46.0).contains(&ly) {
            self.side.close();
            self.a11y9.detail = None;
            self.relayout_pending = true;
            return true;
        }
        if let Some((rule, i)) = self.a11y9.detail {
            let n = self.a9_rule(rule).len();
            if (82.0..122.0).contains(&ly) {
                if lx < 50.0 {
                    self.a11y9.detail = None;
                } else if (300.0..340.0).contains(&lx) && i > 0 {
                    self.a9_detail(Some((rule, i - 1)));
                } else if lx >= 340.0 && i + 1 < n {
                    self.a9_detail(Some((rule, i + 1)));
                }
                return true;
            }
            if (41.0..361.0).contains(&lx) && (343.0..391.0).contains(&ly) {
                self.a9_fix(rule, i);
                return true;
            }
            return false;
        }
        // The switch row.
        if (282.0..322.0).contains(&ly) && (17.0..363.0).contains(&lx) {
            self.a11y9.current_only = !self.a11y9.current_only;
            self.a11y9.scroll = 0.0;
            self.relayout_pending = true;
            return true;
        }
        let y = ly + self.a11y9.scroll;
        let mut top = LIST_TOP;
        for c in 0..CATS.len() {
            top += 41.0;
            for (r, h) in self.a9_rows(c) {
                if (17.0..363.0).contains(&lx) && y >= top && y < top + h {
                    if !self.a9_rule(r).is_empty() {
                        self.a9_detail(Some((r, 0)));
                    }
                    return true;
                }
                top += h;
            }
            top += 30.0;
        }
        false
    }

    /// Show the details of an issue and go to what it is about.
    pub(super) fn a9_detail(&mut self, d: Option<(usize, usize)>) {
        self.a11y9.detail = d;
        let Some((rule, i)) = d else { return };
        let Some(issue) = self.a9_rule(rule).get(i).map(|x| (*x).clone()) else {
            return;
        };
        if let Some(s) = issue.slide {
            if s != self.view.current() {
                self.view.go_to(s);
            }
        }
        if issue.id != 0 {
            self.view.run(SlideCommand::Select {
                ids: vec![issue.id],
            });
        }
    }

    /// The details view's fix button.
    fn a9_fix(&mut self, rule: usize, i: usize) {
        self.a9_detail(Some((rule, i)));
        if RULES[rule].0 == "altText" {
            let _ = self.action("sl.altText");
        }
    }

    // -------------------------------------------------------------------- painting

    /// The pane over the side pane's place.
    pub(super) fn paint_a9(&self, p: &mut dyn Painter) {
        if self.side.kind != super::selpane::SideKind::Accessibility {
            return;
        }
        let r = self.a9_rect();
        let k = self.scale / 1.25;
        let at = |x: f32, y: f32| ((r[0] + x * k).round(), (r[1] + y * k).round());
        p.fill_rect(r[0], r[1], r[2], r[3], PANE_BG);
        p.fill_rect(r[0], r[1], 1.0, r[3], PANE_EDGE);
        p.push_clip(r[0], r[1], r[2], r[3]);
        let title = self.t("sheetAcc.title", "Accessibility");
        let ts = self.a9_style(30.0 / 1.1, false);
        let (tx, ty) = at(16.0, 12.0);
        p.text(&title, tx, ty, ts, INK);
        let tw = self.a11y9.width(&title, ts.size);
        let (cx, cy) = (tx + tw + 12.0 * k, r[1] + 26.0 * k);
        let pen = Some((INK, k.max(1.0)));
        p.path(
            &[
                PathSeg::M(cx, cy),
                PathSeg::L(cx + 6.0 * k, cy + 6.0 * k),
                PathSeg::L(cx + 12.0 * k, cy),
            ],
            None,
            pen,
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
            pen,
        );
        if let Some((rule, i)) = self.a11y9.detail {
            self.paint_a9_detail(p, rule, i, &at);
        } else {
            p.push_clip(r[0], r[1] + 60.0 * k, r[2], r[3] - 60.0 * k);
            let sc = self.a11y9.scroll;
            self.paint_a9_list(p, &|x, y| at(x, y - sc));
            p.pop_clip();
            if let Some((track, thumb)) = self.a9_bar() {
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

    #[allow(clippy::too_many_lines)]
    fn paint_a9_list(&self, p: &mut dyn Painter, at: &dyn Fn(f32, f32) -> (f32, f32)) {
        let k = self.scale / 1.25;
        let shown = self.a9_shown();
        let total = shown.len();
        let accent = self.theme.accent;
        // Status card: 2 px black frame, the heading in the app's accent (reference: its
        // own accent with issues, green when good), the message centred.
        let (x0, y0) = at(17.0, 76.0);
        let (w, h) = ((346.0 * k).round(), (176.0 * k).round());
        p.fill_rect(x0, y0, w, h, BLACK);
        let b = (2.0 * k).round().max(1.0);
        p.fill_rect(x0 + b, y0 + b, w - 2.0 * b, h - 2.0 * b, WHITE);
        let (head, msg) = self.a9_status_texts(total);
        let hs = self.a9_style(28.0 / 1.1, true);
        let hw = self.a11y9.width(&head, hs.size);
        p.text(
            &head,
            (x0 + (w - hw) / 2.0).round(),
            y0 + 30.0 * k,
            hs,
            if total == 0 { GREEN } else { accent },
        );
        let ms = self.a9_style(20.0 / 1.1, false);
        for (n, l) in wrap(&msg, w - 50.0 * k, &|s| self.a11y9.width(s, ms.size))
            .iter()
            .enumerate()
        {
            let lw = self.a11y9.width(l, ms.size);
            p.text(
                l,
                (x0 + (w - lw) / 2.0).round(),
                (y0 + 74.0 * k + n as f32 * 24.0 * k).round(),
                ms,
                INK,
            );
        }
        // The switch: 48 × 22 rounded track, accent when on, white knob; outlined accent
        // track and a filled accent knob when off (reference).
        let (sx, sy) = at(17.0, 291.0);
        let (sw, sh) = ((48.0 * k).round(), (22.0 * k).round());
        let rr = sh / 2.0;
        let on = self.a11y9.current_only;
        let pill = |x: f32, y: f32, w: f32, h: f32| {
            let r = h / 2.0;
            vec![
                PathSeg::M(x + r, y),
                PathSeg::L(x + w - r, y),
                PathSeg::C(x + w - r * 0.45, y, x + w, y + r * 0.55, x + w, y + r),
                PathSeg::C(
                    x + w,
                    y + h - r * 0.55,
                    x + w - r * 0.45,
                    y + h,
                    x + w - r,
                    y + h,
                ),
                PathSeg::L(x + r, y + h),
                PathSeg::C(x + r * 0.45, y + h, x, y + h - r * 0.55, x, y + r),
                PathSeg::C(x, y + r * 0.55, x + r * 0.45, y, x + r, y),
                PathSeg::Z,
            ]
        };
        if on {
            p.path(&pill(sx, sy, sw, sh), Some(accent), None);
        } else {
            p.path(
                &pill(sx, sy, sw, sh),
                Some(WHITE),
                Some((accent, k.max(1.0))),
            );
        }
        let kr = rr - 5.0 * k;
        let kx = if on { sx + sw - rr } else { sx + rr };
        p.path(
            &pill(kx - kr, sy + rr - kr, 2.0 * kr, 2.0 * kr),
            Some(if on { WHITE } else { accent }),
            None,
        );
        let ls = self.a9_style(20.0 / 1.1, false);
        let (lx, ly) = at(75.0, 290.0);
        p.text(
            &self.t(
                "slideNative.acc.currentOnly",
                "Show only issues on current slide",
            ),
            lx,
            ly,
            ls,
            INK,
        );
        // Categories.
        let heading = self.a9_style(21.0 / 1.1, true);
        let item = self.a9_style(21.8 / 1.1, false);
        let mut top = LIST_TOP;
        for (c, (key, fallback)) in CATS.iter().enumerate() {
            let (hx, hy) = at(17.0, top + 7.0);
            p.text(&self.t(key, fallback), hx, hy, heading, INK);
            top += 41.0;
            let rows = self.a9_rows(c);
            let (cx, cy) = at(17.0, top);
            let ch = (rows.iter().map(|r| r.1).sum::<f32>() * k).round();
            p.fill_rect(cx, cy, w, ch, CARD_EDGE);
            p.fill_rect(cx + 1.0, cy + 1.0, w - 2.0, ch - 2.0, WHITE);
            for (n, (r, rh)) in rows.iter().enumerate() {
                let (_, ry) = at(0.0, top);
                if n > 0 {
                    p.fill_rect(cx, ry, w, 1.0, CARD_EDGE);
                }
                let count = self.a9_rule(*r).len();
                let lines = wrap(&self.a9_title(*r), self.a9(273.0), &|s| {
                    self.a11y9.width(s, item.size)
                });
                for (j, l) in lines.iter().enumerate() {
                    let (ix, iy) = at(38.0, top + 13.0 + 31.0 * j as f32);
                    p.text(l, ix, iy, item, if count > 0 { INK } else { PASSED });
                }
                if count > 0 {
                    let t = count.to_string();
                    let (rx, iy) = at(342.0, top + 13.0);
                    p.text(
                        &t,
                        (rx - self.a11y9.width(&t, item.size)).round(),
                        iy,
                        item,
                        INK,
                    );
                } else {
                    let (gx, gy) = at(331.0, top + rh / 2.0);
                    paint_ok(p, gx, gy, 10.0 * k);
                }
                top += rh;
            }
            top += 30.0;
        }
    }

    fn paint_a9_detail(
        &self,
        p: &mut dyn Painter,
        rule: usize,
        i: usize,
        at: &dyn Fn(f32, f32) -> (f32, f32),
    ) {
        let k = self.scale / 1.25;
        let n = self.a9_rule(rule).len();
        let (id, _, why, fix) = TEXTS[rule];
        let pen = Some((INK, k.max(1.0)));
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
            pen,
        );
        let nav = self.a9_style(20.0 / 1.1, false);
        let cat = RULES[rule].1;
        let label = format!(
            "{} {} {} - {}",
            i + 1,
            self.t("sheetAcc.of", "of"),
            n,
            self.t(CATS[cat].0, CATS[cat].1)
        );
        // Wrapped before the ‹ › buttons, as the reference's "1 of 2 - Document /
        // Structure" (24 px lines).
        let (lx, ly) = at(56.0, 89.0);
        let lines = wrap(&label, self.a9(250.0), &|s| self.a11y9.width(s, nav.size));
        let ly = if lines.len() > 1 { ly - 12.0 * k } else { ly };
        for (n, l) in lines.iter().enumerate() {
            p.text(l, lx, ly + n as f32 * 24.0 * k, nav, INK);
        }
        for (x, left) in [(322.0, true), (362.0, false)] {
            let (px, py) = at(x, 102.0);
            let s = 6.0 * k;
            let dx = if left { s } else { -s };
            p.path(
                &[
                    PathSeg::M(px + dx, py - s),
                    PathSeg::L(px, py),
                    PathSeg::L(px + dx, py + s),
                ],
                None,
                pen,
            );
        }
        let (x0, y0) = at(21.0, 141.0);
        let w = (360.0 * k).round();
        p.fill_rect(x0, y0, w, (298.0 * k).round(), WHITE);
        let hs = self.a9_style(26.0 / 1.1, true);
        p.text(&self.a9_title(rule), x0 + 20.0 * k, y0 + 16.0 * k, hs, INK);
        // What the issue is about.
        let ds = self.a9_style(20.0 / 1.1, false);
        let what = self.a9_rule(rule).get(i).map(|x| {
            let slide = x.slide.map_or(String::new(), |s| {
                format!("{} {}", self.t("slideNative.slideWord", "Slide"), s + 1)
            });
            if x.name.is_empty() {
                slide
            } else {
                format!("{} ({slide})", x.name)
            }
        });
        let mut y = 58.0;
        if let Some(wt) = what {
            p.text(&wt, x0 + 20.0 * k, y0 + y * k, ds, INK);
            y += 32.0;
        }
        let reason = self.t(&format!("slideNative.acc.{id}Why"), why);
        let lines = wrap(&reason, w - 40.0 * k, &|s| self.a11y9.width(s, ds.size));
        for l in &lines {
            p.text(l, x0 + 20.0 * k, y0 + y * k, ds, DESC);
            y += 24.0;
        }
        if !fix.is_empty() {
            p.text(
                &self.t("sheetAcc.resolve", "To resolve try"),
                x0 + 20.0 * k,
                // Under the reason, never over it.
                y0 + (y + 10.0).clamp(162.0, 172.0) * k,
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
                &self.t(&format!("slideNative.acc.{id}Fix"), fix),
                bx + 20.0 * k,
                by + 12.0 * k,
                ds,
                INK,
            );
        }
    }
}

/// A green disc with a white tick (own drawing), centred at (cx, cy).
fn paint_ok(p: &mut dyn Painter, cx: f32, cy: f32, r: f32) {
    let c = 0.552 * r;
    p.path(
        &[
            PathSeg::M(cx, cy - r),
            PathSeg::C(cx + c, cy - r, cx + r, cy - c, cx + r, cy),
            PathSeg::C(cx + r, cy + c, cx + c, cy + r, cx, cy + r),
            PathSeg::C(cx - c, cy + r, cx - r, cy + c, cx - r, cy),
            PathSeg::C(cx - r, cy - c, cx - c, cy - r, cx, cy - r),
            PathSeg::Z,
        ],
        Some(GREEN),
        None,
    );
    p.path(
        &[
            PathSeg::M(cx - r * 0.45, cy + r * 0.02),
            PathSeg::L(cx - r * 0.1, cy + r * 0.38),
            PathSeg::L(cx + r * 0.5, cy - r * 0.3),
        ],
        None,
        Some((WHITE, (r * 0.22).max(1.5))),
    );
}
