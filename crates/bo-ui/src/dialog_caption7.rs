//! BlueEditor round 7 (a child of `dialog`): dialogs drawn like the reference editor's own
//! (black-box, `docs/ui/REPORT-editor.md` round 7; 125 % pixels / 1.25 here): a window caption
//! 25.6 high in `#F2F3F3` with the title at x 5.6 and a close mark, a white body, and push buttons
//! 26.4 high, at least 107.2 wide, corners 3.2, face `#FDFDFD`, frame `#D0D0D0` darker at the
//! bottom (`#BABABA`), the default button framed in the focus blue. Opt-in per dialog
//! ([`Dialog::with_caption`]); other apps' dialogs are unchanged.
use super::{Dialog, R};
use crate::theme::Theme;
use crate::Key;
use crate::{Painter, TextMeasure, TextStyle};
use bo_render::{PathSeg, Rgba};

const CAPTION: Rgba = Rgba(0xF2, 0xF3, 0xF3, 255);
const BODY: Rgba = Rgba(255, 255, 255, 255);
const FRAME: Rgba = Rgba(0xA0, 0xA0, 0xA0, 255);
const FACE: Rgba = Rgba(0xFD, 0xFD, 0xFD, 255);
const FACE_HOVER: Rgba = Rgba(0xF5, 0xF5, 0xF5, 255);
const EDGE: Rgba = Rgba(0xD0, 0xD0, 0xD0, 255);
const EDGE_BOTTOM: Rgba = Rgba(0xBA, 0xBA, 0xBA, 255);
const FOCUS: Rgba = Rgba(0x00, 0x78, 0xD4, 255);
const INK: Rgba = Rgba(0, 0, 0, 255);
const MARK: Rgba = Rgba(0x40, 0x40, 0x40, 255);

/// Caption height (logical pixels).
const CAP_H: f32 = 25.6;

fn rounded(x: f32, y: f32, w: f32, h: f32, r: f32) -> Vec<PathSeg> {
    let k = r * 0.448;
    vec![
        PathSeg::M(x + r, y),
        PathSeg::L(x + w - r, y),
        PathSeg::C(x + w - k, y, x + w, y + k, x + w, y + r),
        PathSeg::L(x + w, y + h - r),
        PathSeg::C(x + w, y + h - k, x + w - k, y + h, x + w - r, y + h),
        PathSeg::L(x + r, y + h),
        PathSeg::C(x + k, y + h, x, y + h - k, x, y + h - r),
        PathSeg::L(x, y + r),
        PathSeg::C(x, y + k, x + k, y, x + r, y),
        PathSeg::Z,
    ]
}

impl Dialog {
    /// Draw this dialog like the reference editor's (caption, white body, its buttons).
    #[must_use]
    pub fn with_caption(mut self) -> Self {
        self.caption = true;
        self
    }

    /// Switch the reference editor's look on (lay it out again afterwards).
    pub fn set_caption(&mut self) {
        self.caption = true;
    }

    /// Whether the dialog is drawn like the reference editor's.
    #[must_use]
    pub fn has_caption(&self) -> bool {
        self.caption
    }

    /// The caption's close box.
    fn close_box(&self) -> R {
        let w = self.px(36.8);
        R {
            x: self.frame.x + self.frame.w - w,
            y: self.frame.y,
            w,
            h: self.px(CAP_H),
        }
    }

    /// Whether (`x`, `y`) is on the caption's close mark.
    pub(super) fn caption_close(&self, x: f32, y: f32) -> bool {
        self.close_box().contains(x, y)
    }

    /// Buttons of the reference's size, right-aligned at the bottom.
    pub(super) fn caption_layout(&mut self, m: &dyn TextMeasure) {
        let scale = self.scale;
        let px = |v: f32| (v * scale).round();
        let style = self.style();
        let bh = px(26.4);
        let by = self.frame.y + self.frame.h - bh - px(12.0);
        let mut bx = self.frame.x + self.frame.w - px(16.0);
        for (label, _, r) in self.buttons.iter_mut().rev() {
            let bw = (m.measure(label, style).0 + px(24.0)).max(px(107.2));
            bx -= bw;
            *r = R {
                x: bx,
                y: by,
                w: bw,
                h: bh,
            };
            bx -= px(8.0);
        }
    }

