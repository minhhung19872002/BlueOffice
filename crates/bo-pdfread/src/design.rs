//! Page design and form design for BlueEditor:
//!
//! - header and footer (left, centre, right in the margins, with `<<page>>`, `<<pages>>` and
//!   `<<date>>`), watermark (text across the page, rotated, with opacity), background colour
//!   and Bates numbers, written into each page's content as marked artifacts
//!   (ISO 32000-1 §14.8.2.2.2 `/Artifact` with `/Subtype /Header`, `/Footer`, `/Watermark`,
//!   `/Background`); text is drawn as glyph outlines of the bundled sans face;
//! - new form fields (§12.7.4): text, check box, radio button, combo box, list box and push
//!   button, each with a widget and an appearance stream, added to the AcroForm.
use crate::annot::{nums, text_obj};
use crate::interp::{apply, invert};
use crate::PdfDocument;
use lopdf::{Dictionary, Object, Stream};
use std::fmt::Write as _;

fn n(v: f32) -> String {
    crate::objects::num(v)
}

/// Text in the page margins and over the pages.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PageMarks {
    /// Header: left, centre, right (empty: none).
    pub header: [String; 3],
    /// Footer: left, centre, right.
    pub footer: [String; 3],
    /// Distance of header / footer baselines from the page edge (points).
    pub margin: f32,
    /// Header / footer text size.
    pub size: f32,
    /// Watermark text (empty: none).
    pub watermark: String,
    /// Watermark size in points.
    pub watermark_size: f32,
    /// Watermark colour.
    pub watermark_color: [u8; 3],
    /// Watermark opacity 0–1.
    pub watermark_opacity: f32,
    /// Watermark angle in degrees (counter-clockwise).
    pub watermark_angle: f32,
    /// Background colour.
    pub background: Option<[u8; 3]>,
    /// Bates numbers: prefix, first number, digits, suffix (shown at the bottom right).
    pub bates: Option<(String, u32, usize, String)>,
    /// Date written for `<<date>>`.
    pub date: String,
}

fn face() -> Option<ttf_parser::Face<'static>> {
    let data = crate::fallback::face_data(crate::fallback::SANS)?;
    ttf_parser::Face::parse(data, 0).ok()
}

/// Outline operators for `text` with its baseline start at (x, y) in user space.
fn outlined(face: &ttf_parser::Face<'_>, text: &str, size: f32, x: f32, y: f32) -> String {
    let (p, _) = crate::sigappear::text_path(face, text, size, x, y);
    p
}

fn fill(c: [u8; 3]) -> String {
    let f = |v: u8| n(f32::from(v) / 255.0);
    format!("{} {} {} rg ", f(c[0]), f(c[1]), f(c[2]))
}

