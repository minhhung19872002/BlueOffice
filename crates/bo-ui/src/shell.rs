//! The BlueDoc window: ribbon, document view and status bar, with input routing.
use crate::doc_view::DocView;
use crate::icons::Icons;
use crate::popup::{paint_tooltip, Choice, Menu, MenuItem};
use crate::ribbon::{Ribbon, RibbonHit};
use crate::strings::Strings;
use crate::theme::Theme;
use crate::titlebar::{NcHit, TitleBar};
use crate::{commands, Key, Mods};
use crate::{Input, PageStore, Painter, TextMeasure, TextStyle};
use bo_edit::CharFormat;
use bo_engine::edit::Command;

/// Something only the platform host can do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Request {
    /// Write the document to its file.
    Save,
    /// Minimise the window.
    Minimize,
    /// Maximise or restore the window.
    ToggleMaximize,
    /// Close the window.
    Close,
}

/// Window contents of BlueDoc.
pub struct DocWindow {
    view: DocView,
    title: TitleBar,
    name: String,
    requests: Vec<Request>,
    ribbon: Ribbon,
    theme: Theme,
    icons: Icons,
    strings: Strings,
    size: (f32, f32),
    scale: f32,
    /// The pointer went down in the document (drags select text there).
    doc_press: bool,
    /// Open drop-down list.
    menu: Option<Menu>,
    /// Ticks the pointer has rested on the hovered control, and the tooltip shown.
    rest: u32,
    tooltip: Option<(String, f32, [f32; 4])>,
}

/// Font sizes offered by the size box (points).
const SIZES: [f32; 16] = [
    8.0, 9.0, 10.0, 10.5, 11.0, 12.0, 14.0, 16.0, 18.0, 20.0, 22.0, 24.0, 26.0, 28.0, 36.0, 72.0,
];
/// Ticks (about 100 ms each) of rest before a tooltip shows.
const TOOLTIP_TICKS: u32 = 5;

impl DocWindow {
    /// Wrap the document `name` shown by `view` with BlueDoc's interface in `locale`.
    pub fn new(view: DocView, locale: &str, name: &str) -> Self {
        Self {
            view,
            title: TitleBar::new(),
            name: name.to_owned(),
            requests: Vec::new(),
            ribbon: Ribbon::new(),
            theme: Theme::light("doc"),
            icons: Icons::load(),
            strings: Strings::load(locale),
            size: (800.0, 600.0),
            scale: 1.0,
            doc_press: false,
            menu: None,
            rest: 0,
            tooltip: None,
        }
    }

    /// The document view.
    pub fn view(&self) -> &DocView {
        &self.view
    }

    /// The document view, for the host's file operations.
    pub fn view_mut(&mut self) -> &mut DocView {
        &mut self.view
    }

    /// Requests for the host made since the last call.
    pub fn take_requests(&mut self) -> Vec<Request> {
        std::mem::take(&mut self.requests)
    }

    /// What a window position is for the window manager (caption, window buttons, client).
    pub fn nc_hit(&self, x: f32, y: f32) -> NcHit {
        if self.menu.is_some() {
            return NcHit::Client;
        }
        self.title.hit(x, y)
    }

    fn status_h(&self) -> f32 {
        (24.0 * self.scale).round()
    }

    fn relayout(&mut self, text: &dyn TextMeasure) {
        self.title
            .layout(text, &self.strings, &self.name, self.size.0, self.scale);
        let top = self.title.height();
        self.ribbon
            .layout(text, &self.strings, self.size.0, self.scale, top);
        let top = self.ribbon.bottom();
        let h = (self.size.1 - top - self.status_h()).max(1.0);
        self.view.set_frame(0.0, top, self.size.0, h);
    }

