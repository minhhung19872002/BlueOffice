//! Legacy form fields (ECMA-376 Part 1, §17.16.17 ffData: `FORMTEXT`, `FORMCHECKBOX`,
//! `FORMDROPDOWN`) — inserting, properties, filling in — and the "filling in forms" editing
//! restriction (§17.15.1.29 documentProtection `w:edit="forms"`, §17.6.6 formProt).
//!
//! A field is a [`RunContent::FormField`] marker, its result atoms and a [`RunContent::FormEnd`]
//! marker (see `bo_model::form`). Macros (§17.16.15 entryMacro, §17.16.16 exitMacro) are kept
//! but never run.

use crate::para::{self, Atom};
use crate::{fields, EditError, Pos, Result, Selection, Session};
use bo_model::field::{format_date, format_picture, DateTime, FormulaEnv};
use bo_model::form::{DocProtection, FormField, FormFieldKind, TextInputType, EMPTY_TEXT_RESULT};
use bo_model::index::{body_paragraphs, para_at};
use bo_model::{Paragraph, RunContent, RunProps};
use serde::Serialize;

/// A form field of a body paragraph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormSpan {
    /// Body paragraph id.
    pub para: usize,
    /// Atom index of the [`RunContent::FormField`] marker.
    pub start: usize,
    /// Atom index of the [`RunContent::FormEnd`] marker.
    pub end: usize,
    /// Start of the result (text field) or of the object (check box, list).
    pub from: usize,
    /// End of the result / object.
    pub to: usize,
    /// The field data.
    pub field: FormField,
}

/// The form field around the caret, for hosts.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FormFieldInfo {
    /// The field data (§17.16.17).
    pub field: FormField,
    /// Current result text (text field), the selected entry (list) or ☒ / ☐.
    pub value: String,
    /// Can be filled in now (enabled and not computed).
    pub fillable: bool,
}

/// Text of the result atoms.
fn result_text(atoms: &[Atom]) -> String {
    let mut s = String::new();
    for (_, c) in atoms {
        if let RunContent::Text(t) = c {
            s.push_str(t);
        }
    }
    s
}

/// The form fields of a paragraph (`para` = its id).
pub fn spans(p: &Paragraph, para: usize) -> Vec<FormSpan> {
    let atoms = para::flatten(p);
    let mut out = Vec::new();
    let mut open: Vec<(usize, usize, usize)> = Vec::new(); // atom, from, to (objects)
    let mut pos = 0;
    for (i, (_, c)) in atoms.iter().enumerate() {
        let l = para::atom_len(c);
        match c {
            RunContent::FormField(_) => open.push((i, pos, pos + l)),
            RunContent::FormEnd => {
                if let Some((s, from, obj_end)) = open.pop() {
                    if let RunContent::FormField(f) = &atoms[s].1 {
                        let to = if f.is_text() { pos } else { obj_end };
                        out.push(FormSpan {
                            para,
                            start: s,
                            end: i,
                            from,
                            to,
                            field: (**f).clone(),
                        });
                    }
                }
            }
            _ => {}
        }
        pos += l;
    }
    out.sort_by_key(|s| s.start);
    out
}

/// Value shown by a field.
fn value_of(atoms: &[Atom], s: &FormSpan) -> String {
    match s.field.display() {
        Some(d) => d,
        None => result_text(&atoms[s.start + 1..s.end]),
    }
}

/// The result is still the empty placeholder (five en spaces) or nothing.
fn is_placeholder(text: &str) -> bool {
    text.chars().all(|c| c == '\u{2002}')
}

/// Can the user fill in `f` (enabled, not computed)?
fn fillable(f: &FormField) -> bool {
    f.enabled
        && !matches!(
            &f.kind,
            FormFieldKind::Text(t) if matches!(
                t.input_type,
                TextInputType::Calculated | TextInputType::CurrentDate | TextInputType::CurrentTime
            )
        )
}

/// Parse a typed number: spaces dropped, a lone comma is the decimal separator, otherwise
/// commas group thousands.
fn parse_number(s: &str) -> Option<f64> {
    let t: String = s
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '\u{2002}')
        .collect();
    let t = if t.contains(',') && !t.contains('.') && t.matches(',').count() == 1 {
        t.replace(',', ".")
    } else {
        t.replace(',', "")
    };
    t.parse().ok().filter(|v: &f64| v.is_finite())
}

