//! Outline View, round 19 (N6): the outline's text is edited in place, as on the reference
//! presentation app. A press puts a caret in a slide title or body paragraph; then
//! - typing inserts at the caret; Backspace / Delete remove a character, or join the line
//!   with the one before / after (a paragraph into the one above or the title; a title into
//!   the last line of the slide before, which takes the slide's paragraphs);
//! - Enter splits a paragraph (the new one keeps its level), or splits a title into a new
//!   slide that takes the paragraphs after the caret (at the start of a title: a new empty
//!   slide before it);
//! - Left / Right / Home / End / Up / Down move the caret; Tab / Shift+Tab demote / promote
//!   (round 17, `slide_outline8.rs`).
//!
//! Each change goes back as one undoable `applyOutline` step (titles: §19.3.1.36 ph
//! `title`; paragraphs: the body placeholder, §21.1.2.2.7 `pPr@lvl` kept).
use super::outline8::Line;
use super::viewtab::Pane;
use super::SlideWindow;
use crate::{Key, Mods, Painter, TextMeasure};
use bo_engine::slide_edit::outline::{self, OutlineBullet, OutlineEdit, OutlineSlide};
use bo_engine::slide_edit::views::ViewCommand;
use bo_render::Rgba;

/// A listed outline line: which line, its top and its text pieces (text, x, bold, italic,
/// underline) without the automatic number.
#[derive(Debug, Clone, Default)]
pub(super) struct Hit {
    pub line: Line,
    pub y: f32,
    pub pieces: Vec<(String, f32, bool, bool, bool)>,
}

/// The caret of the outline (its line is the window's `outline_sel`).
#[derive(Debug, Clone, Default)]
pub(super) struct Caret {
    /// Character offset in the line's text.
    pub at: usize,
    /// A press / Up / Down waiting for the next layout to place the caret nearest this x.
    pub click_x: Option<f32>,
    /// Where the caret is drawn (x, y, height) after the last layout.
    pub rect: Option<[f32; 3]>,
    /// Listed lines in order, from the last layout.
    pub hits: Vec<Hit>,
}

/// An edit at the caret.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Op<'a> {
    Insert(&'a str),
    Back,
    Delete,
    Enter,
}

fn byte_at(s: &str, at: usize) -> usize {
    s.char_indices().nth(at).map_or(s.len(), |(i, _)| i)
}

fn len(s: &str) -> usize {
    s.chars().count()
}

/// The outline as edits that keep every slide.
pub(super) fn edits_of(slides: &[OutlineSlide]) -> Vec<OutlineEdit> {
    slides
        .iter()
        .enumerate()
        .map(|(i, o)| OutlineEdit {
            slide: Some(i),
            title: o.title.clone(),
            bullets: o.bullets.clone(),
        })
        .collect()
}

fn text_mut(e: &mut [OutlineEdit], (s, b): Line) -> Option<&mut String> {
    let entry = e.get_mut(s)?;
    match b {
        None => Some(&mut entry.title),
        Some(b) => entry.bullets.get_mut(b).map(|x| &mut x.text),
    }
}

