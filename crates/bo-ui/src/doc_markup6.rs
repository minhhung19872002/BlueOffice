//! Review markup drawn by the native view (parity round 6): the markup area right of each
//! page with its balloons (comments with the author's initials in a circle, name and text;
//! formatting and deletion balloons with "Formatted: …" text), dashed connectors from the
//! anchor in the text, and the change bars in the left margin — all laid out by the engine
//! (`bo_layout::PageMarkup`, ECMA-376 Part 1, §17.13.5 revisions, §17.13.4 comments); Simple
//! Markup gives red bars and comment icons. Sizes follow the reference at 100 % zoom:
//! balloon text about 9 pt, the author bold, a 2 px bar of the author's colour on the
//! balloon's left edge.
use super::DocView;
use crate::{Painter, TextStyle};
use bo_render::Rgba;

fn parse_hex(s: &str) -> Rgba {
    crate::theme::parse_color(s).unwrap_or(Rgba(209, 52, 56, 255))
}

/// Initials of an author ("An Nguyen" → "AN").
fn initials(author: &str) -> String {
    author
        .split_whitespace()
        .filter_map(|w| w.chars().next())
        .flat_map(char::to_uppercase)
        .take(3)
        .collect()
}

impl DocView {
    /// Width of the markup area right of the pages (device pixels): the widest page's.
    pub(crate) fn markup_px(&self) -> f32 {
        (0..self.pages.len())
            .filter_map(|i| self.engine.page_markup(i))
            .map(|m| m.width)
            .fold(0.0_f32, f32::max)
            * self.scale()
    }

    /// Recompute the comment anchors' rectangles (markup shown and comments marked).
    pub(crate) fn refresh_comment_marks(&mut self) {
        use bo_engine::markup::MarkupMode;
        self.comment_marks.clear();
        if matches!(self.markup.mode, MarkupMode::None | MarkupMode::Original)
            || !self.markup.comments
        {
            return;
        }
        for c in self
            .comments
            .iter()
            .filter(|c| c.parent.is_none() && !c.done)
        {
            let (Some(a), Some(b)) = (c.start, c.end) else {
                continue;
            };
            let k = bo_layout::author_color(&c.author);
            let colour = Rgba(k.r, k.g, k.b, 255);
            for r in self.engine.range_rects(a, b) {
                self.comment_marks.push((r, colour));
            }
        }
    }

    /// Comment anchors on a page: a light tint of the author's colour framed in it
    /// (#F8DCDD inside a #D13438 line for the first author on the reference).
    pub(crate) fn paint_comment_marks(&self, p: &mut dyn Painter, slot: (usize, i32, i32)) {
        let s = self.scale();
        let (index, sx, sy) = slot;
        for (r, c) in self.comment_marks.iter().filter(|(r, _)| r.page == index) {
            let (x, y) = ((sx as f32 + r.x * s).round(), (sy as f32 + r.y * s).round());
            let (w, h) = ((r.w * s).round().max(1.0), (r.h * s).round().max(1.0));
            p.fill_rect(x, y, w, h, Rgba(c.0, c.1, c.2, 42));
            p.fill_rect(x, y, w, 1.0, *c);
            p.fill_rect(x, y + h - 1.0, w, 1.0, *c);
            p.fill_rect(x, y, 1.0, h, *c);
            p.fill_rect(x + w - 1.0, y, 1.0, h, *c);
        }
    }

