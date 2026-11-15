//! Proofing pane of BlueDoc (Review › Spelling & Grammar, F7, the status bar's book), laid
//! out after a desktop word processor's: on the right of the window, the flagged word in its
//! sentence, suggestions to click, Ignore Once / Ignore All / Add to Dictionary.
use crate::strings::Strings;
use crate::theme::Theme;
use crate::{Painter, TextMeasure, TextStyle};
use bo_engine::proof::ProofIssue;
use bo_render::{PathSeg, Rgba};

/// Measured on the reference at 125 %: pane 438 px on grey 230 with a 191 edge; context and
/// suggestion boxes white framed 171; labels 118 grey; list text dark.
const PANE_BG: Rgba = Rgba(230, 230, 230, 255);
const PANE_EDGE: Rgba = Rgba(191, 191, 191, 255);
const BOX_EDGE: Rgba = Rgba(171, 171, 171, 255);
const MUTED: Rgba = Rgba(118, 118, 118, 255);
const WHITE: Rgba = Rgba(255, 255, 255, 255);

/// What a press in the pane asks the window to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SpellAction {
    Close,
    /// Replace the flagged word by suggestion `n`.
    Change(usize),
    IgnoreOnce,
    IgnoreAll,
    AddToDictionary,
    Redraw,
}

const COMMANDS: [(SpellAction, &str); 3] = [
    (SpellAction::IgnoreOnce, "spell.ignoreOnce"),
    (SpellAction::IgnoreAll, "spell.ignoreAll"),
    (SpellAction::AddToDictionary, "spell.addToDictionary"),
];

/// The proofing pane.
pub(crate) struct SpellPane {
    /// Issues of the main text, in document order.
    pub(crate) issues: Vec<ProofIssue>,
    /// The issue shown.
    pub(crate) index: usize,
    scale: f32,
    rect: [f32; 4],
    hover: Option<usize>,
    /// The sentence of the current issue wrapped in its box: (text, x, line, flagged, width).
    context: Vec<(String, f32, usize, bool, f32)>,
}

fn inside(r: [f32; 4], x: f32, y: f32) -> bool {
    x >= r[0] && x < r[0] + r[2] && y >= r[1] && y < r[1] + r[3]
}

impl SpellPane {
    pub(crate) fn new(issues: Vec<ProofIssue>) -> Self {
        Self {
            issues,
            index: 0,
            scale: 1.0,
            rect: [0.0; 4],
            hover: None,
            context: Vec::new(),
        }
    }

    fn context_style(&self, bold: bool) -> TextStyle {
        TextStyle {
            size: self.px(11.0),
            bold,
        }
    }

    /// Wrap the current issue's sentence in the context box with measured words.
    pub(crate) fn measure(&mut self, text: &dyn TextMeasure) {
        self.context.clear();
        let Some(issue) = self.current().cloned() else {
            return;
        };
        let width = self.context_rect()[2] - self.px(16.0);
        let space = text.measure(" ", self.context_style(false)).0;
        let mut words: Vec<(String, bool)> = issue
            .before
            .split_whitespace()
            .map(|w| (w.to_owned(), false))
            .collect();
        // The flagged text sticks to the words around it when there is no space between.
        let glued_before = !issue.before.is_empty() && !issue.before.ends_with(char::is_whitespace);
        let glued_after = !issue.after.is_empty() && !issue.after.starts_with(char::is_whitespace);
        let flagged = words.len();
        words.push((issue.word.clone(), true));
        words.extend(
            issue
                .after
                .split_whitespace()
                .map(|w| (w.to_owned(), false)),
        );
        let (mut x, mut line) = (0.0_f32, 0_usize);
        for (i, (w, bold)) in words.into_iter().enumerate() {
            let ww = text.measure(&w, self.context_style(bold)).0;
            let glued = (i == flagged && glued_before) || (i == flagged + 1 && glued_after);
            let gap = if x == 0.0 || glued { 0.0 } else { space };
            if x > 0.0 && x + gap + ww > width {
                line += 1;
                x = 0.0;
            } else {
                x += gap;
            }
            if line > 2 {
                break;
            }
            self.context.push((w, x, line, bold, ww));
            x += ww;
        }
    }