/// Parse a typed date: ISO 8601 or day / month / year with `/`, `-` or `.` (a two-digit year is
/// 20xx).
pub fn parse_date(s: &str) -> Option<DateTime> {
    let s = s.trim().trim_matches('\u{2002}');
    if let Some(d) = DateTime::parse_iso(s) {
        if s.split(['-', 'T']).next().is_some_and(|y| y.len() == 4) {
            return Some(d);
        }
    }
    let parts: Vec<&str> = s.split(['/', '-', '.']).map(str::trim).collect();
    let [d, m, y] = parts.as_slice() else {
        return None;
    };
    let (day, month, mut year): (u32, u32, i32) =
        (d.parse().ok()?, m.parse().ok()?, y.parse().ok()?);
    if y.len() <= 2 {
        year += 2000;
    }
    let dim = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 => 29,
        2 => 28,
        _ => return None,
    };
    (1..=dim).contains(&day).then_some(DateTime {
        year,
        month,
        day,
        ..DateTime::default()
    })
}

/// A text format of §17.16.20: the letter case of regular text.
fn apply_case(text: &str, format: &str) -> String {
    match format.to_ascii_uppercase().as_str() {
        "UPPERCASE" => text.to_uppercase(),
        "LOWERCASE" => text.to_lowercase(),
        "FIRST CAPITAL" => {
            let mut c = text.chars();
            match c.next() {
                Some(f) => f.to_uppercase().chain(c).collect(),
                None => String::new(),
            }
        }
        "TITLE CASE" => {
            let mut out = String::new();
            let mut start = true;
            for ch in text.chars() {
                if start && ch.is_alphabetic() {
                    out.extend(ch.to_uppercase());
                    start = false;
                } else {
                    out.push(ch);
                    if ch.is_whitespace() {
                        start = true;
                    }
                }
            }
            out
        }
        _ => text.to_owned(),
    }
}

/// The result of a text field after leaving it: the format of its type applied. `None` when the
/// text is not valid for the type (a number / date field holding something else).
#[must_use]
pub fn formatted_result(
    input: &bo_model::form::FormTextInput,
    text: &str,
    vi: bool,
) -> Option<String> {
    if is_placeholder(text) {
        return Some(text.to_owned());
    }
    match input.input_type {
        TextInputType::Regular => Some(apply_case(text, &input.format)),
        TextInputType::Number => {
            let v = parse_number(text)?;
            Some(if input.format.is_empty() {
                text.trim().to_owned()
            } else {
                format_picture(v, &input.format)
            })
        }
        TextInputType::Date => {
            let d = parse_date(text)?;
            Some(if input.format.is_empty() {
                text.trim().to_owned()
            } else {
                format_date(&d, &input.format, vi)
            })
        }
        _ => Some(text.to_owned()),
    }
}

/// Numbers of bookmarks for calculated fields (§17.16.3 `=` formulas).
struct Bookmarks(std::collections::HashMap<String, String>);

impl FormulaEnv for Bookmarks {
    fn bookmark(&self, name: &str) -> Option<f64> {
        parse_number(self.0.get(&name.to_ascii_lowercase())?)
    }
    fn cells(&self, _reference: &str) -> Option<Vec<f64>> {
        None
    }
}

/// Smallest `prefix<n>` not yet used by a bookmark or a form field.
fn new_name(doc: &bo_model::Document, prefix: &str) -> String {
    let mut used: Vec<String> = fields::bookmarks(doc)
        .into_iter()
        .map(|b| b.name.to_ascii_lowercase())
        .collect();
    for path in body_paragraphs(doc) {
        if let Some(p) = para_at(doc, &path) {
            for s in spans(p, 0) {
                if let Some(n) = s.field.name {
                    used.push(n.to_ascii_lowercase());
                }
            }
        }
    }
    (1..=used.len() + 1)
        .map(|n| format!("{prefix}{n}"))
        .find(|n| !used.contains(&n.to_ascii_lowercase()))
        .unwrap_or_else(|| prefix.to_owned())
}

