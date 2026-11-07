//! PDF export of worksheets honouring the page setup (ECMA-376 Part 1,
//! §18.3.1.63 `pageSetup`: paper size, orientation, scale, fit-to-page;
//! §18.3.1.62 `pageMargins`; §18.3.1.70 `printOptions`; print area and print
//! titles from the `_xlnm.Print_Area` / `_xlnm.Print_Titles` defined names,
//! §18.2.6; manual breaks §18.3.1.74 / §18.3.1.14).
//!
//! Pages are ordered down, then over. Print titles (rows and/or columns) are
//! repeated on every page that does not already show them.

use super::{EngineError, Result, SheetEngine, ViewRequest};
use bo_model::sheet::{letters_to_col, CellRange, CellRef, PageMargins, Worksheet};
use bo_render::{Command, DisplayList, PageList};
use bo_sheet::render::{render_viewport, RenderOptions};
use bo_sheet::viewport::Viewport;

/// Points per inch.
const PT_PER_IN: f64 = 72.0;

/// Paper size in points for `paperSize` (§18.3.1.63; default A4).
fn paper(size: Option<u32>) -> (f64, f64) {
    match size {
        Some(1) => (612.0, 792.0),  // Letter
        Some(5) => (612.0, 1008.0), // Legal
        Some(8) => (842.0, 1191.0), // A3
        Some(11) => (420.0, 595.0), // A5
        Some(13) => (516.0, 729.0), // B5 (JIS)
        _ => (595.0, 842.0),        // A4
    }
}

/// Row span `(first, last)` and column span of `_xlnm.Print_Titles`
/// (`Sheet1!$1:$2,Sheet1!$A:$A`).
/// An optional inclusive span of rows or columns.
pub(super) type Span = Option<(u32, u32)>;

/// Row span `(first, last)` and column span of `_xlnm.Print_Titles`.
pub(super) fn parse_titles(text: &str) -> (Span, Span) {
    let mut rows = None;
    let mut cols = None;
    for part in text.split(',') {
        let addr = part
            .rsplit_once('!')
            .map_or(part, |(_, a)| a)
            .replace('$', "");
        let Some((a, b)) = addr.trim().split_once(':') else {
            continue;
        };
        if let (Ok(x), Ok(y)) = (a.parse::<u32>(), b.parse::<u32>()) {
            if x >= 1 && y >= x {
                rows = Some((x - 1, y - 1));
            }
        } else if let (Some(x), Some(y)) = (letters_to_col(a), letters_to_col(b)) {
            if y >= x {
                cols = Some((x, y));
            }
        }
    }
    (rows, cols)
}

/// Area printed for a sheet: the print area, else used cells and charts.
fn print_area(wb: &bo_model::sheet::Workbook, sheet: usize, ws: &Worksheet) -> Option<CellRange> {
    if let Some(pa) = &ws.print_area {
        let first = pa.split(',').next().unwrap_or(pa);
        if let Some((_, r)) = bo_sheet::cells::resolve_ref(wb, sheet, first) {
            return Some(r);
        }
    }
    let mut r = ws.used_range();
    for c in &ws.charts {
        let to =
            c.to.unwrap_or(CellRef::new(c.from.row + 15, c.from.col + 8));
        let cr = CellRange::new(
            c.from,
            CellRef::new(to.row.saturating_sub(1), to.col.saturating_sub(1)),
        );
        r = Some(match r {
            Some(u) => CellRange::new(
                CellRef::new(u.first.row.min(cr.first.row), u.first.col.min(cr.first.col)),
                CellRef::new(u.last.row.max(cr.last.row), u.last.col.max(cr.last.col)),
            ),
            None => cr,
        });
    }
    r
}

/// Splits `first..=last` into page spans no wider than `avail` (at least
/// one entry each), honouring manual breaks (break *after* the index).
fn spans(
    sizes: &dyn Fn(u32) -> f64,
    first: u32,
    last: u32,
    avail: f64,
    breaks: &[u32],
) -> Vec<(u32, u32)> {
    let mut out = Vec::new();
    let mut start = first;
    let mut acc = 0.0;
    let mut i = first;
    while i <= last {
        let s = sizes(i);
        if acc > 0.0 && acc + s > avail + 0.01 {
            out.push((start, i - 1));
            start = i;
            acc = 0.0;
        }
        acc += s;
        if breaks.contains(&i) && i < last {
            out.push((start, i));
            start = i + 1;
            acc = 0.0;
        }
        i += 1;
    }
    if start <= last {
        out.push((start, last));
    }
    out
}

