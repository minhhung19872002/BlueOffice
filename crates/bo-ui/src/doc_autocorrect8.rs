//! AutoCorrect dialog of BlueDoc (parity round 8; File › Options › Proofing › AutoCorrect
//! Options…), laid out after the reference word processor's dialog measured at 125 %
//! (`doc8/ref-acdlg.png`: 618 × 533 px without the tab row; coordinates below are those
//! reference pixels, 38 px lower for this form's tab row). Two pages: AutoCorrect (capitals,
//! the replace-as-you-type list) and AutoFormat As You Type (quotes, ordinals, fractions,
//! dashes, hyperlinks, automatic lists). OK sends the options to the engine
//! (`Command::AutoCorrect`, ADR-0008), which honours each of them as you type. Options BlueDoc
//! does not have yet are shown greyed. The list is BlueDoc's own default list
//! (`bo_edit::autocorrect::default_entries`).
use super::form::{Event, Form, Kind};
use super::t;
use crate::strings::Strings;
use crate::{Font, Input, Key, Painter, TextMeasure};
use bo_edit::autocorrect::Replacement;
use bo_edit::AutoCorrectOptions;
use bo_render::Rgba;

/// Rows of the replacement list shown at once (78 px, 19 px rows on the reference).
const ROWS: usize = 4;
/// The list's size and its scroll bar's width (reference pixels).
const LIST_W: f32 = 554.0;
const LIST_H: f32 = 78.0;
const BAR_W: f32 = 17.0;
/// Row height and the "With" column's left edge (reference pixels).
const ROW_H: f32 = 19.0;
const COL2: f32 = 126.0;
/// Selected row: the system highlight (#0078D7) with white text, as on the reference.
const SEL: Rgba = Rgba(0x00, 0x78, 0xD7, 255);
/// Grid lines between rows and columns (#D5D5D5 measured).
const GRID: Rgba = Rgba(0xD5, 0xD5, 0xD5, 255);
/// Scroll bar track, thumb and arrows of the list (the reference's thin bar).
const BAR_FACE: Rgba = Rgba(0xf0, 0xf0, 0xf0, 255);
const BAR_THUMB: Rgba = Rgba(0x85, 0x85, 0x85, 255);
const BAR_ARROW: Rgba = Rgba(0x60, 0x60, 0x60, 255);
/// The list's frame (#828790, the classic list box frame).
const FRAME: Rgba = Rgba(0x82, 0x87, 0x90, 255);

/// Tab-row offset of this form against the reference capture.
const DY: f32 = 38.0;

/// Check boxes of the pages and the option each one sets.
const CHECKS: [(&str, usize, f32, &str, &str); 11] = [
    (
        "caps2",
        0,
        89.0,
        "docR8.ac.twoCaps",
        "Correct TWo INitial CApitals",
    ),
    (
        "capSent",
        0,
        114.0,
        "docR8.ac.capSentences",
        "Capitalize first letter of sentences",
    ),
    (
        "capCell",
        0,
        139.0,
        "docR8.ac.capCells",
        "Capitalize first letter of table cells",
    ),
    (
        "replace",
        0,
        239.0,
        "docR8.ac.replace",
        "Replace text as you type",
    ),
    (
        "quotes",
        1,
        89.0,
        "docR8.ac.quotes",
        "\"Straight quotes\" with \u{201c}smart quotes\u{201d}",
    ),
    (
        "ordinals",
        1,
        114.0,
        "docR8.ac.ordinals",
        "Ordinals (1st) with superscript",
    ),
    (
        "fractions",
        1,
        139.0,
        "docR8.ac.fractions",
        "Fractions (1/2) with fraction character (\u{bd})",
    ),
    (
        "dashes",
        1,
        164.0,
        "docR8.ac.dashes",
        "Hyphens (--) with dash (\u{2014})",
    ),
    (
        "links",
        1,
        189.0,
        "docR8.ac.links",
        "Internet and network paths with hyperlinks",
    ),
    (
        "lists",
        1,
        264.0,
        "docR8.ac.lists",
        "Automatic bulleted and numbered lists",
    ),
    (
        "enabled",
        1,
        339.0,
        "docR8.ac.enabled",
        "Correct and format as you type",
    ),
];

