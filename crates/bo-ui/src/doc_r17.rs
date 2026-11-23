//! BlueDoc parity round 17 (docs/ui/REPORT.md "Vòng 107"): the Insert and View tabs'
//! commands that were dimmed placeholders — Bookmark, Object… / Text from File…, Table ›
//! Excel Spreadsheet (an embedded BlueSheet workbook), Online Videos, View › Macros (view
//! only), Switch Windows, View Side by Side, Properties — and the files the host picks for
//! them. A child module of the shell.
use super::{DocWindow, Request};
use crate::doc_forms::DocFormKind;
use crate::popup::{Choice, Menu, MenuItem};
use crate::ribbon::RibbonHit;
use serde_json::{json, Value};

#[cfg(test)]
#[path = "doc_r17_tests.rs"]
mod tests;

/// Round-17 state of the window.
#[derive(Default)]
pub(super) struct R17 {
    /// The module Macros › Edit shows.
    module: Option<String>,
    /// Other BlueDoc windows (handle, title) the host listed last (Switch Windows).
    windows: Vec<(isize, String)>,
    /// The VBA project of the document (read once per document version).
    vba: Option<(u64, Value)>,
    /// BlueDoc windows open, as the host counted them (0: not asked yet = one).
    count: usize,
    /// Watermark texts the user saved with Save Selection to Watermark Gallery… (newest
    /// last), kept by the host in BlueOffice's own gallery file.
    saved_wm: Vec<String>,
    /// The View tab was shown at the last layout (the host is asked for the windows once
    /// each time it is chosen).
    view_seen: bool,
    /// Immersive Reader settings while it is on.
    pub(super) immersive: Option<Immersive>,
    /// The system's speech voices (names) and the one Read Aloud uses.
    voices: Vec<String>,
    voice: usize,
    /// Page Movement › Side to Side: the zoom to go back to with Vertical.
    pub(super) side: Option<f32>,
    /// The rulers were shown before the Immersive Reader hid them.
    ruler_before: Option<bool>,
    /// Shortcut keys of styles (key name, style name) set with Customize Keyboard.
    keys: Vec<(String, String)>,
}

/// View › Immersive Reader: column width (0 very narrow … 3 wide), page colour (0 none,
/// 1 sepia, 2 inverse), line focus (0 none, 1, 3 or 5 lines), text spacing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Immersive {
    column: u8,
    color: u8,
    focus: u8,
    spacing: bool,
}

/// Text column widths of the Immersive Reader in points (Wide: the whole window).
const COLUMNS: [f32; 4] = [204.0, 306.0, 408.0, 0.0];

/// Immersive Reader menus: (id, English label).
const COLUMN_MENU: [(&str, &str); 4] = [
    ("ir.column.0", "Very Narrow"),
    ("ir.column.1", "Narrow"),
    ("ir.column.2", "Moderate"),
    ("ir.column.3", "Wide"),
];
const COLOR_MENU: [(&str, &str); 3] = [
    ("ir.color.0", "None"),
    ("ir.color.1", "Sepia"),
    ("ir.color.2", "Inverse"),
];
const FOCUS_MENU: [(&str, &str); 4] = [
    ("ir.focus.0", "None"),
    ("ir.focus.1", "One Line"),
    ("ir.focus.3", "Three Lines"),
    ("ir.focus.5", "Five Lines"),
];

/// Commands of round 17 that are no longer dimmed: (id, `&'static` id).
pub(crate) const IDS: [&str; 21] = [
    "insert.models3d",
    "insert.screenshot",
    "view.sideToSide",
    "view.immersiveReader",
    "ir.columnWidth",
    "ir.pageColor",
    "ir.lineFocus",
    "ir.textSpacing",
    "ir.close",
    "insert.icons",
    "insert.bookmarkDialog",
    "insert.object.object",
    "insert.object.textFromFile",
    "insert.table.excel",
    "insert.onlineVideo",
    "view.macros.view",
    "view.properties",
    "view.switchWindows",
    "view.sideBySide",
    "view.resetWindow",
    "insert.mediaGroup",
];

impl DocWindow {
    fn r17_t(&self, key: &str, fallback: &str) -> String {
        let v = self.strings.get(key);
        if v == key {
            fallback.to_owned()
        } else {
            v.to_owned()
        }
    }

