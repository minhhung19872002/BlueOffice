//! Draw tab, round 17 (N6): the Ruler stencil and Ink Replay of BlueSlide.
//!
//! - Ruler: a straight-edge laid over the slide (as on the reference presentation app: it
//!   starts at 45° across the slide, drags with the pointer, turns 1° per wheel notch —
//!   15° with Shift is not available without key state, so notches snap to 0 / 45 / 90° when
//!   within 2°). A pen stroke started just outside one of its long edges is drawn along that
//!   edge. Its look (translucent grey band, centimetre ticks on both edges, angle in the
//!   middle) is our own drawing.
//! - Ink Replay: the current slide's strokes drawn again one after the other over the slide
//!   without its ink (`inkInfo` with `backdrop`), each at its recorded pace when the strokes
//!   carry times ([MS-ODRAWXML] InkML channel `T`), else at a steady pace.
use super::SlideView;
use crate::{Input, Painter, TextStyle};
use bo_engine::slide_edit::ink::{InkCommand, InkStrokeOut};
use bo_render::{PageList, PathSeg, Rgba};

/// Ruler width across its edges (points).
const RULER_T: f64 = 72.0;
/// A stroke starting this close (device px) outside an edge is drawn along it.
const SNAP_PX: f64 = 14.0;
/// Points per centimetre.
const CM: f64 = 72.0 / 2.54;

/// The ruler over the slide (slide points, degrees clockwise).
#[derive(Debug, Clone, PartialEq)]
pub struct Ruler {
    /// Centre.
    pub cx: f64,
    /// Centre.
    pub cy: f64,
    /// Angle of the long edges, clockwise from horizontal.
    pub angle: f64,
    /// Length.
    pub len: f64,
    /// The pointer grabbed it here (offset from the centre) while dragging.
    drag: Option<(f64, f64)>,
    /// Pointer position last seen over the area (device px), for the wheel.
    pointer: (f32, f32),
    /// Edge (-1 top, +1 bottom) the stroke being drawn follows.
    snap: Option<f64>,
}

impl Ruler {
    /// A ruler across a slide `w` × `h` points, at 45° as the reference lays it first.
    #[must_use]
    pub fn across(w: f64, h: f64) -> Ruler {
        Ruler {
            cx: w / 2.0,
            cy: h / 2.0,
            angle: 45.0,
            // Reaching past the slide on both sides, as the reference's.
            len: w.hypot(h) * 1.6,
            drag: None,
            pointer: (0.0, 0.0),
            snap: None,
        }
    }

    fn dir(&self) -> (f64, f64) {
        let a = self.angle.to_radians();
        (a.cos(), a.sin())
    }

    /// Position of a slide point along / across the ruler.
    #[must_use]
    pub fn local(&self, x: f64, y: f64) -> (f64, f64) {
        let (dx, dy) = self.dir();
        let (px, py) = (x - self.cx, y - self.cy);
        (px * dx + py * dy, -px * dy + py * dx)
    }

    /// Slide point of a position along / across the ruler.
    #[must_use]
    pub fn global(&self, u: f64, v: f64) -> (f64, f64) {
        let (dx, dy) = self.dir();
        (self.cx + u * dx - v * dy, self.cy + u * dy + v * dx)
    }

    /// Whether a slide point is on the ruler.
    #[must_use]
    pub fn contains(&self, x: f64, y: f64) -> bool {
        let (u, v) = self.local(x, y);
        u.abs() <= self.len / 2.0 && v.abs() <= RULER_T / 2.0
    }

    /// The edge (-1 / +1) a point lies just outside of, within `reach` points.
    #[must_use]
    pub fn edge_near(&self, x: f64, y: f64, reach: f64) -> Option<f64> {
        let (u, v) = self.local(x, y);
        if u.abs() > self.len / 2.0 {
            return None;
        }
        let d = v.abs() - RULER_T / 2.0;
        (d >= 0.0 && d <= reach).then(|| v.signum())
    }

