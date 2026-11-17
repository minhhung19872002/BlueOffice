//! BlueReader round 5 (a child of `reader_window`): the File page (backstage) like the reference
//! reader's and in the same form as BlueEditor's (`editor_backstage.rs`): a grey column of
//! commands at the left (Open, Properties, Save, Save As, Print | Revert, Close), the chosen
//! one a pill in the accent colour, and a white page for Open (Recent Documents of BlueReader
//! itself and Computer) or Properties (the document's description, with the full Document
//! Properties dialog a click away). Metrics as measured for the editor at 125 % with 110 %
//! text: column 263 px wide, items 47 px apart, middle column to x 647.
use super::{inside, rounded, ReaderRequest, ReaderWindow};
use crate::reader_icons::{self, Ink};
use crate::{Input, Key, Painter, TextStyle};
use bo_render::Rgba;

// Round 6, measured on the reference's left column at 125 %: column 245, 246, 250 with a
// 199, 204, 217 edge, rules 230, 233, 240, the chosen command a box with 4 px corners.
const COLUMN: Rgba = Rgba(245, 246, 250, 255);
const COLUMN_EDGE: Rgba = Rgba(199, 204, 217, 255);
const TEXT: Rgba = Rgba(0x22, 0x22, 0x22, 255);
const MUTED: Rgba = Rgba(0x70, 0x70, 0x70, 255);
const RULE: Rgba = Rgba(0xE0, 0xE0, 0xE0, 255);
const CHOSEN: Rgba = Rgba(230, 233, 240, 255);
const WHITE: Rgba = Rgba(255, 255, 255, 255);
const GREY: Rgba = Rgba(0xC1, 0xC1, 0xC1, 255);

/// Left column: command, label key; `None` is a rule. The reference reader's order; its Share
/// and Skins are ours (round 8, `reader_skins8_win.rs`).
const ITEMS: [Option<(&str, &str)>; 14] = [
    Some(("bs.open", "pdfReader.open")),
    None,
    Some(("bs.properties", "pdfProps.menu")),
    Some(("file.save", "pdfReader.save")),
    Some(("file.saveAs", "pdfReader.saveAs")),
    Some(("file.print", "pdfReader.print")),
    Some(("bs.batch", "pdfBatch.menu")),
    // Round 12, as the reference's column: Share opens the second block; Skins before
    // Preferences.
    None,
    Some(("bs.share", "pdfShare.title")),
    Some(("bs.revert", "pdfBackstage.revert")),
    Some(("file.close", "pdfReader.close")),
    None,
    Some(("bs.skins", "pdfSkins.title")),
    Some(("bs.prefs", "pdfPrefs.menu")),
];

/// The File page: chosen page and the hit boxes of the last paint.
#[derive(Debug, Default)]
pub(crate) struct Backstage {
    pub page: &'static str,
    pub hits: Vec<([f32; 4], String)>,
}

impl ReaderWindow {
    pub(crate) fn open_reader_backstage(&mut self) {
        self.tabs5.backstage = Some(Backstage {
            page: "bs.open",
            hits: Vec::new(),
        });
        self.backstage_open = true;
        self.menu = None;
    }

    fn close_reader_backstage(&mut self) {
        self.tabs5.backstage = None;
        self.backstage_open = false;
    }

    fn bs_style(&self, px125: f32) -> TextStyle {
        TextStyle {
            size: px125 / 1.375 * self.scale,
            bold: false,
        }
    }

