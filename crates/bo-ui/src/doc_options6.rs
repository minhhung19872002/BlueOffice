//! File › Options of BlueDoc (parity round 6): the reference word processor's Options dialog
//! — categories General, Display, Proofing, Save, Language, Accessibility, Advanced,
//! Customize Ribbon, Quick Access Toolbar, Add-ins, Trust Center on the left, the chosen
//! category's page on a white face — measured at 125 % (`show/doc-1xx-options*`): window
//! 1050 × 856 px, the same frame as BlueSheet's Options (`sheet_options.rs`). Coordinates are
//! those reference pixels.
//!
//! What BlueDoc supports is live and kept in the application preferences ([`DocPrefs`],
//! `%LOCALAPPDATA%\BlueOffice\doc-prefs.txt`); the rest is greyed:
//!
//! * General: ScreenTip style; user name and initials (the author of comments and tracked
//!   changes);
//! * Display: show all formatting marks;
//! * Proofing: check spelling as you type;
//! * Save: the format Save As offers first, the default local file location;
//! * Language: the interface language (after a restart);
//! * Advanced: units of measurement, the number of recent documents shown;
//! * Quick Access Toolbar: the commands of the title bar's quick access buttons.
use super::form::{Event, Form, Kind};
use crate::{Font, Input, Key, Painter, TextMeasure};
use bo_render::{PathSeg, Rgba};

/// Round 21: Customize Ribbon and Quick Access Toolbar pages in full.
#[path = "doc_options21.rs"]
pub(crate) mod r21;

/// Application preferences of BlueDoc.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DocPrefs {
    /// ScreenTip style: 0 with descriptions, 1 without, 2 none.
    pub(crate) screentips: u8,
    /// Author of comments and revisions; their initials.
    pub(crate) user_name: String,
    pub(crate) initials: String,
    /// Show all formatting marks.
    pub(crate) show_marks: bool,
    /// Formatting marks always shown (round 7): tab characters, spaces, paragraph marks,
    /// hidden text, optional hyphens, object anchors (the reference's default: anchors).
    pub(crate) mark_kinds: [bool; 6],
    /// Wavy spelling marks while typing.
    pub(crate) check_spelling: bool,
    /// Format Save As offers first (index into the Save page's list: 0 docx, 1 dotx).
    pub(crate) save_format: u8,
    /// Default local file location.
    pub(crate) folder: String,
    /// Interface language (`en-US`, `vi-VN`).
    pub(crate) locale: String,
    /// Units: 0 inches, 1 centimetres, 2 regional default.
    pub(crate) units: u8,
    /// Recent documents shown on the File screen.
    pub(crate) recent_count: u32,
    /// Quick access commands of the title bar.
    pub(crate) quick: Vec<String>,
    /// Round 20: Customize Ribbon, Trust Center ([`super::options20::Extra`]).
    pub(crate) extra: super::options20::Extra,
}

impl Default for DocPrefs {
    fn default() -> Self {
        Self {
            screentips: 0,
            user_name: String::new(),
            initials: String::new(),
            show_marks: false,
            mark_kinds: [false, false, false, false, false, true],
            check_spelling: true,
            save_format: 0,
            folder: String::new(),
            locale: String::new(),
            units: 2,
            recent_count: 50,
            quick: QUICK_DEFAULT.iter().map(|s| (*s).to_owned()).collect(),
            extra: super::options20::Extra::default(),
        }
    }
}

/// The title bar's quick access commands out of the box.
pub(crate) const QUICK_DEFAULT: [&str; 3] = ["file.save", "edit.undo", "edit.redo"];

/// Commands the Quick Access Toolbar page offers: (command id, label key, fallback).
pub(crate) const QUICK_CHOICES: [(&str, &str, &str); 12] = [
    ("file.new", "commands.file.new", "New"),
    ("file.open", "commands.file.open", "Open"),
    ("file.save", "commands.file.save", "Save"),
    ("file.print", "commands.file.print", "Print"),
    ("file.exportPdf", "commands.file.exportPdf", "Export to PDF"),
    ("edit.undo", "commands.edit.undo", "Undo"),
    ("edit.redo", "commands.edit.redo", "Redo"),
    ("edit.find", "commands.edit.find", "Find"),
    ("edit.replace", "commands.edit.replace", "Replace"),
    (
        "view.formattingMarks",
        "commands.view.formattingMarks",
        "Show/Hide ¶",
    ),
    ("insert.table", "commands.insert.table", "Insert Table"),
    ("insert.picture", "commands.insert.picture", "Picture"),
];

impl DocPrefs {
    /// Read `key=value` lines (unknown keys and bad values keep their defaults).
    #[must_use]
    pub(crate) fn from_text(text: &str) -> Self {
        let mut p = Self::default();
        for line in text.lines() {
            let Some((k, v)) = line.split_once('=') else {
                continue;
            };
            let v = v.trim();
            match k.trim() {
                "screentips" => p.screentips = v.parse().unwrap_or(0).min(2),
                "userName" => v.clone_into(&mut p.user_name),
                "initials" => v.clone_into(&mut p.initials),
                "showMarks" => p.show_marks = v == "1",
                "markKinds" => {
                    for (i, c) in v.chars().take(6).enumerate() {
                        p.mark_kinds[i] = c == '1';
                    }
                }
                "checkSpelling" => p.check_spelling = v != "0",
                "saveFormat" => p.save_format = v.parse().unwrap_or(0).min(1),
                "folder" => v.clone_into(&mut p.folder),
                "locale" => {
                    if matches!(v, "en-US" | "vi-VN" | "") {
                        v.clone_into(&mut p.locale);
                    }
                }
                "units" => p.units = v.parse().unwrap_or(2).min(2),
                "recentCount" => p.recent_count = v.parse().unwrap_or(50).min(50),
                "quick" => {
                    p.quick = v
                        .split(',')
                        .map(str::trim)
                        .filter(|s| {
                            QUICK_CHOICES.iter().any(|(id, ..)| id == s)
                                || *s == crate::doc_ribbon21::SEPARATOR
                                || crate::doc_ribbon21::static_command(s).is_some()
                        })
                        .map(str::to_owned)
                        .collect();
                }
                k => p.extra.read(k, v),
            }
        }
        p
    }

