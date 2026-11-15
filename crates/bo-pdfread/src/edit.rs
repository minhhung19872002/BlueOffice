//! Document editing for BlueEditor: page organisation (insert, delete, reorder, rotate,
//! extract, merge, split), undo / redo of every edit and the full rewrite save (ISO 32000-1,
//! §7.5 file structure, §7.7.3 page tree, §7.6 encryption).
//!
//! Edits work on the in-memory object graph. Every change goes through [`PdfDocument::touch`],
//! which remembers the object's previous value in the open undo step, so undo restores objects
//! exactly. The page tree is rewritten flat (one `/Pages` node) with each page's inherited
//! attributes copied onto it (§7.7.3.4), which keeps every later reorder a simple list edit.
use crate::store::Document;
use crate::{resolve, PdfDocument};
use lopdf::{Dictionary, Object, ObjectId, Stream};
use std::collections::{HashMap, HashSet};

/// How [`PdfDocument::save_full`] writes the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SaveOptions {
    /// Flate-compress streams that have no filter (§7.4.4).
    pub compress: bool,
    /// Pack objects into object streams with a cross-reference stream (§7.5.7, §7.5.8);
    /// ignored for encrypted output.
    pub object_streams: bool,
    /// Drop objects nothing refers to (deleted pages, replaced content).
    pub prune: bool,
}

impl Default for SaveOptions {
    fn default() -> Self {
        Self {
            compress: true,
            object_streams: false,
            prune: true,
        }
    }
}

/// What a protected document allows without the permissions password (ISO 32000-1
/// §7.6.3.2, Table 22).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools)]
pub struct Permissions {
    /// Print (bit 3) in high quality (bit 12).
    pub print: bool,
    /// Change the document (bit 4).
    pub modify: bool,
    /// Copy text and graphics (bit 5).
    pub copy: bool,
    /// Add comments and fill fields (bits 6, 9).
    pub annotate: bool,
    /// Insert, delete and rotate pages (bit 11).
    pub assemble: bool,
}

impl Default for Permissions {
    fn default() -> Self {
        Self {
            print: true,
            modify: true,
            copy: true,
            annotate: true,
            assemble: true,
        }
    }
}

impl Permissions {
    fn flags(self) -> lopdf::Permissions {
        use lopdf::Permissions as P;
        let mut p = P::COPYABLE_FOR_ACCESSIBILITY;
        if self.print {
            p |= P::PRINTABLE | P::PRINTABLE_IN_HIGH_QUALITY;
        }
        if self.modify {
            p |= P::MODIFIABLE;
        }
        if self.copy {
            p |= P::COPYABLE;
        }
        if self.annotate {
            p |= P::ANNOTABLE | P::FILLABLE;
        }
        if self.assemble {
            p |= P::ASSEMBLABLE;
        }
        p
    }
}

/// One undoable edit: previous values of the objects it changed (`None`: it created them).
#[derive(Default, Clone)]
pub(crate) struct Step {
    before: Vec<(ObjectId, Option<Object>)>,
    seen: HashSet<ObjectId>,
    /// Page tree order changed (pages info must be rescanned).
    structural: bool,
    label: String,
}

/// Undo state of a document.
#[derive(Default)]
pub(crate) struct EditState {
    undo: Vec<Step>,
    redo: Vec<Step>,
    open: Option<Step>,
    /// Page tree edits since the last save: the next save rewrites the file.
    pub structural: bool,
    /// Edits since the last save.
    pub edits: u32,
    /// Content streams made by editing a page (one per page, already wrapped in `q … Q`).
    pub own_streams: HashSet<ObjectId>,
    /// Character encoders of fonts by font object (`None`: text cannot be encoded).
    pub encoders: HashMap<ObjectId, Option<Encoder>>,
    /// An undo group is open ([`PdfDocument::begin_group`]).
    pub group: bool,
    /// Embedded fallback fonts (serif?) by font object.
    pub embedded: HashMap<usize, ObjectId>,
    /// Page contents swapped out by `hide_text`.
    pub hidden_text: Option<crate::convert::HiddenText>,
}

const UNDO_LIMIT: usize = 100;

/// Codes of each character in one font.
pub(crate) type Encoder = std::rc::Rc<HashMap<char, Vec<u8>>>;

/// Inheritable page attributes (§7.7.3.4, Table 31).
const INHERITED: [&[u8]; 4] = [b"Resources", b"MediaBox", b"CropBox", b"Rotate"];

