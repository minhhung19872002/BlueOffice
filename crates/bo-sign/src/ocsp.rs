//! Online Certificate Status Protocol (RFC 6960): requests (§4.1), basic
//! responses (§4.2) and their checks, used for certificate revocation together
//! with CRLs ([`crate::trust`]).
//!
//! * A request names a certificate by its [`CertId`] (§4.1.1): SHA-1 hashes of
//!   the issuer's name and public key (the default of most responders) and the
//!   serial number; responses with SHA-256 CertIDs are matched too.
//! * A response counts only when it is `successful` with an
//!   `id-pkix-ocsp-basic` body whose signature verifies with the certificate's
//!   issuer, or with a responder certificate included in the response that the
//!   issuer signed and that carries `id-kp-OCSPSigning` (§4.2.2.2).
//! * Transport: HTTP POST `application/ocsp-request` (Appendix A.1) through
//!   [`crate::http::HttpTransport`]; the responder URL comes from the
//!   certificate's Authority Information Access extension (RFC 5280 §4.2.2.1).

use crate::http::HttpTransport;
use crate::{verify_sig, HashAlg, Result, SignError};
use const_oid::db::{rfc5280, rfc5912};
use const_oid::ObjectIdentifier;
use der::asn1::{BitString, GeneralizedTime, OctetString};
use der::{Any, Decode, Encode, Reader, Sequence, SliceReader, Tag, TagNumber, Tagged};
use sha1::Digest as _;
use spki::AlgorithmIdentifierOwned;
use std::time::SystemTime;
use x509_cert::ext::pkix::name::GeneralName;
use x509_cert::ext::pkix::{AuthorityInfoAccessSyntax, ExtendedKeyUsage};
use x509_cert::ext::Extensions;
use x509_cert::serial_number::SerialNumber;
use x509_cert::Certificate;

/// id-pkix-ocsp-basic (RFC 6960 §4.2.1).
pub const ID_PKIX_OCSP_BASIC: ObjectIdentifier =
    ObjectIdentifier::new_unwrap("1.3.6.1.5.5.7.48.1.1");
/// id-sha1 (RFC 3279 §2.2.1).
const ID_SHA1: ObjectIdentifier = ObjectIdentifier::new_unwrap("1.3.14.3.2.26");

/// CertID (RFC 6960 §4.1.1).
#[derive(Clone, Debug, Eq, PartialEq, Sequence)]
pub struct CertId {
    /// Hash algorithm of the two hashes.
    pub hash_algorithm: AlgorithmIdentifierOwned,
    /// Hash of the issuer's distinguished name (DER).
    pub issuer_name_hash: OctetString,
    /// Hash of the issuer's public key (BIT STRING value, RFC 6960 §4.1.1).
    pub issuer_key_hash: OctetString,
    /// Serial number of the certificate.
    pub serial_number: SerialNumber,
}

/// Request (RFC 6960 §4.1.1).
#[derive(Clone, Debug, Eq, PartialEq, Sequence)]
pub struct Request {
    /// The certificate asked about.
    pub req_cert: CertId,
}

/// TBSRequest (RFC 6960 §4.1.1); version v1 (the default, omitted).
#[derive(Clone, Debug, Eq, PartialEq, Sequence)]
pub struct TbsRequest {
    /// Version (`[0] EXPLICIT`, absent for v1).
    #[asn1(context_specific = "0", tag_mode = "EXPLICIT", optional = "true")]
    pub version: Option<u8>,
    /// The certificates asked about.
    pub request_list: Vec<Request>,
}

/// OCSPRequest (RFC 6960 §4.1.1), unsigned.
#[derive(Clone, Debug, Eq, PartialEq, Sequence)]
pub struct OcspRequest {
    /// The request.
    pub tbs_request: TbsRequest,
}

/// ResponseBytes (RFC 6960 §4.2.1).
#[derive(Clone, Debug, Eq, PartialEq, Sequence)]
pub struct ResponseBytes {
    /// `id-pkix-ocsp-basic`.
    pub response_type: ObjectIdentifier,
    /// DER of the BasicOCSPResponse.
    pub response: OctetString,
}

/// OCSPResponse (RFC 6960 §4.2.1).
#[derive(Clone, Debug, Eq, PartialEq, Sequence)]
pub struct OcspResponse {
    /// OCSPResponseStatus (ENUMERATED; 0 = successful).
    pub response_status: Any,
    /// Present when successful.
    #[asn1(context_specific = "0", tag_mode = "EXPLICIT", optional = "true")]
    pub response_bytes: Option<ResponseBytes>,
}

