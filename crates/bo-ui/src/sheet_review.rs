//! BlueSheet round 3: Page Layout and Review tab commands, calculation options (manual
//! calculation, F9 / Shift+F9) and the Text to Columns wizard, on the engine's sheet
//! commands (`setPageSetup`, `pageBreak`, `addComment`, `protectSheet`, `setCalcOptions`,
//! `textToColumns`).
use super::{SheetDialog, SheetWindow};
use crate::dialog::{Dialog, DialogButton, DialogKind};
use crate::TextMeasure;
use serde_json::{json, Value};

/// Margins of the Normal, Wide and Narrow presets in inches (left, right, top, bottom).
const MARGINS: [(&str, [f64; 4]); 3] = [
    ("normal", [0.7, 0.7, 0.75, 0.75]),
    ("wide", [1.0, 1.0, 1.0, 1.0]),
    ("narrow", [0.25, 0.25, 0.75, 0.75]),
];

impl SheetWindow {
    /// Commands of the Page Layout and Review tabs and the calculation menu; `None` when
    /// `kind.arg` is not one of them.
    pub(super) fn more_action(
        &mut self,
        kind: &str,
        arg: &str,
        text: &dyn TextMeasure,
    ) -> Option<bool> {
        let sheet = self.view.sheet();
        let setup = |v: Value| {
            let mut cmd = json!({ "cmd": "setPageSetup", "sheet": sheet });
            if let (Some(o), Some(extra)) = (cmd.as_object_mut(), v.as_object()) {
                o.extend(extra.clone());
            }
            cmd
        };
        let _ = text;
        Some(match (kind, arg) {
            ("margins", m) => match MARGINS.iter().find(|(k, _)| *k == m) {
                Some((_, v)) => self.page_setup(setup(json!({ "margins": v }))),
                None => false,
            },
            ("orient", o @ ("portrait" | "landscape")) => {
                self.page_setup(setup(json!({ "orientation": o })))
            }
            ("paper", n) => match n.parse::<u32>() {
                Ok(code) => self.page_setup(setup(json!({ "paperSize": code }))),
                Err(_) => false,
            },
            ("printArea", "set") => {
                let range = self.view.rng();
                self.page_setup(setup(json!({ "printArea": range })))
            }
            ("printArea", "clear") => self.page_setup(setup(json!({ "printArea": null }))),
            ("breaks", "insert" | "remove") => {
                let (r, c) = self.view.active();
                self.page_setup(json!({
                    "cmd": "pageBreak", "sheet": sheet,
                    "row": (r > 0).then_some(r), "col": (c > 0).then_some(c),
                    "remove": arg == "remove",
                }))
            }
            ("breaks", "reset") => {
                self.page_setup(json!({ "cmd": "resetPageBreaks", "sheet": sheet }))
            }
            ("fitw" | "fith", n) => {
                let n: u32 = n.parse().unwrap_or(0);
                let [w, h] = self.fit_pages();
                let fit = if kind == "fitw" { [n, h] } else { [w, n] };
                let v = if fit == [0, 0] {
                    json!({ "fit": null })
                } else {
                    json!({ "fit": fit })
                };
                self.page_setup(setup(v))
            }
            ("scale", n) => match n.parse::<u32>() {
                Ok(pct) => self.page_setup(setup(json!({ "scale": pct, "fit": null }))),
                Err(_) => false,
            },
            ("calc", mode @ ("auto" | "manual")) => self.set_calc_mode(mode == "manual"),
            ("sheet", "printGridlines") => {
                let on = !self.print_option("gridLines");
                self.page_setup(setup(json!({ "gridLines": on })))
            }
            ("sheet", "printHeadings") => {
                let on = !self.print_option("headings");
                self.page_setup(setup(json!({ "headings": on })))
            }
            ("sheet", "newComment") => self.open_review_dialog(SheetDialog::Comment),
            ("sheet", "deleteComment") => self.delete_comment(),
            ("sheet", "prevComment") => self.step_comment(false),
            ("sheet", "nextComment") => self.step_comment(true),
            ("sheet", "protectSheet") => self.open_protect_sheet(),
            ("sheet", "protectWorkbook") => self.open_protect_workbook(),
            _ => return None,
        })
    }

