//! The File screen of BlueSlide in the window (round 10 of the slide series): opening it from
//! the File tab or Ctrl+P, routing input to it, its drop-down menus (printer, slides, layout,
//! sides, collation, colour; Protect / Check for Issues / Manage), the live print preview
//! drawn by the engine into its own image, and the requests only the host can do.
use super::file10::{BsOut, BsPage, Recent, SlideBackstage, SlideFacts, LAYOUTS};
use super::ins::SlideDialog;
use super::{MenuKind, SlideWindow};
use crate::popup::{Choice, Menu, MenuItem};
use crate::slide_view::colormode::{recolor, ColorMode};
use crate::{Input, Key, PageStore, Painter, Request};

/// Image key of the print preview (outside the keys of the view and the other panes).
const PREVIEW_KEY: usize = 13 << 40;

/// Something the File screen needs from the platform host.
#[derive(Debug, Clone, PartialEq)]
pub enum FileRequest {
    /// Open this presentation in a new window.
    OpenPath(String),
    /// New presentation in a new window: blank, or with one of BlueSlide's own themes
    /// (`bo_engine::slide_edit::master::THEMES` id).
    New(Option<String>),
    /// Pick a name and save a copy as this type (`pptx`, `potx`, `ppsx`, `pdf`,
    /// `png`, `jpg`, `svg`, `gif`).
    SaveAsType(String),
    /// Pick a name and export: an animated GIF of the show, a package (zip) of the
    /// presentation with its media and fonts, handouts (PDF, slides per page).
    ExportVideo,
    /// See [FileRequest::ExportVideo].
    ExportPackage,
    /// See [FileRequest::ExportVideo]; slides per page.
    ExportHandouts(u8),
    /// Show this file in the system file manager.
    OpenLocation(String),
    /// Print with the Print page's settings.
    Print(SlidePrintJob),
}

/// A print job of the Print page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlidePrintJob {
    /// Printer name.
    pub printer: String,
    /// Copies of each page.
    pub copies: u32,
    /// Whole sets one after the other.
    pub collated: bool,
    /// 0 one sided, 1 both sides on the long edge, 2 on the short edge.
    pub sides: u8,
    /// 0 colour, 1 grayscale, 2 pure black and white.
    pub color: u8,
    /// 0-based slides to print.
    pub slides: Vec<usize>,
    /// Handout slides per page (0: full page slides or notes pages).
    pub per_page: u8,
    /// Notes pages.
    pub notes: bool,
    /// Frame slides.
    pub frame: bool,
}

/// State of the File screen kept in the window.
#[derive(Default)]
pub(super) struct F10 {
    /// The open File screen.
    pub(super) bs: Option<SlideBackstage>,
    /// What the host told: the file's path and size, the app's recent list, printers.
    path: Option<String>,
    bytes: Option<u64>,
    recent: Vec<Recent>,
    printers: Vec<String>,
    printer: usize,
    /// Print settings kept between openings.
    print: Option<super::file10::PrintSettings>,
    out: Vec<FileRequest>,
    /// What the preview image shows now: (layout, color, slides, width in device pixels).
    preview: Option<(usize, u8, Vec<usize>, u32)>,
}

impl SlideWindow {
    /// The host's facts about the file: its path and size on disk (after open and save).
    pub fn set_file_info(&mut self, path: Option<String>, bytes: Option<u64>) {
        self.f10.path = path;
        self.f10.bytes = bytes;
    }

    /// The app's own recent presentations (path, date), most recent first.
    pub fn set_recent(&mut self, list: Vec<(String, String)>) {
        self.f10.recent = list
            .into_iter()
            .map(|(path, when)| Recent { path, when })
            .collect();
        if let Some(b) = self.f10.bs.as_mut() {
            b.recent.clone_from(&self.f10.recent);
        }
    }

    /// Installed printers and the default one.
    pub fn set_printers(&mut self, names: Vec<String>, default: usize) {
        self.f10.printers = names;
        self.f10.printer = default;
    }

