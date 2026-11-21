//! BlueEditor's File page (backstage) as the reference editor shows it: a grey column of
//! commands at the left (Properties, Create, Open, Save, Save As, PDF Optimizer, Export,
//! Print, Close), a white page for the chosen one (Open: recent documents and Computer;
//! Create; Export; Properties of the document). Measured at 125 % with 110 % text: column
//! 263 px wide (272 from the window edge, round 8), #EAEAEA, items 47 px apart from y 113, the chosen one a pill in the accent
//! colour, middle column to x 647 with a #E0E0E0 rule, page title 26 px text.
use crate::editor_window::{inside, EditorWindow};
use crate::reader_icons::{self, Ink};
use crate::reader_window::rounded;
use crate::{Input, Key, Painter, TextStyle};
use bo_render::Rgba;

const COLUMN: Rgba = Rgba(0xEA, 0xEA, 0xEA, 255);
const TEXT: Rgba = Rgba(0x22, 0x22, 0x22, 255);
const MUTED: Rgba = Rgba(0x70, 0x70, 0x70, 255);
const RULE: Rgba = Rgba(0xE0, 0xE0, 0xE0, 255);
const CHOSEN: Rgba = Rgba(0xD9, 0xD9, 0xD9, 255);
const WHITE: Rgba = Rgba(255, 255, 255, 255);

/// Entries of the left column: command, label key; `None` is a rule. The reference's order
/// (round 7: Action Wizard, Batch Print, Index, Share and Skins added).
const ITEMS: [Option<(&str, &str)>; 17] = [
    Some(("bs.properties", "pdfEditor.bsProperties")),
    Some(("bs.create", "pdfEditor.bsCreate")),
    Some(("bs.open", "pdfReader.open")),
    Some(("file.save", "pdfReader.save")),
    Some(("file.saveAs", "pdfReader.saveAs")),
    Some(("bs.optimizer", "pdfEditor.bsOptimizer")),
    Some(("bs.actionWizard", "pdfEditor7.actionWizard")),
    Some(("bs.export", "pdfEditor.bsExport")),
    Some(("file.print", "pdfReader.print")),
    Some(("bs.batch", "pdfBatch.menu")),
    Some(("bs.index", "pdfEditor7.index")),
    Some(("bs.share", "pdfShare.title")),
    Some(("bs.revert", "pdfEditor.bsRevert")),
    Some(("file.close", "pdfReader.close")),
    None,
    Some(("bs.preferences", "pdfEditor.bsPreferences")),
    Some(("bs.skins", "pdfSkins.title")),
];

/// Entries shown grey with a tooltip saying why (command, tooltip key).
pub(crate) const BS_NOT_OFFERED: [(&str, &str); 2] = [
    ("bs.actionWizard", "pdfEditor7.tipActionWizard"),
    ("bs.index", "pdfEditor7.tipIndex"),
];

/// The backstage: shown, chosen page, hit boxes of the last paint.
#[derive(Debug, Default)]
pub(crate) struct Backstage {
    pub open: bool,
    pub page: &'static str,
    pub hits: Vec<([f32; 4], String)>,
    /// Grey entries with a tooltip: box and tooltip key.
    pub tips: Vec<([f32; 4], &'static str)>,
    /// Round 10: the chosen row of the PDF Optimizer page (Reduce File Size, Advanced
    /// Optimization, Optimize Scanned PDF).
    pub sub: usize,
}

impl EditorWindow {
    /// Open the File page (on Open when there is a document, else Create).
    pub(crate) fn open_backstage(&mut self) {
        self.bs.open = true;
        self.bs.page = "bs.open";
        self.r.backstage_open = true;
        self.r.close_menu();
    }

    pub(crate) fn close_backstage(&mut self) {
        self.bs.open = false;
        self.r.backstage_open = false;
    }

    fn bs_style(&self, px125: f32) -> TextStyle {
        TextStyle {
            size: px125 / 1.375 * self.ui_scale(),
            bold: false,
        }
    }

