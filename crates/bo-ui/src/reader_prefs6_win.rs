//! BlueReader round 6 (a child of `reader_window`): File › Preferences and File › Batch Print.
//!
//! Preferences keeps what the reference reader's dialog offers for reading (its "Page
//! Display", "Commenting" / "Identity", "Measure" and "General" pages): the default zoom and
//! page layout of documents opened from now on, the author name of new comments, the units of
//! the measuring tools and smooth (animated) wheel scrolling. The host stores them
//! ([`ReaderRequest::SavePreferences`], `key=value` lines) and gives them back at start
//! ([`ReaderWindow::set_preferences`]).
//!
//! Batch Print prints several open documents with one Print dialog: a list of the open
//! documents to tick, then the usual dialog; the host prints every ticked document with its
//! settings (all pages of each).
use super::{inside, ReaderRequest, ReaderWindow};
use crate::dialog::{Dialog, DialogButton, DialogKind};
use crate::popup::{Choice, MenuItem};
use crate::reader_view::{Fit, Layout};
use crate::{Input, Key, Painter, TextMeasure};

/// Default zoom choices: label key, value (`auto` keeps the document's own fit).
const ZOOMS: [(&str, &str); 9] = [
    ("pdfPrefs.auto", "auto"),
    ("pdfReader.actualSize", "actual"),
    ("pdfReader.fitPage", "page"),
    ("pdfReader.fitWidth", "width"),
    ("", "50"),
    ("", "75"),
    ("", "125"),
    ("", "150"),
    ("", "200"),
];
/// Page layouts: label key, value.
const LAYOUTS: [(&str, &str); 5] = [
    ("pdfPrefs.auto", "auto"),
    ("pdfPrefs.single", "single"),
    ("pdfPrefs.continuous", "continuous"),
    ("pdfPrefs.facing", "facing"),
    ("pdfPrefs.continuousFacing", "continuousFacing"),
];
/// Measuring units: label key, value, points per unit.
pub(crate) const UNITS: [(&str, &str, f32); 5] = [
    ("pdfPrefs.inch", "in", 72.0),
    ("pdfPrefs.mm", "mm", 72.0 / 25.4),
    ("pdfPrefs.cm", "cm", 72.0 / 2.54),
    ("pdfPrefs.point", "pt", 1.0),
    ("pdfPrefs.pica", "pc", 12.0),
];

/// History › documents kept in the recent list (round 12).
pub(crate) const HISTORY: [usize; 5] = [5, 10, 15, 20, 30];
/// Speech › reading speeds: label key, SAPI rate (−10…10).
pub(crate) const RATES: [(&str, i32); 5] = [
    ("pdfPrefs12.rateSlowest", -6),
    ("pdfPrefs12.rateSlow", -3),
    ("pdfPrefs12.rateNormal", 0),
    ("pdfPrefs12.rateFast", 3),
    ("pdfPrefs12.rateFastest", 6),
];

/// The preferences.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Prefs {
    /// Index into [`ZOOMS`].
    pub zoom: usize,
    /// Index into [`LAYOUTS`].
    pub layout: usize,
    /// Author of new comments (empty: the Windows user name).
    pub author: String,
    /// Index into [`UNITS`].
    pub units: usize,
    /// Animate wheel scrolling.
    pub smooth: bool,
    /// Window skin (index into `skins8::SKINS`).
    pub skin: usize,
    /// Round 12. Forms › Highlight all fields (on, like the reference).
    pub forms_highlight: bool,
    /// Full Screen › Escape key exits; a left click turns to the next page, a right click
    /// to the previous one.
    pub fs_escape: bool,
    pub fs_click: bool,
    /// History › index into [`HISTORY`].
    pub history: usize,
    /// Search › whole words only, case-sensitive (the search box starts with them).
    pub search_whole: bool,
    pub search_case: bool,
    /// Speech › index into [`RATES`].
    pub speech_rate: usize,
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            zoom: 0,
            layout: 0,
            author: String::new(),
            units: 0,
            smooth: false,
            skin: 0,
            forms_highlight: true,
            fs_escape: true,
            fs_click: true,
            history: 3,
            search_whole: false,
            search_case: false,
            speech_rate: 2,
        }
    }
}

