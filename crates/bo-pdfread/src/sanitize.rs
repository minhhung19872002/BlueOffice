//! Protect › Hidden Data and Form tab extras for BlueEditor: remove metadata (§14.3 document
//! information and XMP), file attachments (§7.11.4 embedded files, §12.5.6.15 file attachment
//! annotations), comments, form fields, hidden layers (§8.11 optional content: content marked
//! with a group that is off is deleted, then `/OCProperties`), bookmarks, links and document
//! JavaScript / actions (§12.6); the calculation order of fields (§12.7.2 `/CO`), field
//! tooltips (`/TU`) and simple button actions (§12.6.4.10 hide, §12.6.4.13 reset form).
use crate::forms::FormField;
use crate::{resolve, PdfDocument};
use lopdf::content::Content;
use lopdf::{Dictionary, Object, ObjectId};
use std::collections::HashSet;

/// What Hidden Data removes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[allow(clippy::struct_excessive_bools)]
pub struct HiddenData {
    /// Document information and XMP metadata.
    pub metadata: bool,
    /// Embedded files and file attachment comments.
    pub attachments: bool,
    /// Comments (annotations other than links and form widgets).
    pub comments: bool,
    /// Form fields and their data.
    pub forms: bool,
    /// Content in optional content groups that are off, and the layer list.
    pub hidden_layers: bool,
    /// Bookmarks.
    pub bookmarks: bool,
    /// Links.
    pub links: bool,
    /// Document JavaScript and automatic actions.
    pub scripts: bool,
}

impl HiddenData {
    /// Everything.
    pub fn all() -> Self {
        Self {
            metadata: true,
            attachments: true,
            comments: true,
            forms: true,
            hidden_layers: true,
            bookmarks: true,
            links: true,
            scripts: true,
        }
    }
}

/// An action of a push button.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ButtonAction {
    /// Clear every field.
    ResetForm,
    /// Hide (`true`) or show the fields named.
    Hide(Vec<String>, bool),
}

/// What the document has of each kind (for the Hidden Data dialog).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HiddenCounts {
    /// Document information entries and XMP.
    pub metadata: usize,
    /// Embedded files and attachment comments.
    pub attachments: usize,
    /// Comments.
    pub comments: usize,
    /// Form widgets.
    pub forms: usize,
    /// Optional content groups that are off.
    pub hidden_layers: usize,
    /// Top-level bookmarks.
    pub bookmarks: usize,
    /// Links.
    pub links: usize,
    /// Scripts and automatic actions.
    pub scripts: usize,
}

impl PdfDocument {
    pub(crate) fn catalog_root(&self) -> Result<ObjectId, String> {
        self.doc
            .trailer
            .get(b"Root")
            .and_then(Object::as_reference)
            .map_err(|e| e.to_string())
    }

    pub(crate) fn catalog_dict(&self) -> Option<&Dictionary> {
        self.doc.catalog().ok()
    }

