//! Drop-down menus (font and size lists) and tooltips, drawn over the window.
use crate::theme::Theme;
use crate::{Painter, TextStyle};
use bo_render::{PathSeg, Rgba};

/// What choosing a menu item does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Choice {
    /// Set the font family.
    Font(String),
    /// Set the font size in half-points.
    Size(u32),
    /// Apply the paragraph style with this id.
    Style(String),
    /// Run an interface command (`layout.margins.narrow`, `edit.copy`, …).
    Command(String),
}

/// One menu row.
#[derive(Debug, Clone)]
pub struct MenuItem {
    /// Text shown.
    pub label: String,
    /// What it does.
    pub choice: Choice,
    /// Shown as the current value.
    pub checked: bool,
    /// Dimmed (e.g. a font that is not installed).
    pub dim: bool,
}

/// Sizes and colours of a menu at 100 % (multiplied by the display scale), so an app can match
/// its reference's menus without changing the others.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MenuLook {
    /// Row height.
    pub item_h: f32,
    /// Label text size.
    pub text: f32,
    /// Label left edge from the menu's outer edge (without / with an icon column).
    pub text_x: (f32, f32),
    /// Icon left edge.
    pub icon_x: f32,
    /// Icon size.
    pub icon: f32,
    /// Space above the first and below the last row.
    pub pad: f32,
    /// Narrowest menu.
    pub min_w: f32,
    /// Width estimate per label character at 13 px text.
    pub char_w: f32,
    /// Background (`None`: the theme's surface).
    pub fill: Option<Rgba>,
    /// Frame (`None`: the theme's border).
    pub frame: Option<Rgba>,
}

impl Default for MenuLook {
    fn default() -> Self {
        Self {
            item_h: 26.0,
            text: 13.0,
            text_x: (24.0, 34.0),
            icon_x: 6.0,
            icon: 16.0,
            pad: 4.0,
            min_w: 200.0,
            char_w: 7.5 * 1.15,
            fill: None,
            frame: None,
        }
    }
}

/// An open drop-down list anchored under a control.
pub struct Menu {
    items: Vec<MenuItem>,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    item_h: f32,
    scroll: f32,
    hover: Option<usize>,
    pressed: bool,
    scale: f32,
    /// Icon of each item (context menus), drawn in a left column.
    icons: Vec<Option<&'static str>>,
    /// Items preceded by a separator line.
    separators: Vec<usize>,
    look: MenuLook,
    /// An app's own icon drawing (`painter, name, x, y, size, dim`) in place of the shared set.
    icon_fn: Option<IconFn>,
    /// Shortcut of each item and its measured width, drawn right-aligned `key_right` device
    /// pixels from the menu's right edge (context menus of BlueReader); empty: none.
    keys: Vec<(String, f32)>,
    key_right: f32,
    /// Extra device pixels a separator adds above its item (0: the line sits on the row).
    sep_gap: f32,
    /// A faint shadow on every side (BlueEditor round 9) in place of the default one.
    soft_shadow: bool,
    /// Access keys ([`Menu::with_access_keys`]): per item the key (lower case) and its
    /// underline (offset from the label's left edge, width; device pixels); empty: none.
    access: Vec<Option<(char, f32, f32)>>,
    /// Draw the access keys' underlines.
    cues: bool,
}

/// Windows' "Underline access keys" setting (`SPI_GETKEYBOARDCUES`), given by the host.
static KEYBOARD_CUES: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Host: whether Windows always underlines access keys (`SPI_GETKEYBOARDCUES`).
pub fn set_keyboard_cues(on: bool) {
    KEYBOARD_CUES.store(on, std::sync::atomic::Ordering::Relaxed);
}

/// Whether access keys are underlined without the keyboard (for popups other than [`Menu`]).
pub(crate) fn keyboard_cues() -> bool {
    KEYBOARD_CUES.load(std::sync::atomic::Ordering::Relaxed)
}

/// A label's access key marked with `&` (`"&Bottom"`; `"&&"` is a literal `&`): the label
/// without the markers, the key's byte offset in it.
pub fn split_access_key(label: &str) -> (String, Option<usize>) {
    let mut out = String::with_capacity(label.len());
    let mut key = None;
    let mut chars = label.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '&' {
            match chars.peek() {
                Some('&') => {
                    chars.next();
                    out.push('&');
                }
                Some(_) if key.is_none() => key = Some(out.len()),
                _ => {}
            }
            continue;
        }
        out.push(c);
    }
    (out, key)
}