    fn page_setup(&mut self, cmd: Value) -> bool {
        let ok = self.view.query(cmd).is_some();
        if ok {
            self.view.changed();
        }
        ok
    }

    /// The sheet's page setup as the engine reports it.
    fn setup_state(&mut self) -> Value {
        let sheet = self.view.sheet();
        self.view
            .query(json!({ "cmd": "pageSetup", "sheet": sheet }))
            .unwrap_or(Value::Null)
    }

    fn fit_pages(&mut self) -> [u32; 2] {
        let s = self.setup_state();
        let at = |i: usize| {
            s.get("fit")
                .and_then(|f| f.get(i))
                .and_then(Value::as_u64)
                .unwrap_or(0) as u32
        };
        [at(0), at(1)]
    }

    fn print_option(&mut self, key: &str) -> bool {
        let s = self.setup_state();
        s.get(key)
            .or_else(|| s.get("extra").and_then(|e| e.get(key)))
            .and_then(Value::as_bool)
            .unwrap_or(false)
    }

    /// Calculation mode: manual (`true`) or automatic, saved in the workbook (calcPr).
    pub(super) fn set_calc_mode(&mut self, manual: bool) -> bool {
        let Some(state) = self.view.query(json!({ "cmd": "calcOptions" })) else {
            return false;
        };
        let mut opts = state.get("options").cloned().unwrap_or_else(|| json!({}));
        opts["mode"] = json!(if manual { "manual" } else { "auto" });
        let ok = self
            .view
            .query(json!({ "cmd": "setCalcOptions", "options": opts }))
            .is_some();
        if ok {
            self.view.changed();
        }
        self.refresh_calc_state();
        ok
    }

    /// Re-read the calculation mode and whether formulas wait for F9.
    pub(super) fn refresh_calc_state(&mut self) {
        let s = self.view.query(json!({ "cmd": "calcOptions" }));
        let manual = s
            .as_ref()
            .and_then(|s| {
                s.get("options")?
                    .get("mode")?
                    .as_str()
                    .map(|m| m == "manual")
            })
            .unwrap_or(false);
        let needs = s
            .as_ref()
            .and_then(|s| s.get("needsCalc")?.as_bool())
            .unwrap_or(false);
        self.calc_state = (manual, manual && needs);
    }

    /// F9: calculate everything; Shift+F9: the active sheet only.
    pub(super) fn calculate(&mut self, sheet_only: bool) -> bool {
        let cmd = if sheet_only {
            json!({ "cmd": "calculate", "sheet": self.view.sheet() })
        } else {
            json!({ "cmd": "calculate" })
        };
        let ok = self.view.query(cmd).is_some();
        self.view.invalidate();
        self.view.reload();
        self.refresh_calc_state();
        ok
    }

    /// Threaded comments of the sheet: (id, row, column), sorted by row then column.
    fn comment_cells(&self) -> Vec<(String, u32, u32)> {
        let mut out: Vec<(String, u32, u32)> = self
            .view
            .engine()
            .workbook()
            .and_then(|w| w.sheets.get(self.view.sheet()))
            .map(|ws| {
                ws.threaded_comments
                    .iter()
                    .filter(|c| c.parent_id.is_none())
                    .map(|c| (c.id.clone(), c.cell.row, c.cell.col))
                    .chain(
                        ws.comments
                            .iter()
                            .map(|c| (String::new(), c.cell.row, c.cell.col)),
                    )
                    .collect()
            })
            .unwrap_or_default();
        out.sort_by_key(|c| (c.1, c.2));
        out.dedup_by_key(|c| (c.1, c.2));
        out
    }

    /// Cells with a comment or note (the grid marks them).
    pub(super) fn commented_cells(&self) -> Vec<(u32, u32)> {
        self.comment_cells()
            .into_iter()
            .map(|(_, r, c)| (r, c))
            .collect()
    }

