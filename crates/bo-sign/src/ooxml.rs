//! OOXML package digital signatures (ECMA-376 Part 2, §12 "Digital
//! Signatures", W3C XML-Signature Syntax and Processing).
//!
//! Layout written into the package (§12.2):
//! * `/_xmlsignatures/origin.sigs` — Digital Signature Origin part, target
//!   of a package relationship of type `…/digital-signature/origin`;
//! * `/_xmlsignatures/sigN.xml` — XML Signature part, target of an origin
//!   relationship of type `…/digital-signature/signature`.
//!
//! Each signature references one `Object` (`#idPackageObject`) holding a
//! `Manifest` of every part (SHA-256 of its bytes) and every relationships
//! part through the Relationship Transform (§12.2.4.24) followed by C14N,
//! plus the `SignatureTime` property (§12.2.4.20).
//!
//! The XML is emitted directly in Canonical XML 1.0 form (no insignificant
//! whitespace, expanded empty elements, sorted attributes), so the
//! canonicalisation of `SignedInfo`/`Object` only adds the inherited default
//! namespace declaration.

use crate::{sha256, verify_with_cert, Result, SignError, Signer};
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine as _;
use bo_opc::{rels_part_name, Limits, Package, Relationship, TargetMode};
use std::fmt::Write as _;
use std::time::{SystemTime, UNIX_EPOCH};

const DSIG_NS: &str = "http://www.w3.org/2000/09/xmldsig#";
const MDSSI_NS: &str = "http://schemas.openxmlformats.org/package/2006/digital-signature";
const C14N: &str = "http://www.w3.org/TR/2001/REC-xml-c14n-20010315";
const RSA_SHA256: &str = "http://www.w3.org/2001/04/xmldsig-more#rsa-sha256";
const SHA256: &str = "http://www.w3.org/2001/04/xmlenc#sha256";
const REL_TRANSFORM: &str = "http://schemas.openxmlformats.org/package/2006/RelationshipTransform";
const RELS_NS: &str = "http://schemas.openxmlformats.org/package/2006/relationships";
/// §12.2.3 relationship types and content types.
const REL_ORIGIN: &str =
    "http://schemas.openxmlformats.org/package/2006/relationships/digital-signature/origin";
const REL_SIGNATURE: &str =
    "http://schemas.openxmlformats.org/package/2006/relationships/digital-signature/signature";
const CT_ORIGIN: &str = "application/vnd.openxmlformats-package.digital-signature-origin";
const CT_SIGNATURE: &str =
    "application/vnd.openxmlformats-package.digital-signature-xmlsignature+xml";
const ORIGIN_PART: &str = "/_xmlsignatures/origin.sigs";

/// Escapes text/attribute content as Canonical XML 1.0 requires (§2.3 of C14N).
fn c14n_escape(s: &str, attr: bool) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' if !attr => out.push_str("&gt;"),
            '"' if attr => out.push_str("&quot;"),
            '\t' if attr => out.push_str("&#x9;"),
            '\n' if attr => out.push_str("&#xA;"),
            '\r' => out.push_str("&#xD;"),
            c => out.push(c),
        }
    }
    out
}

fn is_signature_part(name: &str) -> bool {
    name.to_ascii_lowercase().starts_with("/_xmlsignatures/")
}

fn is_rels_part(name: &str) -> bool {
    name.to_ascii_lowercase().ends_with(".rels")
}

/// `/word/_rels/document.xml.rels` → `/word/document.xml`; `/_rels/.rels` → `/`.
fn source_of_rels(name: &str) -> Option<String> {
    let (dir, file) = name.rsplit_once("/_rels/")?;
    let file = file.strip_suffix(".rels")?;
    Some(if file.is_empty() {
        "/".to_owned()
    } else {
        format!("{dir}/{file}")
    })
}