    /// A round-17 command; `None` when `id` is not one.
    #[inline(never)]
    pub(super) fn r17_action(&mut self, id: &str) -> Option<bool> {
        // Another view closes the Immersive Reader first.
        if id.starts_with("view.layout.") && self.r17.immersive.take().is_some() {
            if let Some(r) = self.r17.ruler_before.take() {
                self.show_ruler = r;
            }
            self.sync_context();
        }
        if let Some(p) = id.strip_prefix("r17.pick:") {
            let filter = match p {
                "object" => "workbooks",
                "textFromFile" => "documents",
                "watermark" => "pictures",
                "model3d" => "models",
                _ => "all",
            };
            self.requests.push(Request::PickFile {
                purpose: p.to_owned(),
                filter: filter.to_owned(),
            });
            return Some(false);
        }
        if let Some(u) = id.strip_prefix("r17.video:") {
            return Some(self.r17_insert_video(u));
        }
        if let Some(r) = self.r17_immersive_action(id) {
            return Some(r);
        }
        if let Some(i) = id.strip_prefix("r17.voice:") {
            if let Ok(i) = i.parse::<usize>() {
                self.r17.voice = i;
                self.requests.push(Request::SetVoice(i));
            }
            return Some(true);
        }
        if let Some(spec) = id.strip_prefix("r17.key.assign:") {
            if let Some((key, style)) = spec.split_once('\u{1f}') {
                self.r17.keys.retain(|(k, _)| k != key);
                self.r17.keys.push((key.to_owned(), style.to_owned()));
                self.r17_save_keys();
            }
            return Some(true);
        }
        if let Some(key) = id.strip_prefix("r17.key.remove:") {
            self.r17.keys.retain(|(k, _)| k != key);
            self.r17_save_keys();
            return Some(true);
        }
        if let Some(p) = id.strip_prefix("r17.styleFx:") {
            let patch: Value = serde_json::from_str(p).unwrap_or(Value::Null);
            if let Some(f) = self.r3.form.as_mut() {
                f.style_set_spec_field("advanced", patch);
            }
            return Some(true);
        }
        if let Some(i) = id.strip_prefix("r17.styleNum:") {
            return Some(self.r17_style_numbering(i.parse().unwrap_or(0)));
        }
        if let Some(list) = id.strip_prefix("r17.icons:") {
            return Some(self.r17_insert_icons(list));
        }
        if let Some(h) = id.strip_prefix("r17.shot:") {
            if let Ok(h) = h.parse::<isize>() {
                self.requests.push(Request::CaptureWindow(h));
            }
            return Some(false);
        }
        if let Some(h) = id.strip_prefix("r17.window:") {
            if let Ok(h) = h.parse::<isize>() {
                self.requests.push(Request::ActivateWindow(h));
            }
            return Some(false);
        }
        Some(match id {
            "insert.bookmarkDialog" => self.r3_open_form(DocFormKind::Bookmark),
            "insert.object.object" => self.r3_open_form(DocFormKind::InsertObject),
            "insert.object.textFromFile" => {
                self.menu = None;
                return self.r17_action("r17.pick:textFromFile");
            }
            // Table › Excel Spreadsheet: a new embedded workbook at the selection.
            "insert.table.excel" => {
                self.menu = None;
                self.r3_run(vec![json!({ "cmd": "oleInsert" })])
            }
            "insert.onlineVideo" => self.r3_open_form(DocFormKind::OnlineVideo),
            // Page Movement: Side to Side shows the pages two by two at the window's height,
            // like facing pages; Vertical goes back to one column at the former zoom.
            "view.sideToSide" => {
                if self.r17.side.is_none() {
                    self.r17.side = Some(self.view.zoom());
                    self.view.set_columns(2);
                    let z = self
                        .view
                        .fit_zoom(true)
                        .min(self.view.fit_zoom(false) * 0.5);
                    self.view.set_zoom(z.max(0.1));
                    self.relayout_pending = true;
                }
                true
            }
            "view.vertical" => {
                if let Some(z) = self.r17.side.take() {
                    self.view.set_columns(1);
                    self.view.set_zoom(z);
                    self.relayout_pending = true;
                }
                true
            }
            "insert.icons" => self.r3_open_form(DocFormKind::Icons),
            // 3D Models: a glTF model from a file, drawn by BlueOffice's renderer.
            "insert.models3d" => return self.r17_action("r17.pick:model3d"),
            "insert.screenshot" => {
                self.requests.push(Request::ListScreenshotWindows);
                false
            }
            "view.macros.view" => {
                self.menu = None;
                self.r3_open_form(DocFormKind::Macros)
            }
            "r17.macroSource" => self.r3_open_form(DocFormKind::MacroSource),
            // View › Properties: the document's properties on the File screen's Info page.
            "view.properties" => self.action("file:info"),
            // Switch Windows: the host lists the BlueDoc windows, then the menu opens.
            "view.switchWindows" => {
                self.requests.push(Request::ListWindows);
                false
            }
            "view.sideBySide" | "view.resetWindow" => {
                self.requests.push(Request::SideBySide);
                false
            }
            _ => return None,
        })
    }

