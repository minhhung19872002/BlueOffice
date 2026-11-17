//! BlueEditor round 9: Combine Files as the reference editor shows it (black-box, 125 %): a
//! document tab "Combine Files" with a contextual ribbon tab of the same name (Add Files, Add
//! Folder, Add WebPage, Add PDF from Scanner, Add From Clipboard, Add Open Files, Remove |
//! Settings ▾, Move Up, Move Down | a filled Combine button and a Cancel button at the right)
//! over a full-width list of the files: Name / Operation / Modified / Page Range / Size /
//! Bookmark for File / Bates Numbering / Warnings/Errors / Location (header 50 px, rows 59 px,
//! #DEDEDE lines, grey headers, near-black rows). Double-clicking a row's page range asks for
//! a new one ("1-3, 5"; empty: all pages). Combine makes a new document of the ranges in list
//! order and opens it in a tab (the files themselves are not changed). Folders, web pages and
//! scanners are dimmed (the host picks files only, no capture devices).
use crate::editor_dlg8::FormWhat;
use crate::editor_form8::{button, edit, label, Form, On, Val};
use crate::editor_window::{EditorWindow, TABS};
use crate::reader_icons::{self, Ink};
use crate::reader_window::{Kind, ReaderRequest};
use crate::{Input, Key, Painter, TextMeasure, TextStyle};
use bo_render::{PathSeg, Rgba};

/// One file of the list.
#[derive(Debug, Clone)]
pub(crate) struct CombineItem {
    pub name: String,
    pub bytes: Vec<u8>,
    pub pages: usize,
    /// Page range text ("" = all pages).
    pub range: String,
    /// Added from an open document tab (closed after combining when asked).
    pub from_tab: bool,
}

/// The Combine Files tab: selected row, whether its document tab is the one shown, whether a
/// clipboard picture is awaited, and the last painted geometry.
#[derive(Debug, Clone, Default)]
pub(crate) struct Comb9 {
    pub row: usize,
    /// Whether a row is selected (the reference starts with none).
    pub picked: bool,
    pub doc_on: bool,
    pub clip: bool,
    tab: [f32; 4],
    close: [f32; 4],
    list: [f32; 4],
    rows: Vec<[f32; 4]>,
    range_x: (f32, f32),
    combine_btn: [f32; 4],
    cancel_btn: [f32; 4],
}

/// Ribbon tab key of the contextual tab.
pub(crate) const TAB_KEY: &str = "pdfEditor9.combineTitle";

/// Column x positions (125 % px from the window's left edge) and label keys.
const COLS: [(f32, &str); 9] = [
    (98.0, "pdfEditor9.colName"),
    (352.0, "pdfEditor9.colOperation"),
    (473.0, "pdfEditor9.colModified"),
    (674.0, "pdfEditor9.colRange"),
    (808.0, "pdfEditor9.colSize"),
    (906.0, "pdfEditor9.colBookmark"),
    (1091.0, "pdfEditor9.colBates"),
    (1273.0, "pdfEditor9.colWarnings"),
    (1446.0, "pdfEditor9.colLocation"),
];

const LINE: Rgba = Rgba(0xDE, 0xDE, 0xDE, 255);
const HEAD: Rgba = Rgba(0x99, 0x99, 0x99, 255);
const INK: Rgba = Rgba(0x22, 0x22, 0x22, 255);
const SEL: Rgba = Rgba(0xCC, 0xE8, 0xFF, 255);
const WHITE: Rgba = Rgba(255, 255, 255, 255);

