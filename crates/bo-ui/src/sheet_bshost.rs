//! The File screen in the BlueSheet window (parity round 13, `docs/ui/REPORT-sheet.md` round
//! 65): opening it from the File tab, routing input, the settings drop-downs of the Print
//! page (run through the Page Layout commands, so the sheet's page setup changes as on the
//! reference), the live print preview drawn into the grid image, and the jobs only the host
//! can do (printing, opening files, new windows, Save As by type).
use super::backstage_impl::{
    range_labels, scaling_labels, BookFacts, BsOut, BsPage, PrintSettings, Recent, SheetBackstage,
    FILE_TYPES, TEMPLATES,
};
use super::SheetWindow;
use crate::popup::{Choice, Menu, MenuItem};
use crate::{GridStore, Input, Key, Painter, TextMeasure};
use bo_model::sheet::{CellRange, CellRef};
use bo_render::{Command, PageList, Rgba};
use serde_json::{json, Value};

/// Work the File screen hands to the host.
#[derive(Debug, Clone, PartialEq)]
pub enum SheetJob {
    /// Print `pages` (0-based) of the job's pages ([`SheetWindow::job_pages`]).
    Print {
        /// Printer name.
        printer: String,
        /// Copies.
        copies: u32,
        /// Collated copies.
        collated: bool,
        /// 0 one side, 1 both sides flipped on the long edge, 2 on the short edge.
        duplex: u8,
        /// Paper turned landscape.
        landscape: bool,
        /// Pages to print (0-based).
        pages: Vec<usize>,
    },
    /// Open this workbook (a new window).
    OpenPath(String),
    /// Pick a workbook to open.
    BrowseOpen,
    /// New window: a blank workbook, or BlueOffice's template with this id.
    New(Option<String>),
    /// Pick a name and save as this type (`xlsx`, `csv`, `ods`, `pdf`, `html`, `mht`, `prn`).
    SaveAsType(String),
    /// Pick a name and export a PDF.
    ExportPdf,
    /// Put the workbook's path on the clipboard.
    CopyPath,
    /// Show the workbook's folder.
    OpenLocation,
    /// The printer's own settings dialog.
    PrinterProperties(String),
}

/// File screen state kept by the window.
#[derive(Default)]
pub(super) struct BsState {
    /// The open File screen.
    pub(super) screen: Option<SheetBackstage>,
    /// The app's own recent workbooks (path, when), given by the host.
    recent: Vec<(String, String)>,
    /// Installed printers, the default one and their spooler states, given by the host.
    printers: Vec<String>,
    default_printer: usize,
    states: Vec<(bool, bool, u32)>,
    /// The open workbook's path and size, given by the host.
    path: Option<String>,
    bytes: Option<u64>,
    /// Print settings kept between openings of the File screen (this session).
    print: Option<PrintSettings>,
    /// Key of the preview drawn into the grid image (`None`: the grid shows the sheet).
    drawn: Option<u64>,
    /// Jobs for the host.
    jobs: Vec<SheetJob>,
}

/// Paper sizes of the Print page (§18.18.? ST_PaperSize codes of `pageSetup/@paperSize`,
/// ECMA-376 Part 1, §18.3.1.63): code, name, width × height in inches.
const PAPERS: [(u32, &str, f32, f32); 5] = [
    (1, "Letter", 8.5, 11.0),
    (5, "Legal", 8.5, 14.0),
    (8, "A3", 11.69, 16.54),
    (9, "A4", 8.27, 11.69),
    (11, "A5", 5.83, 8.27),
];

/// Margin presets (left, right, top, bottom in inches) as the Page Layout tab's Margins.
const MARGINS: [(&str, &str, &str, [f64; 4]); 3] = [
    (
        "normal",
        "sheetBs.marginsNormal",
        "Normal Margins",
        [0.7, 0.7, 0.75, 0.75],
    ),
    (
        "wide",
        "sheetBs.marginsWide",
        "Wide Margins",
        [1.0, 1.0, 1.0, 1.0],
    ),
    (
        "narrow",
        "sheetBs.marginsNarrow",
        "Narrow Margins",
        [0.25, 0.25, 0.75, 0.75],
    ),
];