    /// The preferences as `key=value` lines.
    #[must_use]
    pub(crate) fn to_text(&self) -> String {
        let b = |v: bool| if v { "1" } else { "0" };
        format!(
            "screentips={}\nuserName={}\ninitials={}\nshowMarks={}\nmarkKinds={}\ncheckSpelling={}\nsaveFormat={}\nfolder={}\nlocale={}\nunits={}\nrecentCount={}\nquick={}\n",
            self.screentips,
            self.user_name.replace('\n', " "),
            self.initials.replace('\n', " "),
            b(self.show_marks),
            self.mark_kinds.iter().map(|k| b(*k)).collect::<String>(),
            b(self.check_spelling),
            self.save_format,
            self.folder.replace('\n', " "),
            self.locale,
            self.units,
            self.recent_count,
            self.quick.join(","),
        ) + &self.extra.to_text()
    }

    /// Initials from a user name when none are given ("An Nguyen" → "AN").
    #[must_use]
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn initials_or_default(&self) -> String {
        if !self.initials.trim().is_empty() {
            return self.initials.trim().to_owned();
        }
        self.user_name
            .split_whitespace()
            .filter_map(|w| w.chars().next())
            .flat_map(char::to_uppercase)
            .collect()
    }
}

/// Categories: (label key, fallback).
const CATS: [(&str, &str); 11] = [
    ("docR6.opt.general", "General"),
    ("docR6.opt.display", "Display"),
    ("docR6.opt.proofing", "Proofing"),
    ("docR6.opt.save", "Save"),
    ("docR6.opt.language", "Language"),
    ("docR6.opt.accessibility", "Accessibility"),
    ("docR6.opt.advanced", "Advanced"),
    ("docR6.opt.ribbon", "Customize Ribbon"),
    ("docR6.opt.qat", "Quick Access Toolbar"),
    ("docR6.opt.addins", "Add-ins"),
    ("docR6.opt.trust", "Trust Center"),
];
/// Centre of each category row (reference pixels from the dialog's top, measured).
const CAT_Y: [f32; 11] = [
    68.0, 109.0, 150.0, 191.0, 232.0, 273.0, 319.0, 368.0, 414.0, 463.0, 504.0,
];
/// Separators under Advanced and Quick Access Toolbar.
const CAT_SEP: [f32; 2] = [342.0, 437.0];
/// The category list (reference pixels).
const LIST: [f32; 4] = [16.0, 48.0, 202.0, 750.0];

/// What the dialog asks of the window.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum OptOut {
    None,
    Redraw,
    Close,
    /// OK with these preferences.
    Apply(DocPrefs),
    /// Proofing › AutoCorrect Options… (round 8): open the AutoCorrect dialog over this one.
    AutoCorrect,
    /// Round 21: import / export a customisation file, Customize Keyboard.
    R21(r21::Out),
    /// Round 22: close Options and run this window command (Add-ins › Manage › Go…).
    R22(String),
}

/// The open Options dialog.
pub(crate) struct OptionsDialog {
    form: Form,
    page: usize,
    prefs: DocPrefs,
    pointer: (f32, f32),
    /// Labels of the categories (translated).
    cats: Vec<String>,
    /// The Add-ins page's table (round 20).
    addins: super::options20::AddinTable,
    /// Customize Ribbon and Quick Access Toolbar (round 21).
    r21: r21::State,
}

/// Translated text of `key`, or `fallback`.
fn tr(s: &crate::strings::Strings, key: &str, fallback: &str) -> String {
    let v = s.get(key);
    if v == key {
        fallback.to_owned()
    } else {
        v.to_owned()
    }
}

