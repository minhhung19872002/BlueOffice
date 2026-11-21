//! BlueReader document area: pages of a PDF laid out in one of four page layouts, zoomed and
//! rotated, drawn from GPU page images kept by the host ([`PdfStore`]); text selection, search
//! hits and links come from each page's text layer.
use bo_pdfread::{Link, LinkTarget, PdfDocument, RenderOptions, TextLayer};
use bo_render::{Command, PageList, Rgba};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

/// Page images kept by a backend.
pub trait PdfStore {
    /// Draw `list` at `scale` device pixels per point into image `key`; `images` resolves the
    /// list's picture ids and `fonts` its faces.
    fn update(
        &mut self,
        key: usize,
        list: &PageList,
        fonts: &bo_text::FontManager,
        images: &HashMap<String, Arc<bo_pdfread::Image>>,
        scale: f32,
    ) -> Result<(), String>;
    /// Draw only `band` (left, top, width, height in device pixels) of image `key`, which was
    /// drawn before at `scale` (BlueEditor's typing); stores without bands draw it all.
    fn update_band(
        &mut self,
        key: usize,
        list: &PageList,
        fonts: &bo_text::FontManager,
        images: &HashMap<String, Arc<bo_pdfread::Image>>,
        scale: f32,
        band: [u32; 4],
    ) -> Result<(), String> {
        let _ = band;
        self.update(key, list, fonts, images, scale)
    }
    /// Forget image `key`.
    fn remove(&mut self, key: usize);
}

/// Page cache budget (display lists, text, decoded images), bytes. 12 MB since round 8: after
/// 80 wheel notches and 40 × PgDn through the 1 202-page manual 14 MB less private memory than
/// 32 MB, same latency.
const CACHE_BYTES: usize = 12 * 1024 * 1024;

/// Memory settings for measuring (round 8): `BO_READER_CACHE_MB` (page cache budget) and
/// `BO_READER_NEIGHBOURS` (0: none, 1: the next page only, 2: one each way; default 3: the
/// page ahead in the direction of the last scroll).
fn mem_setting(name: &str, default: usize) -> usize {
    std::env::var(name)
        .ok()
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(default)
}

fn cache_budget() -> usize {
    static V: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *V.get_or_init(|| mem_setting("BO_READER_CACHE_MB", CACHE_BYTES >> 20) << 20)
}

fn neighbours() -> usize {
    static V: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *V.get_or_init(|| mem_setting("BO_READER_NEIGHBOURS", 3))
}

/// Key of page `i`'s thumbnail image in the store.
pub const THUMB_KEY: usize = 1 << 24;
/// Key of the one preview image (print preview) in the store.
pub const PREVIEW_KEY: usize = 2 << 24;

/// How pages are arranged.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layout {
    /// One page at a time.
    Single,
    /// All pages in a column.
    Continuous,
    /// Two pages side by side, one spread at a time.
    Facing,
    /// Spreads in a column.
    ContinuousFacing,
}

/// How the zoom follows the window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fit {
    /// A fixed zoom.
    None,
    /// The whole page fits.
    Page,
    /// The page width fits.
    Width,
}

/// Zoom steps of the zoom-in / zoom-out commands and Ctrl + wheel (fractions of 100 %): the
/// reference reader's preset list (measured, round 5).
pub const ZOOM_STEPS: [f32; 18] = [
    0.01, 0.0833, 0.125, 0.25, 0.3333, 0.5, 0.6667, 0.75, 1.0, 1.25, 1.5, 2.0, 4.0, 6.0, 8.0, 16.0,
    32.0, 64.0,
];
const MIN_ZOOM: f32 = 0.01;
const MAX_ZOOM: f32 = 64.0;

/// A rendered page kept for drawing, selection and search.
struct Rendered {
    list: PageList,
    images: HashMap<String, Arc<bo_pdfread::Image>>,
    text: TextLayer,
    links: Vec<Link>,
    /// Glyphs drawn as outlines (the text engine refused a font).
    paths: bool,
}

/// A text selection: page and first / last character.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Selection {
    /// Page index.
    pub page: usize,
    /// First character (content order).
    pub a: usize,
    /// Last character.
    pub b: usize,
}

/// One search hit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hit {
    /// Page index.
    pub page: usize,
    /// First character.
    pub a: usize,
    /// Last character.
    pub b: usize,
}

/// A search reading the document page by page: from the current page to the end, then from
/// the first page, so the first hit after the current page shows before the rest is read.
#[derive(Debug, Clone)]
struct SearchJob {
    query: String,
    match_case: bool,
    whole_word: bool,
    start: usize,
    /// Pages read so far (in search order).
    done: usize,
}

/// The document view.
pub struct ReaderView {
    doc: PdfDocument,
    /// Page sizes in points after the document's and the view's rotation.
    sizes: Vec<(f32, f32)>,
    rotation: u16,
    zoom: f32,
    fit: Fit,
    layout: Layout,
    /// View rectangle in window pixels.
    frame: [f32; 4],
    /// Device pixels per logical pixel.
    dpi: f32,
    scroll: (f32, f32),
    /// Page rectangles in content pixels (x, y, w, h) and the content size.
    rects: Vec<[f32; 4]>,
    content: (f32, f32),
    cache: HashMap<usize, Rendered>,
    lru: VecDeque<usize>,
    /// Images in the store: key → (scale, generation).
    drawn: HashMap<usize, (f32, u64)>,
    /// Page content changed (annotation added): images older than this are stale.
    generation: Vec<u64>,
    /// Pages whose content changed only inside a box (page points): drawn again by band.
    bands: HashMap<usize, [f32; 4]>,
    selection: Option<Selection>,
    hits: Vec<Hit>,
    current_hit: Option<usize>,
    /// A search still reading pages (continued by [`ReaderView::search_step`]).
    search: Option<SearchJob>,
    /// Pages shown by thumbnails (index, scale) wanted this frame.
    thumbs_wanted: Vec<(usize, f32)>,
    /// Layout pass needed.
    stale: bool,
    /// Gap between pages and the margin around them (logical pixels).
    gap: f32,
    /// Spreads start with a single first page (cover) in facing layouts.
    cover: bool,
    /// Night Mode: pages drawn in the dark scheme (dark paper, grey text).
    night: bool,
    /// BlueEditor's selected text drawn into a page image (`editor_selhi9`).
    pub(crate) text_mark9: Option<(usize, Vec<[f32; 4]>)>,
    /// Scroll position of the last `prepare` and the direction scrolled (up: true), for the
    /// page drawn ahead.
    prepared_y: f32,
    scrolled_up: bool,
    /// Reverse View: spreads read right to left.
    reverse: bool,
    /// First page of the spread shown in single and facing layouts.
    single_page: usize,
    /// Pages too large to keep whole at this zoom: the part drawn (device pixels from the
    /// page's top-left corner: x, y, width, height).
    crops: HashMap<usize, [f32; 4]>,
    /// Preview image wanted this frame (page, scale) and the one drawn (page, scale, generation).
    preview_wanted: Option<(usize, f32)>,
    preview_drawn: Option<(usize, f32, u64)>,
    /// Print preview images wanted (page, scale) and drawn (page, scale, generation).
    previews_wanted: Vec<(usize, f32)>,
    previews_drawn: Vec<Option<(usize, f32, u64)>>,
    /// Other panes of a split window (frame, scroll): their pages are kept drawn too.
    extra_views: Vec<([f32; 4], (f32, f32))>,
    /// Form fields drawn on this background (Highlight Fields, BlueReader round 11).
    field_tint: Option<bo_render::Rgba>,
}

