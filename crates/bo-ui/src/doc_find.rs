//! Find and Replace dialog of BlueDoc (Ctrl+H, Ctrl+G), laid out after the classic dialog of
//! a desktop word processor by measurement at 125 %: caption bar with the title, ? and ✕;
//! property-sheet tabs Find / Replace / Go To over a bordered page; labels left of combo
//! boxes that keep the last 20 searches; More >> opens the search options; the bottom row
//! of 107 × 29 px buttons with Find Next as the default. Sizes below are logical pixels
//! (device pixels at 100 %): the dialog is 649 × 238 (811 × 297 measured at 125 %).
use crate::strings::Strings;
use crate::theme::Theme;
use crate::{Key, Mods, Painter, TextMeasure, TextStyle};
use bo_render::{PathSeg, Rgba};

const CAPTION: Rgba = Rgba(243, 243, 243, 255);
const FACE: Rgba = Rgba(240, 240, 240, 255);
const PAGE: Rgba = Rgba(249, 249, 249, 255);
const PAGE_EDGE: Rgba = Rgba(229, 229, 229, 255);
const TAB_OFF: Rgba = Rgba(243, 243, 243, 255);
const FIELD_EDGE: Rgba = Rgba(171, 173, 179, 255);
const BUTTON: Rgba = Rgba(253, 253, 253, 255);
const BUTTON_EDGE: Rgba = Rgba(208, 208, 208, 255);
const DEFAULT_EDGE: Rgba = Rgba(0, 120, 212, 255);
const FRAME: Rgba = Rgba(170, 170, 170, 255);
const GREY: Rgba = Rgba(160, 160, 160, 255);
const WHITE: Rgba = Rgba(255, 255, 255, 255);
const INK: Rgba = Rgba(0, 0, 0, 255);

/// Width of the dialog and its heights, collapsed and with the search options.
const W: f32 = 649.0;
const H: f32 = 238.0;
const H_MORE: f32 = 489.0;
/// Bottom-row button slots (left x) and size.
const SLOTS: [f32; 5] = [21.0, 237.6, 339.0, 440.6, 541.4];
const BW: f32 = 85.6;
const BH: f32 = 23.2;
const BUTTON_Y: f32 = 196.0;
/// Combo boxes: left, width, height; Find what and Replace with tops.
const FIELD_X: f32 = 90.4;
const FIELD_W: f32 = 539.2;
const FIELD_H: f32 = 20.8;
const FIND_Y: f32 = 69.6;
const WITH_Y: f32 = 132.8;
/// History kept in the combo boxes.
pub(crate) const HISTORY: usize = 20;

/// Labels: key and en-US fallback (`&` marks the access key, underlined).
const LABELS: [(&str, &str); 53] = [
    ("findDlg.title", "Find and Replace"),
    ("findDlg.tabFind", "Fin&d"),
    ("findDlg.tabReplace", "Re&place"),
    ("findDlg.tabGoTo", "&Go To"),
    ("findDlg.findWhat", "Fi&nd what:"),
    ("findDlg.replaceWith", "Replace w&ith:"),
    ("findDlg.more", "&More >>"),
    ("findDlg.less", "<< &Less"),
    ("findDlg.replace", "&Replace"),
    ("findDlg.replaceAll", "Replace &All"),
    ("findDlg.findNext", "&Find Next"),
    ("findDlg.cancel", "Cancel"),
    ("findDlg.close", "Close"),
    ("findDlg.goToWhat", "G&o to what:"),
    ("findDlg.enterPage", "&Enter page number:"),
    ("findDlg.previous", "Pre&vious"),
    ("findDlg.next", "Nex&t"),
    ("findDlg.goTo", "Go &To"),
    ("findDlg.searchOptions", "Search Options"),
    ("findDlg.search", "Searc&h:"),
    ("findDlg.all", "All"),
    ("findDlg.matchCase", "Matc&h case"),
    ("findDlg.wholeWords", "Find wh&ole words only"),
    ("findDlg.wildcards", "&Use wildcards"),
    ("findDlg.soundsLike", "Sounds li&ke (English)"),
    ("findDlg.wordForms", "Find all &word forms (English)"),
    ("findDlg.prefix", "Match prefi&x"),
    ("findDlg.suffix", "Match suffix"),
    ("findDlg.punctuation", "Ignore punctuation charac&ters"),
    ("findDlg.whiteSpace", "Ignore white-space characters"),
    ("findDlg.replaceSection", "Replace"),
    ("findDlg.findSection", "Find"),
    ("findDlg.format", "F&ormat"),
    ("findDlg.special", "Sp&ecial"),
    ("findDlg.noFormatting", "No Formatting"),
    ("findDlg.reading", "Reading Highlight"),
    ("findDlg.findIn", "Find In"),
    (
        "findDlg.hint",
        "Enter + and \u{2013} to move relative to the current location. Example: +4 will move forward four items.",
    ),
    ("findDlg.notFound", "The search item was not found."),
    ("findDlg.replaced", "{{count}} replacements made."),
    ("findDlg.found", "Match {{n}} of {{total}}."),
    ("findDlg.enterSection", "&Enter section number:"),
    ("findDlg.enterLine", "&Enter line number:"),
    ("findDlg.enterBookmark", "&Enter bookmark name:"),
    ("findDlg.enterComment", "&Enter comment number:"),
    ("findDlg.enterHeading", "&Enter heading number:"),
    ("findDlg.enterFootnote", "&Enter footnote number:"),
    ("findDlg.enterEndnote", "&Enter endnote number:"),
    ("findDlg.enterField", "&Enter field name:"),
    ("findDlg.enterTable", "&Enter table number:"),
    ("findDlg.enterGraphic", "&Enter graphic number:"),
    ("findDlg.enterEquation", "&Enter equation number:"),
    ("findDlg.enterObject", "&Enter object name:"),
];
const L_TITLE: usize = 0;
const L_TABS: usize = 1;
const L_FIND_WHAT: usize = 4;
const L_WITH: usize = 5;
const L_MORE: usize = 6;
const L_LESS: usize = 7;
const L_REPLACE: usize = 8;
const L_REPLACE_ALL: usize = 9;
const L_FIND_NEXT: usize = 10;
const L_CANCEL: usize = 11;
const L_CLOSE: usize = 12;
const L_GO_WHAT: usize = 13;
const L_ENTER: usize = 14;
const L_PREVIOUS: usize = 15;
const L_NEXT: usize = 16;
const L_GO_TO: usize = 17;
const L_OPTIONS: usize = 18;
const L_SEARCH: usize = 19;
const L_ALL: usize = 20;
const L_CHECKS: usize = 21;
const L_REPLACE_SECTION: usize = 30;
const L_FIND_SECTION: usize = 31;
const L_FORMAT: usize = 32;
const L_SPECIAL: usize = 33;
const L_NO_FORMAT: usize = 34;
const L_READING: usize = 35;
const L_FIND_IN: usize = 36;
const L_HINT: usize = 37;
pub(crate) const L_NOT_FOUND: usize = 38;
pub(crate) const L_REPLACED: usize = 39;
pub(crate) const L_FOUND: usize = 40;

