//! BlueReader's own Print dialog, laid out like the reference reader's (measured at 125 %:
//! 1535 × 870 px): printer and copies with collate / grayscale, Print Range (current page, all
//! pages, a page list with a subset and reverse order), Print Handling (none, fit to or reduce
//! to the printer margins), auto-rotate / auto-centre, and a preview with page navigation. The
//! host lists the printers and prints the [`PrintSettings`] the dialog returns.
use crate::theme::Theme;
use crate::{Key, Mods, Painter, Strings, TextMeasure, TextStyle};
use bo_pdfread::PrintWhat;
use bo_render::{PathSeg, Rgba};

pub use crate::reader_impose::{BookletSides, Duplex, PageOrder, PrintLayout, PrintScaling};

/// What to print.
#[derive(Debug, Clone, PartialEq)]
pub struct PrintSettings {
    /// Printer name.
    pub printer: String,
    /// Copies.
    pub copies: u32,
    /// Copies collated (each copy all pages) rather than each page `copies` times.
    pub collate: bool,
    /// Page indices in print order.
    pub pages: Vec<usize>,
    /// Page sizing.
    pub scaling: PrintScaling,
    /// Print in shades of grey.
    pub grayscale: bool,
    /// Turn pages to the paper's orientation.
    pub auto_rotate: bool,
    /// Centre pages on the paper (else at the top-left of the printable area).
    pub auto_center: bool,
    /// One page per sheet, several, or a booklet.
    pub layout: PrintLayout,
    /// Both sides of the paper.
    pub duplex: Duplex,
    /// Print only this part of the page (page points x y w h): the current view.
    pub view: Option<[f32; 4]>,
    /// Paper orientation.
    pub orientation: Orientation,
    /// Parts of the pages printed.
    pub what: PrintWhat,
    /// Pages printed as lower-resolution images (150 dpi).
    pub as_image: bool,
    /// Overprinting simulated (inks multiply).
    pub overprint: bool,
    /// Paper chosen by the PDF page size (the first page printed).
    pub paper_by_page: bool,
}

/// Paper orientation (the reference's Orientation box).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Orientation {
    /// Portrait or landscape after the first page printed.
    Auto,
    /// Portrait.
    Portrait,
    /// Landscape.
    Landscape,
}

/// A printer's paper: size and printable area (x y w h), in points.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Paper {
    /// Width and height.
    pub size: (f32, f32),
    /// Printable area.
    pub area: [f32; 4],
}

impl Paper {
    /// The same paper turned landscape (or back).
    #[must_use]
    pub fn turned(self) -> Self {
        let [x, y, w, h] = self.area;
        Self {
            size: (self.size.1, self.size.0),
            area: [y, (self.size.0 - x - w).max(0.0), h, w],
        }
    }
}

/// Answer of a click or key.
#[derive(Debug, Clone, PartialEq)]
pub enum PrintAction {
    /// Print with these settings.
    Print(PrintSettings),
    /// Closed without printing.
    Cancel,
    /// Show the printer driver's settings of this printer (the host keeps what is chosen).
    Properties(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Range {
    View,
    Current,
    All,
    Pages,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Part {
    Printer,
    /// Item of the open drop-down list.
    Item(usize),
    Properties,
    Advanced,
    AsImage,
    Bleed,
    Orientation,
    What,
    Summarize,
    Overprint,
    PageSetting,
    TileZoom,
    TileZoomUp,
    TileZoomDown,
    Overlap,
    OverlapUp,
    OverlapDown,
    CutMarks,
    Labels,
    Slider,
    PaperByPage,
    Copies,
    CopiesUp,
    CopiesDown,
    Collate,
    Gray,
    Range(Range),
    PagesText,
    Subset,
    Reverse,
    Scaling(PrintScaling),
    ScaleText,
    Duplex,
    Flip(bool),
    Handling(Handling),
    PerSheet,
    Order,
    Border,
    BookletSubset,
    Binding,
    Rotate,
    Center,
    PrevPage,
    NextPage,
    Ok,
    Cancel,
    Close,
}

/// Tabs of the Print Handling group.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Handling {
    Scale,
    Tile,
    Multiple,
    Booklet,
}

/// Text box with the typing focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Field {
    Copies,
    Pages,
    Scale,
    TileZoom,
    Overlap,
}

/// Pages per sheet offered, in order.
const PER_SHEET: [u8; 5] = [2, 4, 6, 9, 16];
/// Print What choices in the reference's order.
const WHAT: [PrintWhat; 5] = [
    PrintWhat::Document,
    PrintWhat::DocumentAndMarkups,
    PrintWhat::DocumentAndStamps,
    PrintWhat::FormFields,
    PrintWhat::Annotations,
];
/// Page orders offered.
const ORDERS: [PageOrder; 4] = [
    PageOrder::Horizontal,
    PageOrder::HorizontalReversed,
    PageOrder::Vertical,
    PageOrder::VerticalReversed,
];
/// Booklet subsets offered.
const SIDES: [BookletSides; 3] = [BookletSides::Both, BookletSides::Front, BookletSides::Back];

/// A page placed on the preview's sheet.
struct Shown {
    page: usize,
    /// Part of the page (points).
    src: [f32; 4],
    /// Where it is drawn (device pixels, after turning).
    dest: [f32; 4],
    /// Printed size over page size.
    zoom: f32,
    turn: bool,
    border: bool,
    cut_marks: bool,
    tile: Option<(u16, u16)>,
}

/// Parse a page list such as `1-3, 5, 8-` (1-based) into page indices within `count`.
pub fn parse_pages(s: &str, count: usize) -> Vec<usize> {
    let mut out = Vec::new();
    for part in s.split([',', ';']) {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let num = |v: &str| v.trim().parse::<usize>().ok();
        let (a, b) = match part.split_once('-') {
            Some((a, b)) => (
                num(a).unwrap_or(1),
                if b.trim().is_empty() {
                    Some(count)
                } else {
                    num(b)
                },
            ),
            None => match num(part) {
                Some(v) => (v, Some(v)),
                None => continue,
            },
        };
        let Some(b) = b else { continue };
        let (lo, hi) = (a.min(b).max(1), a.max(b).min(count));
        for p in lo..=hi {
            out.push(p - 1);
        }
    }
    out
}

/// The Print dialog.
pub struct PrintDialog {
    printers: Vec<String>,
    printer: usize,
    copies: u32,
    copies_text: Vec<char>,
    collate: bool,
    gray: bool,
    range: Range,
    pages_text: Vec<char>,
    /// 0 all pages in range, 1 odd, 2 even.
    subset: u8,
    reverse: bool,
    scaling: PrintScaling,
    /// Custom scale text (percent).
    scale_text: Vec<char>,
    handling: Handling,
    per_sheet: u8,
    order: PageOrder,
    border: bool,
    booklet_sides: BookletSides,
    right_binding: bool,
    duplex: bool,
    flip_long: bool,
    /// Visible part of the current page (page, page points x y w h).
    view: Option<(usize, [f32; 4])>,
    rotate: bool,
    center: bool,
    /// Text box receiving typing.
    focus: Field,
    count: usize,
    current: usize,
    /// Index into the sheets of the sheet shown in the preview.
    preview_at: usize,
    /// The combo box whose list is open.
    drop: Option<Part>,
    orientation: Orientation,
    what: PrintWhat,
    as_image: bool,
    overprint: bool,
    tile_zoom: Vec<char>,
    overlap: Vec<char>,
    cut_marks: bool,
    labels: bool,
    paper_by_page: bool,
    /// Size of every page (points).
    sizes: Vec<(f32, f32)>,
    /// Paper of each printer, when the host knows it.
    papers: Vec<Option<Paper>>,
    /// Page images of the sheet shown (one per item), from the host.
    preview_keys: Vec<Option<usize>>,
    scale: f32,
    frame: [f32; 4],
    parts: Vec<(Part, [f32; 4])>,
    hover: Option<Part>,
    preview_rect: [f32; 4],
    /// Window size and display scale of the last layout.
    window: ((f32, f32), f32),
}

const WHITE: Rgba = Rgba(255, 255, 255, 255);
const CAPTION: Rgba = Rgba(242, 244, 243, 255);
const GROUP: Rgba = Rgba(220, 220, 220, 255);
const FIELD: Rgba = Rgba(188, 188, 188, 255);
const FIELD_BG: Rgba = Rgba(253, 253, 253, 255);
const FOCUS: Rgba = Rgba(0x00, 0x78, 0xd7, 255);
const TEXT: Rgba = Rgba(0x1a, 0x1a, 0x1a, 255);
const GREY_TEXT: Rgba = Rgba(0x9a, 0x9a, 0x9a, 255);
const BUTTON: Rgba = Rgba(0xfd, 0xfd, 0xfd, 255);
const BUTTON_EDGE: Rgba = Rgba(0xd0, 0xd0, 0xd0, 255);
const BUTTON_HOVER: Rgba = Rgba(0xe5, 0xf1, 0xfb, 255);
const PAGE_FRAME: Rgba = Rgba(102, 102, 102, 255);
const ITEM_HOVER: Rgba = Rgba(0xe5, 0xf1, 0xfb, 255);

impl PrintDialog {
    /// A dialog for a document of `count` pages showing page `current`; `printers` with the
    /// `default` one chosen.
    pub fn new(printers: Vec<String>, default: Option<&str>, count: usize, current: usize) -> Self {
        let printer = default
            .and_then(|d| printers.iter().position(|p| p == d))
            .unwrap_or(0);
        Self {
            printers,
            printer,
            copies: 1,
            copies_text: vec!['1'],
            collate: true,
            gray: false,
            range: Range::All,
            pages_text: format!("1 - {}", count.max(1)).chars().collect(),
            subset: 0,
            reverse: false,
            scaling: PrintScaling::Fit,
            scale_text: vec!['1', '0', '0'],
            handling: Handling::Scale,
            per_sheet: 2,
            order: PageOrder::Horizontal,
            border: false,
            booklet_sides: BookletSides::Both,
            right_binding: false,
            duplex: false,
            flip_long: true,
            view: None,
            rotate: true,
            center: true,
            focus: Field::Copies,
            count: count.max(1),
            current: current.min(count.saturating_sub(1)),
            preview_at: 0,
            drop: None,
            orientation: Orientation::Portrait,
            what: PrintWhat::DocumentAndMarkups,
            as_image: false,
            overprint: false,
            tile_zoom: vec!['1', '0', '0'],
            overlap: "0.005".chars().collect(),
            cut_marks: false,
            labels: false,
            paper_by_page: false,
            sizes: Vec::new(),
            papers: Vec::new(),
            preview_keys: Vec::new(),
            scale: 1.0,
            frame: [0.0; 4],
            parts: Vec::new(),
            hover: None,
            preview_rect: [0.0; 4],
            window: ((1500.0, 1000.0), 1.0),
        }
    }

