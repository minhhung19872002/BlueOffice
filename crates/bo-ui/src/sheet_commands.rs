//! BlueSheet commands of the ribbon tabs and their drop-down menus (round 2): number
//! formats, styles, editing, insert, formulas, data and view (ADR-0018, N6). Every edit goes
//! through the engine's sheet commands; menu choices arrive as `Choice::Command` ids.
use super::SheetWindow;
use crate::dialog::{Dialog, DialogButton, DialogKind};
use crate::popup::{Choice, Menu, MenuItem};
use crate::ribbon::RibbonHit;
use crate::sheet_ribbon;
use crate::TextMeasure;
use bo_model::sheet::{MAX_COLS, MAX_ROWS};
use serde_json::{json, Value};

/// What the open dialog is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SheetDialog {
    Find,
    Replace,
    GoTo,
    InsertFunction,
    FormatCells,
    /// New comment on the active cell (Review tab).
    Comment,
    /// Text to Columns wizard at step 0 or 1 (Data tab).
    TextToColumns(u8),
}

/// Page of each Format Cells field: number code; horizontal, vertical, indent, orientation;
/// font, size, colour; border preset, line style; fill; then the check boxes wrap, shrink,
/// merge (alignment), bold, italic, underline, strike (font), locked, hidden (protection).
const FORMAT_CELLS_PAGES: [usize; 20] =
    [0, 1, 1, 1, 1, 2, 2, 2, 3, 3, 4, 1, 1, 1, 2, 2, 2, 2, 5, 5];

/// Format parts read from the active cell's style record.
#[derive(Debug, Default)]
struct XfExtra {
    indent: u32,
    rotation: u32,
    shrink: bool,
    locked: bool,
    hidden: bool,
    /// Solid fill colour `RRGGBB` (empty: none).
    fill: String,
}

fn parse_num(s: &str) -> Option<f64> {
    s.trim().replace(',', ".").parse::<f64>().ok()
}

fn hex6(s: &str) -> Option<String> {
    let h = s.trim().trim_start_matches('#').to_ascii_uppercase();
    (h.len() == 6 && h.chars().all(|c| c.is_ascii_hexdigit())).then_some(h)
}

impl SheetWindow {
    /// The engine's worksheet function names (function library menus).
    pub(super) fn load_functions(&mut self) {
        self.functions = self
            .view
            .query(json!({ "cmd": "functionList" }))
            .and_then(|v| {
                v.get("names").and_then(Value::as_array).map(|a| {
                    a.iter()
                        .filter_map(|n| n.as_str().map(str::to_owned))
                        .collect()
                })
            })
            .unwrap_or_default();
    }

    /// Open the drop-down of menu button `id`; `false` when it has none.
    pub(super) fn open_sheet_menu(&mut self, id: &'static str) -> bool {
        // Conditional Formatting has the reference's cascading menu.
        if id == "sheet.tblQuickStyles" {
            return self.open_quick_styles();
        }
        if id == "sheet.conditional" {
            return match self.ribbon.rect_of(RibbonHit::Command(id)) {
                Some(anchor) => self.open_cf_menu(anchor),
                None => false,
            };
        }
        let entries = sheet_ribbon::menu(id, &self.functions);
        if entries.is_empty() {
            return false;
        }
        let Some(anchor) = self.ribbon.rect_of(RibbonHit::Command(id)) else {
            return false;
        };
        let current = if id == "sheet.numberFormat" {
            let cat = sheet_ribbon::number_category(&self.view.format().num_fmt);
            sheet_ribbon::NUMBER_FORMATS
                .iter()
                .find(|(_, k)| *k == cat)
                .map(|(k, _)| format!("numfmt.{k}"))
        } else {
            None
        };
        let items = entries
            .into_iter()
            .map(|(choice, label)| MenuItem {
                label: if label.contains('.') {
                    self.strings.get(&label).to_owned()
                } else {
                    label
                },
                checked: current.as_deref() == Some(choice.as_str()),
                choice: Choice::Command(choice),
                dim: false,
            })
            .collect();
        self.menu = Some(Menu::open(items, anchor, self.size, self.scale));
        true
    }

    /// The selection's current region (a single cell grows to its block of data).
    pub(super) fn region(&mut self) -> Value {
        let range = self.view.rng();
        self.view
            .query(json!({ "cmd": "region", "range": range }))
            .and_then(|v| v.get("range").cloned().or(Some(v)))
            .filter(|v| v.get("r0").is_some())
            .unwrap_or(range)
    }

    fn sort(&mut self, descending: bool) -> bool {
        let range = self.view.rng();
        let Some(plan) = self
            .view
            .query(json!({ "cmd": "sortRegion", "range": range }))
        else {
            return false;
        };
        let target = plan.get("range").cloned().unwrap_or(range);
        let header = plan
            .get("hasHeader")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let col = self.view.active().1;
        self.view.run(json!({
            "cmd": "sort", "range": target, "hasHeader": header,
            "keys": [{ "col": col, "descending": descending }],
        }))
    }