/// Go to what: the reference's targets in its order.
const GO_WHAT: [(&str, &str); 13] = [
    ("findDlg.page", "Page"),
    ("findDlg.section", "Section"),
    ("findDlg.line", "Line"),
    ("findDlg.bookmark", "Bookmark"),
    ("findDlg.comment", "Comment"),
    ("findDlg.footnote", "Footnote"),
    ("findDlg.endnote", "Endnote"),
    ("findDlg.field", "Field"),
    ("findDlg.table", "Table"),
    ("findDlg.graphic", "Graphic"),
    ("findDlg.equation", "Equation"),
    ("findDlg.object", "Object"),
    ("findDlg.heading", "Heading"),
];
/// Rows the Go to what list shows (16 px each in its 82 px box), and its scroll bar width.
const GO_ROWS: usize = 5;
const GO_BAR: f32 = 16.0;

/// Go to what entries BlueDoc carries out: all of them (round 4 added Footnote, Endnote,
/// Field, Table, Graphic, Equation and Object).
pub(crate) fn go_supported(i: usize) -> bool {
    i < GO_WHAT.len()
}

/// Go to what entry of Bookmark (its box lists the bookmark names).
pub(crate) const GO_BOOKMARK: usize = 3;

/// Label of the number box for Go to what entry `i`.
fn enter_label(i: usize) -> usize {
    match i {
        1 => 41,
        2 => 42,
        GO_BOOKMARK => 43,
        4 => 44,
        12 => 45,
        5..=11 => 41 + i,
        _ => L_ENTER,
    }
}

/// Special ▾ entries: label and the code typed into the box (the engine's search codes).
const SPECIAL: [(&str, &str, &str); 9] = [
    ("findDlg.sp.para", "Paragraph Mark", "^p"),
    ("findDlg.sp.tab", "Tab Character", "^t"),
    ("findDlg.sp.any", "Any Character", "^?"),
    ("findDlg.sp.digit", "Any Digit", "^#"),
    ("findDlg.sp.letter", "Any Letter", "^$"),
    ("findDlg.sp.emDash", "Em Dash", "^+"),
    ("findDlg.sp.enDash", "En Dash", "^="),
    ("findDlg.sp.lineBreak", "Manual Line Break", "^l"),
    ("findDlg.sp.pageBreak", "Manual Page Break", "^m"),
];

/// A text box of the dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Field {
    Find,
    With,
    Page,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Button {
    More,
    Replace,
    ReplaceAll,
    FindNext,
    Cancel,
    Reading,
    FindIn,
    Previous,
    Next,
    Format,
    Special,
    NoFormat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Hit {
    Tab(usize),
    Close,
    Field(Field),
    Drop(Field),
    Button(Button),
    Check(usize),
    GoWhat(usize),
    /// Scroll the Go to what list by rows.
    GoScroll(isize),
    Item(usize),
}

/// What the window carries out for a press or key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FindAction {
    None,
    Redraw,
    Close,
    FindNext,
    Replace,
    ReplaceAll,
    /// Go to the page typed (`3`, `+2`, `-1`).
    GoTo(usize, String),
}

/// An open drop-down list under a combo box or the Special button.
#[derive(Debug, Clone)]
struct DropList {
    field: Option<Field>,
    items: Vec<String>,
    rect: [f32; 4],
}

#[derive(Debug, Clone, Default)]
struct Edit {
    text: Vec<char>,
    caret: usize,
    /// Everything selected (typing replaces it), as when a box gets the focus.
    all: bool,
}

impl Edit {
    fn new(s: &str) -> Self {
        let text: Vec<char> = s.chars().collect();
        Self {
            caret: text.len(),
            all: !text.is_empty(),
            text,
        }
    }

    fn value(&self) -> String {
        self.text.iter().collect()
    }

    fn set(&mut self, s: &str) {
        *self = Self::new(s);
    }

    fn insert(&mut self, s: &str) {
        if self.all {
            self.text.clear();
            self.caret = 0;
            self.all = false;
        }
        for c in s.chars().filter(|c| !c.is_control()) {
            self.text.insert(self.caret, c);
            self.caret += 1;
        }
    }
}

/// A measured label: text without `&`, width, and the access key's underline (x, width).
#[derive(Debug, Clone, Default)]
struct Label {
    text: String,
    w: f32,
    key: Option<(f32, f32)>,
    /// Access key letter, lower case.
    letter: Option<char>,
}

/// The open dialog.
pub(crate) struct FindDialog {
    /// Tab shown: 0 Find, 1 Replace, 2 Go To.
    pub(crate) tab: usize,
    pub(crate) more: bool,
    pub(crate) match_case: bool,
    pub(crate) whole_word: bool,
    pub(crate) wildcards: bool,
    /// Sounds like, word forms, match prefix, match suffix, ignore punctuation, ignore white
    /// space (round 10; check boxes 3–8).
    pub(crate) extra: [bool; 6],
    find: Edit,
    with: Edit,
    page: Edit,
    focus: Field,
    go_what: usize,
    /// Bookmark names, for the Bookmark target's ▾ list.
    pub(crate) bookmarks: Vec<String>,
    /// First Go to what row shown (the list scrolls).
    pub(crate) go_scroll: usize,
    /// Last pointer position (the wheel scrolls the list under it).
    pointer: (f32, f32),
    history: Vec<String>,
    drop: Option<DropList>,
    hover: Option<Hit>,
    /// Status line (matches, replacements, not found).
    pub(crate) message: String,
    scale: f32,
    x0: f32,
    y0: f32,
    labels: Vec<Label>,
    go_labels: Vec<Label>,
    special_labels: Vec<String>,
    hint: Vec<String>,
    /// Caret offsets of the three boxes (device px from the text start).
    carets: [f32; 3],
}

fn parse_label(s: &str) -> (String, Option<usize>) {
    let mut out = String::new();
    let mut key = None;
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '&' {
            if chars.peek() == Some(&'&') {
                chars.next();
                out.push('&');
            } else if key.is_none() {
                key = Some(out.chars().count());
            }
            continue;
        }
        out.push(c);
    }
    (out, key)
}

fn inside(r: [f32; 4], x: f32, y: f32) -> bool {
    x >= r[0] && x < r[0] + r[2] && y >= r[1] && y < r[1] + r[3]
}

