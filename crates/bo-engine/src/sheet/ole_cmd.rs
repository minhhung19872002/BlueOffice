//! Workbooks embedded in or linked from BlueDoc and BlueSlide (CORE-EMBED-001): the
//! area of the workbook an object shows, its picture drawn by the BlueSheet renderer,
//! and the cell inputs of that area for the object's editing grid.
//!
//! The shown area is the print area of the first sheet that has one (defined name
//! `_xlnm.Print_Area`, ECMA-376 Part 1, §18.2.6), else the used cells of the first
//! visible sheet — our own convention, so that an embedded range keeps its extent in
//! the workbook itself and a re-embedded or updated object shows the same cells.

use super::SheetEngine;
use crate::{EngineError, Result};
use bo_model::sheet::{CellRange, CellRef, SheetState};
use bo_sheet::render::{render_viewport, RenderOptions};
use bo_sheet::viewport::Viewport;
use bo_text::FontManager;

/// Largest area an object shows (rows × columns).
pub const MAX_ROWS: u32 = 200;
/// See [`MAX_ROWS`].
pub const MAX_COLS: u32 = 50;
/// Area of an empty workbook (3 rows × 4 columns).
const EMPTY_AREA: (u32, u32) = (3, 4);

/// Picture of the shown area.
#[derive(Debug, Clone, PartialEq)]
pub struct RangePicture {
    /// PNG.
    pub png: Vec<u8>,
    /// Size of the area in points.
    pub width: f64,
    /// Size of the area in points.
    pub height: f64,
    /// Sheet index.
    pub sheet: usize,
    /// The area.
    pub range: CellRange,
}

fn clamp(r: CellRange) -> CellRange {
    let last = CellRef::new(
        r.last.row.min(r.first.row + MAX_ROWS - 1),
        r.last.col.min(r.first.col + MAX_COLS - 1),
    );
    CellRange::new(r.first, last)
}

/// `'Sheet name'!$A$1:$C$3` for a defined name (§18.17 formula references).
fn sheet_ref(sheet: &str, r: CellRange) -> String {
    let abs = |c: CellRef| {
        let a1 = c.to_string();
        let split = a1.find(|ch: char| ch.is_ascii_digit()).unwrap_or(a1.len());
        format!("${}${}", &a1[..split], &a1[split..])
    };
    format!(
        "'{}'!{}:{}",
        sheet.replace('\'', "''"),
        abs(r.first),
        abs(r.last)
    )
}

impl SheetEngine {
    /// Run `f` on a fresh engine that borrows `fonts` (the document engine's), so the
    /// picture is drawn with the same faces; the fonts are given back afterwards.
    pub(crate) fn with_fonts<T>(
        fonts: &mut FontManager,
        f: impl FnOnce(&mut SheetEngine) -> T,
    ) -> T {
        let mut e = SheetEngine::new();
        std::mem::swap(&mut e.fonts, fonts);
        let out = f(&mut e);
        std::mem::swap(&mut e.fonts, fonts);
        out
    }

    /// The area an object shows (see the module documentation).
    pub fn embed_area(&self) -> Result<(usize, CellRange)> {
        let wb = self.wb()?;
        for (i, ws) in wb.sheets.iter().enumerate() {
            let Some(pa) = &ws.print_area else { continue };
            let first = pa.split(',').next().unwrap_or_default();
            if let Some((s, r)) = bo_sheet::cells::resolve_ref(wb, i, first.trim()) {
                return Ok((s, clamp(r)));
            }
        }
        let i = wb
            .sheets
            .iter()
            .position(|s| matches!(s.state, SheetState::Visible))
            .ok_or(EngineError::NoDocument)?;
        let r = wb.sheets[i].used_range().unwrap_or_else(|| {
            CellRange::new(
                CellRef::new(0, 0),
                CellRef::new(EMPTY_AREA.0 - 1, EMPTY_AREA.1 - 1),
            )
        });
        // From A1 so the object shows the table as laid out on the sheet.
        Ok((i, clamp(CellRange::new(CellRef::new(0, 0), r.last))))
    }

    /// Make `range` (`Sheet1!A1:C3`, or `A1:C3` on the first sheet) the shown area: it
    /// becomes the sheet's print area and the other sheets' print areas go.
    pub fn set_embed_area(&mut self, range: &str) -> Result<(usize, CellRange)> {
        let wb = self.wb()?;
        let (sheet, r) = bo_sheet::cells::resolve_ref(wb, 0, range.trim())
            .ok_or_else(|| EngineError::BadCommand("badRange".into()))?;
        let r = clamp(r);
        let wb = self.wb.as_mut().ok_or(EngineError::NoDocument)?;
        for (i, ws) in wb.sheets.iter_mut().enumerate() {
            ws.print_area = (i == sheet).then(|| sheet_ref(&ws.name, r));
        }
        Ok((sheet, r))
    }

