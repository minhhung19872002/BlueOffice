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
const CHAR_ROW: [(&str, &str); 6] = [
    ("fmt.bold", "bold"),
    ("fmt.italic", "italic"),
    ("fmt.underline", "fmtUnderline"),
    ("fmt.strike", "fmtStrike"),
    ("fmt.super", "fmtSuper"),
    ("fmt.sub", "fmtSub"),
];
const ALIGN_ROW: [(&str, &str); 6] = [
    ("fmt.alignLeft", "alignLeft"),
    ("fmt.alignCenter", "alignCenter"),
    ("fmt.alignRight", "alignRight"),
    ("fmt.justify", "alignJustify"),
    ("fmt.indent", "fmtIndent"),
    ("fmt.outdent", "fmtOutdent"),
];

/// Ribbon items of the Edit tab (as the reference's; commands without a feature yet are
/// shown disabled).
pub(crate) fn edit_tab_items() -> Vec<(&'static str, crate::reader_window::Kind)> {
    use crate::reader_window::Kind::{Big, Sep};
    vec![
        ("sep1", Sep),
        (
            "ed.editText",
            Big("edEditText", "pdfEditor.editText", false),
        ),
        (
            "ed.editObject",
            Big("editObject", "pdfEditor.editObject", true),
        ),
        ("ed.linkJoin", Big("linkJoin", "pdfEditor.linkJoin", false)),
        ("ed.spell", Big("spell", "pdfEditor.spell", false)),
        (
            "ed.searchReplace",
            Big("searchReplace", "pdfEditor.searchReplace", false),
        ),
        ("sep2", Sep),
        ("ed.addText", Big("addText", "pdfEditor.addText", false)),
        ("ed.addImage", Big("addImage", "pdfEditor.addImages", true)),
        (
            "ed.addShapes",
            Big("addShapes", "pdfEditor.addShapes", true),
        ),
        ("sep3", Sep),
        ("ed.reflow", Big("reflow", "pdfEditor.reflow", false)),
        ("sep4", Sep),
        (
            "ed.articleBox",
            Big("articleBox", "pdfEditor.articleBox", false),
        ),
        ("sep5", Sep),
        ("ed.webLinks", Big("webLinks", "pdfEditor.webLinks", true)),
        ("ed.link", Big("link", "pdfEditor.link", false)),
        ("ed.bookmark", Big("bookmark", "pdfEditor.bookmark", false)),
        ("ed.crossRef", Big("crossRef", "pdfEditor.crossRef", false)),
        ("sep6", Sep),
        ("cm.file", Big("attach", "pdfEditor.fileAttachment", false)),
        (
            "ed.imageAnnot",
            Big("imageAnnot", "pdfEditor.imageAnnot", false),
        ),
        ("ed.media", Big("media", "pdfEditor.media", false)),
        ("ed.add3d", Big("add3d", "pdfEditor.add3d", false)),
    ]
}

/// Edit tab commands with no feature yet (shown disabled).
pub(crate) const EDIT_TAB_LATER: [&str; 0] = [];

