//! The slide show's right-click menu (round 10 of the slide series), laid out as the
//! reference's: Next, Previous, Last Viewed | See All Slides, Zoom In | Custom Show ›,
//! Show Presenter View | Screen ›, Pointer Options › | Pause | End Show. Screen › holds
//! Black / White Screen; Pointer Options › Laser Pointer, Pen, Highlighter, Eraser, Erase
//! All Ink on Slide and Arrow. A submenu opens when its row is pointed at (or → / Enter),
//! beside the row, and closes with ← / Esc or when another row is pointed at.
use super::{MenuKind, SlideWindow};
use crate::popup::{Choice, Menu, MenuItem, MenuLook};
use crate::slide_view::show_tools::Pointer;
use crate::{Input, Key, Painter};
use bo_render::{PathSeg, Rgba};

/// Main entries: (command, label key, fallback, a separator before it).
const MAIN: [(&str, &str, &str, bool); 12] = [
    ("next", "slideR10.show.next", "Next", false),
    ("prev", "slideR10.show.prev", "Previous", false),
    ("last", "slideR10.show.last", "Last Viewed", false),
    ("all", "slideR10.show.all", "See All Slides", true),
    ("magnify", "slideR10.show.magnify", "Magnify Slide", false),
    ("sub:custom", "slideR10.show.custom", "Custom Show", true),
    (
        "presenter",
        "slideR10.show.presenter",
        "Show Presenter View",
        true,
    ),
    ("sub:screen", "slideR10.show.screen", "Screen", true),
    (
        "sub:pointer",
        "slideR10.show.pointer",
        "Pointer Options",
        false,
    ),
    ("help", "slideR10.show.help", "Help", true),
    ("pause", "slideR10.show.pause", "Pause", true),
    ("end", "slideR10.show.end", "End Show", true),
];

/// Submenu entries: (command, label key, fallback, a separator before it).
const SCREEN: [(&str, &str, &str, bool); 2] = [
    ("black", "slideR10.show.black", "Black Screen", false),
    ("white", "slideR10.show.white", "White Screen", false),
];
const POINTER: [(&str, &str, &str, bool); 6] = [
    ("laser", "slideR10.show.laser", "Laser Pointer", false),
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
        true,
    ),
    ("arrow", "slideR10.show.arrow", "Arrow", true),
];

/// Access key of each entry (the reference's underlined letters in English).
const ACCESS: [(&str, char); 20] = [
    ("next", 'N'),
    ("prev", 'P'),
    ("last", 'V'),
    ("all", 'A'),
    ("magnify", 'M'),
    ("custom", 'w'),
    ("presenter", 'r'),
    ("screen", 'c'),
    ("pointer", 'o'),
    ("help", 'H'),
    ("pause", 's'),
    ("end", 'E'),
    ("black", 'B'),
    ("white", 'W'),
    ("laser", 'L'),
    ("pen", 'P'),
    ("highlighter", 'H'),
    ("eraser", 'r'),
    ("eraseAll", 'E'),
    ("arrow", 'A'),
];

fn access_letter(id: &str) -> Option<char> {
    let id = id.trim_start_matches("sub:");
    ACCESS.iter().find(|(k, _)| *k == id).map(|(_, c)| *c)
}

/// `label` with `&` before its access letter (first exact match, else any case; none in
/// labels of other languages without that letter). A literal `&` is doubled.
fn access(label: &str, id: &str) -> String {
    let plain = label.replace('&', "&&");
    let Some(c) = access_letter(id) else {
        return plain;
    };
    let at = plain.find(c).or_else(|| {
        plain
            .char_indices()
            .find(|(_, ch)| ch.eq_ignore_ascii_case(&c))
            .map(|(i, _)| i)
    });
    match at {
        Some(i) => format!("{}&{}", &plain[..i], &plain[i..]),
        None => plain,
    }
}

/// The submenu arrow's colour (dimmed rows: lighter).
const ARROW: Rgba = Rgba(0x26, 0x26, 0x26, 255);
const ARROW_DIM: Rgba = Rgba(0xa0, 0xa0, 0xa0, 255);

