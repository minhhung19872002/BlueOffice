//! Certificate revocation lists fetched from the certificate's CRL
//! distribution points (RFC 5280 §4.2.1.13, HTTP per RFC 5280 §4.2.1.13 /
//! RFC 2585 §3: GET, DER `application/pkix-crl`; PEM is accepted too). The
//! lists are checked like loaded ones ([`crate::trust`]).

use crate::http::HttpTransport;
use crate::{pem_blocks, Result, SignError};
use der::asn1::BitString;
use der::Sequence;
use spki::AlgorithmIdentifierOwned;
use x509_cert::crl::RevokedCert;
use x509_cert::ext::pkix::crl::dp::DistributionPoint;
use x509_cert::ext::pkix::name::{DistributionPointName, GeneralName};
use x509_cert::ext::pkix::CrlDistributionPoints;
use x509_cert::ext::Extensions;
use x509_cert::name::Name;
use x509_cert::time::Time;
use x509_cert::Certificate;

/// HTTP(S) URLs of the CRL distribution points of `cert` (full names only).
#[must_use]
pub fn distribution_urls(cert: &Certificate) -> Vec<String> {
    let Ok(Some((_, dps))) = cert.tbs_certificate.get::<CrlDistributionPoints>() else {
        return Vec::new();
    };
    dps.0
        .iter()
        .filter_map(|dp: &DistributionPoint| match &dp.distribution_point {
            Some(DistributionPointName::FullName(names)) => Some(names),
            _ => None,
        })
        .flatten()
        .filter_map(|n| match n {
            GeneralName::UniformResourceIdentifier(u) => Some(u.to_string()),
            _ => None,
        })
        .filter(|u| u.starts_with("http://") || u.starts_with("https://"))
        .collect()
}

/// Downloads the CRL at `url`; returns its DER (a PEM `X509 CRL` is decoded).
pub fn fetch(transport: &dyn HttpTransport, url: &str) -> Result<Vec<u8>> {
    let resp = transport.get(url)?;
    if resp.status != 200 {
        return Err(SignError::Invalid(format!(
            "CRL download returned HTTP {}",
            resp.status
        )));
    }
    match std::str::from_utf8(&resp.body) {
        Ok(text) if text.contains("-----BEGIN X509 CRL-----") => pem_blocks(text, "X509 CRL")?
            .into_iter()
            .next()
            .ok_or_else(|| SignError::Malformed("empty CRL PEM".into())),
        _ => Ok(resp.body),
    }
}

/// TBSCertList (RFC 5280 §5.1.2). `version` is optional: a v1 list (no
/// extensions) omits it (§5.1.2.1), which the `x509-cert` type does not allow.
/// Re-encoding gives back the original bytes, so the signature checks.
#[derive(Clone, Debug, Eq, PartialEq, Sequence)]
pub struct TbsCertList {
    /// `v2` when present.
    #[asn1(optional = "true")]
    pub version: Option<x509_cert::Version>,
    /// Signature algorithm (repeated outside).
    pub signature: AlgorithmIdentifierOwned,
    /// Issuer of the list.
    pub issuer: Name,
    /// Issue date.
    pub this_update: Time,
    /// Date of the next list.
    #[asn1(optional = "true")]
    pub next_update: Option<Time>,
    /// Revoked certificates.
    #[asn1(optional = "true")]
    pub revoked_certificates: Option<Vec<RevokedCert>>,
    /// crlExtensions (v2).
    #[asn1(context_specific = "0", tag_mode = "EXPLICIT", optional = "true")]
    pub crl_extensions: Option<Extensions>,
}

/// CertificateList (RFC 5280 §5.1), v1 or v2.
#[derive(Clone, Debug, Eq, PartialEq, Sequence)]
pub struct CertificateList {
    /// Signed part.
    pub tbs_cert_list: TbsCertList,
    /// Signature algorithm.
    pub signature_algorithm: AlgorithmIdentifierOwned,
    /// Signature over the DER of `tbs_cert_list`.
    pub signature: BitString,
}
