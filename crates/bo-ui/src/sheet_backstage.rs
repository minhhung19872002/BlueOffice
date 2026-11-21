//! BlueSheet's File screen (parity round 13, `docs/ui/REPORT-sheet.md` rounds 63+), built from
//! BlueDoc's File screen (same navigation column and page layout, which the reference
//! spreadsheet shares with the reference word processor; measured at 125 %, positions below
//! are logical pixels: reference pixels / 1.25):
//!
//! * Home: a greeting, the New row (blank workbook and BlueOffice's own workbook templates)
//!   and the app's own recent workbooks;
//! * New: the blank workbook (a small grid as the reference draws it) and the template gallery;
//! * Open: places (Recent, Browse) and the recent list kept by BlueSheet itself;
//! * Info: path, Copy path / Open file location, Protect / Inspect / Manage Workbook tiles with
//!   menus, the properties column (size, title, tags, categories, dates, people);
//! * Save As: This PC (current folder, name, type) and Browse;
//! * Print: Print button with copies, printer and the spreadsheet settings (Print Active
//!   Sheets / Entire Workbook / Selection, Pages from–to, sides, collation, orientation, paper,
//!   margins, scaling), a live preview of the printed pages with page navigation, Show
//!   Margins and Zoom to Page;
//! * Export: Create PDF/XPS Document and Change File Type.
//!
//! The nav column keeps BlueSheet's own accent colour (the reference's brand colour is not
//! copied); sizes, greys and the layout follow the reference.
use crate::strings::Strings;
use crate::theme::Theme;
use crate::{Font, Painter, TextMeasure, TextStyle};
use bo_render::{PathSeg, Rgba};

/// Pages of the File screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BsPage {
    Home,
    New,
    Open,
    Info,
    SaveAs,
    Print,
    Share,
    Export,
}

/// Navigation rows: (entry, label key, fallback).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Nav {
    Page(BsPage),
    Save,
    /// Version history: greyed for local files, as the reference shows it.
    History,
    Close,
    Options,
}

const NAV_TOP: [(Nav, &str, &str); 3] = [
    (Nav::Page(BsPage::Home), "sheetBs.home", "Home"),
    (Nav::Page(BsPage::New), "backstage.nav.new", "New"),
    (Nav::Page(BsPage::Open), "backstage.nav.open", "Open"),
];
const NAV_MID: [(Nav, &str, &str); 8] = [
    (Nav::Page(BsPage::Info), "backstage.nav.info", "Info"),
    (Nav::Save, "backstage.nav.save", "Save"),
    (Nav::Page(BsPage::SaveAs), "backstage.nav.saveAs", "Save As"),
    (Nav::History, "sheetBs.history", "History"),
    (Nav::Page(BsPage::Print), "backstage.nav.print", "Print"),
    (Nav::Page(BsPage::Share), "sheetBs.share", "Share"),
    (Nav::Page(BsPage::Export), "sheetBs.export", "Export"),
    (Nav::Close, "backstage.nav.close", "Close"),
];

/// BlueOffice's own document templates: (id, label key, fallback).
pub(crate) const TEMPLATES: [(&str, &str, &str); 5] = [
    ("budget", "sheetBs.tpl.budget", "Monthly budget"),
    ("invoice", "sheetBs.tpl.invoice", "Invoice"),
    ("schedule", "sheetBs.tpl.schedule", "Weekly schedule"),
    ("todo", "sheetBs.tpl.todo", "To-do list"),
    ("expenses", "sheetBs.tpl.expenses", "Expense report"),
];

/// File types of Export › Change File Type: (extension, label key, fallback).
pub(crate) const FILE_TYPES: [(&str, &str, &str); 7] = [
    ("xlsx", "sheetBs.type.xlsx", "Workbook (*.xlsx)"),
    ("csv", "sheetBs.type.csv", "CSV (Comma delimited) (*.csv)"),
    (
        "ods",
        "sheetBs.type.ods",
        "OpenDocument Spreadsheet (*.ods)",
    ),
    ("pdf", "sheetBs.type.pdf", "PDF (*.pdf)"),
    ("html", "sheetBs.type.html", "Web Page (*.html)"),
    ("mht", "sheetBs.type.mht", "Single File Web Page (*.mht)"),
    (
        "prn",
        "sheetBs.type.prn",
        "Formatted Text (Space delimited) (*.prn)",
    ),
];

/// A document in the app's recent list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Recent {
    /// Full path.
    pub(crate) path: String,
    /// Last opened, shown as given (host's local date).
    pub(crate) when: String,
}

impl Recent {
    fn name(&self) -> &str {
        self.path.rsplit(['\\', '/']).next().unwrap_or(&self.path)
    }

    fn folder(&self) -> &str {
        match self.path.rfind(['\\', '/']) {
            Some(i) => &self.path[..i],
            None => "",
        }
    }
}

/// What the File screen asks of the window.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum BsOut {
    None,
    Redraw,
    /// Leave the File screen.
    Back,
    Save,
    Close,
    Options,
    /// Pick a file to open (system dialog).
    BrowseOpen,
    /// Open this path in a new window.
    OpenPath(String),
    /// New document: blank (`None`) or from a template id.
    New(Option<&'static str>),
    /// Save under a new name as this type (system dialog).
    SaveAsType(&'static str),
    ExportPdf,
    CopyPath,
    OpenLocation,
    /// Print with the page's settings.
    Print,
    /// The chosen printer's driver settings (Printer Properties).
    PrinterProperties,
    /// File › Print › Page Setup.
    PageSetup,
    /// Convert a compatibility-mode document to the current format.
    Convert,
    /// Share › Email: a new message about the document in the mail program.
    Email,
    /// Open drop-down `id` under `anchor` (device pixels).
    Menu(&'static str, [f32; 4]),
}

/// Print settings of the Print page.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PrintSettings {
    pub(crate) copies: u32,
    /// Installed printers and the chosen one.
    pub(crate) printers: Vec<String>,
    pub(crate) printer: usize,
    /// What prints: 0 the active sheet(s), 1 the entire workbook, 2 the selection.
    pub(crate) range: u8,
    /// Pages: first page ("" from the first).
    pub(crate) pages: String,
    /// Pages: last page ("" to the last).
    pub(crate) pages_to: String,
    /// 0 one sided, 1 both sides (long edge), 2 both sides (short edge).
    pub(crate) sides: u8,
    pub(crate) collated: bool,
    /// Scaling: 0 none, 1 fit sheet on one page, 2 fit all columns on one page, 3 fit all
    /// rows on one page, 4 a custom scale or fit (§18.3.1.63 pageSetup/@scale, @fitToWidth,
    /// @fitToHeight with §18.3.1.65 pageSetUpPr/@fitToPage).
    pub(crate) scaling: u8,
    /// Spooler state of each printer: (error, offline, jobs waiting).
    pub(crate) states: Vec<(bool, bool, u32)>,
    /// The chosen printer prints on both sides (the reference shows the sides box only then).
    pub(crate) duplex: bool,
}

impl Default for PrintSettings {
    fn default() -> Self {
        Self {
            copies: 1,
            printers: Vec::new(),
            printer: 0,
            range: 0,
            pages: String::new(),
            pages_to: String::new(),
            sides: 0,
            collated: true,
            scaling: 0,
            states: Vec::new(),
            duplex: true,
        }
    }
}

/// Facts the pages show about the open workbook.
#[derive(Debug, Clone, Default)]
pub(crate) struct BookFacts {
    pub(crate) name: String,
    pub(crate) path: Option<String>,
    pub(crate) bytes: Option<u64>,
    /// Printed pages of the preview (with the Print page's settings).
    pub(crate) pages: usize,
    /// `docProps/core.xml` category (Info › Categories).
    pub(crate) categories: String,
    pub(crate) modified: bool,
    pub(crate) title: String,
    pub(crate) tags: String,
    pub(crate) author: String,
    pub(crate) last_modified_by: String,
    pub(crate) created: String,
    pub(crate) last_modified: String,
    /// `docProps/app.xml` TotalTime (minutes).
    pub(crate) editing_minutes: Option<u64>,
    /// The workbook structure is protected (Protect Workbook).
    pub(crate) protected: bool,
    /// Orientation (landscape), paper (name, width × height in inches), margins name of the
    /// active sheet's page setup, for the Print settings.
    pub(crate) landscape: bool,
    pub(crate) paper: (String, f32, f32),
    pub(crate) margins: String,
    /// Page sizes in points (preview shape).
    pub(crate) page_sizes: Vec<(f32, f32)>,
    /// The file is in compatibility mode (not used by BlueSheet: always false).
    pub(crate) compat: bool,
    /// More properties for Show All Properties: (label key, fallback, value).
    pub(crate) more: Vec<(&'static str, &'static str, String)>,
}

/// Hot regions of the shown page.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Hit {
    Back,
    Nav(Nav),
    Template(Option<usize>),
    RecentRow(usize),
    Place(u8),
    InfoButton(u8),
    Tile(u8),
    ExportItem(u8),
    TypeRow(usize),
    Action(u8),
    PrintSetting(u8),
    CopiesUp,
    CopiesDown,
    PreviewPrev,
    PreviewNext,
    PagesBox,
    /// The second Pages box ("to").
    PagesTo,
    Link(u8),
    /// Show Margins.
    ZoomOut,
    /// Zoom to Page.
    ZoomFit,
}

/// The open File screen.
pub(crate) struct SheetBackstage {
    pub(crate) page: BsPage,
    top: f32,
    size: (f32, f32),
    scale: f32,
    hover: Option<Hit>,
    pub(crate) facts: BookFacts,
    pub(crate) recent: Vec<Recent>,
    pub(crate) print: PrintSettings,
    /// Page shown in the print preview.
    pub(crate) preview: usize,
    /// Export: 0 PDF/XPS, 1 Change File Type; chosen type.
    pub(crate) export: u8,
    pub(crate) file_type: usize,
    /// Open / Save As: chosen place (0 Recent / This PC, 1 Browse).
    place: u8,
    /// Typing in the first Pages box of the Print page.
    pub(crate) pages_focus: bool,
    /// Typing in the second Pages box ("to").
    pub(crate) to_focus: bool,
    /// Measured widths of the Info page's buttons and of the Save label.
    button_w: [f32; 3],
    /// Print preview zoom (1 = the whole page fitted).
    pub(crate) zoom: f32,
    /// Show Margins: the page's margin guides on the preview.
    pub(crate) show_margins: bool,
    /// Show All Properties.
    pub(crate) all_props: bool,

    hits: Vec<(Hit, [f32; 4])>,
}

/// Navigation column width.
const NAV_W: f32 = 179.0;
/// Navigation row height.
const ROW_H: f32 = 44.0;
/// Content background (#F3F3F3, measured).
const CONTENT_BG: Rgba = Rgba(243, 243, 243, 255);
/// Body text on the content area (#262626).
const INK: Rgba = Rgba(38, 38, 38, 255);
/// Secondary text and placeholders.
const MUTED: Rgba = Rgba(104, 104, 104, 255);
const PLACEHOLDER: Rgba = Rgba(128, 128, 128, 255);
/// Frames of tiles and buttons.
const FRAME: Rgba = Rgba(171, 171, 171, 255);
const WHITE: Rgba = Rgba(255, 255, 255, 255);
/// Hover fill on the content area.
const HOVER: Rgba = Rgba(218, 218, 218, 255);
/// The chosen item on the content area (Export list, places).
const CHOSEN: Rgba = Rgba(198, 198, 198, 255);

impl SheetBackstage {
    pub(crate) fn new(page: BsPage, facts: BookFacts) -> Self {
        Self {
            page,
            top: 0.0,
            size: (0.0, 0.0),
            scale: 1.0,
            hover: None,
            facts,
            recent: Vec::new(),
            print: PrintSettings::default(),
            preview: 0,
            export: 0,
            file_type: 0,
            place: 0,
            pages_focus: false,
            to_focus: false,
            button_w: [0.0; 3],
            zoom: 1.0,
            show_margins: false,
            all_props: false,
            hits: Vec::new(),
        }
    }

    fn px(&self, v: f32) -> f32 {
        (v * self.scale).round()
    }

    /// Place below the title bar (`top`) in a window of `size`.
    pub(crate) fn layout(&mut self, top: f32, size: (f32, f32), scale: f32) {
        (self.top, self.size, self.scale) = (top, size, scale);
        self.hits = self.regions();
    }

    /// Measure the labels whose width the layout needs.
    pub(crate) fn measure(&mut self, m: &dyn TextMeasure, strings: &Strings) {
        let style = self.body();
        for (i, (_, k, f)) in INFO_BUTTONS.iter().enumerate() {
            self.button_w[i] = m.measure(&tr(strings, k, f), style).0;
        }
        self.button_w[2] = m
            .measure(&tr(strings, "backstage.nav.save", "Save"), style)
            .0;
        self.hits = self.regions();
    }

    fn width_of(&self, i: usize, s: &str) -> f32 {
        match self.button_w.get(i) {
            Some(w) if *w > 0.0 => *w,
            _ => s.chars().count() as f32 * 7.5 * self.scale,
        }
    }

    /// Text styles (logical sizes measured on the reference at 125 %).
    fn body(&self) -> TextStyle {
        TextStyle {
            size: self.px(13.0),
            bold: false,
        }
    }

    fn nav_style(&self) -> TextStyle {
        TextStyle {
            size: self.px(13.5),
            bold: false,
        }
    }

    fn content_x(&self) -> f32 {
        self.px(NAV_W + 42.0)
    }

    /// Top of navigation row `i` of the upper block (0 Home…) and the middle block.
    fn nav_rows(&self) -> Vec<(Nav, [f32; 4], &'static str, &'static str)> {
        let w = self.px(NAV_W);
        let mut out = Vec::new();
        let base = self.top.max(self.px(48.0));
        let mut y = base + self.px(44.0);
        for (n, k, f) in NAV_TOP {
            out.push((n, [0.0, y, w, self.px(ROW_H)], k, f));
            y += self.px(ROW_H);
        }
        y += self.px(16.0);
        for (n, k, f) in NAV_MID {
            out.push((n, [0.0, y, w, self.px(ROW_H)], k, f));
            y += self.px(ROW_H);
        }
        let oy = self.size.1 - self.px(23.0 + ROW_H);
        out.push((
            Nav::Options,
            [0.0, oy, w, self.px(ROW_H)],
            "sheetBs.options",
            "Options",
        ));
        out
    }

    fn back_rect(&self) -> [f32; 4] {
        let base = self.top.max(self.px(48.0));
        [0.0, base - self.px(4.0), self.px(NAV_W), self.px(46.0)]
    }