    /// Sizes of the document's pages (points), for the preview's sheets.
    pub fn set_page_sizes(&mut self, sizes: Vec<(f32, f32)>) {
        self.sizes = sizes;
    }

    /// Paper of each printer, in the order of the printers (`None` when unknown).
    pub fn set_papers(&mut self, papers: Vec<Option<Paper>>) {
        self.papers = papers;
    }

    /// Paper of printer `name` after its driver settings changed.
    pub fn set_paper(&mut self, name: &str, paper: Option<Paper>) {
        if let Some(i) = self.printers.iter().position(|p| p == name) {
            if self.papers.len() <= i {
                self.papers.resize(i + 1, None);
            }
            self.papers[i] = paper;
        }
    }

    /// The chosen printer.
    pub fn printer(&self) -> Option<&str> {
        self.printers.get(self.printer).map(String::as_str)
    }

    /// Page images the preview shows: (page, device pixels per point), one per page placed on
    /// the sheet shown; the host answers with [`PrintDialog::set_preview_keys`].
    pub fn preview_requests(&self) -> Vec<(usize, f32)> {
        self.shown()
            .iter()
            .map(|it| {
                let across = if it.turn { it.dest[3] } else { it.dest[2] };
                let (pw, ph) = self.size_of(it.page);
                let scale = across / it.src[2].max(1.0);
                // A tile of a page at a large zoom: the whole page image stays reasonable.
                (it.page, scale.min(3000.0 / pw.max(ph).max(1.0)))
            })
            .collect()
    }

    /// Stored page images (keys) for [`PrintDialog::preview_requests`], in its order.
    pub fn set_preview_keys(&mut self, keys: Vec<Option<usize>>) {
        self.preview_keys = keys;
    }

    fn size_of(&self, i: usize) -> (f32, f32) {
        self.sizes.get(i).copied().unwrap_or((612.0, 792.0))
    }

    /// The chosen printer's paper in the chosen orientation (US Letter with ¼ inch margins
    /// when the printer's is unknown).
    fn paper(&self) -> Paper {
        let base = self
            .papers
            .get(self.printer)
            .copied()
            .flatten()
            .unwrap_or(Paper {
                size: (612.0, 792.0),
                area: [18.0, 18.0, 576.0, 756.0],
            });
        let portrait = base.size.0 <= base.size.1;
        let landscape = match self.orientation {
            Orientation::Portrait => false,
            Orientation::Landscape => true,
            Orientation::Auto => self.pages().first().is_some_and(|&i| {
                let (w, h) = self.size_of(i);
                w > h
            }),
        };
        if landscape == portrait {
            base.turned()
        } else {
            base
        }
    }

    /// The sheets printed (one copy).
    fn sheets(&self) -> Vec<crate::reader_impose::Sheet> {
        let paper = self.paper();
        let view = match self.range {
            Range::View => self.view.map(|v| v.1),
            _ => None,
        };
        let o = crate::reader_impose::PlanOptions {
            scaling: self.scaling,
            auto_rotate: self.rotate && self.handling != Handling::Tile,
            auto_center: self.center,
            layout: self.print_layout(),
            copies: 1,
            collate: true,
        };
        crate::reader_impose::plan(
            &self.pages(),
            &|i| self.size_of(i),
            view,
            (paper.area[2], paper.area[3]),
            &o,
        )
    }

    fn sheet_count(&self) -> usize {
        self.sheets().len().max(1)
    }

    /// The preview's paper (device pixels) and its pixels per point.
    fn paper_rect(&self) -> ([f32; 4], f32) {
        let paper = self.paper();
        let pv = self.preview_rect;
        let k = (pv[2] / paper.size.0.max(1.0)).min(pv[3] / paper.size.1.max(1.0));
        let (w, h) = ((paper.size.0 * k).round(), (paper.size.1 * k).round());
        (
            [
                (pv[0] + (pv[2] - w) / 2.0).round(),
                (pv[1] + (pv[3] - h) / 2.0).round(),
                w,
                h,
            ],
            k,
        )
    }

    /// Pages on the sheet shown, placed on the preview's paper.
    fn shown(&self) -> Vec<Shown> {
        let sheets = self.sheets();
        let Some(sheet) = sheets.get(self.preview_at.min(sheets.len().saturating_sub(1))) else {
            return Vec::new();
        };
        let paper = self.paper();
        let (pr, k) = self.paper_rect();
        sheet
            .items
            .iter()
            .map(|pl| {
                let (pw, ph) = self.size_of(pl.page);
                let d = pl.dest;
                Shown {
                    page: pl.page,
                    src: pl.clip.unwrap_or([0.0, 0.0, pw, ph]),
                    dest: [
                        pr[0] + (paper.area[0] + d[0]) * k,
                        pr[1] + (paper.area[1] + d[1]) * k,
                        d[2] * k,
                        d[3] * k,
                    ],
                    zoom: if pl.turn { d[3] } else { d[2] } / pl.clip.map_or(pw, |c| c[2]).max(1.0),
                    turn: pl.turn,
                    border: pl.border,
                    cut_marks: pl.cut_marks,
                    tile: pl.tile,
                }
            })
            .collect()
    }

    /// Items of a drop-down list.
    fn item_count(&self, part: Part) -> usize {
        match part {
            Part::Printer => self.printers.len(),
            Part::Subset | Part::Orientation | Part::BookletSubset => 3,
            Part::What | Part::PerSheet => 5,
            Part::Order => 4,
            Part::Binding => 2,
            _ => 0,
        }
    }

    /// The chosen item of a drop-down list.
    fn item_now(&self, part: Part) -> usize {
        match part {
            Part::Printer => self.printer,
            Part::Subset => usize::from(self.subset),
            Part::Orientation => self.orientation as usize,
            Part::What => WHAT.iter().position(|w| *w == self.what).unwrap_or(1),
            Part::PerSheet => PER_SHEET
                .iter()
                .position(|n| *n == self.per_sheet)
                .unwrap_or(0),
            Part::Order => ORDERS.iter().position(|o| *o == self.order).unwrap_or(0),
            Part::BookletSubset => SIDES
                .iter()
                .position(|o| *o == self.booklet_sides)
                .unwrap_or(0),
            Part::Binding => usize::from(self.right_binding),
            _ => 0,
        }
    }

    /// Label of item `i` of a drop-down list.
    fn item_label(&self, part: Part, i: usize, s: &Strings) -> String {
        let key = |keys: &[&str]| s.get(keys.get(i).copied().unwrap_or("")).to_owned();
        match part {
            Part::Printer => self.printers.get(i).cloned().unwrap_or_default(),
            Part::Subset => key(&[
                "pdfPrint.subsetAll",
                "pdfPrint.subsetOdd",
                "pdfPrint.subsetEven",
            ]),
            Part::Orientation => key(&[
                "pdfPrint.orientAuto",
                "pdfPrint.orientPortrait",
                "pdfPrint.orientLandscape",
            ]),
            Part::What => key(&[
                "pdfPrint.whatDoc",
                "pdfPrint.whatMarkups",
                "pdfPrint.whatStamps",
                "pdfPrint.whatForms",
                "pdfPrint.whatAnnots",
            ]),
            Part::PerSheet => PER_SHEET
                .get(i)
                .map(ToString::to_string)
                .unwrap_or_default(),
            Part::Order => key(&[
                "pdfPrint.orderH",
                "pdfPrint.orderHR",
                "pdfPrint.orderV",
                "pdfPrint.orderVR",
            ]),
            Part::BookletSubset => key(&[
                "pdfPrint.bothSides",
                "pdfPrint.frontOnly",
                "pdfPrint.backOnly",
            ]),
            Part::Binding => key(&["pdfPrint.left", "pdfPrint.right"]),
            _ => String::new(),
        }
    }

    /// Item `i` of the drop-down list of `part` chosen.
    fn choose(&mut self, part: Part, i: usize) {
        match part {
            Part::Printer if i < self.printers.len() => self.printer = i,
            Part::Subset => self.subset = i.min(2) as u8,
            Part::Orientation => {
                self.orientation = [
                    Orientation::Auto,
                    Orientation::Portrait,
                    Orientation::Landscape,
                ][i.min(2)];
            }
            Part::What => self.what = WHAT[i.min(4)],
            Part::PerSheet => self.per_sheet = PER_SHEET[i.min(4)],
            Part::Order => self.order = ORDERS[i.min(3)],
            Part::BookletSubset => self.booklet_sides = SIDES[i.min(2)],
            Part::Binding => self.right_binding = i == 1,
            _ => {}
        }
        self.preview_at = 0;
    }

    /// Pages to print, in order.
    pub fn pages(&self) -> Vec<usize> {
        let mut v: Vec<usize> = match self.range {
            Range::All => (0..self.count).collect(),
            Range::Current => vec![self.current],
            Range::View => vec![self.view.map_or(self.current, |v| v.0)],
            Range::Pages => {
                let s: String = self.pages_text.iter().collect();
                parse_pages(&s, self.count)
            }
        };
        // Subset by page number (1-based): odd or even.
        match self.subset {
            1 => v.retain(|p| p % 2 == 0),
            2 => v.retain(|p| p % 2 == 1),
            _ => {}
        }
        if self.reverse && self.range != Range::All {
            v.reverse();
        }
        v
    }

