//! Horizontal ruler over the current page: margins, centimetre ticks numbered from the left
//! margin, and draggable paragraph indent markers (first line, hanging, left, right).
use crate::theme::Theme;
use crate::{Painter, TextStyle};
use bo_edit::{FormatState, SectionState};
use bo_render::{PathSeg, Rgba};

/// Margin areas on the ruler (grey measured on a desktop word processor).
const RULER_MARGIN: Rgba = Rgba(0xc6, 0xc6, 0xc6, 255);

/// Indent markers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Marker {
    FirstLine,
    Hanging,
    Left,
    Right,
}

/// Indents snap to this step while dragging (twips; 0.25 cm).
const SNAP: i32 = 142;

/// The ruler.
#[derive(Default)]
pub struct Ruler {
    scale: f32,
    y: f32,
    width: f32,
    /// Left edge of the page and device pixels per twip.
    page_x: f32,
    page_w: f32,
    per_twip: f32,
    sec: SectionState,
    /// Indents shown (twips): left, right, first line.
    indents: (i32, i32, i32),
    drag: Option<(Marker, f32)>,
}

impl Ruler {
    /// Height in device pixels.
    pub fn height(scale: f32) -> f32 {
        (24.0 * scale).round()
    }

    /// Place the ruler at window `y` over the page at `page_x` (`page_w` wide, device pixels),
    /// `px_per_pt` device pixels per point, for the paragraph formatting `f`.
    pub fn layout(
        &mut self,
        y: f32,
        width: f32,
        scale: f32,
        page: (f32, f32),
        px_per_pt: f32,
        f: &FormatState,
    ) {
        self.y = y;
        self.width = width;
        self.scale = scale;
        (self.page_x, self.page_w) = page;
        self.per_twip = px_per_pt / 20.0;
        self.sec = f.section;
        if self.drag.is_none() {
            self.indents = (f.indent_left, f.indent_right, f.first_line);
        }
    }

    fn text_left(&self) -> f32 {
        self.page_x + self.sec.margin_left as f32 * self.per_twip
    }

    fn text_right(&self) -> f32 {
        self.page_x + (self.sec.width - self.sec.margin_right) as f32 * self.per_twip
    }

    fn marker_x(&self, m: Marker) -> f32 {
        let (l, r, first) = self.indents;
        match m {
            Marker::FirstLine => self.text_left() + (l + first) as f32 * self.per_twip,
            Marker::Hanging | Marker::Left => self.text_left() + l as f32 * self.per_twip,
            Marker::Right => self.text_right() - r as f32 * self.per_twip,
        }
    }

    fn h(&self) -> f32 {
        Self::height(self.scale)
    }

    fn marker_at(&self, x: f32, y: f32) -> Option<Marker> {
        if y < self.y || y >= self.y + self.h() {
            return None;
        }
        let k = (6.0 * self.scale).round();
        let mid = self.y + self.h() / 2.0;
        let near = |m: Marker| (x - self.marker_x(m)).abs() <= k;
        if near(Marker::Right) {
            Some(Marker::Right)
        } else if y < mid && near(Marker::FirstLine) {
            Some(Marker::FirstLine)
        } else if y >= self.y + self.h() - k && near(Marker::Left) {
            Some(Marker::Left)
        } else if near(Marker::Hanging) {
            Some(Marker::Hanging)
        } else {
            None
        }
    }

    /// Whether a window y is on the ruler.
    pub fn contains_y(&self, y: f32) -> bool {
        y >= self.y && y < self.y + self.h()
    }

    /// Button pressed: starts dragging a marker under the pointer.
    pub fn press(&mut self, x: f32, y: f32) -> bool {
        self.drag = self.marker_at(x, y).map(|m| (m, x));
        self.drag.is_some()
    }

