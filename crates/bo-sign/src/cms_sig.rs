//! CMS `SignedData` (RFC 5652) with the signed attributes required by
//! CAdES/PAdES baseline B-B (ETSI EN 319 122-1 §5.2, EN 319 142-1 §6.3):
//! content-type, message-digest and signing-certificate-v2. Signing time is
//! carried by the PDF signature dictionary (`/M`), not as a CMS attribute.
//!
//! Baseline B-T adds the unsigned `signature-time-stamp` attribute
//! (ETSI EN 319 122-1 §5.3, RFC 3161 Appendix A): an RFC 3161 token over the
//! `SignerInfo.signature` value.
//!
//! Signing is split in two phases ([`PreparedCms`]): the digest of the
//! signed attributes is computed first, the signature value (from any
//! [`Signer`], a USB token or a remote service) is supplied afterwards.

use crate::timestamp::{self, TimestampClient, TimestampInfo};
use crate::{sha256, verify_rsa, HashAlg, Result, SignError, Signer};
use cms::cert::{CertificateChoices, IssuerAndSerialNumber};
use cms::content_info::{CmsVersion, ContentInfo};
use cms::signed_data::{
    CertificateSet, EncapsulatedContentInfo, SignedData, SignerIdentifier, SignerInfo, SignerInfos,
};
use const_oid::db::{rfc5911, rfc5912};
use const_oid::ObjectIdentifier;
use der::asn1::{OctetString, SetOfVec};
use der::{Any, Decode, Encode, Sequence};
use spki::AlgorithmIdentifierOwned;
use x509_cert::attr::{Attribute, AttributeValue, Attributes};
use x509_cert::ext::pkix::SubjectKeyIdentifier;
use x509_cert::Certificate;

/// id-aa-signingCertificateV2 (RFC 5035 §3).
const ID_AA_SIGNING_CERTIFICATE_V2: ObjectIdentifier =
    ObjectIdentifier::new_unwrap("1.2.840.113549.1.9.16.2.47");
/// id-aa-signingCertificate (RFC 2634 §5.4), used by older TSAs.
const ID_AA_SIGNING_CERTIFICATE: ObjectIdentifier =
    ObjectIdentifier::new_unwrap("1.2.840.113549.1.9.16.2.12");
/// id-aa-timeStampToken (RFC 3161 Appendix A) = CAdES signature-time-stamp.
pub const ID_AA_SIGNATURE_TIME_STAMP_TOKEN: ObjectIdentifier =
    ObjectIdentifier::new_unwrap("1.2.840.113549.1.9.16.2.14");

/// ESSCertIDv2 with the default hash algorithm (SHA-256) omitted (RFC 5035 §4).
#[derive(Sequence)]
struct EssCertIdV2 {
    cert_hash: OctetString,
}

/// SigningCertificateV2 ::= SEQUENCE { certs SEQUENCE OF ESSCertIDv2 }.
#[derive(Sequence)]
struct SigningCertificateV2 {
    certs: Vec<EssCertIdV2>,
}

pub(crate) fn attribute(oid: ObjectIdentifier, value_der: &[u8]) -> Result<Attribute> {
    let value = AttributeValue::from_der(value_der)?;
    Ok(Attribute {
        oid,
        values: SetOfVec::try_from(vec![value])?,
    })
}

fn sha256_alg() -> AlgorithmIdentifierOwned {
    AlgorithmIdentifierOwned {
        oid: rfc5912::ID_SHA_256,
        parameters: None,
    }
}

/// A `SignedData` waiting for its signature value.
pub struct PreparedCms {
    econtent_type: ObjectIdentifier,
    econtent: Option<Vec<u8>>,
    signed_attrs: SetOfVec<Attribute>,
    cert: Certificate,
    chain: Vec<Certificate>,
}

impl PreparedCms {
    /// Detached `id-data` signature over content with the given SHA-256 digest.
    pub fn detached(content_digest: &[u8; 32], cert_der: &[u8], chain: &[Vec<u8>]) -> Result<Self> {
        Self::new(rfc5911::ID_DATA, None, content_digest, cert_der, chain)
    }

