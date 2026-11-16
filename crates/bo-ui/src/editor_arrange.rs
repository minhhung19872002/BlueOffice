//! Edit Object arrangement, as the reference editor's: snapping a moved selection to the page
//! edges and centre and to other objects (with guides), nudging with the arrow keys (1 pt,
//! Shift 10 pt), selecting every object of the page, and the Format pane's "Arrange" section
//! (page centre, alignment, distribution) plus rotating and flipping. Each action is one undo
//! step and the moved objects stay selected.
use crate::editor_edit::{Editing, Item};
use crate::editor_window::{EditTool, EditorWindow};
use crate::reader_icons::{self, Ink};
use crate::reader_view::ReaderView;
use crate::{Key, Mods, Painter, TextMeasure, TextStyle};
use bo_pdfread::{flip_about, rotation_about};
use bo_render::Rgba;

/// Union of boxes (left, top, right, bottom).
pub(crate) fn union_box(boxes: &[[f32; 4]]) -> Option<[f32; 4]> {
    let mut it = boxes.iter();
    let first = *it.next()?;
    Some(it.fold(first, |a, b| {
        [
            a[0].min(b[0]),
            a[1].min(b[1]),
            a[2].max(b[2]),
            a[3].max(b[3]),
        ]
    }))
}

/// Box of `b` transformed by `m` (page points, row vectors).
pub(crate) fn transform_box(b: [f32; 4], m: [f32; 6]) -> [f32; 4] {
    let pts = [(b[0], b[1]), (b[2], b[1]), (b[0], b[3]), (b[2], b[3])];
    let mut r = [
        f32::INFINITY,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::NEG_INFINITY,
    ];
    for (x, y) in pts {
        let (u, v) = (m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5]);
        r = [r[0].min(u), r[1].min(v), r[2].max(u), r[3].max(v)];
    }
    r
}

/// Span of a guide across the other axis (`None`: the whole page).
type Span = Option<(f32, f32)>;
/// A snapping target: position and its guide's span.
type Target = (f32, Span);

/// Snap a moved box: its left / centre / right to the page's edges and centre or another
/// box's, the same for top / middle / bottom, within `tol` points. Returns the extra move
/// and the guides to draw (page points, x0 y0 x1 y1).
pub(crate) fn snap_move(
    moved: Option<[f32; 4]>,
    others: &[[f32; 4]],
    page: (f32, f32),
    tol: f32,
) -> (f32, f32, Vec<[f32; 4]>) {
    let Some(b) = moved else {
        return (0.0, 0.0, Vec::new());
    };
    // (target, span of the guide across the other axis)
    let mut xs = vec![(0.0, None), (page.0 / 2.0, None), (page.0, None)];
    let mut ys = vec![(0.0, None), (page.1 / 2.0, None), (page.1, None)];
    for o in others {
        for x in [o[0], f32::midpoint(o[0], o[2]), o[2]] {
            xs.push((x, Some((o[1], o[3]))));
        }
        for y in [o[1], f32::midpoint(o[1], o[3]), o[3]] {
            ys.push((y, Some((o[0], o[2]))));
        }
    }
    let best = |srcs: [f32; 3], targets: &[Target]| {
        let mut best: Option<(f32, f32, Span)> = None;
        for s in srcs {
            for &(t, span) in targets {
                let d = t - s;
                if d.abs() <= tol && best.is_none_or(|b| d.abs() < b.0.abs()) {
                    best = Some((d, t, span));
                }
            }
        }
        best
    };
    let mut guides = Vec::new();
    let bx = best([b[0], f32::midpoint(b[0], b[2]), b[2]], &xs);
    let by = best([b[1], f32::midpoint(b[1], b[3]), b[3]], &ys);
    let (dx, dy) = (bx.map_or(0.0, |v| v.0), by.map_or(0.0, |v| v.0));
    if let Some((_, x, span)) = bx {
        let (y0, y1) = span.map_or((0.0, page.1), |(a, e)| {
            ((b[1] + dy).min(a), (b[3] + dy).max(e))
        });
        guides.push([x, y0, x, y1]);
    }
    if let Some((_, y, span)) = by {
        let (x0, x1) = span.map_or((0.0, page.0), |(a, e)| {
            ((b[0] + dx).min(a), (b[2] + dx).max(e))
        });
        guides.push([x0, y, x1, y]);
    }
    (dx, dy, guides)
}

