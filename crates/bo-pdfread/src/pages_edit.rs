//! More page organisation for BlueEditor's Organize tab: duplicate, swap, replace and
//! interleave pages (ISO 32000-1 §7.7.3 page tree), resize pages scaling their content
//! (§8.3.4 `cm`, §14.11.2 page boundaries), flatten comments and fields into the page content
//! (§12.5.5 appearance streams) and page labels (§12.4.2 `/PageLabels`).
use crate::{resolve, PdfDocument};
use lopdf::{Dictionary, Object, ObjectId, Stream};

/// Numbering style of a page label range (§12.4.2, Table 159 `/S`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LabelStyle {
    /// Prefix only.
    None,
    /// 1, 2, 3 (`/D`).
    Decimal,
    /// I, II, III (`/R`).
    RomanUpper,
    /// i, ii, iii (`/r`).
    RomanLower,
    /// A … Z, AA (`/A`).
    LettersUpper,
    /// a … z, aa (`/a`).
    LettersLower,
}

impl LabelStyle {
    fn name(self) -> Option<&'static [u8]> {
        Some(match self {
            Self::None => return None,
            Self::Decimal => b"D",
            Self::RomanUpper => b"R",
            Self::RomanLower => b"r",
            Self::LettersUpper => b"A",
            Self::LettersLower => b"a",
        })
    }

    fn from_name(n: Option<&[u8]>) -> Self {
        match n {
            Some(b"D") => Self::Decimal,
            Some(b"R") => Self::RomanUpper,
            Some(b"r") => Self::RomanLower,
            Some(b"A") => Self::LettersUpper,
            Some(b"a") => Self::LettersLower,
            _ => Self::None,
        }
    }
}

/// Pages from `start` on are labelled `prefix` + the number `first`, `first + 1`, … in
/// `style` until the next range.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LabelRange {
    /// First page index of the range.
    pub start: usize,
    /// Numbering style.
    pub style: LabelStyle,
    /// Label prefix (`/P`).
    pub prefix: String,
    /// Number of the range's first page (`/St`, at least 1).
    pub first: u32,
}

/// Annotation keys holding page coordinates in x, y pairs.
const POINT_KEYS: [&[u8]; 5] = [b"Rect", b"QuadPoints", b"Vertices", b"L", b"CL"];

fn pdf_string_text(o: &Object) -> String {
    match o {
        Object::String(b, _) => crate::text_string(b),
        _ => String::new(),
    }
}

impl PdfDocument {
    fn ids(&self) -> Vec<ObjectId> {
        self.pages.iter().map(|p| p.id).collect()
    }

    fn annot_refs(&self, page: ObjectId) -> Vec<ObjectId> {
        self.doc
            .get_dictionary(page)
            .ok()
            .and_then(|d| d.get(b"Annots").ok())
            .and_then(|a| resolve(&self.doc, a).as_array().ok())
            .map(|a| a.iter().filter_map(|o| o.as_reference().ok()).collect())
            .unwrap_or_default()
    }

    fn subtype(&self, id: ObjectId) -> Vec<u8> {
        self.doc
            .get_dictionary(id)
            .ok()
            .and_then(|d| d.get(b"Subtype").ok())
            .and_then(|s| s.as_name().ok())
            .map(<[u8]>::to_vec)
            .unwrap_or_default()
    }

    /// Copy a content stream (or array of them) so the copy can be edited on its own.
    fn copy_contents(&mut self, c: &Object) -> Object {
        match c {
            Object::Reference(r) => match self.doc.get_object(*r) {
                Ok(Object::Stream(s)) => {
                    let s: Stream = s.clone();
                    Object::Reference(self.add(Object::Stream(s)))
                }
                Ok(Object::Array(a)) => {
                    let a = a.clone();
                    Object::Array(a.iter().map(|o| self.copy_contents(o)).collect())
                }
                _ => c.clone(),
            },
            Object::Array(a) => Object::Array(a.iter().map(|o| self.copy_contents(o)).collect()),
            o => o.clone(),
        }
    }