    fn auto_sum(&mut self, function: &str, text: &dyn TextMeasure) -> bool {
        let range = self.view.rng();
        let Some(plan) = self.view.query(json!({
            "cmd": "autoSum", "range": range, "function": function,
        })) else {
            return false;
        };
        let at = |v: &Value, k: &str| v.get(k).and_then(Value::as_u64).unwrap_or(0) as u32;
        match plan.get("mode").and_then(Value::as_str) {
            Some("edit") => {
                let (r, c) = (at(&plan, "row"), at(&plan, "col"));
                self.view.select((r, c), (r, c), (r, c));
                let t = plan.get("text").and_then(Value::as_str).unwrap_or("=SUM()");
                self.view.edit_with(t, text);
                true
            }
            Some("write") => {
                let cells: Vec<Value> = plan
                    .get("cells")
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default();
                let mut cmds =
                    vec![json!({ "cmd": "setInputs", "cells": cells, "sheet": self.view.sheet() })];
                for f in plan
                    .get("formats")
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default()
                {
                    let (r, c) = (at(&f, "row"), at(&f, "col"));
                    cmds.push(json!({
                        "cmd": "format", "sheet": self.view.sheet(),
                        "range": { "r0": r, "c0": c, "r1": r, "c1": c },
                        "patch": { "numFmt": f.get("numFmt").cloned().unwrap_or(Value::Null) },
                    }));
                }
                let ok = self.view.run(json!({ "cmd": "batch", "cmds": cmds }));
                if let Some(s) = plan.get("select") {
                    let (r0, c0, r1, c1) = (at(s, "r0"), at(s, "c0"), at(s, "r1"), at(s, "c1"));
                    let active = self.view.active();
                    self.view.select((r0, c0), (r1, c1), active);
                }
                ok
            }
            Some("table") => {
                let t = at(&plan, "table") as usize;
                self.view
                    .run(json!({ "cmd": "tableUpdate", "table": t, "totalsRow": true }))
            }
            _ => false,
        }
    }

    fn cf(&mut self, which: &str) -> bool {
        let range = self.view.rng();
        let hl = json!({ "fill": "FFC7CE", "fontColor": "9C0006" });
        let rule = match which {
            "greater" | "less" => Value::Null,
            "duplicates" => json!({ "kind": "duplicateValues", "style": hl }),
            "top10" => json!({ "kind": "top10", "rank": 10, "style": hl }),
            "bottom10" => json!({ "kind": "top10", "rank": 10, "bottom": true, "style": hl }),
            "barBlue" => json!({ "kind": "dataBar", "color": "638EC6" }),
            "barGreen" => json!({ "kind": "dataBar", "color": "63BE7B" }),
            "scaleGyr" => json!({
                "kind": "colorScale", "minColor": "F8696B", "midColor": "FFEB84", "maxColor": "63BE7B",
            }),
            "scaleRwb" => json!({
                "kind": "colorScale", "minColor": "5A8AC6", "midColor": "FCFCFF", "maxColor": "F8696B",
            }),
            "icons3Arrows" => json!({ "kind": "iconSet", "set": "3Arrows" }),
            "icons3Traffic" => json!({ "kind": "iconSet", "set": "3TrafficLights1" }),
            "icons5Quarters" => json!({ "kind": "iconSet", "set": "5Quarters" }),
            "clearSel" => return self.view.run(json!({ "cmd": "clearCf", "range": range })),
            "clearSheet" => {
                return self.view.run(json!({
                    "cmd": "clearCf",
                    "range": { "r0": 0, "c0": 0, "r1": MAX_ROWS - 1, "c1": MAX_COLS - 1 },
                }));
            }
            _ => return false,
        };
        // Above / below average: a formula against the selection's average.
        let rule = if matches!(which, "greater" | "less") {
            let (r0, c0, r1, c1) = self.view.range_bounds();
            let a1 = |r: u32, c: u32| format!("{}{}", bo_model::sheet::col_to_letters(c), r + 1);
            let abs = |r: u32, c: u32| format!("${}${}", bo_model::sheet::col_to_letters(c), r + 1);
            let op = if which == "greater" { ">" } else { "<" };
            json!({
                "kind": "expression",
                "formula": format!("{}{op}AVERAGE({}:{})", a1(r0, c0), abs(r0, c0), abs(r1, c1)),
                "style": hl,
            })
        } else {
            rule
        };
        self.view
            .run(json!({ "cmd": "addCf", "range": range, "rule": rule }))
    }

