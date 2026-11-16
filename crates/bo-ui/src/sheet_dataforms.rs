//! Data and Formulas dialogs of BlueSheet on the self-drawn form (`sheet_form.rs`): Sort
//! (several levels), Data Validation (three pages), Goal Seek, Name Manager and Insert
//! Function (categories, search and argument help). Engine commands do the work
//! (`sort`, `sortRegion`, `setValidation`, `goalSeek`, `nameList` / `defineName` /
//! `deleteName`, `functionList`).
use super::form_impl::{Event, Form, FormKind, Kind};
use super::SheetWindow;
use crate::sheet_ribbon::FUNCTION_CATEGORIES;
use bo_model::sheet::col_to_letters;
use serde_json::{json, Value};

/// Sort levels shown at most (ids of the level label, column, sort-on and order boxes).
const LEVELS: [(&str, &str, &str, &str); 5] = [
    ("lv0", "col0", "on0", "ord0"),
    ("lv1", "col1", "on1", "ord1"),
    ("lv2", "col2", "on2", "ord2"),
    ("lv3", "col3", "on3", "ord3"),
    ("lv4", "col4", "on4", "ord4"),
];

/// Argument names of common functions for the Insert Function help (BlueOffice's own
/// wording, as the web edition's formula hint; `[x]` optional, `…` repeats).
const FUNCTION_ARGS: [(&str, &str); 58] = [
    ("SUM", "number1, [number2], …"),
    ("AVERAGE", "number1, [number2], …"),
    ("COUNT", "value1, [value2], …"),
    ("COUNTA", "value1, [value2], …"),
    ("COUNTBLANK", "range"),
    ("MIN", "number1, [number2], …"),
    ("MAX", "number1, [number2], …"),
    ("PRODUCT", "number1, [number2], …"),
    ("IF", "test, value_if_true, [value_if_false]"),
    ("IFS", "test1, value1, [test2, value2], …"),
    ("IFERROR", "value, value_if_error"),
    ("AND", "test1, [test2], …"),
    ("OR", "test1, [test2], …"),
    ("NOT", "test"),
    ("SUMIF", "range, criteria, [sum_range]"),
    ("SUMIFS", "sum_range, criteria_range1, criteria1, …"),
    ("COUNTIF", "range, criteria"),
    ("COUNTIFS", "criteria_range1, criteria1, …"),
    ("AVERAGEIF", "range, criteria, [average_range]"),
    (
        "VLOOKUP",
        "lookup_value, table, column_index, [approximate]",
    ),
    ("HLOOKUP", "lookup_value, table, row_index, [approximate]"),
    (
        "XLOOKUP",
        "lookup_value, lookup_array, return_array, [if_not_found], [match_mode], [search_mode]",
    ),
    ("INDEX", "array, row, [column]"),
    ("MATCH", "lookup_value, lookup_array, [match_type]"),
    ("ROUND", "number, digits"),
    ("ROUNDUP", "number, digits"),
    ("ROUNDDOWN", "number, digits"),
    ("INT", "number"),
    ("ABS", "number"),
    ("MOD", "number, divisor"),
    ("LEFT", "text, [count]"),
    ("RIGHT", "text, [count]"),
    ("MID", "text, start, count"),
    ("LEN", "text"),
    ("TRIM", "text"),
    ("UPPER", "text"),
    ("LOWER", "text"),
    ("PROPER", "text"),
    ("CONCAT", "text1, [text2], …"),
    ("CONCATENATE", "text1, [text2], …"),
    ("TEXT", "value, format"),
    ("VALUE", "text"),
    ("DATE", "year, month, day"),
    ("YEAR", "date"),
    ("MONTH", "date"),
    ("DAY", "date"),
    ("TODAY", ""),
    ("NOW", ""),
    ("DATEDIF", "start_date, end_date, unit"),
    ("NETWORKDAYS", "start_date, end_date, [holidays]"),
    ("EDATE", "start_date, months"),
    ("EOMONTH", "start_date, months"),
    ("WEEKDAY", "date, [type]"),
    ("UNIQUE", "array, [by_column], [exactly_once]"),
    ("FILTER", "array, include, [if_empty]"),
    ("SORT", "array, [sort_index], [order], [by_column]"),
    ("SUBTOTAL", "function_number, ref1, [ref2], …"),
    ("PMT", "rate, nper, pv, [fv], [type]"),
];

const ALLOW: [(&str, &str, &str); 8] = [
    ("sheetDv.any", "Any value", "none"),
    ("sheetDv.whole", "Whole number", "whole"),
    ("sheetDv.decimal", "Decimal", "decimal"),
    ("sheetDv.list", "List", "list"),
    ("sheetDv.date", "Date", "date"),
    ("sheetDv.time", "Time", "time"),
    ("sheetDv.textLength", "Text length", "textLength"),
    ("sheetDv.custom", "Custom", "custom"),
];

