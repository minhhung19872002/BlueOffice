//! PivotTable Fields as the reference's task pane (round 56, measured at 125 % on the
//! reference after Insert › PivotTable › OK, `show/sheet-66`): docked right of the grid
//! (436 px), shown while the active cell is in a pivot table, with "Choose fields to add to
//! report:", the source fields as check boxes, and the Filters / Columns / Rows / Values
//! areas. Ticking a field adds it as the reference does (numbers to Values as a Sum, other
//! fields to Rows), clearing it takes it out; a click on a field in an area moves it on
//! (Rows → Columns → Filters → Rows) or, in Values, changes its summary (Sum → Count →
//! Average → Max → Min). Changes go through the engine's `pivotUpdate` (one undo step).
use super::form_impl::{Event, Form, FormKind, Kind};
use super::SheetWindow;
use crate::{Painter, TextStyle};
use bo_render::{PathSeg, Rgba};
use serde_json::{json, Value};

const PANE_BG: Rgba = Rgba(0xE6, 0xE6, 0xE6, 255);
const PANE_EDGE: Rgba = Rgba(0xBF, 0xBF, 0xBF, 255);
const WHITE: Rgba = Rgba(255, 255, 255, 255);
const INK: Rgba = Rgba(0x24, 0x24, 0x24, 255);
const BOX_EDGE: Rgba = Rgba(0xAB, 0xAB, 0xAB, 255);
const DIM: Rgba = Rgba(0xA0, 0xA0, 0xA0, 255);

/// Pane width (reference pixels at 125 %).
pub(super) const PIVOT_W: f32 = 436.0;
/// Field list box and its rows (reference pixels from the pane's top-left corner).
const LIST: [f32; 4] = [15.0, 176.0, 406.0, 212.0];
const ROW_H: f32 = 33.0;
/// The four areas: Filters, Columns, Rows, Values (boxes under their captions).
const AREAS: [[f32; 4]; 4] = [
    [18.0, 492.0, 194.0, 78.0],
    [222.0, 492.0, 200.0, 78.0],
    [18.0, 617.0, 194.0, 78.0],
    [222.0, 617.0, 200.0, 78.0],
];
const FUNCTIONS: [&str; 5] = ["sum", "count", "average", "max", "min"];

/// The pivot table the pane edits and its layout.
#[derive(Debug, Clone, Default, PartialEq)]
pub(super) struct PivotPane {
    pub sheet: usize,
    pub index: usize,
    /// Source fields: name, holds numbers.
    pub fields: Vec<(String, bool)>,
    pub rows: Vec<u32>,
    pub cols: Vec<u32>,
    pub filters: Vec<u32>,
    /// Value fields: field, function.
    pub values: Vec<(u32, String)>,
}

impl PivotPane {
    fn used(&self, f: u32) -> bool {
        self.rows.contains(&f)
            || self.cols.contains(&f)
            || self.filters.contains(&f)
            || self.values.iter().any(|v| v.0 == f)
    }
}

impl SheetWindow {
    /// The pivot table under the active cell: (sheet, index).
    fn pivot_at_active(&self) -> Option<(usize, usize)> {
        let sheet = self.view.sheet();
        let (r, c) = self.view.active();
        let wb = self.view.engine().workbook()?;
        let ws = wb.sheets.get(sheet)?;
        ws.pivots
            .iter()
            .position(|p| {
                let l = p.location;
                (l.first.row..=l.last.row).contains(&r) && (l.first.col..=l.last.col).contains(&c)
            })
            .map(|i| (sheet, i))
    }

