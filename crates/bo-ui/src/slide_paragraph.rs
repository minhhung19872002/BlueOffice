//! BlueSlide Home > Paragraph, round 7: the Line Spacing menu (1.0 … 3.0, the current one
//! checked, Line Spacing Options…) and the Paragraph dialog (indents and spacing), as the
//! reference presentation app has them.
use super::{MenuKind, SlideWindow};
use crate::dialog::{Dialog, DialogButton, DialogKind};
use crate::popup::{Choice, Menu, MenuItem};
use crate::ribbon::RibbonHit;
use bo_engine::slide_edit::text::{ParaValues, TextFormat};

/// Commands of this module (always enabled).
pub const COMMANDS: [&str; 12] = [
    "sl.toSmartArt",
    "format.lineSpacing",
    "format.paragraphSettings",
    "sl.clearFmt",
    "sl.shadow",
    "sl.changeCase",
    "sl.charSpacing",
    "sl.highlight",
    "sl.fontColor",
    "sl.columns",
    "sl.textDir",
    "sl.alignText",
];

/// Row items of Home > Paragraph placed in a column on the right (reference).
pub(super) fn late_row(id: &str) -> bool {
    matches!(id, "sl.textDir" | "sl.alignText" | "sl.toSmartArt")
}

/// Text Highlight Color menu: key, label, colour (`None`: no colour).
const HIGHLIGHTS: [(&str, &str, Option<&str>); 6] = [
    ("yellow", "Yellow", Some("FFFF00")),
    ("green", "Bright Green", Some("00FF00")),
    ("turquoise", "Turquoise", Some("00FFFF")),
    ("pink", "Pink", Some("FF00FF")),
    ("blue", "Blue", Some("0000FF")),
    ("none", "No Color", None),
];

/// Font Color menu: key, label, colour (`None`: automatic).
const COLORS: [(&str, &str, Option<&str>); 8] = [
    ("auto", "Automatic", None),
    ("black", "Black", Some("000000")),
    ("red", "Red", Some("FF0000")),
    ("orange", "Orange", Some("ED7D31")),
    ("yellow", "Yellow", Some("FFC000")),
    ("green", "Green", Some("70AD47")),
    ("blue", "Blue", Some("4472C4")),
    ("grey", "Gray", Some("7F7F7F")),
];

/// Text box menus of Home > Paragraph: (menu id, choice, label key, label).
const BODY_MENUS: [(&str, &str, &str, &str); 10] = [
    ("sl.textDir", "vert:horz", "dirHorz", "Horizontal"),
    (
        "sl.textDir",
        "vert:vert",
        "dirRotate90",
        "Rotate all text 90°",
    ),
    (
        "sl.textDir",
        "vert:vert270",
        "dirRotate270",
        "Rotate all text 270°",
    ),
    ("sl.textDir", "vert:wordArtVert", "dirStacked", "Stacked"),
    ("sl.alignText", "anchor:t", "alignTop", "Top"),
    ("sl.alignText", "anchor:ctr", "alignMiddle", "Middle"),
    ("sl.alignText", "anchor:b", "alignBottom", "Bottom"),
    ("sl.columns", "cols:1", "oneColumn", "One Column"),
    ("sl.columns", "cols:2", "twoColumns", "Two Columns"),
    ("sl.columns", "cols:3", "threeColumns", "Three Columns"),
];

