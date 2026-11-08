//! Certificate path validation against configured trust anchors.
//!
//! A simplified RFC 5280 §6 path validation: the path is built by matching
//! issuer/subject names (RFC 5280 §4.1.2.4), each certificate signature is
//! checked with the issuer key, every certificate (and the anchor) must be
//! within its validity period at the validation time (§4.1.2.5), CA
//! certificates need `basicConstraints cA` (§4.2.1.9, including
//! `pathLenConstraint`) and `keyCertSign` when a key usage is present
//! (§4.2.1.3); the end-entity key usage must allow signatures and a TSA
//! certificate must carry `id-kp-timeStamping` (RFC 3161 §2.3).
//!
//! Trust anchors are configuration (e.g. the Vietnamese national root CA
//! certificate supplied by the deployment); none is built in.
//!
//! Revocation: certificate revocation lists (RFC 5280 §5) loaded into the
//! store are checked for every certificate of the path (§6.3): a CRL counts
//! when its issuer is the certificate's issuer and its signature verifies with
//! that issuer's key; a certificate listed with a revocation date at or before
//! the validation time is revoked, and a CRL whose `nextUpdate` is before the
//! validation time is reported as stale. Without a CRL for an issuer the
//! status is unknown and not reported as a problem; OCSP is not supported.

use crate::{pem_blocks, verify_rsa, HashAlg, Result, SignError};
use const_oid::db::rfc5280;
use der::{Decode, Encode};
use std::fmt;
use std::path::Path;
use std::time::SystemTime;
use x509_cert::crl::CertificateList;
use x509_cert::ext::pkix::{BasicConstraints, ExtendedKeyUsage, KeyUsage};
use x509_cert::Certificate;

/// Maximum certificates in a path (anchor excluded).
const MAX_PATH: usize = 8;

/// What the end-entity certificate is validated for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Purpose {
    /// Document signatures: key usage digitalSignature or nonRepudiation.
    DocumentSigning,
    /// RFC 3161 time-stamp tokens: EKU id-kp-timeStamping.
    TimeStamping,
}

/// Configured trust anchors (root CA certificates).
#[derive(Debug, Clone, Default)]
pub struct TrustStore {
    anchors: Vec<Certificate>,
    crls: Vec<CertificateList>,
}

impl TrustStore {
    #[must_use]
    /// An empty store (nothing is trusted).
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds one DER certificate as trust anchor.
    pub fn add_der(&mut self, der: &[u8]) -> Result<()> {
        self.anchors.push(Certificate::from_der(der)?);
        Ok(())
    }

    /// Adds one DER certificate revocation list (RFC 5280 §5.1).
    pub fn add_crl_der(&mut self, der: &[u8]) -> Result<()> {
        self.crls.push(CertificateList::from_der(der)?);
        Ok(())
    }

    /// Adds every certificate and CRL (`X509 CRL`) of a PEM bundle, or a single
    /// DER certificate or CRL; returns how many were added.
    pub fn add_bytes(&mut self, data: &[u8]) -> Result<usize> {
        match std::str::from_utf8(data) {
            Ok(text) if text.contains("-----BEGIN ") => {
                let certs = if text.contains("-----BEGIN CERTIFICATE-----") {
                    pem_blocks(text, "CERTIFICATE")?
                } else {
                    Vec::new()
                };
                let crls = if text.contains("-----BEGIN X509 CRL-----") {
                    pem_blocks(text, "X509 CRL")?
                } else {
                    Vec::new()
                };
                if certs.is_empty() && crls.is_empty() {
                    return Err(SignError::Malformed(
                        "no CERTIFICATE or X509 CRL block".into(),
                    ));
                }
                for b in &certs {
                    self.add_der(b)?;
                }
                for b in &crls {
                    self.add_crl_der(b)?;
                }
                Ok(certs.len() + crls.len())
            }
            _ => {
                if Certificate::from_der(data).is_ok() {
                    self.add_der(data)?;
                } else {
                    self.add_crl_der(data)?;
                }
                Ok(1)
            }
        }
    }