/// Raw value (maybe a reference) of an inheritable attribute of a page or its ancestors.
pub(crate) fn inherited_raw(doc: &Document, page: ObjectId, key: &[u8]) -> Option<Object> {
    let mut d = doc.get_dictionary(page).ok()?;
    for _ in 0..32 {
        if let Ok(v) = d.get(key) {
            return Some(v.clone());
        }
        d = resolve(doc, d.get(b"Parent").ok()?).as_dict().ok()?;
    }
    None
}

/// Map every object id of `doc` to `start..` and rewrite references (trailer included).
fn renumber(doc: &mut Document, start: u32) -> HashMap<ObjectId, ObjectId> {
    let ids: Vec<ObjectId> = doc.objects.keys().copied().collect();
    let map: HashMap<ObjectId, ObjectId> = ids
        .iter()
        .enumerate()
        .map(|(k, &id)| (id, (start + k as u32, 0)))
        .collect();
    fn fix(o: &mut Object, map: &HashMap<ObjectId, ObjectId>) {
        match o {
            Object::Reference(r) => {
                // A reference to a missing object becomes null (§7.3.10).
                *o = map.get(r).map_or(Object::Null, |n| Object::Reference(*n));
            }
            Object::Array(a) => a.iter_mut().for_each(|v| fix(v, map)),
            Object::Dictionary(d) => d.iter_mut().for_each(|(_, v)| fix(v, map)),
            Object::Stream(s) => s.dict.iter_mut().for_each(|(_, v)| fix(v, map)),
            _ => {}
        }
    }
    let old = std::mem::take(&mut doc.objects);
    for (id, mut o) in old {
        fix(&mut o, &map);
        doc.objects.insert(map[&id], o);
    }
    let mut t = std::mem::take(&mut doc.trailer);
    t.iter_mut().for_each(|(_, v)| fix(v, &map));
    doc.trailer = t;
    doc.max_id = start + ids.len() as u32;
    map
}

/// Copy the inherited attributes onto each page so it can move under another parent.
fn materialize(doc: &mut Document, pages: &[ObjectId]) {
    for &id in pages {
        let values: Vec<(&[u8], Object)> = INHERITED
            .iter()
            .filter_map(|k| inherited_raw(doc, id, k).map(|v| (*k, v)))
            .collect();
        if let Ok(d) = doc.get_dictionary_mut(id) {
            for (k, v) in values {
                if !d.has(k) {
                    d.set(k.to_vec(), v);
                }
            }
        }
    }
}

fn flate(data: &[u8]) -> Vec<u8> {
    miniz_oxide::deflate::compress_to_vec_zlib(data, 6)
}

/// Streams that may be (re)compressed: no filter yet, not metadata (kept readable, §14.3.2).
fn compressible(s: &Stream) -> bool {
    s.dict.get(b"Filter").is_err()
        && s.dict.get(b"Type").and_then(Object::as_name).ok() != Some(b"Metadata")
        && s.content.len() > 64
}

impl PdfDocument {
    // ---- Undo ----------------------------------------------------------------------------

    /// Start an undo step named `label` (nested calls join the open step).
    pub(crate) fn begin(&mut self, label: &str) {
        if self.edit.open.is_none() {
            self.edit.open = Some(Step {
                label: label.to_owned(),
                ..Step::default()
            });
        }
    }

    /// Remember `id`'s current value in the open step before it changes.
    pub(crate) fn touch(&mut self, id: ObjectId) {
        if let Some(step) = self.edit.open.as_mut() {
            // Only the first change in a step clones the old value (typing touches the page
            // stream on every key).
            if step.seen.insert(id) {
                let prev = self.doc.objects.get(&id).cloned();
                step.before.push((id, prev));
            }
        }
        if !self.changed.contains(&id) {
            self.changed.push(id);
        }
    }

    /// Add a new object inside the open step.
    pub(crate) fn add(&mut self, o: Object) -> ObjectId {
        let id = self.doc.add_object(o);
        if let Some(step) = self.edit.open.as_mut() {
            step.seen.insert(id);
            step.before.push((id, None));
        }
        self.changed.push(id);
        id
    }

