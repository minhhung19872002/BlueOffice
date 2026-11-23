//! Mailings › Update Labels (BlueDoc round 16): in a label sheet (the document's first table),
//! every label after the first gets the first label's content, led by a `NEXT` field
//! (ECMA-376 Part 1, §17.16.5.36 NEXT) so each label takes the next record — as the reference
//! word processor's Update Labels does. One undo step.
use crate::{ops::Op, Result, Selection, Session};
use bo_model::{Block, Paragraph, Run, RunContent};

/// The first table at the top level: (section, block index).
fn first_table(doc: &bo_model::Document) -> Option<(usize, usize)> {
    doc.sections.iter().enumerate().find_map(|(s, sec)| {
        sec.blocks
            .iter()
            .position(|b| matches!(b, Block::Table(_)))
            .map(|b| (s, b))
    })
}

/// The cell's blocks without a leading NEXT field (so updating twice adds one only).
fn without_next(blocks: &[Block]) -> Vec<Block> {
    let mut out = blocks.to_vec();
    if let Some(Block::Paragraph(p)) = out.first_mut() {
        if let Some(r) = p.runs.first_mut() {
            r.content.retain(|c| {
                !matches!(c, RunContent::Field { instr, .. }
                    if crate::fields::field_kind(instr) == "NEXT")
            });
        }
        p.runs.retain(|r| !r.content.is_empty());
    }
    out
}

impl Session {
    /// Update Labels: `false` when the document has no label sheet (table).
    pub fn update_labels(&mut self) -> Result<bool> {
        let Some((s, b)) = first_table(&self.doc) else {
            return Ok(false);
        };
        let Some(Block::Table(table)) = self.doc.sections.get(s).and_then(|sec| sec.blocks.get(b))
        else {
            return Ok(false);
        };
        let old = table.clone();
        let Some(first) = old.rows.first().and_then(|r| r.cells.first()) else {
            return Ok(false);
        };
        let template = without_next(&first.blocks);
        let mut new = old.clone();
        for (ri, row) in new.rows.iter_mut().enumerate() {
            for (ci, cell) in row.cells.iter_mut().enumerate() {
                if ri == 0 && ci == 0 {
                    continue;
                }
                let mut blocks = template.clone();
                let next = Run {
                    content: vec![RunContent::Field {
                        instr: "NEXT".to_owned(),
                        cached: String::new(),
                        locked: false,
                        codes: false,
                    }],
                    ..Run::default()
                };
                match blocks.first_mut() {
                    Some(Block::Paragraph(p)) => p.runs.insert(0, next),
                    _ => blocks.insert(
                        0,
                        Block::Paragraph(Paragraph {
                            runs: vec![next],
                            ..Paragraph::default()
                        }),
                    ),
                }
                cell.blocks = blocks;
            }
        }
        if new == old {
            return Ok(true);
        }
        self.edit(|tx, sel| {
            tx.push(Op::Blocks {
                path: vec![s],
                index: b,
                remove: vec![Block::Table(old)],
                insert: vec![Block::Table(new)],
            })?;
            Ok::<Selection, crate::EditError>(sel)
        })?;
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use crate::Session;
    use bo_model::{Block, Cell, Document, Paragraph, Row, Run, RunContent, Section, Table};

    #[test]
    fn update_labels_copies_the_first_label_with_next() {
        let field = |name: &str| RunContent::Field {
            instr: format!("MERGEFIELD {name}"),
            cached: format!("«{name}»"),
            locked: false,
            codes: false,
        };
        let cell = |runs: Vec<Run>| Cell {
            blocks: vec![Block::Paragraph(Paragraph {
                runs,
                ..Paragraph::default()
            })],
            ..Cell::default()
        };
        let first = cell(vec![Run {
            content: vec![field("Name")],
            ..Run::default()
        }]);
        let table = Table {
            rows: vec![Row {
                cells: vec![first, cell(vec![]), cell(vec![])],
                ..Row::default()
            }],
            ..Table::default()
        };
        let doc = Document {
            sections: vec![Section {
                blocks: vec![Block::Table(table), Block::Paragraph(Paragraph::default())],
                ..Section::default()
            }],
            ..Document::default()
        };
        let mut s = Session::new(doc);
        assert!(s.update_labels().expect("update"));
        let Some(Block::Table(t)) = s.doc().sections[0].blocks.first() else {
            panic!("table");
        };
        let Some(Block::Paragraph(p)) = t.rows[0].cells[2].blocks.first() else {
            panic!("paragraph");
        };
        let kinds: Vec<String> = p
            .runs
            .iter()
            .flat_map(|r| &r.content)
            .filter_map(|c| match c {
                RunContent::Field { instr, .. } => Some(instr.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(kinds, ["NEXT", "MERGEFIELD Name"]);
        // Twice: still one NEXT.
        assert!(s.update_labels().expect("again"));
        s.undo();
        s.undo();
        let Some(Block::Table(t)) = s.doc().sections[0].blocks.first() else {
            panic!("table");
        };
        assert!(t.rows[0].cells[2]
            .blocks
            .iter()
            .all(|b| matches!(b, Block::Paragraph(p) if p.runs.is_empty())));
    }
}
