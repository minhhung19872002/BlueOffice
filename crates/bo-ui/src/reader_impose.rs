//! Where printed pages go on the paper (BlueReader round 3): one page per sheet sized to fit,
//! shrunk, at 100 % or at a custom scale; several pages per sheet (2 / 4 / 6 / 9 / 16) in
//! one of four orders with an optional border; a booklet (sheets folded in the middle, pages
//! in saddle-stitch order, left or right binding, both sides or one side only); the current
//! view (a part of a page). The host prints each [`Sheet`] as one side of paper.
//!
//! Pure geometry so it is tested headless; all lengths are points, the origin is the top-left
//! of the printable area.

/// How pages are sized on paper.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrintScaling {
    /// Grow or shrink to the printable area.
    Fit,
    /// Shrink oversized pages only.
    Shrink,
    /// 100 %.
    Actual,
    /// A percentage of the page's size.
    Custom(u16),
}

/// Order of pages on a sheet with several pages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageOrder {
    /// Rows left to right, top to bottom.
    Horizontal,
    /// Rows right to left, top to bottom.
    HorizontalReversed,
    /// Columns top to bottom, left to right.
    Vertical,
    /// Columns top to bottom, right to left.
    VerticalReversed,
}

/// Which sides of booklet sheets are printed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BookletSides {
    /// Front and back.
    Both,
    /// Front sides only (print, turn the stack over, then print the back sides).
    Front,
    /// Back sides only.
    Back,
}

/// How pages are put on sheets.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PrintLayout {
    /// One page per sheet, sized by the scaling.
    Size,
    /// Several pages per sheet.
    Multiple {
        /// Pages per sheet: 2, 4, 6, 9 or 16.
        per_sheet: u8,
        /// Order of the pages.
        order: PageOrder,
        /// Frame each page.
        border: bool,
        /// Space between the pages (points; the reference's Margins box, round 14).
        margin: f32,
        /// Columns × rows chosen with "Custom…" in place of `per_sheet` (round 14).
        custom: Option<(u8, u8)>,
    },
    /// Folded booklet, two pages on each side of a sheet.
    Booklet {
        /// Sides printed.
        sides: BookletSides,
        /// Bound on the right (right-to-left reading).
        right_binding: bool,
        /// Space kept round the pages at the sheet's outer edges (points, round 14).
        margin: f32,
    },
    /// Large pages split over several sheets (the reference's Tile Large Pages).
    Tile {
        /// Page zoom in percent.
        zoom: u16,
        /// Overlap of neighbouring tiles (points).
        overlap: f32,
        /// Corner marks where the tile's edges are cut.
        cut_marks: bool,
        /// A label with the page and the tile's row and column.
        labels: bool,
    },
}

/// Printing on both sides of the paper.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Duplex {
    /// One side.
    Off,
    /// Flip on the long edge.
    LongEdge,
    /// Flip on the short edge.
    ShortEdge,
}

/// One page drawn on a sheet.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placement {
    /// Page index.
    pub page: usize,
    /// Part of the page printed (page points, x y w h); `None` = the whole page.
    pub clip: Option<[f32; 4]>,
    /// Where it goes on the printable area (points, x y w h, after turning).
    pub dest: [f32; 4],
    /// The page image is turned a quarter clockwise.
    pub turn: bool,
    /// A thin frame is drawn around it.
    pub border: bool,
    /// Tile of a tiled page: (row, column), 1-based, for its label.
    pub tile: Option<(u16, u16)>,
    /// Cut marks at the corners of `dest`.
    pub cut_marks: bool,
}

/// One printed side of paper.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Sheet {
    /// Pages on it (none for a blank booklet side).
    pub items: Vec<Placement>,
}

/// What [`plan`] needs to know.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlanOptions {
    /// Page sizing (one page per sheet).
    pub scaling: PrintScaling,
    /// Turn pages to the paper's orientation.
    pub auto_rotate: bool,
    /// Centre pages (else top-left).
    pub auto_center: bool,
    /// Pages per sheet / booklet.
    pub layout: PrintLayout,
    /// Copies.
    pub copies: u32,
    /// Copies collated.
    pub collate: bool,
}

/// Grid (columns, rows) of `n` pages per sheet on a portrait sheet.
#[must_use]
pub fn grid(n: u8) -> (usize, usize) {
    match n {
        0..=2 => (1, 2),
        3..=4 => (2, 2),
        5..=6 => (2, 3),
        7..=9 => (3, 3),
        _ => (4, 4),
    }
}

