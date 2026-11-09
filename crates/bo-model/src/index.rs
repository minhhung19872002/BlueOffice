//! Paragraph addressing and the *index text* shared by layout and editing.
//!
//! Every paragraph has an index text: the concatenation of its run contents
//! where text contributes its characters and every other editable object one
//! placeholder character (U+FFFC for pictures/fields/notes/equations, `\t`, U+2028 for
//! line breaks, U+000C for page breaks …). Other verbatim markup contributes nothing.
//! Caret positions are byte offsets into this text.

use crate::write::{Block, Document, Paragraph, RunContent};

/// Placeholder for an inline object.
pub const OBJECT: char = '\u{FFFC}';

/// `true` for verbatim markup that is an Office Math zone (ECMA-376 Part 1, §22.1.2.77 oMath,
/// §22.1.2.78 oMathPara): it is laid out and edited as one inline object.
#[must_use]
pub fn is_math_xml(xml: &str) -> bool {
    let name = xml
        .trim_start()
        .strip_prefix('<')
        .and_then(|s| {
            s.split(|c: char| c.is_whitespace() || c == '>' || c == '/')
                .next()
        })
        .unwrap_or("");
    matches!(name.rsplit(':').next(), Some("oMath" | "oMathPara"))
}

/// Index-text representation of one content item.
pub fn item_text(c: &RunContent) -> String {
    match c {
        RunContent::Markup(r) if is_math_xml(&r.xml) => OBJECT.to_string(),
        RunContent::Text(t) => t.clone(),
        RunContent::Tab => "\t".into(),
        RunContent::LineBreak => "\u{2028}".into(),
        RunContent::PageBreak => "\u{000C}".into(),
        RunContent::ColumnBreak => "\u{000B}".into(),
        RunContent::NoBreakHyphen => "\u{2011}".into(),
        RunContent::SoftHyphen => "\u{00AD}".into(),
        // A check box / drop-down form field is one object; a text field's result is text.
        RunContent::FormField(f) if f.is_text() => String::new(),
        RunContent::Image(_)
        | RunContent::Field { .. }
        | RunContent::FormField(_)
        | RunContent::FootnoteRef(_)
        | RunContent::EndnoteRef(_)
        | RunContent::NoteMark => OBJECT.to_string(),
        RunContent::FormEnd
        | RunContent::Opaque(_)
        | RunContent::Markup(_)
        | RunContent::CommentStart(_)
        | RunContent::CommentEnd(_)
        | RunContent::CommentRef(_)
        | RunContent::ControlStart(_)
        | RunContent::ControlEnd(_) => String::new(),
    }
}

/// Byte range of a content item in the paragraph index text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ItemSpan {
    /// 0-based index of the run in `Paragraph::runs`.
    pub run: usize,
    /// 0-based index of the item in that run's `content`.
    pub item: usize,
    /// Start byte offset in the index text (inclusive).
    pub start: usize,
    /// End byte offset in the index text (exclusive).
    pub end: usize,
}

/// Index text and item spans of a paragraph.
pub fn para_index(p: &Paragraph) -> (String, Vec<ItemSpan>) {
    let mut s = String::new();
    let mut spans = Vec::new();
    for (ri, r) in p.runs.iter().enumerate() {
        for (ii, c) in r.content.iter().enumerate() {
            let start = s.len();
            s.push_str(&item_text(c));
            spans.push(ItemSpan {
                run: ri,
                item: ii,
                start,
                end: s.len(),
            });
        }
    }
    (s, spans)
}

/// Index text only.
pub fn para_index_text(p: &Paragraph) -> String {
    para_index(p).0
}

/// Location of a body paragraph: `[section, block, (row, cell, block)*]`.
pub type ParaPath = Vec<usize>;

/// All body paragraphs in document order (descending into table cells).
/// The position in the returned vector is the paragraph id used by layout.
pub fn body_paragraphs(doc: &Document) -> Vec<ParaPath> {
    let mut out = Vec::new();
    for (si, s) in doc.sections.iter().enumerate() {
        collect(&s.blocks, &mut vec![si], &mut out);
    }
    out
}