/// SingleResponse (RFC 6960 §4.2.1).
#[derive(Clone, Debug, Eq, PartialEq, Sequence)]
pub struct SingleResponse {
    /// The certificate this status is about.
    pub cert_id: CertId,
    /// CertStatus CHOICE: `[0]` good, `[1]` revoked (RevokedInfo), `[2]` unknown.
    pub cert_status: Any,
    /// Time the status was known correct.
    pub this_update: GeneralizedTime,
    /// Time newer status will be available.
    #[asn1(context_specific = "0", tag_mode = "EXPLICIT", optional = "true")]
    pub next_update: Option<GeneralizedTime>,
    /// singleExtensions.
    #[asn1(context_specific = "1", tag_mode = "EXPLICIT", optional = "true")]
    pub single_extensions: Option<Extensions>,
}

/// ResponseData (RFC 6960 §4.2.1).
#[derive(Clone, Debug, Eq, PartialEq, Sequence)]
pub struct ResponseData {
    /// Version (`[0] EXPLICIT`, absent for v1).
    #[asn1(context_specific = "0", tag_mode = "EXPLICIT", optional = "true")]
    pub version: Option<u8>,
    /// ResponderID CHOICE: `[1]` byName (Name), `[2]` byKey (KeyHash).
    pub responder_id: Any,
    /// Time the response was signed.
    pub produced_at: GeneralizedTime,
    /// One status per certificate asked about.
    pub responses: Vec<SingleResponse>,
    /// responseExtensions.
    #[asn1(context_specific = "1", tag_mode = "EXPLICIT", optional = "true")]
    pub response_extensions: Option<Extensions>,
}

/// BasicOCSPResponse (RFC 6960 §4.2.1).
#[derive(Clone, Debug, Eq, PartialEq, Sequence)]
pub struct BasicOcspResponse {
    /// Signed data.
    pub tbs_response_data: ResponseData,
    /// Signature algorithm.
    pub signature_algorithm: AlgorithmIdentifierOwned,
    /// Signature over the DER of `tbs_response_data`.
    pub signature: BitString,
    /// Certificates helping to verify the signature (delegated responder).
    #[asn1(context_specific = "0", tag_mode = "EXPLICIT", optional = "true")]
    pub certs: Option<Vec<Certificate>>,
}

/// Status of a certificate (RFC 6960 §4.2.1 CertStatus).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CertStatus {
    /// Not revoked.
    Good,
    /// Revoked at this time.
    Revoked(SystemTime),
    /// The responder does not know the certificate.
    Unknown,
}

fn hash(alg: HashKind, data: &[u8]) -> Vec<u8> {
    match alg {
        HashKind::Sha1 => sha1::Sha1::digest(data).to_vec(),
        HashKind::Sha256 => HashAlg::Sha256.digest(data),
    }
}

#[derive(Clone, Copy)]
enum HashKind {
    Sha1,
    Sha256,
}

impl HashKind {
    fn of(oid: ObjectIdentifier) -> Option<HashKind> {
        if oid == ID_SHA1 {
            Some(HashKind::Sha1)
        } else if oid == rfc5912::ID_SHA_256 {
            Some(HashKind::Sha256)
        } else {
            None
        }
    }
}

/// Hash of the issuer key as used by CertID and ResponderID byKey: the
/// subjectPublicKey BIT STRING value (RFC 6960 §4.1.1, §4.2.1).
fn key_hash(alg: HashKind, issuer: &Certificate) -> Vec<u8> {
    hash(
        alg,
        issuer
            .tbs_certificate
            .subject_public_key_info
            .subject_public_key
            .raw_bytes(),
    )
}

/// CertID of `cert` issued by `issuer` with SHA-1 (RFC 6960 §4.1.1).
pub fn cert_id(cert: &Certificate, issuer: &Certificate) -> Result<CertId> {
    let name = issuer.tbs_certificate.subject.to_der()?;
    Ok(CertId {
        hash_algorithm: AlgorithmIdentifierOwned {
            oid: ID_SHA1,
            parameters: Some(Any::null()),
        },
        issuer_name_hash: OctetString::new(hash(HashKind::Sha1, &name))?,
        issuer_key_hash: OctetString::new(key_hash(HashKind::Sha1, issuer))?,
        serial_number: cert.tbs_certificate.serial_number.clone(),
    })
}

/// Does `id` (any supported hash) name `cert` issued by `issuer`?
fn matches(id: &CertId, cert: &Certificate, issuer: &Certificate) -> bool {
    let Some(alg) = HashKind::of(id.hash_algorithm.oid) else {
        return false;
    };
    let Ok(name) = issuer.tbs_certificate.subject.to_der() else {
        return false;
    };
    id.serial_number == cert.tbs_certificate.serial_number
        && id.issuer_name_hash.as_bytes() == hash(alg, &name).as_slice()
        && id.issuer_key_hash.as_bytes() == key_hash(alg, issuer).as_slice()
}

