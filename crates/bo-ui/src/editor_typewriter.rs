//! Typewriter typed in place, as the reference editor: a click with the Typewriter tool puts a
//! caret on the page and the text is typed there (Enter starts a new line, Backspace removes
//! the last character); a click elsewhere, Escape or another command ends it and the text
//! becomes a typewriter comment (one undo step). Nothing typed: no comment.
use crate::editor_window::EditorWindow;
use crate::{Font, Input, Key, Painter, TextMeasure};
use bo_pdfread::MarkupKind;
use bo_render::Rgba;

/// Font size of typewriter text (points), as [`EditorWindow::add_comment`] writes it.
pub(crate) const TW_SIZE: f32 = 12.0;
/// Frame of the text being typed (the reference: a thin blue box).
const FRAME: Rgba = Rgba(0, 120, 215, 255);

/// A typewriter comment being typed.
#[derive(Debug, Clone, Default)]
pub(crate) struct TwDraft {
    pub page: usize,
    /// Top-left corner (page points).
    pub at: [f32; 2],
    pub text: String,
    /// Width of each line (points).
    pub widths: Vec<f32>,
}

fn font(size: f32) -> Font<'static> {
    Font {
        family: "Arial",
        size,
        weight: 400,
        italic: false,
        underline: false,
    }
}

impl EditorWindow {
    /// Start typing a typewriter comment at `at` of `page` (ends any other first).
    pub(crate) fn tw_start(&mut self, page: usize, at: [f32; 2]) {
        self.tw_commit();
        self.tw = Some(TwDraft {
            page,
            at,
            ..TwDraft::default()
        });
        self.caret_on = true;
    }

    /// End typing: the text becomes a comment.
    pub(crate) fn tw_commit(&mut self) {
        let Some(d) = self.tw.take() else { return };
        if d.text.trim().is_empty() {
            return;
        }
        let [x, y] = d.at;
        self.add_comment(
            MarkupKind::Typewriter,
            d.page,
            [x, y, x + 10.0, y + 10.0],
            Vec::new(),
            d.text,
        );
    }

    /// Keys, text and clicks while a typewriter comment is typed; true when used.
    pub(crate) fn tw_input(&mut self, input: &Input, m: &dyn TextMeasure) -> bool {
        let Some(d) = self.tw.as_mut() else {
            return false;
        };
        match input {
            Input::Text(s) => d.text.extend(s.chars().filter(|c| !c.is_control())),
            Input::Key(Key::Backspace, _) => {
                d.text.pop();
            }
            Input::Key(Key::Enter, _) => d.text.push('\n'),
            Input::Key(Key::Escape, _) => {
                self.tw_commit();
                return true;
            }
            // The keyboard belongs to the text: no page or document shortcuts.
            Input::Key(..) => return true,
            Input::Blink => {
                self.caret_on = !self.caret_on;
                return true;
            }
            // A click ends the text and goes on (a new one with the tool still on).
            Input::Press { .. } | Input::Context { .. } => {
                self.tw_commit();
                return false;
            }
            _ => return false,
        }
        d.widths = d
            .text
            .split('\n')
            .map(|l| m.measure_font(l, &font(TW_SIZE)).0)
            .collect();
        self.caret_on = true;
        true
    }

    /// The text being typed, its frame and the caret.
    pub(crate) fn paint_tw(&self, p: &mut dyn Painter) {
        let Some(d) = &self.tw else { return };
        let Some(v) = self.view() else { return };
        let lines: Vec<&str> = d.text.split('\n').collect();
        let lh = TW_SIZE * 1.2;
        let h = lh * lines.len() as f32 + 4.0;
        let width = d.widths.iter().copied().fold(0.0, f32::max);
        let r = [d.at[0], d.at[1], d.at[0] + width + 6.0, d.at[1] + h];
        let Some(w) = v.doc_rect_to_window(d.page, r) else {
            return;
        };
        let s = w[2] / (r[2] - r[0]).max(1e-3);
        let c = self.comment.color;
        let ink = Rgba(c[0], c[1], c[2], 255);
        let f = font(TW_SIZE * s);
        for (i, l) in lines.iter().enumerate() {
            p.text_font(l, w[0] + 3.0 * s, w[1] + (2.0 + lh * i as f32) * s, &f, ink);
        }
        // Frame: 1 px lines around the box.
        p.fill_rect(w[0], w[1], w[2], 1.0, FRAME);
        p.fill_rect(w[0], w[1] + w[3] - 1.0, w[2], 1.0, FRAME);
        p.fill_rect(w[0], w[1], 1.0, w[3], FRAME);
        p.fill_rect(w[0] + w[2] - 1.0, w[1], 1.0, w[3], FRAME);
        if self.caret_on {
            // After the last character.
            let cx = d.widths.last().copied().unwrap_or(0.0);
            let y = w[1] + (2.0 + lh * (lines.len() - 1) as f32) * s;
            p.fill_rect(
                (w[0] + (3.0 + cx) * s).round(),
                y,
                1.0_f32.max(s.round() * 0.75),
                lh * s,
                Rgba(0, 0, 0, 255),
            );
        }
    }
}
