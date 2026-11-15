//! Interactive forms for BlueEditor's Fill & Sign and Form tab (ISO 32000-1 §12.7): the
//! AcroForm field tree with each widget's page and box, filling text fields, check boxes,
//! radio buttons and choice fields (value and a new appearance stream, text in the embedded
//! Unicode font so Vietnamese shows), resetting and flattening (appearances drawn into the
//! page content, widgets and the AcroForm removed).
use crate::interp::{mul, IDENTITY, M};
use crate::{page_matrix, resolve, text_string, PdfDocument};
use lopdf::{Dictionary, Object, ObjectId, Stream, StringFormat};
use std::collections::HashMap;

/// Kind of a form field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldKind {
    /// Text field.
    Text,
    /// Check box.
    CheckBox,
    /// One button of a radio group.
    Radio,
    /// Drop-down list.
    Combo,
    /// List box.
    List,
    /// Push button.
    Button,
    /// Signature field.
    Signature,
}

/// One widget of a form field.
#[derive(Debug, Clone, PartialEq)]
pub struct FormField {
    /// Fully qualified name (`a.b.c`).
    pub name: String,
    /// Kind.
    pub kind: FieldKind,
    /// Page index of the widget.
    pub page: usize,
    /// Widget box in page points (left, top, right, bottom).
    pub rect: [f32; 4],
    /// Text value (text and choice fields), or the selected state (buttons).
    pub value: String,
    /// Choices (combo and list boxes).
    pub options: Vec<String>,
    /// Check box / radio button: this widget is on.
    pub checked: bool,
    /// Read-only field.
    pub read_only: bool,
    /// Multi-line text field.
    pub multiline: bool,
    pub(crate) field: ObjectId,
    pub(crate) widget: ObjectId,
    /// Name of the widget's "on" appearance state.
    pub(crate) on_state: Option<Vec<u8>>,
}

fn get<'a>(doc: &'a lopdf::Document, d: &'a Dictionary, k: &[u8]) -> Option<&'a Object> {
    d.get(k).ok().map(|o| resolve(doc, o))
}

/// An attribute of a field, inherited from its parents (§12.7.3.1).
fn field_attr(doc: &lopdf::Document, id: ObjectId, key: &[u8]) -> Option<Object> {
    let mut cur = id;
    for _ in 0..32 {
        let d = doc.get_dictionary(cur).ok()?;
        if let Some(v) = get(doc, d, key) {
            return Some(v.clone());
        }
        cur = d.get(b"Parent").ok()?.as_reference().ok()?;
    }
    None
}

fn on_state(doc: &lopdf::Document, w: &Dictionary) -> Option<Vec<u8>> {
    let ap = get(doc, w, b"AP")?.as_dict().ok()?;
    let n = get(doc, ap, b"N")?.as_dict().ok()?;
    n.iter()
        .map(|(k, _)| k.clone())
        .find(|k| k.as_slice() != b"Off")
}

fn text_obj(s: &str) -> Object {
    if s.is_ascii() {
        Object::String(s.as_bytes().to_vec(), StringFormat::Literal)
    } else {
        let mut b = vec![0xfe, 0xff];
        b.extend(s.encode_utf16().flat_map(u16::to_be_bytes));
        Object::String(b, StringFormat::Hexadecimal)
    }
}

fn obj_text(o: &Object) -> String {
    match o {
        Object::String(s, _) => text_string(s),
        Object::Name(n) => String::from_utf8_lossy(n).into_owned(),
        _ => String::new(),
    }
}

fn rect_of(doc: &lopdf::Document, d: &Dictionary) -> Option<[f32; 4]> {
    let a = get(doc, d, b"Rect")?.as_array().ok()?;
    let v: Vec<f32> = a
        .iter()
        .filter_map(|o| resolve(doc, o).as_float().ok())
        .collect();
    let [x0, y0, x1, y1] = <[f32; 4]>::try_from(v.as_slice()).ok()?;
    Some([x0.min(x1), y0.min(y1), x0.max(x1), y0.max(y1)])
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut s = String::from("<");
    for b in bytes {
        let _ = write!(s, "{b:02X}");
    }
    s.push('>');
    s
}

