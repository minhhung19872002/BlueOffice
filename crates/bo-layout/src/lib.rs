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
mod drawing;
mod flow;
mod incremental;
mod markup;
mod math;
mod paragraph;
mod style;
mod types;

pub use blocks::TableMeasure;
pub use incremental::{measure_table, IncrementalLayout, UpdateKind};
pub use markup::{
    balloon_height, page_markup, stack_balloons, Balloon, ChangeBar, Mark, MarkFilter, MarkKind,
    MarkupStyle, PageMarkup, BALLOON_GAP, MAX_HIDDEN_REVIEWERS,
};
pub use math::{layout_equation, EquationGlyphs, EquationRun};
pub use paragraph::{FieldContext, LayoutCache};
pub use style::author_color;
pub use types::{
    Fragment, GlyphPos, Layer, LayoutDocument, Line, LineRotation, LineSource, NoteKey, Page,
    PageItem, PathCmd, ShapePath, SharedStr,
};

use bo_model::change::BlockChanges;
use bo_model::{Block, Document, NoteRestart, NumFormat, RunContent};
use bo_text::FontManager;
use std::collections::HashMap;

/// Layout errors.
#[derive(Debug, thiserror::Error)]
pub enum LayoutError {
    /// The font manager has no fonts to shape with.
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
/// by default (ECMA-376 Part 1, §17.11.17 numFmt), from §17.11.20 numStart, restarted
/// per section with §17.11.19 numRestart `eachSect` (`eachPage` is applied after
/// pagination, see [`page_note_numbers`]); whether the body shows the total page
/// count; whether the body has fields at all.
fn number_notes(doc: &Document) -> (HashMap<NoteKey, String>, Vec<i32>, bool, bool) {
    let mut map = HashMap::new();
    let mut foot = 0i32;
    let mut end = 0i32;
    let mut end_order = Vec::new();
    let mut needs_total = false;
    let mut fields = false;
    for (si, s) in doc.sections.iter().enumerate() {
        let (fp, ep) = (&doc.footnote_props(si), &doc.endnote_props(si));
        if si == 0 || fp.restart == Some(NoteRestart::EachSect) {
            foot = fp.start.map_or(0, |n| n as i32 - 1);
        }
        if si == 0 || ep.restart == Some(NoteRestart::EachSect) {
            end = ep.start.map_or(0, |n| n as i32 - 1);
        }
        let ffmt = fp.fmt.clone().unwrap_or(NumFormat::Decimal);
        let efmt = ep.fmt.clone().unwrap_or(NumFormat::LowerRoman);
        visit_body(&s.blocks, &mut |c| match c {
            RunContent::FootnoteRef(id) => {
                map.entry((false, *id)).or_insert_with(|| {
                    foot += 1;
                    bo_model::format_number(foot, &ffmt)
                });
            }
            RunContent::EndnoteRef(id) => {
                map.entry((true, *id)).or_insert_with(|| {
                    end += 1;
                    end_order.push(*id);
                    bo_model::format_number(end, &efmt)
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

/// Footnote numbers restarted on every page of the sections with §17.11.19 numRestart
/// `eachPage`, from the footnotes placed on each page of a layout (`None`: no such
/// section).
fn page_note_numbers(
    doc: &Document,
    pages: &[Page],
    numbers: &HashMap<NoteKey, String>,
) -> Option<HashMap<NoteKey, String>> {
    let each = |si: usize| {
        doc.sections
            .get(si)
            .is_some_and(|_| doc.footnote_props(si).restart == Some(NoteRestart::EachPage))
    };
    if !(0..doc.sections.len()).any(each) {
        return None;
    }
    let mut map = numbers.clone();
    // A footnote continued from the previous page keeps the number of its first page.
    let mut earlier: std::collections::HashSet<i32> = std::collections::HashSet::new();
    for p in pages {
        let numbered = each(p.section);
        let s = &doc.footnote_props(p.section);
        let fmt = s.fmt.clone().unwrap_or(NumFormat::Decimal);
        let mut n = s.start.map_or(0, |n| n as i32 - 1);
        let mut seen = Vec::new();
        for l in &p.lines {
            if let LineSource::Note {
                endnote: false, id, ..
            } = l.source
            {
                if !seen.contains(&id) && !earlier.contains(&id) {
                    seen.push(id);
                    if !numbered {
                        continue;
                    }
                    n += 1;
                    map.insert((false, id), bo_model::format_number(n, &fmt));
                }
            }
        }
        earlier.extend(seen);
    }
    Some(map)
}

/// Endnotes referenced in `blocks`, in order.
fn endnotes_in(blocks: &[Block]) -> Vec<i32> {
    let mut v = Vec::new();
    let mut seen = std::collections::HashSet::new();
    visit_body(blocks, &mut |c| {
        if let RunContent::EndnoteRef(id) = c {
            if seen.insert(*id) {
                v.push(*id);
            }
        }
    });
    v
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
    /// Which version of tracked changes to lay out.
    pub view: RevisionView,
    /// Show field codes (`{ PAGE \* ROMAN }`) instead of field results.
    pub field_codes: bool,
    /// What the markup view marks (reviewer filter, insertions/deletions, balloons).
    pub marks: MarkFilter,
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