    /// A point moved onto edge `side`.
    #[must_use]
    pub fn onto(&self, side: f64, x: f64, y: f64) -> (f64, f64) {
        let (u, _) = self.local(x, y);
        let u = u.clamp(-self.len / 2.0, self.len / 2.0);
        self.global(u, side * RULER_T / 2.0)
    }

    /// Turn by `deg`, landing on 0 / 45 / 90 / … when within 2°.
    pub fn turn(&mut self, deg: f64) {
        let mut a = (self.angle + deg).rem_euclid(360.0);
        let near = (a / 45.0).round() * 45.0;
        if (a - near).abs() < 2.0 && (self.angle - near).abs() >= 2.0 {
            a = near;
        }
        self.angle = a.rem_euclid(360.0);
    }

    /// The angle shown in the middle (0–180°, as a straight edge has no direction).
    #[must_use]
    pub fn shown_angle(&self) -> u32 {
        let a = self.angle.rem_euclid(180.0).round() as u32;
        if a == 180 {
            0
        } else {
            a
        }
    }
}

/// Ink Replay in progress.
#[derive(Debug, Clone)]
pub struct Replay {
    /// Slide replayed.
    pub slide: usize,
    /// The slide without its ink.
    pub backdrop: PageList,
    /// Strokes in drawing order.
    strokes: Vec<InkStrokeOut>,
    /// Start and end of each stroke (ms after the start).
    spans: Vec<(f64, f64)>,
    /// Host clock when it started (`None` until the first tick).
    start: Option<f64>,
}

/// Gap between two replayed strokes (ms).
const GAP: f64 = 120.0;

impl Replay {
    fn new(slide: usize, backdrop: PageList, strokes: Vec<InkStrokeOut>) -> Replay {
        let mut t = 0.0;
        let spans = strokes
            .iter()
            .map(|s| {
                let recorded = s.times.as_ref().and_then(|ts| {
                    let (a, b) = (ts.first()?, ts.last()?);
                    (b > a).then_some(b - a)
                });
                let n = s.points.len() as f64;
                let d = recorded.unwrap_or(n * 8.0).clamp(80.0, 2500.0);
                let span = (t, t + d);
                t += d + GAP;
                span
            })
            .collect();
        Replay {
            slide,
            backdrop,
            strokes,
            spans,
            start: None,
        }
    }

    /// Length of the whole replay (ms).
    #[must_use]
    pub fn total(&self) -> f64 {
        self.spans.last().map_or(0.0, |s| s.1) + 300.0
    }

    /// Part of stroke `i` drawn at `t` ms (0..=1).
    #[must_use]
    pub fn drawn(&self, i: usize, t: f64) -> f64 {
        let Some(&(a, b)) = self.spans.get(i) else {
            return 0.0;
        };
        ((t - a) / (b - a).max(1.0)).clamp(0.0, 1.0)
    }
}

fn rgb(hex: &str) -> Rgba {
    let v = u32::from_str_radix(hex, 16).unwrap_or(0);
    Rgba((v >> 16) as u8, (v >> 8) as u8, v as u8, 255)
}

impl SlideView {
    /// Show or hide the Draw tab's ruler over the slide; `true` when it changed.
    pub fn toggle_ruler(&mut self) -> bool {
        if self.draw_ruler.take().is_none() {
            let (w, h) = self.slide_size_pt();
            self.draw_ruler = Some(Ruler::across(w, h));
        }
        true
    }

    /// The ruler, when shown.
    pub fn draw_ruler(&self) -> Option<&Ruler> {
        self.draw_ruler.as_ref()
    }

    fn slide_size_pt(&self) -> (f64, f64) {
        self.engine.presentation().map_or((960.0, 540.0), |p| {
            (
                p.slide_size.cx as f64 / 12_700.0,
                p.slide_size.cy as f64 / 12_700.0,
            )
        })
    }

