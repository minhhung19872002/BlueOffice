//! Small modal dialogs with text fields (Find, Replace, Link, Comment, Word Count).
use crate::theme::Theme;
use crate::{Key, Mods, Painter, TextMeasure, TextStyle};
use bo_render::Rgba;

/// What a dialog is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogKind {
    /// Find text.
    Find,
    /// Find and replace.
    Replace,
    /// Insert a hyperlink.
    Link,
    /// New comment.
    Comment,
    /// Word count (message only).
    WordCount,
    /// Font of the selection (family, size, colour).
    Font,
    /// Paragraph indents and spacing.
    Paragraph,
    /// Page setup of the document: margins, paper size, header / footer distance (cm).
    PageSetup,
}

/// A dialog button's action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogButton {
    /// Confirm.
    Ok,
    /// Dismiss.
    Cancel,
    /// Find the next match.
    FindNext,
    /// Replace the current match.
    Replace,
    /// Replace every match.
    ReplaceAll,
}

struct Field {
    label: String,
    value: Vec<char>,
    caret: usize,
    /// Caret x offset in device pixels (measured by the window).
    caret_x: f32,
    /// First character shown (scrolled for long values).
    scroll_x: f32,
    /// A check box instead of a text box (its state).
    check: Option<bool>,
}

#[derive(Clone, Copy, Default)]
struct R {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

impl R {
    fn contains(self, x: f32, y: f32) -> bool {
        x >= self.x && x < self.x + self.w && y >= self.y && y < self.y + self.h
    }
}

/// An open dialog.
pub struct Dialog {
    /// What it is for.
    pub kind: DialogKind,
    title: String,
    fields: Vec<Field>,
    focus: usize,
    buttons: Vec<(String, DialogButton, R)>,
    /// Status line under the fields (matches found, replacements made, counts).
    message: Vec<String>,
    frame: R,
    field_rects: Vec<R>,
    hover: Option<usize>,
    scale: f32,
}

impl Dialog {
    /// A dialog titled `title` with `fields` (label, initial value) and `buttons`.
    pub fn new(
        kind: DialogKind,
        title: String,
        fields: Vec<(String, String)>,
        buttons: Vec<(String, DialogButton)>,
        message: Vec<String>,
    ) -> Self {
        Self {
            kind,
            title,
            fields: fields
                .into_iter()
                .map(|(label, v)| {
                    let value: Vec<char> = v.chars().collect();
                    Field {
                        label,
                        caret: value.len(),
                        value,
                        caret_x: 0.0,
                        scroll_x: 0.0,
                        check: None,
                    }
                })
                .collect(),
            focus: 0,
            buttons: buttons
                .into_iter()
                .map(|(l, b)| (l, b, R::default()))
                .collect(),
            message,
            frame: R::default(),
            field_rects: Vec::new(),
            hover: None,
            scale: 1.0,
        }
    }

    /// Add check boxes (label, checked) after the text fields.
    #[must_use]
    pub fn with_checks(mut self, checks: Vec<(String, bool)>) -> Self {
        for (label, on) in checks {
            self.fields.push(Field {
                label,
                value: Vec::new(),
                caret: 0,
                caret_x: 0.0,
                scroll_x: 0.0,
                check: Some(on),
            });
        }
        self
    }

    /// Field values; check boxes give `1` or `0`.
    pub fn values(&self) -> Vec<String> {
        self.fields
            .iter()
            .map(|f| match f.check {
                Some(on) => if on { "1" } else { "0" }.to_owned(),
                None => f.value.iter().collect(),
            })
            .collect()
    }

    /// Height of the font preview box (Font dialog).
    fn preview_h(&self) -> f32 {
        if self.kind == DialogKind::Font {
            self.px(64.0)
        } else {
            0.0
        }
    }

    /// Replace the status lines.
    pub fn set_message(&mut self, lines: Vec<String>) {
        self.message = lines;
    }

    fn px(&self, v: f32) -> f32 {
        (v * self.scale).round()
    }

    fn style(&self) -> TextStyle {
        TextStyle {
            size: self.px(13.0),
            bold: false,
        }
    }

