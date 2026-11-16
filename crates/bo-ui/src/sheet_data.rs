//! Data tab commands of BlueSheet that reach outside the workbook (round 6): Get Data from
//! Text/CSV through the engine's Get & Transform commands (`bo-query`: `querySource`,
//! `queryInspect`, `querySave`), Refresh All.
use super::form_impl::{Event, Form, FormKind, Kind};
use super::SheetWindow;
use crate::icons::Icons;
use bo_render::Rgba;
use serde_json::{json, Value};

const BLUE: Rgba = Rgba(0x2e, 0x75, 0xb6, 255);
const PALE: Rgba = Rgba(0xd6, 0xe4, 0xf2, 255);
const GREEN: Rgba = Rgba(0x21, 0x73, 0x46, 255);
const ORANGE: Rgba = Rgba(0xed, 0x7d, 0x31, 255);
const GREY: Rgba = Rgba(0x8a, 0x8a, 0x8a, 255);
const RED: Rgba = Rgba(0xc0, 0x39, 0x2b, 255);
const GOLD: Rgba = Rgba(0xf2, 0xb7, 0x00, 255);

/// Labels of the native ribbon that follow the reference's wording: `sheetNative.<name>` over
/// the shared key (`commands.sheet.<name>` when the target has no dot), so the web editor's
/// labels stay as they are.
const NATIVE_LABELS: [(&str, &str); 18] = [
    ("textToColumns", "textToColumns"),
    ("fnMath", "fnMath"),
    ("fnDateTime", "fnDateTime"),
    ("fnLookup", "fnLookup"),
    ("fnMore", "fnMore"),
    ("removeDuplicates", "removeDuplicates"),
    ("autoSumF", "autoSumF"),
    ("calcOptions", "calcOptions"),
    ("freezePanes", "freezePanes"),
    ("zoomSelection", "zoomSelection"),
    ("formulaBar", "formulaBar"),
    ("tracePrecedents", "tracePrecedents"),
    ("traceDependents", "traceDependents"),
    ("removeArrows", "removeArrows"),
    ("groupSortFilter", "sheetEditor.groupSortFilter"),
    ("groupDataTools", "sheetEditor.groupDataTools"),
    ("groupFunctions", "sheetMenu.groupFunctions"),
    ("groupAuditing", "sheetMenu.groupAuditing"),
];

