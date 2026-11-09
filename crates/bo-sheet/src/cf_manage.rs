//! Conditional formatting rules manager: list the rules of a sheet in priority
//! order (ECMA-376 Part 1, §18.3.1.10 `cfRule/@priority`, lower first), move a rule
//! up / down, toggle `stopIfTrue`, change its range, replace or delete it; and rules
//! scoped to the values of a pivot table (§18.3.1.18 `conditionalFormatting/@pivot`,
//! §18.10.1.15 pivot `conditionalFormat` with `scope="data"`).

use crate::cf::{
    build_rule, CfIconSpec, CfRuleSpec, CfStyle, CfvoSpec, DataBarOptions, IconSetOptions,
};
use bo_model::sheet::{
    sqref_to_string, CellRange, CellRef, CfRule, Cfvo, ConditionalFormat, Dxf, Fill,
    PivotConditionalFormat, SheetColor, Workbook, Worksheet,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// One rule as listed by the manager.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CfRuleInfo {
    /// Index of the `conditionalFormatting` block.
    pub block: usize,
    /// Index of the rule in its block.
    pub rule: usize,
    /// `priority` (1 = evaluated first).
    pub priority: i32,
    /// `type` (§18.18.12).
    pub kind: String,
    /// `sqref` of the block, e.g. `A1:A10 C1:C10`.
    pub range: String,
    /// `stopIfTrue`.
    pub stop_if_true: bool,
    /// The block belongs to a pivot table.
    pub pivot: bool,
    /// The rule is stored in the x14 extension only.
    pub ext: bool,
    /// Editable form of the rule (`None` for kinds the editor does not offer).
    pub spec: Option<CfRuleSpec>,
}

/// A change requested by the rules manager.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum CfOp {
    /// Evaluate the rule one step earlier.
    MoveUp,
    /// Evaluate the rule one step later.
    MoveDown,
    /// Remove the rule (and its block when it was the last one).
    Delete,
    /// Set `stopIfTrue`.
    StopIfTrue {
        /// New value.
        value: bool,
    },
    /// Change the cells of the rule's block (`sqref`, space / comma separated).
    SetRange {
        /// The new ranges, e.g. `A1:A10 C1:C10`.
        range: String,
    },
    /// Replace the rule with a new definition (priority and stopIfTrue are kept).
    Replace {
        /// The new rule.
        rule: Box<CfRuleSpec>,
    },
}

/// Rules of `ws` as (block, rule) positions, in evaluation order.
fn ordered(ws: &Worksheet) -> Vec<(usize, usize)> {
    let mut v: Vec<(usize, usize)> = ws
        .conditional_formats
        .iter()
        .enumerate()
        .flat_map(|(b, cf)| (0..cf.rules.len()).map(move |r| (b, r)))
        .collect();
    v.sort_by_key(|&(b, r)| ws.conditional_formats[b].rules[r].priority);
    v
}

/// Renumbers priorities 1..n in evaluation order; pivot `conditionalFormat`
/// priorities follow their rules, entries without a rule are dropped.
pub fn normalize(ws: &mut Worksheet) {
    let order = ordered(ws);
    let mut map: HashMap<i32, i32> = HashMap::new();
    for (i, &(b, r)) in order.iter().enumerate() {
        let new = i32::try_from(i + 1).unwrap_or(i32::MAX);
        let rule = &mut ws.conditional_formats[b].rules[r];
        map.entry(rule.priority).or_insert(new);
        rule.priority = new;
    }
    for pt in &mut ws.pivots {
        let before = pt.conditional_formats.clone();
        pt.conditional_formats
            .retain_mut(|c| match map.get(&c.priority) {
                Some(&p) => {
                    c.priority = p;
                    true
                }
                None => false,
            });
        if pt.conditional_formats != before {
            pt.dirty = true;
        }
    }
}

fn hex_of(c: Option<&SheetColor>) -> Option<String> {
    c.and_then(|c| c.rgb.as_deref())
        .map(|h| h.get(h.len().saturating_sub(6)..).unwrap_or(h).to_owned())
}