impl FindDialog {
    pub(crate) fn new(tab: usize, find: &str, history: Vec<String>) -> Self {
        Self {
            tab: tab.min(2),
            more: false,
            match_case: false,
            whole_word: false,
            wildcards: false,
            extra: [false; 6],
            find: Edit::new(find),
            with: Edit::default(),
            page: Edit::default(),
            focus: if tab == 2 { Field::Page } else { Field::Find },
            go_what: 0,
            go_scroll: 0,
            bookmarks: Vec::new(),
            pointer: (-1.0, -1.0),
            history,
            drop: None,
            hover: None,
            message: String::new(),
            scale: 1.0,
            x0: 0.0,
            y0: 0.0,
            labels: Vec::new(),
            go_labels: Vec::new(),
            special_labels: Vec::new(),
            hint: Vec::new(),
            carets: [0.0; 3],
        }
    }

    pub(crate) fn query(&self) -> String {
        self.find.value()
    }

    pub(crate) fn replacement(&self) -> String {
        self.with.value()
    }

    /// Search options for the engine's `find` / `replace` commands.
    pub(crate) fn options(&self) -> serde_json::Value {
        serde_json::json!({
            "matchCase": self.match_case && self.check_on(0),
            "wholeWord": self.whole_word && self.check_on(1),
            "wildcards": self.wildcards && self.check_on(2),
            "soundsLike": self.extra[0] && self.check_on(3),
            "wordForms": self.extra[1] && self.check_on(4),
            "matchPrefix": self.extra[2] && self.check_on(5),
            "matchSuffix": self.extra[3] && self.check_on(6),
            "ignorePunctuation": self.extra[4] && self.check_on(7),
            "ignoreSpace": self.extra[5] && self.check_on(8),
        })
    }

    /// Whether search option check box `i` (0 match case … 8 ignore white space) can be
    /// used with the others ticked: wildcards leave only Match case; Sounds like and Find all
    /// word forms exclude each other and the word / case options.
    fn check_on(&self, i: usize) -> bool {
        let sounds = self.extra[0];
        let forms = self.extra[1];
        match i {
            0 | 2 => !sounds && !forms,
            1 | 5 | 6 | 7 | 8 => !self.wildcards && !sounds && !forms,
            3 => !self.wildcards && !forms,
            4 => !self.wildcards && !sounds,
            _ => false,
        }
    }

    /// Tick or clear search option check box `i`.
    fn toggle_check(&mut self, i: usize) {
        match i {
            0 => self.match_case = !self.match_case,
            1 => self.whole_word = !self.whole_word,
            2 => self.wildcards = !self.wildcards,
            3..=8 => self.extra[i - 3] = !self.extra[i - 3],
            _ => {}
        }
    }

    /// The searches typed, most recent first (kept by the window between openings).
    pub(crate) fn history(&self) -> &[String] {
        &self.history
    }

    /// Remember a search (and a replacement) in the combo boxes' history.
    pub(crate) fn remember(&mut self) {
        for s in [self.find.value(), self.with.value()] {
            if s.is_empty() {
                continue;
            }
            self.history.retain(|h| *h != s);
            self.history.insert(0, s);
        }
        self.history.truncate(HISTORY);
    }

    fn px(&self, v: f32) -> f32 {
        (v * self.scale).round()
    }

    fn style(&self) -> TextStyle {
        // The dialog font: Segoe UI 9 pt as the reference draws it (its labels measure
        // 56 px for "Find what:" at 125 %; the host applies the Windows text size factor
        // on top of this size).
        TextStyle {
            size: self.px(10.0),
            bold: false,
        }
    }

    fn height(&self) -> f32 {
        if self.more {
            H_MORE
        } else {
            H
        }
    }

    fn r(&self, x: f32, y: f32, w: f32, h: f32) -> [f32; 4] {
        [
            self.x0 + self.px(x),
            self.y0 + self.px(y),
            self.px(w),
            self.px(h),
        ]
    }

    /// Centre the dialog in a window of `size` at display `scale`, and measure its texts.
    pub(crate) fn layout(
        &mut self,
        m: &dyn TextMeasure,
        strings: &Strings,
        size: (f32, f32),
        scale: f32,
    ) {
        self.scale = scale;
        self.x0 = ((size.0 - self.px(W)) / 2.0).round().max(0.0);
        self.y0 = (size.1 * 0.18).round().max(0.0);
        let st = self.style();
        let label = |raw: &str| -> Label {
            let (text, key) = parse_label(raw);
            let w = m.measure(&text, st).0;
            let mut l = Label {
                w,
                ..Label::default()
            };
            if let Some(k) = key {
                let before: String = text.chars().take(k).collect();
                let ch: String = text.chars().skip(k).take(1).collect();
                l.key = Some((m.measure(&before, st).0, m.measure(&ch, st).0));
                l.letter = ch.chars().next().map(|c| c.to_ascii_lowercase());
            }
            l.text = text;
            l
        };
        let get = |key: &str, fallback: &str| -> String {
            let v = strings.get(key);
            if v == key {
                fallback.to_owned()
            } else {
                v.to_owned()
            }
        };
        self.labels = LABELS.iter().map(|(k, f)| label(&get(k, f))).collect();
        self.go_labels = GO_WHAT.iter().map(|(k, f)| label(&get(k, f))).collect();
        self.special_labels = SPECIAL
            .iter()
            .map(|(k, f, code)| format!("{}  {code}", get(k, f)))
            .collect();
        // The Go To hint wraps under the page number box.
        let hint = self.labels[L_HINT].text.clone();
        let width = self.px(FIELD_W - 120.0);
        let mut lines = Vec::new();
        let mut line = String::new();
        for word in hint.split_whitespace() {
            let cand = if line.is_empty() {
                word.to_owned()
            } else {
                format!("{line} {word}")
            };
            if !line.is_empty() && m.measure(&cand, st).0 > width {
                lines.push(std::mem::take(&mut line));
                line = word.to_owned();
            } else {
                line = cand;
            }
        }
        lines.push(line);
        self.hint = lines;
        for (i, e) in [&self.find, &self.with, &self.page].iter().enumerate() {
            let before: String = e.text[..e.caret].iter().collect();
            self.carets[i] = m.measure(&before, st).0;
        }
    }

    fn tab_rects(&self) -> Vec<[f32; 4]> {
        let mut x = self.x0 + self.px(9.0);
        (0..3)
            .map(|i| {
                let w = self
                    .labels
                    .get(L_TABS + i)
                    .map_or(self.px(62.0), |l| l.w + self.px(36.0));
                let sel = i == self.tab;
                let top = if sel { 37.6 } else { 39.2 };
                let r = [x, self.y0 + self.px(top), w, self.px(59.2 - top)];
                x += w;
                r
            })
            .collect()
    }