impl PdfDocument {
    /// Widgets of every form field, in field tree order.
    pub fn form_fields(&self) -> Vec<FormField> {
        let doc = &self.doc;
        let Some(fields) = doc
            .catalog()
            .ok()
            .and_then(|c| get(doc, c, b"AcroForm"))
            .and_then(|a| a.as_dict().ok())
            .and_then(|a| get(doc, a, b"Fields"))
            .and_then(|f| f.as_array().ok())
        else {
            return Vec::new();
        };
        // Widget → page index.
        let mut page_of: HashMap<ObjectId, usize> = HashMap::new();
        for (i, p) in self.pages.iter().enumerate() {
            if let Some(a) = doc
                .get_dictionary(p.id)
                .ok()
                .and_then(|d| get(doc, d, b"Annots"))
                .and_then(|a| a.as_array().ok())
            {
                for o in a {
                    if let Ok(r) = o.as_reference() {
                        page_of.insert(r, i);
                    }
                }
            }
        }
        let mut out = Vec::new();
        let mut stack: Vec<(ObjectId, String, u32)> = fields
            .iter()
            .rev()
            .filter_map(|o| o.as_reference().ok())
            .map(|r| (r, String::new(), 0))
            .collect();
        let mut seen = std::collections::HashSet::new();
        while let Some((id, parent, depth)) = stack.pop() {
            if depth > 32 || !seen.insert(id) || out.len() > 10_000 {
                continue;
            }
            let Ok(d) = doc.get_dictionary(id) else {
                continue;
            };
            let part = get(doc, d, b"T").map(obj_text).unwrap_or_default();
            let name = match (parent.is_empty(), part.is_empty()) {
                (true, _) => part,
                (false, true) => parent.clone(),
                (false, false) => format!("{parent}.{part}"),
            };
            let kids: Vec<ObjectId> = get(doc, d, b"Kids")
                .and_then(|k| k.as_array().ok())
                .map(|a| a.iter().filter_map(|o| o.as_reference().ok()).collect())
                .unwrap_or_default();
            // Kids that are fields (have /T) are walked; widget kids belong to this field.
            let widget_kids: Vec<ObjectId> = kids
                .iter()
                .copied()
                .filter(|k| {
                    doc.get_dictionary(*k)
                        .is_ok_and(|kd| !kd.has(b"T") && (kd.has(b"Rect") || kd.has(b"Subtype")))
                })
                .collect();
            for k in kids.iter().rev() {
                if !widget_kids.contains(k) {
                    stack.push((*k, name.clone(), depth + 1));
                }
            }
            let widgets: Vec<ObjectId> = if d.has(b"Rect") {
                vec![id]
            } else {
                widget_kids
            };
            if widgets.is_empty() {
                continue;
            }
            let ft = field_attr(doc, id, b"FT");
            let flags = field_attr(doc, id, b"Ff")
                .and_then(|o| o.as_i64().ok())
                .unwrap_or(0);
            let kind = match ft.as_ref().and_then(|o| o.as_name().ok()) {
                Some(b"Tx") => FieldKind::Text,
                Some(b"Btn") if flags & (1 << 16) != 0 => FieldKind::Button,
                Some(b"Btn") if flags & (1 << 15) != 0 => FieldKind::Radio,
                Some(b"Btn") => FieldKind::CheckBox,
                Some(b"Ch") if flags & (1 << 17) != 0 => FieldKind::Combo,
                Some(b"Ch") => FieldKind::List,
                Some(b"Sig") => FieldKind::Signature,
                _ => continue,
            };
            let value = field_attr(doc, id, b"V")
                .map(|v| obj_text(&v))
                .unwrap_or_default();
            let options: Vec<String> = field_attr(doc, id, b"Opt")
                .and_then(|o| o.as_array().ok().cloned())
                .map(|a| {
                    a.iter()
                        .map(|o| match resolve(doc, o) {
                            Object::Array(pair) => pair
                                .get(1)
                                .or_else(|| pair.first())
                                .map(|x| obj_text(resolve(doc, x)))
                                .unwrap_or_default(),
                            x => obj_text(x),
                        })
                        .collect()
                })
                .unwrap_or_default();
            for w in widgets {
                let Ok(wd) = doc.get_dictionary(w) else {
                    continue;
                };
                let Some(r) = rect_of(doc, wd) else { continue };
                let page = page_of.get(&w).copied().or_else(|| {
                    let p = wd.get(b"P").ok()?.as_reference().ok()?;
                    self.pages.iter().position(|i| i.id == p)
                });
                let Some(page) = page else { continue };
                let info = &self.pages[page];
                let rect = crate::device_rect(page_matrix(info.crop, info.rotate), r);
                let on = on_state(doc, wd);
                let checked = matches!(kind, FieldKind::CheckBox | FieldKind::Radio)
                    && get(doc, wd, b"AS")
                        .and_then(|a| a.as_name().ok())
                        .is_some_and(|a| Some(a) == on.as_deref());
                out.push(FormField {
                    name: name.clone(),
                    kind,
                    page,
                    rect,
                    value: value.clone(),
                    options: options.clone(),
                    checked,
                    read_only: flags & 1 != 0,
                    multiline: kind == FieldKind::Text && flags & (1 << 12) != 0,
                    field: id,
                    widget: w,
                    on_state: on,
                });
            }
        }
        out
    }

