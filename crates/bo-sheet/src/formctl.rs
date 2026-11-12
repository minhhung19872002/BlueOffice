//! Form controls at work ([`bo_model::sheet::FormControl`], read from [MS-XLSX]
//! `formControlPr` / VML `x:ClientData`).
//!
//! * A check box writes TRUE / FALSE (`#N/A` when mixed) to its linked cell (`fmlaLink`).
//! * Option buttons form groups — the buttons inside the same group box, or all the sheet's
//!   buttons outside any group box — and write the 1-based position of the chosen button.
//! * A combo box / list box lists the shown texts of its input range (`fmlaRange`, or its own
//!   `itemLst`) and writes the 1-based index of the chosen item.
//! * A scroll bar / spin button moves its value between `min` and `max` by `inc` (`page` for a
//!   click in the track) and writes it.
//!
//! The linked cell drives the control: [`state`] reads the control's state from the cell, and
//! [`sync_from_links`] stores it in the model before saving.

use bo_model::sheet::{
    CellError, CellRange, CellRef, CellValue, CheckState, ControlAnchor, FormControl,
    FormControlKind as K, SelType, Workbook,
};
use serde::Serialize;

use crate::numfmt::Locale;
use crate::render::format_cell;

/// Most items a combo / list box lists.
pub const MAX_ITEMS: usize = 10_000;

/// Errors of form control edits (messages for the UI log; the UI shows its own text).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormControlError {
    /// No control with that index.
    NoControl,
    /// The operation does not apply to this kind of control.
    WrongKind,
    /// The linked cell or input range is not a valid reference.
    BadReference(String),
}

impl FormControlError {
    /// English message.
    #[must_use]
    pub fn message(&self) -> String {
        match self {
            Self::NoControl => "no such form control".into(),
            Self::WrongKind => "the control does not support this action".into(),
            Self::BadReference(r) => format!("invalid cell reference '{r}'"),
        }
    }
}

/// Sheet index and range of a reference like `$B$2`, `B2:B9`, `Sheet1!$A$1` or
/// `'Dữ liệu'!$A$1:$A$3` (a leading `=` is ignored); `sheet` is the default sheet.
#[must_use]
pub fn resolve(wb: &Workbook, sheet: usize, reference: &str) -> Option<(usize, CellRange)> {
    let r = reference.trim().trim_start_matches('=').trim();
    let (si, addr) = match r.rsplit_once('!') {
        Some((name, addr)) => {
            let name = name.trim();
            let name = name
                .strip_prefix('\'')
                .and_then(|n| n.strip_suffix('\''))
                .map_or_else(|| name.to_owned(), |n| n.replace("''", "'"));
            let lname = name.to_lowercase();
            let si = wb
                .sheets
                .iter()
                .position(|s| s.name.to_lowercase() == lname)?;
            (si, addr)
        }
        None => (sheet, r),
    };
    if si >= wb.sheets.len() {
        return None;
    }
    let addr: String = addr.chars().filter(|c| *c != '$').collect();
    CellRange::parse(&addr).map(|rg| (si, rg))
}

/// The linked cell of a control (first cell of the reference).
fn link_cell(wb: &Workbook, sheet: usize, c: &FormControl) -> Option<(usize, CellRef)> {
    resolve(wb, sheet, c.link.as_deref()?).map(|(s, r)| (s, r.first))
}

fn link_value<'a>(wb: &'a Workbook, sheet: usize, c: &FormControl) -> Option<&'a CellValue> {
    let (si, at) = link_cell(wb, sheet, c)?;
    Some(
        wb.sheets[si]
            .cell(at.row, at.col)
            .map_or(&CellValue::Empty, |cell| &cell.value),
    )
}

fn as_number(v: &CellValue) -> Option<f64> {
    match v {
        CellValue::Number(n) => Some(*n),
        CellValue::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
        CellValue::Text(t) => t.trim().parse().ok(),
        _ => None,
    }
}

