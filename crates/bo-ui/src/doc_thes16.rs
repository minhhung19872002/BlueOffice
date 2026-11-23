//! Review › Thesaurus (Shift+F7) of BlueDoc (round 16): the task pane at the window's right,
//! laid out as the reference word processor's (measured at 125 %: 250 px wide on #E6E6E6, a
//! 20 pt title with a close cross, a back button and a search box 34 px tall 62 px under the
//! pane's top, the framed list of meanings from 114 px with 27 px rows — a heading "first
//! synonym (part of speech)" with a chevron that folds it, the synonyms under it, antonyms
//! marked — a rule under the list and the language box at the foot). A press on a synonym
//! looks it up (Back returns); its drop-down inserts it in place of the word or copies it.
//! Data: the English thesaurus of `thesaurus10` (Open English WordNet, CC BY 4.0).
use crate::{Input, Key, Painter, TextStyle};
use bo_render::{PathSeg, Rgba};

const BG: Rgba = Rgba(0xE6, 0xE6, 0xE6, 255);
const EDGE: Rgba = Rgba(0xBF, 0xBF, 0xBF, 255);
const FRAME: Rgba = Rgba(0x94, 0x94, 0x94, 255);
const RULE: Rgba = Rgba(0xC6, 0xC6, 0xC6, 255);
const DIM_EDGE: Rgba = Rgba(0xD2, 0xD2, 0xD2, 255);
const WHITE: Rgba = Rgba(255, 255, 255, 255);
const INK: Rgba = Rgba(0x26, 0x26, 0x26, 255);
const DIM: Rgba = Rgba(0xA6, 0xA6, 0xA6, 255);
const ROW_HOVER: Rgba = Rgba(0xE5, 0xF1, 0xFB, 255);

/// A row of the list.
#[derive(Debug, Clone, PartialEq)]
enum Row {
    /// Heading of meaning `i`: text, folded.
    Head(usize, String, bool),
    /// A word: the word, the text shown.
    Word(String, String),
}

/// What a press in the pane asks the window to do.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum ThesAction {
    None,
    Redraw,
    Close,
    /// Open the Insert / Copy menu of a word under rectangle.
    Menu(String, [f32; 4]),
}

/// The Thesaurus pane.
#[derive(Debug, Clone, Default)]
pub(crate) struct ThesPane {
    /// The word looked up.
    pub(crate) word: String,
    /// The text range (body paragraph, byte start, byte end) Insert replaces.
    pub(crate) target: Option<(usize, usize, usize)>,
    back: Vec<String>,
    folded: Vec<usize>,
    scroll: usize,
    hover: Option<usize>,
    /// The search box has the keyboard.
    pub(crate) typing: bool,
    rect: [f32; 4],
    scale: f32,
    /// Placed rows: rectangle and row.
    rows: Vec<([f32; 4], Row)>,
    total: usize,
    fits: usize,
}

impl ThesPane {
    /// Pane width at `scale` (250 px at 125 % on the reference).
    pub(crate) fn width(scale: f32) -> f32 {
        (200.0 * scale).round()
    }

    pub(crate) fn new(word: &str, target: Option<(usize, usize, usize)>) -> Self {
        Self {
            word: word.to_owned(),
            target,
            ..Self::default()
        }
    }

    fn px(&self, v: f32) -> f32 {
        (v * self.scale).round()
    }

    pub(crate) fn contains(&self, x: f32, y: f32) -> bool {
        let r = self.rect;
        x >= r[0] && x < r[0] + r[2] && y >= r[1] && y < r[1] + r[3]
    }

    /// The list rows for the current word.
    fn all_rows(&self) -> Vec<Row> {
        let senses = crate::thesaurus10::senses(&self.word, 40);
        let mut rows = Vec::new();
        for (i, s) in senses.iter().enumerate() {
            let pos = match s.pos.as_str() {
                "v" => "v.",
                "a" | "s" => "adj.",
                "r" => "adv.",
                _ => "n.",
            };
            let head = s
                .synonyms
                .first()
                .map_or(self.word.as_str(), String::as_str);
            let folded = self.folded.contains(&i);
            rows.push(Row::Head(i, format!("{head} ({pos})"), folded));
            if folded {
                continue;
            }
            for w in &s.synonyms {
                rows.push(Row::Word(w.clone(), w.clone()));
            }
            for a in &s.antonyms {
                rows.push(Row::Word(a.clone(), format!("{a} (Antonym)")));
            }
        }
        rows
    }

