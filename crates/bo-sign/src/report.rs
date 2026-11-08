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

/// One verified document time-stamp (PAdES B-LTA).
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DocumentTimestampSummary {
    /// The time-stamp and its TSA path.
    pub timestamp: TimestampSummary,
    /// Covers the whole file, or only validation data / time-stamps follow.
    pub covers_whole_document: bool,
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
    /// PDF document time-stamps (PAdES B-LTA, `/DocTimeStamp`), in document
    /// order; the TSA path is validated at the verification time.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub document_timestamps: Vec<DocumentTimestampSummary>,
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
        document_timestamps: Vec::new(),
        error: None,
    };
    match kind {
        DocumentKind::Pdf => match crate::pdf::verify_pdf(bytes) {
            Ok(sigs) => {
                // Long-term validation data in the file (PAdES B-LT DSS): its
                // certificates help build paths, its CRLs / OCSP responses
                // answer revocation without the network.
                let dss = crate::pdf::read_dss(bytes).unwrap_or_default();
                let mut with_dss = store.clone();
                for d in &dss.crls {
                    let _ = with_dss.add_crl_der(d);
                }
                for d in &dss.ocsps {
                    let _ = with_dss.add_ocsp_der(d);
                }
                for s in sigs {
                    let pool: Vec<Vec<u8>> = s
                        .signer
                        .certificates
                        .iter()
                        .chain(&dss.certs)
                        .cloned()
                        .collect();
                    let (timestamp, chain, at) = summarize(
                        &s.signer.certificate_der,
                        &pool,
                        s.signer.timestamp.as_ref(),
                        &with_dss,
                        now,
                    );
                    report.signatures.push(SignatureSummary {
                        signer: s.signer.subject,
                        signing_time: None,
                        covers_whole_document: s.covers_whole_file || s.only_validation_data_after,
                        timestamp,
                        chain,
                        validated_at: crate::iso_utc(crate::unix_secs(at)),
                    });
                }
                match crate::pdf::verify_pdf_timestamps(bytes) {
                    Ok(list) => {
                        for t in list {
                            let pool: Vec<Vec<u8>> = t
                                .info
                                .certificates
                                .iter()
                                .chain(&dss.certs)
                                .cloned()
                                .collect();
                            report.document_timestamps.push(DocumentTimestampSummary {
                                timestamp: TimestampSummary {
                                    time: t.info.gen_time.clone(),
                                    tsa: t.info.tsa_subject.clone(),
                                    chain: validate_chain(
                                        &t.info.tsa_certificate_der,
                                        &pool,
                                        &with_dss,
                                        now,
                                        Purpose::TimeStamping,
                                    )
                                    .into(),
                                },
                                covers_whole_document: t.covers_whole_file
                                    || t.only_validation_data_after,
                            });
                        }
                    }
                    Err(e) => report.error = Some(e.to_string()),
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

/// Long-term validation data for every signature of a PDF (and its signature
/// time-stamp): the certificates of each path to the trust anchors and, for
/// each certificate, the revocation data `store` has or fetches for it (CRLs,
/// OCSP responses). Validation times as in [`verify_document`].
pub fn collect_validation_data(
    pdf: &[u8],
    store: &TrustStore,
    now: SystemTime,
) -> Result<crate::pdf::Dss> {
    let have = crate::pdf::read_dss(pdf)?;
    let mut out = crate::pdf::Dss::default();
    let push = |v: &mut Vec<Vec<u8>>, d: Vec<u8>| {
        if !v.contains(&d) {
            v.push(d);
        }
    };
    let mut paths = Vec::new();
    for t in crate::pdf::verify_pdf_timestamps(pdf)? {
        let pool: Vec<Vec<u8>> = t
            .info
            .certificates
            .iter()
            .chain(&have.certs)
            .cloned()
            .collect();
        paths.push(validate_chain(
            &t.info.tsa_certificate_der,
            &pool,
            store,
            now,
            Purpose::TimeStamping,
        ));
    }
    for s in crate::pdf::verify_pdf(pdf)? {
        let pool: Vec<Vec<u8>> = s
            .signer
            .certificates
            .iter()
            .chain(&have.certs)
            .cloned()
            .collect();
        let (ts, _, at) = summarize(
            &s.signer.certificate_der,
            &pool,
            s.signer.timestamp.as_ref(),
            store,
            now,
        );
        paths.push(validate_chain(
            &s.signer.certificate_der,
            &pool,
            store,
            at,
            Purpose::DocumentSigning,
        ));
        if let (Some(t), Some(_)) = (&s.signer.timestamp, ts) {
            let at = UNIX_EPOCH + Duration::from_secs(t.gen_time_unix);
            paths.push(validate_chain(
                &t.tsa_certificate_der,
                &t.certificates,
                store,
                at,
                Purpose::TimeStamping,
            ));
        }
    }
    for p in paths {
        for pair in p.certificates.windows(2) {
            let (crls, ocsps) = store.revocation_data(&pair[0], &pair[1]);
            for d in crls {
                push(&mut out.crls, d);
            }
            for d in ocsps {
                push(&mut out.ocsps, d);
            }
        }
        for c in p.certificates {
            push(&mut out.certs, c);
        }
    }
    Ok(out)
}

/// Adds the long-term validation data of every signature to a PDF's DSS
/// (PAdES baseline B-LT, ETSI EN 319 142-1 §6.4) without changing the signed
/// revisions.
pub fn add_validation_data(pdf: &[u8], store: &TrustStore, now: SystemTime) -> Result<Vec<u8>> {
    let dss = collect_validation_data(pdf, store, now)?;
    crate::pdf::add_dss(pdf, &dss)
}