/// Draws icon `name` into a `size`-pixel square at (`x`, `y`), greyed when `dim`.
pub type IconFn = fn(&mut dyn Painter, &str, f32, f32, f32, bool);

impl Menu {
    /// Open near the control at (`x`, `y`, `w`, `h`), within the window's viewport.
    pub fn open(items: Vec<MenuItem>, anchor: [f32; 4], viewport: (f32, f32), scale: f32) -> Self {
        Self::open_with(items, anchor, viewport, scale, MenuLook::default())
    }

    /// [`Menu::open`] with an app's own sizes and colours.
    pub fn open_with(
        items: Vec<MenuItem>,
        anchor: [f32; 4],
        viewport: (f32, f32),
        scale: f32,
        look: MenuLook,
    ) -> Self {
        let [x, y, w, h] = anchor;
        let item_h = (look.item_h * scale).round();
        let gap = (2.0 * scale).round();
        let margin = (look.pad * scale)
            .round()
            .min(viewport.0.max(0.0) / 2.0)
            .min(viewport.1.max(0.0) / 2.0);
        // Wide enough for the longest label (an estimate of 7.5 px per character at 13 px
        // plus the check column and padding; the interface text may be scaled up).
        let longest = items
            .iter()
            .map(|i| i.label.chars().count())
            .max()
            .unwrap_or(0) as f32;
        let label_w = (longest * look.char_w * look.text / 13.0 + look.text_x.0 + 16.0) * scale;
        let menu_width = w
            .max((look.min_w * scale).round())
            .max(label_w.round())
            .min((viewport.0 - 2.0 * margin).max(0.0));
        let left = x.clamp(margin, (viewport.0 - margin - menu_width).max(margin));
        let below_top = (y + h + gap).clamp(margin, (viewport.1 - margin).max(margin));
        let above_bottom = (y - gap).clamp(margin, (viewport.1 - margin).max(margin));
        let below = (viewport.1 - margin - below_top).max(0.0);
        let above = (above_bottom - margin).max(0.0);
        let full = items.len() as f32 * item_h + (2.0 * look.pad * scale).round();
        let use_above = below < full && above > below;
        let height = full.min(if use_above { above } else { below });
        let top = if use_above {
            above_bottom - height
        } else {
            below_top
        };
        // A ticked row that cannot be chosen is not the current one (BlueEditor round 11).
        let current = items.iter().position(|i| i.checked && !i.dim);
        let mut m = Self {
            items,
            x: left,
            y: top,
            w: menu_width,
            h: height.round(),
            item_h,
            scroll: 0.0,
            hover: current,
            pressed: false,
            scale,
            icons: Vec::new(),
            separators: Vec::new(),
            look,
            icon_fn: None,
            keys: Vec::new(),
            key_right: 0.0,
            sep_gap: 0.0,
            soft_shadow: false,
            access: Vec::new(),
            cues: false,
        };
        // Show the current value in the middle of the list.
        if let Some(i) = current {
            m.scroll = (i as f32 * item_h - m.h / 2.0).max(0.0);
            m.clamp();
        }
        m
    }

    /// The entries.
    pub(crate) fn items(&self) -> &[MenuItem] {
        &self.items
    }

    /// Icons per item and separators before items, as in a desktop word processor's
    /// context menu.
    #[must_use]
    pub fn with_decor(mut self, icons: Vec<Option<&'static str>>, separators: Vec<usize>) -> Self {
        self.icons = icons;
        self.separators = separators;
        self
    }

    /// Whether any row has an icon.
    pub fn has_icons(&self) -> bool {
        self.icons.iter().any(Option::is_some)
    }

