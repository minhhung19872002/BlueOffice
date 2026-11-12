//! Row-level content controls (ECMA-376 Part 1, §17.5.2.32 sdt around `w:tr`,
//! `CT_SdtContentRow`): a repeating section of table rows ([MS-DOCX] w15:repeatingSection whose
//! items, w15:repeatingSectionItem, each hold one or more rows). The controls live on the rows
//! (`RowProps::controls` open before a row, `RowProps::controls_end` close after it); adding or
//! deleting an item replaces the table in one undo step.

use crate::table::{cell_at, table_of, CellAt};
use crate::{max_control_id, EditError, Op, Pos, Result, Selection, Session};
use bo_model::{Block, ContentControl, ControlKind, Row, RunContent, Table};

/// A row-level control of a table: its rows `first..=last`, where it opens in the first row's
/// list, how deep it is nested and how many controls close after its last row up to and
/// including it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RowSpan {
    pub cc: ContentControl,
    pub first: usize,
    /// Index in `rows[first].props.controls`.
    pub idx: usize,
    pub last: usize,
    pub depth: usize,
    /// Controls closing after `last` up to and including this one.
    pub closes: usize,
}

/// Every row-level control of `t`, in the order they close (unclosed ones close after the last
/// row, as the writer does).
pub(crate) fn row_spans(t: &Table) -> Vec<RowSpan> {
    let mut out = Vec::new();
    let mut open: Vec<(ContentControl, usize, usize)> = Vec::new();
    let close = |open: &mut Vec<(ContentControl, usize, usize)>, ri: usize, k: usize| {
        open.pop().map(|(cc, first, idx)| RowSpan {
            cc,
            first,
            idx,
            last: ri,
            depth: open.len(),
            closes: k + 1,
        })
    };
    for (ri, row) in t.rows.iter().enumerate() {
        for (j, cc) in row.props.controls.iter().enumerate() {
            open.push((cc.clone(), ri, j));
        }
        let end = usize::from(row.props.controls_end).min(open.len());
        for k in 0..end {
            out.extend(close(&mut open, ri, k));
        }
        if ri + 1 == t.rows.len() {
            let mut k = end;
            while let Some(s) = close(&mut open, ri, k) {
                out.push(s);
                k += 1;
            }
        }
    }
    out
}

/// Remove rows `r0..=r1` keeping the row-level controls balanced: controls opening and closing
/// inside the range go with it; one opening inside and closing below opens again on the next
/// row, one opening above and closing inside closes after the previous row.
pub(crate) fn drain_rows_keep_controls(t: &mut Table, r0: usize, r1: usize) {
    if r1 < r0 || r1 >= t.rows.len() {
        return;
    }
    if t.rows.iter().all(|r| r.props.controls.is_empty()) {
        t.rows.drain(r0..=r1);
        return;
    }
    let spans = row_spans(t);
    // Opening inside, closing below: re-open on the row after the range (outermost first).
    let mut reopen: Vec<(usize, usize, ContentControl)> = spans
        .iter()
        .filter(|s| (r0..=r1).contains(&s.first) && s.last > r1)
        .map(|s| (s.first, s.idx, s.cc.clone()))
        .collect();
    reopen.sort_by_key(|(f, i, _)| (*f, *i));
    // Opening above, closing inside: close after the row before the range.
    let reclose = spans
        .iter()
        .filter(|s| s.first < r0 && (r0..=r1).contains(&s.last))
        .count();
    t.rows.drain(r0..=r1);
    if let Some(next) = t.rows.get_mut(r0) {
        let mut cs: Vec<ContentControl> = reopen.into_iter().map(|(_, _, c)| c).collect();
        cs.append(&mut next.props.controls);
        next.props.controls = cs;
    }
    if reclose > 0 {
        if let Some(prev) = r0.checked_sub(1).and_then(|i| t.rows.get_mut(i)) {
            let add = u8::try_from(reclose).unwrap_or(u8::MAX);
            prev.props.controls_end = prev.props.controls_end.saturating_add(add);
        }
    }
}

/// Give every content control in `blocks` (inline and block-level ones in the cells' paragraphs,
/// row-level ones of nested tables) a new id after `next`, drop data bindings and the source XML
/// (copies are written from the model).
fn renumber(blocks: &mut [Block], next: &mut i64) {
    for b in blocks {
        match b {
            Block::Paragraph(p) => {
                p.src = None;
                for r in &mut p.runs {
                    for c in &mut r.content {
                        if let RunContent::ControlStart(cc) = c {
                            *next += 1;
                            cc.id = Some(*next);
                            cc.data_binding = None;
                        }
                    }
                }
            }
            Block::Table(t) => {
                t.src = None;
                for row in &mut t.rows {
                    renumber_row(row, next);
                }
            }
            Block::Opaque(_) => {}
        }
    }
}