fn collect(blocks: &[Block], prefix: &mut Vec<usize>, out: &mut Vec<ParaPath>) {
    for (bi, b) in blocks.iter().enumerate() {
        prefix.push(bi);
        match b {
            Block::Paragraph(_) => out.push(prefix.clone()),
            Block::Table(t) => {
                for (ri, r) in t.rows.iter().enumerate() {
                    for (ci, c) in r.cells.iter().enumerate() {
                        prefix.push(ri);
                        prefix.push(ci);
                        collect(&c.blocks, prefix, out);
                        prefix.pop();
                        prefix.pop();
                    }
                }
            }
            Block::Opaque(_) => {}
        }
        prefix.pop();
    }
}

/// Blocks container addressed by a path prefix (everything but the last block index).
pub fn blocks_at<'d>(doc: &'d Document, path: &[usize]) -> Option<&'d Vec<Block>> {
    let (&si, rest) = path.split_first()?;
    let mut blocks = &doc.sections.get(si)?.blocks;
    let mut rest = rest;
    while rest.len() >= 3 {
        let Block::Table(t) = blocks.get(rest[0])? else {
            return None;
        };
        blocks = &t.rows.get(rest[1])?.cells.get(rest[2])?.blocks;
        rest = &rest[3..];
    }
    if rest.is_empty() {
        Some(blocks)
    } else {
        None
    }
}

/// Mutable variant of [`blocks_at`].
pub fn blocks_at_mut<'d>(doc: &'d mut Document, path: &[usize]) -> Option<&'d mut Vec<Block>> {
    let (&si, rest) = path.split_first()?;
    let mut blocks = &mut doc.sections.get_mut(si)?.blocks;
    let mut rest = rest;
    while rest.len() >= 3 {
        let Block::Table(t) = blocks.get_mut(rest[0])? else {
            return None;
        };
        blocks = &mut t.rows.get_mut(rest[1])?.cells.get_mut(rest[2])?.blocks;
        rest = &rest[3..];
    }
    if rest.is_empty() {
        Some(blocks)
    } else {
        None
    }
}

/// Paragraph at a path.
pub fn para_at<'d>(doc: &'d Document, path: &[usize]) -> Option<&'d Paragraph> {
    let (last, prefix) = path.split_last()?;
    match blocks_at(doc, prefix)?.get(*last)? {
        Block::Paragraph(p) => Some(p),
        _ => None,
    }
}

/// Mutable paragraph at a path.
pub fn para_at_mut<'d>(doc: &'d mut Document, path: &[usize]) -> Option<&'d mut Paragraph> {
    let (last, prefix) = path.split_last()?;
    match blocks_at_mut(doc, prefix)?.get_mut(*last)? {
        Block::Paragraph(p) => Some(p),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::write::{Image, Run, RunProps, Section, Table};
    use crate::{Emu, Twips};

    #[test]
    fn index_text_and_paths() {
        let img = RunContent::Image(Box::new(Image {
            media: "/word/media/a.png".into(),
            width: Emu(1),
            height: Emu(1),
            anchor: None,
            crop: None,
            rotation: 0,
            description: String::new(),
            shape: None,
            format: None,
            src: None,
        }));
        let p = Paragraph::new(
            crate::write::ParaProps::default(),
            vec![Run {
                props: RunProps::default(),
                content: vec![RunContent::Text("ab".into()), RunContent::Tab, img],
            }],
        );
        let (t, spans) = para_index(&p);
        assert_eq!(t, "ab\t\u{FFFC}");
        assert_eq!(spans[2].start, 3);
        let mut table = Table::simple(1, 2, Twips(1000));
        if let Block::Table(_) = Block::Table(table.clone()) {}
        table.rows[0].cells[1]
            .blocks
            .push(Block::Paragraph(p.clone()));
        let doc = Document {
            sections: vec![Section::new(vec![Block::Paragraph(p), Block::Table(table)])],
            ..Default::default()
        };
        let paths = body_paragraphs(&doc);
        assert_eq!(
            paths,
            vec![
                vec![0, 0],
                vec![0, 1, 0, 0, 0],
                vec![0, 1, 0, 1, 0],
                vec![0, 1, 0, 1, 1]
            ]
        );
        assert_eq!(
            para_index_text(para_at(&doc, &paths[3]).unwrap()),
            "ab\t\u{FFFC}"
        );
        assert!(para_at(&doc, &[0, 1]).is_none());
    }
}
