//! Markup annotations (ISO 32000-1, §12.5.6): listing a document's comments, drawing simple
//! stand-ins for markup without an appearance stream, and adding highlight / underline /
//! strikeout / note / text box / rectangle comments with their own appearance streams.
use crate::interp::{apply, invert, M};
use crate::store::Document;
use crate::{resolve, text_string, PdfDocument};
use lopdf::{Dictionary, Object, Stream, StringFormat};

/// Kind of a comment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnnotKind {
    /// Text highlight.
    Highlight,
    /// Underlined text.
    Underline,
    /// Struck-out text.
    StrikeOut,
    /// Squiggly underline.
    Squiggly,
    /// Sticky note.
    Note,
    /// Text box (free text).
    TextBox,
    /// Rectangle.
    Square,
    /// Ellipse.
    Circle,
    /// Freehand drawing.
    Ink,
    /// Line or arrow.
    Line,
    /// Stamp.
    Stamp,
    /// Any other markup.
    Other,
}

impl AnnotKind {
    fn from_subtype(s: &[u8]) -> Option<Self> {
        Some(match s {
            b"Highlight" => Self::Highlight,
            b"Underline" => Self::Underline,
            b"StrikeOut" => Self::StrikeOut,
            b"Squiggly" => Self::Squiggly,
            b"Text" => Self::Note,
            b"FreeText" => Self::TextBox,
            b"Square" => Self::Square,
            b"Circle" => Self::Circle,
            b"Ink" => Self::Ink,
            b"Line" | b"PolyLine" | b"Polygon" => Self::Line,
            b"Stamp" => Self::Stamp,
            b"Caret" | b"FileAttachment" | b"Sound" | b"Screen" | b"3D" | b"Movie" => Self::Other,
            _ => return None,
        })
    }

    pub(crate) fn subtype(self) -> &'static str {
        match self {
            Self::Highlight => "Highlight",
            Self::Underline => "Underline",
            Self::StrikeOut => "StrikeOut",
            Self::Squiggly => "Squiggly",
            Self::Note => "Text",
            Self::TextBox => "FreeText",
            Self::Square => "Square",
            Self::Circle => "Circle",
            Self::Ink => "Ink",
            Self::Line => "Line",
            Self::Stamp => "Stamp",
            Self::Other => "Caret",
        }
    }
}

/// A comment on a page.
#[derive(Debug, Clone, PartialEq)]
pub struct Annot {
    /// Kind.
    pub kind: AnnotKind,
    /// Page index.
    pub page: usize,
    /// Left, top, right, bottom in page points.
    pub rect: [f32; 4],
    /// Comment text.
    pub contents: String,
    /// Author (`/T`).
    pub author: String,
    /// Modification date as written (`D:YYYYMMDDHHmmSS…`).
    pub modified: String,
    /// Colour.
    pub color: [u8; 3],
    /// The annotation object (to edit or delete it).
    pub(crate) id: Option<lopdf::ObjectId>,
}

fn color_of(doc: &Document, d: &Dictionary) -> [u8; 3] {
    let v: Vec<f32> = d
        .get(b"C")
        .ok()
        .map(|o| resolve(doc, o))
        .and_then(|o| o.as_array().ok())
        .map(|a| {
            a.iter()
                .filter_map(|o| resolve(doc, o).as_float().ok())
                .collect()
        })
        .unwrap_or_default();
    let b = |x: f32| (x.clamp(0.0, 1.0) * 255.0).round() as u8;
    match v.as_slice() {
        [g] => [b(*g); 3],
        [r, g, bl] => [b(*r), b(*g), b(*bl)],
        [c, m, y, k] => {
            let rgb = crate::color::cmyk_to_rgb(*c, *m, *y, *k);
            [b(rgb[0]), b(rgb[1]), b(rgb[2])]
        }
        _ => [255, 220, 0],
    }
}

fn quads(doc: &Document, d: &Dictionary) -> Vec<[f32; 8]> {
    d.get(b"QuadPoints")
        .ok()
        .map(|o| resolve(doc, o))
        .and_then(|o| o.as_array().ok())
        .map(|a| {
            let v: Vec<f32> = a
                .iter()
                .filter_map(|o| resolve(doc, o).as_float().ok())
                .collect();
            v.chunks_exact(8)
                .filter_map(|c| <[f32; 8]>::try_from(c).ok())
                .collect()
        })
        .unwrap_or_default()
}

