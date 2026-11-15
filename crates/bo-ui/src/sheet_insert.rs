//! BlueSheet's Insert tab (round 16): Create Table, Create PivotTable (with row / value
//! fields, as there is no field list pane yet), Create Sparklines, Insert Hyperlink (a
//! `HYPERLINK` formula), Header & Footer (`setPageSetup.headerFooter`) and the Symbol grid,
//! plus own icons for the new buttons. Layouts follow the reference spreadsheet's dialogs
//! (black-box), drawn with BlueSheet's form controls.
use super::form_impl::{Event, Form, FormKind, Kind};
use super::SheetWindow;
use crate::icons::Icons;
use crate::{Painter, TextMeasure, TextStyle};
use bo_render::Rgba;
use serde_json::{json, Value};

const BLUE: Rgba = Rgba(0x2e, 0x75, 0xb6, 255);
const PALE: Rgba = Rgba(0xd6, 0xe4, 0xf2, 255);
const GREEN: Rgba = Rgba(0x21, 0x73, 0x46, 255);
const ORANGE: Rgba = Rgba(0xed, 0x7d, 0x31, 255);
const GREY: Rgba = Rgba(0x8a, 0x8a, 0x8a, 255);
const RED: Rgba = Rgba(0xc0, 0x39, 0x2b, 255);

/// Characters of the Symbol grid (Segoe UI: Latin-1, punctuation, arrows, maths, currency).
pub(super) fn symbols() -> Vec<char> {
    let mut v: Vec<char> = (0x21u32..0x7f)
        .chain(0xa1..0x180)
        .chain(0x2010..0x2027)
        .chain(0x2030..0x203b)
        .chain(0x20a0..0x20c0)
        .chain(0x2100..0x2123)
        .chain(0x2190..0x21a0)
        .chain(0x2200..0x2270)
        .chain(0x25a0..0x25d0)
        .filter_map(char::from_u32)
        .collect();
    v.dedup();
    v
}

/// Width and height of a PNG picture (ECMA-376 pictures are commonly PNG; IHDR, RFC 2083).
fn png_size(b: &[u8]) -> Option<(u32, u32)> {
    if b.len() < 24 || !b.starts_with(b"\x89PNG\r\n\x1a\n") {
        return None;
    }
    let w = u32::from_be_bytes(b[16..20].try_into().ok()?);
    let h = u32::from_be_bytes(b[20..24].try_into().ok()?);
    Some((w, h))
}

/// Symbol grid: columns and cell size (reference pixels).
const SYM_COLS: usize = 16;
const SYM_CELL: f32 = 30.0;
const SYM_ROWS: usize = 8;

