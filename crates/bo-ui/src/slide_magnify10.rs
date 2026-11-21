//! Magnify Slide while presenting (round 10 resume of the slide series), as the reference:
//! - right-click menu › Magnify Slide: a lens of half the slide's size follows the pointer
//!   over a dimmed slide; a click shows that part at twice the size;
//! - "+" zooms in by steps of 2^(1/3) about the slide's centre (measured: title 1.26×, 1.59×,
//!   2.0× after one, two, three presses), "-" steps back out (at the whole slide "-" opens
//!   See All Slides);
//! - dragging pans, Esc or a right click goes back to the whole slide.
//!
//! The magnified part is drawn sharp from an image rendered at the zoomed scale.
use super::{SlideView, R};
use crate::{Input, Key, PageStore, Painter};
use bo_render::{PathSeg, Rgba};

/// Image key of the magnified slide (outside the other kinds of the view).
const MAG_KEY: usize = 22 << 40;
/// Zoom steps ("+" presses) at most: 2^(9/3) = 8×.
const MAX_LEVEL: u8 = 9;
/// Zoom level of the lens click: 2×.
const LENS_LEVEL: u8 = 3;

/// The magnifier's state.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Magnify {
    /// Pointer (window px) while choosing the part with the lens.
    pub pointer: Option<(f32, f32)>,
    /// Centre of the magnified part in fractions of the slide, once zoomed.
    pub center: Option<(f32, f32)>,
    /// Zoom steps: the slide shows at 2^(level/3) of its size.
    pub level: u8,
    /// Last drag position while panning.
    pub drag: Option<(f32, f32)>,
}

impl Magnify {
    /// Size factor of the zoomed slide.
    pub fn factor(&self) -> f32 {
        2f32.powf(f32::from(self.level) / 3.0)
    }
}

/// Keep the shown part inside the slide at zoom `f`.
fn clamp_center(u: f32, v: f32, f: f32) -> (f32, f32) {
    let m = 0.5 / f.max(1.0);
    (u.clamp(m, 1.0 - m), v.clamp(m, 1.0 - m))
}

impl SlideView {
    /// The magnifier's state while presenting.
    pub fn show_magnify(&self) -> Option<Magnify> {
        self.magnify
    }

    /// Right-click menu › Magnify Slide: choose a part with the pointer.
    pub fn start_magnify(&mut self) -> bool {
        if self.show_slide().is_none() {
            return false;
        }
        self.magnify = Some(Magnify::default());
        true
    }

    /// "+" (`step` 1) / "-" (`step` -1) while presenting: one zoom step about the shown
    /// part's centre (the slide's centre at first); back to the whole slide at level 0.
    pub fn magnify_step(&mut self, step: i8) -> bool {
        if self.show_slide().is_none() {
            return false;
        }
        let m = self
            .magnify
            .filter(|m| m.center.is_some())
            .unwrap_or_default();
        let level = (i16::from(m.level) + i16::from(step)).clamp(0, i16::from(MAX_LEVEL));
        let level = u8::try_from(level).unwrap_or(0);
        if level == 0 {
            return self.magnify.take().is_some();
        }
        let next = Magnify { level, ..m };
        let (u, v) = m.center.unwrap_or((0.5, 0.5));
        self.magnify = Some(Magnify {
            center: Some(clamp_center(u, v, next.factor())),
            pointer: None,
            ..next
        });
        true
    }

    /// Back to the whole slide.
    pub fn cancel_magnify(&mut self) -> bool {
        self.magnify.take().is_some()
    }

