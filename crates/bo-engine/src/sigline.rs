//! Signature lines in BlueDoc (CORE-SIGLINE-001; ECMA-376 Part 4, §19.2.2.30
//! signatureline): insertion, the list of lines, the pictures a signature of a
//! line carries ([MS-OFFCRYPTO] idValidSigLnImg / idInvalidSigLnImg) and the
//! verification result shown on signed lines.
//!
//! Signing itself is done by the host with `bo-sign` (ADR-0007) on the saved
//! package; the engine only provides what the signature needs and shows what the
//! package signatures say (`bo_docx::sigline`).

use crate::edit::{Command, EditResult};
use crate::{Engine, EngineError, Result};
use base64::Engine as _;
use bo_edit::sigline::{signature_line, signature_line_image, SignatureLineInfo};
use bo_model::{Image, SignatureLine, SignedLine};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// EMU per point.
const EMU_PT: f32 = 12_700.0;
/// Size of a new line (points): BlueOffice's own choice.
pub const LINE_WIDTH_PT: f32 = 192.0;
/// Height of a new line (points).
pub const LINE_HEIGHT_PT: f32 = 96.0;
/// Pixels per point of the line pictures.
const SCALE: f32 = 2.0;

/// The pictures of a line as signed and as invalid (PNG, base64).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SignatureLineImages {
    /// The line id.
    pub id: String,
    /// `idValidSigLnImg`.
    pub valid: String,
    /// `idInvalidSigLnImg`.
    pub invalid: String,
}

/// Verification result of one line (`SetSignatureLineStatus`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LineStatus {
    /// The line id.
    pub id: String,
    /// `None`: not verified.
    #[serde(default)]
    pub valid: Option<bool>,
}

/// A braced GUID derived from `seed` (shaped like an RFC 9562 version 4 UUID);
/// used when the host does not supply a random one.
fn guid_from(seed: u64) -> String {
    use std::hash::{Hash, Hasher};
    let mut h1 = std::collections::hash_map::DefaultHasher::new();
    (seed, 1u8).hash(&mut h1);
    let mut h2 = std::collections::hash_map::DefaultHasher::new();
    (seed, 2u8).hash(&mut h2);
    let (a, b) = (h1.finish(), h2.finish());
    format!(
        "{{{:08X}-{:04X}-4{:03X}-{:04X}-{:012X}}}",
        a >> 32,
        (a >> 16) & 0xFFFF,
        a & 0x0FFF,
        0x8000 | (b >> 48 & 0x3FFF),
        b & 0xFFFF_FFFF_FFFF
    )
}

/// Is `id` a braced GUID (`ST_Guid`)?
fn is_guid(id: &str) -> bool {
    let Some(inner) = id.strip_prefix('{').and_then(|s| s.strip_suffix('}')) else {
        return false;
    };
    let groups: Vec<&str> = inner.split('-').collect();
    groups.len() == 5
        && groups
            .iter()
            .zip([8, 4, 4, 4, 12])
            .all(|(g, n)| g.len() == n && g.chars().all(|c| c.is_ascii_hexdigit()))
}

impl Engine {
    /// PNG of `line` drawn in the box of `img`; `signature_image` is the signer's
    /// picture of a signed line.
    fn line_png(
        &mut self,
        img: &Image,
        line: &SignatureLine,
        signature_image: Option<&Arc<Vec<u8>>>,
    ) -> Result<Vec<u8>> {
        let page = bo_layout::signature_line_page(img, line, &mut self.fonts);
        let list = bo_render::build_page_list(&page, &self.fonts);
        let key = SignatureLine::image_key(&line.id);
        let mut images = std::collections::BTreeMap::new();
        if let Some(b) = signature_image {
            images.insert(key, b.as_ref().clone());
        }
        let media = bo_render::Media::new();
        Ok(bo_raster::render_page_png_with(
            &list,
            &self.fonts,
            SCALE,
            &bo_render::image::Chain(&media, &images),
        )?)
    }

    /// Signature pictures of the signed lines, for display (`sigline:<id>`).
    pub(crate) fn register_signature_images(&mut self) {
        let Some(s) = self.session.as_ref() else {
            return;
        };
        for l in s.signature_lines() {
            if let Some(b) = l.line.signed.as_ref().and_then(|x| x.image.as_ref()) {
                self.images
                    .insert(SignatureLine::image_key(&l.line.id), b.as_ref().clone());
            }
        }
    }