    /// Duplicate `pages`: each copy follows its page (its comments are copied too; form
    /// fields stay on the original). Returns the indices of the copies.
    pub fn duplicate_pages(&mut self, pages: &[usize]) -> Result<Vec<usize>, String> {
        let mut sel: Vec<usize> = pages
            .iter()
            .copied()
            .filter(|&i| i < self.pages.len())
            .collect();
        sel.sort_unstable();
        sel.dedup();
        if sel.is_empty() {
            return Err("no pages".into());
        }
        self.begin("duplicate");
        // Flatten first so inherited attributes are on each page.
        let order = self.ids();
        self.set_page_order(&order)?;
        let mut order = order;
        let mut out = Vec::new();
        for (k, &i) in sel.iter().enumerate() {
            let src = self.pages[i].id;
            let mut d = self
                .doc
                .get_dictionary(src)
                .map_err(|e| e.to_string())?
                .clone();
            d.remove(b"Annots");
            d.remove(b"StructParents");
            if let Ok(c) = d.get(b"Contents").cloned() {
                let c = self.copy_contents(&c);
                d.set("Contents", c);
            }
            let id = self.add(Object::Dictionary(d));
            let mut annots = Vec::new();
            for a in self.annot_refs(src) {
                let st = self.subtype(a);
                if matches!(st.as_slice(), b"Widget" | b"Popup") {
                    continue;
                }
                if let Ok(mut ad) = self.doc.get_dictionary(a).cloned() {
                    ad.set("P", Object::Reference(id));
                    ad.remove(b"Popup");
                    ad.remove(b"IRT");
                    annots.push(Object::Reference(self.add(Object::Dictionary(ad))));
                }
            }
            if !annots.is_empty() {
                if let Ok(d) = self.doc.get_dictionary_mut(id) {
                    d.set("Annots", annots);
                }
            }
            let at = i + k + 1;
            order.insert(at, id);
            out.push(at);
        }
        self.set_page_order(&order)?;
        self.commit();
        Ok(out)
    }

    /// Swap pages `a` and `b`.
    pub fn swap_pages(&mut self, a: usize, b: usize) -> Result<(), String> {
        let n = self.pages.len();
        if a >= n || b >= n {
            return Err("no such page".into());
        }
        if a == b {
            return Ok(());
        }
        let mut order = self.ids();
        order.swap(a, b);
        self.begin("swap");
        self.set_page_order(&order)?;
        self.commit();
        Ok(())
    }

    /// Replace `pages` (in order) by the pages of another PDF (all, or `pick`), put where the
    /// first replaced page was; one undo step. Returns how many pages came in.
    pub fn replace_pages(
        &mut self,
        pages: &[usize],
        bytes: &[u8],
        pick: Option<&[usize]>,
    ) -> Result<usize, String> {
        let mut sel: Vec<usize> = pages
            .iter()
            .copied()
            .filter(|&i| i < self.pages.len())
            .collect();
        sel.sort_unstable();
        sel.dedup();
        let first = *sel.first().ok_or("no pages")?;
        self.begin_group("replace");
        let r = (|| {
            let n = self.insert_pdf(first, bytes, pick)?;
            self.refresh_pages();
            let old: Vec<usize> = sel.iter().map(|&i| i + n).collect();
            let drop: std::collections::HashSet<usize> = old.into_iter().collect();
            let order: Vec<ObjectId> = self
                .pages
                .iter()
                .enumerate()
                .filter(|(i, _)| !drop.contains(i))
                .map(|(_, p)| p.id)
                .collect();
            self.set_page_order(&order)?;
            Ok(n)
        })();
        self.end_group();
        if r.is_err() {
            self.refresh_pages();
        }
        r
    }

