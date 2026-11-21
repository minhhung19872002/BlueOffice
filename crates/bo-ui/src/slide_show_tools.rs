//! Slide show keys and pointer tools (N6, round 6), as the reference presentation app has
//! them while presenting: B / . blacks the screen and W / , whites it (any key or click
//! brings the slide back), a slide number then Enter jumps to that slide, Ctrl+P draws with
//! a pen, Ctrl+I with a highlighter, Ctrl+L shows a laser pointer, Ctrl+A / Esc go back to
//! the arrow, E erases the drawings of the slide. Drawings stay with their slide until the
//! show ends; they are kept in fractions of the slide so they follow a resize.
use super::{SlideView, BLACK, R};
use crate::{Input, Key, Mods, Painter};
use bo_engine::slide_edit::present::{InkSlide, InkStroke};
use bo_render::{PathSeg, Rgba};
use std::collections::HashMap;

/// A blanked screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    /// B or `.`.
    Black,
    /// W or `,`.
    White,
}

/// The pointer while presenting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Pointer {
    /// The arrow: clicks advance.
    #[default]
    Arrow,
    /// Pen (Ctrl+P): red ink, thin.
    Pen,
    /// Highlighter (Ctrl+I): wide translucent yellow.
    Highlighter,
    /// Laser pointer (Ctrl+L): a red dot follows the pointer.
    Laser,
    /// Eraser (Ctrl+E): a click removes the drawing under the pointer.
    Eraser,
}

/// A drawn stroke, in fractions of the slide.
#[derive(Debug, Clone, PartialEq)]
pub struct ShowStroke {
    /// Points (0–1 across, 0–1 down).
    pub pts: Vec<(f32, f32)>,
    /// Highlighter (else pen).
    pub highlight: bool,
}

/// State of the show tools.
#[derive(Debug, Clone, Default)]
pub struct ShowTools {
    /// The screen is blanked.
    pub screen: Option<Screen>,
    /// Digits typed for "number + Enter".
    pub digits: String,
    /// The pointer tool.
    pub pointer: Pointer,
    /// Drawings per slide index.
    pub strokes: HashMap<usize, Vec<ShowStroke>>,
    /// The stroke being drawn.
    pub drawing: Option<ShowStroke>,
    /// Laser position (window pixels).
    pub laser: Option<(f32, f32)>,
    /// The slide shown before the current one (right-click menu › Last Viewed).
    pub last_viewed: Option<usize>,
    /// The slide shown now (for [`ShowTools::last_viewed`]).
    pub seen: Option<usize>,
    /// Right-click menu › Pause: the show clock when automatic advance was paused.
    pub paused_at: Option<f64>,
    /// The arrow pointer: 0 shown, 1 hidden (Ctrl+H), 2 hidden after 3 s without moving
    /// (Ctrl+U); Ctrl+A shows it again.
    pub arrow_mode: u8,
    /// Show clock of the last pointer move.
    pub last_move: f64,
}

/// Pen ink (reference default: red) and highlighter (yellow, translucent).
const PEN: Rgba = Rgba(0xff, 0x00, 0x00, 255);
const HIGHLIGHT: Rgba = Rgba(0xff, 0xff, 0x00, 110);
const WHITE: Rgba = Rgba(0xff, 0xff, 0xff, 255);

impl SlideView {
    /// The show tools' state (tests, host cursor).
    pub fn show_tools(&self) -> &ShowTools {
        &self.show_tools
    }

    /// Where the show slide is drawn (window pixels).
    pub(crate) fn show_slide_rect(&self) -> R {
        let [ax, ay, ww, wh] = self.show_area();
        let scale = self.show_scale();
        let (w, h) = self.slide_px(scale);
        [
            (ax + (ww - w) / 2.0).floor(),
            (ay + (wh - h) / 2.0).floor(),
            w.round(),
            h.round(),
        ]
    }

