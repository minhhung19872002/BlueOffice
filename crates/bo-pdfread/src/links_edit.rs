//! Links, bookmarks and article threads for BlueEditor's Edit tab.
//!
//! - Link annotations (ISO 32000-1 §12.5.6.5 `Link`) going to a page (`/Dest`, §12.3.2.2
//!   explicit destination `[page /XYZ left top null]`) or an address (`/A` URI action,
//!   §12.6.4.7); web links found in the page text (`http://`, `https://`, `www.`, e-mail).
//! - The document outline (§12.3.3): the whole tree is written again from a list of items,
//!   so adding, renaming, deleting and reordering bookmarks are one undo step each.
//! - Article threads (§12.4.3): `/Threads` in the catalog, beads linked `/N` `/V`, and the
//!   page's `/B` array.
use crate::annot::{nums, text_obj};
use crate::interp::{apply, invert};
use crate::{LinkTarget, OutlineItem, PdfDocument};
use lopdf::{Dictionary, Object, ObjectId, StringFormat};

/// A link found in page text.
#[derive(Debug, Clone, PartialEq)]
pub struct FoundLink {
    /// Page index.
    pub page: usize,
    /// Boxes (page points, left, top, right, bottom), one per text line it covers.
    pub rect: [f32; 4],
    /// The address (`mailto:` added to e-mail addresses, `http://` to `www.` ones).
    pub uri: String,
}

/// Addresses in `text`: byte ranges and the URI each one opens.
#[must_use]
pub fn find_web_addresses(text: &str) -> Vec<(std::ops::Range<usize>, String)> {
    let mut out = Vec::new();
    let mut start = None;
    let bytes = text.as_bytes();
    let mut words = Vec::new();
    for (i, c) in text.char_indices() {
        if c.is_whitespace() || matches!(c, '<' | '>' | '"' | '(' | '[' | '{') {
            if let Some(s) = start.take() {
                words.push(s..i);
            }
        } else if start.is_none() {
            start = Some(i);
        }
    }
    if let Some(s) = start {
        words.push(s..bytes.len());
    }
    for mut r in words {
        // Trailing punctuation belongs to the sentence.
        while r.end > r.start {
            let last = text[r.start..r.end].chars().next_back().unwrap_or(' ');
            if matches!(
                last,
                '.' | ',' | ';' | ':' | '!' | '?' | ')' | ']' | '}' | '\''
            ) {
                r.end -= last.len_utf8();
            } else {
                break;
            }
        }
        let w = &text[r.clone()];
        let lower = w.to_ascii_lowercase();
        let uri =
            if (lower.starts_with("http://") || lower.starts_with("https://")) && w.len() > 8 {
                Some(w.to_owned())
            } else if lower.starts_with("www.") && w[4..].contains('.') {
                Some(format!("http://{w}"))
            } else if let Some(at) = w.find('@') {
                let (user, host) = (&w[..at], &w[at + 1..]);
                let ok = !user.is_empty()
                    && host.contains('.')
                    && !host.starts_with('.')
                    && !host.ends_with('.')
                    && user.chars().chain(host.chars()).all(|c| {
                        c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '+' | '%')
                    });
                ok.then(|| format!("mailto:{w}"))
            } else {
                None
            };
        if let Some(u) = uri {
            out.push((r, u));
        }
    }
    out
}

impl PdfDocument {
    fn root_id(&self) -> Result<ObjectId, String> {
        self.doc
            .trailer
            .get(b"Root")
            .and_then(Object::as_reference)
            .map_err(|e| e.to_string())
    }

    /// A rectangle in page points (left, top, right, bottom) as user space numbers.
    fn user_rect(&self, page: usize, r: [f32; 4]) -> Result<[f32; 4], String> {
        let info = self.pages.get(page).ok_or("no such page")?;
        let inv = invert(crate::page_matrix(info.crop, info.rotate)).ok_or("page matrix")?;
        let a = apply(inv, r[0], r[1]);
        let b = apply(inv, r[2], r[3]);
        Ok([a.0.min(b.0), a.1.min(b.1), a.0.max(b.0), a.1.max(b.1)])
    }

