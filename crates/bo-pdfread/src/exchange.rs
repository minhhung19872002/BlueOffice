//! Comment exchange and summary for BlueReader's Comment tab: export to FDF (ISO 32000-1
//! §12.7.8, the annotation dictionaries with their appearance streams) and XFDF (ISO 19444-1,
//! the XML form), import of both (FDF annotations copied with their objects; annotations
//! without an appearance — every XFDF one — rebuilt through [`PdfDocument::add_annotation`]
//! and [`PdfDocument::add_markup`]), and a summary of the comments as a new PDF (text drawn as
//! glyph outlines of the bundled sans face, so any script shows).
use crate::annot::{nums, text_obj};
use crate::interp::{apply, M};
use crate::markup::{Markup, MarkupKind};
use crate::{resolve, text_string, AnnotKind, NewAnnot, PdfDocument};
use lopdf::{Dictionary, Document, Object, ObjectId, Stream, StringFormat};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt::Write as _;

/// Subtypes exchanged (popups, links and widgets stay in the document).
const EXCHANGED: [&str; 15] = [
    "Text",
    "FreeText",
    "Line",
    "Square",
    "Circle",
    "Polygon",
    "PolyLine",
    "Highlight",
    "Underline",
    "Squiggly",
    "StrikeOut",
    "Stamp",
    "Caret",
    "Ink",
    "FileAttachment",
];

/// One comment in the exchange form (user space, y up).
#[derive(Debug, Clone, Default, PartialEq)]
struct Item {
    subtype: String,
    page: usize,
    rect: [f32; 4],
    color: Option<[u8; 3]>,
    name: String,
    author: String,
    date: String,
    subject: String,
    contents: String,
    flags: i64,
    opacity: Option<f32>,
    width: Option<f32>,
    icon: String,
    /// `QuadPoints` of text markup.
    coords: Vec<f32>,
    /// `L` of a line.
    line: Vec<f32>,
    /// Arrow at the line's end (`/LE`).
    arrow: bool,
    /// `Vertices` of polygons and polylines.
    vertices: Vec<f32>,
    /// `InkList` strokes.
    ink: Vec<Vec<f32>>,
}

/// Labels of a comment summary (localised by the caller).
#[derive(Debug, Clone, PartialEq)]
pub struct SummaryLabels {
    /// Heading, `{file}` replaced by the document's name.
    pub title: String,
    /// `Page {n}`.
    pub page: String,
    /// "Number", "Author", "Subject", "Date".
    pub number: String,
    /// Author.
    pub author: String,
    /// Subject.
    pub subject: String,
    /// Date.
    pub date: String,
}

fn floats(doc: &Document, o: Option<&Object>) -> Vec<f32> {
    o.map(|o| resolve(doc, o))
        .and_then(|o| o.as_array().ok())
        .map(|a| {
            a.iter()
                .filter_map(|o| resolve(doc, o).as_float().ok())
                .collect()
        })
        .unwrap_or_default()
}

fn xml_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\r' => out.push_str("&#13;"),
            c => out.push(c),
        }
    }
    out
}

fn xml_unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        let tail = &rest[i..];
        let Some(end) = tail.find(';') else {
            out.push_str(tail);
            return out;
        };
        let ent = &tail[1..end];
        let c = match ent {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            _ => ent
                .strip_prefix("#x")
                .and_then(|h| u32::from_str_radix(h, 16).ok())
                .or_else(|| ent.strip_prefix('#').and_then(|d| d.parse().ok()))
                .and_then(char::from_u32),
        };
        match c {
            Some(c) => out.push(c),
            None => out.push_str(&tail[..=end]),
        }
        rest = &tail[end + 1..];
    }
    out.push_str(rest);
    out
}

fn join(v: &[f32]) -> String {
    v.iter()
        .map(|x| crate::objects::num(*x))
        .collect::<Vec<_>>()
        .join(",")
}

fn split_nums(s: &str) -> Vec<f32> {
    s.split([',', ';', ' ', '\n', '\r', '\t'])
        .filter_map(|t| t.trim().parse().ok())
        .collect()
}

fn hex_color(c: [u8; 3]) -> String {
    format!("#{:02X}{:02X}{:02X}", c[0], c[1], c[2])
}

fn parse_hex(s: &str) -> Option<[u8; 3]> {
    let h = s.trim().strip_prefix('#')?;
    if h.len() != 6 {
        return None;
    }
    let v = u32::from_str_radix(h, 16).ok()?;
    Some([(v >> 16) as u8, (v >> 8) as u8, v as u8])
}

/// The XFDF element of a subtype.
fn element(subtype: &str) -> String {
    subtype.to_ascii_lowercase()
}

fn subtype_of(element: &str) -> Option<&'static str> {
    EXCHANGED
        .iter()
        .find(|s| s.eq_ignore_ascii_case(element))
        .copied()
}

