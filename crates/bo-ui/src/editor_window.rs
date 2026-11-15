//! BlueEditor window: BlueReader's window (title bar, ribbon, document tabs, navigation panel,
//! view, status bar) with the editor's ribbon (Home, Convert, Edit, Organize, Comment, View,
//! Form, Protect, Help), a coloured title bar, a page grid to organise pages and editing tools.
//!
//! Layout, sizes and greys follow the reference desktop PDF editor by black-box comparison
//! (`docs/ui/REPORT-editor.md`); the accent colour, title bar colour and icons are our own.
use crate::dialog::{Dialog, DialogButton, DialogKind};
use crate::editor_organize::Organize;
use crate::popup::{Choice, MenuItem};
use crate::reader_view::{PdfStore, ReaderView};
use crate::reader_window::{Kind, Look, Panel, Profile, ReaderRequest, ReaderWindow, Tool};
use crate::theme::Theme;
use crate::titlebar::NcHit;
use crate::{Input, Key, Mods, Painter, TextMeasure};
use bo_pdfread::{PdfDocument, SaveOptions};
use bo_render::Rgba;

/// BlueEditor's accent: amber, distinct from BlueReader's crimson and the office apps.
pub const EDITOR_ACCENT: Rgba = Rgba(0xD9, 0x6A, 0x0B, 255);
/// Accent for text on white (WCAG 2.2 AA contrast).
const EDITOR_ACCENT_TEXT: Rgba = Rgba(0xA8, 0x4A, 0x05, 255);
/// Title bar: a deep slate blue of our own (the reference's brand colour is not used).
pub const EDITOR_TITLE: Rgba = Rgba(0x2F, 0x3E, 0x5C, 255);

/// Ribbon tabs after File.
const TABS: [&str; 9] = [
    "pdfReader.tabs.home",
    "pdfEditor.tabs.convert",
    "pdfEditor.tabs.edit",
    "pdfEditor.tabs.organize",
    "pdfReader.tabs.comment",
    "pdfReader.tabs.view",
    "pdfEditor.tabs.form",
    "pdfEditor.tabs.protect",
    "pdfReader.tabs.help",
];
/// Index of the Organize tab in [`TABS`].
pub(crate) const TAB_ORGANIZE: usize = 3;

