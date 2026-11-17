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
    /// Rows scrolled off the top of the list (round 19).
    pub scroll: usize,
    /// The row under the pointer (its drop-down shows), by its place in the list.
    pub hover: Option<usize>,
    /// The pointer is over the pane (the wheel scrolls the list).
    pub ptr: bool,
}

/// A row of the Thesaurus list.
enum ThesRow {
    /// Meaning `n`'s heading, folded or not.
    Head(usize, String, bool),
    /// A synonym or antonym (the word looked up, the text shown).
    Word(String, String),
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
        // Every row of the list (round 19: it scrolls), antonyms marked as the reference
        // marks them.
        let mut rows = Vec::new();
        for (i, m) in meanings.iter().enumerate() {
            let head = format!(
                "{} ({})",
                m.synonyms
                    .first()
                    .map_or(self.thes.word.as_str(), String::as_str),
                pos_label(&m.pos)
            );
            let folded = self.thes.folded.contains(&i);
            rows.push(ThesRow::Head(i, head, folded));
            if folded {
                continue;
            }
            for s in &m.synonyms {
                rows.push(ThesRow::Word(s.clone(), s.clone()));
            }
            for a in &m.antonyms {
                rows.push(ThesRow::Word(a.clone(), format!("{a} ({})", labels[3])));
            }
        }
        let fits = (((bottom - ry) / row).floor().max(1.0)) as usize;
        let max_scroll = rows.len().saturating_sub(fits);
        self.thes.scroll = self.thes.scroll.min(max_scroll);
        let bar = rows.len() > fits;
        // A classic scroll bar inside the frame when the list is longer than its room.
        let bw = if bar { self.px(13.6) } else { 0.0 };
        let row_w = width - 2.0 - bw;
        for (k, r) in rows.iter().enumerate().skip(self.thes.scroll).take(fits) {
            let rr = [left + 1.0, ry, row_w, row];
            match r {
                ThesRow::Head(i, head, folded) => {
                    // Chevron: down when open, right when folded.
                    let (cx, cy, k) = (left + self.px(7.2), ry + row / 2.0, self.px(3.6));
                    let segs = if *folded {
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
                    ops.push(Op::Text(head.clone(), tx, text_y(ry), body, ink));
                    hits.push((rr, SideHit::ThesToggle(*i)));
                }
                ThesRow::Word(word, shown) => {
                    if self.thes.hover == Some(k) {
                        // The hovered word: a light band and its drop-down (Insert / Copy).
                        ops.push(Op::Rect(rr, ROW_SEL));
                        let dw = self.px(20.0);
                        let d = [rr[0] + rr[2] - dw, ry, dw, row];
                        ops.push(Op::Rect(d, BUTTON_EDGE));
                        ops.push(Op::Rect(
                            [d[0] + 1.0, d[1] + 1.0, d[2] - 2.0, d[3] - 2.0],
                            WHITE,
                        ));
                        let (ax, ay, k) = (d[0] + dw / 2.0, ry + row / 2.0, self.px(3.2));
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
                        hits.push((rr, SideHit::ThesLookup(word.clone())));
                        hits.push((d, SideHit::ThesMenu(word.clone())));
                    } else {
                        hits.push((rr, SideHit::ThesLookup(word.clone())));
                    }
                    ops.push(Op::Text(shown.clone(), tx, text_y(ry), body, ink));
                }
            }
            ry += row;
        }
        if bar {
            let track = [left + width - 1.0 - bw, ly + 1.0, bw, lh - 2.0];
            ops.push(Op::Rect(track, Rgba(0xf0, 0xf0, 0xf0, 255)));
            let (ah, k) = (bw, self.px(3.2));
            let cx = track[0] + bw / 2.0;
            for (cy, up) in [
                (track[1] + ah / 2.0, true),
                (track[1] + track[3] - ah / 2.0, false),
            ] {
                let d = if up { -k / 2.0 } else { k / 2.0 };
                ops.push(Op::Path(
                    vec![
                        PathSeg::M(cx - k, cy - d),
                        PathSeg::L(cx + k, cy - d),
                        PathSeg::L(cx, cy + d),
                        PathSeg::Z,
                    ],
                    Some(Rgba(0x60, 0x60, 0x60, 255)),
                    None,
                ));
            }
            let room = track[3] - 2.0 * ah;
            let len = (room * fits as f32 / rows.len() as f32)
                .max(self.px(16.0))
                .min(room);
            let ty =
                track[1] + ah + (room - len) * self.thes.scroll as f32 / max_scroll.max(1) as f32;
            ops.push(Op::Rect(
                [track[0] + 2.0, ty.round(), bw - 4.0, len.round()],
                Rgba(0xc2, 0xc2, 0xc2, 255),
            ));
            let page = i32::try_from(fits.saturating_sub(1).max(1)).unwrap_or(1);
            hits.push(([track[0], track[1], bw, ah], SideHit::ThesScroll(-1)));
            hits.push((
                [track[0], track[1] + track[3] - ah, bw, ah],
                SideHit::ThesScroll(1),
            ));
            hits.push((
                [track[0], track[1] + ah, bw, (ty - track[1] - ah).max(0.0)],
                SideHit::ThesScroll(-page),
            ));
            hits.push((
                [
                    track[0],
                    ty + len,
                    bw,
                    (track[1] + track[3] - ah - ty - len).max(0.0),
                ],
                SideHit::ThesScroll(page),
            ));
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
                t.scroll = 0;
                t.hover = None;
            }
            SideHit::ThesScroll(n) => {
                t.scroll = usize::try_from(i64::try_from(t.scroll).unwrap_or(0) + i64::from(*n))
                    .unwrap_or(0);
            }
            SideHit::ThesMenu(s) => {
                let s = s.clone();
                return self.thesaurus_word_menu(&s);
            }
            _ => return false,
        }
        self.relayout_pending = true;
        true
    }

    /// The drop-down of a hovered word: Insert (in place of the word at the caret) and
    /// Copy, as the reference's.
    fn thesaurus_word_menu(&mut self, s: &str) -> bool {
        let Some(r) = self
            .side
            .hits
            .iter()
            .find(|(_, h)| *h == SideHit::ThesMenu(s.to_owned()))
            .map(|(r, _)| *r)
        else {
            return false;
        };
        let items = vec![
            crate::popup::MenuItem {
                label: self.t("slideNative.thesInsert", "Insert"),
                choice: crate::popup::Choice::Command(format!("rv:thesins:{s}")),
                checked: false,
                dim: self.thesaurus_word().is_none(),
            },
            crate::popup::MenuItem {
                label: self.t("slideNative.thesCopy", "Copy"),
                choice: crate::popup::Choice::Command(format!("rv:thescopy:{s}")),
                checked: false,
                dim: false,
            },
        ];
        self.menu_kind = super::super::MenuKind::Duration;
        self.menu = Some(crate::popup::Menu::open(items, r, self.size, self.scale));
        true
    }

    /// Hover / wheel over the Thesaurus pane: `Some(redraw)` when taken.
    pub(crate) fn thesaurus_pointer(&mut self, input: &crate::Input) -> Option<bool> {
        if self.side.kind != SideKind::Thesaurus {
            return None;
        }
        match *input {
            crate::Input::Move { x, y } => {
                let inside = self.side.contains(x, y);
                self.side.thes.ptr = inside;
                // The hovered word row: its place among the rows drawn.
                let hover = inside
                    .then(|| {
                        let rows: Vec<&([f32; 4], SideHit)> = self
                            .side
                            .hits
                            .iter()
                            .filter(|(_, h)| {
                                matches!(h, SideHit::ThesLookup(_) | SideHit::ThesToggle(_))
                            })
                            .collect();
                        let k = rows.iter().position(|(r, _)| {
                            x >= r[0] && x < r[0] + r[2] && y >= r[1] && y < r[1] + r[3]
                        })?;
                        // Rows keep their order; the hovered word's own pair counts once.
                        let mut seen = Vec::new();
                        for (r, _) in &rows {
                            if !seen.iter().any(|s: &f32| (*s - r[1]).abs() < 0.5) {
                                seen.push(r[1]);
                            }
                        }
                        let ry = rows[k].0[1];
                        let at = seen.iter().position(|s| (*s - ry).abs() < 0.5)?;
                        matches!(rows[k].1, SideHit::ThesLookup(_))
                            .then_some(at + self.side.thes.scroll)
                    })
                    .flatten();
                if hover != self.side.thes.hover {
                    self.side.thes.hover = hover;
                    self.relayout_pending = true;
                    return Some(true);
                }
                inside.then_some(false)
            }
            crate::Input::Wheel(d) if self.side.thes.ptr => {
                let n = if d > 0.0 { 3 } else { -3 };
                self.thesaurus_hit(&SideHit::ThesScroll(n), 1);
                Some(true)
            }
            _ => None,
        }
    }

    /// Copy `s` to the clipboard (the hovered word's Copy).
    pub(crate) fn thesaurus_copy(&mut self, s: &str) -> bool {
        self.requests.push(crate::Request::SetClipboard {
            text: s.to_owned(),
            html: String::new(),
            internal: String::new(),
        });
        true
    }

    /// Insert `s` (the hovered word's Insert).
    pub(crate) fn thesaurus_insert_word(&mut self, s: &str) -> bool {
        self.thesaurus_insert(s)
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