    fn regions(&self) -> Vec<(Hit, [f32; 4])> {
        let mut r = vec![(Hit::Back, self.back_rect())];
        for (n, rect, _, _) in self.nav_rows() {
            r.push((Hit::Nav(n), rect));
        }
        let cx = self.content_x();
        let (w, h) = self.size;
        match self.page {
            BsPage::Home => {
                let y = self.top + self.px(118.0);
                for i in 0..4 {
                    r.push((
                        Hit::Template(if i == 0 { None } else { Some(i - 1) }),
                        [
                            cx + self.px(160.0) * i as f32,
                            y,
                            self.px(134.0),
                            self.px(196.0),
                        ],
                    ));
                }
                let ly = y + self.px(270.0);
                for i in 0..self.recent.len().min(8) {
                    r.push((
                        Hit::RecentRow(i),
                        [
                            cx,
                            ly + self.px(36.0) * i as f32,
                            (w - cx - self.px(60.0)).max(1.0),
                            self.px(36.0),
                        ],
                    ));
                }
            }
            BsPage::New => {
                let y0 = self.top + self.px(88.0);
                r.push((
                    Hit::Template(None),
                    [cx + self.px(36.0), y0, self.px(189.0), self.px(270.0)],
                ));
                let y1 = y0 + self.px(330.0);
                let step = self.px(265.0);
                let per_row = (((w - cx - self.px(36.0)) / step).floor() as usize).max(1);
                for i in 0..TEMPLATES.len() {
                    let (row, col) = (i / per_row, i % per_row);
                    r.push((
                        Hit::Template(Some(i)),
                        [
                            cx + self.px(36.0) + step * col as f32,
                            y1 + self.px(310.0) * row as f32,
                            self.px(189.0),
                            self.px(270.0),
                        ],
                    ));
                }
            }
            BsPage::Open | BsPage::SaveAs => {
                let y = self.top + self.px(100.0);
                for i in 0..2_u8 {
                    r.push((
                        Hit::Place(i),
                        [
                            cx,
                            y + self.px(64.0) * f32::from(i),
                            self.px(300.0),
                            self.px(60.0),
                        ],
                    ));
                }
                let lx = cx + self.px(330.0);
                if self.page == BsPage::Open {
                    let ly = y + self.px(44.0);
                    for i in 0..self.recent.len().min(14) {
                        r.push((
                            Hit::RecentRow(i),
                            [
                                lx,
                                ly + self.px(48.0) * i as f32,
                                (w - lx - self.px(40.0)).max(1.0),
                                self.px(48.0),
                            ],
                        ));
                    }
                } else {
                    let ly = y + self.px(124.0);
                    r.push((Hit::Action(0), [lx, ly, self.px(360.0), self.px(28.0)]));
                    r.push((
                        Hit::Action(1),
                        [lx + self.px(372.0), ly, self.px(84.0), self.px(28.0)],
                    ));
                }
            }
            BsPage::Info => {
                let y = self.top + self.px(162.0);
                let mut x = cx + self.px(3.0);
                for (i, (_, k, f)) in INFO_BUTTONS.iter().enumerate() {
                    let _ = k;
                    let bw = self.width_of(i, f) + self.px(46.0);
                    r.push((Hit::InfoButton(i as u8), [x, y, bw, self.px(30.0)]));
                    x += bw + self.px(12.0);
                }
                for (i, ty) in self.tile_tops().into_iter().enumerate() {
                    r.push((
                        Hit::Tile(i as u8),
                        [cx + self.px(4.0), ty, self.px(104.0), self.px(101.0)],
                    ));
                }
                if self.facts.compat {
                    r.push((Hit::Action(9), self.convert_tile()));
                }
                r.push((Hit::Link(0), self.show_all_rect()));
            }
            BsPage::Share => {
                let y = self.top + self.px(84.0);
                r.push((Hit::ExportItem(0), [cx, y, self.px(375.0), self.px(62.0)]));
                r.push((
                    Hit::Action(0),
                    [
                        cx + self.px(410.0),
                        y + self.px(150.0),
                        self.px(101.0),
                        self.px(101.0),
                    ],
                ));
            }
            BsPage::Print => {
                let y = self.top + self.px(80.0);
                r.push((
                    Hit::Action(0),
                    [cx + self.px(3.0), y, self.px(101.0), self.px(101.0)],
                ));
                let sx = cx + self.px(178.0);
                r.push((
                    Hit::CopiesUp,
                    [sx + self.px(56.0), y, self.px(20.0), self.px(15.0)],
                ));
                r.push((
                    Hit::CopiesDown,
                    [
                        sx + self.px(56.0),
                        y + self.px(15.0),
                        self.px(20.0),
                        self.px(15.0),
                    ],
                ));
                for (i, ry) in self.shown_settings().into_iter().zip(self.setting_tops()) {
                    r.push((
                        Hit::PrintSetting(i),
                        [cx + self.px(4.0), ry, self.px(293.0), self.px(51.0)],
                    ));
                }
                r.push((Hit::PagesBox, self.pages_box()));
                r.push((Hit::PagesTo, self.pages_to_box()));
                for (i, l) in self.print_links().into_iter().enumerate() {
                    r.push((Hit::Link(1 + i as u8), l));
                }
                let [margins, fit] = self.corner_rects();
                r.push((Hit::ZoomOut, margins));
                r.push((Hit::ZoomFit, fit));
                let [px0, _, pw, _] = self.preview_area();
                let ny = h - self.px(34.0);
                r.push((Hit::PreviewPrev, [px0, ny, self.px(20.0), self.px(24.0)]));
                r.push((
                    Hit::PreviewNext,
                    [px0 + self.px(116.0), ny, self.px(20.0), self.px(24.0)],
                ));
                let _ = pw;
            }
            BsPage::Export => {
                let y = self.top + self.px(84.0);
                for i in 0..2_u8 {
                    r.push((
                        Hit::ExportItem(i),
                        [
                            cx,
                            y + self.px(66.0) * f32::from(i),
                            self.px(375.0),
                            self.px(62.0),
                        ],
                    ));
                }
                let rx = cx + self.px(410.0);
                if self.export == 0 {
                    r.push((
                        Hit::Action(0),
                        [rx, y + self.px(150.0), self.px(101.0), self.px(101.0)],
                    ));
                } else {
                    let ty = y + self.px(70.0);
                    for i in 0..FILE_TYPES.len() {
                        r.push((
                            Hit::TypeRow(i),
                            [
                                rx,
                                ty + self.px(46.0) * i as f32,
                                self.px(420.0),
                                self.px(44.0),
                            ],
                        ));
                    }
                    let by = ty + self.px(46.0) * FILE_TYPES.len() as f32 + self.px(16.0);
                    r.push((Hit::Action(1), [rx, by, self.px(101.0), self.px(101.0)]));
                }
            }
        }
        r
    }

    fn tile_tops(&self) -> [f32; 3] {
        // The Compatibility Mode block comes first when shown (measured: 142 px logical).
        let y = self.top + self.px(if self.facts.compat { 364.0 } else { 222.0 });
        [y, y + self.px(134.0), y + self.px(287.0)]
    }

    /// The yellow Compatibility Mode block and its Convert tile.
    fn compat_box(&self) -> [f32; 4] {
        [
            self.content_x(),
            self.top + self.px(206.0),
            self.px(590.0),
            self.px(113.0),
        ]
    }

    fn convert_tile(&self) -> [f32; 4] {
        let b = self.compat_box();
        [
            b[0] + self.px(5.0),
            b[1] + self.px(6.0),
            self.px(104.0),
            self.px(101.0),
        ]
    }

    /// Rows of the properties column (count) for the Show All Properties link.
    fn show_all_rect(&self) -> [f32; 4] {
        let x = (self.size.0 - self.px(322.0)).max(self.content_x() + self.px(640.0));
        let rows = 4 + if self.all_props {
            self.facts.more.len()
        } else {
            0
        };
        let mut y = self.top + self.px(222.0) + self.px(31.0) + self.px(29.0) * rows as f32;
        y += self.px(16.0 + 31.0 + 29.0 * 3.0 + 16.0 + 31.0 + 29.0 * 2.0);
        if self.facts.path.is_some() {
            y += self.px(16.0 + 34.0 + 34.0);
        }
        [x, y + self.px(8.0), self.px(180.0), self.px(24.0)]
    }

    /// Printer Properties (under the printer box), Find more print settings and Page Setup
    /// (under the settings, right-aligned as the reference).
    fn print_links(&self) -> [[f32; 4]; 3] {
        let cx = self.content_x();
        let tops = self.setting_tops();
        let last = tops.last().copied().unwrap_or(0.0);
        [
            [
                cx + self.px(182.0),
                tops[0] + self.px(56.0),
                self.px(115.0),
                self.px(20.0),
            ],
            [
                cx + self.px(220.0),
                last + self.px(84.0),
                self.px(77.0),
                self.px(20.0),
            ],
            [
                cx + self.px(143.0),
                last + self.px(58.0),
                self.px(154.0),
                self.px(20.0),
            ],
        ]
    }

    /// Show Margins and Zoom to Page at the preview's bottom right (measured: 26 × 26 px
    /// logical buttons, 27 px apart, 6 px from the area's right edge).
    fn corner_rects(&self) -> [[f32; 4]; 2] {
        let a = self.preview_area();
        let ny = self.size.1 - self.px(34.0);
        let fit = [
            a[0] + a[2] - self.px(26.0),
            ny,
            self.px(26.0),
            self.px(26.0),
        ];
        let margins = [fit[0] - self.px(27.0), ny, self.px(26.0), self.px(26.0)];
        [margins, fit]
    }

    /// The settings boxes shown: printer, range, sides (when the printer prints on both
    /// sides), collated, orientation, paper, margins, scaling.
    fn shown_settings(&self) -> Vec<u8> {
        (0..8u8).filter(|i| *i != 2 || self.print.duplex).collect()
    }

    fn setting_tops(&self) -> Vec<f32> {
        let y = self.top + self.px(262.0);
        // Printer, then Settings: range, (Pages box), sides, collated, orientation, paper,
        // margins, pages per sheet.
        let mut v = vec![y];
        let s = y + self.px(122.0);
        v.push(s);
        let mut yy = s + self.px(51.0) + self.px(38.0);
        for _ in 2..self.shown_settings().len() {
            v.push(yy);
            yy += self.px(55.0);
        }
        v
    }

    /// Pages: the "from" spin box (measured: 77 × 30 px logical, 54 px right of the
    /// content's left edge); the "to" box 113 px further right.
    fn pages_box(&self) -> [f32; 4] {
        let cx = self.content_x();
        let y = self.top + self.px(262.0 + 122.0 + 51.0 + 5.0);
        [cx + self.px(54.0), y, self.px(77.0), self.px(30.0)]
    }

    fn pages_to_box(&self) -> [f32; 4] {
        let b = self.pages_box();
        [b[0] + self.px(113.0), b[1], b[2], b[3]]
    }

    /// The print preview's area (device pixels).
    pub(crate) fn preview_area(&self) -> [f32; 4] {
        let cx = self.content_x();
        let x = cx + self.px(372.0);
        let y = self.top + self.px(80.0);
        let (w, h) = self.size;
        [
            x,
            y,
            (w - x - self.px(24.0)).max(10.0),
            (h - y - self.px(48.0)).max(10.0),
        ]
    }

    /// Where the preview page goes: (page index, x, y, width, height), fitted in the area.
    pub(crate) fn preview_rect(&self) -> Option<(usize, [f32; 4])> {
        if self.page != BsPage::Print {
            return None;
        }
        let n = self.facts.page_sizes.len();
        if n == 0 {
            return None;
        }
        let i = self.preview.min(n - 1);
        let (pw, ph) = self.facts.page_sizes[i];
        let [x, y, w, h] = self.preview_area();
        let pad = self.px(32.0);
        let k = ((w - 2.0 * pad) / pw.max(1.0)).min((h - 2.0 * pad) / ph.max(1.0)) * self.zoom;
        let (dw, dh) = ((pw * k).round(), (ph * k).round());
        // Larger than the area: from its top-left corner (clipped by the painter).
        let px = if dw > w { x + pad } else { x + (w - dw) / 2.0 };
        let py = if dh > h { y + pad } else { y + (h - dh) / 2.0 };
        Some((i, [px.round(), py.round(), dw, dh]))
    }

    fn hit(&self, x: f32, y: f32) -> Option<Hit> {
        self.hits
            .iter()
            .rev()
            .find(|(_, r)| x >= r[0] && x < r[0] + r[2] && y >= r[1] && y < r[1] + r[3])
            .map(|(h, _)| *h)
    }

    /// The page's main button (Print on the Print page; tests).
    #[cfg(test)]
    pub(crate) fn rect_of_action(&self) -> Option<[f32; 4]> {
        self.rect_of(Hit::Action(0))
    }

    fn rect_of(&self, h: Hit) -> Option<[f32; 4]> {
        self.hits.iter().find(|(x, _)| *x == h).map(|(_, r)| *r)
    }

    /// Pointer moved: `true` when the highlight changed.
    pub(crate) fn hover(&mut self, x: f32, y: f32) -> bool {
        let h = self.hit(x, y);
        std::mem::replace(&mut self.hover, h) != h
    }

    /// Click.
    pub(crate) fn press(&mut self, x: f32, y: f32) -> BsOut {
        let Some(h) = self.hit(x, y) else {
            if self.pages_focus || self.to_focus {
                self.pages_focus = false;
                self.to_focus = false;
                return BsOut::Redraw;
            }
            return BsOut::None;
        };
        self.pages_focus = h == Hit::PagesBox;
        self.to_focus = h == Hit::PagesTo;
        match h {
            Hit::Back => BsOut::Back,
            Hit::Nav(Nav::Page(p)) => {
                self.show(p);
                BsOut::Redraw
            }
            Hit::Nav(Nav::Save) => BsOut::Save,
            Hit::Nav(Nav::History) => BsOut::None,
            Hit::Link(0) => {
                self.all_props = !self.all_props;
                self.hits = self.regions();
                BsOut::Redraw
            }
            Hit::Link(1) => BsOut::PrinterProperties,
            Hit::Link(_) => BsOut::PageSetup,
            Hit::ZoomOut => {
                // Show Margins.
                self.show_margins = !self.show_margins;
                BsOut::Redraw
            }
            Hit::ZoomFit => {
                // Zoom to Page: the page at its actual size, or fitted again.
                self.zoom = if self.zoom > 1.0 { 1.0 } else { 2.0 };
                BsOut::Redraw
            }
            Hit::Nav(Nav::Close) => BsOut::Close,
            Hit::Nav(Nav::Options) => BsOut::Options,
            Hit::Template(t) => BsOut::New(t.and_then(|i| TEMPLATES.get(i)).map(|t| t.0)),
            Hit::RecentRow(i) => self
                .recent
                .get(i)
                .map_or(BsOut::None, |r| BsOut::OpenPath(r.path.clone())),
            Hit::Place(1) => {
                if self.page == BsPage::Open {
                    BsOut::BrowseOpen
                } else {
                    BsOut::SaveAsType("xlsx")
                }
            }
            Hit::Place(_) => {
                self.place = 0;
                BsOut::Redraw
            }
            Hit::InfoButton(0) => BsOut::CopyPath,
            Hit::InfoButton(_) => BsOut::OpenLocation,
            Hit::Tile(i) => {
                let id = ["bs13.protect", "bs13.inspect", "bs13.manage"][usize::from(i.min(2))];
                BsOut::Menu(id, self.rect_of(h).unwrap_or_default())
            }
            Hit::ExportItem(_) if self.page == BsPage::Share => BsOut::None,
            Hit::ExportItem(i) => {
                self.export = i;
                self.hits = self.regions();
                BsOut::Redraw
            }
            Hit::TypeRow(i) => {
                self.file_type = i;
                BsOut::Redraw
            }
            Hit::Action(a) => match (self.page, a) {
                (BsPage::Info, 9) => BsOut::Convert,
                (BsPage::Share, _) => BsOut::Email,
                (BsPage::Print, _) => BsOut::Print,
                (BsPage::Export, 0) => BsOut::ExportPdf,
                (BsPage::Export, _) | (BsPage::SaveAs, 1) => {
                    BsOut::SaveAsType(FILE_TYPES[self.file_type].0)
                }
                (BsPage::SaveAs, _) => {
                    BsOut::Menu("bs13.saveType", self.rect_of(h).unwrap_or_default())
                }
                _ => BsOut::None,
            },
            Hit::PrintSetting(i) => {
                let id = [
                    "bs13.printer",
                    "bs13.range",
                    "bs13.sides",
                    "bs13.collate",
                    "bs13.orientation",
                    "bs13.paper",
                    "bs13.margins",
                    "bs13.scaling",
                ][usize::from(i.min(7))];
                BsOut::Menu(id, self.rect_of(h).unwrap_or_default())
            }
            Hit::CopiesUp => {
                self.print.copies = (self.print.copies + 1).min(32767);
                BsOut::Redraw
            }
            Hit::CopiesDown => {
                self.print.copies = self.print.copies.saturating_sub(1).max(1);
                BsOut::Redraw
            }
            Hit::PreviewPrev => {
                self.preview = self.preview.saturating_sub(1);
                BsOut::Redraw
            }
            Hit::PreviewNext => {
                if self.preview + 1 < self.facts.pages.max(self.facts.page_sizes.len()) {
                    self.preview += 1;
                }
                BsOut::Redraw
            }
            Hit::PagesBox | Hit::PagesTo => BsOut::Redraw,
        }
    }