impl Prefs {
    /// `key=value` lines.
    pub(crate) fn to_text(&self) -> String {
        format!(
            "zoom={}\nlayout={}\nauthor={}\nunits={}\nsmooth={}\n",
            ZOOMS[self.zoom].1,
            LAYOUTS[self.layout].1,
            self.author.replace(['\n', '\r'], " "),
            UNITS[self.units].1,
            u8::from(self.smooth)
        ) + &format!(
            "skin={}
",
            self.skin
        ) + &format!(
            "formsHighlight={}\nfsEscape={}\nfsClick={}\nhistory={}\nsearchWhole={}\nsearchCase={}\nspeechRate={}\n",
            u8::from(self.forms_highlight),
            u8::from(self.fs_escape),
            u8::from(self.fs_click),
            HISTORY[self.history.min(HISTORY.len() - 1)],
            u8::from(self.search_whole),
            u8::from(self.search_case),
            RATES[self.speech_rate.min(RATES.len() - 1)].1
        )
    }

    /// Read `key=value` lines; unknown keys and values keep the defaults.
    pub(crate) fn from_text(s: &str) -> Self {
        let mut p = Self::default();
        for line in s.lines() {
            let Some((k, v)) = line.split_once('=') else {
                continue;
            };
            let v = v.trim();
            match k.trim() {
                "zoom" => p.zoom = ZOOMS.iter().position(|z| z.1 == v).unwrap_or(0),
                "layout" => p.layout = LAYOUTS.iter().position(|z| z.1 == v).unwrap_or(0),
                "author" => p.author = v.to_owned(),
                "units" => p.units = UNITS.iter().position(|z| z.1 == v).unwrap_or(0),
                "smooth" => p.smooth = v == "1",
                "skin" => p.skin = v.parse().unwrap_or(0),
                "formsHighlight" => p.forms_highlight = v != "0",
                "fsEscape" => p.fs_escape = v != "0",
                "fsClick" => p.fs_click = v != "0",
                "history" => {
                    let n: usize = v.parse().unwrap_or(20);
                    p.history = HISTORY.iter().position(|h| *h == n).unwrap_or(3);
                }
                "searchWhole" => p.search_whole = v == "1",
                "searchCase" => p.search_case = v == "1",
                "speechRate" => {
                    let r: i32 = v.parse().unwrap_or(0);
                    p.speech_rate = RATES.iter().position(|x| x.1 == r).unwrap_or(2);
                }
                _ => {}
            }
        }
        p
    }
}

/// The open Preferences dialog.
#[derive(Debug)]
pub(crate) struct PrefsDialog {
    /// Values being edited (applied by OK).
    edit: Prefs,
    /// Category shown.
    page: usize,
    rect: [f32; 4],
    hits: Vec<([f32; 4], String)>,
}

/// State of the round 6 File additions.
#[derive(Default)]
pub(crate) struct Prefs6 {
    pub prefs: Prefs,
    pub dialog: Option<PrefsDialog>,
    /// Batch Print: the list of documents to tick.
    pub batch_ask: Option<Dialog>,
    /// Documents (tab indices) the next Print dialog prints.
    pub batch: Vec<usize>,
    /// Wheel scrolling still to animate (device pixels).
    pub pending: f32,
    /// Asking the author name.
    pub author_ask: Option<Dialog>,
    /// Insert Text: the next click on text places a caret.
    pub caret: bool,
    /// Asking the text to insert at: page, caret box (view points).
    pub caret_ask: Option<(Dialog, usize, [f32; 4])>,
}

/// Colour of Insert Text carets.
const CARET: [u8; 3] = [0, 85, 220];