/// What the owner does after an input.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum AcOut {
    None,
    Redraw,
    Close,
    /// OK with these options.
    Apply(Box<AutoCorrectOptions>),
}

/// The open AutoCorrect dialog.
pub(crate) struct AutoCorrectDialog {
    form: Form,
    opts: AutoCorrectOptions,
    /// Replacement list (sorted by the typed text, as the reference lists it).
    entries: Vec<Replacement>,
    sel: Option<usize>,
    top: usize,
    pointer: (f32, f32),
    /// The Exceptions dialog over this one.
    exceptions: Option<ExceptionsDialog>,
    /// Check box colours given by the window.
    accent: Option<(Rgba, Rgba)>,
    strings_title: String,
    /// First Letter, INitial CAps, their labels, Add, Delete, OK, Cancel.
    strings_ex: [String; 8],
}

/// AutoCorrect Exceptions (505 x 371 at 125 % on the reference, `doc8/ref-exdlg.png`): First
/// Letter (abbreviations after which no capital is forced) and INitial CAps (words that keep
/// two capitals), each a box, a list, Add and Delete.
pub(crate) struct ExceptionsDialog {
    form: Form,
    lists: [Vec<String>; 2],
    pointer: (f32, f32),
}

/// What the Exceptions dialog's owner does.
enum ExOut {
    Stay(bool),
    Close,
    Apply([Vec<String>; 2]),
}

impl ExceptionsDialog {
    fn open(title: &str, s: &[String; 8], first: &[String], caps: &[String]) -> Self {
        let mut f = Form::new(title, (505.0, 371.0), vec![s[0].clone(), s[1].clone()]);
        for (page, label, edit, list, add, del) in [
            (0, "l0", "e0", "list0", "add0", "del0"),
            (1, "l1", "e1", "list1", "add1", "del1"),
        ] {
            f.add(
                label,
                Some(page),
                [33.0, 86.0, 300.0, 20.0],
                Kind::Label(s[2 + page].clone()),
            );
            f.add(
                edit,
                Some(page),
                [32.0, 108.0, 314.0, 25.0],
                Kind::Edit {
                    value: Vec::new(),
                    caret: 0,
                },
            );
            f.add(
                list,
                Some(page),
                [32.0, 134.0, 314.0, 138.0],
                Kind::List {
                    items: Vec::new(),
                    sel: None,
                    top: 0,
                },
            );
            for (id, y, k) in [(add, 108.0, 4), (del, 147.0, 5)] {
                f.add(
                    id,
                    Some(page),
                    [360.0, y, 112.0, 28.0],
                    Kind::Button {
                        label: s[k].clone(),
                        default: false,
                    },
                );
            }
        }
        f.ok_cancel(&s[6], &s[7]);
        f.set_page(0);
        let mut d = Self {
            form: f,
            lists: [first.to_vec(), caps.to_vec()],
            pointer: (0.0, 0.0),
        };
        for l in &mut d.lists {
            l.sort_by_key(|w| w.to_lowercase());
        }
        d.sync();
        d
    }

    fn sync(&mut self) {
        for (i, id) in ["list0", "list1"].into_iter().enumerate() {
            // Nothing chosen, as the reference opens.
            self.form.set_items(id, self.lists[i].clone(), None);
        }
        self.sync_buttons();
    }

    /// Add needs a new word in the box, Delete a chosen row (greyed otherwise, as on the
    /// reference).
    fn sync_buttons(&mut self) {
        let ids = [
            ("e0", "list0", "add0", "del0"),
            ("e1", "list1", "add1", "del1"),
        ];
        for (i, (edit, list, add, del)) in ids.into_iter().enumerate() {
            let w = self.form.text(edit).trim().to_owned();
            let new = !w.is_empty() && !self.lists[i].contains(&w);
            let chosen = self.form.sel(list).is_some();
            self.form.enable(add, new);
            self.form.enable(del, chosen);
        }
    }

