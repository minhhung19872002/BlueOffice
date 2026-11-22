//! BlueDoc parity round 8 (docs/ui/REPORT.md rounds 98+): editing keys and mouse gestures of
//! the desktop word processor measured on the same copy (posted keys + COM selection on the
//! reference, the engine's selection here) — extend mode (F8), Tab in lists and tables, line
//! and page breaks, formatting shortcuts, repeat (F4), change case (Shift+F3), moving
//! paragraphs and outline levels (Alt+Shift+arrows), AutoCorrect as you type. A child module of
//! the shell so the window's state is at hand.
use super::DocWindow;
use crate::{Input, Key, Mods};
use bo_edit::{CaseMode, CharFormat, ParaFormat, Pos, Selection};
use bo_engine::edit::Command;

/// Round-8 state of the window.
#[derive(Default)]
pub(super) struct R8 {
    /// Extend mode (F8) with its press count: moves extend the selection; presses 2–5 select
    /// the word, sentence, paragraph and document around the caret, as on the reference.
    pub(super) extend: Option<u8>,
    /// The last formatting command recorded by the round-8 keyboard handler, for Repeat (F4).
    pub(super) last: Option<Command>,
    /// AutoCorrect options sent to the engine (the engine keeps it off until a host asks).
    pub(super) autocorrect_on: bool,
    /// Keys held with the next pointer press (Ctrl+click selects a sentence).
    pub(super) pointer: Mods,
    /// The AutoCorrect dialog (File › Options › Proofing).
    pub(super) autocorrect: Option<crate::doc_forms::autocorrect8::AutoCorrectDialog>,
    /// The title bar shows Undo's list arrow.
    pub(super) undo_menu: bool,
    /// Start of the word AutoCorrect just changed: the AutoCorrect Options mark is drawn
    /// under its first letter until the caret moves on.
    pub(super) ac_mark: Option<Pos>,
}

/// Columns of a "+---+---+" line typed for a table (at least one `-` between the `+`).
fn table_columns(text: &str) -> Option<usize> {
    let inner = text.strip_prefix('+')?.strip_suffix('+')?;
    let parts: Vec<&str> = inner.split('+').collect();
    parts
        .iter()
        .all(|p| !p.is_empty() && p.chars().all(|c| c == '-'))
        .then_some(parts.len())
}

/// Line break inside a paragraph (Shift+Enter; the engine's §17.3.3.1 br for U+2028).
const LINE_BREAK: &str = "\u{2028}";
/// Non-breaking hyphen (Ctrl+Shift+-; §17.3.3.18 noBreakHyphen).
const NB_HYPHEN: &str = "\u{2011}";
/// Optional hyphen (Ctrl+-; §17.3.3.29 softHyphen).
const SOFT_HYPHEN: &str = "\u{ad}";

impl DocWindow {
    /// Early round-8 input (before menus and panes): Esc leaves extend mode first.
    pub(super) fn r8_input(&mut self, input: &Input) -> Option<bool> {
        if self.r8.extend.is_some() && matches!(input, Input::Key(Key::Escape, _)) {
            self.r8.extend = None;
            return Some(true);
        }
        None
    }

    /// Document input that nothing else took (keys, typing): the round-8 keys, then the view.
    pub(super) fn r8_doc_input(&mut self, input: Input) -> bool {
        if matches!(input, Input::Text(_) | Input::Key(..)) {
            self.r8_autocorrect_defaults();
        }
        match input {
            Input::Key(k, m) => {
                self.r8.ac_mark = None;
                if let Some(r) = self.r9_key(k, m) {
                    return r;
                }
                if let Some(r) = self.r8_key(k, m) {
                    return r;
                }
                if self.r8.extend.is_some() {
                    if is_motion(k) {
                        let m = Mods { shift: true, ..m };
                        return self.view.handle(Input::Key(k, m));
                    }
                    if matches!(k, Key::Backspace | Key::Delete | Key::Enter | Key::Tab) {
                        self.r8.extend = None;
                    }
                }
                self.view.handle(input)
            }
            Input::Text(_) => {
                self.r8.extend = None;
                self.r8_keep_last_mark();
                let before = self.r8_newest_step();
                let r = self.view.handle(input);
                self.r8_note_correction(&before);
                r
            }
            other => self.view.handle(other),
        }
    }

