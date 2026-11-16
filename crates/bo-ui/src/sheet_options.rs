//! File › Options of BlueSheet (round 39): the reference's Options dialog — a category list
//! on the left (General, Formulas, Data, Proofing, Save, Language, Accessibility, Advanced,
//! Customize Ribbon, Quick Access Toolbar, Add-ins, Trust Center) and the page of the chosen
//! category on a white face. What BlueSheet supports is live; the rest is greyed:
//!
//! * General: default font and size of new workbooks, sheets in a new workbook;
//! * Formulas: workbook calculation mode, iterative calculation (maximum iterations / change;
//!   saved in the workbook's §18.2.2 `calcPr`), R1C1 reference style;
//! * Save: AutoRecover interval;
//! * Advanced: after pressing Enter, move selection (direction).
//!
//! Application settings are kept as `key=value` lines ([`SheetPrefs`]) the host stores in
//! `%LOCALAPPDATA%\BlueOffice\sheet-prefs.txt`.
//!
//! Measured on the reference at 125 % (`show/sheet-28`): window 1050 × 856 px, white face,
//! category list x 16–217 framed #949494 on #F3F3F3, rows 41 px, the chosen row #D2D2D2
//! framed 2 px black, separators #D2D2D2 under Advanced and Quick Access Toolbar, section
//! headings bold over a #E6E6E6 rule.

use super::form_impl::{Event, Form, FormKind, Kind};
use super::SheetWindow;
use crate::{Font, Painter};
use bo_render::Rgba;
use serde_json::{json, Value};

/// Application preferences of BlueSheet.
#[derive(Debug, Clone, PartialEq)]
pub struct SheetPrefs {
    /// After pressing Enter, move selection.
    pub enter_move: bool,
    /// Direction: 0 down, 1 right, 2 up, 3 left.
    pub enter_dir: u8,
    /// Default font of new workbooks (`""`: the theme's body font).
    pub font: String,
    /// Default font size of new workbooks (points).
    pub size: f64,
    /// Sheets in a new workbook.
    pub sheets: u32,
    /// R1C1 reference style.
    pub r1c1: bool,
    /// Save AutoRecover information.
    pub autosave: bool,
    /// AutoRecover interval (minutes).
    pub autosave_min: u32,
}

impl Default for SheetPrefs {
    fn default() -> Self {
        Self {
            enter_move: true,
            enter_dir: 0,
            font: String::new(),
            size: 11.0,
            sheets: 1,
            r1c1: false,
            autosave: true,
            autosave_min: 10,
        }
    }
}

impl SheetPrefs {
    /// Read `key=value` lines (unknown keys and bad values keep their defaults).
    #[must_use]
    pub fn from_text(text: &str) -> Self {
        let mut p = Self::default();
        for line in text.lines() {
            let Some((k, v)) = line.split_once('=') else {
                continue;
            };
            let v = v.trim();
            match k.trim() {
                "enterMove" => p.enter_move = v == "1",
                "enterDir" => p.enter_dir = v.parse().unwrap_or(0).min(3),
                "font" => v.clone_into(&mut p.font),
                "size" => p.size = v.parse().unwrap_or(11.0_f64).clamp(1.0, 409.0),
                "sheets" => p.sheets = v.parse().unwrap_or(1).clamp(1, 255),
                "r1c1" => p.r1c1 = v == "1",
                "autosave" => p.autosave = v == "1",
                "autosaveMin" => p.autosave_min = v.parse().unwrap_or(10).clamp(1, 120),
                _ => {}
            }
        }
        p
    }

    /// The preferences as `key=value` lines.
    #[must_use]
    pub fn to_text(&self) -> String {
        let b = |v: bool| if v { "1" } else { "0" };
        format!(
            "enterMove={}\nenterDir={}\nfont={}\nsize={}\nsheets={}\nr1c1={}\nautosave={}\nautosaveMin={}\n",
            b(self.enter_move),
            self.enter_dir,
            self.font,
            self.size,
            self.sheets,
            b(self.r1c1),
            b(self.autosave),
            self.autosave_min,
        )
    }

    /// Where Enter moves the active cell: (rows, columns), `None` to stay.
    #[must_use]
    pub fn enter_step(&self) -> Option<(i64, i64)> {
        self.enter_move.then_some(match self.enter_dir {
            1 => (0, 1),
            2 => (-1, 0),
            3 => (0, -1),
            _ => (1, 0),
        })
    }
}