#[allow(clippy::too_many_lines)]
fn ribbon(tab: usize) -> Vec<(&'static str, Kind)> {
    use Kind::{Big, Sep, Small};
    let common = [
        ("tool.hand", Big("hand", "pdfReader.hand", false)),
        ("tool.select", Big("select", "pdfReader.select", false)),
    ];
    let mut v: Vec<(&'static str, Kind)> = common.to_vec();
    match tab {
        0 => v.extend([
            ("edit.copy", Big("clipboard", "pdfReader.copy", false)),
            ("sep1", Sep),
            ("menu.zoom", Big("zoom", "pdfReader.zoom", true)),
            ("menu.fit", Big("fit", "pdfEditor.pageFitOption", true)),
            ("menu.rotate", Big("rotate", "pdfReader.rotateView", true)),
            ("sep2", Sep),
            ("ed.editText", Big("editText", "pdfEditor.editText", false)),
            (
                "ed.editObject",
                Big("editObject", "pdfEditor.editObject", false),
            ),
            ("sep3", Sep),
            (
                "tool.textbox",
                Big("typewriter", "pdfReader.typewriter", false),
            ),
            (
                "tool.highlight",
                Big("highlight", "pdfReader.highlight", false),
            ),
            ("sep4", Sep),
            (
                "org.rotateMenu",
                Big("rotateRight", "pdfEditor.rotatePages", true),
            ),
            (
                "org.insertMenu",
                Big("insertPage", "pdfEditor.insert", true),
            ),
            ("sep5", Sep),
            ("form.fill", Big("fillSign", "pdfEditor.fillSign", false)),
        ]),
        1 => v.extend([
            ("sep1", Sep),
            (
                "conv.create",
                Big("createPdf", "pdfEditor.fromFiles", false),
            ),
            ("conv.combine", Big("combine", "pdfEditor.combine", false)),
            ("conv.blank", Big("blankPage", "pdfEditor.blank", false)),
            ("sep2", Sep),
            ("conv.word", Big("toWord", "pdfEditor.toWord", false)),
            ("conv.excel", Big("toExcel", "pdfEditor.toExcel", false)),
            ("conv.slides", Big("toSlides", "pdfEditor.toSlides", false)),
            ("conv.image", Big("toImage", "pdfEditor.toImage", false)),
            ("conv.text", Big("file", "pdfEditor.toText", false)),
        ]),
        2 => v.extend([
            ("sep1", Sep),
            ("ed.editText", Big("editText", "pdfEditor.editText", false)),
            (
                "ed.editObject",
                Big("editObject", "pdfEditor.editObject", false),
            ),
            ("sep2", Sep),
            ("ed.addText", Big("addText", "pdfEditor.addText", false)),
            ("ed.addImage", Big("addImage", "pdfEditor.addImage", false)),
            ("sep3", Sep),
            (
                "ed.delete",
                Big("deletePage", "pdfEditor.deleteObject", false),
            ),
        ]),
        TAB_ORGANIZE => v.extend([
            ("sep1", Sep),
            (
                "org.insertMenu",
                Big("insertPage", "pdfEditor.insert", true),
            ),
            ("org.delete", Big("deletePage", "pdfEditor.delete", false)),
            (
                "org.extract",
                Big("extractPage", "pdfEditor.extract", false),
            ),
            (
                "org.reverse",
                Big("reversePages", "pdfEditor.reverse", false),
            ),
            ("sep2", Sep),
            (
                "org.rotateLeft",
                Big("rotateLeft", "pdfEditor.rotateLeft", false),
            ),
            (
                "org.rotateRight",
                Big("rotateRight", "pdfEditor.rotateRight", false),
            ),
            ("sep3", Sep),
            ("org.split", Big("splitPages", "pdfEditor.split", false)),
            ("org.combine", Big("combine", "pdfEditor.combine", false)),
        ]),
        4 => v.extend([
            ("sep1", Sep),
            (
                "tool.highlight",
                Big("highlight", "pdfReader.highlight", false),
            ),
            (
                "tool.underline",
                Big("underline", "pdfReader.underline", false),
            ),
            (
                "tool.strikeout",
                Big("strikeout", "pdfReader.strikeout", false),
            ),
            (
                "tool.squiggly",
                Big("squiggly", "pdfReader.squiggly", false),
            ),
            ("sep2", Sep),
            ("tool.note", Big("note", "pdfReader.note", false)),
            ("cm.file", Big("attach", "pdfEditor.file", false)),
            ("sep3", Sep),
            (
                "cm.typewriter",
                Big("typewriter", "pdfReader.typewriter", false),
            ),
            ("tool.textbox", Big("textbox", "pdfReader.textbox", false)),
            ("cm.callout", Big("callout", "pdfEditor.callout", false)),
            ("sep4", Sep),
            ("cm.drawing", Big("drawing", "pdfEditor.drawing", true)),
            ("cm.pencil", Big("pencil", "pdfEditor.pencil", false)),
            ("sep5", Sep),
            ("cm.stamp", Big("stamp", "pdfEditor.stamp", true)),
            ("sep6", Sep),
            (
                "panel.comments",
                Big("comments", "pdfReader.commentList", false),
            ),
        ]),
        5 => v.extend([
            ("sep1", Sep),
            ("view.single", Small("single")),
            ("view.continuous", Small("continuous")),
            ("view.facing", Small("facing")),
            ("view.contFacing", Small("contFacing")),
            ("sep2", Sep),
            ("menu.rotate", Big("rotate", "pdfReader.rotateView", true)),
            ("sep3", Sep),
            ("zoom.in", Big("zoom", "pdfReader.zoomIn", false)),
            ("zoom.out", Big("zoomOut", "pdfReader.zoomOut", false)),
            ("zoom.actual", Big("actual", "pdfReader.actualSize", false)),
            ("zoom.page", Big("fit", "pdfReader.fitPage", false)),
            ("zoom.width", Big("fitWidth", "pdfReader.fitWidth", false)),
            ("sep4", Sep),
            (
                "panel.bookmarks",
                Big("bookmark", "pdfReader.bookmarks", false),
            ),
            ("panel.pages", Big("pages", "pdfReader.pages", false)),
        ]),
        6 => v.extend([
            ("sep1", Sep),
            ("form.fill", Big("fillSign", "pdfEditor.fillSign", false)),
            (
                "form.highlight",
                Big("formFields", "pdfEditor.highlightFields", false),
            ),
            ("sep2", Sep),
            ("form.reset", Big("resetForm", "pdfEditor.resetForm", false)),
            ("form.flatten", Big("flatten", "pdfEditor.flatten", false)),
        ]),
        7 => v.extend([
            ("sep1", Sep),
            (
                "prot.redact",
                Big("redact", "pdfEditor.markRedaction", false),
            ),
            (
                "prot.applyRedact",
                Big("applyRedact", "pdfEditor.applyRedactions", false),
            ),
            ("sep2", Sep),
            ("prot.password", Big("secure", "pdfEditor.secure", false)),
            (
                "prot.removeSecurity",
                Big("unsecure", "pdfEditor.removeSecurity", false),
            ),
            ("sep3", Sep),
            ("prot.sign", Big("sign", "pdfEditor.signCertify", false)),
            (
                "prot.sigPanel",
                Big("signPanel", "pdfEditor.signaturePanel", false),
            ),
            ("form.fill", Big("fillSign", "pdfEditor.fillSign", false)),
        ]),
        _ => {
            v.clear();
            v.push(("help.about", Big("info", "pdfReader.about", false)));
        }
    }
    v
}

fn profile() -> Profile {
    Profile {
        app: "BlueEditor",
        tabs: TABS.to_vec(),
        items: ribbon,
        file_menu: vec![
            ("file.open", "pdfReader.open"),
            ("file.create", "pdfEditor.fromFiles"),
            ("file.merge", "pdfEditor.combine"),
            ("file.save", "pdfReader.save"),
            ("file.saveAs", "pdfReader.saveAs"),
            ("file.saveOptimized", "pdfEditor.saveOptimized"),
            ("file.print", "pdfReader.print"),
            ("file.close", "pdfReader.close"),
        ],
        title_band: Some(EDITOR_TITLE),
        app_icon: "appEditor",
        look: LOOK,
    }
}

/// Sizes and greys of the reference desktop PDF editor at 125 % (pixels / 1.25):
/// title bar 45 px, ribbon to the line at 180 px, buttons 92 px high with 36 px icons and
/// 17 px labels, document tabs 44 px, status bar 44 px, white chrome with #E0E0E0 lines,
/// canvas #F0F0F0, pressed #E2E2E2 (`docs/ui/REPORT-editor.md`).
const LOOK: Look = Look {
    title_h: 36.0,
    ribbon_bottom: 144.0,
    doc_tabs_h: 35.2,
    status_h: 35.2,
    tab_h: 25.6,
    tab_gap_first: 24.0,
    tab_gap: 25.6,
    underline_up: 4.8,
    body_h: 73.6,
    icon: 28.8,
    icon_y: 4.8,
    label: 17.0,
    label_y: 36.0,
    quick_x: 54.8,
    quick_step: 28.0,
    title_center: true,
    search_in_tabs: Some([38.4, 40.0, 90.4, 22.4]),
    doc_tab_w: 140.0,
    chrome: Rgba(255, 255, 255, 255),
    edge: Rgba(224, 224, 224, 255),
    canvas: Rgba(240, 240, 240, 255),
    nav_bg: Rgba(255, 255, 255, 255),
    pressed: Rgba(226, 226, 226, 255),
    hover: Rgba(236, 236, 236, 255),
    strip_bg: Rgba(255, 255, 255, 255),
    wrap_words: true,
    search_hint: "pdfEditor.find",
    strip: Rgba(226, 226, 226, 255),
    strip_lines: true,
    box_edge: Rgba(224, 224, 224, 255),
    status_font: 17.0,
    body_gap: 4.0,
};

/// Editor tool that takes the document area's pointer input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditTool {
    /// Select and edit text runs.
    Text,
    /// Select, move, resize and delete objects (text blocks, images, paths).
    Object,
    /// Click to place a new text block.
    AddText,
    /// Drag a redaction mark.
    Redact,
}

