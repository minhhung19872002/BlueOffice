//! Printing and PDF export of worksheets honouring the page setup
//! (ECMA-376 Part 1, §18.3.1.63 `pageSetup`: paper size, orientation, scale,
//! fitToWidth / fitToHeight with §18.3.1.65 `pageSetUpPr/@fitToPage`,
//! pageOrder, firstPageNumber / useFirstPageNumber, blackAndWhite, draft,
//! cellComments, errors; §18.3.1.62 `pageMargins`; §18.3.1.70 `printOptions`
//! (gridlines, headings, centring); §18.3.1.46 `headerFooter`; print areas
//! and print titles from the `_xlnm.Print_Area` / `_xlnm.Print_Titles`
//! defined names, §18.2.6; manual breaks §18.3.1.74 / §18.3.1.14).
//!
//! The same pagination drives the PDF export and the print preview
//! (`printPreview` command). Choices of ours where the spec is silent:
//! * each print area starts on a new page and is paginated on its own;
//! * page numbers (`&P`) and the page count (`&N`) are counted per sheet;
//! * even / first page headers follow the physical page position (1 = odd);
//! * row / column headings are 14 pt high and `7 × digits + 8` pt wide, 9 pt text;
//! * black and white: fills become white, text and lines black, shape fills grey;
//! * draft: charts, pictures and SmartArt are left out;
//! * comments "at end": one line per comment `A1: author: text` on pages after
//!   the sheet; comments "as displayed" print nothing extra (comment
//!   visibility is not part of the model).

use super::{EngineError, Result, SheetEngine, ViewRequest};
use bo_model::sheet::{letters_to_col, CellRange, CellRef, RawAttr, Workbook, Worksheet};
use bo_render::{Command, DisplayList, PageList, Paint, Rgba};
use bo_sheet::headerfooter::{self as hf, HfAlign, HfBase, HfContext, HfFormat};
use bo_sheet::render::{render_viewport, RenderOptions};
use bo_sheet::viewport::Viewport;

/// Points per inch.
const PT_PER_IN: f64 = 72.0;
/// Height of the printed column-heading band (points).
pub(super) const HEADING_H: f64 = 14.0;
/// Font size of printed headings and end-of-sheet comment lines.
const HEADING_SIZE: f64 = 9.0;
const COMMENT_SIZE: f64 = 10.0;

/// Paper size in points for `paperSize` (§18.3.1.63; default A4).
pub(super) fn paper(size: Option<u32>) -> (f64, f64) {
    match size {
        Some(1) => (612.0, 792.0),  // Letter
        Some(5) => (612.0, 1008.0), // Legal
        Some(8) => (842.0, 1191.0), // A3
        Some(11) => (420.0, 595.0), // A5
        Some(13) => (516.0, 729.0), // B5 (JIS)
        _ => (595.0, 842.0),        // A4
    }
}

/// An optional inclusive span of rows or columns.
pub(super) type Span = Option<(u32, u32)>;

/// Row span `(first, last)` and column span of `_xlnm.Print_Titles`
/// (`Sheet1!$1:$2,Sheet1!$A:$A`).
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

/// `_xlnm.Print_Titles` text for title rows and / or columns (`None` when both are absent).
pub(super) fn titles_text(quoted_sheet: &str, rows: Span, cols: Span) -> Option<String> {
    let mut parts = Vec::new();
    if let Some((a, b)) = rows {
        parts.push(format!("{quoted_sheet}!${}:${}", a + 1, b + 1));
    }
    if let Some((a, b)) = cols {
        parts.push(format!(
            "{quoted_sheet}!${}:${}",
            bo_model::sheet::col_to_letters(a),
            bo_model::sheet::col_to_letters(b)
        ));
    }
    (!parts.is_empty()).then(|| parts.join(","))
}

/// Value of a preserved `pageSetup` attribute.
pub(super) fn attr<'a>(attrs: &'a [RawAttr], name: &str) -> Option<&'a str> {
    attrs
        .iter()
        .find(|(k, _)| k == name)
        .map(|(_, v)| v.as_str())
}

/// Sets (or removes with `None`) a preserved attribute, keeping its position.
pub(super) fn set_attr(attrs: &mut Vec<RawAttr>, name: &str, value: Option<String>) {
    match (attrs.iter().position(|(k, _)| k == name), value) {
        (Some(i), Some(v)) => attrs[i].1 = v,
        (Some(i), None) => {
            attrs.remove(i);
        }
        (None, Some(v)) => attrs.push((name.to_owned(), v)),
        (None, None) => {}
    }
}