    /// Typing (the Pages boxes of the Print page take digits).
    pub(crate) fn type_text(&mut self, s: &str) -> bool {
        let field = if self.pages_focus {
            &mut self.print.pages
        } else if self.to_focus {
            &mut self.print.pages_to
        } else {
            return false;
        };
        let before = field.len();
        field.extend(s.chars().filter(char::is_ascii_digit).take(5));
        field.len() != before
    }

    /// Backspace in the Pages box; Page Up / Down turn the preview.
    pub(crate) fn key(&mut self, key: crate::Key) -> bool {
        match key {
            crate::Key::Backspace if self.pages_focus => self.print.pages.pop().is_some(),
            crate::Key::Backspace if self.to_focus => self.print.pages_to.pop().is_some(),
            crate::Key::PageDown | crate::Key::Right if self.page == BsPage::Print => {
                let n = self.facts.page_sizes.len();
                if self.preview + 1 < n {
                    self.preview += 1;
                    return true;
                }
                false
            }
            crate::Key::PageUp | crate::Key::Left if self.page == BsPage::Print => {
                let before = self.preview;
                self.preview = self.preview.saturating_sub(1);
                before != self.preview
            }
            _ => false,
        }
    }

    /// Wheel over the preview turns its pages.
    pub(crate) fn wheel(&mut self, dy: f32) -> bool {
        if self.page != BsPage::Print {
            return false;
        }
        self.key(if dy > 0.0 {
            crate::Key::PageDown
        } else {
            crate::Key::PageUp
        })
    }

    /// Show page `p`.
    pub(crate) fn show(&mut self, p: BsPage) {
        self.page = p;
        self.place = 0;
        self.pages_focus = false;
        self.to_focus = false;
        self.hits = self.regions();
    }

    /// Pages the Print page prints (0-based): Pages from–to of what prints (empty boxes are
    /// the first and the last page; a reversed range prints nothing, as on the reference).
    pub(crate) fn pages_to_print(&self) -> Vec<usize> {
        let n = self.facts.page_sizes.len().max(self.facts.pages);
        let from = self.print.pages.parse::<usize>().ok().unwrap_or(1).max(1);
        let to = self
            .print
            .pages_to
            .parse::<usize>()
            .ok()
            .unwrap_or(n)
            .min(n);
        (from..=to).map(|p| p - 1).collect()
    }

    // ---------------------------------------------------------------- painting

    #[allow(clippy::too_many_lines)]
    pub(crate) fn paint(&self, p: &mut dyn Painter, t: &Theme, s: &Strings) {
        let (w, h) = self.size;
        p.fill_rect(0.0, self.top, w, h - self.top, CONTENT_BG);
        self.paint_nav(p, t, s);
        let cx = self.content_x();
        let title = match self.page {
            BsPage::Home => greeting(s),
            BsPage::New => tr(s, "backstage.nav.new", "New"),
            BsPage::Open => tr(s, "backstage.nav.open", "Open"),
            BsPage::Info => tr(s, "backstage.nav.info", "Info"),
            BsPage::SaveAs => tr(s, "backstage.nav.saveAs", "Save As"),
            BsPage::Print => tr(s, "backstage.nav.print", "Print"),
            BsPage::Export => tr(s, "sheetBs.export", "Export"),
            BsPage::Share => tr(s, "sheetBs.share", "Share"),
        };
        let light = Font {
            family: "Segoe UI Light",
            size: self.px(48.0),
            weight: 300,
            italic: false,
            underline: false,
        };
        // Measured on the reference at 125 %: the title's capitals 42 px high, their top
        // 81 px below the window's top, 10 px left of the content.
        p.text_font(
            &title,
            cx - self.px(8.0),
            self.top + self.px(8.0),
            &light,
            INK,
        );
        match self.page {
            BsPage::Home => self.paint_home(p, t, s),
            BsPage::New => self.paint_new(p, t, s),
            BsPage::Open | BsPage::SaveAs => self.paint_places(p, t, s),
            BsPage::Info => self.paint_info(p, t, s),
            BsPage::Print => self.paint_print(p, t, s),
            BsPage::Export => self.paint_export(p, t, s),
            BsPage::Share => self.paint_share(p, t, s),
        }
    }

    fn paint_nav(&self, p: &mut dyn Painter, t: &Theme, s: &Strings) {
        let (_, h) = self.size;
        let nw = self.px(NAV_W);
        p.fill_rect(
            0.0,
            self.top,
            nw,
            h - self.top,
            rail_colour(t.accent_strong),
        );
        let on = t.accent_on;
        // Back: an arrow in a ring.
        let br = self.back_rect();
        if self.hover == Some(Hit::Back) {
            p.fill_rect(br[0], br[1], br[2], br[3], NAV_HOVER);
        }
        let (cx, cy, r) = (self.px(34.0), br[1] + br[3] / 2.0, self.px(12.5));
        let lw = self.scale.max(1.0) * 1.25;
        p.path(&circle(cx, cy, r), None, Some((on, lw)));
        let k = self.px(6.5);
        p.path(
            &[
                PathSeg::M(cx + k, cy),
                PathSeg::L(cx - k, cy),
                PathSeg::M(cx - k / 2.0 - 0.5, cy - k / 2.0 - 1.0),
                PathSeg::L(cx - k, cy),
                PathSeg::L(cx - k / 2.0 - 0.5, cy + k / 2.0 + 1.0),
            ],
            None,
            Some((on, lw)),
        );
        let style = self.nav_style();
        let rows = self.nav_rows();
        for (n, rect, key, fb) in &rows {
            let chosen = matches!(n, Nav::Page(pg) if *pg == self.page);
            if chosen {
                p.fill_rect(rect[0], rect[1], rect[2], rect[3], NAV_CHOSEN);
            } else if self.hover == Some(Hit::Nav(*n)) && *n != Nav::History {
                p.fill_rect(rect[0], rect[1], rect[2], rect[3], NAV_HOVER);
            }
            let ty = rect[1] + (rect[3] - style.size * 1.33) / 2.0;
            // History is greyed for local files, as on the reference.
            let ink = if *n == Nav::History {
                Rgba(on.0, on.1, on.2, 110)
            } else {
                on
            };
            p.text(&tr(s, key, fb), self.px(55.0), ty.round(), style, ink);
            let icon = match n {
                Nav::Page(BsPage::Home) => Some(NavIcon::Home),
                Nav::Page(BsPage::New) => Some(NavIcon::New),
                Nav::Page(BsPage::Open) => Some(NavIcon::Open),
                _ => None,
            };
            if let Some(i) = icon {
                nav_icon(
                    p,
                    i,
                    self.px(25.0),
                    rect[1] + rect[3] / 2.0,
                    self.px(18.0),
                    on,
                    lw,
                );
            }
        }
        // Separators: under Open and above Options (#7DA3C6 on the reference's column: white
        // at about 40 %).
        let sep = Rgba(255, 255, 255, 100);
        if let Some((_, r, _, _)) = rows.get(2) {
            p.fill_rect(
                self.px(27.0),
                (r[1] + r[3] + self.px(8.0)).round(),
                self.px(132.0),
                1.0_f32.max(self.scale.floor()),
                sep,
            );
        }
        if let Some((_, r, _, _)) = rows.last() {
            p.fill_rect(
                self.px(27.0),
                (r[1] - self.px(8.0)).round(),
                self.px(132.0),
                1.0_f32.max(self.scale.floor()),
                sep,
            );
        }
    }

    fn heading(&self, p: &mut dyn Painter, text: &str, x: f32, y: f32, size: f32, c: Rgba) {
        p.text(
            text,
            x,
            y,
            TextStyle {
                size: self.px(size),
                bold: false,
            },
            c,
        );
    }

    fn paint_home(&self, p: &mut dyn Painter, t: &Theme, s: &Strings) {
        let cx = self.content_x();
        let y = self.top + self.px(118.0);
        for i in 0..4 {
            let r = [
                cx + self.px(160.0) * i as f32,
                y,
                self.px(134.0),
                self.px(170.0),
            ];
            let hit = Hit::Template(if i == 0 { None } else { Some(i - 1) });
            if self.hover == Some(hit) {
                p.fill_rect(
                    r[0] - 4.0,
                    r[1] - 4.0,
                    r[2] + 8.0,
                    r[3] + self.px(34.0),
                    HOVER,
                );
            }
            self.paint_template_thumb(p, t, if i == 0 { None } else { Some(i - 1) }, r);
            let label = if i == 0 {
                tr(s, "sheetBs.blank", "Blank workbook")
            } else {
                let (_, k, f) = TEMPLATES[i - 1];
                tr(s, k, f)
            };
            self.heading(p, &label, r[0], r[1] + r[3] + self.px(6.0), 12.5, INK);
        }
        let ly = y + self.px(232.0);
        self.heading(
            p,
            &tr(s, "sheetBs.recent", "Recent"),
            cx,
            ly,
            15.0,
            t.accent_text,
        );
        p.fill_rect(
            cx,
            ly + self.px(30.0),
            self.px(56.0),
            self.px(2.0),
            t.accent_text,
        );
        self.paint_recent(p, s, cx, ly + self.px(38.0), 36.0, 8);
    }

    fn paint_recent(&self, p: &mut dyn Painter, s: &Strings, x: f32, y: f32, row: f32, max: usize) {
        let (w, _) = self.size;
        let body = self.body();
        if self.recent.is_empty() {
            p.text(
                &tr(
                    s,
                    "sheetBs.noRecent",
                    "You haven't opened any files recently.",
                ),
                x,
                y + self.px(8.0),
                body,
                MUTED,
            );
            return;
        }
        let small = TextStyle {
            size: self.px(11.5),
            bold: false,
        };
        for (i, r) in self.recent.iter().take(max).enumerate() {
            let ry = y + self.px(row) * i as f32;
            if self.hover == Some(Hit::RecentRow(i)) {
                p.fill_rect(x, ry, w - x - self.px(40.0), self.px(row), HOVER);
            }
            let ih = self.px(24.0);
            doc_icon(p, x + self.px(8.0), ry + (self.px(row) - ih) / 2.0, ih);
            let tx = x + self.px(44.0);
            if row > 40.0 {
                p.text(r.name(), tx, ry + self.px(6.0), body, INK);
                p.text(r.folder(), tx, ry + self.px(25.0), small, MUTED);
            } else {
                p.text(
                    r.name(),
                    tx,
                    ry + (self.px(row) - body.size * 1.33) / 2.0,
                    body,
                    INK,
                );
                p.text(
                    r.folder(),
                    tx + self.px(300.0),
                    ry + (self.px(row) - small.size * 1.33) / 2.0,
                    small,
                    MUTED,
                );
            }
            p.text(
                &r.when,
                w - self.px(200.0),
                ry + (self.px(row) - small.size * 1.33) / 2.0,
                small,
                MUTED,
            );
        }
    }

    fn paint_new(&self, p: &mut dyn Painter, t: &Theme, s: &Strings) {
        let (w, _) = self.size;
        let cx = self.content_x();
        for (hit, r) in &self.hits {
            if let Hit::Template(tpl) = hit {
                let thumb = [r[0], r[1], r[2], self.px(245.0)];
                if self.hover == Some(*hit) {
                    p.fill_rect(r[0] - 6.0, r[1] - 6.0, r[2] + 12.0, r[3] + 12.0, HOVER);
                }
                self.paint_template_thumb(p, t, *tpl, thumb);
                let label = match tpl {
                    None => tr(s, "sheetBs.blank", "Blank workbook"),
                    Some(i) => {
                        let (_, k, f) = TEMPLATES[*i];
                        tr(s, k, f)
                    }
                };
                let style = TextStyle {
                    size: self.px(13.0),
                    bold: true,
                };
                let lw = label.chars().count() as f32 * self.px(7.2);
                p.text(
                    &label,
                    (r[0] + (r[2] - lw) / 2.0).max(r[0]).round(),
                    r[1] + self.px(252.0),
                    style,
                    INK,
                );
            }
        }
        // Rule between the blank document and the gallery, then the gallery heading.
        let y = self.top + self.px(88.0 + 288.0);
        p.fill_rect(
            cx,
            y,
            (w - cx - self.px(64.0)).max(1.0),
            1.0,
            Rgba(212, 212, 212, 255),
        );
        self.heading(
            p,
            &tr(s, "sheetBs.templates", "BlueOffice templates"),
            cx,
            y + self.px(14.0),
            13.0,
            MUTED,
        );
    }