    /// Close the open step; it becomes the newest undo entry.
    pub(crate) fn commit(&mut self) {
        if self.edit.group {
            return;
        }
        let Some(step) = self.edit.open.take() else {
            return;
        };
        if step.before.is_empty() {
            return;
        }
        if step.structural {
            self.edit.structural = true;
            self.refresh_pages();
        }
        self.edit.edits += 1;
        self.edit.redo.clear();
        self.edit.undo.push(step);
        if self.edit.undo.len() > UNDO_LIMIT {
            self.edit.undo.remove(0);
        }
    }

    /// Whether an edit can be undone.
    pub fn can_undo(&self) -> bool {
        !self.edit.undo.is_empty()
    }

    /// Whether an undone edit can be redone.
    pub fn can_redo(&self) -> bool {
        !self.edit.redo.is_empty()
    }

    /// Name of the edit [`PdfDocument::undo`] would revert.
    pub fn undo_label(&self) -> Option<&str> {
        self.edit.undo.last().map(|s| s.label.as_str())
    }

    fn swap_step(&mut self, step: Step) -> Step {
        let mut inverse = Step {
            label: step.label.clone(),
            structural: step.structural,
            ..Step::default()
        };
        for (id, value) in step.before.into_iter().rev() {
            let now = match value {
                Some(v) => self.doc.objects.insert(id, v),
                None => self.doc.objects.remove(&id),
            };
            inverse.seen.insert(id);
            inverse.before.push((id, now));
            if !self.changed.contains(&id) {
                self.changed.push(id);
            }
        }
        inverse.before.reverse();
        if inverse.structural {
            self.edit.structural = true;
        }
        self.refresh_pages();
        // Cached fonts and images may belong to objects that changed.
        self.font_cache = crate::font::FontCache::default();
        self.images = crate::interp::ImageCache::default();
        self.edit.encoders.clear();
        inverse
    }

    /// Revert the newest edit; false when there is none.
    pub fn undo(&mut self) -> bool {
        if self.edit.group {
            self.end_group();
        }
        let Some(step) = self.edit.undo.pop() else {
            return false;
        };
        let inv = self.swap_step(step);
        self.edit.redo.push(inv);
        self.edit.edits += 1;
        true
    }

    /// Apply the newest undone edit again; false when there is none.
    pub fn redo(&mut self) -> bool {
        let Some(step) = self.edit.redo.pop() else {
            return false;
        };
        let inv = self.swap_step(step);
        self.edit.undo.push(inv);
        self.edit.edits += 1;
        true
    }

    /// Re-read page sizes, rotations and labels after the page tree changed.
    pub(crate) fn refresh_pages(&mut self) {
        let (pages, labelled) = crate::scan_pages(&self.doc);
        self.pages = pages;
        self.labelled = labelled;
    }

    // ---- Page tree -----------------------------------------------------------------------

    fn root_pages(&self) -> Result<ObjectId, String> {
        self.doc
            .catalog()
            .map_err(|e| e.to_string())?
            .get(b"Pages")
            .and_then(Object::as_reference)
            .map_err(|_| "the catalog has no page tree".to_owned())
    }

    /// Make `order` the page list: one `/Pages` node whose kids are the pages.
    pub(crate) fn set_page_order(&mut self, order: &[ObjectId]) -> Result<(), String> {
        let root = self.root_pages()?;
        for &id in order {
            self.touch(id);
        }
        materialize(&mut self.doc, order);
        for &id in order {
            if let Ok(d) = self.doc.get_dictionary_mut(id) {
                d.set("Parent", Object::Reference(root));
            }
        }
        self.touch(root);
        let d = self
            .doc
            .get_dictionary_mut(root)
            .map_err(|e| e.to_string())?;
        d.set(
            "Kids",
            order
                .iter()
                .map(|&id| Object::Reference(id))
                .collect::<Vec<_>>(),
        );
        d.set("Count", order.len() as i64);
        d.remove(b"Parent");
        if let Some(step) = self.edit.open.as_mut() {
            step.structural = true;
        }
        Ok(())
    }

    fn page_ids(&self) -> Vec<ObjectId> {
        self.pages.iter().map(|p| p.id).collect()
    }