    fn input(&mut self, input: &Input) -> ExOut {
        let ev = match input {
            Input::Text(s) => self.form.type_text(s),
            Input::Key(Key::Escape, _) => Event::Cancel,
            Input::Key(k, m) => self.form.key(*k, *m),
            Input::Move { x, y } => {
                self.pointer = (*x, *y);
                return ExOut::Stay(self.form.hover(*x, *y));
            }
            Input::Press { x, y, count, .. } => {
                self.pointer = (*x, *y);
                self.form.press(*x, *y, *count)
            }
            Input::Wheel(d) => {
                let (x, y) = self.pointer;
                return ExOut::Stay(self.form.wheel(x, y, *d));
            }
            Input::Blink => return ExOut::Stay(true),
            _ => return ExOut::Stay(false),
        };
        self.event(&ev)
    }

    fn event(&mut self, ev: &Event) -> ExOut {
        let page = |id: &str| usize::from(id.ends_with('1'));
        match ev {
            Event::Cancel | Event::Clicked("cancel") => ExOut::Close,
            Event::Ok | Event::Clicked("ok") => ExOut::Apply(self.lists.clone()),
            Event::Clicked(id @ ("add0" | "add1")) => {
                let i = page(id);
                let edit = if i == 0 { "e0" } else { "e1" };
                let w = self.form.text(edit).trim().to_owned();
                if !w.is_empty() && !self.lists[i].contains(&w) {
                    self.lists[i].push(w);
                    self.lists[i].sort_by_key(|w| w.to_lowercase());
                }
                self.form.set_text(edit, "");
                self.sync();
                ExOut::Stay(true)
            }
            Event::Clicked(id @ ("del0" | "del1")) => {
                let i = page(id);
                let list = if i == 0 { "list0" } else { "list1" };
                if let Some(k) = self.form.sel(list) {
                    if k < self.lists[i].len() {
                        self.lists[i].remove(k);
                    }
                }
                self.sync();
                ExOut::Stay(true)
            }
            // Choosing a row puts it in the box, as on the reference.
            Event::Changed(id @ ("list0" | "list1")) => {
                let i = page(id);
                let list = if i == 0 { "list0" } else { "list1" };
                if let Some(w) = self.form.sel(list).and_then(|k| self.lists[i].get(k)) {
                    let w = w.clone();
                    self.form.set_text(if i == 0 { "e0" } else { "e1" }, &w);
                }
                self.sync_buttons();
                ExOut::Stay(true)
            }
            Event::Changed(_) => {
                self.sync_buttons();
                ExOut::Stay(true)
            }
            _ => ExOut::Stay(true),
        }
    }
}

fn check_rect(y: f32) -> [f32; 4] {
    [33.0, y + DY - 11.0, 440.0, 22.0]
}