/// Rename the bookmark `old` of a paragraph's atoms (§17.13.6.2 bookmarkStart) to `new`.
fn rename_bookmark(atoms: &mut [Atom], old: &str, new: &str) {
    let key = format!("w:name=\"{}\"", bo_opc_escape(old));
    for (_, c) in atoms {
        if let RunContent::Markup(raw) = c {
            if raw.xml.contains("bookmarkStart") && raw.xml.contains(&key) {
                let xml = raw
                    .xml
                    .replace(&key, &format!("w:name=\"{}\"", bo_opc_escape(new)));
                *raw = bo_model::Raw::new(xml, 0);
            }
        }
    }
}

fn bo_opc_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Where an edit happens while filling in forms is enforced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormsAccess {
    /// No restriction (not enforced, or the selection is in an unprotected section).
    Free,
    /// Inside a text form field that can be filled in.
    Text(FormSpan),
    /// At a check box or drop-down field that can be filled in.
    Choice(FormSpan),
    /// Inside a content control (§17.5.2).
    Control,
    /// Anywhere else: read-only.
    Denied,
}

impl Session {
    /// The document's editing restriction (§17.15.1.29).
    #[must_use]
    pub fn protection(&self) -> Option<&DocProtection> {
        self.doc.protection.as_ref()
    }

    /// Set or clear the editing restriction (a document setting: not undoable).
    pub fn set_protection(&mut self, p: Option<DocProtection>) {
        if self.doc.protection != p {
            self.doc.protection = p;
            self.version += 1;
            // Earlier edits cannot be undone across a change of protection.
            self.undo.clear();
            self.redo.clear();
        }
    }

    /// Filling in forms is enforced (§17.15.1.29 `w:edit="forms"`).
    #[must_use]
    pub fn forms_locked(&self) -> bool {
        self.doc
            .protection
            .as_ref()
            .is_some_and(DocProtection::forms_enforced)
    }

    /// Whether body paragraph `para` is in a section left editable (§17.6.6 formProt).
    #[must_use]
    pub fn para_unprotected(&self, para: usize) -> bool {
        body_paragraphs(&self.doc)
            .get(para)
            .and_then(|p| p.first())
            .and_then(|s| self.doc.sections.get(*s))
            .is_some_and(|s| s.form_unprotected)
    }

    /// What the selection may change while filling in forms is enforced.
    #[must_use]
    pub fn forms_access(&self) -> FormsAccess {
        if !self.forms_locked() {
            return FormsAccess::Free;
        }
        let (a, b) = self.selection.ordered();
        let paths = body_paragraphs(&self.doc);
        let free = |p: usize| {
            paths
                .get(p)
                .and_then(|x| x.first())
                .and_then(|s| self.doc.sections.get(*s))
                .is_some_and(|s| s.form_unprotected)
        };
        if (a.para..=b.para).all(free) {
            return FormsAccess::Free;
        }
        if let Some(s) = self.form_at(self.selection.focus) {
            let inside =
                a.para == s.para && b.para == s.para && a.offset >= s.from && b.offset <= s.to;
            if inside && fillable(&s.field) {
                return if s.field.is_text() {
                    FormsAccess::Text(s)
                } else {
                    FormsAccess::Choice(s)
                };
            }
        }
        if a.para == b.para && self.control_at_caret().is_some() {
            return FormsAccess::Control;
        }
        FormsAccess::Denied
    }

    /// Every form field of the main text, in document order.
    #[must_use]
    pub fn form_fields(&self) -> Vec<FormSpan> {
        body_paragraphs(&self.doc)
            .iter()
            .enumerate()
            .filter_map(|(id, path)| para_at(&self.doc, path).map(|p| spans(p, id)))
            .flatten()
            .collect()
    }

    /// The form field at `pos`: inside a text field's result, or at either side of a check
    /// box / drop-down list.
    #[must_use]
    pub fn form_at(&self, pos: Pos) -> Option<FormSpan> {
        let path = body_paragraphs(&self.doc).get(pos.para)?.clone();
        let p = para_at(&self.doc, &path)?;
        let all = spans(p, pos.para);
        // The innermost text field, else an object field touching the caret.
        all.iter()
            .filter(|s| s.field.is_text() && s.from <= pos.offset && pos.offset <= s.to)
            .max_by_key(|s| s.start)
            .or_else(|| {
                all.iter()
                    .find(|s| !s.field.is_text() && s.from <= pos.offset && pos.offset <= s.to)
            })
            .cloned()
    }

