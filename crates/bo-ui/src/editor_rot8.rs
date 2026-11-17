//! BlueEditor round 8: the rotation handle of Edit Object, as the reference editor shows it
//! (black-box: a small round handle above the middle of the selection's top edge, a dashed
//! line down to its centre; dragging it turns the selection about its centre, Shift in 15°
//! steps; one undo step).
use crate::editor_arrange::union_box;
use crate::editor_window::{EditTool, EditorWindow};
use crate::{Input, Painter};
use bo_render::{PathSeg, Rgba};

/// The reference's selection blue of Edit Object (frame, handles, turn mark): `#2E6DAD`.
pub(crate) const SEL_BLUE: Rgba = Rgba(0x2E, 0x6D, 0xAD, 255);

/// Path of a circular arc from `a0` to `a1` degrees (clockwise on screen, y down).
pub(crate) fn arc(cx: f32, cy: f32, r: f32, a0: f32, a1: f32) -> Vec<PathSeg> {
    let n = ((a1 - a0).abs() / 90.0).ceil().max(1.0);
    let step = (a1 - a0) / n;
    let pt = |a: f32| {
        let (s, c) = a.to_radians().sin_cos();
        (cx + r * c, cy + r * s)
    };
    let (x, y) = pt(a0);
    let mut v = vec![PathSeg::M(x, y)];
    let t = 4.0 / 3.0 * (step.to_radians() / 4.0).tan();
    let mut a = a0;
    // n is a small whole number of quarter turns at most.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    for _ in 0..(n as usize) {
        let b = a + step;
        let (p0, p1) = (pt(a), pt(b));
        let (s0, c0) = a.to_radians().sin_cos();
        let (s1, c1) = b.to_radians().sin_cos();
        v.push(PathSeg::C(
            p0.0 - t * r * s0,
            p0.1 + t * r * c0,
            p1.0 + t * r * s1,
            p1.1 - t * r * c1,
            p1.0,
            p1.1,
        ));
        a = b;
    }
    v
}

/// A window point.
pub(crate) type Pt = (f32, f32);

/// A rotation being dragged: page, centre (window pixels), angle at the press, angle now.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct RotDrag {
    pub page: usize,
    pub centre: (f32, f32),
    pub from: f32,
    pub to: f32,
    pub shift: bool,
}

/// Matrix turning page points `deg` degrees clockwise (y-down page) about `(cx, cy)`.
pub(crate) fn rotation_deg(deg: f32, cx: f32, cy: f32) -> [f32; 6] {
    let (s, c) = deg.to_radians().sin_cos();
    [c, s, -s, c, cx - c * cx + s * cy, cy - s * cx - c * cy]
}

impl EditorWindow {
    /// Window box of the Edit Object selection and its page.
    fn rot_box(&mut self) -> Option<(usize, [f32; 4], [f32; 4])> {
        if self.edit_tool() != Some(EditTool::Object) || self.edit.typing.is_some() {
            return None;
        }
        let page = self.edit.page?;
        let sel = self.edit.selected.clone();
        if sel.is_empty() {
            return None;
        }
        let v = self.r.view_mut()?;
        let list = self.edit.items(v, page).to_vec();
        let boxes: Vec<[f32; 4]> = sel
            .iter()
            .filter_map(|&k| list.get(k).map(|i| i.bbox))
            .collect();
        let u = union_box(&boxes)?;
        let w = v.doc_rect_to_window(page, u)?;
        Some((page, u, w))
    }

    /// Centre of the rotation handle (window pixels).
    pub(crate) fn rot_handle(&mut self) -> Option<(usize, Pt, Pt)> {
        let (page, _, w) = self.rot_box()?;
        let k = self.ui_scale() / 1.25;
        let centre = (w[0] + w[2] / 2.0, w[1] + w[3] / 2.0);
        Some((page, (centre.0, w[1] - (25.0 * k).round()), centre))
    }