/// Content (in user space, no resources) and fill opacity standing in for markup that has
/// no appearance stream.
pub(crate) fn fallback_appearance(
    doc: &Document,
    d: &Dictionary,
    rect: [f32; 4],
) -> Option<(Vec<u8>, f32)> {
    let kind = AnnotKind::from_subtype(d.get(b"Subtype").and_then(Object::as_name).ok()?)?;
    let [r, g, b] = color_of(doc, d).map(|c| f32::from(c) / 255.0);
    let mut s = String::new();
    let qs = quads(doc, d);
    let qs = if qs.is_empty() {
        vec![[
            rect[0], rect[3], rect[2], rect[3], rect[0], rect[1], rect[2], rect[1],
        ]]
    } else {
        qs
    };
    match kind {
        AnnotKind::Highlight => {
            s.push_str(&format!("{r} {g} {b} rg\n"));
            for q in &qs {
                s.push_str(&format!(
                    "{} {} m {} {} l {} {} l {} {} l h f\n",
                    q[0], q[1], q[2], q[3], q[6], q[7], q[4], q[5]
                ));
            }
            return Some((s.into_bytes(), 0.35));
        }
        AnnotKind::Underline | AnnotKind::StrikeOut | AnnotKind::Squiggly => {
            s.push_str(&format!("{r} {g} {b} RG\n"));
            for q in &qs {
                let h = ((q[1] - q[5]).powi(2) + (q[0] - q[4]).powi(2)).sqrt();
                let t = if kind == AnnotKind::StrikeOut {
                    0.45
                } else {
                    0.08
                };
                let (x0, y0) = (q[4] + (q[0] - q[4]) * t, q[5] + (q[1] - q[5]) * t);
                let (x1, y1) = (q[6] + (q[2] - q[6]) * t, q[7] + (q[3] - q[7]) * t);
                if kind == AnnotKind::Squiggly {
                    let up = (q[0] - q[4], q[1] - q[5]);
                    s.push_str(&squiggle((x0, y0), (x1, y1), up, (h / 16.0).max(0.5)));
                    continue;
                }
                s.push_str(&format!(
                    "{} w {x0} {y0} m {x1} {y1} l S\n",
                    (h / 16.0).max(0.5)
                ));
            }
        }
        AnnotKind::Square | AnnotKind::Circle | AnnotKind::Ink | AnnotKind::Line => {
            s.push_str(&format!(
                "{r} {g} {b} RG 1 w {} {} {} {} re S\n",
                rect[0] + 0.5,
                rect[1] + 0.5,
                rect[2] - rect[0] - 1.0,
                rect[3] - rect[1] - 1.0
            ));
        }
        AnnotKind::Note => {
            // §12.5.6.4 /Name: the other icons are drawn as well (round 16).
            let name = d
                .get(b"Name")
                .and_then(Object::as_name)
                .map(|n| String::from_utf8_lossy(n).into_owned())
                .unwrap_or_default();
            if name != "Comment" && crate::note16::NOTE_ICONS_DRAWN.contains(&name.as_str()) {
                s.push_str(&crate::note16::note_icon_content(
                    &name,
                    rect,
                    [r, g, b],
                    1.0,
                ));
            } else {
                s.push_str(&note_icon(rect, [r, g, b]));
            }
        }
        _ => return None,
    }
    Some((s.into_bytes(), 1.0))
}

/// A squiggly underline (§12.5.6.10 Squiggly) from `a` to `b`: a zigzag of line width `w`
/// either side of the line, `up` pointing to the top of the text line (its height sets the
/// wave). Other readers show the appearance stream as it is, so the wave is drawn in it
/// (before: a straight line, which the reference then showed as an underline).
fn squiggle(a: (f32, f32), b: (f32, f32), up: (f32, f32), w: f32) -> String {
    let len = (b.0 - a.0).hypot(b.1 - a.1);
    let h = up.0.hypot(up.1).max(1e-3);
    let (ux, uy) = (up.0 / h, up.1 / h);
    let amp = (h * 0.06).max(w);
    let step = (h * 0.12).max(1.0);
    #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
    let n = ((len / step).ceil() as usize).clamp(1, 4000);
    let mut s = format!("{w} w {} {} m ", a.0 - ux * amp, a.1 - uy * amp);
    for i in 1..=n {
        #[allow(clippy::cast_precision_loss)]
        let t = i as f32 / n as f32;
        let k = if i % 2 == 0 { -amp } else { amp };
        let x = a.0 + (b.0 - a.0) * t + ux * k;
        let y = a.1 + (b.1 - a.1) * t + uy * k;
        s.push_str(&format!("{x} {y} l "));
    }
    s.push_str("S\n");
    s
}

