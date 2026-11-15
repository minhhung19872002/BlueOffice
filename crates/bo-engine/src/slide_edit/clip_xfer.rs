//! Moving the slide clipboard between engines (BlueSlide windows in different processes):
//! the copying window sends its deck and what was copied; the pasting window opens that deck
//! in a scratch engine, copies the same slides or shapes there and takes the clipboard over,
//! so the paste brings pictures, layouts and themes along as a paste from another deck does
//! (ECMA-376 Part 2, §9.3 relationships; Part 1, §19.3.1.39 sldLayout).

use super::clip::Clip;
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
        let mut other = crate::Engine::with_bundled_fonts();
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
}