    /// A text appearance for a widget of `w` × `h` points showing `value`.
    fn text_appearance(
        &mut self,
        w: f32,
        h: f32,
        value: &str,
        size: f32,
        multiline: bool,
    ) -> Result<ObjectId, String> {
        let mut content = String::from("/Tx BMC\n");
        let mut res = Dictionary::new();
        if !value.is_empty() {
            let size = if size > 0.0 {
                size
            } else if multiline {
                10.0
            } else {
                (h * 0.7).clamp(4.0, 12.0)
            };
            let (font, _) = self.embed_text(crate::fallback::SANS, "")?;
            let mut fonts = Dictionary::new();
            fonts.set("BoF", Object::Reference(font));
            res.set("Font", fonts);
            let lines: Vec<&str> = if multiline {
                value.lines().collect()
            } else {
                vec![value.lines().next().unwrap_or("")]
            };
            let first_y = if multiline {
                h - 2.0 - size
            } else {
                (h - size * 0.7) / 2.0
            };
            content.push_str(&format!(
                "q 1 1 {} {} re W n BT /BoF {size} Tf 0 g {} TL 2 {first_y} Td\n",
                w - 2.0,
                h - 2.0,
                size * 1.15
            ));
            for (k, line) in lines.iter().enumerate() {
                let (_, bytes) = self.embed_text(crate::fallback::SANS, line)?;
                if k > 0 {
                    content.push_str("T* ");
                }
                content.push_str(&hex(&bytes));
                content.push_str(" Tj\n");
            }
            content.push_str("ET Q\n");
        }
        content.push_str("EMC\n");
        let mut d = Dictionary::new();
        d.set("Type", Object::Name(b"XObject".to_vec()));
        d.set("Subtype", Object::Name(b"Form".to_vec()));
        d.set(
            "BBox",
            vec![0.into(), 0.into(), Object::Real(w), Object::Real(h)],
        );
        d.set("Resources", res);
        Ok(self.add(Object::Stream(Stream::new(d, content.into_bytes()))))
    }

    /// Font size from a widget's default appearance string (`/Helv 12 Tf`; 0 = auto).
    fn da_size(&self, f: &FormField) -> f32 {
        let da = field_attr(&self.doc, f.widget, b"DA")
            .or_else(|| field_attr(&self.doc, f.field, b"DA"))
            .map(|o| obj_text(&o))
            .unwrap_or_default();
        let parts: Vec<&str> = da.split_whitespace().collect();
        parts
            .iter()
            .position(|p| *p == "Tf")
            .and_then(|i| i.checked_sub(1))
            .and_then(|i| parts[i].parse().ok())
            .unwrap_or(0.0)
    }

    fn widget_size(&self, widget: ObjectId) -> (f32, f32) {
        self.doc
            .get_dictionary(widget)
            .ok()
            .and_then(|d| rect_of(&self.doc, d))
            .map_or((100.0, 20.0), |r| (r[2] - r[0], r[3] - r[1]))
    }

    /// Set a text or choice field's value (every widget of the field gets a new appearance).
    pub fn set_field_text(&mut self, f: &FormField, value: &str) -> Result<(), String> {
        if f.read_only {
            return Err("the field is read-only".into());
        }
        self.begin("fill field");
        let r = (|| {
            self.touch(f.field);
            if let Ok(d) = self.doc.get_dictionary_mut(f.field) {
                d.set("V", text_obj(value));
            }
            let widgets: Vec<FormField> = self
                .form_fields()
                .into_iter()
                .filter(|x| x.field == f.field)
                .collect();
            for w in widgets {
                let (ww, hh) = self.widget_size(w.widget);
                let size = self.da_size(&w);
                let ap = self.text_appearance(ww, hh, value, size, w.multiline)?;
                self.touch(w.widget);
                if let Ok(d) = self.doc.get_dictionary_mut(w.widget) {
                    let mut apd = Dictionary::new();
                    apd.set("N", Object::Reference(ap));
                    d.set("AP", apd);
                }
            }
            Ok(())
        })();
        self.commit();
        r
    }