fn style_of(wb: &Workbook, r: &CfRule) -> CfStyle {
    let Some(d): Option<&Dxf> = r.dxf_id.and_then(|i| wb.styles.dxfs.get(i as usize)) else {
        return CfStyle::default();
    };
    let fill = match &d.fill {
        Some(Fill::Pattern(p)) => hex_of(p.bg.as_ref().or(p.fg.as_ref())),
        _ => None,
    };
    CfStyle {
        fill,
        font_color: d.font.as_ref().and_then(|f| hex_of(f.color.as_ref())),
        bold: d.font.as_ref().is_some_and(|f| f.bold),
        italic: d.font.as_ref().is_some_and(|f| f.italic),
    }
}

fn cfvo_spec(c: &Cfvo) -> CfvoSpec {
    CfvoSpec {
        kind: c.kind.clone(),
        value: c.val.clone(),
        gte: c.gte != Some(false),
    }
}

fn attr<'a>(r: &'a CfRule, name: &str) -> Option<&'a str> {
    r.attrs
        .iter()
        .find(|(k, _)| k == name)
        .map(|(_, v)| v.as_str())
}

/// Editable form of a rule, when the editor offers its kind.
#[must_use]
pub fn spec_of(wb: &Workbook, r: &CfRule) -> Option<CfRuleSpec> {
    let style = || style_of(wb, r);
    Some(match r.kind.as_str() {
        "cellIs" => CfRuleSpec::CellIs {
            operator: r.operator.clone().unwrap_or_else(|| "equal".into()),
            formula1: r.formulas.first().cloned().unwrap_or_default(),
            formula2: r.formulas.get(1).cloned(),
            style: style(),
        },
        "expression" => CfRuleSpec::Expression {
            formula: r.formulas.first().cloned().unwrap_or_default(),
            style: style(),
        },
        "containsText" => CfRuleSpec::ContainsText {
            text: r.text.clone().unwrap_or_default(),
            style: style(),
        },
        "top10" => CfRuleSpec::Top10 {
            rank: attr(r, "rank").and_then(|v| v.parse().ok()).unwrap_or(10),
            bottom: attr(r, "bottom") == Some("1"),
            percent: attr(r, "percent") == Some("1"),
            style: style(),
        },
        "duplicateValues" => CfRuleSpec::DuplicateValues { style: style() },
        "colorScale" => {
            let cs = r.color_scale.as_ref()?;
            let hex = |i: usize| hex_of(cs.colors.get(i)).unwrap_or_else(|| "FFFFFF".into());
            CfRuleSpec::ColorScale {
                min_color: hex(0),
                mid_color: (cs.colors.len() > 2).then(|| hex(1)),
                max_color: hex(cs.colors.len().saturating_sub(1)),
            }
        }
        "dataBar" => {
            let db = r.data_bar.as_ref()?;
            let x = db.x14.clone().unwrap_or_default();
            let cfvos = if x.cfvos.len() == 2 {
                &x.cfvos
            } else {
                &db.cfvos
            };
            let color = hex_of(x.fill_color.as_ref().or(db.color.as_ref()))
                .unwrap_or_else(|| "4A7BB7".into());
            let has_ext = db.x14.is_some();
            CfRuleSpec::DataBar {
                color,
                options: Some(DataBarOptions {
                    min: cfvos.first().map_or_else(
                        || {
                            cfvo_spec(&Cfvo {
                                kind: "min".into(),
                                ..Cfvo::default()
                            })
                        },
                        cfvo_spec,
                    ),
                    max: cfvos.get(1).map_or_else(
                        || {
                            cfvo_spec(&Cfvo {
                                kind: "max".into(),
                                ..Cfvo::default()
                            })
                        },
                        cfvo_spec,
                    ),
                    min_length: x.min_length.or(db.min_length).unwrap_or(10),
                    max_length: x.max_length.or(db.max_length).unwrap_or(90),
                    gradient: x.gradient != Some(false),
                    border: x.border,
                    border_color: hex_of(x.border_color.as_ref()),
                    negative_color: hex_of(x.negative_fill_color.as_ref()),
                    negative_border_color: hex_of(x.negative_border_color.as_ref()),
                    negative_same_color: x.negative_same_color,
                    negative_same_border: x.negative_same_border != Some(false),
                    axis_position: if has_ext {
                        x.axis_position
                            .clone()
                            .unwrap_or_else(|| "automatic".into())
                    } else {
                        "none".into()
                    },
                    axis_color: hex_of(x.axis_color.as_ref()),
                    direction: x.direction.clone().unwrap_or_else(|| "context".into()),
                    show_value: db.show_value != Some(false),
                }),
            }
        }
        "iconSet" => {
            let is = r.icon_set.as_ref()?;
            CfRuleSpec::IconSet {
                set: is
                    .icon_set
                    .clone()
                    .unwrap_or_else(|| "3TrafficLights1".into()),
                reverse: is.reverse == Some(true),
                options: Some(IconSetOptions {
                    thresholds: is.cfvos.iter().skip(1).map(cfvo_spec).collect(),
                    show_value: is.show_value != Some(false),
                    icons: if is.custom {
                        is.icons
                            .iter()
                            .map(|i| CfIconSpec {
                                set: i.set.clone(),
                                id: i.id,
                            })
                            .collect()
                    } else {
                        Vec::new()
                    },
                }),
            }
        }
        _ => return None,
    })
}