impl PdfDocument {
    /// The page's annotations that are exchanged: (object id when indirect, dictionary).
    fn exchanged(&self, page: usize) -> Vec<(Option<ObjectId>, Dictionary)> {
        let doc = &self.doc;
        let Some(info) = self.pages.get(page) else {
            return Vec::new();
        };
        let Ok(d) = doc.get_dictionary(info.id) else {
            return Vec::new();
        };
        let Some(list) = d
            .get(b"Annots")
            .ok()
            .and_then(|o| resolve(doc, o).as_array().ok())
        else {
            return Vec::new();
        };
        list.iter()
            .filter_map(|a| {
                let d = resolve(doc, a).as_dict().ok()?;
                let st = d.get(b"Subtype").and_then(Object::as_name).ok()?;
                EXCHANGED
                    .iter()
                    .any(|s| s.as_bytes() == st)
                    .then(|| (a.as_reference().ok(), d.clone()))
            })
            .collect()
    }

    fn item(&self, page: usize, d: &Dictionary) -> Item {
        let doc = &self.doc;
        let s = |k: &[u8]| {
            d.get(k)
                .ok()
                .and_then(|o| resolve(doc, o).as_str().ok())
                .map(text_string)
                .unwrap_or_default()
        };
        let rect = floats(doc, d.get(b"Rect").ok());
        let color = match floats(doc, d.get(b"C").ok()).as_slice() {
            [g] => Some([(g * 255.0).round() as u8; 3]),
            [r, g, b] => Some([r, g, b].map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8)),
            _ => None,
        };
        let width = d
            .get(b"BS")
            .ok()
            .and_then(|o| resolve(doc, o).as_dict().ok())
            .and_then(|bs| bs.get(b"W").ok())
            .and_then(|o| resolve(doc, o).as_float().ok());
        let ink = d
            .get(b"InkList")
            .ok()
            .and_then(|o| resolve(doc, o).as_array().ok())
            .map(|a| a.iter().map(|o| floats(doc, Some(o))).collect())
            .unwrap_or_default();
        let arrow = d
            .get(b"LE")
            .ok()
            .and_then(|o| resolve(doc, o).as_array().ok())
            .is_some_and(|a| {
                a.iter()
                    .any(|o| o.as_name().is_ok_and(|n| n.ends_with(b"Arrow")))
            });
        Item {
            subtype: d
                .get(b"Subtype")
                .and_then(Object::as_name)
                .map(|n| String::from_utf8_lossy(n).into_owned())
                .unwrap_or_default(),
            page,
            rect: <[f32; 4]>::try_from(rect.as_slice()).unwrap_or_default(),
            color,
            name: s(b"NM"),
            author: s(b"T"),
            date: s(b"M"),
            subject: s(b"Subj"),
            contents: s(b"Contents"),
            flags: d
                .get(b"F")
                .ok()
                .and_then(|o| resolve(doc, o).as_i64().ok())
                .unwrap_or(4),
            opacity: d
                .get(b"CA")
                .ok()
                .and_then(|o| resolve(doc, o).as_float().ok()),
            width,
            icon: d
                .get(b"Name")
                .and_then(Object::as_name)
                .map(|n| String::from_utf8_lossy(n).into_owned())
                .unwrap_or_default(),
            coords: floats(doc, d.get(b"QuadPoints").ok()),
            line: floats(doc, d.get(b"L").ok()),
            arrow,
            vertices: floats(doc, d.get(b"Vertices").ok()),
            ink,
        }
    }

    fn items(&self) -> Vec<Item> {
        (0..self.pages.len())
            .flat_map(|p| {
                self.exchanged(p)
                    .into_iter()
                    .map(move |(_, d)| (p, d))
                    .collect::<Vec<_>>()
            })
            .map(|(p, d)| self.item(p, &d))
            .collect()
    }

    /// The comments as XFDF (ISO 19444-1); `file` is the document's file name.
    #[must_use]
    pub fn export_xfdf(&self, file: &str) -> String {
        let mut x = String::from(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<xfdf xmlns=\"http://ns.adobe.com/xfdf/\" xml:space=\"preserve\">\n<annots>\n",
        );
        for it in self.items() {
            let _ = write!(
                x,
                "<{} page=\"{}\" rect=\"{}\" flags=\"{}\"",
                element(&it.subtype),
                it.page,
                join(&it.rect),
                it.flags
            );
            let mut attr = |k: &str, v: &str| {
                if !v.is_empty() {
                    let _ = write!(x, " {k}=\"{}\"", xml_escape(v));
                }
            };
            attr("color", &it.color.map(hex_color).unwrap_or_default());
            attr("name", &it.name);
            attr("title", &it.author);
            attr("date", &it.date);
            attr("subject", &it.subject);
            attr("icon", &it.icon);
            attr(
                "opacity",
                &it.opacity.map(|o| o.to_string()).unwrap_or_default(),
            );
            attr(
                "width",
                &it.width.map(|o| o.to_string()).unwrap_or_default(),
            );
            attr("coords", &join(&it.coords));
            if it.line.len() == 4 {
                attr("start", &join(&it.line[..2]));
                attr("end", &join(&it.line[2..]));
                if it.arrow {
                    attr("tail", "OpenArrow");
                }
            }
            x.push_str(">\n");
            if !it.contents.is_empty() {
                let _ = writeln!(x, "<contents>{}</contents>", xml_escape(&it.contents));
            }
            if !it.vertices.is_empty() {
                let _ = writeln!(x, "<vertices>{}</vertices>", join(&it.vertices));
            }
            if !it.ink.is_empty() {
                x.push_str("<inklist>");
                for g in &it.ink {
                    let pts: Vec<String> = g.chunks_exact(2).map(join).collect();
                    let _ = write!(x, "<gesture>{}</gesture>", pts.join(";"));
                }
                x.push_str("</inklist>\n");
            }
            let _ = writeln!(x, "</{}>", element(&it.subtype));
        }
        let _ = write!(
            x,
            "</annots>\n<f href=\"{}\"/>\n</xfdf>\n",
            xml_escape(file)
        );
        x
    }

    /// The comments as FDF (§12.7.8): each annotation dictionary with its `/Page` and the
    /// objects it uses (appearance streams, embedded files); `file` names the document.
    pub fn export_fdf(&self, file: &str) -> Result<Vec<u8>, String> {
        let mut out = Document::with_version("1.2");
        let mut map: HashMap<ObjectId, ObjectId> = HashMap::new();
        let mut refs = Vec::new();
        for page in 0..self.pages.len() {
            for (_, mut d) in self.exchanged(page) {
                for k in [&b"P"[..], b"Popup", b"IRT", b"Parent", b"StructParent"] {
                    d.remove(k);
                }
                d.set("Page", page as i64);
                let o = copy_object(&self.doc, &mut out, &Object::Dictionary(d), &mut map, 0);
                refs.push(Object::Reference(out.add_object(o)));
            }
        }
        let mut fdf = Dictionary::new();
        fdf.set("Annots", refs);
        fdf.set("F", text_obj(file));
        let mut cat = Dictionary::new();
        cat.set("FDF", fdf);
        let root = out.add_object(cat);
        out.trailer.set("Root", Object::Reference(root));
        let mut bytes = Vec::new();
        out.save_to(&mut bytes).map_err(|e| e.to_string())?;
        // Same length header: %PDF- → %FDF- (§12.7.8.2).
        if bytes.starts_with(b"%PDF-") {
            bytes[1..4].copy_from_slice(b"FDF");
        }
        Ok(bytes)
    }

    /// Import the comments of an FDF or XFDF file (one undo step per comment); comments whose
    /// name (`/NM`) the document already has are skipped. Returns how many were added.
    pub fn import_comments(&mut self, bytes: &[u8]) -> Result<usize, String> {
        let head = &bytes[..bytes.len().min(1024)];
        if head.starts_with(b"%FDF-") || head.starts_with(b"%PDF-") {
            return self.import_fdf(bytes);
        }
        let text = String::from_utf8_lossy(bytes);
        if text.contains("<xfdf") {
            return Ok(self.import_xfdf(&text));
        }
        Err("not an FDF or XFDF file".into())
    }

    fn existing_names(&self) -> HashSet<String> {
        self.items()
            .into_iter()
            .map(|i| i.name)
            .filter(|n| !n.is_empty())
            .collect()
    }

    fn import_fdf(&mut self, bytes: &[u8]) -> Result<usize, String> {
        let mut b = bytes.to_vec();
        if b.starts_with(b"%FDF-") {
            b[1..4].copy_from_slice(b"PDF");
        }
        let src = Document::load_mem(&b).map_err(|e| e.to_string())?;
        let root = src
            .trailer
            .get(b"Root")
            .ok()
            .map(|o| resolve(&src, o))
            .and_then(|o| o.as_dict().ok())
            .ok_or("FDF without a catalogue")?;
        let fdf = root
            .get(b"FDF")
            .ok()
            .map(|o| resolve(&src, o))
            .and_then(|o| o.as_dict().ok())
            .ok_or("FDF without /FDF")?;
        let list: Vec<Object> = fdf
            .get(b"Annots")
            .ok()
            .map(|o| resolve(&src, o))
            .and_then(|o| o.as_array().ok())
            .cloned()
            .unwrap_or_default();
        let names = self.existing_names();
        let mut n = 0;
        for a in &list {
            let Ok(d) = resolve(&src, a).as_dict() else {
                continue;
            };
            let page = d
                .get(b"Page")
                .ok()
                .and_then(|o| resolve(&src, o).as_i64().ok())
                .and_then(|p| usize::try_from(p).ok())
                .unwrap_or(0);
            let Some(info) = self.pages.get(page).cloned() else {
                continue;
            };
            let name = d
                .get(b"NM")
                .ok()
                .and_then(|o| resolve(&src, o).as_str().ok())
                .map(text_string)
                .unwrap_or_default();
            if !name.is_empty() && names.contains(&name) {
                continue;
            }
            if d.get(b"AP").is_err() {
                // No appearance: rebuild it like an XFDF comment.
                let it = item_of(&src, page, d);
                if self.add_item(&it).is_ok() {
                    n += 1;
                }
                continue;
            }
            let mut map = HashMap::new();
            self.begin("comment");
            let mut copy = Dictionary::new();
            for (k, v) in d {
                if matches!(k.as_slice(), b"Page" | b"Popup" | b"IRT" | b"Parent" | b"P") {
                    continue;
                }
                let o = self.copy_in(&src, v, &mut map);
                copy.set(k.clone(), o);
            }
            copy.set("P", Object::Reference(info.id));
            let id = self.add(Object::Dictionary(copy));
            self.link_annot(info.id, id)?;
            self.commit();
            n += 1;
        }
        Ok(n)
    }

    /// Copy `o` of `src` into this document (new objects through the edit log).
    fn copy_in(
        &mut self,
        src: &Document,
        o: &Object,
        map: &mut HashMap<ObjectId, ObjectId>,
    ) -> Object {
        match o {
            Object::Reference(r) => {
                if let Some(n) = map.get(r) {
                    return Object::Reference(*n);
                }
                let Ok(t) = src.get_object(*r) else {
                    return Object::Null;
                };
                let t = t.clone();
                // Reserve the number first so cycles end.
                let id = self.add(Object::Null);
                map.insert(*r, id);
                let c = self.copy_in(src, &t, map);
                if let Ok(slot) = self.doc.get_object_mut(id) {
                    *slot = c;
                }
                Object::Reference(id)
            }
            Object::Array(v) => {
                Object::Array(v.iter().map(|x| self.copy_in(src, x, map)).collect())
            }
            Object::Dictionary(d) => {
                let mut nd = Dictionary::new();
                for (k, v) in d {
                    if k.as_slice() == b"P" || k.as_slice() == b"Parent" {
                        continue;
                    }
                    nd.set(k.clone(), self.copy_in(src, v, map));
                }
                Object::Dictionary(nd)
            }
            Object::Stream(s) => {
                let mut nd = Dictionary::new();
                for (k, v) in &s.dict {
                    nd.set(k.clone(), self.copy_in(src, v, map));
                }
                Object::Stream(Stream::new(nd, s.content.clone()).with_compression(false))
            }
            other => other.clone(),
        }
    }

    /// Add annotation `id` to the page's `/Annots`.
    fn link_annot(&mut self, page: ObjectId, id: ObjectId) -> Result<(), String> {
        let pd = self.doc.get_dictionary(page).map_err(|e| e.to_string())?;
        if let Ok(Object::Reference(arr)) = pd.get(b"Annots") {
            let arr = *arr;
            self.touch(arr);
            if let Ok(Object::Array(v)) = self.doc.get_object_mut(arr) {
                v.push(Object::Reference(id));
            }
            return Ok(());
        }
        self.touch(page);
        let pd = self
            .doc
            .get_dictionary_mut(page)
            .map_err(|e| e.to_string())?;
        match pd.get_mut(b"Annots") {
            Ok(Object::Array(v)) => v.push(Object::Reference(id)),
            _ => pd.set("Annots", vec![Object::Reference(id)]),
        }
        Ok(())
    }

    fn import_xfdf(&mut self, text: &str) -> usize {
        let names = self.existing_names();
        let mut n = 0;
        for it in parse_xfdf(text) {
            if !it.name.is_empty() && names.contains(&it.name) {
                continue;
            }
            if self.add_item(&it).is_ok() {
                n += 1;
            }
        }
        n
    }

    /// Rebuild an exchanged comment (user-space geometry) with our own appearance.
    fn add_item(&mut self, it: &Item) -> Result<(), String> {
        let info = self.pages.get(it.page).ok_or("no such page")?.clone();
        let base: M = crate::page_matrix(info.crop, info.rotate);
        let dev = |x: f32, y: f32| {
            let p = apply(base, x, y);
            [p.0, p.1]
        };
        let rect = crate::device_rect(base, it.rect);
        let color = it.color.unwrap_or([255, 220, 0]);
        let points =
            |v: &[f32]| -> Vec<[f32; 2]> { v.chunks_exact(2).map(|p| dev(p[0], p[1])).collect() };
        let markup = |kind: MarkupKind, points: Vec<[f32; 2]>| Markup {
            kind,
            page: it.page,
            rect,
            points,
            color,
            opacity: it.opacity.unwrap_or(1.0),
            width: it.width.unwrap_or(1.0),
            author: it.author.clone(),
            contents: it.contents.clone(),
            font_size: 12.0,
        };
        let note = |kind: AnnotKind, rects: Vec<[f32; 4]>| NewAnnot {
            kind,
            page: it.page,
            rects,
            color,
            author: it.author.clone(),
            contents: it.contents.clone(),
        };
        let before = self.edit_count();
        match it.subtype.as_str() {
            "Highlight" | "Underline" | "StrikeOut" | "Squiggly" => {
                let kind = match it.subtype.as_str() {
                    "Highlight" => AnnotKind::Highlight,
                    "Underline" => AnnotKind::Underline,
                    "StrikeOut" => AnnotKind::StrikeOut,
                    _ => AnnotKind::Squiggly,
                };
                let rects: Vec<[f32; 4]> = if it.coords.len() >= 8 {
                    it.coords
                        .chunks_exact(8)
                        .map(|q| {
                            let xs = [q[0], q[2], q[4], q[6]];
                            let ys = [q[1], q[3], q[5], q[7]];
                            crate::device_rect(
                                base,
                                [
                                    xs.iter().copied().fold(f32::INFINITY, f32::min),
                                    ys.iter().copied().fold(f32::INFINITY, f32::min),
                                    xs.iter().copied().fold(f32::NEG_INFINITY, f32::max),
                                    ys.iter().copied().fold(f32::NEG_INFINITY, f32::max),
                                ],
                            )
                        })
                        .collect()
                } else {
                    vec![rect]
                };
                self.add_annotation(&note(kind, rects))?;
            }
            "Text" => self.add_annotation(&note(AnnotKind::Note, vec![rect]))?,
            "FreeText" => self.add_markup(&markup(MarkupKind::Typewriter, Vec::new()))?,
            "Square" => self.add_markup(&markup(MarkupKind::Rectangle, Vec::new()))?,
            "Circle" => self.add_markup(&markup(MarkupKind::Ellipse, Vec::new()))?,
            "Line" if it.line.len() == 4 => {
                let kind = if it.arrow {
                    MarkupKind::Arrow
                } else {
                    MarkupKind::Line
                };
                self.add_markup(&markup(kind, points(&it.line)))?;
            }
            "Polygon" if it.vertices.len() >= 6 => {
                self.add_markup(&markup(MarkupKind::Polygon, points(&it.vertices)))?;
            }
            "PolyLine" if it.vertices.len() >= 4 => {
                self.add_markup(&markup(MarkupKind::Pencil, points(&it.vertices)))?;
            }
            "Ink" => {
                for g in &it.ink {
                    if g.len() >= 4 {
                        self.add_markup(&markup(MarkupKind::Pencil, points(g)))?;
                    }
                }
            }
            "Stamp" => {
                let mut m = markup(MarkupKind::Stamp, Vec::new());
                if m.contents.is_empty() {
                    m.contents = it.icon.clone();
                }
                self.add_markup(&m)?;
            }
            "Caret" => self.add_annotation(&note(AnnotKind::Other, vec![rect]))?,
            _ => return Err("not imported".into()),
        }
        // Keep the comment's own name and date so a second import is recognised.
        if self.edit_count() != before && (!it.name.is_empty() || !it.date.is_empty()) {
            if let Some(id) = self.last_annot(info.id) {
                if let Ok(d) = self.doc.get_dictionary_mut(id) {
                    if !it.name.is_empty() {
                        d.set("NM", text_obj(&it.name));
                    }
                    if !it.date.is_empty() {
                        d.set("M", text_obj(&it.date));
                    }
                    if !it.subject.is_empty() {
                        d.set("Subj", text_obj(&it.subject));
                    }
                }
            }
        }
        Ok(())
    }

    fn last_annot(&self, page: ObjectId) -> Option<ObjectId> {
        let pd = self.doc.get_dictionary(page).ok()?;
        let a = resolve(&self.doc, pd.get(b"Annots").ok()?)
            .as_array()
            .ok()?;
        a.last()?.as_reference().ok()
    }

    /// A summary of the comments as a new PDF: a heading, then per page with comments its
    /// number and each comment's number, author, subject, date and text.
    pub fn comment_summary(&self, file: &str, l: &SummaryLabels) -> Result<Vec<u8>, String> {
        let data = crate::fallback::face_data(crate::fallback::SANS).ok_or("no font")?;
        let face = ttf_parser::Face::parse(data, 0).map_err(|e| e.to_string())?;
        let (pw, ph, margin) = (595.0_f32, 842.0_f32, 54.0_f32);
        let width = pw - 2.0 * margin;
        let mut pages: Vec<String> = Vec::new();
        let mut cur = String::new();
        let mut y = ph - margin;
        let line = |cur: &mut String,
                    y: &mut f32,
                    pages: &mut Vec<String>,
                    text: &str,
                    size: f32,
                    grey: f32,
                    indent: f32| {
            for part in wrap(&face, text, size, width - indent) {
                if *y - size * 1.3 < margin {
                    pages.push(std::mem::take(cur));
                    *y = ph - margin;
                }
                *y -= size * 1.3;
                let (p, _) = crate::sigappear::text_path(&face, &part, size, margin + indent, *y);
                if !p.is_empty() {
                    let _ = writeln!(cur, "{grey} g {p}f");
                }
            }
        };
        let title = l.title.replace("{file}", file);
        line(&mut cur, &mut y, &mut pages, &title, 15.0, 0.0, 0.0);
        y -= 8.0;
        let mut by_page: BTreeMap<usize, Vec<Item>> = BTreeMap::new();
        for it in self.items() {
            by_page.entry(it.page).or_default().push(it);
        }
        for (page, items) in by_page {
            y -= 6.0;
            let label = self
                .pages
                .get(page)
                .map_or_else(|| (page + 1).to_string(), |p| p.label.clone());
            line(
                &mut cur,
                &mut y,
                &mut pages,
                &l.page.replace("{n}", &label),
                12.0,
                0.0,
                0.0,
            );
            // Rule under the page heading.
            let _ = writeln!(
                cur,
                "0.6 G 0.5 w {margin} {} m {} {} l S",
                y - 3.0,
                pw - margin,
                y - 3.0
            );
            y -= 4.0;
            for (k, it) in items.iter().enumerate() {
                let head = format!(
                    "{}: {}   {}: {}   {}: {}   {}: {}",
                    l.number,
                    k + 1,
                    l.author,
                    it.author,
                    l.subject,
                    if it.subject.is_empty() {
                        &it.subtype
                    } else {
                        &it.subject
                    },
                    l.date,
                    display_date(&it.date)
                );
                y -= 4.0;
                line(&mut cur, &mut y, &mut pages, &head, 9.0, 0.35, 8.0);
                for para in it.contents.split(['\n', '\r']) {
                    if !para.trim().is_empty() {
                        line(&mut cur, &mut y, &mut pages, para, 10.0, 0.0, 8.0);
                    }
                }
            }
        }
        pages.push(cur);
        let mut out = Document::with_version("1.7");
        let pages_id = out.new_object_id();
        let mut kids = Vec::new();
        for content in pages {
            let c = out.add_object(Stream::new(Dictionary::new(), content.into_bytes()));
            let mut p = Dictionary::new();
            p.set("Type", Object::Name(b"Page".to_vec()));
            p.set("Parent", Object::Reference(pages_id));
            p.set("MediaBox", nums(&[0.0, 0.0, pw, ph]));
            p.set("Contents", Object::Reference(c));
            p.set("Resources", Dictionary::new());
            kids.push(Object::Reference(out.add_object(p)));
        }
        let mut tree = Dictionary::new();
        tree.set("Type", Object::Name(b"Pages".to_vec()));
        tree.set("Count", kids.len() as i64);
        tree.set("Kids", kids);
        out.objects.insert(pages_id, Object::Dictionary(tree));
        let mut cat = Dictionary::new();
        cat.set("Type", Object::Name(b"Catalog".to_vec()));
        cat.set("Pages", Object::Reference(pages_id));
        let root = out.add_object(cat);
        out.trailer.set("Root", Object::Reference(root));
        let mut info = Dictionary::new();
        info.set(
            "Title",
            Object::String(title.into_bytes(), StringFormat::Literal),
        );
        let info = out.add_object(info);
        out.trailer.set("Info", Object::Reference(info));
        out.compress();
        let mut bytes = Vec::new();
        out.save_to(&mut bytes).map_err(|e| e.to_string())?;
        Ok(bytes)
    }
}

