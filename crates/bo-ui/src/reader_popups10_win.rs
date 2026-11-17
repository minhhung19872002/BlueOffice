//! BlueReader round 10 (a child of `reader_window`): pop-up notes drawn on the page like the
//! reference reader's (open `/Popup` annotations, ISO 32000-1 §12.5.6.14: a header in the
//! comment's colour with its icon, date and close box, the author and Options, the text, resize
//! grips; moved by the header, resized by the grips, the text typed in place), the floating
//! toolbar shown after selecting text, and Paste on the page (clipboard text as a typewriter
//! comment, a clipboard picture as a stamp, a copied comment at the pointer or in place).
//!
//! Sizes are measured on the reference at 125 % (device pixels / 1.25); the icons are ours.
use super::{inside, Drag, ReaderWindow, Tool};
use crate::reader_icons::{self, Ink};
use crate::{Font, Input, Key, Painter, TextMeasure};
use bo_pdfread::{Annot, AnnotKind, PopupNote};
use bo_render::{PathSeg, Rgba};
use std::collections::HashMap;

// Handles of the selected comment (round 10).
#[path = "reader_handles10_win.rs"]
mod handles;

/// Header rows (date, author) and where the text area starts.
const ROW_H: f32 = 20.0;
const BODY_TOP: f32 = 40.8;
/// Text area inset from the frame at the sides and above the bottom edge.
const BODY_SIDE: f32 = 2.4;
const BODY_BOTTOM: f32 = 13.6;
/// Close box size and inset from the frame.
const CLOSE: f32 = 14.4;
const CLOSE_INSET: f32 = 1.6;
/// Header text (Arial 17 device px at 125 %) and its box tops.
const HEAD_FONT: f32 = 13.6;
const DATE_Y: f32 = 2.2;
const AUTHOR_Y: f32 = 22.2;
/// Note text: size, line pitch, inset in the text area.
const BODY_FONT: f32 = 9.0;
const BODY_LINE: f32 = 10.4;
const BODY_PAD: f32 = 2.4;
const BODY_TEXT_Y: f32 = 3.9;
/// Smallest pop-up note (page points).
const MIN_W: f32 = 90.0;
const MIN_H: f32 = 60.0;
const FRAME: Rgba = Rgba(133, 133, 133, 255);
const FRAME_ACTIVE: Rgba = Rgba(80, 80, 80, 255);
const INK: Rgba = Rgba(0, 0, 0, 255);
const FAMILY: &str = "Arial";

/// The floating toolbar after a text selection: padding, icon, step, the separator's extra
/// space, height and the gap above the pointer (25 / 20 / 35 / 8 / 52 / 2 device px).
const BAR_PAD: f32 = 20.0;
const BAR_ICON: f32 = 16.0;
const BAR_STEP: f32 = 28.0;
const BAR_SEP: f32 = 6.4;
const BAR_H: f32 = 41.6;
const BAR_GAP: f32 = 1.6;
const BAR_BG: Rgba = Rgba(251, 251, 253, 255);
const BAR_EDGE: Rgba = Rgba(199, 204, 217, 255);
const BAR_SEP_INK: Rgba = Rgba(230, 233, 240, 255);
const BAR_HOVER: Rgba = Rgba(230, 233, 240, 255);
/// Toolbar items: command, icon; a separator after the first `BAR_LEFT`.
const BAR_ITEMS: [(&str, &str); 9] = [
    ("edit.copy", "copy"),
    ("ctx.lookup", "search"),
    ("view.wordCount", "wordCount"),
    ("ten.more", "more"),
    ("sel.highlight", "highlight"),
    ("sel.strikeout", "strikeout"),
    ("sel.underline", "underline"),
    ("sel.squiggly", "squiggly"),
    ("comment.replace", "replaceText"),
];
const BAR_LEFT: usize = 4;

/// Part of a pop-up note under the pointer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Part {
    Header,
    Close,
    Options,
    Body,
    /// A resize grip: the left (true) or right bottom corner.
    Grip(bool),
}

#[derive(Debug, Clone, PartialEq)]
struct PopDrag {
    note: PopupNote,
    grip: Option<bool>,
    from: (f32, f32),
    to: (f32, f32),
}

/// State of round 10.
#[derive(Debug, Default)]
pub(crate) struct Ten {
    /// Pop-up notes of the active document and what they were read for (tab key, edits).
    notes: Vec<PopupNote>,
    read_for: Option<(usize, u32, usize)>,
    /// Wrapped note text per pop-up (its text and width when wrapped).
    lines: HashMap<(u32, u16), (String, u32, Vec<String>)>,
    /// Header text widths.
    widths: HashMap<(String, u32), f32>,
    drag: Option<PopDrag>,
    /// The note being typed in and its text.
    editing: Option<(PopupNote, String)>,
    /// Floating toolbar: the pointer where the selection ended, the item under the pointer.
    pub(crate) bar: Option<(f32, f32)>,
    bar_hover: Option<usize>,
    /// Where the page menu was opened (Paste goes there).
    pub(crate) ctx_point: Option<(f32, f32)>,
    /// A copied comment and the text put on the clipboard for it.
    clip: Option<(Annot, String)>,
    /// Paste waiting for the clipboard: page, point (page points).
    paste: Option<(usize, [f32; 2])>,
    /// When the document was read (D: date): notes without a date show it, as the reference.
    read_at: String,
    /// A handle of the selected comment being dragged, and Shift on the pointer.
    handle: Option<handles::HandleDrag>,
    shift: bool,
}