    fn settings(&self) -> Option<PrintSettings> {
        let pages = self.pages();
        if pages.is_empty() {
            return None;
        }
        Some(PrintSettings {
            printer: self.printers.get(self.printer)?.clone(),
            copies: self.copies.clamp(1, 999),
            collate: self.collate,
            pages,
            scaling: self.scaling,
            grayscale: self.gray,
            auto_rotate: self.rotate,
            auto_center: self.center,
            layout: self.print_layout(),
            duplex: match (self.duplex, self.flip_long) {
                (false, _) => Duplex::Off,
                (true, true) => Duplex::LongEdge,
                (true, false) => Duplex::ShortEdge,
            },
            view: match self.range {
                Range::View => self.view.map(|v| v.1),
                _ => None,
            },
            orientation: self.orientation,
            what: self.what,
            as_image: self.as_image,
            overprint: self.overprint,
            paper_by_page: self.paper_by_page,
        })
    }

    /// The Print Handling choice as a layout.
    fn print_layout(&self) -> PrintLayout {
        match self.handling {
            Handling::Scale => PrintLayout::Size,
            Handling::Multiple => PrintLayout::Multiple {
                per_sheet: self.per_sheet,
                order: self.order,
                border: self.border,
            },
            Handling::Booklet => PrintLayout::Booklet {
                sides: self.booklet_sides,
                right_binding: self.right_binding,
            },
            Handling::Tile => {
                let zoom: String = self.tile_zoom.iter().collect();
                let overlap: String = self.overlap.iter().collect();
                PrintLayout::Tile {
                    zoom: zoom.parse::<u16>().unwrap_or(100).clamp(1, 1000),
                    overlap: overlap.parse::<f32>().unwrap_or(0.0).clamp(0.0, 10.0) * 72.0,
                    cut_marks: self.cut_marks,
                    labels: self.labels,
                }
            }
        }
    }

    /// Whether "Current view" is offered.
    pub fn has_view(&self) -> bool {
        self.view.is_some()
    }

    /// Offer "Current view": the visible part (page points) of page `page`.
    pub fn set_view(&mut self, page: usize, rect: [f32; 4]) {
        self.view = Some((page, rect));
    }

    fn sync_scale(&mut self) {
        let v: String = self.scale_text.iter().collect();
        if let Ok(p) = v.parse::<u16>() {
            self.scaling = PrintScaling::Custom(p.clamp(1, 1000));
        }
    }

    fn px(&self, v: f32) -> f32 {
        (v * self.scale).round()
    }

    fn style(&self) -> TextStyle {
        TextStyle {
            size: self.px(11.2),
            bold: false,
        }
    }

    /// Lay out centred in a window of `size` device pixels at `scale`.
    pub fn layout(&mut self, _m: &dyn TextMeasure, size: (f32, f32), scale: f32) {
        self.layout_at(size, scale);
    }

    /// Lay out again (a Print Handling tab changed the controls) and answer `r`.
    fn relayout_after(&mut self, r: Option<PrintAction>) -> Option<PrintAction> {
        let (size, scale) = self.window;
        self.layout_at(size, scale);
        r
    }

    fn layout_at(&mut self, size: (f32, f32), scale: f32) {
        self.window = (size, scale);
        // The reference at 125 %: 1535 × 870; smaller windows get a smaller dialog.
        self.scale = scale.min(size.0 / 1250.0).min(size.1 / 712.0).max(0.5);
        let (w, h) = (self.px(1228.0), self.px(696.0));
        let f = [
            ((size.0 - w) / 2.0).round().max(0.0),
            ((size.1 - h) / 2.0).round().max(0.0),
            w,
            h,
        ];
        self.frame = f;
        self.parts.clear();
        let (x0, y0) = (f[0], f[1]);
        let at = |x: f32, y: f32, w: f32, h: f32| [x0 + x, y0 + y, w, h];
        let s = self.scale;
        let r = |x: f32, y: f32, w: f32, h: f32| at(x * s, y * s, w * s, h * s);
        self.parts.push((Part::Close, r(1192.0, 0.0, 36.0, 26.0)));
        self.parts.push((Part::Printer, r(92.0, 44.0, 293.0, 26.0)));
        // Measured on the reference at 125 %: Properties / Advanced 125 × 30 px from x 506 /
        // 656, Print as image and Bleed Marks at x 561 / 726, the Orientation, Print What and
        // Output groups at x 599, Page Setting at the bottom left.
        self.parts
            .push((Part::Properties, r(404.8, 44.0, 100.0, 24.0)));
        self.parts
            .push((Part::Advanced, r(524.8, 44.0, 100.0, 24.0)));
        self.parts
            .push((Part::AsImage, r(448.8, 84.0, 120.0, 20.0)));
        self.parts.push((Part::Bleed, r(580.8, 84.0, 100.0, 20.0)));
        self.parts
            .push((Part::Orientation, r(492.8, 404.0, 293.6, 26.4)));
        self.parts.push((Part::What, r(492.8, 492.0, 293.6, 26.4)));
        self.parts
            .push((Part::Summarize, r(492.8, 532.0, 168.0, 28.0)));
        self.parts
            .push((Part::Overprint, r(492.8, 612.0, 256.0, 20.0)));
        self.parts
            .push((Part::PageSetting, r(16.8, 657.6, 100.0, 28.0)));
        self.parts.push((Part::Slider, r(916.8, 612.0, 48.0, 24.0)));
        self.parts.push((Part::Copies, r(92.0, 80.0, 70.0, 26.0)));
        self.parts
            .push((Part::CopiesUp, r(162.0, 80.0, 14.0, 13.0)));
        self.parts
            .push((Part::CopiesDown, r(162.0, 93.0, 14.0, 13.0)));
        self.parts
            .push((Part::Collate, r(203.0, 84.0, 100.0, 20.0)));
        self.parts.push((Part::Gray, r(296.0, 84.0, 140.0, 20.0)));
        // 125 % reference: Current view at y 192, both sides at x 617, flip at 229 / 264, the
        // Print Handling tabs 123 × 51 px from x 27 / 167 / 307 / 447 at y 506.
        self.parts
            .push((Part::Range(Range::View), r(28.0, 144.0, 200.0, 20.0)));
        self.parts
            .push((Part::Range(Range::Current), r(28.0, 170.0, 200.0, 20.0)));
        self.parts
            .push((Part::Range(Range::All), r(28.0, 196.0, 200.0, 20.0)));
        self.parts
            .push((Part::Range(Range::Pages), r(28.0, 220.0, 80.0, 20.0)));
        self.parts
            .push((Part::PagesText, r(119.0, 220.0, 224.0, 27.0)));
        self.parts
            .push((Part::Subset, r(119.0, 282.0, 172.0, 26.0)));
        self.parts
            .push((Part::Reverse, r(28.0, 312.0, 200.0, 20.0)));
        self.parts
            .push((Part::Duplex, r(492.0, 144.0, 280.0, 20.0)));
        self.parts
            .push((Part::Flip(true), r(511.0, 173.0, 240.0, 20.0)));
        self.parts
            .push((Part::Flip(false), r(511.0, 201.0, 240.0, 20.0)));
        for (k, h) in [
            Handling::Scale,
            Handling::Tile,
            Handling::Multiple,
            Handling::Booklet,
        ]
        .into_iter()
        .enumerate()
        {
            self.parts.push((
                Part::Handling(h),
                r(21.6 + k as f32 * 112.0, 404.8, 98.4, 40.8),
            ));
        }
        match self.handling {
            Handling::Scale => {
                for (sc, y) in [
                    (PrintScaling::Actual, 464.0),
                    (PrintScaling::Fit, 494.0),
                    (PrintScaling::Shrink, 524.0),
                    (PrintScaling::Custom(0), 554.0),
                ] {
                    self.parts
                        .push((Part::Scaling(sc), r(28.0, y, 130.0, 20.0)));
                }
                self.parts
                    .push((Part::ScaleText, r(160.0, 551.0, 60.0, 26.0)));
                // Reference: (59, 880) px at 125 % in the dialog at (23, 150).
                self.parts
                    .push((Part::PaperByPage, r(28.8, 584.0, 360.0, 20.0)));
            }
            Handling::Multiple => {
                self.parts
                    .push((Part::PerSheet, r(160.0, 462.0, 120.0, 26.0)));
                self.parts.push((Part::Order, r(160.0, 498.0, 180.0, 26.0)));
                self.parts.push((Part::Border, r(28.0, 536.0, 240.0, 20.0)));
            }
            Handling::Booklet => {
                self.parts
                    .push((Part::BookletSubset, r(160.0, 462.0, 180.0, 26.0)));
                self.parts
                    .push((Part::Binding, r(160.0, 498.0, 120.0, 26.0)));
            }
            Handling::Tile => {
                // Reference: Page Zoom / Overlap boxes 124 × 34 px at x 209, spinners after.
                self.parts
                    .push((Part::TileZoom, r(167.2, 466.4, 99.2, 27.2)));
                self.parts
                    .push((Part::TileZoomUp, r(266.4, 466.4, 15.2, 13.6)));
                self.parts
                    .push((Part::TileZoomDown, r(266.4, 480.0, 15.2, 13.6)));
                self.parts
                    .push((Part::Overlap, r(167.2, 502.4, 99.2, 27.2)));
                self.parts
                    .push((Part::OverlapUp, r(266.4, 502.4, 15.2, 13.6)));
                self.parts
                    .push((Part::OverlapDown, r(266.4, 516.0, 15.2, 13.6)));
                self.parts
                    .push((Part::CutMarks, r(28.8, 544.0, 200.0, 20.0)));
                self.parts.push((Part::Labels, r(28.8, 582.4, 200.0, 20.0)));
            }
        }
        self.parts
            .push((Part::Rotate, r(492.0, 232.0, 200.0, 20.0)));
        self.parts
            .push((Part::Center, r(492.0, 264.0, 200.0, 20.0)));
        self.parts
            .push((Part::PrevPage, r(858.0, 613.0, 36.0, 24.0)));
        self.parts
            .push((Part::NextPage, r(998.0, 613.0, 36.0, 24.0)));
        self.parts.push((Part::Ok, r(994.0, 657.0, 95.0, 28.0)));
        self.parts
            .push((Part::Cancel, r(1110.0, 657.0, 95.0, 28.0)));
        self.preview_rect = r(818.0, 101.0, 377.0, 487.0);
        // The open drop-down list under its box.
        if let Some(d) = self.drop {
            let b = self.rect(d);
            let ih = (24.0 * s).round();
            for i in 0..self.item_count(d) {
                self.parts
                    .push((Part::Item(i), [b[0], b[1] + b[3] + i as f32 * ih, b[2], ih]));
            }
        }
    }