    fn page_rect(&self) -> [f32; 4] {
        let bottom = self.height() - 9.0;
        self.r(7.2, 59.2, 632.0, bottom - 59.2)
    }

    fn field_rect(&self, f: Field) -> [f32; 4] {
        match f {
            Field::Find => self.r(FIELD_X, FIND_Y, FIELD_W, FIELD_H),
            Field::With => self.r(FIELD_X, WITH_Y, FIELD_W, FIELD_H),
            Field::Page => self.r(210.0, 86.0, 419.6, FIELD_H),
        }
    }

    fn drop_rect(&self, f: Field) -> [f32; 4] {
        let r = self.field_rect(f);
        let w = self.px(21.0);
        [r[0] + r[2] - w - 1.0, r[1] + 1.0, w, r[3] - 2.0]
    }

    fn close_rect(&self) -> [f32; 4] {
        self.r(W - 46.0, 0.0, 46.0, 30.0)
    }

    fn fields(&self) -> Vec<Field> {
        match self.tab {
            0 => vec![Field::Find],
            1 => vec![Field::Find, Field::With],
            _ => vec![Field::Page],
        }
    }

    /// Bottom-row buttons of the tab: button, slot, label, enabled, default.
    fn buttons(&self) -> Vec<(Button, [f32; 4], usize, bool, bool)> {
        let slot = |i: usize| self.r(SLOTS[i], BUTTON_Y, BW, BH);
        let more = if self.more { L_LESS } else { L_MORE };
        let mut out = match self.tab {
            0 => vec![
                (Button::More, slot(0), more, true, false),
                (Button::Reading, slot(1), L_READING, false, false),
                (Button::FindIn, slot(2), L_FIND_IN, false, false),
                (Button::FindNext, slot(3), L_FIND_NEXT, true, true),
                (Button::Cancel, slot(4), L_CANCEL, true, false),
            ],
            1 => vec![
                (Button::More, slot(0), more, true, false),
                (Button::Replace, slot(1), L_REPLACE, true, false),
                (Button::ReplaceAll, slot(2), L_REPLACE_ALL, true, false),
                (Button::FindNext, slot(3), L_FIND_NEXT, true, true),
                (Button::Cancel, slot(4), L_CANCEL, true, false),
            ],
            _ => {
                let typed = !self.page.text.is_empty();
                vec![
                    (Button::Previous, slot(2), L_PREVIOUS, !typed, false),
                    (
                        Button::Next,
                        slot(3),
                        if typed { L_GO_TO } else { L_NEXT },
                        true,
                        true,
                    ),
                    (Button::Cancel, slot(4), L_CLOSE, true, false),
                ]
            }
        };
        if self.more && self.tab < 2 {
            let y = H_MORE - 44.0;
            out.push((
                Button::Format,
                self.r(19.0, y, BW, BH),
                L_FORMAT,
                false,
                false,
            ));
            out.push((
                Button::Special,
                self.r(19.0 + BW + 8.0, y, BW, BH),
                L_SPECIAL,
                true,
                false,
            ));
            out.push((
                Button::NoFormat,
                self.r(19.0 + 2.0 * (BW + 8.0), y, BW, BH),
                L_NO_FORMAT,
                false,
                false,
            ));
        }
        out
    }

    /// Search option check boxes: label index, rectangle, enabled.
    fn checks(&self) -> Vec<(usize, [f32; 4], bool)> {
        if !self.more || self.tab == 2 {
            return Vec::new();
        }
        let mut out = Vec::new();
        for i in 0..5 {
            let enabled = self.check_on(i);
            out.push((
                L_CHECKS + i,
                self.r(19.0, 279.0 + i as f32 * 19.2, 300.0, 18.0),
                enabled,
            ));
        }
        for (k, row) in [(5, 0.0), (6, 1.0), (7, 3.0), (8, 4.0)] {
            out.push((
                L_CHECKS + k,
                self.r(342.0, 279.0 + row * 19.2, 240.0, 18.0),
                self.check_on(k),
            ));
        }
        out
    }

    fn checked(&self, label: usize) -> bool {
        match label - L_CHECKS {
            0 => self.match_case,
            1 => self.whole_word && !self.wildcards,
            2 => self.wildcards,
            i @ 3..=8 => self.extra[i - 3],
            _ => false,
        }
    }

    /// Go to what rows in view: entry index and rectangle.
    fn go_rects(&self) -> Vec<(usize, [f32; 4])> {
        (self.go_scroll..(self.go_scroll + GO_ROWS).min(GO_WHAT.len()))
            .map(|i| {
                let row = (i - self.go_scroll) as f32;
                (i, self.r(20.0, 87.0 + row * 16.0, 172.0 - GO_BAR, 16.0))
            })
            .collect()
    }

    /// The box has a ▾ list: the search boxes (history) and Go To's for Bookmark (names).
    fn has_drop(&self, f: Field) -> bool {
        f != Field::Page || self.go_what == GO_BOOKMARK
    }

    /// Choose Go to what entry `i` (tests).
    #[cfg(test)]
    pub(crate) fn set_go_what(&mut self, i: usize) {
        self.go_what = i;
    }

    fn go_box(&self) -> [f32; 4] {
        self.r(19.0, 86.0, 174.0, 82.0)
    }

    /// Scroll bar of the list: up arrow, down arrow, track.
    fn go_bar(&self) -> [[f32; 4]; 3] {
        let x = 19.0 + 174.0 - 1.0 - GO_BAR;
        [
            self.r(x, 87.0, GO_BAR, 16.0),
            self.r(x, 87.0 + 80.0 - 16.0, GO_BAR, 16.0),
            self.r(x, 87.0, GO_BAR, 80.0),
        ]
    }

    fn go_scroll_by(&mut self, rows: isize) -> bool {
        let max = GO_WHAT.len().saturating_sub(GO_ROWS);
        let next = self.go_scroll.saturating_add_signed(rows).min(max);
        std::mem::replace(&mut self.go_scroll, next) != next
    }

    /// The wheel over the Go to what list scrolls it (`dy` > 0: up).
    pub(crate) fn wheel(&mut self, dy: f32) -> bool {
        let (x, y) = self.pointer;
        if self.tab != 2 || !inside(self.go_box(), x, y) {
            return false;
        }
        self.go_scroll_by(if dy > 0.0 { -1 } else { 1 })
    }