impl AutoCorrectDialog {
    /// The dialog showing `opts` (the engine's current options).
    pub(crate) fn open(s: &Strings, opts: &AutoCorrectOptions, language: &str) -> Self {
        let tr = |k: &str, f: &str| t(s, k, f);
        // "AutoCorrect: English (United States)": the proofing language at the caret.
        let title = format!("{}: {language}", tr("docR8.ac.title", "AutoCorrect"));
        let mut f = Form::new(
            &title,
            (618.0, 533.0 + DY),
            vec![
                tr("docR8.ac.tabAutoCorrect", "AutoCorrect"),
                tr("docR8.ac.tabFormat", "AutoFormat As You Type"),
            ],
        );
        // Options BlueDoc does not have: greyed, as they are on the reference when unavailable.
        f.add(
            "showButtons",
            Some(0),
            check_rect(58.0),
            Kind::Check {
                label: tr("docR8.ac.showButtons", "Show AutoCorrect Options buttons"),
                on: false,
            },
        );
        for (id, y, key, fb) in [
            ("days", 164.0, "docR8.ac.days", "Capitalize names of days"),
            (
                "capsLock",
                189.0,
                "docR8.ac.capsLock",
                "Correct accidental usage of cAPS LOCK key",
            ),
        ] {
            f.add(
                id,
                Some(0),
                check_rect(y),
                Kind::Check {
                    label: tr(key, fb),
                    on: false,
                },
            );
        }
        f.add(
            "exceptions",
            Some(0),
            [487.0, 78.0 + DY, 111.0, 28.0],
            Kind::Button {
                label: tr("docR8.ac.exceptions", "Exceptions..."),
                default: false,
            },
        );
        f.add(
            "frame",
            Some(0),
            [18.0, 236.0 + DY, 580.0, 242.0],
            Kind::Group(String::new()),
        );
        for (id, page, y, key, fb) in CHECKS {
            f.add(
                id,
                Some(page),
                check_rect(y),
                Kind::Check {
                    label: tr(key, fb),
                    on: false,
                },
            );
        }
        f.add(
            "gReplace",
            Some(1),
            [18.0, 62.0 + DY, 580.0, 24.0],
            Kind::Group(tr("docR8.ac.replaceAsYouType", "Replace as you type")),
        );
        f.add(
            "gApply",
            Some(1),
            [18.0, 237.0 + DY, 580.0, 24.0],
            Kind::Group(tr("docR8.ac.applyAsYouType", "Apply as you type")),
        );
        f.add(
            "gAlways",
            Some(1),
            [18.0, 312.0 + DY, 580.0, 24.0],
            Kind::Group(tr("docR8.ac.always", "Always")),
        );
        f.add(
            "fromL",
            Some(0),
            [33.0, 256.0 + DY, 120.0, 20.0],
            Kind::Label(tr("docR8.ac.replaceL", "Replace:")),
        );
        f.add(
            "toL",
            Some(0),
            [157.0, 256.0 + DY, 120.0, 20.0],
            Kind::Label(tr("docR8.ac.withL", "With:")),
        );
        // Plain / Formatted text: BlueDoc's entries are plain text (Formatted needs a
        // selection with formatting, greyed as on the reference without one).
        for (id, x, key, fb, on) in [
            ("plain", 201.0, "docR8.ac.plain", "Plain text", true),
            (
                "formatted",
                293.0,
                "docR8.ac.formatted",
                "Formatted text",
                false,
            ),
        ] {
            f.add(
                id,
                Some(0),
                [x, 255.0 + DY, 110.0, 22.0],
                Kind::Check {
                    label: tr(key, fb),
                    on,
                },
            );
            f.enable(id, false);
        }
        f.set_radios(&["plain", "formatted"]);
        f.add(
            "from",
            Some(0),
            [32.0, 280.0 + DY, 126.0, 26.0],
            Kind::Edit {
                value: Vec::new(),
                caret: 0,
            },
        );
        f.add(
            "to",
            Some(0),
            [158.0, 280.0 + DY, 428.0, 26.0],
            Kind::Edit {
                value: Vec::new(),
                caret: 0,
            },
        );
        f.add(
            "list",
            Some(0),
            [32.0, 333.0 + DY, 554.0, 78.0],
            Kind::Canvas,
        );
        f.add(
            "add",
            Some(0),
            [346.0, 417.0 + DY, 112.0, 28.0],
            Kind::Button {
                label: tr("docR8.ac.add", "Add"),
                default: false,
            },
        );
        f.add(
            "delete",
            Some(0),
            [473.0, 417.0 + DY, 112.0, 28.0],
            Kind::Button {
                label: tr("docR8.ac.delete", "Delete"),
                default: false,
            },
        );
        f.add(
            "spell",
            Some(0),
            check_rect(462.0),
            Kind::Check {
                label: tr(
                    "docR8.ac.spelling",
                    "Automatically use suggestions from the spelling checker",
                ),
                on: false,
            },
        );
        f.ok_cancel(&tr("common.ok", "OK"), &tr("common.cancel", "Cancel"));
        f.set_page(0);
        let mut entries = opts.entries.clone();
        entries.sort_by_key(|e| e.from.to_lowercase());
        let mut d = Self {
            form: f,
            opts: opts.clone(),
            entries,
            // The reference opens with the first entry chosen.
            sel: Some(0),
            top: 0,
            pointer: (0.0, 0.0),
            exceptions: None,
            accent: None,
            strings_title: tr("docR8.ex.title", "AutoCorrect Exceptions"),
            strings_ex: [
                tr("docR8.ex.first", "First Letter"),
                tr("docR8.ex.caps", "INitial CAps"),
                tr("docR8.ex.firstL", "Don't capitalize after:"),
                tr("docR8.ex.capsL", "Don't correct:"),
                tr("docR8.ac.add", "Add"),
                tr("docR8.ac.delete", "Delete"),
                tr("common.ok", "OK"),
                tr("common.cancel", "Cancel"),
            ],
        };
        d.load();
        d.sync_buttons();
        d
    }