    /// Pointer input for the ruler (drag it, turn it with the wheel); `None` when the input
    /// is not for it.
    pub(super) fn ruler_input(&mut self, input: &Input) -> Option<bool> {
        if self.sorter || self.show.is_some() {
            return None;
        }
        let k = f64::from(self.scale());
        let (sx, sy) = match *input {
            Input::Press { x, y, .. }
            | Input::Drag { x, y }
            | Input::Move { x, y }
            | Input::Release { x, y } => {
                let c = self.canvas_rect();
                if (x < c[0] || x >= c[0] + c[2] || y < c[1] || y >= c[1] + c[3])
                    && !matches!(input, Input::Drag { .. } | Input::Release { .. })
                {
                    return None;
                }
                if let Some(r) = self.draw_ruler.as_mut() {
                    r.pointer = (x, y);
                }
                self.to_slide(x, y)
            }
            _ => (0.0, 0.0),
        };
        let r = self.draw_ruler.as_mut()?;
        match *input {
            Input::Press { .. } if r.contains(sx, sy) => {
                r.drag = Some((sx - r.cx, sy - r.cy));
                Some(true)
            }
            Input::Press { .. } => {
                // A pen stroke starting just outside a long edge follows it.
                r.snap = if matches!(self.ink_tool, Some(super::InkTool::Pen { .. })) {
                    r.edge_near(sx, sy, SNAP_PX / k.max(0.01))
                } else {
                    None
                };
                None
            }
            Input::Drag { .. } => {
                let (ox, oy) = r.drag?;
                r.cx = sx - ox;
                r.cy = sy - oy;
                Some(true)
            }
            Input::Release { .. } => {
                if r.drag.take().is_some() {
                    return Some(true);
                }
                None
            }
            Input::Wheel(dy) => {
                let (px, py) = r.pointer;
                let (sx, sy) = {
                    let (ox, oy) = self.slide_origin();
                    let s = self.scale();
                    (f64::from((px - ox) / s), f64::from((py - oy) / s))
                };
                let r = self.draw_ruler.as_mut()?;
                if !r.contains(sx, sy) {
                    return None;
                }
                r.turn(if dy > 0.0 { -1.0 } else { 1.0 });
                Some(true)
            }
            _ => None,
        }
    }

    /// A stroke point, moved onto the ruler's edge when the stroke follows it.
    pub(super) fn ruler_snap(&self, x: f64, y: f64) -> [f64; 2] {
        match self.draw_ruler.as_ref() {
            Some(r) if r.snap.is_some() => {
                let (x, y) = r.onto(r.snap.unwrap_or(1.0), x, y);
                [x, y]
            }
            _ => [x, y],
        }
    }

