//! BlueDoc round 8, window parts: the AutoCorrect dialog (opened from File › Options ›
//! Proofing) and the quick access Undo list (the steps Undo would revert, newest first; a
//! choice undoes it and every newer step, as on the reference).
use super::super::DocWindow;
use crate::doc_forms::autocorrect8::{AcOut, AutoCorrectDialog};
use crate::popup::{Choice, Menu, MenuItem};
use crate::{Input, Painter, TextMeasure};
use bo_engine::edit::Command;

/// Undo list rows shown at most (the reference's list scrolls; ours keeps the newest).
const UNDO_ROWS: usize = 24;
/// Choice id prefix of an Undo list row: `edit.undoSteps:<n>` undoes `n` steps.
const UNDO_STEPS: &str = "edit.undoSteps:";

impl DocWindow {
    /// Round-8 command ids; `None` for the others.
    pub(in crate::shell) fn r8_action(&mut self, id: &str) -> Option<bool> {
        if id == crate::titlebar::UNDO_LIST {
            return Some(self.r8_open_undo_list());
        }
        if id == "tools.autoCorrect" {
            return Some(self.r8_open_autocorrect());
        }
        let n: usize = id.strip_prefix(UNDO_STEPS)?.parse().ok()?;
        let mut any = false;
        for _ in 0..n {
            any |= self.view.run_command(Command::Undo);
        }
        Some(any)
    }

    /// Open the AutoCorrect dialog with the engine's options.
    pub(in crate::shell) fn r8_open_autocorrect(&mut self) -> bool {
        self.r8_autocorrect_defaults();
        let opts = self
            .view
            .engine()
            .autocorrect_options()
            .cloned()
            .unwrap_or_default();
        let lang = self
            .view
            .format()
            .lang
            .clone()
            .unwrap_or_else(|| "en-US".into());
        let language = crate::doc_forms::LANGUAGES
            .iter()
            .find(|(tag, _)| tag.eq_ignore_ascii_case(&lang))
            .map_or(lang.clone(), |(_, name)| (*name).to_owned());
        let mut d = AutoCorrectDialog::open(&self.strings, &opts, &language);
        // Check boxes in BlueDoc's own accent, as its other dialogs.
        d.set_accent(self.theme.accent_strong, self.theme.accent);
        self.r8.autocorrect = Some(d);
        self.relayout_pending = true;
        true
    }

    /// Input while the AutoCorrect dialog is open (over everything, the Options dialog too).
    pub(in crate::shell) fn r8_dialog_input(&mut self, input: &Input) -> Option<bool> {
        let d = self.r8.autocorrect.as_mut()?;
        if let Input::Resize { .. } | Input::Scale(_) = input {
            return None;
        }
        Some(match d.input(input) {
            AcOut::None => false,
            AcOut::Redraw => true,
            AcOut::Close => {
                self.r8.autocorrect = None;
                true
            }
            AcOut::Apply(options) => {
                self.r8.autocorrect = None;
                self.view.run_command(Command::AutoCorrect {
                    options: Some(options),
                    reset: false,
                });
                self.r8.autocorrect_on = true;
                true
            }
        })
    }

    /// Per-frame layout of the round-8 parts.
    pub(in crate::shell) fn r8_layout(&mut self, text: &dyn TextMeasure) {
        if !self.r8.undo_menu {
            self.r8.undo_menu = true;
            // View Gridlines is on by default on the reference.
            self.r7.gridlines = true;
            self.title.set_undo_menu(true);
            self.relayout_pending = true;
        }
        let (size, scale) = (self.size, self.scale);
        if let Some(d) = self.r8.autocorrect.as_mut() {
            d.layout(text, size, scale);
        }
    }

