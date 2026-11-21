//! BlueEditor round 12: copying form fields as the reference editor's field context menu does —
//! Cut / Copy / Paste, Create Multiple Copies (a grid of copies, own names or the same name) and
//! Duplicate (the same field on other pages). ISO 32000-1 §12.7.3.1 field dictionaries
//! (inheritable entries, `/Kids`, `/Parent`, partial names without periods), §12.5.6.19 widget
//! annotations, §12.7.2 the AcroForm `/Fields` array, §12.6.3 Table 196 field / widget triggers.
use crate::annot::{nums, text_obj};
use crate::forms::{FieldKind, FormField};
use crate::{resolve, text_string, PdfDocument};

use lopdf::{Dictionary, Object, ObjectId};
use std::collections::HashMap;

/// Entries of a terminal field (Table 220, Table 222 variable text, Table 229 text, Table 231
/// choice), as opposed to its widget annotation's.
const FIELD_KEYS: [&[u8]; 12] = [
    b"FT", b"T", b"TU", b"TM", b"Ff", b"V", b"DV", b"Opt", b"TI", b"I", b"MaxLen", b"RV",
];

/// Inheritable entries a widget takes from its parents when it has none.
const INHERITED: [&[u8]; 10] = [
    b"FT", b"Ff", b"V", b"DV", b"DA", b"Q", b"Opt", b"MaxLen", b"TU", b"TM",
];

/// Field triggers of `/AA` (Table 196: keystroke, format, validate, calculate); the others
/// (E, X, D, U, Fo, Bl, PO, PC, PV, PI) belong to the widget.
const FIELD_TRIGGERS: [&[u8]; 4] = [b"K", b"F", b"V", b"C"];

/// A form field on the clipboard: its widget dictionary with its field's entries merged in,
/// and every object it refers to (appearance streams, actions), so it can be pasted into this
/// document or another one.
#[derive(Debug, Clone)]
pub struct FieldClip {
    /// Fully qualified name it had.
    pub name: String,
    /// Kind.
    pub kind: FieldKind,
    /// Box in PDF user space (left, bottom, right, top).
    pub rect: [f32; 4],
    root: Dictionary,
    objects: Vec<(ObjectId, Object)>,
}

fn is_page(o: &Object) -> bool {
    let Object::Dictionary(d) = o else {
        return false;
    };
    matches!(
        d.get(b"Type").and_then(Object::as_name),
        Ok(b"Page" | b"Pages")
    )
}

fn rewrite(o: &Object, map: &HashMap<ObjectId, ObjectId>) -> Object {
    match o {
        Object::Reference(id) => Object::Reference(*map.get(id).unwrap_or(id)),
        Object::Array(a) => Object::Array(a.iter().map(|x| rewrite(x, map)).collect()),
        Object::Dictionary(d) => Object::Dictionary(rewrite_dict(d, map)),
        Object::Stream(s) => {
            let mut s = s.clone();
            s.dict = rewrite_dict(&s.dict, map);
            Object::Stream(s)
        }
        other => other.clone(),
    }
}

fn rewrite_dict(d: &Dictionary, map: &HashMap<ObjectId, ObjectId>) -> Dictionary {
    let mut n = Dictionary::new();
    for (k, v) in d {
        n.set(k.clone(), rewrite(v, map));
    }
    n
}

/// `name` without a trailing `_<digits>`.
fn stem(name: &str) -> &str {
    match name.rsplit_once('_') {
        Some((a, b)) if !b.is_empty() && b.bytes().all(|c| c.is_ascii_digit()) => a,
        _ => name,
    }
}

impl PdfDocument {
    /// Objects `o` refers to (not through `/P` or `/Parent`, and never pages), deepest last.
    fn collect12(&self, o: &Object, seen: &mut Vec<ObjectId>, out: &mut Vec<(ObjectId, Object)>) {
        match o {
            Object::Reference(id) => {
                if seen.contains(id) || seen.len() > 4096 {
                    return;
                }
                seen.push(*id);
                let Ok(obj) = self.doc.get_object(*id) else {
                    return;
                };
                if is_page(obj) {
                    return;
                }
                let obj = obj.clone();
                self.collect12(&obj, seen, out);
                out.push((*id, obj));
            }
            Object::Array(a) => {
                for x in a {
                    self.collect12(x, seen, out);
                }
            }
            Object::Dictionary(d) => {
                for (k, v) in d {
                    if k != b"P" && k != b"Parent" {
                        self.collect12(v, seen, out);
                    }
                }
            }
            Object::Stream(s) => {
                for (k, v) in &s.dict {
                    if k != b"P" && k != b"Parent" {
                        self.collect12(v, seen, out);
                    }
                }
            }
            _ => {}
        }
    }