/// `size` (w, h) fitted into `cell` and centred (or top-left), scaled by at most `max`.
fn fit_into(size: (f32, f32), cell: [f32; 4], max: f32, center: bool) -> [f32; 4] {
    let k = (cell[2] / size.0.max(0.01))
        .min(cell[3] / size.1.max(0.01))
        .min(max);
    let (w, h) = (size.0 * k, size.1 * k);
    if center {
        [
            cell[0] + (cell[2] - w) / 2.0,
            cell[1] + (cell[3] - h) / 2.0,
            w,
            h,
        ]
    } else {
        [cell[0], cell[1], w, h]
    }
}

/// A rectangle of a sheet laid out landscape (`area` swapped) on the real portrait sheet: the
/// layout turned a quarter clockwise.
fn turn_rect(r: [f32; 4], area: (f32, f32)) -> [f32; 4] {
    // Virtual (vx, vy) on an (area.1 × area.0) sheet → real (area.0 − vy, vx).
    [area.0 - r[1] - r[3], r[0], r[3], r[2]]
}

/// The sheets printing `pages` (indices in print order) of sizes `size_of` (points, the part
/// printed when `view` is set) on a printable area of `area` points.
#[must_use]
pub fn plan(
    pages: &[usize],
    size_of: &dyn Fn(usize) -> (f32, f32),
    view: Option<[f32; 4]>,
    area: (f32, f32),
    o: &PlanOptions,
) -> Vec<Sheet> {
    let size = |i: usize| view.map_or_else(|| size_of(i), |v| (v[2], v[3]));
    let mut sheets = match o.layout {
        PrintLayout::Size => pages
            .iter()
            .map(|&i| Sheet {
                items: vec![one_page(i, size(i), view, area, o)],
            })
            .collect(),
        PrintLayout::Multiple {
            per_sheet,
            order,
            border,
            margin,
            custom,
        } => {
            let g = Grid {
                cells: custom.map_or_else(
                    || grid(per_sheet),
                    |(c, r)| (usize::from(c.max(1)), usize::from(r.max(1))),
                ),
                order,
                border,
                margin,
            };
            multiple(pages, &size, view, area, &g, o)
        }
        PrintLayout::Booklet {
            sides,
            right_binding,
            margin,
        } => booklet(pages, &size, area, sides, right_binding, margin),
        PrintLayout::Tile {
            zoom,
            overlap,
            cut_marks,
            labels,
        } => pages
            .iter()
            .flat_map(|&i| tiles(i, size(i), view, area, zoom, overlap, cut_marks, labels, o))
            .collect(),
    };
    let n = o.copies.clamp(1, 999) as usize;
    if n > 1 {
        let booklet = matches!(o.layout, PrintLayout::Booklet { .. });
        sheets = if o.collate || booklet {
            (0..n).flat_map(|_| sheets.iter().cloned()).collect()
        } else {
            sheets
                .iter()
                .flat_map(|s| std::iter::repeat_n(s.clone(), n))
                .collect()
        };
    }
    sheets
}

fn one_page(
    i: usize,
    (w, h): (f32, f32),
    view: Option<[f32; 4]>,
    area: (f32, f32),
    o: &PlanOptions,
) -> Placement {
    // Turn the page when its orientation differs from the paper's.
    // The current view prints upright, as it is seen (only whole pages are auto-rotated).
    let turn =
        o.auto_rotate && view.is_none() && (w > h) != (area.0 > area.1) && (w - h).abs() > 1.0;
    let (pw, ph) = if turn { (h, w) } else { (w, h) };
    let fit = (area.0 / pw.max(0.01)).min(area.1 / ph.max(0.01));
    let k = match o.scaling {
        PrintScaling::Fit => fit,
        PrintScaling::Shrink => fit.min(1.0),
        PrintScaling::Actual => 1.0,
        PrintScaling::Custom(p) => f32::from(p.clamp(1, 1000)) / 100.0,
    };
    let (dw, dh) = (pw * k, ph * k);
    let (x, y) = if o.auto_center {
        ((area.0 - dw) / 2.0, (area.1 - dh) / 2.0)
    } else {
        (0.0, 0.0)
    };
    Placement {
        page: i,
        clip: view,
        dest: [x, y, dw, dh],
        turn,
        border: false,
        tile: None,
        cut_marks: false,
    }
}