const OPERATORS: [(&str, &str, &str); 8] = [
    ("sheetDv.between", "between", "between"),
    ("sheetDv.notBetween", "not between", "notBetween"),
    ("sheetDv.equal", "equal to", "equal"),
    ("sheetDv.notEqual", "not equal to", "notEqual"),
    ("sheetDv.greater", "greater than", "greaterThan"),
    ("sheetDv.less", "less than", "lessThan"),
    (
        "sheetDv.greaterEqual",
        "greater than or equal to",
        "greaterThanOrEqual",
    ),
    (
        "sheetDv.lessEqual",
        "less than or equal to",
        "lessThanOrEqual",
    ),
];

impl SheetWindow {
    fn edit(value: &str) -> Kind {
        Kind::Edit {
            value: value.chars().collect(),
            caret: value.chars().count(),
        }
    }

    fn button(label: String) -> Kind {
        Kind::Button {
            label,
            default: false,
        }
    }

    // ---------------------------------------------------------------- Sort

    /// Open the Sort dialog over the selection's region.
    pub(super) fn open_sort_dialog(&mut self) -> bool {
        if self.view.editing() {
            self.view.commit();
        }
        let sheet = self.view.sheet();
        let range = self.view.rng();
        let region = self
            .view
            .query(json!({ "cmd": "sortRegion", "sheet": sheet, "range": range }))
            .unwrap_or(Value::Null);
        let r = region.get("range").cloned().unwrap_or(range);
        let header = region
            .get("hasHeader")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let t = |k: &str, d: &str| self.t(k, d);
        // Geometry measured on the reference's Sort dialog at 125 % (round 39, `sheet-31`):
        // 767 × 344 px, a row of 34 px buttons at y 48, the level grid from y 89, combo
        // boxes 28 px high, OK / Cancel 106 × 33 at y 291.
        let mut f = Form::new(&t("sheetSort.title", "Sort"), (767.0, 344.0), Vec::new());
        // The level grid: white, framed #828790, a header row with #E5E5E5 separators.
        f.add("grid", None, [16.0, 88.0, 735.0, 197.0], Kind::Canvas);
        let bx = [
            ("add", "sheetSort.add", "Add Level", 18.0, 114.0),
            ("del", "sheetSort.delete", "Delete Level", 138.0, 130.0),
            ("copy", "sheetSort.copy", "Copy Level", 274.0, 120.0),
        ];
        for (id, k, d, x, w) in bx {
            f.add(id, None, [x, 48.0, w, 34.0], Self::button(t(k, d)));
        }
        f.add(
            "header",
            None,
            [582.0, 55.0, 175.0, 20.0],
            Kind::Check {
                label: t("sheetSort.headers", "My data has headers"),
                on: header,
            },
        );
        f.add(
            "hCol",
            None,
            [23.0, 96.0, 0.0, 0.0],
            Kind::Label(t("sheetSort.column", "Column")),
        );
        f.add(
            "hOn",
            None,
            [289.0, 96.0, 0.0, 0.0],
            Kind::Label(t("sheetSort.sortOn", "Sort On")),
        );
        f.add(
            "hOrd",
            None,
            [555.0, 96.0, 0.0, 0.0],
            Kind::Label(t("sheetSort.order", "Order")),
        );
        let orders = vec![t("sheetSort.az", "A to Z"), t("sheetSort.za", "Z to A")];
        for (i, (lv, col, on, ord)) in LEVELS.iter().enumerate() {
            let y = 121.0 + i as f32 * 31.0;
            let label = if i == 0 {
                t("sheetSort.sortBy", "Sort by")
            } else {
                t("sheetSort.thenBy", "Then by")
            };
            f.add(lv, None, [20.0, y + 4.0, 0.0, 0.0], Kind::Label(label));
            f.add(
                col,
                None,
                [90.0, y, 178.0, 28.0],
                Kind::Combo {
                    items: Vec::new(),
                    sel: 0,
                },
            );
            f.add(
                on,
                None,
                [285.0, y, 250.0, 28.0],
                Kind::Combo {
                    items: vec![t("sheetSort.cellValues", "Cell Values")],
                    sel: 0,
                },
            );
            f.add(
                ord,
                None,
                [551.0, y, 196.0, 28.0],
                Kind::Combo {
                    items: orders.clone(),
                    sel: 0,
                },
            );
        }
        f.ok_cancel(&t("common.ok", "OK"), &t("common.cancel", "Cancel"));
        if let Some(c) = f.get_mut("ok") {
            c.rect = [522.0, 291.0, 106.0, 33.0];
        }
        if let Some(c) = f.get_mut("cancel") {
            c.rect = [645.0, 291.0, 106.0, 33.0];
        }
        self.sort_range = Some(r);
        self.sort_levels = 1;
        self.form = Some(f);
        self.form_kind = FormKind::Sort;
        self.sort_refresh();
        true
    }