    /// Add copies of `objects` under new numbers; the old → new map.
    fn place12(&mut self, objects: &[(ObjectId, Object)]) -> HashMap<ObjectId, ObjectId> {
        let mut map = HashMap::new();
        for (old, _) in objects {
            let id = self.add(Object::Null);
            map.insert(*old, id);
        }
        for (old, o) in objects {
            let new = rewrite(o, &map);
            if let Some(id) = map.get(old) {
                if let Ok(slot) = self.doc.get_object_mut(*id) {
                    *slot = new;
                }
            }
        }
        map
    }

    /// A point of page `page` in page points (origin top left, as shown after `/Rotate`) in
    /// PDF user space.
    #[must_use]
    pub fn page_to_user(&self, page: usize, x: f32, y: f32) -> Option<(f32, f32)> {
        let info = self.pages.get(page)?;
        let inv = crate::interp::invert(crate::page_matrix(info.crop, info.rotate))?;
        Some(crate::interp::apply(inv, x, y))
    }

    /// Copy widget `f` with its field's entries (Copy / Cut).
    #[must_use]
    pub fn copy_field(&self, f: &FormField) -> Option<FieldClip> {
        let mut root = self.doc.get_dictionary(f.widget).ok()?.clone();
        root.remove(b"Parent");
        root.remove(b"P");
        root.remove(b"Kids");
        // Entries the widget inherits from its field and the field's parents.
        let mut cur = self
            .doc
            .get_dictionary(f.widget)
            .ok()?
            .get(b"Parent")
            .ok()
            .and_then(|p| p.as_reference().ok());
        let mut guard = 0;
        while let Some(id) = cur {
            guard += 1;
            let Ok(d) = self.doc.get_dictionary(id) else {
                break;
            };
            if guard > 32 {
                break;
            }
            for k in INHERITED {
                if !root.has(k) {
                    if let Ok(v) = d.get(k) {
                        root.set(k.to_vec(), v.clone());
                    }
                }
            }
            // The field's keystroke / format / validate / calculate scripts.
            if let Ok(aa) = d.get(b"AA") {
                if let Ok(aa) = resolve(&self.doc, aa).as_dict() {
                    let mut mine = root
                        .get(b"AA")
                        .ok()
                        .map(|o| resolve(&self.doc, o))
                        .and_then(|o| o.as_dict().ok().cloned())
                        .unwrap_or_default();
                    for t in FIELD_TRIGGERS {
                        if !mine.has(t) {
                            if let Ok(v) = aa.get(t) {
                                mine.set(t.to_vec(), v.clone());
                            }
                        }
                    }
                    if !mine.is_empty() {
                        root.set("AA", mine);
                    }
                }
            }
            cur = d.get(b"Parent").ok().and_then(|p| p.as_reference().ok());
        }
        // A partial name has no periods (§12.7.3.2).
        root.set("T", text_obj(&f.name.replace('.', "_")));
        let rect = self.field_rect(f);
        let mut seen = Vec::new();
        let mut objects = Vec::new();
        self.collect12(&Object::Dictionary(root.clone()), &mut seen, &mut objects);
        Some(FieldClip {
            name: f.name.clone(),
            kind: f.kind,
            rect,
            root,
            objects,
        })
    }

    /// A name for a new field like `base`: `base` itself when free, else `<stem>_<n>`.
    fn free_name12(&self, base: &str, taken: &mut Vec<String>) -> String {
        if taken.is_empty() {
            *taken = self.form_fields().into_iter().map(|f| f.name).collect();
        }
        let base = base.replace('.', "_");
        let name = if taken.contains(&base) {
            let s = stem(&base).to_owned();
            (1..=taken.len() + 1)
                .map(|k| format!("{s}_{k}"))
                .find(|n| !taken.contains(n))
                .unwrap_or(base)
        } else {
            base
        };
        taken.push(name.clone());
        name
    }