    /// The form field around the caret, for the host.
    #[must_use]
    pub fn form_field_info(&self) -> Option<FormFieldInfo> {
        let s = self.form_at(self.selection.focus)?;
        let path = body_paragraphs(&self.doc).get(s.para)?.clone();
        let atoms = para::flatten(para_at(&self.doc, &path)?);
        Some(FormFieldInfo {
            value: value_of(&atoms, &s),
            fillable: fillable(&s.field),
            field: s.field,
        })
    }

    /// Insert a form field of `kind` (`text`, `checkBox`, `dropDown`) at the caret, inside a
    /// bookmark with the field's new name (`Text1`, `Check1`, `Dropdown1`, …). A text field shows
    /// five en spaces until it is filled in.
    pub fn insert_form_field(&mut self, kind: &str) -> Result<Selection> {
        let (k, prefix) = match kind {
            "text" => (
                FormFieldKind::Text(bo_model::form::FormTextInput::default()),
                "Text",
            ),
            "checkBox" => (
                FormFieldKind::CheckBox(bo_model::form::FormCheckBox::default()),
                "Check",
            ),
            "dropDown" => (
                FormFieldKind::DropDown(bo_model::form::FormDropDown::default()),
                "Dropdown",
            ),
            _ => return Err(EditError::Unsupported(format!("form field {kind}"))),
        };
        let name = new_name(&self.doc, prefix);
        let id = fields::max_bookmark_id(&self.doc) + 1;
        let (bs, be) = fields::bookmark_xml(id, &name);
        let field = FormField::new(k, Some(name));
        let text = field.is_text();
        self.edit(|tx, sel| {
            let (a, b) = sel.ordered();
            let at = tx.delete_range(a, b)?;
            let (_, _, atoms) = tx.atoms(at.para)?;
            let props = para::props_at(&atoms, at.offset);
            let mut new: Vec<Atom> = vec![
                (RunProps::default(), RunContent::Markup(bs)),
                (props.clone(), RunContent::FormField(Box::new(field))),
            ];
            if text {
                new.push((
                    props.clone(),
                    RunContent::Text(EMPTY_TEXT_RESULT.to_owned()),
                ));
            }
            new.push((props, RunContent::FormEnd));
            new.push((RunProps::default(), RunContent::Markup(be)));
            Ok(Selection::caret(tx.insert_atoms(at, new)?))
        })
    }

    /// Replace the atoms of the field `s` (marker, result, end) by `f` and `result`
    /// (`None` = the result unchanged). One undo step; the selection becomes `sel(new span)`.
    fn rewrite_field(
        &mut self,
        s: &FormSpan,
        f: FormField,
        result: Option<Vec<Atom>>,
        sel: impl FnOnce(usize, usize) -> Selection,
    ) -> Result<()> {
        let para_id = s.para;
        let (start, end, from) = (s.start, s.end, s.from);
        self.edit(|tx, _| {
            let (path, p, atoms) = tx.atoms(para_id)?;
            if !matches!(atoms.get(start), Some((_, RunContent::FormField(_))))
                || !matches!(atoms.get(end), Some((_, RunContent::FormEnd)))
            {
                return Err(EditError::BadPosition);
            }
            let mut out: Vec<Atom> = atoms[..start].to_vec();
            let text = f.is_text();
            out.push((atoms[start].0.clone(), RunContent::FormField(Box::new(f))));
            let res = result.unwrap_or_else(|| atoms[start + 1..end].to_vec());
            let n = if text { para::len(&res) } else { 0 };
            out.extend(res);
            out.extend_from_slice(&atoms[end..]);
            tx.replace_para(&path, para::with_atoms(&p, out))?;
            Ok(sel(from, from + n))
        })?;
        Ok(())
    }

