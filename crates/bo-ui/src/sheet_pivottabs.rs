//! PivotTable Analyze and Design contextual tabs of BlueSheet (round 13), shown while the
//! active cell is in a pivot table, laid out as the reference shows them at 1500 px (measured
//! on a copy, `show/sheet-8x`):
//! - Analyze: PivotTable ▾, Active Field (the field of the active cell, Field Settings),
//!   Group ▾, Filter (Insert Slicer, Insert Timeline, Filter Connections), Data (Refresh ▾,
//!   Change Data Source ▾), Actions ▾ (Clear All, Select Entire PivotTable, Move
//!   PivotTable…), Calculations ▾, Tools (PivotChart, Recommended PivotTables), Show ▾ (Field
//!   List).
//! - Design: Layout (Subtotals ▾, Grand Totals ▾, Report Layout ▾, Blank Rows ▾), PivotTable
//!   Style Options (Row Headers, Column Headers, Banded Rows, Banded Columns) and an inline
//!   row of PivotTable style tiles with a More button.
//!
//! Every change is one `pivotUpdate` (ECMA-376 Part 1, §18.10.1.73 `pivotTableDefinition`
//! `@rowGrandTotals` / `@colGrandTotals` / `@compact` / `@outline`, §18.10.1.69
//! `pivotField/@defaultSubtotal` / `@subtotalTop` / `@insertBlankRow`, §18.10.7.74
//! `pivotTableStyleInfo`) with the report's columns fitted in the same undo step.

use super::form_impl::{Event, Form, FormKind, Kind};
use super::SheetWindow;
use crate::icons::Icons;
use crate::ribbon::{Group, Item, RibbonHit, Tab};
use crate::Painter;
use bo_render::Rgba;
use serde_json::{json, Value};

/// Inline style tiles on the Design tab (the reference shows seven and a More button at
/// 1500 px).
pub(crate) const STYLE_TILES: [&str; 7] = [
    "sheet.pvtStyle0",
    "sheet.pvtStyle1",
    "sheet.pvtStyle2",
    "sheet.pvtStyle3",
    "sheet.pvtStyle4",
    "sheet.pvtStyle5",
    "sheet.pvtStyle6",
];

/// Styles of the inline tiles: the light family around the default (Light16, third as the
/// reference's gallery shows it), drawn with BlueSheet's own palettes.
const TILE_STYLES: [&str; 7] = [
    "PivotStyleLight14",
    "PivotStyleLight15",
    "PivotStyleLight16",
    "PivotStyleLight17",
    "PivotStyleLight18",
    "PivotStyleLight19",
    "PivotStyleLight20",
];

/// The check boxes of PivotTable Style Options: (command, `style` field).
pub(super) const OPTIONS: [(&str, &str); 4] = [
    ("sheet.pvtRowHeaders", "rowHeaders"),
    ("sheet.pvtColHeaders", "colHeaders"),
    ("sheet.pvtBandedRows", "rowStripes"),
    ("sheet.pvtBandedCols", "colStripes"),
];

/// Menu buttons of the two tabs.
pub(super) const MENUS: [&str; 12] = [
    "sheet.pvtPivotTable",
    "sheet.pvtGroup",
    "sheet.pvtRefresh",
    "sheet.pvtChangeSource",
    "sheet.pvtActions",
    "sheet.pvtCalculations",
    "sheet.pvtShow",
    "sheet.pvtSubtotals",
    "sheet.pvtGrandTotals",
    "sheet.pvtReportLayout",
    "sheet.pvtBlankRows",
    "sheet.pvtStylesMore",
];

/// The PivotTable Analyze and Design tabs.
pub(crate) fn pivot_tabs() -> Vec<Tab> {
    use Item::{Button as B, Labeled as S, Large as L};
    vec![
        Tab {
            label: "sheetPvt.tabAnalyze",
            groups: vec![
                // Groups the reference collapses at 1500 px show as one large menu button
                // without a group caption.
                Group {
                    label: "sheetPvt.none",
                    large: vec![L("sheet.pvtPivotTable")],
                    rows: vec![],
                },
                Group {
                    label: "sheetPvt.groupActiveField",
                    large: vec![L("sheet.pvtShowDetails")],
                    // The expand / collapse field buttons stand in a column after Show
                    // Details (late rows).
                    rows: vec![
                        vec![S("sheet.pvtActiveLabel"), B("sheet.pvtExpand")],
                        vec![S("sheet.pvtActiveField"), B("sheet.pvtCollapse")],
                        vec![S("sheet.pvtFieldSettings")],
                    ],
                },
                Group {
                    label: "sheetPvt.none",
                    large: vec![L("sheet.pvtGroup")],
                    rows: vec![],
                },
                Group {
                    label: "sheetPvt.groupFilter",
                    large: vec![],
                    rows: vec![
                        vec![S("sheet.pvtSlicer")],
                        vec![S("sheet.pvtTimeline")],
                        vec![S("sheet.pvtFilterConn")],
                    ],
                },
                Group {
                    label: "sheetPvt.groupData",
                    large: vec![L("sheet.pvtRefresh"), L("sheet.pvtChangeSource")],
                    rows: vec![],
                },
                Group {
                    label: "sheetPvt.none",
                    large: vec![L("sheet.pvtActions")],
                    rows: vec![],
                },
                Group {
                    label: "sheetPvt.none",
                    large: vec![L("sheet.pvtCalculations")],
                    rows: vec![],
                },
                Group {
                    label: "sheetPvt.groupTools",
                    large: vec![L("sheet.pvtChart"), L("sheet.pvtRecommended")],
                    rows: vec![],
                },
                Group {
                    label: "sheetPvt.none",
                    large: vec![L("sheet.pvtShow")],
                    rows: vec![],
                },
            ],
        },
        Tab {
            label: "sheetPvt.tabDesign",
            groups: vec![
                Group {
                    label: "sheetPvt.groupLayout",
                    large: vec![
                        L("sheet.pvtSubtotals"),
                        L("sheet.pvtGrandTotals"),
                        L("sheet.pvtReportLayout"),
                        L("sheet.pvtBlankRows"),
                    ],
                    rows: vec![],
                },
                Group {
                    label: "sheetPvt.groupOptions",
                    large: vec![],
                    rows: vec![
                        vec![S("sheet.pvtRowHeaders"), S("sheet.pvtBandedRows")],
                        vec![S("sheet.pvtColHeaders"), S("sheet.pvtBandedCols")],
                    ],
                },
                Group {
                    label: "sheetPvt.groupStyles",
                    large: vec![
                        L(STYLE_TILES[0]),
                        L(STYLE_TILES[1]),
                        L(STYLE_TILES[2]),
                        L(STYLE_TILES[3]),
                        L(STYLE_TILES[4]),
                        L(STYLE_TILES[5]),
                        L(STYLE_TILES[6]),
                        L("sheet.pvtStylesMore"),
                    ],
                    rows: vec![],
                },
            ],
        },
    ]
}

