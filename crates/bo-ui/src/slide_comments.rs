//! BlueSlide's Comments pane (N6, round 2): the threads of the current slide on the right
//! of the editing area, as the reference presentation app shows them — a header with New,
//! one card per thread (author, date, text, replies), Resolve / Delete on each card, and a
//! box to type a new comment (Enter posts, Escape cancels).
use crate::{Key, Painter, TextMeasure, TextStyle};
use bo_engine::slide_edit::comments::ThreadOut;
use bo_render::Rgba;

// Measured on the reference at 125 %: pane 342 px wide on the canvas grey behind a 191
// splitter, cards 14 px in, light grey (240) with a 210 frame.
const PANE_BG: Rgba = Rgba(0xe6, 0xe6, 0xe6, 255);
const SPLIT: Rgba = Rgba(0xbf, 0xbf, 0xbf, 255);
const EDGE: Rgba = Rgba(0xd2, 0xd2, 0xd2, 255);
const CARD: Rgba = Rgba(0xf0, 0xf0, 0xf0, 255);
const DIM: Rgba = Rgba(0x60, 0x60, 0x60, 255);
const LINK: Rgba = Rgba(0x1f, 0x5f, 0xa8, 255);

/// What a click in the pane does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaneHit {
    /// Start typing a new comment.
    New,
    /// Close the pane.
    Close,
    /// Resolve (or reopen) thread `id`.
    Resolve(String, bool),
    /// Delete thread `id`.
    Delete(String),
}

/// What typing in the pane asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaneEdit {
    /// The text changed.
    Changed,
    /// Post this comment.
    Post(String),
    /// Typing was cancelled.
    Cancel,
}

/// The pane's state and layout.
#[derive(Debug, Default)]
pub struct CommentsPane {
    /// Shown on the right of the editing area.
    pub shown: bool,
    /// The new comment being typed.
    pub draft: Option<String>,
    rect: [f32; 4],
    scale: f32,
    /// Click targets laid out by the last [`CommentsPane::record`].
    pub(crate) hits: std::cell::RefCell<Vec<([f32; 4], PaneHit)>>,
    /// Drawing recorded with text measured, replayed by [`CommentsPane::paint`].
    ops: Vec<Op>,
}

#[derive(Debug, Clone)]
enum Op {
    Rect([f32; 4], Rgba),
    Text(String, f32, f32, TextStyle, Rgba),
    Path(Vec<bo_render::PathSeg>, Option<Rgba>, Option<(Rgba, f32)>),
}

/// A painter that keeps what is drawn.
struct Recorder(Vec<Op>);

impl Painter for Recorder {
    fn clear(&mut self, _: Rgba) {}
    fn fill_rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: Rgba) {
        self.0.push(Op::Rect([x, y, w, h], color));
    }
    fn page(&mut self, _: usize, _: i32, _: i32) {}
    fn text(&mut self, text: &str, x: f32, y: f32, style: TextStyle, color: Rgba) {
        self.0.push(Op::Text(text.to_owned(), x, y, style, color));
    }
    fn path(
        &mut self,
        segs: &[bo_render::PathSeg],
        fill: Option<Rgba>,
        stroke: Option<(Rgba, f32)>,
    ) {
        self.0.push(Op::Path(segs.to_vec(), fill, stroke));
    }
    fn push_clip(&mut self, _: f32, _: f32, _: f32, _: f32) {}
    fn pop_clip(&mut self) {}
}

impl CommentsPane {
    /// Width of the pane (device pixels).
    pub fn width(scale: f32) -> f32 {
        (273.6 * scale).round()
    }

    /// Place the pane.
    pub fn layout(&mut self, rect: [f32; 4], scale: f32) {
        self.rect = rect;
        self.scale = scale;
    }

    /// Whether a window position is inside the pane.
    pub fn contains(&self, x: f32, y: f32) -> bool {
        let [rx, ry, rw, rh] = self.rect;
        self.shown && x >= rx && x < rx + rw && y >= ry && y < ry + rh
    }

    fn px(&self, v: f32) -> f32 {
        (v * self.scale).round()
    }

    /// What a click at a window position hits.
    pub fn press(&self, x: f32, y: f32) -> Option<PaneHit> {
        self.hits
            .borrow()
            .iter()
            .find(|([hx, hy, hw, hh], _)| x >= *hx && x < hx + hw && y >= *hy && y < hy + hh)
            .map(|(_, h)| h.clone())
    }

    /// Typing while a new comment is open.
    pub fn text(&mut self, s: &str) -> Option<PaneEdit> {
        let d = self.draft.as_mut()?;
        d.push_str(s);
        Some(PaneEdit::Changed)
    }