    /// Signature with encapsulated content of the given type (e.g. `TSTInfo`).
    pub fn encapsulated(
        content_type: ObjectIdentifier,
        content: Vec<u8>,
        cert_der: &[u8],
        chain: &[Vec<u8>],
    ) -> Result<Self> {
        let digest = sha256(&content);
        Self::new(content_type, Some(content), &digest, cert_der, chain)
    }

    fn new(
        econtent_type: ObjectIdentifier,
        econtent: Option<Vec<u8>>,
        digest: &[u8; 32],
        cert_der: &[u8],
        chain: &[Vec<u8>],
    ) -> Result<Self> {
        let cert = Certificate::from_der(cert_der)?;
        let chain = chain
            .iter()
            .map(|c| Certificate::from_der(c))
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let signed_attrs = SetOfVec::try_from(vec![
            attribute(rfc5911::ID_CONTENT_TYPE, &econtent_type.to_der()?)?,
            attribute(
                rfc5911::ID_MESSAGE_DIGEST,
                &OctetString::new(digest.to_vec())?.to_der()?,
            )?,
            attribute(
                ID_AA_SIGNING_CERTIFICATE_V2,
                &SigningCertificateV2 {
                    certs: vec![EssCertIdV2 {
                        cert_hash: OctetString::new(sha256(cert_der).to_vec())?,
                    }],
                }
                .to_der()?,
            )?,
        ])?;
        Ok(Self {
            econtent_type,
            econtent,
            signed_attrs,
            cert,
            chain,
        })
    }

    /// SHA-256 digest the signer must sign: RFC 5652 §5.4 — the DER of the
    /// `SET OF` signed attributes.
    pub fn digest(&self) -> Result<[u8; 32]> {
        Ok(sha256(&self.signed_attrs.to_der()?))
    }

    /// Builds the DER `ContentInfo(SignedData)`, optionally with a
    /// signature-time-stamp token (DER `ContentInfo`) as unsigned attribute.
    pub fn assemble(&self, signature: &[u8], timestamp_token: Option<&[u8]>) -> Result<Vec<u8>> {
        let unsigned_attrs = match timestamp_token {
            Some(token) => Some(SetOfVec::try_from(vec![attribute(
                ID_AA_SIGNATURE_TIME_STAMP_TOKEN,
                token,
            )?])?),
            None => None,
        };
        let signer_info = SignerInfo {
            version: CmsVersion::V1,
            sid: SignerIdentifier::IssuerAndSerialNumber(IssuerAndSerialNumber {
                issuer: self.cert.tbs_certificate.issuer.clone(),
                serial_number: self.cert.tbs_certificate.serial_number.clone(),
            }),
            digest_alg: sha256_alg(),
            signed_attrs: Some(self.signed_attrs.clone()),
            signature_algorithm: AlgorithmIdentifierOwned {
                oid: rfc5912::SHA_256_WITH_RSA_ENCRYPTION,
                parameters: Some(Any::null()),
            },
            signature: OctetString::new(signature.to_vec())?,
            unsigned_attrs,
        };
        let mut certs = vec![CertificateChoices::Certificate(self.cert.clone())];
        for c in &self.chain {
            certs.push(CertificateChoices::Certificate(c.clone()));
        }
        // RFC 5652 §5.1: version 3 when eContentType is not id-data.
        let version = if self.econtent_type == rfc5911::ID_DATA {
            CmsVersion::V1
        } else {
            CmsVersion::V3
        };
        let econtent = match &self.econtent {
            Some(c) => Some(Any::encode_from(&OctetString::new(c.clone())?)?),
            None => None,
        };
        let signed_data = SignedData {
            version,
            digest_algorithms: SetOfVec::try_from(vec![sha256_alg()])?,
            encap_content_info: EncapsulatedContentInfo {
                econtent_type: self.econtent_type,
                econtent,
            },
            certificates: Some(CertificateSet(SetOfVec::try_from(certs)?)),
            crls: None,
            signer_infos: SignerInfos(SetOfVec::try_from(vec![signer_info])?),
        };
        let ci = ContentInfo {
            content_type: rfc5911::ID_SIGNED_DATA,
            content: Any::encode_from(&signed_data)?,
        };
        Ok(ci.to_der()?)
    }
}

