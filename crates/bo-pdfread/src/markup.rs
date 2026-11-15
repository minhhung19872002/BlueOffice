//! More comment kinds for BlueEditor (ISO 32000-1 §12.5.6): typewriter and callout text
//! (`FreeText` with `/IT`), rectangle and ellipse (`Square`, `Circle`), line and arrow
//! (`Line` with `/LE`), polygon and cloud (`Polygon`, cloudy border `/BE`), pencil (`Ink`),
//! stamps of our own design (`Stamp`) and file attachments (`FileAttachment` with an embedded
//! file, §7.11.4). Each gets an appearance stream; text is drawn as glyph outlines of the
//! bundled sans face, so it shows in any script without font resources. Colour, opacity
//! (`/CA`, §12.5.2) and line width (`/BS /W`) come from the comment tool's properties.
use crate::annot::{nums, pdf_date, text_obj};
use crate::interp::{apply, invert};
use crate::PdfDocument;
use lopdf::{Dictionary, Object, Stream, StringFormat};
use std::fmt::Write as _;

/// What a [`Markup`] is.
#[derive(Debug, Clone, PartialEq)]
pub enum MarkupKind {
    /// Text typed on the page without a box.
    Typewriter,
    /// Text in a box with a leader line to `points[0]` (the box is `rect`).
    Callout,
    /// Rectangle.
    Rectangle,
    /// Ellipse.
    Ellipse,
    /// Line from `points[0]` to `points[1]`.
    Line,
    /// Arrow from `points[0]` to `points[1]`.
    Arrow,
    /// Closed polygon through `points`.
    Polygon,
    /// Polygon with a cloudy border.
    Cloud,
    /// Freehand stroke through `points`.
    Pencil,
    /// A stamp showing `contents` (our own design).
    Stamp,
    /// A file attached at `rect`: file name and bytes.
    File(String, Vec<u8>),
}

/// A comment to add (geometry in page points: origin top-left, y down).
#[derive(Debug, Clone, PartialEq)]
pub struct Markup {
    /// What it is.
    pub kind: MarkupKind,
    /// Page index.
    pub page: usize,
    /// Its box (left, top, right, bottom).
    pub rect: [f32; 4],
    /// Points of lines, polygons, strokes and the callout's target.
    pub points: Vec<[f32; 2]>,
    /// Colour.
    pub color: [u8; 3],
    /// Opacity 0–1.
    pub opacity: f32,
    /// Line width in points.
    pub width: f32,
    /// Author.
    pub author: String,
    /// Comment, or the text of typewriter, callout and stamp.
    pub contents: String,
    /// Text size in points (typewriter, callout).
    pub font_size: f32,
}

fn n(v: f32) -> String {
    crate::objects::num(v)
}

fn bounds(pts: &[(f32, f32)]) -> [f32; 4] {
    pts.iter().fold(
        [
            f32::INFINITY,
            f32::INFINITY,
            f32::NEG_INFINITY,
            f32::NEG_INFINITY,
        ],
        |b, p| [b[0].min(p.0), b[1].min(p.1), b[2].max(p.0), b[3].max(p.1)],
    )
}

/// Ellipse inscribed in `r` (user space) as four Bézier arcs.
fn ellipse(r: [f32; 4]) -> String {
    let (cx, cy) = (f32::midpoint(r[0], r[2]), f32::midpoint(r[1], r[3]));
    let (rx, ry) = ((r[2] - r[0]) / 2.0, (r[3] - r[1]) / 2.0);
    let (kx, ky) = (0.5523 * rx, 0.5523 * ry);
    format!(
        "{} {} m {} {} {} {} {} {} c {} {} {} {} {} {} c {} {} {} {} {} {} c {} {} {} {} {} {} c h ",
        n(cx + rx), n(cy),
        n(cx + rx), n(cy + ky), n(cx + kx), n(cy + ry), n(cx), n(cy + ry),
        n(cx - kx), n(cy + ry), n(cx - rx), n(cy + ky), n(cx - rx), n(cy),
        n(cx - rx), n(cy - ky), n(cx - kx), n(cy - ry), n(cx), n(cy - ry),
        n(cx + kx), n(cy - ry), n(cx + rx), n(cy - ky), n(cx + rx), n(cy),
    )
}

