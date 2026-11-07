//! Render the visible part of a worksheet into a `bo-render` display list
//! ([`PageList`]) that the web canvas, PNG and PDF backends draw unchanged.
//!
//! Drawing order per region: background, gridlines, cell fills (which hide
//! gridlines, as fills do in common spreadsheet applications), text, borders.
//! Headers (column letters, row numbers) are drawn last. Coordinates are points.
//!
//! Not yet rendered: conditional formatting, row/column default styles on empty
//! cells, dashed/dotted border patterns (drawn solid), rotated text, charts.

use crate::color::resolve_color;
use crate::geometry::{col_to_letters, Rect, SheetGeometry, PT_PER_PX};
use crate::numfmt::{format_code, FormatOptions, Formatted, Locale, NumberFormat};
use crate::viewport::{PaneKind, PaneRegion, ViewLayout, Viewport};
use bo_model::sheet::{
    BorderEdge, Cell, CellRange, CellRef, CellValue, CellXf, Fill, Font, ThemeColors, Workbook,
    Worksheet,
};
use bo_render::{Command, Glyph, PageList, Rgba};
use bo_text::{FaceId, FontManager};

/// Rendering options.
#[derive(Debug, Clone, PartialEq)]
pub struct RenderOptions {
    /// Locale for number formatting (separators, month names).
    pub locale: Locale,
    /// Override the sheet view's `showGridLines` (§18.3.1.87).
    pub gridlines: Option<bool>,
    /// Override the sheet view's `showRowColHeaders`.
    pub headers: Option<bool>,
    pub gridline_color: Rgba,
    pub background: Rgba,
    pub header_background: Rgba,
    pub header_text: Rgba,
    pub header_line: Rgba,
    /// Font size of header labels in points.
    pub header_font_size: f32,
}

impl Default for RenderOptions {
    fn default() -> Self {
        RenderOptions {
            locale: Locale::VI_VN,
            gridlines: None,
            headers: None,
            gridline_color: Rgba(0xDA, 0xDC, 0xE0, 255),
            background: Rgba(255, 255, 255, 255),
            header_background: Rgba(0xF3, 0xF4, 0xF6, 255),
            header_text: Rgba(0x3C, 0x40, 0x43, 255),
            header_line: Rgba(0xC0, 0xC4, 0xC8, 255),
            header_font_size: 9.0,
        }
    }
}

/// Display text of a cell value with its number format (§18.8.30 / §18.8.31).
pub fn format_cell(wb: &Workbook, cell: &Cell, locale: &Locale) -> Formatted {
    let id = wb.styles.xf(cell.style).map_or(0, |x| x.num_fmt_id);
    let code = format_code(&wb.styles, id, locale);
    NumberFormat::parse_with(&code, locale).format(
        &cell.value,
        &FormatOptions {
            locale: *locale,
            date1904: wb.date1904,
        },
    )
}

/// Render the visible cells of sheet `sheet` into one display-list page whose
/// size is the viewport size.
pub fn render_viewport(
    wb: &Workbook,
    sheet: usize,
    geom: &SheetGeometry,
    viewport: &Viewport,
    opts: &RenderOptions,
    fonts: &mut FontManager,
) -> PageList {
    let mut page = PageList {
        width: viewport.width as f32,
        height: viewport.height as f32,
        commands: Vec::new(),
    };
    let Some(ws) = wb.sheets.get(sheet) else {
        return page;
    };
    let view = ws.views.first();
    let show_grid = opts
        .gridlines
        .or_else(|| view.and_then(|v| v.show_grid_lines))
        .unwrap_or(true);
    let show_headers = opts
        .headers
        .or_else(|| view.and_then(|v| v.show_headers))
        .unwrap_or(true);
    let mut vp = *viewport;
    if !show_headers {
        vp.header_width = 0.0;
        vp.header_height = 0.0;
    }
    let layout = ViewLayout::for_sheet(geom, ws, &vp);
    let mut r = Renderer {
        wb,
        ws,
        geom,
        opts,
        fonts,
        theme: wb.theme.as_ref(),
        out: &mut page.commands,
    };
    for pane in &layout.panes {
        r.pane(pane, show_grid);
    }
    if show_headers {
        r.headers(&layout);
    }
    page
}

