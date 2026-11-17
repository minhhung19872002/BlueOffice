//! Data tab dialogs of BlueSheet laid out as the reference's (round 39, measured at 125 %,
//! `show/sheet-31…`): Subtotal (401 × 606 px), Remove Duplicates (501 × 415 px) and Advanced
//! Filter (331 × 314 px). They drive the engine's `subtotal` / `removeSubtotals`,
//! `removeDuplicates` and `advancedFilter` commands; each run is one undo step.

use super::form_impl::{Event, Form, FormKind, Kind};
use super::SheetWindow;
use serde_json::{json, Value};

/// Summary functions of the Subtotal dialog: (engine name, label key, fallback).
const FUNCTIONS: [(&str, &str, &str); 11] = [
    ("sum", "sheetNative.ddSum", "Sum"),
    ("count", "sheetNative.ddCount", "Count"),
    ("average", "sheetNative.ddAverage", "Average"),
    ("max", "sheetNative.ddMax", "Max"),
    ("min", "sheetNative.ddMin", "Min"),
    ("product", "sheetNative.ddProduct", "Product"),
    ("countNums", "sheetNative.ddCountNums", "Count Numbers"),
    ("stdDev", "sheetNative.ddStdDev", "StdDev"),
    ("stdDevp", "sheetNative.ddStdDevp", "StdDevp"),
    ("var", "sheetNative.ddVar", "Var"),
    ("varp", "sheetNative.ddVarp", "Varp"),
];

/// Check box ids of the column lists (at most this many columns are offered).
const COLS: [&str; 12] = [
    "c0", "c1", "c2", "c3", "c4", "c5", "c6", "c7", "c8", "c9", "c10", "c11",
];