impl OptionsDialog {
    /// File › Options with the current preferences.
    #[allow(clippy::too_many_lines)]
    pub(crate) fn open(s: &crate::strings::Strings, p: &DocPrefs) -> Self {
        let t = |k: &str, f: &str| tr(s, k, f);
        let mut f = Form::new(
            &t("docR6.opt.title", "BlueDoc Options"),
            (1040.0, 848.0),
            vec![],
        );
        // Text about 1.3 times the classic dialogs' (as BlueSheet's Options), bold headings
        // over a rule.
        f.set_font_scale(1.3);
        f.set_headings(true);
        f.set_radios(&["gBest", "gCompat"]);
        f.add("bg", None, [1.0, 38.0, 1038.0, 809.0], Kind::Canvas);
        f.add("cats", None, LIST, Kind::Canvas);
        let check = |label: String, on: bool| Kind::Check { label, on };
        let heading = |f: &mut Form, id: &'static str, page: usize, y: f32, text: String| {
            f.add(id, Some(page), [241.0, y, 755.0, 0.0], Kind::Group(text));
        };
        let intro =
            |f: &mut Form, id: &'static str, icon: &'static str, page: usize, text: String| {
                f.add(
                    id,
                    Some(page),
                    [305.0, 70.0, 660.0, 24.0],
                    Kind::Label(text),
                );
                f.add(icon, Some(page), [258.0, 56.0, 40.0, 46.0], Kind::Canvas);
            };
        let checks = |f: &mut Form,
                      page: usize,
                      x: f32,
                      y0: f32,
                      rows: &[(&'static str, String, bool, bool)]| {
            for (i, (id, label, on, live)) in rows.iter().enumerate() {
                f.add(
                    id,
                    Some(page),
                    [x, y0 + 36.0 * i as f32, 600.0, 24.0],
                    check(label.clone(), *on),
                );
                if !live {
                    f.enable(id, false);
                }
            }
        };
        // ------------------------------------------------------------ General
        intro(
            &mut f,
            "gIntro",
            "icon0",
            0,
            t(
                "docR6.opt.generalIntro",
                "General options for working with BlueDoc.",
            ),
        );
        heading(
            &mut f,
            "gUi",
            0,
            130.0,
            t("docR6.opt.ui", "User Interface options"),
        );
        f.add(
            "gDisplaysL",
            Some(0),
            [257.0, 176.0, 400.0, 24.0],
            Kind::Label(t("docR6.opt.displays", "When using multiple displays:")),
        );
        for (i, (id, key, fb, on)) in [
            (
                "gBest",
                "docR6.opt.best",
                "Optimize for best appearance",
                true,
            ),
            (
                "gCompat",
                "docR6.opt.compat",
                "Optimize for compatibility (application restart required)",
                false,
            ),
        ]
        .into_iter()
        .enumerate()
        {
            f.add(
                id,
                Some(0),
                [281.0, 206.0 + 36.0 * i as f32, 520.0, 24.0],
                check(t(key, fb), on),
            );
            f.enable(id, false);
        }
        checks(
            &mut f,
            0,
            257.0,
            278.0,
            &[
                (
                    "gMini",
                    t("docR6.opt.mini", "Show Mini Toolbar on selection"),
                    true,
                    false,
                ),
                (
                    "gLive",
                    t("docR6.opt.live", "Enable Live Preview"),
                    true,
                    false,
                ),
                (
                    "gDrag",
                    t("docR6.opt.drag", "Update document content while dragging"),
                    true,
                    false,
                ),
                (
                    "gCollapse",
                    t("docR6.opt.collapse", "Collapse the ribbon automatically"),
                    false,
                    false,
                ),
            ],
        );
        f.add(
            "gTipsL",
            Some(0),
            [257.0, 428.0, 130.0, 24.0],
            Kind::Label(t("docR6.opt.tips", "ScreenTip style:")),
        );
        f.add(
            "gTips",
            Some(0),
            [393.0, 422.0, 395.0, 30.0],
            Kind::Combo {
                items: vec![
                    t(
                        "docR6.opt.tipsFull",
                        "Show feature descriptions in ScreenTips",
                    ),
                    t(
                        "docR6.opt.tipsShort",
                        "Don't show feature descriptions in ScreenTips",
                    ),
                    t("docR6.opt.tipsNone", "Don't show ScreenTips"),
                ],
                sel: usize::from(p.screentips.min(2)),
            },
        );
        heading(
            &mut f,
            "gPersonal",
            0,
            472.0,
            t("docR6.opt.personal", "Personalize your copy of BlueOffice"),
        );
        f.add(
            "gUserL",
            Some(0),
            [257.0, 525.0, 150.0, 24.0],
            Kind::Label(t("docR6.opt.userName", "User name:")),
        );
        f.add(
            "gUser",
            Some(0),
            [422.0, 518.0, 347.0, 35.0],
            Kind::Edit {
                value: p.user_name.chars().collect(),
                caret: p.user_name.chars().count(),
            },
        );
        f.add(
            "gInitL",
            Some(0),
            [257.0, 566.0, 150.0, 24.0],
            Kind::Label(t("docR6.opt.initials", "Initials:")),
        );
        f.add(
            "gInit",
            Some(0),
            [422.0, 559.0, 101.0, 35.0],
            Kind::Edit {
                value: p.initials.chars().collect(),
                caret: p.initials.chars().count(),
            },
        );
        f.add(
            "gAlways",
            Some(0),
            [257.0, 600.0, 600.0, 24.0],
            check(
                t(
                    "docR6.opt.always",
                    "Always use these values regardless of sign in to BlueOffice.",
                ),
                true,
            ),
        );
        f.enable("gAlways", false);
        // ------------------------------------------------------------ Display
        intro(
            &mut f,
            "dIntro",
            "icon1",
            1,
            t(
                "docR6.opt.displayIntro",
                "Change how document content is displayed on the screen and when printed.",
            ),
        );
        heading(
            &mut f,
            "dPage",
            1,
            130.0,
            t("docR6.opt.pageDisplay", "Page display options"),
        );
        checks(
            &mut f,
            1,
            257.0,
            176.0,
            &[
                (
                    "dWhite",
                    t(
                        "docR6.opt.whiteSpace",
                        "Show white space between pages in Print Layout view",
                    ),
                    true,
                    false,
                ),
                (
                    "dHigh",
                    t("docR6.opt.highlighter", "Show highlighter marks"),
                    true,
                    false,
                ),
                (
                    "dTips",
                    t("docR6.opt.docTips", "Show document tooltips on hover"),
                    true,
                    false,
                ),
            ],
        );
        heading(
            &mut f,
            "dMarks",
            1,
            298.0,
            t(
                "docR6.opt.marksHeading",
                "Always show these formatting marks on the screen",
            ),
        );
        checks(
            &mut f,
            1,
            257.0,
            344.0,
            &[
                (
                    "dTab",
                    t("docR6.opt.markTab", "Tab characters"),
                    p.mark_kinds[0],
                    true,
                ),
                (
                    "dSpace",
                    t("docR6.opt.markSpace", "Spaces"),
                    p.mark_kinds[1],
                    true,
                ),
                (
                    "dPara",
                    t("docR6.opt.markPara", "Paragraph marks"),
                    p.mark_kinds[2],
                    true,
                ),
                (
                    "dHidden",
                    t("docR6.opt.markHidden", "Hidden text"),
                    p.mark_kinds[3],
                    true,
                ),
                (
                    "dHyphen",
                    t("docR6.opt.markHyphen", "Optional hyphens"),
                    p.mark_kinds[4],
                    true,
                ),
                (
                    "dAnchor",
                    t("docR6.opt.markAnchor", "Object anchors"),
                    p.mark_kinds[5],
                    true,
                ),
                (
                    "dAll",
                    t("docR6.opt.markAll", "Show all formatting marks"),
                    p.show_marks,
                    true,
                ),
            ],
        );
        for (i, id) in ["mTab", "mSpace", "mPara", "mHidden", "mHyphen", "mAnchor"]
            .into_iter()
            .enumerate()
        {
            f.add(
                id,
                Some(1),
                [500.0, 344.0 + 36.0 * i as f32, 40.0, 24.0],
                Kind::Canvas,
            );
        }
        heading(
            &mut f,
            "dPrint",
            1,
            610.0,
            t("docR6.opt.printing", "Printing options"),
        );
        checks(
            &mut f,
            1,
            257.0,
            656.0,
            &[
                (
                    "dPDraw",
                    t(
                        "docR6.opt.printDrawings",
                        "Print drawings created in BlueDoc",
                    ),
                    true,
                    false,
                ),
                (
                    "dPBack",
                    t(
                        "docR6.opt.printBackground",
                        "Print background colors and images",
                    ),
                    false,
                    false,
                ),
                (
                    "dPProps",
                    t("docR6.opt.printProps", "Print document properties"),
                    false,
                    false,
                ),
                (
                    "dPHidden",
                    t("docR6.opt.printHidden", "Print hidden text"),
                    false,
                    false,
                ),
            ],
        );
        // ------------------------------------------------------------ Proofing
        intro(
            &mut f,
            "pIntro",
            "icon2",
            2,
            t(
                "docR6.opt.proofingIntro",
                "Change how BlueDoc corrects and formats your text.",
            ),
        );
        heading(
            &mut f,
            "pAuto",
            2,
            130.0,
            t("docR6.opt.autoCorrect", "AutoCorrect options"),
        );
        f.add(
            "pAutoL",
            Some(2),
            [257.0, 181.0, 500.0, 24.0],
            Kind::Label(t(
                "docR6.opt.autoCorrectL",
                "Change how BlueDoc corrects and formats text as you type:",
            )),
        );
        f.add(
            "pAutoB",
            Some(2),
            [722.0, 176.0, 196.0, 34.0],
            Kind::Button {
                label: t("docR6.opt.autoCorrectB", "AutoCorrect Options..."),
                default: false,
            },
        );
        heading(
            &mut f,
            "pAll",
            2,
            230.0,
            t(
                "docR6.opt.spellAll",
                "When correcting spelling in BlueOffice programs",
            ),
        );
        checks(
            &mut f,
            2,
            257.0,
            276.0,
            &[
                (
                    "pUpper",
                    t("docR6.opt.ignoreUpper", "Ignore words in UPPERCASE"),
                    true,
                    false,
                ),
                (
                    "pNum",
                    t(
                        "docR6.opt.ignoreNumbers",
                        "Ignore words that contain numbers",
                    ),
                    true,
                    false,
                ),
                (
                    "pNet",
                    t("docR6.opt.ignoreNet", "Ignore Internet and file addresses"),
                    true,
                    false,
                ),
                (
                    "pRepeat",
                    t("docR6.opt.repeated", "Flag repeated words"),
                    true,
                    false,
                ),
                (
                    "pMain",
                    t("docR6.opt.mainOnly", "Suggest from main dictionary only"),
                    false,
                    false,
                ),
            ],
        );
        heading(
            &mut f,
            "pDoc",
            2,
            480.0,
            t(
                "docR6.opt.spellDoc",
                "When correcting spelling and grammar in BlueDoc",
            ),
        );
        checks(
            &mut f,
            2,
            257.0,
            526.0,
            &[
                (
                    "pSpell",
                    t("docR6.opt.spellAsType", "Check spelling as you type"),
                    p.check_spelling,
                    true,
                ),
                (
                    "pGrammar",
                    t("docR6.opt.grammarAsType", "Mark grammar errors as you type"),
                    false,
                    false,
                ),
                (
                    "pConfused",
                    t("docR6.opt.confused", "Frequently confused words"),
                    false,
                    false,
                ),
                (
                    "pWith",
                    t("docR6.opt.grammarWith", "Check grammar with spelling"),
                    false,
                    false,
                ),
            ],
        );
        // ------------------------------------------------------------ Save
        intro(
            &mut f,
            "sIntro",
            "icon3",
            3,
            t("docR6.opt.saveIntro", "Customize how documents are saved."),
        );
        heading(
            &mut f,
            "sDocs",
            3,
            150.0,
            t("docR6.opt.saveDocs", "Save documents"),
        );
        f.add(
            "sFormatL",
            Some(3),
            [257.0, 204.0, 280.0, 24.0],
            Kind::Label(t("docR6.opt.saveFormat", "Save files in this format:")),
        );
        f.add(
            "sFormat",
            Some(3),
            [555.0, 198.0, 365.0, 30.0],
            Kind::Combo {
                items: vec![
                    t("docR6.type.docx", "Document (*.docx)"),
                    t("docR6.type.dotx", "Template (*.dotx)"),
                ],
                sel: usize::from(p.save_format.min(1)),
            },
        );
        f.add(
            "sAuto",
            Some(3),
            [257.0, 240.0, 320.0, 24.0],
            check(
                t(
                    "docR6.opt.autoRecover",
                    "Save AutoRecover information every",
                ),
                false,
            ),
        );
        f.enable("sAuto", false);
        f.add(
            "sMin",
            Some(3),
            [585.0, 234.0, 95.0, 33.0],
            Kind::Spin {
                value: 10,
                min: 1,
                max: 120,
            },
        );
        f.enable("sMin", false);
        f.add(
            "sMinL",
            Some(3),
            [699.0, 240.0, 100.0, 24.0],
            Kind::Label(t("docR6.opt.minutes", "minutes")),
        );
        f.enable("sMinL", false);
        f.add(
            "sFolderL",
            Some(3),
            [257.0, 320.0, 280.0, 24.0],
            Kind::Label(t("docR6.opt.defaultFolder", "Default local file location:")),
        );
        f.add(
            "sFolder",
            Some(3),
            [546.0, 314.0, 450.0, 35.0],
            Kind::Edit {
                value: p.folder.chars().collect(),
                caret: p.folder.chars().count(),
            },
        );
        // ------------------------------------------------------------ Language
        intro(
            &mut f,
            "lIntro",
            "icon4",
            4,
            t(
                "docR6.opt.languageIntro",
                "Set the BlueOffice Language Preferences.",
            ),
        );
        heading(
            &mut f,
            "lUi",
            4,
            130.0,
            t("docR6.opt.uiLanguage", "Display language"),
        );
        f.add(
            "lList",
            Some(4),
            [257.0, 176.0, 420.0, 120.0],
            Kind::List {
                items: vec![
                    t("docR6.opt.langEn", "English (United States)"),
                    t("docR6.opt.langVi", "Tiếng Việt (Vietnamese)"),
                ],
                sel: Some(usize::from(p.locale == "vi-VN")),
                top: 0,
            },
        );
        f.add(
            "lNote",
            Some(4),
            [257.0, 310.0, 700.0, 24.0],
            Kind::Note(t(
                "docR6.opt.languageNote",
                "The display language changes the next time BlueDoc starts.",
            )),
        );
        // ------------------------------------------------------------ Accessibility
        intro(
            &mut f,
            "aIntro",
            "icon5",
            5,
            t("docR6.opt.accessIntro", "Make BlueDoc more accessible."),
        );
        heading(
            &mut f,
            "aFeedback",
            5,
            130.0,
            t("docR6.opt.feedback", "Feedback options"),
        );
        checks(
            &mut f,
            5,
            257.0,
            176.0,
            &[
                (
                    "aSound",
                    t("docR6.opt.sound", "Provide feedback with sound"),
                    false,
                    false,
                ),
                (
                    "aAnim",
                    t("docR6.opt.animation", "Provide feedback with animation"),
                    true,
                    false,
                ),
            ],
        );
        // ------------------------------------------------------------ Advanced
        intro(
            &mut f,
            "vIntro",
            "icon6",
            6,
            t(
                "docR6.opt.advancedIntro",
                "Advanced options for working with BlueDoc.",
            ),
        );
        heading(
            &mut f,
            "vEdit",
            6,
            130.0,
            t("docR6.opt.editing", "Editing options"),
        );
        checks(
            &mut f,
            6,
            257.0,
            176.0,
            &[
                (
                    "vReplace",
                    t("docR6.opt.typingReplaces", "Typing replaces selected text"),
                    true,
                    false,
                ),
                (
                    "vWord",
                    t(
                        "docR6.opt.selectWord",
                        "When selecting, automatically select entire word",
                    ),
                    true,
                    false,
                ),
                (
                    "vDrag",
                    t("docR6.opt.dragDrop", "Allow text to be dragged and dropped"),
                    true,
                    false,
                ),
                (
                    "vCtrl",
                    t(
                        "docR6.opt.ctrlClick",
                        "Use CTRL + Click to follow hyperlink",
                    ),
                    true,
                    false,
                ),
            ],
        );
        heading(
            &mut f,
            "vDisplay",
            6,
            330.0,
            t("docR6.opt.displayAdv", "Display"),
        );
        f.add(
            "vRecentL",
            Some(6),
            [257.0, 382.0, 380.0, 24.0],
            Kind::Label(t(
                "docR6.opt.recentCount",
                "Show this number of Recent Documents:",
            )),
        );
        f.add(
            "vRecent",
            Some(6),
            [640.0, 376.0, 70.0, 33.0],
            Kind::Spin {
                value: i32::try_from(p.recent_count).unwrap_or(50),
                min: 0,
                max: 50,
            },
        );
        f.add(
            "vUnitsL",
            Some(6),
            [257.0, 426.0, 330.0, 24.0],
            Kind::Label(t("docR6.opt.units", "Show measurements in units of:")),
        );
        f.add(
            "vUnits",
            Some(6),
            [590.0, 420.0, 200.0, 30.0],
            Kind::Combo {
                items: vec![
                    t("docR6.opt.inches", "Inches"),
                    t("docR6.opt.cm", "Centimeters"),
                    t("docR6.opt.regional", "Regional setting"),
                ],
                sel: usize::from(p.units.min(2)),
            },
        );
        checks(
            &mut f,
            6,
            257.0,
            466.0,
            &[
                (
                    "vHScroll",
                    t("docR6.opt.hScroll", "Show horizontal scroll bar"),
                    true,
                    false,
                ),
                (
                    "vVScroll",
                    t("docR6.opt.vScroll", "Show vertical scroll bar"),
                    true,
                    false,
                ),
                (
                    "vVRuler",
                    t(
                        "docR6.opt.vRuler",
                        "Show vertical ruler in Print Layout view",
                    ),
                    true,
                    false,
                ),
            ],
        );
        // ------------------------------------------------------------ Quick Access Toolbar
        intro(
            &mut f,
            "qIntro",
            "icon8",
            8,
            t("docR6.opt.qatIntro", "Customize the Quick Access Toolbar."),
        );
        f.add(
            "qFromL",
            Some(8),
            [257.0, 130.0, 300.0, 24.0],
            Kind::Label(t("docR6.opt.chooseFrom", "Choose commands from:")),
        );
        let label_of = |id: &str| {
            QUICK_CHOICES
                .iter()
                .find(|(c, ..)| *c == id)
                .map_or_else(|| id.to_owned(), |(_, k, fb)| t(k, fb))
        };
        f.add(
            "qFrom",
            Some(8),
            [257.0, 166.0, 300.0, 540.0],
            Kind::List {
                items: QUICK_CHOICES.iter().map(|(id, ..)| label_of(id)).collect(),
                sel: Some(0),
                top: 0,
            },
        );
        f.add(
            "qAdd",
            Some(8),
            [575.0, 380.0, 110.0, 32.0],
            Kind::Button {
                label: t("docR6.opt.add", "Add >>"),
                default: false,
            },
        );
        f.add(
            "qRemove",
            Some(8),
            [575.0, 424.0, 110.0, 32.0],
            Kind::Button {
                label: t("docR6.opt.remove", "<< Remove"),
                default: false,
            },
        );
        f.add(
            "qToL",
            Some(8),
            [703.0, 130.0, 300.0, 24.0],
            Kind::Label(t(
                "docR6.opt.customizeQat",
                "Customize Quick Access Toolbar:",
            )),
        );
        f.add(
            "qTo",
            Some(8),
            [703.0, 166.0, 300.0, 540.0],
            Kind::List {
                items: p.quick.iter().map(|id| label_of(id)).collect(),
                sel: (!p.quick.is_empty()).then_some(0),
                top: 0,
            },
        );
        f.add(
            "qReset",
            Some(8),
            [703.0, 716.0, 110.0, 32.0],
            Kind::Button {
                label: t("docR6.opt.reset", "Reset"),
                default: false,
            },
        );
        // Round 20: Customize Ribbon, Add-ins and Trust Center (`doc_options20`).
        super::options20::add_pages(&mut f, s, p);
        r21::add_pages(&mut f, s);
        f.ok_cancel(&t("common.ok", "OK"), &t("common.cancel", "Cancel"));
        if let Some(c) = f.get_mut("ok") {
            c.rect = [790.0, 804.0, 115.0, 33.0];
        }
        if let Some(c) = f.get_mut("cancel") {
            c.rect = [919.0, 804.0, 115.0, 33.0];
        }
        f.set_page(0);
        let mut d = Self {
            form: f,
            page: 0,
            prefs: p.clone(),
            pointer: (0.0, 0.0),
            cats: CATS.iter().map(|(k, fb)| t(k, fb)).collect(),
            addins: super::options20::AddinTable::new(s),
            r21: r21::State::new(s, p),
        };
        r21::init(&mut d);
        d
    }

