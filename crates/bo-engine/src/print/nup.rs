//! Pages per sheet ("N-up") and scaling to a paper size.
//!
//! Geometry (our own choice):
//! * one page per sheet without a paper: the page prints at its own size;
//! * one page per sheet on a paper: scaled up or down to fit the paper,
//!   centred, no extra margin (the page already has its margins);
//! * several pages per sheet: an outer margin of [`MARGIN`] and a gap of
//!   [`GAP`] between the cells of a `cols × rows` grid; each page is scaled
//!   (aspect kept) to fit its cell and centred in it;
//! * the grid (e.g. 3 × 2 or 2 × 3 for six pages) and the paper orientation
//!   (as given or turned) are the pair giving the largest scale; on a tie the
//!   paper as given and the grid with more columns win.
//!
//! Cells fill across each row then down (`Across`) or down each column then
//! across (`Down`).

use bo_render::{Command, PageList, PathSeg, Rgba, Stroke};
use serde::Deserialize;

/// Outer margin of a sheet with several pages (points; 0.25 in).
pub const MARGIN: f32 = 18.0;
/// Gap between the pages on a sheet (points; 0.125 in).
pub const GAP: f32 = 9.0;

/// Order of the pages on a sheet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum NupOrder {
    /// Across each row, then down.
    #[default]
    Across,
    /// Down each column, then across.
    Down,
}

/// Pages per sheet the panel offers.
pub const PER_SHEET: [u8; 6] = [1, 2, 4, 6, 8, 16];

/// Grids `(cols, rows)` holding `n` pages, more columns first.
fn grids(n: u8) -> &'static [(u32, u32)] {
    match n {
        2 => &[(2, 1), (1, 2)],
        4 => &[(2, 2)],
        6 => &[(3, 2), (2, 3)],
        8 => &[(4, 2), (2, 4)],
        16 => &[(4, 4)],
        _ => &[(1, 1)],
    }
}

/// Layout of one sheet.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SheetLayout {
    /// Sheet width and height (points).
    pub width: f32,
    /// Sheet height (points).
    pub height: f32,
    /// Columns of the grid.
    pub cols: u32,
    /// Rows of the grid.
    pub rows: u32,
    /// Margin around the grid (points).
    pub margin: f32,
    /// Gap between cells (points).
    pub gap: f32,
    /// Scale of the page the layout was chosen for.
    pub scale: f32,
}

impl SheetLayout {
    /// Cell width and height.
    pub fn cell(&self) -> (f32, f32) {
        let c = self.cols as f32;
        let r = self.rows as f32;
        (
            (self.width - 2.0 * self.margin - (c - 1.0) * self.gap) / c,
            (self.height - 2.0 * self.margin - (r - 1.0) * self.gap) / r,
        )
    }

    /// Top-left corner of cell `slot` (0-based) in `order`.
    pub fn cell_origin(&self, slot: usize, order: NupOrder) -> (f32, f32) {
        let (cols, rows) = (self.cols as usize, self.rows as usize);
        let (col, row) = match order {
            NupOrder::Across => (slot % cols, slot / cols),
            NupOrder::Down => (slot / rows, slot % rows),
        };
        let (cw, ch) = self.cell();
        (
            self.margin + col as f32 * (cw + self.gap),
            self.margin + row as f32 * (ch + self.gap),
        )
    }

    /// Transform `[s, 0, 0, s, x, y]` drawing a `w × h` page centred in cell `slot`.
    pub fn place(&self, slot: usize, order: NupOrder, (w, h): (f32, f32)) -> [f32; 6] {
        let (cw, ch) = self.cell();
        let s = fit_scale((cw, ch), (w, h));
        let (x, y) = self.cell_origin(slot, order);
        [
            s,
            0.0,
            0.0,
            s,
            x + (cw - w * s) / 2.0,
            y + (ch - h * s) / 2.0,
        ]
    }
}

/// Largest scale fitting `page` into `cell` with the aspect ratio kept.
fn fit_scale((cw, ch): (f32, f32), (w, h): (f32, f32)) -> f32 {
    if w <= 0.0 || h <= 0.0 {
        return 1.0;
    }
    (cw / w).min(ch / h).max(0.0)
}