    /// Picture of `range` on `sheet` drawn by the BlueSheet renderer (no headers,
    /// frozen panes ignored), `scale` pixels per point.
    pub fn range_picture(
        &mut self,
        sheet: usize,
        range: CellRange,
        scale: f32,
    ) -> Result<RangePicture> {
        self.geometry(sheet)?;
        let wb = self.wb.as_ref().ok_or(EngineError::NoDocument)?;
        let geom = self
            .geometry
            .get(sheet)
            .and_then(Option::as_ref)
            .ok_or(EngineError::NoDocument)?;
        let x0 = geom.col_x(range.first.col);
        let y0 = geom.row_y(range.first.row);
        let w = (geom.col_x(range.last.col + 1) - x0).max(1.0);
        let h = (geom.row_y(range.last.row + 1) - y0).max(1.0);
        let vp = Viewport {
            scroll_x: x0,
            scroll_y: y0,
            width: w,
            height: h,
            header_width: 0.0,
            header_height: 0.0,
        };
        let opts = RenderOptions {
            locale: self.locale,
            headers: Some(false),
            ignore_frozen: true,
            ..RenderOptions::default()
        };
        let page = render_viewport(wb, sheet, geom, &vp, &opts, &mut self.fonts);
        let png = bo_raster::render_page_png(&page, &self.fonts, scale)?;
        Ok(RangePicture {
            png,
            width: w,
            height: h,
            sheet,
            range,
        })
    }

    /// Size of `range` on `sheet` in points.
    pub fn area_size(&mut self, sheet: usize, range: CellRange) -> Result<(f64, f64)> {
        let g = self.geometry(sheet)?;
        Ok((
            g.col_x(range.last.col + 1) - g.col_x(range.first.col),
            g.row_y(range.last.row + 1) - g.row_y(range.first.row),
        ))
    }

    /// Formula-bar inputs of `range` on `sheet`, row by row.
    pub fn range_inputs(&self, sheet: usize, range: CellRange) -> Result<Vec<Vec<String>>> {
        let mut rows = Vec::new();
        for r in range.first.row..=range.last.row {
            let mut row = Vec::new();
            for c in range.first.col..=range.last.col {
                row.push(self.cell(sheet, r, c)?.input);
            }
            rows.push(row);
        }
        Ok(rows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shown_area_follows_print_area_else_used_cells() {
        let mut e = SheetEngine::with_bundled_fonts();
        e.new_workbook("a.xlsx").unwrap();
        // Empty sheet: A1:D3.
        let (_, r) = e.embed_area().unwrap();
        assert_eq!(r.to_string(), "A1:D3");
        e.set_input(0, 4, 1, "x").unwrap();
        let (_, r) = e.embed_area().unwrap();
        assert_eq!(r.to_string(), "A1:B5");
        let (s, r) = e.set_embed_area("B2:C3").unwrap();
        assert_eq!((s, r.to_string()), (0, "B2:C3".to_owned()));
        assert_eq!(
            e.workbook().unwrap().sheets[0].print_area.as_deref(),
            Some("'Sheet1'!$B$2:$C$3")
        );
        assert_eq!(e.embed_area().unwrap().1.to_string(), "B2:C3");
        assert!(e.set_embed_area("nonsense!!").is_err());
    }

    #[test]
    fn picture_has_the_area_size() {
        let mut e = SheetEngine::with_bundled_fonts();
        e.new_workbook("a.xlsx").unwrap();
        e.set_input(0, 0, 0, "1").unwrap();
        e.set_input(0, 0, 1, "=A1*2").unwrap();
        let r = CellRange::parse("A1:B2").unwrap();
        let p = e.range_picture(0, r, 1.0).unwrap();
        assert!(p.png.starts_with(b"\x89PNG"));
        let g = e.geometry(0).unwrap();
        let (w, h) = (g.col_x(2) - g.col_x(0), g.row_y(2) - g.row_y(0));
        assert!((p.width - w).abs() < 1e-9 && (p.height - h).abs() < 1e-9);
        assert_eq!(
            e.range_inputs(0, r).unwrap(),
            vec![
                vec!["1".to_owned(), "=A1*2".to_owned()],
                vec![String::new(), String::new()]
            ]
        );
    }
}
