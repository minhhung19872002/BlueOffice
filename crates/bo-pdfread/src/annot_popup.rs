//! Pop-up notes of comments (ISO 32000-1 §12.5.6.14 Popup annotations): open or close them all
//! (`/Open`), and put one back at its default place beside its comment (BlueReader round 9,
//! the comment menu's Open All Popups / Close All Popups / Reset Popup Note Location).
use crate::{Annot, PdfDocument};
use lopdf::{Object, ObjectId};

/// Default pop-up note size (points), the usual viewers' note window.
const POPUP_W: f32 = 180.0;
const POPUP_H: f32 = 120.0;

impl PdfDocument {
    /// Pop-up annotations of every page.
    fn popup_ids(&self) -> Vec<ObjectId> {
        let doc = &self.doc;
        let mut out = Vec::new();
        for info in &self.pages {
            let Ok(page) = doc.get_dictionary(info.id) else {
                continue;
            };
            let list = match page.get(b"Annots") {
                Ok(Object::Array(a)) => a.clone(),
                Ok(Object::Reference(r)) => doc
                    .get_object(*r)
                    .ok()
                    .and_then(|o| o.as_array().ok())
                    .cloned()
                    .unwrap_or_default(),
                _ => Vec::new(),
            };
            for a in list {
                let Ok(id) = a.as_reference() else { continue };
                let popup = doc
                    .get_dictionary(id)
                    .ok()
                    .and_then(|d| d.get(b"Subtype").ok())
                    .and_then(|s| s.as_name().ok())
                    == Some(&b"Popup"[..]);
                if popup && !out.contains(&id) {
                    out.push(id);
                }
            }
        }
        out
    }

    /// Open (or close) every pop-up note (§12.5.6.14 `/Open`); the number changed.
    pub fn set_popups_open(&mut self, open: bool) -> Result<usize, String> {
        let ids: Vec<ObjectId> = self
            .popup_ids()
            .into_iter()
            .filter(|id| {
                let now = self
                    .doc
                    .get_dictionary(*id)
                    .ok()
                    .and_then(|d| d.get(b"Open").ok())
                    .and_then(|o| o.as_bool().ok())
                    .unwrap_or(false);
                now != open
            })
            .collect();
        if ids.is_empty() {
            return Ok(0);
        }
        self.begin(if open { "open popups" } else { "close popups" });
        for id in &ids {
            self.touch(*id);
            if let Ok(d) = self.doc.get_dictionary_mut(*id) {
                d.set("Open", open);
            }
        }
        self.commit();
        Ok(ids.len())
    }

    /// Put the pop-up note of comment `a` back beside it: its top left at the comment's top
    /// right corner, the default size.
    pub fn reset_popup(&mut self, a: &Annot) -> Result<(), String> {
        let id = a.id.ok_or("the comment cannot be changed")?;
        let d = self.doc.get_dictionary(id).map_err(|e| e.to_string())?;
        let popup = d
            .get(b"Popup")
            .ok()
            .and_then(|p| p.as_reference().ok())
            .ok_or("the comment has no pop-up note")?;
        let rect: Vec<f32> = d
            .get(b"Rect")
            .ok()
            .and_then(|r| r.as_array().ok())
            .map(|r| r.iter().filter_map(|v| v.as_float().ok()).collect())
            .unwrap_or_default();
        let [x0, y0, x1, y1] = <[f32; 4]>::try_from(rect).map_err(|_| "no rectangle")?;
        let (right, top) = (x0.max(x1), y0.max(y1));
        self.begin("reset popup");
        self.touch(popup);
        if let Ok(p) = self.doc.get_dictionary_mut(popup) {
            p.set(
                "Rect",
                vec![
                    Object::Real(right),
                    Object::Real(top - POPUP_H),
                    Object::Real(right + POPUP_W),
                    Object::Real(top),
                ],
            );
        }
        self.commit();
        Ok(())
    }

    /// Whether comment `a` has a pop-up note.
    pub fn has_popup(&self, a: &Annot) -> bool {
        a.id.and_then(|id| self.doc.get_dictionary(id).ok())
            .is_some_and(|d| d.get(b"Popup").is_ok())
    }
}

#[cfg(test)]
mod tests {
    use crate::PdfDocument;

    fn pdf() -> Vec<u8> {
        let objs = [
            "<< /Type /Catalog /Pages 2 0 R >>",
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Annots [4 0 R 5 0 R] >>",
            "<< /Type /Annot /Subtype /Text /Rect [100 700 120 720] /Contents (Hi) /Popup 5 0 R >>",
            "<< /Type /Annot /Subtype /Popup /Rect [300 300 400 400] /Parent 4 0 R /Open false >>",
        ];
        let mut out = b"%PDF-1.7\n".to_vec();
        let mut offs = Vec::new();
        for (i, o) in objs.iter().enumerate() {
            offs.push(out.len());
            out.extend_from_slice(format!("{} 0 obj\n{o}\nendobj\n", i + 1).as_bytes());
        }
        let x = out.len();
        out.extend_from_slice(
            format!("xref\n0 {}\n0000000000 65535 f \n", objs.len() + 1).as_bytes(),
        );
        for o in offs {
            out.extend_from_slice(format!("{o:010} 00000 n \n").as_bytes());
        }
        out.extend_from_slice(
            format!(
                "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{x}\n%%EOF\n",
                objs.len() + 1
            )
            .as_bytes(),
        );
        out
    }

    #[test]
    fn open_close_and_reset_popups() {
        let mut d = PdfDocument::open(pdf(), crate::standard_fonts()).expect("pdf");
        assert_eq!(d.set_popups_open(true), Ok(1));
        assert_eq!(d.set_popups_open(true), Ok(0));
        let a = d.annotations().into_iter().next().expect("comment");
        assert!(d.has_popup(&a));
        d.reset_popup(&a).expect("reset");
        let saved = d.save().expect("save");
        let again = PdfDocument::open(saved, crate::standard_fonts()).expect("reopen");
        let p = again.doc.get_dictionary((5, 0)).expect("popup");
        assert_eq!(
            p.get(b"Open").and_then(lopdf::Object::as_bool).ok(),
            Some(true)
        );
        let r: Vec<f32> = p
            .get(b"Rect")
            .and_then(lopdf::Object::as_array)
            .expect("rect")
            .iter()
            .filter_map(|v| v.as_float().ok())
            .collect();
        assert_eq!(r, vec![120.0, 600.0, 300.0, 720.0]);
    }
}