/// Items of the contextual ribbon tab (Combine / Cancel are painted at the right).
pub(crate) fn ribbon_items() -> Vec<(&'static str, Kind)> {
    use Kind::{Big, Sep};
    vec![
        (
            "comb9.addFiles",
            Big("createPdf", "pdfEditor9.c9AddFiles", false),
        ),
        (
            "comb9.addFolder",
            Big("open", "pdfEditor9.c9AddFolder", false),
        ),
        ("comb9.addWeb", Big("fromWeb", "pdfEditor9.c9AddWeb", false)),
        (
            "comb9.addScanner",
            Big("scanner", "pdfEditor9.c9AddScanner", false),
        ),
        (
            "comb9.addClipboard",
            Big("fromClipboard", "pdfEditor9.c9AddClipboard", false),
        ),
        ("comb9.addOpen", Big("pages", "pdfEditor9.c9AddOpen", false)),
        (
            "comb9.remove",
            Big("deletePage", "pdfEditor9.remove", false),
        ),
        ("comb9.sep1", Sep),
        (
            "comb9.settings",
            Big("properties", "pdfEditor9.c9Settings", true),
        ),
        ("comb9.up", Big("chevronUp", "pdfEditor9.moveUp", false)),
        ("comb9.down", Big("arrowDown", "pdfEditor9.moveDown", false)),
        ("comb9.sep2", Sep),
    ]
}

/// Commands of the contextual tab that are shown but not offered.
pub(crate) const DIMMED: [&str; 2] = ["comb9.addWeb", "comb9.addScanner"];

/// Label keys of the Settings menu's options (the reference's three toggles).
pub(crate) const OPTIONS: [&str; 3] = [
    "pdfEditor9.c9OptToc",
    "pdfEditor9.c9OptLabels",
    "pdfEditor9.c9OptClose",
];

/// Combine options: a contents page made of the files' bookmarks, the files' own page
/// labels kept, the open files that were added closed afterwards.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct CombineOpts {
    pub toc: bool,
    pub labels: bool,
    pub close: bool,
}

fn item(name: &str, bytes: Vec<u8>) -> Option<CombineItem> {
    // Documents, workbooks and presentations are made into PDF first (the BlueOffice engines).
    let bytes = if bytes.starts_with(b"%PDF") || name.to_ascii_lowercase().ends_with(".pdf") {
        bytes
    } else {
        crate::editor_actions::office_to_pdf(name, &bytes).ok()?
    };
    let doc = bo_pdfread::PdfDocument::open(bytes.clone(), bo_pdfread::standard_fonts()).ok()?;
    Some(CombineItem {
        name: name.to_owned(),
        pages: doc.page_count(),
        bytes,
        range: String::new(),
        from_tab: false,
    })
}

fn from_tab(mut c: CombineItem) -> CombineItem {
    c.from_tab = true;
    c
}

fn inside(r: [f32; 4], x: f32, y: f32) -> bool {
    x >= r[0] && x < r[0] + r[2] && y >= r[1] && y < r[1] + r[3]
}

/// A rounded bar (ends are half circles).
fn pill(r: [f32; 4]) -> Vec<PathSeg> {
    let rad = r[3] / 2.0;
    let k = rad * 0.552_285;
    let (x0, y0, x1, y1) = (r[0], r[1], r[0] + r[2], r[1] + r[3]);
    vec![
        PathSeg::M(x0 + rad, y0),
        PathSeg::L(x1 - rad, y0),
        PathSeg::C(x1 - rad + k, y0, x1, y0 + rad - k, x1, y0 + rad),
        PathSeg::C(x1, y0 + rad + k, x1 - rad + k, y1, x1 - rad, y1),
        PathSeg::L(x0 + rad, y1),
        PathSeg::C(x0 + rad - k, y1, x0, y0 + rad + k, x0, y0 + rad),
        PathSeg::C(x0, y0 + rad - k, x0 + rad - k, y0, x0 + rad, y0),
        PathSeg::Z,
    ]
}