    /// Typing over whole paragraphs selected with their last mark keeps that mark (the
    /// reference's smart paragraph selection): the selection stops before it.
    fn r8_keep_last_mark(&mut self) {
        let Some(s) = self.view.engine().session() else {
            return;
        };
        let (a, b) = s.selection().ordered();
        if a.offset == 0 && b.offset == 0 && b.para > a.para {
            let len = s.para_text(b.para - 1).map_or(0, |t| t.len());
            self.r8_select(a, Pos::new(b.para - 1, len));
        }
    }

    /// Turn on AutoCorrect with the reference's defaults (capitals, (c) → ©, dashes, lists as
    /// you type, smart quotes, ordinals, fractions, hyperlinks) once per window.
    fn r8_autocorrect_defaults(&mut self) {
        if !self.r8.autocorrect_on {
            self.view.set_typing_runs(true);
            if self.view.engine().autocorrect_options().is_some() {
                self.r8.autocorrect_on = true;
            } else {
                self.r8.autocorrect_on = self
                    .view
                    .exec(Command::AutoCorrect {
                        options: None,
                        reset: true,
                    })
                    .is_some_and(|result| result.error.is_none());
            }
        }
    }

    /// Run a formatting command and remember it for Repeat (F4).
    fn r8_format(&mut self, cmd: Command) -> bool {
        self.r8.last = Some(cmd.clone());
        self.view.run_command(cmd)
    }

    fn r8_text(&mut self, f: CharFormat) -> bool {
        self.r8_format(Command::FormatText { format: f })
    }

    fn r8_para(&mut self, f: ParaFormat) -> bool {
        self.r8_format(Command::FormatParagraph { format: f })
    }

    pub(super) fn r8_select(&mut self, a: Pos, b: Pos) -> bool {
        self.view.run_command(Command::SetSelection {
            selection: Selection {
                anchor: a,
                focus: b,
            },
        })
    }

    /// Keys of the word processor that the plain view does not know; `None` lets the key go
    /// to the view.
    #[allow(clippy::too_many_lines)] // one arm per shortcut
    fn r8_key(&mut self, k: Key, m: Mods) -> Option<bool> {
        let plain = !m.ctrl && !m.alt;
        Some(match k {
            Key::F(8) if plain && !m.shift => self.r8_extend_step(),
            Key::F(4) if plain && !m.shift => match self.r8.last.clone() {
                Some(cmd) => self.view.run_command(cmd),
                None => false,
            },
            Key::F(3) if plain && m.shift => self.r8_format(Command::ChangeCase {
                mode: CaseMode::Cycle,
            }),
            Key::Enter if m.ctrl && !m.alt && !m.shift => self.r8_page_break(),
            Key::Enter if plain && !m.shift => return self.r8_enter(),
            Key::Enter if plain && m.shift => self.view.run_command(Command::InsertText {
                text: LINE_BREAK.into(),
            }),
            Key::Tab if plain => self.r8_tab(m.shift),
            Key::Up | Key::Down if m.alt && m.shift && !m.ctrl => self.r8_move_paras(k == Key::Up),
            Key::Left | Key::Right if m.alt && m.shift && !m.ctrl => {
                self.r8_outline_level(k == Key::Left)
            }
            Key::Letter(c) if m.ctrl => return self.r8_ctrl(c, m),
            _ => return None,
        })
    }

