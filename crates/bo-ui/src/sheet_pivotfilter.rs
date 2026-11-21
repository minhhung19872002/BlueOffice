//! PivotTable field buttons ▾ of BlueSheet (round 14): the "Row Labels" / "Column Labels"
//! buttons open the AutoFilter-style drop-down as the reference does — Sort A to Z / Z to A
//! (`pivotField/@sortType`, ECMA-376 Part 1, §18.10.1.69), More Sort Options…, Clear Filter
//! From "<field>", Label Filters ▸ and Value Filters ▸ (§18.10.1.35 `filter`), the search
//! box and the item check list (`item/@h`, §18.10.1.43).
use super::filter_impl::{ColumnKind, FilterDrop, Labels, Line};
use super::form_impl::{Event, Form, FormKind, Kind};
use super::SheetWindow;
use crate::popup::{Choice, Menu, MenuItem};
use serde_json::{json, Value};

/// Label filters: id (`ST_PivotFilterType`), label key, fallback, two values.
const LABEL_OPS: [(&str, &str, &str, bool); 14] = [
    ("captionEqual", "sheetFilter.equals", "Equals...", false),
    (
        "captionNotEqual",
        "sheetFilter.notEquals",
        "Does Not Equal...",
        false,
    ),
    (
        "captionBeginsWith",
        "sheetFilter.begins",
        "Begins With...",
        false,
    ),
    (
        "captionNotBeginsWith",
        "sheetPvt.notBegins",
        "Does Not Begin With...",
        false,
    ),
    ("captionEndsWith", "sheetFilter.ends", "Ends With...", false),
    (
        "captionNotEndsWith",
        "sheetPvt.notEnds",
        "Does Not End With...",
        false,
    ),
    (
        "captionContains",
        "sheetFilter.contains",
        "Contains...",
        false,
    ),
    (
        "captionNotContains",
        "sheetFilter.notContains",
        "Does Not Contain...",
        false,
    ),
    (
        "captionGreaterThan",
        "sheetFilter.greater",
        "Greater Than...",
        false,
    ),
    (
        "captionGreaterThanOrEqual",
        "sheetFilter.greaterEq",
        "Greater Than Or Equal To...",
        false,
    ),
    ("captionLessThan", "sheetFilter.less", "Less Than...", false),
    (
        "captionLessThanOrEqual",
        "sheetFilter.lessEq",
        "Less Than Or Equal To...",
        false,
    ),
    ("captionBetween", "sheetFilter.between", "Between...", true),
    (
        "captionNotBetween",
        "sheetPvt.notBetween",
        "Not Between...",
        true,
    ),
];

/// Value filters (and Top 10).
const VALUE_OPS: [(&str, &str, &str, bool); 9] = [
    ("valueEqual", "sheetFilter.equals", "Equals...", false),
    (
        "valueNotEqual",
        "sheetFilter.notEquals",
        "Does Not Equal...",
        false,
    ),
    (
        "valueGreaterThan",
        "sheetFilter.greater",
        "Greater Than...",
        false,
    ),
    (
        "valueGreaterThanOrEqual",
        "sheetFilter.greaterEq",
        "Greater Than Or Equal To...",
        false,
    ),
    ("valueLessThan", "sheetFilter.less", "Less Than...", false),
    (
        "valueLessThanOrEqual",
        "sheetFilter.lessEq",
        "Less Than Or Equal To...",
        false,
    ),
    ("valueBetween", "sheetFilter.between", "Between...", true),
    (
        "valueNotBetween",
        "sheetPvt.notBetween",
        "Not Between...",
        true,
    ),
    ("count", "sheetFilter.top10", "Top 10...", false),
];

/// Operator words of the filter dialogs' drop-down (same order as the menus, without "…").
fn op_words(t: &dyn Fn(&str, &str) -> String, ops: &[(&str, &str, &str, bool)]) -> Vec<String> {
    ops.iter()
        .filter(|o| o.0 != "count")
        .map(|(_, k, d, _)| {
            t(k, d)
                .trim_end_matches('.')
                .trim_end_matches('…')
                .to_lowercase()
        })
        .collect()
}