/// A sheet of several pages: grid (columns, rows), order, frames, space between pages.
struct Grid {
    cells: (usize, usize),
    order: PageOrder,
    border: bool,
    margin: f32,
}

fn multiple(
    pages: &[usize],
    size: &dyn Fn(usize) -> (f32, f32),
    view: Option<[f32; 4]>,
    area: (f32, f32),
    g: &Grid,
    o: &PlanOptions,
) -> Vec<Sheet> {
    let (c, r) = g.cells;
    let (order, border) = (g.order, g.border);
    let n = (c * r).max(1);
    // Lay the grid on the sheet as is or turned (2 and 6 pages on portrait paper go
    // landscape), whichever prints the first page larger.
    let first = pages.first().map_or((612.0, 792.0), |&i| size(i));
    let scale_of = |cols: usize, rows: usize, a: (f32, f32)| {
        let cell = (a.0 / cols as f32, a.1 / rows as f32);
        (cell.0 / first.0.max(0.01)).min(cell.1 / first.1.max(0.01))
    };
    let turned = o.auto_rotate && c != r && scale_of(r, c, (area.1, area.0)) > scale_of(c, r, area);
    let (cols, rows, sheet) = if turned {
        (r, c, (area.1, area.0))
    } else {
        (c, r, area)
    };
    // The Margins box: space between the cells (at most a third of a cell).
    let gap = g
        .margin
        .max(0.0)
        .min(sheet.0 / cols as f32 / 3.0)
        .min(sheet.1 / rows as f32 / 3.0);
    let cw = (sheet.0 - gap * (cols as f32 - 1.0)) / cols as f32;
    let ch = (sheet.1 - gap * (rows as f32 - 1.0)) / rows as f32;
    pages
        .chunks(n)
        .map(|chunk| {
            let items = chunk
                .iter()
                .enumerate()
                .map(|(k, &i)| {
                    let (col, row) = match order {
                        PageOrder::Horizontal => (k % cols, k / cols),
                        PageOrder::HorizontalReversed => (cols - 1 - k % cols, k / cols),
                        PageOrder::Vertical => (k / rows, k % rows),
                        PageOrder::VerticalReversed => (cols - 1 - k / rows, k % rows),
                    };
                    let cell = [col as f32 * (cw + gap), row as f32 * (ch + gap), cw, ch];
                    let d = fit_into(size(i), cell, f32::INFINITY, true);
                    Placement {
                        page: i,
                        clip: view,
                        dest: if turned { turn_rect(d, area) } else { d },
                        turn: turned,
                        border,
                        tile: None,
                        cut_marks: false,
                    }
                })
                .collect();
            Sheet { items }
        })
        .collect()
}

/// Number of tiles of `len` points on paper `paper` long with `overlap`.
fn tile_count(len: f32, paper: f32, overlap: f32) -> usize {
    if len <= paper + 0.01 {
        return 1;
    }
    let step = (paper - overlap).max(paper * 0.1);
    ((len - overlap) / step).ceil().max(1.0) as usize
}

/// The tiles of page `i` at `zoom` percent: one sheet per tile, row by row; each prints its
/// part of the page at the top-left of the printable area (centred when the page fits).
#[allow(clippy::too_many_arguments)]
fn tiles(
    i: usize,
    (w, h): (f32, f32),
    view: Option<[f32; 4]>,
    area: (f32, f32),
    zoom: u16,
    overlap: f32,
    cut_marks: bool,
    labels: bool,
    o: &PlanOptions,
) -> Vec<Sheet> {
    let k = f32::from(zoom.clamp(1, 1000)) / 100.0;
    let (sw, sh) = (w * k, h * k);
    let overlap = overlap.clamp(0.0, area.0.min(area.1) / 2.0);
    let (cols, rows) = (
        tile_count(sw, area.0, overlap),
        tile_count(sh, area.1, overlap),
    );
    let (ox, oy) = view.map_or((0.0, 0.0), |v| (v[0], v[1]));
    let mut out = Vec::with_capacity(cols * rows);
    for r in 0..rows {
        for c in 0..cols {
            // Part of the scaled page on this tile.
            let x0 = c as f32 * (area.0 - overlap);
            let y0 = r as f32 * (area.1 - overlap);
            let tw = (sw - x0).min(area.0);
            let th = (sh - y0).min(area.1);
            let (dx, dy) = if o.auto_center && cols == 1 && rows == 1 {
                ((area.0 - tw) / 2.0, (area.1 - th) / 2.0)
            } else {
                (0.0, 0.0)
            };
            out.push(Sheet {
                items: vec![Placement {
                    page: i,
                    clip: Some([ox + x0 / k, oy + y0 / k, tw / k, th / k]),
                    dest: [dx, dy, tw, th],
                    turn: false,
                    border: false,
                    tile: labels.then_some((r as u16 + 1, c as u16 + 1)),
                    cut_marks,
                }],
            });
        }
    }
    out
}