    /// Pointer input on the rotation handle; true when used.
    pub(crate) fn rot8_input(&mut self, input: &Input) -> bool {
        match *input {
            Input::Press { x, y, shift, .. } => {
                let Some((page, h, c)) = self.rot_handle() else {
                    return false;
                };
                let r = (8.0 * self.ui_scale() / 1.25).max(6.0);
                if (x - h.0).hypot(y - h.1) > r {
                    return false;
                }
                let a = (y - c.1).atan2(x - c.0).to_degrees();
                self.rot8 = Some(RotDrag {
                    page,
                    centre: c,
                    from: a,
                    to: a,
                    shift,
                });
                true
            }
            Input::Drag { x, y } => {
                let Some(d) = self.rot8.as_mut() else {
                    return false;
                };
                d.to = (y - d.centre.1).atan2(x - d.centre.0).to_degrees();
                true
            }
            Input::Release { .. } => {
                let Some(d) = self.rot8.take() else {
                    return false;
                };
                let mut deg = d.to - d.from;
                if d.shift {
                    deg = (deg / 15.0).round() * 15.0;
                }
                if deg.abs() > 0.2 {
                    self.rotate_selection(d.page, deg);
                }
                true
            }
            _ => false,
        }
    }

    /// Turn the selected objects `deg` degrees clockwise about the selection's centre.
    pub(crate) fn rotate_selection(&mut self, page: usize, deg: f32) {
        let Some((_, u, _)) = self.rot_box() else {
            return;
        };
        let (cx, cy) = (f32::midpoint(u[0], u[2]), f32::midpoint(u[1], u[3]));
        let m = rotation_deg(deg, cx, cy);
        let n = self.edit.selected.len();
        let Some(v) = self.r.view_mut() else { return };
        let _ = page;
        if self.edit.apply_moves(v, page, &vec![m; n]) {
            self.after_doc_edit();
        }
    }

    /// The handle, its dashed line and, while dragging, the angle.
    pub(crate) fn paint_rot8(&mut self, p: &mut dyn Painter) {
        let Some((_, h, c)) = self.rot_handle() else {
            return;
        };
        let k = self.ui_scale() / 1.25;
        let blue = SEL_BLUE;
        // The turn mark: an open ring with an arrow head (our own drawing), about 20 px
        // across at 125 % like the reference's handle.
        let r = 8.5 * k;
        let w = (2.4 * k).max(1.0);
        p.path(&arc(h.0, h.1, r, -60.0, 230.0), None, Some((blue, w)));
        // The arrow head at the open end (pointing on, clockwise).
        let (s, c0) = (-60.0_f32).to_radians().sin_cos();
        let (ex, ey) = (h.0 + r * c0, h.1 + r * s);
        p.path(
            &[
                PathSeg::M(ex - 4.2 * k, ey - 3.6 * k),
                PathSeg::L(ex + 3.4 * k, ey - 4.4 * k),
                PathSeg::L(ex + 1.2 * k, ey + 3.4 * k),
                PathSeg::Z,
            ],
            Some(blue),
            None,
        );
        // Dashed line from the mark to the centre: 15 px dashes, 5 px gaps at 125 %.
        let mut y = (h.1 + r + w / 2.0 + 1.0).round();
        let x = c.0.round();
        while y < c.1 - 1.0 {
            p.fill_rect(x, y, 1.0, (15.0 * k).min(c.1 - y), blue);
            y += 20.0 * k;
        }
        // The centre: a small diamond.
        let d = 4.5 * k;
        let (cx, cy) = (x + 0.5, c.1.round() + 0.5);
        p.path(
            &[
                PathSeg::M(cx, cy - d),
                PathSeg::L(cx + d, cy),
                PathSeg::L(cx, cy + d),
                PathSeg::L(cx - d, cy),
                PathSeg::Z,
            ],
            Some(blue),
            None,
        );
        if let Some(d) = self.rot8 {
            let deg = d.to - d.from;
            let deg = if d.shift {
                (deg / 15.0).round() * 15.0
            } else {
                deg
            };
            let style = crate::TextStyle {
                size: (12.0 * k).round(),
                bold: false,
            };
            p.text(
                &format!("{deg:.0}°"),
                h.0 + 12.0 * k,
                h.1 - 9.0 * k,
                style,
                blue,
            );
        }
    }
}