    /// Loads a PEM bundle / DER file, or every `.pem/.crt/.cer/.der/.crl` file of a directory.
    pub fn add_path(&mut self, path: &Path) -> Result<usize> {
        let io = |e: std::io::Error| SignError::Malformed(format!("{}: {e}", path.display()));
        if path.is_dir() {
            let mut n = 0;
            let mut entries: Vec<_> = std::fs::read_dir(path)
                .map_err(io)?
                .filter_map(std::result::Result::ok)
                .map(|e| e.path())
                .filter(|p| {
                    p.extension().and_then(|e| e.to_str()).is_some_and(|e| {
                        matches!(
                            e.to_ascii_lowercase().as_str(),
                            "pem" | "crt" | "cer" | "der" | "crl"
                        )
                    })
                })
                .collect();
            entries.sort();
            for p in entries {
                n += self.add_path(&p)?;
            }
            Ok(n)
        } else {
            let data = std::fs::read(path).map_err(io)?;
            self.add_bytes(&data)
                .map_err(|e| SignError::Malformed(format!("{}: {e}", path.display())))
        }
    }

    #[must_use]
    /// Number of anchors.
    pub fn len(&self) -> usize {
        self.anchors.len()
    }

    #[must_use]
    /// Number of certificate revocation lists.
    pub fn crl_count(&self) -> usize {
        self.crls.len()
    }

    #[must_use]
    /// No anchor configured.
    pub fn is_empty(&self) -> bool {
        self.anchors.is_empty()
    }

    /// Subjects of the anchors (RFC 4514).
    #[must_use]
    pub fn subjects(&self) -> Vec<String> {
        self.anchors
            .iter()
            .map(|c| c.tbs_certificate.subject.to_string())
            .collect()
    }
}

/// A reason a path is not trusted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChainProblem {
    /// No trust anchor is configured.
    NoTrustAnchors,
    /// No issuer (intermediate or anchor) found for this certificate.
    UnknownIssuer {
        /// Subject (RFC 4514) of the certificate concerned.
        subject: String,
    },
    /// The certificate is past its `notAfter` at the validation time.
    Expired {
        /// Subject (RFC 4514) of the certificate concerned.
        subject: String,
    },
    /// The certificate is before its `notBefore` at the validation time.
    NotYetValid {
        /// Subject (RFC 4514) of the certificate concerned.
        subject: String,
    },
    /// The certificate signature does not verify with the issuer key.
    BadSignature {
        /// Subject (RFC 4514) of the certificate concerned.
        subject: String,
    },
    /// An issuing certificate is not a CA (basicConstraints) or exceeds its path length.
    NotCa {
        /// Subject (RFC 4514) of the certificate concerned.
        subject: String,
    },
    /// Key usage does not allow the use (keyCertSign / digitalSignature).
    KeyUsage {
        /// Subject (RFC 4514) of the certificate concerned.
        subject: String,
    },
    /// A TSA certificate without id-kp-timeStamping.
    ExtendedKeyUsage {
        /// Subject (RFC 4514) of the certificate concerned.
        subject: String,
    },
    /// Algorithm or key type not supported by the validator.
    Unsupported {
        /// Subject (RFC 4514) of the certificate concerned.
        subject: String,
        /// What is unsupported (algorithm, key type).
        what: String,
    },
    /// The certificate is listed in its issuer's CRL (RFC 5280 §5.3).
    Revoked {
        /// Subject (RFC 4514) of the certificate concerned.
        subject: String,
        /// Revocation date (`YYYY-MM-DDThh:mm:ssZ`).
        date: String,
    },
    /// The issuer's CRL had expired (`nextUpdate`) at the validation time.
    CrlStale {
        /// Subject (RFC 4514) of the CRL issuer.
        subject: String,
    },
    /// The path is longer than the validator accepts.
    PathTooLong,
    /// A certificate could not be decoded.
    Malformed(String),
}

