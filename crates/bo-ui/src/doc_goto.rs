//! Go To targets found in the document model (BlueDoc round 4): footnote and endnote
//! references (ECMA-376 Part 1, §17.11.14 footnoteReference, §17.11.7 endnoteReference),
//! fields (§17.16), tables (§17.4.38 tbl), graphics (§17.3.3.9 drawing), equations
//! (§22.1.2.77 oMath) and embedded objects (§17.3.3.19 object), in document order as
//! (paragraph id, byte offset of the index text).

use bo_edit::para;
use bo_model::index::{body_paragraphs, is_math_xml, para_at};
use bo_model::{Document, RunContent};

/// Go to what entries of the dialog (its list order).
pub(crate) const FOOTNOTE: usize = 5;
pub(crate) const ENDNOTE: usize = 6;
pub(crate) const FIELD: usize = 7;
pub(crate) const TABLE: usize = 8;
pub(crate) const GRAPHIC: usize = 9;
pub(crate) const EQUATION: usize = 10;
pub(crate) const OBJECT: usize = 11;

/// Does run content `item` count as Go to what entry `what`?
fn is_target(item: &RunContent, what: usize) -> bool {
    match (what, item) {
        (FOOTNOTE, RunContent::FootnoteRef(_))
        | (ENDNOTE, RunContent::EndnoteRef(_))
        | (FIELD, RunContent::Field { .. }) => true,
        (GRAPHIC, RunContent::Image(img)) => img.ole.is_none(),
        (OBJECT, RunContent::Image(img)) => img.ole.is_some(),
        (EQUATION, RunContent::Opaque(r) | RunContent::Markup(r)) => is_math_xml(&r.xml),
        _ => false,
    }
}

/// Name typed into Go To for a target: a field's keyword (`PAGE`), an object's
/// program id; empty for the others.
fn name_of(item: &RunContent) -> String {
    match item {
        RunContent::Field { instr, .. } => instr
            .split_whitespace()
            .next()
            .unwrap_or_default()
            .to_ascii_uppercase(),
        RunContent::Image(img) => img
            .ole
            .as_ref()
            .map(|o| o.prog_id.clone())
            .unwrap_or_default(),
        _ => String::new(),
    }
}

/// Targets of Go to what entry `what` with their names (see [`name_of`]).
pub(crate) fn named_targets(doc: &Document, what: usize) -> (Vec<(usize, usize)>, Vec<String>) {
    let t = targets(doc, what);
    let names = if what == FIELD || what == OBJECT {
        let mut names = Vec::new();
        for path in body_paragraphs(doc) {
            if let Some(p) = para_at(doc, &path) {
                names.extend(
                    para::flatten(p)
                        .iter()
                        .filter(|(_, i)| is_target(i, what))
                        .map(|(_, i)| name_of(i)),
                );
            }
        }
        names
    } else {
        Vec::new()
    };
    (t, names)
}

/// Targets of Go to what entry `what` (one of the constants above) in document order.
pub(crate) fn targets(doc: &Document, what: usize) -> Vec<(usize, usize)> {
    let paths = body_paragraphs(doc);
    if what == TABLE {
        // The first paragraph of every top-level table (paths [section, block, …]).
        let mut out = Vec::new();
        let mut last: Option<&[usize]> = None;
        for (id, path) in paths.iter().enumerate() {
            let in_table = path.len() > 2;
            let top = &path[..path.len().min(2)];
            if in_table && last != Some(top) {
                out.push((id, 0));
            }
            last = in_table.then_some(top);
        }
        return out;
    }
    let mut out = Vec::new();
    for (id, path) in paths.iter().enumerate() {
        let Some(p) = para_at(doc, path) else {
            continue;
        };
        let mut off = 0;
        for (_, item) in para::flatten(p) {
            if is_target(&item, what) {
                out.push((id, off));
            }
            off += para::atom_len(&item);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use bo_model::{Block, ParaProps, Paragraph, Run, RunProps, Section, Table};

    fn para(content: Vec<RunContent>) -> Block {
        Block::Paragraph(Paragraph::new(
            ParaProps::default(),
            vec![Run {
                props: RunProps::default(),
                content,
            }],
        ))
    }

    #[test]
    fn targets_in_document_order() {
        let mut table = Table::default();
        let mut row = bo_model::Row::default();
        let cell = bo_model::Cell {
            blocks: vec![para(vec![RunContent::Text("cell".into())])],
            ..bo_model::Cell::default()
        };
        row.cells = vec![cell.clone(), cell];
        table.rows = vec![row];
        let doc = Document {
            sections: vec![Section::new(vec![
                para(vec![
                    RunContent::Text("ab".into()),
                    RunContent::FootnoteRef(1),
                    RunContent::Text("c".into()),
                    RunContent::FootnoteRef(2),
                ]),
                Block::Table(table),
                para(vec![RunContent::EndnoteRef(1)]),
            ])],
            ..Document::default()
        };
        // "ab" then the reference's placeholder (3 bytes of U+FFFC or the mark text).
        let notes = targets(&doc, FOOTNOTE);
        assert_eq!(notes.len(), 2);
        assert_eq!(notes[0], (0, 2));
        assert!(notes[1].1 > 3);
        assert_eq!(targets(&doc, ENDNOTE), [(3, 0)]);
        // One table: its first cell paragraph.
        assert_eq!(targets(&doc, TABLE), [(1, 0)]);
        assert_eq!(targets(&doc, GRAPHIC), []);
        let doc = Document {
            sections: vec![Section::new(vec![para(vec![
                RunContent::Text("x".into()),
                RunContent::Field {
                    instr: r" page \* MERGEFORMAT ".into(),
                    cached: "1".into(),
                    locked: false,
                    codes: false,
                },
            ])])],
            ..Document::default()
        };
        let (t, names) = named_targets(&doc, FIELD);
        assert_eq!(t, [(0, 1)]);
        assert_eq!(names, ["PAGE"]);
    }
}
