//! BlueDoc parity round 24 (docs/ui/REPORT.md "Vòng 114"): what the round-23 inventory found
//! missing. Show Tabs Only / Always Show Ribbon (Ctrl+F1, a double click on a tab, the
//! ribbon's display options button at its bottom right; a click on a tab of the collapsed
//! ribbon shows its groups over the document until a command runs or the pointer presses
//! elsewhere); the highlighter pen (Text Highlight Color with no selection: drag over text to
//! highlight it, Esc or Stop Highlighting ends it); Diagonal Down / Up Border of table cells
//! (§17.4.80 tl2br, §17.4.79 tr2bl); context menu Search, Read Aloud and Translate. A child
//! module of the shell.
use super::{DocWindow, Request};
use crate::popup::{Choice, Menu, MenuItem};
use crate::ribbon::RibbonHit;
use crate::{Input, Key, Mods, Painter};
use bo_engine::edit::Command;
use bo_render::{PathSeg, Rgba};
use serde_json::json;

#[cfg(test)]
#[path = "doc_r24_tests.rs"]
mod tests;

/// Round-24 state of the window.
#[derive(Default)]
pub(super) struct R24 {
    /// The highlighter pen is on (Text Highlight Color with no selection).
    pub(super) pen: bool,
    /// A press in the document while the pen is on (its release highlights the drag).
    pen_drag: bool,
    /// A command ran from the peeking ribbon: it hides once no menu is open.
    peek_ran: bool,
    /// The tab pressed on the collapsed ribbon.
    tab_press: Option<usize>,
    /// The display options button is hovered.
    hover_display: bool,
    /// The building block whose properties the Create New Building Block dialog edits.
    editing: Option<String>,
}