    /// `[page /XYZ left top null]` for a page target.
    fn dest_array(&self, index: usize, top: Option<f32>) -> Result<Object, String> {
        let info = self.pages.get(index).ok_or("no such page")?;
        let inv = invert(crate::page_matrix(info.crop, info.rotate)).ok_or("page matrix")?;
        let y = top.map_or(Object::Null, |t| {
            Object::Real((apply(inv, 0.0, t).1 * 100.0).round() / 100.0)
        });
        Ok(Object::Array(vec![
            Object::Reference(info.id),
            Object::Name(b"XYZ".to_vec()),
            Object::Null,
            y,
            Object::Null,
        ]))
    }

    fn set_target(&self, d: &mut Dictionary, target: &LinkTarget) -> Result<(), String> {
        match target {
            LinkTarget::Page { index, top } => d.set("Dest", self.dest_array(*index, *top)?),
            LinkTarget::Uri(u) => {
                let mut a = Dictionary::new();
                a.set("S", Object::Name(b"URI".to_vec()));
                a.set(
                    "URI",
                    Object::String(u.as_bytes().to_vec(), StringFormat::Literal),
                );
                d.set("A", a);
            }
        }
        Ok(())
    }

    /// Append annotation `id` to page `page`'s `/Annots` (inside the open step).
    pub(crate) fn push_annot(&mut self, page: usize, id: ObjectId) -> Result<(), String> {
        let pid = self.pages.get(page).ok_or("no such page")?.id;
        let annots = self
            .doc
            .get_dictionary(pid)
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
        self.touch(pid);
        let p = self
            .doc
            .get_dictionary_mut(pid)
            .map_err(|e| e.to_string())?;
        match p.get_mut(b"Annots") {
            Ok(Object::Array(v)) => v.push(Object::Reference(id)),
            _ => p.set("Annots", vec![Object::Reference(id)]),
        }
        Ok(())
    }

    fn link_dict(
        &self,
        page: usize,
        rect: [f32; 4],
        target: &LinkTarget,
    ) -> Result<Dictionary, String> {
        let mut d = Dictionary::new();
        d.set("Type", Object::Name(b"Annot".to_vec()));
        d.set("Subtype", Object::Name(b"Link".to_vec()));
        d.set("Rect", nums(&self.user_rect(page, rect)?));
        // Invisible border (§12.5.2 Border), highlight by inverting (§12.5.6.5 H).
        d.set("Border", Object::Array(vec![0.into(), 0.into(), 0.into()]));
        d.set("H", Object::Name(b"I".to_vec()));
        d.set("F", 4);
        d.set("P", Object::Reference(self.pages[page].id));
        self.set_target(&mut d, target)?;
        Ok(d)
    }

    /// Add a link over `rect` (page points) of page `page` (one undo step).
    pub fn add_link(
        &mut self,
        page: usize,
        rect: [f32; 4],
        target: &LinkTarget,
    ) -> Result<(), String> {
        if let LinkTarget::Page { index, .. } = target {
            if *index >= self.pages.len() {
                return Err("no such page".into());
            }
        }
        let d = self.link_dict(page, rect, target)?;
        self.begin("link");
        let id = self.add(Object::Dictionary(d));
        let r = self.push_annot(page, id);
        self.commit();
        r
    }

