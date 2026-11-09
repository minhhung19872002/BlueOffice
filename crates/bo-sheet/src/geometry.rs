//! Grid geometry: column widths and row heights in points, cumulative offsets
//! with hidden rows/columns, hit-testing and merged-cell rectangles.
//!
//! Column widths follow ECMA-376 Part 1, §18.3.1.13 `col/@width`: a width is a
//! number of characters of the maximum digit width (MDW) of the default font,
//! including 5 px of padding, and converts to pixels as
//! `Truncate(((256 * width + Truncate(128 / MDW)) / 256) * MDW)`. Pixels are at
//! 96 dpi (1 px = 0.75 pt).

use bo_model::sheet::{CellRange, CellRef, Stylesheet, Worksheet, MAX_COLS, MAX_ROWS};
use bo_text::FontManager;

pub use bo_model::sheet::{col_to_letters, letters_to_col};

/// Points per pixel at 96 dpi.
pub const PT_PER_PX: f64 = 0.75;

/// Column width factor while a sheet shows formulas (`sheetView/@showFormulas`,
/// ECMA-376 Part 1, §18.3.1.87): formula text is usually longer than the result,
/// so every column is drawn twice as wide (our choice; the file widths are unchanged).
pub const SHOW_FORMULAS_WIDTH: f64 = 2.0;

/// Font-derived sheet metrics.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SheetMetrics {
    /// Maximum digit width of the default font, in pixels.
    pub mdw_px: f64,
    /// Default row height in points when the sheet does not specify one.
    pub default_row_height_pt: f64,
}

impl Default for SheetMetrics {
    /// Metrics of an 11 pt default font with a 7 px digit width.
    fn default() -> Self {
        SheetMetrics {
            mdw_px: 7.0,
            default_row_height_pt: 15.0,
        }
    }
}

impl SheetMetrics {
    /// Measure the default font (`fonts[0]` of the stylesheet, §18.8.23).
    pub fn from_styles(styles: &Stylesheet, fonts: &mut FontManager) -> SheetMetrics {
        let f = styles.fonts.first();
        let name = f.and_then(|f| f.name.as_deref()).unwrap_or("Calibri");
        let size = f.and_then(|f| f.size).unwrap_or(11.0);
        let Some(face) = fonts.resolve(name, false, false) else {
            return SheetMetrics {
                mdw_px: (size * 7.0 / 11.0).round().max(1.0),
                default_row_height_pt: size * 15.0 / 11.0,
            };
        };
        let digit_pt = (0..10)
            .map(|d| f64::from(fonts.measure(face, &d.to_string(), size as f32)))
            .fold(0.0, f64::max);
        let mdw_px = (digit_pt / PT_PER_PX).round().max(1.0);
        let info = fonts.info(face);
        let upem = f64::from(info.units_per_em.max(1));
        let line = f64::from(
            i32::from(info.ascender) - i32::from(info.descender) + i32::from(info.line_gap),
        ) * size
            / upem;
        // Text line in whole pixels plus 2 px of cell padding.
        let px = (line / PT_PER_PX).ceil() + 2.0;
        SheetMetrics {
            mdw_px,
            default_row_height_pt: px * PT_PER_PX,
        }
    }

    /// Convert a column width in characters (§18.3.1.13) to points.
    pub fn width_to_pt(&self, width: f64) -> f64 {
        let mdw = self.mdw_px;
        let px = (((256.0 * width + (128.0 / mdw).trunc()) / 256.0) * mdw).trunc();
        px.max(0.0) * PT_PER_PX
    }

    /// Default column width in points for a sheet (§18.3.1.81 `sheetFormatPr`):
    /// `defaultColWidth` when present, otherwise `baseColWidth` (default 8)
    /// characters plus 5 px padding, rounded up to a multiple of 8 px.
    pub fn default_col_pt(&self, ws: &Worksheet) -> f64 {
        if let Some(w) = ws.format.default_col_width {
            return self.width_to_pt(w);
        }
        let base = f64::from(ws.format.base_col_width.unwrap_or(8));
        let px = base * self.mdw_px + 5.0;
        ((px / 8.0).ceil() * 8.0) * PT_PER_PX
    }
}