fn tint(c: [u8; 3], f: f32, base: f32) -> Rgba {
    let m = |v: u8| {
        (f32::from(v) * f + base * (1.0 - f))
            .round()
            .clamp(0.0, 255.0) as u8
    };
    Rgba(m(c[0]), m(c[1]), m(c[2]), 255)
}

/// Header colour: the comment's colour 30 % towards white (yellow → 255, 255, 77 as measured).
fn head_colour(c: [u8; 3]) -> Rgba {
    tint(c, 0.7, 255.0)
}

/// Text area: the header colour 70 % towards grey 239 (→ 244, 244, 190).
fn body_colour(h: Rgba) -> Rgba {
    tint([h.0, h.1, h.2], 0.3, 239.0)
}

/// Darker header ink for the close box and grips (→ 163, 163, 49).
fn edge_colour(h: Rgba) -> Rgba {
    tint([h.0, h.1, h.2], 0.64, 0.0)
}

/// `D:YYYYMMDDHHmmSS…` as the reference shows it: "11/16/2026 1:54:32 PM".
pub(crate) fn popup_date(raw: &str) -> String {
    let (d, t) = super::panels8::split_date(raw);
    let mut p = d.split('-');
    match (p.next(), p.next(), p.next()) {
        (Some(y), Some(m), Some(dd)) if !t.is_empty() => {
            let n = |s: &str| s.trim_start_matches('0').to_owned();
            format!("{}/{}/{y} {t}", n(m), n(dd))
        }
        _ => String::new(),
    }
}

/// The local time now as a PDF date (D:YYYYMMDDHHmmSS).
fn local_now() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
        + i64::from(super::panels8::UTC_OFFSET.load(std::sync::atomic::Ordering::Relaxed)) * 60;
    let (days, rem) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    // Civil date from days since 1970-01-01 (H. Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!(
        "D:{y:04}{m:02}{d:02}{:02}{:02}{:02}",
        rem / 3600,
        rem / 60 % 60,
        rem % 60
    )
}

fn font(size: f32) -> Font<'static> {
    Font {
        family: FAMILY,
        size,
        weight: 400,
        italic: false,
        underline: false,
    }
}

/// Wrap `text` into lines no wider than `w` (words, then characters).
fn wrap(text: &str, w: f32, m: &dyn TextMeasure, f: &Font<'_>) -> Vec<String> {
    let mut out = Vec::new();
    for para in text.split(['\n', '\r']) {
        let mut line = String::new();
        for word in para.split(' ') {
            let cand = if line.is_empty() {
                word.to_owned()
            } else {
                format!("{line} {word}")
            };
            if m.measure_font(&cand, f).0 <= w || line.is_empty() && cand.chars().count() <= 1 {
                line = cand;
                continue;
            }
            if !line.is_empty() {
                out.push(std::mem::take(&mut line));
            }
            // A word wider than the line breaks between characters.
            let mut part = String::new();
            for ch in word.chars() {
                part.push(ch);
                if m.measure_font(&part, f).0 > w && part.chars().count() > 1 {
                    part.pop();
                    out.push(std::mem::take(&mut part));
                    part.push(ch);
                }
            }
            line = part;
        }
        out.push(line);
    }
    out
}

impl ReaderWindow {
    /// BlueReader only (BlueEditor keeps its own comment handling).
    fn ten_on(&self) -> bool {
        self.profile.is_none() && !self.show_start && !self.tabs.is_empty()
    }

    /// Read the pop-up notes again when the document or the tab changed.
    fn refresh10(&mut self) {
        let Some(t) = self.tabs.get(self.active) else {
            self.ten.notes.clear();
            self.ten.read_for = None;
            return;
        };
        let doc = t.view.doc();
        let key = (t.key_base, doc.edit_count(), doc.page_count());
        if self.ten.read_for.map(|k| k.0) != Some(key.0) {
            self.ten.read_at = local_now();
        }
        if self.ten.read_for != Some(key) {
            self.ten.notes = doc.popup_notes();
            self.ten.read_for = Some(key);
        }
    }

    /// Measure what the open notes show (after every input; text wraps by the note's width).
    pub(crate) fn measure10(&mut self, m: &dyn TextMeasure) {
        if !self.ten_on() {
            return;
        }
        if !self.draw.reader_defaults {
            // The reference reader's tool defaults (measured, round 10): yellow highlight,
            // pure red rectangle; the drawing tools in reader_draw_win.rs.
            self.draw.reader_defaults = true;
            for (k, c) in [
                (AnnotKind::Highlight, [255, 255, 0]),
                (AnnotKind::Square, [255, 0, 0]),
            ] {
                if self.nine.default_color(k).is_none() {
                    self.nine.set_default_color(k, c);
                }
            }
        }
        self.refresh10();
        let head = font((HEAD_FONT * self.scale).round());
        let body = font((BODY_FONT * self.scale * 1.25).round() / 1.25);
        let mut want: Vec<String> = vec![self.t("pdfTen.options")];
        let mut wrap_jobs = Vec::new();
        for n in self.ten.notes.iter().filter(|n| n.open) {
            want.push(self.note_date10(n));
            want.push(n.comment.author.clone());
            let text = match &self.ten.editing {
                Some((e, s)) if e.id == n.id => s.clone(),
                _ => n.comment.contents.clone(),
            };
            let w = self.note_size(n).0 - 2.0 * self.px(BODY_SIDE + BODY_PAD) - 2.0;
            wrap_jobs.push((n.id, text, w));
        }
        for s in want {
            let key = (s.clone(), head.size as u32);
            self.ten
                .widths
                .entry(key)
                .or_insert_with(|| m.measure_font(&s, &head).0);
        }
        for (id, text, w) in wrap_jobs {
            let wk = w.max(1.0) as u32;
            let fresh = self
                .ten
                .lines
                .get(&id)
                .is_some_and(|(t, k, _)| *t == text && *k == wk);
            if !fresh {
                let lines = wrap(&text, w.max(1.0), m, &body);
                self.ten.lines.insert(id, (text, wk, lines));
            }
        }
    }

