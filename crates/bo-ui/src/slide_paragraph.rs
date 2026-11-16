//! BlueSlide Home > Paragraph, round 7: the Line Spacing menu (1.0 … 3.0, the current one
//! checked, Line Spacing Options…) and the Paragraph dialog (indents and spacing), as the
//! reference presentation app has them.
use super::{MenuKind, SlideWindow};
use crate::dialog::{Dialog, DialogButton, DialogKind};
use crate::popup::{Choice, Menu, MenuItem};
use crate::ribbon::RibbonHit;
use bo_engine::slide_edit::text::{ParaValues, TextFormat};

/// Commands of this module (always enabled).
pub const COMMANDS: [&str; 5] = [
    "format.lineSpacing",
    "format.paragraphSettings",
    "format.clear",
    "format.changeCase",
    "format.charSpacing",
];

/// Multiples of the Line Spacing menu (reference).
const MULTIPLES: [f64; 5] = [1.0, 1.5, 2.0, 2.5, 3.0];

/// Character Spacing menu: key, label, points (reference: Very Tight −3, Tight −1.5,
/// Normal, Loose +3, Very Loose +6).
const SPACINGS: [(&str, &str, f64); 5] = [
    ("veryTight", "Very Tight", -3.0),
    ("tight", "Tight", -1.5),
    ("normal", "Normal", 0.0),
    ("loose", "Loose", 3.0),
    ("veryLoose", "Very Loose", 6.0),
];

/// Name Character Spacing (tooltip).
pub(super) fn install_strings(s: &mut crate::strings::Strings) {
    let v = s.get("slideNative.charSpacing").to_owned();
    s.set("commands.format.charSpacing", &v);
}

/// Own icon of Character Spacing: an A and a V over a two-headed arrow.
pub(super) fn install_icons(icons: &mut crate::icons::Icons) {
    icons.add_solid(
        "format.charSpacing",
        &[
            ("M3 14l3.5-10l3.5 10M4.3 10.5h4.4", None),
            ("M12 4l3.5 10l3.5-10", None),
            ("M3 19h18M5 17l-2 2l2 2M19 17l2 2l-2 2", None),
        ],
    );
}

/// Points per inch / centimetre.
const PT_IN: f64 = 72.0;
const PT_CM: f64 = 72.0 / 2.54;

impl SlideWindow {
    /// Home > Paragraph commands of this module; `None` when `id` is not one.
    pub(super) fn paragraph_action(&mut self, id: &str) -> Option<bool> {
        Some(match id {
            "format.lineSpacing" => {
                self.open_line_spacing_menu();
                true
            }
            "format.paragraphSettings" => {
                self.open_paragraph_dialog();
                true
            }
            // Clear All Formatting (Ctrl+Space on the reference).
            "format.clear" => self.view.format_text(TextFormat::ClearChar),
            "format.changeCase" => {
                self.open_case_menu();
                true
            }
            "format.charSpacing" => {
                self.open_spacing_menu();
                true
            }
            _ => return None,
        })
    }

    /// Character Spacing: the reference's five steps (points between characters).
    fn open_spacing_menu(&mut self) {
        let Some(r) = self
            .ribbon
            .rect_of(RibbonHit::Command("format.charSpacing"))
        else {
            return;
        };
        let cur = self.view.format().spacing.unwrap_or(0.0);
        let items = SPACINGS
            .iter()
            .map(|(k, label, pt)| MenuItem {
                label: self.t(&format!("slideNative.spacing_{k}"), label),
                choice: Choice::Command(format!("chsp:{pt}")),
                checked: (cur - pt).abs() < 0.01,
                dim: false,
            })
            .collect();
        self.menu_kind = MenuKind::Duration;
        self.menu = Some(Menu::open(items, r, self.size, self.scale));
    }

    /// A Character Spacing menu choice (`chsp:<points>`).
    pub(super) fn choose_spacing(&mut self, v: &str) -> bool {
        match v.parse::<f64>() {
            Ok(points) => self.view.format_text(TextFormat::Spacing { points }),
            Err(_) => false,
        }
    }