/// What a dialog of the editor asks.
#[derive(Debug, Clone, PartialEq)]
enum Ask {
    Split,
    Password,
    OpenPassword(std::path::PathBuf),
}

/// Window contents of BlueEditor.
pub struct EditorWindow {
    pub(crate) r: ReaderWindow,
    org: Organize,
    tool: Option<EditTool>,
    dialog: Option<(Dialog, Ask)>,
    requests: Vec<ReaderRequest>,
    /// Panel shown before the page grid hid it.
    panel_before: Option<Panel>,
    /// Pointer position (window pixels).
    pointer: (f32, f32),
    /// Edit Text / Edit Object / Add Text state.
    pub(crate) edit: crate::editor_edit::Editing,
    /// Format pane hit boxes; the pane was closed with its close box.
    pub(crate) pane: crate::editor_format::FormatPane,
    pub(crate) format_closed: bool,
    /// Signature placement and the Signatures panel.
    pub(crate) sig: crate::editor_sign::SignState,
    /// Comment tools and the properties of new comments.
    pub(crate) comment: crate::editor_comment::CommentState,
    /// Caret shown (blink phase).
    caret_on: bool,
    /// Forms, comments, redaction and signing state.
    pub(crate) x: crate::editor_actions::Extras,
    /// Dialog of `editor_actions`.
    pub(crate) dialog2: Option<(Dialog, crate::editor_actions::Ask2)>,
    /// Field whose option menu is open.
    pub(crate) pending_field: Option<bo_pdfread::FormField>,
}

fn editor_theme() -> Theme {
    let mut t = Theme::light("reader");
    t.accent = EDITOR_ACCENT;
    t.accent_strong = EDITOR_ACCENT_TEXT;
    t.accent_text = EDITOR_ACCENT_TEXT;
    t
}

impl EditorWindow {
    /// An empty window (start page) in `locale`.
    pub fn new(locale: &str) -> Self {
        let mut r = ReaderWindow::with_profile(locale, profile());
        r.set_theme(editor_theme());
        Self {
            r,
            org: Organize::default(),
            tool: None,
            dialog: None,
            requests: Vec::new(),
            panel_before: None,
            pointer: (0.0, 0.0),
            edit: crate::editor_edit::Editing::default(),
            pane: crate::editor_format::FormatPane::default(),
            format_closed: false,
            sig: crate::editor_sign::SignState::default(),
            comment: crate::editor_comment::CommentState::default(),
            caret_on: true,
            x: crate::editor_actions::Extras::default(),
            dialog2: None,
            pending_field: None,
        }
    }

    /// Open `doc` named `name` in a new tab.
    pub fn open(&mut self, doc: PdfDocument, name: &str, path: Option<std::path::PathBuf>) {
        self.edit.reset(self.r.view_mut());
        self.x = crate::editor_actions::Extras::default();
        self.r.open(doc, name, path);
        self.org = Organize::default();
        self.sync();
    }

    /// Recent files shown on the start page.
    pub fn set_recent(&mut self, recent: Vec<std::path::PathBuf>) {
        self.r.set_recent(recent);
    }

    /// Recent files.
    pub fn recent(&self) -> &[std::path::PathBuf] {
        self.r.recent()
    }

    /// Requests since the last call.
    pub fn take_requests(&mut self) -> Vec<ReaderRequest> {
        std::mem::take(&mut self.requests)
    }

    /// Pointer shape (see [`ReaderWindow::cursor`]).
    pub fn cursor(&self) -> u8 {
        if self.organize_on() && inside(self.org.frame, self.pointer.0, self.pointer.1) {
            return 3;
        }
        match self.tool {
            Some(EditTool::Text | EditTool::AddText) if self.over_doc() => 4,
            Some(EditTool::Object | EditTool::Redact) if self.over_doc() => 0,
            _ => self.r.cursor(),
        }
    }

    fn over_doc(&self) -> bool {
        self.r.showing_doc() && inside(self.r.doc_area(), self.pointer.0, self.pointer.1)
    }

    /// Window title.
    pub fn title(&self) -> String {
        self.r.title()
    }

    /// The active document view.
    pub fn view(&self) -> Option<&ReaderView> {
        self.r.view()
    }

    /// The active document view.
    pub fn view_mut(&mut self) -> Option<&mut ReaderView> {
        self.r.view_mut()
    }

    /// File path of the active document.
    pub fn path(&self) -> Option<&std::path::Path> {
        self.r.path()
    }

    /// The active document was saved as `path`.
    pub fn set_path(&mut self, path: std::path::PathBuf) {
        self.r.set_path(path);
    }

    /// A status message.
    pub fn set_status(&mut self, s: &str) {
        self.r.set_status(s);
    }

    /// Window manager hit test.
    pub fn nc_hit(&self, x: f32, y: f32) -> NcHit {
        if self.dialog.is_some() || self.dialog2.is_some() {
            return NcHit::Client;
        }
        self.r.nc_hit(x, y)
    }

    /// The page grid is shown.
    pub fn organize_on(&self) -> bool {
        self.r.showing_doc() && self.r.ribbon_tab() == TAB_ORGANIZE
    }

    /// The editing tool in use.
    pub fn edit_tool(&self) -> Option<EditTool> {
        self.tool
    }

    /// Selected pages of the page grid.
    pub fn selected_pages(&self) -> Vec<usize> {
        self.org.selected.iter().copied().collect()
    }

    /// Show ribbon tab `i` (an index into the tabs after File; 3 = Organize).
    pub fn show_tab(&mut self, i: usize, m: &dyn TextMeasure) {
        self.r.set_ribbon_tab(i, m);
        self.sync();
    }

    /// Cell of page `i` in the page grid (window pixels), after a layout.
    #[cfg(test)]
    pub(crate) fn grid_cell(&mut self, i: usize) -> Option<[f32; 4]> {
        let v = self.r.view()?;
        let sizes: Vec<(f32, f32)> = (0..v.page_count()).map(|k| v.page_size(k)).collect();
        self.org.layout(&sizes);
        self.org.cell(i)
    }

