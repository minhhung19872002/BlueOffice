//! Sort and filter extras (SHEET-DATA-001): the sort settings remembered with an
//! AutoFilter (ECMA-376 Part 1, §18.3.1.92 `sortState`, §18.3.1.91 `sortCondition`),
//! the Advanced Filter (`bo_sheet::advfilter`) and "show all data".

use super::cmd::{Recalc, Rng};
use super::SheetEngine;
use crate::{EngineError, Result};
use bo_model::sheet::{
    CellRange, CellRef, Dxf, Fill, Font, PatternFill, SheetColor, SortCondition, SortState,
    Workbook,
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
        self.edit(sheet, Recalc::All, |wb, _| {
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
        self.edit(sheet, Recalc::HiddenRows(sheet), |wb, _| {
            let shown = bo_sheet::advfilter::show_all(wb, sheet);
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

#[cfg(test)]
#[path = "sortfilter_cmd_tests.rs"]
mod tests;
