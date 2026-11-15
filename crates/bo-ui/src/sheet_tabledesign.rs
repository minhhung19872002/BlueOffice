//! Table Design contextual tab of BlueSheet (round 27), shown while the active cell is in a
//! table as the reference shows it: Properties (Table Name, Resize Table), Tools (Summarize with
//! PivotTable, Remove Duplicates, Convert to Range), Table Style Options (Header Row, Total Row,
//! Banded Rows, First Column, Last Column, Banded Columns, Filter Button as check boxes) and
//! Table Styles (the 60-style gallery). Engine: `tables`, `tableUpdate`, `tableResize`,
//! `tableConvert`, `tableTotal`.

use super::form_impl::{self, Event, Form, FormKind, Kind};
use super::SheetWindow;
use crate::icons::Icons;
use crate::ribbon::{Group, Item, RibbonHit, Tab};
use crate::Painter;
use bo_render::Rgba;
use serde_json::{json, Value};

/// The check boxes of Table Style Options: (command, `tables` field).
pub(super) const OPTIONS: [(&str, &str); 7] = [
    ("sheet.tblHeaderRow", "headerRow"),
    ("sheet.tblTotalRow", "totalsRow"),
    ("sheet.tblBandedRows", "bandedRows"),
    ("sheet.tblFirstColumn", "firstColumn"),
    ("sheet.tblLastColumn", "lastColumn"),
    ("sheet.tblBandedColumns", "bandedColumns"),
    ("sheet.tblFilterButton", "filterButton"),
];

/// The Table Design tab.
pub(crate) fn table_tabs() -> Vec<Tab> {
    use Item::{Labeled as S, Large as L};
    vec![Tab {
        label: "sheetTable.tabDesign",
        groups: vec![
            // As the reference at 1500 px (measured, round 27): Table Name label and box,
            // Resize Table; the tools with Insert Slicer; External Table Data (Export /
            // Refresh, not available); the options as three columns of check boxes.
            Group {
                label: "sheetTable.groupProperties",
                large: vec![],
                rows: vec![
                    vec![S("sheet.tblNameLabel")],
                    vec![S("sheet.tblName")],
                    vec![S("sheet.tblResize")],
                ],
            },
            Group {
                label: "sheetTable.groupTools",
                large: vec![L("sheet.slicer")],
                rows: vec![
                    vec![S("sheet.tblPivot")],
                    vec![S("sheet.tblDuplicates")],
                    vec![S("sheet.tblConvert")],
                ],
            },
            Group {
                label: "sheetTable.groupExternal",
                large: vec![L("sheet.tblExport"), L("sheet.tblRefresh")],
                rows: vec![],
            },
            Group {
                label: "sheetTable.groupOptions",
                large: vec![],
                rows: vec![
                    vec![
                        S("sheet.tblHeaderRow"),
                        S("sheet.tblFirstColumn"),
                        S("sheet.tblFilterButton"),
                    ],
                    vec![S("sheet.tblTotalRow"), S("sheet.tblLastColumn")],
                    vec![S("sheet.tblBandedRows"), S("sheet.tblBandedColumns")],
                ],
            },
            Group {
                label: "sheetTable.groupStyles",
                large: vec![L("sheet.tblQuickStyles")],
                rows: vec![],
            },
        ],
    }]
}

/// Fixed widths (logical px) of the Table Design row items, so the option check boxes stand
/// in columns and the name box is as wide as the reference's (130 px at 125 %).
pub(super) fn row_width(id: &str) -> Option<f32> {
    match id {
        "sheet.tblName" => Some(104.0),
        "sheet.tblHeaderRow" | "sheet.tblTotalRow" | "sheet.tblBandedRows" => Some(110.0),
        "sheet.tblFirstColumn" | "sheet.tblLastColumn" | "sheet.tblBandedColumns" => Some(140.0),
        _ => None,
    }
}