    /// Input while the File page is open; true when used (everything below the tab row).
    pub(crate) fn backstage_input(&mut self, input: &Input) -> bool {
        if !self.bs.open {
            return false;
        }
        let top = self.r.tabs_bottom();
        match input {
            Input::Press { x, y, .. } => {
                if *y < top {
                    // Another ribbon tab closes the page; title bar and File pass through.
                    let boxes = self.r.tab_boxes().to_vec();
                    if boxes.iter().skip(1).any(|r| inside(*r, *x, *y)) {
                        self.close_backstage();
                    }
                    return false;
                }
                let hit = self
                    .bs
                    .hits
                    .iter()
                    .find(|(r, _)| inside(*r, *x, *y))
                    .map(|h| h.1.clone());
                if let Some(cmd) = hit {
                    self.backstage_command(&cmd);
                } else {
                    // Round 12: a click elsewhere closes an open drop-down.
                    self.bsp12.open = None;
                }
                true
            }
            Input::Key(Key::Escape, _) => {
                self.close_backstage();
                true
            }
            Input::Move { y, .. } | Input::Drag { y, .. } | Input::Release { y, .. } => *y >= top,
            Input::Wheel(_) | Input::Text(_) | Input::Key(..) => true,
            _ => false,
        }
    }

    pub(crate) fn backstage_command(&mut self, cmd: &str) {
        if self.props12_command(cmd) {
            return;
        }
        match cmd {
            "bs.optimizer" => {
                self.bs.page = "bs.optimizer";
                self.bs.sub = 0;
            }
            c if c.starts_with("bs.opt.row") => {
                self.bs.sub = c[10..].parse().unwrap_or(0);
            }
            "bs.opt.current" => {
                self.close_backstage();
                match self.bs.sub {
                    0 => self.open_reduce10(),
                    _ => self.open_opt10(),
                }
            }
            "bs.properties" | "bs.create" | "bs.open" | "bs.export" | "bs.share" | "bs.skins" => {
                self.bs.page = match cmd {
                    "bs.properties" => "bs.properties",
                    "bs.create" => "bs.create",
                    "bs.export" => "bs.export",
                    "bs.share" => "bs.share",
                    "bs.skins" => "bs.skins",
                    _ => "bs.open",
                };
            }
            c if c.starts_with("bs.share") => {
                self.r.backstage_command8(c);
            }
            c if c.starts_with("bs.skin.") => {
                if let Some(k) = c.strip_prefix("bs.skin.").and_then(|v| v.parse().ok()) {
                    self.r.use_product_skin(k);
                }
            }
            "bs.batch" => {
                self.close_backstage();
                self.open_batch_print();
            }
            "bs.back" => self.close_backstage(),
            "bs.revert" => {
                self.close_backstage();
                self.revert();
            }
            "bs.preferences" => {
                self.close_backstage();
                self.ask_preferences();
            }
            _ => {
                if let Some(path) = cmd.strip_prefix("bs.recent.") {
                    self.close_backstage();
                    self.push(crate::ReaderRequest::Open(Some(std::path::PathBuf::from(
                        path,
                    ))));
                    return;
                }
                // Everything else is a command of the window.
                self.close_backstage();
                self.push_command(cmd);
            }
        }
    }