    /// Column names of the sort range (header texts or "Column X") and level visibility.
    fn sort_refresh(&mut self) {
        let Some(r) = self.sort_range.clone() else {
            return;
        };
        let get = |k: &str| r.get(k).and_then(Value::as_u64).unwrap_or(0) as u32;
        let (r0, c0, c1) = (get("r0"), get("c0"), get("c1"));
        let header = self.form.as_ref().is_some_and(|f| f.on("header"));
        let column = self.t("sheetSort.columnN", "Column");
        let names: Vec<String> = (c0..=c1.min(c0 + 200))
            .map(|c| {
                let label = if header {
                    self.view
                        .engine()
                        .cell(self.view.sheet(), r0, c)
                        .map(|d| d.display)
                        .unwrap_or_default()
                } else {
                    String::new()
                };
                if label.trim().is_empty() {
                    format!("{column} {}", col_to_letters(c))
                } else {
                    label
                }
            })
            .collect();
        let levels = self.sort_levels;
        let Some(f) = self.form.as_mut() else {
            return;
        };
        for (i, (lv, col, on, ord)) in LEVELS.iter().enumerate() {
            let shown = i < levels;
            for id in [lv, col, on, ord] {
                f.show(id, shown);
            }
            let same = matches!(f.get(col).map(|c| &c.kind), Some(Kind::Combo { items, .. }) if *items == names);
            if !same {
                let sel = f.sel(col).unwrap_or(0).min(names.len().saturating_sub(1));
                f.set_items(col, names.clone(), Some(sel));
            }
        }
        f.enable("del", levels > 1);
        f.enable("add", levels < LEVELS.len());
        f.enable("copy", levels < LEVELS.len());
    }

    fn sort_event(&mut self, ev: &Event) {
        match ev {
            Event::Clicked("add") if self.sort_levels < LEVELS.len() => {
                self.sort_levels += 1;
                self.sort_refresh();
            }
            Event::Clicked("copy") if self.sort_levels < LEVELS.len() => {
                let n = self.sort_levels;
                if let Some(f) = self.form.as_mut() {
                    let (_, pc, _, po) = LEVELS[n - 1];
                    let (_, nc, _, no) = LEVELS[n];
                    let (c, o) = (f.sel(pc), f.sel(po));
                    f.set_sel(nc, c);
                    f.set_sel(no, o);
                }
                self.sort_levels += 1;
                self.sort_refresh();
            }
            Event::Clicked("del") if self.sort_levels > 1 => {
                self.sort_levels -= 1;
                self.sort_refresh();
            }
            Event::Changed("header") => self.sort_refresh(),
            Event::Ok => {
                let Some(r) = self.sort_range.clone() else {
                    return;
                };
                let c0 = r.get("c0").and_then(Value::as_u64).unwrap_or(0) as u32;
                let keys: Vec<Value> = LEVELS
                    .iter()
                    .take(self.sort_levels)
                    .filter_map(|(_, col, _, ord)| {
                        let f = self.form.as_ref()?;
                        Some(json!({
                            "col": c0 + f.sel(col).unwrap_or(0) as u32,
                            "descending": f.sel(ord) == Some(1),
                        }))
                    })
                    .collect();
                let header = self.form.as_ref().is_some_and(|f| f.on("header"));
                let sheet = self.view.sheet();
                self.view.run(json!({
                    "cmd": "sort", "sheet": sheet, "range": r, "keys": keys, "hasHeader": header,
                }));
                self.form = None;
            }
            Event::Cancel => self.form = None,
            _ => {}
        }
    }

    // ---------------------------------------------------------------- Data Validation