/// Largest page image side and area (device pixels) drawn whole; larger pages at high zoom
/// draw only the part around the view.
const MAX_IMAGE_SIDE: f32 = 8192.0;
const MAX_IMAGE_AREA: f32 = 32.0 * 1024.0 * 1024.0;

/// Points to device pixels at 100 % zoom and a scale of 1 (96 / 72).
const PT: f32 = 96.0 / 72.0;

impl ReaderView {
    /// View `doc`.
    pub fn new(doc: PdfDocument) -> Self {
        let n = doc.page_count();
        let mut v = Self {
            doc,
            sizes: Vec::new(),
            rotation: 0,
            zoom: 1.0,
            fit: Fit::Width,
            layout: if n == 1 {
                Layout::Single
            } else {
                Layout::Continuous
            },
            frame: [0.0, 0.0, 800.0, 600.0],
            dpi: 1.0,
            scroll: (0.0, 0.0),
            rects: Vec::new(),
            content: (0.0, 0.0),
            cache: HashMap::new(),
            lru: VecDeque::new(),
            drawn: HashMap::new(),
            generation: vec![0; n],
            bands: HashMap::new(),
            selection: None,
            hits: Vec::new(),
            current_hit: None,
            search: None,
            thumbs_wanted: Vec::new(),
            stale: true,
            gap: 6.0,
            cover: true,
            night: false,
            field_tint: None,
            text_mark9: None,
            prepared_y: 0.0,
            scrolled_up: false,
            reverse: false,
            single_page: 0,
            crops: HashMap::new(),
            preview_wanted: None,
            preview_drawn: None,
            previews_wanted: Vec::new(),
            previews_drawn: Vec::new(),
            extra_views: Vec::new(),
        };
        v.update_sizes();
        v
    }

    /// The document.
    pub fn doc(&self) -> &PdfDocument {
        &self.doc
    }

    /// The document, for comments and saving.
    pub fn doc_mut(&mut self) -> &mut PdfDocument {
        &mut self.doc
    }

    /// Number of pages.
    pub fn page_count(&self) -> usize {
        self.sizes.len()
    }

    fn update_sizes(&mut self) {
        self.sizes = (0..self.doc.page_count())
            .map(|i| {
                let info = self.doc.page_info(i);
                let (w, h) = info.map_or((612.0, 792.0), |p| (p.width, p.height));
                if self.rotation % 180 == 90 {
                    (h, w)
                } else {
                    (w, h)
                }
            })
            .collect();
        self.stale = true;
    }

    /// Page size in points as shown.
    pub fn page_size(&self, i: usize) -> (f32, f32) {
        self.sizes.get(i).copied().unwrap_or((612.0, 792.0))
    }

    /// Device pixels per point.
    pub fn px_per_pt(&self) -> f32 {
        self.zoom * PT * self.dpi
    }

    /// Zoom (1 = 100 %).
    pub fn zoom(&self) -> f32 {
        self.zoom
    }

    /// Fit mode.
    pub fn fit(&self) -> Fit {
        self.fit
    }

    /// Page layout.
    pub fn layout(&self) -> Layout {
        self.layout
    }

    /// View rotation in degrees.
    pub fn rotation(&self) -> u16 {
        self.rotation
    }

    /// Set the view rectangle (window pixels) and the display scale.
    pub fn set_frame(&mut self, frame: [f32; 4], dpi: f32) {
        if self.frame != frame || (self.dpi - dpi).abs() > 1e-4 {
            let anchor = self.anchor();
            self.frame = frame;
            self.dpi = dpi;
            self.stale = true;
            self.relayout();
            self.restore_anchor(anchor);
        }
    }

    /// View rectangle.
    pub fn frame(&self) -> [f32; 4] {
        self.frame
    }

    /// Gap between pages and around them in logical pixels (6 normally; Full Screen uses 1).
    pub fn set_gap(&mut self, gap: f32) {
        if (self.gap - gap).abs() > f32::EPSILON {
            self.gap = gap.max(0.0);
            self.stale = true;
        }
    }

    fn margin(&self) -> f32 {
        (self.gap * self.dpi).round()
    }

    /// Pages in each row of the layout.
    fn rows(&self) -> Vec<Vec<usize>> {
        let n = self.sizes.len();
        match self.layout {
            Layout::Single | Layout::Continuous => (0..n).map(|i| vec![i]).collect(),
            Layout::Facing | Layout::ContinuousFacing => {
                let mut rows = Vec::new();
                let mut i = 0;
                if self.cover && n > 0 {
                    rows.push(vec![0]);
                    i = 1;
                }
                while i < n {
                    if i + 1 < n {
                        rows.push(if self.reverse {
                            vec![i + 1, i]
                        } else {
                            vec![i, i + 1]
                        });
                    } else {
                        rows.push(vec![i]);
                    }
                    i += 2;
                }
                rows
            }
        }
    }

    /// The row holding page `i`.
    fn row_of(&self, i: usize) -> Vec<usize> {
        self.rows()
            .into_iter()
            .find(|r| r.contains(&i))
            .unwrap_or_else(|| vec![i])
    }

    fn fit_zoom(&mut self) {
        if self.fit == Fit::None || self.sizes.is_empty() {
            return;
        }
        let m = self.margin();
        let [_, _, fw, fh] = self.frame;
        let cur = self.current_page();
        let row = self.row_of(cur);
        let row_pt = |r: &[usize]| r.iter().map(|&i| self.page_size(i).0).sum::<f32>();
        // Fit width fits the widest row of the document (a landscape page among portrait
        // ones sets the zoom, like the reference reader); fit page fits the current row.
        let (w, gaps) = if self.fit == Fit::Width {
            self.rows()
                .iter()
                .map(|r| (row_pt(r), r.len().saturating_sub(1)))
                .fold((0.0_f32, 0), |a, b| if b.0 > a.0 { b } else { a })
        } else {
            (row_pt(&row), row.len().saturating_sub(1))
        };
        let h = row
            .iter()
            .map(|&i| self.page_size(i).1)
            .fold(0.0_f32, f32::max);
        let per_pt = PT * self.dpi;
        // 125 %: the widest page spans the view less 7.5 px on each side.
        let mm = self.gap * self.dpi;
        let zw = (fw - 2.0 * mm - gaps as f32 * m) / (w * per_pt);
        let zh = (fh - 2.0 * m - 2.0) / (h * per_pt);
        let z = match self.fit {
            Fit::Width => zw,
            Fit::Page => zw.min(zh),
            Fit::None => self.zoom,
        };
        if z.is_finite() && z > 0.0 {
            let z = z.clamp(MIN_ZOOM, MAX_ZOOM);
            // Round 14: a fitted zoom that changes (window resized, first layout) rebuilds the
            // lists like `set_zoom`: NoZoom notes and hairlines depend on the zoom (a note
            // sized at the first, smaller window's zoom was drawn 1.4 × too large).
            if (z - self.zoom).abs() > self.zoom * 1e-3 {
                self.cache_reset_lines();
            }
            self.zoom = z;
        }
    }