    /// Put widget `id` in page `page`'s `/Annots`.
    fn append_annot12(&mut self, page: usize, id: ObjectId) -> Result<(), String> {
        let page_id = self.pages.get(page).ok_or("no such page")?.id;
        let annots = self
            .doc
            .get_dictionary(page_id)
            .map_err(|e| e.to_string())?
            .get(b"Annots")
            .ok()
            .cloned();
        if let Some(Object::Reference(arr)) = annots {
            self.touch(arr);
            if let Ok(Object::Array(v)) = self.doc.get_object_mut(arr) {
                v.push(Object::Reference(id));
                return Ok(());
            }
        }
        self.touch(page_id);
        let d = self
            .doc
            .get_dictionary_mut(page_id)
            .map_err(|e| e.to_string())?;
        match d.get_mut(b"Annots") {
            Ok(Object::Array(v)) => v.push(Object::Reference(id)),
            _ => d.set("Annots", vec![Object::Reference(id)]),
        }
        Ok(())
    }

    /// The AcroForm dictionary's object (made indirect when it is not yet).
    fn acroform12(&mut self) -> Result<ObjectId, String> {
        let root = self
            .doc
            .trailer
            .get(b"Root")
            .and_then(Object::as_reference)
            .map_err(|e| e.to_string())?;
        let acro = self
            .doc
            .get_dictionary(root)
            .ok()
            .and_then(|c| c.get(b"AcroForm").ok().cloned());
        if let Some(Object::Reference(id)) = acro {
            return Ok(id);
        }
        let d = if let Some(Object::Dictionary(d)) = acro {
            d
        } else {
            let mut d = Dictionary::new();
            d.set("Fields", Vec::<Object>::new());
            d
        };
        let id = self.add(Object::Dictionary(d));
        self.touch(root);
        if let Ok(c) = self.doc.get_dictionary_mut(root) {
            c.set("AcroForm", Object::Reference(id));
        }
        Ok(id)
    }

    /// Append `id` to array entry `key` of dictionary object `holder`; or, with `replace`, put
    /// it where `replace` was.
    fn push_ref12(
        &mut self,
        holder: ObjectId,
        key: &[u8],
        id: ObjectId,
        replace: Option<ObjectId>,
    ) {
        let inner = self
            .doc
            .get_dictionary(holder)
            .ok()
            .and_then(|d| d.get(key).ok())
            .and_then(|o| o.as_reference().ok());
        let target = inner.unwrap_or(holder);
        self.touch(target);
        let arr = match inner {
            Some(r) => self.doc.get_object_mut(r).ok(),
            None => self.doc.get_dictionary_mut(holder).ok().and_then(|d| {
                if !d.has(key) {
                    d.set(key.to_vec(), Vec::<Object>::new());
                }
                d.get_mut(key).ok()
            }),
        };
        if let Some(Object::Array(v)) = arr {
            if let Some(old) = replace {
                if let Some(slot) = v.iter_mut().find(|o| o.as_reference().ok() == Some(old)) {
                    *slot = Object::Reference(id);
                    return;
                }
            }
            v.push(Object::Reference(id));
        }
    }

    /// Paste clipboard fields on page `page`, moved by (`dx`, `dy`) points in user space, each
    /// under a new name (`<name>_<n>` when taken); one undo step. The new fields' names.
    ///
    /// # Errors
    /// When there is no such page.
    pub fn paste_fields(
        &mut self,
        page: usize,
        clips: &[FieldClip],
        dx: f32,
        dy: f32,
    ) -> Result<Vec<String>, String> {
        let page_id = self.pages.get(page).ok_or("no such page")?.id;
        self.begin("paste fields");
        let acro = self.acroform12()?;
        let mut taken = Vec::new();
        let mut names = Vec::new();
        for c in clips {
            let map = self.place12(&c.objects);
            let mut d = rewrite_dict(&c.root, &map);
            let r = c.rect;
            d.set("Rect", nums(&[r[0] + dx, r[1] + dy, r[2] + dx, r[3] + dy]));
            d.set("P", Object::Reference(page_id));
            let base = d
                .get(b"T")
                .and_then(Object::as_str)
                .map_or_else(|_| c.name.clone(), text_string);
            let name = self.free_name12(&base, &mut taken);
            d.set("T", text_obj(&name));
            let id = self.add(Object::Dictionary(d));
            self.append_annot12(page, id)?;
            self.push_ref12(acro, b"Fields", id, None);
            names.push(name);
        }
        self.commit();
        Ok(names)
    }

