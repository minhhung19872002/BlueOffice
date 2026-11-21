//! BlueReader round 15 (a child of `reader_window`): keyboard behaviour measured on the
//! reference reader with posted key messages on a scratch copy (`docs/ui/REPORT-reader.md`).
//!
//! - Page Down / Page Up / Space scroll the view height less 41 px (125 %), like the reference.
//! - Alt+← / Alt+→ (and the status bar's Previous / Next View) step through the views the
//!   document was shown in: every key, zoom or click that moved the view records the view it
//!   left (page position, zoom and fit), per document tab.
//! - Ctrl+3 Fit Visible; F4 hides / shows the navigation column; Ctrl+D opens File ›
//!   Properties; Ctrl+K opens Preferences; Ctrl+G edits the page box.
//! - Ctrl+F opens a small Search box at the top right of the document (title, find field with
//!   an options button, Previous / Next) that marks only the current match, in the
//!   reference's light blue (176, 206, 225 on white).
use super::extra::Mode;
use super::{circle, inside, rounded, ReaderWindow, MUTED, TEXT, WHITE};
use crate::reader_view::Fit;
use crate::{Input, Key, Mods, Painter, TextMeasure};
use bo_render::Rgba;
use std::collections::HashMap;

/// Title strip of the Search box (measured 235, 246, 249).
const FIND_TITLE: Rgba = Rgba(235, 246, 249, 255);
/// Window edge of the Search box (a translucent hairline: 174 over white, darker over grey).
const FIND_EDGE: Rgba = Rgba(0, 0, 0, 80);
/// Focused field edge (measured 0, 120, 215) and the idle one.
const FIELD_FOCUS: Rgba = Rgba(0, 120, 215, 255);
const FIELD_EDGE: Rgba = Rgba(122, 122, 122, 255);
/// Push buttons (measured fill 230, edge 173).
const BUTTON: Rgba = Rgba(230, 230, 230, 255);
const BUTTON_HOVER: Rgba = Rgba(229, 241, 251, 255);
const BUTTON_EDGE: Rgba = Rgba(173, 173, 173, 255);
const DISABLED: Rgba = Rgba(160, 160, 160, 255);
/// The current match of the Search box: over white it gives the reference's (176, 206, 225),
/// over black text (10, 40, 59).
pub(crate) const FIND_CURRENT: Rgba = Rgba(29, 115, 169, 89);
/// The page box while its number is edited: selected text, edge, inner ring, drop-down part.
const EDIT_SELECTION: Rgba = Rgba(0, 120, 212, 255);
const EDIT_EDGE: Rgba = Rgba(100, 100, 100, 255);
const EDIT_RING: Rgba = Rgba(251, 251, 253, 255);
const EDIT_ARROW: Rgba = Rgba(230, 233, 240, 255);

/// A view of a document: what Alt+← brings back.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ViewState {
    page: usize,
    /// Position inside the page (fraction of its height) and horizontal scroll.
    frac: f32,
    sx: f32,
    zoom: f32,
    fit: Fit,
}

/// Back and forward views of one document.
#[derive(Debug, Default)]
struct History {
    back: Vec<ViewState>,
    forward: Vec<ViewState>,
}

/// The Search box (Ctrl+F).
#[derive(Debug, Default)]
pub(crate) struct FindBox {
    pub(crate) query: String,
    pub(crate) focus: bool,
    /// The box and its parts of the last layout (window pixels).
    rect: [f32; 4],
    close: [f32; 4],
    field: [f32; 4],
    prev: [f32; 4],
    next: [f32; 4],
    hover: Option<u8>,
    /// The query the matches were found for.
    found: Option<String>,
}

impl FindBox {
    /// The find field (window pixels), for assistive technology.
    pub(crate) fn field_rect(&self) -> [f32; 4] {
        self.field
    }
}

