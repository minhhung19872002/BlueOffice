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
    /// DrawingML / VML shape outline (ECMA-376 Part 1, §20.1.9 geometry): `paths` are
    /// relative to the shape box at (`x`, `y`), already rotated and flipped.
    Shape {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        paths: Vec<ShapePath>,
        layer: Layer,
    },
}

/// Path segment of a shape outline (points, relative to the shape box).
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub enum PathCmd {
    M(f32, f32),
    L(f32, f32),
    Q(f32, f32, f32, f32),
    C(f32, f32, f32, f32, f32, f32),
    Z,
}

/// One filled and/or stroked path of a shape.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ShapePath {
    pub segs: Vec<PathCmd>,
    pub fill: Option<Color>,
    /// (width in points, colour, `ST_PresetLineDashVal` dash name).
    pub stroke: Option<(f32, Color, Option<String>)>,
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
    /// Paragraph `para` (document order inside the note, table cells included) of a
    /// footnote or endnote (ECMA-376 Part 1, §17.11), edited in place on the page.
    Note { endnote: bool, id: i32, para: u32 },
    /// Paragraph `sub` (document order inside the text box) of the text box anchored at
    /// index offset `at` of body paragraph `para` (§17.17.1 txbxContent); `para` is
    /// [`LineSource::NO_PARA`] for text boxes outside the body (headers, notes).
    TextBox { para: u32, at: u32, sub: u32 },
    /// Header/footer content and repeated table header rows (not editable in place).
    Other,
}

impl LineSource {
    /// `para` of text boxes that are not in the body.
    pub const NO_PARA: u32 = u32::MAX;

    /// Add `base` to a relative body paragraph id (body lines and body text boxes).
    pub fn rebase(&mut self, base: u32) {
        match self {
            LineSource::Body(id) => *id += base,
            LineSource::TextBox { para, .. } if *para != Self::NO_PARA => *para += base,
            _ => {}
        }
    }

    /// Shift a body paragraph id by `delta` (incremental layout).
    pub fn shift(&mut self, delta: i64) {
        match self {
            LineSource::Body(id) => *id = (i64::from(*id) + delta) as u32,
            LineSource::TextBox { para, .. } if *para != Self::NO_PARA => {
                *para = (i64::from(*para) + delta) as u32;
            }
            _ => {}
        }
    }

    /// Body lines become [`LineSource::Other`], body text boxes lose their paragraph.
    pub fn detach(&mut self) {
        match self {
            LineSource::Body(_) => *self = LineSource::Other,
            LineSource::TextBox { para, .. } => *para = Self::NO_PARA,
            _ => {}
        }
    }
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
    pub text: SharedStr,
    /// Glyphs (shared between layouts, like `text`).
    pub glyphs: Arc<[GlyphPos]>,
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
            PageItem::Rect { x, y, .. }
            | PageItem::Image { x, y, .. }
            | PageItem::Shape { x, y, .. } => {
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
            PageItem::Rect { layer, .. }
            | PageItem::Image { layer, .. }
            | PageItem::Shape { layer, .. } => *layer,
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