/// An arrangement of the selected objects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Arrange {
    /// Centre the selection on the page across, down or both.
    PageCenter {
        across: bool,
        down: bool,
    },
    AlignLeft,
    AlignCenter,
    AlignRight,
    AlignTop,
    AlignMiddle,
    AlignBottom,
    DistributeAcross,
    DistributeDown,
    /// Quarter turns clockwise about the selection's centre.
    Rotate(i32),
    FlipAcross,
    FlipDown,
}

impl Arrange {
    /// The command of a pane button.
    pub(crate) fn from_command(cmd: &str) -> Option<Self> {
        Some(match cmd {
            "obj.centerAcross" => Self::PageCenter {
                across: true,
                down: false,
            },
            "obj.centerDown" => Self::PageCenter {
                across: false,
                down: true,
            },
            "obj.centerBoth" => Self::PageCenter {
                across: true,
                down: true,
            },
            "obj.alignLeft" => Self::AlignLeft,
            "obj.alignCenter" => Self::AlignCenter,
            "obj.alignRight" => Self::AlignRight,
            "obj.alignTop" => Self::AlignTop,
            "obj.alignMiddle" => Self::AlignMiddle,
            "obj.alignBottom" => Self::AlignBottom,
            "obj.distAcross" => Self::DistributeAcross,
            "obj.distDown" => Self::DistributeDown,
            "obj.rotateRight" => Self::Rotate(1),
            "obj.rotateLeft" => Self::Rotate(-1),
            "obj.flipAcross" => Self::FlipAcross,
            "obj.flipDown" => Self::FlipDown,
            _ => return None,
        })
    }

    /// Selected objects it needs.
    fn needs(self) -> usize {
        match self {
            Self::AlignLeft
            | Self::AlignCenter
            | Self::AlignRight
            | Self::AlignTop
            | Self::AlignMiddle
            | Self::AlignBottom => 2,
            Self::DistributeAcross | Self::DistributeDown => 3,
            _ => 1,
        }
    }
}

fn shift(dx: f32, dy: f32) -> [f32; 6] {
    [1.0, 0.0, 0.0, 1.0, dx, dy]
}