    /// Template thumbnails (own designs): the blank workbook as a small grid with column
    /// letters and row numbers (the reference draws its blank workbook this way), templates
    /// as white sheets with their table layout in blocks.
    #[allow(clippy::too_many_lines, clippy::unused_self)]
    fn paint_template_thumb(
        &self,
        p: &mut dyn Painter,
        t: &Theme,
        tpl: Option<usize>,
        r: [f32; 4],
    ) {
        p.fill_rect(r[0], r[1], r[2], r[3], Rgba(212, 212, 212, 255));
        p.fill_rect(r[0] + 1.0, r[1] + 1.0, r[2] - 2.0, r[3] - 2.0, WHITE);
        let (x, y, w, h) = (r[0], r[1], r[2], r[3]);
        let grid = Rgba(212, 212, 212, 255);
        let Some(i) = tpl else {
            // Headers: a grey band on top and on the left, three columns, rows of 1/8 height.
            let head = Rgba(230, 230, 230, 255);
            let rh = (h / 8.0).round();
            let hw = (w * 0.12).round();
            p.fill_rect(x + 1.0, y + 1.0, w - 2.0, rh - 1.0, head);
            p.fill_rect(x + 1.0, y + 1.0, hw - 1.0, h - 2.0, head);
            let cw = ((w - hw) / 2.6).round();
            for c in 0..3 {
                let cx = x + hw + cw * (c as f32 + 1.0);
                if cx < x + w - 1.0 {
                    p.fill_rect(cx, y + 1.0, 1.0, h - 2.0, grid);
                }
            }
            p.fill_rect(x + hw, y + 1.0, 1.0, h - 2.0, grid);
            let mut k = 1.0;
            while y + rh * k < y + h - 1.0 {
                p.fill_rect(x + 1.0, y + rh * k, w - 2.0, 1.0, grid);
                k += 1.0;
            }
            // The active cell A1: an accent frame.
            let a = t.accent_strong;
            let (ax, ay) = (x + hw, y + rh);
            p.fill_rect(ax, ay, cw + 1.0, 2.0, a);
            p.fill_rect(ax, ay + rh - 1.0, cw + 1.0, 2.0, a);
            p.fill_rect(ax, ay, 2.0, rh + 1.0, a);
            p.fill_rect(ax + cw - 1.0, ay, 2.0, rh + 1.0, a);
            return;
        };
        let accent = t.accent_strong;
        let light = Rgba(
            ((u16::from(accent.0) + 3 * 255) / 4) as u8,
            ((u16::from(accent.1) + 3 * 255) / 4) as u8,
            ((u16::from(accent.2) + 3 * 255) / 4) as u8,
            255,
        );
        let dark = Rgba(90, 90, 90, 255);
        let bar = |p: &mut dyn Painter, lx: f32, ly: f32, lw: f32, c: Rgba| {
            p.fill_rect(
                (x + w * lx).round(),
                (y + h * ly).round(),
                (w * lw).round(),
                (h * 0.014).max(1.0).round(),
                c,
            );
        };
        // A table: header row in the accent, `rows` rows with lines, `cols` columns.
        let table = |p: &mut dyn Painter, top: f32, rows: usize, cols: usize, banded: bool| {
            let (tx, tw) = (x + w * 0.08, w * 0.84);
            let rh = h * 0.045;
            p.fill_rect(tx, y + h * top, tw, rh, accent);
            for k in 0..rows {
                let ry = y + h * top + rh * (k as f32 + 1.0);
                if banded && k % 2 == 1 {
                    p.fill_rect(tx, ry, tw, rh, light);
                }
                p.fill_rect(tx, ry + rh - 1.0, tw, 1.0, grid);
            }
            for c in 1..cols {
                let cx = tx + tw * c as f32 / cols as f32;
                p.fill_rect(cx.round(), y + h * top + rh, 1.0, rh * rows as f32, grid);
            }
        };
        match TEMPLATES[i].0 {
            "budget" => {
                bar(p, 0.08, 0.07, 0.5, dark);
                bar(p, 0.08, 0.11, 0.3, accent);
                // Income and expense blocks, a bar chart at the bottom.
                table(p, 0.18, 4, 3, true);
                table(p, 0.45, 5, 3, true);
                for (k, v) in [0.6_f32, 0.85, 0.4, 0.7, 0.5].into_iter().enumerate() {
                    let bw = w * 0.1;
                    let bx = x + w * 0.12 + (bw + w * 0.06) * k as f32;
                    let bh = h * 0.16 * v;
                    p.fill_rect(bx, y + h * 0.93 - bh, bw, bh, accent);
                }
            }
            "invoice" => {
                bar(p, 0.08, 0.07, 0.35, accent);
                bar(p, 0.62, 0.07, 0.3, dark);
                bar(p, 0.62, 0.11, 0.22, grid);
                bar(p, 0.08, 0.16, 0.25, dark);
                bar(p, 0.08, 0.19, 0.3, grid);
                bar(p, 0.08, 0.22, 0.28, grid);
                table(p, 0.3, 8, 4, false);
                p.fill_rect(x + w * 0.6, y + h * 0.75, w * 0.32, h * 0.05, light);
                bar(p, 0.62, 0.77, 0.15, dark);
            }
            "schedule" => {
                bar(p, 0.08, 0.07, 0.45, dark);
                let (tx, tw) = (x + w * 0.08, w * 0.84);
                let rh = h * 0.06;
                p.fill_rect(tx, y + h * 0.15, tw, rh, accent);
                for k in 0..11 {
                    let ry = y + h * 0.15 + rh * (k as f32 + 1.0);
                    p.fill_rect(tx, ry + rh - 1.0, tw, 1.0, grid);
                }
                for c in 1..8 {
                    let cx = tx + tw * c as f32 / 8.0;
                    p.fill_rect(cx.round(), y + h * 0.15, 1.0, rh * 12.0, grid);
                }
                for (c, rr) in [(2, 1), (4, 3), (3, 6), (6, 2), (5, 8)] {
                    p.fill_rect(
                        tx + tw * c as f32 / 8.0 + 1.0,
                        y + h * 0.15 + rh * rr as f32 + 1.0,
                        tw / 8.0 - 2.0,
                        rh * 2.0 - 2.0,
                        light,
                    );
                }
            }
            "todo" => {
                bar(p, 0.08, 0.07, 0.35, dark);
                for k in 0..12 {
                    let ry = y + h * (0.16 + 0.065 * k as f32);
                    let s = (h * 0.035).round();
                    p.fill_rect(x + w * 0.1, ry, s, s, accent);
                    p.fill_rect(x + w * 0.1 + 1.0, ry + 1.0, s - 2.0, s - 2.0, WHITE);
                    let lw = if k % 3 == 2 { 0.45 } else { 0.62 };
                    bar(p, 0.24, 0.16 + 0.065 * k as f32 + 0.012, lw, grid);
                }
            }
            _ => {
                // Expense report: title, details, a banded table with a totals row.
                bar(p, 0.08, 0.07, 0.5, accent);
                bar(p, 0.08, 0.12, 0.3, dark);
                bar(p, 0.08, 0.15, 0.25, grid);
                table(p, 0.24, 10, 5, true);
                p.fill_rect(x + w * 0.08, y + h * 0.745, w * 0.84, h * 0.045, light);
            }
        }
    }

    fn paint_places(&self, p: &mut dyn Painter, t: &Theme, s: &Strings) {
        let (w, _) = self.size;
        let cx = self.content_x();
        let y = self.top + self.px(100.0);
        let open = self.page == BsPage::Open;
        let places = [
            if open {
                tr(s, "sheetBs.recent", "Recent")
            } else {
                tr(s, "sheetBs.thisPc", "This PC")
            },
            tr(s, "sheetBs.browse", "Browse"),
        ];
        let big = TextStyle {
            size: self.px(14.0),
            bold: false,
        };
        for (i, label) in places.iter().enumerate() {
            let r = [
                cx,
                y + self.px(64.0) * i as f32,
                self.px(300.0),
                self.px(60.0),
            ];
            let hit = Hit::Place(i as u8);
            if i == usize::from(self.place) {
                p.fill_rect(r[0], r[1], r[2], r[3], CHOSEN);
            } else if self.hover == Some(hit) {
                p.fill_rect(r[0], r[1], r[2], r[3], HOVER);
            }
            let ih = self.px(28.0);
            if i == 0 {
                if open {
                    clock_icon(p, r[0] + self.px(16.0), r[1] + (r[3] - ih) / 2.0, ih, INK);
                } else {
                    pc_icon(p, r[0] + self.px(16.0), r[1] + (r[3] - ih) / 2.0, ih, INK);
                }
            } else {
                folder_icon(p, r[0] + self.px(16.0), r[1] + (r[3] - ih) / 2.0, ih);
            }
            p.text(
                label,
                r[0] + self.px(60.0),
                r[1] + (r[3] - big.size * 1.33) / 2.0,
                big,
                INK,
            );
        }
        // Divider between the places and the list.
        let lx = cx + self.px(330.0);
        p.fill_rect(
            lx - self.px(14.0),
            y,
            1.0,
            (self.size.1 - y - self.px(40.0)).max(1.0),
            Rgba(212, 212, 212, 255),
        );
        if open {
            self.heading(
                p,
                &tr(s, "sheetBs.documents", "Documents"),
                lx,
                y,
                15.0,
                t.accent_text,
            );
            p.fill_rect(
                lx,
                y + self.px(30.0),
                self.px(84.0),
                self.px(2.0),
                t.accent_text,
            );
            self.paint_recent(p, s, lx, y + self.px(44.0), 48.0, 14);
        } else {
            self.heading(p, &tr(s, "sheetBs.thisPc", "This PC"), lx, y, 18.0, INK);
            let folder = self
                .facts
                .path
                .as_deref()
                .map(|p| match p.rfind(['\\', '/']) {
                    Some(i) => p[..i].to_owned(),
                    None => String::new(),
                })
                .unwrap_or_default();
            let crumbs = folder.split(['\\', '/']).collect::<Vec<_>>().join(" › ");
            p.text(&crumbs, lx, y + self.px(44.0), self.body(), MUTED);
            let small = self.body();
            p.text(
                &tr(s, "sheetBs.fileName", "File name"),
                lx,
                y + self.px(80.0),
                small,
                MUTED,
            );
            // Name box, type drop-down, Save button.
            let by = y + self.px(124.0);
            let nb = [
                lx,
                by - self.px(32.0) - self.px(10.0),
                self.px(456.0),
                self.px(28.0),
            ];
            field(p, nb, self.scale);
            let stem = self
                .facts
                .name
                .rsplit_once('.')
                .map_or(self.facts.name.as_str(), |(a, _)| a);
            p.text(stem, nb[0] + self.px(6.0), nb[1] + self.px(5.0), small, INK);
            let tb = [lx, by, self.px(360.0), self.px(28.0)];
            field(p, tb, self.scale);
            let (_, k, f) = FILE_TYPES[self.file_type];
            p.text(
                &tr(s, k, f),
                tb[0] + self.px(6.0),
                tb[1] + self.px(5.0),
                small,
                INK,
            );
            chevron(
                p,
                tb[0] + tb[2] - self.px(16.0),
                tb[1] + tb[3] / 2.0,
                self.px(4.0),
                INK,
            );
            let sb = [lx + self.px(372.0), by, self.px(84.0), self.px(28.0)];
            let hover = self.hover == Some(Hit::Action(1));
            p.fill_rect(sb[0], sb[1], sb[2], sb[3], t.accent_strong);
            if hover {
                p.fill_rect(sb[0], sb[1], sb[2], sb[3], Rgba(0, 0, 0, 40));
            }
            let lbl = tr(s, "backstage.nav.save", "Save");
            let lw = self.width_of(2, &lbl);
            p.text(
                &lbl,
                (sb[0] + (sb[2] - lw) / 2.0).round(),
                sb[1] + self.px(5.0),
                small,
                t.accent_on,
            );
            let _ = w;
        }
    }

    fn paint_info(&self, p: &mut dyn Painter, t: &Theme, s: &Strings) {
        let (w, _) = self.size;
        let cx = self.content_x();
        let name = self
            .facts
            .name
            .rsplit_once('.')
            .map_or(self.facts.name.as_str(), |(a, _)| a);
        p.text(
            name,
            cx,
            self.top + self.px(92.0),
            TextStyle {
                size: self.px(21.0),
                bold: false,
            },
            t.accent_text,
        );
        let body = self.body();
        // The folder holding the workbook, as the reference (the file name is the heading).
        let path = match &self.facts.path {
            Some(path) => {
                let mut parts: Vec<&str> =
                    path.split(['\\', '/']).filter(|x| !x.is_empty()).collect();
                parts.pop();
                parts.join(" » ")
            }
            None => tr(
                s,
                "sheetBs.notSaved",
                "This workbook has not been saved yet.",
            ),
        };
        // The path line runs above the properties column, up to the window's edge (measured:
        // the reference shows the whole folder at 1500 px).
        let clip_w = w - cx - self.px(24.0);
        p.push_clip(
            cx,
            self.top + self.px(120.0),
            clip_w.max(1.0),
            self.px(30.0),
        );
        p.text(&path, cx, self.top + self.px(127.0), body, INK);
        p.pop_clip();
        // Buttons: Copy path, Open file location.
        for (hit, r) in &self.hits {
            let Hit::InfoButton(i) = hit else { continue };
            let enabled = self.facts.path.is_some();
            p.fill_rect(r[0], r[1], r[2], r[3], Rgba(144, 144, 144, 255));
            let fill = if enabled && self.hover == Some(*hit) {
                Rgba(229, 241, 251, 255)
            } else {
                WHITE
            };
            p.fill_rect(r[0] + 1.0, r[1] + 1.0, r[2] - 2.0, r[3] - 2.0, fill);
            let (_, k, f) = INFO_BUTTONS[usize::from(*i)];
            let ih = self.px(16.0);
            let iy = r[1] + (r[3] - ih) / 2.0;
            let ink = if enabled { INK } else { PLACEHOLDER };
            if *i == 0 {
                link_icon(p, r[0] + self.px(10.0), iy, ih, ink);
            } else {
                folder_icon(p, r[0] + self.px(10.0), iy, ih);
            }
            p.text(
                &tr(s, k, f),
                r[0] + self.px(34.0),
                r[1] + (r[3] - body.size * 1.33) / 2.0,
                body,
                ink,
            );
        }
        // Tiles with their headings and descriptions.
        let tiles: [(&str, &str, &str, &str, Vec<String>); 3] = [
            (
                "sheetBs.protect",
                "Protect Workbook",
                "sheetBs.protectDesc",
                if self.facts.protected {
                    "Restrictions are enforced: some changes are limited."
                } else {
                    "Control what types of changes people can make to this workbook."
                },
                vec![],
            ),
            (
                "sheetBs.inspect",
                "Inspect Workbook",
                "sheetBs.inspectDesc",
                "Before publishing this file, be aware that it contains:",
                vec![tr(
                    s,
                    "sheetBs.hasProps",
                    "Document properties, author's name, related dates and absolute path",
                )],
            ),
            (
                "sheetBs.manage",
                "Manage Workbook",
                "sheetBs.manageDesc",
                if self.facts.modified {
                    "There are unsaved changes."
                } else {
                    "There are no unsaved changes."
                },
                vec![],
            ),
        ];
        let tile_labels = [
            ("sheetBs.protectTile", "Protect\nWorkbook"),
            ("sheetBs.inspectTile", "Check for\nIssues"),
            ("sheetBs.manageTile", "Manage\nWorkbook"),
        ];
        let desc_key_text = |key: &str, fb: &str| {
            if key == "sheetBs.protectDesc" && self.facts.protected {
                tr(s, "sheetBs.protectOn", fb)
            } else if key == "sheetBs.manageDesc" && self.facts.modified {
                tr(s, "sheetBs.unsaved", fb)
            } else {
                tr(s, key, fb)
            }
        };
        for (i, ty) in self.tile_tops().into_iter().enumerate() {
            let r = [cx + self.px(4.0), ty, self.px(104.0), self.px(101.0)];
            let hover = self.hover == Some(Hit::Tile(i as u8));
            p.fill_rect(
                r[0],
                r[1],
                r[2],
                r[3],
                if hover {
                    Rgba(144, 144, 144, 255)
                } else {
                    FRAME
                },
            );
            p.fill_rect(
                r[0] + 1.0,
                r[1] + 1.0,
                r[2] - 2.0,
                r[3] - 2.0,
                if hover {
                    Rgba(229, 241, 251, 255)
                } else {
                    WHITE
                },
            );
            let ih = self.px(34.0);
            let ix = r[0] + (r[2] - ih) / 2.0;
            match i {
                0 => lock_icon(p, ix, r[1] + self.px(12.0), ih, t.accent_strong),
                1 => inspect_icon(p, ix, r[1] + self.px(12.0), ih, t.accent_strong),
                _ => manage_icon(p, ix, r[1] + self.px(12.0), ih, t.accent_strong),
            }
            let (k, f) = tile_labels[i];
            let label = tr(s, k, f);
            let semi = TextStyle {
                size: self.px(12.5),
                bold: true,
            };
            let lines: Vec<&str> = label.lines().collect();
            for (j, l) in lines.iter().enumerate() {
                let text = if j + 1 == lines.len() {
                    format!("{l} ▾")
                } else {
                    (*l).to_owned()
                };
                let lw = text.chars().count() as f32 * self.px(6.6);
                p.text(
                    &text,
                    (r[0] + (r[2] - lw) / 2.0).round(),
                    r[1] + self.px(52.0) + self.px(19.0) * j as f32,
                    semi,
                    INK,
                );
            }
            let (hk, hf, dk, df, bullets) = &tiles[i];
            let tx = cx + self.px(126.0);
            self.heading(p, &tr(s, hk, hf), tx, ty + self.px(2.0), 18.0, INK);
            p.text(&desc_key_text(dk, df), tx, ty + self.px(36.0), body, INK);
            for (j, b) in bullets.iter().enumerate() {
                let by = ty + self.px(60.0) + self.px(24.0) * j as f32;
                p.fill_rect(
                    tx + self.px(5.0),
                    by + self.px(7.0),
                    self.px(7.0),
                    self.px(7.0),
                    Rgba(166, 166, 166, 255),
                );
                p.text(b, tx + self.px(30.0), by, body, INK);
            }
        }
        if self.facts.compat {
            self.paint_compat(p, t, s);
        }
        self.paint_properties(p, t, s, w);
    }

