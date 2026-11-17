//! Review > Thesaurus (Shift+F7): the Thesaurus task pane of BlueSlide (round 18), laid out
//! as the reference presentation app's (measured at 125 %): a back button and a search box
//! with the word, a framed list of its meanings — a heading "first synonym (part of speech)"
//! with a chevron that folds it, the synonyms under it — and the language box at the foot.
//! A press on a synonym looks it up (Back returns), a double press puts it in place of the
//! word in the text being edited. Data: the English thesaurus of `slide_thesaurus.rs`.
use super::super::thesaurus::{self, Meaning};
use super::super::SlideWindow;
use super::{wrap, Op, SideHit, SideKind, SidePane, BUTTON_EDGE, DIM, ROW_SEL, WHITE};
use crate::{TextMeasure, TextStyle};
use bo_engine::slide_edit::SlideCommand;
use bo_render::{PathSeg, Rgba};

/// Meanings listed for a word (the reference lists every sense; a long list is clipped).
const MAX_MEANINGS: usize = 40;

/// What the pane looks up and how its list is folded.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ThesState {
    /// The word looked up.
    pub word: String,
    /// Words looked up before (Back).
    pub back: Vec<String>,
    /// Folded meanings (indices).
    pub folded: Vec<usize>,
}

/// Abbreviation of a WordNet part of speech, as the reference writes it.
fn pos_label(pos: &str) -> &'static str {
    match pos {
        "v" => "v.",
        "a" | "s" => "adj.",
        "r" => "adv.",
        _ => "n.",
    }
}

/// The word at UTF-16 offset `at` of `text` (letters, digits, apostrophes, hyphens):
/// its UTF-16 range.
pub(crate) fn word_range(text: &str, at: usize) -> Option<(usize, usize)> {
    let units: Vec<(usize, char)> = {
        let mut o = 0;
        text.chars()
            .map(|c| {
                let r = (o, c);
                o += c.len_utf16();
                r
            })
            .collect()
    };
    let is_word = |c: char| c.is_alphanumeric() || c == '\'' || c == '-';
    // The char that starts at `at`, or the one before it (caret after a word).
    let i = units
        .iter()
        .position(|(o, c)| *o == at && is_word(*c))
        .or_else(|| {
            units
                .iter()
                .position(|(o, c)| *o + c.len_utf16() == at && is_word(*c))
        })?;
    let mut a = i;
    while a > 0 && is_word(units[a - 1].1) {
        a -= 1;
    }
    let mut b = i;
    while b + 1 < units.len() && is_word(units[b + 1].1) {
        b += 1;
    }
    Some((units[a].0, units[b].0 + units[b].1.len_utf16()))
}

/// The UTF-16 range `a..b` of `text` as a string.
fn slice16(text: &str, a: usize, b: usize) -> String {
    let mut o = 0;
    let mut out = String::new();
    for c in text.chars() {
        if o >= a && o < b {
            out.push(c);
        }
        o += c.len_utf16();
    }
    out
}

