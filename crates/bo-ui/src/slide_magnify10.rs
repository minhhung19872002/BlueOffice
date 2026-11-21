//! Magnify Slide while presenting (round 10 resume of the slide series), as the reference's
//! right-click menu entry: a lens of half the slide's size follows the pointer over a dimmed
//! slide, a click shows that part at twice the size (drawn sharp from an image rendered at
//! twice the show's scale), dragging pans, Esc or a right click goes back.
use super::{SlideView, R};
use crate::{Input, Key, PageStore, Painter};
use bo_render::Rgba;

/// Image key of the magnified slide (outside the other kinds of the view).
const MAG_KEY: usize = 22 << 40;

/// The magnifier's state.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Magnify {
    /// Pointer (window px) while choosing the part.
    pub pointer: Option<(f32, f32)>,
    /// Centre of the magnified part in fractions of the slide (0.25-0.75), once chosen.
    pub center: Option<(f32, f32)>,
    /// Last drag position while panning.
    pub drag: Option<(f32, f32)>,
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

    /// "+" while presenting: twice the size about the slide's centre.
    pub fn magnify_center(&mut self) -> bool {
        if self.show_slide().is_none() {
            return false;
        }
        self.magnify = Some(Magnify {
            center: Some((0.5, 0.5)),
            ..Magnify::default()
        });
        true
    }

    /// Back to the whole slide.
    pub fn cancel_magnify(&mut self) -> bool {
        self.magnify.take().is_some()
    }

    fn clamp_center(u: f32, v: f32) -> (f32, f32) {
        (u.clamp(0.25, 0.75), v.clamp(0.25, 0.75))
    }

    /// Input while magnifying; `None` when the magnifier is off.
    pub(super) fn magnify_input(&mut self, input: &Input) -> Option<bool> {
        let m = self.magnify?;
        let r = self.show_slide_rect();
        Some(match *input {
            Input::Key(Key::Escape, _) => self.cancel_magnify(),
            Input::Move { x, y } if m.center.is_none() => {
                if let Some(g) = self.magnify.as_mut() {
                    g.pointer = Some((x, y));
                }
                true
            }
            Input::Press { x, y, .. } if m.center.is_none() => {
                let c = Self::clamp_center((x - r[0]) / r[2].max(1.0), (y - r[1]) / r[3].max(1.0));
                if let Some(g) = self.magnify.as_mut() {
                    g.center = Some(c);
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
                // The image is twice the slide: a pointer move of d pans d / 2 slide widths.
                let c = Self::clamp_center(
                    cu - (x - dx0) / (2.0 * r[2].max(1.0)),
                    cv - (y - dy0) / (2.0 * r[3].max(1.0)),
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
            // Keys and the wheel do nothing else while magnifying.
            _ => false,
        })
    }

    /// The twice-as-large image of the shown slide while a part is magnified.
    pub(super) fn prepare_magnify(&mut self, store: &mut dyn PageStore) -> Result<(), String> {
        let Some(i) = self.show_slide() else {
            return Ok(());
        };
        let key = MAG_KEY + self.ids[i];
        if self.magnify.is_some_and(|m| m.center.is_some()) {
            let scale = self.show_scale() * 2.0;
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
                let src = [
                    (u * 2.0 * r[2] - r[2] / 2.0).round(),
                    (v * 2.0 * r[3] - r[3] / 2.0).round(),
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
        // The lens frame as one stroked path (separate thin rectangles can be merged into
        // their bounding box by a backend).
        let one = self.display_scale.round().max(1.0);
        let h = one / 2.0;
        let frame = [
            bo_render::PathSeg::M(lx + h, ly + h),
            bo_render::PathSeg::L(lx + lw - h, ly + h),
            bo_render::PathSeg::L(lx + lw - h, ly + lh - h),
            bo_render::PathSeg::L(lx + h, ly + lh - h),
            bo_render::PathSeg::Z,
        ];
        p.path(&frame, None, Some((Rgba(255, 255, 255, 255), one)));
    }
}