/// Items of a combo / list box: the shown texts of its input range, else its own items.
#[must_use]
pub fn items(wb: &Workbook, sheet: usize, c: &FormControl, locale: &Locale) -> Vec<String> {
    let Some((si, rg)) = c.input_range.as_deref().and_then(|r| resolve(wb, sheet, r)) else {
        return c.items.clone();
    };
    let ws = &wb.sheets[si];
    let mut out = Vec::new();
    'rows: for row in rg.first.row..=rg.last.row {
        for col in rg.first.col..=rg.last.col {
            if out.len() >= MAX_ITEMS {
                break 'rows;
            }
            out.push(
                ws.cell(row, col)
                    .map(|cell| format_cell(wb, cell, locale).text)
                    .unwrap_or_default(),
            );
        }
    }
    out
}

/// `a` contains `b` (anchor rectangles compared as (cell, offset) positions).
fn contains(a: &ControlAnchor, b: &ControlAnchor) -> bool {
    let pos = |c: CellRef, off: (i64, i64)| ((c.row, off.1), (c.col, off.0));
    let (a0, a1) = (pos(a.from, a.from_off), pos(a.to, a.to_off));
    let (b0, b1) = (pos(b.from, b.from_off), pos(b.to, b.to_off));
    a0.0 <= b0.0 && a0.1 <= b0.1 && b1.0 <= a1.0 && b1.1 <= a1.1
}

/// Option-button groups of a sheet: indexes of the buttons of each group, in drawing order.
/// A button belongs to the smallest group box that contains it; the buttons outside every
/// group box form one group.
#[must_use]
pub fn option_groups(controls: &[FormControl]) -> Vec<Vec<usize>> {
    let boxes: Vec<usize> = (0..controls.len())
        .filter(|&i| controls[i].kind == K::GroupBox)
        .collect();
    let area = |a: &ControlAnchor| {
        (u64::from(a.to.row.saturating_sub(a.from.row)) + 1)
            * (u64::from(a.to.col.saturating_sub(a.from.col)) + 1)
    };
    // Group key: index of the group box, or `usize::MAX` for the sheet.
    let mut groups: Vec<(usize, Vec<usize>)> = Vec::new();
    for (i, c) in controls.iter().enumerate() {
        if c.kind != K::OptionButton {
            continue;
        }
        let key = boxes
            .iter()
            .copied()
            .filter(|&b| contains(&controls[b].anchor, &c.anchor))
            .min_by_key(|&b| area(&controls[b].anchor))
            .unwrap_or(usize::MAX);
        match groups.iter_mut().find(|(k, _)| *k == key) {
            Some((_, v)) => v.push(i),
            None => groups.push((key, vec![i])),
        }
    }
    groups.into_iter().map(|(_, v)| v).collect()
}

/// The group of option button `index` and its linked cell reference (the first one set).
fn group_of(controls: &[FormControl], index: usize) -> (Vec<usize>, Option<String>) {
    let group = option_groups(controls)
        .into_iter()
        .find(|g| g.contains(&index))
        .unwrap_or_else(|| vec![index]);
    let link = group.iter().find_map(|&i| controls[i].link.clone());
    (group, link)
}

/// The state of a control as the UI shows it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ControlState {
    /// Check box / option button state.
    pub checked: CheckState,
    /// Chosen item (1-based, 0 = none).
    pub sel: u32,
    /// Scroll bar / spin button value.
    pub val: i32,
}

