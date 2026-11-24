//! BlueDoc dialogs of parity round 3, laid out after the reference word processor's dialogs
//! measured at 125 % (coordinates below are those reference pixels): Paste Special, Set
//! Default Paste, Create New Style from Formatting, Table of Contents, Footnote and Endnote,
//! Create Source, Source Manager, Mark Index Entry, Index, Cross-reference, Caption, Compare
//! Documents, Language, Envelopes and Labels, Start Enforcing Protection. They reuse the
//! self-drawn form of BlueSheet (`sheet_form.rs`); option buttons are drawn here.
//! Every dialog ends in engine commands through the JSON bridge (ADR-0008).
#![allow(clippy::match_same_arms)]
#[allow(dead_code, clippy::duplicate_mod)]
#[path = "sheet_form.rs"]
mod form;

use form::Event;
use form::{Form, Kind};

#[path = "doc_forms5.rs"]
mod r5;

#[path = "doc_options6.rs"]
pub(crate) mod options6;

#[path = "doc_forms7.rs"]
mod r7;

#[path = "doc_autocorrect8.rs"]
pub(crate) mod autocorrect8;

#[path = "doc_findfmt11.rs"]
pub(crate) mod findfmt11;

#[path = "doc_tprops11.rs"]
mod tprops11;

#[path = "doc_forms12.rs"]
pub(crate) mod r12;

#[path = "doc_forms13.rs"]
pub(crate) mod r13;

#[path = "doc_forms14.rs"]
pub(crate) mod r14;

#[path = "doc_forms15.rs"]
pub(crate) mod r15;

#[path = "doc_forms16.rs"]
pub(crate) mod r16;

#[path = "doc_options20.rs"]
pub(crate) mod options20;
#[path = "doc_forms17.rs"]
pub(crate) mod r17;
#[path = "doc_fill18.rs"]
pub(crate) mod r18;
#[path = "doc_forms22.rs"]
pub(crate) mod r22;
#[path = "doc_forms23.rs"]
pub(crate) mod r23;
pub(crate) use r5::{open_manage_styles, ManagedStyle};
pub(crate) use r7::INSPECT;

use crate::strings::Strings;
use crate::{Font, Input, Key, Painter, TextMeasure};
use bo_render::{PathSeg, Rgba};
use serde_json::{json, Value};

/// Which dialog a form is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DocFormKind {
    PasteSpecial,
    PasteDefaults,
    NewStyle,
    Toc,
    Notes,
    NewSource,
    Sources,
    MarkEntry,
    Index,
    CrossRef,
    Caption,
    Compare,
    Language,
    Envelopes,
    Protect,
    Unprotect,
    Recipients,
    TocOptions,
    /// Insert › Date & Time (round 4).
    DateTime,
    /// View › Zoom (round 4).
    Zoom,
    /// Mailings › Rules › Ask (round 5).
    AskRule,
    /// Mailings › Rules › Fill-in.
    FillInRule,
    /// Mailings › Rules › If…Then…Else.
    IfRule,
    /// Mailings › Rules › Next Record If.
    NextIfRule,
    /// Mailings › Rules › Set Bookmark.
    SetRule,
    /// Mailings › Rules › Skip Record If.
    SkipIfRule,
    /// Mailings › Match Fields.
    MatchFields,
    /// Mailings › Find Recipient (Find in Field).
    FindRecipient,
    /// Styles pane › Manage Styles.
    ManageStyles,
    /// Picture / Shape Format › Alt Text (round 7).
    AltText,
    /// File › Info › Protect Document › Encrypt with Password.
    Encrypt,
    /// File › Info › Check for Issues › Inspect Document.
    Inspect,
    /// File › Info › Check for Issues › Check Compatibility.
    Compatibility,
    /// Find and Replace › Format ▾ › Font… (round 11).
    FindFont,
    /// Find and Replace › Format ▾ › Paragraph….
    FindParagraph,
    /// Find and Replace › Format ▾ › Tabs….
    FindTabs,
    /// Find and Replace › Format ▾ › Language….
    FindLanguage,
    /// Find and Replace › Format ▾ › Frame….
    FindFrame,
    /// Find and Replace › Format ▾ › Style….
    FindStyle,
    /// Table Properties with its five tabs (round 11).
    TableProps,
    /// Table Properties › Options… (default cell margins).
    TableOptions,
    /// Insert › Table › Insert Table….
    InsertTable,
    /// Table Properties › Positioning… (round 12).
    TablePosition,
    /// Borders and Shading with its Borders, Page Border and Shading tabs (round 12).
    BordersShading,
    /// Create New Building Block (Quick Tables › Save Selection…, round 12).
    NewBlock,
    /// Home › Paragraph dialog launcher: Indents and Spacing / Line and Page Breaks (round 13).
    Paragraph,
    /// Paragraph › Tabs… (round 13).
    Tabs,
    /// Layout › Page Setup with its Margins, Paper and Layout tabs (round 13).
    PageSetup,
    /// Border and Shading Options (Borders and Shading › Options…, round 13).
    BorderOptions,
    /// Style Pane Options (Styles pane › Options…, round 13).
    PaneOptions,
    /// Line Numbers (Page Setup › Layout › Line Numbers…, round 13).
    LineNumbers,
    /// Index and Tables › Table of Figures (round 14).
    Tof,
    /// Index and Tables › Table of Authorities (round 14).
    Toa,
    /// References › Mark Citation (round 14).
    MarkCitation,
    /// Design › Watermark › Custom Watermark… (Printed Watermark, round 14).
    Watermark,
    /// Index and Tables › Table of Figures › Options… (round 14).
    TofOptions,
    /// Mark Citation › Category… (Edit Category, round 14).
    Categories,
    /// More Colors… of a colour picker (Colors, round 15).
    Colors,
    /// Design › Colors › Customize Colors… (Create New Theme Colors, round 15).
    ThemeColors,
    /// Design › Fonts › Customize Fonts… (Create New Theme Fonts, round 15).
    ThemeFonts,
    /// Review › Markup launcher: Track Changes Options (round 16).
    TrackOptions,
    /// Track Changes Options › Advanced Options… (round 16).
    AdvTrackOptions,
    /// Mailings › Finish & Merge › Edit Individual Documents… (round 16).
    MergeNew,
    /// Mailings › Finish & Merge › Print Documents… (round 16).
    MergePrint,
    /// Mailings › Check for Errors (round 16).
    CheckErrors,
    /// The result of Check for Errors (round 16).
    MergeReport,
    /// Mailings › Select Recipients › Type a New List… (New Address List, round 16).
    NewList,
    /// Mailings › Finish & Merge › Send Email Messages… (Merge to E-mail, round 16).
    MergeEmail,
    /// New Address List › Find… (Find Entry, round 16).
    NlFind,
    /// New Address List › Customize Columns… (Customize Address List, round 16).
    NlColumns,
    /// Design › Page Color › Fill Effects… (layout round 8).
    FillEffects,
    /// Insert › Links › Bookmark (round 17).
    Bookmark,
    /// Insert › Text › Object › Object… (round 17).
    InsertObject,
    /// View › Macros › View Macros (round 17, view only).
    Macros,
    /// Insert › Media › Online Videos (round 17).
    OnlineVideo,
    /// Macros › Edit: a module's source, read only (round 17).
    MacroSource,
    /// Insert › Icons: BlueOffice's own icon library (round 17).
    Icons,
    /// New / Modify Style › Format ▾ › Numbering… (round 17).
    StyleNumbering,
    /// New / Modify Style › Format ▾ › Shortcut key… (Customize Keyboard, round 17).
    CustomizeKeyboard,
    /// New / Modify Style › Format ▾ › Text Effects… (round 17).
    StyleEffects,
    /// New / Modify Style › Format ▾ › Frame… (round 17).
    StyleFrame,
    /// Insert › Pictures › Online Pictures… (round 17).
    OnlinePicture,
    /// Insert › Object › Display as icon › Change Icon… (round 22).
    ChangeIcon,
    /// File › Options › Add-ins › Manage ▾ › Go… (round 22).
    AddinList,
    /// Home › Font dialog with its Font and Advanced tabs (round 23).
    Font23,
    /// Numbering ▾ › Set Numbering Value… (round 23).
    SetNumbering,
    /// Bullets ▾ › Define New Bullet… (round 23).
    DefineBullet,
    /// Numbering ▾ › Define New Number Format… (round 23).
    DefineNumber,
    /// Multilevel List ▾ › Define New Multilevel List… (round 23).
    DefineMultilevel,
    /// Home › Sort… (Sort Text, round 23).
    SortText,
}