    /// Change the properties of the form field at the caret (`data`: the new §17.16.17 ffData;
    /// its type must stay). A new default text replaces a result still showing the old default
    /// or the placeholder; a new name renames the field's bookmark. Returns whether a field
    /// changed.
    pub fn set_form_field(&mut self, data: &FormField) -> Result<bool> {
        let Some(s) = self.form_at(self.selection.focus) else {
            return Ok(false);
        };
        if std::mem::discriminant(&s.field.kind) != std::mem::discriminant(&data.kind) {
            return Err(EditError::Unsupported("form field type change".into()));
        }
        let mut f = FormField {
            unknown: s.field.unknown.clone(),
            instr: s.field.instr.clone(),
            separate: s.field.separate,
            locked: s.field.locked,
            ff_xml: s.field.ff_xml.clone(),
            ..data.clone()
        };
        if let FormFieldKind::DropDown(d) = &mut f.kind {
            let n = d.entries.len() as u32;
            d.result = d.result.filter(|r| *r < n);
            d.default = d.default.filter(|r| *r < n);
        }
        if f == s.field {
            return Ok(false);
        }
        let path = body_paragraphs(&self.doc)
            .get(s.para)
            .cloned()
            .ok_or(EditError::BadPosition)?;
        let atoms = para::flatten(para_at(&self.doc, &path).ok_or(EditError::BadPath)?);
        let result = match (&s.field.kind, &f.kind) {
            (FormFieldKind::Text(old), FormFieldKind::Text(new)) if old.default != new.default => {
                let cur = result_text(&atoms[s.start + 1..s.end]);
                if is_placeholder(&cur) || cur == old.default {
                    let props = atoms
                        .get(s.start + 1..s.end)
                        .and_then(|r| r.iter().find(|a| matches!(a.1, RunContent::Text(_))))
                        .map_or_else(|| atoms[s.start].0.clone(), |a| a.0.clone());
                    let t = if new.default.is_empty() {
                        EMPTY_TEXT_RESULT.to_owned()
                    } else {
                        new.default.clone()
                    };
                    Some(vec![(props, RunContent::Text(t))])
                } else {
                    None
                }
            }
            _ => None,
        };
        let rename = match (&s.field.name, &f.name) {
            (Some(o), Some(n)) if o != n => Some((o.clone(), n.clone())),
            _ => None,
        };
        let caret = self.selection;
        self.rewrite_field(&s, f, result, |_, _| caret)?;
        if let Some((o, n)) = rename {
            let sel = self.selection;
            self.edit(|tx, _| {
                for id in 0..tx.paths().len() {
                    let (path, p, mut atoms) = tx.atoms(id)?;
                    rename_bookmark(&mut atoms, &o, &n);
                    tx.replace_para(&path, para::with_atoms(&p, atoms))?;
                }
                Ok(sel)
            })?;
        }
        self.fix_selection();
        Ok(true)
    }

    /// Fill in the form field at the caret: a text field takes `value` as its result (cut to
    /// its maximum length), a check box toggles (`""`) or is set (`"1"` / `"0"`), a drop-down list
    /// selects the entry `value` (its text or index). Returns whether the field changed.
    pub fn set_form_value(&mut self, value: &str) -> Result<bool> {
        let Some(s) = self.form_at(self.selection.focus) else {
            return Ok(false);
        };
        if !fillable(&s.field) {
            return Ok(false);
        }
        let mut f = s.field.clone();
        let mut result = None;
        match &mut f.kind {
            FormFieldKind::CheckBox(c) => {
                let on = match value {
                    "" => !c.is_checked(),
                    "1" | "true" => true,
                    "0" | "false" => false,
                    _ => return Ok(false),
                };
                c.checked = Some(on);
            }
            FormFieldKind::DropDown(d) => {
                let i = d
                    .entries
                    .iter()
                    .position(|e| e == value)
                    .or_else(|| value.parse::<usize>().ok().filter(|i| *i < d.entries.len()));
                let Some(i) = i else {
                    return Ok(false);
                };
                d.result = Some(i as u32);
            }
            FormFieldKind::Text(t) => {
                let mut v: String = value.to_owned();
                if t.max_length > 0 {
                    v = v.chars().take(t.max_length as usize).collect();
                }
                if v.is_empty() {
                    v = EMPTY_TEXT_RESULT.to_owned();
                }
                let path = body_paragraphs(&self.doc)
                    .get(s.para)
                    .cloned()
                    .ok_or(EditError::BadPosition)?;
                let atoms = para::flatten(para_at(&self.doc, &path).ok_or(EditError::BadPath)?);
                let props = atoms[s.start + 1..s.end]
                    .iter()
                    .find(|a| matches!(a.1, RunContent::Text(_)))
                    .map_or_else(|| atoms[s.start].0.clone(), |a| a.0.clone());
                result = Some(vec![(props, RunContent::Text(v))]);
            }
        }
        if f == s.field && result.is_none() {
            return Ok(false);
        }
        let text = f.is_text();
        let para_id = s.para;
        self.rewrite_field(&s, f, result, |from, to| {
            if text {
                Selection::caret(Pos::new(para_id, to))
            } else {
                Selection::caret(Pos::new(para_id, from))
            }
        })?;
        Ok(true)
    }

