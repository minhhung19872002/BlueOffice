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
    /// Forget image `key`.
    fn remove(&mut self, key: usize);
}

/// Key of page `i`'s thumbnail image in the store.
pub const THUMB_KEY: usize = 1 << 24;

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

/// Zoom steps of the zoom-in / zoom-out commands (fractions of 100 %).
pub const ZOOM_STEPS: [f32; 17] = [
    0.0833, 0.125, 0.25, 0.3333, 0.5, 0.6667, 0.75, 1.0, 1.25, 1.5, 2.0, 3.0, 4.0, 6.0, 8.0, 12.0,
    16.0,
];
const MIN_ZOOM: f32 = 0.0833;
const MAX_ZOOM: f32 = 16.0;

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
    selection: Option<Selection>,
    hits: Vec<Hit>,
    current_hit: Option<usize>,
    /// Pages shown by thumbnails (index, scale) wanted this frame.
    thumbs_wanted: Vec<(usize, f32)>,
    /// Layout pass needed.
    stale: bool,
    /// Gap between pages and the margin around them (logical pixels).
    gap: f32,
    /// Spreads start with a single first page (cover) in facing layouts.
    cover: bool,
    /// First page of the spread shown in single and facing layouts.
    single_page: usize,
}

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
            selection: None,
            hits: Vec::new(),
            current_hit: None,
            thumbs_wanted: Vec::new(),
            stale: true,
            gap: 6.0,
            cover: true,
            single_page: 0,
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
                        rows.push(vec![i, i + 1]);
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
            self.zoom = z.clamp(MIN_ZOOM, MAX_ZOOM);
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
        let Ok(page) = self.doc.render(i, opts) else {
            return false;
        };
        let list = rotate_list(page.list, self.rotation);
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
        self.lru.push_back(i);
        while self.lru.len() > 48 {
            if let Some(old) = self.lru.pop_front() {
                self.cache.remove(&old);
            }
        }
        true
    }

    fn unrotated(&self, i: usize) -> (f32, f32) {
        let info = self.doc.page_info(i);
        info.map_or((612.0, 792.0), |p| (p.width, p.height))
    }

    /// Re-render page `i` with glyphs as outlines (after the text engine refused a font).
    fn render_paths(&mut self, i: usize) -> bool {
        let mut opts = self.render_opts();
        opts.text_as_paths = true;
        let Ok(page) = self.doc.render(i, opts) else {
            return false;
        };
        if let Some(r) = self.cache.get_mut(&i) {
            r.list = rotate_list(page.list, self.rotation);
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
        let visible = self.visible_pages();
        let mut more = false;
        let mut wanted: Vec<usize> = visible.clone();
        // Neighbours for smooth scrolling.
        if let (Some(&lo), Some(&hi)) = (visible.iter().min(), visible.iter().max()) {
            for k in 1..=2 {
                if hi + k < self.sizes.len() {
                    wanted.push(hi + k);
                }
                if lo >= k {
                    wanted.push(lo - k);
                }
            }
        }
        for (n, &i) in wanted.iter().enumerate() {
            let fresh = self
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
            if self.draw(store, i, i, scale).is_ok() {
                self.drawn.insert(i, (scale, self.generation[i]));
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
        self.hits.clear();
        self.current_hit = None;
        if query.trim().is_empty() {
            return 0;
        }
        let start = self.current_page();
        for i in 0..self.sizes.len() {
            if self.ensure(i) {
                if let Some(r) = self.cache.get(&i) {
                    for (a, b) in r.text.find(query, match_case, whole_word) {
                        self.hits.push(Hit { page: i, a, b });
                    }
                }
            }
        }
        if !self.hits.is_empty() {
            let k = self.hits.iter().position(|h| h.page >= start).unwrap_or(0);
            self.show_hit(k);
        }
        self.hits.len()
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

    /// The page's content changed (a comment was added): draw it again.
    pub fn invalidate_page(&mut self, i: usize) {
        self.cache.remove(&i);
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