/// Own drawings for the Table Design buttons (24-unit grid).
pub(super) fn add_icons(icons: &mut Icons) {
    let white = Rgba(255, 255, 255, 255);
    let blue = Rgba(0x2e, 0x75, 0xb6, 255);
    let pale = Rgba(0xd6, 0xe4, 0xf2, 255);
    let green = Rgba(0x21, 0x73, 0x46, 255);
    icons.add_solid(
        "sheet.tblName",
        &[
            ("M3 7h18v10h-18Z", Some(white)),
            ("M3 7h18v10h-18Z", None),
            ("M6 10v4M6 12h4M10 10v4", None),
        ],
    );
    icons.add_solid(
        "sheet.tblResize",
        &[
            ("M3 3h12v12h-12Z", Some(pale)),
            ("M3 3h12v12h-12ZM3 7h12", None),
            ("M14 14l7 7M17 21h4v-4", None),
        ],
    );
    icons.add_solid(
        "sheet.tblPivot",
        &[
            ("M3 3h18v18h-18Z", Some(white)),
            ("M3 3h18v18h-18ZM3 8h18M8 3v18", None),
            ("M11 11h7v7h-7Z", Some(pale)),
        ],
    );
    icons.add_solid(
        "sheet.tblDuplicates",
        &[
            ("M3 4h12v6h-12ZM3 13h12v6h-12Z", Some(pale)),
            ("M3 4h12v6h-12ZM3 13h12v6h-12Z", None),
            ("M17 14l5 5M22 14l-5 5", None),
        ],
    );
    icons.add_solid(
        "sheet.tblConvert",
        &[
            ("M3 4h10v16h-10Z", Some(pale)),
            ("M3 4h10v16h-10ZM3 9h10M3 14h10", None),
            ("M15 12h7M19 9l3 3l-3 3", None),
        ],
    );
    icons.add_solid(
        "sheet.tblQuickStyles",
        &[
            ("M3 4h18v16h-18Z", Some(white)),
            ("M3 4h18v4h-18Z", Some(green)),
            ("M3 12h18v4h-18Z", Some(Rgba(0xe2, 0xef, 0xda, 255))),
            (
                "M3 4h18v16h-18ZM3 8h18M3 12h18M3 16h18M9 4v16M15 4v16",
                None,
            ),
        ],
    );
    icons.add_solid(
        "sheet.tblExport",
        &[
            ("M3 4h12v14h-12Z", Some(white)),
            ("M3 4h12v14h-12ZM3 8h12M3 12h12", None),
            ("M14 15h8M19 12l3 3l-3 3", None),
        ],
    );
    icons.add_solid(
        "sheet.tblRefresh",
        &[
            ("M4 3h12v18h-12Z", Some(white)),
            ("M4 3h12v18h-12Z", None),
            ("M14 14a5 5 0 1 0 1.5-3.5", None),
        ],
    );
    icons.add_solid("sheet.tblNameLabel", &[]);
    for (id, _) in OPTIONS {
        // Drawn over by a check box; a plain square keeps the place.
        icons.add_solid(id, &[("M5 5h14v14h-14Z", Some(white))]);
    }
    let _ = blue;
}

impl SheetWindow {
    /// The table holding the active cell: (index, its `tables` entry).
    pub(super) fn table_at_active(&mut self) -> Option<(usize, Value)> {
        let (r, c) = self.view.active();
        let hit = {
            let ws = self
                .view
                .engine()
                .workbook()?
                .sheets
                .get(self.view.sheet())?;
            ws.tables
                .iter()
                .position(|t| t.range.contains(bo_model::sheet::CellRef::new(r, c)))?
        };
        let all = self.view.query(json!({ "cmd": "tables" }))?;
        let list = all.get("tables").and_then(Value::as_array).cloned()?;
        let t = list.get(hit)?.clone();
        Some((hit, t))
    }

    /// Whether the active cell is in a table (cheap: the model's table ranges).
    fn in_table(&self) -> bool {
        let (r, c) = self.view.active();
        self.view
            .engine()
            .workbook()
            .and_then(|w| w.sheets.get(self.view.sheet()))
            .is_some_and(|ws| {
                ws.tables
                    .iter()
                    .any(|t| t.range.contains(bo_model::sheet::CellRef::new(r, c)))
            })
    }

    /// Show the Table Design tab while the active cell is in a table (and no chart is
    /// selected), hide it otherwise.
    pub(super) fn sync_table_tab(&mut self) {
        if self.chart_sel.is_some() {
            self.table_tab = false;
            return;
        }
        let want = self.in_table();
        if want != self.table_tab {
            self.table_tab = want;
            self.ribbon
                .set_contextual(if want { table_tabs() } else { Vec::new() }, false);
        }
    }