// Round 12: the dialog's categories, groups and painting, like the reference's.
#[path = "reader_prefs12_win.rs"]
mod prefs12;
/// The category the dialog opens on.
const FIRST_PAGE: usize = prefs12::FIRST;
/// Rows drawn as check boxes.
const CHECKS: [&str; 6] = [
    "smooth",
    "formsHighlight",
    "fsEscape",
    "fsClick",
    "searchWhole",
    "searchCase",
];

/// The check box of row `id`.
fn check_mut<'a>(p: &'a mut Prefs, id: &str) -> Option<&'a mut bool> {
    Some(match id {
        "smooth" => &mut p.smooth,
        "formsHighlight" => &mut p.forms_highlight,
        "fsEscape" => &mut p.fs_escape,
        "fsClick" => &mut p.fs_click,
        "searchWhole" => &mut p.search_whole,
        "searchCase" => &mut p.search_case,
        _ => return None,
    })
}

impl ReaderWindow {
    /// Preferences stored by the host (`key=value` lines).
    pub fn set_preferences(&mut self, text: &str) {
        self.prefs6.prefs = Prefs::from_text(text);
        self.apply_author();
        self.apply_skin(self.prefs6.prefs.skin);
        self.apply_prefs12();
    }

    /// Round 12's preferences that set the window's state: field highlight, the search box's
    /// options, the reading speed, the length of the recent list (BlueReader only).
    fn apply_prefs12(&mut self) {
        if self.profile.is_some() {
            return;
        }
        let p = self.prefs6.prefs.clone();
        self.tabs4.highlight_fields = p.forms_highlight;
        self.set_find_options8(p.search_case, p.search_whole);
        self.view4.rate = RATES[p.speech_rate.min(RATES.len() - 1)].1;
        let cap = self.history_cap6();
        self.recent.truncate(cap);
    }

    /// Most documents in the recent list (History preference; 12 in windows built on this).
    pub(crate) fn history_cap6(&self) -> usize {
        if self.profile.is_some() {
            12
        } else {
            HISTORY[self.prefs6.prefs.history.min(HISTORY.len() - 1)]
        }
    }

    /// Full Screen preferences: Escape exits, clicks turn pages.
    pub(crate) fn full_screen_prefs6(&self) -> (bool, bool) {
        let p = &self.prefs6.prefs;
        (p.fs_escape, p.fs_click)
    }

    /// The preferences as stored (`key=value` lines).
    pub fn preferences(&self) -> String {
        self.prefs6.prefs.to_text()
    }

    /// Documents (tab indices) a Batch Print prints; empty for a normal print. Taken once.
    pub fn take_batch(&mut self) -> Vec<usize> {
        std::mem::take(&mut self.prefs6.batch)
    }

    /// Document of tab `i`.
    pub fn view_at_mut(&mut self, i: usize) -> Option<&mut crate::reader_view::ReaderView> {
        self.tabs.get_mut(i).map(|t| &mut t.view)
    }

    /// Name of tab `i`.
    pub fn tab_name(&self, i: usize) -> Option<&str> {
        self.tabs.get(i).map(|t| t.name.as_str())
    }

    fn apply_author(&mut self) {
        if self.profile.is_none() {
            let a = self.prefs6.prefs.author.trim();
            self.author = if a.is_empty() {
                std::env::var("USERNAME").unwrap_or_else(|_| "BlueReader".into())
            } else {
                a.to_owned()
            };
        }
    }

    /// A new document view takes the default zoom and layout (BlueReader only).
    pub(crate) fn apply_view_prefs(&mut self) {
        if self.profile.is_some() {
            return;
        }
        let (zoom, layout) = (self.prefs6.prefs.zoom, self.prefs6.prefs.layout);
        let Some(t) = self.tabs.get_mut(self.active) else {
            return;
        };
        match LAYOUTS[layout].1 {
            "single" => t.view.set_layout(Layout::Single),
            "continuous" => t.view.set_layout(Layout::Continuous),
            "facing" => t.view.set_layout(Layout::Facing),
            "continuousFacing" => t.view.set_layout(Layout::ContinuousFacing),
            _ => {}
        }
        match ZOOMS[zoom].1 {
            "auto" => {}
            "actual" => t.view.set_zoom(1.0),
            "page" => t.view.set_fit(Fit::Page),
            "width" => t.view.set_fit(Fit::Width),
            v => {
                if let Ok(n) = v.parse::<f32>() {
                    t.view.set_zoom(n / 100.0);
                }
            }
        }
    }

