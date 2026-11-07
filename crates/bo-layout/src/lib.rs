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
//! in [`LayoutCache`] keyed by a hash of those inputs. Editing one paragraph only
//! re-shapes and re-breaks that paragraph; pagination is a cheap linear pass.
#![forbid(unsafe_code)]

mod blocks;
mod flow;
mod paragraph;
mod style;
mod types;

pub use paragraph::{FieldContext, LayoutCache};
pub use style::author_color;
pub use types::{
    Fragment, GlyphPos, Layer, LayoutDocument, Line, LineSource, NoteKey, Page, PageItem,
};

use blocks::{stack_blocks, Ctx};
use bo_model::{Block, Document, NumFormat, RunContent, Twips};
use bo_text::FontManager;
use flow::{float_item, hf_blocks, Flow};
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
/// (ECMA-376 Part 1, §17.11.11 numFmt defaults).
fn number_notes(doc: &Document) -> (HashMap<NoteKey, String>, Vec<i32>, bool) {
    let mut map = HashMap::new();
    let mut foot = 0;
    let mut end = 0;
    let mut end_order = Vec::new();
    let mut needs_total = false;
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
                let k = instr.trim_start().to_ascii_uppercase();
                if k.starts_with("NUMPAGES") || k.starts_with("SECTIONPAGES") {
                    needs_total = true;
                }
            }
            _ => {}
        });
    }
    (map, end_order, needs_total)
}

/// Content width of a section in points.
fn content_width(s: &bo_model::Section) -> f32 {
    (s.page.width.to_pt() - s.page.margin_left.to_pt() - s.page.margin_right.to_pt()).max(36.0)
}

/// Header/footer variant shown on a page: 0 default, 1 first, 2 even (§17.10.5, §17.10.6 titlePg).
fn variant_for(doc: &Document, flow: &Flow, page_index: usize) -> u8 {
    let page = &flow.pages[page_index];
    let sec = &doc.sections[page.section.min(doc.sections.len() - 1)];
    if sec.title_page && flow.section_start.get(page.section).copied().flatten() == Some(page_index)
    {
        1
    } else if doc.even_and_odd_headers && page.number & 1 == 0 {
        2
    } else {
        0
    }
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
    if fonts.is_empty() {
        return Err(LayoutError::NoFonts);
    }
    let (notes, endnote_order, needs_total) = number_notes(doc);
    let mut ctx = Ctx {
        doc,
        fonts,
        cache,
        lists: style::ListCounters::default(),
        other_lists: style::ListCounters::default(),
        notes,
        fields: FieldContext::default(),
        default_tab: doc.default_tab_stop.unwrap_or(Twips(720)).to_pt(),
        next_id: 0,
        endnote_order,
        view: opts.view,
    };
    if doc.sections.is_empty() {
        let mut flow = Flow::new(Vec::new(), 0);
        flow.run(&mut ctx)?;
        return Ok(LayoutDocument { pages: flow.pages });
    }

    // Header/footer heights per section (they push the body when taller than the margin).
    let mut hf = Vec::with_capacity(doc.sections.len());
    for (si, s) in doc.sections.iter().enumerate() {
        let w = content_width(s);
        let mut heights = (0.0f32, 0.0f32);
        for header in [true, false] {
            for v in 0..3u8 {
                if let Some(blocks) = hf_blocks(doc, si, header, v) {
                    let h = stack_blocks(&mut ctx, blocks, w, false, None)?.h;
                    if header {
                        heights.0 = heights.0.max(h);
                    } else {
                        heights.1 = heights.1.max(h);
                    }
                }
            }
        }
        hf.push(heights);
    }
    ctx.other_lists = style::ListCounters::default();

    // Pagination (twice when the body shows the total page count).
    let mut flow = Flow::new(hf.clone(), doc.sections.len());
    flow.run(&mut ctx)?;
    if needs_total {
        let total = flow.pages.len() as u32;
        ctx.fields.pages = total;
        ctx.lists = style::ListCounters::default();
        ctx.next_id = 0;
        flow = Flow::new(hf, doc.sections.len());
        flow.run(&mut ctx)?;
    }

    // Headers and footers with page fields.
    let total = flow.pages.len() as u32;
    let mut section_pages: HashMap<usize, u32> = HashMap::new();
    for p in &flow.pages {
        *section_pages.entry(p.section).or_default() += 1;
    }
    for pi in 0..flow.pages.len() {
        let (si, number) = (flow.pages[pi].section, flow.pages[pi].number);
        let sec = &doc.sections[si.min(doc.sections.len() - 1)];
        let setup = sec.page;
        let variant = variant_for(doc, &flow, pi);
        ctx.fields = FieldContext {
            page: number,
            pages: total,
            section_pages: section_pages.get(&si).copied().unwrap_or(0),
        };
        let left = setup.margin_left.to_pt();
        let w = content_width(sec);
        for header in [true, false] {
            let Some(blocks) = hf_blocks(doc, si, header, variant) else {
                continue;
            };
            let st = stack_blocks(&mut ctx, blocks, w, false, None)?;
            let y0 = if header {
                setup.header.to_pt()
            } else {
                setup.height.to_pt() - setup.footer.to_pt() - st.h
            };
            let page = &mut flow.pages[pi];
            let mut lines = Vec::new();
            for (top, band) in &st.bands {
                let b = band.translated(left, y0 + top);
                lines.extend(b.lines);
                page.items.extend(b.items);
            }
            // Keep reading order: header lines first, footer lines last.
            if header {
                page.lines.splice(0..0, lines);
            } else {
                page.lines.extend(lines);
            }
            for (top, img) in &st.floats {
                let (item, _) = float_item(img, &setup, left, y0 + top);
                page.items.push(item);
            }
        }
    }
    Ok(LayoutDocument { pages: flow.pages })
}
