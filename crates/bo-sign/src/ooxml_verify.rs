//! Verification of OOXML package signatures written by any producer (ECMA-376 Part 2, §13 "Digital
//! signatures"; W3C XML-DSig 1.1; ETSI TS 101 903 XAdES): the signature part is parsed, `SignedInfo` and
//! every same-document reference are canonicalised with the algorithm they name ([`crate::c14n`]), the
//! package `Manifest` references are checked against the parts (with the §13.2.4.24 relationship
//! transform for relationships parts), and the XAdES signing-certificate binding and signature
//! time-stamp are checked when present.

use crate::c14n::{self, Element, Method};
use crate::timestamp::{self, TimestampInfo};
use crate::{HashAlg, KeyKind, Result, SignError};
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine as _;
use bo_opc::{Package, Relationship};

const DSIG_NS: &str = "http://www.w3.org/2000/09/xmldsig#";
const REL_TRANSFORM: &str = "http://schemas.openxmlformats.org/package/2006/RelationshipTransform";
const XADES_SIGNED_PROPERTIES: &str = "http://uri.etsi.org/01903#SignedProperties";

/// A verified package signature.
pub(crate) struct Verified {
    pub certificate_der: Vec<u8>,
    pub certificates: Vec<Vec<u8>>,
    pub signing_time: Option<String>,
    pub timestamp: Option<TimestampInfo>,
    /// Parts of the package the manifest does not cover (signature parts and `[Content_Types].xml` excluded).
    pub unsigned_parts: Vec<String>,
    /// Signed `SignatureInfoV1` with a `SetupID` ([MS-OFFCRYPTO] idOfficeObject).
    pub signature_line: Option<crate::ooxml::SignedLineInfo>,
}

/// Text of the first child `local` of `e`.
fn child_text(e: &Element, local: &str) -> String {
    e.elements()
        .find(|c| c.local() == local)
        .map(Element::text)
        .unwrap_or_default()
}

/// `SetupID` of a signature part, unverified (`None`: no signature line).
pub(crate) fn setup_id_of(xml: &[u8]) -> Option<String> {
    let root = c14n::parse(xml).ok()?;
    let info = c14n::find_path(&root, &|e| e.local() == "SignatureInfoV1")
        .and_then(|p| c14n::at(&root, &p))?;
    let id = child_text(info, "SetupID").trim().to_owned();
    (!id.is_empty()).then_some(id)
}

/// XML-DSig §6.2 digest methods (SHA-1 only where policy allows it, see [`crate::policy`]).
fn digest_alg(uri: &str) -> Result<HashAlg> {
    match uri {
        "http://www.w3.org/2000/09/xmldsig#sha1" => Ok(HashAlg::Sha1),
        "http://www.w3.org/2001/04/xmlenc#sha256" => Ok(HashAlg::Sha256),
        "http://www.w3.org/2001/04/xmldsig-more#sha384" => Ok(HashAlg::Sha384),
        "http://www.w3.org/2001/04/xmlenc#sha512" => Ok(HashAlg::Sha512),
        u => Err(SignError::Unsupported(format!("digest method {u}"))),
    }
}

/// XML-DSig §6.4 / RFC 4051 signature methods: (hash, ECDSA).
fn signature_alg(uri: &str) -> Result<(HashAlg, bool)> {
    let more = "http://www.w3.org/2001/04/xmldsig-more#";
    match uri {
        "http://www.w3.org/2000/09/xmldsig#rsa-sha1" => Ok((HashAlg::Sha1, false)),
        u if u == format!("{more}rsa-sha256") => Ok((HashAlg::Sha256, false)),
        u if u == format!("{more}rsa-sha384") => Ok((HashAlg::Sha384, false)),
        u if u == format!("{more}rsa-sha512") => Ok((HashAlg::Sha512, false)),
        u if u == format!("{more}ecdsa-sha1") => Ok((HashAlg::Sha1, true)),
        u if u == format!("{more}ecdsa-sha256") => Ok((HashAlg::Sha256, true)),
        u if u == format!("{more}ecdsa-sha384") => Ok((HashAlg::Sha384, true)),
        u if u == format!("{more}ecdsa-sha512") => Ok((HashAlg::Sha512, true)),
        u => Err(SignError::Unsupported(format!("signature method {u}"))),
    }
}

fn b64(s: &str) -> Result<Vec<u8>> {
    let clean: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    B64.decode(clean)
        .map_err(|e| SignError::Malformed(format!("base64: {e}")))
}