impl SheetEngine {
    fn render_band(
        &mut self,
        sheet: usize,
        (sx, sy, w, h): (f64, f64, f64, f64),
        opts: &RenderOptions,
    ) -> Result<PageList> {
        self.geometry(sheet)?;
        let wb = self.wb.as_ref().ok_or(EngineError::NoDocument)?;
        let geom = self
            .geometry
            .get(sheet)
            .and_then(Option::as_ref)
            .ok_or(EngineError::NoDocument)?;
        let req = ViewRequest {
            sheet,
            scroll_x: sx,
            scroll_y: sy,
            width: w,
            height: h,
        };
        let mut vp: Viewport = req.viewport();
        vp.header_width = 0.0;
        vp.header_height = 0.0;
        Ok(render_viewport(wb, sheet, geom, &vp, opts, &mut self.fonts))
    }

    /// The printed pages of one sheet (empty when there is nothing to print).
    #[allow(clippy::too_many_lines)]
    pub(crate) fn print_pages(&mut self, sheet: usize) -> Result<Vec<PageList>> {
        let wb = self.wb()?;
        let ws = self.sheet(sheet)?;
        let Some(area) = print_area(wb, sheet, ws) else {
            return Ok(Vec::new());
        };
        let ps = ws.page_setup.clone();
        let fit = ws.props.fit_to_page == Some(true);
        let (titles_rows, titles_cols) = ws
            .print_titles
            .as_deref()
            .map_or((None, None), parse_titles);
        let (mut pw, mut ph) = paper(ps.paper_size);
        if ps.orientation.as_deref() == Some("landscape") {
            std::mem::swap(&mut pw, &mut ph);
        }
        let m = ps.margins.unwrap_or_default();
        let PageMargins {
            left,
            right,
            top,
            bottom,
            ..
        } = m;
        let (ml, mr, mt, mb) = (
            left * PT_PER_IN,
            right * PT_PER_IN,
            top * PT_PER_IN,
            bottom * PT_PER_IN,
        );
        let opts = RenderOptions {
            locale: self.locale,
            now: self.live.now(),
            gridlines: Some(ps.print_grid_lines),
            headers: Some(false),
            ignore_frozen: true,
            ..RenderOptions::default()
        };
        let g = self.geometry(sheet)?.clone();
        let width = |a: u32, b: u32| g.col_x(b + 1) - g.col_x(a);
        let height = |a: u32, b: u32| g.row_y(b + 1) - g.row_y(a);
        let tr_h = titles_rows.map_or(0.0, |(a, b)| height(a, b));
        let tc_w = titles_cols.map_or(0.0, |(a, b)| width(a, b));
        let content_w = width(area.first.col, area.last.col);
        let content_h = height(area.first.row, area.last.row);
        let (aw0, ah0) = ((pw - ml - mr).max(36.0), (ph - mt - mb).max(36.0));
        // §18.3.1.63 scale (10–400 %) or fitToWidth / fitToHeight pages (0 = no limit).
        let scale = if fit {
            let fw = ps.fit_to_width.unwrap_or(1);
            let fh = ps.fit_to_height.unwrap_or(1);
            let mut s: f64 = 1.0;
            if fw > 0 && content_w > 0.0 {
                s = s.min(aw0 * f64::from(fw) / (content_w + tc_w * f64::from(fw)));
            }
            if fh > 0 && content_h > 0.0 {
                s = s.min(ah0 * f64::from(fh) / (content_h + tr_h * f64::from(fh)));
            }
            s.clamp(0.1, 1.0)
        } else {
            f64::from(ps.scale.unwrap_or(100).clamp(10, 400)) / 100.0
        };
        let aw = (aw0 / scale - tc_w).max(10.0);
        let ah = (ah0 / scale - tr_h).max(10.0);
        let col_pages = spans(
            &|c| g.col_width(c),
            area.first.col,
            area.last.col,
            aw,
            &ps.col_breaks,
        );
        let row_pages = spans(
            &|r| g.row_height(r),
            area.first.row,
            area.last.row,
            ah,
            &ps.row_breaks,
        );
        let f = |v: f64| v as f32;
        let mut pages = Vec::new();
        for &(c0, c1) in &col_pages {
            for &(r0, r1) in &row_pages {
                let bw = width(c0, c1);
                let bh = height(r0, r1);
                // Titles already on the page are not repeated.
                let rows_band = titles_rows.filter(|(_, b)| r0 > *b);
                let cols_band = titles_cols.filter(|(_, b)| c0 > *b);
                let th = rows_band.map_or(0.0, |(a, b)| height(a, b));
                let tw = cols_band.map_or(0.0, |(a, b)| width(a, b));
                let mut x0 = ml;
                let mut y0 = mt;
                if ps.horizontal_centered {
                    x0 += ((aw0 - (bw + tw) * scale) / 2.0).max(0.0);
                }
                if ps.vertical_centered {
                    y0 += ((ah0 - (bh + th) * scale) / 2.0).max(0.0);
                }
                let mut commands = Vec::new();
                let mut place = |eng: &mut SheetEngine,
                                 (sx, sy, w, h): (f64, f64, f64, f64),
                                 dx: f64,
                                 dy: f64|
                 -> Result<()> {
                    if w <= 0.0 || h <= 0.0 {
                        return Ok(());
                    }
                    let mut band = eng.render_band(sheet, (sx, sy, w, h), &opts)?;
                    commands.push(Command::PushClip {
                        x: f(dx),
                        y: f(dy),
                        w: f(w * scale),
                        h: f(h * scale),
                    });
                    commands.push(Command::PushTransform {
                        m: [f(scale), 0.0, 0.0, f(scale), f(dx), f(dy)],
                    });
                    commands.append(&mut band.commands);
                    commands.push(Command::PopTransform);
                    commands.push(Command::PopClip);
                    Ok(())
                };
                let bx = x0 + tw * scale;
                let by = y0 + th * scale;
                place(self, (g.col_x(c0), g.row_y(r0), bw, bh), bx, by)?;
                if let Some((a, b)) = rows_band {
                    place(self, (g.col_x(c0), g.row_y(a), bw, height(a, b)), bx, y0)?;
                }
                if let Some((a, b)) = cols_band {
                    place(self, (g.col_x(a), g.row_y(r0), width(a, b), bh), x0, by)?;
                }
                if let (Some((ra, rb)), Some((ca, cb))) = (rows_band, cols_band) {
                    place(
                        self,
                        (g.col_x(ca), g.row_y(ra), width(ca, cb), height(ra, rb)),
                        x0,
                        y0,
                    )?;
                }
                pages.push(PageList {
                    width: f(pw),
                    height: f(ph),
                    commands,
                });
            }
        }
        Ok(pages)
    }

