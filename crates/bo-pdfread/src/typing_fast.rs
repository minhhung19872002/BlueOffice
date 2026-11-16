//! BlueEditor typing in a paragraph: while a block edit runs, the page content is the edit's
//! prefix (the page with the paragraph's runs neutralised, wrapped in `q … Q`) plus one text
//! object for the paragraph. The prefix's drawing is kept, so each keystroke interprets only
//! the paragraph again instead of the whole page (same display list: the prefix leaves the
//! graphics state as it found it, which is checked).
use crate::interp::{Out, M};
use crate::RenderOptions;

/// The prefix of the block edit in progress and its drawing.
#[derive(Default)]
pub(crate) struct TypingCache {
    page: usize,
    prefix: Option<Vec<u8>>,
    /// Drawing of the prefix with the options, page matrix and hidden-layer count it used.
    pub drawn: Option<(RenderOptions, M, usize, Out)>,
}

impl TypingCache {
    /// A block edit of page `page` wrote `prefix` plus its paragraph.
    pub(crate) fn note(&mut self, page: usize, prefix: &[u8]) {
        if self.page == page && self.prefix.as_deref() == Some(prefix) {
            return;
        }
        self.page = page;
        self.prefix = Some(prefix.to_vec());
        self.drawn = None;
    }

    /// The block edit ended.
    pub(crate) fn clear(&mut self) {
        *self = Self::default();
    }

    /// Length of the prefix when `content` of page `index` is the prefix followed by more.
    pub(crate) fn split(&self, index: usize, content: &[u8]) -> Option<usize> {
        let p = self.prefix.as_ref()?;
        (self.page == index && content.len() > p.len() && content.starts_with(p)).then_some(p.len())
    }

    /// Whether the kept drawing was made with these settings.
    pub(crate) fn fresh(&self, opts: RenderOptions, base: M, hidden: usize) -> bool {
        matches!(&self.drawn, Some((o, b, h, _)) if *o == opts && *b == base && *h == hidden)
    }

    /// A copy of the kept drawing to append the paragraph to.
    pub(crate) fn drawing(&self) -> Option<Out> {
        self.drawn.as_ref().map(|d| Out {
            commands: d.3.commands.clone(),
            images: d.3.images.clone(),
            chars: d.3.chars.clone(),
            objects: None,
        })
    }
}