    fn delete_comment(&mut self) -> bool {
        let at = self.view.active();
        let Some((id, ..)) = self
            .comment_cells()
            .into_iter()
            .find(|c| (c.1, c.2) == at && !c.0.is_empty())
        else {
            return false;
        };
        let sheet = self.view.sheet();
        let ok = self
            .view
            .query(json!({ "cmd": "deleteComment", "sheet": sheet, "id": id }))
            .is_some();
        if ok {
            self.view.changed();
        }
        ok
    }

    fn step_comment(&mut self, forward: bool) -> bool {
        let list = self.comment_cells();
        if list.is_empty() {
            self.status = self.t("sheetMenu.noComments", "This sheet has no comments.");
            return true;
        }
        let at = self.view.active();
        let next = if forward {
            list.iter()
                .find(|c| (c.1, c.2) > at)
                .or_else(|| list.first())
        } else {
            list.iter()
                .rev()
                .find(|c| (c.1, c.2) < at)
                .or_else(|| list.last())
        };
        match next {
            Some(&(_, r, c)) => {
                self.view.select((r, c), (r, c), (r, c));
                self.status = self.comment_text(r, c);
                true
            }
            None => false,
        }
    }

    /// Text of the comment on a cell (first post of its thread).
    pub(super) fn comment_text(&self, r: u32, c: u32) -> String {
        self.view
            .engine()
            .workbook()
            .and_then(|w| w.sheets.get(self.view.sheet()))
            .and_then(|ws| {
                ws.threaded_comments
                    .iter()
                    .find(|t| t.parent_id.is_none() && (t.cell.row, t.cell.col) == (r, c))
                    .map(|t| t.text.clone())
            })
            .unwrap_or_default()
    }

    /// Open the comment, protection or Text to Columns dialog.
    pub(super) fn open_review_dialog(&mut self, kind: SheetDialog) -> bool {
        let ok = self.t("common.ok", "OK");
        let cancel = self.t("common.cancel", "Cancel");
        let d = match kind {
            SheetDialog::Comment => Dialog::new(
                DialogKind::Link,
                self.t("sheetMenu.commentTitle", "New Comment"),
                vec![(
                    self.t("sheetMenu.commentText", "Comment"),
                    self.comment_text(self.view.active().0, self.view.active().1),
                )],
                vec![(ok, DialogButton::Ok), (cancel, DialogButton::Cancel)],
                Vec::new(),
            ),
            SheetDialog::TextToColumns(step) => self.ttc_dialog(step),
            _ => return false,
        };
        self.dialog = Some((kind, d));
        true
    }

    /// A button of a dialog this module opened; `false` when it is not one of them.
    pub(super) fn review_dialog_button(
        &mut self,
        kind: SheetDialog,
        b: DialogButton,
        values: &[String],
    ) -> bool {
        let v = |i: usize| values.get(i).cloned().unwrap_or_default();
        let sheet = self.view.sheet();
        match kind {
            SheetDialog::Comment => {
                self.dialog = None;
                let text = v(0).trim().to_owned();
                if text.is_empty() {
                    return true;
                }
                let (row, col) = self.view.active();
                // A new text replaces the cell's thread.
                if self
                    .comment_cells()
                    .iter()
                    .any(|c| (c.1, c.2) == (row, col) && !c.0.is_empty())
                {
                    self.delete_comment();
                }
                let author = std::env::var("USERNAME").unwrap_or_default();
                if self
                    .view
                    .query(json!({
                        "cmd": "addComment", "sheet": sheet, "row": row, "col": col,
                        "text": text, "author": author,
                    }))
                    .is_some()
                {
                    self.view.changed();
                }
                true
            }
            SheetDialog::TextToColumns(step) => {
                self.ttc_button(step, b, values);
                true
            }
            _ => false,
        }
    }

