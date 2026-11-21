//! PivotTable dialogs of BlueSheet round 14, laid out on the self-drawn form as the
//! reference spreadsheet's (measured at 125 % on its own instance, opened with
//! `ExecuteMso` on a scratch copy):
//! - Grouping (Group Field / Group Selection on a date or number field): Auto Starting at /
//!   Ending at, the multi-select By list (Seconds … Years) and Number of days, or By for
//!   numbers (ECMA-376 Part 1, §18.10.1.27 `fieldGroup`, §18.10.1.77 `rangePr`).
//! - Ungroup, Group Selection of text items (§18.10.1.26 `discretePr`).
//! - Insert Calculated Field / Item (§18.10.1.3 `cacheField/@formula`, §18.10.1.8
//!   `calculatedItem`): Name, Formula, Add / Modify, Delete, the field and item lists.
//! - Field Settings of a row / column field: Custom Name (§18.10.1.69 `pivotField/@name`),
//!   Subtotals Automatic / None (`defaultSubtotal`), Include new items in manual filter,
//!   and Layout & Print (outline / compact / tabular, subtotals at the top, repeat labels,
//!   blank line, items with no data, page break).
//! - PivotTable Options (§18.10.1.73 `pivotTableDefinition` attributes): name, layout &
//!   format, totals & filters, display, printing, data and alternative text.
use super::form_impl::{Event, Form, FormKind, Kind};
use super::SheetWindow;
use crate::Painter;
use bo_formula::date::{parse_datetime_text, serial_to_ymd};
use bo_formula::DateSystem;
use bo_render::Rgba;
use serde_json::{json, Value};

/// The By list of the date Grouping dialog, finest first (as the reference lists them).
pub(super) const DATE_PARTS: [(&str, &str, &str); 7] = [
    ("seconds", "sheetPvt.bySeconds", "Seconds"),
    ("minutes", "sheetPvt.byMinutes", "Minutes"),
    ("hours", "sheetPvt.byHours", "Hours"),
    ("days", "sheetPvt.byDays", "Days"),
    ("months", "sheetPvt.byMonths", "Months"),
    ("quarters", "sheetPvt.byQuarters", "Quarters"),
    ("years", "sheetPvt.byYears", "Years"),
];

/// Row height of the By list (reference pixels).
const BY_ROW: f32 = 17.0;

/// State of an open PivotTable dialog.
#[derive(Debug, Clone, Default)]
pub(super) struct PivotDlg {
    pub sheet: usize,
    pub index: usize,
    /// Source field the dialog edits.
    pub field: u32,
    /// Grouping by date parts (else numbers).
    pub dates: bool,
    /// Ticked date parts, in [`DATE_PARTS`] order.
    pub by: [bool; 7],
    /// Calculated item dialog (else calculated field).
    pub item: bool,
    /// Existing calculated fields / items: (name, formula, cache field).
    pub calcs: Vec<(String, String, u32)>,
    /// Report info as `pivots` lists it.
    pub info: Value,
}

fn s(v: &Value, k: &str) -> String {
    v.get(k).and_then(Value::as_str).unwrap_or("").to_owned()
}

fn b(v: &Value, k: &str, d: bool) -> bool {
    v.get(k).and_then(Value::as_bool).unwrap_or(d)
}

fn chars(t: &str) -> Kind {
    Kind::Edit {
        value: t.chars().collect(),
        caret: t.chars().count(),
    }
}

impl SheetWindow {
    fn date_system(&self) -> DateSystem {
        let d1904 = self.view.engine().workbook().is_some_and(|w| w.date1904);
        if d1904 {
            DateSystem::Date1904
        } else {
            DateSystem::Date1900
        }
    }

    /// `m/d/yyyy` text of a date serial (the reference's Grouping boxes).
    fn date_box(&self, serial: f64) -> String {
        serial_to_ymd(serial, self.date_system())
            .map_or_else(|| serial.to_string(), |(y, m, d)| format!("{m}/{d}/{y}"))
    }

    fn field_info(info: &Value, f: u32) -> Value {
        info.get("fields")
            .and_then(Value::as_array)
            .and_then(|a| a.get(f as usize))
            .cloned()
            .unwrap_or(Value::Null)
    }

    fn pvt_field_names(info: &Value) -> Vec<String> {
        info.get("fields")
            .and_then(Value::as_array)
            .map(|a| a.iter().map(|f| s(f, "name")).collect())
            .unwrap_or_default()
    }

    /// The row / column field of the active cell (not a value field).
    fn axis_field(&mut self) -> Option<u32> {
        match self.pivot_active_field() {
            Some((f, _, false)) => Some(f),
            _ => None,
        }
    }

    /// Whether field `f` of `info` can be grouped by range: dates or numbers only.
    fn rangeable(info: &Value, f: u32) -> Option<bool> {
        let fi = Self::field_info(info, f);
        if b(&fi, "derived", false) || !fi.get("formula").is_none_or(Value::is_null) {
            return None;
        }
        if b(&fi, "date", false) {
            return Some(true);
        }
        let by = fi
            .get("group")
            .and_then(|g| g.get("by"))
            .and_then(Value::as_str);
        if by == Some("numbers") {
            return Some(false);
        }
        if by.is_some() {
            return None;
        }
        let items = fi.get("items").and_then(Value::as_array)?;
        let numeric = |t: &str| t.trim().is_empty() || t.replace(',', "").parse::<f64>().is_ok();
        (!items.is_empty() && items.iter().all(|i| numeric(i.as_str().unwrap_or(""))))
            .then_some(false)
    }