fn renumber_row(row: &mut Row, next: &mut i64) {
    for cc in &mut row.props.controls {
        *next += 1;
        cc.id = Some(*next);
        cc.data_binding = None;
    }
    for c in &mut row.cells {
        renumber(&mut c.blocks, next);
    }
}

/// Highest row-level control id in `blocks` (0 when none).
pub(crate) fn max_row_control_id(blocks: &[Block]) -> i64 {
    let mut m = 0;
    for b in blocks {
        if let Block::Table(t) = b {
            for row in &t.rows {
                for cc in &row.props.controls {
                    m = m.max(cc.id.unwrap_or(0));
                }
                for c in &row.cells {
                    m = m.max(max_row_control_id(&c.blocks));
                }
            }
        }
    }
    m
}

fn content_locked(cc: &ContentControl) -> bool {
    matches!(
        cc.lock.as_deref(),
        Some("contentLocked" | "sdtContentLocked")
    )
}

impl Session {
    /// The innermost row-level repeating-section item around the caret's table row, with its
    /// section, the table and where it is.
    fn row_item_at(&self) -> Option<(CellAt, Table, RowSpan, RowSpan)> {
        let at = cell_at(&self.doc, self.selection.focus.para)?;
        let t = table_of(&self.doc, &at)?.clone();
        let spans = row_spans(&t);
        let item = spans
            .iter()
            .filter(|s| {
                s.cc.kind == ControlKind::RepeatingSectionItem
                    && s.first <= at.row
                    && at.row <= s.last
            })
            .max_by_key(|s| s.depth)?
            .clone();
        let section = spans
            .iter()
            .filter(|s| {
                s.cc.kind == ControlKind::RepeatingSection
                    && s.depth < item.depth
                    && s.first <= item.first
                    && item.last <= s.last
            })
            .max_by_key(|s| s.depth)?
            .clone();
        Some((at, t, item, section))
    }

    /// The row-level control around the caret's table row (the innermost; an item of a
    /// repeating section stands for its section).
    pub(crate) fn row_control_at_caret(&self) -> Option<ContentControl> {
        if let Some((_, _, _, section)) = self.row_item_at() {
            return Some(section.cc);
        }
        let at = cell_at(&self.doc, self.selection.focus.para)?;
        let t = table_of(&self.doc, &at)?;
        row_spans(t)
            .into_iter()
            .filter(|s| s.first <= at.row && at.row <= s.last)
            .max_by_key(|s| s.depth)
            .map(|s| s.cc)
    }

    /// The caret is in a row-level repeating-section item.
    pub(crate) fn in_repeating_row_item(&self) -> bool {
        self.row_item_at().is_some()
    }

    /// Add a copy of the row-level item around the caret after it (new control ids); the caret
    /// goes to the first cell of the new item. One undo step.
    pub(crate) fn add_repeating_row_item(&mut self) -> Result<bool> {
        let Some((at, old, item, section)) = self.row_item_at() else {
            return Ok(false);
        };
        if content_locked(&section.cc) {
            return Ok(false);
        }
        let mut next = max_control_id(&self.doc);
        let mut copies: Vec<Row> = old.rows[item.first..=item.last].to_vec();
        // The copy opens only the item (and what opens inside it) and takes over the controls
        // the item's last row closed after the item; the original item then closes only its own.
        let last_end = old.rows[item.last].props.controls_end;
        if let Some(first) = copies.first_mut() {
            first.props.controls.drain(..item.idx);
        }
        for r in &mut copies {
            r.props.ins = None;
            r.props.del = None;
            r.props.change = None;
            r.props.header = false;
            renumber_row(r, &mut next);
        }
        if let Some(last) = copies.last_mut() {
            last.props.controls_end = last_end;
        }
        let mut t = old.clone();
        t.src = None;
        t.rows[item.last].props.controls_end = u8::try_from(item.closes).unwrap_or(u8::MAX);
        let at_row = item.last + 1;
        let container = at.container.clone();
        let index = at.index;
        self.edit(move |tx, _| {
            let mut copies = copies;
            if let Some(tr) = tx.track.as_mut() {
                for r in &mut copies {
                    r.props.ins = Some(tr.mark());
                }
            }
            t.rows.splice(at_row..at_row, copies);
            tx.push(Op::Blocks {
                path: container.clone(),
                index,
                remove: vec![Block::Table(old)],
                insert: vec![Block::Table(t)],
            })?;
            let mut path = container;
            path.extend([index, at_row, 0, 0]);
            let id = crate::id_of(tx.doc, &path).ok_or(EditError::BadPath)?;
            Ok(Selection::caret(Pos::new(id, 0)))
        })?;
        self.fix_selection();
        Ok(true)
    }