    /// Open Data Validation for the selection (the active cell's rule filled in).
    pub(super) fn open_validation_dialog(&mut self) -> bool {
        if self.view.editing() {
            self.view.commit();
        }
        let (r, c) = self.view.active();
        let sheet = self.view.sheet();
        let now = self
            .view
            .query(json!({ "cmd": "validation", "sheet": sheet, "row": r, "col": c }))
            .unwrap_or(Value::Null);
        let s = |k: &str| now.get(k).and_then(Value::as_str).unwrap_or("").to_owned();
        let t = |k: &str, d: &str| self.t(k, d);
        let tabs = vec![
            t("sheetDv.settings", "Settings"),
            t("sheetDv.inputMessage", "Input Message"),
            t("sheetDv.errorAlert", "Error Alert"),
        ];
        let mut f = Form::new(&t("sheetDv.title", "Data Validation"), (560.0, 470.0), tabs);
        let p = Some(0);
        f.add(
            "crit",
            p,
            [32.0, 88.0, 0.0, 0.0],
            Kind::Label(t("sheetDv.criteria", "Validation criteria")),
        );
        f.add(
            "allowL",
            p,
            [46.0, 114.0, 0.0, 0.0],
            Kind::Label(t("sheetDv.allow", "Allow:")),
        );
        let kind = s("kind");
        f.add(
            "allow",
            p,
            [46.0, 134.0, 200.0, 26.0],
            Kind::Combo {
                items: ALLOW.iter().map(|(k, d, _)| t(k, d)).collect(),
                sel: ALLOW.iter().position(|(_, _, v)| *v == kind).unwrap_or(0),
            },
        );
        f.add(
            "blank",
            p,
            [280.0, 138.0, 200.0, 20.0],
            Kind::Check {
                label: t("sheetDv.ignoreBlank", "Ignore blank"),
                on: now
                    .get("allowBlank")
                    .and_then(Value::as_bool)
                    .unwrap_or(true),
            },
        );
        f.add(
            "dataL",
            p,
            [46.0, 170.0, 0.0, 0.0],
            Kind::Label(t("sheetDv.data", "Data:")),
        );
        let op = s("operator");
        f.add(
            "op",
            p,
            [46.0, 190.0, 200.0, 26.0],
            Kind::Combo {
                items: OPERATORS.iter().map(|(k, d, _)| t(k, d)).collect(),
                sel: OPERATORS.iter().position(|(_, _, v)| *v == op).unwrap_or(0),
            },
        );
        f.add(
            "f1L",
            p,
            [46.0, 226.0, 0.0, 0.0],
            Kind::Label(t("sheetDv.minimum", "Minimum:")),
        );
        f.add(
            "f1",
            p,
            [46.0, 246.0, 460.0, 24.0],
            Self::edit(s("formula1").trim_start_matches('=')),
        );
        f.add(
            "f2L",
            p,
            [46.0, 280.0, 0.0, 0.0],
            Kind::Label(t("sheetDv.maximum", "Maximum:")),
        );
        f.add(
            "f2",
            p,
            [46.0, 300.0, 460.0, 24.0],
            Self::edit(s("formula2").trim_start_matches('=')),
        );
        let p = Some(1);
        f.add(
            "showIn",
            p,
            [32.0, 92.0, 400.0, 20.0],
            Kind::Check {
                label: t(
                    "sheetDv.showInput",
                    "Show input message when cell is selected",
                ),
                on: true,
            },
        );
        f.add(
            "inTitleL",
            p,
            [46.0, 130.0, 0.0, 0.0],
            Kind::Label(t("sheetDv.titleLabel", "Title:")),
        );
        f.add(
            "inTitle",
            p,
            [46.0, 150.0, 300.0, 24.0],
            Self::edit(&s("promptTitle")),
        );
        f.add(
            "inMsgL",
            p,
            [46.0, 184.0, 0.0, 0.0],
            Kind::Label(t("sheetDv.message", "Input message:")),
        );
        f.add(
            "inMsg",
            p,
            [46.0, 204.0, 460.0, 24.0],
            Self::edit(&s("prompt")),
        );
        let p = Some(2);
        f.add(
            "showErr",
            p,
            [32.0, 92.0, 400.0, 20.0],
            Kind::Check {
                label: t(
                    "sheetDv.showError",
                    "Show error alert after invalid data is entered",
                ),
                on: true,
            },
        );
        f.add(
            "styleL",
            p,
            [46.0, 130.0, 0.0, 0.0],
            Kind::Label(t("sheetDv.style", "Style:")),
        );
        let style = s("errorStyle");
        f.add(
            "estyle",
            p,
            [46.0, 150.0, 160.0, 26.0],
            Kind::Combo {
                items: vec![
                    t("sheetDv.stop", "Stop"),
                    t("sheetDv.warning", "Warning"),
                    t("sheetDv.information", "Information"),
                ],
                sel: ["stop", "warning", "information"]
                    .iter()
                    .position(|v| *v == style)
                    .unwrap_or(0),
            },
        );
        f.add(
            "eTitleL",
            p,
            [230.0, 130.0, 0.0, 0.0],
            Kind::Label(t("sheetDv.titleLabel", "Title:")),
        );
        f.add(
            "eTitle",
            p,
            [230.0, 150.0, 276.0, 24.0],
            Self::edit(&s("errorTitle")),
        );
        f.add(
            "eMsgL",
            p,
            [230.0, 184.0, 0.0, 0.0],
            Kind::Label(t("sheetDv.errorMessage", "Error message:")),
        );
        f.add(
            "eMsg",
            p,
            [230.0, 204.0, 276.0, 24.0],
            Self::edit(&s("error")),
        );
        f.add(
            "clear",
            None,
            [20.0, 470.0 - 48.0, 112.0, 28.0],
            Self::button(t("sheetDv.clearAll", "Clear All")),
        );
        f.ok_cancel(&t("common.ok", "OK"), &t("common.cancel", "Cancel"));
        self.form = Some(f);
        self.form_kind = FormKind::Validation;
        self.validation_refresh();
        true
    }

