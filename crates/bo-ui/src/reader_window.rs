//! BlueReader window: title bar with quick access and search, ribbon tabs (Home, Comment, View,
//! Help) with large tool buttons, document tabs, navigation panel (bookmarks, page thumbnails,
//! comments), the document view and the status bar with page navigation, page layouts and zoom.
//!
//! Sizes, greys and positions follow the reference desktop PDF reader by black-box measurement
//! at 125 % (`docs/ui/REPORT-reader.md`); icons and the accent colour are our own.
use crate::dialog::{Dialog, DialogButton, DialogKind};
use crate::popup::{Choice, Menu, MenuItem, MenuLook};
use crate::reader_icons::{self, Ink};
use crate::reader_view::{
    Fit, Layout, PdfStore, ReaderView, Selection, HIT, HIT_CURRENT, SELECTION, THUMB_KEY,
};
use crate::strings::Strings;
use crate::theme::Theme;
use crate::titlebar::NcHit;
use crate::{Input, Key, Mods, Painter, Request, TextMeasure, TextStyle};
use bo_pdfread::{AnnotKind, LinkTarget, OutlineItem, PdfDocument};
use bo_render::{PathSeg, Rgba};
use std::collections::HashSet;

// Print dialog, Read Mode, Full Screen and comment editing (BlueReader round 2).
#[path = "reader_extra.rs"]
mod extra;
// Night Mode, Reflow, AutoScroll, cover page, rulers and grid (BlueReader round 3).
#[path = "reader_viewx_win.rs"]
mod viewx;
// Drawing tools, resize handles and the properties panel (BlueReader round 3).
#[path = "reader_draw_win.rs"]
mod draw;
// Text Viewer, split panes, Read aloud, smooth AutoScroll (BlueReader round 4).
#[path = "reader_view4_win.rs"]
mod view4;
// Form and Protect tabs, Eraser, Area Highlight, Custom Stamp, comment list order (round 4).
#[path = "reader_tabs4_win.rs"]
mod tabs4;

// Helpers used by BlueEditor (tool hand-off, status text in tests).
#[path = "editor_rows_win.rs"]
mod rows;
// BlueEditor round 7: Skins of the File page over the editor's own look.
#[path = "editor_skin7_win.rs"]
mod edskin7;

#[path = "reader_tabs5_win.rs"]
mod tabs5;

#[path = "reader_bookmarks5_win.rs"]
mod bm5;

#[path = "reader_props5_win.rs"]
mod props5;

#[path = "reader_backstage5_win.rs"]
mod bs5;
// Preferences, Batch Print (BlueReader round 6).
#[path = "reader_dark9_win.rs"]
mod dark9;
#[path = "reader_keys7_win.rs"]
mod keys7;
#[path = "reader_marks9_win.rs"]
mod marks9;
#[path = "reader_menus9_win.rs"]
mod menus9;
// Pop-up notes on the page, the selection toolbar, Paste (BlueReader round 10).
#[path = "reader_panels8_win.rs"]
mod panels8;
#[path = "reader_popups10_win.rs"]
mod popups10;
#[path = "reader_prefs6_win.rs"]
mod prefs6;
#[path = "reader_search9_win.rs"]
mod search9;
#[path = "reader_skins8_win.rs"]
mod skins8;
#[path = "reader_tabs9_win.rs"]
mod tabs9;

/// Measured greys of the reference reader at 125 %.
const CHROME: Rgba = Rgba(251, 251, 253, 255);
const EDGE: Rgba = Rgba(199, 204, 217, 255);
const CANVAS: Rgba = Rgba(240, 241, 245, 255);
const NAV_BG: Rgba = Rgba(245, 246, 250, 255);
const STRIP: Rgba = Rgba(230, 233, 240, 255);
const PAGE_EDGE: Rgba = Rgba(119, 120, 122, 255);
const TEXT: Rgba = Rgba(51, 51, 51, 255);
const MUTED: Rgba = Rgba(150, 150, 151, 255);
const PRESSED: Rgba = Rgba(230, 233, 240, 255);
const HOVER: Rgba = Rgba(238, 240, 245, 255);
const TRACK: Rgba = Rgba(240, 240, 240, 255);
const THUMB: Rgba = Rgba(133, 133, 133, 255);
const WHITE: Rgba = Rgba(255, 255, 255, 255);
const CLOSE_BG: Rgba = Rgba(0xC4, 0x2B, 0x1C, 255);
/// Page thumbnails measured on the reference at 125 %: pages 126 px wide in a 2 px frame,
/// 200 px apart for Letter pages (163 px high).
const THUMB_W: f32 = 100.8;
const THUMB_GAP: f32 = 29.6;
const THUMB_EDGE: Rgba = Rgba(207, 207, 208, 255);

/// Pointer tools.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    /// Pan; selects text when pressed over text.
    Hand,
    /// Select text.
    Select,
    /// Text markup comments.
    Markup(AnnotKind),
    /// Sticky note.
    Note,
    /// Text box (typewriter).
    TextBox,
    /// Rectangle.
    Rectangle,
}

/// Navigation panels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Panel {
    /// Outline.
    Bookmarks,
    /// Page thumbnails.
    Pages,
    /// Comment list.
    Comments,
    /// A panel of a window built on this one ([`ExtraPanel`], BlueEditor's signatures).
    Extra,
    /// Optional content groups (BlueReader, shown when the document has layers).
    Layers,
    /// Files attached to the document and its pages (BlueReader).
    Attachments,
}

/// A navigation panel supplied by a window built on this one: its column icon and rows.
#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct ExtraPanel {
    /// Command of its navigation column icon.
    pub nav_id: &'static str,
    /// Icon name.
    pub icon: &'static str,
    /// Panel title.
    pub title: String,
    /// Text shown when there are no rows.
    pub empty: String,
    /// Rows: status colour, title, detail line, command run when clicked.
    pub rows: Vec<(Rgba, String, String, String)>,
}

/// What the window asks of the host.
#[derive(Debug, Clone, PartialEq)]
pub enum ReaderRequest {
    /// Pick a PDF (or open this one) in a new tab.
    Open(Option<std::path::PathBuf>),
    /// Write the active document to its file.
    Save,
    /// Pick a name and write the active document there.
    SaveAs,
    /// Print the active document.
    Print,
    /// Put text on the clipboard.
    Copy(String),
    /// Open an address in the browser.
    OpenUrl(String),
    /// A window operation.
    Window(Request),
    /// Store the preferences (`key=value` lines, File › Preferences).
    SavePreferences(String),
    /// Print with these settings (from the Print dialog, [ReaderWindow::open_print]).
    PrintPages(crate::reader_print::PrintSettings),
    /// A command this window does not know (handled by a window built on it, BlueEditor).
    Command(String),
    /// Pick a file for `purpose`; the host passes its bytes back (BlueEditor `file_picked`).
    PickFile {
        /// What the file is for (`insert`, `merge`, `create`, `image`).
        purpose: String,
        /// Dialog filters: description, pattern.
        filters: Vec<(String, String)>,
    },
    /// Ask where to write `bytes` (suggested file name) and write them.
    SaveBytes {
        /// Suggested file name.
        name: String,
        /// File contents.
        bytes: Vec<u8>,
    },
    /// Ask for a place and write several files (suggested names) next to each other.
    SaveMany(Vec<(String, Vec<u8>)>),
    /// Render pages to PNG images for `purpose` (`images`, `slides`); the host passes them
    /// back (BlueEditor `pages_rendered`).
    RenderPages {
        /// What the images are for.
        purpose: String,
        /// Pixels per point.
        scale: f32,
    },
    /// Open an attachment (not a PDF) with the program Windows associates with its type; the
    /// window has asked first and only passes documents and pictures.
    OpenAttachment {
        /// File name.
        name: String,
        /// File contents.
        bytes: Vec<u8>,
    },
    /// Open a protected file with a password.
    OpenWithPassword {
        /// The file.
        path: std::path::PathBuf,
        /// Its user or owner password.
        password: String,
    },
    /// Open a PDF made in memory (converted, new blank) in a new tab without a file yet.
    OpenDocument {
        /// Tab name.
        name: String,
        /// PDF bytes.
        bytes: Vec<u8>,
    },
    /// Show the driver settings of this printer (the Print dialog's Properties); the host
    /// keeps them for printing and passes the paper back ([`ReaderWindow::set_print_paper`]).
    PrinterProperties(String),
    /// Read text aloud (View > Read) at a rate from −10 to 10, replacing what is being read.
    Speak {
        /// Text to read.
        text: String,
        /// Rate −10…10.
        rate: i32,
    },
    /// Reading aloud: 0 stop, 1 pause, 2 resume, 10 + rate: change the rate.
    SpeechControl(i32),
    /// SnapShot: copy this part of a page (page points: left, top, right, bottom) to the
    /// clipboard as a picture.
    Snapshot {
        /// Page index.
        page: usize,
        /// The part.
        rect: [f32; 4],
    },
}

/// What a window built on this one changes (BlueEditor): app name, ribbon tabs and items, File
/// menu and a coloured title bar.
pub(crate) struct Profile {
    /// Product name in the title.
    pub app: &'static str,
    /// Ribbon tab label keys after File.
    pub tabs: Vec<&'static str>,
    /// Ribbon items of tab `i` (an index into `tabs`).
    pub items: fn(usize) -> Vec<(&'static str, Kind)>,
    /// File menu entries: command, label key.
    pub file_menu: Vec<(&'static str, &'static str)>,
    /// Title bar fill (white text and icons on it).
    pub title_band: Option<Rgba>,
    /// App mark icon in the title bar.
    pub app_icon: &'static str,
    /// Sizes and greys.
    pub look: Look,
}

/// Sizes (logical pixels; 125 % values / 1.25) and greys of the window, so a product built
/// on it (BlueEditor) can follow its own reference app.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Look {
    pub title_h: f32,
    pub ribbon_bottom: f32,
    pub doc_tabs_h: f32,
    pub status_h: f32,
    /// Ribbon tab row height, first label x, gap after File and between labels.
    pub tab_h: f32,
    pub tab_gap_first: f32,
    pub tab_gap: f32,
    /// Underline of the selected tab from the row bottom.
    pub underline_up: f32,
    /// Ribbon buttons: height, icon size and offset, label size (125 % px) and offset.
    pub body_h: f32,
    pub icon: f32,
    pub icon_y: f32,
    pub label: f32,
    pub label_y: f32,
    /// Quick access: first box x, step, box size.
    pub quick_x: f32,
    pub quick_step: f32,
    /// Title text centred in the window (else after the quick access icons).
    pub title_center: bool,
    /// Search box in the tab row at the right (x from the right edge, y, w, h); else in the
    /// title bar.
    pub search_in_tabs: Option<[f32; 4]>,
    /// Document tab width.
    pub doc_tab_w: f32,
    pub chrome: Rgba,
    pub edge: Rgba,
    pub canvas: Rgba,
    pub nav_bg: Rgba,
    pub pressed: Rgba,
    pub hover: Rgba,
    /// Document tab strip background.
    pub strip_bg: Rgba,
    /// Ribbon labels of several words always take two lines.
    pub wrap_words: bool,
    /// Placeholder of the search box (string key).
    pub search_hint: &'static str,
    /// Panel collapse strip fill, and whether lines frame it.
    pub strip: Rgba,
    pub strip_lines: bool,
    /// Border of the status bar's page and zoom boxes.
    pub box_edge: Rgba,
    /// Status bar text size (125 % px).
    pub status_font: f32,
    /// Space between the tab row and the ribbon buttons.
    pub body_gap: f32,
    /// Status bar page and zoom box height.
    pub status_box_h: f32,
    /// Top of those boxes below the status bar's edge (`None`: centred).
    pub status_box_top: Option<f32>,
    /// Stroke width of ribbon icons in icon grid units (24 to the icon).
    pub icon_stroke: f32,
    /// Large buttons: label padding, smallest width, gap after each; space before and after
    /// a group separator (logical pixels).
    pub big_pad: f32,
    pub big_min: f32,
    pub big_gap: f32,
    pub sep_space: (f32, f32),
    /// Width a drop-down arrow after a two-line label adds to its button.
    pub arrow_w: f32,
    /// Label padding of large buttons with a one-line label.
    pub big_pad_one: f32,
}