    /// Show the pane while the active cell is in a pivot table (as the reference), read its
    /// layout again after changes.
    pub(super) fn sync_pivot_pane(&mut self) {
        let at = self.pivot_at_active();
        // Closed by its × : stays closed until the active cell leaves that pivot table.
        if at.is_some() && at == self.pivot_closed {
            return;
        }
        self.pivot_closed = None;
        let was = self.pivot.as_ref().map(|p| (p.sheet, p.index));
        let Some((sheet, index)) = at else {
            if self.pivot.take().is_some() {
                self.pane_changed = true;
            }
            return;
        };
        let info = self
            .view
            .query(json!({ "cmd": "pivots", "sheet": sheet }))
            .and_then(|v| v.as_array().and_then(|a| a.get(index)).cloned())
            .unwrap_or(Value::Null);
        let nums = |k: &str| -> Vec<u32> {
            info.get(k)
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_u64().and_then(|n| u32::try_from(n).ok()))
                        .collect()
                })
                .unwrap_or_default()
        };
        let fields: Vec<(String, bool)> = info
            .get("fields")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .map(|f| {
                        let name = f
                            .get("name")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .to_owned();
                        let items = f.get("items").and_then(Value::as_array);
                        let numeric = items.is_some_and(|it| {
                            !it.is_empty()
                                && it.iter().all(|v| {
                                    v.as_str()
                                        .is_some_and(|s| s.replace(',', ".").parse::<f64>().is_ok())
                                })
                        });
                        (name, numeric)
                    })
                    .collect()
            })
            .unwrap_or_default();
        let values = info
            .get("values")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|v| {
                        Some((
                            u32::try_from(v.get("field")?.as_u64()?).ok()?,
                            v.get("function")
                                .and_then(Value::as_str)
                                .unwrap_or("sum")
                                .to_owned(),
                        ))
                    })
                    .collect()
            })
            .unwrap_or_default();
        let filters = info
            .get("filters")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|f| u32::try_from(f.get("field")?.as_u64()?).ok())
                    .collect()
            })
            .unwrap_or_default();
        let pane = PivotPane {
            sheet,
            index,
            fields,
            rows: nums("rows"),
            cols: nums("cols"),
            filters,
            values,
        };
        if was.is_none() {
            self.pane_changed = true;
        }
        self.pivot = Some(pane);
    }

    /// Apply the pane's layout to the pivot table.
    fn pivot_apply(&mut self, p: &PivotPane) {
        let labels = json!({
            "grandTotal": self.t("sheetInsert.grandTotal", "Grand Total"),
            "rowLabels": self.t("sheetInsert.rowLabels", "Row Labels"),
        });
        let values: Vec<Value> = p
            .values
            .iter()
            .map(|(f, func)| json!({ "field": f, "function": func }))
            .collect();
        let filters: Vec<Value> = p.filters.iter().map(|f| json!({ "field": f })).collect();
        self.view.run(json!({
            "cmd": "pivotUpdate", "sheet": p.sheet, "index": p.index,
            "rows": p.rows, "cols": p.cols, "values": values, "filters": filters,
            "labels": labels,
        }));
        self.sync_pivot_pane();
    }

    /// Tick or clear field `f`.
    pub(super) fn pivot_toggle(&mut self, f: u32) {
        let Some(mut p) = self.pivot.clone() else {
            return;
        };
        if p.used(f) {
            p.rows.retain(|x| *x != f);
            p.cols.retain(|x| *x != f);
            p.filters.retain(|x| *x != f);
            p.values.retain(|x| x.0 != f);
        } else if p.fields.get(f as usize).is_some_and(|x| x.1) {
            p.values.push((f, "sum".to_owned()));
        } else {
            p.rows.push(f);
        }
        self.pivot_apply(&p);
    }

    /// A click on field `f` in area `area` (0 Filters, 1 Columns, 2 Rows, 3 Values).
    fn pivot_area_click(&mut self, area: usize, f: u32) {
        let Some(mut p) = self.pivot.clone() else {
            return;
        };
        match area {
            3 => {
                if let Some(v) = p.values.iter_mut().find(|v| v.0 == f) {
                    let i = FUNCTIONS.iter().position(|x| *x == v.1).unwrap_or(0);
                    v.1 = FUNCTIONS[(i + 1) % FUNCTIONS.len()].to_owned();
                }
            }
            2 => {
                p.rows.retain(|x| *x != f);
                p.cols.push(f);
            }
            1 => {
                p.cols.retain(|x| *x != f);
                p.filters.push(f);
            }
            _ => {
                p.filters.retain(|x| *x != f);
                p.rows.push(f);
            }
        }
        self.pivot_apply(&p);
    }

    /// A press at (lx, ly) reference pixels in the pane.
    pub(super) fn pivot_press(&mut self, lx: f32, ly: f32) -> bool {
        if (PIVOT_W - 40.0..PIVOT_W - 4.0).contains(&lx) && (10.0..46.0).contains(&ly) {
            self.pivot = None;
            self.pivot_closed = self.pivot_at_active();
            self.pane_changed = true;
            return true;
        }
        let Some(p) = self.pivot.clone() else {
            return false;
        };
        if (LIST[0]..LIST[0] + LIST[2]).contains(&lx) && (LIST[1]..LIST[1] + LIST[3]).contains(&ly)
        {
            let i = ((ly - LIST[1] - 4.0) / ROW_H).floor().max(0.0) as usize;
            if i < p.fields.len() {
                self.pivot_toggle(i as u32);
            }
            return true;
        }
        for (a, r) in AREAS.iter().enumerate() {
            if (r[0]..r[0] + r[2]).contains(&lx) && (r[1]..r[1] + r[3]).contains(&ly) {
                let list = Self::area_fields(&p, a);
                let i = ((ly - r[1] - 3.0) / 26.0).floor().max(0.0) as usize;
                if let Some(f) = list.get(i) {
                    self.pivot_area_click(a, *f);
                }
                return true;
            }
        }
        false
    }

    fn area_fields(p: &PivotPane, area: usize) -> Vec<u32> {
        match area {
            0 => p.filters.clone(),
            1 => p.cols.clone(),
            2 => p.rows.clone(),
            _ => p.values.iter().map(|v| v.0).collect(),
        }
    }

    /// The pane (drawn in the pane rectangle).
    pub(super) fn paint_pivot_pane(&self, p: &mut dyn Painter) {
        let Some(pane) = &self.pivot else {
            return;
        };
        if self.acc.is_some() {
            return;
        }
        let r = self.pane_rect();
        let k = self.scale / 1.25;
        let at = |x: f32, y: f32| ((r[0] + x * k).round(), (r[1] + y * k).round());
        let style = |px: f32, bold: bool| TextStyle {
            size: px * self.scale / 1.25 / 1.1,
            bold,
        };
        p.fill_rect(r[0], r[1], r[2], r[3], PANE_BG);
        p.fill_rect(r[0], r[1], 1.0, r[3], PANE_EDGE);
        p.push_clip(r[0], r[1], r[2], r[3]);
        let (tx, ty) = at(16.0, 12.0);
        p.text(
            &self.t("sheetPivot.fields", "PivotTable Fields"),
            tx,
            ty,
            style(30.0, false),
            INK,
        );
        // ˅ and × at the top right.
        let s = 6.0 * k;
        let (vx, vy) = at(PIVOT_W - 86.0, 24.0);
        p.path(
            &[
                PathSeg::M(vx, vy),
                PathSeg::L(vx + s, vy + s),
                PathSeg::L(vx + 2.0 * s, vy),
            ],
            None,
            Some((INK, k.max(1.0))),
        );
        let (xx, xy) = at(PIVOT_W - 34.0, 22.0);
        let s = 11.0 * k;
        p.path(
            &[
                PathSeg::M(xx, xy),
                PathSeg::L(xx + s, xy + s),
                PathSeg::M(xx + s, xy),
                PathSeg::L(xx, xy + s),
            ],
            None,
            Some((INK, k.max(1.0))),
        );
        let ui = style(20.0, false);
        let (cx, cy) = at(15.0, 70.0);
        p.text(
            &self.t("sheetPivot.choose", "Choose fields to add to report:"),
            cx,
            cy,
            ui,
            INK,
        );
        // Search box (own magnifier mark).
        let (sx, sy) = at(15.0, 118.0);
        let (sw, sh) = ((406.0 * k).round(), (36.0 * k).round());
        p.fill_rect(sx, sy, sw, sh, BOX_EDGE);
        p.fill_rect(sx + 1.0, sy + 1.0, sw - 2.0, sh - 2.0, WHITE);
        p.text(
            &self.t("sheetPivot.search", "Search"),
            sx + 7.0 * k,
            sy + 6.0 * k,
            ui,
            Rgba(0x60, 0x60, 0x60, 255),
        );
        let (mx, my) = (sx + sw - 22.0 * k, sy + 14.0 * k);
        let rr = 7.0 * k;
        let kk = rr * 0.552_284_8;
        p.path(
            &[
                PathSeg::M(mx + rr, my),
                PathSeg::C(mx + rr, my + kk, mx + kk, my + rr, mx, my + rr),
                PathSeg::C(mx - kk, my + rr, mx - rr, my + kk, mx - rr, my),
                PathSeg::C(mx - rr, my - kk, mx - kk, my - rr, mx, my - rr),
                PathSeg::C(mx + kk, my - rr, mx + rr, my - kk, mx + rr, my),
                PathSeg::Z,
                PathSeg::M(mx - rr * 0.7, my + rr * 0.7),
                PathSeg::L(mx - rr * 1.8, my + rr * 1.8),
            ],
            None,
            Some((INK, k.max(1.0))),
        );
        // Field list.
        let (lx, ly) = at(LIST[0], LIST[1]);
        let (lw, lh) = ((LIST[2] * k).round(), (LIST[3] * k).round());
        p.fill_rect(lx, ly, lw, lh, BOX_EDGE);
        p.fill_rect(lx + 1.0, ly + 1.0, lw - 2.0, lh - 2.0, WHITE);
        for (i, (name, _)) in pane.fields.iter().enumerate() {
            let y = LIST[1] + 4.0 + i as f32 * ROW_H;
            let (bx, by) = at(LIST[0] + 4.0, y + 6.0);
            let b = (16.0 * k).round();
            let on = pane.used(i as u32);
            p.fill_rect(bx, by, b, b, Rgba(0x33, 0x33, 0x33, 255));
            p.fill_rect(bx + 1.0, by + 1.0, b - 2.0, b - 2.0, WHITE);
            if on {
                p.path(
                    &[
                        PathSeg::M(bx + 3.0 * k, by + 8.0 * k),
                        PathSeg::L(bx + 6.5 * k, by + 11.5 * k),
                        PathSeg::L(bx + 13.0 * k, by + 4.5 * k),
                    ],
                    None,
                    Some((INK, (1.6 * k).max(1.0))),
                );
            }
            let (nx, ny) = at(LIST[0] + 28.0, y + 3.0);
            p.text(name, nx, ny, style(20.0, on), INK);
        }
        let (dx, dy) = at(19.0, 405.0);
        p.text(
            &self.t("sheetPivot.drag", "Drag fields between areas below:"),
            dx,
            dy,
            ui,
            INK,
        );
        let captions = [
            ("sheetPivot.filters", "Filters"),
            ("sheetPivot.columns", "Columns"),
            ("sheetPivot.rows", "Rows"),
            ("sheetPivot.values", "Values"),
        ];
        for (a, (key, d)) in captions.iter().enumerate() {
            let b = AREAS[a];
            let (hx, hy) = at(b[0] + 30.0, b[1] - 36.0);
            p.text(&self.t(key, d), hx, hy, ui, INK);
            Self::paint_area_mark(p, a, at(b[0] + 6.0, b[1] - 27.0), k);
            let (ax, ay) = at(b[0], b[1]);
            p.fill_rect(ax, ay, (b[2] * k).round(), (b[3] * k).round(), WHITE);
            for (n, f) in Self::area_fields(pane, a).iter().enumerate() {
                let name = pane.fields.get(*f as usize).map_or("", |x| x.0.as_str());
                let label = if a == 3 {
                    let func = pane
                        .values
                        .iter()
                        .find(|v| v.0 == *f)
                        .map_or("sum", |v| v.1.as_str());
                    let fname = self.t(&format!("sheetPivot.fn_{func}"), func);
                    format!("{fname} {} {name}", self.t("sheetPivot.of", "of"))
                } else {
                    name.to_owned()
                };
                let (ix, iy) = at(b[0] + 6.0, b[1] + 3.0 + n as f32 * 26.0);
                p.fill_rect(
                    ix,
                    iy,
                    ((b[2] - 12.0) * k).round(),
                    (24.0 * k).round(),
                    Rgba(0xF3, 0xF3, 0xF3, 255),
                );
                p.text(&label, ix + 6.0 * k, iy + 2.0 * k, style(18.0, false), INK);
            }
        }
        // Defer Layout Update (not offered: shown dimmed) and Update.
        let (ux, uy) = at(22.0, 735.0);
        let b = (16.0 * k).round();
        p.fill_rect(ux, uy, b, b, DIM);
        p.fill_rect(ux + 1.0, uy + 1.0, b - 2.0, b - 2.0, WHITE);
        let (vx, vy) = at(48.0, 730.0);
        p.text(
            &self.t("sheetPivot.defer", "Defer Layout Update"),
            vx,
            vy,
            ui,
            DIM,
        );
        p.pop_clip();
    }

    /// Own marks before the area captions: a funnel, columns, rows, a sigma.
    fn paint_area_mark(p: &mut dyn Painter, area: usize, (x, y): (f32, f32), k: f32) {
        let ink = Rgba(0x6E, 0x6E, 0x6E, 255);
        let w = k.max(1.0);
        match area {
            0 => p.path(
                &[
                    PathSeg::M(x, y),
                    PathSeg::L(x + 14.0 * k, y),
                    PathSeg::L(x + 9.0 * k, y + 7.0 * k),
                    PathSeg::L(x + 9.0 * k, y + 14.0 * k),
                    PathSeg::L(x + 5.0 * k, y + 12.0 * k),
                    PathSeg::L(x + 5.0 * k, y + 7.0 * k),
                    PathSeg::Z,
                ],
                Some(ink),
                None,
            ),
            1 => {
                for i in 0..3 {
                    p.fill_rect(x + (i as f32 * 5.0) * k, y, w * 2.0, 14.0 * k, ink);
                }
            }
            2 => {
                for i in 0..3 {
                    p.fill_rect(x, y + (i as f32 * 5.0) * k, 14.0 * k, w * 2.0, ink);
                }
            }
            _ => p.path(
                &[
                    PathSeg::M(x + 12.0 * k, y),
                    PathSeg::L(x, y),
                    PathSeg::L(x + 7.0 * k, y + 7.0 * k),
                    PathSeg::L(x, y + 14.0 * k),
                    PathSeg::L(x + 12.0 * k, y + 14.0 * k),
                ],
                None,
                Some((ink, w * 1.5)),
            ),
        }
    }

    /// Insert › PivotTable as the reference's "PivotTable from table or range" (513 × 416
    /// px, `show/sheet-65`): the table / range with a picker, New / Existing Worksheet with
    /// a location picker, the Data Model box (not offered: dimmed). OK makes an empty
    /// pivot table and the PivotTable Fields pane shows (as the reference).
    pub(super) fn open_new_pivot(&mut self) -> bool {
        let region = self.region();
        let sheet_name = self
            .view
            .sheet_names()
            .get(self.view.sheet())
            .cloned()
            .flatten()
            .unwrap_or_default();
        let quoted = if sheet_name.chars().all(|c| c.is_alphanumeric() || c == '_') {
            sheet_name
        } else {
            format!("'{}'", sheet_name.replace('\'', "''"))
        };
        let table = format!("{quoted}!{}", super::insert_impl::a1(&region));
        let t = |k: &str, d: &str| self.t(k, d);
        let mut f = Form::new(
            &t("sheetPivot.newTitle", "PivotTable from table or range"),
            (513.0, 416.0),
            vec![],
        );
        f.set_font_scale(1.15);
        let edit = |s: &str| Kind::Edit {
            value: s.chars().collect(),
            caret: s.chars().count(),
        };
        f.add(
            "h1",
            None,
            [17.0, 48.0, 0.0, 0.0],
            Kind::Label(t("sheetPivot.select", "Select a table or range")),
        );
        f.add(
            "tableL",
            None,
            [26.0, 88.0, 0.0, 0.0],
            Kind::Label(t("sheetInsert.tableRange", "Table/Range:")),
        );
        f.add("table", None, [142.0, 84.0, 318.0, 27.0], edit(&table));
        f.add("pickTable", None, [460.0, 84.0, 27.0, 27.0], Kind::Canvas);
        f.add(
            "h2",
            None,
            [17.0, 128.0, 0.0, 0.0],
            Kind::Label(t(
                "sheetPivot.where",
                "Choose where you want the PivotTable to be placed",
            )),
        );
        f.add(
            "newSheet",
            None,
            [26.0, 166.0, 250.0, 24.0],
            Kind::Check {
                label: t("sheetInsert.newWorksheet", "New Worksheet"),
                on: true,
            },
        );
        f.add(
            "existing",
            None,
            [26.0, 202.0, 250.0, 24.0],
            Kind::Check {
                label: t("sheetInsert.existingWorksheet", "Existing Worksheet"),
                on: false,
            },
        );
        f.set_radios(&["newSheet", "existing"]);
        f.add(
            "locL",
            None,
            [28.0, 245.0, 0.0, 0.0],
            Kind::Label(t("sheetInsert.location", "Location:")),
        );
        f.add("loc", None, [112.0, 239.0, 348.0, 31.0], edit(""));
        f.add("pickLoc", None, [460.0, 239.0, 27.0, 31.0], Kind::Canvas);
        f.enable("locL", false);
        f.enable("loc", false);
        f.add(
            "h3",
            None,
            [17.0, 285.0, 0.0, 0.0],
            Kind::Label(t(
                "sheetPivot.multiple",
                "Choose whether you want to analyze multiple tables",
            )),
        );
        f.add(
            "model",
            None,
            [26.0, 318.0, 320.0, 24.0],
            Kind::Check {
                label: t("sheetPivot.dataModel", "Add this data to the Data Model"),
                on: false,
            },
        );
        // No data model in BlueSheet: dimmed, as the reference shows unavailable options.
        f.enable("model", false);
        for id in ["h1", "h2", "h3"] {
            f.set_bold(id);
        }
        f.set_access("tableL", 0);
        f.set_access("newSheet", 0);
        f.set_access("existing", 0);
        f.set_access("locL", 0);
        f.ok_cancel(&t("common.ok", "OK"), &t("common.cancel", "Cancel"));
        if let Some(c) = f.get_mut("ok") {
            c.rect = [341.0, 362.0, 71.0, 35.0];
        }
        if let Some(c) = f.get_mut("cancel") {
            c.rect = [425.0, 362.0, 71.0, 35.0];
        }
        f.focus("table");
        self.form = Some(f);
        self.form_kind = FormKind::PivotNew;
        true
    }

    /// A reference typed in a box: (sheet, range JSON).
    fn pivot_resolve(&mut self, text: &str) -> Option<(usize, Value)> {
        let sheet = self.view.sheet();
        let out = self
            .view
            .query(json!({ "cmd": "resolve", "sheet": sheet, "text": text.trim() }))?;
        let s = out
            .get("sheet")
            .and_then(Value::as_u64)
            .map_or(sheet, |s| s as usize);
        Some((s, out.get("range")?.clone()))
    }

    /// Events of "PivotTable from table or range".
    pub(super) fn new_pivot_event(&mut self, ev: &Event, text: &dyn crate::TextMeasure) {
        match ev {
            Event::Cancel => self.form = None,
            Event::Changed(id @ ("newSheet" | "existing")) => {
                let existing = *id == "existing";
                if let Some(f) = self.form.as_mut() {
                    f.set_on("newSheet", !existing);
                    f.set_on("existing", existing);
                    f.enable("loc", existing);
                    f.enable("locL", existing);
                    if existing {
                        f.focus("loc");
                    }
                }
            }
            Event::Ok => {
                let Some(f) = self.form.as_ref() else {
                    return;
                };
                let (table, existing, loc) = (f.text("table"), f.on("existing"), f.text("loc"));
                let Some((src_sheet, range)) = self.pivot_resolve(&table) else {
                    self.status = self.t(
                        "sheetPivot.badRange",
                        "The PivotTable source reference is not valid.",
                    );
                    return;
                };
                let dest = if existing {
                    let Some((ds, r)) = self.pivot_resolve(&loc) else {
                        self.status = self.t(
                            "sheetPivot.badDest",
                            "The PivotTable destination reference is not valid.",
                        );
                        return;
                    };
                    Some((ds, r))
                } else {
                    None
                };
                self.form = None;
                let labels = json!({
                    "grandTotal": self.t("sheetInsert.grandTotal", "Grand Total"),
                    "rowLabels": self.t("sheetInsert.rowLabels", "Row Labels"),
                });
                let mut cmd = json!({
                    "cmd": "pivotCreate", "sheet": src_sheet, "range": range, "labels": labels,
                });
                if let Some((ds, r)) = &dest {
                    cmd["dest"] = json!({ "sheet": ds, "row": r["r0"], "col": r["c0"] });
                }
                let before = self.view.sheet_names().len();
                let Some(made) = self.view.query(cmd) else {
                    return;
                };
                self.view.changed();
                // Go to the report so that its fields pane shows.
                let sheet = made
                    .get("sheet")
                    .and_then(Value::as_u64)
                    .map_or_else(|| self.view.sheet(), |s| s as usize);
                if sheet != self.view.sheet() {
                    self.view.set_sheet(sheet);
                }
                if self.view.sheet_names().len() != before {
                    self.layout_tabs(text);
                }
                let first = self
                    .view
                    .engine()
                    .workbook()
                    .and_then(|wb| wb.sheets.get(sheet))
                    .and_then(|ws| ws.pivots.last())
                    .map(|p| (p.location.first.row, p.location.first.col));
                if let Some((r, c)) = first {
                    self.view.select((r, c), (r, c), (r, c));
                }
            }
            _ => {}
        }
    }
}
