//! Viewport → visible cells, with frozen panes (ECMA-376 Part 1, §18.3.1.66 `pane`).
//!
//! The screen is split into up to four regions: the frozen corner (top-left), the
//! frozen rows (top), the frozen columns (left) and the scrollable main region. Each
//! [`PaneRegion`] tells which rows/columns are visible and where they are drawn.
//! All values are in points; the UI multiplies by zoom × devicePixelRatio.

use crate::geometry::{Rect, SheetGeometry};
use bo_model::sheet::{CellRef, PaneState, Worksheet};
use std::ops::Range;

/// What the user is looking at.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Viewport {
    /// Horizontal scroll of the scrollable region, from the first unfrozen column.
    pub scroll_x: f64,
    /// Vertical scroll of the scrollable region, from the first unfrozen row.
    pub scroll_y: f64,
    /// Size of the drawing surface, headers included.
    pub width: f64,
    /// Height of the drawing surface, headers included.
    pub height: f64,
    /// Width of the row-number header band (0 = hidden).
    pub header_width: f64,
    /// Height of the column-letter header band (0 = hidden).
    pub header_height: f64,
}

impl Viewport {
    /// A viewport at the top-left of the sheet with default header sizes.
    pub fn new(width: f64, height: f64) -> Viewport {
        Viewport {
            scroll_x: 0.0,
            scroll_y: 0.0,
            width,
            height,
            header_width: 30.0,
            header_height: 15.0,
        }
    }
}

/// Which quadrant a region is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaneKind {
    /// Frozen rows x frozen columns (top-left, never scrolls).
    Corner,
    /// Frozen rows (scrolls horizontally).
    Top,
    /// Frozen columns (scrolls vertically).
    Left,
    /// The scrollable region.
    Main,
}

/// One visible region of the grid.
#[derive(Debug, Clone, PartialEq)]
pub struct PaneRegion {
    /// Which quadrant this region is.
    pub kind: PaneKind,
    /// Visible rows (may include hidden rows of size 0).
    pub rows: Range<u32>,
    /// Visible columns.
    pub cols: Range<u32>,
    /// Screen rectangle of the region (clip rectangle).
    pub screen: Rect,
    /// Sheet coordinate drawn at `screen.x` / `screen.y`.
    pub sheet_x: f64,
    /// Sheet y coordinate drawn at `screen.y`.
    pub sheet_y: f64,
}

impl PaneRegion {
    /// Screen x of a sheet x inside this region.
    pub fn to_screen_x(&self, x: f64) -> f64 {
        self.screen.x + x - self.sheet_x
    }

    /// Screen y of a sheet y inside this region.
    pub fn to_screen_y(&self, y: f64) -> f64 {
        self.screen.y + y - self.sheet_y
    }

    fn contains(&self, x: f64, y: f64) -> bool {
        x >= self.screen.x
            && y >= self.screen.y
            && x < self.screen.x + self.screen.w
            && y < self.screen.y + self.screen.h
    }
}

/// Result of hit-testing a screen point.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hit {
    /// A cell (merged cells resolve to their anchor).
    Cell(CellRef),
    /// The header of a column.
    ColumnHeader(u32),
    /// The header of a row.
    RowHeader(u32),
    /// The select-all corner.
    Corner,
    /// Outside the grid.
    None,
}

/// Layout of the visible regions.
#[derive(Debug, Clone, PartialEq)]
pub struct ViewLayout {
    /// Viewport the layout was computed for.
    pub viewport: Viewport,
    /// Number of frozen rows (`pane/@ySplit`, §18.3.1.66).
    pub frozen_rows: u32,
    /// Number of frozen columns (`pane/@xSplit`).
    pub frozen_cols: u32,
    /// Regions in drawing order: main, left, top, corner.
    pub panes: Vec<PaneRegion>,
}

/// Frozen rows/columns of a sheet (`pane` with state `frozen` / `frozenSplit`).
/// Split panes are not frozen and yield `(0, 0)`.
pub fn frozen_counts(ws: &Worksheet) -> (u32, u32) {
    ws.views
        .first()
        .and_then(|v| v.pane.as_ref())
        .filter(|p| p.state != PaneState::Split)
        .map_or((0, 0), |p| {
            (p.y_split.max(0.0) as u32, p.x_split.max(0.0) as u32)
        })
}