/// What the owner does after an input.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum FormOut {
    None,
    Redraw,
    Close,
    /// Run these engine commands (JSON), then close the dialog when `true`.
    Run(Vec<Value>, bool),
    /// Paste the clipboard as `keep` / `merge` / `text`.
    Paste(&'static str),
    /// New default paste modes (within a document, between documents, other programs).
    Defaults([usize; 3]),
    /// Open another dialog over this one (closing this one).
    Open(DocFormKind),
    /// Ask the host for a file to compare with (`original`: the original's slot).
    PickFile(bool),
    /// A new document from this envelope / label sheet command (opened in a new window).
    NewDocument(Value),
    /// Open another dialog over this one, keeping this one to come back to.
    Stash(DocFormKind),
    /// TOC Options chosen: the style → level pairs (back to the kept dialog).
    TocMap(String),
    /// Compare two files (the original `None`: this document) into a new window.
    Compare(Option<(String, String)>, (String, String)),
    /// Date and Time › Set As Default: this format (index into [`DATE_PICTURES`]).
    DateDefault(usize),
    /// Document Inspector: inspect these categories, or remove everything of one.
    Inspect(Vec<String>, Option<String>),
    /// A Find … dialog of Find and Replace › Format ▾: its part, the `FindFormat` fields and
    /// the words shown after "Format:" (round 11).
    FindFormat(&'static str, Value, String),
    /// Table Properties › Previous / Next Row / Column: run these edits, move the caret
    /// (`tableEdit` select part) and show the new row / column (round 11).
    TableMove(Vec<Value>, &'static str),
}

/// A radio button drawn on a canvas control.
struct Radio {
    id: &'static str,
    group: &'static str,
    label: String,
    on: bool,
    enabled: bool,
}

/// An open BlueDoc dialog.
pub(crate) struct DocForm {
    pub(crate) kind: DocFormKind,
    form: Form,
    radios: Vec<Radio>,
    pointer: (f32, f32),
    /// Ids behind list rows (cross-reference targets, source tags, style ids …).
    pub(crate) ids: Vec<String>,
    /// Style ids of the New Style dialog's drop-downs.
    pub(crate) style_ids: Vec<String>,
    /// Files picked for Compare (original, revised): name and base64.
    pub(crate) files: [Option<(String, String)>; 2],
    /// Preview lines of the TOC / Index dialogs and the source preview.
    preview: Vec<String>,
    /// Measured width (dialog font) of each preview line's entry text.
    label_w: Vec<f32>,
    /// TOC Options: style → level pairs (`TOC \t`), when set.
    pub(crate) toc_map: Option<String>,
    /// Create New Style used as Modify Style: the style changed (`styleModify`).
    pub(crate) modify: Option<String>,
    /// Manage Styles: the document's styles and the texts of its preview.
    managed: Vec<r5::ManagedStyle>,
    manage_text: Vec<String>,
    /// Three-state check boxes of the Find … dialogs (round 11).
    tris: Vec<findfmt11::Tri>,
    /// Translated words a dialog's OK puts together (round 11).
    pub(crate) words: Vec<(String, String)>,
    /// Borders and Shading state (round 12).
    bs: Option<Box<r12::Bs>>,
    /// Paragraph / Tabs / Page Setup / New Style state (round 13).
    r13: Option<Box<r13::R13>>,
    /// Colors dialog state (round 15).
    r15: Option<Box<r15::ColorsState>>,
    /// Fill Effects state (layout round 8).
    r18: Option<Box<r18::Fill18>>,
    /// Bookmark / Object / Macros state (round 17).
    r17: Option<Box<r17::R17>>,
    /// The canvas a press started a drag on (Colors › Custom field / bar, round 17).
    drag: Option<&'static str>,
}

/// Reference ids of `ST_NumberFormat` values offered for notes (§17.18.59).
const NOTE_FORMATS: [(&str, &str); 6] = [
    ("decimal", "1, 2, 3, …"),
    ("lowerLetter", "a, b, c, …"),
    ("upperLetter", "A, B, C, …"),
    ("lowerRoman", "i, ii, iii, …"),
    ("upperRoman", "I, II, III, …"),
    ("chicago", "*, †, ‡, §, …"),
];

/// Proofing languages offered by the Language dialog (BCP 47 tags, own list).
pub(crate) const LANGUAGES: [(&str, &str); 14] = [
    ("en-US", "English (United States)"),
    ("en-GB", "English (United Kingdom)"),
    ("vi-VN", "Vietnamese"),
    ("fr-FR", "French (France)"),
    ("de-DE", "German (Germany)"),
    ("es-ES", "Spanish (Spain)"),
    ("it-IT", "Italian (Italy)"),
    ("ja-JP", "Japanese"),
    ("ko-KR", "Korean"),
    ("zh-CN", "Chinese (PRC)"),
    ("ru-RU", "Russian"),
    ("pt-BR", "Portuguese (Brazil)"),
    ("nl-NL", "Dutch (Netherlands)"),
    ("th-TH", "Thai"),
];

/// Source types of the Create Source dialog (§22.6.2.63 ST_SourceType).
const SOURCE_TYPES: [&str; 6] = [
    "Book",
    "BookSection",
    "JournalArticle",
    "ArticleInAPeriodical",
    "Report",
    "InternetSite",
];

/// Caption labels (the reference's built-in three).
pub(crate) const CAPTION_LABELS: [&str; 3] = ["Figure", "Table", "Equation"];

/// Level boxes of TOC Options (Heading 1–9).
const TOC_LEVEL_IDS: [&str; 9] = ["l1", "l2", "l3", "l4", "l5", "l6", "l7", "l8", "l9"];
const TOC_STYLE_LABELS: [&str; 9] = ["s1", "s2", "s3", "s4", "s5", "s6", "s7", "s8", "s9"];

/// Tab leaders of the TOC / Index dialogs.
const LEADERS: [(&str, &str); 4] = [
    ("dot", "......."),
    ("hyphen", "-------"),
    ("underscore", "_______"),
    ("none", "(none)"),
];

/// Context a dialog is built from.
#[derive(Default)]
pub(crate) struct FormContext {
    /// Styles (id, name, paragraph?).
    pub(crate) styles: Vec<(String, String, bool)>,
    /// Font at the caret and size in half points.
    pub(crate) font: String,
    pub(crate) size: u32,
    /// Proofing language at the caret.
    pub(crate) lang: Option<String>,
    pub(crate) no_proof: bool,
    /// Cross-reference targets (id, label).
    pub(crate) targets: Vec<(String, String)>,
    /// Document sources (tag, one-line description, cited).
    pub(crate) sources: Vec<(String, String, bool)>,
    /// Selected text (Mark Entry main entry, envelope delivery address).
    pub(crate) selection: String,
    /// The paste defaults.
    pub(crate) paste_defaults: [usize; 3],
    /// Font families (Find Font, round 11).
    pub(crate) fonts: Vec<String>,
    /// The table at the caret (Table Properties, round 11).
    pub(crate) table: Option<bo_edit::TableState>,
    /// More state for round-12 dialogs (borders, shading, palette; see `doc_r12`).
    pub(crate) extra: Value,
    /// Lengths in inches (else centimetres).
    pub(crate) inches: bool,
    /// Mail merge recipients: field names and rows.
    pub(crate) recipients: (Vec<String>, Vec<Vec<String>>),
    /// Local date and time now (year, month, day, weekday 0 = Sunday, hour, minute,
    /// second) for the Date and Time formats.
    pub(crate) now: [i32; 7],
    /// The view's zoom (1.0 = 100 %).
    pub(crate) zoom: f32,
    /// Date and Time format chosen with Set As Default (index into [`DATE_PICTURES`]).
    pub(crate) date_default: usize,
    /// Match Fields: address field → recipient column chosen before.
    pub(crate) match_fields: std::collections::BTreeMap<String, String>,
}

/// Date and Time formats offered, in the reference's order (`\@` pictures, §17.16.4.1).
pub(crate) const DATE_PICTURES: [&str; 17] = [
    "M/d/yyyy",
    "dddd, MMMM d, yyyy",
    "MMMM d, yyyy",
    "M/d/yy",
    "yyyy-MM-dd",
    "d-MMM-yy",
    "M.d.yyyy",
    "MMM. d, yy",
    "d MMMM yyyy",
    "MMMM yy",
    "MMM-yy",
    "M/d/yyyy h:mm am/pm",
    "M/d/yyyy h:mm:ss am/pm",
    "h:mm am/pm",
    "h:mm:ss am/pm",
    "HH:mm",
    "HH:mm:ss",
];

/// Date `now` written with picture `pic` (the subset of §17.16.4.1 the list uses).
pub(crate) fn format_date(pic: &str, now: [i32; 7]) -> String {
    const MONTHS: [&str; 12] = [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ];
    const DAYS: [&str; 7] = [
        "Sunday",
        "Monday",
        "Tuesday",
        "Wednesday",
        "Thursday",
        "Friday",
        "Saturday",
    ];
    let [y, mo, d, wd, h, mi, se] = now;
    let month = MONTHS[usize::try_from(mo - 1).unwrap_or(0).min(11)];
    let day = DAYS[usize::try_from(wd).unwrap_or(0).min(6)];
    let chars: Vec<char> = pic.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if pic[pic.char_indices().nth(i).map_or(0, |(b, _)| b)..].starts_with("am/pm") {
            out.push_str(if h < 12 { "AM" } else { "PM" });
            i += 5;
            continue;
        }
        let mut n = 1;
        while i + n < chars.len() && chars[i + n] == c {
            n += 1;
        }
        match (c, n) {
            ('y', 4) => out.push_str(&y.to_string()),
            ('y', _) => out.push_str(&format!("{:02}", y % 100)),
            ('M', 1) => out.push_str(&mo.to_string()),
            ('M', 2) => out.push_str(&format!("{mo:02}")),
            ('M', 3) => out.push_str(&month[..3]),
            ('M', _) => out.push_str(month),
            ('d', 1) => out.push_str(&d.to_string()),
            ('d', 2) => out.push_str(&format!("{d:02}")),
            ('d', 3) => out.push_str(&day[..3]),
            ('d', _) => out.push_str(day),
            ('h', _) => out.push_str(&(if h % 12 == 0 { 12 } else { h % 12 }).to_string()),
            ('H', _) => out.push_str(&format!("{h:02}")),
            ('m', _) => out.push_str(&format!("{mi:02}")),
            ('s', _) => out.push_str(&format!("{se:02}")),
            _ => {
                for _ in 0..n {
                    out.push(c);
                }
            }
        }
        i += n;
    }
    out
}

impl DocForm {
    fn new(kind: DocFormKind, title: &str, size: (f32, f32), tabs: Vec<String>) -> Self {
        // Checked boxes and option buttons in the reference dialogs' blue (#005FB8 measured
        // at 125 %, round 14), not BlueSheet's green.
        let mut form = Form::new(title, size, tabs);
        form.set_accent(Rgba(0x00, 0x5f, 0xb8, 255), Rgba(0x00, 0x4c, 0x93, 255));
        Self {
            kind,
            form,
            radios: Vec::new(),
            pointer: (0.0, 0.0),
            ids: Vec::new(),
            style_ids: Vec::new(),
            files: [None, None],
            preview: Vec::new(),
            label_w: Vec::new(),
            toc_map: None,
            modify: None,
            managed: Vec::new(),
            manage_text: Vec::new(),
            tris: Vec::new(),
            words: Vec::new(),
            bs: None,
            r13: None,
            r15: None,
            r18: None,
            r17: None,
            drag: None,
        }
    }

    fn label(&mut self, id: &'static str, r: [f32; 4], text: &str) {
        self.form.add(id, None, r, Kind::Label(text.to_owned()));
    }

    fn label_on(&mut self, page: usize, id: &'static str, r: [f32; 4], text: &str) {
        self.form
            .add(id, Some(page), r, Kind::Label(text.to_owned()));
    }

    fn group(&mut self, id: &'static str, r: [f32; 4], text: &str) {
        self.form.add(id, None, r, Kind::Group(text.to_owned()));
    }

    fn edit(&mut self, id: &'static str, r: [f32; 4], v: &str) {
        self.form.add(
            id,
            None,
            r,
            Kind::Edit {
                value: v.chars().collect(),
                caret: v.chars().count(),
            },
        );
    }

    fn edit_on(&mut self, page: usize, id: &'static str, r: [f32; 4], v: &str) {
        self.form.add(
            id,
            Some(page),
            r,
            Kind::Edit {
                value: v.chars().collect(),
                caret: v.chars().count(),
            },
        );
    }

    fn check(&mut self, id: &'static str, r: [f32; 4], label: &str, on: bool) {
        self.form.add(
            id,
            None,
            r,
            Kind::Check {
                label: label.to_owned(),
                on,
            },
        );
    }

    fn check_on(&mut self, page: usize, id: &'static str, r: [f32; 4], label: &str, on: bool) {
        self.form.add(
            id,
            Some(page),
            r,
            Kind::Check {
                label: label.to_owned(),
                on,
            },
        );
    }

    fn combo(&mut self, id: &'static str, r: [f32; 4], items: Vec<String>, sel: usize) {
        self.form.add(id, None, r, Kind::Combo { items, sel });
    }

    fn list(&mut self, id: &'static str, r: [f32; 4], items: Vec<String>, sel: Option<usize>) {
        self.form
            .add(id, None, r, Kind::List { items, sel, top: 0 });
    }

    fn spin(&mut self, id: &'static str, r: [f32; 4], value: i32, min: i32, max: i32) {
        self.form.add(id, None, r, Kind::Spin { value, min, max });
    }

    fn button(&mut self, id: &'static str, r: [f32; 4], label: &str, default: bool) {
        self.form.add(
            id,
            None,
            r,
            Kind::Button {
                label: label.to_owned(),
                default,
            },
        );
    }

    fn button_on(&mut self, page: usize, id: &'static str, r: [f32; 4], label: &str) {
        self.form.add(
            id,
            Some(page),
            r,
            Kind::Button {
                label: label.to_owned(),
                default: false,
            },
        );
    }

    fn canvas(&mut self, id: &'static str, r: [f32; 4]) {
        self.form.add(id, None, r, Kind::Canvas);
    }

    fn radio(&mut self, id: &'static str, group: &'static str, r: [f32; 4], label: &str, on: bool) {
        self.form.add(id, None, r, Kind::Canvas);
        self.radios.push(Radio {
            id,
            group,
            label: label.to_owned(),
            on,
            enabled: true,
        });
    }

    fn radio_on(
        &mut self,
        page: usize,
        id: &'static str,
        group: &'static str,
        r: [f32; 4],
        label: &str,
        on: bool,
    ) {
        self.form.add(id, Some(page), r, Kind::Canvas);
        self.radios.push(Radio {
            id,
            group,
            label: label.to_owned(),
            on,
            enabled: true,
        });
    }

    fn disable(&mut self, id: &str) {
        self.form.enable(id, false);
        if let Some(r) = self.radios.iter_mut().find(|r| r.id == id) {
            r.enabled = false;
        }
    }

    /// The radio button of `group` that is on.
    pub(crate) fn chosen(&self, group: &str) -> Option<&'static str> {
        self.radios
            .iter()
            .find(|r| r.group == group && r.on)
            .map(|r| r.id)
    }

    pub(crate) fn set_radio(&mut self, id: &str) {
        let Some(group) = self.radios.iter().find(|r| r.id == id).map(|r| r.group) else {
            return;
        };
        for r in &mut self.radios {
            if r.group == group {
                r.on = r.id == id;
            }
        }
    }

    fn ok_cancel(&mut self, s: &Strings) {
        self.form
            .ok_cancel(&t(s, "common.ok", "OK"), &t(s, "common.cancel", "Cancel"));
    }

    fn close_only(&mut self, s: &Strings) {
        let (w, h) = self.form.size;
        self.button(
            "cancel",
            [w - 131.0, h - 48.0, 112.0, 28.0],
            &t(s, "docR3.close", "Close"),
            true,
        );
    }

    /// The dialog's title (Create New Style used as Modify Style).
    pub(crate) fn set_title(&mut self, title: &str) {
        title.clone_into(&mut self.form.title);
    }

    /// Items of list or drop-down `id` (tests).
    #[cfg(test)]
    pub(crate) fn items_for_test(&self, id: &str) -> Vec<String> {
        match self.form.get(id).map(|c| &c.kind) {
            Some(Kind::List { items, .. } | Kind::Combo { items, .. }) => items.clone(),
            _ => Vec::new(),
        }
    }

    /// Change the text of label `id`.
    pub(crate) fn set_label(&mut self, id: &str, text: &str) {
        if let Some(Kind::Label(t)) = self.form.get_mut(id).map(|c| &mut c.kind) {
            text.clone_into(t);
        }
    }

    /// Text of label `id` (tests).
    #[cfg(test)]
    pub(crate) fn label_for_test(&self, id: &str) -> String {
        match self.form.get(id).map(|c| &c.kind) {
            Some(Kind::Label(t)) => t.clone(),
            _ => String::new(),
        }
    }

    /// Pick the item of drop-down `id` that reads `value` (an edit box takes the text).
    pub(crate) fn choose_item(&mut self, id: &str, value: &str) {
        let pos = match self.form.get(id).map(|c| &c.kind) {
            Some(Kind::Combo { items, .. }) => items.iter().position(|i| i == value),
            Some(Kind::Edit { .. }) => {
                self.form.set_text(id, value);
                return;
            }
            _ => None,
        };
        if pos.is_some() {
            self.form.set_sel(id, pos);
        }
    }

    /// Set check box `id`.
    pub(crate) fn set_on(&mut self, id: &str, on: bool) {
        self.form.set_on(id, on);
    }

    /// Text of control `id`.
    pub(crate) fn text(&self, id: &str) -> String {
        self.form.text(id)
    }

    pub(crate) fn sel(&self, id: &str) -> Option<usize> {
        self.form.sel(id)
    }

    pub(crate) fn on(&self, id: &str) -> bool {
        self.form.on(id)
    }

    pub(crate) fn set_text(&mut self, id: &str, v: &str) {
        self.form.set_text(id, v);
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn set_sel(&mut self, id: &str, i: Option<usize>) {
        self.form.set_sel(id, i);
    }

    /// Show tab page `i`.
    pub(crate) fn set_tab(&mut self, i: usize) {
        self.form.set_tab(i);
    }

    /// Choose radio button `id` (tests and scripted captures).
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn choose(&mut self, id: &str) {
        self.set_radio(id);
    }

    pub(crate) fn layout(&mut self, m: &dyn TextMeasure, size: (f32, f32), scale: f32) {
        self.form.layout(m, size, scale);
        let mut strings: Vec<String> = self.radios.iter().map(|r| r.label.clone()).collect();
        strings.extend(self.preview.iter().cloned());
        strings.extend(self.words.iter().map(|w| w.1.clone()));
        self.form.measure(m, &strings);
        // Entry text before the page number, for the preview's leaders (Index and Tables
        // previews use the body font, round 14).
        let mut font = self.form.font();
        if matches!(
            self.kind,
            DocFormKind::Toc | DocFormKind::Index | DocFormKind::Tof
        ) {
            font.family = "Calibri";
        }
        self.label_w = self
            .preview
            .iter()
            .map(|l| m.measure_font(l.split('\t').next().unwrap_or(""), &font).0)
            .collect();
    }

    /// Window point (device pixels) of the centre of control `id` (tests).
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn point_of(&self, id: &str) -> Option<(f32, f32)> {
        let c = self.form.get(id)?;
        let d = self.form.dev(c.rect);
        Some((d[0] + d[2] / 2.0, d[1] + d[3] / 2.0))
    }

    /// Handle an input; `None` lets it through (never for a modal dialog).
    pub(crate) fn input(&mut self, input: &Input) -> FormOut {
        let ev = match input {
            Input::Text(s) => self.form.type_text(s),
            Input::Key(Key::Escape, _) => Event::Cancel,
            Input::Key(k, m) if self.r17_key(*k, *m) => return FormOut::Redraw,
            Input::Key(k, m) => self.form.key(*k, *m),
            Input::Move { x, y } => {
                self.pointer = (*x, *y);
                return if self.form.hover(*x, *y) {
                    FormOut::Redraw
                } else {
                    FormOut::None
                };
            }
            Input::Press { x, y, count, .. } => {
                self.pointer = (*x, *y);
                let ev = self.form.press(*x, *y, *count);
                // Colors › Custom: a press on the field or the luminance bar keeps picking
                // while the pointer drags (round 17).
                self.drag = match &ev {
                    Event::Canvas(id @ ("spectrum" | "lum"), ..) => Some(id),
                    _ => None,
                };
                ev
            }
            Input::Drag { x, y } => {
                let Some(id) = self.drag else {
                    return FormOut::None;
                };
                let (lx, ly) = self.form.to_local(*x, *y);
                let Some(r) = self.form.get(id).map(|c| c.rect) else {
                    return FormOut::None;
                };
                let (cx, cy) = ((lx - r[0]).clamp(0.0, r[2]), (ly - r[1]).clamp(0.0, r[3]));
                return if self.r15_canvas(id, cx, cy) {
                    FormOut::Redraw
                } else {
                    FormOut::None
                };
            }
            Input::Release { .. } => {
                self.drag = None;
                return FormOut::None;
            }
            Input::Wheel(d) if self.r17_wheel(*d) => return FormOut::Redraw,
            Input::Wheel(d) => {
                let (x, y) = self.pointer;
                return if self.form.wheel(x, y, *d) {
                    FormOut::Redraw
                } else {
                    FormOut::None
                };
            }
            Input::Blink => return FormOut::Redraw,
            _ => return FormOut::None,
        };
        // Index and Tables: each tab is its own dialog of the same size (round 14).
        if let Some(out) = self.r14_tab_switch() {
            return out;
        }
        self.event(&ev)
    }

    fn event(&mut self, ev: &Event) -> FormOut {
        match ev {
            Event::None => FormOut::Redraw,
            Event::Cancel => FormOut::Close,
            Event::Canvas(id, x, y) if self.r18_canvas(id, *x, *y) => FormOut::Redraw,
            Event::Canvas(id, x, y) if self.r22_canvas(id, *x, *y) => FormOut::Redraw,
            Event::Canvas(id, x, y) if self.r17_canvas(id, *x, *y) => FormOut::Redraw,
            Event::Canvas(id, x, y) if self.r15_canvas(id, *x, *y) => FormOut::Redraw,
            Event::Canvas(id, x, y) if self.r13_canvas(id, *x, *y) => FormOut::Redraw,
            Event::Canvas(id, _, y) if self.r11_canvas(id, *y) => FormOut::Redraw,
            Event::Canvas(id, x, y) if self.r12_canvas(id, *x, *y) => FormOut::Redraw,
            Event::Canvas(id, ..) if self.kind == DocFormKind::TableProps && self.tp_canvas(id) => {
                FormOut::Redraw
            }
            Event::Canvas(id, ..) => {
                if self.radios.iter().any(|r| r.id == *id && r.enabled) {
                    self.set_radio(id);
                    self.changed(id);
                }
                FormOut::Redraw
            }
            Event::Changed(id) => {
                self.changed(id);
                FormOut::Redraw
            }
            Event::Clicked(id) => self.clicked(id),
            Event::Ok => self.ok(),
        }
    }

    /// Dependent controls follow a change.
    fn changed(&mut self, id: &str) {
        match (self.kind, id) {
            (DocFormKind::Notes, "foot" | "end") => {
                let foot = self.chosen("where") == Some("foot");
                self.form.enable("footPos", foot);
                self.form.enable("endPos", !foot);
            }
            (DocFormKind::CrossRef, "type") => {}
            (DocFormKind::Caption, "label" | "exclude") => {
                let label = self.text("label");
                let text = if self.on("exclude") {
                    "1".to_owned()
                } else {
                    format!("{label} 1")
                };
                self.set_text("caption", &text);
            }
            (DocFormKind::MarkEntry, "xref" | "page" | "range") => {
                let range = self.chosen("opt") == Some("range");
                self.form.enable("bookmark", range);
            }
            (DocFormKind::Protect, _) => {}
            (k, _) if r23::handles(k) => self.r23_changed(id),
            (k, _) if r18::handles(k) => self.r18_changed(id),
            (k, _) if r15::handles(k) => self.r15_changed(id),
            (DocFormKind::InsertObject, "asIcon" | "asIcon1") => self.r22_icon_toggled(id),
            (k, _) if r17::handles(k) => self.r17_changed(id),
            (k, _) if r16::handles(k) => self.r16_changed(id),
            (k, _) if r14::handles(k) => self.r14_changed(id),
            (k, _) if r13::handles(k) => {
                self.r11_changed(id);
                self.r13_changed(id);
            }
            (DocFormKind::NewStyle, _) => self.style_describe(),
            (k, _) if findfmt11::handles(k) => self.r11_changed(id),
            (DocFormKind::TableProps, _) => self.tp_changed(id),
            (k, _) if r12::handles(k) => self.r12_changed(id),
            _ => self.r5_changed(id),
        }
    }

    /// Button `id` pressed (tests).
    #[cfg(test)]
    pub(crate) fn click_for_test(&mut self, id: &str) -> FormOut {
        self.clicked(id)
    }

    fn clicked(&mut self, id: &str) -> FormOut {
        if let Some(out) = self.r23_clicked(id) {
            return out;
        }
        if let Some(out) = self.r22_clicked(id) {
            return out;
        }
        if let Some(out) = self.r18_clicked(id) {
            return out;
        }
        if let Some(out) = self.r17_clicked(id) {
            return out;
        }
        if let Some(out) = self.r16_clicked(id) {
            return out;
        }
        if let Some(out) = self.r15_clicked(id) {
            return out;
        }
        if let Some(out) = self.r14_clicked(id) {
            return out;
        }
        match (self.kind, id) {
            (DocFormKind::Sources, "new") => FormOut::Open(DocFormKind::NewSource),
            (DocFormKind::Sources, "delete") => match self.sel("current") {
                Some(i) => {
                    let tag = self.ids.get(i).cloned().unwrap_or_default();
                    FormOut::Run(vec![json!({ "cmd": "citeDeleteSource", "tag": tag })], true)
                }
                None => FormOut::Redraw,
            },
            (DocFormKind::Index, "markEntry") => FormOut::Open(DocFormKind::MarkEntry),
            (DocFormKind::Toc, "options") => FormOut::Stash(DocFormKind::TocOptions),
            (DocFormKind::Caption, "newLabel") => FormOut::Redraw,
            (DocFormKind::Compare, "pickOriginal") => FormOut::PickFile(true),
            (DocFormKind::Compare, "pickRevised") => FormOut::PickFile(false),
            (DocFormKind::Compare, "swap") => {
                self.files.swap(0, 1);
                FormOut::Redraw
            }
            (DocFormKind::MarkEntry, "markAll") => self.mark_entry(true),
            (DocFormKind::Notes, "apply") => FormOut::Run(vec![self.note_options()], true),
            (DocFormKind::Envelopes, "addToDocument" | "print") => {
                FormOut::NewDocument(self.envelope())
            }
            (DocFormKind::Envelopes, "newDocument" | "printLabels") => {
                FormOut::NewDocument(self.labels())
            }
            (DocFormKind::CrossRef | DocFormKind::Language, "list") => self.ok(),
            (k, _) if r13::handles(k) => self.r13_clicked(id),
            (DocFormKind::NewStyle, "format") => self.r13_clicked(id),
            (DocFormKind::BordersShading, "bOptions") => self.r13_clicked(id),
            (k, _) if findfmt11::handles(k) => self.r11_clicked(id),
            (DocFormKind::TableProps, _) => self.tp_clicked(id),
            (k, _) if r5::handles(k) => self.r5_clicked(id),
            (k, _) if r7::handles(k) => self.r7_clicked(id),
            (DocFormKind::DateTime, "setDefault") => FormOut::DateDefault(
                self.sel("formats")
                    .unwrap_or(0)
                    .min(DATE_PICTURES.len() - 1),
            ),
            _ => FormOut::Redraw,
        }
    }

    fn mark_entry(&self, all: bool) -> FormOut {
        let opt = self.chosen("opt").unwrap_or("page");
        let mark = json!({
            "main": self.text("main"),
            "sub": self.text("sub"),
            "crossRef": if opt == "xref" { self.text("xrefText") } else { String::new() },
            "bookmark": if opt == "range" { self.text("bookmark") } else { String::new() },
            "bold": self.on("bold"),
            "italic": self.on("italic"),
        });
        if self.text("main").trim().is_empty() {
            return FormOut::Redraw;
        }
        FormOut::Run(
            vec![json!({ "cmd": "markIndexEntry", "mark": mark, "all": all })],
            false,
        )
    }

    fn note_options(&self) -> Value {
        let fmt = NOTE_FORMATS
            .get(self.sel("format").unwrap_or(0))
            .map_or("decimal", |f| f.0);
        let restart = ["continuous", "eachSect", "eachPage"]
            .get(self.sel("numbering").unwrap_or(0))
            .copied()
            .unwrap_or("continuous");
        let endnote = self.chosen("where") == Some("end");
        json!({
            "cmd": "setNoteOptions",
            "endnote": endnote,
            "options": {
                "fmt": fmt,
                "start": self.form.num("start").max(1),
                "restart": restart,
                "pos": if endnote {
                    if self.sel("endPos") == Some(1) { "sectEnd" } else { "docEnd" }
                } else if self.sel("footPos") == Some(1) {
                    "beneathText"
                } else {
                    "pageBottom"
                },
            },
        })
    }

    fn envelope(&self) -> Value {
        let delivery: Vec<String> = ["d1", "d2", "d3", "d4"]
            .iter()
            .map(|id| self.text(id))
            .filter(|s| !s.trim().is_empty())
            .collect();
        let ret: Vec<String> = if self.on("omit") {
            Vec::new()
        } else {
            ["r1", "r2", "r3"]
                .iter()
                .map(|id| self.text(id))
                .filter(|s| !s.trim().is_empty())
                .collect()
        };
        json!({
            "cmd": "mergeEnvelope",
            "size": { "id": "dl", "width": 220.0, "height": 110.0 },
            "delivery": delivery.join("\n"),
            "returnAddress": ret.join("\n"),
        })
    }

    fn labels(&self) -> Value {
        let text: Vec<String> = ["l1", "l2", "l3", "l4"]
            .iter()
            .map(|id| self.text(id))
            .filter(|s| !s.trim().is_empty())
            .collect();
        json!({
            "cmd": "mergeLabels",
            "sheet": {
                "id": "a4-3x8", "pageWidth": 210.0, "pageHeight": 297.0, "rows": 8, "cols": 3,
                "top": 12.9, "left": 7.25, "labelWidth": 63.5, "labelHeight": 33.9,
                "hPitch": 66.0, "vPitch": 33.9,
            },
            "text": text.join("\n"),
            "mode": if self.chosen("labelPrint") == Some("single") { "perRecord" } else { "same" },
        })
    }

    fn ok(&mut self) -> FormOut {
        if let Some(out) = self.r23_ok() {
            return out;
        }
        if let Some(out) = self.r22_ok() {
            return out;
        }
        if let Some(out) = self.r18_ok() {
            return out;
        }
        if r17::handles(self.kind) {
            return self.r17_ok();
        }
        if let Some(out) = self.r16_ok() {
            return out;
        }
        if let Some(out) = self.r15_ok() {
            return out;
        }
        if let Some(out) = self.r13_ok() {
            return out;
        }
        if let Some(out) = self.r14_ok() {
            return out;
        }
        match self.kind {
            DocFormKind::PasteSpecial => {
                let mode = match self.sel("as").unwrap_or(0) {
                    0 => "keep",
                    1 => "text",
                    _ => "merge",
                };
                FormOut::Paste(mode)
            }
            DocFormKind::PasteDefaults => FormOut::Defaults([
                self.sel("same").unwrap_or(0),
                self.sel("between").unwrap_or(0),
                self.sel("other").unwrap_or(0),
            ]),
            // Round 13 (`r13_ok`).
            DocFormKind::NewStyle
            | DocFormKind::BorderOptions
            | DocFormKind::PaneOptions
            | DocFormKind::LineNumbers
            | DocFormKind::Tof
            | DocFormKind::Toa
            | DocFormKind::MarkCitation
            | DocFormKind::Watermark
            | DocFormKind::TofOptions
            | DocFormKind::Categories
            | DocFormKind::Colors
            | DocFormKind::ThemeColors
            | DocFormKind::ThemeFonts
            | DocFormKind::FillEffects
            | DocFormKind::TrackOptions
            | DocFormKind::AdvTrackOptions
            | DocFormKind::MergeNew
            | DocFormKind::MergePrint
            | DocFormKind::CheckErrors
            | DocFormKind::MergeReport
            | DocFormKind::NewList
            | DocFormKind::MergeEmail
            | DocFormKind::NlFind
            | DocFormKind::NlColumns
            | DocFormKind::Bookmark
            | DocFormKind::InsertObject
            | DocFormKind::Macros
            | DocFormKind::OnlineVideo
            | DocFormKind::MacroSource
            | DocFormKind::Icons
            | DocFormKind::StyleNumbering
            | DocFormKind::CustomizeKeyboard
            | DocFormKind::StyleEffects
            | DocFormKind::StyleFrame
            | DocFormKind::OnlinePicture
            | DocFormKind::ChangeIcon
            | DocFormKind::AddinList => FormOut::Close,
            DocFormKind::Toc => {
                let levels = self.form.num("levels").clamp(1, 9);
                let mut instr = match &self.toc_map {
                    // §17.16.5.68 TOC \o for the heading levels TOC Options keeps, plus \t
                    // with its style → level pairs when they are not Heading n → n.
                    Some(map) => {
                        let pairs: Vec<(u8, u8)> = map
                            .split(',')
                            .collect::<Vec<_>>()
                            .chunks(2)
                            .filter_map(|c| {
                                let n = c.first()?.trim().strip_prefix("Heading ")?.parse().ok()?;
                                Some((n, c.get(1)?.trim().parse().ok()?))
                            })
                            .collect();
                        let lo = pairs.iter().map(|p| p.0).min().unwrap_or(1);
                        let hi = pairs.iter().map(|p| p.0).max().unwrap_or(levels as u8);
                        if pairs.iter().all(|(n, l)| n == l) {
                            format!("TOC \\o \"{lo}-{hi}\" \\h \\z \\u")
                        } else {
                            format!("TOC \\o \"{lo}-{hi}\" \\t \"{map}\" \\h \\z \\u")
                        }
                    }
                    None => format!("TOC \\o \"1-{levels}\" \\h \\z \\u"),
                };
                if !self.on("pageNumbers") {
                    instr.push_str(" \\n");
                }
                let leader = LEADERS
                    .get(self.sel("leader").unwrap_or(0))
                    .map_or("dot", |l| l.0);
                FormOut::Run(
                    vec![
                        json!({ "cmd": "insertReferenceTable", "instr": instr, "leader": leader }),
                    ],
                    true,
                )
            }
            DocFormKind::Notes => {
                let endnote = self.chosen("where") == Some("end");
                FormOut::Run(
                    vec![
                        self.note_options(),
                        json!({ "cmd": "insertNote", "endnote": endnote }),
                    ],
                    true,
                )
            }
            DocFormKind::NewSource => {
                let author = self.text("author");
                let title = self.text("title");
                if author.trim().is_empty() && title.trim().is_empty() {
                    return FormOut::Redraw;
                }
                let mut tag = self.text("tag");
                if tag.trim().is_empty() {
                    let last: String = author
                        .split_whitespace()
                        .last()
                        .unwrap_or("Src")
                        .chars()
                        .filter(|c| c.is_alphanumeric())
                        .take(3)
                        .collect();
                    tag = format!("{last}{}", self.text("year").trim());
                }
                let people: Vec<Value> = author
                    .split(';')
                    .filter(|a| !a.trim().is_empty())
                    .map(|a| {
                        let words: Vec<&str> = a.split_whitespace().collect();
                        let (first, last) = match words.split_last() {
                            Some((l, f)) => (f.join(" "), (*l).to_owned()),
                            None => (String::new(), String::new()),
                        };
                        json!({ "last": last, "first": first })
                    })
                    .collect();
                let kind = SOURCE_TYPES[self.sel("type").unwrap_or(0).min(SOURCE_TYPES.len() - 1)];
                let source = json!({
                    "tag": tag.trim(),
                    "kind": kind,
                    "contributors": [{ "role": "Author", "people": people }],
                    "fields": {
                        "Title": title.trim(),
                        "Year": self.text("year").trim(),
                        "City": self.text("city").trim(),
                        "Publisher": self.text("publisher").trim(),
                    },
                });
                let cite = json!({ "items": [{ "tag": tag.trim() }] });
                FormOut::Run(
                    vec![json!({ "cmd": "citeInsert", "citation": cite, "sources": [source] })],
                    true,
                )
            }
            DocFormKind::Sources => FormOut::Close,
            DocFormKind::MarkEntry => self.mark_entry(false),
            DocFormKind::Index => {
                let cols = self.form.num("columns").clamp(1, 4);
                let mut instr = format!("INDEX \\c \"{cols}\"");
                if self.chosen("type") == Some("runIn") {
                    instr.push_str(" \\r");
                }
                let leader = if self.on("rightAlign") {
                    LEADERS
                        .get(self.sel("leader").unwrap_or(0))
                        .map_or("dot", |l| l.0)
                } else {
                    "none"
                };
                FormOut::Run(
                    vec![json!({
                        "cmd": "insertReferenceTable", "instr": instr, "leader": leader,
                        "columns": cols,
                    })],
                    true,
                )
            }
            DocFormKind::CrossRef => match self.sel("list").and_then(|i| self.ids.get(i)) {
                Some(target) => {
                    let page = self.sel("to") == Some(1);
                    FormOut::Run(
                        vec![
                            json!({ "cmd": "insertCrossReference", "target": target, "page": page }),
                        ],
                        false,
                    )
                }
                None => FormOut::Redraw,
            },
            DocFormKind::Caption => {
                let label = self.text("label");
                let caption = self.text("caption");
                // The typed text after "Label n" is the caption's own text.
                let text = caption
                    .split_once(char::is_numeric)
                    .map_or("", |(_, rest)| {
                        rest.trim_start_matches(|c: char| c.is_numeric())
                    })
                    .trim()
                    .to_owned();
                FormOut::Run(
                    vec![json!({ "cmd": "insertCaption", "label": label, "text": text })],
                    true,
                )
            }
            DocFormKind::Compare => match &self.files {
                // Revised = this document: its differences from the original.
                [Some((name, b64)), None] => FormOut::Run(
                    vec![json!({ "cmd": "compareWith", "name": name, "base64": b64 })],
                    true,
                ),
                [None, None] => FormOut::PickFile(true),
                // Another revised file: the result opens in a new window.
                [original, Some(revised)] => FormOut::Compare(original.clone(), revised.clone()),
            },
            DocFormKind::Language => {
                let tag = self.sel("list").and_then(|i| LANGUAGES.get(i)).map(|l| l.0);
                let mut cmds = Vec::new();
                if self.chosen("scope") == Some("document") {
                    cmds.push(json!({ "cmd": "selectAll" }));
                }
                cmds.push(
                    json!({ "cmd": "formatText", "format": { "kind": "language", "lang": tag } }),
                );
                cmds.push(json!({ "cmd": "formatText", "format": { "kind": "noProof", "on": self.on("noProof") } }));
                FormOut::Run(cmds, true)
            }
            DocFormKind::Envelopes => FormOut::Close,
            DocFormKind::Protect => {
                let (a, b) = (self.text("pw"), self.text("pw2"));
                if a != b {
                    return FormOut::Redraw;
                }
                let edit = self
                    .ids
                    .first()
                    .cloned()
                    .unwrap_or_else(|| "readOnly".into());
                FormOut::Run(
                    vec![json!({
                        "cmd": "protectDocument", "edit": edit,
                        "password": (!a.is_empty()).then_some(a),
                    })],
                    true,
                )
            }
            DocFormKind::Recipients => FormOut::Close,
            DocFormKind::TocOptions => {
                let mut pairs = Vec::new();
                for (i, id) in TOC_LEVEL_IDS.iter().enumerate() {
                    let v = self.text(id);
                    if let Ok(l) = v.trim().parse::<u8>() {
                        if (1..=9).contains(&l) {
                            pairs.push(format!("Heading {},{l}", i + 1));
                        }
                    }
                }
                FormOut::TocMap(pairs.join(","))
            }
            DocFormKind::Unprotect => FormOut::Run(
                vec![json!({ "cmd": "unprotectDocument", "password": self.text("pw") })],
                true,
            ),
            DocFormKind::DateTime => {
                let i = self
                    .sel("formats")
                    .unwrap_or(0)
                    .min(DATE_PICTURES.len() - 1);
                let cmd = if self.on("update") {
                    json!({ "cmd": "insertField", "instr": format!("DATE \\@ \"{}\"", DATE_PICTURES[i]) })
                } else {
                    json!({ "cmd": "insertText", "text": self.text("formats") })
                };
                FormOut::Run(vec![cmd], true)
            }
            DocFormKind::AskRule
            | DocFormKind::FillInRule
            | DocFormKind::IfRule
            | DocFormKind::NextIfRule
            | DocFormKind::SetRule
            | DocFormKind::SkipIfRule
            | DocFormKind::MatchFields
            | DocFormKind::FindRecipient
            | DocFormKind::ManageStyles => self.r5_ok(),
            DocFormKind::AltText
            | DocFormKind::Encrypt
            | DocFormKind::Inspect
            | DocFormKind::Compatibility => self.r7_ok(),
            DocFormKind::FindFont
            | DocFormKind::FindParagraph
            | DocFormKind::FindTabs
            | DocFormKind::FindLanguage
            | DocFormKind::FindFrame
            | DocFormKind::FindStyle => self.r11_ok(),
            DocFormKind::TableProps => self.tp_ok(),
            DocFormKind::TableOptions => self.options_ok(),
            DocFormKind::InsertTable => self.insert_table_ok(),
            DocFormKind::TablePosition | DocFormKind::BordersShading | DocFormKind::NewBlock => {
                self.r12_ok()
            }
            DocFormKind::Paragraph | DocFormKind::Tabs | DocFormKind::PageSetup => FormOut::Close,
            DocFormKind::Zoom => {
                let pct = match self.chosen("zoom") {
                    Some("z200") => "200".to_owned(),
                    Some("z75") => "75".to_owned(),
                    Some("pageWidth") => "pageWidth".to_owned(),
                    Some("textWidth") => "textWidth".to_owned(),
                    Some("wholePage") => "wholePage".to_owned(),
                    Some("z100") => "100".to_owned(),
                    _ => self.text("pct"),
                };
                FormOut::Run(
                    vec![json!({ "cmd": "ui.action", "id": format!("view.zoomTo.{pct}") })],
                    true,
                )
            }
            // Round 23: answered by `r23_ok` above.
            DocFormKind::Font23
            | DocFormKind::SetNumbering
            | DocFormKind::DefineBullet
            | DocFormKind::DefineNumber
            | DocFormKind::DefineMultilevel
            | DocFormKind::SortText => FormOut::Redraw,
        }
    }

    pub(crate) fn paint(&self, p: &mut dyn Painter, window: (f32, f32), caret_on: bool) {
        let font = self.form.font();
        let k = self.form.font().size / 13.5;
        let radios = &self.radios;
        let preview = &self.preview;
        let label_w = &self.label_w;
        let kind = self.kind;
        let files = &self.files;
        let this = self;
        let mut canvas = |id: &str, r: [f32; 4], p: &mut dyn Painter| {
            if r18::paint_canvas(this, id, r, p, &font)
                || r23::paint_canvas(this, id, r, p, &font)
                || r22::paint_canvas(this, id, r, p, &font)
                || r17::paint_canvas(this, id, r, p, &font)
                || r15::paint_canvas(this, id, r, p, &font)
                || r13::paint_canvas(this, id, r, p, &font)
                || r14::paint_canvas(this, id, r, p, &font)
                || findfmt11::paint_canvas(this, id, r, p, &font, k)
                || tprops11::paint_canvas(this, id, r, p, k)
                || r12::paint_canvas(this, id, r, p, &font)
            {
                return;
            }
            if let Some(rb) = radios.iter().find(|rb| rb.id == id) {
                paint_radio(p, r, rb, &font, k);
                return;
            }
            match id {
                "preview" | "webPreview" => {
                    paint_preview(label_w, p, r, preview, kind, id == "webPreview", &font);
                }
                "clipIcon" => crate::doc_r3_ui::paste_icon(p, "text", r[0], r[1], r[3]),
                "envPreview" => paint_envelope(p, r),
                "feed" => {
                    p.fill_rect(r[0], r[1], r[2], r[3], Rgba(0xdc, 0xdc, 0xdc, 255));
                    p.fill_rect(
                        r[0] + 1.0,
                        r[1] + 1.0,
                        r[2] - 2.0,
                        r[3] - 2.0,
                        Rgba(0xf9, 0xf9, 0xf9, 255),
                    );
                }
                "origName" | "revName" => {
                    let i = usize::from(id == "revName");
                    paint_combo_box(p, r, &font, k);
                    let name = match &files[i] {
                        Some((name, _)) => name.as_str(),
                        None if i == 1 => preview.first().map_or("", String::as_str),
                        None => "",
                    };
                    p.push_clip(r[0], r[1], r[2] - 20.0 * k, r[3]);
                    p.text_font(
                        name,
                        r[0] + 4.0 * k,
                        (r[1] + (r[3] - font.size * 1.33) / 2.0).round(),
                        &font,
                        Rgba(0, 0, 0, 255),
                    );
                    p.pop_clip();
                }
                "pickOriginal" | "pickRevised" => paint_folder(p, r, k),
                "swap" => paint_swap(p, r, k),
                _ => {}
            }
        };
        self.form.paint(p, window, caret_on, &mut canvas);
    }
}

fn t(s: &Strings, key: &str, fallback: &str) -> String {
    let v = s.get(key);
    if v == key {
        fallback.to_owned()
    } else {
        v.to_owned()
    }
}

fn paint_radio(p: &mut dyn Painter, r: [f32; 4], rb: &Radio, font: &Font<'_>, k: f32) {
    let d = (13.0 * k).round();
    let (cx, cy) = (r[0] + d / 2.0, r[1] + r[3] / 2.0);
    let ring = |rad: f32| -> Vec<PathSeg> {
        let n = 24;
        let mut v = Vec::with_capacity(n + 1);
        for i in 0..=n {
            let a = i as f32 / n as f32 * std::f32::consts::TAU;
            let (x, y) = (cx + rad * a.cos(), cy + rad * a.sin());
            v.push(if i == 0 {
                PathSeg::M(x, y)
            } else {
                PathSeg::L(x, y)
            });
        }
        v.push(PathSeg::Z);
        v
    };
    let edge = if rb.enabled {
        if rb.on {
            Rgba(0x00, 0x5f, 0xb8, 255)
        } else {
            Rgba(0x33, 0x33, 0x33, 255)
        }
    } else {
        Rgba(0xbc, 0xbc, 0xbc, 255)
    };
    p.path(
        &ring(d / 2.0),
        Some(Rgba(255, 255, 255, 255)),
        Some((edge, 1.0)),
    );
    if rb.on {
        p.path(&ring(d / 2.0 - 3.0 * k), Some(edge), None);
    }
    let line = font.size * 1.33;
    p.text_font(
        &rb.label,
        r[0] + d + 6.0 * k,
        (cy - line / 2.0).round(),
        font,
        if rb.enabled {
            Rgba(0, 0, 0, 255)
        } else {
            Rgba(0x83, 0x83, 0x83, 255)
        },
    );
}

/// Print Preview of the TOC / Index dialogs and the source preview: white box with sample
/// lines (headings indented by level, dotted leaders, page numbers right).
fn paint_preview(
    label_w: &[f32],
    p: &mut dyn Painter,
    r: [f32; 4],
    lines: &[String],
    kind: DocFormKind,
    web: bool,
    font: &Font<'_>,
) {
    p.fill_rect(r[0], r[1], r[2], r[3], Rgba(0x82, 0x87, 0x90, 255));
    p.fill_rect(
        r[0] + 1.0,
        r[1] + 1.0,
        r[2] - 2.0,
        r[3] - 2.0,
        Rgba(255, 255, 255, 255),
    );
    if kind == DocFormKind::ManageStyles {
        // Manage Styles: the font's name in that font, the size on a grey tab at the right.
        let (Some(family), Some(size)) = (lines.first(), lines.get(1)) else {
            return;
        };
        let face = Font {
            family: family.as_str(),
            size: (font.size * 1.6).round(),
            weight: 400,
            italic: false,
            underline: false,
        };
        let tab = (font.size * 2.8).round();
        p.push_clip(r[0] + 1.0, r[1] + 1.0, r[2] - 2.0, r[3] - 2.0);
        p.text_font(
            family,
            r[0] + 8.0,
            (r[1] + (r[3] - face.size * 1.3) / 2.0).round(),
            &face,
            Rgba(0, 0, 0, 255),
        );
        p.fill_rect(
            r[0] + r[2] - tab,
            r[1],
            tab,
            r[3],
            Rgba(0xa6, 0xa6, 0xa6, 255),
        );
        p.text_font(
            &format!("{size} pt"),
            r[0] + r[2] - tab + 4.0,
            r[1] + r[3] - font.size * 1.5,
            font,
            Rgba(0x26, 0x26, 0x26, 255),
        );
        p.pop_clip();
        return;
    }
    p.push_clip(r[0] + 1.0, r[1] + 1.0, r[2] - 2.0, r[3] - 2.0);
    // Index and Tables previews: the body font, one entry every 2.2 lines (round 14, as
    // the reference draws its samples).
    let tables = matches!(
        kind,
        DocFormKind::Toc | DocFormKind::Index | DocFormKind::Tof
    );
    let big = Font {
        family: if tables { "Calibri" } else { "Times New Roman" },
        size: (font.size * 1.75).round(),
        weight: 400,
        italic: false,
        underline: web,
    };
    let line = (big.size * if tables { 2.2 } else { 1.3 }).round();
    let blue = Rgba(0x05, 0x63, 0xc1, 255);
    for (i, text) in lines.iter().enumerate() {
        let y = r[1] + 4.0 + i as f32 * line;
        let indent = if kind == DocFormKind::Toc {
            i as f32 * big.size * if tables { 1.05 } else { 1.2 }
        } else {
            0.0
        };
        let (label, page) = text.split_once('\t').unwrap_or((text.as_str(), ""));
        p.text_font(
            label,
            r[0] + 4.0 + indent,
            y,
            &big,
            if web { blue } else { Rgba(0, 0, 0, 255) },
        );
        if !web && !page.is_empty() {
            p.text_font(
                page,
                r[0] + r[2] - big.size * 1.1,
                y,
                &big,
                Rgba(0, 0, 0, 255),
            );
            // Dotted leader.
            // From the end of the entry text (measured in the dialog font, scaled to the
            // preview's) to the page number.
            let text_w = label_w
                .get(i)
                .map_or(big.size * 0.5 * label.chars().count() as f32, |w| {
                    w * big.size / font.size
                });
            // Index and Tables: period-sized dots on the baseline, from the text right up
            // to the number (round 14, as the reference's "Text.......1").
            let (start, dy, step, stop) = if tables {
                (text_w * 0.96, 0.93, 0.255, 1.18)
            } else {
                (text_w + 2.0, 1.05, 0.3, 1.4)
            };
            let dot = if tables { big.size * 0.09 } else { 1.5 };
            let mut x = r[0] + 4.0 + indent + start;
            while x < r[0] + r[2] - big.size * stop {
                p.fill_rect(x.round(), y + big.size * dy, dot, dot, Rgba(0, 0, 0, 255));
                x += big.size * step;
            }
        }
    }
    p.pop_clip();
    if tables {
        // The reference's preview scrolls: a bar beside the box, its thumb at the top.
        let k = font.size / 13.5;
        let bx = r[0] + r[2];
        p.fill_rect(
            bx,
            r[1],
            (20.0 * k).round(),
            r[3],
            Rgba(0xf0, 0xf0, 0xf0, 255),
        );
        p.fill_rect(
            bx + (8.0 * k).round(),
            r[1] + (20.0 * k).round(),
            (3.0 * k).max(2.0).round(),
            (20.0 * k).round(),
            Rgba(0x60, 0x60, 0x60, 255),
        );
    }
}

/// A drop-down box drawn on a canvas (white, grey frame, ▾ column).
fn paint_combo_box(p: &mut dyn Painter, r: [f32; 4], _font: &Font<'_>, k: f32) {
    p.fill_rect(r[0], r[1], r[2], r[3], Rgba(0x7a, 0x7a, 0x7a, 255));
    p.fill_rect(
        r[0] + 1.0,
        r[1] + 1.0,
        r[2] - 2.0,
        r[3] - 2.0,
        Rgba(255, 255, 255, 255),
    );
    let (cx, cy, a) = (r[0] + r[2] - 10.0 * k, r[1] + r[3] / 2.0, 4.0 * k);
    p.path(
        &[
            PathSeg::M(cx - a, cy - a / 2.0),
            PathSeg::L(cx, cy + a / 2.0),
            PathSeg::L(cx + a, cy - a / 2.0),
        ],
        None,
        Some((Rgba(0x40, 0x40, 0x40, 255), k.max(1.0))),
    );
}

/// The folder button of a file box (own drawing: a yellow folder).
fn paint_folder(p: &mut dyn Painter, r: [f32; 4], k: f32) {
    let (x, y) = (r[0] + 4.0 * k, r[1] + 6.0 * k);
    let gold = Rgba(0xd8, 0x9b, 0x2b, 255);
    p.fill_rect(x, y, 7.0 * k, 3.0 * k, gold);
    p.fill_rect(x, y + 2.0 * k, 17.0 * k, 12.0 * k, gold);
    p.fill_rect(
        x + 1.0 * k,
        y + 5.0 * k,
        15.0 * k,
        8.0 * k,
        Rgba(0xf3, 0xc6, 0x5a, 255),
    );
}

/// The ⇄ swap button between the two documents (own drawing).
fn paint_swap(p: &mut dyn Painter, r: [f32; 4], k: f32) {
    let ink = Rgba(0x70, 0x70, 0x70, 255);
    let (x0, x1, cy) = (r[0] + 4.0 * k, r[0] + r[2] - 4.0 * k, r[1] + r[3] / 2.0);
    let w = k.max(1.0) * 1.3;
    for (y, right) in [(cy - 4.0 * k, true), (cy + 4.0 * k, false)] {
        let (a, b) = if right { (x0, x1) } else { (x1, x0) };
        let d = if right { -4.0 * k } else { 4.0 * k };
        p.path(
            &[
                PathSeg::M(a, y),
                PathSeg::L(b, y),
                PathSeg::M(b + d, y - 3.0 * k),
                PathSeg::L(b, y),
                PathSeg::L(b + d, y + 3.0 * k),
            ],
            None,
            Some((ink, w)),
        );
    }
}

/// The Envelopes tab's preview: an envelope with return lines top-left, stamp top-right and
/// delivery lines in the middle (own drawing).
fn paint_envelope(p: &mut dyn Painter, r: [f32; 4]) {
    p.fill_rect(r[0], r[1], r[2], r[3], Rgba(0xdc, 0xdc, 0xdc, 255));
    p.fill_rect(
        r[0] + 1.0,
        r[1] + 1.0,
        r[2] - 2.0,
        r[3] - 2.0,
        Rgba(0xf9, 0xf9, 0xf9, 255),
    );
    let (w, h) = (r[2] * 0.7, r[3] * 0.55);
    let (x, y) = (
        r[0] + (r[2] - w) / 2.0,
        r[1] + (r[3] - h) / 2.0 + r[3] * 0.08,
    );
    let ink = Rgba(0, 0, 0, 255);
    p.fill_rect(x, y, w, h, ink);
    p.fill_rect(x + 1.0, y + 1.0, w - 2.0, h - 2.0, Rgba(255, 255, 255, 255));
    for i in 0..2 {
        p.fill_rect(x + 6.0, y + 4.0 + i as f32 * 3.0, w * 0.12, 1.0, ink);
    }
    p.fill_rect(x + w - 16.0, y + 4.0, 9.0, 9.0, ink);
    for i in 0..3 {
        p.fill_rect(
            x + w * 0.45,
            y + h * 0.55 + i as f32 * 5.0,
            w * 0.35,
            1.0,
            ink,
        );
    }
}

// ---------------------------------------------------------------------------------------
// Builders.

/// Build dialog `kind` from `c` with the interface strings `s`.
pub(crate) fn open(kind: DocFormKind, s: &Strings, c: &FormContext) -> DocForm {
    let tr = |k: &str, f: &str| t(s, k, f);
    match kind {
        DocFormKind::PasteSpecial => {
            let mut f = DocForm::new(
                kind,
                &tr("docR3.pasteSpecial", "Paste Special"),
                (760.0, 462.0),
                vec![],
            );
            f.label(
                "src",
                [10.0, 46.0, 300.0, 18.0],
                &tr("docR3.source", "Source:   Clipboard"),
            );
            f.label(
                "asLabel",
                [107.0, 100.0, 60.0, 18.0],
                &tr("docR3.as", "As:"),
            );
            f.radio(
                "paste",
                "how",
                [10.0, 121.0, 90.0, 20.0],
                &tr("docR3.paste", "Paste:"),
                true,
            );
            f.radio(
                "link",
                "how",
                [10.0, 147.0, 90.0, 20.0],
                &tr("docR3.pasteLink", "Paste link:"),
                false,
            );
            f.disable("link");
            f.list(
                "as",
                [107.0, 118.0, 404.0, 158.0],
                vec![
                    tr("docR3.asHtml", "Formatted Text (HTML)"),
                    tr("docR3.asText", "Unformatted Text"),
                    tr("docR3.asMerge", "Text with Destination Formatting"),
                ],
                Some(0),
            );
            f.check(
                "icon",
                [526.0, 123.0, 200.0, 18.0],
                &tr("docR3.asIcon", "Display as icon"),
                false,
            );
            f.disable("icon");
            f.group(
                "result",
                [10.0, 302.0, 740.0, 1.0],
                &tr("docR3.result", "Result"),
            );
            f.canvas("clipIcon", [44.0, 330.0, 46.0, 46.0]);
            f.label(
                "resultText",
                [112.0, 330.0, 600.0, 18.0],
                &tr(
                    "docR3.resultText",
                    "Inserts the contents of the Clipboard as the format you choose.",
                ),
            );
            f.ok_cancel(s);
            f
        }
        DocFormKind::PasteDefaults => {
            let mut f = DocForm::new(
                kind,
                &tr("docR3.pasteDefaults", "Set Default Paste"),
                (560.0, 250.0),
                vec![],
            );
            let modes = vec![
                tr("docR3.keepSource", "Keep Source Formatting (Default)"),
                tr("docR3.mergeFormatting", "Merge Formatting"),
                tr("docR3.keepText", "Keep Text Only"),
            ];
            f.group(
                "cut",
                [10.0, 48.0, 540.0, 1.0],
                &tr("docR3.cutCopyPaste", "Cut, copy, and paste"),
            );
            for (i, (id, key, fb)) in [
                (
                    "same",
                    "docR3.pasteSame",
                    "Pasting within the same document:",
                ),
                (
                    "between",
                    "docR3.pasteBetween",
                    "Pasting between documents:",
                ),
                ("other", "docR3.pasteOther", "Pasting from other programs:"),
            ]
            .into_iter()
            .enumerate()
            {
                let y = 66.0 + i as f32 * 36.0;
                let lid = ["l0", "l1", "l2"][i];
                f.label(lid, [20.0, y + 3.0, 250.0, 18.0], &tr(key, fb));
                f.combo(
                    id,
                    [280.0, y, 260.0, 24.0],
                    modes.clone(),
                    c.paste_defaults[i].min(2),
                );
            }
            f.ok_cancel(s);
            f
        }
        // Round 13: measured on the reference (669 × 703 window).
        DocFormKind::NewStyle => r13::open_new_style(s, c),
        DocFormKind::Toc => {
            let tabs = vec![
                tr("docR3.tabIndex", "Index"),
                tr("docR3.tabToc", "Table of Contents"),
                tr("docR3.tabFigures", "Table of Figures"),
                tr("docR3.tabAuthorities", "Table of Authorities"),
            ];
            let mut f = DocForm::new(kind, &r14::index_and_tables_title(s), (766.0, 598.0), tabs);
            f.form.tab = 1;
            f.label_on(
                1,
                "pp",
                [24.0, 88.0, 200.0, 18.0],
                &tr("docR3.printPreview", "Print Preview"),
            );
            f.label_on(
                1,
                "wp",
                [404.0, 88.0, 200.0, 18.0],
                &tr("docR3.webPreview", "Web Preview"),
            );
            f.preview = (1..=5)
                .map(|i| format!("{} {i}\t{}", tr("docR3.heading", "Heading"), 2 * i - 1))
                .collect();
            f.form.add(
                "preview",
                Some(1),
                [24.0, 108.0, 330.0, 158.0],
                Kind::Canvas,
            );
            f.form.add(
                "webPreview",
                Some(1),
                [404.0, 108.0, 318.0, 158.0],
                Kind::Canvas,
            );
            f.check_on(
                1,
                "pageNumbers",
                [24.0, 272.0, 330.0, 18.0],
                &tr("docR3.showPageNumbers", "Show page numbers"),
                true,
            );
            f.check_on(
                1,
                "rightAlign",
                [24.0, 297.0, 330.0, 18.0],
                &tr("docR3.rightAlign", "Right align page numbers"),
                true,
            );
            f.label_on(
                1,
                "leaderL",
                [24.0, 325.0, 80.0, 18.0],
                &tr("docR3.tabLeader", "Tab leader:"),
            );
            f.form.add(
                "leader",
                Some(1),
                [104.0, 320.0, 270.0, 26.0],
                Kind::Combo {
                    items: LEADERS.iter().map(|l| l.1.to_owned()).collect(),
                    sel: 0,
                },
            );
            f.check_on(
                1,
                "hyperlinks",
                [404.0, 272.0, 330.0, 18.0],
                &tr(
                    "docR3.useHyperlinks",
                    "Use hyperlinks instead of page numbers",
                ),
                true,
            );
            f.form.add(
                "general",
                Some(1),
                [24.0, 384.0, 718.0, 1.0],
                Kind::Group(tr("docR3.general", "General")),
            );
            f.label_on(
                1,
                "formatsL",
                [38.0, 416.0, 90.0, 18.0],
                &tr("docR3.formats", "Formats:"),
            );
            f.form.add(
                "formats",
                Some(1),
                [132.0, 410.0, 178.0, 26.0],
                Kind::Combo {
                    items: vec![tr("docR3.fromTemplate", "From template")],
                    sel: 0,
                },
            );
            f.label_on(
                1,
                "levelsL",
                [38.0, 448.0, 90.0, 18.0],
                &tr("docR3.showLevels", "Show levels:"),
            );
            f.form.add(
                "levels",
                Some(1),
                [132.0, 442.0, 92.0, 24.0],
                Kind::Spin {
                    value: 3,
                    min: 1,
                    max: 9,
                },
            );
            f.button_on(
                1,
                "options",
                [502.0, 504.0, 112.0, 28.0],
                &tr("docR3.options", "Options..."),
            );
            f.button_on(
                1,
                "modify",
                [629.0, 504.0, 112.0, 28.0],
                &tr("docR3.modify", "Modify..."),
            );
            f.disable("modify");
            f.ok_cancel(s);
            f
        }
        DocFormKind::Index => {
            let tabs = vec![
                tr("docR3.tabIndex", "Index"),
                tr("docR3.tabToc", "Table of Contents"),
                tr("docR3.tabFigures", "Table of Figures"),
                tr("docR3.tabAuthorities", "Table of Authorities"),
            ];
            let mut f = DocForm::new(kind, &r14::index_and_tables_title(s), (766.0, 598.0), tabs);
            f.label_on(
                0,
                "pp",
                [24.0, 88.0, 200.0, 18.0],
                &tr("docR3.printPreview", "Print Preview"),
            );
            f.preview = vec![
                format!("{}\t2", tr("docR3.indexSample1", "Asteroid")),
                tr("docR3.indexSample2", "Atmosphere"),
                format!("   {}\t4", tr("docR3.indexSample3", "Earth")),
            ];
            f.form.add(
                "preview",
                Some(0),
                [24.0, 108.0, 330.0, 158.0],
                Kind::Canvas,
            );
            f.form.add(
                "typeL",
                Some(0),
                [396.0, 112.0, 60.0, 18.0],
                Kind::Label(tr("docR3.type", "Type:")),
            );
            f.radio_on(
                0,
                "indented",
                "type",
                [470.0, 109.0, 120.0, 20.0],
                &tr("docR3.indented", "Indented"),
                true,
            );
            f.radio_on(
                0,
                "runIn",
                "type",
                [608.0, 109.0, 120.0, 20.0],
                &tr("docR3.runIn", "Run-in"),
                false,
            );
            f.form.add(
                "colsL",
                Some(0),
                [396.0, 140.0, 70.0, 18.0],
                Kind::Label(tr("docR3.columns", "Columns:")),
            );
            f.form.add(
                "columns",
                Some(0),
                [470.0, 134.0, 92.0, 24.0],
                Kind::Spin {
                    value: 2,
                    min: 1,
                    max: 4,
                },
            );
            f.form.add(
                "langL",
                Some(0),
                [396.0, 170.0, 70.0, 18.0],
                Kind::Label(tr("docR3.languageL", "Language:")),
            );
            f.form.add(
                "language",
                Some(0),
                [470.0, 164.0, 262.0, 26.0],
                Kind::Combo {
                    items: LANGUAGES.iter().map(|l| l.1.to_owned()).collect(),
                    sel: 0,
                },
            );
            f.check_on(
                0,
                "rightAlign",
                [24.0, 296.0, 330.0, 18.0],
                &tr("docR3.rightAlign", "Right align page numbers"),
                true,
            );
            f.label_on(
                0,
                "leaderL",
                [24.0, 325.0, 80.0, 18.0],
                &tr("docR3.tabLeader", "Tab leader:"),
            );
            f.form.add(
                "leader",
                Some(0),
                [96.0, 320.0, 270.0, 26.0],
                Kind::Combo {
                    items: LEADERS.iter().map(|l| l.1.to_owned()).collect(),
                    sel: 0,
                },
            );
            f.label_on(
                0,
                "formatsL",
                [24.0, 357.0, 80.0, 18.0],
                &tr("docR3.formats", "Formats:"),
            );
            f.form.add(
                "formats",
                Some(0),
                [96.0, 352.0, 270.0, 26.0],
                Kind::Combo {
                    items: vec![tr("docR3.fromTemplate", "From template")],
                    sel: 0,
                },
            );
            f.button_on(
                0,
                "markEntry",
                [367.0, 504.0, 112.0, 28.0],
                &tr("docR3.markEntryB", "Mark Entry..."),
            );
            f.button_on(
                0,
                "autoMark",
                [494.0, 504.0, 112.0, 28.0],
                &tr("docR3.autoMark", "AutoMark..."),
            );
            f.button_on(
                0,
                "modify",
                [621.0, 504.0, 112.0, 28.0],
                &tr("docR3.modify", "Modify..."),
            );
            f.disable("autoMark");
            f.disable("modify");
            f.ok_cancel(s);
            f
        }
        DocFormKind::Notes => {
            let mut f = DocForm::new(
                kind,
                &tr("docR3.notesTitle", "Footnote and Endnote"),
                (387.0, 520.0),
                vec![],
            );
            f.group(
                "loc",
                [10.0, 48.0, 367.0, 1.0],
                &tr("docR3.location", "Location"),
            );
            f.radio(
                "foot",
                "where",
                [24.0, 77.0, 120.0, 20.0],
                &tr("docR3.footnotes", "Footnotes:"),
                true,
            );
            f.radio(
                "end",
                "where",
                [24.0, 109.0, 120.0, 20.0],
                &tr("docR3.endnotes", "Endnotes:"),
                false,
            );
            f.combo(
                "footPos",
                [142.0, 74.0, 235.0, 26.0],
                vec![
                    tr("docR3.bottomOfPage", "Bottom of page"),
                    tr("docR3.belowText", "Below text"),
                ],
                0,
            );
            f.combo(
                "endPos",
                [142.0, 106.0, 235.0, 26.0],
                vec![
                    tr("docR3.endOfDocument", "End of document"),
                    tr("docR3.endOfSection", "End of section"),
                ],
                0,
            );
            f.disable("endPos");
            f.button(
                "convert",
                [264.0, 138.0, 112.0, 28.0],
                &tr("docR3.convert", "Convert..."),
                false,
            );
            f.disable("convert");
            f.group(
                "layoutG",
                [10.0, 178.0, 367.0, 1.0],
                &tr("docR3.footnoteLayout", "Footnote layout"),
            );
            f.label(
                "colsL",
                [24.0, 209.0, 100.0, 18.0],
                &tr("docR3.columns", "Columns:"),
            );
            f.combo(
                "columns",
                [142.0, 204.0, 235.0, 26.0],
                vec![tr("docR3.matchSection", "Match section layout")],
                0,
            );
            f.group(
                "formatG",
                [10.0, 242.0, 367.0, 1.0],
                &tr("docR3.format", "Format"),
            );
            f.label(
                "fmtL",
                [24.0, 273.0, 110.0, 18.0],
                &tr("docR3.numberFormat", "Number format:"),
            );
            f.combo(
                "format",
                [142.0, 268.0, 235.0, 26.0],
                NOTE_FORMATS.iter().map(|f| f.1.to_owned()).collect(),
                0,
            );
            f.label(
                "markL",
                [24.0, 306.0, 110.0, 18.0],
                &tr("docR3.customMark", "Custom mark:"),
            );
            f.edit("mark", [142.0, 301.0, 72.0, 24.0], "");
            f.button(
                "symbol",
                [264.0, 300.0, 112.0, 28.0],
                &tr("docR3.symbol", "Symbol..."),
                false,
            );
            f.disable("mark");
            f.disable("symbol");
            f.label(
                "startL",
                [24.0, 338.0, 110.0, 18.0],
                &tr("docR3.startAt", "Start at:"),
            );
            f.spin("start", [142.0, 334.0, 64.0, 24.0], 1, 1, 999);
            f.label(
                "numL",
                [24.0, 369.0, 110.0, 18.0],
                &tr("docR3.numbering", "Numbering:"),
            );
            f.combo(
                "numbering",
                [142.0, 364.0, 235.0, 26.0],
                vec![
                    tr("docR3.continuous", "Continuous"),
                    tr("docR3.restartSection", "Restart each section"),
                    tr("docR3.restartPage", "Restart each page"),
                ],
                0,
            );
            f.group(
                "applyG",
                [10.0, 402.0, 367.0, 1.0],
                &tr("docR3.applyChanges", "Apply changes"),
            );
            f.label(
                "toL",
                [24.0, 433.0, 116.0, 18.0],
                &tr("docR3.applyTo", "Apply changes to:"),
            );
            f.combo(
                "scope",
                [142.0, 428.0, 235.0, 26.0],
                vec![tr("docR3.wholeDocument", "Whole document")],
                0,
            );
            f.button(
                "ok",
                [10.0, 473.0, 112.0, 28.0],
                &tr("docR3.insert", "Insert"),
                true,
            );
            f.button(
                "cancel",
                [137.0, 473.0, 112.0, 28.0],
                &tr("common.cancel", "Cancel"),
                false,
            );
            f.button(
                "apply",
                [264.0, 473.0, 112.0, 28.0],
                &tr("docR3.apply", "Apply"),
                false,
            );
            f
        }
        DocFormKind::NewSource => {
            let mut f = DocForm::new(
                kind,
                &tr("docR3.createSource", "Create Source"),
                (939.0, 380.0),
                vec![],
            );
            f.label(
                "typeL",
                [20.0, 56.0, 140.0, 18.0],
                &tr("docR3.sourceType", "Type of Source"),
            );
            f.combo(
                "type",
                [170.0, 52.0, 300.0, 24.0],
                vec![
                    tr("docR3.srcBook", "Book"),
                    tr("docR3.srcSection", "Book Section"),
                    tr("docR3.srcJournal", "Journal Article"),
                    tr("docR3.srcPeriodical", "Article in a Periodical"),
                    tr("docR3.srcReport", "Report"),
                    tr("docR3.srcWeb", "Web site"),
                ],
                0,
            );
            f.label(
                "langL",
                [500.0, 56.0, 120.0, 18.0],
                &tr("docR3.languageL", "Language"),
            );
            f.combo(
                "lang",
                [620.0, 52.0, 300.0, 24.0],
                vec![tr("docR3.default", "Default")],
                0,
            );
            f.group(
                "fields",
                [10.0, 92.0, 920.0, 1.0],
                &tr("docR3.bibFields", "Bibliography Fields"),
            );
            for (i, (id, key, fb)) in [
                ("author", "docR3.author", "Author"),
                ("title", "docR3.title", "Title"),
                ("year", "docR3.year", "Year"),
                ("city", "docR3.city", "City"),
                ("publisher", "docR3.publisher", "Publisher"),
            ]
            .into_iter()
            .enumerate()
            {
                let y = 112.0 + i as f32 * 32.0;
                let lid = ["fa", "fb", "fc", "fd", "fe"][i];
                f.label(lid, [40.0, y + 4.0, 160.0, 18.0], &tr(key, fb));
                f.edit(
                    id,
                    [220.0, y, 700.0, 24.0],
                    if id == "author" { &c.selection } else { "" },
                );
            }
            f.label(
                "tagL",
                [20.0, 300.0, 100.0, 18.0],
                &tr("docR3.tagName", "Tag name"),
            );
            f.edit("tag", [20.0, 322.0, 200.0, 24.0], "");
            f.ok_cancel(s);
            f
        }
        DocFormKind::Sources => {
            let mut f = DocForm::new(
                kind,
                &tr("docR3.sourceManager", "Source Manager"),
                (1045.0, 603.0),
                vec![],
            );
            f.label(
                "masterL",
                [20.0, 90.0, 200.0, 18.0],
                &tr("docR3.masterList", "Master List"),
            );
            f.list("master", [20.0, 112.0, 400.0, 320.0], vec![], None);
            f.label(
                "currentL",
                [610.0, 90.0, 200.0, 18.0],
                &tr("docR3.currentList", "Current List"),
            );
            f.list(
                "current",
                [610.0, 112.0, 415.0, 320.0],
                c.sources
                    .iter()
                    .map(|(tag, desc, cited)| {
                        format!("{}{desc}", if *cited { "✓ " } else { "   " })
                            .replace("  ", if tag.is_empty() { " " } else { "  " })
                    })
                    .collect(),
                (!c.sources.is_empty()).then_some(0),
            );
            f.ids = c.sources.iter().map(|s| s.0.clone()).collect();
            f.button(
                "copy",
                [455.0, 180.0, 120.0, 28.0],
                &tr("docR3.copy", "Copy ->"),
                false,
            );
            f.button(
                "delete",
                [455.0, 220.0, 120.0, 28.0],
                &tr("docR3.delete", "Delete"),
                false,
            );
            f.button(
                "edit",
                [455.0, 260.0, 120.0, 28.0],
                &tr("docR3.edit", "Edit..."),
                false,
            );
            f.button(
                "new",
                [455.0, 300.0, 120.0, 28.0],
                &tr("docR3.new", "New..."),
                false,
            );
            f.disable("copy");
            f.disable("edit");
            f.label(
                "citedNote",
                [610.0, 440.0, 415.0, 18.0],
                &tr("docR3.citedNote", "✓ cited source"),
            );
            f.group(
                "prev",
                [20.0, 470.0, 1005.0, 1.0],
                &tr("docR3.previewApa", "Preview:"),
            );
            f.close_only(s);
            f
        }
        DocFormKind::MarkEntry => {
            let mut f = DocForm::new(
                kind,
                &tr("docR3.markIndexEntry", "Mark Index Entry"),
                (395.0, 462.0),
                vec![],
            );
            f.group(
                "idx",
                [10.0, 48.0, 375.0, 1.0],
                &tr("docR3.tabIndex", "Index"),
            );
            f.label(
                "mainL",
                [24.0, 77.0, 80.0, 18.0],
                &tr("docR3.mainEntry", "Main entry:"),
            );
            f.edit("main", [103.0, 72.0, 282.0, 26.0], &c.selection);
            f.label(
                "subL",
                [24.0, 107.0, 80.0, 18.0],
                &tr("docR3.subentry", "Subentry:"),
            );
            f.edit("sub", [103.0, 102.0, 282.0, 26.0], "");
            f.group(
                "opts",
                [10.0, 140.0, 375.0, 1.0],
                &tr("docR3.options2", "Options"),
            );
            f.radio(
                "xref",
                "opt",
                [24.0, 165.0, 130.0, 20.0],
                &tr("docR3.crossReference", "Cross-reference:"),
                false,
            );
            f.edit(
                "xrefText",
                [160.0, 163.0, 225.0, 26.0],
                &tr("docR3.see", "See"),
            );
            f.radio(
                "page",
                "opt",
                [24.0, 197.0, 200.0, 20.0],
                &tr("docR3.currentPage", "Current page"),
                true,
            );
            f.radio(
                "range",
                "opt",
                [24.0, 227.0, 200.0, 20.0],
                &tr("docR3.pageRange", "Page range"),
                false,
            );
            f.label(
                "bmL",
                [48.0, 257.0, 70.0, 18.0],
                &tr("docR3.bookmark", "Bookmark:"),
            );
            f.combo(
                "bookmark",
                [123.0, 252.0, 262.0, 26.0],
                c.targets.iter().map(|t| t.0.clone()).collect(),
                0,
            );
            f.disable("bookmark");
            f.group(
                "pnf",
                [10.0, 292.0, 375.0, 1.0],
                &tr("docR3.pageNumberFormat", "Page number format"),
            );
            f.check(
                "bold",
                [24.0, 316.0, 120.0, 18.0],
                &tr("docR3.bold", "Bold"),
                false,
            );
            f.check(
                "italic",
                [24.0, 341.0, 120.0, 18.0],
                &tr("docR3.italic", "Italic"),
                false,
            );
            f.label(
                "stays",
                [10.0, 368.0, 375.0, 36.0],
                &tr(
                    "docR3.staysOpen",
                    "This dialog box stays open so that you can mark multiple index entries.",
                ),
            );
            f.button(
                "ok",
                [18.0, 421.0, 112.0, 28.0],
                &tr("docR3.mark", "Mark"),
                true,
            );
            f.button(
                "markAll",
                [145.0, 421.0, 112.0, 28.0],
                &tr("docR3.markAll", "Mark All"),
                false,
            );
            f.button(
                "cancel",
                [272.0, 421.0, 112.0, 28.0],
                &tr("common.cancel", "Cancel"),
                false,
            );
            f
        }
        DocFormKind::CrossRef => {
            let mut f = DocForm::new(
                kind,
                &tr("docR3.crossReference2", "Cross-reference"),
                (572.0, 439.0),
                vec![],
            );
            f.label(
                "typeL",
                [10.0, 48.0, 200.0, 18.0],
                &tr("docR3.referenceType", "Reference type:"),
            );
            f.combo(
                "type",
                [10.0, 68.0, 312.0, 26.0],
                vec![tr("docR3.bookmarksCaptions", "Bookmark / Caption")],
                0,
            );
            f.label(
                "toL",
                [338.0, 48.0, 200.0, 18.0],
                &tr("docR3.insertReferenceTo", "Insert reference to:"),
            );
            f.combo(
                "to",
                [338.0, 68.0, 224.0, 26.0],
                vec![
                    tr("docR3.refText", "Text"),
                    tr("docR3.refPage", "Page number"),
                ],
                0,
            );
            f.check(
                "hyperlink",
                [10.0, 103.0, 220.0, 18.0],
                &tr("docR3.asHyperlink", "Insert as hyperlink"),
                true,
            );
            f.check(
                "above",
                [338.0, 103.0, 220.0, 18.0],
                &tr("docR3.aboveBelow", "Include above/below"),
                false,
            );
            f.disable("above");
            f.label(
                "forL",
                [10.0, 156.0, 300.0, 18.0],
                &tr("docR3.forWhich", "For which bookmark or caption:"),
            );
            f.list(
                "list",
                [10.0, 176.0, 552.0, 207.0],
                c.targets.iter().map(|t| t.1.clone()).collect(),
                (!c.targets.is_empty()).then_some(0),
            );
            f.ids = c.targets.iter().map(|t| t.0.clone()).collect();
            f.button(
                "ok",
                [323.0, 398.0, 112.0, 28.0],
                &tr("docR3.insert", "Insert"),
                true,
            );
            f.button(
                "cancel",
                [450.0, 398.0, 112.0, 28.0],
                &tr("docR3.close", "Close"),
                false,
            );
            f
        }
        DocFormKind::Caption => {
            let mut f = DocForm::new(
                kind,
                &tr("docR3.captionTitle", "Caption"),
                (447.0, 307.0),
                vec![],
            );
            f.label(
                "capL",
                [10.0, 48.0, 100.0, 18.0],
                &tr("docR3.caption", "Caption:"),
            );
            f.edit(
                "caption",
                [10.0, 68.0, 427.0, 26.0],
                &format!("{} 1", CAPTION_LABELS[0]),
            );
            f.group(
                "opts",
                [10.0, 106.0, 427.0, 1.0],
                &tr("docR3.options2", "Options"),
            );
            f.label(
                "labelL",
                [24.0, 135.0, 70.0, 18.0],
                &tr("docR3.label", "Label:"),
            );
            f.combo(
                "label",
                [101.0, 130.0, 336.0, 26.0],
                CAPTION_LABELS.iter().map(|s| (*s).to_owned()).collect(),
                0,
            );
            f.label(
                "posL",
                [24.0, 166.0, 70.0, 18.0],
                &tr("docR3.position", "Position:"),
            );
            f.combo(
                "position",
                [101.0, 161.0, 336.0, 26.0],
                vec![tr("docR3.belowItem", "Below selected item")],
                0,
            );
            f.disable("position");
            f.check(
                "exclude",
                [24.0, 196.0, 300.0, 18.0],
                &tr("docR3.excludeLabel", "Exclude label from caption"),
                false,
            );
            f.button(
                "newLabel",
                [24.0, 220.0, 130.0, 28.0],
                &tr("docR3.newLabel", "New Label..."),
                false,
            );
            f.button(
                "deleteLabel",
                [170.0, 220.0, 130.0, 28.0],
                &tr("docR3.deleteLabel", "Delete Label"),
                false,
            );
            f.button(
                "numbering",
                [317.0, 220.0, 120.0, 28.0],
                &tr("docR3.numberingB", "Numbering..."),
                false,
            );
            f.disable("newLabel");
            f.disable("deleteLabel");
            f.disable("numbering");
            f.button(
                "autoCaption",
                [24.0, 266.0, 130.0, 28.0],
                &tr("docR3.autoCaption", "AutoCaption..."),
                false,
            );
            f.disable("autoCaption");
            f.button(
                "ok",
                [204.0, 266.0, 112.0, 28.0],
                &tr("common.ok", "OK"),
                true,
            );
            f.button(
                "cancel",
                [325.0, 266.0, 112.0, 28.0],
                &tr("common.cancel", "Cancel"),
                false,
            );
            f
        }
        DocFormKind::Compare => {
            let mut f = DocForm::new(
                kind,
                &tr("docR3.compareDocuments", "Compare Documents"),
                (672.0, 213.0),
                vec![],
            );
            f.label(
                "origL",
                [10.0, 48.0, 200.0, 18.0],
                &tr("docR3.originalDocument", "Original document"),
            );
            // Both documents are drop-down boxes with a folder button, as the reference;
            // the revised one starts as this document.
            f.canvas("origName", [10.0, 68.0, 262.0, 26.0]);
            f.canvas("pickOriginal", [282.0, 68.0, 26.0, 26.0]);
            f.label(
                "origLab",
                [10.0, 102.0, 130.0, 18.0],
                &tr("docR3.labelChanges", "Label changes with"),
            );
            f.edit("origAuthor", [134.0, 98.0, 186.0, 26.0], "");
            f.label(
                "revL",
                [350.0, 48.0, 200.0, 18.0],
                &tr("docR3.revisedDocument", "Revised document"),
            );
            f.canvas("revName", [350.0, 68.0, 262.0, 26.0]);
            f.canvas("pickRevised", [622.0, 68.0, 26.0, 26.0]);
            f.canvas("swap", [320.0, 134.0, 26.0, 26.0]);
            f.preview = vec![tr("docR3.thisDocument", "(this document)")];
            f.label(
                "revLab",
                [350.0, 102.0, 130.0, 18.0],
                &tr("docR3.labelChanges", "Label changes with"),
            );
            f.edit("revAuthor", [474.0, 98.0, 186.0, 26.0], "");
            f.button(
                "more",
                [10.0, 168.0, 112.0, 28.0],
                &tr("docR3.more", "More >>"),
                false,
            );
            f.disable("more");
            f.button(
                "ok",
                [422.0, 168.0, 112.0, 28.0],
                &tr("common.ok", "OK"),
                true,
            );
            f.button(
                "cancel",
                [549.0, 168.0, 112.0, 28.0],
                &tr("common.cancel", "Cancel"),
                false,
            );
            f
        }
        DocFormKind::Language => {
            let mut f = DocForm::new(
                kind,
                &tr("docR3.languageTitle", "Language"),
                (387.0, 432.0),
                vec![],
            );
            f.label(
                "forL",
                [10.0, 46.0, 360.0, 18.0],
                &tr("docR3.changeFor", "Change proofing language for:"),
            );
            f.radio(
                "selection",
                "scope",
                [10.0, 69.0, 110.0, 20.0],
                &tr("docR3.selectedText", "Selected text"),
                true,
            );
            f.radio(
                "document",
                "scope",
                [124.0, 69.0, 200.0, 20.0],
                &tr("docR3.currentDocument", "Current Document"),
                false,
            );
            f.label(
                "markL",
                [10.0, 94.0, 360.0, 18.0],
                &tr("docR3.markAs", "Mark selected text as:"),
            );
            let cur = c.lang.as_deref().and_then(|l| {
                LANGUAGES.iter().position(|(tag, _)| {
                    tag.eq_ignore_ascii_case(l) || tag.split('-').next() == l.split('-').next()
                })
            });
            f.list(
                "list",
                [10.0, 113.0, 365.0, 164.0],
                LANGUAGES.iter().map(|l| l.1.to_owned()).collect(),
                Some(cur.unwrap_or(0)),
            );
            f.label(
                "note",
                [10.0, 284.0, 370.0, 36.0],
                &tr("docR3.languageNote", "The speller and other proofing tools automatically use dictionaries of the selected language, if available."),
            );
            f.check(
                "noProof",
                [10.0, 334.0, 300.0, 18.0],
                &tr("docR3.noProof", "Do not check spelling or grammar"),
                c.no_proof,
            );
            f.check(
                "detect",
                [10.0, 359.0, 300.0, 18.0],
                &tr("docR3.detect", "Detect language automatically"),
                true,
            );
            f.disable("detect");
            f.button(
                "setDefault",
                [10.0, 391.0, 112.0, 28.0],
                &tr("docR3.setAsDefault", "Set As Default"),
                false,
            );
            f.disable("setDefault");
            f.button(
                "ok",
                [137.0, 391.0, 112.0, 28.0],
                &tr("common.ok", "OK"),
                true,
            );
            f.button(
                "cancel",
                [264.0, 391.0, 112.0, 28.0],
                &tr("common.cancel", "Cancel"),
                false,
            );
            f
        }
        DocFormKind::Envelopes => {
            let tabs = vec![
                tr("docR3.envelopes", "Envelopes"),
                tr("docR3.labels", "Labels"),
            ];
            let mut f = DocForm::new(
                kind,
                &tr("docR3.envelopesTitle", "Envelopes and Labels"),
                (715.0, 619.0),
                tabs,
            );
            f.label_on(
                0,
                "dL",
                [24.0, 92.0, 200.0, 18.0],
                &tr("docR3.deliveryAddress", "Delivery address:"),
            );
            let lines: Vec<String> = c.selection.lines().map(str::to_owned).collect();
            for (i, id) in ["d1", "d2", "d3", "d4"].into_iter().enumerate() {
                f.edit_on(
                    0,
                    id,
                    [24.0, 112.0 + i as f32 * 33.0, 666.0, 34.0],
                    lines.get(i).map_or("", String::as_str),
                );
            }
            f.check_on(
                0,
                "postage",
                [24.0, 262.0, 300.0, 18.0],
                &tr("docR3.postage", "Add electronic postage"),
                false,
            );
            f.disable("postage");
            f.label_on(
                0,
                "rL",
                [24.0, 290.0, 200.0, 18.0],
                &tr("docR3.returnAddress", "Return address:"),
            );
            f.check_on(
                0,
                "omit",
                [308.0, 290.0, 80.0, 18.0],
                &tr("docR3.omit", "Omit"),
                false,
            );
            for (i, id) in ["r1", "r2", "r3"].into_iter().enumerate() {
                f.edit_on(0, id, [24.0, 308.0 + i as f32 * 33.0, 328.0, 34.0], "");
            }
            f.form.add(
                "previewG",
                Some(0),
                [376.0, 316.0, 216.0, 1.0],
                Kind::Group(tr("docR3.preview", "Preview")),
            );
            f.form.add(
                "envPreview",
                Some(0),
                [384.0, 340.0, 200.0, 124.0],
                Kind::Canvas,
            );
            f.form.add(
                "feedG",
                Some(0),
                [591.0, 316.0, 98.0, 1.0],
                Kind::Group(tr("docR3.feed", "Feed")),
            );
            f.form
                .add("feed", Some(0), [599.0, 340.0, 82.0, 124.0], Kind::Canvas);
            f.label_on(
                0,
                "verify",
                [24.0, 486.0, 600.0, 18.0],
                &tr(
                    "docR3.verify",
                    "Verify that an envelope is loaded before printing.",
                ),
            );
            f.button_on(
                0,
                "print",
                [24.0, 517.0, 112.0, 28.0],
                &tr("docR3.print", "Print"),
            );
            f.button_on(
                0,
                "addToDocument",
                [151.0, 517.0, 133.0, 28.0],
                &tr("docR3.addToDocument", "Add to Document"),
            );
            f.button_on(
                0,
                "envOptions",
                [404.0, 517.0, 112.0, 28.0],
                &tr("docR3.options", "Options..."),
            );
            f.button_on(
                0,
                "epostage",
                [531.0, 517.0, 159.0, 28.0],
                &tr("docR3.epostage", "E-postage Properties..."),
            );
            f.disable("envOptions");
            f.disable("epostage");
            f.label_on(
                1,
                "aL",
                [24.0, 92.0, 200.0, 18.0],
                &tr("docR3.address", "Address:"),
            );
            for (i, id) in ["l1", "l2", "l3", "l4"].into_iter().enumerate() {
                f.edit_on(
                    1,
                    id,
                    [24.0, 112.0 + i as f32 * 33.0, 666.0, 34.0],
                    lines.get(i).map_or("", String::as_str),
                );
            }
            f.form.add(
                "printG",
                Some(1),
                [24.0, 268.0, 330.0, 1.0],
                Kind::Group(tr("docR3.printG", "Print")),
            );
            f.radio_on(
                1,
                "full",
                "labelPrint",
                [38.0, 290.0, 260.0, 20.0],
                &tr("docR3.fullPage", "Full page of the same label"),
                true,
            );
            f.radio_on(
                1,
                "single",
                "labelPrint",
                [38.0, 318.0, 260.0, 20.0],
                &tr("docR3.singleLabel", "Single label"),
                false,
            );
            f.form.add(
                "labelG",
                Some(1),
                [376.0, 268.0, 314.0, 1.0],
                Kind::Group(tr("docR3.label2", "Label")),
            );
            f.label_on(
                1,
                "labelName",
                [390.0, 290.0, 300.0, 18.0],
                &tr(
                    "docR3.labelPreset",
                    "A4 sheet, 3 × 8 labels (63.5 × 33.9 mm)",
                ),
            );
            f.button_on(
                1,
                "printLabels",
                [24.0, 517.0, 112.0, 28.0],
                &tr("docR3.print", "Print"),
            );
            f.button_on(
                1,
                "newDocument",
                [151.0, 517.0, 133.0, 28.0],
                &tr("docR3.newDocument", "New Document"),
            );
            let (w, h) = f.form.size;
            f.button(
                "cancel",
                [w - 131.0, h - 41.0, 112.0, 28.0],
                &tr("common.cancel", "Cancel"),
                false,
            );
            f
        }
        DocFormKind::Protect => {
            let mut f = DocForm::new(
                kind,
                &tr("docR3.startEnforcing", "Start Enforcing Protection"),
                (420.0, 250.0),
                vec![],
            );
            f.group(
                "pwG",
                [10.0, 48.0, 400.0, 1.0],
                &tr("docR3.protectionMethod", "Protection method"),
            );
            f.label(
                "pwL",
                [24.0, 76.0, 200.0, 18.0],
                &tr("docR3.newPassword", "Enter new password (optional):"),
            );
            f.edit("pw", [24.0, 96.0, 372.0, 24.0], "");
            f.label(
                "pw2L",
                [24.0, 130.0, 200.0, 18.0],
                &tr("docR3.reenterPassword", "Reenter password to confirm:"),
            );
            f.edit("pw2", [24.0, 150.0, 372.0, 24.0], "");
            f.ok_cancel(s);
            f
        }
        DocFormKind::Recipients => {
            let mut f = DocForm::new(
                kind,
                &tr("docR3.mailMergeRecipients", "Mail Merge Recipients"),
                (760.0, 460.0),
                vec![],
            );
            f.label(
                "intro",
                [12.0, 48.0, 730.0, 18.0],
                &tr(
                    "docR3.recipientsIntro",
                    "This is the list of recipients that will be used in your merge.",
                ),
            );
            let (fields, rows) = &c.recipients;
            f.label("head", [12.0, 76.0, 730.0, 18.0], &fields.join("  |  "));
            f.list(
                "rows",
                [12.0, 96.0, 736.0, 300.0],
                rows.iter().map(|r| r.join("  |  ")).collect(),
                (!rows.is_empty()).then_some(0),
            );
            let (w, h) = f.form.size;
            f.button(
                "cancel",
                [w - 131.0, h - 48.0, 112.0, 28.0],
                &tr("common.ok", "OK"),
                true,
            );
            f
        }
        DocFormKind::TocOptions => {
            let mut f = DocForm::new(
                kind,
                &tr("docR3.tocOptions", "Table of Contents Options"),
                (420.0, 470.0),
                vec![],
            );
            f.label(
                "buildL",
                [16.0, 48.0, 380.0, 18.0],
                &tr("docR3.buildFrom", "Build table of contents from:"),
            );
            f.label(
                "stylesL",
                [32.0, 72.0, 200.0, 18.0],
                &tr("docR3.availableStyles", "Available styles:"),
            );
            f.label(
                "levelL",
                [300.0, 72.0, 100.0, 18.0],
                &tr("docR3.tocLevel", "TOC level:"),
            );
            for (i, id) in TOC_LEVEL_IDS.iter().enumerate() {
                let y = 96.0 + i as f32 * 30.0;
                let lid = TOC_STYLE_LABELS[i];
                f.label(
                    lid,
                    [32.0, y + 4.0, 250.0, 18.0],
                    &format!("{} {}", tr("docR3.heading", "Heading"), i + 1),
                );
                f.edit(
                    id,
                    [300.0, y, 60.0, 24.0],
                    &if i < 3 {
                        (i + 1).to_string()
                    } else {
                        String::new()
                    },
                );
            }
            f.ok_cancel(s);
            f
        }
        DocFormKind::Unprotect => {
            let mut f = DocForm::new(
                kind,
                &tr("docR3.unprotect", "Unprotect Document"),
                (360.0, 170.0),
                vec![],
            );
            f.label(
                "pwL",
                [20.0, 56.0, 200.0, 18.0],
                &tr("docR3.password", "Password:"),
            );
            f.edit("pw", [20.0, 78.0, 320.0, 24.0], "");
            f.ok_cancel(s);
            f
        }
        // Measured on the reference at 125 %: 673 × 475.
        DocFormKind::DateTime => {
            let mut f = DocForm::new(
                kind,
                &tr("docR4.dateTimeTitle", "Date and Time"),
                (673.0, 475.0),
                vec![],
            );
            f.label(
                "formatsL",
                [18.0, 47.0, 300.0, 18.0],
                &tr("docR4.availableFormats", "Available formats:"),
            );
            f.list(
                "formats",
                [18.0, 67.0, 444.0, 343.0],
                DATE_PICTURES
                    .iter()
                    .map(|p| format_date(p, c.now))
                    .collect(),
                Some(c.date_default.min(DATE_PICTURES.len() - 1)),
            );
            f.label(
                "langL",
                [481.0, 47.0, 170.0, 18.0],
                &tr("docR4.language", "Language:"),
            );
            f.combo(
                "lang",
                [481.0, 67.0, 173.0, 30.0],
                vec![tr("docR4.langEnglish", "English (United States)")],
                0,
            );
            f.check(
                "update",
                [481.0, 393.0, 180.0, 20.0],
                &tr("docR4.updateAuto", "Update automatically"),
                true,
            );
            f.button(
                "setDefault",
                [18.0, 426.0, 112.0, 30.0],
                &tr("docR3.setAsDefault", "Set As Default"),
                false,
            );
            f.button(
                "ok",
                [415.0, 426.0, 112.0, 30.0],
                &tr("common.ok", "OK"),
                true,
            );
            f.button(
                "cancel",
                [542.0, 426.0, 112.0, 30.0],
                &tr("common.cancel", "Cancel"),
                false,
            );
            f
        }
        // Measured on the reference at 125 %: 381 × 258.
        DocFormKind::AskRule
        | DocFormKind::FillInRule
        | DocFormKind::IfRule
        | DocFormKind::NextIfRule
        | DocFormKind::SetRule
        | DocFormKind::SkipIfRule
        | DocFormKind::MatchFields
        | DocFormKind::FindRecipient
        | DocFormKind::ManageStyles => r5::open(kind, s, c),
        DocFormKind::AltText
        | DocFormKind::Encrypt
        | DocFormKind::Inspect
        | DocFormKind::Compatibility => r7::open(kind, s),
        DocFormKind::FindFont
        | DocFormKind::FindParagraph
        | DocFormKind::FindTabs
        | DocFormKind::FindLanguage
        | DocFormKind::FindFrame
        | DocFormKind::FindStyle => findfmt11::open(kind, s, c),
        DocFormKind::TableProps => tprops11::open(s, c),
        DocFormKind::TableOptions => tprops11::open_options(s, c),
        DocFormKind::InsertTable => tprops11::open_insert(s),
        DocFormKind::TablePosition | DocFormKind::BordersShading | DocFormKind::NewBlock => {
            r12::open(kind, s, c)
        }
        DocFormKind::Paragraph
        | DocFormKind::Tabs
        | DocFormKind::PageSetup
        | DocFormKind::BorderOptions
        | DocFormKind::PaneOptions
        | DocFormKind::LineNumbers => r13::open(kind, s, c),
        DocFormKind::Tof
        | DocFormKind::Toa
        | DocFormKind::MarkCitation
        | DocFormKind::Watermark
        | DocFormKind::TofOptions
        | DocFormKind::Categories => r14::open(kind, s, c),
        DocFormKind::Colors | DocFormKind::ThemeColors | DocFormKind::ThemeFonts => {
            r15::open(kind, s, c)
        }
        DocFormKind::TrackOptions
        | DocFormKind::AdvTrackOptions
        | DocFormKind::MergeNew
        | DocFormKind::MergePrint
        | DocFormKind::CheckErrors
        | DocFormKind::MergeReport
        | DocFormKind::NewList
        | DocFormKind::MergeEmail
        | DocFormKind::NlFind
        | DocFormKind::NlColumns => r16::open(kind, s, c),
        DocFormKind::FillEffects => r18::open(s, c),
        DocFormKind::Bookmark
        | DocFormKind::InsertObject
        | DocFormKind::Macros
        | DocFormKind::OnlineVideo
        | DocFormKind::MacroSource
        | DocFormKind::Icons
        | DocFormKind::StyleNumbering
        | DocFormKind::CustomizeKeyboard
        | DocFormKind::StyleEffects
        | DocFormKind::StyleFrame
        | DocFormKind::OnlinePicture => r17::open(kind, s, c),
        DocFormKind::ChangeIcon | DocFormKind::AddinList => r22::open(kind, s, c),
        DocFormKind::Font23
        | DocFormKind::SetNumbering
        | DocFormKind::DefineBullet
        | DocFormKind::DefineNumber
        | DocFormKind::DefineMultilevel
        | DocFormKind::SortText => r23::open(kind, s, c),
        DocFormKind::Zoom => {
            let mut f = DocForm::new(kind, &tr("docR4.zoomTitle", "Zoom"), (381.0, 258.0), vec![]);
            f.label(
                "zoomTo",
                [18.0, 46.0, 120.0, 18.0],
                &tr("docR4.zoomTo", "Zoom to"),
            );
            let pct = (c.zoom * 100.0).round() as i32;
            let radios = [
                ("z200", [32.0, 74.0, 100.0, 20.0], "200%", pct == 200),
                ("z100", [32.0, 103.0, 100.0, 20.0], "100%", pct == 100),
                ("z75", [32.0, 132.0, 100.0, 20.0], "75%", pct == 75),
                ("pageWidth", [148.0, 74.0, 110.0, 20.0], "Page width", false),
                (
                    "textWidth",
                    [148.0, 103.0, 110.0, 20.0],
                    "Text width",
                    false,
                ),
                (
                    "wholePage",
                    [148.0, 132.0, 110.0, 20.0],
                    "Whole page",
                    false,
                ),
                (
                    "manyPages",
                    [262.0, 74.0, 110.0, 20.0],
                    "Many pages:",
                    false,
                ),
            ];
            for (id, r, label, on) in radios {
                let text = if label.ends_with('%') {
                    label.to_owned()
                } else {
                    tr(&format!("docR4.zoom_{id}"), label)
                };
                f.radio(id, "zoom", r, &text, on);
            }
            f.disable("manyPages");
            f.label(
                "pctL",
                [32.0, 168.0, 60.0, 18.0],
                &tr("docR4.percent", "Percent:"),
            );
            f.spin("pct", [92.0, 164.0, 82.0, 26.0], pct, 10, 500);
            f.button(
                "ok",
                [137.0, 210.0, 112.0, 30.0],
                &tr("common.ok", "OK"),
                true,
            );
            f.button(
                "cancel",
                [250.0, 210.0, 112.0, 30.0],
                &tr("common.cancel", "Cancel"),
                false,
            );
            f
        }
    }
}
