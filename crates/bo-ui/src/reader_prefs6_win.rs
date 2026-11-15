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
use super::{inside, rounded, ReaderRequest, ReaderWindow, MUTED, TEXT, WHITE};
use crate::dialog::{Dialog, DialogButton, DialogKind};
use crate::popup::{Choice, MenuItem};
use crate::reader_view::{Fit, Layout};
use crate::{Input, Key, Painter, TextMeasure};
use bo_render::Rgba;

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

/// The preferences.
#[derive(Debug, Clone, PartialEq, Default)]
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

/// Categories of the dialog: label key and the rows on it.
const PAGES: [(&str, &[&str]); 4] = [
    ("pdfPrefs.pageDisplay", &["zoom", "layout"]),
    ("pdfPrefs.identity", &["author"]),
    ("pdfPrefs.measure", &["units"]),
    ("pdfPrefs.general", &["smooth"]),
];

impl ReaderWindow {
    /// Preferences stored by the host (`key=value` lines).
    pub fn set_preferences(&mut self, text: &str) {
        self.prefs6.prefs = Prefs::from_text(text);
        self.apply_author();
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
        let (_, u, k) = UNITS[self.prefs6.prefs.units];
        let k = if self.profile.is_none() { k } else { 72.0 };
        let u = if self.profile.is_none() { u } else { "in" };
        Some(match shape {
            bo_pdfread::Shape::Distance(a, b) => {
                format!("{:.2} {u}", bo_pdfread::path_length(&[*a, *b]) / k)
            }
            bo_pdfread::Shape::Perimeter(p) => {
                format!("{:.2} {u}", bo_pdfread::path_length(p) / k)
            }
            bo_pdfread::Shape::Area(p) => {
                format!("{:.2} sq {u}", bo_pdfread::polygon_area(p) / (k * k))
            }
            _ => return None,
        })
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
                    _ => {}
                }
            }
            return true;
        }
        match cmd {
            "bs.prefs" => {
                self.prefs6.dialog = Some(PrefsDialog {
                    edit: self.prefs6.prefs.clone(),
                    page: 0,
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
                } else if matches!(id.as_str(), "zoom" | "layout" | "units") {
                    self.pick(&id, r);
                } else if id == "smooth" {
                    if let Some(d) = self.prefs6.dialog.as_mut() {
                        d.edit.smooth = !d.edit.smooth;
                    }
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
            let (w, h) = self.size;
            p.fill_rect(0.0, 0.0, w, h, Rgba(0, 0, 0, 40));
            let (dw, dh) = (self.px(560.0), self.px(360.0));
            let r = [((w - dw) / 2.0).round(), ((h - dh) / 2.0).round(), dw, dh];
            d.rect = r;
            d.hits.clear();
            p.fill_rect(
                r[0] - 1.0,
                r[1] - 1.0,
                r[2] + 2.0,
                r[3] + 2.0,
                self.look.edge,
            );
            p.fill_rect(r[0], r[1], r[2], r[3], WHITE);
            let title = self.style(15.0, false);
            let body = self.style(13.0, false);
            let line = body.size * 1.33;
            p.text(
                &self.t("pdfPrefs.title"),
                r[0] + self.px(16.0),
                r[1] + self.px(10.0),
                title,
                TEXT,
            );
            // Categories.
            let lx = r[0] + self.px(12.0);
            let lw = self.px(150.0);
            let top = r[1] + self.px(44.0);
            p.fill_rect(lx, top, lw, r[3] - self.px(100.0), Rgba(245, 246, 250, 255));
            for (k, (key, _)) in PAGES.iter().enumerate() {
                let row = [lx, top + k as f32 * self.px(28.0), lw, self.px(28.0)];
                if k == d.page {
                    p.fill_rect(row[0], row[1], row[2], row[3], Rgba(230, 233, 240, 255));
                }
                p.text(
                    &self.t(key),
                    row[0] + self.px(10.0),
                    row[1] + ((row[3] - line) / 2.0).round(),
                    body,
                    TEXT,
                );
                d.hits.push((row, format!("page.{k}")));
            }
            // Rows of the page.
            let cx = lx + lw + self.px(20.0);
            let vx = cx + self.px(140.0);
            let vw = r[0] + r[2] - vx - self.px(20.0);
            let mut y = top + self.px(4.0);
            for id in PAGES[d.page].1 {
                let (label, value) = match *id {
                    "zoom" => (
                        self.t("pdfPrefs.zoom"),
                        self.zoom_label(ZOOMS[d.edit.zoom].0, ZOOMS[d.edit.zoom].1),
                    ),
                    "layout" => (self.t("pdfPrefs.layout"), self.t(LAYOUTS[d.edit.layout].0)),
                    "author" => (
                        self.t("pdfPrefs.author"),
                        if d.edit.author.is_empty() {
                            std::env::var("USERNAME").unwrap_or_default()
                        } else {
                            d.edit.author.clone()
                        },
                    ),
                    "units" => (self.t("pdfPrefs.units"), self.t(UNITS[d.edit.units].0)),
                    _ => (self.t("pdfPrefs.smooth"), String::new()),
                };
                let bh = self.px(26.0);
                if *id == "smooth" {
                    let b = self.px(14.0);
                    let bx = [cx, y + (bh - b) / 2.0, b, b];
                    p.fill_rect(bx[0], bx[1], b, b, Rgba(120, 120, 120, 255));
                    p.fill_rect(bx[0] + 1.0, bx[1] + 1.0, b - 2.0, b - 2.0, WHITE);
                    if d.edit.smooth {
                        let k = b / 4.0;
                        p.path(
                            &[
                                bo_render::PathSeg::M(bx[0] + k, bx[1] + 2.0 * k),
                                bo_render::PathSeg::L(bx[0] + 1.8 * k, bx[1] + 3.0 * k),
                                bo_render::PathSeg::L(bx[0] + 3.2 * k, bx[1] + k),
                            ],
                            None,
                            Some((TEXT, self.px(1.6))),
                        );
                    }
                    p.text(
                        &label,
                        cx + b + self.px(8.0),
                        y + ((bh - line) / 2.0).round(),
                        body,
                        TEXT,
                    );
                    d.hits.push(([cx, y, vw + vx - cx, bh], (*id).to_owned()));
                } else {
                    p.text(&label, cx, y + ((bh - line) / 2.0).round(), body, TEXT);
                    let field = [vx, y, vw, bh];
                    p.fill_rect(
                        field[0],
                        field[1],
                        field[2],
                        field[3],
                        Rgba(190, 190, 190, 255),
                    );
                    p.fill_rect(
                        field[0] + 1.0,
                        field[1] + 1.0,
                        field[2] - 2.0,
                        field[3] - 2.0,
                        WHITE,
                    );
                    p.text(
                        &value,
                        field[0] + self.px(8.0),
                        y + ((bh - line) / 2.0).round(),
                        body,
                        TEXT,
                    );
                    if *id != "author" {
                        // Drop-down arrow.
                        let (ax, ay) = (field[0] + field[2] - self.px(14.0), y + bh / 2.0);
                        let k = self.px(3.5);
                        p.path(
                            &[
                                bo_render::PathSeg::M(ax - k, ay - k / 2.0),
                                bo_render::PathSeg::L(ax + k, ay - k / 2.0),
                                bo_render::PathSeg::L(ax, ay + k / 2.0),
                                bo_render::PathSeg::Z,
                            ],
                            Some(MUTED),
                            None,
                        );
                    }
                    d.hits.push((field, (*id).to_owned()));
                }
                y += self.px(38.0);
            }
            // OK / Cancel.
            let (bw, bh) = (self.px(80.0), self.px(28.0));
            let by = r[1] + r[3] - bh - self.px(14.0);
            for (k, (key, id)) in [("pdfReader.ok", "ok"), ("pdfPrint.cancel", "cancel")]
                .iter()
                .enumerate()
            {
                let bx = r[0] + r[2] - (2.0 - k as f32) * (bw + self.px(10.0));
                let accent = k == 0;
                p.path(
                    &rounded(bx, by, bw, bh, self.px(3.0)),
                    Some(if accent {
                        self.theme.accent
                    } else {
                        Rgba(230, 233, 240, 255)
                    }),
                    None,
                );
                let label = self.t(key);
                let lw = label.chars().count() as f32 * body.size * 0.55;
                p.text(
                    &label,
                    (bx + (bw - lw) / 2.0).round(),
                    by + ((bh - line) / 2.0).round(),
                    body,
                    if accent { WHITE } else { TEXT },
                );
                d.hits.push(([bx, by, bw, bh], (*id).to_owned()));
            }
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
        };
        let t = p.to_text();
        assert!(t.contains("zoom=width") && t.contains("units=mm"));
        assert_eq!(Prefs::from_text(&t), p);
        assert_eq!(Prefs::from_text("zoom=nonsense\nfoo=1"), Prefs::default());
    }
}