    /// A customisation file the host read (round 21, Import/Export ▾ › Import).
    pub(crate) fn r21_import(&mut self, text: &str) -> bool {
        r21::import(self, text)
    }

    pub(crate) fn layout(&mut self, m: &dyn TextMeasure, size: (f32, f32), scale: f32) {
        self.form.layout(m, size, scale);
        self.form.measure(m, &self.cats);
    }

    /// The category shown.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn page(&self) -> usize {
        self.page
    }

    /// Show category `i`.
    pub(crate) fn set_page(&mut self, i: usize) {
        self.page = i.min(CATS.len() - 1);
        self.form.set_page(self.page);
    }

    /// Form access for tests.
    #[cfg(test)]
    fn form_mut(&mut self) -> &mut Form {
        &mut self.form
    }

    pub(crate) fn input(&mut self, input: &Input) -> OptOut {
        if let Some(o) = r21::input(self, input) {
            return o;
        }
        let ev = match input {
            Input::Text(s) => self.form.type_text(s),
            Input::Key(Key::Escape, _) => Event::Cancel,
            Input::Key(k, m) => self.form.key(*k, *m),
            Input::Move { x, y } => {
                self.pointer = (*x, *y);
                return if self.form.hover(*x, *y) {
                    OptOut::Redraw
                } else {
                    OptOut::None
                };
            }
            Input::Press { x, y, count, .. } => {
                self.pointer = (*x, *y);
                self.form.press(*x, *y, *count)
            }
            Input::Wheel(d) => {
                let (x, y) = self.pointer;
                return if self.form.wheel(x, y, *d) {
                    OptOut::Redraw
                } else {
                    OptOut::None
                };
            }
            Input::Blink => return OptOut::Redraw,
            _ => return OptOut::None,
        };
        self.event(&ev)
    }

    fn event(&mut self, ev: &Event) -> OptOut {
        if let Some(o) = r21::event(self, ev) {
            return o;
        }
        match ev {
            Event::Cancel | Event::Clicked("cancel") => OptOut::Close,
            Event::Canvas("cats", _, y) => {
                let y = y + LIST[1];
                if let Some(i) = CAT_Y.iter().position(|c| (y - c).abs() <= 20.0) {
                    self.set_page(i);
                }
                OptOut::Redraw
            }
            Event::Clicked("qAdd") => {
                if let Some(i) = self.form.sel("qFrom") {
                    if let Some((id, ..)) = QUICK_CHOICES.get(i) {
                        if !self.prefs.quick.iter().any(|q| q == id) {
                            self.prefs.quick.push((*id).to_owned());
                        }
                    }
                }
                self.sync_quick();
                OptOut::Redraw
            }
            Event::Clicked("qRemove") => {
                if let Some(i) = self.form.sel("qTo") {
                    if i < self.prefs.quick.len() {
                        self.prefs.quick.remove(i);
                    }
                }
                self.sync_quick();
                OptOut::Redraw
            }
            Event::Clicked("qReset") => {
                self.prefs.quick = QUICK_DEFAULT.iter().map(|s| (*s).to_owned()).collect();
                self.sync_quick();
                OptOut::Redraw
            }
            Event::Clicked("pAutoB") => OptOut::AutoCorrect,
            Event::Clicked("aGo") => OptOut::R22(format!(
                "r22.addins:{}",
                self.form.sel("aManage").unwrap_or(0)
            )),
            Event::Clicked(id) if super::options20::handles(id) => {
                super::options20::clicked(&mut self.form, id);
                OptOut::Redraw
            }
            Event::Ok | Event::Clicked("ok") => OptOut::Apply(self.collect()),
            _ => OptOut::Redraw,
        }
    }

    fn sync_quick(&mut self) {
        let labels: Vec<String> = self
            .prefs
            .quick
            .iter()
            .map(|id| {
                QUICK_CHOICES
                    .iter()
                    .position(|(c, ..)| c == id)
                    .and_then(|i| self.form.get("qFrom").map(|c| (i, c)))
                    .and_then(|(i, c)| match &c.kind {
                        Kind::List { items, .. } => items.get(i).cloned(),
                        _ => None,
                    })
                    .unwrap_or_else(|| id.clone())
            })
            .collect();
        let sel = (!labels.is_empty()).then_some(labels.len() - 1);
        self.form.set_items("qTo", labels, sel);
    }

    /// The preferences the dialog shows.
    pub(crate) fn collect(&self) -> DocPrefs {
        let f = &self.form;
        let mut p = self.prefs.clone();
        p.screentips = f.sel("gTips").unwrap_or(0).min(2) as u8;
        p.user_name = f.text("gUser").trim().to_owned();
        p.initials = f.text("gInit").trim().to_owned();
        p.show_marks = f.on("dAll");
        for (i, id) in ["dTab", "dSpace", "dPara", "dHidden", "dHyphen", "dAnchor"]
            .into_iter()
            .enumerate()
        {
            p.mark_kinds[i] = f.on(id);
        }
        p.check_spelling = f.on("pSpell");
        p.save_format = f.sel("sFormat").unwrap_or(0).min(1) as u8;
        p.folder = f.text("sFolder").trim().to_owned();
        p.locale = if f.sel("lList") == Some(1) {
            "vi-VN".into()
        } else {
            "en-US".into()
        };
        p.units = f.sel("vUnits").unwrap_or(2).min(2) as u8;
        p.recent_count = u32::try_from(f.num("vRecent").clamp(0, 50)).unwrap_or(50);
        super::options20::collect(f, &mut p);
        r21::collect(self, &mut p);
        p
    }

    #[allow(dead_code)]
    pub(crate) fn paint(&self, p: &mut dyn Painter, window: (f32, f32), caret_on: bool) {
        self.paint_with(p, window, caret_on, None);
    }

    /// [`OptionsDialog::paint`] with the window's icons (round 21's command lists).
    pub(crate) fn paint_with(
        &self,
        p: &mut dyn Painter,
        window: (f32, f32),
        caret_on: bool,
        icons: Option<&crate::icons::Icons>,
    ) {
        // Device pixels per reference pixel.
        let k = self.form.scale_k();
        let base = self.form.font().size;
        let page = self.page;
        let cats = &self.cats;
        let addins = &self.addins;
        let st21 = &self.r21;
        let mut canvas = |id: &str, r: [f32; 4], p: &mut dyn Painter| match id {
            _ if r21::paint(st21, p, id, r, k, base, icons) => {}
            "bg" => p.fill_rect(r[0], r[1], r[2], r[3], Rgba(255, 255, 255, 255)),
            "cats" => paint_cats(p, r, k, base, page, cats),
            "mTab" | "mSpace" | "mPara" | "mHidden" | "mHyphen" | "mAnchor" => {
                paint_mark(p, id, r, k, base);
            }
            "aTable" => super::options20::paint_addins(p, r, k, base, addins),
            _ if id.starts_with("icon") => paint_page_icon(p, id, r, k),
            _ => {}
        };
        self.form.paint(p, window, caret_on, &mut canvas);
    }
}

