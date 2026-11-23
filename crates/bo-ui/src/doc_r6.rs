//! BlueDoc parity round 6 (docs/ui/REPORT.md rounds 88+): the File screen with its pages
//! (`doc_backstage6`), File › Options (`doc_options6`) and the application preferences they
//! keep. A child module of the shell so the window's state is at hand.
use super::{DocWindow, Request};
use crate::doc_backstage6::{BsOut, BsPage, DocBackstage, DocFacts, Recent, FILE_TYPES};
use crate::popup::{Choice, Menu, MenuItem};
use crate::{Input, Key, Painter, TextMeasure};
use bo_engine::edit::Command;
use serde_json::json;

use crate::doc_forms::options6 as options;

#[path = "doc_review6.rs"]
mod review;
pub(crate) use options::DocPrefs;

/// Labels of round 6's ribbon commands.
pub(crate) fn install_strings(strings: &mut crate::strings::Strings) {
    let v = strings.get("docR6.rv.pane").to_owned();
    let label = if v == "docR6.rv.pane" {
        "Reviewing Pane".to_owned()
    } else {
        v
    };
    strings.set("commands.review.reviewingPane", &label);
}

/// Icons of round 6's ribbon commands.
pub(crate) fn install_icons(icons: &mut crate::icons::Icons) {
    // Our own drawing: a page with a list panel down its left side.
    icons.add_solid(
        "review.reviewingPane",
        &[
            ("M3 4h18v16H3z", None),
            ("M3 4h7v16H3z", Some(bo_render::Rgba(0x2b, 0x6c, 0xc4, 255))),
            ("M12.5 8h6M12.5 11h6M12.5 14h4", None),
        ],
    );
}

/// Round-6 state of the window.
#[derive(Default)]
pub(super) struct R6 {
    /// The File screen, when open.
    pub(super) bs: Option<DocBackstage>,
    /// Path and size of the open file (host).
    pub(super) file: Option<(String, Option<u64>)>,
    /// BlueDoc's own recent documents (host).
    pub(super) recent: Vec<Recent>,
    /// Installed printers and the default one (host).
    pub(super) printers: (Vec<String>, usize),
    /// Application preferences and their text for the host to store when changed.
    pub(super) prefs: DocPrefs,
    pub(super) prefs_out: Option<String>,
    /// File › Options, when open.
    pub(super) options: Option<options::OptionsDialog>,
    /// Review › Reviewing Pane, when shown.
    pub(super) review: Option<review::ReviewPane>,
    /// What the host read from the file, and the printers' spooler states.
    pub(super) details: FileDetails,
    pub(super) printer_states: Vec<(bool, bool, u32)>,
}

/// File › Info facts only the host can read from the file.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FileDetails {
    /// Total editing time in minutes (`docProps/app.xml` TotalTime).
    pub edit_minutes: Option<u64>,
    /// Compatibility mode (a `.doc`, or `compatibilityMode` below 15).
    pub compat: bool,
    /// File system created / modified times (local), used when the document has none.
    pub created: String,
    /// See `created`.
    pub modified: String,
}

/// Print job the File screen's Print page asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrintJob {
    /// Printer name.
    pub printer: String,
    /// Copies of each page.
    pub copies: u32,
    /// Pages (0-based) in order.
    pub pages: Vec<usize>,
    /// Collated copies.
    pub collate: bool,
    /// 0 one sided, 1 long edge, 2 short edge.
    pub duplex: u8,
    /// Pages per sheet.
    pub per_sheet: u8,
}

impl DocWindow {
    /// Open the File screen on `page`.
    pub(super) fn r6_open_backstage(&mut self, page: BsPage) -> bool {
        self.menu = None;
        let facts = self.r6_facts();
        let mut b = DocBackstage::new(page, facts);
        b.recent = self
            .r6
            .recent
            .iter()
            .take(self.r6.prefs.recent_count as usize)
            .cloned()
            .collect();
        b.print.printers.clone_from(&self.r6.printers.0);
        b.print.printer = self.r6.printers.1;
        b.print.states.clone_from(&self.r6.printer_states);
        b.preview = self.view.current_page();
        b.layout(self.title.height(), self.size, self.scale);
        self.r6.bs = Some(b);
        true
    }