fn attr_bool(attrs: &[RawAttr], name: &str) -> bool {
    matches!(attr(attrs, name), Some("1" | "true"))
}

/// `pageSetup/@cellComments` (§18.18 ST_CellComments).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CommentsMode {
    None,
    AtEnd,
    AsDisplayed,
}

/// Print settings of a sheet resolved from the model.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Settings {
    /// Paper width and height in points (after orientation).
    pub page: (f64, f64),
    /// Left, right, top, bottom, header, footer margins in points.
    pub margins: [f64; 6],
    /// Fit to `(wide, tall)` pages (0 = no limit), when `fitToPage` is on.
    pub fit: Option<(u32, u32)>,
    /// Scale percent (10–400).
    pub scale: u32,
    /// `pageOrder="overThenDown"`.
    pub over_then_down: bool,
    pub headings: bool,
    pub gridlines: bool,
    pub center_h: bool,
    pub center_v: bool,
    /// Number of the first page.
    pub first_page: u32,
    pub black_white: bool,
    pub draft: bool,
    /// Error replacement text (`None` = as displayed).
    pub errors: Option<String>,
    pub comments: CommentsMode,
}

impl Settings {
    /// Printable width and height of the paper inside the margins (points).
    pub(super) fn avail(&self) -> (f64, f64) {
        let [ml, mr, mt, mb, ..] = self.margins;
        let (pw, ph) = self.page;
        ((pw - ml - mr).max(36.0), (ph - mt - mb).max(36.0))
    }
}

/// Where the printed block of one page goes (see `SheetEngine::frame_box`).
struct FrameBox {
    /// Page position of the block (margins and centring applied).
    x0: f64,
    y0: f64,
    /// Heading column width / heading row height (unscaled).
    hw: f64,
    hh: f64,
    /// Repeated title columns width / title rows height (unscaled).
    tw: f64,
    th: f64,
    rows_band: Span,
    cols_band: Span,
    /// Body width / height (unscaled).
    bw: f64,
    bh: f64,
}

/// Cells of one printed page (page layout view).
#[derive(Debug, Clone, PartialEq)]
pub(super) struct PageCells {
    pub cols: (u32, u32),
    pub rows: (u32, u32),
    pub scale: f64,
    /// Page position (points) of the top-left corner of the first body cell.
    pub origin: (f64, f64),
}

/// One printed page (page layout view).
#[derive(Debug, Clone, PartialEq)]
pub(super) struct PageBox {
    /// Paper width and height in points (after orientation).
    pub size: (f64, f64),
    /// Left, right, top, bottom, header, footer margins in points.
    pub margins: [f64; 6],
    /// `None` for an end-of-sheet comments page.
    pub cells: Option<PageCells>,
}

pub(super) fn settings(ws: &Worksheet) -> Settings {
    let ps = &ws.page_setup;
    let (mut pw, mut ph) = paper(ps.paper_size);
    if ps.orientation.as_deref() == Some("landscape") {
        std::mem::swap(&mut pw, &mut ph);
    }
    let m = ps.margins.unwrap_or_default();
    let a = &ps.attrs;
    // §18.18 ST_PrintError: displayed / blank / dash / NA.
    let errors = match attr(a, "errors") {
        Some("blank") => Some(String::new()),
        Some("dash") => Some("--".to_owned()),
        Some("NA") => Some("#N/A".to_owned()),
        _ => None,
    };
    let comments = match attr(a, "cellComments") {
        Some("atEnd") => CommentsMode::AtEnd,
        Some("asDisplayed") => CommentsMode::AsDisplayed,
        _ => CommentsMode::None,
    };
    let first_page = if attr_bool(a, "useFirstPageNumber") {
        attr(a, "firstPageNumber")
            .and_then(|v| v.parse().ok())
            .unwrap_or(1)
    } else {
        1
    };
    Settings {
        page: (pw, ph),
        margins: [m.left, m.right, m.top, m.bottom, m.header, m.footer].map(|v| v * PT_PER_IN),
        fit: (ws.props.fit_to_page == Some(true))
            .then(|| (ps.fit_to_width.unwrap_or(1), ps.fit_to_height.unwrap_or(1))),
        scale: ps.scale.unwrap_or(100).clamp(10, 400),
        over_then_down: attr(a, "pageOrder") == Some("overThenDown"),
        headings: ps.print_headings,
        gridlines: ps.print_grid_lines,
        center_h: ps.horizontal_centered,
        center_v: ps.vertical_centered,
        first_page,
        black_white: attr_bool(a, "blackAndWhite"),
        draft: attr_bool(a, "draft"),
        errors,
        comments,
    }
}