    /// Split settings of the wizard's first step (mode, delimiters, breaks, consecutive).
    fn ttc_split(values: &[String]) -> Value {
        let v = |i: usize| values.get(i).cloned().unwrap_or_default();
        if v(0).trim().eq_ignore_ascii_case("fixed") {
            let breaks: Vec<usize> = v(2)
                .split([',', ';', ' '])
                .filter_map(|n| n.trim().parse().ok())
                .collect();
            return json!({ "mode": "fixed", "breaks": breaks });
        }
        let mut delims = Vec::new();
        for part in v(1).split(',').map(str::trim).filter(|p| !p.is_empty()) {
            match part.to_ascii_lowercase().as_str() {
                "tab" => delims.push('\t'),
                "semicolon" => delims.push(';'),
                "comma" => delims.push(','),
                "space" => delims.push(' '),
                other => delims.extend(other.chars()),
            }
        }
        if delims.is_empty() {
            delims.push('\t');
        }
        json!({
            "mode": "delimited", "delimiters": delims,
            "consecutive": v(3) == "1", "qualifier": "\"",
        })
    }

    fn ttc_dialog(&mut self, step: u8) -> Dialog {
        let cancel = self.t("common.cancel", "Cancel");
        let title = format!(
            "{} – {}",
            self.t("sheetMenu.ttcTitle", "Convert Text to Columns"),
            self.strings
                .fmt("sheetMenu.ttcStep", &[("n", &(step + 1).to_string())])
        );
        if step == 0 {
            let saved = self.ttc_values.clone();
            let s = |i: usize, d: &str| saved.get(i).cloned().unwrap_or_else(|| d.to_owned());
            return Dialog::new(
                DialogKind::Paragraph,
                title,
                vec![
                    (
                        self.t("sheetMenu.ttcMode", "Original data type"),
                        s(0, "delimited"),
                    ),
                    (
                        self.t("sheetMenu.ttcDelims", "Delimiters"),
                        s(1, "tab, comma"),
                    ),
                    (self.t("sheetMenu.ttcWidths", "Column breaks"), s(2, "")),
                ],
                vec![
                    (
                        self.t("sheetMenu.ttcNext", "Next >"),
                        DialogButton::FindNext,
                    ),
                    (cancel, DialogButton::Cancel),
                ],
                Vec::new(),
            )
            .with_checks(vec![(
                self.t(
                    "sheetMenu.ttcConsecutive",
                    "Treat consecutive delimiters as one",
                ),
                s(3, "0") == "1",
            )]);
        }
        // Step 2: preview of the first rows as the split leaves them.
        let range = self.view.rng();
        let split = Self::ttc_split(&self.ttc_values);
        let preview = self
            .view
            .query(json!({
                "cmd": "textToColumns", "range": range, "split": split, "preview": true,
            }))
            .and_then(|p| p.get("rows").and_then(Value::as_array).cloned())
            .unwrap_or_default();
        let mut lines = vec![self.t("sheetMenu.ttcPreview", "Preview")];
        lines.extend(preview.iter().take(8).map(|row| {
            row.as_array()
                .map(|f| {
                    f.iter()
                        .map(|x| format!("[{}]", x.as_str().unwrap_or("")))
                        .collect::<Vec<_>>()
                        .join(" ")
                })
                .unwrap_or_default()
        }));
        Dialog::new(
            DialogKind::Paragraph,
            title,
            Vec::new(),
            vec![
                (self.t("sheetMenu.ttcFinish", "Finish"), DialogButton::Ok),
                (self.t("sheetMenu.ttcBack", "< Back"), DialogButton::Replace),
                (cancel, DialogButton::Cancel),
            ],
            lines,
        )
    }

    fn ttc_button(&mut self, step: u8, b: DialogButton, values: &[String]) {
        match (step, b) {
            (0, DialogButton::FindNext | DialogButton::Ok) => {
                self.ttc_values = values.to_vec();
                self.open_review_dialog(SheetDialog::TextToColumns(1));
            }
            (_, DialogButton::Replace) => {
                self.open_review_dialog(SheetDialog::TextToColumns(0));
            }
            (_, DialogButton::Ok) => {
                self.dialog = None;
                let range = self.view.rng();
                let split = Self::ttc_split(&self.ttc_values);
                self.view.run(json!({
                    "cmd": "textToColumns", "range": range, "split": split,
                }));
            }
            _ => {}
        }
    }
}
