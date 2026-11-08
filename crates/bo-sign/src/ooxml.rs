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
//! plus the `SignatureTime` property (§12.2.4.20). A second `Object`
//! carries XAdES qualifying properties ([MS-OFFCRYPTO] §2.5.2, ETSI TS
//! 101 903): signed `SigningTime` + `SigningCertificate` (XAdES-BES) and,
//! with a time-stamp authority, the unsigned `SignatureTimeStamp` (XAdES-T).
//!
//! The XML is emitted directly in Canonical XML 1.0 form (no insignificant
//! whitespace, expanded empty elements, sorted attributes), so the
//! canonicalisation of `SignedInfo`/`Object` only adds the inherited default
//! namespace declaration.

use crate::timestamp::{self, TimestampClient, TimestampInfo};
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
/// RFC 4051 §2.3.6 / XML-DSig 1.1 §6.4.3: ECDSA over SHA-256, value = `r || s`.
const ECDSA_SHA256: &str = "http://www.w3.org/2001/04/xmldsig-more#ecdsa-sha256";

/// `SignatureMethod` of a certificate's key.
fn signature_method(cert_der: &[u8]) -> Result<&'static str> {
    Ok(match crate::KeyKind::of_cert(cert_der)? {
        crate::KeyKind::Rsa => RSA_SHA256,
        _ => ECDSA_SHA256,
    })
}
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
/// XAdES 1.3.2 namespace (ETSI TS 101 903) and `SignedProperties` reference type.
const XADES_NS: &str = "http://uri.etsi.org/01903/v1.3.2#";
const XADES_SIGNED_PROPERTIES: &str = "http://uri.etsi.org/01903#SignedProperties";

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

/// Decimal string of a big-endian unsigned integer (`X509SerialNumber`,
/// XML-DSig §4.4.4: `integer`).
fn decimal(be: &[u8]) -> String {
    let mut digits: Vec<u8> = vec![0];
    for &byte in be {
        // digits (little-endian base 10) = digits * 256 + byte
        let mut carry = u32::from(byte);
        for d in &mut digits {
            let v = u32::from(*d) * 256 + carry;
            *d = (v % 10) as u8;
            carry = v / 10;
        }
        while carry > 0 {
            digits.push((carry % 10) as u8);
            carry /= 10;
        }
    }
    while digits.len() > 1 && digits.last() == Some(&0) {
        digits.pop();
    }
    digits.iter().rev().map(|d| char::from(b'0' + d)).collect()
}

/// XAdES `SignedProperties` (ETSI TS 101 903 §7.2.1–7.2.2, as used by
/// [MS-OFFCRYPTO] §2.5.2): signing time and signing-certificate binding.
fn xades_signed_properties(cert_der: &[u8], time: SystemTime) -> Result<String> {
    use der::Decode;
    let cert = x509_cert::Certificate::from_der(cert_der)?;
    Ok(format!(
        "<xd:SignedProperties Id=\"idSignedProperties\"><xd:SignedSignatureProperties><xd:SigningTime>{}</xd:SigningTime><xd:SigningCertificate><xd:Cert><xd:CertDigest><DigestMethod Algorithm=\"{SHA256}\"></DigestMethod><DigestValue>{}</DigestValue></xd:CertDigest><xd:IssuerSerial><X509IssuerName>{}</X509IssuerName><X509SerialNumber>{}</X509SerialNumber></xd:IssuerSerial></xd:Cert></xd:SigningCertificate></xd:SignedSignatureProperties></xd:SignedProperties>",
        w3c_time(time),
        digest_b64(cert_der),
        c14n_escape(&cert.tbs_certificate.issuer.to_string(), false),
        decimal(cert.tbs_certificate.serial_number.as_bytes()),
    ))
}

/// Canonical form of the `SignedProperties` subset: the apex element gets
/// the in-scope default (`ds`) and `xd` namespace declarations (C14N §2.4).
fn canonical_signed_properties(sp: &str) -> String {
    sp.replacen(
        "<xd:SignedProperties",
        &format!("<xd:SignedProperties xmlns=\"{DSIG_NS}\" xmlns:xd=\"{XADES_NS}\""),
        1,
    )
}