    /// The `extra["r17"]` context of the round-17 dialogs: bookmarks (and the one at the
    /// selection), the VBA project, the document's name, the module to show.
    pub(super) fn r17_form_extra(&mut self) -> Value {
        let marks = self.view.engine().doc_bookmarks();
        let sel = self
            .view
            .engine()
            .session()
            .map(|s| s.selection().ordered());
        let current = sel.and_then(|(a, b)| {
            marks
                .iter()
                .find(|m| m.start <= a && b <= m.end && !m.hidden)
                .map(|m| m.name.clone())
        });
        let module = self
            .r3
            .form
            .as_ref()
            .or(self.r3.stash.as_ref())
            .and_then(crate::doc_forms::DocForm::r17_macro_module)
            .or_else(|| self.r17.module.clone());
        let style_name = self
            .r3
            .stash
            .as_ref()
            .filter(|f| f.kind == DocFormKind::NewStyle)
            .map(|f| {
                f.style_spec()["name"]
                    .as_str()
                    .unwrap_or_default()
                    .to_owned()
            });
        json!({
            "styleName": style_name,
            "keys": self.r17.keys,
            "bookmarks": marks,
            "current": current,
            "vba": self.r17_vba(),
            "docName": self.name,
            "module": module,
        })
    }

    /// The document's VBA project (`vbaProject`, read only), cached per version.
    fn r17_vba(&mut self) -> Value {
        let version = self
            .view
            .engine()
            .session()
            .map_or(0, bo_edit::Session::version);
        if let Some((v, p)) = &self.r17.vba {
            if *v == version {
                return p.clone();
            }
        }
        let p = self
            .view
            .engine_for_export()
            .command_json(r#"{"cmd":"vbaProject"}"#)
            .ok()
            .and_then(|s| serde_json::from_str::<Value>(&s).ok())
            .unwrap_or(Value::Null);
        self.r17.vba = Some((version, p.clone()));
        p
    }

    /// A file the host read for a round-17 request (`purpose` of [`Request::PickFile`]):
    /// Text from File inserts its content at the selection, Create from File fills the
    /// Object dialog, a watermark picture fills Printed Watermark.
    pub fn file_picked(&mut self, purpose: &str, path: &str, bytes: &[u8]) -> bool {
        use base64::Engine as _;
        let name = path.rsplit(['\\', '/']).next().unwrap_or(path).to_owned();
        match purpose {
            "textFromFile" => self.r17_text_from_file(&name, bytes),
            "object" => {
                let b64 = base64::engine::general_purpose::STANDARD.encode(bytes);
                if let Some(f) = self.r3.form.as_mut() {
                    f.r17_set_file(&name, path, &b64);
                }
                true
            }
            "model3d" => {
                let Some(png) = crate::doc_model3d17::render(bytes) else {
                    return false;
                };
                let stem = name.rsplit_once('.').map_or(name.as_str(), |(s, _)| s);
                let ok = self.view.insert_picture(png, &format!("{stem}.png"));
                self.sync_context();
                ok
            }
            "watermark" => {
                let b64 = base64::engine::general_purpose::STANDARD.encode(bytes);
                if let Some(f) = self.r3.form.as_mut() {
                    f.wm_set_picture(path, &b64);
                }
                true
            }
            _ => false,
        }
    }

    /// Insert › Object › Text from File…: the other file's content at the selection, kept
    /// with its formatting (opened in an engine of its own, copied whole, pasted here).
    fn r17_text_from_file(&mut self, name: &str, bytes: &[u8]) -> bool {
        use bo_engine::edit::Command;
        let mut e = bo_engine::Engine::with_bundled_fonts();
        if e.open(bytes, name).is_err() {
            return false;
        }
        let clip = e
            .command(Command::SelectAll)
            .and_then(|_| e.command(Command::Copy))
            .ok()
            .and_then(|r| r.clip);
        let Some(c) = clip else {
            return false;
        };
        let ok = self
            .view
            .paste(Some(c.internal), Some(c.html), Some(c.text));
        self.relayout_pending = true;
        ok
    }

    /// Format ▾ › Numbering… of the style dialog (back on top): a new list definition of
    /// the chosen format (§17.9.1 abstractNum through `defineList`, not applied to the text)
    /// whose instance the style refers to (§17.3.1.19 numPr); None removes numbering.
    fn r17_style_numbering(&mut self, i: usize) -> bool {
        let Some(&(_, fmt, text)) = crate::doc_forms::r17::STYLE_NUMBERS.get(i) else {
            return false;
        };
        let num = if fmt.is_empty() {
            0
        } else {
            let lvl = json!({ "format": fmt, "text": text, "indent": 720, "hanging": 360 });
            self.r3_run(vec![
                json!({ "cmd": "defineList", "levels": [lvl], "apply": false }),
            ]);
            self.view
                .engine()
                .document()
                .and_then(|d| d.numbering.nums.iter().map(|n| n.id).max())
                .unwrap_or(0)
        };
        if let Some(f) = self.r3.form.as_mut() {
            f.style_set_spec_field("numId", json!(num));
            f.style_set_spec_field("numLevel", json!(0));
        }
        true
    }

    /// Insert Icons › Insert: each chosen icon of the library as an SVG picture.
    fn r17_insert_icons(&mut self, list: &str) -> bool {
        let lib = crate::slide_pickers::load_library();
        let mut ok = false;
        for id in list.split(',') {
            if let Some(icon) = lib.iter().find(|i| i.id == id) {
                ok |= self
                    .view
                    .insert_picture(icon.svg.clone().into_bytes(), &format!("{id}.svg"));
            }
        }
        self.sync_context();
        self.relayout_pending = true;
        ok
    }

    /// Online Videos › Insert: a video frame (BlueOffice's own play-button picture) linked
    /// to the address, at the selection.
    fn r17_insert_video(&mut self, url: &str) -> bool {
        let Some(png) = video_frame() else {
            return false;
        };
        if !self.view.insert_picture(png, "Online video.png") {
            return false;
        }
        // The inserted picture is selected: the link goes on it (§17.16.22 hyperlink).
        self.r3_run(vec![
            json!({ "cmd": "setLink", "target": url, "display": "" }),
        ]);
        self.sync_context();
        true
    }

    /// The host's list of BlueDoc windows for Switch Windows (this one included, marked).
    pub fn show_windows(&mut self, windows: Vec<(isize, String, bool)>) -> bool {
        let anchor = self
            .ribbon
            .rect_of(RibbonHit::Command("view.switchWindows"))
            .or_else(|| self.ribbon.rect_of(RibbonHit::Command("view.windowGroup")))
            .unwrap_or([self.size.0 / 2.0, 120.0, 60.0, 90.0]);
        let items = windows
            .iter()
            .enumerate()
            .map(|(i, (h, title, me))| MenuItem {
                label: format!("{} {title}", i + 1),
                choice: Choice::Command(format!("r17.window:{h}")),
                checked: *me,
                dim: false,
            })
            .collect::<Vec<_>>();
        if items.is_empty() {
            return false;
        }
        self.r17.windows = windows.into_iter().map(|(h, t, _)| (h, t)).collect();
        self.menu = Some(Menu::open(items, anchor, self.size, self.scale));
        true
    }

    /// Insert › Screenshot: the Available Windows the host listed, as a menu under the
    /// button (a choice captures that window into the document).
    pub fn show_screenshot_windows(&mut self, windows: &[(isize, String)]) -> bool {
        let anchor = self
            .ribbon
            .rect_of(RibbonHit::Command("insert.screenshot"))
            .unwrap_or([self.size.0 / 2.0, 120.0, 60.0, 90.0]);
        let mut items = vec![MenuItem {
            label: self.r17_t("doc17.shotAvailable", "Available Windows"),
            choice: Choice::Command(String::new()),
            checked: false,
            dim: true,
        }];
        items.extend(windows.iter().map(|(h, t)| MenuItem {
            label: t.chars().take(60).collect(),
            choice: Choice::Command(format!("r17.shot:{h}")),
            checked: false,
            dim: false,
        }));
        if windows.is_empty() {
            items.push(MenuItem {
                label: self.r17_t("doc17.shotNone", "No windows are available"),
                choice: Choice::Command(String::new()),
                checked: false,
                dim: true,
            });
        }
        self.menu = Some(Menu::open(items, anchor, self.size, self.scale));
        true
    }

    /// Whether more than one BlueDoc window is open (View Side by Side, Reset Window
    /// Position need two).
    pub fn set_window_count(&mut self, n: usize) {
        self.r17.count = n;
    }
}

impl DocWindow {
    /// The View tab dims the window commands one window cannot use; choosing the tab asks
    /// the host how many BlueDoc windows are open.
    pub(super) fn r17_layout(&mut self) {
        let view = self.ribbon.active_label() == "ribbon.tabs.view";
        if view && !self.r17.view_seen {
            self.requests.push(Request::CountWindows);
        }
        self.r17.view_seen = view;
        if view {
            let dim = if self.r17.count < 2 {
                vec!["view.sideBySide", "view.resetWindow", "view.syncScroll"]
            } else {
                // Synchronous Scrolling follows View Side by Side across processes: not yet.
                vec!["view.syncScroll"]
            };
            self.ribbon.set_extra_dim(dim);
        }
    }
}

impl DocWindow {
    /// The user's saved watermarks as a gallery category (General), when there are some.
    pub(super) fn r17_wm_set(&self) -> Option<(String, Vec<crate::doc_wm14::WmEntry>)> {
        if self.r17.saved_wm.is_empty() {
            return None;
        }
        let entries = self
            .r17
            .saved_wm
            .iter()
            .map(|t| crate::doc_wm14::WmEntry {
                text: t.clone(),
                diagonal: true,
                name: t.clone(),
            })
            .collect();
        Some((self.r17_t("doc17.wmGeneral", "General"), entries))
    }