/// Layouts offered by Convert to SmartArt.
const SMART_LAYOUTS: [&str; 7] = [
    "list",
    "process",
    "cycle",
    "hierarchy",
    "relationship",
    "matrix",
    "pyramid",
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

/// Name this module's buttons (tooltips).
pub(super) fn install_strings(s: &mut crate::strings::Strings) {
    for (id, key) in [
        ("sl.clearFmt", "clearFormatting"),
        ("sl.shadow", "textShadow"),
        ("sl.changeCase", "changeCase"),
        ("sl.charSpacing", "charSpacing"),
        ("sl.highlight", "highlightColor"),
        ("sl.fontColor", "fontColor"),
        ("sl.columns", "columns"),
        ("sl.textDir", "textDirection"),
        ("sl.alignText", "alignText"),
        ("sl.toSmartArt", "toSmartArt"),
    ] {
        let v = s.get(&format!("slideNative.{key}")).to_owned();
        s.set(&format!("commands.{id}"), &v);
    }
}

/// Own icons: Character Spacing (an A and a V over a two-headed arrow), Text Shadow (an S
/// over a grey copy), Clear Formatting (a letter beside a slanted pink eraser), Columns,
/// Text Direction and Align Text; aliases for the shared ones.
pub(super) fn install_icons(icons: &mut crate::icons::Icons) {
    use bo_render::Rgba;
    icons.alias("sl.changeCase", "format.changeCase");
    icons.alias("sl.highlight", "format.highlight");
    icons.alias("sl.fontColor", "format.color");
    let grey = Rgba(0xa6, 0xa6, 0xa6, 255);
    let pink = Rgba(0xe8, 0x8a, 0xb4, 255);
    let ink = Rgba(0x40, 0x40, 0x40, 255);
    icons.add_solid(
        "sl.charSpacing",
        &[
            ("M3 14l3.5-10l3.5 10M4.3 10.5h4.4", None),
            ("M12 4l3.5 10l3.5-10", None),
            ("M3 19h18M5 17l-2 2l2 2M19 17l2 2l-2 2", None),
        ],
    );
    icons.add_solid(
        "sl.shadow",
        &[
            (
                "M17 8.5a3.5 2.8 0 0 0-7 0.3c0 3.2 7 2.4 7 5.6a3.5 2.8 0 0 1-7 0.3",
                Some(grey),
            ),
            (
                "M15.5 6.5a3.5 2.8 0 0 0-7 0.3c0 3.2 7 2.4 7 5.6a3.5 2.8 0 0 1-7 0.3",
                None,
            ),
        ],
    );
    icons.add_solid(
        "sl.clearFmt",
        &[
            ("M3 16l4-12h2l4 12M5 12h6", None),
            ("M12 18l6-8l4 3l-6 8h-3Z", Some(pink)),
            ("M12 18l6-8l4 3l-6 8h-3Z", None),
        ],
    );
    icons.add_solid(
        "sl.columns",
        &[(
            "M3 5h7M3 9h7M3 13h7M3 17h7M14 5h7M14 9h7M14 13h7M14 17h7",
            None,
        )],
    );
    icons.add_solid(
        "sl.textDir",
        &[("M4 4h10M9 4v12", None), ("M19 5v14M17 17l2 2l2-2", None)],
    );
    // Convert to SmartArt: three linked boxes (green, as Insert > SmartArt).
    icons.add_solid(
        "sl.toSmartArt",
        &[
            ("M8 3h8v5H8Z", Some(Rgba(0x1e, 0x8e, 0x3e, 255))),
            ("M2 16h8v5H2Z", Some(Rgba(0x1e, 0x8e, 0x3e, 255))),
            ("M14 16h8v5h-8Z", Some(Rgba(0x1e, 0x8e, 0x3e, 255))),
            ("M12 8v4M6 16v-4h12v4", None),
        ],
    );
    icons.add_solid(
        "sl.alignText",
        &[("M3 3h18v18h-18Z", None), ("M7 10h10M7 14h10", Some(ink))],
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
            "sl.clearFmt" => self.view.format_text(TextFormat::ClearChar),
            "sl.shadow" => self.view.format_text(TextFormat::Shadow { value: None }),
            "sl.changeCase" => {
                self.open_case_menu();
                true
            }
            "sl.charSpacing" => {
                self.open_spacing_menu();
                true
            }
            "sl.toSmartArt" => {
                self.open_smart_convert();
                true
            }
            "sl.highlight" | "sl.fontColor" | "sl.columns" | "sl.textDir" | "sl.alignText" => {
                self.open_home_menu(id);
                true
            }
            _ => return None,
        })
    }

    /// Character Spacing: the reference's five steps (points between characters).
    fn open_spacing_menu(&mut self) {
        let Some(r) = self.ribbon.rect_of(RibbonHit::Command("sl.charSpacing")) else {
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

    /// Highlight / Font Color / Columns / Text Direction / Align Text menus.
    fn open_home_menu(&mut self, id: &str) {
        let Some(r) = COMMANDS
            .iter()
            .find(|c| **c == id)
            .and_then(|c| self.ribbon.rect_of(RibbonHit::Command(c)))
        else {
            return;
        };
        let item = |label: String, choice: String| MenuItem {
            label,
            choice: Choice::Command(choice),
            checked: false,
            dim: false,
        };
        let items: Vec<MenuItem> = match id {
            "sl.highlight" => HIGHLIGHTS
                .iter()
                .map(|(k, l, c)| {
                    item(
                        self.t(&format!("slideNative.hl_{k}"), l),
                        format!("hl:{}", c.unwrap_or("")),
                    )
                })
                .collect(),
            "sl.fontColor" => COLORS
                .iter()
                .map(|(k, l, c)| {
                    item(
                        self.t(&format!("slideNative.fc_{k}"), l),
                        format!("fc:{}", c.unwrap_or("")),
                    )
                })
                .collect(),
            _ => BODY_MENUS
                .iter()
                .filter(|(m, ..)| *m == id)
                .map(|(_, c, k, l)| {
                    item(self.t(&format!("slideNative.{k}"), l), format!("body:{c}"))
                })
                .collect(),
        };
        self.menu_kind = MenuKind::Duration;
        self.menu = Some(Menu::open(items, r, self.size, self.scale));
    }

    /// A choice of [`Self::open_home_menu`]: `hl:<RRGGBB or empty>`, `fc:…`,
    /// `body:vert:…` / `body:anchor:…` / `body:cols:n`.
    pub(super) fn choose_home(&mut self, kind: &str, v: &str) -> bool {
        let hex = (!v.is_empty()).then(|| v.to_owned());
        let f = match kind {
            "hl" => TextFormat::Highlight { hex },
            "fc" => TextFormat::Color { hex },
            _ => {
                let Some((what, val)) = v.split_once(':') else {
                    return false;
                };
                let s = Some(val.to_owned());
                match what {
                    "vert" => TextFormat::Body {
                        anchor: None,
                        vert: s,
                        columns: None,
                    },
                    "anchor" => TextFormat::Body {
                        anchor: s,
                        vert: None,
                        columns: None,
                    },
                    _ => TextFormat::Body {
                        anchor: None,
                        vert: None,
                        columns: val.parse().ok(),
                    },
                }
            }
        };
        self.view.format_text(f)
    }

    /// Convert to SmartArt: the diagram layouts, as Insert > SmartArt lists them.
    fn open_smart_convert(&mut self) {
        let Some(r) = self.ribbon.rect_of(RibbonHit::Command("sl.toSmartArt")) else {
            return;
        };
        let items = SMART_LAYOUTS
            .iter()
            .map(|k| MenuItem {
                label: self.t(&format!("slideNative.smartart_{k}"), k),
                choice: Choice::Command(format!("tosmart:{k}")),
                checked: false,
                dim: false,
            })
            .collect();
        self.menu_kind = MenuKind::Duration;
        self.menu = Some(Menu::open(items, r, self.size, self.scale));
    }

    /// Replace the selected text shape by a SmartArt diagram of `layout` holding one node
    /// per paragraph (list level kept), in the shape's place — one undo step per command.
    pub(super) fn convert_to_smartart(&mut self, layout: &str) -> bool {
        use bo_engine::slide_edit::smartart::SmartItem;
        let Some(st) = self.view.state() else {
            return false;
        };
        let [sel] = &st.selection[..] else {
            return false;
        };
        let id = sel.id;
        let rect = st
            .bounds
            .as_ref()
            .map(|b| bo_engine::slide_edit::geom::Rect {
                x: b.cx - b.w / 2.0,
                y: b.cy - b.h / 2.0,
                w: b.w,
                h: b.h,
            });
        let Some(tb) = self.view.engine().presentation().and_then(|p| {
            p.slides
                .get(self.view.current())?
                .data
                .shapes
                .iter()
                .find_map(|n| match n {
                    bo_model::slide::ShapeNode::Shape(s) if s.nv.id == id => s.text.clone(),
                    _ => None,
                })
        }) else {
            return false;
        };
        let items: Vec<SmartItem> = tb
            .paragraphs
            .iter()
            .map(|p| SmartItem {
                text: bo_engine::slide_edit::text::para_text(p),
                level: p
                    .props
                    .as_ref()
                    .and_then(|pp| pp.lvl)
                    .map_or(0, usize::from),
                ..Default::default()
            })
            .filter(|i| !i.text.trim().is_empty())
            .collect();
        if items.is_empty() {
            return false;
        }
        // Leave the text (if editing) so Delete removes the shape itself.
        self.view.run(bo_engine::slide_edit::SlideCommand::TextExit);
        self.view
            .run(bo_engine::slide_edit::SlideCommand::Select { ids: vec![id] });
        self.view.run(bo_engine::slide_edit::SlideCommand::Delete);
        self.activate_context = true;
        self.view
            .run(bo_engine::slide_edit::SlideCommand::InsertSmartArt {
                layout: layout.to_owned(),
                items,
                colors: None,
                style: None,
                rect,
            })
    }

    /// A Character Spacing menu choice (`chsp:<points>`).
    pub(super) fn choose_spacing(&mut self, v: &str) -> bool {
        match v.parse::<f64>() {
            Ok(points) => self.view.format_text(TextFormat::Spacing { points }),
            Err(_) => false,
        }
    }

    fn open_case_menu(&mut self) {
        let Some(r) = self.ribbon.rect_of(RibbonHit::Command("sl.changeCase")) else {
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