    fn hit(&self, x: f32, y: f32) -> Option<Hit> {
        if let Some(d) = &self.drop {
            if inside(d.rect, x, y) {
                let row = ((y - d.rect[1]) / self.px(20.0)).floor().max(0.0) as usize;
                return (row < d.items.len()).then_some(Hit::Item(row));
            }
        }
        if inside(self.close_rect(), x, y) {
            return Some(Hit::Close);
        }
        if let Some(i) = self.tab_rects().iter().position(|r| inside(*r, x, y)) {
            return Some(Hit::Tab(i));
        }
        for f in self.fields() {
            if self.has_drop(f) && inside(self.drop_rect(f), x, y) {
                return Some(Hit::Drop(f));
            }
            if inside(self.field_rect(f), x, y) {
                return Some(Hit::Field(f));
            }
        }
        if let Some((b, ..)) = self
            .buttons()
            .into_iter()
            .find(|(_, r, _, on, _)| *on && inside(*r, x, y))
        {
            return Some(Hit::Button(b));
        }
        if let Some((l, ..)) = self
            .checks()
            .into_iter()
            .find(|(_, r, on)| *on && inside(*r, x, y))
        {
            return Some(Hit::Check(l));
        }
        if self.tab == 2 {
            let [up, down, track] = self.go_bar();
            if inside(up, x, y) {
                return Some(Hit::GoScroll(-1));
            }
            if inside(down, x, y) {
                return Some(Hit::GoScroll(1));
            }
            if inside(track, x, y) {
                let mid = track[1] + track[3] / 2.0;
                return Some(Hit::GoScroll(if y < mid { -1 } else { 1 }));
            }
            if let Some((i, _)) = self
                .go_rects()
                .into_iter()
                .find(|(i, r)| go_supported(*i) && inside(*r, x, y))
            {
                return Some(Hit::GoWhat(i));
            }
        }
        None
    }

    pub(crate) fn hover(&mut self, x: f32, y: f32) -> bool {
        self.pointer = (x, y);
        let h = self.hit(x, y);
        std::mem::replace(&mut self.hover, h) != h
    }

    fn edit(&mut self, f: Field) -> &mut Edit {
        match f {
            Field::Find => &mut self.find,
            Field::With => &mut self.with,
            Field::Page => &mut self.page,
        }
    }

    fn set_tab(&mut self, tab: usize) {
        self.tab = tab.min(2);
        self.drop = None;
        self.message.clear();
        self.focus = if self.tab == 2 {
            Field::Page
        } else {
            Field::Find
        };
        let f = self.focus;
        let e = self.edit(f);
        e.all = !e.text.is_empty();
        e.caret = e.text.len();
    }

    fn open_drop(&mut self, field: Option<Field>) {
        let (items, anchor) = if let Some(f) = field {
            let items = if f == Field::Page {
                self.bookmarks.clone()
            } else {
                self.history.clone()
            };
            (items, self.field_rect(f))
        } else {
            let r = self
                .buttons()
                .into_iter()
                .find(|b| b.0 == Button::Special)
                .map_or([0.0; 4], |b| b.1);
            (self.special_labels.clone(), r)
        };
        if items.is_empty() {
            return;
        }
        let h = self.px(20.0) * items.len() as f32 + 2.0;
        let w = if field.is_some() {
            anchor[2]
        } else {
            self.px(220.0)
        };
        self.drop = Some(DropList {
            field,
            items,
            rect: [anchor[0], anchor[1] + anchor[3], w, h],
        });
    }

    fn button(&mut self, b: Button) -> FindAction {
        match b {
            Button::More => {
                self.more = !self.more;
                FindAction::Redraw
            }
            Button::FindNext => FindAction::FindNext,
            Button::Replace => FindAction::Replace,
            Button::ReplaceAll => FindAction::ReplaceAll,
            Button::Cancel => FindAction::Close,
            Button::Next => FindAction::GoTo(
                self.go_what,
                if self.page.text.is_empty() {
                    "+1".into()
                } else {
                    self.page.value()
                },
            ),
            Button::Previous => FindAction::GoTo(self.go_what, "-1".into()),
            Button::Special => {
                self.open_drop(None);
                FindAction::Redraw
            }
            Button::Reading | Button::FindIn | Button::Format | Button::NoFormat => {
                FindAction::None
            }
        }
    }

    pub(crate) fn press(&mut self, x: f32, y: f32) -> FindAction {
        let hit = self.hit(x, y);
        let drop = self.drop.take();
        match hit {
            Some(Hit::Item(i)) => {
                if let Some(d) = drop {
                    if let Some(f) = d.field {
                        let v = d.items.get(i).cloned().unwrap_or_default();
                        self.edit(f).set(&v);
                        self.focus = f;
                    } else {
                        let code = SPECIAL.get(i).map_or("", |s| s.2);
                        let f = if self.focus == Field::Page {
                            Field::Find
                        } else {
                            self.focus
                        };
                        let e = self.edit(f);
                        e.all = false;
                        e.insert(code);
                    }
                }
                FindAction::Redraw
            }
            Some(Hit::Close) => FindAction::Close,
            Some(Hit::Tab(i)) => {
                self.set_tab(i);
                FindAction::Redraw
            }
            Some(Hit::Drop(f)) => {
                if drop.as_ref().and_then(|d| d.field) != Some(f) {
                    self.focus = f;
                    self.open_drop(Some(f));
                }
                FindAction::Redraw
            }
            Some(Hit::Field(f)) => {
                self.focus = f;
                let e = self.edit(f);
                e.all = false;
                e.caret = e.text.len();
                FindAction::Redraw
            }
            Some(Hit::Button(b)) => self.button(b),
            Some(Hit::Check(l)) => {
                self.toggle_check(l - L_CHECKS);
                FindAction::Redraw
            }
            Some(Hit::GoWhat(i)) => {
                self.go_what = i;
                FindAction::Redraw
            }
            Some(Hit::GoScroll(rows)) => {
                self.go_scroll_by(rows);
                FindAction::Redraw
            }
            None => FindAction::Redraw,
        }
    }

    pub(crate) fn text(&mut self, s: &str) {
        let f = self.focus;
        self.edit(f).insert(s);
    }