    /// Access keys (opt-in): labels mark their key with `&` (`"&Bottom of Group"`); the
    /// markers are taken out, the letter chooses the item ([`Menu::access_choice`]) and is
    /// underlined when the menu was opened from the keyboard or Windows always underlines
    /// access keys ([`set_keyboard_cues`]). `text` measures the underline's place.
    #[must_use]
    pub fn with_access_keys(mut self, text: &dyn crate::TextMeasure, from_keyboard: bool) -> Self {
        let style = TextStyle {
            size: (self.look.text * self.scale).round(),
            bold: false,
        };
        self.access = self
            .items
            .iter_mut()
            .map(|item| {
                let (label, at) = split_access_key(&item.label);
                item.label = label;
                let at = at?;
                let c = item.label[at..].chars().next()?;
                let x = text.measure(&item.label[..at], style).0;
                let w = text.measure(&c.to_string(), style).0;
                Some((c.to_lowercase().next().unwrap_or(c), x, w))
            })
            .collect();
        self.cues = from_keyboard || KEYBOARD_CUES.load(std::sync::atomic::Ordering::Relaxed);
        self
    }

    /// Whether the menu has access keys.
    pub fn has_access_keys(&self) -> bool {
        self.access.iter().any(Option::is_some)
    }

    /// The item whose access key is `c` (case-insensitive; dimmed items are skipped).
    pub fn access_choice(&self, c: char) -> Option<Choice> {
        let c = c.to_lowercase().next().unwrap_or(c);
        self.access
            .iter()
            .zip(&self.items)
            .find(|(a, item)| a.is_some_and(|a| a.0 == c) && !item.dim)
            .map(|(_, item)| item.choice.clone())
    }

    /// A faint shadow spread on every side, as Windows 11 menus (BlueEditor round 9).
    #[must_use]
    pub fn with_soft_shadow(mut self) -> Self {
        self.soft_shadow = true;
        self
    }

    /// Draw the icons of [`Menu::with_decor`] with an app's own icon set.
    #[must_use]
    pub fn with_icon_fn(mut self, f: IconFn) -> Self {
        self.icon_fn = Some(f);
        self
    }

    /// Shortcuts (text, measured width in device pixels) right-aligned `right` logical pixels
    /// from the menu's right edge, and separators `sep_gap` logical pixels tall (as a desktop
    /// PDF reader's context menus). `content_w` is the widest row's label, gap and shortcut
    /// (device pixels); the menu grows to fit it and stays inside `viewport`.
    #[must_use]
    pub fn with_keys(
        mut self,
        keys: Vec<(String, f32)>,
        content_w: f32,
        right: f32,
        sep_gap: f32,
        viewport: (f32, f32),
    ) -> Self {
        let s = self.scale;
        let text_x = if self.icons.is_empty() {
            self.look.text_x.0
        } else {
            self.look.text_x.1
        };
        self.key_right = (right * s).round();
        self.sep_gap = (sep_gap * s).round();
        let need = (text_x * s + content_w + self.key_right).round();
        self.w = self.w.max(need).min(viewport.0.max(0.0));
        self.keys = keys;
        let gaps = self
            .separators
            .iter()
            .filter(|&&i| i > 0 && i < self.items.len())
            .count() as f32;
        self.h = (self.h + gaps * self.sep_gap).min(viewport.1.max(0.0));
        self.x = self.x.min((viewport.0 - self.w).max(0.0));
        self.y = self.y.min((viewport.1 - self.h).max(0.0));
        self
    }

    /// Top of row `i` from the first row (separators may add space).
    fn row_top(&self, i: usize) -> f32 {
        let gaps = if self.sep_gap > 0.0 {
            self.separators.iter().filter(|&&k| k > 0 && k <= i).count() as f32
        } else {
            0.0
        };
        i as f32 * self.item_h + gaps * self.sep_gap
    }

    fn pad(&self) -> f32 {
        (self.look.pad * self.scale).round()
    }

    fn clamp(&mut self) {
        let max = (self.items.len() as f32 * self.item_h + 2.0 * self.pad() - self.h).max(0.0);
        self.scroll = self.scroll.clamp(0.0, max).round();
    }

    /// Rectangle of row `i` in window pixels (cascading submenus open beside it).
    pub fn row_rect(&self, i: usize) -> [f32; 4] {
        [
            self.x,
            self.y + self.pad() + self.row_top(i) - self.scroll,
            self.w,
            self.item_h,
        ]
    }

    /// The highlighted row.
    pub fn hovered(&self) -> Option<usize> {
        self.hover
    }