    /// Input while magnifying; `None` when the magnifier is off.
    pub(super) fn magnify_input(&mut self, input: &Input) -> Option<bool> {
        let m = self.magnify?;
        let r = self.show_slide_rect();
        Some(match *input {
            Input::Key(Key::Escape, _) => self.cancel_magnify(),
            Input::Text(ref t) if m.center.is_some() && matches!(t.as_str(), "+" | "=") => {
                self.magnify_step(1)
            }
            Input::Text(ref t) if m.center.is_some() && t == "-" => self.magnify_step(-1),
            Input::Move { x, y } if m.center.is_none() => {
                if let Some(g) = self.magnify.as_mut() {
                    g.pointer = Some((x, y));
                }
                true
            }
            Input::Press { x, y, .. } if m.center.is_none() => {
                let f = 2f32.powf(f32::from(LENS_LEVEL) / 3.0);
                let c = clamp_center((x - r[0]) / r[2].max(1.0), (y - r[1]) / r[3].max(1.0), f);
                if let Some(g) = self.magnify.as_mut() {
                    g.center = Some(c);
                    g.level = LENS_LEVEL;
                    g.pointer = None;
                }
                true
            }
            Input::Press { x, y, .. } => {
                if let Some(g) = self.magnify.as_mut() {
                    g.drag = Some((x, y));
                }
                false
            }
            Input::Drag { x, y } => {
                let (Some((cu, cv)), Some((dx0, dy0))) = (m.center, m.drag) else {
                    return Some(false);
                };
                // The image is f times the slide: a pointer move of d pans d / f slide widths.
                let f = m.factor();
                let c = clamp_center(
                    cu - (x - dx0) / (f * r[2].max(1.0)),
                    cv - (y - dy0) / (f * r[3].max(1.0)),
                    f,
                );
                if let Some(g) = self.magnify.as_mut() {
                    g.center = Some(c);
                    g.drag = Some((x, y));
                }
                true
            }
            Input::Release { .. } => {
                if let Some(g) = self.magnify.as_mut() {
                    g.drag = None;
                }
                false
            }
            // Other keys and the wheel do nothing while magnifying.
            _ => false,
        })
    }

    /// The zoomed image of the shown slide while a part is magnified.
    pub(super) fn prepare_magnify(&mut self, store: &mut dyn PageStore) -> Result<(), String> {
        let Some(i) = self.show_slide() else {
            return Ok(());
        };
        let key = MAG_KEY + self.ids[i];
        if let Some(m) = self.magnify.filter(|m| m.center.is_some()) {
            let scale = self.show_scale() * m.factor();
            self.ensure(store, key, i, scale)?;
        } else {
            let old: Vec<usize> = self
                .shown
                .keys()
                .copied()
                .filter(|k| k & !((1 << 40) - 1) == MAG_KEY)
                .collect();
            for k in old {
                store.remove(k);
                self.shown.remove(&k);
            }
        }
        Ok(())
    }

    /// Paint the lens or the magnified part over the show.
    pub(super) fn paint_magnify(&self, p: &mut dyn Painter) {
        let Some(m) = self.magnify else { return };
        let Some(i) = self.show_slide() else { return };
        let r: R = self.show_slide_rect();
        if let Some((u, v)) = m.center {
            let key = MAG_KEY + self.ids[i];
            if self.shown.contains_key(&key) {
                let f = m.factor();
                let src = [
                    (u * f * r[2] - r[2] / 2.0).round(),
                    (v * f * r[3] - r[3] / 2.0).round(),
                    r[2],
                    r[3],
                ];
                p.page_part(key, src, r, 1.0);
            }
            return;
        }
        // Choosing: the slide dimmed outside a lens of half its size at the pointer.
        let (lw, lh) = ((r[2] / 2.0).round(), (r[3] / 2.0).round());
        let (px, py) = m.pointer.unwrap_or((r[0] + r[2] / 2.0, r[1] + r[3] / 2.0));
        let lx = (px - lw / 2.0).clamp(r[0], r[0] + r[2] - lw).round();
        let ly = (py - lh / 2.0).clamp(r[1], r[1] + r[3] - lh).round();
        let dim = Rgba(0, 0, 0, 110);
        p.fill_rect(r[0], r[1], r[2], ly - r[1], dim);
        p.fill_rect(r[0], ly + lh, r[2], r[1] + r[3] - ly - lh, dim);
        p.fill_rect(r[0], ly, lx - r[0], lh, dim);
        p.fill_rect(lx + lw, ly, r[0] + r[2] - lx - lw, lh, dim);
        // The lens frame as one stroked path.
        let one = self.display_scale.round().max(1.0);
        let h = one / 2.0;
        let frame = [
            PathSeg::M(lx + h, ly + h),
            PathSeg::L(lx + lw - h, ly + h),
            PathSeg::L(lx + lw - h, ly + lh - h),
            PathSeg::L(lx + h, ly + lh - h),
            PathSeg::Z,
        ];
        p.path(&frame, None, Some((Rgba(255, 255, 255, 255), one)));
    }
}