fn booklet(
    pages: &[usize],
    size: &dyn Fn(usize) -> (f32, f32),
    area: (f32, f32),
    sides: BookletSides,
    right_binding: bool,
    margin: f32,
) -> Vec<Sheet> {
    // Saddle stitch: pad to a multiple of four with blank pages; sheet s carries
    // (last − 2s, first + 2s) on its front and (first + 2s + 1, last − 2s − 1) on its back.
    let total = pages.len().div_ceil(4) * 4;
    let at = |k: usize| pages.get(k).copied();
    // Pages laid side by side on the sheet turned landscape when the paper is portrait.
    let portrait = area.1 >= area.0;
    let sheet = if portrait { (area.1, area.0) } else { area };
    let half = sheet.0 / 2.0;
    // Margins: kept free at the top, bottom and outer edge of each half (pages still meet at
    // the fold).
    let m = margin.max(0.0).min(half / 3.0).min(sheet.1 / 3.0);
    let side = |left: Option<usize>, right: Option<usize>| {
        let (left, right) = if right_binding {
            (right, left)
        } else {
            (left, right)
        };
        let mut items = Vec::new();
        for (slot, page) in [(0.0, left), (1.0, right)] {
            let Some(i) = page else { continue };
            let cell = [
                slot * half + (1.0 - slot) * m,
                m,
                half - m,
                sheet.1 - 2.0 * m,
            ];
            let mut d = fit_into(size(i), cell, f32::INFINITY, true);
            // Pages meet at the fold.
            d[0] = if slot == 0.0 { half - d[2] } else { half };
            items.push(Placement {
                page: i,
                clip: None,
                dest: if portrait { turn_rect(d, area) } else { d },
                turn: portrait,
                border: false,
                tile: None,
                cut_marks: false,
            });
        }
        Sheet { items }
    };
    let mut out = Vec::new();
    for s in 0..total / 4 {
        let (lo, hi) = (2 * s, total - 1 - 2 * s);
        if sides != BookletSides::Back {
            out.push(side(at(hi), at(lo)));
        }
        if sides != BookletSides::Front {
            out.push(side(at(lo + 1), at(hi - 1)));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const LETTER: (f32, f32) = (612.0, 792.0);

    fn opts(layout: PrintLayout) -> PlanOptions {
        PlanOptions {
            scaling: PrintScaling::Fit,
            auto_rotate: true,
            auto_center: true,
            layout,
            copies: 1,
            collate: true,
        }
    }

    fn near(a: [f32; 4], b: [f32; 4]) -> bool {
        a.iter().zip(b).all(|(x, y)| (x - y).abs() < 0.01)
    }

    #[test]
    fn one_page_per_sheet_fit_actual_custom() {
        let size = |_| (306.0, 396.0);
        let s = plan(&[0, 1], &size, None, LETTER, &opts(PrintLayout::Size));
        assert_eq!(s.len(), 2);
        assert!(near(s[0].items[0].dest, [0.0, 0.0, 612.0, 792.0]));
        let mut o = opts(PrintLayout::Size);
        o.scaling = PrintScaling::Actual;
        let s = plan(&[0], &size, None, LETTER, &o);
        assert!(near(s[0].items[0].dest, [153.0, 198.0, 306.0, 396.0]));
        o.scaling = PrintScaling::Custom(50);
        o.auto_center = false;
        let s = plan(&[0], &size, None, LETTER, &o);
        assert!(near(s[0].items[0].dest, [0.0, 0.0, 153.0, 198.0]));
        // Landscape pages turn on portrait paper.
        let wide = |_| (792.0, 612.0);
        let s = plan(&[0], &wide, None, LETTER, &opts(PrintLayout::Size));
        assert!(s[0].items[0].turn);
        assert!(near(s[0].items[0].dest, [0.0, 0.0, 612.0, 792.0]));
    }

    #[test]
    fn two_pages_go_side_by_side_on_turned_paper() {
        let size = |_| LETTER;
        let o = opts(PrintLayout::Multiple {
            per_sheet: 2,
            order: PageOrder::Horizontal,
            border: true,
            margin: 6.0,
            custom: None,
        });
        let s = plan(&[0, 1, 2], &size, None, LETTER, &o);
        assert_eq!(s.len(), 2);
        assert_eq!(s[1].items.len(), 1);
        let (a, b) = (s[0].items[0], s[0].items[1]);
        assert!(a.turn && b.turn && a.border);
        // Turned clockwise: the first page is at the top of the portrait sheet.
        assert!(a.dest[1] < b.dest[1]);
        // Each takes about half the sheet's height and is drawn landscape (turned).
        assert!(a.dest[2] > a.dest[3]);
        assert!(a.dest[3] < 400.0 && a.dest[3] > 380.0);
    }

    #[test]
    fn orders_of_four_and_nine() {
        let size = |_| LETTER;
        let pos = |order| {
            let o = opts(PrintLayout::Multiple {
                per_sheet: 4,
                order,
                border: false,
                margin: 6.0,
                custom: None,
            });
            let s = plan(&[0, 1, 2, 3], &size, None, LETTER, &o);
            s[0].items
                .iter()
                .map(|p| (p.dest[0] > 300.0, p.dest[1] > 390.0))
                .collect::<Vec<_>>()
        };
        assert_eq!(
            pos(PageOrder::Horizontal),
            vec![(false, false), (true, false), (false, true), (true, true)]
        );
        assert_eq!(
            pos(PageOrder::HorizontalReversed),
            vec![(true, false), (false, false), (true, true), (false, true)]
        );
        assert_eq!(
            pos(PageOrder::Vertical),
            vec![(false, false), (false, true), (true, false), (true, true)]
        );
        assert_eq!(
            pos(PageOrder::VerticalReversed),
            vec![(true, false), (true, true), (false, false), (false, true)]
        );
        for (n, cells) in [(6, 6), (9, 9), (16, 16)] {
            let o = opts(PrintLayout::Multiple {
                per_sheet: n,
                order: PageOrder::Horizontal,
                border: false,
                margin: 6.0,
                custom: None,
            });
            let pages: Vec<usize> = (0..40).collect();
            let s = plan(&pages, &size, None, LETTER, &o);
            assert_eq!(s[0].items.len(), cells);
            assert_eq!(s.len(), 40usize.div_ceil(cells));
        }
    }

    #[test]
    fn booklet_saddle_stitch_order() {
        let size = |_| LETTER;
        let pages: Vec<usize> = (0..6).collect();
        let o = opts(PrintLayout::Booklet {
            sides: BookletSides::Both,
            right_binding: false,
            margin: 0.0,
        });
        let s = plan(&pages, &size, None, LETTER, &o);
        // 6 pages → 8 slots → 2 sheets, 4 sides.
        let ids: Vec<Vec<usize>> = s
            .iter()
            .map(|sh| sh.items.iter().map(|p| p.page).collect())
            .collect();
        // Front 1: blank(8) | 1, back 1: 2 | blank(7), front 2: 6 | 3, back 2: 4 | 5.
        assert_eq!(ids, vec![vec![0], vec![1], vec![5, 2], vec![3, 4]]);
        // Left page ends at the fold (turned: the fold is the middle of the sheet height).
        let left = s[2].items[0];
        assert!(left.turn);
        assert!((left.dest[1] + left.dest[3] - 396.0).abs() < 0.01);
        let front = plan(
            &pages,
            &size,
            None,
            LETTER,
            &opts(PrintLayout::Booklet {
                sides: BookletSides::Front,
                right_binding: true,
                margin: 0.0,
            }),
        );
        assert_eq!(front.len(), 2);
        // Right binding swaps the halves.
        assert_eq!(
            front[1].items.iter().map(|p| p.page).collect::<Vec<_>>(),
            vec![2, 5]
        );
    }

    #[test]
    fn tile_large_pages() {
        // A 2 × 2 letter page at 100 % on letter paper with a 36 pt overlap: 3 × 3 tiles.
        let size = |_| (1224.0, 1584.0);
        let o = opts(PrintLayout::Tile {
            zoom: 100,
            overlap: 36.0,
            cut_marks: true,
            labels: true,
        });
        let s = plan(&[0], &size, None, LETTER, &o);
        assert_eq!(s.len(), 9);
        let first = s[0].items[0];
        assert_eq!(first.tile, Some((1, 1)));
        assert!(first.cut_marks);
        assert!(near(first.clip.expect("clip"), [0.0, 0.0, 612.0, 792.0]));
        // The second tile starts one paper width minus the overlap to the right.
        assert!(near(
            s[1].items[0].clip.expect("clip"),
            [576.0, 0.0, 612.0, 792.0]
        ));
        // The last column holds what is left.
        assert!((s[2].items[0].clip.expect("clip")[2] - (1224.0 - 1152.0)).abs() < 0.01);
        // At 50 % the page fits on one sheet, centred.
        let o = opts(PrintLayout::Tile {
            zoom: 50,
            overlap: 36.0,
            cut_marks: false,
            labels: false,
        });
        let s = plan(&[0], &size, None, LETTER, &o);
        assert_eq!(s.len(), 1);
        assert!(near(s[0].items[0].dest, [0.0, 0.0, 612.0, 792.0]));
        assert_eq!(s[0].items[0].tile, None);
    }

    #[test]
    fn copies_and_current_view() {
        let size = |_| LETTER;
        let mut o = opts(PrintLayout::Size);
        o.copies = 2;
        o.collate = false;
        let s = plan(&[0, 1], &size, None, LETTER, &o);
        let ids: Vec<usize> = s.iter().map(|sh| sh.items[0].page).collect();
        assert_eq!(ids, vec![0, 0, 1, 1]);
        o.collate = true;
        let s = plan(&[0, 1], &size, None, LETTER, &o);
        let ids: Vec<usize> = s.iter().map(|sh| sh.items[0].page).collect();
        assert_eq!(ids, vec![0, 1, 0, 1]);
        // The current view: a 300 × 200 pt part of page 3, upright, fitted to the width.
        let v = [100.0, 50.0, 300.0, 200.0];
        let s = plan(&[3], &size, Some(v), LETTER, &opts(PrintLayout::Size));
        assert_eq!(s[0].items[0].clip, Some(v));
        assert!(!s[0].items[0].turn);
        assert!(near(s[0].items[0].dest, [0.0, 192.0, 612.0, 408.0]));
    }

    #[test]
    fn custom_grid_margins_and_landscape_booklet() {
        let size = |_| LETTER;
        // 3 columns × 1 row ("Custom…") with a 0.2 inch margin between the pages.
        let mut o = opts(PrintLayout::Multiple {
            per_sheet: 4,
            order: PageOrder::Horizontal,
            border: true,
            margin: 14.4,
            custom: Some((1, 3)),
        });
        // As laid out (a 1 × 3 grid would otherwise turn to fit the first page larger).
        o.auto_rotate = false;
        let pages: Vec<usize> = (0..7).collect();
        let s = plan(&pages, &size, None, LETTER, &o);
        assert_eq!(s.len(), 3);
        assert_eq!(s[0].items.len(), 3);
        // One column of three rows: the second page starts one cell and a margin lower.
        let (a, b) = (s[0].items[0], s[0].items[1]);
        assert!(!a.turn);
        let cell = (792.0 - 2.0 * 14.4) / 3.0;
        assert!((b.dest[1] + b.dest[3] / 2.0 - (cell + 14.4 + cell / 2.0)).abs() < 0.01);
        // Margins off: pages touch.
        let o = opts(PrintLayout::Multiple {
            per_sheet: 4,
            order: PageOrder::Horizontal,
            border: false,
            margin: 0.0,
            custom: None,
        });
        let s = plan(&[0, 1, 2, 3], &size, None, LETTER, &o);
        assert!((s[0].items[0].dest[1] + s[0].items[0].dest[3] - 396.0).abs() < 0.01);
        // A booklet on landscape paper keeps the pages upright, meeting at the fold, inside
        // the margin at the outer edges.
        let o = opts(PrintLayout::Booklet {
            sides: BookletSides::Both,
            right_binding: false,
            margin: 14.4,
        });
        let s = plan(&[0, 1, 2, 3], &size, None, (792.0, 612.0), &o);
        let (l, r) = (s[0].items[0], s[0].items[1]);
        assert!(!l.turn && !r.turn);
        assert!((l.dest[0] + l.dest[2] - 396.0).abs() < 0.01);
        assert!((r.dest[0] - 396.0).abs() < 0.01);
        assert!(l.dest[1] >= 14.4 - 0.01 && l.dest[1] + l.dest[3] <= 612.0 - 14.4 + 0.01);
    }
}
