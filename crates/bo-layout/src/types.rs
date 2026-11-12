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
        /// Opacity in 0..=1 (washed-out picture watermarks).
        alpha: f32,
    },
    /// DrawingML / VML shape outline (ECMA-376 Part 1, §20.1.9 geometry): `paths` are
    /// relative to the shape box at (`x`, `y`), already rotated and flipped.
    Shape {
        /// Left edge of the shape box on the page, in points.
        x: f32,
        /// Top edge of the shape box on the page, in points.
        y: f32,
        /// Width of the shape box, in points.
        w: f32,
        /// Height of the shape box, in points.
        h: f32,
        /// Outline paths, drawn in order.
        paths: Vec<ShapePath>,
        /// The drawing object: [`LineSource::TextBox`] with `sub` 0 (rebased like the
        /// lines of its paragraph).
        source: LineSource,
        /// Paint order relative to text.
        layer: Layer,
        /// Opacity in 0..=1 (VML fill opacity of text watermarks).
        alpha: f32,
    },
}

/// Page-level decoration drawn around the content: the page colour (ECMA-376
/// Part 1, §17.2.1 background) and the page borders (§17.6.10 pgBorders).
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct PageDecor {
    /// Page colour, painted first over the whole page.
    pub background: Option<Color>,
    /// Page border lines and art motifs.
    pub borders: Vec<PageItem>,
    /// The borders go behind the text (§17.18.65 `ST_PageBorderZOrder` back).
    pub borders_behind: bool,
}

/// Path segment of a shape outline (points, relative to the shape box).
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub enum PathCmd {
    /// Move to (x, y).
    M(f32, f32),
    /// Line to (x, y).
    L(f32, f32),
    /// Quadratic Bézier: control point (x1, y1), end (x, y).
    Q(f32, f32, f32, f32),
    /// Cubic Bézier: control points (x1, y1), (x2, y2), end (x, y).
    C(f32, f32, f32, f32, f32, f32),
    /// Close the current subpath.
    Z,
}

/// One filled and/or stroked path of a shape.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ShapePath {
    /// Path segments.
    pub segs: Vec<PathCmd>,
    /// Fill colour; `None` = not filled.
    pub fill: Option<Color>,
    /// (width in points, colour, `ST_PresetLineDashVal` dash name).
    pub stroke: Option<(f32, Color, Option<String>)>,
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
    /// Page colour and page borders.
    pub decor: PageDecor,
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
    /// Paragraph `sub` (document order inside the text box) of the text box anchored at
    /// index offset `at` of body paragraph `para` (§17.17.1 txbxContent); `para` is
    /// [`LineSource::NO_PARA`] for text boxes outside the body (headers, notes).
    TextBox {
        /// Body paragraph of the anchor, or [`LineSource::NO_PARA`].
        para: u32,
        /// Index offset of the anchor in that paragraph.
        at: u32,
        /// Paragraph index inside the text box.
        sub: u32,
    },
    /// Header/footer content and repeated table header rows (not editable in place).
    Other,
    /// End of a table row (§17.4.79 tr): an empty line right of the row's last cell,
    /// where the view draws the end-of-row mark with the formatting marks; its only
    /// caret stop is at the row's right edge.
    RowEnd,
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
    pub carets: Carets,
    /// Vertical text (ECMA-376 Part 1, §17.4.72 textDirection): the line is drawn
    /// rotated about a pivot; its own coordinates are those before the rotation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rotation: Option<LineRotation>,
}

/// Rotation of a line about a pivot point (page coordinates, points).
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct LineRotation {
    /// Degrees clockwise (90 = text reads top to bottom, -90 = bottom to top).
    pub degrees: f32,
    /// Pivot x.
    pub cx: f32,
    /// Pivot y.
    pub cy: f32,
}

impl LineRotation {
    /// Where the rotation takes the unrotated point (`x`, `y`).
    #[must_use]
    pub fn apply(&self, x: f32, y: f32) -> (f32, f32) {
        let (s, c) = self.degrees.to_radians().sin_cos();
        let (dx, dy) = (x - self.cx, y - self.cy);
        (self.cx + dx * c - dy * s, self.cy + dx * s + dy * c)
    }

    /// The unrotated point that the rotation takes to (`x`, `y`) (hit testing).
    #[must_use]
    pub fn invert(&self, x: f32, y: f32) -> (f32, f32) {
        LineRotation {
            degrees: -self.degrees,
            ..*self
        }
        .apply(x, y)
    }

    /// Affine matrix `[a, b, c, d, e, f]` of the rotation (`x' = a·x + c·y + e`).
    #[must_use]
    pub fn matrix(&self) -> [f32; 6] {
        let (s, c) = self.degrees.to_radians().sin_cos();
        [
            c,
            s,
            -s,
            c,
            self.cx - c * self.cx + s * self.cy,
            self.cy - s * self.cx - c * self.cy,
        ]
    }
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
    /// Glyph effects (outline, shadow, glow, gradient fill, horizontal scale…).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fx: Option<Arc<crate::charfx::TextFx>>,
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
        self.carets.shift(dx);
        if let Some(r) = &mut self.rotation {
            r.cx += dx;
            r.cy += dy;
        }
    }
}

/// Caret stops of a line, (offset, x) ascending by offset: shared between the
/// paragraph cache, the incremental layout and the pages (8 bytes per character
/// once, not per copy), each copy with its own horizontal shift.
#[derive(Debug, Clone, Default)]
pub struct Carets {
    stops: Arc<[(u32, f32)]>,
    dx: f32,
}

impl Carets {
    /// Number of stops.
    pub fn len(&self) -> usize {
        self.stops.len()
    }
    /// No stops.
    pub fn is_empty(&self) -> bool {
        self.stops.is_empty()
    }
    /// Stop `i` (x shifted).
    pub fn get(&self, i: usize) -> Option<(u32, f32)> {
        self.stops.get(i).map(|&(o, x)| (o, x + self.dx))
    }
    /// First stop.
    pub fn first(&self) -> Option<(u32, f32)> {
        self.get(0)
    }
    /// Last stop.
    pub fn last(&self) -> Option<(u32, f32)> {
        self.stops.len().checked_sub(1).and_then(|i| self.get(i))
    }
    /// The stops in order (x shifted).
    pub fn iter(&self) -> impl DoubleEndedIterator<Item = (u32, f32)> + ExactSizeIterator + '_ {
        let dx = self.dx;
        self.stops.iter().map(move |&(o, x)| (o, x + dx))
    }
    /// Move every stop by `dx` (the shared stops are not copied).
    pub fn shift(&mut self, dx: f32) {
        self.dx += dx;
    }
}

impl From<Vec<(u32, f32)>> for Carets {
    fn from(v: Vec<(u32, f32)>) -> Self {
        Self {
            stops: v.into(),
            dx: 0.0,
        }
    }
}

impl Serialize for Carets {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_seq(self.iter())
    }
}

impl PageItem {
    /// The drawing object a shape item belongs to (see [`PageItem::Shape`]).
    pub fn source_mut(&mut self) -> Option<&mut LineSource> {
        match self {
            PageItem::Shape { source, .. } => Some(source),
            _ => None,
        }
    }

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
