//! Page Break Preview and Page Layout view of BlueSheet (round 34), measured on the reference
//! at 125 % (docs/ui/REPORT-sheet.md):
//!
//! - Page Break Preview is the normal grid (its own zoom, 60 % by default) with the cells
//!   outside the print areas grey #A0A0A0, a 3 px #0000D0 frame around each print area, the
//!   automatic page breaks dashed 9 px on / 3 px off and manual ones solid (2 px), and a big
//!   grey "Page N" in each page. Breaks are dragged with the mouse (engine `movePageBreak`).
//! - Page Layout view shows the printed pages (engine page lists with the sheet's gridlines)
//!   on a #E6E6E6 ground with an 8 px gap and a #C6C6C6 edge, a ruler above and left of the
//!   pages (margins #F2F2F2), the column letters / row numbers of each page beside it, and the
//!   selection drawn on the page.
use super::{SheetView, SEL_GREEN};
use crate::{Font, Painter};
use bo_model::sheet::{col_to_letters, CellRange, CellRef};
use bo_render::{Command, PageList, Rgba};
use serde_json::{json, Value};

/// Grey of the cells outside the print areas in Page Break Preview.
pub(super) const OUTSIDE_GREY: Rgba = Rgba(0xa0, 0xa0, 0xa0, 255);
const BREAK_BLUE: Rgba = Rgba(0x00, 0x00, 0xd0, 255);
const PAGE_LABEL: Rgba = Rgba(0x96, 0x96, 0x96, 150);
const LAYOUT_BG: Rgba = Rgba(0xe6, 0xe6, 0xe6, 255);
const PAGE_EDGE: Rgba = Rgba(0xc6, 0xc6, 0xc6, 255);
const RULER_MARGIN: Rgba = Rgba(0xf2, 0xf2, 0xf2, 255);
const RULER_EDGE: Rgba = Rgba(0xab, 0xab, 0xab, 255);
const RULER_TICK: Rgba = Rgba(0xab, 0xab, 0xab, 255);
const RULER_TEXT: Rgba = Rgba(0x59, 0x59, 0x59, 255);
const HEADER_LINE: Rgba = Rgba(0xc6, 0xc6, 0xc6, 255);
const HEADER_SEL: Rgba = Rgba(0xd2, 0xd2, 0xd2, 255);
const HINT: Rgba = Rgba(0xa6, 0xa6, 0xa6, 255);

/// One print area of the page break preview.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct BreakArea {
    pub range: CellRange,
    /// Column / row spans of the pages (first, last).
    pub cols: Vec<(u32, u32)>,
    pub rows: Vec<(u32, u32)>,
    /// (column span index, row span index, page number).
    pub pages: Vec<(usize, usize, u32)>,
    /// First column / row of the next page at each break, and whether it is manual.
    pub col_breaks: Vec<(u32, bool)>,
    pub row_breaks: Vec<(u32, bool)>,
}