impl EditorWindow {
    /// Open Combine Files with the active document first.
    pub(crate) fn open_combine9(&mut self) {
        if self.comb9.is_none() {
            self.combine9.clear();
            let name = self.doc_name();
            let name = if name.to_ascii_lowercase().ends_with(".pdf") {
                name
            } else {
                format!("{name}.pdf")
            };
            if let Some(bytes) = self.r.view_mut().and_then(|v| v.doc_mut().save().ok()) {
                self.combine9.extend(item(&name, bytes).map(from_tab));
            }
        }
        let row = self.comb9.as_ref().map_or(0, |c| c.row);
        self.comb9 = Some(Comb9 {
            row,
            doc_on: true,
            ..Comb9::default()
        });
        self.close_backstage();
        self.r
            .set_context_tab9(&TABS, Some(TAB_KEY), true, &crate::editor_window::Estimate);
    }

    /// Whether the Combine Files tab is the one shown.
    #[cfg(test)]
    pub(crate) fn combine9_shown(&self) -> bool {
        self.comb9.as_ref().is_some_and(|c| c.doc_on)
    }

    /// Close the Combine Files tab (Cancel, its close box, or after Combine).
    pub(crate) fn close_combine9(&mut self) {
        self.comb9 = None;
        self.combine9.clear();
        self.r
            .set_context_tab9(&TABS, None, false, &crate::editor_window::Estimate);
    }

    fn comb_row(&self) -> usize {
        self.comb9.as_ref().map_or(0, |c| c.row)
    }

    fn set_row(&mut self, row: usize) {
        let n = self.combine9.len();
        if let Some(c) = &mut self.comb9 {
            c.row = row.min(n.saturating_sub(1));
            c.picked = n > 0;
        }
    }

    /// Commands of the contextual tab; false when `cmd` is not one.
    pub(crate) fn combine9_command(&mut self, cmd: &str) -> bool {
        if !cmd.starts_with("comb9.") {
            return false;
        }
        if self.comb9.is_none() {
            return true;
        }
        let row = self.comb_row();
        let picked = self.comb9.as_ref().is_some_and(|c| c.picked);
        if !picked && matches!(cmd, "comb9.remove" | "comb9.up" | "comb9.down") {
            return true;
        }
        match cmd {
            "comb9.addFiles" => self.push(ReaderRequest::PickFile {
                purpose: "combine9".into(),
                filters: vec![("PDF".into(), "*.pdf".into())],
            }),
            "comb9.addFolder" => {
                let mut filters = vec![("PDF".to_owned(), "*.pdf".to_owned())];
                filters.extend(
                    crate::editor_actions::OFFICE_FILTERS
                        .iter()
                        .map(|(a, b)| ((*a).to_owned(), (*b).to_owned())),
                );
                self.push(ReaderRequest::PickFile {
                    purpose: "combine9.folder".into(),
                    filters,
                });
            }
            "comb9.settings" => self.combine9_settings_menu(),
            "comb9.opt.0" => self.comb9_opts.toc = !self.comb9_opts.toc,
            "comb9.opt.1" => self.comb9_opts.labels = !self.comb9_opts.labels,
            "comb9.opt.2" => self.comb9_opts.close = !self.comb9_opts.close,
            "comb9.addClipboard" => {
                if let Some(c) = &mut self.comb9 {
                    c.clip = true;
                }
                self.push(ReaderRequest::Command("host.clipboardImage".into()));
            }
            "comb9.addOpen" => {
                for i in 0..self.r.tab_count() {
                    let name = self.r.doc_tab_name(i).unwrap_or_default().to_owned();
                    if self.combine9.iter().any(|c| c.name == name) {
                        continue;
                    }
                    if let Some(bytes) =
                        self.r.tab_view_mut(i).and_then(|v| v.doc_mut().save().ok())
                    {
                        self.combine9.extend(item(&name, bytes).map(from_tab));
                    }
                }
            }
            "comb9.remove" => {
                if row < self.combine9.len() {
                    self.combine9.remove(row);
                }
                self.set_row(row);
            }
            "comb9.up" if row > 0 && row < self.combine9.len() => {
                self.combine9.swap(row, row - 1);
                self.set_row(row - 1);
            }
            "comb9.down" if row + 1 < self.combine9.len() => {
                self.combine9.swap(row, row + 1);
                self.set_row(row + 1);
            }
            "comb9.combine" => self.combine9_run(),
            "comb9.cancel" => self.close_combine9(),
            _ => {}
        }
        if let Some(c) = &mut self.comb9 {
            c.doc_on = true;
        }
        true
    }

