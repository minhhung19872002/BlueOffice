//! BlueEditor's Format panel: a pane at the right of the document while Edit Text is on (as
//! in the reference editor, measured at 125 %: a "Format" header with a close box, a "Text
//! Style" section with the font and size boxes and a colour swatch, a row of character
//! buttons and a row of paragraph alignment buttons). It changes the selected text (the whole
//! paragraph when nothing is selected) of the paragraph being typed into.
#![allow(clippy::single_match_else)]
use crate::editor_window::{inside, EditTool, EditorWindow};
use crate::popup::{Choice, MenuItem};
use crate::reader_icons::{self, Ink};
use crate::{Input, Painter, TextStyle};
use bo_pdfread::{Align, StyleChange};
use bo_render::{PathSeg, Rgba};

/// Pane width in logical pixels (reference: 437 px at 125 %).
pub(crate) const PANE_W: f32 = 350.0;

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

const TEXT: Rgba = Rgba(51, 51, 51, 255);
const MUTED: Rgba = Rgba(160, 160, 160, 255);
const EDGE: Rgba = Rgba(204, 204, 204, 255);
const LINE: Rgba = Rgba(224, 224, 224, 255);
const PRESSED: Rgba = Rgba(226, 226, 226, 255);

/// Buttons of the pane: command and icon; rows of six, 48 logical pixels apart.
const CHAR_ROW: [(&str, &str); 4] = [
    ("fmt.bold", "bold"),
    ("fmt.italic", "italic"),
    ("fmt.underline", "fmtUnderline"),
    ("fmt.strike", "fmtStrike"),
];
const ALIGN_ROW: [(&str, &str); 4] = [
    ("fmt.alignLeft", "alignLeft"),
    ("fmt.alignCenter", "alignCenter"),
    ("fmt.alignRight", "alignRight"),
    ("fmt.justify", "alignJustify"),
];