    /// Draw the ruler over the slide.
    pub(super) fn paint_ruler(&self, p: &mut dyn Painter) {
        let Some(r) = self.draw_ruler.as_ref() else {
            return;
        };
        let k = self.scale();
        let corner = |u: f64, v: f64| {
            let (x, y) = r.global(u, v);
            self.to_px(x, y)
        };
        let (h, t) = (r.len / 2.0, RULER_T / 2.0);
        let pts = [corner(-h, -t), corner(h, -t), corner(h, t), corner(-h, t)];
        let mut segs: Vec<PathSeg> = pts
            .iter()
            .enumerate()
            .map(|(i, &(x, y))| {
                if i == 0 {
                    PathSeg::M(x, y)
                } else {
                    PathSeg::L(x, y)
                }
            })
            .collect();
        segs.push(PathSeg::Z);
        p.path(
            &segs,
            Some(Rgba(0xe6, 0xe6, 0xe6, 210)),
            Some((Rgba(0x8a, 0x8a, 0x8a, 255), 1.0)),
        );
        // Millimetre ticks on both long edges: longer every 5 mm, longest every centimetre.
        let ink = Rgba(0x50, 0x50, 0x50, 255);
        let mut ticks = Vec::new();
        let n = (r.len / (CM / 10.0)).floor() as i64;
        for i in 0..=n {
            let u = -h + i as f64 * CM / 10.0;
            let l = if i % 10 == 0 {
                9.0
            } else if i % 5 == 0 {
                6.0
            } else {
                3.5
            };
            // Ticks shorter than a device pixel apart are left out at small zooms.
            if i % 10 != 0 && f64::from(k) * CM / 10.0 < 3.0 {
                continue;
            }
            for side in [-1.0, 1.0] {
                let (x0, y0) = corner(u, side * t);
                let (x1, y1) = corner(u, side * (t - l));
                ticks.push(PathSeg::M(x0, y0));
                ticks.push(PathSeg::L(x1, y1));
            }
        }
        p.path(&ticks, None, Some((ink, 1.0)));
        let label = format!("{}°", r.shown_angle());
        let size = (13.0 * k).clamp(9.0, 22.0);
        let (cx, cy) = self.to_px(r.cx, r.cy);
        // A ring around the angle (own drawing).
        let rad = size * 1.9;
        let ring: Vec<PathSeg> = (0..=36)
            .map(|i| {
                let a = std::f32::consts::TAU * i as f32 / 36.0;
                let (x, y) = (cx + rad * a.cos(), cy + rad * a.sin());
                if i == 0 {
                    PathSeg::M(x, y)
                } else {
                    PathSeg::L(x, y)
                }
            })
            .collect();
        p.path(&ring, Some(Rgba(0xf4, 0xf4, 0xf4, 230)), Some((ink, 1.0)));
        let w = size * 0.55 * label.chars().count() as f32;
        p.text(
            &label,
            (cx - w / 2.0).round(),
            (cy - size * 0.7).round(),
            TextStyle { size, bold: false },
            Rgba(0x26, 0x26, 0x26, 255),
        );
    }

    /// Start Ink Replay on the current slide; `false` when it has no ink.
    pub fn start_ink_replay(&mut self) -> bool {
        let Ok(r) = self
            .engine
            .slide_ink_command(InkCommand::InkInfo { backdrop: true })
        else {
            return false;
        };
        let Some(ink) = r.ink else {
            return false;
        };
        let strokes: Vec<InkStrokeOut> = ink.objects.into_iter().flat_map(|o| o.strokes).collect();
        let Some(backdrop) = ink.backdrop else {
            return false;
        };
        if strokes.is_empty() {
            return false;
        }
        self.replay = Some(Replay::new(self.current, backdrop, strokes));
        self.invalidate_page_images();
        true
    }

    /// Review > Hide Ink ▾ > Delete All Ink on Slide / in Presentation: remove the ink
    /// objects (one undo step per slide).
    pub fn delete_ink(&mut self, all: bool) -> bool {
        let here = self.current;
        let slides: Vec<usize> = if all {
            (0..self.slide_count()).collect()
        } else {
            vec![here]
        };
        let mut done = false;
        for s in slides {
            let has = self.engine.presentation().is_some_and(|p| {
                p.slides.get(s).is_some_and(|sl| {
                    sl.data
                        .shapes
                        .iter()
                        .any(|n| bo_pptx::ink::node_content_part(n).is_some())
                })
            });
            if !has {
                continue;
            }
            self.go_to(s);
            // An eraser sweep over the whole slide removes every stroke.
            let (w, h) = self.slide_size_pt();
            let mut points = Vec::new();
            let mut y = 0.0;
            while y <= h {
                let mut x = 0.0;
                while x <= w {
                    points.push([x, y]);
                    x += 20.0;
                }
                y += 20.0;
            }
            if let Ok(r) = self.engine.slide_ink_command(InkCommand::EraseInk {
                points,
                radius: 20.0,
                mode: bo_engine::slide_edit::ink::EraseMode::Stroke,
            }) {
                done |= r.error.is_none();
                self.apply(r);
            }
        }
        if all {
            self.go_to(here);
        }
        done
    }

    /// Whether Ink Replay is running.
    pub fn replaying(&self) -> bool {
        self.replay.is_some()
    }