/// Rules of sheet `sheet` in evaluation order.
#[must_use]
pub fn list(wb: &Workbook, sheet: usize) -> Vec<CfRuleInfo> {
    let Some(ws) = wb.sheets.get(sheet) else {
        return Vec::new();
    };
    ordered(ws)
        .into_iter()
        .map(|(b, r)| {
            let cf = &ws.conditional_formats[b];
            let rule = &cf.rules[r];
            CfRuleInfo {
                block: b,
                rule: r,
                priority: rule.priority,
                kind: rule.kind.clone(),
                range: sqref_to_string(&cf.sqref),
                stop_if_true: rule.stop_if_true,
                pivot: cf.pivot,
                ext: cf.ext || rule.needs_ext(),
                spec: spec_of(wb, rule),
            }
        })
        .collect()
}

/// Parses `A1:B2 C3,D4:D9` (a sheet prefix and `$` are ignored).
fn parse_sqref(s: &str) -> Option<Vec<CellRange>> {
    let v: Option<Vec<CellRange>> = s
        .split(|c: char| c == ',' || c == ';' || c.is_whitespace())
        .filter(|t| !t.is_empty())
        .map(|t| {
            let t = t.rsplit_once('!').map_or(t, |(_, r)| r).replace('$', "");
            CellRange::parse(&t).or_else(|| CellRef::parse(&t).map(CellRange::single))
        })
        .collect();
    v.filter(|v| !v.is_empty())
}