/// DER OCSPRequest for `cert` issued by `issuer`.
pub fn request_der(cert: &Certificate, issuer: &Certificate) -> Result<Vec<u8>> {
    Ok(OcspRequest {
        tbs_request: TbsRequest {
            version: None,
            request_list: vec![Request {
                req_cert: cert_id(cert, issuer)?,
            }],
        },
    }
    .to_der()?)
}

/// The basic response inside a DER OCSPResponse (`successful` only).
pub fn parse_response(der: &[u8]) -> Result<BasicOcspResponse> {
    let resp = OcspResponse::from_der(der)?;
    // OCSPResponseStatus ENUMERATED, 0 = successful (§4.2.1).
    if resp.response_status.tag() != Tag::Enumerated || resp.response_status.value() != [0] {
        return Err(SignError::Invalid(format!(
            "OCSP response status {:?}",
            resp.response_status.value()
        )));
    }
    let bytes = resp
        .response_bytes
        .ok_or_else(|| SignError::Malformed("OCSP response without responseBytes".into()))?;
    if bytes.response_type != ID_PKIX_OCSP_BASIC {
        return Err(SignError::Unsupported(format!(
            "OCSP response type {}",
            bytes.response_type
        )));
    }
    Ok(BasicOcspResponse::from_der(bytes.response.as_bytes())?)
}

fn context(any: &Any) -> Option<u8> {
    match any.tag() {
        Tag::ContextSpecific { number, .. } => Some(number.value()),
        _ => None,
    }
}

/// Status carried by a SingleResponse.
fn status_of(s: &SingleResponse) -> Option<CertStatus> {
    match context(&s.cert_status)? {
        0 => Some(CertStatus::Good),
        1 => {
            // RevokedInfo (IMPLICIT SEQUENCE): revocationTime first.
            let mut r = SliceReader::new(s.cert_status.value()).ok()?;
            let t: GeneralizedTime = r.decode().ok()?;
            Some(CertStatus::Revoked(t.to_system_time()))
        }
        2 => Some(CertStatus::Unknown),
        _ => None,
    }
}

/// Is `signer` allowed to answer for certificates of `issuer` (RFC 6960
/// §4.2.2.2): the issuer itself, or a certificate the issuer signed with
/// `id-kp-OCSPSigning`, valid when the response was produced?
fn authorized(signer: &Certificate, issuer: &Certificate, produced: SystemTime) -> bool {
    if signer == issuer {
        return true;
    }
    let eku = signer
        .tbs_certificate
        .get::<ExtendedKeyUsage>()
        .ok()
        .flatten();
    if !eku.is_some_and(|(_, e)| e.0.contains(&rfc5280::ID_KP_OCSP_SIGNING)) {
        return false;
    }
    let v = &signer.tbs_certificate.validity;
    if produced < v.not_before.to_system_time() || produced > v.not_after.to_system_time() {
        return false;
    }
    if signer.tbs_certificate.issuer != issuer.tbs_certificate.subject {
        return false;
    }
    let (Ok(alg), Ok(tbs), Some(sig)) = (
        HashAlg::from_signature_oid(signer.signature_algorithm.oid),
        signer.tbs_certificate.to_der(),
        signer.signature.as_bytes(),
    ) else {
        return false;
    };
    verify_sig(issuer, alg, &alg.digest(&tbs), sig).is_ok()
}

/// Does the responder named by `id` hold `cert`'s key / name (§4.2.1 ResponderID)?
fn names(id: &Any, cert: &Certificate) -> bool {
    match context(id) {
        Some(1) => cert
            .tbs_certificate
            .subject
            .to_der()
            .is_ok_and(|n| n == id.value()),
        Some(2) => OctetString::from_der(id.value())
            .is_ok_and(|h| h.as_bytes() == key_hash(HashKind::Sha1, cert).as_slice()),
        _ => false,
    }
}

/// Status of `cert` (issued by `issuer`) according to a verified basic
/// response, `None` when the response does not cover it or does not verify.
pub fn status(
    resp: &BasicOcspResponse,
    cert: &Certificate,
    issuer: &Certificate,
) -> Option<Checked> {
    let data = &resp.tbs_response_data;
    let single = data
        .responses
        .iter()
        .find(|s| matches(&s.cert_id, cert, issuer))?;
    let produced = data.produced_at.to_system_time();
    let tbs = data.to_der().ok()?;
    let alg = HashAlg::from_signature_oid(resp.signature_algorithm.oid).ok()?;
    let sig = resp.signature.as_bytes()?;
    let candidates = std::iter::once(issuer).chain(resp.certs.iter().flatten());
    let signer = candidates
        .filter(|c| names(&data.responder_id, c))
        .find(|c| {
            authorized(c, issuer, produced) && verify_sig(c, alg, &alg.digest(&tbs), sig).is_ok()
        })?;
    let _ = signer;
    Some(Checked {
        status: status_of(single)?,
        this_update: single.this_update.to_system_time(),
        next_update: single.next_update.map(|t| t.to_system_time()),
    })
}