    /// Type `text` into the text form field holding the selection (filling in a protected
    /// form): the selection is clamped to the result, a placeholder result is replaced, the
    /// maximum length (§17.16.26) is kept and a number field takes only number characters.
    /// Returns whether anything changed.
    pub fn form_type(&mut self, text: &str) -> Result<bool> {
        let FormsAccess::Text(s) = self.forms_access_any() else {
            return Ok(false);
        };
        let FormFieldKind::Text(input) = &s.field.kind else {
            return Ok(false);
        };
        let path = body_paragraphs(&self.doc)
            .get(s.para)
            .cloned()
            .ok_or(EditError::BadPosition)?;
        let atoms = para::flatten(para_at(&self.doc, &path).ok_or(EditError::BadPath)?);
        let mut res: Vec<Atom> = atoms[s.start + 1..s.end].to_vec();
        let cur = result_text(&res);
        let (a, b) = self.selection.ordered();
        let clamp = |p: Pos| match p.para.cmp(&s.para) {
            std::cmp::Ordering::Equal => p.offset.clamp(s.from, s.to) - s.from,
            std::cmp::Ordering::Less => 0,
            std::cmp::Ordering::Greater => s.to - s.from,
        };
        let (mut la, mut lb) = (clamp(a), clamp(b));
        if is_placeholder(&cur) {
            la = 0;
            lb = para::len(&res);
        }
        let mut typed: String = text
            .chars()
            .filter(|c| !matches!(c, '\n' | '\r' | '\u{2029}'))
            .collect();
        if input.input_type == TextInputType::Number {
            typed.retain(|c| c.is_ascii_digit() || matches!(c, '-' | '+' | '.' | ',' | ' ' | '%'));
        }
        let props = para::props_at(&res, la);
        let props = if res.iter().any(|x| para::atom_len(&x.1) > 0) {
            props
        } else {
            atoms[s.start].0.clone()
        };
        para::delete(&mut res, la, lb);
        if input.max_length > 0 {
            let room =
                (input.max_length as usize).saturating_sub(result_text(&res).chars().count());
            typed = typed.chars().take(room).collect();
        }
        if typed.is_empty() && la == lb {
            return Ok(false);
        }
        let n = typed.len();
        if n > 0 {
            let i = para::split_at(&mut res, la);
            res.insert(i, (props, RunContent::Text(typed)));
        }
        let para_id = s.para;
        self.rewrite_field(&s, s.field.clone(), Some(res), |from, _| {
            Selection::caret(Pos::new(para_id, from + la + n))
        })?;
        Ok(true)
    }