impl SidePane {
    /// Lay out the Thesaurus pane: `labels` title, "no results" text, language name,
    /// "Antonym".
    pub fn record_thesaurus(
        &mut self,
        text: &dyn TextMeasure,
        meanings: &[Meaning],
        labels: &[String; 4],
        ink: Rgba,
    ) {
        let mut ops = Vec::new();
        let mut hits = Vec::new();
        self.header(&mut ops, &mut hits, &labels[0], ink);
        let [x, y, w, h] = self.rect;
        // Pane options: a chevron after the title, as the reference's.
        let tw = text
            .measure(
                &labels[0],
                TextStyle {
                    size: self.px(20.0),
                    bold: false,
                },
            )
            .0;
        let cs = self.px(22.0);
        let opt = [
            x + self.px(12.8) + tw + self.px(4.0),
            y + self.px(12.0),
            cs,
            cs,
        ];
        let (ox, oy, k) = (opt[0] + cs / 2.0, opt[1] + cs / 2.0, self.px(5.0));
        ops.push(Op::Path(
            vec![
                PathSeg::M(ox - k, oy - k / 2.0),
                PathSeg::L(ox, oy + k / 2.0),
                PathSeg::L(ox + k, oy - k / 2.0),
            ],
            None,
            Some((ink, self.scale.max(1.0).round())),
        ));
        hits.push((opt, SideHit::PaneMenu));
        let body = TextStyle {
            size: self.px(12.8),
            bold: false,
        };
        let left = x + self.px(9.6);
        let width = w - self.px(19.2);
        let stroke = self.scale.max(1.0).round();
        let frame = |ops: &mut Vec<Op>, r: [f32; 4]| {
            ops.push(Op::Rect(r, BUTTON_EDGE));
            ops.push(Op::Rect(
                [r[0] + 1.0, r[1] + 1.0, r[2] - 2.0, r[3] - 2.0],
                WHITE,
            ));
        };
        // Reference at 125 %: back button and search box 34 px tall 60 px under the pane's
        // top, the back button 34 px wide, the box 11 px right of it.
        let (by, bh) = (y + self.px(48.0), self.px(27.2));
        let back = [left, by, bh, bh];
        frame(&mut ops, back);
        let can_back = !self.thes.back.is_empty();
        let (cx, cy, k) = (back[0] + bh / 2.0, back[1] + bh / 2.0, self.px(3.6));
        ops.push(Op::Path(
            vec![
                PathSeg::M(cx + k / 2.0, cy - k),
                PathSeg::L(cx - k / 2.0, cy),
                PathSeg::L(cx + k / 2.0, cy + k),
            ],
            None,
            Some((if can_back { ink } else { DIM }, stroke)),
        ));
        if can_back {
            hits.push((back, SideHit::ThesBack));
        }
        let sx = left + bh + self.px(8.8);
        let search = [sx, by, left + width - sx, bh];
        frame(&mut ops, search);
        let th = text.measure("Ag", body).1;
        ops.push(Op::Text(
            self.thes.word.clone(),
            sx + self.px(6.4),
            (by + (bh - th) / 2.0).round(),
            body,
            ink,
        ));
        // A magnifier (own drawing) at the box's right end.
        let (mx, my, r) = (
            search[0] + search[2] - self.px(15.0),
            cy - self.px(1.6),
            self.px(5.6),
        );
        let n = 16;
        let mut ring: Vec<PathSeg> = (0..=n)
            .map(|i| {
                let t = std::f32::consts::TAU * i as f32 / n as f32;
                let (px, py) = (mx + r * t.cos(), my + r * t.sin());
                if i == 0 {
                    PathSeg::M(px, py)
                } else {
                    PathSeg::L(px, py)
                }
            })
            .collect();
        ring.push(PathSeg::M(mx - r * 0.71, my + r * 0.71));
        ring.push(PathSeg::L(mx - r * 1.75, my + r * 1.75));
        ops.push(Op::Path(ring, None, Some((ink, stroke))));
        // The list: reference 350–775 px (rows 27 px), framed, with the meanings.
        let foot = y + h - self.px(44.0);
        let ly = y + self.px(90.4);
        let lh = self
            .px(340.0)
            .min(foot - self.px(24.0) - ly)
            .max(self.px(40.0));
        let list = [left, ly, width, lh];
        frame(&mut ops, list);
        let row = self.px(21.6);
        let tx = left + self.px(15.2);
        let mut ry = ly + self.px(2.4);
        let bottom = ly + lh - 1.0;
        let text_y = |ry: f32| (ry + (row - th) / 2.0).round();
        if meanings.is_empty() {
            for l in wrap(&labels[1], width - self.px(12.0), text, body) {
                ops.push(Op::Text(l, left + self.px(6.4), text_y(ry), body, DIM));
                ry += row;
            }
        }
        'list: for (i, m) in meanings.iter().enumerate() {
            if ry + row > bottom {
                break;
            }
            let head = format!(
                "{} ({})",
                m.synonyms
                    .first()
                    .map_or(self.thes.word.as_str(), String::as_str),
                pos_label(&m.pos)
            );
            let folded = self.thes.folded.contains(&i);
            // Chevron: down when open, right when folded.
            let (cx, cy, k) = (left + self.px(7.2), ry + row / 2.0, self.px(3.6));
            let segs = if folded {
                vec![
                    PathSeg::M(cx - k / 2.0, cy - k),
                    PathSeg::L(cx + k / 2.0, cy),
                    PathSeg::L(cx - k / 2.0, cy + k),
                ]
            } else {
                vec![
                    PathSeg::M(cx - k, cy - k / 2.0),
                    PathSeg::L(cx, cy + k / 2.0),
                    PathSeg::L(cx + k, cy - k / 2.0),
                ]
            };
            ops.push(Op::Path(segs, None, Some((ink, stroke))));
            ops.push(Op::Text(head, tx, text_y(ry), body, ink));
            hits.push(([left + 1.0, ry, width - 2.0, row], SideHit::ThesToggle(i)));
            ry += row;
            if folded {
                continue;
            }
            for s in &m.synonyms {
                if ry + row > bottom {
                    break 'list;
                }
                ops.push(Op::Text(s.clone(), tx, text_y(ry), body, ink));
                hits.push((
                    [left + 1.0, ry, width - 2.0, row],
                    SideHit::ThesLookup(s.clone()),
                ));
                ry += row;
            }
            // Antonyms after the synonyms, marked as the reference marks them.
            for a in &m.antonyms {
                if ry + row > bottom {
                    break 'list;
                }
                ops.push(Op::Text(
                    format!("{a} ({})", labels[3]),
                    tx,
                    text_y(ry),
                    body,
                    ink,
                ));
                hits.push((
                    [left + 1.0, ry, width - 2.0, row],
                    SideHit::ThesLookup(a.clone()),
                ));
                ry += row;
            }
        }
        // A rule under the list, the language box at the pane's foot.
        ops.push(Op::Rect(
            [left, ly + lh + self.px(16.0), width, 1.0],
            ROW_SEL,
        ));
        let lb = [left, foot, width, self.px(24.0)];
        frame(&mut ops, lb);
        ops.push(Op::Text(
            labels[2].clone(),
            left + self.px(5.6),
            (foot + (lb[3] - th) / 2.0).round(),
            body,
            ink,
        ));
        let (ax, ay, k) = (
            lb[0] + lb[2] - self.px(10.0),
            foot + lb[3] / 2.0,
            self.px(3.2),
        );
        ops.push(Op::Path(
            vec![
                PathSeg::M(ax - k, ay - k / 2.0),
                PathSeg::L(ax + k, ay - k / 2.0),
                PathSeg::L(ax, ay + k / 2.0),
                PathSeg::Z,
            ],
            Some(ink),
            None,
        ));
        self.ops = ops;
        self.hits = hits;
    }
}

