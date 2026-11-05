//! Word-processing document model.

use crate::units::{HalfPoints, Twips};
use serde::{Deserialize, Serialize};

/// sRGB colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Color {
    pub const BLACK: Color = Color { r: 0, g: 0, b: 0 };

    /// Parse `RRGGBB` (ECMA-376 Part 1, §17.18.38 `ST_HexColorRGB`). Returns `None` for `auto` or junk.
    pub fn from_hex(s: &str) -> Option<Color> {
        if s.len() != 6 || !s.is_ascii() {
            return None;
        }
        let p = |i: usize| u8::from_str_radix(&s[i..i + 2], 16).ok();
        Some(Color {
            r: p(0)?,
            g: p(2)?,
            b: p(4)?,
        })
    }
}

/// A whole word-processing document.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Document {
    pub sections: Vec<Section>,
    /// Document-wide default run properties (`w:docDefaults/w:rPrDefault`).
    pub default_run: RunProps,
    /// Document-wide default paragraph properties (`w:docDefaults/w:pPrDefault`).
    pub default_para: ParaProps,
}

impl Document {
    /// Iterate over all paragraphs in document order.
    pub fn paragraphs(&self) -> impl Iterator<Item = &Paragraph> {
        self.sections.iter().flat_map(|s| {
            s.blocks.iter().map(|b| match b {
                Block::Paragraph(p) => p,
            })
        })
    }

    /// Plain text of the document, paragraphs separated by `\n`.
    pub fn plain_text(&self) -> String {
        let mut out = String::new();
        for (i, p) in self.paragraphs().enumerate() {
            if i > 0 {
                out.push('\n');
            }
            out.push_str(&p.plain_text());
        }
        out
    }
}

/// A section (ECMA-376 Part 1, §17.6 Sections).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Section {
    pub page: PageSetup,
    pub blocks: Vec<Block>,
}

/// Page size and margins (ECMA-376 Part 1, §17.6.13 pgSz, §17.6.11 pgMar).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PageSetup {
    pub width: Twips,
    pub height: Twips,
    pub margin_top: Twips,
    pub margin_bottom: Twips,
    pub margin_left: Twips,
    pub margin_right: Twips,
}

impl Default for PageSetup {
    /// A4 portrait with the margins recommended by Nghị định 30/2020/NĐ-CP
    /// (top/bottom 20 mm, left 30 mm, right 15 mm).
    fn default() -> Self {
        PageSetup {
            width: Twips(11_906),
            height: Twips(16_838),
            margin_top: Twips(1134),
            margin_bottom: Twips(1134),
            margin_left: Twips(1701),
            margin_right: Twips(850),
        }
    }
}

/// Block-level content.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Block {
    Paragraph(Paragraph),
}

/// A paragraph (ECMA-376 Part 1, §17.3.1.22 p).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct Paragraph {
    pub props: ParaProps,
    pub runs: Vec<Run>,
}

impl Paragraph {
    /// Concatenated text; tabs as `\t`, line breaks as `\u{2028}`.
    pub fn plain_text(&self) -> String {
        let mut s = String::new();
        for r in &self.runs {
            for c in &r.content {
                match c {
                    RunContent::Text(t) => s.push_str(t),
                    RunContent::Tab => s.push('\t'),
                    RunContent::LineBreak => s.push('\u{2028}'),
                    RunContent::PageBreak => {}
                }
            }
        }
        s
    }
}

/// Horizontal alignment (ECMA-376 Part 1, §17.18.44 `ST_Jc`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum Alignment {
    #[default]
    Left,
    Center,
    Right,
    Justify,
}

/// Line spacing rule (ECMA-376 Part 1, §17.3.1.33 spacing, §17.18.48 `ST_LineSpacingRule`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LineSpacing {
    /// Multiple of single spacing in 240ths (240 = single).
    Auto(u32),
    /// Exact line height.
    Exact(Twips),
    /// Minimum line height.
    AtLeast(Twips),
}