impl SheetWindow {
    /// The field button of a compact PivotTable under (`x`, `y`): (sheet, pivot, field).
    pub(super) fn pivot_button_at(&self, x: f32, y: f32) -> Option<(usize, usize, u32)> {
        let (r, c) = self.view.cell_under(x, y)?;
        let sheet = self.view.sheet();
        let ws = self.view.engine().workbook()?.sheets.get(sheet)?;
        let (index, field) = ws.pivots.iter().enumerate().find_map(|(i, pt)| {
            if pt.layout != bo_model::sheet::PivotLayout::Compact || !pt.options.show_headers {
                return None;
            }
            let at = pt.location.first;
            if pt.first_data_row > 0
                && r == at.row + pt.first_data_row - 1
                && c == at.col
                && !pt.rows.is_empty()
            {
                return Some((i, pt.rows[0]));
            }
            if pt.first_header_row > 0
                && r == at.row + pt.first_header_row - 1
                && c == at.col + pt.first_data_col
                && !pt.cols.is_empty()
            {
                return Some((i, pt.cols[0]));
            }
            None
        })?;
        let [_, top, right, bottom] = self.view.cell_box(r, c)?;
        let size = (bottom - top - 3.0).clamp(8.0 * self.scale, 16.0 * self.scale);
        (x >= right - size - 2.0).then_some((sheet, index, field))
    }

    fn pivot_info_at(&mut self, sheet: usize, index: usize) -> Option<Value> {
        self.view
            .query(json!({ "cmd": "pivots", "sheet": sheet }))?
            .as_array()?
            .get(index)
            .cloned()
    }