    /// Rotate pages by `quarter_turns` × 90° clockwise (`/Rotate`, §7.7.3.3).
    pub fn rotate_pages(&mut self, pages: &[usize], quarter_turns: i32) -> Result<(), String> {
        let ids: Vec<(ObjectId, u16)> = pages
            .iter()
            .filter_map(|&i| self.pages.get(i).map(|p| (p.id, p.rotate)))
            .collect();
        if ids.is_empty() {
            return Err("no pages".into());
        }
        self.begin("rotate");
        // Flatten first so the page's own /Rotate wins over an inherited one.
        let order = self.page_ids();
        self.set_page_order(&order)?;
        for (id, rot) in ids {
            self.touch(id);
            let r = (i32::from(rot) + quarter_turns * 90).rem_euclid(360);
            if let Ok(d) = self.doc.get_dictionary_mut(id) {
                d.set("Rotate", i64::from(r));
            }
        }
        self.commit();
        Ok(())
    }

    /// Delete pages (at least one page stays).
    pub fn delete_pages(&mut self, pages: &[usize]) -> Result<(), String> {
        let drop: HashSet<usize> = pages.iter().copied().collect();
        let order: Vec<ObjectId> = self
            .pages
            .iter()
            .enumerate()
            .filter(|(i, _)| !drop.contains(i))
            .map(|(_, p)| p.id)
            .collect();
        if order.is_empty() {
            return Err("a document keeps at least one page".into());
        }
        if order.len() == self.pages.len() {
            return Ok(());
        }
        self.begin("delete");
        self.set_page_order(&order)?;
        self.commit();
        Ok(())
    }

    /// Reverse the order of `pages` among themselves (all pages when `pages` is empty).
    pub fn reverse_pages(&mut self, pages: &[usize]) -> Result<(), String> {
        let n = self.pages.len();
        let mut sel: Vec<usize> = if pages.is_empty() {
            (0..n).collect()
        } else {
            pages.iter().copied().filter(|&i| i < n).collect()
        };
        sel.sort_unstable();
        sel.dedup();
        if sel.len() < 2 {
            return Ok(());
        }
        let mut order = self.page_ids();
        let ids: Vec<ObjectId> = sel.iter().map(|&i| order[i]).collect();
        for (k, &i) in sel.iter().enumerate() {
            order[i] = ids[ids.len() - 1 - k];
        }
        self.begin("reverse");
        self.set_page_order(&order)?;
        self.commit();
        Ok(())
    }

    /// Protect the document with passwords (standard security handler, AES-128, §7.6.3.2):
    /// `user` opens it (empty: anyone opens it), `owner` changes the permissions. An empty
    /// `owner` removes the protection. Written by the next save, which rewrites the file.
    pub fn set_password(&mut self, user: &str, owner: &str) -> Result<(), String> {
        self.set_protection(user, owner, Permissions::default(), true)
    }

    /// Protect the document: `user` opens it, `owner` changes the permissions, `allow` is what
    /// it allows without the owner password; AES-256 (security handler revision 6, ISO
    /// 32000-2 §7.6.4.4) or AES-128 (revision 4). An empty `owner` removes the protection.
    pub fn set_protection(
        &mut self,
        user: &str,
        owner: &str,
        allow: Permissions,
        aes256: bool,
    ) -> Result<(), String> {
        use lopdf::encryption::crypt_filters::{Aes128CryptFilter, Aes256CryptFilter, CryptFilter};
        use lopdf::{EncryptionState, EncryptionVersion};
        if let Ok(e) = self
            .doc
            .trailer
            .get(b"Encrypt")
            .and_then(Object::as_reference)
        {
            self.doc.objects.remove(&e);
        }
        self.doc.trailer.remove(b"Encrypt");
        self.doc.encryption_state = None;
        if !owner.is_empty() {
            if !self.doc.trailer.has(b"ID") {
                let id = Object::String(
                    new_file_id(self.doc.objects.len()),
                    lopdf::StringFormat::Hexadecimal,
                );
                self.doc.trailer.set("ID", vec![id.clone(), id]);
            }
            let mut filters: std::collections::BTreeMap<Vec<u8>, std::sync::Arc<dyn CryptFilter>> =
                std::collections::BTreeMap::new();
            let state = if aes256 {
                filters.insert(b"StdCF".to_vec(), std::sync::Arc::new(Aes256CryptFilter));
                let mut key = new_file_id(1);
                key.extend(new_file_id(2));
                EncryptionState::try_from(EncryptionVersion::V5 {
                    encrypt_metadata: true,
                    crypt_filters: filters,
                    file_encryption_key: &key,
                    stream_filter: b"StdCF".to_vec(),
                    string_filter: b"StdCF".to_vec(),
                    owner_password: owner,
                    user_password: user,
                    permissions: allow.flags(),
                })
            } else {
                filters.insert(b"StdCF".to_vec(), std::sync::Arc::new(Aes128CryptFilter));
                EncryptionState::try_from(EncryptionVersion::V4 {
                    document: &self.doc,
                    encrypt_metadata: true,
                    crypt_filters: filters,
                    stream_filter: b"StdCF".to_vec(),
                    string_filter: b"StdCF".to_vec(),
                    owner_password: owner,
                    user_password: user,
                    permissions: allow.flags(),
                })
            }
            .map_err(|e| e.to_string())?;
            self.doc.encryption_state = Some(state);
        }
        self.edit.structural = true;
        self.edit.edits += 1;
        Ok(())
    }

