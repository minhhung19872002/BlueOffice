//! BlueSheet print jobs (CORE-PRINT-001): the sheets to print (active ones,
//! the whole workbook or the selected cells), the panel's overrides of the
//! page setup (gridlines and headings, §18.3.1.70 `printOptions`; print titles
//! and print area, §18.2.6 `_xlnm.Print_Titles` / `_xlnm.Print_Area`), then the
//! shared pipeline of [`crate::print`]. The pages themselves come from the page
//! setup pagination of `sheet::print` (scaling, fit to N × M pages, order).
//!
//! Overrides apply for the job only: the sheet's page setup is put back
//! afterwards (the model is not changed and nothing is added to undo).

use super::{EngineError, Result, SheetEngine};
use crate::print::{choose, finish, nothing, with_copies, Logical, PrintOptions, SheetScope};
use bo_model::sheet::{col_to_letters, letters_to_col};
use bo_render::{DisplayList, PageList};

/// Page setup values a job may override.
struct Saved {
    print_area: Option<String>,
    print_titles: Option<String>,
    gridlines: bool,
    headings: bool,
}

/// `1:2` / `$1:$2` / `3` → 0-based inclusive rows.
fn rows_of(text: &str) -> Option<(u32, u32)> {
    let t = text.replace('$', "");
    let (a, b) = t.trim().split_once(':').unwrap_or((t.trim(), t.trim()));
    let (a, b): (u32, u32) = (a.trim().parse().ok()?, b.trim().parse().ok()?);
    (a >= 1 && b >= a).then(|| (a - 1, b - 1))
}

/// `A:B` / `$A:$B` / `C` → 0-based inclusive columns.
fn cols_of(text: &str) -> Option<(u32, u32)> {
    let t = text.replace('$', "").to_ascii_uppercase();
    let (a, b) = t.trim().split_once(':').unwrap_or((t.trim(), t.trim()));
    let (a, b) = (letters_to_col(a.trim())?, letters_to_col(b.trim())?);
    (b >= a).then_some((a, b))
}

/// `_xlnm.Print_Titles` text with the job's title rows / columns.
fn titles(current: Option<&str>, rows: Option<&str>, cols: Option<&str>) -> Option<String> {
    let (mut r, mut c) = current.map_or((None, None), super::print::parse_titles);
    if let Some(t) = rows {
        r = rows_of(t);
    }
    if let Some(t) = cols {
        c = cols_of(t);
    }
    let mut parts = Vec::new();
    if let Some((a, b)) = r {
        parts.push(format!("${}:${}", a + 1, b + 1));
    }
    if let Some((a, b)) = c {
        parts.push(format!("${}:${}", col_to_letters(a), col_to_letters(b)));
    }
    (!parts.is_empty()).then(|| parts.join(","))
}

impl SheetEngine {
    /// Sets the job's overrides on `sheet`; returns what to put back.
    fn override_setup(&mut self, sheet: usize, o: &PrintOptions) -> Result<Saved> {
        let wb = self.wb.as_mut().ok_or(EngineError::NoDocument)?;
        let ws = wb
            .sheets
            .get_mut(sheet)
            .ok_or(EngineError::PageOutOfRange(sheet))?;
        let saved = Saved {
            print_area: ws.print_area.clone(),
            print_titles: ws.print_titles.clone(),
            gridlines: ws.page_setup.print_grid_lines,
            headings: ws.page_setup.print_headings,
        };
        let s = &o.sheet;
        if s.scope == SheetScope::Selection {
            ws.print_area = s.selection.clone().filter(|v| !v.trim().is_empty());
        } else if s.ignore_print_area {
            ws.print_area = None;
        }
        if s.title_rows.is_some() || s.title_cols.is_some() {
            ws.print_titles = titles(
                ws.print_titles.as_deref(),
                s.title_rows.as_deref(),
                s.title_cols.as_deref(),
            );
        }
        if let Some(g) = s.gridlines {
            ws.page_setup.print_grid_lines = g;
        }
        if let Some(h) = s.headings {
            ws.page_setup.print_headings = h;
        }
        Ok(saved)
    }