    /// Requests of the File screen made since the last call.
    pub fn take_file_requests(&mut self) -> Vec<FileRequest> {
        std::mem::take(&mut self.f10.out)
    }

    /// The File screen is open.
    pub fn file_screen_open(&self) -> bool {
        self.f10.bs.is_some()
    }

    fn file_facts(&self) -> SlideFacts {
        let engine = self.view.engine();
        let props = engine.doc_properties();
        let pres = engine.presentation();
        let slides = pres.map_or(0, |p| p.slides.len());
        let hidden = pres.map_or(0, |p| {
            p.slides.iter().filter(|s| s.show == Some(false)).count()
        });
        let has_notes = pres.is_some_and(|p| {
            p.slides.iter().any(|s| {
                s.notes.as_ref().is_some_and(|n| {
                    n.data.shapes.iter().any(|sh| match sh {
                        bo_model::slide::ShapeNode::Shape(s) => s.text.as_ref().is_some_and(|t| {
                            t.paragraphs.iter().any(|p| !p.text().trim().is_empty())
                        }),
                        _ => false,
                    })
                })
            })
        });
        let size = pres.map_or((960.0, 540.0), |p| {
            (
                p.slide_size.cx as f32 / 12_700.0,
                p.slide_size.cy as f32 / 12_700.0,
            )
        });
        let date = |s: &str| s.replace('T', " ").trim_end_matches('Z').to_owned();
        SlideFacts {
            name: self.name.clone(),
            path: self.f10.path.clone(),
            bytes: self.f10.bytes,
            slides,
            hidden,
            current: self.view.current(),
            modified: self.view.modified(),
            has_notes,
            title: props.title.clone(),
            tags: props.keywords.clone(),
            categories: props.category.clone(),
            author: props.author.clone(),
            last_modified_by: props.last_modified_by.clone(),
            created: date(&props.created),
            last_modified: date(&props.modified),
            slide_size: size,
        }
    }

    /// Open the File screen on `page`.
    pub(super) fn open_file_screen(&mut self, page: BsPage) -> bool {
        let mut b = SlideBackstage::new(page, self.file_facts());
        b.recent.clone_from(&self.f10.recent);
        let mut print = self.f10.print.clone().unwrap_or_default();
        print.printers.clone_from(&self.f10.printers);
        if self.f10.print.is_none() {
            print.printer = self.f10.printer;
        }
        print.printer = print.printer.min(print.printers.len().saturating_sub(1));
        b.print = print;
        b.layout(self.title.height(), self.size, self.scale);
        self.menu = None;
        self.tooltip = None;
        self.backstage = None;
        self.f10.bs = Some(b);
        self.f10.preview = None;
        true
    }

    fn close_file_screen(&mut self) -> bool {
        if let Some(b) = self.f10.bs.take() {
            self.f10.print = Some(b.print);
        }
        self.menu = None;
        true
    }

    /// Lay the File screen out again (window resized, text measured).
    pub(super) fn file10_relayout(&mut self, text: &dyn crate::TextMeasure) {
        let (top, size, scale) = (self.title.height(), self.size, self.scale);
        if let Some(b) = self.f10.bs.as_mut() {
            b.layout(top, size, scale);
            b.measure(text, &self.strings);
        }
    }

    /// Input while the File screen is open (`None`: not handled here).
    pub(super) fn file10_input(&mut self, input: &Input) -> Option<bool> {
        self.f10.bs.as_ref()?;
        if self.dialog.is_some() || self.form8.is_some() {
            return None;
        }
        if self.menu.is_some() {
            if let Some(r) = self.menu_input(input) {
                return Some(r);
            }
        }
        let top = self.title.height();
        let b = self.f10.bs.as_mut()?;
        match *input {
            Input::Move { x, y } if y >= top => Some(b.hover(x, y)),
            Input::Press { x, y, .. } if y >= top => {
                let out = b.press(x, y);
                Some(self.file10_out(out))
            }
            Input::Key(Key::Escape, _) => Some(self.close_file_screen()),
            Input::Key(k, _) => Some(b.key(k)),
            Input::Text(ref t) => Some(b.type_text(t)),
            Input::Wheel(dy) => Some(b.wheel(dy)),
            Input::Context { .. } | Input::Preedit(_) => Some(false),
            _ => None,
        }
    }