fn inches(v: f64) -> String {
    let s = format!("{v:.2}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    format!("{s}\"")
}

/// Text of element `local` (any prefix) in a small XML part.
fn element_text(xml: &str, local: &str) -> String {
    let mut from = 0;
    while let Some(i) = xml[from..].find('<') {
        let start = from + i + 1;
        let Some(end) = xml[start..].find('>') else {
            break;
        };
        let tag = &xml[start..start + end];
        let name = tag.split([' ', '/']).next().unwrap_or("");
        let bare = name.rsplit(':').next().unwrap_or(name);
        if bare == local && !tag.starts_with('/') && !tag.ends_with('/') {
            let body = start + end + 1;
            if let Some(close) = xml[body..].find("</") {
                return xml[body..body + close].trim().to_owned();
            }
        }
        from = start + end;
    }
    String::new()
}

impl SheetWindow {
    /// Host: the app's own recent workbooks (path, when opened), newest first.
    pub fn set_recent(&mut self, list: Vec<(String, String)>) {
        self.bs.recent = list;
    }

    /// Host: installed printers, the default one, and each printer's spooler state (error,
    /// offline, jobs waiting).
    pub fn set_printers(
        &mut self,
        names: Vec<String>,
        default: usize,
        states: Vec<(bool, bool, u32)>,
    ) {
        self.bs.printers = names;
        self.bs.default_printer = default;
        self.bs.states = states;
    }

    /// Host: the open workbook's file (path and size in bytes).
    pub fn set_file_info(&mut self, path: Option<String>, bytes: Option<u64>) {
        self.bs.path = path;
        self.bs.bytes = bytes;
    }

    /// Jobs the File screen asked of the host since the last call.
    pub fn take_jobs(&mut self) -> Vec<SheetJob> {
        std::mem::take(&mut self.bs.jobs)
    }

    /// Whether the File screen is open.
    pub fn backstage_open(&self) -> bool {
        self.bs.screen.is_some()
    }

    /// Sheets and selection the Print page's "what prints" choice covers.
    fn job_scope(&self, range: u8) -> (Vec<usize>, Option<CellRange>) {
        let n = self.view.sheet_names().len();
        match range {
            1 => ((0..n).collect(), None),
            2 => {
                let v = self.view.rng();
                let at = |k: &str| v[k].as_u64().unwrap_or(0) as u32;
                (
                    vec![self.view.sheet()],
                    Some(CellRange::new(
                        CellRef::new(at("r0"), at("c0")),
                        CellRef::new(at("r1"), at("c1")),
                    )),
                )
            }
            _ => (vec![self.view.sheet()], None),
        }
    }

    /// Display lists of the pages a print job prints (all pages of the Print page's scope).
    pub fn job_pages(&mut self, range: u8) -> Vec<PageList> {
        let (sheets, sel) = self.job_scope(range);
        self.view
            .engine_mut()
            .print_job_pages(&sheets, sel)
            .unwrap_or_default()
    }

    /// The Print page's "what prints" choice of the open File screen (0 active sheets).
    pub fn job_range(&self) -> u8 {
        self.bs.screen.as_ref().map_or(0, |b| b.print.range)
    }

    /// File › (page): open the File screen.
    pub(super) fn open_backstage(&mut self, page: BsPage, text: &dyn TextMeasure) -> bool {
        self.menu = None;
        let mut b = SheetBackstage::new(page, BookFacts::default());
        b.print = self.bs.print.clone().unwrap_or_default();
        if b.print.printers.is_empty() {
            b.print.printer = self.bs.default_printer;
        }
        b.recent = self
            .bs
            .recent
            .iter()
            .map(|(path, when)| Recent {
                path: path.clone(),
                when: when.clone(),
            })
            .collect();
        self.bs.screen = Some(b);
        self.bs_refresh(text);
        true
    }