/// Hit boxes of the pane (window pixels) and their commands, from the last paint.
#[derive(Debug, Default, Clone)]
pub(crate) struct FormatPane {
    pub hits: Vec<([f32; 4], &'static str)>,
}

fn button_xy(k: usize) -> f32 {
    33.0 + 48.0 * k as f32
}

impl EditorWindow {
    /// Run a Format command; false when `cmd` is not one.
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
                self.pane_menu(items, cmd);
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
                self.pane_menu(items, cmd);
                return true;
            }
            "fmt.color" => {
                let cur = style.as_ref().map(|(s, _)| s.color);
                let items = COLORS
                    .iter()
                    .enumerate()
                    .map(|(i, (key, c))| (format!("fmt.color.{i}"), self.t(key), cur == Some(*c)))
                    .collect();
                self.pane_menu(items, cmd);
                return true;
            }
            "fmt.close" => {
                self.format_closed = true;
                return true;
            }
            "fmt.cColor" => {
                let cur = self.comment.color;
                let items = COLORS
                    .iter()
                    .enumerate()
                    .map(|(i, (key, c))| (format!("fmt.cColor.{i}"), self.t(key), cur == *c))
                    .collect();
                self.pane_menu(items, cmd);
                return true;
            }
            "fmt.cOpacity" => {
                let cur = self.comment.opacity;
                let items = [100, 75, 50, 25]
                    .iter()
                    .map(|v| {
                        let f = *v as f32 / 100.0;
                        (
                            format!("fmt.cOpacity.{v}"),
                            format!("{v}%"),
                            (f - cur).abs() < 0.01,
                        )
                    })
                    .collect();
                self.pane_menu(items, cmd);
                return true;
            }
            "fmt.cWidth" => {
                let cur = self.comment.width;
                let items = [1, 2, 3, 4, 6, 8]
                    .iter()
                    .map(|v| {
                        (
                            format!("fmt.cWidth.{v}"),
                            format!("{v} pt"),
                            (*v as f32 - cur).abs() < 0.01,
                        )
                    })
                    .collect();
                self.pane_menu(items, cmd);
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
            _ => {
                if let Some((_, c)) = cmd
                    .strip_prefix("fmt.cColor.")
                    .and_then(|s| s.parse::<usize>().ok())
                    .and_then(|i| COLORS.get(i))
                {
                    self.comment.color = *c;
                    return true;
                }
                if let Some(v) = cmd
                    .strip_prefix("fmt.cOpacity.")
                    .and_then(|s| s.parse::<f32>().ok())
                {
                    self.comment.opacity = (v / 100.0).clamp(0.05, 1.0);
                    return true;
                }
                if let Some(v) = cmd
                    .strip_prefix("fmt.cWidth.")
                    .and_then(|s| s.parse::<f32>().ok())
                {
                    self.comment.width = v.clamp(0.5, 12.0);
                    return true;
                }
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

    fn pane_menu(&mut self, items: Vec<(String, String, bool)>, cmd: &str) {
        let Some(r) = self.pane.hits.iter().find(|h| h.1 == cmd).map(|h| h.0) else {
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
        self.r.menu_at(items, r, "fmt.menu");
    }

    /// Whether the Format pane is shown: Edit Text is on and it was not closed.
    fn format_shown(&self) -> bool {
        (self.edit_tool() == Some(EditTool::Text) || self.comment.tool.is_some())
            && !self.format_closed
            && self.r.showing_doc()
    }

    /// Show or hide the Format pane with the Edit Text tool.
    pub(crate) fn sync_format(&mut self) {
        if self.edit_tool() != Some(EditTool::Text) && self.comment.tool.is_none() {
            self.format_closed = false;
        }
        let w = if self.format_shown() { PANE_W } else { 0.0 };
        self.r.set_right_pane(w);
    }

    /// A press in the Format pane; false when it is elsewhere.
    pub(crate) fn format_input(&mut self, input: &Input) -> bool {
        let Input::Press { x, y, .. } = *input else {
            return false;
        };
        let pane = self.r.right_pane_rect();
        if pane[2] <= 0.0 || !inside(pane, x, y) {
            return false;
        }
        let typing = self.edit.typing.as_ref().is_some_and(|t| t.block.is_some());
        let hit = self
            .pane
            .hits
            .iter()
            .find(|h| inside(h.0, x, y))
            .map(|h| h.1);
        if let Some(cmd) = hit {
            if typing || cmd == "fmt.close" || self.comment.tool.is_some() {
                self.format_command(cmd);
            }
        }
        true
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

    /// Paint the Format pane.
    #[allow(clippy::too_many_lines)]
    pub(crate) fn paint_format(&mut self, p: &mut dyn Painter) {
        let r = self.r.right_pane_rect();
        self.pane.hits.clear();
        if r[2] <= 0.0 {
            return;
        }
        let k = self.r.ui_scale();
        let px = |v: f32| (v * k).round();
        let style = |s125: f32| TextStyle {
            size: s125 / 1.375 * k,
            bold: false,
        };
        let accent = self.r.theme().accent;
        let accent_text = self.r.theme().accent_text;
        let (x0, y0) = (r[0], r[1]);
        p.fill_rect(r[0], r[1], r[2], r[3], Rgba(255, 255, 255, 255));
        p.fill_rect(x0, y0, 1.0, r[3], LINE);
        // Header: "Format" in the accent with a close box.
        p.text(
            &self.t("pdfEditor.format"),
            x0 + px(14.0),
            y0 + px(6.0),
            style(17.0),
            accent_text,
        );
        let cx = x0 + px(76.0);
        let cy = y0 + px(9.0);
        let c = px(8.0);
        p.path(
            &[
                PathSeg::M(cx, cy),
                PathSeg::L(cx + c, cy + c),
                PathSeg::M(cx + c, cy),
                PathSeg::L(cx, cy + c),
            ],
            None,
            Some((TEXT, 1.0)),
        );
        self.pane.hits.push((
            [cx - px(4.0), cy - px(4.0), c + px(8.0), c + px(8.0)],
            "fmt.close",
        ));
        p.fill_rect(x0 + 1.0, y0 + px(27.0), r[2] - 1.0, 1.0, LINE);
        if self.comment.tool.is_some() && self.edit_tool() != Some(EditTool::Text) {
            self.paint_comment_props(p, x0, y0);
            return;
        }
        // "Text Style" section.
        let ty = y0 + px(41.0);
        p.path(
            &[
                PathSeg::M(x0 + px(7.0), ty + px(4.0)),
                PathSeg::L(x0 + px(12.0), ty + px(9.0)),
                PathSeg::L(x0 + px(7.0), ty + px(9.0)),
                PathSeg::Z,
            ],
            Some(MUTED),
            None,
        );
        p.text(
            &self.t("pdfEditor.textStyle"),
            x0 + px(18.0),
            ty,
            style(17.0),
            TEXT,
        );
        let typing = self.edit.typing.as_ref().is_some_and(|t| t.block.is_some());
        let cur = self.edit.current_style();
        let ink = if typing { TEXT } else { MUTED };
        // Font and size boxes, colour swatch.
        let by = y0 + px(68.0);
        let bh = px(24.0);
        let boxes = [
            (
                "fmt.font",
                20.0,
                156.0,
                cur.as_ref().map(|(s, _)| s.family.clone()),
            ),
            (
                "fmt.size",
                164.0 + 20.0,
                68.0,
                cur.as_ref()
                    .map(|(s, _)| format!("{}", (s.size * 10.0).round() / 10.0)),
            ),
        ];
        for (cmd, bx, bw, label) in boxes {
            let (bx, bw) = (x0 + px(bx), px(bw));
            frame(p, [bx, by, bw, bh], EDGE);
            if let Some(l) = label {
                p.text(&l, bx + px(6.0), by + px(4.0), style(16.0), ink);
            }
            // Drop-down arrow.
            let (ax, ay) = (bx + bw - px(14.0), by + px(10.0));
            p.path(
                &[
                    PathSeg::M(ax, ay),
                    PathSeg::L(ax + px(7.0), ay),
                    PathSeg::L(ax + px(3.5), ay + px(4.0)),
                    PathSeg::Z,
                ],
                Some(ink),
                None,
            );
            self.pane.hits.push(([bx, by, bw, bh], cmd));
        }
        let sw = [x0 + px(262.0), by + px(1.0), px(22.0), px(22.0)];
        let col = cur.as_ref().map_or(Rgba(0, 0, 0, 255), |(s, _)| {
            Rgba(s.color[0], s.color[1], s.color[2], 255)
        });
        p.fill_rect(sw[0], sw[1], sw[2], sw[3], col);
        frame(p, sw, EDGE);
        self.pane.hits.push((sw, "fmt.color"));
        // Character and alignment rows.
        let on = |cmd: &str| -> bool {
            cur.as_ref().is_some_and(|(s, a)| match cmd {
                "fmt.bold" => s.bold,
                "fmt.italic" => s.italic,
                "fmt.alignLeft" => *a == Align::Left,
                "fmt.alignCenter" => *a == Align::Center,
                "fmt.alignRight" => *a == Align::Right,
                "fmt.justify" => *a == Align::Justify,
                _ => false,
            })
        };
        for (row, items) in [(128.0, &CHAR_ROW), (172.0, &ALIGN_ROW)] {
            for (i, (cmd, icon)) in items.iter().enumerate() {
                let (cx, cy) = (x0 + px(button_xy(i)), y0 + px(row));
                let s = px(28.0);
                let b = [cx - s / 2.0, cy - s / 2.0, s, s];
                let works = !matches!(*cmd, "fmt.underline" | "fmt.strike");
                if on(cmd) {
                    p.fill_rect(b[0], b[1], b[2], b[3], PRESSED);
                }
                let color = if typing && works { TEXT } else { MUTED };
                let is = px(20.0);
                reader_icons::draw(
                    p,
                    icon,
                    cx - is / 2.0,
                    cy - is / 2.0,
                    is,
                    Ink {
                        ink: color,
                        accent: if typing && works { accent } else { MUTED },
                    },
                );
                if works {
                    self.pane.hits.push((b, cmd));
                }
            }
        }
    }
}

impl EditorWindow {
    /// The Format pane of a comment tool: colour, opacity and line width of new comments.
    fn paint_comment_props(&mut self, p: &mut dyn Painter, x0: f32, y0: f32) {
        let k = self.r.ui_scale();
        let px = |v: f32| (v * k).round();
        let style = |s125: f32| TextStyle {
            size: s125 / 1.375 * k,
            bold: false,
        };
        let ty = y0 + px(41.0);
        p.text(
            &self.t("pdfEditor.appearance"),
            x0 + px(18.0),
            ty,
            style(17.0),
            TEXT,
        );
        let rows = [
            ("fmt.cColor", self.t("pdfEditor.fontColor"), None),
            (
                "fmt.cOpacity",
                self.t("pdfEditor.opacity"),
                Some(format!("{}%", (self.comment.opacity * 100.0).round())),
            ),
            (
                "fmt.cWidth",
                self.t("pdfEditor.thickness"),
                Some(format!("{} pt", self.comment.width)),
            ),
        ];
        for (i, (cmd, label, value)) in rows.into_iter().enumerate() {
            let y = y0 + px(68.0 + 36.0 * i as f32);
            p.text(&label, x0 + px(20.0), y + px(4.0), style(16.0), TEXT);
            let bx = x0 + px(150.0);
            let r = match value {
                Some(v) => {
                    let r = [bx, y, px(100.0), px(24.0)];
                    frame(p, r, EDGE);
                    p.text(&v, bx + px(6.0), y + px(4.0), style(16.0), TEXT);
                    let (ax, ay) = (bx + px(86.0), y + px(10.0));
                    p.path(
                        &[
                            PathSeg::M(ax, ay),
                            PathSeg::L(ax + px(7.0), ay),
                            PathSeg::L(ax + px(3.5), ay + px(4.0)),
                            PathSeg::Z,
                        ],
                        Some(TEXT),
                        None,
                    );
                    r
                }
                None => {
                    let c = self.comment.color;
                    let r = [bx, y + px(1.0), px(22.0), px(22.0)];
                    p.fill_rect(r[0], r[1], r[2], r[3], Rgba(c[0], c[1], c[2], 255));
                    frame(p, r, EDGE);
                    r
                }
            };
            self.pane.hits.push((r, cmd));
        }
    }
}

fn frame(p: &mut dyn Painter, r: [f32; 4], c: Rgba) {
    let (x, y) = (r[0].round() + 0.5, r[1].round() + 0.5);
    let (x1, y1) = ((r[0] + r[2]).round() - 0.5, (r[1] + r[3]).round() - 0.5);
    p.path(
        &[
            PathSeg::M(x, y),
            PathSeg::L(x1, y),
            PathSeg::L(x1, y1),
            PathSeg::L(x, y1),
            PathSeg::Z,
        ],
        None,
        Some((c, 1.0)),
    );
}