/// Layout of a sheet holding `per_sheet` pages of size `page` on `paper`
/// (`None`: the page's own size).
pub fn layout(per_sheet: u8, paper: Option<(f32, f32)>, page: (f32, f32)) -> SheetLayout {
    let n = if PER_SHEET.contains(&per_sheet) {
        per_sheet
    } else {
        1
    };
    if n == 1 && paper.is_none() {
        return SheetLayout {
            width: page.0,
            height: page.1,
            cols: 1,
            rows: 1,
            margin: 0.0,
            gap: 0.0,
            scale: 1.0,
        };
    }
    let (pw, ph) = paper.unwrap_or(page);
    let (margin, gap) = if n == 1 { (0.0, 0.0) } else { (MARGIN, GAP) };
    let mut best: Option<SheetLayout> = None;
    for (w, h) in [(pw, ph), (ph, pw)] {
        for &(cols, rows) in grids(n) {
            let mut l = SheetLayout {
                width: w,
                height: h,
                cols,
                rows,
                margin,
                gap,
                scale: 0.0,
            };
            l.scale = fit_scale(l.cell(), page);
            if best.is_none_or(|b| l.scale > b.scale + 1e-4) {
                best = Some(l);
            }
        }
    }
    best.unwrap_or(SheetLayout {
        width: pw,
        height: ph,
        cols: 1,
        rows: 1,
        margin,
        gap,
        scale: 1.0,
    })
}

/// Thin black frame around a placed page.
fn frame(x: f32, y: f32, w: f32, h: f32) -> Command {
    Command::Path {
        segs: vec![
            PathSeg::M(x, y),
            PathSeg::L(x + w, y),
            PathSeg::L(x + w, y + h),
            PathSeg::L(x, y + h),
            PathSeg::Z,
        ],
        fill: None,
        stroke: Some(Stroke {
            width: 0.5,
            color: Rgba(0, 0, 0, 255),
            dash: Vec::new(),
            cap: "butt".into(),
            join: "miter".into(),
        }),
    }
}