    pub(crate) fn key(&mut self, key: Key, m: Mods) -> FindAction {
        if key == Key::Escape {
            if self.drop.take().is_some() {
                return FindAction::Redraw;
            }
            return FindAction::Close;
        }
        // Ctrl+Tab / Ctrl+PageDown / Ctrl+PageUp change tab, as in a property sheet.
        if m.ctrl && matches!(key, Key::Tab | Key::PageDown | Key::PageUp) {
            let back = m.shift || key == Key::PageUp;
            let t = if back {
                (self.tab + 2) % 3
            } else {
                (self.tab + 1) % 3
            };
            self.set_tab(t);
            return FindAction::Redraw;
        }
        // Alt+access key: the box or button whose label underlines it.
        if let (true, Key::Letter(c)) = (m.alt, key) {
            return self.access(c.to_ascii_lowercase());
        }
        if key == Key::Enter {
            if self.drop.take().is_some() {
                return FindAction::Redraw;
            }
            return match self.tab {
                2 => self.button(Button::Next),
                _ => FindAction::FindNext,
            };
        }
        if key == Key::Tab {
            let fields = self.fields();
            let i = fields.iter().position(|f| *f == self.focus).unwrap_or(0);
            let n = fields.len();
            self.focus = fields[if m.shift {
                (i + n - 1) % n
            } else {
                (i + 1) % n
            }];
            let f = self.focus;
            let e = self.edit(f);
            e.all = !e.text.is_empty();
            return FindAction::Redraw;
        }
        if key == Key::Down && m.alt && self.focus != Field::Page {
            self.open_drop(Some(self.focus));
            return FindAction::Redraw;
        }
        let f = self.focus;
        let e = self.edit(f);
        match key {
            Key::Left => {
                e.all = false;
                e.caret = e.caret.saturating_sub(1);
            }
            Key::Right => {
                e.all = false;
                e.caret = (e.caret + 1).min(e.text.len());
            }
            Key::Home => {
                e.all = false;
                e.caret = 0;
            }
            Key::End => {
                e.all = false;
                e.caret = e.text.len();
            }
            Key::Backspace | Key::Delete if e.all => {
                e.text.clear();
                e.caret = 0;
                e.all = false;
            }
            Key::Backspace if e.caret > 0 => {
                e.caret -= 1;
                e.text.remove(e.caret);
            }
            Key::Delete if e.caret < e.text.len() => {
                e.text.remove(e.caret);
            }
            Key::Letter('a') if m.ctrl => e.all = true,
            _ => return FindAction::None,
        }
        FindAction::Redraw
    }

    fn access(&mut self, c: char) -> FindAction {
        let has = |l: usize, s: &Self| s.labels.get(l).and_then(|x| x.letter) == Some(c);
        for (i, l) in (L_TABS..L_TABS + 3).enumerate() {
            if has(l, self) {
                self.set_tab(i);
                return FindAction::Redraw;
            }
        }
        if has(L_FIND_WHAT, self) && self.tab < 2 {
            self.focus = Field::Find;
            return FindAction::Redraw;
        }
        if has(L_WITH, self) && self.tab == 1 {
            self.focus = Field::With;
            return FindAction::Redraw;
        }
        if let Some(b) = self
            .buttons()
            .into_iter()
            .find(|(_, _, l, on, _)| *on && has(*l, self))
        {
            return self.button(b.0);
        }
        if let Some((l, ..)) = self
            .checks()
            .into_iter()
            .find(|(l, _, on)| *on && has(*l, self))
        {
            self.toggle_check(l - L_CHECKS);
            return FindAction::Redraw;
        }
        FindAction::None
    }

    /// Text of label `i` (measured), for the window's messages.
    pub(crate) fn label(&self, i: usize) -> String {
        self.labels
            .get(i)
            .map_or_else(|| LABELS[i].1.to_owned(), |l| l.text.clone())
    }

    /// Centre of the bottom-row button with label `label` (tests).
    #[cfg(test)]
    pub(crate) fn button_center(&self, label: usize) -> Option<(f32, f32)> {
        self.buttons()
            .into_iter()
            .find(|b| b.2 == label)
            .map(|(_, r, ..)| (r[0] + r[2] / 2.0, r[1] + r[3] / 2.0))
    }

    /// Centre of tab `i` (tests).
    #[cfg(test)]
    pub(crate) fn tab_center(&self, i: usize) -> (f32, f32) {
        let r = self.tab_rects()[i];
        (r[0] + r[2] / 2.0, r[1] + r[3] / 2.0)
    }

    /// Centre of the Find what box's ▾ (tests).
    #[cfg(test)]
    pub(crate) fn drop_center(&self) -> (f32, f32) {
        let r = self.drop_rect(Field::Find);
        (r[0] + r[2] / 2.0, r[1] + r[3] / 2.0)
    }

    /// Centre of the first entry of the open list (tests).
    #[cfg(test)]
    pub(crate) fn first_item(&self) -> Option<(f32, f32)> {
        self.drop
            .as_ref()
            .map(|d| (d.rect[0] + self.px(20.0), d.rect[1] + self.px(10.0)))
    }

    /// Go to what rows in view (tests): entry index and rectangle.
    #[cfg(test)]
    pub(crate) fn go_rows(&self) -> Vec<(usize, [f32; 4])> {
        self.go_rects()
    }

    /// The list box and its down arrow (tests).
    #[cfg(test)]
    pub(crate) fn go_list(&self) -> ([f32; 4], [f32; 4]) {
        (self.go_box(), self.go_bar()[1])
    }

    /// Centre of the Match case box (tests).
    #[cfg(test)]
    pub(crate) fn match_case_center(&self) -> Option<(f32, f32)> {
        self.checks()
            .first()
            .map(|(_, r, _)| (r[0] + self.px(6.0), r[1] + r[3] / 2.0))
    }

    fn draw_label(&self, p: &mut dyn Painter, l: &Label, x: f32, y: f32, color: Rgba) {
        let st = self.style();
        p.text(&l.text, x, y, st, color);
        if let Some((kx, kw)) = l.key {
            let uy = (y + st.size * 1.18).round();
            p.fill_rect(x + kx, uy, kw.max(1.0), self.scale.max(1.0).round(), color);
        }
    }

    fn text_y(&self, top: f32, h: f32) -> f32 {
        (top + (h - self.style().size * 1.33) / 2.0).round()
    }

    fn arrow(&self, p: &mut dyn Painter, cx: f32, cy: f32, color: Rgba) {
        let k = self.px(3.5);
        p.path(
            &[
                PathSeg::M(cx - k, cy - k / 2.0),
                PathSeg::L(cx, cy + k / 2.0),
                PathSeg::L(cx + k, cy - k / 2.0),
            ],
            None,
            Some((color, self.scale.max(1.0))),
        );
    }

