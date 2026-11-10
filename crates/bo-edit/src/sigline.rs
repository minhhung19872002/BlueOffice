//! Signature lines (CORE-SIGLINE-001): insertion of an inline VML picture shape
//! carrying `o:signatureline` (ECMA-376 Part 4, §19.2.2.30), the list of the
//! document's lines, and the verification state shown on them (display only).

use crate::{para, EditError, Op, Result, Selection, Session};
use bo_model::index::{body_paragraphs, para_at, para_at_mut, para_index};
use bo_model::{Emu, Image, RunContent, Shape, ShapeArt, SignatureLine};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// What the "signature line" dialog asks for.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SignatureLineSpec {
    /// Suggested signer (name).
    pub suggested_signer: String,
    /// Suggested signer's title.
    pub suggested_signer2: String,
    /// Suggested signer's e-mail address.
    pub suggested_signer_email: String,
    /// Instructions shown to the signer.
    pub signing_instructions: String,
    /// The signer may add a comment.
    pub allow_comments: bool,
    /// The signed line shows the signing date.
    pub show_sign_date: bool,
}

/// A signature line of the body, for hosts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SignatureLineInfo {
    /// Body paragraph id of the line.
    pub para: usize,
    /// Index-text offset of the line in that paragraph.
    pub at: usize,
    /// The line's attributes and signed state.
    pub line: SignatureLine,
}

fn line_of(c: &RunContent) -> Option<&SignatureLine> {
    match c {
        RunContent::Image(img) => img.shape.as_ref()?.art.as_ref()?.signature_line.as_deref(),
        _ => None,
    }
}

/// The signature line model for `spec` (`id`: a braced GUID).
#[must_use]
pub fn signature_line(spec: &SignatureLineSpec, id: &str) -> SignatureLine {
    SignatureLine {
        id: id.to_owned(),
        provider_id: "{00000000-0000-0000-0000-000000000000}".into(),
        is_signature_line: true,
        suggested_signer: spec.suggested_signer.trim().to_owned(),
        suggested_signer2: spec.suggested_signer2.trim().to_owned(),
        suggested_signer_email: spec.suggested_signer_email.trim().to_owned(),
        signing_instructions: spec.signing_instructions.trim().to_owned(),
        allow_comments: spec.allow_comments,
        show_sign_date: spec.show_sign_date,
        signed: None,
    }
}

/// The inline picture shape of a signature line whose picture is media `media`,
/// `width` × `height` EMU.
#[must_use]
pub fn signature_line_image(line: SignatureLine, media: &str, width: i64, height: i64) -> Image {
    Image {
        media: String::new(),
        width: Emu(width.max(12_700)),
        height: Emu(height.max(12_700)),
        anchor: None,
        crop: None,
        rotation: 0,
        description: String::new(),
        format: None,
        shape: Some(Box::new(Shape {
            preset: "rect".into(),
            vml: true,
            art: Some(Box::new(ShapeArt {
                picture: Some(media.to_owned()),
                signature_line: Some(Box::new(line)),
                ..ShapeArt::default()
            })),
            ..Shape::default()
        })),
        src: None,
        ole: None,
    }
}

impl Session {
    /// Insert signature line `line` at the caret (replacing the selection) as an
    /// inline shape `width` × `height` EMU whose picture is `png` (shown by
    /// applications that draw the stored picture). Returns the paragraph id and
    /// offset of the new line.
    pub fn insert_signature_line(
        &mut self,
        line: SignatureLine,
        png: Vec<u8>,
        width: i64,
        height: i64,
    ) -> Result<(usize, usize)> {
        if self.signature_lines().iter().any(|l| l.line.id == line.id) {
            return Err(EditError::Unsupported(format!(
                "signature line {} already exists",
                line.id
            )));
        }
        let mut n = self.part_number_start(self.doc.media.len() + 1);
        let name = loop {
            let cand = format!("/word/media/image{n}.png");
            if !self.doc.media.contains_key(&cand) {
                break cand;
            }
            n += 1;
        };
        let bytes = Arc::new(png);
        let img = signature_line_image(line, &name, width, height);
        let mut placed = (0, 0);
        self.edit(|tx, sel| {
            let (a, b) = sel.ordered();
            let at = tx.delete_range(a, b)?;
            tx.push(Op::Media {
                name: name.clone(),
                old: None,
                new: Some(bytes),
            })?;
            let (_, _, atoms) = tx.atoms(at.para)?;
            let props = para::props_at(&atoms, at.offset);
            placed = (at.para, at.offset);
            let caret = tx.insert_atoms(at, vec![(props, RunContent::Image(Box::new(img)))])?;
            Ok(Selection::caret(caret))
        })?;
        Ok(placed)
    }

