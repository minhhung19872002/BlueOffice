//! Mailings › Finish & Merge › Edit Individual Documents (BlueDoc round 16): the merged
//! document of all records or a range of them, written as a new `.docx` for a new window —
//! the open document (the main document with its merge fields) stays as it is, as on the
//! reference word processor. Fields: ECMA-376 Part 1, §17.16.5.35 MERGEFIELD.
use crate::{Engine, EngineError, Result};

impl Engine {
    /// The merged document of records `from..=to` of the recipient list (1-based, all when
    /// `None`) as `.docx` bytes; the open document is not changed.
    pub fn merged_docx(&self, range: Option<(usize, usize)>) -> Result<Vec<u8>> {
        let (Some(m), Some(s)) = (&self.merge, self.session.as_ref()) else {
            return Err(EngineError::BadCommand("mail merge: no data source".into()));
        };
        let order: Vec<usize> = match range {
            Some((a, b)) => self
                .merge_order
                .iter()
                .enumerate()
                .filter(|(i, _)| *i + 1 >= a.max(1) && *i < b)
                .map(|(_, r)| *r)
                .collect(),
            None => self.merge_order.clone(),
        };
        if order.is_empty() {
            return Err(EngineError::BadCommand("mail merge: no records".into()));
        }
        let run = bo_edit::merge::rules::MergeRun {
            data: m,
            order: &order,
            answers: &self.merge_answers,
        };
        let merged =
            bo_edit::merge::combine(s.doc(), bo_edit::merge::rules::merge_each(s.doc(), run));
        Ok(bo_docx::write_docx(&merged)?)
    }
}

#[cfg(test)]
mod tests {
    use crate::edit::Command;
    use crate::Engine;

    #[test]
    fn edit_individual_documents_leaves_the_main_document() {
        let mut e = Engine::with_bundled_fonts();
        e.new_document("letter.docx").expect("new");
        e.command(Command::InsertText {
            text: "Dear ".into(),
        })
        .expect("text");
        e.command(Command::MergeTable {
            name: "list".into(),
            fields: vec!["Name".into()],
            rows: vec![vec!["Ann".into()], vec!["Bob".into()], vec!["Cy".into()]],
        })
        .expect("data");
        e.command(Command::MergeInsertField {
            name: "Name".into(),
        })
        .expect("field");
        let all = e.merged_docx(None).expect("all");
        let two = e.merged_docx(Some((2, 3))).expect("2-3");
        let text = |b: &[u8]| {
            let d = bo_docx::read_docx(b).expect("read");
            d.plain_text()
        };
        let t = text(&all);
        assert!(
            t.contains("Dear Ann") && t.contains("Dear Bob") && t.contains("Dear Cy"),
            "{t}"
        );
        let t = text(&two);
        assert!(
            !t.contains("Dear Ann") && t.contains("Dear Bob") && t.contains("Dear Cy"),
            "{t}"
        );
        // The open document still has its field.
        let main = e
            .document()
            .map(bo_model::Document::plain_text)
            .unwrap_or_default();
        assert!(!main.contains("Dear Bob"), "{main}");
    }
}