    /// Delete the row-level item around the caret — not the last item of its section. The caret
    /// goes to the first cell of the item after it (else the one before). One undo step.
    pub(crate) fn delete_repeating_row_item(&mut self) -> Result<bool> {
        let Some((at, old, item, section)) = self.row_item_at() else {
            return Ok(false);
        };
        if content_locked(&section.cc) {
            return Ok(false);
        }
        if self.track {
            return Err(EditError::Unsupported(
                "deleting a repeating section item is not recorded as a tracked change".into(),
            ));
        }
        let items = row_spans(&old)
            .into_iter()
            .filter(|s| {
                s.cc.kind == ControlKind::RepeatingSectionItem
                    && s.depth == item.depth
                    && section.first <= s.first
                    && s.last <= section.last
            })
            .count();
        if items < 2 {
            return Ok(false);
        }
        let mut t = old.clone();
        t.src = None;
        drain_rows_keep_controls(&mut t, item.first, item.last);
        let caret_row = if item.last == section.last {
            item.first.saturating_sub(1)
        } else {
            item.first
        };
        let container = at.container.clone();
        let index = at.index;
        self.edit(move |tx, _| {
            tx.push(Op::Blocks {
                path: container.clone(),
                index,
                remove: vec![Block::Table(old)],
                insert: vec![Block::Table(t)],
            })?;
            let mut path = container;
            path.extend([index, caret_row, 0, 0]);
            let id = crate::id_of(tx.doc, &path).ok_or(EditError::BadPath)?;
            Ok(Selection::caret(Pos::new(id, 0)))
        })?;
        self.fix_selection();
        Ok(true)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use bo_model::index::{body_paragraphs, para_at};
    use bo_model::{Cell, Document, Paragraph, Run, RunProps, Section};

    fn cc(kind: ControlKind, id: i64) -> ContentControl {
        ContentControl {
            id: Some(id),
            alias: None,
            tag: None,
            kind,
            lock: None,
            placeholder: false,
            block: true,
            props_xml: None,
            data_binding: None,
            appearance: None,
            color: None,
            temporary: false,
        }
    }

    fn row(text: &str) -> Row {
        Row {
            cells: vec![Cell {
                blocks: vec![Block::Paragraph(Paragraph {
                    runs: vec![Run {
                        props: RunProps::default(),
                        content: vec![RunContent::Text(text.into())],
                    }],
                    ..Paragraph::default()
                })],
                ..Cell::default()
            }],
            ..Row::default()
        }
    }

    /// Header row, a repeating section of two one-row items, a total row.
    fn doc() -> Document {
        let mut rows = vec![row("Đầu"), row("An"), row("Bình"), row("Cuối")];
        rows[1].props.controls = vec![
            cc(ControlKind::RepeatingSection, 10),
            cc(ControlKind::RepeatingSectionItem, 11),
        ];
        rows[1].props.controls_end = 1;
        rows[2].props.controls = vec![cc(ControlKind::RepeatingSectionItem, 12)];
        rows[2].props.controls_end = 2;
        Document {
            sections: vec![Section {
                blocks: vec![
                    Block::Table(Table {
                        grid: vec![bo_model::Twips(2000)],
                        rows,
                        ..Table::default()
                    }),
                    Block::Paragraph(Paragraph::default()),
                ],
                ..Section::default()
            }],
            ..Document::default()
        }
    }

    fn table(s: &Session) -> Table {
        match &s.doc().sections[0].blocks[0] {
            Block::Table(t) => t.clone(),
            _ => panic!("table"),
        }
    }

    fn texts(s: &Session) -> Vec<String> {
        body_paragraphs(s.doc())
            .iter()
            .filter_map(|p| para_at(s.doc(), p))
            .map(Paragraph::plain_text)
            .collect()
    }

    fn shape(t: &Table) -> Vec<(Vec<i64>, u8)> {
        t.rows
            .iter()
            .map(|r| {
                (
                    r.props.controls.iter().map(|c| c.id.unwrap_or(0)).collect(),
                    r.props.controls_end,
                )
            })
            .collect()
    }

    #[test]
    fn spans_of_rows() {
        let spans = row_spans(&table(&Session::new(doc())));
        let v: Vec<_> = spans
            .iter()
            .map(|s| (s.cc.id, s.first, s.idx, s.last, s.depth, s.closes))
            .collect();
        assert_eq!(
            v,
            vec![
                (Some(11), 1, 1, 1, 1, 1),
                (Some(12), 2, 0, 2, 1, 1),
                (Some(10), 1, 0, 2, 0, 2),
            ]
        );
    }

    /// "+" in the last item copies its row after it (new id); the copy closes the section; one
    /// undo step removes it.
    #[test]
    fn add_item_after_the_last_row_item() {
        let mut s = Session::new(doc());
        // Paragraph 2 = "Bình".
        s.set_selection(Selection::caret(Pos::new(2, 1)));
        assert!(s.in_repeating_item());
        assert_eq!(
            s.block_control_at_caret().map(|c| c.kind),
            Some(ControlKind::RepeatingSection)
        );
        assert!(s.add_repeating_item().unwrap());
        assert_eq!(texts(&s), ["Đầu", "An", "Bình", "Bình", "Cuối", ""]);
        assert_eq!(s.selection().focus, Pos::new(3, 0));
        assert_eq!(
            shape(&table(&s)),
            vec![
                (vec![], 0),
                (vec![10, 11], 1),
                (vec![12], 1),
                (vec![13], 2),
                (vec![], 0)
            ]
        );
        assert!(s.undo());
        assert_eq!(texts(&s), ["Đầu", "An", "Bình", "Cuối", ""]);
        assert_eq!(table(&s), table(&Session::new(doc())));
    }

    /// "+" in the first item: the copy opens only an item, the section's start stays put.
    #[test]
    fn add_item_after_the_first_row_item() {
        let mut s = Session::new(doc());
        s.set_selection(Selection::caret(Pos::new(1, 0)));
        assert!(s.add_repeating_item().unwrap());
        assert_eq!(texts(&s), ["Đầu", "An", "An", "Bình", "Cuối", ""]);
        assert_eq!(
            shape(&table(&s)),
            vec![
                (vec![], 0),
                (vec![10, 11], 1),
                (vec![13], 1),
                (vec![12], 2),
                (vec![], 0)
            ]
        );
    }

    /// Deleting the first item moves the section's start to the next item; the last item left
    /// cannot be deleted; deleting the last item closes the section after the one before.
    #[test]
    fn delete_row_items() {
        let mut s = Session::new(doc());
        s.set_selection(Selection::caret(Pos::new(1, 0)));
        assert!(s.delete_repeating_item().unwrap());
        assert_eq!(texts(&s), ["Đầu", "Bình", "Cuối", ""]);
        assert_eq!(
            shape(&table(&s)),
            vec![(vec![], 0), (vec![10, 12], 2), (vec![], 0)]
        );
        assert_eq!(s.selection().focus.para, 1);
        assert!(!s.delete_repeating_item().unwrap());
        assert!(s.undo());
        assert_eq!(table(&s), table(&Session::new(doc())));
        s.set_selection(Selection::caret(Pos::new(2, 0)));
        assert!(s.delete_repeating_item().unwrap());
        assert_eq!(texts(&s), ["Đầu", "An", "Cuối", ""]);
        assert_eq!(
            shape(&table(&s)),
            vec![(vec![], 0), (vec![10, 11], 2), (vec![], 0)]
        );
        assert_eq!(s.selection().focus.para, 1);
    }

    /// Table commands keep the row-level controls balanced: a row inserted inside an item has no
    /// controls of its own; deleting the rows of a whole item drops its control only.
    #[test]
    fn table_commands_keep_controls_balanced() {
        let mut s = Session::new(doc());
        s.set_selection(Selection::caret(Pos::new(1, 0)));
        s.edit_table(&crate::table::TableEdit::InsertRow { below: true })
            .unwrap();
        assert_eq!(
            shape(&table(&s)),
            vec![
                (vec![], 0),
                (vec![10, 11], 1),
                (vec![], 0),
                (vec![12], 2),
                (vec![], 0)
            ]
        );
        assert!(s.undo());
        s.set_selection(Selection::caret(Pos::new(1, 0)));
        s.edit_table(&crate::table::TableEdit::DeleteRow).unwrap();
        assert_eq!(
            shape(&table(&s)),
            vec![(vec![], 0), (vec![10, 12], 2), (vec![], 0)]
        );
        let mut t = table(&Session::new(doc()));
        drain_rows_keep_controls(&mut t, 1, 2);
        assert_eq!(shape(&t), vec![(vec![], 0), (vec![], 0)]);
        let mut t = table(&Session::new(doc()));
        drain_rows_keep_controls(&mut t, 0, 1);
        assert_eq!(shape(&t), vec![(vec![10, 12], 2), (vec![], 0)]);
    }
}