    /// The Compatibility Mode block (measured: #FCF7B6 framed #D9C24C, 590 × 113 px logical)
    /// with its Convert tile.
    fn paint_compat(&self, p: &mut dyn Painter, t: &Theme, s: &Strings) {
        let b = self.compat_box();
        p.fill_rect(b[0], b[1], b[2], b[3], Rgba(217, 194, 76, 255));
        p.fill_rect(
            b[0] + 1.0,
            b[1] + 1.0,
            b[2] - 2.0,
            b[3] - 2.0,
            Rgba(252, 247, 182, 255),
        );
        let r = self.convert_tile();
        let hover = self.hover == Some(Hit::Action(9));
        p.fill_rect(r[0], r[1], r[2], r[3], FRAME);
        p.fill_rect(
            r[0] + 1.0,
            r[1] + 1.0,
            r[2] - 2.0,
            r[3] - 2.0,
            if hover {
                Rgba(229, 241, 251, 255)
            } else {
                WHITE
            },
        );
        let ih = self.px(30.0);
        save_type_icon(p, r[0] + (r[2] - ih) / 2.0, r[1] + self.px(14.0), ih, t);
        let label = tr(s, "sheetBs.convert", "Convert");
        let semi = TextStyle {
            size: self.px(12.5),
            bold: true,
        };
        let lw = label.chars().count() as f32 * self.px(6.8);
        p.text(
            &label,
            (r[0] + (r[2] - lw) / 2.0).round(),
            r[1] + self.px(64.0),
            semi,
            INK,
        );
        let tx = b[0] + self.px(126.0);
        self.heading(
            p,
            &tr(s, "sheetBs.compat", "Compatibility Mode"),
            tx,
            b[1] + self.px(8.0),
            18.0,
            INK,
        );
        let body = self.body();
        let desc = tr(
            s,
            "sheetBs.compatDesc",
            "Some new features are disabled to prevent problems when working with\nprevious versions of word processors. Converting this file will enable these\nfeatures, but may result in layout changes.",
        );
        for (i, line) in desc.lines().enumerate() {
            p.text(
                line,
                tx,
                b[1] + self.px(42.0) + self.px(19.0) * i as f32,
                body,
                INK,
            );
        }
    }

    #[allow(clippy::too_many_lines)]
    fn paint_properties(&self, p: &mut dyn Painter, t: &Theme, s: &Strings, w: f32) {
        let x = (w - self.px(322.0)).max(self.content_x() + self.px(640.0));
        let vx = x + self.px(130.0);
        let mut y = self.top + self.px(222.0);
        let body = self.body();
        let head = |p: &mut dyn Painter, text: &str, y: f32| {
            p.text(
                text,
                x,
                y,
                TextStyle {
                    size: self.px(16.0),
                    bold: false,
                },
                INK,
            );
        };
        head(
            p,
            &format!("{} ▾", tr(s, "backstage.info.properties", "Properties")),
            y,
        );
        y += self.px(31.0);
        let f = &self.facts;
        let size = f.bytes.map_or_else(String::new, |b| {
            if b < 1024 * 1024 {
                format!("{:.2}KB", b as f64 / 1024.0)
            } else {
                format!("{:.1}MB", b as f64 / 1024.0 / 1024.0)
            }
        });
        let minutes = format!(
            "{} {}",
            f.editing_minutes.unwrap_or(0),
            tr(s, "sheetBs.minutes", "Minutes")
        );
        let _ = minutes;
        let mut rows: Vec<(String, String, &str, &str)> = vec![
            (tr(s, "sheetBs.size", "Size"), size, "", ""),
            (
                tr(s, "backstage.props.title", "Title"),
                f.title.clone(),
                "sheetBs.addTitle",
                "Add a title",
            ),
            (
                tr(s, "sheetBs.tags", "Tags"),
                f.tags.clone(),
                "sheetBs.addTag",
                "Add a tag",
            ),
            (
                tr(s, "sheetBs.categories", "Categories"),
                f.categories.clone(),
                "sheetBs.addCategory",
                "Add a category",
            ),
        ];
        // Show All Properties: the rest of the core and extended properties.
        if self.all_props {
            for (k, fb, v) in &f.more {
                rows.push((tr(s, k, fb), v.clone(), "", ""));
            }
        }
        for (k, v, pk, pf) in rows {
            p.text(&k, x, y, body, INK);
            if v.is_empty() {
                if !pk.is_empty() {
                    p.text(&tr(s, pk, pf), vx, y, body, PLACEHOLDER);
                }
            } else {
                p.text(&v, vx, y, body, INK);
            }
            y += self.px(29.0);
        }
        y += self.px(16.0);
        head(p, &tr(s, "sheetBs.relatedDates", "Related Dates"), y);
        y += self.px(31.0);
        let never = tr(s, "sheetBs.never", "Never");
        for (k, v) in [
            (
                tr(s, "backstage.props.modified", "Last Modified"),
                f.last_modified.clone(),
            ),
            (
                tr(s, "backstage.props.created", "Created"),
                f.created.clone(),
            ),
            (tr(s, "sheetBs.printed", "Last Printed"), String::new()),
        ] {
            p.text(&k, x, y, body, INK);
            p.text(if v.is_empty() { &never } else { &v }, vx, y, body, INK);
            y += self.px(29.0);
        }
        y += self.px(16.0);
        head(p, &tr(s, "sheetBs.relatedPeople", "Related People"), y);
        y += self.px(31.0);
        let not_saved = tr(s, "sheetBs.notSavedYet", "Not saved yet");
        for (k, v, ph) in [
            (
                tr(s, "backstage.props.author", "Author"),
                f.author.clone(),
                tr(s, "sheetBs.addAuthor", "Add an author"),
            ),
            (
                tr(s, "backstage.props.lastModifiedBy", "Last Modified By"),
                f.last_modified_by.clone(),
                not_saved,
            ),
        ] {
            p.text(&k, x, y, body, INK);
            if v.is_empty() {
                p.text(&ph, vx, y, body, PLACEHOLDER);
            } else {
                p.text(&v, vx, y, body, INK);
            }
            y += self.px(29.0);
        }
        if f.path.is_some() {
            y += self.px(16.0);
            head(p, &tr(s, "sheetBs.relatedDocs", "Related Documents"), y);
            y += self.px(34.0);
            folder_icon(p, x + self.px(8.0), y, self.px(22.0));
            p.text(
                &tr(s, "sheetBs.openLocation", "Open File Location"),
                x + self.px(46.0),
                y + self.px(1.0),
                body,
                INK,
            );
        }
        let link = self.show_all_rect();
        p.text(
            &if self.all_props {
                tr(s, "sheetBs.fewerProps", "Show Fewer Properties")
            } else {
                tr(s, "sheetBs.allProps", "Show All Properties")
            },
            link[0],
            link[1],
            body,
            t.accent_text,
        );
    }

    #[allow(clippy::too_many_lines)]
    fn paint_print(&self, p: &mut dyn Painter, t: &Theme, s: &Strings) {
        let (_, h) = self.size;
        let cx = self.content_x();
        let y = self.top + self.px(80.0);
        let body = self.body();
        // The Print tile.
        let r = [cx + self.px(3.0), y, self.px(101.0), self.px(101.0)];
        let hover = self.hover == Some(Hit::Action(0));
        p.fill_rect(r[0], r[1], r[2], r[3], FRAME);
        p.fill_rect(
            r[0] + 1.0,
            r[1] + 1.0,
            r[2] - 2.0,
            r[3] - 2.0,
            if hover {
                Rgba(229, 241, 251, 255)
            } else {
                WHITE
            },
        );
        let ih = self.px(34.0);
        printer_icon(
            p,
            r[0] + (r[2] - ih) / 2.0,
            r[1] + self.px(14.0),
            ih,
            t.accent_strong,
        );
        let lbl = tr(s, "backstage.nav.print", "Print");
        let semi = TextStyle {
            size: self.px(12.5),
            bold: true,
        };
        let lw = lbl.chars().count() as f32 * self.px(6.8);
        p.text(
            &lbl,
            (r[0] + (r[2] - lw) / 2.0).round(),
            r[1] + self.px(64.0),
            semi,
            INK,
        );
        // Copies.
        let sx = cx + self.px(178.0);
        p.text(
            &tr(s, "sheetBs.copies", "Copies:"),
            cx + self.px(118.0),
            y + self.px(6.0),
            body,
            INK,
        );
        let cb = [sx, y, self.px(76.0), self.px(30.0)];
        field(p, cb, self.scale);
        p.text(
            &self.print.copies.to_string(),
            cb[0] + self.px(6.0),
            cb[1] + self.px(6.0),
            body,
            INK,
        );
        let ax = sx + self.px(64.0);
        chevron_up(p, ax, y + self.px(9.0), self.px(3.0), INK);
        chevron(p, ax, y + self.px(21.0), self.px(3.0), INK);
        // Printer heading and its box.
        let tops = self.setting_tops();
        let hs = |p: &mut dyn Painter, text: &str, y: f32| {
            p.text(
                text,
                cx + self.px(4.0),
                y,
                TextStyle {
                    size: self.px(20.0),
                    bold: false,
                },
                t.accent_text,
            );
        };
        hs(
            p,
            &tr(s, "sheetBs.printer", "Printer"),
            tops[0] - self.px(36.0),
        );
        hs(
            p,
            &tr(s, "sheetBs.settings", "Settings"),
            tops[1] - self.px(36.0),
        );
        let printer = self
            .print
            .printers
            .get(self.print.printer)
            .cloned()
            .unwrap_or_else(|| tr(s, "sheetBs.noPrinter", "No printer installed"));
        // The spooler's state of the chosen printer, worded as the reference words it.
        let (error, offline, jobs) = self
            .print
            .states
            .get(self.print.printer)
            .copied()
            .unwrap_or((false, false, 0));
        let waiting = match jobs {
            0 => String::new(),
            1 => tr(s, "sheetBs.waiting1", "1 document waiting"),
            n => tr(s, "sheetBs.waitingN", "{n} documents waiting").replace("{n}", &n.to_string()),
        };
        let trouble = error || offline || jobs > 0;
        let ready = if offline {
            tr(s, "sheetBs.offline", "Offline")
        } else if (error || jobs > 0) && !waiting.is_empty() {
            format!("{}: {waiting}", tr(s, "sheetBs.error", "Error"))
        } else if error {
            tr(s, "sheetBs.error", "Error")
        } else {
            tr(s, "sheetBs.ready", "Ready")
        };
        let (range, range_sub) = range_labels(s, self.print.range);
        let (sides, sides_sub) = match self.print.sides {
            1 => (
                tr(s, "sheetBs.bothLong", "Print on Both Sides"),
                tr(s, "sheetBs.bothLongSub", "Flip pages on long edge"),
            ),
            2 => (
                tr(s, "sheetBs.bothShort", "Print on Both Sides"),
                tr(s, "sheetBs.bothShortSub", "Flip pages on short edge"),
            ),
            _ => (
                tr(s, "sheetBs.oneSided", "Print One Sided"),
                tr(
                    s,
                    "sheetBs.oneSidedSub",
                    "Only print on one side of the page",
                ),
            ),
        };
        let (collate, collate_sub) = if self.print.collated {
            (
                tr(s, "sheetBs.collated", "Collated"),
                "1,2,3    1,2,3    1,2,3".to_owned(),
            )
        } else {
            (
                tr(s, "sheetBs.uncollated", "Uncollated"),
                "1,1,1    2,2,2    3,3,3".to_owned(),
            )
        };
        let orient = if self.facts.landscape {
            tr(s, "sheetBs.landscape", "Landscape Orientation")
        } else {
            tr(s, "sheetBs.portrait", "Portrait Orientation")
        };
        let (pname, pw, ph) = &self.facts.paper;
        let paper_sub = format!("{pw:.2}\" x {ph:.2}\"");
        let (per, per_sub) = scaling_labels(s, self.print.scaling);
        let boxes: [(String, String, u8); 8] = [
            (printer, ready, 0),
            (range, range_sub, 1),
            (sides, sides_sub, 2),
            (collate, collate_sub, 3),
            (orient, String::new(), 4),
            (pname.clone(), paper_sub, 5),
            (self.facts.margins.clone(), String::new(), 6),
            (per, per_sub, 7),
        ];
        let shown = self.shown_settings();
        for ((label, sub, icon), ry) in boxes.iter().filter(|b| shown.contains(&b.2)).zip(&tops) {
            let r = [cx + self.px(4.0), *ry, self.px(293.0), self.px(51.0)];
            let hover = self.hover == Some(Hit::PrintSetting(*icon));
            p.fill_rect(
                r[0],
                r[1],
                r[2],
                r[3],
                if hover {
                    Rgba(144, 144, 144, 255)
                } else {
                    FRAME
                },
            );
            p.fill_rect(
                r[0] + 1.0,
                r[1] + 1.0,
                r[2] - 2.0,
                r[3] - 2.0,
                if hover {
                    Rgba(229, 241, 251, 255)
                } else {
                    WHITE
                },
            );
            setting_icon(
                p,
                *icon,
                r[0] + self.px(8.0),
                r[1] + self.px(8.0),
                self.px(34.0),
                t,
            );
            if *icon == 0 && !self.print.printers.is_empty() {
                status_badge(
                    p,
                    r[0] + self.px(36.0),
                    r[1] + self.px(36.0),
                    self.px(8.0),
                    trouble,
                );
            }
            let tx = r[0] + self.px(56.0);
            p.push_clip(r[0], r[1], r[2] - self.px(24.0), r[3]);
            if sub.is_empty() {
                p.text(label, tx, r[1] + (r[3] - body.size * 1.33) / 2.0, body, INK);
            } else {
                p.text(label, tx, r[1] + self.px(4.0), body, INK);
                p.text(sub, tx, r[1] + self.px(26.0), body, INK);
            }
            p.pop_clip();
            chevron(
                p,
                r[0] + r[2] - self.px(12.0),
                r[1] + r[3] / 2.0,
                self.px(4.0),
                INK,
            );
        }
        // Links: Printer Properties, Page Setup; info marks after Printer and Pages.
        let links = self.print_links();
        for (i, (k, fb)) in [
            ("sheetBs.printerProps", "Printer Properties"),
            ("sheetBs.pageSetup", "Page Setup"),
            ("sheetBs.morePrint", "Find more print settings"),
        ]
        .into_iter()
        .enumerate()
        {
            let l = links[i];
            p.text(&tr(s, k, fb), l[0], l[1], body, t.accent_text);
            p.fill_rect(l[0], l[1] + self.px(17.0), l[2], 1.0, t.accent_text);
        }
        info_mark(
            p,
            cx + self.px(290.0),
            tops[0] - self.px(26.0),
            self.px(7.0),
            t.accent_text,
        );
        // Pages: [from] to [to], two spin boxes under what prints.
        let pb = self.pages_box();
        p.text(
            &tr(s, "sheetBs.pages", "Pages:"),
            cx + self.px(0.0),
            pb[1] + self.px(6.0),
            body,
            INK,
        );
        p.text(
            &tr(s, "sheetBs.to", "to"),
            pb[0] + pb[2] + self.px(12.0),
            pb[1] + self.px(6.0),
            body,
            INK,
        );
        for (b, text, focus) in [
            (pb, &self.print.pages, self.pages_focus),
            (self.pages_to_box(), &self.print.pages_to, self.to_focus),
        ] {
            field(p, b, self.scale);
            if focus {
                p.fill_rect(b[0], b[1], b[2], 1.0, t.accent_strong);
                p.fill_rect(b[0], b[1] + b[3] - 1.0, b[2], 1.0, t.accent_strong);
            }
            p.text(text, b[0] + self.px(5.0), b[1] + self.px(6.0), body, INK);
            let ax = b[0] + b[2] - self.px(10.0);
            chevron_up(p, ax, b[1] + self.px(9.0), self.px(3.0), INK);
            chevron(p, ax, b[1] + self.px(21.0), self.px(3.0), INK);
        }
        // Preview: a rule left of it, the page framed dark grey on the content colour
        // (measured on the reference: frame #404040), Show Margins / Zoom to Page at the
        // bottom right.
        let a = self.preview_area();
        p.fill_rect(
            a[0] - self.px(28.0),
            a[1],
            1.0,
            a[3],
            Rgba(212, 212, 212, 255),
        );
        if let Some((_, r)) = self.preview_rect() {
            p.push_clip(a[0], a[1], a[2], a[3]);
            // A black outline 2 px wide at 125 % (measured).
            let e = self.px(1.6);
            p.fill_rect(
                r[0] - e,
                r[1] - e,
                r[2] + 2.0 * e,
                r[3] + 2.0 * e,
                Rgba(0, 0, 0, 255),
            );
            p.fill_rect(r[0], r[1], r[2], r[3], WHITE);
            p.pop_clip();
        }
        let [margins, fit] = self.corner_rects();
        for (i, b) in [margins, fit].into_iter().enumerate() {
            let on = if i == 0 {
                self.show_margins
            } else {
                self.zoom > 1.0
            };
            let hit = if i == 0 { Hit::ZoomOut } else { Hit::ZoomFit };
            if on || self.hover == Some(hit) {
                p.fill_rect(b[0], b[1], b[2], b[3], Rgba(144, 144, 144, 255));
                p.fill_rect(
                    b[0] + 1.0,
                    b[1] + 1.0,
                    b[2] - 2.0,
                    b[3] - 2.0,
                    if on { CHOSEN } else { HOVER },
                );
            }
            corner_icon(p, i == 1, b, self.scale);
        }
        let ny = h - self.px(34.0);
        let n = self.facts.page_sizes.len().max(1);
        chevron_left(
            p,
            a[0] + self.px(8.0),
            ny + self.px(12.0),
            self.px(4.0),
            MUTED,
        );
        let nb = [a[0] + self.px(26.0), ny, self.px(38.0), self.px(24.0)];
        field(p, nb, self.scale);
        p.text(
            &(self.preview.min(n - 1) + 1).to_string(),
            nb[0] + self.px(6.0),
            nb[1] + self.px(3.0),
            body,
            INK,
        );
        p.text(
            &format!("{} {n}", tr(s, "sheetBs.of", "of")),
            nb[0] + nb[2] + self.px(8.0),
            nb[1] + self.px(3.0),
            body,
            INK,
        );
        chevron_right(
            p,
            a[0] + self.px(124.0),
            ny + self.px(12.0),
            self.px(4.0),
            MUTED,
        );
    }