    fn rect(&self, part: Part) -> [f32; 4] {
        self.parts
            .iter()
            .find(|(p, _)| *p == part)
            .map_or([0.0; 4], |(_, r)| *r)
    }

    fn part_at(&self, x: f32, y: f32) -> Option<Part> {
        let inside = |r: &[f32; 4]| x >= r[0] && x < r[0] + r[2] && y >= r[1] && y < r[1] + r[3];
        if self.drop.is_some() {
            if let Some((p, _)) = self
                .parts
                .iter()
                .find(|(p, r)| matches!(p, Part::Item(_)) && inside(r))
            {
                return Some(*p);
            }
        }
        self.parts
            .iter()
            .find(|(p, r)| !matches!(p, Part::Item(_)) && inside(r))
            .map(|(p, _)| *p)
    }

    /// Whether a window position is over the dialog.
    pub fn contains(&self, x: f32, y: f32) -> bool {
        let f = self.frame;
        x >= f[0] && x < f[0] + f[2] && y >= f[1] && y < f[1] + f[3]
    }

    /// Pointer moved: `true` when the hover changed.
    pub fn hover(&mut self, x: f32, y: f32) -> bool {
        let h = self.part_at(x, y);
        std::mem::replace(&mut self.hover, h) != h
    }

    fn sync_copies(&mut self) {
        self.copies_text = self.copies.to_string().chars().collect();
    }

    /// A click.
    pub fn press(&mut self, x: f32, y: f32) -> Option<PrintAction> {
        let part = self.part_at(x, y);
        if let Some(open) = self.drop.take() {
            if let Some(Part::Item(i)) = part {
                self.choose(open, i);
            }
            return self.relayout_after(None);
        }
        match part? {
            p @ (Part::Printer
            | Part::Subset
            | Part::Orientation
            | Part::What
            | Part::PerSheet
            | Part::Order
            | Part::BookletSubset
            | Part::Binding) => {
                self.drop = Some(p);
                return self.relayout_after(None);
            }
            Part::Properties => {
                return self
                    .printers
                    .get(self.printer)
                    .map(|p| PrintAction::Properties(p.clone()));
            }
            // Not offered (drawn disabled).
            Part::Item(_) | Part::Advanced | Part::Bleed | Part::Summarize | Part::PageSetting => {}
            Part::AsImage => self.as_image = !self.as_image,
            Part::Overprint => self.overprint = !self.overprint,
            Part::CutMarks => self.cut_marks = !self.cut_marks,
            Part::PaperByPage => self.paper_by_page = !self.paper_by_page,
            Part::Labels => self.labels = !self.labels,
            Part::TileZoom => self.focus = Field::TileZoom,
            Part::Overlap => self.focus = Field::Overlap,
            p @ (Part::TileZoomUp | Part::TileZoomDown) => {
                let v: String = self.tile_zoom.iter().collect();
                let v = v.parse::<i32>().unwrap_or(100);
                let v = if p == Part::TileZoomUp { v + 1 } else { v - 1 };
                self.tile_zoom = v.clamp(1, 1000).to_string().chars().collect();
                self.preview_at = 0;
            }
            p @ (Part::OverlapUp | Part::OverlapDown) => {
                let v: String = self.overlap.iter().collect();
                let v = v.parse::<f32>().unwrap_or(0.0);
                let v = if p == Part::OverlapUp {
                    v + 0.1
                } else {
                    v - 0.1
                };
                self.overlap = format!("{:.3}", v.clamp(0.0, 10.0)).chars().collect();
                self.preview_at = 0;
            }
            Part::Slider => {
                let r = self.rect(Part::Slider);
                let n = self.sheet_count();
                let t = ((x - r[0]) / r[2].max(1.0)).clamp(0.0, 1.0);
                self.preview_at =
                    ((t * n.saturating_sub(1) as f32).round() as usize).min(n.saturating_sub(1));
            }
            Part::Copies => self.focus = Field::Copies,
            Part::CopiesUp => {
                self.copies = (self.copies + 1).min(999);
                self.sync_copies();
            }
            Part::CopiesDown => {
                self.copies = self.copies.saturating_sub(1).max(1);
                self.sync_copies();
            }
            // One copy: nothing to collate (disabled like the reference's).
            Part::Collate if self.copies <= 1 => {}
            Part::Collate => self.collate = !self.collate,
            Part::Gray => self.gray = !self.gray,
            Part::Range(Range::View) if self.view.is_none() => {}
            Part::Range(r) => {
                self.range = r;
                if r == Range::Pages {
                    self.focus = Field::Pages;
                }
                self.preview_at = 0;
            }
            Part::PagesText => {
                self.range = Range::Pages;
                self.focus = Field::Pages;
            }
            Part::Reverse if self.range == Range::All => {}
            Part::Reverse => self.reverse = !self.reverse,
            Part::Scaling(PrintScaling::Custom(_)) | Part::ScaleText => {
                self.focus = Field::Scale;
                self.sync_scale();
                if !matches!(self.scaling, PrintScaling::Custom(_)) {
                    self.scaling = PrintScaling::Custom(100);
                }
            }
            Part::Scaling(s) => self.scaling = s,
            Part::Duplex => self.duplex = !self.duplex,
            Part::Flip(long) => {
                if self.duplex {
                    self.flip_long = long;
                }
            }
            Part::Handling(h) => {
                self.handling = h;
                // A booklet is printed on both sides, flipped on the short edge.
                if h == Handling::Booklet {
                    self.duplex = true;
                    self.flip_long = false;
                }
                self.preview_at = 0;
                return self.relayout_after(None);
            }
            Part::Border => self.border = !self.border,
            Part::Rotate => self.rotate = !self.rotate,
            Part::Center => self.center = !self.center,
            Part::PrevPage => self.preview_at = self.preview_at.saturating_sub(1),
            Part::NextPage => {
                let n = self.sheet_count();
                self.preview_at = (self.preview_at + 1).min(n.saturating_sub(1));
            }
            Part::Ok => return self.settings().map(PrintAction::Print),
            Part::Cancel | Part::Close => return Some(PrintAction::Cancel),
        }
        None
    }

    /// Typed text: page list, custom scale or copies.
    pub fn text(&mut self, s: &str) {
        if self.focus == Field::TileZoom {
            self.tile_zoom
                .extend(s.chars().filter(char::is_ascii_digit));
            self.tile_zoom.truncate(4);
            self.preview_at = 0;
        } else if self.focus == Field::Overlap {
            self.overlap
                .extend(s.chars().filter(|c| c.is_ascii_digit() || *c == '.'));
            self.overlap.truncate(6);
            self.preview_at = 0;
        } else if self.focus == Field::Scale {
            self.scale_text
                .extend(s.chars().filter(char::is_ascii_digit));
            self.scale_text.truncate(4);
            self.sync_scale();
        } else if self.focus == Field::Pages {
            self.pages_text.extend(
                s.chars()
                    .filter(|c| c.is_ascii_digit() || matches!(c, ',' | '-' | ' ' | ';')),
            );
            self.preview_at = 0;
        } else {
            self.copies_text
                .extend(s.chars().filter(char::is_ascii_digit));
            self.copies_text.truncate(3);
            let v: String = self.copies_text.iter().collect();
            self.copies = v.parse().unwrap_or(1).clamp(1, 999);
        }
    }

    /// A key: Enter prints, Escape cancels, Backspace edits, Up / Down change the copies,
    /// Page Up / Down move the preview.
    pub fn key(&mut self, key: Key, _m: Mods) -> Option<PrintAction> {
        match key {
            Key::Enter => return self.settings().map(PrintAction::Print),
            Key::Escape => {
                if self.drop.take().is_some() {
                    return self.relayout_after(None);
                }
                return Some(PrintAction::Cancel);
            }
            Key::Backspace => {
                if self.focus == Field::TileZoom {
                    self.tile_zoom.pop();
                } else if self.focus == Field::Overlap {
                    self.overlap.pop();
                } else if self.focus == Field::Scale {
                    self.scale_text.pop();
                    self.sync_scale();
                } else if self.focus == Field::Pages {
                    self.pages_text.pop();
                } else {
                    self.copies_text.pop();
                    let v: String = self.copies_text.iter().collect();
                    self.copies = v.parse().unwrap_or(1).clamp(1, 999);
                }
            }
            Key::Up => {
                self.copies = (self.copies + 1).min(999);
                self.sync_copies();
            }
            Key::Down => {
                self.copies = self.copies.saturating_sub(1).max(1);
                self.sync_copies();
            }
            Key::PageDown => {
                let n = self.sheet_count();
                self.preview_at = (self.preview_at + 1).min(n.saturating_sub(1));
            }
            Key::PageUp => self.preview_at = self.preview_at.saturating_sub(1),
            Key::Tab => {
                self.focus = match self.focus {
                    Field::Copies => Field::Pages,
                    Field::Pages | Field::Scale | Field::TileZoom | Field::Overlap => Field::Copies,
                };
            }
            _ => {}
        }
        None
    }

    fn label(&self, p: &mut dyn Painter, text: &str, x: f32, y: f32, color: Rgba) {
        p.text(text, x, y, self.style(), color);
    }

