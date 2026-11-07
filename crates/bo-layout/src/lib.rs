//! Word-processing layout.
//!
//! Converts a [`bo_model::Document`] into a [`LayoutDocument`]: pages → lines →
//! fragments of positioned glyphs plus non-text page items (shading, borders,
//! pictures), all in points (1/72 inch), origin at the top-left of each page.
//!
//! Features: style resolution (§17.7.2), list labels (§17.9), custom tab stops,
//! keepNext/keepLines/widow control, tables (rows split across pages, repeated
//! header rows, merged cells, borders, shading), inline and anchored pictures,
//! headers/footers (first/even/default with inheritance), text columns,
//! footnotes at the page bottom, endnotes at the document end and the PAGE /
//! NUMPAGES / SECTIONPAGES fields.
//!
//! Incremental layout: each paragraph's line layout depends only on the
//! paragraph, its effective properties and the available width, so it is cached
//! in [`LayoutCache`] keyed by a hash of those inputs (shaping itself is cached by
//! font, size and text in `bo_text::FontManager`). [`IncrementalLayout`] keeps the
//! previous layout and re-paginates only from the first changed block until the
//! page breaks re-synchronise (see the `incremental` module).
#![forbid(unsafe_code)]

mod blocks;
mod flow;
mod incremental;
mod paragraph;
mod style;
mod types;

pub use incremental::{IncrementalLayout, UpdateKind};
pub use paragraph::{FieldContext, LayoutCache};
pub use style::author_color;
pub use types::{
    Fragment, GlyphPos, Layer, LayoutDocument, Line, LineSource, NoteKey, Page, PageItem, SharedStr,
};

use bo_model::change::BlockChanges;
use bo_model::{Block, Document, NumFormat, RunContent};
use bo_text::FontManager;
use std::collections::HashMap;

/// Layout errors.
#[derive(Debug, thiserror::Error)]
pub enum LayoutError {
    #[error("no fonts are loaded")]
    NoFonts,
}

/// Visit every run content item of the main story in document order.
fn visit_body(blocks: &[Block], f: &mut impl FnMut(&RunContent)) {
    for b in blocks {
        match b {
            Block::Paragraph(p) => {
                for r in &p.runs {
                    for c in &r.content {
                        f(c);
                    }
                }
            }
            Block::Table(t) => {
                for row in &t.rows {
                    for cell in &row.cells {
                        visit_body(&cell.blocks, f);
                    }
                }
            }
            Block::Opaque(_) => {}
        }
    }
}

/// Note numbers in order of first reference: footnotes decimal, endnotes lower-roman
/// (ECMA-376 Part 1, §17.11.11 numFmt defaults); whether the body shows the total
/// page count; whether the body has fields at all.
fn number_notes(doc: &Document) -> (HashMap<NoteKey, String>, Vec<i32>, bool, bool) {
    let mut map = HashMap::new();
    let mut foot = 0;
    let mut end = 0;
    let mut end_order = Vec::new();
    let mut needs_total = false;
    let mut fields = false;
    for s in &doc.sections {
        visit_body(&s.blocks, &mut |c| match c {
            RunContent::FootnoteRef(id) => {
                map.entry((false, *id)).or_insert_with(|| {
                    foot += 1;
                    foot.to_string()
                });
            }
            RunContent::EndnoteRef(id) => {
                map.entry((true, *id)).or_insert_with(|| {
                    end += 1;
                    end_order.push(*id);
                    bo_model::format_number(end, &NumFormat::LowerRoman)
                });
            }
            RunContent::Field { instr, .. } => {
                fields = true;
                let k = instr.trim_start().to_ascii_uppercase();
                if k.starts_with("NUMPAGES") || k.starts_with("SECTIONPAGES") {
                    needs_total = true;
                }
            }
            _ => {}
        });
    }
    (map, end_order, needs_total, fields)
}

/// Content width of a section in points.
fn content_width(s: &bo_model::Section) -> f32 {
    (s.page.width.to_pt() - s.page.margin_left.to_pt() - s.page.margin_right.to_pt()).max(36.0)
}

/// How tracked changes (ECMA-376 Part 1, §17.13.5) are displayed.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "camelCase")]
pub enum RevisionView {
    /// All markup: insertions underlined, deletions struck through, coloured per author.
    #[default]
    Markup,
    /// The document as if every change were accepted.
    Final,
    /// The document as if every change were rejected.
    Original,
}

/// Layout options that are not part of the document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LayoutOptions {
    pub view: RevisionView,
}

/// Lay out a whole document.
pub fn layout_document(
    doc: &Document,
    fonts: &mut FontManager,
    cache: &mut LayoutCache,
) -> Result<LayoutDocument, LayoutError> {
    layout_document_with(doc, fonts, cache, LayoutOptions::default())
}

/// Lay out a whole document with explicit [`LayoutOptions`].
pub fn layout_document_with(
    doc: &Document,
    fonts: &mut FontManager,
    cache: &mut LayoutCache,
    opts: LayoutOptions,
) -> Result<LayoutDocument, LayoutError> {
    let mut inc = IncrementalLayout::new();
    inc.update(doc, fonts, cache, opts, &BlockChanges::everything())?;
    Ok(inc.into_document())
}