    /// Lay the pages out for the current zoom, layout and frame.
    fn relayout(&mut self) {
        if !self.stale {
            return;
        }
        self.stale = false;
        self.fit_zoom();
        let s = self.px_per_pt();
        let m = self.margin();
        let fw = self.frame[2];
        let rows = self.rows();
        let mut rects = vec![[0.0_f32; 4]; self.sizes.len()];
        let mut y = m;
        let mut max_w = 0.0_f32;
        let row_w = |r: &Vec<usize>| -> f32 {
            r.iter()
                .map(|&i| (self.page_size(i).0 * s).round())
                .sum::<f32>()
                + (r.len().saturating_sub(1)) as f32 * m
        };
        for r in &rows {
            max_w = max_w.max(row_w(r));
        }
        let content_w = (max_w + 2.0 * m).max(fw);
        for r in &rows {
            let w = row_w(r);
            let h = r
                .iter()
                .map(|&i| (self.page_size(i).1 * s).round())
                .fold(0.0_f32, f32::max);
            let mut x = ((content_w - w) / 2.0).round();
            for &i in r {
                let (pw, ph) = (
                    (self.page_size(i).0 * s).round(),
                    (self.page_size(i).1 * s).round(),
                );
                rects[i] = [x, y + ((h - ph) / 2.0).round(), pw, ph];
                x += pw + m;
            }
            y += h + m;
        }
        self.rects = rects;
        self.content = (content_w, y);
        self.clamp_scroll();
    }

    fn clamp_scroll(&mut self) {
        let [_, _, fw, fh] = self.frame;
        let (lo, hi) = self.scroll_range_y();
        self.scroll.1 = self.scroll.1.clamp(lo, hi.max(lo));
        self.scroll.0 = self.scroll.0.clamp(0.0, (self.content.0 - fw).max(0.0));
        let _ = fh;
    }

    /// Vertical scroll range: the whole document, or the current spread in single layouts.
    fn scroll_range_y(&self) -> (f32, f32) {
        let fh = self.frame[3];
        match self.layout {
            Layout::Continuous | Layout::ContinuousFacing => (0.0, (self.content.1 - fh).max(0.0)),
            Layout::Single | Layout::Facing => {
                let cur = self.single_page;
                let row = self.row_of(cur);
                let top = row
                    .iter()
                    .filter_map(|&i| self.rects.get(i))
                    .map(|r| r[1])
                    .fold(f32::INFINITY, f32::min);
                let bottom = row
                    .iter()
                    .filter_map(|&i| self.rects.get(i))
                    .map(|r| r[1] + r[3])
                    .fold(0.0_f32, f32::max);
                if !top.is_finite() {
                    return (0.0, 0.0);
                }
                let m = self.margin();
                let lo = top - m;
                let hi = (bottom + m - fh).max(lo);
                (lo, hi)
            }
        }
    }

    /// Content y of the view top and the page it falls on, to keep the place on relayout.
    fn anchor(&self) -> (usize, f32) {
        // At the very top: stay at the top (the margin above the first page shows).
        if self.scroll.1 <= 0.5
            && matches!(self.layout, Layout::Continuous | Layout::ContinuousFacing)
        {
            return (0, f32::NEG_INFINITY);
        }
        let p = self.current_page();
        let r = self.rects.get(p).copied().unwrap_or([0.0; 4]);
        let frac = if r[3] > 0.0 {
            (self.scroll.1 - r[1]) / r[3]
        } else {
            0.0
        };
        (p, frac)
    }

    fn restore_anchor(&mut self, (p, frac): (usize, f32)) {
        if frac == f32::NEG_INFINITY {
            self.scroll.1 = 0.0;
            self.clamp_scroll();
            return;
        }
        if let Some(r) = self.rects.get(p).copied() {
            self.scroll.1 = r[1] + frac * r[3];
            self.clamp_scroll();
        }
    }

    /// The page shown in single layouts, or the one at the view's centre line.
    pub fn current_page(&self) -> usize {
        if matches!(self.layout, Layout::Single | Layout::Facing) {
            return self.single_page;
        }
        let mid = self.scroll.1 + self.frame[3] * 0.25;
        let mut best = 0;
        for (i, r) in self.rects.iter().enumerate() {
            if r[1] <= mid {
                best = i;
            } else {
                break;
            }
        }
        // Facing rows: the left page of the row.
        best
    }

    /// Set the zoom (fraction of 100 %), keeping the view centre.
    pub fn set_zoom(&mut self, z: f32) {
        let z = z.clamp(MIN_ZOOM, MAX_ZOOM);
        let anchor = self.anchor();
        let cx = (self.scroll.0 + self.frame[2] / 2.0) / self.content.0.max(1.0);
        self.fit = Fit::None;
        self.zoom = z;
        self.stale = true;
        self.relayout();
        self.restore_anchor(anchor);
        self.scroll.0 = (cx * self.content.0 - self.frame[2] / 2.0).max(0.0);
        self.clamp_scroll();
        self.cache_reset_lines();
    }

    /// Next zoom step in or out.
    pub fn step_zoom(&mut self, dir: i32) {
        let z = self.zoom;
        let next = if dir > 0 {
            ZOOM_STEPS.iter().copied().find(|s| *s > z * 1.001)
        } else {
            ZOOM_STEPS.iter().rev().copied().find(|s| *s < z * 0.999)
        };
        if let Some(n) = next {
            self.set_zoom(n);
        }
    }

    /// Follow the window: fit the page or its width.
    pub fn set_fit(&mut self, fit: Fit) {
        let anchor = self.anchor();
        self.fit = fit;
        self.stale = true;
        self.relayout();
        self.restore_anchor(anchor);
        if fit == Fit::Page {
            self.go_to(self.current_page(), None);
        }
        self.cache_reset_lines();
    }

    /// Change the page layout.
    pub fn set_layout(&mut self, layout: Layout) {
        let page = self.current_page();
        self.layout = layout;
        self.single_page = page;
        self.stale = true;
        self.relayout();
        self.go_to(page, None);
    }

    /// Rotate the view by ±90°.
    pub fn rotate(&mut self, clockwise: bool) {
        let page = self.current_page();
        self.rotation = (self.rotation + if clockwise { 90 } else { 270 }) % 360;
        self.update_sizes();
        self.cache.clear();
        self.lru.clear();
        self.relayout();
        self.go_to(page, None);
    }

    fn cache_reset_lines(&mut self) {
        // Hairlines are one device pixel at the zoom they are drawn at: lists are rebuilt.
        self.cache.clear();
        self.lru.clear();
    }

    /// Show page `i` (at `top` points from its top).
    pub fn go_to(&mut self, i: usize, top: Option<f32>) {
        if i >= self.sizes.len() {
            return;
        }
        self.relayout();
        if matches!(self.layout, Layout::Single | Layout::Facing) {
            self.single_page = self.row_of(i).first().copied().unwrap_or(i);
            if self.fit == Fit::Page {
                self.stale = true;
                self.relayout();
            }
        }
        let r = self.rects[i];
        let m = self.margin();
        let y = match top {
            Some(t) => r[1] + t * self.px_per_pt() - m,
            None => r[1] - m,
        };
        self.scroll.1 = y;
        self.clamp_scroll();
    }