    /// Settings: the reference's three options, ticked when on.
    fn combine9_settings_menu(&mut self) {
        let Some(r) = self.r.control_rect("comb9.settings") else {
            return;
        };
        let o = self.comb9_opts;
        let on = [o.toc, o.labels, o.close];
        let items = OPTIONS
            .iter()
            .enumerate()
            .map(|(k, key)| crate::popup::MenuItem {
                label: self.t(key),
                choice: crate::popup::Choice::Command(format!("comb9.opt.{k}")),
                checked: on[k],
                dim: false,
            })
            .collect();
        let menu = self.menu_open9(items, r);
        self.r.set_menu9(menu, "comb9.settings");
    }

    /// Enabled states of the contextual tab's commands.
    pub(crate) fn combine9_enabled(&self, id: &str) -> Option<bool> {
        if !id.starts_with("comb9.") {
            return None;
        }
        let (row, n) = (self.comb_row(), self.combine9.len());
        let picked = self.comb9.as_ref().is_some_and(|c| c.picked) && n > 0;
        Some(match id {
            "comb9.remove" => picked,
            "comb9.up" => picked && row > 0,
            "comb9.down" => picked && row + 1 < n,
            _ => !DIMMED.contains(&id),
        })
    }

    /// A file (or clipboard picture) picked for Combine Files.
    pub(crate) fn combine9_file_picked(&mut self, purpose: &str, name: &str, bytes: &[u8]) -> bool {
        let clip = purpose == "clipboardImage" && self.comb9.as_ref().is_some_and(|c| c.clip);
        if purpose != "combine9" && !clip {
            return false;
        }
        let new = if clip {
            if let Some(c) = &mut self.comb9 {
                c.clip = false;
            }
            let png = bo_raster::codecs::decode_dib(bytes).and_then(bo_raster::rgba_to_png);
            let n = self.combine9.len() + 1;
            png.and_then(|p| crate::editor_more::picture_pdf(&p).ok())
                .and_then(|pdf| {
                    item(
                        &format!("{} {n}.pdf", self.t("pdfEditor9.fromClipboard")),
                        pdf,
                    )
                })
        } else {
            item(name, bytes.to_vec())
        };
        if let Some(c) = new {
            self.combine9.push(c);
        } else {
            let s = self.t("pdfEditor9.notPdf");
            self.set_status(&s);
        }
        let row = self.combine9.len().saturating_sub(1);
        self.set_row(row);
        true
    }

    /// Combine: the ranges in list order as a new document in a tab.
    fn combine9_run(&mut self) {
        let opts = self.comb9_opts;
        let title = self.t("pdfEditor9.c9TocTitle");
        match combine_with(&self.combine9, opts, &title) {
            Ok(bytes) => {
                let name = format!("{}.pdf", self.t("pdfEditor9.combinedName"));
                if opts.close {
                    self.close_combined_tabs();
                }
                self.close_combine9();
                self.push(ReaderRequest::OpenDocument { name, bytes });
            }
            Err(e) => self.set_status(&e),
        }
    }

    /// Close the open files that were added and have no unsaved changes.
    fn close_combined_tabs(&mut self) {
        let names: Vec<String> = self
            .combine9
            .iter()
            .filter(|c| c.from_tab)
            .map(|c| c.name.clone())
            .collect();
        for i in (0..self.r.tab_count()).rev() {
            let n = self.r.doc_tab_name(i).unwrap_or_default().to_owned();
            let saved = self.r.tab_view_mut(i).is_some_and(|v| !v.modified());
            let named = names
                .iter()
                .any(|x| *x == n || x.strip_suffix(".pdf") == Some(n.as_str()));
            if saved && named {
                self.r.close_tab_now(i);
            }
        }
    }