    /// What the File screen shows about the document.
    fn r6_facts(&mut self) -> DocFacts {
        let props = self.view.doc_properties();
        let sec = self.view.format().section;
        let inch = |tw: i32| tw as f32 / 1440.0;
        let cm = |tw: i32| tw as f32 * 2.54 / 1440.0;
        let (w, h) = (sec.width.min(sec.height), sec.width.max(sec.height));
        let paper = crate::doc_r4_menus::SIZES
            .iter()
            .find(|(_, _, a, b)| (cm(w) - a).abs() < 0.1 && (cm(h) - b).abs() < 0.1)
            .map_or_else(
                || self.t("docR6.bs.customSize", "Custom Size"),
                |(_, n, ..)| (*n).to_owned(),
            );
        let margins = crate::doc_r4_menus::MARGINS
            .iter()
            .find(|(_, _, m)| {
                (cm(sec.margin_top) - m[0]).abs() < 0.03
                    && (cm(sec.margin_left) - m[2]).abs() < 0.03
                    && (cm(sec.margin_right) - m[3]).abs() < 0.03
            })
            .map_or_else(
                || self.t("docR6.bs.customMargins", "Custom Margins"),
                |(_, n, _)| format!("{n} {}", self.t("docR6.bs.margins", "Margins")),
            );
        let date = |s: &str| s.split('T').next().unwrap_or("").to_owned();
        let protected = self
            .view
            .engine()
            .document()
            .is_some_and(|d| d.protection.as_ref().is_some_and(|p| p.enforced));
        let (path, bytes) = self
            .r6
            .file
            .clone()
            .map_or((None, None), |(p, b)| (Some(p), b));
        let mut f = DocFacts {
            name: self.name.clone(),
            path,
            bytes,
            pages: self.view.page_count(),
            words: self.view.word_count(),
            modified: self.view.modified(),
            protected,
            landscape: sec.width > sec.height,
            paper: (paper, inch(sec.width), inch(sec.height)),
            margins,
            page_sizes: self.view.page_sizes_pt(),
            compat: self.r6.details.compat,
            editing_minutes: self.r6.details.edit_minutes,
            ..DocFacts::default()
        };
        if let Some(p) = props {
            f.title = p.title;
            f.tags = p.keywords;
            f.comments = p.comments;
            f.author = p.author;
            f.last_modified_by = p.last_modified_by;
            f.created = date(&p.created);
            f.last_modified = date(&p.modified);
            f.more = vec![
                ("docR6.bs.subject", "Subject", p.subject),
                ("docR6.bs.category", "Categories", p.category),
                ("docR6.bs.status", "Status", p.status),
                ("docR6.bs.company", "Company", p.company),
                ("docR6.bs.manager", "Manager", p.manager),
                ("docR6.bs.revision", "Revision Number", p.revision),
            ];
        }
        // Dates the document does not carry come from the file system, as the reference
        // shows them.
        if f.created.is_empty() {
            f.created.clone_from(&self.r6.details.created);
        }
        if f.last_modified.is_empty() {
            f.last_modified.clone_from(&self.r6.details.modified);
        }
        f
    }

    /// Input while the File screen or the Options dialog is open; `None` lets it through.
    pub(super) fn r6_input(&mut self, input: &Input, text: &dyn TextMeasure) -> Option<bool> {
        if self.r6.options.is_some() {
            return Some(self.r6_options_input(input, text));
        }
        if self.r6.bs.is_none() {
            if self.menu.is_none() {
                if let Some(r) = self.r6_review_input(input) {
                    return Some(r);
                }
            }
            return self.r6_split_release(input);
        }
        if self.menu.is_some() {
            if let Some(r) = self.menu_input(input) {
                return Some(r);
            }
        }
        let top = self.title.height();
        let b = self.r6.bs.as_mut()?;
        let out = match *input {
            Input::Move { x, y } if y >= top => {
                return Some(b.hover(x, y));
            }
            Input::Press { x, y, .. } if y >= top => b.press(x, y),
            Input::Key(Key::Escape, _) => BsOut::Back,
            Input::Text(ref s) => {
                return Some(b.type_text(s));
            }
            Input::Key(k, _) => return Some(b.key(k)),
            Input::Wheel(dy) => return Some(b.wheel(dy)),
            Input::Context { .. } | Input::Preedit(_) | Input::Drag { .. } => {
                return Some(false);
            }
            Input::Release { y, .. } if y >= top => return Some(false),
            _ => return None,
        };
        Some(self.r6_out(out))
    }

