//! BlueSlide's File screen (parity round 10 of the slide series, `docs/ui/REPORT-slide.md`
//! rounds 20+): the navigation column (Home, New, Open | Info, Save, Save As, Print, Export,
//! Close | Options) and one page per entry, laid out after the reference presentation app's
//! File screen measured at 125 % (positions below are logical pixels: reference pixels / 1.25).
//! Started from BlueDoc's File screen (`doc_backstage6.rs` on its branch) and adapted:
//!
//! * Home: a greeting, the New row (blank presentation and BlueOffice's own theme templates)
//!   and the app's own recent presentations;
//! * New: the blank presentation and the template gallery (own theme designs drawn here);
//! * Open: places (Recent, Browse) and the recent list kept by BlueSlide itself;
//! * Info: path, Copy path / Open file location, Protect / Check for Issues / Manage tiles
//!   with menus, the properties column (size, slides, hidden slides, title, tags, categories,
//!   dates, people);
//! * Save As: This PC (current folder, name, type) and Browse;
//! * Print: Print button with copies, printer and settings drop-downs (slides, Slides box,
//!   Full Page Slides / Notes Pages / Handouts, sides, collation, colour), a live preview of
//!   the printed page with page navigation, Edit Header & Footer;
//! * Export: Create PDF/XPS Document, Create a Video (animated GIF), Package Presentation,
//!   Create Handouts, Change File Type.
//!
//! The nav column keeps BlueSlide's own accent colour (the reference's brand colour is not
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
    (Nav::Page(BsPage::Home), "slideR10.bs.home", "Home"),
    (Nav::Page(BsPage::New), "backstage.nav.new", "New"),
    (Nav::Page(BsPage::Open), "backstage.nav.open", "Open"),
];
const NAV_MID: [(Nav, &str, &str); 6] = [
    (Nav::Page(BsPage::Info), "backstage.nav.info", "Info"),
    (Nav::Save, "backstage.nav.save", "Save"),
    (Nav::Page(BsPage::SaveAs), "backstage.nav.saveAs", "Save As"),
    (Nav::Page(BsPage::Print), "backstage.nav.print", "Print"),
    (Nav::Page(BsPage::Export), "slideR10.bs.export", "Export"),
    (Nav::Close, "backstage.nav.close", "Close"),
];

/// BlueOffice's own presentation templates: the Design tab's themes
/// (`bo_engine::slide_edit::master::THEMES` id, label key, fallback).
pub(crate) const TEMPLATES: [(&str, &str, &str); 5] = [
    ("harbor", "slideR10.tpl.harbor", "Harbor"),
    ("lotus", "slideR10.tpl.lotus", "Lotus"),
    ("bamboo", "slideR10.tpl.bamboo", "Bamboo"),
    ("ember", "slideR10.tpl.ember", "Ember"),
    ("slate", "slideR10.tpl.slate", "Slate"),
];

/// File types of Export › Change File Type and Save As: (extension, label key, fallback).
pub(crate) const FILE_TYPES: [(&str, &str, &str); 8] = [
    ("pptx", "slideR10.type.pptx", "Presentation (*.pptx)"),
    ("potx", "slideR10.type.potx", "Template (*.potx)"),
    ("ppsx", "slideR10.type.ppsx", "Slide Show (*.ppsx)"),
    ("pdf", "slideR10.type.pdf", "PDF (*.pdf)"),
    (
        "png",
        "slideR10.type.png",
        "PNG Portable Network Graphics (*.png)",
    ),
    (
        "jpg",
        "slideR10.type.jpg",
        "JPEG File Interchange Format (*.jpg)",
    ),
    (
        "svg",
        "slideR10.type.svg",
        "Scalable Vector Graphics (*.svg)",
    ),
    ("gif", "slideR10.type.gif", "Animated GIF Format (*.gif)"),
];

/// Export page entries: (label key, fallback).
const EXPORT_ITEMS: [(&str, &str); 5] = [
    ("slideR10.bs.createPdf", "Create PDF/XPS Document"),
    ("slideR10.bs.createVideo", "Create a Video"),
    ("slideR10.bs.package", "Package Presentation"),
    ("slideR10.bs.handouts", "Create Handouts"),
    ("slideR10.bs.changeType", "Change File Type"),
];

/// Print layouts: (handout slides per page or 0, label key, fallback, sub-label key, fallback).
pub(crate) const LAYOUTS: [(u8, &str, &str); 8] = [
    (0, "slideR10.bs.fullPage", "Full Page Slides"),
    (0, "slideR10.bs.notesPages", "Notes Pages"),
    (1, "slideR10.bs.ho1", "1 Slide"),
    (2, "slideR10.bs.ho2", "2 Slides"),
    (3, "slideR10.bs.ho3", "3 Slides"),
    (4, "slideR10.bs.ho4", "4 Slides Horizontal"),
    (6, "slideR10.bs.ho6", "6 Slides Horizontal"),
    (9, "slideR10.bs.ho9", "9 Slides Horizontal"),
];