/// Round 15 state of the window.
#[derive(Debug, Default)]
pub(crate) struct R15 {
    history: HashMap<usize, History>,
    /// Navigation column hidden (F4).
    pub(crate) nav_hidden: bool,
    /// Page box being edited: typed text and whether it is all selected.
    pub(crate) page_edit: Option<(String, bool)>,
    pub(crate) find: Option<FindBox>,
    /// The hits shown come from the Search box (only the current one is marked).
    pub(crate) find_hits: bool,
    /// The last input moved through the history itself (not recorded).
    stepped: bool,
}

/// Cap of the back list (the reference keeps a long list; this keeps memory bounded).
const HISTORY_MAX: usize = 200;

impl ReaderWindow {
    fn px15(&self, v: f32) -> f32 {
        (v * self.scale).round()
    }

    /// The view of the active document now, with its tab's key.
    pub(crate) fn snap15(&self) -> Option<(usize, ViewState)> {
        if self.show_start {
            return None;
        }
        let t = self.tabs.get(self.active)?;
        let v = &t.view;
        let page = v.current_page();
        let ((sx, sy), _) = v.scroll_state();
        let r = v.page_rect(page)?;
        // Page rect in content pixels: window rect less the frame plus the scroll.
        let top = r[1] - v.frame()[1] + sy;
        let frac = if r[3] > 0.0 { (sy - top) / r[3] } else { 0.0 };
        Some((
            t.key_base,
            ViewState {
                page,
                frac,
                sx,
                zoom: v.zoom(),
                fit: v.fit(),
            },
        ))
    }

    /// After an input: record the view it left when it moved the view.
    pub(crate) fn after15(&mut self, before: Option<(usize, ViewState)>, nav: bool) {
        if std::mem::take(&mut self.r15.stepped) {
            return;
        }
        let (Some((key, old)), true) = (before, nav) else {
            return;
        };
        let Some((key_now, new)) = self.snap15() else {
            return;
        };
        if key_now != key || same_view(&old, &new) {
            return;
        }
        let h = self.r15.history.entry(key).or_default();
        h.back.push(old);
        if h.back.len() > HISTORY_MAX {
            h.back.remove(0);
        }
        h.forward.clear();
    }

    /// Whether an input may move the view and is recorded (not the history keys themselves).
    pub(crate) fn nav_input15(input: &Input) -> bool {
        match input {
            Input::Key(Key::Left | Key::Right, m) if m.alt => false,
            Input::Key(..) | Input::Zoom(_) | Input::Press { .. } => true,
            Input::Text(s) => s == " ",
            _ => false,
        }
    }

    /// Previous (`back`) or next view of the active document; `false` when there is none.
    pub(crate) fn step_view15(&mut self, back: bool) -> bool {
        let Some((key, now)) = self.snap15() else {
            return false;
        };
        let Some(h) = self.r15.history.get_mut(&key) else {
            return false;
        };
        let (from, to) = if back {
            (&mut h.back, &mut h.forward)
        } else {
            (&mut h.forward, &mut h.back)
        };
        let Some(v) = from.pop() else {
            return false;
        };
        to.push(now);
        self.r15.stepped = true;
        if let Some(t) = self.tabs.get_mut(self.active) {
            let view = &mut t.view;
            if v.fit == Fit::None {
                if (view.zoom() - v.zoom).abs() > 1e-4 || view.fit() != Fit::None {
                    view.set_zoom(v.zoom);
                }
            } else if view.fit() != v.fit {
                view.set_fit(v.fit);
            }
            view.go_to(v.page, None);
            if let Some(r) = view.page_rect(v.page) {
                let ((_, sy), _) = view.scroll_state();
                let top = r[1] - view.frame()[1] + sy;
                view.set_scroll((v.sx, top + v.frac * r[3]));
            }
        }
        true
    }

    /// Whether the status bar's Previous / Next View has a view to go to.
    pub(crate) fn has_view15(&self, back: bool) -> bool {
        let Some(t) = self.tabs.get(self.active) else {
            return false;
        };
        self.r15.history.get(&t.key_base).is_some_and(|h| {
            if back {
                !h.back.is_empty()
            } else {
                !h.forward.is_empty()
            }
        })
    }

