//! Render the visible part of a worksheet into a `bo-render` display list
//! ([`PageList`]) that the web canvas, PNG and PDF backends draw unchanged.
//!
//! Drawing order per region: background, gridlines, cell fills (which hide
//! gridlines, as fills do in common spreadsheet applications), text, borders.
//! Headers (column letters, row numbers) are drawn last. Coordinates are points.
//!
//! Conditional formatting (§18.3.1.18) overlays fills, font changes, data bars
//! and icons; sparklines ([MS-XLSX] `x14:sparklineGroup`, [`crate::sparkline`]) are drawn
//! in their cells before the text; AutoFilter header buttons (§18.3.1.2) and charts placed on the
//! sheet (§20.5 drawing anchors) are drawn on top of the cells.
//!
//! Row/column default styles are drawn on empty cells; rotated (and stacked) text is drawn
//! with a transform (§18.8.1 `textRotation`).

use crate::cf::CellFormat;
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
    /// Colour of the cell gridlines.
    pub gridline_color: Rgba,
    /// Background of cells without a fill.
    pub background: Rgba,
    /// Background of the row and column headers.
    pub header_background: Rgba,
    /// Text colour of the header labels.
    pub header_text: Rgba,
    /// Colour of the lines between header cells.
    pub header_line: Rgba,
    /// Font size of header labels in points.
    pub header_font_size: f32,
    /// Serial date-time for `TODAY()`/`NOW()` in conditional formats.
    pub now: Option<f64>,
    /// Draw charts placed on the sheet.
    pub charts: bool,
    /// Lay the sheet out without its frozen panes (printing).
    pub ignore_frozen: bool,
    /// Pre-drawn SmartArt fallbacks by drawing part name ([`DrawingOverlay`]),
    /// placed at the anchors of the sheet's [`Worksheet::diagrams`].
    pub drawings: std::sync::Arc<std::collections::BTreeMap<String, DrawingOverlay>>,
}

