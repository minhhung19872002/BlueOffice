//! Moving the slide clipboard between engines (BlueSlide windows in different processes):
//! the copying window sends its deck and what was copied; the pasting window opens that deck
//! in a scratch engine, copies the same slides or shapes there and takes the clipboard over,
//! so the paste brings pictures, layouts and themes along as a paste from another deck does
//! (ECMA-376 Part 2, §9.3 relationships; Part 1, §19.3.1.39 sldLayout).

use super::clip::{Clip, ClipKind};
use super::SlideCommand;

/// What a copy in another window held.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Copied {
    /// Whole slides (0-based indices).
    Slides(Vec<usize>),
    /// Shapes of one slide by id (`cNvPr@id`).
    Shapes {
        /// 0-based slide index.
        slide: usize,
        /// Shape ids.
        ids: Vec<u32>,
    },
}

impl crate::Engine {
    /// A small presentation holding only what the slide clipboard holds (the copied slides
    /// with their layouts, masters and media, or the copied shapes on one blank slide), and
    /// what to copy from it: what another window needs to paste, instead of the whole deck.
    pub fn slide_clip_deck(&self) -> Option<(Vec<u8>, Copied)> {
        let clip = self.slide_clip.clone()?;
        let slides = match &clip.kind {
            ClipKind::Slides(s) => Some(s.len()),
            ClipKind::Shapes { .. } => None,
        };
        let mut other = crate::Engine::new();
        let blank = super::album::new_deck("").ok()?;
        other.open(&blank, "clipboard.pptx").ok()?;
        other.set_foreign_slide_clip(clip);
        let r = other
            .slide_command(SlideCommand::Paste {
                keep_source: slides.is_some(),
            })
            .ok()
            .filter(|r| r.error.is_none())?;
        let copied = match slides {
            Some(n) => {
                // The pasted slides follow the blank first slide, which goes.
                other
                    .slide_command(SlideCommand::DeleteSlide { index: Some(0) })
                    .ok()?;
                Copied::Slides((0..n).collect())
            }
            None => Copied::Shapes {
                slide: 0,
                ids: r.selection.iter().map(|s| s.id).collect(),
            },
        };
        Some((other.save_pptx().ok()?, copied))
    }

    /// Take the slide clipboard out of this engine.
    pub fn take_slide_clip(&mut self) -> Option<Clip> {
        self.slide_clip.take()
    }

    /// Put `clip`, copied in another engine, on this engine's slide clipboard. It is
    /// marked as coming from another deck, so pasting imports what it references.
    pub fn set_foreign_slide_clip(&mut self, mut clip: Clip) {
        clip.deck = u64::MAX;
        clip.pastes = 0;
        self.slide_clip = Some(clip);
    }

    /// Copy `what` from the presentation `bytes` (another window's deck) onto this engine's
    /// slide clipboard; `false` when the deck cannot be read or nothing was copied.
    pub fn copy_from_deck(&mut self, bytes: &[u8], what: &Copied) -> bool {
        // Copying moves parts; no text is laid out, so no fonts are needed.
        let mut other = crate::Engine::new();
        if other.open(bytes, "clipboard.pptx").is_err() {
            return false;
        }
        let ok = match what {
            Copied::Slides(indices) => other
                .slide_command(SlideCommand::CopySlides {
                    indices: Some(indices.clone()),
                })
                .is_ok_and(|r| r.error.is_none()),
            Copied::Shapes { slide, ids } => {
                let went = other
                    .slide_command(SlideCommand::GoTo { slide: *slide })
                    .is_ok_and(|r| r.error.is_none())
                    && other
                        .slide_command(SlideCommand::Select { ids: ids.clone() })
                        .is_ok_and(|r| r.error.is_none());
                went && other
                    .slide_command(SlideCommand::Copy)
                    .is_ok_and(|r| r.error.is_none())
            }
        };
        match other.take_slide_clip() {
            Some(clip) if ok => {
                self.set_foreign_slide_clip(clip);
                true
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn deck(name: &str) -> Vec<u8> {
        let path = format!(
            "{}/../../tests/corpus/slide/{name}",
            env!("CARGO_MANIFEST_DIR")
        );
        std::fs::read(path).unwrap()
    }

    #[test]
    fn slides_and_shapes_cross_engines() {
        let src = deck("slide-text-bullets.pptx");
        let mut a = crate::Engine::with_bundled_fonts();
        a.open(&deck("slide-edit-slides.pptx"), "a.pptx").unwrap();
        let count = |e: &mut crate::Engine| e.presentation().map_or(0, |p| p.slides.len());
        let n = count(&mut a);
        assert!(a.copy_from_deck(&src, &Copied::Slides(vec![0, 1])));
        assert!(a
            .slide_command(SlideCommand::Paste { keep_source: false })
            .is_ok_and(|r| r.error.is_none()));
        assert_eq!(count(&mut a), n + 2);
        // Shapes: the first shape of slide 1.
        let mut b = crate::Engine::with_bundled_fonts();
        b.open(&src, "b.pptx").unwrap();
        let id = b
            .slide_command(SlideCommand::SelectAll)
            .ok()
            .and_then(|r| r.selection.first().map(|s| s.id))
            .expect("a shape");
        let shapes = |e: &crate::Engine| {
            e.presentation()
                .map_or(0, |p| p.slides[0].data.shapes.len())
        };
        a.slide_command(SlideCommand::GoTo { slide: 0 }).unwrap();
        let before = shapes(&a);
        assert!(a.copy_from_deck(
            &src,
            &Copied::Shapes {
                slide: 0,
                ids: vec![id]
            }
        ));
        a.slide_command(SlideCommand::Paste { keep_source: false })
            .unwrap();
        assert_eq!(shapes(&a), before + 1);
        assert!(!a.copy_from_deck(b"not a deck", &Copied::Slides(vec![0])));
    }

    #[test]
    fn clipboard_deck_carries_only_what_was_copied() {
        let src = deck("slide-album-photos.pptx");
        let mut b = crate::Engine::with_bundled_fonts();
        b.open(&src, "b.pptx").unwrap();
        // One slide of five.
        b.slide_command(SlideCommand::CopySlides {
            indices: Some(vec![1]),
        })
        .unwrap();
        let (bytes, what) = b.slide_clip_deck().expect("deck");
        assert_eq!(what, Copied::Slides(vec![0]));
        assert!(bytes.len() < src.len(), "{} < {}", bytes.len(), src.len());
        let mut a = crate::Engine::with_bundled_fonts();
        a.open(&deck("slide-text-bullets.pptx"), "a.pptx").unwrap();
        let n = a.presentation().map_or(0, |p| p.slides.len());
        assert!(a.copy_from_deck(&bytes, &what));
        a.slide_command(SlideCommand::Paste { keep_source: false })
            .unwrap();
        assert_eq!(a.presentation().map_or(0, |p| p.slides.len()), n + 1);
        // Shapes travel on one blank slide.
        b.slide_command(SlideCommand::GoTo { slide: 0 }).unwrap();
        b.slide_command(SlideCommand::SelectAll).unwrap();
        b.slide_command(SlideCommand::Copy).unwrap();
        let (bytes, what) = b.slide_clip_deck().expect("deck");
        let Copied::Shapes { slide: 0, ids } = &what else {
            panic!("{what:?}");
        };
        assert_ne!(ids.len(), 0);
        assert!(bytes.len() < src.len());
        assert!(a.copy_from_deck(&bytes, &what));
    }
}
