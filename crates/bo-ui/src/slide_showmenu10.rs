//! The slide show's right-click menu (round 10 of the slide series), with the reference's
//! entries: Next, Previous, Last Viewed, Black / White Screen, Laser Pointer, Pen,
//! Highlighter, Eraser, Erase All Ink on Slide, Arrow, End Show.
use super::{MenuKind, SlideWindow};
use crate::popup::{Choice, Menu, MenuItem};
use crate::slide_view::show_tools::Pointer;
use crate::Input;

/// Entries: (command, label key, fallback, a separator before it).
const ENTRIES: [(&str, &str, &str, bool); 11] = [
    ("next", "slideR10.show.next", "Next", false),
    ("prev", "slideR10.show.prev", "Previous", false),
    ("last", "slideR10.show.last", "Last Viewed", false),
    ("black", "slideR10.show.black", "Black Screen", true),
    ("white", "slideR10.show.white", "White Screen", false),
    ("laser", "slideR10.show.laser", "Laser Pointer", true),
    ("pen", "slideR10.show.pen", "Pen", false),
    (
        "highlighter",
        "slideR10.show.highlighter",
        "Highlighter",
        false,
    ),
    ("eraser", "slideR10.show.eraser", "Eraser", false),
    (
        "eraseAll",
        "slideR10.show.eraseAll",
        "Erase All Ink on Slide",
        false,
    ),
    ("end", "slideR10.show.end", "End Show", true),
];

impl SlideWindow {
    /// Input while the show runs: the open menu first, a right click opens it.
    pub(super) fn show_menu_input(&mut self, input: &Input) -> Option<bool> {
        if self.menu.is_some() {
            return self.menu_input(input).or(Some(false));
        }
        if let Input::Context { x, y } = *input {
            return Some(self.open_show_menu(x, y));
        }
        None
    }

    /// Open the menu at the pointer.
    pub(super) fn open_show_menu(&mut self, x: f32, y: f32) -> bool {
        let tools = self.view.show_tools();
        let pointer = tools.pointer;
        let has_last = tools.last_viewed.is_some();
        let on = |id: &str| match id {
            "laser" => pointer == Pointer::Laser,
            "pen" => pointer == Pointer::Pen,
            "highlighter" => pointer == Pointer::Highlighter,
            "eraser" => pointer == Pointer::Eraser,
            _ => false,
        };
        let mut separators = Vec::new();
        let items: Vec<MenuItem> = ENTRIES
            .iter()
            .enumerate()
            .map(|(i, (id, k, f, sep))| {
                if *sep {
                    separators.push(i);
                }
                MenuItem {
                    label: self.t(k, f),
                    choice: Choice::Command(format!("shw:{id}")),
                    checked: on(id),
                    dim: *id == "last" && !has_last,
                }
            })
            .collect();
        let icons = vec![None; items.len()];
        self.menu_kind = MenuKind::Duration;
        self.menu = Some(
            Menu::open(items, [x, y, 1.0, 1.0], self.size, self.scale)
                .with_decor(icons, separators),
        );
        true
    }
}