/// Translation of each box for an alignment or distribution (`None`: one matrix for all).
fn moves_for(a: Arrange, boxes: &[[f32; 4]], page: (f32, f32)) -> Option<Vec<[f32; 6]>> {
    let u = union_box(boxes)?;
    let mid = |b: &[f32; 4]| (f32::midpoint(b[0], b[2]), f32::midpoint(b[1], b[3]));
    let all = |m: [f32; 6]| Some(vec![m; boxes.len()]);
    match a {
        Arrange::PageCenter { across, down } => {
            let (cx, cy) = mid(&u);
            let dx = if across { page.0 / 2.0 - cx } else { 0.0 };
            let dy = if down { page.1 / 2.0 - cy } else { 0.0 };
            all(shift(dx, dy))
        }
        Arrange::AlignLeft => Some(boxes.iter().map(|b| shift(u[0] - b[0], 0.0)).collect()),
        Arrange::AlignRight => Some(boxes.iter().map(|b| shift(u[2] - b[2], 0.0)).collect()),
        Arrange::AlignTop => Some(boxes.iter().map(|b| shift(0.0, u[1] - b[1])).collect()),
        Arrange::AlignBottom => Some(boxes.iter().map(|b| shift(0.0, u[3] - b[3])).collect()),
        Arrange::AlignCenter => {
            let c = mid(&u).0;
            Some(boxes.iter().map(|b| shift(c - mid(b).0, 0.0)).collect())
        }
        Arrange::AlignMiddle => {
            let c = mid(&u).1;
            Some(boxes.iter().map(|b| shift(0.0, c - mid(b).1)).collect())
        }
        Arrange::DistributeAcross | Arrange::DistributeDown => {
            let across = a == Arrange::DistributeAcross;
            let (lo, hi) = if across { (u[0], u[2]) } else { (u[1], u[3]) };
            let size = |b: &[f32; 4]| if across { b[2] - b[0] } else { b[3] - b[1] };
            let mut order: Vec<usize> = (0..boxes.len()).collect();
            let key = |b: &[f32; 4]| if across { b[0] } else { b[1] };
            order.sort_by(|&i, &j| key(&boxes[i]).total_cmp(&key(&boxes[j])));
            let total: f32 = boxes.iter().map(size).sum();
            let gap = (hi - lo - total) / (boxes.len().max(2) - 1) as f32;
            let mut out = vec![shift(0.0, 0.0); boxes.len()];
            let mut at = lo;
            for i in order {
                let b = &boxes[i];
                let d = at - key(b);
                out[i] = if across { shift(d, 0.0) } else { shift(0.0, d) };
                at += size(b) + gap;
            }
            Some(out)
        }
        Arrange::Rotate(q) => {
            let (cx, cy) = mid(&u);
            all(rotation_about(q, cx, cy))
        }
        Arrange::FlipAcross => {
            let (cx, cy) = mid(&u);
            all(flip_about(true, cx, cy))
        }
        Arrange::FlipDown => {
            let (cx, cy) = mid(&u);
            all(flip_about(false, cx, cy))
        }
    }
}

impl Editing {
    /// Apply `moves` (one matrix per selected item) as one undo step and keep the moved
    /// items selected.
    fn apply_moves(&mut self, v: &mut ReaderView, page: usize, moves: &[[f32; 6]]) -> bool {
        let list: Vec<Item> = self.items(v, page).to_vec();
        let sel: Vec<&Item> = self.selected.iter().filter_map(|&k| list.get(k)).collect();
        if sel.is_empty() || sel.len() != moves.len() {
            return false;
        }
        let mut each = Vec::new();
        let mut expect = Vec::new();
        for (it, m) in sel.iter().zip(moves) {
            if (m[0] - 1.0).abs() < 1e-6
                && (m[3] - 1.0).abs() < 1e-6
                && m[1].abs() < 1e-6
                && m[2].abs() < 1e-6
                && m[4].abs() < 1e-4
                && m[5].abs() < 1e-4
            {
                expect.push((it.bbox, it.kind));
                continue;
            }
            each.extend(it.ids.iter().map(|&i| (i, *m)));
            expect.push((transform_box(it.bbox, *m), it.kind));
        }
        if each.is_empty() {
            return false;
        }
        match v.doc_mut().transform_each(page, &each) {
            Ok(()) => {
                v.invalidate_page(page);
                self.reselect(v, page, &expect);
            }
            Err(e) => self.status = Some(e),
        }
        true
    }

    /// Move the selection by (dx, dy) page points (arrow keys).
    pub(crate) fn nudge(&mut self, v: &mut ReaderView, dx: f32, dy: f32) -> bool {
        let Some(page) = self.page else { return false };
        let n = self.selected.len();
        if n == 0 || self.typing.is_some() {
            return false;
        }
        self.apply_moves(v, page, &vec![shift(dx, dy); n])
    }

    /// Arrange the selection.
    pub(crate) fn arrange(&mut self, v: &mut ReaderView, a: Arrange) -> bool {
        let Some(page) = self.page else { return false };
        if self.selected.len() < a.needs() || self.typing.is_some() {
            return false;
        }
        let list: Vec<Item> = self.items(v, page).to_vec();
        let boxes: Vec<[f32; 4]> = self
            .selected
            .iter()
            .filter_map(|&k| list.get(k).map(|it| it.bbox))
            .collect();
        let size = v
            .doc()
            .page_info(page)
            .map_or_else(|| v.page_size(page), |p| (p.width, p.height));
        let Some(moves) = moves_for(a, &boxes, size) else {
            return false;
        };
        self.apply_moves(v, page, &moves)
    }