/// Splits `first..=last` into page spans no wider than `avail` (at least
/// one entry each), honouring manual breaks (break *after* the index).
pub(super) fn spans(
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

/// Inputs of [`paginate`] (all lengths in unscaled sheet points).
pub(super) struct PageInput<'a> {
    pub col_w: &'a dyn Fn(u32) -> f64,
    pub row_h: &'a dyn Fn(u32) -> f64,
    pub area: CellRange,
    /// Printable width and height of the paper (inside the margins).
    pub avail: (f64, f64),
    /// Width / height repeated on every page (title columns / rows plus headings).
    pub fixed: (f64, f64),
    pub fit: Option<(u32, u32)>,
    pub scale_pct: u32,
    pub col_breaks: &'a [u32],
    pub row_breaks: &'a [u32],
    pub over_then_down: bool,
}

/// Pages of one print area.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Pagination {
    pub scale: f64,
    /// Column spans, left to right.
    pub cols: Vec<(u32, u32)>,
    /// Row spans, top to bottom.
    pub rows: Vec<(u32, u32)>,
    /// Pages in print order as `(column span index, row span index)`.
    pub order: Vec<(usize, usize)>,
    /// Width / height repeated on every page (from [`PageInput::fixed`]).
    pub fixed: (f64, f64),
}

/// Scale and page breaks of one print area. Fitting (§18.3.1.63) uses
/// `scale = min(wide·W / (content width + wide·fixed width), tall·H / (content
/// height + tall·fixed height))`, never above 100 % nor below 10 %.
pub(super) fn paginate(p: &PageInput<'_>) -> Pagination {
    let a = p.area;
    let content_w: f64 = (a.first.col..=a.last.col).map(p.col_w).sum();
    let content_h: f64 = (a.first.row..=a.last.row).map(p.row_h).sum();
    let (aw0, ah0) = p.avail;
    let scale = if let Some((fw, fh)) = p.fit {
        let mut s: f64 = 1.0;
        if fw > 0 && content_w > 0.0 {
            let n = f64::from(fw);
            s = s.min(aw0 * n / (content_w + p.fixed.0 * n));
        }
        if fh > 0 && content_h > 0.0 {
            let n = f64::from(fh);
            s = s.min(ah0 * n / (content_h + p.fixed.1 * n));
        }
        s.clamp(0.1, 1.0)
    } else {
        f64::from(p.scale_pct.clamp(10, 400)) / 100.0
    };
    let aw = (aw0 / scale - p.fixed.0).max(10.0);
    let ah = (ah0 / scale - p.fixed.1).max(10.0);
    let cols = spans(p.col_w, a.first.col, a.last.col, aw, p.col_breaks);
    let rows = spans(p.row_h, a.first.row, a.last.row, ah, p.row_breaks);
    let mut order = Vec::with_capacity(cols.len() * rows.len());
    if p.over_then_down {
        for r in 0..rows.len() {
            for c in 0..cols.len() {
                order.push((c, r));
            }
        }
    } else {
        for c in 0..cols.len() {
            for r in 0..rows.len() {
                order.push((c, r));
            }
        }
    }
    Pagination {
        scale,
        cols,
        rows,
        order,
        fixed: p.fixed,
    }
}