    /// Scroll by device pixels; in single layouts scrolling past the end turns the page.
    pub fn scroll_by(&mut self, dx: f32, dy: f32) {
        self.relayout();
        let (lo, hi) = self.scroll_range_y();
        let y = self.scroll.1 + dy;
        if matches!(self.layout, Layout::Single | Layout::Facing) {
            let row = self.row_of(self.single_page);
            if y > hi + 1.0 && dy > 0.0 && self.scroll.1 >= hi - 0.5 {
                let next = row.last().copied().unwrap_or(0) + 1;
                if next < self.sizes.len() {
                    self.go_to(next, None);
                    return;
                }
            } else if y < lo - 1.0 && dy < 0.0 && self.scroll.1 <= lo + 0.5 {
                if let Some(prev) = row.first().copied().filter(|p| *p > 0) {
                    self.go_to(prev - 1, None);
                    let (_, hi) = self.scroll_range_y();
                    self.scroll.1 = hi;
                    return;
                }
            }
        }
        self.scroll.1 = y;
        self.scroll.0 += dx;
        self.clamp_scroll();
    }

    /// Scroll position and content size (device pixels).
    pub fn scroll_state(&self) -> ((f32, f32), (f32, f32)) {
        (self.scroll, self.content)
    }

    /// Set the vertical scroll position (scroll bar drags).
    pub fn set_scroll_y(&mut self, y: f32) {
        self.scroll.1 = y;
        self.clamp_scroll();
    }

    /// Visible range of the vertical scroll (top, bottom) for the scroll bar.
    pub fn scroll_range(&self) -> (f32, f32) {
        self.scroll_range_y()
    }

    /// Page rectangle in window pixels.
    pub fn page_rect(&self, i: usize) -> Option<[f32; 4]> {
        let r = self.rects.get(i)?;
        Some([
            self.frame[0] + r[0] - self.scroll.0,
            self.frame[1] + r[1] - self.scroll.1,
            r[2],
            r[3],
        ])
    }

    /// Page rectangle in window pixels in another pane (`frame`, scrolled to `scroll`) of
    /// the same size as the view.
    pub fn page_rect_at(&self, i: usize, frame: [f32; 4], scroll: (f32, f32)) -> Option<[f32; 4]> {
        let r = self.rects.get(i)?;
        Some([
            frame[0] + r[0] - scroll.0,
            frame[1] + r[1] - scroll.1,
            r[2],
            r[3],
        ])
    }

    /// Pages intersecting another pane (`frame`, scrolled to `scroll`).
    pub fn visible_pages_at(&self, frame: [f32; 4], scroll: (f32, f32)) -> Vec<usize> {
        let [fx, fy, fw, fh] = frame;
        let shown: Vec<usize> = match self.layout {
            Layout::Single | Layout::Facing => self.row_of(self.single_page),
            _ => (0..self.rects.len()).collect(),
        };
        shown
            .into_iter()
            .filter(|&i| {
                self.page_rect_at(i, frame, scroll).is_some_and(|r| {
                    r[1] < fy + fh && r[1] + r[3] > fy && r[0] < fx + fw && r[0] + r[2] > fx
                })
            })
            .collect()
    }

    /// Other panes of a split window (frame, scroll); their pages stay drawn.
    pub fn set_extra_views(&mut self, views: Vec<([f32; 4], (f32, f32))>) {
        self.extra_views = views;
    }

    /// Scroll to `scroll` (switching panes of a split window).
    pub fn set_scroll(&mut self, scroll: (f32, f32)) {
        self.scroll = scroll;
        self.clamp_scroll();
    }

    /// Pages intersecting the view.
    pub fn visible_pages(&self) -> Vec<usize> {
        let [fx, fy, fw, fh] = self.frame;
        let shown: Vec<usize> = match self.layout {
            Layout::Single | Layout::Facing => self.row_of(self.single_page),
            _ => (0..self.rects.len()).collect(),
        };
        shown
            .into_iter()
            .filter(|&i| {
                self.page_rect(i).is_some_and(|r| {
                    r[1] < fy + fh && r[1] + r[3] > fy && r[0] < fx + fw && r[0] + r[2] > fx
                })
            })
            .collect()
    }

    /// Page under a window point, with the point in page points.
    pub fn page_at(&self, x: f32, y: f32) -> Option<(usize, f32, f32)> {
        let s = self.px_per_pt();
        for i in self.visible_pages() {
            let r = self.page_rect(i)?;
            if x >= r[0] && x < r[0] + r[2] && y >= r[1] && y < r[1] + r[3] {
                return Some((i, (x - r[0]) / s, (y - r[1]) / s));
            }
        }
        None
    }

    /// A rectangle of page `i` in the document's page points (left, top, right, bottom) as a
    /// window rectangle (x, y, w, h), with the view's rotation.
    pub fn doc_rect_to_window(&self, i: usize, r: [f32; 4]) -> Option<[f32; 4]> {
        let pr = self.page_rect(i)?;
        let shown = rotate_rect(r, self.rotation, self.unrotated(i));
        let s = self.px_per_pt();
        Some([
            pr[0] + shown[0] * s,
            pr[1] + shown[1] * s,
            (shown[2] - shown[0]) * s,
            (shown[3] - shown[1]) * s,
        ])
    }

    /// The page under a window point and the point in the document's page points.
    pub fn window_to_doc(&self, x: f32, y: f32) -> Option<(usize, f32, f32)> {
        let (i, px, py) = self.page_at(x, y)?;
        let r = self.unrotate_rect(i, [px, py, px, py]);
        Some((i, r[0], r[1]))
    }

    /// Window pixels per document point.
    pub fn scale(&self) -> f32 {
        self.px_per_pt()
    }

    fn render_opts(&self) -> RenderOptions {
        RenderOptions {
            text_as_paths: false,
            min_line: 1.0 / self.px_per_pt().max(0.01),
            annotations: true,
        }
    }

    /// Make sure page `i` is rendered into a display list.
    fn ensure(&mut self, i: usize) -> bool {
        if self.cache.contains_key(&i) {
            if let Some(p) = self.lru.iter().position(|&k| k == i) {
                self.lru.remove(p);
            }
            self.lru.push_back(i);
            return true;
        }
        let opts = self.render_opts();
        let page = match self.doc.render_extra(i, opts, self.render_extra()) {
            Ok(p) => p,
            Err(e) => {
                draw_log(&format!("render page {i}: {e}"));
                return false;
            }
        };
        let list = self.with_text_mark9(i, page.list);
        let list = rotate_list(list, self.rotation);
        let list = if self.night {
            crate::reader_viewx::night_list(list)
        } else {
            list
        };
        let links = page
            .links
            .into_iter()
            .map(|mut l| {
                l.rect = rotate_rect(l.rect, self.rotation, self.unrotated(i));
                l
            })
            .collect();
        let chars = page
            .chars
            .into_iter()
            .map(|mut c| {
                c.rect = rotate_rect(c.rect, self.rotation, self.unrotated(i));
                let o = rotate_point(c.origin, self.rotation, self.unrotated(i));
                let d = rotate_vec(c.dir, self.rotation);
                c.origin = o;
                c.dir = d;
                c
            })
            .collect();
        self.cache.insert(
            i,
            Rendered {
                list,
                images: page.images,
                text: TextLayer::new(chars),
                links,
                paths: false,
            },
        );
        self.lru.retain(|&k| k != i);
        self.lru.push_back(i);
        // At most 48 pages and the cache budget of display lists, text and images (the least
        // recently used go first; the pages on screen are always the most recent).
        while self.lru.len() > 48 || (self.lru.len() > 6 && self.cache_bytes() > cache_budget()) {
            if let Some(old) = self.lru.pop_front() {
                self.cache.remove(&old);
            }
        }
        true
    }