/// The current state of control `index` of `sheet`: from its linked cell when it has one
/// (the cell drives the control), else as stored.
#[must_use]
pub fn state(wb: &Workbook, sheet: usize, index: usize) -> Option<ControlState> {
    let controls = &wb.sheets.get(sheet)?.form_controls;
    let c = controls.get(index)?;
    let mut s = ControlState {
        checked: c.checked,
        sel: c.sel,
        val: c.val,
    };
    match c.kind {
        K::CheckBox => {
            if let Some(v) = link_value(wb, sheet, c) {
                s.checked = match v {
                    CellValue::Error(CellError::NA) => CheckState::Mixed,
                    v => match as_number(v) {
                        Some(n) if n != 0.0 => CheckState::Checked,
                        _ => CheckState::Unchecked,
                    },
                };
            }
        }
        K::OptionButton => {
            let (group, link) = group_of(controls, index);
            if let Some(link) = link {
                let probe = FormControl {
                    link: Some(link),
                    ..FormControl::default()
                };
                if let Some(v) = link_value(wb, sheet, &probe) {
                    let pos = group.iter().position(|&i| i == index).map_or(0, |p| p + 1);
                    s.checked = if as_number(v).is_some_and(|n| n.round() as usize == pos) {
                        CheckState::Checked
                    } else {
                        CheckState::Unchecked
                    };
                }
            }
        }
        // A multi-select list box ignores its linked cell ([MS-XLSX] `selType`).
        K::ListBox if c.sel_type != SelType::Single => {}
        K::ComboBox | K::ListBox => {
            if let Some(n) = link_value(wb, sheet, c).and_then(as_number) {
                s.sel = if n >= 1.0 { n.floor() as u32 } else { 0 };
            }
        }
        K::ScrollBar | K::Spinner => {
            if let Some(n) = link_value(wb, sheet, c).and_then(as_number) {
                s.val = clamp(c, n.round() as i32);
            }
        }
        K::Button | K::GroupBox | K::Label | K::ActiveX => {}
    }
    Some(s)
}

/// `v` limited to the control's range (`min` / `max` in either order).
#[must_use]
pub fn clamp(c: &FormControl, v: i32) -> i32 {
    v.clamp(c.min.min(c.max), c.max.max(c.min))
}

/// Writes `value` to a reference (its first cell); the cell's formula is replaced.
fn write_link(
    wb: &mut Workbook,
    sheet: usize,
    link: &str,
    value: CellValue,
) -> Result<(), FormControlError> {
    let (si, rg) =
        resolve(wb, sheet, link).ok_or_else(|| FormControlError::BadReference(link.to_owned()))?;
    let cell = wb.sheets[si]
        .cells
        .entry((rg.first.row, rg.first.col))
        .or_default();
    cell.formula = None;
    cell.value = value;
    Ok(())
}

fn control_mut(
    wb: &mut Workbook,
    sheet: usize,
    index: usize,
) -> Result<&mut FormControl, FormControlError> {
    wb.sheets
        .get_mut(sheet)
        .and_then(|s| s.form_controls.get_mut(index))
        .ok_or(FormControlError::NoControl)
}

/// What a click did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Clicked {
    /// The control changed (and wrote its linked cell, if any).
    Changed,
    /// A button: its assigned macro (not run).
    Macro(Option<String>),
    /// Nothing to do (label, group box, ActiveX, a disabled control …).
    Nothing,
}

/// A click on control `index`: a check box toggles (mixed → checked), an option button is
/// chosen in its group, a button reports its macro.
pub fn click(wb: &mut Workbook, sheet: usize, index: usize) -> Result<Clicked, FormControlError> {
    let now = state(wb, sheet, index).ok_or(FormControlError::NoControl)?;
    let kind = control_mut(wb, sheet, index)?.kind;
    match kind {
        K::Button => Ok(Clicked::Macro(
            control_mut(wb, sheet, index)?.macro_name.clone(),
        )),
        K::CheckBox => {
            let next = match now.checked {
                CheckState::Checked => CheckState::Unchecked,
                CheckState::Unchecked | CheckState::Mixed => CheckState::Checked,
            };
            set_checked(wb, sheet, index, next)?;
            Ok(Clicked::Changed)
        }
        K::OptionButton => {
            set_checked(wb, sheet, index, CheckState::Checked)?;
            Ok(Clicked::Changed)
        }
        _ => Ok(Clicked::Nothing),
    }
}

