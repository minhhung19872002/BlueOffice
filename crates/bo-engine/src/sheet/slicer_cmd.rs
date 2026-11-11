//! Slicer and timeline commands over the JSON bridge ([MS-XLSX] slicer and timeline parts):
//! listing with the colours of their styles, inserting over a pivot table field, options
//! (caption, columns, style, time level), report connections, moving / sizing the frame and
//! deleting.

use super::cmd::Recalc;
use super::SheetEngine;
use crate::{EngineError, Result};
use bo_model::sheet::{CellRef, SlicerSource, Workbook, MAX_COLS, MAX_ROWS};
use bo_sheet::pivot::PivotLabels;
use serde_json::{json, Value};

fn bad(msg: impl Into<String>) -> EngineError {
    EngineError::Sheet(msg.into())
}

/// Pivot tables a slicer or timeline may filter, for the report connections dialog.
fn connections(wb: &Workbook, pivots: &[String]) -> Vec<Value> {
    bo_sheet::slicer::connectable(wb, pivots)
        .into_iter()
        .map(|(s, i, name, on)| {
            json!({
                "sheet": s,
                "sheetName": wb.sheets[s].name,
                "index": i,
                "name": name,
                "connected": on,
            })
        })
        .collect()
}

/// Where a new slicer or timeline of pivot table `pivot` goes on `sheet`: right of the report
/// when it is on that sheet, at the current view's top otherwise.
fn place(wb: &Workbook, sheet: usize, pivot_sheet: usize, pivot: usize) -> CellRef {
    if sheet != pivot_sheet {
        return CellRef::new(1, 1);
    }
    wb.sheets
        .get(pivot_sheet)
        .and_then(|ws| ws.pivots.get(pivot))
        .map_or(CellRef::new(1, 1), |p| {
            let area = bo_sheet::pivot::occupied(p);
            // Below the frames and charts already placed there (a pivot chart starts in
            // the same column).
            let col = (area.last.col + 2).min(MAX_COLS - 1);
            let ws = &wb.sheets[sheet];
            let taken = ws
                .slicers
                .iter()
                .map(|s| (s.from, s.to))
                .chain(ws.timelines.iter().map(|t| (t.from, t.to)))
                .chain(ws.charts.iter().map(|c| (c.from, c.to)))
                .filter(|(from, _)| from.col == col)
                .map(|(from, to)| to.map_or(from.row + 15, |t| t.row) + 1)
                .max();
            CellRef::new(taken.unwrap_or(area.first.row).min(MAX_ROWS - 1), col)
        })
}

/// Before pivot table `name` is deleted: slicers and timelines connected to it lose that
/// connection; the ones it was the only pivot table of are removed (a slicer cache lists at
/// least one pivot table).
pub(super) fn detach_frames(wb: &mut Workbook, name: &str) {
    for si in 0..wb.sheets.len() {
        let mut i = 0;
        while i < wb.sheets[si].slicers.len() {
            let SlicerSource::Pivot { pivots, .. } = &mut wb.sheets[si].slicers[i].source else {
                i += 1;
                continue;
            };
            if !pivots.iter().any(|p| p == name) {
                i += 1;
                continue;
            }
            pivots.retain(|p| p != name);
            if pivots.is_empty() {
                bo_xlsx::slicer::remove_slicer(wb, si, i);
            } else {
                bo_xlsx::slicer::patch_slicer(wb, si, i);
                i += 1;
            }
        }
        let mut i = 0;
        while i < wb.sheets[si].timelines.len() {
            let pivots = &mut wb.sheets[si].timelines[i].pivots;
            if !pivots.iter().any(|p| p == name) {
                i += 1;
                continue;
            }
            pivots.retain(|p| p != name);
            if pivots.is_empty() {
                bo_xlsx::timeline::remove_timeline(wb, si, i);
            } else {
                bo_xlsx::timeline::patch_timeline(wb, si, i);
                i += 1;
            }
        }
    }
}

