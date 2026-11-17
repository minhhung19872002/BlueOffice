//! Outline View, round 17 (N6): a click chooses a line (a slide title or one of its body
//! paragraphs); Tab / Alt+Shift+Right demote it, Shift+Tab / Alt+Shift+Left promote it, as
//! on the reference presentation app:
//! - a body paragraph moves one level down / up (levels 0–8, §21.1.2.2.7 `pPr@lvl`);
//! - promoting a first-level paragraph makes it the title of a new slide that takes the
//!   paragraphs after it (one level up);
//! - demoting a title (not the first slide's) makes it a first-level paragraph of the slide
//!   before, its paragraphs one level down.
//!
//! The edited outline goes back in one undoable step (`applyOutline`).
use super::viewtab::Pane;
use super::SlideWindow;
use crate::{Key, Mods, Painter};
use bo_engine::slide_edit::outline::{self, OutlineBullet, OutlineEdit, OutlineSlide, MAX_LEVEL};
use bo_engine::slide_edit::views::ViewCommand;

/// A line of the outline: slide, and its body paragraph (`None`: the title).
pub(super) type Line = (usize, Option<usize>);

/// The edited outline after demoting (`demote`) or promoting `line`, and the entry and line
/// chosen afterwards; `None` when the line cannot move that way.
pub(super) fn shifted(
    slides: &[OutlineSlide],
    line: Line,
    demote: bool,
) -> Option<(Vec<OutlineEdit>, Line)> {
    let mut edits: Vec<OutlineEdit> = slides
        .iter()
        .enumerate()
        .map(|(i, o)| OutlineEdit {
            slide: Some(i),
            title: o.title.clone(),
            bullets: o.bullets.clone(),
        })
        .collect();
    let (s, b) = line;
    let entry = edits.get(s)?;
    match (b, demote) {
        (Some(b), true) => {
            let lv = &mut edits[s].bullets.get_mut(b)?.level;
            if *lv >= MAX_LEVEL {
                return None;
            }
            *lv += 1;
            Some((edits, (s, Some(b))))
        }
        (Some(b), false) => {
            let level = entry.bullets.get(b)?.level;
            if level > 0 {
                edits[s].bullets[b].level -= 1;
                return Some((edits, (s, Some(b))));
            }
            // A first-level paragraph becomes the title of a new slide after this one.
            let rest: Vec<OutlineBullet> = edits[s].bullets.split_off(b);
            let title = rest[0].text.clone();
            let bullets = rest[1..]
                .iter()
                .map(|p| OutlineBullet {
                    text: p.text.clone(),
                    level: p.level.saturating_sub(1),
                })
                .collect();
            edits.insert(
                s + 1,
                OutlineEdit {
                    slide: None,
                    title,
                    bullets,
                },
            );
            Some((edits, (s + 1, None)))
        }
        (None, true) if s > 0 => {
            let gone = edits.remove(s);
            let prev = &mut edits[s - 1];
            let at = prev.bullets.len();
            prev.bullets.push(OutlineBullet {
                text: gone.title,
                level: 0,
            });
            prev.bullets
                .extend(gone.bullets.into_iter().map(|p| OutlineBullet {
                    text: p.text,
                    level: (p.level + 1).min(MAX_LEVEL),
                }));
            Some((edits, (s - 1, Some(at))))
        }
        _ => None,
    }
}

impl SlideWindow {
    /// The outline line at window position `y` (listed lines from the last layout).
    pub(super) fn outline_line_at(&self, y: f32) -> Option<Line> {
        self.outline9
            .hits
            .iter()
            .rev()
            .find(|h| y >= h.y)
            .map(|h| h.line)
    }

    /// Demote / promote the chosen outline line.
    pub(super) fn outline_shift(&mut self, demote: bool) -> bool {
        let Some(line) = self.outline_sel else {
            return false;
        };
        let Some(pres) = self.view.engine().presentation() else {
            return false;
        };
        let slides = outline::outline(pres);
        let Some((edits, next)) = shifted(&slides, line, demote) else {
            return false;
        };
        let current = Some(next.0);
        let done = self.view.run_view(ViewCommand::ApplyOutline {
            slides: edits,
            current,
        });
        if done {
            self.outline_sel = Some(next);
            self.relayout_pending = true;
        }
        done
    }

    /// Keys while the outline has a chosen line: `None` when not for it.
    pub(super) fn outline_key(&mut self, k: Key, m: Mods) -> Option<bool> {
        if self.pane != Pane::Outline || self.outline_sel.is_none() {
            return None;
        }
        match k {
            Key::Tab if !m.ctrl && !m.alt => Some(self.outline_shift(!m.shift)),
            Key::Right if m.alt && m.shift => Some(self.outline_shift(true)),
            Key::Left if m.alt && m.shift => Some(self.outline_shift(false)),
            Key::Escape => {
                self.outline_sel = None;
                Some(true)
            }
            _ => self.outline_caret_key(k, m),
        }
    }

    /// The caret on the chosen outline line (round 19: the reference shows a caret, not a
    /// frame).
    pub(super) fn paint_outline_sel(&self, p: &mut dyn Painter) {
        self.paint_outline_caret(p);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn deck() -> Vec<OutlineSlide> {
        let b = |t: &str, level| OutlineBullet {
            text: t.into(),
            level,
        };
        vec![
            OutlineSlide {
                title: "One".into(),
                bullets: vec![b("a", 0), b("b", 0), b("c", 1)],
                has_title: true,
                has_body: true,
            },
            OutlineSlide {
                title: "Two".into(),
                bullets: vec![b("x", 0)],
                has_title: true,
                has_body: true,
            },
        ]
    }

    #[test]
    fn demote_and_promote() {
        let d = deck();
        // Demote a paragraph: one level down.
        let (e, l) = shifted(&d, (0, Some(0)), true).unwrap();
        assert_eq!(e[0].bullets[0].level, 1);
        assert_eq!(l, (0, Some(0)));
        // Promote a first-level paragraph: a new slide "b" taking "c" (now level 0).
        let (e, l) = shifted(&d, (0, Some(1)), false).unwrap();
        assert_eq!(e.len(), 3);
        assert_eq!(e[0].bullets.len(), 1);
        assert_eq!((e[1].slide, e[1].title.as_str()), (None, "b"));
        assert_eq!(e[1].bullets[0].text, "c");
        assert_eq!(e[1].bullets[0].level, 0);
        assert_eq!(l, (1, None));
        // Demote the second title: a paragraph of slide one, its paragraphs one level down.
        let (e, l) = shifted(&d, (1, None), true).unwrap();
        assert_eq!(e.len(), 1);
        assert_eq!(e[0].bullets[3].text, "Two");
        assert_eq!(e[0].bullets[4].level, 1);
        assert_eq!(l, (0, Some(3)));
        // The first title cannot be demoted; a title cannot be promoted.
        assert!(shifted(&d, (0, None), true).is_none());
        assert!(shifted(&d, (1, None), false).is_none());
    }
}