/// Own drawings for the Insert tab's buttons (24-unit grid).
pub(crate) fn add_icons(icons: &mut Icons) {
    icons.alias("sheet.pivotTable", "insert.pivot");
    icons.alias("sheet.link", "insert.link");
    icons.alias("sheet.pictures", "insert.picture");
    icons.alias("sheet.shapes", "insert.rect");
    icons.alias("sheet.slicer", "insert.slicer");
    icons.add_solid(
        "sheet.pivotChart",
        &[
            ("M3 3h11v14h-11Z", Some(PALE)),
            ("M3 3h11v14h-11Z", None),
            ("M3 7h11M7 3v14", None),
            ("M14 13h3v8h-3Z", Some(BLUE)),
            ("M18 9h3v12h-3Z", Some(ORANGE)),
        ],
    );
    // Text: a framed box with a blue letter, like the reference's Text button (own drawing).
    icons.add_solid(
        "sheet.text",
        &[
            ("M3 3h18v18h-18Z", None),
            (
                "M8 17l3.2-10h1.6l3.2 10h-1.8l-0.8-2.6h-2.8l-0.8 2.6Z",
                Some(BLUE),
            ),
        ],
    );
    icons.alias("sheet.symbol", "insert.symbol");
    icons.alias("sheet.symbols", "insert.symbol");
    icons.alias("sheet.illustrations", "insert.rect");
    // Icons: a person and a star; SmartArt: three linked boxes; Screenshot: a frame with a
    // camera corner; 3D Models: a cube; Timeline: a ruler of dated slots; Equation: a pi
    // sign (own drawings).
    icons.add_solid(
        "sheet.icons",
        &[
            ("M7 5a3 3 0 1 0 0.1 0Z", Some(BLUE)),
            ("M2 18c0-4 2-6 5-6s5 2 5 6Z", Some(BLUE)),
            (
                "M17 7l1.3 2.7l3 0.4l-2.2 2l0.6 3l-2.7-1.5l-2.7 1.5l0.6-3l-2.2-2l3-0.4Z",
                Some(ORANGE),
            ),
        ],
    );
    icons.add_solid(
        "sheet.smartArt",
        &[
            ("M2 9h6v6h-6Z", Some(GREEN)),
            ("M9 4h6v6h-6Z", Some(GREEN)),
            ("M16 9h6v6h-6Z", Some(GREEN)),
            ("M8 12h1M15 7h1v5", None),
        ],
    );
    icons.add_solid(
        "sheet.screenshot",
        &[
            ("M2 6h20v14h-20Z", Some(PALE)),
            ("M2 6h20v14h-20Z", None),
            ("M2 16h4v4h-4Z", Some(GREY)),
        ],
    );
    icons.add_solid(
        "sheet.models3d",
        &[
            ("M12 3l8 4.5v9l-8 4.5l-8-4.5v-9Z", Some(PALE)),
            ("M12 3l8 4.5v9l-8 4.5l-8-4.5v-9Z", None),
            ("M4 7.5l8 4.5l8-4.5M12 12v9", None),
        ],
    );
    icons.add_solid(
        "sheet.timeline",
        &[
            ("M2 5h20v14h-20Z", Some(PALE)),
            ("M2 5h20v14h-20Z", None),
            ("M5 13h6v3h-6Z", Some(BLUE)),
            ("M12 13h3v3h-3Z", Some(BLUE)),
            ("M2 9h20", None),
        ],
    );
    icons.add_solid(
        "sheet.equation",
        &[(
            "M4 6h16v2.4h-3.2v10h-2.6v-10h-4.4v6.5c0 2-1 3.5-3 3.5h-1v-2.4h0.6c0.6 0 0.8-0.4 0.8-1.2v-6.4h-3.2Z",
            Some(BLUE),
        )],
    );
    icons.alias("sheet.headerFooter", "insert.header");
    // Filled chart type pictures like the reference's Charts group (own drawings).
    icons.add_solid(
        "sheet.chartColumn",
        &[
            ("M4 12h4v8h-4Z", Some(BLUE)),
            ("M10 5h4v15h-4Z", Some(BLUE)),
            ("M16 9h4v11h-4Z", Some(BLUE)),
        ],
    );
    icons.add_solid(
        "sheet.chartLine",
        &[
            ("M3 3v18h18", None),
            ("M5 16l4-6l4 3l6-8", None),
            ("M5 19l4-3l4 1l6-4", None),
        ],
    );
    icons.add_solid(
        "sheet.chartPie",
        &[
            ("M11 4a8 8 0 1 0 8 8h-8Z", Some(BLUE)),
            ("M13 2a8 8 0 0 1 8 8h-8Z", Some(ORANGE)),
        ],
    );
    icons.add_solid(
        "sheet.recPivot",
        &[
            ("M3 3h13v15h-13Z", Some(PALE)),
            ("M3 3h13v15h-13Z", None),
            ("M3 7h13M7 3v15", None),
            ("M15 13a4 4 0 1 0 0.1 0Z", Some(BLUE)),
            ("M14.2 15.2h1.6v3h-1.6Z", Some(Rgba(255, 255, 255, 255))),
        ],
    );
    icons.add_solid(
        "sheet.recCharts",
        &[
            ("M3 12h4v9h-4Z", Some(BLUE)),
            ("M9 6h4v15h-4Z", Some(ORANGE)),
            ("M15 9h4v12h-4Z", Some(GREY)),
            ("M17 2a4 4 0 1 0 0.1 0Z", Some(BLUE)),
        ],
    );
    icons.add_solid(
        "sheet.chartHier",
        &[
            ("M3 4h10v16h-10Z", Some(BLUE)),
            ("M14 4h7v9h-7Z", Some(ORANGE)),
            ("M14 14h7v6h-7Z", Some(GREY)),
        ],
    );
    icons.add_solid(
        "sheet.chartWaterfall",
        &[
            ("M3 10h4v10h-4Z", Some(BLUE)),
            ("M8 6h4v4h-4Z", Some(GREEN)),
            ("M13 6h4v7h-4Z", Some(RED)),
            ("M18 13h3v7h-3Z", Some(BLUE)),
        ],
    );
    icons.add_solid(
        "sheet.chartStat",
        &[
            ("M3 14h3v6h-3Z", Some(BLUE)),
            ("M7 8h3v12h-3Z", Some(BLUE)),
            ("M11 4h3v16h-3Z", Some(BLUE)),
            ("M15 10h3v10h-3Z", Some(BLUE)),
            ("M19 16h2v4h-2Z", Some(BLUE)),
        ],
    );
    icons.add_solid(
        "sheet.chartRadar",
        &[
            ("M12 2l9 7l-3.5 11h-11l-3.5-11Z", None),
            ("M12 7l5 4l-2 6h-6l-2-6Z", Some(PALE)),
            ("M12 7l5 4l-2 6h-6l-2-6Z", None),
        ],
    );
    icons.add_solid(
        "sheet.chartScatter",
        &[
            ("M3 3v18h18", None),
            ("M7 15h2.5v2.5h-2.5Z", Some(BLUE)),
            ("M11 10h2.5v2.5h-2.5Z", Some(BLUE)),
            ("M15 12h2.5v2.5h-2.5Z", Some(ORANGE)),
            ("M17 6h2.5v2.5h-2.5Z", Some(ORANGE)),
        ],
    );
    icons.add_solid(
        "sheet.chart3D",
        &[
            ("M4 11l3-2h4l-3 2Z", Some(PALE)),
            ("M4 11h4v10h-4Z", Some(BLUE)),
            ("M8 11l3-2v10l-3 2Z", Some(PALE)),
            ("M13 6l3-2h4l-3 2Z", Some(PALE)),
            ("M13 6h4v15h-4Z", Some(ORANGE)),
            ("M17 6l3-2v15l-3 2Z", Some(PALE)),
        ],
    );
    icons.add_solid(
        "sheet.chartMap",
        &[
            ("M12 2a10 10 0 1 0 0.1 0Z", Some(PALE)),
            ("M12 2a10 10 0 1 0 0.1 0Z", None),
            ("M6 7l4-1l2 3l-2 4l-3-1Z", Some(GREEN)),
            ("M14 12l4 1l-1 5l-3-1Z", Some(GREEN)),
        ],
    );
    icons.add_solid(
        "sheet.sparkLine",
        &[("M3 4h18v16h-18Z", None), ("M5 15l4-5l3 3l3-6l4 5", None)],
    );
    icons.add_solid(
        "sheet.sparkColumn",
        &[
            ("M3 4h18v16h-18Z", None),
            ("M5 12h2v6h-2Z", Some(BLUE)),
            ("M9 8h2v10h-2Z", Some(BLUE)),
            ("M13 13h2v5h-2Z", Some(BLUE)),
            ("M17 7h2v11h-2Z", Some(BLUE)),
        ],
    );
    icons.add_solid(
        "sheet.sparkWinLoss",
        &[
            ("M3 4h18v16h-18Z", None),
            ("M5 8h2v4h-2Z", Some(BLUE)),
            ("M9 12h2v4h-2Z", Some(RED)),
            ("M13 8h2v4h-2Z", Some(BLUE)),
            ("M17 8h2v4h-2Z", Some(BLUE)),
        ],
    );
}