    /// Ctrl shortcuts of the word processor's Home tab measured on the reference.
    fn r8_ctrl(&mut self, c: char, m: Mods) -> Option<bool> {
        let size = || {
            self.view
                .format()
                .size_half_points
                .unwrap_or(22)
                .clamp(2, 3276)
        };
        Some(match (c, m.shift, m.alt) {
            // One point bigger / smaller (Ctrl+] / Ctrl+[).
            (']', false, false) => self.r8_text(CharFormat::Size {
                half_points: size() + 2,
            }),
            ('[', false, false) => self.r8_text(CharFormat::Size {
                half_points: size().saturating_sub(2).max(2),
            }),
            // Ctrl+= subscript, Ctrl+Shift+= (Ctrl++) superscript.
            ('+', false, false) => self.r8_text(CharFormat::Subscript),
            ('+', true, false) => self.r8_text(CharFormat::Superscript),
            (' ', false, false) => self.r8_text(CharFormat::Clear),
            (' ', true, false) => self.view.run_command(Command::InsertText {
                text: "\u{a0}".into(),
            }),
            ('-', false, false) => self.view.run_command(Command::InsertText {
                text: SOFT_HYPHEN.into(),
            }),
            ('-', true, false) => self.view.run_command(Command::InsertText {
                text: NB_HYPHEN.into(),
            }),
            ('q', false, false) => self.r8_para(ParaFormat::Reset),
            // Line spacing: single, double, one and a half (240ths of a line).
            ('1', false, false) => self.r8_para(ParaFormat::LineSpacing { value: 240 }),
            ('2', false, false) => self.r8_para(ParaFormat::LineSpacing { value: 480 }),
            ('5', false, false) => self.r8_para(ParaFormat::LineSpacing { value: 360 }),
            // Ctrl+0: 12 pt before the paragraph, or none when it has some.
            ('0', false, false) => {
                let twips = if self.view.format().space_before > 0 {
                    0
                } else {
                    240
                };
                self.r8_para(ParaFormat::SpaceBefore { twips })
            }
            // Ctrl+Alt+1/2/3: Heading 1–3.
            (d @ ('1' | '2' | '3'), false, true) => self.r8_format(Command::ApplyStyle {
                id: format!("Heading{d}"),
            }),
            ('n', true, false) => self.r8_format(Command::ApplyStyle {
                id: "Normal".into(),
            }),
            ('l', true, false) => self.r8_list_bullet(),
            ('a', true, false) => self.r8_text(CharFormat::AllCaps),
            ('k', true, false) => self.r8_text(CharFormat::SmallCaps),
            ('d', true, false) => self.r8_text(CharFormat::DoubleUnderline),
            ('d', false, false) => self.action("format.fontSettings"),
            ('m', shift, false) => self.r8_para(if shift {
                ParaFormat::DecreaseIndent
            } else {
                ParaFormat::IncreaseIndent
            }),
            // Ctrl+Shift+> / < : the font size list's next / previous size.
            ('.', true, false) => self.action("format.growStep"),
            (',', true, false) => self.action("format.shrinkStep"),
            // Formatting run by the view: remembered for F4.
            ('b' | 'i' | 'u', false, false) => {
                let f = match c {
                    'b' => CharFormat::Bold,
                    'i' => CharFormat::Italic,
                    _ => CharFormat::Underline,
                };
                self.r8_text(f)
            }
            _ => return None,
        })
    }

    /// Ctrl+Enter: the page break in a paragraph of its own (the text after the caret starts
    /// the next page's paragraph), as the reference's `center` row of the key table shows.
    fn r8_page_break(&mut self) -> bool {
        let mid = self
            .view
            .engine()
            .session()
            .is_some_and(|s| s.selection().ordered().0.offset > 0);
        if mid {
            self.view.run_command(Command::SplitParagraph);
        }
        self.view.run_command(Command::InsertPageBreak)
    }