    /// A note's size on screen: its rectangle at 96 dpi whatever the zoom (as the reference).
    fn note_size(&self, n: &PopupNote) -> (f32, f32) {
        let k = 4.0 / 3.0 * self.scale;
        (
            ((n.rect[2] - n.rect[0]).abs() * k).round(),
            ((n.rect[3] - n.rect[1]).abs() * k).round(),
        )
    }

    /// Window box (x, y, w, h) of an open note: its top left at the pop-up's place on the page,
    /// kept inside the document area; `None` when its page is not shown.
    fn note_box(&self, n: &PopupNote) -> Option<[f32; 4]> {
        let t = self.tabs.get(self.active)?;
        if !t.view.visible_pages().contains(&n.comment.page) {
            return None;
        }
        let at = t
            .view
            .doc_rect_to_window(n.comment.page, [n.rect[0], n.rect[1], n.rect[0], n.rect[1]])?;
        let (mut w, mut h) = self.note_size(n);
        let (mut x, mut y) = (at[0].round(), at[1].round());
        if let Some(d) = self.ten.drag.as_ref().filter(|d| d.note.id == n.id) {
            let (dx, dy) = (d.to.0 - d.from.0, d.to.1 - d.from.1);
            let k = 4.0 / 3.0 * self.scale;
            match d.grip {
                None => {
                    x += dx;
                    y += dy;
                }
                Some(false) => {
                    w = (w + dx).max(MIN_W * k);
                    h = (h + dy).max(MIN_H * k);
                }
                Some(true) => {
                    let nw = (w - dx).max(MIN_W * k);
                    x += w - nw;
                    w = nw;
                    h = (h + dy).max(MIN_H * k);
                }
            }
        }
        let f = self.doc_rect;
        x = x.min(f[0] + f[2] - w).max(f[0]);
        y = y.min(f[1] + f[3] - h).max(f[1]);
        Some([x.round(), y.round(), w.round(), h.round()])
    }

    /// The open note and part under a window point (the topmost: drawn last).
    pub(crate) fn popup_hit(&self, x: f32, y: f32) -> Option<(PopupNote, Part)> {
        for n in self.ten.notes.iter().rev().filter(|n| n.open) {
            let Some(b) = self.note_box(n) else { continue };
            if !inside(b, x, y) {
                continue;
            }
            let close = self.close_box(b);
            let part = if inside(close, x, y) {
                Part::Close
            } else if y < b[1] + self.px(ROW_H) {
                Part::Header
            } else if y < b[1] + self.px(BODY_TOP) {
                let ow = self.head_width(&self.t("pdfTen.options")) + self.px(12.0);
                if x >= b[0] + b[2] - self.px(6.4) - ow {
                    Part::Options
                } else {
                    Part::Header
                }
            } else if y >= b[1] + b[3] - self.px(BODY_BOTTOM) {
                let g = self.px(BODY_BOTTOM);
                if x <= b[0] + g {
                    Part::Grip(true)
                } else if x >= b[0] + b[2] - g {
                    Part::Grip(false)
                } else {
                    Part::Header
                }
            } else {
                Part::Body
            };
            return Some((n.clone(), part));
        }
        None
    }

    /// A note's date: its comment's, else when the document was read.
    fn note_date10(&self, n: &PopupNote) -> String {
        let d = popup_date(&n.comment.modified);
        if d.is_empty() {
            popup_date(&self.ten.read_at)
        } else {
            d
        }
    }

    fn close_box(&self, b: [f32; 4]) -> [f32; 4] {
        let s = self.px(CLOSE);
        let i = self.px(CLOSE_INSET);
        [b[0] + b[2] - 1.0 - i - s, b[1] + 1.0 + i, s, s]
    }

    fn head_width(&self, s: &str) -> f32 {
        let size = (HEAD_FONT * self.scale).round();
        self.ten
            .widths
            .get(&(s.to_owned(), size as u32))
            .copied()
            .unwrap_or(s.chars().count() as f32 * size * 0.5)
    }