fn is_dsig(e: &Element, local: &str, root: &Element, path: &[usize]) -> bool {
    e.local() == local && namespace_of(root, path, e) == Some(DSIG_NS.to_owned())
}

/// Namespace URI of `e` (located at `path` under `root`).
fn namespace_of(root: &Element, path: &[usize], e: &Element) -> Option<String> {
    let prefix = e.name.split_once(':').map_or("", |(p, _)| p);
    let key = if prefix.is_empty() {
        "xmlns".to_owned()
    } else {
        format!("xmlns:{prefix}")
    };
    let mut found = None;
    let mut cur = root;
    let mut chain = vec![root];
    for &i in path {
        cur = cur.elements().nth(i)?;
        chain.push(cur);
    }
    for el in chain {
        if let Some(v) = el.attr(&key) {
            found = Some(v.to_owned());
        }
    }
    found
}

/// The canonicalisation named by a `CanonicalizationMethod` / `Transform` element.
fn method_of_element(e: &Element) -> Option<Method> {
    let uri = e.attr("Algorithm")?;
    let list = e
        .child("InclusiveNamespaces")
        .and_then(|n| n.attr("PrefixList"));
    c14n::method_of(uri, list)
}

/// Path of the element whose `Id` (or `ID` / `id`) attribute is `id`.
fn by_id(root: &Element, id: &str) -> Option<Vec<usize>> {
    c14n::find_path(root, &|e| {
        ["Id", "ID", "id"].iter().any(|a| e.attr(a) == Some(id))
    })
}

/// Children with a local name.
fn children<'a>(e: &'a Element, local: &'a str) -> impl Iterator<Item = &'a Element> + 'a {
    e.elements().filter(move |c| c.local() == local)
}