/// Sets a check box's or option button's state (choosing an option button clears the others
/// of its group) and writes the linked cell.
pub fn set_checked(
    wb: &mut Workbook,
    sheet: usize,
    index: usize,
    checked: CheckState,
) -> Result<(), FormControlError> {
    let kind = control_mut(wb, sheet, index)?.kind;
    match kind {
        K::CheckBox => {
            let c = control_mut(wb, sheet, index)?;
            c.checked = checked;
            c.dirty = true;
            if let Some(link) = c.link.clone() {
                let v = match checked {
                    CheckState::Checked => CellValue::Bool(true),
                    CheckState::Unchecked => CellValue::Bool(false),
                    CheckState::Mixed => CellValue::Error(CellError::NA),
                };
                write_link(wb, sheet, &link, v)?;
            }
            Ok(())
        }
        K::OptionButton => {
            let (group, link) = group_of(&wb.sheets[sheet].form_controls, index);
            let on = checked == CheckState::Checked;
            for &i in &group {
                let c = control_mut(wb, sheet, i)?;
                let want = if i == index && on {
                    CheckState::Checked
                } else if on || i == index {
                    CheckState::Unchecked
                } else {
                    c.checked
                };
                if c.checked != want {
                    c.checked = want;
                    c.dirty = true;
                }
            }
            if let (Some(link), true) = (link, on) {
                let pos = group.iter().position(|&i| i == index).map_or(1, |p| p + 1);
                write_link(wb, sheet, &link, CellValue::Number(pos as f64))?;
            }
            Ok(())
        }
        _ => Err(FormControlError::WrongKind),
    }
}

/// Chooses item `sel` (1-based; 0 = none) of a combo / list box and writes it to the linked
/// cell (nothing is written for "none").
pub fn select(
    wb: &mut Workbook,
    sheet: usize,
    index: usize,
    sel: u32,
    locale: &Locale,
) -> Result<(), FormControlError> {
    let c = wb
        .sheets
        .get(sheet)
        .and_then(|s| s.form_controls.get(index))
        .ok_or(FormControlError::NoControl)?;
    if !matches!(c.kind, K::ComboBox | K::ListBox) {
        return Err(FormControlError::WrongKind);
    }
    let n = items(wb, sheet, c, locale).len() as u32;
    let sel = sel.min(n);
    let c = control_mut(wb, sheet, index)?;
    c.sel = sel;
    c.dirty = true;
    if c.kind == K::ListBox && c.sel_type != SelType::Single {
        c.multi_sel = if sel > 0 { vec![sel] } else { Vec::new() };
        return Ok(());
    }
    if let (Some(link), true) = (c.link.clone(), sel > 0) {
        write_link(wb, sheet, &link, CellValue::Number(f64::from(sel)))?;
    }
    Ok(())
}

/// How a click on a list box item combines with the current selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ListClick {
    /// Ctrl held (an `extend` list box toggles the item).
    pub ctrl: bool,
    /// Shift held (an `extend` list box selects from the last clicked item).
    pub shift: bool,
}

/// A click on item `item` (1-based) of a multi-select list box ([MS-XLSX] `selType`
/// `multi` / `extend`): `multi` toggles the item; `extend` chooses only it, toggles it with
/// Ctrl, or selects the run from the last clicked item (`sel`) with Shift. The selection is
/// kept in `multiSel`; such a list box does not write its linked cell. A single-selection
/// list box behaves like [`select`].
pub fn click_item(
    wb: &mut Workbook,
    sheet: usize,
    index: usize,
    item: u32,
    how: ListClick,
    locale: &Locale,
) -> Result<(), FormControlError> {
    let c = wb
        .sheets
        .get(sheet)
        .and_then(|s| s.form_controls.get(index))
        .ok_or(FormControlError::NoControl)?;
    if c.kind != K::ListBox {
        return select(wb, sheet, index, item, locale);
    }
    if c.sel_type == SelType::Single {
        return select(wb, sheet, index, item, locale);
    }
    let n = items(wb, sheet, c, locale).len() as u32;
    if item == 0 || item > n {
        return Ok(());
    }
    let c = control_mut(wb, sheet, index)?;
    let mut set: Vec<u32> = c.multi_sel.iter().copied().filter(|&i| i <= n).collect();
    let toggle = |set: &mut Vec<u32>| {
        if let Some(p) = set.iter().position(|&i| i == item) {
            set.remove(p);
        } else {
            set.push(item);
        }
    };
    match c.sel_type {
        SelType::Multi => toggle(&mut set),
        SelType::Extend if how.shift && c.sel > 0 => {
            let (a, b) = (c.sel.min(item), c.sel.max(item));
            if !how.ctrl {
                set.clear();
            }
            set.extend(a..=b);
        }
        SelType::Extend if how.ctrl => toggle(&mut set),
        _ => set = vec![item],
    }
    set.sort_unstable();
    set.dedup();
    c.multi_sel = set;
    // The last clicked item: the start of a later Shift+click run.
    if !(how.shift && c.sel > 0) {
        c.sel = item;
    }
    c.dirty = true;
    Ok(())
}