    /// Delete in the text form field holding the selection (filling in a protected form): the
    /// selection, else the character before (`back`) or after the caret — never past the
    /// result's ends. Returns whether anything changed.
    pub fn form_delete(&mut self, back: bool) -> Result<bool> {
        let FormsAccess::Text(s) = self.forms_access_any() else {
            return Ok(false);
        };
        let path = body_paragraphs(&self.doc)
            .get(s.para)
            .cloned()
            .ok_or(EditError::BadPosition)?;
        let atoms = para::flatten(para_at(&self.doc, &path).ok_or(EditError::BadPath)?);
        let mut res: Vec<Atom> = atoms[s.start + 1..s.end].to_vec();
        let text: String = res
            .iter()
            .map(|a| bo_model::index::item_text(&a.1))
            .collect();
        let (a, b) = self.selection.ordered();
        let (mut la, mut lb) = (
            a.offset.clamp(s.from, s.to) - s.from,
            b.offset.clamp(s.from, s.to) - s.from,
        );
        if is_placeholder(&result_text(&res)) {
            la = 0;
            lb = text.len();
        } else if la == lb {
            if back {
                let Some(c) = text[..la].chars().next_back() else {
                    return Ok(false);
                };
                la -= c.len_utf8();
            } else {
                let Some(c) = text[lb..].chars().next() else {
                    return Ok(false);
                };
                lb += c.len_utf8();
            }
        }
        if la == lb {
            return Ok(false);
        }
        para::delete(&mut res, la, lb);
        let para_id = s.para;
        self.rewrite_field(&s, s.field.clone(), Some(res), |from, _| {
            Selection::caret(Pos::new(para_id, from + la))
        })?;
        Ok(true)
    }

    /// [`Session::forms_access`] without the enforcement check (for filling in a form field
    /// whatever the protection).
    fn forms_access_any(&self) -> FormsAccess {
        let Some(s) = self.form_at(self.selection.focus) else {
            return FormsAccess::Denied;
        };
        let (a, b) = self.selection.ordered();
        let inside = a.para == s.para && b.para == s.para;
        match (inside, fillable(&s.field), s.field.is_text()) {
            (true, true, true) => FormsAccess::Text(s),
            (true, true, false) => FormsAccess::Choice(s),
            _ => FormsAccess::Denied,
        }
    }

    /// Move to the next (`back`: previous) form field that can be filled in, wrapping around,
    /// and select its result (a text field) or the field itself. Returns the field.
    pub fn next_form_field(&mut self, back: bool) -> Option<FormSpan> {
        let all: Vec<FormSpan> = self
            .form_fields()
            .into_iter()
            .filter(|s| fillable(&s.field))
            .collect();
        if all.is_empty() {
            return None;
        }
        let at = self.selection.focus;
        let (a, b) = self.selection.ordered();
        // The field selected as a whole (after a Tab), else the one at the caret.
        let cur = self
            .form_fields()
            .into_iter()
            .find(|s| a == Pos::new(s.para, s.from) && b == Pos::new(s.para, s.to) && a != b)
            .or_else(|| self.form_at(at));
        let key = |s: &FormSpan| (s.para, s.from, s.start);
        let here = cur.as_ref().map_or((at.para, at.offset, usize::MAX), key);
        let next = if back {
            all.iter()
                .rev()
                .find(|s| key(s) < here && cur.as_ref() != Some(*s))
                .or_else(|| all.last())
        } else {
            all.iter()
                .find(|s| key(s) > here && cur.as_ref() != Some(*s))
                .or_else(|| all.first())
        }?
        .clone();
        self.selection = Selection {
            anchor: Pos::new(next.para, next.from),
            focus: Pos::new(next.para, next.to),
        };
        Some(next)
    }

    /// Leaving the text field `s` while filling in a form: the format of its type is applied
    /// (§17.16.20). Returns `Ok(true)` when the field asks for fields to be recalculated
    /// (§17.16.6 calcOnExit), `Err` when its text is not valid for the type (a number / date).
    pub fn form_exit(&mut self, s: &FormSpan) -> Result<bool> {
        let FormFieldKind::Text(input) = &s.field.kind else {
            return Ok(s.field.calc_on_exit);
        };
        let Some(path) = body_paragraphs(&self.doc).get(s.para).cloned() else {
            return Ok(false);
        };
        let Some(p) = para_at(&self.doc, &path) else {
            return Ok(false);
        };
        // The field must still be where it was.
        let Some(now) = spans(p, s.para).into_iter().find(|x| x.start == s.start) else {
            return Ok(false);
        };
        let atoms = para::flatten(p);
        let cur = result_text(&atoms[now.start + 1..now.end]);
        let vi = self.field_env.vi;
        let Some(t) = formatted_result(input, &cur, vi) else {
            return Err(EditError::Unsupported(match input.input_type {
                TextInputType::Number => "formNumber".into(),
                _ => "formDate".into(),
            }));
        };
        if t != cur {
            let props = atoms[now.start + 1..now.end]
                .iter()
                .find(|a| matches!(a.1, RunContent::Text(_)))
                .map_or_else(|| atoms[now.start].0.clone(), |a| a.0.clone());
            let delta = t.len() as isize - cur.len() as isize;
            let sel = self.selection;
            let shift = |p: Pos| {
                if p.para == now.para && p.offset >= now.to {
                    Pos::new(p.para, (p.offset as isize + delta).max(0) as usize)
                } else {
                    p
                }
            };
            let after = Selection {
                anchor: shift(sel.anchor),
                focus: shift(sel.focus),
            };
            self.rewrite_field(
                &now,
                now.field.clone(),
                Some(vec![(props, RunContent::Text(t))]),
                |_, _| after,
            )?;
        }
        Ok(now.field.calc_on_exit)
    }