impl ChainProblem {
    /// Stable code for UIs (i18n key suffix).
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::NoTrustAnchors => "noTrustAnchors",
            Self::UnknownIssuer { .. } => "unknownIssuer",
            Self::Expired { .. } => "expired",
            Self::NotYetValid { .. } => "notYetValid",
            Self::BadSignature { .. } => "badSignature",
            Self::NotCa { .. } => "notCa",
            Self::KeyUsage { .. } => "keyUsage",
            Self::ExtendedKeyUsage { .. } => "extendedKeyUsage",
            Self::Unsupported { .. } => "unsupported",
            Self::Revoked { .. } => "revoked",
            Self::CrlStale { .. } => "crlStale",
            Self::PathTooLong => "pathTooLong",
            Self::Malformed(_) => "malformed",
        }
    }

    /// Subject of the certificate concerned, if any.
    #[must_use]
    pub fn subject(&self) -> Option<&str> {
        match self {
            Self::UnknownIssuer { subject }
            | Self::Expired { subject }
            | Self::NotYetValid { subject }
            | Self::BadSignature { subject }
            | Self::NotCa { subject }
            | Self::KeyUsage { subject }
            | Self::ExtendedKeyUsage { subject }
            | Self::Revoked { subject, .. }
            | Self::CrlStale { subject }
            | Self::Unsupported { subject, .. } => Some(subject),
            _ => None,
        }
    }
}

impl fmt::Display for ChainProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoTrustAnchors => write!(f, "no trust anchor configured"),
            Self::UnknownIssuer { subject } => {
                write!(f, "issuer of \"{subject}\" is not a trusted or embedded CA")
            }
            Self::Expired { subject } => write!(f, "\"{subject}\" has expired"),
            Self::NotYetValid { subject } => write!(f, "\"{subject}\" is not yet valid"),
            Self::BadSignature { subject } => {
                write!(f, "signature on \"{subject}\" does not verify")
            }
            Self::NotCa { subject } => {
                write!(f, "\"{subject}\" is not allowed to issue certificates")
            }
            Self::KeyUsage { subject } => {
                write!(f, "key usage of \"{subject}\" does not allow this use")
            }
            Self::ExtendedKeyUsage { subject } => {
                write!(f, "\"{subject}\" is not a time-stamping certificate")
            }
            Self::Unsupported { subject, what } => write!(f, "\"{subject}\": unsupported {what}"),
            Self::Revoked { subject, date } => {
                write!(f, "\"{subject}\" was revoked on {date}")
            }
            Self::CrlStale { subject } => {
                write!(f, "the revocation list of \"{subject}\" is out of date")
            }
            Self::PathTooLong => write!(f, "certificate path too long"),
            Self::Malformed(m) => write!(f, "malformed certificate: {m}"),
        }
    }
}

/// Result of [`validate_chain`].
#[derive(Debug, Clone)]
pub struct ChainReport {
    /// Path built to an anchor with no problem.
    pub trusted: bool,
    /// Subjects from the end entity up to (excluding) the anchor.
    pub path: Vec<String>,
    /// Anchor subject the path ends at.
    pub anchor: Option<String>,
    /// Why the path is not trusted (empty when trusted).
    pub problems: Vec<ChainProblem>,
}

fn subject(c: &Certificate) -> String {
    c.tbs_certificate.subject.to_string()
}

/// Checks `child`'s signature with `issuer`'s public key.
fn signed_by(child: &Certificate, issuer: &Certificate) -> std::result::Result<(), ChainProblem> {
    let alg = HashAlg::from_rsa_signature_oid(child.signature_algorithm.oid).map_err(|_| {
        ChainProblem::Unsupported {
            subject: subject(child),
            what: format!("signature algorithm {}", child.signature_algorithm.oid),
        }
    })?;
    let tbs = child
        .tbs_certificate
        .to_der()
        .map_err(|e| ChainProblem::Malformed(e.to_string()))?;
    let sig = child
        .signature
        .as_bytes()
        .ok_or_else(|| ChainProblem::Malformed("signature bit string".into()))?;
    match verify_rsa(issuer, alg, &alg.digest(&tbs), sig) {
        Ok(()) => Ok(()),
        Err(SignError::Unsupported(what)) => Err(ChainProblem::Unsupported {
            subject: subject(issuer),
            what,
        }),
        Err(_) => Err(ChainProblem::BadSignature {
            subject: subject(child),
        }),
    }
}

