//! Pop-up notes of comments (ISO 32000-1 §12.5.6.14 Popup annotations): open or close them all
//! (`/Open`), and put one back at its default place beside its comment (BlueReader round 9,
//! the comment menu's Open All Popups / Close All Popups / Reset Popup Note Location).
use crate::interp::{apply, invert};
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

    /// The pop-up notes of every comment that has one (BlueReader round 10: drawn on the page
    /// while open), in page order.
    pub fn popup_notes(&self) -> Vec<PopupNote> {
        let mut out = Vec::new();
        for comment in self.annotations() {
            let Some(id) = comment.id else { continue };
            let Some(note) = self.popup_of(id, comment) else {
                continue;
            };
            out.push(note);
        }
        out
    }

    /// The pop-up note of comment `a`, if it has one.
    pub fn popup_note(&self, a: &Annot) -> Option<PopupNote> {
        self.popup_of(a.id?, a.clone())
    }

    fn popup_of(&self, id: ObjectId, comment: Annot) -> Option<PopupNote> {
        let d = self.doc.get_dictionary(id).ok()?;
        let pid = d.get(b"Popup").ok()?.as_reference().ok()?;
        let p = self.doc.get_dictionary(pid).ok()?;
        let info = self.pages.get(comment.page)?;
        let base = crate::page_matrix(info.crop, info.rotate);
        let rect = crate::annot_rect(&self.doc, p).map(|r| crate::device_rect(base, r))?;
        let open = p
            .get(b"Open")
            .ok()
            .and_then(|o| o.as_bool().ok())
            .unwrap_or(false);
        Some(PopupNote {
            comment,
            rect,
            open,
            id: (pid.0, pid.1),
        })
    }

    /// Move / resize pop-up note `n` to `rect` (page points: left, top, right, bottom) and
    /// open or close it (§12.5.6.14 `/Rect`, `/Open`), as one undo step.
    pub fn set_popup_note(
        &mut self,
        n: &PopupNote,
        rect: [f32; 4],
        open: bool,
    ) -> Result<(), String> {
        let id: ObjectId = n.id;
        let info = self
            .pages
            .get(n.comment.page)
            .ok_or("no such page")?
            .clone();
        let base = crate::page_matrix(info.crop, info.rotate);
        let inv = invert(base).ok_or("page matrix")?;
        let (x0, y0) = apply(inv, rect[0], rect[1]);
        let (x1, y1) = apply(inv, rect[2], rect[3]);
        self.doc.get_dictionary(id).map_err(|e| e.to_string())?;
        self.begin(if open {
            "pop-up note"
        } else {
            "close pop-up note"
        });
        self.touch(id);
        if let Ok(p) = self.doc.get_dictionary_mut(id) {
            p.set(
                "Rect",
                vec![
                    Object::Real(x0.min(x1)),
                    Object::Real(y0.min(y1)),
                    Object::Real(x0.max(x1)),
                    Object::Real(y0.max(y1)),
                ],
            );
            p.set("Open", open);
        }
        self.commit();
        Ok(())
    }

    /// Give comment `a` a pop-up note (closed or open) beside it when it has none: its top
    /// left at the comment's top right corner, the default size (§12.5.6.14: `/Parent` and
    /// the comment's `/Popup`, listed in the page's `/Annots`).
    pub fn add_popup_note(&mut self, a: &Annot, open: bool) -> Result<PopupNote, String> {
        if let Some(n) = self.popup_note(a) {
            return Ok(n);
        }
        let id = a.id.ok_or("the comment cannot be changed")?;
        let info = self.pages.get(a.page).ok_or("no such page")?.clone();
        let base = crate::page_matrix(info.crop, info.rotate);
        let inv = invert(base).ok_or("page matrix")?;
        let (right, top) = (a.rect[2], a.rect[1]);
        let (x0, y0) = apply(inv, right, top);
        let (x1, y1) = apply(inv, right + POPUP_W, top + POPUP_H);
        self.begin("pop-up note");
        let mut d = lopdf::Dictionary::new();
        d.set("Type", Object::Name(b"Annot".to_vec()));
        d.set("Subtype", Object::Name(b"Popup".to_vec()));
        d.set(
            "Rect",
            vec![
                Object::Real(x0.min(x1)),
                Object::Real(y0.min(y1)),
                Object::Real(x0.max(x1)),
                Object::Real(y0.max(y1)),
            ],
        );
        d.set("Parent", Object::Reference(id));
        d.set("Open", open);
        d.set("P", Object::Reference(info.id));
        let pid = self.add(Object::Dictionary(d));
        self.touch(id);
        if let Ok(c) = self.doc.get_dictionary_mut(id) {
            c.set("Popup", Object::Reference(pid));
        }
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
                v.push(Object::Reference(pid));
                linked = true;
            }
        }
        if !linked {
            self.touch(info.id);
            if let Ok(p) = self.doc.get_dictionary_mut(info.id) {
                match p.get_mut(b"Annots") {
                    Ok(Object::Array(v)) => v.push(Object::Reference(pid)),
                    _ => p.set("Annots", vec![Object::Reference(pid)]),
                }
            }
        }
        self.commit();
        self.popup_note(a)
            .ok_or_else(|| "no pop-up note".to_owned())
    }
}

/// A comment's pop-up note (ISO 32000-1 §12.5.6.14).
#[derive(Debug, Clone, PartialEq)]
pub struct PopupNote {
    /// The comment it belongs to (`/Parent`).
    pub comment: Annot,
    /// Its rectangle in page points (left, top, right, bottom, y down as [`Annot::rect`]).
    pub rect: [f32; 4],
    /// Shown (`/Open`).
    pub open: bool,
    /// The pop-up annotation object (object number, generation).
    pub id: (u32, u16),
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

    #[test]
    fn popup_notes_move_close_and_add() {
        let mut d = PdfDocument::open(pdf(), crate::standard_fonts()).expect("pdf");
        let notes = d.popup_notes();
        assert_eq!(notes.len(), 1);
        let n = &notes[0];
        // Page points, y down: user y 300…400 on a 792 pt page is 392…492.
        assert_eq!(n.rect, [300.0, 392.0, 400.0, 492.0]);
        assert!(!n.open);
        assert_eq!(n.comment.contents, "Hi");
        d.set_popup_note(n, [310.0, 100.0, 490.0, 220.0], true)
            .expect("move");
        let n = d.popup_notes().remove(0);
        assert_eq!(n.rect, [310.0, 100.0, 490.0, 220.0]);
        assert!(n.open);
        assert!(d.undo());
        assert!(!d.popup_notes()[0].open);
        // A highlight without a note gets one beside it.
        d.add_annotation(&crate::NewAnnot {
            kind: crate::AnnotKind::Highlight,
            page: 0,
            rects: vec![[72.0, 300.0, 200.0, 312.0]],
            color: [255, 230, 0],
            author: "me".into(),
            contents: "x".into(),
        })
        .expect("highlight");
        let h = d
            .annotations()
            .into_iter()
            .find(|a| a.kind == crate::AnnotKind::Highlight)
            .expect("highlight");
        assert!(d.popup_note(&h).is_none());
        let n = d.add_popup_note(&h, true).expect("popup");
        assert!(n.open);
        assert_eq!(n.rect, [200.0, 300.0, 380.0, 420.0]);
        assert_eq!(d.popup_notes().len(), 2);
        let saved = d.save().expect("save");
        let again = PdfDocument::open(saved, crate::standard_fonts()).expect("reopen");
        assert_eq!(again.popup_notes().len(), 2);
    }
}