    fn r6_out(&mut self, out: BsOut) -> bool {
        let close = |w: &mut Self| {
            w.r6.bs = None;
            true
        };
        match out {
            BsOut::None => false,
            BsOut::Redraw => true,
            BsOut::Back => close(self),
            BsOut::Save => {
                self.requests.push(Request::Save);
                close(self)
            }
            BsOut::Close => {
                self.requests.push(Request::Close);
                close(self)
            }
            BsOut::Options => {
                self.r6.bs = None;
                self.r6_open_options()
            }
            BsOut::BrowseOpen => {
                self.requests.push(Request::Open);
                close(self)
            }
            BsOut::OpenPath(p) => {
                self.requests.push(Request::OpenPath(p));
                close(self)
            }
            BsOut::New(t) => {
                self.requests.push(match t {
                    Some(id) => Request::NewFromTemplate(id.to_owned()),
                    None => Request::New,
                });
                close(self)
            }
            BsOut::SaveAsType(ext) => {
                self.requests.push(if ext == "docx" {
                    Request::SaveAs
                } else {
                    Request::SaveAsType(ext.to_owned())
                });
                close(self)
            }
            BsOut::ExportPdf => {
                self.requests.push(Request::ExportPdf);
                close(self)
            }
            BsOut::CopyPath => {
                if let Some((p, _)) = &self.r6.file {
                    self.requests.push(Request::SetClipboard {
                        text: p.clone(),
                        html: String::new(),
                        internal: String::new(),
                    });
                }
                false
            }
            BsOut::OpenLocation => {
                if let Some((p, _)) = &self.r6.file {
                    self.requests.push(Request::OpenLocation(p.clone()));
                }
                false
            }
            BsOut::Print => {
                let Some(b) = self.r6.bs.as_ref() else {
                    return false;
                };
                let printer = b.print.printers.get(b.print.printer).cloned();
                if let Some(printer) = printer {
                    self.requests.push(Request::PrintJob(PrintJob {
                        printer,
                        copies: b.print.copies.max(1),
                        pages: b.pages_to_print(),
                        collate: b.print.collated,
                        duplex: b.print.sides,
                        per_sheet: b.print.per_sheet.max(1),
                    }));
                    return close(self);
                }
                false
            }
            BsOut::Menu(id, anchor) => self.r6_menu(id, anchor),
            BsOut::PrinterProperties => {
                if let Some(b) = self.r6.bs.as_ref() {
                    if let Some(n) = b.print.printers.get(b.print.printer) {
                        self.requests.push(Request::PrinterProperties(n.clone()));
                    }
                }
                false
            }
            BsOut::PageSetup => {
                self.r6.bs = None;
                self.action("layout.pageSetup")
            }
            BsOut::Convert => {
                // The next save writes the current compatibility mode (15).
                self.r6.details.compat = false;
                if let Some(b) = self.r6.bs.as_mut() {
                    b.facts.compat = false;
                    b.show(BsPage::Info);
                }
                true
            }
            BsOut::Email => {
                let body = self
                    .r6
                    .file
                    .as_ref()
                    .map_or_else(String::new, |(p, _)| p.clone());
                self.requests.push(Request::Email {
                    subject: self.name.clone(),
                    body,
                });
                close(self)
            }
        }
    }