    /// Whether a window position is over the menu.
    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x && x < self.x + self.w && y >= self.y && y < self.y + self.h
    }

    fn index_at(&self, x: f32, y: f32) -> Option<usize> {
        if !self.contains(x, y) {
            return None;
        }
        if self.sep_gap > 0.0 {
            let dy = y - self.y - self.pad() + self.scroll;
            return (0..self.items.len())
                .find(|&i| dy >= self.row_top(i) && dy < self.row_top(i) + self.item_h);
        }
        let i = ((y - self.y - self.pad() + self.scroll) / self.item_h).floor();
        (i >= 0.0 && (i as usize) < self.items.len()).then_some(i as usize)
    }

    /// Pointer moved: `true` when the highlighted row changed.
    pub fn hover(&mut self, x: f32, y: f32) -> bool {
        let h = self.index_at(x, y);
        if h.is_none() {
            return false;
        }
        std::mem::replace(&mut self.hover, h) != h
    }

    /// Button pressed over the menu.
    pub fn press(&mut self, x: f32, y: f32) {
        self.pressed = self.contains(x, y);
        self.hover(x, y);
    }

    /// Button released: the chosen item, if released over one after pressing in the menu.
    pub fn release(&mut self, x: f32, y: f32) -> Option<Choice> {
        let pressed = std::mem::take(&mut self.pressed);
        let i = self.index_at(x, y)?;
        pressed.then(|| self.items[i].choice.clone())
    }

    /// Scroll by `dy` device pixels.
    pub fn wheel(&mut self, dy: f32) -> bool {
        let before = self.scroll;
        self.scroll += dy;
        self.clamp();
        before != self.scroll
    }

    /// Move the highlight by `delta` rows (keyboard), keeping it in view.
    pub fn step(&mut self, delta: i32) {
        let n = self.items.len() as i32;
        if n == 0 {
            return;
        }
        let i = self.hover.map_or(0, |i| i as i32 + delta).clamp(0, n - 1) as usize;
        self.hover = Some(i);
        let top = i as f32 * self.item_h;
        let inner = self.h - 2.0 * self.pad();
        if top < self.scroll {
            self.scroll = top;
        } else if top + self.item_h > self.scroll + inner {
            self.scroll = top + self.item_h - inner;
        }
        self.clamp();
    }

    /// The highlighted item's choice (Enter).
    pub fn current(&self) -> Option<Choice> {
        self.hover.map(|i| self.items[i].choice.clone())
    }

    /// Paint the menu with a soft shadow.
    pub fn paint(&self, p: &mut dyn Painter, t: &Theme, icons: &crate::icons::Icons) {
        let s = self.scale;
        if self.soft_shadow {
            for (k, a) in [(4.0, 4u8), (3.0, 5), (2.0, 7), (1.0, 9)] {
                let o = (k * s).round();
                p.fill_rect(
                    self.x - o,
                    self.y - o + s.round(),
                    self.w + 2.0 * o,
                    self.h + 2.0 * o,
                    Rgba(0, 0, 0, a),
                );
            }
        } else {
            for (k, a) in [(3.0, 18u8), (2.0, 26), (1.0, 34)] {
                let o = (k * s).round();
                p.fill_rect(
                    self.x - o + 2.0 * s,
                    self.y - o + 3.0 * s,
                    self.w + 2.0 * o,
                    self.h + 2.0 * o,
                    Rgba(0, 0, 0, a),
                );
            }
        }
        let look = &self.look;
        p.fill_rect(
            self.x,
            self.y,
            self.w,
            self.h,
            look.frame.unwrap_or(t.border),
        );
        p.fill_rect(
            self.x + 1.0,
            self.y + 1.0,
            self.w - 2.0,
            self.h - 2.0,
            look.fill.unwrap_or(t.surface),
        );
        p.push_clip(self.x + 1.0, self.y + 1.0, self.w - 2.0, self.h - 2.0);
        let style = TextStyle {
            size: (look.text * s).round(),
            bold: false,
        };
        let line = style.size * 1.33;
        let (first, last) = if self.sep_gap > 0.0 {
            (0, self.items.len())
        } else {
            (
                (self.scroll / self.item_h).floor() as usize,
                ((self.scroll + self.h) / self.item_h).ceil() as usize,
            )
        };
        for i in first..last.min(self.items.len()) {
            let item = &self.items[i];
            let y = self.y + self.pad() + self.row_top(i) - self.scroll;
            if self.hover == Some(i) {
                p.fill_rect(
                    self.x + self.pad(),
                    y,
                    self.w - 2.0 * self.pad(),
                    self.item_h,
                    t.hover,
                );
            }
            if item.checked && item.dim {
                // Ticked but unavailable (the reference PDF editor's Unrestricted Access): a
                // grey check mark, no band.
                let k = (4.0 * s).round();
                let cx = self.x + self.pad() + (10.0 * s).round();
                let cy = y.round() + self.item_h / 2.0;
                p.path(
                    &[
                        PathSeg::M(cx - k, cy),
                        PathSeg::L(cx - k / 3.0, cy + k * 0.7),
                        PathSeg::L(cx + k * 1.2, cy - k),
                    ],
                    None,
                    Some((Rgba(0xA0, 0xA0, 0xA0, 255), (1.6 * s).max(1.5))),
                );
            } else if item.checked {
                // The current value: grey fill, darker frame and a check mark, as a desktop
                // word processor marks it (198 / 128).
                let (x0, w0) = (self.x + self.pad(), self.w - 2.0 * self.pad());
                let (yy, hh) = (y.round(), self.item_h.round());
                p.fill_rect(x0, yy, w0, hh, Rgba(0x80, 0x80, 0x80, 255));
                p.fill_rect(
                    x0 + 1.0,
                    yy + 1.0,
                    w0 - 2.0,
                    hh - 2.0,
                    Rgba(0xc6, 0xc6, 0xc6, 255),
                );
                let k = (4.0 * s).round();
                let (cx, cy) = (x0 + (10.0 * s).round(), yy + hh / 2.0);
                p.path(
                    &[
                        PathSeg::M(cx - k, cy),
                        PathSeg::L(cx - k / 3.0, cy + k * 0.7),
                        PathSeg::L(cx + k * 1.2, cy - k),
                    ],
                    None,
                    Some((Rgba(0x1e, 0x6e, 0x3e, 255), (1.6 * s).max(1.5))),
                );
            }
            if self.separators.contains(&i) && i > 0 {
                p.fill_rect(
                    self.x + (28.0 * s).round(),
                    (y - (self.sep_gap / 2.0).ceil()).round(),
                    self.w - (32.0 * s).round(),
                    1.0,
                    Rgba(0xe1, 0xe1, 0xe1, 255),
                );
            }
            if let Some((k, kw)) = self.keys.get(i).filter(|k| !k.0.is_empty()) {
                p.text(
                    k,
                    (self.x + self.w - self.key_right - kw).round(),
                    y + (self.item_h - line) / 2.0,
                    style,
                    if item.dim { t.text_muted } else { t.text },
                );
            }
            if let Some(Some(name)) = self.icons.get(i) {
                let size = (look.icon * s).round();
                let (ix, iy) = (
                    self.x + (look.icon_x * s).round(),
                    (y + (self.item_h - size) / 2.0).round(),
                );
                if let Some(f) = self.icon_fn {
                    f(p, name, ix, iy, size, item.dim);
                } else {
                    icons.draw(
                        p,
                        name,
                        ix,
                        iy,
                        size,
                        crate::icons::IconColors {
                            ink: t.text,
                            accent: t.accent,
                            on: t.accent_on,
                            tinted: !item.dim,
                        },
                    );
                }
            }
            let color = if item.dim { t.text_muted } else { t.text };
            let tx = self.x
                + if self.icons.is_empty() {
                    (look.text_x.0 * s).round()
                } else {
                    (look.text_x.1 * s).round()
                };
            let ty = y + (self.item_h - line) / 2.0;
            p.text(&item.label, tx, ty, style, color);
            if let Some(Some((_, ux, uw))) = self.access.get(i).filter(|_| self.cues) {
                // The access key's underline just under the baseline.
                let uy = (ty + style.size * 1.12).round();
                p.fill_rect((tx + ux).round(), uy, uw.round().max(1.0), 1.0, color);
            }
        }
        p.pop_clip();
    }
}