    fn freeze(&mut self, which: &str) -> bool {
        let (r, c) = self.view.active();
        let top = self.view.top_left();
        let (rows, cols) = match which {
            "panes" => (
                r.saturating_sub(top.0) + top.0,
                c.saturating_sub(top.1) + top.1,
            ),
            "row" => (top.0 + 1, 0),
            "col" => (0, top.1 + 1),
            _ => (0, 0),
        };
        self.view
            .run(json!({ "cmd": "freeze", "rows": rows, "cols": cols }))
    }

    /// Open one of BlueSheet's dialogs.
    pub(super) fn open_sheet_dialog(&mut self, kind: SheetDialog) -> bool {
        let ok = self.t("common.ok", "OK");
        let cancel = self.t("common.cancel", "Cancel");
        let (dk, title, fields, buttons) = match kind {
            SheetDialog::Find => (
                DialogKind::Find,
                self.t("sheetMenu.dialog.find", "Find"),
                vec![(
                    self.t("sheetMenu.dialog.findWhat", "Find what"),
                    String::new(),
                )],
                vec![
                    (
                        self.t("sheetMenu.dialog.findNext", "Find next"),
                        DialogButton::FindNext,
                    ),
                    (cancel, DialogButton::Cancel),
                ],
            ),
            SheetDialog::Replace => (
                DialogKind::Replace,
                self.t("sheetMenu.dialog.replace", "Replace"),
                vec![
                    (
                        self.t("sheetMenu.dialog.findWhat", "Find what"),
                        String::new(),
                    ),
                    (
                        self.t("sheetMenu.dialog.replaceWith", "Replace with"),
                        String::new(),
                    ),
                ],
                vec![
                    (
                        self.t("sheetMenu.dialog.findNext", "Find next"),
                        DialogButton::FindNext,
                    ),
                    (
                        self.t("sheetMenu.dialog.replaceAll", "Replace all"),
                        DialogButton::ReplaceAll,
                    ),
                    (cancel, DialogButton::Cancel),
                ],
            ),
            SheetDialog::GoTo => (
                DialogKind::Link,
                self.t("sheetMenu.dialog.goto", "Go to"),
                vec![(
                    self.t("sheetMenu.dialog.reference", "Reference"),
                    String::new(),
                )],
                vec![(ok, DialogButton::Ok), (cancel, DialogButton::Cancel)],
            ),
            SheetDialog::InsertFunction => (
                DialogKind::Link,
                self.t("commands.sheet.insertFunction", "Insert function"),
                vec![(
                    self.t("sheetMenu.dialog.function", "Function"),
                    "SUM".to_owned(),
                )],
                vec![(ok, DialogButton::Ok), (cancel, DialogButton::Cancel)],
            ),
            SheetDialog::Comment | SheetDialog::TextToColumns(_) => {
                return self.open_review_dialog(kind);
            }
            SheetDialog::FormatCells => (
                DialogKind::Paragraph,
                self.t("sheetMenu.cells.formatCells", "Format cells")
                    .replace('…', ""),
                self.format_cells_fields(),
                vec![(ok, DialogButton::Ok), (cancel, DialogButton::Cancel)],
            ),
        };
        let mut d = Dialog::new(dk, title, fields, buttons, Vec::new());
        if kind == SheetDialog::FormatCells {
            d = self.format_cells_pages(d);
        }
        self.dialog = Some((kind, d));
        true
    }

    /// A dialog button was chosen.
    pub(super) fn sheet_dialog_button(&mut self, b: DialogButton, text: &dyn TextMeasure) -> bool {
        let Some((kind, d)) = self.dialog.as_ref() else {
            return false;
        };
        let kind = *kind;
        let values = d.values();
        let v = |i: usize| values.get(i).cloned().unwrap_or_default();
        if b == DialogButton::Cancel {
            self.dialog = None;
            return true;
        }
        match kind {
            SheetDialog::Find | SheetDialog::Replace if b == DialogButton::FindNext => {
                let found = self.find_next(&v(0));
                let msg = if found {
                    Vec::new()
                } else {
                    vec![self.t("sheetMenu.dialog.notFound", "Nothing found.")]
                };
                if let Some((_, d)) = self.dialog.as_mut() {
                    d.set_message(msg);
                }
                true
            }
            SheetDialog::Replace if b == DialogButton::ReplaceAll => {
                let out = self.view.query(json!({
                    "cmd": "replace", "query": v(0), "with": v(1),
                }));
                let n = out
                    .as_ref()
                    .and_then(|o| o.get("replaced").or_else(|| o.get("count")))
                    .and_then(Value::as_u64)
                    .unwrap_or(0);
                self.view.changed();
                let msg = format!("{} {n}", self.t("sheetMenu.dialog.replaced", "Replaced:"));
                if let Some((_, d)) = self.dialog.as_mut() {
                    d.set_message(vec![msg]);
                }
                true
            }
            SheetDialog::GoTo => {
                self.dialog = None;
                self.go_to(&v(0))
            }
            SheetDialog::InsertFunction => {
                self.dialog = None;
                let name = v(0).trim().to_ascii_uppercase();
                if name.is_empty() {
                    return true;
                }
                self.view.edit_with(&format!("={name}("), text);
                true
            }
            SheetDialog::FormatCells => {
                self.dialog = None;
                self.apply_format_cells(&values);
                true
            }
            _ => self.review_dialog_button(kind, b, &values),
        }
    }

