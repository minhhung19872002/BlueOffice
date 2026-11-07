use bo_model::{Color, Underline};
use bo_text::FaceId;
use serde::Serialize;

/// Result of laying out a document.
#[derive(Debug, Clone, Serialize, Default)]
pub struct LayoutDocument {
    pub pages: Vec<Page>,
}

/// Paint order of a page item relative to text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Layer {
    /// Behind text (cell shading, highlights, `behindDoc` pictures).
    Back,
    /// In front of text (borders, in-front pictures).
    Front,
}

/// Non-text page content.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum PageItem {
    Rect {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        color: Color,
        layer: Layer,
    },
    Rule {
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        width: f32,
        color: Color,
        /// `single`, `double`, `dotted`, `dashed`… (border style name).
        style: String,
    },
    Image {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        media: String,
        /// l, t, r, b fractions.
        crop: Option<[f32; 4]>,
        /// Degrees clockwise.
        rotation: f32,
        layer: Layer,
    },
}

/// One page (points).
#[derive(Debug, Clone, Serialize)]
pub struct Page {
    pub width: f32,
    pub height: f32,
    pub lines: Vec<Line>,
    pub items: Vec<PageItem>,
    /// 1-based page number as displayed (PAGE field).
    pub number: u32,
    /// Section index this page belongs to.
    pub section: usize,
}

/// Where a line's text comes from (for editing).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum LineSource {
    /// Body paragraph with a document-order id (see `bo_model` paragraph indexing).
    Body(u32),
    /// Header/footer/footnote content (not editable in this version).
    Other,
}

/// A positioned line box.
#[derive(Debug, Clone, Serialize)]
pub struct Line {
    /// Top of the line box.
    pub top: f32,
    pub height: f32,
    /// Absolute baseline y.
    pub baseline: f32,
    pub fragments: Vec<Fragment>,
    pub source: LineSource,
    /// Byte range of the paragraph index text covered by the line.
    pub start: u32,
    pub end: u32,
    /// Caret stops: (offset in paragraph index text, absolute x), ascending by offset.
    pub carets: Vec<(u32, f32)>,
}

/// A run of glyphs sharing face, size and colour.
#[derive(Debug, Clone, Serialize)]
pub struct Fragment {
    pub face: FaceId,
    /// Font size in points.
    pub size: f32,
    /// Absolute x of the fragment origin.
    pub x: f32,
    /// Absolute baseline y.
    pub y: f32,
    pub width: f32,
    pub color: Color,
    pub underline: Underline,
    pub strike: bool,
    pub double_strike: bool,
    /// Source text (for search, accessibility and PDF text extraction).
    pub text: String,
    pub glyphs: Vec<GlyphPos>,
    /// Hyperlink target, if any.
    pub link: Option<String>,
}

/// A glyph relative to its fragment origin (points, y down).
#[derive(Debug, Clone, Copy, Serialize)]
pub struct GlyphPos {
    pub id: u16,
    pub x: f32,
    pub y: f32,
    /// Byte offset of the glyph's cluster in [`Fragment::text`].
    pub cluster: u32,
}

/// Footnote/endnote identity: (is endnote, note id).
pub type NoteKey = (bool, i32);

impl Line {
    /// Move the line by (`dx`, `dy`).
    pub fn translate(&mut self, dx: f32, dy: f32) {
        self.top += dy;
        self.baseline += dy;
        for f in &mut self.fragments {
            f.x += dx;
            f.y += dy;
        }
        for c in &mut self.carets {
            c.1 += dx;
        }
    }
}

impl PageItem {
    /// Move the item by (`dx`, `dy`).
    pub fn translate(&mut self, dx: f32, dy: f32) {
        match self {
            PageItem::Rect { x, y, .. } | PageItem::Image { x, y, .. } => {
                *x += dx;
                *y += dy;
            }
            PageItem::Rule { x1, y1, x2, y2, .. } => {
                *x1 += dx;
                *x2 += dx;
                *y1 += dy;
                *y2 += dy;
            }
        }
    }

    /// Paint layer (rules are always drawn in front of text).
    pub fn layer(&self) -> Layer {
        match self {
            PageItem::Rect { layer, .. } | PageItem::Image { layer, .. } => *layer,
            PageItem::Rule { .. } => Layer::Front,
        }
    }
}

impl LayoutDocument {
    /// Text of a page, lines separated by `\n` (used by tests and search).
    pub fn page_text(&self, page: usize) -> String {
        self.pages[page]
            .lines
            .iter()
            .map(|l| {
                l.fragments
                    .iter()
                    .map(|f| f.text.as_str())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Body lines of paragraph `para` with their page index.
    pub fn lines_of(&self, para: u32) -> impl Iterator<Item = (usize, &Line)> {
        self.pages.iter().enumerate().flat_map(move |(pi, p)| {
            p.lines
                .iter()
                .filter(move |l| l.source == LineSource::Body(para))
                .map(move |l| (pi, l))
        })
    }
}