    /// Forget the views of a closed document.
    pub(crate) fn forget15(&mut self, key_base: usize) {
        self.r15.history.remove(&key_base);
    }

    /// Page Down / Page Up step: the view height less 41 px at 125 % (measured: 773 of 814).
    pub(crate) fn page_step15(&self) -> f32 {
        let h = self.view().map_or(0.0, |v| v.frame()[3]);
        (h - self.px15(32.8)).max(self.px15(20.0))
    }

    // ---- Keys --------------------------------------------------------------------------------

    /// Round 15 keys of the document window; `Some(redraw)` when taken.
    pub(crate) fn key15(&mut self, k: Key, mods: Mods, m: &dyn TextMeasure) -> Option<bool> {
        if self.extra.mode == Mode::Full {
            return None;
        }
        match (k, mods.ctrl, mods.alt) {
            (Key::Left, false, true) => Some(self.step_view15(true)),
            (Key::Right, false, true) => Some(self.step_view15(false)),
            (Key::PageDown | Key::PageUp, false, false) => {
                let d = self.page_step15();
                let t = self.tabs.get_mut(self.active)?;
                t.view
                    .scroll_by(0.0, if k == Key::PageDown { d } else { -d });
                Some(true)
            }
            (Key::Letter('3'), true, false) => {
                let t = self.tabs.get_mut(self.active)?;
                t.view.fit_visible();
                Some(true)
            }
            (Key::F(4), false, false) if self.extra.mode == Mode::Normal => {
                self.r15.nav_hidden = !self.r15.nav_hidden;
                self.relayout(m);
                Some(true)
            }
            (Key::Letter('d'), true, false) if !mods.shift && !self.tabs.is_empty() => {
                if self.file_backstage {
                    self.requests
                        .push(super::ReaderRequest::Command("file.backstage".into()));
                } else {
                    self.open_reader_backstage();
                    if let Some(b) = self.tabs5.backstage.as_mut() {
                        b.page = "bs.properties";
                    }
                }
                Some(true)
            }
            (Key::Letter('k'), true, false) if !mods.shift => {
                self.run_command("bs.prefs", m);
                Some(true)
            }
            (Key::Letter('g'), true, false) if !mods.shift && self.view().is_some() => {
                let label = self.page_label15();
                self.r15.page_edit = Some((label, true));
                self.search_focus = false;
                Some(true)
            }
            (Key::Letter('f'), true, false) if !mods.shift && self.profile.is_none() => {
                self.open_find15();
                Some(true)
            }
            _ => None,
        }
    }

    /// Space (and the reference's Shift+Space, which arrives as text too) pages down.
    pub(crate) fn space15(&mut self) -> bool {
        if self.extra.mode == Mode::Full {
            return false;
        }
        let d = self.page_step15();
        match self.tabs.get_mut(self.active) {
            Some(t) => {
                t.view.scroll_by(0.0, d);
                true
            }
            None => false,
        }
    }

    /// The page box's text ("1 / 12"), all selected when Ctrl+G starts editing it.
    fn page_label15(&self) -> String {
        self.view()
            .map(|v| super::page_label(v.doc(), v.current_page(), v.page_count()))
            .unwrap_or_default()
    }

    // ---- Page box editing ------------------------------------------------------------------