    fn paint_export(&self, p: &mut dyn Painter, t: &Theme, s: &Strings) {
        let cx = self.content_x();
        let y = self.top + self.px(84.0);
        let big = TextStyle {
            size: self.px(14.5),
            bold: false,
        };
        let body = self.body();
        let items = [
            tr(s, "sheetBs.createPdf", "Create PDF/XPS Document"),
            tr(s, "sheetBs.changeType", "Change File Type"),
        ];
        for (i, label) in items.iter().enumerate() {
            let r = [
                cx,
                y + self.px(66.0) * i as f32,
                self.px(375.0),
                self.px(62.0),
            ];
            let hit = Hit::ExportItem(i as u8);
            if i == usize::from(self.export) {
                p.fill_rect(r[0], r[1], r[2], r[3], Rgba(171, 171, 171, 255));
                p.fill_rect(r[0] + 1.0, r[1] + 1.0, r[2] - 2.0, r[3] - 2.0, CHOSEN);
            } else if self.hover == Some(hit) {
                p.fill_rect(r[0], r[1], r[2], r[3], HOVER);
            }
            let ih = self.px(30.0);
            if i == 0 {
                pdf_icon(p, r[0] + self.px(12.0), r[1] + (r[3] - ih) / 2.0, ih, t);
            } else {
                save_type_icon(p, r[0] + self.px(12.0), r[1] + (r[3] - ih) / 2.0, ih, t);
            }
            p.text(
                label,
                r[0] + self.px(66.0),
                r[1] + (r[3] - big.size * 1.33) / 2.0,
                big,
                INK,
            );
        }
        p.fill_rect(
            cx + self.px(376.0),
            y,
            1.0,
            (self.size.1 - y - self.px(40.0)).max(1.0),
            Rgba(212, 212, 212, 255),
        );
        let rx = cx + self.px(410.0);
        if self.export == 0 {
            let head = tr(s, "sheetBs.createPdfHead", "Create a PDF/XPS Document");
            self.heading(p, &head, rx, y + self.px(2.0), 21.0, t.accent_text);
            for (i, (k, f)) in [
                (
                    "sheetBs.pdf1",
                    "Preserves layout, formatting, fonts, and images",
                ),
                ("sheetBs.pdf2", "Content can't be easily changed"),
                ("sheetBs.pdf3", "Free viewers are available on the web"),
            ]
            .into_iter()
            .enumerate()
            {
                let by = y + self.px(44.0) + self.px(26.0) * i as f32;
                p.fill_rect(
                    rx + self.px(6.0),
                    by + self.px(6.0),
                    self.px(7.0),
                    self.px(7.0),
                    Rgba(166, 166, 166, 255),
                );
                p.text(&tr(s, k, f), rx + self.px(30.0), by, body, INK);
            }
            if let Some(r) = self.rect_of(Hit::Action(0)) {
                self.big_button(
                    p,
                    t,
                    r,
                    &tr(s, "sheetBs.createPdfTile", "Create PDF/\nXPS"),
                    0,
                );
            }
        } else {
            self.heading(p, &items[1], rx, y + self.px(2.0), 21.0, t.accent_text);
            p.text(
                &tr(s, "sheetBs.docTypes", "Workbook File Types"),
                rx,
                y + self.px(44.0),
                TextStyle {
                    size: self.px(13.0),
                    bold: true,
                },
                INK,
            );
            for (hit, r) in &self.hits {
                let Hit::TypeRow(i) = hit else { continue };
                if *i == self.file_type {
                    p.fill_rect(r[0], r[1], r[2], r[3], CHOSEN);
                } else if self.hover == Some(*hit) {
                    p.fill_rect(r[0], r[1], r[2], r[3], HOVER);
                }
                let (_, k, f) = FILE_TYPES[*i];
                doc_icon(p, r[0] + self.px(8.0), r[1] + self.px(8.0), self.px(26.0));
                p.text(
                    &tr(s, k, f),
                    r[0] + self.px(48.0),
                    r[1] + (r[3] - body.size * 1.33) / 2.0,
                    body,
                    INK,
                );
            }
            if let Some(r) = self.rect_of(Hit::Action(1)) {
                self.big_button(p, t, r, &tr(s, "sheetBs.saveAsTile", "Save\nAs"), 1);
            }
        }
    }

    /// Share: Email (a new message in the mail program, naming the file and its location).
    fn paint_share(&self, p: &mut dyn Painter, t: &Theme, s: &Strings) {
        let cx = self.content_x();
        let y = self.top + self.px(84.0);
        let big = TextStyle {
            size: self.px(14.5),
            bold: false,
        };
        let body = self.body();
        let r = [cx, y, self.px(375.0), self.px(62.0)];
        p.fill_rect(r[0], r[1], r[2], r[3], Rgba(171, 171, 171, 255));
        p.fill_rect(r[0] + 1.0, r[1] + 1.0, r[2] - 2.0, r[3] - 2.0, CHOSEN);
        let ih = self.px(30.0);
        mail_icon(
            p,
            r[0] + self.px(12.0),
            r[1] + (r[3] - ih) / 2.0,
            ih,
            t.accent_strong,
        );
        let label = tr(s, "sheetBs.email", "Email");
        p.text(
            &label,
            r[0] + self.px(66.0),
            r[1] + (r[3] - big.size * 1.33) / 2.0,
            big,
            INK,
        );
        p.fill_rect(
            cx + self.px(376.0),
            y,
            1.0,
            (self.size.1 - y - self.px(40.0)).max(1.0),
            Rgba(212, 212, 212, 255),
        );
        let rx = cx + self.px(410.0);
        self.heading(p, &label, rx, y + self.px(2.0), 21.0, t.accent_text);
        for (i, (k, f)) in [
            ("sheetBs.email1", "Opens a new message in your mail program"),
            ("sheetBs.email2", "Names the document and where it is saved"),
            (
                "sheetBs.email3",
                "Attach the file in the message before you send it",
            ),
        ]
        .into_iter()
        .enumerate()
        {
            let by = y + self.px(44.0) + self.px(26.0) * i as f32;
            p.fill_rect(
                rx + self.px(6.0),
                by + self.px(6.0),
                self.px(7.0),
                self.px(7.0),
                Rgba(166, 166, 166, 255),
            );
            p.text(&tr(s, k, f), rx + self.px(30.0), by, body, INK);
        }
        if let Some(b) = self.rect_of(Hit::Action(0)) {
            let hover = self.hover == Some(Hit::Action(0));
            p.fill_rect(b[0], b[1], b[2], b[3], FRAME);
            p.fill_rect(
                b[0] + 1.0,
                b[1] + 1.0,
                b[2] - 2.0,
                b[3] - 2.0,
                if hover {
                    Rgba(229, 241, 251, 255)
                } else {
                    WHITE
                },
            );
            mail_icon(
                p,
                b[0] + (b[2] - ih) / 2.0,
                b[1] + self.px(12.0),
                ih,
                t.accent_strong,
            );
            let semi = TextStyle {
                size: self.px(12.5),
                bold: true,
            };
            let text = tr(s, "sheetBs.sendEmail", "Send by\nEmail");
            for (j, l) in text.lines().enumerate() {
                let lw = l.chars().count() as f32 * self.px(6.8);
                p.text(
                    l,
                    (b[0] + (b[2] - lw) / 2.0).round(),
                    b[1] + self.px(52.0) + self.px(19.0) * j as f32,
                    semi,
                    INK,
                );
            }
        }
    }

    fn big_button(&self, p: &mut dyn Painter, t: &Theme, r: [f32; 4], label: &str, icon: u8) {
        let hover = self.hover == Some(Hit::Action(icon));
        p.fill_rect(r[0], r[1], r[2], r[3], FRAME);
        p.fill_rect(
            r[0] + 1.0,
            r[1] + 1.0,
            r[2] - 2.0,
            r[3] - 2.0,
            if hover {
                Rgba(229, 241, 251, 255)
            } else {
                WHITE
            },
        );
        let ih = self.px(30.0);
        if icon == 0 {
            pdf_icon(p, r[0] + (r[2] - ih) / 2.0, r[1] + self.px(12.0), ih, t);
        } else {
            save_type_icon(p, r[0] + (r[2] - ih) / 2.0, r[1] + self.px(12.0), ih, t);
        }
        let semi = TextStyle {
            size: self.px(12.5),
            bold: true,
        };
        for (j, l) in label.lines().enumerate() {
            let lw = l.chars().count() as f32 * self.px(6.8);
            p.text(
                l,
                (r[0] + (r[2] - lw) / 2.0).round(),
                r[1] + self.px(52.0) + self.px(19.0) * j as f32,
                semi,
                INK,
            );
        }
    }
}

/// What prints (Print Active Sheets / Entire Workbook / Selection): label and line below.
pub(crate) fn range_labels(s: &Strings, range: u8) -> (String, String) {
    match range {
        1 => (
            tr(s, "sheetBs.entireWorkbook", "Print Entire Workbook"),
            tr(s, "sheetBs.entireWorkbookSub", "Print the entire workbook"),
        ),
        2 => (
            tr(s, "sheetBs.selection", "Print Selection"),
            tr(
                s,
                "sheetBs.selectionSub",
                "Only print the current selection",
            ),
        ),
        _ => (
            tr(s, "sheetBs.activeSheets", "Print Active Sheets"),
            tr(s, "sheetBs.activeSheetsSub", "Only print the active sheets"),
        ),
    }
}

/// Scaling (No Scaling / Fit Sheet on One Page / Fit All Columns / Fit All Rows / custom):
/// label and line below.
pub(crate) fn scaling_labels(s: &Strings, scaling: u8) -> (String, String) {
    match scaling {
        1 => (
            tr(s, "sheetBs.fitSheet", "Fit Sheet on One Page"),
            tr(
                s,
                "sheetBs.fitSheetSub",
                "Shrink the printout so that it fits on one page",
            ),
        ),
        2 => (
            tr(s, "sheetBs.fitColumns", "Fit All Columns on One Page"),
            tr(
                s,
                "sheetBs.fitColumnsSub",
                "Shrink the printout so that it is one page wide",
            ),
        ),
        3 => (
            tr(s, "sheetBs.fitRows", "Fit All Rows on One Page"),
            tr(
                s,
                "sheetBs.fitRowsSub",
                "Shrink the printout so that it is one page high",
            ),
        ),
        4 => (
            tr(s, "sheetBs.customScaling", "Custom Scaling"),
            tr(s, "sheetBs.customScalingSub", "Scaling set in Page Setup"),
        ),
        _ => (
            tr(s, "sheetBs.noScaling", "No Scaling"),
            tr(
                s,
                "sheetBs.noScalingSub",
                "Print sheets at their actual size",
            ),
        ),
    }
}

/// Copy path, Open file location (label key, fallback): the Info page's button row.
const INFO_BUTTONS: [(u8, &str, &str); 2] = [
    (0, "sheetBs.copyPath", "Copy path"),
    (1, "sheetBs.openLocation", "Open file location"),
];

/// The File screen's rail: BlueDoc's own accent hue at the lightness and saturation of the
/// reference's rail (#2B579A: L 39 %, S 56 %), calmer than the bright accent of buttons.
fn rail_colour(accent: Rgba) -> Rgba {
    let (r, g, b) = (
        f32::from(accent.0) / 255.0,
        f32::from(accent.1) / 255.0,
        f32::from(accent.2) / 255.0,
    );
    let (max, min) = (r.max(g).max(b), r.min(g).min(b));
    let d = max - min;
    let hue = if d < 1e-6 {
        0.0
    } else if (max - r).abs() < 1e-6 {
        ((g - b) / d).rem_euclid(6.0)
    } else if (max - g).abs() < 1e-6 {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    } * 60.0;
    let (l, sat) = (0.39_f32, 0.56_f32);
    let c = (1.0 - (2.0 * l - 1.0).abs()) * sat;
    let x = c * (1.0 - ((hue / 60.0).rem_euclid(2.0) - 1.0).abs());
    let m = l - c / 2.0;
    let (r1, g1, b1) = match (hue / 60.0) as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let to = |v: f32| ((v + m) * 255.0).round().clamp(0.0, 255.0) as u8;
    Rgba(to(r1), to(g1), to(b1), 255)
}