/// One `Reference`: (URI, transforms, digest method, digest value).
fn reference_parts(r: &Element) -> Result<(String, Vec<&Element>, HashAlg, Vec<u8>)> {
    let uri = r.attr("URI").unwrap_or_default().to_owned();
    let transforms: Vec<&Element> = r
        .child("Transforms")
        .map(|t| children(t, "Transform").collect())
        .unwrap_or_default();
    let alg = digest_alg(
        r.child("DigestMethod")
            .and_then(|d| d.attr("Algorithm"))
            .ok_or_else(|| SignError::Malformed("Reference without DigestMethod".into()))?,
    )?;
    let value = b64(&r
        .child("DigestValue")
        .ok_or_else(|| SignError::Malformed("Reference without DigestValue".into()))?
        .text())?;
    Ok((uri, transforms, alg, value))
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// §13.2.4.24 relationship transform: the selected relationships of a relationships part, sorted by
/// `Id`, `TargetMode` made explicit, in canonical form.
fn relationship_transform(pkg: &Package, rels_part: &str, t: &Element) -> Result<Vec<u8>> {
    let source = crate::ooxml::source_of_rels_part(rels_part)
        .ok_or_else(|| SignError::Malformed(format!("relationships part name {rels_part}")))?;
    let rels: Vec<Relationship> = pkg.relationships(&source)?;
    let ids: Vec<&str> = t
        .elements()
        .filter(|e| e.local() == "RelationshipReference")
        .filter_map(|e| e.attr("SourceId"))
        .collect();
    let types: Vec<&str> = t
        .elements()
        .filter(|e| e.local() == "RelationshipsGroupReference")
        .filter_map(|e| e.attr("SourceType"))
        .collect();
    let selected: Vec<String> = rels
        .iter()
        .filter(|r| ids.contains(&r.id.as_str()) || types.contains(&r.rel_type.as_str()))
        .map(|r| r.id.clone())
        .collect();
    Ok(crate::ooxml::transformed_relationships(&rels, &selected).into_bytes())
}

/// Verifies one signature part (`xml`) of `pkg`.
#[allow(clippy::too_many_lines)]
pub(crate) fn verify_part(pkg: &Package, name: &str, xml: &[u8]) -> Result<Verified> {
    let root = c14n::parse(xml)?;
    let sig_path = c14n::find_path(&root, &|e| e.local() == "Signature")
        .ok_or_else(|| SignError::Malformed(format!("{name}: no Signature element")))?;
    let sig = c14n::at(&root, &sig_path).ok_or_else(|| SignError::Malformed("Signature".into()))?;
    if !is_dsig(sig, "Signature", &root, &sig_path) {
        return Err(SignError::Unsupported(format!(
            "{name}: not an XML-DSig signature"
        )));
    }
    let child_path = |local: &str| -> Option<Vec<usize>> {
        sig.elements().position(|e| e.local() == local).map(|i| {
            let mut p = sig_path.clone();
            p.push(i);
            p
        })
    };
    let si_path = child_path("SignedInfo")
        .ok_or_else(|| SignError::Malformed(format!("{name}: no SignedInfo")))?;
    let signed_info =
        c14n::at(&root, &si_path).ok_or_else(|| SignError::Malformed("SignedInfo".into()))?;
    let method = signed_info
        .child("CanonicalizationMethod")
        .and_then(method_of_element)
        .ok_or_else(|| SignError::Unsupported(format!("{name}: canonicalization method")))?;
    let (sig_hash, ecdsa) = signature_alg(
        signed_info
            .child("SignatureMethod")
            .and_then(|m| m.attr("Algorithm"))
            .unwrap_or_default(),
    )?;
    // KeyInfo: the first X509Certificate is the signer's (ECMA-376 Part 2 §13.2.4.4 requires one).
    let certificates: Vec<Vec<u8>> = sig
        .child("KeyInfo")
        .and_then(|k| k.child("X509Data"))
        .map(|d| {
            children(d, "X509Certificate")
                .map(|c| b64(&c.text()))
                .collect()
        })
        .transpose()?
        .unwrap_or_default();
    let cert_der = certificates
        .first()
        .cloned()
        .ok_or_else(|| SignError::Unsupported(format!("{name}: no X509Certificate in KeyInfo")))?;
    let cert = <x509_cert::Certificate as der::Decode>::from_der(&cert_der)?;
    let sig_value_el = sig
        .child("SignatureValue")
        .ok_or_else(|| SignError::Malformed(format!("{name}: no SignatureValue")))?;
    let mut sig_value = b64(&sig_value_el.text())?;
    if ecdsa {
        // XML-DSig 1.1 §6.4.3: r || s.
        sig_value = crate::ecdsa_raw_to_der(&sig_value, KeyKind::of_cert(&cert_der)?)?;
    }
    let canonical_si = c14n::canonicalize(&root, &si_path, &method)?;
    crate::verify_sig(
        &cert,
        sig_hash,
        &sig_hash.digest(canonical_si.as_bytes()),
        &sig_value,
    )
    .map_err(|_| SignError::Invalid(format!("{name}: SignedInfo signature does not verify")))?;

    // Same-document references (package object, XAdES signed properties, …).
    let mut referenced: Vec<Vec<usize>> = Vec::new();
    for r in children(signed_info, "Reference") {
        let (uri, transforms, alg, expected) = reference_parts(r)?;
        let id = uri
            .strip_prefix('#')
            .ok_or_else(|| SignError::Unsupported(format!("{name}: reference {uri}")))?;
        let path = by_id(&root, id)
            .ok_or_else(|| SignError::Invalid(format!("{name}: {uri} not found")))?;
        // Transforms of a same-document reference: canonicalisations; none = Canonical XML 1.0.
        let mut m = Method::Inclusive;
        for t in &transforms {
            m = method_of_element(t).ok_or_else(|| {
                SignError::Unsupported(format!(
                    "{name}: transform {}",
                    t.attr("Algorithm").unwrap_or_default()
                ))
            })?;
        }
        let data = c14n::canonicalize(&root, &path, &m)?;
        if alg.digest(data.as_bytes()) != expected {
            return Err(SignError::Invalid(format!("{name}: {uri} digest mismatch")));
        }
        referenced.push(path);
    }

    // The package Object's Manifest must be signed (§13.2.4.6) and match the parts.
    let manifest_path = c14n::find_path(&root, &|e| e.local() == "Manifest")
        .ok_or_else(|| SignError::Unsupported(format!("{name}: no package Manifest")))?;
    if !referenced.iter().any(|p| manifest_path.starts_with(p)) {
        return Err(SignError::Invalid(format!(
            "{name}: the Manifest is not signed"
        )));
    }
    let manifest =
        c14n::at(&root, &manifest_path).ok_or_else(|| SignError::Malformed("Manifest".into()))?;
    let mut covered: Vec<String> = Vec::new();
    for r in children(manifest, "Reference") {
        let (uri, transforms, alg, expected) = reference_parts(r)?;
        let part_name = percent_decode(uri.split('?').next().unwrap_or_default());
        let part = pkg
            .part(&part_name)
            .ok_or_else(|| SignError::Invalid(format!("{name}: {part_name} was removed")))?;
        let mut data = part.data.clone();
        for t in &transforms {
            match t.attr("Algorithm") {
                Some(REL_TRANSFORM) => data = relationship_transform(pkg, &part_name, t)?,
                // The canonical form produced by the relationship transform is already C14N.
                Some(a) if c14n::method_of(a, None).is_some() => {
                    let tree = c14n::parse(&data)?;
                    data = c14n::canonicalize(
                        &tree,
                        &[],
                        &c14n::method_of(a, None).unwrap_or(Method::Inclusive),
                    )?
                    .into_bytes();
                }
                a => {
                    return Err(SignError::Unsupported(format!(
                        "{name}: manifest transform {}",
                        a.unwrap_or_default()
                    )))
                }
            }
        }
        if alg.digest(&data) != expected {
            return Err(SignError::Invalid(format!(
                "{name}: {part_name} was modified"
            )));
        }
        covered.push(part_name.to_ascii_lowercase());
    }
    let unsigned_parts: Vec<String> = pkg
        .parts()
        .iter()
        .map(|p| p.name.clone())
        .filter(|n| {
            !n.eq_ignore_ascii_case("/[Content_Types].xml")
                && !crate::ooxml::is_signature_part_name(n)
                && !covered.contains(&n.to_ascii_lowercase())
        })
        .collect();

    // XAdES: the signed properties name the signer certificate (TS 101 903 §7.2.2).
    let signed_props_ref =
        children(signed_info, "Reference").any(|r| r.attr("Type") == Some(XADES_SIGNED_PROPERTIES));
    if let Some(sc) = c14n::find_path(&root, &|e| {
        e.local() == "SigningCertificate" || e.local() == "SigningCertificateV2"
    })
    .and_then(|p| c14n::at(&root, &p))
    {
        if !signed_props_ref {
            return Err(SignError::Invalid(format!(
                "{name}: XAdES properties are not signed"
            )));
        }
        let matches = children(sc, "Cert").any(|c| {
            let Some(d) = c.child("CertDigest") else {
                return false;
            };
            let (Some(m), Some(v)) = (
                d.child("DigestMethod").and_then(|m| m.attr("Algorithm")),
                d.child("DigestValue"),
            ) else {
                return false;
            };
            match (digest_alg(m), b64(&v.text())) {
                (Ok(alg), Ok(v)) => alg.digest(&cert_der) == v,
                _ => false,
            }
        });
        if !matches {
            return Err(SignError::Invalid(format!(
                "{name}: signing certificate does not match KeyInfo"
            )));
        }
    }

    // XAdES SignatureTimeStamp over the canonical SignatureValue (TS 101 903 §7.3).
    let timestamp = match c14n::find_path(&root, &|e| e.local() == "SignatureTimeStamp")
        .and_then(|p| c14n::at(&root, &p))
    {
        Some(ts) => {
            let token = b64(&ts
                .child("EncapsulatedTimeStamp")
                .ok_or_else(|| SignError::Malformed(format!("{name}: empty SignatureTimeStamp")))?
                .text())?;
            let m = ts
                .child("CanonicalizationMethod")
                .and_then(method_of_element)
                .unwrap_or(Method::Inclusive);
            let sv_path = child_path("SignatureValue").unwrap_or_default();
            let message = c14n::canonicalize(&root, &sv_path, &m)?;
            Some(timestamp::verify_token(&token, message.as_bytes())?)
        }
        None => None,
    };
    let signing_time = c14n::find_path(&root, &|e| e.local() == "SignatureTime")
        .and_then(|p| c14n::at(&root, &p))
        .and_then(|t| t.child("Value"))
        .map(Element::text)
        .or_else(|| {
            c14n::find_path(&root, &|e| e.local() == "SigningTime")
                .and_then(|p| c14n::at(&root, &p))
                .map(Element::text)
        });
    // [MS-OFFCRYPTO] idOfficeObject: the signature line, when its object is signed.
    let signature_line = c14n::find_path(&root, &|e| e.local() == "SignatureInfoV1")
        .filter(|p| referenced.iter().any(|r| p.starts_with(r)))
        .and_then(|p| c14n::at(&root, &p))
        .map(|info| crate::ooxml::SignedLineInfo {
            setup_id: child_text(info, "SetupID").trim().to_owned(),
            text: child_text(info, "SignatureText"),
            comments: child_text(info, "SignatureComments"),
        })
        .filter(|l| !l.setup_id.is_empty());
    Ok(Verified {
        certificate_der: cert_der,
        certificates,
        signing_time,
        timestamp,
        unsigned_parts,
        signature_line,
    })
}
