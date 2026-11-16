//! Sort and filter extras (SHEET-DATA-001): the sort settings remembered with an
//! AutoFilter (ECMA-376 Part 1, §18.3.1.92 `sortState`, §18.3.1.91 `sortCondition`),
//! the Advanced Filter (`bo_sheet::advfilter`) and "show all data".

use super::cmd::Rng;
use super::SheetEngine;
use crate::{EngineError, Result};
use bo_model::sheet::{
    AutoFilter, CellRange, CellRef, Dxf, Fill, Font, PatternFill, SheetColor, SortCondition,
    SortState, Workbook,
};
use bo_sheet::sort::{SortBy, SortKey};
use serde_json::{json, Value};

fn bad(msg: impl Into<String>) -> EngineError {
    EngineError::Sheet(msg.into())
}

/// Index of a differential format holding `rgb` as a fill (`cell`) or font colour,
/// added when missing (§18.8.14 `dxf`; solid differential fills carry the colour
/// in `bgColor`, §18.8.32 `patternFill`).
pub(super) fn color_dxf(wb: &mut Workbook, rgb: (u8, u8, u8), cell: bool) -> u32 {
    let c = SheetColor::rgb(&format!("{:02X}{:02X}{:02X}", rgb.0, rgb.1, rgb.2));
    let dxf = if cell {
        Dxf {
            fill: Some(Fill::Pattern(PatternFill {
                pattern: Some("solid".into()),
                fg: None,
                bg: Some(c),
            })),
            ..Dxf::default()
        }
    } else {
        Dxf {
            font: Some(Font {
                color: Some(c),
                ..Font::default()
            }),
            ..Dxf::default()
        }
    };
    let dxfs = &mut wb.styles.dxfs;
    let i = dxfs.iter().position(|d| *d == dxf).unwrap_or_else(|| {
        dxfs.push(dxf);
        dxfs.len() - 1
    });
    i as u32
}

/// Remember the sort levels with the sheet's AutoFilter when the sorted range is its
/// data (§18.3.1.92 `sortState` inside §18.3.1.2 `autoFilter`), replacing any earlier
/// state. Sorts elsewhere are not remembered.
pub(super) fn record_sort_state(
    wb: &mut Workbook,
    sheet: usize,
    range: CellRange,
    keys: &[SortKey],
    has_header: bool,
    left_to_right: bool,
) {
    let Some(af) = wb.sheets.get(sheet).and_then(|ws| ws.auto_filter.as_ref()) else {
        return;
    };
    if left_to_right || af.range.first.col != range.first.col || af.range.last.col != range.last.col
    {
        return;
    }
    let first = range.first.row + u32::from(has_header);
    if first != af.range.first.row + 1 || first > range.last.row {
        return;
    }
    let data = CellRange::new(CellRef::new(first, range.first.col), range.last);
    let mut conditions = Vec::new();
    for k in keys {
        let col_range = CellRange::new(
            CellRef::new(first, k.col),
            CellRef::new(range.last.row, k.col),
        );
        let mut c = SortCondition {
            descending: k.descending,
            range: col_range,
            custom_list: k.custom_list.as_ref().map(|l| l.join(",")),
            ..SortCondition::default()
        };
        match &k.by {
            SortBy::Value => {}
            SortBy::CellColor(rgb) => {
                c.sort_by = Some("cellColor".into());
                c.dxf_id = Some(color_dxf(wb, *rgb, true));
            }
            SortBy::FontColor(rgb) => {
                c.sort_by = Some("fontColor".into());
                c.dxf_id = Some(color_dxf(wb, *rgb, false));
            }
            SortBy::Icon { set, id } => {
                c.sort_by = Some("icon".into());
                c.icon_set = Some(set.clone());
                c.icon_id = Some(*id);
            }
        }
        conditions.push(c);
    }
    let case_sensitive = keys.iter().any(|k| k.case_sensitive);
    if let Some(af) = wb.sheets[sheet].auto_filter.as_mut() {
        // A raw (unmodelled) state is superseded by the new one.
        af.unknown.retain(|u| !u.xml.starts_with("<sortState"));
        af.sort_state = Some(SortState {
            range: data,
            column_sort: false,
            case_sensitive,
            sort_method: None,
            conditions,
        });
    }
}

impl SheetEngine {
    /// Advanced filter on `sheet` (one undo step). Returns `{ matched, hidden, extract }`.
    pub(super) fn advanced_filter(
        &mut self,
        sheet: usize,
        list: Rng,
        criteria_sheet: Option<usize>,
        criteria: Rng,
        copy_to: Option<Rng>,
        unique: bool,
    ) -> Result<Value> {
        let locale = self.locale;
        let now = self.live.now();
        let criteria_sheet = criteria_sheet.unwrap_or(sheet);
        self.sheet(criteria_sheet)?;
        // A single selected cell stands for its current region.
        let list = {
            let r = list.range();
            if r.first == r.last {
                super::cmd::current_region(self.sheet(sheet)?, r)
            } else {
                r
            }
        };
        let spec = bo_sheet::advfilter::AdvancedFilter {
            list,
            criteria_sheet,
            criteria: criteria.range(),
            copy_to: copy_to.map(Rng::range),
            unique,
        };
        // Light undo step (SHEET-PERF-002): filtering in place changes hidden rows and
        // names only; copying writes below-right of the destination and clears the
        // previous extract (the sheet-scoped name `Extract`, §18.2.5).
        let mut areas = Vec::new();
        if let Some(dest) = spec.copy_to {
            areas.push((sheet, super::light_undo::quadrant(dest.first)));
            let wb = self.wb()?;
            let old = wb
                .defined_names
                .iter()
                .find(|d| {
                    d.local_sheet == Some(sheet as u32) && d.name.eq_ignore_ascii_case("Extract")
                })
                .and_then(|d| d.formula.rsplit('!').next())
                .and_then(|body| CellRange::parse(&body.replace('$', "")));
            if let Some(o) = old {
                areas.push((sheet, o));
            }
        }
        self.edit_light(sheet, &areas, |wb, _, fx| {
            fx.hidden.push(sheet);
            let out = bo_sheet::advfilter::apply(wb, sheet, &spec, &locale, now)
                .map_err(|e| bad(e.to_string()))?;
            Ok(json!({
                "matched": out.matched,
                "hidden": out.hidden,
                "list": Rng::of(list),
                "extract": out.extract.map(Rng::of),
            }))
        })
    }