    /// Frame, white body, caption with the title and the close mark.
    pub(super) fn paint_caption(&self, p: &mut dyn Painter, _t: &Theme) {
        let f = self.frame;
        p.fill_rect(f.x, f.y, f.w, f.h, FRAME);
        p.fill_rect(f.x + 1.0, f.y + 1.0, f.w - 2.0, f.h - 2.0, BODY);
        let ch = self.px(CAP_H);
        p.fill_rect(f.x + 1.0, f.y + 1.0, f.w - 2.0, ch - 1.0, CAPTION);
        let title = TextStyle {
            size: self.px(12.0),
            bold: false,
        };
        p.text(
            &self.title,
            f.x + self.px(5.6),
            f.y + ((ch - title.size * 1.33) / 2.0).round(),
            title,
            INK,
        );
        let c = self.close_box();
        let (cx, cy, r) = (c.x + c.w / 2.0, c.y + c.h / 2.0, self.px(4.4));
        p.path(
            &[
                PathSeg::M(cx - r, cy - r),
                PathSeg::L(cx + r, cy + r),
                PathSeg::M(cx + r, cy - r),
                PathSeg::L(cx - r, cy + r),
            ],
            None,
            Some((MARK, self.scale.max(1.0))),
        );
    }

    /// Push buttons as the reference draws them; the first is the default one.
    pub(super) fn paint_caption_buttons(&self, p: &mut dyn Painter, _t: &Theme) {
        let style = self.style();
        let rad = self.px(3.2);
        for (i, (label, _, r)) in self.buttons.iter().enumerate() {
            let hover = self.hover == Some(i);
            // Frame: lighter sides, a darker bottom line; the default button in the focus blue.
            p.path(
                &rounded(r.x, r.y, r.w, r.h, rad),
                Some(if i == 0 { FOCUS } else { EDGE_BOTTOM }),
                None,
            );
            let inset = if i == 0 { 1.0 } else { 0.0 };
            p.path(
                &rounded(r.x, r.y, r.w, r.h - 1.0, rad),
                Some(if i == 0 { FOCUS } else { EDGE }),
                None,
            );
            p.path(
                &rounded(
                    r.x + 1.0 + inset,
                    r.y + 1.0 + inset,
                    r.w - 2.0 - 2.0 * inset,
                    r.h - 2.0 - 2.0 * inset,
                    (rad - 1.0).max(0.0),
                ),
                Some(if hover { FACE_HOVER } else { FACE }),
                None,
            );
            let tw = label.chars().count() as f32 * style.size * 0.5;
            p.text(
                label,
                (r.x + (r.w - tw) / 2.0).round(),
                r.y + ((r.h - style.size * 1.33) / 2.0).round(),
                style,
                INK,
            );
        }
    }

    /// Make field `i` a drop-down list of `choices` (its value becomes the first one when it
    /// is not among them). Clicking it or Up / Down moves through the list.
    #[must_use]
    pub fn with_choices(mut self, i: usize, choices: Vec<String>) -> Self {
        self.set_choices(i, choices);
        self
    }

    /// Replace the list of field `i` (keeps its value when it is still offered).
    pub fn set_choices(&mut self, i: usize, choices: Vec<String>) {
        let Some(f) = self.fields.get_mut(i) else {
            return;
        };
        let v: String = f.value.iter().collect();
        if !choices.contains(&v) {
            f.value = choices
                .first()
                .map(|c| c.chars().collect())
                .unwrap_or_default();
        }
        f.caret = f.value.len();
        f.choices = choices;
    }

    /// Move drop-down field `i` by `delta` choices (wrapping); false when it is not one.
    pub(super) fn choice_step(&mut self, i: usize, delta: isize) -> bool {
        let Some(f) = self.fields.get_mut(i) else {
            return false;
        };
        let n = f.choices.len();
        if n == 0 {
            return false;
        }
        let v: String = f.value.iter().collect();
        let at = f.choices.iter().position(|c| *c == v).unwrap_or(0);
        #[allow(clippy::cast_possible_wrap, clippy::cast_sign_loss)]
        let next = (at as isize + delta).rem_euclid(n as isize) as usize;
        f.value = f.choices[next].chars().collect();
        f.caret = f.value.len();
        true
    }

    /// Up / Down on a focused drop-down field; true when used.
    pub(super) fn choice_key(&mut self, key: Key) -> bool {
        let delta = match key {
            Key::Down => 1,
            Key::Up => -1,
            _ => return false,
        };
        self.choice_step(self.focus, delta)
    }

    /// The drop-down arrow at the right of a list field.
    pub(super) fn paint_choice_arrow(&self, p: &mut dyn Painter, r: R, ink: Rgba) {
        let s = self.px(4.0);
        let cx = r.x + r.w - self.px(14.0);
        let cy = r.y + r.h / 2.0;
        p.path(
            &[
                PathSeg::M(cx - s, cy - s / 2.0),
                PathSeg::L(cx, cy + s / 2.0),
                PathSeg::L(cx + s, cy - s / 2.0),
            ],
            None,
            Some((ink, self.scale.max(1.0))),
        );
    }
}