/// Output of the Relationship Transform (§12.2.4.24) + C14N for the given ids.
fn transformed_rels(rels: &[Relationship], ids: &[String]) -> String {
    let mut sel: Vec<&Relationship> = rels.iter().filter(|r| ids.contains(&r.id)).collect();
    sel.sort_by(|a, b| a.id.cmp(&b.id));
    let mut s = format!("<Relationships xmlns=\"{RELS_NS}\">");
    for r in sel {
        let mode = match r.mode {
            TargetMode::Internal => "Internal",
            TargetMode::External => "External",
        };
        let _ = write!(
            s,
            "<Relationship Id=\"{}\" Target=\"{}\" TargetMode=\"{mode}\" Type=\"{}\"></Relationship>",
            c14n_escape(&r.id, true),
            c14n_escape(&r.target, true),
            c14n_escape(&r.rel_type, true)
        );
    }
    s.push_str("</Relationships>");
    s
}

fn digest_b64(data: &[u8]) -> String {
    B64.encode(sha256(data))
}

fn reference(uri: &str, transforms: &str, digest: &str) -> String {
    format!(
        "<Reference URI=\"{}\">{transforms}<DigestMethod Algorithm=\"{SHA256}\"></DigestMethod><DigestValue>{digest}</DigestValue></Reference>",
        c14n_escape(uri, true)
    )
}

/// Builds the `Manifest` entries for the current package contents.
fn manifest(pkg: &Package) -> Result<String> {
    let ct = pkg.content_types()?;
    let mut parts: Vec<&str> = pkg
        .parts()
        .iter()
        .map(|p| p.name.as_str())
        .filter(|n| !n.eq_ignore_ascii_case("/[Content_Types].xml") && !is_signature_part(n))
        .collect();
    parts.sort_unstable();
    let mut out = String::new();
    for name in parts {
        let content_type = ct.get(name).unwrap_or("application/octet-stream");
        let uri = format!("{name}?ContentType={content_type}");
        if is_rels_part(name) {
            let source = source_of_rels(name)
                .ok_or_else(|| SignError::Malformed(format!("relationships part name {name}")))?;
            let rels = pkg.relationships(&source)?;
            let ids: Vec<String> = rels
                .iter()
                .filter(|r| r.rel_type != REL_ORIGIN)
                .map(|r| r.id.clone())
                .collect();
            let mut transforms = format!("<Transforms><Transform Algorithm=\"{REL_TRANSFORM}\">");
            let mut sorted = ids.clone();
            sorted.sort();
            for id in &sorted {
                let _ = write!(
                    transforms,
                    "<mdssi:RelationshipReference xmlns:mdssi=\"{MDSSI_NS}\" SourceId=\"{}\"></mdssi:RelationshipReference>",
                    c14n_escape(id, true)
                );
            }
            let _ = write!(
                transforms,
                "</Transform><Transform Algorithm=\"{C14N}\"></Transform></Transforms>"
            );
            let digest = digest_b64(transformed_rels(&rels, &ids).as_bytes());
            out.push_str(&reference(&uri, &transforms, &digest));
        } else if let Some(p) = pkg.part(name) {
            out.push_str(&reference(&uri, "", &digest_b64(&p.data)));
        }
    }
    Ok(out)
}

/// `YYYY-MM-DDThh:mm:ssZ` (W3C date-time, §12.2.4.20 `Format`).
fn w3c_time(t: SystemTime) -> String {
    let secs = t.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let d = crate::pdf::civil_from_unix(secs);
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        d.0, d.1, d.2, d.3, d.4, d.5
    )
}

/// Inserts the in-scope default namespace into a top-level element so the
/// subset is in canonical form (C14N §2.4 namespace axis).
fn with_dsig_ns(element: &str, local: &str) -> String {
    element.replacen(
        &format!("<{local}"),
        &format!("<{local} xmlns=\"{DSIG_NS}\""),
        1,
    )
}