    /// Merge another PDF page by page: this document's page 1, the other's page 1, page 2,
    /// … (the other's pages last to first when `reverse`, for a stack scanned backwards);
    /// the longer document's remaining pages follow. Returns how many pages came in.
    pub fn interleave_pdf(&mut self, bytes: &[u8], reverse: bool) -> Result<usize, String> {
        let own = self.pages.len();
        self.begin_group("interleave");
        let r = (|| {
            let n = self.append_pdf(bytes)?;
            self.refresh_pages();
            let ids = self.ids();
            let (a, b) = ids.split_at(own.min(ids.len()));
            let mut b = b.to_vec();
            if reverse {
                b.reverse();
            }
            let mut order = Vec::with_capacity(ids.len());
            for k in 0..a.len().max(b.len()) {
                if let Some(x) = a.get(k) {
                    order.push(*x);
                }
                if let Some(x) = b.get(k) {
                    order.push(*x);
                }
            }
            self.set_page_order(&order)?;
            Ok(n)
        })();
        self.end_group();
        if r.is_err() {
            self.refresh_pages();
        }
        r
    }

    /// Give `pages` the size `width` × `height` points (as shown, after rotation). With
    /// `fit` the content is scaled to fit and centred, otherwise it keeps its size and is
    /// centred. Comments move with the content.
    pub fn resize_pages(
        &mut self,
        pages: &[usize],
        width: f32,
        height: f32,
        fit: bool,
    ) -> Result<(), String> {
        if !(width >= 3.0 && height >= 3.0 && width <= 14_400.0 && height <= 14_400.0) {
            return Err("page size out of range".into());
        }
        let sel: Vec<usize> = pages
            .iter()
            .copied()
            .filter(|&i| i < self.pages.len())
            .collect();
        if sel.is_empty() {
            return Err("no pages".into());
        }
        self.begin("resize pages");
        let r = (|| {
            let order = self.ids();
            self.set_page_order(&order)?;
            for i in sel {
                let info = self.pages[i].clone();
                let c = info.crop;
                let (mw, mh) = if info.rotate % 180 == 90 {
                    (height, width)
                } else {
                    (width, height)
                };
                let (cw, ch) = ((c[2] - c[0]).max(1.0), (c[3] - c[1]).max(1.0));
                let s = if fit { (mw / cw).min(mh / ch) } else { 1.0 };
                let tx = (mw - cw * s) / 2.0 - c[0] * s;
                let ty = (mh - ch * s) / 2.0 - c[1] * s;
                let m = [s, 0.0, 0.0, s, tx, ty];
                let (sid, content) = self.page_stream(i)?;
                let prefix = format!("{s} 0 0 {s} {tx} {ty} cm\n").into_bytes();
                self.write_page(sid, content, vec![((0, 0), prefix)], b"");
                self.touch(info.id);
                if let Ok(d) = self.doc.get_dictionary_mut(info.id) {
                    d.set(
                        "MediaBox",
                        vec![0.into(), 0.into(), Object::Real(mw), Object::Real(mh)],
                    );
                    for k in [&b"CropBox"[..], b"BleedBox", b"TrimBox", b"ArtBox"] {
                        d.remove(k);
                    }
                }
                for a in self.annot_refs(info.id) {
                    self.touch(a);
                    if let Ok(d) = self.doc.get_dictionary_mut(a) {
                        map_points(d, m);
                    }
                }
            }
            Ok(())
        })();
        self.commit();
        self.refresh_pages();
        r
    }