    #[allow(clippy::too_many_lines)]
    pub(crate) fn paint(&self, p: &mut dyn Painter, t: &Theme, caret_on: bool) {
        let [x, y, w, h] = self.r(0.0, 0.0, W, self.height());
        // Soft shadow and frame of a top-level dialog.
        for (k, a) in [(6.0, 12u8), (3.0, 20), (1.0, 28)] {
            let o = self.px(k);
            p.fill_rect(
                x - o,
                y - o + self.px(3.0),
                w + 2.0 * o,
                h + 2.0 * o,
                Rgba(0, 0, 0, a),
            );
        }
        p.fill_rect(x, y, w, h, FRAME);
        p.fill_rect(x + 1.0, y + 1.0, w - 2.0, h - 2.0, FACE);
        let cap = self.px(30.0);
        p.fill_rect(x + 1.0, y + 1.0, w - 2.0, cap - 1.0, CAPTION);
        if let Some(l) = self.labels.get(L_TITLE) {
            p.text(
                &l.text,
                x + self.px(10.0),
                self.text_y(y, cap),
                self.style(),
                INK,
            );
        }
        // ? and ✕ caption buttons (own drawing).
        let c = self.close_rect();
        if self.hover == Some(Hit::Close) {
            p.fill_rect(
                c[0],
                c[1] + 1.0,
                c[2] - 1.0,
                c[3] - 1.0,
                Rgba(196, 43, 28, 255),
            );
        }
        let (cx, cy, k) = (c[0] + c[2] / 2.0, c[1] + c[3] / 2.0, self.px(5.0));
        let xc = if self.hover == Some(Hit::Close) {
            WHITE
        } else {
            INK
        };
        p.path(
            &[
                PathSeg::M(cx - k, cy - k),
                PathSeg::L(cx + k, cy + k),
                PathSeg::M(cx + k, cy - k),
                PathSeg::L(cx - k, cy + k),
            ],
            None,
            Some((xc, self.scale.max(1.0))),
        );
        let q = self.style();
        p.text(
            "?",
            c[0] - self.px(46.0) + self.px(19.0),
            self.text_y(c[1], c[3]),
            q,
            INK,
        );
        // Tabs over the bordered page; the shown tab joins the page.
        let pg = self.page_rect();
        p.fill_rect(pg[0], pg[1], pg[2], pg[3], PAGE_EDGE);
        p.fill_rect(pg[0] + 1.0, pg[1] + 1.0, pg[2] - 2.0, pg[3] - 2.0, PAGE);
        for (i, r) in self.tab_rects().into_iter().enumerate() {
            let sel = i == self.tab;
            p.fill_rect(r[0], r[1], r[2], r[3] + 1.0, PAGE_EDGE);
            let fill = if sel {
                PAGE
            } else if self.hover == Some(Hit::Tab(i)) {
                Rgba(250, 250, 250, 255)
            } else {
                TAB_OFF
            };
            let bottom = if sel { 2.0 } else { 1.0 };
            p.fill_rect(
                r[0] + 1.0,
                r[1] + 1.0,
                r[2] - 2.0,
                r[3] - 2.0 + bottom,
                fill,
            );
            if let Some(l) = self.labels.get(L_TABS + i) {
                self.draw_label(
                    p,
                    l,
                    r[0] + (r[2] - l.w) / 2.0,
                    self.text_y(r[1], r[3]),
                    INK,
                );
            }
        }
        // Boxes with their labels on the left.
        let field_labels: Vec<(Field, usize)> = match self.tab {
            0 => vec![(Field::Find, L_FIND_WHAT)],
            1 => vec![(Field::Find, L_FIND_WHAT), (Field::With, L_WITH)],
            _ => vec![(Field::Page, enter_label(self.go_what))],
        };
        for (f, l) in field_labels {
            let r = self.field_rect(f);
            if let Some(lb) = self.labels.get(l) {
                if f == Field::Page {
                    self.draw_label(p, lb, r[0], r[1] - self.px(18.0), INK);
                } else {
                    self.draw_label(p, lb, self.x0 + self.px(19.0), self.text_y(r[1], r[3]), INK);
                }
            }
            p.fill_rect(r[0], r[1], r[2], r[3], FIELD_EDGE);
            p.fill_rect(r[0] + 1.0, r[1] + 1.0, r[2] - 2.0, r[3] - 2.0, WHITE);
            let e = match f {
                Field::Find => &self.find,
                Field::With => &self.with,
                Field::Page => &self.page,
            };
            let tx = r[0] + self.px(4.0);
            let ty = self.text_y(r[1], r[3]);
            let value: String = e.text.iter().collect();
            let focused = self.focus == f;
            if focused && e.all && !value.is_empty() {
                let vw = self.carets[f as usize].max(self.px(4.0));
                p.fill_rect(
                    tx,
                    r[1] + self.px(3.0),
                    vw,
                    r[3] - self.px(6.0),
                    DEFAULT_EDGE,
                );
                p.text(&value, tx, ty, self.style(), WHITE);
            } else {
                p.text(&value, tx, ty, self.style(), INK);
            }
            if focused && caret_on && !e.all {
                p.fill_rect(
                    tx + self.carets[f as usize],
                    r[1] + self.px(3.0),
                    self.scale.max(1.0).round(),
                    r[3] - self.px(6.0),
                    INK,
                );
            }
            if self.has_drop(f) {
                let d = self.drop_rect(f);
                let on = self.hover == Some(Hit::Drop(f));
                p.fill_rect(d[0], d[1], 1.0, d[3], Rgba(210, 210, 210, 255));
                if on {
                    p.fill_rect(d[0] + 1.0, d[1], d[2] - 1.0, d[3], Rgba(229, 241, 251, 255));
                } else {
                    p.fill_rect(d[0] + 1.0, d[1], d[2] - 1.0, d[3], BUTTON);
                }
                self.arrow(
                    p,
                    d[0] + d[2] / 2.0,
                    d[1] + d[3] / 2.0,
                    Rgba(94, 94, 94, 255),
                );
            }
        }
        if self.tab == 2 {
            if let Some(l) = self.labels.get(L_GO_WHAT) {
                self.draw_label(p, l, self.x0 + self.px(19.0), self.y0 + self.px(68.0), INK);
            }
            let lr = self.go_box();
            p.fill_rect(lr[0], lr[1], lr[2], lr[3], FIELD_EDGE);
            p.fill_rect(lr[0] + 1.0, lr[1] + 1.0, lr[2] - 2.0, lr[3] - 2.0, WHITE);
            // Rows clipped to the box; entries BlueDoc cannot go to yet are greyed.
            p.push_clip(lr[0] + 1.0, lr[1] + 1.0, lr[2] - 2.0, lr[3] - 2.0);
            for (i, r) in self.go_rects() {
                let Some(l) = self.go_labels.get(i) else {
                    continue;
                };
                let sel = i == self.go_what;
                if sel {
                    p.fill_rect(r[0], r[1], r[2], r[3], DEFAULT_EDGE);
                }
                let color = if sel {
                    WHITE
                } else if go_supported(i) {
                    INK
                } else {
                    GREY
                };
                self.draw_label(p, l, r[0] + self.px(3.0), self.text_y(r[1], r[3]), color);
            }
            // Scroll bar: track, arrows and a thumb sized to the rows in view.
            let [up, down, track] = self.go_bar();
            p.fill_rect(track[0], track[1], track[2], track[3], FACE);
            let n = GO_WHAT.len() as f32;
            let inner = track[3] - up[3] - down[3];
            let th = (inner * GO_ROWS as f32 / n).max(self.px(8.0));
            let ty = up[1]
                + up[3]
                + (inner - th) * self.go_scroll as f32 / (n - GO_ROWS as f32).max(1.0);
            p.fill_rect(
                track[0] + self.px(4.0),
                ty,
                track[2] - self.px(8.0),
                th,
                Rgba(205, 205, 205, 255),
            );
            for (r, dir) in [(up, -1.0_f32), (down, 1.0)] {
                let (cx, cy, k) = (r[0] + r[2] / 2.0, r[1] + r[3] / 2.0, self.px(3.0));
                p.path(
                    &[
                        PathSeg::M(cx - k, cy - dir * k / 2.0),
                        PathSeg::L(cx, cy + dir * k / 2.0),
                        PathSeg::L(cx + k, cy - dir * k / 2.0),
                        PathSeg::Z,
                    ],
                    Some(Rgba(96, 96, 96, 255)),
                    None,
                );
            }
            p.pop_clip();
            let hx = self.x0 + self.px(210.0);
            let mut hy = self.y0 + self.px(114.0);
            for line in &self.hint {
                p.text(line, hx, hy, self.style(), INK);
                hy += (self.style().size * 1.33).round();
            }
        }
        // Status line (matches, replacements).
        if !self.message.is_empty() {
            p.text(
                &self.message,
                self.x0 + self.px(19.0),
                self.y0 + self.px(170.0),
                self.style(),
                t.text_muted,
            );
        }
        // Bottom-row buttons; the default one framed in the accent blue.
        for (b, r, l, on, default) in self.buttons() {
            let hovered = on && self.hover == Some(Hit::Button(b));
            let edge = if default { DEFAULT_EDGE } else { BUTTON_EDGE };
            p.fill_rect(r[0], r[1], r[2], r[3], edge);
            let inset = if default { 2.0 } else { 1.0 };
            let face = if hovered {
                Rgba(229, 241, 251, 255)
            } else {
                BUTTON
            };
            p.fill_rect(
                r[0] + inset,
                r[1] + inset,
                r[2] - 2.0 * inset,
                r[3] - 2.0 * inset,
                face,
            );
            if let Some(lb) = self.labels.get(l) {
                let color = if on { INK } else { GREY };
                let drop = matches!(
                    b,
                    Button::Format | Button::Special | Button::Reading | Button::FindIn
                );
                let extra = if drop { self.px(10.0) } else { 0.0 };
                let tx = (r[0] + (r[2] - lb.w - extra) / 2.0).round();
                self.draw_label(p, lb, tx, self.text_y(r[1], r[3]), color);
                if drop {
                    self.arrow(p, tx + lb.w + self.px(7.0), r[1] + r[3] / 2.0, color);
                }
            }
        }
        // Search options (More >>).
        if self.more && self.tab < 2 {
            let sep_y = self.y0 + self.px(229.0);
            p.fill_rect(
                pg[0] + self.px(10.0),
                sep_y,
                pg[2] - self.px(20.0),
                1.0,
                PAGE_EDGE,
            );
            if let Some(l) = self.labels.get(L_OPTIONS) {
                self.draw_label(p, l, self.x0 + self.px(19.0), self.y0 + self.px(238.0), INK);
            }
            if let Some(l) = self.labels.get(L_SEARCH) {
                self.draw_label(p, l, self.x0 + self.px(19.0), self.y0 + self.px(263.0), INK);
            }
            let cb = self.r(66.0, 261.0, 78.0, FIELD_H);
            p.fill_rect(cb[0], cb[1], cb[2], cb[3], FIELD_EDGE);
            p.fill_rect(cb[0] + 1.0, cb[1] + 1.0, cb[2] - 2.0, cb[3] - 2.0, WHITE);
            if let Some(l) = self.labels.get(L_ALL) {
                self.draw_label(p, l, cb[0] + self.px(4.0), self.text_y(cb[1], cb[3]), INK);
            }
            self.arrow(p, cb[0] + cb[2] - self.px(10.0), cb[1] + cb[3] / 2.0, INK);
            for (l, r, on) in self.checks() {
                let s = self.px(13.0);
                let by = r[1] + (r[3] - s) / 2.0;
                let edge = if on {
                    Rgba(51, 51, 51, 255)
                } else {
                    Rgba(204, 204, 204, 255)
                };
                p.fill_rect(r[0], by, s, s, edge);
                p.fill_rect(r[0] + 1.0, by + 1.0, s - 2.0, s - 2.0, WHITE);
                if self.checked(l) {
                    p.path(
                        &[
                            PathSeg::M(r[0] + s * 0.22, by + s * 0.52),
                            PathSeg::L(r[0] + s * 0.42, by + s * 0.72),
                            PathSeg::L(r[0] + s * 0.80, by + s * 0.28),
                        ],
                        None,
                        Some((INK, self.scale.max(1.0) * 1.4)),
                    );
                }
                if let Some(lb) = self.labels.get(l) {
                    let color = if on { INK } else { GREY };
                    self.draw_label(
                        p,
                        lb,
                        r[0] + s + self.px(6.0),
                        self.text_y(r[1], r[3]),
                        color,
                    );
                }
            }
            let section = if self.tab == 1 {
                L_REPLACE_SECTION
            } else {
                L_FIND_SECTION
            };
            let sy = self.y0 + self.px(H_MORE - 60.0);
            if let Some(l) = self.labels.get(section) {
                self.draw_label(p, l, self.x0 + self.px(19.0), sy, INK);
                p.fill_rect(
                    self.x0 + self.px(19.0) + l.w + self.px(8.0),
                    sy + (self.style().size * 0.7).round(),
                    pg[0] + pg[2] - self.px(10.0) - (self.x0 + self.px(27.0) + l.w),
                    1.0,
                    PAGE_EDGE,
                );
            }
        }
        // An open history / Special list.
        if let Some(d) = &self.drop {
            let r = d.rect;
            p.fill_rect(r[0], r[1], r[2], r[3], Rgba(130, 135, 144, 255));
            p.fill_rect(r[0] + 1.0, r[1] + 1.0, r[2] - 2.0, r[3] - 2.0, WHITE);
            let rh = self.px(20.0);
            for (i, item) in d.items.iter().enumerate() {
                let iy = r[1] + 1.0 + i as f32 * rh;
                let hovered = self.hover == Some(Hit::Item(i));
                if hovered {
                    p.fill_rect(r[0] + 1.0, iy, r[2] - 2.0, rh, DEFAULT_EDGE);
                }
                let color = if hovered { WHITE } else { INK };
                p.text(
                    item,
                    r[0] + self.px(4.0),
                    self.text_y(iy, rh),
                    self.style(),
                    color,
                );
            }
        }
    }
}
