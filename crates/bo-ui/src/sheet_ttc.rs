//! Data › Text to Columns as the reference's three-step wizard (round 45, measured at
//! 125 % on the reference's Convert Text to Columns Wizard: 771 × 546 px, `show/sheet-40`):
//! step 1 the original data type (the engine's `textToColumnsGuess` chooses), step 2 the
//! delimiters or the fixed-width column breaks on a character ruler (click to add, drag to
//! move, double-click to delete), step 3 each column's data format, the destination and the
//! Advanced number settings. Finish runs the engine's `textToColumns` (one undo step).
use super::form_impl::{Event, Form, FormKind, Kind};
use super::SheetWindow;
use crate::{Font, Painter};
use bo_render::{PathSeg, Rgba};
use bo_sheet::datatools::{split_text, SplitMode};
use serde_json::{json, Value};

/// Wizard size (reference pixels, title bar included).
const SIZE: (f32, f32) = (771.0, 546.0);
/// The preview area: left, top, width, height of the owner-drawn canvas.
const CANVAS: [f32; 4] = [32.0, 315.0, 720.0, 160.0];
/// Inside the canvas: the box's top (steps 1 and 2; step 3 has a header row above it).
const BOX_TOP: f32 = 34.0;
const HEAD_TOP: f32 = 18.0;
/// Box width and bottom (canvas pixels): the box spans x 0…686, y top…139.
const BOX_W: f32 = 686.0;
const BOX_BOTTOM: f32 = 139.0;
/// Character cell of the fixed-pitch preview font.
const CH_W: f32 = 8.0;
const CH_H: f32 = 16.0;
/// Width of step 1's row-number column.
const NUM_W: f32 = 16.0;
/// Most source lines read.
const MAX_LINES: usize = 1000;
/// Most characters a break can be placed at.
const MAX_BREAK: usize = 1000;

const INK: Rgba = Rgba(0, 0, 0, 255);
const WHITE: Rgba = Rgba(255, 255, 255, 255);
const FACE: Rgba = Rgba(0xf0, 0xf0, 0xf0, 255);
const ARROW: Rgba = Rgba(0xa4, 0xa4, 0xa4, 255);
const THUMB: Rgba = Rgba(0x85, 0x85, 0x85, 255);

/// Date orders of step 3 (the engine's `DateOrder`).
const ORDERS: [&str; 6] = ["MDY", "DMY", "YMD", "MYD", "DYM", "YDM"];
/// Text qualifiers.
const QUALIFIERS: [Option<char>; 3] = [Some('"'), Some('\''), None];

/// A column's format in step 3.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum ColFmt {
    #[default]
    General,
    Text,
    /// Date in `ORDERS[i]`.
    Date(usize),
    Skip,
}

/// State of the open wizard.
#[derive(Debug, Clone, Default)]
pub(super) struct Ttc {
    pub sheet: usize,
    /// The column of cells split: first row, column, last row.
    pub range: [u32; 3],
    /// Source text of each row (up to `MAX_LINES`).
    pub lines: Vec<String>,
    pub step: u8,
    pub fixed: bool,
    pub guessed_fixed: bool,
    /// Tab, semicolon, comma, space, other.
    pub delims: [bool; 5],
    pub other: String,
    pub consecutive: bool,
    pub qualifier: usize,
    pub breaks: Vec<usize>,
    pub formats: Vec<ColFmt>,
    /// Date order last chosen (new date columns take it).
    pub order: usize,
    /// Selected column of step 3.
    pub col: usize,
    pub dest: String,
    /// Advanced: decimal and thousands separators, trailing minus.
    pub decimal: String,
    pub thousands: String,
    pub trailing: bool,
    /// Preview scroll: first row and first character shown.
    pub top: usize,
    pub left: usize,
    /// The break being dragged (index into `breaks`).
    pub drag: Option<usize>,
}

impl Ttc {
    fn split(&self) -> SplitMode {
        if self.fixed {
            return SplitMode::Fixed {
                breaks: self.breaks.clone(),
            };
        }
        let mut delimiters = Vec::new();
        for (on, ch) in self.delims.iter().zip(['\t', ';', ',', ' ']) {
            if *on {
                delimiters.push(ch);
            }
        }
        if self.delims[4] {
            delimiters.extend(self.other.chars().take(1));
        }
        SplitMode::Delimited {
            delimiters,
            consecutive: self.consecutive,
            qualifier: QUALIFIERS[self.qualifier.min(2)],
        }
    }

    /// The fields of each source line as the current settings cut them.
    fn fields(&self) -> Vec<Vec<String>> {
        let mode = self.split();
        self.lines.iter().map(|l| split_text(l, &mode)).collect()
    }

    fn columns(&self) -> usize {
        self.fields().iter().map(Vec::len).max().unwrap_or(0).max(1)
    }

    fn format(&self, i: usize) -> ColFmt {
        self.formats.get(i).copied().unwrap_or_default()
    }

    /// Header label of column `i` in step 3.
    fn label(&self, i: usize, skip: &str, general: &str, text: &str) -> String {
        match self.format(i) {
            ColFmt::General => general.to_owned(),
            ColFmt::Text => text.to_owned(),
            ColFmt::Date(o) => ORDERS[o.min(5)].to_owned(),
            ColFmt::Skip => skip.to_owned(),
        }
    }

    fn longest(&self) -> usize {
        self.lines
            .iter()
            .map(|l| l.chars().count())
            .max()
            .unwrap_or(0)
    }

    /// The engine's `formats` list.
    fn formats_json(&self, n: usize) -> Vec<Value> {
        (0..n)
            .map(|i| match self.format(i) {
                ColFmt::General => json!("general"),
                ColFmt::Text => json!("text"),
                ColFmt::Date(o) => json!({ "date": ORDERS[o.min(5)].to_ascii_lowercase() }),
                ColFmt::Skip => json!("skip"),
            })
            .collect()
    }
}