    /// Keys while a new comment is open.
    pub fn key(&mut self, k: Key) -> Option<PaneEdit> {
        let d = self.draft.as_mut()?;
        Some(match k {
            Key::Backspace => {
                d.pop();
                PaneEdit::Changed
            }
            Key::Enter => {
                let t = self.draft.take().unwrap_or_default();
                if t.trim().is_empty() {
                    PaneEdit::Cancel
                } else {
                    PaneEdit::Post(t)
                }
            }
            Key::Escape => {
                self.draft = None;
                PaneEdit::Cancel
            }
            _ => return None,
        })
    }

    /// Draw the pane as last recorded.
    pub fn paint(&self, p: &mut dyn Painter) {
        if !self.shown {
            return;
        }
        let [x, y, w, h] = self.rect;
        p.push_clip(x, y, w, h);
        for op in &self.ops {
            match op {
                Op::Rect([x, y, w, h], c) => p.fill_rect(*x, *y, *w, *h, *c),
                Op::Text(t, x, y, s, c) => p.text(t, *x, *y, *s, *c),
                Op::Path(segs, f, st) => p.path(segs, *f, *st),
            }
        }
        p.pop_clip();
    }

    /// Lay the pane out with `threads` of the current slide (text measured now).
    pub fn record(
        &mut self,
        text: &dyn TextMeasure,
        threads: &[ThreadOut],
        labels: &[String; 7],
        ink: Rgba,
    ) {
        let mut rec = Recorder(Vec::new());
        self.draw(&mut rec, text, threads, labels, ink, true);
        self.ops = rec.0;
    }

