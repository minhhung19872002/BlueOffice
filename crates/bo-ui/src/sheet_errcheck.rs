//! Formulas › Error Checking as the reference's dialog (round 52, measured at 125 %: 680 ×
//! 286 px, `show/sheet-59`): "Error in cell D8" with the formula, the error's title and
//! description, Help / Show Calculation Steps / Ignore Error / Edit in Formula Bar on the
//! right, Options… / Previous / Next at the bottom. The engine's `errorCheck` lists the
//! findings; Ignore Error records `ignoredError` (§18.3.1.50) through `ignoreError`.
use super::form_impl::{Event, Form, FormKind, Kind};
use super::SheetWindow;
use bo_model::sheet::col_to_letters;
use serde_json::{json, Value};

/// One finding: row, column, rule (`evalError`, `inconsistentFormula`, …).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Finding {
    pub row: u32,
    pub col: u32,
    pub rule: String,
}

/// The open dialog: the sheet's findings and the one shown.
#[derive(Debug, Clone, Default)]
pub(super) struct ErrCheck {
    pub sheet: usize,
    pub list: Vec<Finding>,
    pub at: usize,
}

impl SheetWindow {
    fn err_findings(&mut self) -> Vec<Finding> {
        let sheet = self.view.sheet();
        let out = self
            .view
            .query(json!({ "cmd": "errorCheck", "sheet": sheet }))
            .unwrap_or(Value::Null);
        out.as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|e| {
                        Some(Finding {
                            row: u32::try_from(e.get("row")?.as_u64()?).ok()?,
                            col: u32::try_from(e.get("col")?.as_u64()?).ok()?,
                            rule: e.get("rule")?.as_str()?.to_owned(),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Formulas › Error Checking: the dialog on the first finding from the active cell, or
    /// "complete" on the status bar when there is none.
    pub(super) fn open_error_check(&mut self) -> bool {
        if self.view.editing() {
            self.view.commit();
        }
        let list = self.err_findings();
        if list.is_empty() {
            self.status = self.t(
                "sheetFn.errDone",
                "The error check is complete for the entire sheet.",
            );
            return true;
        }
        let active = self.view.active();
        let at = list
            .iter()
            .position(|f| (f.row, f.col) >= active)
            .unwrap_or(0);
        let t = |k: &str, d: &str| self.t(k, d);
        let mut f = Form::new(
            &t("sheetFn.errTitle", "Error Checking"),
            (680.0, 286.0),
            vec![],
        );
        f.add(
            "cellG",
            None,
            [18.0, 47.0, 377.0, 50.0],
            Kind::Group(String::new()),
        );
        f.add(
            "formula",
            None,
            [36.0, 66.0, 350.0, 22.0],
            Kind::Label(String::new()),
        );
        f.add(
            "ruleG",
            None,
            [18.0, 110.0, 377.0, 110.0],
            Kind::Group(String::new()),
        );
        f.add(
            "desc",
            None,
            [28.0, 130.0, 355.0, 80.0],
            Kind::Note(String::new()),
        );
        let buttons = [
            ("help", "sheetFn.errHelp", "Help on this Error", 57.0),
            (
                "steps",
                "sheetFn.errSteps",
                "Show Calculation Steps...",
                96.0,
            ),
            ("ignore", "sheetFn.errIgnore", "Ignore Error", 135.0),
            ("edit", "sheetFn.errEdit", "Edit in Formula Bar", 174.0),
        ];
        for (id, key, d, y) in buttons {
            f.add(
                id,
                None,
                [411.0, y, 249.0, 28.0],
                Kind::Button {
                    label: t(key, d),
                    default: id == "help",
                },
            );
        }
        // No per-error help pages in BlueSheet yet.
        f.enable("help", false);
        let bottom = [
            ("options", "sheetFn.errOptions", "Options...", 18.0),
            ("prev", "sheetFn.errPrev", "Previous", 422.0),
            ("next", "sheetFn.errNext", "Next", 549.0),
        ];
        for (id, key, d, x) in bottom {
            f.add(
                id,
                None,
                [x, 238.0, 111.0, 28.0],
                Kind::Button {
                    label: t(key, d),
                    default: false,
                },
            );
        }
        f.set_access("steps", 5);
        f.set_access("ignore", 0);
        f.set_access("edit", 8);
        f.set_access("options", 0);
        f.set_access("prev", 0);
        f.set_access("next", 0);
        self.form = Some(f);
        self.form_kind = FormKind::ErrorCheck;
        self.errc = Some(ErrCheck {
            sheet: self.view.sheet(),
            list,
            at,
        });
        self.err_show();
        true
    }

    /// Show the current finding: select its cell, fill the texts.
    fn err_show(&mut self) {
        let Some(st) = self.errc.as_ref() else {
            return;
        };
        let Some(fd) = st.list.get(st.at).cloned() else {
            return;
        };
        let first = st.at == 0;
        let last = st.at + 1 >= st.list.len();
        self.view
            .select((fd.row, fd.col), (fd.row, fd.col), (fd.row, fd.col));
        let d = self
            .view
            .engine()
            .cell(self.view.sheet(), fd.row, fd.col)
            .ok();
        let input = d.as_ref().map(|d| d.input.clone()).unwrap_or_default();
        let shown = d.map(|d| d.display).unwrap_or_default();
        let key = match (fd.rule.as_str(), shown.as_str()) {
            ("evalError", "#DIV/0!") => "div0",
            ("evalError", "#N/A") => "na",
            ("evalError", "#REF!") => "ref",
            ("evalError", "#NAME?") => "name",
            (r, _) => r,
        }
        .to_owned();
        let cell = format!("{}{}", col_to_letters(fd.col), fd.row + 1);
        let head = self
            .t("sheetFn.errIn", "Error in cell {cell}")
            .replace("{cell}", &cell);
        let title = self.t(&format!("sheetFn.rule_{key}"), &fd.rule);
        let desc = self.t(&format!("sheetFn.rule_{key}D"), "");
        if let Some(f) = self.form.as_mut() {
            for (id, text) in [("cellG", head), ("ruleG", title)] {
                if let Some(Kind::Group(s)) = f.get_mut(id).map(|c| &mut c.kind) {
                    *s = text;
                }
            }
            if let Some(Kind::Label(s)) = f.get_mut("formula").map(|c| &mut c.kind) {
                *s = input;
            }
            if let Some(Kind::Note(s)) = f.get_mut("desc").map(|c| &mut c.kind) {
                *s = desc;
            }
            f.enable("prev", !first);
            f.enable("next", !last);
        }
    }

    /// Events of Error Checking.
    pub(super) fn errcheck_event(&mut self, ev: &Event, text: &dyn crate::TextMeasure) {
        match ev {
            Event::Cancel | Event::Ok => {
                self.form = None;
                self.errc = None;
            }
            Event::Clicked("next" | "prev") => {
                let fwd = *ev == Event::Clicked("next");
                if let Some(st) = self.errc.as_mut() {
                    if fwd && st.at + 1 < st.list.len() {
                        st.at += 1;
                    } else if !fwd && st.at > 0 {
                        st.at -= 1;
                    }
                }
                self.err_show();
            }
            Event::Clicked("ignore") => {
                let Some(st) = self.errc.clone() else {
                    return;
                };
                if let Some(fd) = st.list.get(st.at) {
                    let _ = self.view.query(json!({
                        "cmd": "ignoreError", "sheet": st.sheet, "row": fd.row, "col": fd.col,
                        "rule": fd.rule,
                    }));
                    self.view.changed();
                }
                // The rest of the sheet's findings, from the same place.
                let list = self.err_findings();
                if list.is_empty() {
                    self.form = None;
                    self.errc = None;
                    self.status = self.t(
                        "sheetFn.errDone",
                        "The error check is complete for the entire sheet.",
                    );
                    return;
                }
                let at = st.at.min(list.len() - 1);
                self.errc = Some(ErrCheck { list, at, ..st });
                self.err_show();
            }
            Event::Clicked("edit") => {
                self.form = None;
                self.errc = None;
                self.view.edit_cell(text);
            }
            Event::Clicked("steps") => {
                self.form = None;
                self.errc = None;
                self.open_evaluate_formula();
            }
            Event::Clicked("options") => {
                self.form = None;
                self.errc = None;
                self.open_options();
                self.options_page = 1;
                if let Some(f) = self.form.as_mut() {
                    f.set_page(1);
                }
            }
            _ => {}
        }
    }
}