/// A sticky-note icon of our own drawing in `rect` (user space): a speech box with a tail at
/// the bottom left and three text lines.
fn note_icon(rect: [f32; 4], [r, g, b]: [f32; 3]) -> String {
    let (x, y, w, h) = (rect[0], rect[1], rect[2] - rect[0], rect[3] - rect[1]);
    let pts = [
        (x + 1.0, y + h - 1.0),
        (x + 1.0, y + h * 0.25),
        (x + w * 0.25, y + h * 0.25),
        (x + w * 0.2, y + 1.0),
        (x + w * 0.45, y + h * 0.25),
        (x + w - 1.0, y + h * 0.25),
        (x + w - 1.0, y + h - 1.0),
    ];
    let mut s = format!(
        "{r} {g} {b} rg 0.25 0.25 0.25 RG 0.8 w {} {} m ",
        pts[0].0, pts[0].1
    );
    for p in &pts[1..] {
        s.push_str(&format!("{} {} l ", p.0, p.1));
    }
    s.push_str(
        "h B
",
    );
    for k in 0..3 {
        let ly = y + h * (0.45 + 0.15 * k as f32);
        s.push_str(&format!("{} {ly} m {} {ly} l ", x + w * 0.2, x + w * 0.8));
    }
    s.push_str(
        "S
",
    );
    s
}

/// PDF date string of now (UTC).
pub(crate) fn pdf_date() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let days = secs / 86_400;
    let rem = secs % 86_400;
    // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!(
        "D:{y:04}{m:02}{d:02}{:02}{:02}{:02}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

pub(crate) fn text_obj(s: &str) -> Object {
    if s.is_ascii() {
        Object::String(s.as_bytes().to_vec(), StringFormat::Literal)
    } else {
        let mut b = vec![0xfe, 0xff];
        b.extend(s.encode_utf16().flat_map(u16::to_be_bytes));
        Object::String(b, StringFormat::Hexadecimal)
    }
}

pub(crate) fn nums(v: &[f32]) -> Object {
    Object::Array(
        v.iter()
            .map(|x| Object::Real((x * 1000.0).round() / 1000.0))
            .collect(),
    )
}

/// A new comment to add.
#[derive(Debug, Clone, PartialEq)]
pub struct NewAnnot {
    /// Kind (highlight, underline, strikeout, note, text box, square).
    pub kind: AnnotKind,
    /// Page index.
    pub page: usize,
    /// Areas in page points (left, top, right, bottom): one per text line for text markup,
    /// the box for the others.
    pub rects: Vec<[f32; 4]>,
    /// Colour.
    pub color: [u8; 3],
    /// Author.
    pub author: String,
    /// Comment (or the text of a text box).
    pub contents: String,
}

impl PdfDocument {
    /// The comments of every page.
    pub fn annotations(&self) -> Vec<Annot> {
        let doc = &self.doc;
        let mut out = Vec::new();
        for (index, info) in self.pages.iter().enumerate() {
            let Ok(page) = doc.get_dictionary(info.id) else {
                continue;
            };
            let Some(list) = page
                .get(b"Annots")
                .ok()
                .and_then(|o| resolve(doc, o).as_array().ok())
            else {
                continue;
            };
            let base = crate::page_matrix(info.crop, info.rotate);
            for a in list {
                let Ok(d) = resolve(doc, a).as_dict() else {
                    continue;
                };
                let Some(kind) = d
                    .get(b"Subtype")
                    .and_then(Object::as_name)
                    .ok()
                    .and_then(AnnotKind::from_subtype)
                else {
                    continue;
                };
                let Some(rect) = crate::annot_rect(doc, d) else {
                    continue;
                };
                let s = |k: &[u8]| {
                    d.get(k)
                        .ok()
                        .and_then(|o| resolve(doc, o).as_str().ok())
                        .map(text_string)
                        .unwrap_or_default()
                };
                out.push(Annot {
                    kind,
                    page: index,
                    rect: crate::device_rect(base, rect),
                    contents: s(b"Contents"),
                    author: s(b"T"),
                    // The creation date when there is no modification date (round 8).
                    modified: Some(s(b"M"))
                        .filter(|m| !m.is_empty())
                        .unwrap_or_else(|| s(b"CreationDate")),
                    color: color_of(doc, d),
                    id: a.as_reference().ok(),
                });
            }
        }
        out
    }