    /// Move `pages` (in their current order) so they start at index `to` of the pages that
    /// remain (`to` counts positions before the move: dropping before page `k` passes `k`).
    pub fn move_pages(&mut self, pages: &[usize], to: usize) -> Result<(), String> {
        let set: HashSet<usize> = pages.iter().copied().collect();
        let ids = self.page_ids();
        let moving: Vec<ObjectId> = (0..ids.len())
            .filter(|i| set.contains(i))
            .map(|i| ids[i])
            .collect();
        if moving.is_empty() {
            return Ok(());
        }
        let before = (0..to.min(ids.len())).filter(|i| !set.contains(i)).count();
        let mut rest: Vec<ObjectId> = (0..ids.len())
            .filter(|i| !set.contains(i))
            .map(|i| ids[i])
            .collect();
        let tail = rest.split_off(before);
        rest.extend(moving);
        rest.extend(tail);
        if rest == ids {
            return Ok(());
        }
        self.begin("move");
        self.set_page_order(&rest)?;
        self.commit();
        Ok(())
    }

    /// Insert an empty page of `width` × `height` points at index `at`.
    pub fn insert_blank_page(&mut self, at: usize, width: f32, height: f32) -> Result<(), String> {
        let root = self.root_pages()?;
        self.begin("insert");
        let content = self.add(Object::Stream(Stream::new(Dictionary::new(), Vec::new())));
        let mut page = Dictionary::new();
        page.set("Type", Object::Name(b"Page".to_vec()));
        page.set("Parent", Object::Reference(root));
        page.set(
            "MediaBox",
            vec![
                0.into(),
                0.into(),
                Object::Real(width.clamp(3.0, 14_400.0)),
                Object::Real(height.clamp(3.0, 14_400.0)),
            ],
        );
        page.set("Resources", Dictionary::new());
        page.set("Contents", Object::Reference(content));
        let id = self.add(Object::Dictionary(page));
        let mut order = self.page_ids();
        order.insert(at.min(order.len()), id);
        self.set_page_order(&order)?;
        self.commit();
        Ok(())
    }

    /// Insert pages of another PDF (all, or the indices in `pick`) at index `at`; returns how
    /// many pages were inserted. Its objects are copied in with new numbers.
    pub fn insert_pdf(
        &mut self,
        at: usize,
        bytes: &[u8],
        pick: Option<&[usize]>,
    ) -> Result<usize, String> {
        let start = bytes
            .windows(5)
            .take(1024)
            .position(|w| w == b"%PDF-")
            .ok_or("not a PDF file")?;
        let mut other = Document::from(
            lopdf::Document::load_mem(&bytes[start..]).map_err(|e| format!("PDF: {e}"))?,
        );
        if other.is_encrypted() && other.encryption_state.is_none() {
            return Err("the document is protected by a password".into());
        }
        let encrypt = other
            .trailer
            .get(b"Encrypt")
            .and_then(Object::as_reference)
            .ok();
        if let Some(e) = encrypt {
            other.objects.remove(&e);
            other.trailer.remove(b"Encrypt");
        }
        let all: Vec<ObjectId> = other.get_pages().into_values().collect();
        let chosen: Vec<ObjectId> = match pick {
            Some(p) => p.iter().filter_map(|&i| all.get(i).copied()).collect(),
            None => all,
        };
        if chosen.is_empty() {
            return Err("no pages to insert".into());
        }
        materialize(&mut other, &chosen);
        // Object streams and cross-reference streams of the source are not copied.
        other.objects.retain(|_, o| {
            !matches!(
                o.type_name(),
                Ok(b"ObjStm" | b"XRef" | b"Catalog" | b"Pages")
            )
        });
        let map = renumber(&mut other, self.doc.max_id + 1);
        let new_pages: Vec<ObjectId> = chosen
            .iter()
            .filter_map(|id| map.get(id))
            .copied()
            .collect();
        self.begin("insert");
        for (id, o) in std::mem::take(&mut other.objects) {
            if let Some(step) = self.edit.open.as_mut() {
                step.seen.insert(id);
                step.before.push((id, None));
            }
            self.changed.push(id);
            self.doc.objects.insert(id, o);
        }
        self.doc.max_id = self.doc.max_id.max(other.max_id);
        // Annotations of copied pages point at their page (`/P`); the parent is set below.
        let mut order = self.page_ids();
        let at = at.min(order.len());
        for (k, id) in new_pages.iter().enumerate() {
            order.insert(at + k, *id);
        }
        self.set_page_order(&order)?;
        self.commit();
        Ok(new_pages.len())
    }