impl Look {
    /// BlueReader, measured against the reference reader (`docs/ui/REPORT-reader.md`).
    pub(crate) const READER: Look = Look {
        title_h: 56.0,
        ribbon_bottom: 160.0,
        doc_tabs_h: 35.2,
        status_h: 36.8,
        tab_h: 26.4,
        tab_gap_first: 22.4,
        tab_gap: 27.5,
        underline_up: 2.4,
        body_h: 72.8,
        icon: 25.6,
        icon_y: 6.4,
        label: 14.0,
        label_y: 28.8,
        quick_x: 52.0,
        quick_step: 32.0,
        title_center: false,
        search_in_tabs: None,
        doc_tab_w: 144.0,
        chrome: CHROME,
        edge: EDGE,
        canvas: CANVAS,
        nav_bg: NAV_BG,
        pressed: PRESSED,
        hover: HOVER,
        strip_bg: CANVAS,
        wrap_words: false,
        search_hint: "pdfReader.searchPlaceholder",
        strip: STRIP,
        strip_lines: false,
        box_edge: Rgba(175, 175, 176, 255),
        status_font: 12.5,
        body_gap: 0.8,
        status_box_h: 19.2,
        status_box_top: None,
        icon_stroke: 1.5,
        big_pad: 13.6,
        big_min: 41.6,
        big_gap: 1.6,
        sep_space: (4.8, 6.4),
        arrow_w: 0.0,
        big_pad_one: 13.6,
    };
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Kind {
    /// Title bar button with an icon.
    Quick(&'static str),
    /// Large ribbon button: icon, label key, drop-down arrow.
    Big(&'static str, &'static str, bool),
    /// Small icon button.
    Small(&'static str),
    /// A row of a three-row column: small icon, label key (empty: icon only), drop-down
    /// arrow.
    Row(&'static str, &'static str, bool),
    /// Navigation column icon.
    Nav(Panel),
    /// Window button.
    Win(NcHit),
    /// Separator line.
    Sep,
}

#[derive(Debug, Clone, Copy)]
struct Control {
    id: &'static str,
    kind: Kind,
    r: [f32; 4],
}

/// One open document.
struct DocTab {
    view: ReaderView,
    name: String,
    path: Option<std::path::PathBuf>,
    /// Expanded outline entries (paths of child indices).
    expanded: HashSet<Vec<usize>>,
    outline: Vec<OutlineItem>,
    panel: Option<Panel>,
    panel_scroll: f32,
    key_base: usize,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Drag {
    Pan { x: f32, y: f32, sx: f32, sy: f32 },
    Select { page: usize, a: usize },
    Shape { page: usize, x: f32, y: f32 },
    VBar { offset: f32 },
    Slider,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Pending {
    Note { page: usize, x: f32, y: f32 },
    TextBox { page: usize, rect: [f32; 4] },
}

/// BlueReader's menus at 100 % (see [`ReaderWindow::reader_menu`]).
pub(crate) const READER_MENU: MenuLook = MenuLook {
    item_h: 24.0,
    text: 9.6,
    text_x: (32.8, 32.8),
    icon_x: 8.0,
    icon: 16.0,
    pad: 4.0,
    min_w: 96.0,
    char_w: 7.7,
    fill: Some(Rgba(251, 251, 253, 255)),
    frame: Some(Rgba(195, 195, 195, 255)),
};

/// Window contents of BlueReader.
pub struct ReaderWindow {
    tabs: Vec<DocTab>,
    active: usize,
    next_key: usize,
    show_start: bool,
    strings: Strings,
    theme: Theme,
    size: (f32, f32),
    scale: f32,
    maximized: bool,
    ribbon_tab: usize,
    tool: Tool,
    controls: Vec<Control>,
    tab_rects: Vec<[f32; 4]>,
    doc_tab_rects: Vec<([f32; 4], [f32; 4])>,
    /// Closing a tab asks the owner first (a `Command("ed.closeTab.N")` request).
    pub(crate) close_guard: bool,
    start_tab: [f32; 4],
    search: [f32; 4],
    search_text: String,
    search_focus: bool,
    hover: Option<&'static str>,
    pressed: Option<&'static str>,
    menu: Option<(Menu, &'static str)>,
    dialog: Option<(Dialog, Option<Pending>)>,
    icons: crate::icons::Icons,
    /// Last pointer position (wheel over the panel scrolls it).
    pointer: (f32, f32),
    drag: Option<Drag>,
    requests: Vec<ReaderRequest>,
    cursor: u8,
    recent: Vec<std::path::PathBuf>,
    recent_rects: Vec<[f32; 4]>,
    author: String,
    status: String,
    page_box: [f32; 4],
    zoom_box: [f32; 4],
    slider: [f32; 4],
    vbar: [f32; 4],
    /// Doc area and navigation geometry.
    doc_rect: [f32; 4],
    nav_rect: [f32; 4],
    panel_rect: [f32; 4],
    strip_rect: [f32; 4],
    /// Measured text widths (label → width) for centring.
    widths: std::collections::HashMap<(String, u32, bool), f32>,
    /// Panel rows: (y, height, action) for hit testing.
    panel_rows: Vec<(f32, f32, PanelRow)>,
    /// Set by a window built on this one (BlueEditor).
    pub(crate) profile: Option<Profile>,
    /// Controls shown pressed in addition to the pointer tool, layout and panel.
    pub(crate) on: HashSet<&'static str>,
    /// Controls enabled or disabled regardless of the defaults.
    pub(crate) enabled_override: std::collections::HashMap<&'static str, bool>,
    /// The pointer tool is not shown pressed (another window's tool is active).
    pub(crate) tool_hidden: bool,
    /// The document area is drawn by a window built on this one (page grid).
    pub(crate) doc_hidden: bool,
    /// A panel of a window built on this one (its icon follows the comments icon).
    pub(crate) extra_panel: Option<ExtraPanel>,
    /// Outline entry last clicked in the Bookmarks panel (path of child indices; BlueEditor
    /// edits it).
    pub(crate) selected_outline: Option<Vec<usize>>,
    /// File opens a full-window page (BlueEditor's backstage) through the `file.backstage`
    /// command instead of a menu; `backstage_open` shows File as the selected tab.
    pub(crate) file_backstage: bool,
    pub(crate) backstage_open: bool,
    /// Width (logical pixels) of a pane at the right of the document drawn by a window built
    /// on this one (BlueEditor's Format panel); 0 for none.
    right_pane: f32,
    /// Sizes and greys.
    look: Look,
    /// Print dialog, Read Mode / Full Screen and the selected comment.
    extra: extra::Extra,
    /// View tab additions.
    viewx: viewx::ViewX,
    /// Comment tab drawing tools.
    draw: draw::Draw,
    /// View tab additions of round 4.
    view4: view4::View4,
    /// Form / Protect tabs and Comment tab additions of round 4.
    tabs4: tabs4::Tabs4,
    /// Fill & Sign, signing, comment exchange of round 5.
    tabs5: tabs5::Tabs5,
    prefs6: prefs6::Prefs6,
    /// Keyboard access to the ribbon (key tips, focus ring).
    keys7: keys7::KeyLevel,
    /// Layers / Attachments panels and the comment list of round 8.
    panels8: panels8::Panels8,
    /// Ribbon tooltips of round 9.
    nine: dark9::Nine,
    /// Advanced Search pane of round 9.
    search9: search9::Search9,
    /// Bookmark editing and thumbnail size of round 9.
    marks9: marks9::Marks9,
    /// Pop-up notes, the selection toolbar and Paste of round 10.
    ten: popups10::Ten,
}

#[derive(Debug, Clone, PartialEq)]
enum PanelRow {
    /// Outline entry: path, destination, x range of its expand box.
    Outline(Vec<usize>, Option<LinkTarget>, Option<(f32, f32)>),
    Page(usize),
    Comment(usize, [f32; 4]),
    /// A row of the extra panel: its command.
    Extra(String),
}

const COLOR_HIGHLIGHT: [u8; 3] = [255, 230, 0];
const COLOR_MARK: [u8; 3] = [229, 57, 53];
const COLOR_NOTE: [u8; 3] = [255, 214, 0];

impl ReaderWindow {
    /// An empty window (start page) in `locale`.
    pub fn new(locale: &str) -> Self {
        Self {
            tabs: Vec::new(),
            active: 0,
            next_key: 0,
            show_start: true,
            strings: Strings::load(locale),
            theme: Theme::light("reader"),
            size: (1200.0, 800.0),
            scale: 1.0,
            maximized: false,
            ribbon_tab: 0,
            tool: Tool::Hand,
            controls: Vec::new(),
            tab_rects: Vec::new(),
            doc_tab_rects: Vec::new(),
            close_guard: false,
            start_tab: [0.0; 4],
            search: [0.0; 4],
            search_text: String::new(),
            search_focus: false,
            hover: None,
            pressed: None,
            menu: None,
            dialog: None,
            icons: crate::icons::Icons::load(),
            pointer: (0.0, 0.0),
            drag: None,
            requests: Vec::new(),
            cursor: 3,
            recent: Vec::new(),
            recent_rects: Vec::new(),
            author: std::env::var("USERNAME").unwrap_or_else(|_| "BlueReader".into()),
            status: String::new(),
            page_box: [0.0; 4],
            zoom_box: [0.0; 4],
            slider: [0.0; 4],
            vbar: [0.0; 4],
            doc_rect: [0.0; 4],
            nav_rect: [0.0; 4],
            panel_rect: [0.0; 4],
            strip_rect: [0.0; 4],
            widths: std::collections::HashMap::new(),
            panel_rows: Vec::new(),
            profile: None,
            on: HashSet::new(),
            enabled_override: std::collections::HashMap::new(),
            tool_hidden: false,
            doc_hidden: false,
            extra_panel: None,
            selected_outline: None,
            file_backstage: false,
            backstage_open: false,
            right_pane: 0.0,
            look: Look::READER,
            extra: extra::Extra::default(),
            viewx: viewx::ViewX::default(),
            draw: draw::Draw::default(),
            view4: view4::View4::default(),
            tabs4: tabs4::Tabs4::default(),
            tabs5: tabs5::Tabs5::default(),
            prefs6: prefs6::Prefs6::default(),
            keys7: keys7::KeyLevel::Off,
            panels8: panels8::Panels8::default(),
            nine: dark9::Nine::default(),
            search9: search9::Search9::default(),
            marks9: marks9::Marks9::default(),
            ten: popups10::Ten::default(),
        }
    }

    /// A window with another product's ribbon and title (BlueEditor).
    pub(crate) fn with_profile(locale: &str, profile: Profile) -> Self {
        let mut w = Self::new(locale);
        w.author = std::env::var("USERNAME").unwrap_or_else(|_| profile.app.into());
        w.look = profile.look;
        w.profile = Some(profile);
        w
    }

    /// A drop-down or context menu. BlueReader's menus are measured on the reference reader at
    /// 125 % (rows 30 px, label 13 px at 41 px from the frame, 20 px icons at 10 px, frame
    /// 195 on 251, 251, 253); BlueEditor keeps the shared look.
    pub(crate) fn reader_menu(&self, items: Vec<MenuItem>, anchor: [f32; 4]) -> Menu {
        let look = if self.profile.is_none() {
            READER_MENU
        } else {
            MenuLook::default()
        };
        Menu::open_with(items, anchor, self.size, self.scale, look)
    }

    fn app_name(&self) -> &'static str {
        self.profile.as_ref().map_or("BlueReader", |p| p.app)
    }

    fn tab_label_keys(&self) -> Vec<&'static str> {
        let mut v = vec!["pdfReader.tabs.file"];
        match &self.profile {
            Some(p) => v.extend(p.tabs.iter().copied()),
            None => v.extend([
                "pdfReader.tabs.home",
                "pdfReader.tabs.comment",
                "pdfReader.tabs.view",
                "pdfReader.tabs.form",
                "pdfReader.tabs.protect",
                "pdfReader.tabs.share",
                "pdfReader.tabs.help",
            ]),
        }
        if self.profile.is_none() && self.tabs5.fill_mode {
            v.push("pdfFill.tab");
        }
        v
    }

    /// Document area in window pixels.
    pub(crate) fn doc_area(&self) -> [f32; 4] {
        self.doc_rect
    }

    /// Display scale (device pixels per logical pixel).
    pub(crate) fn ui_scale(&self) -> f32 {
        self.scale
    }

    /// Window size in pixels.
    pub(crate) fn window_size(&self) -> (f32, f32) {
        self.size
    }

    /// Image key base of the active document (page images are `base + index`).
    pub(crate) fn key_base(&self) -> Option<usize> {
        self.tabs.get(self.active).map(|t| t.key_base)
    }

    /// A document is shown (not the start page).
    pub(crate) fn showing_doc(&self) -> bool {
        !self.show_start && !self.tabs.is_empty()
    }

    /// Selected ribbon tab (0 = the first after File).
    pub(crate) fn ribbon_tab(&self) -> usize {
        self.ribbon_tab
    }

    /// Show ribbon tab `i` (0 = the first after File).
    pub(crate) fn set_ribbon_tab(&mut self, i: usize, m: &dyn TextMeasure) {
        self.ribbon_tab = i;
        self.relayout(m);
    }

    /// The navigation panel shown for the active document.
    pub(crate) fn panel(&self) -> Option<Panel> {
        self.panel_open()
    }

    /// Run a command as if its control was clicked.
    pub(crate) fn run_command(&mut self, cmd: &str, m: &dyn TextMeasure) {
        self.command(cmd, m);
    }

    /// The interface colours.
    pub(crate) fn theme(&self) -> &Theme {
        &self.theme
    }

    /// Use other interface colours (another product's accent).
    pub(crate) fn set_theme(&mut self, theme: Theme) {
        self.theme = theme;
    }

    /// Interface strings.
    pub(crate) fn strings(&self) -> &Strings {
        &self.strings
    }

    /// The document's pages or outline changed: read the outline again and lay out.
    pub(crate) fn document_changed(&mut self) {
        if let Some(t) = self.tabs.get_mut(self.active) {
            t.outline = t.view.doc().outline();
            t.expanded.clear();
            t.view.document_changed();
        }
        self.layout_doc();
    }

    /// Show or hide a navigation panel.
    pub(crate) fn set_panel(&mut self, p: Option<Panel>) {
        if let Some(t) = self.tabs.get_mut(self.active) {
            t.panel = p;
            t.panel_scroll = 0.0;
        }
        self.layout_doc();
    }

    /// Open a menu of commands under `r`.
    pub(crate) fn menu_at(&mut self, items: Vec<MenuItem>, r: [f32; 4], id: &'static str) {
        self.menu = Some((self.reader_menu(items, r), id));
    }

    /// Ribbon tab label boxes (File first), from the last layout.
    pub(crate) fn tab_boxes(&self) -> &[[f32; 4]] {
        &self.tab_rects
    }

    /// Bottom of the ribbon tab row.
    pub(crate) fn tabs_bottom(&self) -> f32 {
        self.tabs_y() + self.px(self.look.tab_h)
    }

    /// Close an open drop-down menu.
    pub(crate) fn close_menu(&mut self) {
        self.menu = None;
    }

    /// A menu or dialog of this window is open.
    pub(crate) fn modal_open(&self) -> bool {
        self.menu.is_some() || self.dialog.is_some()
    }

    /// Rectangle of control `id`.
    pub(crate) fn control_rect(&self, id: &str) -> Option<[f32; 4]> {
        self.controls.iter().find(|c| c.id == id).map(|c| c.r)
    }

    /// Open `doc` named `name` in a new tab and show it.
    pub fn open(&mut self, doc: PdfDocument, name: &str, path: Option<std::path::PathBuf>) {
        let outline = doc.outline();
        // The reference shows bookmarks when the document asks (/PageMode /UseOutlines).
        let panel = (!outline.is_empty() && doc.page_mode().as_deref() == Some("UseOutlines"))
            .then_some(Panel::Bookmarks);
        // ... and attachments when the document has embedded files (round 8, measured).
        let panel = panel.or_else(|| self.first_panel8(&doc));
        let mut expanded = HashSet::new();
        tabs5::open_items(&outline, &mut Vec::new(), &mut expanded);
        let view = ReaderView::new(doc);
        self.tabs.push(DocTab {
            view,
            name: name.to_owned(),
            path: path.clone(),
            expanded,
            outline,
            panel,
            panel_scroll: 0.0,
            key_base: self.next_key,
        });
        self.next_key += 1 << 26;
        self.active = self.tabs.len() - 1;
        self.show_start = false;
        self.apply_view_prefs();
        if let Some(p) = path {
            self.recent.retain(|r| r != &p);
            self.recent.insert(0, p);
            self.recent.truncate(12);
        }
        self.layout_doc();
    }

    /// Recent files shown on the start page.
    pub fn set_recent(&mut self, recent: Vec<std::path::PathBuf>) {
        self.recent = recent;
    }

    /// Recent files (for the host to remember).
    pub fn recent(&self) -> &[std::path::PathBuf] {
        &self.recent
    }

    /// The tool in use.
    pub(crate) fn current_tool(&self) -> Tool {
        self.tool
    }

    /// Choose the pointer tool.
    pub fn set_tool(&mut self, tool: Tool) {
        self.tool = tool;
    }

    /// Requests since the last call.
    pub fn take_requests(&mut self) -> Vec<ReaderRequest> {
        std::mem::take(&mut self.requests)
    }

    /// Pointer shape: `3` arrow, `4` text, `5` link hand, `6` move.
    pub fn cursor(&self) -> u8 {
        self.cursor
    }

    /// Window title text.
    pub fn title(&self) -> String {
        match self.tabs.get(self.active) {
            Some(t) if !self.show_start => format!("{} - {}", t.name, self.app_name()),
            _ => self.app_name().into(),
        }
    }

    /// The active document view.
    pub fn view(&self) -> Option<&ReaderView> {
        self.tabs.get(self.active).map(|t| &t.view)
    }

    /// The active document view, for saving.
    pub fn view_mut(&mut self) -> Option<&mut ReaderView> {
        self.tabs.get_mut(self.active).map(|t| &mut t.view)
    }

    /// File path of the active document.
    pub fn path(&self) -> Option<&std::path::Path> {
        self.tabs.get(self.active).and_then(|t| t.path.as_deref())
    }

    /// The active document was saved as `path`.
    pub fn set_path(&mut self, path: std::path::PathBuf) {
        if let Some(t) = self.tabs.get_mut(self.active) {
            t.name = path
                .file_name()
                .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
            t.path = Some(path);
        }
    }

    /// A status message (errors of the host).
    pub fn set_status(&mut self, s: &str) {
        s.clone_into(&mut self.status);
    }

    fn px(&self, v: f32) -> f32 {
        (v * self.scale).round()
    }

    fn t(&self, key: &str) -> String {
        self.strings.get(key).to_owned()
    }

    /// Text style with an em of `px125` device pixels as measured at 125 % with the 110 % text
    /// size (the host grows interface text by the text size factor).
    fn style(&self, px125: f32, bold: bool) -> TextStyle {
        TextStyle {
            size: px125 / 1.375 * self.scale,
            bold,
        }
    }

    fn measure(&mut self, m: &dyn TextMeasure, s: &str, st: TextStyle) -> f32 {
        let key = (s.to_owned(), (st.size * 100.0) as u32, st.bold);
        if let Some(w) = self.widths.get(&key) {
            return *w;
        }
        let w = m.measure(s, st).0;
        self.widths.insert(key, w);
        w
    }

    fn width_of(&self, s: &str, st: TextStyle) -> f32 {
        let key = (s.to_owned(), (st.size * 100.0) as u32, st.bold);
        self.widths
            .get(&key)
            .copied()
            .unwrap_or(s.chars().count() as f32 * st.size * 0.55)
    }

    // ---- Layout ------------------------------------------------------------------------

    fn title_h(&self) -> f32 {
        self.px(self.look.title_h)
    }

    fn tabs_y(&self) -> f32 {
        self.title_h() + 1.0
    }

    fn ribbon_bottom(&self) -> f32 {
        // Title 70 px, tabs + body to the line at 200 px (125 %).
        self.px(self.look.ribbon_bottom)
    }

    fn doc_tabs_h(&self) -> f32 {
        self.px(self.look.doc_tabs_h)
    }

    fn status_h(&self) -> f32 {
        self.px(self.look.status_h)
    }

    fn ribbon_items(&self) -> Vec<(&'static str, Kind)> {
        use Kind::{Big, Sep, Small};
        if let Some(p) = &self.profile {
            return (p.items)(self.ribbon_tab);
        }
        let common = [
            ("tool.hand", Big("hand", "pdfReader.hand", false)),
            ("tool.select", Big("select", "pdfReader.select", false)),
        ];
        let mut v: Vec<(&'static str, Kind)> = common.to_vec();
        match self.ribbon_tab {
            0 => v.extend([
                // The reference's Home tab: SnapShot, Clipboard | Zoom, Page Fit, Reflow,
                // Rotate View | Typewriter, Highlight | Fill & Sign.
                ("home.snapshot", Big("snapshot", "pdfHome.snapshot", false)),
                (
                    "menu.clipboard",
                    Big("clipboard", "pdfHome.clipboard", true),
                ),
                ("sep1", Sep),
                ("menu.zoom", Big("zoom", "pdfReader.zoom", true)),
                ("menu.fit", Big("fit", "pdfHome.pageFitOption", true)),
                ("view.reflow", Big("reflow", "pdfView.reflow", false)),
                ("menu.rotate", Big("rotate", "pdfReader.rotateView", true)),
                ("sep2", Sep),
                (
                    "tool.textbox",
                    Big("typewriter", "pdfReader.typewriter", false),
                ),
                (
                    "tool.highlight",
                    Big("highlight", "pdfReader.highlight", false),
                ),
                ("sep3", Sep),
                (
                    "prot.fillSign",
                    Big("fillSign", "pdfProtect.fillSign", false),
                ),
            ]),
            1 => v.extend(
                [
                    ("sep1", Sep),
                    // Text markup as a grid of small buttons, like the reference's Comment tab.
                    // Six text markup tools in a 3 × 2 grid like the reference's (filled column
                    // by column).
                    ("tool.highlight", Small("highlight")),
                    ("tool.strikeout", Small("strikeout")),
                    ("tool.squiggly", Small("squiggly")),
                    ("comment.replace", Small("replaceText")),
                    ("tool.underline", Small("underline")),
                    ("comment.insert", Small("insertText")),
                    ("sep2", Sep),
                    // Order of the reference's Comment tab: note | typewriter, text box, callout |
                    // drawing, pencil | measure | stamp.
                    ("tool.note", Big("note", "pdfReader.note", false)),
                    ("comment.file", Big("attach", "pdfComment.file", false)),
                    ("sep3", Sep),
                    (
                        "draw.typewriter",
                        Big("typewriter", "pdfDraw.typewriter", false),
                    ),
                    ("tool.textbox", Big("textbox", "pdfReader.textbox", false)),
                    ("draw.callout", Big("callout", "pdfDraw.callout", false)),
                    ("sep5", Sep),
                    ("menu.drawing", Big("rectangle", "pdfDraw.drawing", true)),
                    ("draw.pencil", Big("pencil", "pdfDraw.pencil", false)),
                    ("comment.eraser", Big("eraser", "pdfComment.eraser", false)),
                    ("sep8", Sep),
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
                    ("sep7", Sep),
                    ("menu.stamp", Big("stamp", "pdfDraw.stamp", true)),
                    (
                        "menu.customStamp",
                        Big("customStamp", "pdfComment.customStamp", true),
                    ),
                ]
                .into_iter()
                .chain(Self::comment_rows()),
            ),
            2 => v.extend([
                ("sep0", Sep),
                // Read Mode first after the pointer tools, like the reference's View tab.
                // Then Reflow and Night Mode, Rotate View, the page display grid and
                // AutoScroll, in the reference's order.
                ("view.readMode", Big("readMode", "pdfExtra.readMode", false)),
                ("view.reflow", Big("reflow", "pdfView.reflow", false)),
                ("view.reverse", Big("reverseView", "pdfView.reverse", false)),
                ("view.night", Big("night", "pdfView.night", false)),
                (
                    "view.textViewer",
                    Big("textViewer", "pdfView.textViewer", false),
                ),
                ("sep1", Sep),
                ("menu.rotate", Big("rotate", "pdfReader.rotateView", true)),
                ("sep2", Sep),
                // Page display as a 3 × 2 grid like the reference's.
                ("view.single", Small("single")),
                ("view.contFacing", Small("contFacing")),
                ("view.continuous", Small("continuous")),
                ("view.cover", Small("cover")),
                ("view.facing", Small("facing")),
                // The reference's sixth cell: split windows (the grid is in View Setting).
                ("menu.split", Small("split")),
                ("sep5", Sep),
                (
                    "view.autoScroll",
                    Big("autoScroll", "pdfView.autoScroll", false),
                ),
                ("sep6", Sep),
                ("menu.read", Big("read", "pdfView.read", true)),
                ("sep3", Sep),
                (
                    "view.wordCount",
                    Big("wordCount", "pdfView.wordCount", false),
                ),
                ("sep4", Sep),
                (
                    "menu.viewSetting",
                    Big("viewSetting", "pdfView.viewSetting", true),
                ),
            ]),
            3 | 4 => v.extend(Self::tabs4_items(self.ribbon_tab)),
            tabs5::FILL_TAB => v.extend(Self::fill_items()),
            tabs9::SHARE_TAB => v.extend(Self::share_items9()),
            _ => {
                v.clear();
                v.extend(Self::help_items9());
            }
        }
        v
    }

    fn relayout(&mut self, m: &dyn TextMeasure) {
        let (w, h) = self.size;
        self.controls.clear();
        let th = self.title_h();
        // Title bar quick access (125 %: icons 40 px apart from x 85).
        let qs = self.px(self.look.quick_step);
        let mut x = self.px(self.look.quick_x);
        for (id, icon) in [
            ("file.open", "open"),
            ("file.save", "save"),
            ("file.print", "print"),
            ("edit.undo", "undo"),
            ("edit.redo", "redo"),
        ] {
            self.controls.push(Control {
                id,
                kind: Kind::Quick(icon),
                r: [x, ((th - qs) / 2.0).round(), qs, qs],
            });
            x += qs;
        }
        // Window buttons, 52 px apart.
        let bw = self.px(41.6);
        for (i, (id, k)) in [
            ("win.min", NcHit::Minimize),
            ("win.max", NcHit::Maximize),
            ("win.close", NcHit::Close),
        ]
        .into_iter()
        .enumerate()
        {
            self.controls.push(Control {
                id,
                kind: Kind::Win(k),
                r: [w - bw * (3 - i) as f32, 0.0, bw, th],
            });
        }
        // Search box 458 × 40 px from x 580.
        let sw = self.px(366.4).min((w - self.px(560.0)).max(self.px(120.0)));
        self.search = match self.look.search_in_tabs {
            Some([right, y, sw, sh]) => [
                w - self.px(right) - self.px(sw),
                self.px(y),
                self.px(sw),
                self.px(sh),
            ],
            None => [self.px(464.0), self.px(16.0), sw, self.px(32.0)],
        };
        // Ribbon tabs: label + 35 px between labels, first at x 27.
        let tab_style = self.style(17.0, true);
        let labels = self.tab_label_keys();
        self.tab_rects.clear();
        let mut x = self.px(21.6);
        let ty = self.tabs_y();
        let tab_h = self.px(self.look.tab_h);
        for key in labels {
            let label = self.t(key);
            let lw = self.measure(m, &label, tab_style);
            self.tab_rects
                .push([x - self.px(4.0), ty, lw + self.px(8.0), tab_h]);
            // 125 %: 28 px after "File", 35–37 px between the other labels.
            x += lw
                + if key == "pdfReader.tabs.file" {
                    self.px(self.look.tab_gap_first)
                } else {
                    self.px(self.look.tab_gap)
                };
        }
        // Ribbon body: buttons 91 px high from y 104.
        let by = ty + tab_h + self.px(self.look.body_gap);
        let bh = self.px(self.look.body_h);
        let label_style = self.style(self.look.label, false);
        let mut x = self.px(11.2);
        let mut small_col = 0_usize;
        // Columns of three rows (round 5): rows in the column and its width.
        let (mut row_n, mut row_w) = (0_usize, 0.0_f32);
        for (id, kind) in self.ribbon_items() {
            if let Kind::Row(_, label, dropdown) = kind {
                if row_n == 3 {
                    x += row_w + self.px(2.4);
                    row_n = 0;
                    row_w = 0.0;
                }
                let tw = if label.is_empty() {
                    0.0
                } else {
                    let text = self.t(label);
                    self.measure(m, &text, label_style) + self.px(2.4)
                };
                let w = (self.px(4.0)
                    + self.px(17.6)
                    + tw
                    + if dropdown { self.px(11.2) } else { 0.0 }
                    + self.px(4.0))
                .round();
                // 125 %: rows 29 px apart from y 105 (measured).
                let h = self.px(23.2);
                self.controls.push(Control {
                    id,
                    kind,
                    r: [x, by + self.px(0.8) + row_n as f32 * h, w, h],
                });
                row_w = row_w.max(w);
                row_n += 1;
                continue;
            }
            if row_n > 0 {
                x += row_w + self.px(2.4);
                row_n = 0;
                row_w = 0.0;
            }
            match kind {
                Kind::Big(_, label, dropdown) => {
                    if small_col > 0 {
                        // Past the columns of the small-button grid.
                        let cols = small_col.div_ceil(2) as f32;
                        x += cols * self.px(34.0) - self.px(2.0);
                        small_col = 0;
                    }
                    let text = self.t(label);
                    let words: Vec<&str> = text.split(' ').collect();
                    // Two-line labels wrap at the last space.
                    let two = words.len() > 1
                        && (text.len() > self.wrap_after9() || self.look.wrap_words);
                    // The drop-down arrow follows the second line: it widens the button only
                    // when that line with the arrow is the longer one.
                    let arrow = if dropdown && two {
                        self.px(self.look.arrow_w)
                    } else {
                        0.0
                    };
                    let lw = if two {
                        let (a, b) = self.split_label9(&text);
                        let wa = self.measure(m, a, label_style);
                        let wb = self.measure(m, b, label_style);
                        wa.max(wb + arrow)
                    } else {
                        self.measure(m, &text, label_style)
                    };
                    let pad = if two {
                        self.look.big_pad
                    } else {
                        self.look.big_pad_one
                    };
                    let bw = (lw + self.px(pad)).max(self.px(self.look.big_min)).round();
                    self.controls.push(Control {
                        id,
                        kind,
                        r: [x, by, bw, bh],
                    });
                    x += bw + self.px(self.look.big_gap);
                }
                Kind::Small(_) => {
                    // A 2 × 2 grid of small buttons.
                    let s = self.px(28.0);
                    let col = small_col / 2;
                    let row = small_col % 2;
                    let gx = x + col as f32 * (s + self.px(6.0));
                    let gy = by + self.px(6.0) + row as f32 * (s + self.px(8.0));
                    self.controls.push(Control {
                        id,
                        kind,
                        r: [gx, gy, s, s],
                    });
                    small_col += 1;
                }
                Kind::Sep => {
                    if small_col > 0 {
                        // Two-column grids keep their measured width; wider ones grow by
                        // a column each.
                        let cols = small_col.div_ceil(2).max(2) as f32;
                        x += self.px(28.0 * cols + 6.0 * (cols - 1.0) + 4.0);
                        small_col = 0;
                    }
                    x += self.px(self.look.sep_space.0);
                    self.controls.push(Control {
                        id,
                        kind,
                        r: [x, by + self.px(2.4), 1.0, bh - self.px(4.8)],
                    });
                    x += self.px(self.look.sep_space.1);
                }
                _ => {}
            }
        }
        // Navigation column and status bar.
        let top = self.ribbon_bottom() + 1.0 + self.doc_tabs_h();
        let sh = self.status_h();
        let nav_x = self.px(7.2);
        let nav_w = self.px(40.0);
        self.nav_rect = [nav_x, top, nav_w, h - sh - top];
        let mut icons = vec![
            ("nav.bookmarks", Panel::Bookmarks),
            ("nav.pages", Panel::Pages),
            ("nav.comments", Panel::Comments),
        ];
        icons.extend(self.nav8());
        if let Some(e) = &self.extra_panel {
            icons.push((e.nav_id, Panel::Extra));
        }
        let is = self.px(32.0);
        for (k, (id, panel)) in icons.into_iter().enumerate() {
            let cy = top + self.px(36.0) + k as f32 * self.px(44.0);
            self.controls.push(Control {
                id,
                kind: Kind::Nav(panel),
                r: [
                    nav_x + ((nav_w - is) / 2.0).round(),
                    (cy - is / 2.0).round(),
                    is,
                    is,
                ],
            });
        }
        // Status bar controls (125 %: first 38, prev 72, page box 91–232, next 250, last 284).
        let sy = h - sh;
        let s = self.px(24.0);
        let cy = (sy + (sh - s) / 2.0).round();
        let status = [
            ("page.first", "first", 18.4),
            ("page.prev", "prev", 45.6),
            ("page.next", "next", 187.6),
            ("page.last", "last", 214.8),
            ("view.prev", "prevView", 242.8),
            ("view.next", "nextView", 270.0),
        ];
        for (id, icon, xd) in status {
            self.controls.push(Control {
                id,
                kind: Kind::Small(icon),
                r: [self.px(xd), cy, s, s],
            });
        }
        let bh = self.px(self.look.status_box_h);
        self.page_box = [
            self.px(72.8),
            self.look
                .status_box_top
                .map_or((sy + (sh - bh) / 2.0).round(), |t| {
                    (sy + self.px(t)).round()
                }),
            self.px(113.6),
            bh,
        ];
        // Right side, from the right edge (125 %): full screen at w − 48, zoom box w − 165…
        let right = [
            ("view.single", "single", 381.2),
            ("view.continuous", "continuous", 354.0),
            ("view.facing", "facing", 327.2),
            ("view.contFacing", "contFacing", 300.0),
            ("zoom.out", "minus", 280.4),
            ("zoom.in", "plus", 153.2),
            ("view.fullscreen", "fullscreen", 50.8),
        ];
        for (id, icon, xd) in right {
            self.controls.push(Control {
                id,
                kind: Kind::Small(icon),
                r: [w - self.px(xd), cy, s, s],
            });
        }
        self.slider = [w - self.px(261.6), cy, self.px(113.6), s];
        self.zoom_box = [w - self.px(132.8), self.page_box[1], self.px(79.2), bh];
        // Texts drawn centred or elided later.
        let title = self.title();
        let ts = self.style(14.0, true);
        self.measure(m, &title, ts);
        let names: Vec<String> = self.tabs.iter().map(|t| t.name.clone()).collect();
        let ds = self.style(14.0, false);
        for n in names {
            self.measure(m, &n, ds);
        }
        self.layout_doc();
    }

    fn panel_open(&self) -> Option<Panel> {
        if self.show_start {
            return None;
        }
        self.tabs.get(self.active).and_then(|t| t.panel)
    }

    /// Show a pane of `width` logical pixels at the right of the document (0 hides it).
    pub(crate) fn set_right_pane(&mut self, width: f32) {
        if (self.right_pane - width).abs() > 0.01 {
            self.right_pane = width;
            self.layout_doc();
        }
    }

    /// The right pane in window pixels (zero width when hidden); it starts at the document
    /// tab row, which ends where it begins.
    pub(crate) fn right_pane_rect(&self) -> [f32; 4] {
        let (w, h) = self.size;
        let top = self.ribbon_bottom() + 1.0;
        let rp = self.px(self.right_pane);
        [w - rp, top, rp, (h - self.status_h() - top).max(0.0)]
    }

    /// Lay out the navigation panel and the document view.
    fn layout_doc(&mut self) {
        let (w, h) = self.size;
        let top = self.ribbon_bottom() + 1.0 + self.doc_tabs_h();
        let sh = self.status_h();
        let nav_end = self.nav_rect[0] + self.nav_rect[2];
        let strip_w = self.px(13.6);
        let panel_w = if self.panel_open().is_some() {
            self.px(242.4)
        } else {
            0.0
        };
        self.panel_rect = [nav_end, top, panel_w, h - sh - top];
        self.strip_rect = [nav_end + panel_w, top, strip_w, h - sh - top];
        let dx = nav_end + panel_w + strip_w;
        let sbw = self.px(16.8);
        let margin = self.px(6.4);
        let rp = self.px(self.right_pane);
        self.vbar = [w - rp - margin - sbw, top, sbw, h - sh - top];
        self.doc_rect = [
            dx,
            top,
            (w - rp - margin - sbw - dx).max(1.0),
            (h - sh - top).max(1.0),
        ];
        self.mode_layout();
        self.viewx_layout();
        let (rect, scale) = (self.doc_rect, self.scale);
        if let Some(t) = self.tabs.get_mut(self.active) {
            t.view.set_frame(rect, scale);
        }
    }

    // ---- Input ------------------------------------------------------------------------------

    /// What a window point is for the window manager.
    pub fn nc_hit(&self, x: f32, y: f32) -> NcHit {
        if self.menu.is_some() || self.dialog.is_some() || self.extra_modal() {
            return NcHit::Client;
        }
        if y >= self.title_h() {
            return NcHit::Client;
        }
        for c in &self.controls {
            if let Kind::Win(k) = c.kind {
                if inside(c.r, x, y) {
                    return k;
                }
            }
            if matches!(c.kind, Kind::Quick(_)) && inside(c.r, x, y) {
                return NcHit::Client;
            }
        }
        if inside(self.search, x, y) {
            return NcHit::Client;
        }
        NcHit::Caption
    }

    /// Handle an input event; true when the window needs a new frame.
    #[allow(clippy::too_many_lines)]
    pub fn handle(&mut self, input: Input, m: &dyn TextMeasure) -> bool {
        let redraw = self.handle_input(input, m);
        // Reflowed text follows the page and the window width.
        self.reflow_layout(m);
        self.measure10(m);
        redraw
    }

    fn handle_input(&mut self, input: Input, m: &dyn TextMeasure) -> bool {
        match input {
            Input::Scale(s) => {
                self.scale = s;
                self.widths.clear();
                self.relayout(m);
                true
            }
            Input::Resize { width, height } => {
                self.size = (width, height);
                self.relayout(m);
                true
            }
            Input::Maximized(on) => {
                self.maximized = on;
                true
            }
            Input::Tick => {
                let searched = self.tick_search();
                let tip = self.tick9(m);
                let advanced = self.tick_search9(m);
                self.tick_viewx(m) || self.search_focus || searched || tip || advanced
            }
            Input::Blink => self.search_focus,
            Input::Leave => self.hover.take().is_some(),
            _ => {
                if self.controls.is_empty() {
                    self.relayout(m);
                }
                if matches!(
                    input,
                    Input::Press { .. } | Input::Key(..) | Input::Wheel(_)
                ) {
                    self.hide_tip9();
                }
                self.route(input, m)
            }
        }
    }

    fn control_at(&self, x: f32, y: f32) -> Option<Control> {
        self.controls
            .iter()
            .find(|c| !matches!(c.kind, Kind::Sep) && inside(c.r, x, y))
            .copied()
    }

    #[allow(clippy::too_many_lines)]
    fn route(&mut self, input: Input, m: &dyn TextMeasure) -> bool {
        if let Some(redraw) = self.route_extra(&input, m) {
            return redraw;
        }
        // Dialog first.
        if let Some((d, _)) = self.dialog.as_mut() {
            let done = match &input {
                Input::Text(s) => {
                    d.text(s);
                    None
                }
                Input::Key(k, mods) => d.key(*k, *mods),
                Input::Press { x, y, .. } => {
                    if d.contains(*x, *y) {
                        d.press(*x, *y)
                    } else {
                        None
                    }
                }
                Input::Move { x, y } => {
                    d.hover(*x, *y);
                    None
                }
                _ => None,
            };
            if let Some(b) = done {
                if let Some((d, Some(pending))) = self.dialog.take() {
                    if b == DialogButton::Ok {
                        let text = d.values().into_iter().next().unwrap_or_default();
                        self.finish_pending(pending, &text);
                    }
                }
            }
            return true;
        }
        // Open menu.
        if let Some((menu, _)) = self.menu.as_mut() {
            match &input {
                Input::Move { x, y } | Input::Drag { x, y } => return menu.hover(*x, *y),
                Input::Press { x, y, .. } => {
                    if menu.contains(*x, *y) {
                        menu.press(*x, *y);
                        return true;
                    }
                    self.menu = None;
                    return true;
                }
                Input::Release { x, y } => {
                    if let Some(c) = menu.release(*x, *y) {
                        self.menu = None;
                        if let Choice::Command(cmd) = c {
                            self.command(&cmd, m);
                        }
                    }
                    return true;
                }
                Input::Wheel(dy) => return menu.wheel(*dy),
                Input::Key(Key::Escape, _) => {
                    self.menu = None;
                    return true;
                }
                Input::Key(Key::Down, _) => {
                    menu.step(1);
                    return true;
                }
                Input::Key(Key::Up, _) => {
                    menu.step(-1);
                    return true;
                }
                Input::Key(Key::Enter, _) => {
                    let c = menu.current();
                    self.menu = None;
                    if let Some(Choice::Command(cmd)) = c {
                        self.command(&cmd, m);
                    }
                    return true;
                }
                _ => {}
            }
        }
        match input {
            Input::Text(s) => {
                if self.search_focus {
                    self.search_text.push_str(&s);
                    let (t, st) = (self.search_text.clone(), self.style(13.0, false));
                    self.measure(m, &t, st);
                    return true;
                }
                false
            }
            Input::Key(k, mods) => self.key(k, mods, m),
            Input::Wheel(dy) => {
                let (x, y) = self.pointer;
                self.wheel_at(x, y, dy)
            }
            Input::Zoom(n) => {
                if let Some(t) = self.tabs.get_mut(self.active) {
                    // One preset per notch (the reference's Ctrl + wheel).
                    let steps = n.abs().round().max(1.0) as usize;
                    for _ in 0..steps {
                        t.view.step_zoom(if n > 0.0 { 1 } else { -1 });
                    }
                    return true;
                }
                false
            }
            Input::Move { x, y } => {
                self.pointer = (x, y);
                self.hover_at(x, y)
            }
            Input::Press { x, y, count, shift } => self.press(x, y, count, shift, m),
            Input::Drag { x, y } => self.drag_to(x, y),
            Input::Release { x, y } => self.release(x, y, m),
            _ => false,
        }
    }

    fn hover_at(&mut self, x: f32, y: f32) -> bool {
        let id = self.control_at(x, y).map(|c| c.id);
        let changed = id != self.hover;
        self.hover = id;
        // Pointer shape over the document.
        let mut cursor = 3;
        if !self.show_start && inside(self.doc_rect, x, y) {
            if let Some(t) = self.tabs.get(self.active) {
                if let Some((p, px, py)) = t.view.page_at(x, y) {
                    let over_text = t.view.text_if_ready(p).is_some_and(|l| l.over_text(px, py));
                    if t.view.link_at(p, px, py).is_some() {
                        cursor = 5;
                    } else if over_text
                        && matches!(self.tool, Tool::Hand | Tool::Select | Tool::Markup(_))
                    {
                        cursor = 4;
                    } else if self.tool == Tool::Hand {
                        cursor = 6;
                    } else if matches!(self.tool, Tool::Rectangle | Tool::TextBox | Tool::Note) {
                        cursor = 0;
                    }
                } else if self.tool == Tool::Hand {
                    cursor = 6;
                }
            }
        }
        // Split bars: left-right / up-down arrows (BlueReader round 6).
        match self.split_bar_at(x, y) {
            Some((true, false)) => cursor = 1,
            Some((false, true)) => cursor = 2,
            Some((true, true)) => cursor = 6,
            _ => {}
        }
        if self.prefs6.caret && cursor != 5 && inside(self.doc_rect, x, y) {
            cursor = 4;
        }
        let c = cursor != self.cursor;
        self.cursor = cursor;
        changed || c
    }

    #[allow(clippy::too_many_lines)]
    fn press(&mut self, x: f32, y: f32, count: u8, shift: bool, m: &dyn TextMeasure) -> bool {
        self.pressed = None;
        let was_search = self.search_focus;
        self.search_focus = inside(self.search, x, y);
        if self.search_focus {
            return true;
        }
        if let Some(c) = self.control_at(x, y) {
            self.pressed = Some(c.id);
            return true;
        }
        // Ribbon tabs.
        for (i, r) in self.tab_rects.clone().into_iter().enumerate() {
            if inside(r, x, y) {
                if i == 0 && self.file_backstage {
                    self.requests
                        .push(ReaderRequest::Command("file.backstage".into()));
                } else if i == 0 {
                    self.open_file_menu(r);
                } else {
                    self.ribbon_tab = i - 1;
                    self.relayout(m);
                }
                return true;
            }
        }
        // Document tabs.
        if inside(self.start_tab, x, y) {
            self.show_start = true;
            return true;
        }
        for (i, (r, close)) in self.doc_tab_rects.clone().into_iter().enumerate() {
            if inside(close, x, y) {
                self.close_tab(i);
                return true;
            }
            if inside(r, x, y) {
                self.active = i;
                self.show_start = false;
                self.layout_doc();
                return true;
            }
        }
        // Start page.
        if self.show_start || self.tabs.is_empty() {
            for (i, r) in self.recent_rects.clone().into_iter().enumerate() {
                if inside(r, x, y) {
                    // Row 0 is "Open File", then the recent files.
                    let path = match i.checked_sub(1) {
                        None => None,
                        Some(k) => match self.recent.get(k) {
                            Some(p) => Some(p.clone()),
                            None => return true,
                        },
                    };
                    self.requests.push(ReaderRequest::Open(path));
                    return true;
                }
            }
            return was_search;
        }
        // Status bar boxes.
        if inside(self.page_box, x, y) {
            self.open_page_menu();
            return true;
        }
        if inside(self.zoom_box, x, y) {
            self.open_zoom_menu(self.zoom_box);
            return true;
        }
        if inside(self.slider, x, y) {
            self.drag = Some(Drag::Slider);
            self.slider_to(x);
            return true;
        }
        if inside(self.vbar, x, y) {
            let (thumb_y, thumb_h) = self.thumb();
            let offset = if y >= thumb_y && y < thumb_y + thumb_h {
                y - thumb_y
            } else {
                thumb_h / 2.0
            };
            self.drag = Some(Drag::VBar { offset });
            self.vbar_to(y, offset);
            return true;
        }
        if inside(self.strip_rect, x, y) {
            if let Some(t) = self.tabs.get_mut(self.active) {
                t.panel = if t.panel.is_some() {
                    None
                } else {
                    Some(Panel::Bookmarks)
                };
            }
            self.layout_doc();
            return true;
        }
        if inside(self.panel_rect, x, y) {
            return self.panel_press(x, y);
        }
        if !inside(self.doc_rect, x, y) {
            return was_search;
        }
        let Some(t) = self.tabs.get_mut(self.active) else {
            return false;
        };
        let hit = t.view.page_at(x, y);
        if let Some((p, px, py)) = hit {
            if let Some(target) = t.view.link_at(p, px, py) {
                if matches!(self.tool, Tool::Hand | Tool::Select) {
                    match target {
                        LinkTarget::Page { index, top } => t.view.go_to(index, top),
                        LinkTarget::Uri(u) => self.confirm_url(u, m),
                    }
                    return true;
                }
            }
        }
        match self.tool {
            Tool::Hand | Tool::Select | Tool::Markup(_) => {
                let over = hit.and_then(|(p, px, py)| {
                    let reach = if self.tool == Tool::Hand { 0.0 } else { 40.0 };
                    let layer = t.view.text(p)?;
                    if self.tool == Tool::Hand && !layer.over_text(px, py) {
                        return None;
                    }
                    layer.hit(px, py, reach).map(|c| (p, c))
                });
                if let Some((page, c)) = over {
                    let (a, b) = if count >= 2 {
                        word_around(t.view.text_if_ready(page), c)
                    } else if shift {
                        match t.view.selection() {
                            Some(s) if s.page == page => (s.a, c),
                            _ => (c, c),
                        }
                    } else {
                        (c, c)
                    };
                    t.view.set_selection(if count >= 2 || shift {
                        Some(Selection { page, a, b })
                    } else {
                        None
                    });
                    self.drag = Some(Drag::Select { page, a });
                } else {
                    t.view.set_selection(None);
                    if self.tool == Tool::Hand {
                        let ((sx, sy), _) = t.view.scroll_state();
                        self.drag = Some(Drag::Pan { x, y, sx, sy });
                    }
                }
            }
            Tool::Note => {
                if let Some((page, px, py)) = hit {
                    self.ask_text(Pending::Note { page, x: px, y: py }, "pdfReader.noteTitle");
                }
            }
            Tool::TextBox | Tool::Rectangle => {
                if let Some((page, px, py)) = hit {
                    self.drag = Some(Drag::Shape { page, x: px, y: py });
                }
            }
        }
        true
    }

    fn drag_to(&mut self, x: f32, y: f32) -> bool {
        match self.drag {
            Some(Drag::Pan {
                x: x0,
                y: y0,
                sx,
                sy,
            }) => {
                if let Some(t) = self.tabs.get_mut(self.active) {
                    let ((cx, cy), _) = t.view.scroll_state();
                    t.view.scroll_by(sx - (x - x0) - cx, sy - (y - y0) - cy);
                }
                true
            }
            Some(Drag::Select { page, a }) => {
                let Some(t) = self.tabs.get_mut(self.active) else {
                    return false;
                };
                let s = t.view.px_per_pt();
                let Some(r) = t.view.page_rect(page) else {
                    return false;
                };
                let (px, py) = ((x - r[0]) / s, (y - r[1]) / s);
                if let Some(c) = t.view.text(page).and_then(|l| l.hit(px, py, f32::INFINITY)) {
                    t.view.set_selection(Some(Selection { page, a, b: c }));
                }
                // Scroll when dragging past the view.
                let f = t.view.frame();
                if y > f[1] + f[3] {
                    t.view.scroll_by(0.0, (y - f[1] - f[3]).min(40.0));
                } else if y < f[1] {
                    t.view.scroll_by(0.0, -(f[1] - y).min(40.0));
                }
                true
            }
            Some(Drag::VBar { offset }) => {
                self.vbar_to(y, offset);
                true
            }
            Some(Drag::Slider) => {
                self.slider_to(x);
                true
            }
            Some(Drag::Shape { .. }) => true,
            None => self.hover_at(x, y),
        }
    }

    fn release(&mut self, x: f32, y: f32, m: &dyn TextMeasure) -> bool {
        if let Some(id) = self.pressed.take() {
            if self.control_at(x, y).is_some_and(|c| c.id == id) && self.enabled(id) {
                let r = self.controls.iter().find(|c| c.id == id).map(|c| c.r);
                if let Some(r) = r {
                    self.click(id, r, m);
                }
            }
            return true;
        }
        match self.drag.take() {
            Some(Drag::Select { page, .. }) => {
                if let Tool::Markup(kind) = self.tool {
                    self.markup_selection(kind, page);
                    self.after_markup(m);
                }
                true
            }
            Some(Drag::Shape { page, x: x0, y: y0 }) => {
                let Some(t) = self.tabs.get(self.active) else {
                    return false;
                };
                let s = t.view.px_per_pt();
                let Some(r) = t.view.page_rect(page) else {
                    return false;
                };
                let (x1, y1) = ((x - r[0]) / s, (y - r[1]) / s);
                let mut rect = [x0.min(x1), y0.min(y1), x0.max(x1), y0.max(y1)];
                if rect[2] - rect[0] < 4.0 || rect[3] - rect[1] < 4.0 {
                    // A click: a default size box.
                    rect = [x0, y0, x0 + 160.0, y0 + 28.0];
                }
                if self.tool == Tool::TextBox {
                    self.ask_text(Pending::TextBox { page, rect }, "pdfReader.textboxTitle");
                } else {
                    let rect = self.view().map_or(rect, |v| v.unrotate_rect(page, rect));
                    let color = self
                        .nine
                        .default_color(AnnotKind::Square)
                        .unwrap_or(COLOR_MARK);
                    self.add_annot(AnnotKind::Square, page, vec![rect], color, String::new());
                }
                true
            }
            Some(_) => true,
            None => false,
        }
    }

    fn markup_selection(&mut self, kind: AnnotKind, page: usize) {
        let Some(t) = self.tabs.get(self.active) else {
            return;
        };
        let rects = t.view.selection_rects();
        if rects.is_empty() {
            return;
        }
        let rects: Vec<[f32; 4]> = rects
            .iter()
            .map(|r| t.view.unrotate_rect(page, *r))
            .collect();
        let text = t.view.selected_text().unwrap_or_default();
        let color = self
            .nine
            .default_color(kind)
            .unwrap_or(if kind == AnnotKind::Highlight {
                COLOR_HIGHLIGHT
            } else {
                COLOR_MARK
            });
        self.add_annot(kind, page, rects, color, text);
        if let Some(t) = self.tabs.get_mut(self.active) {
            t.view.set_selection(None);
        }
    }

    fn add_annot(
        &mut self,
        kind: AnnotKind,
        page: usize,
        rects: Vec<[f32; 4]>,
        color: [u8; 3],
        contents: String,
    ) {
        let author = self.author.clone();
        let Some(t) = self.tabs.get_mut(self.active) else {
            return;
        };
        let r = t.view.doc_mut().add_annotation(&bo_pdfread::NewAnnot {
            kind,
            page,
            rects,
            color,
            author,
            contents,
        });
        match r {
            Ok(()) => t.view.invalidate_page(page),
            Err(e) => self.status = e,
        }
    }

    fn ask_text(&mut self, pending: Pending, title: &str) {
        let d = Dialog::new(
            DialogKind::Comment,
            self.t(title),
            vec![(self.t("pdfReader.commentText"), String::new())],
            vec![
                (self.t("pdfReader.ok"), DialogButton::Ok),
                (self.t("pdfReader.cancel"), DialogButton::Cancel),
            ],
            Vec::new(),
        );
        // Placed at once (BlueEditor draws it before the next layout).
        let mut d = d;
        d.layout(&crate::editor_window::Estimate, self.size, self.scale);
        self.dialog = Some((d, Some(pending)));
    }

    fn finish_pending(&mut self, p: Pending, text: &str) {
        match p {
            Pending::Note { page, x, y } => {
                let rect = [x, y, x + 20.0, y + 20.0];
                let rect = self.view().map_or(rect, |v| v.unrotate_rect(page, rect));
                self.add_annot(
                    AnnotKind::Note,
                    page,
                    vec![rect],
                    COLOR_NOTE,
                    text.to_owned(),
                );
            }
            Pending::TextBox { page, rect } => {
                let lines = text.lines().count().max(1) as f32;
                let rect = [
                    rect[0],
                    rect[1],
                    rect[2],
                    rect[1].max(rect[3]).max(rect[1] + 8.0 + 13.0 * lines),
                ];
                let rect = self.view().map_or(rect, |v| v.unrotate_rect(page, rect));
                self.add_annot(
                    AnnotKind::TextBox,
                    page,
                    vec![rect],
                    [80, 80, 80],
                    text.to_owned(),
                );
            }
        }
    }

    fn close_tab(&mut self, i: usize) {
        // BlueEditor asks to save changes first (`editor_close`).
        if self.close_guard {
            self.requests
                .push(ReaderRequest::Command(format!("ed.closeTab.{i}")));
            return;
        }
        self.close_tab_now(i);
    }

    /// Close document tab `i` without asking.
    pub(crate) fn close_tab_now(&mut self, i: usize) {
        if i < self.tabs.len() {
            self.tabs.remove(i);
            if self.active >= self.tabs.len() {
                self.active = self.tabs.len().saturating_sub(1);
            }
            self.show_start = self.tabs.is_empty();
            self.layout_doc();
        }
    }

    fn thumb(&self) -> (f32, f32) {
        let Some(t) = self.tabs.get(self.active) else {
            return (self.vbar[1], self.vbar[3]);
        };
        let ((_, sy), (_, ch)) = t.view.scroll_state();
        let (lo, hi) = t.view.scroll_range();
        let track = self.vbar[3] - self.px(16.0);
        let view_h = t.view.frame()[3];
        let total = (hi - lo) + view_h;
        // A window not laid out yet (or very short) has no track: no panic on min > max.
        let track = track.max(self.px(20.0));
        let th = (track * view_h / total.max(1.0)).clamp(self.px(20.0), track);
        let frac = if hi > lo { (sy - lo) / (hi - lo) } else { 0.0 };
        let _ = ch;
        (self.vbar[1] + self.px(16.0) + frac * (track - th), th)
    }

    fn vbar_to(&mut self, y: f32, offset: f32) {
        let (_, th) = self.thumb();
        let track = self.vbar[3] - self.px(16.0);
        let top = self.vbar[1] + self.px(16.0);
        let frac = ((y - offset - top) / (track - th).max(1.0)).clamp(0.0, 1.0);
        if let Some(t) = self.tabs.get_mut(self.active) {
            let (lo, hi) = t.view.scroll_range();
            t.view.set_scroll_y(lo + frac * (hi - lo));
        }
    }

    /// Zoom slider: logarithmic from 8.33 % to 1600 %, 100 % near the left third.
    fn slider_to(&mut self, x: f32) {
        let [sx, _, sw, _] = self.slider;
        let f = ((x - sx) / sw).clamp(0.0, 1.0);
        let z = (0.0833_f32.ln() + f * (16.0_f32.ln() - 0.0833_f32.ln())).exp();
        if let Some(t) = self.tabs.get_mut(self.active) {
            t.view.set_zoom(z);
        }
    }

    fn slider_pos(&self) -> f32 {
        let z = self.view().map_or(1.0, ReaderView::zoom);
        ((z.ln() - 0.0833_f32.ln()) / (16.0_f32.ln() - 0.0833_f32.ln())).clamp(0.0, 1.0)
    }

    fn key(&mut self, k: Key, mods: Mods, m: &dyn TextMeasure) -> bool {
        if self.search_focus {
            match k {
                Key::Backspace => {
                    self.search_text.pop();
                    return true;
                }
                Key::Enter => {
                    self.run_search(!mods.shift);
                    return true;
                }
                Key::Escape => {
                    self.search_focus = false;
                    if let Some(t) = self.tabs.get_mut(self.active) {
                        t.view.clear_hits();
                    }
                    return true;
                }
                Key::Letter('a') if mods.ctrl => return false,
                _ => {}
            }
        }
        let Some(t) = self.tabs.get_mut(self.active) else {
            if let Key::Letter('o') = k {
                self.requests.push(ReaderRequest::Open(None));
                return true;
            }
            return false;
        };
        // 125 %: 54 px per arrow press (measured on the reference).
        let line = 43.2 * self.scale;
        let page_h = t.view.frame()[3] * 0.9;
        match (k, mods.ctrl) {
            (Key::Down, false) => t.view.scroll_by(0.0, line),
            (Key::Up, false) => t.view.scroll_by(0.0, -line),
            (Key::PageDown, _) => t.view.scroll_by(0.0, page_h),
            (Key::PageUp, _) => t.view.scroll_by(0.0, -page_h),
            (Key::Right, _) => {
                let p = t.view.current_page();
                t.view.go_to(p + 1, None);
            }
            (Key::Left, _) => {
                let p = t.view.current_page();
                t.view.go_to(p.saturating_sub(1), None);
            }
            (Key::Home, _) => t.view.go_to(0, None),
            (Key::End, _) => {
                let n = t.view.page_count();
                t.view.go_to(n.saturating_sub(1), None);
            }
            (Key::Escape, _) => {
                t.view.set_selection(None);
                t.view.clear_hits();
                self.tool = Tool::Hand;
            }
            (Key::F(3), _) => t.view.next_hit(!mods.shift),
            (Key::Letter('f'), true) => self.search_focus = true,
            (Key::Letter('c'), true) => {
                if let Some(s) = t.view.selected_text() {
                    self.requests.push(ReaderRequest::Copy(s));
                }
            }
            (Key::Letter('a'), true) => {
                let p = t.view.current_page();
                if let Some(n) = t.view.text(p).map(bo_pdfread::TextLayer::len) {
                    if n > 0 {
                        t.view.set_selection(Some(Selection {
                            page: p,
                            a: 0,
                            b: n - 1,
                        }));
                    }
                }
            }
            (Key::Letter('o'), true) => self.requests.push(ReaderRequest::Open(None)),
            (Key::Letter('s'), true) if mods.shift => self.requests.push(ReaderRequest::SaveAs),
            (Key::Letter('s'), true) => self.requests.push(ReaderRequest::Save),
            (Key::Letter('p'), true) => self.requests.push(ReaderRequest::Print),
            (Key::Letter('w'), true) => {
                let a = self.active;
                self.close_tab(a);
            }
            (Key::Letter('+' | '='), true) => t.view.step_zoom(1),
            (Key::Letter('-'), true) => t.view.step_zoom(-1),
            (Key::Letter('0'), true) => t.view.set_fit(Fit::Page),
            (Key::Letter('1'), true) => t.view.set_zoom(1.0),
            (Key::Letter('2'), true) => t.view.set_fit(Fit::Width),
            (Key::F(11), _) => {
                self.command("view.fullscreen", m);
            }
            _ => return false,
        }
        true
    }

    fn run_search(&mut self, forward: bool) {
        let q = self.search_text.clone();
        let (case, word) = self.find_options8();
        let Some(t) = self.tabs.get_mut(self.active) else {
            return;
        };
        let same = !t.view.hits().is_empty();
        if same && t.view.current_hit().is_some() && self.status.starts_with(&q) {
            t.view.next_hit(forward);
        } else {
            // The first hit after the current page shows at once; the rest of a long document
            // is read on ticks (`tick_search`).
            t.view.find_start(&q, case, word);
        }
        self.search_status();
    }

    /// Status of the search box's query: hit k / n (with the pages read while searching).
    fn search_status(&mut self) {
        let q = self.search_text.clone();
        let Some(t) = self.tabs.get(self.active) else {
            return;
        };
        let n = t.view.hits().len();
        let cur = t.view.current_hit().map_or(0, |c| c + 1);
        let progress = t
            .view
            .search_progress()
            .map(|(done, all)| format!(" ({done} / {all})"))
            .unwrap_or_default();
        self.status = if n == 0 && progress.is_empty() {
            format!("{q}: {}", self.strings.get("pdfReader.noMatches"))
        } else {
            format!("{q}: {cur} / {n}{progress}")
        };
    }

    /// Continue a running search for part of a tick; `true` when the window changed.
    fn tick_search(&mut self) -> bool {
        let Some(t) = self.tabs.get_mut(self.active) else {
            return false;
        };
        if !t.view.searching() {
            return false;
        }
        t.view.search_step(std::time::Duration::from_millis(60));
        self.search_status();
        true
    }

    fn click(&mut self, id: &'static str, r: [f32; 4], m: &dyn TextMeasure) {
        match id {
            "menu.zoom" => self.open_zoom_menu(r),
            "menu.fit" => {
                let items = [
                    ("zoom.actual", "pdfReader.actualSize"),
                    ("zoom.page", "pdfReader.fitPage"),
                    ("zoom.width", "pdfReader.fitWidth"),
                ];
                self.open_menu(&items, r, id);
            }
            "menu.rotate" => {
                let items = [
                    ("rotate.right", "pdfReader.rotateRight"),
                    ("rotate.left", "pdfReader.rotateLeft"),
                ];
                self.open_menu(&items, r, id);
            }
            "page.number" => self.open_page_menu(),
            other => self.command(other, m),
        }
    }

    fn open_menu(&mut self, items: &[(&str, &str)], r: [f32; 4], id: &'static str) {
        let items = items
            .iter()
            .map(|(cmd, key)| MenuItem {
                label: self.t(key),
                choice: Choice::Command((*cmd).to_owned()),
                checked: false,
                dim: false,
            })
            .collect();
        self.menu = Some((self.reader_menu(items, r), id));
    }

    fn open_file_menu(&mut self, r: [f32; 4]) {
        if let Some(p) = &self.profile {
            let items = p.file_menu.clone();
            let open = self.tabs.is_empty();
            let mut items: Vec<MenuItem> = items
                .iter()
                .map(|(cmd, key)| MenuItem {
                    label: self.t(key),
                    choice: Choice::Command((*cmd).to_owned()),
                    checked: false,
                    dim: open && !matches!(*cmd, "file.open" | "file.create" | "file.merge"),
                })
                .collect();
            items.retain(|i| !(open && i.dim));
            self.menu = Some((self.reader_menu(items, r), "file"));
            return;
        }
        let mut items = vec![
            ("file.open", "pdfReader.open"),
            ("file.properties", "pdfProps.menu"),
            ("file.save", "pdfReader.save"),
            ("file.saveAs", "pdfReader.saveAs"),
            ("file.print", "pdfReader.print"),
            ("file.close", "pdfReader.close"),
        ];
        if self.tabs.is_empty() {
            items.retain(|i| i.0 == "file.open");
        }
        self.open_menu(&items, r, "file");
    }

    fn open_zoom_menu(&mut self, r: [f32; 4]) {
        let z = self.view().map_or(1.0, ReaderView::zoom);
        let mut items: Vec<MenuItem> = Vec::new();
        for p in [
            "6400", "3200", "1600", "800", "600", "400", "200", "150", "125", "100", "75", "66.67",
            "50", "33.33", "25", "12.5", "8.33", "1",
        ] {
            let v: f32 = p.parse().unwrap_or(100.0);
            items.push(MenuItem {
                label: format!("{p}%"),
                choice: Choice::Command(format!("zoom.set.{p}")),
                checked: (z * 100.0 - v).abs() < 0.05,
                dim: false,
            });
        }
        items.extend(
            [
                ("zoom.actual", "pdfReader.actualSize"),
                ("zoom.page", "pdfReader.fitPage"),
                ("zoom.width", "pdfReader.fitWidth"),
            ]
            .iter()
            .map(|(c, k)| MenuItem {
                label: self.t(k),
                choice: Choice::Command((*c).to_owned()),
                checked: false,
                dim: false,
            }),
        );
        // A rule before Actual Size, as the reference's list.
        let menu = self.reader_menu(items, r).with_decor(Vec::new(), vec![18]);
        self.menu = Some((menu, "zoom"));
    }

    fn open_page_menu(&mut self) {
        let Some(t) = self.tabs.get(self.active) else {
            return;
        };
        let n = t.view.page_count();
        let cur = t.view.current_page();
        let items = (0..n)
            .map(|i| MenuItem {
                label: page_label(t.view.doc(), i, n),
                choice: Choice::Command(format!("page.go.{i}")),
                checked: i == cur,
                dim: false,
            })
            .collect();
        let mut menu = self.reader_menu(items, self.page_box);
        // Keep the current page in view.
        for _ in 0..cur.min(2000) {
            menu.step(1);
        }
        self.menu = Some((menu, "pages"));
    }

    #[allow(clippy::too_many_lines)]
    fn command(&mut self, cmd: &str, m: &dyn TextMeasure) {
        if self.command_extra(cmd, m) {
            return;
        }
        if let Some(z) = cmd
            .strip_prefix("zoom.set.")
            .and_then(|v| v.parse::<f32>().ok())
        {
            if let Some(t) = self.tabs.get_mut(self.active) {
                t.view.set_zoom(z / 100.0);
            }
            return;
        }
        if let Some(i) = cmd
            .strip_prefix("page.go.")
            .and_then(|v| v.parse::<usize>().ok())
        {
            if let Some(t) = self.tabs.get_mut(self.active) {
                t.view.go_to(i, None);
            }
            return;
        }
        match cmd {
            "file.open" => self.requests.push(ReaderRequest::Open(None)),
            "file.save" => self.requests.push(ReaderRequest::Save),
            "file.saveAs" => self.requests.push(ReaderRequest::SaveAs),
            "file.print" => self.requests.push(ReaderRequest::Print),
            "file.close" => {
                let a = self.active;
                self.close_tab(a);
            }
            "win.min" => self.requests.push(ReaderRequest::Window(Request::Minimize)),
            "win.max" => self
                .requests
                .push(ReaderRequest::Window(Request::ToggleMaximize)),
            "win.close" => self.requests.push(ReaderRequest::Window(Request::Close)),
            "tool.hand" => self.tool = Tool::Hand,
            "tool.select" => self.tool = Tool::Select,
            "tool.highlight" => self.tool = Tool::Markup(AnnotKind::Highlight),
            "tool.underline" => self.tool = Tool::Markup(AnnotKind::Underline),
            "tool.strikeout" => self.tool = Tool::Markup(AnnotKind::StrikeOut),
            "tool.squiggly" => self.tool = Tool::Markup(AnnotKind::Squiggly),
            "tool.note" => self.tool = Tool::Note,
            "tool.textbox" => self.tool = Tool::TextBox,
            "tool.rectangle" => self.tool = Tool::Rectangle,
            "edit.copy" => {
                if let Some(s) = self.view().and_then(ReaderView::selected_text) {
                    self.requests.push(ReaderRequest::Copy(s));
                }
            }
            "find" => self.search_focus = true,
            "help.about" => {
                let d = Dialog::new(
                    DialogKind::WordCount,
                    self.app_name().into(),
                    Vec::new(),
                    vec![(self.t("pdfReader.ok"), DialogButton::Ok)],
                    vec![self.t("pdfReader.aboutText")],
                );
                let mut d = d;
                d.layout(&crate::editor_window::Estimate, self.size, self.scale);
                self.dialog = Some((d, None));
            }
            "nav.bookmarks" | "panel.bookmarks" => self.toggle_panel(Panel::Bookmarks),
            "nav.pages" | "panel.pages" => self.toggle_panel(Panel::Pages),
            "nav.comments" | "panel.comments" => self.toggle_panel(Panel::Comments),
            c if self.extra_panel.as_ref().is_some_and(|e| e.nav_id == c) => {
                self.toggle_panel(Panel::Extra);
            }
            "view.fullscreen" => {
                self.requests
                    .push(ReaderRequest::Window(Request::ToggleMaximize));
            }
            _ => {
                let Some(t) = self.tabs.get_mut(self.active) else {
                    self.requests.push(ReaderRequest::Command(cmd.to_owned()));
                    return;
                };
                match cmd {
                    "page.first" => t.view.go_to(0, None),
                    "page.prev" => {
                        let p = t.view.current_page();
                        t.view.go_to(p.saturating_sub(1), None);
                    }
                    "page.next" => {
                        let p = t.view.current_page();
                        t.view.go_to(p + 1, None);
                    }
                    "page.last" => {
                        let n = t.view.page_count();
                        t.view.go_to(n.saturating_sub(1), None);
                    }
                    "view.single" => t.view.set_layout(Layout::Single),
                    "view.continuous" => t.view.set_layout(Layout::Continuous),
                    "view.facing" => t.view.set_layout(Layout::Facing),
                    "view.contFacing" => t.view.set_layout(Layout::ContinuousFacing),
                    "zoom.in" => t.view.step_zoom(1),
                    "zoom.out" => t.view.step_zoom(-1),
                    "zoom.actual" => t.view.set_zoom(1.0),
                    "zoom.page" => t.view.set_fit(Fit::Page),
                    "zoom.width" => t.view.set_fit(Fit::Width),
                    "rotate.right" => t.view.rotate(true),
                    "rotate.left" => t.view.rotate(false),
                    other => self.requests.push(ReaderRequest::Command(other.to_owned())),
                }
            }
        }
        let _ = m;
    }

    /// Show panel `p` (kept open when it already is).
    pub(crate) fn show_panel(&mut self, p: Panel) {
        if self
            .tabs
            .get(self.active)
            .is_some_and(|t| t.panel != Some(p))
        {
            self.toggle_panel(p);
        }
    }

    /// Text selected in the active document.
    pub(crate) fn selected_text(&self) -> Option<String> {
        self.tabs.get(self.active)?.view.selected_text()
    }

    /// Show navigation panel `p`, or hide it when it is shown.
    pub fn toggle_panel(&mut self, p: Panel) {
        if let Some(t) = self.tabs.get_mut(self.active) {
            t.panel = if t.panel == Some(p) { None } else { Some(p) };
            t.panel_scroll = 0.0;
        }
        self.layout_doc();
    }

    fn panel_press(&mut self, x: f32, y: f32) -> bool {
        let row = self
            .panel_rows
            .iter()
            .find(|(ry, rh, _)| y >= *ry && y < ry + rh)
            .map(|r| r.2.clone());
        let Some(row) = row else { return false };
        let Some(t) = self.tabs.get_mut(self.active) else {
            return false;
        };
        match row {
            PanelRow::Outline(path, target, toggle) => {
                if toggle.is_some_and(|(a, b)| x >= a - 2.0 && x <= b + 2.0) {
                    if !t.expanded.remove(&path) {
                        t.expanded.insert(path);
                    }
                } else {
                    self.selected_outline = Some(path);
                    if let Some(LinkTarget::Page { index, top }) = target {
                        t.view.go_to(index, top);
                    }
                }
            }
            PanelRow::Page(i) => t.view.go_to(i, None),
            PanelRow::Comment(page, rect) => {
                t.view.go_to(page, Some((rect[1] - 40.0).max(0.0)));
                // The comment is selected on the page too.
                self.extra.selected = t
                    .view
                    .doc()
                    .annotations()
                    .into_iter()
                    .find(|a| a.page == page && a.rect == rect);
            }
            PanelRow::Extra(cmd) => self.requests.push(ReaderRequest::Command(cmd)),
        }
        true
    }

    /// Scroll the panel under the pointer instead of the document (wheel over the panel).
    pub fn wheel_at(&mut self, x: f32, y: f32, dy: f32) -> bool {
        if inside(self.panel_rect, x, y) {
            if let Some(t) = self.tabs.get_mut(self.active) {
                // 60 px per notch at 125 % like the document (measured on the reference's
                // page thumbnails too).
                t.panel_scroll = (t.panel_scroll + dy * 0.8).max(0.0);
                return true;
            }
        }
        if let Some(t) = self.tabs.get_mut(self.active) {
            if !self.show_start {
                // The reference scrolls 16 logical px per wheel line (the host sends 20).
                if self.prefs6.prefs.smooth && self.profile.is_none() {
                    return self.smooth_wheel(dy * 0.8);
                }
                t.view.scroll_by(0.0, dy * 0.8);
                return true;
            }
        }
        false
    }

    // ---- Rendering -------------------------------------------------------------------------

    /// Bring page images up to date; true when more work remains.
    pub fn prepare(&mut self, store: &mut dyn PdfStore) -> bool {
        if self.show_start {
            return false;
        }
        self.prepare_extra();
        let animating = self.view4_animating() | self.step_smooth();
        let pages_panel = self.panel_open() == Some(Panel::Pages);
        let panel = self.panel_rect;
        let thumb_w = self.px(self.thumb_w9());
        let thumb_gap = self.px(THUMB_GAP);
        let thumb_top = if self.profile.is_none() {
            self.px(marks9::THUMB_TOP) + self.px(4.0)
        } else {
            8.0
        };
        let Some(t) = self.tabs.get_mut(self.active) else {
            return false;
        };
        let mut keyed = Keyed {
            inner: store,
            base: t.key_base,
        };
        if pages_panel {
            // Thumbnails of the rows shown in the panel.
            let n = t.view.page_count();
            let mut y = panel[1] + thumb_top - t.panel_scroll;
            for i in 0..n {
                let (pw, ph) = t.view.page_size(i);
                let s = thumb_w / pw.max(1.0);
                let h = ph * s;
                if y + h > panel[1] && y < panel[1] + panel[3] {
                    t.view.want_thumb(i, s);
                }
                y += h + thumb_gap;
                if y > panel[1] + panel[3] {
                    break;
                }
            }
        }
        t.view.prepare(&mut keyed, 24) || animating
    }

    /// Paint the window.
    #[allow(clippy::too_many_lines)]
    pub fn paint(&mut self, p: &mut dyn Painter) {
        if self.paint_full(p) {
            return;
        }
        if self.dark_skin() {
            self.paint_skin(&mut dark9::DarkPainter::new(p));
        } else {
            self.paint_skin(p);
        }
    }

    /// Paint the window in the current skin's colours (`p` maps them for the dark skin).
    fn paint_skin(&mut self, p: &mut dyn Painter) {
        let (w, h) = self.size;
        p.clear(self.look.chrome);
        let ink = Ink {
            ink: TEXT,
            accent: self.theme.accent,
        };
        // Title bar.
        let th = self.title_h();
        let band = self.profile.as_ref().and_then(|p| p.title_band);
        let (title_ink, title_edge, title_hover) = match band {
            Some(c) => {
                p.fill_rect(0.0, 0.0, w, th, c);
                (WHITE, Rgba(255, 255, 255, 90), Rgba(255, 255, 255, 46))
            }
            None => (TEXT, self.look.edge, self.look.hover),
        };
        let mark = self.px(16.0);
        reader_icons::draw(
            p,
            self.profile.as_ref().map_or("app", |p| p.app_icon),
            self.px(19.2),
            ((th - mark) / 2.0).round(),
            mark,
            ink,
        );
        let sep_y = ((th - self.px(16.0)) / 2.0).round();
        p.fill_rect(self.px(46.4), sep_y, 1.0, self.px(16.0), title_edge);
        let title = self.title();
        let tstyle = self.style(14.0, true);
        let qe = self.px(self.look.quick_x) + 5.0 * self.px(self.look.quick_step);
        p.fill_rect(qe + self.px(6.4), sep_y, 1.0, self.px(16.0), title_edge);
        let mut tx = qe + self.px(19.2);
        let mut max_tx = self.search[0] - self.px(12.0);
        if self.look.title_center {
            let tw = self.width_of(&title, tstyle);
            max_tx = w - self.px(41.6) * 3.0 - self.px(12.0);
            tx = ((w - tw) / 2.0).round().max(tx);
        }
        let shown = elide(
            &title,
            self.width_of(&title, tstyle),
            max_tx - tx,
            tstyle.size,
        );
        p.text(
            &shown,
            tx,
            ((th - tstyle.size * 1.33) / 2.0).round(),
            tstyle,
            title_ink,
        );
        // Search box.
        let s = self.search;
        let edge = if self.search_focus {
            self.theme.accent
        } else {
            self.look.edge
        };
        p.path(
            &rounded(s[0] + 0.5, s[1] + 0.5, s[2] - 1.0, s[3] - 1.0, self.px(4.0)),
            Some(WHITE),
            Some((edge, 1.0)),
        );
        let si = self.px(19.2);
        reader_icons::draw(
            p,
            "search",
            s[0] + self.px(6.4),
            s[1] + ((s[3] - si) / 2.0).round(),
            si,
            Ink {
                ink: Rgba(90, 90, 92, 255),
                accent: TEXT,
            },
        );
        let sst = self.style(13.0, false);
        let (stext, scol) = if self.search_text.is_empty() && !self.search_focus {
            (self.t(self.look.search_hint), MUTED)
        } else {
            (self.search_text.clone(), TEXT)
        };
        let sty = s[1] + ((s[3] - sst.size * 1.33) / 2.0).round();
        p.text(&stext, s[0] + self.px(33.6), sty, sst, scol);
        if self.search_focus {
            let cx = s[0] + self.px(33.6) + self.width_of(&self.search_text, sst) + 1.0;
            p.fill_rect(cx.round(), sty, 1.0, sst.size * 1.33, TEXT);
        }
        if band.is_none() {
            p.fill_rect(0.0, th, w, 1.0, self.look.edge);
        }
        // Ribbon tabs.
        let tab_style = self.style(17.0, true);
        let labels = self.tab_label_keys();
        for (i, r) in self.tab_rects.iter().enumerate() {
            let label = self.strings.get(labels[i]);
            let active = if self.backstage_open {
                i == 0
            } else {
                i > 0 && i - 1 == self.ribbon_tab
            };
            let color = if active { self.theme.accent_text } else { TEXT };
            let ty = r[1] + ((r[3] - tab_style.size * 1.33) / 2.0).round();
            p.text(label, r[0] + self.px(4.0), ty, tab_style, color);
            if active {
                let lw = r[2] - self.px(8.0);
                let uy = r[1] + r[3] - self.px(self.look.underline_up);
                p.path(
                    &rounded(r[0] + self.px(4.0), uy, lw, self.px(2.4), self.px(1.2)),
                    Some(self.theme.accent),
                    None,
                );
            }
        }
        // Navigation column (its icons are controls drawn below).
        let nr = self.nav_rect;
        p.fill_rect(nr[0], nr[1], nr[2], nr[3], self.look.nav_bg);
        // Ribbon controls.
        let label_style = self.style(self.look.label, false);
        let tool_id = self.tool_id();
        for c in self.controls.clone() {
            let hot = self.hover == Some(c.id);
            let down = self.pressed == Some(c.id) && hot;
            let on = c.id == tool_id
                || self.layout_on(c.id)
                || self.panel_on(c.id)
                || self.on.contains(c.id)
                || self.tabs5_on(c.id);
            let r = c.r;
            match c.kind {
                Kind::Big(icon, label, dropdown) => {
                    // A command this window does not offer: grey, no hover.
                    let off = !self.enabled(c.id);
                    let (ink, label_ink) = if off {
                        let g = Rgba(175, 175, 176, 255);
                        (Ink { ink: g, accent: g }, g)
                    } else {
                        (ink, TEXT)
                    };
                    if !off && (on || down || hot) {
                        let fill = if on || down {
                            self.look.pressed
                        } else {
                            self.look.hover
                        };
                        p.path(
                            &rounded(r[0], r[1], r[2], r[3], self.px(3.2)),
                            Some(fill),
                            None,
                        );
                    }
                    let is = self.px(self.look.icon);
                    reader_icons::draw_weight(
                        p,
                        icon,
                        r[0] + ((r[2] - is) / 2.0).round(),
                        r[1] + self.px(self.look.icon_y),
                        is,
                        ink,
                        self.look.icon_stroke,
                    );
                    let text = self.t(label);
                    let lines: Vec<String> = if text.contains(' ')
                        && (text.len() > self.wrap_after9() || self.look.wrap_words)
                    {
                        let (a, b) = self.split_label9(&text);
                        vec![a.to_owned(), b.to_owned()]
                    } else {
                        vec![text]
                    };
                    let lh = (label_style.size * 1.33 * 0.93).round();
                    let mut ly = r[1] + self.px(self.look.label_y);
                    let n = lines.len();
                    for (k, l) in lines.iter().enumerate() {
                        let lw = self.width_of(l, label_style);
                        let extra = if dropdown && k == n - 1 && n > 1 {
                            self.px(8.0)
                        } else {
                            0.0
                        };
                        let lx = r[0] + ((r[2] - lw - extra) / 2.0).round();
                        p.text(l, lx, ly, label_style, label_ink);
                        if dropdown && k == n - 1 && n > 1 {
                            tri(
                                p,
                                lx + lw + self.px(2.0),
                                ly + lh * 0.45,
                                self.px(3.2),
                                label_ink,
                            );
                        }
                        ly += lh;
                    }
                    if dropdown && n == 1 {
                        tri(
                            p,
                            r[0] + r[2] / 2.0 - self.px(3.2),
                            ly + self.px(3.2),
                            self.px(3.2),
                            label_ink,
                        );
                    }
                }
                Kind::Small(_) | Kind::Quick(_) | Kind::Nav(_) => {
                    let icon = match c.kind {
                        Kind::Nav(Panel::Bookmarks) => "bookmark",
                        Kind::Nav(Panel::Pages) => "pages",
                        Kind::Nav(Panel::Comments) => "comments",
                        Kind::Nav(Panel::Extra) => self.extra_panel.as_ref().map_or("", |e| e.icon),
                        Kind::Nav(Panel::Layers) => "layers",
                        Kind::Nav(Panel::Attachments) => "attach",
                        Kind::Small(i) | Kind::Quick(i) => i,
                        _ => "",
                    };
                    let enabled = self.enabled(c.id);
                    let quick_band = band.is_some() && matches!(c.kind, Kind::Quick(_));
                    let tab8 = on && matches!(c.kind, Kind::Nav(_)) && self.profile.is_none();
                    if tab8 {
                        self.paint_nav_tab8(p, r);
                    } else if enabled && (on || down || hot) {
                        let fill = if quick_band {
                            title_hover
                        } else if on || down {
                            self.look.pressed
                        } else {
                            self.look.hover
                        };
                        p.path(
                            &rounded(r[0], r[1], r[2], r[3], self.px(2.4)),
                            Some(fill),
                            None,
                        );
                    }
                    let is = match c.kind {
                        Kind::Quick(_) => self.px(19.2),
                        Kind::Nav(_) => self.px(20.8),
                        // Text markup marks are drawn larger, like the reference's (round 5).
                        Kind::Small(i) if tabs5::markup_icon(i) => self.px(20.8),
                        _ => self.px(17.6),
                    };
                    let col = match (enabled, quick_band) {
                        (true, true) => WHITE,
                        (false, true) => Rgba(255, 255, 255, 120),
                        (true, false) => TEXT,
                        (false, false) => Rgba(175, 175, 176, 255),
                    };
                    let accent = if matches!(c.kind, Kind::Nav(_)) && on {
                        self.theme.accent
                    } else {
                        col
                    };
                    let ink2 = Ink {
                        ink: if matches!(c.kind, Kind::Nav(_)) && on {
                            self.theme.accent
                        } else {
                            col
                        },
                        accent,
                    };
                    reader_icons::draw_weight(
                        p,
                        icon,
                        r[0] + ((r[2] - is) / 2.0).round(),
                        r[1] + ((r[3] - is) / 2.0).round(),
                        is,
                        ink2,
                        self.look.icon_stroke,
                    );
                    if c.kind == Kind::Nav(Panel::Attachments) {
                        self.paint_nav_badge8(p, r);
                    }
                    if matches!(c.kind, Kind::Small(_)) {
                        self.paint_small_arrow9(p, c.id, r, col);
                    }
                }
                Kind::Row(icon, label, dropdown) => {
                    let enabled = self.enabled(c.id);
                    if enabled && (on || down || hot) {
                        let fill = if on || down {
                            self.look.pressed
                        } else {
                            self.look.hover
                        };
                        p.path(
                            &rounded(r[0], r[1], r[2], r[3], self.px(2.4)),
                            Some(fill),
                            None,
                        );
                    }
                    let col = if enabled {
                        TEXT
                    } else {
                        Rgba(175, 175, 176, 255)
                    };
                    let is = self.px(17.6);
                    let ix = r[0] + self.px(4.0);
                    reader_icons::draw_weight(
                        p,
                        icon,
                        ix,
                        r[1] + ((r[3] - is) / 2.0).round(),
                        is,
                        Ink {
                            ink: col,
                            accent: col,
                        },
                        self.look.icon_stroke,
                    );
                    let mut tx = ix + is;
                    if !label.is_empty() {
                        let text = self.t(label);
                        tx += self.px(2.4);
                        let ty = r[1] + ((r[3] - label_style.size * 1.33) / 2.0).round();
                        p.text(&text, tx, ty, label_style, col);
                        tx += self.width_of(&text, label_style);
                    }
                    if dropdown {
                        tri(
                            p,
                            tx + self.px(3.2),
                            r[1] + r[3] / 2.0 - self.px(1.6),
                            self.px(3.2),
                            col,
                        );
                    }
                }
                Kind::Win(k) => {
                    let close = k == NcHit::Close;
                    if hot {
                        let fill = if close { CLOSE_BG } else { title_hover };
                        p.fill_rect(r[0], r[1], r[2], r[3], fill);
                    }
                    let col = if hot && close { WHITE } else { title_ink };
                    win_glyph(p, k, r, self.scale, self.maximized, col);
                }
                Kind::Sep => p.fill_rect(r[0], r[1], 1.0, r[3], self.look.edge),
            }
        }
        let rb = self.ribbon_bottom();
        p.fill_rect(0.0, rb, w, 1.0, self.look.edge);
        // Document tabs strip.
        self.paint_doc_tabs(p, rb + 1.0);
        let top = rb + 1.0 + self.doc_tabs_h();
        p.fill_rect(0.0, top - 1.0, w, 1.0, self.look.edge);
        if self.show_start || self.tabs.is_empty() {
            self.paint_start(p);
        } else {
            self.paint_panel(p);
            let sr = self.strip_rect;
            p.fill_rect(sr[0], sr[1], sr[2], sr[3], self.look.strip);
            if self.look.strip_lines {
                p.fill_rect(sr[0], sr[1], 1.0, sr[3], self.look.edge);
                p.fill_rect(sr[0] + sr[2] - 1.0, sr[1], 1.0, sr[3], self.look.edge);
            }
            let open = self.panel_open().is_some();
            let ay = sr[1] + sr[3] / 2.0;
            let ax = sr[0] + sr[2] / 2.0;
            let a = self.px(3.2);
            let segs = if open {
                vec![
                    PathSeg::M(ax + a / 2.0, ay - a),
                    PathSeg::L(ax - a / 2.0, ay),
                    PathSeg::L(ax + a / 2.0, ay + a),
                    PathSeg::Z,
                ]
            } else {
                vec![
                    PathSeg::M(ax - a / 2.0, ay - a),
                    PathSeg::L(ax + a / 2.0, ay),
                    PathSeg::L(ax - a / 2.0, ay + a),
                    PathSeg::Z,
                ]
            };
            p.path(&segs, Some(Rgba(20, 20, 20, 255)), None);
            if self.doc_hidden {
                let d = self.doc_rect;
                p.fill_rect(d[0], d[1], d[2] + self.vbar[2], d[3], self.look.canvas);
            } else {
                self.paint_doc(p);
                self.paint_vbar(p);
            }
        }
        // Status bar.
        let sh = self.status_h();
        p.fill_rect(0.0, h - sh, w, 1.0, self.look.edge);
        self.paint_status(p);
        // Window border (1 px, measured).
        p.fill_rect(0.0, 0.0, 1.0, h, self.look.edge);
        p.fill_rect(w - 1.0, 0.0, 1.0, h, self.look.edge);
        p.fill_rect(0.0, h - 1.0, w, 1.0, self.look.edge);
        self.paint_extra(p);
        self.paint_search9(p);
        self.paint_marks9(p);
        self.paint_popups10(p);
        self.paint_bar10(p);
        if let Some((menu, _)) = &self.menu {
            menu.paint(p, &self.theme, &self.icons);
        }
        if let Some((d, _)) = &self.dialog {
            d.paint(p, &self.theme, self.size, true);
        }
        self.paint_extra_dialogs(p);
        self.paint_panels8_overlay(p);
        self.paint_tip9(p);
    }

    fn tool_id(&self) -> &'static str {
        if self.tool_hidden {
            return "";
        }
        match self.tool {
            Tool::Hand => "tool.hand",
            Tool::Select => "tool.select",
            Tool::Markup(AnnotKind::Highlight) => "tool.highlight",
            Tool::Markup(AnnotKind::Underline) => "tool.underline",
            Tool::Markup(AnnotKind::StrikeOut) => "tool.strikeout",
            Tool::Markup(_) => "tool.squiggly",
            Tool::Note => "tool.note",
            Tool::TextBox => "tool.textbox",
            Tool::Rectangle => "tool.rectangle",
        }
    }

    fn layout_on(&self, id: &str) -> bool {
        let Some(v) = self.view() else { return false };
        matches!(
            (id, v.layout()),
            ("view.single", Layout::Single)
                | ("view.continuous", Layout::Continuous)
                | ("view.facing", Layout::Facing)
                | ("view.contFacing", Layout::ContinuousFacing)
        )
    }

    fn panel_on(&self, id: &str) -> bool {
        matches!(
            (id, self.panel_open()),
            ("nav.bookmarks" | "panel.bookmarks", Some(Panel::Bookmarks))
                | ("nav.pages" | "panel.pages", Some(Panel::Pages))
                | ("nav.comments" | "panel.comments", Some(Panel::Comments))
                | ("nav.layers" | "panel.layers", Some(Panel::Layers))
                | (
                    "nav.attachments" | "panel.attachments",
                    Some(Panel::Attachments)
                )
        ) || (self.panel_open() == Some(Panel::Extra)
            && self.extra_panel.as_ref().is_some_and(|e| e.nav_id == id))
    }

    fn enabled(&self, id: &str) -> bool {
        if let Some(&e) = self.enabled_override.get(id) {
            return e;
        }
        if let Some(e) = self.tabs5_enabled(id) {
            return e && self.showing_doc();
        }
        if let Some(e) = self.tabs9_enabled(id) {
            return e && self.showing_doc();
        }
        if Self::tabs4_disabled(id) {
            return false;
        }
        let Some(v) = self.view() else {
            return id == "file.open";
        };
        if self.show_start {
            return id == "file.open";
        }
        let cur = v.current_page();
        let n = v.page_count();
        match id {
            "page.first" | "page.prev" => cur > 0,
            "page.next" | "page.last" => cur + 1 < n,
            "edit.undo" | "edit.redo" | "view.prev" | "view.next" => false,
            "file.save" => v.modified(),
            _ => true,
        }
    }

    fn paint_doc_tabs(&mut self, p: &mut dyn Painter, y: f32) {
        let (w, _) = self.size;
        let hgt = self.doc_tabs_h() - 1.0;
        p.fill_rect(1.0, y, w - 2.0, hgt, self.look.strip_bg);
        let st = self.style(14.0, false);
        let tab_w = self.px(self.look.doc_tab_w);
        let ty0 = y + self.px(7.2);
        let th = hgt - self.px(7.2);
        let mut x = self.px(10.4);
        // Start tab.
        let start_on = self.show_start || self.tabs.is_empty();
        self.start_tab = [x, ty0, tab_w, th];
        self.paint_tab(
            p,
            self.start_tab,
            &self.t("pdfReader.start"),
            start_on,
            false,
            st,
        );
        x += tab_w + self.px(1.6);
        let mut rects = Vec::new();
        for i in 0..self.tabs.len() {
            let r = [x, ty0, tab_w, th];
            let on = !start_on && i == self.active;
            let name = self.tabs[i].name.clone();
            let modified = self.tabs[i].view.modified();
            let label = if modified { format!("{name}*") } else { name };
            self.paint_tab(p, r, &label, on, true, st);
            let cs = self.px(12.8);
            let close = [
                r[0] + r[2] - cs - self.px(6.4),
                r[1] + ((r[3] - cs) / 2.0).round(),
                cs,
                cs,
            ];
            if on {
                reader_icons::draw(
                    p,
                    "close",
                    close[0],
                    close[1],
                    cs,
                    Ink {
                        ink: TEXT,
                        accent: TEXT,
                    },
                );
            }
            rects.push((r, close));
            x += tab_w + self.px(1.6);
        }
        self.doc_tab_rects = rects;
    }

    fn paint_tab(
        &self,
        p: &mut dyn Painter,
        r: [f32; 4],
        label: &str,
        on: bool,
        closable: bool,
        st: TextStyle,
    ) {
        if on {
            p.fill_rect(r[0], r[1], r[2], r[3] + 1.0, self.look.edge);
            p.fill_rect(r[0] + 1.0, r[1] + 1.0, r[2] - 2.0, r[3] + 1.0, WHITE);
        } else {
            p.fill_rect(r[0], r[1], r[2], r[3], self.look.edge);
            p.fill_rect(
                r[0] + 1.0,
                r[1] + 1.0,
                r[2] - 2.0,
                r[3] - 1.0,
                self.look.canvas,
            );
        }
        let color = if on && closable {
            self.theme.accent_text
        } else {
            TEXT
        };
        let max = r[2] - self.px(30.0);
        let shown = elide(label, self.width_of(label, st), max, st.size);
        p.text(
            &shown,
            r[0] + self.px(8.0),
            r[1] + ((r[3] - st.size * 1.33) / 2.0).round(),
            st,
            color,
        );
    }

    /// Start page: a "Recent Files" card with an Open File row, as on the reference's start tab
    /// (125 %: card 34–409 px, title 292 px, rows 29.5 px apart; no promotions).
    fn paint_start(&mut self, p: &mut dyn Painter) {
        let top = self.nav_rect[1];
        let (w, h) = self.size;
        let d = [1.0, top, w - 2.0, h - self.status_h() - top];
        p.fill_rect(d[0], d[1], d[2], d[3], Rgba(235, 236, 240, 255));
        let card = [
            self.px(27.2),
            top + self.px(20.0),
            self.px(300.0),
            d[3] - self.px(40.0),
        ];
        p.fill_rect(card[0], card[1], card[2], card[3], WHITE);
        let title = self.style(19.0, false);
        let body = self.style(15.0, false);
        let x = card[0] + self.px(10.4);
        p.text(
            &self.t("pdfReader.recent"),
            x,
            card[1] + self.px(10.4),
            title,
            TEXT,
        );
        let line_y = card[1] + self.px(36.0);
        p.fill_rect(
            x,
            line_y,
            card[2] - self.px(20.8),
            1.0,
            Rgba(229, 229, 229, 255),
        );
        let row = self.px(23.6);
        let mut y = line_y + self.px(12.0);
        let ink = Ink {
            ink: TEXT,
            accent: self.theme.accent,
        };
        self.recent_rects.clear();
        let mut rows: Vec<(String, &'static str)> = vec![(self.t("pdfReader.openFile"), "open")];
        rows.extend(self.recent.iter().map(|r| {
            (
                r.file_name().map_or_else(
                    || r.to_string_lossy().into_owned(),
                    |n| n.to_string_lossy().into_owned(),
                ),
                "file",
            )
        }));
        let is = self.px(14.4);
        for (label, icon) in rows {
            if y + row > card[1] + card[3] {
                break;
            }
            let rr = [card[0], y, card[2], row];
            if inside(rr, self.pointer.0, self.pointer.1) {
                p.fill_rect(rr[0], rr[1], rr[2], rr[3], self.look.hover);
            }
            reader_icons::draw(p, icon, x, y + ((row - is) / 2.0).round(), is, ink);
            let shown = elide(
                &label,
                self.width_of(&label, body),
                card[2] - self.px(44.0),
                body.size,
            );
            p.text(
                &shown,
                x + self.px(19.2),
                y + ((row - body.size * 1.33) / 2.0).round(),
                body,
                TEXT,
            );
            self.recent_rects.push(rr);
            y += row;
        }
    }

    #[allow(clippy::too_many_lines)]
    fn paint_panel(&mut self, p: &mut dyn Painter) {
        let r = self.panel_rect;
        self.panel_rows.clear();
        if r[2] <= 0.0 {
            return;
        }
        p.fill_rect(r[0], r[1], r[2], r[3], self.look.chrome);
        self.paint_panel_edge8(p);
        let Some(panel) = self.panel_open() else {
            return;
        };
        let title = match panel {
            Panel::Bookmarks => self.t("pdfReader.bookmarks"),
            Panel::Pages => self.t("pdfReader.pages"),
            Panel::Comments => self.t("pdfReader.comments"),
            Panel::Extra => self
                .extra_panel
                .as_ref()
                .map_or_else(String::new, |e| e.title.clone()),
            Panel::Layers => self.t("pdfPanels.layers"),
            Panel::Attachments => self.t("pdfPanels.attachments"),
        };
        if self.panel8(panel) {
            // Measured at 125 %: title 3 px higher and larger than the shared one.
            let ts = self.style(17.5, false);
            p.text(&title, r[0] + self.px(8.8), r[1] + self.px(7.6), ts, TEXT);
            self.paint_panel8(p, r, panel);
            return;
        }
        let ts = self.style(16.0, false);
        p.text(&title, r[0] + self.px(8.8), r[1] + self.px(10.4), ts, TEXT);
        let list_top = r[1]
            + if panel == Panel::Bookmarks {
                self.paint_bookmark_tools(p, r)
            } else if panel == Panel::Pages && self.profile.is_none() {
                self.paint_thumb_tools9(p, r)
            } else {
                self.px(40.0)
            };
        p.push_clip(r[0], list_top, r[2], r[3] - (list_top - r[1]));
        let body = self.style(14.0, false);
        let scroll = self.tabs.get(self.active).map_or(0.0, |t| t.panel_scroll);
        let mut y = list_top + self.px(4.0) - scroll;
        match panel {
            Panel::Bookmarks => self.paint_bookmarks(p, r, list_top, y),
            Panel::Pages => {
                let Some(t) = self.tabs.get(self.active) else {
                    p.pop_clip();
                    return;
                };
                let n = t.view.page_count();
                let cur = t.view.current_page();
                let key_base = t.key_base;
                let tw = self.px(self.thumb_w9());
                let mut rows = Vec::new();
                for i in 0..n {
                    let (pw, ph) = t.view.page_size(i);
                    let s = tw / pw.max(1.0);
                    let hh = (ph * s).round();
                    if y > r[1] + r[3] {
                        break;
                    }
                    if y + hh > list_top {
                        let x = (r[0] + (r[2] - tw) / 2.0).round();
                        // BlueReader marks the current page by its visible area (round 9).
                        if i == cur && self.profile.is_some() {
                            p.fill_rect(x - 4.0, y - 4.0, tw + 8.0, hh + 8.0, self.theme.accent);
                            p.fill_rect(x - 2.0, y - 2.0, tw + 4.0, hh + 4.0, WHITE);
                        }
                        // The reference: a 2 px light grey frame (207, 207, 208 at 125 %).
                        let b = self.px(1.6).round().max(1.0);
                        p.fill_rect(x - b, y - b, tw + 2.0 * b, hh + 2.0 * b, THUMB_EDGE);
                        p.raw_colours(true);
                        p.fill_rect(x, y, tw, hh, WHITE);
                        if t.view.has_image(THUMB_KEY + i) {
                            p.page(key_base + THUMB_KEY + i, x as i32, y as i32);
                        }
                        p.raw_colours(false);
                        let label = page_label_short(t.view.doc(), i);
                        let lw = self.width_of(&label, body);
                        p.text(
                            &label,
                            (r[0] + (r[2] - lw) / 2.0).round(),
                            y + hh + self.px(4.0),
                            body,
                            TEXT,
                        );
                    }
                    rows.push((y, hh + self.px(24.0), PanelRow::Page(i)));
                    y += hh + self.px(THUMB_GAP);
                }
                self.panel_rows = rows;
            }
            Panel::Comments => {
                let Some(t) = self.tabs.get(self.active) else {
                    p.pop_clip();
                    return;
                };
                let list = self.comments_shown(t.view.doc().comment_threads());
                y += self.paint_comment_header(p, r);
                if list.is_empty() {
                    p.text(
                        &self.t("pdfReader.noComments"),
                        r[0] + self.px(8.8),
                        y,
                        body,
                        MUTED,
                    );
                }
                let small = self.style(12.5, false);
                let mut last_page = usize::MAX;
                let mut rows = Vec::new();
                for thread in list {
                    let a = thread.comment;
                    if a.page != last_page {
                        last_page = a.page;
                        p.text(
                            &format!("{} {}", self.t("pdfReader.page"), a.page + 1),
                            r[0] + self.px(8.8),
                            y,
                            small,
                            MUTED,
                        );
                        y += self.px(20.0);
                    }
                    let c = Rgba(a.color[0], a.color[1], a.color[2], 255);
                    p.fill_rect(r[0] + self.px(8.8), y + 2.0, self.px(4.0), self.px(32.0), c);
                    let kind = kind_label(a.kind);
                    p.text(
                        &format!("{} · {}", self.t(kind), a.author),
                        r[0] + self.px(17.6),
                        y,
                        small,
                        MUTED,
                    );
                    let text = if a.contents.is_empty() {
                        String::from("—")
                    } else {
                        a.contents.replace('\n', " ")
                    };
                    let shown = elide(
                        &text,
                        self.width_of(&text, body),
                        r[2] - self.px(28.0),
                        body.size,
                    );
                    p.text(&shown, r[0] + self.px(17.6), y + self.px(16.0), body, TEXT);
                    let selected = self
                        .extra
                        .selected
                        .as_ref()
                        .is_some_and(|s| s.page == a.page && s.rect == a.rect);
                    if selected {
                        p.fill_rect(
                            r[0] + self.px(4.0),
                            y - 2.0,
                            2.0,
                            self.px(36.0),
                            self.theme.accent,
                        );
                    }
                    rows.push((y, self.px(38.0), PanelRow::Comment(a.page, a.rect)));
                    y += self.px(44.0);
                    // Replies, indented under the comment.
                    for reply in thread.replies {
                        let rx = r[0] + self.px(30.0);
                        p.text(&format!("↳ {}", reply.author), rx, y, small, MUTED);
                        let text = reply.contents.replace('\n', " ");
                        let shown = elide(
                            &text,
                            self.width_of(&text, body),
                            r[2] - self.px(40.0),
                            body.size,
                        );
                        p.text(&shown, rx, y + self.px(16.0), body, TEXT);
                        rows.push((y, self.px(38.0), PanelRow::Comment(a.page, a.rect)));
                        y += self.px(40.0);
                    }
                }
                self.panel_rows = rows;
            }
            Panel::Layers | Panel::Attachments => {}
            Panel::Extra => {
                let extra = self.extra_panel.clone().unwrap_or_default();
                if extra.rows.is_empty() {
                    p.text(&extra.empty, r[0] + self.px(8.8), y, body, MUTED);
                }
                let small = self.style(12.5, false);
                let mut rows = Vec::new();
                for (color, title, detail, cmd) in extra.rows {
                    // Status mark: a filled circle in the row's colour.
                    let d = self.px(12.0);
                    let (cx, cy) = (r[0] + self.px(8.8), y + self.px(3.0));
                    let k = 0.5523 * d / 2.0;
                    let (mx, my, rr) = (cx + d / 2.0, cy + d / 2.0, d / 2.0);
                    p.path(
                        &[
                            PathSeg::M(mx + rr, my),
                            PathSeg::C(mx + rr, my + k, mx + k, my + rr, mx, my + rr),
                            PathSeg::C(mx - k, my + rr, mx - rr, my + k, mx - rr, my),
                            PathSeg::C(mx - rr, my - k, mx - k, my - rr, mx, my - rr),
                            PathSeg::C(mx + k, my - rr, mx + rr, my - k, mx + rr, my),
                            PathSeg::Z,
                        ],
                        Some(color),
                        None,
                    );
                    let tx = r[0] + self.px(26.0);
                    let w = r[2] - self.px(34.0);
                    let shown = elide(&title, self.width_of(&title, body), w, body.size);
                    p.text(&shown, tx, y, body, TEXT);
                    let shown = elide(&detail, self.width_of(&detail, small), w, small.size);
                    p.text(&shown, tx, y + self.px(18.0), small, MUTED);
                    rows.push((y, self.px(38.0), PanelRow::Extra(cmd)));
                    y += self.px(44.0);
                }
                self.panel_rows = rows;
            }
        }
        p.pop_clip();
    }

    fn paint_doc(&self, p: &mut dyn Painter) {
        let d = self.doc_rect;
        p.fill_rect(d[0], d[1], d[2], d[3], self.doc_canvas());
        self.paint_split(p);
        p.raw_colours(true);
        let reflowed = self.paint_reflow(p);
        p.raw_colours(false);
        if reflowed {
            self.paint_viewx_overlay(p);
            return;
        }
        let Some(t) = self.tabs.get(self.active) else {
            return;
        };
        let v = &t.view;
        p.push_clip(d[0], d[1], d[2], d[3]);
        p.raw_colours(true);
        let s = v.px_per_pt();
        for i in v.visible_pages() {
            let Some(r) = v.page_rect(i) else { continue };
            p.fill_rect(r[0] - 1.0, r[1] - 1.0, r[2] + 2.0, r[3] + 2.0, PAGE_EDGE);
            p.fill_rect(r[0], r[1], r[2], r[3], self.paper());
            if v.has_image(i) {
                let (ox, oy) = v.image_offset(i);
                p.page(t.key_base + i, (r[0] + ox) as i32, (r[1] + oy) as i32);
            }
            for (hr, cur) in v.hit_rects(i) {
                p.fill_rect(
                    r[0] + hr[0] * s,
                    r[1] + hr[1] * s,
                    (hr[2] - hr[0]) * s,
                    (hr[3] - hr[1]) * s,
                    if cur { HIT_CURRENT } else { HIT },
                );
            }
            if v.selection().is_some_and(|sel| sel.page == i) {
                for sr in v.selection_rects() {
                    p.fill_rect(
                        r[0] + sr[0] * s,
                        r[1] + sr[1] * s,
                        (sr[2] - sr[0]) * s,
                        (sr[3] - sr[1]) * s,
                        SELECTION,
                    );
                }
            }
        }
        p.raw_colours(false);
        p.pop_clip();
        self.paint_viewx_overlay(p);
    }

    fn paint_vbar(&self, p: &mut dyn Painter) {
        let b = self.vbar;
        p.fill_rect(b[0] - 1.0, b[1], 1.0, b[3], WHITE);
        p.fill_rect(b[0], b[1], b[2], b[3], TRACK);
        // Split box at the top.
        let sb = self.px(8.8);
        p.fill_rect(
            b[0] - 1.0,
            b[1],
            b[2] + 1.0,
            sb + 1.0,
            Rgba(145, 151, 168, 255),
        );
        p.fill_rect(b[0], b[1], b[2] - 1.0, sb, self.look.strip);
        let (ty, th) = self.thumb();
        let tw = 3.0_f32.max((2.4 * self.scale).round());
        p.fill_rect((b[0] + (b[2] - tw) / 2.0).round(), ty, tw, th, THUMB);
    }

    #[allow(clippy::too_many_lines)]
    fn paint_status(&self, p: &mut dyn Painter) {
        let (w, h) = self.size;
        let sh = self.status_h();
        let _ = w;
        let st = self.style(self.look.status_font, false);
        // Page box.
        let b = self.page_box;
        p.fill_rect(b[0], b[1], b[2], b[3], self.look.box_edge);
        p.fill_rect(b[0] + 1.0, b[1] + 1.0, b[2] - 2.0, b[3] - 2.0, WHITE);
        let label = self.view().map_or_else(String::new, |v| {
            let n = v.page_count();
            let i = v.current_page();
            page_label(v.doc(), i, n)
        });
        p.text(
            &label,
            b[0] + self.px(4.0),
            b[1] + ((b[3] - st.size * 1.33) / 2.0).round(),
            st,
            TEXT,
        );
        tri(
            p,
            b[0] + b[2] - self.px(10.4),
            b[1] + b[3] / 2.0 - self.px(1.6),
            self.px(3.2),
            TEXT,
        );
        // Zoom box and slider.
        let z = self.zoom_box;
        p.fill_rect(z[0], z[1], z[2], z[3], self.look.box_edge);
        p.fill_rect(z[0] + 1.0, z[1] + 1.0, z[2] - 2.0, z[3] - 2.0, WHITE);
        let zt = self
            .view()
            .map_or_else(String::new, |v| format!("{:.2}%", v.zoom() * 100.0));
        p.text(
            &zt,
            z[0] + self.px(4.0),
            z[1] + ((z[3] - st.size * 1.33) / 2.0).round(),
            st,
            TEXT,
        );
        tri(
            p,
            z[0] + z[2] - self.px(10.4),
            z[1] + z[3] / 2.0 - self.px(1.6),
            self.px(3.2),
            Rgba(150, 150, 151, 255),
        );
        let sl = self.slider;
        let cy = (sl[1] + sl[3] / 2.0).round();
        p.fill_rect(sl[0], cy, sl[2], 1.0, Rgba(160, 160, 162, 255));
        let kx = sl[0] + self.slider_pos() * sl[2];
        let kr = self.px(5.6);
        p.path(
            &circle(kx, cy + 0.5, kr),
            Some(WHITE),
            Some((Rgba(100, 100, 101, 255), 1.0)),
        );
        // Status message (search results, errors) after the view buttons.
        if !self.status.is_empty() {
            let x = self.px(304.0);
            p.text(
                &self.status,
                x,
                h - sh + ((sh - st.size * 1.33) / 2.0).round(),
                st,
                TEXT,
            );
        }
    }
}

/// Wraps a store so each document tab uses its own image keys.
struct Keyed<'a> {
    inner: &'a mut dyn PdfStore,
    base: usize,
}

impl PdfStore for Keyed<'_> {
    fn update(
        &mut self,
        key: usize,
        list: &bo_render::PageList,
        fonts: &bo_text::FontManager,
        images: &std::collections::HashMap<String, std::sync::Arc<bo_pdfread::Image>>,
        scale: f32,
    ) -> Result<(), String> {
        self.inner
            .update(self.base + key, list, fonts, images, scale)
    }
    fn update_band(
        &mut self,
        key: usize,
        list: &bo_render::PageList,
        fonts: &bo_text::FontManager,
        images: &std::collections::HashMap<String, std::sync::Arc<bo_pdfread::Image>>,
        scale: f32,
        band: [u32; 4],
    ) -> Result<(), String> {
        self.inner
            .update_band(self.base + key, list, fonts, images, scale, band)
    }
    fn remove(&mut self, key: usize) {
        self.inner.remove(self.base + key);
    }
}

fn inside(r: [f32; 4], x: f32, y: f32) -> bool {
    x >= r[0] && x < r[0] + r[2] && y >= r[1] && y < r[1] + r[3]
}

fn word_around(layer: Option<&bo_pdfread::TextLayer>, c: usize) -> (usize, usize) {
    let Some(l) = layer else { return (c, c) };
    let chars = l.chars();
    let word = |i: usize| {
        chars
            .get(i)
            .is_some_and(|ch| ch.text.chars().all(char::is_alphanumeric) && !ch.text.is_empty())
    };
    let mut a = c;
    while a > 0
        && word(a - 1)
        && (chars[a].rect[0] - chars[a - 1].rect[2]).abs() < chars[a].size * 0.15
    {
        a -= 1;
    }
    let mut b = c;
    while b + 1 < chars.len()
        && word(b + 1)
        && (chars[b + 1].rect[0] - chars[b].rect[2]).abs() < chars[b].size * 0.15
    {
        b += 1;
    }
    (a, b)
}

fn flatten_outline<'o>(
    items: &'o [OutlineItem],
    expanded: &HashSet<Vec<usize>>,
    path: &mut Vec<usize>,
    depth: usize,
    out: &mut Vec<(Vec<usize>, &'o OutlineItem, usize, bool, bool)>,
) {
    for (k, it) in items.iter().enumerate() {
        path.push(k);
        let open = expanded.contains(path.as_slice());
        out.push((path.clone(), it, depth, !it.children.is_empty(), open));
        if open && depth < 16 {
            flatten_outline(&it.children, expanded, path, depth + 1, out);
        }
        path.pop();
    }
}

fn page_label(doc: &PdfDocument, i: usize, n: usize) -> String {
    let label = doc
        .page_info(i)
        .map_or_else(String::new, |p| p.label.clone());
    if !doc.has_page_labels() || label.is_empty() {
        format!("{} / {n}", i + 1)
    } else {
        format!("{label} ({} / {n})", i + 1)
    }
}

fn page_label_short(doc: &PdfDocument, i: usize) -> String {
    doc.page_info(i)
        .map_or_else(|| (i + 1).to_string(), |p| p.label.clone())
}

fn kind_label(k: AnnotKind) -> &'static str {
    match k {
        AnnotKind::Highlight => "pdfReader.highlight",
        AnnotKind::Underline => "pdfReader.underline",
        AnnotKind::StrikeOut => "pdfReader.strikeout",
        AnnotKind::Squiggly => "pdfReader.squiggly",
        AnnotKind::Note => "pdfReader.note",
        AnnotKind::TextBox => "pdfReader.textbox",
        AnnotKind::Square | AnnotKind::Circle => "pdfReader.rectangle",
        _ => "pdfReader.comment",
    }
}

/// Cut `s` with an ellipsis to fit `max` pixels (estimated per character from `w`).
fn elide(s: &str, w: f32, max: f32, size: f32) -> String {
    if w <= max || max <= 0.0 {
        return s.to_owned();
    }
    let n = s.chars().count();
    let per = if n > 0 { w / n as f32 } else { size * 0.55 };
    let keep = ((max / per) as usize).saturating_sub(2);
    let mut out: String = s.chars().take(keep).collect();
    out.push_str("...");
    out
}

pub(crate) fn rounded(x: f32, y: f32, w: f32, h: f32, r: f32) -> Vec<PathSeg> {
    let r = r.min(w / 2.0).min(h / 2.0);
    vec![
        PathSeg::M(x + r, y),
        PathSeg::L(x + w - r, y),
        PathSeg::Q(x + w, y, x + w, y + r),
        PathSeg::L(x + w, y + h - r),
        PathSeg::Q(x + w, y + h, x + w - r, y + h),
        PathSeg::L(x + r, y + h),
        PathSeg::Q(x, y + h, x, y + h - r),
        PathSeg::L(x, y + r),
        PathSeg::Q(x, y, x + r, y),
        PathSeg::Z,
    ]
}

fn circle(cx: f32, cy: f32, r: f32) -> Vec<PathSeg> {
    let k = 0.552_284_8 * r;
    vec![
        PathSeg::M(cx + r, cy),
        PathSeg::C(cx + r, cy + k, cx + k, cy + r, cx, cy + r),
        PathSeg::C(cx - k, cy + r, cx - r, cy + k, cx - r, cy),
        PathSeg::C(cx - r, cy - k, cx - k, cy - r, cx, cy - r),
        PathSeg::C(cx + k, cy - r, cx + r, cy - k, cx + r, cy),
        PathSeg::Z,
    ]
}

/// A small down-pointing triangle with its top-left at (`x`, `y`).
fn tri(p: &mut dyn Painter, x: f32, y: f32, s: f32, c: Rgba) {
    let (x, y) = (x.round(), y.round());
    p.path(
        &[
            PathSeg::M(x, y),
            PathSeg::L(x + 2.0 * s, y),
            PathSeg::L(x + s, y + s),
            PathSeg::Z,
        ],
        Some(c),
        None,
    );
}

fn win_glyph(p: &mut dyn Painter, k: NcHit, b: [f32; 4], scale: f32, maximized: bool, c: Rgba) {
    let g = (12.0 * scale).round();
    let (cx, cy) = (
        (b[0] + b[2] / 2.0).round() + 0.5,
        (b[1] + b[3] / 2.0).round() + 0.5,
    );
    let (l, t, r, bt) = (cx - g / 2.0, cy - g / 2.0, cx + g / 2.0, cy + g / 2.0);
    let w = scale.max(1.0).round();
    let segs: Vec<PathSeg> = match k {
        NcHit::Minimize => vec![PathSeg::M(l, cy), PathSeg::L(r, cy)],
        NcHit::Maximize if maximized => {
            let o = (2.5 * scale).round();
            vec![
                PathSeg::M(l, t + o),
                PathSeg::L(r - o, t + o),
                PathSeg::L(r - o, bt),
                PathSeg::L(l, bt),
                PathSeg::Z,
                PathSeg::M(l + o, t + o),
                PathSeg::L(l + o, t),
                PathSeg::L(r, t),
                PathSeg::L(r, bt - o),
                PathSeg::L(r - o, bt - o),
            ]
        }
        NcHit::Maximize => vec![
            PathSeg::M(l, t),
            PathSeg::L(r, t),
            PathSeg::L(r, bt),
            PathSeg::L(l, bt),
            PathSeg::Z,
        ],
        _ => vec![
            PathSeg::M(l, t),
            PathSeg::L(r, bt),
            PathSeg::M(r, t),
            PathSeg::L(l, bt),
        ],
    };
    p.path(&segs, None, Some((c, w)));
}

/// Other open documents for BlueEditor's page grid (dragging pages onto another tab).
impl ReaderWindow {
    /// Document tab under a window point (from the last paint).
    pub(crate) fn doc_tab_at(&self, x: f32, y: f32) -> Option<usize> {
        self.doc_tab_rects
            .iter()
            .position(|(r, _)| x >= r[0] && x < r[0] + r[2] && y >= r[1] && y < r[1] + r[3])
    }

    /// Index of the active document tab.
    pub(crate) fn active_tab(&self) -> usize {
        self.active
    }

    /// The view of document tab `i`.
    pub(crate) fn tab_view_mut(&mut self, i: usize) -> Option<&mut ReaderView> {
        self.tabs.get_mut(i).map(|t| &mut t.view)
    }

    /// Number of document tabs and the name shown on tab `i`.
    pub(crate) fn tab_count(&self) -> usize {
        self.tabs.len()
    }

    /// Name shown on document tab `i`.
    pub(crate) fn doc_tab_name(&self, i: usize) -> Option<&str> {
        self.tabs.get(i).map(|t| t.name.as_str())
    }

    /// Show document tab `i`.
    pub(crate) fn select_tab(&mut self, i: usize) {
        if i < self.tabs.len() {
            self.active = i;
            self.show_start = false;
            self.layout_doc();
        }
    }
}

impl ReaderWindow {
    /// Paint the open popup menu again, over what a host drew after [`ReaderWindow::paint`]
    /// (BlueEditor's Organize page grid, round 8).
    pub(crate) fn paint_open_menu(&self, p: &mut dyn Painter) {
        if let Some((menu, _)) = &self.menu {
            menu.paint(p, &self.theme, &self.icons);
        }
    }
}
