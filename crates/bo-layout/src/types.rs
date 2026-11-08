use bo_model::{Color, Underline};
use bo_text::FaceId;
use serde::Serialize;
use std::sync::Arc;

/// Immutable text shared between layouts: re-paginating a page clones its lines
/// without copying their text (incremental layout, prompt §5.4).
#[derive(Clone, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SharedStr(Arc<str>);

impl SharedStr {
    /// The text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::ops::Deref for SharedStr {
    type Target = str;
    fn deref(&self) -> &str {
        &self.0
    }
}

impl std::borrow::Borrow<str> for SharedStr {
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for SharedStr {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for SharedStr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(&*self.0, f)
    }
}

impl std::fmt::Display for SharedStr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<String> for SharedStr {
    fn from(s: String) -> Self {
        SharedStr(s.into())
    }
}

impl From<&str> for SharedStr {
    fn from(s: &str) -> Self {
        SharedStr(s.into())
    }
}

impl PartialEq<str> for SharedStr {
    fn eq(&self, other: &str) -> bool {
        &*self.0 == other
    }
}

impl PartialEq<&str> for SharedStr {
    fn eq(&self, other: &&str) -> bool {
        &*self.0 == *other
    }
}

impl PartialEq<String> for SharedStr {
    fn eq(&self, other: &String) -> bool {
        *self.0 == **other
    }
}

impl Serialize for SharedStr {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.0)
    }
}

/// Result of laying out a document.
#[derive(Debug, Clone, Serialize, Default)]
pub struct LayoutDocument {
    /// Laid-out pages in document order.
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
    /// Filled rectangle (shading, highlight).
    Rect {
        /// Left edge (points).
        x: f32,
        /// Top edge (points).
        y: f32,
        /// Width (points).
        w: f32,
        /// Height (points).
        h: f32,
        /// Fill colour.
        color: Color,
        /// Paint order relative to text.
        layer: Layer,
    },
    /// Straight line (borders, underlines, separators).
    Rule {
        /// Start x (points).
        x1: f32,
        /// Start y (points).
        y1: f32,
        /// End x (points).
        x2: f32,
        /// End y (points).
        y2: f32,
        /// Line width (points).
        width: f32,
        /// Line colour.
        color: Color,
        /// `single`, `double`, `dotted`, `dashed`… (border style name).
        style: String,
    },
    /// Picture drawn into a box.
    Image {
        /// Left edge of the box (points).
        x: f32,
        /// Top edge of the box (points).
        y: f32,
        /// Box width (points).
        w: f32,
        /// Box height (points).
        h: f32,
        /// Media id of the picture (e.g. `/word/media/image1.png`).
        media: String,
        /// l, t, r, b fractions.
        crop: Option<[f32; 4]>,
        /// Degrees clockwise.
        rotation: f32,
        /// Paint order relative to text.
        layer: Layer,
    },
}

/// One page (points).
#[derive(Debug, Clone, Serialize)]
pub struct Page {
    /// Page width (points).
    pub width: f32,
    /// Page height (points).
    pub height: f32,
    /// Text lines.
    pub lines: Vec<Line>,
    /// Non-text content (shading, borders, pictures).
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
    /// Paragraph `para` (document order inside the note, table cells included) of a
    /// footnote or endnote (ECMA-376 Part 1, §17.11), edited in place on the page.
    Note {
        /// `true` for an endnote, `false` for a footnote.
        endnote: bool,
        /// Note id (`@w:id`).
        id: i32,
        /// Paragraph index inside the note.
        para: u32,
    },
    /// Header/footer content and repeated table header rows (not editable in place).
    Other,
}

/// A positioned line box.
#[derive(Debug, Clone, Serialize)]
pub struct Line {
    /// Top of the line box.
    pub top: f32,
    /// Height of the line box (points).
    pub height: f32,
    /// Absolute baseline y.
    pub baseline: f32,
    /// Glyph runs of the line.
    pub fragments: Vec<Fragment>,
    /// Paragraph the line comes from.
    pub source: LineSource,
    /// Byte range of the paragraph index text covered by the line.
    pub start: u32,
    /// End (exclusive) of the covered byte range.
    pub end: u32,
    /// Caret stops: (offset in paragraph index text, absolute x), ascending by offset.
    pub carets: Vec<(u32, f32)>,
}

/// A run of glyphs sharing face, size and colour.
#[derive(Debug, Clone, Serialize)]
pub struct Fragment {
    /// Font face.
    pub face: FaceId,
    /// Font size in points.
    pub size: f32,
    /// Absolute x of the fragment origin.
    pub x: f32,
    /// Absolute baseline y.
    pub y: f32,
    /// Advance width (points).
    pub width: f32,
    /// Text colour.
    pub color: Color,
    /// Underline style.
    pub underline: Underline,
    /// Single strikethrough.
    pub strike: bool,
    /// Double strikethrough.
    pub double_strike: bool,
    /// Source text (for search, accessibility and PDF text extraction).
    pub text: SharedStr,
    /// Glyphs (shared between layouts, like `text`).
    pub glyphs: Arc<[GlyphPos]>,
    /// Hyperlink target, if any.
    pub link: Option<String>,
}

/// A glyph relative to its fragment origin (points, y down).
#[derive(Debug, Clone, Copy, Serialize)]
pub struct GlyphPos {
    /// Glyph id in the face.
    pub id: u16,
    /// X offset from the fragment origin (points).
    pub x: f32,
    /// Y offset from the fragment origin (points).
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