/// Canonical `SignatureValue` element: input of the XAdES
/// `SignatureTimeStamp` (ETSI TS 101 903 §7.3).
fn canonical_signature_value(b64: &str) -> String {
    format!("<SignatureValue xmlns=\"{DSIG_NS}\">{b64}</SignatureValue>")
}

/// A package with its signature computed up to `SignedInfo`, waiting for
/// the signature value (two-phase signing).
pub struct PreparedOoxml {
    pkg: Package,
    sig_part: String,
    sig_index: u32,
    origin_rels: Vec<Relationship>,
    signed_info: String,
    object: String,
    signed_properties: String,
    cert_der: Vec<u8>,
    chain: Vec<Vec<u8>>,
}

impl PreparedOoxml {
    /// SHA-256 of the canonical `SignedInfo`, to sign with PKCS#1 v1.5.
    #[must_use]
    pub fn digest(&self) -> [u8; 32] {
        sha256(with_dsig_ns(&self.signed_info, "SignedInfo").as_bytes())
    }

    /// The `SignatureValue` bytes of a signer's output: PKCS#1 v1.5 as is, ECDSA as `r || s`.
    fn signature_value(&self, signature: &[u8]) -> Result<Vec<u8>> {
        let kind = crate::KeyKind::of_cert(&self.cert_der)?;
        if kind == crate::KeyKind::Rsa || signature.len() == 2 * kind.field_bytes() {
            return Ok(signature.to_vec());
        }
        crate::ecdsa_der_to_raw(signature, kind)
    }

    /// Bytes an RFC 3161 token must cover for this signature value.
    #[must_use]
    pub fn timestamp_message(&self, signature: &[u8]) -> Vec<u8> {
        let value = self
            .signature_value(signature)
            .unwrap_or_else(|_| signature.to_vec());
        canonical_signature_value(&B64.encode(value)).into_bytes()
    }

    /// Writes the signature part (with an optional XAdES `SignatureTimeStamp`
    /// holding `timestamp_token`) and returns the package bytes.
    pub fn finish(mut self, signature: &[u8], timestamp_token: Option<&[u8]>) -> Result<Vec<u8>> {
        let signature = &self.signature_value(signature)?;
        let mut certs = format!(
            "<X509Certificate>{}</X509Certificate>",
            B64.encode(&self.cert_der)
        );
        for c in &self.chain {
            let _ = write!(
                certs,
                "<X509Certificate>{}</X509Certificate>",
                B64.encode(c)
            );
        }
        let unsigned = match timestamp_token {
            Some(token) => format!(
                "<xd:UnsignedProperties><xd:UnsignedSignatureProperties><xd:SignatureTimeStamp Id=\"idSignatureTimeStamp\"><CanonicalizationMethod Algorithm=\"{C14N}\"></CanonicalizationMethod><xd:EncapsulatedTimeStamp>{}</xd:EncapsulatedTimeStamp></xd:SignatureTimeStamp></xd:UnsignedSignatureProperties></xd:UnsignedProperties>",
                B64.encode(token)
            ),
            None => String::new(),
        };
        let xml = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?><Signature xmlns=\"{DSIG_NS}\" Id=\"idPackageSignature\">{}<SignatureValue>{}</SignatureValue><KeyInfo><X509Data>{certs}</X509Data></KeyInfo>{}<Object><xd:QualifyingProperties xmlns:xd=\"{XADES_NS}\" Target=\"#idPackageSignature\">{}{unsigned}</xd:QualifyingProperties></Object></Signature>",
            self.signed_info,
            B64.encode(signature),
            self.object,
            self.signed_properties,
        );
        self.pkg
            .add_part(&self.sig_part, CT_SIGNATURE, xml.into_bytes())?;
        let id = fresh_id(&self.origin_rels);
        self.origin_rels.push(Relationship {
            id,
            rel_type: REL_SIGNATURE.into(),
            target: format!("sig{}.xml", self.sig_index),
            mode: TargetMode::Internal,
        });
        self.pkg.set_relationships(ORIGIN_PART, &self.origin_rels);
        Ok(self.pkg.write()?)
    }
}