    fn load(&mut self) {
        let o = self.opts.clone();
        for (id, on) in [
            ("caps2", o.two_initial_caps),
            ("capSent", o.capitalize_sentences),
            ("capCell", o.capitalize_cells),
            ("replace", o.replace_text),
            ("quotes", o.smart_quotes),
            ("ordinals", o.ordinals),
            ("fractions", o.fractions),
            ("dashes", o.dashes),
            ("links", o.auto_links),
            ("lists", o.auto_lists),
            ("enabled", o.enabled),
            ("showButtons", o.show_buttons),
            ("days", o.capitalize_days),
            ("capsLock", o.caps_lock),
            ("spell", o.spelling_suggestions),
        ] {
            self.form.set_on(id, on);
        }
    }

    /// Check boxes in BlueDoc's accent (the window's theme), as its other dialogs.
    pub(crate) fn set_accent(&mut self, fill: Rgba, edge: Rgba) {
        self.accent = Some((fill, edge));
        self.form.set_accent(fill, edge);
    }

    /// The options as the dialog shows them.
    pub(crate) fn collect(&self) -> AutoCorrectOptions {
        let f = &self.form;
        AutoCorrectOptions {
            enabled: f.on("enabled"),
            replace_text: f.on("replace"),
            capitalize_sentences: f.on("capSent"),
            capitalize_cells: f.on("capCell"),
            two_initial_caps: f.on("caps2"),
            smart_quotes: f.on("quotes"),
            dashes: f.on("dashes"),
            ordinals: f.on("ordinals"),
            fractions: f.on("fractions"),
            auto_lists: f.on("lists"),
            auto_links: f.on("links"),
            show_buttons: f.on("showButtons"),
            capitalize_days: f.on("days"),
            caps_lock: f.on("capsLock"),
            spelling_suggestions: f.on("spell"),
            entries: self.entries.clone(),
            ..self.opts.clone()
        }
    }

    pub(crate) fn layout(&mut self, m: &dyn TextMeasure, size: (f32, f32), scale: f32) {
        self.form.layout(m, size, scale);
        if let Some(x) = self.exceptions.as_mut() {
            x.form.layout(m, size, scale);
        }
    }

    /// Add is offered for a new or changed replacement, Delete for a listed one.
    fn sync_buttons(&mut self) {
        let from = self.form.text("from");
        let to = self.form.text("to");
        let listed = self.entries.iter().position(|e| e.from == from);
        let same = listed.is_some_and(|i| self.entries[i].to == to);
        self.form
            .enable("add", !from.trim().is_empty() && !to.is_empty() && !same);
        self.form.enable("delete", listed.is_some());
    }

    fn select(&mut self, i: Option<usize>) {
        self.sel = i;
        if let Some(i) = i {
            if i < self.top {
                self.top = i;
            } else if i >= self.top + ROWS {
                self.top = i + 1 - ROWS;
            }
        }
    }

    pub(crate) fn input(&mut self, input: &Input) -> AcOut {
        if let Some(x) = self.exceptions.as_mut() {
            return match x.input(input) {
                ExOut::Stay(true) => AcOut::Redraw,
                ExOut::Stay(false) => AcOut::None,
                ExOut::Close => {
                    self.exceptions = None;
                    AcOut::Redraw
                }
                ExOut::Apply([first, caps]) => {
                    self.opts.exceptions = first;
                    self.opts.caps_exceptions = caps;
                    self.exceptions = None;
                    AcOut::Redraw
                }
            };
        }
        let ev = match input {
            Input::Text(s) => self.form.type_text(s),
            Input::Key(Key::Escape, _) => Event::Cancel,
            Input::Key(k, m) => self.form.key(*k, *m),
            Input::Move { x, y } => {
                self.pointer = (*x, *y);
                return if self.form.hover(*x, *y) {
                    AcOut::Redraw
                } else {
                    AcOut::None
                };
            }
            Input::Press { x, y, count, .. } => {
                self.pointer = (*x, *y);
                self.form.press(*x, *y, *count)
            }
            Input::Wheel(d) => {
                let (x, y) = self.pointer;
                if let Some(c) = self.form.get("list") {
                    let r = self.form.dev(c.rect);
                    if x >= r[0] && x < r[0] + r[2] && y >= r[1] && y < r[1] + r[3] {
                        let max = self.entries.len().saturating_sub(ROWS);
                        self.top = if *d > 0.0 {
                            (self.top + 1).min(max)
                        } else {
                            self.top.saturating_sub(1)
                        };
                        return AcOut::Redraw;
                    }
                }
                return AcOut::None;
            }
            Input::Blink => return AcOut::Redraw,
            _ => return AcOut::None,
        };
        self.event(&ev)
    }

