//! The drop-down of a collapsed ribbon group (BlueDoc round 16): as on the reference word
//! processor, pressing a collapsed group's button shows the whole group — its large and
//! small buttons and its label — in a panel under the button, drawn by the ribbon itself
//! (a one-group ribbon laid out off screen and painted through an offset painter).
use crate::icons::Icons;
use crate::ribbon::{Group, Ribbon, RibbonHit, Tab};
use crate::strings::Strings;
use crate::theme::Theme;
use crate::{Font, Painter, TextMeasure, TextStyle};
use bo_edit::FormatState;
use bo_render::{PathSeg, Rgba};

/// Draws into `inner` shifted by (`dx`, `dy`).
struct Offset<'a> {
    inner: &'a mut dyn Painter,
    dx: f32,
    dy: f32,
}

impl Painter for Offset<'_> {
    fn clear(&mut self, color: Rgba) {
        self.inner.clear(color);
    }
    fn fill_rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: Rgba) {
        self.inner.fill_rect(x + self.dx, y + self.dy, w, h, color);
    }
    fn page(&mut self, index: usize, x: i32, y: i32) {
        self.inner
            .page(index, x + self.dx as i32, y + self.dy as i32);
    }
    fn text(&mut self, text: &str, x: f32, y: f32, style: TextStyle, color: Rgba) {
        self.inner
            .text(text, x + self.dx, y + self.dy, style, color);
    }
    fn text_font(&mut self, text: &str, x: f32, y: f32, font: &Font<'_>, color: Rgba) {
        self.inner
            .text_font(text, x + self.dx, y + self.dy, font, color);
    }
    fn path(&mut self, segs: &[PathSeg], fill: Option<Rgba>, stroke: Option<(Rgba, f32)>) {
        let (dx, dy) = (self.dx, self.dy);
        let moved: Vec<PathSeg> = segs
            .iter()
            .map(|s| match *s {
                PathSeg::M(x, y) => PathSeg::M(x + dx, y + dy),
                PathSeg::L(x, y) => PathSeg::L(x + dx, y + dy),
                PathSeg::Q(a, b, x, y) => PathSeg::Q(a + dx, b + dy, x + dx, y + dy),
                PathSeg::C(a, b, c, d, x, y) => {
                    PathSeg::C(a + dx, b + dy, c + dx, d + dy, x + dx, y + dy)
                }
                PathSeg::Z => PathSeg::Z,
            })
            .collect();
        self.inner.path(&moved, fill, stroke);
    }
    fn push_clip(&mut self, x: f32, y: f32, w: f32, h: f32) {
        self.inner.push_clip(x + self.dx, y + self.dy, w, h);
    }
    fn pop_clip(&mut self) {
        self.inner.pop_clip();
    }
}

/// An open collapsed-group drop-down.
pub(crate) struct GroupPopup {
    /// The collapsed group's button id.
    pub(crate) id: &'static str,
    ribbon: Ribbon,
    anchor: [f32; 4],
    /// Panel rectangle (window pixels); empty until laid out.
    pub(crate) rect: [f32; 4],
    /// Window point = ribbon point + (`dx`, `dy`).
    dx: f32,
    dy: f32,
    laid_out: bool,
}

/// Frame of the panel (the reference's drop-down frame at 125 %).
const FRAME: Rgba = Rgba(0xC6, 0xC6, 0xC6, 255);

impl GroupPopup {
    /// The drop-down of collapsed group `id` showing `group`, under `anchor`.
    pub(crate) fn new(
        id: &'static str,
        label: &'static str,
        group: Group,
        anchor: [f32; 4],
    ) -> Self {
        let ribbon = Ribbon::with_tabs(vec![Tab {
            label,
            groups: vec![group],
        }])
        .with_two_line_labels()
        .with_labeled_widths(crate::doc_table::labeled_w)
        .with_large_widths(crate::doc_table::large_w)
        .with_label_drop(4.0)
        .with_trailing(crate::doc_table::trailing)
        .with_mark_on_own_line()
        .with_row_widths(crate::doc_r4_fit::row_w);
        Self {
            id,
            ribbon,
            anchor,
            rect: [0.0; 4],
            dx: 0.0,
            dy: 0.0,
            laid_out: false,
        }
    }

    /// Lay the group out (once) for a window `size` at `scale`.
    pub(crate) fn layout(
        &mut self,
        text: &dyn TextMeasure,
        strings: &Strings,
        size: (f32, f32),
        scale: f32,
    ) {
        if self.laid_out {
            return;
        }
        self.laid_out = true;
        self.ribbon.layout(text, strings, 4000.0, scale, 0.0);
        let [_, gy, right, gh] = self.ribbon.groups_box();
        let x0 = (6.0 * scale).round();
        let w = (right - x0).max(1.0);
        let pw = w + 2.0;
        let ph = gh + 2.0;
        let mut x = self.anchor[0].round();
        if x + pw > size.0 {
            x = (size.0 - pw).max(0.0);
        }
        let y = (self.anchor[1] + self.anchor[3]).round();
        self.rect = [x, y, pw, ph];
        self.dx = x + 1.0 - x0;
        self.dy = y + 1.0 - gy;
    }

    pub(crate) fn contains(&self, x: f32, y: f32) -> bool {
        let r = self.rect;
        x >= r[0] && x < r[0] + r[2] && y >= r[1] && y < r[1] + r[3]
    }

    /// Pointer moved; `true` when the drawing changed.
    pub(crate) fn hover(&mut self, x: f32, y: f32) -> bool {
        if self.contains(x, y) {
            self.ribbon.hover(x - self.dx, y - self.dy)
        } else {
            self.ribbon.clear_pointer()
        }
    }

    pub(crate) fn press(&mut self, x: f32, y: f32) -> bool {
        self.ribbon.press(x - self.dx, y - self.dy)
    }

    /// The command released on, if any.
    pub(crate) fn release(&mut self, x: f32, y: f32) -> Option<&'static str> {
        match self.ribbon.release(x - self.dx, y - self.dy).0 {
            Some(RibbonHit::Command(id)) => Some(id),
            _ => None,
        }
    }

    /// Window rectangle of command `id` in the panel (menus open under it).
    pub(crate) fn rect_of(&self, id: &'static str) -> Option<[f32; 4]> {
        self.ribbon
            .rect_of(RibbonHit::Command(id))
            .map(|r| [r[0] + self.dx, r[1] + self.dy, r[2], r[3]])
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn paint(
        &self,
        p: &mut dyn Painter,
        theme: &Theme,
        icons: &Icons,
        strings: &Strings,
        format: &FormatState,
        on_ids: &[&str],
    ) {
        if !self.laid_out {
            return;
        }
        let [x, y, w, h] = self.rect;
        p.fill_rect(x, y, w, h, FRAME);
        let white = Rgba(255, 255, 255, 255);
        p.fill_rect(x + 1.0, y + 1.0, w - 2.0, h - 2.0, white);
        let mut t = theme.clone();
        t.ribbon = white;
        t.surface = white;
        p.push_clip(x + 1.0, y + 1.0, w - 2.0, h - 2.0);
        let mut o = Offset {
            inner: p,
            dx: self.dx,
            dy: self.dy,
        };
        self.ribbon
            .paint(&mut o, &t, icons, strings, format, &[], on_ids);
        p.pop_clip();
    }
}