    fn show_fraction(&self, x: f32, y: f32) -> (f32, f32) {
        let [rx, ry, rw, rh] = self.show_slide_rect();
        ((x - rx) / rw.max(1.0), (y - ry) / rh.max(1.0))
    }

    /// Keys and pointer input of the show tools; `None` lets the show handle `input`.
    pub(super) fn show_tools_input(&mut self, input: &Input) -> Option<bool> {
        if matches!(input, Input::Move { .. } | Input::Drag { .. }) {
            self.show_tools.last_move = self.clock;
        }
        // A blanked screen: any key or click brings the slide back (and does nothing else).
        if self.show_tools.screen.is_some() {
            return match input {
                Input::Text(_) | Input::Key(..) | Input::Press { .. } => {
                    self.show_tools.screen = None;
                    Some(true)
                }
                _ => None,
            };
        }
        match input {
            Input::Text(t) => {
                let t = t.as_str();
                if t.len() == 1 && t.chars().all(|c| c.is_ascii_digit()) {
                    self.show_tools.digits.push_str(t);
                    return Some(false);
                }
                self.show_tools.digits.clear();
                let screen = match t {
                    "b" | "B" | "." => Some(Screen::Black),
                    "w" | "W" | "," => Some(Screen::White),
                    _ => None,
                };
                if screen.is_some() {
                    self.show_tools.screen = screen;
                    return Some(true);
                }
                if matches!(t, "g" | "G" | "-") {
                    return Some(self.open_show_overview());
                }
                if matches!(t, "s" | "S") {
                    let paused = self.show_tools.paused_at.is_some();
                    return Some(self.show_command(if paused { "resume" } else { "pause" }));
                }
                if matches!(t, "h" | "H") {
                    // The next slide in deck order, hidden or not.
                    let i = self.show_slide()?;
                    return Some(i + 1 < self.ids.len() && self.show_jump(i + 1));
                }
                // "+" zooms into the middle of the slide (reference).
                if matches!(t, "+" | "=") {
                    return Some(self.magnify_step(1));
                }
                if matches!(t, "e" | "E") {
                    let i = self.show_slide()?;
                    return Some(self.show_tools.strokes.remove(&i).is_some());
                }
                None
            }
            Input::Key(Key::Enter, _) if !self.show_tools.digits.is_empty() => {
                let n: usize = std::mem::take(&mut self.show_tools.digits)
                    .parse()
                    .unwrap_or(0);
                if n == 0 || n > self.ids.len() {
                    return Some(false);
                }
                Some(self.show_jump(n - 1))
            }
            Input::Key(Key::Letter(c), Mods { ctrl: true, .. }) => {
                let tool = match c {
                    'p' => Pointer::Pen,
                    'i' => Pointer::Highlighter,
                    'l' => Pointer::Laser,
                    'e' => Pointer::Eraser,
                    'a' => Pointer::Arrow,
                    's' => return Some(self.open_show_overview()),
                    'h' | 'u' => {
                        self.show_tools.arrow_mode = if *c == 'h' { 1 } else { 2 };
                        self.show_tools.last_move = self.clock;
                        return Some(true);
                    }
                    _ => return None,
                };
                self.show_tools.digits.clear();
                if tool == Pointer::Arrow {
                    self.show_tools.arrow_mode = 0;
                }
                self.set_show_pointer(tool);
                Some(true)
            }
            Input::Key(Key::Escape, _) if self.show_tools.pointer != Pointer::Arrow => {
                self.set_show_pointer(Pointer::Arrow);
                Some(true)
            }
            Input::Key(..) => {
                self.show_tools.digits.clear();
                None
            }
            &Input::Press { x, y, .. }
                if matches!(self.show_tools.pointer, Pointer::Pen | Pointer::Highlighter) =>
            {
                self.show_slide()?;
                self.show_tools.drawing = Some(ShowStroke {
                    pts: vec![self.show_fraction(x, y)],
                    highlight: self.show_tools.pointer == Pointer::Highlighter,
                });
                Some(true)
            }
            &Input::Press { .. } if self.show_tools.pointer == Pointer::Laser => Some(false),
            &Input::Press { x, y, .. } if self.show_tools.pointer == Pointer::Eraser => {
                Some(self.erase_stroke_at(x, y))
            }
            &Input::Drag { x, y } | &Input::Move { x, y } => {
                if self.show_tools.pointer == Pointer::Laser {
                    self.show_tools.laser = Some((x, y));
                    return Some(true);
                }
                let f = self.show_fraction(x, y);
                let d = self.show_tools.drawing.as_mut()?;
                d.pts.push(f);
                Some(true)
            }
            Input::Release { .. } => {
                let d = self.show_tools.drawing.take()?;
                let i = self.show_slide()?;
                self.show_tools.strokes.entry(i).or_default().push(d);
                Some(true)
            }
            Input::Leave => {
                self.show_tools.laser = None;
                Some(true)
            }
            _ => None,
        }
    }

