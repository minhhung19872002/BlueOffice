//! BlueReader round 7: keyboard access to the ribbon like the desktop reader (and the desktop
//! office suites): Alt (pressed and released alone) or F10 shows key tips on the ribbon tabs;
//! typing a tab's letter opens it and shows the tips of its commands; typing a command's tip
//! runs it. Arrow keys move a focus ring along the tabs, Down / Enter goes into the tab's
//! commands, Left / Right / Tab move between them, Enter runs the focused one, Escape steps
//! back. The host turns Alt / F10 into `Key::F(10)` (`win32::enable_ribbon_keys`).
use super::{Kind, ReaderWindow};
use crate::{Input, Key, Mods, Painter, TextMeasure, TextStyle};
use bo_render::Rgba;

/// Key tip badge: dark fill, white letter (contrast 12.6 : 1).
const TIP_BG: Rgba = Rgba(64, 64, 64, 255);
const TIP_TEXT: Rgba = Rgba(255, 255, 255, 255);
/// Keyboard focus ring: the accent at full strength (≥ 3 : 1 against the ribbon, WCAG 1.4.11).
const FOCUS: Rgba = Rgba(0, 84, 153, 255);

/// Where keyboard access to the ribbon is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum KeyLevel {
    /// Not in use.
    #[default]
    Off,
    /// Tips on the tabs; `focus` is the tab with the focus ring (0 = File).
    Tabs { focus: usize },
    /// Tips on the commands of the open tab; `focus` indexes [`ReaderWindow::key_items`].
    Items { focus: usize },
}

impl ReaderWindow {
    /// Commands of the open tab that take key tips: (control id, tip).
    pub(crate) fn key_items(&self) -> Vec<(&'static str, String)> {
        let mut used: Vec<String> = Vec::new();
        let mut out = Vec::new();
        // Only the ribbon's body (not the title bar, navigation column or status bar).
        let (top, bottom) = (self.tabs_y(), self.ribbon_bottom());
        for c in self
            .controls
            .iter()
            .filter(|c| c.r[1] > top && c.r[1] + c.r[3] <= bottom + 1.0)
        {
            let label = match c.kind {
                Kind::Big(_, key, _) | Kind::Row(_, key, _) if !key.is_empty() => {
                    self.strings.get(key).to_owned()
                }
                Kind::Big(..) | Kind::Row(..) | Kind::Small(_) => {
                    c.id.rsplit('.').next().unwrap_or(c.id).to_owned()
                }
                _ => continue,
            };
            let tip = tip_for(&label, &used);
            used.push(tip.clone());
            out.push((c.id, tip));
        }
        out
    }

    /// Tips of the ribbon tabs, File first.
    pub(crate) fn key_tabs(&self) -> Vec<String> {
        let mut used: Vec<String> = Vec::new();
        self.tab_label_keys()
            .iter()
            .map(|k| {
                let t = tip_for(self.strings.get(k), &used);
                used.push(t.clone());
                t
            })
            .collect()
    }

    /// Keyboard access: `Some(redraw)` when it took the input.
    pub(crate) fn route_keys7(&mut self, input: &Input, m: &dyn TextMeasure) -> Option<bool> {
        let level = self.keys7;
        match (input, level) {
            (Input::Key(Key::F(10), _), KeyLevel::Off) => {
                if self.menu.is_some() || self.dialog.is_some() || self.show_start {
                    return None;
                }
                self.keys7 = KeyLevel::Tabs {
                    focus: self.ribbon_tab + 1,
                };
                Some(true)
            }
            (_, KeyLevel::Off) => None,
            (Input::Key(Key::F(10) | Key::Escape, _), KeyLevel::Tabs { .. })
            | (Input::Key(Key::F(10), _), KeyLevel::Items { .. }) => {
                self.keys7 = KeyLevel::Off;
                Some(true)
            }
            (Input::Key(Key::Escape, _), KeyLevel::Items { .. }) => {
                self.keys7 = KeyLevel::Tabs {
                    focus: self.ribbon_tab + 1,
                };
                Some(true)
            }
            (Input::Key(k, mods), KeyLevel::Tabs { focus }) => {
                Some(self.keys7_tabs_key(*k, *mods, focus, m))
            }
            (Input::Key(k, mods), KeyLevel::Items { focus }) => {
                Some(self.keys7_items_key(*k, *mods, focus, m))
            }
            (Input::Text(s), _) => Some(self.keys7_text(s, m)),
            // A click or wheel ends keyboard access and goes on as usual.
            (Input::Press { .. } | Input::Wheel(_), _) => {
                self.keys7 = KeyLevel::Off;
                None
            }
            _ => None,
        }
    }