    /// Web addresses in the text of `pages` (every page when empty) that have no link yet.
    pub fn find_web_links(&mut self, pages: &[usize]) -> Vec<FoundLink> {
        let list: Vec<usize> = if pages.is_empty() {
            (0..self.pages.len()).collect()
        } else {
            pages.to_vec()
        };
        let mut out = Vec::new();
        for page in list {
            let existing: Vec<[f32; 4]> = self
                .render(
                    page,
                    crate::RenderOptions {
                        annotations: false,
                        ..crate::RenderOptions::default()
                    },
                )
                .map(|p| p.links.into_iter().map(|l| l.rect).collect())
                .unwrap_or_default();
            let Ok(objs) = self.page_objects(page) else {
                continue;
            };
            for line in crate::text_lines(&objs) {
                let idx: Vec<usize> = line.text.char_indices().map(|(i, _)| i).collect();
                for (r, uri) in find_web_addresses(&line.text) {
                    let first = idx.iter().position(|&i| i >= r.start).unwrap_or(0);
                    let last = idx.iter().rposition(|&i| i < r.end).unwrap_or(first);
                    let mut b = [f32::MAX, f32::MAX, f32::MIN, f32::MIN];
                    for c in line.char_boxes.get(first..=last).unwrap_or(&[]) {
                        b = [
                            b[0].min(c[0]),
                            b[1].min(c[1]),
                            b[2].max(c[2]),
                            b[3].max(c[3]),
                        ];
                    }
                    if b[0] >= b[2] {
                        continue;
                    }
                    let cx = f32::midpoint(b[0], b[2]);
                    let cy = f32::midpoint(b[1], b[3]);
                    let linked = existing
                        .iter()
                        .any(|e| cx >= e[0] && cx <= e[2] && cy >= e[1] && cy <= e[3]);
                    if !linked {
                        out.push(FoundLink { page, rect: b, uri });
                    }
                }
            }
        }
        out
    }

    /// Make every web address in the text of `pages` (all when empty) a link; returns how
    /// many were added (one undo step).
    pub fn create_web_links(&mut self, pages: &[usize]) -> Result<usize, String> {
        let found = self.find_web_links(pages);
        if found.is_empty() {
            return Ok(0);
        }
        self.begin("web links");
        let mut r = Ok(found.len());
        for f in &found {
            let d = match self.link_dict(f.page, f.rect, &LinkTarget::Uri(f.uri.clone())) {
                Ok(d) => d,
                Err(e) => {
                    r = Err(e);
                    break;
                }
            };
            let id = self.add(Object::Dictionary(d));
            if let Err(e) = self.push_annot(f.page, id) {
                r = Err(e);
                break;
            }
        }
        self.commit();
        r
    }

    /// Remove the link annotations of `pages` (all when empty); `web_only` keeps links to
    /// pages of this document. Returns how many were removed (one undo step).
    pub fn remove_links(&mut self, pages: &[usize], web_only: bool) -> Result<usize, String> {
        let list: Vec<usize> = if pages.is_empty() {
            (0..self.pages.len()).collect()
        } else {
            pages.to_vec()
        };
        let mut count = 0;
        self.begin("remove links");
        for page in list {
            let Some(pid) = self.pages.get(page).map(|p| p.id) else {
                continue;
            };
            let annots = self
                .doc
                .get_dictionary(pid)
                .ok()
                .and_then(|p| p.get(b"Annots").ok().cloned());
            let (holder, arr) = match annots {
                Some(Object::Reference(r)) => match self.doc.get_object(r) {
                    Ok(Object::Array(a)) => (Some(r), a.clone()),
                    _ => continue,
                },
                Some(Object::Array(a)) => (None, a),
                _ => continue,
            };
            let keep: Vec<Object> = arr
                .iter()
                .filter(|o| {
                    let d = match o {
                        Object::Reference(r) => self.doc.get_dictionary(*r).ok(),
                        Object::Dictionary(d) => Some(d),
                        _ => None,
                    };
                    let Some(d) = d else { return true };
                    if d.get(b"Subtype").and_then(Object::as_name).ok() != Some(b"Link") {
                        return true;
                    }
                    if web_only {
                        let uri = d
                            .get(b"A")
                            .ok()
                            .and_then(|a| match a {
                                Object::Reference(r) => self.doc.get_dictionary(*r).ok(),
                                Object::Dictionary(d) => Some(d),
                                _ => None,
                            })
                            .and_then(|a| a.get(b"S").and_then(Object::as_name).ok())
                            == Some(b"URI");
                        return !uri;
                    }
                    false
                })
                .cloned()
                .collect();
            if keep.len() == arr.len() {
                continue;
            }
            count += arr.len() - keep.len();
            if let Some(r) = holder {
                self.touch(r);
                if let Ok(o) = self.doc.get_object_mut(r) {
                    *o = Object::Array(keep);
                }
            } else {
                self.touch(pid);
                if let Ok(p) = self.doc.get_dictionary_mut(pid) {
                    p.set("Annots", keep);
                }
            }
        }
        self.commit();
        Ok(count)
    }