    /// Ask for the page range of row `row` (double click on its Page Range).
    fn ask_range9(&mut self, row: usize) {
        let Some(c) = self.combine9.get(row) else {
            return;
        };
        let range = c.range.clone();
        let t = |k: &str| self.t(k);
        let f = Form::new(
            t("pdfEditor9.colRange"),
            (420.0, 160.0),
            vec![
                label(17.0, 64.0, t("pdfEditor9.pageRange")),
                edit("c9range", [123.0, 50.0, 280.0, 28.0], On::Always),
                label(123.0, 96.0, t("pdfEditor9.sample")),
                button(
                    "ok",
                    [212.0, 118.0, 91.0, 28.0],
                    t("pdfReader.ok"),
                    On::Always,
                ),
                button(
                    "cancel",
                    [312.0, 118.0, 91.0, 28.0],
                    t("pdfReader.cancel"),
                    On::Always,
                ),
            ],
        )
        .with("c9range", Val::Text(range))
        .focused("c9range");
        self.form8.push((f, FormWhat::Combine9));
    }

    /// OK of the page range question.
    pub(crate) fn combine9_ok(&mut self, f: &Form) {
        let row = self.comb_row();
        if let Some(c) = self.combine9.get_mut(row) {
            c.range = f.text("c9range").trim().to_owned();
        }
    }

    /// Pointer and keys of the Combine Files tab; true when used.
    pub(crate) fn combine9_input(&mut self, input: &Input, m: &dyn TextMeasure) -> bool {
        let Some(c) = self.comb9.clone() else {
            return false;
        };
        match *input {
            Input::Press { x, y, count, .. } => {
                if inside(c.close, x, y) {
                    self.close_combine9();
                    return true;
                }
                if inside(c.tab, x, y) {
                    if let Some(s) = &mut self.comb9 {
                        s.doc_on = true;
                    }
                    let last = TABS.len();
                    self.r.set_ribbon_tab(last, m);
                    return true;
                }
                if self.r.ribbon_tab() == TABS.len() {
                    if inside(c.combine_btn, x, y) {
                        self.combine9_command("comb9.combine");
                        return true;
                    }
                    if inside(c.cancel_btn, x, y) {
                        self.close_combine9();
                        return true;
                    }
                }
                if !c.doc_on {
                    return false;
                }
                // Another document tab (or Start): the list steps aside.
                let (start, tabs) = self.r.doc_tab_rects9();
                if std::iter::once(start).chain(tabs).any(|r| inside(r, x, y)) {
                    if let Some(s) = &mut self.comb9 {
                        s.doc_on = false;
                    }
                    if self.r.ribbon_tab() == TABS.len() {
                        self.r.set_ribbon_tab(0, m);
                    }
                    return false;
                }
                if inside(c.list, x, y) {
                    if let Some(k) = c.rows.iter().position(|r| inside(*r, x, y)) {
                        self.set_row(k);
                        if count >= 2 && x >= c.range_x.0 && x < c.range_x.1 {
                            self.ask_range9(k);
                        }
                    }
                    return true;
                }
                false
            }
            Input::Key(k, _) if c.doc_on => {
                let row = c.row;
                match k {
                    Key::Up if row > 0 => self.set_row(row - 1),
                    Key::Down => self.set_row(row + 1),
                    Key::Delete => {
                        self.combine9_command("comb9.remove");
                    }
                    Key::Enter if c.picked => self.ask_range9(row),
                    _ => return false,
                }
                true
            }
            Input::Release { x, y } | Input::Drag { x, y } | Input::Move { x, y }
                if c.doc_on && inside(c.list, x, y) =>
            {
                true
            }
            _ => false,
        }
    }