    pub(super) fn file10_out(&mut self, out: BsOut) -> bool {
        match out {
            BsOut::Redraw => true,
            BsOut::Back => self.close_file_screen(),
            BsOut::Save => {
                self.requests.push(Request::Save);
                self.close_file_screen()
            }
            BsOut::Close => {
                self.requests.push(Request::Close);
                true
            }
            // BlueSlide has no options of its own yet: the language and proofing choices live
            // in the Review tab; the File screen stays.
            BsOut::None | BsOut::Options => false,
            BsOut::BrowseOpen => {
                self.requests.push(Request::Open);
                true
            }
            BsOut::OpenPath(p) => {
                self.f10.out.push(FileRequest::OpenPath(p));
                true
            }
            BsOut::New(t) => {
                self.f10.out.push(FileRequest::New(t.map(str::to_owned)));
                true
            }
            BsOut::SaveAsType(ext) => {
                if ext == "pptx" {
                    self.requests.push(Request::SaveAs);
                } else {
                    self.f10.out.push(FileRequest::SaveAsType(ext.to_owned()));
                }
                true
            }
            BsOut::ExportPdf => {
                self.requests.push(Request::ExportPdf);
                true
            }
            BsOut::ExportVideo => {
                self.f10.out.push(FileRequest::ExportVideo);
                true
            }
            BsOut::ExportPackage => {
                self.f10.out.push(FileRequest::ExportPackage);
                true
            }
            BsOut::ExportHandouts => {
                // The handout layout chosen on the Print page, else six per page.
                let per = self
                    .f10
                    .bs
                    .as_ref()
                    .map(|b| LAYOUTS[b.print.layout.min(LAYOUTS.len() - 1)].0)
                    .filter(|p| *p > 0)
                    .unwrap_or(6);
                self.f10.out.push(FileRequest::ExportHandouts(per));
                true
            }
            BsOut::CopyPath => {
                if let Some(path) = self.f10.path.clone() {
                    self.requests.push(Request::SetClipboard {
                        text: path.clone(),
                        html: String::new(),
                        internal: String::new(),
                    });
                }
                false
            }
            BsOut::OpenLocation => {
                if let Some(path) = self.f10.path.clone() {
                    self.f10.out.push(FileRequest::OpenLocation(path));
                }
                false
            }
            BsOut::Print => {
                let Some(b) = self.f10.bs.as_ref() else {
                    return false;
                };
                let (per, _, _) = LAYOUTS[b.print.layout.min(LAYOUTS.len() - 1)];
                let job = SlidePrintJob {
                    printer: b
                        .print
                        .printers
                        .get(b.print.printer)
                        .cloned()
                        .unwrap_or_default(),
                    copies: b.print.copies,
                    collated: b.print.collated,
                    sides: b.print.sides,
                    color: b.print.color,
                    slides: b.slides_to_print(),
                    per_page: per,
                    notes: b.print.layout == 1,
                    frame: b.print.frame,
                };
                if job.printer.is_empty() || job.slides.is_empty() {
                    return false;
                }
                self.f10.out.push(FileRequest::Print(job));
                true
            }
            BsOut::HeaderFooter => {
                self.open_dialog(SlideDialog::HeaderFooter);
                true
            }
            BsOut::Menu(id, anchor) => self.file10_menu(id, anchor),
        }
    }

