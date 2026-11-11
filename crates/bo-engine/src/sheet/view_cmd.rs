//! Workbook views of BlueSheet (SHEET-VIEW-001): the sheet view type
//! (ECMA-376 Part 1, §18.3.1.87 `sheetView/@view`, §18.18.69 ST_SheetViewType),
//! show formulas and the per-view zoom; page break preview with automatic and
//! manual breaks (§18.3.1.74 `rowBreaks`, §18.3.1.14 `colBreaks`) that can be
//! dragged; the page layout view (pages with margins and header / footer areas);
//! custom views (§18.3.1.25 `customSheetView`, §18.2.2 `customWorkbookView`).
//!
//! Choices of ours where the spec is silent:
//! * view type, show formulas and zoom are view state: they change the sheet
//!   view without an undo step;
//! * dragging a break so that the page before it no longer fits lowers the
//!   print scale (`pageSetup/@scale`, fit-to-page turned off) to the largest
//!   whole percent at which that page fits; manual breaks the drag passes over
//!   are removed; dragging a break out of the print area removes it;
//! * a saved custom view keeps the exact hidden rows / columns and the print
//!   area (§18.3.1.25 only flags them; `bo-xlsx` writes the lists as hidden
//!   `Z_<guid>_.wvu.*` defined names); a view without them re-applies its
//!   AutoFilter instead.

use super::cmd::Recalc;
use super::print::{settings, PageBox, Pagination};
use super::SheetEngine;
use crate::{EngineError, Result};
use bo_model::sheet::{
    CellRef, CustomSheetView, CustomWorkbookView, SheetState, SheetView, SheetViewType, Workbook,
    Worksheet,
};
use serde::Deserialize;
use serde_json::{json, Value};

/// A view command (parsed before [`super::cmd::SheetCommand`]).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "cmd", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum ViewCommand {
    /// View type, show formulas and zoom of a sheet → `{ view, showFormulas, zoom, zooms }`.
    SheetViewState { sheet: usize },
    /// Changes the view type, show formulas, the outline bar and / or the zoom (no
    /// undo step).
    SetSheetView {
        sheet: usize,
        #[serde(default)]
        view: Option<SheetViewType>,
        #[serde(default)]
        show_formulas: Option<bool>,
        /// `showOutlineSymbols` (§18.3.1.87).
        #[serde(default)]
        outline_symbols: Option<bool>,
        /// Zoom percent of the (new) view type, 10–400.
        #[serde(default)]
        zoom: Option<u32>,
    },
    /// Pages and breaks of every print area → `{ firstPage, areas: [...] }`.
    PageBreakPreview { sheet: usize },
    /// Moves (or inserts) a page break. One undo step.
    MovePageBreak {
        sheet: usize,
        /// Row breaks (`true`) or column breaks.
        rows: bool,
        /// Print area index (from `pageBreakPreview`).
        #[serde(default)]
        area: usize,
        /// First row / column of the next page at the break being dragged
        /// (absent: a new break).
        #[serde(default)]
        from: Option<u32>,
        /// First row / column of the next page where the break goes; outside
        /// the print area removes the break.
        to: u32,
    },
    /// Printed pages of a sheet → `{ pages: [{ width, height, margins, cells }] }`.
    PageLayout { sheet: usize },
    /// The cell under a point of page `page` (points) → `{ row, col }` or `null`.
    PageHit {
        sheet: usize,
        page: usize,
        x: f64,
        y: f64,
    },
    /// Custom views of the workbook → `[{ name, guid, includePrintSettings, includeHiddenRowCol }]`.
    CustomViews,
    /// Saves the current view of every sheet under `name` (an existing view of
    /// that name is replaced). One undo step.
    AddCustomView {
        name: String,
        /// The active sheet.
        sheet: usize,
        #[serde(default = "yes")]
        include_print_settings: bool,
        #[serde(default = "yes")]
        include_hidden_row_col: bool,
        /// What the editor shows on the active sheet: zoom percent, active
        /// cell, selected range (A1) and top-left visible cell.
        #[serde(default)]
        zoom: Option<u32>,
        #[serde(default)]
        active: Option<[u32; 2]>,
        #[serde(default)]
        sqref: Option<String>,
        #[serde(default)]
        top_left: Option<[u32; 2]>,
    },
    /// Shows a custom view → `{ sheet, zoom, active, topLeft }`. One undo step.
    ApplyCustomView { guid: String },
    /// Deletes a custom view. One undo step.
    DeleteCustomView { guid: String },
}