    /// Draw the comments and form fields of `pages` into their content and remove them
    /// (links stay). Returns how many were flattened.
    pub fn flatten_pages(&mut self, pages: &[usize]) -> Result<usize, String> {
        let sel: Vec<usize> = pages
            .iter()
            .copied()
            .filter(|&i| i < self.pages.len())
            .collect();
        self.begin("flatten");
        let r = (|| {
            let mut n = 0;
            let mut widgets = Vec::new();
            for page in sel {
                let pid = self.pages[page].id;
                let mut tail = String::new();
                let mut gone = Vec::new();
                for a in self.annot_refs(pid) {
                    let st = self.subtype(a);
                    if st == b"Link" {
                        continue;
                    }
                    if st == b"Popup" {
                        gone.push(a);
                        continue;
                    }
                    let Some((ap, m)) = self.widget_placement(a) else {
                        continue;
                    };
                    let name = self.add_resource(page, b"XObject", "BoFl", ap)?;
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
                    if st == b"Widget" {
                        widgets.push(a);
                    }
                    gone.push(a);
                    n += 1;
                }
                if gone.iter().any(|a| self.subtype(*a) != b"Popup") {
                    let (sid, content) = self.page_stream(page)?;
                    self.write_page(sid, content, Vec::new(), tail.as_bytes());
                }
                if !gone.is_empty() {
                    self.remove_annots(page, &gone)?;
                }
            }
            self.remove_fields(&widgets);
            Ok(n)
        })();
        self.commit();
        r
    }

