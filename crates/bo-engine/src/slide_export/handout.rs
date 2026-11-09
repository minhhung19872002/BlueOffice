//! Handout page geometry (our own layout; ECMA-376 only names the handout
//! layouts, §19.7.8 `ST_PrintWhat` / `handouts1` … `handouts9`).
//!
//! The page keeps a 36 pt side margin and a 54 pt top / bottom margin (room for
//! the header and footer). The slides sit in a grid of equal cells 18 pt apart;
//! each slide keeps its aspect ratio and is centred in its cell. With three
//! slides per page the slides take the left half and the right half holds
//! ruled lines for notes, 24 pt apart, beside each slide.

use super::masters::HfKind;
use serde::Deserialize;

/// Side margin of handout pages (points).
pub const MARGIN_X: f32 = 36.0;
/// Top and bottom margin of handout pages (points).
pub const MARGIN_Y: f32 = 54.0;
/// Space between cells (points).
pub const GAP: f32 = 18.0;
/// Distance between the note lines of a three-slide handout (points).
pub const LINE_PITCH: f32 = 24.0;

/// A rectangle in points.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Frame {
    /// Left.
    pub x: f32,
    /// Top.
    pub y: f32,
    /// Width.
    pub w: f32,
    /// Height.
    pub h: f32,
}

/// Horizontal note line from `x1` to `x2` at `y`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NoteLine {
    /// Start x.
    pub x1: f32,
    /// End x.
    pub x2: f32,
    /// Height.
    pub y: f32,
}

/// Reading order of the slides on a handout page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Order {
    /// Across each row, then down.
    #[default]
    Horizontal,
    /// Down each column, then across.
    Vertical,
}

/// Slide frames of a page (row-major cell order) and the note lines of each cell.
#[derive(Debug, Clone, PartialEq)]
pub struct Layout {
    /// Columns of the grid.
    pub cols: usize,
    /// Rows of the grid.
    pub rows: usize,
    /// Frame of the slide in each cell, row by row.
    pub frames: Vec<Frame>,
    /// Note lines beside each cell (only with three slides per page).
    pub lines: Vec<Vec<NoteLine>>,
}

/// Grid (columns, rows) of a handout with `per_page` slides.
pub fn grid(per_page: u8) -> Option<(usize, usize)> {
    Some(match per_page {
        1 => (1, 1),
        2 => (1, 2),
        3 => (1, 3),
        4 => (2, 2),
        6 => (2, 3),
        9 => (3, 3),
        _ => return None,
    })
}

/// Largest frame of aspect ratio `aspect` (width / height) centred in `cell`.
pub fn fit(cell: &Frame, aspect: f32) -> Frame {
    let aspect = if aspect > 0.0 { aspect } else { 4.0 / 3.0 };
    let (w, h) = if cell.w / cell.h.max(f32::MIN_POSITIVE) > aspect {
        (cell.h * aspect, cell.h)
    } else {
        (cell.w, cell.w / aspect)
    };
    Frame {
        x: cell.x + (cell.w - w) / 2.0,
        y: cell.y + (cell.h - h) / 2.0,
        w,
        h,
    }
}

/// Layout of a `pw` × `ph` page with `per_page` slides of aspect `aspect`.
pub fn layout(per_page: u8, pw: f32, ph: f32, aspect: f32) -> Option<Layout> {
    let (cols, rows) = grid(per_page)?;
    let cw = pw - 2.0 * MARGIN_X;
    let ch = ph - 2.0 * MARGIN_Y;
    if cw <= 0.0 || ch <= 0.0 {
        return None;
    }
    // Three per page: the slide column takes half the width, lines the rest.
    let slide_w = if per_page == 3 {
        (cw - GAP) / 2.0
    } else {
        (cw - (cols as f32 - 1.0) * GAP) / cols as f32
    };
    let cell_h = (ch - (rows as f32 - 1.0) * GAP) / rows as f32;
    let mut frames = Vec::with_capacity(cols * rows);
    let mut lines = Vec::with_capacity(cols * rows);
    for r in 0..rows {
        for c in 0..cols {
            let cell = Frame {
                x: MARGIN_X + c as f32 * (slide_w + GAP),
                y: MARGIN_Y + r as f32 * (cell_h + GAP),
                w: slide_w,
                h: cell_h,
            };
            let f = fit(&cell, aspect);
            let mut ls = Vec::new();
            if per_page == 3 {
                let x1 = MARGIN_X + slide_w + GAP;
                let mut y = f.y + LINE_PITCH;
                while y <= f.y + f.h + 0.01 {
                    ls.push(NoteLine {
                        x1,
                        x2: pw - MARGIN_X,
                        y,
                    });
                    y += LINE_PITCH;
                }
            }
            frames.push(f);
            lines.push(ls);
        }
    }
    Some(Layout {
        cols,
        rows,
        frames,
        lines,
    })
}