    /// Input of the File page; `Some` when used.
    pub(crate) fn route_backstage(
        &mut self,
        input: &Input,
        m: &dyn crate::TextMeasure,
    ) -> Option<bool> {
        // File opens the page (BlueReader's own window; BlueEditor has its own page).
        if self.profile.is_none() {
            if let Input::Press { x, y, .. } = input {
                if self.tab_rects.first().is_some_and(|r| inside(*r, *x, *y)) {
                    if self.tabs5.backstage.is_some() {
                        self.close_reader_backstage();
                    } else {
                        self.open_reader_backstage();
                    }
                    return Some(true);
                }
            }
        }
        self.tabs5.backstage.as_ref()?;
        let top = self.tabs_bottom();
        match input {
            Input::Press { x, y, .. } => {
                if *y < top {
                    // Another ribbon tab closes the page; the title bar passes through.
                    if self.tab_rects.iter().skip(1).any(|r| inside(*r, *x, *y)) {
                        self.close_reader_backstage();
                    }
                    return None;
                }
                let hit = self.tabs5.backstage.as_ref().and_then(|b| {
                    b.hits
                        .iter()
                        .find(|(r, _)| inside(*r, *x, *y))
                        .map(|h| h.1.clone())
                });
                if let Some(cmd) = hit {
                    self.backstage_command(&cmd, m);
                }
                Some(true)
            }
            Input::Key(Key::Escape, _) => {
                self.close_reader_backstage();
                Some(true)
            }
            Input::Move { y, .. } | Input::Drag { y, .. } | Input::Release { y, .. } => {
                (*y >= top).then_some(true)
            }
            Input::Wheel(_) | Input::Text(_) | Input::Key(..) => Some(true),
            _ => None,
        }
    }

    fn backstage_command(&mut self, cmd: &str, m: &dyn crate::TextMeasure) {
        match cmd {
            "bs.open" | "bs.properties" | "bs.share" | "bs.skins" => {
                if let Some(b) = self.tabs5.backstage.as_mut() {
                    b.page = match cmd {
                        "bs.open" => "bs.open",
                        "bs.share" => "bs.share",
                        "bs.skins" => "bs.skins",
                        _ => "bs.properties",
                    };
                }
            }
            c if c.starts_with("bs.skin.") || c.starts_with("bs.share") => {
                self.backstage_command8(c);
            }
            "bs.revert" => {
                self.close_reader_backstage();
                if let Some(t) = self.tabs.get_mut(self.active) {
                    match t.view.doc_mut().revert() {
                        Ok(()) => self.document_changed(),
                        Err(e) => self.status = e,
                    }
                }
            }
            "bs.details" => {
                self.close_reader_backstage();
                self.open_properties();
            }
            _ => {
                self.close_reader_backstage();
                if let Some(path) = cmd.strip_prefix("bs.recent.") {
                    self.requests
                        .push(ReaderRequest::Open(Some(std::path::PathBuf::from(path))));
                    return;
                }
                self.command(cmd, m);
            }
        }
    }

