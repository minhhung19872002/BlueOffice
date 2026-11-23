//! Blocks a view hides (BlueDoc round 15: headings collapsed in the editor, the reference word
//! processor's "Expand / Collapse" of a heading's content). View state only — the document is
//! not changed; the layout skips the listed top-level blocks of the body as if they took no
//! room, while their paragraphs keep their body ids (`LineSource::Body`).
use bo_model::{Block, Document};
use std::cell::RefCell;

thread_local! {
    /// The document they belong to (its address, so a scratch document laid out on the
    /// same thread is not touched) and the (section, block) pairs skipped, sorted.
    static HIDDEN: RefCell<(usize, Vec<(usize, usize)>)> = const { RefCell::new((0, Vec::new())) };
}

/// Hide these top-level body blocks (section index, block index) of the document at address
/// `doc` (`std::ptr::from_ref(doc) as usize`) in later layouts on this thread; an empty list
/// shows everything.
pub fn set_hidden_blocks(doc: usize, mut blocks: Vec<(usize, usize)>) {
    blocks.sort_unstable();
    blocks.dedup();
    HIDDEN.with(|h| *h.borrow_mut() = (doc, blocks));
}

/// The blocks hidden for the document at address `doc` (sorted).
pub fn hidden_blocks(doc: usize) -> Vec<(usize, usize)> {
    HIDDEN.with(|h| {
        let h = h.borrow();
        if h.0 == doc {
            h.1.clone()
        } else {
            Vec::new()
        }
    })
}

/// Whether block `k` of section `si` of `doc` is hidden.
pub(crate) fn is_hidden(doc: &Document, si: usize, k: usize) -> bool {
    HIDDEN.with(|h| {
        let h = h.borrow();
        !h.1.is_empty()
            && h.0 == std::ptr::from_ref(doc) as usize
            && h.1.binary_search(&(si, k)).is_ok()
    })
}

/// Body ids a block takes (paragraphs, those of table cells included), as
/// `bo_model::index::body_paragraphs` counts them.
pub(crate) fn para_count(b: &Block) -> u32 {
    match b {
        Block::Paragraph(_) => 1,
        Block::Table(t) => t
            .rows
            .iter()
            .flat_map(|r| &r.cells)
            .flat_map(|c| &c.blocks)
            .map(para_count)
            .sum(),
        Block::Opaque(_) => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hidden_blocks_belong_to_one_document_on_one_thread() {
        let (doc, other) = (Document::default(), Document::default());
        let key = std::ptr::from_ref(&doc) as usize;
        set_hidden_blocks(key, vec![(0, 5), (0, 2), (0, 2)]);
        assert_eq!(hidden_blocks(key), vec![(0, 2), (0, 5)]);
        assert!(is_hidden(&doc, 0, 2) && is_hidden(&doc, 0, 5) && !is_hidden(&doc, 0, 3));
        assert!(!is_hidden(&other, 0, 2), "another document");
        std::thread::spawn(|| {
            let d = Document::default();
            assert!(!is_hidden(&d, 0, 2));
        })
        .join()
        .expect("thread");
        set_hidden_blocks(key, Vec::new());
        assert!(!is_hidden(&doc, 0, 2));
    }
}