/// Use the native labels for this window's ribbon.
pub(super) fn install_labels(s: &mut crate::strings::Strings) {
    // Chart style tiles: no label, "Style n" as the tooltip.
    for (i, id) in super::chart_impl::STYLE_TILES.iter().enumerate() {
        s.set(&format!("commands.{id}"), "");
        s.set(&format!("commands.tip.{id}"), &format!("Style {}", i + 1));
    }
    s.set("commands.sheet.chartStylesMore", "");
    for (name, target) in NATIVE_LABELS {
        let v = s.get(&format!("sheetNative.{name}")).to_owned();
        if v.is_empty() || v.starts_with("sheetNative.") {
            continue;
        }
        let key = if target.contains('.') {
            target.to_owned()
        } else {
            format!("commands.sheet.{target}")
        };
        s.set(&key, &v);
    }
}
/// Own drawings for the round-6 Data / Formulas / View buttons (24-unit grid).
pub(super) fn add_icons(icons: &mut Icons) {
    let table = |extra: &[(&'static str, Option<Rgba>)]| {
        let mut v: Vec<(&'static str, Option<Rgba>)> = vec![
            ("M3 5h14v14h-14Z", Some(Rgba(255, 255, 255, 255))),
            ("M3 5h14v14h-14Z", None),
            ("M3 9h14M3 14h14M8 5v14", None),
        ];
        v.extend_from_slice(extra);
        v
    };
    icons.add_solid(
        "sheet.getData",
        &table(&[("M16 2l-4 7h3l-2 6l6-8h-3l2-5Z", Some(GOLD))]),
    );
    icons.add_solid(
        "sheet.fromText",
        &[
            ("M5 2h10l4 4v16h-14Z", Some(Rgba(255, 255, 255, 255))),
            ("M5 2h10l4 4v16h-14Z", None),
            ("M8 9h8M8 12h8M8 15h6", None),
        ],
    );
    icons.add_solid(
        "sheet.fromWeb",
        &[
            ("M12 3a9 9 0 1 0 0.1 0Z", Some(PALE)),
            ("M12 3a9 9 0 1 0 0.1 0Z", None),
            ("M3 12h18M12 3c-4 4-4 14 0 18M12 3c4 4 4 14 0 18", None),
        ],
    );
    icons.add_solid("sheet.fromTable", &table(&[("M3 5h14v4h-14Z", Some(BLUE))]));
    icons.add_solid(
        "sheet.recentSources",
        &[
            ("M5 2h10l4 4v16h-14Z", Some(Rgba(255, 255, 255, 255))),
            ("M5 2h10l4 4v16h-14Z", None),
            ("M12 9a5 5 0 1 0 0.1 0ZM12 11v3h2", None),
        ],
    );
    icons.add_solid(
        "sheet.connections",
        &[
            ("M5 2h10l4 4v16h-14Z", Some(Rgba(255, 255, 255, 255))),
            ("M5 2h10l4 4v16h-14Z", None),
            ("M9 10h6v8h-6Z", Some(GOLD)),
        ],
    );
    icons.add_solid(
        "sheet.refreshAll",
        &[
            ("M3 3h11v14h-11Z", Some(Rgba(255, 255, 255, 255))),
            ("M3 3h11v14h-11Z", None),
            ("M12 15a5 5 0 0 1 9-3M21 17a5 5 0 0 1-9 3", None),
            ("M19 9l2 3l-3 1Z", Some(GREEN)),
        ],
    );
    icons.add_solid(
        "sheet.queriesConn",
        &table(&[("M17 9h5v10h-5Z", Some(PALE)), ("M17 9h5v10h-5Z", None)]),
    );
    icons.alias("sheet.queryProps", "sheet.fromText");
    icons.add_solid(
        "sheet.editLinks",
        &[("M4 14a4 4 0 0 1 4-4h3M20 10a4 4 0 0 1-4 4h-3M8 12h8", None)],
    );
    icons.add_solid(
        "sheet.filterClear",
        &[
            ("M3 4h14l-5 7v6l-4 2v-8Z", Some(PALE)),
            ("M3 4h14l-5 7v6l-4 2v-8Z", None),
            ("M15 14l6 6M21 14l-6 6", None),
        ],
    );
    icons.add_solid(
        "sheet.filterReapply",
        &[
            ("M3 4h14l-5 7v6l-4 2v-8Z", Some(PALE)),
            ("M3 4h14l-5 7v6l-4 2v-8Z", None),
            ("M15 18a3 3 0 1 0 3-3", None),
        ],
    );
    icons.add_solid(
        "sheet.filterAdvanced",
        &[
            ("M3 4h14l-5 7v6l-4 2v-8Z", Some(GREY)),
            ("M17 13l3 3l-4 5h-2v-2Z", Some(GOLD)),
        ],
    );
    icons.add_solid(
        "sheet.flashFill",
        &table(&[("M19 2l-4 7h3l-2 6l6-8h-3l2-5Z", Some(GOLD))]),
    );
    icons.add_solid(
        "sheet.consolidate",
        &[
            ("M2 3h7v7h-7ZM2 14h7v7h-7Z", Some(PALE)),
            ("M2 3h7v7h-7ZM2 14h7v7h-7Z", None),
            ("M15 8h7v8h-7Z", Some(BLUE)),
            ("M9 6l5 4M9 17l5-3", None),
        ],
    );
    icons.add_solid(
        "sheet.relationships",
        &[
            ("M2 3h8v7h-8ZM14 14h8v7h-8Z", Some(PALE)),
            ("M2 3h8v7h-8ZM14 14h8v7h-8Z", None),
            ("M6 10v7h8", None),
        ],
    );
    icons.add_solid(
        "sheet.dataModel",
        &[
            ("M4 5a8 3 0 1 0 16 0a8 3 0 1 0-16 0Z", Some(GREEN)),
            ("M4 5v13a8 3 0 0 0 16 0v-13", Some(GREEN)),
            ("M4 11a8 3 0 0 0 16 0", None),
        ],
    );
    icons.add_solid(
        "sheet.whatIf",
        &table(&[
            ("M18 8a4 4 0 1 1 3 4v3h-3v-4c3 0 3-4 0-4", Some(BLUE)),
            ("M18 17h3v3h-3Z", Some(BLUE)),
        ]),
    );
    icons.add_solid(
        "sheet.forecastSheet",
        &[
            ("M3 3h16v16h-16Z", Some(Rgba(255, 255, 255, 255))),
            ("M3 3h16v16h-16Z", None),
            ("M5 16l4-5l3 2l3-4", None),
            ("M15 9l5-4", Some(RED)),
            ("M15 9l5-4", None),
        ],
    );
    icons.add_solid(
        "sheet.outline",
        &[
            ("M3 4h7v6h-7Z", Some(PALE)),
            ("M3 4h7v6h-7Z", None),
            ("M12 4h9v4h-9ZM12 10h9v4h-9ZM12 16h9v4h-9Z", None),
            ("M6 10v8h6", None),
        ],
    );
    icons.add_solid(
        "sheet.definedNames",
        &[
            ("M3 6h14v12h-14Z", Some(Rgba(255, 255, 255, 255))),
            ("M3 6h14v12h-14Z", None),
            ("M6 3h14v12", None),
            ("M6 10h8M6 14h5", None),
        ],
    );
    icons.add_solid(
        "sheet.watchWindow",
        &[
            ("M2 3h20v9h-20Z", Some(Rgba(255, 255, 255, 255))),
            ("M2 3h20v9h-20Z", None),
            (
                "M3 18a3 3 0 1 0 6 0a3 3 0 1 0-6 0ZM15 18a3 3 0 1 0 6 0a3 3 0 1 0-6 0ZM9 18h6",
                None,
            ),
        ],
    );
    icons.add_solid(
        "sheet.fnRecent",
        &[
            ("M5 3h12v18h-12Z", Some(BLUE)),
            ("M8 7l2 2l3-4", Some(Rgba(255, 255, 255, 255))),
        ],
    );
    icons.add_solid(
        "sheet.viewNormal",
        &table(&[
            ("M17 5h4v14h-4Z", Some(Rgba(255, 255, 255, 255))),
            ("M17 5h4v14h-4ZM17 9h4M17 14h4", None),
        ]),
    );
    icons.add_solid(
        "sheet.viewPageBreak",
        &[
            (
                "M3 3h8v18h-8ZM13 3h8v18h-8Z",
                Some(Rgba(255, 255, 255, 255)),
            ),
            ("M3 3h8v18h-8ZM13 3h8v18h-8Z", None),
            ("M3 8h18M3 13h18", None),
            ("M12 2v20", Some(BLUE)),
            ("M12 2v20", None),
        ],
    );
    icons.add_solid(
        "sheet.viewPageLayout",
        &[
            ("M5 2h14v20h-14Z", Some(Rgba(255, 255, 255, 255))),
            ("M5 2h14v20h-14Z", None),
            ("M8 7h8M8 11h8M8 15h8", None),
        ],
    );
    icons.add_solid(
        "sheet.customViews",
        &[
            ("M3 5h14v13h-14Z", Some(Rgba(255, 255, 255, 255))),
            ("M3 5h14v13h-14Z", None),
            ("M7 2h14v13", None),
        ],
    );
    icons.add_solid(
        "sheet.ruler",
        &[
            ("M2 8h20v8h-20Z", None),
            ("M6 8v3M10 8v4M14 8v3M18 8v4", None),
        ],
    );
    icons.add_solid(
        "sheet.zoomDialog",
        &[
            ("M10 3a7 7 0 1 0 0.1 0Z", Some(Rgba(255, 255, 255, 255))),
            ("M10 3a7 7 0 1 0 0.1 0Z", None),
            ("M15 15l6 6", None),
        ],
    );
    icons.add_solid(
        "sheet.newWindow",
        &[
            ("M2 5h15v12h-15Z", Some(Rgba(255, 255, 255, 255))),
            ("M2 5h15v12h-15ZM2 8h15", None),
            ("M17 13h5M19.5 10.5v5", Some(BLUE)),
            ("M17 13h5M19.5 10.5v5", None),
        ],
    );
    icons.add_solid(
        "sheet.arrangeAll",
        &[
            ("M3 4h18v7h-18Z", Some(BLUE)),
            ("M3 13h18v7h-18Z", Some(BLUE)),
        ],
    );
    icons.add_solid(
        "sheet.split",
        &[
            ("M3 4h18v16h-18Z", Some(Rgba(255, 255, 255, 255))),
            ("M3 4h18v16h-18ZM3 12h18", None),
            ("M3 4h18v3h-18Z", Some(BLUE)),
        ],
    );
    icons.add_solid(
        "sheet.hideWindow",
        &[
            ("M3 4h18v16h-18Z", Some(Rgba(255, 255, 255, 255))),
            ("M3 4h18v16h-18Z", None),
            ("M3 4h18v3h-18Z", Some(BLUE)),
        ],
    );
    icons.alias("sheet.unhideWindow", "sheet.hideWindow");
    icons.add_solid(
        "sheet.switchWindows",
        &[
            ("M2 3h14v10h-14Z", Some(Rgba(255, 255, 255, 255))),
            ("M2 3h14v10h-14Z", None),
            ("M8 10h14v10h-14Z", Some(Rgba(255, 255, 255, 255))),
            ("M8 10h14v10h-14Z", None),
            ("M8 10h14v3h-14Z", Some(BLUE)),
        ],
    );
    icons.add_solid(
        "sheet.macros",
        &[
            ("M3 4h15v14h-15Z", Some(Rgba(255, 255, 255, 255))),
            ("M3 4h15v14h-15ZM3 8h15", None),
            ("M14 14l7 7l-2 1l-7-6Z", Some(ORANGE)),
        ],
    );
}

/// Zoom dialog choices (percent; 0 = fit selection).
const ZOOMS: [u32; 6] = [200, 100, 75, 50, 25, 0];
/// Option buttons of the Zoom dialog (the presets, then Custom).
const ZOOM_IDS: [&str; 7] = ["z0", "z1", "z2", "z3", "z4", "z5", "z6"];

fn edit(value: &str) -> Kind {
    Kind::Edit {
        value: value.chars().collect(),
        caret: value.chars().count(),
    }
}

impl SheetWindow {
    /// Data / Formulas / View tab commands added in round 6; `None` for other ids.
    pub(super) fn data_action(&mut self, id: &str, text: &dyn crate::TextMeasure) -> Option<bool> {
        let (r, c) = self.view.active();
        let (r0, c0, r1, c1) = self.view.range_bounds();
        Some(match id {
            "sheet.fromText" => self.pick_text_file(),
            "sheet.fromTable" => return self.insert_action("sheet.table"),
            "sheet.queriesConn" | "sheet.connections" => self.open_queries(),
            "sheet.refreshAllNow" => {
                self.view.run(json!({ "cmd": "pivotRefreshAll" }));
                let _ = self.view.take_error();
                self.refresh_queries()
            }
            "sheet.refreshPivots" => self.view.run(json!({ "cmd": "pivotRefreshAll" })),
            "sheet.filterClear" => self.view.run(json!({ "cmd": "showAllData" })),
            "sheet.flashFill" => self
                .view
                .run(json!({ "cmd": "flashFill", "row": r, "col": c })),
            "sheet.consolidate" => self.open_consolidate(),
            "sheet.dataTableWhatIf" => self.open_data_table(),
            "outline.group" | "outline.ungroup" => {
                // Whole columns selected group columns, otherwise rows (as the reference
                // without its Rows / Columns question).
                let cols = r0 == 0 && r1 >= 1_048_575;
                let (first, last) = if cols { (c0, c1) } else { (r0, r1) };
                self.view.run(json!({
                    "cmd": "outlineGroup", "rows": !cols, "first": first, "last": last,
                    "ungroup": id == "outline.ungroup",
                }))
            }
            "outline.subtotal" => {
                let region = self.region();
                let n = |k: &str| region.get(k).and_then(Value::as_u64).unwrap_or(0) as u32;
                self.view.run(json!({
                    "cmd": "subtotal", "range": region, "groupBy": n("c0"), "function": "sum",
                    "columns": [n("c1")], "replace": true, "pageBreaks": false,
                    "summaryBelow": true,
                }))
            }
            "outline.show" | "outline.hide" => self.view.run(json!({
                "cmd": "outlineDetail", "rows": true, "at": r, "show": id == "outline.show",
            })),
            "outline.auto" => self.view.run(json!({ "cmd": "autoOutline" })),
            "outline.clear" => self.view.run(json!({ "cmd": "clearOutline" })),
            "sheet.viewNormal" => self.set_view_mode(super::ViewMode::Normal, text),
            "sheet.viewPageBreak" => self.set_view_mode(super::ViewMode::PageBreak, text),
            "sheet.viewPageLayout" => self.set_view_mode(super::ViewMode::PageLayout, text),
            "sheet.customViews" => self.open_custom_views(),
            "sheet.zoomDialog" => self.open_zoom(),
            _ => {
                let _ = text;
                return None;
            }
        })
    }

    /// Queries & Connections: the workbook's queries (names), Refresh All, Close.
    fn open_queries(&mut self) -> bool {
        let names: Vec<String> = self
            .view
            .query(json!({ "cmd": "queryList" }))
            .and_then(|v| v.get("queries").cloned().or(Some(v)))
            .and_then(|v| v.as_array().cloned())
            .unwrap_or_default()
            .iter()
            .filter_map(|q| q.get("name").and_then(Value::as_str).map(str::to_owned))
            .collect();
        let t = |k: &str, d: &str| self.t(k, d);
        let mut f = Form::new(
            &t("commands.sheet.queriesConn", "Queries & Connections"),
            (360.0, 380.0),
            vec![],
        );
        f.add(
            "label",
            None,
            [18.0, 50.0, 0.0, 0.0],
            Kind::Label(if names.is_empty() {
                t("sheetData.noQueries", "This workbook has no queries.")
            } else {
                t("sheetData.queries", "Queries:")
            }),
        );
        f.add(
            "list",
            None,
            [18.0, 74.0, 324.0, 232.0],
            Kind::List {
                items: names,
                sel: None,
                top: 0,
            },
        );
        f.ok_cancel(
            &t("commands.sheet.refreshAll", "Refresh All"),
            &t("common.close", "Close"),
        );
        self.form = Some(f);
        self.form_kind = FormKind::Queries;
        true
    }

    /// Consolidate (measured layout of the reference simplified): function, one reference,
    /// labels in top row / left column; the result goes to the active cell.
    fn open_consolidate(&mut self) -> bool {
        let t = |k: &str, d: &str| self.t(k, d);
        let mut f = Form::new(
            &t("commands.sheet.consolidate", "Consolidate"),
            (520.0, 300.0),
            vec![],
        );
        f.add(
            "fnLabel",
            None,
            [18.0, 50.0, 0.0, 0.0],
            Kind::Label(t("sheetData.function", "Function:")),
        );
        let fns = ["Sum", "Count", "Average", "Max", "Min", "Product"];
        f.add(
            "fn",
            None,
            [18.0, 72.0, 200.0, 24.0],
            Kind::Combo {
                items: fns.iter().map(|s| (*s).to_owned()).collect(),
                sel: 0,
            },
        );
        f.add(
            "refLabel",
            None,
            [18.0, 108.0, 0.0, 0.0],
            Kind::Label(t("sheetData.reference", "Reference:")),
        );
        let sel = super::insert_impl::a1(&self.view.rng());
        f.add("ref", None, [18.0, 130.0, 480.0, 25.0], edit(&sel));
        f.add(
            "useLabels",
            None,
            [18.0, 170.0, 0.0, 0.0],
            Kind::Group(t("sheetData.useLabels", "Use labels in")),
        );
        f.add(
            "top",
            None,
            [30.0, 194.0, 200.0, 20.0],
            Kind::Check {
                label: t("sheetData.topRow", "Top row"),
                on: true,
            },
        );
        f.add(
            "left",
            None,
            [30.0, 218.0, 200.0, 20.0],
            Kind::Check {
                label: t("sheetData.leftColumn", "Left column"),
                on: true,
            },
        );
        f.ok_cancel(&t("common.ok", "OK"), &t("common.cancel", "Cancel"));
        f.focus("ref");
        self.form = Some(f);
        self.form_kind = FormKind::Consolidate;
        true
    }

    /// What-If › Data Table: row and column input cells for the selected table.
    fn open_data_table(&mut self) -> bool {
        let t = |k: &str, d: &str| self.t(k, d);
        let mut f = Form::new(
            &t("sheetData.dataTable", "Data Table"),
            (330.0, 170.0),
            vec![],
        );
        f.add(
            "rowLabel",
            None,
            [18.0, 54.0, 0.0, 0.0],
            Kind::Label(t("sheetData.rowInput", "Row input cell:")),
        );
        f.add("row", None, [170.0, 50.0, 140.0, 25.0], edit(""));
        f.add(
            "colLabel",
            None,
            [18.0, 88.0, 0.0, 0.0],
            Kind::Label(t("sheetData.colInput", "Column input cell:")),
        );
        f.add("col", None, [170.0, 84.0, 140.0, 25.0], edit(""));
        f.ok_cancel(&t("common.ok", "OK"), &t("common.cancel", "Cancel"));
        f.focus("row");
        self.form = Some(f);
        self.form_kind = FormKind::WhatIfTable;
        true
    }

    pub(super) fn custom_view_names(&mut self) -> Vec<(String, String)> {
        self.view
            .query(json!({ "cmd": "customViews" }))
            .and_then(|v| v.as_array().cloned())
            .unwrap_or_default()
            .iter()
            .filter_map(|v| {
                Some((
                    v.get("name")?.as_str()?.to_owned(),
                    v.get("guid")?.as_str()?.to_owned(),
                ))
            })
            .collect()
    }

    /// Custom Views: the saved views, Show / Add (with a name) / Delete.
    fn open_custom_views(&mut self) -> bool {
        let views = self.custom_view_names();
        let t = |k: &str, d: &str| self.t(k, d);
        let mut f = Form::new(
            &t("commands.sheet.customViews", "Custom Views"),
            (420.0, 330.0),
            vec![],
        );
        f.add(
            "viewsLabel",
            None,
            [18.0, 50.0, 0.0, 0.0],
            Kind::Label(t("sheetView.views", "Views:")),
        );
        f.add(
            "views",
            None,
            [18.0, 72.0, 260.0, 180.0],
            Kind::List {
                items: views.into_iter().map(|(n, _)| n).collect(),
                sel: Some(0),
                top: 0,
            },
        );
        for (i, (id, key, fb)) in [
            ("show", "sheetView.show", "Show"),
            ("add", "sheetView.add", "Add..."),
            ("delete", "sheetView.delete", "Delete"),
        ]
        .into_iter()
        .enumerate()
        {
            f.add(
                id,
                None,
                [292.0, 72.0 + 36.0 * i as f32, 110.0, 28.0],
                Kind::Button {
                    label: t(key, fb),
                    default: i == 0,
                },
            );
        }
        f.add(
            "nameLabel",
            None,
            [18.0, 264.0, 0.0, 0.0],
            Kind::Label(t("sheetView.name", "Name:")),
        );
        f.add("name", None, [70.0, 260.0, 208.0, 25.0], edit(""));
        f.add(
            "cancel",
            None,
            [292.0, 260.0, 110.0, 28.0],
            Kind::Button {
                label: t("common.close", "Close"),
                default: false,
            },
        );
        self.form = Some(f);
        self.form_kind = FormKind::CustomViews;
        true
    }

    /// Zoom: magnifications as the reference's dialog (200 % … 25 %, Fit selection, Custom).
    fn open_zoom(&mut self) -> bool {
        let t = |k: &str, d: &str| self.t(k, d);
        let mut items: Vec<String> = ZOOMS
            .iter()
            .map(|z| {
                if *z == 0 {
                    t("sheetView.fitSelection", "Fit selection")
                } else {
                    format!("{z}%")
                }
            })
            .collect();
        items.push(t("sheetView.custom", "Custom:"));
        let now = (self.view.zoom() * 100.0).round() as i32;
        let sel = ZOOMS
            .iter()
            .position(|z| *z as i32 == now)
            .unwrap_or(ZOOMS.len());
        // The reference's Zoom (227 × 405 px at 125 %, round 48): a bold caption, option
        // buttons 35 px apart with larger text, Custom with a percent box.
        let mut f = Form::new(
            &t("commands.sheet.zoomDialog", "Zoom"),
            (227.0, 405.0),
            vec![],
        );
        f.set_font_scale(1.3);
        // The reference's newer dialogs have a white face.
        f.add("face", None, [1.0, 38.0, 225.0, 366.0], Kind::Canvas);
        f.add(
            "magLabel",
            None,
            [17.0, 48.0, 190.0, 26.0],
            Kind::Label(t("sheetView.magnification", "Magnification")),
        );
        f.set_bold("magLabel");
        for (i, label) in items.iter().enumerate() {
            let y = if i < ZOOMS.len() {
                87.0 + 35.0 * i as f32
            } else {
                303.0
            };
            f.add(
                ZOOM_IDS[i],
                None,
                [27.0, y, if i < ZOOMS.len() { 170.0 } else { 100.0 }, 28.0],
                Kind::Check {
                    label: label.clone(),
                    on: i == sel,
                },
            );
            // Access letters as the reference: 2(0)0%, (1)00%, (7)5%, (5)0%, (2)5%,
            // F(i)t selection, (C)ustom.
            f.set_access(ZOOM_IDS[i], [1, 0, 0, 0, 0, 1, 0][i]);
        }
        f.set_radios(&ZOOM_IDS);
        f.add(
            "custom",
            None,
            [134.0, 301.0, 47.0, 32.0],
            Kind::Edit {
                value: now.to_string().chars().collect(),
                caret: now.to_string().len(),
            },
        );
        f.add(
            "pct",
            None,
            [186.0, 305.0, 20.0, 26.0],
            Kind::Label("%".into()),
        );
        f.ok_cancel(&t("common.ok", "OK"), &t("common.cancel", "Cancel"));
        if let Some(c) = f.get_mut("ok") {
            c.rect = [57.0, 355.0, 69.0, 33.0];
        }
        if let Some(c) = f.get_mut("cancel") {
            c.rect = [141.0, 355.0, 70.0, 33.0];
        }
        f.focus(ZOOM_IDS[sel.min(ZOOMS.len())]);
        self.form = Some(f);
        self.form_kind = FormKind::Zoom;
        true
    }

    /// Events of the round-6 Data / View forms.
    pub(super) fn data_form_event(&mut self, ev: &Event, text: &dyn crate::TextMeasure) {
        let kind = self.form_kind;
        let Some(f) = self.form.as_mut() else {
            return;
        };
        match (kind, ev) {
            (_, Event::Cancel) | (FormKind::CustomViews, Event::Clicked("cancel")) => {
                self.form = None;
            }
            (FormKind::Queries, Event::Ok) => {
                self.form = None;
                self.refresh_queries();
            }
            (FormKind::Consolidate, Event::Ok) => {
                let fns = ["sum", "count", "average", "max", "min", "product"];
                let function = fns[f.sel("fn").unwrap_or(0).min(fns.len() - 1)];
                let source = f.text("ref");
                let (top, left) = (f.on("top"), f.on("left"));
                let (r, c) = self.view.active();
                let sheet = self.view.sheet();
                self.form = None;
                if !self.view.run(json!({
                    "cmd": "consolidate", "sources": [source], "function": function,
                    "topRow": top, "leftCol": left, "links": false,
                    "dest": { "sheet": sheet, "row": r, "col": c },
                })) {
                    self.status = self.view.take_error().unwrap_or_default();
                }
            }
            (FormKind::WhatIfTable, Event::Ok) => {
                let (row, col) = (f.text("row"), f.text("col"));
                let range = self.view.rng();
                self.form = None;
                let opt = |s: String| (!s.trim().is_empty()).then(|| s.trim().replace('$', ""));
                if !self.view.run(json!({
                    "cmd": "dataTable", "range": range, "rowInput": opt(row), "colInput": opt(col),
                })) {
                    self.status = self.view.take_error().unwrap_or_default();
                }
            }
            (FormKind::CustomViews, Event::Clicked("show") | Event::Ok) => {
                let i = f.sel("views").unwrap_or(0);
                let views = self.custom_view_names();
                if let Some((_, guid)) = views.get(i) {
                    let shown = self
                        .view
                        .query(json!({ "cmd": "applyCustomView", "guid": guid }));
                    self.view.changed();
                    if let Some(v) = shown {
                        if let Some(z) = v.get("zoom").and_then(Value::as_f64) {
                            self.zoom_to((z / 100.0) as f32, text);
                        }
                    }
                }
                self.form = None;
            }
            (FormKind::CustomViews, Event::Clicked("add")) => {
                let name = f.text("name");
                let (r, c) = self.view.active();
                let zoom = (self.view.zoom() * 100.0).round() as u32;
                if self
                    .view
                    .query(json!({
                        "cmd": "addCustomView", "name": name, "zoom": zoom, "active": [r, c],
                    }))
                    .is_some()
                {
                    self.view.changed();
                    let names = self
                        .custom_view_names()
                        .into_iter()
                        .map(|(n, _)| n)
                        .collect();
                    if let Some(f) = self.form.as_mut() {
                        f.set_items("views", names, None);
                        f.set_text("name", "");
                    }
                } else {
                    self.status = self.view.take_error().unwrap_or_default();
                }
            }
            (FormKind::CustomViews, Event::Clicked("delete")) => {
                let i = f.sel("views").unwrap_or(0);
                let views = self.custom_view_names();
                if let Some((_, guid)) = views.get(i) {
                    self.view
                        .query(json!({ "cmd": "deleteCustomView", "guid": guid }));
                    self.view.changed();
                }
                let names = self
                    .custom_view_names()
                    .into_iter()
                    .map(|(n, _)| n)
                    .collect();
                if let Some(f) = self.form.as_mut() {
                    f.set_items("views", names, Some(0));
                }
            }
            (FormKind::Zoom, Event::Changed(id)) if ZOOM_IDS.contains(id) || *id == "custom" => {
                let pick = if *id == "custom" { ZOOM_IDS[6] } else { *id };
                for z in ZOOM_IDS {
                    f.set_on(z, z == pick);
                }
            }
            (FormKind::Zoom, Event::Ok) => {
                let i = ZOOM_IDS.iter().position(|z| f.on(z)).unwrap_or(1);
                let custom: i32 = f
                    .text("custom")
                    .trim()
                    .parse()
                    .unwrap_or(100)
                    .clamp(10, 400);
                self.form = None;
                match ZOOMS.get(i) {
                    Some(0) => {
                        self.action("sheet.zoomSelection", text);
                    }
                    Some(z) => {
                        self.zoom_to(*z as f32 / 100.0, text);
                    }
                    None => {
                        self.zoom_to(custom as f32 / 100.0, text);
                    }
                }
            }
            _ => {}
        }
    }

    /// Get Data › From Text/CSV: ask the host for a text file.
    pub(super) fn pick_text_file(&mut self) -> bool {
        self.pick = Some("text");
        self.requests.push(crate::Request::PickMergeData);
        true
    }

    /// Load text file `bytes` (named `name`) as a query whose table goes to a new sheet, with
    /// the steps the engine proposes (promoted headers, detected column types), as the
    /// reference's Load button does.
    pub(super) fn import_text(&mut self, bytes: &[u8], name: &str) -> bool {
        use base64::Engine as _;
        let file = std::path::Path::new(name)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("data.csv")
            .to_owned();
        let data = base64::engine::general_purpose::STANDARD.encode(bytes);
        let source = json!({ "kind": "csv", "file": file });
        let ok = self.view.query(json!({
            "cmd": "querySource", "key": file, "access": "file",
            "files": [{ "name": file, "data": data }],
        }));
        let steps = ok
            .and_then(|_| {
                self.view
                    .query(json!({ "cmd": "queryInspect", "source": source }))
            })
            .and_then(|v| v.get("suggested").cloned())
            .unwrap_or_else(|| json!([]));
        let stem = std::path::Path::new(&file)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Query")
            .to_owned();
        let before = self.view.sheet_names().len();
        let saved = self.view.query(json!({
            "cmd": "querySave",
            "query": { "name": stem, "source": source, "steps": steps },
            "load": "newSheet",
        }));
        self.view.changed();
        if saved.is_none() {
            let _ = self.view.take_error();
            self.status = self.t(
                "sheetData.importFailed",
                "The file could not be read as text.",
            );
            return true;
        }
        let after = self.view.sheet_names().len();
        if after > before {
            self.view.set_sheet(after - 1);
        }
        true
    }

    /// Refresh All: re-run every query of the workbook.
    pub(super) fn refresh_queries(&mut self) -> bool {
        let r = self.view.query(json!({ "cmd": "queryRefresh" }));
        self.view.changed();
        let n = r
            .as_ref()
            .and_then(|v| v.get("refreshed"))
            .and_then(Value::as_array)
            .map_or(0, Vec::len);
        self.status = self
            .t("sheetData.refreshed", "{n} queries refreshed")
            .replace("{n}", &n.to_string());
        true
    }
}