    /// A distance, perimeter or area label in the chosen unit (the measuring tools).
    pub(crate) fn unit_label(&self, shape: &bo_pdfread::Shape) -> Option<String> {
        bo_pdfread::measure_label_in(shape, &self.measure_unit())
    }

    /// The measuring unit of the preferences (BlueEditor's window: inches), used for the
    /// labels while measuring and for the `/Measure` of saved measurements (§12.9).
    pub(crate) fn measure_unit(&self) -> bo_pdfread::MeasureUnit {
        if self.profile.is_none() {
            let (_, u, k) = UNITS[self.prefs6.prefs.units.min(UNITS.len() - 1)];
            bo_pdfread::MeasureUnit::new(u, k)
        } else {
            bo_pdfread::MeasureUnit::default()
        }
    }

    /// Wheel over the document: animated when smooth scrolling is on; `true` when taken.
    pub(crate) fn smooth_wheel(&mut self, dy: f32) -> bool {
        if !self.prefs6.prefs.smooth || self.profile.is_some() {
            return false;
        }
        self.prefs6.pending += dy;
        true
    }

    /// One animation frame of smooth scrolling; `true` while it moves.
    pub(crate) fn step_smooth(&mut self) -> bool {
        let pend = self.prefs6.pending;
        if pend.abs() < 0.5 {
            self.prefs6.pending = 0.0;
            return false;
        }
        // About 40 % of what is left per frame: a notch settles in ~100 ms at 60 Hz.
        let step = if pend.abs() <= 2.0 { pend } else { pend * 0.4 };
        self.prefs6.pending -= step;
        if let Some(t) = self.tabs.get_mut(self.active) {
            t.view.scroll_by(0.0, step);
        }
        true
    }

    /// Commands of the File page additions.
    pub(crate) fn command_prefs6(&mut self, cmd: &str, m: &dyn TextMeasure) -> bool {
        if cmd == "comment.insert" {
            self.prefs6.caret = !self.prefs6.caret;
            if self.prefs6.caret {
                self.tool = super::Tool::Select;
            }
            return true;
        }
        if cmd.starts_with("tool.") || cmd.starts_with("comment.") || cmd.starts_with("draw.") {
            self.prefs6.caret = false;
        }
        if let Some(rest) = cmd.strip_prefix("prefs.set.") {
            if let (Some((k, v)), Some(d)) = (rest.split_once('.'), self.prefs6.dialog.as_mut()) {
                let v: usize = v.parse().unwrap_or(0);
                match k {
                    "zoom" => d.edit.zoom = v.min(ZOOMS.len() - 1),
                    "layout" => d.edit.layout = v.min(LAYOUTS.len() - 1),
                    "units" => d.edit.units = v.min(UNITS.len() - 1),
                    "history" => d.edit.history = v.min(HISTORY.len() - 1),
                    "speechRate" => d.edit.speech_rate = v.min(RATES.len() - 1),
                    _ => {}
                }
            }
            return true;
        }
        match cmd {
            "bs.prefs" => {
                self.prefs6.dialog = Some(PrefsDialog {
                    edit: self.prefs6.prefs.clone(),
                    page: FIRST_PAGE,
                    rect: [0.0; 4],
                    hits: Vec::new(),
                });
                true
            }
            "bs.batch" => {
                let checks: Vec<(String, bool)> =
                    self.tabs.iter().map(|t| (t.name.clone(), true)).collect();
                let mut d = Dialog::new(
                    DialogKind::Comment,
                    self.t("pdfBatch.ask"),
                    Vec::new(),
                    vec![
                        (self.t("pdfBatch.print"), DialogButton::Ok),
                        (self.t("pdfPrint.cancel"), DialogButton::Cancel),
                    ],
                    Vec::new(),
                )
                .with_checks(checks);
                d.layout(m, self.size, self.scale);
                self.prefs6.batch_ask = Some(d);
                true
            }
            _ => false,
        }
    }

