//! Headings collapsed in the editor (BlueDoc round 15): the view hides the top-level blocks
//! under them (`bo_layout::set_hidden_blocks`); the document is not changed.
use crate::Engine;

impl Engine {
    /// Hide these top-level body blocks (section, block) from now on (empty: show all). The
    /// next layout is complete; `false` when the list did not change.
    pub fn set_hidden_blocks(&mut self, blocks: Vec<(usize, usize)>) -> bool {
        let Some(key) = self.hidden_key() else {
            return false;
        };
        let mut sorted = blocks;
        sorted.sort_unstable();
        sorted.dedup();
        if sorted == bo_layout::hidden_blocks(key) {
            return false;
        }
        bo_layout::set_hidden_blocks(key, sorted);
        self.layout.reset();
        // A complete layout now (the view takes it with its next state).
        let _ = self.relayout();
        true
    }

    /// The blocks hidden now (sorted).
    pub fn hidden_blocks(&self) -> Vec<(usize, usize)> {
        self.hidden_key()
            .map(bo_layout::hidden_blocks)
            .unwrap_or_default()
    }

    /// The open document's address: hidden blocks belong to it alone.
    fn hidden_key(&self) -> Option<usize> {
        self.session
            .as_ref()
            .map(|s| std::ptr::from_ref(s.doc()) as usize)
    }
}

#[cfg(test)]
mod tests {
    use crate::edit::Command;
    use crate::Engine;
    use bo_layout::LineSource;

    fn lines(e: &Engine) -> Vec<(u32, f32)> {
        e.layout()
            .expect("layout")
            .pages
            .iter()
            .flat_map(|p| &p.lines)
            .filter_map(|l| match l.source {
                LineSource::Body(id) => Some((id, l.top)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn hidden_blocks_take_no_room_and_keep_body_ids() {
        let mut e = Engine::with_bundled_fonts();
        e.new_document("a.docx").expect("new");
        for t in ["one", "two", "three"] {
            e.command(Command::InsertText { text: t.into() })
                .expect("text");
            e.command(Command::SplitParagraph).expect("split");
        }
        e.command(Command::State).expect("state");
        let before = lines(&e);
        assert_eq!(before.len(), 4);
        assert!(e.set_hidden_blocks(vec![(0, 1)]));
        assert!(!e.set_hidden_blocks(vec![(0, 1)]));
        e.command(Command::State).expect("state");
        let after = lines(&e);
        assert_eq!(after.iter().map(|l| l.0).collect::<Vec<_>>(), vec![0, 2, 3]);
        // "three" moved up into the room of "two".
        assert!((after[1].1 - before[1].1).abs() < 0.01);
        // Typing after the hidden paragraph keeps it hidden (incremental layout).
        e.command(Command::InsertText { text: "x".into() })
            .expect("text");
        assert_eq!(
            lines(&e).iter().map(|l| l.0).collect::<Vec<_>>(),
            vec![0, 2, 3]
        );
        assert!(e.set_hidden_blocks(Vec::new()));
        e.command(Command::State).expect("state");
        assert_eq!(lines(&e).len(), 4);
    }
}