    /// Text fields of the tabbed Format Cells dialog (Number, Alignment, Font, Border, Fill;
    /// the check boxes follow in [`Self::format_cells_pages`]).
    fn format_cells_fields(&self) -> Vec<(String, String)> {
        let f = self.view.format().clone();
        let x = self.xf_extra();
        vec![
            (
                self.t("sheetMenu.dialog.numberCode", "Number format code"),
                if f.num_fmt.is_empty() {
                    "General".to_owned()
                } else {
                    f.num_fmt.clone()
                },
            ),
            (
                self.t("sheetMenu.fcHorizontal", "Horizontal"),
                if f.h_align.is_empty() {
                    "general".to_owned()
                } else {
                    f.h_align.clone()
                },
            ),
            (
                self.t("sheetMenu.fcVertical", "Vertical"),
                if f.v_align.is_empty() {
                    "bottom".to_owned()
                } else {
                    f.v_align.clone()
                },
            ),
            (self.t("sheetMenu.fcIndent", "Indent"), x.indent.to_string()),
            (
                self.t("sheetMenu.fcRotation", "Orientation"),
                x.rotation.to_string(),
            ),
            (self.t("sheetMenu.dialog.font", "Font"), f.font.clone()),
            (
                self.t("sheetMenu.dialog.size", "Size (pt)"),
                format!("{}", f.size),
            ),
            (
                self.t("sheetMenu.dialog.fontColor", "Font colour (RRGGBB)"),
                f.color.clone(),
            ),
            (self.t("sheetMenu.fcBorderPreset", "Presets"), String::new()),
            (
                self.t("sheetMenu.fcBorderStyle", "Line style"),
                "thin".to_owned(),
            ),
            (self.t("sheetMenu.fcFillColor", "Background colour"), x.fill),
        ]
    }

    /// Check boxes and pages of the Format Cells dialog.
    fn format_cells_pages(&self, d: Dialog) -> Dialog {
        let f = self.view.format().clone();
        let x = self.xf_extra();
        let d = d.with_checks(vec![
            (self.t("sheetMenu.dialog.wrap", "Wrap text"), f.wrap),
            (self.t("sheetMenu.fcShrink", "Shrink to fit"), x.shrink),
            (self.t("sheetMenu.fcMerge", "Merge cells"), self.merged()),
            (self.t("sheetMenu.dialog.bold", "Bold"), f.bold),
            (self.t("sheetMenu.dialog.italic", "Italic"), f.italic),
            (self.t("sheetMenu.fcUnderline", "Underline"), f.underline),
            (self.t("sheetMenu.fcStrike", "Strikethrough"), f.strike),
            (self.t("sheetMenu.fcLocked", "Locked"), x.locked),
            (self.t("sheetMenu.fcHidden", "Hidden"), x.hidden),
        ]);
        let tabs = [
            ("sheetMenu.fcTabNumber", "Number"),
            ("sheetMenu.fcTabAlignment", "Alignment"),
            ("sheetMenu.fcTabFont", "Font"),
            ("sheetMenu.fcTabBorder", "Border"),
            ("sheetMenu.fcTabFill", "Fill"),
            ("sheetMenu.fcTabProtection", "Protection"),
        ]
        .iter()
        .map(|(k, fb)| self.t(k, fb))
        .collect();
        d.with_tabs(tabs, &FORMAT_CELLS_PAGES)
    }

    /// Whether the selection touches merged cells.
    fn merged(&self) -> bool {
        let (r0, c0, r1, c1) = self.view.range_bounds();
        self.view.engine().workbook().is_some_and(|w| {
            w.sheets.get(self.view.sheet()).is_some_and(|ws| {
                ws.merges.iter().any(|m| {
                    m.first.row <= r1 && m.last.row >= r0 && m.first.col <= c1 && m.last.col >= c0
                })
            })
        })
    }

