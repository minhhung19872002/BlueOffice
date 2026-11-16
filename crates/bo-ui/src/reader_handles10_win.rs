//! BlueReader round 10 (a child of `reader_popups10_win`): the selected comment's frame and
//! handles as the reference reader draws them (measured at 125 %): a dotted outline, round
//! blue handles (12 device px, 51, 142, 244 with a white edge) at the corners and the middle of
//! each side, and a rotate handle on a stalk above the top edge; lines show a handle at each
//! end instead. Handles resize (Shift keeps the proportions), move a line's ends and turn the
//! comment; every change is one undo step written by File › Save.
use super::super::{inside, ReaderWindow, Tool};
use crate::{Input, Painter};
use bo_pdfread::{Annot, Shape};
use bo_render::{PathSeg, Rgba};

const HANDLE_R: f32 = 4.8;
const HANDLE_FILL: Rgba = Rgba(51, 142, 244, 255);
const HANDLE_EDGE: Rgba = Rgba(255, 255, 255, 255);
const STALK: Rgba = Rgba(38, 128, 235, 255);
/// The rotate handle's centre above the top edge, and its circle's radius.
const ROTATE_UP: f32 = 22.4;
const ROTATE_R: f32 = 6.4;
/// Least size of a resized comment (page points).
const MIN_SIZE: f32 = 4.0;

/// A handle of the selected comment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Grip {
    /// Box handle 0–7: top left, top, top right, right, bottom right, bottom, bottom left,
    /// left.
    Box(u8),
    /// End `i` of a line.
    End(usize),
    /// The rotate handle.
    Rotate,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct HandleDrag {
    a: Annot,
    grip: Grip,
    from: (f32, f32),
    to: (f32, f32),
}

/// The two ends of a line or distance measurement.
fn line_ends(s: &Shape) -> Option<[[f32; 2]; 2]> {
    match s {
        Shape::Line { a, b, .. } | Shape::Distance(a, b) => Some([*a, *b]),
        _ => None,
    }
}

fn with_end(s: &Shape, i: usize, p: [f32; 2]) -> Option<Shape> {
    Some(match s {
        Shape::Line { a, b, arrow } => Shape::Line {
            a: if i == 0 { p } else { *a },
            b: if i == 1 { p } else { *b },
            arrow: *arrow,
        },
        Shape::Distance(a, b) => {
            Shape::Distance(if i == 0 { p } else { *a }, if i == 1 { p } else { *b })
        }
        _ => return None,
    })
}

/// Window box `b` (x, y, w, h) with handle `i` dragged by (dx, dy); `ratio` keeps the
/// proportions when a corner moves.
pub(crate) fn resized_box(b: [f32; 4], i: u8, dx: f32, dy: f32, ratio: bool) -> [f32; 4] {
    let (mut l, mut t, mut r, mut bo) = (b[0], b[1], b[0] + b[2], b[1] + b[3]);
    match i {
        0 => (l, t) = (l + dx, t + dy),
        1 => t += dy,
        2 => (r, t) = (r + dx, t + dy),
        3 => r += dx,
        4 => (r, bo) = (r + dx, bo + dy),
        5 => bo += dy,
        6 => (l, bo) = (l + dx, bo + dy),
        _ => l += dx,
    }
    if ratio && i.is_multiple_of(2) && b[2] > 0.0 && b[3] > 0.0 {
        // The larger change wins; the corner opposite the handle stays.
        let k = ((r - l) / b[2]).abs().max(((bo - t) / b[3]).abs());
        let (w, h) = (b[2] * k, b[3] * k);
        match i {
            0 => (l, t) = (r - w, bo - h),
            2 => (r, t) = (l + w, bo - h),
            4 => (r, bo) = (l + w, t + h),
            _ => (l, bo) = (r - w, t + h),
        }
    }
    [l.min(r), t.min(bo), (r - l).abs(), (bo - t).abs()]
}