fn edit(value: &str) -> Kind {
    Kind::Edit {
        value: value.chars().collect(),
        caret: value.chars().count(),
    }
}

/// `R0C0:R1C1` range JSON as A1 text (`$A$1:$D$13`).
pub(super) fn a1(r: &Value) -> String {
    let n = |k: &str| r.get(k).and_then(Value::as_u64).unwrap_or(0) as u32;
    let cell = |row: u32, col: u32| {
        format!(
            "${}${}",
            bo_model::sheet::CellRef::new(row, col)
                .to_string()
                .trim_end_matches(char::is_numeric),
            row + 1
        )
    };
    let (a, b) = (cell(n("r0"), n("c0")), cell(n("r1"), n("c1")));
    if a == b {
        a
    } else {
        format!("{a}:{b}")
    }
}

/// A1 text (`$A$1:$B$2`, `A1`) as range JSON.
fn parse_a1(s: &str) -> Option<Value> {
    let s = s.trim().replace('$', "");
    let s = s.rsplit('!').next()?.to_owned();
    let (a, b) = s.split_once(':').unwrap_or((&s, &s));
    let a = bo_model::sheet::CellRef::parse(a.trim())?;
    let b = bo_model::sheet::CellRef::parse(b.trim())?;
    Some(json!({
        "r0": a.row.min(b.row), "c0": a.col.min(b.col),
        "r1": a.row.max(b.row), "c1": a.col.max(b.col),
    }))
}

/// Split an `&L…&C…&R…` header / footer into its three sections.
pub(super) fn hf_sections(s: &str) -> [String; 3] {
    let mut out = [String::new(), String::new(), String::new()];
    let mut cur = 1;
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        if c == '&' {
            match it.peek().copied() {
                Some('L') => {
                    cur = 0;
                    it.next();
                    continue;
                }
                Some('C') => {
                    cur = 1;
                    it.next();
                    continue;
                }
                Some('R') => {
                    cur = 2;
                    it.next();
                    continue;
                }
                _ => {}
            }
        }
        out[cur].push(c);
    }
    out
}

/// Join three sections into `&L…&C…&R…` (empty sections left out).
pub(super) fn hf_join(l: &str, c: &str, r: &str) -> String {
    let mut s = String::new();
    for (code, t) in [("&L", l), ("&C", c), ("&R", r)] {
        if !t.is_empty() {
            s.push_str(code);
            s.push_str(t);
        }
    }
    s
}