    /// Eraser: remove the drawing of the shown slide passing within 10 px of (`x`, `y`).
    fn erase_stroke_at(&mut self, x: f32, y: f32) -> bool {
        let Some(i) = self.show_slide() else {
            return false;
        };
        let [rx, ry, rw, rh] = self.show_slide_rect();
        let reach = 10.0 * self.display_scale.max(1.0);
        let Some(list) = self.show_tools.strokes.get_mut(&i) else {
            return false;
        };
        let at = |(fx, fy): (f32, f32)| (rx + fx * rw, ry + fy * rh);
        // Distance from the pointer to the segment a–b.
        let near = |a: (f32, f32), b: (f32, f32)| {
            let (dx, dy) = (b.0 - a.0, b.1 - a.1);
            let len2 = dx * dx + dy * dy;
            let t = if len2 > 0.0 {
                (((x - a.0) * dx + (y - a.1) * dy) / len2).clamp(0.0, 1.0)
            } else {
                0.0
            };
            (a.0 + t * dx - x).hypot(a.1 + t * dy - y) <= reach
        };
        let hit = list.iter().rposition(|st| {
            let pts: Vec<(f32, f32)> = st.pts.iter().copied().map(at).collect();
            match pts.as_slice() {
                [one] => near(*one, *one),
                _ => pts.windows(2).any(|w| near(w[0], w[1])),
            }
        });
        match hit {
            Some(k) => {
                list.remove(k);
                true
            }
            None => false,
        }
    }

    /// A command of the slide show's right-click menu (as the reference's): `next`, `prev`,
    /// `last` (last viewed), `goto:N`, `black`, `white`, `laser`, `pen`, `highlighter`,
    /// `eraser`, `arrow`, `eraseAll`, `all` (See All Slides), `end`.
    pub fn show_command(&mut self, id: &str) -> bool {
        if self.show.is_none() {
            return false;
        }
        if let Some(n) = id
            .strip_prefix("goto:")
            .and_then(|n| n.parse::<usize>().ok())
        {
            return n < self.ids.len() && self.show_jump(n);
        }
        match id {
            "next" => self.show_step(true),
            "prev" => self.show_step(false),
            "last" => match self.show_tools.last_viewed {
                Some(n) if n < self.ids.len() => self.show_jump(n),
                _ => false,
            },
            "black" => {
                self.show_tools.screen = Some(Screen::Black);
                true
            }
            "white" => {
                self.show_tools.screen = Some(Screen::White);
                true
            }
            "laser" => {
                self.set_show_pointer(Pointer::Laser);
                true
            }
            "pen" => {
                self.set_show_pointer(Pointer::Pen);
                true
            }
            "highlighter" => {
                self.set_show_pointer(Pointer::Highlighter);
                true
            }
            "eraser" => {
                self.set_show_pointer(Pointer::Eraser);
                true
            }
            "arrow" => {
                self.set_show_pointer(Pointer::Arrow);
                true
            }
            "eraseAll" => match self.show_slide() {
                Some(i) => self.show_tools.strokes.remove(&i).is_some(),
                None => false,
            },
            "end" => self.end_show(),
            "all" => self.open_show_overview(),
            "magnify" => self.start_magnify(),
            "presenter" => match self.presenting.as_mut() {
                Some(p) => {
                    p.plan.presenter = !p.plan.presenter;
                    true
                }
                None => false,
            },
            "pause" => {
                if !self.show_can_pause() {
                    return false;
                }
                self.show_tools.paused_at = Some(self.clock);
                true
            }
            "resume" => {
                let Some(at) = self.show_tools.paused_at.take() else {
                    return false;
                };
                // The slide's timing goes on from where it stopped.
                let now = self.clock;
                if let Some(s) = self.show.as_mut() {
                    s.arrived += now - at;
                }
                true
            }
            _ => false,
        }
    }