impl ReaderWindow {
    /// BlueReader draws the selected comment's handles itself (BlueEditor keeps its own).
    pub(crate) fn handles10_on(&self) -> bool {
        self.profile.is_none()
            && self
                .view()
                .is_some_and(|v| v.rotation().is_multiple_of(360))
    }

    /// Window box of the selected comment (x, y, w, h).
    fn sel_box10(&self, a: &Annot) -> Option<[f32; 4]> {
        let t = self.tabs.get(self.active)?;
        let w = t.view.doc_rect_to_window(a.page, self.shown_rect11(a))?;
        Some(w.map(f32::round))
    }

    /// The selected comment's line ends in window points, when it is a line.
    fn ends10(&self, a: &Annot) -> Option<[(f32, f32); 2]> {
        let t = self.tabs.get(self.active)?;
        let (s, _, _) = t.view.doc().drawing(a)?;
        let e = line_ends(&s)?;
        let w = |p: [f32; 2]| {
            t.view
                .doc_rect_to_window(a.page, [p[0], p[1], p[0], p[1]])
                .map(|r| (r[0], r[1]))
        };
        Some([w(e[0])?, w(e[1])?])
    }

    /// Handle centres of the selected comment.
    fn grips10(&self, a: &Annot) -> Vec<(Grip, (f32, f32))> {
        if let Some(e) = self.ends10(a) {
            return vec![(Grip::End(0), e[0]), (Grip::End(1), e[1])];
        }
        let Some(b) = self.sel_box10(a) else {
            return Vec::new();
        };
        // Round 11: NoZoom notes have no handles; Fill & Sign marks one corner (ratio kept).
        match self.grip_mode11(a) {
            super::super::fill11::GripMode::Frame => return Vec::new(),
            super::super::fill11::GripMode::Corner => {
                return vec![(Grip::Box(4), (b[0] + b[2], b[1] + b[3]))];
            }
            super::super::fill11::GripMode::Full => {}
        }
        let (l, t, r, bo) = (b[0], b[1], b[0] + b[2], b[1] + b[3]);
        let (cx, cy) = (l.midpoint(r), t.midpoint(bo));
        let mut v: Vec<(Grip, (f32, f32))> = [
            (l, t),
            (cx, t),
            (r, t),
            (r, cy),
            (r, bo),
            (cx, bo),
            (l, bo),
            (l, cy),
        ]
        .into_iter()
        .enumerate()
        .map(|(i, p)| (Grip::Box(i as u8), p))
        .collect();
        if self.view().is_some_and(|v| v.doc().can_rotate(a)) {
            v.push((Grip::Rotate, (cx, t - self.px(ROTATE_UP))));
        }
        v
    }

    fn grip_at10(&self, x: f32, y: f32) -> Option<Grip> {
        let a = self.extra.selected.as_ref()?;
        let reach = self.px(HANDLE_R) + 2.0;
        self.grips10(a)
            .into_iter()
            .find(|(g, (gx, gy))| {
                let r = if *g == Grip::Rotate {
                    self.px(ROTATE_R) + 2.0
                } else {
                    reach
                };
                (x - gx).abs() <= r && (y - gy).abs() <= r
            })
            .map(|(g, _)| g)
    }

    /// Input of the handles: `Some(redraw)` when handled.
    pub(crate) fn route_handles10(&mut self, input: &Input) -> Option<bool> {
        if !self.handles10_on() {
            return None;
        }
        match *input {
            Input::Press { x, y, .. }
                if matches!(self.tool, Tool::Hand | Tool::Select) && self.draw.tool.is_none() =>
            {
                let grip = self.grip_at10(x, y)?;
                let a = self.extra.selected.clone()?;
                self.ten.handle = Some(HandleDrag {
                    a,
                    grip,
                    from: (x, y),
                    to: (x, y),
                });
                Some(true)
            }
            Input::Drag { x, y } => {
                let d = self.ten.handle.as_mut()?;
                d.to = (x, y);
                Some(true)
            }
            Input::Release { x, y } => {
                let mut d = self.ten.handle.take()?;
                d.to = (x, y);
                if (x - d.from.0).abs() + (y - d.from.1).abs() > 1.0 {
                    self.apply_handle10(&d);
                }
                Some(true)
            }
            Input::Move { x, y }
                if self.grip_at10(x, y).is_some() && inside(self.doc_rect, x, y) =>
            {
                self.cursor = 6;
                Some(true)
            }
            _ => None,
        }
    }