    /// Text centred vertically in `r`, `dx` from its left.
    fn text_in(&self, p: &mut dyn Painter, text: &str, r: [f32; 4], dx: f32, color: Rgba) {
        let st = self.style();
        let y = r[1] + ((r[3] - st.size * 1.33) / 2.0).round();
        p.text(text, r[0] + dx, y, st, color);
    }

    fn radio(&self, p: &mut dyn Painter, r: [f32; 4], on: bool, label: &str) {
        let d = self.px(12.8);
        let (cx, cy) = (r[0] + d / 2.0 + 1.0, r[1] + r[3] / 2.0);
        p.path(
            &circle(cx, cy, d / 2.0),
            Some(WHITE),
            Some((
                if on { FOCUS } else { Rgba(51, 51, 51, 255) },
                self.scale.max(1.0),
            )),
        );
        if on {
            p.path(&circle(cx, cy, d / 4.0), Some(FOCUS), None);
        }
        self.text_in(p, label, r, d + self.px(6.4), TEXT);
    }

    fn check(&self, p: &mut dyn Painter, r: [f32; 4], on: bool, label: &str) {
        let b = self.px(12.8);
        let (bx, by) = (r[0] + 1.0, (r[1] + (r[3] - b) / 2.0).round());
        if on {
            p.fill_rect(bx, by, b, b, FOCUS);
            let k = b / 4.0;
            p.path(
                &[
                    PathSeg::M(bx + k * 0.9, by + b / 2.0),
                    PathSeg::L(bx + b / 2.0 - k / 4.0, by + b - k * 1.1),
                    PathSeg::L(bx + b - k * 0.9, by + k * 1.1),
                ],
                None,
                Some((WHITE, (1.4 * self.scale).max(1.2))),
            );
        } else {
            p.fill_rect(bx, by, b, b, Rgba(51, 51, 51, 255));
            p.fill_rect(bx + 1.0, by + 1.0, b - 2.0, b - 2.0, WHITE);
        }
        self.text_in(p, label, r, b + self.px(6.4), TEXT);
    }

    fn field(&self, p: &mut dyn Painter, r: [f32; 4], focused: bool, text: &str, caret: bool) {
        p.fill_rect(r[0], r[1], r[2], r[3], if focused { FOCUS } else { FIELD });
        p.fill_rect(r[0] + 1.0, r[1] + 1.0, r[2] - 2.0, r[3] - 2.0, FIELD_BG);
        p.push_clip(r[0] + 1.0, r[1] + 1.0, r[2] - 2.0, r[3] - 2.0);
        self.text_in(p, text, r, self.px(3.2), TEXT);
        if focused && caret {
            let st = self.style();
            let x = r[0] + self.px(3.2) + text.chars().count() as f32 * st.size * 0.55;
            p.fill_rect(
                x.round(),
                r[1] + self.px(5.0),
                1.0,
                r[3] - self.px(10.0),
                TEXT,
            );
        }
        p.pop_clip();
    }

    fn combo(&self, p: &mut dyn Painter, r: [f32; 4], text: &str, open: bool) {
        self.field(p, r, open, text, false);
        let (ax, ay, k) = (r[0] + r[2] - self.px(10.0), r[1] + r[3] / 2.0, self.px(3.2));
        p.path(
            &[
                PathSeg::M(ax - k, ay - k / 2.0),
                PathSeg::L(ax, ay + k / 2.0),
                PathSeg::L(ax + k, ay - k / 2.0),
            ],
            None,
            Some((TEXT, self.scale.max(1.0))),
        );
    }

    fn group(&self, p: &mut dyn Painter, title: &str, r: [f32; 4]) {
        let s = self.scale;
        let [x, y, w, h] = [
            self.frame[0] + r[0] * s,
            self.frame[1] + r[1] * s,
            r[2] * s,
            r[3] * s,
        ];
        p.fill_rect(x, y, w, 1.0, GROUP);
        p.fill_rect(x, y + h - 1.0, w, 1.0, GROUP);
        p.fill_rect(x, y, 1.0, h, GROUP);
        p.fill_rect(x + w - 1.0, y, 1.0, h, GROUP);
        if !title.is_empty() {
            self.label(p, title, x + self.px(6.4), y - self.px(13.0) + 1.0, TEXT);
        }
    }

    fn button(&self, p: &mut dyn Painter, part: Part, text: &str, default: bool) {
        let r = self.rect(part);
        let hover = self.hover == Some(part);
        let edge = if default { FOCUS } else { BUTTON_EDGE };
        p.fill_rect(r[0], r[1], r[2], r[3], edge);
        p.fill_rect(
            r[0] + 1.0,
            r[1] + 1.0,
            r[2] - 2.0,
            r[3] - 2.0,
            if hover { BUTTON_HOVER } else { BUTTON },
        );
        let st = self.style();
        let tw = text.chars().count() as f32 * st.size * 0.5;
        self.text_in(p, text, r, ((r[2] - tw) / 2.0).round(), TEXT);
    }