    /// What the right-click menu greys or names: (at the first step of the show, Presenter
    /// View can be switched, Presenter View is on, paused).
    pub fn show_menu_state(&self) -> (bool, bool, bool, bool) {
        let at_start = self
            .show
            .as_ref()
            .is_some_and(|s| s.step == 0 && !s.ended && self.prev_in_show(s.index).is_none());
        (
            at_start,
            self.presenting.is_some(),
            self.presenter(),
            self.show_tools.paused_at.is_some(),
        )
    }

    /// The show clock (ms) last given by the host.
    pub fn clock_ms(&self) -> f64 {
        self.clock
    }

    /// Whether the host should hide the mouse pointer now (Ctrl+H, or Ctrl+U after 3 s
    /// without moving), with the arrow tool of a running show.
    pub fn show_cursor_hidden(&self) -> bool {
        let t = &self.show_tools;
        self.show.is_some()
            && t.pointer == Pointer::Arrow
            && (t.arrow_mode == 1 || (t.arrow_mode == 2 && self.clock - t.last_move >= 3000.0))
    }

    /// Whether the shown slide advances by itself (right-click menu › Pause is available).
    pub fn show_can_pause(&self) -> bool {
        self.show.as_ref().is_some_and(|s| {
            !s.ended
                && s.info
                    .as_ref()
                    .and_then(|i| i.transition.as_ref())
                    .and_then(|t| t.advance_after_ms)
                    .is_some()
        }) && self.timings_on()
    }

    /// Slide `index` is shown: the one before is remembered for "Last Viewed".
    pub(super) fn note_shown(&mut self, index: usize) {
        if self.show_tools.seen != Some(index) {
            self.show_tools.last_viewed = self.show_tools.seen;
            self.show_tools.seen = Some(index);
        }
    }

    /// Change the show's pointer tool.
    pub fn set_show_pointer(&mut self, tool: Pointer) {
        self.show_tools.pointer = tool;
        self.show_tools.drawing = None;
        if tool != Pointer::Laser {
            self.show_tools.laser = None;
        }
    }

    /// Forget the drawings and tools (the show ended).
    pub(super) fn reset_show_tools(&mut self) {
        self.show_tools = ShowTools::default();
    }