    /// Select every object of the page (Ctrl+A with Edit Object).
    pub(crate) fn select_all(&mut self, v: &mut ReaderView) -> bool {
        let page = self.page.unwrap_or_else(|| v.current_page());
        self.page = Some(page);
        let n = self.items(v, page).len();
        self.selected = (0..n).collect();
        n > 0
    }

    /// Selected kinds (for the pane).
    pub(crate) fn selected_count(&self) -> usize {
        self.selected.len()
    }

    /// Kinds of the selected items (from the cache).
    pub(crate) fn selected_kinds(&self) -> Vec<bo_pdfread::ObjectKind> {
        self.cached_items()
            .iter()
            .enumerate()
            .filter(|(k, _)| self.selected.contains(k))
            .map(|(_, it)| it.kind)
            .collect()
    }
}

/// A pane row of the Arrange section: sub-heading, buttons (command, icon), x step in logical
/// pixels.
pub(crate) type ArrangeRow = (&'static str, &'static [(&'static str, &'static str)], f32);

/// Pane rows of the Arrange section.
pub(crate) const ARRANGE_ROWS: [ArrangeRow; 4] = [
    (
        "pdfEditor.pageCenter",
        &[
            ("obj.centerDown", "objCenterDown"),
            ("obj.centerAcross", "objCenterAcross"),
            ("obj.centerBoth", "objCenterBoth"),
        ],
        72.0,
    ),
    (
        "pdfEditor.alignment",
        &[
            ("obj.alignLeft", "objAlignLeft"),
            ("obj.alignCenter", "objAlignCenter"),
            ("obj.alignRight", "objAlignRight"),
            ("obj.alignTop", "objAlignTop"),
            ("obj.alignMiddle", "objAlignMiddle"),
            ("obj.alignBottom", "objAlignBottom"),
        ],
        48.0,
    ),
    (
        "pdfEditor.distribute",
        &[
            ("obj.distAcross", "objDistAcross"),
            ("obj.distDown", "objDistDown"),
        ],
        48.0,
    ),
    (
        "pdfEditor.rotateFlip",
        &[
            ("obj.rotateLeft", "objRotateLeft"),
            ("obj.rotateRight", "objRotateRight"),
            ("obj.flipAcross", "objFlipAcross"),
            ("obj.flipDown", "objFlipDown"),
        ],
        48.0,
    ),
];

const TEXT: Rgba = Rgba(51, 51, 51, 255);
const MUTED: Rgba = Rgba(160, 160, 160, 255);
const SUB: Rgba = Rgba(118, 118, 118, 255);

impl EditorWindow {
    /// Shift / Ctrl held with the last mouse message (from the host).
    pub fn set_pointer_mods(&mut self, m: Mods) {
        self.edit.mods = m;
    }

    /// Arrow keys, Ctrl+A and arrangement commands for Edit Object; false when not used.
    pub(crate) fn object_key(&mut self, k: Key, mods: Mods) -> bool {
        if self.edit_tool() != Some(EditTool::Object) || self.edit.typing.is_some() {
            return false;
        }
        let step = if mods.shift { 10.0 } else { 1.0 };
        let d = match k {
            Key::Left => (-step, 0.0),
            Key::Right => (step, 0.0),
            Key::Up => (0.0, -step),
            Key::Down => (0.0, step),
            Key::Letter('a') if mods.ctrl => {
                return self.r.view_mut().is_some_and(|v| self.edit.select_all(v));
            }
            _ => return false,
        };
        if self.edit.selected.is_empty() {
            return false;
        }
        let Some(v) = self.r.view_mut() else {
            return false;
        };
        self.edit.nudge(v, d.0, d.1);
        if let Some(s) = self.edit.status.take() {
            self.r.set_status(&s);
        }
        true
    }