    /// Save Selection to Watermark Gallery…: the selected text (first line, 60 characters
    /// at most) becomes a design of the General category, stored by the host.
    pub(super) fn r17_save_watermark(&mut self) -> bool {
        let text = self.view.copy(false).map(|c| c.text).unwrap_or_default();
        let text: String = text
            .lines()
            .map(str::trim)
            .find(|l| !l.is_empty())
            .unwrap_or_default()
            .chars()
            .take(60)
            .collect();
        if text.is_empty() {
            return false;
        }
        self.r17.saved_wm.retain(|t| *t != text);
        self.r17.saved_wm.push(text);
        let json = serde_json::to_string(&self.r17.saved_wm).unwrap_or_default();
        self.requests.push(Request::SaveWatermarks(json));
        true
    }

    /// The user's watermark gallery file (JSON list of texts) read by the host at start.
    pub fn load_watermarks(&mut self, json: &str) {
        self.r17.saved_wm = serde_json::from_str(json).unwrap_or_default();
    }
}

impl DocWindow {
    /// The Immersive Reader is on (its tab shows).
    pub(super) fn r17_immersive_on(&self) -> bool {
        self.r17.immersive.is_some()
    }

    fn r17_immersive_action(&mut self, id: &str) -> Option<bool> {
        let menu = |w: &mut Self, anchor: &'static str, rows: &[(&str, &str)], on: usize| {
            let r = w.ribbon.rect_of(RibbonHit::Command(anchor)).unwrap_or([
                w.size.0 / 2.0,
                120.0,
                60.0,
                90.0,
            ]);
            let items = rows
                .iter()
                .enumerate()
                .map(|(i, (cmd, en))| MenuItem {
                    label: w.r17_t(&format!("doc17.{}", cmd.replace('.', "_")), en),
                    choice: Choice::Command((*cmd).to_owned()),
                    checked: i == on,
                    dim: false,
                })
                .collect();
            w.menu = Some(Menu::open(items, r, w.size, w.scale));
            true
        };
        let im = self.r17.immersive;
        Some(match id {
            "view.immersiveReader" => {
                // Leaves Read Mode and Focus, as the reference's.
                if self.read_mode.is_some() {
                    self.leave_read_mode();
                }
                // No rulers in the Immersive Reader, as the reference.
                if self.r17.ruler_before.is_none() {
                    self.r17.ruler_before = Some(self.show_ruler);
                }
                self.show_ruler = false;
                self.r17.immersive = Some(Immersive {
                    column: 2,
                    color: 0,
                    focus: 0,
                    spacing: true,
                });
                self.activate_context = true;
                self.r17_apply_immersive()
            }
            "ir.close" => {
                self.r17.immersive = None;
                if let Some(r) = self.r17.ruler_before.take() {
                    self.show_ruler = r;
                }
                self.relayout_pending = true;
                self.view_mode = "print";
                let ok = self.r3_run(vec![json!({ "cmd": "setViewLayout", "mode": "print" })]);
                self.sync_context();
                ok
            }
            "ir.columnWidth" => menu(self, "ir.columnWidth", &COLUMN_MENU, im?.column as usize),
            "ir.pageColor" => menu(self, "ir.pageColor", &COLOR_MENU, im?.color as usize),
            "ir.lineFocus" => {
                let on = match im?.focus {
                    0 => 0,
                    1 => 1,
                    3 => 2,
                    _ => 3,
                };
                menu(self, "ir.lineFocus", &FOCUS_MENU, on)
            }
            "ir.textSpacing" => {
                let mut m = im?;
                m.spacing = !m.spacing;
                self.r17.immersive = Some(m);
                self.r17_apply_immersive()
            }
            _ => {
                let (kind, n) = id.strip_prefix("ir.")?.split_once('.')?;
                let n: u8 = n.parse().ok()?;
                let mut m = im?;
                match kind {
                    "column" => m.column = n.min(3),
                    "color" => m.color = n.min(2),
                    "focus" => m.focus = n.min(5),
                    _ => return None,
                }
                self.menu = None;
                self.r17.immersive = Some(m);
                if kind == "focus" {
                    return Some(true);
                }
                self.r17_apply_immersive()
            }
        })
    }