    /// Run a ribbon or menu command.
    pub fn run(&mut self, cmd: &str, m: &dyn TextMeasure) {
        if cmd.starts_with("tool.") {
            self.comment.tool = None;
        }
        self.r.run_command(cmd, m);
        self.drain(m);
        self.sync();
    }

    pub(crate) fn t(&self, key: &str) -> String {
        self.r.strings().get(key).to_owned()
    }

    /// File name of the active document without its extension.
    pub(crate) fn doc_stem(&self) -> String {
        self.doc_name()
    }

    pub(crate) fn push(&mut self, r: ReaderRequest) {
        self.requests.push(r);
    }

    pub(crate) fn window_size(&self) -> (f32, f32) {
        self.r.window_size()
    }

    pub(crate) fn ui_scale(&self) -> f32 {
        self.r.ui_scale()
    }

    pub(crate) fn doc_area(&self) -> [f32; 4] {
        self.r.doc_area()
    }

    pub(crate) fn showing_doc(&self) -> bool {
        self.r.showing_doc()
    }

    pub(crate) fn accent(&self) -> Rgba {
        self.r.theme().accent
    }

    /// Choose the editing tool (`None`: the reader's hand / select tools).
    pub(crate) fn set_tool(&mut self, t: Option<EditTool>) {
        self.edit.reset(self.r.view_mut());
        self.tool = t;
    }

    /// Run `f` on the active document as an edit.
    pub(crate) fn doc_edit<T>(
        &mut self,
        f: impl FnOnce(&mut PdfDocument) -> Result<T, String>,
    ) -> Result<T, String> {
        let v = self.r.view_mut().ok_or("no document")?;
        f(v.doc_mut())
    }

    /// Run `f` on the active document (no edit).
    pub(crate) fn with_doc<T>(
        &mut self,
        f: impl FnOnce(&mut PdfDocument) -> Result<T, String>,
    ) -> Result<T, String> {
        self.doc_edit(f)
    }

    /// The document changed: redraw it.
    pub(crate) fn after_doc_edit(&mut self) {
        self.after_edit();
    }

    /// Open a menu of `items` under window rectangle `r`.
    pub(crate) fn menu_items(&mut self, items: Vec<MenuItem>, r: [f32; 4]) {
        self.r.menu_at(items, r, "field");
    }

    fn doc_name(&self) -> String {
        self.r.path().and_then(|p| p.file_stem()).map_or_else(
            || "document".to_owned(),
            |s| s.to_string_lossy().into_owned(),
        )
    }

    /// Handle an input event; true when the window needs a new frame.
    pub fn handle(&mut self, input: Input, m: &dyn TextMeasure) -> bool {
        if let Input::Move { x, y } | Input::Drag { x, y } = input {
            self.pointer = (x, y);
        }
        if self.dialog.is_some() || self.dialog2.is_some() {
            return self.dialog_input(&input, m);
        }
        if let Input::Blink = input {
            if self.edit.typing.is_some() {
                self.caret_on = !self.caret_on;
                return true;
            }
        }
        let mut redraw = false;
        let mut used = false;
        if !self.r.modal_open() {
            if let Input::Key(k, mods) = input {
                used = self.key(k, mods, m);
            }
            if !used && self.organize_on() {
                used = self.org_input(&input, m);
            }
            if !used {
                used = self.format_input(&input);
            }
            if !used && self.comment.tool.is_some() && self.r.showing_doc() && !self.organize_on() {
                used = self.comment_input(&input);
            }
            if !used && self.sig.pending.is_some() && self.r.showing_doc() {
                used = self.sign_input(&input);
            }
            if !used && self.tool.is_some() && self.r.showing_doc() && !self.organize_on() {
                used = if self.tool == Some(EditTool::Redact) {
                    self.redact_input(&input)
                } else {
                    self.edit_input(&input, m)
                };
            }
            if !used && self.tool.is_none() && self.r.showing_doc() && !self.organize_on() {
                if let Input::Press { x, y, count, .. } = input {
                    if inside(self.r.doc_area(), x, y) {
                        used = self.press_field_or_comment(x, y, count, m);
                    }
                }
            }
        }
        if used {
            redraw = true;
        } else {
            redraw |= self.r.handle(input, m);
        }
        redraw |= self.drain(m);
        self.sync();
        self.sync_signatures();
        if redraw && self.edit.typing.is_some() {
            if let Some(s) = self.format_status() {
                self.r.set_status(&s);
            }
        }
        redraw
    }

    fn key(&mut self, k: Key, mods: Mods, m: &dyn TextMeasure) -> bool {
        if self.edit.typing.is_some() && !(mods.ctrl && matches!(k, Key::Letter('z' | 'y' | 's'))) {
            let used = match self.r.view_mut() {
                Some(v) => self.edit.key(v, k, mods),
                None => false,
            };
            if used {
                self.caret_on = true;
                return true;
            }
        }
        if k == Key::Delete && !self.edit.selected.is_empty() && self.tool.is_some() {
            self.command("ed.delete", m);
            return true;
        }
        if k == Key::Delete && self.tool.is_none() && self.delete_comment() {
            return true;
        }
        match (k, mods.ctrl, mods.shift) {
            (Key::Letter('z'), true, false) => {
                self.undo(true);
                true
            }
            (Key::Letter('y'), true, false) | (Key::Letter('z'), true, true) => {
                self.undo(false);
                true
            }
            (Key::Escape, _, _) if self.tool.is_some() => {
                self.tool = None;
                true
            }
            _ => false,
        }
    }

    /// Undo (`back`) or redo the last edit of the active document.
    pub fn undo(&mut self, back: bool) {
        self.edit.reset(self.r.view_mut());
        let Some(v) = self.r.view_mut() else { return };
        let done = if back {
            v.doc_mut().undo()
        } else {
            v.doc_mut().redo()
        };
        if done {
            self.after_edit();
        }
    }

    fn after_edit(&mut self) {
        self.r.document_changed();
        let n = self.r.view().map_or(0, ReaderView::page_count);
        self.org.clamp(n);
    }

