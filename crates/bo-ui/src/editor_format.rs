//! BlueEditor's Format tab: a contextual ribbon tab shown while typing into a paragraph with
//! Edit Text (like the reference editor's text format controls): font family, size, colour,
//! bold, italic and paragraph alignment of the selected text (the whole paragraph when
//! nothing is selected).
use crate::editor_window::EditorWindow;
use crate::popup::{Choice, MenuItem};
use crate::reader_window::Kind;
use crate::TextMeasure;
use bo_pdfread::{Align, StyleChange};

/// Index of the Format tab after the editor's fixed tabs.
pub(crate) const TAB_FORMAT: usize = 9;

/// Label key of the Format tab.
const FORMAT_LABEL: &str = "pdfEditor.tabs.format";

/// Font sizes offered by the size menu (points).
const SIZES: [f32; 15] = [
    6.0, 8.0, 9.0, 10.0, 11.0, 12.0, 14.0, 16.0, 18.0, 20.0, 24.0, 28.0, 36.0, 48.0, 72.0,
];

/// Colours offered by the colour menu: label key and RGB.
const COLORS: [(&str, [u8; 3]); 8] = [
    ("pdfEditor.colors.black", [0, 0, 0]),
    ("pdfEditor.colors.gray", [0x59, 0x59, 0x59]),
    ("pdfEditor.colors.red", [0xC0, 0x00, 0x00]),
    ("pdfEditor.colors.orange", [0xE3, 0x6C, 0x09]),
    ("pdfEditor.colors.green", [0x00, 0x80, 0x3C]),
    ("pdfEditor.colors.blue", [0x1F, 0x4E, 0xA8]),
    ("pdfEditor.colors.purple", [0x70, 0x30, 0xA0]),
    ("pdfEditor.colors.white", [0xFF, 0xFF, 0xFF]),
];

/// Ribbon items of the Format tab.
pub(crate) fn format_items() -> Vec<(&'static str, Kind)> {
    use Kind::{Big, Sep};
    vec![
        ("tool.hand", Big("hand", "pdfReader.hand", false)),
        ("tool.select", Big("select", "pdfReader.select", false)),
        ("sep1", Sep),
        ("ed.editText", Big("editText", "pdfEditor.editText", false)),
        ("sep2", Sep),
        ("fmt.font", Big("font", "pdfEditor.font", true)),
        ("fmt.size", Big("fontSize", "pdfEditor.fontSize", true)),
        ("fmt.color", Big("fontColor", "pdfEditor.fontColor", true)),
        ("fmt.bold", Big("bold", "pdfEditor.bold", false)),
        ("fmt.italic", Big("italic", "pdfEditor.italic", false)),
        ("sep3", Sep),
        (
            "fmt.alignLeft",
            Big("alignLeft", "pdfEditor.alignLeft", false),
        ),
        (
            "fmt.alignCenter",
            Big("alignCenter", "pdfEditor.alignCenter", false),
        ),
        (
            "fmt.alignRight",
            Big("alignRight", "pdfEditor.alignRight", false),
        ),
        (
            "fmt.justify",
            Big("alignJustify", "pdfEditor.justify", false),
        ),
        ("sep4", Sep),
        ("fmt.done", Big("done", "pdfEditor.done", false)),
    ]
}