    /// Enter that the reference treats specially (`None`: an ordinary paragraph split): on
    /// an empty list item it ends the list; after a line of three or more `-` `_` `=` `*`
    /// `~` `#` it turns the line into a bottom border (AutoFormat As You Type).
    fn r8_enter(&mut self) -> Option<bool> {
        let s = self.view.engine().session()?;
        let sel = s.selection();
        if !sel.is_collapsed() {
            return None;
        }
        let at = sel.focus;
        let text = s.para_text(at.para).unwrap_or_default();
        let f = self.view.format();
        if text.is_empty() && (f.bullets || f.numbering) && f.table.is_none() {
            let cmd = if f.bullets {
                ParaFormat::Bullets
            } else {
                ParaFormat::Numbering
            };
            return Some(
                self.view
                    .run_command(Command::FormatParagraph { format: cmd }),
            );
        }
        let opts = self.view.engine().autocorrect_options();
        let lists_on = opts.is_some_and(|o| o.enabled && o.borders);
        let tables_on = opts.is_some_and(|o| o.enabled && o.tables);
        // "+---+---+" + Enter: a table with one column per segment (AutoFormat As You Type
        // › Tables, round 9).
        if tables_on && at.offset == text.len() && f.table.is_none() {
            if let Some(cols) = table_columns(&text) {
                let para = at.para;
                // Each column as wide as its "+---" run as typed (round 10: the reference's
                // cells are 24 / 42 pt for "+-----+----------+" in Calibri 12 pt).
                let widths = self.r10_plus_widths(para, &text);
                self.r8_select(Pos::new(para, 0), Pos::new(para, text.len()));
                self.view.run_command(Command::DeleteBackward);
                let made = self
                    .view
                    .run_command(Command::InsertTable { rows: 1, cols });
                self.r10_set_column_widths(&widths);
                return Some(made);
            }
        }
        let rule = text.len() >= 3
            && at.offset == text.len()
            && text
                .chars()
                .all(|c| c == text.chars().next().unwrap_or(' '))
            && text.starts_with(['-', '_', '=', '*', '~', '#']);
        if !(rule && lists_on && f.table.is_none()) {
            return None;
        }
        let para = at.para;
        // The line goes under the paragraph before (measured: "Last paragraph." gets the
        // border and the typed line's paragraph stays for the next text); after a table or
        // at the start, under the typed line's own paragraph, with a new one after it.
        let doc = s.doc();
        let body_level =
            |id: usize| bo_model::index::para_path(doc, id).is_some_and(|p| p.len() == 2);
        let above = para > 0 && body_level(para - 1) && body_level(para);
        self.r8_select(Pos::new(para, 0), Pos::new(para, text.len()));
        self.view.run_command(Command::DeleteBackward);
        if above {
            self.r8_select(Pos::new(para - 1, 0), Pos::new(para - 1, 0));
            self.view.run_command(Command::FormatParagraph {
                format: ParaFormat::Borders {
                    preset: bo_edit::ParaBorderPreset::Bottom,
                },
            });
            return Some(self.r8_select(Pos::new(para, 0), Pos::new(para, 0)));
        }
        self.view.run_command(Command::SplitParagraph);
        self.r8_select(Pos::new(para, 0), Pos::new(para, 0));
        self.view.run_command(Command::FormatParagraph {
            format: ParaFormat::Borders {
                preset: bo_edit::ParaBorderPreset::Bottom,
            },
        });
        Some(self.r8_select(Pos::new(para + 1, 0), Pos::new(para + 1, 0)))
    }

    /// Ctrl+Shift+L: the List Bullet style (the bulleted list when the document lacks it).
    fn r8_list_bullet(&mut self) -> bool {
        let cmd = Command::ApplyStyle {
            id: "ListBullet".into(),
        };
        let styled = self
            .view
            .exec(cmd.clone())
            .is_some_and(|r| r.error.is_none());
        if styled && self.view.format().bullets {
            self.r8.last = Some(cmd);
            return true;
        }
        self.view.format().bullets || self.r8_para(ParaFormat::Bullets)
    }