    fn keys7_tabs_key(&mut self, k: Key, _mods: Mods, focus: usize, m: &dyn TextMeasure) -> bool {
        let n = self.tab_rects.len().max(1);
        match k {
            Key::Left => self.keys7_focus_tab((focus + n - 1) % n, m),
            Key::Right => self.keys7_focus_tab((focus + 1) % n, m),
            Key::Down | Key::Enter => self.keys7_open_tab(focus, m),
            _ => return false,
        }
        true
    }

    /// Move the focus ring to tab `i`, showing it (not File: its menu opens with Enter).
    fn keys7_focus_tab(&mut self, i: usize, m: &dyn TextMeasure) {
        if i > 0 && i - 1 != self.ribbon_tab {
            self.ribbon_tab = i - 1;
            self.relayout(m);
        }
        self.keys7 = KeyLevel::Tabs { focus: i };
    }

    /// Open tab `i` (File: its menu) and show its commands' tips.
    fn keys7_open_tab(&mut self, i: usize, m: &dyn TextMeasure) {
        if i == 0 {
            self.keys7 = KeyLevel::Off;
            if let Some(&r) = self.tab_rects.first() {
                if self.file_backstage {
                    self.requests
                        .push(super::ReaderRequest::Command("file.backstage".into()));
                } else {
                    self.open_file_menu(r);
                }
            }
            return;
        }
        if i - 1 != self.ribbon_tab {
            self.ribbon_tab = i - 1;
            self.relayout(m);
        }
        self.keys7 = KeyLevel::Items { focus: 0 };
    }

    fn keys7_items_key(&mut self, k: Key, mods: Mods, focus: usize, m: &dyn TextMeasure) -> bool {
        let n = self.key_items().len();
        if n == 0 {
            self.keys7 = KeyLevel::Off;
            return true;
        }
        let back = (focus + n - 1) % n;
        let next = (focus + 1) % n;
        match k {
            Key::Left | Key::Up => self.keys7 = KeyLevel::Items { focus: back },
            Key::Tab if mods.shift => self.keys7 = KeyLevel::Items { focus: back },
            Key::Right | Key::Down | Key::Tab => self.keys7 = KeyLevel::Items { focus: next },
            Key::Enter => {
                if let Some((id, _)) = self.key_items().get(focus).cloned() {
                    self.keys7_run(id, m);
                }
            }
            _ => return false,
        }
        true
    }

    /// Typed tip letters: a tab's or a command's.
    fn keys7_text(&mut self, s: &str, m: &dyn TextMeasure) -> bool {
        let typed = s.trim().to_uppercase();
        if typed.is_empty() {
            return true;
        }
        match self.keys7 {
            KeyLevel::Tabs { .. } => {
                if let Some(i) = self.key_tabs().iter().position(|t| *t == typed) {
                    self.keys7_open_tab(i, m);
                }
            }
            KeyLevel::Items { .. } => {
                let hit = self.key_items().into_iter().find(|(_, t)| *t == typed);
                if let Some((id, _)) = hit {
                    self.keys7_run(id, m);
                }
            }
            KeyLevel::Off => return false,
        }
        true
    }

    /// Run a ribbon command as a click on it would (greyed commands do nothing).
    fn keys7_run(&mut self, id: &'static str, m: &dyn TextMeasure) {
        self.keys7 = KeyLevel::Off;
        if !self.enabled(id) {
            return;
        }
        if let Some(r) = self.controls.iter().find(|c| c.id == id).map(|c| c.r) {
            self.click(id, r, m);
        }
    }