impl SlideWindow {
    /// Input while the show runs: the help list, the open menu, then a right click opens it
    /// (or leaves Magnify Slide).
    pub(super) fn show_menu_input(
        &mut self,
        input: &Input,
        text: &dyn crate::TextMeasure,
    ) -> Option<bool> {
        if self.show_help {
            return Some(self.help_input(input));
        }
        // Ctrl+T: leave full screen so the taskbar shows; the next key or click returns.
        if self.show_windowed
            && matches!(input, Input::Key(..) | Input::Text(_) | Input::Press { .. })
        {
            self.show_windowed = false;
            self.requests.push(crate::Request::FullScreen(true));
            if matches!(input, Input::Press { .. }) {
                return Some(true);
            }
        }
        if let Input::Key(Key::Letter('t'), m) = input {
            if m.ctrl && self.menu.is_none() {
                self.show_windowed = true;
                self.requests.push(crate::Request::FullScreen(false));
                return Some(true);
            }
        }
        if self.menu.is_none() && matches!(input, Input::Key(Key::F(1), _)) {
            return Some(self.open_show_help());
        }
        if self.menu.is_none()
            && matches!(input, Input::Context { .. })
            && self.view.cancel_magnify()
        {
            return Some(true);
        }
        if self.menu.is_some() {
            return Some(self.show_menu_event(input, text));
        }
        if let Input::Context { x, y } = *input {
            return Some(self.open_show_menu(x, y, text));
        }
        None
    }

    fn show_menu_event(&mut self, input: &Input, text: &dyn crate::TextMeasure) -> bool {
        // The open submenu takes the pointer over it and the keys.
        if let Some((_, sub)) = self.show_sub.as_mut() {
            match *input {
                Input::Move { x, y } | Input::Drag { x, y } if sub.contains(x, y) => {
                    return sub.hover(x, y);
                }
                Input::Press { x, y, .. } if sub.contains(x, y) => {
                    sub.press(x, y);
                    return true;
                }
                Input::Release { x, y } if sub.contains(x, y) => {
                    if let Some(c) = sub.release(x, y) {
                        self.close_show_menu();
                        return self.choose(c);
                    }
                    return true;
                }
                Input::Key(Key::Up, _) => {
                    sub.step(-1);
                    return true;
                }
                Input::Key(Key::Down, _) => {
                    sub.step(1);
                    return true;
                }
                Input::Key(Key::Enter | Key::Right, _) => {
                    let c = sub.current();
                    if let Some(c) = c {
                        self.close_show_menu();
                        return self.choose(c);
                    }
                    return false;
                }
                Input::Key(Key::Left | Key::Escape, _) => {
                    self.show_sub = None;
                    return true;
                }
                Input::Text(ref t) => {
                    let c = t.chars().next().and_then(|ch| sub.access_choice(ch));
                    if let Some(c) = c {
                        self.close_show_menu();
                        return self.choose(c);
                    }
                    return false;
                }
                _ => {}
            }
        }
        let Some(menu) = self.menu.as_mut() else {
            return false;
        };
        match *input {
            Input::Move { x, y } | Input::Drag { x, y } => {
                let changed = menu.hover(x, y);
                if changed || (self.show_sub.is_none() && menu.contains(x, y)) {
                    return self.sync_show_sub(text) || changed;
                }
                false
            }
            Input::Press { x, y, .. } => {
                if menu.contains(x, y) {
                    menu.press(x, y);
                } else {
                    self.close_show_menu();
                }
                true
            }
            Input::Release { x, y } => match menu.release(x, y) {
                Some(Choice::Command(c)) if c.starts_with("shwsub:") => self.sync_show_sub(text),
                Some(c) => {
                    self.close_show_menu();
                    self.choose(c)
                }
                None => true,
            },
            Input::Key(Key::Escape, _) => {
                self.close_show_menu();
                true
            }
            Input::Key(Key::Up, _) => {
                menu.step(-1);
                self.show_sub = None;
                true
            }
            Input::Key(Key::Down, _) => {
                menu.step(1);
                self.show_sub = None;
                true
            }
            Input::Key(Key::Right, _) => self.sync_show_sub(text),
            // Access keys: a submenu row opens its submenu, others run.
            Input::Text(ref t) => {
                let Some(c) = t.chars().next().and_then(|ch| menu.access_choice(ch)) else {
                    return false;
                };
                if matches!(&c, Choice::Command(s) if s.starts_with("shwsub:")) {
                    if let Some(i) = menu.items().iter().position(|it| it.choice == c) {
                        menu.step(0);
                        menu.step(i32::try_from(i).unwrap_or(0));
                    }
                    return self.sync_show_sub(text);
                }
                self.close_show_menu();
                self.choose(c)
            }
            Input::Key(Key::Enter, _) => match menu.current() {
                Some(Choice::Command(c)) if c.starts_with("shwsub:") => self.sync_show_sub(text),
                Some(c) => {
                    self.close_show_menu();
                    self.choose(c)
                }
                None => false,
            },
            Input::Wheel(_) => false,
            _ => true,
        }
    }