    /// Input while the page box is edited; `Some(redraw)` when taken.
    fn route_page_edit15(&mut self, input: &Input) -> Option<bool> {
        let (text, all) = self.r15.page_edit.clone()?;
        match input {
            Input::Text(s) => {
                let digits: String = s.chars().filter(char::is_ascii_digit).collect();
                let mut t = if all { String::new() } else { text };
                t.push_str(&digits);
                self.r15.page_edit = Some((t, false));
                Some(true)
            }
            Input::Key(Key::Backspace, _) => {
                let mut t = if all { String::new() } else { text };
                t.pop();
                self.r15.page_edit = Some((t, false));
                Some(true)
            }
            Input::Key(Key::Enter, _) => {
                self.r15.page_edit = None;
                // "7", or the untouched "1 / 12" (its first number).
                let number = text.split('/').next().unwrap_or("").trim().to_owned();
                if let (Ok(n), Some(t)) = (number.parse::<usize>(), self.tabs.get_mut(self.active))
                {
                    let count = t.view.page_count();
                    if n >= 1 && count > 0 {
                        t.view.go_to(n.min(count) - 1, None);
                    }
                }
                Some(true)
            }
            Input::Key(Key::Escape, _) => {
                self.r15.page_edit = None;
                Some(true)
            }
            Input::Key(..) => Some(true),
            Input::Press { x, y, .. } => {
                self.r15.page_edit = None;
                // A click elsewhere ends the edit and goes on as usual.
                let _ = (x, y);
                None
            }
            _ => None,
        }
    }

    // ---- Search box -------------------------------------------------------------------------

    fn open_find15(&mut self) {
        self.search_focus = false;
        let f = self.r15.find.get_or_insert_with(FindBox::default);
        f.focus = true;
        self.layout_find15();
    }

    fn close_find15(&mut self) {
        self.r15.find = None;
        if self.r15.find_hits {
            self.r15.find_hits = false;
            if let Some(t) = self.tabs.get_mut(self.active) {
                t.view.clear_hits();
            }
        }
    }

    /// Place the Search box: at the top right of the document, left of the scroll bar
    /// (measured 304 × 123 px, 4 px from the bar, at the document's top).
    pub(crate) fn layout_find15(&mut self) {
        let d = self.doc_rect;
        // Measured in a 1500 × 1102 window: 4 px before the drawn scroll bar, top at 245 px
        // (the document tab row's foot).
        let right = self.vbar[0].max(d[0] + 1.0) - self.px15(3.2);
        let (w, h) = (self.px15(243.2), self.px15(98.4));
        let x = (right - w).max(d[0]);
        let y = d[1];
        let px = |v: f32| (v * self.scale).round();
        let Some(f) = self.r15.find.as_mut() else {
            return;
        };
        f.rect = [x, y, w, h];
        f.close = [x + w - px(28.8), y, px(28.8), px(25.6)];
        f.field = [x + px(12.8), y + px(36.0), px(220.0), px(22.4)];
        f.prev = [x + px(12.8), y + px(64.0), px(107.2), px(24.0)];
        f.next = [x + px(125.6), y + px(64.0), px(107.2), px(24.0)];
    }

    /// Run the Search box's query forwards or backwards.
    fn find_go15(&mut self, forward: bool) {
        let Some(f) = self.r15.find.as_mut() else {
            return;
        };
        let q = f.query.clone();
        if q.is_empty() {
            return;
        }
        let again = f.found.as_deref() == Some(q.as_str());
        f.found = Some(q.clone());
        let (case, word) = self.find_options8();
        let Some(t) = self.tabs.get_mut(self.active) else {
            return;
        };
        self.r15.find_hits = true;
        if again && !t.view.hits().is_empty() {
            t.view.next_hit(forward);
        } else {
            t.view.find_start(&q, case, word);
            if !forward && t.view.current_hit().is_some() {
                t.view.next_hit(false);
            }
        }
    }