/// Sets a scroll bar / spin button's value (limited to its range) and writes it; returns the
/// value set.
pub fn set_value(
    wb: &mut Workbook,
    sheet: usize,
    index: usize,
    value: i32,
) -> Result<i32, FormControlError> {
    let c = control_mut(wb, sheet, index)?;
    if !matches!(c.kind, K::ScrollBar | K::Spinner) {
        return Err(FormControlError::WrongKind);
    }
    let v = clamp(c, value);
    c.val = v;
    c.dirty = true;
    if let Some(link) = c.link.clone() {
        write_link(wb, sheet, &link, CellValue::Number(f64::from(v)))?;
    }
    Ok(v)
}

/// Moves a scroll bar / spin button by `steps` arrow steps (`inc`) or track pages (`page`).
pub fn step(
    wb: &mut Workbook,
    sheet: usize,
    index: usize,
    steps: i32,
    page: bool,
) -> Result<i32, FormControlError> {
    let now = state(wb, sheet, index).ok_or(FormControlError::NoControl)?;
    let c = control_mut(wb, sheet, index)?;
    let by = if page { c.page } else { c.inc }.max(1);
    let v = i64::from(now.val) + i64::from(steps) * i64::from(by);
    set_value(
        wb,
        sheet,
        index,
        v.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32,
    )
}

/// Stores every control's state from its linked cell (marking changed controls dirty), so a
/// saved file agrees with its cells.
pub fn sync_from_links(wb: &mut Workbook) {
    for si in 0..wb.sheets.len() {
        for i in 0..wb.sheets[si].form_controls.len() {
            let Some(s) = state(wb, si, i) else { continue };
            let c = &mut wb.sheets[si].form_controls[i];
            if (c.checked, c.sel, c.val) != (s.checked, s.sel, s.val) {
                c.checked = s.checked;
                c.sel = s.sel;
                c.val = s.val;
                c.dirty = true;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bo_model::sheet::Worksheet;

    fn at(r0: u32, c0: u32, r1: u32, c1: u32) -> ControlAnchor {
        ControlAnchor {
            from: CellRef::new(r0, c0),
            to: CellRef::new(r1, c1),
            ..ControlAnchor::default()
        }
    }

    fn ctl(kind: K, anchor: ControlAnchor, link: Option<&str>) -> FormControl {
        FormControl {
            kind,
            anchor,
            link: link.map(str::to_owned),
            ..FormControl::default()
        }
    }

    #[test]
    fn groups_option_buttons_by_group_box() {
        let list = vec![
            ctl(K::GroupBox, at(0, 0, 10, 3), None),
            ctl(K::OptionButton, at(1, 1, 1, 2), Some("$E$1")),
            ctl(K::OptionButton, at(20, 1, 20, 2), Some("$E$2")),
            ctl(K::OptionButton, at(3, 1, 3, 2), None),
            ctl(K::OptionButton, at(22, 1, 22, 2), None),
        ];
        assert_eq!(option_groups(&list), vec![vec![1, 3], vec![2, 4]]);
    }

    #[test]
    fn resolves_sheet_qualified_references() {
        let mut wb = Workbook::new();
        wb.sheets.push(Worksheet::new("Dữ liệu"));
        assert_eq!(
            resolve(&wb, 0, "'Dữ liệu'!$A$1:$A$3"),
            Some((1, CellRange::parse("A1:A3").unwrap()))
        );
        assert_eq!(resolve(&wb, 1, "=$C$2").map(|x| x.0), Some(1));
        assert_eq!(resolve(&wb, 0, "Khác!A1"), None);
    }
}
