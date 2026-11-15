//! BlueEditor's File page (backstage) as the reference editor shows it: a grey column of
//! commands at the left (Properties, Create, Open, Save, Save As, PDF Optimizer, Export,
//! Print, Close), a white page for the chosen one (Open: recent documents and Computer;
//! Create; Export; Properties of the document). Measured at 125 % with 110 % text: column
//! 263 px wide, #EAEAEA, items 47 px apart from y 113, the chosen one a pill in the accent
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

/// Entries of the left column: command, label key; `None` is a rule.
const ITEMS: [Option<(&str, &str)>; 11] = [
    Some(("bs.properties", "pdfEditor.bsProperties")),
    Some(("bs.create", "pdfEditor.bsCreate")),
    Some(("bs.open", "pdfReader.open")),
    Some(("file.save", "pdfReader.save")),
    Some(("file.saveAs", "pdfReader.saveAs")),
    Some(("file.optimize", "pdfEditor.bsOptimizer")),
    Some(("bs.export", "pdfEditor.bsExport")),
    Some(("file.print", "pdfReader.print")),
    Some(("file.close", "pdfReader.close")),
    None,
    Some(("bs.back", "pdfEditor.bsBack")),
];

/// The backstage: shown, chosen page, hit boxes of the last paint.
#[derive(Debug, Default)]
pub(crate) struct Backstage {
    pub open: bool,
    pub page: &'static str,
    pub hits: Vec<([f32; 4], String)>,
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

    fn backstage_command(&mut self, cmd: &str) {
        match cmd {
            "bs.properties" | "bs.create" | "bs.open" | "bs.export" => {
                self.bs.page = match cmd {
                    "bs.properties" => "bs.properties",
                    "bs.create" => "bs.create",
                    "bs.export" => "bs.export",
                    _ => "bs.open",
                };
            }
            "bs.back" => self.close_backstage(),
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
        // Left column.
        let cw = px(210.4);
        p.fill_rect(0.0, top, cw, h - top, COLUMN);
        let item = self.bs_style(17.0);
        let doc = self.showing_doc() || self.view().is_some();
        let modified = self.view().is_some_and(|v| v.doc().modified());
        let accent = self.accent();
        let mut y = top + px(11.2);
        for it in ITEMS {
            let Some((cmd, key)) = it else {
                y += px(4.0);
                p.fill_rect(px(20.0), y, cw - px(40.0), 1.0, Rgba(0xD4, 0xD4, 0xD4, 255));
                y += px(8.0);
                continue;
            };
            // Save is grey until the document changes, as in the reference.
            let enabled = (doc && (cmd != "file.save" || modified))
                || matches!(cmd, "bs.create" | "bs.open" | "bs.back");
            let r = [px(10.4), y, cw - px(22.4), px(31.2)];
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
            p.text(label.trim_end_matches(['.', '…']), px(25.6), ty, item, col);
            if enabled {
                hits.push((r, cmd.to_owned()));
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
            _ => {
                // Properties of the document.
                p.text(
                    &self.t("pdfEditor.bsProperties"),
                    mx - px(0.8),
                    ty,
                    title,
                    MUTED,
                );
                let rows = self.properties();
                let mut y = top + px(85.6);
                for (k, v) in rows {
                    p.text(&k, mx, y, item, MUTED);
                    p.text(&v, mx + px(160.0), y, item, TEXT);
                    y += px(30.4);
                }
            }
        }
        self.bs.hits = hits;
    }

    /// Label and value rows of the Properties page.
    pub(crate) fn properties(&mut self) -> Vec<(String, String)> {
        let path = self.path().map(std::path::Path::to_path_buf);
        let name = self.doc_stem();
        let label = |w: &Self, k: &str| w.t(k);
        let mut rows = Vec::new();
        let info = self.with_doc(|d| {
            let size = d.page_info(0).map(|p| (p.width, p.height));
            Ok((
                d.page_count(),
                size,
                d.version(),
                d.bytes().len(),
                d.title(),
                d.info("Author"),
                d.info("Producer"),
                d.is_encrypted(),
            ))
        });
        let Ok((pages, size, version, bytes, title, author, producer, enc)) = info else {
            return rows;
        };
        rows.push((label(self, "pdfEditor.propFile"), format!("{name}.pdf")));
        if let Some(dir) = path.as_ref().and_then(|p| p.parent()) {
            rows.push((
                label(self, "pdfEditor.propLocation"),
                dir.to_string_lossy().into_owned(),
            ));
        }
        rows.push((label(self, "pdfEditor.propPages"), pages.to_string()));
        if let Some((w, h)) = size {
            rows.push((
                label(self, "pdfEditor.propPageSize"),
                format!("{:.1} × {:.1} mm", w / 72.0 * 25.4, h / 72.0 * 25.4),
            ));
        }
        rows.push((label(self, "pdfEditor.propVersion"), version));
        rows.push((
            label(self, "pdfEditor.propFileSize"),
            format!("{:.1} KB ({bytes} B)", bytes as f32 / 1024.0),
        ));
        for (k, v) in [
            ("pdfEditor.propTitle", title),
            ("pdfEditor.propAuthor", author),
            ("pdfEditor.propProducer", producer),
        ] {
            rows.push((label(self, k), v.unwrap_or_else(|| "—".into())));
        }
        let sec = if enc {
            "pdfEditor.propEncrypted"
        } else {
            "pdfEditor.propNoSecurity"
        };
        rows.push((label(self, "pdfEditor.propSecurity"), label(self, sec)));
        rows
    }
}