    /// Lay the document out in the Immersive Reader view with the current settings.
    fn r17_apply_immersive(&mut self) -> bool {
        let Some(m) = self.r17.immersive else {
            return false;
        };
        let pt = ((self.size.0 - 24.0 * self.scale) / self.view.scale()).max(144.0);
        self.view_mode = "web";
        self.spell_pending = true;
        self.relayout_pending = true;
        let ok = self.r3_run(vec![json!({
            "cmd": "setViewLayout", "mode": "immersive", "width": pt,
            "immersive": {
                "column": COLUMNS[usize::from(m.column.min(3))],
                "spacing": m.spacing, "color": m.color,
            },
        })]);
        self.sync_context();
        ok
    }

    /// Ids shown pressed by the Immersive Reader tab (Text Spacing).
    pub(super) fn r17_on_ids(&self) -> Vec<&'static str> {
        let mut on = match self.r17.immersive {
            Some(m) if m.spacing => vec!["ir.textSpacing"],
            _ => Vec::new(),
        };
        if self.r17.side.is_some() {
            on.push("view.sideToSide");
        }
        on
    }

    /// Side to Side is on (Vertical is then not shown pressed).
    pub(super) fn r17_side_to_side(&self) -> bool {
        self.r17.side.is_some()
    }

    /// Line Focus: everything but the 1, 3 or 5 lines around the caret is shaded.
    pub(super) fn r17_paint(&self, p: &mut dyn crate::Painter) {
        let Some(m) = self.r17.immersive else {
            return;
        };
        if m.focus == 0 {
            return;
        }
        let Some([_, cy, _, ch]) = self.view.caret_rect() else {
            return;
        };
        let line = ch.max(1.0) * if m.spacing { 2.5 } else { 1.2 };
        let band = line * f32::from(m.focus);
        let mid = cy + ch / 2.0;
        let (top, bottom) = (self.chrome_bottom(), self.size.1 - self.status_h());
        let (a, b) = ((mid - band / 2.0).max(top), (mid + band / 2.0).min(bottom));
        let shade = bo_render::Rgba(0, 0, 0, 0xb0);
        if a > top {
            p.fill_rect(0.0, top, self.size.0, a - top, shade);
        }
        if bottom > b {
            p.fill_rect(0.0, b, self.size.0, bottom - b, shade);
        }
    }
}