    /// Turn a check box on or off, or choose this radio button (`on` true).
    pub fn set_field_checked(&mut self, f: &FormField, on: bool) -> Result<(), String> {
        if f.read_only {
            return Err("the field is read-only".into());
        }
        let state = f.on_state.clone().unwrap_or_else(|| b"Yes".to_vec());
        self.begin("fill field");
        let widgets: Vec<FormField> = self
            .form_fields()
            .into_iter()
            .filter(|x| x.field == f.field)
            .collect();
        let value = if on { state.clone() } else { b"Off".to_vec() };
        self.touch(f.field);
        if let Ok(d) = self.doc.get_dictionary_mut(f.field) {
            d.set("V", Object::Name(value.clone()));
        }
        for w in widgets {
            let this = w.widget == f.widget || f.kind == FieldKind::CheckBox;
            let s = match (&w.on_state, this && on) {
                (Some(s), true) if f.kind == FieldKind::Radio => s.clone(),
                (Some(_), true) => value.clone(),
                _ => b"Off".to_vec(),
            };
            self.touch(w.widget);
            if let Ok(d) = self.doc.get_dictionary_mut(w.widget) {
                d.set("AS", Object::Name(s));
            }
        }
        self.commit();
        Ok(())
    }

    /// Clear every field (text empty, buttons off, choices unset).
    pub fn reset_form(&mut self) -> Result<(), String> {
        let fields = self.form_fields();
        self.begin("reset form");
        let r = (|| {
            for f in &fields {
                match f.kind {
                    FieldKind::Text | FieldKind::Combo | FieldKind::List => {
                        if !f.value.is_empty() {
                            self.touch(f.field);
                            if let Ok(d) = self.doc.get_dictionary_mut(f.field) {
                                d.remove(b"V");
                            }
                            let (w, h) = self.widget_size(f.widget);
                            let ap = self.text_appearance(w, h, "", 0.0, false)?;
                            self.touch(f.widget);
                            if let Ok(d) = self.doc.get_dictionary_mut(f.widget) {
                                let mut apd = Dictionary::new();
                                apd.set("N", Object::Reference(ap));
                                d.set("AP", apd);
                            }
                        }
                    }
                    FieldKind::CheckBox | FieldKind::Radio => {
                        self.touch(f.field);
                        if let Ok(d) = self.doc.get_dictionary_mut(f.field) {
                            d.set("V", Object::Name(b"Off".to_vec()));
                        }
                        self.touch(f.widget);
                        if let Ok(d) = self.doc.get_dictionary_mut(f.widget) {
                            d.set("AS", Object::Name(b"Off".to_vec()));
                        }
                    }
                    _ => {}
                }
            }
            Ok(())
        })();
        self.commit();
        r
    }

    /// Draw every widget's appearance into its page and remove the form (fields can no longer
    /// be changed). Returns how many widgets were flattened.
    pub fn flatten_form(&mut self) -> Result<usize, String> {
        let fields = self.form_fields();
        if fields.is_empty() {
            return Ok(0);
        }
        self.begin("flatten form");
        let r = (|| {
            let mut by_page: HashMap<usize, Vec<FormField>> = HashMap::new();
            for f in fields {
                by_page.entry(f.page).or_default().push(f);
            }
            let mut n = 0;
            let mut pages: Vec<usize> = by_page.keys().copied().collect();
            pages.sort_unstable();
            for page in pages {
                let mut tail = String::new();
                let mut widgets = Vec::new();
                for f in &by_page[&page] {
                    widgets.push(f.widget);
                    let Some((ap, m)) = self.widget_placement(f.widget) else {
                        continue;
                    };
                    let name = self.add_resource(page, b"XObject", "BoFx", ap)?;
                    tail.push_str(&format!(
                        "\nq {} {} {} {} {} {} cm /{} Do Q\n",
                        m[0],
                        m[1],
                        m[2],
                        m[3],
                        m[4],
                        m[5],
                        String::from_utf8_lossy(&name)
                    ));
                    n += 1;
                }
                let (sid, content) = self.page_stream(page)?;
                self.write_page(sid, content, Vec::new(), tail.as_bytes());
                self.remove_annots(page, &widgets)?;
            }
            let root = self
                .doc
                .trailer
                .get(b"Root")
                .and_then(Object::as_reference)
                .map_err(|e| e.to_string())?;
            self.touch(root);
            if let Ok(c) = self.doc.get_dictionary_mut(root) {
                c.remove(b"AcroForm");
            }
            Ok(n)
        })();
        self.commit();
        r
    }