    fn close_show_menu(&mut self) {
        self.menu = None;
        self.show_sub = None;
    }

    /// Open the submenu of the highlighted main row (or close it when the row has none).
    fn sync_show_sub(&mut self, text: &dyn crate::TextMeasure) -> bool {
        let Some(menu) = self.menu.as_ref() else {
            return false;
        };
        let Some(row) = menu.hovered() else {
            return false;
        };
        if self.show_sub.as_ref().is_some_and(|(r, _)| *r == row) {
            return false;
        }
        let item = &menu.items()[row];
        let id = match &item.choice {
            Choice::Command(c) => c.strip_prefix("shwsub:").map(str::to_owned),
            _ => None,
        };
        let had = self.show_sub.take().is_some();
        let Some(id) = id.filter(|_| !item.dim) else {
            return had;
        };
        let rect = menu.row_rect(row);
        let tools = self.view.show_tools();
        let pointer = tools.pointer;
        let on = |c: &str| match c {
            "laser" => pointer == Pointer::Laser,
            "pen" => pointer == Pointer::Pen,
            "highlighter" => pointer == Pointer::Highlighter,
            "eraser" => pointer == Pointer::Eraser,
            "arrow" => pointer == Pointer::Arrow,
            _ => false,
        };
        let shown = self.view.show_slide();
        let has_ink = shown.is_some_and(|i| tools.strokes.get(&i).is_some_and(|s| !s.is_empty()));
        let entries: &[(&str, &str, &str, bool)] = match id.as_str() {
            "screen" => &SCREEN,
            "pointer" => &POINTER,
            _ => return had,
        };
        let mut separators = Vec::new();
        let items: Vec<MenuItem> = entries
            .iter()
            .enumerate()
            .map(|(i, (c, k, f, sep))| {
                if *sep {
                    separators.push(i);
                }
                MenuItem {
                    label: access(&self.t(k, f), c),
                    choice: Choice::Command(format!("shw:{c}")),
                    checked: on(c),
                    dim: *c == "eraseAll" && !has_ink,
                }
            })
            .collect();
        let icons = vec![None; items.len()];
        // Beside the row, its top level with the row (the list opens downwards).
        let pad = (4.0 * self.scale).round();
        let anchor = [rect[0] + rect[2] - pad, rect[1] - pad, 1.0, 0.0];
        let mut sub = self.show_menu_box(items, anchor, icons, separators, text);
        if !sub.contains(anchor[0] + 2.0, rect[1] + rect[3] / 2.0) {
            // No room on the right: open on the left of the menu.
            let w = sub.row_rect(0)[2];
            let left = (rect[0] - w + pad).max(0.0);
            let items = sub.items().to_vec();
            let icons = vec![None; items.len()];
            let seps = entries
                .iter()
                .enumerate()
                .filter(|(_, e)| e.3)
                .map(|(i, _)| i)
                .collect();
            sub = self.show_menu_box(items, [left, rect[1] - pad, 1.0, 0.0], icons, seps, text);
        }
        self.show_sub = Some((row, sub));
        true
    }