    /// Place the pane at the right of the window (`right`), between `top` and `bottom`.
    pub(crate) fn layout(&mut self, right: f32, top: f32, bottom: f32, scale: f32) {
        self.scale = scale;
        let w = Self::width(scale);
        self.rect = [right - w, top, w, bottom - top];
        let list = self.list_rect();
        let row = self.px(21.6);
        let rows = self.all_rows();
        self.fits = (((list[3] - 2.0) / row).floor().max(1.0)) as usize;
        self.total = rows.len();
        self.scroll = self.scroll.min(self.total.saturating_sub(self.fits));
        self.rows = rows
            .into_iter()
            .skip(self.scroll)
            .take(self.fits)
            .enumerate()
            .map(|(k, r)| {
                let y = list[1] + 1.0 + k as f32 * row;
                ([list[0] + 1.0, y, list[2] - 2.0 - self.bar_w(), row], r)
            })
            .collect();
    }

    fn bar_w(&self) -> f32 {
        if self.total > self.fits {
            self.px(13.6)
        } else {
            0.0
        }
    }

    fn left(&self) -> f32 {
        self.rect[0] + self.px(12.0)
    }

    fn inner_w(&self) -> f32 {
        self.rect[2] - self.px(24.0)
    }

    fn close_rect(&self) -> [f32; 4] {
        let s = self.px(24.0);
        [
            self.rect[0] + self.rect[2] - s - self.px(4.0),
            self.rect[1] + self.px(12.0),
            s,
            s,
        ]
    }

    fn back_rect(&self) -> [f32; 4] {
        let h = self.px(27.2);
        [self.left(), self.rect[1] + self.px(49.6), h, h]
    }

    fn search_rect(&self) -> [f32; 4] {
        let b = self.back_rect();
        let x = b[0] + b[2] + self.px(8.8);
        [x, b[1], self.left() + self.inner_w() - x, b[3]]
    }

    fn list_rect(&self) -> [f32; 4] {
        let y = self.rect[1] + self.px(91.2);
        let foot = self.rect[1] + self.rect[3] - self.px(54.4);
        let h = self
            .px(310.4)
            .min(foot - self.px(40.0) - y)
            .max(self.px(40.0));
        [self.left(), y, self.inner_w(), h]
    }

    fn lang_rect(&self) -> [f32; 4] {
        [
            self.left(),
            self.rect[1] + self.rect[3] - self.px(37.6),
            self.inner_w(),
            self.px(24.0),
        ]
    }

    /// Look `word` up (the current word goes on the Back list).
    pub(crate) fn look_up(&mut self, word: &str) {
        if word != self.word {
            self.back.push(std::mem::take(&mut self.word));
            self.word = word.to_owned();
            self.folded.clear();
            self.scroll = 0;
            self.hover = None;
        }
    }

    pub(crate) fn hover(&mut self, x: f32, y: f32) -> bool {
        let h = self
            .rows
            .iter()
            .position(|(r, row)| matches!(row, Row::Word(..)) && inside(*r, x, y));
        let changed = h != self.hover;
        self.hover = h;
        changed
    }

    pub(crate) fn wheel(&mut self, dy: f32) -> bool {
        let before = self.scroll;
        let step = if dy > 0.0 { 3 } else { -3 };
        let max = self.total.saturating_sub(self.fits);
        self.scroll = usize::try_from((self.scroll as i64 + step).max(0))
            .unwrap_or(0)
            .min(max);
        self.scroll != before
    }

    /// A press at (`x`, `y`) inside the pane.
    pub(crate) fn press(&mut self, x: f32, y: f32) -> ThesAction {
        if inside(self.close_rect(), x, y) {
            return ThesAction::Close;
        }
        self.typing = inside(self.search_rect(), x, y);
        if self.typing {
            return ThesAction::Redraw;
        }
        if inside(self.back_rect(), x, y) {
            if let Some(w) = self.back.pop() {
                self.word = w;
                self.folded.clear();
                self.scroll = 0;
                return ThesAction::Redraw;
            }
            return ThesAction::None;
        }
        let hit = self.rows.iter().find(|(r, _)| inside(*r, x, y)).cloned();
        match hit {
            Some((_, Row::Head(i, _, _))) => {
                if let Some(k) = self.folded.iter().position(|f| *f == i) {
                    self.folded.remove(k);
                } else {
                    self.folded.push(i);
                }
                ThesAction::Redraw
            }
            Some((r, Row::Word(w, _))) => {
                let d = self.drop_rect(r);
                if inside(d, x, y) {
                    ThesAction::Menu(w, d)
                } else {
                    self.look_up(&w);
                    ThesAction::Redraw
                }
            }
            None => ThesAction::None,
        }
    }

    fn drop_rect(&self, r: [f32; 4]) -> [f32; 4] {
        let dw = self.px(20.0);
        [r[0] + r[2] - dw, r[1], dw, r[3]]
    }