impl SheetWindow {
    /// Insert tab commands that open BlueSheet's forms; `None` for other ids.
    pub(super) fn insert_action(&mut self, id: &str) -> Option<bool> {
        Some(match id {
            "sheet.table" => self.open_create_table(),
            "sheet.pivotTable" => self.open_create_pivot(false),
            "sheet.recPivot" => self.open_create_pivot(true),
            "sheet.sparkLine" => self.open_sparklines("line"),
            "sheet.sparkColumn" => self.open_sparklines("column"),
            "sheet.sparkWinLoss" => self.open_sparklines("stacked"),
            "sheet.link" => self.open_link(),
            "sheet.headerFooter" => self.open_header_footer(),
            "sheet.symbol" => self.open_symbol(),
            "sheet.pictures" => {
                self.requests.push(crate::Request::PickPicture);
                true
            }
            "sheet.pivotChart" => self.insert_pivot_chart(),
            "sheet.slicer" => self.insert_slicer(),
            _ if id.starts_with("shape.") => {
                let (r, c) = self.view.active();
                self.view.run(json!({
                    "cmd": "insertDrawing", "from": [r, c], "to": [r + 6, c + 3],
                    "shape": &id["shape.".len()..],
                }))
            }
            "sheet.chartMap" => {
                let region = self.region();
                self.view
                    .run(json!({ "cmd": "insertChart", "range": region, "kind": "regionMap" }))
            }
            _ => return None,
        })
    }

    /// Insert picture `bytes` at the active cell, as large as the picture at 100 % (PNG
    /// size; other formats 8 columns × 15 rows); `name` is the file name (unused).
    pub fn insert_picture(&mut self, bytes: &[u8], _name: &str) -> bool {
        use base64::Engine as _;
        let (cols, rows) = png_size(bytes).map_or((8, 15), |(w, h)| {
            // Default column 64 px and row 20 px at 100 %.
            ((w / 64).clamp(1, 20), (h / 20).clamp(1, 40))
        });
        let (r, c) = self.view.active();
        let image = base64::engine::general_purpose::STANDARD.encode(bytes);
        self.view.run(json!({
            "cmd": "insertDrawing", "from": [r, c], "to": [r + rows, c + cols], "image": image,
        }))
    }

    /// PivotChart: a recommended pivot table on a new sheet and a column chart of it.
    fn insert_pivot_chart(&mut self) -> bool {
        let region = self.region();
        let n = |k: &str| region.get(k).and_then(Value::as_u64).unwrap_or(0) as u32;
        let numeric: Vec<bool> = (n("c0")..=n("c1"))
            .map(|c| {
                self.view
                    .engine()
                    .cell(self.view.sheet(), n("r0") + 1, c)
                    .is_ok_and(|d| d.input.trim().parse::<f64>().is_ok())
            })
            .collect();
        let rows = numeric.iter().position(|v| !v).unwrap_or(0);
        let values = numeric.iter().position(|v| *v).unwrap_or(0);
        let before = self.view.sheet_names().len();
        let made = self.view.query(json!({
            "cmd": "pivotCreate", "range": region, "rows": [rows],
            "values": [{ "field": values, "function": "sum" }],
        }));
        if made.is_none() {
            return false;
        }
        self.view.changed();
        let after = self.view.sheet_names().len();
        if after > before {
            self.view.set_sheet(after - 1);
        }
        self.view
            .run(json!({ "cmd": "pivotChart", "index": 0, "kind": "column" }))
    }

    /// Slicer for the table column of the active cell (the reference asks for a table).
    fn insert_slicer(&mut self) -> bool {
        let (r, c) = self.view.active();
        if self
            .view
            .run(json!({ "cmd": "insertSlicer", "row": r, "col": c }))
        {
            return true;
        }
        let _ = self.view.take_error();
        self.status = self.t(
            "sheetInsert.slicerNeedsTable",
            "Select a cell in a table to insert a slicer.",
        );
        true
    }

    fn open_create_table(&mut self) -> bool {
        let region = self.region();
        let header = self
            .view
            .query(json!({ "cmd": "sortRegion", "range": region }))
            .and_then(|p| p.get("hasHeader").and_then(Value::as_bool))
            .unwrap_or(true);
        let t = |k: &str, d: &str| self.t(k, d);
        let mut f = Form::new(
            &t("sheetInsert.createTable", "Create Table"),
            (360.0, 200.0),
            vec![],
        );
        f.add(
            "where",
            None,
            [20.0, 54.0, 0.0, 0.0],
            Kind::Label(t(
                "sheetInsert.tableWhere",
                "Where is the data for your table?",
            )),
        );
        f.add("range", None, [40.0, 78.0, 300.0, 25.0], edit(&a1(&region)));
        f.add(
            "headers",
            None,
            [40.0, 114.0, 280.0, 20.0],
            Kind::Check {
                label: t("sheetInsert.hasHeaders", "My table has headers"),
                on: header,
            },
        );
        f.ok_cancel(&t("common.ok", "OK"), &t("common.cancel", "Cancel"));
        f.focus("range");
        self.form = Some(f);
        self.form_kind = FormKind::CreateTable;
        true
    }

    /// Column labels of a range's first row (pivot fields).
    fn field_names(&self, range: &Value) -> Vec<String> {
        let n = |k: &str| range.get(k).and_then(Value::as_u64).unwrap_or(0) as u32;
        (n("c0")..=n("c1"))
            .map(|c| {
                self.view
                    .engine()
                    .cell(self.view.sheet(), n("r0"), c)
                    .map(|d| d.display)
                    .unwrap_or_default()
            })
            .collect()
    }

