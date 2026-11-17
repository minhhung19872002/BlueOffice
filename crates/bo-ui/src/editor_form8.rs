//! BlueEditor round 8: dialogs in the reference editor's classic captioned layout — group boxes,
//! labels to the left of their controls, radio buttons, check boxes, spin boxes, drop-down lists,
//! links and push buttons placed where the reference puts them (black-box, measured on its
//! dialogs at 125 %: `docs/ui/REPORT-editor.md` round 8). A [`Form`] is a list of controls in
//! dialog coordinates of the 125 % capture (caption included), scaled to the window's UI scale;
//! values live in a small store keyed by control id, so each dialog's owner only builds the
//! controls and reads the values on OK.
use crate::editor_window::inside;
use crate::reader_window::rounded;
use crate::{Input, Key, Painter, TextStyle};
use bo_render::{PathSeg, Rgba};
use std::collections::BTreeMap;

/// When a control can be used: always, never, or while a radio group / check box / drop-down
/// has (or has not) a value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum On {
    Always,
    Never,
    If(&'static str, usize),
    IfNot(&'static str, usize),
    /// Either of two conditions.
    Either(&'static str, usize, &'static str, usize),
}

/// A control of a classic dialog (125 % pixels from the dialog's top-left corner).
#[derive(Debug, Clone)]
pub(crate) enum Ctl {
    /// Group box: frame and title on its top line.
    Group { r: [f32; 4], label: String },
    /// Static text whose vertical centre is `y`; `right`: it ends at `x`.
    Label {
        x: f32,
        y: f32,
        text: String,
        right: bool,
        on: On,
    },
    /// Text box (`multi`: several lines, Enter breaks the line).
    Edit {
        id: &'static str,
        r: [f32; 4],
        multi: bool,
        on: On,
    },
    /// Number box with up / down arrows (value stepped by `step` within `min`..=`max`).
    Spin {
        id: &'static str,
        r: [f32; 4],
        step: f32,
        min: f32,
        max: f32,
        on: On,
    },
    /// Drop-down list.
    Combo {
        id: &'static str,
        r: [f32; 4],
        items: Vec<String>,
        on: On,
    },
    /// Radio button `k` of group `id`, centred on `y`.
    Radio {
        id: &'static str,
        k: usize,
        x: f32,
        y: f32,
        label: String,
        on: On,
    },
    /// Check box centred on `y`.
    Check {
        id: &'static str,
        x: f32,
        y: f32,
        label: String,
        on: On,
    },
    /// Push button (`ok` is the default one, framed in the focus blue).
    Button {
        id: &'static str,
        r: [f32; 4],
        label: String,
        on: On,
    },
    /// Colour box (a click cycles through a few colours).
    Swatch { id: &'static str, r: [f32; 4] },
    /// Underlined blue text that acts as a button (round 8: the reference's links).
    Link {
        id: &'static str,
        x: f32,
        y: f32,
        text: String,
    },
    /// Small square button with one of our own icons (`IconKind`).
    Icon {
        id: &'static str,
        r: [f32; 4],
        icon: IconKind,
        on: On,
    },
    /// Slider bound to spin box `id` (0–100).
    Slider { id: &'static str, r: [f32; 4] },
    /// Information mark (blue disc with an "i").
    Info { x: f32, y: f32 },
    /// White box framed in grey (previews).
    Frame { r: [f32; 4] },
    /// List with column headers and rows.
    Table {
        r: [f32; 4],
        cols: Vec<(String, f32)>,
        rows: Vec<Vec<String>>,
    },
}

/// Icons of the classic dialogs' small buttons (our own drawings).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum IconKind {
    /// A text-style "T".
    TextStyle,
    /// Dynamic text: a figure with a tag.
    Dynamic,
    /// Save a preset: a tray with an arrow into it.
    SavePreset,
    /// Load a preset: a tray with an arrow out of it.
    LoadPreset,
    /// Delete a preset: a bin.
    DeletePreset,
}

/// A value of the store.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Val {
    Text(String),
    Pick(usize),
    Color([u8; 3]),
}

/// What the form did with an input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FormOut {
    /// Nothing happened (or only hover).
    Kept,
    /// Values changed: repaint.
    Changed,
    /// A button or link was pressed (`ok`, `cancel` or the dialog's own).
    Button(&'static str),
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Hit {
    Ctl(usize),
    SpinArrow(usize, bool),
    Item(usize, usize),
    Close,
}

/// An open classic dialog.
#[derive(Debug, Clone)]
pub(crate) struct Form {
    pub title: String,
    /// Size at 125 % (caption included).
    pub size: (f32, f32),
    pub ctls: Vec<Ctl>,
    pub vals: BTreeMap<&'static str, Val>,
    /// Focused text / spin box.
    pub focus: Option<usize>,
    /// Open drop-down list (control index).
    pub list: Option<usize>,
    hover: Option<Hit>,
    hits: Vec<([f32; 4], Hit)>,
    /// Window rectangle of the dialog (set when painted).
    pub frame: [f32; 4],
    /// Typing into text box `.0` ticks check box `.1` (Password Protection).
    pub ticks: Vec<(&'static str, &'static str)>,
    /// The focused box's whole text is selected (round 9: as the reference on opening a
    /// dialog and on Tab); typing replaces it.
    pub all: bool,
}

const WHITE: Rgba = Rgba(255, 255, 255, 255);
const INK: Rgba = Rgba(0, 0, 0, 255);
const DIM: Rgba = Rgba(0xA0, 0xA0, 0xA0, 255);
const BLUE: Rgba = Rgba(0x00, 0x5F, 0xB8, 255);
const FOCUS: Rgba = Rgba(0x00, 0x78, 0xD4, 255);
const GROUP_EDGE: Rgba = Rgba(0xDC, 0xDC, 0xDC, 255);

/// Colours a swatch click cycles through (black, red, blue, grey, white).
const SWATCHES: [[u8; 3]; 5] = [
    [0, 0, 0],
    [0xC0, 0, 0],
    [0, 0x40, 0xC0],
    [0x80, 0x80, 0x80],
    [0xFF, 0xFF, 0xFF],
];

/// Width of `s` in the dialog font of `size` pixels (Segoe-like proportions; the dialogs are
/// laid out before the host can measure).
pub(crate) fn text_w(s: &str, size: f32) -> f32 {
    s.chars()
        .map(|c| match c {
            'i' | 'l' | 'j' | '\'' | '.' | ',' | ':' | ';' | '!' | '|' => 0.25,
            ' ' | 'f' | 't' | 'r' | 'I' | '(' | ')' | '[' | ']' | '-' | '/' => 0.32,
            'm' | 'w' | 'M' | 'W' | '%' => 0.8,
            'A'..='Z' => 0.62,
            '0'..='9' => 0.53,
            _ => 0.5,
        })
        .sum::<f32>()
        * size
        * 1.1
}

impl Form {
    pub(crate) fn new(title: String, size: (f32, f32), ctls: Vec<Ctl>) -> Self {
        Self {
            title,
            size,
            ctls,
            vals: BTreeMap::new(),
            focus: None,
            list: None,
            hover: None,
            hits: Vec::new(),
            frame: [0.0; 4],
            ticks: Vec::new(),
            all: false,
        }
    }

    /// Set a value.
    pub(crate) fn set(&mut self, id: &'static str, v: Val) {
        self.vals.insert(id, v);
    }

    /// Builder form of [`Form::set`].
    #[must_use]
    pub(crate) fn with(mut self, id: &'static str, v: Val) -> Self {
        self.set(id, v);
        self
    }

    /// Replace the items of drop-down `id`.
    pub(crate) fn set_items(&mut self, id: &str, new: &[String]) {
        for c in &mut self.ctls {
            if let Ctl::Combo { id: i, items, .. } = c {
                if *i == id {
                    *items = new.to_vec();
                }
            }
        }
    }

    /// Builder: focus text / spin box `id`.
    #[must_use]
    pub(crate) fn focused(mut self, id: &str) -> Self {
        self.focus = self.ctls.iter().position(
            |c| matches!(c, Ctl::Edit { id: i, .. } | Ctl::Spin { id: i, .. } if *i == id),
        );
        self.all = self.focus.is_some();
        self
    }

    /// Text of a text / spin box ("" when unset).
    pub(crate) fn text(&self, id: &str) -> String {
        match self.vals.get(id) {
            Some(Val::Text(s)) => s.clone(),
            _ => String::new(),
        }
    }

    /// Number in a spin box (its digits, ignoring a unit such as "%" or "°").
    pub(crate) fn num(&self, id: &str, default: f32) -> f32 {
        let t: String = self
            .text(id)
            .chars()
            .filter(|c| c.is_ascii_digit() || *c == '.' || *c == '-' || *c == ',')
            .collect();
        t.replace(',', ".").parse().unwrap_or(default)
    }

    /// Chosen radio button / list item / check box state.
    pub(crate) fn pick(&self, id: &str) -> usize {
        match self.vals.get(id) {
            Some(Val::Pick(k)) => *k,
            _ => 0,
        }
    }

    /// Colour of a swatch.
    pub(crate) fn color(&self, id: &str) -> [u8; 3] {
        match self.vals.get(id) {
            Some(Val::Color(c)) => *c,
            _ => [0, 0, 0],
        }
    }

    /// Whether a control can be used now.
    pub(crate) fn enabled(&self, on: On) -> bool {
        match on {
            On::Always => true,
            On::Never => false,
            On::If(id, k) => self.pick(id) == k,
            On::IfNot(id, k) => self.pick(id) != k,
            On::Either(a, j, b, k) => self.pick(a) == j || self.pick(b) == k,
        }
    }

    fn ctl_on(&self, c: &Ctl) -> bool {
        match c {
            Ctl::Label { on, .. }
            | Ctl::Edit { on, .. }
            | Ctl::Spin { on, .. }
            | Ctl::Combo { on, .. }
            | Ctl::Radio { on, .. }
            | Ctl::Check { on, .. }
            | Ctl::Button { on, .. }
            | Ctl::Icon { on, .. } => self.enabled(*on),
            _ => true,
        }
    }

    /// Index of the first control with `id` (radio `k` for radio groups).
    #[cfg(test)]
    pub(crate) fn index_of(&self, id: &str, k: usize) -> Option<usize> {
        self.ctls.iter().position(|c| match c {
            Ctl::Radio { id: i, k: j, .. } => *i == id && *j == k,
            Ctl::Edit { id: i, .. }
            | Ctl::Spin { id: i, .. }
            | Ctl::Combo { id: i, .. }
            | Ctl::Check { id: i, .. }
            | Ctl::Button { id: i, .. }
            | Ctl::Swatch { id: i, .. }
            | Ctl::Link { id: i, .. }
            | Ctl::Icon { id: i, .. } => *i == id,
            _ => false,
        })
    }

    /// Where control `i` was painted (only controls that can be used are).
    #[cfg(test)]
    pub(crate) fn rect_of(&self, i: usize) -> Option<[f32; 4]> {
        self.hits.iter().find(|h| h.1 == Hit::Ctl(i)).map(|h| h.0)
    }

    /// Where the up / down arrow of spin box `i` was painted.
    #[cfg(test)]
    pub(crate) fn arrow_of(&self, i: usize, up: bool) -> Option<[f32; 4]> {
        self.hits
            .iter()
            .find(|h| h.1 == Hit::SpinArrow(i, up))
            .map(|h| h.0)
    }

    /// Step spin box `i` up or down.
    fn spin(&mut self, i: usize, up: bool) {
        let Some(Ctl::Spin {
            id, step, min, max, ..
        }) = self.ctls.get(i).cloned()
        else {
            return;
        };
        let now = self.text(id);
        let unit: String = now
            .chars()
            .skip_while(|c| c.is_ascii_digit() || *c == '.' || *c == '-')
            .collect();
        let v = (self.num(id, min) + if up { step } else { -step }).clamp(min, max);
        let digits = now
            .split('.')
            .nth(1)
            .map_or(0, |d| d.chars().take_while(char::is_ascii_digit).count());
        self.set(id, Val::Text(format!("{v:.digits$}{unit}")));
    }

    /// Input while the dialog is open (it keeps every input).
    pub(crate) fn input(&mut self, input: &Input) -> FormOut {
        match input {
            Input::Move { x, y } => {
                let h = self.hit(*x, *y);
                if h == self.hover {
                    FormOut::Kept
                } else {
                    self.hover = h;
                    FormOut::Changed
                }
            }
            Input::Press { x, y, .. } => {
                let was_list = self.list.take();
                if was_list.is_none() && self.slide(*x, *y) {
                    return FormOut::Changed;
                }
                match self.hit(*x, *y) {
                    Some(h) => self.press(h, was_list),
                    None => FormOut::Changed,
                }
            }
            Input::Key(Key::Escape, _) => {
                if self.list.take().is_some() {
                    FormOut::Changed
                } else {
                    FormOut::Button("cancel")
                }
            }
            Input::Drag { x, y } => {
                if self.slide(*x, *y) {
                    FormOut::Changed
                } else {
                    FormOut::Kept
                }
            }
            Input::Key(k, m) => self.key(*k, m.shift),
            Input::Text(s) => self.typed(s),
            _ => FormOut::Kept,
        }
    }

    fn hit(&self, x: f32, y: f32) -> Option<Hit> {
        self.hits
            .iter()
            .find(|(r, _)| inside(*r, x, y))
            .map(|h| h.1)
    }

    fn press(&mut self, h: Hit, was_list: Option<usize>) -> FormOut {
        match h {
            Hit::Close => FormOut::Button("cancel"),
            Hit::SpinArrow(i, up) => {
                self.focus = Some(i);
                self.spin(i, up);
                FormOut::Changed
            }
            Hit::Item(i, k) => {
                match self.ctls.get(i) {
                    Some(Ctl::Combo { id, .. }) => {
                        let id = *id;
                        self.set(id, Val::Pick(k));
                    }
                    Some(Ctl::Table { .. }) => {
                        self.set("row", Val::Pick(k));
                        return FormOut::Button("row");
                    }
                    _ => {}
                }
                FormOut::Changed
            }
            Hit::Ctl(i) => {
                let Some(c) = self.ctls.get(i).cloned() else {
                    return FormOut::Kept;
                };
                if !self.ctl_on(&c) {
                    return FormOut::Kept;
                }
                match c {
                    Ctl::Edit { .. } | Ctl::Spin { .. } => {
                        self.focus = Some(i);
                        self.all = false;
                        FormOut::Changed
                    }
                    Ctl::Combo { .. } => {
                        if was_list != Some(i) {
                            self.list = Some(i);
                        }
                        FormOut::Changed
                    }
                    Ctl::Radio { id, k, .. } => {
                        self.set(id, Val::Pick(k));
                        FormOut::Changed
                    }
                    Ctl::Check { id, .. } => {
                        let v = 1 - self.pick(id).min(1);
                        self.set(id, Val::Pick(v));
                        FormOut::Changed
                    }
                    Ctl::Swatch { id, .. } => {
                        let now = self.color(id);
                        let at = SWATCHES.iter().position(|c| *c == now).unwrap_or(0);
                        self.set(id, Val::Color(SWATCHES[(at + 1) % SWATCHES.len()]));
                        FormOut::Changed
                    }
                    Ctl::Button { id, .. } | Ctl::Link { id, .. } | Ctl::Icon { id, .. } => {
                        FormOut::Button(id)
                    }
                    _ => FormOut::Kept,
                }
            }
        }
    }

    /// Click on a slider at window x: its spin box takes the percentage.
    pub(crate) fn slide(&mut self, x: f32, y: f32) -> bool {
        let Some(&(r, Hit::Ctl(i))) = self
            .hits
            .iter()
            .find(|(r, h)| inside(*r, x, y) && matches!(h, Hit::Ctl(_)))
        else {
            return false;
        };
        let Some(Ctl::Slider { id, .. }) = self.ctls.get(i) else {
            return false;
        };
        let id = *id;
        let v = (((x - r[0]) / r[2].max(1.0)) * 100.0)
            .round()
            .clamp(0.0, 100.0);
        self.set(id, Val::Text(format!("{v:.0}%")));
        true
    }

    fn focusable(&self) -> Vec<usize> {
        self.ctls
            .iter()
            .enumerate()
            .filter(|(_, c)| matches!(c, Ctl::Edit { .. } | Ctl::Spin { .. }) && self.ctl_on(c))
            .map(|(i, _)| i)
            .collect()
    }

    fn key(&mut self, k: Key, shift: bool) -> FormOut {
        let focused = self.focus.and_then(|i| self.ctls.get(i).cloned());
        match k {
            Key::Enter => {
                if let Some(Ctl::Edit {
                    id, multi: true, ..
                }) = focused
                {
                    let mut s = self.text(id);
                    s.push('\n');
                    self.set(id, Val::Text(s));
                    return FormOut::Changed;
                }
                if let Some(i) = self.list.take() {
                    let _ = i;
                    return FormOut::Changed;
                }
                FormOut::Button("ok")
            }
            Key::Tab => {
                let f = self.focusable();
                if f.is_empty() {
                    return FormOut::Kept;
                }
                let at = self.focus.and_then(|i| f.iter().position(|x| *x == i));
                let n = f.len();
                let next = match at {
                    Some(a) if shift => (a + n - 1) % n,
                    Some(a) => (a + 1) % n,
                    None => 0,
                };
                self.focus = Some(f[next]);
                self.all = true;
                FormOut::Changed
            }
            Key::Up | Key::Down => {
                let up = k == Key::Up;
                if let Some(i) = self.list {
                    if let Some(Ctl::Combo { id, items, .. }) = self.ctls.get(i) {
                        let id = *id;
                        let n = items.len().max(1);
                        let v = self.pick(id);
                        self.set(
                            id,
                            Val::Pick(if up { (v + n - 1) % n } else { (v + 1) % n }),
                        );
                    }
                    return FormOut::Changed;
                }
                if let Some(Ctl::Spin { .. }) = focused {
                    if let Some(i) = self.focus {
                        self.spin(i, up);
                    }
                    return FormOut::Changed;
                }
                FormOut::Kept
            }
            Key::Backspace => match focused {
                Some(Ctl::Edit { id, .. } | Ctl::Spin { id, .. }) => {
                    let mut s = self.text(id);
                    if std::mem::take(&mut self.all) {
                        s.clear();
                    } else {
                        s.pop();
                    }
                    self.set(id, Val::Text(s));
                    FormOut::Changed
                }
                _ => FormOut::Kept,
            },
            _ => FormOut::Kept,
        }
    }

    fn typed(&mut self, s: &str) -> FormOut {
        let focused = self.focus.and_then(|i| self.ctls.get(i).cloned());
        match focused {
            Some(Ctl::Edit { id, .. } | Ctl::Spin { id, .. }) => {
                let mut v = self.text(id);
                if std::mem::take(&mut self.all) {
                    v.clear();
                }
                v.extend(s.chars().filter(|c| !c.is_control()));
                self.set(id, Val::Text(v));
                if let Some(&(_, check)) = self.ticks.iter().find(|t| t.0 == id) {
                    self.set(check, Val::Pick(1));
                }
                FormOut::Changed
            }
            _ => FormOut::Kept,
        }
    }

    /// Paint centred in a window of `win` at UI scale `scale` (`caret`: show the text caret).
    #[allow(clippy::too_many_lines)]
    pub(crate) fn paint(&mut self, p: &mut dyn Painter, win: (f32, f32), scale: f32, caret: bool) {
        let s = scale / 1.25;
        let q = |v: f32| (v * s).round();
        let (w, h) = (q(self.size.0), q(self.size.1));
        let x0 = ((win.0 - w) / 2.0).round().max(0.0);
        let y0 = ((win.1 - h) / 2.0).round().max(0.0);
        self.frame = [x0, y0, w, h];
        let at = |r: [f32; 4]| [x0 + q(r[0]), y0 + q(r[1]), q(r[2]), q(r[3])];
        let text = TextStyle {
            size: (14.4 * s).round(),
            bold: false,
        };
        let th = text.size * 1.33;
        let est = |t: &str| text_w(t, text.size);
        let mut hits: Vec<([f32; 4], Hit)> = Vec::new();
        // Backdrop, shadow, frame, caption with the title and the close mark.
        p.fill_rect(0.0, 0.0, win.0, win.1, Rgba(0, 0, 0, 40));
        for (k, a) in [(3.0, 18u8), (2.0, 28), (1.0, 40)] {
            p.fill_rect(
                x0 - k,
                y0 - k + 1.0,
                w + 2.0 * k,
                h + 2.0 * k,
                Rgba(0, 0, 0, a),
            );
        }
        p.fill_rect(x0, y0, w, h, Rgba(0xA0, 0xA0, 0xA0, 255));
        p.fill_rect(x0 + 1.0, y0 + 1.0, w - 2.0, h - 2.0, WHITE);
        p.fill_rect(
            x0 + 1.0,
            y0 + 1.0,
            w - 2.0,
            q(31.0),
            Rgba(0xF2, 0xF3, 0xF3, 255),
        );
        p.text(
            &self.title,
            x0 + q(7.0),
            y0 + ((q(32.0) - th) / 2.0).round(),
            text,
            INK,
        );
        let (cx, cy, r) = (x0 + w - q(23.0), y0 + q(16.0), q(5.5));
        p.path(
            &[
                PathSeg::M(cx - r, cy - r),
                PathSeg::L(cx + r, cy + r),
                PathSeg::M(cx + r, cy - r),
                PathSeg::L(cx - r, cy + r),
            ],
            None,
            Some((Rgba(0x40, 0x40, 0x40, 255), s.max(1.0))),
        );
        hits.push(([x0 + w - q(46.0), y0, q(46.0), q(32.0)], Hit::Close));
        let mut open_list = None;
        let ctls = self.ctls.clone();
        for (i, c) in ctls.iter().enumerate() {
            let on = self.ctl_on(c);
            let hot = self.hover == Some(Hit::Ctl(i));
            match c {
                Ctl::Group { r, label } => {
                    let g = at(*r);
                    p.fill_rect(g[0], g[1], g[2], 1.0, GROUP_EDGE);
                    p.fill_rect(g[0], g[1] + g[3], g[2] + 1.0, 1.0, GROUP_EDGE);
                    p.fill_rect(g[0], g[1], 1.0, g[3], GROUP_EDGE);
                    p.fill_rect(g[0] + g[2], g[1], 1.0, g[3], GROUP_EDGE);
                    p.fill_rect(
                        g[0] + q(6.0),
                        g[1] - th / 2.0,
                        est(label) + q(6.0),
                        th,
                        WHITE,
                    );
                    p.text(label, g[0] + q(9.0), (g[1] - th / 2.0).round(), text, INK);
                }
                Ctl::Label {
                    x,
                    y,
                    text: t,
                    right,
                    ..
                } => {
                    let lx = if *right {
                        x0 + q(*x) - est(t)
                    } else {
                        x0 + q(*x)
                    };
                    p.text(
                        t,
                        lx.round(),
                        (y0 + q(*y) - th / 2.0).round(),
                        text,
                        if on { INK } else { DIM },
                    );
                }
                Ctl::Edit { id, r, multi, .. } => {
                    let b = at(*r);
                    let focused = self.focus == Some(i);
                    let edge = if !on {
                        Rgba(0xEC, 0xEC, 0xEC, 255)
                    } else if focused {
                        FOCUS
                    } else if *multi {
                        Rgba(0x64, 0x64, 0x64, 255)
                    } else {
                        // The reference's single-line boxes: a light #ECECEC frame.
                        Rgba(0xEC, 0xEC, 0xEC, 255)
                    };
                    p.fill_rect(b[0], b[1], b[2], b[3], edge);
                    p.fill_rect(
                        b[0] + 1.0,
                        b[1] + 1.0,
                        b[2] - 2.0,
                        b[3] - 2.0,
                        if on {
                            WHITE
                        } else {
                            Rgba(0xFB, 0xFB, 0xFB, 255)
                        },
                    );
                    if on && !focused && !*multi {
                        // Round 9: the reference's boxes have a #838383 line along the bottom.
                        p.fill_rect(
                            b[0] + 1.0,
                            b[1] + b[3] - 1.0,
                            b[2] - 2.0,
                            1.0,
                            Rgba(0x83, 0x83, 0x83, 255),
                        );
                    }
                    let v = self.text(id);
                    p.push_clip(b[0] + 1.0, b[1] + 1.0, b[2] - 2.0, b[3] - 2.0);
                    let lines: Vec<&str> = v.split('\n').collect();
                    let first = if *multi {
                        b[1] + q(4.0)
                    } else {
                        (b[1] + (b[3] - th) / 2.0).round()
                    };
                    let sel = focused && self.all && !v.is_empty() && !*multi;
                    if sel {
                        // The reference's selected text: white on #0078D4.
                        p.fill_rect(
                            b[0] + q(3.0),
                            (first + q(1.0)).round(),
                            est(&v).round() + q(2.0),
                            th - q(2.0),
                            FOCUS,
                        );
                    }
                    for (k, line) in lines.iter().enumerate() {
                        p.text(
                            line,
                            b[0] + q(4.0),
                            first + th * k as f32,
                            text,
                            if sel {
                                WHITE
                            } else if on {
                                INK
                            } else {
                                DIM
                            },
                        );
                    }
                    if focused && caret && !sel {
                        let last = lines.last().copied().unwrap_or("");
                        let cy = first + th * (lines.len().saturating_sub(1)) as f32;
                        p.fill_rect(
                            (b[0] + q(4.0) + est(last)).round(),
                            cy + q(2.0),
                            s.max(1.0).round(),
                            th - q(4.0),
                            INK,
                        );
                    }
                    p.pop_clip();
                    if on {
                        hits.push((b, Hit::Ctl(i)));
                    }
                }
                Ctl::Spin { id, r, .. } => {
                    let b = at(*r);
                    let focused = self.focus == Some(i);
                    let cyy = b[1] + b[3] / 2.0;
                    p.fill_rect(
                        b[0],
                        b[1],
                        b[2],
                        b[3],
                        if focused {
                            FOCUS
                        } else {
                            Rgba(0xD9, 0xD9, 0xD9, 255)
                        },
                    );
                    p.fill_rect(
                        b[0] + 1.0,
                        b[1] + 1.0,
                        b[2] - 2.0,
                        b[3] - 2.0,
                        if on {
                            WHITE
                        } else {
                            Rgba(0xF0, 0xF0, 0xF0, 255)
                        },
                    );
                    let bw = q(20.0);
                    let bx = b[0] + b[2] - bw - 1.0;
                    p.fill_rect(bx, b[1] + 1.0, bw, b[3] - 2.0, Rgba(0xAB, 0xAD, 0xB3, 255));
                    p.fill_rect(
                        bx + 1.0,
                        b[1] + 2.0,
                        bw - 2.0,
                        b[3] / 2.0 - 2.0,
                        Rgba(0xF0, 0xF0, 0xF0, 255),
                    );
                    p.fill_rect(
                        bx + 1.0,
                        cyy + 1.0,
                        bw - 2.0,
                        b[3] / 2.0 - 3.0,
                        Rgba(0xF0, 0xF0, 0xF0, 255),
                    );
                    let ax = bx + bw / 2.0;
                    let ink = if on { Rgba(0x20, 0x20, 0x20, 255) } else { DIM };
                    for (up, ay) in [(true, cyy - b[3] / 4.0), (false, cyy + b[3] / 4.0)] {
                        let a = q(3.0);
                        let dy = if up { -a / 2.0 } else { a / 2.0 };
                        p.path(
                            &[
                                PathSeg::M(ax - a, ay - dy),
                                PathSeg::L(ax, ay + dy),
                                PathSeg::L(ax + a, ay - dy),
                                PathSeg::Z,
                            ],
                            Some(ink),
                            None,
                        );
                        if on {
                            hits.push((
                                [bx, if up { b[1] } else { cyy }, bw + 1.0, b[3] / 2.0],
                                Hit::SpinArrow(i, up),
                            ));
                        }
                    }
                    let v = self.text(id);
                    let sel = focused && self.all && on && !v.is_empty();
                    if sel {
                        p.fill_rect(
                            b[0] + q(4.0),
                            (cyy - th / 2.0 + q(1.0)).round(),
                            est(&v).round() + q(2.0),
                            th - q(2.0),
                            FOCUS,
                        );
                    }
                    p.text(
                        &v,
                        b[0] + q(5.0),
                        (cyy - th / 2.0).round(),
                        text,
                        if sel {
                            WHITE
                        } else if on {
                            INK
                        } else {
                            DIM
                        },
                    );
                    if focused && caret && on && !sel {
                        p.fill_rect(
                            (b[0] + q(5.0) + est(&self.text(id))).round(),
                            b[1] + q(5.0),
                            s.max(1.0).round(),
                            b[3] - q(10.0),
                            INK,
                        );
                    }
                    if on {
                        hits.push(([b[0], b[1], b[2] - bw, b[3]], Hit::Ctl(i)));
                    }
                }
                Ctl::Combo { id, r, items, .. } => {
                    let b = at(*r);
                    let edge = if hot && on {
                        FOCUS
                    } else {
                        Rgba(0x99, 0x99, 0x99, 255)
                    };
                    p.fill_rect(b[0], b[1], b[2], b[3], edge);
                    p.fill_rect(
                        b[0] + 1.0,
                        b[1] + 1.0,
                        b[2] - 2.0,
                        b[3] - 2.0,
                        if on {
                            Rgba(0xE2, 0xE2, 0xE2, 255)
                        } else {
                            Rgba(0xCC, 0xCC, 0xCC, 255)
                        },
                    );
                    let cyy = b[1] + b[3] / 2.0;
                    if let Some(v) = items.get(self.pick(id)) {
                        p.push_clip(b[0] + 1.0, b[1] + 1.0, b[2] - q(26.0), b[3] - 2.0);
                        p.text(
                            v,
                            b[0] + q(3.0),
                            (cyy - th / 2.0).round(),
                            text,
                            if on { INK } else { Rgba(0x6D, 0x6D, 0x6D, 255) },
                        );
                        p.pop_clip();
                    }
                    let (ax, a) = (b[0] + b[2] - q(14.0), q(4.5));
                    p.path(
                        &[
                            PathSeg::M(ax - a, cyy - a / 2.0),
                            PathSeg::L(ax, cyy + a / 2.0),
                            PathSeg::L(ax + a, cyy - a / 2.0),
                        ],
                        None,
                        Some((Rgba(0x80, 0x80, 0x80, 255), s.max(1.0))),
                    );
                    if on {
                        hits.push((b, Hit::Ctl(i)));
                    }
                    if self.list == Some(i) {
                        open_list = Some((i, b, items.clone(), self.pick(id)));
                    }
                }
                Ctl::Radio {
                    id, k, x, y, label, ..
                } => {
                    let (rx, ry, rr) = (x0 + q(*x) + q(8.0), y0 + q(*y), q(8.5));
                    if self.pick(id) == *k {
                        p.path(
                            &circle(rx, ry, rr),
                            Some(if on {
                                BLUE
                            } else {
                                Rgba(0xC6, 0xC6, 0xC6, 255)
                            }),
                            None,
                        );
                        p.path(&circle(rx, ry, rr * 0.45), Some(WHITE), None);
                    } else {
                        p.path(
                            &circle(rx, ry, rr - 0.5),
                            Some(WHITE),
                            Some((
                                if on {
                                    Rgba(0x85, 0x85, 0x85, 255)
                                } else {
                                    Rgba(0xC6, 0xC6, 0xC6, 255)
                                },
                                1.0,
                            )),
                        );
                    }
                    p.text(
                        label,
                        x0 + q(*x) + q(23.0),
                        (ry - th / 2.0).round(),
                        text,
                        if on { INK } else { DIM },
                    );
                    if on {
                        hits.push((
                            [x0 + q(*x), ry - q(12.0), q(25.0) + est(label), q(24.0)],
                            Hit::Ctl(i),
                        ));
                    }
                }
                Ctl::Check {
                    id, x, y, label, ..
                } => {
                    let b = q(17.0);
                    let (bx, by) = (x0 + q(*x), (y0 + q(*y) - b / 2.0).round());
                    if self.pick(id) == 1 {
                        p.path(
                            &rounded(bx, by, b, b, q(3.0)),
                            Some(if on {
                                BLUE
                            } else {
                                Rgba(0xC6, 0xC6, 0xC6, 255)
                            }),
                            None,
                        );
                        p.path(
                            &[
                                PathSeg::M(bx + b * 0.24, by + b * 0.52),
                                PathSeg::L(bx + b * 0.42, by + b * 0.7),
                                PathSeg::L(bx + b * 0.77, by + b * 0.32),
                            ],
                            None,
                            Some((WHITE, (1.4 * s).max(1.2))),
                        );
                    } else {
                        p.path(
                            &rounded(bx + 0.5, by + 0.5, b - 1.0, b - 1.0, q(3.0)),
                            Some(WHITE),
                            Some((
                                if on {
                                    Rgba(0x85, 0x85, 0x85, 255)
                                } else {
                                    Rgba(0xC6, 0xC6, 0xC6, 255)
                                },
                                1.0,
                            )),
                        );
                    }
                    p.text(
                        label,
                        bx + q(20.0),
                        (y0 + q(*y) - th / 2.0).round(),
                        text,
                        if on { INK } else { DIM },
                    );
                    if on {
                        hits.push((
                            [bx, by - q(4.0), q(24.0) + est(label), b + q(8.0)],
                            Hit::Ctl(i),
                        ));
                    }
                }
                Ctl::Button { r, label, id, .. } => {
                    let b = at(*r);
                    let default = *id == "ok";
                    let edge = if !on {
                        Rgba(0xE5, 0xE5, 0xE5, 255)
                    } else if default {
                        FOCUS
                    } else {
                        Rgba(0xD0, 0xD0, 0xD0, 255)
                    };
                    p.path(&rounded(b[0], b[1], b[2], b[3], q(4.0)), Some(edge), None);
                    let inset = if default && on { 1.5 } else { 1.0 };
                    p.path(
                        &rounded(
                            b[0] + inset,
                            b[1] + inset,
                            b[2] - 2.0 * inset,
                            b[3] - 2.0 * inset,
                            q(3.0),
                        ),
                        Some(if !on || hot {
                            Rgba(0xF5, 0xF5, 0xF5, 255)
                        } else {
                            Rgba(0xFD, 0xFD, 0xFD, 255)
                        }),
                        None,
                    );
                    p.text(
                        label,
                        (b[0] + (b[2] - est(label)) / 2.0).round(),
                        (b[1] + (b[3] - th) / 2.0).round(),
                        text,
                        if on { INK } else { DIM },
                    );
                    if on {
                        hits.push((b, Hit::Ctl(i)));
                    }
                }
                Ctl::Link { x, y, text: t, .. } => {
                    let (lx, ly) = (x0 + q(*x), (y0 + q(*y) - th / 2.0).round());
                    let link = Rgba(0x00, 0x00, 0xFF, 255);
                    p.text(t, lx, ly, text, link);
                    let tw = est(t);
                    p.fill_rect(lx, ly + th - q(2.0), tw, 1.0, link);
                    hits.push(([lx, ly, tw, th], Hit::Ctl(i)));
                }
                Ctl::Icon { r, icon, .. } => {
                    let b = at(*r);
                    if hot && on {
                        p.fill_rect(b[0], b[1], b[2], b[3], Rgba(0xE5, 0xF1, 0xFB, 255));
                    }
                    paint_icon(
                        p,
                        b,
                        *icon,
                        if on { Rgba(0x30, 0x30, 0x30, 255) } else { DIM },
                        s,
                    );
                    if on {
                        hits.push((b, Hit::Ctl(i)));
                    }
                }
                Ctl::Swatch { id, r } => {
                    let b = at(*r);
                    let c = self.color(id);
                    p.fill_rect(b[0], b[1], b[2], b[3], Rgba(0xE3, 0xE3, 0xE3, 255));
                    p.fill_rect(
                        b[0] + 1.0,
                        b[1] + 1.0,
                        b[2] - 2.0,
                        b[3] - 2.0,
                        Rgba(c[0], c[1], c[2], 255),
                    );
                    hits.push((b, Hit::Ctl(i)));
                }
                Ctl::Slider { id, r } => {
                    let b = at(*r);
                    let cyy = b[1] + b[3] / 2.0;
                    p.fill_rect(b[0], cyy - 2.0, b[2], 4.0, Rgba(0xE0, 0xE0, 0xE0, 255));
                    let v = self.num(id, 0.0).clamp(0.0, 100.0) / 100.0;
                    let tx = b[0] + b[2] * v;
                    p.path(
                        &[
                            PathSeg::M(tx - q(5.0), cyy - q(9.0)),
                            PathSeg::L(tx + q(5.0), cyy - q(9.0)),
                            PathSeg::L(tx + q(5.0), cyy + q(4.0)),
                            PathSeg::L(tx, cyy + q(9.0)),
                            PathSeg::L(tx - q(5.0), cyy + q(4.0)),
                            PathSeg::Z,
                        ],
                        Some(BLUE),
                        None,
                    );
                    for k in [0.0, 0.5] {
                        p.fill_rect(
                            b[0] + b[2] * k,
                            cyy + q(13.0),
                            1.0,
                            q(4.0),
                            Rgba(0xB0, 0xB0, 0xB0, 255),
                        );
                    }
                    hits.push((b, Hit::Ctl(i)));
                }
                Ctl::Info { x, y } => {
                    let (ix, iy, rr) = (x0 + q(*x), y0 + q(*y), q(10.5));
                    p.path(&circle(ix, iy, rr), Some(Rgba(0x1E, 0x6F, 0xC8, 255)), None);
                    p.fill_rect(ix - q(1.5), iy - q(6.5), q(3.0), q(3.0), WHITE);
                    p.fill_rect(ix - q(1.5), iy - q(2.0), q(3.0), q(9.0), WHITE);
                }
                Ctl::Frame { r } => {
                    let b = at(*r);
                    p.fill_rect(b[0], b[1], b[2], b[3], Rgba(0x8F, 0x8F, 0x8F, 255));
                    p.fill_rect(b[0] + 1.0, b[1] + 1.0, b[2] - 2.0, b[3] - 2.0, WHITE);
                }
                Ctl::Table { r, cols, rows } => {
                    let b = at(*r);
                    p.fill_rect(b[0], b[1], b[2], b[3], Rgba(0xD9, 0xD9, 0xD9, 255));
                    p.fill_rect(b[0] + 1.0, b[1] + 1.0, b[2] - 2.0, b[3] - 2.0, WHITE);
                    // Round 9: without column titles it is a plain list box (the reference's
                    // Converting To PDF: rows 20 px, the chosen one white on #0078D4).
                    let plain = cols.iter().all(|c| c.0.is_empty());
                    let hh = if plain { q(2.0) } else { q(28.0) };
                    let rh = if plain { q(20.0) } else { q(24.0) };
                    // Round 9: a form with a `row` value has selectable rows (Combine Files).
                    if self.vals.contains_key("row") {
                        let sel = self.pick("row");
                        for j in 0..rows.len() {
                            let rr = [b[0] + 1.0, b[1] + hh + rh * j as f32, b[2] - 2.0, rh];
                            if rr[1] + rh > b[1] + b[3] {
                                break;
                            }
                            if j == sel {
                                p.fill_rect(
                                    rr[0],
                                    rr[1],
                                    rr[2],
                                    rr[3],
                                    if plain {
                                        FOCUS
                                    } else {
                                        Rgba(0xCC, 0xE8, 0xFF, 255)
                                    },
                                );
                            }
                            hits.push((rr, Hit::Item(i, j)));
                        }
                    }
                    let mut cx = b[0] + 1.0;
                    for (k, (label, cw)) in cols.iter().enumerate() {
                        let cw = q(*cw);
                        p.text(
                            label,
                            cx + q(5.0),
                            (b[1] + (hh - th) / 2.0).round(),
                            text,
                            INK,
                        );
                        if k + 1 < cols.len() && !plain {
                            p.fill_rect(
                                cx + cw,
                                b[1] + 1.0,
                                1.0,
                                b[3] - 2.0,
                                Rgba(0xEC, 0xEC, 0xEC, 255),
                            );
                        }
                        for (j, row) in rows.iter().enumerate() {
                            if let Some(v) = row.get(k) {
                                p.push_clip(cx, b[1] + hh, cw - q(4.0), b[3] - hh);
                                let chosen =
                                    plain && self.vals.contains_key("row") && self.pick("row") == j;
                                p.text(
                                    v,
                                    cx + q(if plain { 3.0 } else { 5.0 }),
                                    (b[1] + hh + rh * j as f32 + (rh - th) / 2.0).round(),
                                    text,
                                    if chosen { WHITE } else { INK },
                                );
                                p.pop_clip();
                            }
                        }
                        cx += cw;
                    }
                    let mut y = if plain { b[1] + b[3] } else { b[1] + hh };
                    while y < b[1] + b[3] - 1.0 {
                        p.fill_rect(b[0] + 1.0, y, b[2] - 2.0, 1.0, Rgba(0xF0, 0xF0, 0xF0, 255));
                        y += rh;
                    }
                }
            }
        }
        // An open drop-down list over everything, its items hit first.
        if let Some((i, b, items, sel)) = open_list {
            let hover = match self.hover {
                Some(Hit::Item(j, k)) if j == i => Some(k),
                _ => None,
            };
            let mut front: Vec<([f32; 4], Hit)> = paint_list(p, b, &items, sel, hover, s, text)
                .into_iter()
                .map(|(r, k)| (r, Hit::Item(i, k)))
                .collect();
            front.extend(hits);
            hits = front;
        }
        self.hits = hits;
    }
}

/// The list of an open drop-down as the Windows combo box of the reference's dialogs draws it
/// (125 %: frame `#7A7A7A`, items 21 px, the highlighted one `#0078D4` with white text, at most
/// 11 shown with a scroll bar); under combo rectangle `b` (window pixels) at scale `s`
/// (UI scale / 1.25). Returns the item rectangles.
pub(crate) fn paint_list(
    p: &mut dyn Painter,
    b: [f32; 4],
    items: &[String],
    sel: usize,
    hover: Option<usize>,
    s: f32,
    text: TextStyle,
) -> Vec<([f32; 4], usize)> {
    let q = |v: f32| (v * s).round();
    let th = text.size * 1.33;
    let ih = q(21.0);
    let shown = items.len().min(11);
    let first = if sel >= shown { sel + 1 - shown } else { 0 };
    let lr = [b[0], b[1] + b[3], b[2], ih * shown as f32 + 2.0];
    p.fill_rect(lr[0], lr[1], lr[2], lr[3], Rgba(0x7A, 0x7A, 0x7A, 255));
    p.fill_rect(lr[0] + 1.0, lr[1] + 1.0, lr[2] - 2.0, lr[3] - 2.0, WHITE);
    let scroll = items.len() > shown;
    let sw = if scroll { q(17.0) } else { 0.0 };
    let lit = hover.unwrap_or(sel);
    let mut out = Vec::new();
    for j in 0..shown {
        let k = first + j;
        let Some(it) = items.get(k) else { break };
        let ir = [
            lr[0] + 1.0,
            lr[1] + 1.0 + ih * j as f32,
            lr[2] - 2.0 - sw,
            ih,
        ];
        let on = k == lit;
        if on {
            p.fill_rect(ir[0] + 1.0, ir[1], ir[2] - 1.0, ir[3], FOCUS);
        }
        p.text(
            it,
            ir[0] + q(2.0),
            (ir[1] + (ih - th) / 2.0).round(),
            text,
            if on { WHITE } else { INK },
        );
        out.push((ir, k));
    }
    if scroll {
        let tx = lr[0] + lr[2] - 1.0 - sw;
        p.fill_rect(
            tx,
            lr[1] + 1.0,
            sw,
            lr[3] - 2.0,
            Rgba(0xF0, 0xF0, 0xF0, 255),
        );
        let track = lr[3] - 2.0 - 2.0 * q(10.0);
        let th_len = (track * shown as f32 / items.len() as f32).max(q(12.0));
        let at = (track - th_len) * first as f32 / (items.len() - shown).max(1) as f32;
        p.fill_rect(
            tx + (sw - q(3.0)) / 2.0,
            lr[1] + 1.0 + q(10.0) + at,
            q(3.0),
            th_len,
            Rgba(0x85, 0x85, 0x85, 255),
        );
    }
    out
}

/// Our own small icons (strokes in `ink`), centred in `b`.
fn paint_icon(p: &mut dyn Painter, b: [f32; 4], icon: IconKind, ink: Rgba, s: f32) {
    let (cx, cy) = (b[0] + b[2] / 2.0, b[1] + b[3] / 2.0);
    let u = (b[2].min(b[3]) / 2.0 - 3.0 * s).max(4.0);
    let w = (1.3 * s).max(1.0);
    let line = |p: &mut dyn Painter, pts: &[(f32, f32)]| {
        let mut segs = Vec::new();
        for (k, (x, y)) in pts.iter().enumerate() {
            let pt = (cx + x * u, cy + y * u);
            segs.push(if k == 0 {
                PathSeg::M(pt.0, pt.1)
            } else {
                PathSeg::L(pt.0, pt.1)
            });
        }
        p.path(&segs, None, Some((ink, w)));
    };
    match icon {
        IconKind::TextStyle => {
            line(p, &[(-0.7, -0.8), (0.7, -0.8)]);
            line(p, &[(0.0, -0.8), (0.0, 0.85)]);
            line(p, &[(-0.3, 0.85), (0.3, 0.85)]);
        }
        IconKind::Dynamic => {
            // A head and shoulders with a small tag at the right.
            p.path(
                &circle(cx - 0.15 * u, cy - 0.4 * u, 0.35 * u),
                None,
                Some((ink, w)),
            );
            line(p, &[(-0.85, 0.9), (-0.75, 0.3), (0.45, 0.3), (0.55, 0.9)]);
            line(p, &[(0.55, -0.2), (0.9, -0.2), (0.9, 0.15), (0.55, 0.15)]);
        }
        IconKind::SavePreset | IconKind::LoadPreset => {
            line(p, &[(-0.85, 0.2), (-0.85, 0.85), (0.85, 0.85), (0.85, 0.2)]);
            line(p, &[(0.0, -0.9), (0.0, 0.45)]);
            if icon == IconKind::SavePreset {
                line(p, &[(-0.35, 0.1), (0.0, 0.45), (0.35, 0.1)]);
            } else {
                line(p, &[(-0.35, -0.55), (0.0, -0.9), (0.35, -0.55)]);
            }
        }
        IconKind::DeletePreset => {
            line(p, &[(-0.85, -0.6), (0.85, -0.6)]);
            line(p, &[(-0.3, -0.6), (-0.3, -0.9), (0.3, -0.9), (0.3, -0.6)]);
            line(p, &[(-0.6, -0.6), (-0.5, 0.9), (0.5, 0.9), (0.6, -0.6)]);
            line(p, &[(-0.15, -0.3), (-0.15, 0.6)]);
            line(p, &[(0.2, -0.3), (0.2, 0.6)]);
        }
    }
}

/// A link control.
pub(crate) fn link(id: &'static str, x: f32, y: f32, text: String) -> Ctl {
    Ctl::Link { id, x, y, text }
}

pub(crate) fn circle(cx: f32, cy: f32, r: f32) -> Vec<PathSeg> {
    let k = r * 0.552_285;
    vec![
        PathSeg::M(cx + r, cy),
        PathSeg::C(cx + r, cy + k, cx + k, cy + r, cx, cy + r),
        PathSeg::C(cx - k, cy + r, cx - r, cy + k, cx - r, cy),
        PathSeg::C(cx - r, cy - k, cx - k, cy - r, cx, cy - r),
        PathSeg::C(cx + k, cy - r, cx + r, cy - k, cx + r, cy),
        PathSeg::Z,
    ]
}

// Short builders for the dialogs' tables of controls.

pub(crate) fn group(label: String, r: [f32; 4]) -> Ctl {
    Ctl::Group { r, label }
}

pub(crate) fn label(x: f32, y: f32, text: String) -> Ctl {
    Ctl::Label {
        x,
        y,
        text,
        right: false,
        on: On::Always,
    }
}

pub(crate) fn label_on(x: f32, y: f32, text: String, on: On) -> Ctl {
    Ctl::Label {
        x,
        y,
        text,
        right: false,
        on,
    }
}

pub(crate) fn rlabel(x: f32, y: f32, text: String, on: On) -> Ctl {
    Ctl::Label {
        x,
        y,
        text,
        right: true,
        on,
    }
}

pub(crate) fn radio(id: &'static str, k: usize, x: f32, y: f32, label: String) -> Ctl {
    Ctl::Radio {
        id,
        k,
        x,
        y,
        label,
        on: On::Always,
    }
}

pub(crate) fn check(id: &'static str, x: f32, y: f32, label: String) -> Ctl {
    Ctl::Check {
        id,
        x,
        y,
        label,
        on: On::Always,
    }
}

pub(crate) fn edit(id: &'static str, r: [f32; 4], on: On) -> Ctl {
    Ctl::Edit {
        id,
        r,
        multi: false,
        on,
    }
}

pub(crate) fn spin(id: &'static str, r: [f32; 4], step: f32, range: (f32, f32), on: On) -> Ctl {
    Ctl::Spin {
        id,
        r,
        step,
        min: range.0,
        max: range.1,
        on,
    }
}

pub(crate) fn combo(id: &'static str, r: [f32; 4], items: Vec<String>, on: On) -> Ctl {
    Ctl::Combo { id, r, items, on }
}

pub(crate) fn button(id: &'static str, r: [f32; 4], label: String, on: On) -> Ctl {
    Ctl::Button { id, r, label, on }
}
