//! Named page templates (ISO 32000-1 §12.7.6, the `/Pages` name tree of the document's name
//! dictionary, §7.7.4 Table 31) and embedded files of a PDF portfolio (§7.11.4 `/EmbeddedFiles`,
//! §12.3.5 `/Collection`).
//!
//! A template names a page of the document; spawning it puts a copy of that page (content,
//! resources, comments) after a given page. Embedded files are listed and read only.
#![allow(clippy::single_match_else)]
use crate::{resolve, PdfDocument};
use lopdf::{Dictionary, Object, ObjectId, StringFormat};

/// A file embedded in the document (portfolio member or attachment).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmbeddedFile {
    /// Name in the `/EmbeddedFiles` tree (or the file specification's `/UF` / `/F`).
    pub name: String,
    /// Uncompressed size when the file says it (`/Params /Size`), else the stream's size.
    pub size: usize,
    /// Description (`/Desc`).
    pub description: String,
    /// Modification date as `YYYY-MM-DD HH:MM` (`/Params /ModDate`, §7.9.4), empty when unknown.
    pub modified: String,
}

/// `D:YYYYMMDDHHmm…` (§7.9.4) as `YYYY-MM-DD HH:MM` (or the date alone).
pub(crate) fn date_text(s: &str) -> String {
    let d: String = s
        .trim_start_matches("D:")
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    if d.len() < 8 {
        return String::new();
    }
    let mut out = format!("{}-{}-{}", &d[0..4], &d[4..6], &d[6..8]);
    if d.len() >= 12 {
        out.push_str(&format!(" {}:{}", &d[8..10], &d[10..12]));
    }
    out
}

/// Text of a PDF string (UTF-16BE with a byte order mark, else PDFDocEncoding as Latin-1).
pub(crate) fn text(b: &[u8]) -> String {
    if b.len() >= 2 && b[0] == 0xFE && b[1] == 0xFF {
        let u: Vec<u16> = b[2..]
            .chunks(2)
            .map(|c| u16::from_be_bytes([c[0], *c.get(1).unwrap_or(&0)]))
            .collect();
        return String::from_utf16_lossy(&u);
    }
    b.iter().map(|&c| char::from(c)).collect()
}

/// A PDF text string: ASCII as is, anything else UTF-16BE with a byte order mark.
fn pdf_text(s: &str) -> Object {
    if s.is_ascii() {
        return Object::String(s.as_bytes().to_vec(), StringFormat::Literal);
    }
    let mut b = vec![0xFE, 0xFF];
    for u in s.encode_utf16() {
        b.extend(u.to_be_bytes());
    }
    Object::String(b, StringFormat::Hexadecimal)
}

impl PdfDocument {
    /// Entries of the name tree `key` of the catalog's `/Names` (§7.9.6), in tree order.
    pub(crate) fn name_tree(&self, key: &[u8]) -> Vec<(String, Object)> {
        fn walk(
            doc: &crate::store::Document,
            node: &Dictionary,
            out: &mut Vec<(String, Object)>,
            depth: u32,
        ) {
            if depth > 16 {
                return;
            }
            if let Some(a) = node
                .get(b"Names")
                .ok()
                .and_then(|o| resolve(doc, o).as_array().ok())
            {
                for pair in a.chunks(2) {
                    if let [k, v] = pair {
                        if let Ok(k) = resolve(doc, k).as_str() {
                            out.push((text(k), v.clone()));
                        }
                    }
                }
            }
            if let Some(kids) = node
                .get(b"Kids")
                .ok()
                .and_then(|o| resolve(doc, o).as_array().ok())
            {
                for k in kids {
                    if let Ok(d) = resolve(doc, k).as_dict() {
                        walk(doc, d, out, depth + 1);
                    }
                }
            }
        }
        let mut out = Vec::new();
        if let Some(root) = self
            .doc
            .catalog()
            .ok()
            .and_then(|c| c.get(b"Names").ok())
            .and_then(|o| resolve(&self.doc, o).as_dict().ok())
            .and_then(|n| n.get(key).ok())
            .and_then(|o| resolve(&self.doc, o).as_dict().ok())
        {
            walk(&self.doc, root, &mut out, 0);
        }
        out
    }