/// Categories: (label key, fallback, page supported).
const CATS: [(&str, &str, bool); 12] = [
    ("sheetNative.optGeneral", "General", true),
    ("sheetNative.optFormulas", "Formulas", true),
    ("sheetNative.optData", "Data", false),
    ("sheetNative.optProofing", "Proofing", false),
    ("sheetNative.optSave", "Save", true),
    ("sheetNative.optLanguage", "Language", false),
    ("sheetNative.optAccessibility", "Accessibility", false),
    ("sheetNative.optAdvanced", "Advanced", true),
    ("sheetNative.optRibbon", "Customize Ribbon", false),
    ("sheetNative.optQat", "Quick Access Toolbar", false),
    ("sheetNative.optAddins", "Add-ins", false),
    ("sheetNative.optTrust", "Trust Center", false),
];
/// Centre of each category row (reference pixels from the dialog's top).
const CAT_Y: [f32; 12] = [
    68.0, 109.0, 150.0, 191.0, 232.0, 273.0, 314.0, 360.0, 409.0, 455.0, 504.0, 545.0,
];
/// Separators between category groups.
const CAT_SEP: [f32; 2] = [383.0, 478.0];
/// The category list (reference pixels).
const LIST: [f32; 4] = [16.0, 48.0, 202.0, 750.0];
/// Fonts offered for new workbooks (the first is the theme's body font).
const FONTS: [&str; 10] = [
    "",
    "Arial",
    "Calibri",
    "Cambria",
    "Consolas",
    "Courier New",
    "Segoe UI",
    "Tahoma",
    "Times New Roman",
    "Verdana",
];
const SIZES: [f64; 16] = [
    8.0, 9.0, 10.0, 11.0, 12.0, 14.0, 16.0, 18.0, 20.0, 22.0, 24.0, 26.0, 28.0, 36.0, 48.0, 72.0,
];

