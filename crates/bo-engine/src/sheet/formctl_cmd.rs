//! Form control commands over the JSON bridge ([MS-XLSX] `formControlPr`, ECMA-376 Part 1
//! §18.3.1.21 `controls`, legacy VML `x:ClientData`): list, insert, click, choose an item,
//! change a value, format (linked cell, input range, limits, state), move and delete.
//!
//! Every change is one undo step; a change that writes the linked cell recalculates the
//! workbook. Buttons report their assigned macro, which is kept but never run.

use base64::Engine as _;
use bo_model::sheet::{CellRef, CheckState, ControlAnchor, FormControl, FormControlKind};
use bo_sheet::formctl::{self as fc, FormControlError};
use serde::Deserialize;
use serde_json::{json, Value};

use super::cmd::Recalc;
use super::{SheetEngine, MAX_COLS, MAX_ROWS};
use crate::{EngineError, Result};

fn bad(msg: impl Into<String>) -> EngineError {
    EngineError::Sheet(msg.into())
}

fn err(e: &FormControlError) -> EngineError {
    bad(e.message())
}

/// EMU per point.
const EMU_PER_PT: f64 = 12_700.0;

/// Fields of the Format Control dialog; absent fields are left as they are, an empty
/// reference / macro clears it.
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FormatControlSpec {
    /// Caption.
    pub text: Option<String>,
    /// Name.
    pub name: Option<String>,
    /// Linked cell (`""` clears).
    pub link: Option<String>,
    /// Input range of a combo / list box (`""` clears).
    pub input_range: Option<String>,
    /// Check box / option button state.
    pub checked: Option<CheckState>,
    /// Current value of a scroll bar / spin button.
    pub val: Option<i32>,
    /// Minimum value.
    pub min: Option<i32>,
    /// Maximum value.
    pub max: Option<i32>,
    /// Arrow step.
    pub inc: Option<i32>,
    /// Page step.
    pub page: Option<i32>,
    /// Rows an open combo box shows.
    pub drop_lines: Option<u32>,
    /// Flat look.
    pub no_three_d: Option<bool>,
    /// Horizontal scroll bar / spin button.
    pub horiz: Option<bool>,
    /// Assigned macro (`""` clears).
    pub macro_name: Option<String>,
}

/// Size of a new control in cells (columns, rows) and the end offset (EMU): an up-down
/// button is narrow so that it is drawn upright.
fn default_size(kind: FormControlKind) -> (u32, u32, (i64, i64)) {
    use FormControlKind as K;
    match kind {
        K::Button => (2, 2, (0, 0)),
        K::ListBox => (2, 5, (0, 0)),
        K::ScrollBar => (1, 6, (0, 0)),
        K::GroupBox => (3, 6, (0, 0)),
        K::Spinner => (0, 2, (18 * 12_700, 0)),
        _ => (2, 1, (0, 0)),
    }
}

fn kind_name(kind: FormControlKind) -> &'static str {
    kind.object_type().unwrap_or("ActiveX")
}