    /// Recalculate the computed text form fields: `calculated` ones evaluate the `=` expression
    /// of their default text against the bookmarks (§17.16.3), `currentDate` / `currentTime`
    /// show the clock; each in its format. Returns how many results changed (one undo step).
    pub fn recalc_form_fields(&mut self) -> Result<usize> {
        let env = self.fields_now();
        let marks = Bookmarks(
            fields::bookmarks(&self.doc)
                .into_iter()
                .map(|b| (b.name.to_ascii_lowercase(), b.text))
                .collect(),
        );
        let mut jobs: Vec<(FormSpan, String)> = Vec::new();
        for s in self.form_fields() {
            let FormFieldKind::Text(t) = &s.field.kind else {
                continue;
            };
            let v = match t.input_type {
                TextInputType::Calculated => {
                    let expr = t.default.trim().trim_start_matches('=');
                    match bo_model::field::formula(expr, &marks) {
                        Some(v) if t.format.is_empty() => {
                            bo_model::field::plain(&bo_model::field::FieldValue::Number(v), env.vi)
                        }
                        Some(v) => format_picture(v, &t.format),
                        None => continue,
                    }
                }
                TextInputType::CurrentDate | TextInputType::CurrentTime => {
                    let Some(now) = env.now else { continue };
                    let pic = match (t.format.is_empty(), t.input_type) {
                        (false, _) => t.format.clone(),
                        (true, TextInputType::CurrentDate) => "dd/MM/yyyy".into(),
                        _ => "HH:mm".into(),
                    };
                    format_date(&now, &pic, env.vi)
                }
                _ => continue,
            };
            jobs.push((s, v));
        }
        let sel = self.selection;
        let mut n = 0;
        self.edit(|tx, _| {
            // Later fields of a paragraph first, so earlier atom indices stay valid.
            for (s, v) in jobs.iter().rev() {
                let (path, p, atoms) = tx.atoms(s.para)?;
                if result_text(&atoms[s.start + 1..s.end]) == *v {
                    continue;
                }
                let props = atoms[s.start + 1..s.end]
                    .iter()
                    .find(|a| matches!(a.1, RunContent::Text(_)))
                    .map_or_else(|| atoms[s.start].0.clone(), |a| a.0.clone());
                let mut out: Vec<Atom> = atoms[..=s.start].to_vec();
                out.push((props, RunContent::Text(v.clone())));
                out.extend_from_slice(&atoms[s.end..]);
                tx.replace_para(&path, para::with_atoms(&p, out))?;
                n += 1;
            }
            Ok(sel)
        })?;
        self.fix_selection();
        Ok(n)
    }

    /// Set which sections stay editable while filling in forms is enforced (§17.6.6 formProt):
    /// the indices in `unprotected`. One undo step.
    pub fn set_unprotected_sections(&mut self, unprotected: &[usize]) -> Result<()> {
        let old = self.doc.sections.clone();
        let new: Vec<bo_model::Section> = old
            .iter()
            .enumerate()
            .map(|(i, s)| bo_model::Section {
                form_unprotected: unprotected.contains(&i),
                ..s.clone()
            })
            .collect();
        if new == old {
            return Ok(());
        }
        let sel = self.selection;
        self.edit(|tx, _| {
            tx.push(crate::Op::Sections {
                index: 0,
                remove: old,
                insert: new,
            })?;
            Ok(sel)
        })?;
        Ok(())
    }
}