impl SheetWindow {
    /// File › Options.
    pub(super) fn open_options(&mut self) -> bool {
        let calc = self
            .view
            .query(json!({ "cmd": "calcOptions" }))
            .and_then(|v| v.get("options").cloned())
            .unwrap_or(Value::Null);
        let t = |k: &str, f: &str| self.t(k, f);
        let p = self.prefs.clone();
        let mode = calc.get("mode").and_then(Value::as_str).unwrap_or("auto");
        let iterate = calc
            .get("iterate")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let count = calc
            .get("iterateCount")
            .and_then(Value::as_u64)
            .unwrap_or(100);
        let delta = calc
            .get("iterateDelta")
            .and_then(Value::as_f64)
            .unwrap_or(0.001);
        let mut f = Form::new(
            &t("sheetNative.optTitle", "BlueSheet Options"),
            (1040.0, 848.0),
            vec![],
        );
        // White face and the category list, drawn by the owner.
        f.add("bg", None, [1.0, 38.0, 1038.0, 809.0], Kind::Canvas);
        f.add("cats", None, LIST, Kind::Canvas);
        let check = |label: String, on: bool| Kind::Check { label, on };
        let label = Kind::Label;
        let heading = |f: &mut Form, id: &'static str, page: usize, y: f32, text: String| {
            f.add(id, Some(page), [241.0, y, 755.0, 0.0], Kind::Group(text));
        };
        let intro = |f: &mut Form, id: &'static str, page: usize, text: String| {
            f.add(
                id,
                Some(page),
                [305.0, 70.0, 640.0, 24.0],
                Kind::Label(text),
            );
        };
        // General.
        intro(
            &mut f,
            "gIntro",
            0,
            t(
                "sheetNative.optGeneralIntro",
                "General options for working with BlueSheet.",
            ),
        );
        heading(
            &mut f,
            "gUi",
            0,
            130.0,
            t("sheetNative.optUi", "User Interface options"),
        );
        for (i, (id, key, fb, on)) in [
            (
                "gMini",
                "sheetNative.optMini",
                "Show Mini Toolbar on selection",
                true,
            ),
            (
                "gQuick",
                "sheetNative.optQuick",
                "Show Quick Analysis options on selection",
                true,
            ),
            ("gLive", "sheetNative.optLive", "Enable Live Preview", true),
            (
                "gCollapse",
                "sheetNative.optCollapse",
                "Collapse the ribbon automatically",
                false,
            ),
        ]
        .into_iter()
        .enumerate()
        {
            f.add(
                id,
                Some(0),
                [257.0, 170.0 + 36.0 * i as f32, 480.0, 24.0],
                check(t(key, fb), on),
            );
            f.enable(id, false);
        }
        heading(
            &mut f,
            "gNew",
            0,
            472.0,
            t("sheetNative.optNewBooks", "When creating new workbooks"),
        );
        let font_items: Vec<String> = FONTS
            .iter()
            .map(|n| {
                if n.is_empty() {
                    t("sheetNative.optBodyFont", "Body Font")
                } else {
                    (*n).to_owned()
                }
            })
            .collect();
        let font_sel = FONTS.iter().position(|n| *n == p.font).unwrap_or(0);
        let size_sel = SIZES
            .iter()
            .position(|s| (s - p.size).abs() < 0.01)
            .unwrap_or(3);
        let rows: [(&'static str, &str, &str, f32); 4] = [
            (
                "gFontL",
                "sheetNative.optFont",
                "Use this as the default font:",
                525.0,
            ),
            ("gSizeL", "sheetNative.optSize", "Font size:", 566.0),
            (
                "gViewL",
                "sheetNative.optView",
                "Default view for new sheets:",
                605.0,
            ),
            (
                "gSheetsL",
                "sheetNative.optSheets",
                "Include this many sheets:",
                644.0,
            ),
        ];
        for (id, key, fb, y) in rows {
            f.add(id, Some(0), [257.0, y, 230.0, 24.0], label(t(key, fb)));
        }
        f.add(
            "gFont",
            Some(0),
            [492.0, 518.0, 310.0, 30.0],
            Kind::Combo {
                items: font_items,
                sel: font_sel,
            },
        );
        f.add(
            "gSize",
            Some(0),
            [492.0, 558.0, 58.0, 30.0],
            Kind::Combo {
                items: SIZES.iter().map(|s| format!("{s}")).collect(),
                sel: size_sel,
            },
        );
        f.add(
            "gView",
            Some(0),
            [492.0, 598.0, 186.0, 30.0],
            Kind::Combo {
                items: vec![
                    t("sheetNative.optNormalView", "Normal View"),
                    t("sheetNative.optPageBreak", "Page Break Preview"),
                    t("sheetNative.optPageLayout", "Page Layout View"),
                ],
                sel: 0,
            },
        );
        f.enable("gView", false);
        f.add(
            "gSheets",
            Some(0),
            [492.0, 637.0, 98.0, 30.0],
            Kind::Spin {
                value: p.sheets as i32,
                min: 1,
                max: 255,
            },
        );
        // Formulas.
        intro(
            &mut f,
            "fIntro",
            1,
            t(
                "sheetNative.optFormulasIntro",
                "Change options related to formula calculation, performance, and error handling.",
            ),
        );
        heading(
            &mut f,
            "fCalc",
            1,
            130.0,
            t("sheetNative.optCalc", "Calculation options"),
        );
        f.add(
            "fModeL",
            Some(1),
            [257.0, 165.0, 260.0, 24.0],
            label(t("sheetNative.optCalcMode", "Workbook Calculation")),
        );
        for (i, (id, key, fb, m)) in [
            ("fAuto", "sheetNative.optAuto", "Automatic", "auto"),
            (
                "fAutoNoTable",
                "sheetNative.optAutoNoTable",
                "Automatic except for data tables",
                "autoNoTable",
            ),
            ("fManual", "sheetNative.optManual", "Manual", "manual"),
        ]
        .into_iter()
        .enumerate()
        {
            f.add(
                id,
                Some(1),
                [273.0, 197.0 + 32.0 * i as f32, 300.0, 24.0],
                check(t(key, fb), mode == m),
            );
        }
        f.add(
            "fIterate",
            Some(1),
            [600.0, 165.0, 360.0, 24.0],
            check(
                t("sheetNative.optIterate", "Enable iterative calculation"),
                iterate,
            ),
        );
        f.add(
            "fCountL",
            Some(1),
            [624.0, 202.0, 170.0, 24.0],
            label(t("sheetNative.optMaxIter", "Maximum Iterations:")),
        );
        f.add(
            "fCount",
            Some(1),
            [800.0, 197.0, 90.0, 28.0],
            Kind::Spin {
                value: count.min(32767) as i32,
                min: 1,
                max: 32767,
            },
        );
        f.add(
            "fDeltaL",
            Some(1),
            [624.0, 236.0, 170.0, 24.0],
            label(t("sheetNative.optMaxChange", "Maximum Change:")),
        );
        f.add(
            "fDelta",
            Some(1),
            [800.0, 231.0, 90.0, 28.0],
            Kind::Edit {
                value: format!("{delta}").chars().collect(),
                caret: 0,
            },
        );
        heading(
            &mut f,
            "fWork",
            1,
            300.0,
            t("sheetNative.optWorking", "Working with formulas"),
        );
        f.add(
            "fR1c1",
            Some(1),
            [257.0, 336.0, 400.0, 24.0],
            check(t("sheetNative.optR1c1", "R1C1 reference style"), p.r1c1),
        );
        for (i, (id, key, fb)) in [
            (
                "fAutoComplete",
                "sheetNative.optAutoComplete",
                "Formula AutoComplete",
            ),
            (
                "fTableNames",
                "sheetNative.optTableNames",
                "Use table names in formulas",
            ),
        ]
        .into_iter()
        .enumerate()
        {
            f.add(
                id,
                Some(1),
                [257.0, 372.0 + 36.0 * i as f32, 400.0, 24.0],
                check(t(key, fb), true),
            );
            f.enable(id, false);
        }
        heading(
            &mut f,
            "fErr",
            1,
            470.0,
            t("sheetNative.optErrors", "Error Checking"),
        );
        f.add(
            "fBackground",
            Some(1),
            [257.0, 506.0, 420.0, 24.0],
            check(
                t(
                    "sheetNative.optBackground",
                    "Enable background error checking",
                ),
                true,
            ),
        );
        f.enable("fBackground", false);
        // Save.
        intro(
            &mut f,
            "sIntro",
            4,
            t(
                "sheetNative.optSaveIntro",
                "Customize how workbooks are saved.",
            ),
        );
        heading(
            &mut f,
            "sBooks",
            4,
            130.0,
            t("sheetNative.optSaveBooks", "Save workbooks"),
        );
        f.add(
            "sFormatL",
            Some(4),
            [257.0, 170.0, 230.0, 24.0],
            label(t("sheetNative.optSaveFormat", "Save files in this format:")),
        );
        f.add(
            "sFormat",
            Some(4),
            [492.0, 164.0, 310.0, 30.0],
            Kind::Combo {
                items: vec![t("sheetNative.optXlsx", "Workbook (*.xlsx)")],
                sel: 0,
            },
        );
        f.enable("sFormat", false);
        f.add(
            "sAuto",
            Some(4),
            [257.0, 210.0, 330.0, 24.0],
            check(
                t(
                    "sheetNative.optAutoRecover",
                    "Save AutoRecover information every",
                ),
                p.autosave,
            ),
        );
        f.add(
            "sMinutes",
            Some(4),
            [592.0, 206.0, 70.0, 28.0],
            Kind::Spin {
                value: p.autosave_min as i32,
                min: 1,
                max: 120,
            },
        );
        f.add(
            "sMinutesL",
            Some(4),
            [670.0, 210.0, 100.0, 24.0],
            label(t("sheetNative.optMinutes", "minutes")),
        );
        f.add(
            "sDirL",
            Some(4),
            [257.0, 250.0, 230.0, 24.0],
            label(t(
                "sheetNative.optAutoRecoverDir",
                "AutoRecover file location:",
            )),
        );
        f.add(
            "sDir",
            Some(4),
            [492.0, 244.0, 440.0, 30.0],
            Kind::Edit {
                value: "%LOCALAPPDATA%\\BlueOffice\\AutoRecover".chars().collect(),
                caret: 0,
            },
        );
        f.enable("sDir", false);
        // Advanced.
        intro(
            &mut f,
            "aIntro",
            7,
            t(
                "sheetNative.optAdvancedIntro",
                "Advanced options for working with BlueSheet.",
            ),
        );
        heading(
            &mut f,
            "aEdit",
            7,
            130.0,
            t("sheetNative.optEditing", "Editing options"),
        );
        f.add(
            "aEnter",
            Some(7),
            [257.0, 166.0, 400.0, 24.0],
            check(
                t(
                    "sheetNative.optEnterMove",
                    "After pressing Enter, move selection",
                ),
                p.enter_move,
            ),
        );
        f.add(
            "aDirL",
            Some(7),
            [281.0, 202.0, 100.0, 24.0],
            label(t("sheetNative.optDirection", "Direction:")),
        );
        f.add(
            "aDir",
            Some(7),
            [380.0, 197.0, 120.0, 30.0],
            Kind::Combo {
                items: vec![
                    t("sheetNative.optDown", "Down"),
                    t("sheetNative.optRight", "Right"),
                    t("sheetNative.optUp", "Up"),
                    t("sheetNative.optLeft", "Left"),
                ],
                sel: usize::from(p.enter_dir.min(3)),
            },
        );
        for (i, (id, key, fb, on)) in [
            (
                "aDecimal",
                "sheetNative.optDecimal",
                "Automatically insert a decimal point",
                false,
            ),
            (
                "aFill",
                "sheetNative.optFillHandle",
                "Enable fill handle and cell drag-and-drop",
                true,
            ),
            (
                "aInCell",
                "sheetNative.optInCell",
                "Allow editing directly in cells",
                true,
            ),
        ]
        .into_iter()
        .enumerate()
        {
            f.add(
                id,
                Some(7),
                [257.0, 238.0 + 36.0 * i as f32, 440.0, 24.0],
                check(t(key, fb), on),
            );
            f.enable(id, false);
        }
        // Categories without settings here.
        for page in [2usize, 3, 5, 6, 8, 9, 10, 11] {
            f.add(
                "na",
                Some(page),
                [257.0, 70.0, 700.0, 24.0],
                Kind::Note(t(
                    "sheetNative.optNotYet",
                    "These options are not available in BlueSheet yet.",
                )),
            );
        }
        f.ok_cancel(&t("common.ok", "OK"), &t("common.cancel", "Cancel"));
        if let Some(c) = f.get_mut("ok") {
            c.rect = [790.0, 804.0, 115.0, 33.0];
        }
        if let Some(c) = f.get_mut("cancel") {
            c.rect = [919.0, 804.0, 115.0, 33.0];
        }
        f.set_page(0);
        self.options_page = 0;
        self.form = Some(f);
        self.form_kind = FormKind::Options;
        self.sync_options_form();
        true
    }

    /// Dim what depends on an unchecked box.
    fn sync_options_form(&mut self) {
        let Some(f) = self.form.as_mut() else {
            return;
        };
        let it = f.on("fIterate");
        f.enable("fCount", it);
        f.enable("fDelta", it);
        f.enable("fCountL", it);
        f.enable("fDeltaL", it);
        let enter = f.on("aEnter");
        f.enable("aDir", enter);
        f.enable("aDirL", enter);
        let auto = f.on("sAuto");
        f.enable("sMinutes", auto);
    }

    /// Events of the Options dialog.
    pub(super) fn options_event(&mut self, ev: &Event) {
        match ev {
            Event::Cancel | Event::Clicked("cancel") => self.form = None,
            Event::Canvas("cats", _, y) => {
                // `y` is local to the list.
                let y = y + LIST[1];
                if let Some(i) = CAT_Y.iter().position(|c| (y - c).abs() <= 20.0) {
                    self.options_page = i;
                    if let Some(f) = self.form.as_mut() {
                        f.set_page(i);
                    }
                }
            }
            Event::Changed(id @ ("fAuto" | "fAutoNoTable" | "fManual")) => {
                if let Some(f) = self.form.as_mut() {
                    for o in ["fAuto", "fAutoNoTable", "fManual"] {
                        f.set_on(o, o == *id);
                    }
                }
            }
            Event::Changed(_) => self.sync_options_form(),
            Event::Ok | Event::Clicked("ok") => self.options_ok(),
            _ => {}
        }
    }

    fn options_ok(&mut self) {
        let Some(f) = self.form.take() else {
            return;
        };
        let mut p = self.prefs.clone();
        p.font = FONTS[f.sel("gFont").unwrap_or(0).min(FONTS.len() - 1)].to_owned();
        p.size = SIZES[f.sel("gSize").unwrap_or(3).min(SIZES.len() - 1)];
        p.sheets = f.num("gSheets").clamp(1, 255) as u32;
        p.r1c1 = f.on("fR1c1");
        p.autosave = f.on("sAuto");
        p.autosave_min = f.num("sMinutes").clamp(1, 120) as u32;
        p.enter_move = f.on("aEnter");
        p.enter_dir = f.sel("aDir").unwrap_or(0).min(3) as u8;
        // Calculation settings belong to the workbook (§18.2.2 calcPr): one undo step when
        // they change.
        let mode = if f.on("fManual") {
            "manual"
        } else if f.on("fAutoNoTable") {
            "autoNoTable"
        } else {
            "auto"
        };
        let iterate = f.on("fIterate");
        let count = f.num("fCount").clamp(1, 32767);
        let delta = f
            .text("fDelta")
            .trim()
            .replace(',', ".")
            .parse::<f64>()
            .ok()
            .filter(|d| *d >= 0.0);
        if let Some(mut opts) = self
            .view
            .query(json!({ "cmd": "calcOptions" }))
            .and_then(|v| v.get("options").cloned())
        {
            let before = opts.clone();
            opts["mode"] = json!(mode);
            opts["iterate"] = json!(iterate);
            opts["iterateCount"] = json!(count);
            if let Some(d) = delta {
                opts["iterateDelta"] = json!(d);
            }
            if opts != before
                && self
                    .view
                    .query(json!({ "cmd": "setCalcOptions", "options": opts }))
                    .is_some()
            {
                self.view.changed();
            }
        }
        self.set_prefs(p);
    }

    /// Apply preferences to the window and mark them for the host to store.
    pub(super) fn set_prefs(&mut self, p: SheetPrefs) {
        self.view.set_enter_move(p.enter_step());
        self.view.set_r1c1(p.r1c1);
        if p != self.prefs {
            self.prefs_out = Some(p.to_text());
        }
        self.prefs = p;
    }

    /// Replace the (untouched) new workbook by one made with the preferences: default font
    /// and size in the Normal style's font (§18.8.22 `font` 0, the theme's body font when
    /// none is chosen) and the number of sheets. `false` when the defaults apply.
    pub(super) fn new_book_with_prefs(&mut self, name: &str) -> bool {
        let p = self.prefs.clone();
        if p.font.is_empty() && (p.size - 11.0).abs() < 0.01 && p.sheets <= 1 {
            return false;
        }
        let mut wb = bo_model::sheet::Workbook::new();
        if wb.sheets.is_empty() {
            wb.sheets.push(bo_model::sheet::Worksheet::new("Sheet1"));
        }
        for i in wb.sheets.len()..p.sheets as usize {
            wb.sheets
                .push(bo_model::sheet::Worksheet::new(&format!("Sheet{}", i + 1)));
        }
        if let Some(f) = wb.styles.fonts.get_mut(0) {
            if !p.font.is_empty() {
                f.name = Some(p.font.clone());
                // A named font is no longer the theme's (§18.8.35 scheme).
                f.scheme = None;
            }
            f.size = Some(p.size);
        }
        if self.view.engine_mut().load(wb, name).is_err() {
            return false;
        }
        self.view.reload();
        true
    }

    /// The white face and the category list.
    pub(super) fn paint_options(&self, id: &str, r: [f32; 4], p: &mut dyn Painter) {
        let k = self.scale / 1.25;
        match id {
            "bg" => p.fill_rect(r[0], r[1], r[2], r[3], Rgba(255, 255, 255, 255)),
            "cats" => {
                p.fill_rect(r[0], r[1], r[2], r[3], Rgba(0x94, 0x94, 0x94, 255));
                p.fill_rect(
                    r[0] + 1.0,
                    r[1] + 1.0,
                    r[2] - 2.0,
                    r[3] - 2.0,
                    Rgba(0xF3, 0xF3, 0xF3, 255),
                );
                let base = self.form.as_ref().map_or(15.0, |f| f.font().size);
                // The reference's category list is in a larger size than the page.
                let font = Font {
                    size: (base * 1.2).round(),
                    ..crate::Font {
                        family: "Segoe UI",
                        size: base,
                        weight: 400,
                        italic: false,
                        underline: false,
                    }
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
                for (i, (key, fb, _)) in CATS.iter().enumerate() {
                    let cy = r[1] + (CAT_Y[i] - LIST[1]) * k;
                    if i == self.options_page {
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
                    let ink = Rgba(0x26, 0x26, 0x26, 255);
                    p.text_font(
                        &self.t(key, fb),
                        r[0] + 17.0 * k,
                        (cy - line / 2.0).round(),
                        &font,
                        ink,
                    );
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::SheetPrefs;

    #[test]
    fn prefs_round_trip_and_enter_step() {
        let p = SheetPrefs {
            enter_dir: 1,
            font: "Arial".into(),
            size: 12.0,
            r1c1: true,
            autosave_min: 5,
            ..SheetPrefs::default()
        };
        let back = SheetPrefs::from_text(&p.to_text());
        assert_eq!(back, p);
        assert_eq!(back.enter_step(), Some((0, 1)));
        let off = SheetPrefs {
            enter_move: false,
            ..SheetPrefs::default()
        };
        assert_eq!(off.enter_step(), None);
        assert_eq!(
            SheetPrefs::from_text("size=abc\nzzz"),
            SheetPrefs::default()
        );
    }
}
