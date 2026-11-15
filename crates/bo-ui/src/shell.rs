//! The BlueDoc window: ribbon, document view and status bar, with input routing.
use crate::backstage::{Backstage, FileAction};
use crate::catalog;
use crate::dialog::{Dialog, DialogButton, DialogKind};
use crate::doc_view::DocView;
use crate::icons::Icons;
use crate::popup::{paint_tooltip, Choice, Menu, MenuItem};
use crate::ribbon::{Ribbon, RibbonHit};
use crate::ruler::Ruler;
use crate::strings::Strings;
use crate::theme::Theme;
use crate::titlebar::{NcHit, TitleBar};
use crate::{commands, Key, Mods};
use crate::{Input, PageStore, Painter, TextMeasure, TextStyle};
use bo_edit::CharFormat;
use bo_engine::edit::Command;
use bo_render::{PathSeg, Rgba};
use serde_json::json;

/// Something only the platform host can do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Request {
    /// Write the document to its file.
    Save,
    /// Put these forms on the system clipboard.
    SetClipboard {
        /// Plain text.
        text: String,
        /// HTML fragment.
        html: String,
        /// BlueOffice internal form (lossless within the suite).
        internal: String,
    },
    /// Read the system clipboard and call [`DocWindow::paste`].
    Paste,
    /// Let the user pick a mail-merge data file and call [`DocWindow::merge_data`].
    PickMergeData,
    /// New blank document (new window).
    New,
    /// Pick a file and open it (new window).
    Open,
    /// Pick a name and save the document there, then [`DocWindow::set_name`].
    SaveAs,
    /// Print the document.
    Print,
    /// Pick a name and export the document as PDF.
    ExportPdf,
    /// Minimise the window.
    Minimize,
    /// Maximise or restore the window.
    ToggleMaximize,
    /// Close the window.
    Close,
    /// Cover the whole screen without window frame (slide show), or go back.
    FullScreen(bool),
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
    /// Quick styles of the gallery.
    styles: Vec<bo_engine::gallery::StyleEntry>,
    /// Status bar zoom controls: minus, slider track, plus, percentage.
    zoom_rects: [[f32; 4]; 4],
    /// Status bar view buttons (print layout, web layout, draft), as in a desktop word
    /// processor's status bar.
    view_rects: [[f32; 4]; 3],
    /// Current view layout (`print`, `web`, `draft`).
    view_mode: &'static str,
    /// Vertical scroll bar drag: pointer offset from the thumb top.
    sb_drag: Option<f32>,
    /// The pointer is dragging the zoom slider.
    zoom_drag: bool,
    dialog: Option<Dialog>,
    ruler: Ruler,
    show_ruler: bool,
    ruler_drag: bool,
    marks: bool,
    /// Format painter armed: the next selection gets the picked formatting.
    painter: bool,
    /// Mail-merge preview record, when previewing.
    merge_record: Option<usize>,
    /// Find state: query, matches and the current one.
    find: Option<(String, Vec<bo_edit::find::Match>, usize)>,
    /// Layout must be redone at the next input (needs text measurement).
    relayout_pending: bool,
    /// Last pointer position of a drag in the document (auto-scroll ticks).
    last_drag: Option<(f32, f32)>,
    /// The File screen, when open.
    backstage: Option<Backstage>,
}

/// Font sizes offered by the size box (points).
const SIZES: [f32; 18] = [
    8.0, 9.0, 10.0, 10.5, 11.0, 12.0, 13.0, 14.0, 16.0, 18.0, 20.0, 22.0, 24.0, 26.0, 28.0, 36.0,
    48.0, 72.0,
];

/// Next standard font size, in half-points, matching the shared editor's size steps.
/// Points from twips, without trailing zeros (`12`, `7,5`).
fn fmt_pt(twips: i32) -> String {
    let pt = f64::from(twips) / 20.0;
    let s = format!("{pt:.1}");
    s.trim_end_matches('0')
        .trim_end_matches('.')
        .replace('.', ",")
}

/// A number typed with a comma or a point as decimal separator.
fn parse_num(s: &str) -> Option<f64> {
    let t = s.trim().replace(',', ".");
    if t.is_empty() {
        return None;
    }
    t.parse::<f64>().ok().filter(|v| v.is_finite())
}

/// Status bar view buttons: view layout and its icon (Bluestar's own drawings).
const VIEW_MODES: [(&str, &str); 3] = [
    ("print", "layout.margins"),
    ("web", "view.normal"),
    ("draft", "format.justify"),
];

