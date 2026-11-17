//! BlueDoc's File screen (parity round 6, `docs/ui/REPORT.md` rounds 88+): the navigation column
//! (Home, New, Open | Info, Save, Save As, Print, Export, Close | Options) and one page per
//! entry, laid out after the reference word processor's File screen measured at 125 %
//! (positions below are logical pixels: reference pixels / 1.25).
//!
//! * Home: a greeting, the New row (blank document and BlueOffice's own templates) and the
//!   app's own recent documents;
//! * New: the blank document and the template gallery (own designs drawn here);
//! * Open: places (Recent, Browse) and the recent list kept by BlueDoc itself;
//! * Info: path, Copy path / Open file location, Protect / Inspect / Manage tiles with menus,
//!   the properties column (size, pages, words, editing time, title, dates, people);
//! * Save As: This PC (current folder, name, type) and Browse;
//! * Print: Print button with copies, printer and settings drop-downs, a preview of the
//!   pages with page navigation;
//! * Export: Create PDF/XPS Document and Change File Type.
//!
//! The nav column keeps BlueDoc's own accent colour (the reference's brand colour is not
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
    Export,
}

/// Navigation rows: (entry, label key, fallback).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Nav {
    Page(BsPage),
    Save,
    Close,
    Options,
}

const NAV_TOP: [(Nav, &str, &str); 3] = [
    (Nav::Page(BsPage::Home), "docR6.bs.home", "Home"),
    (Nav::Page(BsPage::New), "backstage.nav.new", "New"),
    (Nav::Page(BsPage::Open), "backstage.nav.open", "Open"),
];
const NAV_MID: [(Nav, &str, &str); 6] = [
    (Nav::Page(BsPage::Info), "backstage.nav.info", "Info"),
    (Nav::Save, "backstage.nav.save", "Save"),
    (Nav::Page(BsPage::SaveAs), "backstage.nav.saveAs", "Save As"),
    (Nav::Page(BsPage::Print), "backstage.nav.print", "Print"),
    (Nav::Page(BsPage::Export), "docR6.bs.export", "Export"),
    (Nav::Close, "backstage.nav.close", "Close"),
];

/// BlueOffice's own document templates: (id, label key, fallback).
pub(crate) const TEMPLATES: [(&str, &str, &str); 5] = [
    ("letter", "docR6.tpl.letter", "Official letter"),
    ("report", "docR6.tpl.report", "Report"),
    ("minutes", "docR6.tpl.minutes", "Meeting minutes"),
    ("resume", "docR6.tpl.resume", "Resume"),
    ("memo", "docR6.tpl.memo", "Memo"),
];

/// File types of Export › Change File Type: (extension, label key, fallback).
pub(crate) const FILE_TYPES: [(&str, &str, &str); 7] = [
    ("docx", "docR6.type.docx", "Document (*.docx)"),
    ("dotx", "docR6.type.dotx", "Template (*.dotx)"),
    ("pdf", "docR6.type.pdf", "PDF (*.pdf)"),
    ("html", "docR6.type.html", "Web Page (*.html)"),
    ("mht", "docR6.type.mht", "Single File Web Page (*.mht)"),
    ("xml", "docR6.type.xml", "XML Document (*.xml)"),
    ("epub", "docR6.type.epub", "EPUB (*.epub)"),
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
    /// 0 all pages, 1 current page, 2 custom range.
    pub(crate) range: u8,
    /// Custom pages ("1-3, 5").
    pub(crate) pages: String,
    /// 0 one sided, 1 both sides (long edge), 2 both sides (short edge).
    pub(crate) sides: u8,
    pub(crate) collated: bool,
    /// Pages per sheet (1, 2, 4, 6, 8, 16).
    pub(crate) per_sheet: u8,
}

impl Default for PrintSettings {
    fn default() -> Self {
        Self {
            copies: 1,
            printers: Vec::new(),
            printer: 0,
            range: 0,
            pages: String::new(),
            sides: 0,
            collated: true,
            per_sheet: 1,
        }
    }
}