    fn restore_setup(&mut self, sheet: usize, saved: Saved) {
        if let Some(ws) = self.wb.as_mut().and_then(|w| w.sheets.get_mut(sheet)) {
            ws.print_area = saved.print_area;
            ws.print_titles = saved.print_titles;
            ws.page_setup.print_grid_lines = saved.gridlines;
            ws.page_setup.print_headings = saved.headings;
        }
    }

    /// Sheets of the job in workbook order.
    fn job_sheets(&self, o: &PrintOptions) -> Result<Vec<usize>> {
        let info = self.info()?;
        let visible = |i: &usize| info.sheets.get(*i).is_some_and(|s| !s.hidden);
        Ok(match o.sheet.scope {
            SheetScope::Workbook => (0..info.sheets.len()).filter(visible).collect(),
            SheetScope::Active | SheetScope::Selection => {
                let mut v: Vec<usize> = o
                    .sheet
                    .sheets
                    .iter()
                    .copied()
                    .filter(|&i| i < info.sheets.len())
                    .collect();
                if v.is_empty() {
                    v.push(0);
                }
                if o.sheet.scope == SheetScope::Selection {
                    v.truncate(1);
                }
                v.sort_unstable();
                v.dedup();
                v
            }
        })
    }

    /// Printed pages of the job before the page choice.
    fn job_pages(&mut self, o: &PrintOptions) -> Result<Vec<PageList>> {
        let mut pages = Vec::new();
        for sheet in self.job_sheets(o)? {
            let saved = self.override_setup(sheet, o)?;
            let r = self.print_pages(sheet);
            self.restore_setup(sheet, saved);
            pages.extend(r?);
        }
        Ok(pages)
    }

    /// The sheets of paper of one copy of a print job.
    pub fn print_sheets(&mut self, o: &PrintOptions) -> Result<Vec<PageList>> {
        let pages = self.job_pages(o)?;
        let chosen = choose(o, pages.len(), None)?;
        let logical: Vec<Logical> =
            if chosen.len() == pages.len() && chosen.iter().enumerate().all(|(k, &i)| k == i) {
                (1..).zip(pages).collect()
            } else {
                chosen
                    .into_iter()
                    .filter_map(|i| pages.get(i).map(|p| (i + 1, p.clone())))
                    .collect()
            };
        Ok(finish(logical, o, o.frame))
    }

    /// PDF of a print job.
    pub fn print_pdf(&mut self, o: &PrintOptions) -> Result<Vec<u8>> {
        let sheets = with_copies(self.print_sheets(o)?, o);
        if sheets.is_empty() {
            return Err(nothing());
        }
        bo_pdf::write_pdf(
            &DisplayList { pages: sheets },
            &self.fonts,
            &bo_pdf::PdfMeta {
                title: Some(self.name.clone()),
            },
        )
        .map_err(EngineError::from)
    }

    /// JSON variant of [`SheetEngine::print_pdf`].
    pub fn print_pdf_json(&mut self, json: &str) -> Result<Vec<u8>> {
        self.print_pdf(&PrintOptions::from_json(json)?)
    }

    /// Print preview of sheet `index` of the job in `json`
    /// (`{"sheets","copies","index","page"}`).
    pub fn print_preview_json(&mut self, json: &str, index: usize) -> Result<String> {
        let mut cache = std::mem::take(&mut self.print_cache);
        let r = cache.preview(json, index, |o| self.print_sheets(o));
        if r.is_err() {
            cache.clear();
        }
        self.print_cache = cache;
        r
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn title_texts() {
        assert_eq!(rows_of("$1:$2"), Some((0, 1)));
        assert_eq!(rows_of("3"), Some((2, 2)));
        assert_eq!(rows_of("2:1"), None);
        assert_eq!(cols_of("a:c"), Some((0, 2)));
        assert_eq!(
            titles(None, Some("1:2"), Some("A")),
            Some("$1:$2,$A:$A".into())
        );
        // Columns kept from the sheet, rows removed.
        assert_eq!(
            titles(Some("Sheet1!$1:$1,Sheet1!$B:$C"), Some(""), None),
            Some("$B:$C".into())
        );
        assert_eq!(titles(Some("$1:$1"), Some(""), None), None);
    }
}