fn yes() -> bool {
    true
}

impl ViewCommand {
    /// Parses a JSON command when it is a view command.
    pub fn parse(v: &Value) -> Option<ViewCommand> {
        const NAMES: &[&str] = &[
            "sheetViewState",
            "setSheetView",
            "pageBreakPreview",
            "movePageBreak",
            "pageLayout",
            "pageHit",
            "customViews",
            "addCustomView",
            "applyCustomView",
            "deleteCustomView",
        ];
        let name = v.get("cmd")?.as_str()?;
        if !NAMES.contains(&name) {
            return None;
        }
        serde_json::from_value(v.clone()).ok()
    }

    /// The command records an undo step (changes the file content).
    fn edits(&self) -> bool {
        matches!(
            self,
            ViewCommand::MovePageBreak { .. }
                | ViewCommand::AddCustomView { .. }
                | ViewCommand::ApplyCustomView { .. }
                | ViewCommand::DeleteCustomView { .. }
        )
    }
}

fn bad(msg: impl Into<String>) -> EngineError {
    EngineError::Sheet(msg.into())
}

/// A page break between two pages: `at` is the first row / column of the next page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Break {
    pub at: u32,
    pub manual: bool,
}

/// Breaks between the spans of one axis; a break is manual when the sheet has
/// a manual break after the last index of the span before it.
pub(super) fn breaks_of(spans: &[(u32, u32)], manual: &[u32]) -> Vec<Break> {
    spans
        .windows(2)
        .map(|w| Break {
            at: w[1].0,
            manual: manual.contains(&w[0].1),
        })
        .collect()
}

/// Scale percent at which a page of `size` points (plus `fixed` points of
/// repeated titles / headings) fits `avail` points, when it does not fit at
/// `scale` (a factor). `None` when it already fits. Clamped to 10–400 %
/// (§18.3.1.63 `scale`).
pub(super) fn fit_scale(size: f64, fixed: f64, avail: f64, scale: f64) -> Option<u32> {
    let need = size + fixed;
    if need <= 0.0 || need * scale <= avail + 0.01 {
        return None;
    }
    let pct = (avail / need * 100.0).floor();
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let pct = pct.clamp(10.0, 400.0) as u32;
    Some(pct)
}

/// The sheet view of `ws` that BlueSheet shows (the first; created when missing).
fn view_mut(ws: &mut Worksheet) -> &mut SheetView {
    if ws.views.is_empty() {
        ws.views.push(SheetView::default());
    }
    &mut ws.views[0]
}

/// Zoom percent stored for view type `v` (the current `zoomScale` for the
/// current type), 100 when none.
fn zoom_for(view: Option<&SheetView>, v: SheetViewType) -> u32 {
    let Some(sv) = view else {
        return 100;
    };
    sv.zoom_of(v)
        .or_else(|| (sv.view_type() == v).then_some(sv.zoom_scale).flatten())
        .filter(|z| *z > 0)
        .unwrap_or(100)
}

fn state_json(ws: &Worksheet) -> Value {
    let v = ws.views.first();
    let t = v.map_or(SheetViewType::Normal, SheetView::view_type);
    json!({
        "view": t,
        "showFormulas": v.is_some_and(SheetView::show_formulas),
        "outlineSymbols": v.is_none_or(SheetView::show_outline_symbols),
        "zoom": v.and_then(|v| v.zoom_scale).filter(|z| *z > 0).unwrap_or_else(|| zoom_for(v, t)),
        "zooms": {
            "normal": zoom_for(v, SheetViewType::Normal),
            "pageBreakPreview": zoom_for(v, SheetViewType::PageBreakPreview),
            "pageLayout": zoom_for(v, SheetViewType::PageLayout),
        },
    })
}

fn sel_cell(text: Option<&str>) -> Option<[u32; 2]> {
    let c = CellRef::parse(text?.split_whitespace().next()?.split(':').next()?)?;
    Some([c.row, c.col])
}