    /// Input for the Search box; `Some(redraw)` when it took it.
    fn route_find15(&mut self, input: &Input) -> Option<bool> {
        let f = self.r15.find.as_ref()?;
        let (focus, rect) = (f.focus, f.rect);
        match input {
            Input::Text(s) if focus => {
                if let Some(f) = self.r15.find.as_mut() {
                    f.query.push_str(s);
                }
                Some(true)
            }
            Input::Key(k, mods) if focus => {
                match k {
                    Key::Backspace => {
                        if let Some(f) = self.r15.find.as_mut() {
                            f.query.pop();
                        }
                    }
                    Key::Enter | Key::F(3) => self.find_go15(!mods.shift),
                    Key::Escape => self.close_find15(),
                    Key::Letter('f') if mods.ctrl => {}
                    _ => return None,
                }
                Some(true)
            }
            Input::Key(Key::F(3), mods) => {
                self.find_go15(!mods.shift);
                Some(true)
            }
            Input::Move { x, y } => {
                let f = self.r15.find.as_mut()?;
                let h = if inside(f.close, *x, *y) {
                    Some(0)
                } else if inside(f.prev, *x, *y) {
                    Some(1)
                } else if inside(f.next, *x, *y) {
                    Some(2)
                } else {
                    None
                };
                let changed = h != f.hover;
                f.hover = h;
                inside(rect, *x, *y).then_some(changed)
            }
            Input::Press { x, y, .. } => {
                if !inside(rect, *x, *y) {
                    if let Some(f) = self.r15.find.as_mut() {
                        f.focus = false;
                    }
                    return None;
                }
                let f = self.r15.find.as_mut()?;
                let (close, prev, next) = (f.close, f.prev, f.next);
                f.focus = true;
                if inside(close, *x, *y) {
                    self.close_find15();
                } else if inside(prev, *x, *y) {
                    self.find_go15(false);
                } else if inside(next, *x, *y) {
                    self.find_go15(true);
                }
                Some(true)
            }
            Input::Release { x, y } | Input::Drag { x, y } if inside(rect, *x, *y) => Some(false),
            _ => None,
        }
    }

    /// Round 15 input that comes before the other handlers; `Some(redraw)` when taken.
    pub(crate) fn route15(&mut self, input: &Input) -> Option<bool> {
        if self.menu.is_some() || self.dialog.is_some() || self.extra.print.is_some() {
            return None;
        }
        if let Some(r) = self.route_page_edit15(input) {
            return Some(r);
        }
        if self.r15.find.is_some() {
            if matches!(input, Input::Resize { .. } | Input::Scale(_)) {
                return None;
            }
            if let Some(r) = self.route_find15(input) {
                return Some(r);
            }
        }
        None
    }

    /// Colour of a search match on the page, or `None` when it is not marked.
    pub(crate) fn hit_colour15(&self, current: bool) -> Option<Rgba> {
        match (self.r15.find_hits, current) {
            (true, true) => Some(FIND_CURRENT),
            (true, false) => None,
            (false, true) => Some(crate::reader_view::HIT_CURRENT),
            (false, false) => Some(crate::reader_view::HIT),
        }
    }

    /// Hide the navigation column (F4) after the normal layout of the document area.
    pub(crate) fn layout15(&mut self) {
        if !self.r15.nav_hidden || self.extra.mode != Mode::Normal {
            if self.r15.find.is_some() {
                self.layout_find15();
            }
            return;
        }
        let x = self.nav_rect[0];
        let right = self.doc_rect[0] + self.doc_rect[2];
        self.nav_rect = [0.0; 4];
        self.panel_rect = [0.0; 4];
        self.strip_rect = [0.0; 4];
        self.doc_rect[0] = x;
        self.doc_rect[2] = (right - x).max(1.0);
        self.controls
            .retain(|c| !matches!(c.kind, super::Kind::Nav(_)));
        if self.r15.find.is_some() {
            self.layout_find15();
        }
    }

    // ---- Paint -------------------------------------------------------------------------------

    pub(crate) fn paint15(&self, p: &mut dyn Painter) {
        self.paint_page_edit15(p);
        self.paint_find15(p);
    }