/// Signs an OOXML package (bytes of a `.docx/.xlsx/.pptx`), returning the new package.
pub fn sign_ooxml(bytes: &[u8], signer: &dyn Signer, time: SystemTime) -> Result<Vec<u8>> {
    let mut pkg = Package::read(bytes, &Limits::default())?;

    // Origin part + package relationship (§12.2.3).
    let mut root_rels = pkg.relationships("/")?;
    if !root_rels.iter().any(|r| r.rel_type == REL_ORIGIN) {
        let id = fresh_id(&root_rels);
        root_rels.push(Relationship {
            id,
            rel_type: REL_ORIGIN.into(),
            target: "_xmlsignatures/origin.sigs".into(),
            mode: TargetMode::Internal,
        });
        pkg.set_relationships("/", &root_rels);
    }
    if pkg.part(ORIGIN_PART).is_none() {
        pkg.add_part(ORIGIN_PART, CT_ORIGIN, Vec::new())?;
    }
    let mut origin_rels = pkg.relationships(ORIGIN_PART)?;
    let n = (1..=u32::MAX)
        .find(|k| pkg.part(&format!("/_xmlsignatures/sig{k}.xml")).is_none())
        .unwrap_or(1);
    let sig_part = format!("/_xmlsignatures/sig{n}.xml");

    let object = format!(
        "<Object Id=\"idPackageObject\"><Manifest>{}</Manifest><SignatureProperties><SignatureProperty Id=\"idSignatureTime\" Target=\"#idPackageSignature\"><mdssi:SignatureTime xmlns:mdssi=\"{MDSSI_NS}\"><mdssi:Format>YYYY-MM-DDThh:mm:ssTZD</mdssi:Format><mdssi:Value>{}</mdssi:Value></mdssi:SignatureTime></SignatureProperty></SignatureProperties></Object>",
        manifest(&pkg)?,
        w3c_time(time)
    );
    let signed_info = format!(
        "<SignedInfo><CanonicalizationMethod Algorithm=\"{C14N}\"></CanonicalizationMethod><SignatureMethod Algorithm=\"{RSA_SHA256}\"></SignatureMethod><Reference Type=\"{DSIG_NS}Object\" URI=\"#idPackageObject\"><DigestMethod Algorithm=\"{SHA256}\"></DigestMethod><DigestValue>{}</DigestValue></Reference></SignedInfo>",
        digest_b64(with_dsig_ns(&object, "Object").as_bytes())
    );
    let sig_value =
        signer.sign_sha256(&sha256(with_dsig_ns(&signed_info, "SignedInfo").as_bytes()))?;
    let mut certs = format!(
        "<X509Certificate>{}</X509Certificate>",
        B64.encode(signer.certificate_der())
    );
    for c in signer.chain_der() {
        let _ = write!(
            certs,
            "<X509Certificate>{}</X509Certificate>",
            B64.encode(c)
        );
    }
    let xml = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><Signature xmlns=\"{DSIG_NS}\" Id=\"idPackageSignature\">{signed_info}<SignatureValue>{}</SignatureValue><KeyInfo><X509Data>{certs}</X509Data></KeyInfo>{object}</Signature>",
        B64.encode(sig_value)
    );
    pkg.add_part(&sig_part, CT_SIGNATURE, xml.into_bytes())?;
    let id = fresh_id(&origin_rels);
    origin_rels.push(Relationship {
        id,
        rel_type: REL_SIGNATURE.into(),
        target: format!("sig{n}.xml"),
        mode: TargetMode::Internal,
    });
    pkg.set_relationships(ORIGIN_PART, &origin_rels);
    Ok(pkg.write()?)
}

fn fresh_id(rels: &[Relationship]) -> String {
    (1..=u32::MAX)
        .map(|k| format!("rIdSig{k}"))
        .find(|id| rels.iter().all(|r| &r.id != id))
        .unwrap_or_else(|| "rIdSig".into())
}

fn between<'a>(s: &'a str, open: &str, close: &str) -> Option<&'a str> {
    let a = s.find(open)?;
    let b = s[a..].find(close)? + a;
    Some(&s[a..b + close.len()])
}

fn inner<'a>(s: &'a str, tag: &str) -> Option<&'a str> {
    let open = format!("<{tag}>");
    let a = s.find(&open)? + open.len();
    let b = s[a..].find(&format!("</{tag}>"))? + a;
    Some(&s[a..b])
}

/// A verified package signature.
#[derive(Debug, Clone)]
pub struct OoxmlSignature {
    /// Name of the signature part.
    pub part: String,
    /// DER-encoded X.509 signer certificate.
    pub certificate_der: Vec<u8>,
    /// Claimed signing time (`mdssi:SignatureTime` value) verbatim, if present.
    pub signing_time: Option<String>,
}