/// The category list: framed #949494 on #F3F3F3, the chosen row #D2D2D2 framed 2 px black,
/// separators #D2D2D2 (measured on the reference).
fn paint_cats(p: &mut dyn Painter, r: [f32; 4], k: f32, base: f32, page: usize, cats: &[String]) {
    p.fill_rect(r[0], r[1], r[2], r[3], Rgba(0x94, 0x94, 0x94, 255));
    p.fill_rect(
        r[0] + 1.0,
        r[1] + 1.0,
        r[2] - 2.0,
        r[3] - 2.0,
        Rgba(0xF3, 0xF3, 0xF3, 255),
    );
    let font = Font {
        family: "Segoe UI",
        size: base.round(),
        weight: 400,
        italic: false,
        underline: false,
    };
    let line = font.size * 1.33;
    for sep in CAT_SEP {
        let y = r[1] + (sep - LIST[1]) * k;
        p.fill_rect(
            r[0] + 17.0 * k,
            y.round(),
            r[2] - 26.0 * k,
            1.0,
            Rgba(0xD2, 0xD2, 0xD2, 255),
        );
    }
    for (i, label) in cats.iter().enumerate() {
        let cy = r[1] + (CAT_Y[i] - LIST[1]) * k;
        if i == page {
            let (y0, h) = ((cy - 20.0 * k).round(), (41.0 * k).round());
            p.fill_rect(r[0] + 1.0, y0, r[2] - 2.0, h, Rgba(0, 0, 0, 255));
            let b = (2.0 * k).round();
            p.fill_rect(
                r[0] + 1.0 + b,
                y0 + b,
                r[2] - 2.0 - 2.0 * b,
                h - 2.0 * b,
                Rgba(0xD2, 0xD2, 0xD2, 255),
            );
        }
        p.text_font(
            label,
            r[0] + 17.0 * k,
            (cy - line / 2.0).round(),
            &font,
            Rgba(0x26, 0x26, 0x26, 255),
        );
    }
}