    fn open_create_pivot(&mut self, recommended: bool) -> bool {
        let region = self.region();
        let fields = self.field_names(&region);
        // Recommended: the first text column as rows, the first numeric column as values.
        let n = |k: &str| region.get(k).and_then(Value::as_u64).unwrap_or(0) as u32;
        let numeric: Vec<bool> = (n("c0")..=n("c1"))
            .map(|c| {
                self.view
                    .engine()
                    .cell(self.view.sheet(), n("r0") + 1, c)
                    .is_ok_and(|d| d.input.trim().parse::<f64>().is_ok())
            })
            .collect();
        let row_pick = numeric.iter().position(|v| !v).unwrap_or(0);
        let value_pick = numeric.iter().position(|v| *v).unwrap_or(0);
        let t = |k: &str, d: &str| self.t(k, d);
        let title = if recommended {
            t("sheetInsert.recPivotTitle", "Recommended PivotTables")
        } else {
            t("sheetInsert.createPivot", "Create PivotTable")
        };
        let mut f = Form::new(&title, (520.0, 380.0), vec![]);
        f.add(
            "choose",
            None,
            [20.0, 54.0, 480.0, 0.0],
            Kind::Group(t(
                "sheetInsert.pivotData",
                "Choose the data that you want to analyze",
            )),
        );
        f.add(
            "rangeLabel",
            None,
            [40.0, 82.0, 0.0, 0.0],
            Kind::Label(t("sheetInsert.tableRange", "Table/Range:")),
        );
        f.add(
            "range",
            None,
            [150.0, 78.0, 350.0, 25.0],
            edit(&a1(&region)),
        );
        f.add(
            "place",
            None,
            [20.0, 118.0, 480.0, 0.0],
            Kind::Group(t(
                "sheetInsert.pivotWhere",
                "Choose where you want the PivotTable report to be placed",
            )),
        );
        f.add(
            "newSheet",
            None,
            [40.0, 142.0, 200.0, 20.0],
            Kind::Check {
                label: t("sheetInsert.newWorksheet", "New Worksheet"),
                on: true,
            },
        );
        f.add(
            "existing",
            None,
            [40.0, 168.0, 200.0, 20.0],
            Kind::Check {
                label: t("sheetInsert.existingWorksheet", "Existing Worksheet"),
                on: false,
            },
        );
        f.add(
            "locLabel",
            None,
            [60.0, 198.0, 0.0, 0.0],
            Kind::Label(t("sheetInsert.location", "Location:")),
        );
        f.add("loc", None, [150.0, 194.0, 350.0, 25.0], edit(""));
        f.enable("loc", false);
        f.add(
            "fields",
            None,
            [20.0, 234.0, 480.0, 0.0],
            Kind::Group(t("sheetInsert.pivotFields", "PivotTable fields")),
        );
        f.add(
            "rowsLabel",
            None,
            [40.0, 262.0, 0.0, 0.0],
            Kind::Label(t("sheetInsert.rowsField", "Rows:")),
        );
        f.add(
            "rows",
            None,
            [150.0, 258.0, 350.0, 25.0],
            Kind::Combo {
                items: fields.clone(),
                sel: row_pick,
            },
        );
        f.add(
            "valuesLabel",
            None,
            [40.0, 296.0, 0.0, 0.0],
            Kind::Label(t("sheetInsert.valuesField", "Sum of:")),
        );
        f.add(
            "values",
            None,
            [150.0, 292.0, 350.0, 25.0],
            Kind::Combo {
                items: fields,
                sel: value_pick,
            },
        );
        f.ok_cancel(&t("common.ok", "OK"), &t("common.cancel", "Cancel"));
        self.form = Some(f);
        self.form_kind = FormKind::CreatePivot;
        true
    }

    fn open_sparklines(&mut self, kind: &str) -> bool {
        let range = self.view.rng();
        let t = |k: &str, d: &str| self.t(k, d);
        let mut f = Form::new(
            &t("sheetInsert.createSparklines", "Create Sparklines"),
            (440.0, 250.0),
            vec![],
        );
        f.add(
            "dataGroup",
            None,
            [20.0, 54.0, 400.0, 0.0],
            Kind::Group(t("sheetInsert.sparkData", "Choose the data that you want")),
        );
        f.add(
            "dataLabel",
            None,
            [40.0, 82.0, 0.0, 0.0],
            Kind::Label(t("sheetInsert.dataRange", "Data Range:")),
        );
        f.add(
            "data",
            None,
            [160.0, 78.0, 260.0, 25.0],
            edit(&a1(&range).replace('$', "")),
        );
        f.add(
            "locGroup",
            None,
            [20.0, 118.0, 400.0, 0.0],
            Kind::Group(t(
                "sheetInsert.sparkWhere",
                "Choose where you want the sparklines to be placed",
            )),
        );
        f.add(
            "locLabel",
            None,
            [40.0, 146.0, 0.0, 0.0],
            Kind::Label(t("sheetInsert.locationRange", "Location Range:")),
        );
        f.add("loc", None, [160.0, 142.0, 260.0, 25.0], edit(""));
        f.ok_cancel(&t("common.ok", "OK"), &t("common.cancel", "Cancel"));
        f.focus("loc");
        self.form = Some(f);
        self.form_kind = FormKind::Sparklines;
        self.spark_kind = kind.to_owned();
        true
    }