    /// Write the name tree `key` as one sorted `/Names` array (inside the open undo step).
    fn set_name_tree(
        &mut self,
        key: &str,
        mut entries: Vec<(String, Object)>,
    ) -> Result<(), String> {
        entries.sort_by(|a, b| a.0.cmp(&b.0));
        let root: ObjectId = self
            .doc
            .trailer
            .get(b"Root")
            .and_then(Object::as_reference)
            .map_err(|e| e.to_string())?;
        let names_ref = self
            .doc
            .get_dictionary(root)
            .ok()
            .and_then(|c| c.get(b"Names").ok())
            .and_then(|o| o.as_reference().ok());
        let mut arr = Vec::new();
        for (k, v) in entries {
            arr.push(pdf_text(&k));
            arr.push(v);
        }
        let mut tree = Dictionary::new();
        tree.set("Names", Object::Array(arr));
        let tree = Object::Dictionary(tree);
        match names_ref {
            Some(id) => {
                self.touch(id);
                let d = self.doc.get_dictionary_mut(id).map_err(|e| e.to_string())?;
                d.set(key, tree);
            }
            None => {
                self.touch(root);
                let c = self
                    .doc
                    .get_dictionary_mut(root)
                    .map_err(|e| e.to_string())?;
                let mut names = match c.get(b"Names") {
                    Ok(Object::Dictionary(d)) => d.clone(),
                    _ => Dictionary::new(),
                };
                names.set(key, tree);
                c.set("Names", Object::Dictionary(names));
            }
        }
        Ok(())
    }

    /// Named page templates: name and page index (visible templates of `/Pages` whose page
    /// is in the document).
    pub fn page_templates(&self) -> Vec<(String, usize)> {
        self.name_tree(b"Pages")
            .into_iter()
            .filter_map(|(name, v)| {
                let id = v.as_reference().ok()?;
                let i = self.pages.iter().position(|p| p.id == id)?;
                Some((name, i))
            })
            .collect()
    }

    /// Name page `page` as template `name` (replacing a template of that name); one undo step.
    pub fn add_page_template(&mut self, name: &str, page: usize) -> Result<(), String> {
        let name = name.trim();
        if name.is_empty() {
            return Err("a template needs a name".into());
        }
        let id = self.pages.get(page).ok_or("no such page")?.id;
        let mut entries: Vec<(String, Object)> = self
            .name_tree(b"Pages")
            .into_iter()
            .filter(|(n, _)| n != name)
            .collect();
        entries.push((name.to_owned(), Object::Reference(id)));
        self.begin("page template");
        let r = self.set_name_tree("Pages", entries);
        self.commit();
        r
    }

    /// Forget template `name` (its page stays); one undo step.
    pub fn remove_page_template(&mut self, name: &str) -> Result<(), String> {
        let all = self.name_tree(b"Pages");
        let entries: Vec<(String, Object)> =
            all.iter().filter(|(n, _)| n != name).cloned().collect();
        if entries.len() == all.len() {
            return Err("no such template".into());
        }
        self.begin("page template");
        let r = self.set_name_tree("Pages", entries);
        self.commit();
        r
    }

    /// Rename template `from` to `to` (its page stays; a template already named `to` is
    /// replaced); one undo step.
    pub fn rename_page_template(&mut self, from: &str, to: &str) -> Result<(), String> {
        let to = to.trim();
        if to.is_empty() {
            return Err("a template needs a name".into());
        }
        let all = self.name_tree(b"Pages");
        let v = all
            .iter()
            .find(|(n, _)| n == from)
            .map(|e| e.1.clone())
            .ok_or("no such template")?;
        let mut entries: Vec<(String, Object)> = all
            .into_iter()
            .filter(|(n, _)| n != from && n != to)
            .collect();
        entries.push((to.to_owned(), v));
        self.begin("page template");
        let r = self.set_name_tree("Pages", entries);
        self.commit();
        r
    }