/// Facts the pages show about the open document.
#[derive(Debug, Clone, Default)]
pub(crate) struct DocFacts {
    pub(crate) name: String,
    pub(crate) path: Option<String>,
    pub(crate) bytes: Option<u64>,
    pub(crate) pages: usize,
    pub(crate) words: usize,
    pub(crate) modified: bool,
    pub(crate) title: String,
    pub(crate) tags: String,
    pub(crate) comments: String,
    pub(crate) author: String,
    pub(crate) last_modified_by: String,
    pub(crate) created: String,
    pub(crate) last_modified: String,
    /// `docProps/app.xml` TotalTime (minutes).
    pub(crate) editing_minutes: Option<u64>,
    /// Restrict Editing is enforced.
    pub(crate) protected: bool,
    /// Orientation (landscape), paper (name, width × height in inches), margins name of the
    /// section at the caret, for the Print settings.
    pub(crate) landscape: bool,
    pub(crate) paper: (String, f32, f32),
    pub(crate) margins: String,
    /// Page sizes in points (preview shape).
    pub(crate) page_sizes: Vec<(f32, f32)>,
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
}

/// The open File screen.
pub(crate) struct DocBackstage {
    pub(crate) page: BsPage,
    top: f32,
    size: (f32, f32),
    scale: f32,
    hover: Option<Hit>,
    pub(crate) facts: DocFacts,
    pub(crate) recent: Vec<Recent>,
    pub(crate) print: PrintSettings,
    /// Page shown in the print preview.
    pub(crate) preview: usize,
    /// Export: 0 PDF/XPS, 1 Change File Type; chosen type.
    pub(crate) export: u8,
    pub(crate) file_type: usize,
    /// Open / Save As: chosen place (0 Recent / This PC, 1 Browse).
    place: u8,
    /// Typing in the Pages box of the Print page.
    pub(crate) pages_focus: bool,
    /// Measured widths of the Info page's buttons and of the Save label.
    button_w: [f32; 3],
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

impl DocBackstage {
    pub(crate) fn new(page: BsPage, facts: DocFacts) -> Self {
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
            button_w: [0.0; 3],
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
            "docR6.bs.options",
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
                for (i, ry) in self.setting_tops().into_iter().enumerate() {
                    r.push((
                        Hit::PrintSetting(i as u8),
                        [cx + self.px(4.0), ry, self.px(293.0), self.px(51.0)],
                    ));
                }
                r.push((Hit::PagesBox, self.pages_box()));
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
        let y = self.top + self.px(222.0);
        [y, y + self.px(134.0), y + self.px(287.0)]
    }

    fn setting_tops(&self) -> Vec<f32> {
        let y = self.top + self.px(262.0);
        // Printer, then Settings: range, (Pages box), sides, collated, orientation, paper,
        // margins, pages per sheet.
        let mut v = vec![y];
        let s = y + self.px(122.0);
        v.push(s);
        let mut yy = s + self.px(51.0) + self.px(38.0);
        for _ in 0..6 {
            v.push(yy);
            yy += self.px(55.0);
        }
        v
    }

    fn pages_box(&self) -> [f32; 4] {
        let cx = self.content_x();
        let y = self.top + self.px(262.0 + 122.0 + 51.0 + 5.0);
        [cx + self.px(52.0), y, self.px(222.0), self.px(28.0)]
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
        let k = ((w - 2.0 * pad) / pw.max(1.0)).min((h - 2.0 * pad) / ph.max(1.0));
        let (dw, dh) = ((pw * k).round(), (ph * k).round());
        Some((
            i,
            [
                (x + (w - dw) / 2.0).round(),
                (y + (h - dh) / 2.0).round(),
                dw,
                dh,
            ],
        ))
    }