    /// The drag's result as a window box, the rotation angle or the moved line end.
    fn rotation10(&self, d: &HandleDrag) -> f32 {
        let Some(b) = self.sel_box10(&d.a) else {
            return 0.0;
        };
        let (cx, cy) = (b[0] + b[2] / 2.0, b[1] + b[3] / 2.0);
        let a0 = (d.from.1 - cy).atan2(d.from.0 - cx);
        let a1 = (d.to.1 - cy).atan2(d.to.0 - cx);
        let mut angle = a1 - a0;
        if self.ten.shift {
            let step = std::f32::consts::PI / 12.0;
            angle = (angle / step).round() * step;
        }
        angle
    }

    fn apply_handle10(&mut self, d: &HandleDrag) {
        let Some(t) = self.tabs.get(self.active) else {
            return;
        };
        let Some(pr) = t.view.page_rect(d.a.page) else {
            return;
        };
        let s = t.view.px_per_pt().max(0.01);
        let to_page = |x: f32, y: f32| [(x - pr[0]) / s, (y - pr[1]) / s];
        let (dx, dy) = (d.to.0 - d.from.0, d.to.1 - d.from.1);
        let shift =
            self.ten.shift || self.grip_mode11(&d.a) == super::super::fill11::GripMode::Corner;
        let target = match d.grip {
            Grip::Box(i) => {
                let Some(b) = self.sel_box10(&d.a) else {
                    return;
                };
                let nb = resized_box(b, i, dx, dy, shift);
                let p0 = to_page(nb[0], nb[1]);
                let p1 = to_page(nb[0] + nb[2], nb[1] + nb[3]);
                let rect = [
                    p0[0],
                    p0[1],
                    p1[0].max(p0[0] + MIN_SIZE),
                    p1[1].max(p0[1] + MIN_SIZE),
                ];
                let r = self
                    .tabs
                    .get_mut(self.active)
                    .map(|t| t.view.doc_mut().resize_annotation(&d.a, rect));
                if let Some(Err(e)) = r {
                    self.status = e;
                }
                rect
            }
            Grip::End(i) => {
                let doc = t.view.doc();
                let Some((shape, style, text)) = doc.drawing(&d.a) else {
                    return;
                };
                let Some(shape) = with_end(&shape, i, to_page(d.to.0, d.to.1)) else {
                    return;
                };
                let r = self
                    .tabs
                    .get_mut(self.active)
                    .map(|t| t.view.doc_mut().update_drawing(&d.a, &shape, &style, &text));
                if let Some(Err(e)) = r {
                    self.status = e;
                }
                d.a.rect
            }
            Grip::Rotate => {
                let angle = self.rotation10(d);
                let r = self
                    .tabs
                    .get_mut(self.active)
                    .map(|t| t.view.doc_mut().rotate_drawing(&d.a, angle));
                if let Some(Err(e)) = r {
                    self.status = e;
                }
                d.a.rect
            }
        };
        self.document_changed();
        self.reselect10(d.a.page, target);
    }

    /// Select the comment of `page` whose box is closest to `rect`.
    fn reselect10(&mut self, page: usize, rect: [f32; 4]) {
        let Some(t) = self.tabs.get(self.active) else {
            return;
        };
        let dist = |a: &Annot| {
            a.rect
                .iter()
                .zip(rect)
                .map(|(x, y)| (x - y).abs())
                .sum::<f32>()
        };
        self.extra.selected = t
            .view
            .doc()
            .annotations()
            .into_iter()
            .filter(|a| a.page == page)
            .min_by(|a, b| dist(a).total_cmp(&dist(b)));
    }