/// A verified status with its validity interval (RFC 6960 §4.2.2.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Checked {
    /// The status.
    pub status: CertStatus,
    /// thisUpdate.
    pub this_update: SystemTime,
    /// nextUpdate, if any.
    pub next_update: Option<SystemTime>,
}

/// OCSP responder URLs of `cert` (Authority Information Access, RFC 5280
/// §4.2.2.1, access method `id-ad-ocsp`).
#[must_use]
pub fn responder_urls(cert: &Certificate) -> Vec<String> {
    let Ok(Some((_, aia))) = cert.tbs_certificate.get::<AuthorityInfoAccessSyntax>() else {
        return Vec::new();
    };
    aia.0
        .iter()
        .filter(|d| d.access_method == rfc5280::ID_AD_OCSP)
        .filter_map(|d| match &d.access_location {
            GeneralName::UniformResourceIdentifier(u) => Some(u.to_string()),
            _ => None,
        })
        .filter(|u| u.starts_with("http://") || u.starts_with("https://"))
        .collect()
}

/// Asks the responder at `url` about `cert` (HTTP POST, RFC 6960 Appendix A.1);
/// returns the DER OCSPResponse.
pub fn fetch(
    transport: &dyn HttpTransport,
    url: &str,
    cert: &Certificate,
    issuer: &Certificate,
) -> Result<Vec<u8>> {
    let req = request_der(cert, issuer)?;
    let headers = vec![(
        "Content-Type".to_owned(),
        "application/ocsp-request".to_owned(),
    )];
    let resp = transport.post(url, &headers, &req)?;
    if resp.status != 200 {
        return Err(SignError::Invalid(format!(
            "OCSP responder returned HTTP {}",
            resp.status
        )));
    }
    Ok(resp.body)
}

/// Builds a signed basic OCSP response (responders and tests): `signer` answers
/// with `entries` (CertID, status, thisUpdate, nextUpdate) produced at
/// `produced`; `certs` are included for a delegated responder.
pub(crate) fn build_response(
    signer: &dyn crate::Signer,
    by_name: bool,
    entries: &[(CertId, CertStatus, SystemTime, Option<SystemTime>)],
    produced: SystemTime,
    certs: Option<Vec<Certificate>>,
) -> Result<Vec<u8>> {
    let gt = |t: SystemTime| {
        GeneralizedTime::from_system_time(t).map_err(|e| SignError::Malformed(e.to_string()))
    };
    let ctx = |n: TagNumber, constructed: bool| Tag::ContextSpecific {
        constructed,
        number: n,
    };
    let me = Certificate::from_der(signer.certificate_der())?;
    let responder_id = if by_name {
        Any::new(
            ctx(TagNumber::N1, true),
            me.tbs_certificate.subject.to_der()?,
        )?
    } else {
        Any::new(
            ctx(TagNumber::N2, true),
            OctetString::new(key_hash(HashKind::Sha1, &me))?.to_der()?,
        )?
    };
    let mut responses = Vec::new();
    for (id, st, this, next) in entries {
        let cert_status = match st {
            CertStatus::Good => Any::new(ctx(TagNumber::N0, false), Vec::new())?,
            CertStatus::Revoked(t) => Any::new(ctx(TagNumber::N1, true), gt(*t)?.to_der()?)?,
            CertStatus::Unknown => Any::new(ctx(TagNumber::N2, false), Vec::new())?,
        };
        responses.push(SingleResponse {
            cert_id: id.clone(),
            cert_status,
            this_update: gt(*this)?,
            next_update: next.map(gt).transpose()?,
            single_extensions: None,
        });
    }
    let data = ResponseData {
        version: None,
        responder_id,
        produced_at: gt(produced)?,
        responses,
        response_extensions: None,
    };
    let digest = crate::sha256(&data.to_der()?);
    let sig = signer.sign_sha256(&digest)?;
    let basic = BasicOcspResponse {
        tbs_response_data: data,
        signature_algorithm: AlgorithmIdentifierOwned {
            oid: rfc5912::SHA_256_WITH_RSA_ENCRYPTION,
            parameters: Some(Any::null()),
        },
        signature: BitString::from_bytes(&sig)?,
        certs,
    };
    Ok(OcspResponse {
        response_status: Any::new(Tag::Enumerated, vec![0])?,
        response_bytes: Some(ResponseBytes {
            response_type: ID_PKIX_OCSP_BASIC,
            response: OctetString::new(basic.to_der()?)?,
        }),
    }
    .to_der()?)
}