    fn hit(&self, x: f32, y: f32) -> Option<Hit> {
        self.hits
            .iter()
            .rev()
            .find(|(_, r)| x >= r[0] && x < r[0] + r[2] && y >= r[1] && y < r[1] + r[3])
            .map(|(h, _)| *h)
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
            if self.pages_focus {
                self.pages_focus = false;
                return BsOut::Redraw;
            }
            return BsOut::None;
        };
        self.pages_focus = h == Hit::PagesBox;
        match h {
            Hit::Back => BsOut::Back,
            Hit::Nav(Nav::Page(p)) => {
                self.show(p);
                BsOut::Redraw
            }
            Hit::Nav(Nav::Save) => BsOut::Save,
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
                    BsOut::SaveAsType("docx")
                }
            }
            Hit::Place(_) => {
                self.place = 0;
                BsOut::Redraw
            }
            Hit::InfoButton(0) => BsOut::CopyPath,
            Hit::InfoButton(_) => BsOut::OpenLocation,
            Hit::Tile(i) => {
                let id = ["bs6.protect", "bs6.inspect", "bs6.manage"][usize::from(i.min(2))];
                BsOut::Menu(id, self.rect_of(h).unwrap_or_default())
            }
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
                (BsPage::Print, _) => BsOut::Print,
                (BsPage::Export, 0) => BsOut::ExportPdf,
                (BsPage::Export, _) | (BsPage::SaveAs, 1) => {
                    BsOut::SaveAsType(FILE_TYPES[self.file_type].0)
                }
                (BsPage::SaveAs, _) => {
                    BsOut::Menu("bs6.saveType", self.rect_of(h).unwrap_or_default())
                }
                _ => BsOut::None,
            },
            Hit::PrintSetting(i) => {
                let id = [
                    "bs6.printer",
                    "bs6.range",
                    "bs6.sides",
                    "bs6.collate",
                    "bs6.orientation",
                    "bs6.paper",
                    "bs6.margins",
                    "bs6.perSheet",
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
            Hit::PagesBox => {
                self.print.range = 2;
                BsOut::Redraw
            }
        }
    }

    /// Typing (Pages box of the Print page).
    pub(crate) fn type_text(&mut self, s: &str) -> bool {
        if !self.pages_focus {
            return false;
        }
        self.print.pages.extend(
            s.chars()
                .filter(|c| c.is_ascii_digit() || matches!(c, ',' | '-' | ' ')),
        );
        self.print.range = 2;
        true
    }

    /// Backspace in the Pages box; Page Up / Down turn the preview.
    pub(crate) fn key(&mut self, key: crate::Key) -> bool {
        match key {
            crate::Key::Backspace if self.pages_focus => self.print.pages.pop().is_some(),
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
        self.hits = self.regions();
    }

    /// Pages the Print page prints (1-based ranges resolved, 0-based indices).
    pub(crate) fn pages_to_print(&self) -> Vec<usize> {
        let n = self.facts.page_sizes.len().max(self.facts.pages);
        match self.print.range {
            1 => vec![self.preview.min(n.saturating_sub(1))],
            2 => parse_ranges(&self.print.pages, n),
            _ => (0..n).collect(),
        }
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
            BsPage::Export => tr(s, "docR6.bs.export", "Export"),
        };
        let light = Font {
            family: "Segoe UI Light",
            size: self.px(40.0),
            weight: 300,
            italic: false,
            underline: false,
        };
        p.text_font(&title, cx, self.top + self.px(8.0), &light, INK);
        match self.page {
            BsPage::Home => self.paint_home(p, t, s),
            BsPage::New => self.paint_new(p, t, s),
            BsPage::Open | BsPage::SaveAs => self.paint_places(p, t, s),
            BsPage::Info => self.paint_info(p, t, s),
            BsPage::Print => self.paint_print(p, t, s),
            BsPage::Export => self.paint_export(p, t, s),
        }
    }

    fn paint_nav(&self, p: &mut dyn Painter, t: &Theme, s: &Strings) {
        let (_, h) = self.size;
        let nw = self.px(NAV_W);
        p.fill_rect(0.0, self.top, nw, h - self.top, t.accent_strong);
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
            } else if self.hover == Some(Hit::Nav(*n)) {
                p.fill_rect(rect[0], rect[1], rect[2], rect[3], NAV_HOVER);
            }
            let ty = rect[1] + (rect[3] - style.size * 1.33) / 2.0;
            p.text(&tr(s, key, fb), self.px(55.0), ty.round(), style, on);
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
                tr(s, "docR6.bs.blank", "Blank document")
            } else {
                let (_, k, f) = TEMPLATES[i - 1];
                tr(s, k, f)
            };
            self.heading(p, &label, r[0], r[1] + r[3] + self.px(6.0), 12.5, INK);
        }
        let ly = y + self.px(232.0);
        self.heading(
            p,
            &tr(s, "docR6.bs.recent", "Recent"),
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
                    "docR6.bs.noRecent",
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
                    None => tr(s, "docR6.bs.blank", "Blank document"),
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
            &tr(s, "docR6.bs.templates", "BlueOffice templates"),
            cx,
            y + self.px(14.0),
            13.0,
            MUTED,
        );
    }

    /// Our own template thumbnails: a white sheet with the template's layout in blocks.
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
        let Some(i) = tpl else {
            return;
        };
        let (x, y, w, h) = (r[0], r[1], r[2], r[3]);
        let line = |p: &mut dyn Painter, lx: f32, ly: f32, lw: f32, c: Rgba| {
            p.fill_rect(
                lx.round(),
                ly.round(),
                lw.round(),
                (h * 0.012).max(1.0).round(),
                c,
            );
        };
        let grey = Rgba(190, 190, 190, 255);
        let dark = Rgba(90, 90, 90, 255);
        let accent = t.accent_strong;
        let body = |p: &mut dyn Painter, from: f32, to: f32, left: f32, right: f32| {
            let mut yy = from;
            let mut n = 0;
            while yy < to {
                let short = if n % 5 == 4 { 0.55 } else { 1.0 };
                line(
                    p,
                    x + w * left,
                    y + h * yy,
                    w * (1.0 - left - right) * short,
                    grey,
                );
                yy += 0.032;
                n += 1;
            }
        };
        match TEMPLATES[i].0 {
            "letter" => {
                // Two header blocks (agency left, national motto right), number, title,
                // body, signature block on the right.
                line(p, x + w * 0.08, y + h * 0.07, w * 0.32, dark);
                line(p, x + w * 0.12, y + h * 0.1, w * 0.24, dark);
                line(p, x + w * 0.52, y + h * 0.07, w * 0.42, dark);
                line(p, x + w * 0.58, y + h * 0.1, w * 0.3, dark);
                line(p, x + w * 0.62, y + h * 0.13, w * 0.22, grey);
                line(p, x + w * 0.1, y + h * 0.17, w * 0.2, grey);
                line(p, x + w * 0.56, y + h * 0.17, w * 0.34, grey);
                line(p, x + w * 0.3, y + h * 0.25, w * 0.4, accent);
                line(p, x + w * 0.35, y + h * 0.28, w * 0.3, dark);
                body(p, 0.34, 0.68, 0.1, 0.08);
                line(p, x + w * 0.6, y + h * 0.75, w * 0.26, dark);
                line(p, x + w * 0.64, y + h * 0.88, w * 0.18, dark);
                line(p, x + w * 0.08, y + h * 0.78, w * 0.16, grey);
                line(p, x + w * 0.08, y + h * 0.81, w * 0.22, grey);
            }
            "report" => {
                p.fill_rect(x + 1.0, y + h * 0.1, w - 2.0, h * 0.2, accent);
                let on = t.accent_on;
                line(p, x + w * 0.1, y + h * 0.16, w * 0.6, on);
                line(p, x + w * 0.1, y + h * 0.21, w * 0.4, on);
                line(p, x + w * 0.1, y + h * 0.38, w * 0.35, accent);
                body(p, 0.43, 0.6, 0.1, 0.1);
                line(p, x + w * 0.1, y + h * 0.64, w * 0.3, accent);
                body(p, 0.69, 0.9, 0.1, 0.1);
            }
            "minutes" => {
                line(p, x + w * 0.1, y + h * 0.08, w * 0.55, accent);
                line(p, x + w * 0.1, y + h * 0.12, w * 0.3, dark);
                for k in 0..4 {
                    let yy = y + h * (0.2 + 0.04 * k as f32);
                    line(p, x + w * 0.1, yy, w * 0.18, dark);
                    line(p, x + w * 0.32, yy, w * 0.45, grey);
                }
                line(p, x + w * 0.1, y + h * 0.4, w * 0.25, accent);
                for k in 0..5 {
                    let yy = y + h * (0.45 + 0.05 * k as f32);
                    p.fill_rect(x + w * 0.1, yy - h * 0.006, w * 0.03, w * 0.03, accent);
                    line(p, x + w * 0.16, yy, w * (0.7 - 0.08 * (k % 2) as f32), grey);
                }
                line(p, x + w * 0.1, y + h * 0.76, w * 0.25, accent);
                body(p, 0.8, 0.92, 0.1, 0.1);
            }
            "resume" => {
                p.fill_rect(
                    x + 1.0,
                    y + 1.0,
                    w * 0.32,
                    h - 2.0,
                    Rgba(232, 238, 246, 255),
                );
                let cxp = x + w * 0.16;
                p.path(&circle(cxp, y + h * 0.12, w * 0.09), Some(accent), None);
                for k in 0..6 {
                    line(
                        p,
                        x + w * 0.05,
                        y + h * (0.26 + 0.035 * k as f32),
                        w * 0.22,
                        grey,
                    );
                }
                line(p, x + w * 0.4, y + h * 0.07, w * 0.5, dark);
                line(p, x + w * 0.4, y + h * 0.11, w * 0.3, accent);
                for (k, yy) in [0.2_f32, 0.45, 0.7].into_iter().enumerate() {
                    line(p, x + w * 0.4, y + h * yy, w * 0.25, accent);
                    body(p, yy + 0.04, yy + 0.18 - 0.02 * k as f32, 0.4, 0.08);
                }
            }
            _ => {
                // Memo: a bold MEMO word, To / From / Date / Subject rows, a rule, body.
                line(p, x + w * 0.1, y + h * 0.08, w * 0.3, dark);
                line(p, x + w * 0.1, y + h * 0.09, w * 0.3, dark);
                for k in 0..4 {
                    let yy = y + h * (0.17 + 0.04 * k as f32);
                    line(p, x + w * 0.1, yy, w * 0.12, accent);
                    line(p, x + w * 0.26, yy, w * 0.4, grey);
                }
                p.fill_rect(
                    x + w * 0.1,
                    y + h * 0.35,
                    w * 0.8,
                    (h * 0.006).max(1.0),
                    dark,
                );
                body(p, 0.4, 0.9, 0.1, 0.1);
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
                tr(s, "docR6.bs.recent", "Recent")
            } else {
                tr(s, "docR6.bs.thisPc", "This PC")
            },
            tr(s, "docR6.bs.browse", "Browse"),
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
                &tr(s, "docR6.bs.documents", "Documents"),
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
            self.heading(p, &tr(s, "docR6.bs.thisPc", "This PC"), lx, y, 18.0, INK);
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
                &tr(s, "docR6.bs.fileName", "File name"),
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
        let path = match &self.facts.path {
            Some(path) => path
                .split(['\\', '/'])
                .filter(|x| !x.is_empty())
                .collect::<Vec<_>>()
                .join(" » "),
            None => tr(
                s,
                "docR6.bs.notSaved",
                "This document has not been saved yet.",
            ),
        };
        // Clipped before the properties column.
        let clip_w = (w - self.px(322.0)).max(cx + self.px(640.0)) - cx - self.px(16.0);
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
                "docR6.bs.protect",
                "Protect Document",
                "docR6.bs.protectDesc",
                if self.facts.protected {
                    "Restrictions are enforced: some changes are limited."
                } else {
                    "Control what types of changes people can make to this document."
                },
                vec![],
            ),
            (
                "docR6.bs.inspect",
                "Inspect Document",
                "docR6.bs.inspectDesc",
                "Before publishing this file, be aware that it contains:",
                vec![
                    tr(
                        s,
                        "docR6.bs.hasProps",
                        "Document properties and author's name",
                    ),
                    tr(
                        s,
                        "docR6.bs.hasLinks",
                        "Content that people with disabilities may find difficult to read",
                    ),
                ],
            ),
            (
                "docR6.bs.manage",
                "Manage Document",
                "docR6.bs.manageDesc",
                if self.facts.modified {
                    "There are unsaved changes."
                } else {
                    "There are no unsaved changes."
                },
                vec![],
            ),
        ];
        let tile_labels = [
            ("docR6.bs.protectTile", "Protect\nDocument"),
            ("docR6.bs.inspectTile", "Check for\nIssues"),
            ("docR6.bs.manageTile", "Manage\nDocument"),
        ];
        let desc_key_text = |key: &str, fb: &str| {
            if key == "docR6.bs.protectDesc" && self.facts.protected {
                tr(s, "docR6.bs.protectOn", fb)
            } else if key == "docR6.bs.manageDesc" && self.facts.modified {
                tr(s, "docR6.bs.unsaved", fb)
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
        self.paint_properties(p, t, s, w);
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
        let minutes = f
            .editing_minutes
            .map(|m| format!("{m} {}", tr(s, "docR6.bs.minutes", "Minutes")));
        let rows: Vec<(String, String, &str, &str)> = vec![
            (tr(s, "docR6.bs.size", "Size"), size, "", ""),
            (
                tr(s, "wordCount.pages", "Pages"),
                f.pages.to_string(),
                "",
                "",
            ),
            (
                tr(s, "wordCount.words", "Words"),
                f.words.to_string(),
                "",
                "",
            ),
            (
                tr(s, "docR6.bs.editTime", "Total Editing Time"),
                minutes.unwrap_or_default(),
                "",
                "",
            ),
            (
                tr(s, "backstage.props.title", "Title"),
                f.title.clone(),
                "docR6.bs.addTitle",
                "Add a title",
            ),
            (
                tr(s, "docR6.bs.tags", "Tags"),
                f.tags.clone(),
                "docR6.bs.addTag",
                "Add a tag",
            ),
            (
                tr(s, "docR6.bs.comments", "Comments"),
                f.comments.clone(),
                "docR6.bs.addComments",
                "Add comments",
            ),
        ];
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
        head(p, &tr(s, "docR6.bs.relatedDates", "Related Dates"), y);
        y += self.px(31.0);
        let never = tr(s, "docR6.bs.never", "Never");
        for (k, v) in [
            (
                tr(s, "backstage.props.modified", "Last Modified"),
                f.last_modified.clone(),
            ),
            (
                tr(s, "backstage.props.created", "Created"),
                f.created.clone(),
            ),
            (tr(s, "docR6.bs.printed", "Last Printed"), String::new()),
        ] {
            p.text(&k, x, y, body, INK);
            p.text(if v.is_empty() { &never } else { &v }, vx, y, body, INK);
            y += self.px(29.0);
        }
        y += self.px(16.0);
        head(p, &tr(s, "docR6.bs.relatedPeople", "Related People"), y);
        y += self.px(31.0);
        let not_saved = tr(s, "docR6.bs.notSavedYet", "Not saved yet");
        for (k, v, ph) in [
            (
                tr(s, "backstage.props.author", "Author"),
                f.author.clone(),
                tr(s, "docR6.bs.addAuthor", "Add an author"),
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
            head(p, &tr(s, "docR6.bs.relatedDocs", "Related Documents"), y);
            y += self.px(34.0);
            folder_icon(p, x + self.px(8.0), y, self.px(22.0));
            p.text(
                &tr(s, "docR6.bs.openLocation", "Open File Location"),
                x + self.px(46.0),
                y + self.px(1.0),
                body,
                INK,
            );
        }
        let _ = t;
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
            &tr(s, "docR6.bs.copies", "Copies:"),
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
            &tr(s, "docR6.bs.printer", "Printer"),
            tops[0] - self.px(36.0),
        );
        hs(
            p,
            &tr(s, "docR6.bs.settings", "Settings"),
            tops[1] - self.px(36.0),
        );
        let printer = self
            .print
            .printers
            .get(self.print.printer)
            .cloned()
            .unwrap_or_else(|| tr(s, "docR6.bs.noPrinter", "No printer installed"));
        let ready = tr(s, "docR6.bs.ready", "Ready");
        let all = tr(s, "docR6.bs.allPages", "Print All Pages");
        let current = tr(s, "docR6.bs.currentPage", "Print Current Page");
        let custom = tr(s, "docR6.bs.customPrint", "Custom Print");
        let (range, range_sub) = match self.print.range {
            1 => (current, tr(s, "docR6.bs.currentSub", "Only this page")),
            2 => (custom, tr(s, "docR6.bs.customSub", "Type specific pages")),
            _ => (all, tr(s, "docR6.bs.allSub", "The whole document")),
        };
        let (sides, sides_sub) = match self.print.sides {
            1 => (
                tr(s, "docR6.bs.bothLong", "Print on Both Sides"),
                tr(s, "docR6.bs.bothLongSub", "Flip pages on long edge"),
            ),
            2 => (
                tr(s, "docR6.bs.bothShort", "Print on Both Sides"),
                tr(s, "docR6.bs.bothShortSub", "Flip pages on short edge"),
            ),
            _ => (
                tr(s, "docR6.bs.oneSided", "Print One Sided"),
                tr(
                    s,
                    "docR6.bs.oneSidedSub",
                    "Only print on one side of the page",
                ),
            ),
        };
        let (collate, collate_sub) = if self.print.collated {
            (
                tr(s, "docR6.bs.collated", "Collated"),
                "1,2,3    1,2,3    1,2,3".to_owned(),
            )
        } else {
            (
                tr(s, "docR6.bs.uncollated", "Uncollated"),
                "1,1,1    2,2,2    3,3,3".to_owned(),
            )
        };
        let orient = if self.facts.landscape {
            tr(s, "docR6.bs.landscape", "Landscape Orientation")
        } else {
            tr(s, "docR6.bs.portrait", "Portrait Orientation")
        };
        let (pname, pw, ph) = &self.facts.paper;
        let paper_sub = format!("{pw:.2}\" x {ph:.2}\"");
        let per = if self.print.per_sheet <= 1 {
            tr(s, "docR6.bs.onePerSheet", "1 Page Per Sheet")
        } else {
            format!(
                "{} {}",
                self.print.per_sheet,
                tr(s, "docR6.bs.perSheet", "Pages Per Sheet")
            )
        };
        let boxes: [(String, String, u8); 8] = [
            (printer, ready, 0),
            (range, range_sub, 1),
            (sides, sides_sub, 2),
            (collate, collate_sub, 3),
            (orient, String::new(), 4),
            (pname.clone(), paper_sub, 5),
            (self.facts.margins.clone(), String::new(), 6),
            (per, String::new(), 7),
        ];
        for ((label, sub, icon), ry) in boxes.iter().zip(&tops) {
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
        // Pages box under the range.
        let pb = self.pages_box();
        p.text(
            &tr(s, "docR6.bs.pages", "Pages:"),
            cx + self.px(4.0),
            pb[1] + self.px(5.0),
            body,
            INK,
        );
        field(p, pb, self.scale);
        if self.pages_focus {
            p.fill_rect(pb[0], pb[1], pb[2], 1.0, t.accent_strong);
            p.fill_rect(pb[0], pb[1] + pb[3] - 1.0, pb[2], 1.0, t.accent_strong);
        }
        p.text(
            &self.print.pages,
            pb[0] + self.px(5.0),
            pb[1] + self.px(5.0),
            body,
            INK,
        );
        // Preview: a light frame, the page, page navigation.
        let a = self.preview_area();
        p.fill_rect(
            a[0] - self.px(28.0),
            a[1],
            1.0,
            a[3],
            Rgba(212, 212, 212, 255),
        );
        p.fill_rect(a[0], a[1], a[2], a[3], Rgba(255, 255, 255, 255));
        if let Some((_, r)) = self.preview_rect() {
            p.fill_rect(
                r[0] - 1.0,
                r[1] - 1.0,
                r[2] + 2.0,
                r[3] + 2.0,
                Rgba(198, 198, 198, 255),
            );
            p.fill_rect(r[0], r[1], r[2], r[3], WHITE);
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
            &format!("{} {n}", tr(s, "docR6.bs.of", "of")),
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
            tr(s, "docR6.bs.createPdf", "Create PDF/XPS Document"),
            tr(s, "docR6.bs.changeType", "Change File Type"),
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
            self.heading(p, &items[0], rx, y + self.px(2.0), 21.0, t.accent_text);
            for (i, (k, f)) in [
                (
                    "docR6.bs.pdf1",
                    "Preserves layout, formatting, fonts, and images",
                ),
                ("docR6.bs.pdf2", "Content can't be easily changed"),
                ("docR6.bs.pdf3", "Free viewers are available on the web"),
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
                    &tr(s, "docR6.bs.createPdfTile", "Create PDF/\nXPS"),
                    0,
                );
            }
        } else {
            self.heading(p, &items[1], rx, y + self.px(2.0), 21.0, t.accent_text);
            p.text(
                &tr(s, "docR6.bs.docTypes", "Document File Types"),
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
                self.big_button(p, t, r, &tr(s, "docR6.bs.saveAsTile", "Save\nAs"), 1);
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

/// Copy path, Open file location (label key, fallback): the Info page's button row.
const INFO_BUTTONS: [(u8, &str, &str); 2] = [
    (0, "docR6.bs.copyPath", "Copy path"),
    (1, "docR6.bs.openLocation", "Open file location"),
];

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
    let hour = crate::doc_backstage6::local_hour();
    if hour < 12 {
        tr(s, "docR6.bs.morning", "Good morning")
    } else if hour < 18 {
        tr(s, "docR6.bs.afternoon", "Good afternoon")
    } else {
        tr(s, "docR6.bs.evening", "Good evening")
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

#[cfg(test)]
mod tests {
    use super::*;

    fn facts() -> DocFacts {
        DocFacts {
            name: "a.docx".into(),
            path: Some("C:\\docs\\a.docx".into()),
            pages: 3,
            page_sizes: vec![(612.0, 792.0); 3],
            ..DocFacts::default()
        }
    }

    fn centre(b: &DocBackstage, h: Hit) -> (f32, f32) {
        let r = b.rect_of(h).expect("region");
        (r[0] + r[2] / 2.0, r[1] + r[3] / 2.0)
    }

    #[test]
    fn nav_switches_pages_and_actions() {
        let mut b = DocBackstage::new(BsPage::Info, facts());
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
    fn print_page_copies_range_and_preview() {
        let mut b = DocBackstage::new(BsPage::Print, facts());
        b.layout(40.0, (1200.0, 880.0), 1.0);
        let (x, y) = centre(&b, Hit::CopiesUp);
        b.press(x, y);
        b.press(x, y);
        assert_eq!(b.print.copies, 3);
        let (x, y) = centre(&b, Hit::PagesBox);
        b.press(x, y);
        assert!(b.type_text("1, 3-9x"));
        assert_eq!(b.print.pages, "1, 3-9");
        assert_eq!(b.pages_to_print(), vec![0, 2]);
        assert!(b.key(crate::Key::PageDown));
        assert_eq!(b.preview, 1);
        let (_, r) = b.preview_rect().expect("preview");
        let a = b.preview_area();
        assert!(r[0] >= a[0] && r[0] + r[2] <= a[0] + a[2]);
        // Letter is 8.5 × 11: the preview keeps the shape.
        assert!(((r[2] / r[3]) - 612.0 / 792.0).abs() < 0.01);
        let (x, y) = centre(&b, Hit::PrintSetting(4));
        assert!(matches!(b.press(x, y), BsOut::Menu("bs6.orientation", _)));
    }

    #[test]
    fn export_and_templates() {
        let mut b = DocBackstage::new(BsPage::Export, facts());
        b.layout(40.0, (1200.0, 880.0), 1.0);
        let (x, y) = centre(&b, Hit::Action(0));
        assert_eq!(b.press(x, y), BsOut::ExportPdf);
        let (x, y) = centre(&b, Hit::ExportItem(1));
        b.press(x, y);
        let (x, y) = centre(&b, Hit::TypeRow(4));
        b.press(x, y);
        let (x, y) = centre(&b, Hit::Action(1));
        assert_eq!(b.press(x, y), BsOut::SaveAsType("mht"));
        b.show(BsPage::New);
        let (x, y) = centre(&b, Hit::Template(Some(1)));
        assert_eq!(b.press(x, y), BsOut::New(Some("report")));
        let (x, y) = centre(&b, Hit::Template(None));
        assert_eq!(b.press(x, y), BsOut::New(None));
    }

    #[test]
    fn open_page_lists_the_apps_own_recent_files() {
        let mut b = DocBackstage::new(BsPage::Open, facts());
        b.recent = vec![Recent {
            path: "D:\\x\\b.docx".into(),
            when: String::new(),
        }];
        b.layout(40.0, (1200.0, 880.0), 1.0);
        let (x, y) = centre(&b, Hit::RecentRow(0));
        assert_eq!(b.press(x, y), BsOut::OpenPath("D:\\x\\b.docx".into()));
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