/// `D:20261116093005Z` → `2026-11-16 09:30:05`.
fn display_date(d: &str) -> String {
    let s = d.trim_start_matches("D:");
    let g = |a: usize, b: usize| s.get(a..b).unwrap_or("");
    if s.len() < 8 {
        return s.to_owned();
    }
    let mut out = format!("{}-{}-{}", g(0, 4), g(4, 6), g(6, 8));
    if s.len() >= 14 {
        let _ = write!(out, " {}:{}:{}", g(8, 10), g(10, 12), g(12, 14));
    }
    out
}

/// Words of `text` in lines no wider than `width` at `size`.
fn wrap(face: &ttf_parser::Face<'_>, text: &str, size: f32, width: f32) -> Vec<String> {
    let mut lines = Vec::new();
    let mut cur = String::new();
    for w in text.split(' ') {
        let trial = if cur.is_empty() {
            w.to_owned()
        } else {
            format!("{cur} {w}")
        };
        if crate::sigappear::advance(face, &trial, size) > width && !cur.is_empty() {
            lines.push(std::mem::take(&mut cur));
            w.clone_into(&mut cur);
        } else {
            cur = trial;
        }
    }
    lines.push(cur);
    lines
}

/// Copy `o` of `src` into `dst` (FDF export), following references (depth-limited).
fn copy_object(
    src: &Document,
    dst: &mut Document,
    o: &Object,
    map: &mut HashMap<ObjectId, ObjectId>,
    depth: usize,
) -> Object {
    if depth > 64 {
        return Object::Null;
    }
    match o {
        Object::Reference(r) => {
            if let Some(n) = map.get(r) {
                return Object::Reference(*n);
            }
            let Ok(t) = src.get_object(*r) else {
                return Object::Null;
            };
            let t = t.clone();
            let id = dst.new_object_id();
            map.insert(*r, id);
            let c = copy_object(src, dst, &t, map, depth + 1);
            dst.objects.insert(id, c);
            Object::Reference(id)
        }
        Object::Array(v) => Object::Array(
            v.iter()
                .map(|x| copy_object(src, dst, x, map, depth + 1))
                .collect(),
        ),
        Object::Dictionary(d) => {
            let mut nd = Dictionary::new();
            for (k, v) in d {
                if matches!(k.as_slice(), b"P" | b"Parent" | b"Popup" | b"IRT") {
                    continue;
                }
                nd.set(k.clone(), copy_object(src, dst, v, map, depth + 1));
            }
            Object::Dictionary(nd)
        }
        Object::Stream(s) => {
            let mut nd = Dictionary::new();
            for (k, v) in &s.dict {
                nd.set(k.clone(), copy_object(src, dst, v, map, depth + 1));
            }
            Object::Stream(Stream::new(nd, s.content.clone()).with_compression(false))
        }
        other => other.clone(),
    }
}