/// Areas printed for a sheet: every print area, else used cells and charts.
fn print_areas(wb: &Workbook, sheet: usize, ws: &Worksheet) -> Vec<CellRange> {
    if let Some(pa) = &ws.print_area {
        let v: Vec<CellRange> = pa
            .split(',')
            .filter_map(|part| bo_sheet::cells::resolve_ref(wb, sheet, part.trim()))
            .map(|(_, r)| r)
            .collect();
        if !v.is_empty() {
            return v;
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
    r.into_iter().collect()
}

/// Width of the printed row-heading column for row numbers up to `last_row` (0-based).
pub(super) fn heading_width(last_row: u32) -> f64 {
    let digits = (last_row + 1).to_string().len();
    7.0 * digits as f64 + 8.0
}

/// One printed page before drawing.
enum Frame {
    Cells {
        cols: (u32, u32),
        rows: (u32, u32),
        scale: f64,
    },
    Comments(Vec<String>),
}

#[allow(clippy::cast_possible_truncation)]
fn f(v: f64) -> f32 {
    v as f32
}

/// Header / footer text of the page at `index` (0-based) for the sheet's settings.
fn hf_texts(ws: &Worksheet, index: usize) -> (Option<&str>, Option<&str>) {
    let p = &ws.page_setup;
    if p.different_first && index == 0 {
        (p.first_header.as_deref(), p.first_footer.as_deref())
    } else if p.different_odd_even && index % 2 == 1 {
        (p.even_header.as_deref(), p.even_footer.as_deref())
    } else {
        (p.odd_header.as_deref(), p.odd_footer.as_deref())
    }
}

/// Black-and-white printing: white fills, black text and lines, grey shape fills.
fn black_and_white(commands: &mut [Command]) {
    const BLACK: Rgba = Rgba(0, 0, 0, 255);
    for c in commands {
        match c {
            Command::FillRect { color, .. } => *color = Rgba(255, 255, 255, color.3),
            Command::Glyphs { color, .. } | Command::Line { color, .. } => *color = BLACK,
            Command::Path { fill, stroke, .. } => {
                if let Some(Paint::Solid { color }) = fill {
                    let l = (0.299 * f64::from(color.0)
                        + 0.587 * f64::from(color.1)
                        + 0.114 * f64::from(color.2))
                    .round()
                    .clamp(0.0, 255.0);
                    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                    let g = l as u8;
                    *color = Rgba(g, g, g, color.3);
                }
                if let Some(s) = stroke {
                    s.color = BLACK;
                }
            }
            _ => {}
        }
    }
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

    /// Default font (family, size) of the workbook (first `font` of the stylesheet).
    fn base_font(&self) -> (String, f64) {
        let font = self.wb.as_ref().and_then(|w| w.styles.fonts.first());
        (
            font.and_then(|x| x.name.clone())
                .unwrap_or_else(|| "Calibri".into()),
            font.and_then(|x| x.size).unwrap_or(11.0),
        )
    }

    /// Values for `&D` `&T` `&Z` `&F` `&A` (from the engine clock, file and sheet names).
    fn hf_context(&self, sheet: usize) -> HfContext {
        let wb = self.wb.as_ref();
        let opts = bo_sheet::numfmt::FormatOptions {
            locale: self.locale,
            date1904: wb.is_some_and(|w| w.date1904),
        };
        let (date, time) = self.live.now().map_or_else(Default::default, |now| {
            (
                bo_sheet::numfmt::format_number(now, self.locale.short_date, &opts).text,
                bo_sheet::numfmt::format_number(now, "hh:mm", &opts).text,
            )
        });
        let (path, file) = match self.name.rfind(['/', '\\']) {
            Some(i) => (self.name[..=i].to_owned(), self.name[i + 1..].to_owned()),
            None => (String::new(), self.name.clone()),
        };
        HfContext {
            page: 0,
            pages: 0,
            date,
            time,
            path,
            file,
            sheet: wb
                .and_then(|w| w.sheets.get(sheet))
                .map(|s| s.name.clone())
                .unwrap_or_default(),
        }
    }

    /// Lines listed on the "comments at end" pages.
    fn comment_lines(&self, sheet: usize) -> Vec<String> {
        let Some(ws) = self.wb.as_ref().and_then(|w| w.sheets.get(sheet)) else {
            return Vec::new();
        };
        let mut items: Vec<(CellRef, String, String)> = ws
            .comments
            .iter()
            .map(|c| {
                (
                    c.cell,
                    c.author.clone(),
                    c.text.iter().map(|r| r.text.as_str()).collect::<String>(),
                )
            })
            .collect();
        let persons = self.wb.as_ref().map(|w| &w.persons);
        for t in &ws.threaded_comments {
            let who = persons
                .and_then(|p| p.iter().find(|p| p.id == t.person_id))
                .map(|p| p.display_name.clone())
                .unwrap_or_default();
            if !items.iter().any(|(c, _, x)| *c == t.cell && *x == t.text) {
                items.push((t.cell, who, t.text.clone()));
            }
        }
        items.sort_by_key(|(c, _, _)| (c.row, c.col));
        let mut lines = Vec::new();
        for (cell, author, text) in items {
            let mut parts = text.split('\n');
            let first = parts.next().unwrap_or_default();
            if author.is_empty() {
                lines.push(format!("{cell}: {first}"));
            } else {
                lines.push(format!("{cell}: {author}: {first}"));
            }
            lines.extend(parts.map(|p| format!("    {p}")));
        }
        lines
    }

    /// Where the cells of a page go: the page origin of the printed block
    /// (margins and centring applied) and the bands in front of the body.
    fn frame_box(
        &mut self,
        sheet: usize,
        st: &Settings,
        (c0, c1): (u32, u32),
        (r0, r1): (u32, u32),
        scale: f64,
    ) -> Result<FrameBox> {
        let ws = self.sheet(sheet)?;
        let (titles_rows, titles_cols) = ws
            .print_titles
            .as_deref()
            .map_or((None, None), parse_titles);
        let g = self.geometry(sheet)?;
        let width = |a: u32, b: u32| g.col_x(b + 1) - g.col_x(a);
        let height = |a: u32, b: u32| g.row_y(b + 1) - g.row_y(a);
        let (pw, ph) = st.page;
        let [ml, mr, mt, mb, ..] = st.margins;
        let (aw0, ah0) = ((pw - ml - mr).max(36.0), (ph - mt - mb).max(36.0));
        // Titles already on the page are not repeated.
        let rows_band = titles_rows.filter(|(_, b)| r0 > *b);
        let cols_band = titles_cols.filter(|(_, b)| c0 > *b);
        let th = rows_band.map_or(0.0, |(a, b)| height(a, b));
        let tw = cols_band.map_or(0.0, |(a, b)| width(a, b));
        let (hw, hh) = if st.headings {
            let last = titles_rows.map_or(r1, |(_, b)| b.max(r1));
            (heading_width(last), HEADING_H)
        } else {
            (0.0, 0.0)
        };
        let bw = width(c0, c1);
        let bh = height(r0, r1);
        let mut x0 = ml;
        let mut y0 = mt;
        if st.center_h {
            x0 += ((aw0 - (bw + tw + hw) * scale) / 2.0).max(0.0);
        }
        if st.center_v {
            y0 += ((ah0 - (bh + th + hh) * scale) / 2.0).max(0.0);
        }
        Ok(FrameBox {
            x0,
            y0,
            hw,
            hh,
            tw,
            th,
            rows_band,
            cols_band,
            bw,
            bh,
        })
    }

    /// Pagination of every print area of `sheet` (§18.2.6 `_xlnm.Print_Area`, else
    /// the used range), in print order.
    pub(super) fn paginations(
        &mut self,
        sheet: usize,
        st: &Settings,
    ) -> Result<Vec<(CellRange, Pagination)>> {
        let wb = self.wb()?;
        let ws = self.sheet(sheet)?;
        let areas = print_areas(wb, sheet, ws);
        let (titles_rows, titles_cols) = ws
            .print_titles
            .as_deref()
            .map_or((None, None), parse_titles);
        let col_breaks = ws.page_setup.col_breaks.clone();
        let row_breaks = ws.page_setup.row_breaks.clone();
        let g = self.geometry(sheet)?.clone();
        let width = |a: u32, b: u32| g.col_x(b + 1) - g.col_x(a);
        let height = |a: u32, b: u32| g.row_y(b + 1) - g.row_y(a);
        let avail = st.avail();
        let mut out = Vec::new();
        for area in areas {
            let last_row = titles_rows.map_or(area.last.row, |(_, b)| b.max(area.last.row));
            let (hw, hh) = if st.headings {
                (heading_width(last_row), HEADING_H)
            } else {
                (0.0, 0.0)
            };
            let fixed = (
                titles_cols.map_or(0.0, |(a, b)| width(a, b)) + hw,
                titles_rows.map_or(0.0, |(a, b)| height(a, b)) + hh,
            );
            let p = paginate(&PageInput {
                col_w: &|c| g.col_width(c),
                row_h: &|r| g.row_height(r),
                area,
                avail,
                fixed,
                fit: st.fit,
                scale_pct: st.scale,
                col_breaks: &col_breaks,
                row_breaks: &row_breaks,
                over_then_down: st.over_then_down,
            });
            out.push((area, p));
        }
        Ok(out)
    }

    /// Pages of one sheet in print order (cells pages, then comments pages).
    fn frames(&mut self, sheet: usize, st: &Settings) -> Result<Vec<Frame>> {
        let avail = st.avail();
        let mut out = Vec::new();
        for (_, p) in self.paginations(sheet, st)? {
            for (c, r) in p.order {
                out.push(Frame::Cells {
                    cols: p.cols[c],
                    rows: p.rows[r],
                    scale: p.scale,
                });
            }
        }
        if st.comments == CommentsMode::AtEnd {
            let lines = self.comment_lines(sheet);
            let per_page = ((avail.1 / (COMMENT_SIZE * 1.2)).floor() as usize).max(1);
            for chunk in lines.chunks(per_page) {
                out.push(Frame::Comments(chunk.to_vec()));
            }
        }
        Ok(out)
    }

    /// Page layout of a sheet (SHEET-VIEW-001 page layout view): paper size,
    /// margins and, for pages of cells, the cell spans, the scale and the page
    /// position of the first body cell.
    pub(super) fn page_layout(&mut self, sheet: usize) -> Result<Vec<PageBox>> {
        let st = settings(self.sheet(sheet)?);
        let frames = self.frames(sheet, &st)?;
        let mut out = Vec::with_capacity(frames.len());
        for fr in &frames {
            let cells = match fr {
                Frame::Cells { cols, rows, scale } => {
                    let b = self.frame_box(sheet, &st, *cols, *rows, *scale)?;
                    Some(PageCells {
                        cols: *cols,
                        rows: *rows,
                        scale: *scale,
                        origin: (b.x0 + (b.hw + b.tw) * scale, b.y0 + (b.hh + b.th) * scale),
                    })
                }
                Frame::Comments(_) => None,
            };
            out.push(PageBox {
                size: st.page,
                margins: st.margins,
                cells,
            });
        }
        Ok(out)
    }

    /// Draws one page of `sheet`.
    #[allow(clippy::too_many_lines, clippy::too_many_arguments)]
    fn draw_frame(
        &mut self,
        sheet: usize,
        st: &Settings,
        frame: &Frame,
        index: usize,
        pages: usize,
        opts: &RenderOptions,
    ) -> Result<PageList> {
        let (pw, ph) = st.page;
        let [ml, mr, mt, mb, mh, mf] = st.margins;
        let (aw0, ah0) = ((pw - ml - mr).max(36.0), (ph - mt - mb).max(36.0));
        let (font, size) = self.base_font();
        let theme = self.wb.as_ref().and_then(|w| w.theme.clone());
        let mut commands = Vec::new();
        let mut page_scale = 1.0;
        match frame {
            Frame::Cells {
                cols: (c0, c1),
                rows: (r0, r1),
                scale,
            } => {
                let (c0, c1, r0, r1, scale) = (*c0, *c1, *r0, *r1, *scale);
                page_scale = scale;
                let g = self.geometry(sheet)?.clone();
                let width = |a: u32, b: u32| g.col_x(b + 1) - g.col_x(a);
                let height = |a: u32, b: u32| g.row_y(b + 1) - g.row_y(a);
                let FrameBox {
                    x0,
                    y0,
                    hw,
                    hh,
                    tw,
                    th,
                    rows_band,
                    cols_band,
                    bw,
                    bh,
                } = self.frame_box(sheet, st, (c0, c1), (r0, r1), scale)?;
                // Local (unscaled) origin of the cells, right / below the headings.
                let lx = hw;
                let ly = hh;
                let place = |eng: &mut SheetEngine,
                             commands: &mut Vec<Command>,
                             (sx, sy, w, h): (f64, f64, f64, f64),
                             dx: f64,
                             dy: f64|
                 -> Result<()> {
                    if w <= 0.0 || h <= 0.0 {
                        return Ok(());
                    }
                    let mut band = eng.render_band(sheet, (sx, sy, w, h), opts)?;
                    let (px, py) = (x0 + dx * scale, y0 + dy * scale);
                    commands.push(Command::PushClip {
                        x: f(px),
                        y: f(py),
                        w: f(w * scale),
                        h: f(h * scale),
                    });
                    commands.push(Command::PushTransform {
                        m: [f(scale), 0.0, 0.0, f(scale), f(px), f(py)],
                    });
                    commands.append(&mut band.commands);
                    commands.push(Command::PopTransform);
                    commands.push(Command::PopClip);
                    Ok(())
                };
                let (bx, by) = (lx + tw, ly + th);
                place(
                    self,
                    &mut commands,
                    (g.col_x(c0), g.row_y(r0), bw, bh),
                    bx,
                    by,
                )?;
                if let Some((a, b)) = rows_band {
                    place(
                        self,
                        &mut commands,
                        (g.col_x(c0), g.row_y(a), bw, height(a, b)),
                        bx,
                        ly,
                    )?;
                }
                if let Some((a, b)) = cols_band {
                    place(
                        self,
                        &mut commands,
                        (g.col_x(a), g.row_y(r0), width(a, b), bh),
                        lx,
                        by,
                    )?;
                }
                if let (Some((ra, rb)), Some((ca, cb))) = (rows_band, cols_band) {
                    place(
                        self,
                        &mut commands,
                        (g.col_x(ca), g.row_y(ra), width(ca, cb), height(ra, rb)),
                        lx,
                        ly,
                    )?;
                }
                if st.headings {
                    // §18.3.1.70 printOptions/@headings: column letters and row numbers.
                    let mut cols = Vec::new();
                    if let Some((a, b)) = cols_band {
                        cols.push((a, b, lx));
                    }
                    cols.push((c0, c1, bx));
                    let mut rows = Vec::new();
                    if let Some((a, b)) = rows_band {
                        rows.push((a, b, ly));
                    }
                    rows.push((r0, r1, by));
                    let base = HfBase {
                        font: &font,
                        size: HEADING_SIZE,
                        theme: None,
                        scale: 1.0,
                    };
                    let mut local = Vec::new();
                    let line = Rgba(0x80, 0x80, 0x80, 255);
                    let cell = |local: &mut Vec<Command>,
                                fonts: &mut bo_text::FontManager,
                                (x, y, w, h): (f64, f64, f64, f64),
                                label: &str| {
                        local.push(Command::FillRect {
                            x: f(x),
                            y: f(y),
                            w: f(w),
                            h: f(h),
                            color: Rgba(0xF0, 0xF0, 0xF0, 255),
                        });
                        for (x1, y1, x2, y2) in [
                            (x, y, x + w, y),
                            (x, y + h, x + w, y + h),
                            (x, y, x, y + h),
                            (x + w, y, x + w, y + h),
                        ] {
                            local.push(Command::Line {
                                x1: f(x1),
                                y1: f(y1),
                                x2: f(x2),
                                y2: f(y2),
                                width: 0.5,
                                color: line,
                                dash: None,
                            });
                        }
                        if !label.is_empty() {
                            let runs = vec![(HfFormat::default(), label.to_owned())];
                            let top = y + (h - HEADING_SIZE * 1.2) / 2.0;
                            local.extend(hf::draw(
                                fonts,
                                &runs,
                                &base,
                                HfAlign::Center,
                                x + w / 2.0,
                                top,
                            ));
                        }
                    };
                    cell(&mut local, &mut self.fonts, (0.0, 0.0, hw, hh), "");
                    for (a, b, x) in cols {
                        let mut cx = x;
                        for c in a..=b {
                            let w = g.col_width(c);
                            if w > 0.0 {
                                let label = bo_model::sheet::col_to_letters(c);
                                cell(&mut local, &mut self.fonts, (cx, 0.0, w, hh), &label);
                            }
                            cx += w;
                        }
                    }
                    for (a, b, y) in rows {
                        let mut cy = y;
                        for r in a..=b {
                            let h = g.row_height(r);
                            if h > 0.0 {
                                let label = (r + 1).to_string();
                                cell(&mut local, &mut self.fonts, (0.0, cy, hw, h), &label);
                            }
                            cy += h;
                        }
                    }
                    commands.push(Command::PushTransform {
                        m: [f(scale), 0.0, 0.0, f(scale), f(x0), f(y0)],
                    });
                    commands.append(&mut local);
                    commands.push(Command::PopTransform);
                }
            }
            Frame::Comments(lines) => {
                let base = HfBase {
                    font: &font,
                    size: COMMENT_SIZE,
                    theme: None,
                    scale: 1.0,
                };
                commands.push(Command::PushClip {
                    x: f(ml),
                    y: f(mt),
                    w: f(aw0),
                    h: f(ah0),
                });
                let mut y = mt;
                for l in lines {
                    let runs = vec![(HfFormat::default(), l.clone())];
                    commands.extend(hf::draw(
                        &mut self.fonts,
                        &runs,
                        &base,
                        HfAlign::Left,
                        ml,
                        y,
                    ));
                    y += COMMENT_SIZE * 1.2;
                }
                commands.push(Command::PopClip);
            }
        }
        // §18.3.1.46 headerFooter: header top at the header margin, footer bottom
        // at the footer margin; `scaleWithDoc` (default on) scales the text.
        let ws = self.sheet(sheet)?;
        let (head, foot) = hf_texts(ws, index);
        let (head, foot) = (head.map(hf::parse), foot.map(hf::parse));
        let scale_with_doc = attr(&ws.page_setup.header_footer_attrs, "scaleWithDoc")
            .is_none_or(|v| v == "1" || v == "true");
        let mut ctx = self.hf_context(sheet);
        ctx.page = st.first_page + u32::try_from(index).unwrap_or(0);
        ctx.pages = u32::try_from(pages).unwrap_or(u32::MAX);
        let base = HfBase {
            font: &font,
            size,
            theme: theme.as_ref(),
            scale: if scale_with_doc { page_scale } else { 1.0 },
        };
        for (text, header) in [(head, true), (foot, false)] {
            let Some(text) = text else { continue };
            for (sec, align, x) in [
                (&text.left, HfAlign::Left, ml),
                (&text.center, HfAlign::Center, pw / 2.0),
                (&text.right, HfAlign::Right, pw - mr),
            ] {
                let runs = hf::expand(sec, &ctx);
                if runs.is_empty() {
                    continue;
                }
                let top = if header {
                    mh
                } else {
                    ph - mf - hf::height(&runs, &base)
                };
                commands.extend(hf::draw(&mut self.fonts, &runs, &base, align, x, top));
            }
        }
        if st.draft {
            commands.retain(|c| !matches!(c, Command::Image { .. }));
        }
        if st.black_white {
            black_and_white(&mut commands);
        }
        Ok(PageList {
            width: f(pw),
            height: f(ph),
            commands,
        })
    }

    /// The printed pages of one sheet (empty when there is nothing to print).
    pub(crate) fn print_pages(&mut self, sheet: usize) -> Result<Vec<PageList>> {
        let st = settings(self.sheet(sheet)?);
        let frames = self.frames(sheet, &st)?;
        let opts = self.print_options(&st);
        let n = frames.len();
        frames
            .iter()
            .enumerate()
            .map(|(i, fr)| self.draw_frame(sheet, &st, fr, i, n, &opts))
            .collect()
    }

    /// Render options of printed pages.
    fn print_options(&self, st: &Settings) -> RenderOptions {
        RenderOptions {
            locale: self.locale,
            now: self.live.now(),
            gridlines: Some(st.gridlines),
            headers: Some(false),
            ignore_frozen: true,
            charts: !st.draft,
            drawings: if st.draft {
                std::sync::Arc::default()
            } else {
                self.drawings.clone()
            },
            error_text: st.errors.clone(),
            images: self.web_images.states(),
            ..RenderOptions::default()
        }
    }

    /// Print preview: page count of `sheet` and the display list of page `page`
    /// (only that page is drawn).
    pub(crate) fn print_preview(&mut self, sheet: usize, page: usize) -> Result<serde_json::Value> {
        let st = settings(self.sheet(sheet)?);
        let frames = self.frames(sheet, &st)?;
        let n = frames.len();
        let opts = self.print_options(&st);
        let p = match frames.get(page.min(n.saturating_sub(1))) {
            Some(fr) => Some(self.draw_frame(sheet, &st, fr, page.min(n - 1), n, &opts)?),
            None => None,
        };
        Ok(serde_json::json!({ "pages": n, "page": p }))
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
        // Pictures in cells and IMAGE pictures (SHEET-CELL-001).
        let images = self.media_of(&pages);
        bo_pdf::write_pdf_with_images(
            &DisplayList { pages },
            &self.fonts,
            &bo_pdf::PdfMeta {
                title: Some(self.name.clone()),
            },
            &images,
        )
        .map_err(EngineError::from)
    }
}

#[cfg(test)]
#[path = "print_tests.rs"]
mod tests;