    /// Replace the document outline with `items` (one undo step; empty removes it).
    pub fn set_outline(&mut self, items: &[OutlineItem]) -> Result<(), String> {
        let root = self.root_id()?;
        self.begin("bookmarks");
        let r = (|| {
            if items.is_empty() {
                self.touch(root);
                if let Ok(c) = self.doc.get_dictionary_mut(root) {
                    c.remove(b"Outlines");
                }
                return Ok(());
            }
            let top = self.add(Object::Dictionary(Dictionary::new()));
            let (first, last, count) = self.write_items(items, top)?;
            let mut d = Dictionary::new();
            d.set("Type", Object::Name(b"Outlines".to_vec()));
            d.set("First", Object::Reference(first));
            d.set("Last", Object::Reference(last));
            d.set("Count", count);
            if let Ok(o) = self.doc.get_object_mut(top) {
                *o = Object::Dictionary(d);
            }
            self.touch(root);
            let c = self
                .doc
                .get_dictionary_mut(root)
                .map_err(|e| e.to_string())?;
            c.set("Outlines", Object::Reference(top));
            Ok(())
        })();
        self.commit();
        r
    }

    /// Write `items` as children of `parent`; returns first, last and the visible count.
    fn write_items(
        &mut self,
        items: &[OutlineItem],
        parent: ObjectId,
    ) -> Result<(ObjectId, ObjectId, i64), String> {
        let ids: Vec<ObjectId> = items
            .iter()
            .map(|_| self.add(Object::Dictionary(Dictionary::new())))
            .collect();
        let mut visible = 0_i64;
        for (k, it) in items.iter().enumerate() {
            let mut d = Dictionary::new();
            d.set("Title", text_obj(&it.title));
            d.set("Parent", Object::Reference(parent));
            if k > 0 {
                d.set("Prev", Object::Reference(ids[k - 1]));
            }
            if let Some(n) = ids.get(k + 1) {
                d.set("Next", Object::Reference(*n));
            }
            if let Some(t) = &it.target {
                match t {
                    LinkTarget::Page { index, .. } if *index >= self.pages.len() => {}
                    _ => self.set_target(&mut d, t)?,
                }
            }
            visible += 1;
            if !it.children.is_empty() {
                let (f, l, c) = self.write_items(&it.children, ids[k])?;
                d.set("First", Object::Reference(f));
                d.set("Last", Object::Reference(l));
                // Positive count: open, its visible descendants; negative: closed (§12.3.3).
                let n = c.max(it.children.len() as i64);
                if it.open {
                    d.set("Count", n);
                    visible += n;
                } else {
                    d.set("Count", -n);
                }
            }
            if let Ok(o) = self.doc.get_object_mut(ids[k]) {
                *o = Object::Dictionary(d);
            }
        }
        let (Some(first), Some(last)) = (ids.first(), ids.last()) else {
            return Err("no bookmarks".into());
        };
        Ok((*first, *last, visible))
    }