    /// Width of the pane at display `scale` (350 logical px, as the reference).
    pub(crate) fn width(scale: f32) -> f32 {
        (350.0 * scale).round()
    }

    fn px(&self, v: f32) -> f32 {
        (v * self.scale).round()
    }

    /// Place the pane at the right edge `right` of the window, from `top` to `bottom`.
    pub(crate) fn layout(&mut self, right: f32, top: f32, bottom: f32, scale: f32) {
        self.scale = scale;
        let w = Self::width(scale);
        self.rect = [right - w, top, w, (bottom - top).max(0.0)];
    }

    /// The issue shown, if any.
    pub(crate) fn current(&self) -> Option<&ProofIssue> {
        self.issues.get(self.index)
    }

    pub(crate) fn contains(&self, x: f32, y: f32) -> bool {
        inside(self.rect, x, y)
    }

    fn close_rect(&self) -> [f32; 4] {
        let s = self.px(32.0);
        [
            self.rect[0] + self.rect[2] - self.px(14.0) - s,
            self.rect[1] + self.px(10.0),
            s,
            s,
        ]
    }

    fn box_x(&self) -> (f32, f32) {
        (self.rect[0] + self.px(13.0), self.rect[2] - self.px(28.0))
    }

    fn context_rect(&self) -> [f32; 4] {
        let (x, w) = self.box_x();
        [x, self.rect[1] + self.px(115.0), w, self.px(58.0)]
    }

    fn row_h(&self) -> f32 {
        self.px(32.0)
    }

    fn suggestions_rect(&self) -> [f32; 4] {
        let (x, w) = self.box_x();
        let n = self
            .current()
            .map_or(0, |i| i.suggestions.len().clamp(1, 6));
        [
            x,
            self.rect[1] + self.px(218.0),
            w,
            n as f32 * self.row_h() + self.px(8.0),
        ]
    }

    fn command_rect(&self, i: usize) -> [f32; 4] {
        let s = self.suggestions_rect();
        let (x, w) = self.box_x();
        [
            x,
            s[1] + s[3] + self.px(10.0) + i as f32 * self.px(28.0),
            w,
            self.px(28.0),
        ]
    }

    /// Hover item: suggestion `n`, or 100 + command `n`.
    fn item_at(&self, x: f32, y: f32) -> Option<usize> {
        let s = self.suggestions_rect();
        if inside(s, x, y) {
            let i = ((y - s[1] - self.px(4.0)) / self.row_h()).floor().max(0.0) as usize;
            let n = self.current().map_or(0, |c| c.suggestions.len().min(6));
            return (i < n).then_some(i);
        }
        (0..COMMANDS.len())
            .find(|i| inside(self.command_rect(*i), x, y))
            .map(|i| 100 + i)
    }

    pub(crate) fn hover(&mut self, x: f32, y: f32) -> bool {
        let h = self.item_at(x, y);
        let changed = h != self.hover;
        self.hover = h;
        changed
    }

    pub(crate) fn press(&mut self, x: f32, y: f32) -> SpellAction {
        if inside(self.close_rect(), x, y) {
            return SpellAction::Close;
        }
        if self.current().is_none() {
            return SpellAction::Redraw;
        }
        match self.item_at(x, y) {
            Some(i) if i >= 100 => COMMANDS[i - 100].0,
            Some(i) => SpellAction::Change(i),
            None => SpellAction::Redraw,
        }
    }