    /// Pointer dragged: moves the marker (snapped); `true` when it moved.
    pub fn drag(&mut self, x: f32) -> bool {
        let Some((m, _)) = self.drag else {
            return false;
        };
        let snap = |tw: f32| ((tw / SNAP as f32).round() as i32) * SNAP;
        let from_left = snap((x - self.text_left()) / self.per_twip);
        let from_right = snap((self.text_right() - x) / self.per_twip);
        let (mut l, mut r, mut first) = self.indents;
        let text_w = self.sec.width - self.sec.margin_left - self.sec.margin_right;
        match m {
            // The first line's absolute position moves; the left indent stays.
            Marker::FirstLine => first = from_left - l,
            // The left indent moves, the first line stays where it is.
            Marker::Hanging => {
                let abs_first = l + first;
                l = from_left;
                first = abs_first - l;
            }
            Marker::Left => l = from_left,
            Marker::Right => r = from_right,
        }
        // Keep at least 1 cm of text width.
        if l + r > text_w - 567 {
            return false;
        }
        let changed = (l, r, first) != self.indents;
        self.indents = (l, r, first);
        changed
    }

    /// Button released: the indents to apply (left, right, first line), if a marker moved.
    pub fn release(&mut self, f: &FormatState) -> Option<(i32, i32, i32)> {
        self.drag.take()?;
        (self.indents != (f.indent_left, f.indent_right, f.first_line)).then_some(self.indents)
    }

    /// Paint.
    pub fn paint(&self, p: &mut dyn Painter, t: &Theme) {
        let s = self.scale;
        let (y, h) = (self.y, self.h());
        // Neutral greys measured on a desktop word processor's ruler: the strip matches
        // the canvas around the pages, the margins are a darker grey.
        p.fill_rect(0.0, y, self.width, h, crate::doc_view::CANVAS_BG);
        let band = (y + (4.0 * s).round(), h - (8.0 * s).round());
        p.fill_rect(self.page_x, band.0, self.page_w, band.1, RULER_MARGIN);
        let (l, r) = (self.text_left(), self.text_right());
        p.fill_rect(l, band.0, r - l, band.1, t.surface);
        // Ticks every 0.25 cm, numbered every centimetre from the left margin.
        let per_cm = 567.0 * self.per_twip;
        if per_cm > 4.0 {
            let style = TextStyle {
                size: (10.0 * s).round(),
                bold: false,
            };
            let line_w = s.max(1.0).round();
            let first = ((self.page_x - l) / per_cm * 4.0).floor() as i32;
            let last = ((self.page_x + self.page_w - l) / per_cm * 4.0).ceil() as i32;
            for q in first..=last {
                let x = (l + q as f32 * per_cm / 4.0).round();
                if x < self.page_x || x > self.page_x + self.page_w {
                    continue;
                }
                let mid = band.0 + band.1 / 2.0;
                if q % 4 == 0 {
                    if q != 0 {
                        let n = (q / 4).abs().to_string();
                        p.text(
                            &n,
                            x - style.size * 0.3,
                            mid - style.size * 0.66,
                            style,
                            t.text_muted,
                        );
                    }
                } else if q % 2 == 0 {
                    // Half centimetre: a short line.
                    let len = (5.0 * s).round();
                    p.fill_rect(x, (mid - len / 2.0).round(), line_w, len, t.text_muted);
                } else {
                    // Quarter centimetre: a dot.
                    p.fill_rect(x, mid.round(), line_w, line_w, t.text_muted);
                }
            }
        }
        // Indent markers: first line (down, top), hanging (up, bottom) on the left box,
        // right indent (up, bottom).
        let k = (5.0 * s).round();
        let line = s.max(1.0).round();
        let top = band.0;
        let bottom = band.0 + band.1;
        let tri = |p: &mut dyn Painter, x: f32, down: bool, base: f32| {
            let tip = if down { base + k } else { base - k };
            let segs = [
                PathSeg::M(x - k, base),
                PathSeg::L(x + k, base),
                PathSeg::L(x, tip),
                PathSeg::Z,
            ];
            p.path(&segs, Some(t.surface), Some((t.text_muted, line)));
        };
        tri(p, self.marker_x(Marker::FirstLine), true, top - line);
        let lx = self.marker_x(Marker::Hanging);
        tri(p, lx, false, bottom - k / 2.0);
        p.fill_rect(lx - k, bottom - k / 2.0, 2.0 * k, k, t.text_muted);
        p.fill_rect(
            lx - k + line,
            bottom - k / 2.0 + line,
            2.0 * k - 2.0 * line,
            k - 2.0 * line,
            t.surface,
        );
        tri(p, self.marker_x(Marker::Right), false, bottom);
    }
}