/// Checks a CRL's signature with `issuer`'s key (RFC 5280 §6.3.3 (f)–(g)).
fn crl_signed_by(crl: &CertificateList, issuer: &Certificate) -> bool {
    let Ok(alg) = HashAlg::from_rsa_signature_oid(crl.signature_algorithm.oid) else {
        return false;
    };
    let (Ok(tbs), Some(sig)) = (crl.tbs_cert_list.to_der(), crl.signature.as_bytes()) else {
        return false;
    };
    verify_rsa(issuer, alg, &alg.digest(&tbs), sig).is_ok()
}

/// Revocation status of `c` (issued by `issuer`) from the store's CRLs
/// (RFC 5280 §6.3; module documentation).
fn check_revocation(
    c: &Certificate,
    issuer: &Certificate,
    store: &TrustStore,
    at: SystemTime,
    problems: &mut Vec<ChainProblem>,
) {
    let crls: Vec<&CertificateList> = store
        .crls
        .iter()
        .filter(|l| l.tbs_cert_list.issuer == c.tbs_certificate.issuer)
        .filter(|l| crl_signed_by(l, issuer))
        .collect();
    if crls.is_empty() {
        return;
    }
    let serial = &c.tbs_certificate.serial_number;
    let revoked = crls
        .iter()
        .flat_map(|l| l.tbs_cert_list.revoked_certificates.iter().flatten())
        .filter(|r| &r.serial_number == serial)
        .map(|r| r.revocation_date.to_system_time())
        .filter(|d| *d <= at)
        .min();
    if let Some(d) = revoked {
        problems.push(ChainProblem::Revoked {
            subject: subject(c),
            date: crate::iso_utc(crate::unix_secs(d)),
        });
        return;
    }
    // Fresh when one CRL (issued by `at` or later) still covers `at`.
    let fresh = crls.iter().any(|l| {
        l.tbs_cert_list
            .next_update
            .is_none_or(|n| n.to_system_time() >= at)
    });
    if !fresh {
        problems.push(ChainProblem::CrlStale {
            subject: subject(issuer),
        });
    }
}

fn check_validity(c: &Certificate, at: SystemTime, problems: &mut Vec<ChainProblem>) {
    let v = &c.tbs_certificate.validity;
    if at < v.not_before.to_system_time() {
        problems.push(ChainProblem::NotYetValid {
            subject: subject(c),
        });
    } else if at > v.not_after.to_system_time() {
        problems.push(ChainProblem::Expired {
            subject: subject(c),
        });
    }
}

/// Checks a CA certificate that has `below` intermediate CAs under it.
fn check_ca(c: &Certificate, below: usize, problems: &mut Vec<ChainProblem>) {
    let bc = c.tbs_certificate.get::<BasicConstraints>().ok().flatten();
    match bc {
        Some((_, bc)) if bc.ca => {
            if bc
                .path_len_constraint
                .is_some_and(|max| below > usize::from(max))
            {
                problems.push(ChainProblem::NotCa {
                    subject: subject(c),
                });
            }
        }
        _ => problems.push(ChainProblem::NotCa {
            subject: subject(c),
        }),
    }
    if let Ok(Some((_, ku))) = c.tbs_certificate.get::<KeyUsage>() {
        if !ku.key_cert_sign() {
            problems.push(ChainProblem::KeyUsage {
                subject: subject(c),
            });
        }
    }
}

fn check_end_entity(c: &Certificate, purpose: Purpose, problems: &mut Vec<ChainProblem>) {
    if let Ok(Some((_, ku))) = c.tbs_certificate.get::<KeyUsage>() {
        if !ku.digital_signature() && !ku.non_repudiation() {
            problems.push(ChainProblem::KeyUsage {
                subject: subject(c),
            });
        }
    }
    if purpose == Purpose::TimeStamping {
        let eku = c.tbs_certificate.get::<ExtendedKeyUsage>().ok().flatten();
        if !eku.is_some_and(|(_, e)| e.0.contains(&rfc5280::ID_KP_TIME_STAMPING)) {
            problems.push(ChainProblem::ExtendedKeyUsage {
                subject: subject(c),
            });
        }
    }
}