    fn open_link(&mut self) -> bool {
        let (r, c) = self.view.active();
        let shown = self
            .view
            .engine()
            .cell(self.view.sheet(), r, c)
            .map(|d| d.display)
            .unwrap_or_default();
        let t = |k: &str, d: &str| self.t(k, d);
        let mut f = Form::new(
            &t("sheetInsert.linkTitle", "Insert Hyperlink"),
            (560.0, 210.0),
            vec![],
        );
        f.add(
            "textLabel",
            None,
            [20.0, 60.0, 0.0, 0.0],
            Kind::Label(t("sheetInsert.linkText", "Text to display:")),
        );
        f.add("text", None, [140.0, 56.0, 400.0, 25.0], edit(&shown));
        f.add(
            "addrLabel",
            None,
            [20.0, 100.0, 0.0, 0.0],
            Kind::Label(t("sheetInsert.linkAddress", "Address:")),
        );
        f.add("addr", None, [140.0, 96.0, 400.0, 25.0], edit(""));
        f.ok_cancel(&t("common.ok", "OK"), &t("common.cancel", "Cancel"));
        f.focus("addr");
        self.form = Some(f);
        self.form_kind = FormKind::Link;
        true
    }

    fn open_header_footer(&mut self) -> bool {
        let (head, foot) = self
            .view
            .engine()
            .workbook()
            .and_then(|w| w.sheets.get(self.view.sheet()))
            .map(|ws| {
                (
                    ws.page_setup.odd_header.clone().unwrap_or_default(),
                    ws.page_setup.odd_footer.clone().unwrap_or_default(),
                )
            })
            .unwrap_or_default();
        let (h, ft) = (hf_sections(&head), hf_sections(&foot));
        let t = |k: &str, d: &str| self.t(k, d);
        let mut f = Form::new(
            &t("sheetInsert.hfTitle", "Header/Footer"),
            (600.0, 280.0),
            vec![],
        );
        let cols = [
            ("sheetInsert.left", "Left section:"),
            ("sheetInsert.center", "Center section:"),
            ("sheetInsert.right", "Right section:"),
        ];
        let ids = [["hl", "hc", "hr"], ["fl", "fc", "fr"]];
        for (row, (label, values)) in [
            (t("sheetInsert.header", "Header"), &h),
            (t("sheetInsert.footer", "Footer"), &ft),
        ]
        .into_iter()
        .enumerate()
        {
            let y = 54.0 + row as f32 * 84.0;
            f.add(
                if row == 0 { "hGroup" } else { "fGroup" },
                None,
                [20.0, y, 560.0, 0.0],
                Kind::Group(label),
            );
            for (i, (k, d)) in cols.iter().enumerate() {
                let x = 20.0 + i as f32 * 190.0;
                f.add(
                    ["l0", "l1", "l2", "l3", "l4", "l5"][row * 3 + i],
                    None,
                    [x, y + 22.0, 0.0, 0.0],
                    Kind::Label(t(k, d)),
                );
                f.add(
                    ids[row][i],
                    None,
                    [x, y + 42.0, 176.0, 25.0],
                    edit(&values[i]),
                );
            }
        }
        f.ok_cancel(&t("common.ok", "OK"), &t("common.cancel", "Cancel"));
        f.focus("hc");
        self.form = Some(f);
        self.form_kind = FormKind::HeaderFooter;
        true
    }

    fn open_symbol(&mut self) -> bool {
        let t = |k: &str, d: &str| self.t(k, d);
        let w = 20.0 * 2.0 + SYM_CELL * SYM_COLS as f32;
        let h = 120.0 + SYM_CELL * SYM_ROWS as f32 + 60.0;
        let mut f = Form::new(&t("sheetInsert.symbolTitle", "Symbol"), (w, h), vec![]);
        // Pages of the grid by their first and last character (the reference's Subset box).
        let all = symbols();
        let pages: Vec<String> = all
            .chunks(SYM_COLS * SYM_ROWS)
            .map(|p| format!("U+{:04X} – U+{:04X}", p[0] as u32, p[p.len() - 1] as u32))
            .collect();
        f.add(
            "subsetLabel",
            None,
            [20.0, 56.0, 0.0, 0.0],
            Kind::Label(t("sheetInsert.subset", "Subset:")),
        );
        f.add(
            "subset",
            None,
            [90.0, 52.0, 260.0, 25.0],
            Kind::Combo {
                items: pages,
                sel: 0,
            },
        );
        f.add(
            "grid",
            None,
            [
                20.0,
                90.0,
                SYM_CELL * SYM_COLS as f32,
                SYM_CELL * SYM_ROWS as f32,
            ],
            Kind::Canvas,
        );
        f.add(
            "code",
            None,
            [20.0, 100.0 + SYM_CELL * SYM_ROWS as f32, 0.0, 0.0],
            Kind::Label(String::new()),
        );
        let (w0, h0) = f.size;
        f.add(
            "insert",
            None,
            [w0 - 258.0, h0 - 48.0, 112.0, 28.0],
            Kind::Button {
                label: t("sheetInsert.insert", "Insert"),
                default: true,
            },
        );
        f.add(
            "cancel",
            None,
            [w0 - 131.0, h0 - 48.0, 112.0, 28.0],
            Kind::Button {
                label: t("common.cancel", "Cancel"),
                default: false,
            },
        );
        self.form = Some(f);
        self.form_kind = FormKind::Symbol;
        self.symbol = (0, 0);
        self.symbol_code();
        true
    }