impl PdfDocument {
    /// Write header, footer, watermark, background and Bates numbers on `pages` (all when
    /// empty) as one undo step.
    #[allow(clippy::too_many_lines)]
    pub fn add_page_marks(&mut self, pages: &[usize], m: &PageMarks) -> Result<(), String> {
        let face = face().ok_or("no bundled font")?;
        let all: Vec<usize> = if pages.is_empty() {
            (0..self.pages.len()).collect()
        } else {
            pages
                .iter()
                .copied()
                .filter(|&p| p < self.pages.len())
                .collect()
        };
        let total = self.pages.len();
        self.begin("page design");
        for (k, &i) in all.iter().enumerate() {
            let info = self.pages[i].clone();
            let inv = invert(crate::page_matrix(info.crop, info.rotate)).ok_or("page matrix")?;
            // Page corners in user space (the page as shown, rotation included).
            let (w, h) = (info.width, info.height);
            let ul = apply(inv, 0.0, 0.0);
            let lr = apply(inv, w, h);
            let (x0, y0, x1, y1) = (
                ul.0.min(lr.0),
                ul.1.min(lr.1),
                ul.0.max(lr.0),
                ul.1.max(lr.1),
            );
            let sub = |s: &str| {
                s.replace("<<page>>", &(i + 1).to_string())
                    .replace("<<pages>>", &total.to_string())
                    .replace("<<date>>", &m.date)
            };
            let mut front = String::new();
            let mut back = String::new();
            if let Some(c) = m.background {
                let _ = writeln!(
                    back,
                    "/Artifact <</Type /Pagination /Subtype /Background>> BDC q {}{} {} {} {} re f Q EMC",
                    fill(c),
                    n(x0),
                    n(y0),
                    n(x1 - x0),
                    n(y1 - y0)
                );
            }
            let size = m.size.max(4.0);
            let margin = m.margin.max(size);
            let side = 36.0_f32.min((x1 - x0) / 6.0);
            let line = |texts: &[String; 3], y: f32, subtype: &str, out: &mut String| {
                let mut s = String::new();
                for (j, t) in texts.iter().enumerate() {
                    let t = sub(t);
                    if t.is_empty() {
                        continue;
                    }
                    let tw = crate::sigappear::advance(&face, &t, size);
                    let x = match j {
                        0 => x0 + side,
                        1 => f32::midpoint(x0, x1) - tw / 2.0,
                        _ => x1 - side - tw,
                    };
                    s.push_str(&outlined(&face, &t, size, x, y));
                }
                if !s.is_empty() {
                    let _ = writeln!(
                        out,
                        "/Artifact <</Type /Pagination /Subtype /{subtype}>> BDC q 0 g {s}f Q EMC"
                    );
                }
            };
            line(&m.header, y1 - margin, "Header", &mut front);
            let mut footer = m.footer.clone();
            if let Some((prefix, first, digits, suffix)) = &m.bates {
                let num = *first as usize + k;
                footer[2] = format!("{prefix}{num:0digits$}{suffix}", digits = *digits);
            }
            line(&footer, y0 + margin - size, "Footer", &mut front);
            if !m.watermark.is_empty() {
                let t = sub(&m.watermark);
                let size = m.watermark_size.max(6.0);
                let tw = crate::sigappear::advance(&face, &t, size);
                let (cx, cy) = (f32::midpoint(x0, x1), f32::midpoint(y0, y1));
                let a = m.watermark_angle.to_radians();
                let (c, s) = (a.cos(), a.sin());
                let path = outlined(&face, &t, size, -tw / 2.0, -size * 0.35);
                let op = m.watermark_opacity.clamp(0.05, 1.0);
                let gs = String::from_utf8_lossy(&self.watermark_gs(i, op)?).into_owned();
                let _ = writeln!(
                    front,
                    "/Artifact <</Type /Pagination /Subtype /Watermark>> BDC q /{gs} gs {}{} {} {} {} {} {} cm {path}f Q EMC",
                    fill(m.watermark_color),
                    n(c),
                    n(s),
                    n(-s),
                    n(c),
                    n(cx),
                    n(cy)
                );
            }
            if front.is_empty() && back.is_empty() {
                continue;
            }
            let (sid, content) = self.page_stream(i)?;
            let mut out = back.into_bytes();
            out.extend_from_slice(&content);
            out.extend_from_slice(front.as_bytes());
            self.set_stream(sid, out);
        }
        self.commit();
        Ok(())
    }

    /// An ExtGState with constant opacity `op` in page `i`'s resources; its name.
    fn watermark_gs(&mut self, i: usize, op: f32) -> Result<Vec<u8>, String> {
        let mut gs = Dictionary::new();
        gs.set("Type", Object::Name(b"ExtGState".to_vec()));
        gs.set("ca", Object::Real(op));
        gs.set("CA", Object::Real(op));
        let id = self.add(Object::Dictionary(gs));
        let name = self.add_resource(i, b"ExtGState", "BoGS", id)?;
        Ok(name)
    }
}

/// Kind of a new form field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NewFieldKind {
    /// Text field.
    Text,
    /// Check box.
    Check,
    /// Radio button.
    Radio,
    /// Combo box (drop-down list).
    Combo,
    /// List box.
    List,
    /// Push button.
    Button,
}

/// A form field to create (rectangle in page points, origin top-left).
#[derive(Debug, Clone, PartialEq)]
pub struct NewField {
    /// What it is.
    pub kind: NewFieldKind,
    /// Page index.
    pub page: usize,
    /// Its box: left, top, right, bottom.
    pub rect: [f32; 4],
    /// Field name (`/T`).
    pub name: String,
    /// Tooltip (`/TU`).
    pub tooltip: String,
    /// Choices of combo and list boxes; the caption of a push button (first entry).
    pub options: Vec<String>,
    /// Required (`/Ff` bit 2).
    pub required: bool,
    /// Read-only (`/Ff` bit 1).
    pub read_only: bool,
}

