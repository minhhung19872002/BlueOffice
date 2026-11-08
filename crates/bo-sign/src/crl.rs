//! Certificate revocation lists fetched from the certificate's CRL
//! distribution points (RFC 5280 §4.2.1.13, HTTP per RFC 5280 §4.2.1.13 /
//! RFC 2585 §3: GET, DER `application/pkix-crl`; PEM is accepted too). The
//! lists are checked like loaded ones ([`crate::trust`]).

use crate::http::HttpTransport;
use crate::{pem_blocks, Result, SignError};
use x509_cert::ext::pkix::crl::dp::DistributionPoint;
use x509_cert::ext::pkix::name::{DistributionPointName, GeneralName};
use x509_cert::ext::pkix::CrlDistributionPoints;
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