    #[allow(clippy::too_many_lines)]
    fn draw(
        &self,
        p: &mut dyn Painter,
        text: &dyn TextMeasure,
        threads: &[ThreadOut],
        labels: &[String; 7],
        ink: Rgba,
        caret: bool,
    ) {
        let [x, y, w, h] = self.rect;
        let mut hits = Vec::new();
        p.fill_rect(x, y, w, h, PANE_BG);
        p.fill_rect(x, y, 1.0, h, SPLIT);
        let pad = self.px(11.2);
        let title = TextStyle {
            size: self.px(16.0),
            bold: false,
        };
        let body = TextStyle {
            size: self.px(12.0),
            bold: false,
        };
        let bold = TextStyle {
            size: self.px(12.0),
            bold: true,
        };
        let line = text.measure("Ag", body).1.max(self.px(14.0));
        // Header: title, close cross, New button.
        p.text(&labels[0], x + pad, y + pad, title, ink);
        let cs = self.px(20.0);
        let close = [x + w - pad - cs, y + pad, cs, cs];
        let k = self.px(5.0);
        let (cx, cy) = (close[0] + cs / 2.0, close[1] + cs / 2.0);
        p.path(
            &[
                bo_render::PathSeg::M(cx - k, cy - k),
                bo_render::PathSeg::L(cx + k, cy + k),
                bo_render::PathSeg::M(cx + k, cy - k),
                bo_render::PathSeg::L(cx - k, cy + k),
            ],
            None,
            Some((ink, self.scale.max(1.0).round())),
        );
        hits.push((close, PaneHit::Close));
        let mut cy = y + pad + self.px(30.0);
        let (nw, _) = text.measure(&labels[1], body);
        let new = [x + pad, cy, nw + self.px(20.0), self.px(26.0)];
        p.fill_rect(new[0], new[1], new[2], new[3], EDGE);
        p.fill_rect(new[0] + 1.0, new[1] + 1.0, new[2] - 2.0, new[3] - 2.0, CARD);
        p.text(
            &labels[1],
            new[0] + self.px(10.0),
            new[1] + (new[3] - line) / 2.0,
            body,
            ink,
        );
        hits.push((new, PaneHit::New));
        cy += new[3] + pad;
        let inner = w - 2.0 * pad - self.px(16.0);
        // The comment being typed.
        if let Some(d) = &self.draft {
            let rows = wrap(d, inner, text, body);
            let bh = (rows.len().max(1) as f32) * line + self.px(16.0);
            p.fill_rect(x + pad, cy, w - 2.0 * pad, bh, LINK);
            p.fill_rect(x + pad + 1.0, cy + 1.0, w - 2.0 * pad - 2.0, bh - 2.0, CARD);
            let mut ty = cy + self.px(8.0);
            if d.is_empty() {
                p.text(&labels[6], x + pad + self.px(8.0), ty, body, DIM);
            }
            let mut end_x = x + pad + self.px(8.0);
            for r in &rows {
                p.text(r, x + pad + self.px(8.0), ty, body, ink);
                end_x = x + pad + self.px(8.0) + text.measure(r, body).0;
                ty += line;
            }
            if caret {
                let top = if rows.is_empty() { ty } else { ty - line };
                p.fill_rect(end_x.round(), top, 1.0, line, ink);
            }
            cy += bh + pad;
        }
        if threads.is_empty() && self.draft.is_none() {
            p.text(&labels[2], x + pad, cy, body, DIM);
        }
        for t in threads {
            if cy > y + h {
                break;
            }
            let top = cy;
            // Card height first (author line, wrapped text, links), for its background.
            let mut card_h = self.px(8.0) + line + self.px(8.0);
            for (i, c) in t.comments.iter().enumerate() {
                let indent = if i == 0 { 0.0 } else { self.px(16.0) };
                let rows = wrap(&c.text, inner - indent, text, body).len() as f32;
                card_h += (1.0 + rows) * line + self.px(4.0);
            }
            p.fill_rect(x + pad, top, w - 2.0 * pad, card_h, CARD);
            let mut ty = cy + self.px(8.0);
            for (i, c) in t.comments.iter().enumerate() {
                let indent = if i == 0 { 0.0 } else { self.px(16.0) };
                let tx = x + pad + self.px(8.0) + indent;
                p.text(&c.author, tx, ty, bold, ink);
                let date: String = c.date.chars().take(10).collect();
                let (aw, _) = text.measure(&c.author, bold);
                p.text(&date, tx + aw + self.px(8.0), ty, body, DIM);
                ty += line;
                for r in wrap(&c.text, inner - indent, text, body) {
                    p.text(&r, tx, ty, body, if t.done { DIM } else { ink });
                    ty += line;
                }
                ty += self.px(4.0);
            }
            // Resolve / Reopen and Delete links.
            let tx = x + pad + self.px(8.0);
            let resolve = if t.done { &labels[4] } else { &labels[3] };
            let (rw, _) = text.measure(resolve, body);
            let (dw, _) = text.measure(&labels[5], body);
            p.text(resolve, tx, ty, body, LINK);
            p.text(&labels[5], tx + rw + self.px(16.0), ty, body, LINK);
            hits.push(([tx, ty, rw, line], PaneHit::Resolve(t.id.clone(), !t.done)));
            hits.push((
                [tx + rw + self.px(16.0), ty, dw, line],
                PaneHit::Delete(t.id.clone()),
            ));
            ty += line + self.px(8.0);
            // Card frame drawn around what was laid out.
            let card_h = ty - top;
            p.fill_rect(x + pad, top, w - 2.0 * pad, 1.0, EDGE);
            p.fill_rect(x + pad, top + card_h, w - 2.0 * pad, 1.0, EDGE);
            p.fill_rect(x + pad, top, 1.0, card_h, EDGE);
            p.fill_rect(x + w - pad - 1.0, top, 1.0, card_h, EDGE);
            cy = ty + pad;
        }
        *self.hits.borrow_mut() = hits;
    }
}

/// The current UTC time as ISO 8601 (`2026-11-15T07:30:00Z`), `None` without a clock.
pub fn now_iso() -> Option<String> {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs();
    iso_of(secs)
}

/// `secs` after 1970-01-01 UTC as ISO 8601.
fn iso_of(secs: u64) -> Option<String> {
    let days = i64::try_from(secs / 86_400).ok()?;
    let rem = secs % 86_400;
    // Civil date from days since 1970-01-01 (proleptic Gregorian calendar).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    Some(format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    ))
}

/// Break `s` into lines no wider than `width` (words, then characters).
fn wrap(s: &str, width: f32, text: &dyn TextMeasure, style: TextStyle) -> Vec<String> {
    let mut out = Vec::new();
    for para in s.split('\n') {
        let mut cur = String::new();
        for word in para.split(' ') {
            let trial = if cur.is_empty() {
                word.to_owned()
            } else {
                format!("{cur} {word}")
            };
            if text.measure(&trial, style).0 <= width || cur.is_empty() {
                cur = trial;
            } else {
                out.push(std::mem::take(&mut cur));
                cur = word.to_owned();
            }
        }
        out.push(cur);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::iso_of;

    #[test]
    fn iso_dates() {
        assert_eq!(iso_of(0).as_deref(), Some("1970-01-01T00:00:00Z"));
        assert_eq!(
            iso_of(1_763_200_000).as_deref(),
            Some("2025-11-15T09:46:40Z")
        );
        assert_eq!(iso_of(951_782_400).as_deref(), Some("2000-02-29T00:00:00Z"));
    }
}