/// Hit boxes of the pane (window pixels) and their commands, from the last paint.
#[derive(Debug, Default, Clone)]
pub(crate) struct FormatPane {
    pub hits: Vec<([f32; 4], &'static str)>,
    /// Paragraph spacing of the paragraph at (page, top, bottom) (reading the page's
    /// paragraphs is too slow for every frame).
    para: Option<((usize, u32, u32), Option<f32>)>,
    /// Scroll of the Edit Object pane (window pixels).
    pub scroll: f32,
    /// Style of the text object selected with Edit Object.
    pub obj_style: crate::editor_objtext::ObjStyleCache,
    /// Whether the selected text runs have kerning: (page, runs, edit count) and the answer.
    pub kerning: crate::editor_objtext::KerningCache,
}

fn button_xy(k: usize) -> f32 {
    42.0 + 48.0 * k as f32
}

impl EditorWindow {
    /// Edit tab commands of this module; false when `cmd` is not one.
    pub(crate) fn edit_tab_command(&mut self, cmd: &str) -> bool {
        if cmd != "ed.searchReplace" {
            return false;
        }
        let mut d = crate::dialog::Dialog::new(
            crate::dialog::DialogKind::Replace,
            self.t("pdfEditor.searchReplace"),
            vec![
                (self.t("pdfEditor.findWhat"), String::new()),
                (self.t("pdfEditor.replaceWith"), String::new()),
            ],
            vec![
                (self.t("pdfReader.ok"), crate::dialog::DialogButton::Ok),
                (
                    self.t("pdfReader.cancel"),
                    crate::dialog::DialogButton::Cancel,
                ),
            ],
            Vec::new(),
        );
        d.layout(
            &crate::editor_window::Estimate,
            self.r.window_size(),
            self.r.ui_scale(),
        );
        self.dialog2 = Some((d, crate::editor_actions::Ask2::Replace));
        true
    }

    /// Replace every `find` in the document's paragraphs with `with` (one undo step per
    /// paragraph); returns how many were replaced.
    pub(crate) fn replace_all(&mut self, find: &str, with: &str) -> usize {
        if find.is_empty() {
            return 0;
        }
        if let Some(v) = self.r.view_mut() {
            self.edit.finish_typing(Some(v));
        }
        let mut count = 0;
        let pages = self.r.view().map_or(0, crate::ReaderView::page_count);
        for page in 0..pages {
            let Some(v) = self.r.view_mut() else { break };
            let doc = v.doc_mut();
            loop {
                let Ok(objs) = doc.page_objects(page) else {
                    break;
                };
                let Some(b) = bo_pdfread::text_blocks(&objs)
                    .into_iter()
                    .find(|b| b.text.contains(find))
                else {
                    break;
                };
                let Ok(mut e) = doc.begin_block_edit(page, &b) else {
                    break;
                };
                let text: String = e.text.iter().collect();
                let n = text.matches(find).count();
                let new = text.replace(find, with);
                // Rebuild text and styles: each kept character keeps its style.
                let mut chars = Vec::new();
                let mut styles = Vec::new();
                let mut i = 0;
                let src: Vec<char> = e.text.clone();
                let f: Vec<char> = find.chars().collect();
                while i < src.len() {
                    if src[i..].starts_with(&f) {
                        let st = e.style_of.get(i).copied().unwrap_or(0);
                        for c in with.chars() {
                            chars.push(c);
                            styles.push(st);
                        }
                        i += f.len();
                    } else {
                        chars.push(src[i]);
                        styles.push(e.style_of.get(i).copied().unwrap_or(0));
                        i += 1;
                    }
                }
                debug_assert_eq!(chars.iter().collect::<String>(), new);
                e.text = chars;
                e.style_of = styles;
                let ok = doc.block_edit_update(&mut e).is_ok();
                doc.end_block_edit(&e);
                if !ok {
                    break;
                }
                count += n;
            }
            v.invalidate_page(page);
        }
        self.after_doc_edit();
        count
    }