    fn page_annots(&self, page: ObjectId) -> Vec<(ObjectId, Vec<u8>)> {
        self.doc
            .get_dictionary(page)
            .ok()
            .and_then(|d| d.get(b"Annots").ok())
            .and_then(|a| resolve(&self.doc, a).as_array().ok())
            .map(|a| {
                a.iter()
                    .filter_map(|o| o.as_reference().ok())
                    .map(|id| {
                        let st = self
                            .doc
                            .get_dictionary(id)
                            .ok()
                            .and_then(|d| d.get(b"Subtype").ok())
                            .and_then(|s| s.as_name().ok())
                            .map(<[u8]>::to_vec)
                            .unwrap_or_default();
                        (id, st)
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Names of the optional content groups that are off by default (§8.11.4.3 `/D /OFF`).
    fn off_groups(&self) -> HashSet<ObjectId> {
        let Some(oc) = self
            .catalog_dict()
            .and_then(|c| c.get(b"OCProperties").ok())
            .and_then(|o| resolve(&self.doc, o).as_dict().ok())
        else {
            return HashSet::new();
        };
        let d = oc
            .get(b"D")
            .ok()
            .and_then(|o| resolve(&self.doc, o).as_dict().ok());
        let off: HashSet<ObjectId> = d
            .and_then(|d| d.get(b"OFF").ok())
            .and_then(|o| resolve(&self.doc, o).as_array().ok())
            .map(|a| a.iter().filter_map(|o| o.as_reference().ok()).collect())
            .unwrap_or_default();
        off
    }

    /// How much of each kind of hidden data the document has.
    pub fn hidden_counts(&self) -> HiddenCounts {
        let mut c = HiddenCounts::default();
        let cat = self.catalog_dict();
        if let Some(info) = self
            .doc
            .trailer
            .get(b"Info")
            .ok()
            .and_then(|o| resolve(&self.doc, o).as_dict().ok())
        {
            c.metadata += info.len();
        }
        if cat.is_some_and(|c| c.has(b"Metadata")) {
            c.metadata += 1;
        }
        let names = cat
            .and_then(|c| c.get(b"Names").ok())
            .and_then(|o| resolve(&self.doc, o).as_dict().ok());
        let tree_len = |k: &[u8]| {
            names
                .and_then(|n| n.get(k).ok())
                .and_then(|o| resolve(&self.doc, o).as_dict().ok())
                .and_then(|t| t.get(b"Names").ok())
                .and_then(|o| resolve(&self.doc, o).as_array().ok())
                .map_or(0, |a| a.len() / 2)
        };
        c.attachments += tree_len(b"EmbeddedFiles");
        c.scripts += tree_len(b"JavaScript");
        if cat.is_some_and(|c| c.has(b"OpenAction") || c.has(b"AA")) {
            c.scripts += 1;
        }
        c.hidden_layers = self.off_groups().len();
        c.bookmarks = self.outline().len();
        for p in &self.pages {
            for (_, st) in self.page_annots(p.id) {
                match st.as_slice() {
                    b"Link" => c.links += 1,
                    b"Widget" => c.forms += 1,
                    b"Popup" => {}
                    b"FileAttachment" => c.attachments += 1,
                    _ => c.comments += 1,
                }
            }
        }
        c
    }

    /// Remove the hidden data chosen in `what` (one undo step).
    #[allow(clippy::too_many_lines)]
    pub fn remove_hidden_data(&mut self, what: HiddenData) -> Result<(), String> {
        let root = self.catalog_root()?;
        self.begin("remove hidden data");
        let r = (|| {
            if what.metadata {
                if let Ok(info) = self.doc.trailer.get(b"Info").and_then(Object::as_reference) {
                    self.touch(info);
                    if let Ok(d) = self.doc.get_dictionary_mut(info) {
                        *d = Dictionary::new();
                    }
                } else if self.doc.trailer.has(b"Info") {
                    self.doc.trailer.remove(b"Info");
                }
            }
            // Annotations of every page.
            for i in 0..self.pages.len() {
                let pid = self.pages[i].id;
                let gone: Vec<ObjectId> = self
                    .page_annots(pid)
                    .into_iter()
                    .filter(|(_, st)| match st.as_slice() {
                        b"Link" => what.links,
                        b"Widget" => what.forms,
                        b"FileAttachment" => what.attachments || what.comments,
                        b"Popup" => what.comments,
                        _ => what.comments,
                    })
                    .map(|(id, _)| id)
                    .collect();
                if !gone.is_empty() {
                    self.remove_annots(i, &gone)?;
                }
                if what.scripts {
                    self.touch(pid);
                    if let Ok(d) = self.doc.get_dictionary_mut(pid) {
                        d.remove(b"AA");
                    }
                }
            }
            if what.hidden_layers {
                self.remove_off_content()?;
            }
            self.touch(root);
            let c = self
                .doc
                .get_dictionary_mut(root)
                .map_err(|e| e.to_string())?;
            if what.metadata {
                c.remove(b"Metadata");
            }
            if what.forms {
                c.remove(b"AcroForm");
            }
            if what.bookmarks {
                c.remove(b"Outlines");
                if c.get(b"PageMode").and_then(Object::as_name).ok() == Some(b"UseOutlines") {
                    c.remove(b"PageMode");
                }
            }
            if what.hidden_layers {
                c.remove(b"OCProperties");
            }
            if what.scripts {
                c.remove(b"OpenAction");
                c.remove(b"AA");
            }
            let names = c.get(b"Names").ok().cloned();
            if what.attachments || what.scripts {
                let mut drop: Vec<&[u8]> = Vec::new();
                if what.attachments {
                    drop.push(b"EmbeddedFiles");
                }
                if what.scripts {
                    drop.push(b"JavaScript");
                }
                match names {
                    Some(Object::Reference(n)) => {
                        self.touch(n);
                        if let Ok(d) = self.doc.get_dictionary_mut(n) {
                            for k in drop {
                                d.remove(k);
                            }
                        }
                    }
                    Some(Object::Dictionary(_)) => {
                        if let Ok(Object::Dictionary(d)) = self
                            .doc
                            .get_dictionary_mut(root)
                            .map_err(|e| e.to_string())?
                            .get_mut(b"Names")
                        {
                            for k in drop {
                                d.remove(k);
                            }
                        }
                    }
                    _ => {}
                }
            }
            Ok(())
        })();
        self.commit();
        self.refresh_pages();
        r
    }

    /// Delete the page content marked with optional content groups that are off.
    fn remove_off_content(&mut self) -> Result<(), String> {
        let off = self.off_groups();
        if off.is_empty() {
            return Ok(());
        }
        for i in 0..self.pages.len() {
            let pid = self.pages[i].id;
            // Property names of the page that map to a group that is off.
            let props: HashSet<Vec<u8>> = self
                .doc
                .get_dictionary(pid)
                .ok()
                .and_then(|p| crate::inherited(&self.doc, p, b"Resources"))
                .and_then(|r| r.as_dict().ok())
                .and_then(|r| r.get(b"Properties").ok())
                .and_then(|o| resolve(&self.doc, o).as_dict().ok())
                .map(|p| {
                    p.iter()
                        .filter(|(_, v)| v.as_reference().is_ok_and(|r| off.contains(&r)))
                        .map(|(k, _)| k.clone())
                        .collect()
                })
                .unwrap_or_default();
            if props.is_empty() {
                continue;
            }
            let (sid, content) = self.page_stream(i)?;
            let ops = Content::decode(&content).map_err(|e| e.to_string())?;
            let mut out = Vec::with_capacity(ops.operations.len());
            // Depth of marked content inside a removed section (0: keeping).
            let mut skip = 0_usize;
            for op in ops.operations {
                let name = op.operator.as_str();
                if skip > 0 {
                    match name {
                        "BDC" | "BMC" => skip += 1,
                        "EMC" => skip -= 1,
                        _ => {}
                    }
                    continue;
                }
                if name == "BDC"
                    && op.operands.first().and_then(|o| o.as_name().ok()) == Some(b"OC")
                    && op
                        .operands
                        .get(1)
                        .and_then(|o| o.as_name().ok())
                        .is_some_and(|n| props.contains(n))
                {
                    skip = 1;
                    continue;
                }
                out.push(op);
            }
            let bytes = Content { operations: out }
                .encode()
                .map_err(|e| e.to_string())?;
            self.write_page(sid, bytes, Vec::new(), b"");
        }
        Ok(())
    }

    /// Names of the fields in calculation order (`/AcroForm /CO`).
    pub fn calculation_order(&self) -> Vec<String> {
        let fields = self.form_fields();
        let co: Vec<ObjectId> = self
            .catalog_dict()
            .and_then(|c| c.get(b"AcroForm").ok())
            .and_then(|o| resolve(&self.doc, o).as_dict().ok())
            .and_then(|a| a.get(b"CO").ok())
            .and_then(|o| resolve(&self.doc, o).as_array().ok())
            .map(|a| a.iter().filter_map(|o| o.as_reference().ok()).collect())
            .unwrap_or_default();
        co.iter()
            .filter_map(|id| {
                fields
                    .iter()
                    .find(|f| f.field == *id)
                    .map(|f| f.name.clone())
            })
            .collect()
    }

    /// Set the calculation order to the fields named in `names` (unknown names skipped).
    pub fn set_calculation_order(&mut self, names: &[String]) -> Result<usize, String> {
        let fields = self.form_fields();
        let ids: Vec<Object> = names
            .iter()
            .filter_map(|n| fields.iter().find(|f| f.name == *n).map(|f| f.field))
            .fold(Vec::new(), |mut v: Vec<ObjectId>, id| {
                if !v.contains(&id) {
                    v.push(id);
                }
                v
            })
            .into_iter()
            .map(Object::Reference)
            .collect();
        let n = ids.len();
        let root = self.catalog_root()?;
        let form = self
            .catalog_dict()
            .and_then(|c| c.get(b"AcroForm").ok())
            .cloned()
            .ok_or("the document has no form")?;
        self.begin("calculation order");
        if let Object::Reference(f) = form {
            self.touch(f);
            if let Ok(d) = self.doc.get_dictionary_mut(f) {
                d.set("CO", ids);
            }
        } else {
            self.touch(root);
            if let Ok(c) = self.doc.get_dictionary_mut(root) {
                if let Ok(Object::Dictionary(d)) = c.get_mut(b"AcroForm") {
                    d.set("CO", ids);
                }
            }
        }
        self.commit();
        Ok(n)
    }

    /// Tooltip of a field (`/TU`, its alternate name).
    pub fn field_tooltip(&self, f: &FormField) -> String {
        self.doc
            .get_dictionary(f.field)
            .ok()
            .and_then(|d| d.get(b"TU").ok())
            .and_then(|o| resolve(&self.doc, o).as_str().ok())
            .map(crate::text_string)
            .unwrap_or_default()
    }

    /// Set the tooltip of a field (empty removes it).
    pub fn set_field_tooltip(&mut self, f: &FormField, tip: &str) -> Result<(), String> {
        self.begin("tooltip");
        self.touch(f.field);
        if let Ok(d) = self.doc.get_dictionary_mut(f.field) {
            if tip.is_empty() {
                d.remove(b"TU");
            } else {
                d.set("TU", crate::annot::text_obj(tip));
            }
        }
        self.commit();
        Ok(())
    }

    /// Give push button `f` an action run when it is clicked (`/A`).
    pub fn set_button_action(&mut self, f: &FormField, a: &ButtonAction) -> Result<(), String> {
        let action = match a {
            ButtonAction::ResetForm => {
                let mut d = Dictionary::new();
                d.set("S", Object::Name(b"ResetForm".to_vec()));
                d
            }
            ButtonAction::Hide(names, hide) => {
                let mut d = Dictionary::new();
                d.set("S", Object::Name(b"Hide".to_vec()));
                let t: Vec<Object> = names.iter().map(|n| crate::annot::text_obj(n)).collect();
                d.set(
                    "T",
                    if t.len() == 1 {
                        t[0].clone()
                    } else {
                        Object::Array(t)
                    },
                );
                d.set("H", *hide);
                d
            }
        };
        self.begin("button action");
        self.touch(f.widget);
        if let Ok(d) = self.doc.get_dictionary_mut(f.widget) {
            d.set("A", action);
        }
        self.commit();
        Ok(())
    }

    /// The action of push button `f`, when it is one BlueEditor runs.
    pub fn button_action(&self, f: &FormField) -> Option<ButtonAction> {
        let d = self.doc.get_dictionary(f.widget).ok()?;
        let a = resolve(&self.doc, d.get(b"A").ok()?).as_dict().ok()?;
        match a.get(b"S").ok()?.as_name().ok()? {
            b"ResetForm" => Some(ButtonAction::ResetForm),
            b"Hide" => {
                let text = |o: &Object| resolve(&self.doc, o).as_str().ok().map(crate::text_string);
                let names = match a.get(b"T").ok().map(|o| resolve(&self.doc, o)) {
                    Some(Object::Array(v)) => v.iter().filter_map(text).collect(),
                    Some(o) => text(o).into_iter().collect(),
                    None => Vec::new(),
                };
                let hide = a.get(b"H").and_then(Object::as_bool).unwrap_or(true);
                Some(ButtonAction::Hide(names, hide))
            }
            _ => None,
        }
    }

    /// Run a hide / show action: set or clear the hidden flag of the named fields' widgets.
    pub fn hide_fields(&mut self, names: &[String], hide: bool) -> Result<usize, String> {
        let widgets: Vec<ObjectId> = self
            .form_fields()
            .into_iter()
            .filter(|f| names.contains(&f.name))
            .map(|f| f.widget)
            .collect();
        self.begin("hide fields");
        for &w in &widgets {
            self.touch(w);
            if let Ok(d) = self.doc.get_dictionary_mut(w) {
                let f = d.get(b"F").and_then(Object::as_i64).unwrap_or(0);
                d.set("F", if hide { f | 2 } else { f & !2 });
            }
        }
        self.commit();
        Ok(widgets.len())
    }
}