    /// Add an article thread titled `title` through `beads` (page, rect in page points) in
    /// reading order (one undo step).
    pub fn add_article(&mut self, title: &str, beads: &[(usize, [f32; 4])]) -> Result<(), String> {
        if beads.is_empty() {
            return Err("an article needs a box".into());
        }
        let root = self.root_id()?;
        let mut rects = Vec::new();
        for (p, r) in beads {
            rects.push((
                *p,
                self.user_rect(*p, *r)?,
                self.pages.get(*p).ok_or("no such page")?.id,
            ));
        }
        self.begin("article");
        let r = (|| {
            let thread = self.add(Object::Dictionary(Dictionary::new()));
            let ids: Vec<ObjectId> = rects
                .iter()
                .map(|_| self.add(Object::Dictionary(Dictionary::new())))
                .collect();
            let n = ids.len();
            for (k, (_, rect, pid)) in rects.iter().enumerate() {
                let mut b = Dictionary::new();
                b.set("Type", Object::Name(b"Bead".to_vec()));
                if k == 0 {
                    b.set("T", Object::Reference(thread));
                }
                // The beads form a ring (§12.4.3 Table 162).
                b.set("N", Object::Reference(ids[(k + 1) % n]));
                b.set("V", Object::Reference(ids[(k + n - 1) % n]));
                b.set("P", Object::Reference(*pid));
                b.set("R", nums(rect));
                if let Ok(o) = self.doc.get_object_mut(ids[k]) {
                    *o = Object::Dictionary(b);
                }
                self.touch(*pid);
                if let Ok(p) = self.doc.get_dictionary_mut(*pid) {
                    match p.get_mut(b"B") {
                        Ok(Object::Array(v)) => v.push(Object::Reference(ids[k])),
                        _ => p.set("B", vec![Object::Reference(ids[k])]),
                    }
                }
            }
            let mut t = Dictionary::new();
            t.set("Type", Object::Name(b"Thread".to_vec()));
            t.set("F", Object::Reference(ids[0]));
            let mut info = Dictionary::new();
            info.set("Title", text_obj(title));
            t.set("I", info);
            if let Ok(o) = self.doc.get_object_mut(thread) {
                *o = Object::Dictionary(t);
            }
            self.touch(root);
            let c = self
                .doc
                .get_dictionary_mut(root)
                .map_err(|e| e.to_string())?;
            match c.get_mut(b"Threads") {
                Ok(Object::Array(v)) => v.push(Object::Reference(thread)),
                _ => c.set("Threads", vec![Object::Reference(thread)]),
            }
            Ok(())
        })();
        self.commit();
        r
    }

    /// Titles and bead counts of the article threads.
    #[must_use]
    pub fn articles(&self) -> Vec<(String, usize)> {
        let Some(list) = self
            .doc
            .catalog()
            .ok()
            .and_then(|c| c.get(b"Threads").ok())
            .and_then(|o| crate::resolve(&self.doc, o).as_array().ok())
        else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for t in list {
            let Ok(t) = crate::resolve(&self.doc, t).as_dict() else {
                continue;
            };
            let title = t
                .get(b"I")
                .ok()
                .and_then(|i| crate::resolve(&self.doc, i).as_dict().ok())
                .and_then(|i| i.get(b"Title").ok())
                .and_then(|s| crate::resolve(&self.doc, s).as_str().ok())
                .map(crate::text_string)
                .unwrap_or_default();
            let mut n = 0;
            let first = t.get(b"F").and_then(Object::as_reference).ok();
            let mut cur = first;
            while let Some(id) = cur {
                n += 1;
                if n > 10_000 {
                    break;
                }
                cur = self
                    .doc
                    .get_dictionary(id)
                    .ok()
                    .and_then(|b| b.get(b"N").and_then(Object::as_reference).ok())
                    .filter(|nx| Some(*nx) != first);
            }
            out.push((title, n));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{standard_fonts, RenderOptions};

    fn doc() -> PdfDocument {
        let objs = [
            "<< /Type /Catalog /Pages 2 0 R >>",
            "<< /Type /Pages /Kids [3 0 R 5 0 R] /Count 2 >>",
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 300] /Resources << /Font << /F1 6 0 R >> >> /Contents 4 0 R >>",
            "<< /Length 80 >>\nstream\nBT /F1 9 Tf 20 250 Td (See https://example.com/a, or mail me@example.org.) Tj ET\nendstream",
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 300] /Resources << >> >>",
            "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>",
        ];
        PdfDocument::open(crate::tests::build_pdf(&objs), standard_fonts()).expect("opens")
    }