    /// Show every row a filter hides on `sheet` (one undo step). Returns `{ shown }`.
    pub(super) fn show_all_data(&mut self, sheet: usize) -> Result<Value> {
        self.edit_light(sheet, &[], |wb, _, fx| {
            fx.hidden.push(sheet);
            let shown = clear_table_filters(wb, sheet) + bo_sheet::advfilter::show_all(wb, sheet);
            Ok(json!({ "shown": shown }))
        })
    }

    /// Built-in lists offered as custom sort orders.
    pub(super) fn sort_lists() -> Value {
        Value::from(
            bo_sheet::fill::BUILTIN_LISTS
                .iter()
                .map(|l| Value::from(l.iter().map(|s| Value::from(*s)).collect::<Vec<_>>()))
                .collect::<Vec<_>>(),
        )
    }
}

/// The data rows (header excluded, totals row excluded) a table's filter covers, header
/// row included in the range as `autoFilter/@ref` has it (§18.3.1.2).
fn table_filter_range(t: &bo_model::sheet::Table) -> CellRange {
    let last = t.range.last.row - u32::from(bo_sheet::table::has_totals(t));
    CellRange::new(t.range.first, CellRef::new(last, t.range.last.col))
}

/// The filter that column `col` belongs to: the table at `row` when the table shows
/// filter buttons (§18.5.1.2 `table/autoFilter`, parsed from the part's XML), else the
/// sheet's AutoFilter. `(Some(table index), filter)` for a table.
pub(super) fn filter_of(
    ws: &bo_model::sheet::Worksheet,
    col: u32,
    row: Option<u32>,
) -> Result<(Option<usize>, AutoFilter)> {
    if let Some(row) = row {
        let at = CellRef::new(row, col);
        if let Some((ti, t)) = ws.tables.iter().enumerate().find(|(_, t)| {
            t.auto_filter.is_some() && bo_sheet::table::has_header(t) && t.range.contains(at)
        }) {
            let mut af = t
                .auto_filter
                .as_deref()
                .and_then(bo_xlsx::parse_auto_filter_xml)
                .unwrap_or_default();
            // The table's own extent wins over a stale `ref`.
            af.range = table_filter_range(t);
            return Ok((Some(ti), af));
        }
    }
    let af = ws.auto_filter.clone().ok_or_else(|| bad("no AutoFilter"))?;
    if col < af.range.first.col || col > af.range.last.col {
        return Err(bad("column outside the AutoFilter"));
    }
    Ok((None, af))
}

/// Store `af` as table `ti`'s filter (written back into the table part's `autoFilter`,
/// §18.5.1.2) and hide the data rows it leaves out. Returns the rows hidden.
pub(super) fn apply_table_filter(
    wb: &mut Workbook,
    sheet: usize,
    ti: usize,
    af: &AutoFilter,
    locale: &bo_sheet::numfmt::Locale,
    now: Option<f64>,
) -> usize {
    let hidden = bo_sheet::filter::hidden_rows_at(wb, &wb.sheets[sheet], af, locale, now);
    let ws = &mut wb.sheets[sheet];
    for r in af.range.first.row + 1..=af.range.last.row {
        let hide = hidden.binary_search(&r).is_ok();
        let info = ws.rows.entry(r).or_default();
        info.hidden = hide;
        if *info == bo_model::sheet::RowInfo::default() {
            ws.rows.remove(&r);
        }
    }
    if let Some(t) = ws.tables.get_mut(ti) {
        t.auto_filter = Some(bo_xlsx::auto_filter_xml(af));
    }
    hidden.len()
}

/// Clear the criteria of every table filter of `sheet` (Data › Clear) and show the rows
/// they hid. Returns the rows shown.
fn clear_table_filters(wb: &mut Workbook, sheet: usize) -> usize {
    let mut shown = 0;
    let ws = &mut wb.sheets[sheet];
    for ti in 0..ws.tables.len() {
        let t = &ws.tables[ti];
        let Some(mut af) = t
            .auto_filter
            .as_deref()
            .and_then(bo_xlsx::parse_auto_filter_xml)
        else {
            continue;
        };
        if af
            .columns
            .iter()
            .all(|c| matches!(c.filter, bo_model::sheet::ColumnFilter::None))
        {
            continue;
        }
        af.range = table_filter_range(t);
        // Columns whose button is hidden stay (without a criterion).
        af.columns.retain(|c| c.hidden_button);
        for c in &mut af.columns {
            c.filter = bo_model::sheet::ColumnFilter::None;
        }
        for r in af.range.first.row + 1..=af.range.last.row {
            if let Some(info) = ws.rows.get_mut(&r) {
                if info.hidden {
                    info.hidden = false;
                    shown += 1;
                }
                if *info == bo_model::sheet::RowInfo::default() {
                    ws.rows.remove(&r);
                }
            }
        }
        ws.tables[ti].auto_filter = Some(bo_xlsx::auto_filter_xml(&af));
    }
    shown
}

#[cfg(test)]
#[path = "sortfilter_cmd_tests.rs"]
mod tests;