    fn pick(&mut self, what: &str, anchor: [f32; 4]) {
        let Some(d) = self.prefs6.dialog.as_ref() else {
            return;
        };
        let (list, cur): (Vec<String>, usize) = match what {
            "zoom" => (
                ZOOMS.iter().map(|z| self.zoom_label(z.0, z.1)).collect(),
                d.edit.zoom,
            ),
            "layout" => (LAYOUTS.iter().map(|z| self.t(z.0)).collect(), d.edit.layout),
            "history" => (
                HISTORY.iter().map(ToString::to_string).collect(),
                d.edit.history,
            ),
            "speechRate" => (
                RATES.iter().map(|z| self.t(z.0)).collect(),
                d.edit.speech_rate,
            ),
            _ => (UNITS.iter().map(|z| self.t(z.0)).collect(), d.edit.units),
        };
        let items = list
            .into_iter()
            .enumerate()
            .map(|(i, label)| MenuItem {
                label,
                choice: Choice::Command(format!("prefs.set.{what}.{i}")),
                checked: i == cur,
                dim: false,
            })
            .collect();
        self.menu = Some((self.reader_menu(items, anchor), "prefs"));
    }

    fn zoom_label(&self, key: &str, v: &str) -> String {
        if key.is_empty() {
            format!("{v}%")
        } else {
            self.t(key)
        }
    }

    /// Input of the Preferences and Batch Print dialogs; `Some` when used.
    pub(crate) fn route_prefs6(&mut self, input: &Input, m: &dyn TextMeasure) -> Option<bool> {
        if let Some(r) = self.route_caret(input, m) {
            return Some(r);
        }
        if let Some(r) = self.route_small_dialogs(input) {
            return Some(r);
        }
        // A drop-down of the dialog takes the input first.
        if self.prefs6.dialog.is_none() || self.menu.is_some() {
            return None;
        }
        match input {
            Input::Press { x, y, .. } => {
                let hit = self.prefs6.dialog.as_ref().and_then(|d| {
                    d.hits
                        .iter()
                        .find(|(r, _)| inside(*r, *x, *y))
                        .map(|h| (h.0, h.1.clone()))
                });
                let Some((r, id)) = hit else {
                    return Some(true);
                };
                if let Some(k) = id.strip_prefix("page.") {
                    if let Some(d) = self.prefs6.dialog.as_mut() {
                        d.page = k.parse().unwrap_or(0);
                    }
                } else if matches!(
                    id.as_str(),
                    "zoom" | "layout" | "units" | "history" | "speechRate"
                ) {
                    self.pick(&id, r);
                } else if CHECKS.contains(&id.as_str()) {
                    if let Some(c) = self
                        .prefs6
                        .dialog
                        .as_mut()
                        .and_then(|d| check_mut(&mut d.edit, &id))
                    {
                        *c = !*c;
                    }
                } else if id == "clearHistory" {
                    // History › Clear History: the recent list empties at once.
                    self.recent.clear();
                    self.status = self.t("pdfPrefs12.cleared");
                } else if id == "setDefault" {
                    // File Associations: Windows' Default apps page (as Help › Set to Default).
                    self.command("help.default", m);
                } else if id == "author" {
                    let cur = self
                        .prefs6
                        .dialog
                        .as_ref()
                        .map(|d| d.edit.author.clone())
                        .unwrap_or_default();
                    let mut a = Dialog::new(
                        DialogKind::Comment,
                        self.t("pdfPrefs.author"),
                        vec![(self.t("pdfPrefs.name"), cur)],
                        vec![
                            (self.t("pdfReader.ok"), DialogButton::Ok),
                            (self.t("pdfPrint.cancel"), DialogButton::Cancel),
                        ],
                        Vec::new(),
                    );
                    a.layout(m, self.size, self.scale);
                    self.prefs6.author_ask = Some(a);
                } else if id == "ok" {
                    self.prefs_ok();
                } else if id == "cancel" {
                    self.prefs6.dialog = None;
                }
                Some(true)
            }
            Input::Key(Key::Escape, _) => {
                self.prefs6.dialog = None;
                Some(true)
            }
            Input::Key(Key::Enter, _) => {
                self.prefs_ok();
                Some(true)
            }
            Input::Resize { .. } | Input::Scale(_) | Input::Tick | Input::Blink => None,
            _ => Some(true),
        }
    }