struct Renderer<'a> {
    wb: &'a Workbook,
    ws: &'a Worksheet,
    geom: &'a SheetGeometry,
    opts: &'a RenderOptions,
    fonts: &'a mut FontManager,
    theme: Option<&'a ThemeColors>,
    out: &'a mut Vec<Command>,
}

fn f(v: f64) -> f32 {
    v as f32
}

fn intersects(m: &CellRange, rows: &std::ops::Range<u32>, cols: &std::ops::Range<u32>) -> bool {
    m.first.row < rows.end
        && m.last.row >= rows.start
        && m.first.col < cols.end
        && m.last.col >= cols.start
}

/// Horizontal placement of cell text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HAlign {
    Left,
    Center,
    Right,
}

impl Renderer<'_> {
    fn fill_rect(&mut self, r: Rect, color: Rgba) {
        if r.w > 0.0 && r.h > 0.0 {
            self.out.push(Command::FillRect {
                x: f(r.x),
                y: f(r.y),
                w: f(r.w),
                h: f(r.h),
                color,
            });
        }
    }

    fn line(&mut self, x1: f64, y1: f64, x2: f64, y2: f64, width: f64, color: Rgba) {
        self.out.push(Command::Line {
            x1: f(x1),
            y1: f(y1),
            x2: f(x2),
            y2: f(y2),
            width: f(width),
            color,
        });
    }

    fn screen_rect(p: &PaneRegion, r: Rect) -> Rect {
        Rect {
            x: p.to_screen_x(r.x),
            y: p.to_screen_y(r.y),
            w: r.w,
            h: r.h,
        }
    }

    fn xf(&self, cell: &Cell) -> Option<&CellXf> {
        self.wb.styles.xf(cell.style)
    }

    fn pane(&mut self, p: &PaneRegion, show_grid: bool) {
        let s = p.screen;
        if s.w <= 0.0 || s.h <= 0.0 {
            return;
        }
        self.out.push(Command::PushClip {
            x: f(s.x),
            y: f(s.y),
            w: f(s.w),
            h: f(s.h),
        });
        self.fill_rect(s, self.opts.background);
        let merges: Vec<CellRange> = self
            .geom
            .merges()
            .iter()
            .copied()
            .filter(|m| intersects(m, &p.rows, &p.cols))
            .collect();
        if show_grid {
            self.gridlines(p, &merges);
        }
        // Cells: visit the visible rows, plus anchors of merges that start off-screen.
        let mut cells: Vec<(CellRef, &Cell)> = Vec::new();
        for row in p.rows.clone() {
            if self.geom.row_height(row) <= 0.0 {
                continue;
            }
            for (&(r, c), cell) in self.ws.cells.range((row, p.cols.start)..(row, p.cols.end)) {
                let at = CellRef::new(r, c);
                match merges.iter().find(|m| m.contains(at)) {
                    Some(m) if m.first != at => {}
                    _ => cells.push((at, cell)),
                }
            }
        }
        for m in &merges {
            let inside = p.rows.contains(&m.first.row) && p.cols.contains(&m.first.col);
            if !inside {
                if let Some(cell) = self.ws.cell(m.first.row, m.first.col) {
                    cells.push((m.first, cell));
                }
            }
        }
        for &(at, cell) in &cells {
            let rect = Self::screen_rect(p, self.geom.cell_rect(at.row, at.col));
            self.cell_fill(cell, rect);
        }
        for &(at, cell) in &cells {
            self.cell_text(p, at, cell, &merges);
        }
        for &(at, cell) in &cells {
            let rect = Self::screen_rect(p, self.geom.cell_rect(at.row, at.col));
            self.cell_borders(cell, rect);
        }
        self.out.push(Command::PopClip);
    }

    fn gridlines(&mut self, p: &PaneRegion, merges: &[CellRange]) {
        let s = p.screen;
        let color = self.opts.gridline_color;
        let w = PT_PER_PX;
        for c in p.cols.clone() {
            if self.geom.col_width(c) <= 0.0 {
                continue;
            }
            let x = p.to_screen_x(self.geom.col_x(c + 1)) - w / 2.0;
            self.line(x, s.y, x, s.y + s.h, w, color);
        }
        for r in p.rows.clone() {
            if self.geom.row_height(r) <= 0.0 {
                continue;
            }
            let y = p.to_screen_y(self.geom.row_y(r + 1)) - w / 2.0;
            self.line(s.x, y, s.x + s.w, y, w, color);
        }
        // Merged areas have no inner gridlines.
        for m in merges {
            let r = Self::screen_rect(p, self.geom.range_rect(*m));
            let inner = Rect {
                x: r.x,
                y: r.y,
                w: r.w - w,
                h: r.h - w,
            };
            self.fill_rect(inner, self.opts.background);
        }
    }

    fn cell_fill(&mut self, cell: &Cell, rect: Rect) {
        let Some(xf) = self.xf(cell) else { return };
        let Some(fill) = self.wb.styles.fills.get(xf.fill_id as usize) else {
            return;
        };
        // §18.8.32 patternFill: a solid pattern paints fgColor; other patterns are
        // approximated by their foreground colour. §18.8.24 gradientFill: first stop.
        let color = match fill {
            Fill::Pattern(pf) => match pf.pattern.as_deref() {
                None | Some("none" | "gray125") => None,
                _ => pf
                    .fg
                    .as_ref()
                    .and_then(|c| resolve_color(c, self.theme))
                    .or(Some(Rgba(0, 0, 0, 255))),
            },
            Fill::Gradient(g) => g
                .stops
                .first()
                .and_then(|(_, c)| resolve_color(c, self.theme)),
        };
        if let Some(c) = color {
            self.fill_rect(rect, c);
        }
    }

    fn edge(&mut self, e: &BorderEdge, x1: f64, y1: f64, x2: f64, y2: f64, vertical: bool) {
        // §18.18.3 ST_BorderStyle → line width in points.
        let width = match e.style.as_deref() {
            None | Some("none") => return,
            Some("hair") => 0.5,
            Some(
                "medium" | "mediumDashed" | "mediumDashDot" | "mediumDashDotDot" | "slantDashDot",
            ) => 1.5,
            Some("thick") => 2.25,
            _ => PT_PER_PX,
        };
        let color = e
            .color
            .as_ref()
            .and_then(|c| resolve_color(c, self.theme))
            .unwrap_or(Rgba(0, 0, 0, 255));
        if e.style.as_deref() == Some("double") {
            let d = PT_PER_PX;
            if vertical {
                self.line(x1 - d, y1, x2 - d, y2, PT_PER_PX, color);
                self.line(x1 + d, y1, x2 + d, y2, PT_PER_PX, color);
            } else {
                self.line(x1, y1 - d, x2, y2 - d, PT_PER_PX, color);
                self.line(x1, y1 + d, x2, y2 + d, PT_PER_PX, color);
            }
        } else {
            self.line(x1, y1, x2, y2, width, color);
        }
    }

    fn cell_borders(&mut self, cell: &Cell, r: Rect) {
        let Some(xf) = self.xf(cell) else { return };
        let Some(b) = self.wb.styles.borders.get(xf.border_id as usize).cloned() else {
            return;
        };
        let (x0, y0, x1, y1) = (r.x, r.y, r.x + r.w, r.y + r.h);
        self.edge(&b.top, x0, y0, x1, y0, false);
        self.edge(&b.bottom, x0, y1, x1, y1, false);
        self.edge(&b.left, x0, y0, x0, y1, true);
        self.edge(&b.right, x1, y0, x1, y1, true);
        if b.diagonal_down {
            self.edge(&b.diagonal, x0, y0, x1, y1, false);
        }
        if b.diagonal_up {
            self.edge(&b.diagonal, x0, y1, x1, y0, false);
        }
    }

    fn font_of(&self, cell: &Cell) -> Font {
        let fonts = &self.wb.styles.fonts;
        self.xf(cell)
            .and_then(|x| fonts.get(x.font_id as usize))
            .or_else(|| fonts.first())
            .cloned()
            .unwrap_or_default()
    }

    /// Glyph runs for `text` (with per-character font fallback); returns the width.
    #[allow(clippy::type_complexity)]
    fn shape_runs(
        &mut self,
        face: FaceId,
        text: &str,
        size: f32,
    ) -> (Vec<(FaceId, String, Vec<Glyph>, f64)>, f64) {
        let mut x = 0f32;
        let mut runs = Vec::new();
        for (range, fid) in self.fonts.itemize(face, text) {
            let part = &text[range];
            let fscale = size / f32::from(self.fonts.info(fid).units_per_em.max(1));
            let shaped = self.fonts.shape(fid, part);
            let start = x;
            let mut glyphs = Vec::with_capacity(shaped.len());
            for g in shaped.iter() {
                glyphs.push(Glyph(
                    g.glyph_id,
                    x - start + g.x_offset as f32 * fscale,
                    -(g.y_offset as f32) * fscale,
                    g.cluster,
                ));
                x += g.x_advance as f32 * fscale;
            }
            runs.push((fid, part.to_owned(), glyphs, f64::from(x - start)));
        }
        (runs, f64::from(x))
    }

    fn emit_text(
        &mut self,
        face: FaceId,
        text: &str,
        size: f32,
        x: f64,
        baseline: f64,
        color: Rgba,
    ) -> f64 {
        let (runs, width) = self.shape_runs(face, text, size);
        let mut rx = x;
        for (fid, part, glyphs, run_w) in runs {
            if !part.trim().is_empty() {
                self.out.push(Command::Glyphs {
                    face: fid.0,
                    size,
                    x: f(rx),
                    y: f(baseline),
                    color,
                    text: part,
                    glyphs,
                });
            }
            rx += run_w;
        }
        width
    }

    fn measure(&mut self, face: FaceId, text: &str, size: f32) -> f64 {
        self.shape_runs(face, text, size).1
    }

    #[allow(clippy::too_many_lines)]
    fn cell_text(&mut self, p: &PaneRegion, at: CellRef, cell: &Cell, merges: &[CellRange]) {
        if matches!(cell.value, CellValue::Empty) {
            return;
        }
        let fmt = format_cell(self.wb, cell, &self.opts.locale);
        if fmt.text.is_empty() {
            return;
        }
        let font = self.font_of(cell);
        let size = font.size.unwrap_or(11.0) as f32;
        let name = font.name.clone().unwrap_or_else(|| "Calibri".into());
        let Some(face) = self.fonts.resolve(&name, font.bold, font.italic) else {
            return;
        };
        let color = fmt
            .color
            .map(|c| {
                let (r, g, b) = c.rgb();
                Rgba(r, g, b, 255)
            })
            .or_else(|| {
                font.color
                    .as_ref()
                    .and_then(|c| resolve_color(c, self.theme))
            })
            .unwrap_or(Rgba(0, 0, 0, 255));
        let align = self
            .xf(cell)
            .and_then(|x| x.alignment.clone())
            .unwrap_or_default();
        let is_number = matches!(cell.value, CellValue::Number(_));
        // §18.18.40 ST_HorizontalAlignment; `general` aligns numbers right, text
        // left, booleans and errors centred.
        let halign = match align.horizontal.as_deref() {
            Some("left" | "fill" | "justify" | "distributed") => HAlign::Left,
            Some("center" | "centerContinuous") => HAlign::Center,
            Some("right") => HAlign::Right,
            _ => match cell.value {
                CellValue::Number(_) | CellValue::Date(_) => HAlign::Right,
                CellValue::Bool(_) | CellValue::Error(_) => HAlign::Center,
                _ => HAlign::Left,
            },
        };
        let rect = Self::screen_rect(p, self.geom.cell_rect(at.row, at.col));
        let pad = 2.0 * PT_PER_PX;
        let indent =
            f64::from(align.indent.unwrap_or(0)) * 3.0 * self.geom.metrics.mdw_px * PT_PER_PX;
        let info = self.fonts.info(face).clone();
        let upem = f64::from(info.units_per_em.max(1));
        let ascent = f64::from(info.ascender) * f64::from(size) / upem;
        let descent = -f64::from(info.descender) * f64::from(size) / upem;
        let line_h = ascent + descent + f64::from(info.line_gap.max(0)) * f64::from(size) / upem;

        // Lines: wrapped text or a single line.
        let avail = (rect.w - 2.0 * pad - indent).max(0.0);
        let mut lines: Vec<String> = Vec::new();
        let mut text = fmt.text.clone();
        if let Some((pos, ch)) = fmt.fill {
            // `*x` fill: repeat the character to fill the cell (§18.8.31).
            let base = self.measure(face, &text, size);
            let cw = self.measure(face, &ch.to_string(), size).max(0.1);
            let n = ((avail - base) / cw).floor().max(0.0) as usize;
            if pos <= text.len() && text.is_char_boundary(pos) {
                text.insert_str(pos, &ch.to_string().repeat(n.min(1000)));
            }
        }
        if align.wrap_text && !is_number {
            for para in text.split('\n') {
                let mut cur = String::new();
                for word in para.split(' ') {
                    let cand = if cur.is_empty() {
                        word.to_owned()
                    } else {
                        format!("{cur} {word}")
                    };
                    if !cur.is_empty() && self.measure(face, &cand, size) > avail {
                        lines.push(std::mem::take(&mut cur));
                        cur = word.to_owned();
                    } else {
                        cur = cand;
                    }
                }
                lines.push(cur);
            }
        } else {
            lines.push(text.replace('\n', " "));
        }
        // Numbers that do not fit show `#` across the cell.
        if is_number
            && !align.shrink_to_fit
            && (fmt.invalid || self.measure(face, &lines[0], size) > avail)
        {
            let hw = self.measure(face, "#", size).max(0.1);
            let n = ((avail / hw).floor() as usize).max(1);
            lines = vec!["#".repeat(n.min(255))];
        }

        // Clip: the cell, or for unwrapped left/right text the run of empty cells it overflows into.
        let mut clip = rect;
        let width0 = self.measure(face, &lines[0], size);
        if !align.wrap_text
            && !is_number
            && width0 > avail
            && merges.iter().all(|m| !m.contains(at))
        {
            let row_cells = |c: u32| {
                self.ws
                    .cell(at.row, c)
                    .is_some_and(|c| !matches!(c.value, CellValue::Empty))
            };
            match halign {
                HAlign::Left => {
                    let mut c = at.col + 1;
                    let mut right = rect.x + rect.w;
                    while c < p.cols.end
                        && !row_cells(c)
                        && right - rect.x - pad - indent < width0 + pad
                    {
                        right += self.geom.col_width(c);
                        c += 1;
                    }
                    clip.w = right - rect.x;
                }
                HAlign::Right => {
                    let mut c = at.col;
                    let mut left = rect.x;
                    while c > p.cols.start
                        && !row_cells(c - 1)
                        && rect.x + rect.w - left - pad < width0 + pad
                    {
                        c -= 1;
                        left -= self.geom.col_width(c);
                    }
                    clip.w += rect.x - left;
                    clip.x = left;
                }
                HAlign::Center => {}
            }
        }
        self.out.push(Command::PushClip {
            x: f(clip.x),
            y: f(clip.y),
            w: f(clip.w),
            h: f(clip.h),
        });
        let total_h = line_h * lines.len() as f64;
        // §18.18.88 ST_VerticalAlignment (default bottom).
        let top = match align.vertical.as_deref() {
            Some("top") => rect.y + pad / 2.0,
            Some("center" | "justify" | "distributed") => rect.y + (rect.h - total_h) / 2.0,
            _ => rect.y + rect.h - total_h - pad / 2.0,
        };
        for (i, line) in lines.iter().enumerate() {
            let w = self.measure(face, line, size);
            let x = match halign {
                HAlign::Left => rect.x + pad + indent,
                HAlign::Right => rect.x + rect.w - pad - w - indent,
                HAlign::Center => rect.x + (rect.w - w) / 2.0,
            };
            let baseline = top + line_h * i as f64 + ascent;
            let width = self.emit_text(face, line, size, x, baseline, color);
            // §18.8.22 font u / strike: underline below the baseline, strike through x-height.
            let thick = (f64::from(info.underline_thickness) * f64::from(size) / upem).max(0.5);
            if font.underline.as_deref().is_some_and(|u| u != "none") {
                let y = baseline - f64::from(info.underline_position) * f64::from(size) / upem;
                self.line(x, y, x + width, y, thick, color);
                if font
                    .underline
                    .as_deref()
                    .is_some_and(|u| u.starts_with("double"))
                {
                    self.line(x, y + 2.0 * thick, x + width, y + 2.0 * thick, thick, color);
                }
            }
            if font.strike {
                let y = baseline - f64::from(info.strikeout_position) * f64::from(size) / upem;
                self.line(x, y, x + width, y, thick, color);
            }
        }
        self.out.push(Command::PopClip);
    }

    fn header_label(&mut self, text: &str, r: Rect) {
        let size = self.opts.header_font_size;
        let name = self
            .wb
            .styles
            .fonts
            .first()
            .and_then(|f| f.name.clone())
            .unwrap_or_else(|| "Calibri".into());
        let Some(face) = self.fonts.resolve(&name, false, false) else {
            return;
        };
        let info = self.fonts.info(face).clone();
        let upem = f64::from(info.units_per_em.max(1));
        let ascent = f64::from(info.ascender) * f64::from(size) / upem;
        let descent = -f64::from(info.descender) * f64::from(size) / upem;
        let w = self.measure(face, text, size);
        let x = r.x + (r.w - w) / 2.0;
        let baseline = r.y + (r.h - ascent - descent) / 2.0 + ascent;
        let color = self.opts.header_text;
        self.emit_text(face, text, size, x, baseline, color);
    }

    fn headers(&mut self, layout: &ViewLayout) {
        let vp = layout.viewport;
        let (bg, line) = (self.opts.header_background, self.opts.header_line);
        let lw = PT_PER_PX;
        // Column headers above the bottom regions (main + left), row headers left
        // of the right regions (main + top).
        for p in &layout.panes {
            if matches!(p.kind, PaneKind::Main | PaneKind::Left) && vp.header_height > 0.0 {
                let band = Rect {
                    x: p.screen.x,
                    y: 0.0,
                    w: p.screen.w,
                    h: vp.header_height,
                };
                if band.w > 0.0 {
                    self.out.push(Command::PushClip {
                        x: f(band.x),
                        y: f(band.y),
                        w: f(band.w),
                        h: f(band.h),
                    });
                    self.fill_rect(band, bg);
                    for c in p.cols.clone() {
                        let w = self.geom.col_width(c);
                        if w <= 0.0 {
                            continue;
                        }
                        let x = p.to_screen_x(self.geom.col_x(c));
                        let cell = Rect {
                            x,
                            y: 0.0,
                            w,
                            h: vp.header_height,
                        };
                        self.header_label(&col_to_letters(c), cell);
                        self.line(
                            x + w - lw / 2.0,
                            0.0,
                            x + w - lw / 2.0,
                            vp.header_height,
                            lw,
                            line,
                        );
                    }
                    self.line(
                        band.x,
                        band.h - lw / 2.0,
                        band.x + band.w,
                        band.h - lw / 2.0,
                        lw,
                        line,
                    );
                    self.out.push(Command::PopClip);
                }
            }
            if matches!(p.kind, PaneKind::Main | PaneKind::Top) && vp.header_width > 0.0 {
                let band = Rect {
                    x: 0.0,
                    y: p.screen.y,
                    w: vp.header_width,
                    h: p.screen.h,
                };
                if band.h > 0.0 {
                    self.out.push(Command::PushClip {
                        x: f(band.x),
                        y: f(band.y),
                        w: f(band.w),
                        h: f(band.h),
                    });
                    self.fill_rect(band, bg);
                    for r in p.rows.clone() {
                        let h = self.geom.row_height(r);
                        if h <= 0.0 {
                            continue;
                        }
                        let y = p.to_screen_y(self.geom.row_y(r));
                        let cell = Rect {
                            x: 0.0,
                            y,
                            w: vp.header_width,
                            h,
                        };
                        self.header_label(&(r + 1).to_string(), cell);
                        self.line(
                            0.0,
                            y + h - lw / 2.0,
                            vp.header_width,
                            y + h - lw / 2.0,
                            lw,
                            line,
                        );
                    }
                    self.line(
                        band.w - lw / 2.0,
                        band.y,
                        band.w - lw / 2.0,
                        band.y + band.h,
                        lw,
                        line,
                    );
                    self.out.push(Command::PopClip);
                }
            }
        }
        if vp.header_width > 0.0 && vp.header_height > 0.0 {
            let corner = Rect {
                x: 0.0,
                y: 0.0,
                w: vp.header_width,
                h: vp.header_height,
            };
            self.fill_rect(corner, bg);
            self.line(
                corner.w - lw / 2.0,
                0.0,
                corner.w - lw / 2.0,
                corner.h,
                lw,
                line,
            );
            self.line(
                0.0,
                corner.h - lw / 2.0,
                corner.w,
                corner.h - lw / 2.0,
                lw,
                line,
            );
        }
        // Frozen-pane divider lines.
        if layout.frozen_cols > 0 || layout.frozen_rows > 0 {
            if let Some(main) = layout.panes.iter().find(|p| p.kind == PaneKind::Main) {
                let c = Rgba(0x9A, 0xA0, 0xA6, 255);
                if layout.frozen_cols > 0 {
                    self.line(
                        main.screen.x,
                        vp.header_height,
                        main.screen.x,
                        vp.height,
                        lw,
                        c,
                    );
                }
                if layout.frozen_rows > 0 {
                    self.line(
                        vp.header_width,
                        main.screen.y,
                        vp.width,
                        main.screen.y,
                        lw,
                        c,
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::SheetMetrics;
    use bo_model::sheet::{Border, CellAlignment, Stylesheet};

    fn fonts() -> FontManager {
        let mut fm = FontManager::new();
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/fonts");
        fm.add_fonts_from_dir(&dir);
        fm
    }

    fn workbook() -> Workbook {
        let mut wb = Workbook::new();
        let mut ss = Stylesheet::default_styles();
        let red_fill = ss.add_fill(Fill::solid("FF0000"));
        let num = ss.add_num_fmt("#,##0.00");
        let border = ss.add_border(Border {
            bottom: BorderEdge {
                style: Some("thick".into()),
                color: None,
            },
            ..Border::default()
        });
        let filled = ss.add_xf(CellXf {
            fill_id: red_fill,
            border_id: border,
            ..CellXf::default()
        });
        let numxf = ss.add_xf(CellXf {
            num_fmt_id: num,
            ..CellXf::default()
        });
        let centered = ss.add_xf(CellXf {
            alignment: Some(CellAlignment {
                horizontal: Some("center".into()),
                ..CellAlignment::default()
            }),
            ..CellXf::default()
        });
        wb.styles = ss;
        let ws = &mut wb.sheets[0];
        ws.set_cell(
            0,
            0,
            Cell::new(CellValue::Text("Xin chào thế giới, overflow".into())),
        );
        ws.set_cell(1, 1, Cell::new(CellValue::Number(1234.5)).with_style(numxf));
        ws.set_cell(2, 0, Cell::new(CellValue::Empty).with_style(filled));
        ws.set_cell(3, 2, Cell::new(CellValue::Bool(true)).with_style(centered));
        ws.set_cell(4, 0, Cell::new(CellValue::Number(123_456_789_012.0)));
        ws.merges.push(CellRange::parse("D1:E2").unwrap());
        ws.set_cell(0, 3, Cell::new(CellValue::Text("merged".into())));
        wb
    }

    #[test]
    fn format_cell_uses_style_number_format() {
        let wb = workbook();
        let c = wb.sheets[0].cell(1, 1).unwrap();
        assert_eq!(format_cell(&wb, c, &Locale::VI_VN).text, "1.234,50");
        assert_eq!(format_cell(&wb, c, &Locale::EN_US).text, "1,234.50");
    }

    #[test]
    fn renders_cells_fills_borders_and_headers() {
        let wb = workbook();
        let geom = SheetGeometry::with_metrics(&wb.sheets[0], SheetMetrics::default());
        let vp = Viewport::new(400.0, 200.0);
        let mut fm = fonts();
        let page = render_viewport(&wb, 0, &geom, &vp, &RenderOptions::default(), &mut fm);
        assert_eq!((page.width, page.height), (400.0, 200.0));
        let texts: Vec<&str> = page
            .commands
            .iter()
            .filter_map(|c| match c {
                Command::Glyphs { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect();
        if !fm.is_empty() {
            assert!(texts.iter().any(|t| t.contains("Xin")), "{texts:?}");
            assert!(texts.contains(&"1.234,50"));
            assert!(texts.contains(&"TRUE"));
            assert!(texts.contains(&"merged"));
            assert!(texts.contains(&"A") && texts.contains(&"1"), "headers");
        }
        // Red fill of A3 at x = 30 (header), y = 15 + 2 × 15.
        assert!(page.commands.iter().any(|c| matches!(c,
            Command::FillRect { x, y, color: Rgba(255, 0, 0, 255), .. } if *x == 30.0 && *y == 45.0)));
        // Thick bottom border of A3.
        assert!(page.commands.iter().any(|c| matches!(c,
            Command::Line { width, y1, .. } if *width == 2.25 && *y1 == 60.0)));
        // Clips are balanced.
        let push = page
            .commands
            .iter()
            .filter(|c| matches!(c, Command::PushClip { .. }))
            .count();
        let pop = page
            .commands
            .iter()
            .filter(|c| matches!(c, Command::PopClip))
            .count();
        assert_eq!(push, pop);
        // JSON bridge works.
        assert!(page.to_json().contains("\"op\":\"fillRect\""));
    }

    #[test]
    fn hidden_gridlines_and_headers() {
        let mut wb = workbook();
        wb.sheets[0].views.push(bo_model::sheet::SheetView {
            show_grid_lines: Some(false),
            show_headers: Some(false),
            ..Default::default()
        });
        let geom = SheetGeometry::with_metrics(&wb.sheets[0], SheetMetrics::default());
        let vp = Viewport::new(200.0, 100.0);
        let page = render_viewport(
            &wb,
            0,
            &geom,
            &vp,
            &RenderOptions::default(),
            &mut FontManager::new(),
        );
        let grid = RenderOptions::default().gridline_color;
        assert!(!page
            .commands
            .iter()
            .any(|c| matches!(c, Command::Line { color, .. } if *color == grid)));
        // Without headers the grid starts at the origin: A3 fill at y = 30.
        assert!(page.commands.iter().any(|c| matches!(c,
            Command::FillRect { x, y, color: Rgba(255, 0, 0, 255), .. } if *x == 0.0 && *y == 30.0)));
        // Out-of-range sheet → empty page.
        let empty = render_viewport(
            &wb,
            9,
            &geom,
            &vp,
            &RenderOptions::default(),
            &mut FontManager::new(),
        );
        assert_eq!(empty.commands.len(), 0);
    }
}