/// Page break preview of a sheet (engine `pageBreakPreview`).
pub(crate) fn parse_breaks(v: &Value) -> Vec<BreakArea> {
    let n = |v: &Value, k: &str| v[k].as_u64().map_or(0, |x| x as u32);
    let spans = |v: &Value| -> Vec<(u32, u32)> {
        v.as_array()
            .map(|a| {
                a.iter()
                    .map(|s| {
                        (
                            s[0].as_u64().unwrap_or(0) as u32,
                            s[1].as_u64().unwrap_or(0) as u32,
                        )
                    })
                    .collect()
            })
            .unwrap_or_default()
    };
    let brks = |v: &Value| -> Vec<(u32, bool)> {
        v.as_array()
            .map(|a| {
                a.iter()
                    .map(|b| (n(b, "at"), b["manual"].as_bool().unwrap_or(false)))
                    .collect()
            })
            .unwrap_or_default()
    };
    v["areas"]
        .as_array()
        .map(|areas| {
            areas
                .iter()
                .map(|a| BreakArea {
                    range: CellRange::new(
                        CellRef::new(n(a, "r0"), n(a, "c0")),
                        CellRef::new(n(a, "r1"), n(a, "c1")),
                    ),
                    cols: spans(&a["cols"]),
                    rows: spans(&a["rows"]),
                    pages: a["pages"]
                        .as_array()
                        .map(|p| {
                            p.iter()
                                .map(|p| {
                                    (
                                        p["col"].as_u64().unwrap_or(0) as usize,
                                        p["row"].as_u64().unwrap_or(0) as usize,
                                        n(p, "number"),
                                    )
                                })
                                .collect()
                        })
                        .unwrap_or_default(),
                    col_breaks: brks(&a["colBreaks"]),
                    row_breaks: brks(&a["rowBreaks"]),
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Cells printed on a page of the page layout view.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PageCells {
    pub cols: (u32, u32),
    pub rows: (u32, u32),
    pub scale: f64,
    /// Page position (points) of the first cell's top-left corner.
    pub origin: (f64, f64),
    /// Column / row boundaries from the first cell, unscaled points (len = count + 1).
    pub xs: Vec<f64>,
    pub ys: Vec<f64>,
}

/// A page of the page layout view.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct LayoutPage {
    /// Column and row of the page in the page grid.
    pub at: (usize, usize),
    pub size: (f64, f64),
    /// Left, right, top, bottom, header, footer margins (points).
    pub margins: [f64; 6],
    pub cells: Option<PageCells>,
}

/// State of the page layout view.
#[derive(Debug, Clone, Default)]
pub(crate) struct PageLayoutView {
    pub pages: Vec<LayoutPage>,
    /// Left / top of each page column / row in layout points.
    pub col_x: Vec<f64>,
    pub row_y: Vec<f64>,
    /// Size of the page grid (points).
    pub extent: (f64, f64),
    /// Scroll in layout points.
    pub scroll: (f64, f64),
    /// Ruler in inches (the region's measurement system), else centimetres.
    pub inches: bool,
}

/// Gap between pages (8 px at 125 %, in points at 100 % zoom).
const GAP_PT: f64 = 4.8;

impl PageLayoutView {
    /// Pages laid out on a grid from the engine's `pageLayout` boxes.
    pub(crate) fn build(v: &Value, xs_ys: &dyn Fn(u32, u32, bool) -> Vec<f64>) -> Vec<LayoutPage> {
        let boxes = v["pages"].as_array().cloned().unwrap_or_default();
        let mut c0s: Vec<u32> = Vec::new();
        let mut r0s: Vec<u32> = Vec::new();
        for b in &boxes {
            if let Some(c) = b.get("cells").filter(|c| !c.is_null()) {
                let c0 = c["c0"].as_u64().unwrap_or(0) as u32;
                let r0 = c["r0"].as_u64().unwrap_or(0) as u32;
                if !c0s.contains(&c0) {
                    c0s.push(c0);
                }
                if !r0s.contains(&r0) {
                    r0s.push(r0);
                }
            }
        }
        c0s.sort_unstable();
        r0s.sort_unstable();
        let mut extra_row = r0s.len();
        boxes
            .iter()
            .map(|b| {
                let f = |k: &str| b[k].as_f64().unwrap_or(0.0);
                let m = |i: usize| b["margins"][i].as_f64().unwrap_or(0.0);
                let cells = b.get("cells").filter(|c| !c.is_null()).map(|c| {
                    let u = |k: &str| c[k].as_u64().unwrap_or(0) as u32;
                    let (c0, c1, r0, r1) = (u("c0"), u("c1"), u("r0"), u("r1"));
                    PageCells {
                        cols: (c0, c1),
                        rows: (r0, r1),
                        scale: c["scale"].as_f64().unwrap_or(1.0),
                        origin: (
                            c["x"].as_f64().unwrap_or(0.0),
                            c["y"].as_f64().unwrap_or(0.0),
                        ),
                        xs: xs_ys(c0, c1, false),
                        ys: xs_ys(r0, r1, true),
                    }
                });
                let at = if let Some(c) = &cells {
                    (
                        c0s.iter().position(|x| *x == c.cols.0).unwrap_or(0),
                        r0s.iter().position(|x| *x == c.rows.0).unwrap_or(0),
                    )
                } else {
                    // Comments pages follow below the cells.
                    extra_row += 1;
                    (0, extra_row - 1)
                };
                LayoutPage {
                    at,
                    size: (f("width"), f("height")),
                    margins: [m(0), m(1), m(2), m(3), m(4), m(5)],
                    cells,
                }
            })
            .collect()
    }

    pub(crate) fn set_pages(&mut self, pages: Vec<LayoutPage>) {
        let cols = pages.iter().map(|p| p.at.0 + 1).max().unwrap_or(0);
        let rows = pages.iter().map(|p| p.at.1 + 1).max().unwrap_or(0);
        let mut w = vec![0.0_f64; cols];
        let mut h = vec![0.0_f64; rows];
        for p in &pages {
            w[p.at.0] = w[p.at.0].max(p.size.0);
            h[p.at.1] = h[p.at.1].max(p.size.1);
        }
        let mut x = 0.0;
        self.col_x = w
            .iter()
            .map(|v| {
                let a = x;
                x += v + GAP_PT;
                a
            })
            .collect();
        let mut y = 0.0;
        self.row_y = h
            .iter()
            .map(|v| {
                let a = y;
                y += v + GAP_PT;
                a
            })
            .collect();
        self.extent = ((x - GAP_PT).max(0.0), (y - GAP_PT).max(0.0));
        self.pages = pages;
    }

    /// Layout position of page `i`'s top-left corner (points).
    pub(crate) fn page_pos(&self, i: usize) -> (f64, f64) {
        let p = &self.pages[i];
        (
            self.col_x.get(p.at.0).copied().unwrap_or(0.0),
            self.row_y.get(p.at.1).copied().unwrap_or(0.0),
        )
    }

    /// Page position (points) of the rectangle of cells `r` on page `i`, clipped to the
    /// page's cells; `None` when the page does not print any of them.
    pub(crate) fn cells_rect(&self, i: usize, r: CellRange) -> Option<[f64; 4]> {
        let c = self.pages.get(i)?.cells.as_ref()?;
        let (rc0, rc1) = (r.first.col.max(c.cols.0), r.last.col.min(c.cols.1));
        let (rr0, rr1) = (r.first.row.max(c.rows.0), r.last.row.min(c.rows.1));
        if rc0 > rc1 || rr0 > rr1 {
            return None;
        }
        let x = |col: u32| c.origin.0 + c.xs[(col - c.cols.0) as usize] * c.scale;
        let y = |row: u32| c.origin.1 + c.ys[(row - c.rows.0) as usize] * c.scale;
        Some([x(rc0), y(rr0), x(rc1 + 1), y(rr1 + 1)])
    }

    /// Page and cell under the layout point (points).
    pub(crate) fn hit(&self, lx: f64, ly: f64) -> Option<(usize, Option<(u32, u32)>)> {
        let i = (0..self.pages.len()).find(|&i| {
            let (px, py) = self.page_pos(i);
            let s = self.pages[i].size;
            lx >= px && lx < px + s.0 && ly >= py && ly < py + s.1
        })?;
        let (px, py) = self.page_pos(i);
        let Some(c) = &self.pages[i].cells else {
            return Some((i, None));
        };
        let (x, y) = (
            (lx - px - c.origin.0) / c.scale,
            (ly - py - c.origin.1) / c.scale,
        );
        let find = |v: &[f64], t: f64| -> Option<usize> {
            if t < 0.0 || v.len() < 2 || t >= v[v.len() - 1] {
                return None;
            }
            Some(
                v.windows(2)
                    .position(|w| t >= w[0] && t < w[1])
                    .unwrap_or(0),
            )
        };
        let cell = find(&c.xs, x)
            .zip(find(&c.ys, y))
            .map(|(ci, ri)| (c.rows.0 + ri as u32, c.cols.0 + ci as u32));
        Some((i, cell))
    }
}

impl SheetView {
    /// Pixels at 125 % scaled to the current display scale.
    fn u(&self, px: f32) -> f32 {
        (px * self.scale / 1.25).round()
    }

    /// Bands above / left of the pages in page layout view (ruler and headers).
    fn layout_bands(&self) -> (f32, f32) {
        // Top band measured on the reference at 125 %: 98 px from the frame to the first page.
        (self.u(76.0), self.u(67.0))
    }

    /// Window position of the page area and of layout point (0, 0).
    fn layout_origin(&self) -> ([f32; 4], (f32, f32)) {
        let [fx, fy, fw, fh] = self.frame;
        let (lb, tb) = self.layout_bands();
        let area = [fx + lb, fy + tb, (fw - lb).max(1.0), (fh - tb).max(1.0)];
        let Some(pl) = self.layout.as_ref() else {
            return (area, (area[0], area[1]));
        };
        let k = f64::from(self.k());
        (
            area,
            (
                area[0] + self.u(20.0) - (pl.scroll.0 * k) as f32,
                area[1] + self.u(18.0) - (pl.scroll.1 * k) as f32,
            ),
        )
    }

    /// Labels of the page views in the UI language: "Page {n}" and "Add header".
    pub fn set_page_labels(&mut self, page: String, add_header: String) {
        self.page_labels = (page, add_header);
    }

    /// Whether the page layout view is shown.
    pub fn page_layout(&self) -> bool {
        self.layout.is_some()
    }

    /// Whether Page Break Preview is shown.
    pub fn break_preview(&self) -> bool {
        self.breaks.is_some()
    }

    /// Switch the page layout view on / off (ruler units in inches with `inches`).
    pub fn set_page_layout(&mut self, on: bool, inches: bool) {
        if on {
            self.breaks = None;
            self.layout = Some(PageLayoutView {
                inches,
                ..PageLayoutView::default()
            });
            self.reload_layout();
        } else {
            self.layout = None;
        }
        self.redraw_all();
        self.refresh();
    }

    /// Switch Page Break Preview on / off.
    pub fn set_break_preview(&mut self, on: bool) {
        if on {
            self.layout = None;
            self.breaks = Some(Vec::new());
            self.reload_breaks();
        } else {
            self.breaks = None;
        }
        self.redraw_all();
        self.refresh();
    }

    /// Read the print areas and breaks again (after an edit).
    pub(crate) fn reload_breaks(&mut self) {
        if self.breaks.is_some() {
            let v = self
                .engine
                .exec_json(&json!({ "cmd": "pageBreakPreview", "sheet": self.sheet }).to_string())
                .ok()
                .and_then(|s| serde_json::from_str::<Value>(&s).ok())
                .unwrap_or(Value::Null);
            self.breaks = Some(parse_breaks(&v));
        }
    }

    /// Read the pages again (after an edit or a page setup change).
    pub(crate) fn reload_layout(&mut self) {
        let Some(mut pl) = self.layout.take() else {
            return;
        };
        let v = self
            .engine
            .exec_json(&json!({ "cmd": "pageLayout", "sheet": self.sheet }).to_string())
            .ok()
            .and_then(|s| serde_json::from_str::<Value>(&s).ok())
            .unwrap_or(Value::Null);
        let vp = self.viewport();
        let pages = match self.engine.view_layout(self.sheet, &vp) {
            Ok((_, g)) => PageLayoutView::build(&v, &|a, b, rows| {
                let base = if rows { g.row_y(a) } else { g.col_x(a) };
                (a..=b + 1)
                    .map(|i| (if rows { g.row_y(i) } else { g.col_x(i) }) - base)
                    .collect()
            }),
            Err(_) => Vec::new(),
        };
        pl.set_pages(pages);
        self.layout = Some(pl);
    }

    /// The composed display list of the visible pages (page area coordinates, points).
    pub(crate) fn layout_list(&mut self) -> Result<PageList, String> {
        let k = f64::from(self.k());
        let (area, origin) = self.layout_origin();
        let Some(pl) = self.layout.as_ref() else {
            return Err("no page layout".into());
        };
        let (aw, ah) = (f64::from(area[2]) / k, f64::from(area[3]) / k);
        let (ox, oy) = (
            f64::from(origin.0 - area[0]) / k,
            f64::from(origin.1 - area[1]) / k,
        );
        let edge = 1.0 / k;
        let mut commands = vec![Command::FillRect {
            x: 0.0,
            y: 0.0,
            w: aw as f32,
            h: ah as f32,
            color: LAYOUT_BG,
        }];
        let mut visible = Vec::new();
        for i in 0..pl.pages.len() {
            let (px, py) = pl.page_pos(i);
            let (x, y) = (ox + px, oy + py);
            let s = pl.pages[i].size;
            if x + s.0 < 0.0 || y + s.1 < 0.0 || x > aw || y > ah {
                continue;
            }
            visible.push((i, x, y, s));
        }
        let ids: Vec<usize> = visible.iter().map(|v| v.0).collect();
        let lists = self
            .engine
            .page_layout_lists(self.sheet, &ids)
            .map_err(|e| e.to_string())?;
        for (i, x, y, s) in visible {
            let f = |v: f64| v as f32;
            commands.push(Command::FillRect {
                x: f(x - edge),
                y: f(y - edge),
                w: f(s.0 + 2.0 * edge),
                h: f(s.1 + 2.0 * edge),
                color: PAGE_EDGE,
            });
            commands.push(Command::FillRect {
                x: f(x),
                y: f(y),
                w: f(s.0),
                h: f(s.1),
                color: Rgba(255, 255, 255, 255),
            });
            if let Some((_, list)) = lists.iter().find(|l| l.0 == i) {
                commands.push(Command::PushClip {
                    x: f(x),
                    y: f(y),
                    w: f(s.0),
                    h: f(s.1),
                });
                commands.push(Command::PushTransform {
                    m: [1.0, 0.0, 0.0, 1.0, f(x), f(y)],
                });
                commands.extend(list.commands.iter().cloned());
                commands.push(Command::PopTransform);
                commands.push(Command::PopClip);
            }
        }
        Ok(PageList {
            width: aw as f32,
            height: ah as f32,
            commands,
        })
    }

    /// Scroll bar figures of the page layout view (position, content, view; points).
    /// In the page layout view: the page the active cell is printed on (1-based) and the
    /// number of pages, for the status bar's "Page: 1 of 6".
    pub fn layout_page_of_active(&self) -> Option<(usize, usize)> {
        let pl = self.layout.as_ref()?;
        let at = CellRange::single(CellRef::new(self.active.0, self.active.1));
        let i = (0..pl.pages.len()).find(|&i| pl.cells_rect(i, at).is_some())?;
        Some((i + 1, pl.pages.len()))
    }

    pub(crate) fn layout_scroll_info(&self, vertical: bool) -> (f64, f64, f64) {
        let k = f64::from(self.k());
        let (area, _) = self.layout_origin();
        let Some(pl) = self.layout.as_ref() else {
            return (0.0, 1.0, 1.0);
        };
        let pad = f64::from(self.u(40.0)) / k;
        if vertical {
            let view = f64::from(area[3]) / k;
            (
                pl.scroll.1,
                (pl.extent.1 + pad).max(pl.scroll.1 + view),
                view,
            )
        } else {
            let view = f64::from(area[2]) / k;
            (
                pl.scroll.0,
                (pl.extent.0 + pad).max(pl.scroll.0 + view),
                view,
            )
        }
    }

    /// Scroll the page layout view to `pos` points.
    pub(crate) fn layout_scroll_to(&mut self, vertical: bool, pos: f64) -> bool {
        let (_, content, view) = self.layout_scroll_info(vertical);
        let Some(pl) = self.layout.as_mut() else {
            return false;
        };
        let pos = pos.clamp(0.0, (content - view).max(0.0));
        let s = if vertical {
            &mut pl.scroll.1
        } else {
            &mut pl.scroll.0
        };
        if (*s - pos).abs() < 1e-6 {
            return false;
        }
        *s = pos;
        self.grid_dirty = true;
        self.full_redraw = true;
        true
    }

    /// Scroll so the active cell's page rectangle is in view.
    pub(crate) fn layout_reveal(&mut self) {
        let k = f64::from(self.k());
        let (area, _) = self.layout_origin();
        let (r, c) = self.active;
        let Some(pl) = self.layout.as_ref() else {
            return;
        };
        let at = CellRange::single(CellRef::new(r, c));
        let Some((x0, y0, x1, y1)) = (0..pl.pages.len()).find_map(|i| {
            let [a, b, cc, d] = pl.cells_rect(i, at)?;
            let (px, py) = pl.page_pos(i);
            Some((px + a, py + b, px + cc, py + d))
        }) else {
            return;
        };
        let (vw, vh) = (f64::from(area[2]) / k, f64::from(area[3]) / k);
        let mut s = pl.scroll;
        let pad = f64::from(self.u(20.0)) / k;
        if x0 < s.0 {
            s.0 = (x0 - pad).max(0.0);
        } else if x1 + pad > s.0 + vw {
            s.0 = x1 + pad - vw;
        }
        if y0 < s.1 {
            s.1 = (y0 - pad).max(0.0);
        } else if y1 + pad > s.1 + vh {
            s.1 = y1 + pad - vh;
        }
        if s != pl.scroll {
            if let Some(pl) = self.layout.as_mut() {
                pl.scroll = s;
            }
            self.grid_dirty = true;
            self.full_redraw = true;
        }
    }

    /// Press in the page layout view: select the cell under the pointer.
    pub(crate) fn layout_press(&mut self, x: f32, y: f32, shift: bool) -> bool {
        let k = f64::from(self.k());
        let (area, origin) = self.layout_origin();
        if x < area[0] || y < area[1] {
            return false;
        }
        let lx = f64::from(x - origin.0) / k;
        let ly = f64::from(y - origin.1) / k;
        let Some((_, Some(cell))) = self.layout.as_ref().and_then(|pl| pl.hit(lx, ly)) else {
            return false;
        };
        if self.edit.is_some() && !self.commit() {
            return true;
        }
        if shift {
            self.extent = cell;
        } else {
            self.anchor = cell;
            self.extent = cell;
        }
        self.active = if shift { self.active } else { cell };
        self.refresh();
        true
    }

    /// Ruler, column letters, row numbers, selection and header hint of the page layout view.
    pub(crate) fn paint_layout(&self, p: &mut dyn Painter) {
        let Some(pl) = self.layout.as_ref() else {
            return;
        };
        let [fx, fy, fw, fh] = self.frame;
        let k = f64::from(self.k());
        let (area, origin) = self.layout_origin();
        p.push_clip(fx, fy, fw, fh);
        p.fill_rect(fx, fy, fw, fh, LAYOUT_BG);
        p.page(0, area[0] as i32, area[1] as i32);
        let wx = |lx: f64| origin.0 + (lx * k) as f32;
        let wy = |ly: f64| origin.1 + (ly * k) as f32;
        let (r0, c0, r1, c1) = self.range_bounds();
        let sel = CellRange::new(CellRef::new(r0, c0), CellRef::new(r1, c1));
        let font = Font {
            family: &self.header_font.0,
            size: (self.header_font.1 as f32 * self.k()).max(1.0),
            weight: 400,
            italic: false,
            underline: false,
        };
        let small = Font {
            size: (7.5 * self.scale * 4.0 / 3.0).round(),
            ..font
        };
        let line_h = (font.size * 1.22).round();
        // Selection frame on every page that prints part of it.
        p.push_clip(area[0], area[1], area[2], area[3]);
        let w2 = (2.0 * self.scale / 1.25).round().max(2.0);
        for i in 0..pl.pages.len() {
            let (px, py) = pl.page_pos(i);
            if let Some([a, b, c, d]) = pl.cells_rect(i, sel) {
                let (l, t, r, bt) = (
                    wx(px + a).round(),
                    wy(py + b).round(),
                    wx(px + c).round(),
                    wy(py + d).round(),
                );
                p.fill_rect(l - w2 / 2.0, t - w2 / 2.0, r - l + w2, w2, SEL_GREEN);
                p.fill_rect(l - w2 / 2.0, bt - w2 / 2.0, r - l + w2, w2, SEL_GREEN);
                p.fill_rect(l - w2 / 2.0, t - w2 / 2.0, w2, bt - t + w2, SEL_GREEN);
                p.fill_rect(r - w2 / 2.0, t - w2 / 2.0, w2, bt - t + w2, SEL_GREEN);
            }
        }
        // "Add header" in the top margin of the active cell's page.
        let (ar, ac) = self.active;
        if let Some(i) = (0..pl.pages.len()).find(|&i| {
            pl.cells_rect(i, CellRange::single(CellRef::new(ar, ac)))
                .is_some()
        }) {
            let (px, py) = pl.page_pos(i);
            let pg = &pl.pages[i];
            let hint = Font {
                size: (font.size * 1.6).round(),
                ..font
            };
            let label = self.page_labels.1.as_str();
            let w = self.label_w(label, &hint);
            let cx = wx(px + pg.size.0 / 2.0);
            let ty = wy(py + f64::midpoint(pg.margins[2], pg.margins[4])) - hint.size * 0.6;
            p.text_font(label, (cx - w / 2.0).round(), ty.round(), &hint, HINT);
        }
        p.pop_clip();
        // Bands: ruler and column letters above, ruler and row numbers on the left.
        let (lb, tb) = self.layout_bands();
        p.fill_rect(fx, fy, fw, tb, LAYOUT_BG);
        p.fill_rect(fx, fy, lb, fh, LAYOUT_BG);
        let unit = if pl.inches { 72.0 } else { 72.0 / 2.54 };
        let ticks = if pl.inches { 8 } else { 4 };
        // Top ruler and column letters, one per page column.
        let ry0 = fy + self.u(10.0);
        let rh = self.u(22.0);
        let hy0 = fy + self.u(40.0);
        let hh = self.u(25.0);
        p.push_clip(area[0], fy, area[2], tb);
        let mut seen = Vec::new();
        for (i, pg) in pl.pages.iter().enumerate() {
            if seen.contains(&pg.at.0) {
                continue;
            }
            seen.push(pg.at.0);
            let (px, _) = pl.page_pos(i);
            let (x0, x1) = (wx(px).round(), wx(px + pg.size.0).round());
            if x1 < area[0] || x0 > area[0] + area[2] {
                continue;
            }
            p.fill_rect(x0, ry0, x1 - x0, 1.0, RULER_EDGE);
            p.fill_rect(x0, ry0 + 1.0, x1 - x0, rh - 1.0, RULER_MARGIN);
            let (ml, mr) = (pg.margins[0], pg.margins[1]);
            let (m0, m1) = (wx(px + ml).round(), wx(px + pg.size.0 - mr).round());
            p.fill_rect(m0, ry0 + 1.0, m1 - m0, rh - 1.0, LAYOUT_BG);
            if let Some(c) = &pg.cells {
                for col in c.cols.0..=c.cols.1 {
                    let j = (col - c.cols.0) as usize;
                    let a = wx(px + c.origin.0 + c.xs[j] * c.scale).round();
                    let b = wx(px + c.origin.0 + c.xs[j + 1] * c.scale).round();
                    let on = col >= c0 && col <= c1;
                    if on {
                        p.fill_rect(a, hy0, b - a, hh, HEADER_SEL);
                        p.fill_rect(a, ry0 + 1.0, b - a, rh - 1.0, Rgba(0xbd, 0xdc, 0xcb, 255));
                    }
                    p.fill_rect(b, hy0, 1.0, hh, HEADER_LINE);
                    if on {
                        p.fill_rect(a, hy0 + hh - w2, b - a, w2, SEL_GREEN);
                    }
                    let label = col_to_letters(col);
                    let lw = self.label_w(&label, &font);
                    p.text_font(
                        &label,
                        ((a + b - lw) / 2.0).round(),
                        (hy0 + (hh - line_h) / 2.0).round(),
                        &font,
                        if on { SEL_GREEN } else { Rgba(0, 0, 0, 255) },
                    );
                }
            }
            // Ticks from the left margin: numbers every unit, marks every 1/ticks.
            let step = unit / f64::from(ticks);
            let n_left = (ml / step).floor() as i32;
            let n_right = ((pg.size.0 - ml) / step).floor() as i32;
            for q in -n_left..=n_right {
                let x = wx(px + ml + f64::from(q) * step).round();
                if q % ticks == 0 {
                    if q != 0 {
                        let s = q.abs() / ticks;
                        let label = s.to_string();
                        let lw = self.label_w(&label, &small);
                        p.text_font(
                            &label,
                            (x - lw / 2.0).round(),
                            ry0 + self.u(2.0),
                            &small,
                            RULER_TEXT,
                        );
                    }
                } else {
                    let len = if q % (ticks / 2) == 0 {
                        self.u(5.0)
                    } else {
                        self.u(3.0)
                    };
                    p.fill_rect(x, ry0 + rh - len - self.u(1.0), 1.0, len, RULER_TICK);
                }
            }
        }
        p.pop_clip();
        // Left ruler and row numbers, one per page row.
        let rx0 = fx + self.u(9.0);
        let rw = self.u(23.0);
        let hx0 = fx + self.u(42.0);
        let hw = self.u(34.0);
        p.push_clip(fx, area[1], lb, area[3]);
        let mut seen = Vec::new();
        for (i, pg) in pl.pages.iter().enumerate() {
            if seen.contains(&pg.at.1) {
                continue;
            }
            seen.push(pg.at.1);
            let (_, py) = pl.page_pos(i);
            let (y0, y1) = (wy(py).round(), wy(py + pg.size.1).round());
            if y1 < area[1] || y0 > area[1] + area[3] {
                continue;
            }
            p.fill_rect(rx0, y0, 1.0, y1 - y0, RULER_EDGE);
            p.fill_rect(rx0 + 1.0, y0, rw - 1.0, y1 - y0, RULER_MARGIN);
            let (mt, mb) = (pg.margins[2], pg.margins[3]);
            let (m0, m1) = (wy(py + mt).round(), wy(py + pg.size.1 - mb).round());
            p.fill_rect(rx0 + 1.0, m0, rw - 1.0, m1 - m0, LAYOUT_BG);
            if let Some(c) = &pg.cells {
                for row in c.rows.0..=c.rows.1 {
                    let j = (row - c.rows.0) as usize;
                    let a = wy(py + c.origin.1 + c.ys[j] * c.scale).round();
                    let b = wy(py + c.origin.1 + c.ys[j + 1] * c.scale).round();
                    if b < area[1] || a > area[1] + area[3] {
                        continue;
                    }
                    let on = row >= r0 && row <= r1;
                    p.fill_rect(
                        hx0,
                        a,
                        hw,
                        b - a,
                        if on {
                            HEADER_SEL
                        } else {
                            Rgba(255, 255, 255, 255)
                        },
                    );
                    p.fill_rect(hx0, b, hw, 1.0, HEADER_LINE);
                    p.fill_rect(hx0 + hw, a, 1.0, b - a, HEADER_LINE);
                    if on {
                        p.fill_rect(hx0 + hw - w2 + 1.0, a, w2, b - a, SEL_GREEN);
                        p.fill_rect(rx0 + 1.0, a, rw - 1.0, b - a, Rgba(0xbd, 0xdc, 0xcb, 255));
                    }
                    let label = (row + 1).to_string();
                    let lw = self.label_w(&label, &font);
                    p.text_font(
                        &label,
                        ((hx0 * 2.0 + hw - lw) / 2.0).round(),
                        ((a + b - line_h) / 2.0).round(),
                        &font,
                        if on { SEL_GREEN } else { Rgba(0, 0, 0, 255) },
                    );
                }
            }
            let step = unit / f64::from(ticks);
            let n = (pg.size.1 / step).floor() as i32;
            let first = -((mt / step).floor() as i32);
            for q in first..=n + first {
                let y = wy(py + mt + f64::from(q) * step).round();
                if q % ticks != 0 {
                    let len = if q % (ticks / 2) == 0 {
                        self.u(5.0)
                    } else {
                        self.u(3.0)
                    };
                    p.fill_rect(rx0 + rw - len - self.u(1.0), y, len, 1.0, RULER_TICK);
                }
            }
        }
        p.pop_clip();
        // Corner mark.
        p.fill_rect(fx, fy, lb, tb.min(self.u(30.0)), LAYOUT_BG);
        p.pop_clip();
    }

    /// Page Break Preview overlay: print area frames, page breaks and page numbers.
    pub(crate) fn paint_breaks(&self, p: &mut dyn Painter) {
        let Some(areas) = self.breaks.as_ref() else {
            return;
        };
        let (gx, gy) = self.grid_origin();
        let [fx, fy, fw, fh] = self.frame;
        p.push_clip(gx, gy, fx + fw - gx, fy + fh - gy);
        // Window x of the gridline before column `c` / row `r` (left / top edge).
        let edge = |spans: &[super::Span], i: u32| -> Option<f32> {
            spans.iter().find(|s| s.index == i).map(|s| s.start - 1.0)
        };
        let end = |spans: &[super::Span], i: u32| -> Option<f32> {
            spans.iter().find(|s| s.index == i).map(|s| s.line)
        };
        let (right, bottom) = (fx + fw, fy + fh);
        let x_of = |c: u32, last: bool| {
            if last {
                end(&self.cols, c).or_else(|| {
                    (self.cols.last().is_some_and(|s| s.index < c)).then_some(right + 4.0)
                })
            } else {
                edge(&self.cols, c).or_else(|| {
                    (self.cols.first().is_some_and(|s| s.index > c)).then_some(gx - 4.0)
                })
            }
        };
        let y_of = |r: u32, last: bool| {
            if last {
                end(&self.rows, r).or_else(|| {
                    (self.rows.last().is_some_and(|s| s.index < r)).then_some(bottom + 4.0)
                })
            } else {
                edge(&self.rows, r).or_else(|| {
                    (self.rows.first().is_some_and(|s| s.index > r)).then_some(gy - 4.0)
                })
            }
        };
        let w2 = self.u(2.0);
        let w3 = self.u(3.0);
        let (on, off) = (self.u(9.0), self.u(3.0));
        for a in areas {
            let (Some(l), Some(t), Some(r), Some(b)) = (
                x_of(a.range.first.col, false),
                y_of(a.range.first.row, false),
                x_of(a.range.last.col, true),
                y_of(a.range.last.row, true),
            ) else {
                continue;
            };
            // Page numbers, under the lines.
            for &(ci, ri, n) in &a.pages {
                let (Some(cs), Some(rs)) = (a.cols.get(ci), a.rows.get(ri)) else {
                    continue;
                };
                let (Some(x0), Some(y0), Some(x1), Some(y1)) = (
                    x_of(cs.0, false),
                    y_of(rs.0, false),
                    x_of(cs.1, true),
                    y_of(rs.1, true),
                ) else {
                    continue;
                };
                let label = self.page_labels.0.replace("{n}", &n.to_string());
                // About a third of the page width, measured; never taller than the page.
                let size = ((x1 - x0) * 0.36 / 2.9).min((y1 - y0) * 0.3).max(6.0);
                let font = Font {
                    family: &self.header_font.0,
                    size,
                    weight: 400,
                    italic: false,
                    underline: false,
                };
                let lw = label.chars().count() as f32 * size * 0.48;
                p.text_font(
                    &label,
                    ((x0 + x1 - lw) / 2.0).round(),
                    (f32::midpoint(y0, y1) - size * 0.62).round(),
                    &font,
                    PAGE_LABEL,
                );
            }
            // Breaks inside the area.
            for &(at, manual) in &a.col_breaks {
                if let Some(x) = x_of(at, false) {
                    let x = (x - w2 / 2.0 + 0.5).round();
                    if manual {
                        p.fill_rect(x, t, w2, b - t, BREAK_BLUE);
                    } else {
                        let mut y = t;
                        while y < b {
                            p.fill_rect(x, y, w2, on.min(b - y), BREAK_BLUE);
                            y += on + off;
                        }
                    }
                }
            }
            for &(at, manual) in &a.row_breaks {
                if let Some(y) = y_of(at, false) {
                    let y = (y - w2 / 2.0 + 0.5).round();
                    if manual {
                        p.fill_rect(l, y, r - l, w2, BREAK_BLUE);
                    } else {
                        let mut x = l;
                        while x < r {
                            p.fill_rect(x, y, on.min(r - x), w2, BREAK_BLUE);
                            x += on + off;
                        }
                    }
                }
            }
            // The print area's frame.
            p.fill_rect(l - 1.0, t - 1.0, r - l + w3, w3, BREAK_BLUE);
            p.fill_rect(l - 1.0, b - 1.0, r - l + w3, w3, BREAK_BLUE);
            p.fill_rect(l - 1.0, t - 1.0, w3, b - t + w3, BREAK_BLUE);
            p.fill_rect(r - 1.0, t - 1.0, w3, b - t + w3, BREAK_BLUE);
        }
        p.pop_clip();
    }

    /// A page break under the pointer in Page Break Preview: (area, rows, first row /
    /// column of the next page).
    pub(crate) fn break_at(&self, x: f32, y: f32) -> Option<(usize, bool, u32)> {
        let areas = self.breaks.as_ref()?;
        let near = self.u(3.0);
        for (ai, a) in areas.iter().enumerate() {
            for &(at, _) in &a.col_breaks {
                if let Some(s) = self.cols.iter().find(|s| s.index == at) {
                    if (x - (s.start - 1.0)).abs() <= near {
                        return Some((ai, false, at));
                    }
                }
            }
            for &(at, _) in &a.row_breaks {
                if let Some(s) = self.rows.iter().find(|s| s.index == at) {
                    if (y - (s.start - 1.0)).abs() <= near {
                        return Some((ai, true, at));
                    }
                }
            }
        }
        None
    }

    /// Drop a dragged page break at the pointer (engine `movePageBreak`, one undo step).
    pub(crate) fn drop_break(
        &mut self,
        area: usize,
        rows: bool,
        from: u32,
        x: f32,
        y: f32,
    ) -> bool {
        let (r, c) = self.cell_at(x, y);
        // The break goes before the cell whose near edge is closest to the pointer.
        let to = if rows {
            self.rows
                .iter()
                .min_by(|a, b| (a.start - y).abs().total_cmp(&(b.start - y).abs()))
                .map_or(r, |s| s.index)
        } else {
            self.cols
                .iter()
                .min_by(|a, b| (a.start - x).abs().total_cmp(&(b.start - x).abs()))
                .map_or(c, |s| s.index)
        };
        if to == from {
            return false;
        }
        // A view command (not a `SheetCommand`): through the JSON bridge.
        let ok = self
            .query(json!({
                "cmd": "movePageBreak", "rows": rows, "area": area, "from": from, "to": to,
            }))
            .is_some();
        if ok {
            self.changed();
        }
        ok
    }
}

#[cfg(test)]
impl SheetView {
    /// Window centre of cell (r, c) on its page in the page layout view.
    pub(crate) fn layout_cell_center(&self, r: u32, c: u32) -> Option<(f32, f32)> {
        let pl = self.layout.as_ref()?;
        let k = f64::from(self.k());
        let (_, origin) = self.layout_origin();
        let at = CellRange::single(CellRef::new(r, c));
        (0..pl.pages.len()).find_map(|i| {
            let [a, b, cc, d] = pl.cells_rect(i, at)?;
            let (px, py) = pl.page_pos(i);
            Some((
                origin.0 + ((px + f64::midpoint(a, cc)) * k) as f32,
                origin.1 + ((py + f64::midpoint(b, d)) * k) as f32,
            ))
        })
    }
}