    /// Where a caret goes for a click at page point (`px`, `py`): the nearest character edge
    /// of the text there, the caret's point at the baseline (view points: left, top, right,
    /// bottom of its box).
    pub(crate) fn caret_box(&mut self, page: usize, px: f32, py: f32) -> Option<[f32; 4]> {
        let t = self.tabs.get_mut(self.active)?;
        let layer = t.view.text(page)?;
        let reach = 12.0;
        let i = layer.hit(px, py, reach)?;
        let c = layer.chars().get(i)?;
        let r = c.rect;
        let x = if px > f32::midpoint(r[0], r[2]) {
            r[2]
        } else {
            r[0]
        };
        let h = (r[3] - r[1]).max(4.0);
        let (w, ch) = (h * 0.45, h * 0.55);
        // The point at the bottom of the line's text, the base below it.
        let tip = r[3] - h * 0.18;
        Some([x - w / 2.0, tip, x + w / 2.0, tip + ch])
    }

    fn route_caret(&mut self, input: &Input, m: &dyn TextMeasure) -> Option<bool> {
        if let Some((d, _, _)) = self.prefs6.caret_ask.as_mut() {
            let done = match input {
                Input::Text(s) => {
                    d.text(s);
                    None
                }
                Input::Key(k, mods) => d.key(*k, *mods),
                Input::Press { x, y, .. } => d.contains(*x, *y).then(|| d.press(*x, *y)).flatten(),
                Input::Move { x, y } => {
                    d.hover(*x, *y);
                    None
                }
                Input::Resize { .. } | Input::Scale(_) | Input::Tick | Input::Blink => return None,
                _ => None,
            };
            if let Some(b) = done {
                if let Some((d, page, rect)) = self.prefs6.caret_ask.take() {
                    let text = d.values().into_iter().next().unwrap_or_default();
                    if b == DialogButton::Ok && !text.is_empty() {
                        let author = self.author.clone();
                        if let Some(t) = self.tabs.get_mut(self.active) {
                            let rect = t.view.unrotate_rect(page, rect);
                            match t
                                .view
                                .doc_mut()
                                .add_caret(page, rect, CARET, &author, &text)
                            {
                                Ok(()) => self.document_changed(),
                                Err(e) => self.status = e,
                            }
                        }
                    }
                }
            }
            return Some(true);
        }
        if !self.prefs6.caret || self.menu.is_some() || self.dialog.is_some() {
            return None;
        }
        match input {
            Input::Key(Key::Escape, _) => {
                self.prefs6.caret = false;
                Some(true)
            }
            Input::Press { x, y, .. } if inside(self.doc_rect, *x, *y) => {
                let hit = self.tabs.get(self.active)?.view.page_at(*x, *y)?;
                let (page, px, py) = hit;
                let Some(rect) = self.caret_box(page, px, py) else {
                    self.status = self.t("pdfComment.caretNoText");
                    return Some(true);
                };
                let mut d = Dialog::new(
                    DialogKind::Comment,
                    self.t("pdfComment.insertTitle"),
                    vec![(self.t("pdfComment.insertText"), String::new())],
                    vec![
                        (self.t("pdfReader.ok"), DialogButton::Ok),
                        (self.t("pdfPrint.cancel"), DialogButton::Cancel),
                    ],
                    Vec::new(),
                );
                d.layout(m, self.size, self.scale);
                self.prefs6.caret_ask = Some((d, page, rect));
                Some(true)
            }
            _ => None,
        }
    }