/// Paint a tooltip box with `text` below the control at `anchor`, kept inside `width`.
pub fn paint_tooltip(
    p: &mut dyn Painter,
    t: &Theme,
    text: &str,
    text_w: f32,
    anchor: [f32; 4],
    width: f32,
    scale: f32,
) {
    let [ax, ay, aw, ah] = anchor;
    let style = TextStyle {
        size: (12.0 * scale).round(),
        bold: false,
    };
    let pad = (8.0 * scale).round();
    let (w, h) = (
        text_w + 2.0 * pad,
        (style.size * 1.33 + 2.0 * (5.0 * scale)).round(),
    );
    let x = (ax + aw / 2.0 - w / 2.0)
        .clamp(2.0, (width - w - 2.0).max(2.0))
        .round();
    let y = (ay + ah + (6.0 * scale)).round();
    p.fill_rect(x + 1.0, y + 2.0, w, h, Rgba(0, 0, 0, 30));
    p.fill_rect(x, y, w, h, t.border);
    p.fill_rect(x + 1.0, y + 1.0, w - 2.0, h - 2.0, t.surface);
    p.text(
        text,
        x + pad,
        y + (h - style.size * 1.33) / 2.0,
        style,
        t.text,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixed;
    impl crate::TextMeasure for Fixed {
        fn measure(&self, text: &str, style: TextStyle) -> (f32, f32) {
            (
                text.chars().count() as f32 * style.size * 0.5,
                style.size * 1.33,
            )
        }
    }

    fn items() -> Vec<MenuItem> {
        (0..3)
            .map(|i| MenuItem {
                label: i.to_string(),
                choice: Choice::Size(i),
                checked: false,
                dim: false,
            })
            .collect()
    }

    #[test]
    fn access_keys_strip_markers_choose_items_and_underline_on_cues() {
        let (l, k) = super::split_access_key("Show all Subtotals at &Bottom of Group");
        assert_eq!(
            (l.as_str(), k),
            ("Show all Subtotals at Bottom of Group", Some(22))
        );
        assert_eq!(
            super::split_access_key("A && B"),
            ("A & B".to_owned(), None)
        );
        let keyed = vec![
            MenuItem {
                label: "&Do Not Show".into(),
                choice: Choice::Command("a".into()),
                checked: false,
                dim: false,
            },
            MenuItem {
                label: "&Include".into(),
                choice: Choice::Command("b".into()),
                checked: false,
                dim: true,
            },
        ];
        let m = Menu::open(keyed.clone(), [0.0, 0.0, 10.0, 10.0], (800.0, 600.0), 1.0)
            .with_access_keys(&Fixed, false);
        assert!(m.has_access_keys());
        assert_eq!(m.items()[0].label, "Do Not Show");
        assert_eq!(m.access_choice('D'), Some(Choice::Command("a".into())));
        assert_eq!(m.access_choice('i'), None, "dimmed items are not chosen");
        assert!(
            !m.cues,
            "no underline unless opened from the keyboard or cues are on"
        );
        let k = Menu::open(keyed, [0.0, 0.0, 10.0, 10.0], (800.0, 600.0), 1.0)
            .with_access_keys(&Fixed, true);
        assert!(k.cues);
        // Menus without access keys are unchanged.
        let plain = Menu::open(items(), [0.0, 0.0, 10.0, 10.0], (800.0, 600.0), 1.0);
        assert!(!plain.has_access_keys());
    }

    #[test]
    fn menus_stay_inside_window_and_remain_clickable() {
        for scale in [1.0, 1.25, 1.5, 2.0] {
            let viewport = (480.0 * scale, 300.0 * scale);
            for anchor in [
                [
                    viewport.0 - 30.0 * scale,
                    50.0 * scale,
                    28.0 * scale,
                    28.0 * scale,
                ],
                [viewport.0 - 5.0 * scale, viewport.1 - 5.0 * scale, 1.0, 1.0],
                [-40.0 * scale, 30.0 * scale, 10.0 * scale, 20.0 * scale],
            ] {
                let mut menu = Menu::open(items(), anchor, viewport, scale);
                assert!(menu.x >= 0.0 && menu.x + menu.w <= viewport.0);
                assert!(menu.y >= 0.0 && menu.y + menu.h <= viewport.1);
                let (x, y) = (
                    menu.x + menu.w / 2.0,
                    menu.y + menu.pad() + menu.item_h / 2.0,
                );
                menu.press(x, y);
                assert_eq!(menu.release(x, y), Some(Choice::Size(0)));
            }
        }
    }
}