/// A cloudy outline through a closed polygon: a half-circle bulge on each step of about
/// `step` points along every edge.
fn cloud(pts: &[(f32, f32)], step: f32) -> String {
    let mut s = String::new();
    let mut first = true;
    for i in 0..pts.len() {
        let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
        let len = ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt();
        let k = (len / step).ceil().max(1.0) as usize;
        for j in 0..k {
            let t0 = j as f32 / k as f32;
            let t1 = (j + 1) as f32 / k as f32;
            let p0 = (a.0 + (b.0 - a.0) * t0, a.1 + (b.1 - a.1) * t0);
            let p1 = (a.0 + (b.0 - a.0) * t1, a.1 + (b.1 - a.1) * t1);
            if first {
                let _ = write!(s, "{} {} m ", n(p0.0), n(p0.1));
                first = false;
            }
            // Bulge outwards (polygon points run counter-clockwise in user space).
            let (dx, dy) = (p1.0 - p0.0, p1.1 - p0.1);
            let (nx, ny) = (dy * 0.65, -dx * 0.65);
            let _ = write!(
                s,
                "{} {} {} {} {} {} c ",
                n(p0.0 + nx),
                n(p0.1 + ny),
                n(p1.0 + nx),
                n(p1.1 + ny),
                n(p1.0),
                n(p1.1)
            );
        }
    }
    s.push_str("h ");
    s
}

/// Signed area of a polygon (positive: counter-clockwise).
fn area(pts: &[(f32, f32)]) -> f32 {
    (0..pts.len())
        .map(|i| {
            let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
            a.0 * b.1 - b.0 * a.1
        })
        .sum::<f32>()
        / 2.0
}

/// Lines of `text` as outline paths at `size`, starting at (x, top) in user space.
fn text_paths(text: &str, size: f32, x: f32, top: f32) -> (String, f32, f32) {
    let Some(data) = crate::fallback::face_data(crate::fallback::SANS) else {
        return (String::new(), 0.0, 0.0);
    };
    let Ok(face) = ttf_parser::Face::parse(data, 0) else {
        return (String::new(), 0.0, 0.0);
    };
    let mut s = String::new();
    let mut y = top - size * 0.9;
    let mut w: f32 = 0.0;
    let mut lines = 0;
    for line in text.split('\n') {
        let (p, adv) = crate::sigappear::text_path(&face, line, size, x, y);
        s.push_str(&p);
        w = w.max(adv);
        y -= size * 1.2;
        lines += 1;
    }
    if !s.is_empty() {
        s.push_str("f ");
    }
    (s, w, lines as f32 * size * 1.2)
}