    fn validation_refresh(&mut self) {
        let labels = (
            self.t("sheetDv.minimum", "Minimum:"),
            self.t("sheetDv.source", "Source:"),
            self.t("sheetDv.formula", "Formula:"),
            self.t("sheetDv.value", "Value:"),
        );
        let Some(f) = self.form.as_mut() else {
            return;
        };
        let allow = f.sel("allow").unwrap_or(0);
        let op = f.sel("op").unwrap_or(0);
        let ranged = matches!(allow, 1 | 2 | 4 | 5 | 6);
        f.show("dataL", ranged);
        f.show("op", ranged);
        f.show("f1L", allow > 0);
        f.show("f1", allow > 0);
        let two = ranged && op < 2;
        f.show("f2L", two);
        f.show("f2", two);
        let first = match allow {
            3 => labels.1,
            7 => labels.2,
            _ if two => labels.0,
            _ => labels.3,
        };
        if let Some(Kind::Label(s)) = f.get_mut("f1L").map(|c| &mut c.kind) {
            *s = first;
        }
    }

    fn validation_event(&mut self, ev: &Event) {
        match ev {
            Event::Changed("allow" | "op") => self.validation_refresh(),
            Event::Clicked("clear") => {
                if let Some(f) = self.form.as_mut() {
                    f.set_sel("allow", Some(0));
                    for id in ["f1", "f2", "inTitle", "inMsg", "eTitle", "eMsg"] {
                        f.set_text(id, "");
                    }
                }
                self.validation_refresh();
            }
            Event::Ok => {
                let Some(f) = self.form.as_ref() else {
                    return;
                };
                let allow = ALLOW[f.sel("allow").unwrap_or(0).min(7)].2;
                let op = OPERATORS[f.sel("op").unwrap_or(0).min(7)].2;
                let opt = |id: &str| {
                    let v = f.text(id);
                    (!v.trim().is_empty()).then_some(v)
                };
                let mut spec = json!({
                    "kind": allow,
                    "formula1": f.text("f1"),
                    "allowBlank": f.on("blank"),
                });
                if matches!(allow, "whole" | "decimal" | "date" | "time" | "textLength") {
                    spec["operator"] = json!(op);
                    if let Some(v) = opt("f2").filter(|_| op == "between" || op == "notBetween") {
                        spec["formula2"] = json!(v);
                    }
                }
                if f.on("showIn") {
                    if let Some(v) = opt("inMsg") {
                        spec["prompt"] = json!(v);
                    }
                }
                if f.on("showErr") {
                    if let Some(v) = opt("eTitle") {
                        spec["errorTitle"] = json!(v);
                    }
                    if let Some(v) = opt("eMsg") {
                        spec["error"] = json!(v);
                    }
                    spec["errorStyle"] = json!(
                        ["stop", "warning", "information"][f.sel("estyle").unwrap_or(0).min(2)]
                    );
                }
                let sheet = self.view.sheet();
                let range = self.view.rng();
                self.view.run(
                    json!({ "cmd": "setValidation", "sheet": sheet, "range": range, "spec": spec }),
                );
                self.form = None;
            }
            Event::Cancel => self.form = None,
            _ => {}
        }
    }

    // ---------------------------------------------------------------- Goal Seek

    /// Open Goal Seek (What-If Analysis) with the active cell as the formula cell.
    pub(super) fn open_goal_seek(&mut self) -> bool {
        if self.view.editing() {
            self.view.commit();
        }
        let (r, c) = self.view.active();
        let here = format!("{}{}", col_to_letters(c), r + 1);
        let t = |k: &str, d: &str| self.t(k, d);
        let mut f = Form::new(
            &t("sheetGoal.title", "Goal Seek"),
            (420.0, 250.0),
            Vec::new(),
        );
        for (i, (id, k, d, v)) in [
            ("set", "sheetGoal.setCell", "Set cell:", here.as_str()),
            ("to", "sheetGoal.toValue", "To value:", ""),
            ("by", "sheetGoal.byChanging", "By changing cell:", ""),
        ]
        .iter()
        .enumerate()
        {
            let y = 56.0 + i as f32 * 40.0;
            let lid = ["setL", "toL", "byL"][i];
            f.add(lid, None, [20.0, y + 4.0, 0.0, 0.0], Kind::Label(t(k, d)));
            f.add(id, None, [170.0, y, 230.0, 26.0], Self::edit(v));
        }
        f.add(
            "status",
            None,
            [20.0, 178.0, 0.0, 0.0],
            Kind::Note(String::new()),
        );
        f.ok_cancel(&t("common.ok", "OK"), &t("common.cancel", "Cancel"));
        f.focus("to");
        self.form = Some(f);
        self.form_kind = FormKind::GoalSeek;
        true
    }