fn visible(axis: &crate::geometry::Axis, start: f64, len: f64) -> Range<u32> {
    if len <= 0.0 {
        return 0..0;
    }
    let first = axis.index_at(start);
    let last = axis.index_at(start + len);
    first..(last + 1).min(axis.count())
}

impl ViewLayout {
    /// Compute the visible regions for `vp` with `frozen_rows` × `frozen_cols` frozen.
    pub fn compute(
        geom: &SheetGeometry,
        vp: &Viewport,
        frozen_rows: u32,
        frozen_cols: u32,
    ) -> ViewLayout {
        let fr = frozen_rows.min(geom.rows.count());
        let fc = frozen_cols.min(geom.cols.count());
        let fw = geom.col_x(fc).min((vp.width - vp.header_width).max(0.0));
        let fh = geom.row_y(fr).min((vp.height - vp.header_height).max(0.0));
        let x0 = vp.header_width;
        let y0 = vp.header_height;
        let main_w = (vp.width - x0 - fw).max(0.0);
        let main_h = (vp.height - y0 - fh).max(0.0);
        let sx = geom.col_x(fc) + vp.scroll_x.max(0.0);
        let sy = geom.row_y(fr) + vp.scroll_y.max(0.0);
        let mut panes = Vec::with_capacity(4);
        let main_cols = visible(&geom.cols, sx, main_w);
        let main_rows = visible(&geom.rows, sy, main_h);
        panes.push(PaneRegion {
            kind: PaneKind::Main,
            rows: main_rows.clone(),
            cols: main_cols.clone(),
            screen: Rect {
                x: x0 + fw,
                y: y0 + fh,
                w: main_w,
                h: main_h,
            },
            sheet_x: sx,
            sheet_y: sy,
        });
        if fc > 0 {
            panes.push(PaneRegion {
                kind: PaneKind::Left,
                rows: main_rows,
                cols: 0..fc,
                screen: Rect {
                    x: x0,
                    y: y0 + fh,
                    w: fw,
                    h: main_h,
                },
                sheet_x: 0.0,
                sheet_y: sy,
            });
        }
        if fr > 0 {
            panes.push(PaneRegion {
                kind: PaneKind::Top,
                rows: 0..fr,
                cols: main_cols,
                screen: Rect {
                    x: x0 + fw,
                    y: y0,
                    w: main_w,
                    h: fh,
                },
                sheet_x: sx,
                sheet_y: 0.0,
            });
        }
        if fr > 0 && fc > 0 {
            panes.push(PaneRegion {
                kind: PaneKind::Corner,
                rows: 0..fr,
                cols: 0..fc,
                screen: Rect {
                    x: x0,
                    y: y0,
                    w: fw,
                    h: fh,
                },
                sheet_x: 0.0,
                sheet_y: 0.0,
            });
        }
        ViewLayout {
            viewport: *vp,
            frozen_rows: fr,
            frozen_cols: fc,
            panes,
        }
    }

    /// Layout using the sheet's own frozen panes.
    pub fn for_sheet(geom: &SheetGeometry, ws: &Worksheet, vp: &Viewport) -> ViewLayout {
        let (r, c) = frozen_counts(ws);
        Self::compute(geom, vp, r, c)
    }

    /// The region that displays sheet row `row` and column `col`.
    pub fn pane_of(&self, row: u32, col: u32) -> Option<&PaneRegion> {
        let kind = match (row < self.frozen_rows, col < self.frozen_cols) {
            (true, true) => PaneKind::Corner,
            (true, false) => PaneKind::Top,
            (false, true) => PaneKind::Left,
            (false, false) => PaneKind::Main,
        };
        self.panes.iter().find(|p| p.kind == kind)
    }

    /// Screen rectangle of a cell (merged cells: the whole merge), if its region exists.
    pub fn cell_screen_rect(&self, geom: &SheetGeometry, row: u32, col: u32) -> Option<Rect> {
        let p = self.pane_of(row, col)?;
        let r = geom.cell_rect(row, col);
        Some(Rect {
            x: p.to_screen_x(r.x),
            y: p.to_screen_y(r.y),
            w: r.w,
            h: r.h,
        })
    }