/// Validates `leaf_der` against `store` at time `at`, building the path
/// through the certificates of `pool` (those embedded in the signature).
#[must_use]
pub fn validate_chain(
    leaf_der: &[u8],
    pool: &[Vec<u8>],
    store: &TrustStore,
    at: SystemTime,
    purpose: Purpose,
) -> ChainReport {
    let mut report = ChainReport {
        trusted: false,
        path: Vec::new(),
        anchor: None,
        problems: Vec::new(),
    };
    let leaf = match Certificate::from_der(leaf_der) {
        Ok(c) => c,
        Err(e) => {
            report.problems.push(ChainProblem::Malformed(e.to_string()));
            return report;
        }
    };
    if store.is_empty() {
        report.problems.push(ChainProblem::NoTrustAnchors);
    }
    let pool: Vec<Certificate> = pool
        .iter()
        .filter_map(|d| Certificate::from_der(d).ok())
        .collect();

    let mut path = vec![leaf];
    let mut anchor: Option<&Certificate> = None;
    loop {
        let cur = path[path.len() - 1].clone();
        // A certificate configured as an anchor is trusted as such (an
        // embedded copy of the root is dropped; an end entity configured
        // directly as anchor stays in the path).
        if let Some(a) = store.anchors.iter().find(|a| **a == cur) {
            if path.len() > 1 {
                path.pop();
            }
            anchor = Some(a);
            break;
        }
        let issuer_name = &cur.tbs_certificate.issuer;
        let mut last_err = None;
        if let Some(a) = store
            .anchors
            .iter()
            .filter(|a| &a.tbs_certificate.subject == issuer_name)
            .find(|a| match signed_by(&cur, a) {
                Ok(()) => true,
                Err(e) => {
                    last_err = Some(e);
                    false
                }
            })
        {
            anchor = Some(a);
            break;
        }
        // Self-signed but not an anchor: stop.
        if cur.tbs_certificate.issuer == cur.tbs_certificate.subject {
            report
                .problems
                .push(last_err.unwrap_or(ChainProblem::UnknownIssuer {
                    subject: subject(&cur),
                }));
            break;
        }
        let next = pool
            .iter()
            .filter(|c| &c.tbs_certificate.subject == issuer_name && !path.contains(c))
            .find(|c| match signed_by(&cur, c) {
                Ok(()) => true,
                Err(e) => {
                    last_err = Some(e);
                    false
                }
            });
        match next {
            Some(c) if path.len() < MAX_PATH => path.push(c.clone()),
            Some(_) => {
                report.problems.push(ChainProblem::PathTooLong);
                break;
            }
            None => {
                report
                    .problems
                    .push(last_err.unwrap_or(ChainProblem::UnknownIssuer {
                        subject: subject(&cur),
                    }));
                break;
            }
        }
    }

    for (i, c) in path.iter().enumerate() {
        check_validity(c, at, &mut report.problems);
        if i == 0 {
            check_end_entity(c, purpose, &mut report.problems);
        } else {
            check_ca(c, i - 1, &mut report.problems);
        }
    }
    // Revocation of each certificate by its issuer (the next one, or the anchor).
    for (i, c) in path.iter().enumerate() {
        let issuer = path.get(i + 1).or(anchor.filter(|a| *a != c));
        if let Some(issuer) = issuer {
            check_revocation(c, issuer, store, at, &mut report.problems);
        }
    }
    if let Some(a) = anchor {
        if path.first() != Some(a) {
            check_validity(a, at, &mut report.problems);
            check_ca(a, path.len().saturating_sub(1), &mut report.problems);
        }
        report.anchor = Some(subject(a));
    }
    report.path = path.iter().map(subject).collect();
    report.trusted = anchor.is_some() && report.problems.is_empty();
    report
}