    /// Drop-down lists of the File screen.
    fn r6_menu(&mut self, id: &str, anchor: [f32; 4]) -> bool {
        let Some(b) = self.r6.bs.as_ref() else {
            return false;
        };
        let item = |label: String, cmd: &str, checked: bool, dim: bool| MenuItem {
            label,
            choice: Choice::Command(cmd.to_owned()),
            checked,
            dim,
        };
        let protected = b.facts.protected;
        let p = &b.print;
        let items: Vec<MenuItem> = match id {
            // Round 7: every entry works but the digital signature (no certificate store).
            "bs6.protect" => {
                let sec = self
                    .view
                    .run_json(&json!({ "cmd": "security" }))
                    .map(|v| v["security"].clone())
                    .unwrap_or_default();
                let flag = |k: &str| sec[k].as_bool().unwrap_or(false);
                vec![
                    item(
                        self.t("docR6.m.readOnly", "Always Open Read-Only"),
                        "file.readOnlyRecommended",
                        flag("readOnlyRecommended"),
                        false,
                    ),
                    item(
                        self.t("docR6.m.encrypt", "Encrypt with Password"),
                        "file.encrypt",
                        self.view.engine().save_password_set(),
                        false,
                    ),
                    item(
                        self.t("docR6.m.restrict", "Restrict Editing"),
                        "bs6.restrict",
                        protected,
                        false,
                    ),
                    item(
                        self.t("docR6.m.signature", "Add a Digital Signature"),
                        "bs6.none",
                        false,
                        true,
                    ),
                    item(
                        self.t("docR6.m.final", "Mark as Final"),
                        "file.markFinal",
                        flag("markedFinal"),
                        false,
                    ),
                ]
            }
            "bs6.inspect" => vec![
                item(
                    self.t("docR6.m.inspect", "Inspect Document"),
                    "file.inspect",
                    false,
                    false,
                ),
                item(
                    self.t("docR6.m.accessibility", "Check Accessibility"),
                    "file.accessibility",
                    false,
                    false,
                ),
                item(
                    self.t("docR6.m.compatibility", "Check Compatibility"),
                    "file.compatibility",
                    false,
                    false,
                ),
            ],
            "bs6.manage" => vec![item(
                self.t("docR6.m.recover", "Recover Unsaved Documents"),
                "bs6.none",
                false,
                true,
            )],
            "bs6.printer" => {
                let mut v: Vec<MenuItem> = p
                    .printers
                    .iter()
                    .enumerate()
                    .map(|(i, n)| {
                        item(
                            n.clone(),
                            &format!("bs6.printer.{i}"),
                            i == p.printer,
                            false,
                        )
                    })
                    .collect();
                if v.is_empty() {
                    v.push(item(
                        self.t("docR6.bs.noPrinter", "No printer installed"),
                        "bs6.none",
                        false,
                        true,
                    ));
                }
                v
            }
            "bs6.range" => vec![
                item(
                    self.t("docR6.bs.allPages", "Print All Pages"),
                    "bs6.range.0",
                    p.range == 0,
                    false,
                ),
                item(
                    self.t("docR6.bs.currentPage", "Print Current Page"),
                    "bs6.range.1",
                    p.range == 1,
                    false,
                ),
                item(
                    self.t("docR6.bs.customPrint", "Custom Print"),
                    "bs6.range.2",
                    p.range == 2,
                    false,
                ),
            ],
            "bs6.sides" => vec![
                item(
                    self.t("docR6.bs.oneSided", "Print One Sided"),
                    "bs6.sides.0",
                    p.sides == 0,
                    false,
                ),
                item(
                    self.t(
                        "docR6.bs.bothLongSub",
                        "Print on Both Sides — flip pages on long edge",
                    ),
                    "bs6.sides.1",
                    p.sides == 1,
                    false,
                ),
                item(
                    self.t(
                        "docR6.bs.bothShortSub",
                        "Print on Both Sides — flip pages on short edge",
                    ),
                    "bs6.sides.2",
                    p.sides == 2,
                    false,
                ),
            ],
            "bs6.collate" => vec![
                item(
                    self.t("docR6.bs.collated", "Collated"),
                    "bs6.collate.1",
                    p.collated,
                    false,
                ),
                item(
                    self.t("docR6.bs.uncollated", "Uncollated"),
                    "bs6.collate.0",
                    !p.collated,
                    false,
                ),
            ],
            "bs6.orientation" => vec![
                item(
                    self.t("docR6.bs.portrait", "Portrait Orientation"),
                    "bs6.do.layout.orientation.portrait",
                    !b.facts.landscape,
                    false,
                ),
                item(
                    self.t("docR6.bs.landscape", "Landscape Orientation"),
                    "bs6.do.layout.orientation.landscape",
                    b.facts.landscape,
                    false,
                ),
            ],
            "bs6.paper" => crate::doc_r4_menus::SIZES
                .iter()
                .filter(|(k, ..)| matches!(*k, "letter" | "legal" | "a3" | "a4" | "a5"))
                .map(|(k, n, w, h)| {
                    item(
                        format!("{n}  ({w} × {h} cm)"),
                        &format!("bs6.do.layout.size.{k}"),
                        b.facts.paper.0 == *n,
                        false,
                    )
                })
                .collect(),
            "bs6.margins" => crate::doc_r4_menus::MARGINS
                .iter()
                .filter(|(k, ..)| *k != "mirrored")
                .map(|(k, n, _)| {
                    item(
                        (*n).to_owned(),
                        &format!("bs6.do.layout.margins.{k}"),
                        b.facts.margins.starts_with(n),
                        false,
                    )
                })
                .collect(),
            "bs6.perSheet" => [1_u8, 2, 4, 6, 8, 16]
                .iter()
                .map(|n| {
                    let label = if *n == 1 {
                        self.t("docR6.bs.onePerSheet", "1 Page Per Sheet")
                    } else {
                        format!("{n} {}", self.t("docR6.bs.perSheet", "Pages Per Sheet"))
                    };
                    item(
                        label,
                        &format!("bs6.perSheet.{n}"),
                        p.per_sheet == *n,
                        false,
                    )
                })
                .collect(),
            "bs6.saveType" => FILE_TYPES
                .iter()
                .enumerate()
                .map(|(i, (_, k, f))| {
                    item(
                        self.t(k, f),
                        &format!("bs6.type.{i}"),
                        i == b.file_type,
                        false,
                    )
                })
                .collect(),
            _ => return false,
        };
        let anchor = [anchor[0], anchor[1], anchor[2].max(1.0), anchor[3]];
        self.menu = Some(Menu::open(items, anchor, self.size, self.scale));
        true
    }