    fn paint_page_edit15(&self, p: &mut dyn Painter) {
        let Some((text, all)) = &self.r15.page_edit else {
            return;
        };
        let b = self.page_box;
        if b[2] <= 0.0 {
            return;
        }
        let st = self.style(self.look.status_font, false);
        // Measured on the reference while its page box is edited: a 100 grey edge, a 2 px ring
        // of the chrome colour, the text selected, the drop-down part on (230, 233, 240).
        p.fill_rect(b[0], b[1], b[2], b[3], EDIT_EDGE);
        p.fill_rect(b[0] + 1.0, b[1] + 1.0, b[2] - 2.0, b[3] - 2.0, EDIT_RING);
        let arrow_w = self.px15(12.0);
        p.fill_rect(
            b[0] + 3.0,
            b[1] + 3.0,
            b[2] - 6.0 - arrow_w,
            b[3] - 6.0,
            WHITE,
        );
        p.fill_rect(
            b[0] + b[2] - 2.0 - arrow_w,
            b[1] + 1.0,
            arrow_w,
            b[3] - 2.0,
            EDIT_ARROW,
        );
        super::tri(
            p,
            b[0] + b[2] - self.px15(10.4),
            b[1] + b[3] / 2.0 - self.px15(1.6),
            self.px15(3.2),
            TEXT,
        );
        let ty = b[1] + ((b[3] - st.size * 1.33) / 2.0).round();
        let tx = b[0] + self.px15(4.0);
        let tw = self.width_of(text, st);
        if *all && !text.is_empty() {
            p.fill_rect(tx - 1.0, ty, tw + 2.0, st.size * 1.33, EDIT_SELECTION);
            p.text(text, tx, ty, st, WHITE);
        } else {
            p.text(text, tx, ty, st, TEXT);
            p.fill_rect((tx + tw).round(), ty, 1.0, st.size * 1.33, TEXT);
        }
    }

    fn paint_find15(&self, p: &mut dyn Painter) {
        let Some(f) = &self.r15.find else {
            return;
        };
        let r = f.rect;
        if r[2] <= 0.0 {
            return;
        }
        // A Windows 11 pop-up like the reference's (measured on screen): 8 px round corners, a
        // grey hairline (174 on the left, 153 at the foot over white), a soft shadow that
        // reaches about 15 px at the sides and 30 px below.
        let rad = self.px15(6.4);
        for k in (1..=16).rev() {
            let e = k as f32;
            p.path(
                &rounded(
                    r[0] - e * 0.9,
                    r[1] - e * 0.5 + e * 0.8,
                    r[2] + e * 1.8,
                    r[3] + e,
                    rad + e,
                ),
                Some(Rgba(0, 0, 0, 5)),
                None,
            );
        }
        p.path(
            &rounded(r[0] - 1.0, r[1] - 1.0, r[2] + 2.0, r[3] + 2.0, rad + 1.0),
            Some(FIND_EDGE),
            None,
        );
        p.path(&rounded(r[0], r[1], r[2], r[3], rad), Some(WHITE), None);
        let th = self.px15(25.6);
        p.path(
            &rounded(r[0], r[1], r[2], th + rad, rad),
            Some(FIND_TITLE),
            None,
        );
        p.fill_rect(r[0], r[1] + th, r[2], rad, WHITE);
        let st = self.style(15.0, false);
        let line = st.size * 1.33;
        p.text(
            &self.t("pdfReader15.find"),
            r[0] + self.px15(6.4),
            r[1] + ((th - line) / 2.0).round(),
            st,
            TEXT,
        );
        // Close.
        let c = f.close;
        if f.hover == Some(0) {
            p.fill_rect(c[0], c[1], c[2], c[3], Rgba(218, 226, 230, 255));
        }
        let cs = self.px15(12.0);
        crate::reader_icons::draw(
            p,
            "close",
            c[0] + ((c[2] - cs) / 2.0).round(),
            c[1] + ((c[3] - cs) / 2.0).round(),
            cs,
            crate::reader_icons::Ink {
                ink: TEXT,
                accent: TEXT,
            },
        );
        // Field with the options button.
        let fd = f.field;
        p.fill_rect(
            fd[0],
            fd[1],
            fd[2],
            fd[3],
            if f.focus { FIELD_FOCUS } else { FIELD_EDGE },
        );
        p.fill_rect(fd[0] + 1.0, fd[1] + 1.0, fd[2] - 2.0, fd[3] - 2.0, WHITE);
        let ty = fd[1] + ((fd[3] - line) / 2.0).round();
        let tx = fd[0] + self.px15(4.0);
        if f.query.is_empty() {
            p.text(&self.t("pdfReader15.findHint"), tx, ty, st, MUTED);
            if f.focus {
                p.fill_rect(tx, ty, 1.0, line, TEXT);
            }
        } else {
            p.text(&f.query, tx, ty, st, TEXT);
            if f.focus {
                let cx = tx + self.width_of(&f.query, st);
                p.fill_rect(cx.round(), ty, 1.0, line, TEXT);
            }
        }
        let gs = self.px15(16.0);
        gear(
            p,
            fd[0] + fd[2] - self.px15(4.0) - gs / 2.0,
            fd[1] + fd[3] / 2.0,
            gs / 2.0,
        );
        // Previous / Next: grey labels until there is a query.
        let enabled = !f.query.is_empty();
        for (i, (b, key)) in [
            (f.prev, "pdfReader15.previous"),
            (f.next, "pdfReader15.next"),
        ]
        .into_iter()
        .enumerate()
        {
            let hover = enabled && f.hover == Some(i as u8 + 1);
            p.fill_rect(b[0], b[1], b[2], b[3], BUTTON_EDGE);
            p.fill_rect(
                b[0] + 1.0,
                b[1] + 1.0,
                b[2] - 2.0,
                b[3] - 2.0,
                if hover { BUTTON_HOVER } else { BUTTON },
            );
            let label = self.t(key);
            let lw = self.width_of(&label, st);
            p.text(
                &label,
                (b[0] + (b[2] - lw) / 2.0).round(),
                b[1] + ((b[3] - line) / 2.0).round(),
                st,
                if enabled { TEXT } else { DISABLED },
            );
        }
    }

