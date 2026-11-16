//! Insert Text: a caret annotation (ISO 32000-1 §12.5.6.11) marking where text goes in, with
//! the text to insert as its contents. The appearance is a filled caret whose point touches
//! the insertion point and whose base sits under the line.
use crate::annot::{pdf_date, text_obj};
use crate::interp::{apply, invert};
use crate::PdfDocument;
use lopdf::{Dictionary, Object, Stream};

impl PdfDocument {
    /// Add a caret in `rect` (page points: left, top, right, bottom) on `page`, with `text`
    /// (what is to be inserted) as its comment. It shows at once and is written by
    /// [`PdfDocument::save`].
    pub fn add_caret(
        &mut self,
        page: usize,
        rect: [f32; 4],
        color: [u8; 3],
        author: &str,
        text: &str,
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
        let [cr, cg, cb] = color.map(|c| f32::from(c) / 255.0);
        self.begin("caret");
        let result = (|| -> Result<(), String> {
            // §12.5.5: the form's BBox maps onto the annotation's Rect.
            let mut fd = Dictionary::new();
            fd.set("Type", Object::Name(b"XObject".to_vec()));
            fd.set("Subtype", Object::Name(b"Form".to_vec()));
            fd.set("BBox", vec![0.into(), 0.into(), w.into(), h.into()]);
            let (m, k) = (w / 2.0, h * 0.28);
            let content = format!(
                "{cr:.3} {cg:.3} {cb:.3} rg 0 0 m {m:.3} {h:.3} l {w:.3} 0 l \
                 {:.3} {k:.3} {m:.3} {k:.3} {m:.3} {k:.3} c {:.3} {k:.3} 0 0 0 0 c f",
                w * 0.75,
                w * 0.25
            );
            let ap = self.add(Object::Stream(Stream::new(fd, content.into_bytes())));
            let mut d = Dictionary::new();
            d.set("Type", Object::Name(b"Annot".to_vec()));
            d.set("Subtype", Object::Name(b"Caret".to_vec()));
            d.set("F", 4);
            d.set("P", Object::Reference(info.id));
            d.set("T", text_obj(author));
            d.set("Contents", text_obj(text));
            d.set("Subj", text_obj("Inserted Text"));
            let now = pdf_date();
            d.set("CreationDate", text_obj(&now));
            d.set("M", text_obj(&now));
            d.set("Rect", vec![l.into(), b.into(), r.into(), t.into()]);
            d.set(
                "C",
                vec![Object::Real(cr), Object::Real(cg), Object::Real(cb)],
            );
            // No paragraph symbol (Table 183).
            d.set("Sy", Object::Name(b"None".to_vec()));
            let mut apd = Dictionary::new();
            apd.set("N", Object::Reference(ap));
            d.set("AP", Object::Dictionary(apd));
            let id = self.add(Object::Dictionary(d));
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

    #[test]
    fn caret_draws_and_saves_as_a_caret_annotation() {
        let pdf = build_pdf(&[
            "<< /Type /Catalog /Pages 2 0 R >>",
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 100] >>",
        ]);
        let mut doc = PdfDocument::open(pdf, standard_fonts()).expect("opens");
        doc.add_caret(0, [50.0, 40.0, 58.0, 52.0], [0, 0, 255], "Me", "new words")
            .expect("adds");
        let p = doc.render(0, RenderOptions::default()).expect("renders");
        assert!(p.list.commands.len() > 1);
        let saved = doc.save().expect("saves");
        let s = String::from_utf8_lossy(&saved);
        assert!(s.contains("/Caret") && s.contains("(new words)") && s.contains("/Sy /None"));
        let again = PdfDocument::open(saved, standard_fonts()).expect("reopens");
        let a = again.annotations();
        assert_eq!(a.len(), 1);
        assert_eq!(a[0].contents, "new words");
    }
}