/// Widths (logical px) of the style tiles (78 px apart 86 px at 125 %) and the More button
/// (36 px).
pub(crate) fn large_width(id: &str) -> Option<f32> {
    if STYLE_TILES.contains(&id) {
        Some(68.8)
    } else if id == "sheet.pvtStylesMore" {
        Some(28.8)
    } else {
        None
    }
}

/// Fixed widths (logical px) of row items: the Active Field box (150 px at 125 %) and the
/// option check boxes in two columns (the reference's 170 / 160 px).
pub(super) fn row_width(id: &str) -> Option<f32> {
    match id {
        "sheet.pvtActiveField" => Some(120.0),
        "sheet.pvtRowHeaders" | "sheet.pvtColHeaders" => Some(136.0),
        "sheet.pvtBandedRows" | "sheet.pvtBandedCols" => Some(128.0),
        _ => None,
    }
}

/// Menus of the tabs (choice id, label key).
pub(crate) fn pivot_menu(id: &str) -> Vec<(String, String)> {
    let e = |a: &str, b: &str| (a.to_owned(), b.to_owned());
    match id {
        "sheet.pvtPivotTable" => vec![
            e("pvt.selectAll", "sheetPvt.selectEntire"),
            e("pvt.options", "sheetPvt.options"),
        ],
        "sheet.pvtGroup" => vec![
            e("pvt.groupSelection", "sheetPvt.groupSelection"),
            e("pvt.ungroup", "sheetPvt.ungroup"),
            e("pvt.groupField", "sheetPvt.groupField"),
        ],
        "sheet.pvtRefresh" => vec![
            e("pvt.refresh", "sheetPvt.refresh"),
            e("pvt.refreshAll", "sheetPvt.refreshAll"),
        ],
        "sheet.pvtChangeSource" => vec![e("pvt.changeSource", "sheetPvt.changeSourceDots")],
        "sheet.pvtActions" => vec![
            e("pvt.clearAll", "sheetPvt.clearAll"),
            e("pvt.selectAll", "sheetPvt.selectEntire"),
            e("pvt.move", "sheetPvt.moveDots"),
        ],
        "sheet.pvtCalculations" => vec![
            e("pvt.calcField", "sheetPvt.calcField"),
            e("pvt.calcItem", "sheetPvt.calcItem"),
        ],
        "sheet.pvtShow" => vec![
            e("pvt.fieldList", "sheetPvt.fieldList"),
            e("pvt.buttons", "sheetPvt.buttons"),
            e("pvt.headers", "sheetPvt.fieldHeaders"),
        ],
        "sheet.pvtSubtotals" => vec![
            e("pvt.subNone", "sheetPvt.subNone"),
            e("pvt.subBottom", "sheetPvt.subBottom"),
            e("pvt.subTop", "sheetPvt.subTop"),
            e("pvt.subFiltered", "sheetPvt.subFiltered"),
        ],
        "sheet.pvtGrandTotals" => vec![
            e("pvt.grandOff", "sheetPvt.grandOff"),
            e("pvt.grandOn", "sheetPvt.grandOn"),
            e("pvt.grandRows", "sheetPvt.grandRows"),
            e("pvt.grandCols", "sheetPvt.grandCols"),
        ],
        "sheet.pvtReportLayout" => vec![
            e("pvt.layoutCompact", "sheetPvt.layoutCompact"),
            e("pvt.layoutOutline", "sheetPvt.layoutOutline"),
            e("pvt.layoutTabular", "sheetPvt.layoutTabular"),
            e("pvt.repeatLabels", "sheetPvt.repeatLabels"),
            e("pvt.noRepeatLabels", "sheetPvt.noRepeatLabels"),
        ],
        "sheet.pvtBlankRows" => vec![
            e("pvt.blankInsert", "sheetPvt.blankInsert"),
            e("pvt.blankRemove", "sheetPvt.blankRemove"),
        ],
        "sheet.pvtStylesMore" => bo_sheet::pivot::style::STYLE_NAMES
            .iter()
            .map(|s| (format!("pvtStyle.{s}"), (*s).to_owned()))
            .collect(),
        _ => Vec::new(),
    }
}