    fn open_case_menu(&mut self) {
        let Some(r) = self.ribbon.rect_of(RibbonHit::Command("format.changeCase")) else {
            return;
        };
        let items = [
            ("sentence", "Sentence case."),
            ("lower", "lowercase"),
            ("upper", "UPPERCASE"),
            ("title", "Capitalize Each Word"),
            ("toggle", "tOGGLE cASE"),
        ]
        .iter()
        .map(|(k, label)| MenuItem {
            label: self.t(&format!("slideNative.case_{k}"), label),
            choice: Choice::Command(format!("case:{k}")),
            checked: false,
            dim: false,
        })
        .collect();
        self.menu_kind = MenuKind::Duration;
        self.menu = Some(Menu::open(items, r, self.size, self.scale));
    }

    /// Change Case of the selected text (`case:<kind>`); it stays selected.
    pub(super) fn change_case(&mut self, kind: &str) -> bool {
        let Some(te) = self.view.text_edit().cloned() else {
            return false;
        };
        let (a, b) = (te.start.min(te.end), te.start.max(te.end));
        if a == b {
            return false;
        }
        let units: Vec<u16> = te.text.encode_utf16().collect();
        let Some(part) = units.get(a..b).map(String::from_utf16_lossy) else {
            return false;
        };
        let new = change_case(&part, kind);
        if new == part {
            return false;
        }
        let id = te.id;
        let cell = te.cell;
        self.view.handle(crate::Input::Text(new.clone()));
        self.view
            .run(bo_engine::slide_edit::SlideCommand::TextSelect {
                id: Some(id),
                start: a,
                end: a + new.encode_utf16().count(),
                cell,
            })
    }

    fn open_line_spacing_menu(&mut self) {
        let Some(r) = self
            .ribbon
            .rect_of(RibbonHit::Command("format.lineSpacing"))
        else {
            return;
        };
        let cur = self.view.format().line_spacing.unwrap_or(1.0);
        let mut items: Vec<MenuItem> = MULTIPLES
            .iter()
            .map(|m| MenuItem {
                label: format!("{m:.1}"),
                choice: Choice::Command(format!("lspc:{m}")),
                checked: (cur - m).abs() < 1e-6,
                dim: false,
            })
            .collect();
        items.push(MenuItem {
            label: self.t("slideNative.lineSpacingOptions", "Line Spacing Options..."),
            choice: Choice::Command("format.paragraphSettings".into()),
            checked: false,
            dim: false,
        });
        self.menu_kind = MenuKind::Duration;
        // A line above Line Spacing Options (reference).
        self.menu =
            Some(Menu::open(items, r, self.size, self.scale).with_decor(Vec::new(), vec![5]));
    }

    /// A Line Spacing menu choice (`lspc:<multiple>`).
    pub(super) fn choose_line_spacing(&mut self, v: &str) -> bool {
        match v.parse::<f64>() {
            Ok(value) => self.view.format_text(TextFormat::LineSpacing { value }),
            Err(_) => false,
        }
    }

    fn unit(&self) -> (f64, &'static str) {
        if self.metric {
            (PT_CM, "cm")
        } else {
            (PT_IN, "\"")
        }
    }