impl SlideWindow {
    /// The word of the text being edited: the selection when it is one word, else the word
    /// at the caret; with the text box id and its UTF-16 range.
    fn thesaurus_word(&self) -> Option<(u32, usize, usize, String)> {
        let te = self.view.text_edit()?;
        let (a, b) = (te.start.min(te.end), te.start.max(te.end));
        let (a, b) = if a == b {
            word_range(&te.text, a)?
        } else {
            (a, b)
        };
        let w = slice16(&te.text, a, b).trim().to_owned();
        (!w.is_empty() && !w.contains(char::is_whitespace)).then_some((te.id, a, b, w))
    }

    /// Review > Thesaurus: look up the word at the caret in the pane (open it).
    pub(crate) fn open_thesaurus(&mut self) -> bool {
        if let Some((_, _, _, w)) = self.thesaurus_word() {
            self.side.thes = ThesState {
                word: w,
                ..ThesState::default()
            };
        }
        if self.side.kind == SideKind::Thesaurus {
            self.relayout_pending = true;
            return true;
        }
        self.open_side(SideKind::Thesaurus)
    }

    /// What the pane lists for its word.
    pub(crate) fn thesaurus_meanings(&self) -> Vec<Meaning> {
        let w = self.side.thes.word.to_lowercase();
        if w.is_empty() {
            return Vec::new();
        }
        // The word itself is not offered (synonyms come without it); senses with the same
        // heading and part of speech are merged; a sense left without synonyms or antonyms
        // is not listed.
        let mut out: Vec<Meaning> = Vec::new();
        for m in thesaurus::meanings(&w, MAX_MEANINGS) {
            if m.synonyms.is_empty() && m.antonyms.is_empty() {
                continue;
            }
            let head = |x: &Meaning| x.synonyms.first().cloned();
            if let Some(o) = out
                .iter_mut()
                .find(|o| o.pos == m.pos && head(o) == head(&m))
            {
                for s in m.synonyms {
                    if !o.synonyms.contains(&s) {
                        o.synonyms.push(s);
                    }
                }
                for s in m.antonyms {
                    if !o.antonyms.contains(&s) {
                        o.antonyms.push(s);
                    }
                }
            } else {
                out.push(m);
            }
        }
        out
    }

    /// A press in the Thesaurus pane (`count` 2: a double press).
    pub(crate) fn thesaurus_hit(&mut self, hit: &SideHit, count: u8) -> bool {
        let t = &mut self.side.thes;
        match hit {
            SideHit::ThesBack => {
                let Some(w) = t.back.pop() else {
                    return false;
                };
                t.word = w;
                t.folded.clear();
            }
            SideHit::ThesToggle(i) => {
                if let Some(k) = t.folded.iter().position(|f| f == i) {
                    t.folded.remove(k);
                } else {
                    t.folded.push(*i);
                }
            }
            SideHit::ThesLookup(s) if count >= 2 => return self.thesaurus_insert(&s.clone()),
            SideHit::ThesLookup(s) => {
                if *s == t.word {
                    return false;
                }
                let old = std::mem::replace(&mut t.word, s.clone());
                t.back.push(old);
                t.folded.clear();
            }
            _ => return false,
        }
        self.relayout_pending = true;
        true
    }

    /// Put `s` in place of the word at the caret (the selection) of the text being edited.
    fn thesaurus_insert(&mut self, s: &str) -> bool {
        let Some((id, a, b, _)) = self.thesaurus_word() else {
            return false;
        };
        self.view.run(SlideCommand::TextSelect {
            id: Some(id),
            start: a,
            end: b,
            cell: self.view.text_edit().and_then(|t| t.cell),
        });
        self.view.handle(crate::Input::Text(s.to_owned()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn word_at_the_caret() {
        let t = "We increase the revenue";
        assert_eq!(word_range(t, 5), Some((3, 11)));
        // Right after a word.
        assert_eq!(word_range(t, 11), Some((3, 11)));
        assert_eq!(word_range(t, 0), Some((0, 2)));
        assert_eq!(word_range("  ", 1), None);
        assert_eq!(slice16(t, 3, 11), "increase");
        // UTF-16 offsets past a wide char.
        assert_eq!(word_range("😀 tốt", 4), Some((3, 6)));
    }

    #[test]
    fn parts_of_speech() {
        assert_eq!(pos_label("n"), "n.");
        assert_eq!(pos_label("v"), "v.");
        assert_eq!(pos_label("s"), "adj.");
        assert_eq!(pos_label("r"), "adv.");
    }
}
