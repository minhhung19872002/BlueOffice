//! BlueSlide's Animation Pane (N6, round 3): the effects of the current slide in sequence
//! order on the right of the editing area, as the reference presentation app lists them —
//! a header with Play From / Play All and the reorder arrows, one row per effect (click
//! number, a star in the class colour, the shape's name, a timing bar), the chosen row
//! highlighted. Clicking a row selects its effect (and its shape); the arrows move it
//! earlier or later; Delete removes it.
use crate::{Key, Painter, TextMeasure, TextStyle};
use bo_engine::slide_edit::show::AnimOut;
use bo_render::{PathSeg, Rgba};

// The reference's pane is grey (230) with white-filled buttons framed 171 (round 19).
const PANE_BG: Rgba = Rgba(0xe6, 0xe6, 0xe6, 255);
const BUTTON_EDGE: Rgba = Rgba(0xab, 0xab, 0xab, 255);
const BUTTON_FILL: Rgba = Rgba(0xfd, 0xfd, 0xfd, 255);
const DIM_EDGE: Rgba = Rgba(0xe1, 0xe1, 0xe1, 255);
const SPLIT: Rgba = Rgba(0xbf, 0xbf, 0xbf, 255);
// The chosen rows on a light tint of BlueSlide's accent (the reference tints its own).
const SELECTED: Rgba = Rgba(0xf6, 0xd1, 0xde, 255);
const ENTR: Rgba = Rgba(0x3f, 0xa3, 0x4d, 255);
const EMPH: Rgba = Rgba(0xe8, 0xb3, 0x1e, 255);
const EXIT: Rgba = Rgba(0xd2, 0x4a, 0x3c, 255);
const PATH: Rgba = Rgba(0x5b, 0x8f, 0xd0, 255);

/// What a click in the pane does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimHit {
    /// Close the pane.
    Close,
    /// Play the slide's effects from the selected one (all without a selection).
    Play,
    /// Move the selected effect one place earlier.
    Earlier,
    /// Move it one place later.
    Later,
    /// Choose effect `index`.
    Select(usize),
    /// The drop-down of effect `index`'s row (start, remove; round 17).
    Menu(usize),
}

/// What a key in the pane asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimKey {
    /// Choose another effect.
    Select(usize),
    /// Remove effect `index`.
    Remove(usize),
}

/// The pane's state and layout.
#[derive(Debug, Default)]
pub struct AnimPane {
    /// Shown on the right of the editing area.
    pub shown: bool,
    /// The chosen effect (position in the main sequence).
    pub selected: Option<usize>,
    rect: [f32; 4],
    scale: f32,
    /// Effects listed by the last [`AnimPane::record`].
    count: usize,
    /// Rows added to the chosen one with Shift (round 19: "Play Selected").
    pub extra: Vec<usize>,
    /// Advanced Timeline hidden (the rows' menu toggles it).
    pub hide_timeline: bool,
    /// Click targets laid out by the last [`AnimPane::record`].
    pub(crate) hits: Vec<([f32; 4], AnimHit)>,
    ops: Vec<Op>,
}

#[derive(Debug, Clone)]
enum Op {
    Rect([f32; 4], Rgba),
    Text(String, f32, f32, TextStyle, Rgba),
    Path(Vec<PathSeg>, Option<Rgba>, Option<(Rgba, f32)>),
}

/// A five-pointed star centred on (`cx`, `cy`), outer radius `r`.
fn star(cx: f32, cy: f32, r: f32) -> Vec<PathSeg> {
    let mut segs = Vec::with_capacity(11);
    for k in 0..10 {
        let a = std::f32::consts::PI * (k as f32) / 5.0 - std::f32::consts::FRAC_PI_2;
        let rr = if k % 2 == 0 { r } else { r * 0.45 };
        let (x, y) = (cx + rr * a.cos(), cy + rr * a.sin());
        segs.push(if k == 0 {
            PathSeg::M(x, y)
        } else {
            PathSeg::L(x, y)
        });
    }
    segs.push(PathSeg::Z);
    segs
}