    fn goal_event(&mut self, ev: &Event) {
        match ev {
            Event::Ok => {
                let Some(f) = self.form.as_ref() else {
                    return;
                };
                let goal = f.text("to").trim().replace(',', ".").parse::<f64>();
                let (target, changing) = (f.text("set"), f.text("by"));
                let Ok(goal) = goal else {
                    let msg = self.t("sheetGoal.needNumber", "To value must contain a number.");
                    self.goal_status(msg);
                    return;
                };
                let sheet = self.view.sheet();
                let out = self.view.query(json!({
                    "cmd": "goalSeek", "sheet": sheet, "target": target.trim(), "goal": goal,
                    "changing": changing.trim(), "apply": true,
                }));
                self.view.changed();
                let found = out
                    .as_ref()
                    .and_then(|o| o.get("found"))
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                if found {
                    self.form = None;
                } else {
                    let msg = self.t(
                        "sheetGoal.notFound",
                        "Goal Seek may not have found a solution.",
                    );
                    self.goal_status(msg);
                }
            }
            Event::Cancel => self.form = None,
            _ => {}
        }
    }

    fn goal_status(&mut self, msg: String) {
        if let Some(Kind::Note(s)) = self
            .form
            .as_mut()
            .and_then(|f| f.get_mut("status"))
            .map(|c| &mut c.kind)
        {
            *s = msg;
        }
    }

    // ---------------------------------------------------------------- Name Manager

    /// Open the Name Manager (defined names, add / change / delete).
    pub(super) fn open_name_manager(&mut self) -> bool {
        if self.view.editing() {
            self.view.commit();
        }
        let t = |k: &str, d: &str| self.t(k, d);
        let mut f = Form::new(
            &t("sheetNames.title", "Name Manager"),
            (760.0, 500.0),
            Vec::new(),
        );
        f.add(
            "new",
            None,
            [20.0, 50.0, 100.0, 28.0],
            Self::button(t("sheetNames.new", "New...")),
        );
        f.add(
            "apply",
            None,
            [130.0, 50.0, 100.0, 28.0],
            Self::button(t("sheetNames.edit", "Edit...")),
        );
        f.add(
            "del",
            None,
            [240.0, 50.0, 100.0, 28.0],
            Self::button(t("sheetNames.delete", "Delete")),
        );
        f.add(
            "hName",
            None,
            [24.0, 90.0, 0.0, 0.0],
            Kind::Label(t("sheetNames.name", "Name")),
        );
        f.add(
            "hRef",
            None,
            [260.0, 90.0, 0.0, 0.0],
            Kind::Label(t("sheetNames.refersTo", "Refers To")),
        );
        f.add(
            "names",
            None,
            [20.0, 110.0, 720.0, 240.0],
            Kind::List {
                items: Vec::new(),
                sel: None,
                top: 0,
            },
        );
        f.add(
            "nameL",
            None,
            [20.0, 364.0, 0.0, 0.0],
            Kind::Label(t("sheetNames.nameLabel", "Name:")),
        );
        f.add("name", None, [120.0, 360.0, 300.0, 24.0], Self::edit(""));
        f.add(
            "refL",
            None,
            [20.0, 398.0, 0.0, 0.0],
            Kind::Label(t("sheetNames.refersToLabel", "Refers to:")),
        );
        let rng = {
            let (r0, c0, r1, c1) = self.view.range_bounds();
            let sheet = self
                .view
                .sheet_names()
                .get(self.view.sheet())
                .cloned()
                .flatten()
                .unwrap_or_default();
            let a = |r: u32, c: u32| format!("${}${}", col_to_letters(c), r + 1);
            if (r0, c0) == (r1, c1) {
                format!("={sheet}!{}", a(r0, c0))
            } else {
                format!("={sheet}!{}:{}", a(r0, c0), a(r1, c1))
            }
        };
        f.add("ref", None, [120.0, 394.0, 620.0, 24.0], Self::edit(&rng));
        f.add(
            "err",
            None,
            [20.0, 430.0, 0.0, 0.0],
            Kind::Note(String::new()),
        );
        let close = t("sheetNames.close", "Close");
        f.add(
            "cancel",
            None,
            [760.0 - 131.0, 500.0 - 48.0, 112.0, 28.0],
            Kind::Button {
                label: close,
                default: true,
            },
        );
        self.form = Some(f);
        self.form_kind = FormKind::Names;
        self.names_refresh();
        true
    }

    fn names_refresh(&mut self) {
        let list = self
            .view
            .query(json!({ "cmd": "nameList" }))
            .and_then(|v| v.as_array().cloned())
            .unwrap_or_default();
        let rows: Vec<(String, String)> = list
            .iter()
            .map(|n| {
                (
                    n.get("name")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_owned(),
                    n.get("refersTo")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_owned(),
                )
            })
            .collect();
        self.name_rows.clone_from(&rows);
        if let Some(f) = self.form.as_mut() {
            let items = rows
                .iter()
                .map(|(n, r)| format!("{n:<28}\t{r}"))
                .map(|s| s.replace('\t', "    "))
                .collect();
            f.set_items("names", items, None);
        }
    }