/// The mark shown after each formatting-mark option (own drawings of the marks themselves).
fn paint_mark(p: &mut dyn Painter, id: &str, r: [f32; 4], k: f32, base: f32) {
    let ink = Rgba(0x40, 0x40, 0x40, 255);
    let (x, cy) = (r[0] + 8.0 * k, r[1] + r[3] / 2.0);
    let lw = k.max(1.0);
    match id {
        "mTab" => p.path(
            &[
                PathSeg::M(x, cy),
                PathSeg::L(x + 16.0 * k, cy),
                PathSeg::M(x + 12.0 * k, cy - 4.0 * k),
                PathSeg::L(x + 16.0 * k, cy),
                PathSeg::L(x + 12.0 * k, cy + 4.0 * k),
            ],
            None,
            Some((ink, lw)),
        ),
        "mSpace" => {
            for i in 0..3 {
                p.fill_rect(
                    (x + 2.0 * k + 6.0 * k * i as f32).round(),
                    (cy - lw / 2.0).round(),
                    (2.0 * k).round().max(2.0),
                    (2.0 * k).round().max(2.0),
                    ink,
                );
            }
        }
        "mPara" => {
            let f = Font {
                family: "Segoe UI",
                size: (base * 1.3).round(),
                weight: 400,
                italic: false,
                underline: false,
            };
            p.text_font("¶", x + 2.0 * k, cy - f.size * 0.66, &f, ink);
        }
        "mHidden" => {
            let f = Font {
                family: "Segoe UI",
                size: (base * 0.9).round(),
                weight: 400,
                italic: false,
                underline: false,
            };
            p.text_font("abc", x, cy - f.size * 0.7, &f, ink);
            let mut xx = x;
            while xx < x + 22.0 * k {
                p.fill_rect(xx.round(), (cy + 6.0 * k).round(), lw, lw, ink);
                xx += 3.0 * k;
            }
        }
        "mHyphen" => p.path(
            &[
                PathSeg::M(x + 2.0 * k, cy - 2.0 * k),
                PathSeg::L(x + 14.0 * k, cy - 2.0 * k),
                PathSeg::L(x + 14.0 * k, cy + 5.0 * k),
            ],
            None,
            Some((ink, lw)),
        ),
        _ => {
            // Anchor: a ring, a shank, a crossbar and a curved arm.
            let (ax, top) = (x + 8.0 * k, cy - 8.0 * k);
            p.path(
                &[
                    PathSeg::M(ax + 2.5 * k, top + 2.5 * k),
                    PathSeg::C(
                        ax + 2.5 * k,
                        top + 4.0 * k,
                        ax - 2.5 * k,
                        top + 4.0 * k,
                        ax - 2.5 * k,
                        top + 2.5 * k,
                    ),
                    PathSeg::C(
                        ax - 2.5 * k,
                        top + 1.0 * k,
                        ax + 2.5 * k,
                        top + 1.0 * k,
                        ax + 2.5 * k,
                        top + 2.5 * k,
                    ),
                    PathSeg::M(ax, top + 5.0 * k),
                    PathSeg::L(ax, top + 16.0 * k),
                    PathSeg::M(ax - 4.0 * k, top + 8.0 * k),
                    PathSeg::L(ax + 4.0 * k, top + 8.0 * k),
                    PathSeg::M(ax - 7.0 * k, top + 11.0 * k),
                    PathSeg::C(
                        ax - 6.0 * k,
                        top + 16.0 * k,
                        ax + 6.0 * k,
                        top + 16.0 * k,
                        ax + 7.0 * k,
                        top + 11.0 * k,
                    ),
                ],
                None,
                Some((ink, lw * 1.5)),
            );
        }
    }
}