impl PdfDocument {
    /// A field name not used yet: `prefix` followed by the first free number.
    pub fn free_field_name(&self, prefix: &str) -> String {
        let names: Vec<String> = self.form_fields().into_iter().map(|f| f.name).collect();
        (1..=names.len() + 1)
            .map(|k| format!("{prefix}{k}"))
            .find(|n| !names.contains(n))
            .unwrap_or_else(|| prefix.to_owned())
    }

    /// Create a form field with its widget on the page (one undo step).
    #[allow(clippy::too_many_lines)]
    pub fn add_field(&mut self, f: &NewField) -> Result<(), String> {
        let info = self.pages.get(f.page).ok_or("no such page")?.clone();
        let inv = invert(crate::page_matrix(info.crop, info.rotate)).ok_or("page matrix")?;
        let a = apply(inv, f.rect[0], f.rect[1]);
        let b = apply(inv, f.rect[2], f.rect[3]);
        let rect = [a.0.min(b.0), a.1.min(b.1), a.0.max(b.0), a.1.max(b.1)];
        let (w, h) = (rect[2] - rect[0], rect[3] - rect[1]);
        if w < 2.0 || h < 2.0 {
            return Err("the field is too small".into());
        }
        self.begin("add field");
        let form = |s: String, me: &mut Self| -> lopdf::ObjectId {
            let mut d = Dictionary::new();
            d.set("Type", Object::Name(b"XObject".to_vec()));
            d.set("Subtype", Object::Name(b"Form".to_vec()));
            d.set("BBox", nums(&[0.0, 0.0, w, h]));
            d.set("Resources", Dictionary::new());
            me.add(Object::Stream(Stream::new(d, s.into_bytes())))
        };
        let frame = format!(
            "0.5 0.5 0.5 RG 1 w 0.5 0.5 {} {} re S ",
            n(w - 1.0),
            n(h - 1.0)
        );
        let mut d = Dictionary::new();
        d.set("Type", Object::Name(b"Annot".to_vec()));
        d.set("Subtype", Object::Name(b"Widget".to_vec()));
        d.set("Rect", nums(&rect));
        d.set("F", 4);
        d.set("P", Object::Reference(info.id));
        d.set("T", text_obj(&f.name));
        if !f.tooltip.is_empty() {
            d.set("TU", text_obj(&f.tooltip));
        }
        let mut mk = Dictionary::new();
        mk.set("BC", nums(&[0.5, 0.5, 0.5]));
        let mut flags: i64 = i64::from(f.read_only) | (i64::from(f.required) << 1);
        let mut ap = Dictionary::new();
        match f.kind {
            NewFieldKind::Text => {
                d.set("FT", Object::Name(b"Tx".to_vec()));
                d.set("DA", text_obj("/Helv 0 Tf 0 g"));
                ap.set("N", Object::Reference(form(frame.clone(), self)));
            }
            NewFieldKind::Check | NewFieldKind::Radio => {
                d.set("FT", Object::Name(b"Btn".to_vec()));
                let on_name: &[u8] = if f.kind == NewFieldKind::Radio {
                    b"Choice1"
                } else {
                    b"Yes"
                };
                let s = w.min(h);
                let mark = if f.kind == NewFieldKind::Radio {
                    let r = s * 0.25;
                    let (cx, cy) = (w / 2.0, h / 2.0);
                    let k = 0.5523 * r;
                    format!(
                        "0 g {} {} m {} {} {} {} {} {} c {} {} {} {} {} {} c {} {} {} {} {} {} c {} {} {} {} {} {} c f ",
                        n(cx + r), n(cy),
                        n(cx + r), n(cy + k), n(cx + k), n(cy + r), n(cx), n(cy + r),
                        n(cx - k), n(cy + r), n(cx - r), n(cy + k), n(cx - r), n(cy),
                        n(cx - r), n(cy - k), n(cx - k), n(cy - r), n(cx), n(cy - r),
                        n(cx + k), n(cy - r), n(cx + r), n(cy - k), n(cx + r), n(cy),
                    )
                } else {
                    format!(
                        "0 G {} w 1 J 1 j {} {} m {} {} l {} {} l S ",
                        n((s * 0.1).max(1.0)),
                        n(w * 0.22),
                        n(h * 0.5),
                        n(w * 0.42),
                        n(h * 0.28),
                        n(w * 0.78),
                        n(h * 0.75)
                    )
                };
                let on = form(format!("{frame}{mark}"), self);
                let off = form(frame.clone(), self);
                let mut states = Dictionary::new();
                states.set(on_name.to_vec(), Object::Reference(on));
                states.set("Off", Object::Reference(off));
                ap.set("N", states);
                d.set("AS", Object::Name(b"Off".to_vec()));
                d.set("V", Object::Name(b"Off".to_vec()));
                if f.kind == NewFieldKind::Radio {
                    // Radio and NoToggleToOff (§12.7.4.2.4, Table 226).
                    flags |= (1 << 15) | (1 << 14);
                    mk.set(
                        "CA",
                        Object::String(b"l".to_vec(), lopdf::StringFormat::Literal),
                    );
                }
            }
            NewFieldKind::Combo | NewFieldKind::List => {
                d.set("FT", Object::Name(b"Ch".to_vec()));
                if f.kind == NewFieldKind::Combo {
                    flags |= 1 << 17;
                }
                d.set(
                    "Opt",
                    f.options.iter().map(|o| text_obj(o)).collect::<Vec<_>>(),
                );
                d.set("DA", text_obj("/Helv 0 Tf 0 g"));
                let mut s = frame.clone();
                if f.kind == NewFieldKind::Combo {
                    // A drop-down arrow at the right.
                    let a = (h * 0.3).min(8.0);
                    let _ = write!(
                        s,
                        "0.3 g {} {} m {} {} l {} {} l f ",
                        n(w - a * 2.2),
                        n(h / 2.0 + a / 2.0),
                        n(w - a * 0.2),
                        n(h / 2.0 + a / 2.0),
                        n(w - a * 1.2),
                        n(h / 2.0 - a / 2.0)
                    );
                }
                ap.set("N", Object::Reference(form(s, self)));
            }
            NewFieldKind::Button => {
                d.set("FT", Object::Name(b"Btn".to_vec()));
                flags |= 1 << 16;
                let caption = f.options.first().cloned().unwrap_or_default();
                mk.set("CA", text_obj(&caption));
                mk.set("BG", nums(&[0.85, 0.85, 0.85]));
                let mut s = format!("0.85 g 0 0 {} {} re f {frame}", n(w), n(h));
                if let Some(face) = face() {
                    let size = (h * 0.5).min(12.0);
                    let tw = crate::sigappear::advance(&face, &caption, size);
                    let _ = write!(
                        s,
                        "0 g {}f ",
                        outlined(
                            &face,
                            &caption,
                            size,
                            (w - tw) / 2.0,
                            (h - size * 0.7) / 2.0
                        )
                    );
                }
                ap.set("N", Object::Reference(form(s, self)));
            }
        }
        d.set("MK", mk);
        d.set("AP", ap);
        if flags != 0 {
            d.set("Ff", flags);
        }
        let field = self.add(Object::Dictionary(d));
        // The page's /Annots and the AcroForm's /Fields get it (§12.7.2).
        let page = self
            .doc
            .get_dictionary(info.id)
            .map_err(|e| e.to_string())?;
        if let Ok(Object::Reference(arr)) = page.get(b"Annots") {
            let arr = *arr;
            self.touch(arr);
            if let Ok(Object::Array(v)) = self.doc.get_object_mut(arr) {
                v.push(Object::Reference(field));
            }
        } else {
            self.touch(info.id);
            let page = self
                .doc
                .get_dictionary_mut(info.id)
                .map_err(|e| e.to_string())?;
            match page.get_mut(b"Annots") {
                Ok(Object::Array(v)) => v.push(Object::Reference(field)),
                _ => page.set("Annots", vec![Object::Reference(field)]),
            }
        }
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
        let helv = || {
            let mut font = Dictionary::new();
            font.set("Type", Object::Name(b"Font".to_vec()));
            font.set("Subtype", Object::Name(b"Type1".to_vec()));
            font.set("BaseFont", Object::Name(b"Helvetica".to_vec()));
            font.set("Encoding", Object::Name(b"WinAnsiEncoding".to_vec()));
            let mut fonts = Dictionary::new();
            fonts.set("Helv", font);
            let mut dr = Dictionary::new();
            dr.set("Font", fonts);
            dr
        };
        match acro {
            Some(Object::Reference(id)) => {
                self.touch(id);
                if let Ok(d) = self.doc.get_dictionary_mut(id) {
                    match d.get_mut(b"Fields") {
                        Ok(Object::Array(v)) => v.push(Object::Reference(field)),
                        _ => d.set("Fields", vec![Object::Reference(field)]),
                    }
                    if !d.has(b"DR") {
                        d.set("DR", helv());
                    }
                }
            }
            other => {
                let mut d = match other {
                    Some(Object::Dictionary(d)) => d,
                    _ => Dictionary::new(),
                };
                match d.get_mut(b"Fields") {
                    Ok(Object::Array(v)) => v.push(Object::Reference(field)),
                    _ => d.set("Fields", vec![Object::Reference(field)]),
                }
                if !d.has(b"DR") {
                    d.set("DR", helv());
                }
                if !d.has(b"DA") {
                    d.set("DA", text_obj("/Helv 0 Tf 0 g"));
                }
                let id = self.add(Object::Dictionary(d));
                self.touch(root);
                if let Ok(c) = self.doc.get_dictionary_mut(root) {
                    c.set("AcroForm", Object::Reference(id));
                }
            }
        }
        self.commit();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{standard_fonts, FieldKind, RenderOptions, TextLayer};

    fn doc(pages: usize) -> PdfDocument {
        let kids: Vec<String> = (0..pages).map(|i| format!("{} 0 R", i + 3)).collect();
        let mut objs = vec![
            "<< /Type /Catalog /Pages 2 0 R >>".to_owned(),
            format!(
                "<< /Type /Pages /Kids [{}] /Count {pages} >>",
                kids.join(" ")
            ),
        ];
        for _ in 0..pages {
            objs.push(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 400] /Resources << >> >>".into(),
            );
        }
        let refs: Vec<&str> = objs.iter().map(String::as_str).collect();
        PdfDocument::open(crate::tests::build_pdf(&refs), standard_fonts()).expect("opens")
    }