    /// Append every page of another PDF (merge).
    pub fn append_pdf(&mut self, bytes: &[u8]) -> Result<usize, String> {
        let n = self.pages.len();
        self.insert_pdf(n, bytes, None)
    }

    /// A new PDF with only `pages` (in that order), written with `opts`.
    pub fn extract_pages(&self, pages: &[usize], opts: SaveOptions) -> Result<Vec<u8>, String> {
        let ids: Vec<ObjectId> = pages
            .iter()
            .filter_map(|&i| self.pages.get(i).map(|p| p.id))
            .collect();
        if ids.is_empty() {
            return Err("no pages".into());
        }
        let mut doc = self.doc.clone();
        let root = self.root_pages()?;
        materialize(&mut doc, &ids);
        for &id in &ids {
            if let Ok(d) = doc.get_dictionary_mut(id) {
                d.set("Parent", Object::Reference(root));
            }
        }
        let d = doc.get_dictionary_mut(root).map_err(|e| e.to_string())?;
        d.set(
            "Kids",
            ids.iter()
                .map(|&id| Object::Reference(id))
                .collect::<Vec<_>>(),
        );
        d.set("Count", ids.len() as i64);
        // Outline entries and page labels refer to the full document.
        if let Ok(c) = doc.catalog_mut() {
            c.remove(b"Outlines");
            c.remove(b"PageLabels");
        }
        write_document(doc, opts, true).map(|w| w.0)
    }

    /// Split into files of `every` pages each.
    pub fn split(&self, every: usize, opts: SaveOptions) -> Result<Vec<Vec<u8>>, String> {
        let every = every.max(1);
        let n = self.pages.len();
        (0..n)
            .step_by(every)
            .map(|s| {
                let pages: Vec<usize> = (s..(s + every).min(n)).collect();
                self.extract_pages(&pages, opts)
            })
            .collect()
    }

    /// Page tree edits since the last save (the next [`PdfDocument::save`] rewrites the file).
    pub fn has_structural_edits(&self) -> bool {
        self.edit.structural
    }

    /// Edits since opening (or the last save), undo and redo included.
    pub fn edit_count(&self) -> u32 {
        self.edit.edits
    }

    /// Whether the file was encrypted (it is written back with the same security handler).
    pub fn is_encrypted(&self) -> bool {
        self.doc.is_encrypted()
    }

    /// The whole document rewritten: no earlier revisions, unused objects dropped, streams
    /// compressed, encrypted again with the original security handler when it was encrypted.
    /// Afterwards the returned bytes are the document's original.
    pub fn save_full(&mut self, opts: SaveOptions) -> Result<Vec<u8>, String> {
        let (out, encrypt) = write_document(self.doc.clone(), opts, true)?;
        // Later incremental saves name the encryption dictionary the file now has.
        if let Some((id, dict)) = encrypt {
            if let Ok(old) = self
                .doc
                .trailer
                .get(b"Encrypt")
                .and_then(Object::as_reference)
            {
                self.doc.objects.remove(&old);
            }
            self.doc.objects.insert(id, dict);
            self.doc.trailer.set("Encrypt", Object::Reference(id));
            self.doc.max_id = self.doc.max_id.max(id.0);
        }
        self.saved(out.clone());
        Ok(out)
    }