    fn names_event(&mut self, ev: &Event) {
        let set_err = |w: &mut Self, s: String| {
            if let Some(Kind::Note(n)) = w
                .form
                .as_mut()
                .and_then(|f| f.get_mut("err"))
                .map(|c| &mut c.kind)
            {
                *n = s;
            }
        };
        match ev {
            Event::Changed("names") => {
                let i = self.form.as_ref().and_then(|f| f.sel("names"));
                if let Some((n, r)) = i.and_then(|i| self.name_rows.get(i).cloned()) {
                    if let Some(f) = self.form.as_mut() {
                        f.set_text("name", &n);
                        f.set_text("ref", &r);
                    }
                }
            }
            Event::Clicked("new" | "apply") => {
                let Some(f) = self.form.as_ref() else {
                    return;
                };
                let (name, refers) = (f.text("name").trim().to_owned(), f.text("ref"));
                let old = matches!(ev, Event::Clicked("apply"))
                    .then(|| f.sel("names").and_then(|i| self.name_rows.get(i)))
                    .flatten()
                    .map(|(n, _)| n.clone());
                let mut cmd = json!({ "cmd": "defineName", "name": name, "refersTo": refers });
                if let Some(o) = old {
                    cmd["old"] = json!({ "name": o });
                }
                let ok = self.view.query(cmd).is_some();
                self.view.changed();
                let msg = if ok {
                    String::new()
                } else {
                    self.view
                        .take_error()
                        .unwrap_or_else(|| self.t("sheetNames.invalid", "The name is not valid."))
                };
                set_err(self, msg);
                self.names_refresh();
            }
            Event::Clicked("del") => {
                let i = self.form.as_ref().and_then(|f| f.sel("names"));
                if let Some((n, _)) = i.and_then(|i| self.name_rows.get(i).cloned()) {
                    self.view.query(json!({ "cmd": "deleteName", "name": n }));
                    self.view.changed();
                    self.names_refresh();
                }
            }
            Event::Ok | Event::Cancel => self.form = None,
            _ => {}
        }
    }

    // ---------------------------------------------------------------- Insert Function

    /// Open Insert Function: search, category, function list and its arguments.
    pub(super) fn open_insert_function(&mut self) -> bool {
        let t = |k: &str, d: &str| self.t(k, d);
        let mut f = Form::new(
            &t("sheetFn.title", "Insert Function"),
            (560.0, 520.0),
            Vec::new(),
        );
        f.add(
            "searchL",
            None,
            [20.0, 50.0, 0.0, 0.0],
            Kind::Label(t("sheetFn.search", "Search for a function:")),
        );
        f.add("search", None, [20.0, 70.0, 420.0, 24.0], Self::edit(""));
        f.add(
            "go",
            None,
            [452.0, 68.0, 88.0, 28.0],
            Self::button(t("sheetFn.go", "Go")),
        );
        f.add(
            "catL",
            None,
            [20.0, 110.0, 0.0, 0.0],
            Kind::Label(t("sheetFn.category", "Or select a category:")),
        );
        let mut cats = vec![t("sheetFn.all", "All")];
        cats.extend(FUNCTION_CATEGORIES.iter().map(|(id, _)| {
            let key = format!("commands.{id}");
            t(&key, id.trim_start_matches("sheet.fn"))
        }));
        f.add(
            "cat",
            None,
            [200.0, 106.0, 240.0, 26.0],
            Kind::Combo {
                items: cats,
                sel: 0,
            },
        );
        f.add(
            "fnL",
            None,
            [20.0, 144.0, 0.0, 0.0],
            Kind::Label(t("sheetFn.select", "Select a function:")),
        );
        f.add(
            "fns",
            None,
            [20.0, 164.0, 520.0, 208.0],
            Kind::List {
                items: Vec::new(),
                sel: None,
                top: 0,
            },
        );
        f.add(
            "sig",
            None,
            [20.0, 384.0, 0.0, 0.0],
            Kind::Label(String::new()),
        );
        f.add(
            "help",
            None,
            [20.0, 408.0, 0.0, 0.0],
            Kind::Note(String::new()),
        );
        f.ok_cancel(&t("common.ok", "OK"), &t("common.cancel", "Cancel"));
        self.form = Some(f);
        self.form_kind = FormKind::Function;
        self.function_refresh();
        if let Some(f) = self.form.as_mut() {
            f.focus("search");
        }
        true
    }

    fn function_refresh(&mut self) {
        let Some(f) = self.form.as_ref() else {
            return;
        };
        let cat = f.sel("cat").unwrap_or(0);
        let query = f.text("search").trim().to_ascii_uppercase();
        let mut names: Vec<String> = if cat == 0 {
            self.functions.clone()
        } else {
            FUNCTION_CATEGORIES
                .get(cat - 1)
                .map(|(_, n)| n.iter().map(|s| (*s).to_owned()).collect())
                .unwrap_or_default()
        };
        if !query.is_empty() {
            names.retain(|n| n.contains(&query));
        }
        let pick = names
            .iter()
            .position(|n| *n == query)
            .or((!names.is_empty()).then_some(0));
        if let Some(f) = self.form.as_mut() {
            f.set_items("fns", names, pick);
        }
        self.function_help();
    }

