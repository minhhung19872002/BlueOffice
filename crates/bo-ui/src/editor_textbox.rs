//! Comment › Text Box typed in place, as the reference editor: a click with the tool puts an
//! empty box on the page (its first line centred on the click) and the text is typed into it;
//! lines wrap at the box's fixed width and the box grows downwards. A click elsewhere, Escape or
//! another command ends it and the text becomes a text box comment (one undo step); nothing
//! typed, no comment. The look is the reference's saved one (`bo_pdfread::TextBoxLook`: 9 pt
//! `#0000FF` text, 1 pt `#FF0000` border, white fill, 100 pt wide text area). Typing goes
//! through the typewriter draft (`editor_typewriter`, `TwDraft::boxed`).
use crate::editor_typewriter::{font, TwDraft};
use crate::editor_window::EditorWindow;
use crate::{Painter, TextMeasure};
use bo_pdfread::TextBoxLook;
use bo_render::Rgba;

/// Width of the text area (points), as the reference's text box.
pub(crate) const TB_WIDTH: f32 = 100.0;

fn look() -> TextBoxLook {
    TextBoxLook::default()
}

/// Width of `s` in the text box's font (points).
fn width_of(s: &str, m: &dyn TextMeasure) -> f32 {
    // Measured large and scaled: small sizes round each advance to whole pixels.
    m.measure_font(s, &font(100.0)).0 * look().size / 100.0
}

/// Lines of `d`'s text wrapped at the box width (words; a word longer than a line is cut).
pub(crate) fn wrap(d: &mut TwDraft, m: &dyn TextMeasure) {
    let mut lines = Vec::new();
    for para in d.text.split('\n') {
        let mut line = String::new();
        for word in para.split_inclusive(' ') {
            let cand = format!("{line}{word}");
            if line.is_empty() || width_of(cand.trim_end(), m) <= TB_WIDTH {
                line = cand;
                // A single word wider than the box breaks by characters.
                while width_of(line.trim_end(), m) > TB_WIDTH && line.chars().count() > 1 {
                    let mut head = String::new();
                    for c in line.chars() {
                        if !head.is_empty() && width_of(&format!("{head}{c}"), m) > TB_WIDTH {
                            break;
                        }
                        head.push(c);
                    }
                    let rest = line[head.len()..].to_owned();
                    lines.push(head);
                    line = rest;
                }
            } else {
                lines.push(std::mem::replace(&mut line, word.to_owned()));
            }
        }
        lines.push(line);
    }
    d.widths = lines.iter().map(|l| width_of(l, m)).collect();
    d.lines = lines;
}

impl EditorWindow {
    /// Start typing a text box whose first line is centred on `at` of `page`.
    pub(crate) fn tb_start(&mut self, page: usize, at: [f32; 2]) {
        self.tw_commit();
        let l = look();
        self.tw = Some(TwDraft {
            page,
            at: [at[0] - 3.0, at[1] - l.line_height() / 2.0],
            boxed: true,
            lines: vec![String::new()],
            widths: vec![0.0],
            ..TwDraft::default()
        });
        self.caret_on = true;
    }

    /// The typed text becomes a text box comment.
    pub(crate) fn tb_commit(&mut self, d: TwDraft) {
        let author = self.prefs.author.clone();
        let mut lines = d.lines;
        while lines.last().is_some_and(String::is_empty) && lines.len() > 1 {
            lines.pop();
        }
        let lines: Vec<String> = lines.iter().map(|l| l.trim_end().to_owned()).collect();
        let page = d.page;
        let r =
            self.doc_edit(|doc| doc.add_text_box(page, d.at, TB_WIDTH, &lines, &look(), &author));
        match r {
            Ok(()) => {
                if let Some(v) = self.r.view_mut() {
                    v.invalidate_page(page);
                }
                self.after_doc_edit();
            }
            Err(e) => self.set_status(&e),
        }
    }

    /// The text box being typed: white area, border, text and caret.
    pub(crate) fn paint_tb(&self, p: &mut dyn Painter) {
        let Some(d) = &self.tw else { return };
        let Some(v) = self.view() else { return };
        let l = look();
        let lh = l.line_height();
        let n = d.lines.len().max(1);
        let r = [
            d.at[0],
            d.at[1],
            d.at[0] + TB_WIDTH,
            d.at[1] + lh * n as f32,
        ];
        let Some(w) = v.doc_rect_to_window(d.page, r) else {
            return;
        };
        let s = w[2] / TB_WIDTH;
        let bw = (l.width * s).round().max(1.0);
        let c = |c: [u8; 3]| Rgba(c[0], c[1], c[2], 255);
        p.fill_rect(w[0], w[1], w[2], w[3], Rgba(255, 255, 255, 255));
        let b = c(l.border);
        p.fill_rect(w[0] - bw, w[1] - bw, w[2] + 2.0 * bw, bw, b);
        p.fill_rect(w[0] - bw, w[1] + w[3], w[2] + 2.0 * bw, bw, b);
        p.fill_rect(w[0] - bw, w[1], bw, w[3], b);
        p.fill_rect(w[0] + w[2], w[1], bw, w[3], b);
        let f = font(l.size * s);
        // The painter places text by its top; the font's ascent puts the baseline 0.905 em
        // down like the reference.
        for (i, line) in d.lines.iter().enumerate() {
            p.text_font(line, w[0], w[1] + lh * i as f32 * s, &f, c(l.text));
        }
        if self.caret_on {
            let cx = d.widths.last().copied().unwrap_or(0.0);
            let y = w[1] + lh * (n - 1) as f32 * s;
            p.fill_rect(
                (w[0] + cx * s).round(),
                y.round(),
                1.0,
                (lh * s).round(),
                Rgba(0, 0, 0, 255),
            );
        }
    }
}
