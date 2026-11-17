//! Convert tab exports of BlueEditor: every picture of the document as a file (JPEG streams
//! as they are, other images decoded to PNG; ISO 32000-1 §8.9 image XObjects) and the
//! document as one HTML file (each page a box of its size with the text lines and pictures
//! positioned where they are on the page).
use crate::color::ColorSpace;
use crate::objects::ObjectKind;
use crate::{resolve, PdfDocument};
use lopdf::{Object, ObjectId};
use std::collections::HashSet;

/// A picture taken out of the document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractedImage {
    /// Page it is first drawn on.
    pub page: usize,
    /// File contents.
    pub data: Vec<u8>,
    /// File extension: `jpg` or `png`.
    pub ext: &'static str,
}

/// Standard base64 (RFC 4648 §4) for `data:` URIs.
fn base64(data: &[u8]) -> String {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::with_capacity(data.len().div_ceil(3) * 4);
    for c in data.chunks(3) {
        let b = [c[0], *c.get(1).unwrap_or(&0), *c.get(2).unwrap_or(&0)];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for k in 0..4 {
            if k <= c.len() {
                s.push(char::from(A[((n >> (18 - 6 * k)) & 63) as usize]));
            } else {
                s.push('=');
            }
        }
    }
    s
}

fn escape(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            c if c.is_control() => {}
            c => o.push(c),
        }
    }
    o
}

/// PNG of premultiplied RGBA pixels.
fn png_of(w: u32, h: u32, premul: &[u8]) -> Option<Vec<u8>> {
    let mut px = premul.to_vec();
    let mut opaque = true;
    for p in px.chunks_exact_mut(4) {
        let a = p[3];
        if a < 255 {
            opaque = false;
            if a > 0 {
                for c in &mut p[..3] {
                    *c = ((u32::from(*c) * 255 + u32::from(a) / 2) / u32::from(a)).min(255) as u8;
                }
            }
        }
    }
    let (data, color) = if opaque {
        (
            px.chunks_exact(4)
                .flat_map(|p| [p[0], p[1], p[2]])
                .collect::<Vec<u8>>(),
            png::ColorType::Rgb,
        )
    } else {
        (px, png::ColorType::Rgba)
    };
    let mut out = Vec::new();
    {
        let mut e = png::Encoder::new(&mut out, w, h);
        e.set_color(color);
        e.set_depth(png::BitDepth::Eight);
        let mut wr = e.write_header().ok()?;
        wr.write_image_data(&data).ok()?;
    }
    Some(out)
}

impl PdfDocument {
    /// Image XObject drawn by page object `o` of page `page` (through `Do`).
    fn drawn_image(&self, page: usize, o: &crate::PageObject) -> Option<ObjectId> {
        if o.kind != ObjectKind::Image || o.rec.op != b"Do" {
            return None;
        }
        let name = o.rec.args.first()?.as_name().ok()?;
        let pid = self.pages.get(page)?.id;
        let p = self.doc.get_dictionary(pid).ok()?;
        let res = crate::inherited(&self.doc, p, b"Resources")?
            .as_dict()
            .ok()?;
        let x = resolve(&self.doc, res.get(b"XObject").ok()?)
            .as_dict()
            .ok()?;
        x.get(name).ok()?.as_reference().ok()
    }

    /// The picture file of image XObject `id`: a JPEG stream as it is (grey or RGB), any
    /// other image decoded to PNG.
    fn image_file(&self, id: ObjectId) -> Option<(Vec<u8>, &'static str)> {
        let s = self.doc.get_object(id).ok()?.as_stream().ok()?;
        let d = &s.dict;
        let none = |_: &[u8]| None;
        let space = d
            .get(b"ColorSpace")
            .ok()
            .and_then(|c| ColorSpace::parse(&self.doc, c, &none, 0));
        let filter = match d.get(b"Filter").map(|f| resolve(&self.doc, f)) {
            Ok(Object::Name(f)) => f.clone(),
            Ok(Object::Array(a)) if a.len() == 1 => {
                a[0].as_name().map(<[u8]>::to_vec).unwrap_or_default()
            }
            _ => Vec::new(),
        };
        let plain = matches!(space, Some(ColorSpace::Gray | ColorSpace::Rgb))
            && !d.has(b"SMask")
            && !d.has(b"Mask")
            && !d.has(b"Decode");
        if filter == b"DCTDecode" && plain {
            return Some((s.content.clone(), "jpg"));
        }
        let img = crate::image::decode(&crate::image::Params {
            doc: &self.doc,
            dict: d,
            data: &s.content,
            space,
            fill: [0.0; 3],
        })?;
        Some((png_of(img.width, img.height, &img.pixels)?, "png"))
    }

    /// The picture file of page object `id` of `page` when it is an image XObject (JPEG as
    /// stored, else PNG; Edit Object › Extract Image, round 8).
    pub fn object_image(&mut self, page: usize, id: usize) -> Option<(Vec<u8>, &'static str)> {
        let objs = self.page_objects(page).ok()?;
        let x = self.drawn_image(page, objs.get(id)?)?;
        self.image_file(x)
    }