/// After pivot table `name` moved to another sheet: the parts of the slicers and timelines
/// read from the file name it by its sheet (`pivotTable/@tabId`) and are written again.
pub(super) fn sync_frames(wb: &mut Workbook, name: &str) {
    for si in 0..wb.sheets.len() {
        for i in 0..wb.sheets[si].slicers.len() {
            let hit = matches!(
                &wb.sheets[si].slicers[i].source,
                SlicerSource::Pivot { pivots, .. } if pivots.iter().any(|p| p == name)
            );
            if hit {
                bo_xlsx::slicer::patch_slicer(wb, si, i);
            }
        }
        for i in 0..wb.sheets[si].timelines.len() {
            if wb.sheets[si].timelines[i].pivots.iter().any(|p| p == name) {
                bo_xlsx::timeline::patch_timeline(wb, si, i);
            }
        }
    }
}

/// Names of the workbook's custom slicer (or timeline) styles, for the options dialog.
fn custom_styles(wb: &Workbook, timeline: bool) -> Vec<&str> {
    wb.styles
        .frame_styles
        .iter()
        .filter(|s| s.timeline == timeline)
        .map(|s| s.name.as_str())
        .collect()
}

/// Which parts of a timeline are shown ([MS-XLSX] `CT_Timeline` `showHeader`,
/// `showSelectionLabel`, `showTimeLevel`, `showHorizontalScrollbar`); absent = unchanged.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize)]
pub struct TimelineShow {
    /// The caption and its buttons.
    #[serde(default, rename = "showHeader")]
    pub header: Option<bool>,
    /// The label of the chosen range.
    #[serde(default, rename = "showSelectionLabel")]
    pub selection_label: Option<bool>,
    /// The time level choice.
    #[serde(default, rename = "showTimeLevel")]
    pub time_level: Option<bool>,
    /// The scroll bar under the periods.
    #[serde(default, rename = "showScrollbar")]
    pub scrollbar: Option<bool>,
}

/// The format of one element of a new slicer or timeline style (the Format of the New Slicer
/// Style dialog): colours as `#RRGGBB`.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ElementFormat {
    /// Fill colour.
    pub fill: Option<String>,
    /// Text colour.
    pub text: Option<String>,
    /// Bold text.
    pub bold: bool,
    /// Italic text.
    pub italic: bool,
    /// Colour of a thin border all around.
    pub border: Option<String>,
}

/// The element formats of a new style: the whole frame, its header, the selected and the
/// unselected items (slicer) or time blocks (timeline).
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct FrameStyleFormats {
    /// Whole slicer / timeline.
    pub whole: Option<ElementFormat>,
    /// Header.
    pub header: Option<ElementFormat>,
    /// Selected item with data / selected time block.
    pub selected: Option<ElementFormat>,
    /// Unselected item with data / unselected time block.
    pub unselected: Option<ElementFormat>,
}

/// `#RRGGBB` → colour, `None` for anything else.
fn hex_color(s: Option<&str>) -> Option<bo_model::sheet::SheetColor> {
    let h = s?.trim().trim_start_matches('#');
    (h.len() == 6 && h.chars().all(|c| c.is_ascii_hexdigit()))
        .then(|| bo_model::sheet::SheetColor::rgb(&h.to_ascii_uppercase()))
}