    /// Input of round 10 first: `Some(redraw)` when handled.
    pub(crate) fn route_ten(&mut self, input: &Input, m: &dyn TextMeasure) -> Option<bool> {
        if !self.ten_on() {
            return None;
        }
        if let Some(r) = self.route_bar10(input, m) {
            return Some(r);
        }
        let over_note = match *input {
            Input::Press { x, y, .. } | Input::Move { x, y } => self.popup_hit(x, y).is_some(),
            _ => false,
        };
        if !over_note {
            if let Some(r) = self.route_handles10(input) {
                return Some(r);
            }
        }
        match *input {
            Input::Press { x, y, count, .. } => {
                if !inside(self.doc_rect, x, y) {
                    self.commit_edit10();
                    return None;
                }
                if let Some((n, part)) = self.popup_hit(x, y) {
                    if self.ten.editing.as_ref().is_some_and(|(e, _)| e.id != n.id) {
                        self.commit_edit10();
                    }
                    self.press_note10(n, part, x, y, m);
                    return Some(true);
                }
                let was = self.commit_edit10();
                if count >= 2 && matches!(self.tool, Tool::Hand | Tool::Select) {
                    if let Some(a) = self.comment_at(x, y) {
                        if a.kind != AnnotKind::TextBox {
                            self.open_note10(&a, true);
                            return Some(true);
                        }
                    }
                }
                let _ = was;
                None
            }
            Input::Drag { x, y } => {
                let d = self.ten.drag.as_mut()?;
                d.to = (x, y);
                Some(true)
            }
            Input::Release { x, y } => {
                let d = self.ten.drag.take()?;
                self.ten.drag = Some(PopDrag {
                    to: (x, y),
                    ..d.clone()
                });
                let b = self.note_box(&d.note);
                self.ten.drag = None;
                let moved = (x - d.from.0).abs() + (y - d.from.1).abs() > 2.0;
                if let (Some(b), true) = (b, moved) {
                    self.place_note10(&d.note, b);
                }
                Some(true)
            }
            // Ctrl+V / Ctrl+Alt+V paste at the pointer; Ctrl+C / Ctrl+X copy or cut the
            // selected comment when no text is selected.
            Input::Key(Key::Letter(c), mods) if mods.ctrl && self.ten.editing.is_none() => {
                let text_sel = self
                    .view()
                    .and_then(super::ReaderView::selection)
                    .is_some_and(|s| s.a != s.b);
                match c {
                    'v' => {
                        let (x, y) = self.pointer;
                        if inside(self.doc_rect, x, y) {
                            self.ten.ctx_point = Some((x, y));
                        }
                        self.paste10(mods.alt);
                        Some(true)
                    }
                    'c' | 'x' if self.extra.selected.is_some() && !text_sel => {
                        self.copy_comment10(c == 'x');
                        Some(true)
                    }
                    _ => None,
                }
            }
            Input::Text(ref s) if self.ten.editing.is_some() => {
                if let Some((_, t)) = self.ten.editing.as_mut() {
                    t.push_str(s);
                }
                Some(true)
            }
            Input::Key(k, _) if self.ten.editing.is_some() => {
                match k {
                    Key::Backspace => {
                        if let Some((_, t)) = self.ten.editing.as_mut() {
                            t.pop();
                        }
                    }
                    Key::Enter => {
                        if let Some((_, t)) = self.ten.editing.as_mut() {
                            t.push('\n');
                        }
                    }
                    Key::Escape => {
                        self.commit_edit10();
                    }
                    _ => {}
                }
                Some(true)
            }
            Input::Move { x, y } => {
                let over = self.popup_hit(x, y);
                over.map(|(_, part)| {
                    self.cursor = match part {
                        Part::Body => 4,
                        Part::Header | Part::Grip(_) => 6,
                        _ => 3,
                    };
                    self.pointer = (x, y);
                    true
                })
            }
            _ => None,
        }
    }

    fn press_note10(&mut self, n: PopupNote, part: Part, x: f32, y: f32, m: &dyn TextMeasure) {
        self.extra.selected = Some(n.comment.clone());
        match part {
            Part::Close => {
                self.ten.editing = None;
                let r = self
                    .tabs
                    .get_mut(self.active)
                    .map(|t| t.view.doc_mut().set_popup_note(&n, n.rect, false));
                if let Some(Err(e)) = r {
                    self.status = e;
                }
                self.refresh10();
            }
            Part::Options => {
                if let Some(b) = self.note_box(&n) {
                    let anchor = [
                        b[0] + b[2] - self.px(60.0),
                        b[1] + self.px(BODY_TOP),
                        0.0,
                        0.0,
                    ];
                    self.open_comment_menu9(&n.comment, anchor, m);
                }
            }
            Part::Body => {
                if self.ten.editing.is_none() {
                    let text = n.comment.contents.clone();
                    self.ten.editing = Some((n, text));
                }
            }
            Part::Header | Part::Grip(_) => {
                self.ten.drag = Some(PopDrag {
                    note: n,
                    grip: match part {
                        Part::Grip(l) => Some(l),
                        _ => None,
                    },
                    from: (x, y),
                    to: (x, y),
                });
            }
        }
    }

    /// Store a moved / resized note's window box as its pop-up rectangle.
    fn place_note10(&mut self, n: &PopupNote, b: [f32; 4]) {
        let k = 4.0 / 3.0 * self.scale;
        let Some(t) = self.tabs.get_mut(self.active) else {
            return;
        };
        let Some(pr) = t.view.page_rect(n.comment.page) else {
            return;
        };
        let s = t.view.px_per_pt().max(0.01);
        let (x0, y0) = ((b[0] - pr[0]) / s, (b[1] - pr[1]) / s);
        let rect = [x0, y0, x0 + b[2] / k, y0 + b[3] / k];
        if let Err(e) = t.view.doc_mut().set_popup_note(n, rect, true) {
            self.status = e;
        }
        self.refresh10();
    }