    /// Make merged field + widget `widget` a field with one kid widget (§12.7.3.1), so more
    /// widgets can share it; the field's id.
    fn split_field12(&mut self, widget: ObjectId) -> Result<ObjectId, String> {
        let mut w = self
            .doc
            .get_dictionary(widget)
            .map_err(|e| e.to_string())?
            .clone();
        let mut f = Dictionary::new();
        for k in FIELD_KEYS {
            if let Ok(v) = w.get(k) {
                f.set(k.to_vec(), v.clone());
            }
            w.remove(k);
        }
        // Variable text entries stay on the widget and are given to the field too.
        for k in [b"DA".as_slice(), b"Q".as_slice()] {
            if let Ok(v) = w.get(k) {
                f.set(k.to_vec(), v.clone());
            }
        }
        if let Some(aa) = w
            .get(b"AA")
            .ok()
            .map(|o| resolve(&self.doc, o))
            .and_then(|o| o.as_dict().ok().cloned())
        {
            let mut field_aa = Dictionary::new();
            let mut widget_aa = Dictionary::new();
            for (k, v) in &aa {
                if FIELD_TRIGGERS.contains(&k.as_slice()) {
                    field_aa.set(k.clone(), v.clone());
                } else {
                    widget_aa.set(k.clone(), v.clone());
                }
            }
            if field_aa.is_empty() {
                w.set("AA", widget_aa);
            } else {
                f.set("AA", field_aa);
                if widget_aa.is_empty() {
                    w.remove(b"AA");
                } else {
                    w.set("AA", widget_aa);
                }
            }
        }
        let old_parent = w.get(b"Parent").ok().and_then(|p| p.as_reference().ok());
        if let Some(p) = old_parent {
            f.set("Parent", Object::Reference(p));
        }
        f.set("Kids", vec![Object::Reference(widget)]);
        let fid = self.add(Object::Dictionary(f));
        w.set("Parent", Object::Reference(fid));
        self.touch(widget);
        if let Ok(slot) = self.doc.get_object_mut(widget) {
            *slot = Object::Dictionary(w);
        }
        if let Some(p) = old_parent {
            self.push_ref12(p, b"Kids", fid, Some(widget));
        } else {
            let acro = self.acroform12()?;
            self.push_ref12(acro, b"Fields", fid, Some(widget));
        }
        Ok(fid)
    }

    /// New widgets of `f`'s field at `at` (page, box in user space): the same field — one value
    /// — shown in more places (Duplicate, Create Multiple Copies with Same name); one undo
    /// step. How many were added.
    ///
    /// # Errors
    /// When a page is missing.
    pub fn add_widgets(
        &mut self,
        f: &FormField,
        at: &[(usize, [f32; 4])],
    ) -> Result<usize, String> {
        if at.is_empty() {
            return Ok(0);
        }
        self.begin("duplicate fields");
        let field = if f.field == f.widget {
            self.split_field12(f.widget)?
        } else {
            f.field
        };
        let mut tmpl = self
            .doc
            .get_dictionary(f.widget)
            .map_err(|e| e.to_string())?
            .clone();
        tmpl.remove(b"P");
        tmpl.remove(b"Parent");
        let mut n = 0;
        for (page, rect) in at {
            let page_id = self.pages.get(*page).ok_or("no such page")?.id;
            let mut seen = Vec::new();
            let mut objs = Vec::new();
            self.collect12(&Object::Dictionary(tmpl.clone()), &mut seen, &mut objs);
            let map = self.place12(&objs);
            let mut d = rewrite_dict(&tmpl, &map);
            d.set("Rect", nums(rect));
            d.set("P", Object::Reference(page_id));
            d.set("Parent", Object::Reference(field));
            let id = self.add(Object::Dictionary(d));
            self.push_ref12(field, b"Kids", id, None);
            self.append_annot12(*page, id)?;
            n += 1;
        }
        self.commit();
        Ok(n)
    }

    /// Open one undo step that inner steps join; whether a group was open already.
    fn group_on12(&mut self, label: &str) -> bool {
        let was = self.edit.group;
        self.begin(label);
        self.edit.group = true;
        was
    }

    /// Close the step of [`Self::group_on12`].
    fn group_off12(&mut self, was: bool) {
        self.edit.group = was;
        if !was {
            self.commit();
        }
    }