    /// Every picture of the document once, in page order (Export All Images).
    pub fn extract_images(&mut self) -> Vec<ExtractedImage> {
        let mut seen = HashSet::new();
        let mut out = Vec::new();
        for page in 0..self.pages.len() {
            let Ok(objs) = self.page_objects(page) else {
                continue;
            };
            for o in &objs {
                let Some(id) = self.drawn_image(page, o) else {
                    continue;
                };
                if !seen.insert(id) {
                    continue;
                }
                if let Some((data, ext)) = self.image_file(id) {
                    out.push(ExtractedImage { page, data, ext });
                }
            }
        }
        out
    }

    /// The document as one HTML file: a box per page with its text lines and pictures where
    /// they are (pictures as `data:` URIs).
    pub fn to_html(&mut self, title: &str) -> String {
        let mut h = format!(
            "<!DOCTYPE html>\n<html>\n<head>\n<meta charset=\"utf-8\">\n<title>{}</title>\n\
             <style>\nbody{{background:#e8e8e8;margin:0;padding:12pt 0}}\n\
             .page{{position:relative;background:#fff;margin:0 auto 12pt;overflow:hidden;\
             box-shadow:0 1px 3px rgba(0,0,0,.3)}}\n\
             .page span{{position:absolute;white-space:pre;line-height:1;\
             font-family:'Times New Roman',serif}}\n\
             .page img{{position:absolute}}\n</style>\n</head>\n<body>\n",
            escape(title)
        );
        let mut uris: std::collections::HashMap<ObjectId, Option<String>> =
            std::collections::HashMap::new();
        for page in 0..self.pages.len() {
            let (w, ht) = self
                .pages
                .get(page)
                .map_or((595.0, 842.0), |p| (p.width, p.height));
            h.push_str(&format!(
                "<div class=\"page\" style=\"width:{w:.1}pt;height:{ht:.1}pt\">\n"
            ));
            let objs = self.page_objects(page).unwrap_or_default();
            for o in &objs {
                let Some(id) = self.drawn_image(page, o) else {
                    continue;
                };
                let uri = uris
                    .entry(id)
                    .or_insert_with(|| {
                        self.image_file(id).map(|(data, ext)| {
                            let mime = if ext == "jpg" { "jpeg" } else { "png" };
                            format!("data:image/{mime};base64,{}", base64(&data))
                        })
                    })
                    .clone();
                if let Some(uri) = uri {
                    let b = o.bbox;
                    h.push_str(&format!(
                        "<img src=\"{uri}\" style=\"left:{:.1}pt;top:{:.1}pt;width:{:.1}pt;height:{:.1}pt\" alt=\"\">\n",
                        b[0],
                        b[1],
                        (b[2] - b[0]).abs(),
                        (b[3] - b[1]).abs()
                    ));
                }
            }
            for line in crate::text_lines(&objs) {
                if line.text.trim().is_empty() {
                    continue;
                }
                let b = line.bbox;
                let c = line.color;
                h.push_str(&format!(
                    "<span style=\"left:{:.1}pt;top:{:.1}pt;font-size:{:.1}pt;color:#{:02x}{:02x}{:02x}\">{}</span>\n",
                    b[0],
                    b[1],
                    line.font_size,
                    c[0],
                    c[1],
                    c[2],
                    escape(&line.text)
                ));
            }
            h.push_str("</div>\n");
        }
        h.push_str("</body>\n</html>\n");
        h
    }
}

#[cfg(test)]
mod tests {
    use super::base64;

    #[test]
    fn pictures_and_html() {
        // A 2 × 1 RGB picture (red, blue) and a text line.
        let body = "q 40 0 0 20 10 60 cm /Im1 Do Q BT /F1 12 Tf 20 30 Td (Hello <you>) Tj ET";
        let objs = [
            "<< /Type /Catalog /Pages 2 0 R >>".to_owned(),
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_owned(),
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 100] /Contents 4 0 R \
             /Resources << /XObject << /Im1 5 0 R >> /Font << /F1 6 0 R >> >> >>"
                .to_owned(),
            format!("<< /Length {} >>\nstream\n{body}\nendstream", body.len()),
            "<< /Type /XObject /Subtype /Image /Width 2 /Height 1 /ColorSpace /DeviceRGB \
             /BitsPerComponent 8 /Length 6 >>\nstream\nA\0\0\0\0A\nendstream"
                .to_owned(),
            "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_owned(),
        ];
        let refs: Vec<&str> = objs.iter().map(String::as_str).collect();
        let bytes: Vec<u8> = crate::tests::build_pdf(&refs);
        let mut doc = crate::PdfDocument::open(bytes, crate::standard_fonts()).expect("opens");
        let imgs = doc.extract_images();
        assert_eq!(imgs.len(), 1);
        assert_eq!(imgs[0].ext, "png");
        assert!(imgs[0].data.starts_with(b"\x89PNG"));
        let html = doc.to_html("t");
        assert!(html.contains("data:image/png;base64,"));
        assert!(html.contains("Hello &lt;you&gt;"));
        assert!(html.contains("width:200.0pt;height:100.0pt"));
    }

    #[test]
    fn base64_matches_rfc4648_vectors() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }
}