    /// Run a Format command; false when `cmd` is not one.
    pub(crate) fn format_command(&mut self, cmd: &str) -> bool {
        if !cmd.starts_with("fmt.") {
            return false;
        }
        let style = self
            .edit
            .current_style()
            .or_else(|| self.object_text_style());
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
            "fmt.underline" => {
                StyleChange::Underline(!style.as_ref().is_some_and(|(s, _)| s.underline))
            }
            "fmt.strike" => StyleChange::Strike(!style.as_ref().is_some_and(|(s, _)| s.strike)),
            "fmt.super" => {
                StyleChange::Script(i8::from(style.as_ref().is_none_or(|(s, _)| s.script <= 0)))
            }
            "fmt.sub" => {
                StyleChange::Script(if style.as_ref().is_some_and(|(s, _)| s.script < 0) {
                    0
                } else {
                    -1
                })
            }
            "fmt.indent" | "fmt.outdent" => {
                let d = if cmd == "fmt.indent" { 18.0 } else { -18.0 };
                if let Some(v) = self.r.view_mut() {
                    self.edit.with_block(v, |b, _| b.shift_left(d));
                }
                return true;
            }
            "fmt.bullets" | "fmt.numbering" => {
                let items: Vec<(String, String, bool)> = if cmd == "fmt.bullets" {
                    ["\u{2022}", "\u{2013}", "\u{2713}", "\u{25E6}"]
                        .iter()
                        .enumerate()
                        .map(|(i, b)| (format!("fmt.list.b{i}"), (*b).to_owned(), false))
                        .collect()
                } else {
                    ["1.", "a)", "I."]
                        .iter()
                        .enumerate()
                        .map(|(i, b)| (format!("fmt.list.n{i}"), (*b).to_owned(), false))
                        .collect()
                };
                let mut items = items;
                items.push((
                    "fmt.list.none".into(),
                    self.t("pdfEditor.bulletNone"),
                    false,
                ));
                self.pane_menu(items, cmd);
                return true;
            }
            "fmt.lineSp" => {
                // Space added to the line height, in points (as the reference shows it).
                let Some((h, gap)) = self
                    .edit
                    .block()
                    .map(|b| (b.pitch - b.line_gap(), b.line_gap()))
                else {
                    return true;
                };
                let items = [0.0_f32, 0.6, 1.0, 2.0, 3.0, 6.0, 12.0]
                    .iter()
                    .map(|g| {
                        let v = ((h + g) * 100.0).round() / 100.0;
                        (
                            format!("fmt.pitch.{v}"),
                            format!("{g:.2}"),
                            (g - gap).abs() < 0.05,
                        )
                    })
                    .collect();
                self.pane_menu(items, cmd);
                return true;
            }
            "fmt.scale" => {
                let items = [50, 75, 90, 100, 110, 125, 150, 200]
                    .iter()
                    .map(|v| (format!("fmt.scale.{v}"), format!("{v}%"), false))
                    .collect();
                self.pane_menu(items, cmd);
                return true;
            }
            "fmt.charSp" | "fmt.wordSp" => {
                let key = if cmd == "fmt.charSp" {
                    "fmt.tc."
                } else {
                    "fmt.tw."
                };
                let items = [-0.5_f32, 0.0, 0.5, 1.0, 2.0, 4.0]
                    .iter()
                    .map(|v| (format!("{key}{v}"), format!("{v:.2}"), false))
                    .collect();
                self.pane_menu(items, cmd);
                return true;
            }
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
                if let Some(kind) = cmd.strip_prefix("fmt.list.") {
                    self.apply_list(kind);
                    return true;
                }
                if let Some(v) = cmd
                    .strip_prefix("fmt.pitch.")
                    .and_then(|s| s.parse::<f32>().ok())
                {
                    if let Some(view) = self.r.view_mut() {
                        self.edit.with_block(view, |b, _| b.pitch = v.max(1.0));
                    }
                    return true;
                }
                let num = |pre: &str| cmd.strip_prefix(pre).and_then(|s| s.parse::<f32>().ok());
                let change = if let Some(v) = num("fmt.scale.") {
                    Some(StyleChange::Scale(v))
                } else if let Some(v) = num("fmt.tc.") {
                    Some(StyleChange::CharSpacing(v))
                } else {
                    num("fmt.tw.").map(StyleChange::WordSpacing)
                };
                if let Some(change) = change {
                    if self.object_text_active() {
                        self.object_text_change(&change);
                    } else if let Some(v) = self.r.view_mut() {
                        self.edit.style(v, &change);
                    }
                    return true;
                }
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
        if self.object_text_active() {
            self.object_text_change(&change);
        } else if let Some(v) = self.r.view_mut() {
            self.edit.style(v, &change);
        }
        true
    }