    fn function_help(&mut self) {
        let name = self
            .form
            .as_ref()
            .map(|f| f.text("fns"))
            .unwrap_or_default();
        let args = FUNCTION_ARGS
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, a)| (*a).to_owned());
        let sig = if name.is_empty() {
            String::new()
        } else {
            format!("{name}({})", args.clone().unwrap_or_else(|| "…".into()))
        };
        let help = if name.is_empty() {
            String::new()
        } else if args.is_some() {
            self.t(
                "sheetFn.helpArgs",
                "Arguments in brackets are optional; … repeats the last one.",
            )
        } else {
            self.t(
                "sheetFn.helpNone",
                "Choose OK to type the arguments in the formula bar.",
            )
        };
        if let Some(f) = self.form.as_mut() {
            if let Some(Kind::Label(s)) = f.get_mut("sig").map(|c| &mut c.kind) {
                *s = sig;
            }
            if let Some(Kind::Note(s)) = f.get_mut("help").map(|c| &mut c.kind) {
                *s = help;
            }
        }
    }

    fn function_event(&mut self, ev: &Event, text: &dyn crate::TextMeasure) {
        match ev {
            Event::Changed("cat" | "search") | Event::Clicked("go") => self.function_refresh(),
            Event::Changed("fns") => self.function_help(),
            Event::Ok | Event::Clicked("fns") => {
                let name = self
                    .form
                    .as_ref()
                    .map(|f| f.text("fns"))
                    .unwrap_or_default();
                self.form = None;
                if !name.is_empty() {
                    self.view.edit_with(&format!("={name}("), text);
                }
            }
            Event::Cancel => self.form = None,
            _ => {}
        }
    }

    // ---------------------------------------------------------------- Print Titles

    fn print_titles_event(&mut self, ev: &Event) {
        match ev {
            Event::Ok => {
                let text = self
                    .form
                    .as_ref()
                    .map(|f| f.text("rows"))
                    .unwrap_or_default();
                let nums: Vec<u32> = text
                    .split(':')
                    .filter_map(|p| p.trim().trim_start_matches('$').parse::<u32>().ok())
                    .filter(|n| *n >= 1)
                    .collect();
                let rows = match nums.as_slice() {
                    [] => Value::Null,
                    [a] => json!([a - 1, a - 1]),
                    [a, b, ..] => json!([a.min(b) - 1, a.max(b) - 1]),
                };
                let sheet = self.view.sheet();
                if self
                    .view
                    .query(json!({ "cmd": "setPageSetup", "sheet": sheet, "titleRows": rows }))
                    .is_some()
                {
                    self.view.changed();
                }
                self.form = None;
            }
            Event::Cancel => self.form = None,
            _ => {}
        }
    }

    // ---------------------------------------------------------------- dispatch

    /// An event of form `kind`.
    pub(super) fn form_event(
        &mut self,
        kind: FormKind,
        ev: &Event,
        text: &dyn crate::TextMeasure,
    ) -> bool {
        match kind {
            FormKind::Sort => self.sort_event(ev),
            FormKind::Validation => self.validation_event(ev),
            FormKind::GoalSeek => self.goal_event(ev),
            FormKind::Names => self.names_event(ev),
            FormKind::Function => self.function_event(ev, text),
            FormKind::PrintTitles => self.print_titles_event(ev),
            FormKind::CreateTable
            | FormKind::CreatePivot
            | FormKind::Sparklines
            | FormKind::Link
            | FormKind::HeaderFooter
            | FormKind::Symbol => self.insert_form_event(ev, text),
            FormKind::MoreColors => self.more_colors_event(ev),
            FormKind::CellStyle => self.cell_style_event(ev),
            FormKind::Queries
            | FormKind::Consolidate
            | FormKind::WhatIfTable
            | FormKind::CustomViews
            | FormKind::Zoom => self.data_form_event(ev, text),
            FormKind::CustomFilter | FormKind::Top10 => {
                self.filter_form_event(ev);
            }
            FormKind::Help => {
                if matches!(ev, Event::Ok | Event::Cancel) {
                    self.form = None;
                }
            }
            FormKind::ChartData
            | FormKind::ChartSeries
            | FormKind::ChartAxisLabels
            | FormKind::MoveChart => {
                self.chart_data_event(kind, ev);
            }
            FormKind::TableName | FormKind::TableResize => self.table_form_event(kind, ev),
            FormKind::CfRule | FormKind::CfNewRule | FormKind::CfManager => {
                self.cf_form_event(kind, ev);
            }
            FormKind::PasteSpecial => self.paste_special_event(ev),
            FormKind::Unhide | FormKind::TabColor => self.tabs_form_event(kind, ev),
            FormKind::TableStyle => self.tstyle_event(ev),
            FormKind::Options => self.options_event(ev),
            FormKind::FormatCells => {}
        }
        true
    }
}