/// [`PdfDocument::item`] for an annotation of another document (FDF without appearances).
fn item_of(src: &Document, page: usize, d: &Dictionary) -> Item {
    let s = |k: &[u8]| {
        d.get(k)
            .ok()
            .and_then(|o| resolve(src, o).as_str().ok())
            .map(text_string)
            .unwrap_or_default()
    };
    let rect = floats(src, d.get(b"Rect").ok());
    let color = match floats(src, d.get(b"C").ok()).as_slice() {
        [r, g, b] => Some([r, g, b].map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8)),
        _ => None,
    };
    Item {
        subtype: d
            .get(b"Subtype")
            .and_then(Object::as_name)
            .map(|n| String::from_utf8_lossy(n).into_owned())
            .unwrap_or_default(),
        page,
        rect: <[f32; 4]>::try_from(rect.as_slice()).unwrap_or_default(),
        color,
        name: s(b"NM"),
        author: s(b"T"),
        date: s(b"M"),
        subject: s(b"Subj"),
        contents: s(b"Contents"),
        flags: 4,
        coords: floats(src, d.get(b"QuadPoints").ok()),
        line: floats(src, d.get(b"L").ok()),
        vertices: floats(src, d.get(b"Vertices").ok()),
        ink: d
            .get(b"InkList")
            .ok()
            .and_then(|o| resolve(src, o).as_array().ok())
            .map(|a| a.iter().map(|o| floats(src, Some(o))).collect())
            .unwrap_or_default(),
        ..Item::default()
    }
}