    /// Draw the selected comment's outline and handles (or the drag in progress).
    pub(crate) fn paint_handles10(&self, p: &mut dyn Painter) {
        if !self.handles10_on() {
            return;
        }
        let Some(a) = self.extra.selected.as_ref() else {
            return;
        };
        let d = self.doc_rect;
        p.push_clip(d[0], d[1], d[2], d[3]);
        let (mx, my) = self
            .extra
            .moving
            .map_or((0.0, 0.0), |(x0, y0, x1, y1)| (x1 - x0, y1 - y0));
        let drag = self.ten.handle.as_ref();
        if let Some(e) = self.ends10(a) {
            let mut e = e.map(|(x, y)| (x + mx, y + my));
            if let Some(HandleDrag {
                grip: Grip::End(i),
                to,
                ..
            }) = drag
            {
                e[*i] = *to;
            }
            if drag.is_some() {
                p.path(
                    &[PathSeg::M(e[0].0, e[0].1), PathSeg::L(e[1].0, e[1].1)],
                    None,
                    Some((HANDLE_FILL, 1.0)),
                );
            }
            for (x, y) in e {
                self.paint_grip10(p, x, y);
            }
            p.pop_clip();
            return;
        }
        let Some(mut b) = self.sel_box10(a) else {
            p.pop_clip();
            return;
        };
        b[0] += mx;
        b[1] += my;
        match drag {
            Some(
                dr @ HandleDrag {
                    grip: Grip::Rotate, ..
                },
            ) => {
                // The box turned about its centre.
                let angle = self.rotation10(dr);
                let (cx, cy) = (b[0] + b[2] / 2.0, b[1] + b[3] / 2.0);
                let (s, k) = angle.sin_cos();
                let pt = |x: f32, y: f32| {
                    let (dx, dy) = (x - cx, y - cy);
                    (cx + dx * k - dy * s, cy + dx * s + dy * k)
                };
                let c = [
                    pt(b[0], b[1]),
                    pt(b[0] + b[2], b[1]),
                    pt(b[0] + b[2], b[1] + b[3]),
                    pt(b[0], b[1] + b[3]),
                ];
                let mut segs = vec![PathSeg::M(c[0].0, c[0].1)];
                segs.extend(c[1..].iter().map(|q| PathSeg::L(q.0, q.1)));
                segs.push(PathSeg::Z);
                p.path(&segs, None, Some((HANDLE_FILL, 1.0)));
                p.pop_clip();
                return;
            }
            Some(HandleDrag {
                grip: Grip::Box(i),
                from,
                to,
                ..
            }) => {
                let ratio =
                    self.ten.shift || self.grip_mode11(a) == super::super::fill11::GripMode::Corner;
                b = resized_box(b, *i, to.0 - from.0, to.1 - from.1, ratio);
            }
            _ => {}
        }
        if self.paint_grips11(p, a, b) {
            p.pop_clip();
            return;
        }
        Self::paint_dotted10(p, b);
        let (l, t, r, bo) = (b[0], b[1], b[0] + b[2], b[1] + b[3]);
        let (cx, cy) = (l.midpoint(r), t.midpoint(bo));
        if self.view().is_some_and(|v| v.doc().can_rotate(a)) {
            let ry = t - self.px(ROTATE_UP);
            let rr = self.px(ROTATE_R);
            p.fill_rect(cx.round() - 1.0, ry + rr, 2.0, t - ry - rr, STALK);
            self.paint_rotate10(p, cx, ry, rr);
        }
        for (x, y) in [
            (l, t),
            (cx, t),
            (r, t),
            (r, cy),
            (r, bo),
            (cx, bo),
            (l, bo),
            (l, cy),
        ] {
            self.paint_grip10(p, x, y);
        }
        p.pop_clip();
    }