/// Signs an OOXML package (bytes of a `.docx/.xlsx/.pptx`), returning the new package.
pub fn sign_ooxml(bytes: &[u8], signer: &dyn Signer, time: SystemTime) -> Result<Vec<u8>> {
    sign_ooxml_with(bytes, signer, time, None)
}

/// [`sign_ooxml`] plus, with a time-stamp authority, an XAdES
/// `SignatureTimeStamp` (XAdES-T, ETSI TS 101 903 §7.3).
pub fn sign_ooxml_with(
    bytes: &[u8],
    signer: &dyn Signer,
    time: SystemTime,
    tsa: Option<&dyn TimestampClient>,
) -> Result<Vec<u8>> {
    let prepared = prepare_ooxml(bytes, signer.certificate_der(), &signer.chain_der(), time)?;
    let signature = signer.sign_sha256(&prepared.digest())?;
    let token = match tsa {
        Some(client) => {
            let message = prepared.timestamp_message(&signature);
            Some(timestamp::timestamp(
                client,
                &message,
                timestamp::fresh_nonce(&message),
            )?)
        }
        None => None,
    };
    prepared.finish(&signature, token.as_deref())
}

/// Adds the origin part, computes the manifest and `SignedInfo` for a
/// signature by `cert_der` at `time`.
pub fn prepare_ooxml(
    bytes: &[u8],
    cert_der: &[u8],
    chain: &[Vec<u8>],
    time: SystemTime,
) -> Result<PreparedOoxml> {
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
    let origin_rels = pkg.relationships(ORIGIN_PART)?;
    let n = (1..=u32::MAX)
        .find(|k| pkg.part(&format!("/_xmlsignatures/sig{k}.xml")).is_none())
        .unwrap_or(1);
    let sig_part = format!("/_xmlsignatures/sig{n}.xml");

    let object = format!(
        "<Object Id=\"idPackageObject\"><Manifest>{}</Manifest><SignatureProperties><SignatureProperty Id=\"idSignatureTime\" Target=\"#idPackageSignature\"><mdssi:SignatureTime xmlns:mdssi=\"{MDSSI_NS}\"><mdssi:Format>YYYY-MM-DDThh:mm:ssTZD</mdssi:Format><mdssi:Value>{}</mdssi:Value></mdssi:SignatureTime></SignatureProperty></SignatureProperties></Object>",
        manifest(&pkg)?,
        w3c_time(time)
    );
    let signed_properties = xades_signed_properties(cert_der, time)?;
    let method = signature_method(cert_der)?;
    let signed_info = format!(
        "<SignedInfo><CanonicalizationMethod Algorithm=\"{C14N}\"></CanonicalizationMethod><SignatureMethod Algorithm=\"{method}\"></SignatureMethod><Reference Type=\"{DSIG_NS}Object\" URI=\"#idPackageObject\"><DigestMethod Algorithm=\"{SHA256}\"></DigestMethod><DigestValue>{}</DigestValue></Reference><Reference Type=\"{XADES_SIGNED_PROPERTIES}\" URI=\"#idSignedProperties\"><Transforms><Transform Algorithm=\"{C14N}\"></Transform></Transforms><DigestMethod Algorithm=\"{SHA256}\"></DigestMethod><DigestValue>{}</DigestValue></Reference></SignedInfo>",
        digest_b64(with_dsig_ns(&object, "Object").as_bytes()),
        digest_b64(canonical_signed_properties(&signed_properties).as_bytes()),
    );
    Ok(PreparedOoxml {
        pkg,
        sig_part,
        sig_index: n,
        origin_rels,
        signed_info,
        object,
        signed_properties,
        cert_der: cert_der.to_vec(),
        chain: chain.to_vec(),
    })
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
    /// Every `X509Certificate` of `KeyInfo` (signer first), for path building.
    pub certificates: Vec<Vec<u8>>,
    /// Claimed signing time (`mdssi:SignatureTime` value) verbatim, if present.
    pub signing_time: Option<String>,
    /// Verified XAdES `SignatureTimeStamp`, if present.
    pub timestamp: Option<TimestampInfo>,
}