/// The edited outline after `op` at character `at` of `line`, with the caret's line and
/// offset afterwards; `None` when nothing changes.
pub(super) fn edit(
    slides: &[OutlineSlide],
    line: Line,
    at: usize,
    op: Op<'_>,
) -> Option<(Vec<OutlineEdit>, Line, usize)> {
    let mut e = edits_of(slides);
    let text = text_mut(&mut e, line)?;
    let at = at.min(len(text));
    let (s, b) = line;
    match op {
        Op::Insert(t) => {
            // One line: breaks typed or pasted become spaces.
            let t: String = t
                .chars()
                .map(|c| if c == '\r' || c == '\n' { ' ' } else { c })
                .filter(|c| !c.is_control() || *c == '\u{b}')
                .collect();
            if t.is_empty() {
                return None;
            }
            text.insert_str(byte_at(text, at), &t);
            Some((e, line, at + len(&t)))
        }
        Op::Back if at > 0 => {
            let i = byte_at(text, at - 1);
            text.remove(i);
            Some((e, line, at - 1))
        }
        Op::Back => join_into_previous(e, line),
        Op::Delete if at < len(text) => {
            let i = byte_at(text, at);
            text.remove(i);
            Some((e, line, at))
        }
        Op::Delete => {
            // Join the next line into this one: Backspace at the next line's start.
            let next = match b {
                Some(b) if b + 1 < e[s].bullets.len() => (s, Some(b + 1)),
                None if !e[s].bullets.is_empty() => (s, Some(0)),
                _ if s + 1 < e.len() => (s + 1, None),
                _ => return None,
            };
            join_into_previous(e, next)
        }
        Op::Enter => {
            let tail = text.split_off(byte_at(text, at));
            match b {
                Some(b) => {
                    let level = e[s].bullets[b].level;
                    e[s].bullets
                        .insert(b + 1, OutlineBullet { text: tail, level });
                    Some((e, (s, Some(b + 1)), 0))
                }
                None if at == 0 && !tail.is_empty() => {
                    // At the start of a title: a new empty slide before this one.
                    *text_mut(&mut e, line)? = tail;
                    e.insert(
                        s,
                        OutlineEdit {
                            slide: None,
                            title: String::new(),
                            bullets: Vec::new(),
                        },
                    );
                    Some((e, (s + 1, None), 0))
                }
                None => {
                    // A new slide after this one, titled with the rest of the title, that
                    // takes this slide's paragraphs (the outline reads on).
                    let bullets = std::mem::take(&mut e[s].bullets);
                    e.insert(
                        s + 1,
                        OutlineEdit {
                            slide: None,
                            title: tail,
                            bullets,
                        },
                    );
                    Some((e, (s + 1, None), 0))
                }
            }
        }
    }
}

/// Backspace at the start of `line`: join it with the line before.
fn join_into_previous(
    mut e: Vec<OutlineEdit>,
    line: Line,
) -> Option<(Vec<OutlineEdit>, Line, usize)> {
    match line {
        (s, Some(b)) => {
            let gone = e[s].bullets.remove(b);
            let prev = if b == 0 { (s, None) } else { (s, Some(b - 1)) };
            let t = text_mut(&mut e, prev)?;
            let at = len(t);
            t.push_str(&gone.text);
            Some((e, prev, at))
        }
        (0, None) => None,
        (s, None) => {
            let gone = e.remove(s);
            let prev = &mut e[s - 1];
            // Hidden empty paragraphs (an empty body) do not count as the last line.
            let target = if prev.bullets.iter().all(|x| x.text.trim().is_empty()) {
                prev.bullets.clear();
                None
            } else {
                Some(prev.bullets.len() - 1)
            };
            let into = if let Some(k) = target {
                &mut prev.bullets[k].text
            } else {
                &mut prev.title
            };
            let at = len(into);
            into.push_str(&gone.title);
            prev.bullets
                .extend(gone.bullets.into_iter().filter(|x| !x.text.is_empty()));
            Some((e, (s - 1, target), at))
        }
    }
}

impl SlideWindow {
    /// Apply an edit at the outline caret.
    pub(super) fn outline_edit(&mut self, op: Op<'_>) -> bool {
        let Some(line) = self.outline_sel else {
            return false;
        };
        let Some(pres) = self.view.engine().presentation() else {
            return false;
        };
        let slides = outline::outline(pres);
        let Some((edits, next, at)) = edit(&slides, line, self.outline9.at, op) else {
            return false;
        };
        let done = self.view.run_view(ViewCommand::ApplyOutline {
            slides: edits,
            current: Some(next.0),
        });
        if done {
            self.outline_sel = Some(next);
            self.outline9.at = at;
            self.outline9.click_x = None;
            self.relayout_pending = true;
        }
        done
    }