/// Own drawings for the tab buttons (24-unit grid).
#[allow(clippy::too_many_lines)]
pub(super) fn add_icons(icons: &mut Icons) {
    let white = Rgba(255, 255, 255, 255);
    let pale = Rgba(0xd6, 0xe4, 0xf2, 255);
    let blue = Rgba(0x2e, 0x75, 0xb6, 255);
    let green = Rgba(0x21, 0x73, 0x46, 255);
    let grey = Rgba(0xd9, 0xd9, 0xd9, 255);
    // A report sheet: frame, header band and rows (the family's grid look).
    icons.add_solid(
        "sheet.pvtPivotTable",
        &[
            ("M3 3h18v18h-18Z", Some(white)),
            ("M3 3h18v5h-18Z", Some(pale)),
            ("M3 3h18v18h-18ZM3 8h18M9 8v13M9 12h12M9 16h12", None),
        ],
    );
    icons.add_solid(
        "sheet.pvtShowDetails",
        &[
            ("M3 4h12v16h-12Z", Some(white)),
            ("M3 4h12v16h-12ZM3 9h12M3 14h12", None),
            ("M17 8v8M14 13l3 3l3-3", None),
        ],
    );
    icons.add_solid(
        "sheet.pvtExpand",
        &[("M10 6h11M10 12h11M10 18h11", None), ("M2 9h6M5 6v6", None)],
    );
    icons.add_solid(
        "sheet.pvtCollapse",
        &[("M10 6h11M10 12h11M10 18h11", None), ("M2 15h6", None)],
    );
    icons.add_solid(
        "sheet.pvtGroup",
        &[
            ("M5 4h10v4h-10ZM5 10h10v4h-10ZM5 16h10v4h-10Z", Some(pale)),
            ("M5 4h10v4h-10ZM5 10h10v4h-10ZM5 16h10v4h-10Z", None),
            ("M17 6h3v12h-3", None),
        ],
    );
    icons.add_solid(
        "sheet.pvtRefresh",
        &[
            ("M4 3h11v16h-11Z", Some(white)),
            ("M4 3h11v16h-11ZM4 7h11M4 11h11", None),
            ("M13 16a5 5 0 1 0 2-4", Some(green)),
            ("M13 16a5 5 0 1 0 2-4M13 11v5h5", None),
        ],
    );
    icons.add_solid(
        "sheet.pvtChangeSource",
        &[
            ("M3 3h12v14h-12Z", Some(white)),
            ("M3 3h12v14h-12ZM3 7h12M3 11h12M9 3v14", None),
            ("M14 14h7v7h-7Z", Some(pale)),
            ("M14 14h7v7h-7ZM16 18l2-2l2 2", None),
        ],
    );
    icons.add_solid(
        "sheet.pvtActions",
        &[
            ("M4 4h14v16h-14Z", Some(white)),
            ("M4 4h14v16h-14ZM7 9h8M7 13h8", None),
            ("M14 15l6 6M20 15l-6 6", None),
        ],
    );
    icons.add_solid(
        "sheet.pvtCalculations",
        &[
            ("M4 3h16v18h-16Z", Some(white)),
            ("M4 3h16v18h-16Z", None),
            ("M8 8h8M12 4v8M8 16h8M8 19h8", None),
        ],
    );
    icons.add_solid(
        "sheet.pvtShow",
        &[
            ("M5 3h14v18h-14Z", Some(white)),
            ("M5 3h14v18h-14ZM8 7h8M8 11h8M8 15h8", None),
            ("M5 3h14v3h-14Z", Some(blue)),
        ],
    );
    icons.alias("sheet.pvtChart", "sheet.pivotChart");
    icons.alias("sheet.pvtRecommended", "sheet.recPivot");
    icons.alias("sheet.pvtSlicer", "sheet.slicer");
    icons.add_solid(
        "sheet.pvtTimeline",
        &[
            ("M3 6h18v12h-18Z", Some(white)),
            ("M3 6h18v12h-18ZM3 10h18M7 13h3M12 13h3M17 13h2", None),
        ],
    );
    icons.add_solid(
        "sheet.pvtFilterConn",
        &[
            ("M4 6h16l-6 7v6l-4 2v-8Z", Some(grey)),
            ("M4 6h16l-6 7v6l-4 2v-8Z", None),
        ],
    );
    icons.add_solid(
        "sheet.pvtFieldSettings",
        &[
            ("M4 4h16v16h-16Z", Some(white)),
            ("M4 4h16v16h-16ZM8 9h8M8 13h8M8 17h5", None),
        ],
    );
    // The label and the box are drawn by the window; plain places.
    icons.add_solid("sheet.pvtActiveLabel", &[]);
    icons.add_solid("sheet.pvtActiveField", &[]);
    icons.add_solid(
        "sheet.pvtSubtotals",
        &[
            ("M4 3h16v18h-16Z", Some(white)),
            ("M4 9h16v3h-16ZM4 18h16v3h-16Z", Some(pale)),
            (
                "M4 3h16v18h-16ZM4 6h16M4 9h16M4 12h16M4 15h16M4 18h16",
                None,
            ),
        ],
    );
    icons.add_solid(
        "sheet.pvtGrandTotals",
        &[
            ("M4 3h16v18h-16Z", Some(white)),
            ("M4 17h16v4h-16ZM16 3h4v18h-4Z", Some(pale)),
            ("M4 3h16v18h-16ZM4 7h16M4 12h16M4 17h16M16 3v18", None),
        ],
    );
    icons.add_solid(
        "sheet.pvtReportLayout",
        &[
            ("M4 3h16v18h-16Z", Some(white)),
            ("M4 3h16v4h-16Z", Some(pale)),
            ("M4 3h16v18h-16ZM4 7h16M7 10h6M9 13h6M9 16h6M11 19h6", None),
        ],
    );
    icons.add_solid(
        "sheet.pvtBlankRows",
        &[
            ("M4 3h16v18h-16Z", Some(white)),
            ("M4 3h16v18h-16ZM4 7h16M4 11h16M4 15h16", None),
            ("M7 13h10", Some(blue)),
        ],
    );
    for id in STYLE_TILES {
        // Drawn over by the window (each style's own look).
        icons.add_solid(id, &[]);
    }
    icons.add_solid("sheet.pvtStylesMore", &[]);
    for (id, _) in OPTIONS {
        // Drawn over by a check box; a plain square keeps the place.
        icons.add_solid(id, &[("M5 5h14v14h-14Z", Some(white))]);
    }
}

/// `RRGGBB` → colour (white when malformed).
fn rgb(hex: &str) -> Rgba {
    let v = u32::from_str_radix(hex, 16).unwrap_or(0x00FF_FFFF);
    Rgba((v >> 16) as u8, (v >> 8) as u8, v as u8, 255)
}

/// The pivot table of the active cell as the engine lists it.
struct PivotRef {
    sheet: usize,
    index: usize,
    info: Value,
}

impl SheetWindow {
    /// Whether the active cell is in a pivot table (cheap: the model's report ranges).
    pub(super) fn in_pivot(&self) -> bool {
        let (r, c) = self.view.active();
        self.view
            .engine()
            .workbook()
            .and_then(|w| w.sheets.get(self.view.sheet()))
            .is_some_and(|ws| {
                ws.pivots.iter().any(|p| {
                    let l = p.location;
                    (l.first.row..=l.last.row).contains(&r)
                        && (l.first.col..=l.last.col).contains(&c)
                })
            })
    }

    fn pivot_ref(&mut self) -> Option<PivotRef> {
        let sheet = self.view.sheet();
        let (r, c) = self.view.active();
        let index = {
            let ws = self.view.engine().workbook()?.sheets.get(sheet)?;
            ws.pivots.iter().position(|p| {
                let l = p.location;
                (l.first.row..=l.last.row).contains(&r) && (l.first.col..=l.last.col).contains(&c)
            })?
        };
        let info = self
            .view
            .query(json!({ "cmd": "pivots", "sheet": sheet }))?
            .as_array()?
            .get(index)?
            .clone();
        Some(PivotRef { sheet, index, info })
    }