    /// Paint the pane.
    pub(crate) fn paint(&self, p: &mut dyn Painter, t: &Theme, strings: &Strings) {
        let [x, y, w, h] = self.rect;
        if w <= 0.0 || h <= 0.0 {
            return;
        }
        p.push_clip(x, y, w, h);
        p.fill_rect(x, y, w, h, PANE_BG);
        p.fill_rect(x, y, 1.0, h, PANE_EDGE);
        let style = |size: f32, bold: bool| TextStyle {
            size: self.px(size),
            bold,
        };
        let lx = x + self.px(13.0);
        p.text(
            strings.get("spell.checkTitle"),
            lx,
            y + self.px(12.0),
            style(18.0, false),
            t.text,
        );
        let c = self.close_rect();
        let (cx, cy, k) = (c[0] + c[2] / 2.0, c[1] + c[3] / 2.0, self.px(5.0));
        p.path(
            &[
                PathSeg::M(cx - k, cy - k),
                PathSeg::L(cx + k, cy + k),
                PathSeg::M(cx + k, cy - k),
                PathSeg::L(cx - k, cy + k),
            ],
            None,
            Some((t.text, self.scale.max(1.0))),
        );
        let Some(issue) = self.current() else {
            let done = if self.issues.is_empty() {
                "spell.noIssues"
            } else {
                "spell.done"
            };
            p.text(
                strings.get(done),
                lx,
                y + self.px(60.0),
                style(12.0, false),
                t.text,
            );
            p.pop_clip();
            return;
        };
        // Kind of issue, then the sentence with the flagged text in bold.
        p.text(
            strings.get("spell.title"),
            lx,
            y + self.px(52.0),
            style(14.0, false),
            t.text,
        );
        let kind = if issue.grammar {
            "spell.grammarIssue"
        } else {
            "spell.notInDictionary"
        };
        p.text(
            strings.get(kind),
            lx,
            y + self.px(78.0),
            style(12.0, false),
            MUTED,
        );
        let b = self.context_rect();
        p.fill_rect(b[0], b[1], b[2], b[3], BOX_EDGE);
        p.fill_rect(b[0] + 1.0, b[1] + 1.0, b[2] - 2.0, b[3] - 2.0, WHITE);
        p.push_clip(b[0] + 2.0, b[1], b[2] - 4.0, b[3]);
        let color = if issue.grammar {
            Rgba(0x2b, 0x57, 0xd8, 255)
        } else {
            Rgba(0xe8, 0x11, 0x23, 255)
        };
        let line_h = (self.px(11.0) * 1.33).round();
        let (bx, by) = (b[0] + self.px(8.0), b[1] + self.px(6.0));
        for (word, wx, line, flagged, ww) in &self.context {
            let st = self.context_style(*flagged);
            let ty = by + *line as f32 * line_h;
            p.text(word, bx + wx, ty, st, t.text);
            if *flagged {
                // Wavy-line colour under the flagged word.
                p.fill_rect(
                    bx + wx,
                    ty + st.size * 1.25,
                    *ww,
                    self.scale.max(1.0),
                    color,
                );
            }
        }
        p.pop_clip();
        // Suggestions: click one to use it, as on the reference.
        p.text(
            strings.get("spell.suggestions"),
            lx,
            b[1] + b[3] + self.px(22.0),
            style(12.0, false),
            MUTED,
        );
        let s = self.suggestions_rect();
        p.fill_rect(s[0], s[1], s[2], s[3], BOX_EDGE);
        p.fill_rect(s[0] + 1.0, s[1] + 1.0, s[2] - 2.0, s[3] - 2.0, WHITE);
        let rh = self.row_h();
        if issue.suggestions.is_empty() {
            p.text(
                strings.get("spell.noSuggestions"),
                s[0] + self.px(12.0),
                s[1] + self.px(4.0) + (rh - self.px(13.0) * 1.33) / 2.0,
                style(13.0, false),
                MUTED,
            );
        }
        for (i, sug) in issue.suggestions.iter().take(6).enumerate() {
            let ry = s[1] + self.px(4.0) + i as f32 * rh;
            if self.hover == Some(i) {
                p.fill_rect(s[0] + 1.0, ry, s[2] - 2.0, rh, t.hover);
            }
            p.text(
                sug,
                s[0] + self.px(12.0),
                ry + (rh - self.px(13.0) * 1.33) / 2.0,
                style(13.0, true),
                t.text,
            );
        }
        for (i, (_, key)) in COMMANDS.iter().enumerate() {
            let r = self.command_rect(i);
            if self.hover == Some(100 + i) {
                p.fill_rect(r[0], r[1], r[2], r[3], t.hover);
            }
            let st = style(13.0, false);
            p.text(
                strings.get(key),
                r[0] + self.px(12.0),
                r[1] + (r[3] - st.size * 1.33) / 2.0,
                st,
                t.text,
            );
        }
        p.pop_clip();
    }
}