    /// Open comment `a`'s pop-up note (adding one beside it when it has none); `edit` puts the
    /// caret in it.
    pub(crate) fn open_note10(&mut self, a: &Annot, edit: bool) {
        let Some(t) = self.tabs.get_mut(self.active) else {
            return;
        };
        let d = t.view.doc_mut();
        let r = match d.popup_note(a) {
            Some(n) if n.open => Ok(n),
            Some(n) => d.set_popup_note(&n, n.rect, true).map(|()| n),
            None => d.add_popup_note(a, true),
        };
        match r {
            Ok(n) => {
                self.extra.selected = Some(a.clone());
                if edit {
                    self.ten.editing = Some((n, a.contents.clone()));
                }
            }
            Err(e) => self.status = e,
        }
        self.refresh10();
    }

    /// Close comment `a`'s pop-up note.
    pub(crate) fn close_note10(&mut self, a: &Annot) {
        self.ten.editing = None;
        if let Some(t) = self.tabs.get_mut(self.active) {
            let d = t.view.doc_mut();
            if let Some(n) = d.popup_note(a) {
                if let Err(e) = d.set_popup_note(&n, n.rect, false) {
                    self.status = e;
                }
            }
        }
        self.refresh10();
    }

    /// Whether comment `a`'s pop-up note is open.
    pub(crate) fn note_open10(&self, a: &Annot) -> bool {
        self.view()
            .and_then(|v| v.doc().popup_note(a))
            .is_some_and(|n| n.open)
    }

    /// Write the text typed in a note; true when one was being typed in.
    fn commit_edit10(&mut self) -> bool {
        let Some((n, text)) = self.ten.editing.take() else {
            return false;
        };
        if text != n.comment.contents {
            if let Some(t) = self.tabs.get_mut(self.active) {
                if let Err(e) = t.view.doc_mut().set_annotation_contents(&n.comment, &text) {
                    self.status = e;
                }
            }
            self.refresh10();
        }
        true
    }

    /// Draw the open notes over the pages.
    pub(crate) fn paint_popups10(&mut self, p: &mut dyn Painter) {
        if !self.ten_on() {
            return;
        }
        self.paint_handles10(p);
        self.refresh10();
        let open: Vec<PopupNote> = self.ten.notes.iter().filter(|n| n.open).cloned().collect();
        if open.is_empty() {
            return;
        }
        let d = self.doc_rect;
        p.raw_colours(true);
        p.push_clip(d[0], d[1], d[2], d[3]);
        for n in &open {
            if let Some(b) = self.note_box(n) {
                self.paint_note10(p, n, b);
            }
        }
        p.pop_clip();
        p.raw_colours(false);
    }