    /// Table Layout › View Gridlines: the layout's gridlines of borderless cell edges, drawn
    /// over the visible pages as on the reference (1 px #939393, 3 px on / 1 px off at
    /// 125 %). On by default, as on the reference.
    pub(in crate::shell) fn r8_paint_gridlines(&self, p: &mut dyn Painter) {
        self.r8_paint_ac_mark(p);
        if !self.r7.gridlines {
            return;
        }
        let ink = bo_render::Rgba(0x93, 0x93, 0x93, 255);
        let s = self.view.scale();
        let (on, period) = (3.0_f32, 4.0_f32);
        for slot in self.view.slots() {
            if (slot.y + slot.height) < 0 || slot.y as f32 > self.size.1 {
                continue;
            }
            for g in self.view.engine().table_gridlines(slot.index) {
                let (x1, y1) = (
                    (slot.x as f32 + g[0] * s).round(),
                    (slot.y as f32 + g[1] * s).round(),
                );
                let (x2, y2) = (
                    (slot.x as f32 + g[2] * s).round(),
                    (slot.y as f32 + g[3] * s).round(),
                );
                if (y1 - y2).abs() < 0.5 {
                    let mut x = x1.min(x2);
                    while x < x1.max(x2) {
                        p.fill_rect(x, y1, on.min(x1.max(x2) - x), 1.0, ink);
                        x += period;
                    }
                } else {
                    let mut y = y1.min(y2);
                    while y < y1.max(y2) {
                        p.fill_rect(x1, y, 1.0, on.min(y1.max(y2) - y), ink);
                        y += period;
                    }
                }
            }
        }
    }

    /// Number and kind of the newest undo step (to see what a keystroke added).
    pub(in crate::shell) fn r8_newest_step(&self) -> (usize, String) {
        let list = self.view.engine().undo_list();
        let kind = list.first().map(|l| l.kind.clone()).unwrap_or_default();
        (list.len(), kind)
    }

    /// After typing: when AutoCorrect changed the word just ended (a new AutoCorrect step),
    /// remember its start for the AutoCorrect Options mark.
    pub(in crate::shell) fn r8_note_correction(&mut self, before: &(usize, String)) {
        let after = self.r8_newest_step();
        let show = self
            .view
            .engine()
            .autocorrect_options()
            .is_some_and(|o| o.show_buttons);
        if !show || &after == before || after.1 != "AutoCorrect" {
            return;
        }
        let Some(s) = self.view.engine().session() else {
            return;
        };
        let at = s.selection().focus;
        let text = s.para_text(at.para).unwrap_or_default();
        let head = text.get(..at.offset).unwrap_or("");
        // Back over the delimiter just typed, then over the word.
        let body = head.trim_end_matches(|c: char| !c.is_alphanumeric());
        let start = body
            .char_indices()
            .rev()
            .find(|(_, c)| c.is_whitespace())
            .map_or(0, |(i, c)| i + c.len_utf8());
        self.r8.ac_mark = Some(bo_edit::Pos::new(at.para, start));
    }

    /// The AutoCorrect Options mark: a short blue bar under the corrected word's first letter
    /// (window pixels), as the reference shows it.
    pub(in crate::shell) fn r8_ac_mark_rect(&self) -> Option<[f32; 4]> {
        let a = self.r8.ac_mark?;
        let s = self.view.engine().session()?;
        let text = s.para_text(a.para)?;
        let next = text.get(a.offset..)?.chars().next()?;
        let b = bo_edit::Pos::new(a.para, a.offset + next.len_utf8());
        let r = self.view.engine().range_rects(a, b).into_iter().next()?;
        let slot = self
            .view
            .slots()
            .into_iter()
            .find(|sl| sl.index == r.page)?;
        let k = self.view.scale();
        let x = (slot.x as f32 + r.x * k).round();
        let y = (slot.y as f32 + (r.y + r.h) * k).round();
        let w = (r.w * k).round().max((6.0 * self.scale).round());
        Some([x, y, w, (2.0 * self.scale).round().max(1.0)])
    }

    fn r8_paint_ac_mark(&self, p: &mut dyn Painter) {
        if let Some(r) = self.r8_ac_mark_rect() {
            p.fill_rect(
                r[0],
                r[1],
                r[2],
                r[3],
                bo_render::Rgba(0x2B, 0x57, 0x9A, 255),
            );
        }
    }