    /// Close the File screen (the grid image shows the sheet again).
    pub(super) fn close_backstage(&mut self) {
        if let Some(b) = self.bs.screen.take() {
            self.bs.print = Some(b.print);
        }
        self.menu = None;
        self.view.invalidate();
    }

    /// Facts and settings of the File screen from the workbook and its page setup.
    fn bs_refresh(&mut self, text: &dyn TextMeasure) {
        if self.bs.screen.is_none() {
            return;
        }
        let setup = self
            .view
            .query(json!({ "cmd": "pageSetup", "sheet": self.view.sheet() }))
            .unwrap_or(Value::Null);
        let protected = self
            .view
            .query(json!({ "cmd": "protection", "sheet": self.view.sheet() }))
            .and_then(|v| v["workbook"]["structure"].as_bool())
            .unwrap_or(false);
        let core = self
            .view
            .engine()
            .workbook()
            .and_then(|wb| {
                wb.opaque
                    .parts
                    .iter()
                    .find(|p| p.name.eq_ignore_ascii_case("/docProps/core.xml"))
                    .map(|p| String::from_utf8_lossy(&p.data).into_owned())
            })
            .unwrap_or_default();
        let range = self.bs.screen.as_ref().map_or(0, |b| b.print.range);
        let (sheets, sel) = self.job_scope(range);
        let preview = self.bs.screen.as_ref().map_or(0, |b| b.preview);
        let (count, first) = self
            .view
            .engine_mut()
            .print_job_preview(&sheets, sel, preview)
            .unwrap_or((0, None));
        let page_size = first
            .as_ref()
            .map_or((595.0, 842.0), |p| (p.width, p.height));
        let landscape = setup["orientation"].as_str() == Some("landscape");
        let code = setup["paperSize"].as_u64().unwrap_or(9) as u32;
        let (pname, pw, ph) = PAPERS
            .iter()
            .find(|p| p.0 == code)
            .map_or(("A4".to_owned(), 8.27, 11.69), |p| {
                (p.1.to_owned(), p.2, p.3)
            });
        let paper = (
            format!(
                "{pname} ({:.1} x {:.1} in)",
                (pw * 10.0).round() / 10.0,
                (ph * 10.0).round() / 10.0
            )
            .replace(".0 ", " ")
            .replace(".0)", ")"),
            pw,
            ph,
        );
        let m: Vec<f64> = setup["margins"]
            .as_array()
            .map(|a| a.iter().filter_map(Value::as_f64).collect())
            .unwrap_or_default();
        let margins = MARGINS
            .iter()
            .find(|(_, _, _, v)| {
                m.len() == 4 && v.iter().zip(&m).all(|(a, b)| (a - b).abs() < 1e-6)
            })
            .map_or_else(
                || self.t("sheetBs.customMargins", "Custom Margins"),
                |(_, k, f, v)| {
                    format!(
                        "{}\n{} {}    {} {}",
                        self.t(k, f),
                        self.t("sheetBs.left", "Left:"),
                        inches(v[0]),
                        self.t("sheetBs.right", "Right:"),
                        inches(v[1])
                    )
                },
            );
        let fit = setup["fit"].as_array().map(|a| {
            (
                a.first().and_then(Value::as_u64).unwrap_or(1),
                a.get(1).and_then(Value::as_u64).unwrap_or(1),
            )
        });
        let scaling = match (fit, setup["scale"].as_u64().unwrap_or(100)) {
            (Some((1, 1)), _) => 1,
            (Some((1, 0)), _) => 2,
            (Some((0, 1)), _) => 3,
            (None, 100) => 0,
            _ => 4,
        };
        let name = self.name.clone();
        let facts = BookFacts {
            name,
            path: self.bs.path.clone(),
            bytes: self.bs.bytes,
            pages: count,
            categories: element_text(&core, "category"),
            modified: self.view.modified(),
            title: element_text(&core, "title"),
            tags: element_text(&core, "keywords"),
            author: element_text(&core, "creator"),
            last_modified_by: element_text(&core, "lastModifiedBy"),
            created: element_text(&core, "created")
                .replace(['T', 'Z'], " ")
                .trim()
                .to_owned(),
            last_modified: element_text(&core, "modified")
                .replace(['T', 'Z'], " ")
                .trim()
                .to_owned(),
            protected,
            landscape,
            paper,
            margins,
            page_sizes: vec![page_size; count],
            ..BookFacts::default()
        };
        let (printers, states, top, size, scale) = (
            self.bs.printers.clone(),
            self.bs.states.clone(),
            self.title.height(),
            self.size,
            self.scale,
        );
        if let Some(b) = self.bs.screen.as_mut() {
            b.facts = facts;
            b.print.printers = printers;
            b.print.states = states;
            b.print.scaling = scaling;
            if b.preview >= count.max(1) {
                b.preview = count.saturating_sub(1);
            }
            b.layout(top, size, scale);
            b.measure(text, &self.strings);
        }
        self.bs.drawn = None;
    }