impl SheetEngine {
    /// Runs a view command.
    pub fn view_command(&mut self, cmd: ViewCommand) -> Result<Value> {
        if cmd.edits() {
            self.writable()?;
        }
        match cmd {
            ViewCommand::SheetViewState { sheet } => Ok(state_json(self.sheet(sheet)?)),
            ViewCommand::SetSheetView {
                sheet,
                view,
                show_formulas,
                outline_symbols,
                zoom,
            } => {
                if let Some(on) = outline_symbols {
                    let ws = self
                        .wb
                        .as_mut()
                        .ok_or(EngineError::NoDocument)?
                        .sheets
                        .get_mut(sheet)
                        .ok_or(EngineError::PageOutOfRange(sheet))?;
                    view_mut(ws).set_show_outline_symbols(on);
                }
                self.set_sheet_view(sheet, view, show_formulas, zoom)
            }
            ViewCommand::PageBreakPreview { sheet } => self.break_preview(sheet),
            ViewCommand::MovePageBreak {
                sheet,
                rows,
                area,
                from,
                to,
            } => self.move_page_break(sheet, rows, area, from, to),
            ViewCommand::PageLayout { sheet } => {
                let pages: Vec<Value> = self.page_layout(sheet)?.iter().map(page_json).collect();
                Ok(json!({ "pages": pages }))
            }
            ViewCommand::PageHit { sheet, page, x, y } => self.page_hit(sheet, page, x, y),
            ViewCommand::CustomViews => {
                let wb = self.wb()?;
                Ok(Value::Array(
                    wb.custom_views
                        .iter()
                        .map(|v| {
                            json!({
                                "name": v.name,
                                "guid": v.guid,
                                "includePrintSettings": v.include_print_settings,
                                "includeHiddenRowCol": v.include_hidden_row_col,
                            })
                        })
                        .collect(),
                ))
            }
            ViewCommand::AddCustomView {
                name,
                sheet,
                include_print_settings,
                include_hidden_row_col,
                zoom,
                active,
                sqref,
                top_left,
            } => {
                let name = name.trim().to_owned();
                if name.is_empty() {
                    return Err(bad("custom view name is empty"));
                }
                self.sheet(sheet)?;
                let ui = UiView {
                    zoom,
                    active,
                    sqref,
                    top_left,
                };
                self.edit(sheet, Recalc::No, |wb, _| {
                    let guid = add_custom_view(
                        wb,
                        &name,
                        sheet,
                        include_print_settings,
                        include_hidden_row_col,
                        &ui,
                    );
                    Ok(json!({ "guid": guid }))
                })
            }
            ViewCommand::ApplyCustomView { guid } => {
                let out = self.edit(0, Recalc::No, |wb, _| apply_custom_view(wb, &guid))?;
                // Re-apply AutoFilters restored without their hidden rows.
                if let Some(list) = out.get("refilter").and_then(Value::as_array) {
                    let locale = self.locale;
                    let sheets: Vec<usize> = list
                        .iter()
                        .filter_map(Value::as_u64)
                        .filter_map(|v| usize::try_from(v).ok())
                        .collect();
                    if let Some(wb) = self.wb.as_mut() {
                        for s in sheets {
                            bo_sheet::filter::apply_auto_filter(wb, s, &locale);
                        }
                    }
                }
                self.invalidate(None);
                Ok(out)
            }
            ViewCommand::DeleteCustomView { guid } => self.edit(0, Recalc::No, |wb, _| {
                let before = wb.custom_views.len();
                wb.custom_views.retain(|v| v.guid != guid);
                for ws in &mut wb.sheets {
                    ws.custom_views.retain(|v| v.guid != guid);
                }
                if wb.custom_views.len() == before {
                    return Err(bad("no such custom view"));
                }
                Ok(json!({ "ok": true }))
            }),
        }
    }

    fn set_sheet_view(
        &mut self,
        sheet: usize,
        view: Option<SheetViewType>,
        show_formulas: Option<bool>,
        zoom: Option<u32>,
    ) -> Result<Value> {
        let wb = self.wb.as_mut().ok_or(EngineError::NoDocument)?;
        let ws = wb
            .sheets
            .get_mut(sheet)
            .ok_or(EngineError::PageOutOfRange(sheet))?;
        let v = view_mut(ws);
        if let Some(t) = view {
            if t != v.view_type() {
                // Keep the zoom of the view being left (§18.3.1.87 zoomScaleNormal /
                // zoomScaleSheetLayoutView / zoomScalePageLayoutView).
                let old = v.view_type();
                if let Some(z) = v.zoom_scale.filter(|z| *z > 0) {
                    v.set_attr(old.zoom_attr(), Some(z.to_string()));
                }
                v.set_view_type(t);
                v.zoom_scale = v.zoom_of(t).filter(|z| *z != 100);
            }
        }
        if let Some(z) = zoom {
            let z = z.clamp(10, 400);
            let t = v.view_type();
            v.zoom_scale = (z != 100).then_some(z);
            v.set_attr(t.zoom_attr(), Some(z.to_string()));
        }
        if let Some(on) = show_formulas {
            v.set_show_formulas(on);
        }
        let out = state_json(ws);
        // Show formulas widens the columns.
        self.invalidate(Some(sheet));
        Ok(out)
    }