    #[test]
    fn addresses_are_found_without_punctuation() {
        let v = find_web_addresses("Go to www.bluestar.vn, https://a.b/c?d=1). Mail x.y@z.com!");
        let uris: Vec<&str> = v.iter().map(|(_, u)| u.as_str()).collect();
        assert_eq!(
            uris,
            [
                "http://www.bluestar.vn",
                "https://a.b/c?d=1",
                "mailto:x.y@z.com"
            ]
        );
        assert_eq!(find_web_addresses("no links here at all. e@ www.").len(), 0);
    }

    #[test]
    fn links_are_added_found_and_removed() {
        let mut d = doc();
        d.add_link(
            0,
            [10.0, 10.0, 60.0, 30.0],
            &LinkTarget::Page {
                index: 1,
                top: Some(20.0),
            },
        )
        .expect("link");
        let page = d.render(0, RenderOptions::default()).expect("renders");
        assert_eq!(page.links.len(), 1);
        assert_eq!(
            page.links[0].target,
            LinkTarget::Page {
                index: 1,
                top: Some(20.0)
            }
        );
        let found = d.find_web_links(&[]);
        assert_eq!(found.len(), 2, "{found:?}");
        assert_eq!(found[0].uri, "https://example.com/a");
        assert_eq!(found[1].uri, "mailto:me@example.org");
        assert_eq!(d.create_web_links(&[]).expect("web links"), 2);
        assert!(
            d.find_web_links(&[]).is_empty(),
            "linked text is not found again"
        );
        let saved = d.save().expect("saves");
        let mut again = PdfDocument::open(saved, standard_fonts()).expect("opens");
        assert_eq!(
            again
                .render(0, RenderOptions::default())
                .expect("r")
                .links
                .len(),
            3
        );
        assert_eq!(again.remove_links(&[], true).expect("removes"), 2);
        assert_eq!(
            again
                .render(0, RenderOptions::default())
                .expect("r")
                .links
                .len(),
            1
        );
        assert!(again.undo());
        assert_eq!(
            again
                .render(0, RenderOptions::default())
                .expect("r")
                .links
                .len(),
            3
        );
    }

    #[test]
    fn outline_is_written_again() {
        let mut d = doc();
        assert_eq!(d.outline().len(), 0);
        let items = vec![
            OutlineItem {
                title: "Chương 1".into(),
                target: Some(LinkTarget::Page {
                    index: 0,
                    top: Some(50.0),
                }),
                open: true,
                children: vec![OutlineItem {
                    title: "Mục 1.1".into(),
                    target: Some(LinkTarget::Page {
                        index: 1,
                        top: None,
                    }),
                    open: false,
                    children: Vec::new(),
                }],
            },
            OutlineItem {
                title: "Web".into(),
                target: Some(LinkTarget::Uri("https://example.com".into())),
                open: false,
                children: Vec::new(),
            },
        ];
        d.set_outline(&items).expect("outline");
        let saved = d.save().expect("saves");
        let again = PdfDocument::open(saved, standard_fonts()).expect("opens");
        assert_eq!(again.outline(), items);
        d.set_outline(&[]).expect("cleared");
        assert_eq!(d.outline().len(), 0);
        assert!(d.undo());
        assert_eq!(d.outline().len(), 2);
    }

    #[test]
    fn article_threads_ring_their_beads() {
        let mut d = doc();
        d.add_article(
            "Bài 1",
            &[
                (0, [10.0, 10.0, 100.0, 100.0]),
                (1, [10.0, 10.0, 100.0, 200.0]),
            ],
        )
        .expect("article");
        let saved = d.save().expect("saves");
        let again = PdfDocument::open(saved, standard_fonts()).expect("opens");
        assert_eq!(again.articles(), vec![("Bài 1".to_owned(), 2)]);
    }
}
