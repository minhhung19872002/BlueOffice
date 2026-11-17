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
pub(crate) const TABS: [&str; 9] = [
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

/// Ribbon items of tab `tab` (an index into the tabs after File).
pub(crate) fn ribbon_items(tab: usize) -> Vec<(&'static str, Kind)> {
    ribbon(tab)
}

#[allow(clippy::too_many_lines)]
fn ribbon(tab: usize) -> Vec<(&'static str, Kind)> {
    use Kind::{Big, Row, Sep, Small};
    let common = [
        ("tool.hand", Big("hand", "pdfReader.hand", false)),
        // The reference's Select has a menu: text and images, or comments (the editor's own
        // command: the reader's `tool.select` has none).
        ("ed.selectMenu", Big("select", "pdfReader.select", true)),
    ];
    let mut v: Vec<(&'static str, Kind)> = common.to_vec();
    match tab {
        // The reference's Home tab (SnapShot is BlueReader's); From Scanner and Quick
        // Recognition are shown disabled so the groups keep their places.
        0 => v.extend([
            ("home.snapshot", Big("snapshot", "pdfHome.snapshot", false)),
            (
                "ed.clipboard",
                Big("clipboard", "pdfEditor.clipboard", true),
            ),
            ("ed.bookmark", Big("bookmark", "pdfEditor.bookmark", false)),
            ("sep1", Sep),
            ("menu.zoom", Big("zoom", "pdfReader.zoom", true)),
            ("menu.fit", Big("fit", "pdfEditor.pageFitOption", true)),
            ("view.reflow", Big("reflow", "pdfView.reflow", false)),
            ("menu.rotate", Big("rotate", "pdfReader.rotateView", true)),
            ("sep2", Sep),
            (
                "ed.editText",
                Big("edEditText", "pdfEditor.editText", false),
            ),
            (
                "ed.editObject",
                Big("editObject", "pdfEditor.editObject", true),
            ),
            ("sep3", Sep),
            // The same Typewriter as the Comment tab's: typed in place (`editor_typewriter`).
            (
                "cm.typewriter",
                Big("typewriter", "pdfReader.typewriter", false),
            ),
            (
                "tool.highlight",
                Big("highlight", "pdfReader.highlight", false),
            ),
            ("sep4", Sep),
            (
                "org.rotateMenu",
                Big("edRotatePages", "pdfEditor.rotatePages", true),
            ),
            (
                "org.insertMenu",
                Big("edInsertPage", "pdfEditor.insert", true),
            ),
            ("sep5", Sep),
            ("ed.scanner", Big("scanner", "pdfEditor.fromScanner", true)),
            ("ed.ocr", Big("ocr", "pdfEditor.quickRecognition", false)),
            ("sep6", Sep),
            ("form.fill", Big("fillSign", "pdfEditor.fillSign", false)),
            // The reference closes the last group with a separator too.
            ("sep7", Sep),
        ]),
        // The reference's Convert tab; From Web Page and text recognition stay grey.
        1 => v.extend([
            ("sep1", Sep),
            ("conv.create", Big("createPdf", "pdfEditor.fromFiles", true)),
            (
                "conv.scanner",
                Big("scanner", "pdfEditor.fromScanner", true),
            ),
            (
                "conv.clipboard",
                Big("fromClipboard", "pdfEditor.fromClipboard", false),
            ),
            ("conv.web", Big("fromWeb", "pdfEditor.fromWebPage", false)),
            ("conv.form", Big("formPdf", "pdfEditor.formFromFile", true)),
            (
                "conv.portfolio",
                Big("portfolio", "pdfEditor.portfolio", true),
            ),
            ("conv.combine", Big("combine", "pdfEditor.combine", false)),
            ("conv.blank", Big("blankPage", "pdfEditor.blank", false)),
            (
                "conv.templateMenu",
                Big("template", "pdfEditor.fromTemplate", false),
            ),
            ("sep2", Sep),
            (
                "conv.exportImages",
                Big("exportImages", "pdfEditor.exportImages", false),
            ),
            ("sep3", Sep),
            (
                "conv.officeMenu",
                Big("toOffice", "pdfEditor.toOffice", true),
            ),
            ("conv.imageMenu", Big("toImage", "pdfEditor.toImage", true)),
            ("conv.html", Big("toHtml", "pdfEditor.toHtml", false)),
            ("conv.otherMenu", Big("toOther", "pdfEditor.toOther", true)),
            ("sep4", Sep),
            (
                "conv.ocr",
                Big("recognizeText", "pdfEditor.recognizeText", true),
            ),
            (
                "conv.quickOcr",
                Big("ocr", "pdfEditor.quickRecognition", false),
            ),
            (
                "conv.suspect",
                Big("suspect", "pdfEditor.suspectResults", true),
            ),
            ("sep5", Sep),
            (
                "conv.preflight",
                Big("validate", "pdfEditor.preflight", false),
            ),
            ("sep6", Sep),
        ]),
        2 => v.extend(crate::editor_format::edit_tab_items()),
        // The reference's Organize tab: row buttons in two columns of three.
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
            (
                "org.rearrange",
                Big("rearrange", "pdfEditor.rearrange", false),
            ),
            ("org.move", Row("movePages", "pdfEditor.move", false)),
            (
                "org.duplicate",
                Row("duplicatePages", "pdfEditor.duplicate", false),
            ),
            (
                "org.replace",
                Row("replacePages", "pdfEditor.replace", false),
            ),
            ("org.split", Row("splitPages", "pdfEditor.split", false)),
            ("org.swap", Row("swapPages", "pdfEditor.swap", false)),
            (
                "org.interleave",
                Row("interleave", "pdfEditor.interleave", false),
            ),
            ("sep2", Sep),
            (
                "org.rotateMenu",
                Big("edRotatePages", "pdfEditor.rotatePages", true),
            ),
            ("org.crop", Big("cropPages", "pdfEditor.cropPages", false)),
            (
                "org.resize",
                Big("resizePages", "pdfEditor.resizePages", false),
            ),
            ("sep3", Sep),
            ("org.flatten", Big("flatten", "pdfEditor.flatten", false)),
            ("sep4", Sep),
            (
                "org.watermark",
                Big("watermark", "pdfEditor.watermark", true),
            ),
            (
                "org.background",
                Big("background", "pdfEditor.background", true),
            ),
            (
                "org.headerFooter",
                Big("headerFooter", "pdfEditor.headerFooter", true),
            ),
            ("org.bates", Big("bates", "pdfEditor.bates", true)),
            (
                "org.pageNumbers",
                Big("pageNumbers", "pdfEditor.pageNumbers", false),
            ),
            ("sep5", Sep),
        ]),
        // The reference's Comment tab: text markup as a 3 × 2 grid, row buttons for the
        // drawing tools, stamps and comment data.
        4 => v.extend([
            ("sep1", Sep),
            ("tool.highlight", Small("highlight")),
            ("tool.strikeout", Small("strikeout")),
            ("tool.squiggly", Small("squiggly")),
            ("comment.replace", Small("replaceText")),
            ("tool.underline", Small("underline")),
            ("comment.insert", Small("insertText")),
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
            ("cm.drawing", Row("drawing", "pdfEditor.drawing", true)),
            ("cm.pencil", Row("pencil", "pdfEditor.pencil", false)),
            ("comment.eraser", Row("eraser", "pdfComment.eraser", false)),
            ("sep5", Sep),
            (
                "comment.areaHighlight",
                Big("areaHighlight", "pdfComment.areaHighlight", false),
            ),
            (
                "comment.searchHighlight",
                Big("searchHighlight", "pdfComment.searchHighlight", false),
            ),
            ("sep6", Sep),
            ("menu.measure", Big("measure", "pdfDraw.measure", true)),
            (
                "cm.calculator",
                Big("calculator", "pdfEditor.calculator", false),
            ),
            ("sep7", Sep),
            ("cm.stamp", Row("stamp", "pdfEditor.stamp", true)),
            (
                "menu.customStamp",
                Row("createStamp", "pdfEditor.createStamp", true),
            ),
            ("sep8", Sep),
            ("cm.summary", Row("summary", "pdfEditor.summarize", false)),
            ("cm.import", Row("import", "pdfComment.import", false)),
            ("cm.exportMenu", Row("export", "pdfComment.export", true)),
            ("sep9", Sep),
            (
                "panel.comments",
                Big("comments", "pdfReader.commentList", false),
            ),
            ("sep10", Sep),
        ]),
        // The reference editor's View tab: Read Mode, Reflow, Reverse View, Night Mode, Text
        // Viewer | Rotate View, Toggle Ruler | page display | AutoScroll | Page Transitions |
        // Read | Compare, Word Count | View Setting.
        5 => v.extend([
            ("sep1", Sep),
            ("view.readMode", Big("readMode", "pdfExtra.readMode", false)),
            ("view.reflow", Big("reflow", "pdfView.reflow", false)),
            ("view.reverse", Big("reverseView", "pdfView.reverse", false)),
            ("view.night", Big("night", "pdfView.night", false)),
            (
                "view.textViewer",
                Big("textViewer", "pdfView.textViewer", false),
            ),
            ("sep2", Sep),
            ("menu.rotate", Big("rotate", "pdfReader.rotateView", true)),
            ("view.rulers", Big("ruler", "pdfEditor.toggleRuler", false)),
            ("sep3", Sep),
            ("view.single", Small("single")),
            ("view.contFacing", Small("contFacing")),
            ("view.continuous", Small("continuous")),
            ("view.cover", Small("cover")),
            ("view.facing", Small("facing")),
            ("menu.split", Small("split")),
            ("sep4", Sep),
            (
                "view.autoScroll",
                Big("autoScroll", "pdfView.autoScroll", false),
            ),
            ("sep5", Sep),
            (
                "view.transitions",
                Big("transitions", "pdfEditor.pageTransitions", false),
            ),
            ("sep6", Sep),
            ("menu.read", Big("read", "pdfView.read", true)),
            ("sep7", Sep),
            ("view.compare", Big("compare", "pdfEditor.compare", false)),
            (
                "view.wordCount",
                Big("wordCount", "pdfView.wordCount", false),
            ),
            ("sep8", Sep),
            (
                "menu.viewSetting",
                Big("viewSetting", "pdfView.viewSetting", true),
            ),
            ("sep9", Sep),
        ]),
        6 => v.extend(crate::editor_design::form_tab_items()),
        // The reference's Protect tab (Sensitivity labels and Batch Encrypt are not offered).
        7 => v.extend([
            ("sep1", Sep),
            (
                "prot.redact",
                Big("redact", "pdfEditor.markRedaction", false),
            ),
            (
                "prot.whiteout",
                Big("whiteout", "pdfEditor.whiteout", false),
            ),
            (
                "prot.applyRedact",
                Big("applyRedact", "pdfEditor.applyRedactions", false),
            ),
            ("sep2", Sep),
            (
                "prot.hiddenData",
                Big("hiddenData", "pdfEditor.hiddenData", false),
            ),
            ("sep3", Sep),
            ("prot.secureMenu", Big("secure", "pdfEditor.secure", true)),
            ("sep4", Sep),
            (
                "prot.restrict",
                Big("restrictAccess", "pdfEditor.restrictAccess", false),
            ),
            (
                "prot.removeSecurity",
                Big("unsecure", "pdfEditor.removeSecurity", false),
            ),
            ("sep5", Sep),
            ("form.fill", Big("fillSign", "pdfEditor.fillSign", false)),
            ("prot.sign", Big("sign", "pdfEditor.signCertify", false)),
            (
                "prot.timestamp",
                Big("timestamp", "pdfEditor.timeStamp", false),
            ),
            (
                "prot.validate",
                Big("validate", "pdfProtect.validate", false),
            ),
            (
                "prot.sigPanel",
                Big("signPanel", "pdfEditor.signaturePanel", false),
            ),
            ("sep6", Sep),
        ]),
        // Round 9: the contextual Combine Files tab.
        9 => v = crate::editor_combine9::ribbon_items(),
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
            ("file.optimize", "pdfEditor.optimize"),
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
    status_h: 36.0,
    tab_h: 25.6,
    tab_gap_first: 24.0,
    tab_gap: 25.6,
    underline_up: 4.8,
    body_h: 73.6,
    icon: 28.8,
    icon_y: 4.8,
    label: 16.2,
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
    // Reference: status bar 45 px, boxes 28 px high 4 px below its edge (125 %).
    status_box_h: 22.4,
    status_box_top: Some(3.2),
    // The reference's toolbar icons are drawn with 1 px lines at 36 px.
    icon_stroke: 0.66,
    // Reference at 125 % with 110 % text: labels 9 % narrower than BlueReader's, buttons the
    // label plus 18 px, at least 49 px, side by side; 1 px before a separator, 8 px after
    // (Edit tab: Hand 14–65, Edit Text 131–179, separators at 123 and 470).
    big_pad: 13.1,
    big_min: 40.0,
    big_gap: 0.0,
    sep_space: (0.8, 5.8),
    // "Edit Object▾": the arrow widens the button (reference 180–248 for a 44 px label).
    arrow_w: 6.4,
    // One-line labels: about 12 px between neighbouring labels (Home: Select 111 →
    // SnapShot 123, Clipboard 270 → Bookmark 281).
    big_pad_one: 9.6,
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
    pub(crate) org: Organize,
    tool: Option<EditTool>,
    dialog: Option<(Dialog, Ask)>,
    pub(crate) requests: Vec<ReaderRequest>,
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
    /// Typewriter text being typed on the page.
    pub(crate) tw: Option<crate::editor_typewriter::TwDraft>,
    /// Tab to close once the host saved it (Yes in the close question).
    pub(crate) close_after_save: Option<usize>,
    /// The save-changes question being shown.
    pub(crate) close_box: Option<crate::editor_close::CloseBox>,
    /// The Page Transitions dialog (`editor_trans7`).
    pub(crate) trans: Option<crate::editor_trans7::TransDlg>,
    /// Round 8 classic dialogs (`editor_dlg8`), the top one last.
    pub(crate) form8: Vec<(crate::editor_form8::Form, crate::editor_dlg8::FormWhat)>,
    /// Password Protection permissions: print, copy, change, comment.
    pub(crate) perm8: (bool, bool, bool, bool),
    /// The Format pane shows a comment tool's properties (asked for with `cmt8.props`).
    pub(crate) comment_pane8: bool,
    /// The Format pane is open (else a collapsed strip, as the reference starts it).
    pub(crate) format_open8: bool,
    /// Saved Settings of the page design dialogs: kind (`hf`, `wm`, `bg`) → named values.
    pub(crate) presets8:
        std::collections::BTreeMap<&'static str, Vec<(String, crate::editor_dlg8b::Vals)>>,
    /// Password Protection choices kept with "Save the settings as a new policy".
    pub(crate) policy8: Option<crate::editor_dlg8b::Vals>,
    /// Picture picked as the watermark (File source).
    pub(crate) wm_image8: Option<Vec<u8>>,
    /// The presets file was read (`editor_dlg8b::presets_file`).
    pub(crate) presets_loaded8: bool,
    /// The Edit Object rotation handle being dragged (`editor_rot8`).
    pub(crate) rot8: Option<crate::editor_rot8::RotDrag>,
    /// Values last applied per page design kind, for Update (`editor_org9`).
    pub(crate) org9_last: std::collections::BTreeMap<&'static str, crate::editor_dlg8b::Vals>,
    /// The open page design dialog came from Update: replace that kind on OK.
    pub(crate) org9_update: Option<&'static str>,
    /// The clipboard picture asked for is for Insert › From Clipboard.
    pub(crate) clip_insert9: bool,
    /// Files of Combine Files (`editor_combine9`).
    pub(crate) combine9: Vec<crate::editor_combine9::CombineItem>,
    /// The Combine Files tab while it is open (round 9).
    pub(crate) comb9: Option<crate::editor_combine9::Comb9>,
    /// Combine Files › Settings options (kept for the session).
    pub(crate) comb9_opts: crate::editor_combine9::CombineOpts,
    /// Converting To PDF choices (round 9).
    pub(crate) conv9_settings: crate::editor_conv9b::ConvSettings,
    /// Form field tool.
    pub(crate) design: crate::editor_design::DesignState,
    /// Caret shown (blink phase).
    pub(crate) caret_on: bool,
    /// Forms, comments, redaction and signing state.
    pub(crate) x: crate::editor_actions::Extras,
    /// Dialog of `editor_actions`.
    pub(crate) dialog2: Option<(Dialog, crate::editor_actions::Ask2)>,
    /// Field whose option menu is open.
    pub(crate) pending_field: Option<bo_pdfread::FormField>,
    /// Edit tab box tools, spelling, bookmarks, command search (`editor_tools`).
    pub(crate) tools: crate::editor_tools::ToolsState,
    /// The File page.
    pub(crate) bs: crate::editor_backstage::Backstage,
    /// File › Preferences.
    pub(crate) prefs: Prefs,
    /// Pages copied or cut in the page grid (a PDF of them), for Ctrl+V in any open document.
    pub(crate) page_clip: Option<Vec<u8>>,
    /// Batch Encrypt files and passwords (`editor_five`).
    pub(crate) five: crate::editor_five::FiveState,
}

/// Preferences (File › Preferences): comment author, zoom and page display of opened
/// documents (empty: as the document asks), units of measurement.
#[derive(Debug, Clone)]
pub(crate) struct Prefs {
    pub author: String,
    pub zoom: String,
    pub layout: String,
    pub units: String,
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            author: std::env::var("USERNAME").unwrap_or_else(|_| "BlueEditor".into()),
            zoom: String::new(),
            layout: String::new(),
            units: "mm".into(),
        }
    }
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
        r.file_backstage = true;
        // Closing a changed document asks first (`editor_close`).
        r.close_guard = true;
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
            tw: None,
            close_after_save: None,
            trans: None,
            form8: Vec::new(),
            perm8: (true, true, true, true),
            comment_pane8: false,
            format_open8: false,
            presets8: std::collections::BTreeMap::new(),
            policy8: None,
            wm_image8: None,
            presets_loaded8: false,
            rot8: None,
            org9_last: std::collections::BTreeMap::new(),
            org9_update: None,
            clip_insert9: false,
            combine9: Vec::new(),
            comb9: None,
            comb9_opts: crate::editor_combine9::CombineOpts::default(),
            conv9_settings: crate::editor_conv9b::ConvSettings::default(),
            close_box: None,
            design: crate::editor_design::DesignState::default(),
            caret_on: true,
            x: crate::editor_actions::Extras::default(),
            dialog2: None,
            pending_field: None,
            tools: crate::editor_tools::ToolsState::default(),
            bs: crate::editor_backstage::Backstage::default(),
            prefs: Prefs::default(),
            page_clip: None,
            five: crate::editor_five::FiveState::default(),
        }
    }

    /// Open `doc` named `name` in a new tab.
    pub fn open(&mut self, doc: PdfDocument, name: &str, path: Option<std::path::PathBuf>) {
        self.edit.reset(self.r.view_mut());
        self.x = crate::editor_actions::Extras::default();
        self.r.open(doc, name, path);
        self.portfolio_opened();
        self.apply_view_prefs();
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

    /// Whether one of the window's dialogs is open.
    pub(crate) fn dialog_open(&self) -> bool {
        self.dialog.is_some()
            || self.dialog2.is_some()
            || self.trans.is_some()
            || !self.form8.is_empty()
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
        // Round 9: with the Crop Pages tool the page itself shows, to draw the crop box on.
        self.r.showing_doc()
            && self.r.ribbon_tab() == TAB_ORGANIZE
            && self.tools.tool != Some(crate::editor_tools::BoxTool::Crop)
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
            self.design.tool = None;
            self.tools.tool = None;
        }
        self.r.run_command(cmd, m);
        self.reflow_check();
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

    /// Pointer position (window pixels).
    pub(crate) fn pointer(&self) -> (f32, f32) {
        self.pointer
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

    pub(crate) fn doc_name(&self) -> String {
        self.r.path().and_then(|p| p.file_stem()).map_or_else(
            || "document".to_owned(),
            |s| s.to_string_lossy().into_owned(),
        )
    }

    /// Handle an input event; true when the window needs a new frame.
    pub fn handle(&mut self, input: Input, m: &dyn TextMeasure) -> bool {
        self.edit.object_lines = self.tool == Some(EditTool::Object);
        if let Input::Move { x, y } | Input::Drag { x, y } = input {
            self.pointer = (x, y);
        }
        if self.dialog.is_some() || self.dialog2.is_some() {
            return self.dialog_input(&input, m);
        }
        if self.close_box_input(&input)
            || self.trans_input(&input)
            || self.form8_input(&input)
            || self.rot8_input(&input)
        {
            return true;
        }
        if self.tw_input(&input, m) {
            return true;
        }
        if let Input::Blink = input {
            if self.edit.typing.is_some() {
                self.caret_on = !self.caret_on;
                return true;
            }
        }
        if self.portfolio_input(&input) {
            self.sync();
            return true;
        }
        if self.combine9_input(&input, m) {
            self.sync();
            return true;
        }
        let mut redraw = false;
        let mut used = false;
        let reader_tool = self.r.current_tool();
        let reader_extra = self.r.extra_tool_on();
        if self.backstage_input(&input) {
            return true;
        }
        // The command search box keeps the keyboard while its results list is open.
        if self.tools.search_on && matches!(input, Input::Text(_) | Input::Key(..)) {
            used = self.search_input(&input);
        }
        if !used && !self.r.modal_open() {
            used = self.search_input(&input);
            if !used {
                if let Input::Key(k, mods) = input {
                    used = self.key(k, mods, m);
                }
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
            if !used && self.design.tool.is_some() && self.r.showing_doc() && !self.organize_on() {
                used = self.design_input(&input);
            }
            if !used && self.tools.tool.is_some() && self.r.showing_doc() && !self.organize_on() {
                used = self.tools_input(&input);
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
            // A reader tool chosen on the ribbon (highlight, note, select…) turns the
            // editor's tools off.
            let now = self.r.current_tool();
            if (now != reader_tool && now != Tool::Hand)
                || (!reader_extra && self.r.extra_tool_on())
            {
                self.set_tool(None);
                self.comment.tool = None;
                self.design.tool = None;
                self.tools.tool = None;
            }
        }
        redraw |= self.drain(m);
        redraw |= self.reflow_check();
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
        // Clipboard while typing: copy and cut the selected text, paste plain text from
        // the host.
        if self.edit.typing.is_some() && mods.ctrl && matches!(k, Key::Letter('c' | 'x' | 'v')) {
            let sel = self
                .edit
                .typing
                .as_ref()
                .and_then(crate::editor_edit::Typing::selected_text);
            match k {
                Key::Letter('v') => self
                    .requests
                    .push(ReaderRequest::Command("host.pasteText".into())),
                _ => {
                    if let Some(s) = sel {
                        self.requests.push(ReaderRequest::Copy(s));
                        if k == Key::Letter('x') {
                            if let Some(v) = self.r.view_mut() {
                                self.edit.cut(v);
                            }
                        }
                    }
                }
            }
            self.caret_on = true;
            return true;
        }
        if self.edit.typing.is_some() && !(mods.ctrl && matches!(k, Key::Letter('s'))) {
            let used = match self.r.view_mut() {
                Some(v) => self.edit.key(v, k, mods),
                None => false,
            };
            if used {
                self.caret_on = true;
                return true;
            }
        }
        if self.object_key(k, mods) {
            return true;
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

    pub(crate) fn after_edit(&mut self) {
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
    pub(crate) fn target_pages(&self) -> Vec<usize> {
        if self.organize_on() && !self.org.selected.is_empty() {
            return self.selected_pages();
        }
        self.r
            .view()
            .map(|v| vec![v.current_page()])
            .unwrap_or_default()
    }

    pub(crate) fn insert_at(&self) -> usize {
        self.target_pages().last().map_or(0, |p| p + 1)
    }

    pub(crate) fn edit_result(&mut self, r: Result<(), String>) {
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
        if cmd == "file.backstage" {
            if self.bs.open {
                self.close_backstage();
            } else {
                self.open_backstage();
            }
            return;
        }
        if self.org9_command(cmd)
            || self.combine9_command(cmd)
            || self.conv9_command(cmd)
            || self.dlg8_command(cmd)
            || self.action(cmd, m)
            || self.search_command(cmd)
            || self.tools_command(cmd)
            || self.format_command(cmd)
            || self.sign_command(cmd)
            || self.comment_command(cmd)
            || self.edit_tab_command(cmd)
            || self.design_command(cmd)
            || self.more_command(cmd)
            || self.more2_command(cmd)
            || self.split_command(cmd)
            || self.image_command(cmd)
            || self.arrange_command(cmd)
            || self.five_command(cmd)
            || self.close_tab_command(cmd)
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
            "ed.editText" => {
                self.toggle_tool(EditTool::Text);
                // The reference shows the Edit tab while Edit Text is on.
                if self.tool == Some(EditTool::Text) && self.r.ribbon_tab() != 2 {
                    self.r.set_ribbon_tab(2, m);
                }
            }
            "ed.editObject" => self.toggle_tool(EditTool::Object),
            "ed.addText" => self.toggle_tool(EditTool::AddText),
            "prot.redact" => self.toggle_tool(EditTool::Redact),
            "tool.hand" | "tool.select" | "ed.selectMenu" => {
                self.edit.reset(self.r.view_mut());
                self.tool = None;
                self.comment.tool = None;
                self.design.tool = None;
                self.tools.tool = None;
                if cmd == "ed.selectMenu" {
                    self.r.set_tool(Tool::Select);
                    let items = [
                        ("ed.selTextImage", "pdfEditor.selTextImage"),
                        ("ed.selAnnot", "pdfEditor.selAnnot"),
                    ];
                    self.menu(&items, "ed.selectMenu");
                }
            }
            "ed.selTextImage" => self.r.set_tool(Tool::Select),
            // Comments are picked, moved and resized with the hand.
            "ed.selAnnot" => self.r.set_tool(Tool::Hand),
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
            "org.delete" | "org.deleteNow" => {
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
            "prot.password" => {
                // Passwords, what is allowed without the permissions password, AES-256.
                let mut d = Dialog::new(
                    DialogKind::Comment,
                    self.t("pdfEditor.secure"),
                    vec![
                        (self.t("pdfEditor.openPassword"), String::new()),
                        (self.t("pdfEditor.permissionsPassword"), String::new()),
                    ],
                    vec![
                        (self.t("pdfReader.ok"), DialogButton::Ok),
                        (self.t("pdfReader.cancel"), DialogButton::Cancel),
                    ],
                    Vec::new(),
                )
                .with_checks(vec![
                    (self.t("pdfEditor.allowPrint"), true),
                    (self.t("pdfEditor.allowCopy"), true),
                    (self.t("pdfEditor.allowModify"), false),
                    (self.t("pdfEditor.allowComments"), true),
                    (self.t("pdfEditor.allowAssemble"), false),
                    (self.t("pdfEditor.aes256"), true),
                ]);
                d.layout(m, self.r.window_size(), self.r.ui_scale());
                self.dialog = Some((d, Ask::Password));
            }
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
        self.design.tool = None;
        self.tools.tool = None;
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
        let Some((d, _)) = self.dialog2.as_mut() else {
            return true;
        };
        d.layout(m, size, scale);
        if let Some(b) = done {
            if let Some((d, what)) = self.dialog2.take() {
                if let crate::editor_actions::Ask2::Tools(t) = what {
                    self.tools_answer(t, b, &d.values());
                } else if let crate::editor_actions::Ask2::Five(t) = what {
                    self.five_answer(t, b, &d.values());
                } else if b == DialogButton::Ok {
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
                let on = |i: usize| values.get(i).is_none_or(|v| v == "1");
                let allow = bo_pdfread::Permissions {
                    print: on(2),
                    copy: on(3),
                    modify: on(4),
                    annotate: on(5),
                    assemble: on(6),
                };
                let aes256 = on(7);
                let Some(v) = self.r.view_mut() else { return };
                let r = v.doc_mut().set_protection(&user, &owner, allow, aes256);
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
        if self.conv9_rendered(purpose, &pngs) || self.conv9b_rendered(purpose, &pngs) {
            return;
        }
        let stem = self.doc_name();
        if purpose == "slides" {
            self.slides_rendered(pngs);
            return;
        }
        if purpose == "jpeg" {
            self.jpeg_rendered(&pngs);
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
        if self.org9_file_picked(purpose, bytes)
            || self.combine9_file_picked(purpose, &name, bytes)
            || self.pages9_file_picked(purpose, &name, bytes)
        {
            return;
        }
        if purpose == "batchPrintAdd" {
            self.batch_print_picked(path, bytes);
            return;
        }
        if purpose == "pasteText" {
            let text = String::from_utf8_lossy(bytes).into_owned();
            if let Some(v) = self.r.view_mut() {
                self.edit.paste_text(v, &text);
            }
            return;
        }
        if purpose == "certificate" {
            self.certificate_picked(bytes);
            return;
        }
        if self.dlg8_file_picked(purpose, &name, bytes)
            || self.five_file_picked(purpose, &name, bytes)
            || self.more_file_picked(purpose, bytes)
            || self.more2_file_picked(purpose, &name, bytes)
        {
            return;
        }
        if self.tools_file_picked(purpose, &name, bytes) {
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
            Input::Context { x, y } if inside(area, x, y) && tool == EditTool::Object => {
                self.object_menu(x, y);
                true
            }
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
        if self.tool.is_none()
            && self.comment.tool.is_none()
            && self.r.current_tool() == Tool::Select
        {
            self.r.on.insert("ed.selectMenu");
        }
        for id in ["ed.scanner", "ed.ocr"] {
            self.r.enabled_override.insert(id, false);
        }
        if self.comb9.is_some() {
            for (id, _) in crate::editor_combine9::ribbon_items() {
                if let Some(on) = self.combine9_enabled(id) {
                    self.r.enabled_override.insert(id, on);
                }
            }
        }
        // Round 4: commands shown grey with a tooltip; the reader's Time Stamp works here.
        for (id, _) in crate::editor_more::NOT_OFFERED
            .iter()
            .chain(crate::editor_more2::NOT_OFFERED2.iter())
        {
            self.r.enabled_override.insert(*id, false);
        }
        self.r.enabled_override.insert("prot.timestamp", doc);
        for id in crate::editor_format::EDIT_TAB_LATER
            .iter()
            .chain(&crate::editor_design::FORM_TAB_LATER)
        {
            self.r.enabled_override.insert(id, false);
        }
        if let Some(k) = self.design.tool {
            use bo_pdfread::NewFieldKind as F;
            self.r.on.insert(match k {
                F::Text => "fd.text",
                F::Check => "fd.check",
                F::Radio => "fd.radio",
                F::Combo => "fd.combo",
                F::List => "fd.list",
                F::Button => "fd.button",
                F::Signature => "fd.signature",
                F::Image => "fd.image",
                F::Barcode => "fd.barcode",
            });
        }
        if let Some(t) = self.tools.tool {
            use crate::editor_tools::BoxTool as B;
            self.r.on.insert(match t {
                B::Link => "ed.link",
                B::CrossRef => "ed.crossRef",
                B::Article => "ed.articleBox",
                B::ImageAnnot => "ed.imageAnnot",
                B::Media => "ed.media",
                B::Model3d => "ed.add3d",
                B::Shape(_) => "ed.addShapes",
                B::Whiteout => "prot.whiteout",
                B::Join => "ed.linkJoin",
                B::Crop => "org.crop",
            });
        }
        if self.tools.reflow {
            self.r.on.insert("ed.reflow");
        }
        self.r.tool_hidden = self.tool.is_some()
            || self.comment.tool.is_some()
            || self.design.tool.is_some()
            || self.tools.tool.is_some();
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
            Input::Press { x, y, .. } if self.org.slider_press(x, y) => true,
            Input::Drag { x, .. } if self.org.sliding => {
                self.org.slide_to(x);
                true
            }
            Input::Release { .. } if self.org.sliding => {
                self.org.sliding = false;
                true
            }
            Input::Zoom(n) if inside(self.org.frame, self.pointer.0, self.pointer.1) => {
                let z = self.org.zoom() * 1.1_f32.powf(n);
                self.org.set_zoom(z);
                true
            }
            Input::Press { x, y, count, shift } if inside(self.org.frame, x, y) => {
                if let Some(i) = self.org.press(x, y, shift, self.edit.mods.ctrl) {
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
            Input::Release { x, y }
                if self.org.dragging() && self.org.drag_moved() && {
                    let t = self.r.doc_tab_at(x, y);
                    t.is_some() && t != Some(self.r.active_tab())
                } =>
            {
                // Dropped on another document's tab: copy the pages to its end.
                self.org.cancel_drag();
                if let Some(t) = self.r.doc_tab_at(x, y) {
                    self.copy_pages_to_tab(t);
                }
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
            (Key::Delete, _) => self.command("org.deleteNow", m),
            (Key::Letter('a'), true) => self.org.select_range(0, n.saturating_sub(1)),
            (Key::Letter('c'), true) => self.copy_pages(false),
            (Key::Letter('x'), true) => self.copy_pages(true),
            (Key::Letter('v'), true) => self.paste_pages(),
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
        let closed = self.close_pending();
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
        // Round 8: the Watermark / Background preview shows the page itself.
        if let Some((i, s)) = self.preview_want8() {
            if let Some(v) = self.r.view_mut() {
                v.want_preview(i, s);
            }
        }
        // Round 9: selected text of the paragraph being typed goes into the page image.
        self.sync_text_mark9();
        self.r.prepare(store) || closed
    }

    /// Paint the window.
    pub fn paint(&mut self, p: &mut dyn Painter) {
        self.edit.object_lines = self.tool == Some(EditTool::Object);
        self.r.paint(p);
        self.paint_portfolio(p);
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
            // Round 8: a ribbon menu stays over the page grid (it was cut at the tabs).
            self.r.paint_open_menu(p);
        }
        if self.comb9.is_some() {
            self.paint_combine9(p);
            self.r.paint_open_menu(p);
        }
        if let Some(tool) = self.tool {
            if self.r.showing_doc() && !self.organize_on() {
                let accent = self.r.theme().accent;
                let caret_on = self.caret_on;
                if let Some(v) = self.r.view_mut() {
                    self.edit.paint(p, v, accent, tool, caret_on);
                }
                self.paint_rot8(p);
            }
        }
        self.paint_extras(p);
        self.paint_sign(p);
        self.paint_comment(p);
        self.paint_tw(p);
        self.paint_design(p);
        self.paint_tools(p, &Estimate);
        self.paint_format(p);
        self.paint_more_tip(p);
        self.paint_backstage(p);
        // Round 7: every BlueEditor dialog has the reference editor's caption and buttons.
        let (size, scale) = (self.r.window_size(), self.r.ui_scale());
        for d in [
            self.dialog.as_mut().map(|d| &mut d.0),
            self.dialog2.as_mut().map(|d| &mut d.0),
        ]
        .into_iter()
        .flatten()
        {
            if !d.has_caption() {
                d.set_caption();
                d.layout(&Estimate, size, scale);
            }
        }
        if let Some((d, _)) = &self.dialog {
            d.paint(p, self.r.theme(), self.r.window_size(), true);
        }
        if let Some((d, _)) = &self.dialog2 {
            d.paint(p, self.r.theme(), self.r.window_size(), true);
        }
        self.paint_trans(p);
        self.paint_form8(p);
        self.paint_close_box(p);
    }
}

pub(crate) fn image_filters() -> Vec<(String, String)> {
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