    pub(crate) fn pane_menu(&mut self, items: Vec<(String, String, bool)>, cmd: &str) {
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
        (matches!(self.edit_tool(), Some(EditTool::Text | EditTool::Object))
            || self.comment.tool.is_some())
            && !self.format_closed
            && self.r.showing_doc()
    }

    /// Show or hide the Format pane with the Edit Text / Edit Object tools.
    pub(crate) fn sync_format(&mut self) {
        if !matches!(self.edit_tool(), Some(EditTool::Text | EditTool::Object))
            && self.comment.tool.is_none()
        {
            self.format_closed = false;
        }
        let w = if self.format_shown() { PANE_W } else { 0.0 };
        self.r.set_right_pane(w);
    }

    /// A press in the Format pane; false when it is elsewhere.
    pub(crate) fn format_input(&mut self, input: &Input) -> bool {
        if let Input::Wheel(dy) = *input {
            let pane = self.r.right_pane_rect();
            let (px, py) = self.pointer();
            if pane[2] > 0.0 && inside(pane, px, py) && self.edit_tool() == Some(EditTool::Object) {
                // The text object's sections are about 700 logical pixels tall.
                let max = (700.0 * self.ui_scale() - pane[3]).max(0.0);
                self.pane.scroll = (self.pane.scroll + dy).clamp(0.0, max);
                return true;
            }
            return false;
        }
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
            if cmd.starts_with("obj.") {
                if !self.split_command(cmd) && !self.image_command(cmd) {
                    self.arrange_command(cmd);
                }
            } else if typing
                || cmd == "fmt.close"
                || self.comment.tool.is_some()
                || (self.object_text_active()
                    && crate::editor_objtext::OBJECT_TEXT_CMDS.contains(&cmd))
            {
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
        // Header: a "Format" tab (framed, open at the bottom) with a close box, as the
        // reference's (125 %: tab 1095–1200 × 190–224, label at y 205).
        let tab = [x0 + px(6.0), y0 + px(4.0), px(84.0), px(27.0)];
        let line_y = tab[1] + tab[3];
        p.fill_rect(x0 + 1.0, line_y, tab[0] - x0 - 1.0, 1.0, LINE);
        p.fill_rect(
            tab[0] + tab[2],
            line_y,
            r[2] - (tab[0] + tab[2] - x0),
            1.0,
            LINE,
        );
        p.fill_rect(tab[0], tab[1], 1.0, tab[3], LINE);
        p.fill_rect(tab[0] + tab[2] - 1.0, tab[1], 1.0, tab[3], LINE);
        p.fill_rect(tab[0], tab[1], tab[2], 1.0, LINE);
        p.text(
            &self.t("pdfEditor.format"),
            x0 + px(17.0),
            y0 + px(10.0),
            style(17.0),
            accent_text,
        );
        let cx = x0 + px(72.0);
        let cy = y0 + px(14.0);
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
        if self.comment.tool.is_some() && self.edit_tool() != Some(EditTool::Text) {
            self.paint_comment_props(p, x0, y0);
            return;
        }
        // Edit Object: a text object shows Text Style, Split and Arrange (the pane scrolls),
        // other objects only Arrange, as the reference's.
        let object = self.edit_tool() == Some(EditTool::Object);
        let text_object = object
            && self
                .edit
                .selected_kinds()
                .contains(&bo_pdfread::ObjectKind::Text);
        if object && !text_object {
            // Arrange, then Effect for images and other graphics (the pane scrolls).
            let graphics = self
                .edit
                .selected_kinds()
                .iter()
                .any(|k| *k != bo_pdfread::ObjectKind::Text);
            p.push_clip(r[0], y0 + px(36.0), r[2], r[3] - px(36.0));
            let y = y0 - self.pane.scroll;
            self.paint_arrange(p, x0, y + px(46.0));
            // The reference shows Effect (grey) with nothing selected too.
            if graphics || self.edit.selected.is_empty() {
                self.paint_effect(p, x0, y + px(370.0));
            }
            p.pop_clip();
            let top = r[1] + px(36.0);
            self.pane
                .hits
                .retain(|h| h.1 == "fmt.close" || h.0[1] >= top);
            return;
        }
        let y0 = if object {
            p.push_clip(r[0], y0 + px(36.0), r[2], r[3] - px(36.0));
            y0 - self.pane.scroll
        } else {
            y0
        };
        // "Text Style" section.
        let ty = y0 + px(46.0);
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
        if object {
            // The reference's section heading with its rule.
            self.section_heading(p, x0, ty, "pdfEditor.textStyle");
        } else {
            p.text(
                &self.t("pdfEditor.textStyle"),
                x0 + px(27.0),
                ty,
                style(17.0),
                TEXT,
            );
        }
        let typing = self.edit.typing.as_ref().is_some_and(|t| t.block.is_some());
        let cur = if object {
            self.object_text_style()
        } else {
            self.edit.current_style()
        };
        // Edit Object offers only some controls on a text object, as the reference.
        let live = |cmd: &str| !object || crate::editor_objtext::OBJECT_TEXT_CMDS.contains(&cmd);
        let grey = crate::editor_objtext::OFF;
        let ink = if typing || (object && cur.is_some()) {
            Rgba(0, 0, 0, 255)
        } else {
            MUTED
        };
        // Font and size boxes, colour swatch.
        let by = y0 + px(73.0);
        let bh = px(24.0);
        let boxes = [
            (
                "fmt.font",
                28.0,
                156.0,
                cur.as_ref().map(|(s, _)| s.family.clone()),
            ),
            (
                "fmt.size",
                192.0,
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
        let sw = [x0 + px(268.0), by + px(1.0), px(22.0), px(22.0)];
        let col = cur.as_ref().map_or(Rgba(0, 0, 0, 255), |(s, _)| {
            Rgba(s.color[0], s.color[1], s.color[2], 255)
        });
        p.fill_rect(sw[0], sw[1], sw[2], sw[3], col);
        frame(p, sw, EDGE);
        self.pane.hits.push((sw, "fmt.color"));
        // Character and paragraph rows (plain ink, as the reference draws them).
        let script = cur.as_ref().map_or(0, |(s, _)| s.script);
        let on = |cmd: &str| -> bool {
            cur.as_ref().is_some_and(|(s, a)| match cmd {
                "fmt.bold" => s.bold,
                "fmt.italic" => s.italic,
                "fmt.underline" => s.underline,
                "fmt.strike" => s.strike,
                "fmt.super" => script > 0,
                "fmt.sub" => script < 0,
                "fmt.alignLeft" => *a == Align::Left,
                "fmt.alignCenter" => *a == Align::Center,
                "fmt.alignRight" => *a == Align::Right,
                "fmt.justify" => *a == Align::Justify,
                _ => false,
            })
        };
        let _ = (accent, ink);
        for (row, items) in [(131.0, &CHAR_ROW[..]), (178.0, &ALIGN_ROW[..])] {
            for (i, (cmd, icon)) in items.iter().enumerate() {
                let (cx, cy) = (x0 + px(button_xy(i)), y0 + px(row));
                let s = px(28.0);
                let b = [cx - s / 2.0, cy - s / 2.0, s, s];
                if on(cmd) {
                    p.fill_rect(
                        b[0],
                        b[1],
                        b[2],
                        b[3],
                        if live(cmd) {
                            PRESSED
                        } else {
                            Rgba(244, 244, 244, 255)
                        },
                    );
                }
                icon_at(
                    p,
                    icon,
                    cx,
                    cy,
                    px(20.0),
                    if live(cmd) { TEXT } else { grey },
                );
                if live(cmd) {
                    self.pane.hits.push((b, cmd));
                }
            }
        }
        // Bullets, numbering (menus) and baseline (not offered: grey, as the reference).
        for (i, (cmd, icon, menu, color)) in [
            ("fmt.bullets", "bullets", true, TEXT),
            ("fmt.numbering", "numbering", true, TEXT),
            ("fmt.baseline", "fmtBaseline", true, MUTED),
        ]
        .into_iter()
        .enumerate()
        {
            let color = if object { grey } else { color };
            let (cx, cy) = (
                x0 + px(button_xy(i) + 6.0 + 10.0 * i as f32),
                y0 + px(220.0),
            );
            icon_at(p, icon, cx, cy, px(20.0), color);
            if menu {
                arrow(p, cx + px(14.0), cy - px(1.0), px(6.0), color);
            }
            if color == TEXT {
                let s = px(28.0);
                self.pane
                    .hits
                    .push(([cx - s / 2.0, cy - s / 2.0, s + px(12.0), s], cmd));
            }
        }
        // Spacing: line and paragraph spacing, character scale, character and word spacing.
        let blk = self.edit.block();
        let zero = (object && cur.is_some()).then(|| "0.00".to_owned());
        let pitch = blk.map(|b| format!("{:.2}", b.line_gap())).or(zero.clone());
        let para = self.para_spacing().map(|v| format!("{v:.2}")).or(zero);
        let fields = [
            ("fmt.lineSp", "lineSpacing", 42.0, 61.0, 270.0, pitch),
            ("fmt.paraSp", "paraSpacing", 171.0, 199.0, 270.0, para),
            (
                "fmt.scale",
                "charScale",
                42.0,
                61.0,
                308.0,
                cur.as_ref().map(|(s, _)| format!("{}%", s.scale().round())),
            ),
            (
                "fmt.charSp",
                "charSpacing",
                171.0,
                199.0,
                308.0,
                cur.as_ref()
                    .map(|(s, _)| format!("{:.2}", s.char_spacing())),
            ),
            (
                "fmt.wordSp",
                "wordSpacing",
                42.0,
                61.0,
                346.0,
                cur.as_ref()
                    .map(|(s, _)| format!("{:.2}", s.word_spacing())),
            ),
        ];
        for (cmd, icon, ix, bx, y, value) in fields {
            let cy = y0 + px(y);
            let live = live(cmd);
            icon_at(
                p,
                icon,
                x0 + px(ix),
                cy,
                px(20.0),
                if live { TEXT } else { grey },
            );
            let r = [x0 + px(bx), cy - px(12.0), px(76.0), px(24.0)];
            frame(p, r, EDGE);
            // Values of controls not offered show grey (reference: #6D6D6D, arrow #B1B1B1).
            let col = if live { ink } else { Rgba(109, 109, 109, 255) };
            if let Some(v) = value {
                p.text(&v, r[0] + px(6.0), r[1] + px(4.0), style(16.0), col);
            }
            arrow(
                p,
                r[0] + r[2] - px(14.0),
                r[1] + px(10.0),
                px(7.0),
                if live { col } else { grey },
            );
            if live {
                self.pane.hits.push((r, cmd));
            }
        }
        if object {
            // 125 %: Split at +467, Arrange at +566 from the pane top.
            self.paint_split_live(p, x0, y0 + px(374.0));
            self.paint_arrange(p, x0, y0 + px(453.0));
            p.pop_clip();
            // Hit boxes scrolled under the header are not pressable.
            let top = r[1] + px(36.0);
            self.pane
                .hits
                .retain(|h| h.1 == "fmt.close" || h.0[1] >= top);
        }
    }
}

/// Icon `name` centred on (`cx`, `cy`) in one ink colour.
fn icon_at(p: &mut dyn Painter, name: &str, cx: f32, cy: f32, size: f32, ink: Rgba) {
    // Thin 1 px lines as the reference's pane icons.
    reader_icons::draw_weight(
        p,
        name,
        cx - size / 2.0,
        cy - size / 2.0,
        size,
        Ink { ink, accent: ink },
        0.9,
    );
}

/// A small drop-down triangle with its top-left at (`x`, `y`).
fn arrow(p: &mut dyn Painter, x: f32, y: f32, w: f32, c: Rgba) {
    p.path(
        &[
            PathSeg::M(x, y),
            PathSeg::L(x + w, y),
            PathSeg::L(x + w / 2.0, y + w * 0.57),
            PathSeg::Z,
        ],
        Some(c),
        None,
    );
}

impl EditorWindow {
    /// Space between the paragraph being typed into and the next one below it, as the
    /// reference shows it: the gap between their boxes less the line gap.
    fn para_spacing(&mut self) -> Option<f32> {
        let t = self.edit.typing.as_ref()?;
        let b = t.block.as_ref()?;
        let (page, bottom, top0, gap) = (t.page, b.bbox[3], b.bbox[1], b.line_gap());
        let key = (page, top0.to_bits(), bottom.to_bits());
        if let Some((k, v)) = self.pane.para {
            if k == key {
                return v;
            }
        }
        let v = self.r.view_mut()?;
        let objs = v.doc_mut().page_objects(page).ok()?;
        let next = bo_pdfread::text_blocks(&objs)
            .into_iter()
            .filter(|o| o.bbox[1] > top0 + 1.0 && o.bbox[1] >= bottom - 2.0)
            .map(|o| o.bbox[1])
            .fold(f32::INFINITY, f32::min);
        let r = next.is_finite().then(|| (next - bottom - gap).max(0.0));
        self.pane.para = Some((key, r));
        r
    }

    /// Bullets or numbers at the start of each paragraph of the block (`b0`…, `n0`…, `none`).
    fn apply_list(&mut self, kind: &str) {
        const MARKS: [&str; 4] = ["\u{2022}", "\u{2013}", "\u{2713}", "\u{25E6}"];
        let Some(v) = self.r.view_mut() else { return };
        self.edit.with_block(v, |b, caret| {
            // Paragraph starts: 0 and after each typed break.
            let mut starts = vec![0];
            starts.extend(
                (0..b.text.len())
                    .filter(|&i| b.text[i] == '\n')
                    .map(|i| i + 1),
            );
            for (n, &st) in starts.iter().enumerate().rev() {
                // Drop a mark this tool wrote before ("x " or "1. " / "a) " / "I. ").
                let end = (st..b.text.len()).find(|&i| b.text[i] == ' ').unwrap_or(st);
                let head: String = b.text[st..end].iter().collect();
                let is_mark = MARKS.contains(&head.as_str())
                    || (end > st
                        && end - st <= 5
                        && (head.ends_with('.') || head.ends_with(')'))
                        && head[..head.len() - 1].chars().all(char::is_alphanumeric));
                if is_mark && end < b.text.len() {
                    b.remove(st, end + 1);
                    if *caret > st {
                        *caret = caret.saturating_sub(end + 1 - st).max(st);
                    }
                }
                let mark = match kind.as_bytes() {
                    [b'b', d] => MARKS.get(usize::from(d - b'0')).map(|m| (*m).to_owned()),
                    [b'n', b'0'] => Some(format!("{}.", n + 1)),
                    [b'n', b'1'] => Some(format!("{})", char::from(b'a' + (n % 26) as u8))),
                    [b'n', b'2'] => Some(format!("{}.", roman(n + 1))),
                    _ => None,
                };
                if let Some(m) = mark {
                    let k = b.insert(st, &format!("{m} "));
                    if *caret >= st {
                        *caret += k - st;
                    }
                }
            }
        });
    }

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

/// Upper-case Roman numeral of `n` (1–3999).
fn roman(mut n: usize) -> String {
    const R: [(usize, &str); 13] = [
        (1000, "M"),
        (900, "CM"),
        (500, "D"),
        (400, "CD"),
        (100, "C"),
        (90, "XC"),
        (50, "L"),
        (40, "XL"),
        (10, "X"),
        (9, "IX"),
        (5, "V"),
        (4, "IV"),
        (1, "I"),
    ];
    let mut s = String::new();
    for (v, r) in R {
        while n >= v {
            s.push_str(r);
            n -= v;
        }
    }
    s
}