    /// Compress changed streams written without a filter (typed text, embedded faces)
    /// before an incremental save.
    pub(crate) fn compress_changed(&mut self) {
        for id in self.changed.clone() {
            if let Some(Object::Stream(s)) = self.doc.objects.get_mut(&id) {
                if compressible(s) {
                    let z = flate(&s.content);
                    if z.len() + 19 < s.content.len() {
                        s.dict.set("Filter", Object::Name(b"FlateDecode".to_vec()));
                        s.dict.remove(b"DecodeParms");
                        s.set_content(z);
                    }
                }
            }
        }
    }

    pub(crate) fn saved(&mut self, bytes: Vec<u8>) {
        self.bytes = std::sync::Arc::new(bytes);
        self.changed.clear();
        self.edit.structural = false;
        self.edit.edits = 0;
    }
}

/// Write a whole document. `keep_encryption`: encrypt again with the state it was opened with.
/// A new file identifier (§14.4): 16 bytes from the clock and `salt`.
fn new_file_id(salt: usize) -> Vec<u8> {
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos())
        ^ ((salt as u128) << 64);
    let mut x = ((seed as u64) ^ ((seed >> 64) as u64)) | 1;
    (0..16)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            (x >> 24) as u8
        })
        .collect()
}

/// The written file, and the encryption dictionary it got (id and object) when encrypted.
type Written = (Vec<u8>, Option<(ObjectId, Object)>);

pub(crate) fn write_document(
    mut doc: Document,
    opts: SaveOptions,
    keep_encryption: bool,
) -> Result<Written, String> {
    let state = doc.encryption_state.take();
    if doc.is_encrypted() && state.is_none() {
        return Err("the document is protected by a password".into());
    }
    if let Ok(e) = doc.trailer.get(b"Encrypt").and_then(Object::as_reference) {
        doc.objects.remove(&e);
    }
    doc.trailer.remove(b"Encrypt");
    doc.objects.retain(|_, o| {
        !matches!(o.type_name(), Ok(b"ObjStm" | b"XRef"))
            && !(o.as_dict().is_ok_and(|d| d.has(b"Linearized")))
    });
    // A clean trailer: cross-reference stream keys of the original do not apply.
    let mut trailer = Dictionary::new();
    for k in [&b"Root"[..], b"Info", b"ID"] {
        if let Ok(v) = doc.trailer.get(k) {
            trailer.set(k.to_vec(), v.clone());
        }
    }
    if !trailer.has(b"ID") {
        let id = Object::String(
            new_file_id(doc.objects.len()),
            lopdf::StringFormat::Hexadecimal,
        );
        trailer.set("ID", vec![id.clone(), id]);
    }
    doc.trailer = trailer;
    if opts.prune {
        doc.prune_objects();
    }
    if opts.compress {
        for o in doc.objects.values_mut() {
            if let Object::Stream(s) = o {
                if compressible(s) {
                    let z = flate(&s.content);
                    if z.len() + 19 < s.content.len() {
                        s.dict.set("Filter", Object::Name(b"FlateDecode".to_vec()));
                        s.dict.remove(b"DecodeParms");
                        s.set_content(z);
                    }
                }
            }
        }
    }
    doc.max_id = doc.objects.keys().map(|k| k.0).max().unwrap_or(0);
    let encrypt = keep_encryption.then_some(state).flatten();
    let mut out = Vec::new();
    let mut dict = None;
    if let Some(state) = encrypt {
        doc.encrypt(&state).map_err(|e| format!("encrypt: {e}"))?;
        dict = doc
            .trailer
            .get(b"Encrypt")
            .and_then(Object::as_reference)
            .ok()
            .and_then(|id| doc.objects.get(&id).map(|o| (id, o.clone())));
        doc.reference_table.cross_reference_type = lopdf::xref::XrefType::CrossReferenceTable;
        doc.save_to(&mut out).map_err(|e| e.to_string())?;
    } else if opts.object_streams {
        doc.save_modern(&mut out).map_err(|e| e.to_string())?;
    } else {
        doc.reference_table.cross_reference_type = lopdf::xref::XrefType::CrossReferenceTable;
        doc.save_to(&mut out).map_err(|e| e.to_string())?;
    }
    Ok((out, dict))
}

/// Objects in `doc` keyed by id (for tests comparing revisions).
#[cfg(test)]
pub(crate) fn object_map(doc: &PdfDocument) -> std::collections::BTreeMap<ObjectId, Object> {
    doc.doc.objects.clone()
}