/// One axis (rows or columns): a default size with sorted, non-overlapping
/// overrides, supporting O(log n) offset and index lookups.
#[derive(Debug, Clone, PartialEq)]
pub struct Axis {
    default: f64,
    count: u32,
    /// (first, last inclusive, size)
    overrides: Vec<(u32, u32, f64)>,
    /// Sum of (size − default) × len of all overrides before index i.
    prefix: Vec<f64>,
}

impl Axis {
    /// Build an axis. Overlapping overrides: later entries win where they overlap the start.
    pub fn new(default: f64, count: u32, mut overrides: Vec<(u32, u32, f64)>) -> Axis {
        overrides.retain(|o| o.0 <= o.1 && o.0 < count);
        overrides.sort_by_key(|o| o.0);
        let mut clean: Vec<(u32, u32, f64)> = Vec::with_capacity(overrides.len());
        for (a, b, s) in overrides {
            let b = b.min(count - 1);
            if let Some(last) = clean.last() {
                if a <= last.1 {
                    if b <= last.1 {
                        continue;
                    }
                    clean.push((last.1 + 1, b, s));
                    continue;
                }
            }
            clean.push((a, b, s));
        }
        let mut prefix = Vec::with_capacity(clean.len() + 1);
        let mut acc = 0.0;
        for (a, b, s) in &clean {
            prefix.push(acc);
            acc += (s - default) * f64::from(b - a + 1);
        }
        prefix.push(acc);
        Axis {
            default,
            count,
            overrides: clean,
            prefix,
        }
    }

    /// Number of entries on the axis.
    pub fn count(&self) -> u32 {
        self.count
    }

    /// Size of entry `i` in points (0 when hidden).
    pub fn size(&self, i: u32) -> f64 {
        let k = self.overrides.partition_point(|o| o.0 <= i);
        if k > 0 {
            let (_, b, s) = self.overrides[k - 1];
            if i <= b {
                return s;
            }
        }
        self.default
    }

    /// Offset of the start of entry `i` from the start of the axis.
    pub fn offset(&self, i: u32) -> f64 {
        let i = i.min(self.count);
        let k = self.overrides.partition_point(|o| o.0 < i);
        let mut off = f64::from(i) * self.default + self.prefix[k];
        if k > 0 {
            let (a, b, s) = self.overrides[k - 1];
            if i <= b {
                // Inside override k-1: remove the part not yet reached.
                off -= (s - self.default) * f64::from(b + 1 - i);
            }
            let _ = a;
        }
        off
    }

    /// Index of the entry containing position `pos` (clamped to the axis).
    pub fn index_at(&self, pos: f64) -> u32 {
        if pos <= 0.0 {
            return self.first_visible(0);
        }
        let (mut lo, mut hi) = (0u32, self.count);
        // Largest i with offset(i) <= pos.
        while hi - lo > 1 {
            let mid = lo + (hi - lo) / 2;
            if self.offset(mid) <= pos {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        lo
    }

    /// First entry ≥ `i` with a non-zero size (or `i` when none).
    pub fn first_visible(&self, i: u32) -> u32 {
        let mut j = i;
        while j < self.count && self.size(j) <= 0.0 {
            j += 1;
        }
        if j >= self.count {
            i
        } else {
            j
        }
    }
}

/// A rectangle in points.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Rect {
    /// Left edge in points.
    pub x: f64,
    /// Top edge in points.
    pub y: f64,
    /// Width in points.
    pub w: f64,
    /// Height in points.
    pub h: f64,
}

/// Geometry of one worksheet.
#[derive(Debug, Clone, PartialEq)]
pub struct SheetGeometry {
    /// Column widths and positions.
    pub cols: Axis,
    /// Row heights and positions.
    pub rows: Axis,
    /// Default sizes and font metrics used to convert units.
    pub metrics: SheetMetrics,
    merges: Vec<CellRange>,
}

impl SheetGeometry {
    /// Geometry using font metrics measured from the stylesheet's default font.
    pub fn new(ws: &Worksheet, styles: &Stylesheet, fonts: &mut FontManager) -> SheetGeometry {
        Self::with_metrics(ws, SheetMetrics::from_styles(styles, fonts))
    }