    /// Context menu of Edit Object at window point (x, y): delete, select all and the
    /// Arrange commands (dim when the selection is too small for them).
    pub(crate) fn object_menu(&mut self, x: f32, y: f32) {
        let n = self.edit.selected_count();
        let mut items = vec![
            crate::popup::MenuItem {
                label: self.t("pdfEditor.objDelete"),
                choice: crate::popup::Choice::Command("ed.delete".into()),
                checked: false,
                dim: n == 0,
            },
            crate::popup::MenuItem {
                label: self.t("pdfEditor.objSelectAll"),
                choice: crate::popup::Choice::Command("obj.selectAll".into()),
                checked: false,
                dim: false,
            },
        ];
        items.extend(self.image_menu8());
        for (i, (_, buttons, _)) in ARRANGE_ROWS.iter().enumerate() {
            let need = [1, 2, 3, 1][i];
            for (cmd, icon) in *buttons {
                items.push(crate::popup::MenuItem {
                    label: self.t(&format!("pdfEditor.{icon}")),
                    choice: crate::popup::Choice::Command((*cmd).into()),
                    checked: false,
                    dim: n < need,
                });
            }
        }
        self.menu_items(items, [x, y, 1.0, 1.0]);
    }

    /// `obj.*` commands of the Arrange section; false when `cmd` is not one.
    pub(crate) fn arrange_command(&mut self, cmd: &str) -> bool {
        if cmd == "obj.selectAll" {
            if let Some(v) = self.r.view_mut() {
                self.edit.select_all(v);
            }
            return true;
        }
        let Some(a) = Arrange::from_command(cmd) else {
            return false;
        };
        if let Some(v) = self.r.view_mut() {
            self.edit.arrange(v, a);
        }
        if let Some(s) = self.edit.status.take() {
            self.r.set_status(&s);
        }
        true
    }

    /// A collapsible section heading of the Format pane as the reference's: a small triangle,
    /// the title at x 27 and a rule from the title to the pane's edge.
    pub(crate) fn section_heading(&self, p: &mut dyn Painter, x0: f32, ty: f32, label: &str) {
        let k = self.ui_scale();
        let px = |v: f32| (v * k).round();
        p.path(
            &[
                bo_render::PathSeg::M(x0 + px(7.0), ty + px(4.0)),
                bo_render::PathSeg::L(x0 + px(12.0), ty + px(9.0)),
                bo_render::PathSeg::L(x0 + px(7.0), ty + px(9.0)),
                bo_render::PathSeg::Z,
            ],
            Some(MUTED),
            None,
        );
        let text = self.t(label);
        let style = TextStyle {
            size: 17.0 / 1.375 * k,
            bold: false,
        };
        let w = crate::editor_window::Estimate.measure(&text, style).0 * 0.95;
        let right = self.r.right_pane_rect();
        let x = x0 + px(27.0) + w + px(8.0);
        p.fill_rect(
            x.round(),
            ty + px(9.0),
            (right[0] + right[2] - px(14.0) - x).max(0.0).round(),
            1.0,
            Rgba(224, 224, 224, 255),
        );
        p.text(&text, x0 + px(27.0), ty, style, TEXT);
    }

