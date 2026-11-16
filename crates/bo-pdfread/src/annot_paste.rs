//! Pasting a copied comment (BlueReader round 10, the page menu's Paste / Paste in Place): a
//! copy of the annotation dictionary on another (or the same) page, sharing its appearance
//! stream (ISO 32000-1 §12.5.2: `/P` is the new page; the copy has no `/Popup`, `/IRT` or `/NM`
//! of its own), then moved to where it is pasted.
use crate::annot::{pdf_date, text_obj, Annot};
use crate::PdfDocument;
use lopdf::Object;

impl PdfDocument {
    /// Paste a copy of comment `a` on `page`, its top left corner at `at` (page points, y
    /// down), or at the same place as the original when `at` is `None` (Paste in Place). One
    /// undo step.
    pub fn paste_annotation(
        &mut self,
        a: &Annot,
        page: usize,
        at: Option<[f32; 2]>,
    ) -> Result<(), String> {
        if self.doc.is_encrypted() && self.doc.encryption_state.is_none() {
            return Err("comments cannot be added to an encrypted document".into());
        }
        let id = a.id.ok_or("the comment cannot be copied")?;
        let info = self.pages.get(page).ok_or("no such page")?.clone();
        let mut d = self
            .doc
            .get_dictionary(id)
            .map_err(|e| e.to_string())?
            .clone();
        for k in [&b"Popup"[..], b"IRT", b"NM", b"Parent"] {
            d.remove(k);
        }
        d.set("P", Object::Reference(info.id));
        let now = pdf_date();
        d.set("CreationDate", text_obj(&now));
        d.set("M", text_obj(&now));
        self.begin("paste comment");
        let new_id = self.add(Object::Dictionary(d));
        let annots = self
            .doc
            .get_dictionary(info.id)
            .ok()
            .and_then(|p| p.get(b"Annots").ok())
            .and_then(|o| o.as_reference().ok());
        let mut linked = false;
        if let Some(arr) = annots {
            self.touch(arr);
            if let Ok(Object::Array(v)) = self.doc.get_object_mut(arr) {
                v.push(Object::Reference(new_id));
                linked = true;
            }
        }
        if !linked {
            self.touch(info.id);
            if let Ok(p) = self.doc.get_dictionary_mut(info.id) {
                match p.get_mut(b"Annots") {
                    Ok(Object::Array(v)) => v.push(Object::Reference(new_id)),
                    _ => p.set("Annots", vec![Object::Reference(new_id)]),
                }
            }
        }
        // The copy sits where the original was (in this page's space); move it to `at`.
        if let Some([x, y]) = at {
            let copy = Annot {
                id: Some(new_id),
                page,
                ..a.clone()
            };
            let (dx, dy) = (x - a.rect[0], y - a.rect[1]);
            if dx.abs() + dy.abs() > 1e-3 {
                self.move_annotation(&copy, dx, dy)?;
            }
        }
        self.commit();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::{standard_fonts, AnnotKind, NewAnnot, PdfDocument};

    fn doc() -> PdfDocument {
        let pdf = crate::tests::build_pdf(&[
            "<< /Type /Catalog /Pages 2 0 R >>",
            "<< /Type /Pages /Kids [3 0 R 4 0 R] /Count 2 >>",
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] >>",
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] >>",
        ]);
        PdfDocument::open(pdf, standard_fonts()).expect("opens")
    }

    #[test]
    fn paste_at_a_point_and_in_place() {
        let mut d = doc();
        d.add_annotation(&NewAnnot {
            kind: AnnotKind::Square,
            page: 0,
            rects: vec![[100.0, 100.0, 200.0, 150.0]],
            color: [229, 57, 53],
            author: "me".into(),
            contents: "box".into(),
        })
        .expect("square");
        let a = d.annotations().remove(0);
        d.paste_annotation(&a, 1, Some([300.0, 400.0]))
            .expect("paste");
        d.paste_annotation(&a, 0, None).expect("in place");
        let all = d.annotations();
        assert_eq!(all.len(), 3);
        let moved = all.iter().find(|x| x.page == 1).expect("on page 2");
        assert!((moved.rect[0] - 300.0).abs() < 0.01 && (moved.rect[1] - 400.0).abs() < 0.01);
        assert_eq!(moved.contents, "box");
        let same: Vec<_> = all.iter().filter(|x| x.page == 0).collect();
        assert_eq!(same.len(), 2);
        assert_eq!(same[0].rect, same[1].rect);
        // One undo step per paste.
        assert!(d.undo());
        assert_eq!(d.annotations().len(), 2);
        let saved = d.save().expect("saves");
        let again = PdfDocument::open(saved, standard_fonts()).expect("reopens");
        assert_eq!(again.annotations().len(), 2);
    }
}
