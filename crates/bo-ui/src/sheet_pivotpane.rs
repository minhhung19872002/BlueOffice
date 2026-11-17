//! PivotTable Fields as the reference's task pane (round 56, measured at 125 % on the
//! reference after Insert › PivotTable › OK, `show/sheet-66`): docked right of the grid
//! (436 px), shown while the active cell is in a pivot table, with "Choose fields to add to
//! report:", the source fields as check boxes, and the Filters / Columns / Rows / Values
//! areas. Ticking a field adds it as the reference does (numbers to Values as a Sum, other
//! fields to Rows), clearing it takes it out. Round 58: a click on a field in an area opens
//! its menu (Move Up / Down / to Beginning / to End, Move to an area, Remove Field, Value
//! Field Settings…), a drag moves it between areas (dropped on the field list or outside
//! the pane it is taken out); Value Field Settings sets the summary, Show Values As and the
//! name. Changes go through the engine's `pivotUpdate` (one undo step, column widths fitted).
use super::form_impl::{Event, Form, FormKind, Kind};
use super::SheetWindow;
use crate::popup::{Choice, Menu, MenuItem, MenuLook};
use crate::{Painter, TextStyle};
use bo_render::{PathSeg, Rgba};
use serde_json::{json, Value};

const PANE_BG: Rgba = Rgba(0xE6, 0xE6, 0xE6, 255);
const PANE_EDGE: Rgba = Rgba(0xBF, 0xBF, 0xBF, 255);
const WHITE: Rgba = Rgba(255, 255, 255, 255);
const INK: Rgba = Rgba(0x24, 0x24, 0x24, 255);
const BOX_EDGE: Rgba = Rgba(0xAB, 0xAB, 0xAB, 255);
const DIM: Rgba = Rgba(0xA0, 0xA0, 0xA0, 255);

/// The tools (gear) button: left, top, width, height (reference pixels; measured 62 × 37 px,
/// centred on the "Choose fields" line, 25 px from the pane's right edge).
const GEAR: [f32; 4] = [349.0, 65.0, 62.0, 37.0];

/// Field items in an area: inset, top offset, height and pitch (reference pixels).
const ITEM_X: f32 = 1.0;
const ITEM_Y: f32 = 2.0;
const ITEM_H: f32 = 31.0;
const ITEM_PITCH: f32 = 32.0;

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
    /// Items of each field as the report lists them (Show Values As base items).
    pub items: Vec<Vec<String>>,
    /// Value field settings beyond the function, by field.
    pub opts: std::collections::BTreeMap<u32, ValueOpts>,
    /// The value field whose Value Field Settings dialog is open.
    pub vfs: Option<u32>,
    /// Field list sorted A to Z (gear menu) instead of in data source order.
    pub sort_az: bool,
}

impl PivotPane {
    /// Field indexes in the order the list shows them.
    pub(super) fn order(&self) -> Vec<usize> {
        let mut v: Vec<usize> = (0..self.fields.len()).collect();
        if self.sort_az {
            v.sort_by_key(|&i| self.fields[i].0.to_lowercase());
        }
        v
    }
}