/// Paint the vertical ruler in the strip at `x` (`w` wide) between `top` and `bottom`:
/// the page from `page_y` (`page_h` pixels high) with its top and bottom margins in grey,
/// centimetres numbered from the top margin, half centimetres as short lines and quarters
/// as dots, as on a desktop word processor's vertical ruler.
#[allow(clippy::too_many_arguments)]
pub fn paint_vertical(
    p: &mut dyn Painter,
    t: &Theme,
    (x, w, top, bottom): (f32, f32, f32, f32),
    (page_y, page_h): (f32, f32),
    sec: &SectionState,
    per_twip: f32,
    scale: f32,
) {
    p.fill_rect(x, top, w, bottom - top, crate::doc_view::CANVAS_BG);
    let band = (x + (3.0 * scale).round(), w - (6.0 * scale).round());
    p.push_clip(x, top, w, bottom - top);
    p.fill_rect(band.0, page_y, band.1, page_h, RULER_MARGIN);
    let text_top = page_y + sec.margin_top as f32 * per_twip;
    let text_bottom = page_y + (sec.height - sec.margin_bottom) as f32 * per_twip;
    p.fill_rect(
        band.0,
        text_top,
        band.1,
        (text_bottom - text_top).max(0.0),
        t.surface,
    );
    let per_cm = 567.0 * per_twip;
    if per_cm > 4.0 {
        let style = TextStyle {
            size: (10.0 * scale).round(),
            bold: false,
        };
        let line_w = scale.max(1.0).round();
        let mid = band.0 + band.1 / 2.0;
        let first = ((page_y - text_top) / per_cm * 4.0).floor() as i32;
        let last = ((page_y + page_h - text_top) / per_cm * 4.0).ceil() as i32;
        for q in first..=last {
            let y = (text_top + q as f32 * per_cm / 4.0).round();
            if y < page_y.max(top) || y > (page_y + page_h).min(bottom) {
                continue;
            }
            if q % 4 == 0 {
                if q != 0 {
                    let n = (q / 4).abs().to_string();
                    let tw = style.size * 0.55 * n.len() as f32;
                    p.text(
                        &n,
                        mid - tw / 2.0,
                        y - style.size * 0.66,
                        style,
                        t.text_muted,
                    );
                }
            } else if q % 2 == 0 {
                let len = (5.0 * scale).round();
                p.fill_rect((mid - len / 2.0).round(), y, len, line_w, t.text_muted);
            } else {
                p.fill_rect(mid.round(), y, line_w, line_w, t.text_muted);
            }
        }
    }
    p.pop_clip();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn format() -> FormatState {
        FormatState {
            section: SectionState {
                width: 11906,
                height: 16838,
                margin_left: 1440,
                margin_right: 1440,
                ..SectionState::default()
            },
            ..FormatState::default()
        }
    }

    #[test]
    fn dragging_markers_changes_indents() {
        let f = format();
        let mut r = Ruler::default();
        // 1 px per point: 0.05 px per twip.
        r.layout(100.0, 1200.0, 1.0, (100.0, 595.0), 1.0, &f);
        let left = r.text_left();
        // First-line marker at the left margin, top half.
        assert!(r.press(left, 105.0));
        assert!(r.drag(left + 28.35)); // 1 cm
        assert_eq!(r.release(&f), Some((0, 0, 568)));
        // Left box at the bottom moves the left indent.
        assert!(r.press(left, 122.0));
        r.drag(left + 14.2);
        let (l, _, first) = r.release(&f).unwrap();
        assert_eq!((l, first), (284, 568), "the left box moves both");
        // Right marker.
        let right = r.text_right();
        assert!(r.press(right, 120.0));
        r.drag(right - 28.35);
        assert_eq!(r.release(&f).map(|v| v.1), Some(568));
    }
}