    /// Handle an input event; `true` when a new frame is needed.
    pub fn handle(&mut self, input: Input, text: &dyn TextMeasure) -> bool {
        // Pointer and keyboard input goes to an open menu first.
        if self.menu.is_some() {
            if let Some(redraw) = self.menu_input(&input) {
                return redraw;
            }
        }
        let tip_before = self.tooltip.is_some();
        if matches!(
            input,
            Input::Move { .. }
                | Input::Press { .. }
                | Input::Wheel(_)
                | Input::Key(..)
                | Input::Text(_)
        ) {
            self.rest = 0;
            self.tooltip = None;
        }
        let in_title = |w: &Self, y: f32| y < w.title.height();
        let in_ribbon = |w: &Self, y: f32| y >= w.ribbon.top() && y < w.ribbon.bottom();
        if let Input::Key(Key::Letter('s'), Mods { ctrl: true, .. }) = input {
            self.requests.push(Request::Save);
            return false;
        }
        let redraw = match input {
            Input::Resize { width, height } => {
                self.size = (width.max(1.0), height.max(1.0));
                self.menu = None;
                self.relayout(text);
                true
            }
            Input::Scale(scale) => {
                self.scale = scale;
                self.menu = None;
                self.view.handle(Input::Scale(scale));
                self.relayout(text);
                true
            }
            Input::Tick => self.tick(text),
            Input::Maximized(m) => self.title.set_maximized(m),
            Input::Leave => {
                let a = self.title.hover(-1.0, -1.0);
                self.ribbon.clear_pointer() || a
            }
            Input::Move { x, y } => {
                let a = self.title.hover(x, y);
                self.ribbon.hover(x, y) || a
            }
            Input::Press { x, y, .. } if in_title(self, y) => {
                self.doc_press = false;
                self.title.press(x, y)
            }
            Input::Press { x, y, .. } if in_ribbon(self, y) => {
                self.doc_press = false;
                self.ribbon.press(x, y)
            }
            Input::Press { .. } => {
                self.doc_press = true;
                let cleared = self.ribbon.clear_pointer();
                self.view.handle(input) || cleared
            }
            Input::Drag { .. } if !self.doc_press => false,
            Input::Release { x, y } => {
                self.doc_press = false;
                let quick = match self.title.release(x, y) {
                    Some("file.save") => {
                        self.requests.push(Request::Save);
                        true
                    }
                    Some(id) => self.view.run_action(id),
                    None => false,
                };
                let (fired, changed) = self.ribbon.release(x, y);
                let ran = match fired {
                    Some(RibbonHit::Command(id)) => self.view.run_action(id),
                    Some(hit @ (RibbonHit::FontBox | RibbonHit::SizeBox)) => {
                        self.open_menu(hit);
                        true
                    }
                    _ => false,
                };
                if self.ribbon.needs_layout() {
                    self.relayout(text);
                }
                quick || ran || changed || fired.is_some()
            }
            Input::Wheel(_)
            | Input::Text(_)
            | Input::Key(..)
            | Input::Blink
            | Input::Drag { .. } => self.view.handle(input),
        };
        redraw || tip_before != self.tooltip.is_some()
    }

    /// Input while a menu is open; `None` lets the event through to the window.
    fn menu_input(&mut self, input: &Input) -> Option<bool> {
        let menu = self.menu.as_mut()?;
        Some(match *input {
            Input::Move { x, y } | Input::Drag { x, y } => menu.hover(x, y),
            Input::Press { x, y, .. } => {
                if menu.contains(x, y) {
                    menu.press(x, y);
                } else {
                    // A click outside closes the menu and is not passed on.
                    self.menu = None;
                    self.ribbon.clear_pointer();
                }
                true
            }
            Input::Release { x, y } => {
                if let Some(choice) = menu.release(x, y) {
                    self.menu = None;
                    self.choose(choice);
                }
                true
            }
            Input::Wheel(dy) => menu.wheel(dy),
            Input::Key(Key::Escape, _) => {
                self.menu = None;
                true
            }
            Input::Key(Key::Up, _) => {
                menu.step(-1);
                true
            }
            Input::Key(Key::Down, _) => {
                menu.step(1);
                true
            }
            Input::Key(Key::Enter, _) => {
                let choice = menu.current();
                self.menu = None;
                if let Some(c) = choice {
                    self.choose(c);
                }
                true
            }
            Input::Text(_) | Input::Key(..) => true,
            _ => return None,
        })
    }