    /// Commands from the reader window that it does not know.
    fn drain(&mut self, m: &dyn TextMeasure) -> bool {
        let mut redraw = false;
        for req in self.r.take_requests() {
            match req {
                ReaderRequest::Command(cmd) => {
                    self.command(&cmd, m);
                    redraw = true;
                }
                other => self.requests.push(other),
            }
        }
        redraw
    }

    /// Selected pages of the grid, or the current page.
    fn target_pages(&self) -> Vec<usize> {
        if self.organize_on() && !self.org.selected.is_empty() {
            return self.selected_pages();
        }
        self.r
            .view()
            .map(|v| vec![v.current_page()])
            .unwrap_or_default()
    }

    fn insert_at(&self) -> usize {
        self.target_pages().last().map_or(0, |p| p + 1)
    }

    fn edit_result(&mut self, r: Result<(), String>) {
        match r {
            Ok(()) => self.after_edit(),
            Err(e) => self.r.set_status(&e),
        }
    }

    #[allow(clippy::too_many_lines)]
    fn command(&mut self, cmd: &str, m: &dyn TextMeasure) {
        if let Some(i) = cmd
            .strip_prefix("field.choose.")
            .and_then(|v| v.parse::<usize>().ok())
        {
            self.choose_option(i);
            return;
        }
        if self.action(cmd, m)
            || self.format_command(cmd)
            || self.sign_command(cmd)
            || self.comment_command(cmd)
        {
            return;
        }
        match cmd {
            "edit.undo" => self.undo(true),
            "ed.delete" => {
                if let Some(v) = self.r.view_mut() {
                    self.edit.finish_typing(Some(&mut *v));
                    self.edit.delete(v);
                }
                self.take_edit_status();
            }
            "ed.addImage" => self.requests.push(ReaderRequest::PickFile {
                purpose: "addImage".into(),
                filters: image_filters(),
            }),
            "ed.replaceImage" => self.requests.push(ReaderRequest::PickFile {
                purpose: "replaceImage".into(),
                filters: image_filters(),
            }),
            "edit.redo" => self.undo(false),
            "ed.editText" => self.toggle_tool(EditTool::Text),
            "ed.editObject" => self.toggle_tool(EditTool::Object),
            "ed.addText" => self.toggle_tool(EditTool::AddText),
            "prot.redact" => self.toggle_tool(EditTool::Redact),
            "tool.hand" | "tool.select" => {
                self.edit.reset(self.r.view_mut());
                self.tool = None;
                self.comment.tool = None;
            }
            "org.insertMenu" => {
                let items = [
                    ("org.insertBlank", "pdfEditor.blankPage"),
                    ("org.insertFile", "pdfEditor.fromFile"),
                ];
                self.menu(&items, "org.insertMenu");
            }
            "org.rotateMenu" => {
                let items = [
                    ("org.rotateRight", "pdfEditor.rotateRight"),
                    ("org.rotateLeft", "pdfEditor.rotateLeft"),
                    ("org.rotate180", "pdfEditor.rotate180"),
                ];
                self.menu(&items, "org.rotateMenu");
            }
            "org.insertBlank" => {
                let at = self.insert_at();
                let Some(v) = self.r.view_mut() else { return };
                let (w, h) = v
                    .doc()
                    .page_info(at.saturating_sub(1))
                    .map_or((595.0, 842.0), |p| (p.width, p.height));
                let r = v.doc_mut().insert_blank_page(at, w, h);
                self.edit_result(r);
                self.org.select_range(at, at);
            }
            "org.insertFile" => self.requests.push(ReaderRequest::PickFile {
                purpose: "insert".into(),
                filters: pdf_filters(),
            }),
            "org.combine" | "conv.combine" | "file.merge" => {
                self.requests.push(ReaderRequest::PickFile {
                    purpose: "merge".into(),
                    filters: pdf_filters(),
                });
            }
            "org.delete" => {
                let pages = self.target_pages();
                let Some(v) = self.r.view_mut() else { return };
                let r = v.doc_mut().delete_pages(&pages);
                self.edit_result(r);
                let first = pages.first().copied().unwrap_or(0);
                self.org.select_range(first, first);
            }
            "org.rotateLeft" | "org.rotateRight" | "org.rotate180" => {
                let turns = match cmd {
                    "org.rotateLeft" => -1,
                    "org.rotateRight" => 1,
                    _ => 2,
                };
                let pages = self.target_pages();
                let Some(v) = self.r.view_mut() else { return };
                let r = v.doc_mut().rotate_pages(&pages, turns);
                self.edit_result(r);
            }
            "org.reverse" => {
                let pages = if self.org.selected.len() > 1 {
                    self.selected_pages()
                } else {
                    Vec::new()
                };
                let Some(v) = self.r.view_mut() else { return };
                let r = v.doc_mut().reverse_pages(&pages);
                self.edit_result(r);
            }
            "org.extract" => {
                let pages = self.target_pages();
                let name = format!("{}-{}.pdf", self.doc_name(), page_list_label(&pages));
                let Some(v) = self.r.view() else { return };
                match v.doc().extract_pages(&pages, SaveOptions::default()) {
                    Ok(bytes) => self.requests.push(ReaderRequest::SaveBytes { name, bytes }),
                    Err(e) => self.r.set_status(&e),
                }
            }
            "org.split" => self.ask(
                Ask::Split,
                "pdfEditor.split",
                vec![(self.t("pdfEditor.pagesPerFile"), "1".into())],
                m,
            ),
            "prot.password" => self.ask(
                Ask::Password,
                "pdfEditor.secure",
                vec![
                    (self.t("pdfEditor.openPassword"), String::new()),
                    (self.t("pdfEditor.permissionsPassword"), String::new()),
                ],
                m,
            ),
            "prot.removeSecurity" => {
                let Some(v) = self.r.view_mut() else { return };
                let r = v.doc_mut().set_password("", "");
                self.edit_result(r);
                self.r.set_status(&self.t("pdfEditor.securityRemoved"));
            }
            "file.saveOptimized" => {
                let name = format!("{}-optimized.pdf", self.doc_name());
                let Some(v) = self.r.view_mut() else { return };
                let opts = SaveOptions {
                    object_streams: true,
                    ..SaveOptions::default()
                };
                match v.doc_mut().save_full(opts) {
                    Ok(bytes) => self.requests.push(ReaderRequest::SaveBytes { name, bytes }),
                    Err(e) => self.r.set_status(&e),
                }
            }
            _ => {}
        }
    }