    /// Make template `name` name page `page` instead of its page (the reference's Replace);
    /// one undo step.
    pub fn replace_page_template(&mut self, name: &str, page: usize) -> Result<(), String> {
        if !self.page_templates().iter().any(|(n, _)| n == name) {
            return Err("no such template".into());
        }
        self.add_page_template(name, page)
    }

    /// Spawn template `name`: a copy of its page put at index `at`; one undo step. Returns the
    /// new page's index.
    pub fn spawn_page_template(&mut self, name: &str, at: usize) -> Result<usize, String> {
        let (_, page) = self
            .page_templates()
            .into_iter()
            .find(|(n, _)| n == name)
            .ok_or("no such template")?;
        let copy = *self.duplicate_pages(&[page])?.first().ok_or("no copy")?;
        // `at` counted pages before the copy went in right after its template.
        let at = if at > page { at + 1 } else { at }.min(self.pages.len());
        if copy == at || copy + 1 == at {
            return Ok(copy);
        }
        self.move_pages(&[copy], at)?;
        self.merge_last_steps(2);
        Ok(if at > copy { at - 1 } else { at })
    }

    /// The document is a PDF portfolio (`/Collection` in the catalog).
    pub fn is_portfolio(&self) -> bool {
        self.doc.catalog().is_ok_and(|c| c.has(b"Collection"))
    }

    pub(crate) fn file_stream<'a>(
        &'a self,
        spec: &'a Object,
    ) -> Option<(&'a lopdf::Stream, &'a Dictionary)> {
        let spec = resolve(&self.doc, spec).as_dict().ok()?;
        let ef = resolve(&self.doc, spec.get(b"EF").ok()?).as_dict().ok()?;
        let s = ef.get(b"UF").or_else(|_| ef.get(b"F")).ok()?;
        let s = resolve(&self.doc, s).as_stream().ok()?;
        Some((s, spec))
    }

    /// Files embedded in the document (`/EmbeddedFiles`).
    pub fn embedded_files(&self) -> Vec<EmbeddedFile> {
        self.name_tree(b"EmbeddedFiles")
            .into_iter()
            .filter_map(|(name, v)| {
                let (s, spec) = self.file_stream(&v)?;
                let size = s
                    .dict
                    .get(b"Params")
                    .ok()
                    .and_then(|p| resolve(&self.doc, p).as_dict().ok())
                    .and_then(|p| p.get(b"Size").ok())
                    .and_then(|v| resolve(&self.doc, v).as_i64().ok())
                    .map_or(s.content.len(), |v| v.max(0) as usize);
                let modified = s
                    .dict
                    .get(b"Params")
                    .ok()
                    .and_then(|p| resolve(&self.doc, p).as_dict().ok())
                    .and_then(|p| p.get(b"ModDate").ok())
                    .and_then(|v| resolve(&self.doc, v).as_str().ok())
                    .map(|b| date_text(&text(b)))
                    .unwrap_or_default();
                let description = spec
                    .get(b"Desc")
                    .ok()
                    .and_then(|o| resolve(&self.doc, o).as_str().ok())
                    .map(text)
                    .unwrap_or_default();
                let shown = spec
                    .get(b"UF")
                    .or_else(|_| spec.get(b"F"))
                    .ok()
                    .and_then(|o| resolve(&self.doc, o).as_str().ok())
                    .map_or_else(|| name.clone(), text);
                Some(EmbeddedFile {
                    name: shown,
                    size,
                    description,
                    modified,
                })
            })
            .collect()
    }

    /// Bytes of embedded file `index` (in [`PdfDocument::embedded_files`] order).
    pub fn embedded_file_bytes(&self, index: usize) -> Result<Vec<u8>, String> {
        let entries: Vec<(String, Object)> = self
            .name_tree(b"EmbeddedFiles")
            .into_iter()
            .filter(|(_, v)| self.file_stream(v).is_some())
            .collect();
        let (_, v) = entries.get(index).ok_or("no such file")?;
        let (s, _) = self.file_stream(v).ok_or("no such file")?;
        crate::filters::stream_bytes(&self.doc, s)
    }
}