/// A presentation in the app's recent list.
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
    /// New presentation: blank (`None`) or with one of the own themes.
    New(Option<&'static str>),
    /// Save under a new name as this type (system dialog).
    SaveAsType(&'static str),
    ExportPdf,
    /// Export › Create a Video (animated GIF), Package Presentation, Create Handouts.
    ExportVideo,
    ExportPackage,
    ExportHandouts,
    CopyPath,
    OpenLocation,
    /// Print with the page's settings.
    Print,
    /// Print › Edit Header & Footer.
    HeaderFooter,
    /// The chosen printer's own settings dialog.
    PrinterProperties,
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
    /// 0 all slides, 1 current slide, 2 custom range.
    pub(crate) range: u8,
    /// Custom slides ("1-3, 5").
    pub(crate) pages: String,
    /// [`LAYOUTS`] index.
    pub(crate) layout: usize,
    /// Frame slides (handouts and full page slides).
    pub(crate) frame: bool,
    /// 0 one sided, 1 both sides (long edge), 2 both sides (short edge).
    pub(crate) sides: u8,
    pub(crate) collated: bool,
    /// 0 colour, 1 grayscale, 2 pure black and white.
    pub(crate) color: u8,
}

impl Default for PrintSettings {
    fn default() -> Self {
        Self {
            copies: 1,
            printers: Vec::new(),
            printer: 0,
            range: 0,
            pages: String::new(),
            layout: 0,
            frame: false,
            sides: 0,
            collated: true,
            color: 0,
        }
    }
}

/// Facts the pages show about the open presentation.
#[derive(Debug, Clone, Default)]
pub(crate) struct SlideFacts {
    pub(crate) name: String,
    pub(crate) path: Option<String>,
    pub(crate) bytes: Option<u64>,
    pub(crate) slides: usize,
    pub(crate) hidden: usize,
    /// The slide shown in the editing area (Print Current Slide).
    pub(crate) current: usize,
    pub(crate) modified: bool,
    /// Some slide has speaker notes (Check for Issues lists them).
    pub(crate) has_notes: bool,
    pub(crate) title: String,
    pub(crate) tags: String,
    pub(crate) categories: String,
    pub(crate) author: String,
    pub(crate) last_modified_by: String,
    pub(crate) created: String,
    pub(crate) last_modified: String,
    /// Slide size in points (preview shape).
    pub(crate) slide_size: (f32, f32),
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
    HfLink,
    PrinterProps,
}

/// The open File screen.
pub(crate) struct SlideBackstage {
    pub(crate) page: BsPage,
    top: f32,
    size: (f32, f32),
    scale: f32,
    hover: Option<Hit>,
    pub(crate) facts: SlideFacts,
    pub(crate) recent: Vec<Recent>,
    pub(crate) print: PrintSettings,
    /// Printed page shown in the print preview.
    pub(crate) preview: usize,
    /// Export: [`EXPORT_ITEMS`] index; chosen type.
    pub(crate) export: u8,
    pub(crate) file_type: usize,
    /// Open / Save As: chosen place (0 Recent / This PC, 1 Browse).
    place: u8,
    /// Typing in the Slides box of the Print page.
    pub(crate) pages_focus: bool,
    /// Measured widths of the Info page's buttons and of the Save label.
    button_w: [f32; 3],
    /// Print page: texts of the setting boxes cut to fit with "…" (full text, shown text).
    fitted: Vec<(String, String)>,
    /// Measured widths of the Print page's links (Printer Properties, Edit Header & Footer).
    link_w: [f32; 2],
    hits: Vec<(Hit, [f32; 4])>,
}

/// Navigation column width.
const NAV_W: f32 = 171.2;
/// Navigation row height.
const ROW_H: f32 = 44.0;
/// Content background (#F3F3F3, measured).
const CONTENT_BG: Rgba = Rgba(243, 243, 243, 255);
/// Body text on the content area (#262626).
const INK: Rgba = Rgba(38, 38, 38, 255);
/// Secondary text and placeholders.
const MUTED: Rgba = Rgba(104, 104, 104, 255);
const PLACEHOLDER: Rgba = Rgba(128, 128, 128, 255);
/// Initials badge of an author (File › Info) and its letters.
const BADGE: Rgba = Rgba(214, 214, 214, 255);
const BADGE_INK: Rgba = Rgba(110, 110, 110, 255);
/// Frames of tiles and buttons.
const FRAME: Rgba = Rgba(171, 171, 171, 255);
const WHITE: Rgba = Rgba(255, 255, 255, 255);
/// Hover fill on the content area.
const HOVER: Rgba = Rgba(218, 218, 218, 255);
/// The chosen item on the content area (Export list, places).
const CHOSEN: Rgba = Rgba(198, 198, 198, 255);

impl SlideBackstage {
    pub(crate) fn new(page: BsPage, facts: SlideFacts) -> Self {
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
            fitted: Vec::new(),
            link_w: [0.0; 2],
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
        // Print settings: a text wider than its box ends with "…" (reference).
        if self.page == BsPage::Print {
            for (i, (k, f)) in [
                ("slideR10.bs.printerProps", "Printer Properties"),
                ("slideR10.bs.editHf", "Edit Header & Footer"),
            ]
            .iter()
            .enumerate()
            {
                self.link_w[i] = m.measure(&tr(strings, k, f), style).0;
            }
            let avail = self.px(293.0 - 56.0 - 24.0);
            self.fitted = self
                .print_boxes(strings)
                .iter()
                .flat_map(|(a, b, _)| [a.clone(), b.clone()])
                .filter(|t| m.measure(t, style).0 > avail)
                .map(|t| {
                    let chars: Vec<char> = t.chars().collect();
                    let mut n = chars.len();
                    while n > 0 {
                        let cut: String = chars[..n].iter().collect::<String>() + "…";
                        if m.measure(&cut, style).0 <= avail {
                            return (t, cut);
                        }
                        n -= 1;
                    }
                    (t, "…".to_owned())
                })
                .collect();
        }
        self.hits = self.regions();
    }

    /// The Print page's setting boxes: (label, second line, icon / id).
    fn print_boxes(&self, s: &Strings) -> [(String, String, u8); 6] {
        let printer = self
            .print
            .printers
            .get(self.print.printer)
            .cloned()
            .unwrap_or_else(|| tr(s, "slideR10.bs.noPrinter", "No printer installed"));
        let ready = tr(s, "slideR10.bs.ready", "Ready");
        let (range, range_sub) = match self.print.range {
            1 => (
                tr(s, "slideR10.bs.currentSlide", "Print Current Slide"),
                tr(s, "slideR10.bs.currentSub", "Only print the current slide"),
            ),
            2 => (
                tr(s, "slideR10.bs.customRange", "Custom Range"),
                tr(s, "slideR10.bs.customSub", "Enter specific slides to print"),
            ),
            _ => (
                tr(s, "slideR10.bs.allSlides", "Print All Slides"),
                tr(s, "slideR10.bs.allSub", "Print entire presentation"),
            ),
        };
        let (per, lk, lf) = LAYOUTS[self.print.layout.min(LAYOUTS.len() - 1)];
        let layout_sub = match (self.print.layout, per) {
            (0, _) => tr(s, "slideR10.bs.fullPageSub", "Print 1 slide per page"),
            (_, 0) => tr(s, "slideR10.bs.notesSub", "Print slides with notes"),
            _ => tr(
                s,
                "slideR10.bs.handoutSub",
                "Handouts ({n} slides per page)",
            )
            .replace("{n}", &per.to_string()),
        };
        let (sides, sides_sub) = match self.print.sides {
            1 => (
                tr(s, "slideR10.bs.bothLong", "Print on Both Sides"),
                tr(s, "slideR10.bs.bothLongSub", "Flip pages on long edge"),
            ),
            2 => (
                tr(s, "slideR10.bs.bothShort", "Print on Both Sides"),
                tr(s, "slideR10.bs.bothShortSub", "Flip pages on short edge"),
            ),
            _ => (
                tr(s, "slideR10.bs.oneSided", "Print One Sided"),
                tr(
                    s,
                    "slideR10.bs.oneSidedSub",
                    "Only print on one side of the page",
                ),
            ),
        };
        let (collate, collate_sub) = if self.print.collated {
            (
                tr(s, "slideR10.bs.collated", "Collated"),
                "1,2,3    1,2,3    1,2,3".to_owned(),
            )
        } else {
            (
                tr(s, "slideR10.bs.uncollated", "Uncollated"),
                "1,1,1    2,2,2    3,3,3".to_owned(),
            )
        };
        let color = match self.print.color {
            1 => tr(s, "slideR10.bs.grayscale", "Grayscale"),
            2 => tr(s, "slideR10.bs.pureBw", "Pure Black and White"),
            _ => tr(s, "slideR10.bs.color", "Color"),
        };
        [
            (printer, ready, 0),
            (range, range_sub, 1),
            (tr(s, lk, lf), layout_sub, 2),
            (sides, sides_sub, 3),
            (collate, collate_sub, 4),
            (color, String::new(), 5),
        ]
    }

    /// Width of Print page link `i` (measured, else estimated).
    fn link_width(&self, label: &str, i: usize) -> f32 {
        match self.link_w.get(i) {
            Some(w) if *w > 0.0 => w.round(),
            _ => label.chars().count() as f32 * self.px(6.4),
        }
    }

    /// A Print setting's text as shown (cut to its box when measured too wide).
    fn fit<'a>(&'a self, t: &'a str) -> &'a str {
        self.fitted
            .iter()
            .find(|(full, _)| full == t)
            .map_or(t, |(_, cut)| cut.as_str())
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
            "slideR10.bs.options",
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
                            cx + self.px(216.0) * i as f32,
                            y,
                            self.px(196.0),
                            self.px(144.0),
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
                // Reference at 125 %: blank card 306 × 172 at x 285, y 171; gallery cards
                // from y 567, 332 px apart, rows 246 px apart; label 26 px under the card.
                let (cw, ch) = (self.px(245.0), self.px(166.0));
                let y0 = self.top + self.px(105.0);
                r.push((Hit::Template(None), [cx + self.px(8.0), y0, cw, ch]));
                let y1 = y0 + self.px(276.0);
                let step = self.px(265.6);
                let per_row = (((w - cx - self.px(8.0)) / step).floor() as usize).max(1);
                for i in 0..TEMPLATES.len() {
                    let (row, col) = (i / per_row, i % per_row);
                    r.push((
                        Hit::Template(Some(i)),
                        [
                            cx + self.px(8.0) + step * col as f32,
                            y1 + self.px(197.0) * row as f32,
                            cw,
                            ch,
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
                        [cx + self.px(4.0), ty, self.px(120.0), self.px(101.0)],
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
                r.push((Hit::PrinterProps, self.printer_props_rect()));
                if let Some(last) = self.setting_tops().last() {
                    r.push((
                        Hit::HfLink,
                        [
                            cx + self.px(160.0),
                            last + self.px(56.0),
                            self.px(137.0),
                            self.px(22.0),
                        ],
                    ));
                }
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
                for i in 0..EXPORT_ITEMS.len() as u8 {
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
                if usize::from(self.export) + 1 < EXPORT_ITEMS.len() {
                    // The tile under the panel's description (PDF: y 150 as measured; the
                    // other panels have one line less).
                    let ty = if self.export == 0 { 150.0 } else { 130.0 };
                    r.push((
                        Hit::Action(0),
                        [rx, y + self.px(ty), self.px(101.0), self.px(101.0)],
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
        // Printer box 352 px and the first setting 510 px down the window at 125 %
        // (reference; the Printer Properties link sits between them).
        let y = self.top + self.px(245.0);
        // Printer, then Settings: slides, (Slides box), layout, sides, collated, colour.
        let mut v = vec![y];
        let s = y + self.px(126.5);
        v.push(s);
        let mut yy = s + self.px(51.0) + self.px(38.0);
        for _ in 0..4 {
            v.push(yy);
            yy += self.px(55.0);
        }
        v
    }

    fn pages_box(&self) -> [f32; 4] {
        let cx = self.content_x();
        let y = self.top + self.px(245.0 + 126.5 + 51.0 + 5.0);
        [cx + self.px(52.0), y, self.px(222.0), self.px(29.6)]
    }

    /// The Printer Properties link under the printer box (right-aligned to it).
    fn printer_props_rect(&self) -> [f32; 4] {
        let cx = self.content_x();
        let y = self.setting_tops()[0] + self.px(51.0) + self.px(6.0);
        [cx + self.px(180.0), y, self.px(117.0), self.px(22.0)]
    }

    /// The print preview's area (device pixels): from 38 px right of the column rule to the
    /// scroll bar, 118 px under the title bar to above the zoom bar (reference).
    pub(crate) fn preview_area(&self) -> [f32; 4] {
        let cx = self.content_x();
        let x = cx + self.px(372.0);
        let y = self.top + self.px(94.0);
        let (w, h) = self.size;
        [
            x,
            y,
            (w - x - self.px(22.0)).max(10.0),
            (h - y - self.px(48.0)).max(10.0),
        ]
    }

    /// Zoom of the preview in per cent of the printed paper (A4: 842 x 595 pt) at the
    /// screen's resolution, as the reference's zoom box.
    pub(crate) fn preview_zoom(&self) -> Option<u32> {
        let (_, r) = self.preview_rect()?;
        let (pw, ph) = self.printed_size();
        let paper = if pw >= ph { 842.0 } else { 595.0 };
        let page_px = paper * 96.0 / 72.0 * self.scale;
        Some((r[2] / page_px * 100.0).round() as u32)
    }

    /// Size in points of a printed page with the chosen layout: the slide for Full Page
    /// Slides, US Letter portrait for notes pages and handouts.
    pub(crate) fn printed_size(&self) -> (f32, f32) {
        if self.print.layout == 0 {
            let (w, h) = self.facts.slide_size;
            if w > 0.0 && h > 0.0 {
                return (w, h);
            }
            return (960.0, 540.0);
        }
        (612.0, 792.0)
    }

    /// Slides the Print page prints (0-based, in order).
    pub(crate) fn slides_to_print(&self) -> Vec<usize> {
        let n = self.facts.slides;
        match self.print.range {
            1 => vec![self.facts.current.min(n.saturating_sub(1))],
            2 => parse_ranges(&self.print.pages, n),
            _ => (0..n).collect(),
        }
    }

    /// Number of printed pages with the chosen layout.
    pub(crate) fn printed_pages(&self) -> usize {
        let k = self.slides_to_print().len();
        match LAYOUTS[self.print.layout.min(LAYOUTS.len() - 1)].0 {
            0 => k,
            per => k.div_ceil(usize::from(per)),
        }
    }

    /// What the preview shows: the layout's slides per handout page (0: one slide or notes
    /// page) and the slides on the previewed page.
    pub(crate) fn preview_target(&self) -> (usize, Vec<usize>) {
        let slides = self.slides_to_print();
        let per = usize::from(LAYOUTS[self.print.layout.min(LAYOUTS.len() - 1)].0);
        let i = self.preview.min(self.printed_pages().saturating_sub(1));
        if per == 0 {
            (0, slides.get(i).copied().into_iter().collect())
        } else {
            (
                per,
                slides.iter().skip(i * per).take(per).copied().collect(),
            )
        }
    }

    /// Where the preview page goes: (page index, x, y, width, height), fitted in the area.
    pub(crate) fn preview_rect(&self) -> Option<(usize, [f32; 4])> {
        if self.page != BsPage::Print {
            return None;
        }
        let n = self.printed_pages();
        if n == 0 {
            return None;
        }
        let i = self.preview.min(n - 1);
        let (pw, ph) = self.printed_size();
        let [x, y, w, h] = self.preview_area();
        let pad = self.px(13.6);
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
                    BsOut::SaveAsType("pptx")
                }
            }
            Hit::Place(_) => {
                self.place = 0;
                BsOut::Redraw
            }
            Hit::InfoButton(0) => BsOut::CopyPath,
            Hit::InfoButton(_) => BsOut::OpenLocation,
            Hit::Tile(i) => {
                let id = ["sbs10.protect", "sbs10.inspect", "sbs10.manage"][usize::from(i.min(2))];
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
            Hit::HfLink => BsOut::HeaderFooter,
            Hit::PrinterProps => BsOut::PrinterProperties,
            Hit::Action(a) => match (self.page, a) {
                (BsPage::Print, _) => BsOut::Print,
                (BsPage::Export, 0) => match self.export {
                    0 => BsOut::ExportPdf,
                    1 => BsOut::ExportVideo,
                    2 => BsOut::ExportPackage,
                    _ => BsOut::ExportHandouts,
                },
                (BsPage::Export, _) | (BsPage::SaveAs, 1) => {
                    BsOut::SaveAsType(FILE_TYPES[self.file_type].0)
                }
                (BsPage::SaveAs, _) => {
                    BsOut::Menu("sbs10.saveType", self.rect_of(h).unwrap_or_default())
                }
                _ => BsOut::None,
            },
            Hit::PrintSetting(i) => {
                let id = [
                    "sbs10.printer",
                    "sbs10.range",
                    "sbs10.layout",
                    "sbs10.sides",
                    "sbs10.collate",
                    "sbs10.color",
                ][usize::from(i.min(5))];
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
                if self.preview + 1 < self.printed_pages() {
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
                let n = self.printed_pages();
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
            BsPage::Export => tr(s, "slideR10.bs.export", "Export"),
        };
        let light = Font {
            family: "Segoe UI Light",
            size: self.px(48.0),
            weight: 300,
            italic: false,
            underline: false,
        };
        // Cap height 43 px at 125 %, cap top 36 px under the title bar (reference).
        p.text_font(&title, cx, self.top + self.px(11.5), &light, INK);
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
                cx + self.px(216.0) * i as f32,
                y,
                self.px(196.0),
                self.px(110.0),
            ];
            let tpl = if i == 0 { None } else { Some(i - 1) };
            if self.hover == Some(Hit::Template(tpl)) {
                p.fill_rect(
                    r[0] - 4.0,
                    r[1] - 4.0,
                    r[2] + 8.0,
                    r[3] + self.px(34.0),
                    HOVER,
                );
            }
            self.paint_template_thumb(p, t, tpl, r);
            self.heading(
                p,
                &template_label(s, tpl),
                r[0],
                r[1] + r[3] + self.px(6.0),
                12.5,
                INK,
            );
        }
        let ly = y + self.px(232.0);
        self.heading(
            p,
            &tr(s, "slideR10.bs.recent", "Recent"),
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
                    "slideR10.bs.noRecent",
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
        let style = TextStyle {
            size: self.px(13.0),
            bold: true,
        };
        for (hit, r) in &self.hits {
            let Hit::Template(tpl) = hit else { continue };
            if self.hover == Some(*hit) {
                p.fill_rect(
                    r[0] - 6.0,
                    r[1] - 6.0,
                    r[2] + 12.0,
                    r[3] + self.px(40.0),
                    HOVER,
                );
            }
            self.paint_template_thumb(p, t, *tpl, [r[0], r[1], r[2], self.px(138.0)]);
            let label = template_label(s, *tpl);
            let lw = label.chars().count() as f32 * self.px(7.2);
            p.text(
                &label,
                (r[0] + (r[2] - lw) / 2.0).max(r[0]).round(),
                r[1] + self.px(147.0),
                style,
                INK,
            );
        }
        // Rule between the blank presentation and the gallery, then the gallery heading.
        let y = self.top + self.px(105.0 + 196.0);
        p.fill_rect(
            cx,
            y,
            (w - cx - self.px(64.0)).max(1.0),
            1.0,
            Rgba(212, 212, 212, 255),
        );
        self.heading(
            p,
            &tr(s, "slideR10.bs.templates", "BlueOffice themes"),
            cx,
            y + self.px(40.0),
            13.0,
            MUTED,
        );
    }

    /// Our own template thumbnails: a 16:9 slide in the theme's colours with a title in its
    /// heading typeface, a subtitle line and the six accent colours (own design).
    #[allow(clippy::unused_self)]
    fn paint_template_thumb(
        &self,
        p: &mut dyn Painter,
        t: &Theme,
        tpl: Option<usize>,
        r: [f32; 4],
    ) {
        let _ = t;
        p.fill_rect(r[0], r[1], r[2], r[3], Rgba(212, 212, 212, 255));
        p.fill_rect(r[0] + 1.0, r[1] + 1.0, r[2] - 2.0, r[3] - 2.0, WHITE);
        let Some((def, colors, major)) = tpl.and_then(|i| theme_parts(TEMPLATES[i].0)) else {
            return;
        };
        let (x, y, w, h) = (r[0] + 1.0, r[1] + 1.0, r[2] - 2.0, r[3] - 2.0);
        let rgb = |v: u32| Rgba((v >> 16) as u8, (v >> 8) as u8, v as u8, 255);
        // Background: the light colour 2 for most themes; Slate is dark.
        let dark = def.id == "slate";
        let (bg, ink) = if dark {
            (rgb(colors[2]), rgb(colors[1]))
        } else {
            (rgb(colors[3]), rgb(colors[2]))
        };
        p.fill_rect(x, y, w, h, bg);
        // A band in accent 1 on the left and a thin rule under the title.
        p.fill_rect(x, y, (w * 0.035).round(), h, rgb(colors[4]));
        let title = Font {
            family: major,
            size: (h * 0.2).round(),
            weight: 400,
            italic: false,
            underline: false,
        };
        p.push_clip(x, y, w, h);
        p.text_font("Aa", x + w * 0.1, y + h * 0.22, &title, ink);
        p.pop_clip();
        p.fill_rect(
            (x + w * 0.1).round(),
            (y + h * 0.56).round(),
            (w * 0.5).round(),
            (h * 0.02).max(1.0).round(),
            rgb(colors[5]),
        );
        p.fill_rect(
            (x + w * 0.1).round(),
            (y + h * 0.63).round(),
            (w * 0.34).round(),
            (h * 0.015).max(1.0).round(),
            Rgba(ink.0, ink.1, ink.2, 120),
        );
        // The six accents along the foot.
        let sw = (w * 0.08).round();
        for (k, c) in colors[4..10].iter().enumerate() {
            p.fill_rect(
                (x + w * 0.1 + (sw + w * 0.015) * k as f32).round(),
                (y + h * 0.8).round(),
                sw,
                (h * 0.06).round().max(2.0),
                rgb(*c),
            );
        }
    }

    fn paint_places(&self, p: &mut dyn Painter, t: &Theme, s: &Strings) {
        let (w, _) = self.size;
        let cx = self.content_x();
        let y = self.top + self.px(100.0);
        let open = self.page == BsPage::Open;
        let places = [
            if open {
                tr(s, "slideR10.bs.recent", "Recent")
            } else {
                tr(s, "slideR10.bs.thisPc", "This PC")
            },
            tr(s, "slideR10.bs.browse", "Browse"),
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
                &tr(s, "slideR10.bs.presentations", "Presentations"),
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
            self.heading(p, &tr(s, "slideR10.bs.thisPc", "This PC"), lx, y, 18.0, INK);
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
                &tr(s, "slideR10.bs.fileName", "File name"),
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
                "slideR10.bs.notSaved",
                "This presentation has not been saved yet.",
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
                "slideR10.bs.protect",
                "Protect Presentation",
                "slideR10.bs.protectDesc",
                "Control what types of changes people can make to this presentation.",
                vec![],
            ),
            (
                "slideR10.bs.inspect",
                "Inspect Presentation",
                "slideR10.bs.inspectDesc",
                "Before publishing this file, be aware that it contains:",
                {
                    let mut v = vec![tr(
                        s,
                        "slideR10.bs.hasProps",
                        "Document properties and author's name",
                    )];
                    if self.facts.has_notes {
                        v.push(tr(s, "slideR10.bs.hasNotes", "Presentation notes"));
                    }
                    v.push(tr(
                        s,
                        "slideR10.bs.hasA11y",
                        "Content that people with disabilities are unable to read",
                    ));
                    v
                },
            ),
            (
                "slideR10.bs.manage",
                "Manage Presentation",
                "slideR10.bs.manageDesc",
                if self.facts.modified {
                    "There are unsaved changes."
                } else {
                    "There are no unsaved changes."
                },
                vec![],
            ),
        ];
        let tile_labels = [
            ("slideR10.bs.protectTile", "Protect\nPresentation"),
            ("slideR10.bs.inspectTile", "Check for\nIssues"),
            ("slideR10.bs.manageTile", "Manage\nPresentation"),
        ];
        let desc_key_text = |key: &str, fb: &str| {
            if key == "slideR10.bs.manageDesc" && self.facts.modified {
                tr(s, "slideR10.bs.unsaved", fb)
            } else {
                tr(s, key, fb)
            }
        };
        for (i, ty) in self.tile_tops().into_iter().enumerate() {
            let r = [cx + self.px(4.0), ty, self.px(120.0), self.px(101.0)];
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
            let tx = cx + self.px(141.0);
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
                format!("{:.1}KB", b as f64 / 1024.0)
            } else {
                format!("{:.1}MB", b as f64 / 1024.0 / 1024.0)
            }
        });
        let rows: Vec<(String, String, &str, &str)> = vec![
            (tr(s, "slideR10.bs.size", "Size"), size, "", ""),
            (
                tr(s, "slideR10.bs.slides", "Slides"),
                f.slides.to_string(),
                "",
                "",
            ),
            (
                tr(s, "slideR10.bs.hidden", "Hidden slides"),
                f.hidden.to_string(),
                "",
                "",
            ),
            (
                tr(s, "backstage.props.title", "Title"),
                f.title.clone(),
                "slideR10.bs.addTitle",
                "Add a title",
            ),
            (
                tr(s, "slideR10.bs.tags", "Tags"),
                f.tags.clone(),
                "slideR10.bs.addTag",
                "Add a tag",
            ),
            (
                tr(s, "slideR10.bs.categories", "Categories"),
                f.categories.clone(),
                "slideR10.bs.addCategory",
                "Add a category",
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
        head(p, &tr(s, "slideR10.bs.relatedDates", "Related Dates"), y);
        y += self.px(31.0);
        let never = tr(s, "slideR10.bs.never", "Never");
        for (k, v) in [
            (
                tr(s, "slideR10.bs.lastModified", "Last Modified"),
                f.last_modified.clone(),
            ),
            (
                tr(s, "backstage.props.created", "Created"),
                f.created.clone(),
            ),
            (tr(s, "slideR10.bs.printed", "Last Printed"), String::new()),
        ] {
            p.text(&k, x, y, body, INK);
            if v.is_empty() {
                p.text(&never, vx, y, body, PLACEHOLDER);
            } else {
                p.text(&v, vx, y, body, INK);
            }
            y += self.px(29.0);
        }
        y += self.px(16.0);
        head(p, &tr(s, "slideR10.bs.relatedPeople", "Related People"), y);
        y += self.px(31.0);
        let not_saved = tr(s, "slideR10.bs.notSavedYet", "Not saved yet");
        // Author: each author with an initials badge (grey disc), then "Add an author".
        p.text(&tr(s, "backstage.props.author", "Author"), x, y, body, INK);
        let authors: Vec<&str> = f
            .author
            .split(';')
            .map(str::trim)
            .filter(|a| !a.is_empty())
            .collect();
        for a in &authors {
            let d = self.px(43.0);
            let (cx, cy) = (vx + d / 2.0, y + self.px(9.0));
            p.path(&disc(cx, cy, d / 2.0), Some(BADGE), None);
            let ini = initials(a);
            let isz = self.px(15.0);
            p.text(
                &ini,
                (cx - ini.chars().count() as f32 * isz * 0.32).round(),
                (cy - isz * 0.68).round(),
                TextStyle {
                    size: isz,
                    bold: false,
                },
                BADGE_INK,
            );
            // Presence ring at the lower right (no status known: hollow).
            let (rx, ry, rr) = (cx + d * 0.36, cy + d * 0.36, self.px(6.0));
            p.path(&disc(rx, ry, rr), Some(CONTENT_BG), None);
            p.path(
                &disc(rx, ry, rr - self.px(1.0)),
                None,
                Some((Rgba(140, 168, 186, 255), self.px(1.2))),
            );
            p.text(a, vx + d + self.px(5.0), y, body, INK);
            y += self.px(53.0);
        }
        p.text(
            &tr(s, "slideR10.bs.addAuthor", "Add an author"),
            vx,
            y - if authors.is_empty() {
                0.0
            } else {
                self.px(12.0)
            },
            body,
            PLACEHOLDER,
        );
        y += self.px(29.0);
        p.text(
            &tr(s, "slideR10.bs.lastModifiedBy", "Last Modified By"),
            x,
            y,
            body,
            INK,
        );
        if f.last_modified_by.is_empty() {
            p.text(&not_saved, vx, y, body, PLACEHOLDER);
        } else {
            p.text(&f.last_modified_by, vx, y, body, INK);
        }
        y += self.px(29.0);
        if f.path.is_some() {
            y += self.px(16.0);
            head(p, &tr(s, "slideR10.bs.relatedDocs", "Related Documents"), y);
            y += self.px(34.0);
            folder_icon(p, x + self.px(8.0), y, self.px(22.0));
            p.text(
                &tr(s, "slideR10.bs.openLocation", "Open File Location"),
                x + self.px(46.0),
                y + self.px(1.0),
                body,
                INK,
            );
            y += self.px(22.0);
        }
        // Kept inside the window (the reference scrolls this column).
        y = (y + self.px(24.0)).min(self.size.1 - self.px(26.0));
        p.text(
            &tr(s, "slideR10.bs.allProps", "Show All Properties"),
            x,
            y,
            body,
            t.accent_text,
        );
    }

    #[allow(clippy::too_many_lines)]
    fn paint_print(&self, p: &mut dyn Painter, t: &Theme, s: &Strings) {
        let (_, h) = self.size;
        let cx = self.content_x();
        let y = self.top + self.px(92.0);
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
            &tr(s, "slideR10.bs.copies", "Copies:"),
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
            &tr(s, "slideR10.bs.printer", "Printer"),
            tops[0] - self.px(36.0),
        );
        hs(
            p,
            &tr(s, "slideR10.bs.settings", "Settings"),
            tops[1] - self.px(36.0),
        );
        let boxes = self.print_boxes(s);
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
                p.text(
                    self.fit(label),
                    tx,
                    r[1] + (r[3] - body.size * 1.33) / 2.0,
                    body,
                    INK,
                );
            } else {
                p.text(self.fit(label), tx, r[1] + self.px(4.0), body, INK);
                p.text(self.fit(sub), tx, r[1] + self.px(26.0), body, INK);
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
            &tr(s, "slideR10.bs.slidesBox", "Slides:"),
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
        // Printer Properties link and the (i) marks beside Printer and the Slides box.
        {
            let r = self.printer_props_rect();
            let label = tr(s, "slideR10.bs.printerProps", "Printer Properties");
            let link = Rgba(t.accent_text.0, t.accent_text.1, t.accent_text.2, 255);
            let lw = self.link_width(&label, 0);
            let lx = (r[0] + r[2] - lw).round();
            p.text(&label, lx, r[1], body, link);
            p.fill_rect(lx, r[1] + self.px(18.0), lw, 1.0, link);
            let d = self.px(14.0);
            info_mark(
                p,
                cx + self.px(4.0) + self.px(293.0) - self.px(7.0) - d / 2.0,
                tops[0] - self.px(32.0),
                d,
            );
            info_mark(
                p,
                pb[0] + pb[2] + self.px(5.0),
                pb[1] + (pb[3] - d) / 2.0,
                d,
            );
        }
        if let Some(r) = self.rect_of(Hit::HfLink) {
            let label = tr(s, "slideR10.bs.editHf", "Edit Header & Footer");
            let link = Rgba(t.accent_text.0, t.accent_text.1, t.accent_text.2, 255);
            let lw = self.link_width(&label, 1);
            let lx = (r[0] + r[2] - lw).round();
            p.text(&label, lx, r[1], body, link);
            p.fill_rect(lx, r[1] + self.px(18.0), lw, 1.0, link);
        }
        // Preview: a light frame, the page, page navigation.
        let a = self.preview_area();
        p.fill_rect(
            a[0] - self.px(30.0),
            self.top + self.px(84.0),
            1.0,
            h - self.top - self.px(84.0),
            Rgba(212, 212, 212, 255),
        );
        self.paint_preview_bars(p, a);
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
        let n = self.printed_pages().max(1);
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
            &format!("{} {n}", tr(s, "slideR10.bs.of", "of")),
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

    /// The preview's scroll bar (one step per printed page) and the zoom bar under it:
    /// percentage, minus, slider, plus, fit button (reference).
    fn paint_preview_bars(&self, p: &mut dyn Painter, a: [f32; 4]) {
        let n = self.printed_pages().max(1);
        let i = self.preview.min(n - 1);
        let line = Rgba(171, 171, 171, 255);
        // Scroll bar right of the area.
        let sw = self.px(13.6);
        let sx = a[0] + a[2] + self.px(1.6);
        let (sy, sh) = (a[1], a[3]);
        p.fill_rect(sx, sy, sw, sh, line);
        p.fill_rect(sx + 1.0, sy + 1.0, sw - 2.0, sh - 2.0, WHITE);
        let c = sx + sw / 2.0;
        let k = self.px(3.2);
        let top = sy + sw / 2.0;
        p.path(
            &[
                PathSeg::M(c - k, top + k / 2.0),
                PathSeg::L(c + k, top + k / 2.0),
                PathSeg::L(c, top - k / 2.0),
                PathSeg::Z,
            ],
            Some(INK),
            None,
        );
        let bottom = sy + sh - sw / 2.0;
        p.path(
            &[
                PathSeg::M(c - k, bottom - k / 2.0),
                PathSeg::L(c + k, bottom - k / 2.0),
                PathSeg::L(c, bottom + k / 2.0),
                PathSeg::Z,
            ],
            Some(INK),
            None,
        );
        p.fill_rect(sx + 1.0, sy + sw, sw - 2.0, 1.0, line);
        p.fill_rect(sx + 1.0, sy + sh - sw - 1.0, sw - 2.0, 1.0, line);
        let track = (sy + sw + self.px(2.0), sh - 2.0 * sw - self.px(4.0));
        let th = (track.1 / n as f32).max(self.px(16.0));
        let f = if n > 1 {
            i as f32 / (n - 1) as f32
        } else {
            0.0
        };
        let ty = track.0 + (track.1 - th) * f;
        let inset = self.px(2.4);
        p.fill_rect(
            sx + inset,
            ty,
            sw - 2.0 * inset,
            th,
            Rgba(205, 205, 205, 255),
        );
        // Zoom bar: fit button under the scroll bar, then plus, slider, minus, percentage.
        let Some(z) = self.preview_zoom() else {
            return;
        };
        let (_, h) = self.size;
        let cy = (h - self.px(34.0) + self.px(12.0)).round();
        let fb = self.px(16.0);
        let fx = (sx + (sw - fb) / 2.0).round();
        let dark = Rgba(68, 68, 68, 255);
        p.fill_rect(fx, cy - fb / 2.0, fb, fb, Rgba(118, 118, 118, 255));
        p.fill_rect(
            fx + 1.0,
            cy - fb / 2.0 + 1.0,
            fb - 2.0,
            fb - 2.0,
            CONTENT_BG,
        );
        let q = fb * 0.18;
        for (dx, dy) in [(-1.0f32, -1.0f32), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
            let (ox, oy) = (fx + fb / 2.0 + dx * fb * 0.12, cy + dy * fb * 0.12);
            p.path(
                &[PathSeg::M(ox, oy), PathSeg::L(ox + dx * q, oy + dy * q)],
                None,
                Some((dark, 1.0)),
            );
        }
        let stroke = self.scale.round().max(1.0);
        let plus = (fx - self.px(25.6)).round();
        let arm = self.px(4.8).round();
        p.fill_rect(plus - arm, cy, 2.0 * arm, stroke, MUTED);
        p.fill_rect(plus, cy - arm, stroke, 2.0 * arm, MUTED);
        let tr_r = plus - self.px(9.6);
        let tr_l = tr_r - self.px(121.6);
        let grey = Rgba(150, 150, 150, 255);
        p.fill_rect(tr_l, cy, tr_r - tr_l, 1.0, grey);
        p.fill_rect(
            f32::midpoint(tr_l, tr_r).round(),
            cy - self.px(3.2),
            1.0,
            self.px(6.4),
            grey,
        );
        // 10 % at the left end, 100 % in the middle, 400 % at the right end.
        let zf = z as f32;
        let pos = if zf <= 100.0 {
            0.5 * ((zf - 10.0) / 90.0).clamp(0.0, 1.0)
        } else {
            0.5 + 0.5 * ((zf - 100.0) / 300.0).clamp(0.0, 1.0)
        };
        let kx = (tr_l + (tr_r - tr_l) * pos).round();
        p.fill_rect(
            kx - self.px(1.6),
            cy - self.px(6.4),
            self.px(3.2),
            self.px(12.8),
            dark,
        );
        let minus = (tr_l - self.px(9.6)).round();
        p.fill_rect(minus - arm, cy, 2.0 * arm, stroke, MUTED);
        let pct = format!("{z}%");
        let pw = pct.chars().count() as f32 * self.px(7.6);
        p.text(
            &pct,
            (minus - arm - self.px(10.0) - pw).round(),
            cy - self.px(9.0),
            self.body(),
            INK,
        );
    }

    #[allow(clippy::too_many_lines)]
    fn paint_export(&self, p: &mut dyn Painter, t: &Theme, s: &Strings) {
        let cx = self.content_x();
        let y = self.top + self.px(84.0);
        let big = TextStyle {
            size: self.px(14.5),
            bold: false,
        };
        let body = self.body();
        for (i, (k, f)) in EXPORT_ITEMS.iter().enumerate() {
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
            export_icon(
                p,
                i as u8,
                r[0] + self.px(12.0),
                r[1] + (r[3] - ih) / 2.0,
                ih,
                t,
            );
            p.text(
                &tr(s, k, f),
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
        let chosen = usize::from(self.export).min(EXPORT_ITEMS.len() - 1);
        let (hk, hf) = EXPORT_ITEMS[chosen];
        let heading = match chosen {
            0 => tr(s, "slideR10.bs.createPdfHead", "Create a PDF/XPS Document"),
            _ => tr(s, hk, hf),
        };
        self.heading(p, &heading, rx, y + self.px(2.0), 21.0, t.accent_text);
        if chosen + 1 == EXPORT_ITEMS.len() {
            p.text(
                &tr(s, "slideR10.bs.presTypes", "Presentation File Types"),
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
                self.big_button(p, t, r, &tr(s, "slideR10.bs.saveAsTile", "Save\nAs"), 4);
            }
            return;
        }
        let bullets: &[(&str, &str)] = match chosen {
            0 => &[
                (
                    "slideR10.bs.pdf1",
                    "Preserves layout, formatting, fonts, and images",
                ),
                ("slideR10.bs.pdf2", "Content can't be easily changed"),
                ("slideR10.bs.pdf3", "Free viewers are available on the web"),
            ],
            1 => &[
                (
                    "slideR10.bs.video1",
                    "Save your presentation as an animated picture you can share",
                ),
                (
                    "slideR10.bs.video2",
                    "Includes the transitions and the slide timings",
                ),
            ],
            2 => &[
                (
                    "slideR10.bs.pkg1",
                    "Collect the presentation, its media and fonts in one folder file",
                ),
                (
                    "slideR10.bs.pkg2",
                    "Linked files are included in the package",
                ),
            ],
            _ => &[
                (
                    "slideR10.bs.ho1Desc",
                    "Put slides and notes on pages people can take away",
                ),
                (
                    "slideR10.bs.ho2Desc",
                    "Choose how many slides go on each page",
                ),
            ],
        };
        for (i, (k, f)) in bullets.iter().enumerate() {
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
        let tile = match chosen {
            0 => tr(s, "slideR10.bs.createPdfTile", "Create PDF/\nXPS"),
            1 => tr(s, "slideR10.bs.createVideoTile", "Create\nVideo"),
            2 => tr(s, "slideR10.bs.packageTile", "Package\nPresentation"),
            _ => tr(s, "slideR10.bs.handoutsTile", "Create\nHandouts"),
        };
        if let Some(r) = self.rect_of(Hit::Action(0)) {
            self.big_button(p, t, r, &tile, chosen as u8);
        }
    }

    fn big_button(&self, p: &mut dyn Painter, t: &Theme, r: [f32; 4], label: &str, icon: u8) {
        let action = u8::from(icon >= 4);
        let hover = self.hover == Some(Hit::Action(action));
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
        export_icon(
            p,
            icon,
            r[0] + (r[2] - ih) / 2.0,
            r[1] + self.px(12.0),
            ih,
            t,
        );
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

/// Icons of the Export list (own drawings): PDF page, film strip, disc-less package box,
/// handout sheet with slide frames, save-as-type mark.
fn export_icon(p: &mut dyn Painter, i: u8, x: f32, y: f32, s: f32, t: &Theme) {
    match i {
        0 => pdf_icon(p, x, y, s, t),
        1 => {
            // A film frame with a play triangle.
            p.fill_rect(
                x + s * 0.04,
                y + s * 0.18,
                s * 0.92,
                s * 0.64,
                Rgba(70, 76, 86, 255),
            );
            for k in 0..5 {
                let fx = x + s * (0.1 + 0.17 * k as f32);
                p.fill_rect(fx, y + s * 0.22, s * 0.08, s * 0.06, WHITE);
                p.fill_rect(fx, y + s * 0.72, s * 0.08, s * 0.06, WHITE);
            }
            p.path(
                &[
                    PathSeg::M(x + s * 0.4, y + s * 0.36),
                    PathSeg::L(x + s * 0.64, y + s * 0.5),
                    PathSeg::L(x + s * 0.4, y + s * 0.64),
                    PathSeg::Z,
                ],
                Some(t.accent_strong),
                None,
            );
        }
        2 => {
            // A box with a lid and a ribbon.
            let tan = Rgba(214, 170, 96, 255);
            let dark = Rgba(170, 126, 60, 255);
            p.fill_rect(x + s * 0.12, y + s * 0.36, s * 0.76, s * 0.56, tan);
            p.fill_rect(x + s * 0.06, y + s * 0.24, s * 0.88, s * 0.16, dark);
            p.fill_rect(
                x + s * 0.46,
                y + s * 0.24,
                s * 0.08,
                s * 0.68,
                t.accent_strong,
            );
        }
        3 => {
            // A portrait sheet with three slide frames and lines beside them.
            p.fill_rect(
                x + s * 0.18,
                y + s * 0.02,
                s * 0.64,
                s * 0.94,
                Rgba(120, 120, 120, 255),
            );
            p.fill_rect(
                x + s * 0.18 + 1.0,
                y + s * 0.02 + 1.0,
                s * 0.64 - 2.0,
                s * 0.94 - 2.0,
                WHITE,
            );
            for k in 0..3 {
                let fy = y + s * (0.1 + 0.29 * k as f32);
                p.fill_rect(x + s * 0.24, fy, s * 0.26, s * 0.18, t.accent_strong);
                p.fill_rect(
                    x + s * 0.56,
                    fy + s * 0.04,
                    s * 0.2,
                    1.0,
                    Rgba(150, 150, 150, 255),
                );
                p.fill_rect(
                    x + s * 0.56,
                    fy + s * 0.12,
                    s * 0.2,
                    1.0,
                    Rgba(150, 150, 150, 255),
                );
            }
        }
        _ => save_type_icon(p, x, y, s, t),
    }
}

/// Copy path, Open file location (label key, fallback): the Info page's button row.
const INFO_BUTTONS: [(u8, &str, &str); 2] = [
    (0, "slideR10.bs.copyPath", "Copy path"),
    (1, "slideR10.bs.openLocation", "Open file location"),
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

/// Name of a New row card: the blank presentation or a theme template.
fn template_label(s: &Strings, tpl: Option<usize>) -> String {
    match tpl.and_then(|i| TEMPLATES.get(i)) {
        None => tr(s, "slideR10.bs.blank", "Blank Presentation"),
        Some((_, k, f)) => tr(s, k, f),
    }
}

/// A theme of the Design tab: its definition, colour slots and heading typeface.
fn theme_parts(
    id: &str,
) -> Option<(
    &'static bo_engine::slide_edit::master::ThemeDef,
    [u32; 12],
    &'static str,
)> {
    use bo_engine::slide_edit::master::{COLOR_SETS, FONT_SETS, THEMES};
    let def = THEMES.iter().find(|t| t.id == id)?;
    let colors = COLOR_SETS.iter().find(|c| c.id == def.colors)?.c;
    let major = FONT_SETS
        .iter()
        .find(|f| f.id == def.fonts)
        .map_or("Calibri Light", |f| f.major);
    Some((def, colors, major))
}

/// "Good morning" / "Good afternoon" / "Good evening" by the local hour.
fn greeting(s: &Strings) -> String {
    let hour = local_hour();
    if hour < 12 {
        tr(s, "slideR10.bs.morning", "Good morning")
    } else if hour < 18 {
        tr(s, "slideR10.bs.afternoon", "Good afternoon")
    } else {
        tr(s, "slideR10.bs.evening", "Good evening")
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

/// Icons of the Print page's settings (own drawings, kept simple): printer, slides, layout,
/// sides, collation, colour.
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
            // Two slides, the front one with a title bar.
            sheet(p, x + s * 0.2, y + s * 0.12, s * 0.74, s * 0.5);
            sheet(p, x + s * 0.04, y + s * 0.3, s * 0.74, s * 0.5);
            p.fill_rect(
                x + s * 0.14,
                y + s * 0.4,
                s * 0.4,
                s * 0.08,
                t.accent_strong,
            );
            lines(p, x + s * 0.14, y + s * 0.56, s * 0.5, 2);
        }
        2 => sheet(p, x + s * 0.02, y + s * 0.2, s * 0.96, s * 0.56),
        3 => {
            sheet(p, x + s * 0.06, y + s * 0.08, s * 0.88, s * 0.8);
            lines(p, x + s * 0.16, y + s * 0.3, s * 0.66, 5);
            p.fill_rect(
                x + s * 0.16,
                y + s * 0.16,
                s * 0.18,
                s * 0.1,
                t.accent_strong,
            );
        }
        4 => {
            for k in 0..3 {
                let dx = s * 0.3 * k as f32;
                sheet(p, x + dx + s * 0.06, y + s * 0.2, s * 0.24, s * 0.34);
                sheet(p, x + dx, y + s * 0.26, s * 0.24, s * 0.34);
            }
        }
        _ => {
            // Colour: white, grey and black bands.
            p.fill_rect(
                x + s * 0.1,
                y + s * 0.1,
                s * 0.8,
                s * 0.26,
                Rgba(250, 250, 250, 255),
            );
            p.fill_rect(
                x + s * 0.1,
                y + s * 0.36,
                s * 0.8,
                s * 0.26,
                Rgba(150, 150, 150, 255),
            );
            p.fill_rect(
                x + s * 0.1,
                y + s * 0.62,
                s * 0.8,
                s * 0.26,
                Rgba(20, 20, 20, 255),
            );
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

/// Up to two initials of a name ("BlueOffice corpus" → "BC").
fn initials(name: &str) -> String {
    name.split_whitespace()
        .filter_map(|w| w.chars().next())
        .take(2)
        .flat_map(char::to_uppercase)
        .collect()
}

/// A filled circle as a 32-sided polygon.
fn disc(cx: f32, cy: f32, r: f32) -> Vec<bo_render::PathSeg> {
    use bo_render::PathSeg;
    let mut v: Vec<PathSeg> = (0..32)
        .map(|i| {
            let a = i as f32 / 32.0 * std::f32::consts::TAU;
            let (x, y) = (cx + r * a.cos(), cy + r * a.sin());
            if i == 0 {
                PathSeg::M(x, y)
            } else {
                PathSeg::L(x, y)
            }
        })
        .collect();
    v.push(PathSeg::Z);
    v
}

/// A small "information" mark: a blue ring with an "i" (Print page, beside Printer and the
/// Slides box).
fn info_mark(p: &mut dyn Painter, x: f32, y: f32, d: f32) {
    let blue = Rgba(0, 102, 204, 255);
    let r = d / 2.0;
    let (cx, cy) = (x + r, y + r);
    p.path(
        &disc(cx, cy, r - 0.6),
        None,
        Some((blue, (d / 14.0).max(1.0))),
    );
    let w = (d / 9.0).max(1.0).round();
    p.fill_rect(
        (cx - w / 2.0).round(),
        (cy - r * 0.15).round(),
        w,
        (r * 0.75).round(),
        blue,
    );
    p.fill_rect((cx - w / 2.0).round(), (cy - r * 0.55).round(), w, w, blue);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts() -> SlideFacts {
        SlideFacts {
            name: "a.pptx".into(),
            path: Some("C:\\decks\\a.pptx".into()),
            slides: 7,
            current: 2,
            slide_size: (960.0, 540.0),
            ..SlideFacts::default()
        }
    }

    fn centre(b: &SlideBackstage, h: Hit) -> (f32, f32) {
        let r = b.rect_of(h).expect("region");
        (r[0] + r[2] / 2.0, r[1] + r[3] / 2.0)
    }

    #[test]
    fn nav_switches_pages_and_actions() {
        let mut b = SlideBackstage::new(BsPage::Info, facts());
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
    fn print_page_slides_layouts_and_preview() {
        let mut b = SlideBackstage::new(BsPage::Print, facts());
        b.layout(40.0, (1200.0, 880.0), 1.0);
        let (x, y) = centre(&b, Hit::CopiesUp);
        b.press(x, y);
        b.press(x, y);
        assert_eq!(b.print.copies, 3);
        // Full Page Slides: one page per slide, the slide's shape.
        assert_eq!(b.printed_pages(), 7);
        let (_, r) = b.preview_rect().expect("preview");
        let a = b.preview_area();
        assert!(r[0] >= a[0] && r[0] + r[2] <= a[0] + a[2]);
        assert!(((r[2] / r[3]) - 16.0 / 9.0).abs() < 0.01);
        // Slides box: a custom range.
        let (x, y) = centre(&b, Hit::PagesBox);
        b.press(x, y);
        assert!(b.type_text("1, 3-5x"));
        assert_eq!(b.print.pages, "1, 3-5");
        assert_eq!(b.slides_to_print(), vec![0, 2, 3, 4]);
        // Handouts with 3 slides per page: two pages, the second with one slide.
        b.print.layout = 4;
        assert_eq!(b.printed_pages(), 2);
        assert!(b.key(crate::Key::PageDown));
        assert_eq!(b.preview_target(), (3, vec![4]));
        let (_, r) = b.preview_rect().expect("preview");
        assert!(((r[2] / r[3]) - 612.0 / 792.0).abs() < 0.01);
        // Current slide.
        b.print.range = 1;
        assert_eq!(b.slides_to_print(), vec![2]);
        let (x, y) = centre(&b, Hit::PrintSetting(2));
        assert!(matches!(b.press(x, y), BsOut::Menu("sbs10.layout", _)));
        let (x, y) = centre(&b, Hit::HfLink);
        assert_eq!(b.press(x, y), BsOut::HeaderFooter);
    }

    #[test]
    fn export_panels_and_templates() {
        let mut b = SlideBackstage::new(BsPage::Export, facts());
        b.layout(40.0, (1200.0, 880.0), 1.0);
        let (x, y) = centre(&b, Hit::Action(0));
        assert_eq!(b.press(x, y), BsOut::ExportPdf);
        for (i, out) in [
            (1, BsOut::ExportVideo),
            (2, BsOut::ExportPackage),
            (3, BsOut::ExportHandouts),
        ] {
            let (x, y) = centre(&b, Hit::ExportItem(i));
            b.press(x, y);
            let (x, y) = centre(&b, Hit::Action(0));
            assert_eq!(b.press(x, y), out);
        }
        let (x, y) = centre(&b, Hit::ExportItem(4));
        b.press(x, y);
        let (x, y) = centre(&b, Hit::TypeRow(2));
        b.press(x, y);
        let (x, y) = centre(&b, Hit::Action(1));
        assert_eq!(b.press(x, y), BsOut::SaveAsType("ppsx"));
        b.show(BsPage::New);
        let (x, y) = centre(&b, Hit::Template(Some(1)));
        assert_eq!(b.press(x, y), BsOut::New(Some("lotus")));
        let (x, y) = centre(&b, Hit::Template(None));
        assert_eq!(b.press(x, y), BsOut::New(None));
        // Every template is one of the Design tab's themes.
        for (id, _, _) in TEMPLATES {
            assert!(theme_parts(id).is_some(), "{id}");
        }
    }

    #[test]
    fn open_page_lists_the_apps_own_recent_files() {
        let mut b = SlideBackstage::new(BsPage::Open, facts());
        b.recent = vec![Recent {
            path: "D:\\x\\b.pptx".into(),
            when: String::new(),
        }];
        b.layout(40.0, (1200.0, 880.0), 1.0);
        let (x, y) = centre(&b, Hit::RecentRow(0));
        assert_eq!(b.press(x, y), BsOut::OpenPath("D:\\x\\b.pptx".into()));
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