    /// Commands of the File screen's menus (`bs6.*`) and File › Options.
    pub(super) fn r6_action(&mut self, id: &str) -> Option<bool> {
        if id == "file.options" {
            return Some(self.r6_open_options());
        }
        if let Some(r) = self.r6_review_action(id) {
            return Some(r);
        }
        // Scripted captures (BLUEOFFICE_ACTIONS): `file:<page>` opens the File screen there
        // (`file:export.types` with Change File Type chosen), `options:<n>` File › Options on
        // category n.
        if let Some(pg) = id.strip_prefix("file:") {
            let (pg, sub) = pg.split_once('.').unwrap_or((pg, ""));
            let r = self.open_file_screen(pg);
            if sub == "types" {
                if let Some(b) = self.r6.bs.as_mut() {
                    b.export = 1;
                    b.show(BsPage::Export);
                }
            }
            return Some(r);
        }
        // `select:<text>` selects the first match of the text (scripted comparisons).
        if let Some(q) = id.strip_prefix("select:") {
            let m = crate::catalog::from_json(json!({ "cmd": "find", "query": q }))
                .and_then(|c| self.view.exec(c))
                .and_then(|r| r.matches)
                .and_then(|v| v.first().copied());
            return Some(
                m.and_then(|m| {
                    crate::catalog::from_json(json!({ "cmd": "selectMatch", "match": m }))
                })
                .is_some_and(|c| self.view.run_command(c)),
            );
        }
        if let Some(n) = id.strip_prefix("options:") {
            self.r6_open_options();
            if let Some(o) = self.r6.options.as_mut() {
                o.set_page(n.parse().unwrap_or(0));
            }
            return Some(true);
        }
        let rest = id.strip_prefix("bs6.")?;
        let num = |s: &str| s.rsplit('.').next().and_then(|v| v.parse::<usize>().ok());
        if let Some(cmd) = rest.strip_prefix("do.") {
            // A page setup command of the Layout tab, then fresh facts for the page.
            let r = self.action(cmd);
            let facts = self.r6_facts();
            if let Some(b) = self.r6.bs.as_mut() {
                b.facts = facts;
            }
            return Some(r);
        }
        if rest == "restrict" {
            self.r6.bs = None;
            return Some(self.action("review.restrictEditing"));
        }
        let b = self.r6.bs.as_mut()?;
        match rest.split('.').next().unwrap_or("") {
            "printer" => b.print.printer = num(rest).unwrap_or(0),
            "range" => b.print.range = num(rest).unwrap_or(0).min(2) as u8,
            "sides" => b.print.sides = num(rest).unwrap_or(0).min(2) as u8,
            "collate" => b.print.collated = num(rest) == Some(1),
            "perSheet" => b.print.per_sheet = num(rest).unwrap_or(1).clamp(1, 16) as u8,
            "type" => b.file_type = num(rest).unwrap_or(0).min(FILE_TYPES.len() - 1),
            _ => return Some(false),
        }
        Some(true)
    }