    /// Paint the File page over the ribbon and the document.
    #[allow(clippy::too_many_lines)]
    pub(crate) fn paint_backstage(&mut self, p: &mut dyn Painter) {
        if !self.bs.open {
            return;
        }
        let sc = self.ui_scale();
        let px = move |v: f32| (v * sc).round();
        let (w, h) = self.window_size();
        let top = self.r.tabs_bottom() + 1.0;
        let mut hits = Vec::new();
        p.fill_rect(0.0, top, w, h - top, WHITE);
        // Left column: 272 px from the window's edge at 125 % like the reference (round 8; its
        // column starts after a 9 px frame strip, items and pill keep their place from it).
        let cw = px(217.6);
        p.fill_rect(0.0, top, cw, h - top, COLUMN);
        let item = self.bs_style(17.0);
        let doc = self.showing_doc() || self.view().is_some();
        let modified = self.view().is_some_and(|v| v.doc().modified());
        let accent = self.accent();
        let mut y = top + px(11.2);
        let mut tips = Vec::new();
        for it in ITEMS {
            let Some((cmd, key)) = it else {
                // The reference (125 %): rule 26 px below Close's label centre, Preferences 54.
                y -= px(1.2);
                p.fill_rect(px(27.2), y, cw - px(47.2), 1.0, Rgba(0xD4, 0xD4, 0xD4, 255));
                y += px(6.8);
                continue;
            };
            let tip = BS_NOT_OFFERED.iter().find(|t| t.0 == cmd).map(|t| t.1);
            // Save and Revert are grey until the document changes, as in the reference.
            let enabled = tip.is_none()
                && ((doc && (!matches!(cmd, "file.save" | "bs.revert") || modified))
                    || matches!(cmd, "bs.create" | "bs.open" | "bs.preferences" | "bs.skins"));
            let r = [px(17.6), y, cw - px(29.6), px(31.2)];
            let chosen = cmd == self.bs.page;
            if chosen {
                p.path(
                    &rounded(r[0], r[1], r[2], r[3], r[3] / 2.0),
                    Some(accent),
                    None,
                );
            }
            let col = if chosen {
                WHITE
            } else if enabled {
                TEXT
            } else {
                Rgba(0xC1, 0xC1, 0xC1, 255)
            };
            let ty = r[1] + ((r[3] - item.size * 1.33) / 2.0).round();
            let label = self.t(key);
            p.text(label.trim_end_matches(['.', '…']), px(32.8), ty, item, col);
            if enabled {
                hits.push((r, cmd.to_owned()));
            }
            if let Some(t) = tip {
                tips.push((r, t));
            }
            y += px(37.6);
        }
        // Page title and contents.
        let title = self.bs_style(26.0);
        let head = self.bs_style(21.0);
        let small = self.bs_style(14.0);
        let mx = cw + px(24.0);
        let ty = top + px(14.8);
        let ink = Ink {
            ink: Rgba(0x40, 0x40, 0x40, 255),
            accent,
        };
        match self.bs.page {
            "bs.open" | "bs.create" | "bs.export" => {
                let (title_key, rows): (&str, Vec<(&str, &str, &str)>) = match self.bs.page {
                    "bs.create" => (
                        "pdfEditor.bsCreate",
                        vec![
                            ("conv.blank", "blankPage", "pdfEditor.blank"),
                            ("conv.create", "createPdf", "pdfEditor.fromFiles"),
                            ("file.merge", "combine", "pdfEditor.combine"),
                        ],
                    ),
                    "bs.export" => (
                        "pdfEditor.bsExport",
                        vec![
                            ("conv.word", "toWord", "pdfEditor.toWord"),
                            ("conv.excel", "toExcel", "pdfEditor.toExcel"),
                            ("conv.slides", "toSlides", "pdfEditor.toSlides"),
                            ("conv.image", "toImage", "pdfEditor.toImage"),
                            ("conv.text", "file", "pdfEditor.toText"),
                        ],
                    ),
                    _ => (
                        "pdfReader.open",
                        vec![
                            ("bs.open", "clock", "pdfEditor.bsRecent"),
                            ("file.open", "computer", "pdfEditor.bsComputer"),
                        ],
                    ),
                };
                let head_text = self.t(title_key);
                let head_text = head_text.trim_end_matches(['.', '…']);
                p.text(head_text, mx - px(0.8), ty, title, MUTED);
                let mw = px(300.0);
                p.fill_rect(px(517.6), top, 1.0, h - top, RULE);
                let mut ry = top + px(85.6);
                for (k, (cmd, icon, key)) in rows.iter().enumerate() {
                    let r = [mx - px(9.6), ry, mw - px(30.4), px(34.4)];
                    if self.bs.page == "bs.open" && k == 0 {
                        p.path(
                            &rounded(r[0], r[1], r[2], r[3], r[3] / 2.0),
                            Some(CHOSEN),
                            None,
                        );
                    }
                    let is = px(20.8);
                    reader_icons::draw(p, icon, r[0] + px(16.0), r[1] + (r[3] - is) / 2.0, is, ink);
                    let col = if self.bs.page == "bs.open" && k == 0 {
                        Rgba(0xA8, 0x4A, 0x05, 255)
                    } else {
                        TEXT
                    };
                    let lt = r[1] + ((r[3] - item.size * 1.33) / 2.0).round();
                    p.text(&self.t(key), r[0] + px(43.2), lt, item, col);
                    hits.push((r, (*cmd).to_owned()));
                    ry += px(45.6);
                }
                if self.bs.page == "bs.open" {
                    // Recent documents.
                    let rx = px(547.2);
                    p.text(&self.t("pdfEditor.bsRecent"), rx, ty + px(4.0), head, TEXT);
                    p.text(
                        &self.t("pdfEditor.bsLocal"),
                        rx + px(1.6),
                        top + px(64.0),
                        item,
                        TEXT,
                    );
                    let mut y = top + px(84.0);
                    let rw = (w - rx - px(100.0)).min(px(578.0));
                    let mut recent: Vec<std::path::PathBuf> = Vec::new();
                    for r in self.recent() {
                        if !recent.contains(r) {
                            recent.push(r.clone());
                        }
                    }
                    if recent.is_empty() {
                        p.text(
                            &self.t("pdfEditor.bsNoRecent"),
                            rx + px(1.6),
                            y + px(8.0),
                            item,
                            MUTED,
                        );
                    }
                    for path in recent.iter().take(12) {
                        if y + px(54.4) > h {
                            break;
                        }
                        let r = [rx, y, rw, px(54.4)];
                        let is = px(24.0);
                        reader_icons::draw(p, "file", rx + px(8.0), y + (r[3] - is) / 2.0, is, ink);
                        let name = path
                            .file_name()
                            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
                        p.push_clip(rx, y, rw, r[3]);
                        p.text(&name, rx + px(41.6), y + px(8.0), item, TEXT);
                        p.text(
                            &path.to_string_lossy(),
                            rx + px(41.6),
                            y + px(30.4),
                            small,
                            MUTED,
                        );
                        p.pop_clip();
                        p.fill_rect(rx, y + r[3] - 1.0, rw, 1.0, RULE);
                        hits.push((r, format!("bs.recent.{}", path.to_string_lossy())));
                        y += r[3];
                    }
                }
            }
            "bs.optimizer" => {
                // Round 10, measured on the reference (125 %): rows as the Open page's, the
                // chosen one a #D9D9D9 pill with accent text; at the right a #666666 title, a
                // #999999 description and 142 × 128 px tiles framed #E0E0E0 with the icon
                // (#333333) over a #707070 label.
                let head_text = self.t("pdfEditor.bsOptimizer");
                p.text(
                    head_text.trim_end_matches(['.', '…']),
                    mx - px(0.8),
                    ty,
                    title,
                    MUTED,
                );
                p.fill_rect(px(517.6), top, 1.0, h - top, RULE);
                let rows = [
                    ("file", "pdfEditor10.reduceTitle"),
                    ("properties", "pdfEditor10.advanced"),
                    ("pages", "pdfEditor10.scanned"),
                ];
                let mut ry = top + px(85.6);
                let mw = px(300.0);
                for (k, (icon, key)) in rows.iter().enumerate() {
                    let r = [mx - px(9.6), ry, mw - px(30.4), px(34.4)];
                    let chosen = k == self.bs.sub;
                    if chosen {
                        p.path(
                            &rounded(r[0], r[1], r[2], r[3], r[3] / 2.0),
                            Some(CHOSEN),
                            None,
                        );
                    }
                    let is = px(20.8);
                    let ink2 = Ink {
                        ink: Rgba(0x33, 0x33, 0x33, 255),
                        accent,
                    };
                    reader_icons::draw(
                        p,
                        icon,
                        r[0] + px(16.0),
                        r[1] + (r[3] - is) / 2.0,
                        is,
                        ink2,
                    );
                    let lt = r[1] + ((r[3] - item.size * 1.33) / 2.0).round();
                    p.text(
                        &self.t(key),
                        r[0] + px(43.2),
                        lt,
                        item,
                        if chosen { accent } else { TEXT },
                    );
                    hits.push((r, format!("bs.opt.row{k}")));
                    ry += px(45.6);
                }
                let rx = px(547.2);
                let grey = Rgba(0x99, 0x99, 0x99, 255);
                let (title_key, desc_key) = match self.bs.sub {
                    0 => ("pdfEditor10.reduceTitle", "pdfEditor10.reduceDesc"),
                    1 => ("pdfEditor10.advanced", "pdfEditor10.advancedDesc"),
                    _ => ("pdfEditor10.scanned", "pdfEditor10.scannedDesc"),
                };
                p.text(
                    &self.t(title_key),
                    rx,
                    ty + px(4.0),
                    head,
                    Rgba(0x66, 0x66, 0x66, 255),
                );
                p.text(&self.t(desc_key), rx, top + px(66.4), item, grey);
                let tile = |p: &mut dyn Painter, y: f32, icon: &str, lines: &[String], on: bool| {
                    let r = [rx, y, px(113.6), px(102.4)];
                    p.fill_rect(r[0], r[1], r[2], r[3], Rgba(0xE0, 0xE0, 0xE0, 255));
                    p.fill_rect(r[0] + 1.0, r[1] + 1.0, r[2] - 2.0, r[3] - 2.0, WHITE);
                    let is = px(20.8);
                    let ink3 = Ink {
                        ink: if on {
                            Rgba(0x33, 0x33, 0x33, 255)
                        } else {
                            Rgba(0xC1, 0xC1, 0xC1, 255)
                        },
                        accent,
                    };
                    let n = lines.len() as f32;
                    let icon_y = r[1] + (r[3] - is - px(18.4) * n - px(6.0)) / 2.0;
                    reader_icons::draw(p, icon, r[0] + (r[2] - is) / 2.0, icon_y, is, ink3);
                    for (j, l) in lines.iter().enumerate() {
                        let lw = l.chars().count() as f32 * item.size * 0.5;
                        p.text(
                            l,
                            (r[0] + (r[2] - lw) / 2.0).round(),
                            icon_y + is + px(6.0) + px(18.4) * j as f32,
                            item,
                            if on {
                                Rgba(0x70, 0x70, 0x70, 255)
                            } else {
                                Rgba(0xC1, 0xC1, 0xC1, 255)
                            },
                        );
                    }
                    r
                };
                if self.bs.sub == 0 {
                    p.text(
                        &self.t("pdfEditor10.applyCurrent"),
                        rx,
                        top + px(108.0),
                        item,
                        grey,
                    );
                    let r = tile(
                        p,
                        top + px(129.6),
                        "file",
                        &[self.t("pdfEditor10.currentFile")],
                        true,
                    );
                    hits.push((r, "bs.opt.current".to_owned()));
                    p.text(
                        &self.t("pdfEditor10.applyMultiple"),
                        rx,
                        top + px(272.0),
                        item,
                        grey,
                    );
                    // Several files at once is not offered: the tile is grey.
                    tile(
                        p,
                        top + px(293.6),
                        "combine",
                        &[self.t("pdfEditor10.multipleFiles")],
                        false,
                    );
                } else {
                    let key = if self.bs.sub == 1 {
                        "pdfEditor10.advanced"
                    } else {
                        "pdfEditor10.scanned"
                    };
                    let words: Vec<String> = self.t(key).split(' ').map(str::to_owned).collect();
                    let half = words.len().div_ceil(2);
                    let lines = vec![words[..half].join(" "), words[half..].join(" ")];
                    let icon = if self.bs.sub == 1 {
                        "properties"
                    } else {
                        "pages"
                    };
                    let r = tile(p, top + px(87.2), icon, &lines, true);
                    hits.push((r, "bs.opt.current".to_owned()));
                }
            }
            "bs.share" | "bs.skins" => {
                // BlueReader's Share and Skins pages (Skins over the editor's own look).
                let page = self.bs.page;
                self.r.paint_backstage8(p, page, mx, top, &mut hits);
            }
            _ => {
                // Properties of the document (round 12: the reference's five pages).
                self.paint_props12(p, mx, top, &mut hits);
            }
        }
        self.bs.hits = hits;
        self.paint_bs_tip(p, &tips);
        self.bs.tips = tips;
    }