    /// Input while the File screen is open: `Some(redraw)` when it took it.
    pub(super) fn bs_input(&mut self, input: &Input, text: &dyn TextMeasure) -> Option<bool> {
        self.bs.screen.as_ref()?;
        if self.menu.is_some() {
            // The settings drop-downs are ordinary menus.
            return None;
        }
        let b = self.bs.screen.as_mut()?;
        Some(match input {
            Input::Move { x, y } => b.hover(*x, *y),
            Input::Press { x, y, .. } => {
                // The title bar keeps its buttons.
                if *y < self.title.height() {
                    return None;
                }
                let out = b.press(*x, *y);
                self.bs_out(out, text)
            }
            Input::Text(s) => {
                let typed = b.type_text(s);
                if typed {
                    self.bs_refresh(text);
                }
                typed
            }
            Input::Key(Key::Escape, _) => {
                self.close_backstage();
                true
            }
            Input::Key(k, _) => {
                let done = b.key(*k);
                if done {
                    self.bs_refresh(text);
                }
                done
            }
            Input::Wheel(d) => {
                let turned = b.wheel(-*d);
                if turned {
                    self.bs_refresh(text);
                }
                turned
            }
            Input::Resize { .. } | Input::Scale(_) | Input::Maximized(_) => return None,
            Input::Blink | Input::Tick => false,
            _ => true,
        })
    }

    fn bs_out(&mut self, out: BsOut, text: &dyn TextMeasure) -> bool {
        let job = |w: &mut Self, j: SheetJob| {
            w.bs.jobs.push(j);
            true
        };
        match out {
            BsOut::None | BsOut::Convert | BsOut::Email => false,
            BsOut::Redraw => {
                self.bs_refresh(text);
                true
            }
            BsOut::Back => {
                self.close_backstage();
                true
            }
            BsOut::Save => {
                self.close_backstage();
                self.action("file.save", text)
            }
            BsOut::Close => {
                self.close_backstage();
                self.action("file.close", text)
            }
            BsOut::Options => {
                self.close_backstage();
                self.action("file.options", text)
            }
            BsOut::BrowseOpen => job(self, SheetJob::BrowseOpen),
            BsOut::OpenPath(p) => job(self, SheetJob::OpenPath(p)),
            BsOut::New(t) => {
                self.close_backstage();
                job(self, SheetJob::New(t.map(str::to_owned)))
            }
            BsOut::SaveAsType(t) => job(self, SheetJob::SaveAsType(t.to_owned())),
            BsOut::ExportPdf => job(self, SheetJob::ExportPdf),
            BsOut::CopyPath => job(self, SheetJob::CopyPath),
            BsOut::OpenLocation => job(self, SheetJob::OpenLocation),
            BsOut::Print => {
                let Some(b) = self.bs.screen.as_ref() else {
                    return false;
                };
                let p = &b.print;
                let Some(printer) = p.printers.get(p.printer).cloned() else {
                    return false;
                };
                let j = SheetJob::Print {
                    printer,
                    copies: p.copies,
                    collated: p.collated,
                    duplex: p.sides,
                    landscape: b.facts.landscape,
                    pages: b.pages_to_print(),
                };
                job(self, j)
            }
            BsOut::PrinterProperties => {
                let name = self
                    .bs
                    .screen
                    .as_ref()
                    .and_then(|b| b.print.printers.get(b.print.printer).cloned());
                match name {
                    Some(n) => job(self, SheetJob::PrinterProperties(n)),
                    None => false,
                }
            }
            BsOut::PageSetup => {
                // The reference opens Page Setup; BlueSheet shows the Page Layout tab's
                // commands in the window behind (its Page Setup group).
                self.close_backstage();
                self.action("sheet.viewPageLayout", text);
                true
            }
            BsOut::Menu(id, anchor) => {
                self.bs_menu(id, anchor);
                true
            }
        }
    }

