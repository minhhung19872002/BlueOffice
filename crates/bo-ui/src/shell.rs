//! The BlueDoc window: ribbon, document view and status bar, with input routing.
use crate::doc_view::DocView;
use crate::icons::Icons;
use crate::ribbon::{Ribbon, RibbonHit};
use crate::strings::Strings;
use crate::theme::Theme;
use crate::{Input, PageStore, Painter, TextMeasure, TextStyle};

/// Window contents of BlueDoc.
pub struct DocWindow {
    view: DocView,
    ribbon: Ribbon,
    theme: Theme,
    icons: Icons,
    strings: Strings,
    size: (f32, f32),
    scale: f32,
    /// The pointer went down in the document (drags select text there).
    doc_press: bool,
}

impl DocWindow {
    /// Wrap a document view with BlueDoc's interface in `locale`.
    pub fn new(view: DocView, locale: &str) -> Self {
        Self {
            view,
            ribbon: Ribbon::new(),
            theme: Theme::light("doc"),
            icons: Icons::load(),
            strings: Strings::load(locale),
            size: (800.0, 600.0),
            scale: 1.0,
            doc_press: false,
        }
    }

    /// The document view.
    pub fn view(&self) -> &DocView {
        &self.view
    }

    fn status_h(&self) -> f32 {
        (24.0 * self.scale).round()
    }

    fn relayout(&mut self, text: &dyn TextMeasure) {
        self.ribbon
            .layout(text, &self.strings, self.size.0, self.scale);
        let top = self.ribbon.height();
        let h = (self.size.1 - top - self.status_h()).max(1.0);
        self.view.set_frame(0.0, top, self.size.0, h);
    }

    /// Handle an input event; `true` when a new frame is needed.
    pub fn handle(&mut self, input: Input, text: &dyn TextMeasure) -> bool {
        let in_ribbon = |w: &Self, y: f32| y < w.ribbon.height();
        match input {
            Input::Resize { width, height } => {
                self.size = (width.max(1.0), height.max(1.0));
                self.relayout(text);
                true
            }
            Input::Scale(scale) => {
                self.scale = scale;
                self.view.handle(Input::Scale(scale));
                self.relayout(text);
                true
            }
            Input::Move { x, y } => self.ribbon.hover(x, y),
            Input::Press { x, y, .. } if in_ribbon(self, y) => {
                self.doc_press = false;
                self.ribbon.press(x, y)
            }
            Input::Press { .. } => {
                self.doc_press = true;
                let hovered = self.ribbon.hover(-1.0, -1.0);
                self.view.handle(input) || hovered
            }
            Input::Drag { .. } if !self.doc_press => false,
            Input::Release { x, y } => {
                self.doc_press = false;
                let (fired, changed) = self.ribbon.release(x, y);
                let ran = match fired {
                    Some(RibbonHit::Command(id)) => self.view.run_action(id),
                    _ => false,
                };
                if self.ribbon.needs_layout() {
                    self.relayout(text);
                }
                ran || changed || fired.is_some()
            }
            Input::Wheel(_)
            | Input::Text(_)
            | Input::Key(..)
            | Input::Blink
            | Input::Drag { .. } => self.view.handle(input),
        }
    }

    /// Update page images of the visible pages.
    pub fn prepare(&mut self, store: &mut dyn PageStore) -> Result<(), String> {
        self.view.prepare(store)
    }

    /// Paint the whole window.
    pub fn paint(&self, p: &mut dyn Painter) {
        p.clear(self.theme.bg);
        self.view.paint(p);
        self.ribbon.paint(
            p,
            &self.theme,
            &self.icons,
            &self.strings,
            self.view.format(),
        );
        // Status bar.
        let (w, h) = self.size;
        let sh = self.status_h();
        let y = h - sh;
        p.fill_rect(0.0, y, w, sh, self.theme.bg);
        p.fill_rect(0.0, y, w, 1.0, self.theme.border);
        let style = TextStyle {
            size: (12.0 * self.scale).round(),
            bold: false,
        };
        let ty = y + (sh - style.size * 1.33) / 2.0;
        let page = self.strings.fmt(
            "status.page",
            &[
                ("current", &(self.view.current_page() + 1).to_string()),
                ("total", &self.view.page_count().to_string()),
            ],
        );
        let words = self.strings.fmt(
            "status.words",
            &[("count", &self.view.word_count().to_string())],
        );
        let x0 = (12.0 * self.scale).round();
        p.text(&page, x0, ty, style, self.theme.text_muted);
        p.text(
            &words,
            x0 + (110.0 * self.scale).round(),
            ty,
            style,
            self.theme.text_muted,
        );
        let zoom = self.strings.fmt(
            "status.zoom",
            &[(
                "value",
                &((self.view.zoom() * 100.0).round() as i32).to_string(),
            )],
        );
        p.text(
            &zoom,
            w - (60.0 * self.scale).round(),
            ty,
            style,
            self.theme.text_muted,
        );
    }

    /// Caret rectangle in window pixels (IME placement).
    pub fn caret_rect(&self) -> Option<[f32; 4]> {
        self.view.caret_rect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Key, Mods};
    use bo_engine::Engine;

    struct Fixed;
    impl TextMeasure for Fixed {
        fn measure(&self, text: &str, style: TextStyle) -> (f32, f32) {
            (
                text.chars().count() as f32 * style.size * 0.55,
                style.size * 1.33,
            )
        }
    }

    #[test]
    fn ribbon_button_runs_its_command_and_undo_restores() {
        let mut engine = Engine::with_bundled_fonts();
        engine.new_document("a.docx").unwrap();
        let mut w = DocWindow::new(DocView::new(engine).unwrap(), "vi-VN");
        w.handle(Input::Scale(1.25), &Fixed);
        w.handle(
            Input::Resize {
                width: 1600.0,
                height: 900.0,
            },
            &Fixed,
        );
        w.handle(Input::Text("abc".into()), &Fixed);
        w.handle(
            Input::Key(
                Key::Letter('a'),
                Mods {
                    ctrl: true,
                    ..Mods::default()
                },
            ),
            &Fixed,
        );
        // Find the Bold button on a grid over the ribbon.
        let (mut bx, mut by) = (None, 0.0);
        'scan: for y in (0..w.ribbon.height() as i32).step_by(4) {
            for x in (0..1600).step_by(4) {
                if w.ribbon.hit(x as f32, y as f32) == Some(RibbonHit::Command("format.bold")) {
                    (bx, by) = (Some(x as f32), y as f32);
                    break 'scan;
                }
            }
        }
        let bx = bx.expect("bold button");
        assert!(!w.view().format().bold);
        w.handle(
            Input::Press {
                x: bx,
                y: by,
                count: 1,
                shift: false,
            },
            &Fixed,
        );
        assert!(w.handle(Input::Release { x: bx, y: by }, &Fixed));
        assert!(w.view().format().bold, "the button made the selection bold");
        w.handle(
            Input::Key(
                Key::Letter('z'),
                Mods {
                    ctrl: true,
                    ..Mods::default()
                },
            ),
            &Fixed,
        );
        assert!(!w.view().format().bold, "undo");
        // A release over another control than the press does nothing.
        w.handle(
            Input::Press {
                x: bx,
                y: by,
                count: 1,
                shift: false,
            },
            &Fixed,
        );
        w.handle(Input::Release { x: 5.0, y: 600.0 }, &Fixed);
        assert!(!w.view().format().bold);
    }
}