    /// Tooltip of a grey entry under the pointer (as the ribbon's grey commands).
    fn paint_bs_tip(&self, p: &mut dyn Painter, tips: &[([f32; 4], &'static str)]) {
        let (x, y) = self.pointer();
        let Some((r, key)) = tips.iter().find(|(r, _)| inside(*r, x, y)) else {
            return;
        };
        let s = self.ui_scale();
        let text = self.t(key);
        let style = TextStyle {
            size: 12.0 * s,
            bold: false,
        };
        let w = text.chars().count() as f32 * style.size * 0.5 + 16.0 * s;
        let h = 24.0 * s;
        let tx = (r[0] + r[2] + 4.0 * s).round();
        let ty = (r[1] + (r[3] - h) / 2.0).round();
        p.fill_rect(tx, ty, w, h, Rgba(118, 118, 118, 255));
        p.fill_rect(tx + 1.0, ty + 1.0, w - 2.0, h - 2.0, WHITE);
        p.text(
            &text,
            tx + 8.0 * s,
            ty + ((h - style.size * 1.33) / 2.0).round(),
            style,
            Rgba(51, 51, 51, 255),
        );
    }

    /// Revert: drop every change since the document was opened or saved.
    pub(crate) fn revert(&mut self) {
        if let Some(v) = self.r.view_mut() {
            self.edit.finish_typing(Some(v));
        }
        match self.doc_edit(bo_pdfread::PdfDocument::revert) {
            Ok(()) => self.after_doc_edit(),
            Err(e) => self.set_status(&e),
        }
    }

    /// The Preferences dialog: comment author, zoom and page display of opened documents,
    /// units of measurement.
    pub(crate) fn ask_preferences(&mut self) {
        let p = &self.prefs;
        let fields = vec![
            (self.t("pdfEditor.prefAuthor"), p.author.clone()),
            (self.t("pdfEditor.prefZoom"), p.zoom.clone()),
            (self.t("pdfEditor.prefLayout"), p.layout.clone()),
            (self.t("pdfEditor.prefUnits"), p.units.clone()),
        ];
        let mut d = crate::dialog::Dialog::new(
            crate::dialog::DialogKind::Comment,
            self.t("pdfEditor.bsPreferences"),
            fields,
            vec![
                (self.t("pdfReader.ok"), crate::dialog::DialogButton::Ok),
                (
                    self.t("pdfReader.cancel"),
                    crate::dialog::DialogButton::Cancel,
                ),
            ],
            Vec::new(),
        );
        d.layout(
            &crate::editor_window::Estimate,
            self.window_size(),
            self.ui_scale(),
        );
        self.dialog2 = Some((
            d,
            crate::editor_actions::Ask2::Tools(crate::editor_tools::ToolAsk::Preferences),
        ));
    }

    /// Preferences typed in the dialog: kept and applied to the open document.
    pub(crate) fn set_preferences(&mut self, v: &[String]) {
        let get = |i: usize| v.get(i).map(|s| s.trim().to_owned()).unwrap_or_default();
        let author = get(0);
        if !author.is_empty() {
            self.prefs.author = author;
        }
        self.prefs.zoom = get(1);
        self.prefs.layout = get(2);
        let units = get(3).to_lowercase();
        self.prefs.units = if matches!(units.as_str(), "mm" | "cm" | "in" | "pt") {
            units
        } else {
            "mm".into()
        };
        self.apply_view_prefs();
    }

    /// Zoom and page display of the preferences on the active document.
    pub(crate) fn apply_view_prefs(&mut self) {
        let zoom = self.prefs.zoom.to_lowercase();
        let layout = self.prefs.layout.to_lowercase();
        let Some(v) = self.r.view_mut() else { return };
        if zoom.contains("width") || zoom.contains("rộng") {
            v.set_fit(crate::Fit::Width);
        } else if zoom.contains("page") || zoom.contains("trang") {
            v.set_fit(crate::Fit::Page);
        } else if let Ok(z) = zoom.trim_end_matches('%').trim().parse::<f32>() {
            v.set_zoom((z / 100.0).clamp(0.1, 64.0));
        }
        let l = match layout.as_str() {
            s if s.starts_with("single") || s.starts_with("một") => Some(crate::Layout::Single),
            s if s.starts_with("facing") || s.starts_with("hai") => Some(crate::Layout::Facing),
            s if s.starts_with("cont") || s.starts_with("liên") => Some(crate::Layout::Continuous),
            _ => None,
        };
        if let Some(l) = l {
            v.set_layout(l);
        }
    }
}