fn col_letters(mut c: u32) -> String {
    let mut s = Vec::new();
    loop {
        s.push(b'A' + (c % 26) as u8);
        if c < 26 {
            break;
        }
        c = c / 26 - 1;
    }
    s.reverse();
    String::from_utf8(s).unwrap_or_default()
}

/// Rows of preview text that fit the box.
fn visible_rows(step: u8) -> usize {
    let top = if step == 2 { HEAD_TOP + CH_H } else { BOX_TOP };
    ((BOX_BOTTOM - top - 1.0) / CH_H).floor() as usize
}

impl SheetWindow {
    /// Data › Text to Columns: open the wizard on the selected column.
    pub(super) fn open_ttc(&mut self) -> bool {
        if self.view.editing() {
            self.view.commit();
        }
        let (r0, c0, r1, c1) = {
            let v = self.view.rng();
            let n = |k: &str| v.get(k).and_then(Value::as_u64).unwrap_or(0) as u32;
            (n("r0"), n("c0"), n("r1"), n("c1"))
        };
        if c0 != c1 {
            self.status = self.t(
                "sheetTtc.oneColumn",
                "Text to Columns can convert only one column at a time.",
            );
            return true;
        }
        // A single cell: the cells below it down to the first blank one.
        let sheet = self.view.sheet();
        let last = if r0 == r1 {
            let mut r = r0;
            while r < r0 + MAX_LINES as u32
                && self
                    .view
                    .engine()
                    .cell(sheet, r + 1, c0)
                    .is_ok_and(|d| !d.display.is_empty())
            {
                r += 1;
            }
            r
        } else {
            r1
        };
        let lines: Vec<String> = (r0..=last.min(r0 + MAX_LINES as u32 - 1))
            .map(|r| {
                self.view
                    .engine()
                    .cell(sheet, r, c0)
                    .map(|d| d.display)
                    .unwrap_or_default()
            })
            .collect();
        let range = json!({ "r0": r0, "c0": c0, "r1": last, "c1": c0 });
        let guess = self
            .view
            .query(json!({ "cmd": "textToColumnsGuess", "range": range }))
            .unwrap_or(Value::Null);
        let fixed = guess.get("mode").and_then(Value::as_str) == Some("fixed");
        let breaks = guess
            .get("breaks")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|b| b.as_u64().map(|b| b as usize))
                    .collect()
            })
            .unwrap_or_default();
        // Step 2 starts with Tab only, as the reference's (the guess only picks the type).
        let delims = [true, false, false, false, false];
        let ui = self.ui_locale_numbers();
        self.ttc = Some(Ttc {
            sheet,
            range: [r0, c0, last],
            lines,
            fixed,
            guessed_fixed: fixed,
            delims,
            breaks,
            dest: format!("${}${}", col_letters(c0), r0 + 1),
            decimal: ui.0.to_string(),
            thousands: ui.1.to_string(),
            ..Ttc::default()
        });
        self.ttc_form();
        true
    }

    /// Decimal and thousands separators of the engine's locale.
    fn ui_locale_numbers(&self) -> (char, char) {
        let l = self.view.engine().number_locale();
        (l.decimal, l.thousands)
    }

    /// Build the form of the wizard's current step.
    pub(super) fn ttc_form(&mut self) {
        let Some(st) = self.ttc.clone() else {
            return;
        };
        let t = |k: &str, f: &str| self.t(k, f);
        let title = t(
            "sheetTtc.title",
            "Convert Text to Columns Wizard - Step {0} of 3",
        )
        .replace("{0}", &(st.step + 1).to_string());
        let mut f = Form::new(&title, SIZE, vec![]);
        let label = |s: String| Kind::Label(s);
        match st.step {
            0 => {
                let found = if st.guessed_fixed {
                    t(
                        "sheetTtc.foundFixed",
                        "The Text Wizard has determined that your data is Fixed Width.",
                    )
                } else {
                    t(
                        "sheetTtc.foundDelimited",
                        "The Text Wizard has determined that your data is Delimited.",
                    )
                };
                f.add("found", None, [18.0, 46.0, 740.0, 22.0], label(found));
                f.add(
                    "hint",
                    None,
                    [18.0, 74.0, 740.0, 22.0],
                    label(t(
                        "sheetTtc.ifCorrect",
                        "If this is correct, choose Next, or choose the data type that best describes your data.",
                    )),
                );
                f.add(
                    "typeG",
                    None,
                    [18.0, 102.0, 733.0, 101.0],
                    Kind::Group(t("sheetTtc.dataType", "Original data type")),
                );
                f.add(
                    "choose",
                    None,
                    [32.0, 128.0, 700.0, 22.0],
                    label(t(
                        "sheetTtc.chooseType",
                        "Choose the file type that best describes your data:",
                    )),
                );
                f.add(
                    "delimited",
                    None,
                    [55.0, 150.0, 100.0, 22.0],
                    Kind::Check {
                        label: t("sheetTtc.delimited", "Delimited"),
                        on: !st.fixed,
                    },
                );
                f.add(
                    "fixed",
                    None,
                    [55.0, 175.0, 100.0, 22.0],
                    Kind::Check {
                        label: t("sheetTtc.fixedWidth", "Fixed width"),
                        on: st.fixed,
                    },
                );
                f.add(
                    "delimitedN",
                    None,
                    [161.0, 151.0, 590.0, 22.0],
                    label(t(
                        "sheetTtc.delimitedNote",
                        "- Characters such as commas or tabs separate each field.",
                    )),
                );
                f.add(
                    "fixedN",
                    None,
                    [161.0, 176.0, 590.0, 22.0],
                    label(t(
                        "sheetTtc.fixedNote",
                        "- Fields are aligned in columns with spaces between each field.",
                    )),
                );
                f.set_radios(&["delimited", "fixed"]);
                f.set_access("delimited", 0);
                f.set_access("fixed", 6);
                let prev = t("sheetTtc.previewOf", "Preview of selected data:");
                f.add("previewL", None, [32.0, 319.0, 400.0, 22.0], label(prev));
            }
            1 if st.fixed => {
                for (id, y, key, fb, x) in [
                    (
                        "w1",
                        46.0,
                        "sheetTtc.widths1",
                        "This screen lets you set field widths (column breaks).",
                        18.0,
                    ),
                    (
                        "w2",
                        67.0,
                        "sheetTtc.widths2",
                        "Lines with arrows signify a column break.",
                        18.0,
                    ),
                    (
                        "w3",
                        100.0,
                        "sheetTtc.create",
                        "To CREATE a break line, click at the desired position.",
                        42.0,
                    ),
                    (
                        "w4",
                        121.0,
                        "sheetTtc.delete",
                        "To DELETE a break line, double click on the line.",
                        42.0,
                    ),
                    (
                        "w5",
                        142.0,
                        "sheetTtc.move",
                        "To MOVE a break line, click and drag it.",
                        42.0,
                    ),
                ] {
                    f.add(id, None, [x, y, 700.0, 22.0], label(t(key, fb)));
                }
                f.add(
                    "previewG",
                    None,
                    [18.0, 290.0, 733.0, 190.0],
                    Kind::Group(t("sheetTtc.dataPreview", "Data preview")),
                );
                f.set_access("previewG", 5);
            }
            1 => {
                f.add(
                    "intro",
                    None,
                    [18.0, 46.0, 740.0, 22.0],
                    label(t(
                        "sheetTtc.delimsIntro",
                        "This screen lets you set the delimiters your data contains.  You can see how your text is affected in the preview below.",
                    )),
                );
                f.add(
                    "delimsG",
                    None,
                    [18.0, 78.0, 148.0, 160.0],
                    Kind::Group(t("sheetTtc.delimiters", "Delimiters")),
                );
                for (i, (id, key, fb, y, acc)) in [
                    ("dTab", "sheetTtc.tab", "Tab", 105.0, 0),
                    ("dSemi", "sheetTtc.semicolon", "Semicolon", 130.0, 2),
                    ("dComma", "sheetTtc.comma", "Comma", 155.0, 0),
                    ("dSpace", "sheetTtc.space", "Space", 180.0, 0),
                    ("dOther", "sheetTtc.other", "Other:", 206.0, 0),
                ]
                .into_iter()
                .enumerate()
                {
                    f.add(
                        id,
                        None,
                        [31.0, y, 75.0, 22.0],
                        Kind::Check {
                            label: t(key, fb),
                            on: st.delims[i],
                        },
                    );
                    f.set_access(id, acc);
                }
                f.add(
                    "other",
                    None,
                    [108.0, 205.0, 45.0, 26.0],
                    Kind::Edit {
                        value: st.other.chars().collect(),
                        caret: st.other.chars().count(),
                    },
                );
                f.add(
                    "consecutive",
                    None,
                    [181.0, 127.0, 300.0, 22.0],
                    Kind::Check {
                        label: t(
                            "sheetTtc.consecutive",
                            "Treat consecutive delimiters as one",
                        ),
                        on: st.consecutive,
                    },
                );
                f.set_access("consecutive", 1);
                f.add(
                    "qualL",
                    None,
                    [181.0, 166.0, 90.0, 22.0],
                    label(t("sheetTtc.qualifier", "Text qualifier:")),
                );
                f.set_access("qualL", 5);
                f.add(
                    "qualifier",
                    None,
                    [273.0, 163.0, 138.0, 27.0],
                    Kind::Combo {
                        items: vec![
                            "\"".to_owned(),
                            "'".to_owned(),
                            t("sheetTtc.none", "{none}"),
                        ],
                        sel: st.qualifier.min(2),
                    },
                );
                f.add(
                    "previewG",
                    None,
                    [18.0, 290.0, 733.0, 190.0],
                    Kind::Group(t("sheetTtc.dataPreview", "Data preview")),
                );
                f.set_access("previewG", 5);
            }
            _ => {
                f.add(
                    "intro",
                    None,
                    [18.0, 46.0, 740.0, 22.0],
                    label(t(
                        "sheetTtc.formatIntro",
                        "This screen lets you select each column and set the Data Format.",
                    )),
                );
                f.add(
                    "fmtG",
                    None,
                    [18.0, 74.0, 231.0, 137.0],
                    Kind::Group(t("sheetTtc.columnFormat", "Column data format")),
                );
                let fmt = st.format(st.col);
                for (id, key, fb, y, on) in [
                    (
                        "fGeneral",
                        "sheetTtc.general",
                        "General",
                        100.0,
                        fmt == ColFmt::General,
                    ),
                    ("fText", "sheetTtc.text", "Text", 125.0, fmt == ColFmt::Text),
                    (
                        "fDate",
                        "sheetTtc.date",
                        "Date:",
                        153.0,
                        matches!(fmt, ColFmt::Date(_)),
                    ),
                    (
                        "fSkip",
                        "sheetTtc.skip",
                        "Do not import column (skip)",
                        182.0,
                        fmt == ColFmt::Skip,
                    ),
                ] {
                    f.add(
                        id,
                        None,
                        [32.0, y, 215.0, 22.0],
                        Kind::Check {
                            label: t(key, fb),
                            on,
                        },
                    );
                }
                f.set_radios(&["fGeneral", "fText", "fDate", "fSkip"]);
                f.set_access("fGeneral", 0);
                f.set_access("fText", 0);
                f.set_access("fDate", 0);
                f.set_access("fSkip", 7);
                let order = match fmt {
                    ColFmt::Date(o) => o,
                    _ => st.order,
                };
                f.add(
                    "order",
                    None,
                    [102.0, 151.0, 134.0, 27.0],
                    Kind::Combo {
                        items: ORDERS.iter().map(|s| (*s).to_owned()).collect(),
                        sel: order,
                    },
                );
                f.add(
                    "generalN",
                    None,
                    [264.0, 117.0, 480.0, 44.0],
                    Kind::Note(t(
                        "sheetTtc.generalNote",
                        "'General' converts numeric values to numbers, date values to dates, and all remaining values to text.",
                    )),
                );
                f.add(
                    "advanced",
                    None,
                    [452.0, 153.0, 111.0, 28.0],
                    Kind::Button {
                        label: t("sheetTtc.advanced", "Advanced..."),
                        default: false,
                    },
                );
                f.set_access("advanced", 0);
                f.show("generalN", fmt == ColFmt::General);
                f.show("advanced", fmt == ColFmt::General);
                f.add(
                    "destL",
                    None,
                    [18.0, 227.0, 82.0, 22.0],
                    label(t("sheetTtc.destination", "Destination:")),
                );
                f.set_access("destL", 0);
                f.add(
                    "dest",
                    None,
                    [100.0, 224.0, 626.0, 26.0],
                    Kind::Edit {
                        value: st.dest.chars().collect(),
                        caret: st.dest.chars().count(),
                    },
                );
                f.add("pickDest", None, [725.0, 224.0, 26.0, 26.0], Kind::Canvas);
                f.add(
                    "previewG",
                    None,
                    [18.0, 290.0, 733.0, 190.0],
                    Kind::Group(t("sheetTtc.dataPreview", "Data preview")),
                );
                f.set_access("previewG", 5);
            }
        }
        f.add("preview", None, CANVAS, Kind::Canvas);
        for (id, x, key, fb, acc) in [
            ("cancel", 259.0, "common.cancel", "Cancel", None),
            ("back", 386.0, "sheetTtc.back", "< Back", Some(2)),
            ("next", 513.0, "sheetTtc.next", "Next >", Some(0)),
            ("finish", 640.0, "sheetTtc.finish", "Finish", Some(0)),
        ] {
            f.add(
                id,
                None,
                [x, 498.0, 111.0, 28.0],
                Kind::Button {
                    label: t(key, fb),
                    default: id == if st.step == 2 { "finish" } else { "next" },
                },
            );
            if let Some(a) = acc {
                f.set_access(id, a);
            }
        }
        f.enable("back", st.step > 0);
        f.enable("next", st.step < 2);
        if st.step > 0 {
            f.focus(if st.step == 2 { "finish" } else { "next" });
        }
        self.form = Some(f);
        self.form_kind = FormKind::TextToColumns;
    }

    /// Store the controls' values of the current step in the state.
    fn ttc_take(&mut self) {
        let (Some(f), Some(st)) = (self.form.as_ref(), self.ttc.as_mut()) else {
            return;
        };
        match st.step {
            0 => st.fixed = f.on("fixed"),
            1 if !st.fixed => {
                for (i, id) in ["dTab", "dSemi", "dComma", "dSpace", "dOther"]
                    .into_iter()
                    .enumerate()
                {
                    st.delims[i] = f.on(id);
                }
                st.other = f.text("other").chars().take(1).collect();
                st.consecutive = f.on("consecutive");
                st.qualifier = f.sel("qualifier").unwrap_or(0);
            }
            2 => st.dest = f.text("dest"),
            _ => {}
        }
    }

    /// Show step `step` (0-based).
    fn ttc_goto(&mut self, step: u8) {
        self.ttc_take();
        let Some(st) = self.ttc.as_mut() else {
            return;
        };
        st.step = step;
        st.top = 0;
        st.left = 0;
        let n = st.columns();
        if st.col >= n {
            st.col = 0;
        }
        self.ttc_form();
    }

    /// Finish: split the cells.
    fn ttc_finish(&mut self) {
        self.ttc_take();
        let Some(st) = self.ttc.clone() else {
            return;
        };
        let n = st.columns();
        let range =
            json!({ "r0": st.range[0], "c0": st.range[1], "r1": st.range[2], "c1": st.range[1] });
        let dest = match self.typed_range_at(&st.dest) {
            Some((sheet, r)) => Some(json!({
                "sheet": sheet.unwrap_or(st.sheet),
                "row": r.get("r0").and_then(Value::as_u64).unwrap_or(0),
                "col": r.get("c0").and_then(Value::as_u64).unwrap_or(0),
            })),
            None if st.dest.trim().is_empty() => None,
            None => {
                self.status = self.t(
                    "sheetTtc.badDest",
                    "The destination reference is not valid.",
                );
                return;
            }
        };
        let num = |s: &str| s.chars().next();
        self.form = None;
        self.ttc = None;
        self.view.run(json!({
            "cmd": "textToColumns", "sheet": st.sheet, "range": range,
            "split": st.split(), "formats": st.formats_json(n), "dest": dest,
            "decimal": num(&st.decimal), "thousands": num(&st.thousands),
            "trailingMinus": st.trailing,
        }));
    }

    /// A range typed in a dialog: (sheet, range).
    fn typed_range_at(&mut self, text: &str) -> Option<(Option<usize>, Value)> {
        let text = text.trim();
        if text.is_empty() {
            return None;
        }
        let out = self.view.query(json!({ "cmd": "resolve", "text": text }))?;
        let r = out.get("range")?.clone();
        let sheet = out.get("sheet").and_then(Value::as_u64).map(|s| s as usize);
        Some((sheet, r))
    }

    /// Events of the wizard.
    pub(super) fn ttc_event(&mut self, ev: &Event, count: u8) {
        let step = self.ttc.as_ref().map_or(0, |s| s.step);
        match ev {
            Event::Cancel | Event::Clicked("cancel") => {
                self.form = None;
                self.ttc = None;
            }
            Event::Clicked("back") if step > 0 => self.ttc_goto(step - 1),
            Event::Clicked("next") | Event::Ok if step < 2 => self.ttc_goto(step + 1),
            Event::Clicked("finish") | Event::Ok => self.ttc_finish(),
            Event::Clicked("advanced") => self.open_ttc_advanced(),
            Event::Changed(id @ ("delimited" | "fixed")) => {
                let fixed = *id == "fixed";
                if let Some(f) = self.form.as_mut() {
                    f.set_on("delimited", !fixed);
                    f.set_on("fixed", fixed);
                }
                self.ttc_take();
            }
            Event::Changed(id @ ("fGeneral" | "fText" | "fDate" | "fSkip" | "order")) => {
                let Some(f) = self.form.as_ref() else {
                    return;
                };
                let order = f.sel("order").unwrap_or(0);
                let fmt = match *id {
                    "fGeneral" => ColFmt::General,
                    "fText" => ColFmt::Text,
                    "fSkip" => ColFmt::Skip,
                    _ => ColFmt::Date(order),
                };
                if *id == "order" && !f.on("fDate") {
                    return;
                }
                if let Some(st) = self.ttc.as_mut() {
                    let col = st.col;
                    if st.formats.len() <= col {
                        st.formats.resize(col + 1, ColFmt::General);
                    }
                    st.formats[col] = fmt;
                    st.order = order;
                }
                self.ttc_sync_format();
            }
            Event::Changed(_) => self.ttc_take(),
            Event::Canvas("preview", x, y) => self.ttc_canvas_press(*x, *y, count),
            _ => {}
        }
    }

    /// Step 3: the radio buttons and date order follow the selected column.
    fn ttc_sync_format(&mut self) {
        let (Some(f), Some(st)) = (self.form.as_mut(), self.ttc.as_ref()) else {
            return;
        };
        let fmt = st.format(st.col);
        f.set_on("fGeneral", fmt == ColFmt::General);
        f.set_on("fText", fmt == ColFmt::Text);
        f.set_on("fDate", matches!(fmt, ColFmt::Date(_)));
        f.set_on("fSkip", fmt == ColFmt::Skip);
        if let ColFmt::Date(o) = fmt {
            f.set_sel("order", Some(o));
        }
        f.show("generalN", fmt == ColFmt::General);
        f.show("advanced", fmt == ColFmt::General);
    }

    /// A press in the preview canvas (canvas pixels).
    fn ttc_canvas_press(&mut self, x: f32, y: f32, count: u8) {
        let Some(st) = self.ttc.as_mut() else {
            return;
        };
        let rows = visible_rows(st.step);
        let n = st.lines.len();
        // Scroll arrows: below the box (left / right), right of it (up / down).
        if y > BOX_BOTTOM + 1.0 {
            if x < 30.0 {
                st.left = st.left.saturating_sub(8);
            } else if x > BOX_W - 30.0 {
                st.left = (st.left + 8).min(st.longest().saturating_sub(8));
            }
            return;
        }
        if x > BOX_W + 1.0 {
            if y < BOX_TOP + 30.0 {
                st.top = st.top.saturating_sub(1);
            } else if y > BOX_BOTTOM - 30.0 {
                st.top = (st.top + 1).min(n.saturating_sub(rows));
            }
            return;
        }
        let at = st.left + ((x - 1.0) / CH_W).round().max(0.0) as usize;
        match st.step {
            1 if st.fixed && y >= 0.0 => {
                let near = st.breaks.iter().position(|b| {
                    let bx = (*b as f32 - st.left as f32) * CH_W + 1.0;
                    (bx - x).abs() <= 4.0
                });
                match (near, count) {
                    (Some(i), 2..) => {
                        st.breaks.remove(i);
                        st.drag = None;
                    }
                    (Some(i), _) => st.drag = Some(i),
                    (None, _) => {
                        if at > 0 && at < MAX_BREAK && !st.breaks.contains(&at) {
                            st.breaks.push(at);
                            st.breaks.sort_unstable();
                            st.drag = st.breaks.iter().position(|b| *b == at);
                        }
                    }
                }
            }
            2 if y >= HEAD_TOP => {
                // The column under the pointer.
                let fields = st.fields();
                let widths = column_widths(&fields, st.fixed, &st.breaks);
                let mut edge = 0usize;
                let ch = st.left + ((x - 1.0) / CH_W).floor().max(0.0) as usize;
                let mut col = widths.len().saturating_sub(1);
                for (i, w) in widths.iter().enumerate() {
                    edge += w;
                    if ch < edge {
                        col = i;
                        break;
                    }
                }
                st.col = col;
                self.ttc_sync_format();
            }
            _ => {}
        }
    }

    /// Dragging a break of the fixed-width step (window point).
    pub(super) fn ttc_drag(&mut self, x: f32) -> bool {
        let Some(f) = self.form.as_ref() else {
            return false;
        };
        let (lx, _) = f.to_local(x, 0.0);
        let Some(st) = self.ttc.as_mut() else {
            return false;
        };
        let Some(i) = st.drag else {
            return false;
        };
        let at = st.left + ((lx - CANVAS[0] - 1.0) / CH_W).round().max(1.0) as usize;
        if let Some(b) = st.breaks.get_mut(i) {
            *b = at.min(MAX_BREAK);
        }
        true
    }

    /// Release ends a drag; a break dropped on another or at 0 goes away.
    pub(super) fn ttc_release(&mut self) -> bool {
        let Some(st) = self.ttc.as_mut() else {
            return false;
        };
        if st.drag.take().is_none() {
            return false;
        }
        st.breaks.sort_unstable();
        st.breaks.dedup();
        st.breaks.retain(|b| *b > 0);
        true
    }

    /// Advanced… of step 3: Advanced Text Import Settings (493 × 280 px).
    fn open_ttc_advanced(&mut self) {
        let Some(st) = self.ttc.clone() else {
            return;
        };
        let t = |k: &str, f: &str| self.t(k, f);
        let mut f = Form::new(
            &t("sheetTtc.advTitle", "Advanced Text Import Settings"),
            (493.0, 280.0),
            vec![],
        );
        f.add(
            "settingsG",
            None,
            [18.0, 46.0, 460.0, 22.0],
            Kind::Group(t(
                "sheetTtc.advSettings",
                "Settings used to recognize numeric data",
            )),
        );
        let seps: Vec<String> = [".", ",", " ", "'", "-"]
            .iter()
            .map(|s| (*s).to_owned())
            .collect();
        let pos = |s: &str| seps.iter().position(|x| x == s).unwrap_or(0);
        f.add(
            "decL",
            None,
            [32.0, 77.0, 140.0, 22.0],
            Kind::Label(t("sheetTtc.decimal", "Decimal separator:")),
        );
        f.set_access("decL", 0);
        f.add(
            "decimal",
            None,
            [172.0, 74.0, 115.0, 27.0],
            Kind::Combo {
                items: seps.clone(),
                sel: pos(&st.decimal),
            },
        );
        f.add(
            "thL",
            None,
            [32.0, 107.0, 140.0, 22.0],
            Kind::Label(t("sheetTtc.thousands", "Thousands separator:")),
        );
        f.set_access("thL", 0);
        f.add(
            "thousands",
            None,
            [172.0, 104.0, 115.0, 27.0],
            Kind::Combo {
                items: seps.clone(),
                sel: pos(&st.thousands),
            },
        );
        f.add(
            "note",
            None,
            [32.0, 140.0, 450.0, 44.0],
            Kind::Note(t(
                "sheetTtc.advNote",
                "Note: Numbers will be displayed using the numeric settings specified in the Regional Settings control panel.",
            )),
        );
        f.add(
            "reset",
            None,
            [32.0, 186.0, 111.0, 28.0],
            Kind::Button {
                label: t("sheetTtc.reset", "Reset"),
                default: false,
            },
        );
        f.set_access("reset", 0);
        f.add(
            "trailing",
            None,
            [240.0, 189.0, 240.0, 22.0],
            Kind::Check {
                label: t("sheetTtc.trailing", "Trailing minus for negative numbers"),
                on: st.trailing,
            },
        );
        f.set_access("trailing", 9);
        f.ok_cancel(&t("common.ok", "OK"), &t("common.cancel", "Cancel"));
        if let Some(c) = f.get_mut("ok") {
            c.rect = [235.0, 232.0, 111.0, 28.0];
        }
        if let Some(c) = f.get_mut("cancel") {
            c.rect = [362.0, 232.0, 111.0, 28.0];
        }
        if let Some(old) = self.form.take() {
            self.form_stash = Some((old, self.form_kind));
        }
        self.form = Some(f);
        self.form_kind = FormKind::TtcAdvanced;
    }

    /// Events of Advanced Text Import Settings.
    pub(super) fn ttc_advanced_event(&mut self, ev: &Event) {
        let back = |w: &mut Self| {
            if let Some((f, k)) = w.form_stash.take() {
                w.form = Some(f);
                w.form_kind = k;
            }
        };
        match ev {
            Event::Cancel | Event::Clicked("cancel") => back(self),
            Event::Clicked("reset") => {
                let (d, th) = self.ui_locale_numbers();
                if let Some(f) = self.form.as_mut() {
                    let items = [".", ",", " ", "'", "-"];
                    let pos = |c: char| items.iter().position(|x| x.starts_with(c)).unwrap_or(0);
                    f.set_sel("decimal", Some(pos(d)));
                    f.set_sel("thousands", Some(pos(th)));
                    f.set_on("trailing", true);
                }
            }
            Event::Ok | Event::Clicked("ok") => {
                let items = [".", ",", " ", "'", "-"];
                if let (Some(f), Some(st)) = (self.form.as_ref(), self.ttc.as_mut()) {
                    st.decimal = items[f.sel("decimal").unwrap_or(0).min(4)].to_owned();
                    st.thousands = items[f.sel("thousands").unwrap_or(0).min(4)].to_owned();
                    st.trailing = f.on("trailing");
                }
                back(self);
            }
            _ => {}
        }
    }

    /// Draw the wizard's owner-drawn areas.
    pub(super) fn paint_ttc(&self, f: &Form, id: &str, r: [f32; 4], p: &mut dyn Painter) {
        let Some(st) = self.ttc.as_ref() else {
            return;
        };
        let k = f.scale_k();
        if id == "pickDest" {
            paint_pick_button(p, r, k);
            return;
        }
        if id != "preview" {
            return;
        }
        let px = |v: f32| (v * k).round();
        let (x0, y0) = (r[0], r[1]);
        let mono = Font {
            family: "Courier New",
            size: (CH_W / 0.6 * k).round(),
            weight: 400,
            italic: false,
            underline: false,
        };
        let text_dy = ((CH_H * k - mono.size * 1.13) / 2.0).round();
        let rows = visible_rows(st.step);
        let shown: Vec<&String> = st.lines.iter().skip(st.top).take(rows + 1).collect();
        let cut =
            |s: &str, from: usize, n: usize| -> String { s.chars().skip(from).take(n).collect() };
        let max_chars = ((BOX_W - 2.0) / CH_W) as usize + 1;
        let box_top = if st.step == 2 { HEAD_TOP } else { BOX_TOP };
        // Box: black top / left edge, white bottom / right edge (sunken).
        let bx = [
            x0,
            y0 + px(box_top),
            px(BOX_W) + 1.0,
            px(BOX_BOTTOM - box_top) + 1.0,
        ];
        let inside = if st.step == 0 { FACE } else { WHITE };
        p.fill_rect(bx[0], bx[1], bx[2], bx[3], WHITE);
        p.fill_rect(bx[0], bx[1], bx[2] - 1.0, bx[3] - 1.0, INK);
        p.fill_rect(bx[0] + 1.0, bx[1] + 1.0, bx[2] - 2.0, bx[3] - 2.0, inside);
        p.push_clip(bx[0] + 1.0, bx[1] + 1.0, bx[2] - 2.0, bx[3] - 2.0);
        match st.step {
            0 => {
                // Row numbers in a column of cells, then the raw text.
                let small = Font {
                    family: "Segoe UI",
                    size: (11.0 * k).round(),
                    ..mono
                };
                for i in 0..=rows {
                    let ry = bx[1] + 1.0 + px(i as f32 * CH_H);
                    let n = (st.range[0] as usize + st.top + i + 1).to_string();
                    p.fill_rect(bx[0] + 1.0, ry, px(NUM_W) - 1.0, 1.0, WHITE);
                    p.fill_rect(bx[0] + 1.0, ry + px(CH_H) - 2.0, px(NUM_W) - 1.0, 1.0, INK);
                    p.text_font(&n, bx[0] + px(3.0), ry + text_dy, &small, INK);
                    if let Some(line) = shown.get(i) {
                        p.text_font(
                            &cut(line, st.left, max_chars),
                            bx[0] + px(NUM_W) + 1.0,
                            ry + text_dy,
                            &mono,
                            INK,
                        );
                    }
                }
                p.fill_rect(bx[0] + px(NUM_W), bx[1], 1.0, bx[3], INK);
            }
            1 if st.fixed => {
                for (i, line) in shown.iter().enumerate() {
                    let ry = bx[1] + 1.0 + px(i as f32 * CH_H) + px(4.0);
                    p.text_font(
                        &cut(line, st.left, max_chars),
                        bx[0] + 1.0,
                        ry + text_dy,
                        &mono,
                        INK,
                    );
                }
            }
            1 => {
                let fields = st.fields();
                let widths = column_widths(&fields, false, &[]);
                for (i, row) in fields.iter().skip(st.top).take(rows + 1).enumerate() {
                    let ry = bx[1] + 1.0 + px(i as f32 * CH_H) + px(4.0);
                    let mut c = 0usize;
                    for (j, field) in row.iter().enumerate() {
                        let fx = c as f32 - st.left as f32;
                        p.text_font(field, bx[0] + 1.0 + px(fx * CH_W), ry + text_dy, &mono, INK);
                        c += widths.get(j).copied().unwrap_or(0);
                    }
                }
                // Column lines.
                let mut c = 0usize;
                for w in widths.iter().take(widths.len().saturating_sub(1)) {
                    c += w;
                    let lx = bx[0] + px((c as f32 - st.left as f32) * CH_W);
                    p.fill_rect(lx, bx[1], 1.0, bx[3], INK);
                }
            }
            _ => {
                let fields = st.fields();
                let widths = column_widths(&fields, st.fixed, &st.breaks);
                let general = self.t("sheetTtc.general", "General");
                let text = self.t("sheetTtc.text", "Text");
                let skip = self.t("sheetTtc.skipColumn", "Skip Column");
                let head = px(CH_H);
                let mut c = 0usize;
                for (j, w) in widths.iter().enumerate() {
                    let lx = bx[0] + px((c as f32 - st.left as f32) * CH_W);
                    let cw = px(*w as f32 * CH_W);
                    let sel = j == st.col;
                    // Header cell (grey) and the column (black when selected).
                    p.fill_rect(lx + 1.0, bx[1] + 1.0, cw, head - 1.0, FACE);
                    if sel {
                        p.fill_rect(
                            lx + 1.0,
                            bx[1] + head + 1.0,
                            cw - 1.0,
                            bx[3] - head - 2.0,
                            INK,
                        );
                    }
                    p.push_clip(lx + 1.0, bx[1], cw - 1.0, bx[3]);
                    let lab = st.label(j, &skip, &general, &text);
                    p.text_font(&lab, lx + 1.0, bx[1] + 1.0 + text_dy, &mono, INK);
                    p.fill_rect(lx + 1.0, bx[1] + head - 2.0, cw, 1.0, INK);
                    for (i, row) in fields.iter().skip(st.top).take(rows).enumerate() {
                        let ry = bx[1] + head + 1.0 + px(i as f32 * CH_H);
                        if let Some(field) = row.get(j) {
                            p.text_font(
                                field,
                                lx + 1.0,
                                ry + text_dy,
                                &mono,
                                if sel { WHITE } else { INK },
                            );
                        }
                    }
                    p.pop_clip();
                    c += w;
                    let ex = bx[0] + px((c as f32 - st.left as f32) * CH_W);
                    p.fill_rect(ex, bx[1], 1.0, bx[3], INK);
                }
                p.fill_rect(bx[0] + 1.0, bx[1] + head, bx[2] - 2.0, 2.0, INK);
            }
        }
        p.pop_clip();
        // Ruler and break lines of the fixed-width step.
        if st.step == 1 && st.fixed {
            let base = y0 + px(BOX_TOP - 12.0);
            p.fill_rect(x0, base, bx[2] - 1.0, 2.0, INK);
            let small = Font {
                family: "Segoe UI",
                size: (12.5 * k).round(),
                ..mono
            };
            let mut i = st.left;
            loop {
                let tx = x0 + px((i - st.left) as f32 * CH_W);
                if tx > x0 + bx[2] - 2.0 {
                    break;
                }
                let tall = if i % 10 == 0 {
                    5.0
                } else if i % 5 == 0 {
                    3.0
                } else {
                    2.0
                };
                p.fill_rect(tx, base - px(tall), 1.0, px(tall), INK);
                if i % 10 == 0 && i > 0 {
                    // Centred over its tick, as the reference's.
                    let label = i.to_string();
                    let w = label.len() as f32 * small.size * 0.55;
                    p.text_font(&label, (tx - w / 2.0).round(), base - px(18.0), &small, INK);
                }
                i += 1;
            }
            for b in &st.breaks {
                if *b < st.left {
                    continue;
                }
                let lx = x0 + px((*b - st.left) as f32 * CH_W);
                if lx > x0 + bx[2] - 2.0 {
                    continue;
                }
                p.fill_rect(lx, base + 2.0, 1.0, bx[1] + bx[3] - base - 3.0, INK);
                let a = px(4.0);
                p.path(
                    &[
                        PathSeg::M(lx + 0.5, base + 2.0),
                        PathSeg::L(lx + 0.5 + a, base + 2.0 + a),
                        PathSeg::L(lx + 0.5 - a, base + 2.0 + a),
                        PathSeg::Z,
                    ],
                    Some(INK),
                    None,
                );
            }
        }
        // Scroll arrows (grey): left / right under the box, up / down right of it when the
        // rows do not fit; the vertical thumb.
        let tri = |p: &mut dyn Painter, cx: f32, cy: f32, dir: u8| {
            let a = px(4.0);
            let pts = match dir {
                0 => [
                    (cx - a / 2.0, cy),
                    (cx + a / 2.0, cy - a),
                    (cx + a / 2.0, cy + a),
                ],
                1 => [
                    (cx + a / 2.0, cy),
                    (cx - a / 2.0, cy - a),
                    (cx - a / 2.0, cy + a),
                ],
                2 => [
                    (cx, cy - a / 2.0),
                    (cx - a, cy + a / 2.0),
                    (cx + a, cy + a / 2.0),
                ],
                _ => [
                    (cx, cy + a / 2.0),
                    (cx - a, cy - a / 2.0),
                    (cx + a, cy - a / 2.0),
                ],
            };
            p.path(
                &[
                    PathSeg::M(pts[0].0, pts[0].1),
                    PathSeg::L(pts[1].0, pts[1].1),
                    PathSeg::L(pts[2].0, pts[2].1),
                    PathSeg::Z,
                ],
                Some(ARROW),
                None,
            );
        };
        let ay = y0 + px(BOX_BOTTOM + 9.0);
        tri(p, x0 + px(9.0), ay, 0);
        tri(p, x0 + px(675.0), ay, 1);
        let vx = x0 + px(BOX_W + 11.0);
        if st.lines.len() > rows {
            tri(p, vx, y0 + px(box_top + 10.0), 2);
            tri(p, vx, y0 + px(BOX_BOTTOM - 10.0), 3);
        }
        let span = BOX_BOTTOM - box_top - 50.0;
        let frac = if st.lines.len() > rows {
            st.top as f32 / (st.lines.len() - rows) as f32
        } else {
            0.0
        };
        p.fill_rect(
            vx - 1.0,
            y0 + px(box_top + 21.0 + frac * span),
            px(2.0),
            px(18.0),
            THUMB,
        );
    }
}

