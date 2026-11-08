//! One verification report for every front end (`bo-cli verify`, desktop,
//! browser): cryptographic checks, signature time-stamp, and certificate
//! paths of the signer and the TSA against the configured trust anchors.

use crate::timestamp::TimestampInfo;
use crate::trust::{validate_chain, ChainReport, Purpose, TrustStore};
use crate::{DocumentKind, Result, SignError};
use serde::Serialize;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// A chain problem for UIs: stable `code` (i18n key) + English message.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProblemReport {
    /// Stable problem code (`ChainProblem::code`), used as i18n key suffix.
    pub code: String,
    /// Subject of the certificate concerned, if any.
    pub subject: Option<String>,
    /// English description.
    pub message: String,
}

/// Certificate path result.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ChainSummary {
    /// A path to a trust anchor was built and every check passed.
    pub trusted: bool,
    /// Subjects from the end entity up to (excluding) the anchor.
    pub path: Vec<String>,
    /// Subject of the trust anchor the path ends at.
    pub anchor: Option<String>,
    /// Why the path is not trusted (empty when trusted).
    pub problems: Vec<ProblemReport>,
}

impl From<ChainReport> for ChainSummary {
    fn from(r: ChainReport) -> Self {
        Self {
            trusted: r.trusted,
            path: r.path,
            anchor: r.anchor,
            problems: r
                .problems
                .iter()
                .map(|p| ProblemReport {
                    code: p.code().to_owned(),
                    subject: p.subject().map(str::to_owned),
                    message: p.to_string(),
                })
                .collect(),
        }
    }
}

/// Verified signature time-stamp.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TimestampSummary {
    /// `YYYY-MM-DDThh:mm:ssZ`.
    pub time: String,
    /// Subject of the time-stamp authority certificate.
    pub tsa: String,
    /// Path of the TSA certificate, validated at the time-stamp time.
    pub chain: ChainSummary,
}

/// One cryptographically valid signature.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SignatureSummary {
    /// Signer certificate subject (RFC 4514).
    pub signer: String,
    /// Claimed signing time (OOXML `SignatureTime`); not trusted by itself.
    pub signing_time: Option<String>,
    /// PDF: the signature covers the whole file (no later changes).
    pub covers_whole_document: bool,
    /// Verified signature time-stamp (PAdES B-T / XAdES-T), if present.
    pub timestamp: Option<TimestampSummary>,
    /// Signer path, validated at the time-stamp time when the time-stamp is
    /// trusted, otherwise at the verification time.
    pub chain: ChainSummary,
    /// Validation time used for the signer path.
    pub validated_at: String,
}

/// Verification of a whole document.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DocumentReport {
    /// `pdf` / `ooxml`.
    pub kind: String,
    /// Every cryptographically valid signature, in document order.
    pub signatures: Vec<SignatureSummary>,
    /// Set when a signature is cryptographically invalid (document changed,
    /// broken signature) or the file cannot be read.
    pub error: Option<String>,
}

fn summarize(
    signer_der: &[u8],
    pool: &[Vec<u8>],
    ts: Option<&TimestampInfo>,
    store: &TrustStore,
    now: SystemTime,
) -> (Option<TimestampSummary>, ChainSummary, SystemTime) {
    let ts_summary = ts.map(|t| {
        let at = UNIX_EPOCH + Duration::from_secs(t.gen_time_unix);
        TimestampSummary {
            time: t.gen_time.clone(),
            tsa: t.tsa_subject.clone(),
            chain: validate_chain(
                &t.tsa_certificate_der,
                &t.certificates,
                store,
                at,
                Purpose::TimeStamping,
            )
            .into(),
        }
    });
    // A trusted time-stamp proves the signature existed at genTime: the
    // signer certificate must have been valid then (ETSI EN 319 102-1 §5.6.2.4).
    let at = match (ts, &ts_summary) {
        (Some(t), Some(s)) if s.chain.trusted => UNIX_EPOCH + Duration::from_secs(t.gen_time_unix),
        _ => now,
    };
    let chain = validate_chain(signer_der, pool, store, at, Purpose::DocumentSigning).into();
    (ts_summary, chain, at)
}

/// Verifies every signature of a PDF or OOXML file.
pub fn verify_document(
    bytes: &[u8],
    store: &TrustStore,
    now: SystemTime,
) -> Result<DocumentReport> {
    let kind = DocumentKind::sniff(bytes)
        .ok_or_else(|| SignError::Unsupported("not a PDF or OOXML file".into()))?;
    let mut report = DocumentReport {
        kind: match kind {
            DocumentKind::Pdf => "pdf",
            DocumentKind::Ooxml => "ooxml",
        }
        .to_owned(),
        signatures: Vec::new(),
        error: None,
    };
    match kind {
        DocumentKind::Pdf => match crate::pdf::verify_pdf(bytes) {
            Ok(sigs) => {
                for s in sigs {
                    let (timestamp, chain, at) = summarize(
                        &s.signer.certificate_der,
                        &s.signer.certificates,
                        s.signer.timestamp.as_ref(),
                        store,
                        now,
                    );
                    report.signatures.push(SignatureSummary {
                        signer: s.signer.subject,
                        signing_time: None,
                        covers_whole_document: s.covers_whole_file,
                        timestamp,
                        chain,
                        validated_at: crate::iso_utc(crate::unix_secs(at)),
                    });
                }
            }
            Err(e) => report.error = Some(e.to_string()),
        },
        DocumentKind::Ooxml => match crate::ooxml::verify_ooxml(bytes) {
            Ok(sigs) => {
                for s in sigs {
                    let (timestamp, chain, at) = summarize(
                        &s.certificate_der,
                        &s.certificates,
                        s.timestamp.as_ref(),
                        store,
                        now,
                    );
                    let signer =
                        <x509_cert::Certificate as der::Decode>::from_der(&s.certificate_der)
                            .map(|c| c.tbs_certificate.subject.to_string())
                            .unwrap_or_default();
                    report.signatures.push(SignatureSummary {
                        signer,
                        signing_time: s.signing_time,
                        covers_whole_document: true,
                        timestamp,
                        chain,
                        validated_at: crate::iso_utc(crate::unix_secs(at)),
                    });
                }
            }
            Err(e) => report.error = Some(e.to_string()),
        },
    }
    Ok(report)
}