impl PdfDocument {
    /// Add a comment of the kinds in [`MarkupKind`] (one undo step).
    #[allow(clippy::too_many_lines)]
    pub fn add_markup(&mut self, m: &Markup) -> Result<(), String> {
        if self.doc.is_encrypted() && self.doc.encryption_state.is_none() {
            return Err("comments cannot be added to an encrypted document".into());
        }
        let info = self.pages.get(m.page).ok_or("no such page")?.clone();
        let inv = invert(crate::page_matrix(info.crop, info.rotate)).ok_or("page matrix")?;
        let user = |p: [f32; 2]| apply(inv, p[0], p[1]);
        let r0 = user([m.rect[0], m.rect[1]]);
        let r1 = user([m.rect[2], m.rect[3]]);
        let mut rect = [
            r0.0.min(r1.0),
            r0.1.min(r1.1),
            r0.0.max(r1.0),
            r0.1.max(r1.1),
        ];
        let pts: Vec<(f32, f32)> = m.points.iter().map(|p| user(*p)).collect();
        let [r, g, b] = m.color.map(|c| f32::from(c) / 255.0);
        let w = m.width.max(0.25);
        let stroke = format!(
            "{} {} {} RG {} {} {} rg {} w 1 J 1 j ",
            n(r),
            n(g),
            n(b),
            n(r),
            n(g),
            n(b),
            n(w)
        );
        let mut d = Dictionary::new();
        let (subtype, body): (&str, String) = match &m.kind {
            MarkupKind::Rectangle => (
                "Square",
                format!(
                    "{stroke}{} {} {} {} re S ",
                    n(rect[0] + w / 2.0),
                    n(rect[1] + w / 2.0),
                    n(rect[2] - rect[0] - w),
                    n(rect[3] - rect[1] - w)
                ),
            ),
            MarkupKind::Ellipse => (
                "Circle",
                format!(
                    "{stroke}{}S ",
                    ellipse([
                        rect[0] + w / 2.0,
                        rect[1] + w / 2.0,
                        rect[2] - w / 2.0,
                        rect[3] - w / 2.0
                    ])
                ),
            ),
            MarkupKind::Line | MarkupKind::Arrow => {
                let (a, bp) = match pts.as_slice() {
                    [a, b, ..] => (*a, *b),
                    _ => return Err("a line needs two points".into()),
                };
                d.set("L", nums(&[a.0, a.1, bp.0, bp.1]));
                let mut s = format!(
                    "{stroke}{} {} m {} {} l S ",
                    n(a.0),
                    n(a.1),
                    n(bp.0),
                    n(bp.1)
                );
                if m.kind == MarkupKind::Arrow {
                    d.set(
                        "LE",
                        vec![
                            Object::Name(b"None".to_vec()),
                            Object::Name(b"OpenArrow".to_vec()),
                        ],
                    );
                    let len = ((bp.0 - a.0).powi(2) + (bp.1 - a.1).powi(2))
                        .sqrt()
                        .max(0.01);
                    let (ux, uy) = ((bp.0 - a.0) / len, (bp.1 - a.1) / len);
                    let h = (w * 4.0).max(6.0);
                    let (lx, ly) = (bp.0 - ux * h - uy * h * 0.5, bp.1 - uy * h + ux * h * 0.5);
                    let (rx, ry) = (bp.0 - ux * h + uy * h * 0.5, bp.1 - uy * h - ux * h * 0.5);
                    let _ = write!(
                        s,
                        "{} {} m {} {} l {} {} l S ",
                        n(lx),
                        n(ly),
                        n(bp.0),
                        n(bp.1),
                        n(rx),
                        n(ry)
                    );
                }
                let pad = (w * 4.0).max(6.0);
                let bb = bounds(&[a, bp]);
                rect = [bb[0] - pad, bb[1] - pad, bb[2] + pad, bb[3] + pad];
                ("Line", s)
            }
            MarkupKind::Polygon | MarkupKind::Cloud => {
                if pts.len() < 3 {
                    return Err("a polygon needs three points".into());
                }
                let mut pv = pts.clone();
                if area(&pv) < 0.0 {
                    pv.reverse();
                }
                let flat: Vec<f32> = pv.iter().flat_map(|p| [p.0, p.1]).collect();
                d.set("Vertices", nums(&flat));
                let s = if m.kind == MarkupKind::Cloud {
                    let mut be = Dictionary::new();
                    be.set("S", Object::Name(b"C".to_vec()));
                    be.set("I", 1);
                    d.set("BE", be);
                    format!("{stroke}{}S ", cloud(&pv, (w * 6.0).max(9.0)))
                } else {
                    let mut s = stroke.clone();
                    for (i, p) in pv.iter().enumerate() {
                        let _ = write!(
                            s,
                            "{} {} {} ",
                            n(p.0),
                            n(p.1),
                            if i == 0 { "m" } else { "l" }
                        );
                    }
                    s.push_str("h S ");
                    s
                };
                let pad = if m.kind == MarkupKind::Cloud {
                    (w * 6.0).max(9.0)
                } else {
                    w
                };
                let bb = bounds(&pv);
                rect = [bb[0] - pad, bb[1] - pad, bb[2] + pad, bb[3] + pad];
                ("Polygon", s)
            }
            MarkupKind::Pencil => {
                if pts.len() < 2 {
                    return Err("a stroke needs two points".into());
                }
                let flat: Vec<f32> = pts.iter().flat_map(|p| [p.0, p.1]).collect();
                d.set("InkList", vec![nums(&flat)]);
                let mut s = stroke.clone();
                for (i, p) in pts.iter().enumerate() {
                    let _ = write!(
                        s,
                        "{} {} {} ",
                        n(p.0),
                        n(p.1),
                        if i == 0 { "m" } else { "l" }
                    );
                }
                s.push_str("S ");
                let bb = bounds(&pts);
                rect = [bb[0] - w, bb[1] - w, bb[2] + w, bb[3] + w];
                ("Ink", s)
            }
            MarkupKind::Typewriter => {
                let size = m.font_size.max(4.0);
                let (paths, tw, th) = text_paths(&m.contents, size, rect[0] + 2.0, rect[3] - 2.0);
                rect = [rect[0], rect[3] - th - 4.0, rect[0] + tw + 4.0, rect[3]];
                d.set("IT", Object::Name(b"FreeTextTypeWriter".to_vec()));
                d.set(
                    "DA",
                    text_obj(&format!(
                        "{} {} {} rg /Helv {} Tf",
                        n(r),
                        n(g),
                        n(b),
                        n(size)
                    )),
                );
                ("FreeText", format!("{} {} {} rg {paths}", n(r), n(g), n(b)))
            }
            MarkupKind::Callout => {
                let size = m.font_size.max(4.0);
                let (paths, _, _) = text_paths(&m.contents, size, rect[0] + 4.0, rect[3] - 4.0);
                let target = pts
                    .first()
                    .copied()
                    .unwrap_or((rect[0] - 40.0, rect[1] - 40.0));
                // Leader from the target to the nearest side of the box.
                let cy = f32::midpoint(rect[1], rect[3]);
                let knee_x = if target.0 < rect[0] { rect[0] } else { rect[2] };
                let mut s = format!(
                    "{stroke}{} {} {} {} re S {} {} m {} {} l S 0 0 0 rg {paths}",
                    n(rect[0] + w / 2.0),
                    n(rect[1] + w / 2.0),
                    n(rect[2] - rect[0] - w),
                    n(rect[3] - rect[1] - w),
                    n(target.0),
                    n(target.1),
                    n(knee_x),
                    n(cy)
                );
                let _ = write!(s, "{} {} {} rg ", n(r), n(g), n(b));
                d.set("IT", Object::Name(b"FreeTextCallout".to_vec()));
                d.set("CL", nums(&[target.0, target.1, knee_x, cy]));
                d.set("DA", text_obj(&format!("0 g /Helv {} Tf", n(size))));
                let bb = bounds(&[target, (rect[0], rect[1]), (rect[2], rect[3])]);
                rect = [bb[0] - w, bb[1] - w, bb[2] + w, bb[3] + w];
                ("FreeText", s)
            }
            MarkupKind::Stamp => {
                // Our own stamp: a rounded frame with the text in capitals.
                let label = m.contents.to_uppercase();
                let h = rect[3] - rect[1];
                let size = (h * 0.45).max(6.0);
                let (paths, tw, _) = text_paths(&label, size, 0.0, 0.0);
                let width = tw + size * 1.6;
                rect = [rect[0], rect[1], rect[0] + width, rect[1] + h];
                let rr = h * 0.18;
                let (x0, y0, x1, y1) = (rect[0] + 1.5, rect[1] + 1.5, rect[2] - 1.5, rect[3] - 1.5);
                let frame = format!(
                    "{} {} m {} {} l {} {} {} {} {} {} c {} {} l {} {} {} {} {} {} c {} {} l {} {} {} {} {} {} c {} {} l {} {} {} {} {} {} c h S ",
                    n(x0 + rr), n(y0), n(x1 - rr), n(y0),
                    n(x1), n(y0), n(x1), n(y0), n(x1), n(y0 + rr),
                    n(x1), n(y1 - rr),
                    n(x1), n(y1), n(x1), n(y1), n(x1 - rr), n(y1),
                    n(x0 + rr), n(y1),
                    n(x0), n(y1), n(x0), n(y1), n(x0), n(y1 - rr),
                    n(x0), n(y0 + rr),
                    n(x0), n(y0), n(x0), n(y0), n(x0 + rr), n(y0),
                );
                let s = format!(
                    "{} {} {} RG 3 w {frame}{} {} {} rg q 1 0 0 1 {} {} cm {paths}Q ",
                    n(r),
                    n(g),
                    n(b),
                    n(r),
                    n(g),
                    n(b),
                    n(rect[0] + size * 0.8),
                    n(rect[1] + (h - size * 0.7) / 2.0)
                );
                d.set("Name", Object::Name(b"Approved".to_vec()));
                ("Stamp", s)
            }
            MarkupKind::File(name, bytes) => {
                let mut ef_stream = Dictionary::new();
                ef_stream.set("Type", Object::Name(b"EmbeddedFile".to_vec()));
                let mut params = Dictionary::new();
                params.set("Size", bytes.len() as i64);
                ef_stream.set("Params", params);
                self.begin("comment");
                let ef = self.add(Object::Stream(Stream::new(ef_stream, bytes.clone())));
                let mut efd = Dictionary::new();
                efd.set("F", Object::Reference(ef));
                efd.set("UF", Object::Reference(ef));
                let mut fs = Dictionary::new();
                fs.set("Type", Object::Name(b"Filespec".to_vec()));
                fs.set(
                    "F",
                    Object::String(name.as_bytes().to_vec(), StringFormat::Literal),
                );
                fs.set("UF", text_obj(name));
                fs.set("EF", efd);
                d.set("FS", fs);
                d.set("Name", Object::Name(b"Paperclip".to_vec()));
                rect = [rect[0], rect[3] - 20.0, rect[0] + 14.0, rect[3]];
                // A paper clip of our own.
                let (x, y) = (rect[0] + 3.0, rect[1] + 3.0);
                let s = format!(
                    "{} {} {} RG 1.4 w 1 J {} {} m {} {} l {} {} {} {} {} {} c {} {} {} {} {} {} c {} {} l {} {} {} {} {} {} c {} {} {} {} {} {} c {} {} l S ",
                    n(r), n(g), n(b),
                    n(x + 2.0), n(y + 8.0), n(x + 2.0), n(y + 2.0),
                    n(x + 2.0), n(y - 1.0), n(x + 7.0), n(y - 1.0), n(x + 7.0), n(y + 2.0),
                    n(x + 7.0), n(y + 2.0), n(x + 7.0), n(y + 2.0), n(x + 7.0), n(y + 2.0),
                    n(x + 7.0), n(y + 13.0),
                    n(x + 7.0), n(y + 16.0), n(x + 4.0), n(y + 16.0), n(x + 4.0), n(y + 16.0),
                    n(x + 1.0), n(y + 16.0), n(x), n(y + 15.0), n(x), n(y + 13.0),
                    n(x), n(y + 4.0),
                );
                ("FileAttachment", s)
            }
        };
        let opacity = m.opacity.clamp(0.05, 1.0);
        let mut res = Dictionary::new();
        let mut content = String::new();
        if opacity < 0.999 {
            let mut gs = Dictionary::new();
            gs.set("Type", Object::Name(b"ExtGState".to_vec()));
            gs.set("CA", Object::Real(opacity));
            gs.set("ca", Object::Real(opacity));
            let mut ext = Dictionary::new();
            ext.set("GS0", gs);
            res.set("ExtGState", ext);
            content.push_str("/GS0 gs ");
            d.set("CA", Object::Real(opacity));
        }
        content.push_str(&body);
        let mut ap_dict = Dictionary::new();
        ap_dict.set("Type", Object::Name(b"XObject".to_vec()));
        ap_dict.set("Subtype", Object::Name(b"Form".to_vec()));
        ap_dict.set("BBox", nums(&rect));
        ap_dict.set("Resources", res);
        self.begin("comment");
        let ap_id = self.add(Object::Stream(Stream::new(ap_dict, content.into_bytes())));
        let mut ap = Dictionary::new();
        ap.set("N", Object::Reference(ap_id));
        let now = pdf_date();
        d.set("Type", Object::Name(b"Annot".to_vec()));
        d.set("Subtype", Object::Name(subtype.as_bytes().to_vec()));
        d.set("Rect", nums(&rect));
        d.set("C", nums(&[r, g, b]));
        let mut bs = Dictionary::new();
        bs.set("W", Object::Real(w));
        d.set("BS", bs);
        d.set("F", 4);
        d.set("P", Object::Reference(info.id));
        d.set("T", text_obj(&m.author));
        d.set("Contents", text_obj(&m.contents));
        d.set("M", text_obj(&now));
        d.set("CreationDate", text_obj(&now));
        d.set("NM", text_obj(&format!("bo-{}-{}", ap_id.0, now)));
        d.set("AP", ap);
        let annot_id = self.add(Object::Dictionary(d));
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{standard_fonts, AnnotKind, RenderOptions};

    fn doc() -> PdfDocument {
        let objs = [
            "<< /Type /Catalog /Pages 2 0 R >>",
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 300] /Resources << >> >>",
        ];
        PdfDocument::open(crate::tests::build_pdf(&objs), standard_fonts()).expect("opens")
    }