    fn symbol_code(&mut self) {
        let all = symbols();
        let i = self.symbol.0 * SYM_COLS + self.symbol.1;
        let label = all.get(i).map_or_else(String::new, |c| {
            format!(
                "{} U+{:04X}",
                self.t("sheetInsert.charCode", "Character code:"),
                *c as u32
            )
        });
        if let Some(Kind::Label(s)) = self
            .form
            .as_mut()
            .and_then(|f| f.get_mut("code"))
            .map(|c| &mut c.kind)
        {
            *s = label;
        }
    }

    /// Paint the Symbol grid (first rows scrolled by `symbol.0 / SYM_ROWS` pages).
    pub(super) fn paint_symbol_grid(&self, f: &Form, r: [f32; 4], p: &mut dyn Painter) {
        let all = symbols();
        let k = r[2] / (SYM_CELL * SYM_COLS as f32);
        let cell = SYM_CELL * k;
        let first = (self.symbol.0 / SYM_ROWS) * SYM_ROWS;
        let line = Rgba(0xa0, 0xa0, 0xa0, 255);
        p.fill_rect(r[0], r[1], r[2], r[3], Rgba(255, 255, 255, 255));
        let font = f.font();
        let style = TextStyle {
            size: (font.size * 1.25).round(),
            bold: false,
        };
        for row in 0..SYM_ROWS {
            for col in 0..SYM_COLS {
                let i = (first + row) * SYM_COLS + col;
                let (x, y) = (r[0] + col as f32 * cell, r[1] + row as f32 * cell);
                let sel = self.symbol == (first + row, col);
                if sel {
                    p.fill_rect(x, y, cell, cell, Rgba(0x00, 0x78, 0xd7, 255));
                }
                if let Some(c) = all.get(i) {
                    let s = c.to_string();
                    p.text(
                        &s,
                        (x + cell * 0.3).round(),
                        (y + (cell - style.size * 1.33) / 2.0).round(),
                        style,
                        if sel {
                            Rgba(255, 255, 255, 255)
                        } else {
                            Rgba(0, 0, 0, 255)
                        },
                    );
                }
            }
        }
        for i in 0..=SYM_COLS {
            p.fill_rect((r[0] + i as f32 * cell).round(), r[1], 1.0, r[3], line);
        }
        for i in 0..=SYM_ROWS {
            p.fill_rect(r[0], (r[1] + i as f32 * cell).round(), r[2], 1.0, line);
        }
    }