    /// Key tips and the focus ring over the ribbon.
    pub(crate) fn paint_keys7(&self, p: &mut dyn Painter) {
        let style = TextStyle {
            size: self.px(11.0).round(),
            bold: false,
        };
        let badge = |p: &mut dyn Painter, cx: f32, cy: f32, tip: &str| {
            let w = (self.px(8.0) + self.px(7.0) * tip.chars().count() as f32).round();
            let h = self.px(16.0).round();
            let (x, y) = ((cx - w / 2.0).round(), (cy - h / 2.0).round());
            p.path(
                &super::rounded(x, y, w, h, self.px(2.0)),
                Some(TIP_BG),
                None,
            );
            let tw = self.px(6.6) * tip.chars().count() as f32;
            p.text(
                tip,
                (x + (w - tw) / 2.0).round(),
                (y + (h - style.size * 1.33) / 2.0).round(),
                style,
                TIP_TEXT,
            );
        };
        let ring = |p: &mut dyn Painter, r: [f32; 4]| {
            let k = self.scale.max(1.0).round() * 2.0;
            let (x, y, w, h) = (r[0].round(), r[1].round(), r[2].round(), r[3].round());
            p.fill_rect(x, y, w, k, FOCUS);
            p.fill_rect(x, y + h - k, w, k, FOCUS);
            p.fill_rect(x, y, k, h, FOCUS);
            p.fill_rect(x + w - k, y, k, h, FOCUS);
        };
        match self.keys7 {
            KeyLevel::Off => {}
            KeyLevel::Tabs { focus } => {
                for (i, (r, tip)) in self.tab_rects.iter().zip(self.key_tabs()).enumerate() {
                    if i == focus {
                        ring(p, *r);
                    }
                    badge(p, r[0] + r[2] / 2.0, r[1] + r[3], &tip);
                }
            }
            KeyLevel::Items { focus } => {
                for (k, (id, tip)) in self.key_items().into_iter().enumerate() {
                    let Some(c) = self.controls.iter().find(|c| c.id == id) else {
                        continue;
                    };
                    let r = c.r;
                    if k == focus {
                        ring(p, r);
                    }
                    let cy = match c.kind {
                        Kind::Big(..) => r[1] + r[3] - self.px(4.0),
                        Kind::Small(_) => r[1] + r[3] - self.px(2.0),
                        _ => r[1] + r[3] / 2.0,
                    };
                    let cx = match c.kind {
                        Kind::Big(..) => r[0] + r[2] / 2.0,
                        Kind::Small(_) => r[0] + r[2] - self.px(2.0),
                        _ => r[0] + self.px(10.0),
                    };
                    badge(p, cx, cy, &tip);
                }
            }
        }
    }
}