    /// Open the menu at the pointer.
    pub(super) fn open_show_menu(&mut self, x: f32, y: f32, text: &dyn crate::TextMeasure) -> bool {
        let tools = self.view.show_tools();
        let has_last = tools.last_viewed.is_some();
        let (at_start, presenter_ok, presenter_on, paused) = self.view.show_menu_state();
        let can_pause = self.view.show_can_pause();
        let mut separators = Vec::new();
        let items: Vec<MenuItem> = MAIN
            .iter()
            .enumerate()
            .map(|(i, (id, k, f, sep))| {
                if *sep {
                    separators.push(i);
                }
                let (label, cmd) = match *id {
                    "presenter" if presenter_on => (
                        self.t("slideR10.show.hidePresenter", "Hide Presenter View"),
                        "shw:presenter".to_owned(),
                    ),
                    "pause" if paused => (
                        self.t("slideR10.show.resume", "Resume"),
                        "shw:resume".to_owned(),
                    ),
                    _ => (
                        access(&self.t(k, f), id),
                        match id.strip_prefix("sub:") {
                            Some(s) => format!("shwsub:{s}"),
                            None => format!("shw:{id}"),
                        },
                    ),
                };
                let dim = match *id {
                    "prev" => at_start,
                    "last" => !has_last,
                    // No custom shows to pick from.
                    "sub:custom" => true,
                    "presenter" => !presenter_ok,
                    "pause" => !paused && !can_pause,
                    _ => false,
                };
                MenuItem {
                    label,
                    choice: Choice::Command(cmd),
                    checked: false,
                    dim,
                }
            })
            .collect();
        let icons = vec![None; items.len()];
        self.menu_kind = MenuKind::Duration;
        self.show_sub = None;
        // The menu's top left corner at the pointer (reference).
        let gap = (2.0 * self.scale).round();
        let menu = self.show_menu_box(items, [x, y - gap - 1.0, 1.0, 1.0], icons, separators, text);
        self.menu = Some(menu);
        true
    }

    /// A menu of the show with the reference's metrics at 125 %: rows 36 px, text 47 px in,
    /// 246 px wide at least, 3 px more around separators.
    fn show_menu_box(
        &self,
        items: Vec<MenuItem>,
        anchor: [f32; 4],
        icons: Vec<Option<&'static str>>,
        separators: Vec<usize>,
        text: &dyn crate::TextMeasure,
    ) -> Menu {
        let look = MenuLook {
            item_h: 28.8,
            text: 13.8,
            text_x: (37.6, 37.6),
            pad: 3.2,
            min_w: 196.8,
            char_w: 7.0,
            ..MenuLook::default()
        };
        Menu::open_with(items, anchor, self.size, self.scale, look)
            .with_decor(icons, separators)
            .with_keys(Vec::new(), 0.0, 0.0, 2.4, self.size)
            .with_access_keys(text, false)
    }

    /// Right-click menu › Help: the show's keys (own list), until any key or click.
    pub(super) fn open_show_help(&mut self) -> bool {
        self.show_help = true;
        self.help_tab = 0;
        self.help_hover = None;
        true
    }

    /// Paint the show menu, its submenu arrows and the open submenu.
    pub(super) fn paint_show_menu(&self, p: &mut dyn Painter) {
        if self.show_help {
            self.paint_show_help(p);
        }
        let Some(menu) = &self.menu else { return };
        menu.paint(p, &self.theme, &self.icons);
        let s = self.scale;
        for (i, item) in menu.items().iter().enumerate() {
            let Choice::Command(c) = &item.choice else {
                continue;
            };
            if !c.starts_with("shwsub:") {
                continue;
            }
            // A small solid triangle pointing right, 14 px in from the menu's edge.
            let [x, y, w, h] = menu.row_rect(i);
            let (cx, cy) = ((x + w - 16.0 * s).round(), (y + h / 2.0).round());
            let a = (4.4 * s).round();
            let tri = vec![
                PathSeg::M(cx - a / 2.0, cy - a),
                PathSeg::L(cx + a / 2.0, cy),
                PathSeg::L(cx - a / 2.0, cy + a),
                PathSeg::Z,
            ];
            p.path(&tri, Some(if item.dim { ARROW_DIM } else { ARROW }), None);
        }
        if let Some((_, sub)) = &self.show_sub {
            sub.paint(p, &self.theme, &self.icons);
        }
    }
}