    /// Events of the Insert tab's forms.
    #[allow(clippy::too_many_lines)]
    pub(super) fn insert_form_event(&mut self, ev: &Event, text: &dyn TextMeasure) {
        let kind = self.form_kind;
        let Some(f) = self.form.as_mut() else {
            return;
        };
        match (kind, ev) {
            (_, Event::Cancel) | (FormKind::Symbol, Event::Clicked("cancel")) => {
                self.form = None;
            }
            (FormKind::CreatePivot, Event::Changed("newSheet")) => {
                let on = f.on("newSheet");
                f.set_on("existing", !on);
                f.enable("loc", !on);
            }
            (FormKind::CreatePivot, Event::Changed("existing")) => {
                let on = f.on("existing");
                f.set_on("newSheet", !on);
                f.enable("loc", on);
            }
            (FormKind::Symbol, Event::Changed("subset")) => {
                self.symbol = (f.sel("subset").unwrap_or(0) * SYM_ROWS, 0);
                self.symbol_code();
            }
            (FormKind::Symbol, Event::Canvas("grid", x, y)) => {
                let row = ((y / SYM_CELL).floor() as usize).min(SYM_ROWS - 1);
                let col = ((x / SYM_CELL).floor() as usize).min(SYM_COLS - 1);
                let first = (self.symbol.0 / SYM_ROWS) * SYM_ROWS;
                self.symbol = (first + row, col);
                self.symbol_code();
            }
            (FormKind::Symbol, Event::Ok | Event::Clicked("insert")) => {
                let i = self.symbol.0 * SYM_COLS + self.symbol.1;
                if let Some(c) = symbols().get(i) {
                    let s = c.to_string();
                    if self.view.editing() {
                        self.view.edit_insert(&s, text);
                    } else {
                        self.view.edit_with(&s, text);
                    }
                }
                self.form = None;
            }
            (FormKind::CreateTable, Event::Ok) => {
                let range = parse_a1(&f.text("range"));
                let headers = f.on("headers");
                self.form = None;
                let Some(mut range) = range else {
                    return;
                };
                if !headers {
                    // A header row is inserted above the data, as the reference does
                    // ("Column1", …); the engine's table takes the first row as its header.
                    let (r0, c0, c1) = (
                        range["r0"].as_u64().unwrap_or(0) as u32,
                        range["c0"].as_u64().unwrap_or(0) as u32,
                        range["c1"].as_u64().unwrap_or(0) as u32,
                    );
                    self.view.run(json!({
                        "cmd": "shiftCells", "down": true, "insert": true,
                        "range": { "r0": r0, "c0": c0, "r1": r0, "c1": c1 },
                    }));
                    let names: Vec<(u32, u32, String)> = (c0..=c1)
                        .map(|c| (r0, c, format!("Column{}", c - c0 + 1)))
                        .collect();
                    let sheet = self.view.sheet();
                    let _ = self.view.engine_mut().set_inputs(sheet, &names);
                    self.view.changed();
                    range["r1"] = json!(range["r1"].as_u64().unwrap_or(0) + 1);
                }
                self.view
                    .run(json!({ "cmd": "createTable", "range": range }));
            }
            (FormKind::CreatePivot, Event::Ok) => {
                let range = parse_a1(&f.text("range"));
                let dest = if f.on("existing") {
                    parse_a1(&f.text("loc")).map(|r| json!({ "row": r["r0"], "col": r["c0"] }))
                } else {
                    None
                };
                let (rows, values) = (f.sel("rows").unwrap_or(0), f.sel("values").unwrap_or(0));
                self.form = None;
                let Some(range) = range else {
                    return;
                };
                let labels = json!({
                    "grandTotal": self.t("sheetInsert.grandTotal", "Grand Total"),
                    "rowLabels": self.t("sheetInsert.rowLabels", "Row Labels"),
                });
                let mut cmd = json!({
                    "cmd": "pivotCreate", "range": range, "rows": [rows],
                    "values": [{ "field": values, "function": "sum" }], "labels": labels,
                });
                if let Some(d) = dest {
                    cmd["dest"] = d;
                }
                let before = self.view.sheet_names().len();
                if self.view.query(cmd).is_some() {
                    self.view.changed();
                    let after = self.view.sheet_names().len();
                    if after > before {
                        self.view.set_sheet(after - 1);
                        self.layout_tabs(text);
                    }
                }
            }
            (FormKind::Sparklines, Event::Ok) => {
                let data = f.text("data").replace('$', "");
                let loc = parse_a1(&f.text("loc"));
                self.form = None;
                if let Some(location) = loc {
                    let kind = self.spark_kind.clone();
                    self.view.run(json!({
                        "cmd": "sparklineInsert", "type": kind, "data": data, "location": location,
                    }));
                }
            }
            (FormKind::Link, Event::Ok) => {
                let (shown, addr) = (f.text("text"), f.text("addr"));
                self.form = None;
                if addr.trim().is_empty() {
                    return;
                }
                let q = |s: &str| s.replace('"', "\"\"");
                let formula = if shown.trim().is_empty() || shown == addr {
                    format!("=HYPERLINK(\"{}\")", q(&addr))
                } else {
                    format!("=HYPERLINK(\"{}\",\"{}\")", q(&addr), q(&shown))
                };
                let (r, c) = self.view.active();
                let sheet = self.view.sheet();
                if self
                    .view
                    .engine_mut()
                    .set_inputs(sheet, &[(r, c, formula)])
                    .is_ok()
                {
                    self.view.changed();
                }
            }
            (FormKind::HeaderFooter, Event::Ok) => {
                let head = hf_join(&f.text("hl"), &f.text("hc"), &f.text("hr"));
                let foot = hf_join(&f.text("fl"), &f.text("fc"), &f.text("fr"));
                self.form = None;
                self.view.run(json!({
                    "cmd": "setPageSetup",
                    "headerFooter": { "oddHeader": head, "oddFooter": foot },
                }));
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_footer_sections_round_trip() {
        assert_eq!(hf_sections("&LLeft&CMid&RRight"), ["Left", "Mid", "Right"]);
        assert_eq!(hf_sections("Page &P"), ["", "Page &P", ""]);
        assert_eq!(hf_join("", "Page &P", ""), "&CPage &P");
        assert_eq!(hf_join("a", "", "c"), "&La&Rc");
    }

    #[test]
    fn ranges_read_and_write_a1() {
        let r = json!({ "r0": 0, "c0": 0, "r1": 12, "c1": 3 });
        assert_eq!(a1(&r), "$A$1:$D$13");
        assert_eq!(parse_a1("$A$1:$D$13"), Some(r));
        assert_eq!(
            parse_a1("E2"),
            Some(json!({ "r0": 1, "c0": 4, "r1": 1, "c1": 4 }))
        );
        assert_eq!(parse_a1("nonsense!"), None);
        assert!(symbols().len() > SYM_COLS * SYM_ROWS);
    }
}