    fn paint_note10(&self, p: &mut dyn Painter, n: &PopupNote, b: [f32; 4]) {
        let [x, y, w, h] = b;
        // The selected note shows its comment's own colour in a darker frame, the text area
        // grey while typing (measured: 255, 255, 0 / 80 / 240 against 255, 255, 77 / 133).
        let typing = self.ten.editing.as_ref().is_some_and(|(e, _)| e.id == n.id);
        let active = typing || self.extra.selected.as_ref() == Some(&n.comment);
        let tinted = head_colour(n.comment.color);
        let head = if active {
            let c = n.comment.color;
            Rgba(c[0], c[1], c[2], 255)
        } else {
            tinted
        };
        let body = if typing {
            Rgba(240, 240, 240, 255)
        } else {
            body_colour(tinted)
        };
        let edge = edge_colour(tinted);
        let side = self.px(BODY_SIDE);
        let top = self.px(BODY_TOP);
        let bottom = self.px(BODY_BOTTOM);
        let bx = x + 1.0 + side;
        let bw = w - 2.0 - 2.0 * side;
        // Round 14: a note that is not being used lets the page show through (measured on the
        // reference: 77 % opaque, the same colours over white); each pixel is filled once.
        let glass = !active;
        let fill = |p: &mut dyn Painter, r: [f32; 4], c: Rgba| {
            p.fill_rect(
                r[0],
                r[1],
                r[2],
                r[3],
                if glass { see_through14(c) } else { c },
            );
        };
        let frame = if active { FRAME_ACTIVE } else { FRAME };
        for r in [
            [x, y, w, 1.0],
            [x, y + h - 1.0, w, 1.0],
            [x, y + 1.0, 1.0, h - 2.0],
            [x + w - 1.0, y + 1.0, 1.0, h - 2.0],
        ] {
            fill(p, r, frame);
        }
        let body_h = h - top - bottom - 1.0;
        for r in [
            [x + 1.0, y + 1.0, w - 2.0, top - 1.0],
            [x + 1.0, y + top + body_h, w - 2.0, h - 1.0 - top - body_h],
            [x + 1.0, y + top, side, body_h],
            [bx + bw, y + top, x + w - 1.0 - bx - bw, body_h],
        ] {
            fill(p, r, head);
        }
        fill(p, [bx, y + top, bw, body_h], body);
        // Header: the comment's icon, its date, the close box.
        let ink = Ink {
            ink: Rgba(60, 60, 40, 255),
            accent: Rgba(60, 60, 40, 255),
        };
        let icon = self.px(12.8);
        // Only sticky notes show their icon in the header (as the reference).
        if n.comment.kind == AnnotKind::Note {
            reader_icons::draw(p, "note", x + self.px(1.6), y + self.px(2.4), icon, ink);
        }
        let hf = font((HEAD_FONT * self.scale).round());
        let cb = self.close_box(b);
        if glass {
            // The box's face is the header already: only its frame.
            for r in [
                [cb[0], cb[1], cb[2], 1.0],
                [cb[0], cb[1] + cb[3] - 1.0, cb[2], 1.0],
                [cb[0], cb[1] + 1.0, 1.0, cb[3] - 2.0],
                [cb[0] + cb[2] - 1.0, cb[1] + 1.0, 1.0, cb[3] - 2.0],
            ] {
                p.fill_rect(r[0], r[1], r[2], r[3], edge);
            }
        } else {
            p.fill_rect(cb[0], cb[1], cb[2], cb[3], edge);
            p.fill_rect(cb[0] + 1.0, cb[1] + 1.0, cb[2] - 2.0, cb[3] - 2.0, head);
        }
        let i = (cb[2] * 0.22).round();
        let cross = vec![
            PathSeg::M(cb[0] + i, cb[1] + i),
            PathSeg::L(cb[0] + cb[2] - i, cb[1] + cb[3] - i),
            PathSeg::M(cb[0] + cb[2] - i, cb[1] + i),
            PathSeg::L(cb[0] + i, cb[1] + cb[3] - i),
        ];
        p.path(&cross, None, Some((edge, 1.0)));
        let date = self.note_date10(n);
        let dw = self.head_width(&date);
        let dx = (cb[0] - self.px(4.0) - dw).max(x + icon + self.px(4.0));
        p.text_font(&date, dx.round(), y + self.px(DATE_Y), &hf, INK);
        // Round 14: the comment's subject (`/Subj`) at the left of the header, as the
        // reference shows "Note", "Insert Text"…, clipped before the date.
        if let Some(subject) = self.subject14(&n.comment) {
            // At the left edge, after the icon only for sticky notes (the reference).
            let sx = if n.comment.kind == AnnotKind::Note {
                x + icon + self.px(4.0)
            } else {
                x + self.px(4.8)
            };
            p.push_clip(sx, y, (dx - sx - self.px(4.0)).max(0.0), self.px(ROW_H));
            p.text_font(&subject, sx.round(), y + self.px(DATE_Y), &hf, INK);
            p.pop_clip();
        }
        // Author and Options with its arrow.
        p.text_font(
            &n.comment.author,
            x + self.px(4.8),
            y + self.px(AUTHOR_Y),
            &hf,
            INK,
        );
        let opt = self.t("pdfTen.options");
        let ow = self.head_width(&opt);
        let ax = x + w - self.px(6.4);
        let ay = y + self.px(30.4);
        let a = self.px(2.4);
        p.path(
            &[
                PathSeg::M(ax - 2.0 * a, ay - a / 2.0),
                PathSeg::L(ax, ay - a / 2.0),
                PathSeg::L(ax - a, ay + a / 2.0),
                PathSeg::Z,
            ],
            Some(INK),
            None,
        );
        p.text_font(
            &opt,
            (ax - 2.0 * a - self.px(4.0) - ow).round(),
            y + self.px(AUTHOR_Y),
            &hf,
            INK,
        );
        // Text, clipped to its area, with a caret while typing.
        let bf = font((BODY_FONT * self.scale * 1.25).round() / 1.25);
        let tx = bx + self.px(BODY_PAD);
        let mut ty = y + top + self.px(BODY_TEXT_Y);
        let pitch = self.px(BODY_LINE);
        p.push_clip(bx, y + top, bw, h - top - bottom - 1.0);
        let lines = self.ten.lines.get(&n.id).map(|(_, _, l)| l.clone());
        let lines =
            lines.unwrap_or_else(|| n.comment.contents.split('\n').map(str::to_owned).collect());
        for l in &lines {
            p.text_font(l, tx, ty, &bf, INK);
            ty += pitch;
        }
        if self.ten.editing.as_ref().is_some_and(|(e, _)| e.id == n.id) {
            let last = lines.last().map_or("", String::as_str);
            let cx = tx + last.chars().count() as f32 * bf.size * 0.5;
            p.fill_rect(cx.round(), ty - pitch, 1.0, bf.size * 1.2, INK);
        }
        p.pop_clip();
        // Resize grips in the bottom corners.
        let g = self.px(4.8);
        let (gy, l, r) = (y + h - 2.0, x + 2.0, x + w - 2.0);
        p.path(
            &[
                PathSeg::M(l, gy - g),
                PathSeg::L(l + g, gy),
                PathSeg::M(l, gy - g / 2.0),
                PathSeg::L(l + g / 2.0, gy),
                PathSeg::M(r, gy - g),
                PathSeg::L(r - g, gy),
                PathSeg::M(r, gy - g / 2.0),
                PathSeg::L(r - g / 2.0, gy),
            ],
            None,
            Some((edge, 1.0)),
        );
    }

    // ---- Floating toolbar -------------------------------------------------------------

    /// The floating toolbar's box: its bottom left just above where the selection ended, kept
    /// in the document area (below the pointer when there is no room above).
    pub(crate) fn bar_rect10(&self) -> Option<[f32; 4]> {
        let (x, y) = self.ten.bar?;
        let n = BAR_ITEMS.len() as f32;
        let w = self.px(2.0 * BAR_PAD + (n - 1.0) * BAR_STEP + BAR_ICON + BAR_SEP);
        let h = self.px(BAR_H);
        let d = self.doc_rect;
        let mut top = y - self.px(BAR_GAP) - h;
        if top < d[1] {
            top = y + self.px(BAR_GAP) * 8.0;
        }
        let left = x.min(d[0] + d[2] - w).max(d[0]);
        Some([left.round(), top.round(), w, h])
    }

    fn bar_item_x(&self, i: usize) -> f32 {
        let sep = if i >= BAR_LEFT { BAR_SEP } else { 0.0 };
        self.px(BAR_PAD + i as f32 * BAR_STEP + sep)
    }