    /// Add a comment; it shows at once and is written by [`PdfDocument::save`].
    pub fn add_annotation(&mut self, a: &NewAnnot) -> Result<(), String> {
        if self.doc.is_encrypted() && self.doc.encryption_state.is_none() {
            return Err("comments cannot be added to an encrypted document".into());
        }
        let info = self.pages.get(a.page).ok_or("no such page")?.clone();
        let base: M = crate::page_matrix(info.crop, info.rotate);
        let inv = invert(base).ok_or("page matrix")?;
        // Device rectangles to user-space quads (x1 y1 x2 y2 x3 y3 x4 y4: upper left, upper
        // right, lower left, lower right as viewers write them).
        let mut quad_list = Vec::new();
        let mut lo = (f32::INFINITY, f32::INFINITY);
        let mut hi = (f32::NEG_INFINITY, f32::NEG_INFINITY);
        for r in &a.rects {
            let ul = apply(inv, r[0], r[1]);
            let ur = apply(inv, r[2], r[1]);
            let ll = apply(inv, r[0], r[3]);
            let lr = apply(inv, r[2], r[3]);
            for p in [ul, ur, ll, lr] {
                lo = (lo.0.min(p.0), lo.1.min(p.1));
                hi = (hi.0.max(p.0), hi.1.max(p.1));
            }
            quad_list.extend_from_slice(&[ul.0, ul.1, ur.0, ur.1, ll.0, ll.1, lr.0, lr.1]);
        }
        if !lo.0.is_finite() {
            return Err("no area".into());
        }
        let rect = [lo.0, lo.1, hi.0, hi.1];
        let [r, g, b] = a.color.map(|c| f32::from(c) / 255.0);
        let mut res = Dictionary::new();
        let content = match a.kind {
            AnnotKind::Highlight => {
                let mut gs = Dictionary::new();
                gs.set("Type", Object::Name(b"ExtGState".to_vec()));
                gs.set("BM", Object::Name(b"Multiply".to_vec()));
                gs.set("ca", Object::Real(0.4));
                let mut ext = Dictionary::new();
                ext.set("GS0", gs);
                res.set("ExtGState", ext);
                let mut s = format!("/GS0 gs {r} {g} {b} rg\n");
                for q in quad_list.chunks_exact(8) {
                    s.push_str(&format!(
                        "{} {} m {} {} l {} {} l {} {} l h f\n",
                        q[0], q[1], q[2], q[3], q[6], q[7], q[4], q[5]
                    ));
                }
                s
            }
            AnnotKind::Underline | AnnotKind::StrikeOut | AnnotKind::Squiggly => {
                let mut s = format!("{r} {g} {b} RG\n");
                for q in quad_list.chunks_exact(8) {
                    let h = ((q[1] - q[5]).powi(2) + (q[0] - q[4]).powi(2)).sqrt();
                    let t = if a.kind == AnnotKind::StrikeOut {
                        0.45
                    } else {
                        0.1
                    };
                    let (x0, y0) = (q[4] + (q[0] - q[4]) * t, q[5] + (q[1] - q[5]) * t);
                    let (x1, y1) = (q[6] + (q[2] - q[6]) * t, q[7] + (q[3] - q[7]) * t);
                    if a.kind == AnnotKind::Squiggly {
                        let up = (q[0] - q[4], q[1] - q[5]);
                        s.push_str(&squiggle((x0, y0), (x1, y1), up, (h / 14.0).max(0.6)));
                        continue;
                    }
                    s.push_str(&format!(
                        "{} w {x0} {y0} m {x1} {y1} l S\n",
                        (h / 14.0).max(0.6)
                    ));
                }
                s
            }
            AnnotKind::Note => note_icon(rect, [r, g, b]),
            AnnotKind::TextBox => {
                let mut font = Dictionary::new();
                font.set("Type", Object::Name(b"Font".to_vec()));
                font.set("Subtype", Object::Name(b"Type1".to_vec()));
                font.set("BaseFont", Object::Name(b"Helvetica".to_vec()));
                font.set("Encoding", Object::Name(b"WinAnsiEncoding".to_vec()));
                let mut fonts = Dictionary::new();
                fonts.set("Helv", font);
                res.set("Font", fonts);
                let mut s = format!(
                    "{r} {g} {b} RG 1 w {} {} {} {} re S\nBT /Helv 11 Tf 0 g 13 TL {} {} Td\n",
                    rect[0] + 0.5,
                    rect[1] + 0.5,
                    rect[2] - rect[0] - 1.0,
                    rect[3] - rect[1] - 1.0,
                    rect[0] + 4.0,
                    rect[3] - 14.0
                );
                for line in a.contents.lines() {
                    let enc: String = line
                        .chars()
                        .map(|c| match c {
                            '(' | ')' | '\\' => format!("\\{c}"),
                            c if (c as u32) < 256 => c.to_string(),
                            _ => "?".to_owned(),
                        })
                        .collect();
                    s.push_str(&format!("({enc}) Tj T*\n"));
                }
                s.push_str("ET\n");
                s
            }
            _ => format!(
                "{r} {g} {b} RG 1.5 w {} {} {} {} re S\n",
                rect[0] + 0.75,
                rect[1] + 0.75,
                rect[2] - rect[0] - 1.5,
                rect[3] - rect[1] - 1.5
            ),
        };
        let mut ap_dict = Dictionary::new();
        ap_dict.set("Type", Object::Name(b"XObject".to_vec()));
        ap_dict.set("Subtype", Object::Name(b"Form".to_vec()));
        ap_dict.set("BBox", nums(&rect));
        ap_dict.set("Resources", res);
        self.begin("comment");
        let ap_id = self.add(Object::Stream(Stream::new(ap_dict, content.into_bytes())));
        let mut ap = Dictionary::new();
        ap.set("N", Object::Reference(ap_id));
        let mut d = Dictionary::new();
        let now = pdf_date();
        d.set("Type", Object::Name(b"Annot".to_vec()));
        d.set(
            "Subtype",
            Object::Name(a.kind.subtype().as_bytes().to_vec()),
        );
        d.set("Rect", nums(&rect));
        d.set("C", nums(&[r, g, b]));
        d.set("F", 4);
        d.set("P", Object::Reference(info.id));
        d.set("T", text_obj(&a.author));
        d.set("Contents", text_obj(&a.contents));
        d.set("M", text_obj(&now));
        d.set("CreationDate", text_obj(&now));
        d.set("NM", text_obj(&format!("bo-{}-{}", ap_id.0, now)));
        d.set("AP", ap);
        match a.kind {
            AnnotKind::Highlight
            | AnnotKind::Underline
            | AnnotKind::StrikeOut
            | AnnotKind::Squiggly => {
                d.set("QuadPoints", nums(&quad_list));
            }
            AnnotKind::Note => d.set("Name", Object::Name(b"Comment".to_vec())),
            AnnotKind::TextBox => {
                d.set("DA", text_obj("0 g /Helv 11 Tf"));
            }
            _ => {}
        }
        let annot_id = self.add(Object::Dictionary(d));
        // Link it from the page's /Annots.
        let page = self
            .doc
            .get_dictionary(info.id)
            .map_err(|e| e.to_string())?;
        if let Ok(Object::Reference(arr)) = page.get(b"Annots") {
            let arr = *arr;
            self.touch(arr);
            if let Ok(Object::Array(v)) = self.doc.get_object_mut(arr) {
                v.push(Object::Reference(annot_id));
            }
        } else {
            self.touch(info.id);
            let page = self
                .doc
                .get_dictionary_mut(info.id)
                .map_err(|e| e.to_string())?;
            match page.get_mut(b"Annots") {
                Ok(Object::Array(v)) => v.push(Object::Reference(annot_id)),
                _ => page.set("Annots", vec![Object::Reference(annot_id)]),
            }
        }
        self.commit();
        Ok(())
    }