    /// `pivotUpdate` keeping the report's areas, with `settings` (layout, totals, field
    /// options, style) changed; columns fitted in the same undo step.
    pub(super) fn pivot_update_settings(&mut self, settings: &Value) -> bool {
        let Some(pr) = self.pivot_ref() else {
            return false;
        };
        let info = &pr.info;
        let names: Vec<String> = info
            .get("fields")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .map(|f| {
                        f.get("name")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .to_owned()
                    })
                    .collect()
            })
            .unwrap_or_default();
        let values: Vec<Value> = info
            .get("values")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .map(|v| {
                        let field = v.get("field").and_then(Value::as_u64).unwrap_or(0);
                        let mut o = json!({
                            "field": field,
                            "function": v.get("function").cloned().unwrap_or(json!("sum")),
                            "showAs": v.get("showAs").cloned().unwrap_or(json!("normal")),
                            "baseField": v.get("baseField").cloned().unwrap_or(json!(0)),
                            "baseItem": v.get("baseItem").cloned().unwrap_or(json!(0)),
                        });
                        // Only a name other than "<Function> of <field>" is the user's own.
                        let fname = names.get(field as usize).map_or("", String::as_str);
                        if let Some(n) = v.get("name").and_then(Value::as_str) {
                            if !n.ends_with(&format!(" of {fname}")) {
                                o["name"] = json!(n);
                            }
                        }
                        if let Some(f) = v.get("numFmt").and_then(Value::as_str) {
                            o["numFmt"] = json!(f);
                        }
                        o
                    })
                    .collect()
            })
            .unwrap_or_default();
        let mut cmd = json!({
            "cmd": "pivotUpdate", "sheet": pr.sheet, "index": pr.index,
            "rows": info.get("rows").cloned().unwrap_or(json!([])),
            "cols": info.get("cols").cloned().unwrap_or(json!([])),
            "values": values,
            "filters": info.get("filters").cloned().unwrap_or(json!([])),
            "labels": {
                "grandTotal": self.t("sheetInsert.grandTotal", "Grand Total"),
                "rowLabels": self.t("sheetInsert.rowLabels", "Row Labels"),
            },
        });
        if let (Some(c), Some(s)) = (cmd.as_object_mut(), settings.as_object()) {
            for (k, v) in s {
                c.insert(k.clone(), v.clone());
            }
        }
        let depth = self.view.engine().history_len();
        let ok = self.view.run(cmd);
        if ok {
            self.pivot_autofit(pr.sheet, pr.index, depth);
            self.sync_pivot_pane();
        } else {
            self.status = self.view.take_error().unwrap_or_default();
        }
        ok
    }

    /// Field options of every source field with `change` applied to the row and column
    /// fields (the `fieldOptions` of `pivotUpdate`).
    fn axis_field_options(info: &Value, change: &dyn Fn(&mut Value)) -> Value {
        let axis: Vec<u64> = ["rows", "cols"]
            .iter()
            .filter_map(|k| info.get(*k).and_then(Value::as_array))
            .flatten()
            .filter_map(Value::as_u64)
            .collect();
        let list: Vec<Value> = info
            .get("fieldOptions")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .enumerate()
                    .map(|(i, o)| {
                        let mut o = o.clone();
                        o["field"] = json!(i);
                        if axis.contains(&(i as u64)) {
                            change(&mut o);
                        }
                        o
                    })
                    .collect()
            })
            .unwrap_or_default();
        Value::Array(list)
    }

    /// The report's style info (the default light style when it has none).
    fn pivot_style(info: &Value) -> Value {
        info.get("style")
            .filter(|s| !s.is_null())
            .cloned()
            .unwrap_or_else(|| {
                json!({
                    "name": bo_sheet::pivot::style::DEFAULT_STYLE,
                    "rowHeaders": true, "colHeaders": true,
                    "rowStripes": false, "colStripes": false, "lastColumn": true,
                })
            })
    }

    /// Checked PivotTable Style Options.
    pub(super) fn pivot_checks(&self) -> Vec<(&'static str, bool)> {
        if !self.pivot_tab {
            return Vec::new();
        }
        let (r, c) = self.view.active();
        let Some(p) = self
            .view
            .engine()
            .workbook()
            .and_then(|w| w.sheets.get(self.view.sheet()))
            .and_then(|ws| {
                ws.pivots.iter().find(|p| {
                    let l = p.location;
                    (l.first.row..=l.last.row).contains(&r)
                        && (l.first.col..=l.last.col).contains(&c)
                })
            })
        else {
            return Vec::new();
        };
        let s = p.style.clone();
        let v = [
            s.as_ref().is_none_or(|s| s.show_row_headers),
            s.as_ref().is_none_or(|s| s.show_col_headers),
            s.as_ref().is_some_and(|s| s.show_row_stripes),
            s.as_ref().is_some_and(|s| s.show_col_stripes),
        ];
        OPTIONS
            .iter()
            .zip(v)
            .map(|((id, _), on)| (*id, on))
            .collect()
    }

    /// The field of the active cell: a row / column field whose item the cell shows, or the
    /// value field of a value cell (index into the source fields, caption).
    pub(super) fn pivot_active_field(&mut self) -> Option<(u32, String, bool)> {
        let pr = self.pivot_ref()?;
        let info = &pr.info;
        let (r, c) = self.view.active();
        let text = self
            .view
            .engine()
            .cell(pr.sheet, r, c)
            .map_or_else(|_| String::new(), |d| d.display.trim().to_owned());
        let fields = info.get("fields").and_then(Value::as_array)?;
        let name = |f: u64| {
            fields
                .get(f as usize)
                .and_then(|x| x.get("name"))
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned()
        };
        for k in ["rows", "cols"] {
            for f in info
                .get(k)
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_u64)
            {
                let has = fields
                    .get(f as usize)
                    .and_then(|x| x.get("items"))
                    .and_then(Value::as_array)
                    .is_some_and(|it| it.iter().any(|v| v.as_str() == Some(text.as_str())));
                if has || name(f) == text {
                    return Some((f as u32, name(f), false));
                }
            }
        }
        // The "Row Labels" / "Column Labels" headers and the row label column of a compact
        // report stand for their first field (the reference names it).
        let first = |k: &str| {
            info.get(k)
                .and_then(Value::as_array)
                .and_then(|a| a.first())
                .and_then(Value::as_u64)
        };
        let row_label = self.t("sheetInsert.rowLabels", "Row Labels");
        let col_label = self.t("sheetInsert.colLabels", "Column Labels");
        if text == row_label {
            if let Some(f) = first("rows") {
                return Some((f as u32, name(f), false));
            }
        }
        if text == col_label {
            if let Some(f) = first("cols") {
                return Some((f as u32, name(f), false));
            }
        }
        let values = info.get("values").and_then(Value::as_array)?;
        let area = info.get("dataArea");
        let n = |k: &str| {
            area.and_then(|a| a.get(k))
                .and_then(Value::as_u64)
                .unwrap_or(u64::MAX)
        };
        let in_data = area.is_some_and(|a| !a.is_null())
            && (n("r0")..=n("r1")).contains(&u64::from(r))
            && (n("c0")..=n("c1")).contains(&u64::from(c));
        let by_name = values
            .iter()
            .find(|v| v.get("name").and_then(Value::as_str) == Some(text.as_str()));
        let v = by_name.or_else(|| {
            in_data.then(|| {
                let i = (u64::from(c) - n("c0")) as usize % values.len().max(1);
                values.get(i)
            })?
        })?;
        let f = v.get("field").and_then(Value::as_u64)?;
        let caption = v
            .get("name")
            .and_then(Value::as_str)
            .map_or_else(|| name(f), str::to_owned);
        Some((f as u32, caption, true))
    }

    /// Ticked menu entries (current layout, totals, subtotals).
    pub(super) fn pivot_menu_current(&mut self, id: &str) -> Vec<String> {
        let Some(pr) = self.pivot_ref() else {
            return Vec::new();
        };
        let info = &pr.info;
        let b = |k: &str| info.get(k).and_then(Value::as_bool).unwrap_or(true);
        match id {
            "sheet.pvtReportLayout" => {
                let l = info.get("layout").and_then(Value::as_str).unwrap_or("");
                vec![format!(
                    "pvt.layout{}",
                    match l {
                        "compact" => "Compact",
                        "outline" => "Outline",
                        _ => "Tabular",
                    }
                )]
            }
            "sheet.pvtGrandTotals" => vec![match (b("rowGrandTotals"), b("colGrandTotals")) {
                (false, false) => "pvt.grandOff".into(),
                (true, true) => "pvt.grandOn".into(),
                (true, false) => "pvt.grandRows".into(),
                (false, true) => "pvt.grandCols".into(),
            }],
            "sheet.pvtSubtotals" => {
                let axis: Vec<u64> = ["rows", "cols"]
                    .iter()
                    .filter_map(|k| info.get(*k).and_then(Value::as_array))
                    .flatten()
                    .filter_map(Value::as_u64)
                    .collect();
                let opts = info.get("fieldOptions").and_then(Value::as_array);
                let o = |f: u64, k: &str| {
                    opts.and_then(|a| a.get(f as usize))
                        .and_then(|o| o.get(k))
                        .and_then(Value::as_bool)
                        .unwrap_or(true)
                };
                if axis.iter().all(|f| !o(*f, "subtotal")) {
                    vec!["pvt.subNone".into()]
                } else if axis.iter().all(|f| o(*f, "subtotalTop")) {
                    vec!["pvt.subTop".into()]
                } else {
                    vec!["pvt.subBottom".into()]
                }
            }
            _ => Vec::new(),
        }
    }

    /// Menu entries the report cannot use yet (shown dimmed, as the reference dims what
    /// does not apply).
    pub(super) fn pivot_menu_dim(choice: &str) -> bool {
        matches!(
            choice,
            "pvt.options"
                | "pvt.subFiltered"
                | "pvt.groupSelection"
                | "pvt.ungroup"
                | "pvt.groupField"
                | "pvt.calcItem"
                | "pvt.buttons"
                | "pvt.headers"
        )
    }

    /// Show the PivotTable tabs while the active cell is in a pivot table (no table or chart
    /// tab then); returns whether they show.
    pub(super) fn sync_pivot_tabs(&mut self) -> bool {
        let want = self.chart_sel.is_none() && self.in_pivot();
        if want != self.pivot_tab {
            self.pivot_tab = want;
            self.ribbon
                .set_contextual(if want { pivot_tabs() } else { Vec::new() }, false);
        }
        let mut dim: Vec<&'static str> = Vec::new();
        if want {
            let field = self.pivot_active_field();
            self.pivot_field_caption = field.as_ref().map(|f| f.1.clone()).unwrap_or_default();
            // As the reference on a report over a range: drilling, expanding and filter
            // connections need hierarchies / slicers; Field Settings here edits value fields.
            dim = vec![
                "sheet.pvtShowDetails",
                "sheet.pvtExpand",
                "sheet.pvtCollapse",
                "sheet.pvtTimeline",
                "sheet.pvtFilterConn",
            ];
            if !field.is_some_and(|f| f.2) {
                dim.push("sheet.pvtFieldSettings");
            }
        }
        self.ribbon.set_extra_dim(dim);
        want
    }

    /// The Active Field label and box, the option check boxes and the style tiles over their
    /// ribbon places.
    pub(super) fn paint_pivot_tabs(&self, p: &mut dyn Painter) {
        if !self.pivot_tab {
            return;
        }
        let ink = Rgba(0x26, 0x26, 0x26, 255);
        let style = crate::TextStyle {
            size: self.px(12.0),
            bold: false,
        };
        if let Some([x, y, w, h]) = self
            .ribbon
            .rect_of(RibbonHit::Command("sheet.pvtActiveLabel"))
        {
            p.fill_rect(x, y, w, h, self.theme.ribbon);
            p.text(
                &self.t("sheetPvt.activeField", "Active Field:"),
                x + self.px(2.0),
                (y + (h - style.size * 1.33) / 2.0).round(),
                style,
                ink,
            );
        }
        if let Some([x, y, w, h]) = self
            .ribbon
            .rect_of(RibbonHit::Command("sheet.pvtActiveField"))
        {
            let (y, h) = (y + self.px(1.0), h - self.px(2.0));
            p.fill_rect(x, y, w, h, Rgba(0xab, 0xab, 0xab, 255));
            p.fill_rect(x + 1.0, y + 1.0, w - 2.0, h - 2.0, Rgba(255, 255, 255, 255));
            p.text(
                &self.pivot_field_caption,
                x + self.px(4.0),
                (y + (h - style.size * 1.33) / 2.0).round(),
                style,
                ink,
            );
        }
        for (id, on) in self.pivot_checks() {
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
            super::form_impl::paint_check(p, [bx, by, b, b], on, true, false, self.scale);
        }
        self.paint_pivot_tiles(p);
    }

    /// Each inline tile: a small report in the style's colours (header band, rows, the
    /// grand total line), the current style framed.
    fn paint_pivot_tiles(&self, p: &mut dyn Painter) {
        let current = {
            let (r, c) = self.view.active();
            self.view
                .engine()
                .workbook()
                .and_then(|w| w.sheets.get(self.view.sheet()))
                .and_then(|ws| {
                    ws.pivots.iter().find(|p| {
                        let l = p.location;
                        (l.first.row..=l.last.row).contains(&r)
                            && (l.first.col..=l.last.col).contains(&c)
                    })
                })
                .and_then(|p| p.style.as_ref().map(|s| s.name.clone()))
                .unwrap_or_else(|| bo_sheet::pivot::style::DEFAULT_STYLE.to_owned())
        };
        // The gallery: a white box with a 1 px grey frame holding the tiles and the More
        // column (measured on the reference).
        let first = self.ribbon.rect_of(RibbonHit::Command(STYLE_TILES[0]));
        let more = self
            .ribbon
            .rect_of(RibbonHit::Command("sheet.pvtStylesMore"));
        if let (Some(a), Some(b)) = (first, more) {
            p.fill_rect(a[0], a[1], b[0] + b[2] - a[0], a[3], self.theme.ribbon);
            let (gy, gh) = (a[1] + self.px(2.0), self.px(72.0).min(a[3] - self.px(4.0)));
            let gw = b[0] + b[2] - a[0];
            p.fill_rect(a[0], gy, gw, gh, Rgba(0xc6, 0xc6, 0xc6, 255));
            p.fill_rect(
                a[0] + 1.0,
                gy + 1.0,
                gw - 2.0,
                gh - 2.0,
                Rgba(255, 255, 255, 255),
            );
        }
        for (id, name) in STYLE_TILES.iter().zip(TILE_STYLES) {
            let Some([x, y, w, h]) = self.ribbon.rect_of(RibbonHit::Command(id)) else {
                continue;
            };
            let (tx, ty, tw, th) = (
                x + self.px(3.0),
                y + self.px(8.0),
                w - self.px(6.0),
                self.px(48.0).min(h - self.px(16.0)),
            );
            if name == current {
                p.fill_rect(
                    tx - self.px(3.0),
                    ty - self.px(3.0),
                    tw + self.px(6.0),
                    th + self.px(6.0),
                    Rgba(0x8a, 0x8a, 0x8a, 255),
                );
                p.fill_rect(
                    tx - self.px(2.0),
                    ty - self.px(2.0),
                    tw + self.px(4.0),
                    th + self.px(4.0),
                    Rgba(0xd2, 0xd2, 0xd2, 255),
                );
            }
            let pal = bo_sheet::pivot::style::palette(name);
            p.fill_rect(tx, ty, tw, th, Rgba(255, 255, 255, 255));
            let rows = 7;
            let rh = (th / rows as f32).floor();
            for i in 0..rows {
                let ry = ty + i as f32 * rh;
                let fill = match i {
                    0 => pal.header_fill.as_deref().map(rgb),
                    6 => pal.grand_fill.as_deref().map(rgb),
                    _ if i % 2 == 0 => Some(rgb(&pal.stripe_fill)),
                    _ => None,
                };
                if let Some(f) = fill {
                    p.fill_rect(tx, ry, tw, rh, f);
                }
                // Text dashes in each row: label and two numbers.
                let dash = if i == 0 {
                    pal.header_font
                        .as_deref()
                        .map_or(Rgba(0x40, 0x40, 0x40, 255), rgb)
                } else {
                    Rgba(0x40, 0x40, 0x40, 255)
                };
                let my = (ry + rh / 2.0).floor();
                for (fx, fw) in [(0.08, 0.22), (0.42, 0.18), (0.70, 0.2)] {
                    p.fill_rect((tx + tw * fx).round(), my, (tw * fw).round(), 1.0, dash);
                }
            }
            // Header line and the grand total's top line.
            let edge = rgb(&pal.header_border);
            p.fill_rect(tx, ty + rh - 1.0, tw, 1.0, edge);
            p.fill_rect(tx, ty + 6.0 * rh, tw, 1.0, edge);
        }
        // More: a narrow button with a menu mark.
        if let Some([x, y, w, h]) = self
            .ribbon
            .rect_of(RibbonHit::Command("sheet.pvtStylesMore"))
        {
            let edge = Rgba(0xc6, 0xc6, 0xc6, 255);
            let (gy, gh) = (y + self.px(2.0), self.px(72.0).min(h - self.px(4.0)));
            p.fill_rect(x, gy, 1.0, gh, edge);
            let h = gh + 2.0 * (gy - y);
            let cx = (x + w / 2.0).round();
            let cy = (y + h / 2.0).round();
            let ink = Rgba(0x44, 0x44, 0x44, 255);
            p.fill_rect(cx - self.px(4.0), cy - self.px(4.0), self.px(8.0), 1.0, ink);
            for i in 0..4 {
                let fi = i as f32;
                p.fill_rect(
                    cx - self.px(4.0) + fi,
                    cy - self.px(1.0) + fi,
                    self.px(8.0) - 2.0 * fi,
                    1.0,
                    ink,
                );
            }
        }
    }

    /// PivotTable tab commands and menu choices; `None` for other ids.
    #[allow(clippy::too_many_lines)]
    pub(super) fn pivot_tab_action(
        &mut self,
        id: &str,
        text: &dyn crate::TextMeasure,
    ) -> Option<bool> {
        if let Some(done) = self.open_design_menu(id) {
            return Some(done);
        }
        if let Some(m) = MENUS.iter().find(|m| **m == id) {
            return Some(self.open_sheet_menu(m));
        }
        if let Some(name) = id.strip_prefix("pvtStyle.") {
            return Some(self.pivot_set_style(name));
        }
        if let Some(i) = STYLE_TILES.iter().position(|t| *t == id) {
            return Some(self.pivot_set_style(TILE_STYLES[i]));
        }
        if let Some((_, field)) = OPTIONS.iter().find(|(o, _)| *o == id) {
            let pr = self.pivot_ref()?;
            let mut s = Self::pivot_style(&pr.info);
            let on = s.get(*field).and_then(Value::as_bool).unwrap_or(false);
            s[*field] = json!(!on);
            return Some(self.pivot_update_settings(&json!({ "style": s })));
        }
        if !id.starts_with("pvt.") && !id.starts_with("sheet.pvt") {
            return None;
        }
        let set_fields = |w: &mut Self, change: &dyn Fn(&mut Value)| -> bool {
            let Some(pr) = w.pivot_ref() else {
                return false;
            };
            let opts = Self::axis_field_options(&pr.info, change);
            w.pivot_update_settings(&json!({ "fieldOptions": opts }))
        };
        Some(match id {
            "pvt.layoutCompact" => self.pivot_update_settings(&json!({ "layout": "compact" })),
            "pvt.layoutOutline" => self.pivot_update_settings(&json!({ "layout": "outline" })),
            "pvt.layoutTabular" => self.pivot_update_settings(&json!({ "layout": "tabular" })),
            "pvt.grandOff" => self.pivot_update_settings(
                &json!({ "rowGrandTotals": false, "colGrandTotals": false }),
            ),
            "pvt.grandOn" => self
                .pivot_update_settings(&json!({ "rowGrandTotals": true, "colGrandTotals": true })),
            "pvt.grandRows" => self
                .pivot_update_settings(&json!({ "rowGrandTotals": true, "colGrandTotals": false })),
            "pvt.grandCols" => self
                .pivot_update_settings(&json!({ "rowGrandTotals": false, "colGrandTotals": true })),
            "pvt.subNone" => set_fields(self, &|o| o["subtotal"] = json!(false)),
            "pvt.subBottom" => set_fields(self, &|o| {
                o["subtotal"] = json!(true);
                o["subtotalTop"] = json!(false);
            }),
            "pvt.subTop" => set_fields(self, &|o| {
                o["subtotal"] = json!(true);
                o["subtotalTop"] = json!(true);
            }),
            "pvt.repeatLabels" => set_fields(self, &|o| o["repeatLabels"] = json!(true)),
            "pvt.noRepeatLabels" => set_fields(self, &|o| o["repeatLabels"] = json!(false)),
            "pvt.blankInsert" => set_fields(self, &|o| o["blankRow"] = json!(true)),
            "pvt.blankRemove" => set_fields(self, &|o| o["blankRow"] = json!(false)),
            "pvt.refresh" => {
                let Some(pr) = self.pivot_ref() else {
                    return Some(false);
                };
                let depth = self.view.engine().history_len();
                let ok = self.view.run(json!({
                    "cmd": "pivotRefresh", "sheet": pr.sheet, "index": pr.index,
                    "labels": self.pivot_labels(),
                }));
                if ok {
                    self.pivot_autofit(pr.sheet, pr.index, depth);
                    self.sync_pivot_pane();
                }
                ok
            }
            "pvt.refreshAll" => {
                let labels = self.pivot_labels();
                self.view
                    .run(json!({ "cmd": "pivotRefreshAll", "labels": labels }))
            }
            "pvt.clearAll" => {
                let Some(pr) = self.pivot_ref() else {
                    return Some(false);
                };
                let depth = self.view.engine().history_len();
                let ok = self.view.run(json!({
                    "cmd": "pivotUpdate", "sheet": pr.sheet, "index": pr.index,
                    "labels": self.pivot_labels(),
                }));
                if ok {
                    self.pivot_autofit(pr.sheet, pr.index, depth);
                    self.sync_pivot_pane();
                }
                ok
            }
            "pvt.selectAll" => {
                let Some(pr) = self.pivot_ref() else {
                    return Some(false);
                };
                let area = pr
                    .info
                    .get("area")
                    .or_else(|| pr.info.get("location"))
                    .cloned()
                    .unwrap_or(Value::Null);
                let n = |k: &str| area.get(k).and_then(Value::as_u64).unwrap_or(0) as u32;
                self.view
                    .select((n("r0"), n("c0")), (n("r1"), n("c1")), (n("r0"), n("c0")));
                true
            }
            "pvt.move" => self.open_pivot_move(),
            "pvt.changeSource" | "sheet.pvtChangeSourceDlg" => self.open_pivot_source(),
            "pvt.fieldList" | "sheet.pvtFieldList" => {
                if self.pivot.is_some() {
                    self.pivot_closed = self.pivot.as_ref().map(|p| (p.sheet, p.index));
                    self.pivot = None;
                } else {
                    self.pivot_closed = None;
                    self.sync_pivot_pane();
                }
                self.pane_changed = true;
                true
            }
            "pvt.calcField" => {
                self.status = self.t(
                    "sheetPvt.calcLater",
                    "Calculated fields are edited from the PivotTable Fields list.",
                );
                true
            }
            "sheet.pvtFieldSettings" => match self.pivot_active_field() {
                Some((f, _, true)) => {
                    self.sync_pivot_pane();
                    self.open_value_settings(f)
                }
                _ => false,
            },
            "sheet.pvtSlicer" => self.action("sheet.slicer", text),
            "sheet.pvtRecommended" => self.action("sheet.recPivot", text),
            "sheet.pvtChart" => {
                let Some(pr) = self.pivot_ref() else {
                    return Some(false);
                };
                self.view
                    .run(json!({ "cmd": "pivotChart", "index": pr.index, "kind": "column" }))
            }
            _ => false,
        })
    }

    fn pivot_labels(&self) -> Value {
        json!({
            "grandTotal": self.t("sheetInsert.grandTotal", "Grand Total"),
            "rowLabels": self.t("sheetInsert.rowLabels", "Row Labels"),
        })
    }

    fn pivot_set_style(&mut self, name: &str) -> bool {
        let Some(pr) = self.pivot_ref() else {
            return false;
        };
        let mut s = Self::pivot_style(&pr.info);
        s["name"] = json!(name);
        self.pivot_update_settings(&json!({ "style": s }))
    }

    /// Move PivotTable: the new top-left cell.
    fn open_pivot_move(&mut self) -> bool {
        let Some(pr) = self.pivot_ref() else {
            return false;
        };
        let at = pr
            .info
            .get("location")
            .map(|r| {
                let n = |k: &str| r.get(k).and_then(Value::as_u64).unwrap_or(0) as u32;
                format!(
                    "=${}${}",
                    bo_model::sheet::col_to_letters(n("c0")),
                    n("r0") + 1
                )
            })
            .unwrap_or_default();
        let tt = |k: &str, d: &str| self.t(k, d);
        let mut f = Form::new(
            &tt("sheetPvt.moveTitle", "Move PivotTable"),
            (440.0, 190.0),
            vec![],
        );
        f.add(
            "label",
            None,
            [20.0, 56.0, 0.0, 0.0],
            Kind::Label(tt(
                "sheetPvt.moveLabel",
                "Choose where you want the PivotTable to be placed",
            )),
        );
        f.add(
            "dest",
            None,
            [20.0, 84.0, 400.0, 25.0],
            Kind::Edit {
                caret: at.chars().count(),
                value: at.chars().collect(),
            },
        );
        f.ok_cancel(&tt("common.ok", "OK"), &tt("common.cancel", "Cancel"));
        f.focus("dest");
        self.form = Some(f);
        self.form_kind = FormKind::PivotMove;
        true
    }

    /// Change PivotTable Data Source: the source range.
    fn open_pivot_source(&mut self) -> bool {
        let Some(pr) = self.pivot_ref() else {
            return false;
        };
        let src = pr
            .info
            .get("source")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned();
        let tt = |k: &str, d: &str| self.t(k, d);
        let mut f = Form::new(
            &tt("sheetPvt.sourceTitle", "Change PivotTable Data Source"),
            (480.0, 190.0),
            vec![],
        );
        f.add(
            "label",
            None,
            [20.0, 56.0, 0.0, 0.0],
            Kind::Label(tt("sheetPvt.sourceLabel", "Table/Range:")),
        );
        f.add(
            "source",
            None,
            [20.0, 84.0, 440.0, 25.0],
            Kind::Edit {
                caret: src.chars().count(),
                value: src.chars().collect(),
            },
        );
        f.ok_cancel(&tt("common.ok", "OK"), &tt("common.cancel", "Cancel"));
        f.focus("source");
        self.form = Some(f);
        self.form_kind = FormKind::PivotSource;
        true
    }

    /// Events of the Move PivotTable / Change Data Source forms.
    pub(super) fn pivot_tab_form_event(&mut self, kind: FormKind, ev: &Event) {
        match (kind, ev) {
            (_, Event::Cancel) => self.form = None,
            (FormKind::PivotMove, Event::Ok) => {
                let text = self
                    .form
                    .as_ref()
                    .map(|f| f.text("dest"))
                    .unwrap_or_default();
                let cell = text.trim().trim_start_matches('=').replace('$', "");
                let cell = cell.rsplit('!').next().unwrap_or_default().to_owned();
                let Some(r) = bo_model::sheet::CellRange::parse(&cell) else {
                    self.status = self.t(
                        "sheetPivot.badDest",
                        "The PivotTable destination reference is not valid.",
                    );
                    return;
                };
                let Some(pr) = self.pivot_ref() else {
                    self.form = None;
                    return;
                };
                let labels = self.pivot_labels();
                let depth = self.view.engine().history_len();
                if self.view.run(json!({
                    "cmd": "pivotMove", "sheet": pr.sheet, "index": pr.index,
                    "dest": { "sheet": pr.sheet, "row": r.first.row, "col": r.first.col },
                    "labels": labels,
                })) {
                    self.form = None;
                    self.view.select(
                        (r.first.row, r.first.col),
                        (r.first.row, r.first.col),
                        (r.first.row, r.first.col),
                    );
                    self.pivot_autofit(pr.sheet, pr.index, depth);
                } else {
                    self.status = self.view.take_error().unwrap_or_default();
                }
            }
            (FormKind::PivotSource, Event::Ok) => {
                let src = self
                    .form
                    .as_ref()
                    .map(|f| f.text("source"))
                    .unwrap_or_default();
                let src = src.trim().trim_start_matches('=').replace('$', "");
                let Some(pr) = self.pivot_ref() else {
                    self.form = None;
                    return;
                };
                let labels = self.pivot_labels();
                let depth = self.view.engine().history_len();
                if self.view.run(json!({
                    "cmd": "pivotChangeSource", "sheet": pr.sheet, "index": pr.index,
                    "source": src, "labels": labels,
                })) {
                    self.form = None;
                    self.pivot_autofit(pr.sheet, pr.index, depth);
                    self.sync_pivot_pane();
                } else {
                    self.status = self.view.take_error().unwrap_or_else(|| {
                        self.t(
                            "sheetPivot.badRange",
                            "The PivotTable source reference is not valid.",
                        )
                    });
                }
            }
            _ => {}
        }
    }
}