    /// The drop-down of a Print setting or an Info tile.
    fn file10_menu(&mut self, id: &str, anchor: [f32; 4]) -> bool {
        let Some(b) = self.f10.bs.as_ref() else {
            return false;
        };
        let t = |w: &Self, k: &str, f: &str| {
            let v = w.strings.get(k);
            if v == k {
                f.to_owned()
            } else {
                v.to_owned()
            }
        };
        let pr = &b.print;
        let items: Vec<(String, String, bool, bool)> = match id {
            "sbs10.printer" => pr
                .printers
                .iter()
                .enumerate()
                .map(|(i, n)| {
                    (
                        n.clone(),
                        format!("fbs:printer:{i}"),
                        i == pr.printer,
                        false,
                    )
                })
                .collect(),
            "sbs10.range" => vec![
                (
                    t(self, "slideR10.bs.allSlides", "Print All Slides"),
                    "fbs:range:0".into(),
                    pr.range == 0,
                    false,
                ),
                (
                    t(self, "slideR10.bs.currentSlide", "Print Current Slide"),
                    "fbs:range:1".into(),
                    pr.range == 1,
                    false,
                ),
                (
                    t(self, "slideR10.bs.customRange", "Custom Range"),
                    "fbs:range:2".into(),
                    pr.range == 2,
                    false,
                ),
            ],
            "sbs10.layout" => {
                let mut v: Vec<(String, String, bool, bool)> = LAYOUTS
                    .iter()
                    .enumerate()
                    .map(|(i, (_, k, f))| {
                        (
                            t(self, k, f),
                            format!("fbs:layout:{i}"),
                            i == pr.layout,
                            false,
                        )
                    })
                    .collect();
                v.push((
                    t(self, "slideR10.bs.frame", "Frame Slides"),
                    "fbs:frame:".into(),
                    pr.frame,
                    false,
                ));
                v
            }
            "sbs10.sides" => vec![
                (
                    t(self, "slideR10.bs.oneSided", "Print One Sided"),
                    "fbs:sides:0".into(),
                    pr.sides == 0,
                    false,
                ),
                (
                    t(
                        self,
                        "slideR10.bs.bothLongMenu",
                        "Print on Both Sides (long edge)",
                    ),
                    "fbs:sides:1".into(),
                    pr.sides == 1,
                    false,
                ),
                (
                    t(
                        self,
                        "slideR10.bs.bothShortMenu",
                        "Print on Both Sides (short edge)",
                    ),
                    "fbs:sides:2".into(),
                    pr.sides == 2,
                    false,
                ),
            ],
            "sbs10.collate" => vec![
                (
                    t(self, "slideR10.bs.collated", "Collated"),
                    "fbs:collate:1".into(),
                    pr.collated,
                    false,
                ),
                (
                    t(self, "slideR10.bs.uncollated", "Uncollated"),
                    "fbs:collate:0".into(),
                    !pr.collated,
                    false,
                ),
            ],
            "sbs10.color" => vec![
                (
                    t(self, "slideR10.bs.color", "Color"),
                    "fbs:color:0".into(),
                    pr.color == 0,
                    false,
                ),
                (
                    t(self, "slideR10.bs.grayscale", "Grayscale"),
                    "fbs:color:1".into(),
                    pr.color == 1,
                    false,
                ),
                (
                    t(self, "slideR10.bs.pureBw", "Pure Black and White"),
                    "fbs:color:2".into(),
                    pr.color == 2,
                    false,
                ),
            ],
            "sbs10.protect" => vec![
                (
                    t(self, "slideR10.bs.markFinal", "Always Open Read-Only"),
                    "fbs:none:".into(),
                    false,
                    true,
                ),
                (
                    t(self, "slideR10.bs.encrypt", "Encrypt with Password"),
                    "fbs:none:".into(),
                    false,
                    true,
                ),
            ],
            "sbs10.inspect" => vec![
                (
                    t(self, "slideR10.bs.checkA11y", "Check Accessibility"),
                    "fbs:a11y:".into(),
                    false,
                    false,
                ),
                (
                    t(self, "slideR10.bs.inspectDoc", "Inspect Document"),
                    "fbs:none:".into(),
                    false,
                    true,
                ),
            ],
            "sbs10.manage" => vec![(
                t(self, "slideR10.bs.recover", "Recover Unsaved Presentations"),
                "fbs:none:".into(),
                false,
                true,
            )],
            _ => return false,
        };
        if items.is_empty() {
            return false;
        }
        let items = items
            .into_iter()
            .map(|(label, c, checked, dim)| MenuItem {
                label,
                choice: Choice::Command(c),
                checked,
                dim,
            })
            .collect();
        self.menu_kind = MenuKind::Duration;
        self.menu = Some(Menu::open(items, anchor, self.size, self.scale));
        true
    }