    /// Checked Table Style Options of the active cell's table.
    pub(super) fn table_checks(&self) -> Vec<(&'static str, bool)> {
        if !self.table_tab {
            return Vec::new();
        }
        let (r, c) = self.view.active();
        let Some(t) = self
            .view
            .engine()
            .workbook()
            .and_then(|w| w.sheets.get(self.view.sheet()))
            .and_then(|ws| {
                ws.tables
                    .iter()
                    .find(|t| t.range.contains(bo_model::sheet::CellRef::new(r, c)))
            })
        else {
            return Vec::new();
        };
        let s = t.style.clone().unwrap_or_default();
        let values = [
            bo_sheet::table::has_header(t),
            bo_sheet::table::has_totals(t),
            s.show_row_stripes,
            s.show_first_column,
            s.show_last_column,
            s.show_column_stripes,
            t.auto_filter.is_some(),
        ];
        OPTIONS
            .iter()
            .zip(values)
            .map(|((id, _), on)| (*id, on))
            .collect()
    }

    /// The Table Name box: the table's name in a white box over its ribbon place.
    fn paint_table_name(&self, p: &mut dyn Painter) {
        let Some([x, y, w, h]) = self.ribbon.rect_of(RibbonHit::Command("sheet.tblName")) else {
            return;
        };
        let (r, c) = self.view.active();
        let name = self
            .view
            .engine()
            .workbook()
            .and_then(|wb| wb.sheets.get(self.view.sheet()))
            .and_then(|ws| {
                ws.tables
                    .iter()
                    .find(|t| t.range.contains(bo_model::sheet::CellRef::new(r, c)))
            })
            .map(|t| t.name.clone())
            .unwrap_or_default();
        let (y, h) = (y + self.px(1.0), h - self.px(2.0));
        p.fill_rect(x, y, w, h, Rgba(0xab, 0xab, 0xab, 255));
        p.fill_rect(x + 1.0, y + 1.0, w - 2.0, h - 2.0, Rgba(255, 255, 255, 255));
        let style = crate::TextStyle {
            size: self.px(12.0),
            bold: false,
        };
        p.text(
            &name,
            x + self.px(4.0),
            (y + (h - style.size * 1.33) / 2.0).round(),
            style,
            Rgba(0x26, 0x26, 0x26, 255),
        );
    }

    /// Check boxes over the Table Style Options items (as the View tab's Show group).
    pub(super) fn paint_table_checks(&self, p: &mut dyn Painter) {
        if self.table_tab {
            self.paint_table_name(p);
        }
        for (id, on) in self.table_checks() {
            let Some([x, y, _, h]) = self.ribbon.rect_of(RibbonHit::Command(id)) else {
                continue;
            };
            let size = self.px(18.0);
            let ix = x + self.px(5.0);
            let iy = y + (h - size) / 2.0;
            p.fill_rect(
                ix - 1.0,
                iy - 1.0,
                size + 2.0,
                size + 2.0,
                self.theme.ribbon,
            );
            let b = self.px(13.0);
            let (bx, by) = ((ix + (size - b) / 2.0).round(), (y + (h - b) / 2.0).round());
            form_impl::paint_check(p, [bx, by, b, b], on, true, false, self.scale);
        }
    }

    /// Table Design commands; `None` for other ids.
    pub(super) fn table_action(&mut self, id: &str, text: &dyn crate::TextMeasure) -> Option<bool> {
        if !id.starts_with("sheet.tbl") {
            return None;
        }
        let (index, t) = self.table_at_active()?;
        if let Some((_, field)) = OPTIONS.iter().find(|(o, _)| *o == id) {
            let on = t.get(*field).and_then(Value::as_bool).unwrap_or(false);
            let mut cmd = json!({ "cmd": "tableUpdate", "table": index });
            cmd[*field] = json!(!on);
            if *field == "totalsRow" && !on {
                cmd["totalsLabel"] = json!(self.t("sheetTable.totalLabel", "Total"));
            }
            let ok = self.view.run(cmd);
            if !ok {
                self.status = self.view.take_error().unwrap_or_default();
            }
            return Some(true);
        }
        Some(match id {
            "sheet.tblName" | "sheet.tblNameLabel" => self.open_table_name(&t),
            "sheet.tblResize" => self.open_table_resize(&t),
            "sheet.tblConvert" => {
                let ok = self
                    .view
                    .run(json!({ "cmd": "tableConvert", "table": index }));
                if ok {
                    self.sync_table_tab();
                }
                ok
            }
            "sheet.tblPivot" => {
                // Create PivotTable with the table's range (the selection becomes the table).
                if let Some(r) = t.get("range") {
                    let n = |k: &str| r.get(k).and_then(Value::as_u64).unwrap_or(0) as u32;
                    self.view
                        .select((n("r0"), n("c0")), (n("r1"), n("c1")), (n("r0"), n("c0")));
                }
                self.action("sheet.pivotTable", text)
            }
            "sheet.tblDuplicates" => self.action("sheet.removeDuplicates", text),
            "sheet.tblQuickStyles" => self.open_gallery(false, text),
            _ => false,
        })
    }