    /// Paint the tab, the list and the Combine / Cancel buttons.
    #[allow(clippy::too_many_lines)]
    pub(crate) fn paint_combine9(&mut self, p: &mut dyn Painter) {
        let Some(mut c) = self.comb9.clone() else {
            return;
        };
        let scale = self.r.ui_scale();
        let s = scale / 1.25;
        let q = |v: f32| (v * s).round();
        let (w, _) = self.r.window_size();
        let accent = self.r.theme().accent;
        let accent_text = self.r.theme().accent_text;
        let st = TextStyle {
            size: (14.4 * s).round(),
            bold: false,
        };
        let (start, tabs) = self.r.doc_tab_rects9();
        let last = tabs.last().copied().unwrap_or(start);
        // Our document tab after the last one.
        let tab = [
            last[0] + last[2] + (1.6 * scale).round(),
            last[1],
            last[2],
            last[3],
        ];
        if c.doc_on {
            // The document tab the reader drew as selected is shown unselected.
            let active = self.r.active_tab();
            if let Some(r) = tabs.get(active) {
                p.fill_rect(r[0], r[1], r[2], r[3] + 1.0, Rgba(0xE0, 0xE0, 0xE0, 255));
                p.fill_rect(
                    r[0] + 1.0,
                    r[1] + 1.0,
                    r[2] - 2.0,
                    r[3] - 1.0,
                    Rgba(0xF0, 0xF0, 0xF0, 255),
                );
                let name = self.r.doc_tab_name(active).unwrap_or_default().to_owned();
                let modified = self.r.view().is_some_and(crate::ReaderView::modified);
                let label = if modified { format!("{name}*") } else { name };
                p.text(
                    &label,
                    r[0] + (8.0 * scale).round(),
                    r[1] + ((r[3] - st.size * 1.33) / 2.0).round(),
                    st,
                    Rgba(51, 51, 51, 255),
                );
            }
            p.fill_rect(
                tab[0],
                tab[1],
                tab[2],
                tab[3] + 1.0,
                Rgba(0xE0, 0xE0, 0xE0, 255),
            );
            p.fill_rect(
                tab[0] + 1.0,
                tab[1] + 1.0,
                tab[2] - 2.0,
                tab[3] + 1.0,
                WHITE,
            );
        } else {
            p.fill_rect(tab[0], tab[1], tab[2], tab[3], Rgba(0xE0, 0xE0, 0xE0, 255));
            p.fill_rect(
                tab[0] + 1.0,
                tab[1] + 1.0,
                tab[2] - 2.0,
                tab[3] - 1.0,
                Rgba(0xF0, 0xF0, 0xF0, 255),
            );
        }
        p.text(
            &self.t(TAB_KEY),
            tab[0] + (8.0 * scale).round(),
            tab[1] + ((tab[3] - st.size * 1.33) / 2.0).round(),
            st,
            if c.doc_on {
                accent_text
            } else {
                Rgba(51, 51, 51, 255)
            },
        );
        let cs = (12.8 * scale).round();
        let close = [
            tab[0] + tab[2] - cs - (6.4 * scale).round(),
            tab[1] + ((tab[3] - cs) / 2.0).round(),
            cs,
            cs,
        ];
        reader_icons::draw(
            p,
            "close",
            close[0],
            close[1],
            cs,
            Ink {
                ink: Rgba(51, 51, 51, 255),
                accent: Rgba(51, 51, 51, 255),
            },
        );
        c.tab = tab;
        c.close = close;
        // Combine / Cancel at the right of the contextual tab's ribbon.
        c.combine_btn = [0.0; 4];
        c.cancel_btn = [0.0; 4];
        if self.r.ribbon_tab() == TABS.len() && self.r.showing_doc() {
            let top = self.r.tabs_bottom();
            let bottom = tab[1] - (7.2 * scale).round();
            let (bw, bh) = (q(116.0), q(48.0));
            let by = (top + (bottom - top - bh) / 2.0 - q(4.0)).round();
            let cancel = [w - q(78.0) - bw, by, bw, bh];
            let combine = [cancel[0] - q(19.0) - bw, by, bw, bh];
            let enabled = !self.combine9.is_empty();
            p.path(
                &pill(combine),
                Some(if enabled {
                    accent
                } else {
                    Rgba(0xC8, 0xC8, 0xC8, 255)
                }),
                None,
            );
            p.path(
                &pill([
                    cancel[0] + 0.5,
                    cancel[1] + 0.5,
                    cancel[2] - 1.0,
                    cancel[3] - 1.0,
                ]),
                Some(WHITE),
                Some((Rgba(0xE4, 0xE4, 0xE4, 255), 1.0)),
            );
            for (r, key, color) in [
                (combine, "pdfEditor9.combine", WHITE),
                (cancel, "pdfReader.cancel", Rgba(51, 51, 51, 255)),
            ] {
                let label = self.t(key);
                let tw = crate::editor_form8::text_w(&label, st.size);
                p.text(
                    &label,
                    (r[0] + (r[2] - tw) / 2.0).round(),
                    (r[1] + (r[3] - st.size * 1.33) / 2.0).round(),
                    st,
                    color,
                );
            }
            c.combine_btn = if enabled { combine } else { [0.0; 4] };
            c.cancel_btn = cancel;
        }
        // The list, full width under the document tabs.
        c.rows.clear();
        c.list = [0.0; 4];
        if c.doc_on && self.r.showing_doc() {
            let area = self.r.doc_area();
            let top = tab[1] + tab[3] + 1.0;
            let bottom = area[1] + area[3];
            let list = [1.0, top, w - 2.0, bottom - top];
            p.fill_rect(list[0], list[1], list[2], list[3], WHITE);
            let head_h = q(50.0);
            let row_h = q(59.0);
            let row_st = TextStyle {
                size: (16.8 * s).round(),
                bold: false,
            };
            let at_x = |v: f32| (v * s).round();
            p.fill_rect(list[0], top + head_h - 1.0, list[2], 1.0, LINE);
            for (x, key) in COLS {
                p.text(
                    &self.t(key),
                    at_x(x),
                    (top + (head_h - st.size * 1.33) / 2.0).round(),
                    st,
                    HEAD,
                );
            }
            let all = self.t("pdfEditor9.allPages");
            let mut y = top + head_h;
            for (k, it) in self.combine9.iter().enumerate() {
                let r = [list[0], y, list[2], row_h];
                if c.picked && k == c.row {
                    p.fill_rect(r[0], r[1], r[2], r[3] - 1.0, SEL);
                }
                p.fill_rect(r[0], y + row_h - 1.0, r[2], 1.0, LINE);
                let icon = q(19.0);
                reader_icons::draw(
                    p,
                    "file",
                    at_x(55.0),
                    (y + (row_h - icon) / 2.0).round(),
                    icon,
                    Ink {
                        ink: accent,
                        accent,
                    },
                );
                let stem = it
                    .name
                    .strip_suffix(".pdf")
                    .or_else(|| it.name.strip_suffix(".PDF"))
                    .unwrap_or(&it.name)
                    .to_owned();
                let range = if it.range.trim().is_empty() {
                    all.clone()
                } else {
                    it.range.clone()
                };
                let size = format!("{:.2} KB", it.bytes.len() as f32 / 1024.0);
                let ty = (y + (row_h - row_st.size * 1.33) / 2.0).round();
                for (x, text) in [
                    (98.0, it.name.as_str()),
                    (674.0, range.as_str()),
                    (808.0, size.as_str()),
                    (906.0, stem.as_str()),
                ] {
                    p.push_clip(at_x(x), y, at_x(240.0), row_h);
                    p.text(text, at_x(x), ty, row_st, INK);
                    p.pop_clip();
                }
                c.rows.push(r);
                y += row_h;
            }
            c.list = list;
            c.range_x = (at_x(674.0), at_x(808.0));
        }
        self.comb9 = Some(c);
    }
}