    fn toggle_tool(&mut self, t: EditTool) {
        self.edit.reset(self.r.view_mut());
        self.comment.tool = None;
        self.tool = if self.tool == Some(t) { None } else { Some(t) };
        if self.tool.is_some() {
            self.r.set_tool(Tool::Hand);
        }
        self.x.marking = None;
    }

    pub(crate) fn menu(&mut self, items: &[(&str, &str)], id: &'static str) {
        let Some(r) = self.r.control_rect(id) else {
            return;
        };
        let items = items
            .iter()
            .map(|(cmd, key)| MenuItem {
                label: self.t(key),
                choice: Choice::Command((*cmd).to_owned()),
                checked: false,
                dim: false,
            })
            .collect();
        self.r.menu_at(items, r, id);
    }

    fn ask(&mut self, what: Ask, title: &str, fields: Vec<(String, String)>, m: &dyn TextMeasure) {
        let mut d = Dialog::new(
            DialogKind::Comment,
            self.t(title),
            fields,
            vec![
                (self.t("pdfReader.ok"), DialogButton::Ok),
                (self.t("pdfReader.cancel"), DialogButton::Cancel),
            ],
            Vec::new(),
        );
        d.layout(m, self.r.window_size(), self.r.ui_scale());
        self.dialog = Some((d, what));
    }

    fn dialog_input(&mut self, input: &Input, m: &dyn TextMeasure) -> bool {
        if self.dialog.is_none() {
            return self.dialog2_input(input, m);
        }
        let Some((d, _)) = self.dialog.as_mut() else {
            return false;
        };
        let done = match input {
            Input::Text(s) => {
                d.text(s);
                None
            }
            Input::Key(k, mods) => d.key(*k, *mods),
            Input::Press { x, y, .. } => d.press(*x, *y),
            Input::Move { x, y } => {
                d.hover(*x, *y);
                None
            }
            Input::Resize { .. } | Input::Scale(_) => {
                self.r.handle(input.clone(), m);
                None
            }
            _ => None,
        };
        let size = self.r.window_size();
        let scale = self.r.ui_scale();
        if let Some((d, _)) = self.dialog.as_mut() {
            d.layout(m, size, scale);
        }
        if let Some(b) = done {
            if let Some((d, what)) = self.dialog.take() {
                if b == DialogButton::Ok {
                    self.answer(&what, &d.values());
                }
            }
        }
        true
    }

    fn dialog2_input(&mut self, input: &Input, m: &dyn TextMeasure) -> bool {
        let size = self.r.window_size();
        let scale = self.r.ui_scale();
        let Some((d, _)) = self.dialog2.as_mut() else {
            return false;
        };
        let done = match input {
            Input::Text(s) => {
                d.text(s);
                None
            }
            Input::Key(k, mods) => d.key(*k, *mods),
            Input::Press { x, y, .. } => d.press(*x, *y),
            Input::Move { x, y } => {
                d.hover(*x, *y);
                None
            }
            _ => None,
        };
        d.layout(m, size, scale);
        if let Some(b) = done {
            if let Some((d, what)) = self.dialog2.take() {
                if b == DialogButton::Ok {
                    self.answer2(what, &d.values());
                }
            }
        }
        true
    }

    fn answer(&mut self, what: &Ask, values: &[String]) {
        match what {
            Ask::Split => {
                let every: usize = values
                    .first()
                    .and_then(|v| v.trim().parse().ok())
                    .unwrap_or(1);
                let stem = self.doc_name();
                let Some(v) = self.r.view() else { return };
                match v.doc().split(every, SaveOptions::default()) {
                    Ok(parts) => {
                        let files = parts
                            .into_iter()
                            .enumerate()
                            .map(|(i, b)| (format!("{stem}-part{}.pdf", i + 1), b))
                            .collect();
                        self.requests.push(ReaderRequest::SaveMany(files));
                    }
                    Err(e) => self.r.set_status(&e),
                }
            }
            Ask::OpenPassword(path) => {
                self.requests.push(ReaderRequest::OpenWithPassword {
                    path: path.clone(),
                    password: values.first().cloned().unwrap_or_default(),
                });
            }
            Ask::Password => {
                let user = values.first().cloned().unwrap_or_default();
                let mut owner = values.get(1).cloned().unwrap_or_default();
                if owner.is_empty() {
                    owner.clone_from(&user);
                }
                if owner.is_empty() {
                    return;
                }
                let Some(v) = self.r.view_mut() else { return };
                let r = v.doc_mut().set_password(&user, &owner);
                self.edit_result(r);
                self.r.set_status(&self.t("pdfEditor.securityAdded"));
            }
        }
    }

    /// A file needs a password to open: ask for it.
    pub fn password_needed(&mut self, path: &std::path::Path) {
        let name = path
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        let mut d = Dialog::new(
            DialogKind::Comment,
            format!("{name}: {}", self.t("pdfEditor.passwordProtected")),
            vec![(self.t("pdfEditor.enterPassword"), String::new())],
            vec![
                (self.t("pdfReader.ok"), DialogButton::Ok),
                (self.t("pdfReader.cancel"), DialogButton::Cancel),
            ],
            Vec::new(),
        );
        d.layout(
            &crate::editor_window::Estimate,
            self.r.window_size(),
            self.r.ui_scale(),
        );
        self.dialog = Some((d, Ask::OpenPassword(path.to_path_buf())));
    }

    /// Pages rendered as PNG for `purpose` (`images`: one file per page).
    pub fn pages_rendered(&mut self, purpose: &str, pngs: Vec<Vec<u8>>) {
        let stem = self.doc_name();
        if purpose == "slides" {
            self.slides_rendered(pngs);
            return;
        }
        if purpose == "images" {
            let files = pngs
                .into_iter()
                .enumerate()
                .map(|(i, b)| (format!("{stem}-{}.png", i + 1), b))
                .collect();
            self.requests.push(ReaderRequest::SaveMany(files));
        }
    }