    /// Paint the File page over the ribbon body and the document.
    #[allow(clippy::too_many_lines)]
    pub(crate) fn paint_reader_backstage(&mut self, p: &mut dyn Painter) {
        let Some(page) = self.tabs5.backstage.as_ref().map(|b| b.page) else {
            return;
        };
        let sc = self.scale;
        let px = move |v: f32| (v * sc).round();
        let (w, h) = self.size;
        let top = self.tabs_bottom() + 1.0;
        let mut hits = Vec::new();
        p.fill_rect(1.0, top, w - 2.0, h - top - 1.0, WHITE);
        let cw = px(210.4);
        p.fill_rect(1.0, top, cw - 1.0, h - top - 1.0, COLUMN);
        p.fill_rect(cw, top, 1.0, h - top - 1.0, COLUMN_EDGE);
        let item = self.bs_style(17.0);
        let doc = self.view().is_some();
        let modified = self.view().is_some_and(|v| v.doc().modified());
        let accent = self.theme.accent;
        // Rows 46 px apart from 14 px under the tabs, 38 px high; a rule adds 7 px.
        let mut y = top + px(11.2);
        for it in ITEMS {
            let Some((cmd, key)) = it else {
                p.fill_rect(px(20.0), y - 1.0, cw - px(36.0), 1.0, CHOSEN);
                y += px(5.6);
                continue;
            };
            // Save and Revert stay grey until the document changes, like the reference's.
            let enabled = cmd == "bs.open"
                || (doc && (!matches!(cmd, "file.save" | "bs.revert") || modified));
            let r = [px(10.4), y, cw - px(22.4), px(30.4)];
            let chosen = cmd == page;
            if chosen {
                p.path(
                    &rounded(r[0], r[1], r[2], r[3], px(3.2)),
                    Some(accent),
                    None,
                );
            }
            let col = if chosen {
                WHITE
            } else if enabled {
                TEXT
            } else {
                GREY
            };
            let ty = r[1] + ((r[3] - item.size * 1.33) / 2.0).round();
            let label = self.t(key);
            p.text(label.trim_end_matches(['.', '…']), px(25.6), ty, item, col);
            if enabled {
                hits.push((r, cmd.to_owned()));
            }
            y += px(36.8);
        }
        let title = self.bs_style(26.0);
        let head = self.bs_style(21.0);
        let small = self.bs_style(14.0);
        let mx = cw + px(24.0);
        let ty = top + px(14.8);
        let ink = Ink {
            ink: Rgba(0x40, 0x40, 0x40, 255),
            accent,
        };
        if page == "bs.open" {
            let head_text = self.t("pdfReader.open");
            p.text(
                head_text.trim_end_matches(['.', '…']),
                mx - px(0.8),
                ty,
                title,
                MUTED,
            );
            p.fill_rect(px(517.6), top, 1.0, h - top - 1.0, RULE);
            let rows = [
                ("bs.open", "clock", "pdfBackstage.recent"),
                ("file.open", "computer", "pdfBackstage.computer"),
            ];
            let mut ry = top + px(85.6);
            for (k, (cmd, icon, key)) in rows.iter().enumerate() {
                let r = [mx - px(9.6), ry, px(300.0) - px(30.4), px(33.6)];
                if k == 0 {
                    p.path(
                        &rounded(r[0], r[1], r[2], r[3], px(3.2)),
                        Some(CHOSEN),
                        None,
                    );
                }
                let is = px(20.8);
                reader_icons::draw(p, icon, r[0] + px(16.0), r[1] + (r[3] - is) / 2.0, is, ink);
                let col = if k == 0 { self.theme.accent_text } else { TEXT };
                let lt = r[1] + ((r[3] - item.size * 1.33) / 2.0).round();
                p.text(&self.t(key), r[0] + px(43.2), lt, item, col);
                hits.push((r, (*cmd).to_owned()));
                ry += px(45.6);
            }
            // Recent documents: BlueReader's own list.
            let rx = px(547.2);
            p.text(&self.t("pdfBackstage.recent"), rx, ty + px(4.0), head, TEXT);
            p.text(
                &self.t("pdfBackstage.local"),
                rx + px(1.6),
                top + px(64.0),
                item,
                TEXT,
            );
            let mut y = top + px(84.0);
            let rw = (w - rx - px(100.0)).min(px(578.0));
            let mut recent: Vec<std::path::PathBuf> = Vec::new();
            for r in &self.recent {
                if !recent.contains(r) {
                    recent.push(r.clone());
                }
            }
            if recent.is_empty() {
                p.text(
                    &self.t("pdfBackstage.noRecent"),
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
        } else if page == "bs.share" || page == "bs.skins" {
            self.paint_backstage8(p, page, mx, top, &mut hits);
        } else {
            p.text(&self.t("pdfProps.menu"), mx - px(0.8), ty, title, MUTED);
            let rows = self.description_rows();
            let mut y = top + px(85.6);
            for (k, v) in rows {
                p.text(&k, mx, y, item, MUTED);
                let n = v.chars().count();
                let v = if n > 70 {
                    format!("…{}", v.chars().skip(n - 69).collect::<String>())
                } else {
                    v
                };
                p.text(&v, mx + px(176.0), y, item, TEXT);
                y += px(30.4);
            }
            // The full dialog (Description, Security, Fonts, Advanced).
            let label = self.t("pdfBackstage.details");
            let b = [mx, y + px(12.0), px(220.0), px(30.4)];
            p.path(
                &rounded(b[0], b[1], b[2], b[3], px(3.2)),
                Some(CHOSEN),
                None,
            );
            let lt = b[1] + ((b[3] - item.size * 1.33) / 2.0).round();
            p.text(&label, b[0] + px(12.0), lt, item, TEXT);
            hits.push((b, "bs.details".to_owned()));
        }
        if let Some(b) = self.tabs5.backstage.as_mut() {
            b.hits = hits;
        }
    }
}