    fn break_preview(&mut self, sheet: usize) -> Result<Value> {
        let ws = self.sheet(sheet)?;
        let st = settings(ws);
        let manual_rows = ws.page_setup.row_breaks.clone();
        let manual_cols = ws.page_setup.col_breaks.clone();
        let mut number = st.first_page;
        let mut areas = Vec::new();
        for (area, p) in self.paginations(sheet, &st)? {
            let pages: Vec<Value> = p
                .order
                .iter()
                .map(|&(c, r)| {
                    let n = number;
                    number += 1;
                    json!({ "col": c, "row": r, "number": n })
                })
                .collect();
            let brk = |b: &Break| json!({ "at": b.at, "manual": b.manual });
            areas.push(json!({
                "r0": area.first.row,
                "c0": area.first.col,
                "r1": area.last.row,
                "c1": area.last.col,
                "scale": p.scale,
                "cols": p.cols.iter().map(|s| [s.0, s.1]).collect::<Vec<_>>(),
                "rows": p.rows.iter().map(|s| [s.0, s.1]).collect::<Vec<_>>(),
                "pages": pages,
                "rowBreaks": breaks_of(&p.rows, &manual_rows).iter().map(brk).collect::<Vec<_>>(),
                "colBreaks": breaks_of(&p.cols, &manual_cols).iter().map(brk).collect::<Vec<_>>(),
            }));
        }
        Ok(json!({ "firstPage": st.first_page, "areas": areas }))
    }

    fn move_page_break(
        &mut self,
        sheet: usize,
        rows: bool,
        area: usize,
        from: Option<u32>,
        to: u32,
    ) -> Result<Value> {
        let st = settings(self.sheet(sheet)?);
        let pags = self.paginations(sheet, &st)?;
        let (range, p): &(bo_model::sheet::CellRange, Pagination) =
            pags.get(area).ok_or_else(|| bad("no such print area"))?;
        let (first, last) = if rows {
            (range.first.row, range.last.row)
        } else {
            (range.first.col, range.last.col)
        };
        let spans = if rows { &p.rows } else { &p.cols };
        let inside = to > first && to <= last;
        // A break dragged later makes the page before it longer: from the start of
        // the page that ended at the old break up to the new one.
        let mut rescale = None;
        if let (Some(f), true) = (from, inside) {
            if to > f {
                let start = spans
                    .iter()
                    .find(|s| s.0 < f && f - 1 <= s.1)
                    .map_or(first, |s| s.0);
                let g = self.geometry(sheet)?;
                let size = if rows {
                    g.row_y(to) - g.row_y(start)
                } else {
                    g.col_x(to) - g.col_x(start)
                };
                let (aw, ah) = st.avail();
                let (avail, fixed) = if rows {
                    (ah, p.fixed.1)
                } else {
                    (aw, p.fixed.0)
                };
                rescale = fit_scale(size, fixed, avail, p.scale);
            }
        }
        self.edit(sheet, Recalc::No, |wb, _| {
            let ws = &mut wb.sheets[sheet];
            let list = if rows {
                &mut ws.page_setup.row_breaks
            } else {
                &mut ws.page_setup.col_breaks
            };
            if let Some(f) = from.filter(|f| *f > 0) {
                // Breaks the drag passes over go too.
                let (lo, hi) = (f.min(to), f.max(to));
                list.retain(|b| *b + 1 != f && (*b < lo || *b + 1 >= hi));
            }
            if inside && !list.contains(&(to - 1)) {
                list.push(to - 1);
                list.sort_unstable();
            }
            if let Some(pct) = rescale {
                ws.page_setup.scale = Some(pct);
                ws.page_setup.has_page_setup = true;
                if ws.props.fit_to_page == Some(true) {
                    ws.props.fit_to_page = Some(false);
                }
            }
            Ok(json!({ "scale": rescale }))
        })
    }