/// Verifies the signatures of a package produced by [`sign_ooxml`]: the
/// `SignedInfo` RSA signature, the `Object` digest and every manifest
/// reference against the current package contents. Signatures written by
/// other producers with a different serialisation are reported as
/// [`SignError::Unsupported`].
pub fn verify_ooxml(bytes: &[u8]) -> Result<Vec<OoxmlSignature>> {
    let pkg = Package::read(bytes, &Limits::default())?;
    if pkg.part(ORIGIN_PART).is_none() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for rel in pkg.relationships(ORIGIN_PART)? {
        if rel.rel_type != REL_SIGNATURE {
            continue;
        }
        let name = bo_opc::resolve_target(ORIGIN_PART, &rel.target);
        let part = pkg
            .part(&name)
            .ok_or_else(|| SignError::Malformed(format!("missing {name}")))?;
        let xml = std::str::from_utf8(&part.data)
            .map_err(|_| SignError::Malformed("signature part is not UTF-8".into()))?;
        let unsupported = || SignError::Unsupported(format!("{name}: unrecognised serialisation"));
        let signed_info = between(xml, "<SignedInfo>", "</SignedInfo>").ok_or_else(unsupported)?;
        let object =
            between(xml, "<Object Id=\"idPackageObject\">", "</Object>").ok_or_else(unsupported)?;
        let sig_value = B64
            .decode(inner(xml, "SignatureValue").ok_or_else(unsupported)?)
            .map_err(|e| SignError::Malformed(e.to_string()))?;
        let cert = B64
            .decode(inner(xml, "X509Certificate").ok_or_else(unsupported)?)
            .map_err(|e| SignError::Malformed(e.to_string()))?;
        verify_with_cert(
            &cert,
            &sha256(with_dsig_ns(signed_info, "SignedInfo").as_bytes()),
            &sig_value,
        )?;
        let object_digest = inner(signed_info, "DigestValue").ok_or_else(unsupported)?;
        if object_digest != digest_b64(with_dsig_ns(object, "Object").as_bytes()) {
            return Err(SignError::Invalid(format!(
                "{name}: Object digest mismatch"
            )));
        }
        // Every manifest reference must match the package now.
        let current = manifest(&pkg)?;
        let signed_manifest = inner(object, "Manifest").ok_or_else(unsupported)?;
        for r in signed_manifest.split("<Reference ").skip(1) {
            let full = format!("<Reference {r}");
            if !current.contains(&full) {
                let uri = r.split('"').nth(1).unwrap_or("?");
                return Err(SignError::Invalid(format!("{name}: {uri} was modified")));
            }
        }
        out.push(OoxmlSignature {
            part: name,
            certificate_der: cert,
            signing_time: inner(object, "mdssi:Value").map(str::to_owned),
        });
    }
    Ok(out)
}

/// Name of the relationships part of a source part (re-export for callers).
#[must_use]
pub fn rels_of(source: &str) -> String {
    rels_part_name(source)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rels_source_names() {
        assert_eq!(source_of_rels("/_rels/.rels").as_deref(), Some("/"));
        assert_eq!(
            source_of_rels("/word/_rels/document.xml.rels").as_deref(),
            Some("/word/document.xml")
        );
        assert_eq!(source_of_rels("/word/document.xml"), None);
    }

    #[test]
    fn relationship_transform_sorts_and_defaults_mode() {
        let rels = vec![
            Relationship {
                id: "rId2".into(),
                rel_type: "t2".into(),
                target: "b&c.xml".into(),
                mode: TargetMode::External,
            },
            Relationship {
                id: "rId1".into(),
                rel_type: "t1".into(),
                target: "a.xml".into(),
                mode: TargetMode::Internal,
            },
        ];
        let out = transformed_rels(&rels, &["rId1".into(), "rId2".into()]);
        assert_eq!(
            out,
            format!("<Relationships xmlns=\"{RELS_NS}\"><Relationship Id=\"rId1\" Target=\"a.xml\" TargetMode=\"Internal\" Type=\"t1\"></Relationship><Relationship Id=\"rId2\" Target=\"b&amp;c.xml\" TargetMode=\"External\" Type=\"t2\"></Relationship></Relationships>")
        );
        assert!(!transformed_rels(&rels, &["rId1".into()]).contains("rId2"));
    }
}
