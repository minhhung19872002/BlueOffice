use bo_model::{Color, Underline};
use bo_text::FaceId;
use serde::Serialize;

/// Result of laying out a document.
#[derive(Debug, Clone, Serialize)]
pub struct LayoutDocument {
    pub pages: Vec<Page>,
}

/// One page (points).
#[derive(Debug, Clone, Serialize)]
pub struct Page {
    pub width: f32,
    pub height: f32,
    pub lines: Vec<Line>,
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
    /// Source text (for search, accessibility and PDF text extraction).
    pub text: String,
    pub glyphs: Vec<GlyphPos>,
}

/// A glyph relative to its fragment origin (points, y down).
#[derive(Debug, Clone, Copy, Serialize)]
pub struct GlyphPos {
    pub id: u16,
    pub x: f32,
    pub y: f32,
    /// Byte offset of the glyph's cluster in [Fragment::text].
    pub cluster: u32,
}

impl LayoutDocument {
    /// Text of a page, lines separated by `\n` (used by tests and search).
    pub fn page_text(&self, page: usize) -> String {
        self.pages[page]
            .lines
            .iter()
            .map(|l| l.fragments.iter().map(|f| f.text.as_str()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }
}