    /// Keys while the search box has the keyboard (`true`: used).
    pub(crate) fn key(&mut self, input: &Input) -> bool {
        if !self.typing {
            return false;
        }
        match input {
            Input::Text(t) => {
                self.word.push_str(t);
                true
            }
            Input::Key(Key::Backspace, _) => {
                self.word.pop();
                true
            }
            Input::Key(Key::Enter, _) => {
                self.typing = false;
                self.folded.clear();
                self.scroll = 0;
                true
            }
            Input::Key(Key::Escape, _) => {
                self.typing = false;
                true
            }
            _ => false,
        }
    }

    #[allow(clippy::too_many_lines)]
    pub(crate) fn paint(&self, p: &mut dyn Painter, labels: &[String; 3]) {
        let [x, y, w, h] = self.rect;
        p.fill_rect(x, y, w, h, BG);
        p.fill_rect(x, y, 1.0, h, EDGE);
        let stroke = self.scale.max(1.0).round();
        // Title and its options chevron, the close cross.
        let title = TextStyle {
            size: self.px(20.0),
            bold: false,
        };
        p.text(&labels[0], self.left(), y + self.px(12.0), title, INK);
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
            Some((INK, stroke)),
        );
        let body = TextStyle {
            size: self.px(12.8),
            bold: false,
        };
        let th = body.size * 1.33;
        let frame = |p: &mut dyn Painter, r: [f32; 4], edge: Rgba| {
            p.fill_rect(r[0], r[1], r[2], r[3], edge);
            p.fill_rect(r[0] + 1.0, r[1] + 1.0, r[2] - 2.0, r[3] - 2.0, WHITE);
        };
        // Back button (dim without history) and the search box with a magnifier.
        let b = self.back_rect();
        let can_back = !self.back.is_empty();
        frame(p, b, if can_back { RULE } else { DIM_EDGE });
        let (bx, by, k) = (b[0] + b[2] / 2.0, b[1] + b[3] / 2.0, self.px(3.6));
        p.path(
            &[
                PathSeg::M(bx + k / 2.0, by - k),
                PathSeg::L(bx - k / 2.0, by),
                PathSeg::L(bx + k / 2.0, by + k),
            ],
            None,
            Some((if can_back { INK } else { DIM }, stroke)),
        );
        let s = self.search_rect();
        frame(p, s, if self.typing { FRAME } else { RULE });
        p.push_clip(s[0], s[1], s[2] - self.px(24.0), s[3]);
        p.text(
            &self.word,
            s[0] + self.px(4.8),
            (s[1] + (s[3] - th) / 2.0).round(),
            body,
            INK,
        );
        p.pop_clip();
        let (mx, my, r) = (s[0] + s[2] - self.px(14.0), by - self.px(1.6), self.px(5.6));
        let n = 16;
        let mut ring: Vec<PathSeg> = (0..=n)
            .map(|i| {
                let t = std::f32::consts::TAU * i as f32 / n as f32;
                let (px, py) = (mx + r * t.cos(), my + r * t.sin());
                if i == 0 {
                    PathSeg::M(px, py)
                } else {
                    PathSeg::L(px, py)
                }
            })
            .collect();
        ring.push(PathSeg::M(mx - r * 0.71, my + r * 0.71));
        ring.push(PathSeg::L(mx - r * 1.75, my + r * 1.75));
        p.path(&ring, None, Some((INK, stroke)));
        // The list.
        let l = self.list_rect();
        frame(p, l, FRAME);
        let tx = l[0] + self.px(15.2);
        if self.rows.is_empty() {
            p.text(
                &labels[1],
                l[0] + self.px(6.4),
                l[1] + self.px(4.0),
                body,
                DIM,
            );
        }
        p.push_clip(l[0] + 1.0, l[1] + 1.0, l[2] - 2.0, l[3] - 2.0);
        for (i, (rr, row)) in self.rows.iter().enumerate() {
            let ty = (rr[1] + (rr[3] - th) / 2.0).round();
            match row {
                Row::Head(_, text, folded) => {
                    let (cx, cy, k) = (l[0] + self.px(7.2), rr[1] + rr[3] / 2.0, self.px(3.6));
                    let segs = if *folded {
                        [
                            PathSeg::M(cx - k / 2.0, cy - k),
                            PathSeg::L(cx + k / 2.0, cy),
                            PathSeg::L(cx - k / 2.0, cy + k),
                        ]
                    } else {
                        [
                            PathSeg::M(cx - k, cy - k / 2.0),
                            PathSeg::L(cx, cy + k / 2.0),
                            PathSeg::L(cx + k, cy - k / 2.0),
                        ]
                    };
                    p.path(&segs, None, Some((INK, stroke)));
                    p.text(text, tx, ty, body, INK);
                }
                Row::Word(_, shown) => {
                    if self.hover == Some(i) {
                        p.fill_rect(rr[0], rr[1], rr[2], rr[3], ROW_HOVER);
                        let d = self.drop_rect(*rr);
                        frame(p, d, RULE);
                        let (ax, ay, k) = (d[0] + d[2] / 2.0, d[1] + d[3] / 2.0, self.px(3.2));
                        p.path(
                            &[
                                PathSeg::M(ax - k, ay - k / 2.0),
                                PathSeg::L(ax + k, ay - k / 2.0),
                                PathSeg::L(ax, ay + k / 2.0),
                                PathSeg::Z,
                            ],
                            Some(INK),
                            None,
                        );
                    }
                    p.text(shown, tx, ty, body, INK);
                }
            }
        }
        p.pop_clip();
        let bw = self.bar_w();
        if bw > 0.0 {
            let track = [l[0] + l[2] - 1.0 - bw, l[1] + 1.0, bw, l[3] - 2.0];
            p.fill_rect(
                track[0],
                track[1],
                track[2],
                track[3],
                Rgba(0xF0, 0xF0, 0xF0, 255),
            );
            let room = track[3] - 2.0 * bw;
            let len = (room * self.fits as f32 / self.total.max(1) as f32)
                .max(self.px(16.0))
                .min(room);
            let max = self.total.saturating_sub(self.fits).max(1);
            let ty = track[1] + bw + (room - len) * self.scroll as f32 / max as f32;
            p.fill_rect(
                track[0] + 2.0,
                ty.round(),
                bw - 4.0,
                len.round(),
                Rgba(0xC2, 0xC2, 0xC2, 255),
            );
            for (cy, up) in [
                (track[1] + bw / 2.0, true),
                (track[1] + track[3] - bw / 2.0, false),
            ] {
                let k = self.px(3.2);
                let d = if up { -k / 2.0 } else { k / 2.0 };
                let cx = track[0] + bw / 2.0;
                p.path(
                    &[
                        PathSeg::M(cx - k, cy - d),
                        PathSeg::L(cx + k, cy - d),
                        PathSeg::L(cx, cy + d),
                        PathSeg::Z,
                    ],
                    Some(Rgba(0x60, 0x60, 0x60, 255)),
                    None,
                );
            }
        }
        // Rule under the list, rule above the language box, the language box.
        p.fill_rect(l[0], l[1] + l[3] + self.px(16.8), l[2], 1.0, RULE);
        let lb = self.lang_rect();
        p.fill_rect(lb[0], lb[1] - self.px(16.8), lb[2], 1.0, RULE);
        frame(p, lb, FRAME);
        p.text(
            &labels[2],
            lb[0] + self.px(4.0),
            (lb[1] + (lb[3] - th) / 2.0).round(),
            body,
            INK,
        );
        let (ax, ay, k) = (
            lb[0] + lb[2] - self.px(10.0),
            lb[1] + lb[3] / 2.0,
            self.px(3.2),
        );
        p.path(
            &[
                PathSeg::M(ax - k, ay - k / 2.0),
                PathSeg::L(ax + k, ay - k / 2.0),
                PathSeg::L(ax, ay + k / 2.0),
                PathSeg::Z,
            ],
            Some(INK),
            None,
        );
    }
}