impl Default for LineSpacing {
    fn default() -> Self {
        LineSpacing::Auto(240)
    }
}

/// Paragraph properties. `None` means "inherit".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct ParaProps {
    pub align: Option<Alignment>,
    pub indent_left: Option<Twips>,
    pub indent_right: Option<Twips>,
    /// Positive = first-line indent, negative = hanging indent.
    pub indent_first_line: Option<Twips>,
    pub space_before: Option<Twips>,
    pub space_after: Option<Twips>,
    pub line_spacing: Option<LineSpacing>,
    pub page_break_before: Option<bool>,
}

impl ParaProps {
    /// Overlay `self` on top of `base`.
    #[must_use]
    pub fn inherit(&self, base: &ParaProps) -> ParaProps {
        ParaProps {
            align: self.align.or(base.align),
            indent_left: self.indent_left.or(base.indent_left),
            indent_right: self.indent_right.or(base.indent_right),
            indent_first_line: self.indent_first_line.or(base.indent_first_line),
            space_before: self.space_before.or(base.space_before),
            space_after: self.space_after.or(base.space_after),
            line_spacing: self.line_spacing.or(base.line_spacing),
            page_break_before: self.page_break_before.or(base.page_break_before),
        }
    }
}

/// Underline style (ECMA-376 Part 1, §17.18.99 `ST_Underline`) — subset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Underline {
    None,
    Single,
    Double,
}

/// Run properties. `None` means "inherit".
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct RunProps {
    pub font: Option<String>,
    pub size: Option<HalfPoints>,
    pub bold: Option<bool>,
    pub italic: Option<bool>,
    pub underline: Option<Underline>,
    pub strike: Option<bool>,
    pub color: Option<Color>,
}

impl RunProps {
    /// Overlay `self` on top of `base`.
    #[must_use]
    pub fn inherit(&self, base: &RunProps) -> RunProps {
        RunProps {
            font: self.font.clone().or_else(|| base.font.clone()),
            size: self.size.or(base.size),
            bold: self.bold.or(base.bold),
            italic: self.italic.or(base.italic),
            underline: self.underline.or(base.underline),
            strike: self.strike.or(base.strike),
            color: self.color.or(base.color),
        }
    }
}

/// A run of uniformly formatted content (ECMA-376 Part 1, §17.3.2.25 r).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct Run {
    pub props: RunProps,
    pub content: Vec<RunContent>,
}

impl Run {
    /// Convenience constructor for a plain text run.
    pub fn text(text: impl Into<String>, props: RunProps) -> Run {
        Run {
            props,
            content: vec![RunContent::Text(text.into())],
        }
    }
}

/// Run content items (ECMA-376 Part 1, §17.3.3).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RunContent {
    /// §17.3.3.31 t
    Text(String),
    /// §17.3.3.32 tab
    Tab,
    /// §17.3.3.1 br (textWrapping)
    LineBreak,
    /// §17.3.3.1 br type="page"
    PageBreak,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_color() {
        assert_eq!(
            Color::from_hex("FF0080"),
            Some(Color { r: 255, g: 0, b: 128 })
        );
        assert_eq!(Color::from_hex("auto"), None);
        assert_eq!(Color::from_hex("éé"), None);
    }

    #[test]
    fn inheritance() {
        let base = RunProps {
            size: Some(HalfPoints(24)),
            bold: Some(false),
            ..Default::default()
        };
        let own = RunProps {
            bold: Some(true),
            ..Default::default()
        };
        let r = own.inherit(&base);
        assert_eq!(r.size, Some(HalfPoints(24)));
        assert_eq!(r.bold, Some(true));
    }

    #[test]
    fn plain_text() {
        let p = Paragraph {
            props: ParaProps::default(),
            runs: vec![Run {
                props: RunProps::default(),
                content: vec![
                    RunContent::Text("a".into()),
                    RunContent::Tab,
                    RunContent::Text("b".into()),
                ],
            }],
        };
        assert_eq!(p.plain_text(), "a\tb");
    }
}