    /// Menu entries of the Analyze tab that do not apply to the active cell (dimmed).
    pub(super) fn pivot_dlg_dims(&mut self) -> Vec<&'static str> {
        let mut dim = vec!["pvt.subFiltered"];
        let Some(pr) = self.pivot_ref() else {
            return dim;
        };
        let field = self.axis_field();
        let grouped = field.is_some_and(|f| {
            let fi = Self::field_info(&pr.info, f);
            fi.get("group").is_some_and(|g| !g.is_null())
        });
        match field {
            None => dim.extend(["pvt.groupSelection", "pvt.groupField", "pvt.calcItem"]),
            Some(f) => {
                if Self::rangeable(&pr.info, f).is_none() {
                    dim.push("pvt.groupField");
                }
                if grouped {
                    dim.push("pvt.calcItem");
                }
            }
        }
        if !grouped {
            dim.push("pvt.ungroup");
        }
        dim
    }

    /// Dialog commands of the PivotTable tabs; `None` for other ids.
    pub(super) fn pivot_dlg_action(&mut self, id: &str) -> Option<bool> {
        Some(match id {
            "pvt.groupField" => {
                let f = self.axis_field()?;
                self.open_pivot_grouping(f)
            }
            "pvt.groupSelection" => self.pivot_group_selection(),
            "pvt.ungroup" => self.pivot_ungroup(),
            "pvt.calcField" => self.open_pivot_calc(false),
            "pvt.calcItem" => self.open_pivot_calc(true),
            "pvt.options" => self.open_pivot_options(),
            "pvt.buttons" | "pvt.headers" => {
                let pr = self.pivot_ref()?;
                let mut o = pr.info.get("options").cloned().unwrap_or(json!({}));
                let k = if id == "pvt.buttons" {
                    "showDrill"
                } else {
                    "showHeaders"
                };
                o[k] = json!(!b(&o, k, true));
                self.pivot_update_settings(&json!({ "options": o }))
            }
            "sheet.pvtFieldSettings" => match self.pivot_active_field() {
                Some((f, _, false)) => self.open_pivot_field_settings(f),
                _ => return None,
            },
            _ => return None,
        })
    }

    fn pivot_dlg_state(&mut self, field: u32) -> Option<PivotDlg> {
        let pr = self.pivot_ref()?;
        Some(PivotDlg {
            sheet: pr.sheet,
            index: pr.index,
            field,
            info: pr.info,
            ..PivotDlg::default()
        })
    }

    // ---- Grouping ---------------------------------------------------------------------

    /// Grouping dialog of field `f` (a grouping field built on a date field opens its base).
    pub(super) fn open_pivot_grouping(&mut self, f: u32) -> bool {
        let Some(mut st) = self.pivot_dlg_state(f) else {
            return false;
        };
        let fi = Self::field_info(&st.info, f);
        let g = fi.get("group").cloned().unwrap_or(Value::Null);
        if s(&g, "by") == "derived" {
            if let Some(base) = g.get("base").and_then(Value::as_u64) {
                return self.open_pivot_grouping(base as u32);
            }
        }
        let Some(dates) = Self::rangeable(&st.info, f) else {
            return false;
        };
        st.dates = dates;
        // Bounds: the grouping's, else the data's.
        let sys = self.date_system();
        // The cache's values (numbers and date serials); item captions only when the
        // cache's records are not loaded.
        let mut values: Vec<f64> = self
            .view
            .engine()
            .workbook()
            .and_then(|wb| {
                let pt = wb.sheets.get(st.sheet)?.pivots.get(st.index)?;
                let c = wb.pivot_caches.iter().find(|c| c.cache_id == pt.cache_id)?;
                Some(
                    c.records
                        .iter()
                        .filter_map(|r| match r.get(f as usize)? {
                            bo_model::sheet::CellValue::Number(n) => Some(*n),
                            bo_model::sheet::CellValue::Date(d) => {
                                parse_datetime_text(&d.replacen('T', " ", 1), sys)
                            }
                            _ => None,
                        })
                        .collect(),
                )
            })
            .unwrap_or_default();
        if values.is_empty() {
            values = fi
                .get("items")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .filter_map(|t| {
                    if dates {
                        parse_datetime_text(t, sys)
                    } else {
                        t.replace(',', "").parse::<f64>().ok()
                    }
                })
                .collect();
        }
        let lo = values.iter().copied().fold(f64::INFINITY, f64::min);
        let hi = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let num = |k: &str, d: f64| g.get(k).and_then(Value::as_f64).unwrap_or(d);
        let (start, end) = (num("start", lo), num("end", hi));
        let t = |k: &str, d: &str| self.t(k, d);
        let fmt = |v: f64| {
            if !v.is_finite() {
                String::new()
            } else if dates {
                self.date_box(v.floor())
            } else {
                bo_sheet::pivot::value_text(&bo_model::sheet::CellValue::Number(v))
            }
        };
        let (start_t, end_t) = (fmt(start), fmt(end));
        let size = if dates {
            (364.0, 404.0)
        } else {
            (306.0, 228.0)
        };
        let mut form = Form::new(&t("sheetPvt.groupingTitle", "Grouping"), size, vec![]);
        let box_w = if dates { 210.0 } else { 151.0 };
        form.add(
            "autoG",
            None,
            [18.0, 56.0, size.0 - 36.0, 0.0],
            Kind::Group(t("sheetPvt.auto", "Auto")),
        );
        form.add(
            "autoS",
            None,
            [32.0, 77.0, 100.0, 18.0],
            Kind::Check {
                label: t("sheetPvt.startingAt", "Starting at:"),
                on: g.get("autoStart").and_then(Value::as_bool).unwrap_or(true),
            },
        );
        form.add("start", None, [136.0, 74.0, box_w, 25.0], chars(&start_t));
        form.add(
            "autoE",
            None,
            [32.0, 107.0, 100.0, 18.0],
            Kind::Check {
                label: t("sheetPvt.endingAt", "Ending at:"),
                on: g.get("autoEnd").and_then(Value::as_bool).unwrap_or(true),
            },
        );
        form.add("end", None, [136.0, 104.0, box_w, 25.0], chars(&end_t));
        if dates {
            let mut by = [false; 7];
            if s(&g, "by") == "dates" {
                for (i, (k, ..)) in DATE_PARTS.iter().enumerate() {
                    by[i] = b(&g, k, false);
                }
            }
            if !by.iter().any(|x| *x) {
                by[4] = true; // Months, as the reference proposes.
            }
            st.by = by;
            form.add(
                "byL",
                None,
                [18.0, 149.0, 0.0, 0.0],
                Kind::Label(t("sheetPvt.by", "By")),
            );
            form.add("by", None, [32.0, 165.0, 314.0, 139.0], Kind::Canvas);
            form.add(
                "daysL",
                None,
                [159.0, 322.0, 0.0, 0.0],
                Kind::Label(t("sheetPvt.numberOfDays", "Number of days:")),
            );
            let days = g.get("daysInterval").and_then(Value::as_f64).unwrap_or(1.0) as i32;
            form.add(
                "days",
                None,
                [271.0, 311.0, 75.0, 24.0],
                Kind::Spin {
                    value: days.max(1),
                    min: 1,
                    max: 32767,
                },
            );
        } else {
            let by = num("interval", 1.0);
            form.add(
                "byL",
                None,
                [56.0, 147.0, 0.0, 0.0],
                Kind::Label(t("sheetPvt.byColon", "By:")),
            );
            form.add(
                "byN",
                None,
                [136.0, 134.0, box_w, 25.0],
                chars(&bo_sheet::pivot::value_text(
                    &bo_model::sheet::CellValue::Number(by),
                )),
            );
        }
        form.ok_cancel(&t("common.ok", "OK"), &t("common.cancel", "Cancel"));
        let y = size.1 - 47.0;
        if let Some(c) = form.get_mut("ok") {
            c.rect = [size.0 - 258.0, y, 111.0, 27.0];
        }
        if let Some(c) = form.get_mut("cancel") {
            c.rect = [size.0 - 131.0, y, 111.0, 27.0];
        }
        form.select_all("start");
        self.pvt_dlg = Some(st);
        self.pivot_grouping_enable(&mut form);
        self.form = Some(form);
        self.form_kind = FormKind::PivotGrouping;
        true
    }

    /// Number of days only with Days as the only part (as the reference).
    fn pivot_grouping_enable(&self, form: &mut Form) {
        if let Some(st) = &self.pvt_dlg {
            let only_days = st.by == [false, false, false, true, false, false, false];
            form.enable("days", only_days);
            form.enable("daysL", only_days);
        }
    }

    /// The By list: white box framed grey, ticked parts highlighted.
    pub(super) fn paint_pivot_dlg(&self, f: &Form, id: &str, r: [f32; 4], p: &mut dyn Painter) {
        let Some(st) = &self.pvt_dlg else {
            return;
        };
        if id != "by" {
            return;
        }
        let k = f.scale_k();
        p.fill_rect(r[0], r[1], r[2], r[3], Rgba(0x82, 0x87, 0x90, 255));
        p.fill_rect(
            r[0] + 1.0,
            r[1] + 1.0,
            r[2] - 2.0,
            r[3] - 2.0,
            Rgba(255, 255, 255, 255),
        );
        let font = f.font();
        let line = font.size * 1.33;
        let rh = BY_ROW * k;
        for (i, (_, key, fb)) in DATE_PARTS.iter().enumerate() {
            let y = (r[1] + 2.0 * k + i as f32 * rh).round();
            let on = st.by[i];
            if on {
                p.fill_rect(
                    r[0] + 2.0 * k,
                    y,
                    r[2] - 4.0 * k - 17.0 * k,
                    rh.round(),
                    Rgba(0x00, 0x78, 0xD7, 255),
                );
            }
            let ink = if on {
                Rgba(255, 255, 255, 255)
            } else {
                Rgba(0, 0, 0, 255)
            };
            p.text_font(
                &self.t(key, fb),
                r[0] + 5.0 * k,
                (y + (rh - line) / 2.0).round(),
                &font,
                ink,
            );
        }
        // The list's (inactive) scroll bar, as the reference draws it.
        let sx = r[0] + r[2] - 1.0 - 17.0 * k;
        p.fill_rect(
            sx,
            r[1] + 1.0,
            17.0 * k,
            r[3] - 2.0,
            Rgba(0xF0, 0xF0, 0xF0, 255),
        );
    }

    fn pivot_grouping_ok(&mut self) {
        let Some(st) = self.pvt_dlg.clone() else {
            self.form = None;
            return;
        };
        let Some(form) = self.form.as_ref() else {
            return;
        };
        let sys = self.date_system();
        let parse = |t: &str| -> Option<f64> {
            if st.dates {
                parse_datetime_text(t.trim(), sys)
            } else {
                t.trim().replace(',', "").parse::<f64>().ok()
            }
        };
        let bound = |auto: &str, text: &str| -> Result<Option<f64>, ()> {
            if form.on(auto) {
                Ok(None)
            } else {
                parse(&form.text(text)).map(Some).ok_or(())
            }
        };
        let (Ok(start), Ok(end)) = (bound("autoS", "start"), bound("autoE", "end")) else {
            self.status = self.t(
                "sheetPvt.badBound",
                "The starting and ending values must be numbers or dates.",
            );
            return;
        };
        let mut cmd = json!({
            "cmd": "pivotGroup", "sheet": st.sheet, "index": st.index, "field": st.field,
            "labels": self.pivot_group_labels(),
        });
        if st.dates {
            if !st.by.iter().any(|x| *x) {
                self.status = self.t("sheetPvt.pickPart", "Select at least one grouping option.");
                return;
            }
            cmd["by"] = json!("dates");
            for (i, (k, ..)) in DATE_PARTS.iter().enumerate() {
                cmd[*k] = json!(st.by[i]);
            }
            cmd["daysInterval"] = json!(f64::from(form.num("days").max(1)));
        } else {
            let Some(by) = parse(&form.text("byN")).filter(|v| *v > 0.0) else {
                self.status = self.t("sheetPvt.badBy", "The By value must be a positive number.");
                return;
            };
            cmd["by"] = json!("numbers");
            cmd["interval"] = json!(by);
        }
        if let Some(v) = start {
            cmd["start"] = json!(v);
        }
        if let Some(v) = end {
            cmd["end"] = json!(v);
        }
        self.pivot_dlg_run(&st, cmd);
    }

    /// Captions of grouping fields and items in the UI language.
    fn pivot_group_labels(&self) -> Value {
        let months: Vec<String> = (1..=12)
            .map(|m| self.t(&format!("sheetPvt.mon{m}"), MONTHS[m - 1]))
            .collect();
        json!({
            "grandTotal": self.t("sheetInsert.grandTotal", "Grand Total"),
            "rowLabels": self.t("sheetInsert.rowLabels", "Row Labels"),
            "months": months,
            "quarter": self.t("sheetPvt.quarterN", "Qtr{n}"),
            "yearsField": self.t("sheetPvt.byYears", "Years"),
            "quartersField": self.t("sheetPvt.byQuarters", "Quarters"),
            "monthsField": self.t("sheetPvt.byMonths", "Months"),
            "daysField": self.t("sheetPvt.byDays", "Days"),
            "hoursField": self.t("sheetPvt.byHours", "Hours"),
            "minutesField": self.t("sheetPvt.byMinutes", "Minutes"),
            "group": self.t("sheetPvt.groupN", "Group{n}"),
        })
    }

    /// Run a pivot command from a dialog: one undo step with the fitted columns.
    fn pivot_dlg_run(&mut self, st: &PivotDlg, cmd: Value) -> bool {
        let depth = self.view.engine().history_len();
        if self.view.run(cmd) {
            self.form = None;
            self.pvt_dlg = None;
            self.pivot_autofit(st.sheet, st.index, depth);
            self.sync_pivot_pane();
            true
        } else {
            self.status = self.view.take_error().unwrap_or_default();
            false
        }
    }

    /// Group Selection: date / number fields open Grouping; text items selected in the
    /// report become a manual group.
    fn pivot_group_selection(&mut self) -> bool {
        let Some(f) = self.axis_field() else {
            return false;
        };
        let Some(pr) = self.pivot_ref() else {
            return false;
        };
        let fi = Self::field_info(&pr.info, f);
        let discrete = fi
            .get("group")
            .and_then(|g| g.get("by"))
            .and_then(Value::as_str)
            == Some("discrete");
        if !discrete && Self::rangeable(&pr.info, f).is_some() {
            return self.open_pivot_grouping(f);
        }
        let items: Vec<String> = fi
            .get("items")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .map(|v| v.as_str().unwrap_or("").to_owned())
                    .collect()
            })
            .unwrap_or_default();
        let (r0, c0, r1, c1) = self.view.range_bounds();
        let mut chosen: Vec<usize> = Vec::new();
        for r in r0..=r1 {
            for c in c0..=c1 {
                let text = self
                    .view
                    .engine()
                    .cell(pr.sheet, r, c)
                    .map(|d| d.display.trim().to_owned())
                    .unwrap_or_default();
                if let Some(i) = items.iter().position(|x| *x == text) {
                    if !chosen.contains(&i) {
                        chosen.push(i);
                    }
                }
            }
        }
        if chosen.is_empty() {
            self.status = self.t("sheetPvt.cannotGroup", "Cannot group that selection.");
            return false;
        }
        let st = PivotDlg {
            sheet: pr.sheet,
            index: pr.index,
            field: f,
            ..PivotDlg::default()
        };
        let cmd = json!({
            "cmd": "pivotGroupItems", "sheet": pr.sheet, "index": pr.index, "field": f,
            "items": chosen, "labels": self.pivot_group_labels(),
        });
        self.pivot_dlg_run(&st, cmd)
    }

    /// Ungroup: range groups go back to their values; a manual group's items leave it.
    fn pivot_ungroup(&mut self) -> bool {
        let Some(mut f) = self.axis_field() else {
            return false;
        };
        let Some(pr) = self.pivot_ref() else {
            return false;
        };
        let mut fi = Self::field_info(&pr.info, f);
        let g = fi.get("group").cloned().unwrap_or(Value::Null);
        if s(&g, "by") == "derived" {
            if let Some(base) = g.get("base").and_then(Value::as_u64) {
                // A grouping field (Years over Months, Region2 over Region).
                let base_g = Self::field_info(&pr.info, base as u32)
                    .get("group")
                    .and_then(|x| x.get("by"))
                    .and_then(Value::as_str)
                    .map(str::to_owned);
                if base_g.as_deref() != Some("discrete") {
                    f = base as u32;
                    fi = Self::field_info(&pr.info, f);
                }
            }
        }
        let st = PivotDlg {
            sheet: pr.sheet,
            index: pr.index,
            field: f,
            ..PivotDlg::default()
        };
        let by = fi
            .get("group")
            .and_then(|x| x.get("by"))
            .and_then(Value::as_str)
            .unwrap_or("");
        let cmd = if matches!(by, "dates" | "numbers") {
            json!({
                "cmd": "pivotGroup", "sheet": pr.sheet, "index": pr.index, "field": f,
                "by": "none", "labels": self.pivot_group_labels(),
            })
        } else {
            let items: Vec<String> = fi
                .get("items")
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .map(|v| v.as_str().unwrap_or("").to_owned())
                        .collect()
                })
                .unwrap_or_default();
            let (r, c) = self.view.active();
            let text = self
                .view
                .engine()
                .cell(pr.sheet, r, c)
                .map(|d| d.display.trim().to_owned())
                .unwrap_or_default();
            let i = items.iter().position(|x| *x == text).unwrap_or(0);
            json!({
                "cmd": "pivotGroupItems", "sheet": pr.sheet, "index": pr.index, "field": f,
                "items": [i], "ungroup": true, "labels": self.pivot_group_labels(),
            })
        };
        self.pivot_dlg_run(&st, cmd)
    }

    // ---- Calculated field / item ------------------------------------------------------

    /// Insert Calculated Field (`item` false) or Calculated Item in "<field>".
    pub(super) fn open_pivot_calc(&mut self, item: bool) -> bool {
        let field = if item {
            match self.axis_field() {
                Some(f) => f,
                None => return false,
            }
        } else {
            0
        };
        let Some(mut st) = self.pivot_dlg_state(field) else {
            return false;
        };
        st.item = item;
        let names = Self::pvt_field_names(&st.info);
        st.calcs = if item {
            st.info
                .get("calcItems")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter(|c| c.get("field").and_then(Value::as_u64) == Some(u64::from(field)))
                .map(|c| (s(c, "name"), s(c, "formula"), field))
                .collect()
        } else {
            st.info
                .get("fields")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .enumerate()
                .filter(|(_, f)| f.get("formula").and_then(Value::as_str).is_some())
                .map(|(i, f)| (s(f, "name"), s(f, "formula"), i as u32))
                .collect()
        };
        let t = |k: &str, d: &str| self.t(k, d);
        let fname = names.get(field as usize).cloned().unwrap_or_default();
        let (title, size) = if item {
            (
                t(
                    "sheetPvt.calcItemTitle",
                    "Insert Calculated Item in \"{0}\"",
                )
                .replace("{0}", &fname),
                (678.0, 406.0),
            )
        } else {
            (
                t("sheetPvt.calcFieldTitle", "Insert Calculated Field"),
                (504.0, 406.0),
            )
        };
        let mut form = Form::new(&title, size, vec![]);
        let base = if item { "Formula" } else { "Field" };
        let n = (1..10_000)
            .map(|i| format!("{base}{i}"))
            .find(|c| {
                !st.calcs.iter().any(|x| x.0.eq_ignore_ascii_case(c))
                    && !names.iter().any(|x| x.eq_ignore_ascii_case(c))
            })
            .unwrap_or_default();
        let box_w = if item { 402.0 } else { 280.0 };
        let bx = if item { 500.0 } else { 380.0 };
        form.add(
            "nameL",
            None,
            [18.0, 66.0, 0.0, 0.0],
            Kind::Label(t("sheetPvt.calcName", "Name:")),
        );
        form.add("name", None, [84.0, 53.0, box_w, 25.0], chars(&n));
        form.add(
            "formulaL",
            None,
            [18.0, 96.0, 0.0, 0.0],
            Kind::Label(t("sheetPvt.calcFormula", "Formula:")),
        );
        form.add("formula", None, [84.0, 83.0, box_w, 25.0], chars("= 0"));
        form.add(
            "add",
            None,
            [bx, 47.0, 111.0, 28.0],
            Kind::Button {
                label: t("sheetPvt.calcAdd", "Add"),
                default: false,
            },
        );
        form.add(
            "delete",
            None,
            [bx, 86.0, 111.0, 28.0],
            Kind::Button {
                label: t("sheetPvt.calcDelete", "Delete"),
                default: false,
            },
        );
        form.add(
            "rule1",
            None,
            [18.0, 130.0, size.0 - 36.0, 1.0],
            Kind::Canvas,
        );
        form.add(
            "fieldsL",
            None,
            [18.0, 157.0, 0.0, 0.0],
            Kind::Label(t("sheetPvt.calcFields", "Fields:")),
        );
        form.add(
            "fields",
            None,
            [18.0, 169.0, 313.0, 140.0],
            Kind::List {
                items: names.clone(),
                sel: Some(field as usize),
                top: 0,
            },
        );
        form.add(
            "insField",
            None,
            [220.0, 314.0, 111.0, 28.0],
            Kind::Button {
                label: t("sheetPvt.insertField", "Insert Field"),
                default: false,
            },
        );
        if item {
            let items: Vec<String> = Self::field_info(&st.info, field)
                .get("items")
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .map(|v| v.as_str().unwrap_or("").to_owned())
                        .collect()
                })
                .unwrap_or_default();
            form.add(
                "itemsL",
                None,
                [346.0, 157.0, 0.0, 0.0],
                Kind::Label(t("sheetPvt.calcItems", "Items:")),
            );
            form.add(
                "items",
                None,
                [346.0, 169.0, 313.0, 140.0],
                Kind::List {
                    items,
                    sel: None,
                    top: 0,
                },
            );
            form.add(
                "insItem",
                None,
                [548.0, 314.0, 111.0, 28.0],
                Kind::Button {
                    label: t("sheetPvt.insertItem", "Insert Item"),
                    default: false,
                },
            );
        }
        form.ok_cancel(&t("common.ok", "OK"), &t("common.close", "Close"));
        if let Some(c) = form.get_mut("ok") {
            c.rect = [size.0 - 258.0, 358.0, 111.0, 28.0];
        }
        if let Some(c) = form.get_mut("cancel") {
            c.rect = [size.0 - 131.0, 358.0, 111.0, 28.0];
        }
        form.enable("delete", false);
        form.select_all("name");
        self.pvt_dlg = Some(st);
        self.form = Some(form);
        self.form_kind = FormKind::PivotCalc;
        true
    }

    /// Add / Modify and Delete follow the name (as the reference).
    fn pivot_calc_sync(&mut self) {
        let Some(st) = &self.pvt_dlg else {
            return;
        };
        let name = self
            .form
            .as_ref()
            .map(|f| f.text("name"))
            .unwrap_or_default();
        let known = st
            .calcs
            .iter()
            .find(|c| c.0.eq_ignore_ascii_case(name.trim()))
            .cloned();
        let add = if known.is_some() {
            self.t("sheetPvt.calcModify", "Modify")
        } else {
            self.t("sheetPvt.calcAdd", "Add")
        };
        let Some(form) = self.form.as_mut() else {
            return;
        };
        if let Some(c) = form.get_mut("add") {
            if let Kind::Button { label, .. } = &mut c.kind {
                *label = add;
            }
        }
        form.enable("delete", known.is_some());
    }

    /// Add or modify the named calculated field / item; `false` when it failed.
    fn pivot_calc_add(&mut self) -> bool {
        let Some(st) = self.pvt_dlg.clone() else {
            return false;
        };
        let Some(form) = self.form.as_ref() else {
            return false;
        };
        let name = form.text("name").trim().to_owned();
        let formula = form.text("formula").trim().to_owned();
        if name.is_empty() {
            return true;
        }
        let known = st.calcs.iter().find(|c| c.0.eq_ignore_ascii_case(&name));
        let labels = self.pivot_group_labels();
        let cmd = if st.item {
            json!({
                "cmd": "pivotCalcItem", "sheet": st.sheet, "index": st.index,
                "field": st.field, "old": known.map(|k| k.0.clone()), "name": name,
                "formula": formula, "labels": labels,
            })
        } else {
            json!({
                "cmd": "pivotCalcField", "sheet": st.sheet, "index": st.index,
                "field": known.map(|k| k.2), "name": name, "formula": formula,
                "labels": labels,
            })
        };
        let depth = self.view.engine().history_len();
        if !self.view.run(cmd) {
            self.status = self.view.take_error().unwrap_or_default();
            return false;
        }
        self.pivot_autofit(st.sheet, st.index, depth);
        // Re-read the list so a second Add modifies.
        if let Some(pr) = self.pivot_ref() {
            if let Some(d) = self.pvt_dlg.as_mut() {
                d.info = pr.info;
                if !d.calcs.iter().any(|c| c.0.eq_ignore_ascii_case(&name)) {
                    let f = Self::pvt_field_names(&d.info)
                        .iter()
                        .position(|x| x.eq_ignore_ascii_case(&name))
                        .map_or(d.field, |i| i as u32);
                    d.calcs
                        .push((name, formula, if d.item { d.field } else { f }));
                }
            }
        }
        self.pivot_calc_sync();
        true
    }

    fn pivot_calc_delete(&mut self) {
        let Some(st) = self.pvt_dlg.clone() else {
            return;
        };
        let name = self
            .form
            .as_ref()
            .map(|f| f.text("name").trim().to_owned())
            .unwrap_or_default();
        let Some(known) = st
            .calcs
            .iter()
            .find(|c| c.0.eq_ignore_ascii_case(&name))
            .cloned()
        else {
            return;
        };
        let labels = self.pivot_group_labels();
        let cmd = if st.item {
            json!({
                "cmd": "pivotCalcItem", "sheet": st.sheet, "index": st.index,
                "field": st.field, "old": known.0, "remove": true, "labels": labels,
            })
        } else {
            json!({
                "cmd": "pivotCalcField", "sheet": st.sheet, "index": st.index,
                "field": known.2, "remove": true, "labels": labels,
            })
        };
        if self.view.run(cmd) {
            if let Some(d) = self.pvt_dlg.as_mut() {
                d.calcs.retain(|c| !c.0.eq_ignore_ascii_case(&name));
            }
            self.sync_pivot_pane();
        } else {
            self.status = self.view.take_error().unwrap_or_default();
        }
        self.pivot_calc_sync();
    }

    /// Insert a field / item name into the formula (quoted when it has spaces).
    fn pivot_calc_insert(&mut self, list: &str) {
        let Some(form) = self.form.as_mut() else {
            return;
        };
        let Some(i) = form.sel(list) else {
            return;
        };
        let name = match &form.get(list).map(|c| c.kind.clone()) {
            Some(Kind::List { items, .. }) => items.get(i).cloned().unwrap_or_default(),
            _ => return,
        };
        let part = if name.chars().all(|c| c.is_alphanumeric() || c == '_') {
            name
        } else {
            format!("'{}'", name.replace('\'', "''"))
        };
        let mut text = form.text("formula");
        if text.trim() == "= 0" || text.trim() == "=0" {
            text = "= ".to_owned();
        }
        text.push_str(&part);
        form.set_text("formula", &text);
        form.focus("formula");
    }

    // ---- Field Settings ---------------------------------------------------------------

    /// Field Settings of row / column field `f`.
    pub(super) fn open_pivot_field_settings(&mut self, f: u32) -> bool {
        let Some(st) = self.pivot_dlg_state(f) else {
            return false;
        };
        let names = Self::pvt_field_names(&st.info);
        let src = names.get(f as usize).cloned().unwrap_or_default();
        let o = st
            .info
            .get("fieldOptions")
            .and_then(Value::as_array)
            .and_then(|a| a.get(f as usize))
            .cloned()
            .unwrap_or(json!({}));
        let custom = o
            .get("name")
            .and_then(Value::as_str)
            .map_or_else(|| src.clone(), str::to_owned);
        let layout = s(&st.info, "layout");
        let t = |k: &str, d: &str| self.t(k, d);
        let mut form = Form::new(
            &t("sheetPvt.fsTitle", "Field Settings"),
            (519.0, 546.0),
            vec![
                t("sheetPvt.fsSubtotalsTab", "Subtotals & Filters"),
                t("sheetPvt.fsLayoutTab", "Layout & Print"),
            ],
        );
        form.tab_top = Some(115.0);
        form.page_h = Some(334.0);
        form.add(
            "srcL",
            None,
            [18.0, 56.0, 0.0, 0.0],
            Kind::Label(t("sheetPivot.sourceName", "Source Name:")),
        );
        form.add("src", None, [114.0, 56.0, 0.0, 0.0], Kind::Label(src));
        form.add(
            "nameL",
            None,
            [18.0, 88.0, 0.0, 0.0],
            Kind::Label(t("sheetPivot.customName", "Custom Name:")),
        );
        form.add("name", None, [117.0, 75.0, 378.0, 26.0], chars(&custom));
        // Subtotals & Filters.
        form.add(
            "subG",
            Some(0),
            [32.0, 161.0, 455.0, 0.0],
            Kind::Group(t("sheetPvt.fsSubtotals", "Subtotals")),
        );
        form.set_bold("subG");
        let sub = b(&o, "subtotal", true);
        for (id, key, fb, y, on) in [
            ("subAuto", "sheetPvt.fsAutomatic", "Automatic", 190.0, sub),
            ("subNone", "sheetPvt.fsNone", "None", 215.0, !sub),
            ("subCustom", "sheetPvt.fsCustom", "Custom", 240.0, false),
        ] {
            form.add(
                id,
                Some(0),
                [32.0, y - 9.0, 200.0, 18.0],
                Kind::Check {
                    label: t(key, fb),
                    on,
                },
            );
        }
        form.add(
            "funcsL",
            Some(0),
            [59.0, 263.0, 0.0, 0.0],
            Kind::Label(t(
                "sheetPvt.fsSelectFunctions",
                "Select one or more functions:",
            )),
        );
        let funcs = [
            ("sum", "Sum"),
            ("count", "Count"),
            ("average", "Average"),
            ("max", "Max"),
            ("min", "Min"),
            ("product", "Product"),
            ("countNums", "Count Numbers"),
            ("stdDev", "StdDev"),
            ("stdDevp", "StdDevp"),
            ("var", "Var"),
            ("varp", "Varp"),
        ];
        form.add(
            "funcs",
            Some(0),
            [59.0, 276.0, 359.0, 105.0],
            Kind::List {
                items: funcs
                    .iter()
                    .map(|(k, d)| t(&format!("sheetPivot.fn_{k}"), d))
                    .collect(),
                sel: None,
                top: 0,
            },
        );
        form.add(
            "filtG",
            Some(0),
            [32.0, 394.0, 455.0, 0.0],
            Kind::Group(t("sheetPvt.fsFilter", "Filter")),
        );
        form.set_bold("filtG");
        form.add(
            "newItems",
            Some(0),
            [32.0, 414.0, 400.0, 18.0],
            Kind::Check {
                label: t("sheetPvt.fsNewItems", "Include new items in manual filter"),
                on: b(&o, "includeNewItems", false),
            },
        );
        // Layout & Print.
        form.add(
            "layG",
            Some(1),
            [32.0, 161.0, 455.0, 0.0],
            Kind::Group(t("sheetPvt.fsLayout", "Layout")),
        );
        form.set_bold("layG");
        let tabular = layout == "tabular";
        let checks: [(&'static str, &str, &str, f32, f32, bool); 8] = [
            (
                "outline",
                "sheetPvt.fsOutline",
                "Show item labels in outline form",
                32.0,
                190.0,
                !tabular,
            ),
            (
                "compact",
                "sheetPvt.fsCompact",
                "Display labels from the next field in the same column (compact form)",
                59.0,
                215.0,
                layout == "compact",
            ),
            (
                "top",
                "sheetPvt.fsTop",
                "Display subtotals at the top of each group",
                59.0,
                240.0,
                b(&o, "subtotalTop", true),
            ),
            (
                "tabular",
                "sheetPvt.fsTabular",
                "Show item labels in tabular form",
                32.0,
                265.0,
                tabular,
            ),
            (
                "repeat",
                "sheetPvt.fsRepeat",
                "Repeat item labels",
                32.0,
                301.0,
                b(&o, "repeatLabels", false),
            ),
            (
                "blank",
                "sheetPvt.fsBlank",
                "Insert blank line after each item label",
                32.0,
                326.0,
                b(&o, "blankRow", false),
            ),
            (
                "noData",
                "sheetPvt.fsNoData",
                "Show items with no data",
                32.0,
                351.0,
                b(&o, "showAll", false),
            ),
            (
                "pageBreak",
                "sheetPvt.fsPageBreak",
                "Insert page break after each item",
                32.0,
                453.0,
                b(&o, "pageBreak", false),
            ),
        ];
        for (id, key, fb, x, y, on) in checks {
            form.add(
                id,
                Some(1),
                [x, y - 9.0, 440.0, 18.0],
                Kind::Check {
                    label: t(key, fb),
                    on,
                },
            );
        }
        form.add(
            "printG",
            Some(1),
            [32.0, 424.0, 455.0, 0.0],
            Kind::Group(t("sheetPvt.fsPrint", "Print")),
        );
        form.set_bold("printG");
        form.set_radios(&["subAuto", "subNone", "subCustom", "outline", "tabular"]);
        form.ok_cancel(&t("common.ok", "OK"), &t("common.cancel", "Cancel"));
        if let Some(c) = form.get_mut("ok") {
            c.rect = [261.0, 500.0, 111.0, 27.0];
        }
        if let Some(c) = form.get_mut("cancel") {
            c.rect = [388.0, 500.0, 111.0, 27.0];
        }
        form.select_all("name");
        Self::field_settings_enable(&mut form);
        self.pvt_dlg = Some(st);
        self.form = Some(form);
        self.form_kind = FormKind::PivotFieldSettings;
        true
    }

    fn field_settings_enable(form: &mut Form) {
        let custom = form.on("subCustom");
        form.enable("funcs", custom);
        form.enable("funcsL", custom);
        let outline = form.on("outline");
        form.enable("compact", outline);
        form.enable("top", outline);
    }

    fn pivot_field_settings_ok(&mut self) {
        let Some(st) = self.pvt_dlg.clone() else {
            self.form = None;
            return;
        };
        let Some(form) = self.form.as_ref() else {
            return;
        };
        let names = Self::pvt_field_names(&st.info);
        let src = names.get(st.field as usize).cloned().unwrap_or_default();
        let name = form.text("name").trim().to_owned();
        let custom = (!name.is_empty() && name != src).then_some(name);
        let on = |id: &str| form.on(id);
        let layout = if on("tabular") {
            "tabular"
        } else if on("compact") {
            "compact"
        } else {
            "outline"
        };
        let mut list: Vec<Value> = st
            .info
            .get("fieldOptions")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        for (i, o) in list.iter_mut().enumerate() {
            o["field"] = json!(i);
        }
        if let Some(o) = list.get_mut(st.field as usize) {
            // Custom subtotal functions are kept as automatic subtotals for now.
            o["subtotal"] = json!(!on("subNone"));
            o["subtotalTop"] = json!(on("top"));
            o["repeatLabels"] = json!(on("repeat"));
            o["blankRow"] = json!(on("blank"));
            o["showAll"] = json!(on("noData"));
            o["pageBreak"] = json!(on("pageBreak"));
            o["includeNewItems"] = json!(on("newItems"));
            o["name"] = json!(custom);
        }
        if self.pivot_update_settings(&json!({ "fieldOptions": list, "layout": layout })) {
            self.form = None;
            self.pvt_dlg = None;
        }
    }

    // ---- PivotTable Options -----------------------------------------------------------

    /// PivotTable Options of the report of the active cell.
    #[allow(clippy::too_many_lines)]
    pub(super) fn open_pivot_options(&mut self) -> bool {
        let Some(st) = self.pivot_dlg_state(0) else {
            return false;
        };
        let o = st.info.get("options").cloned().unwrap_or(json!({}));
        let t = |k: &str, d: &str| self.t(k, d);
        let mut form = Form::new(
            &t("sheetPvt.optTitle", "PivotTable Options"),
            (580.0, 652.0),
            vec![
                t("sheetPvt.optLayoutTab", "Layout & Format"),
                t("sheetPvt.optTotalsTab", "Totals & Filters"),
                t("sheetPvt.optDisplayTab", "Display"),
                t("sheetPvt.optPrintingTab", "Printing"),
                t("sheetPvt.optDataTab", "Data"),
                t("sheetPvt.optAltTab", "Alt Text"),
            ],
        );
        form.tab_top = Some(110.0);
        form.page_h = Some(452.0);
        form.add(
            "nameL",
            None,
            [18.0, 60.0, 0.0, 0.0],
            Kind::Label(t("sheetPvt.optName", "PivotTable Name:")),
        );
        form.add(
            "name",
            None,
            [124.0, 47.0, 288.0, 26.0],
            chars(&s(&st.info, "name")),
        );
        let group = |form: &mut Form, id: &'static str, page: usize, y: f32, text: String| {
            form.add(id, Some(page), [32.0, y, 515.0, 0.0], Kind::Group(text));
        };
        let check = |form: &mut Form,
                     id: &'static str,
                     page: usize,
                     (x, y): (f32, f32),
                     label: String,
                     on: bool| {
            form.add(
                id,
                Some(page),
                [x, y - 9.0, 480.0, 18.0],
                Kind::Check { label, on },
            );
        };
        // Layout & Format.
        group(&mut form, "g0", 0, 157.0, t("sheetPvt.optLayout", "Layout"));
        check(
            &mut form,
            "merge",
            0,
            (46.0, 184.0),
            t("sheetPvt.optMerge", "Merge and center cells with labels"),
            b(&o, "mergeItem", false),
        );
        form.add(
            "indentL",
            Some(0),
            [46.0, 211.0, 0.0, 0.0],
            Kind::Label(t(
                "sheetPvt.optIndent",
                "When in compact form indent row labels:",
            )),
        );
        form.add(
            "indent",
            Some(0),
            [290.0, 199.0, 76.0, 24.0],
            Kind::Spin {
                value: o.get("indent").and_then(Value::as_i64).unwrap_or(1) as i32,
                min: 0,
                max: 127,
            },
        );
        form.add(
            "indentU",
            Some(0),
            [371.0, 211.0, 0.0, 0.0],
            Kind::Label(t("sheetPvt.optChars", "character(s)")),
        );
        form.add(
            "pageL",
            Some(0),
            [46.0, 259.0, 0.0, 0.0],
            Kind::Label(t(
                "sheetPvt.optPageOrder",
                "Display fields in report filter area:",
            )),
        );
        form.add(
            "pageOrder",
            Some(0),
            [246.0, 246.0, 144.0, 26.0],
            Kind::Combo {
                items: vec![
                    t("sheetPvt.optDownOver", "Down, Then Over"),
                    t("sheetPvt.optOverDown", "Over, Then Down"),
                ],
                sel: usize::from(b(&o, "pageOverThenDown", false)),
            },
        );
        form.add(
            "wrapL",
            Some(0),
            [46.0, 289.0, 0.0, 0.0],
            Kind::Label(t("sheetPvt.optWrap", "Report filter fields per column:")),
        );
        form.add(
            "wrap",
            Some(0),
            [237.0, 277.0, 76.0, 24.0],
            Kind::Spin {
                value: o.get("pageWrap").and_then(Value::as_i64).unwrap_or(0) as i32,
                min: 0,
                max: 255,
            },
        );
        group(&mut form, "g1", 0, 322.0, t("sheetPvt.optFormat", "Format"));
        check(
            &mut form,
            "showErr",
            0,
            (46.0, 352.0),
            t("sheetPvt.optErrors", "For error values show:"),
            b(&o, "showError", false),
        );
        form.add(
            "errText",
            Some(0),
            [216.0, 340.0, 90.0, 24.0],
            chars(&s(&o, "errorCaption")),
        );
        check(
            &mut form,
            "showEmpty",
            0,
            (46.0, 382.0),
            t("sheetPvt.optEmpty", "For empty cells show:"),
            b(&o, "showMissing", true),
        );
        form.add(
            "emptyText",
            Some(0),
            [216.0, 370.0, 90.0, 24.0],
            chars(&s(&o, "missingCaption")),
        );
        check(
            &mut form,
            "autofit",
            0,
            (46.0, 427.0),
            t("sheetPvt.optAutofit", "Autofit column widths on update"),
            b(&o, "autofit", true),
        );
        check(
            &mut form,
            "preserve",
            0,
            (46.0, 452.0),
            t("sheetPvt.optPreserve", "Preserve cell formatting on update"),
            b(&o, "preserveFormatting", true),
        );
        // Totals & Filters.
        group(
            &mut form,
            "g2",
            1,
            157.0,
            t("sheetPvt.optGrand", "Grand Totals"),
        );
        check(
            &mut form,
            "grandRows",
            1,
            (32.0, 184.0),
            t("sheetPvt.optGrandRows", "Show grand totals for rows"),
            b(&st.info, "rowGrandTotals", true),
        );
        check(
            &mut form,
            "grandCols",
            1,
            (32.0, 209.0),
            t("sheetPvt.optGrandCols", "Show grand totals for columns"),
            b(&st.info, "colGrandTotals", true),
        );
        group(
            &mut form,
            "g3",
            1,
            238.0,
            t("sheetPvt.optFilters", "Filters"),
        );
        check(
            &mut form,
            "subHidden",
            1,
            (32.0, 265.0),
            t("sheetPvt.optSubHidden", "Subtotal filtered page items"),
            b(&o, "subtotalHiddenItems", false),
        );
        check(
            &mut form,
            "multiFilters",
            1,
            (32.0, 290.0),
            t(
                "sheetPvt.optMultiFilters",
                "Allow multiple filters per field",
            ),
            b(&o, "multipleFieldFilters", false),
        );
        group(
            &mut form,
            "g4",
            1,
            320.0,
            t("sheetPvt.optSorting", "Sorting"),
        );
        check(
            &mut form,
            "customLists",
            1,
            (32.0, 347.0),
            t("sheetPvt.optCustomLists", "Use Custom Lists when sorting"),
            b(&o, "customListSort", true),
        );
        // Display.
        group(
            &mut form,
            "g5",
            2,
            157.0,
            t("sheetPvt.optDisplay", "Display"),
        );
        let display: [(&'static str, f32, &str, &str, bool); 9] = [
            (
                "drill",
                184.0,
                "sheetPvt.optDrill",
                "Show expand/collapse buttons",
                b(&o, "showDrill", true),
            ),
            (
                "tips",
                225.0,
                "sheetPvt.optTips",
                "Show contextual tooltips",
                b(&o, "showDataTips", true),
            ),
            (
                "props",
                251.0,
                "sheetPvt.optProps",
                "Show properties in tooltips",
                false,
            ),
            (
                "headers",
                292.0,
                "sheetPvt.optHeaders",
                "Display field captions and filter drop downs",
                b(&o, "showHeaders", true),
            ),
            (
                "classic",
                318.0,
                "sheetPvt.optClassic",
                "Classic PivotTable layout (enables dragging of fields in the grid)",
                b(&o, "gridDropZones", false),
            ),
            (
                "valuesRow",
                343.0,
                "sheetPvt.optValuesRow",
                "Show the Values row",
                false,
            ),
            (
                "emptyRows",
                384.0,
                "sheetPvt.optEmptyRows",
                "Show items with no data on rows",
                false,
            ),
            (
                "emptyCols",
                409.0,
                "sheetPvt.optEmptyCols",
                "Show items with no data on columns",
                false,
            ),
            (
                "itemLabels",
                451.0,
                "sheetPvt.optItemLabels",
                "Display item labels when no fields are in the values area",
                true,
            ),
        ];
        for (id, y, key, fb, on) in display {
            check(&mut form, id, 2, (32.0, y), t(key, fb), on);
        }
        group(
            &mut form,
            "g6",
            2,
            485.0,
            t("sheetPvt.optFieldList", "Field List"),
        );
        let asc = b(&o, "fieldListSortAscending", false);
        check(
            &mut form,
            "sortAZ",
            2,
            (32.0, 512.0),
            t("sheetPvt.optSortAZ", "Sort A to Z"),
            asc,
        );
        check(
            &mut form,
            "sortSource",
            2,
            (32.0, 538.0),
            t("sheetPvt.optSortSource", "Sort in data source order"),
            !asc,
        );
        // Printing.
        group(&mut form, "g7", 3, 157.0, t("sheetPvt.optPrint", "Print"));
        check(
            &mut form,
            "printDrill",
            3,
            (32.0, 184.0),
            t(
                "sheetPvt.optPrintDrill",
                "Print expand/collapse buttons when displayed on PivotTable",
            ),
            b(&o, "printDrill", false),
        );
        check(
            &mut form,
            "itemTitles",
            3,
            (32.0, 209.0),
            t(
                "sheetPvt.optItemTitles",
                "Repeat row labels on each printed page",
            ),
            b(&o, "itemPrintTitles", true),
        );
        check(
            &mut form,
            "fieldTitles",
            3,
            (32.0, 234.0),
            t("sheetPvt.optFieldTitles", "Set print titles"),
            b(&o, "fieldPrintTitles", false),
        );
        // Data.
        group(
            &mut form,
            "g8",
            4,
            157.0,
            t("sheetPvt.optData", "PivotTable Data"),
        );
        check(
            &mut form,
            "saveData",
            4,
            (32.0, 184.0),
            t("sheetPvt.optSaveData", "Save source data with file"),
            true,
        );
        check(
            &mut form,
            "enableDrill",
            4,
            (32.0, 209.0),
            t("sheetPvt.optEnableDrill", "Enable show details"),
            true,
        );
        check(
            &mut form,
            "refreshLoad",
            4,
            (32.0, 234.0),
            t(
                "sheetPvt.optRefreshLoad",
                "Refresh data when opening the file",
            ),
            false,
        );
        // Alt Text.
        form.add(
            "altTL",
            Some(5),
            [32.0, 157.0, 0.0, 0.0],
            Kind::Label(t("sheetPvt.optAltTitle", "Title")),
        );
        form.add(
            "altTitle",
            Some(5),
            [32.0, 170.0, 515.0, 24.0],
            chars(&s(&o, "altText")),
        );
        form.add(
            "altDL",
            Some(5),
            [32.0, 207.0, 0.0, 0.0],
            Kind::Label(t("sheetPvt.optAltDesc", "Description")),
        );
        form.add(
            "altDesc",
            Some(5),
            [32.0, 220.0, 515.0, 24.0],
            chars(&s(&o, "altTextSummary")),
        );
        form.add(
            "altNote",
            Some(5),
            [32.0, 270.0, 515.0, 90.0],
            Kind::Note(t(
                "sheetPvt.optAltNote",
                "A title and a description give a text alternative to the table for people who cannot see it; screen readers read them aloud.",
            )),
        );
        form.set_radios(&["sortAZ", "sortSource"]);
        for id in [
            "props",
            "emptyRows",
            "emptyCols",
            "itemLabels",
            "saveData",
            "enableDrill",
            "refreshLoad",
        ] {
            form.enable(id, false);
        }
        form.ok_cancel(&t("common.ok", "OK"), &t("common.cancel", "Cancel"));
        if let Some(c) = form.get_mut("ok") {
            c.rect = [322.0, 605.0, 111.0, 27.0];
        }
        if let Some(c) = form.get_mut("cancel") {
            c.rect = [449.0, 605.0, 111.0, 27.0];
        }
        form.select_all("name");
        Self::options_enable(&mut form);
        self.pvt_dlg = Some(st);
        self.form = Some(form);
        self.form_kind = FormKind::PivotOptions;
        true
    }

    fn options_enable(form: &mut Form) {
        let err = form.on("showErr");
        form.enable("errText", err);
        let empty = form.on("showEmpty");
        form.enable("emptyText", empty);
    }

    fn pivot_options_ok(&mut self) {
        let Some(st) = self.pvt_dlg.clone() else {
            self.form = None;
            return;
        };
        let Some(form) = self.form.as_ref() else {
            return;
        };
        let on = |id: &str| form.on(id);
        let mut o = st.info.get("options").cloned().unwrap_or(json!({}));
        let set = |o: &mut Value, k: &str, v: Value| o[k] = v;
        set(&mut o, "mergeItem", json!(on("merge")));
        set(&mut o, "indent", json!(form.num("indent").max(0)));
        set(
            &mut o,
            "pageOverThenDown",
            json!(form.sel("pageOrder") == Some(1)),
        );
        set(&mut o, "pageWrap", json!(form.num("wrap").max(0)));
        set(&mut o, "showError", json!(on("showErr")));
        set(&mut o, "errorCaption", json!(form.text("errText")));
        set(&mut o, "showMissing", json!(on("showEmpty")));
        set(&mut o, "missingCaption", json!(form.text("emptyText")));
        set(&mut o, "autofit", json!(on("autofit")));
        set(&mut o, "preserveFormatting", json!(on("preserve")));
        set(&mut o, "subtotalHiddenItems", json!(on("subHidden")));
        set(&mut o, "multipleFieldFilters", json!(on("multiFilters")));
        set(&mut o, "customListSort", json!(on("customLists")));
        set(&mut o, "showDrill", json!(on("drill")));
        set(&mut o, "showDataTips", json!(on("tips")));
        set(&mut o, "showHeaders", json!(on("headers")));
        set(&mut o, "gridDropZones", json!(on("classic")));
        set(&mut o, "fieldListSortAscending", json!(on("sortAZ")));
        set(&mut o, "printDrill", json!(on("printDrill")));
        set(&mut o, "itemPrintTitles", json!(on("itemTitles")));
        set(&mut o, "fieldPrintTitles", json!(on("fieldTitles")));
        set(&mut o, "altText", json!(form.text("altTitle")));
        set(&mut o, "altTextSummary", json!(form.text("altDesc")));
        let name = form.text("name").trim().to_owned();
        let settings = json!({
            "name": name,
            "options": o,
            "rowGrandTotals": on("grandRows"),
            "colGrandTotals": on("grandCols"),
        });
        if self.pivot_update_settings(&settings) {
            self.form = None;
            self.pvt_dlg = None;
            if let Some(p) = self.pivot.as_mut() {
                p.sort_az = on_sort(&settings);
            }
        }
    }

    // ---- events -----------------------------------------------------------------------

    /// Events of the round 14 PivotTable dialogs.
    pub(super) fn pivot_dlg_event(&mut self, kind: FormKind, ev: &Event) {
        if matches!(ev, Event::Cancel) {
            self.form = None;
            self.pvt_dlg = None;
            return;
        }
        match (kind, ev) {
            (FormKind::PivotGrouping, Event::Ok) => self.pivot_grouping_ok(),
            (FormKind::PivotGrouping, Event::Canvas("by", _, y)) => {
                let row = ((y - 2.0) / BY_ROW).floor();
                if row >= 0.0 && (row as usize) < DATE_PARTS.len() {
                    if let Some(st) = self.pvt_dlg.as_mut() {
                        st.by[row as usize] = !st.by[row as usize];
                    }
                    if let Some(mut f) = self.form.take() {
                        self.pivot_grouping_enable(&mut f);
                        self.form = Some(f);
                    }
                }
            }
            (FormKind::PivotGrouping, Event::Changed(id @ ("start" | "end" | "byN"))) => {
                // Typing a bound turns its Auto box off, as the reference does.
                if let Some(f) = self.form.as_mut() {
                    match *id {
                        "start" => f.set_on("autoS", false),
                        "end" => f.set_on("autoE", false),
                        _ => {}
                    }
                }
            }
            (FormKind::PivotCalc, Event::Ok) => {
                if self.pivot_calc_add() {
                    self.form = None;
                    self.pvt_dlg = None;
                }
            }
            (FormKind::PivotCalc, Event::Clicked("add")) => {
                self.pivot_calc_add();
            }
            (FormKind::PivotCalc, Event::Clicked("delete")) => self.pivot_calc_delete(),
            (FormKind::PivotCalc, Event::Clicked("insField" | "fields")) => {
                self.pivot_calc_insert("fields");
            }
            (FormKind::PivotCalc, Event::Clicked("insItem" | "items")) => {
                self.pivot_calc_insert("items");
            }
            (FormKind::PivotCalc, Event::Changed("name")) => {
                let name = self
                    .form
                    .as_ref()
                    .map(|f| f.text("name"))
                    .unwrap_or_default();
                let known = self.pvt_dlg.as_ref().and_then(|st| {
                    st.calcs
                        .iter()
                        .find(|c| c.0.eq_ignore_ascii_case(name.trim()))
                        .map(|c| c.1.clone())
                });
                if let (Some(formula), Some(f)) = (known, self.form.as_mut()) {
                    // Shown as the reference shows it: "= Sales*2".
                    let shown = if formula.trim_start().starts_with('=') {
                        formula
                    } else {
                        format!("= {formula}")
                    };
                    f.set_text("formula", &shown);
                }
                self.pivot_calc_sync();
            }
            (FormKind::PivotFieldSettings, Event::Ok) => self.pivot_field_settings_ok(),
            (FormKind::PivotFieldSettings, Event::Changed(id)) => {
                if let Some(f) = self.form.as_mut() {
                    radio(f, id, &["subAuto", "subNone", "subCustom"]);
                    radio(f, id, &["outline", "tabular"]);
                    Self::field_settings_enable(f);
                }
            }
            (FormKind::PivotOptions, Event::Ok) => self.pivot_options_ok(),
            (FormKind::PivotOptions, Event::Changed(id)) => {
                if let Some(f) = self.form.as_mut() {
                    radio(f, id, &["sortAZ", "sortSource"]);
                    Self::options_enable(f);
                }
            }
            _ => {}
        }
    }
}

/// Keep exactly one option button of `group` on: the one just clicked.
fn radio(f: &mut Form, id: &str, group: &[&str]) {
    if !group.contains(&id) {
        return;
    }
    for g in group {
        f.set_on(g, *g == id);
    }
}

fn on_sort(settings: &Value) -> bool {
    settings
        .get("options")
        .and_then(|o| o.get("fieldListSortAscending"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];