    /// Markup of page slot `(index, x, y, width, height)`: area, bars, balloons.
    #[allow(clippy::too_many_lines)]
    pub(crate) fn paint_markup(&self, p: &mut dyn Painter, slot: (usize, i32, i32, i32, i32)) {
        let (index, sx, sy, sw, sh) = slot;
        let Some(m) = self.engine.page_markup(index) else {
            return;
        };
        let s = self.scale();
        let (x0, y0) = (sx as f32, sy as f32);
        let area = m.width * s;
        if area > 0.0 {
            // The markup area continues the paper to the right.
            p.fill_rect(
                x0 + sw as f32,
                y0,
                area,
                sh as f32,
                Rgba(255, 255, 255, 255),
            );
            // A thin grey rule at the page's edge (#C6C6C6 on the reference).
            p.fill_rect(x0 + sw as f32, y0, 1.0, sh as f32, Rgba(198, 198, 198, 255));
        }
        let bar_w = (1.25 * self.display_scale).round().max(1.0);
        for b in &m.bars {
            p.fill_rect(
                (x0 + b.x * s).round(),
                (y0 + b.top * s).round(),
                bar_w,
                ((b.bottom - b.top) * s).round().max(1.0),
                parse_hex(&b.color),
            );
        }
        let name = TextStyle {
            size: (8.5 * s).round().max(8.0),
            bold: true,
        };
        let body = TextStyle {
            size: (8.5 * s).round().max(8.0),
            bold: false,
        };
        let line = body.size * 1.3;
        for bl in &m.balloons {
            let c = parse_hex(&bl.color);
            let bx = x0 + bl.x * s;
            let by = y0 + bl.y * s;
            let bh = (bl.h * s).max(line * 2.0);
            if bl.icon {
                // Simple Markup: a speech-bubble icon for the comment.
                let k = (12.0 * s).round();
                p.fill_rect(bx, by, k, k * 0.75, c);
                p.fill_rect(
                    bx + 1.0,
                    by + 1.0,
                    k - 2.0,
                    k * 0.75 - 2.0,
                    Rgba(255, 255, 255, 255),
                );
                p.fill_rect(bx + k * 0.2, by + k * 0.75 - 1.0, k * 0.2, k * 0.2, c);
                continue;
            }
            // Dashed connector at the anchor's height to the balloon's edge.
            let ax = x0 + bl.anchor_x * s;
            let ay = (y0 + bl.anchor_y * s).round();
            let dash = (3.0 * self.display_scale).round().max(2.0);
            let mut x = ax;
            while x < bx {
                p.fill_rect(
                    x.round(),
                    ay,
                    dash.min(bx - x),
                    1.0,
                    Rgba(160, 160, 160, 255),
                );
                x += dash * 2.0;
            }
            // Bar of the author's colour down the balloon's left edge.
            p.fill_rect(bx.round(), by.round(), bar_w + 1.0, bh.round(), c);
            let comment = matches!(bl.kind, bo_layout::MarkKind::Comment);
            let mut tx = bx + 10.0 * s;
            if comment {
                // The author's initials in a filled circle.
                let r = 7.5 * s;
                let (cx, cy) = (bx + 6.0 * s + r, by + 2.0 * s + r);
                let k = r * 0.552_284_8;
                p.path(
                    &[
                        bo_render::PathSeg::M(cx + r, cy),
                        bo_render::PathSeg::C(cx + r, cy + k, cx + k, cy + r, cx, cy + r),
                        bo_render::PathSeg::C(cx - k, cy + r, cx - r, cy + k, cx - r, cy),
                        bo_render::PathSeg::C(cx - r, cy - k, cx - k, cy - r, cx, cy - r),
                        bo_render::PathSeg::C(cx + k, cy - r, cx + r, cy - k, cx + r, cy),
                        bo_render::PathSeg::Z,
                    ],
                    Some(c),
                    None,
                );
                let ini = initials(&bl.author);
                let small = TextStyle {
                    size: (6.5 * s).round().max(7.0),
                    bold: false,
                };
                p.text(
                    &ini,
                    cx - small.size * 0.33 * ini.chars().count() as f32,
                    cy - small.size * 0.66,
                    small,
                    Rgba(255, 255, 255, 255),
                );
                tx = cx + r + 7.0 * s;
            }
            let right = bx + bl.w * s;
            p.push_clip(tx, by, (right - tx).max(1.0), bh + line);
            p.text(&bl.author, tx, by + 1.0 * s, name, Rgba(38, 38, 38, 255));
            let mut yy = by + 1.0 * s + line;
            for l in bl.text.lines().take(6) {
                p.text(l, tx, yy, body, Rgba(38, 38, 38, 255));
                yy += line;
            }
            p.pop_clip();
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn initials_of_authors() {
        assert_eq!(super::initials("An Nguyen"), "AN");
        assert_eq!(super::initials("binh"), "B");
    }
}