    /// Geometry from known metrics.
    pub fn with_metrics(ws: &Worksheet, metrics: SheetMetrics) -> SheetGeometry {
        let widen = if ws
            .views
            .first()
            .is_some_and(bo_model::sheet::SheetView::show_formulas)
        {
            SHOW_FORMULAS_WIDTH
        } else {
            1.0
        };
        let col_default = metrics.default_col_pt(ws) * widen;
        let cols = ws
            .cols
            .iter()
            .filter(|c| c.hidden || c.width.is_some())
            .map(|c| {
                let size = if c.hidden {
                    0.0
                } else {
                    c.width
                        .map_or(col_default, |w| metrics.width_to_pt(w) * widen)
                };
                (c.min, c.max, size)
            })
            .collect();
        let row_default = if ws.format.zero_height {
            0.0
        } else {
            ws.format
                .default_row_height
                .unwrap_or(metrics.default_row_height_pt)
        };
        let rows = ws
            .rows
            .iter()
            .filter(|(_, r)| r.hidden || r.height.is_some())
            .map(|(&i, r)| {
                let size = if r.hidden {
                    0.0
                } else {
                    r.height.unwrap_or(row_default)
                };
                (i, i, size)
            })
            .collect();
        SheetGeometry {
            cols: Axis::new(col_default, MAX_COLS, cols),
            rows: Axis::new(row_default, MAX_ROWS, rows),
            metrics,
            merges: ws.merges.clone(),
        }
    }

    /// Column width in points (0 if hidden).
    pub fn col_width(&self, col: u32) -> f64 {
        self.cols.size(col)
    }

    /// Row height in points (0 if hidden).
    pub fn row_height(&self, row: u32) -> f64 {
        self.rows.size(row)
    }

    /// X of the left edge of `col`.
    pub fn col_x(&self, col: u32) -> f64 {
        self.cols.offset(col)
    }

    /// Y of the top edge of `row`.
    pub fn row_y(&self, row: u32) -> f64 {
        self.rows.offset(row)
    }

    /// Merged range containing `c`.
    pub fn merge_at(&self, c: CellRef) -> Option<CellRange> {
        self.merges.iter().copied().find(|m| m.contains(c))
    }

    /// Merged ranges.
    pub fn merges(&self) -> &[CellRange] {
        &self.merges
    }

    /// Rectangle of a range.
    pub fn range_rect(&self, r: CellRange) -> Rect {
        let x = self.col_x(r.first.col);
        let y = self.row_y(r.first.row);
        Rect {
            x,
            y,
            w: self.col_x(r.last.col + 1) - x,
            h: self.row_y(r.last.row + 1) - y,
        }
    }

    /// Rectangle of a cell, expanded to its merged range if it belongs to one.
    pub fn cell_rect(&self, row: u32, col: u32) -> Rect {
        let c = CellRef::new(row, col);
        self.range_rect(self.merge_at(c).unwrap_or(CellRange::single(c)))
    }