    fn prefs_ok(&mut self) {
        if let Some(d) = self.prefs6.dialog.take() {
            self.prefs6.prefs = d.edit;
            self.apply_author();
            self.apply_prefs12();
            self.requests
                .push(ReaderRequest::SavePreferences(self.prefs6.prefs.to_text()));
        }
    }

    /// The author name and Batch Print lists (generic dialogs).
    fn route_small_dialogs(&mut self, input: &Input) -> Option<bool> {
        let author = self.prefs6.author_ask.is_some();
        let d = if author {
            self.prefs6.author_ask.as_mut()?
        } else {
            self.prefs6.batch_ask.as_mut()?
        };
        let done = match input {
            Input::Text(s) => {
                d.text(s);
                None
            }
            Input::Key(k, mods) => d.key(*k, *mods),
            Input::Press { x, y, .. } => d.contains(*x, *y).then(|| d.press(*x, *y)).flatten(),
            Input::Move { x, y } => {
                d.hover(*x, *y);
                None
            }
            Input::Resize { .. } | Input::Scale(_) | Input::Tick | Input::Blink => return None,
            _ => None,
        };
        if let Some(b) = done {
            let d = if author {
                self.prefs6.author_ask.take()
            } else {
                self.prefs6.batch_ask.take()
            };
            if let (Some(d), DialogButton::Ok) = (d, b) {
                let values = d.values();
                if author {
                    if let Some(p) = self.prefs6.dialog.as_mut() {
                        p.edit.author = values.into_iter().next().unwrap_or_default();
                    }
                } else {
                    self.prefs6.batch = values
                        .iter()
                        .enumerate()
                        .filter(|(_, v)| *v == "1")
                        .map(|(i, _)| i)
                        .collect();
                    if !self.prefs6.batch.is_empty() {
                        self.requests.push(ReaderRequest::Print);
                    }
                }
            }
        }
        Some(true)
    }