/// Percent-encode `s` for a URL query (RFC 3986 unreserved characters kept).
fn query(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

impl DocWindow {
    fn r24_t(&self, key: &str, en: &str) -> String {
        self.t(key, en)
    }

    /// Extra height the peeking ribbon covers over the document (0 unless peeking).
    pub(super) fn r24_peek_h(&self) -> f32 {
        if self.ribbon.peek() && self.read_mode.is_none() && !self.r4.focus {
            (self.ribbon.full_height() - self.ribbon.dock_height()).max(0.0)
        } else {
            0.0
        }
    }

    /// Whether numeric dates read day first (the interface's short date order).
    pub(super) fn locale_day_first(&self) -> bool {
        self.strings.get("doc24.dateOrder") != "mdy"
    }

    /// The ribbon display options button (bottom right of the ribbon): [x, y, w, h].
    fn r24_display_rect(&self) -> Option<[f32; 4]> {
        if self.read_mode.is_some() || self.r4.focus || self.backstage.is_some() {
            return None;
        }
        let s = self.scale;
        let size = (22.0 * s).round();
        let bottom = if self.ribbon.collapsed() && !self.ribbon.peek() {
            return None;
        } else {
            self.ribbon.top() + self.ribbon.full_height() - self.ribbon.qat_strip_h()
        };
        Some([
            self.size.0 - size - (6.0 * s).round(),
            bottom - size - (4.0 * s).round(),
            size,
            size,
        ])
    }

    fn r24_in_display(&self, x: f32, y: f32) -> bool {
        self.r24_display_rect()
            .is_some_and(|r| x >= r[0] && x < r[0] + r[2] && y >= r[1] && y < r[1] + r[3])
    }

    /// Collapse the ribbon to its tabs, or show it all again.
    pub(super) fn r24_set_collapsed(&mut self, on: bool) -> bool {
        self.menu = None;
        self.ribbon.set_collapsed(on);
        self.r24.peek_ran = false;
        self.relayout_pending = true;
        true
    }

    /// The display options menu: Show Tabs Only, Always Show Ribbon (the reference's
    /// Ribbon Display Options button).
    fn r24_display_menu(&mut self) -> bool {
        let Some(r) = self.r24_display_rect() else {
            return false;
        };
        let collapsed = self.ribbon.collapsed();
        let items = vec![
            MenuItem {
                label: self.r24_t("doc24.showTabsOnly", "Show tabs only"),
                choice: Choice::Command("r24.ribbon:tabs".into()),
                checked: collapsed,
                dim: false,
            },
            MenuItem {
                label: self.r24_t("doc24.alwaysShowRibbon", "Always show Ribbon"),
                choice: Choice::Command("r24.ribbon:always".into()),
                checked: !collapsed,
                dim: false,
            },
        ];
        // Right-aligned under the button.
        let at = [r[0] + r[2] - 1.0, r[1] + r[3], 1.0, 0.0];
        self.menu = Some(Menu::open(items, at, self.size, self.scale));
        true
    }

    /// Round-24 commands; `None` passes `id` on to the earlier rounds.
    pub(super) fn r24_action(&mut self, id: &str) -> Option<bool> {
        if self.ribbon.peek() {
            self.r24.peek_ran = true;
        }
        let collapsed_sel = self
            .view
            .engine()
            .session()
            .is_some_and(|s| s.selection().is_collapsed());
        if let Some(hex) = id.strip_prefix("r23.hl:") {
            match hex {
                "stop" => {
                    self.menu = None;
                    self.r22.gallery = None;
                    let was = self.r24.pen;
                    self.r24.pen = false;
                    return Some(was);
                }
                "none" => {}
                // A colour picked with no selection turns the pen on with it.
                hex if collapsed_sel => {
                    self.r22.gallery = None;
                    self.r23.highlight = Some(hex.to_owned());
                    self.r24.pen = true;
                    return Some(true);
                }
                _ => {}
            }
            return None;
        }
        if let Some(v) = id.strip_prefix("r24.ribbon:") {
            return Some(match v {
                "tabs" => self.r24_set_collapsed(true),
                "always" => self.r24_set_collapsed(false),
                _ => self.r24_set_collapsed(!self.ribbon.collapsed()),
            });
        }
        if let Some(v) = id.strip_prefix("r24.diag:") {
            return Some(self.r24_diagonal(v == "down"));
        }
        if let Some(key) = id.strip_prefix("r24.bbDelete:") {
            let key = key.to_owned();
            return Some(self.r24_delete_block(&key));
        }
        if let Some(key) = id.strip_prefix("r24.bbEdit:") {
            let key = key.to_owned();
            return Some(self.r24_edit_block(&key));
        }
        if let Some(key) = id.strip_prefix("r24.bbInsert:") {
            self.menu = None;
            let v = json!({ "cmd": "insertBuildingBlock", "key": key });
            return Some(self.view.run_json(&v).is_some());
        }
        // Edit Properties… OK: the block's new properties (not a new block).
        if let Some(spec) = id.strip_prefix("blocks12.save:") {
            if let Some(key) = self.r24.editing.take() {
                let Ok(spec) = serde_json::from_str::<serde_json::Value>(spec) else {
                    return Some(false);
                };
                let out = self.view.run_json(&json!({
                    "cmd": "editBuildingBlock", "key": key, "spec": spec,
                }));
                if let Some(file) = out.as_ref().and_then(|v| v["blocks"]["store"].as_str()) {
                    self.requests.push(Request::SaveBlockStore(file.to_owned()));
                }
                return Some(self.r24_open(crate::doc_forms::DocFormKind::Organizer24));
            }
            return None;
        }
        Some(match id {
            // The button with no selection: the highlighter pen (on / off).
            "format.highlight" if collapsed_sel && !self.r24_on_highlight_arrow() => {
                self.menu = None;
                self.r24.pen = !self.r24.pen;
                true
            }
            "r24.search" | "r24.translate" => {
                self.menu = None;
                let text = self.r24_lookup_text();
                if text.is_empty() {
                    return Some(false);
                }
                let url = if id == "r24.search" {
                    format!("https://www.bing.com/search?q={}", query(&text))
                } else {
                    format!("https://www.bing.com/translator?text={}", query(&text))
                };
                self.requests.push(Request::OpenUrl(url));
                true
            }
            "r24.readAloud" => {
                self.menu = None;
                self.action("review.readAloud")
            }
            // Equation › Equation: the equation gallery (not the placeholder list that
            // marks the button with ▾).
            "eq.equation" => {
                let Some(r) = self.ribbon.rect_of(RibbonHit::Command("eq.equation")) else {
                    return None;
                };
                self.r4_open_gallery_at("insert.equation", r)
            }
            "r24.dropCapOptions" => self.r24_open(crate::doc_forms::DocFormKind::DropCap24),
            "r24.hyphOptions" => self.r24_open(crate::doc_forms::DocFormKind::Hyphenation24),
            "r24.hyphManual" | "r24.manualHyph" => {
                self.r24_open(crate::doc_forms::DocFormKind::ManualHyph24)
            }
            "r24.organizer" => self.r24_open(crate::doc_forms::DocFormKind::Organizer24),
            "r24.autoText" => self.r24_autotext_menu(),
            "r24.saveQuickPart" | "r24.saveAutoText" => {
                self.menu = None;
                self.r3_open_form(crate::doc_forms::DocFormKind::NewBlock);
                // Gallery: Quick Parts / AutoText (NEW_BLOCK_GALLERIES order).
                let g = if id == "r24.saveAutoText" { 2 } else { 1 };
                if let Some(f) = self.r3.form.as_mut() {
                    f.set_sel("gallery", Some(g));
                }
                true
            }
            _ => return None,
        })
    }

    /// The press that ran Text Highlight Color was on its drop-down part (not taken: round
    /// 23 reads it after).
    fn r24_on_highlight_arrow(&self) -> bool {
        let Some((x, y)) = self.r23.press else {
            return false;
        };
        let Some(r) = self.ribbon.rect_of(RibbonHit::Command("format.highlight")) else {
            return false;
        };
        x >= r[0] + r[2] * 0.6 && x < r[0] + r[2] && y >= r[1] && y < r[1] + r[3]
    }

    /// The text Search / Translate look up: the selection (one line, at most 255
    /// characters) or the word at the caret.
    fn r24_lookup_text(&self) -> String {
        let Some(s) = self.view.engine().session() else {
            return String::new();
        };
        let mut text = s.selected_text();
        if text.trim().is_empty() {
            let at = s.selection().focus;
            let para = s.para_text(at.para).unwrap_or_default();
            let i = at.offset.min(para.len());
            let is_word = |c: char| c.is_alphanumeric() || c == '\'' || c == '-';
            let start = para[..i]
                .char_indices()
                .rev()
                .find(|(_, c)| !is_word(*c))
                .map_or(0, |(j, c)| j + c.len_utf8());
            let end = para[i..]
                .char_indices()
                .find(|(_, c)| !is_word(*c))
                .map_or(para.len(), |(j, _)| i + j);
            text = para[start..end].to_owned();
        }
        let one: String = text
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .chars()
            .take(255)
            .collect();
        one
    }

    /// A message box with a Close button (the reference's message boxes).
    fn r24_message(&mut self, key: &str, en: &str, text: String) -> bool {
        let title = self.r24_t(key, en);
        let close = self.r24_t("common.close", "Close");
        self.menu = None;
        self.dialog = Some(crate::dialog::Dialog::new(
            crate::dialog::DialogKind::WordCount,
            title,
            Vec::new(),
            vec![(close, crate::dialog::DialogButton::Cancel)],
            vec![text],
        ));
        self.relayout_pending = true;
        true
    }

    /// Every building block (document, user, library) as `buildingBlocks` lists them.
    fn r24_blocks(&mut self) -> Vec<serde_json::Value> {
        self.view
            .run_json(&json!({ "cmd": "buildingBlocks" }))
            .and_then(|v| v["blocks"]["list"].as_array().cloned())
            .unwrap_or_default()
    }

    /// Open a round-24 dialog with its state in `extra["r24"]`.
    fn r24_open(&mut self, kind: crate::doc_forms::DocFormKind) -> bool {
        use crate::doc_forms::DocFormKind as K;
        self.menu = None;
        self.r22.gallery = None;
        let mut c = self.form_context();
        c.extra["r24"] = match kind {
            K::DropCap24 => {
                let frame = self.view.engine().session().and_then(|s| {
                    // The caret's paragraph or the drop cap letter's paragraph before it.
                    let at = s.selection().focus.para;
                    [Some(at), at.checked_sub(1)]
                        .into_iter()
                        .flatten()
                        .find_map(|i| {
                            bo_model::index::para_by_id(s.doc(), i).and_then(|p| p.props.frame)
                        })
                });
                c.fonts = self.view.font_list().into_iter().map(|f| f.name).collect();
                match frame {
                    Some(f) => json!({
                        "position": f.drop_cap.as_str(), "lines": f.lines,
                        "distance": f.h_space.0,
                    }),
                    None => json!({ "position": "none" }),
                }
            }
            K::Hyphenation24 => {
                let h = self
                    .view
                    .engine()
                    .session()
                    .map(|s| s.doc().hyphenation.clone())
                    .unwrap_or_default();
                json!({
                    "auto": h.auto, "noCaps": h.no_caps,
                    "zone": h.zone.map_or(360, |z| z.0),
                    "limit": h.consecutive_limit.unwrap_or(0),
                })
            }
            K::ManualHyph24 => {
                let words = self.r24_hyphen_candidates();
                if words.is_empty() {
                    // The reference says so in a message box.
                    let text = self.r24_t(
                        "doc24.hyphDone",
                        "Hyphenation is complete: no line-start word can be hyphenated.",
                    );
                    return self.r24_message("doc24.hyphTitle", "Hyphenation", text);
                }
                json!({ "words": words })
            }
            K::Organizer24 => json!({ "blocks": self.r24_blocks() }),
            _ => json!({}),
        };
        let f = crate::doc_forms::open(kind, &self.strings, &c);
        self.r3.form = Some(f);
        self.relayout_pending = true;
        true
    }

    /// Manual hyphenation candidates: the first word of each line that continues a
    /// paragraph, when it can be hyphenated (BlueOffice's English rules, ADR-0011), as
    /// "para|offset|word|points".
    fn r24_hyphen_candidates(&self) -> Vec<String> {
        let Some(layout) = self.view.engine().layout() else {
            return Vec::new();
        };
        let Some(s) = self.view.engine().session() else {
            return Vec::new();
        };
        let mut out = Vec::new();
        let mut last: Option<u32> = None;
        for page in &layout.pages {
            for line in &page.lines {
                let bo_layout::LineSource::Body(p) = line.source else {
                    last = None;
                    continue;
                };
                let continues = last == Some(p);
                last = Some(p);
                if !continues {
                    continue;
                }
                let Some((start, _)) = line.carets.get(0) else {
                    continue;
                };
                let text = s.para_text(p as usize).unwrap_or_default();
                let start = (start as usize).min(text.len());
                let word: String = text[start..]
                    .chars()
                    .take_while(|c| c.is_alphabetic())
                    .collect();
                let points = bo_layout::hyphenation_points(&word);
                if word.chars().count() < 5 || points.is_empty() {
                    continue;
                }
                let pts: Vec<String> = points.iter().map(ToString::to_string).collect();
                out.push(format!("{p}|{start}|{word}|{}", pts.join(",")));
            }
        }
        out
    }

    /// Quick Parts › AutoText ▸: the AutoText blocks, Save Selection to AutoText Gallery.
    fn r24_autotext_menu(&mut self) -> bool {
        let anchor = self
            .ribbon
            .rect_of(RibbonHit::Command("insert.quickParts"))
            .unwrap_or([self.size.0 / 3.0, 120.0, 40.0, 30.0]);
        let mut items: Vec<MenuItem> = self
            .r24_blocks()
            .iter()
            .filter(|b| b["gallery"] == "autoTxt")
            .map(|b| MenuItem {
                label: b["name"].as_str().unwrap_or("").to_owned(),
                choice: Choice::Command(format!(
                    "r24.bbInsert:{}",
                    b["key"].as_str().unwrap_or("")
                )),
                checked: false,
                dim: false,
            })
            .collect();
        let sep = items.len();
        items.push(MenuItem {
            label: self.r24_t("doc24.saveAutoText", "Save Selection to AutoText Gallery"),
            choice: Choice::Command("r24.saveAutoText".into()),
            checked: false,
            dim: !self.view.has_selection(),
        });
        let n = items.len();
        let seps = if sep > 0 { vec![sep] } else { Vec::new() };
        let at = [anchor[0], anchor[1] + anchor[3], anchor[2], 0.0];
        self.menu =
            Some(Menu::open(items, at, self.size, self.scale).with_decor(vec![None; n], seps));
        true
    }

    /// Building Blocks Organizer › Delete: the block goes (user blocks: the host stores the
    /// file again); the organizer shows the list again.
    fn r24_delete_block(&mut self, key: &str) -> bool {
        let out = self
            .view
            .run_json(&json!({ "cmd": "deleteBuildingBlock", "key": key }));
        if let Some(file) = out.as_ref().and_then(|v| v["blocks"]["store"].as_str()) {
            self.requests.push(Request::SaveBlockStore(file.to_owned()));
        }
        self.r24_open(crate::doc_forms::DocFormKind::Organizer24)
    }

    /// Building Blocks Organizer › Edit Properties…: the Create New Building Block dialog
    /// with the block's properties; its OK changes them.
    fn r24_edit_block(&mut self, key: &str) -> bool {
        let blocks = self.r24_blocks();
        let Some(b) = blocks.iter().find(|b| b["key"] == key).cloned() else {
            return false;
        };
        if b["store"] == "builtin" {
            // Built-in blocks cannot change (the engine refuses).
            let text = self.r24_t(
                "doc24.builtinBlock",
                "Built-in building blocks cannot be changed.",
            );
            return self.r24_message("doc24.organizerTitle", "Building Blocks Organizer", text);
        }
        self.r3_open_form(crate::doc_forms::DocFormKind::NewBlock);
        let title = self.r24_t("doc24.modifyBlock", "Modify Building Block");
        if let Some(f) = self.r3.form.as_mut() {
            f.set_title(&title);
            f.set_text("name", b["name"].as_str().unwrap_or(""));
            f.set_text("category", b["category"].as_str().unwrap_or(""));
            f.set_text("desc", b["description"].as_str().unwrap_or(""));
            let gallery = b["gallery"].as_str().unwrap_or("");
            if let Some(i) = [
                "tbls", "docParts", "autoTxt", "coverPg", "hdrs", "ftrs", "txtBox",
            ]
            .iter()
            .position(|g| *g == gallery)
            {
                f.set_sel("gallery", Some(i));
            }
        }
        self.r24.editing = Some(key.to_owned());
        true
    }

    /// Context menu rows of round 24 (label, command): Search "…", Read Aloud, Translate.
    pub(super) fn r24_ctx_rows(&self) -> Vec<(String, String)> {
        let text = self.r24_lookup_text();
        let mut v = Vec::new();
        if !text.is_empty() {
            let short: String = if text.chars().count() > 20 {
                let mut s: String = text.chars().take(20).collect();
                s.push('…');
                s
            } else {
                text
            };
            v.push((
                self.strings.fmt("doc24.searchFor", &[("text", &short)]),
                "r24.search".into(),
            ));
        }
        v.push((
            self.r24_t("doc24.readAloud", "Read Aloud"),
            "r24.readAloud".into(),
        ));
        if v.len() > 1 {
            v.push((
                self.r24_t("doc24.translate", "Translate"),
                "r24.translate".into(),
            ));
        }
        v
    }

    /// Borders ▾ › Diagonal Down / Up Border on the selected cells: on unless the caret's
    /// cell has that diagonal already (then off), as the reference's toggles.
    fn r24_diagonal(&mut self, down: bool) -> bool {
        self.menu = None;
        let Some(t) = self.view.format().table.as_ref() else {
            return false;
        };
        let k = usize::from(!down);
        let has = t.cell_diagonals.as_deref().is_some_and(|d| d[k].is_some());
        let pen = (!has).then(|| json!({ "style": "single", "size": 4, "color": null }));
        crate::catalog::from_json(json!({
            "cmd": "tableEdit", "edit": { "kind": "cellDiagonal", "down": down, "pen": pen }
        }))
        .is_some_and(|c| self.view.run_command(c))
    }

    /// Round-24 input: Ctrl+F1, the collapsed ribbon's tabs, the display options button,
    /// the highlighter pen.
    pub(super) fn r24_input(&mut self, input: &Input) -> Option<bool> {
        // The peeking ribbon hides after a command ran (once its menus are closed).
        if self.r24.peek_ran
            && !matches!(input, Input::Release { .. })
            && self.menu.is_none()
            && self.r22.gallery.is_none()
        {
            self.r24.peek_ran = false;
            self.ribbon.set_peek(false);
        }
        match *input {
            Input::Key(
                Key::F(1),
                Mods {
                    ctrl: true,
                    shift: false,
                    alt: false,
                },
            ) if self.backstage.is_none() => {
                return Some(self.r24_set_collapsed(!self.ribbon.collapsed()));
            }
            Input::Resize { .. } | Input::Scale(_) => self.ribbon.set_peek(false),
            Input::Key(Key::Escape, _) if self.menu.is_none() && self.r22.gallery.is_none() => {
                if self.ribbon.peek() {
                    self.ribbon.set_peek(false);
                    return Some(true);
                }
                if self.r24.pen {
                    self.r24.pen = false;
                    return Some(true);
                }
            }
            Input::Move { x, y } => {
                let on = self.r24_in_display(x, y);
                if on != self.r24.hover_display {
                    self.r24.hover_display = on;
                    return Some(true);
                }
            }
            Input::Press { x, y, count, .. }
                if self.menu.is_none() && self.dialog.is_none() && self.backstage.is_none() =>
            {
                if self.r24_in_display(x, y) {
                    // The pin of the peeking ribbon: Always Show Ribbon.
                    if self.ribbon.peek() {
                        return Some(self.r24_set_collapsed(false));
                    }
                    return Some(self.r24_display_menu());
                }
                let tab = match self.ribbon.hit(x, y) {
                    Some(RibbonHit::Tab(i)) => Some(i),
                    _ => None,
                };
                // A double click on a tab collapses the ribbon (or shows it again).
                if count == 2 && tab.is_some() {
                    return Some(self.r24_set_collapsed(!self.ribbon.collapsed()));
                }
                if self.ribbon.collapsed() {
                    self.r24.tab_press = tab;
                    // A press below the peeking ribbon hides it (and goes on).
                    if self.ribbon.peek() && tab.is_none() && y >= self.ribbon.bottom() {
                        self.ribbon.set_peek(false);
                    }
                }
                if self.r24.pen && y >= self.view_top() && y < self.size.1 - self.status_h() {
                    self.r24.pen_drag = true;
                }
            }
            Input::Release { x, y } => {
                if let Some(i) = self.r24.tab_press.take() {
                    if self.ribbon.hit(x, y) == Some(RibbonHit::Tab(i)) {
                        let again = self.ribbon.peek() && self.ribbon.active() == i;
                        self.ribbon.set_peek(!again);
                    }
                }
                if std::mem::take(&mut self.r24.pen_drag) {
                    let selected = self
                        .view
                        .engine()
                        .session()
                        .is_some_and(|s| !s.selection().is_collapsed());
                    if selected {
                        let hex = self
                            .r23
                            .highlight
                            .clone()
                            .unwrap_or_else(|| "FFFF00".to_owned());
                        self.view.run_command(Command::FormatText {
                            format: bo_edit::CharFormat::Highlight { hex: Some(hex) },
                        });
                        if let Some(focus) =
                            self.view.engine().session().map(|s| s.selection().focus)
                        {
                            self.view.run_command(Command::SetSelection {
                                selection: bo_edit::Selection::caret(focus),
                            });
                        }
                    }
                }
            }
            _ => {}
        }
        None
    }

    /// The peeking ribbon over the document and the display options button.
    pub(super) fn r24_paint(&self, p: &mut dyn Painter) {
        if self.r24_peek_h() > 0.0 {
            let on = self.on_ids();
            self.ribbon.paint(
                p,
                &self.theme,
                &self.icons,
                &self.strings,
                self.view.format(),
                &self.styles,
                &on,
            );
            // Its shadow on the document (the reference's floating ribbon).
            let bottom = self.ribbon.top() + self.ribbon.full_height();
            for (i, a) in [40u8, 24, 12].iter().enumerate() {
                p.fill_rect(0.0, bottom + i as f32, self.size.0, 1.0, Rgba(0, 0, 0, *a));
            }
        }
        if let Some(r) = self.r24_display_rect() {
            if self.r24.hover_display {
                p.fill_rect(r[0], r[1], r[2], r[3], self.theme.hover);
            }
            // A chevron: up (collapse) while docked, a pin while peeking.
            let ink = Rgba(0x44, 0x44, 0x44, 255);
            let (cx, cy, s) = (r[0] + r[2] / 2.0, r[1] + r[3] / 2.0, self.scale);
            let segs = if self.ribbon.peek() {
                // Pin: a head, a needle.
                vec![
                    PathSeg::M(cx - 3.0 * s, cy - 4.0 * s),
                    PathSeg::L(cx + 3.0 * s, cy - 4.0 * s),
                    PathSeg::L(cx + 2.0 * s, cy + 1.0 * s),
                    PathSeg::L(cx - 2.0 * s, cy + 1.0 * s),
                    PathSeg::Z,
                    PathSeg::M(cx, cy + 1.0 * s),
                    PathSeg::L(cx, cy + 5.0 * s),
                ]
            } else {
                vec![
                    PathSeg::M(cx - 4.0 * s, cy + 2.0 * s),
                    PathSeg::L(cx, cy - 2.0 * s),
                    PathSeg::L(cx + 4.0 * s, cy + 2.0 * s),
                ]
            };
            p.path(&segs, None, Some((ink, (1.0 * s).round().max(1.0))));
        }
    }
}

pub(crate) fn install(_icons: &mut crate::icons::Icons, _s: &mut crate::strings::Strings) {}