    /// Cell at sheet position (x, y) in points; merged cells resolve to the
    /// top-left (anchor) cell of the merge.
    pub fn hit_test(&self, x: f64, y: f64) -> CellRef {
        let c = CellRef::new(self.rows.index_at(y), self.cols.index_at(x));
        self.merge_at(c).map_or(c, |m| m.first)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bo_model::sheet::{ColInfo, RowInfo};

    fn sheet() -> Worksheet {
        let mut ws = Worksheet::new("S");
        ws.cols.push(ColInfo {
            min: 1,
            max: 2,
            width: Some(20.0),
            custom_width: true,
            ..ColInfo::default()
        });
        ws.cols.push(ColInfo {
            min: 4,
            max: 4,
            hidden: true,
            ..ColInfo::default()
        });
        ws.rows.insert(
            1,
            RowInfo {
                height: Some(30.0),
                custom_height: true,
                ..RowInfo::default()
            },
        );
        ws.rows.insert(
            3,
            RowInfo {
                hidden: true,
                ..RowInfo::default()
            },
        );
        ws.merges.push(CellRange::parse("B2:C3").unwrap());
        ws
    }

    // source: ECMA-376 Part 1, §18.3.1.13 col/@width pixel formula, MDW = 7 px
    #[test]
    fn width_conversion() {
        let m = SheetMetrics::default();
        // 9.140625 characters (8.43 + padding) → 64 px → 48 pt.
        assert_eq!(m.width_to_pt(9.140_625), 48.0);
        // 20 characters → trunc((5120+18)/256*7) = 140 px → 105 pt.
        assert_eq!(m.width_to_pt(20.0), 105.0);
        assert_eq!(m.width_to_pt(0.0), 0.0);
        // Default: 8 × 7 + 5 = 61 px → 64 px → 48 pt.
        assert_eq!(m.default_col_pt(&Worksheet::new("x")), 48.0);
    }

    #[test]
    fn offsets_and_hidden() {
        let g = SheetGeometry::with_metrics(&sheet(), SheetMetrics::default());
        assert_eq!(g.col_width(0), 48.0);
        assert_eq!(g.col_width(1), 105.0);
        assert_eq!(g.col_width(4), 0.0);
        assert_eq!(g.col_x(0), 0.0);
        assert_eq!(g.col_x(1), 48.0);
        assert_eq!(g.col_x(2), 153.0);
        assert_eq!(g.col_x(3), 258.0);
        assert_eq!(g.col_x(5), 306.0, "hidden column E adds nothing");
        assert_eq!(g.col_x(6), 354.0);
        assert_eq!(g.row_y(1), 15.0);
        assert_eq!(g.row_y(2), 45.0);
        assert_eq!(g.row_y(4), 60.0, "hidden row 4");
        assert_eq!(g.row_y(5), 75.0);
        assert_eq!(g.row_y(1000), 15.0 * 998.0 + 30.0);
    }

    #[test]
    fn hit_testing() {
        let g = SheetGeometry::with_metrics(&sheet(), SheetMetrics::default());
        assert_eq!(g.hit_test(0.0, 0.0), CellRef::new(0, 0));
        assert_eq!(g.hit_test(47.9, 14.9), CellRef::new(0, 0));
        assert_eq!(g.hit_test(48.0, 0.0), CellRef::new(0, 1));
        // Inside the B2:C3 merge → anchor B2.
        assert_eq!(g.hit_test(200.0, 50.0), CellRef::new(1, 1));
        // Past the hidden column E (x 306..354 is F).
        assert_eq!(g.hit_test(310.0, 0.0), CellRef::new(0, 5));
        // Past the hidden row 4: y 60..75 is row 5 (index 4).
        assert_eq!(g.hit_test(0.0, 61.0), CellRef::new(4, 0));
        let r = g.cell_rect(2, 2);
        assert_eq!((r.x, r.y, r.w, r.h), (48.0, 15.0, 210.0, 45.0));
        // Far away cells.
        assert_eq!(
            g.hit_test(48.0 * 100.0 + 200.0, 15.0 * 50_000.0 - 1.0).row,
            49_999
        );
    }

    #[test]
    fn column_letters_reexported() {
        assert_eq!(col_to_letters(27), "AB");
        assert_eq!(letters_to_col("AB"), Some(27));
    }
}