    /// Create Multiple Copies: `down` × `across` copies of each of `fields` (the originals
    /// count as the first cell), cells `gap` points (horizontal, vertical) apart, the grid moved
    /// by `shift`; with `same_name` the copies are widgets of the same field, else new fields
    /// `<name>_<n>`. One undo step; how many fields were added.
    ///
    /// # Errors
    /// When a page is missing.
    pub fn field_copies(
        &mut self,
        fields: &[FormField],
        down: usize,
        across: usize,
        gap: (f32, f32),
        shift: (f32, f32),
        same_name: bool,
    ) -> Result<usize, String> {
        let boxes: Vec<[f32; 4]> = fields.iter().map(|f| self.field_rect(f)).collect();
        let Some(first) = boxes.first() else {
            return Ok(0);
        };
        // The selection's bounding box is one cell.
        let x0 = boxes.iter().map(|b| b[0]).fold(first[0], f32::min);
        let x1 = boxes.iter().map(|b| b[2]).fold(first[2], f32::max);
        let y0 = boxes.iter().map(|b| b[1]).fold(first[1], f32::min);
        let y1 = boxes.iter().map(|b| b[3]).fold(first[3], f32::max);
        let (cw, ch) = (x1 - x0 + gap.0, y1 - y0 + gap.1);
        let clips: Vec<FieldClip> = fields.iter().filter_map(|f| self.copy_field(f)).collect();
        let was = self.group_on12("multiple copies");
        let mut moves = Vec::new();
        if shift != (0.0, 0.0) {
            for (f, b) in fields.iter().zip(&boxes) {
                moves.push((
                    f.clone(),
                    [
                        b[0] + shift.0,
                        b[1] + shift.1,
                        b[2] + shift.0,
                        b[3] + shift.1,
                    ],
                ));
            }
        }
        if let Err(e) = self.set_field_rects(&moves) {
            self.group_off12(was);
            return Err(e);
        }
        let mut n = 0;
        for row in 0..down.max(1) {
            for col in 0..across.max(1) {
                if row == 0 && col == 0 {
                    continue;
                }
                let dx = shift.0 + col as f32 * cw;
                let dy = shift.1 - row as f32 * ch;
                if same_name {
                    for (f, b) in fields.iter().zip(&boxes) {
                        // Look the widget up again: the first copy splits a merged field.
                        let Some(now) = self
                            .form_fields()
                            .into_iter()
                            .find(|x| x.widget == f.widget)
                        else {
                            continue;
                        };
                        let r = [b[0] + dx, b[1] + dy, b[2] + dx, b[3] + dy];
                        match self.add_widgets(&now, &[(f.page, r)]) {
                            Ok(k) => n += k,
                            Err(e) => {
                                self.group_off12(was);
                                return Err(e);
                            }
                        }
                    }
                    continue;
                }
                match self.paste_fields(fields[0].page, &clips, dx, dy) {
                    Ok(v) => n += v.len(),
                    Err(e) => {
                        self.group_off12(was);
                        return Err(e);
                    }
                }
            }
        }
        self.group_off12(was);
        Ok(n)
    }

    /// Duplicate: each of `fields` as a widget of the same field on every page of `pages`
    /// other than its own, at the same place; one undo step. How many widgets were added.
    ///
    /// # Errors
    /// When a page is missing.
    pub fn duplicate_fields(
        &mut self,
        fields: &[FormField],
        pages: &[usize],
    ) -> Result<usize, String> {
        let was = self.group_on12("duplicate fields");
        let mut n = 0;
        let names: Vec<(String, ObjectId)> =
            fields.iter().map(|f| (f.name.clone(), f.widget)).collect();
        for (name, widget) in names {
            // Look the widget up again: an earlier one may have split a shared field.
            let Some(f) = self
                .form_fields()
                .into_iter()
                .find(|x| x.widget == widget && x.name == name)
            else {
                continue;
            };
            let r = self.field_rect(&f);
            let at: Vec<(usize, [f32; 4])> = pages
                .iter()
                .filter(|&&p| p != f.page && p < self.pages.len())
                .map(|&p| (p, r))
                .collect();
            match self.add_widgets(&f, &at) {
                Ok(k) => n += k,
                Err(e) => {
                    self.group_off12(was);
                    return Err(e);
                }
            }
        }
        self.group_off12(was);
        Ok(n)
    }
}

#[cfg(test)]
#[path = "fields12_tests.rs"]
mod tests;