    /// Estimated bytes of the page cache.
    fn cache_bytes(&self) -> usize {
        self.cache
            .values()
            .map(|r| {
                r.list.commands.len() * std::mem::size_of::<Command>()
                    + r.text.len() * std::mem::size_of::<bo_pdfread::TextChar>()
                    + r.images.values().map(|i| i.pixels.len()).sum::<usize>()
            })
            .sum()
    }

    /// Night Mode greys every shown text, outlined text included.
    fn render_extra(&self) -> bo_pdfread::RenderExtra {
        bo_pdfread::RenderExtra {
            text_color: self.night.then_some(crate::reader_viewx::NIGHT_TEXT),
            // Sticky notes keep one point per interface pixel whatever the zoom, as the
            // reference reader draws them (16 pt note: 20 px at 125 %, round 10).
            no_zoom: 1.0 / (self.zoom * PT).max(0.01),
            field_tint: self.field_tint,
            ..bo_pdfread::RenderExtra::default()
        }
    }

    fn unrotated(&self, i: usize) -> (f32, f32) {
        let info = self.doc.page_info(i);
        info.map_or((612.0, 792.0), |p| (p.width, p.height))
    }

    /// Re-render page `i` with glyphs as outlines (after the text engine refused a font).
    fn render_paths(&mut self, i: usize) -> bool {
        let mut opts = self.render_opts();
        opts.text_as_paths = true;
        let page = match self.doc.render_extra(i, opts, self.render_extra()) {
            Ok(p) => p,
            Err(e) => {
                draw_log(&format!("render page {i}: {e}"));
                return false;
            }
        };
        let night = self.night;
        if let Some(r) = self.cache.get_mut(&i) {
            r.list = rotate_list(page.list, self.rotation);
            if night {
                r.list = crate::reader_viewx::night_list(std::mem::replace(
                    &mut r.list,
                    PageList {
                        width: 0.0,
                        height: 0.0,
                        commands: Vec::new(),
                    },
                ));
            }
            r.images = page.images;
            r.paths = true;
        }
        true
    }

    /// Bring page images up to date: visible pages first, then neighbours and thumbnails while
    /// within `budget_ms`. Returns true when more work remains (call again next frame).
    pub fn prepare(&mut self, store: &mut dyn PdfStore, budget_ms: u32) -> bool {
        self.relayout();
        let start = std::time::Instant::now();
        let scale = self.px_per_pt();
        let mut visible = self.visible_pages();
        for &(frame, scroll) in &self.extra_views {
            for i in self.visible_pages_at(frame, scroll) {
                if !visible.contains(&i) {
                    visible.push(i);
                }
            }
        }
        let mut more = false;
        let mut wanted: Vec<usize> = visible.clone();
        // Neighbours for smooth scrolling (one each way keeps memory near the reference's).
        if let (Some(&lo), Some(&hi)) = (visible.iter().min(), visible.iter().max()) {
            // One page ahead in the scroll direction keeps scrolling smooth with one page
            // image less than one each way (8 MB at 107 % on Letter; round 8).
            let n = neighbours();
            let y = self.scroll.1;
            if (y - self.prepared_y).abs() > 0.5 {
                self.scrolled_up = y < self.prepared_y;
                self.prepared_y = y;
            }
            let (next, prev) = match n {
                0 => (false, false),
                1 => (true, false),
                2 => (true, true),
                _ => (!self.scrolled_up, self.scrolled_up),
            };
            if next && hi + 1 < self.sizes.len() {
                wanted.push(hi + 1);
            }
            if prev && lo >= 1 {
                wanted.push(lo - 1);
            }
        }
        for (n, &i) in wanted.iter().enumerate() {
            let crop = self.crop_for(i, scale);
            let covered = match (crop, self.crops.get(&i)) {
                (None, None) => true,
                (Some(_), Some(have)) => self.visible_part(i).is_none_or(|v| {
                    v[0] >= have[0]
                        && v[1] >= have[1]
                        && v[0] + v[2] <= have[0] + have[2]
                        && v[1] + v[3] <= have[1] + have[3]
                }),
                _ => false,
            };
            let fresh = covered
                && self
                    .drawn
                    .get(&i)
                    .is_some_and(|&(s, g)| (s - scale).abs() < 1e-4 && g == self.generation[i]);
            if fresh {
                continue;
            }
            if n >= visible.len() && start.elapsed().as_millis() > u128::from(budget_ms) {
                more = true;
                break;
            }
            if !self.ensure(i) {
                continue;
            }
            let band = self.bands.remove(&i);
            let same_scale = self
                .drawn
                .get(&i)
                .is_some_and(|&(s, _)| (s - scale).abs() < 1e-4);
            let same_crop = crop == self.crops.get(&i).copied();
            let drawn = match (crop, band) {
                (_, Some(b)) if same_scale && same_crop => self.draw_band(store, i, scale, b, crop),
                (None, _) => self.draw(store, i, i, scale),
                (Some(c), _) => self.draw_part(store, i, scale, c),
            };
            if let Err(e) = &drawn {
                draw_log(&format!("draw page {i}: {e}"));
            }
            if drawn.is_ok() {
                self.drawn.insert(i, (scale, self.generation[i]));
                match crop {
                    Some(c) => self.crops.insert(i, c),
                    None => self.crops.remove(&i),
                };
            }
        }
        // Print preview.
        if let Some((i, s)) = self.preview_wanted.take() {
            if i < self.sizes.len()
                && !self.has_preview(i, s)
                && self.ensure(i)
                && self.draw(store, PREVIEW_KEY, i, s).is_ok()
            {
                self.preview_drawn = Some((i, s, self.generation[i]));
            }
        }
        // Print preview pages of the sheet shown.
        let previews = std::mem::take(&mut self.previews_wanted);
        if self.previews_drawn.len() < previews.len() {
            self.previews_drawn.resize(previews.len(), None);
        }
        for (k, &(i, s)) in previews.iter().enumerate() {
            let fresh = i < self.sizes.len()
                && self.previews_drawn[k].is_some_and(|(p, ds, g)| {
                    p == i && (ds - s).abs() < 1e-4 && g == self.generation[i]
                });
            if fresh || i >= self.sizes.len() {
                continue;
            }
            if self.ensure(i) && self.draw(store, PREVIEW_KEY + 1 + k, i, s).is_ok() {
                self.previews_drawn[k] = Some((i, s, self.generation[i]));
            }
        }
        // Thumbnails.
        let thumbs = std::mem::take(&mut self.thumbs_wanted);
        for (i, s) in thumbs {
            let key = THUMB_KEY + i;
            let fresh = self
                .drawn
                .get(&key)
                .is_some_and(|&(ds, g)| (ds - s).abs() < 1e-4 && g == self.generation[i]);
            if fresh {
                continue;
            }
            if start.elapsed().as_millis() > u128::from(budget_ms) {
                more = true;
                break;
            }
            if self.ensure(i) && self.draw(store, key, i, s).is_ok() {
                self.drawn.insert(key, (s, self.generation[i]));
            }
        }
        // Forget page images far from the view.
        let keep: Vec<usize> = wanted;
        let drop: Vec<usize> = self
            .drawn
            .keys()
            .copied()
            .filter(|k| *k < THUMB_KEY && !keep.contains(k))
            .collect();
        for k in drop {
            store.remove(k);
            self.drawn.remove(&k);
            self.crops.remove(&k);
        }
        let thumbs: Vec<usize> = self
            .drawn
            .keys()
            .copied()
            .filter(|k| *k >= THUMB_KEY)
            .collect();
        if thumbs.len() > 200 {
            for k in thumbs.into_iter().take(100) {
                store.remove(k);
                self.drawn.remove(&k);
            }
        }
        more
    }