    fn event(&mut self, ev: &Event) -> AcOut {
        match ev {
            Event::Cancel | Event::Clicked("cancel") => AcOut::Close,
            Event::Ok | Event::Clicked("ok") => AcOut::Apply(Box::new(self.collect())),
            Event::Clicked("exceptions") => {
                let mut x = ExceptionsDialog::open(
                    &self.strings_title,
                    &self.strings_ex,
                    &self.opts.exceptions,
                    &self.opts.caps_exceptions,
                );
                if let Some((fill, edge)) = self.accent {
                    x.form.set_accent(fill, edge);
                }
                self.exceptions = Some(x);
                AcOut::Redraw
            }
            // Typing in Replace finds the first entry from there, as on the reference.
            Event::Changed("from") => {
                let from = self.form.text("from").to_lowercase();
                let i = self
                    .entries
                    .iter()
                    .position(|e| e.from.to_lowercase() >= from);
                if let Some(i) = i {
                    self.top = i.min(self.entries.len().saturating_sub(ROWS));
                }
                self.sel = i.filter(|&i| self.entries[i].from.to_lowercase() == from);
                if let Some(i) = self.sel {
                    let to = self.entries[i].to.clone();
                    self.form.set_text("to", &to);
                }
                self.sync_buttons();
                AcOut::Redraw
            }
            Event::Changed(_) => {
                self.sync_buttons();
                AcOut::Redraw
            }
            Event::Canvas("list", x, y) if *x >= LIST_W - BAR_W => {
                // The scroll bar: arrows step a row, the track a page.
                let max = self.entries.len().saturating_sub(ROWS);
                self.top = if *y < BAR_W {
                    self.top.saturating_sub(1)
                } else if *y > LIST_H - BAR_W {
                    (self.top + 1).min(max)
                } else if *y < LIST_H / 2.0 {
                    self.top.saturating_sub(ROWS)
                } else {
                    (self.top + ROWS).min(max)
                };
                AcOut::Redraw
            }
            Event::Canvas("list", _, y) => {
                let row = (y / ROW_H).max(0.0) as usize + self.top;
                if row < self.entries.len() {
                    self.select(Some(row));
                    let e = self.entries[row].clone();
                    self.form.set_text("from", &e.from);
                    self.form.set_text("to", &e.to);
                    self.sync_buttons();
                }
                AcOut::Redraw
            }
            Event::Clicked("add") => {
                let from = self.form.text("from");
                let to = self.form.text("to");
                if from.trim().is_empty() || to.is_empty() {
                    return AcOut::Redraw;
                }
                let i = if let Some(i) = self.entries.iter().position(|e| e.from == from) {
                    self.entries[i].to = to;
                    i
                } else {
                    let at = self
                        .entries
                        .iter()
                        .position(|e| e.from.to_lowercase() > from.to_lowercase())
                        .unwrap_or(self.entries.len());
                    self.entries.insert(at, Replacement { from, to });
                    at
                };
                self.select(Some(i));
                self.sync_buttons();
                AcOut::Redraw
            }
            Event::Clicked("delete") => {
                let from = self.form.text("from");
                if let Some(i) = self.entries.iter().position(|e| e.from == from) {
                    self.entries.remove(i);
                    self.form.set_text("from", "");
                    self.form.set_text("to", "");
                    self.sel = None;
                    self.top = self.top.min(self.entries.len().saturating_sub(ROWS));
                }
                self.sync_buttons();
                AcOut::Redraw
            }
            _ => AcOut::Redraw,
        }
    }