/// Builds a DER `ContentInfo(SignedData)` detached signature over `content`.
pub fn sign_detached(content: &[u8], signer: &dyn Signer) -> Result<Vec<u8>> {
    sign_detached_with(content, signer, None)
}

/// [`sign_detached`] plus, with a time-stamp authority, a signature
/// time-stamp (CAdES baseline B-T).
pub fn sign_detached_with(
    content: &[u8],
    signer: &dyn Signer,
    tsa: Option<&dyn TimestampClient>,
) -> Result<Vec<u8>> {
    let prepared = PreparedCms::detached(
        &sha256(content),
        signer.certificate_der(),
        &signer.chain_der(),
    )?;
    let signature = signer.sign_sha256(&prepared.digest()?)?;
    let token = match tsa {
        Some(client) => Some(timestamp::timestamp(
            client,
            &signature,
            timestamp::fresh_nonce(&signature),
        )?),
        None => None,
    };
    prepared.assemble(&signature, token.as_deref())
}

/// Result of verifying a detached CMS signature.
#[derive(Debug, Clone)]
pub struct VerifiedSigner {
    /// DER of the signing certificate.
    pub certificate_der: Vec<u8>,
    /// Signer subject, RFC 4514 string.
    pub subject: String,
    /// Every certificate embedded in the `SignedData` (DER), for path building.
    pub certificates: Vec<Vec<u8>>,
    /// Verified signature time-stamp (PAdES B-T), if present.
    pub timestamp: Option<TimestampInfo>,
    /// Hash the signature relies on (the weaker of digestAlgorithm and signatureAlgorithm), checked
    /// against [`crate::policy`] at the validation time.
    pub digest_alg: HashAlg,
}

/// Parts of a verified `SignedData`.
pub(crate) struct VerifiedCms {
    pub signer: Certificate,
    /// Weaker of the digest and signature hashes.
    pub digest_alg: HashAlg,
    pub certificates: Vec<Vec<u8>>,
    pub econtent_type: ObjectIdentifier,
    /// Encapsulated content (for encapsulated signatures).
    pub econtent: Option<Vec<u8>>,
    pub signature: Vec<u8>,
    pub unsigned_attrs: Option<Attributes>,
}

fn ski_of(cert: &Certificate) -> Option<Vec<u8>> {
    cert.tbs_certificate
        .get::<SubjectKeyIdentifier>()
        .ok()
        .flatten()
        .map(|(_, ski)| ski.0.as_bytes().to_vec())
}