/// A tip for `label` not in `used`: its first letter, else another letter of it, else the
/// first letter with a digit, else a number.
fn tip_for(label: &str, used: &[String]) -> String {
    let letters: Vec<char> = label
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_uppercase())
        .collect();
    for c in &letters {
        let t = c.to_string();
        if !used.contains(&t) {
            return t;
        }
    }
    if let Some(first) = letters.first() {
        for d in 1..=9 {
            let t = format!("{first}{d}");
            if !used.contains(&t) {
                return t;
            }
        }
    }
    (1..=used.len() as u32 + 1)
        .map(|n| n.to_string())
        .find(|t| !used.contains(t))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reader_tests::Fixed;
    use crate::reader_window::ReaderWindow;

    fn window() -> ReaderWindow {
        let doc = bo_pdfread::PdfDocument::open(
            crate::reader_tests::pdf(3),
            bo_pdfread::standard_fonts(),
        )
        .expect("pdf");
        let mut w = ReaderWindow::new("en-US");
        w.handle(Input::Scale(1.25), &Fixed);
        w.handle(
            Input::Resize {
                width: 1500.0,
                height: 1100.0,
            },
            &Fixed,
        );
        w.open(doc, "t.pdf", None);
        w
    }

    fn key(w: &mut ReaderWindow, k: Key, shift: bool) {
        w.handle(
            Input::Key(
                k,
                Mods {
                    shift,
                    ..Mods::default()
                },
            ),
            &Fixed,
        );
    }

    /// WCAG 2.2 contrast ratio of two sRGB colours.
    fn contrast(a: Rgba, b: Rgba) -> f32 {
        let lum = |c: Rgba| {
            let ch = |v: u8| {
                let v = f32::from(v) / 255.0;
                if v <= 0.040_45 {
                    v / 12.92
                } else {
                    ((v + 0.055) / 1.055).powf(2.4)
                }
            };
            0.2126 * ch(c.0) + 0.7152 * ch(c.1) + 0.0722 * ch(c.2)
        };
        let (x, y) = (lum(a), lum(b));
        (x.max(y) + 0.05) / (x.min(y) + 0.05)
    }

    #[test]
    fn colours_meet_wcag_contrast() {
        use super::super::{CANVAS, CHROME, HOVER, NAV_BG, PAGE_EDGE, PRESSED, TEXT};
        let theme = crate::theme::Theme::light("reader");
        // Text (1.4.3): 4.5 : 1 on every background it is drawn on, hovered or pressed too.
        for bg in [CHROME, NAV_BG, CANVAS, HOVER, PRESSED] {
            assert!(contrast(TEXT, bg) >= 4.5, "{bg:?} {}", contrast(TEXT, bg));
        }
        assert!(contrast(theme.accent_text, CHROME) >= 4.5);
        assert!(contrast(TIP_TEXT, TIP_BG) >= 4.5);
        // Non-text (1.4.11): 3 : 1 for the focus ring, key tips and the page edge.
        for bg in [CHROME, NAV_BG, Rgba(255, 255, 255, 255)] {
            assert!(contrast(FOCUS, bg) >= 3.0, "{bg:?}");
            assert!(contrast(TIP_BG, bg) >= 3.0, "{bg:?}");
        }
        assert!(contrast(PAGE_EDGE, CANVAS) >= 3.0);
    }

    #[test]
    fn tips_are_unique() {
        let used = vec!["H".to_owned(), "O".to_owned()];
        assert_eq!(tip_for("Home", &used), "M");
        assert_eq!(tip_for("Ho", &used), "H1");
        assert_eq!(tip_for("", &[]), "1");
        let w = window();
        let tabs = w.key_tabs();
        assert_eq!(tabs[0], "F");
        assert_eq!(tabs[1], "H");
        let items = w.key_items();
        assert!(items.len() > 5);
        let mut tips: Vec<&String> = items.iter().map(|i| &i.1).collect();
        tips.sort();
        tips.dedup();
        assert_eq!(tips.len(), items.len());
    }

    #[test]
    fn f10_tabs_arrows_and_typed_tips() {
        let mut w = window();
        key(&mut w, Key::F(10), false);
        assert_eq!(w.keys7, KeyLevel::Tabs { focus: 1 });
        // Right moves to the next tab and shows it.
        key(&mut w, Key::Right, false);
        assert_eq!(w.keys7, KeyLevel::Tabs { focus: 2 });
        assert_eq!(w.ribbon_tab, 1);
        // Typing View's tip opens it with its commands' tips.
        let v = w
            .key_tabs()
            .iter()
            .position(|t| t == "V")
            .expect("view tip");
        w.handle(Input::Text("v".into()), &Fixed);
        assert_eq!(w.ribbon_tab, v - 1);
        assert_eq!(w.keys7, KeyLevel::Items { focus: 0 });
        // Tab / Shift+Tab move the focus ring; Escape back to the tabs, again off.
        key(&mut w, Key::Tab, false);
        key(&mut w, Key::Tab, false);
        key(&mut w, Key::Tab, true);
        assert_eq!(w.keys7, KeyLevel::Items { focus: 1 });
        key(&mut w, Key::Escape, false);
        assert!(matches!(w.keys7, KeyLevel::Tabs { .. }));
        key(&mut w, Key::Escape, false);
        assert_eq!(w.keys7, KeyLevel::Off);
        // Typing a command's tip runs it: Home › Select.
        key(&mut w, Key::F(10), false);
        w.handle(Input::Text("h".into()), &Fixed);
        let items = w.key_items();
        let (_, tip) = items
            .iter()
            .find(|(id, _)| *id == "tool.select")
            .cloned()
            .expect("select");
        w.handle(Input::Text(tip.to_lowercase()), &Fixed);
        assert_eq!(w.keys7, KeyLevel::Off);
        assert_eq!(w.tool, super::super::Tool::Select);
        // Enter on the focused command runs it: Hand is first.
        key(&mut w, Key::F(10), false);
        key(&mut w, Key::Down, false);
        key(&mut w, Key::Enter, false);
        assert_eq!(w.tool, super::super::Tool::Hand);
        // A click ends keyboard access.
        key(&mut w, Key::F(10), false);
        w.handle(
            Input::Press {
                x: 700.0,
                y: 700.0,
                count: 1,
                shift: false,
            },
            &Fixed,
        );
        assert_eq!(w.keys7, KeyLevel::Off);
    }
}