/// Puts `pages` on sheets of `per_sheet` pages; `frame` draws a thin border
/// around every placed page. The layout of each sheet follows its first page.
pub fn compose(
    pages: Vec<PageList>,
    per_sheet: u8,
    order: NupOrder,
    paper: Option<(f32, f32)>,
    framed: bool,
) -> Vec<PageList> {
    let n = if PER_SHEET.contains(&per_sheet) {
        usize::from(per_sheet)
    } else {
        1
    };
    let mut out = Vec::with_capacity(pages.len().div_ceil(n));
    let mut it = pages.into_iter().peekable();
    while let Some(first) = it.peek() {
        let l = layout(per_sheet, paper, (first.width, first.height));
        // One page at its own size: kept as it is.
        if n == 1 && paper.is_none() {
            let Some(mut p) = it.next() else { break };
            if framed {
                p.commands.push(frame(0.0, 0.0, p.width, p.height));
            }
            out.push(p);
            continue;
        }
        let mut commands = Vec::new();
        for slot in 0..n {
            let Some(p) = it.next() else { break };
            let m = l.place(slot, order, (p.width, p.height));
            commands.push(Command::PushTransform { m });
            commands.push(Command::PushClip {
                x: 0.0,
                y: 0.0,
                w: p.width,
                h: p.height,
            });
            commands.extend(p.commands);
            commands.push(Command::PopClip);
            commands.push(Command::PopTransform);
            if framed {
                commands.push(frame(m[4], m[5], p.width * m[0], p.height * m[3]));
            }
        }
        out.push(PageList {
            width: l.width,
            height: l.height,
            commands,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f32, b: f32) {
        assert!((a - b).abs() < 0.01, "{a} != {b}");
    }

    const LETTER: (f32, f32) = (612.0, 792.0);

    #[test]
    fn one_page_without_paper_is_unchanged() {
        let l = layout(1, None, (500.0, 700.0));
        assert_eq!((l.width, l.height, l.cols, l.rows), (500.0, 700.0, 1, 1));
        assert_eq!(
            l.place(0, NupOrder::Across, (500.0, 700.0)),
            [1.0, 0.0, 0.0, 1.0, 0.0, 0.0]
        );
    }

    #[test]
    fn scale_to_paper_fits_and_centres() {
        // A 960 × 540 slide on Letter: turning the paper (792 × 612) gives
        // s = min(792/960, 612/540) = 0.825 > portrait min(612/960, 792/540) = 0.6375.
        let l = layout(1, Some(LETTER), (960.0, 540.0));
        assert_eq!((l.width, l.height), (792.0, 612.0));
        let m = l.place(0, NupOrder::Across, (960.0, 540.0));
        close(m[0], 0.825);
        close(m[4], 0.0);
        // Height 540 × 0.825 = 445.5; centred: (612 − 445.5) / 2 = 83.25.
        close(m[5], 83.25);
    }

    #[test]
    fn four_up_letter_on_letter() {
        // Cells: (612 − 36 − 9) / 2 = 283.5 wide, (792 − 36 − 9) / 2 = 373.5 high;
        // s = min(283.5/612, 373.5/792) = 0.463235; page 283.5 × 366.88.
        // Turned paper: min(373.5/612, 283.5/792) = 0.3580 — smaller.
        let l = layout(4, Some(LETTER), LETTER);
        assert_eq!((l.width, l.height, l.cols, l.rows), (612.0, 792.0, 2, 2));
        let s = 283.5 / 612.0;
        let dy = (373.5 - 792.0 * s) / 2.0; // 3.309
        let expect = [
            (18.0, 18.0 + dy),
            (310.5, 18.0 + dy),
            (18.0, 400.5 + dy),
            (310.5, 400.5 + dy),
        ];
        for (slot, (x, y)) in expect.iter().enumerate() {
            let m = l.place(slot, NupOrder::Across, LETTER);
            close(m[0], s);
            close(m[3], s);
            close(m[4], *x);
            close(m[5], *y);
        }
        // Down: the second page goes under the first.
        let m = l.place(1, NupOrder::Down, LETTER);
        close(m[4], 18.0);
        close(m[5], 400.5 + dy);
    }

    #[test]
    fn two_up_turns_the_paper() {
        // Portrait 1 × 2: cells 576 × 373.5 → s = 0.4716; 2 × 1: 283.5 × 756 → 0.4632.
        // Landscape 2 × 1: cells (792 − 45)/2 = 373.5 × 576 → s = min(0.6103, 0.7273) = 0.6103.
        let l = layout(2, Some(LETTER), LETTER);
        assert_eq!((l.width, l.height, l.cols, l.rows), (792.0, 612.0, 2, 1));
        close(l.scale, 373.5 / 612.0);
        let m = l.place(1, NupOrder::Across, LETTER);
        // x = 18 + 373.5 + 9 = 400.5 (page fills the cell width);
        // y = 18 + (576 − 792 × 0.6103) / 2 = 18 + 46.32 = 46.32 + 18.
        close(m[4], 400.5);
        close(m[5], 18.0 + (576.0 - 792.0 * (373.5 / 612.0)) / 2.0);
    }

    #[test]
    fn grids_for_six_eight_sixteen() {
        // Six portrait Letter pages: landscape 3 × 2 cells (792−36−18)/3 = 246 ×
        // (612−36−9)/2 = 283.5 → min(0.4020, 0.3580) = 0.3580; portrait 2 × 3 cells
        // 283.5 × (792−36−18)/3 = 246 → min(0.4632, 0.3106) = 0.3106; portrait 3 × 2
        // cells 186 × 373.5 → 0.3039; landscape 2 × 3 cells 373.5 × 186 → 0.2348.
        let l = layout(6, Some(LETTER), LETTER);
        assert_eq!((l.width, l.cols, l.rows), (792.0, 3, 2));
        close(l.scale, 283.5 / 792.0);
        // Sixteen: 4 × 4 on portrait: cells (612−36−27)/4 = 137.25 × (792−36−27)/4 = 182.25.
        let l = layout(16, Some(LETTER), LETTER);
        assert_eq!((l.width, l.cols, l.rows), (612.0, 4, 4));
        close(l.scale, (137.25_f32 / 612.0).min(182.25 / 792.0));
        // Eight: landscape 4 × 2 cells (792−36−27)/4 = 182.25 × 283.5 → min(0.2978, 0.3580).
        let l = layout(8, Some(LETTER), LETTER);
        assert_eq!((l.width, l.cols, l.rows), (792.0, 4, 2));
        close(l.scale, 182.25 / 612.0);
    }

    #[test]
    fn compose_groups_and_clips() {
        let page = |r: u8| PageList {
            width: 612.0,
            height: 792.0,
            commands: vec![Command::FillRect {
                x: 0.0,
                y: 0.0,
                w: 10.0,
                h: 10.0,
                color: Rgba(r, 0, 0, 255),
            }],
        };
        let sheets = compose(
            (1..=5).map(page).collect(),
            4,
            NupOrder::Across,
            Some(LETTER),
            true,
        );
        assert_eq!(sheets.len(), 2);
        // Sheet 1: four pages, each Push/Clip/rect/Pop/Pop + frame = 6 commands.
        assert_eq!(sheets[0].commands.len(), 24);
        assert!(
            matches!(sheets[0].commands[1], Command::PushClip { w, h, .. } if w == 612.0 && h == 792.0)
        );
        assert_eq!(sheets[1].commands.len(), 6);
        // Unknown counts fall back to one page per sheet.
        assert_eq!(
            compose(
                (1..=3).map(page).collect(),
                5,
                NupOrder::Across,
                None,
                false
            )
            .len(),
            3
        );
    }
}