    /// PDF of every visible sheet laid out by its page setup.
    pub fn export_pdf(&mut self) -> Result<Vec<u8>> {
        let n = self.wb()?.sheets.len();
        let mut pages = Vec::new();
        for sheet in 0..n {
            if self.info()?.sheets.get(sheet).is_some_and(|s| s.hidden) {
                continue;
            }
            pages.extend(self.print_pages(sheet)?);
        }
        if pages.is_empty() {
            pages.push(PageList {
                width: 595.0,
                height: 842.0,
                commands: Vec::new(),
            });
        }
        bo_pdf::write_pdf(
            &DisplayList { pages },
            &self.fonts,
            &bo_pdf::PdfMeta {
                title: Some(self.name.clone()),
            },
        )
        .map_err(EngineError::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titles_and_spans() {
        assert_eq!(parse_titles("Sheet1!$1:$2"), (Some((0, 1)), None));
        assert_eq!(
            parse_titles("'Bảng 1'!$A:$B,'Bảng 1'!$3:$3"),
            (Some((2, 2)), Some((0, 1)))
        );
        let s = spans(&|_| 10.0, 0, 9, 35.0, &[]);
        assert_eq!(s, vec![(0, 2), (3, 5), (6, 8), (9, 9)]);
        let s = spans(&|_| 10.0, 0, 9, 1000.0, &[4]);
        assert_eq!(s, vec![(0, 4), (5, 9)]);
        assert_eq!(paper(Some(1)), (612.0, 792.0));
    }
}