    /// Per-frame layout of round-6 parts and the print preview's page image.
    pub(super) fn r6_layout(&mut self, text: &dyn TextMeasure) {
        let (top, size, scale) = (self.title.height(), self.size, self.scale);
        if let Some(b) = self.r6.bs.as_mut() {
            b.measure(text, &self.strings);
            b.layout(top, size, scale);
            if let Some((i, r)) = b.preview_rect() {
                self.view.set_thumbnails(vec![i], r[2]);
            }
        }
        if let Some(o) = self.r6.options.as_mut() {
            o.layout(text, size, scale);
        }
        self.r6_layout_review();
    }

    /// Paint the File screen (with its open menu) and the Options dialog over the window.
    /// Panes of round 6 (under menus and dialogs).
    pub(super) fn r6_paint_panes(&self, p: &mut dyn Painter) {
        if let Some(rp) = &self.r6.review {
            rp.paint(p, &self.strings, self.theme.accent_strong);
        }
    }

    pub(super) fn r6_paint(&self, p: &mut dyn Painter) {
        if let Some(b) = &self.r6.bs {
            b.paint(p, &self.theme, &self.strings);
            if let Some((i, r)) = b.preview_rect() {
                let a = b.preview_area();
                p.push_clip(a[0], a[1], a[2], a[3]);
                p.page(crate::THUMB_KEY + i, r[0] as i32, r[1] as i32);
                p.pop_clip();
            }
            if let Some(menu) = &self.menu {
                menu.paint(p, &self.theme, &self.icons);
            }
        }
        if let Some(o) = &self.r6.options {
            o.paint(p, self.size, true);
        }
    }

    // ------------------------------------------------------------ host interface

    /// Path and size of the open file, shown on File › Info (`None`: not saved yet).
    pub fn set_file_info(&mut self, path: Option<&str>, bytes: Option<u64>) {
        self.r6_set_file(path, bytes);
    }

    /// File › Info facts the host read from the file (editing time, compatibility mode,
    /// file system dates).
    pub fn set_file_details(&mut self, d: FileDetails) {
        self.r6.details = d;
    }

    /// Spooler state of each printer given to [`DocWindow::set_printers`]: (error, offline,
    /// jobs waiting).
    pub fn set_printer_states(&mut self, states: Vec<(bool, bool, u32)>) {
        self.r6.printer_states = states;
    }

    /// BlueDoc's own recent documents, most recent first: (path, when opened as text).
    pub fn set_recent(&mut self, recent: Vec<(String, String)>) {
        self.r6.recent = recent
            .into_iter()
            .map(|(path, when)| Recent { path, when })
            .collect();
    }

    /// Installed printers and the index of the default one (File › Print).
    pub fn set_printers(&mut self, names: Vec<String>, default: usize) {
        let d = default.min(names.len().saturating_sub(1));
        self.r6.printers = (names, d);
    }

    /// Application preferences the host read at start (`key=value` lines).
    pub fn set_prefs(&mut self, text: &str) {
        self.r6_set_prefs(text);
    }

    /// Preferences changed in File › Options, for the host to store.
    pub fn take_prefs(&mut self) -> Option<String> {
        self.r6.prefs_out.take()
    }

    /// Interface language chosen in File › Options (empty: the default).
    pub fn preferred_locale(&self) -> &str {
        &self.r6.prefs.locale
    }

    /// Default local file location (File › Options › Save; empty: none).
    pub fn default_folder(&self) -> &str {
        &self.r6.prefs.folder
    }