/// Cell (row-major index) of the `i`-th slide of a page.
pub fn slot(i: usize, per_page: u8, order: Order) -> usize {
    let Some((cols, rows)) = grid(per_page) else {
        return i;
    };
    match order {
        Order::Horizontal => i,
        Order::Vertical => {
            let (col, row) = (i / rows, i % rows);
            row * cols + col
        }
    }
}

/// Header / footer box when the master has none: the top or bottom margin band.
pub fn default_hf_box(kind: HfKind, pw: f32, ph: f32) -> Frame {
    let band = MARGIN_Y - 18.0;
    let y = if kind.at_bottom() {
        ph - MARGIN_Y + 9.0
    } else {
        9.0
    };
    Frame {
        x: MARGIN_X,
        y,
        w: pw - 2.0 * MARGIN_X,
        h: band,
    }
}

/// Slide image box of a notes page without notes master: the upper part of the page.
pub fn default_notes_image(pw: f32, ph: f32) -> Frame {
    Frame {
        x: 72.0,
        y: 72.0,
        w: pw - 144.0,
        h: ph * 0.4 - 72.0,
    }
}

/// Notes body box without notes master: below the slide image down to the bottom margin.
pub fn default_notes_body(pw: f32, ph: f32, image: &Frame) -> Frame {
    let y = image.y + image.h + 18.0;
    Frame {
        x: 72.0,
        y,
        w: pw - 144.0,
        h: (ph - 72.0 - y).max(0.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const A4: (f32, f32) = (595.275_6, 841.889_8);
    const LETTER: (f32, f32) = (612.0, 792.0);

    fn close(a: f32, b: f32) {
        assert!((a - b).abs() < 0.01, "{a} != {b}");
    }

    fn frame(f: &Frame, x: f32, y: f32, w: f32, h: f32) {
        close(f.x, x);
        close(f.y, y);
        close(f.w, w);
        close(f.h, h);
    }

    #[test]
    fn six_per_page_a4_wide_slides() {
        // Content 523.2756 × 733.8898; cells (523.2756 − 18) / 2 = 252.6378 wide,
        // (733.8898 − 36) / 3 = 232.6299 high. A 16:9 slide fills the width:
        // height 252.6378 × 9 / 16 = 142.1088, centred: top offset (232.6299 − 142.1088) / 2 = 45.2606.
        let l = layout(6, A4.0, A4.1, 16.0 / 9.0).unwrap();
        assert_eq!((l.cols, l.rows, l.frames.len()), (2, 3, 6));
        frame(&l.frames[0], 36.0, 99.2606, 252.6378, 142.1088);
        frame(&l.frames[1], 306.6378, 99.2606, 252.6378, 142.1088);
        // Row 3 starts at 54 + 2 × (232.6299 + 18) = 555.2598.
        frame(&l.frames[4], 36.0, 600.5204, 252.6378, 142.1088);
        assert!(l.lines.iter().all(Vec::is_empty));
    }

    #[test]
    fn one_and_two_per_page_letter() {
        // One 4:3 slide on Letter: content 540 × 684, width-limited: 540 × 405,
        // top = 54 + (684 − 405) / 2 = 193.5.
        let l = layout(1, LETTER.0, LETTER.1, 4.0 / 3.0).unwrap();
        frame(&l.frames[0], 36.0, 193.5, 540.0, 405.0);
        // Two: cells 540 × (684 − 18) / 2 = 333; 4:3 → 444 × 333, x = 36 + (540 − 444) / 2 = 84.
        let l = layout(2, LETTER.0, LETTER.1, 4.0 / 3.0).unwrap();
        frame(&l.frames[0], 84.0, 54.0, 444.0, 333.0);
        frame(&l.frames[1], 84.0, 405.0, 444.0, 333.0);
    }

    #[test]
    fn four_and_nine_per_page_a4() {
        // Four: cells 252.6378 × (733.8898 − 18) / 2 = 357.9449; 4:3 → 252.6378 × 189.4784,
        // top = 54 + (357.9449 − 189.4784) / 2 = 138.2333.
        let l = layout(4, A4.0, A4.1, 4.0 / 3.0).unwrap();
        frame(&l.frames[0], 36.0, 138.2333, 252.6378, 189.4784);
        frame(&l.frames[3], 306.6378, 514.1782, 252.6378, 189.4784);
        // Nine: cells (523.2756 − 36) / 3 = 162.4252 × (733.8898 − 36) / 3 = 232.6299;
        // 16:9 → 162.4252 × 91.3642, top = 54 + (232.6299 − 91.3642) / 2 = 124.6329.
        let l = layout(9, A4.0, A4.1, 16.0 / 9.0).unwrap();
        frame(&l.frames[0], 36.0, 124.6329, 162.4252, 91.3642);
        frame(&l.frames[8], 396.8504, 625.8927, 162.4252, 91.3642);
    }

    #[test]
    fn three_per_page_has_note_lines() {
        // Letter: slide column (540 − 18) / 2 = 261 wide, cells (684 − 36) / 3 = 216 high.
        // 4:3 → 261 × 195.75, top = 54 + (216 − 195.75) / 2 = 64.125.
        let l = layout(3, LETTER.0, LETTER.1, 4.0 / 3.0).unwrap();
        assert_eq!((l.cols, l.rows), (1, 3));
        frame(&l.frames[0], 36.0, 64.125, 261.0, 195.75);
        frame(&l.frames[1], 36.0, 298.125, 261.0, 195.75);
        // Lines from x = 36 + 261 + 18 = 315 to 612 − 36 = 576, every 24 pt below the
        // frame top while inside it: floor(195.75 / 24) = 8 lines.
        let lines = &l.lines[0];
        assert_eq!(lines.len(), 8);
        close(lines[0].x1, 315.0);
        close(lines[0].x2, 576.0);
        close(lines[0].y, 88.125);
        close(lines[7].y, 64.125 + 8.0 * 24.0);
        assert_eq!(l.lines[2].len(), 8);
    }

    #[test]
    fn reading_order() {
        // Six per page (2 × 3): vertical order fills the left column first.
        let v: Vec<usize> = (0..6).map(|i| slot(i, 6, Order::Vertical)).collect();
        assert_eq!(v, [0, 2, 4, 1, 3, 5]);
        let h: Vec<usize> = (0..6).map(|i| slot(i, 6, Order::Horizontal)).collect();
        assert_eq!(h, [0, 1, 2, 3, 4, 5]);
        // Nine (3 × 3): slide 4 (index 3) of a vertical page is row 0, column 1.
        assert_eq!(slot(3, 9, Order::Vertical), 1);
        assert!(layout(5, A4.0, A4.1, 1.0).is_none());
    }

    #[test]
    fn fit_keeps_the_aspect_ratio() {
        let cell = Frame {
            x: 10.0,
            y: 20.0,
            w: 100.0,
            h: 100.0,
        };
        frame(&fit(&cell, 2.0), 10.0, 45.0, 100.0, 50.0);
        frame(&fit(&cell, 0.5), 35.0, 20.0, 50.0, 100.0);
    }
}