    /// Text typed while the outline has a caret: `None` when not for it.
    pub(super) fn outline_text(&mut self, t: &str) -> Option<bool> {
        if self.pane != Pane::Outline || self.outline_sel.is_none() {
            return None;
        }
        Some(self.outline_edit(Op::Insert(t)))
    }

    /// Caret keys of the outline (after Tab / Alt+Shift+arrows): `None` when not for it.
    pub(super) fn outline_caret_key(&mut self, k: Key, m: Mods) -> Option<bool> {
        if m.ctrl || m.alt {
            return None;
        }
        let line = self.outline_sel?;
        let i = self.outline9.hits.iter().position(|h| h.line == line);
        let cur_len = i.map_or(0, |i| {
            self.outline9.hits[i]
                .pieces
                .iter()
                .map(|p| len(&p.0))
                .sum::<usize>()
        });
        let at = self.outline9.at.min(cur_len);
        let goto = |w: &mut Self, j: usize, at: Option<usize>, x: Option<f32>| -> bool {
            let Some(h) = w.outline9.hits.get(j) else {
                return false;
            };
            w.outline_sel = Some(h.line);
            let n: usize = h.pieces.iter().map(|p| len(&p.0)).sum();
            w.outline9.at = at.map_or(n, |a| a.min(n));
            w.outline9.click_x = x;
            w.view.go_to(h.line.0);
            true
        };
        match k {
            Key::Enter => Some(self.outline_edit(Op::Enter)),
            Key::Backspace => Some(self.outline_edit(Op::Back)),
            Key::Delete => Some(self.outline_edit(Op::Delete)),
            Key::Left if at > 0 => {
                self.outline9.at = at - 1;
                Some(true)
            }
            Key::Left => Some(i.is_some_and(|i| i > 0 && goto(self, i - 1, None, None))),
            Key::Right if at < cur_len => {
                self.outline9.at = at + 1;
                Some(true)
            }
            Key::Right => Some(i.is_some_and(|i| goto(self, i + 1, Some(0), None))),
            Key::Home => {
                self.outline9.at = 0;
                Some(true)
            }
            Key::End => {
                self.outline9.at = cur_len;
                Some(true)
            }
            Key::Up | Key::Down => {
                let x = self.outline9.rect.map(|r| r[0]);
                let j = match (k, i) {
                    (Key::Up, Some(i)) if i > 0 => i - 1,
                    (Key::Down, Some(i)) => i + 1,
                    _ => return Some(false),
                };
                Some(goto(self, j, Some(0), x))
            }
            _ => None,
        }
    }

    /// Place the caret after a layout: a pending press / Up / Down x picks the character
    /// boundary nearest it.
    pub(super) fn place_outline_caret(&mut self, text: &dyn TextMeasure, size: f32, line: f32) {
        self.outline9.rect = None;
        let Some(sel) = self.outline_sel else {
            return;
        };
        let Some(h) = self.outline9.hits.iter().find(|h| h.line == sel) else {
            return;
        };
        let font = |b: bool, i: bool, u: bool| crate::Font {
            family: "Segoe UI",
            size,
            weight: if b { 700 } else { 400 },
            italic: i,
            underline: u,
        };
        // Character boundaries of the line: x of each.
        let mut xs = Vec::new();
        for (t, x, b, i, u) in &h.pieces {
            let f = font(*b, *i, *u);
            if xs.is_empty() {
                xs.push(*x);
            }
            for (k, _) in t.char_indices().skip(1) {
                xs.push(x + text.measure_font(&t[..k], &f).0);
            }
            xs.push(x + text.measure_font(t, &f).0);
        }
        if xs.is_empty() {
            xs.push(h.pieces.first().map_or(0.0, |p| p.1));
        }
        if let Some(cx) = self.outline9.click_x.take() {
            self.outline9.at = xs
                .iter()
                .enumerate()
                .min_by(|a, b| (a.1 - cx).abs().total_cmp(&(b.1 - cx).abs()))
                .map_or(0, |(k, _)| k);
        }
        let at = self.outline9.at.min(xs.len() - 1);
        self.outline9.at = at;
        self.outline9.rect = Some([xs[at].round(), h.y, line]);
    }