/// Applies `op` to rule `rule` of block `block` on sheet `sheet`.
///
/// # Errors
/// A message when the sheet, block or rule does not exist, or the new range / rule is invalid.
pub fn apply(
    wb: &mut Workbook,
    sheet: usize,
    block: usize,
    rule: usize,
    op: &CfOp,
) -> Result<(), String> {
    let exists = wb
        .sheets
        .get(sheet)
        .and_then(|ws| ws.conditional_formats.get(block))
        .is_some_and(|cf| rule < cf.rules.len());
    if !exists {
        return Err("no such conditional formatting rule".into());
    }
    normalize(&mut wb.sheets[sheet]);
    match op {
        CfOp::MoveUp | CfOp::MoveDown => {
            let ws = &mut wb.sheets[sheet];
            let order = ordered(ws);
            let i = order
                .iter()
                .position(|&p| p == (block, rule))
                .ok_or("no such rule")?;
            let j = if matches!(op, CfOp::MoveUp) {
                i.checked_sub(1)
            } else {
                (i + 1 < order.len()).then_some(i + 1)
            };
            if let Some(j) = j {
                let (a, b) = (order[i], order[j]);
                let pa = ws.conditional_formats[a.0].rules[a.1].priority;
                let pb = ws.conditional_formats[b.0].rules[b.1].priority;
                ws.conditional_formats[a.0].rules[a.1].priority = pb;
                ws.conditional_formats[b.0].rules[b.1].priority = pa;
                for pt in &mut ws.pivots {
                    for c in &mut pt.conditional_formats {
                        if c.priority == pa {
                            c.priority = pb;
                            pt.dirty = true;
                        } else if c.priority == pb {
                            c.priority = pa;
                            pt.dirty = true;
                        }
                    }
                }
            }
        }
        CfOp::Delete => {
            let ws = &mut wb.sheets[sheet];
            ws.conditional_formats[block].rules.remove(rule);
            if ws.conditional_formats[block].rules.is_empty() {
                ws.conditional_formats.remove(block);
            }
            normalize(ws);
        }
        CfOp::StopIfTrue { value } => {
            wb.sheets[sheet].conditional_formats[block].rules[rule].stop_if_true = *value;
        }
        CfOp::SetRange { range } => {
            let sq = parse_sqref(range).ok_or("invalid range")?;
            wb.sheets[sheet].conditional_formats[block].sqref = sq;
        }
        CfOp::Replace { rule: spec } => {
            let first = wb.sheets[sheet].conditional_formats[block]
                .sqref
                .first()
                .copied()
                .unwrap_or_default();
            let mut new = build_rule(wb, first, spec).ok_or("invalid rule")?;
            let ws = &mut wb.sheets[sheet];
            let cf = &mut ws.conditional_formats[block];
            let old = &cf.rules[rule];
            new.priority = old.priority;
            new.stop_if_true = old.stop_if_true;
            new.ext_id.clone_from(&old.ext_id);
            let ext = new.needs_ext();
            if ext == cf.ext || cf.rules.len() == 1 {
                cf.ext = ext;
                cf.rules[rule] = new;
            } else {
                // The rule moves to its own block stored in the other schema.
                cf.rules.remove(rule);
                let moved = ConditionalFormat {
                    sqref: cf.sqref.clone(),
                    pivot: cf.pivot,
                    rules: vec![new],
                    ext,
                };
                ws.conditional_formats.push(moved);
            }
        }
    }
    Ok(())
}

/// Values area of a pivot table: the data rows and columns of its report, without
/// the grand total row / column.
#[must_use]
pub fn pivot_values_area(pt: &bo_model::sheet::PivotTable) -> Option<CellRange> {
    let loc = pt.location;
    let r0 = loc.first.row + pt.first_data_row;
    let c0 = loc.first.col + pt.first_data_col;
    let mut r1 = loc.last.row;
    let mut c1 = loc.last.col;
    if pt.col_grand_totals && !pt.rows.is_empty() && r1 > r0 {
        r1 -= 1;
    }
    if pt.row_grand_totals && !pt.cols.is_empty() && c1 > c0 {
        c1 -= 1;
    }
    (r0 <= r1 && c0 <= c1).then(|| CellRange::new(CellRef::new(r0, c0), CellRef::new(r1, c1)))
}

/// `pivotArea` (§18.3.1.68) selecting the values of data field `field`
/// (a `reference` to the values pseudo-field, `field` = 4294967294, i.e. −2).
fn data_area_xml(field: usize) -> String {
    format!(
        "<pivotArea outline=\"0\" collapsedLevelsAreSubtotals=\"1\" fieldPosition=\"0\"><references count=\"1\"><reference field=\"4294967294\" count=\"1\" selected=\"0\"><x v=\"{field}\"/></reference></references></pivotArea>"
    )
}

/// Adds `spec` to the values of the pivot table covering (`row`, `col`): a
/// `conditionalFormatting pivot="1"` block over the values area and a pivot
/// `conditionalFormat scope="data"` with the same priority.
///
/// # Errors
/// A message when no pivot table covers the cell or the rule is invalid.
pub fn add_pivot_rule(
    wb: &mut Workbook,
    sheet: usize,
    row: u32,
    col: u32,
    spec: &CfRuleSpec,
) -> Result<CellRange, String> {
    let ws = wb.sheets.get(sheet).ok_or("no such sheet")?;
    let index = crate::pivot::pivot_at(ws, row, col).ok_or("the cell is not in a pivot table")?;
    let pt = &ws.pivots[index];
    let area = pivot_values_area(pt).ok_or("the pivot table has no values")?;
    let fields = pt.values.len().max(1);
    crate::cf::add_rule_in(wb, sheet, vec![area], spec, true).ok_or("invalid rule")?;
    let pt = &mut wb.sheets[sheet].pivots[index];
    pt.conditional_formats.push(PivotConditionalFormat {
        priority: 1,
        scope: Some("data".into()),
        kind: None,
        areas: (0..fields).map(data_area_xml).collect(),
    });
    pt.dirty = true;
    Ok(area)
}