    /// F8: extend mode, then the word, sentence, paragraph and whole document.
    fn r8_extend_step(&mut self) -> bool {
        let n = self.r8.extend.map_or(1, |n| (n + 1).min(5));
        self.r8.extend = Some(n);
        let Some(s) = self.view.engine().session() else {
            return true;
        };
        let (a, b) = s.selection().ordered();
        let count = s.paragraph_count();
        let sel = match n {
            2 => {
                let w = s.word_at(b).ordered();
                // The word with the spaces after it, as a double click.
                let text = s.para_text(w.1.para).unwrap_or_default();
                let spaces = text
                    .get(w.1.offset..)
                    .map_or(0, |t| t.len() - t.trim_start_matches(' ').len());
                Some((w.0, Pos::new(w.1.para, w.1.offset + spaces)))
            }
            3 => {
                let st = s.sentence_at(a).ordered();
                Some((st.0.min(a), st.1.max(b)))
            }
            4 => {
                let end = if b.para + 1 < count {
                    Pos::new(b.para + 1, 0)
                } else {
                    Pos::new(b.para, s.para_text(b.para).map_or(0, |t| t.len()))
                };
                Some((Pos::new(a.para, 0), end))
            }
            5 => {
                let last = count.saturating_sub(1);
                Some((
                    Pos::new(0, 0),
                    Pos::new(last, s.para_text(last).map_or(0, |t| t.len())),
                ))
            }
            _ => None,
        };
        match sel {
            Some((a, b)) => self.r8_select(a, b),
            None => true,
        }
    }

    /// Tab / Shift+Tab: the next / previous table cell with its text selected, a list level
    /// down / up at the start of an item, the first-line indent at the start of a paragraph,
    /// else a tab character (the engine's `tab` command).
    fn r8_tab(&mut self, back: bool) -> bool {
        let before = self.view.caret_para();
        let in_table = self.view.format().table.is_some();
        if !self.view.run_command(Command::Tab { back }) {
            return false;
        }
        let after = self.view.caret_para();
        if in_table && before != after {
            if let (Some(p), Some(s)) = (after, self.view.engine().session()) {
                let len = s.para_text(p).map_or(0, |t| t.len());
                if len > 0 && s.selection().is_collapsed() {
                    return self.r8_select(Pos::new(p, 0), Pos::new(p, len));
                }
            }
        }
        true
    }

    /// Alt+Shift+Up / Down: move the selected paragraphs, which end up selected whole.
    fn r8_move_paras(&mut self, up: bool) -> bool {
        if !self.view.run_command(Command::MoveParagraphs { up }) {
            return false;
        }
        let Some(s) = self.view.engine().session() else {
            return true;
        };
        let (a, b) = s.selection().ordered();
        let end = if b.para + 1 < s.paragraph_count() {
            Pos::new(b.para + 1, 0)
        } else {
            Pos::new(b.para, s.para_text(b.para).map_or(0, |t| t.len()))
        };
        self.r8_select(Pos::new(a.para, 0), end)
    }

    /// Alt+Shift+Left / Right: promote / demote the paragraph's heading level (body text
    /// demotes to Heading 2 and promotes to Heading 1, as measured on the reference).
    fn r8_outline_level(&mut self, promote: bool) -> bool {
        let level = heading_level(self.view.format().style.as_deref());
        let next = match (level, promote) {
            (Some(0), true) => return false,
            (Some(l), true) => l - 1,
            (Some(l), false) => (l + 1).min(8),
            (None, true) => 0,
            (None, false) => 1,
        };
        self.r8_format(Command::ApplyStyle {
            id: format!("Heading{}", next + 1),
        })
    }
}

/// 0-based heading level of a paragraph style id ("Heading2" → 1).
fn heading_level(style: Option<&str>) -> Option<u8> {
    let s = style?;
    let digits = s
        .strip_prefix("Heading")
        .or_else(|| s.strip_prefix("heading"))?
        .trim();
    let n: u8 = digits.parse().ok()?;
    (1..=9).contains(&n).then(|| n - 1)
}

fn is_motion(k: Key) -> bool {
    matches!(
        k,
        Key::Left
            | Key::Right
            | Key::Up
            | Key::Down
            | Key::Home
            | Key::End
            | Key::PageUp
            | Key::PageDown
    )
}

#[path = "doc_r8_mouse.rs"]
mod mouse;
#[path = "doc_r8_ui.rs"]
mod ui;

#[cfg(test)]
#[path = "doc_r8_tests.rs"]
mod tests;