    /// The drop-downs of the File screen (settings of the Print page, Info tiles, the Save As
    /// type).
    fn bs_menu(&mut self, id: &str, anchor: [f32; 4]) {
        let Some(b) = self.bs.screen.as_ref() else {
            return;
        };
        let p = b.print.clone();
        let landscape = b.facts.landscape;
        let item = |label: String, cmd: String, checked: bool| MenuItem {
            label,
            choice: Choice::Command(cmd),
            checked,
            dim: false,
        };
        let s = &self.strings;
        let items: Vec<MenuItem> = match id {
            "bs13.printer" => p
                .printers
                .iter()
                .enumerate()
                .map(|(i, n)| item(n.clone(), format!("bs13.printer.{i}"), i == p.printer))
                .collect(),
            "bs13.range" => (0..3)
                .map(|i| {
                    let (l, _) = range_labels(s, i);
                    item(l, format!("bs13.range.{i}"), i == p.range)
                })
                .collect(),
            "bs13.sides" => [
                ("sheetBs.oneSided", "Print One Sided"),
                ("sheetBs.bothLongMenu", "Print on Both Sides (long edge)"),
                ("sheetBs.bothShortMenu", "Print on Both Sides (short edge)"),
            ]
            .iter()
            .enumerate()
            .map(|(i, (k, f))| item(self.t(k, f), format!("bs13.sides.{i}"), i as u8 == p.sides))
            .collect(),
            "bs13.collate" => vec![
                item(
                    self.t("sheetBs.collated", "Collated"),
                    "bs13.collate.1".into(),
                    p.collated,
                ),
                item(
                    self.t("sheetBs.uncollated", "Uncollated"),
                    "bs13.collate.0".into(),
                    !p.collated,
                ),
            ],
            "bs13.orientation" => vec![
                item(
                    self.t("sheetBs.portrait", "Portrait Orientation"),
                    "orient.portrait".into(),
                    !landscape,
                ),
                item(
                    self.t("sheetBs.landscape", "Landscape Orientation"),
                    "orient.landscape".into(),
                    landscape,
                ),
            ],
            "bs13.paper" => PAPERS
                .iter()
                .map(|(code, name, w, h)| {
                    item(
                        format!("{name}    {w}\" x {h}\""),
                        format!("paper.{code}"),
                        b.facts.paper.0.starts_with(name),
                    )
                })
                .collect(),
            "bs13.margins" => MARGINS
                .iter()
                .map(|(id, k, f, _)| {
                    item(
                        self.t(k, f),
                        format!("margins.{id}"),
                        b.facts.margins.starts_with(&self.t(k, f)),
                    )
                })
                .collect(),
            "bs13.scaling" => (0..4)
                .map(|i| {
                    let (l, _) = scaling_labels(s, i);
                    item(l, format!("bs13.scaling.{i}"), i == p.scaling)
                })
                .collect(),
            "bs13.saveType" => FILE_TYPES
                .iter()
                .enumerate()
                .map(|(i, (_, k, f))| {
                    item(self.t(k, f), format!("bs13.type.{i}"), i == b.file_type)
                })
                .collect(),
            "bs13.protect" => vec![
                item(
                    self.t("sheetBs.protectSheet", "Protect Current Sheet"),
                    "sheet.protectSheet".into(),
                    false,
                ),
                item(
                    self.t("sheetBs.protectStructure", "Protect Workbook Structure"),
                    "sheet.protectWorkbook".into(),
                    b.facts.protected,
                ),
            ],
            "bs13.inspect" => vec![item(
                self.t("sheetBs.checkAccessibility", "Check Accessibility"),
                "sheet.accessCheck".into(),
                false,
            )],
            _ => vec![MenuItem {
                label: self.t("sheetBs.noUnsaved", "There are no unsaved changes."),
                choice: Choice::Command(String::new()),
                checked: false,
                dim: true,
            }],
        };
        self.menu = Some(Menu::open(items, anchor, self.size, self.scale));
    }