    /// Every signature line of the body (tables included), in document order.
    #[must_use]
    pub fn signature_lines(&self) -> Vec<SignatureLineInfo> {
        let mut out = Vec::new();
        for (id, path) in body_paragraphs(&self.doc).iter().enumerate() {
            let Some(p) = para_at(&self.doc, path) else {
                continue;
            };
            let (_, spans) = para_index(p);
            for s in spans {
                if let Some(l) = line_of(&p.runs[s.run].content[s.item]) {
                    out.push(SignatureLineInfo {
                        para: id,
                        at: s.start,
                        line: l.clone(),
                    });
                }
            }
        }
        out
    }

    /// Show the verification result of each signed line (`(id, valid)`; lines not
    /// listed show no result). Display only: no undo step, nothing to save.
    /// Returns whether a line changed.
    pub fn set_signature_line_status(&mut self, status: &[(String, Option<bool>)]) -> bool {
        let mut changed = false;
        for path in body_paragraphs(&self.doc) {
            let Some(p) = para_at_mut(&mut self.doc, &path) else {
                continue;
            };
            let mut hit = false;
            for c in p.runs.iter_mut().flat_map(|r| r.content.iter_mut()) {
                let RunContent::Image(img) = c else { continue };
                let Some(l) = img
                    .shape
                    .as_mut()
                    .and_then(|s| s.art.as_mut())
                    .and_then(|a| a.signature_line.as_mut())
                else {
                    continue;
                };
                let Some(signed) = l.signed.as_mut() else {
                    continue;
                };
                let v = status
                    .iter()
                    .find(|(id, _)| id.eq_ignore_ascii_case(&l.id))
                    .and_then(|(_, v)| *v);
                if signed.valid != v {
                    signed.valid = v;
                    hit = true;
                }
            }
            if hit {
                changed = true;
                // Not an op: mark the top-level block for the incremental layout.
                if let [si, bi, ..] = path[..] {
                    self.changes.splice(si, bi, 1, 1);
                }
            }
        }
        if changed {
            self.version += 1;
        }
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Pos;
    use bo_model::{Block, Document, ParaProps, Paragraph, Run, RunProps, Section, SignedLine};

    fn session() -> Session {
        let doc = Document {
            sections: vec![Section::new(vec![Block::Paragraph(Paragraph::new(
                ParaProps::default(),
                vec![Run::text("Ký tên:", RunProps::default())],
            ))])],
            ..Document::default()
        };
        Session::new(doc)
    }

    #[test]
    fn inserts_two_lines_and_lists_them() {
        let mut s = session();
        // "Ký tên:" is 9 bytes of UTF-8 (ý is 2 bytes, ê is 2 bytes).
        s.set_selection(Selection::caret(Pos::new(0, 9)));
        let spec = SignatureLineSpec {
            suggested_signer: " Nguyễn Văn A ".into(),
            suggested_signer2: "Giám đốc".into(),
            allow_comments: true,
            show_sign_date: true,
            ..SignatureLineSpec::default()
        };
        let a = signature_line(&spec, "{00000000-0000-4000-8000-000000000001}");
        assert_eq!(a.suggested_signer, "Nguyễn Văn A");
        let placed = s
            .insert_signature_line(a, vec![1, 2, 3], 2_438_400, 1_219_200)
            .unwrap();
        assert_eq!(placed, (0, 9));
        let b = signature_line(
            &SignatureLineSpec::default(),
            "{00000000-0000-4000-8000-000000000002}",
        );
        s.insert_signature_line(b.clone(), vec![4], 2_438_400, 1_219_200)
            .unwrap();
        // The same id cannot be inserted twice.
        assert!(s
            .insert_signature_line(b, vec![4], 2_438_400, 1_219_200)
            .is_err());
        let lines = s.signature_lines();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].at, 9);
        assert_eq!(lines[0].line.suggested_signer2, "Giám đốc");
        assert!(lines[0].line.allow_comments);
        assert_eq!(s.doc().media.len(), 2);
        // One undo removes the second line and its picture.
        assert!(s.undo());
        assert_eq!(s.signature_lines().len(), 1);
        assert_eq!(s.doc().media.len(), 1);
    }

    #[test]
    fn status_is_display_only() {
        let mut s = session();
        s.set_selection(Selection::caret(Pos::new(0, 0)));
        let mut l = signature_line(&SignatureLineSpec::default(), "{AB}");
        l.signed = Some(SignedLine {
            text: "A".into(),
            ..SignedLine::default()
        });
        s.insert_signature_line(l, vec![1], 1_000_000, 500_000)
            .unwrap();
        let undo_before = s.can_undo();
        assert!(s.set_signature_line_status(&[("{ab}".into(), Some(false))]));
        let line = &s.signature_lines()[0].line;
        assert_eq!(line.signed.as_ref().unwrap().valid, Some(false));
        // Same state again: nothing changes.
        assert!(!s.set_signature_line_status(&[("{AB}".into(), Some(false))]));
        assert_eq!(s.can_undo(), undo_before);
    }
}