    fn bar_item_at(&self, x: f32, y: f32) -> Option<usize> {
        let r = self.bar_rect10()?;
        if !inside(r, x, y) {
            return None;
        }
        let half = self.px(BAR_STEP) / 2.0;
        (0..BAR_ITEMS.len()).find(|&i| {
            let cx = r[0] + self.bar_item_x(i) + self.px(BAR_ICON) / 2.0;
            (x - cx).abs() <= half
        })
    }

    fn route_bar10(&mut self, input: &Input, m: &dyn TextMeasure) -> Option<bool> {
        if let Some(r) = self.bar_rect10() {
            let has_sel = self
                .view()
                .and_then(super::ReaderView::selection)
                .is_some_and(|s| s.a != s.b);
            if has_sel {
                match *input {
                    Input::Move { x, y } => {
                        let h = self.bar_item_at(x, y);
                        let changed = h != self.ten.bar_hover;
                        self.ten.bar_hover = h;
                        if inside(r, x, y) {
                            self.cursor = 3;
                            return Some(true);
                        }
                        if changed {
                            return Some(true);
                        }
                    }
                    Input::Press { x, y, .. } => {
                        if inside(r, x, y) {
                            if let Some(i) = self.bar_item_at(x, y) {
                                self.bar_command10(i, m);
                            }
                            return Some(true);
                        }
                        self.ten.bar = None;
                    }
                    Input::Key(..)
                    | Input::Wheel(_)
                    | Input::Zoom(_)
                    | Input::Context { .. }
                    | Input::Resize { .. }
                    | Input::Scale(_) => self.ten.bar = None,
                    _ => {}
                }
            } else {
                self.ten.bar = None;
            }
        }
        if let Input::Release { x, y } = *input {
            if matches!(self.drag, Some(Drag::Select { .. }))
                && matches!(self.tool, Tool::Hand | Tool::Select)
                && self.draw.tool.is_none()
            {
                let sel = self
                    .view()
                    .and_then(super::ReaderView::selection)
                    .is_some_and(|s| s.a != s.b);
                self.ten.bar = sel.then_some((x, y));
                self.ten.bar_hover = None;
            }
        }
        None
    }

    fn bar_command10(&mut self, i: usize, m: &dyn TextMeasure) {
        let Some((cmd, _)) = BAR_ITEMS.get(i).copied() else {
            return;
        };
        if cmd == "ten.more" {
            let r = self.bar_rect10().unwrap_or([0.0; 4]);
            let x = r[0] + self.bar_item_x(i);
            self.ten.bar = None;
            self.open_selection_menu9([x, r[1] + r[3], 0.0, 0.0], m);
            return;
        }
        self.ten.bar = None;
        self.run_command(cmd, m);
    }

    /// Draw the floating toolbar.
    pub(crate) fn paint_bar10(&self, p: &mut dyn Painter) {
        let Some(r) = self.bar_rect10() else { return };
        if self.menu.is_some() {
            return;
        }
        p.fill_rect(r[0], r[1], r[2], r[3], BAR_EDGE);
        p.fill_rect(r[0] + 1.0, r[1] + 1.0, r[2] - 2.0, r[3] - 2.0, BAR_BG);
        let icon = self.px(BAR_ICON);
        let iy = r[1] + ((r[3] - icon) / 2.0).round();
        let ink = Ink {
            ink: Rgba(51, 51, 51, 255),
            accent: self.theme.accent,
        };
        for (i, (_, name)) in BAR_ITEMS.iter().enumerate() {
            let ix = r[0] + self.bar_item_x(i);
            if self.ten.bar_hover == Some(i) {
                let pad = self.px(4.0);
                p.fill_rect(
                    ix - pad,
                    iy - pad,
                    icon + 2.0 * pad,
                    icon + 2.0 * pad,
                    BAR_HOVER,
                );
            }
            if *name == "more" {
                let d = self.px(1.6).max(2.0);
                let cy = (iy + icon / 2.0 - d / 2.0).round();
                for k in 0..3 {
                    let cx = (ix + icon / 2.0 + (k as f32 - 1.0) * self.px(4.8) - d / 2.0).round();
                    p.fill_rect(cx, cy, d, d, self.theme.accent);
                }
            } else {
                reader_icons::draw(p, name, ix, iy, icon, ink);
            }
        }
        let sx = r[0] + self.bar_item_x(BAR_LEFT - 1) + icon + self.px(8.8);
        p.fill_rect(
            sx.round(),
            r[1] + self.px(8.0),
            1.0,
            r[3] - 2.0 * self.px(8.0),
            BAR_SEP_INK,
        );
    }

    // ---- Paste -------------------------------------------------------------------------

    /// Copy the selected comment (Ctrl+C / the comment menu): kept for Paste / Paste in Place,
    /// its text on the clipboard.
    pub(crate) fn copy_comment10(&mut self, cut: bool) {
        let Some(a) = self.extra.selected.clone() else {
            return;
        };
        let text = if a.contents.is_empty() {
            format!("[{}]", super::kind_label(a.kind))
        } else {
            a.contents.clone()
        };
        self.requests.push(super::ReaderRequest::Copy(text.clone()));
        self.ten.clip = Some((a.clone(), text));
        if cut {
            if let Some(t) = self.tabs.get_mut(self.active) {
                if let Err(e) = t.view.doc_mut().delete_annotation(&a) {
                    self.status = e;
                }
            }
            self.extra.selected = None;
            self.document_changed();
        }
    }