    /// A click on the AutoCorrect Options mark opens its menu: change back (undo the
    /// correction) or the AutoCorrect dialog.
    pub(in crate::shell) fn r8_ac_mark_press(&mut self, x: f32, y: f32) -> Option<bool> {
        let r = self.r8_ac_mark_rect()?;
        let pad = (4.0 * self.scale).round();
        let inside =
            x >= r[0] - pad && x <= r[0] + r[2] + pad && y >= r[1] - pad && y <= r[1] + r[3] + pad;
        if !inside {
            self.r8.ac_mark = None;
            return None;
        }
        let items = vec![
            MenuItem {
                label: self.strings.get("docR8.acMark.undo").to_owned(),
                choice: Choice::Command(format!("{UNDO_STEPS}1")),
                checked: false,
                dim: false,
            },
            MenuItem {
                label: self.strings.get("docR8.acMark.options").to_owned(),
                choice: Choice::Command("tools.autoCorrect".into()),
                checked: false,
                dim: false,
            },
        ];
        self.r8.ac_mark = None;
        self.menu = Some(Menu::open(
            items,
            [r[0], r[1], r[2], r[3]],
            self.size,
            self.scale,
        ));
        Some(true)
    }

    /// Paint the round-8 dialogs over the window.
    pub(in crate::shell) fn r8_paint(&self, p: &mut dyn Painter) {
        if let Some(d) = &self.r8.autocorrect {
            d.paint(p, self.size, true);
        }
    }

    /// The names of the steps Undo would revert, newest first (the reference's labels:
    /// `Typing "…"`, the formatting kind, AutoCorrect …).
    pub(in crate::shell) fn r8_undo_labels(&self) -> Vec<String> {
        self.view
            .engine()
            .undo_list()
            .into_iter()
            .take(UNDO_ROWS)
            .map(|l| {
                let key = match l.kind.as_str() {
                    // Enter and breaks: "Typing" alone, as on the reference.
                    "SplitParagraph" | "InsertPageBreak" => "docR8.undo.typingPlain",
                    "FormatText" if l.text == "Bold" => "docR8.undo.bold",
                    "FormatText" if l.text == "Italic" => "docR8.undo.italic",
                    "FormatText" if l.text == "Underline" => "docR8.undo.underline",
                    "InsertText" => {
                        let text: String = l
                            .text
                            .chars()
                            .filter(|c| !c.is_control())
                            .take(40)
                            .collect();
                        return self.strings.fmt("docR8.undo.typing", &[("text", &text)]);
                    }
                    "FormatText" | "ClearFormatting" => "docR8.undo.font",
                    "FormatParagraph" | "Tab" => "docR8.undo.paragraph",
                    "ApplyStyle" => "docR8.undo.style",
                    "AutoCorrect" => "docR8.undo.autoCorrect",
                    "DeleteBackward" | "DeleteForward" | "DeleteWordBackward"
                    | "DeleteWordForward" | "Cut" => "docR8.undo.delete",
                    k if k.starts_with("Paste") => "docR8.undo.paste",
                    "ChangeCase" => "docR8.undo.case",
                    "MoveParagraphs" => "docR8.undo.move",
                    _ => "docR8.undo.edit",
                };
                self.strings.get(key).to_owned()
            })
            .collect()
    }

    /// The Undo list under its arrow in the title bar.
    fn r8_open_undo_list(&mut self) -> bool {
        let labels = self.r8_undo_labels();
        let Some(anchor) = self
            .title
            .quick_rect("edit.undo")
            .zip(self.title.quick_rect(crate::titlebar::UNDO_LIST))
            .map(|(a, b)| [a[0], a[1], b[0] + b[2] - a[0], a[3]])
        else {
            return false;
        };
        if labels.is_empty() {
            return false;
        }
        let n = labels.len();
        let mut items: Vec<MenuItem> = labels
            .into_iter()
            .enumerate()
            .map(|(i, label)| MenuItem {
                label,
                choice: Choice::Command(format!("{UNDO_STEPS}{}", i + 1)),
                checked: false,
                dim: false,
            })
            .collect();
        // The reference ends the list with Cancel under a line.
        items.push(MenuItem {
            label: self.strings.get("docR8.undo.cancel").to_owned(),
            choice: Choice::Command(format!("{UNDO_STEPS}0")),
            checked: false,
            dim: false,
        });
        self.menu = Some(
            Menu::open(items, anchor, self.size, self.scale)
                .with_decor(vec![None; n + 1], vec![n])
                .with_range_footer(self.strings.get("docR8.undo.footer").to_owned()),
        );
        true
    }
}