    fn open_paragraph_dialog(&mut self) {
        let f = self.view.format();
        let v = f.paragraph.unwrap_or_default();
        let (k, u) = self.unit();
        let len = |pt: f64| format!("{:.2}{u}", pt / k);
        let pts = |pt: Option<f64>| format!("{} pt", fmt_num(pt.unwrap_or(0.0)));
        let line = match (v.line_exact, f.line_spacing) {
            (Some(e), _) => format!("{} pt", fmt_num(e)),
            (None, Some(m)) => fmt_num(m),
            (None, None) => "1".into(),
        };
        let d = Dialog::new(
            DialogKind::Paragraph,
            self.t("slideNative.paragraphTitle", "Paragraph"),
            vec![
                (
                    self.t("slideNative.beforeText", "Before text:"),
                    len(v.before_text),
                ),
                (
                    self.t("slideNative.firstLine", "First line (- hanging):"),
                    len(v.first_line),
                ),
                (
                    self.t("slideNative.spaceBefore", "Before:"),
                    pts(v.space_before),
                ),
                (
                    self.t("slideNative.spaceAfter", "After:"),
                    pts(v.space_after),
                ),
                (
                    self.t(
                        "slideNative.lineSpacingAt",
                        "Line Spacing (multiple or pt):",
                    ),
                    line,
                ),
            ],
            vec![
                (self.t("slideNative.ok", "OK"), DialogButton::Ok),
                (self.t("slideNative.cancel", "Cancel"), DialogButton::Cancel),
            ],
            Vec::new(),
        );
        self.menu = None;
        self.dialog = Some((d, super::ins::SlideDialog::Paragraph));
        self.dialog_layout = true;
    }

    /// OK in the Paragraph dialog: the typed values (lengths in the window's unit, spacing
    /// in points; the line spacing a multiple, or points with "pt").
    pub(super) fn paragraph_button(&mut self, values: &[String]) {
        let (k, _) = self.unit();
        let num = |i: usize| -> Option<f64> {
            let s = values.get(i)?;
            let t: String = s
                .chars()
                .filter(|c| c.is_ascii_digit() || matches!(c, '.' | ',' | '-'))
                .collect();
            t.replace(',', ".").parse().ok()
        };
        let cur = self.view.format().paragraph.unwrap_or_default();
        let line = values.get(4).map(|s| s.to_ascii_lowercase());
        let exact = line.as_deref().is_some_and(|s| s.contains("pt"));
        let mut v = ParaValues {
            before_text: num(0).map_or(cur.before_text, |x| (x * k).max(0.0)),
            first_line: num(1).map_or(cur.first_line, |x| x * k),
            space_before: num(2).map(|x| x.max(0.0)),
            space_after: num(3).map(|x| x.max(0.0)),
            line_exact: None,
        };
        if exact {
            v.line_exact = num(4).map(|x| x.max(0.0));
        }
        let mut done = self.view.format_text(TextFormat::Paragraph { values: v });
        if !exact {
            if let Some(m) = num(4).filter(|m| *m > 0.0) {
                done |= self.view.format_text(TextFormat::LineSpacing { value: m });
            }
        }
        let _ = done;
        self.dialog = None;
    }
}

/// `s` in another case: `sentence` (first letter of each sentence upper, the rest lower),
/// `lower`, `upper`, `title` (each word capitalised), `toggle` (each letter swapped).
pub(super) fn change_case(s: &str, kind: &str) -> String {
    match kind {
        "lower" => s.to_lowercase(),
        "upper" => s.to_uppercase(),
        "toggle" => s
            .chars()
            .flat_map(|c| {
                if c.is_uppercase() {
                    c.to_lowercase().collect::<Vec<_>>()
                } else {
                    c.to_uppercase().collect()
                }
            })
            .collect(),
        "title" | "sentence" => {
            let title = kind == "title";
            let mut out = String::with_capacity(s.len());
            let mut start = true;
            for c in s.chars() {
                if start && c.is_alphabetic() {
                    out.extend(c.to_uppercase());
                    start = false;
                } else {
                    out.extend(c.to_lowercase());
                    if title && c.is_whitespace() {
                        start = true;
                    } else if c.is_alphabetic() || c.is_numeric() {
                        start = false;
                    }
                }
                if !title && matches!(c, '.' | '!' | '?' | '\n') {
                    start = true;
                }
            }
            out
        }
        _ => s.to_owned(),
    }
}

/// A number without trailing zeros (10, 0.9, 1.5).
fn fmt_num(v: f64) -> String {
    let s = format!("{v:.2}");
    s.trim_end_matches('0').trim_end_matches('.').to_owned()
}