    fn page_hit(&mut self, sheet: usize, page: usize, x: f64, y: f64) -> Result<Value> {
        let pages = self.page_layout(sheet)?;
        let Some(cells) = pages.get(page).and_then(|p| p.cells.clone()) else {
            return Ok(Value::Null);
        };
        let (ox, oy) = cells.origin;
        if x < ox || y < oy || cells.scale <= 0.0 {
            return Ok(Value::Null);
        }
        let g = self.geometry(sheet)?;
        let sx = g.col_x(cells.cols.0) + (x - ox) / cells.scale;
        let sy = g.row_y(cells.rows.0) + (y - oy) / cells.scale;
        if sx >= g.col_x(cells.cols.1 + 1) || sy >= g.row_y(cells.rows.1 + 1) {
            return Ok(Value::Null);
        }
        let c = g.hit_test(sx, sy);
        Ok(json!({ "row": c.row, "col": c.col }))
    }
}

fn page_json(p: &PageBox) -> Value {
    json!({
        "width": p.size.0,
        "height": p.size.1,
        "margins": p.margins,
        "cells": p.cells.as_ref().map(|c| json!({
            "c0": c.cols.0,
            "c1": c.cols.1,
            "r0": c.rows.0,
            "r1": c.rows.1,
            "scale": c.scale,
            "x": c.origin.0,
            "y": c.origin.1,
        })),
    })
}

/// What the editor shows on the active sheet when a custom view is saved.
struct UiView {
    zoom: Option<u32>,
    active: Option<[u32; 2]>,
    sqref: Option<String>,
    top_left: Option<[u32; 2]>,
}

/// Window size written for new custom views (required by §18.2.2; our choice).
const WINDOW: (u32, u32) = (1280, 720);

fn add_custom_view(
    wb: &mut Workbook,
    name: &str,
    active_sheet: usize,
    print: bool,
    hidden: bool,
    ui: &UiView,
) -> String {
    // A view of the same name is replaced (names compare case-insensitively).
    let lname = name.to_lowercase();
    if let Some(old) = wb
        .custom_views
        .iter()
        .find(|v| v.name.to_lowercase() == lname)
        .map(|v| v.guid.clone())
    {
        wb.custom_views.retain(|v| v.guid != old);
        for ws in &mut wb.sheets {
            ws.custom_views.retain(|v| v.guid != old);
        }
    }
    let taken: Vec<String> = wb.custom_views.iter().map(|v| v.guid.clone()).collect();
    let guid = super::comments::guid(&(name, wb.custom_views.len(), wb.sheets.len()), &|g| {
        taken.iter().any(|t| t == g)
    });
    for (i, ws) in wb.sheets.iter_mut().enumerate() {
        let sv = ws.views.first().cloned().unwrap_or_default();
        let mut selection = sv.selections.first().cloned();
        let mut top_left = sv.top_left_cell.clone();
        let mut scale = sv.zoom_scale;
        if i == active_sheet {
            if let Some(z) = ui.zoom {
                scale = Some(z.clamp(10, 400));
            }
            if let Some([r, c]) = ui.top_left {
                top_left = Some(CellRef::new(r, c).to_string());
            }
            if let Some([r, c]) = ui.active {
                let cell = CellRef::new(r, c).to_string();
                let mut s = vec![("activeCell".to_owned(), cell.clone())];
                if let Some(p) = sv.pane.as_ref().and_then(|p| p.active_pane.clone()) {
                    s.insert(0, ("pane".to_owned(), p));
                }
                s.push(("sqref".to_owned(), ui.sqref.clone().unwrap_or(cell)));
                selection = Some(s);
            }
        }
        let hidden_rows = bo_sheet::rowcol::hidden_spans(ws, true);
        let hidden_cols = bo_sheet::rowcol::hidden_spans(ws, false);
        let t = sv.view_type();
        ws.custom_views.push(CustomSheetView {
            guid: guid.clone(),
            scale: scale.filter(|z| *z > 0),
            show_formulas: sv.show_formulas(),
            show_grid_lines: sv.show_grid_lines,
            show_row_col: sv.show_headers,
            fit_to_page: print && ws.props.fit_to_page == Some(true),
            print_area: print && ws.print_area.is_some(),
            filter: ws
                .auto_filter
                .as_ref()
                .is_some_and(|af| !af.columns.is_empty()),
            show_auto_filter: ws.auto_filter.is_some(),
            hidden_rows: hidden && !hidden_rows.is_empty(),
            hidden_columns: hidden && !hidden_cols.is_empty(),
            state: match ws.state {
                SheetState::Visible => None,
                SheetState::Hidden => Some("hidden".into()),
                SheetState::VeryHidden => Some("veryHidden".into()),
            },
            view: (t != SheetViewType::Normal).then(|| t.as_str().to_owned()),
            top_left_cell: top_left,
            pane: sv.pane.clone(),
            selection,
            page_setup: print.then(|| ws.page_setup.clone()),
            auto_filter: ws.auto_filter.clone(),
            session_hidden_rows: hidden.then_some(hidden_rows),
            session_hidden_cols: hidden.then_some(hidden_cols),
            session_print_area: print.then(|| ws.print_area.clone()),
            ..CustomSheetView::default()
        });
    }
    let active_sheet_id = wb.sheets.get(active_sheet).map_or(1, |s| s.sheet_id.max(1));
    wb.custom_views.push(CustomWorkbookView {
        name: name.to_owned(),
        guid: guid.clone(),
        active_sheet_id,
        window_width: WINDOW.0,
        window_height: WINDOW.1,
        include_print_settings: print,
        include_hidden_row_col: hidden,
        ..CustomWorkbookView::default()
    });
    guid
}