    /// Pages held as display lists (round 14 tests: a new fitted zoom rebuilds them).
    pub fn cached_lists(&self) -> usize {
        self.cache.len()
    }

    /// Every image key this view has in the store (pages, thumbnails, previews), so a closed
    /// tab frees them (BlueReader round 14).
    pub fn stored_keys(&self) -> Vec<usize> {
        let mut v: Vec<usize> = self.drawn.keys().copied().collect();
        if self.preview_drawn.is_some() {
            v.push(PREVIEW_KEY);
        }
        v.extend(
            self.previews_drawn
                .iter()
                .enumerate()
                .filter(|(_, d)| d.is_some())
                .map(|(k, _)| PREVIEW_KEY + 1 + k),
        );
        v
    }

    /// Ask for the print preview's page images this frame: `(page, scale)` drawn under keys
    /// [`PREVIEW_KEY`] + 1 + their position.
    pub fn want_previews(&mut self, wanted: &[(usize, f32)]) {
        self.previews_wanted = wanted.to_vec();
    }

    /// Whether preview image `k` (key [`PREVIEW_KEY`] + 1 + `k`) shows page `i` at `scale`
    /// (BlueReader round 13: the Loupe and Magnifier draw from it).
    pub fn preview_ready(&self, k: usize, i: usize, scale: f32) -> bool {
        self.previews_drawn
            .get(k)
            .copied()
            .flatten()
            .is_some_and(|(p, s, g)| {
                p == i && (s - scale).abs() < 1e-4 && self.generation.get(i) == Some(&g)
            })
    }

    /// Ask for the preview image of page `i` at `scale` (key [`PREVIEW_KEY`]) this frame.
    pub fn want_preview(&mut self, i: usize, scale: f32) {
        self.preview_wanted = Some((i, scale));
    }

    /// Whether the preview image shows page `i` at `scale`.
    pub fn has_preview(&self, i: usize, scale: f32) -> bool {
        self.preview_drawn
            .is_some_and(|(p, s, g)| p == i && (s - scale).abs() < 1e-4 && g == self.generation[i])
    }

    /// The part of page `i` in the view (device pixels from the page's corner).
    fn visible_part(&self, i: usize) -> Option<[f32; 4]> {
        let r = self.page_rect(i)?;
        let f = self.frame;
        let x0 = (f[0].max(r[0]) - r[0]).max(0.0);
        let y0 = (f[1].max(r[1]) - r[1]).max(0.0);
        let x1 = (f[0] + f[2]).min(r[0] + r[2]) - r[0];
        let y1 = (f[1] + f[3]).min(r[1] + r[3]) - r[1];
        (x1 > x0 && y1 > y0).then_some([x0, y0, x1 - x0, y1 - y0])
    }

    /// For a page too large to draw whole at `scale`: the part to draw, the view's part grown
    /// by half a view each way.
    fn crop_for(&self, i: usize, scale: f32) -> Option<[f32; 4]> {
        let (w, h) = self.sizes.get(i).map(|s| (s.0 * scale, s.1 * scale))?;
        if w <= MAX_IMAGE_SIDE && h <= MAX_IMAGE_SIDE && w * h <= MAX_IMAGE_AREA {
            return None;
        }
        let v = self
            .visible_part(i)
            .unwrap_or([0.0, 0.0, self.frame[2], self.frame[3]]);
        let (mx, my) = ((self.frame[2] / 2.0).round(), (self.frame[3] / 2.0).round());
        let x0 = (v[0] - mx).max(0.0).floor();
        let y0 = (v[1] - my).max(0.0).floor();
        let x1 = (v[0] + v[2] + mx).min(w.floor());
        let y1 = (v[1] + v[3] + my).min(h.floor());
        Some([x0, y0, (x1 - x0).max(1.0), (y1 - y0).max(1.0)])
    }

    /// Where image `key`'s top-left corner lies on its page (device pixels): the drawn part
    /// of a page too large to draw whole, else (0, 0).
    pub fn image_offset(&self, key: usize) -> (f32, f32) {
        self.crops.get(&key).map_or((0.0, 0.0), |c| (c[0], c[1]))
    }

    /// Draw part `c` (device pixels) of page `i` as its image.
    fn draw_part(
        &mut self,
        store: &mut dyn PdfStore,
        i: usize,
        scale: f32,
        c: [f32; 4],
    ) -> Result<(), String> {
        let r = self.cache.get(&i).ok_or("not rendered")?;
        let mut commands = Vec::with_capacity(r.list.commands.len() + 2);
        commands.push(Command::PushTransform {
            m: [1.0, 0.0, 0.0, 1.0, -c[0] / scale, -c[1] / scale],
        });
        commands.extend(r.list.commands.iter().cloned());
        commands.push(Command::PopTransform);
        let part = PageList {
            width: c[2] / scale,
            height: c[3] / scale,
            commands,
        };
        store.update(i, &part, self.doc.fonts(), &r.images, scale)
    }

    fn draw_band(
        &mut self,
        store: &mut dyn PdfStore,
        i: usize,
        scale: f32,
        b: [f32; 4],
        crop: Option<[f32; 4]>,
    ) -> Result<(), String> {
        let r = self.cache.get(&i).ok_or("not rendered")?;
        // The image is the page, or the `crop` part of it (device pixels).
        let c = crop.unwrap_or([
            0.0,
            0.0,
            (r.list.width * scale).ceil(),
            (r.list.height * scale).ceil(),
        ]);
        let x0 = (b[0] * scale - 2.0 - c[0]).floor().clamp(0.0, c[2]);
        let y0 = (b[1] * scale - 2.0 - c[1]).floor().clamp(0.0, c[3]);
        let x1 = (b[2] * scale + 2.0 - c[0]).ceil().clamp(0.0, c[2]);
        let y1 = (b[3] * scale + 2.0 - c[1]).ceil().clamp(0.0, c[3]);
        if x1 <= x0 || y1 <= y0 {
            return Ok(());
        }
        let band = [x0 as u32, y0 as u32, (x1 - x0) as u32, (y1 - y0) as u32];
        if crop.is_none() {
            return store.update_band(i, &r.list, self.doc.fonts(), &r.images, scale, band);
        }
        let mut commands = Vec::with_capacity(r.list.commands.len() + 2);
        commands.push(Command::PushTransform {
            m: [1.0, 0.0, 0.0, 1.0, -c[0] / scale, -c[1] / scale],
        });
        commands.extend(r.list.commands.iter().cloned());
        commands.push(Command::PopTransform);
        let part = PageList {
            width: c[2] / scale,
            height: c[3] / scale,
            commands,
        };
        store.update_band(i, &part, self.doc.fonts(), &r.images, scale, band)
    }

    fn draw(
        &mut self,
        store: &mut dyn PdfStore,
        key: usize,
        i: usize,
        scale: f32,
    ) -> Result<(), String> {
        let r = self.cache.get(&i).ok_or("not rendered")?;
        let first = store.update(key, &r.list, self.doc.fonts(), &r.images, scale);
        if first.is_ok() || r.paths {
            return first;
        }
        // The text engine refused a font: draw the glyphs as outlines.
        if !self.render_paths(i) {
            return first;
        }
        let r = self.cache.get(&i).ok_or("not rendered")?;
        store.update(key, &r.list, self.doc.fonts(), &r.images, scale)
    }