    /// Measure the Search box's strings (widths for centring and the caret).
    pub(crate) fn measure15(&mut self, m: &dyn TextMeasure) {
        let st = self.style(15.0, false);
        let mut words = vec![self.t("pdfReader15.previous"), self.t("pdfReader15.next")];
        if let Some(f) = &self.r15.find {
            words.push(f.query.clone());
        }
        if let Some((t, _)) = &self.r15.page_edit {
            words.push(t.clone());
        }
        for w in words {
            self.measure(m, &w, st);
        }
        let sst = self.style(self.look.status_font, false);
        if let Some((t, _)) = self.r15.page_edit.clone() {
            self.measure(m, &t, sst);
        }
    }
}

/// Same document position (within half a device pixel) and zoom.
fn same_view(a: &ViewState, b: &ViewState) -> bool {
    a.page == b.page
        && (a.frac - b.frac).abs() < 1e-4
        && (a.sx - b.sx).abs() < 0.5
        && (a.zoom - b.zoom).abs() < 1e-4
}

/// Options button of the Search box: our own cog (ring with eight teeth and a hub).
fn gear(p: &mut dyn Painter, cx: f32, cy: f32, r: f32) {
    let ink = Rgba(96, 96, 98, 255);
    let teeth = 8;
    let mut segs = Vec::new();
    for i in 0..teeth * 2 {
        let a0 = i as f32 * std::f32::consts::PI / teeth as f32;
        let a1 = a0 + std::f32::consts::PI / teeth as f32;
        let rr = if i % 2 == 0 { r } else { r * 0.78 };
        let (x0, y0) = (cx + rr * a0.cos(), cy + rr * a0.sin());
        let (x1, y1) = (cx + rr * a1.cos(), cy + rr * a1.sin());
        if i == 0 {
            segs.push(bo_render::PathSeg::M(x0, y0));
        } else {
            segs.push(bo_render::PathSeg::L(x0, y0));
        }
        segs.push(bo_render::PathSeg::L(x1, y1));
    }
    segs.push(bo_render::PathSeg::Z);
    p.path(&segs, None, Some((ink, 1.0)));
    p.path(&circle(cx, cy, r * 0.32), None, Some((ink, 1.0)));
}

#[cfg(test)]
#[path = "reader_r15_tests.rs"]
mod tests;