/// Vector drawing of a SmartArt fallback ([MS-ODRAWXML] `dsp:drawing`), drawn
/// for a `w`×`h` point box and scaled to the anchor's current size.
#[derive(Debug, Clone, PartialEq)]
pub struct DrawingOverlay {
    /// Width of the drawing box (points).
    pub w: f64,
    /// Height of the drawing box (points).
    pub h: f64,
    /// Drawing commands in box coordinates.
    pub commands: Vec<Command>,
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
            now: None,
            charts: true,
            ignore_frozen: false,
            drawings: std::sync::Arc::default(),
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

/// Width and height in points that the displayed text of `cell` needs,
/// cell padding included (column / row AutoFit).
pub fn measure_cell(
    wb: &Workbook,
    cell: &Cell,
    fonts: &mut FontManager,
    locale: &Locale,
) -> (f64, f64) {
    let text = format_cell(wb, cell, locale).text;
    let font = wb
        .styles
        .xf(cell.style)
        .and_then(|x| wb.styles.fonts.get(x.font_id as usize))
        .or_else(|| wb.styles.fonts.first())
        .cloned()
        .unwrap_or_default();
    let size = font.size.unwrap_or(11.0);
    let name = font.name.unwrap_or_else(|| "Calibri".into());
    let lines: Vec<&str> = text.split('\n').collect();
    let n = lines.len() as f64;
    let Some(face) = fonts.resolve(&name, font.bold, font.italic) else {
        let chars = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0) as f64;
        return (chars * size * 0.55 + 6.0, n * size * 1.3 + 2.0);
    };
    let w = lines
        .iter()
        .map(|l| f64::from(fonts.measure(face, l, size as f32)))
        .fold(0.0, f64::max);
    let info = fonts.info(face);
    let upem = f64::from(info.units_per_em.max(1));
    let line_h = f64::from(
        i32::from(info.ascender) - i32::from(info.descender) + i32::from(info.line_gap.max(0)),
    ) * size
        / upem;
    (
        w + 6.0 * PT_PER_PX,
        ((line_h * n) / PT_PER_PX).ceil() * PT_PER_PX + 2.0 * PT_PER_PX,
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
    let layout = if opts.ignore_frozen {
        ViewLayout::compute(geom, &vp, 0, 0)
    } else {
        ViewLayout::for_sheet(geom, ws, &vp)
    };
    let mut r = Renderer {
        wb,
        ws,
        sheet,
        cf: std::collections::HashMap::new(),
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
    sheet: usize,
    /// Conditional formatting of the pane being drawn.
    cf: std::collections::HashMap<(u32, u32), CellFormat>,
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
            dash: None,
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
        // Empty positions take the row style (§18.3.1.73 `row/@s` with
        // `customFormat`), else the column style (§18.3.1.13 `col/@style`).
        let synth = self.default_styled_cells(p, &merges);
        cells.extend(synth.iter().map(|(at, c)| (*at, c)));
        self.cf = crate::cf::evaluate(
            self.wb,
            self.sheet,
            p.rows.clone(),
            p.cols.clone(),
            self.opts.now,
        );
        // Table styles (§18.5.1.5) under the conditional formats.
        crate::table_style::overlay(
            self.wb,
            self.sheet,
            p.rows.clone(),
            p.cols.clone(),
            &mut self.cf,
        );
        for &(at, cell) in &cells {
            let rect = Self::screen_rect(p, self.geom.cell_rect(at.row, at.col));
            self.cell_fill(cell, rect);
        }
        // Conditional fills (also on positions without a cell).
        let mut cf: Vec<((u32, u32), CellFormat)> =
            self.cf.iter().map(|(k, v)| (*k, v.clone())).collect();
        cf.sort_by_key(|(k, _)| *k);
        for ((r, c), f) in &cf {
            let at = CellRef::new(*r, *c);
            if self.geom.row_height(*r) <= 0.0 || self.geom.col_width(*c) <= 0.0 {
                continue;
            }
            if merges.iter().any(|m| m.contains(at) && m.first != at) {
                continue;
            }
            let rect = Self::screen_rect(p, self.geom.cell_rect(*r, *c));
            if let Some(color) = f.fill {
                self.fill_rect(rect, color);
            }
            if let Some(bar) = f.bar {
                let pad = 2.0 * PT_PER_PX;
                let inner = Rect {
                    x: rect.x + pad,
                    y: rect.y + pad,
                    w: (rect.w - 2.0 * pad).max(0.0),
                    h: (rect.h - 2.0 * pad).max(0.0),
                };
                self.out.extend(crate::cf_draw::data_bar(&bar, inner));
            }
            if let Some(icon) = &f.icon {
                let s = (rect.h - 4.0).clamp(4.0, 12.0);
                let cmds = crate::icons::icon(
                    &icon.set,
                    icon.index,
                    icon.count,
                    rect.x + 2.0,
                    rect.y + (rect.h - s) / 2.0,
                    s,
                );
                self.out.extend(cmds);
            }
        }
        self.sparklines(p);
        for &(at, cell) in &cells {
            self.cell_text(p, at, cell, &merges);
        }
        for &(at, cell) in &cells {
            let rect = Self::screen_rect(p, self.geom.cell_rect(at.row, at.col));
            self.cell_borders(cell, rect);
        }
        self.filter_buttons(p);
        if self.opts.charts {
            self.diagrams(p);
            self.charts(p);
        }
        self.out.push(Command::PopClip);
    }

    /// Sparklines ([MS-XLSX] `x14:sparklineGroup`) located in the pane, clipped to their cell.
    fn sparklines(&mut self, p: &PaneRegion) {
        for (gi, g) in self.ws.sparkline_groups.iter().enumerate() {
            for (si, s) in g.sparklines.iter().enumerate() {
                let at = s.location;
                if !p.rows.contains(&at.row)
                    || !p.cols.contains(&at.col)
                    || self.geom.row_height(at.row) <= 0.0
                    || self.geom.col_width(at.col) <= 0.0
                {
                    continue;
                }
                let rect = Self::screen_rect(p, self.geom.cell_rect(at.row, at.col));
                let cmds = crate::sparkline::render(self.wb, self.sheet, gi, si, rect);
                if cmds.is_empty() {
                    continue;
                }
                self.out.push(Command::PushClip {
                    x: f(rect.x),
                    y: f(rect.y),
                    w: f(rect.w),
                    h: f(rect.h),
                });
                self.out.extend(cmds);
                self.out.push(Command::PopClip);
            }
        }
    }

    /// Style-only cells for visible empty positions of styled rows / columns.
    fn default_styled_cells(&self, p: &PaneRegion, merges: &[CellRange]) -> Vec<(CellRef, Cell)> {
        let col_styles: Vec<(u32, u32)> = p
            .cols
            .clone()
            .filter_map(|c| {
                let s = self.ws.col_info(c)?.style?;
                (s != 0 && self.geom.col_width(c) > 0.0).then_some((c, s))
            })
            .collect();
        let mut out = Vec::new();
        for row in p.rows.clone() {
            if self.geom.row_height(row) <= 0.0 {
                continue;
            }
            let row_style = self
                .ws
                .rows
                .get(&row)
                .and_then(|r| r.style)
                .filter(|s| *s != 0);
            let candidates: Vec<(u32, u32)> = match row_style {
                Some(s) => p
                    .cols
                    .clone()
                    .filter(|c| self.geom.col_width(*c) > 0.0)
                    .map(|c| (c, s))
                    .collect(),
                None => col_styles.clone(),
            };
            for (c, style) in candidates {
                let at = CellRef::new(row, c);
                if self.ws.cell(row, c).is_some() || merges.iter().any(|m| m.contains(at)) {
                    continue;
                }
                out.push((at, Cell::new(CellValue::Empty).with_style(style)));
                if out.len() >= 200_000 {
                    return out;
                }
            }
        }
        out
    }

    /// AutoFilter dropdown buttons on the header row (§18.3.1.2 `autoFilter`,
    /// §18.3.2.7 `filterColumn/@hiddenButton`).
    fn filter_buttons(&mut self, p: &PaneRegion) {
        let Some(af) = &self.ws.auto_filter else {
            return;
        };
        let row = af.range.first.row;
        if !p.rows.contains(&row) || self.geom.row_height(row) <= 0.0 {
            return;
        }
        for col in af.range.first.col..=af.range.last.col {
            if !p.cols.contains(&col) || self.geom.col_width(col) <= 0.0 {
                continue;
            }
            let fc = af
                .columns
                .iter()
                .find(|c| af.range.first.col + c.col_id == col);
            if fc.is_some_and(|c| c.hidden_button || c.show_button == Some(false)) {
                continue;
            }
            let active =
                fc.is_some_and(|c| !matches!(c.filter, bo_model::sheet::ColumnFilter::None));
            let r = Self::screen_rect(p, self.geom.cell_rect(row, col));
            let s = (r.h - 3.0).clamp(6.0, 12.0);
            let b = Rect {
                x: r.x + r.w - s - 1.5,
                y: r.y + (r.h - s) / 2.0,
                w: s,
                h: s,
            };
            self.fill_rect(b, Rgba(0xF3, 0xF4, 0xF6, 255));
            let edge = Rgba(0xA0, 0xA6, 0xAD, 255);
            self.line(b.x, b.y, b.x + b.w, b.y, 0.5, edge);
            self.line(b.x, b.y + b.h, b.x + b.w, b.y + b.h, 0.5, edge);
            self.line(b.x, b.y, b.x, b.y + b.h, 0.5, edge);
            self.line(b.x + b.w, b.y, b.x + b.w, b.y + b.h, 0.5, edge);
            let color = if active {
                Rgba(0x1F, 0x7A, 0x4D, 255)
            } else {
                Rgba(0x3C, 0x40, 0x43, 255)
            };
            let (cx, cy, h) = (b.x + b.w / 2.0, b.y + b.h / 2.0, b.w * 0.28);
            // Down triangle (a funnel stem is added when a criterion is active).
            self.out.push(Command::Path {
                segs: vec![
                    bo_render::PathSeg::M(f(cx - h), f(cy - h * 0.6)),
                    bo_render::PathSeg::L(f(cx + h), f(cy - h * 0.6)),
                    bo_render::PathSeg::L(f(cx), f(cy + h * 0.6)),
                    bo_render::PathSeg::Z,
                ],
                fill: Some(bo_render::Paint::Solid { color }),
                stroke: None,
            });
            if active {
                self.line(cx, cy + h * 0.5, cx, cy + h * 1.3, 1.0, color);
            }
        }
    }

    /// Charts anchored on the sheet (§20.5.2.33 `twoCellAnchor`), clipped to the pane.
    /// SmartArt drawing fallbacks over the cells of their two-cell anchor
    /// (§20.5.2.33 twoCellAnchor; §21.4.2.6 relIds).
    fn diagrams(&mut self, p: &PaneRegion) {
        for d in &self.ws.diagrams {
            let Some(ov) = self.opts.drawings.get(&d.part) else {
                continue;
            };
            let x0 = self.geom.col_x(d.from.col);
            let y0 = self.geom.row_y(d.from.row);
            let (w, h) = match d.to {
                Some(to) => (self.geom.col_x(to.col) - x0, self.geom.row_y(to.row) - y0),
                None => (ov.w, ov.h),
            };
            if w <= 0.0 || h <= 0.0 || ov.w <= 0.0 || ov.h <= 0.0 {
                continue;
            }
            let sx = p.to_screen_x(x0);
            let sy = p.to_screen_y(y0);
            let s = p.screen;
            if sx > s.x + s.w || sy > s.y + s.h || sx + w < s.x || sy + h < s.y {
                continue;
            }
            self.out.push(Command::PushClip {
                x: f(sx),
                y: f(sy),
                w: f(w),
                h: f(h),
            });
            self.out.push(Command::PushTransform {
                m: [f(w / ov.w), 0.0, 0.0, f(h / ov.h), f(sx), f(sy)],
            });
            self.out.extend(ov.commands.iter().cloned());
            self.out.push(Command::PopTransform);
            self.out.push(Command::PopClip);
        }
    }

    fn charts(&mut self, p: &PaneRegion) {
        for chart in &self.ws.charts {
            let x0 = self.geom.col_x(chart.from.col);
            let y0 = self.geom.row_y(chart.from.row);
            let to = chart.to.unwrap_or(CellRef::new(
                chart.from.row.saturating_add(15),
                chart.from.col.saturating_add(8),
            ));
            let (x1, y1) = (self.geom.col_x(to.col), self.geom.row_y(to.row));
            let (w, h) = ((x1 - x0).max(60.0), (y1 - y0).max(40.0));
            let sx = p.to_screen_x(x0);
            let sy = p.to_screen_y(y0);
            let s = p.screen;
            if sx > s.x + s.w || sy > s.y + s.h || sx + w < s.x || sy + h < s.y {
                continue;
            }
            self.out.push(Command::PushClip {
                x: f(sx),
                y: f(sy),
                w: f(w),
                h: f(h),
            });
            self.out.push(Command::PushTransform {
                m: [1.0, 0.0, 0.0, 1.0, f(sx), f(sy)],
            });
            let mut cmds = Vec::new();
            let name = self
                .wb
                .styles
                .fonts
                .first()
                .and_then(|f| f.name.clone())
                .unwrap_or_else(|| "Calibri".into());
            let face = self.fonts.resolve(&name, false, false);
            let locale = self.opts.locale;
            {
                let fonts = &mut *self.fonts;
                let mut label = |text: &str,
                                 [l, t, r, b]: [f64; 4],
                                 align: crate::chart::Align,
                                 size: f64,
                                 color: Rgba,
                                 out: &mut Vec<Command>| {
                    let Some(face) = face else { return };
                    let size32 = size as f32;
                    let width = f64::from(fonts.measure(face, text, size32));
                    let info = fonts.info(face).clone();
                    let upem = f64::from(info.units_per_em.max(1));
                    let asc = f64::from(info.ascender) * size / upem;
                    let desc = -f64::from(info.descender) * size / upem;
                    let x = match align {
                        crate::chart::Align::Left => l,
                        crate::chart::Align::Center => f64::midpoint(l, r) - width / 2.0,
                        crate::chart::Align::Right => r - width,
                    };
                    let baseline = t + ((b - t) - asc - desc) / 2.0 + asc;
                    let mut gx = 0f32;
                    let mut x_run = x;
                    for (range, fid) in fonts.itemize(face, text) {
                        let part = &text[range];
                        let fscale = size32 / f32::from(fonts.info(fid).units_per_em.max(1));
                        let shaped = fonts.shape(fid, part);
                        let start = gx;
                        let glyphs: Vec<Glyph> = shaped
                            .iter()
                            .map(|g| {
                                let gl = Glyph(
                                    g.glyph_id,
                                    gx - start + g.x_offset as f32 * fscale,
                                    -(g.y_offset as f32) * fscale,
                                    g.cluster,
                                );
                                gx += g.x_advance as f32 * fscale;
                                gl
                            })
                            .collect();
                        if !part.trim().is_empty() {
                            out.push(Command::Glyphs {
                                face: fid.0,
                                size: size32,
                                x: f(x_run),
                                y: f(baseline),
                                color,
                                text: part.to_owned(),
                                glyphs,
                            });
                        }
                        x_run = x + f64::from(gx);
                    }
                };
                crate::chart::draw_chart(
                    self.wb, self.sheet, chart, w, h, &locale, &mut label, &mut cmds,
                );
            }
            self.out.extend(cmds);
            self.out.push(Command::PopTransform);
            self.out.push(Command::PopClip);
        }
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
            // §18.18.3 dashed / dotted styles.
            let dash = match e.style.as_deref() {
                Some("dotted" | "hair") => Some([1.0, 1.0]),
                Some("dashed" | "mediumDashed") => Some([3.0, 1.5]),
                Some(
                    "dashDot" | "mediumDashDot" | "dashDotDot" | "mediumDashDotDot"
                    | "slantDashDot",
                ) => Some([3.0, 1.0]),
                _ => None,
            };
            self.out.push(Command::Line {
                x1: f(x1),
                y1: f(y1),
                x2: f(x2),
                y2: f(y2),
                width: f(width),
                color,
                dash,
            });
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
        let cf = self.cf.get(&(at.row, at.col)).cloned().unwrap_or_default();
        if cf.hide_value {
            return;
        }
        let mut font = self.font_of(cell);
        // Conditional formatting font overrides (§18.8.14 dxf font).
        if let Some(b) = cf.bold {
            font.bold = b;
        }
        if let Some(i) = cf.italic {
            font.italic = i;
        }
        if let Some(s) = cf.strike {
            font.strike = s;
        }
        if let Some(u) = cf.underline {
            font.underline = u.then(|| "single".to_owned());
        }
        let size = font.size.unwrap_or(11.0) as f32;
        let name = font.name.clone().unwrap_or_else(|| "Calibri".into());
        let Some(face) = self.fonts.resolve(&name, font.bold, font.italic) else {
            return;
        };
        let color = cf
            .font_color
            .or_else(|| {
                fmt.color.map(|c| {
                    let (r, g, b) = c.rgb();
                    Rgba(r, g, b, 255)
                })
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

        // §18.8.1 alignment/@textRotation: 1–90 counter-clockwise, 91–180 clockwise
        // by (value − 90) degrees, 255 = vertical (stacked) text.
        if let Some(rot) = align
            .text_rotation
            .filter(|r| *r != 0 && (*r <= 180 || *r == 255))
        {
            let v = align.vertical.as_deref();
            let text = fmt.text.replace('\n', " ");
            self.out.push(Command::PushClip {
                x: f(rect.x),
                y: f(rect.y),
                w: f(rect.w),
                h: f(rect.h),
            });
            if rot == 255 {
                let chars: Vec<char> = text.chars().filter(|c| *c != ' ').take(1000).collect();
                let total = line_h * chars.len() as f64;
                let top = match v {
                    Some("top") => rect.y + pad / 2.0,
                    Some("center" | "justify" | "distributed") => rect.y + (rect.h - total) / 2.0,
                    _ => rect.y + rect.h - total - pad / 2.0,
                };
                for (i, ch) in chars.iter().enumerate() {
                    let s = ch.to_string();
                    let w = self.measure(face, &s, size);
                    let x = match halign {
                        HAlign::Left => rect.x + pad + indent,
                        HAlign::Right => rect.x + rect.w - pad - w,
                        HAlign::Center => rect.x + (rect.w - w) / 2.0,
                    };
                    self.emit_text(face, &s, size, x, top + line_h * i as f64 + ascent, color);
                }
            } else {
                let deg = if rot <= 90 {
                    f64::from(rot)
                } else {
                    -f64::from(rot - 90)
                };
                let (sin, cos) = deg.to_radians().sin_cos();
                let w = self.measure(face, &text, size);
                let h = ascent + descent;
                // Bounding box of the rotated line, placed by the cell alignment.
                let bw = (w * cos).abs() + (h * sin).abs();
                let bh = (w * sin).abs() + (h * cos).abs();
                let cx = match halign {
                    HAlign::Left => rect.x + pad + indent + bw / 2.0,
                    HAlign::Right => rect.x + rect.w - pad - bw / 2.0,
                    HAlign::Center => rect.x + rect.w / 2.0,
                };
                let cy = match v {
                    Some("top") => rect.y + pad / 2.0 + bh / 2.0,
                    Some("center" | "justify" | "distributed") => rect.y + rect.h / 2.0,
                    _ => rect.y + rect.h - pad / 2.0 - bh / 2.0,
                };
                // Counter-clockwise on screen (y down): x' = cos·x + sin·y, y' = −sin·x + cos·y.
                self.out.push(Command::PushTransform {
                    m: [f(cos), f(-sin), f(sin), f(cos), f(cx), f(cy)],
                });
                let baseline = ascent - h / 2.0;
                let width = self.emit_text(face, &text, size, -w / 2.0, baseline, color);
                let thick = (f64::from(info.underline_thickness) * f64::from(size) / upem).max(0.5);
                if font.underline.as_deref().is_some_and(|u| u != "none") {
                    let y = baseline - f64::from(info.underline_position) * f64::from(size) / upem;
                    self.line(-w / 2.0, y, -w / 2.0 + width, y, thick, color);
                }
                if font.strike {
                    let y = baseline - f64::from(info.strikeout_position) * f64::from(size) / upem;
                    self.line(-w / 2.0, y, -w / 2.0 + width, y, thick, color);
                }
                self.out.push(Command::PopTransform);
            }
            self.out.push(Command::PopClip);
            return;
        }

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
    fn draws_sparklines_in_their_cells() {
        let mut wb = Workbook::new();
        for (c, v) in [1.0, 3.0, 2.0].iter().enumerate() {
            wb.sheets[0].set_cell(0, c as u32, Cell::new(CellValue::Number(*v)));
        }
        crate::sparkline::insert(
            &mut wb,
            0,
            bo_model::sheet::SparklineType::Column,
            "A1:C1",
            CellRange::parse("D1").unwrap(),
        )
        .unwrap();
        let geom = SheetGeometry::with_metrics(&wb.sheets[0], SheetMetrics::default());
        let cell = geom.cell_rect(0, 3);
        let opts = RenderOptions {
            headers: Some(false),
            ..RenderOptions::default()
        };
        let page = render_viewport(
            &wb,
            0,
            &geom,
            &Viewport::new(400.0, 200.0),
            &opts,
            &mut fonts(),
        );
        let series = Rgba(0x2E, 0x6D, 0xB4, 255);
        let bars: Vec<(f32, f32, f32, f32)> = page
            .commands
            .iter()
            .filter_map(|c| match c {
                Command::FillRect { x, y, w, h, color } if *color == series => {
                    Some((*x, *y, *w, *h))
                }
                _ => None,
            })
            .collect();
        assert_eq!(bars.len(), 3);
        // The tallest bar (3 of 0..3) fills the plot box: cell height − 2 × 2 pt.
        let tall = bars[1];
        assert!((tall.3 - (cell.h - 4.0) as f32).abs() < 1e-3);
        assert!((tall.1 - (cell.y + 2.0) as f32).abs() < 1e-3);
        assert!(bars
            .iter()
            .all(|b| b.0 >= cell.x as f32 && b.0 + b.2 <= (cell.x + cell.w) as f32));
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

    #[test]
    fn row_and_column_styles_fill_empty_cells() {
        let mut wb = workbook();
        let blue = wb.styles.add_fill(Fill::solid("0000FF"));
        let green = wb.styles.add_fill(Fill::solid("00FF00"));
        let row_xf = wb.styles.add_xf(CellXf {
            fill_id: blue,
            ..CellXf::default()
        });
        let col_xf = wb.styles.add_xf(CellXf {
            fill_id: green,
            ..CellXf::default()
        });
        let ws = &mut wb.sheets[0];
        ws.rows.insert(
            6,
            bo_model::sheet::RowInfo {
                style: Some(row_xf),
                ..Default::default()
            },
        );
        ws.cols.push(bo_model::sheet::ColInfo {
            min: 5,
            max: 5,
            style: Some(col_xf),
            ..Default::default()
        });
        let geom = SheetGeometry::with_metrics(&wb.sheets[0], SheetMetrics::default());
        let vp = Viewport::new(600.0, 200.0);
        let page = render_viewport(&wb, 0, &geom, &vp, &RenderOptions::default(), &mut fonts());
        let fill = |r: u8, g: u8, b: u8| {
            page.commands
                .iter()
                .filter(|c| matches!(c, Command::FillRect { color, .. } if *color == Rgba(r, g, b, 255)))
                .count()
        };
        // Row 7 is blue across the visible columns; column F green on the other rows.
        assert!(fill(0, 0, 255) >= 5, "row style");
        assert!(fill(0, 255, 0) >= 5, "column style");
    }

    #[test]
    fn rotated_text_is_drawn_rotated() {
        let mut wb = workbook();
        let up = wb.styles.add_xf(CellXf {
            alignment: Some(CellAlignment {
                text_rotation: Some(45),
                ..CellAlignment::default()
            }),
            ..CellXf::default()
        });
        let stacked = wb.styles.add_xf(CellXf {
            alignment: Some(CellAlignment {
                text_rotation: Some(255),
                ..CellAlignment::default()
            }),
            ..CellXf::default()
        });
        let ws = &mut wb.sheets[0];
        ws.set_cell(
            7,
            1,
            Cell::new(CellValue::Text("Xoay".into())).with_style(up),
        );
        ws.set_cell(
            7,
            2,
            Cell::new(CellValue::Text("Dọc".into())).with_style(stacked),
        );
        let geom = SheetGeometry::with_metrics(&wb.sheets[0], SheetMetrics::default());
        let vp = Viewport::new(400.0, 200.0);
        let mut fm = fonts();
        if fm.is_empty() {
            return;
        }
        let page = render_viewport(&wb, 0, &geom, &vp, &RenderOptions::default(), &mut fm);
        let i = page
            .commands
            .iter()
            .position(|c| matches!(c, Command::PushTransform { .. }))
            .expect("rotation transform");
        let Command::PushTransform { m } = &page.commands[i] else {
            unreachable!()
        };
        let s = std::f32::consts::FRAC_1_SQRT_2;
        assert!((m[0] - s).abs() < 1e-3 && (m[1] + s).abs() < 1e-3, "{m:?}");
        assert!(matches!(&page.commands[i + 1], Command::Glyphs { text, .. } if text == "Xoay"));
        // Stacked text: one glyph run per character.
        for ch in ["D", "ọ", "c"] {
            assert!(page
                .commands
                .iter()
                .any(|c| matches!(c, Command::Glyphs { text, .. } if text == ch)));
        }
    }
}
