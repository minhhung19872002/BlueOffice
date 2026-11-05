//! Word-processing layout.
//!
//! Converts a [`bo_model::Document`] into a [`LayoutDocument`]: pages → lines →
//! fragments of positioned glyphs, all in points (1/72 inch), origin at the
//! top-left of each page.
//!
//! Phase 1 features: multi-run paragraphs, left/right/center/justify alignment,
//! auto/exact/at-least line spacing, space before/after, left/right/first-line/hanging
//! indents, default tab stops, Unicode line breaking (UAX #14), emergency breaking of
//! over-long words, manual line/page breaks, page-break-before and section page setup.
//!
//! Incremental layout: each paragraph's line layout depends only on the paragraph,
//! the document defaults and the available width, so it is cached in
//! [`LayoutCache`] keyed by a hash of those inputs. Editing one paragraph only
//! re-shapes and re-breaks that paragraph; pagination is a cheap linear pass.
#![forbid(unsafe_code)]

mod paragraph;
mod types;

pub use paragraph::LayoutCache;
pub use types::{Fragment, GlyphPos, LayoutDocument, Line, Page};

use bo_model::{Block, Document, PageSetup};
use bo_text::FontManager;

/// Layout errors.
#[derive(Debug, thiserror::Error)]
pub enum LayoutError {
    #[error("no fonts are loaded")]
    NoFonts,
}

/// Lay out a whole document.
pub fn layout_document(
    doc: &Document,
    fonts: &mut FontManager,
    cache: &mut LayoutCache,
) -> Result<LayoutDocument, LayoutError> {
    if fonts.is_empty() {
        return Err(LayoutError::NoFonts);
    }
    let mut pages: Vec<Page> = Vec::new();
    for section in &doc.sections {
        let setup = section.page;
        let mut cur = new_page(&setup);
        let mut y = setup.margin_top.to_pt();
        let bottom = setup.height.to_pt() - setup.margin_bottom.to_pt();
        let content_left = setup.margin_left.to_pt();
        let content_width =
            (setup.width.to_pt() - setup.margin_left.to_pt() - setup.margin_right.to_pt()).max(36.0);

        for block in &section.blocks {
            let Block::Paragraph(p) = block;
            let para = paragraph::layout_paragraph(doc, p, content_width, fonts, cache)?;

            if para.page_break_before && !cur.lines.is_empty() {
                pages.push(std::mem::replace(&mut cur, new_page(&setup)));
                y = setup.margin_top.to_pt();
            }
            // Space before is suppressed at the top of a page (matches common word-processor behaviour).
            if !cur.lines.is_empty() {
                y += para.space_before;
            }
            for line in &para.lines {
                if y + line.height > bottom + 0.01 && !cur.lines.is_empty() {
                    pages.push(std::mem::replace(&mut cur, new_page(&setup)));
                    y = setup.margin_top.to_pt();
                }
                cur.lines.push(line.place(content_left, y));
                y += line.height;
                if line.page_break_after {
                    pages.push(std::mem::replace(&mut cur, new_page(&setup)));
                    y = setup.margin_top.to_pt();
                }
            }
            y += para.space_after;
        }
        pages.push(cur);
    }
    Ok(LayoutDocument { pages })
}

fn new_page(setup: &PageSetup) -> Page {
    Page {
        width: setup.width.to_pt(),
        height: setup.height.to_pt(),
        lines: Vec::new(),
    }
}