/// 0-based pages of `range` ("" or "all": every page) of a file of `n` pages.
pub(crate) fn range_pages(range: &str, n: usize) -> Vec<usize> {
    crate::editor_more::parse_pages(range, "all", n)
        .into_iter()
        .filter(|&p| p < n)
        .collect()
}

/// [`combine`] with the Settings options: the files' page labels kept, and a contents page
/// first listing each file (its "Bookmark for File") with the page it starts on.
pub(crate) fn combine_with(
    items: &[CombineItem],
    opts: CombineOpts,
    title: &str,
) -> Result<Vec<u8>, String> {
    let bytes = combine(items)?;
    if !opts.toc && !opts.labels {
        return Ok(bytes);
    }
    let mut doc = bo_pdfread::PdfDocument::open(bytes, bo_pdfread::standard_fonts())?;
    // Where each file starts and which of its pages came in.
    let mut starts = Vec::new();
    let mut at = 0;
    for c in items {
        let pick = range_pages(&c.range, c.pages);
        at += pick.len();
        starts.push((at - pick.len(), pick));
    }
    if opts.toc {
        let mut rows = String::new();
        for (c, (start, _)) in items.iter().zip(&starts) {
            use std::fmt::Write as _;
            let stem = c.name.strip_suffix(".pdf").unwrap_or(&c.name);
            let _ = write!(
                rows,
                "<tr><td>{}</td><td style='text-align:right'>{}</td></tr>",
                html_escape(stem),
                start + 2
            );
        }
        let html = format!(
            "<html><body><h1>{}</h1><table style='width:100%'>{rows}</table></body></html>",
            html_escape(title)
        );
        let toc = crate::editor_actions::office_to_pdf("contents.html", html.as_bytes())?;
        doc.insert_pdf(0, &toc, Some(&[0]))?;
    }
    let off = usize::from(opts.toc);
    if opts.labels {
        for (c, (start, pick)) in items.iter().zip(&starts) {
            let src = bo_pdfread::PdfDocument::open(c.bytes.clone(), bo_pdfread::standard_fonts())
                .map(|d| d.page_label_ranges())
                .unwrap_or_default();
            for (k, &p) in pick.iter().enumerate() {
                let (style, prefix, n) = match src.iter().rev().find(|r| r.start <= p) {
                    Some(r) => (
                        r.style,
                        r.prefix.clone(),
                        r.first.max(1) + u32::try_from(p - r.start).unwrap_or(0),
                    ),
                    None => (
                        bo_pdfread::LabelStyle::Decimal,
                        String::new(),
                        u32::try_from(p + 1).unwrap_or(1),
                    ),
                };
                doc.set_page_labels(off + start + k, off + start + k, style, &prefix, n)?;
            }
        }
    }
    doc.save()
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// A new PDF of the items' page ranges in order.
pub(crate) fn combine(items: &[CombineItem]) -> Result<Vec<u8>, String> {
    let first = items.first().ok_or("no files")?;
    let mut doc = bo_pdfread::PdfDocument::open(first.bytes.clone(), bo_pdfread::standard_fonts())?;
    let keep = range_pages(&first.range, first.pages);
    if keep.is_empty() {
        return Err(format!("{}: no pages in the range", first.name));
    }
    let drop: Vec<usize> = (0..first.pages).filter(|p| !keep.contains(p)).collect();
    if !drop.is_empty() {
        doc.delete_pages(&drop)?;
    }
    for c in &items[1..] {
        let pick = range_pages(&c.range, c.pages);
        if pick.is_empty() {
            continue;
        }
        let at = doc.page_count();
        doc.insert_pdf(at, &c.bytes, Some(&pick))?;
    }
    doc.save()
}