/// The chosen page's row on the nav column: darker than the column.
const NAV_CHOSEN: Rgba = Rgba(0, 0, 0, 110);
const NAV_HOVER: Rgba = Rgba(255, 255, 255, 36);

fn tr(s: &Strings, key: &str, fallback: &str) -> String {
    let v = s.get(key);
    if v == key {
        fallback.to_owned()
    } else {
        v.to_owned()
    }
}

/// "Good morning" / "Good afternoon" / "Good evening" by the local hour.
fn greeting(s: &Strings) -> String {
    let hour = local_hour();
    if hour < 12 {
        tr(s, "sheetBs.morning", "Good morning")
    } else if hour < 18 {
        tr(s, "sheetBs.afternoon", "Good afternoon")
    } else {
        tr(s, "sheetBs.evening", "Good evening")
    }
}

/// Local hour for the greeting (UTC offset set by the host; UTC without one).
pub(crate) fn local_hour() -> u32 {
    let offset = i64::from(LOCAL_OFFSET.load(std::sync::atomic::Ordering::Relaxed));
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
        + offset * 60;
    ((secs.rem_euclid(86_400)) / 3600) as u32
}

/// UTC offset in minutes used by [`local_hour`].
pub(crate) static LOCAL_OFFSET: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(0);

/// "1-3, 5" → 0-based indices below `n`, in order, without repeats.
#[cfg(test)]
pub(crate) fn parse_ranges(text: &str, n: usize) -> Vec<usize> {
    let mut out = Vec::new();
    for part in text.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        #[allow(clippy::single_match_else)]
        let (a, b) = match part.split_once('-') {
            Some((a, b)) => (
                a.trim().parse::<usize>().ok(),
                b.trim().parse::<usize>().ok(),
            ),
            None => {
                let v = part.parse::<usize>().ok();
                (v, v)
            }
        };
        let (Some(a), Some(b)) = (a, b.or(Some(n))) else {
            continue;
        };
        for pg in a.min(b)..=a.max(b) {
            if pg >= 1 && pg <= n && !out.contains(&(pg - 1)) {
                out.push(pg - 1);
            }
        }
    }
    out
}

// ---------------------------------------------------------------- own icons

fn circle(cx: f32, cy: f32, r: f32) -> Vec<PathSeg> {
    let c = r * 0.552_284_8;
    vec![
        PathSeg::M(cx + r, cy),
        PathSeg::C(cx + r, cy + c, cx + c, cy + r, cx, cy + r),
        PathSeg::C(cx - c, cy + r, cx - r, cy + c, cx - r, cy),
        PathSeg::C(cx - r, cy - c, cx - c, cy - r, cx, cy - r),
        PathSeg::C(cx + c, cy - r, cx + r, cy - c, cx + r, cy),
        PathSeg::Z,
    ]
}

#[derive(Clone, Copy)]
enum NavIcon {
    Home,
    New,
    Open,
}

/// Outline icons of the nav column (own drawings): a house with a door, a ring with a plus,
/// an open folder.
fn nav_icon(p: &mut dyn Painter, i: NavIcon, x: f32, cy: f32, s: f32, c: Rgba, lw: f32) {
    let y = cy - s / 2.0;
    match i {
        NavIcon::Home => p.path(
            &[
                PathSeg::M(x + s * 0.08, y + s * 0.48),
                PathSeg::L(x + s * 0.5, y + s * 0.08),
                PathSeg::L(x + s * 0.92, y + s * 0.48),
                PathSeg::M(x + s * 0.2, y + s * 0.38),
                PathSeg::L(x + s * 0.2, y + s * 0.94),
                PathSeg::L(x + s * 0.8, y + s * 0.94),
                PathSeg::L(x + s * 0.8, y + s * 0.38),
                PathSeg::M(x + s * 0.41, y + s * 0.94),
                PathSeg::L(x + s * 0.41, y + s * 0.66),
                PathSeg::L(x + s * 0.59, y + s * 0.66),
                PathSeg::L(x + s * 0.59, y + s * 0.94),
            ],
            None,
            Some((c, lw)),
        ),
        NavIcon::New => {
            p.path(&circle(x + s / 2.0, cy, s * 0.46), None, Some((c, lw)));
            p.path(
                &[
                    PathSeg::M(x + s * 0.5, y + s * 0.27),
                    PathSeg::L(x + s * 0.5, y + s * 0.73),
                    PathSeg::M(x + s * 0.27, y + s * 0.5),
                    PathSeg::L(x + s * 0.73, y + s * 0.5),
                ],
                None,
                Some((c, lw)),
            );
        }
        NavIcon::Open => p.path(
            &[
                PathSeg::M(x + s * 0.04, y + s * 0.86),
                PathSeg::L(x + s * 0.04, y + s * 0.16),
                PathSeg::L(x + s * 0.36, y + s * 0.16),
                PathSeg::L(x + s * 0.46, y + s * 0.28),
                PathSeg::L(x + s * 0.82, y + s * 0.28),
                PathSeg::L(x + s * 0.82, y + s * 0.44),
                PathSeg::M(x + s * 0.04, y + s * 0.86),
                PathSeg::L(x + s * 0.2, y + s * 0.44),
                PathSeg::L(x + s * 0.98, y + s * 0.44),
                PathSeg::L(x + s * 0.8, y + s * 0.86),
                PathSeg::Z,
            ],
            None,
            Some((c, lw)),
        ),
    }
}

/// A document sheet with a folded corner and a blue band (own drawing).
fn doc_icon(p: &mut dyn Painter, x: f32, y: f32, s: f32) {
    let w = s * 0.78;
    let x = x + (s - w) / 2.0;
    let f = s * 0.24;
    p.path(
        &[
            PathSeg::M(x, y),
            PathSeg::L(x + w - f, y),
            PathSeg::L(x + w, y + f),
            PathSeg::L(x + w, y + s),
            PathSeg::L(x, y + s),
            PathSeg::Z,
        ],
        Some(WHITE),
        Some((Rgba(120, 120, 120, 255), 1.0)),
    );
    p.fill_rect(
        x + 1.0,
        y + s * 0.62,
        w - 1.0,
        s * 0.22,
        Rgba(37, 99, 186, 255),
    );
    for k in 0..3 {
        p.fill_rect(
            x + w * 0.2,
            y + s * (0.26 + 0.1 * k as f32),
            w * 0.6,
            1.0,
            Rgba(150, 150, 150, 255),
        );
    }
}

/// A yellow folder (own drawing).
fn folder_icon(p: &mut dyn Painter, x: f32, y: f32, s: f32) {
    let back = Rgba(222, 178, 82, 255);
    let front = Rgba(244, 205, 112, 255);
    p.path(
        &[
            PathSeg::M(x, y + s * 0.18),
            PathSeg::L(x + s * 0.38, y + s * 0.18),
            PathSeg::L(x + s * 0.48, y + s * 0.3),
            PathSeg::L(x + s, y + s * 0.3),
            PathSeg::L(x + s, y + s * 0.86),
            PathSeg::L(x, y + s * 0.86),
            PathSeg::Z,
        ],
        Some(back),
        None,
    );
    p.fill_rect(x, y + s * 0.38, s, s * 0.48, front);
}

/// Two chain rings (own drawing).
fn link_icon(p: &mut dyn Painter, x: f32, y: f32, s: f32, c: Rgba) {
    let lw = (s / 12.0).max(1.0);
    for dx in [0.0, s * 0.42] {
        p.path(
            &[
                PathSeg::M(x + dx + s * 0.18, y + s * 0.32),
                PathSeg::L(x + dx + s * 0.42, y + s * 0.32),
                PathSeg::C(
                    x + dx + s * 0.6,
                    y + s * 0.32,
                    x + dx + s * 0.6,
                    y + s * 0.68,
                    x + dx + s * 0.42,
                    y + s * 0.68,
                ),
                PathSeg::L(x + dx + s * 0.18, y + s * 0.68),
                PathSeg::C(
                    x + dx,
                    y + s * 0.68,
                    x + dx,
                    y + s * 0.32,
                    x + dx + s * 0.18,
                    y + s * 0.32,
                ),
                PathSeg::Z,
            ],
            None,
            Some((c, lw)),
        );
    }
}

/// A padlock with a key mark in the accent (own drawing).
fn lock_icon(p: &mut dyn Painter, x: f32, y: f32, s: f32, accent: Rgba) {
    let lw = (s / 14.0).max(1.0);
    p.path(
        &[
            PathSeg::M(x + s * 0.28, y + s * 0.45),
            PathSeg::L(x + s * 0.28, y + s * 0.28),
            PathSeg::C(x + s * 0.28, y, x + s * 0.72, y, x + s * 0.72, y + s * 0.28),
            PathSeg::L(x + s * 0.72, y + s * 0.45),
        ],
        None,
        Some((Rgba(110, 110, 110, 255), lw * 1.4)),
    );
    p.fill_rect(
        x + s * 0.16,
        y + s * 0.44,
        s * 0.68,
        s * 0.5,
        Rgba(232, 176, 60, 255),
    );
    p.path(
        &circle(x + s * 0.5, y + s * 0.62, s * 0.07),
        Some(accent),
        None,
    );
    p.fill_rect(x + s * 0.47, y + s * 0.64, s * 0.06, s * 0.16, accent);
}

/// A page with a check mark and a magnifier (own drawing).
fn inspect_icon(p: &mut dyn Painter, x: f32, y: f32, s: f32, accent: Rgba) {
    let lw = (s / 16.0).max(1.0);
    p.fill_rect(
        x + s * 0.18,
        y + s * 0.04,
        s * 0.56,
        s * 0.74,
        Rgba(120, 120, 120, 255),
    );
    p.fill_rect(
        x + s * 0.18 + 1.0,
        y + s * 0.04 + 1.0,
        s * 0.56 - 2.0,
        s * 0.74 - 2.0,
        WHITE,
    );
    p.path(
        &[
            PathSeg::M(x + s * 0.28, y + s * 0.34),
            PathSeg::L(x + s * 0.38, y + s * 0.44),
            PathSeg::L(x + s * 0.58, y + s * 0.2),
        ],
        None,
        Some((Rgba(204, 64, 52, 255), lw * 1.6)),
    );
    p.path(
        &circle(x + s * 0.64, y + s * 0.66, s * 0.16),
        Some(WHITE),
        Some((accent, lw * 1.5)),
    );
    p.path(
        &[
            PathSeg::M(x + s * 0.76, y + s * 0.78),
            PathSeg::L(x + s * 0.94, y + s * 0.96),
        ],
        None,
        Some((accent, lw * 2.2)),
    );
}

/// Stacked pages with a clock (own drawing).
fn manage_icon(p: &mut dyn Painter, x: f32, y: f32, s: f32, accent: Rgba) {
    for (k, c) in [
        (0.0, Rgba(232, 176, 60, 255)),
        (0.1, Rgba(244, 205, 112, 255)),
    ] {
        p.fill_rect(x + s * (0.1 + k), y + s * (0.06 + k), s * 0.5, s * 0.66, c);
    }
    let lw = (s / 16.0).max(1.0);
    p.path(
        &circle(x + s * 0.64, y + s * 0.64, s * 0.26),
        Some(WHITE),
        Some((accent, lw * 1.4)),
    );
    p.path(
        &[
            PathSeg::M(x + s * 0.64, y + s * 0.5),
            PathSeg::L(x + s * 0.64, y + s * 0.64),
            PathSeg::L(x + s * 0.75, y + s * 0.7),
        ],
        None,
        Some((accent, lw * 1.4)),
    );
}

/// A printer with paper (own drawing).
fn printer_icon(p: &mut dyn Painter, x: f32, y: f32, s: f32, accent: Rgba) {
    p.fill_rect(
        x + s * 0.24,
        y + s * 0.06,
        s * 0.52,
        s * 0.3,
        Rgba(120, 120, 120, 255),
    );
    p.fill_rect(
        x + s * 0.24 + 1.0,
        y + s * 0.06 + 1.0,
        s * 0.52 - 2.0,
        s * 0.3,
        WHITE,
    );
    p.fill_rect(
        x + s * 0.04,
        y + s * 0.34,
        s * 0.92,
        s * 0.38,
        Rgba(108, 114, 124, 255),
    );
    p.fill_rect(x + s * 0.76, y + s * 0.42, s * 0.1, s * 0.06, accent);
    p.fill_rect(
        x + s * 0.22,
        y + s * 0.6,
        s * 0.56,
        s * 0.34,
        Rgba(120, 120, 120, 255),
    );
    p.fill_rect(
        x + s * 0.22 + 1.0,
        y + s * 0.6,
        s * 0.56 - 2.0,
        s * 0.34 - 1.0,
        WHITE,
    );
    for k in 0..2 {
        p.fill_rect(
            x + s * 0.3,
            y + s * (0.72 + 0.1 * k as f32),
            s * 0.4,
            1.0,
            Rgba(150, 150, 150, 255),
        );
    }
}

/// A computer screen (own drawing).
fn pc_icon(p: &mut dyn Painter, x: f32, y: f32, s: f32, c: Rgba) {
    let lw = (s / 16.0).max(1.0);
    p.path(
        &[
            PathSeg::M(x + s * 0.06, y + s * 0.12),
            PathSeg::L(x + s * 0.94, y + s * 0.12),
            PathSeg::L(x + s * 0.94, y + s * 0.7),
            PathSeg::L(x + s * 0.06, y + s * 0.7),
            PathSeg::Z,
            PathSeg::M(x + s * 0.5, y + s * 0.7),
            PathSeg::L(x + s * 0.5, y + s * 0.86),
            PathSeg::M(x + s * 0.3, y + s * 0.88),
            PathSeg::L(x + s * 0.7, y + s * 0.88),
        ],
        None,
        Some((c, lw)),
    );
}

/// A clock face (own drawing): Recent.
fn clock_icon(p: &mut dyn Painter, x: f32, y: f32, s: f32, c: Rgba) {
    let lw = (s / 16.0).max(1.0);
    p.path(
        &circle(x + s / 2.0, y + s / 2.0, s * 0.42),
        None,
        Some((c, lw)),
    );
    p.path(
        &[
            PathSeg::M(x + s * 0.5, y + s * 0.24),
            PathSeg::L(x + s * 0.5, y + s * 0.5),
            PathSeg::L(x + s * 0.68, y + s * 0.6),
        ],
        None,
        Some((c, lw)),
    );
}

/// A page with a red band carrying "PDF"-like bars (own drawing).
fn pdf_icon(p: &mut dyn Painter, x: f32, y: f32, s: f32, t: &Theme) {
    doc_icon(p, x, y, s);
    let w = s * 0.78;
    let x0 = x + (s - w) / 2.0;
    p.fill_rect(
        x0 + 1.0,
        y + s * 0.62,
        w - 1.0,
        s * 0.22,
        Rgba(196, 43, 28, 255),
    );
    for k in 0..3 {
        p.fill_rect(
            x0 + w * (0.14 + 0.26 * k as f32),
            y + s * 0.7,
            w * 0.18,
            (s * 0.06).max(1.0),
            WHITE,
        );
    }
    let _ = t;
}