    /// Cell format parts the view's summary does not carry (active cell).
    fn xf_extra(&self) -> XfExtra {
        let (r, c) = self.view.active();
        let mut out = XfExtra {
            locked: true,
            ..XfExtra::default()
        };
        let Some(w) = self.view.engine().workbook() else {
            return out;
        };
        let style = w
            .sheets
            .get(self.view.sheet())
            .and_then(|ws| ws.cell(r, c))
            .map_or(0, |cell| cell.style);
        if let Some(xf) = w.styles.xf(style) {
            if let Some(a) = &xf.alignment {
                out.indent = a.indent.unwrap_or(0);
                out.rotation = a.text_rotation.unwrap_or(0);
                out.shrink = a.shrink_to_fit;
            }
            if let Some(p) = &xf.protection {
                out.locked = p.locked.unwrap_or(true);
                out.hidden = p.hidden.unwrap_or(false);
            }
            out.fill = w
                .styles
                .fills
                .get(xf.fill_id as usize)
                .and_then(|f| match f {
                    bo_model::sheet::Fill::Pattern(p) if p.pattern.as_deref() == Some("solid") => {
                        p.fg.as_ref()
                            .and_then(|c| c.rgb.as_ref())
                            .map(|rgb| rgb[rgb.len().saturating_sub(6)..].to_ascii_uppercase())
                    }
                    _ => None,
                })
                .unwrap_or_default();
        }
        out
    }

    fn apply_format_cells(&mut self, values: &[String]) {
        let v = |i: usize| values.get(i).cloned().unwrap_or_default();
        let on = |i: usize| v(i) == "1";
        let f = self.view.format().clone();
        let x = self.xf_extra();
        let mut patch = serde_json::Map::new();
        let code = v(0).trim().to_owned();
        if !code.is_empty() && code != f.num_fmt && !(f.num_fmt.is_empty() && code == "General") {
            patch.insert("numFmt".into(), json!(code));
        }
        let ha = v(1).trim().to_ascii_lowercase();
        let ha = if ha == "general" { String::new() } else { ha };
        if ha != f.h_align && matches!(ha.as_str(), "" | "left" | "center" | "right" | "justify") {
            patch.insert("hAlign".into(), json!(ha));
        }
        let va = v(2).trim().to_ascii_lowercase();
        if va != f.v_align
            && !(va == "bottom" && f.v_align.is_empty())
            && matches!(va.as_str(), "top" | "center" | "bottom")
        {
            patch.insert("vAlign".into(), json!(va));
        }
        if let Some(n) = v(3).trim().parse::<u32>().ok().filter(|n| *n <= 250) {
            if n != x.indent {
                patch.insert("indent".into(), json!(n));
            }
        }
        if let Some(n) = v(4)
            .trim()
            .parse::<u32>()
            .ok()
            .filter(|n| *n <= 180 || *n == 255)
        {
            if n != x.rotation {
                patch.insert("rotation".into(), json!(n));
            }
        }
        let font = v(5).trim().to_owned();
        if !font.is_empty() && font != f.font {
            patch.insert("fontName".into(), json!(font));
        }
        if let Some(pt) = parse_num(&v(6)).filter(|p| (1.0..=409.0).contains(p)) {
            if (pt - f.size).abs() > 1e-6 {
                patch.insert("fontSize".into(), json!(pt));
            }
        }
        if let Some(h) = hex6(&v(7)) {
            if h != f.color {
                patch.insert("fontColor".into(), json!(h));
            }
        }
        let fill = v(10).trim().to_ascii_lowercase();
        if fill == "none" && !x.fill.is_empty() {
            patch.insert("fillColor".into(), json!(""));
        } else if let Some(h) = hex6(&fill) {
            if h != x.fill {
                patch.insert("fillColor".into(), json!(h));
            }
        }
        for (i, key, was) in [
            (11, "wrap", f.wrap),
            (12, "shrink", x.shrink),
            (14, "bold", f.bold),
            (15, "italic", f.italic),
            (17, "strike", f.strike),
            (18, "locked", x.locked),
            (19, "formulaHidden", x.hidden),
        ] {
            if on(i) != was {
                patch.insert(key.into(), json!(on(i)));
            }
        }
        if on(16) != f.underline {
            patch.insert(
                "underline".into(),
                json!(if on(16) { "single" } else { "none" }),
            );
        }
        let range = self.view.rng();
        let sheet = self.view.sheet();
        let mut cmds = Vec::new();
        if !patch.is_empty() {
            cmds.push(json!({ "cmd": "format", "sheet": sheet, "range": range, "patch": patch }));
        }
        if on(13) != self.merged() {
            cmds.push(if on(13) {
                json!({ "cmd": "merge", "sheet": sheet, "range": range, "center": false })
            } else {
                json!({ "cmd": "unmerge", "sheet": sheet, "range": range })
            });
        }
        let preset = v(8).trim().to_ascii_lowercase();
        if matches!(
            preset.as_str(),
            "none" | "outline" | "all" | "inside" | "top" | "bottom" | "left" | "right"
        ) {
            let style = v(9).trim().to_ascii_lowercase();
            let style = if matches!(
                style.as_str(),
                "thin" | "medium" | "thick" | "dashed" | "dotted" | "double"
            ) {
                style
            } else {
                "thin".to_owned()
            };
            cmds.push(json!({
                "cmd": "borders", "sheet": sheet, "range": range,
                "spec": { "preset": preset, "style": style },
            }));
        }
        if !cmds.is_empty() {
            self.view.run(json!({ "cmd": "batch", "cmds": cmds }));
        }
    }