    pub(crate) fn paint(&self, p: &mut dyn Painter, window: (f32, f32), caret_on: bool) {
        let k = self.form.scale_k();
        let font = self.form.font();
        let entries = &self.entries;
        let (top, sel) = (self.top, self.sel);
        let mut canvas = |id: &str, r: [f32; 4], p: &mut dyn Painter| {
            if id == "list" {
                paint_list(p, r, k, &font, entries, top, sel);
            }
        };
        self.form.paint(p, window, caret_on, &mut canvas);
        if let Some(x) = &self.exceptions {
            let mut none = |_: &str, _: [f32; 4], _: &mut dyn Painter| {};
            x.form.paint(p, window, caret_on, &mut none);
        }
    }
}

/// The two-column replacement list: white, framed, one grid line under each row and one
/// between the columns; the selected row highlighted.
fn paint_list(
    p: &mut dyn Painter,
    r: [f32; 4],
    k: f32,
    font: &Font<'_>,
    entries: &[Replacement],
    top: usize,
    sel: Option<usize>,
) {
    p.fill_rect(r[0], r[1], r[2], r[3], FRAME);
    p.fill_rect(
        r[0] + 1.0,
        r[1] + 1.0,
        r[2] - 2.0,
        r[3] - 2.0,
        Rgba(255, 255, 255, 255),
    );
    let row_h = (ROW_H * k).round();
    let col = r[0] + (COL2 * k).round();
    let line = font.size * 1.33;
    // The vertical scroll bar of the reference's list (17 px at 125 %).
    let bar = (BAR_W * k).round();
    let inner = r[2] - 2.0 - bar;
    p.push_clip(r[0] + 1.0, r[1] + 1.0, inner, r[3] - 2.0);
    for (n, e) in entries.iter().enumerate().skip(top).take(ROWS) {
        let y = r[1] + 1.0 + (n - top) as f32 * row_h;
        let chosen = sel == Some(n);
        let ink = if chosen {
            p.fill_rect(r[0] + 1.0, y, inner, row_h - 1.0, SEL);
            Rgba(255, 255, 255, 255)
        } else {
            Rgba(0, 0, 0, 255)
        };
        p.fill_rect(r[0] + 1.0, y + row_h - 1.0, inner, 1.0, GRID);
        let ty = (y + (row_h - line) / 2.0).round();
        p.text_font(&e.from, r[0] + (4.0 * k).round(), ty, font, ink);
        p.text_font(&e.to, col + (4.0 * k).round(), ty, font, ink);
    }
    p.fill_rect(col, r[1] + 1.0, 1.0, r[3] - 2.0, GRID);
    p.pop_clip();
    let sx = r[0] + 1.0 + inner;
    p.fill_rect(sx, r[1] + 1.0, bar, r[3] - 2.0, BAR_FACE);
    let n = entries.len().max(1) as f32;
    let track = r[3] - 2.0 - 2.0 * bar;
    let th = (track * ROWS as f32 / n).clamp(8.0 * k, track);
    let ty = r[1] + 1.0 + bar + (track - th) * top as f32 / (n - ROWS as f32).max(1.0);
    p.fill_rect(sx + (6.0 * k).round(), ty, (5.0 * k).round(), th, BAR_THUMB);
    // Arrows: small filled triangles.
    let cx = sx + bar / 2.0;
    for (cy, up) in [
        (r[1] + 1.0 + bar / 2.0, true),
        (r[1] + r[3] - 1.0 - bar / 2.0, false),
    ] {
        for i in 0..3 {
            let half = i as f32 * k;
            let y = if up {
                cy - k + i as f32 * k
            } else {
                cy + k - i as f32 * k
            };
            p.fill_rect(
                (cx - half - k / 2.0).round(),
                y.round(),
                (2.0 * half + k).round(),
                k.max(1.0),
                BAR_ARROW,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dialog() -> AutoCorrectDialog {
        AutoCorrectDialog::open(
            &Strings::load("en-US"),
            &AutoCorrectOptions::default(),
            "English (United States)",
        )
    }

    #[test]
    fn shows_the_options_and_returns_the_changed_ones() {
        let mut d = dialog();
        assert!(d.form.on("capSent") && d.form.on("quotes") && d.form.on("enabled"));
        d.form.set_on("quotes", false);
        d.form.set_on("capSent", false);
        let AcOut::Apply(o) = d.event(&Event::Ok) else {
            panic!("ok applies");
        };
        assert!(!o.smart_quotes && !o.capitalize_sentences && o.dashes);
        assert_eq!(o.entries.len(), AutoCorrectOptions::default().entries.len());
        assert_eq!(d.event(&Event::Cancel), AcOut::Close);
    }

    #[test]
    fn replace_box_finds_entries_and_add_delete_edit_the_list() {
        let mut d = dialog();
        d.form.set_text("from", "(c)");
        d.event(&Event::Changed("from"));
        assert_eq!(d.form.text("to"), "\u{a9}");
        assert!(d.sel.is_some());
        // A new entry, sorted in.
        d.form.set_text("from", "bo");
        d.form.set_text("to", "BlueOffice");
        d.event(&Event::Changed("to"));
        d.event(&Event::Clicked("add"));
        let i = d
            .entries
            .iter()
            .position(|e| e.from == "bo")
            .expect("added");
        assert!(d.entries[i - 1].from.to_lowercase().as_str() <= "bo");
        let AcOut::Apply(o) = d.event(&Event::Ok) else {
            panic!("ok applies");
        };
        assert!(o
            .entries
            .iter()
            .any(|e| e.from == "bo" && e.to == "BlueOffice"));
        d.form.set_text("from", "bo");
        d.event(&Event::Clicked("delete"));
        assert!(!d.entries.iter().any(|e| e.from == "bo"));
    }

    #[test]
    fn title_names_the_language_and_every_option_is_live() {
        let mut d = dialog();
        assert_eq!(d.form.title, "AutoCorrect: English (United States)");
        for id in ["showButtons", "days", "capsLock", "spell", "exceptions"] {
            assert!(d.form.get(id).is_some_and(|c| c.enabled), "{id}");
        }
        d.form.set_on("days", false);
        d.form.set_on("capsLock", false);
        let AcOut::Apply(o) = d.event(&Event::Ok) else {
            panic!("ok applies");
        };
        assert!(!o.capitalize_days && !o.caps_lock && o.spelling_suggestions);
    }

    #[test]
    fn exceptions_dialog_edits_both_lists() {
        let mut d = dialog();
        d.event(&Event::Clicked("exceptions"));
        let x = d.exceptions.as_mut().expect("exceptions open");
        assert_eq!(x.form.sel("list0"), None);
        assert!(!x.form.get("add0").is_some_and(|c| c.enabled));
        assert!(!x.form.get("del0").is_some_and(|c| c.enabled));
        x.form.set_text("e0", "blt.");
        x.event(&Event::Changed("e0"));
        assert!(x.form.get("add0").is_some_and(|c| c.enabled));
        x.event(&Event::Clicked("add0"));
        x.form.set_text("e1", "PDFs");
        x.event(&Event::Clicked("add1"));
        assert!(matches!(
            d.input(&Input::Key(Key::Enter, crate::Mods::default())),
            AcOut::Redraw
        ));
        assert!(d.exceptions.is_none());
        let AcOut::Apply(o) = d.event(&Event::Ok) else {
            panic!("ok applies");
        };
        assert!(o.exceptions.iter().any(|w| w == "blt."));
        assert!(o.caps_exceptions.iter().any(|w| w == "PDFs"));
    }

    #[test]
    fn scroll_bar_steps_the_list() {
        let mut d = dialog();
        d.event(&Event::Canvas("list", LIST_W - 5.0, LIST_H - 5.0));
        assert_eq!(d.top, 1);
        d.event(&Event::Canvas("list", LIST_W - 5.0, 5.0));
        assert_eq!(d.top, 0);
    }

    #[test]
    fn clicking_a_row_fills_the_boxes() {
        let mut d = dialog();
        d.event(&Event::Canvas("list", 10.0, ROW_H * 1.5));
        let e = d.entries[1].clone();
        assert_eq!((d.form.text("from"), d.form.text("to")), (e.from, e.to));
    }
}