    /// Paint over a dimmed window of `window` size.
    #[allow(clippy::too_many_lines)]
    pub fn paint(
        &self,
        p: &mut dyn Painter,
        _t: &Theme,
        s: &Strings,
        window: (f32, f32),
        caret: bool,
    ) {
        p.fill_rect(0.0, 0.0, window.0, window.1, Rgba(0, 0, 0, 40));
        let f = self.frame;
        for (k, a) in [(8.0, 10u8), (4.0, 18), (1.0, 28)] {
            let o = self.px(k);
            p.fill_rect(
                f[0] - o,
                f[1] - o + self.px(3.0),
                f[2] + 2.0 * o,
                f[3] + 2.0 * o,
                Rgba(0, 0, 0, a),
            );
        }
        p.fill_rect(f[0], f[1], f[2], f[3], Rgba(0, 0, 0, 255));
        p.fill_rect(f[0] + 1.0, f[1] + 1.0, f[2] - 2.0, f[3] - 2.0, WHITE);
        // Caption.
        p.fill_rect(
            f[0] + 1.0,
            f[1] + 1.0,
            f[2] - 2.0,
            self.px(25.6) - 1.0,
            CAPTION,
        );
        self.label(
            p,
            s.get("pdfPrint.title"),
            f[0] + self.px(4.8),
            f[1] + self.px(5.6),
            TEXT,
        );
        let c = self.rect(Part::Close);
        let (cx, cy, k) = (c[0] + c[2] / 2.0, c[1] + c[3] / 2.0, self.px(5.6));
        if self.hover == Some(Part::Close) {
            p.fill_rect(c[0], c[1], c[2], c[3], Rgba(0xc4, 0x2b, 0x1c, 255));
        }
        let xc = if self.hover == Some(Part::Close) {
            WHITE
        } else {
            TEXT
        };
        p.path(
            &[
                PathSeg::M(cx - k, cy - k),
                PathSeg::L(cx + k, cy + k),
                PathSeg::M(cx + k, cy - k),
                PathSeg::L(cx - k, cy + k),
            ],
            None,
            Some((xc, self.scale.max(1.0))),
        );
        // Printer and copies.
        self.group(p, "", [17.0, 35.0, 781.0, 81.0]);
        let pr = self.rect(Part::Printer);
        self.text_in(
            p,
            s.get("pdfPrint.name"),
            [f[0], pr[1], 0.0, pr[3]],
            self.px(28.8),
            TEXT,
        );
        let name = self
            .printers
            .get(self.printer)
            .map_or_else(|| s.get("pdfPrint.noPrinter").to_owned(), Clone::clone);
        self.combo(p, pr, &name, self.drop == Some(Part::Printer));
        self.button(p, Part::Properties, s.get("pdfPrint.properties"), false);
        self.button_off(p, Part::Advanced, s.get("pdfPrint.advanced"));
        self.check(
            p,
            self.rect(Part::AsImage),
            self.as_image,
            s.get("pdfPrint.asImage"),
        );
        self.check_off(p, self.rect(Part::Bleed), s.get("pdfPrint.bleed"));
        let cp = self.rect(Part::Copies);
        self.text_in(
            p,
            s.get("pdfPrint.copies"),
            [f[0], cp[1], 0.0, cp[3]],
            self.px(28.8),
            TEXT,
        );
        let copies: String = self.copies_text.iter().collect();
        self.field(p, cp, self.focus == Field::Copies, &copies, caret);
        for (part, up) in [(Part::CopiesUp, true), (Part::CopiesDown, false)] {
            let r = self.rect(part);
            p.fill_rect(r[0], r[1], r[2], r[3], FIELD);
            p.fill_rect(r[0] + 1.0, r[1] + 1.0, r[2] - 2.0, r[3] - 2.0, BUTTON);
            let (mx, my, k) = (r[0] + r[2] / 2.0, r[1] + r[3] / 2.0, self.px(2.4));
            let dy = if up { -k / 2.0 } else { k / 2.0 };
            p.path(
                &[
                    PathSeg::M(mx - k, my - dy),
                    PathSeg::L(mx, my + dy),
                    PathSeg::L(mx + k, my - dy),
                ],
                None,
                Some((TEXT, self.scale.max(1.0))),
            );
        }
        if self.copies > 1 {
            self.check(
                p,
                self.rect(Part::Collate),
                self.collate,
                s.get("pdfPrint.collate"),
            );
        } else {
            self.check_off(p, self.rect(Part::Collate), s.get("pdfPrint.collate"));
        }
        self.check(
            p,
            self.rect(Part::Gray),
            self.gray,
            s.get("pdfPrint.grayscale"),
        );
        // Print Range.
        self.group(p, s.get("pdfPrint.range"), [17.0, 132.0, 449.0, 235.0]);
        let view = self.rect(Part::Range(Range::View));
        if self.view.is_some() {
            self.radio(
                p,
                view,
                self.range == Range::View,
                s.get("pdfPrint.currentView"),
            );
        } else {
            self.radio_off(p, view, s.get("pdfPrint.currentView"));
        }
        self.radio(
            p,
            self.rect(Part::Range(Range::Current)),
            self.range == Range::Current,
            s.get("pdfPrint.current"),
        );
        self.radio(
            p,
            self.rect(Part::Range(Range::All)),
            self.range == Range::All,
            s.get("pdfPrint.all"),
        );
        self.radio(
            p,
            self.rect(Part::Range(Range::Pages)),
            self.range == Range::Pages,
            s.get("pdfPrint.pages"),
        );
        let pt = self.rect(Part::PagesText);
        let pages_text: String = self.pages_text.iter().collect();
        if self.range == Range::Pages {
            self.field(p, pt, self.focus == Field::Pages, &pages_text, caret);
        } else {
            p.fill_rect(pt[0], pt[1], pt[2], pt[3], GROUP);
            p.fill_rect(pt[0] + 1.0, pt[1] + 1.0, pt[2] - 2.0, pt[3] - 2.0, WHITE);
            self.text_in(p, &pages_text, pt, self.px(6.4), GREY_TEXT);
        }
        self.text_in(
            p,
            &format!("/ {}", self.count),
            [pt[0] + pt[2], pt[1], 0.0, pt[3]],
            self.px(6.4),
            TEXT,
        );
        self.label(
            p,
            s.get("pdfPrint.sample"),
            pt[0],
            pt[1] + pt[3] + self.px(8.0),
            TEXT,
        );
        let sb = self.rect(Part::Subset);
        self.text_in(
            p,
            s.get("pdfPrint.subset"),
            [f[0], sb[1], 0.0, sb[3]],
            self.px(28.8),
            TEXT,
        );
        let subset = [
            "pdfPrint.subsetAll",
            "pdfPrint.subsetOdd",
            "pdfPrint.subsetEven",
        ][usize::from(self.subset.min(2))];
        self.combo(p, sb, s.get(subset), self.drop == Some(Part::Subset));
        if self.range == Range::All {
            self.check_off(p, self.rect(Part::Reverse), s.get("pdfPrint.reverse"));
        } else {
            self.check(
                p,
                self.rect(Part::Reverse),
                self.reverse,
                s.get("pdfPrint.reverse"),
            );
        }
        // Print Handling.
        self.group(p, s.get("pdfPrint.handling"), [17.0, 386.0, 449.0, 258.0]);
        self.paint_handling(p, s, caret);
        // Both sides, rotate and centre (an untitled group like the reference's).
        self.group(p, "", [479.0, 132.0, 316.0, 235.0]);
        self.check(
            p,
            self.rect(Part::Duplex),
            self.duplex,
            s.get("pdfPrint.duplex"),
        );
        for (long, key) in [(true, "pdfPrint.flipLong"), (false, "pdfPrint.flipShort")] {
            let r = self.rect(Part::Flip(long));
            if self.duplex {
                self.radio(p, r, self.flip_long == long, s.get(key));
            } else {
                self.radio_off(p, r, s.get(key));
            }
        }
        if self.handling == Handling::Tile {
            self.check_off(p, self.rect(Part::Rotate), s.get("pdfPrint.autoRotate"));
        } else {
            self.check(
                p,
                self.rect(Part::Rotate),
                self.rotate,
                s.get("pdfPrint.autoRotate"),
            );
        }
        self.check(
            p,
            self.rect(Part::Center),
            self.center,
            s.get("pdfPrint.autoCenter"),
        );
        // Orientation, Print What, Output (reference: groups at x 599 under the duplex group).
        self.group(
            p,
            s.get("pdfPrint.orientation"),
            [479.2, 377.6, 316.0, 74.4],
        );
        let orient = self.item_label(Part::Orientation, self.item_now(Part::Orientation), s);
        self.combo(
            p,
            self.rect(Part::Orientation),
            &orient,
            self.drop == Some(Part::Orientation),
        );
        self.group(p, s.get("pdfPrint.what"), [479.2, 464.0, 316.0, 108.0]);
        let what = self.item_label(Part::What, self.item_now(Part::What), s);
        self.combo(
            p,
            self.rect(Part::What),
            &what,
            self.drop == Some(Part::What),
        );
        self.button_off(p, Part::Summarize, s.get("pdfPrint.summarize"));
        self.group(p, s.get("pdfPrint.output"), [479.2, 580.0, 316.0, 68.0]);
        self.check(
            p,
            self.rect(Part::Overprint),
            self.overprint,
            s.get("pdfPrint.overprint"),
        );
        self.button_off(p, Part::PageSetting, s.get("pdfPrint.pageSetting"));
        self.paint_preview(p, s);
        // Buttons.
        self.button(p, Part::Ok, s.get("pdfPrint.ok"), true);
        self.button(p, Part::Cancel, s.get("pdfPrint.cancel"), false);
        // The open drop-down list on top.
        if let Some(d) = self.drop {
            let items: Vec<(usize, [f32; 4])> = self
                .parts
                .iter()
                .filter_map(|(p, r)| match p {
                    Part::Item(i) => Some((*i, *r)),
                    _ => None,
                })
                .collect();
            if let (Some(first), Some(last)) = (items.first(), items.last()) {
                let (x, y) = (first.1[0], first.1[1]);
                let h = last.1[1] + last.1[3] - y;
                p.fill_rect(x, y, first.1[2], h, FIELD);
                p.fill_rect(x + 1.0, y, first.1[2] - 2.0, h - 1.0, WHITE);
            }
            let now = self.item_now(d);
            for (i, r) in items {
                if self.hover == Some(Part::Item(i)) || i == now {
                    p.fill_rect(r[0] + 1.0, r[1], r[2] - 2.0, r[3] - 1.0, ITEM_HOVER);
                }
                self.text_in(p, &self.item_label(d, i, s), r, self.px(4.8), TEXT);
            }
        }
    }
}

impl PrintDialog {
    /// The Preview group: zoom, document and paper sizes, the sheet shown with its pages,
    /// the page buttons, slider and "Page n of m".
    #[allow(clippy::too_many_lines)]
    fn paint_preview(&self, p: &mut dyn Painter, s: &Strings) {
        let sc = self.scale;
        let f = self.frame;
        let at = |x: f32, y: f32| (f[0] + (x * sc).round(), f[1] + (y * sc).round());
        // Reference: group (1011, 35)–(1508, 813) px at 125 %, a rule above the page buttons.
        self.group(p, s.get("pdfPrint.preview"), [808.8, 28.0, 397.6, 622.4]);
        let (lx, ly) = at(808.8, 598.4);
        p.fill_rect(lx, ly, (397.6 * sc).round(), 1.0, GROUP);
        let shown = self.shown();
        let paper = self.paper();
        let first = self.pages().first().copied().unwrap_or(0);
        let (dw, dh) = self.size_of(first);
        let inch = |v: f32| format!("{:.1}", v / 72.0);
        let (x, y) = at(816.8, 53.6);
        self.label(p, s.get("pdfPrint.zoom"), x, y, TEXT);
        if let Some(z) = shown.first().map(|it| it.zoom) {
            let (x, y) = at(894.4, 53.6);
            self.label(p, &format!("{:.2}%", z * 100.0), x, y, TEXT);
        }
        let (x, y) = at(816.8, 76.0);
        self.label(p, &format!("{}:", s.get("pdfPrint.document")), x, y, TEXT);
        let (x, y) = at(891.2, 76.0);
        self.label(p, &format!("{} x {} inch", inch(dw), inch(dh)), x, y, TEXT);
        let (x, y) = at(1000.8, 76.0);
        self.label(p, s.get("pdfPrint.paper"), x, y, TEXT);
        let (x, y) = at(1060.8, 76.0);
        self.label(
            p,
            &format!("{} x {} inch", inch(paper.size.0), inch(paper.size.1)),
            x,
            y,
            TEXT,
        );
        // The paper: a dark frame like the reference's.
        let (pr, _) = self.paper_rect();
        let fw = sc.max(1.0).round();
        p.fill_rect(
            pr[0] - fw,
            pr[1] - fw,
            pr[2] + 2.0 * fw,
            pr[3] + 2.0 * fw,
            PAGE_FRAME,
        );
        p.fill_rect(pr[0], pr[1], pr[2], pr[3], WHITE);
        p.push_clip(pr[0], pr[1], pr[2], pr[3]);
        let requests = self.preview_requests();
        for (n, it) in shown.iter().enumerate() {
            let d = it.dest;
            let key = self.preview_keys.get(n).copied().flatten();
            if let (Some(key), Some(&(_, k))) = (key, requests.get(n)) {
                let src = [it.src[0] * k, it.src[1] * k, it.src[2] * k, it.src[3] * k];
                if it.turn {
                    let (cx, cy) = (d[0] + d[2] / 2.0, d[1] + d[3] / 2.0);
                    let upright = [cx - d[3] / 2.0, cy - d[2] / 2.0, d[3], d[2]];
                    p.page_part_rotated(key, src, upright, 1.0, 90.0);
                } else {
                    p.page_part(key, src, d, 1.0);
                }
            } else {
                p.fill_rect(d[0], d[1], d[2], d[3], GROUP);
                p.fill_rect(d[0] + 1.0, d[1] + 1.0, d[2] - 2.0, d[3] - 2.0, CAPTION);
            }
            if it.border {
                for r in [
                    [d[0], d[1], d[2], 1.0],
                    [d[0], d[1] + d[3] - 1.0, d[2], 1.0],
                    [d[0], d[1], 1.0, d[3]],
                    [d[0] + d[2] - 1.0, d[1], 1.0, d[3]],
                ] {
                    p.fill_rect(r[0], r[1], r[2], r[3], TEXT);
                }
            }
            if it.cut_marks {
                let m = self.px(6.0);
                for (cx, cy, sx, sy) in [
                    (d[0], d[1], 1.0, 1.0),
                    (d[0] + d[2], d[1], -1.0, 1.0),
                    (d[0], d[1] + d[3], 1.0, -1.0),
                    (d[0] + d[2], d[1] + d[3], -1.0, -1.0),
                ] {
                    p.path(
                        &[
                            PathSeg::M(cx + sx * m, cy),
                            PathSeg::L(cx, cy),
                            PathSeg::L(cx, cy + sy * m),
                        ],
                        None,
                        Some((TEXT, 1.0)),
                    );
                }
            }
            if let Some((row, col)) = it.tile {
                let label = s
                    .get("pdfPrint.tileLabel")
                    .replace("{{page}}", &(it.page + 1).to_string())
                    .replace("{{row}}", &row.to_string())
                    .replace("{{col}}", &col.to_string());
                let st = TextStyle {
                    size: self.px(8.0),
                    bold: false,
                };
                p.text(&label, d[0] + 2.0, d[1] + 1.0, st, TEXT);
            }
        }
        p.pop_clip();
        // Page buttons, slider and position.
        let n = self.sheet_count();
        let at_n = self.preview_at.min(n - 1) + 1;
        for (part, left) in [(Part::PrevPage, true), (Part::NextPage, false)] {
            let r = self.rect(part);
            let enabled = if left { at_n > 1 } else { at_n < n };
            p.fill_rect(r[0], r[1], r[2], r[3], BUTTON_EDGE);
            p.fill_rect(r[0] + 1.0, r[1] + 1.0, r[2] - 2.0, r[3] - 2.0, BUTTON);
            let (mx, my, k) = (r[0] + r[2] / 2.0, r[1] + r[3] / 2.0, self.px(3.2));
            let dx = if left { k / 2.0 } else { -k / 2.0 };
            p.path(
                &[
                    PathSeg::M(mx + dx, my - k),
                    PathSeg::L(mx - dx, my),
                    PathSeg::L(mx + dx, my + k),
                ],
                None,
                Some((if enabled { TEXT } else { GREY_TEXT }, sc.max(1.0))),
            );
        }
        // Slider (reference: a 12 × 25 px blue thumb on a grey track 60 px long).
        let r = self.rect(Part::Slider);
        let cy = (r[1] + r[3] / 2.0).round();
        p.fill_rect(r[0], cy - 1.0, r[2], 2.0, Rgba(204, 204, 204, 255));
        let tw = self.px(9.6);
        let t = if n > 1 {
            (at_n - 1) as f32 / (n - 1) as f32
        } else {
            0.0
        };
        p.fill_rect(
            (r[0] + t * (r[2] - tw)).round(),
            r[1] + self.px(2.0),
            tw,
            r[3] - self.px(4.0),
            FOCUS,
        );
        let nr = self.rect(Part::NextPage);
        let page_of = s
            .get(if self.handling == Handling::Scale {
                "pdfPrint.pageOf"
            } else {
                "pdfPrint.sheetOf"
            })
            .replace("{{n}}", &at_n.to_string())
            .replace("{{total}}", &n.to_string());
        self.text_in(
            p,
            &page_of,
            [nr[0] + nr[2], nr[1], 0.0, nr[3]],
            self.px(48.0),
            TEXT,
        );
    }