    /// Select the next cell after the active one whose text contains `query`.
    fn find_next(&mut self, query: &str) -> bool {
        if query.is_empty() {
            return false;
        }
        let sheet = self.view.sheet();
        let Some(out) = self
            .view
            .query(json!({ "cmd": "find", "query": query, "sheet": sheet }))
        else {
            return false;
        };
        let at = |m: &Value, k: &str| m.get(k).and_then(Value::as_u64).unwrap_or(0) as u32;
        let list: Vec<(u32, u32)> = out
            .get("matches")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter(|m| {
                        m.get("sheet")
                            .and_then(Value::as_u64)
                            .is_none_or(|s| s as usize == sheet)
                    })
                    .map(|m| (at(m, "row"), at(m, "col")))
                    .collect()
            })
            .unwrap_or_default();
        let cur = self.view.active();
        let next = list
            .iter()
            .copied()
            .find(|p| *p > cur)
            .or_else(|| list.first().copied());
        match next {
            Some(p) => {
                self.view.select(p, p, p);
                true
            }
            None => false,
        }
    }

    /// Go to a reference or name typed in Go To or the name box.
    pub(super) fn go_to(&mut self, text: &str) -> bool {
        let sheet = self.view.sheet();
        let Some(out) = self
            .view
            .query(json!({ "cmd": "resolve", "sheet": sheet, "text": text.trim() }))
        else {
            return false;
        };
        let Some(r) = out.get("range") else {
            return false;
        };
        if let Some(s) = out.get("sheet").and_then(Value::as_u64) {
            self.view.set_sheet(s as usize);
        }
        let at = |k: &str| r.get(k).and_then(Value::as_u64).unwrap_or(0) as u32;
        self.view.select(
            (at("r0"), at("c0")),
            (at("r1"), at("c1")),
            (at("r0"), at("c0")),
        );
        true
    }

    fn zoom_selection(&mut self, text: &dyn TextMeasure) -> bool {
        let Some((w, h)) = self.view.selection_size_pt() else {
            return false;
        };
        let (vw, vh) = self.view.view_size_pt();
        let z = (self.view.zoom() as f64 * (vw / w.max(1.0)).min(vh / h.max(1.0))).clamp(0.1, 4.0);
        let z = ((z * 100.0).floor() / 100.0) as f32;
        let changed = self.zoom_to(z, text);
        let (r0, c0, _, _) = self.view.range_bounds();
        self.view.scroll_to_cell(r0, c0);
        changed
    }

    /// Run a BlueSheet ribbon command or menu choice; `None` when `id` is not one.
    #[allow(clippy::too_many_lines)]
    pub(super) fn sheet_action(&mut self, id: &str, text: &dyn TextMeasure) -> Option<bool> {
        if matches!(id, "sheet.cellStyles" | "sheet.formatTable") {
            return Some(self.open_gallery(id == "sheet.cellStyles", text));
        }
        if let Some(r) = self.review_tab_action(id) {
            return Some(r);
        }
        if let Some(r) = self.datadlg_action(id) {
            return Some(r);
        }
        if let Some(r) = self.chart_action(id) {
            return Some(r);
        }
        if let Some(r) = self.table_action(id, text) {
            return Some(r);
        }
        if let Some(r) = self.theme_action(id) {
            return Some(r);
        }
        if id == "sheet.pasteSpecial" {
            self.view.commit();
            return Some(self.open_paste_special());
        }
        if let Some(r) = self.illus_action(id) {
            return Some(r);
        }
        if let Some(r) = self.data_action(id, text) {
            return Some(r);
        }
        if let Some(static_id) = sheet_ribbon::tab_ids().into_iter().find(|i| *i == id) {
            if self.open_sheet_menu(static_id) {
                return Some(true);
            }
        }
        if let Some(r) = self.audit_action(id, text) {
            return Some(r);
        }
        if let Some(r) = self.gallery_action(id) {
            return Some(r);
        }
        if let Some(r) = self.insert_action(id) {
            return Some(r);
        }
        let (kind, arg) = id.split_once('.').unwrap_or((id, ""));
        let range = self.view.rng();
        Some(match (kind, arg) {
            ("numfmt", k) => match sheet_ribbon::number_code(k) {
                Some(code) => self.patch(json!({ "numFmt": code })),
                None => false,
            },
            ("autosum", f) => self.auto_sum(f, text),
            ("fill", "down" | "right") => self
                .view
                .run(json!({ "cmd": "fillDir", "range": range, "dir": arg })),
            ("fill", "up" | "left") => {
                let (r0, c0, r1, c1) = self.view.range_bounds();
                let source = if arg == "up" {
                    json!({ "r0": r1, "c0": c0, "r1": r1, "c1": c1 })
                } else {
                    json!({ "r0": r0, "c0": c1, "r1": r1, "c1": c1 })
                };
                self.view.run(json!({
                    "cmd": "autofill", "source": source, "target": range, "mode": "copy",
                }))
            }
            ("fill", "series") => self.view.run(json!({
                "cmd": "fillSeries", "range": range, "columns": true, "kind": "linear", "step": 1.0,
            })),
            ("clear", "all") => self.view.clear(true, true),
            ("clear", "formats") => self.view.clear(false, true),
            ("clear", "contents") => self.view.clear(true, false),
            ("sort", "asc") | ("sheet", "sortAsc") => self.sort(false),
            ("sort", "desc") | ("sheet", "sortDesc") => self.sort(true),
            ("filter", "toggle") | ("sheet", "filter") => {
                let region = self.region();
                self.view
                    .run(json!({ "cmd": "toggleFilter", "range": region }))
            }
            ("filter", "clear") => self.view.run(json!({ "cmd": "showAllData" })),
            ("find", "find") => self.open_sheet_dialog(SheetDialog::Find),
            ("find", "replace") => self.open_sheet_dialog(SheetDialog::Replace),
            ("find", "goto") => self.open_sheet_dialog(SheetDialog::GoTo),
            ("cf", which) => self.cf(which),
            ("table", style) => {
                // In a table the style applies to it, as the reference's gallery does.
                if let Some((i, _)) = self.table_at_active() {
                    return Some(
                        self.view
                            .run(json!({ "cmd": "tableUpdate", "table": i, "style": style })),
                    );
                }
                let region = self.region();
                let made = self
                    .view
                    .query(json!({ "cmd": "createTable", "range": region }));
                let Some(i) = made
                    .as_ref()
                    .and_then(|m| m.get("index"))
                    .and_then(Value::as_u64)
                else {
                    return Some(false);
                };
                self.view.changed();
                self.view
                    .run(json!({ "cmd": "tableUpdate", "table": i, "style": style }))
            }
            ("style", name) => self
                .view
                .run(json!({ "cmd": "cellStyle", "range": range, "name": name })),
            ("cells", "deleteCol") => {
                let (_, c0, _, c1) = self.view.range_bounds();
                self.view.run(json!({
                    "cmd": "shiftCells", "down": false, "insert": false,
                    "range": { "r0": 0, "c0": c0, "r1": MAX_ROWS - 1, "c1": c1 },
                }))
            }
            ("cells", "rowHeight") => {
                let (r0, _, r1, _) = self.view.range_bounds();
                self.view
                    .run(json!({ "cmd": "autofit", "rows": true, "first": r0, "last": r1 }))
            }
            ("cells", which @ ("hideRows" | "hideCols" | "unhideRows" | "unhideCols")) => {
                let (r0, c0, r1, c1) = self.view.range_bounds();
                let rows = which.ends_with("Rows");
                let (first, last) = if rows { (r0, r1) } else { (c0, c1) };
                self.view.run(json!({
                    "cmd": "hide", "rows": rows, "first": first, "last": last,
                    "hidden": which.starts_with("hide"),
                }))
            }
            ("chart", kind) => {
                let region = self.region();
                let made = self
                    .view
                    .run(json!({ "cmd": "insertChart", "range": region, "kind": kind }));
                if made {
                    // As the reference: the new chart is selected with Chart Design shown.
                    self.select_new_chart();
                }
                made
            }
            ("freeze", which) => self.freeze(which),
            ("fn", name) => {
                self.view.edit_with(&format!("={name}("), text);
                true
            }
            ("sheet", "addSheet") => self.view.add_sheet(),
            ("sheet", "formatCellsDialog") => self.open_format_cells(0),
            ("sheet", "insertFunction") => self.open_sheet_dialog(SheetDialog::InsertFunction),
            ("sheet", "table") => {
                let region = self.region();
                self.view
                    .run(json!({ "cmd": "createTable", "range": region }))
            }
            ("sheet", "calcNow" | "calcSheet") => self.calculate(arg == "calcSheet"),

            ("sheet", "textToColumns") => self.open_ttc(),
            ("sheet", "removeDuplicates") => {
                let region = self.region();
                let header = self
                    .view
                    .query(json!({ "cmd": "sortRegion", "range": region }))
                    .and_then(|p| p.get("hasHeader").and_then(Value::as_bool))
                    .unwrap_or(false);
                let c0 = region.get("c0").and_then(Value::as_u64).unwrap_or(0);
                let c1 = region.get("c1").and_then(Value::as_u64).unwrap_or(0);
                let columns: Vec<u64> = (c0..=c1).collect();
                self.view.run(json!({
                    "cmd": "removeDuplicates", "range": region, "columns": columns, "header": header,
                }))
            }
            ("sheet", "gridlines") => {
                let on = !self.view.gridlines();
                self.view.set_gridlines(on);
                true
            }
            ("sheet", "headings") => {
                let on = !self.view.headings();
                self.view.set_headings(on);
                true
            }
            ("sheet", "formulaBar") => {
                self.show_fbar = !self.show_fbar;
                self.relayout(text);
                true
            }
            ("sheet", "zoom100") => self.zoom_to(1.0, text),
            ("sheet", "zoomSelection") => self.zoom_selection(text),
            _ => return self.more_action(kind, arg, text),
        })
    }

    /// The grid's context menu at (`x`, `y`): clipboard, then insert / delete / clear,
    /// then formatting, for cells, whole rows or whole columns.
    pub(super) fn context_menu(&mut self, x: f32, y: f32) {
        let lines = self.view.whole_lines();
        let mut ids: Vec<(&str, &str, Option<&'static str>)> = vec![
            ("edit.cut", "commands.edit.cut", Some("edit.cut")),
            ("edit.copy", "commands.edit.copy", Some("edit.copy")),
            ("edit.paste", "commands.edit.paste", Some("edit.paste")),
            ("sheet.pasteSpecial", "commands.sheet.pasteSpecial", None),
        ];
        let mut separators = vec![4];
        match lines {
            Some(true) => ids.extend([
                (
                    "cells.insertRow",
                    "sheetMenu.cells.insertRows",
                    Some("cells.insertRow"),
                ),
                (
                    "cells.deleteRow",
                    "sheetMenu.cells.deleteRows",
                    Some("cells.deleteRow"),
                ),
                (
                    "clear.contents",
                    "sheetMenu.clear.contents",
                    Some("edit.clear"),
                ),
            ]),
            Some(false) => ids.extend([
                (
                    "cells.insertCol",
                    "sheetMenu.cells.insertCols",
                    Some("cells.insertCol"),
                ),
                (
                    "cells.deleteCol",
                    "sheetMenu.cells.deleteCols",
                    Some("cells.deleteCol"),
                ),
                (
                    "clear.contents",
                    "sheetMenu.clear.contents",
                    Some("edit.clear"),
                ),
            ]),
            None => ids.extend([
                (
                    "cells.insertRow",
                    "sheetMenu.cells.insertRows",
                    Some("cells.insertRow"),
                ),
                (
                    "cells.insertCol",
                    "sheetMenu.cells.insertCols",
                    Some("cells.insertCol"),
                ),
                (
                    "cells.deleteRow",
                    "sheetMenu.cells.deleteRows",
                    Some("cells.deleteRow"),
                ),
                (
                    "cells.deleteCol",
                    "sheetMenu.cells.deleteCols",
                    Some("cells.deleteCol"),
                ),
                (
                    "clear.contents",
                    "sheetMenu.clear.contents",
                    Some("edit.clear"),
                ),
            ]),
        }
        separators.push(ids.len());
        if let Some(rows) = lines {
            ids.push((
                "sheet.formatCellsDialog",
                "sheetMenu.cells.formatCells",
                Some("format.cells"),
            ));
            if rows {
                ids.extend([
                    (
                        "cells.rowHeight",
                        "sheetMenu.cells.autofitRow",
                        Some("cells.autofitRow"),
                    ),
                    (
                        "cells.hideRows",
                        "sheetMenu.cells.hideRows",
                        Some("cells.hideRows"),
                    ),
                    (
                        "cells.unhideRows",
                        "sheetMenu.cells.unhideRows",
                        Some("cells.unhideRows"),
                    ),
                ]);
            } else {
                ids.extend([
                    (
                        "cells.autofitCol",
                        "sheetMenu.cells.autofitCol",
                        Some("cells.autofitCol"),
                    ),
                    (
                        "cells.hideCols",
                        "sheetMenu.cells.hideCols",
                        Some("cells.hideCols"),
                    ),
                    (
                        "cells.unhideCols",
                        "sheetMenu.cells.unhideCols",
                        Some("cells.unhideCols"),
                    ),
                ]);
            }
        } else {
            ids.extend([
                ("sort.asc", "sheetMenu.sort.asc", Some("data.sortAsc")),
                ("sort.desc", "sheetMenu.sort.desc", Some("data.sortDesc")),
            ]);
            separators.push(ids.len());
            ids.push((
                "sheet.formatCellsDialog",
                "sheetMenu.cells.formatCells",
                Some("format.cells"),
            ));
        }
        let items = ids
            .iter()
            .map(|(id, key, _)| MenuItem {
                label: self.strings.get(key).to_owned(),
                choice: Choice::Command((*id).to_owned()),
                checked: false,
                dim: false,
            })
            .collect();
        let icons = ids.iter().map(|(_, _, icon)| *icon).collect();
        self.menu = Some(
            Menu::open(items, [x, y - 2.0, 1.0, 0.0], self.size, self.scale)
                .with_decor(icons, separators),
        );
    }
}