/// Our own page icons left of each page's intro line: a sheet with a mark for the category
/// (sliders, magnifier, check, disk, globe, person, list, star).
fn paint_page_icon(p: &mut dyn Painter, id: &str, r: [f32; 4], k: f32) {
    let ink = Rgba(0x55, 0x55, 0x55, 255);
    let blue = Rgba(0x1E, 0x6F, 0xC6, 255);
    let (x, y) = (r[0] + 4.0 * k, r[1] + 4.0 * k);
    let (w, h) = (24.0 * k, 30.0 * k);
    p.fill_rect(x, y, w, h, ink);
    p.fill_rect(x + 1.0, y + 1.0, w - 2.0, h - 2.0, Rgba(255, 255, 255, 255));
    let lw = k.max(1.0);
    match id {
        "icon0" | "icon6" => {
            for (i, kx) in [(0.0, 0.35), (1.0, 0.65), (2.0, 0.45)] {
                let ly = y + (8.0 + 7.0 * i) * k;
                p.fill_rect(x + 4.0 * k, ly, w - 8.0 * k, lw, ink);
                p.fill_rect(
                    x + 4.0 * k + (w - 10.0 * k) * kx,
                    ly - 2.0 * k,
                    3.0 * k,
                    5.0 * k,
                    blue,
                );
            }
        }
        "icon1" => {
            let (cx, cy, rr) = (x + w - 2.0 * k, y + h - 6.0 * k, 6.0 * k);
            let c = rr * 0.552;
            p.path(
                &[
                    PathSeg::M(cx + rr, cy),
                    PathSeg::C(cx + rr, cy + c, cx + c, cy + rr, cx, cy + rr),
                    PathSeg::C(cx - c, cy + rr, cx - rr, cy + c, cx - rr, cy),
                    PathSeg::C(cx - rr, cy - c, cx - c, cy - rr, cx, cy - rr),
                    PathSeg::C(cx + c, cy - rr, cx + rr, cy - c, cx + rr, cy),
                    PathSeg::M(cx + 4.0 * k, cy + 4.0 * k),
                    PathSeg::L(cx + 9.0 * k, cy + 9.0 * k),
                ],
                Some(Rgba(255, 255, 255, 255)),
                Some((ink, lw * 1.5)),
            );
        }
        "icon2" => p.path(
            &[
                PathSeg::M(x + 5.0 * k, y + 16.0 * k),
                PathSeg::L(x + 10.0 * k, y + 22.0 * k),
                PathSeg::L(x + 20.0 * k, y + 8.0 * k),
            ],
            None,
            Some((blue, lw * 2.0)),
        ),
        "icon3" => {
            p.fill_rect(x + 4.0 * k, y + 6.0 * k, w - 8.0 * k, h - 12.0 * k, blue);
            p.fill_rect(
                x + 8.0 * k,
                y + 6.0 * k,
                w - 16.0 * k,
                6.0 * k,
                Rgba(255, 255, 255, 255),
            );
        }
        _ => {
            for i in 0..4 {
                p.fill_rect(
                    x + 5.0 * k,
                    y + (7.0 + 5.0 * i as f32) * k,
                    w - 10.0 * k,
                    lw,
                    Rgba(0x9a, 0x9a, 0x9a, 255),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefs_round_trip() {
        let p = DocPrefs {
            screentips: 1,
            user_name: "An Nguyen".into(),
            initials: String::new(),
            show_marks: true,
            mark_kinds: [true, false, true, false, false, true],
            check_spelling: false,
            save_format: 1,
            folder: "D:\\Docs".into(),
            locale: "vi-VN".into(),
            units: 1,
            recent_count: 7,
            quick: vec!["file.save".into(), "file.print".into()],
            extra: super::super::options20::Extra::default(),
        };
        assert_eq!(DocPrefs::from_text(&p.to_text()), p);
        assert_eq!(p.initials_or_default(), "AN");
        // Unknown commands and keys are dropped, bad numbers fall back.
        let q = DocPrefs::from_text("quick=file.save,evil.cmd\nunits=x\nzzz=1");
        assert_eq!(q.quick, vec!["file.save".to_owned()]);
        assert_eq!(q.units, 2);
    }

    #[test]
    fn options_collect_and_quick_access() {
        let s = crate::strings::Strings::load("en-US");
        let mut d = OptionsDialog::open(&s, &DocPrefs::default());
        d.set_page(8);
        assert_eq!(d.page(), 8);
        // Round 21: the page's own command list (`doc_options21`).
        r21::pick_quick(&mut d, "file.print");
        assert_eq!(d.event(&Event::Clicked("qAdd")), OptOut::Redraw);
        d.form_mut().set_on("dAll", true);
        d.form_mut().set_text("gUser", "Binh Tran");
        d.form_mut().set_sel("vUnits", Some(1));
        let OptOut::Apply(p) = d.event(&Event::Ok) else {
            panic!("ok applies");
        };
        assert_eq!(p.quick.last().map(String::as_str), Some("file.print"));
        assert!(p.show_marks);
        assert_eq!(p.user_name, "Binh Tran");
        assert_eq!(p.units, 1);
        assert_eq!(d.event(&Event::Cancel), OptOut::Close);
    }
}