/// Design tab menus drawn as the reference's: tall rows with a report picture before each
/// label (measured at 125 %: rows 57 px, pictures 32 px 12 px from the edge, labels 62 px in).
const DESIGN_MENUS: [&str; 4] = [
    "sheet.pvtSubtotals",
    "sheet.pvtGrandTotals",
    "sheet.pvtReportLayout",
    "sheet.pvtBlankRows",
];

fn design_look() -> crate::popup::MenuLook {
    crate::popup::MenuLook {
        item_h: 45.6,
        text: 13.0,
        text_x: (49.6, 49.6),
        icon_x: 9.6,
        icon: 25.6,
        pad: 2.0,
        min_w: 240.0,
        ..crate::popup::MenuLook::default()
    }
}

/// Own report pictures of the Design menus: a page of grey text lines with the rows the
/// choice is about in blue (totals, labels, blank lines).
#[allow(clippy::many_single_char_names)]
fn design_icon(p: &mut dyn Painter, name: &str, x: f32, y: f32, size: f32, enabled: bool) {
    let k = size / 32.0;
    let edge = Rgba(0x9a, 0x9a, 0x9a, 255);
    let grey = if enabled {
        Rgba(0x8c, 0x8c, 0x8c, 255)
    } else {
        Rgba(0xc8, 0xc8, 0xc8, 255)
    };
    let blue = Rgba(0x2e, 0x75, 0xb6, 255);
    let (w, h) = ((24.0 * k).round(), (30.0 * k).round());
    let x0 = (x + (size - w) / 2.0).round();
    let y0 = y.round();
    p.fill_rect(x0, y0, w, h, edge);
    p.fill_rect(
        x0 + 1.0,
        y0 + 1.0,
        w - 2.0,
        h - 2.0,
        Rgba(255, 255, 255, 255),
    );
    let rows = 8;
    let step = ((h - 4.0) / rows as f32).floor().max(2.0);
    for i in 0..rows {
        let ry = y0 + 3.0 + i as f32 * step;
        // Which rows stand out, and how far in the label starts.
        let (hi, indent) = match name {
            "pvt.subBottom" => (i == 3 || i == 7, 0.0),
            "pvt.subTop" => (i == 0 || i == 4, 0.0),
            "pvt.grandOn" | "pvt.grandCols" => (i == 7, 0.0),
            "pvt.layoutCompact" => (false, if i % 4 == 0 { 0.0 } else { 3.0 }),
            "pvt.layoutOutline" => (i % 4 == 0, if i % 4 == 0 { 0.0 } else { 7.0 }),
            "pvt.layoutTabular" => (false, 7.0 * f32::from(u8::from(i % 4 != 0))),
            "pvt.repeatLabels" => (true, 0.0),
            _ => (false, 0.0),
        };
        if name == "pvt.blankInsert" && i % 3 == 2 {
            continue;
        }
        let lx = x0 + 3.0 + indent * k;
        let lw = (w - 6.0 - indent * k).max(2.0);
        let th = if hi { (2.0 * k).round().max(2.0) } else { 1.0 };
        p.fill_rect(lx, ry, lw, th, if hi { blue } else { grey });
        if matches!(name, "pvt.grandRows" | "pvt.grandOn") {
            // The grand total column at the right.
            p.fill_rect(x0 + w - 5.0 * k, ry, 2.0 * k, 1.0, blue);
        }
    }
}