    /// A choice of a File screen menu (`fbs:setting:value`).
    pub(super) fn choose_file10(&mut self, v: &str) -> bool {
        let (what, val) = v.split_once(':').unwrap_or((v, ""));
        if what == "a11y" {
            self.close_file_screen();
            return self.open_a11y();
        }
        let Some(b) = self.f10.bs.as_mut() else {
            return false;
        };
        let n: usize = val.parse().unwrap_or(0);
        let small = u8::try_from(n.min(255)).unwrap_or(0);
        match what {
            "printer" => b.print.printer = n,
            "range" => b.print.range = small,
            "layout" => b.print.layout = n.min(LAYOUTS.len() - 1),
            "frame" => b.print.frame = !b.print.frame,
            "sides" => b.print.sides = small,
            "collate" => b.print.collated = n == 1,
            "color" => b.print.color = small,
            _ => return false,
        }
        b.preview = 0;
        b.layout(self.title.height(), self.size, self.scale);
        true
    }

    /// Bring the print preview image up to date.
    pub(super) fn prepare_file10(&mut self, store: &mut dyn PageStore) -> Result<(), String> {
        let Some(b) = self.f10.bs.as_ref() else {
            return Ok(());
        };
        let Some((_, r)) = b.preview_rect() else {
            return Ok(());
        };
        let (per, slides) = b.preview_target();
        let layout = b.print.layout;
        let color = b.print.color;
        let frame = b.print.frame;
        let want = (layout, color, slides.clone(), r[2] as u32);
        if self.f10.preview.as_ref() == Some(&want) || slides.is_empty() {
            return Ok(());
        }
        let engine = self.view.engine_mut();
        let list = if layout == 0 {
            engine.slide(slides[0]).map_err(|e| e.to_string())?.clone()
        } else if layout == 1 {
            use bo_engine::slide_export::{NotesOptions, Paper};
            let pages = engine
                .notes_pages(&NotesOptions {
                    paper: Paper::Letter,
                    slides: Some(slides.clone()),
                    ..NotesOptions::default()
                })
                .map_err(|e| e.to_string())?;
            pages.pages.into_iter().next().ok_or("no notes page")?
        } else {
            use bo_engine::slide_export::{HandoutOptions, Paper};
            let pages = engine
                .handout_pages(&HandoutOptions {
                    per_page: u8::try_from(per).unwrap_or(6),
                    frame,
                    paper: Paper::Letter,
                    slides: Some(slides.clone()),
                    ..HandoutOptions::default()
                })
                .map_err(|e| e.to_string())?;
            pages.pages.into_iter().next().ok_or("no handout page")?
        };
        let list = match color {
            1 => recolor(&list, ColorMode::Grayscale),
            2 => recolor(&list, ColorMode::BlackWhite),
            _ => list,
        };
        let scale = r[2] / list.width.max(1.0);
        store.update(PREVIEW_KEY, &list, self.view.engine(), scale, None)?;
        self.f10.preview = Some(want);
        Ok(())
    }

    /// Paint the File screen and its preview image.
    pub(super) fn paint_file10(&self, p: &mut dyn Painter) {
        let Some(b) = self.f10.bs.as_ref() else {
            return;
        };
        b.paint(p, &self.theme, &self.strings);
        if let (Some((_, r)), Some(_)) = (b.preview_rect(), self.f10.preview.as_ref()) {
            p.raw_colours(true);
            p.page(PREVIEW_KEY, r[0] as i32, r[1] as i32);
            p.raw_colours(false);
        }
        if let Some(menu) = &self.menu {
            menu.paint(p, &self.theme, &self.icons);
        }
    }
}