/// A floppy-like save mark with a pencil (own drawing): Change File Type.
fn save_type_icon(p: &mut dyn Painter, x: f32, y: f32, s: f32, t: &Theme) {
    p.fill_rect(
        x + s * 0.06,
        y + s * 0.06,
        s * 0.74,
        s * 0.74,
        t.accent_strong,
    );
    p.fill_rect(x + s * 0.22, y + s * 0.06, s * 0.42, s * 0.24, WHITE);
    p.fill_rect(
        x + s * 0.2,
        y + s * 0.5,
        s * 0.46,
        s * 0.3,
        Rgba(230, 236, 246, 255),
    );
    p.path(
        &[
            PathSeg::M(x + s * 0.5, y + s * 0.94),
            PathSeg::L(x + s * 0.56, y + s * 0.78),
            PathSeg::L(x + s * 0.86, y + s * 0.48),
            PathSeg::L(x + s * 0.96, y + s * 0.58),
            PathSeg::L(x + s * 0.66, y + s * 0.88),
            PathSeg::Z,
        ],
        Some(Rgba(232, 176, 60, 255)),
        Some((Rgba(90, 90, 90, 255), 1.0)),
    );
}

/// Icons of the Print page's settings (own drawings, kept simple): printer, pages, sides,
/// collation, orientation, paper, margins, pages per sheet.
fn setting_icon(p: &mut dyn Painter, i: u8, x: f32, y: f32, s: f32, t: &Theme) {
    let grey = Rgba(120, 120, 120, 255);
    let sheet = |p: &mut dyn Painter, x: f32, y: f32, w: f32, h: f32| {
        p.fill_rect(x, y, w, h, grey);
        p.fill_rect(x + 1.0, y + 1.0, w - 2.0, h - 2.0, WHITE);
    };
    let lines = |p: &mut dyn Painter, x: f32, y: f32, w: f32, n: usize| {
        for k in 0..n {
            p.fill_rect(x, y + s * 0.1 * k as f32, w, 1.0, Rgba(160, 160, 160, 255));
        }
    };
    match i {
        0 => printer_icon(p, x, y, s, t.accent_strong),
        1 => {
            sheet(p, x + s * 0.32, y + s * 0.02, s * 0.56, s * 0.72);
            sheet(p, x + s * 0.16, y + s * 0.14, s * 0.56, s * 0.72);
            lines(p, x + s * 0.24, y + s * 0.3, s * 0.4, 5);
        }
        2 => {
            sheet(p, x + s * 0.2, y + s * 0.06, s * 0.6, s * 0.86);
            lines(p, x + s * 0.28, y + s * 0.2, s * 0.44, 6);
        }
        3 => {
            for k in 0..3 {
                let dx = s * 0.3 * k as f32;
                sheet(p, x + dx + s * 0.06, y + s * 0.2, s * 0.24, s * 0.34);
                sheet(p, x + dx, y + s * 0.26, s * 0.24, s * 0.34);
            }
        }
        4 => {
            sheet(p, x + s * 0.22, y + s * 0.04, s * 0.56, s * 0.86);
            lines(p, x + s * 0.3, y + s * 0.18, s * 0.4, 6);
        }
        5 => sheet(p, x + s * 0.26, y + s * 0.06, s * 0.48, s * 0.84),
        6 => {
            sheet(p, x + s * 0.18, y + s * 0.04, s * 0.64, s * 0.88);
            let a = t.accent_strong;
            p.fill_rect(x + s * 0.3, y + s * 0.04, 1.0, s * 0.88, a);
            p.fill_rect(x + s * 0.7, y + s * 0.04, 1.0, s * 0.88, a);
            p.fill_rect(x + s * 0.18, y + s * 0.16, s * 0.64, 1.0, a);
            p.fill_rect(x + s * 0.18, y + s * 0.8, s * 0.64, 1.0, a);
        }
        _ => {
            sheet(p, x + s * 0.18, y + s * 0.04, s * 0.64, s * 0.88);
            lines(p, x + s * 0.26, y + s * 0.16, s * 0.48, 7);
        }
    }
}

/// The status mark on the printer icon: a green check when ready, an amber "!" otherwise.
fn status_badge(p: &mut dyn Painter, cx: f32, cy: f32, r: f32, trouble: bool) {
    let fill = if trouble {
        Rgba(234, 157, 22, 255)
    } else {
        Rgba(16, 124, 16, 255)
    };
    p.path(&circle(cx, cy, r + 1.0), Some(WHITE), None);
    p.path(&circle(cx, cy, r), Some(fill), None);
    let lw = (r / 4.0).max(1.0);
    if trouble {
        p.fill_rect(cx - lw / 2.0, cy - r * 0.55, lw, r * 0.7, WHITE);
        p.fill_rect(cx - lw / 2.0, cy + r * 0.3, lw, lw, WHITE);
    } else {
        p.path(
            &[
                PathSeg::M(cx - r * 0.45, cy),
                PathSeg::L(cx - r * 0.1, cy + r * 0.35),
                PathSeg::L(cx + r * 0.45, cy - r * 0.35),
            ],
            None,
            Some((WHITE, lw * 1.2)),
        );
    }
}

/// A small ring with an "i" (our own info mark).
fn info_mark(p: &mut dyn Painter, cx: f32, cy: f32, r: f32, c: Rgba) {
    let lw = (r / 7.0).max(1.0);
    p.path(&circle(cx, cy, r), None, Some((c, lw)));
    p.fill_rect(
        (cx - lw / 2.0).round(),
        (cy - r * 0.55).round(),
        lw.max(1.0),
        lw.max(1.0),
        c,
    );
    p.fill_rect(
        (cx - lw / 2.0).round(),
        (cy - r * 0.2).round(),
        lw.max(1.0),
        (r * 0.75).round(),
        c,
    );
}

/// An envelope (own drawing).
fn mail_icon(p: &mut dyn Painter, x: f32, y: f32, s: f32, accent: Rgba) {
    let (y0, h) = (y + s * 0.18, s * 0.62);
    p.fill_rect(x + s * 0.04, y0, s * 0.92, h, Rgba(120, 120, 120, 255));
    p.fill_rect(x + s * 0.04 + 1.0, y0 + 1.0, s * 0.92 - 2.0, h - 2.0, WHITE);
    p.path(
        &[
            PathSeg::M(x + s * 0.06, y0 + 1.0),
            PathSeg::L(x + s * 0.5, y0 + h * 0.6),
            PathSeg::L(x + s * 0.94, y0 + 1.0),
        ],
        None,
        Some((accent, (s / 16.0).max(1.0))),
    );
}

/// A white edit box framed in grey.
fn field(p: &mut dyn Painter, r: [f32; 4], scale: f32) {
    let _ = scale;
    p.fill_rect(r[0], r[1], r[2], r[3], Rgba(122, 122, 122, 255));
    p.fill_rect(r[0] + 1.0, r[1] + 1.0, r[2] - 2.0, r[3] - 2.0, WHITE);
}

fn chevron(p: &mut dyn Painter, cx: f32, cy: f32, k: f32, c: Rgba) {
    p.path(
        &[
            PathSeg::M(cx - k, cy - k / 2.0),
            PathSeg::L(cx, cy + k / 2.0),
            PathSeg::L(cx + k, cy - k / 2.0),
            PathSeg::Z,
        ],
        Some(c),
        None,
    );
}

fn chevron_up(p: &mut dyn Painter, cx: f32, cy: f32, k: f32, c: Rgba) {
    p.path(
        &[
            PathSeg::M(cx - k, cy + k / 2.0),
            PathSeg::L(cx, cy - k / 2.0),
            PathSeg::L(cx + k, cy + k / 2.0),
            PathSeg::Z,
        ],
        Some(c),
        None,
    );
}

fn chevron_left(p: &mut dyn Painter, cx: f32, cy: f32, k: f32, c: Rgba) {
    p.path(
        &[
            PathSeg::M(cx + k / 2.0, cy - k),
            PathSeg::L(cx - k / 2.0, cy),
            PathSeg::L(cx + k / 2.0, cy + k),
            PathSeg::Z,
        ],
        Some(c),
        None,
    );
}

fn chevron_right(p: &mut dyn Painter, cx: f32, cy: f32, k: f32, c: Rgba) {
    p.path(
        &[
            PathSeg::M(cx - k / 2.0, cy - k),
            PathSeg::L(cx + k / 2.0, cy),
            PathSeg::L(cx - k / 2.0, cy + k),
            PathSeg::Z,
        ],
        Some(c),
        None,
    );
}

/// Own drawings of the preview's corner buttons: Show Margins (a page with its margin guides)
/// and Zoom to Page (a page in a frame with corner ticks).
fn corner_icon(p: &mut dyn Painter, zoom: bool, b: [f32; 4], scale: f32) {
    let k = scale.max(1.0);
    let ink = Rgba(96, 96, 96, 255);
    let (cx, cy) = (b[0] + b[2] / 2.0, b[1] + b[3] / 2.0);
    let (w, h) = ((12.0 * k).round(), (16.0 * k).round());
    let (x, y) = ((cx - w / 2.0).round(), (cy - h / 2.0).round());
    let line = k.floor().max(1.0);
    if zoom {
        // Frame corners around a smaller page.
        let s = (4.0 * k).round();
        let (fx, fy, fw, fh) = (x - 3.0 * k, y - 2.0 * k, w + 6.0 * k, h + 4.0 * k);
        for (ax, ay, dx, dy) in [
            (fx, fy, 1.0, 1.0),
            (fx + fw, fy, -1.0, 1.0),
            (fx, fy + fh, 1.0, -1.0),
            (fx + fw, fy + fh, -1.0, -1.0),
        ] {
            p.fill_rect(ax.min(ax + dx * s), ay, s, line, ink);
            p.fill_rect(ax, ay.min(ay + dy * s), line, s, ink);
        }
        let (px, py, pw, ph) = (x + 2.0 * k, y + 2.0 * k, w - 4.0 * k, h - 4.0 * k);
        p.fill_rect(px, py, pw, ph, ink);
        p.fill_rect(
            px + line,
            py + line,
            pw - 2.0 * line,
            ph - 2.0 * line,
            WHITE,
        );
    } else {
        p.fill_rect(x, y, w, h, ink);
        p.fill_rect(x + line, y + line, w - 2.0 * line, h - 2.0 * line, WHITE);
        // Margin guides: two dotted verticals and two dotted horizontals.
        let guide = Rgba(150, 150, 150, 255);
        let mut t = y + line;
        while t < y + h - line {
            p.fill_rect(x + 3.0 * k, t, line, line, guide);
            p.fill_rect(x + w - 3.0 * k - line, t, line, line, guide);
            t += 2.0 * line;
        }
        let mut l = x + line;
        while l < x + w - line {
            p.fill_rect(l, y + 3.0 * k, line, line, guide);
            p.fill_rect(l, y + h - 3.0 * k - line, line, line, guide);
            l += 2.0 * line;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts() -> BookFacts {
        BookFacts {
            name: "a.xlsx".into(),
            path: Some(r"C:\books\a.xlsx".into()),
            pages: 3,
            page_sizes: vec![(595.0, 842.0); 3],
            ..BookFacts::default()
        }
    }

    fn centre(b: &SheetBackstage, h: Hit) -> (f32, f32) {
        let r = b.rect_of(h).expect("region");
        (r[0] + r[2] / 2.0, r[1] + r[3] / 2.0)
    }

    #[test]
    fn nav_switches_pages_and_actions() {
        let mut b = SheetBackstage::new(BsPage::Info, facts());
        b.layout(40.0, (1200.0, 880.0), 1.0);
        let (x, y) = centre(&b, Hit::Nav(Nav::Page(BsPage::Print)));
        assert_eq!(b.press(x, y), BsOut::Redraw);
        assert_eq!(b.page, BsPage::Print);
        let (x, y) = centre(&b, Hit::Nav(Nav::Save));
        assert_eq!(b.press(x, y), BsOut::Save);
        let (x, y) = centre(&b, Hit::Nav(Nav::Options));
        assert_eq!(b.press(x, y), BsOut::Options);
        let (x, y) = centre(&b, Hit::Back);
        assert_eq!(b.press(x, y), BsOut::Back);
    }

    #[test]
    fn print_page_copies_pages_from_to_and_preview() {
        let mut b = SheetBackstage::new(BsPage::Print, facts());
        b.layout(40.0, (1200.0, 880.0), 1.0);
        let (x, y) = centre(&b, Hit::CopiesUp);
        b.press(x, y);
        b.press(x, y);
        assert_eq!(b.print.copies, 3);
        assert_eq!(b.pages_to_print(), vec![0, 1, 2]);
        let (x, y) = centre(&b, Hit::PagesBox);
        b.press(x, y);
        assert!(b.type_text("2x"));
        assert_eq!(b.print.pages, "2");
        let (x, y) = centre(&b, Hit::PagesTo);
        b.press(x, y);
        assert!(b.type_text("9"));
        // The last page bounds the range.
        assert_eq!(b.pages_to_print(), vec![1, 2]);
        assert!(b.key(crate::Key::Backspace));
        assert!(b.type_text("2"));
        assert_eq!(b.pages_to_print(), vec![1]);
        assert!(b.key(crate::Key::PageDown));
        assert_eq!(b.preview, 1);
        let (_, r) = b.preview_rect().expect("preview");
        let a = b.preview_area();
        assert!(r[0] >= a[0] && r[0] + r[2] <= a[0] + a[2]);
        // A4 keeps its shape.
        assert!(((r[2] / r[3]) - 595.0 / 842.0).abs() < 0.01);
        let (x, y) = centre(&b, Hit::PrintSetting(7));
        assert!(matches!(b.press(x, y), BsOut::Menu("bs13.scaling", _)));
        let (x, y) = centre(&b, Hit::PrintSetting(1));
        assert!(matches!(b.press(x, y), BsOut::Menu("bs13.range", _)));
        // Show Margins and Zoom to Page toggle.
        let (x, y) = centre(&b, Hit::ZoomOut);
        b.press(x, y);
        assert!(b.show_margins);
        let (x, y) = centre(&b, Hit::ZoomFit);
        b.press(x, y);
        assert!(b.zoom > 1.0);
    }

    #[test]
    fn export_and_templates() {
        let mut b = SheetBackstage::new(BsPage::Export, facts());
        b.layout(40.0, (1200.0, 880.0), 1.0);
        let (x, y) = centre(&b, Hit::Action(0));
        assert_eq!(b.press(x, y), BsOut::ExportPdf);
        let (x, y) = centre(&b, Hit::ExportItem(1));
        b.press(x, y);
        let (x, y) = centre(&b, Hit::TypeRow(1));
        b.press(x, y);
        let (x, y) = centre(&b, Hit::Action(1));
        assert_eq!(b.press(x, y), BsOut::SaveAsType("csv"));
        b.show(BsPage::New);
        let (x, y) = centre(&b, Hit::Template(Some(1)));
        assert_eq!(b.press(x, y), BsOut::New(Some("invoice")));
        let (x, y) = centre(&b, Hit::Template(None));
        assert_eq!(b.press(x, y), BsOut::New(None));
    }

    #[test]
    fn open_page_lists_the_apps_own_recent_files() {
        let mut b = SheetBackstage::new(BsPage::Open, facts());
        b.recent = vec![Recent {
            path: r"D:\x\b.xlsx".into(),
            when: String::new(),
        }];
        b.layout(40.0, (1200.0, 880.0), 1.0);
        let (x, y) = centre(&b, Hit::RecentRow(0));
        assert_eq!(b.press(x, y), BsOut::OpenPath(r"D:\x\b.xlsx".into()));
        let (x, y) = centre(&b, Hit::Place(1));
        assert_eq!(b.press(x, y), BsOut::BrowseOpen);
    }

    #[test]
    fn ranges() {
        assert_eq!(parse_ranges("2-1, 5, 9", 6), vec![0, 1, 4]);
        assert_eq!(parse_ranges("4-", 6), vec![3, 4, 5]);
        assert_eq!(parse_ranges("x", 6), Vec::<usize>::new());
    }
}