/// A differential format (§18.8.14 dxf: font, fill, border) of an element format; `None` when it
/// sets nothing.
fn element_dxf(f: &ElementFormat) -> Option<bo_model::sheet::Dxf> {
    use bo_model::sheet::{Border, BorderEdge, Dxf, Fill, Font, PatternFill};
    let text = hex_color(f.text.as_deref());
    let font = (text.is_some() || f.bold || f.italic).then(|| Font {
        bold: f.bold,
        italic: f.italic,
        color: text,
        ..Font::default()
    });
    // §18.8.32 patternFill: a solid differential fill keeps its colour in bgColor.
    let fill = hex_color(f.fill.as_deref()).map(|c| {
        Fill::Pattern(PatternFill {
            pattern: Some("solid".into()),
            bg: Some(c),
            ..PatternFill::default()
        })
    });
    let border = hex_color(f.border.as_deref()).map(|c| {
        let edge = BorderEdge {
            style: Some("thin".into()),
            color: Some(c),
        };
        Border {
            left: edge.clone(),
            right: edge.clone(),
            top: edge.clone(),
            bottom: edge,
            ..Border::default()
        }
    });
    (font.is_some() || fill.is_some() || border.is_some()).then(|| Dxf {
        font,
        fill,
        border,
        ..Dxf::default()
    })
}

fn clamp([r, c]: [u32; 2]) -> CellRef {
    CellRef::new(r.min(MAX_ROWS - 1), c.min(MAX_COLS - 1))
}