    /// The show's drawings in slide points, to keep or discard when it ends (the
    /// reference asks "Do you want to keep your ink annotations?"); slides in order.
    pub(super) fn show_ink(&self) -> Vec<InkSlide> {
        let (w, h) = self.slide_points();
        let (w, h) = (f64::from(w), f64::from(h));
        // Widths as drawn: 2.6 / 14 px per 960 px of slide.
        let k = w / 960.0;
        let mut slides: Vec<usize> = self.show_tools.strokes.keys().copied().collect();
        slides.sort_unstable();
        slides
            .into_iter()
            .filter_map(|slide| {
                let strokes: Vec<InkStroke> = self
                    .show_tools
                    .strokes
                    .get(&slide)?
                    .iter()
                    .filter(|s| !s.pts.is_empty())
                    .map(|s| InkStroke {
                        points: s
                            .pts
                            .iter()
                            .map(|&(u, v)| [f64::from(u) * w, f64::from(v) * h])
                            .collect(),
                        color: if s.highlight { "FFFF00" } else { "FF0000" }.into(),
                        width: if s.highlight { 14.0 * k } else { 2.6 * k },
                        highlighter: s.highlight,
                    })
                    .collect();
                (!strokes.is_empty()).then_some(InkSlide { slide, strokes })
            })
            .collect()
    }

    /// Ink left by the show that ended, waiting for the keep / discard answer.
    pub fn has_pending_ink(&self) -> bool {
        self.pending_ink.is_some()
    }

    /// Answer the keep / discard question: keep adds the drawings to their slides as ink
    /// (one undo step) under `name`.
    pub fn resolve_pending_ink(&mut self, keep: bool, name: &str) -> bool {
        let Some(ink) = self.pending_ink.take() else {
            return false;
        };
        keep && self.run(bo_engine::slide_edit::SlideCommand::KeepInk {
            ink,
            name: name.to_owned(),
        })
    }

    /// Drawings, the laser dot and a blanked screen over the show.
    pub(super) fn paint_show_tools(&self, p: &mut dyn Painter) {
        if let Some(screen) = self.show_tools.screen {
            let [ax, ay, w, h] = self.show_area();
            p.fill_rect(
                ax,
                ay,
                w,
                h,
                if screen == Screen::Black {
                    BLACK
                } else {
                    WHITE
                },
            );
            return;
        }
        let Some(i) = self.show_slide() else {
            return;
        };
        let r = self.show_slide_rect();
        let to_px = |(u, v): (f32, f32)| (r[0] + u * r[2], r[1] + v * r[3]);
        let k = r[2] / 960.0;
        let strokes = self.show_tools.strokes.get(&i).into_iter().flatten();
        for s in strokes.chain(self.show_tools.drawing.as_ref()) {
            let (color, width) = if s.highlight {
                (HIGHLIGHT, (14.0 * k).max(6.0))
            } else {
                (PEN, (2.6 * k).max(2.0))
            };
            let segs: Vec<PathSeg> = s
                .pts
                .iter()
                .enumerate()
                .map(|(n, &f)| {
                    let (x, y) = to_px(f);
                    if n == 0 {
                        PathSeg::M(x, y)
                    } else {
                        PathSeg::L(x, y)
                    }
                })
                .collect();
            if segs.len() == 1 {
                let (x, y) = to_px(s.pts[0]);
                p.fill_rect(x - width / 2.0, y - width / 2.0, width, width, color);
            } else {
                p.path(&segs, None, Some((color, width)));
            }
        }
        if let Some((x, y)) = self.show_tools.laser {
            // A red dot with a soft halo (reference laser pointer).
            let d = (10.0 * self.display_scale).round();
            for (grow, a) in [(2.2f32, 50u8), (1.5, 110), (1.0, 255)] {
                let rr = d * grow / 2.0;
                p.path(&circle(x, y, rr), Some(Rgba(0xff, 0x10, 0x10, a)), None);
            }
        }
    }
}

/// A circle as cubic-free polygon (24 sides).
fn circle(cx: f32, cy: f32, r: f32) -> Vec<PathSeg> {
    let mut v: Vec<PathSeg> = (0..24)
        .map(|i| {
            let t = i as f32 / 24.0 * std::f32::consts::TAU;
            let (x, y) = (cx + r * t.cos(), cy + r * t.sin());
            if i == 0 {
                PathSeg::M(x, y)
            } else {
                PathSeg::L(x, y)
            }
        })
        .collect();
    v.push(PathSeg::Z);
    v
}