/// Attributes of an XML start tag (`name="value"` pairs).
fn attributes(tag: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();
    let mut rest = tag;
    while let Some(eq) = rest.find('=') {
        let key = rest[..eq]
            .trim()
            .rsplit(char::is_whitespace)
            .next()
            .unwrap_or("");
        let after = rest[eq + 1..].trim_start();
        let Some(q) = after.chars().next().filter(|c| *c == '"' || *c == '\'') else {
            break;
        };
        let body = &after[1..];
        let Some(end) = body.find(q) else { break };
        out.insert(key.to_owned(), xml_unescape(&body[..end]));
        rest = &body[end + 1..];
    }
    out
}

/// Text between `<name ...>` and `</name>` inside `body`.
fn child<'a>(body: &'a str, name: &str) -> Option<&'a str> {
    let open = body.find(&format!("<{name}"))?;
    let start = open + body[open..].find('>')? + 1;
    let end = start + body[start..].find(&format!("</{name}>"))?;
    Some(&body[start..end])
}

/// The comments of an XFDF document (a small reader for the elements ISO 19444-1 defines
/// for annotations; rich text, appearances and pop-ups are ignored).
fn parse_xfdf(text: &str) -> Vec<Item> {
    let Some(start) = text.find("<annots") else {
        return Vec::new();
    };
    let body_start = start + text[start..].find('>').map_or(0, |i| i + 1);
    let end = text.find("</annots>").unwrap_or(text.len());
    let mut s = &text[body_start..end.max(body_start)];
    let mut out = Vec::new();
    while let Some(lt) = s.find('<') {
        s = &s[lt + 1..];
        let name_end = s
            .find(|c: char| c.is_whitespace() || c == '>' || c == '/')
            .unwrap_or(s.len());
        let name = &s[..name_end];
        let Some(gt) = s.find('>') else { break };
        let tag = &s[name_end..gt];
        let self_closing = tag.ends_with('/');
        let Some(subtype) = subtype_of(name) else {
            s = &s[gt + 1..];
            continue;
        };
        let a = attributes(tag.trim_end_matches('/'));
        let (inner, next) = if self_closing {
            ("", &s[gt + 1..])
        } else {
            let close = format!("</{name}>");
            match s[gt + 1..].find(&close) {
                Some(e) => (&s[gt + 1..gt + 1 + e], &s[gt + 1 + e + close.len()..]),
                None => (&s[gt + 1..], ""),
            }
        };
        let get = |k: &str| a.get(k).cloned().unwrap_or_default();
        let rect = split_nums(&get("rect"));
        let mut line = split_nums(&get("start"));
        line.extend(split_nums(&get("end")));
        let ink = child(inner, "inklist")
            .map(|l| {
                l.split("<gesture>")
                    .filter_map(|g| g.split("</gesture>").next())
                    .map(split_nums)
                    .filter(|v| !v.is_empty())
                    .collect()
            })
            .unwrap_or_default();
        let arrow = ["head", "tail"]
            .iter()
            .any(|k| a.get(*k).is_some_and(|v| v.ends_with("Arrow")));
        out.push(Item {
            subtype: subtype.to_owned(),
            page: get("page").parse().unwrap_or(0),
            rect: <[f32; 4]>::try_from(rect.as_slice()).unwrap_or_default(),
            color: a.get("color").and_then(|c| parse_hex(c)),
            name: get("name"),
            author: get("title"),
            date: get("date"),
            subject: get("subject"),
            contents: child(inner, "contents")
                .map(xml_unescape)
                .unwrap_or_default(),
            flags: 4,
            opacity: a.get("opacity").and_then(|v| v.parse().ok()),
            width: a.get("width").and_then(|v| v.parse().ok()),
            icon: get("icon"),
            coords: split_nums(&get("coords")),
            line,
            arrow,
            vertices: child(inner, "vertices").map(split_nums).unwrap_or_default(),
            ink,
        });
        s = next;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> PdfDocument {
        let bytes = crate::tests::build_pdf(&[
            "<< /Type /Catalog /Pages 2 0 R >>",
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] >>",
        ]);
        PdfDocument::open(bytes, crate::standard_fonts()).expect("opens")
    }

    fn with_comments() -> PdfDocument {
        let mut d = sample();
        d.add_annotation(&NewAnnot {
            kind: AnnotKind::Highlight,
            page: 0,
            rects: vec![[72.0, 100.0, 200.0, 114.0]],
            color: [255, 230, 0],
            author: "An".into(),
            contents: "Look <here> & there".into(),
        })
        .expect("highlight");
        d.add_annotation(&NewAnnot {
            kind: AnnotKind::Note,
            page: 0,
            rects: vec![[300.0, 300.0, 320.0, 320.0]],
            color: [255, 214, 0],
            author: "Bình".into(),
            contents: "Ghi chú tiếng Việt".into(),
        })
        .expect("note");
        d
    }

    #[test]
    fn xfdf_round_trip() {
        let d = with_comments();
        let x = d.export_xfdf("a.pdf");
        assert!(x.contains("<highlight page=\"0\""), "{x}");
        assert!(x.contains("Look &lt;here&gt; &amp; there"));
        assert!(x.contains("<text page=\"0\""));
        assert!(x.contains("title=\"Bình\""));
        assert!(x.contains("<f href=\"a.pdf\"/>"));
        let mut e = sample();
        assert_eq!(e.import_comments(x.as_bytes()), Ok(2));
        let a = e.annotations();
        assert_eq!(a.len(), 2);
        let h = a
            .iter()
            .find(|a| a.kind == AnnotKind::Highlight)
            .expect("h");
        assert_eq!(h.contents, "Look <here> & there");
        assert!(
            (h.rect[0] - 72.0).abs() < 0.5 && (h.rect[3] - 114.0).abs() < 0.5,
            "{:?}",
            h.rect
        );
        let n = a.iter().find(|a| a.kind == AnnotKind::Note).expect("n");
        assert_eq!(n.author, "Bình");
        assert_eq!(n.contents, "Ghi chú tiếng Việt");
        // A second import adds nothing (same names).
        assert_eq!(e.import_comments(x.as_bytes()), Ok(0));
    }

    #[test]
    fn fdf_round_trip_keeps_appearances() {
        let d = with_comments();
        let fdf = d.export_fdf("a.pdf").expect("fdf");
        assert!(fdf.starts_with(b"%FDF-"));
        let text = String::from_utf8_lossy(&fdf);
        assert!(text.contains("/FDF") && text.contains("/Page 0"), "{text}");
        let mut e = sample();
        assert_eq!(e.import_comments(&fdf), Ok(2));
        let a = e.annotations();
        assert_eq!(a.len(), 2);
        assert!(a.iter().any(|a| a.contents == "Ghi chú tiếng Việt"));
        // Saved and opened again, the comments are there with their appearance streams.
        let saved = e.save().expect("save");
        let f = PdfDocument::open(saved, crate::standard_fonts()).expect("reopen");
        assert_eq!(f.annotations().len(), 2);
        assert_eq!(e.import_comments(&fdf), Ok(0));
    }

    #[test]
    fn xfdf_reader_accepts_other_writers() {
        let x = r##"<?xml version="1.0"?><xfdf xmlns="http://ns.adobe.com/xfdf/"><annots>
<square page="0" rect="100,500,200,600" color="#FF0000" title="X" name="s1"/>
<ink page="0" rect="10,10,90,90" color="#0000FF"><inklist><gesture>10,10;50,80;90,10</gesture></inklist></ink>
<line page="0" rect="0,0,100,100" start="10,10" end="90,90" tail="OpenArrow"/>
<freetext page="0" rect="300,700,400,720"><contents>Typed</contents></freetext>
</annots></xfdf>"##;
        let mut e = sample();
        assert_eq!(e.import_comments(x.as_bytes()), Ok(4));
        let kinds: Vec<AnnotKind> = e.annotations().iter().map(|a| a.kind).collect();
        assert!(kinds.contains(&AnnotKind::Square));
        assert!(kinds.contains(&AnnotKind::Ink));
        assert!(kinds.contains(&AnnotKind::Line));
        assert!(kinds.contains(&AnnotKind::TextBox));
        assert!(e.import_comments(b"hello").is_err());
    }

    #[test]
    fn summary_is_a_pdf_with_the_comments() {
        let d = with_comments();
        let l = SummaryLabels {
            title: "Summary of Comments on {file}".into(),
            page: "Page {n}".into(),
            number: "Number".into(),
            author: "Author".into(),
            subject: "Subject".into(),
            date: "Date".into(),
        };
        let pdf = d.comment_summary("a.pdf", &l).expect("summary");
        let mut s = PdfDocument::open(pdf, crate::standard_fonts()).expect("opens");
        assert_eq!(s.page_count(), 1);
        let page = s
            .render(0, crate::RenderOptions::default())
            .expect("render");
        assert!(page.list.commands.len() > 2);
        assert_eq!(s.title().as_deref(), Some("Summary of Comments on a.pdf"));
        assert_eq!(display_date("D:20261116093005Z"), "2026-11-16 09:30:05");
    }

    #[test]
    fn xml_entities() {
        assert_eq!(xml_unescape("a&amp;b&#233;&#x41;&lt;"), "a&béA<");
        assert_eq!(
            attributes(" a=\"1\" b='x y'").get("b").map(String::as_str),
            Some("x y")
        );
    }
}