    /// The appearance stream a widget shows and the matrix placing its box on `/Rect`
    /// (Algorithm 8.1), in default user space.
    pub(crate) fn widget_placement(&self, widget: ObjectId) -> Option<(ObjectId, M)> {
        let doc = &self.doc;
        let d = doc.get_dictionary(widget).ok()?;
        if get(doc, d, b"F")
            .and_then(|f| f.as_i64().ok())
            .is_some_and(|f| f & 2 != 0)
        {
            return None;
        }
        let rect = rect_of(doc, d)?;
        let ap = get(doc, d, b"AP")?.as_dict().ok()?;
        let n = ap.get(b"N").ok()?;
        let id = match n {
            Object::Reference(r) if doc.get_object(*r).is_ok_and(|o| o.as_stream().is_ok()) => *r,
            o => {
                let states = resolve(doc, o).as_dict().ok()?;
                let st = get(doc, d, b"AS")?.as_name().ok()?;
                states.get(st).ok()?.as_reference().ok()?
            }
        };
        let s = doc.get_object(id).ok()?.as_stream().ok()?;
        let nums = |k: &[u8]| -> Vec<f32> {
            s.dict
                .get(k)
                .ok()
                .map(|o| resolve(doc, o))
                .and_then(|o| o.as_array().ok())
                .map(|a| {
                    a.iter()
                        .filter_map(|o| resolve(doc, o).as_float().ok())
                        .collect()
                })
                .unwrap_or_default()
        };
        let matrix = <[f32; 6]>::try_from(nums(b"Matrix").as_slice()).unwrap_or(IDENTITY);
        let bbox = <[f32; 4]>::try_from(nums(b"BBox").as_slice()).ok()?;
        let pts = [
            crate::interp::apply(matrix, bbox[0], bbox[1]),
            crate::interp::apply(matrix, bbox[2], bbox[3]),
            crate::interp::apply(matrix, bbox[0], bbox[3]),
            crate::interp::apply(matrix, bbox[2], bbox[1]),
        ];
        let x0 = pts.iter().map(|p| p.0).fold(f32::INFINITY, f32::min);
        let x1 = pts.iter().map(|p| p.0).fold(f32::NEG_INFINITY, f32::max);
        let y0 = pts.iter().map(|p| p.1).fold(f32::INFINITY, f32::min);
        let y1 = pts.iter().map(|p| p.1).fold(f32::NEG_INFINITY, f32::max);
        if !(x1 - x0 > 1e-3 && y1 - y0 > 1e-3) {
            return None;
        }
        let sx = (rect[2] - rect[0]) / (x1 - x0);
        let sy = (rect[3] - rect[1]) / (y1 - y0);
        // The XObject applies its own /Matrix; the placement maps the transformed box.
        let a = [sx, 0.0, 0.0, sy, rect[0] - x0 * sx, rect[1] - y0 * sy];
        Some((id, mul(IDENTITY, a)))
    }

    /// Remove annotation references `ids` from page `index`'s `/Annots`.
    pub(crate) fn remove_annots(&mut self, index: usize, ids: &[ObjectId]) -> Result<(), String> {
        let page_id = self.pages.get(index).ok_or("no such page")?.id;
        let annots = self
            .doc
            .get_dictionary(page_id)
            .map_err(|e| e.to_string())?
            .get(b"Annots")
            .ok()
            .cloned();
        let keep = |a: &mut Vec<Object>| {
            a.retain(|o| o.as_reference().map_or(true, |r| !ids.contains(&r)));
        };
        match annots {
            Some(Object::Reference(r)) => {
                self.touch(r);
                if let Ok(Object::Array(a)) = self.doc.get_object_mut(r) {
                    keep(a);
                }
            }
            Some(Object::Array(_)) => {
                self.touch(page_id);
                if let Ok(d) = self.doc.get_dictionary_mut(page_id) {
                    if let Ok(Object::Array(a)) = d.get_mut(b"Annots") {
                        keep(a);
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }
}