/// Every `<tag>…</tag>` text in `s`.
fn all_inner<'a>(s: &'a str, tag: &str) -> Vec<&'a str> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let mut out = Vec::new();
    let mut rest = s;
    while let Some(a) = rest.find(&open) {
        let start = a + open.len();
        let Some(b) = rest[start..].find(&close) else {
            break;
        };
        out.push(&rest[start..start + b]);
        rest = &rest[start + b + close.len()..];
    }
    out
}

/// Verifies the signatures of a package produced by [`sign_ooxml`]: the
/// `SignedInfo` RSA signature, the digests of every `SignedInfo` reference
/// (package object, XAdES signed properties), every manifest reference
/// against the current package contents, the XAdES signing-certificate
/// binding and the signature time-stamp. Signatures written by other
/// producers with a different serialisation are reported as
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
        let sig_b64 = inner(xml, "SignatureValue").ok_or_else(unsupported)?;
        let sig_value = B64
            .decode(sig_b64)
            .map_err(|e| SignError::Malformed(e.to_string()))?;
        let key_info = inner(xml, "KeyInfo").ok_or_else(unsupported)?;
        let certificates = all_inner(key_info, "X509Certificate")
            .into_iter()
            .map(|c| {
                B64.decode(c)
                    .map_err(|e| SignError::Malformed(e.to_string()))
            })
            .collect::<Result<Vec<_>>>()?;
        let cert = certificates.first().cloned().ok_or_else(unsupported)?;
        // ECDSA SignatureValue is `r || s` (XML-DSig 1.1 §6.4.3); the verifier takes DER.
        let sig_value = if signed_info.contains(ECDSA_SHA256) {
            crate::ecdsa_raw_to_der(&sig_value, crate::KeyKind::of_cert(&cert)?)?
        } else {
            sig_value
        };
        verify_with_cert(
            &cert,
            &sha256(with_dsig_ns(signed_info, "SignedInfo").as_bytes()),
            &sig_value,
        )?;
        let signed_properties = between(
            xml,
            "<xd:SignedProperties Id=\"idSignedProperties\">",
            "</xd:SignedProperties>",
        );
        let mut has_object = false;
        for r in signed_info.split("<Reference ").skip(1) {
            let uri = r
                .split_once("URI=\"")
                .and_then(|(_, rest)| rest.split_once('"'))
                .map(|(u, _)| u)
                .ok_or_else(unsupported)?;
            let digest = inner(r, "DigestValue").ok_or_else(unsupported)?;
            let expected = match uri {
                "#idPackageObject" => {
                    has_object = true;
                    digest_b64(with_dsig_ns(object, "Object").as_bytes())
                }
                "#idSignedProperties" => {
                    let sp = signed_properties.ok_or_else(unsupported)?;
                    digest_b64(canonical_signed_properties(sp).as_bytes())
                }
                _ => return Err(unsupported()),
            };
            if digest != expected {
                return Err(SignError::Invalid(format!("{name}: {uri} digest mismatch")));
            }
        }
        if !has_object {
            return Err(unsupported());
        }
        // XAdES SigningCertificate must name the KeyInfo certificate (TS 101 903 §7.2.2).
        if let Some(sp) = signed_properties {
            let cert_digest = inner(sp, "xd:CertDigest")
                .and_then(|d| inner(d, "DigestValue"))
                .ok_or_else(unsupported)?;
            if cert_digest != digest_b64(&cert) {
                return Err(SignError::Invalid(format!(
                    "{name}: signing certificate does not match KeyInfo"
                )));
            }
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
        let timestamp = match inner(xml, "xd:EncapsulatedTimeStamp") {
            Some(t) => {
                let token = B64
                    .decode(t)
                    .map_err(|e| SignError::Malformed(e.to_string()))?;
                Some(timestamp::verify_token(
                    &token,
                    canonical_signature_value(sig_b64).as_bytes(),
                )?)
            }
            None => None,
        };
        out.push(OoxmlSignature {
            part: name,
            certificate_der: cert,
            certificates,
            signing_time: inner(object, "mdssi:Value").map(str::to_owned),
            timestamp,
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