    /// Signature line commands (`None`: not one).
    pub(crate) fn sigline_command(&mut self, cmd: &Command) -> Result<Option<EditResult>> {
        match cmd {
            Command::InsertSignatureLine { spec, id } => {
                self.close_text_box()?;
                let s = self.session.as_ref().ok_or(EngineError::NoDocument)?;
                let existing = s.signature_lines();
                let id = match id {
                    Some(i) if is_guid(i) => i.to_ascii_uppercase(),
                    Some(i) => return Err(EngineError::BadCommand(format!("bad line id {i}"))),
                    None => {
                        let mut seed = s.version() ^ (existing.len() as u64) << 40;
                        loop {
                            let g = guid_from(seed);
                            if existing.iter().all(|l| !l.line.id.eq_ignore_ascii_case(&g)) {
                                break g;
                            }
                            seed = seed.wrapping_add(1);
                        }
                    }
                };
                let line = signature_line(spec, &id);
                let (w, h) = (
                    (LINE_WIDTH_PT * EMU_PT) as i64,
                    (LINE_HEIGHT_PT * EMU_PT) as i64,
                );
                // The stored picture: the unsigned line as drawn here.
                let img = signature_line_image(line.clone(), "", w, h);
                let png = self.line_png(&img, &line, None)?;
                let s = self.session.as_mut().ok_or(EngineError::NoDocument)?;
                let error = s
                    .insert_signature_line(line, png, w, h)
                    .err()
                    .map(|e| e.to_string());
                let dirty = self.relayout()?;
                let mut r = self.edit_result(dirty, (None, None, None), error)?;
                r.signature_lines = Some(self.signature_line_list());
                r.collab = self.collab_after_edit();
                Ok(Some(r))
            }
            Command::SignatureLines => {
                let mut r = self.edit_result(Vec::new(), (None, None, None), None)?;
                r.signature_lines = Some(self.signature_line_list());
                Ok(Some(r))
            }
            Command::SignatureLineImages {
                id,
                text,
                image_base64,
                date,
            } => {
                let s = self.session.as_ref().ok_or(EngineError::NoDocument)?;
                let found = s
                    .signature_lines()
                    .into_iter()
                    .find(|l| l.line.id.eq_ignore_ascii_case(id))
                    .and_then(|l| Some((s.drawing(l.para, l.at)?.clone(), l.line)));
                let Some((img, line)) = found else {
                    let r = self.edit_result(
                        Vec::new(),
                        (None, None, None),
                        Some(format!("no signature line {id}")),
                    )?;
                    return Ok(Some(r));
                };
                let picture = match image_base64.as_deref().filter(|b| !b.is_empty()) {
                    Some(b) => Some(Arc::new(
                        base64::engine::general_purpose::STANDARD
                            .decode(b.trim())
                            .map_err(|e| EngineError::BadCommand(e.to_string()))?,
                    )),
                    None => None,
                };
                let mut pngs = Vec::new();
                for valid in [true, false] {
                    let signed = SignatureLine {
                        signed: Some(SignedLine {
                            text: text.clone(),
                            image: picture.clone(),
                            date: date.clone(),
                            valid: Some(valid),
                        }),
                        ..line.clone()
                    };
                    let png = self.line_png(&img, &signed, picture.as_ref())?;
                    pngs.push(base64::engine::general_purpose::STANDARD.encode(png));
                }
                let mut r = self.edit_result(Vec::new(), (None, None, None), None)?;
                let invalid = pngs.pop().unwrap_or_default();
                let valid = pngs.pop().unwrap_or_default();
                r.signature_line_images = Some(SignatureLineImages {
                    id: line.id,
                    valid,
                    invalid,
                });
                Ok(Some(r))
            }
            Command::SetSignatureLineStatus { lines } => {
                let s = self.session.as_mut().ok_or(EngineError::NoDocument)?;
                let status: Vec<(String, Option<bool>)> =
                    lines.iter().map(|l| (l.id.clone(), l.valid)).collect();
                let dirty = if s.set_signature_line_status(&status) {
                    self.relayout()?
                } else {
                    Vec::new()
                };
                let mut r = self.edit_result(dirty, (None, None, None), None)?;
                r.signature_lines = Some(self.signature_line_list());
                Ok(Some(r))
            }
            _ => Ok(None),
        }
    }

    /// The document's signature lines.
    fn signature_line_list(&self) -> Vec<SignatureLineInfo> {
        self.session
            .as_ref()
            .map(bo_edit::Session::signature_lines)
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derived_ids_are_guids() {
        let g = guid_from(42);
        assert!(is_guid(&g), "{g}");
        // Version nibble 4 and variant bits 10xx (RFC 9562 §4.1, §4.2).
        assert_eq!(&g[15..16], "4");
        assert!(matches!(&g[20..21], "8" | "9" | "A" | "B"), "{g}");
        assert_ne!(guid_from(42), guid_from(43));
        assert!(!is_guid("{1234}"));
        assert!(is_guid("{6F1B2C3D-4E5F-4061-8273-94A5B6C7D8E9}"));
    }
}
