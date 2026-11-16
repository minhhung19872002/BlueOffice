//! XFA forms (ISO 32000-1 §12.7.8, `/AcroForm /XFA`). A static XFA form also carries ordinary
//! AcroForm fields with appearances that show the same form; dropping the XFA packets leaves a
//! plain AcroForm document every viewer fills the same way (BlueEditor's Form › XFA). A dynamic
//! form (no AcroForm fields, `/NeedsRendering`) has nothing to fall back to and is left alone.
use crate::PdfDocument;
use lopdf::Object;

/// What kind of XFA a document has.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XfaKind {
    /// No XFA.
    None,
    /// XFA with AcroForm fields beside it (can become a plain AcroForm form).
    Static,
    /// XFA only (drawn from the XFA template; not offered).
    Dynamic,
}

impl PdfDocument {
    /// The XFA of the document.
    #[must_use]
    pub fn xfa_kind(&self) -> XfaKind {
        let Some(c) = self.catalog_dict() else {
            return XfaKind::None;
        };
        let needs = c
            .get(b"NeedsRendering")
            .ok()
            .and_then(|o| o.as_bool().ok())
            .unwrap_or(false);
        let form = c.get(b"AcroForm").ok().and_then(|o| match o {
            Object::Reference(r) => self.doc.get_dictionary(*r).ok(),
            Object::Dictionary(d) => Some(d),
            _ => None,
        });
        let Some(form) = form else {
            return XfaKind::None;
        };
        if form.get(b"XFA").is_err() {
            return XfaKind::None;
        }
        let fields = form
            .get(b"Fields")
            .ok()
            .and_then(|o| match o {
                Object::Reference(r) => self.doc.get_object(*r).ok(),
                o => Some(o),
            })
            .and_then(|o| o.as_array().ok())
            .is_some_and(|a| !a.is_empty());
        if fields && !needs {
            XfaKind::Static
        } else {
            XfaKind::Dynamic
        }
    }

    /// Turn a static XFA form into a plain AcroForm form (one undo step).
    pub fn drop_static_xfa(&mut self) -> Result<(), String> {
        match self.xfa_kind() {
            XfaKind::Static => {}
            XfaKind::None => return Err("the document has no XFA form".into()),
            XfaKind::Dynamic => {
                return Err("a dynamic XFA form has no AcroForm fields to keep".into())
            }
        }
        let root = self.catalog_root()?;
        let form = self
            .catalog_dict()
            .and_then(|c| c.get(b"AcroForm").ok())
            .cloned()
            .ok_or("the document has no form")?;
        self.begin("drop XFA");
        if let Object::Reference(f) = form {
            self.touch(f);
            if let Ok(d) = self.doc.get_dictionary_mut(f) {
                d.remove(b"XFA");
            }
        }
        self.touch(root);
        if let Ok(c) = self.doc.get_dictionary_mut(root) {
            c.remove(b"NeedsRendering");
            if let Ok(Object::Dictionary(d)) = c.get_mut(b"AcroForm") {
                d.remove(b"XFA");
            }
        }
        self.commit();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::standard_fonts;

    fn doc(form: &str) -> PdfDocument {
        let catalog = format!("<< /Type /Catalog /Pages 2 0 R /AcroForm {form} >>");
        let bytes = crate::tests::build_pdf(&[
            &catalog,
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 300] /Annots [4 0 R] >>",
            "<< /Type /Annot /Subtype /Widget /FT /Tx /T (name) /Rect [10 10 100 30] /P 3 0 R >>",
            "<< /Length 10 >>\nstream\n<xdp:xdp/>\nendstream",
        ]);
        PdfDocument::open(bytes, standard_fonts()).expect("opens")
    }

    #[test]
    fn static_xfa_becomes_a_plain_form() {
        let mut d = doc("<< /Fields [4 0 R] /XFA 5 0 R >>");
        assert_eq!(d.xfa_kind(), XfaKind::Static);
        d.drop_static_xfa().expect("drops");
        assert_eq!(d.xfa_kind(), XfaKind::None);
        let saved = d.save().expect("saves");
        let again = PdfDocument::open(saved, standard_fonts()).expect("opens");
        assert_eq!(again.xfa_kind(), XfaKind::None);
        assert!(d.undo());
        assert_eq!(d.xfa_kind(), XfaKind::Static);
    }

    #[test]
    fn dynamic_xfa_and_plain_forms_are_left_alone() {
        let mut d = doc("<< /Fields [] /XFA 5 0 R >>");
        assert_eq!(d.xfa_kind(), XfaKind::Dynamic);
        assert!(d.drop_static_xfa().is_err());
        let d = doc("<< /Fields [4 0 R] >>");
        assert_eq!(d.xfa_kind(), XfaKind::None);
    }
}