    /// Whether page image `key` is ready at the current scale.
    pub fn has_image(&self, key: usize) -> bool {
        self.drawn.contains_key(&key)
    }

    /// Ask for the thumbnail of page `i` at `scale` device pixels per point this frame.
    pub fn want_thumb(&mut self, i: usize, scale: f32) {
        self.thumbs_wanted.push((i, scale));
    }

    /// Text layer of a rendered page.
    pub fn text(&mut self, i: usize) -> Option<&TextLayer> {
        if !self.ensure(i) {
            return None;
        }
        self.cache.get(&i).map(|r| &r.text)
    }

    /// Text layer if the page is already rendered.
    pub fn text_if_ready(&self, i: usize) -> Option<&TextLayer> {
        self.cache.get(&i).map(|r| &r.text)
    }

    /// Link at a point of page `i` (page points).
    pub fn link_at(&self, i: usize, x: f32, y: f32) -> Option<LinkTarget> {
        let r = self.cache.get(&i)?;
        r.links
            .iter()
            .find(|l| x >= l.rect[0] && x <= l.rect[2] && y >= l.rect[1] && y <= l.rect[3])
            .map(|l| l.target.clone())
    }

    /// The selection.
    pub fn selection(&self) -> Option<Selection> {
        self.selection
    }

    /// Set or clear the selection.
    pub fn set_selection(&mut self, s: Option<Selection>) {
        self.selection = s;
    }

    /// Selected text.
    pub fn selected_text(&self) -> Option<String> {
        let s = self.selection?;
        let t = self.cache.get(&s.page)?;
        Some(t.text.slice(s.a, s.b))
    }

    /// Selection rectangles in page points.
    pub fn selection_rects(&self) -> Vec<[f32; 4]> {
        let Some(s) = self.selection else {
            return Vec::new();
        };
        self.cache
            .get(&s.page)
            .map(|r| r.text.rects(s.a, s.b))
            .unwrap_or_default()
    }

    /// Find `query` in every page; returns the number of hits and shows the first one after
    /// the current page.
    pub fn find(&mut self, query: &str, match_case: bool, whole_word: bool) -> usize {
        self.find_start(query, match_case, whole_word);
        while self.search_step(std::time::Duration::from_secs(3600)) {}
        self.hits.len()
    }

    /// Start finding `query`: pages are read from the current page on until the first hit is
    /// shown (or `FIRST_BUDGET` passes); [`ReaderView::search_step`] reads the rest, a slice
    /// at a time, so a long document stays responsive. Pages read for the search are not kept
    /// in the page cache (the pages on screen stay drawn).
    pub fn find_start(&mut self, query: &str, match_case: bool, whole_word: bool) {
        const FIRST_BUDGET: std::time::Duration = std::time::Duration::from_millis(250);
        self.hits.clear();
        self.current_hit = None;
        self.search = None;
        if query.trim().is_empty() {
            return;
        }
        self.search = Some(SearchJob {
            query: query.to_owned(),
            match_case,
            whole_word,
            start: self.current_page(),
            done: 0,
        });
        let t0 = std::time::Instant::now();
        while self.current_hit.is_none() && t0.elapsed() < FIRST_BUDGET {
            if !self.search_step(std::time::Duration::ZERO) {
                break;
            }
        }
    }

    /// Whether a search is still reading pages.
    pub fn searching(&self) -> bool {
        self.search.is_some()
    }

    /// Search progress: pages read and page count.
    pub fn search_progress(&self) -> Option<(usize, usize)> {
        self.search.as_ref().map(|j| (j.done, self.sizes.len()))
    }

    /// Read pages of the running search for about `budget` (at least one page); `true` while
    /// pages remain.
    pub fn search_step(&mut self, budget: std::time::Duration) -> bool {
        let t0 = std::time::Instant::now();
        let n = self.sizes.len();
        loop {
            let Some(job) = self.search.as_ref() else {
                return false;
            };
            if job.done >= n {
                self.search = None;
                return false;
            }
            let page = (job.start + job.done) % n;
            let (query, case, whole) = (job.query.clone(), job.match_case, job.whole_word);
            let found: Vec<(usize, usize)> = match self.cache.get(&page) {
                Some(r) => r.text.find(&query, case, whole),
                None => self
                    .page_text(page)
                    .map_or_else(Vec::new, |t| t.find(&query, case, whole)),
            };
            if let Some(j) = self.search.as_mut() {
                j.done += 1;
            }
            if !found.is_empty() {
                // Hits stay in page order; pages before the start come last in search order
                // but first in the list.
                let at = self.hits.partition_point(|h| h.page < page);
                let k = found.len();
                self.hits
                    .splice(at..at, found.into_iter().map(|(a, b)| Hit { page, a, b }));
                match self.current_hit {
                    Some(c) if c >= at => self.current_hit = Some(c + k),
                    Some(_) => {}
                    None => self.show_hit(at),
                }
            }
            if t0.elapsed() >= budget {
                return self.search.is_some();
            }
        }
    }

    /// Text of page `i` (rotated like the view) without keeping it in the page cache.
    fn page_text(&mut self, i: usize) -> Option<TextLayer> {
        let opts = self.render_opts();
        let page = self.doc.render_extra(i, opts, self.render_extra()).ok()?;
        let chars = page
            .chars
            .into_iter()
            .map(|mut c| {
                c.rect = rotate_rect(c.rect, self.rotation, self.unrotated(i));
                c.origin = rotate_point(c.origin, self.rotation, self.unrotated(i));
                c.dir = rotate_vec(c.dir, self.rotation);
                c
            })
            .collect();
        Some(TextLayer::new(chars))
    }

    /// Search hits.
    pub fn hits(&self) -> &[Hit] {
        &self.hits
    }

    /// Index of the current hit.
    pub fn current_hit(&self) -> Option<usize> {
        self.current_hit
    }

    /// Move to the next (`forward`) or previous hit.
    pub fn next_hit(&mut self, forward: bool) {
        if self.hits.is_empty() {
            return;
        }
        let n = self.hits.len();
        let k = match self.current_hit {
            Some(k) if forward => (k + 1) % n,
            Some(k) => (k + n - 1) % n,
            None => 0,
        };
        self.show_hit(k);
    }

    fn show_hit(&mut self, k: usize) {
        let Some(h) = self.hits.get(k).copied() else {
            return;
        };
        self.current_hit = Some(k);
        // The hit's page text (a search does not keep the pages it reads).
        self.ensure(h.page);
        self.selection = Some(Selection {
            page: h.page,
            a: h.a,
            b: h.b,
        });
        let rect = self
            .cache
            .get(&h.page)
            .and_then(|r| r.text.rects(h.a, h.b).first().copied());
        // Bring the hit into view, about a third from the top.
        let visible = self.page_rect(h.page).is_some_and(|pr| {
            rect.is_some_and(|r| {
                let s = self.px_per_pt();
                let y = pr[1] + r[1] * s;
                y > self.frame[1] + 20.0 && y < self.frame[1] + self.frame[3] - 40.0
            })
        });
        if !visible {
            let top = rect.map(|r| (r[1] - self.frame[3] / self.px_per_pt() / 3.0).max(0.0));
            self.go_to(h.page, top);
        }
    }

    /// Clear search hits.
    pub fn clear_hits(&mut self) {
        self.hits.clear();
        self.current_hit = None;
        self.search = None;
    }

