//! Stamps showing a picture (BlueReader's Custom Stamp, ISO 32000-1 §12.5.6.12): a `/Stamp`
//! annotation whose normal appearance draws a JPEG or PNG image over its rectangle.
use crate::annot::{pdf_date, text_obj};
use crate::interp::{apply, invert};
use crate::objects::NewImage;
use crate::PdfDocument;
use lopdf::{Dictionary, Object, Stream};

impl PdfDocument {
    /// Add a stamp showing `image` (JPEG or PNG bytes) in `rect` (page points: left, top,
    /// right, bottom) on `page`. It shows at once and is written by [`PdfDocument::save`].
    pub fn add_image_stamp(
        &mut self,
        page: usize,
        rect: [f32; 4],
        image: &[u8],
        author: &str,
    ) -> Result<(), String> {
        if self.doc.is_encrypted() && self.doc.encryption_state.is_none() {
            return Err("comments cannot be added to an encrypted document".into());
        }
        let info = self.pages.get(page).ok_or("no such page")?.clone();
        let base = crate::page_matrix(info.crop, info.rotate);
        let inv = invert(base).ok_or("page matrix")?;
        let (x0, y0) = apply(inv, rect[0], rect[1]);
        let (x1, y1) = apply(inv, rect[2], rect[3]);
        let (l, b, r, t) = (x0.min(x1), y0.min(y1), x0.max(x1), y0.max(y1));
        let (w, h) = ((r - l).max(1.0), (t - b).max(1.0));
        self.begin("image stamp");
        let result = (|| -> Result<(), String> {
            let img = self.image_xobject(NewImage(image))?;
            // §12.5.5: the form's BBox maps onto the annotation's Rect.
            let mut res = Dictionary::new();
            let mut xo = Dictionary::new();
            xo.set("Im0", Object::Reference(img));
            res.set("XObject", Object::Dictionary(xo));
            let mut fd = Dictionary::new();
            fd.set("Type", Object::Name(b"XObject".to_vec()));
            fd.set("Subtype", Object::Name(b"Form".to_vec()));
            fd.set("BBox", vec![0.into(), 0.into(), w.into(), h.into()]);
            fd.set("Resources", Object::Dictionary(res));
            let content = format!("q {w:.3} 0 0 {h:.3} 0 0 cm /Im0 Do Q");
            let ap = self.add(Object::Stream(Stream::new(fd, content.into_bytes())));
            let mut d = Dictionary::new();
            d.set("Type", Object::Name(b"Annot".to_vec()));
            d.set("Subtype", Object::Name(b"Stamp".to_vec()));
            d.set("Name", Object::Name(b"Image".to_vec()));
            d.set("F", 4);
            d.set("P", Object::Reference(info.id));
            d.set("T", text_obj(author));
            let now = pdf_date();
            d.set("CreationDate", text_obj(&now));
            d.set("M", text_obj(&now));
            d.set("Rect", vec![l.into(), b.into(), r.into(), t.into()]);
            let mut apd = Dictionary::new();
            apd.set("N", Object::Reference(ap));
            d.set("AP", Object::Dictionary(apd));
            let id = self.add(Object::Dictionary(d));
            // Link it from the page's /Annots (an indirect array or the page's own).
            let annots = self
                .doc
                .get_dictionary(info.id)
                .map_err(|e| e.to_string())?
                .get(b"Annots")
                .ok()
                .and_then(|o| o.as_reference().ok());
            if let Some(arr) = annots {
                self.touch(arr);
                if let Ok(Object::Array(v)) = self.doc.get_object_mut(arr) {
                    v.push(Object::Reference(id));
                    return Ok(());
                }
            }
            self.touch(info.id);
            let p = self
                .doc
                .get_dictionary_mut(info.id)
                .map_err(|e| e.to_string())?;
            match p.get_mut(b"Annots") {
                Ok(Object::Array(v)) => v.push(Object::Reference(id)),
                _ => p.set("Annots", vec![Object::Reference(id)]),
            }
            Ok(())
        })();
        self.commit();
        result
    }
}

#[cfg(test)]
mod tests {
    use crate::tests::build_pdf;
    use crate::{standard_fonts, PdfDocument, RenderOptions};

    fn png() -> Vec<u8> {
        let mut out = Vec::new();
        {
            let mut e = png::Encoder::new(&mut out, 2, 2);
            e.set_color(png::ColorType::Rgb);
            e.set_depth(png::BitDepth::Eight);
            let mut w = e.write_header().expect("header");
            w.write_image_data(&[255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255])
                .expect("data");
        }
        out
    }

    #[test]
    fn picture_stamp_draws_and_saves() {
        let pdf = build_pdf(&[
            "<< /Type /Catalog /Pages 2 0 R >>",
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 100] >>",
        ]);
        let mut doc = PdfDocument::open(pdf, standard_fonts()).expect("opens");
        doc.add_image_stamp(0, [10.0, 10.0, 60.0, 40.0], &png(), "me")
            .expect("stamp");
        let page = doc.render(0, RenderOptions::default()).expect("renders");
        assert!(page
            .list
            .commands
            .iter()
            .any(|c| matches!(c, bo_render::Command::Image { .. })));
        let saved = doc.save().expect("saves");
        let again = PdfDocument::open(saved, standard_fonts()).expect("reopens");
        let a = again.annotations();
        assert_eq!(a.len(), 1);
        assert_eq!(a[0].author, "me");
    }
}