    /// Extension Save As offers first (File › Options › Save).
    pub fn save_format(&self) -> &'static str {
        if self.r6.prefs.save_format == 1 {
            "dotx"
        } else {
            "docx"
        }
    }

    /// Fill a new document with one of BlueOffice's own templates (File › New).
    pub fn apply_template(&mut self, id: &str) -> bool {
        let Some(html) = crate::doc_templates6::html(id) else {
            return false;
        };
        self.view
            .paste_with(None, Some(html.to_owned()), None, None)
    }

    /// Open the File screen on a page (`home`, `new`, `open`, `info`, `saveAs`, `print`,
    /// `export`) — scripted captures (`BLUEOFFICE_ACTIONS` `file:<page>`).
    pub fn open_file_screen(&mut self, page: &str) -> bool {
        let p = match page {
            "home" => BsPage::Home,
            "new" => BsPage::New,
            "open" => BsPage::Open,
            "saveAs" => BsPage::SaveAs,
            "print" => BsPage::Print,
            "export" => BsPage::Export,
            "share" => BsPage::Share,
            _ => BsPage::Info,
        };
        self.r6_open_backstage(p)
    }

    /// Path and size of the open file, for the File screen.
    pub(super) fn r6_set_file(&mut self, path: Option<&str>, bytes: Option<u64>) {
        self.r6.file = path.map(|p| (p.to_owned(), bytes));
    }

    /// Preferences read by the host at start (`key=value` lines).
    pub(super) fn r6_set_prefs(&mut self, text: &str) {
        let p = DocPrefs::from_text(text);
        self.r6_apply_prefs(&p);
        self.r6_set_quick(&p.quick);
        self.r20_apply_ribbon(&p.extra.hidden_tabs);
        self.r6.prefs = p;
    }

    /// Make preferences take effect.
    pub(super) fn r6_apply_prefs(&mut self, p: &DocPrefs) {
        if !p.user_name.trim().is_empty() {
            let name = p.user_name.trim().to_owned();
            self.view.run_command(Command::SetAuthor { name });
        }
        self.view.set_hide_spelling(!p.check_spelling);
        if p.units <= 1 {
            let decimal = self.units.decimal;
            self.units = crate::doc_table::Units {
                inches: p.units == 0,
                decimal,
            };
        }
        // Each kind of mark on its own (round 7), and all of them with the ¶ button.
        let k = p.mark_kinds;
        let kinds = bo_engine::views::MarkKinds {
            tabs: k[0],
            spaces: k[1],
            paragraphs: k[2],
            hidden: k[3],
            optional_hyphens: k[4],
            anchors: k[5],
        };
        self.marks = p.show_marks;
        let on = self.marks;
        self.view.run_command(Command::ShowFormattingMarks {
            on,
            kinds: Some(kinds),
        });
    }

    /// Preferences changed in File › Options: store them through the host.
    pub(super) fn r6_store_prefs(&mut self, p: DocPrefs) {
        self.r6_apply_prefs(&p);
        if p != self.r6.prefs {
            self.r6.prefs_out = Some(p.to_text());
            self.requests.push(Request::SavePrefs);
        }
        self.r6.prefs = p;
    }

    /// File › Options.
    pub(super) fn r6_open_options(&mut self) -> bool {
        self.menu = None;
        let mut o = options::OptionsDialog::open(&self.strings, &self.r6.prefs);
        o.set_page(0);
        self.r6.options = Some(o);
        true
    }

    fn r6_options_input(&mut self, input: &Input, text: &dyn TextMeasure) -> bool {
        let Some(o) = self.r6.options.as_mut() else {
            return false;
        };
        match o.input(input) {
            options::OptOut::None => false,
            options::OptOut::Redraw => true,
            options::OptOut::AutoCorrect => self.r8_open_autocorrect(),
            options::OptOut::Close => {
                self.r6.options = None;
                true
            }
            options::OptOut::Apply(p) => {
                self.r6.options = None;
                self.r6_set_quick(&p.quick);
                self.r20_apply_ribbon(&p.extra.hidden_tabs);
                self.r6_store_prefs(p);
                self.relayout(text);
                true
            }
        }
    }

    /// The title bar's quick access commands.
    fn r6_set_quick(&mut self, ids: &[String]) {
        let ids: Vec<&'static str> = ids
            .iter()
            .filter_map(|id| {
                options::QUICK_CHOICES
                    .iter()
                    .find(|(c, ..)| c == id)
                    .map(|(c, ..)| *c)
            })
            .collect();
        self.title.set_quick_commands(ids);
    }

    /// ScreenTip style: 0 with descriptions, 1 without, 2 none.
    pub(super) fn r6_screentips(&self) -> u8 {
        self.r6.prefs.screentips
    }

    #[allow(dead_code)]
    fn r6_debug(&self) -> serde_json::Value {
        json!({ "page": self.r6.bs.as_ref().map(|b| format!("{:?}", b.page)) })
    }
}

#[cfg(test)]
#[path = "doc_r6_tests.rs"]
mod tests;
