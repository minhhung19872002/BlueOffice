//! Detached CMS `SignedData` (RFC 5652) with the signed attributes required
//! by CAdES/PAdES baseline B-B (ETSI EN 319 122-1 §5.2, EN 319 142-1 §6.3):
//! content-type, message-digest and signing-certificate-v2. Signing time is
//! carried by the PDF signature dictionary (`/M`), not as a CMS attribute.

use crate::{sha256, verify_with_cert, Result, SignError, Signer};
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
use x509_cert::attr::{Attribute, AttributeValue};
use x509_cert::Certificate;

/// id-aa-signingCertificateV2 (RFC 5035 §3).
const ID_AA_SIGNING_CERTIFICATE_V2: ObjectIdentifier =
    ObjectIdentifier::new_unwrap("1.2.840.113549.1.9.16.2.47");

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

fn attribute(oid: ObjectIdentifier, value_der: &[u8]) -> Result<Attribute> {
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

/// Builds a DER `ContentInfo(SignedData)` detached signature over `content`.
pub fn sign_detached(content: &[u8], signer: &dyn Signer) -> Result<Vec<u8>> {
    let cert = Certificate::from_der(signer.certificate_der())?;
    let digest = sha256(content);
    let cert_hash = sha256(signer.certificate_der());

    let signed_attrs = SetOfVec::try_from(vec![
        attribute(rfc5911::ID_CONTENT_TYPE, &rfc5911::ID_DATA.to_der()?)?,
        attribute(
            rfc5911::ID_MESSAGE_DIGEST,
            &OctetString::new(digest.to_vec())?.to_der()?,
        )?,
        attribute(
            ID_AA_SIGNING_CERTIFICATE_V2,
            &SigningCertificateV2 {
                certs: vec![EssCertIdV2 {
                    cert_hash: OctetString::new(cert_hash.to_vec())?,
                }],
            }
            .to_der()?,
        )?,
    ])?;
    // RFC 5652 §5.4: the signature covers the DER of the SET OF attributes.
    let attrs_digest = sha256(&signed_attrs.to_der()?);
    let signature = signer.sign_sha256(&attrs_digest)?;

    let signer_info = SignerInfo {
        version: CmsVersion::V1,
        sid: SignerIdentifier::IssuerAndSerialNumber(IssuerAndSerialNumber {
            issuer: cert.tbs_certificate.issuer.clone(),
            serial_number: cert.tbs_certificate.serial_number.clone(),
        }),
        digest_alg: sha256_alg(),
        signed_attrs: Some(signed_attrs),
        signature_algorithm: AlgorithmIdentifierOwned {
            oid: rfc5912::SHA_256_WITH_RSA_ENCRYPTION,
            parameters: Some(Any::null()),
        },
        signature: OctetString::new(signature)?,
        unsigned_attrs: None,
    };

    let mut certs = vec![CertificateChoices::Certificate(cert)];
    for der_cert in signer.chain_der() {
        certs.push(CertificateChoices::Certificate(Certificate::from_der(
            &der_cert,
        )?));
    }
    let signed_data = SignedData {
        version: CmsVersion::V1,
        digest_algorithms: SetOfVec::try_from(vec![sha256_alg()])?,
        encap_content_info: EncapsulatedContentInfo {
            econtent_type: rfc5911::ID_DATA,
            econtent: None,
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

/// Result of verifying a detached CMS signature.
#[derive(Debug, Clone)]
pub struct VerifiedSigner {
    /// DER of the signing certificate.
    pub certificate_der: Vec<u8>,
    /// Signer subject, RFC 4514 string.
    pub subject: String,
}

/// Verifies a detached `ContentInfo(SignedData)` over `content`
/// (message digest + RSA signature; certificate trust is the caller's job).
pub fn verify_detached(cms_der: &[u8], content: &[u8]) -> Result<VerifiedSigner> {
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
    let SignerIdentifier::IssuerAndSerialNumber(ias) = &si.sid else {
        return Err(SignError::Unsupported(
            "subjectKeyIdentifier signer id".into(),
        ));
    };
    let cert = sd
        .certificates
        .as_ref()
        .and_then(|cs| {
            cs.0.iter().find_map(|c| match c {
                CertificateChoices::Certificate(c)
                    if c.tbs_certificate.serial_number == ias.serial_number
                        && c.tbs_certificate.issuer == ias.issuer =>
                {
                    Some(c.clone())
                }
                _ => None,
            })
        })
        .ok_or_else(|| SignError::Malformed("signer certificate not embedded".into()))?;
    let attrs = si
        .signed_attrs
        .as_ref()
        .ok_or_else(|| SignError::Malformed("no signed attributes".into()))?;
    let md = attrs
        .iter()
        .find(|a| a.oid == rfc5911::ID_MESSAGE_DIGEST)
        .and_then(|a| a.values.iter().next())
        .ok_or_else(|| SignError::Malformed("no message-digest attribute".into()))?;
    let md: OctetString = md.decode_as()?;
    if md.as_bytes() != sha256(content) {
        return Err(SignError::Invalid("content digest mismatch".into()));
    }
    let cert_der = cert.to_der()?;
    verify_with_cert(
        &cert_der,
        &sha256(&attrs.to_der()?),
        si.signature.as_bytes(),
    )?;
    Ok(VerifiedSigner {
        subject: cert.tbs_certificate.subject.to_string(),
        certificate_der: cert_der,
    })
}