/// Verifies the first `SignerInfo` of a `ContentInfo(SignedData)`: the
/// message-digest attribute against `detached` content (or the
/// encapsulated content) and the RSA signature over the signed attributes.
/// Certificate trust is checked separately ([`crate::trust`]).
pub(crate) fn verify_signed_data(cms_der: &[u8], detached: Option<&[u8]>) -> Result<VerifiedCms> {
    let ci = ContentInfo::from_der(cms_der)?;
    if ci.content_type != rfc5911::ID_SIGNED_DATA {
        return Err(SignError::Malformed("not a SignedData".into()));
    }
    let sd: SignedData = ci.content.decode_as()?;
    let si = sd
        .signer_infos
        .0
        .iter()
        .next()
        .ok_or_else(|| SignError::Malformed("no SignerInfo".into()))?;
    let embedded: Vec<Certificate> = sd
        .certificates
        .as_ref()
        .map(|cs| {
            cs.0.iter()
                .filter_map(|c| match c {
                    CertificateChoices::Certificate(c) => Some(c.clone()),
                    CertificateChoices::Other(_) => None,
                })
                .collect()
        })
        .unwrap_or_default();
    let cert = embedded
        .iter()
        .find(|c| match &si.sid {
            SignerIdentifier::IssuerAndSerialNumber(ias) => {
                c.tbs_certificate.serial_number == ias.serial_number
                    && c.tbs_certificate.issuer == ias.issuer
            }
            SignerIdentifier::SubjectKeyIdentifier(ski) => {
                ski_of(c).as_deref() == Some(ski.0.as_bytes())
            }
        })
        .cloned()
        .ok_or_else(|| SignError::Malformed("signer certificate not embedded".into()))?;

    let econtent = match &sd.encap_content_info.econtent {
        Some(any) => Some(any.decode_as::<OctetString>()?.into_bytes()),
        None => None,
    };
    let content = match (detached, &econtent) {
        (Some(d), _) => d,
        (None, Some(e)) => e.as_slice(),
        (None, None) => return Err(SignError::Malformed("no content to verify".into())),
    };
    let digest_alg = HashAlg::from_digest_oid(si.digest_alg.oid)?;
    let attrs = si
        .signed_attrs
        .as_ref()
        .ok_or_else(|| SignError::Malformed("no signed attributes".into()))?;
    let first_value = |oid: ObjectIdentifier| {
        attrs
            .iter()
            .find(|a| a.oid == oid)
            .and_then(|a| a.values.iter().next())
    };
    let md: OctetString = first_value(rfc5911::ID_MESSAGE_DIGEST)
        .ok_or_else(|| SignError::Malformed("no message-digest attribute".into()))?
        .decode_as()?;
    if md.as_bytes() != digest_alg.digest(content).as_slice() {
        return Err(SignError::Invalid("content digest mismatch".into()));
    }
    // RFC 5652 §11.1: the content-type attribute must match eContentType.
    if let Some(ct) = first_value(rfc5911::ID_CONTENT_TYPE) {
        let ct: ObjectIdentifier = ct.decode_as()?;
        if ct != sd.encap_content_info.econtent_type {
            return Err(SignError::Invalid("content-type attribute mismatch".into()));
        }
    }
    if first_value(ID_AA_SIGNING_CERTIFICATE_V2).is_none()
        && first_value(ID_AA_SIGNING_CERTIFICATE).is_none()
        && sd.encap_content_info.econtent_type != rfc5911::ID_DATA
    {
        // RFC 3161 §2.4.2: tokens must identify their signing certificate.
        return Err(SignError::Invalid(
            "no signing-certificate attribute".into(),
        ));
    }
    // RFC 4055 §5: either `rsaEncryption` (hash from digestAlgorithm) or `shaNWithRSAEncryption`.
    let sig_alg = if si.signature_algorithm.oid == rfc5912::RSA_ENCRYPTION {
        digest_alg
    } else {
        HashAlg::from_rsa_signature_oid(si.signature_algorithm.oid)?
    };
    verify_rsa(
        &cert,
        sig_alg,
        &sig_alg.digest(&attrs.to_der()?),
        si.signature.as_bytes(),
    )?;
    let certificates = embedded
        .iter()
        .map(Encode::to_der)
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let weakest = if digest_alg == HashAlg::Sha1 || sig_alg == HashAlg::Sha1 {
        HashAlg::Sha1
    } else {
        digest_alg
    };
    Ok(VerifiedCms {
        signer: cert,
        digest_alg: weakest,
        certificates,
        econtent_type: sd.encap_content_info.econtent_type,
        econtent,
        signature: si.signature.as_bytes().to_vec(),
        unsigned_attrs: si.unsigned_attrs.clone(),
    })
}

/// Verifies a detached `ContentInfo(SignedData)` over `content` (message
/// digest + RSA signature, plus the signature time-stamp if present;
/// certificate trust is checked separately with [`crate::trust`]).
pub fn verify_detached(cms_der: &[u8], content: &[u8]) -> Result<VerifiedSigner> {
    let v = verify_signed_data(cms_der, Some(content))?;
    let timestamp = match v.unsigned_attrs.as_ref().and_then(|attrs| {
        attrs
            .iter()
            .find(|a| a.oid == ID_AA_SIGNATURE_TIME_STAMP_TOKEN)
            .and_then(|a| a.values.iter().next())
    }) {
        Some(token) => Some(timestamp::verify_token(&token.to_der()?, &v.signature)?),
        None => None,
    };
    Ok(VerifiedSigner {
        subject: v.signer.tbs_certificate.subject.to_string(),
        certificate_der: v.signer.to_der()?,
        certificates: v.certificates,
        timestamp,
        digest_alg: v.digest_alg,
    })
}