    /// A button that is not offered: grey label.
    fn button_off(&self, p: &mut dyn Painter, part: Part, text: &str) {
        let r = self.rect(part);
        p.fill_rect(r[0], r[1], r[2], r[3], Rgba(0xe0, 0xe0, 0xe0, 255));
        p.fill_rect(r[0] + 1.0, r[1] + 1.0, r[2] - 2.0, r[3] - 2.0, BUTTON);
        let st = self.style();
        let tw = text.chars().count() as f32 * st.size * 0.5;
        self.text_in(p, text, r, ((r[2] - tw) / 2.0).round(), GREY_TEXT);
    }

    /// A check box that is not offered: grey box and label.
    fn check_off(&self, p: &mut dyn Painter, r: [f32; 4], label: &str) {
        let b = self.px(12.8);
        let (bx, by) = (r[0] + 1.0, (r[1] + (r[3] - b) / 2.0).round());
        p.fill_rect(bx, by, b, b, GROUP);
        p.fill_rect(
            bx + 1.0,
            by + 1.0,
            b - 2.0,
            b - 2.0,
            Rgba(249, 249, 249, 255),
        );
        self.text_in(p, label, r, b + self.px(6.4), GREY_TEXT);
    }

    /// Up / down arrows of a number box.
    fn spinner(&self, p: &mut dyn Painter, part: Part, up: bool) {
        let r = self.rect(part);
        p.fill_rect(r[0], r[1], r[2], r[3], FIELD);
        p.fill_rect(r[0] + 1.0, r[1] + 1.0, r[2] - 2.0, r[3] - 2.0, BUTTON);
        let (mx, my, k) = (r[0] + r[2] / 2.0, r[1] + r[3] / 2.0, self.px(2.4));
        let dy = if up { -k / 2.0 } else { k / 2.0 };
        p.path(
            &[
                PathSeg::M(mx - k, my - dy),
                PathSeg::L(mx, my + dy),
                PathSeg::L(mx + k, my - dy),
            ],
            None,
            Some((TEXT, self.scale.max(1.0))),
        );
    }

    fn radio_off(&self, p: &mut dyn Painter, r: [f32; 4], label: &str) {
        let d = self.px(12.8);
        let (cx, cy) = (r[0] + d / 2.0 + 1.0, r[1] + r[3] / 2.0);
        p.path(
            &circle(cx, cy, d / 2.0),
            Some(Rgba(249, 249, 249, 255)),
            Some((GROUP, self.scale.max(1.0))),
        );
        self.text_in(p, label, r, d + self.px(6.4), GREY_TEXT);
    }

    /// A tab button of the Print Handling group (measured: selected 0,84,153 frame on
    /// 204,228,247; others 208,208,208 on 253,253,253), label centred on one or two lines.
    fn tab_button(&self, p: &mut dyn Painter, part: Part, label: &str, on: bool) {
        let r = self.rect(part);
        let (edge, fill) = if on {
            (Rgba(0, 84, 153, 255), Rgba(204, 228, 247, 255))
        } else if self.hover == Some(part) {
            (BUTTON_EDGE, BUTTON_HOVER)
        } else {
            (Rgba(208, 208, 208, 255), BUTTON)
        };
        p.fill_rect(r[0], r[1], r[2], r[3], edge);
        p.fill_rect(r[0] + 1.0, r[1] + 1.0, r[2] - 2.0, r[3] - 2.0, fill);
        let st = self.style();
        let lines: Vec<String> = if label.chars().count() > 14 {
            // Wrap at the space nearest the middle.
            let mid = label.len() / 2;
            let cut = label
                .match_indices(' ')
                .map(|(i, _)| i)
                .min_by_key(|i| i.abs_diff(mid));
            match cut {
                Some(i) => vec![label[..i].to_owned(), label[i + 1..].to_owned()],
                None => vec![label.to_owned()],
            }
        } else {
            vec![label.to_owned()]
        };
        let lh = (st.size * 1.33).round();
        let top = r[1] + ((r[3] - lh * lines.len() as f32) / 2.0).round();
        for (k, line) in lines.iter().enumerate() {
            let tw = line.chars().count() as f32 * st.size * 0.5;
            self.label(
                p,
                line,
                (r[0] + (r[2] - tw) / 2.0).round(),
                top + lh * k as f32,
                TEXT,
            );
        }
    }