/// The Immersive Reader contextual tab: Column Width ▾, Page Color ▾, Line Focus ▾, Text
/// Spacing, Read Aloud; Close Immersive Reader.
pub(crate) fn immersive_tab() -> crate::ribbon::Tab {
    use crate::ribbon::{Group, Item, Tab};
    Tab {
        label: "doc17.tabImmersive",
        groups: vec![
            Group {
                label: "doc17.groupImmersive",
                large: vec![
                    Item::Large("ir.columnWidth"),
                    Item::Large("ir.pageColor"),
                    Item::Large("ir.lineFocus"),
                    Item::Large("ir.textSpacing"),
                    Item::Large("review.readAloud"),
                ],
                rows: vec![],
            },
            Group {
                label: "doc17.groupClose",
                large: vec![Item::Large("ir.close")],
                rows: vec![],
            },
        ],
    }
}

/// Menus of the Immersive Reader tab (they mark their buttons with ▾).
pub(crate) fn menu(id: &str) -> Vec<crate::catalog::Entry> {
    let rows: &[(&'static str, &'static str)] = match id {
        "ir.columnWidth" => &COLUMN_MENU,
        "ir.pageColor" => &COLOR_MENU,
        "ir.lineFocus" => &FOCUS_MENU,
        // The windows are listed when the menu opens; the entry marks the button ▾.
        "insert.screenshot" => &[("insert.screenshot", "Available Windows")],
        _ => return Vec::new(),
    };
    rows.iter()
        .map(|(id, label)| crate::catalog::Entry { id, label })
        .collect()
}

impl DocWindow {
    /// The system's speech voices (host, at the start of Read Aloud) and the current one.
    pub fn set_voices(&mut self, voices: Vec<String>, current: usize) {
        self.r17.voices = voices;
        self.r17.voice = current;
    }