    /// A choice from a File screen drop-down (`bs13.*`); page setup choices run as the Page
    /// Layout commands and refresh the screen afterwards (see [`Self::bs_after_choice`]).
    pub(super) fn bs_choose(&mut self, id: &str, text: &dyn TextMeasure) -> bool {
        let Some((what, n)) = id
            .strip_prefix("bs13.")
            .and_then(|r| r.rsplit_once('.'))
            .and_then(|(w, n)| n.parse::<usize>().ok().map(|n| (w.to_owned(), n)))
        else {
            return false;
        };
        let done = if what == "scaling" {
            {
                let sheet = self.view.sheet();
                let v = match n {
                    1 => json!({ "fit": [1, 1] }),
                    2 => json!({ "fit": [1, 0] }),
                    3 => json!({ "fit": [0, 1] }),
                    _ => json!({ "fit": null, "scale": 100 }),
                };
                let mut cmd = json!({ "cmd": "setPageSetup", "sheet": sheet });
                if let (Some(o), Some(extra)) = (cmd.as_object_mut(), v.as_object()) {
                    o.extend(extra.clone());
                }
                let ok = self.view.query(cmd).is_some();
                if ok {
                    self.view.changed();
                }
                ok
            }
        } else {
            {
                let Some(b) = self.bs.screen.as_mut() else {
                    return false;
                };
                match what.as_str() {
                    "printer" => b.print.printer = n,
                    "range" => {
                        b.print.range = n.min(2) as u8;
                        b.preview = 0;
                    }
                    "sides" => b.print.sides = n.min(2) as u8,
                    "collate" => b.print.collated = n == 1,
                    "type" => b.file_type = n.min(FILE_TYPES.len() - 1),
                    _ => return false,
                }
                true
            }
        };
        self.bs_refresh(text);
        done
    }

    /// After a menu choice while the File screen is open: its facts follow the page setup.
    pub(super) fn bs_after_choice(&mut self, text: &dyn TextMeasure) {
        if self.bs.screen.is_some() {
            self.bs_refresh(text);
        }
    }

    /// Lay the File screen out again (window resized).
    pub(super) fn bs_layout(&mut self, text: &dyn TextMeasure) {
        if self.bs.screen.is_some() {
            let (top, size, scale) = (self.title.height(), self.size, self.scale);
            if let Some(b) = self.bs.screen.as_mut() {
                b.layout(top, size, scale);
                b.measure(text, &self.strings);
            }
            self.bs.drawn = None;
        }
    }

