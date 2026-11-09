//! Signed state of signature lines (CORE-SIGLINE-001).
//!
//! A signature line (ECMA-376 Part 4, §19.2.2.30 signatureline) is signed by an
//! OOXML package signature (ECMA-376 Part 2, §12) whose Office object
//! ([MS-OFFCRYPTO] "idOfficeObject Object Element") holds a `SignatureInfoV1`
//! with `SetupID` = the line's `id`, the typed `SignatureText` and/or the
//! `SignatureImage`. This module reads those signature parts (without checking
//! them cryptographically — that is `bo-sign`'s job) and records the signed state
//! on the matching lines for display.

use base64::Engine as _;
use bo_model::slide::XmlElement;
use bo_model::{Block, Document, RunContent, SignatureLine, SignedLine};
use bo_opc::Package;
use std::sync::Arc;

/// Signature part folder (ECMA-376 Part 2, §12.2.3).
const SIG_DIR: &str = "/_xmlsignatures/";

/// What one package signature says about the line it signs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineSignature {
    /// `SetupID`: the signed line's `id`.
    pub setup_id: String,
    /// `SignatureText`.
    pub text: String,
    /// `SignatureImage` bytes.
    pub image: Option<Vec<u8>>,
    /// `YYYY-MM-DD` of `mdssi:SignatureTime/mdssi:Value` (else XAdES `SigningTime`).
    pub date: String,
}

fn find<'a>(el: &'a XmlElement, local: &str) -> Option<&'a XmlElement> {
    if el.local() == local {
        return Some(el);
    }
    el.elements().find_map(|c| find(c, local))
}

/// Same GUID whatever the case of its hex digits and braces.
fn same_guid(a: &str, b: &str) -> bool {
    let norm = |s: &str| {
        s.trim()
            .trim_start_matches('{')
            .trim_end_matches('}')
            .to_ascii_lowercase()
    };
    !a.trim().is_empty() && norm(a) == norm(b)
}

/// The line signatures of a package (one per signature part with a non-empty
/// `SetupID`), in part-name order.
#[must_use]
pub fn line_signatures(pkg: &Package) -> Vec<LineSignature> {
    let mut names: Vec<&str> = pkg
        .parts()
        .iter()
        .map(|p| p.name.as_str())
        .filter(|n| {
            n.to_ascii_lowercase().starts_with(SIG_DIR) && n.to_ascii_lowercase().ends_with(".xml")
        })
        .collect();
    names.sort_unstable();
    names
        .into_iter()
        .filter_map(|n| pkg.part(n))
        .filter_map(|p| parse_signature(&p.data, &p.name))
        .collect()
}

/// `SignatureInfoV1` of one signature part ([MS-OFFCRYPTO] idOfficeObject).
#[must_use]
pub fn parse_signature(xml: &[u8], name: &str) -> Option<LineSignature> {
    let root = bo_drawingml::dom::parse(xml, name).ok()?;
    let info = find(&root, "SignatureInfoV1")?;
    let setup_id = info
        .elements()
        .find(|e| e.local() == "SetupID")
        .map(XmlElement::text)
        .unwrap_or_default();
    if setup_id.trim().is_empty() {
        return None;
    }
    let child = |k: &str| {
        info.elements()
            .find(|e| e.local() == k)
            .map(XmlElement::text)
            .unwrap_or_default()
    };
    let image = {
        let b64: String = child("SignatureImage")
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect();
        if b64.is_empty() {
            None
        } else {
            base64::engine::general_purpose::STANDARD.decode(b64).ok()
        }
    };
    let time = find(&root, "SignatureTime")
        .and_then(|t| t.elements().find(|e| e.local() == "Value"))
        .or_else(|| find(&root, "SigningTime"))
        .map(XmlElement::text)
        .unwrap_or_default();
    let date = time.trim().get(..10).unwrap_or_default().to_owned();
    Some(LineSignature {
        setup_id: setup_id.trim().to_owned(),
        text: child("SignatureText"),
        image,
        date,
    })
}

/// Every signature line of the body (tables and text boxes included), mutably.
pub fn for_each_line_mut(doc: &mut Document, f: &mut dyn FnMut(&mut SignatureLine)) {
    for sec in &mut doc.sections {
        lines_in_blocks(&mut sec.blocks, f);
    }
}

fn lines_in_blocks(blocks: &mut [Block], f: &mut dyn FnMut(&mut SignatureLine)) {
    for b in blocks {
        match b {
            Block::Paragraph(p) => {
                for r in &mut p.runs {
                    for c in &mut r.content {
                        let RunContent::Image(img) = c else { continue };
                        let Some(s) = img.shape.as_mut() else {
                            continue;
                        };
                        if let Some(l) = s.art.as_mut().and_then(|a| a.signature_line.as_mut()) {
                            f(l);
                        }
                        if let Some(t) = s.text.as_mut() {
                            lines_in_blocks(t, f);
                        }
                    }
                }
            }
            Block::Table(t) => {
                for row in &mut t.rows {
                    for cell in &mut row.cells {
                        lines_in_blocks(&mut cell.blocks, f);
                    }
                }
            }
            Block::Opaque(_) => {}
        }
    }
}

/// Records on each line of `doc` the signature (if any) that names it.
pub fn apply_signatures(doc: &mut Document, sigs: &[LineSignature]) {
    for_each_line_mut(doc, &mut |l| {
        l.signed = sigs
            .iter()
            .rev()
            .find(|s| same_guid(&s.setup_id, &l.id))
            .map(|s| SignedLine {
                text: s.text.clone(),
                image: s.image.clone().map(Arc::new),
                date: s.date.clone(),
                valid: None,
            });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_signature_info() {
        let xml = br##"<Signature xmlns="http://www.w3.org/2000/09/xmldsig#"><Object Id="idPackageObject"><SignatureProperties><SignatureProperty Id="idSignatureTime" Target="#idPackageSignature"><mdssi:SignatureTime xmlns:mdssi="http://schemas.openxmlformats.org/package/2006/digital-signature"><mdssi:Format>YYYY-MM-DDThh:mm:ssTZD</mdssi:Format><mdssi:Value>2026-03-04T05:06:07Z</mdssi:Value></mdssi:SignatureTime></SignatureProperty></SignatureProperties></Object><Object Id="idOfficeObject"><SignatureProperties><SignatureProperty Id="idOfficeV1Details" Target="#idPackageSignature"><SignatureInfoV1 xmlns="http://schemas.microsoft.com/office/2006/digsig"><SetupID>{ABC}</SetupID><SignatureText>Nguyen Van A</SignatureText><SignatureImage>AQID</SignatureImage></SignatureInfoV1></SignatureProperty></SignatureProperties></Object></Signature>"##;
        let s = parse_signature(xml, "/_xmlsignatures/sig1.xml").expect("info");
        assert_eq!(s.setup_id, "{ABC}");
        assert_eq!(s.text, "Nguyen Van A");
        // "AQID" is base64 of the bytes 1, 2, 3 (RFC 4648 §4).
        assert_eq!(s.image.as_deref(), Some(&[1u8, 2, 3][..]));
        assert_eq!(s.date, "2026-03-04");
        assert!(same_guid("{abc}", "{ABC}"));
        assert!(!same_guid("", ""));
    }
}