    /// Print Handling: tab buttons and the chosen tab's options.
    fn paint_handling(&self, p: &mut dyn Painter, s: &Strings, caret: bool) {
        for (h, key) in [
            (Handling::Scale, "pdfPrint.tabScale"),
            (Handling::Tile, "pdfPrint.tabTile"),
            (Handling::Multiple, "pdfPrint.tabMultiple"),
            (Handling::Booklet, "pdfPrint.tabBooklet"),
        ] {
            self.tab_button(p, Part::Handling(h), s.get(key), self.handling == h);
        }
        let caption = |p: &mut dyn Painter, part: Part, key: &str| {
            let r = self.rect(part);
            self.text_in(
                p,
                s.get(key),
                [self.frame[0], r[1], 0.0, r[3]],
                self.px(28.8),
                TEXT,
            );
        };
        match self.handling {
            Handling::Scale => {
                for (sc, key) in [
                    (PrintScaling::Actual, "pdfPrint.none"),
                    (PrintScaling::Fit, "pdfPrint.fit"),
                    (PrintScaling::Shrink, "pdfPrint.shrink"),
                ] {
                    self.radio(
                        p,
                        self.rect(Part::Scaling(sc)),
                        self.scaling == sc,
                        s.get(key),
                    );
                }
                let custom = matches!(self.scaling, PrintScaling::Custom(_));
                self.radio(
                    p,
                    self.rect(Part::Scaling(PrintScaling::Custom(0))),
                    custom,
                    s.get("pdfPrint.custom"),
                );
                let r = self.rect(Part::ScaleText);
                let text: String = self.scale_text.iter().collect();
                self.field(p, r, custom && self.focus == Field::Scale, &text, caret);
                self.text_in(p, "%", [r[0] + r[2], r[1], 0.0, r[3]], self.px(6.4), TEXT);
                self.check(
                    p,
                    self.rect(Part::PaperByPage),
                    self.paper_by_page,
                    s.get("pdfPrint.paperByPage"),
                );
            }
            Handling::Multiple => {
                caption(p, Part::PerSheet, "pdfPrint.perSheet");
                self.combo(
                    p,
                    self.rect(Part::PerSheet),
                    &self.per_sheet.to_string(),
                    self.drop == Some(Part::PerSheet),
                );
                caption(p, Part::Order, "pdfPrint.order");
                let order = match self.order {
                    PageOrder::Horizontal => "pdfPrint.orderH",
                    PageOrder::HorizontalReversed => "pdfPrint.orderHR",
                    PageOrder::Vertical => "pdfPrint.orderV",
                    PageOrder::VerticalReversed => "pdfPrint.orderVR",
                };
                self.combo(
                    p,
                    self.rect(Part::Order),
                    s.get(order),
                    self.drop == Some(Part::Order),
                );
                self.check(
                    p,
                    self.rect(Part::Border),
                    self.border,
                    s.get("pdfPrint.border"),
                );
            }
            Handling::Booklet => {
                caption(p, Part::BookletSubset, "pdfPrint.bookletSubset");
                let sides = match self.booklet_sides {
                    BookletSides::Both => "pdfPrint.bothSides",
                    BookletSides::Front => "pdfPrint.frontOnly",
                    BookletSides::Back => "pdfPrint.backOnly",
                };
                self.combo(
                    p,
                    self.rect(Part::BookletSubset),
                    s.get(sides),
                    self.drop == Some(Part::BookletSubset),
                );
                caption(p, Part::Binding, "pdfPrint.binding");
                let binding = if self.right_binding {
                    "pdfPrint.right"
                } else {
                    "pdfPrint.left"
                };
                self.combo(
                    p,
                    self.rect(Part::Binding),
                    s.get(binding),
                    self.drop == Some(Part::Binding),
                );
            }
            Handling::Tile => {
                caption(p, Part::TileZoom, "pdfPrint.tileZoom");
                let r = self.rect(Part::TileZoom);
                let zoom: String = self.tile_zoom.iter().collect();
                self.field(p, r, self.focus == Field::TileZoom, &zoom, caret);
                self.spinner(p, Part::TileZoomUp, true);
                self.spinner(p, Part::TileZoomDown, false);
                let up = self.rect(Part::TileZoomUp);
                self.text_in(
                    p,
                    "%",
                    [up[0] + up[2], r[1], 0.0, r[3]],
                    self.px(12.8),
                    TEXT,
                );
                caption(p, Part::Overlap, "pdfPrint.overlap");
                let r = self.rect(Part::Overlap);
                let overlap: String = self.overlap.iter().collect();
                self.field(p, r, self.focus == Field::Overlap, &overlap, caret);
                self.spinner(p, Part::OverlapUp, true);
                self.spinner(p, Part::OverlapDown, false);
                self.text_in(
                    p,
                    s.get("pdfPrint.inch"),
                    [up[0] + up[2], r[1], 0.0, r[3]],
                    self.px(12.8),
                    TEXT,
                );
                self.check(
                    p,
                    self.rect(Part::CutMarks),
                    self.cut_marks,
                    s.get("pdfPrint.cutMarks"),
                );
                self.check(
                    p,
                    self.rect(Part::Labels),
                    self.labels,
                    s.get("pdfPrint.labels"),
                );
            }
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixed;
    impl TextMeasure for Fixed {
        fn measure(&self, text: &str, style: TextStyle) -> (f32, f32) {
            (text.chars().count() as f32 * style.size * 0.5, style.size)
        }
    }

    fn centre(d: &PrintDialog, part: Part) -> (f32, f32) {
        let r = d.rect(part);
        (r[0] + r[2] / 2.0, r[1] + r[3] / 2.0)
    }

    #[test]
    fn page_lists() {
        assert_eq!(parse_pages("1-3, 5", 10), vec![0, 1, 2, 4]);
        assert_eq!(parse_pages("8-", 10), vec![7, 8, 9]);
        assert_eq!(parse_pages("4-2;x;12", 10), vec![1, 2, 3]);
        assert_eq!(parse_pages("", 3), Vec::<usize>::new());
    }

    #[test]
    fn view_duplex_custom_scale_multiple_and_booklet() {
        let mut d = PrintDialog::new(vec!["A".into()], None, 12, 2);
        d.set_view(2, [10.0, 20.0, 300.0, 200.0]);
        d.layout(&Fixed, (1600.0, 1000.0), 1.25);
        let click = |d: &mut PrintDialog, part| {
            let (x, y) = centre(d, part);
            d.press(x, y)
        };
        click(&mut d, Part::Range(Range::View));
        click(&mut d, Part::Duplex);
        click(&mut d, Part::Flip(false));
        click(&mut d, Part::Scaling(PrintScaling::Custom(0)));
        for _ in 0..3 {
            d.key(Key::Backspace, Mods::default());
        }
        d.text("75");
        let s = d.settings().unwrap();
        assert_eq!(s.pages, vec![2]);
        assert_eq!(s.view, Some([10.0, 20.0, 300.0, 200.0]));
        assert_eq!(s.duplex, Duplex::ShortEdge);
        assert_eq!(s.scaling, PrintScaling::Custom(75));
        // Multiple pages per sheet: 2 → 4, order horizontal → reversed, border.
        click(&mut d, Part::Range(Range::All));
        click(&mut d, Part::Handling(Handling::Multiple));
        click(&mut d, Part::PerSheet);
        click(&mut d, Part::Item(1));
        click(&mut d, Part::Order);
        click(&mut d, Part::Item(1));
        click(&mut d, Part::Border);
        assert_eq!(
            d.settings().unwrap().layout,
            PrintLayout::Multiple {
                per_sheet: 4,
                order: PageOrder::HorizontalReversed,
                border: true,
            }
        );
        assert_eq!(d.sheet_count(), 3);
        // Booklet: both sides, short edge; right binding.
        click(&mut d, Part::Handling(Handling::Booklet));
        click(&mut d, Part::Binding);
        click(&mut d, Part::Item(1));
        let s = d.settings().unwrap();
        assert_eq!(
            s.layout,
            PrintLayout::Booklet {
                sides: BookletSides::Both,
                right_binding: true,
            }
        );
        assert_eq!(s.duplex, Duplex::ShortEdge);
        // 12 pages: 3 sheets, 6 sides.
        assert_eq!(d.sheet_count(), 6);
    }

    #[test]
    fn tile_orientation_print_what_and_preview() {
        let mut d = PrintDialog::new(vec!["A".into(), "B".into()], None, 2, 0);
        d.set_page_sizes(vec![(1224.0, 1584.0), (612.0, 792.0)]);
        d.set_papers(vec![
            Some(Paper {
                size: (612.0, 792.0),
                area: [18.0, 18.0, 576.0, 756.0],
            }),
            None,
        ]);
        d.layout(&Fixed, (1600.0, 1000.0), 1.25);
        let click = |d: &mut PrintDialog, part| {
            let (x, y) = centre(d, part);
            d.press(x, y)
        };
        // Fit to the printable area: 576 / 612 of the page.
        let req = d.preview_requests();
        assert_eq!(req.len(), 1);
        assert!((d.shown()[0].zoom - 576.0 / 1224.0).abs() < 1e-3);
        // Tile Large Pages at 100 %: the 17 × 22 inch page on 3 × 3 sheets, the letter page
        // (larger than the printable area) on 2 × 2.
        click(&mut d, Part::Handling(Handling::Tile));
        click(&mut d, Part::Labels);
        assert_eq!(d.sheet_count(), 13);
        assert_eq!(d.shown()[0].tile, Some((1, 1)));
        // Landscape, Annotations only, overprint, as image.
        click(&mut d, Part::Orientation);
        click(&mut d, Part::Item(2));
        assert_eq!(d.paper().size, (792.0, 612.0));
        click(&mut d, Part::What);
        click(&mut d, Part::Item(4));
        click(&mut d, Part::Overprint);
        click(&mut d, Part::AsImage);
        let s = d.settings().unwrap();
        assert_eq!(s.orientation, Orientation::Landscape);
        assert_eq!(s.what, PrintWhat::Annotations);
        assert!(s.overprint && s.as_image);
        assert!(matches!(
            s.layout,
            PrintLayout::Tile {
                zoom: 100,
                labels: true,
                ..
            }
        ));
        // Properties asks the host for the chosen printer's driver settings.
        assert_eq!(
            click(&mut d, Part::Properties),
            Some(PrintAction::Properties("A".into()))
        );
        // Keys of the preview images come back from the host.
        d.set_preview_keys(vec![Some(7)]);
        assert_eq!(d.preview_keys, vec![Some(7)]);
    }

    #[test]
    fn same_size_as_the_reference_at_125_percent() {
        let mut d = PrintDialog::new(vec!["A".into()], None, 3, 0);
        d.layout(&Fixed, (1920.0, 1080.0), 1.25);
        assert_eq!((d.frame[2], d.frame[3]), (1535.0, 870.0));
    }

    #[test]
    fn choose_printer_range_copies_and_print() {
        let mut d = PrintDialog::new(vec!["A".into(), "B".into()], Some("B"), 12, 4);
        d.layout(&Fixed, (1600.0, 1000.0), 1.25);
        let (x, y) = centre(&d, Part::Printer);
        assert_eq!(d.press(x, y), None);
        d.layout(&Fixed, (1600.0, 1000.0), 1.25);
        let (x, y) = centre(&d, Part::Item(0));
        d.press(x, y);
        let (x, y) = centre(&d, Part::Range(Range::Current));
        d.press(x, y);
        assert_eq!(d.pages(), vec![4]);
        let (x, y) = centre(&d, Part::PagesText);
        d.press(x, y);
        for _ in 0..4 {
            d.key(Key::Backspace, Mods::default());
        }
        d.text("-3,9");
        assert_eq!(d.pages(), vec![0, 1, 2, 8]);
        // Odd pages only, reversed.
        let (x, y) = centre(&d, Part::Subset);
        d.press(x, y);
        let (x, y) = centre(&d, Part::Item(1));
        d.press(x, y);
        let (x, y) = centre(&d, Part::Reverse);
        d.press(x, y);
        assert_eq!(d.pages(), vec![8, 2, 0]);
        let (x, y) = centre(&d, Part::CopiesUp);
        d.press(x, y);
        let (x, y) = centre(&d, Part::Gray);
        d.press(x, y);
        let (x, y) = centre(&d, Part::Scaling(PrintScaling::Actual));
        d.press(x, y);
        let (x, y) = centre(&d, Part::Ok);
        assert_eq!(
            d.press(x, y),
            Some(PrintAction::Print(PrintSettings {
                printer: "A".into(),
                copies: 2,
                collate: true,
                pages: vec![8, 2, 0],
                scaling: PrintScaling::Actual,
                grayscale: true,
                auto_rotate: true,
                auto_center: true,
                layout: PrintLayout::Size,
                duplex: Duplex::Off,
                view: None,
                orientation: Orientation::Portrait,
                what: PrintWhat::DocumentAndMarkups,
                as_image: false,
                overprint: false,
                paper_by_page: false,
            }))
        );
        assert_eq!(
            d.key(Key::Escape, Mods::default()),
            Some(PrintAction::Cancel)
        );
    }
}