    /// Hit-test a screen point (points, relative to the drawing surface).
    pub fn hit_test(&self, geom: &SheetGeometry, x: f64, y: f64) -> Hit {
        let vp = &self.viewport;
        if x < 0.0 || y < 0.0 || x >= vp.width || y >= vp.height {
            return Hit::None;
        }
        let in_hx = x < vp.header_width;
        let in_hy = y < vp.header_height;
        if in_hx && in_hy {
            return Hit::Corner;
        }
        // Map the coordinate through the region under it (headers use the region
        // on the same row / column).
        let px = if in_hx { vp.header_width } else { x };
        let py = if in_hy { vp.header_height } else { y };
        let Some(p) = self.panes.iter().find(|p| p.contains(px, py)) else {
            return Hit::None;
        };
        let sx = x - p.screen.x + p.sheet_x;
        let sy = y - p.screen.y + p.sheet_y;
        if in_hy {
            return Hit::ColumnHeader(geom.cols.index_at(sx));
        }
        if in_hx {
            return Hit::RowHeader(geom.rows.index_at(sy));
        }
        Hit::Cell(geom.hit_test(sx, sy))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::SheetMetrics;
    use bo_model::sheet::{Pane, SheetView};

    fn geom() -> SheetGeometry {
        SheetGeometry::with_metrics(&Worksheet::new("S"), SheetMetrics::default())
    }

    #[test]
    fn visible_range_without_freeze() {
        let g = geom();
        // 48 pt columns, 15 pt rows; 30 pt / 15 pt headers.
        let vp = Viewport::new(30.0 + 48.0 * 4.0, 15.0 + 15.0 * 10.0);
        let l = ViewLayout::compute(&g, &vp, 0, 0);
        assert_eq!(l.panes.len(), 1);
        let m = &l.panes[0];
        assert_eq!(
            m.cols,
            0..5,
            "the column at the right edge is partially visible"
        );
        assert_eq!(m.rows, 0..11);
        let mut vp2 = vp;
        vp2.scroll_x = 100.0;
        vp2.scroll_y = 1500.0;
        let l = ViewLayout::compute(&g, &vp2, 0, 0);
        assert_eq!(l.panes[0].cols.start, 2);
        assert_eq!(l.panes[0].rows.start, 100);
        assert_eq!(l.hit_test(&g, 31.0, 16.0), Hit::Cell(CellRef::new(100, 2)));
    }

    #[test]
    fn frozen_panes_and_hit_test() {
        let g = geom();
        let mut ws = Worksheet::new("S");
        ws.views.push(SheetView {
            pane: Some(Pane {
                x_split: 1.0,
                y_split: 2.0,
                state: PaneState::Frozen,
                ..Pane::default()
            }),
            ..SheetView::default()
        });
        assert_eq!(frozen_counts(&ws), (2, 1));
        let mut vp = Viewport::new(400.0, 300.0);
        vp.scroll_y = 15.0 * 50.0;
        let l = ViewLayout::for_sheet(&g, &ws, &vp);
        assert_eq!(l.panes.len(), 4);
        let corner = l.pane_of(0, 0).unwrap();
        assert_eq!(corner.kind, PaneKind::Corner);
        assert_eq!((corner.rows.clone(), corner.cols.clone()), (0..2, 0..1));
        let main = l.pane_of(60, 3).unwrap();
        assert_eq!(main.rows.start, 52);
        // Frozen row 1 stays on screen while scrolled.
        assert_eq!(l.hit_test(&g, 31.0, 16.0), Hit::Cell(CellRef::new(0, 0)));
        assert_eq!(l.hit_test(&g, 80.0, 50.0), Hit::Cell(CellRef::new(52, 1)));
        assert_eq!(l.hit_test(&g, 5.0, 5.0), Hit::Corner);
        assert_eq!(l.hit_test(&g, 80.0, 5.0), Hit::ColumnHeader(1));
        assert_eq!(l.hit_test(&g, 5.0, 50.0), Hit::RowHeader(52));
        assert_eq!(l.hit_test(&g, -1.0, 5.0), Hit::None);
        let r = l.cell_screen_rect(&g, 52, 1).unwrap();
        assert_eq!((r.x, r.y), (78.0, 45.0));
    }
}