impl EditorWindow {
    /// Run a Format tab command; false when `cmd` is not one.
    pub(crate) fn format_command(&mut self, cmd: &str) -> bool {
        if !cmd.starts_with("fmt.") {
            return false;
        }
        let style = self.edit.current_style();
        let change = match cmd {
            "fmt.font" => {
                let serif = style
                    .as_ref()
                    .is_some_and(|(s, _)| s.family == "Tinos" || s.family.contains("Times"));
                let items = vec![
                    (
                        "fmt.family.serif".to_owned(),
                        self.t("pdfEditor.serif"),
                        serif,
                    ),
                    (
                        "fmt.family.sans".to_owned(),
                        self.t("pdfEditor.sans"),
                        !serif,
                    ),
                ];
                self.menu_checked(items, "fmt.font");
                return true;
            }
            "fmt.size" => {
                let cur = style.as_ref().map_or(0.0, |(s, _)| s.size);
                let items = SIZES
                    .iter()
                    .map(|&s| {
                        (
                            format!("fmt.size.{s}"),
                            format!("{s}"),
                            (s - cur).abs() < 0.05,
                        )
                    })
                    .collect();
                self.menu_checked(items, "fmt.size");
                return true;
            }
            "fmt.color" => {
                let cur = style.as_ref().map(|(s, _)| s.color);
                let items = COLORS
                    .iter()
                    .enumerate()
                    .map(|(i, (key, c))| (format!("fmt.color.{i}"), self.t(key), cur == Some(*c)))
                    .collect();
                self.menu_checked(items, "fmt.color");
                return true;
            }
            "fmt.bold" => StyleChange::Bold(!style.as_ref().is_some_and(|(s, _)| s.bold)),
            "fmt.italic" => StyleChange::Italic(!style.as_ref().is_some_and(|(s, _)| s.italic)),
            "fmt.family.serif" => StyleChange::Serif(true),
            "fmt.family.sans" => StyleChange::Serif(false),
            "fmt.alignLeft" | "fmt.alignCenter" | "fmt.alignRight" | "fmt.justify" => {
                let a = match cmd {
                    "fmt.alignLeft" => Align::Left,
                    "fmt.alignCenter" => Align::Center,
                    "fmt.alignRight" => Align::Right,
                    _ => Align::Justify,
                };
                if let Some(v) = self.r.view_mut() {
                    self.edit.align(v, a);
                }
                return true;
            }
            "fmt.done" => {
                if let Some(v) = self.r.view_mut() {
                    self.edit.finish_typing(Some(v));
                }
                return true;
            }
            _ => {
                if let Some(s) = cmd
                    .strip_prefix("fmt.size.")
                    .and_then(|s| s.parse::<f32>().ok())
                {
                    StyleChange::Size(s)
                } else if let Some((_, c)) = cmd
                    .strip_prefix("fmt.color.")
                    .and_then(|s| s.parse::<usize>().ok())
                    .and_then(|i| COLORS.get(i))
                {
                    StyleChange::Color(*c)
                } else {
                    return true;
                }
            }
        };
        if let Some(v) = self.r.view_mut() {
            self.edit.style(v, &change);
        }
        true
    }

    fn menu_checked(&mut self, items: Vec<(String, String, bool)>, id: &'static str) {
        let Some(r) = self.r.control_rect(id) else {
            return;
        };
        let items = items
            .into_iter()
            .map(|(cmd, label, checked)| MenuItem {
                label,
                choice: Choice::Command(cmd),
                checked,
                dim: false,
            })
            .collect();
        self.r.menu_at(items, r, id);
    }

    /// Show the Format tab while typing into a paragraph (selected when it appears), hide it
    /// afterwards; pressed states of its buttons follow the style at the caret.
    pub(crate) fn sync_format(&mut self, m: &dyn TextMeasure) {
        let typing = self.edit.typing.as_ref().is_some_and(|t| t.block.is_some());
        let shown = self
            .r
            .profile
            .as_ref()
            .is_some_and(|p| p.tabs.len() > TAB_FORMAT);
        if typing && !shown {
            if let Some(p) = self.r.profile.as_mut() {
                p.tabs.push(FORMAT_LABEL);
            }
            self.format_before = Some(self.r.ribbon_tab());
            self.r.set_ribbon_tab(TAB_FORMAT, m);
        } else if !typing && shown {
            if let Some(p) = self.r.profile.as_mut() {
                p.tabs.truncate(TAB_FORMAT);
            }
            let back = self.format_before.take().unwrap_or(2);
            let tab = self.r.ribbon_tab();
            self.r
                .set_ribbon_tab(if tab >= TAB_FORMAT { back } else { tab }, m);
        }
        for id in [
            "fmt.font",
            "fmt.size",
            "fmt.color",
            "fmt.bold",
            "fmt.italic",
            "fmt.alignLeft",
            "fmt.alignCenter",
            "fmt.alignRight",
            "fmt.justify",
        ] {
            self.r.enabled_override.insert(id, typing);
        }
        if let Some((s, a)) = self.edit.current_style() {
            if s.bold {
                self.r.on.insert("fmt.bold");
            }
            if s.italic {
                self.r.on.insert("fmt.italic");
            }
            self.r.on.insert(match a {
                Align::Left => "fmt.alignLeft",
                Align::Center => "fmt.alignCenter",
                Align::Right => "fmt.alignRight",
                Align::Justify => "fmt.justify",
            });
        }
    }

    /// Status bar text for the style at the caret: family and size.
    pub(crate) fn format_status(&self) -> Option<String> {
        let (s, _) = self.edit.current_style()?;
        Some(format!(
            "{}, {} pt",
            s.family,
            (s.size * 10.0).round() / 10.0
        ))
    }
}