    fn open_table_name(&mut self, t: &Value) -> bool {
        let name = t
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        let tt = |k: &str, d: &str| self.t(k, d);
        let mut f = Form::new(
            &tt("sheetTable.nameTitle", "Table Name"),
            (400.0, 170.0),
            vec![],
        );
        f.add(
            "label",
            None,
            [20.0, 56.0, 0.0, 0.0],
            Kind::Label(tt("sheetTable.nameLabel", "Table Name:")),
        );
        f.add(
            "name",
            None,
            [20.0, 78.0, 360.0, 25.0],
            Kind::Edit {
                caret: name.chars().count(),
                value: name.chars().collect(),
            },
        );
        f.ok_cancel(&tt("common.ok", "OK"), &tt("common.cancel", "Cancel"));
        f.focus("name");
        self.form = Some(f);
        self.form_kind = FormKind::TableName;
        true
    }

    fn open_table_resize(&mut self, t: &Value) -> bool {
        let a1 = t
            .get("range")
            .map(|r| {
                let n = |k: &str| r.get(k).and_then(Value::as_u64).unwrap_or(0) as u32;
                format!(
                    "=${}${}:${}${}",
                    bo_model::sheet::col_to_letters(n("c0")),
                    n("r0") + 1,
                    bo_model::sheet::col_to_letters(n("c1")),
                    n("r1") + 1
                )
            })
            .unwrap_or_default();
        let tt = |k: &str, d: &str| self.t(k, d);
        let mut f = Form::new(
            &tt("sheetTable.resizeTitle", "Resize Table"),
            (440.0, 200.0),
            vec![],
        );
        f.add(
            "label",
            None,
            [20.0, 56.0, 0.0, 0.0],
            Kind::Label(tt(
                "sheetTable.resizeLabel",
                "Select the new data range for your table:",
            )),
        );
        f.add(
            "range",
            None,
            [20.0, 80.0, 400.0, 25.0],
            Kind::Edit {
                caret: a1.chars().count(),
                value: a1.chars().collect(),
            },
        );
        f.add(
            "note",
            None,
            [20.0, 114.0, 400.0, 0.0],
            Kind::Note(tt(
                "sheetTable.resizeNote",
                "Note: The headers must remain in the same row, and the resulting table range must overlap the original table range.",
            )),
        );
        f.ok_cancel(&tt("common.ok", "OK"), &tt("common.cancel", "Cancel"));
        f.focus("range");
        self.form = Some(f);
        self.form_kind = FormKind::TableResize;
        true
    }

    /// Events of the Table Name / Resize Table forms.
    pub(super) fn table_form_event(&mut self, kind: FormKind, ev: &Event) {
        match (kind, ev) {
            (_, Event::Cancel) => self.form = None,
            (FormKind::TableName, Event::Ok) => {
                let name = self
                    .form
                    .as_ref()
                    .map(|f| f.text("name"))
                    .unwrap_or_default();
                if let Some((index, _)) = self.table_at_active() {
                    if self
                        .view
                        .run(json!({ "cmd": "tableUpdate", "table": index, "name": name.trim() }))
                    {
                        self.form = None;
                    } else {
                        self.status = self.view.take_error().unwrap_or_default();
                    }
                }
            }
            (FormKind::TableResize, Event::Ok) => {
                let text = self
                    .form
                    .as_ref()
                    .map(|f| f.text("range"))
                    .unwrap_or_default();
                let cells = text.trim().trim_start_matches('=').replace('$', "");
                let cells = cells.rsplit('!').next().unwrap_or_default().to_owned();
                let Some(r) = bo_model::sheet::CellRange::parse(&cells) else {
                    self.status = self.t("sheetTable.badRange", "The reference isn't valid.");
                    return;
                };
                if let Some((index, _)) = self.table_at_active() {
                    let range = json!({ "r0": r.first.row, "c0": r.first.col,
                        "r1": r.last.row, "c1": r.last.col });
                    if self
                        .view
                        .run(json!({ "cmd": "tableResize", "table": index, "range": range }))
                    {
                        self.form = None;
                    } else {
                        self.status = self.view.take_error().unwrap_or_default();
                    }
                }
            }
            _ => {}
        }
    }
}