    /// Text caret in window pixels while editing text.
    pub fn caret(&self) -> Option<[f32; 4]> {
        self.r.view().and_then(|v| self.edit.caret(v))
    }

    /// A file the host picked for `purpose` (`insert`, `merge`, `addImage`, `replaceImage`).
    pub fn file_picked(&mut self, purpose: &str, path: &std::path::Path, bytes: &[u8]) {
        let name = path
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        if purpose == "addImage" || purpose == "replaceImage" {
            self.picture_picked(purpose, bytes);
            return;
        }
        if purpose == "certificate" {
            self.certificate_picked(bytes);
            return;
        }
        if purpose == "attach" {
            self.attach_picked(&name, bytes);
            return;
        }
        if purpose == "create" {
            self.create_picked(path, bytes);
            return;
        }
        if bo_pdfread::PdfDocument::needs_password(bytes) {
            self.r.set_status(&format!(
                "{name}: {}",
                self.t("pdfEditor.passwordProtected")
            ));
            return;
        }
        let at = self.insert_at();
        let Some(v) = self.r.view_mut() else { return };
        let r = match purpose {
            "insert" => v.doc_mut().insert_pdf(at, bytes, None),
            "merge" => v.doc_mut().append_pdf(bytes),
            _ => return,
        };
        match r {
            Ok(n) => {
                let first = if purpose == "insert" {
                    at
                } else {
                    v.page_count()
                };
                self.after_edit();
                self.org.select_range(first, first + n.saturating_sub(1));
                self.r.set_status(&format!("{name}: +{n}"));
            }
            Err(e) => self.r.set_status(&e),
        }
    }

    /// Place or swap in a picture.
    fn picture_picked(&mut self, purpose: &str, bytes: &[u8]) {
        let Some(v) = self.r.view_mut() else { return };
        let r = if purpose == "replaceImage" {
            match self.edit.selected_image(v) {
                Some((page, id)) => v
                    .doc_mut()
                    .replace_image(page, id, bo_pdfread::NewImage(bytes))
                    .map(|()| page),
                None => Err("select an image first".into()),
            }
        } else {
            let page = v.current_page();
            let (pw, ph) = v
                .doc()
                .page_info(page)
                .map_or((595.0, 842.0), |p| (p.width, p.height));
            let (iw, ih) = bo_pdfread::image_size(bytes).unwrap_or((4, 3));
            // Natural size at 96 dpi, at most half the page.
            let mut w = iw as f32 * 0.75;
            let mut h = ih as f32 * 0.75;
            let k = (pw * 0.5 / w).min(ph * 0.5 / h).min(1.0);
            w *= k;
            h *= k;
            let rect = [
                (pw - w) / 2.0,
                (ph - h) / 2.0,
                f32::midpoint(pw, w),
                f32::midpoint(ph, h),
            ];
            v.doc_mut()
                .add_image(page, rect, bo_pdfread::NewImage(bytes))
                .map(|()| page)
        };
        match r {
            Ok(page) => {
                if let Some(v) = self.r.view_mut() {
                    v.invalidate_page(page);
                }
                self.edit.reset(None);
                if self.tool.is_none() {
                    self.tool = Some(EditTool::Object);
                }
            }
            Err(e) => self.r.set_status(&e),
        }
    }

    fn take_edit_status(&mut self) {
        if let Some(s) = self.edit.status.take() {
            self.r.set_status(&s);
        }
    }

    /// Pointer and text input for the editing tools over the document area.
    fn edit_input(&mut self, input: &Input, m: &dyn TextMeasure) -> bool {
        let Some(tool) = self.tool else { return false };
        if matches!(tool, EditTool::Redact) {
            return false;
        }
        let area = self.r.doc_area();
        let used = match *input {
            Input::Text(ref s) => match self.r.view_mut() {
                Some(v) => {
                    self.caret_on = true;
                    self.edit.type_text(v, s)
                }
                None => false,
            },
            Input::Press { x, y, count, shift } if inside(area, x, y) => {
                let image = self.r.view_mut().and_then(|v| {
                    let (p, px, py) = v.window_to_doc(x, y)?;
                    let _ = (p, px, py);
                    Some(())
                });
                if let Some(v) = self.r.view_mut() {
                    self.edit.press(v, x, y, count, shift, tool);
                }
                if count >= 2 && image.is_some() && tool == EditTool::Object {
                    if let Some(v) = self.r.view_mut() {
                        if self.edit.selected_image(v).is_some() {
                            self.command("ed.replaceImage", m);
                        }
                    }
                }
                self.caret_on = true;
                true
            }
            Input::Drag { x, y } => match self.r.view() {
                Some(v) => self.edit.drag(v, x, y),
                None => false,
            },
            Input::Release { .. } => match self.r.view_mut() {
                Some(v) => self.edit.release(v),
                None => false,
            },
            Input::Move { x, y } if inside(area, x, y) => {
                if let Some(v) = self.r.view_mut() {
                    self.edit.hover(v, x, y, tool);
                }
                // The reader window still tracks the pointer (status bar, controls).
                return false;
            }
            _ => false,
        };
        self.take_edit_status();
        used
    }

    /// Keep the reader window's pressed / enabled states and the page grid in step.
    fn sync(&mut self) {
        let organize = self.organize_on();
        if organize != self.r.doc_hidden {
            self.r.doc_hidden = organize;
            if organize {
                self.panel_before = self.r.panel();
                self.r.set_panel(None);
                if let Some(v) = self.r.view() {
                    let cur = v.current_page();
                    if self.org.selected.is_empty() {
                        self.org.select_range(cur, cur);
                    }
                }
            } else {
                let p = self.panel_before.take();
                self.r.set_panel(p);
            }
        }
        let (can_undo, can_redo) = self
            .r
            .view()
            .map_or((false, false), |v| (v.doc().can_undo(), v.doc().can_redo()));
        let doc = self.r.showing_doc();
        self.r.enabled_override.insert("edit.undo", doc && can_undo);
        self.r.enabled_override.insert("edit.redo", doc && can_redo);
        self.r.on.clear();
        if let Some(t) = self.tool {
            self.r.on.insert(match t {
                EditTool::Text => "ed.editText",
                EditTool::Object => "ed.editObject",
                EditTool::AddText => "ed.addText",
                EditTool::Redact => "prot.redact",
            });
        }
        if let Some(t) = self.comment.tool {
            self.r.on.insert(crate::editor_comment::tool_command(t));
        }
        self.r.tool_hidden = self.tool.is_some() || self.comment.tool.is_some();
        let area = self.r.doc_area();
        let (w, _) = self.r.window_size();
        self.org.frame = [area[0], area[1], (w - 1.0 - area[0]).max(1.0), area[3]];
        self.org.scale = self.r.ui_scale();
        self.sync_format();
    }