    /// Take widgets out of the form (`/AcroForm /Fields` and their parents' `/Kids`).
    fn remove_fields(&mut self, widgets: &[ObjectId]) {
        if widgets.is_empty() {
            return;
        }
        let Ok(root) = self.doc.trailer.get(b"Root").and_then(Object::as_reference) else {
            return;
        };
        let mut parents = Vec::new();
        for &w in widgets {
            let parent = self
                .doc
                .get_dictionary(w)
                .ok()
                .and_then(|d| d.get(b"Parent").ok())
                .and_then(|p| p.as_reference().ok());
            if let Some(p) = parent {
                self.touch(p);
                if let Ok(d) = self.doc.get_dictionary_mut(p) {
                    if let Ok(Object::Array(k)) = d.get_mut(b"Kids") {
                        k.retain(|o| o.as_reference().ok() != Some(w));
                        if k.is_empty() {
                            parents.push(p);
                        }
                    }
                }
            }
        }
        let gone: Vec<ObjectId> = widgets.iter().copied().chain(parents).collect();
        let fields = self
            .doc
            .get_dictionary(root)
            .ok()
            .and_then(|c| c.get(b"AcroForm").ok())
            .cloned();
        let keep = |a: &mut Vec<Object>| {
            a.retain(|o| o.as_reference().map_or(true, |r| !gone.contains(&r)));
        };
        match fields {
            Some(Object::Reference(f)) => {
                self.touch(f);
                if let Ok(d) = self.doc.get_dictionary_mut(f) {
                    if let Ok(Object::Array(a)) = d.get_mut(b"Fields") {
                        keep(a);
                    }
                }
            }
            Some(Object::Dictionary(_)) => {
                self.touch(root);
                if let Ok(c) = self.doc.get_dictionary_mut(root) {
                    if let Ok(Object::Dictionary(f)) = c.get_mut(b"AcroForm") {
                        if let Ok(Object::Array(a)) = f.get_mut(b"Fields") {
                            keep(a);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    /// The page label ranges (empty when the document has no `/PageLabels`).
    pub fn page_label_ranges(&self) -> Vec<LabelRange> {
        fn walk(
            doc: &crate::store::Document,
            node: &Dictionary,
            out: &mut Vec<LabelRange>,
            depth: u32,
        ) {
            if depth > 16 {
                return;
            }
            if let Some(nums) = node
                .get(b"Nums")
                .ok()
                .and_then(|o| resolve(doc, o).as_array().ok())
            {
                for pair in nums.chunks(2) {
                    if let [k, v] = pair {
                        if let (Ok(k), Ok(d)) =
                            (resolve(doc, k).as_i64(), resolve(doc, v).as_dict())
                        {
                            out.push(LabelRange {
                                start: k.max(0) as usize,
                                style: LabelStyle::from_name(
                                    d.get(b"S").ok().and_then(|s| s.as_name().ok()),
                                ),
                                prefix: d.get(b"P").map(pdf_string_text).unwrap_or_default(),
                                first: d
                                    .get(b"St")
                                    .ok()
                                    .and_then(|o| o.as_i64().ok())
                                    .map_or(1, |v| v.clamp(1, i64::from(u32::MAX)) as u32),
                            });
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
            .and_then(|c| c.get(b"PageLabels").ok())
            .and_then(|o| resolve(&self.doc, o).as_dict().ok())
        {
            walk(&self.doc, root, &mut out, 0);
        }
        out.sort_by_key(|r| r.start);
        out
    }

    /// Number pages `first..=last` with `style`, `prefix` and `start` (Format Page
    /// Numbers); the pages after keep their labels. Writes `/PageLabels` (§12.4.2).
    pub fn set_page_labels(
        &mut self,
        first: usize,
        last: usize,
        style: LabelStyle,
        prefix: &str,
        start: u32,
    ) -> Result<(), String> {
        let n = self.pages.len();
        if first > last || last >= n {
            return Err("no such page".into());
        }
        let ranges = self.page_label_ranges();
        // Each page's (style, prefix, number) now.
        let mut per: Vec<(LabelStyle, String, u32)> = (0..n)
            .map(|i| match ranges.iter().rev().find(|r| r.start <= i) {
                Some(r) => (
                    r.style,
                    r.prefix.clone(),
                    r.first.saturating_add((i - r.start) as u32),
                ),
                None => (LabelStyle::Decimal, String::new(), i as u32 + 1),
            })
            .collect();
        for (k, p) in per.iter_mut().enumerate().take(last + 1).skip(first) {
            *p = (
                style,
                prefix.to_owned(),
                start.max(1).saturating_add((k - first) as u32),
            );
        }
        let mut nums = Vec::new();
        let mut prev: Option<&(LabelStyle, String, u32)> = None;
        for (i, p) in per.iter().enumerate() {
            let continues = prev.is_some_and(|q| {
                q.0 == p.0
                    && q.1 == p.1
                    && (p.0 == LabelStyle::None || q.2.saturating_add(1) == p.2)
            });
            if !continues {
                let mut d = Dictionary::new();
                if let Some(s) = p.0.name() {
                    d.set("S", Object::Name(s.to_vec()));
                }
                if !p.1.is_empty() {
                    d.set("P", crate::annot::text_obj(&p.1));
                }
                if p.2 != 1 {
                    d.set("St", i64::from(p.2));
                }
                nums.push(Object::Integer(i as i64));
                nums.push(Object::Dictionary(d));
            }
            prev = Some(p);
        }
        let root = self
            .doc
            .trailer
            .get(b"Root")
            .and_then(Object::as_reference)
            .map_err(|e| e.to_string())?;
        self.begin("page labels");
        self.touch(root);
        if let Ok(c) = self.doc.get_dictionary_mut(root) {
            let mut l = Dictionary::new();
            l.set("Nums", nums);
            c.set("PageLabels", l);
        }
        self.commit();
        self.refresh_pages();
        Ok(())
    }
}

/// Move the page coordinates of an annotation by matrix `m` (scale and translate).
fn map_points(d: &mut Dictionary, m: [f32; 6]) {
    let map = |a: &mut Vec<Object>| {
        for (k, v) in a.iter_mut().enumerate() {
            if let Ok(f) = v.as_float() {
                *v = Object::Real(if k % 2 == 0 {
                    f * m[0] + m[4]
                } else {
                    f * m[3] + m[5]
                });
            }
        }
    };
    for key in POINT_KEYS {
        if let Ok(Object::Array(a)) = d.get_mut(key) {
            map(a);
        }
    }
    if let Ok(Object::Array(ink)) = d.get_mut(b"InkList") {
        for s in ink.iter_mut() {
            if let Object::Array(a) = s {
                map(a);
            }
        }
    }
    // The appearance stream is placed on /Rect (§12.5.5), so it scales with it.
}

#[cfg(test)]
#[path = "pages_edit_tests.rs"]
mod tests;