    /// A dotted outline (2 px on, 2 px off) on the box edges.
    fn paint_dotted10(p: &mut dyn Painter, b: [f32; 4]) {
        let (l, t, r, bo) = (b[0], b[1], b[0] + b[2], b[1] + b[3]);
        let mut x = l;
        while x < r {
            let w = 2.0_f32.min(r - x);
            p.fill_rect(x, t, w, 1.0, HANDLE_FILL);
            p.fill_rect(x, bo - 1.0, w, 1.0, HANDLE_FILL);
            x += 4.0;
        }
        let mut y = t;
        while y < bo {
            let h = 2.0_f32.min(bo - y);
            p.fill_rect(l, y, 1.0, h, HANDLE_FILL);
            p.fill_rect(r - 1.0, y, 1.0, h, HANDLE_FILL);
            y += 4.0;
        }
    }

    fn paint_grip10(&self, p: &mut dyn Painter, x: f32, y: f32) {
        let r = self.px(HANDLE_R) + 0.5;
        p.path(&circle(x, y, r + 1.0), Some(HANDLE_EDGE), None);
        p.path(&circle(x, y, r), Some(HANDLE_FILL), None);
    }

    /// The rotate handle: an open circle with an arrow head (our own drawing).
    fn paint_rotate10(&self, p: &mut dyn Painter, cx: f32, cy: f32, r: f32) {
        let w = self.px(1.6).max(1.0);
        let n = 20;
        let (start, sweep) = (-1.2_f32, 5.2_f32);
        let mut segs = Vec::with_capacity(n + 1);
        for i in 0..=n {
            let t = start + sweep * i as f32 / n as f32;
            let (px, py) = (cx + r * t.cos(), cy + r * t.sin());
            segs.push(if i == 0 {
                PathSeg::M(px, py)
            } else {
                PathSeg::L(px, py)
            });
        }
        p.path(&segs, None, Some((STALK, w)));
        let (ex, ey) = (cx + r * start.cos(), cy + r * start.sin());
        let h = self.px(3.2);
        p.path(
            &[
                PathSeg::M(ex - h, ey - h * 0.6),
                PathSeg::L(ex + h * 0.6, ey - h * 0.2),
                PathSeg::L(ex - h * 0.2, ey + h),
                PathSeg::Z,
            ],
            Some(STALK),
            None,
        );
    }

    /// Shift held on the pointer (the host passes it before presses and drags).
    pub(crate) fn set_pointer_mods10(&mut self, m: crate::Mods) {
        self.ten.shift = m.shift;
    }
}

fn circle(cx: f32, cy: f32, r: f32) -> Vec<PathSeg> {
    let k = 0.552_284_8 * r;
    vec![
        PathSeg::M(cx + r, cy),
        PathSeg::C(cx + r, cy + k, cx + k, cy + r, cx, cy + r),
        PathSeg::C(cx - k, cy + r, cx - r, cy + k, cx - r, cy),
        PathSeg::C(cx - r, cy - k, cx - k, cy - r, cx, cy - r),
        PathSeg::C(cx + k, cy - r, cx + r, cy - k, cx + r, cy),
        PathSeg::Z,
    ]
}

#[cfg(test)]
mod tests {
    use super::{resized_box, Grip};
    use crate::reader_tests::{pdf, Fixed};
    use crate::reader_window::ReaderWindow;
    use crate::{Input, Mods};
    use bo_pdfread::{AnnotKind, DrawStyle, PdfDocument, Shape};

    fn window() -> ReaderWindow {
        let doc = PdfDocument::open(pdf(1), bo_pdfread::standard_fonts()).expect("pdf");
        let mut w = ReaderWindow::new("en-US");
        w.handle(Input::Scale(1.25), &Fixed);
        w.handle(
            Input::Resize {
                width: 1500.0,
                height: 1100.0,
            },
            &Fixed,
        );
        w.open(doc, "test.pdf", None);
        w
    }

    fn drag(w: &mut ReaderWindow, a: (f32, f32), b: (f32, f32)) {
        w.handle(
            Input::Press {
                x: a.0,
                y: a.1,
                count: 1,
                shift: false,
            },
            &Fixed,
        );
        w.handle(Input::Drag { x: b.0, y: b.1 }, &Fixed);
        w.handle(Input::Release { x: b.0, y: b.1 }, &Fixed);
    }