    /// The Arrange section of the Format pane with Edit Object, its heading at `ty`.
    pub(crate) fn paint_arrange(&mut self, p: &mut dyn Painter, x0: f32, ty: f32) {
        let k = self.ui_scale();
        let px = |v: f32| (v * k).round();
        let style = |s125: f32| TextStyle {
            size: s125 / 1.375 * k,
            bold: false,
        };
        let n = self.edit.selected_count();
        self.section_heading(p, x0, ty, "pdfEditor.arrange");
        // Rows as the reference's (125 %: sub-heading +26, buttons +71, next sub-heading +119,
        // buttons +153, next +201, buttons +235).
        let mut y = ty + px(21.0);
        for (i, (label, buttons, step)) in ARRANGE_ROWS.iter().enumerate() {
            p.text(&self.t(label), x0 + px(27.0), y, style(16.0), SUB);
            let cy = y + px(if i == 0 { 36.0 } else { 27.0 });
            let need = match i {
                1 => 2,
                2 => 3,
                _ => 1,
            };
            let on = n >= need;
            for (j, (cmd, icon)) in buttons.iter().enumerate() {
                let cx = x0 + px(42.0 + step * j as f32);
                let s = px(28.0);
                reader_icons::draw_weight(
                    p,
                    icon,
                    cx - px(10.0),
                    cy - px(10.0),
                    px(20.0),
                    Ink {
                        ink: if on { TEXT } else { MUTED },
                        accent: if on { TEXT } else { MUTED },
                    },
                    0.9,
                );
                if on {
                    self.pane
                        .hits
                        .push(([cx - s / 2.0, cy - s / 2.0, s, s], cmd));
                }
            }
            y = cy + px(if i == 0 { 38.0 } else { 39.0 });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapping_prefers_the_nearest_edge_and_draws_a_guide() {
        // A 50 pt box 3 pt right of another box's left edge snaps to it.
        let (dx, dy, g) = snap_move(
            Some([103.0, 300.0, 153.0, 320.0]),
            &[[100.0, 100.0, 200.0, 120.0]],
            (600.0, 800.0),
            5.0,
        );
        assert!((dx + 3.0).abs() < 1e-4 && dy.abs() < 1e-4, "{dx} {dy}");
        assert_eq!(g.len(), 1);
        assert_eq!(g[0][0], 100.0);
        assert!(g[0][1] <= 100.0 && g[0][3] >= 320.0);
        // The page centre: a box centred 2 pt off snaps across and down to both centres.
        let (dx, dy, g) = snap_move(Some([252.0, 380.0, 352.0, 424.0]), &[], (600.0, 800.0), 5.0);
        assert!(
            (dx + 2.0).abs() < 1e-4 && (dy + 2.0).abs() < 1e-4,
            "{dx} {dy}"
        );
        assert_eq!(g.len(), 2);
        // Out of reach: no snap.
        let (dx, dy, g) = snap_move(Some([20.0, 20.0, 40.0, 40.0]), &[], (600.0, 800.0), 5.0);
        assert!(dx == 0.0 && dy == 0.0 && g.is_empty());
    }

    #[test]
    fn alignment_and_distribution_moves() {
        let boxes = [
            [10.0, 10.0, 30.0, 20.0],
            [50.0, 40.0, 60.0, 50.0],
            [100.0, 0.0, 140.0, 5.0],
        ];
        let m = moves_for(Arrange::AlignLeft, &boxes, (600.0, 800.0)).expect("moves");
        assert_eq!(m[1][4], -40.0);
        assert_eq!(m[2][4], -90.0);
        let m = moves_for(Arrange::AlignBottom, &boxes, (600.0, 800.0)).expect("moves");
        assert_eq!(m[0][5], 30.0);
        // Distribute across: widths 20 + 10 + 40 in 130 → gaps of 30.
        let m = moves_for(Arrange::DistributeAcross, &boxes, (600.0, 800.0)).expect("moves");
        assert_eq!(m[0][4], 0.0);
        assert_eq!(m[1][4], 60.0 - 50.0);
        assert_eq!(m[2][4], 0.0);
        let m = moves_for(
            Arrange::PageCenter {
                across: true,
                down: false,
            },
            &boxes[..1],
            (600.0, 800.0),
        )
        .expect("moves");
        assert_eq!(m[0][4], 280.0);
        assert_eq!(
            transform_box([0.0, 0.0, 10.0, 20.0], rotation_about(1, 5.0, 10.0)),
            [-5.0, 5.0, 15.0, 15.0]
        );
    }
}