/// "Show values as", base field / item and custom name of a value field.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct ValueOpts {
    pub show_as: String,
    pub base_field: u32,
    pub base_item: u32,
    /// Custom name ("" = "<Function> of <field>").
    pub name: String,
    /// Number format code of the values ("" = the source's; §18.10.1.21 `numFmtId`).
    pub num_fmt: String,
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
        let items: Vec<Vec<String>> = info
            .get("fields")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .map(|f| {
                        f.get("items")
                            .and_then(Value::as_array)
                            .map(|it| {
                                it.iter()
                                    .map(|v| {
                                        v.as_str().map_or_else(|| v.to_string(), str::to_owned)
                                    })
                                    .collect()
                            })
                            .unwrap_or_default()
                    })
                    .collect()
            })
            .unwrap_or_default();
        let mut opts = std::collections::BTreeMap::new();
        for v in info
            .get("values")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let Some(f) = v.get("field").and_then(Value::as_u64) else {
                continue;
            };
            let f = u32::try_from(f).unwrap_or(0);
            let num = |k: &str| {
                v.get(k)
                    .and_then(Value::as_u64)
                    .and_then(|n| u32::try_from(n).ok())
                    .unwrap_or(0)
            };
            let field_name = fields.get(f as usize).map_or("", |x| x.0.as_str());
            let name = v.get("name").and_then(Value::as_str).unwrap_or("");
            // Only a name other than "<Function> of <field>" is the user's own.
            let custom = !name.ends_with(&format!(" of {field_name}"));
            opts.insert(
                f,
                ValueOpts {
                    show_as: v
                        .get("showAs")
                        .and_then(Value::as_str)
                        .unwrap_or("normal")
                        .to_owned(),
                    base_field: num("baseField"),
                    base_item: num("baseItem"),
                    name: if custom {
                        name.to_owned()
                    } else {
                        String::new()
                    },
                    num_fmt: v
                        .get("numFmt")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_owned(),
                },
            );
        }
        // A value field whose settings dialog is open keeps what was chosen there (its
        // number format is applied on OK).
        let vfs = self.pivot.as_ref().and_then(|p| p.vfs);
        if let Some(o) = vfs.and_then(|f| Some((f, self.pivot.as_ref()?.opts.get(&f)?.clone()))) {
            opts.insert(o.0, o.1);
        }
        let pane = PivotPane {
            sheet,
            index,
            fields,
            rows: nums("rows"),
            cols: nums("cols"),
            filters,
            values,
            items,
            opts,
            vfs,
            sort_az: self.pivot.as_ref().is_some_and(|p| p.sort_az),
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
            .map(|(f, func)| {
                let mut v = json!({ "field": f, "function": func });
                if let Some(o) = p.opts.get(f) {
                    if !o.show_as.is_empty() {
                        v["showAs"] = json!(o.show_as);
                        v["baseField"] = json!(o.base_field);
                        v["baseItem"] = json!(o.base_item);
                    }
                    if !o.name.is_empty() {
                        v["name"] = json!(o.name);
                    }
                    if !o.num_fmt.is_empty() {
                        v["numFmt"] = json!(o.num_fmt);
                    }
                }
                v
            })
            .collect();
        let filters: Vec<Value> = p.filters.iter().map(|f| json!({ "field": f })).collect();
        let depth = self.view.engine().history_len();
        self.view.run(json!({
            "cmd": "pivotUpdate", "sheet": p.sheet, "index": p.index,
            "rows": p.rows, "cols": p.cols, "values": values, "filters": filters,
            "labels": labels,
        }));
        self.pivot_autofit(p.sheet, p.index, depth);
        self.sync_pivot_pane();
    }

    /// Fits the columns of pivot table `index` of `sheet` to its cells ("Autofit column
    /// widths on update", on by default in the reference: "Grand Total" is not cut), as
    /// part of the undo step started at history depth `depth`.
    pub(super) fn pivot_autofit(&mut self, sheet: usize, index: usize, depth: usize) {
        let cols = self
            .view
            .engine()
            .workbook()
            .and_then(|wb| wb.sheets.get(sheet))
            .and_then(|ws| ws.pivots.get(index))
            .map(|p| (p.location.first.col, p.location.last.col));
        let Some((c0, c1)) = cols else {
            return;
        };
        if c1 < c0 || c1 - c0 > 256 {
            return;
        }
        self.view.run(json!({
            "cmd": "autofit", "sheet": sheet, "rows": false, "first": c0, "last": c1,
        }));
        // The "Row Labels" header's field button needs room beside its text (12 pt).
        let compact = self
            .view
            .engine()
            .workbook()
            .and_then(|wb| wb.sheets.get(sheet)?.pivots.get(index))
            .is_some_and(|p| {
                p.layout == bo_model::sheet::PivotLayout::Compact && !p.rows.is_empty()
            });
        if compact {
            let vp = self.view.viewport();
            let w = self
                .view
                .engine_mut()
                .view_layout(sheet, &vp)
                .map_or(0.0, |(_, g)| g.col_width(c0));
            if w > 0.0 {
                self.view.run(json!({
                    "cmd": "resize", "sheet": sheet, "rows": false, "first": c0, "last": c0,
                    "size": w + 12.0,
                }));
            }
        }
        self.view.engine_mut().merge_history_since(depth);
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
            if let Some(&f) = p.order().get(i) {
                self.pivot_toggle(f as u32);
            }
            return true;
        }
        if (GEAR[0]..GEAR[0] + GEAR[2]).contains(&lx) && (GEAR[1]..GEAR[1] + GEAR[3]).contains(&ly)
        {
            self.pivot_gear_menu();
            return true;
        }
        for (a, r) in AREAS.iter().enumerate() {
            if (r[0]..r[0] + r[2]).contains(&lx) && (r[1]..r[1] + r[3]).contains(&ly) {
                let list = Self::area_fields(&p, a);
                let i = ((ly - r[1] - ITEM_Y) / ITEM_PITCH).floor().max(0.0) as usize;
                if let Some(f) = list.get(i) {
                    self.pivot_drag = Some((a, *f, lx, ly));
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
        // Tools (gear) button with its drop-down arrow, right of "Choose fields…".
        Self::paint_gear(p, at(GEAR[0], GEAR[1]), k);
        // Field list.
        let (lx, ly) = at(LIST[0], LIST[1]);
        let (lw, lh) = ((LIST[2] * k).round(), (LIST[3] * k).round());
        p.fill_rect(lx, ly, lw, lh, BOX_EDGE);
        p.fill_rect(lx + 1.0, ly + 1.0, lw - 2.0, lh - 2.0, WHITE);
        for (row, i) in pane.order().into_iter().enumerate() {
            let name = &pane.fields[i].0;
            let y = LIST[1] + 4.0 + row as f32 * ROW_H;
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
                let custom = pane
                    .opts
                    .get(f)
                    .filter(|_| a == 3)
                    .map(|o| o.name.clone())
                    .filter(|n| !n.is_empty());
                let label = if let Some(n) = custom {
                    n
                } else if a == 3 {
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
                // A white box framed #BFBFBF, 1 px in from the area's sides and 2 px below
                // its top, 31 px high every 32 px (measured on the reference, round 58).
                let (ix, iy) = at(b[0] + ITEM_X, b[1] + ITEM_Y + n as f32 * ITEM_PITCH);
                let (iw, ih) = (((b[2] - 2.0 * ITEM_X) * k).round(), (ITEM_H * k).round());
                p.fill_rect(ix, iy, iw, ih, PANE_EDGE);
                p.fill_rect(ix + 1.0, iy + 1.0, iw - 2.0, ih - 2.0, WHITE);
                p.text(&label, ix + 9.0 * k, iy + 4.0 * k, style(18.0, false), INK);
                // Solid drop-down triangle 11 × 6 px, its centre 18 px from the right edge.
                let cx = ix + iw - (18.0 * k).round();
                let cy = iy + (ih / 2.0).round();
                let (hw, h) = (5.5 * k, 6.0 * k);
                p.path(
                    &[
                        PathSeg::M(cx - hw, cy - h / 2.0),
                        PathSeg::L(cx + hw, cy - h / 2.0),
                        PathSeg::L(cx, cy + h / 2.0),
                        PathSeg::Z,
                    ],
                    Some(INK),
                    None,
                );
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

    /// An empty pivot table (no field chosen yet) shows a placeholder over its cells, as the
    /// reference: a framed box three columns by eighteen rows with the report's name and
    /// "To build a report, choose fields from the PivotTable Field List".
    pub(super) fn paint_pivot_placeholder(&self, p: &mut dyn Painter) {
        let Some(pane) = &self.pivot else {
            return;
        };
        if pane.sheet != self.view.sheet()
            || !(pane.rows.is_empty()
                && pane.cols.is_empty()
                && pane.values.is_empty()
                && pane.filters.is_empty())
        {
            return;
        }
        let Some((r, c, name)) = self
            .view
            .engine()
            .workbook()
            .and_then(|wb| wb.sheets.get(pane.sheet))
            .and_then(|ws| ws.pivots.get(pane.index))
            .map(|pt| {
                (
                    pt.location.first.row,
                    pt.location.first.col,
                    pt.name.clone(),
                )
            })
        else {
            return;
        };
        let (Some(a), Some(b)) = (
            self.view.cell_rect(r, c),
            self.view.cell_rect(r + 17, c + 2),
        ) else {
            return;
        };
        let k = self.scale / 1.25;
        let [fx, fy, fw, fh] = self.view.grid_frame();
        p.push_clip(fx, fy, fw, fh);
        let (x0, y0, x1, y1) = (a[0], a[1], b[0] + b[2], b[1] + b[3]);
        p.fill_rect(x0, y0, x1 - x0, y1 - y0, Rgba(0xA6, 0xA6, 0xA6, 255));
        p.fill_rect(x0 + 1.0, y0 + 1.0, x1 - x0 - 2.0, y1 - y0 - 2.0, WHITE);
        let ts = TextStyle {
            size: 15.0 * k / 1.1,
            bold: false,
        };
        let center = |s: &str| x0 + ((x1 - x0) - s.chars().count() as f32 * ts.size * 0.5) / 2.0;
        // The name in a small framed box.
        let (bw, bh) = (180.0 * k, 26.0 * k);
        let (bx, by) = (x0 + ((x1 - x0) - bw) / 2.0, y0 + 30.0 * k);
        p.fill_rect(bx, by, bw, bh, Rgba(0x80, 0x80, 0x80, 255));
        p.fill_rect(bx + 1.0, by + 1.0, bw - 2.0, bh - 2.0, WHITE);
        p.text(
            &name,
            center(&name),
            by + 3.0 * k,
            ts,
            Rgba(0x80, 0x80, 0x80, 255),
        );
        let msg = self.t(
            "sheetPivot.placeholder",
            "To build a report, choose fields from the PivotTable Field List",
        );
        let words: Vec<&str> = msg.split_whitespace().collect();
        let half = words.len().div_ceil(2);
        for (i, line) in [words[..half].join(" "), words[half..].join(" ")]
            .iter()
            .enumerate()
        {
            p.text(
                line,
                center(line),
                by + bh + (12.0 + 24.0 * i as f32) * k,
                ts,
                INK,
            );
        }
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
        f.set_font_scale(1.3);
        // The newer dialogs' white face (as Insert Hyperlink, round 49).
        f.add("face", None, [0.0, 38.0, 513.0, 378.0], Kind::Canvas);
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
        f.select_all("table");
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
                let depth = self.view.engine().history_len();
                let Some(made) = self.view.query(cmd) else {
                    return;
                };
                // Go to the report so that its fields pane shows.
                let mut sheet = made
                    .get("sheet")
                    .and_then(Value::as_u64)
                    .map_or_else(|| self.view.sheet(), |s| s as usize);
                // A new worksheet goes in front of the source sheet, as the reference puts
                // it (round 58); creating and moving it are one undo step.
                // Compact layout, as the reference's new PivotTables ("Row Labels" with its
                // field button in one indented column).
                let index = made.get("index").and_then(Value::as_u64).unwrap_or(0);
                let labels = json!({
                    "grandTotal": self.t("sheetInsert.grandTotal", "Grand Total"),
                    "rowLabels": self.t("sheetInsert.rowLabels", "Row Labels"),
                });
                self.view.run(json!({
                    "cmd": "pivotUpdate", "sheet": sheet, "index": index,
                    "rows": [], "cols": [], "values": [], "filters": [],
                    "labels": labels, "layout": "compact",
                }));
                self.view.engine_mut().merge_history_since(depth);
                if dest.is_none() && sheet >= before && src_sheet < sheet {
                    self.view
                        .run(json!({ "cmd": "moveSheet", "from": sheet, "to": src_sheet }));
                    self.view.engine_mut().merge_history_since(depth);
                    sheet = src_sheet;
                }
                self.view.changed();
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

/// Summary functions of Value Field Settings in the reference's order (§18.18.17
/// `ST_DataConsolidateFunction` tokens of the engine) with their labels.
const SUMMARIES: [(&str, &str); 11] = [
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

/// "Show values as" choices in the reference's order (§18.18.70 `ST_ShowDataAs` and the
/// x14 additions) and whether they need a base field / item.
const SHOW_AS: [(&str, &str, bool, bool); 15] = [
    ("normal", "No Calculation", false, false),
    ("percentOfTotal", "% of Grand Total", false, false),
    ("percentOfCol", "% of Column Total", false, false),
    ("percentOfRow", "% of Row Total", false, false),
    ("percent", "% Of", true, true),
    ("percentOfParentRow", "% of Parent Row Total", false, false),
    (
        "percentOfParentCol",
        "% of Parent Column Total",
        false,
        false,
    ),
    ("percentOfParent", "% of Parent Total", true, false),
    ("difference", "Difference From", true, true),
    ("percentDiff", "% Difference From", true, true),
    ("runTotal", "Running Total In", true, false),
    ("percentOfRunningTotal", "% Running Total In", true, false),
    ("rankAscending", "Rank Smallest to Largest", true, false),
    ("rankDescending", "Rank Largest to Smallest", true, false),
    ("index", "Index", false, false),
];

/// Base item indexes meaning "(previous)" / "(next)" (§18.10.1.11 `dataField/@baseItem`).
const BASE_PREVIOUS: u32 = 1_048_828;
const BASE_NEXT: u32 = 1_048_829;

impl SheetWindow {
    /// Area (0 Filters, 1 Columns, 2 Rows, 3 Values) and item slot under a pane point.
    fn pivot_area_at(lx: f32, ly: f32) -> Option<(usize, usize)> {
        AREAS.iter().enumerate().find_map(|(a, r)| {
            ((r[0]..r[0] + r[2]).contains(&lx) && (r[1] - 40.0..r[1] + r[3]).contains(&ly)).then(
                || {
                    (
                        a,
                        ((ly - r[1] - ITEM_Y) / ITEM_PITCH).floor().max(0.0) as usize,
                    )
                },
            )
        })
    }

    /// Release after pressing an area item: a click opens the item's menu (as the
    /// reference), a drag moves the field to the area / position under the pointer, or
    /// takes it out when dropped on the field list.
    pub(super) fn pivot_release(&mut self, lx: f32, ly: f32) -> bool {
        let Some((from, f, px, py)) = self.pivot_drag.take() else {
            return false;
        };
        if (lx - px).abs() + (ly - py).abs() < 5.0 {
            self.pivot_open_item_menu(from, f);
            return true;
        }
        if (LIST[0]..LIST[0] + LIST[2]).contains(&lx) && (LIST[1]..LIST[1] + LIST[3]).contains(&ly)
        {
            self.pivot_item_command("remove", from, f);
            return true;
        }
        let Some((to, at)) = Self::pivot_area_at(lx, ly) else {
            return true;
        };
        self.pivot_move(f, from, to, Some(at));
        true
    }

    /// Opens the menu of field `f` in area `area`, under its item.
    pub(super) fn pivot_open_item_menu(&mut self, area: usize, f: u32) {
        let Some(p) = self.pivot.as_ref() else {
            return;
        };
        let n = Self::area_fields(p, area)
            .iter()
            .position(|x| *x == f)
            .unwrap_or(0);
        let r = AREAS[area.min(3)];
        let rect = [
            r[0] + ITEM_X,
            r[1] + ITEM_Y + n as f32 * ITEM_PITCH,
            r[2] - 2.0 * ITEM_X,
            ITEM_H,
        ];
        self.pivot_item_menu(area, f, rect);
    }

    /// The menu of an area item (`rect`: the item, pane reference pixels): Move Up / Down /
    /// to Beginning / to End, Move to Report Filter / Row Labels / Column Labels / Values,
    /// Remove Field, Field Settings… (dimmed: row and column field settings are not offered
    /// yet) or Value Field Settings….
    pub(super) fn pivot_item_menu(&mut self, area: usize, f: u32, rect: [f32; 4]) {
        let Some(p) = self.pivot.as_ref() else {
            return;
        };
        let list = Self::area_fields(p, area);
        let i = list.iter().position(|x| *x == f).unwrap_or(0);
        let last = list.len().saturating_sub(1);
        let entries: [(&str, &str, &str, bool); 10] = [
            ("up", "sheetPivot.moveUp", "Move Up", i == 0),
            ("down", "sheetPivot.moveDown", "Move Down", i == last),
            ("begin", "sheetPivot.moveBegin", "Move to Beginning", i == 0),
            ("end", "sheetPivot.moveEnd", "Move to End", i == last),
            (
                "to0",
                "sheetPivot.toFilter",
                "Move to Report Filter",
                area == 0,
            ),
            ("to2", "sheetPivot.toRows", "Move to Row Labels", area == 2),
            (
                "to1",
                "sheetPivot.toCols",
                "Move to Column Labels",
                area == 1,
            ),
            ("to3", "sheetPivot.toValues", "Move to Values", area == 3),
            ("remove", "sheetPivot.remove", "Remove Field", false),
            if area == 3 {
                (
                    "settings",
                    "sheetPivot.valueSettings",
                    "Value Field Settings...",
                    false,
                )
            } else {
                (
                    "settings",
                    "sheetPivot.fieldSettings",
                    "Field Settings...",
                    true,
                )
            },
        ];
        let items = entries
            .iter()
            .map(|(op, key, d, dim)| MenuItem {
                label: self.t(key, d),
                choice: Choice::Command(format!("pivotitem.{op}.{area}.{f}")),
                checked: false,
                dim: *dim,
            })
            .collect();
        let pane = self.pane_rect();
        let k = self.scale / 1.25;
        let anchor = [
            pane[0] + rect[0] * k,
            pane[1] + rect[1] * k,
            rect[2] * k,
            rect[3] * k,
        ];
        // The reference's menu at 125 %: 236 px wide, 30 px rows, 15 px text 43 px in,
        // rules before Move to Report Filter, Remove Field and the settings (round 58).
        let look = MenuLook {
            item_h: 24.0,
            text: 12.0,
            text_x: (34.4, 34.4),
            min_w: 188.8,
            ..MenuLook::default()
        };
        self.menu = Some(
            Menu::open_with(items, anchor, self.size, self.scale, look)
                .with_decor(Vec::new(), vec![4, 8, 9]),
        );
    }

    /// A command of an area item's menu (`op` as in [`Self::pivot_item_menu`]).
    pub(super) fn pivot_item_command(&mut self, op: &str, area: usize, f: u32) -> bool {
        let Some(p) = self.pivot.clone() else {
            return false;
        };
        let list = Self::area_fields(&p, area);
        let Some(i) = list.iter().position(|x| *x == f) else {
            return false;
        };
        match op {
            "up" => self.pivot_move(f, area, area, Some(i.saturating_sub(1))),
            "down" => self.pivot_move(f, area, area, Some(i + 1)),
            "begin" => self.pivot_move(f, area, area, Some(0)),
            "end" => self.pivot_move(f, area, area, None),
            "remove" => {
                let mut p = p;
                match area {
                    0 => p.filters.retain(|x| *x != f),
                    1 => p.cols.retain(|x| *x != f),
                    2 => p.rows.retain(|x| *x != f),
                    _ => p.values.retain(|x| x.0 != f),
                }
                self.pivot_apply(&p);
            }
            "settings" if area == 3 => return self.open_value_settings(f),
            op => match op.strip_prefix("to").and_then(|a| a.parse::<usize>().ok()) {
                Some(to) if to < 4 && to != area => self.pivot_move(f, area, to, None),
                _ => return false,
            },
        }
        true
    }

    /// Moves field `f` from area `from` to area `to` at position `at` (`None`: the end). A
    /// field reaching Values is summed when it holds numbers, counted otherwise (as the
    /// reference); one leaving Values keeps its other areas.
    pub(super) fn pivot_move(&mut self, f: u32, from: usize, to: usize, at: Option<usize>) {
        let Some(mut p) = self.pivot.clone() else {
            return;
        };
        let func = p.values.iter().find(|v| v.0 == f).map_or_else(
            || {
                if p.fields.get(f as usize).is_some_and(|x| x.1) {
                    "sum".to_owned()
                } else {
                    "count".to_owned()
                }
            },
            |v| v.1.clone(),
        );
        match from {
            0 => p.filters.retain(|x| *x != f),
            1 => p.cols.retain(|x| *x != f),
            2 => p.rows.retain(|x| *x != f),
            _ => p.values.retain(|x| x.0 != f),
        }
        // A field is on one axis at a time (rows, columns or filters).
        if to != 3 {
            p.filters.retain(|x| *x != f);
            p.cols.retain(|x| *x != f);
            p.rows.retain(|x| *x != f);
        }
        let place = |v: &mut Vec<u32>| {
            let i = at.map_or(v.len(), |i| i.min(v.len()));
            v.insert(i, f);
        };
        match to {
            0 => place(&mut p.filters),
            1 => place(&mut p.cols),
            2 => place(&mut p.rows),
            _ => {
                let i = at.map_or(p.values.len(), |i| i.min(p.values.len()));
                p.values.insert(i, (f, func));
            }
        }
        self.pivot_apply(&p);
    }

    /// Value Field Settings for value field `f` (measured on the reference at 125 %:
    /// 514 × 421 px, Source Name, Custom Name, the Summarize Values By / Show Values As
    /// tabs, Number Format, OK, Cancel; `show/sheet-73`).
    pub(super) fn open_value_settings(&mut self, f: u32) -> bool {
        let Some(p) = self.pivot.as_mut() else {
            return false;
        };
        p.vfs = Some(f);
        let p = p.clone();
        let t = |k: &str, d: &str| self.t(k, d);
        let field = p
            .fields
            .get(f as usize)
            .map_or("", |x| x.0.as_str())
            .to_owned();
        let func = p
            .values
            .iter()
            .find(|v| v.0 == f)
            .map_or("sum", |v| v.1.as_str())
            .to_owned();
        let o = p.opts.get(&f).cloned().unwrap_or_default();
        let fi = SUMMARIES.iter().position(|(k, _)| *k == func).unwrap_or(0);
        let name = if o.name.is_empty() {
            format!(
                "{} {} {field}",
                t(&format!("sheetPivot.fn_{func}"), SUMMARIES[fi].1),
                t("sheetPivot.of", "of")
            )
        } else {
            o.name.clone()
        };
        let mut form = Form::new(
            &t("sheetPivot.vfsTitle", "Value Field Settings"),
            (514.0, 421.0),
            vec![
                t("sheetPivot.summarizeTab", "Summarize Values By"),
                t("sheetPivot.showAsTab", "Show Values As"),
            ],
        );
        form.tab_top = Some(115.0);
        form.page_h = Some(210.0);
        form.add(
            "srcL",
            None,
            [18.0, 47.0, 0.0, 0.0],
            Kind::Label(t("sheetPivot.sourceName", "Source Name:")),
        );
        form.add(
            "src",
            None,
            [114.0, 47.0, 0.0, 0.0],
            Kind::Label(field.clone()),
        );
        form.add(
            "nameL",
            None,
            [18.0, 79.0, 0.0, 0.0],
            Kind::Label(t("sheetPivot.customName", "Custom Name:")),
        );
        form.add(
            "name",
            None,
            [117.0, 75.0, 378.0, 26.0],
            Kind::Edit {
                value: name.chars().collect(),
                caret: name.chars().count(),
            },
        );
        form.add(
            "sumG",
            Some(0),
            [32.0, 152.0, 450.0, 0.0],
            Kind::Group(t("sheetPivot.summarizeBy", "Summarize value field by")),
        );
        form.set_bold("sumG");
        form.add(
            "sumN",
            Some(0),
            [32.0, 179.0, 0.0, 0.0],
            Kind::Note(t(
                "sheetPivot.summarizeHelp",
                "Pick the calculation that combines the values of this field",
            )),
        );
        form.add(
            "fn",
            Some(0),
            [31.0, 221.0, 315.0, 106.0],
            Kind::List {
                items: SUMMARIES
                    .iter()
                    .map(|(k, d)| t(&format!("sheetPivot.fn_{k}"), d))
                    .collect(),
                sel: Some(fi),
                top: 0,
            },
        );
        let si = SHOW_AS
            .iter()
            .position(|(k, ..)| *k == o.show_as)
            .unwrap_or(0);
        form.add(
            "showG",
            Some(1),
            [32.0, 152.0, 0.0, 0.0],
            Kind::Label(t("sheetPivot.showAs", "Show values as")),
        );
        form.set_bold("showG");
        form.add(
            "show",
            Some(1),
            [32.0, 179.0, 450.0, 27.0],
            Kind::Combo {
                items: SHOW_AS
                    .iter()
                    .map(|(k, d, ..)| t(&format!("sheetPivot.show_{k}"), d))
                    .collect(),
                sel: si,
            },
        );
        form.add(
            "baseFL",
            Some(1),
            [32.0, 212.0, 0.0, 0.0],
            Kind::Label(t("sheetPivot.baseField", "Base field:")),
        );
        form.add(
            "baseF",
            Some(1),
            [32.0, 232.0, 217.0, 105.0],
            Kind::List {
                items: p.fields.iter().map(|x| x.0.clone()).collect(),
                sel: Some(o.base_field as usize),
                top: 0,
            },
        );
        form.add(
            "baseIL",
            Some(1),
            [264.0, 212.0, 0.0, 0.0],
            Kind::Label(t("sheetPivot.baseItem", "Base item:")),
        );
        // The base item list stays empty while the calculation needs none (as the
        // reference shows it).
        let needs_item = SHOW_AS[si].3;
        let base_items = if needs_item {
            Self::base_items(&p, o.base_field, &t)
        } else {
            Vec::new()
        };
        let bi = match o.base_item {
            _ if !needs_item => None,
            BASE_PREVIOUS => Some(0),
            BASE_NEXT => Some(1),
            n => Some(n as usize + 2),
        };
        form.add(
            "baseI",
            Some(1),
            [264.0, 232.0, 218.0, 105.0],
            Kind::List {
                items: base_items,
                sel: bi,
                top: 0,
            },
        );
        form.add(
            "numFmt",
            None,
            [19.0, 374.0, 118.0, 27.0],
            Kind::Button {
                label: t("sheetPivot.numberFormat", "Number Format"),
                default: false,
            },
        );
        form.ok_cancel(&t("common.ok", "OK"), &t("common.cancel", "Cancel"));
        if let Some(c) = form.get_mut("ok") {
            c.rect = [256.0, 374.0, 111.0, 27.0];
        }
        if let Some(c) = form.get_mut("cancel") {
            c.rect = [383.0, 374.0, 111.0, 27.0];
        }
        Self::vfs_enable(&mut form, si);
        form.select_all("name");
        self.form = Some(form);
        self.form_kind = FormKind::PivotValue;
        true
    }

    /// "(previous)", "(next)" and the items of base field `field`.
    fn base_items(p: &PivotPane, field: u32, t: &dyn Fn(&str, &str) -> String) -> Vec<String> {
        let mut v = vec![
            t("sheetPivot.previous", "(previous)"),
            t("sheetPivot.next", "(next)"),
        ];
        v.extend(p.items.get(field as usize).cloned().unwrap_or_default());
        v
    }

    fn vfs_enable(form: &mut Form, show: usize) {
        let (_, _, field, item) = SHOW_AS[show.min(SHOW_AS.len() - 1)];
        form.enable("baseFL", field);
        form.enable("baseF", field);
        form.enable("baseIL", item);
        form.enable("baseI", item);
    }

    /// Events of Value Field Settings.
    pub(super) fn value_settings_event(&mut self, ev: &Event) {
        match ev {
            Event::Cancel => {
                self.form = None;
                if let Some(p) = self.pivot.as_mut() {
                    p.vfs = None;
                }
                self.sync_pivot_pane();
            }
            // Number Format: Format Cells with its Number page only; OK stores the code on
            // the value field (§18.10.1.21 `dataField/@numFmtId`), then this dialog returns.
            Event::Clicked("numFmt") => {
                self.vfs_stash = self.form.take();
                self.open_format_cells(0);
                if let Some(f) = self.form.as_mut() {
                    f.tabs.truncate(1);
                    let code = self
                        .pivot
                        .as_ref()
                        .and_then(|p| p.vfs.and_then(|v| p.opts.get(&v)))
                        .map(|o| o.num_fmt.clone())
                        .unwrap_or_default();
                    if !code.is_empty() {
                        f.set_text("code", &code);
                    }
                }
            }
            Event::Changed("show") => {
                let Some(p) = self.pivot.clone() else {
                    return;
                };
                let (si, bf) = self.form.as_ref().map_or((0, 0), |f| {
                    (f.sel("show").unwrap_or(0), f.sel("baseF").unwrap_or(0))
                });
                let needs_item = SHOW_AS[si.min(SHOW_AS.len() - 1)].3;
                let items = if needs_item {
                    Self::base_items(&p, bf as u32, &|k, d| self.t(k, d))
                } else {
                    Vec::new()
                };
                if let Some(form) = self.form.as_mut() {
                    Self::vfs_enable(form, si);
                    let had = form.sel("baseI").is_some();
                    if needs_item != had {
                        form.set_items("baseI", items, needs_item.then_some(0));
                    }
                }
            }
            Event::Changed("baseF") => {
                let Some(p) = self.pivot.clone() else {
                    return;
                };
                let bf = self.form.as_ref().and_then(|f| f.sel("baseF")).unwrap_or(0);
                let items = Self::base_items(&p, bf as u32, &|k, d| self.t(k, d));
                if let Some(form) = self.form.as_mut() {
                    let si = form.sel("show").unwrap_or(0).min(SHOW_AS.len() - 1);
                    if !SHOW_AS[si].3 {
                        return;
                    }
                    form.set_items("baseI", items, Some(0));
                }
            }
            Event::Changed("fn") => {
                // The default caption follows the function, as the reference's does.
                let Some(p) = self.pivot.clone() else {
                    return;
                };
                let Some(f) = p.vfs else {
                    return;
                };
                let field = p
                    .fields
                    .get(f as usize)
                    .map_or("", |x| x.0.as_str())
                    .to_owned();
                let of = self.t("sheetPivot.of", "of");
                let labels: Vec<String> = SUMMARIES
                    .iter()
                    .map(|(k, d)| self.t(&format!("sheetPivot.fn_{k}"), d))
                    .collect();
                if let Some(form) = self.form.as_mut() {
                    let name = form.text("name");
                    let default = labels.iter().any(|l| name == format!("{l} {of} {field}"));
                    if default {
                        let i = form.sel("fn").unwrap_or(0);
                        form.set_text("name", &format!("{} {of} {field}", labels[i]));
                    }
                }
            }
            Event::Ok | Event::Clicked("ok") => {
                let Some(form) = self.form.take() else {
                    return;
                };
                let Some(mut p) = self.pivot.clone() else {
                    return;
                };
                let Some(f) = p.vfs.take() else {
                    return;
                };
                if let Some(cur) = self.pivot.as_mut() {
                    cur.vfs = None;
                }
                let fi = form.sel("fn").unwrap_or(0).min(SUMMARIES.len() - 1);
                let si = form.sel("show").unwrap_or(0).min(SHOW_AS.len() - 1);
                let field = p
                    .fields
                    .get(f as usize)
                    .map_or("", |x| x.0.as_str())
                    .to_owned();
                let of = self.t("sheetPivot.of", "of");
                let name = form.text("name").trim().to_owned();
                let default = SUMMARIES.iter().any(|(k, d)| {
                    name == format!("{} {of} {field}", self.t(&format!("sheetPivot.fn_{k}"), d))
                });
                if let Some(v) = p.values.iter_mut().find(|v| v.0 == f) {
                    SUMMARIES[fi].0.clone_into(&mut v.1);
                }
                let base_item = match form.sel("baseI").unwrap_or(0) {
                    0 => BASE_PREVIOUS,
                    1 => BASE_NEXT,
                    n => u32::try_from(n - 2).unwrap_or(0),
                };
                let num_fmt = p
                    .opts
                    .get(&f)
                    .map(|o| o.num_fmt.clone())
                    .unwrap_or_default();
                p.opts.insert(
                    f,
                    ValueOpts {
                        num_fmt,
                        show_as: SHOW_AS[si].0.to_owned(),
                        base_field: form
                            .sel("baseF")
                            .and_then(|n| u32::try_from(n).ok())
                            .unwrap_or(0),
                        base_item,
                        name: if default || name.is_empty() {
                            String::new()
                        } else {
                            name
                        },
                    },
                );
                self.pivot_apply(&p);
            }
            _ => {}
        }
    }
}

impl SheetWindow {
    /// The gear button: a framed box with BlueOffice's own cog and a solid ▾.
    fn paint_gear(p: &mut dyn Painter, (x, y): (f32, f32), k: f32) {
        let (w, h) = ((GEAR[2] * k).round(), (GEAR[3] * k).round());
        p.fill_rect(x, y, w, h, BOX_EDGE);
        p.fill_rect(x + 1.0, y + 1.0, w - 2.0, h - 2.0, WHITE);
        // Cog: eight teeth round a ring.
        let (cx, cy) = (x + (22.0 * k).round(), y + (h / 2.0).round());
        let (ro, ri, rh) = (8.5 * k, 6.0 * k, 2.6 * k);
        let mut segs = Vec::new();
        for t in 0..16 {
            let a = std::f32::consts::PI * 2.0 * (t as f32 + 0.5) / 16.0;
            let r = if t % 2 == 0 { ro } else { ri };
            let (px, py) = (cx + r * a.cos(), cy + r * a.sin());
            segs.push(if t == 0 {
                PathSeg::M(px, py)
            } else {
                PathSeg::L(px, py)
            });
        }
        segs.push(PathSeg::Z);
        p.path(&segs, Some(Rgba(0x60, 0x60, 0x60, 255)), None);
        let kk = rh * 0.552_284_8;
        p.path(
            &[
                PathSeg::M(cx + rh, cy),
                PathSeg::C(cx + rh, cy + kk, cx + kk, cy + rh, cx, cy + rh),
                PathSeg::C(cx - kk, cy + rh, cx - rh, cy + kk, cx - rh, cy),
                PathSeg::C(cx - rh, cy - kk, cx - kk, cy - rh, cx, cy - rh),
                PathSeg::C(cx + kk, cy - rh, cx + rh, cy - kk, cx + rh, cy),
                PathSeg::Z,
            ],
            Some(WHITE),
            None,
        );
        let (ax, ay) = (x + w - (16.0 * k).round(), y + (h / 2.0).round());
        let (hw, ah) = (5.0 * k, 5.0 * k);
        p.path(
            &[
                PathSeg::M(ax - hw, ay - ah / 2.0),
                PathSeg::L(ax + hw, ay - ah / 2.0),
                PathSeg::L(ax, ay + ah / 2.0),
                PathSeg::Z,
            ],
            Some(INK),
            None,
        );
    }

    /// The gear menu: pane layouts (only the stacked one is offered yet; the others are
    /// shown dimmed) and the order of the field list.
    pub(super) fn pivot_gear_menu(&mut self) {
        let az = self.pivot.as_ref().is_some_and(|p| p.sort_az);
        let entries: [(&str, &str, &str, bool, bool); 7] = [
            (
                "stacked",
                "sheetPivot.layoutStacked",
                "Fields Section and Areas Section Stacked",
                true,
                false,
            ),
            (
                "sideBySide",
                "sheetPivot.layoutSide",
                "Fields Section and Areas Section Side-By-Side",
                false,
                true,
            ),
            (
                "fieldsOnly",
                "sheetPivot.layoutFields",
                "Fields Section Only",
                false,
                true,
            ),
            (
                "areas22",
                "sheetPivot.layoutAreas22",
                "Areas Section Only (2 by 2)",
                false,
                true,
            ),
            (
                "areas14",
                "sheetPivot.layoutAreas14",
                "Areas Section Only (1 by 4)",
                false,
                true,
            ),
            ("az", "sheetPivot.sortAz", "Sort A to Z", az, false),
            (
                "source",
                "sheetPivot.sortSource",
                "Sort in Data Source Order",
                !az,
                false,
            ),
        ];
        let items = entries
            .iter()
            .map(|(op, key, d, checked, dim)| MenuItem {
                label: self.t(key, d),
                choice: Choice::Command(format!("pivotgear.{op}")),
                checked: *checked,
                dim: *dim,
            })
            .collect();
        let pane = self.pane_rect();
        let k = self.scale / 1.25;
        let anchor = [
            pane[0] + GEAR[0] * k,
            pane[1] + GEAR[1] * k,
            GEAR[2] * k,
            GEAR[3] * k,
        ];
        let look = MenuLook {
            item_h: 24.0,
            text: 12.0,
            text_x: (34.4, 34.4),
            min_w: 188.8,
            ..MenuLook::default()
        };
        self.menu = Some(
            Menu::open_with(items, anchor, self.size, self.scale, look)
                .with_decor(Vec::new(), vec![5]),
        );
    }

    /// A choice of the gear menu.
    pub(super) fn pivot_gear_command(&mut self, op: &str) -> bool {
        let Some(p) = self.pivot.as_mut() else {
            return false;
        };
        match op {
            "az" => p.sort_az = true,
            "source" => p.sort_az = false,
            _ => return false,
        }
        true
    }
}

impl SheetWindow {
    /// Format Cells opened from Value Field Settings closed: OK keeps its number format code
    /// for the value field; both return to Value Field Settings. `false` when Format Cells
    /// was not opened from there.
    pub(super) fn vfs_number_format_done(&mut self, ok: bool) -> bool {
        let Some(stash) = self.vfs_stash.take() else {
            return false;
        };
        if ok {
            let code = self.fc_number_code();
            if let Some(p) = self.pivot.as_mut() {
                if let Some(f) = p.vfs {
                    p.opts.entry(f).or_default().num_fmt = code;
                }
            }
        }
        self.form = Some(stash);
        self.form_kind = FormKind::PivotValue;
        true
    }
}