    /// Advance Ink Replay to the host clock; `true` when a frame is needed.
    pub(super) fn replay_clock(&mut self, ms: f64) -> bool {
        let Some(r) = self.replay.as_mut() else {
            return false;
        };
        let start = *r.start.get_or_insert(ms);
        if ms - start > r.total() {
            self.replay = None;
            self.invalidate_page_images();
        }
        true
    }

    /// Stop Ink Replay (any input).
    pub(super) fn stop_replay(&mut self) {
        if self.replay.take().is_some() {
            self.invalidate_page_images();
        }
    }

    /// The strokes replayed so far, over the slide.
    pub(super) fn paint_replay(&self, p: &mut dyn Painter) {
        let Some(r) = self.replay.as_ref() else {
            return;
        };
        if r.slide != self.current {
            return;
        }
        let t = r.start.map_or(0.0, |s| self.clock - s);
        let k = self.scale();
        for (i, s) in r.strokes.iter().enumerate() {
            let part = r.drawn(i, t);
            if part <= 0.0 || s.points.is_empty() {
                continue;
            }
            let n = ((s.points.len() as f64 * part).ceil() as usize).clamp(1, s.points.len());
            let mut c = rgb(&s.color);
            c.3 = if s.highlighter {
                128
            } else {
                (s.opacity * 255.0).round().clamp(0.0, 255.0) as u8
            };
            let w = s.points[..n].iter().map(|q| q[2]).sum::<f64>() / n as f64;
            let segs: Vec<PathSeg> = s.points[..n]
                .iter()
                .enumerate()
                .map(|(j, q)| {
                    let (x, y) = self.to_px(q[0], q[1]);
                    if j == 0 {
                        PathSeg::M(x, y)
                    } else {
                        PathSeg::L(x, y)
                    }
                })
                .collect();
            p.path(&segs, None, Some((c, (w as f32 * k).max(1.0))));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ruler_geometry() {
        let mut r = Ruler::across(960.0, 540.0);
        assert_eq!(r.shown_angle(), 45);
        assert!(r.contains(480.0, 270.0));
        // A point just below the bottom edge (local v = T/2 + 5) follows that edge.
        let (x, y) = r.global(10.0, RULER_T / 2.0 + 5.0);
        assert_eq!(r.edge_near(x, y, 10.0), Some(1.0));
        let (ex, ey) = r.onto(1.0, x, y);
        let (u, v) = r.local(ex, ey);
        assert!((u - 10.0).abs() < 1e-9 && (v - RULER_T / 2.0).abs() < 1e-9);
        // Far away: no edge.
        let (x, y) = r.global(10.0, RULER_T / 2.0 + 30.0);
        assert_eq!(r.edge_near(x, y, 10.0), None);
        // Wheel turns 1° and lands on 90° when close.
        r.angle = 88.5;
        r.turn(1.0);
        assert_eq!(r.shown_angle(), 90);
        r.turn(1.0);
        assert_eq!(r.shown_angle(), 91);
        r.angle = 179.6;
        assert_eq!(r.shown_angle(), 0);
    }

    #[test]
    fn replay_spans() {
        let stroke = |times: Option<Vec<f64>>| InkStrokeOut {
            points: vec![[0.0, 0.0, 1.0]; 10],
            times,
            color: "000000".into(),
            opacity: 1.0,
            highlighter: false,
            rect_tip: false,
        };
        let r = Replay::new(
            0,
            PageList {
                width: 1.0,
                height: 1.0,
                commands: vec![],
            },
            vec![stroke(Some(vec![0.0, 500.0])), stroke(None)],
        );
        // Recorded pace 500 ms, then the gap, then 10 points × 8 ms (at least 80).
        assert_eq!(r.spans, vec![(0.0, 500.0), (620.0, 700.0)]);
        assert!((r.drawn(0, 250.0) - 0.5).abs() < 1e-9);
        assert_eq!(r.drawn(1, 250.0), 0.0);
        assert_eq!(r.drawn(1, 800.0), 1.0);
    }
}