fn inside(r: [f32; 4], x: f32, y: f32) -> bool {
    x >= r[0] && x < r[0] + r[2] && y >= r[1] && y < r[1] + r[3]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn meanings_fold_and_look_up() {
        let mut t = ThesPane::new("quick", None);
        t.layout(1500.0, 236.0, 1060.0, 1.25);
        assert!(
            matches!(t.rows.first(), Some((_, Row::Head(0, h, false))) if h.ends_with("(adj.)"))
        );
        let n = t.total;
        let r = t.rows[0].0;
        assert_eq!(t.press(r[0] + 5.0, r[1] + 5.0), ThesAction::Redraw);
        t.layout(1500.0, 236.0, 1060.0, 1.25);
        assert!(t.total < n, "folded");
        // A word looks itself up; Back returns.
        let (r, word) = t
            .rows
            .iter()
            .find_map(|(r, row)| match row {
                Row::Word(w, _) => Some((*r, w.clone())),
                Row::Head(..) => None,
            })
            .expect("a word");
        t.press(r[0] + 5.0, r[1] + 5.0);
        assert_eq!(t.word, word);
        t.layout(1500.0, 236.0, 1060.0, 1.25);
        let b = t.back_rect();
        t.press(b[0] + 2.0, b[1] + 2.0);
        assert_eq!(t.word, "quick");
    }
}
