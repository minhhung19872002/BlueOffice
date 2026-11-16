//! BlueSlide's Animation Pane (N6, round 3): the effects of the current slide in sequence
//! order on the right of the editing area, as the reference presentation app lists them —
//! a header with Play From / Play All and the reorder arrows, one row per effect (click
//! number, a star in the class colour, the shape's name, a timing bar), the chosen row
//! highlighted. Clicking a row selects its effect (and its shape); the arrows move it
//! earlier or later; Delete removes it.
use crate::{Key, Painter, TextMeasure, TextStyle};
use bo_engine::slide_edit::show::AnimOut;
use bo_render::{PathSeg, Rgba};

const PANE_BG: Rgba = Rgba(0xff, 0xff, 0xff, 255);
const SPLIT: Rgba = Rgba(0xbf, 0xbf, 0xbf, 255);
const EDGE: Rgba = Rgba(0xc6, 0xc6, 0xc6, 255);
const SELECTED: Rgba = Rgba(0xcd, 0xe6, 0xf7, 255);
const DIM: Rgba = Rgba(0x60, 0x60, 0x60, 255);
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

    fn px(&self, v: f32) -> f32 {
        (v * self.scale).round()
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
    /// Play All.
    #[allow(clippy::too_many_lines)]
    pub fn record(
        &mut self,
        text: &dyn TextMeasure,
        effects: &[AnimOut],
        labels: &[String; 3],
        ink: Rgba,
    ) {
        let mut ops = Vec::new();
        let mut hits = Vec::new();
        let [x, y, w, h] = self.rect;
        self.count = effects.len();
        if self.selected.is_some_and(|s| s >= effects.len()) {
            self.selected = None;
        }
        ops.push(Op::Rect([x, y, w, h], PANE_BG));
        ops.push(Op::Rect([x, y, 1.0, h], SPLIT));
        let pad = self.px(11.2);
        let title = TextStyle {
            size: self.px(16.0),
            bold: false,
        };
        let body = TextStyle {
            size: self.px(12.0),
            bold: false,
        };
        let line = text.measure("Ag", body).1.max(self.px(14.0));
        ops.push(Op::Text(labels[0].clone(), x + pad, y + pad, title, ink));
        // Close cross.
        let cs = self.px(20.0);
        let close = [x + w - pad - cs, y + pad, cs, cs];
        let k = self.px(5.0);
        let (cx, cy) = (close[0] + cs / 2.0, close[1] + cs / 2.0);
        ops.push(Op::Path(
            vec![
                PathSeg::M(cx - k, cy - k),
                PathSeg::L(cx + k, cy + k),
                PathSeg::M(cx + k, cy - k),
                PathSeg::L(cx - k, cy + k),
            ],
            None,
            Some((ink, self.scale.max(1.0).round())),
        ));
        hits.push((close, AnimHit::Close));
        // Play button (a triangle and its label), reorder arrows at the right.
        let by = y + pad + self.px(32.0);
        let bh = self.px(26.0);
        let label = if self.selected.is_some() {
            &labels[1]
        } else {
            &labels[2]
        };
        let (lw, _) = text.measure(label, body);
        let play = [x + pad, by, lw + self.px(34.0), bh];
        ops.push(Op::Rect(play, EDGE));
        ops.push(Op::Rect(
            [play[0] + 1.0, play[1] + 1.0, play[2] - 2.0, play[3] - 2.0],
            PANE_BG,
        ));
        let (tx, ty) = (play[0] + self.px(10.0), play[1] + bh / 2.0);
        let t = self.px(5.0);
        ops.push(Op::Path(
            vec![
                PathSeg::M(tx, ty - t),
                PathSeg::L(tx + 1.6 * t, ty),
                PathSeg::L(tx, ty + t),
                PathSeg::Z,
            ],
            Some(ENTR),
            None,
        ));
        ops.push(Op::Text(
            label.clone(),
            play[0] + self.px(24.0),
            play[1] + (bh - line) / 2.0,
            body,
            ink,
        ));
        hits.push((play, AnimHit::Play));
        let aw = self.px(24.0);
        for (k, (hit, up)) in [(AnimHit::Earlier, true), (AnimHit::Later, false)]
            .into_iter()
            .enumerate()
        {
            let r = [
                x + w - pad - (2 - k) as f32 * (aw + self.px(4.0)),
                by,
                aw,
                bh,
            ];
            let enabled = match (self.selected, up) {
                (Some(s), true) => s > 0,
                (Some(s), false) => s + 1 < effects.len(),
                _ => false,
            };
            let c = if enabled { ink } else { EDGE };
            let (ax, ay) = (r[0] + aw / 2.0, r[1] + bh / 2.0);
            let d = if up { -1.0 } else { 1.0 };
            ops.push(Op::Path(
                vec![
                    PathSeg::M(ax - k_of(self.scale), ay + d * -k_of(self.scale) / 2.0),
                    PathSeg::L(ax, ay + d * k_of(self.scale) / 2.0),
                    PathSeg::L(ax + k_of(self.scale), ay + d * -k_of(self.scale) / 2.0),
                ],
                None,
                Some((c, self.scale.max(1.0).round() + 0.5)),
            ));
            hits.push((r, hit));
        }
        // The list.
        let top = by + bh + pad;
        ops.push(Op::Rect([x + pad, top, w - 2.0 * pad, 1.0], EDGE));
        let row_h = self.px(25.6);
        let end = effects
            .iter()
            .map(|a| f64::from(a.end_ms))
            .fold(1.0, f64::max);
        let bar_x = x + w * 0.62;
        let bar_w = w - pad - (bar_x - x) - self.px(4.0);
        let mut ry = top + self.px(4.0);
        for (i, a) in effects.iter().enumerate() {
            if ry > y + h {
                break;
            }
            let r = [x + pad, ry, w - 2.0 * pad, row_h];
            if self.selected == Some(i) {
                ops.push(Op::Rect(r, SELECTED));
            }
            let ty = ry + (row_h - line) / 2.0;
            // Click number on effects started by a click.
            if a.trigger == "onClick" {
                ops.push(Op::Text(
                    a.click.to_string(),
                    x + pad + self.px(4.0),
                    ty,
                    body,
                    DIM,
                ));
            }
            let sc = (x + pad + self.px(26.0), ry + row_h / 2.0);
            ops.push(Op::Path(
                star(sc.0, sc.1, self.px(7.0)),
                Some(class_color(a.class)),
                None,
            ));
            let name = fit(
                &a.shape_name,
                bar_x - (x + pad + self.px(40.0)) - self.px(6.0),
                text,
                body,
            );
            ops.push(Op::Text(name, x + pad + self.px(40.0), ty, body, ink));
            // Timing bar: start to end within the slide's longest end.
            let k = bar_w / end as f32;
            let bx = bar_x + a.start_ms as f32 * k;
            let bw = ((a.end_ms.saturating_sub(a.start_ms)) as f32 * k).max(self.px(4.0));
            let bh = self.px(8.0);
            ops.push(Op::Rect(
                [
                    bx.round(),
                    (ry + (row_h - bh) / 2.0).round(),
                    bw.round(),
                    bh,
                ],
                class_color(a.class),
            ));
            hits.push((r, AnimHit::Select(i)));
            // The chosen row's drop-down button at its right end, as on the reference.
            if self.selected == Some(i) {
                let bw = self.px(18.0);
                let b = [r[0] + r[2] - bw, ry + 1.0, bw, row_h - 2.0];
                ops.push(Op::Rect(b, EDGE));
                ops.push(Op::Rect(
                    [b[0] + 1.0, b[1] + 1.0, b[2] - 2.0, b[3] - 2.0],
                    PANE_BG,
                ));
                let (ax, ay, k) = (b[0] + bw / 2.0, b[1] + b[3] / 2.0, self.px(3.5));
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
}

/// Half width of the reorder chevrons.
fn k_of(scale: f32) -> f32 {
    (5.0 * scale).round()
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