    /// The Preferences dialog: categories at the left, the chosen one's rows at the right.
    #[allow(clippy::too_many_lines)]
    pub(crate) fn paint_prefs6(&mut self, p: &mut dyn Painter) {
        if let Some(mut d) = self.prefs6.dialog.take() {
            self.paint_dialog12(p, &mut d);
            self.prefs6.dialog = Some(d);
            // Its drop-down lists over it (the window paints menus before dialogs).
            if let Some((menu, "prefs")) = &self.menu {
                menu.paint(p, &self.theme, &self.icons);
            }
        }
        for d in [&self.prefs6.batch_ask, &self.prefs6.author_ask]
            .into_iter()
            .flatten()
        {
            d.paint(p, &self.theme, self.size, true);
        }
        if let Some((d, _, _)) = &self.prefs6.caret_ask {
            d.paint(p, &self.theme, self.size, true);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preferences_round_trip_as_text() {
        let p = Prefs {
            zoom: 3,
            layout: 4,
            author: "Nguyễn Văn A".into(),
            units: 1,
            smooth: true,
            skin: 2,
            forms_highlight: false,
            fs_escape: false,
            fs_click: true,
            history: 1,
            search_whole: true,
            search_case: false,
            speech_rate: 4,
        };
        let t = p.to_text();
        assert!(t.contains("zoom=width") && t.contains("units=mm"));
        assert!(t.contains("history=10") && t.contains("speechRate=6"));
        assert_eq!(Prefs::from_text(&t), p);
        assert_eq!(Prefs::from_text("zoom=nonsense\nfoo=1"), Prefs::default());
        // Files written before round 12 keep the reference's defaults for the new keys.
        let old = Prefs::from_text("zoom=page\nsmooth=1\n");
        assert!(old.forms_highlight && old.fs_escape && old.fs_click);
        assert_eq!(HISTORY[old.history], 20);
    }

    #[test]
    fn round12_preferences_change_the_window() {
        let doc = bo_pdfread::PdfDocument::open(
            crate::reader_tests::pdf(1),
            bo_pdfread::standard_fonts(),
        )
        .expect("pdf");
        let mut w = ReaderWindow::new("en-US");
        w.open(doc, "t.pdf", None);
        w.set_recent((0..30).map(|i| format!("C:/d/{i}.pdf").into()).collect());
        w.set_preferences(
            "formsHighlight=0\nsearchWhole=1\nsearchCase=1\nspeechRate=-6\nhistory=5\nfsEscape=0\n",
        );
        assert!(!w.tabs4.highlight_fields);
        assert_eq!(w.find_options8(), (true, true));
        assert_eq!(w.view4.rate, -6);
        assert_eq!(w.recent().len(), 5);
        assert_eq!(w.full_screen_prefs6(), (false, true));
        // The dialog: the reference's 26 categories, opening on General; a check box row toggles.
        w.run_command("bs.prefs", &crate::reader_tests::Fixed);
        let mut p = crate::reader_tests::Rec::default();
        w.paint_prefs6(&mut p);
        let d = w.prefs6.dialog.as_ref().expect("dialog");
        assert_eq!(d.page, FIRST_PAGE);
        assert_eq!(
            d.hits.iter().filter(|h| h.1.starts_with("page.")).count(),
            prefs12::CATS.len()
        );
        // Full Screen page: Escape exits is off (set above); a click turns it on, OK keeps it.
        let page = d.hits.iter().find(|h| h.1 == "page.6").map(|h| h.0);
        let r = page.expect("full screen page");
        let click = |w: &mut ReaderWindow, r: [f32; 4]| {
            let (x, y) = (r[0] + 4.0, r[1] + r[3] / 2.0);
            w.handle(
                Input::Press {
                    x,
                    y,
                    count: 1,
                    shift: false,
                },
                &crate::reader_tests::Fixed,
            );
            w.handle(Input::Release { x, y }, &crate::reader_tests::Fixed);
        };
        click(&mut w, r);
        w.paint_prefs6(&mut p);
        let row = w
            .prefs6
            .dialog
            .as_ref()
            .and_then(|d| d.hits.iter().find(|h| h.1 == "fsEscape").map(|h| h.0));
        click(&mut w, row.expect("escape row"));
        w.handle(
            Input::Key(Key::Enter, crate::Mods::default()),
            &crate::reader_tests::Fixed,
        );
        assert_eq!(w.full_screen_prefs6(), (true, true));
        assert!(w
            .take_requests()
            .iter()
            .any(|r| matches!(r, ReaderRequest::SavePreferences(t) if t.contains("fsEscape=1"))));
    }

    #[test]
    fn saved_measurements_use_the_preferred_unit() {
        let doc = bo_pdfread::PdfDocument::open(
            crate::reader_tests::pdf(1),
            bo_pdfread::standard_fonts(),
        )
        .expect("pdf");
        let mut w = ReaderWindow::new("en-US");
        w.open(doc, "t.pdf", None);
        // Centimetres: 144 pt = 5.08 cm, in the label, the contents and the /Measure.
        w.prefs6.prefs.units = 2;
        let shape = bo_pdfread::Shape::Distance([60.0, 100.0], [204.0, 100.0]);
        assert_eq!(w.unit_label(&shape).as_deref(), Some("5.08 cm"));
        w.add_shape(0, &shape, "");
        let doc = w.view().expect("view").doc();
        let a = doc.annotations().pop().expect("annot");
        assert_eq!(a.contents, "5.08 cm");
        assert_eq!(doc.measure_unit(&a).label, "cm");
    }
}