    #[test]
    fn corners_keep_the_ratio_with_shift() {
        let b = [100.0, 100.0, 200.0, 100.0];
        assert_eq!(
            resized_box(b, 4, 50.0, 0.0, false),
            [100.0, 100.0, 250.0, 100.0]
        );
        assert_eq!(
            resized_box(b, 4, 50.0, 0.0, true),
            [100.0, 100.0, 250.0, 125.0]
        );
        assert_eq!(
            resized_box(b, 1, 0.0, -20.0, true),
            [100.0, 80.0, 200.0, 120.0]
        );
        assert_eq!(
            resized_box(b, 0, -100.0, 0.0, true),
            [0.0, 50.0, 300.0, 150.0]
        );
    }

    #[test]
    fn handles_resize_turn_and_move_line_ends() {
        let mut w = window();
        let style = DrawStyle::default();
        let doc = w.view_mut().expect("view").doc_mut();
        doc.add_drawing(
            0,
            &Shape::Rect([100.0, 200.0, 200.0, 250.0]),
            &style,
            "me",
            "",
        )
        .expect("rect");
        doc.add_drawing(
            0,
            &Shape::Line {
                a: [100.0, 400.0],
                b: [200.0, 400.0],
                arrow: false,
            },
            &style,
            "me",
            "",
        )
        .expect("line");
        let all = w.view().expect("view").doc().annotations();
        // The rectangle: eight box handles and a rotate handle.
        w.extra_select(all[0].clone());
        let g = w.grips10(&all[0]);
        assert_eq!(g.len(), 9);
        assert!(g.iter().any(|(k, _)| *k == Grip::Rotate));
        // Drag the bottom right handle 50 px right, Shift held: the ratio stays.
        let br = g
            .iter()
            .find(|(k, _)| *k == Grip::Box(4))
            .expect("corner")
            .1;
        w.set_pointer_mods10(Mods {
            shift: true,
            ..Mods::default()
        });
        drag(&mut w, br, (br.0 + 50.0, br.1));
        w.set_pointer_mods10(Mods::default());
        let a = w.extra_selected().expect("still selected");
        let (wd, ht) = (a.rect[2] - a.rect[0], a.rect[3] - a.rect[1]);
        assert!((wd / ht - 2.0).abs() < 0.1, "{:?}", a.rect);
        assert!(wd > 110.0, "{:?}", a.rect);
        // Turn it a quarter: a polygon.
        let g = w.grips10(&a);
        let rot = g
            .iter()
            .find(|(k, _)| *k == Grip::Rotate)
            .expect("rotate")
            .1;
        let b = w.sel_box10(&a).expect("box");
        let (cx, cy) = (b[0] + b[2] / 2.0, b[1] + b[3] / 2.0);
        // From above the centre to its right: 90° clockwise.
        let r = cy - rot.1;
        drag(&mut w, rot, (cx + r, cy));
        let turned = w.extra_selected().expect("selected");
        assert_eq!(turned.kind, AnnotKind::Line, "{turned:?}");
        // The line: two end handles; dragging one moves that end only.
        let line = w
            .view()
            .expect("view")
            .doc()
            .annotations()
            .into_iter()
            .find(|x| x.rect[1] > 380.0)
            .expect("line");
        w.extra_select(line.clone());
        let g = w.grips10(&line);
        assert_eq!(g.len(), 2);
        let end = g[1].1;
        drag(&mut w, end, (end.0, end.1 + 60.0));
        let line = w.extra_selected().expect("line selected");
        let Some((Shape::Line { a, b, .. }, _, _)) = w.view().expect("v").doc().drawing(&line)
        else {
            panic!("line");
        };
        assert!((a[1] - 400.0).abs() < 0.1, "{a:?}");
        assert!(b[1] > 420.0, "{b:?}");
        // Saved and read back.
        let saved = w.view_mut().expect("v").doc_mut().save().expect("save");
        let again = PdfDocument::open(saved, bo_pdfread::standard_fonts()).expect("reopen");
        assert_eq!(again.annotations().len(), 2);
    }
}