/// Character widths of the preview columns: the widest field + 1 (delimited), or the
/// distances between breaks (fixed).
fn column_widths(fields: &[Vec<String>], fixed: bool, breaks: &[usize]) -> Vec<usize> {
    let n = fields.iter().map(Vec::len).max().unwrap_or(0).max(1);
    if fixed {
        let longest = fields
            .iter()
            .map(|r| r.iter().map(|f| f.chars().count()).sum::<usize>())
            .max()
            .unwrap_or(0);
        let mut out = Vec::new();
        let mut prev = 0;
        for b in breaks {
            out.push(b - prev);
            prev = *b;
        }
        out.push(longest.saturating_sub(prev).max(8));
        out.truncate(n.max(breaks.len() + 1));
        return out;
    }
    (0..n)
        .map(|j| {
            fields
                .iter()
                .filter_map(|r| r.get(j))
                .map(|f| f.chars().count())
                .max()
                .unwrap_or(0)
                .max(1)
                + 1
        })
        .collect()
}

/// The range-picker button at the right of a reference box (a box with an arrow up out of
/// it: collapse the dialog to pick cells). Own drawing.
pub(super) fn paint_pick_button(p: &mut dyn Painter, r: [f32; 4], k: f32) {
    // The box's edge continues round it; inside, a small push button (#D0D0D0 frame, #FDFDFD
    // face, measured on the reference) with a bold arrow up over a bar.
    let edge = Rgba(0x7a, 0x7a, 0x7a, 255);
    p.fill_rect(r[0], r[1], r[2], r[3], edge);
    p.fill_rect(r[0], r[1] + 1.0, r[2] - 1.0, r[3] - 2.0, WHITE);
    let b = [r[0] + 1.0, r[1] + 1.0, r[2] - 3.0, r[3] - 3.0];
    p.fill_rect(b[0], b[1], b[2], b[3], Rgba(0xd0, 0xd0, 0xd0, 255));
    p.fill_rect(
        b[0] + 1.0,
        b[1] + 1.0,
        b[2] - 2.0,
        b[3] - 2.0,
        Rgba(0xfd, 0xfd, 0xfd, 255),
    );
    let (cx, cy) = ((b[0] + b[2] / 2.0).round(), (b[1] + b[3] / 2.0).round());
    let a = (4.5 * k).round();
    let stem = (2.5 * k).round().max(2.0);
    p.path(
        &[
            PathSeg::M(cx, cy - a - 1.0),
            PathSeg::L(cx + a, cy - 1.0),
            PathSeg::L(cx - a, cy - 1.0),
            PathSeg::Z,
        ],
        Some(INK),
        None,
    );
    p.fill_rect(cx - (stem / 2.0).round(), cy - 1.0, stem, a * 0.8, INK);
    p.fill_rect(cx - a, cy + a, 2.0 * a, (1.5 * k).round().max(1.0), INK);
}
