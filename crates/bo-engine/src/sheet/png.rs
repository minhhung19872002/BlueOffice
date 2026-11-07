//! Printed pages of a workbook as PNG (visual regression, thumbnails).

use super::SheetEngine;
use crate::{EngineError, Result};

impl SheetEngine {
    /// Printed pages of every visible sheet, in order, as display lists.
    fn all_print_pages(&mut self) -> Result<Vec<bo_render::PageList>> {
        let n = self.info()?.sheets.len();
        let mut pages = Vec::new();
        for sheet in 0..n {
            if self.info()?.sheets.get(sheet).is_some_and(|s| s.hidden) {
                continue;
            }
            pages.extend(self.print_pages(sheet)?);
        }
        Ok(pages)
    }

    /// Number of printed pages (all visible sheets).
    pub fn print_page_count(&mut self) -> Result<usize> {
        Ok(self.all_print_pages()?.len())
    }

    /// Printed page `index` (0-based, across visible sheets) rendered to PNG.
    pub fn render_print_png(&mut self, index: usize, scale: f32) -> Result<Vec<u8>> {
        let pages = self.all_print_pages()?;
        let page = pages.get(index).ok_or(EngineError::PageOutOfRange(index))?;
        Ok(bo_raster::render_page_png(page, &self.fonts, scale)?)
    }
}

#[cfg(test)]
mod tests {
    use super::SheetEngine;

    #[test]
    fn renders_printed_pages() {
        let mut e = SheetEngine::with_bundled_fonts();
        e.new_workbook("a.xlsx").unwrap();
        e.set_input(0, 0, 0, "Xin chào").unwrap();
        assert_eq!(e.print_page_count().unwrap(), 1);
        assert!(e.render_print_png(0, 0.5).unwrap().starts_with(b"\x89PNG"));
        assert!(e.render_print_png(5, 0.5).is_err());
    }
}