    fn open_menu(&mut self, hit: RibbonHit) {
        let Some(anchor) = self.ribbon.rect_of(hit) else {
            return;
        };
        let f = self.view.format().clone();
        let items: Vec<MenuItem> = if hit == RibbonHit::FontBox {
            self.view
                .font_list()
                .into_iter()
                .map(|c| MenuItem {
                    checked: f.font.as_deref() == Some(c.name.as_str()),
                    dim: !c.available && c.substitute.is_none(),
                    label: c.name.clone(),
                    choice: Choice::Font(c.name),
                })
                .collect()
        } else {
            SIZES
                .iter()
                .map(|pt| {
                    let half = (pt * 2.0).round() as u32;
                    MenuItem {
                        label: if half & 1 == 0 {
                            (half / 2).to_string()
                        } else {
                            format!("{},5", half / 2)
                        },
                        choice: Choice::Size(half),
                        checked: f.size_half_points == Some(half),
                        dim: false,
                    }
                })
                .collect()
        };
        self.menu = Some(Menu::open(items, anchor, self.size.1, self.scale));
    }

    fn choose(&mut self, choice: Choice) {
        let format = match choice {
            Choice::Font(name) => CharFormat::Font { name },
            Choice::Size(half_points) => CharFormat::Size { half_points },
        };
        self.view.run_command(Command::FormatText { format });
    }

    fn tick(&mut self, text: &dyn TextMeasure) -> bool {
        let Some(RibbonHit::Command(id)) = self.ribbon.hovered() else {
            return false;
        };
        self.rest += 1;
        if self.rest != TOOLTIP_TICKS || self.menu.is_some() {
            return false;
        }
        let Some(anchor) = self.ribbon.rect_of(RibbonHit::Command(id)) else {
            return false;
        };
        let name = self.strings.get(&format!("commands.{id}")).to_owned();
        let label = match commands::shortcut(id) {
            Some(keys) => format!("{name} ({keys})"),
            None => name,
        };
        let style = TextStyle {
            size: (12.0 * self.scale).round(),
            bold: false,
        };
        let (w, _) = text.measure(&label, style);
        self.tooltip = Some((label, w, anchor));
        true
    }

    /// Update page images of the visible pages.
    pub fn prepare(&mut self, store: &mut dyn PageStore) -> Result<(), String> {
        self.view.prepare(store)
    }

    /// Paint the whole window.
    pub fn paint(&self, p: &mut dyn Painter) {
        p.clear(self.theme.bg);
        self.view.paint(p);
        self.title.paint(
            p,
            &self.theme,
            &self.icons,
            &self.strings,
            &self.name,
            self.view.modified(),
            self.view.format(),
        );
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
        if let Some(menu) = &self.menu {
            menu.paint(p, &self.theme);
        } else if let Some((label, tw, anchor)) = &self.tooltip {
            paint_tooltip(p, &self.theme, label, *tw, *anchor, w, self.scale);
        }
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
        let mut w = DocWindow::new(DocView::new(engine).unwrap(), "vi-VN", "a.docx");
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
    #[test]
    fn size_menu_sets_the_size_and_escape_closes() {
        let mut engine = Engine::with_bundled_fonts();
        engine.new_document("a.docx").unwrap();
        let mut w = DocWindow::new(DocView::new(engine).unwrap(), "vi-VN", "a.docx");
        w.handle(Input::Scale(1.0), &Fixed);
        w.handle(
            Input::Resize {
                width: 1400.0,
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
        let [x, y, bw, bh] = w.ribbon.rect_of(RibbonHit::SizeBox).expect("size box");
        let (cx, cy) = (x + bw / 2.0, y + bh / 2.0);
        w.handle(
            Input::Press {
                x: cx,
                y: cy,
                count: 1,
                shift: false,
            },
            &Fixed,
        );
        w.handle(Input::Release { x: cx, y: cy }, &Fixed);
        assert!(w.menu.is_some(), "the size box opens its list");
        w.handle(Input::Key(Key::Escape, Mods::default()), &Fixed);
        assert!(w.menu.is_none(), "Escape closes it");
        w.handle(
            Input::Press {
                x: cx,
                y: cy,
                count: 1,
                shift: false,
            },
            &Fixed,
        );
        w.handle(Input::Release { x: cx, y: cy }, &Fixed);
        // Highlight "14" with the keyboard and choose it.
        let menu = w.menu.as_mut().expect("open again");
        let fourteen = SIZES.iter().position(|s| *s == 14.0).unwrap() as i32;
        menu.step(-100);
        menu.step(fourteen);
        w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
        assert!(w.menu.is_none());
        assert_eq!(w.view().format().size_half_points, Some(28));
    }
}