    /// Voice entries of Read Aloud › Settings (after the speeds).
    pub(super) fn r17_voice_items(&self) -> Vec<MenuItem> {
        self.r17
            .voices
            .iter()
            .enumerate()
            .map(|(i, v)| MenuItem {
                label: v.clone(),
                choice: Choice::Command(format!("r17.voice:{i}")),
                checked: i == self.r17.voice,
                dim: false,
            })
            .collect()
    }
}

impl DocWindow {
    fn r17_save_keys(&mut self) {
        let json = serde_json::to_string(&self.r17.keys).unwrap_or_default();
        self.requests.push(Request::SaveKeys(json));
    }

    /// The user's style shortcut keys (Customize Keyboard), read by the host at start.
    pub fn load_key_bindings(&mut self, json: &str) {
        self.r17.keys = serde_json::from_str(json).unwrap_or_default();
    }

    /// A style shortcut key pressed in the document applies its style (`None`: not one).
    pub(super) fn r17_input(&mut self, input: &crate::Input) -> Option<bool> {
        let crate::Input::Key(k, m) = *input else {
            return None;
        };
        if self.r17.keys.is_empty() || self.r3.form.is_some() || self.menu.is_some() {
            return None;
        }
        let name = crate::doc_forms::r17::key_name(k, m)?;
        let style = self
            .r17
            .keys
            .iter()
            .find(|(key, _)| *key == name)?
            .1
            .clone();
        let id = self
            .view
            .all_styles()
            .into_iter()
            .find(|s| s.name == style || s.id == style)
            .map(|s| s.id)?;
        Some(self.r3_run(vec![json!({ "cmd": "applyStyle", "id": id })]))
    }
}

/// A 16:9 video frame: dark face, a white disc with a play triangle (own drawing).
pub(crate) fn video_frame() -> Option<Vec<u8>> {
    let (w, h) = (640u32, 360u32);
    let mut px = vec![0u8; (w * h * 4) as usize];
    let (cx, cy, r) = (w as f32 / 2.0, h as f32 / 2.0, 54.0f32);
    for y in 0..h {
        for x in 0..w {
            let i = ((y * w + x) * 4) as usize;
            let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
            let shade = 40 + (y * 24 / h) as u8;
            let mut c = [shade, shade, shade + 6, 255];
            let d = ((fx - cx).powi(2) + (fy - cy).powi(2)).sqrt();
            if d <= r {
                c = [250, 250, 250, 255];
                // Play triangle pointing right, centred in the disc.
                let (tx, ty) = (fx - cx + 6.0, fy - cy);
                if tx > -20.0 && tx < 26.0 && ty.abs() < (26.0 - tx) * 0.62 {
                    c = [0x2f, 0x6f, 0xba, 255];
                }
            }
            px[i..i + 4].copy_from_slice(&c);
        }
    }
    bo_raster::rgba_to_png(bo_raster::codecs::Rgba {
        width: w,
        height: h,
        pixels: px,
    })
}

/// Labels (id, English) of round-17 commands; translations under `doc17.cmd.<id with _>`.
pub(crate) const LABELS: [(&str, &str); 9] = [
    ("ir.columnWidth", "Column Width"),
    ("ir.pageColor", "Page Color"),
    ("ir.lineFocus", "Line Focus"),
    ("ir.textSpacing", "Text Spacing"),
    ("ir.close", "Close Immersive Reader"),
    ("insert.onlineVideo", "Online Videos"),
    ("view.properties", "Properties"),
    ("insert.table.excel", "Spreadsheet"),
    ("view.macros.view", "View Macros"),
];