    /// Lay out centred in a window of `size`, and measure caret positions.
    pub fn layout(&mut self, m: &dyn TextMeasure, size: (f32, f32), scale: f32) {
        self.scale = scale;
        let w = self.px(420.0);
        let field_h = self.px(28.0);
        let check_h = self.px(26.0);
        let mut h = self.px(52.0);
        for f in &self.fields {
            h += if f.check.is_some() {
                check_h
            } else {
                self.px(22.0) + field_h + self.px(8.0)
            };
        }
        h += self.preview_h()
            + if self.preview_h() > 0.0 {
                self.px(10.0)
            } else {
                0.0
            };
        h += self.message.len() as f32 * self.px(20.0) + self.px(8.0);
        h += self.px(48.0);
        self.frame = R {
            x: ((size.0 - w) / 2.0).round(),
            y: (size.1 * 0.22).round(),
            w,
            h,
        };
        let mut y = self.frame.y + self.px(48.0);
        self.field_rects.clear();
        for f in &self.fields {
            if f.check.is_some() {
                // Check box row: the whole row toggles.
                self.field_rects.push(R {
                    x: self.frame.x + self.px(20.0),
                    y,
                    w: w - self.px(40.0),
                    h: check_h - self.px(4.0),
                });
                y += check_h;
                continue;
            }
            y += self.px(22.0);
            self.field_rects.push(R {
                x: self.frame.x + self.px(20.0),
                y,
                w: w - self.px(40.0),
                h: field_h,
            });
            y += field_h + self.px(8.0);
        }
        // Buttons, right-aligned.
        let bh = self.px(30.0);
        let by = self.frame.y + h - bh - self.px(14.0);
        let mut bx = self.frame.x + w - self.px(20.0);
        let style = self.style();
        for (label, _, r) in self.buttons.iter_mut().rev() {
            let bw =
                (m.measure(label, style).0 + (24.0 * scale).round()).max((80.0 * scale).round());
            bx -= bw;
            *r = R {
                x: bx,
                y: by,
                w: bw,
                h: bh,
            };
            bx -= (8.0 * scale).round();
        }
        // Caret offsets, keeping the caret inside the box.
        let inner = (w - self.px(40.0) - self.px(16.0)).max(1.0);
        for f in &mut self.fields {
            let before: String = f.value[..f.caret].iter().collect();
            f.caret_x = m.measure(&before, style).0;
            if f.caret_x - f.scroll_x > inner {
                f.scroll_x = f.caret_x - inner;
            } else if f.caret_x < f.scroll_x {
                f.scroll_x = f.caret_x;
            }
        }
    }

    /// Typed text goes into the focused field.
    pub fn text(&mut self, s: &str) {
        if let Some(f) = self.fields.get_mut(self.focus) {
            if let Some(on) = f.check.as_mut() {
                if s == " " {
                    *on = !*on;
                }
                return;
            }
            for c in s.chars().filter(|c| !c.is_control()) {
                f.value.insert(f.caret, c);
                f.caret += 1;
            }
        }
    }

    /// A key; returns a button action (Enter: the first button, Escape: Cancel).
    pub fn key(&mut self, key: Key, m: Mods) -> Option<DialogButton> {
        let n = self.fields.len();
        if let Some(f) = self
            .fields
            .get_mut(self.focus)
            .filter(|f| f.check.is_none())
        {
            match key {
                Key::Left => f.caret = f.caret.saturating_sub(1),
                Key::Right => f.caret = (f.caret + 1).min(f.value.len()),
                Key::Home => f.caret = 0,
                Key::End => f.caret = f.value.len(),
                Key::Backspace if f.caret > 0 => {
                    f.caret -= 1;
                    f.value.remove(f.caret);
                }
                Key::Delete if f.caret < f.value.len() => {
                    f.value.remove(f.caret);
                }
                Key::Letter('a') if m.ctrl => f.caret = f.value.len(),
                _ => {}
            }
        }
        match key {
            Key::Tab if n > 0 => {
                self.focus = if m.shift {
                    (self.focus + n - 1) % n
                } else {
                    (self.focus + 1) % n
                };
                None
            }
            Key::Enter => self.buttons.first().map(|b| b.1),
            Key::Escape => Some(DialogButton::Cancel),
            _ => None,
        }
    }

    /// Whether a window position is over the dialog.
    pub fn contains(&self, x: f32, y: f32) -> bool {
        self.frame.contains(x, y)
    }

    /// Pointer moved: `true` when the hovered button changed.
    pub fn hover(&mut self, x: f32, y: f32) -> bool {
        let h = self.buttons.iter().position(|b| b.2.contains(x, y));
        std::mem::replace(&mut self.hover, h) != h
    }