    #[test]
    fn headers_footers_watermark_background_bates() {
        let mut d = doc(3);
        let m = PageMarks {
            header: [String::new(), "Báo cáo".into(), String::new()],
            footer: ["<<date>>".into(), String::new(), String::new()],
            margin: 24.0,
            size: 10.0,
            watermark: "MẬT".into(),
            watermark_size: 60.0,
            watermark_color: [200, 0, 0],
            watermark_opacity: 0.3,
            watermark_angle: 45.0,
            background: Some([255, 250, 230]),
            bates: Some(("BO".into(), 7, 4, String::new())),
            date: "2026-11-15".into(),
        };
        d.add_page_marks(&[], &m).expect("marks");
        let saved = d.save().expect("saves");
        let mut again = PdfDocument::open(saved, standard_fonts()).expect("opens");
        for i in 0..3 {
            let p = again.render(i, RenderOptions::default()).expect("renders");
            // Background fill, header, footer, Bates and watermark are drawn (no text layer:
            // outlines).
            assert!(p.list.commands.len() >= 4, "page {i}");
            assert_eq!(TextLayer::new(p.chars).text(), "");
        }
        assert!(d.undo());
    }

    #[test]
    fn new_fields_are_form_fields() {
        let mut d = doc(1);
        let kinds = [
            NewFieldKind::Text,
            NewFieldKind::Check,
            NewFieldKind::Radio,
            NewFieldKind::Combo,
            NewFieldKind::List,
            NewFieldKind::Button,
        ];
        for (i, k) in kinds.iter().enumerate() {
            let y = 20.0 + 40.0 * i as f32;
            let name = d.free_field_name("Field");
            d.add_field(&NewField {
                kind: *k,
                page: 0,
                rect: [20.0, y, 160.0, y + 24.0],
                name,
                tooltip: "Gợi ý".into(),
                options: vec!["Một".into(), "Hai".into()],
                required: i == 0,
                read_only: false,
            })
            .expect("adds");
        }
        let saved = d.save().expect("saves");
        let again = PdfDocument::open(saved, standard_fonts()).expect("opens");
        let fields = again.form_fields();
        assert_eq!(fields.len(), 6, "{fields:?}");
        assert_eq!(fields[0].name, "Field1");
        assert_eq!(fields[5].name, "Field6");
        assert_eq!(fields[0].kind, FieldKind::Text);
        assert_eq!(fields[1].kind, FieldKind::CheckBox);
        assert_eq!(fields[2].kind, FieldKind::Radio);
        assert!(matches!(fields[3].kind, FieldKind::Combo | FieldKind::List));
        assert_eq!(fields[5].kind, FieldKind::Button);
    }
}