/// Labels and BlueOffice's own icon drawings of the round-17 buttons (24-unit grid, the
/// BlueDoc family).
pub(crate) fn install(icons: &mut crate::icons::Icons, s: &mut crate::strings::Strings) {
    use bo_render::Rgba;
    for (id, en) in LABELS {
        let key = format!("doc17.cmd.{}", id.replace('.', "_"));
        let v = s.get(&key).to_owned();
        s.set(
            &format!("commands.{id}"),
            if v == key { en } else { v.as_str() },
        );
    }
    for (k, en) in [
        ("doc17.tabImmersive", "Immersive Reader"),
        ("doc17.groupImmersive", "Immersive Reader"),
        ("doc17.groupClose", "Close"),
        ("doc17.groupMedia", "Media"),
        ("doc17.groupProperties", "Properties"),
    ] {
        if s.get(k) == k {
            s.set(k, en);
        }
    }
    let rgb = |r, g, b| Some(Rgba(r, g, b, 255));
    let grey = rgb(0x6e, 0x6e, 0x6e);
    let blue = rgb(0x2f, 0x6f, 0xba);
    let white = rgb(255, 255, 255);
    let green = rgb(0x2e, 0x8b, 0x57);
    // Online Videos: a screen with a blue play triangle and a small globe ring.
    icons.add_solid(
        "insert.onlineVideo",
        &[
            ("M2 4h17v12h-17Z", white),
            ("M2 4h17v12h-17ZM3 5v10h15v-10Z", grey),
            ("M8.5 7.5l5 2.5l-5 2.5Z", blue),
            ("M7 18h7v1.3h-7Z", grey),
            (
                "M18 13.5a4 4 0 1 1 0 8a4 4 0 1 1 0-8ZM18 14.7a2.8 2.8 0 1 0 0 5.6a2.8 2.8 0 1 0 0-5.6Z",
                blue,
            ),
            ("M14.6 17.2h6.8v0.9h-6.8ZM17.5 14v7h0.9v-7Z", blue),
        ],
    );
    // Properties: a page with check marks beside its lines.
    icons.add_solid(
        "view.properties",
        &[
            ("M4 2h12l4 4v16h-16Z", white),
            ("M4 2h12l4 4v16h-16ZM5 3v18h14v-14.6l-3.4-3.4Z", grey),
            (
                "M7 8l1 1l2-2.2l0.8 0.7l-2.8 3l-1.8-1.8ZM7 13l1 1l2-2.2l0.8 0.7l-2.8 3l-1.8-1.8Z",
                green,
            ),
            ("M12 8h5v1.3h-5ZM12 13h5v1.3h-5ZM8 18h9v1.3h-9Z", grey),
        ],
    );
    let red = rgb(0xd2, 0x4b, 0x3a);
    // Immersive Reader tab (own drawings): column width — a frame with a double arrow.
    icons.add_solid(
        "ir.columnWidth",
        &[
            ("M3 6h18v12h-18Z", white),
            ("M3 6h18v12h-18ZM4 7v10h16v-10Z", grey),
            ("M6 12l3-3v2h6v-2l3 3l-3 3v-2h-6v2Z", blue),
        ],
    );
    // Page Color: a page over a sepia swatch.
    icons.add_solid(
        "ir.pageColor",
        &[
            ("M8 9h13v13h-13Z", rgb(0xe8, 0xd5, 0xb0)),
            ("M3 2h11l3 3v12h-14Z", white),
            ("M3 2h11l3 3v12h-14ZM4 3v13h12v-10.6l-2.4-2.4Z", grey),
            ("M6 7h8v1h-8ZM6 10h8v1h-8ZM6 13h8v1h-8Z", grey),
        ],
    );
    // Line Focus: dark bands around one bright line.
    icons.add_solid(
        "ir.lineFocus",
        &[
            ("M3 4h18v6h-18ZM3 14h18v6h-18Z", grey),
            ("M3 10h18v4h-18Z", white),
            ("M5 11.5h14v1h-14Z", blue),
        ],
    );
    // Text Spacing: two letters pushed apart.
    icons.add_solid(
        "ir.textSpacing",
        &[
            (
                "M3 14l3-9h1.6l3 9h-1.6l-0.7-2.2h-3l-0.7 2.2ZM5.8 10.6h2.2l-1.1-3.4Z",
                grey,
            ),
            ("M14 5h7v1.4l-5.2 6.2h5.2v1.4h-7v-1.4l5.2-6.2h-5.2Z", grey),
            ("M4 18l3-2.5v1.8h10v-1.8l3 2.5l-3 2.5v-1.8h-10v1.8Z", blue),
        ],
    );
    // Close Immersive Reader: a red square with a white cross.
    icons.add_solid(
        "ir.close",
        &[
            ("M3 3h18v18h-18Z", red),
            (
                "M7 8.4l1.4-1.4l3.6 3.6l3.6-3.6l1.4 1.4l-3.6 3.6l3.6 3.6l-1.4 1.4l-3.6-3.6l-3.6 3.6l-1.4-1.4l3.6-3.6Z",
                white,
            ),
        ],
    );
    // Excel Spreadsheet (Table menu): a green grid.
    icons.add_solid(
        "insert.table.excel",
        &[
            ("M3 4h18v16h-18Z", white),
            (
                "M3 4h18v16h-18ZM4 5v3h16v-3ZM4 9v10h5v-10ZM10 9v10h4v-10ZM15 9v10h5v-10Z",
                green,
            ),
        ],
    );
}