impl SheetEngine {
    /// `slicers`: the slicers of a sheet with their items, anchors and colours.
    pub(super) fn slicers_list(&self, sheet: usize) -> Result<Value> {
        let wb = self.wb()?;
        let ws = self.sheet(sheet)?;
        let locale = self.locale;
        let list: Vec<Value> = ws
            .slicers
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let to =
                    s.to.unwrap_or(CellRef::new(s.from.row + 10, s.from.col + 2));
                let (kind, field, pivots) = match &s.source {
                    SlicerSource::Pivot { field, pivots, .. } => {
                        ("pivot", field.clone(), connections(wb, pivots))
                    }
                    SlicerSource::Table { .. } => ("table", s.caption.clone(), Vec::new()),
                };
                json!({
                    "index": i,
                    "name": s.name,
                    "caption": s.caption,
                    "kind": kind,
                    "field": field,
                    "columns": s.columns.max(1),
                    "style": s.style,
                    "colors": bo_sheet::slicer::frame_colors(wb, s.style.as_deref()),
                    "customStyles": custom_styles(wb, false),
                    "from": [s.from.row, s.from.col],
                    "to": [to.row, to.col],
                    "items": bo_sheet::slicer::items(wb, sheet, i, &locale),
                    "pivots": pivots,
                })
            })
            .collect();
        Ok(Value::Array(list))
    }

    /// `timelines`: the timelines of a sheet with their periods, anchors and colours.
    pub(super) fn timelines_list(&self, sheet: usize) -> Result<Value> {
        let wb = self.wb()?;
        let ws = self.sheet(sheet)?;
        let vi = self.locale == bo_sheet::numfmt::Locale::VI_VN;
        let list: Vec<Value> = ws
            .timelines
            .iter()
            .enumerate()
            .map(|(i, t)| {
                let to = t.to.unwrap_or(CellRef::new(t.from.row + 7, t.from.col + 6));
                json!({
                    "index": i,
                    "name": t.name,
                    "caption": t.caption,
                    "field": t.field,
                    "level": t.level,
                    "style": t.style,
                    "colors": bo_sheet::slicer::frame_colors(wb, t.style.as_deref()),
                    "customStyles": custom_styles(wb, true),
                    "from": [t.from.row, t.from.col],
                    "to": [to.row, to.col],
                    "filtered": t.start.is_some(),
                    "showHeader": !t.hide_header,
                    "showSelectionLabel": !t.hide_selection_label,
                    "showTimeLevel": !t.hide_time_level,
                    "showScrollbar": !t.hide_scrollbar,
                    "scrollDay": bo_sheet::timeline::scroll_day(wb, t),
                    "periods": bo_sheet::timeline::periods(wb, sheet, i, vi),
                    "pivots": connections(wb, &t.pivots),
                })
            })
            .collect();
        Ok(Value::Array(list))
    }

    /// `insertPivotSlicer`: a slicer over a pivot table field. One undo step.
    pub(super) fn pivot_slicer_insert(
        &mut self,
        sheet: usize,
        pivot_sheet: Option<usize>,
        pivot: usize,
        field: u32,
        labels: Option<PivotLabels>,
    ) -> Result<Value> {
        let labels = self.pivot_labels(labels);
        let pivot_sheet = pivot_sheet.unwrap_or(sheet);
        let out = self.edit(sheet, Recalc::All, |wb, _| {
            let at = place(wb, sheet, pivot_sheet, pivot);
            let i = bo_sheet::slicer::insert_pivot_slicer(
                wb,
                sheet,
                pivot_sheet,
                pivot,
                field,
                at,
                &labels,
            )
            .map_err(bad)?;
            Ok(json!({ "index": i }))
        })?;
        self.invalidate(None);
        Ok(out)
    }

    /// `insertTimeline`: a timeline over a date field of a pivot table. One undo step.
    pub(super) fn timeline_insert(
        &mut self,
        sheet: usize,
        pivot_sheet: Option<usize>,
        pivot: usize,
        field: u32,
        labels: Option<PivotLabels>,
    ) -> Result<Value> {
        let labels = self.pivot_labels(labels);
        let pivot_sheet = pivot_sheet.unwrap_or(sheet);
        let out = self.edit(sheet, Recalc::All, |wb, _| {
            let at = place(wb, sheet, pivot_sheet, pivot);
            let i = bo_sheet::timeline::insert_timeline(
                wb,
                sheet,
                pivot_sheet,
                pivot,
                field,
                at,
                &labels,
            )
            .map_err(bad)?;
            Ok(json!({ "index": i }))
        })?;
        self.invalidate(None);
        Ok(out)
    }

    /// `slicerOptions`: caption, column count and style.
    pub(super) fn slicer_options(
        &mut self,
        sheet: usize,
        index: usize,
        caption: Option<String>,
        columns: Option<u32>,
        style: Option<String>,
    ) -> Result<Value> {
        self.edit(sheet, Recalc::No, |wb, _| {
            bo_sheet::slicer::set_options(wb, sheet, index, caption, columns, style)
                .map_err(bad)?;
            bo_xlsx::slicer::patch_slicer(wb, sheet, index);
            Ok(json!({ "ok": true }))
        })
    }

    /// `timelineOptions`: time level, caption, style and the parts shown.
    pub(super) fn timeline_options(
        &mut self,
        sheet: usize,
        index: usize,
        level: Option<u32>,
        caption: Option<String>,
        style: Option<String>,
        show: &TimelineShow,
    ) -> Result<Value> {
        self.edit(sheet, Recalc::No, |wb, _| {
            bo_sheet::timeline::set_options(wb, sheet, index, level, caption, style)
                .map_err(bad)?;
            let t = &mut wb.sheets[sheet].timelines[index];
            let set = |hide: &mut bool, show: Option<bool>| {
                if let Some(on) = show {
                    *hide = !on;
                }
            };
            set(&mut t.hide_header, show.header);
            set(&mut t.hide_selection_label, show.selection_label);
            set(&mut t.hide_time_level, show.time_level);
            set(&mut t.hide_scrollbar, show.scrollbar);
            bo_xlsx::timeline::patch_timeline(wb, sheet, index);
            Ok(json!({ "ok": true }))
        })
    }

    /// `timelineScroll`: the first day the strip shows (`scrollPosition`), saved with the file
    /// like the zoom of a sheet: no undo step.
    pub(super) fn timeline_scroll(
        &mut self,
        sheet: usize,
        index: usize,
        day: Option<f64>,
    ) -> Result<Value> {
        let wb = self.wb.as_mut().ok_or(EngineError::NoDocument)?;
        bo_sheet::timeline::set_scroll(wb, sheet, index, day).map_err(bad)?;
        bo_xlsx::timeline::patch_timeline(wb, sheet, index);
        Ok(json!({ "ok": true }))
    }

    /// `createFrameStyle`: a new custom slicer or timeline style ([MS-XLSX] `x14:slicerStyle` /
    /// `x15:timelineStyle` with its `tableStyle` and formats). One undo step; frames using it are
    /// unchanged until a style is chosen for them.
    pub(super) fn frame_style_create(
        &mut self,
        sheet: usize,
        name: &str,
        timeline: bool,
        formats: &FrameStyleFormats,
    ) -> Result<Value> {
        let parts = bo_model::sheet::FrameStyleParts {
            name: name.trim().to_owned(),
            timeline,
            whole: formats.whole.as_ref().and_then(element_dxf),
            header: formats.header.as_ref().and_then(element_dxf),
            selected: formats.selected.as_ref().and_then(element_dxf),
            unselected: formats.unselected.as_ref().and_then(element_dxf),
        };
        self.edit(sheet, Recalc::No, |wb, _| {
            bo_xlsx::add_frame_style(&mut wb.styles, &parts).map_err(bad)?;
            Ok(json!({ "name": parts.name }))
        })
    }

    /// `slicerConnect` / `timelineConnect`: the pivot tables the frame filters.
    pub(super) fn frame_connect(
        &mut self,
        sheet: usize,
        index: usize,
        timeline: bool,
        pivots: &[String],
        labels: Option<PivotLabels>,
    ) -> Result<Value> {
        let labels = self.pivot_labels(labels);
        let out = self.edit(sheet, Recalc::All, |wb, _| {
            if timeline {
                bo_sheet::timeline::connect(wb, sheet, index, pivots, &labels).map_err(bad)?;
                bo_xlsx::timeline::patch_timeline(wb, sheet, index);
            } else {
                bo_sheet::slicer::connect(wb, sheet, index, pivots, &labels).map_err(bad)?;
                bo_xlsx::slicer::patch_slicer(wb, sheet, index);
            }
            Ok(json!({ "ok": true }))
        })?;
        self.invalidate(None);
        Ok(out)
    }

    /// `moveSlicer` / `moveTimeline`: the anchor cells of the frame (§20.5.2.33).
    pub(super) fn frame_move(
        &mut self,
        sheet: usize,
        index: usize,
        timeline: bool,
        from: [u32; 2],
        to: [u32; 2],
    ) -> Result<Value> {
        let (from, to) = (clamp(from), clamp(to));
        if to.row < from.row || to.col < from.col {
            return Err(bad("bad anchor"));
        }
        self.edit(sheet, Recalc::No, |wb, _| {
            let ws = &mut wb.sheets[sheet];
            if timeline {
                let t = ws
                    .timelines
                    .get_mut(index)
                    .ok_or_else(|| bad("no such timeline"))?;
                t.from = from;
                t.to = Some(to);
            } else {
                let s = ws
                    .slicers
                    .get_mut(index)
                    .ok_or_else(|| bad("no such slicer"))?;
                s.from = from;
                s.to = Some(to);
            }
            Ok(json!({ "ok": true }))
        })
    }

    /// `deleteSlicer`: the filter is cleared first; a slicer read from the file loses its
    /// parts.
    pub(super) fn slicer_delete(&mut self, sheet: usize, index: usize) -> Result<Value> {
        let labels = self.pivot_labels(None);
        let locale = self.locale;
        let out = self.edit(sheet, Recalc::All, |wb, _| {
            if wb.sheets[sheet].slicers.get(index).is_none() {
                return Err(bad("no such slicer"));
            }
            // The filter goes with the slicer when the filtered data can still be shown.
            let mut cleared = wb.clone();
            if bo_sheet::slicer::select(&mut cleared, sheet, index, &[], &locale, &labels).is_ok() {
                *wb = cleared;
            }
            bo_xlsx::slicer::remove_slicer(wb, sheet, index);
            Ok(json!({ "ok": true }))
        })?;
        self.invalidate(None);
        Ok(out)
    }

    /// `deleteTimeline`: the date filter is cleared first; a timeline read from the file
    /// loses its parts.
    pub(super) fn timeline_delete(&mut self, sheet: usize, index: usize) -> Result<Value> {
        let labels = self.pivot_labels(None);
        let out = self.edit(sheet, Recalc::All, |wb, _| {
            if wb.sheets[sheet].timelines.get(index).is_none() {
                return Err(bad("no such timeline"));
            }
            let mut cleared = wb.clone();
            if bo_sheet::timeline::select(&mut cleared, sheet, index, None, &labels).is_ok() {
                *wb = cleared;
            }
            bo_xlsx::timeline::remove_timeline(wb, sheet, index);
            Ok(json!({ "ok": true }))
        })?;
        self.invalidate(None);
        Ok(out)
    }

    /// `pivotDrillDown`: the source records of a value cell on a new sheet.
    pub(super) fn pivot_drill_down(
        &mut self,
        sheet: usize,
        row: u32,
        col: u32,
        name: Option<String>,
        labels: Option<PivotLabels>,
    ) -> Result<Value> {
        let labels = self.pivot_labels(labels);
        let name = name
            .filter(|n| !n.trim().is_empty())
            .unwrap_or_else(|| "Detail".into());
        self.pivot_edit(sheet, |wb| {
            let at = bo_sheet::pivot::drill_down(wb, sheet, row, col, &name, &labels)?;
            let ws = &wb.sheets[at];
            let rows = ws.cells.keys().map(|k| k.0).max().unwrap_or(0);
            Ok(json!({ "sheet": at, "name": ws.name, "rows": rows }))
        })
    }

    /// `pivotFieldItems`: the hidden items of a row / column field, or the items a report
    /// filter shows, by index into the field's items (the filter buttons of a pivot chart).
    pub(super) fn pivot_field_items(
        &mut self,
        sheet: usize,
        index: usize,
        field: u32,
        hidden: &[usize],
        labels: Option<PivotLabels>,
    ) -> Result<Value> {
        let labels = self.pivot_labels(labels);
        self.pivot_edit(sheet, |wb| {
            let pt = wb
                .sheets
                .get(sheet)
                .and_then(|s| s.pivots.get(index))
                .ok_or("no such pivot table")?;
            let items: Vec<bo_model::sheet::CellValue> = wb
                .pivot_caches
                .iter()
                .find(|c| c.cache_id == pt.cache_id)
                .and_then(|c| c.fields.get(field as usize))
                .map(|f| bo_sheet::pivot::axis_items(f).to_vec())
                .ok_or("unknown pivot field")?;
            let mut spec = bo_sheet::pivot::PivotSpec::of(pt);
            if let Some(page) = spec.filters.iter_mut().find(|f| f.field == field) {
                page.selected = if hidden.is_empty() {
                    Vec::new()
                } else {
                    items
                        .iter()
                        .enumerate()
                        .filter(|(i, _)| !hidden.contains(i))
                        .map(|(_, v)| v.clone())
                        .collect()
                };
                if !hidden.is_empty() && page.selected.is_empty() {
                    return Err("keep at least one item".into());
                }
            } else {
                let fi = field as usize;
                if spec.fields.len() <= fi {
                    spec.fields
                        .resize(fi + 1, bo_model::sheet::PivotFieldOptions::default());
                }
                spec.fields[fi].hidden = hidden
                    .iter()
                    .filter_map(|i| items.get(*i).cloned())
                    .collect();
                if spec.fields[fi].hidden.len() >= items.len() && !items.is_empty() {
                    return Err("keep at least one item".into());
                }
            }
            bo_sheet::pivot::update_pivot(wb, sheet, index, &spec, &labels)?;
            Ok(json!({ "ok": true }))
        })
    }
}