    fn org_input(&mut self, input: &Input, m: &dyn TextMeasure) -> bool {
        let Some(v) = self.r.view() else { return false };
        let n = v.page_count();
        let sizes: Vec<(f32, f32)> = (0..n).map(|i| v.page_size(i)).collect();
        self.org.layout(&sizes);
        match *input {
            Input::Press { x, y, count, shift } if inside(self.org.frame, x, y) => {
                if let Some(i) = self.org.press(x, y, shift) {
                    if count >= 2 {
                        // Open the page in the document view.
                        self.open_page(i, m);
                    }
                }
                true
            }
            Input::Drag { x, y } if self.org.dragging() => {
                self.org.drag_to(x, y);
                true
            }
            Input::Release { x, y } if self.org.dragging() => {
                if let Some((pages, to)) = self.org.release(x, y) {
                    let Some(v) = self.r.view_mut() else {
                        return true;
                    };
                    let r = v.doc_mut().move_pages(&pages, to);
                    self.edit_result(r);
                    let before = pages.iter().filter(|&&p| p < to).count();
                    let start = to - before;
                    self.org.select_range(start, start + pages.len() - 1);
                }
                true
            }
            Input::Wheel(dy) if inside(self.org.frame, self.pointer.0, self.pointer.1) => {
                self.org.scroll_by(dy);
                true
            }
            Input::Key(k, mods) => self.org_key(k, mods, n, m),
            _ => false,
        }
    }

    fn open_page(&mut self, i: usize, m: &dyn TextMeasure) {
        self.r.set_ribbon_tab(0, m);
        self.sync();
        if let Some(v) = self.r.view_mut() {
            v.go_to(i, None);
        }
    }

    fn org_key(&mut self, k: Key, mods: Mods, n: usize, m: &dyn TextMeasure) -> bool {
        match (k, mods.ctrl) {
            (Key::Delete, _) => self.command("org.delete", m),
            (Key::Letter('a'), true) => self.org.select_range(0, n.saturating_sub(1)),
            (Key::Left | Key::Up | Key::Right | Key::Down | Key::Home | Key::End, false) => {
                self.org.step(k, mods.shift, n);
            }
            (Key::Enter, _) => {
                if let Some(&i) = self.org.selected.iter().next() {
                    self.open_page(i, m);
                }
            }
            _ => return false,
        }
        true
    }

    /// Bring page images up to date; true when more work remains.
    pub fn prepare(&mut self, store: &mut dyn PdfStore) -> bool {
        if self.organize_on() {
            if let Some(v) = self.r.view_mut() {
                let n = v.page_count();
                let sizes: Vec<(f32, f32)> = (0..n).map(|i| v.page_size(i)).collect();
                self.org.layout(&sizes);
                for (i, s) in self.org.visible_thumbs(&sizes) {
                    v.want_thumb(i, s);
                }
            }
        }
        self.r.prepare(store)
    }

    /// Paint the window.
    pub fn paint(&mut self, p: &mut dyn Painter) {
        self.r.paint(p);
        if self.organize_on() {
            if let (Some(v), Some(base)) = (self.r.view(), self.r.key_base()) {
                let labels: Vec<String> = (0..v.page_count())
                    .map(|i| {
                        v.doc()
                            .page_info(i)
                            .map_or_else(|| (i + 1).to_string(), |p| p.label.clone())
                    })
                    .collect();
                self.org.paint(p, v, base, &labels, self.r.theme());
            }
        }
        if let Some(tool) = self.tool {
            if self.r.showing_doc() && !self.organize_on() {
                let accent = self.r.theme().accent;
                let caret_on = self.caret_on;
                if let Some(v) = self.r.view_mut() {
                    self.edit.paint(p, v, accent, tool, caret_on);
                }
            }
        }
        self.paint_extras(p);
        self.paint_sign(p);
        self.paint_comment(p);
        self.paint_format(p);
        if let Some((d, _)) = &self.dialog {
            d.paint(p, self.r.theme(), self.r.window_size(), true);
        }
        if let Some((d, _)) = &self.dialog2 {
            d.paint(p, self.r.theme(), self.r.window_size(), true);
        }
    }
}

fn image_filters() -> Vec<(String, String)> {
    vec![("Images (*.png; *.jpg)".into(), "*.png;*.jpg;*.jpeg".into())]
}

fn pdf_filters() -> Vec<(String, String)> {
    vec![("PDF (*.pdf)".into(), "*.pdf".into())]
}

/// "p3" or "p2-5" style label of a page list (1-based).
fn page_list_label(pages: &[usize]) -> String {
    match (pages.iter().min(), pages.iter().max()) {
        (Some(a), Some(b)) if a == b => format!("p{}", a + 1),
        (Some(a), Some(b)) => format!("p{}-{}", a + 1, b + 1),
        _ => "pages".into(),
    }
}

/// Text widths estimated from the character count (dialog layout before the host measures).
pub(crate) struct Estimate;

impl TextMeasure for Estimate {
    fn measure(&self, text: &str, style: crate::TextStyle) -> (f32, f32) {
        (
            text.chars().count() as f32 * style.size * 0.55,
            style.size * 1.33,
        )
    }
}

pub(crate) fn inside(r: [f32; 4], x: f32, y: f32) -> bool {
    x >= r[0] && x < r[0] + r[2] && y >= r[1] && y < r[1] + r[3]
}