    /// Click: focuses a field (caret at the end) or returns a button action.
    pub fn press(&mut self, x: f32, y: f32) -> Option<DialogButton> {
        if let Some(i) = self.field_rects.iter().position(|r| r.contains(x, y)) {
            self.focus = i;
            if let Some(f) = self.fields.get_mut(i) {
                f.caret = f.value.len();
                if let Some(on) = f.check.as_mut() {
                    *on = !*on;
                }
            }
            return None;
        }
        self.buttons
            .iter()
            .find(|b| b.2.contains(x, y))
            .map(|b| b.1)
    }

    /// Paint with a dimmed backdrop over the window.
    pub fn paint(&self, p: &mut dyn Painter, t: &Theme, window: (f32, f32), caret_on: bool) {
        p.fill_rect(0.0, 0.0, window.0, window.1, Rgba(0, 0, 0, 40));
        let f = self.frame;
        for (k, a) in [(6.0, 14u8), (3.0, 22), (1.0, 30)] {
            let o = self.px(k);
            p.fill_rect(
                f.x - o,
                f.y - o + self.px(3.0),
                f.w + 2.0 * o,
                f.h + 2.0 * o,
                Rgba(0, 0, 0, a),
            );
        }
        p.fill_rect(f.x, f.y, f.w, f.h, t.border);
        p.fill_rect(f.x + 1.0, f.y + 1.0, f.w - 2.0, f.h - 2.0, CTL_FACE);
        let style = self.style();
        let line = style.size * 1.33;
        p.text(
            &self.title,
            f.x + self.px(20.0),
            f.y + self.px(16.0),
            TextStyle {
                size: self.px(15.0),
                bold: true,
            },
            t.text,
        );
        for (i, (field, r)) in self.fields.iter().zip(&self.field_rects).enumerate() {
            if let Some(on) = field.check {
                // Standard check box: white box with a grey frame (blue when focused) and a
                // dark tick, the label to its right.
                let b = self.px(14.0);
                let (bx, by) = (r.x, (r.y + (r.h - b) / 2.0).round());
                let edge = if i == self.focus {
                    CTL_FOCUS
                } else {
                    EDIT_EDGE
                };
                p.fill_rect(bx, by, b, b, edge);
                p.fill_rect(bx + 1.0, by + 1.0, b - 2.0, b - 2.0, t.surface);
                if on {
                    let k = b / 4.0;
                    p.path(
                        &[
                            bo_render::PathSeg::M(bx + k, by + b / 2.0),
                            bo_render::PathSeg::L(bx + b / 2.0 - k / 3.0, by + b - k * 1.2),
                            bo_render::PathSeg::L(bx + b - k, by + k),
                        ],
                        None,
                        Some((t.text, (1.6 * self.scale).max(1.5))),
                    );
                }
                p.text(
                    &field.label,
                    bx + b + self.px(8.0),
                    r.y + (r.h - line) / 2.0,
                    style,
                    t.text,
                );
                continue;
            }
            p.text(&field.label, r.x, r.y - self.px(20.0), style, t.text);
            let focused = i == self.focus;
            // Standard Windows edit box: grey frame, blue when focused.
            p.fill_rect(
                r.x,
                r.y,
                r.w,
                r.h,
                if focused { CTL_FOCUS } else { EDIT_EDGE },
            );
            p.fill_rect(r.x + 1.0, r.y + 1.0, r.w - 2.0, r.h - 2.0, t.surface);
            let tx = r.x + self.px(8.0);
            p.push_clip(r.x + 1.0, r.y + 1.0, r.w - 2.0, r.h - 2.0);
            let value: String = field.value.iter().collect();
            p.text(
                &value,
                tx - field.scroll_x,
                r.y + (r.h - line) / 2.0,
                style,
                t.text,
            );
            if focused && caret_on {
                p.fill_rect(
                    (tx + field.caret_x - field.scroll_x).round(),
                    r.y + self.px(5.0),
                    self.scale.max(1.0).round(),
                    r.h - self.px(10.0),
                    t.text,
                );
            }
            p.pop_clip();
        }
        let mut my = self
            .field_rects
            .last()
            .map_or(f.y + self.px(52.0), |r| r.y + r.h + self.px(10.0));
        if self.preview_h() > 0.0 {
            // Font preview: sample text in the chosen family, size and style, in a white box.
            let v = self.values();
            let get = |i: usize| v.get(i).cloned().unwrap_or_default();
            let pt = get(1)
                .trim()
                .replace(',', ".")
                .parse::<f32>()
                .unwrap_or(11.0);
            let family = get(0);
            let font = crate::Font {
                family: if family.trim().is_empty() {
                    "Times New Roman"
                } else {
                    family.trim()
                },
                size: (pt * self.scale * 96.0 / 72.0).clamp(self.px(8.0), self.px(36.0)),
                weight: if get(3) == "1" { 700 } else { 400 },
                italic: get(4) == "1",
                underline: get(5) == "1",
            };
            let b = R {
                x: f.x + self.px(20.0),
                y: my,
                w: f.w - self.px(40.0),
                h: self.preview_h(),
            };
            p.fill_rect(b.x, b.y, b.w, b.h, EDIT_EDGE);
            p.fill_rect(b.x + 1.0, b.y + 1.0, b.w - 2.0, b.h - 2.0, t.surface);
            p.push_clip(b.x + 1.0, b.y + 1.0, b.w - 2.0, b.h - 2.0);
            let ink =
                crate::theme::parse_color(&format!("#{}", get(2).trim().trim_start_matches('#')))
                    .unwrap_or(t.text);
            p.text_font(
                "AaBbĐđ Ơơ Ưư 123",
                b.x + self.px(10.0),
                b.y + (b.h - font.size * 1.3) / 2.0,
                &font,
                ink,
            );
            p.pop_clip();
            my += b.h + self.px(10.0);
        }
        for m in &self.message {
            p.text(m, f.x + self.px(20.0), my, style, t.text);
            my += self.px(20.0);
        }
        for (i, (label, _, r)) in self.buttons.iter().enumerate() {
            // Standard Windows push buttons, as in a desktop word processor's dialogs:
            // grey face and frame, the default button framed in blue, light blue on hover.
            let primary = i == 0;
            let hover = self.hover == Some(i);
            let edge = if primary || hover {
                CTL_FOCUS
            } else {
                BUTTON_EDGE
            };
            let face = if hover { BUTTON_HOVER } else { BUTTON_FACE };
            let fg = t.text;
            p.fill_rect(r.x, r.y, r.w, r.h, edge);
            p.fill_rect(r.x + 1.0, r.y + 1.0, r.w - 2.0, r.h - 2.0, face);
            p.text(
                label,
                r.x + self.px(12.0),
                r.y + (r.h - line) / 2.0,
                style,
                fg,
            );
        }
    }
}

/// Dialog face, edit box frame, push button face / frame / hover and the focus blue of
/// standard Windows controls (measured on a desktop word processor's dialogs).
const CTL_FACE: Rgba = Rgba(0xf0, 0xf0, 0xf0, 255);
const EDIT_EDGE: Rgba = Rgba(0x7a, 0x7a, 0x7a, 255);
const BUTTON_FACE: Rgba = Rgba(0xe1, 0xe1, 0xe1, 255);
const BUTTON_EDGE: Rgba = Rgba(0xad, 0xad, 0xad, 255);
const BUTTON_HOVER: Rgba = Rgba(0xe5, 0xf1, 0xfb, 255);
const CTL_FOCUS: Rgba = Rgba(0x00, 0x78, 0xd7, 255);

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixed;
    impl TextMeasure for Fixed {
        fn measure(&self, text: &str, style: TextStyle) -> (f32, f32) {
            (
                text.chars().count() as f32 * style.size * 0.5,
                style.size * 1.33,
            )
        }
    }

    #[test]
    fn fields_edit_and_buttons_answer() {
        let mut d = Dialog::new(
            DialogKind::Replace,
            "Replace".into(),
            vec![
                ("Find what".into(), "ab".into()),
                ("Replace with".into(), String::new()),
            ],
            vec![
                ("Find Next".into(), DialogButton::FindNext),
                ("Cancel".into(), DialogButton::Cancel),
            ],
            vec![],
        );
        d.layout(&Fixed, (1200.0, 800.0), 1.0);
        d.text("c");
        d.key(Key::Left, Mods::default());
        d.key(Key::Backspace, Mods::default());
        d.key(Key::Tab, Mods::default());
        d.text("xyz");
        assert_eq!(d.values(), vec!["ac".to_owned(), "xyz".to_owned()]);
        assert_eq!(
            d.key(Key::Enter, Mods::default()),
            Some(DialogButton::FindNext)
        );
        assert_eq!(
            d.key(Key::Escape, Mods::default()),
            Some(DialogButton::Cancel)
        );
        d.layout(&Fixed, (1200.0, 800.0), 1.0);
        let r = d.buttons[1].2;
        assert_eq!(d.press(r.x + 2.0, r.y + 2.0), Some(DialogButton::Cancel));
    }
}