fn apply_custom_view(wb: &mut Workbook, guid: &str) -> Result<Value> {
    let book = wb
        .custom_views
        .iter()
        .find(|v| v.guid == guid)
        .cloned()
        .ok_or_else(|| bad("no such custom view"))?;
    let mut refilter = Vec::new();
    let mut out_zoom = None;
    let mut out_active = None;
    let mut out_top = None;
    let active_sheet = wb
        .sheets
        .iter()
        .position(|s| s.sheet_id == book.active_sheet_id)
        .unwrap_or(0);
    for (i, ws) in wb.sheets.iter_mut().enumerate() {
        let Some(cv) = ws.custom_views.iter().find(|v| v.guid == guid).cloned() else {
            continue;
        };
        let v = view_mut(ws);
        v.zoom_scale = cv.scale.filter(|z| *z != 100);
        v.set_show_formulas(cv.show_formulas);
        v.show_grid_lines = cv.show_grid_lines;
        v.show_headers = cv.show_row_col;
        v.set_view_type(
            cv.view
                .as_deref()
                .map_or(SheetViewType::Normal, SheetViewType::parse),
        );
        v.top_left_cell.clone_from(&cv.top_left_cell);
        v.pane.clone_from(&cv.pane);
        if let Some(s) = &cv.selection {
            v.selections = vec![s.clone()];
        }
        if i == active_sheet {
            out_zoom = Some(cv.scale.unwrap_or(100));
            let get = |name: &str| {
                cv.selection
                    .as_ref()
                    .and_then(|s| s.iter().find(|(k, _)| k == name))
                    .map(|(_, v)| v.as_str())
            };
            out_active = sel_cell(get("activeCell").or_else(|| get("sqref")));
            out_top = sel_cell(cv.top_left_cell.as_deref());
        }
        if book.include_hidden_row_col {
            for (rows, spans) in [
                (true, &cv.session_hidden_rows),
                (false, &cv.session_hidden_cols),
            ] {
                if let Some(spans) = spans {
                    for (a, b) in bo_sheet::rowcol::hidden_spans(ws, rows) {
                        bo_sheet::rowcol::set_hidden(ws, rows, a, b, false);
                    }
                    for &(a, b) in spans {
                        bo_sheet::rowcol::set_hidden(ws, rows, a, b, true);
                    }
                }
            }
        }
        if book.include_print_settings {
            if let Some(ps) = &cv.page_setup {
                ws.page_setup = ps.clone();
                ws.props.fit_to_page = cv.fit_to_page.then_some(true);
            }
            if let Some(pa) = &cv.session_print_area {
                ws.print_area.clone_from(pa);
            }
        }
        ws.auto_filter.clone_from(&cv.auto_filter);
        if cv.filter && cv.session_hidden_rows.is_none() {
            refilter.push(i);
        }
        ws.state = match cv.state.as_deref() {
            Some("hidden") => SheetState::Hidden,
            Some("veryHidden") => SheetState::VeryHidden,
            _ => SheetState::Visible,
        };
    }
    Ok(json!({
        "sheet": active_sheet,
        "zoom": out_zoom,
        "active": out_active,
        "topLeft": out_top,
        "refilter": refilter,
    }))
}

#[cfg(test)]
#[path = "view_cmd_tests.rs"]
mod tests;