    /// The grid image while the File screen is open: the preview page of the Print page
    /// (drawn once per change), nothing for the other pages. `None`: draw the grid.
    pub(super) fn bs_prepare(&mut self, store: &mut dyn GridStore) -> Option<Result<(), String>> {
        let b = self.bs.screen.as_ref()?;
        let Some((i, r)) = b.preview_rect() else {
            return Some(Ok(()));
        };
        let margins = b.show_margins;
        let range = b.print.range;
        let key = {
            use std::hash::{Hash, Hasher};
            let mut h = std::collections::hash_map::DefaultHasher::new();
            (i, range, margins, self.view.sheet()).hash(&mut h);
            for v in r {
                v.to_bits().hash(&mut h);
            }
            b.facts.pages.hash(&mut h);
            b.facts.paper.0.hash(&mut h);
            b.facts.margins.hash(&mut h);
            b.facts.landscape.hash(&mut h);
            b.print.scaling.hash(&mut h);
            h.finish()
        };
        if self.bs.drawn == Some(key) {
            return Some(Ok(()));
        }
        let (sheets, sel) = self.job_scope(range);
        let setup = self
            .view
            .query(json!({ "cmd": "pageSetup", "sheet": self.view.sheet() }))
            .unwrap_or(Value::Null);
        let page = self
            .view
            .engine_mut()
            .print_job_preview(&sheets, sel, i)
            .ok()
            .and_then(|(_, p)| p);
        let k = self.scale.max(0.01);
        let (w, h) = ((r[2] / k).max(1.0), (r[3] / k).max(1.0));
        let mut list = PageList {
            width: w,
            height: h,
            commands: vec![Command::FillRect {
                x: 0.0,
                y: 0.0,
                w,
                h,
                color: Rgba(255, 255, 255, 255),
            }],
        };
        if let Some(pg) = page {
            let s = w / pg.width.max(1.0);
            list.commands.push(Command::PushTransform {
                m: [s, 0.0, 0.0, s, 0.0, 0.0],
            });
            list.commands.extend(pg.commands);
            if margins {
                // Show Margins: the page's margins as thin lines across the page.
                let m: Vec<f32> = setup["margins"]
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .filter_map(Value::as_f64)
                            .map(|v| v as f32 * 72.0)
                            .collect()
                    })
                    .unwrap_or_default();
                if let [l, rr, t, bb] = m[..] {
                    let (pw, ph) = (pg.width, pg.height);
                    let c = Rgba(64, 64, 64, 255);
                    let lw = 1.0 / (s * k);
                    for (x1, y1, x2, y2) in [
                        (l, 0.0, l, ph),
                        (pw - rr, 0.0, pw - rr, ph),
                        (0.0, t, pw, t),
                        (0.0, ph - bb, pw, ph - bb),
                    ] {
                        list.commands.push(Command::Line {
                            x1,
                            y1,
                            x2,
                            y2,
                            width: lw,
                            color: c,
                            dash: None,
                        });
                    }
                }
            }
            list.commands.push(Command::PopTransform);
        }
        let r = store.update(&list, self.view.engine(), k);
        self.bs.drawn = Some(key);
        Some(r)
    }

    /// Fill the (new, empty) workbook with BlueOffice's template `id` (File › New): its cells,
    /// a bold header row filled with the accent's light tint, wider first columns. The
    /// contents are BlueOffice's own; `false` for an unknown id.
    pub fn load_template(&mut self, id: &str) -> bool {
        let rows: &[&[&str]] = match id {
            "budget" => &[
                &["Monthly budget"],
                &[],
                &["Income", "Planned", "Actual", "Difference"],
                &["Salary", "15000000", "15000000", "=C4-B4"],
                &["Other income", "2000000", "1500000", "=C5-B5"],
                &["Total income", "=SUM(B4:B5)", "=SUM(C4:C5)", "=C6-B6"],
                &[],
                &["Expenses", "Planned", "Actual", "Difference"],
                &["Housing", "5000000", "5000000", "=B9-C9"],
                &["Food", "3000000", "3400000", "=B10-C10"],
                &["Transport", "1000000", "800000", "=B11-C11"],
                &["Savings", "4000000", "4000000", "=B12-C12"],
                &["Total expenses", "=SUM(B9:B12)", "=SUM(C9:C12)", "=B13-C13"],
                &[],
                &["Balance", "=B6-B13", "=C6-C13"],
            ],
            "invoice" => &[
                &["INVOICE"],
                &["Invoice no.", "0001"],
                &["Date", "=TODAY()"],
                &["Bill to", ""],
                &[],
                &["Item", "Quantity", "Unit price", "Amount"],
                &["", "", "", "=B7*C7"],
                &["", "", "", "=B8*C8"],
                &["", "", "", "=B9*C9"],
                &["", "", "", "=B10*C10"],
                &[],
                &["", "", "Subtotal", "=SUM(D7:D10)"],
                &["", "", "Tax (10%)", "=D12*10%"],
                &["", "", "Total", "=D12+D13"],
            ],
            "schedule" => &[
                &["Weekly schedule"],
                &[],
                &[
                    "Time",
                    "Monday",
                    "Tuesday",
                    "Wednesday",
                    "Thursday",
                    "Friday",
                    "Saturday",
                    "Sunday",
                ],
                &["7:00"],
                &["8:00"],
                &["9:00"],
                &["10:00"],
                &["11:00"],
                &["12:00"],
                &["13:00"],
                &["14:00"],
                &["15:00"],
                &["16:00"],
                &["17:00"],
            ],
            "todo" => &[
                &["To-do list"],
                &[],
                &["Done", "Task", "Due date", "Priority", "Notes"],
                &["FALSE", "", "", "Normal"],
                &["FALSE", "", "", "Normal"],
                &["FALSE", "", "", "Normal"],
                &["FALSE", "", "", "Normal"],
                &["FALSE", "", "", "Normal"],
                &[],
                &["Open tasks", "=COUNTIF(A4:A8,FALSE)"],
            ],
            "expenses" => &[
                &["Expense report"],
                &["Name", ""],
                &["Period", ""],
                &[],
                &["Date", "Description", "Category", "Amount", "Notes"],
                &["", "", "", ""],
                &["", "", "", ""],
                &["", "", "", ""],
                &["", "", "", ""],
                &["", "", "", ""],
                &["Total", "", "", "=SUM(D6:D10)"],
            ],
            _ => return false,
        };
        let header = match id {
            "budget" => vec![2, 7],
            "invoice" => vec![5],
            "expenses" => vec![4],
            _ => vec![2],
        };
        let cells: Vec<(u32, u32, String)> = rows
            .iter()
            .enumerate()
            .flat_map(|(r, row)| {
                row.iter()
                    .enumerate()
                    .filter(|(_, v)| !v.is_empty())
                    .map(move |(c, v)| (r as u32, c as u32, (*v).to_owned()))
            })
            .collect();
        let sheet = self.view.sheet();
        if self.view.engine_mut().set_inputs(sheet, &cells).is_err() {
            return false;
        }
        let width = rows.iter().map(|r| r.len()).max().unwrap_or(1).max(1) as u32;
        let tint = {
            let a = self.theme.accent_strong;
            let mix = |v: u8| ((u16::from(v) + 3 * 255) / 4) as u8;
            format!("{:02X}{:02X}{:02X}", mix(a.0), mix(a.1), mix(a.2))
        };
        // Title: bold and larger; header rows bold on the tint.
        self.view.select((0, 0), (0, 0), (0, 0));
        self.patch(json!({ "bold": true, "fontSize": 16.0 }));
        for r in header {
            self.view.select((r, 0), (r, width - 1), (r, 0));
            self.patch(json!({ "bold": true, "fillColor": tint }));
        }
        let _ = self.view.run(json!({
            "cmd": "resize", "rows": false, "first": 0, "last": 0, "size": 110.0,
        }));
        let _ = self.view.run(json!({
            "cmd": "resize", "rows": false, "first": 1, "last": width.max(2) - 1, "size": 90.0,
        }));
        self.view.select((0, 0), (0, 0), (0, 0));
        self.view.changed();
        true
    }

    /// Paint the File screen over the window (below the title bar) with its preview page.
    pub(super) fn paint_backstage(&self, p: &mut dyn Painter) {
        let Some(b) = self.bs.screen.as_ref() else {
            return;
        };
        b.paint(p, &self.theme, &self.strings);
        if self.bs.drawn.is_some() {
            if let Some((_, r)) = b.preview_rect() {
                let a = b.preview_area();
                p.push_clip(a[0], a[1], a[2], a[3]);
                p.page(0, r[0].round() as i32, r[1].round() as i32);
                p.pop_clip();
            }
        }
        let _ = TEMPLATES;
    }
}