/// Colour of an effect class, as the gallery's stars.
fn class_color(class: &str) -> Rgba {
    match class {
        "entr" => ENTR,
        "emph" => EMPH,
        "exit" => EXIT,
        _ => PATH,
    }
}

impl AnimPane {
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

    /// What a click at a window position hits.
    pub fn press(&self, x: f32, y: f32) -> Option<AnimHit> {
        self.hits
            .iter()
            .rev()
            .find(|([hx, hy, hw, hh], _)| x >= *hx && x < hx + hw && y >= *hy && y < hy + hh)
            .map(|(_, h)| *h)
    }

    /// Keys while the pane has a chosen effect.
    pub fn key(&self, k: Key) -> Option<AnimKey> {
        let i = self.selected?;
        match k {
            Key::Up => Some(AnimKey::Select(i.saturating_sub(1))),
            Key::Down if i + 1 < self.count => Some(AnimKey::Select(i + 1)),
            Key::Delete => Some(AnimKey::Remove(i)),
            _ => None,
        }
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

    /// Lay the pane out for `effects` of the current slide; `labels`: title, Play From,
    /// Play All, Play Selected. Measured on the reference at 125 % (round 19, device px from
    /// the pane's corner): grey ground, the title in large text cut with "…" before its
    /// chevron and ✕, Play From 125 × 40 at (17, 64), the reorder buttons 40 × 40 at 183 and
    /// 226 (a light frame when dimmed), rows of 31 px from y 118, and the Advanced Timeline:
    /// each effect a small box 23 px tall with a 1 px darker frame from its start (delay
    /// within its click) for its duration, 20 px a second from 184 px into the list; an
    /// effect started after the previous one has a hairline down its row at its start.
    #[allow(clippy::too_many_lines)]
    pub fn record(
        &mut self,
        text: &dyn TextMeasure,
        effects: &[AnimOut],
        labels: &[String; 4],
        ink: Rgba,
    ) {
        let mut ops = Vec::new();
        let mut hits = Vec::new();
        let [x, y, w, h] = self.rect;
        self.count = effects.len();
        if self.selected.is_some_and(|s| s >= effects.len()) {
            self.selected = None;
        }
        self.extra.retain(|&i| i < effects.len());
        // Reference px at 125 % → device px.
        let d = |v: f32| (v * self.scale / 1.25).round();
        ops.push(Op::Rect([x, y, w, h], PANE_BG));
        ops.push(Op::Rect([x, y, 1.0, h], SPLIT));
        let title = TextStyle {
            size: d(30.0 / 1.1),
            bold: false,
        };
        let body = TextStyle {
            size: d(20.0 / 1.1),
            bold: false,
        };
        let line = text.measure("Ag", body).1.max(d(18.0));
        // Close cross and the chevron of the pane's menu.
        let cs = d(24.0);
        let close = [x + w - d(30.0), y + d(16.0), cs, cs];
        let k = d(7.0);
        let (cx, cy) = (close[0] + cs / 2.0, close[1] + cs / 2.0);
        let pen = Some((ink, (self.scale / 1.25).max(1.0)));
        ops.push(Op::Path(
            vec![
                PathSeg::M(cx - k, cy - k),
                PathSeg::L(cx + k, cy + k),
                PathSeg::M(cx + k, cy - k),
                PathSeg::L(cx - k, cy + k),
            ],
            None,
            pen,
        ));
        hits.push((close, AnimHit::Close));
        let chev_x = close[0] - d(34.0);
        let t = fit(&labels[0], chev_x - d(10.0) - (x + d(16.0)), text, title);
        ops.push(Op::Text(t, x + d(16.0), y + d(12.0), title, ink));
        let s = d(7.0);
        ops.push(Op::Path(
            vec![
                PathSeg::M(chev_x, cy - s / 2.0),
                PathSeg::L(chev_x + s, cy + s / 2.0),
                PathSeg::L(chev_x + 2.0 * s, cy - s / 2.0),
            ],
            None,
            pen,
        ));
        // Play From / Play All / Play Selected: an outlined triangle and the label.
        let by = y + d(64.0);
        let bh = d(40.0);
        let label = if !self.extra.is_empty() {
            &labels[3]
        } else if self.selected.is_some() {
            &labels[1]
        } else {
            &labels[2]
        };
        let (lw, _) = text.measure(label, body);
        let play = [x + d(17.0), by, (lw + d(62.0)).max(d(125.0)), bh];
        ops.push(Op::Rect(play, BUTTON_EDGE));
        ops.push(Op::Rect(
            [play[0] + 1.0, play[1] + 1.0, play[2] - 2.0, play[3] - 2.0],
            BUTTON_FILL,
        ));
        let (tx, ty) = (play[0] + d(15.0), play[1] + bh / 2.0);
        let tri = d(8.0);
        ops.push(Op::Path(
            vec![
                PathSeg::M(tx, ty - tri),
                PathSeg::L(tx + 1.1 * tri, ty),
                PathSeg::L(tx, ty + tri),
                PathSeg::Z,
            ],
            Some(Rgba(0xb4, 0xb4, 0xb4, 255)),
            Some((Rgba(0x80, 0x80, 0x80, 255), 1.0)),
        ));
        ops.push(Op::Text(
            label.clone(),
            play[0] + d(45.0),
            play[1] + (bh - line) / 2.0,
            body,
            ink,
        ));
        hits.push((play, AnimHit::Play));
        // Move Up / Move Down: square buttons with filled triangles.
        let first = self.first_chosen();
        let last = self.last_chosen();
        for (bx, hit, up) in [
            (183.0, AnimHit::Earlier, true),
            (226.0, AnimHit::Later, false),
        ] {
            let r = [x + d(bx), by, d(40.0), d(40.0)];
            let enabled = match (first, last, up) {
                (Some(f), _, true) => f > 0,
                (_, Some(l), false) => l + 1 < effects.len(),
                _ => false,
            };
            ops.push(Op::Rect(r, if enabled { BUTTON_EDGE } else { DIM_EDGE }));
            ops.push(Op::Rect(
                [r[0] + 1.0, r[1] + 1.0, r[2] - 2.0, r[3] - 2.0],
                BUTTON_FILL,
            ));
            let (ax, ay) = (r[0] + r[2] / 2.0, r[1] + r[3] / 2.0);
            let (hw, hh) = (d(7.0), d(4.0));
            let dir = if up { -1.0 } else { 1.0 };
            ops.push(Op::Path(
                vec![
                    PathSeg::M(ax - hw, ay - dir * hh / 2.0),
                    PathSeg::L(ax + hw, ay - dir * hh / 2.0),
                    PathSeg::L(ax, ay + dir * hh),
                    PathSeg::Z,
                ],
                Some(if enabled {
                    Rgba(0x80, 0x80, 0x80, 255)
                } else {
                    Rgba(0xc8, 0xc8, 0xc8, 255)
                }),
                None,
            ));
            hits.push((r, hit));
        }
        // The list.
        let lx = x + d(17.0);
        let lw = (w - d(26.0)).max(d(100.0));
        let row_h = d(31.0);
        let bar_x = lx + d(184.0).min(lw * 0.74);
        // 20 px a second, fewer when the longest click would leave the pane.
        let end = effects.iter().map(|a| a.end_ms).max().unwrap_or(0).max(1);
        let room = (lx + lw - d(30.0) - bar_x).max(d(20.0));
        let per_ms = (d(20.0) / 1000.0).min(room / end as f32);
        let mut ry = y + d(118.0);
        for (i, a) in effects.iter().enumerate() {
            if ry > y + h {
                break;
            }
            let r = [lx, ry, lw, row_h];
            if self.selected == Some(i) || self.extra.contains(&i) {
                ops.push(Op::Rect(r, SELECTED));
            }
            let ty = ry + (row_h - line) / 2.0;
            // Click number on effects started by a click, right-aligned before the star.
            if a.trigger == "onClick" {
                let n = a.click.to_string();
                let nw = text.measure(&n, body).0;
                ops.push(Op::Text(n, lx + d(21.0) - nw, ty, body, ink));
            }
            let sc = (lx + d(34.0), ry + row_h / 2.0);
            ops.push(Op::Path(
                star(sc.0, sc.1, d(9.0)),
                Some(class_color(a.class)),
                None,
            ));
            let name_x = lx + d(51.0);
            let name_end = if self.hide_timeline {
                lx + lw - d(30.0)
            } else {
                bar_x - d(4.0)
            };
            let name = fit(&a.shape_name, name_end - name_x, text, body);
            ops.push(Op::Text(name, name_x, ty, body, ink));
            if !self.hide_timeline {
                let bh = d(23.0);
                let bx = (bar_x + a.start_ms as f32 * per_ms).round();
                let bw = ((a.end_ms.saturating_sub(a.start_ms)) as f32 * per_ms)
                    .round()
                    .max(d(4.0));
                let top = (ry + (row_h - bh) / 2.0).round();
                let (fill, edge) = bar_colors(a.class);
                ops.push(Op::Rect([bx, top, bw, bh], edge));
                ops.push(Op::Rect(
                    [bx + 1.0, top + 1.0, (bw - 2.0).max(1.0), bh - 2.0],
                    fill,
                ));
                if a.trigger == "afterPrevious" && a.start_ms > 0 {
                    // A hairline down the row at the effect's start (reference).
                    ops.push(Op::Rect([bx, ry, 1.0, row_h], edge));
                }
            }
            hits.push((r, AnimHit::Select(i)));
            // The chosen row's drop-down arrow at its right end, as on the reference.
            if self.selected == Some(i) {
                let bw = d(26.0);
                let b = [r[0] + r[2] - bw, ry, bw, row_h];
                let (ax, ay, k) = (b[0] + bw / 2.0, b[1] + b[3] / 2.0, d(6.0));
                ops.push(Op::Path(
                    vec![
                        PathSeg::M(ax - k, ay - k / 2.0),
                        PathSeg::L(ax + k, ay - k / 2.0),
                        PathSeg::L(ax, ay + k / 2.0 + 1.0),
                        PathSeg::Z,
                    ],
                    Some(ink),
                    None,
                ));
                hits.push((b, AnimHit::Menu(i)));
            }
            ry += row_h;
        }
        self.ops = ops;
        self.hits = hits;
    }

    /// First and last chosen rows (the chosen one and Shift-added ones).
    fn first_chosen(&self) -> Option<usize> {
        self.selected
            .into_iter()
            .chain(self.extra.iter().copied())
            .min()
    }

    fn last_chosen(&self) -> Option<usize> {
        self.selected
            .into_iter()
            .chain(self.extra.iter().copied())
            .max()
    }

    /// Shift+click: choose the rows from the chosen one to `i` ("Play Selected").
    pub fn extend_to(&mut self, i: usize) {
        let Some(s) = self.selected else {
            self.selected = Some(i);
            return;
        };
        self.extra = (s.min(i)..=s.max(i)).filter(|&k| k != s).collect();
    }
}

/// Timing bar fill and frame of an effect class: green entrances as the reference's
/// (158, 219, 134 framed 56, 104, 33), and the same lightness for the other classes.
fn bar_colors(class: &str) -> (Rgba, Rgba) {
    match class {
        "entr" => (Rgba(158, 219, 134, 255), Rgba(56, 104, 33, 255)),
        "emph" => (Rgba(250, 216, 120, 255), Rgba(150, 110, 10, 255)),
        "exit" => (Rgba(240, 160, 150, 255), Rgba(150, 45, 30, 255)),
        _ => (Rgba(160, 196, 238, 255), Rgba(40, 80, 150, 255)),
    }
}

/// `s` shortened with an ellipsis to fit `width`.
fn fit(s: &str, width: f32, text: &dyn TextMeasure, style: TextStyle) -> String {
    if text.measure(s, style).0 <= width {
        return s.to_owned();
    }
    let mut out: String = s.to_owned();
    while !out.is_empty() {
        out.pop();
        let t = format!("{out}\u{2026}");
        if text.measure(&t, style).0 <= width {
            return t;
        }
    }
    String::new()
}