    /// The outline's caret: a thin dark bar the height of the line.
    pub(super) fn paint_outline_caret(&self, p: &mut dyn Painter) {
        let Some([x, y, h]) = self.outline9.rect else {
            return;
        };
        let w = self.scale.round().max(1.0);
        p.fill_rect(x, y + 2.0, w, h - 4.0, Rgba(0, 0, 0, 255));
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
                bullets: vec![b("alpha", 0), b("beta", 1)],
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
    fn typing_and_deleting_characters() {
        let d = deck();
        let (e, l, at) = edit(&d, (0, Some(0)), 2, Op::Insert("ộ")).unwrap();
        assert_eq!(e[0].bullets[0].text, "alộpha");
        assert_eq!((l, at), ((0, Some(0)), 3));
        let (e, _, at) = edit(&d, (1, None), 3, Op::Back).unwrap();
        assert_eq!((e[1].title.as_str(), at), ("Tw", 2));
        let (e, _, at) = edit(&d, (1, None), 0, Op::Delete).unwrap();
        assert_eq!((e[1].title.as_str(), at), ("wo", 0));
        // Breaks typed into a line are spaces.
        let (e, _, _) = edit(&d, (1, None), 3, Op::Insert("a\r\nb")).unwrap();
        assert_eq!(e[1].title, "Twoa  b");
    }

    #[test]
    fn enter_splits_paragraphs_and_titles() {
        let d = deck();
        // A paragraph: the new one keeps its level.
        let (e, l, at) = edit(&d, (0, Some(1)), 2, Op::Enter).unwrap();
        assert_eq!(e[0].bullets[1].text, "be");
        assert_eq!(
            (e[0].bullets[2].text.as_str(), e[0].bullets[2].level),
            ("ta", 1)
        );
        assert_eq!((l, at), ((0, Some(2)), 0));
        // End of a title: a new slide that takes the paragraphs.
        let (e, l, _) = edit(&d, (0, None), 3, Op::Enter).unwrap();
        assert_eq!(e.len(), 3);
        assert_eq!(e[0].bullets.len(), 0);
        assert_eq!((e[1].slide, e[1].title.as_str()), (None, ""));
        assert_eq!(e[1].bullets.len(), 2);
        assert_eq!(l, (1, None));
        // Start of a title: a new empty slide before it.
        let (e, l, _) = edit(&d, (1, None), 0, Op::Enter).unwrap();
        assert_eq!((e[1].slide, e[2].slide), (None, Some(1)));
        assert_eq!(e[2].title, "Two");
        assert_eq!(l, (2, None));
    }

    #[test]
    fn backspace_joins_lines() {
        let d = deck();
        // A paragraph into the one above.
        let (e, l, at) = edit(&d, (0, Some(1)), 0, Op::Back).unwrap();
        assert_eq!(e[0].bullets.len(), 1);
        assert_eq!(e[0].bullets[0].text, "alphabeta");
        assert_eq!((l, at), ((0, Some(0)), 5));
        // The first paragraph into the title.
        let (e, l, at) = edit(&d, (0, Some(0)), 0, Op::Back).unwrap();
        assert_eq!(e[0].title, "Onealpha");
        assert_eq!((l, at), ((0, None), 3));
        // A title into the last line of the slide before, which takes its paragraphs.
        let (e, l, at) = edit(&d, (1, None), 0, Op::Back).unwrap();
        assert_eq!(e.len(), 1);
        assert_eq!(e[0].bullets[1].text, "betaTwo");
        assert_eq!(e[0].bullets[2].text, "x");
        assert_eq!((l, at), ((0, Some(1)), 4));
        // Delete at the end of a line joins the next.
        let (e, _, at) = edit(&d, (0, Some(1)), 4, Op::Delete).unwrap();
        assert_eq!((e.len(), at), (1, 4));
        // Nothing before the first title.
        assert!(edit(&d, (0, None), 0, Op::Back).is_none());
    }
}