/// After pivot `index` was laid out again, moves its value-scoped pivot rules to the
/// new values area.
pub fn sync_pivot(ws: &mut Worksheet, index: usize) {
    let Some(pt) = ws.pivots.get(index) else {
        return;
    };
    let Some(area) = pivot_values_area(pt) else {
        return;
    };
    let prios: Vec<i32> = pt
        .conditional_formats
        .iter()
        .filter(|c| c.scope.as_deref() == Some("data"))
        .map(|c| c.priority)
        .collect();
    for cf in &mut ws.conditional_formats {
        if cf.pivot && cf.rules.iter().any(|r| prios.contains(&r.priority)) {
            cf.sqref = vec![area];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cf::{add_rule, evaluate};
    use bo_model::sheet::{Cell, CellValue};

    fn rng(s: &str) -> CellRange {
        CellRange::parse(s).unwrap()
    }

    fn style(fill: &str) -> CfStyle {
        CfStyle {
            fill: Some(fill.into()),
            ..CfStyle::default()
        }
    }

    #[test]
    fn order_move_stop_and_delete() {
        let mut wb = Workbook::new();
        for r in 0..4 {
            wb.sheets[0].set_cell(r, 0, Cell::new(CellValue::Number(f64::from(r + 1) * 10.0)));
        }
        // Added first → ends at priority 2; added second → priority 1.
        add_rule(
            &mut wb,
            0,
            rng("A1:A4"),
            &CfRuleSpec::CellIs {
                operator: "greaterThan".into(),
                formula1: "15".into(),
                formula2: None,
                style: style("FF0000"),
            },
        );
        add_rule(
            &mut wb,
            0,
            rng("A1:A4"),
            &CfRuleSpec::CellIs {
                operator: "greaterThan".into(),
                formula1: "25".into(),
                formula2: None,
                style: style("00FF00"),
            },
        );
        let l = list(&wb, 0);
        assert_eq!(l.iter().map(|i| i.priority).collect::<Vec<_>>(), vec![1, 2]);
        assert_eq!((l[0].block, l[1].block), (1, 0));
        // A3 = 30 matches both: priority 1 (green) wins the fill.
        let m = evaluate(&wb, 0, 0..4, 0..1, None);
        assert_eq!(m[&(2, 0)].fill, Some(bo_render::Rgba(0, 255, 0, 255)));
        // Move the red rule up: now it wins on A3.
        apply(&mut wb, 0, 0, 0, &CfOp::MoveUp).unwrap();
        let m = evaluate(&wb, 0, 0..4, 0..1, None);
        assert_eq!(m[&(2, 0)].fill, Some(bo_render::Rgba(255, 0, 0, 255)));
        assert_eq!(list(&wb, 0)[0].block, 0);
        // Stop If True on the red rule (now first) also hides the green rule's other
        // properties — both only fill, so check with a bold green rule instead.
        apply(&mut wb, 0, 0, 0, &CfOp::StopIfTrue { value: true }).unwrap();
        assert!(list(&wb, 0)[0].stop_if_true);
        apply(&mut wb, 0, 0, 0, &CfOp::Delete).unwrap();
        let l = list(&wb, 0);
        assert_eq!(l.len(), 1);
        assert_eq!(l[0].priority, 1);
        assert!(apply(&mut wb, 0, 5, 0, &CfOp::Delete).is_err());
        apply(
            &mut wb,
            0,
            0,
            0,
            &CfOp::SetRange {
                range: "$A$1:$A$2, C1".into(),
            },
        )
        .unwrap();
        assert_eq!(list(&wb, 0)[0].range, "A1:A2 C1");
    }

    #[test]
    fn pivot_values_rule_follows_refresh() {
        use crate::pivot::{create_pivot, refresh_pivot, PivotLabels, PivotSpec};
        use bo_model::sheet::{PivotDataField, PivotFunction};
        let mut wb = Workbook::new();
        let rows: [(&str, f64); 4] = [("Bắc", 10.0), ("Nam", 5.0), ("Trung", 2.0), ("Bắc", 3.0)];
        let ws = &mut wb.sheets[0];
        ws.set_cell(0, 0, Cell::new(CellValue::Text("Vùng".into())));
        ws.set_cell(0, 1, Cell::new(CellValue::Text("Doanh thu".into())));
        for (i, (a, v)) in rows.iter().enumerate() {
            let r = i as u32 + 1;
            ws.set_cell(r, 0, Cell::new(CellValue::Text((*a).into())));
            ws.set_cell(r, 1, Cell::new(CellValue::Number(*v)));
        }
        wb.sheets.push(Worksheet::new("Pivot"));
        let spec = PivotSpec {
            rows: vec![0],
            values: vec![PivotDataField {
                field: 1,
                function: PivotFunction::Sum,
                ..PivotDataField::default()
            }],
            ..PivotSpec::default()
        };
        let labels = PivotLabels::default();
        create_pivot(
            &mut wb,
            0,
            rng("A1:B5"),
            1,
            CellRef::new(2, 0),
            &spec,
            &labels,
        )
        .unwrap();
        // Report A3:B7 (header, Bắc, Nam, Trung, grand total): values B4:B6.
        let area = add_pivot_rule(
            &mut wb,
            1,
            3,
            1,
            &CfRuleSpec::DataBar {
                color: "4A7BB7".into(),
                options: None,
            },
        )
        .unwrap();
        assert_eq!(area.to_string(), "B4:B6");
        let ws = &wb.sheets[1];
        assert!(ws.conditional_formats[0].pivot);
        let pcf = &ws.pivots[0].conditional_formats;
        assert_eq!((pcf.len(), pcf[0].priority), (1, 1));
        assert_eq!(pcf[0].scope.as_deref(), Some("data"));
        assert!(add_pivot_rule(
            &mut wb,
            1,
            20,
            20,
            &CfRuleSpec::DuplicateValues {
                style: CfStyle::default()
            }
        )
        .is_err());
        // "Trung" becomes "Nam": two items, values B4:B5 after the refresh.
        wb.sheets[0].set_cell(3, 0, Cell::new(CellValue::Text("Nam".into())));
        refresh_pivot(&mut wb, 1, 0, &labels).unwrap();
        assert_eq!(
            sqref_to_string(&wb.sheets[1].conditional_formats[0].sqref),
            "B4:B5"
        );
        // Deleting the rule drops the pivot's conditionalFormat too.
        apply(&mut wb, 1, 0, 0, &CfOp::Delete).unwrap();
        assert_eq!(wb.sheets[1].pivots[0].conditional_formats.len(), 0);
    }

    #[test]
    fn replace_moves_between_schemas() {
        let mut wb = Workbook::new();
        add_rule(
            &mut wb,
            0,
            rng("A1:A4"),
            &CfRuleSpec::IconSet {
                set: "3Arrows".into(),
                reverse: false,
                options: None,
            },
        );
        assert!(!wb.sheets[0].conditional_formats[0].ext);
        let spec = CfRuleSpec::IconSet {
            set: "3Stars".into(),
            reverse: false,
            options: None,
        };
        apply(
            &mut wb,
            0,
            0,
            0,
            &CfOp::Replace {
                rule: Box::new(spec.clone()),
            },
        )
        .unwrap();
        assert!(wb.sheets[0].conditional_formats[0].ext);
        let l = list(&wb, 0);
        assert!(l[0].ext);
        // The listed spec round-trips: equal percent bands 0 / 33 / 66.
        let Some(CfRuleSpec::IconSet {
            set,
            options: Some(o),
            ..
        }) = &l[0].spec
        else {
            panic!("icon set spec");
        };
        assert_eq!(set, "3Stars");
        let vals: Vec<_> = o
            .thresholds
            .iter()
            .map(|t| t.value.clone().unwrap())
            .collect();
        assert_eq!(vals, vec!["33", "66"]);
    }
}