    /// Hit rectangles on page `i` (page points); the current hit flagged.
    pub fn hit_rects(&self, i: usize) -> Vec<([f32; 4], bool)> {
        let Some(r) = self.cache.get(&i) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for (k, h) in self.hits.iter().enumerate() {
            if h.page == i {
                for rect in r.text.rects(h.a, h.b) {
                    out.push((rect, self.current_hit == Some(k)));
                }
            }
        }
        out
    }

    /// Page `i` changed only inside `r` (page points): read it again and draw that band of
    /// its image again (the whole page when it is not drawn yet or the view is rotated).
    pub fn invalidate_band(&mut self, i: usize, r: [f32; 4]) {
        let drawn = self.drawn.contains_key(&i);
        self.invalidate_page(i);
        if drawn && self.rotation == 0 && !self.night {
            let b = self.bands.entry(i).or_insert(r);
            *b = [
                b[0].min(r[0]),
                b[1].min(r[1]),
                b[2].max(r[2]),
                b[3].max(r[3]),
            ];
        }
    }

    /// The page's content changed (a comment was added): draw it again.
    pub fn invalidate_page(&mut self, i: usize) {
        // Out of the recently used list too: a page changed at every keystroke (BlueEditor's
        // typing) otherwise filled the list with itself until the cache dropped it on render.
        if self.cache.remove(&i).is_some() {
            self.lru.retain(|&k| k != i);
        }
        if let Some(g) = self.generation.get_mut(i) {
            *g += 1;
        }
    }

    /// Rotate a page-point rectangle of page `i` back to the document's own orientation.
    pub fn unrotate_rect(&self, i: usize, r: [f32; 4]) -> [f32; 4] {
        let (w, h) = self.unrotated(i);
        let back = (360 - self.rotation) % 360;
        // The shown size is the rotated one; rotating back uses the shown size.
        let shown = if self.rotation % 180 == 90 {
            (h, w)
        } else {
            (w, h)
        };
        rotate_rect(r, back, shown)
    }

    /// The document's pages changed (inserted, deleted, moved, rotated, content edited, undo):
    /// forget every rendered page and lay out again.
    pub fn document_changed(&mut self) {
        let n = self.doc.page_count();
        self.cache.clear();
        self.lru.clear();
        self.drawn.clear();
        let g = self.generation.iter().copied().max().unwrap_or(0) + 1;
        self.generation = vec![g; n];
        self.selection = None;
        self.hits.clear();
        self.current_hit = None;
        self.single_page = self.single_page.min(n.saturating_sub(1));
        self.update_sizes();
        self.relayout();
        self.clamp_scroll();
    }

    /// Night Mode: pages drawn with inverted lightness (every page is drawn again).
    pub fn set_night(&mut self, on: bool) {
        if self.night == on {
            return;
        }
        self.night = on;
        self.cache.clear();
        self.lru.clear();
        for g in &mut self.generation {
            *g += 1;
        }
    }

    /// Form fields drawn on `tint` (Highlight Fields) or as they are; pages are drawn again.
    pub fn set_field_tint(&mut self, tint: Option<bo_render::Rgba>) {
        if self.field_tint == tint {
            return;
        }
        self.field_tint = tint;
        self.cache.clear();
        self.lru.clear();
        for g in &mut self.generation {
            *g += 1;
        }
    }

    /// The form field tint in use.
    pub fn field_tint(&self) -> Option<bo_render::Rgba> {
        self.field_tint
    }

    /// Whether Night Mode is on.
    pub fn night(&self) -> bool {
        self.night
    }

    /// Facing layouts show the first page alone (a cover) or pair it with the second.
    pub fn set_cover(&mut self, on: bool) {
        if self.cover != on {
            let anchor = self.anchor();
            self.cover = on;
            self.stale = true;
            self.relayout();
            self.restore_anchor(anchor);
        }
    }

    /// Reverse View: the pages of a spread right to left.
    pub fn set_reverse(&mut self, on: bool) {
        if self.reverse != on {
            self.reverse = on;
            self.stale = true;
            self.relayout();
        }
    }

    /// Whether spreads read right to left.
    pub fn reverse(&self) -> bool {
        self.reverse
    }

    /// Whether facing layouts show the first page alone.
    pub fn cover(&self) -> bool {
        self.cover
    }

    /// Whether the document was changed.
    pub fn modified(&self) -> bool {
        self.doc.modified()
    }
}

/// Wrap a page's commands in a rotation (clockwise degrees), swapping its size.
fn rotate_list(list: PageList, rotation: u16) -> PageList {
    if rotation == 0 {
        return list;
    }
    let (w, h) = (list.width, list.height);
    let m = match rotation {
        90 => [0.0, 1.0, -1.0, 0.0, h, 0.0],
        180 => [-1.0, 0.0, 0.0, -1.0, w, h],
        _ => [0.0, -1.0, 1.0, 0.0, 0.0, w],
    };
    let mut commands = Vec::with_capacity(list.commands.len() + 2);
    commands.push(Command::PushTransform { m });
    commands.extend(list.commands);
    commands.push(Command::PopTransform);
    let (nw, nh) = if rotation % 180 == 90 { (h, w) } else { (w, h) };
    PageList {
        width: nw,
        height: nh,
        commands,
    }
}

fn rotate_point(p: [f32; 2], rotation: u16, (w, h): (f32, f32)) -> [f32; 2] {
    match rotation {
        90 => [h - p[1], p[0]],
        180 => [w - p[0], h - p[1]],
        270 => [p[1], w - p[0]],
        _ => p,
    }
}

fn rotate_vec(d: [f32; 2], rotation: u16) -> [f32; 2] {
    match rotation {
        90 => [-d[1], d[0]],
        180 => [-d[0], -d[1]],
        270 => [d[1], -d[0]],
        _ => d,
    }
}

fn rotate_rect(r: [f32; 4], rotation: u16, size: (f32, f32)) -> [f32; 4] {
    let a = rotate_point([r[0], r[1]], rotation, size);
    let b = rotate_point([r[2], r[3]], rotation, size);
    [
        a[0].min(b[0]),
        a[1].min(b[1]),
        a[0].max(b[0]),
        a[1].max(b[1]),
    ]
}

/// Highlight colours of search hits and selections over page images.
pub const HIT: Rgba = Rgba(255, 214, 0, 110);
/// The current search hit.
pub const HIT_CURRENT: Rgba = Rgba(255, 140, 0, 130);
/// Selected text.
pub const SELECTION: Rgba = Rgba(0, 120, 215, 80);

/// `BLUEOFFICE_DRAW_LOG=<file>`: append page render / draw failures (diagnostics).
fn draw_log(line: &str) {
    if let Ok(path) = std::env::var("BLUEOFFICE_DRAW_LOG") {
        use std::io::Write as _;
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
        {
            let _ = writeln!(f, "{line}");
        }
    }
}

#[cfg(test)]
mod cache_tests {
    use super::*;

    #[test]
    fn page_cache_stays_within_its_budget() {
        let doc = PdfDocument::open(crate::reader_tests::pdf(60), bo_pdfread::standard_fonts())
            .expect("pdf");
        let mut v = ReaderView::new(doc);
        for i in 0..60 {
            assert!(v.ensure(i));
        }
        assert!(v.cache.len() <= 48);
        assert!(v.cache_bytes() <= CACHE_BYTES || v.cache.len() <= 6);
        // The most recent pages stay.
        assert!(v.cache.contains_key(&59) && !v.cache.contains_key(&0));
    }
}