impl SheetWindow {
    /// Open a Design tab menu with its pictures; `None` for other ids.
    pub(super) fn open_design_menu(&mut self, id: &str) -> Option<bool> {
        if !DESIGN_MENUS.contains(&id) {
            return None;
        }
        let anchor = self.ribbon.rect_of(RibbonHit::Command(
            DESIGN_MENUS.iter().find(|m| **m == id).copied()?,
        ))?;
        let entries = pivot_menu(id);
        let icons: Vec<Option<&'static str>> = entries
            .iter()
            .map(|(c, _)| ICON_NAMES.iter().find(|n| **n == c.as_str()).copied())
            .collect();
        let separators = if id == "sheet.pvtReportLayout" || id == "sheet.pvtSubtotals" {
            vec![3]
        } else {
            Vec::new()
        };
        let items = entries
            .into_iter()
            .map(|(choice, label)| crate::popup::MenuItem {
                label: self.strings.get(&label).to_owned(),
                checked: false,
                dim: Self::pivot_menu_dim(&choice),
                choice: crate::popup::Choice::Command(choice),
            })
            .collect();
        self.menu = Some(
            crate::popup::Menu::open_with(items, anchor, self.size, self.scale, design_look())
                .with_decor(icons, separators)
                .with_icon_fn(design_icon),
        );
        Some(true)
    }
}

/// Choice ids that have a picture (static names for the menu's icon column).
const ICON_NAMES: [&str; 15] = [
    "pvt.subFiltered",
    "pvt.subNone",
    "pvt.subBottom",
    "pvt.subTop",
    "pvt.grandOff",
    "pvt.grandOn",
    "pvt.grandRows",
    "pvt.grandCols",
    "pvt.layoutCompact",
    "pvt.layoutOutline",
    "pvt.layoutTabular",
    "pvt.repeatLabels",
    "pvt.noRepeatLabels",
    "pvt.blankInsert",
    "pvt.blankRemove",
];