    /// Open the field button's drop-down under its cell.
    pub(super) fn open_pivot_filter(&mut self, sheet: usize, index: usize, field: u32) -> bool {
        let Some(info) = self.pivot_info_at(sheet, index) else {
            return false;
        };
        let fi = info["fields"][field as usize].clone();
        let opts = info["fieldOptions"][field as usize].clone();
        let header = opts
            .get("name")
            .and_then(Value::as_str)
            .or_else(|| fi.get("name").and_then(Value::as_str))
            .unwrap_or("")
            .to_owned();
        let hidden: Vec<u64> = opts
            .get("hidden")
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(Value::as_u64).collect())
            .unwrap_or_default();
        let filtered = info
            .get("fieldFilters")
            .and_then(Value::as_array)
            .is_some_and(|a| a.iter().any(|f| f["field"] == json!(field)));
        let blank = self.t("sheetPivot.blank", "(blank)");
        let lines: Vec<Line> = fi
            .get("items")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .enumerate()
                    .map(|(i, v)| {
                        let text = v.as_str().unwrap_or("");
                        Line {
                            text: if text.is_empty() {
                                blank.clone()
                            } else {
                                text.to_owned()
                            },
                            checked: !hidden.contains(&(i as u64)),
                            year: None,
                            month: None,
                        }
                    })
                    .collect()
            })
            .unwrap_or_default();
        // Under the button's cell, its left edge on the cell's left edge.
        let ws_loc = self
            .view
            .engine()
            .workbook()
            .and_then(|wb| wb.sheets.get(sheet)?.pivots.get(index))
            .map(|p| {
                let at = p.location.first;
                if p.rows.first() == Some(&field) {
                    (at.row + p.first_data_row.saturating_sub(1), at.col)
                } else {
                    (
                        at.row + p.first_header_row.saturating_sub(1),
                        at.col + p.first_data_col,
                    )
                }
            });
        let Some((r0, c0)) = ws_loc else {
            return false;
        };
        let k = self.scale;
        let cell = self.view.cell_box(r0, c0).unwrap_or([0.0; 4]);
        let mut drop = FilterDrop {
            col: c0,
            header: header.clone(),
            kind: ColumnKind::Text,
            active: !hidden.is_empty() || filtered,
            lines,
            search: String::new(),
            top: 0,
            hover: None,
            origin: (0.0, 0.0),
            k,
            pivot: true,
        };
        let [_, _, w, h] = drop.rect();
        let x = cell[0].max(2.0 * k).min(self.size.0 - w - 2.0 * k).max(0.0);
        let mut y = cell[3] + 1.0;
        if y + h > self.size.1 {
            y = (self.size.1 - h).max(0.0);
        }
        drop.origin = (x.round(), y.round());
        let has_values = info
            .get("values")
            .and_then(Value::as_array)
            .is_some_and(|v| !v.is_empty());
        self.filter_labels = Labels {
            items: vec![
                self.t("sheetFilter.sortAZ", "Sort A to Z"),
                self.t("sheetFilter.sortZA", "Sort Z to A"),
                self.t("sheetPvt.moreSort", "More Sort Options..."),
                self.t("sheetFilter.clearFrom", "Clear Filter From \"{0}\"")
                    .replace("{0}", &header),
                self.t("sheetPvt.labelFilters", "Label Filters"),
                self.t("sheetPvt.valueFilters", "Value Filters"),
            ],
            enabled: vec![true, true, false, drop.active, true, has_values],
            search: self.t("sheetFilter.search", "Search"),
            select_all: self.t("sheetFilter.selectAll", "(Select All)"),
            select_results: self.t("sheetFilter.selectResults", "(Select All Search Results)"),
            blanks: blank,
            ok: self.t("common.ok", "OK"),
            cancel: self.t("common.cancel", "Cancel"),
        };
        self.filter = Some(drop);
        self.filter_pivot = Some((sheet, index, field));
        self.filter_table = None;
        true
    }

    /// Field options of every field with `change` applied to `field`.
    fn pivot_field_list(info: &Value, field: u32, change: &dyn Fn(&mut Value)) -> Value {
        let list: Vec<Value> = info
            .get("fieldOptions")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .enumerate()
                    .map(|(i, o)| {
                        let mut o = o.clone();
                        o["field"] = json!(i);
                        if i == field as usize {
                            change(&mut o);
                        }
                        o
                    })
                    .collect()
            })
            .unwrap_or_default();
        Value::Array(list)
    }

    /// `pivotUpdate` of the drop-down's report with `settings`; the active cell is moved
    /// into the report first so the settings reach it.
    fn pivot_filter_update(&mut self, settings: &Value) -> bool {
        let Some((sheet, index, _)) = self.filter_pivot else {
            return false;
        };
        let at = self
            .view
            .engine()
            .workbook()
            .and_then(|wb| wb.sheets.get(sheet)?.pivots.get(index))
            .map(|p| (p.location.first.row, p.location.first.col));
        if let Some(at) = at {
            if self.view.sheet() == sheet && !self.in_pivot() {
                self.view.select(at, at, at);
            }
        }
        self.pivot_update_settings(settings)
    }

    /// A menu item of the field drop-down.
    pub(super) fn pivot_filter_item(&mut self, i: usize) -> bool {
        let Some((sheet, index, field)) = self.filter_pivot else {
            return false;
        };
        if !self.filter_labels.enabled.get(i).copied().unwrap_or(false) {
            return true;
        }
        let Some(info) = self.pivot_info_at(sheet, index) else {
            return false;
        };
        match i {
            0 | 1 => {
                self.filter = None;
                let sort = if i == 0 { "ascending" } else { "descending" };
                let list = Self::pivot_field_list(&info, field, &|o| {
                    o["sort"] = json!(sort);
                    o["sortBy"] = Value::Null;
                });
                self.pivot_filter_update(&json!({ "fieldOptions": list }))
            }
            3 => {
                self.filter = None;
                let list = Self::pivot_field_list(&info, field, &|o| o["hidden"] = json!([]));
                let filters: Vec<Value> = info
                    .get("fieldFilters")
                    .and_then(Value::as_array)
                    .map(|a| {
                        a.iter()
                            .filter(|f| f["field"] != json!(field))
                            .cloned()
                            .collect()
                    })
                    .unwrap_or_default();
                self.pivot_filter_update(&json!({ "fieldOptions": list, "fieldFilters": filters }))
            }
            4 | 5 => {
                let ops: &[(&str, &str, &str, bool)] = if i == 4 { &LABEL_OPS } else { &VALUE_OPS };
                let items = ops
                    .iter()
                    .map(|(id, key, fallback, _)| MenuItem {
                        label: self.t(key, fallback),
                        choice: Choice::Command(format!("pfop.{id}")),
                        checked: false,
                        dim: false,
                    })
                    .collect();
                let Some(f) = &self.filter else {
                    return false;
                };
                let [x, y, w, _] = f.rect();
                let iy = y + (f.k * (3.0 + i as f32 * 22.0 + 7.0)).round();
                let anchor = [x + w - 2.0, iy - (30.0 * f.k).round(), 0.0, 0.0];
                self.menu = Some(Menu::open(items, anchor, self.size, self.scale));
                true
            }
            _ => true,
        }
    }

    /// OK of the drop-down: the unchecked items are hidden.
    pub(super) fn pivot_filter_ok(&mut self) -> bool {
        let Some((sheet, index, field)) = self.filter_pivot else {
            return false;
        };
        let Some(f) = self.filter.take() else {
            return false;
        };
        let shown = f.shown();
        let searching = !f.search.is_empty();
        let hidden: Vec<usize> = (0..f.lines.len())
            .filter(|i| !(f.lines[*i].checked && (!searching || shown.contains(i))))
            .collect();
        if hidden.len() == f.lines.len() {
            self.status = self.t("sheetPvt.needOneItem", "Select at least one item to show.");
            return true;
        }
        let depth = self.view.engine().history_len();
        let ok = self.view.run(json!({
            "cmd": "pivotFieldItems", "sheet": sheet, "index": index, "field": field,
            "hidden": hidden,
            "labels": {
                "grandTotal": self.t("sheetInsert.grandTotal", "Grand Total"),
                "rowLabels": self.t("sheetInsert.rowLabels", "Row Labels"),
            },
        }));
        if ok {
            self.pivot_autofit(sheet, index, depth);
            self.sync_pivot_pane();
        } else {
            self.status = self.view.take_error().unwrap_or_default();
        }
        ok
    }

    /// A Label / Value Filters submenu choice: opens its dialog.
    pub(super) fn pivot_filter_choice(&mut self, op: &str) -> bool {
        let Some((sheet, index, field)) = self.filter_pivot else {
            return false;
        };
        self.filter = None;
        let Some(info) = self.pivot_info_at(sheet, index) else {
            return false;
        };
        let caption = info["fieldOptions"][field as usize]
            .get("name")
            .and_then(Value::as_str)
            .or_else(|| info["fields"][field as usize]["name"].as_str())
            .unwrap_or("")
            .to_owned();
        let values: Vec<String> = info
            .get("values")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .map(|v| v["name"].as_str().unwrap_or("").to_owned())
                    .collect()
            })
            .unwrap_or_default();
        // The field's current filter fills the dialog.
        let cur = info
            .get("fieldFilters")
            .and_then(Value::as_array)
            .and_then(|a| a.iter().find(|f| f["field"] == json!(field)).cloned())
            .unwrap_or(Value::Null);
        let t = |k: &str, d: &str| self.t(k, d);
        let s = |v: &Value, k: &str| v.get(k).and_then(Value::as_str).unwrap_or("").to_owned();
        let edit = |text: &str| Kind::Edit {
            value: text.chars().collect(),
            caret: text.chars().count(),
        };
        let form = if op == "count" {
            let mut f = Form::new(
                &t("sheetPvt.top10Title", "Top 10 Filter ({0})").replace("{0}", &caption),
                (560.0, 190.0),
                vec![],
            );
            f.add(
                "showL",
                None,
                [18.0, 52.0, 0.0, 0.0],
                Kind::Label(t("sheetPvt.show", "Show")),
            );
            f.add(
                "dir",
                None,
                [18.0, 76.0, 100.0, 26.0],
                Kind::Combo {
                    items: vec![
                        t("sheetFilter.top", "Top"),
                        t("sheetFilter.bottom", "Bottom"),
                    ],
                    sel: usize::from(cur.get("top").and_then(Value::as_bool) == Some(false)),
                },
            );
            f.add(
                "n",
                None,
                [128.0, 77.0, 76.0, 24.0],
                Kind::Spin {
                    value: cur.get("count").and_then(Value::as_f64).unwrap_or(10.0) as i32,
                    min: 1,
                    max: 32767,
                },
            );
            let kind = s(&cur, "kind");
            f.add(
                "unit",
                None,
                [214.0, 76.0, 100.0, 26.0],
                Kind::Combo {
                    items: vec![
                        t("sheetPvt.items", "Items"),
                        t("sheetFilter.percent", "Percent"),
                        t("sheetPvt.sum", "Sum"),
                    ],
                    sel: match kind.as_str() {
                        "percent" => 1,
                        "sum" => 2,
                        _ => 0,
                    },
                },
            );
            f.add(
                "byL",
                None,
                [324.0, 82.0, 0.0, 0.0],
                Kind::Label(t("sheetPvt.byLower", "by")),
            );
            f.add(
                "measure",
                None,
                [350.0, 76.0, 190.0, 26.0],
                Kind::Combo {
                    items: values,
                    sel: cur.get("measure").and_then(Value::as_u64).unwrap_or(0) as usize,
                },
            );
            f
        } else {
            let label = op.starts_with("caption");
            let ops: &[(&str, &str, &str, bool)] = if label { &LABEL_OPS } else { &VALUE_OPS };
            let words = op_words(&t, ops);
            let sel = ops.iter().position(|o| o.0 == op).unwrap_or(0);
            let title = if label {
                t("sheetPvt.labelFilterTitle", "Label Filter ({0})")
            } else {
                t("sheetPvt.valueFilterTitle", "Value Filter ({0})")
            };
            let mut f = Form::new(&title.replace("{0}", &caption), (600.0, 190.0), vec![]);
            f.add(
                "showL",
                None,
                [18.0, 52.0, 0.0, 0.0],
                Kind::Label(if label {
                    t("sheetPvt.showLabel", "Show items for which the label")
                } else {
                    t("sheetPvt.showValue", "Show items for which")
                }),
            );
            let mut x = 18.0;
            if !label {
                f.add(
                    "measure",
                    None,
                    [x, 76.0, 150.0, 26.0],
                    Kind::Combo {
                        items: values,
                        sel: cur.get("measure").and_then(Value::as_u64).unwrap_or(0) as usize,
                    },
                );
                x += 160.0;
            }
            f.add(
                "op",
                None,
                [x, 76.0, 170.0, 26.0],
                Kind::Combo { items: words, sel },
            );
            x += 180.0;
            let (v1, v2) = (s(&cur, "value1"), s(&cur, "value2"));
            let two = ops[sel].3;
            let w = if two { 100.0 } else { 600.0 - 18.0 - x };
            f.add("v1", None, [x, 77.0, w, 24.0], edit(&v1));
            f.add(
                "andL",
                None,
                [x + w + 8.0, 82.0, 0.0, 0.0],
                Kind::Label(t("sheetPvt.and", "and")),
            );
            f.add("v2", None, [x + w + 38.0, 77.0, 100.0, 24.0], edit(&v2));
            f.show("andL", two);
            f.show("v2", two);
            f.focus("v1");
            f
        };
        let mut form = form;
        form.ok_cancel(&t("common.ok", "OK"), &t("common.cancel", "Cancel"));
        self.pivot_filter_op = op.to_owned();
        self.form = Some(form);
        self.form_kind = FormKind::PivotFilter;
        true
    }

    /// Events of the Label / Value / Top 10 filter dialogs.
    pub(super) fn pivot_filter_form_event(&mut self, ev: &Event) {
        match ev {
            Event::Cancel => self.form = None,
            Event::Changed("op") => {
                let op = self.pivot_filter_op.clone();
                let ops: &[(&str, &str, &str, bool)] = if op.starts_with("caption") {
                    &LABEL_OPS
                } else {
                    &VALUE_OPS
                };
                if let Some(f) = self.form.as_mut() {
                    let two = ops.get(f.sel("op").unwrap_or(0)).is_some_and(|o| o.3);
                    f.show("andL", two);
                    f.show("v2", two);
                    if let Some(c) = f.get_mut("v1") {
                        c.rect[2] = if two { 100.0 } else { 600.0 - 18.0 - c.rect[0] };
                    }
                }
            }
            Event::Ok => {
                let Some((sheet, index, field)) = self.filter_pivot else {
                    self.form = None;
                    return;
                };
                let Some(f) = self.form.as_ref() else {
                    return;
                };
                let op = self.pivot_filter_op.clone();
                let filter = if op == "count" {
                    let unit = ["count", "percent", "sum"][f.sel("unit").unwrap_or(0).min(2)];
                    json!({
                        "field": field,
                        "kind": unit,
                        "measure": f.sel("measure").unwrap_or(0),
                        "top": f.sel("dir") != Some(1),
                        "count": f64::from(f.num("n").max(1)),
                        "value1": "", "value2": "",
                    })
                } else {
                    let ops: &[(&str, &str, &str, bool)] = if op.starts_with("caption") {
                        &LABEL_OPS
                    } else {
                        &VALUE_OPS
                    };
                    let kind = ops
                        .get(f.sel("op").unwrap_or(0))
                        .map_or(op.as_str(), |o| o.0);
                    json!({
                        "field": field,
                        "kind": kind,
                        "measure": f.sel("measure").unwrap_or(0),
                        "value1": f.text("v1"),
                        "value2": f.text("v2"),
                        "top": true,
                        "count": 0.0,
                    })
                };
                let Some(info) = self.pivot_info_at(sheet, index) else {
                    return;
                };
                // One filter per field (the reference's default, `multipleFieldFilters` off).
                let mut list: Vec<Value> = info
                    .get("fieldFilters")
                    .and_then(Value::as_array)
                    .map(|a| {
                        a.iter()
                            .filter(|x| x["field"] != json!(field))
                            .cloned()
                            .collect()
                    })
                    .unwrap_or_default();
                list.push(filter);
                if self.pivot_filter_update(&json!({ "fieldFilters": list })) {
                    self.form = None;
                }
            }
            _ => {}
        }
    }
}