fn rng_of(v: &Value) -> Option<[u32; 4]> {
    let n = |k: &str| v.get(k).and_then(Value::as_u64).map(|n| n as u32);
    Some([n("r0")?, n("c0")?, n("r1")?, n("c1")?])
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

impl SheetWindow {
    /// Data tab commands with a reference-like dialog: `Some(redraw)` when `id` is one.
    pub(super) fn datadlg_action(&mut self, id: &str) -> Option<bool> {
        Some(match id {
            "outline.subtotal" => self.open_subtotal(),
            "sheet.removeDuplicates" => self.open_remove_dups(),
            "sheet.filterAdvanced" => self.open_advanced_filter(),
            _ => return None,
        })
    }

    /// The list around the selection, its header guess and its column captions (the
    /// header cells, or "Column X").
    fn list_columns(&mut self) -> Option<([u32; 4], bool, Vec<String>)> {
        if self.view.editing() {
            self.view.commit();
        }
        let region = self.region();
        let r = rng_of(&region)?;
        let header = self
            .view
            .query(json!({ "cmd": "sortRegion", "range": region }))
            .and_then(|p| p.get("hasHeader").and_then(Value::as_bool))
            .unwrap_or(true);
        let sheet = self.view.sheet();
        let names = (r[1]..=r[3])
            .map(|c| {
                let text = if header {
                    self.view
                        .engine()
                        .cell(sheet, r[0], c)
                        .ok()
                        .map(|d| d.display)
                        .unwrap_or_default()
                } else {
                    String::new()
                };
                if text.trim().is_empty() {
                    self.t("sheetNative.ddColumnX", "Column {0}")
                        .replace("{0}", &col_letters(c))
                } else {
                    text
                }
            })
            .collect();
        Some((r, header, names))
    }

    /// Data › Outline › Subtotal.
    fn open_subtotal(&mut self) -> bool {
        let Some((r, _, names)) = self.list_columns() else {
            return false;
        };
        let t = |k: &str, f: &str| self.t(k, f);
        let mut f = Form::new(
            &t("sheetNative.ddSubtotal", "Subtotal"),
            (401.0, 606.0),
            vec![],
        );
        // The reference's Subtotal: white face, text about 1.3 times the classic dialogs'.
        f.set_font_scale(1.3);
        f.add("face", None, [1.0, 38.0, 399.0, 567.0], Kind::Canvas);
        f.add(
            "groupL",
            None,
            [17.0, 50.0, 360.0, 22.0],
            Kind::Label(t("sheetNative.ddEachChange", "At each change in:")),
        );
        f.add(
            "group",
            None,
            [17.0, 77.0, 368.0, 30.0],
            Kind::Combo {
                items: names.clone(),
                sel: 0,
            },
        );
        f.add(
            "fnL",
            None,
            [17.0, 116.0, 360.0, 22.0],
            Kind::Label(t("sheetNative.ddUseFunction", "Use function:")),
        );
        f.add(
            "fn",
            None,
            [17.0, 143.0, 368.0, 30.0],
            Kind::Combo {
                items: FUNCTIONS.iter().map(|(_, k, d)| t(k, d)).collect(),
                sel: 0,
            },
        );
        f.add(
            "addL",
            None,
            [17.0, 182.0, 360.0, 22.0],
            Kind::Label(t("sheetNative.ddAddTo", "Add subtotal to:")),
        );
        // The column list: a framed box with one check box per column (41 px rows as the
        // reference while five fit, tighter beyond).
        f.add("greybox", None, [17.0, 209.0, 368.0, 231.0], Kind::Canvas);
        let n = names.len().min(COLS.len());
        let pitch = if n <= 5 { 41.0 } else { 220.0 / n as f32 };
        for (i, name) in names.iter().take(n).enumerate() {
            f.add(
                COLS[i],
                None,
                [21.0, 219.0 + pitch * i as f32, 350.0, 24.0],
                Kind::Check {
                    label: name.clone(),
                    on: i + 1 == n,
                },
            );
        }
        for (id, key, fb, y, on) in [
            (
                "replace",
                "sheetNative.ddReplace",
                "Replace current subtotals",
                446.0,
                true,
            ),
            (
                "breaks",
                "sheetNative.ddBreaks",
                "Page break between groups",
                482.0,
                false,
            ),
            (
                "below",
                "sheetNative.ddBelow",
                "Summary below data",
                518.0,
                true,
            ),
        ] {
            f.add(
                id,
                None,
                [17.0, y, 360.0, 24.0],
                Kind::Check {
                    label: t(key, fb),
                    on,
                },
            );
        }
        for (id, at) in [
            ("groupL", 0),
            ("fnL", 0),
            ("addL", 1),
            ("replace", 8),
            ("breaks", 0),
            ("below", 0),
        ] {
            f.set_access(id, at);
        }
        f.add(
            "removeAll",
            None,
            [30.0, 553.0, 110.0, 35.0],
            Kind::Button {
                label: t("sheetNative.ddRemoveAll", "Remove All"),
                default: false,
            },
        );
        f.ok_cancel(&t("common.ok", "OK"), &t("common.cancel", "Cancel"));
        f.set_access("removeAll", 0);
        if let Some(c) = f.get_mut("ok") {
            c.rect = [153.0, 553.0, 110.0, 35.0];
        }
        // As the reference: Count when the ticked (last) column holds text, else Sum.
        let numeric = {
            let sheet = self.view.sheet();
            let wb = self.view.engine().workbook();
            (r[0] + 1..=r[2]).any(|row| {
                wb.and_then(|w| w.sheets.get(sheet))
                    .and_then(|ws| ws.cell(row, r[3]))
                    .is_some_and(|c| matches!(c.value, bo_model::sheet::CellValue::Number(_)))
            })
        };
        if !numeric {
            f.set_sel("fn", Some(1));
        }
        if let Some(c) = f.get_mut("cancel") {
            c.rect = [276.0, 553.0, 110.0, 35.0];
        }
        self.dd_list = Some(r);
        self.form = Some(f);
        self.form_kind = FormKind::Subtotal;
        true
    }

    /// Data › Remove Duplicates.
    fn open_remove_dups(&mut self) -> bool {
        let Some((r, header, names)) = self.list_columns() else {
            return false;
        };
        let t = |k: &str, f: &str| self.t(k, f);
        let mut f = Form::new(
            &t("sheetNative.ddRemoveDups", "Remove Duplicates"),
            (501.0, 415.0),
            vec![],
        );
        // The reference's text is about 1.15 times the classic dialogs'.
        f.set_font_scale(1.15);
        f.add(
            "note",
            None,
            [17.0, 48.0, 470.0, 44.0],
            Kind::Note(t(
                "sheetNative.ddDupsNote",
                "To delete duplicate values, select one or more columns that contain duplicates.",
            )),
        );
        f.add(
            "all",
            None,
            [31.0, 107.0, 110.0, 33.0],
            Kind::Button {
                label: t("sheetNative.ddSelectAll", "Select All"),
                default: false,
            },
        );
        f.add(
            "none",
            None,
            [157.0, 107.0, 130.0, 33.0],
            Kind::Button {
                label: t("sheetNative.ddUnselectAll", "Unselect All"),
                default: false,
            },
        );
        f.add(
            "header",
            None,
            [305.0, 111.0, 180.0, 24.0],
            Kind::Check {
                label: t("sheetNative.ddHeaders", "My data has headers"),
                on: header,
            },
        );
        f.add("rule1", None, [0.0, 95.0, 501.0, 1.0], Kind::Canvas);
        f.add("rule2", None, [0.0, 151.0, 501.0, 1.0], Kind::Canvas);
        f.add("listbox", None, [17.0, 166.0, 467.0, 175.0], Kind::Canvas);
        f.add(
            "colsL",
            None,
            [24.0, 174.0, 200.0, 22.0],
            Kind::Label(t("sheetNative.ddColumns", "Columns")),
        );
        let n = names.len().min(COLS.len());
        let pitch = if n <= 5 { 28.0 } else { 140.0 / n as f32 };
        for (i, name) in names.iter().take(n).enumerate() {
            f.add(
                COLS[i],
                None,
                [21.0, 201.0 + pitch * i as f32, 440.0, 24.0],
                Kind::Check {
                    label: name.clone(),
                    on: true,
                },
            );
        }
        f.ok_cancel(&t("common.ok", "OK"), &t("common.cancel", "Cancel"));
        if let Some(c) = f.get_mut("ok") {
            c.rect = [257.0, 362.0, 106.0, 33.0];
        }
        if let Some(c) = f.get_mut("cancel") {
            c.rect = [379.0, 362.0, 106.0, 33.0];
        }
        self.dd_list = Some(r);
        self.form = Some(f);
        self.form_kind = FormKind::RemoveDups;
        true
    }

    /// Data › Advanced (filter).
    fn open_advanced_filter(&mut self) -> bool {
        if self.view.editing() {
            self.view.commit();
        }
        let region = self.region();
        let list = rng_of(&region).map_or_else(String::new, |r| {
            format!(
                "${}${}:${}${}",
                col_letters(r[1]),
                r[0] + 1,
                col_letters(r[3]),
                r[2] + 1
            )
        });
        let t = |k: &str, f: &str| self.t(k, f);
        let mut f = Form::new(
            &t("sheetNative.ddAdvanced", "Advanced Filter"),
            (331.0, 314.0),
            vec![],
        );
        f.add(
            "action",
            None,
            [17.0, 48.0, 297.0, 0.0],
            Kind::Group(t("sheetNative.ddAction", "Action")),
        );
        f.add(
            "inPlace",
            None,
            [33.0, 75.0, 280.0, 24.0],
            Kind::Check {
                label: t("sheetNative.ddInPlace", "Filter the list, in-place"),
                on: true,
            },
        );
        f.add(
            "copy",
            None,
            [33.0, 98.0, 280.0, 24.0],
            Kind::Check {
                label: t("sheetNative.ddCopyTo", "Copy to another location"),
                on: false,
            },
        );
        let edit = |s: &str| Kind::Edit {
            value: s.chars().collect(),
            caret: s.chars().count(),
        };
        for (id, lid, key, fb, y, v) in [
            (
                "list",
                "listL",
                "sheetNative.ddListRange",
                "List range:",
                128.0,
                list.as_str(),
            ),
            (
                "crit",
                "critL",
                "sheetNative.ddCriteria",
                "Criteria range:",
                159.0,
                "",
            ),
            ("dest", "destL", "sheetNative.ddDest", "Copy to:", 190.0, ""),
        ] {
            f.add(
                lid,
                None,
                [18.0, y + 3.0, 95.0, 22.0],
                Kind::Label(t(key, fb)),
            );
            f.add(id, None, [115.0, y, 172.0, 26.0], edit(v));
        }
        // Range pickers (round 45): collapse the dialog to pick cells.
        for (id, y) in [
            ("pickList", 128.0),
            ("pickCrit", 159.0),
            ("pickDest", 190.0),
        ] {
            f.add(id, None, [287.0, y, 26.0, 26.0], Kind::Canvas);
        }
        f.enable("pickDest", false);
        f.enable("dest", false);
        f.enable("destL", false);
        f.add(
            "unique",
            None,
            [17.0, 229.0, 280.0, 24.0],
            Kind::Check {
                label: t("sheetNative.ddUnique", "Unique records only"),
                on: false,
            },
        );
        f.ok_cancel(&t("common.ok", "OK"), &t("common.cancel", "Cancel"));
        if let Some(c) = f.get_mut("ok") {
            c.rect = [73.0, 265.0, 113.0, 33.0];
        }
        if let Some(c) = f.get_mut("cancel") {
            c.rect = [200.0, 265.0, 113.0, 33.0];
        }
        f.set_radios(&["inPlace", "copy"]);
        f.focus("crit");
        self.form = Some(f);
        self.form_kind = FormKind::AdvancedFilter;
        true
    }

    /// A range typed in a dialog (`$A$1:$E$9`, a name, `Sheet2!A1:B2`): (sheet, range).
    fn typed_range(&mut self, text: &str) -> Option<(Option<usize>, Value)> {
        let text = text.trim();
        if text.is_empty() {
            return None;
        }
        let out = self.view.query(json!({ "cmd": "resolve", "text": text }))?;
        let r = out.get("range")?.clone();
        let sheet = out.get("sheet").and_then(Value::as_u64).map(|s| s as usize);
        Some((sheet, r))
    }

    /// Events of the Subtotal, Remove Duplicates and Advanced Filter dialogs.
    pub(super) fn datadlg_event(&mut self, kind: FormKind, ev: &Event) {
        let Some(f) = self.form.as_mut() else {
            return;
        };
        match (kind, ev) {
            (_, Event::Cancel | Event::Clicked("cancel")) => self.form = None,
            (FormKind::RemoveDups, Event::Clicked(id @ ("all" | "none"))) => {
                for c in COLS {
                    f.set_on(c, *id == "all");
                }
            }
            (FormKind::AdvancedFilter, Event::Changed(id @ ("inPlace" | "copy"))) => {
                let copy = *id == "copy";
                f.set_on("inPlace", !copy);
                f.set_on("copy", copy);
                f.enable("dest", copy);
                f.enable("destL", copy);
                f.enable("pickDest", copy);
            }
            (FormKind::Subtotal, Event::Clicked("removeAll")) => {
                let Some([r0, c0, r1, c1]) = self.dd_list else {
                    return;
                };
                self.form = None;
                self.view.run(json!({
                    "cmd": "removeSubtotals",
                    "range": { "r0": r0, "c0": c0, "r1": r1, "c1": c1 },
                }));
            }
            (FormKind::Subtotal, Event::Ok | Event::Clicked("ok")) => {
                let Some([r0, c0, r1, c1]) = self.dd_list else {
                    return;
                };
                let group = c0 + f.sel("group").unwrap_or(0) as u32;
                let func = FUNCTIONS[f.sel("fn").unwrap_or(0).min(FUNCTIONS.len() - 1)].0;
                let columns: Vec<u32> = (0..COLS.len())
                    .filter(|i| f.on(COLS[*i]))
                    .map(|i| c0 + i as u32)
                    .collect();
                if columns.is_empty() {
                    return;
                }
                let (replace, breaks, below) = (f.on("replace"), f.on("breaks"), f.on("below"));
                self.form = None;
                self.view.run(json!({
                    "cmd": "subtotal",
                    "range": { "r0": r0, "c0": c0, "r1": r1, "c1": c1 },
                    "groupBy": group, "function": func, "columns": columns,
                    "replace": replace, "pageBreaks": breaks, "summaryBelow": below,
                }));
            }
            (FormKind::RemoveDups, Event::Ok | Event::Clicked("ok")) => {
                let Some([r0, c0, r1, c1]) = self.dd_list else {
                    return;
                };
                let columns: Vec<u32> = (0..COLS.len())
                    .filter(|i| f.on(COLS[*i]))
                    .map(|i| c0 + i as u32)
                    .collect();
                if columns.is_empty() {
                    return;
                }
                let header = f.on("header");
                self.form = None;
                let out = self.view.query(json!({
                    "cmd": "removeDuplicates",
                    "range": { "r0": r0, "c0": c0, "r1": r1, "c1": c1 },
                    "columns": columns, "header": header,
                }));
                if out.is_some() {
                    self.view.changed();
                    let removed = out
                        .as_ref()
                        .and_then(|v| v.get("removed"))
                        .and_then(Value::as_u64)
                        .unwrap_or(0);
                    let kept = out
                        .as_ref()
                        .and_then(|v| v.get("remaining").or_else(|| v.get("kept")))
                        .and_then(Value::as_u64);
                    self.status = match kept {
                        Some(k) => self
                            .t(
                                "sheetNative.ddDupsDone",
                                "{0} duplicate values found and removed; {1} unique values remain.",
                            )
                            .replace("{0}", &removed.to_string())
                            .replace("{1}", &k.to_string()),
                        None => self
                            .t(
                                "sheetNative.ddDupsRemoved",
                                "{0} duplicate values found and removed.",
                            )
                            .replace("{0}", &removed.to_string()),
                    };
                    self.filter_status = true;
                }
            }
            (FormKind::AdvancedFilter, Event::Ok | Event::Clicked("ok")) => {
                let (list, crit, dest) = (f.text("list"), f.text("crit"), f.text("dest"));
                let (copy, unique) = (f.on("copy"), f.on("unique"));
                let Some((_, list)) = self.typed_range(&list) else {
                    return;
                };
                let Some((crit_sheet, crit)) = self.typed_range(&crit) else {
                    return;
                };
                let dest = if copy {
                    match self.typed_range(&dest) {
                        Some((_, d)) => Some(d),
                        None => return,
                    }
                } else {
                    None
                };
                self.form = None;
                self.view.run(json!({
                    "cmd": "advancedFilter", "list": list, "criteriaSheet": crit_sheet,
                    "criteria": crit, "copyTo": dest, "unique": unique,
                }));
            }
            _ => {}
        }
    }
}