fn step_font_size(current: u32, grow: bool) -> u32 {
    if grow {
        if current >= 144 {
            return (current / 20)
                .saturating_add(1)
                .saturating_mul(20)
                .min(3276);
        }
        SIZES
            .iter()
            .map(|s| (s * 2.0) as u32)
            .find(|&s| s > current)
            .unwrap_or(144)
    } else {
        if current > 144 {
            return current
                .div_ceil(20)
                .saturating_sub(1)
                .saturating_mul(20)
                .max(144);
        }
        if current <= 16 {
            return current
                .div_ceil(2)
                .saturating_sub(1)
                .saturating_mul(2)
                .max(2);
        }
        SIZES
            .iter()
            .rev()
            .map(|s| (s * 2.0) as u32)
            .find(|&s| s < current)
            .unwrap_or(16)
    }
}
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
            styles: Vec::new(),
            zoom_rects: [[0.0; 4]; 4],
            view_rects: [[0.0; 4]; 3],
            view_mode: "print",
            sb_drag: None,
            zoom_drag: false,
            dialog: None,
            ruler: Ruler::default(),
            show_ruler: true,
            ruler_drag: false,
            marks: false,
            painter: false,
            merge_record: None,
            find: None,
            relayout_pending: false,
            last_drag: None,
            backstage: None,
        }
    }

    /// The document now has this file name (after Save As).
    pub fn set_name(&mut self, name: &str) {
        self.name = name.to_owned();
        self.relayout_pending = true;
    }

    fn file_action(&mut self, a: FileAction) -> bool {
        if a != FileAction::Back {
            let r = match a {
                FileAction::New => Request::New,
                FileAction::Open => Request::Open,
                FileAction::Save => Request::Save,
                FileAction::SaveAs => Request::SaveAs,
                FileAction::Print => Request::Print,
                FileAction::ExportPdf => Request::ExportPdf,
                _ => Request::Close,
            };
            self.requests.push(r);
        }
        self.backstage = None;
        true
    }

    /// Mail-merge data the host read (`name` with its extension, bytes in base64).
    pub fn merge_data(&mut self, name: &str, base64: &str) -> bool {
        catalog::from_json(json!({ "cmd": "mergeData", "name": name, "base64": base64 }))
            .is_some_and(|c| self.view.run_command(c))
    }

    /// Text of `key`, or `fallback` when the locale has no such key.
    fn t(&self, key: &str, fallback: &str) -> String {
        let v = self.strings.get(key);
        if v == key {
            fallback.to_owned()
        } else {
            v.to_owned()
        }
    }

    fn open_dialog(&mut self, kind: DialogKind) -> bool {
        let sel = self.find.as_ref().map(|f| f.0.clone()).unwrap_or_default();
        let ok = self.t("common.ok", "OK");
        let cancel = self.t("common.cancel", "Cancel");
        let (title, fields, buttons, message) = match kind {
            DialogKind::Find => (
                self.t("commands.edit.find", "Find"),
                vec![(self.t("find.findWhat", "Find what"), sel)],
                vec![
                    (self.t("find.next", "Find Next"), DialogButton::FindNext),
                    (cancel, DialogButton::Cancel),
                ],
                vec![],
            ),
            DialogKind::Replace => (
                self.t("commands.edit.replace", "Replace"),
                vec![
                    (self.t("find.findWhat", "Find what"), sel),
                    (self.t("find.replaceWith", "Replace with"), String::new()),
                ],
                vec![
                    (self.t("find.next", "Find Next"), DialogButton::FindNext),
                    (self.t("find.replace", "Replace"), DialogButton::Replace),
                    (
                        self.t("find.replaceAll", "Replace All"),
                        DialogButton::ReplaceAll,
                    ),
                    (cancel, DialogButton::Cancel),
                ],
                vec![],
            ),
            DialogKind::Link => (
                self.t("commands.insert.link", "Insert link"),
                vec![
                    (self.t("link.address", "Address"), "https://".into()),
                    (self.t("link.display", "Text to display"), String::new()),
                ],
                vec![(ok, DialogButton::Ok), (cancel, DialogButton::Cancel)],
                vec![],
            ),
            DialogKind::Font => {
                let f = self.view.format();
                let size = f.size_half_points.map_or_else(String::new, |h| {
                    if h % 2 == 0 {
                        (h / 2).to_string()
                    } else {
                        format!("{},5", h / 2)
                    }
                });
                (
                    self.t("docStyles.dialog.font", "Font"),
                    vec![
                        (
                            self.t("docStyles.dialog.font", "Font"),
                            f.font.clone().unwrap_or_default(),
                        ),
                        (self.t("docStyles.dialog.size", "Size (pt)"), size),
                        (
                            self.t("docStyles.dialog.color", "Colour"),
                            f.color.clone().unwrap_or_default(),
                        ),
                    ],
                    vec![(ok, DialogButton::Ok), (cancel, DialogButton::Cancel)],
                    vec![],
                )
            }
            DialogKind::Paragraph => {
                let f = self.view.format();
                let lines = f64::from(f.line_spacing.value) / 240.0;
                let lines = format!("{lines:.2}")
                    .trim_end_matches('0')
                    .trim_end_matches('.')
                    .replace('.', ",");
                (
                    self.t("docStyles.dialog.typeParagraph", "Paragraph"),
                    vec![
                        (
                            self.t("docStyles.dialog.indentLeft", "Left indent (pt)"),
                            fmt_pt(f.indent_left),
                        ),
                        (
                            self.t("docStyles.dialog.indentRight", "Right indent (pt)"),
                            fmt_pt(f.indent_right),
                        ),
                        (
                            self.t("docStyles.dialog.indentFirstLine", "First line (pt)"),
                            fmt_pt(f.first_line),
                        ),
                        (
                            self.t("docStyles.dialog.spaceBefore", "Space before (pt)"),
                            fmt_pt(f.space_before),
                        ),
                        (
                            self.t("docStyles.dialog.spaceAfter", "Space after (pt)"),
                            fmt_pt(f.space_after),
                        ),
                        (
                            self.t("docStyles.dialog.lineSpacing", "Line spacing (lines)"),
                            lines,
                        ),
                    ],
                    vec![(ok, DialogButton::Ok), (cancel, DialogButton::Cancel)],
                    vec![],
                )
            }
            DialogKind::Comment => (
                self.t("commands.review.newComment", "New comment"),
                vec![(self.t("comments.text", "Comment"), String::new())],
                vec![(ok, DialogButton::Ok), (cancel, DialogButton::Cancel)],
                vec![],
            ),
            DialogKind::WordCount => (
                self.t("commands.review.wordCount", "Word Count"),
                vec![],
                vec![(self.t("common.close", "Close"), DialogButton::Cancel)],
                vec![
                    format!(
                        "{}: {}",
                        self.t("wordCount.pages", "Pages"),
                        self.view.page_count()
                    ),
                    format!(
                        "{}: {}",
                        self.t("wordCount.words", "Words"),
                        self.view.word_count()
                    ),
                ],
            ),
        };
        self.menu = None;
        let mut dialog = Dialog::new(kind, title, fields, buttons, message);
        if kind == DialogKind::Font {
            // Style check boxes and a preview, as in a desktop word processor's Font dialog.
            let f = self.view.format();
            let checks = vec![
                (self.t("docStyles.dialog.bold", "Bold"), f.bold),
                (self.t("docStyles.dialog.italic", "Italic"), f.italic),
                (
                    self.t("docStyles.dialog.underline", "Underline"),
                    f.underline,
                ),
            ];
            dialog = dialog.with_checks(checks);
        }
        self.dialog = Some(dialog);
        self.relayout_pending = true;
        true
    }

    fn dialog_button(&mut self, b: DialogButton) {
        let Some(d) = self.dialog.as_ref() else {
            return;
        };
        let kind = d.kind;
        let values = d.values();
        let v = |i: usize| values.get(i).cloned().unwrap_or_default();
        match (kind, b) {
            (_, DialogButton::Cancel) | (DialogKind::WordCount, _) => self.dialog = None,
            (DialogKind::Link, _) => {
                self.dialog = None;
                self.view.run_command(Command::SetLink {
                    target: v(0),
                    display: v(1),
                });
            }
            (DialogKind::Font, DialogButton::Ok) => {
                self.dialog = None;
                let f = self.view.format().clone();
                let (name, size, color) = (v(0), v(1), v(2));
                let name = name.trim();
                if !name.is_empty() && f.font.as_deref() != Some(name) {
                    self.view.run_command(Command::FormatText {
                        format: CharFormat::Font { name: name.into() },
                    });
                }
                if let Some(pt) = parse_num(&size) {
                    let half_points = (pt * 2.0).round().clamp(2.0, 3276.0) as u32;
                    if f.size_half_points != Some(half_points) {
                        self.view.run_command(Command::FormatText {
                            format: CharFormat::Size { half_points },
                        });
                    }
                }
                let hex = color.trim().trim_start_matches('#').to_uppercase();
                let hex =
                    (hex.len() == 6 && hex.chars().all(|c| c.is_ascii_hexdigit())).then_some(hex);
                if hex != f.color && (hex.is_some() || color.trim().is_empty()) {
                    self.view.run_command(Command::FormatText {
                        format: CharFormat::Color { hex },
                    });
                }
                // Bold, italic and underline toggle when their box changed.
                for (i, on, format) in [
                    (3, f.bold, CharFormat::Bold),
                    (4, f.italic, CharFormat::Italic),
                    (5, f.underline, CharFormat::Underline),
                ] {
                    if (v(i) == "1") != on {
                        self.view.run_command(Command::FormatText { format });
                    }
                }
            }
            (DialogKind::Paragraph, DialogButton::Ok) => {
                self.dialog = None;
                let f = self.view.format().clone();
                let twips = |s: &str| parse_num(s).map(|pt| (pt * 20.0).round() as i32);
                let (left, right, first) = (
                    twips(&v(0)).unwrap_or(f.indent_left),
                    twips(&v(1)).unwrap_or(f.indent_right),
                    twips(&v(2)).unwrap_or(f.first_line),
                );
                if (left, right, first) != (f.indent_left, f.indent_right, f.first_line) {
                    self.view.run_command(Command::FormatParagraph {
                        format: bo_edit::ParaFormat::Indents {
                            left,
                            right,
                            first_line: first,
                        },
                    });
                }
                if let Some(t) = twips(&v(3)).filter(|t| *t != f.space_before) {
                    self.view.run_command(Command::FormatParagraph {
                        format: bo_edit::ParaFormat::SpaceBefore { twips: t.max(0) },
                    });
                }
                if let Some(t) = twips(&v(4)).filter(|t| *t != f.space_after) {
                    self.view.run_command(Command::FormatParagraph {
                        format: bo_edit::ParaFormat::SpaceAfter { twips: t.max(0) },
                    });
                }
                if let Some(lines) = parse_num(&v(5)) {
                    let value = (lines * 240.0).round().clamp(60.0, 5000.0) as u32;
                    if value != f.line_spacing.value {
                        self.view.run_command(Command::FormatParagraph {
                            format: bo_edit::ParaFormat::LineSpacing { value },
                        });
                    }
                }
            }
            (DialogKind::Font | DialogKind::Paragraph, _) => self.dialog = None,
            (DialogKind::Comment, _) => {
                self.dialog = None;
                if let Some(c) = catalog::from_json(json!({ "cmd": "addComment", "text": v(0) })) {
                    self.view.run_command(c);
                }
            }
            (_, DialogButton::FindNext | DialogButton::Ok) => self.find_next(&v(0)),
            (_, DialogButton::Replace) => {
                let (query, with) = (v(0), v(1));
                if let Some((q, matches, i)) = self.find.clone().filter(|f| f.0 == query) {
                    if let Some(m) = matches.get(i) {
                        let cmd = catalog::from_json(json!({
                            "cmd": "replace", "match": m, "with": with, "query": q,
                        }));
                        if let Some(c) = cmd {
                            self.view.run_command(c);
                        }
                    }
                    self.find = None;
                }
                self.find_next(&query);
            }
            (_, DialogButton::ReplaceAll) => {
                let cmd =
                    catalog::from_json(json!({ "cmd": "replaceAll", "query": v(0), "with": v(1) }));
                let n = cmd
                    .and_then(|c| self.view.exec(c))
                    .and_then(|r| r.count)
                    .unwrap_or(0);
                self.find = None;
                let msg = format!("{n} {}", self.t("find.replacements", "replacements made"));
                if let Some(d) = self.dialog.as_mut() {
                    d.set_message(vec![msg]);
                }
            }
        }
    }

    fn find_next(&mut self, query: &str) {
        if query.is_empty() {
            return;
        }
        let fresh = self.find.as_ref().is_none_or(|f| f.0 != query);
        if fresh {
            let matches = catalog::from_json(json!({ "cmd": "find", "query": query }))
                .and_then(|c| self.view.exec(c))
                .and_then(|r| r.matches)
                .unwrap_or_default();
            self.find = Some((query.to_owned(), matches, usize::MAX));
        }
        let msg = match self.find.as_mut() {
            Some((_, matches, i)) if !matches.is_empty() => {
                *i = if *i == usize::MAX {
                    0
                } else {
                    (*i + 1) % matches.len()
                };
                let (n, total, m) = (*i + 1, matches.len(), matches[*i]);
                if let Some(c) = catalog::from_json(json!({ "cmd": "selectMatch", "match": m })) {
                    self.view.run_command(c);
                }
                format!("{n} / {total}")
            }
            _ => self.t("find.none", "No matches"),
        };
        if let Some(d) = self.dialog.as_mut() {
            d.set_message(vec![msg]);
        }
    }

    fn open_command_menu(&mut self, id: &str, anchor: [f32; 4]) {
        let spacing = self.view.format().line_spacing;
        let (before, after) = (
            self.view.format().space_before,
            self.view.format().space_after,
        );
        let items = catalog::menu(id)
            .into_iter()
            .map(|e| MenuItem {
                label: match e.id {
                    "format.spaceBefore.toggle" if before > 0 => {
                        "Remove Space Before Paragraph".to_owned()
                    }
                    "format.spaceAfter.toggle" if after > 0 => {
                        "Remove Space After Paragraph".to_owned()
                    }
                    _ if e.label.contains('.')
                        && !e.label.contains(' ')
                        && e.label.chars().any(char::is_alphabetic) =>
                    {
                        self.strings.get(e.label).to_owned()
                    }
                    _ => e.label.to_owned(),
                },
                choice: Choice::Command(e.id.to_owned()),
                checked: spacing.rule == bo_edit::LineRule::Auto
                    && match e.id {
                        "format.lineSpacing1" => spacing.value == 240,
                        "format.lineSpacing115" => spacing.value == 276,
                        "format.lineSpacing15" => spacing.value == 360,
                        "format.lineSpacing2" => spacing.value == 480,
                        "format.lineSpacing25" => spacing.value == 600,
                        "format.lineSpacing3" => spacing.value == 720,
                        _ => false,
                    },
                dim: false,
            })
            .collect();
        self.menu = Some(Menu::open(items, anchor, self.size, self.scale));
    }

    fn context_menu(&mut self, x: f32, y: f32) {
        // Clipboard, then insert commands behind a separator, with icons; cut and copy dim
        // without a selection, as in a desktop word processor's context menu.
        let selected = self.view.has_selection();
        let ids = [
            ("edit.cut", "edit.cut"),
            ("edit.copy", "edit.copy"),
            ("edit.paste", "edit.paste"),
            ("insert.link", "insert.link"),
            ("review.newComment", "review.newComment"),
        ];
        let items = ids
            .iter()
            .map(|(id, _)| MenuItem {
                label: self.strings.get(&format!("commands.{id}")).to_owned(),
                choice: Choice::Command((*id).to_owned()),
                checked: false,
                dim: !selected && matches!(*id, "edit.cut" | "edit.copy"),
            })
            .collect();
        let icons = ids.iter().map(|(_, icon)| Some(*icon)).collect();
        self.menu = Some(
            Menu::open(items, [x, y - 2.0, 1.0, 0.0], self.size, self.scale)
                .with_decor(icons, vec![3]),
        );
    }

    /// Ids drawn as switched on beyond the formatting state.
    fn on_ids(&self) -> Vec<&'static str> {
        let mut on = Vec::new();
        if self.show_ruler {
            on.push("view.ruler");
        }
        if self.marks {
            on.push("view.formattingMarks");
        }
        if self.painter {
            on.push("format.painter");
        }
        if self.view.track_changes() {
            on.push("review.trackChanges");
        }
        if self.merge_record.is_some() {
            on.push("mailings.preview");
        }
        on
    }

    /// Paste content the host read from the system clipboard.
    pub fn paste(
        &mut self,
        internal: Option<String>,
        html: Option<String>,
        text: Option<String>,
    ) -> bool {
        self.view.paste(internal, html, text)
    }

    fn clipboard(&mut self, id: &str) -> bool {
        match id {
            "edit.copy" | "edit.cut" => {
                let cut = id == "edit.cut";
                if let Some(c) = self.view.copy(cut) {
                    self.requests.push(Request::SetClipboard {
                        text: c.text,
                        html: c.html,
                        internal: c.internal,
                    });
                }
                cut
            }
            _ => {
                self.requests.push(Request::Paste);
                false
            }
        }
    }

    /// Run interface command `id` from the ribbon, title bar, a menu or a shortcut.
    fn action(&mut self, id: &str) -> bool {
        // Space before / after toggles: remove when the paragraph has some, else 12 pt.
        let toggle = match id {
            "format.spaceBefore.toggle" => Some((true, self.view.format().space_before)),
            "format.spaceAfter.toggle" => Some((false, self.view.format().space_after)),
            _ => None,
        };
        if let Some((before, current)) = toggle {
            let twips = if current > 0 { 0 } else { 240 };
            let format = if before {
                bo_edit::ParaFormat::SpaceBefore { twips }
            } else {
                bo_edit::ParaFormat::SpaceAfter { twips }
            };
            return self.view.run_command(Command::FormatParagraph { format });
        }
        if let Some(mode) = id.strip_prefix("view.layout.") {
            self.view_mode = VIEW_MODES
                .iter()
                .find(|(m, _)| *m == mode)
                .map_or("print", |(m, _)| m);
        }
        let merge = |w: &mut Self, record: Option<usize>| {
            w.merge_record = record;
            catalog::from_json(json!({ "cmd": "mergePreview", "record": record }))
                .is_some_and(|c| w.view.run_command(c))
        };
        match id {
            "file.save" => {
                self.requests.push(Request::Save);
                true
            }
            "edit.copy" | "edit.cut" | "edit.paste" => self.clipboard(id),
            "edit.find" => self.open_dialog(DialogKind::Find),
            "edit.replace" => self.open_dialog(DialogKind::Replace),
            "insert.link" => self.open_dialog(DialogKind::Link),
            "format.fontSettings" => self.open_dialog(DialogKind::Font),
            "format.paragraphSettings" => self.open_dialog(DialogKind::Paragraph),
            "review.newComment" => self.open_dialog(DialogKind::Comment),
            "review.wordCount" => self.open_dialog(DialogKind::WordCount),
            "review.deleteComment" => match self.view.comment_at_caret() {
                Some(id) => catalog::from_json(json!({ "cmd": "deleteComment", "id": id }))
                    .is_some_and(|c| self.view.run_command(c)),
                None => false,
            },
            "review.trackChanges" => {
                let on = !self.view.track_changes();
                self.view.run_command(Command::SetTrackChanges { on })
            }
            "view.ruler" => {
                self.show_ruler = !self.show_ruler;
                self.relayout_pending = true;
                true
            }
            "view.formattingMarks" => {
                self.marks = !self.marks;
                let on = self.marks;
                self.view.run_command(Command::ShowFormattingMarks { on });
                true
            }
            "view.zoomIn" | "view.zoomOut" | "view.zoomReset" => {
                let z = self.view.zoom();
                let z = match id {
                    "view.zoomIn" => ((z + 0.1) * 10.0).round() / 10.0,
                    "view.zoomOut" => ((z - 0.1) * 10.0).round() / 10.0,
                    _ => 1.0,
                };
                self.relayout_pending = true;
                self.view.set_zoom(z)
            }
            "format.painter" => {
                self.painter = self.view.run_command(Command::PickFormat);
                true
            }
            "format.growStep" | "format.shrinkStep" => {
                let current = self.view.format().size_half_points.unwrap_or(26);
                let half_points = step_font_size(current, id == "format.growStep");
                self.view.run_command(Command::FormatText {
                    format: CharFormat::Size { half_points },
                })
            }
            "mailings.recipients" => {
                self.requests.push(Request::PickMergeData);
                false
            }
            "mailings.preview" => {
                let record = if self.merge_record.is_some() {
                    None
                } else {
                    Some(0)
                };
                merge(self, record)
            }
            "mailings.nextRecord" => {
                let r = self.merge_record.map_or(0, |r| r + 1);
                merge(self, Some(r))
            }
            "mailings.previousRecord" => {
                let r = self.merge_record.map_or(0, |r| r.saturating_sub(1));
                merge(self, Some(r))
            }
            "mailings.finish" => self.view.run_command(Command::MergeToDocument),
            id if catalog::is_menu(id) => {
                let anchor = self
                    .ribbon
                    .rect_of(RibbonHit::Command(commands::static_id(id)))
                    .unwrap_or([0.0, 0.0, 1.0, 1.0]);
                self.open_command_menu(id, anchor);
                true
            }
            _ => match catalog::command(id, &self.view.format().section) {
                Some(cmd) => self.view.run_command(cmd),
                None => self.view.run_action(id),
            },
        }
    }

    fn zoom_to(&mut self, zoom: f32, text: &dyn TextMeasure) -> bool {
        let changed = self.view.set_zoom(zoom);
        if changed {
            self.relayout(text);
        }
        changed
    }

    /// Zoom of a slider position: the left half covers 10–100 %, the right half 100–500 %.
    fn slider_zoom(&self, x: f32) -> f32 {
        let [sx, _, sw, _] = self.zoom_rects[1];
        let t = ((x - sx) / sw).clamp(0.0, 1.0);
        let z = if t < 0.5 {
            0.1 + t / 0.5 * 0.9
        } else {
            1.0 + (t - 0.5) / 0.5 * 4.0
        };
        // Snap to 100 % near the middle, else to whole percents.
        if (z - 1.0).abs() < 0.04 {
            1.0
        } else {
            (z * 100.0).round() / 100.0
        }
    }

    fn slider_pos(&self) -> f32 {
        let [sx, _, sw, _] = self.zoom_rects[1];
        let z = self.view.zoom();
        let t = if z <= 1.0 {
            (z - 0.1) / 0.9 * 0.5
        } else {
            0.5 + (z - 1.0) / 4.0 * 0.5
        };
        sx + t.clamp(0.0, 1.0) * sw
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

    /// Width of the vertical ruler (shown with the ruler, as on a desktop word processor).
    fn vr_w(&self) -> f32 {
        if self.show_ruler {
            (16.0 * self.scale).round()
        } else {
            0.0
        }
    }

    /// Width of the vertical scroll bar (as on a desktop word processor).
    fn sb_w(&self) -> f32 {
        (15.0 * self.scale).round()
    }

    /// Scroll bar parts: (bar, up button, down button, thumb) as [x, y, w, h].
    fn sb_rects(&self) -> [[f32; 4]; 4] {
        let w = self.sb_w();
        let x = self.size.0 - w;
        let top = self.view_top();
        let bottom = self.size.1 - self.status_h();
        let bar = [x, top, w, (bottom - top).max(0.0)];
        let up = [x, top, w, w];
        let down = [x, bottom - w, w, w];
        let (scroll, content, view) = self.view.scroll_info();
        let track = (bar[3] - 2.0 * w).max(0.0);
        let thumb_h = if content > 0.0 {
            (track * (view / content).min(1.0))
                .max((12.0 * self.scale).round())
                .min(track)
        } else {
            track
        };
        let room = (content - view).max(1.0);
        let thumb_y = top + w + ((track - thumb_h) * (scroll / room).clamp(0.0, 1.0)).round();
        [bar, up, down, [x, thumb_y, w, thumb_h.round()]]
    }

    /// Scroll from a press or drag on the scroll bar at window `y`; `true` when it moved.
    fn sb_press(&mut self, y: f32) -> bool {
        let [_, up, down, thumb] = self.sb_rects();
        let (scroll, _, view) = self.view.scroll_info();
        let line = (40.0 * self.scale).round();
        if y < up[1] + up[3] {
            self.view.scroll_to(scroll - line)
        } else if y >= down[1] {
            self.view.scroll_to(scroll + line)
        } else if y < thumb[1] {
            self.view.scroll_to(scroll - view)
        } else if y >= thumb[1] + thumb[3] {
            self.view.scroll_to(scroll + view)
        } else {
            self.sb_drag = Some(y - thumb[1]);
            false
        }
    }

    /// Drag the thumb so its top follows the pointer.
    fn sb_drag_to(&mut self, y: f32) -> bool {
        let Some(offset) = self.sb_drag else {
            return false;
        };
        let [bar, _, _, thumb] = self.sb_rects();
        let w = self.sb_w();
        let track = (bar[3] - 2.0 * w - thumb[3]).max(1.0);
        let (_, content, view) = self.view.scroll_info();
        let t = ((y - offset - (bar[1] + w)) / track).clamp(0.0, 1.0);
        self.view.scroll_to(t * (content - view).max(0.0))
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
        let mut top = self.ribbon.bottom();
        if self.show_ruler {
            top += Ruler::height(self.scale);
        }
        let h = (self.size.1 - top - self.status_h()).max(1.0);
        // The vertical scroll bar takes the right edge, the vertical ruler the left one.
        let vr = self.vr_w();
        self.view
            .set_frame(vr, top, (self.size.0 - self.sb_w() - vr).max(1.0), h);
        self.layout_ruler();
        if let Some(d) = self.dialog.as_mut() {
            d.layout(text, self.size, self.scale);
        }
        if let Some(b) = self.backstage.as_mut() {
            b.layout(self.title.height(), self.size, self.scale);
        }
        if self.styles.is_empty() {
            self.styles = self.view.styles();
        }
        self.ribbon.measure_previews(text, &self.styles);
        // Status bar zoom controls, right to left: percentage, plus, slider, minus.
        let px = |v: f32| (v * self.scale).round();
        let (w, sh) = (self.size.0, self.status_h());
        let y = self.size.1 - sh;
        let pct = [w - px(56.0), y, px(48.0), sh];
        let plus = [pct[0] - px(24.0), y, px(22.0), sh];
        let track = [plus[0] - px(108.0), y, px(104.0), sh];
        let minus = [track[0] - px(24.0), y, px(22.0), sh];
        self.zoom_rects = [minus, track, plus, pct];
        // View buttons before the zoom controls.
        let bw = px(32.0);
        let gap = px(2.0);
        let right = minus[0] - px(12.0);
        for (i, r) in self.view_rects.iter_mut().enumerate() {
            let x = right - (3 - i) as f32 * (bw + gap);
            *r = [x, y + 1.0, bw, sh - 1.0];
        }
    }

    fn layout_ruler(&mut self) {
        if let Some(page) = self.view.ruler_page() {
            self.ruler.layout(
                self.ribbon.bottom(),
                self.size.0,
                self.scale,
                page,
                self.view.scale(),
                self.view.format(),
            );
        }
    }

    fn zoom_hit(&self, x: f32, y: f32) -> Option<usize> {
        self.zoom_rects
            .iter()
            .position(|[rx, ry, rw, rh]| x >= *rx && x < rx + rw && y >= *ry && y < ry + rh)
    }

    /// Handle an input event; `true` when a new frame is needed.
    pub fn handle(&mut self, input: Input, text: &dyn TextMeasure) -> bool {
        let redraw = self.route(input, text);
        if self.relayout_pending {
            self.relayout_pending = false;
            self.relayout(text);
        } else if self.show_ruler {
            self.layout_ruler();
        }
        if let Some(d) = self.dialog.as_mut() {
            d.layout(text, self.size, self.scale);
        }
        redraw
    }

    fn route(&mut self, input: Input, text: &dyn TextMeasure) -> bool {
        // The File screen covers everything below the title bar.
        if let Some(b) = self.backstage.as_mut() {
            match input {
                Input::Move { x, y } if y >= self.title.height() => return b.hover(x, y),
                Input::Press { x, y, .. } if y >= self.title.height() => {
                    return match b.press(x, y) {
                        Some(a) => self.file_action(a),
                        None => false,
                    };
                }
                Input::Key(Key::Escape, _) => {
                    self.backstage = None;
                    return true;
                }
                Input::Text(_)
                | Input::Key(..)
                | Input::Wheel(_)
                | Input::Context { .. }
                | Input::Preedit(_) => {
                    return false;
                }
                _ => {}
            }
        }
        // A dialog is modal: it takes keyboard and pointer input.
        if let Some(d) = self.dialog.as_mut() {
            match input {
                Input::Text(ref s) => {
                    d.text(s);
                    return true;
                }
                Input::Key(k, m) => {
                    if let Some(b) = d.key(k, m) {
                        self.dialog_button(b);
                    }
                    return true;
                }
                Input::Move { x, y } => return d.hover(x, y),
                Input::Press { x, y, .. } => {
                    if let Some(b) = d.press(x, y) {
                        self.dialog_button(b);
                    }
                    return true;
                }
                Input::Blink => return true,
                Input::Wheel(_)
                | Input::Drag { .. }
                | Input::Release { .. }
                | Input::Context { .. } => {
                    return false;
                }
                _ => {}
            }
        }
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
        if let Input::Key(
            Key::Letter(c @ ('s' | 'c' | 'x' | 'v' | 'f' | 'h' | 'k')),
            Mods {
                ctrl: true, shift, ..
            },
        ) = input
        {
            let id = match (c, shift) {
                ('s', _) => "file.save",
                ('c', true) => "format.painter",
                ('c', _) => "edit.copy",
                ('x', _) => "edit.cut",
                ('v', _) => "edit.paste",
                ('f', _) => "edit.find",
                ('h', _) => "edit.replace",
                _ => "insert.link",
            };
            return self.action(id);
        }
        if let Input::Key(Key::Escape, _) = input {
            if self.painter {
                self.painter = false;
                if let Some(c) = catalog::from_json(json!({ "cmd": "cancelFormat" })) {
                    self.view.run_command(c);
                }
                return true;
            }
        }
        if let Input::Context { x, y } = input {
            if y >= self.view_top() && y < self.size.1 - self.status_h() {
                self.context_menu(x, y);
                return true;
            }
            return false;
        }
        // Ruler: drag indent markers.
        if self.show_ruler {
            match input {
                Input::Press { x, y, .. } if self.ruler.contains_y(y) => {
                    self.ruler_drag = self.ruler.press(x, y);
                    return self.ruler_drag;
                }
                Input::Drag { x, .. } if self.ruler_drag => return self.ruler.drag(x),
                Input::Release { .. } if self.ruler_drag => {
                    self.ruler_drag = false;
                    if let Some((left, right, first_line)) = self.ruler.release(self.view.format())
                    {
                        let cmd = catalog::from_json(json!({
                            "cmd": "formatParagraph",
                            "format": { "kind": "indents", "left": left, "right": right, "firstLine": first_line },
                        }));
                        if let Some(c) = cmd {
                            self.view.run_command(c);
                        }
                    }
                    return true;
                }
                _ => {}
            }
        }
        // Drag-selection auto-scroll continues while the pointer rests past the view.
        if let Input::Tick = input {
            if let (true, Some((x, y))) = (self.doc_press, self.last_drag) {
                let top = self.view_top();
                let bottom = self.size.1 - self.status_h();
                if y < top || y > bottom {
                    self.view.handle(Input::Drag { x, y });
                    return true;
                }
            }
        }
        if let Input::Drag { x, y } = input {
            if self.doc_press {
                self.last_drag = Some((x, y));
            }
        }
        if let Input::Zoom(steps) = input {
            let z = ((self.view.zoom() + 0.1 * steps) * 10.0).round() / 10.0;
            return self.zoom_to(z, text);
        }
        let status_top = self.size.1 - self.status_h();
        // Vertical scroll bar on the right edge of the document area.
        match input {
            Input::Press { x, y, .. }
                if x >= self.size.0 - self.sb_w()
                    && y >= self.view_top()
                    && y < status_top
                    && self.menu.is_none()
                    && self.backstage.is_none() =>
            {
                return self.sb_press(y);
            }
            Input::Drag { y, .. } if self.sb_drag.is_some() => return self.sb_drag_to(y),
            Input::Release { .. } if self.sb_drag.is_some() => {
                self.sb_drag = None;
                return false;
            }
            _ => {}
        }
        match input {
            Input::Press { x, y, .. } if y >= status_top => {
                if let Some(i) = self
                    .view_rects
                    .iter()
                    .position(|[rx, ry, rw, rh]| x >= *rx && x < rx + rw && y >= *ry && y < ry + rh)
                {
                    let id = format!("view.layout.{}", VIEW_MODES[i].0);
                    return self.action(&id);
                }
                return match self.zoom_hit(x, y) {
                    Some(0) => self.zoom_to(((self.view.zoom() - 0.1) * 10.0).round() / 10.0, text),
                    Some(1) => {
                        self.zoom_drag = true;
                        let z = self.slider_zoom(x);
                        self.zoom_to(z, text)
                    }
                    Some(2) => self.zoom_to(((self.view.zoom() + 0.1) * 10.0).round() / 10.0, text),
                    Some(_) => self.zoom_to(1.0, text),
                    None => false,
                };
            }
            Input::Drag { x, .. } if self.zoom_drag => {
                let z = self.slider_zoom(x);
                return self.zoom_to(z, text);
            }
            Input::Release { .. } if self.zoom_drag => {
                self.zoom_drag = false;
                return false;
            }
            _ => {}
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
            Input::Zoom(_) | Input::Context { .. } => false,
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
                // The format painter applies to the selection just made in the document.
                if self.doc_press && self.painter {
                    self.painter = false;
                    self.view.run_command(Command::ApplyFormat);
                }
                self.doc_press = false;
                self.last_drag = None;
                let quick = match self.title.release(x, y) {
                    Some(id) => self.action(id),
                    None => false,
                };
                let (fired, changed) = self.ribbon.release(x, y);
                let ran = match fired {
                    Some(RibbonHit::File) => {
                        let mut b = Backstage::default();
                        if let Some(props) = self.view.doc_properties() {
                            let date = |s: &str| s.split('T').next().unwrap_or("").to_owned();
                            let rows = [
                                ("backstage.props.title", "Title", props.title.clone()),
                                ("backstage.props.author", "Author", props.author.clone()),
                                (
                                    "backstage.props.lastModifiedBy",
                                    "Last modified by",
                                    props.last_modified_by.clone(),
                                ),
                                ("backstage.props.created", "Created", date(&props.created)),
                                (
                                    "backstage.props.modified",
                                    "Last modified",
                                    date(&props.modified),
                                ),
                            ];
                            b.set_properties(
                                rows.into_iter()
                                    .map(|(k, f, v)| (self.t(k, f), v))
                                    .collect(),
                            );
                        }
                        b.layout(self.title.height(), self.size, self.scale);
                        self.backstage = Some(b);
                        true
                    }
                    Some(RibbonHit::Command(id)) => self.action(id),
                    Some(RibbonHit::Style(i)) => match self.styles.get(i) {
                        Some(st) => {
                            let id = st.id.clone();
                            self.view.run_command(Command::ApplyStyle { id })
                        }
                        None => false,
                    },
                    Some(RibbonHit::StylesMore) => {
                        self.open_styles_menu();
                        true
                    }
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
            | Input::Preedit(_)
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
        self.menu = Some(Menu::open(items, anchor, self.size, self.scale));
    }

    fn open_styles_menu(&mut self) {
        let Some(anchor) = self.ribbon.rect_of(RibbonHit::StylesMore) else {
            return;
        };
        let current = self.view.format().style.clone();
        let items = self
            .styles
            .iter()
            .map(|st| MenuItem {
                label: commands::style_name_key(&st.name)
                    .map_or_else(|| st.name.clone(), |k| self.strings.get(k).to_owned()),
                choice: Choice::Style(st.id.clone()),
                checked: current.as_deref() == Some(st.id.as_str()),
                dim: false,
            })
            .collect();
        self.menu = Some(Menu::open(items, anchor, self.size, self.scale));
    }

    fn choose(&mut self, choice: Choice) {
        let format = match choice {
            Choice::Font(name) => CharFormat::Font { name },
            Choice::Size(half_points) => CharFormat::Size { half_points },
            Choice::Style(id) => {
                self.view.run_command(Command::ApplyStyle { id });
                return;
            }
            Choice::Command(id) => {
                self.action(&id);
                return;
            }
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

    /// Whether an animation needs more frames.
    pub fn animating(&self) -> bool {
        self.view.animating()
    }

    /// Advance animations to the host clock (ms); `true` while frames are still needed.
    pub fn advance(&mut self, now_ms: f64) -> bool {
        self.view.advance(now_ms)
    }

    /// Forget page images when the host changes how document text is rendered.
    pub fn invalidate_page_images(&mut self) {
        self.view.invalidate_page_images();
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
            &self.styles,
            &self.on_ids(),
        );
        if self.show_ruler {
            self.ruler.paint(p, &self.theme);
            if let Some(page) = self.view.ruler_page_v() {
                let top = self.view_top();
                let bottom = self.size.1 - self.status_h();
                crate::ruler::paint_vertical(
                    p,
                    &self.theme,
                    (0.0, self.vr_w(), top, bottom),
                    page,
                    &self.view.format().section,
                    self.view.scale() / 20.0,
                    self.scale,
                );
            }
        }
        // Status bar.
        let (w, h) = self.size;
        let sh = self.status_h();
        let y = h - sh;
        // Vertical scroll bar: light track, white buttons and thumb framed in grey, dark
        // arrows (measured on a desktop word processor: 243 / 255 / 148 / 38).
        let [bar, up, down, thumb] = self.sb_rects();
        if bar[3] > 0.0 {
            let edge = Rgba(0x94, 0x94, 0x94, 255);
            p.fill_rect(bar[0], bar[1], bar[2], bar[3], Rgba(0xf3, 0xf3, 0xf3, 255));
            let boxed = |p: &mut dyn Painter, r: [f32; 4]| {
                p.fill_rect(r[0], r[1], r[2], r[3], edge);
                p.fill_rect(
                    r[0] + 1.0,
                    r[1] + 1.0,
                    r[2] - 2.0,
                    r[3] - 2.0,
                    self.theme.surface,
                );
            };
            boxed(p, up);
            boxed(p, down);
            if thumb[3] > 2.0 {
                boxed(p, thumb);
            }
            let k = (3.0 * self.scale).round();
            let arrow = Rgba(0x26, 0x26, 0x26, 255);
            let (cx, cy) = (up[0] + up[2] / 2.0, up[1] + up[3] / 2.0);
            p.path(
                &[
                    PathSeg::M(cx - k, cy + k / 2.0),
                    PathSeg::L(cx + k, cy + k / 2.0),
                    PathSeg::L(cx, cy - k / 2.0 - 1.0),
                    PathSeg::Z,
                ],
                Some(arrow),
                None,
            );
            let (cx, cy) = (down[0] + down[2] / 2.0, down[1] + down[3] / 2.0);
            p.path(
                &[
                    PathSeg::M(cx - k, cy - k / 2.0),
                    PathSeg::L(cx + k, cy - k / 2.0),
                    PathSeg::L(cx, cy + k / 2.0 + 1.0),
                    PathSeg::Z,
                ],
                Some(arrow),
                None,
            );
        }
        // Neutral grey bar with dark text, like the ribbon body (and a desktop word
        // processor's status bar).
        p.fill_rect(0.0, y, w, sh, self.theme.ribbon);
        p.fill_rect(0.0, y, w, 1.0, self.theme.border);
        for (i, r) in self.view_rects.iter().enumerate() {
            let (mode, icon) = VIEW_MODES[i];
            if mode == self.view_mode {
                p.fill_rect(r[0], r[1], r[2], r[3], self.theme.active);
            }
            let size = (16.0 * self.scale).round();
            self.icons.draw(
                p,
                icon,
                (r[0] + (r[2] - size) / 2.0).round(),
                (r[1] + (r[3] - size) / 2.0).round(),
                size,
                crate::icons::IconColors {
                    ink: self.theme.text,
                    accent: self.theme.accent,
                    on: self.theme.accent_on,
                    tinted: true,
                },
            );
        }
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
        p.text(&page, x0, ty, style, self.theme.text);
        p.text(
            &words,
            x0 + (110.0 * self.scale).round(),
            ty,
            style,
            self.theme.text,
        );
        let zoom = self.strings.fmt(
            "status.zoom",
            &[(
                "value",
                &((self.view.zoom() * 100.0).round() as i32).to_string(),
            )],
        );
        let [minus, track, plus, pct] = self.zoom_rects;
        let ink = self.theme.text;
        let k = (4.0 * self.scale).round();
        let line = self.scale.max(1.0).round();
        let (mx, my) = (minus[0] + minus[2] / 2.0, minus[1] + minus[3] / 2.0);
        p.fill_rect((mx - k).round(), my.round(), 2.0 * k, line, ink);
        let (px_, py) = (plus[0] + plus[2] / 2.0, plus[1] + plus[3] / 2.0);
        p.fill_rect((px_ - k).round(), py.round(), 2.0 * k, line, ink);
        p.fill_rect(px_.round(), (py - k).round(), line, 2.0 * k, ink);
        let ty2 = (track[1] + track[3] / 2.0).round();
        p.fill_rect(track[0], ty2, track[2], line, self.theme.border);
        // 100 % tick in the middle, the thumb at the current zoom.
        p.fill_rect(
            (track[0] + track[2] / 2.0).round(),
            ty2 - k,
            line,
            2.0 * k + line,
            self.theme.border,
        );
        let thumb = self.slider_pos();
        p.fill_rect(
            (thumb - k / 2.0).round(),
            ty2 - k - line,
            k,
            2.0 * k + 3.0 * line,
            self.theme.text,
        );
        p.text(&zoom, pct[0] + (6.0 * self.scale).round(), ty, style, ink);
        if let Some(menu) = &self.menu {
            menu.paint(p, &self.theme, &self.icons);
        } else if let Some((label, tw, anchor)) = &self.tooltip {
            paint_tooltip(p, &self.theme, label, *tw, *anchor, w, self.scale);
        }
        if let Some(b) = &self.backstage {
            b.paint(
                p,
                &self.theme,
                &self.strings,
                &self.name,
                self.view.page_count(),
                self.view.word_count(),
                self.view.modified(),
            );
            b.paint_icons(p, &self.theme, &self.icons);
        }
        if let Some(d) = &self.dialog {
            d.paint(p, &self.theme, self.size, true);
        }
    }

    fn view_top(&self) -> f32 {
        self.ribbon.bottom()
            + if self.show_ruler {
                Ruler::height(self.scale)
            } else {
                0.0
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
    fn font_size_steps_match_shared_editor_at_boundaries() {
        assert_eq!(step_font_size(26, true), 28);
        assert_eq!(step_font_size(28, false), 26);
        assert_eq!(step_font_size(25, true), 26);
        assert_eq!(step_font_size(25, false), 24);
        assert_eq!(step_font_size(144, true), 160);
        assert_eq!(step_font_size(160, false), 144);
        assert_eq!(step_font_size(16, false), 14);
        assert_eq!(step_font_size(2, false), 2);
        assert_eq!(step_font_size(3276, true), 3276);
    }

    #[test]
    fn font_ribbon_controls_change_and_clear_formatting() {
        let mut engine = Engine::with_bundled_fonts();
        engine.new_document("a.docx").unwrap();
        let mut w = DocWindow::new(DocView::new(engine).unwrap(), "en-US", "a.docx");
        w.handle(
            Input::Resize {
                width: 1800.0,
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
        w.view.run_command(Command::FormatText {
            format: CharFormat::Size { half_points: 26 },
        });
        let click = |window: &mut DocWindow, id| {
            let [x, y, width, height] = window.ribbon.rect_of(RibbonHit::Command(id)).unwrap();
            let (x, y) = (x + width / 2.0, y + height / 2.0);
            window.handle(
                Input::Press {
                    x,
                    y,
                    count: 1,
                    shift: false,
                },
                &Fixed,
            );
            window.handle(Input::Release { x, y }, &Fixed);
        };
        click(&mut w, "format.growStep");
        assert_eq!(w.view.format().size_half_points, Some(28));
        click(&mut w, "format.shrinkStep");
        assert_eq!(w.view.format().size_half_points, Some(26));
        click(&mut w, "format.bold");
        assert!(w.view.format().bold);
        click(&mut w, "format.color");
        assert_eq!(w.view.format().color.as_deref(), Some("FF0000"));
        click(&mut w, "format.highlight");
        assert!(w.view.format().highlight.is_some());
        click(&mut w, "format.clear");
        assert!(!w.view.format().bold);
        // Menu entries of the Paragraph group run through the engine.
        let sec = bo_edit::SectionState::default();
        for id in [
            "format.multilevel.outline",
            "format.paraBorders.all",
            "format.sortParagraphs.asc",
            "format.textEffects.glow",
            "format.textEffects.none",
        ] {
            let cmd = crate::catalog::command(id, &sec).expect(id);
            w.view.run_command(cmd);
        }
        assert!(w.view.format().numbering);
    }

    #[test]
    fn insert_layout_and_reference_commands_reach_the_saved_document() {
        let mut engine = Engine::with_bundled_fonts();
        engine.new_document("a.docx").unwrap();
        let mut w = DocWindow::new(DocView::new(engine).unwrap(), "en-US", "a.docx");
        w.handle(
            Input::Resize {
                width: 1800.0,
                height: 900.0,
            },
            &Fixed,
        );
        w.handle(Input::Text("Hello world".into()), &Fixed);
        let sec = w.view.format().section;
        let run = |w: &mut DocWindow, id: &str| {
            let cmd = crate::catalog::command(id, &sec).expect(id);
            assert!(w.view.run_command(cmd), "{id} rejected by the engine");
        };
        for id in [
            "insert.field.PAGE",
            "insert.field.NUMPAGES",
            "insert.dateTime",
            "insert.dropCap.drop",
            "layout.lineNumbers.continuous",
            "layout.hyphenation.auto",
            "references.caption.Figure",
            "insert.signatureLine",
            "insert.shape.ellipse",
        ] {
            run(&mut w, id);
        }
        let bytes = w.view.save_docx().unwrap();
        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
        let mut read = |name: &str| {
            let mut s = String::new();
            std::io::Read::read_to_string(&mut zip.by_name(name).unwrap(), &mut s).unwrap();
            s
        };
        let doc = read("word/document.xml");
        // §17.16.5.44 PAGE, §17.16.5.42 NUMPAGES, §17.16.5.16 DATE.
        for instr in ["PAGE", "NUMPAGES", "DATE"] {
            assert!(doc.contains(instr), "{instr} field missing");
        }
        assert!(doc.contains("w:dropCap=\"drop\""), "framePr dropCap");
        assert!(doc.contains("w:lnNumType"), "line numbering");
        assert!(doc.contains("Figure"), "caption");
        assert!(doc.contains("prstGeom prst=\"ellipse\""), "ellipse shape");
        assert!(read("word/settings.xml").contains("w:autoHyphenation"));
        // A text box: shape holding txbxContent.
        run(&mut w, "insert.textBox");
        let bytes = w.view.save_docx().unwrap();
        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
        let mut doc = String::new();
        std::io::Read::read_to_string(&mut zip.by_name("word/document.xml").unwrap(), &mut doc)
            .unwrap();
        assert!(doc.contains("txbxContent"), "text box");
    }

    #[test]
    fn group_settings_buttons_open_font_and_paragraph_dialogs() {
        assert_eq!(parse_num(" 7,5 "), Some(7.5));
        assert_eq!(parse_num("12"), Some(12.0));
        assert_eq!(parse_num(""), None);
        assert_eq!(fmt_pt(150), "7,5");
        assert_eq!(fmt_pt(240), "12");
        let mut engine = Engine::with_bundled_fonts();
        engine.new_document("a.docx").unwrap();
        let mut w = DocWindow::new(DocView::new(engine).unwrap(), "en-US", "a.docx");
        w.handle(
            Input::Resize {
                width: 1800.0,
                height: 900.0,
            },
            &Fixed,
        );
        for (id, kind) in [
            ("format.fontSettings", DialogKind::Font),
            ("format.paragraphSettings", DialogKind::Paragraph),
        ] {
            assert!(
                w.ribbon.rect_of(RibbonHit::Command(id)).is_some(),
                "{id} on the ribbon"
            );
            w.action(id);
            assert_eq!(w.dialog.as_ref().map(|d| d.kind), Some(kind));
            w.dialog = None;
        }
        // Paragraph dialog: the left indent typed in points applies on Enter ("0" + "36").
        w.action("format.paragraphSettings");
        w.handle(Input::Text("36".into()), &Fixed);
        w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
        assert!(w.dialog.is_none());
        assert_eq!(w.view.format().indent_left, 720);
        // Font dialog: the size field (second) takes 7,5 pt.
        w.action("format.fontSettings");
        w.handle(Input::Key(Key::Tab, Mods::default()), &Fixed);
        for _ in 0..4 {
            w.handle(Input::Key(Key::Backspace, Mods::default()), &Fixed);
        }
        w.handle(Input::Text("7,5".into()), &Fixed);
        // Bold check box (fourth field): Tab twice from the size, Space ticks it.
        w.handle(Input::Key(Key::Tab, Mods::default()), &Fixed);
        w.handle(Input::Key(Key::Tab, Mods::default()), &Fixed);
        w.handle(Input::Text(" ".into()), &Fixed);
        w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
        assert_eq!(w.view.format().size_half_points, Some(15));
        assert!(w.view.format().bold);
    }

    #[test]
    fn line_spacing_menu_applies_and_undoes_paragraph_change() {
        let mut engine = Engine::with_bundled_fonts();
        engine.new_document("a.docx").unwrap();
        let mut w = DocWindow::new(DocView::new(engine).unwrap(), "vi-VN", "a.docx");
        w.handle(
            Input::Resize {
                width: 1800.0,
                height: 900.0,
            },
            &Fixed,
        );
        w.handle(Input::Text("hello".into()), &Fixed);
        let before = w.view.format().line_spacing;
        let [x, y, width, height] = w
            .ribbon
            .rect_of(RibbonHit::Command("format.lineSpacing"))
            .unwrap();
        let center = (x + width / 2.0, y + height / 2.0);
        w.handle(
            Input::Press {
                x: center.0,
                y: center.1,
                count: 1,
                shift: false,
            },
            &Fixed,
        );
        w.handle(
            Input::Release {
                x: center.0,
                y: center.1,
            },
            &Fixed,
        );
        assert!(w.menu.is_some());
        // Menu opens below the button: select the third entry (1.0, 1.15, 1.5).
        let (mx, my) = (x + 12.0, y + height + 2.0 + 4.0 + 2.0 * 26.0 + 13.0);
        w.handle(
            Input::Press {
                x: mx,
                y: my,
                count: 1,
                shift: false,
            },
            &Fixed,
        );
        w.handle(Input::Release { x: mx, y: my }, &Fixed);
        assert_eq!(w.view.format().line_spacing.rule, bo_edit::LineRule::Auto);
        assert_eq!(w.view.format().line_spacing.value, 360);
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
        assert_eq!(w.view.format().line_spacing, before);
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

    #[test]
    fn zoom_and_clipboard_requests() {
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
        assert!(w.handle(Input::Zoom(2.0), &Fixed));
        assert!((w.view().zoom() - 1.2).abs() < 1e-4);
        // The percentage resets to 100 %.
        let [x, y, pw, ph] = w.zoom_rects[3];
        w.handle(
            Input::Press {
                x: x + pw / 2.0,
                y: y + ph / 2.0,
                count: 1,
                shift: false,
            },
            &Fixed,
        );
        assert!((w.view().zoom() - 1.0).abs() < 1e-4);
        w.handle(Input::Text("xin chào".into()), &Fixed);
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
        w.handle(
            Input::Key(
                Key::Letter('c'),
                Mods {
                    ctrl: true,
                    ..Mods::default()
                },
            ),
            &Fixed,
        );
        let reqs = w.take_requests();
        assert!(
            matches!(&reqs[..], [Request::SetClipboard { text, html, .. }] if text == "xin chào" && !html.is_empty()),
            "{reqs:?}"
        );
        w.handle(
            Input::Key(
                Key::Letter('v'),
                Mods {
                    ctrl: true,
                    ..Mods::default()
                },
            ),
            &Fixed,
        );
        assert_eq!(w.take_requests(), vec![Request::Paste]);
    }
}