    /// The selected comment (tests).
    #[cfg(test)]
    pub(crate) fn extra_selected(&self) -> Option<Annot> {
        self.extra.selected.clone()
    }

    /// Select comment `a` (tests).
    #[cfg(test)]
    pub(crate) fn extra_select(&mut self, a: Annot) {
        self.extra.selected = Some(a);
    }

    /// Whether Paste in Place has a copied comment to paste.
    pub(crate) fn can_paste_in_place10(&self) -> bool {
        self.ten.clip.is_some()
    }

    /// Paste (at the point the page menu opened, or the middle of the view) or Paste in Place
    /// (a copied comment where it was).
    pub(crate) fn paste10(&mut self, in_place: bool) {
        let Some(t) = self.tabs.get(self.active) else {
            return;
        };
        let d = self.doc_rect;
        let (x, y) = self
            .ten
            .ctx_point
            .take()
            .unwrap_or((d[0] + d[2] / 2.0, d[1] + d[3] / 3.0));
        let (page, at) = match t.view.window_to_doc(x, y) {
            Some((i, px, py)) => (i, [px, py]),
            None => (t.view.current_page(), [72.0, 72.0]),
        };
        if in_place {
            let Some((a, _)) = self.ten.clip.clone() else {
                return;
            };
            self.paste_comment10(&a, page, None);
            return;
        }
        self.ten.paste = Some((page, at));
        self.requests
            .push(super::ReaderRequest::Command("host.paste10".into()));
    }

    fn paste_comment10(&mut self, a: &Annot, page: usize, at: Option<[f32; 2]>) {
        if let Some(t) = self.tabs.get_mut(self.active) {
            if let Err(e) = t.view.doc_mut().paste_annotation(a, page, at) {
                self.status = e;
            }
        }
        self.document_changed();
    }

    /// The host's answer to `host.paste10`: `paste10txt` (UTF-8 text) or `paste10img` (a
    /// packed DIB). True when it was one of these.
    pub(crate) fn file_picked10(&mut self, purpose: &str, bytes: &[u8]) -> bool {
        if purpose != "paste10txt" && purpose != "paste10img" {
            return false;
        }
        let Some((page, at)) = self.ten.paste.take() else {
            return true;
        };
        let author = self.author.clone();
        if purpose == "paste10img" {
            let png = bo_raster::codecs::decode_dib(bytes).and_then(|img| {
                let (w, h) = (img.width as f32 * 0.75, img.height as f32 * 0.75);
                bo_raster::rgba_to_png(img).map(|p| (p, w, h))
            });
            let Some((png, w, h)) = png else {
                self.status = self.t("pdfTen.nothingToPaste");
                return true;
            };
            let size = self
                .view()
                .and_then(|v| v.doc().page_info(page).map(|i| (i.width, i.height)));
            let (pw, ph) = size.unwrap_or((612.0, 792.0));
            // As large as the picture at 96 dpi, inside the page.
            let k = (pw / w).min(ph / h).min(1.0);
            let (w, h) = (w * k, h * k);
            let x = at[0].min(pw - w).max(0.0);
            let y = at[1].min(ph - h).max(0.0);
            if let Some(t) = self.tabs.get_mut(self.active) {
                if let Err(e) =
                    t.view
                        .doc_mut()
                        .add_image_stamp(page, [x, y, x + w, y + h], &png, &author)
                {
                    self.status = e;
                }
            }
            self.document_changed();
            return true;
        }
        let text = String::from_utf8_lossy(bytes).replace("\r\n", "\n");
        if text.trim().is_empty() {
            self.status = self.t("pdfTen.nothingToPaste");
            return true;
        }
        if let Some((a, t)) = self.ten.clip.clone() {
            if t == text {
                self.paste_comment10(&a, page, Some(at));
                return true;
            }
        }
        // Clipboard text becomes a typewriter comment with its top left at the point.
        let style = typewriter_style();
        let lines: Vec<&str> = text.lines().collect();
        let longest = lines.iter().map(|l| l.chars().count()).max().unwrap_or(1) as f32;
        let w = (longest * style.font_size * 0.52 + 6.0).min(540.0);
        let h = lines.len().max(1) as f32 * style.font_size * 1.2 + 4.0;
        let shape = bo_pdfread::Shape::Typewriter([at[0], at[1], at[0] + w, at[1] + h]);
        if let Some(t) = self.tabs.get_mut(self.active) {
            if let Err(e) = t
                .view
                .doc_mut()
                .add_drawing(page, &shape, &style, &author, &text)
            {
                self.status = e;
            }
        }
        self.document_changed();
        true
    }
}

/// A pasted typewriter comment: the reference's typewriter default (black, 12 pt).
fn typewriter_style() -> bo_pdfread::DrawStyle {
    bo_pdfread::DrawStyle {
        color: [0, 0, 0],
        opacity: 1.0,
        width: 1.0,
        font_size: 12.0,
    }
}

#[cfg(test)]
#[path = "reader_popups10_tests.rs"]
mod tests;

/// Colour that shows `c` over white at 77 % opacity (BlueReader round 14: pop-up notes).
fn see_through14(c: Rgba) -> Rgba {
    const A: f32 = 0.77;
    let f = |v: u8| {
        ((f32::from(v) - (1.0 - A) * 255.0) / A)
            .clamp(0.0, 255.0)
            .round() as u8
    };
    Rgba(f(c.0), f(c.1), f(c.2), (A * 255.0).round() as u8)
}