    /// Delete a comment (and its pop-up).
    pub fn delete_annotation(&mut self, a: &Annot) -> Result<(), String> {
        let id = a.id.ok_or("the comment cannot be changed")?;
        let popup = self
            .doc
            .get_dictionary(id)
            .ok()
            .and_then(|d| d.get(b"Popup").ok())
            .and_then(|p| p.as_reference().ok());
        self.begin("delete comment");
        let mut ids = vec![id];
        ids.extend(popup);
        let r = self.remove_annots(a.page, &ids);
        self.commit();
        r
    }

    /// Change a comment's text (a text box shows the new text).
    pub fn set_annotation_contents(&mut self, a: &Annot, text: &str) -> Result<(), String> {
        let id = a.id.ok_or("the comment cannot be changed")?;
        let rect = {
            let d = self.doc.get_dictionary(id).map_err(|e| e.to_string())?;
            crate::annot_rect(&self.doc, d).ok_or("comment box")?
        };
        self.begin("edit comment");
        self.touch(id);
        let now = pdf_date();
        if let Ok(d) = self.doc.get_dictionary_mut(id) {
            d.set("Contents", text_obj(text));
            d.set("M", text_obj(&now));
        }
        if a.kind == AnnotKind::TextBox {
            // A new appearance with the text in the embedded Unicode font.
            let (font, _) = self.embed_text(crate::fallback::SANS, "")?;
            let [r, g, b] = a.color.map(|c| f32::from(c) / 255.0);
            let mut s = format!(
                "{r} {g} {b} RG 1 w {} {} {} {} re S\nBT /BoF 11 Tf 0 g 13 TL {} {} Td\n",
                rect[0] + 0.5,
                rect[1] + 0.5,
                rect[2] - rect[0] - 1.0,
                rect[3] - rect[1] - 1.0,
                rect[0] + 4.0,
                rect[3] - 14.0
            );
            for line in text.lines() {
                let (_, bytes) = self.embed_text(crate::fallback::SANS, line)?;
                let hex = bytes.iter().fold(String::new(), |mut h, b| {
                    use std::fmt::Write as _;
                    let _ = write!(h, "{b:02X}");
                    h
                });
                s.push_str(&format!("<{hex}> Tj T*\n"));
            }
            s.push_str("ET\n");
            let mut fonts = Dictionary::new();
            fonts.set("BoF", Object::Reference(font));
            let mut res = Dictionary::new();
            res.set("Font", fonts);
            let mut ap_dict = Dictionary::new();
            ap_dict.set("Type", Object::Name(b"XObject".to_vec()));
            ap_dict.set("Subtype", Object::Name(b"Form".to_vec()));
            ap_dict.set("BBox", nums(&rect));
            ap_dict.set("Resources", res);
            let ap_id = self.add(Object::Stream(Stream::new(ap_dict, s.into_bytes())));
            if let Ok(d) = self.doc.get_dictionary_mut(id) {
                let mut ap = Dictionary::new();
                ap.set("N", Object::Reference(ap_id));
                d.set("AP", ap);
            }
        }
        self.commit();
        Ok(())
    }

    /// Whether comments were added since opening or the last save.
    pub fn modified(&self) -> bool {
        !self.changed.is_empty() || self.edit.edits > 0
    }

    /// The document with the changes appended as an incremental update; afterwards the
    /// document is unmodified and the returned bytes are its new original.
    /// Page tree edits rewrite the whole file instead ([`PdfDocument::save_full`]); comments,
    /// form values and content edits keep earlier revisions (and signatures over them) intact.
    pub fn save(&mut self) -> Result<Vec<u8>, String> {
        if self.edit.structural {
            return self.save_full(crate::SaveOptions::default());
        }
        if self.changed.is_empty() {
            return Ok(self.bytes.to_vec());
        }
        self.compress_changed();
        let out = crate::write::incremental(&self.bytes, &self.doc, &self.changed)?;
        self.saved(out.clone());
        Ok(out)
    }
}