/// Image bytes as a data URL the UI can draw (PNG / JPEG / GIF as they are).
fn data_url(bytes: &[u8]) -> Option<String> {
    let mime = if bytes.starts_with(b"\x89PNG") {
        "image/png"
    } else if bytes.starts_with(&[0xFF, 0xD8]) {
        "image/jpeg"
    } else if bytes.starts_with(b"GIF8") {
        "image/gif"
    } else {
        return None;
    };
    Some(format!(
        "data:{mime};base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    ))
}

fn pt(emu: i64) -> f64 {
    (emu as f64 / EMU_PER_PT * 100.0).round() / 100.0
}

/// A reference from the dialog: `None` keeps the current one, `""` clears it, anything
/// else must resolve (an error names it otherwise).
fn checked_ref(
    wb: &bo_model::sheet::Workbook,
    sheet: usize,
    r: Option<String>,
) -> Result<Option<String>> {
    let Some(s) = r else { return Ok(None) };
    let s = s.trim().trim_start_matches('=').to_owned();
    if !s.is_empty() && fc::resolve(wb, sheet, &s).is_none() {
        return Err(err(&FormControlError::BadReference(s)));
    }
    Ok(Some(s))
}

/// The stored form of a checked reference (`""` → none).
fn stored(r: String) -> Option<String> {
    (!r.is_empty()).then_some(r)
}

impl SheetEngine {
    /// The controls of a sheet with their current state (from the linked cells).
    pub(super) fn form_controls_list(&mut self, sheet: usize) -> Result<Value> {
        let locale = self.locale;
        let wb = self.wb()?;
        let ws = self.sheet(sheet)?;
        let groups = fc::option_groups(&ws.form_controls);
        let mut images: Vec<(usize, Vec<u8>, i64, i64)> = Vec::new();
        let mut list: Vec<Value> = Vec::with_capacity(ws.form_controls.len());
        for (i, c) in ws.form_controls.iter().enumerate() {
            let st = fc::state(wb, sheet, i).unwrap_or(fc::ControlState {
                checked: c.checked,
                sel: c.sel,
                val: c.val,
            });
            let a = &c.anchor;
            let items = matches!(c.kind, FormControlKind::ComboBox | FormControlKind::ListBox)
                .then(|| fc::items(wb, sheet, c, &locale));
            if let Some(p) = c
                .image_part
                .as_ref()
                .and_then(|n| wb.opaque.parts.iter().find(|p| &p.name == n))
            {
                let w = 12_700 * 64 * i64::from(a.to.col.saturating_sub(a.from.col).max(1));
                let h = 12_700 * 20 * i64::from(a.to.row.saturating_sub(a.from.row).max(1));
                images.push((i, p.data.clone(), w, h));
            }
            list.push(json!({
                "index": i,
                "kind": c.kind,
                "name": c.name,
                "text": c.text,
                "link": c.link,
                "inputRange": c.input_range,
                "items": items,
                "checked": st.checked,
                "sel": st.sel,
                "val": st.val,
                "min": c.min,
                "max": c.max,
                "inc": c.inc,
                "page": c.page,
                "dropLines": c.drop_lines,
                "horiz": c.horiz,
                "noThreeD": c.no_three_d,
                "macro": c.macro_name,
                "group": groups.iter().position(|g| g.contains(&i)),
                "from": [a.from.row, a.from.col],
                "fromOff": [pt(a.from_off.0), pt(a.from_off.1)],
                "to": [a.to.row, a.to.col],
                "toOff": [pt(a.to_off.0), pt(a.to_off.1)],
                "image": Value::Null,
                "isNew": c.is_new(),
            }));
        }
        // Fallback pictures of ActiveX controls (metafiles are rendered to PNG).
        for (i, bytes, w, h) in images {
            let url = data_url(&bytes).or_else(|| {
                crate::media::to_png(&bytes, w, h, &mut self.fonts).and_then(|p| data_url(&p))
            });
            if let (Some(u), Some(v)) = (url, list.get_mut(i)) {
                v["image"] = Value::String(u);
            }
        }
        Ok(Value::Array(list))
    }

    /// Inserts a control of `kind` with its top-left corner at `row`, `col`.
    pub(super) fn form_control_insert(
        &mut self,
        sheet: usize,
        kind: FormControlKind,
        row: u32,
        col: u32,
        text: Option<String>,
        name: Option<String>,
    ) -> Result<Value> {
        if kind == FormControlKind::ActiveX {
            return Err(bad("ActiveX controls cannot be inserted"));
        }
        let (w, h, end_off) = default_size(kind);
        let from = CellRef::new(row.min(MAX_ROWS - h - 1), col.min(MAX_COLS - w - 1));
        let to = CellRef::new(from.row + h, from.col + w);
        self.edit(sheet, Recalc::No, |wb, _| {
            let ws = &mut wb.sheets[sheet];
            let n = ws.form_controls.iter().filter(|c| c.kind == kind).count() + 1;
            let name = name
                .filter(|s| !s.trim().is_empty())
                .unwrap_or_else(|| format!("{} {n}", kind_name(kind)));
            let text = text.unwrap_or_else(|| name.clone());
            ws.form_controls.push(FormControl {
                kind,
                name,
                text,
                anchor: ControlAnchor {
                    from,
                    from_off: (0, 0),
                    to,
                    to_off: end_off,
                },
                ..FormControl::default()
            });
            Ok(json!({ "index": ws.form_controls.len() - 1 }))
        })
    }

    /// A click on a control: `{ changed, macro }` (`macro` for a button, not run).
    pub(super) fn form_control_click(&mut self, sheet: usize, index: usize) -> Result<Value> {
        let kind = self
            .sheet(sheet)?
            .form_controls
            .get(index)
            .map(|c| (c.kind, c.macro_name.clone()))
            .ok_or_else(|| err(&FormControlError::NoControl))?;
        match kind {
            (FormControlKind::CheckBox | FormControlKind::OptionButton, _) => {
                self.edit(sheet, Recalc::All, |wb, _| {
                    fc::click(wb, sheet, index).map_err(|e| err(&e))
                })?;
                self.invalidate(None);
                Ok(json!({ "changed": true, "macro": Value::Null }))
            }
            (FormControlKind::Button, m) => Ok(json!({
                "changed": false,
                "macro": m,
                "button": true,
            })),
            _ => Ok(json!({ "changed": false, "macro": Value::Null })),
        }
    }

    /// Chooses item `sel` (1-based, 0 = none) of a combo / list box.
    pub(super) fn form_control_select(
        &mut self,
        sheet: usize,
        index: usize,
        sel: u32,
    ) -> Result<Value> {
        let locale = self.locale;
        self.edit(sheet, Recalc::All, |wb, _| {
            fc::select(wb, sheet, index, sel, &locale).map_err(|e| err(&e))
        })?;
        self.invalidate(None);
        Ok(json!({ "ok": true }))
    }

    /// Sets (`value`) or steps (`step` arrow steps, or pages with `page`) a scroll bar /
    /// spin button; returns `{ val }`.
    pub(super) fn form_control_value(
        &mut self,
        sheet: usize,
        index: usize,
        value: Option<i32>,
        step: Option<i32>,
        page: bool,
    ) -> Result<Value> {
        let v = self.edit(sheet, Recalc::All, |wb, _| {
            match (value, step) {
                (Some(v), _) => fc::set_value(wb, sheet, index, v),
                (None, Some(s)) => fc::step(wb, sheet, index, s, page),
                (None, None) => Err(FormControlError::WrongKind),
            }
            .map_err(|e| err(&e))
        })?;
        self.invalidate(None);
        Ok(json!({ "val": v }))
    }

    /// Applies the Format Control dialog; the linked cell receives the control's state.
    pub(super) fn form_control_format(
        &mut self,
        sheet: usize,
        index: usize,
        spec: FormatControlSpec,
    ) -> Result<Value> {
        let locale = self.locale;
        self.edit(sheet, Recalc::All, |wb, _| {
            let link = checked_ref(wb, sheet, spec.link.clone())?;
            let range = checked_ref(wb, sheet, spec.input_range.clone())?;
            let now =
                fc::state(wb, sheet, index).ok_or_else(|| err(&FormControlError::NoControl))?;
            let c = wb.sheets[sheet]
                .form_controls
                .get_mut(index)
                .ok_or_else(|| err(&FormControlError::NoControl))?;
            if c.kind == FormControlKind::ActiveX {
                return Err(bad("ActiveX controls are kept as they are"));
            }
            let link_changed = link
                .as_ref()
                .is_some_and(|l| c.link.as_deref() != stored(l.clone()).as_deref());
            if let Some(t) = spec.text {
                c.text = t;
            }
            if let Some(n) = spec.name.filter(|n| !n.trim().is_empty()) {
                c.name = n;
            }
            if let Some(l) = link {
                c.link = stored(l);
            }
            if let Some(r) = range {
                c.input_range = stored(r);
            }
            if let Some(v) = spec.min {
                c.min = v;
            }
            if let Some(v) = spec.max {
                c.max = v;
            }
            if let Some(v) = spec.inc {
                c.inc = v.max(1);
            }
            if let Some(v) = spec.page {
                c.page = v.max(1);
            }
            if let Some(v) = spec.drop_lines {
                c.drop_lines = v.clamp(1, 100);
            }
            if let Some(v) = spec.no_three_d {
                c.no_three_d = v;
            }
            if let Some(v) = spec.horiz {
                c.horiz = v;
            }
            if let Some(m) = spec.macro_name {
                c.macro_name = (!m.trim().is_empty()).then_some(m);
            }
            c.dirty = true;
            let kind = c.kind;
            let has_link = c.link.is_some();
            // The state, from the dialog or as it was, goes to the (new) linked cell.
            match kind {
                FormControlKind::CheckBox | FormControlKind::OptionButton => {
                    let s = spec.checked.unwrap_or(now.checked);
                    if spec.checked.is_some() || (link_changed && has_link) {
                        if kind == FormControlKind::OptionButton && s != CheckState::Checked {
                            c.checked = s;
                        } else {
                            fc::set_checked(wb, sheet, index, s).map_err(|e| err(&e))?;
                        }
                    }
                }
                FormControlKind::ScrollBar | FormControlKind::Spinner => {
                    let v = spec.val.unwrap_or(now.val);
                    fc::set_value(wb, sheet, index, v).map_err(|e| err(&e))?;
                }
                FormControlKind::ComboBox | FormControlKind::ListBox
                    if link_changed && has_link =>
                {
                    fc::select(wb, sheet, index, now.sel, &locale).map_err(|e| err(&e))?;
                }
                _ => {}
            }
            Ok(())
        })?;
        self.invalidate(None);
        Ok(json!({ "ok": true }))
    }

    /// Moves a control so its top-left corner is at `row`, `col` (offsets in points); the
    /// size in cells is kept.
    pub(super) fn form_control_move(
        &mut self,
        sheet: usize,
        index: usize,
        row: u32,
        col: u32,
        off: (f64, f64),
    ) -> Result<Value> {
        self.edit(sheet, Recalc::No, |wb, _| {
            let c = wb.sheets[sheet]
                .form_controls
                .get_mut(index)
                .ok_or_else(|| err(&FormControlError::NoControl))?;
            let a = c.anchor;
            let rows = a.to.row.saturating_sub(a.from.row);
            let cols = a.to.col.saturating_sub(a.from.col);
            let from = CellRef::new(row.min(MAX_ROWS - rows - 1), col.min(MAX_COLS - cols - 1));
            c.anchor = ControlAnchor {
                from,
                from_off: (
                    (off.0.max(0.0) * EMU_PER_PT) as i64,
                    (off.1.max(0.0) * EMU_PER_PT) as i64,
                ),
                to: CellRef::new(from.row + rows, from.col + cols),
                to_off: a.to_off,
            };
            c.dirty = true;
            Ok(json!({ "ok": true }))
        })
    }

    /// Deletes a form control (ActiveX controls are kept).
    pub(super) fn form_control_delete(&mut self, sheet: usize, index: usize) -> Result<Value> {
        self.edit(sheet, Recalc::No, |wb, _| {
            let list = &mut wb.sheets[sheet].form_controls;
            match list.get(index) {
                None => Err(err(&FormControlError::NoControl)),
                Some(c) if c.kind == FormControlKind::ActiveX => {
                    Err(bad("ActiveX controls are kept as they are"))
                }
                Some(_) => {
                    list.remove(index);
                    Ok(json!({ "ok": true }))
                }
            }
        })
    }
}