    fn markup(kind: MarkupKind) -> Markup {
        Markup {
            kind,
            page: 0,
            rect: [50.0, 50.0, 150.0, 100.0],
            points: vec![[40.0, 40.0], [120.0, 60.0], [90.0, 140.0]],
            color: [200, 0, 0],
            opacity: 0.5,
            width: 2.0,
            author: "Tester".into(),
            contents: "Đã duyệt".into(),
            font_size: 12.0,
        }
    }

    #[test]
    fn every_kind_is_written_and_drawn() {
        let kinds = [
            (MarkupKind::Typewriter, AnnotKind::TextBox),
            (MarkupKind::Callout, AnnotKind::TextBox),
            (MarkupKind::Rectangle, AnnotKind::Square),
            (MarkupKind::Ellipse, AnnotKind::Circle),
            (MarkupKind::Line, AnnotKind::Line),
            (MarkupKind::Arrow, AnnotKind::Line),
            (MarkupKind::Polygon, AnnotKind::Line),
            (MarkupKind::Cloud, AnnotKind::Line),
            (MarkupKind::Pencil, AnnotKind::Ink),
            (MarkupKind::Stamp, AnnotKind::Stamp),
            (
                MarkupKind::File("a.txt".into(), b"hello".to_vec()),
                AnnotKind::Other,
            ),
        ];
        let mut d = doc();
        for (k, _) in &kinds {
            d.add_markup(&markup(k.clone())).expect("adds");
        }
        let list = d.annotations();
        assert_eq!(list.len(), kinds.len());
        for (a, (_, want)) in list.iter().zip(&kinds) {
            assert_eq!(a.kind, *want);
        }
        let saved = d.save().expect("saves");
        let text = String::from_utf8_lossy(&saved);
        assert!(text.contains("/IT /FreeTextTypeWriter") && text.contains("/IT /FreeTextCallout"));
        assert!(text.contains("/BE <<") && text.contains("/LE [/None /OpenArrow]"));
        assert!(text.contains("/Type /EmbeddedFile") && text.contains("/CA 0.5"));
        let mut again = PdfDocument::open(saved, standard_fonts()).expect("opens");
        let page = again.render(0, RenderOptions::default()).expect("renders");
        assert!(page.list.commands.len() >= kinds.len());
        // Undo removes them one by one.
        assert!(d.undo());
        assert_eq!(d.annotations().len(), kinds.len() - 1);
    }
}
