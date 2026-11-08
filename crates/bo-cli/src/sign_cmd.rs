//! `bo-cli sign` / `bo-cli verify` (ADR-0007): signer selection (PEM key,
//! PKCS#12 file, remote signing service), RFC 3161 time-stamps and
//! certificate path validation against configured trust anchors.

use bo_sign::http::{HttpTimestampClient, UreqTransport};
use bo_sign::pdf::SignOptions;
use bo_sign::remote::{JsonSigningApi, RemoteConfig, RemoteSigner};
use bo_sign::report::{verify_document, ChainSummary, DocumentReport};
use bo_sign::trust::TrustStore;
use bo_sign::{Signer, SoftwareSigner};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

type R<T> = Result<T, Box<dyn std::error::Error>>;

/// Where the signing key is.
pub struct SignerArgs {
    pub key: Option<PathBuf>,
    pub cert: Option<PathBuf>,
    pub p12: Option<PathBuf>,
    pub password: Option<String>,
    pub remote: Option<String>,
    pub remote_credential: Option<String>,
    pub remote_token: Option<String>,
}

fn env(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.is_empty())
}

/// Builds the signer from the arguments (exactly one source).
pub fn signer(a: &SignerArgs, document_name: &str) -> R<Box<dyn Signer>> {
    match (&a.key, &a.p12, &a.remote) {
        (Some(key), None, None) => {
            let cert = a.cert.as_ref().ok_or("--key needs --cert")?;
            Ok(Box::new(SoftwareSigner::from_pem(
                &std::fs::read_to_string(key)?,
                &std::fs::read(cert)?,
            )?))
        }
        (None, Some(p12), None) => {
            let password = a
                .password
                .clone()
                .or_else(|| env("BO_P12_PASSWORD"))
                .unwrap_or_default();
            Ok(Box::new(SoftwareSigner::from_pkcs12(
                &std::fs::read(p12)?,
                &password,
            )?))
        }
        (None, None, Some(url)) => {
            let mut config = RemoteConfig::new(url.clone());
            config.credential_id.clone_from(&a.remote_credential);
            config.access_token = a.remote_token.clone().or_else(|| env("BO_REMOTE_TOKEN"));
            config.document_name = Some(document_name.to_owned());
            let api = JsonSigningApi::new(config, Box::new(UreqTransport::default()));
            Ok(Box::new(RemoteSigner::connect(Box::new(api))?))
        }
        _ => Err("choose exactly one signer: --key/--cert, --p12 or --remote".into()),
    }
}

/// Signs `bytes`; `tsa` (or `BO_TSA_URL`) adds an RFC 3161 time-stamp.
pub fn sign(
    bytes: &[u8],
    signer: &dyn Signer,
    opts: &SignOptions,
    tsa: Option<&str>,
) -> R<Vec<u8>> {
    let tsa_url = tsa.map(str::to_owned).or_else(|| env("BO_TSA_URL"));
    let client = tsa_url.map(|u| HttpTimestampClient::new(u, Box::new(UreqTransport::default())));
    Ok(bo_sign::sign_document(
        bytes,
        signer,
        opts,
        client
            .as_ref()
            .map(|c| c as &dyn bo_sign::timestamp::TimestampClient),
    )?)
}

/// Trust anchors from `--trust` paths, else `BO_TRUST_ANCHORS`
/// (paths separated by the platform path separator).
pub fn trust_store(paths: &[PathBuf]) -> R<TrustStore> {
    let mut store = TrustStore::new();
    let mut all: Vec<PathBuf> = paths.to_vec();
    if all.is_empty() {
        if let Some(v) = std::env::var_os("BO_TRUST_ANCHORS") {
            all.extend(std::env::split_paths(&v));
        }
    }
    for p in &all {
        store.add_path(p)?;
    }
    Ok(store)
}

fn chain_lines(label: &str, c: &ChainSummary, out: &mut Vec<String>) {
    if c.trusted {
        out.push(format!(
            "  {label}: trusted (anchor {})",
            c.anchor.as_deref().unwrap_or("?")
        ));
    } else {
        out.push(format!("  {label}: NOT trusted"));
        for p in &c.problems {
            out.push(format!("    - {}", p.message));
        }
    }
}

/// Human-readable report lines.
pub fn report_lines(r: &DocumentReport) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(e) = &r.error {
        out.push(format!("INVALID: {e}"));
        return out;
    }
    if r.signatures.is_empty() {
        out.push("no signatures".into());
    }
    for (i, s) in r.signatures.iter().enumerate() {
        out.push(format!(
            "signature {}: valid, signer {}{}",
            i + 1,
            s.signer,
            if s.covers_whole_document {
                ""
            } else {
                " (document changed after this signature)"
            }
        ));
        if let Some(t) = &s.signing_time {
            out.push(format!("  claimed signing time: {t}"));
        }
        match &s.timestamp {
            Some(ts) => {
                out.push(format!("  time-stamp: {} by {}", ts.time, ts.tsa));
                chain_lines("time-stamp authority", &ts.chain, &mut out);
            }
            None => out.push("  time-stamp: none".into()),
        }
        chain_lines(
            &format!("certificate (checked at {})", s.validated_at),
            &s.chain,
            &mut out,
        );
    }
    out
}

/// `bo-cli verify`: prints the report; non-zero exit when invalid.
pub fn verify(input: &Path, trust: &[PathBuf], crls: &[PathBuf], json: bool) -> R<bool> {
    let bytes = std::fs::read(input)?;
    let mut store = trust_store(trust)?;
    for p in crls {
        store.add_path(p)?;
    }
    let report = verify_document(&bytes, &store, SystemTime::now())?;
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        if store.is_empty() {
            println!("note: no trust anchors (--trust or BO_TRUST_ANCHORS); certificate paths are not trusted");
        }
        for line in report_lines(&report) {
            println!("{line}");
        }
    }
    Ok(report.error.is_none())
}

#[cfg(test)]
mod tests {
    use super::*;
    use bo_sign::testing::{mock_remote_service, mock_tsa_service, LocalTsa, TestPki};

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("bo-cli-sign-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        d.join(name)
    }

    #[test]
    fn signs_remotely_with_timestamp_and_reports_trust() {
        let pki = TestPki::new(&mut rand::thread_rng());
        let (key, cert) = bo_sign::testing::to_pem(&pki.signer);
        let mut chain_pem = cert.clone();
        chain_pem.push_str(&bo_sign::cert_to_pem(pki.intermediate.certificate_der()));
        let remote_signer = SoftwareSigner::from_pem(&key, chain_pem.as_bytes()).unwrap();
        let (tkey, tcert) = bo_sign::testing::to_pem(&pki.tsa);
        let tsa = LocalTsa::new(
            SoftwareSigner::from_pem(&tkey, tcert.as_bytes()).unwrap(),
            SystemTime::now(),
        );
        let service = mock_remote_service(remote_signer, "tok", 1).unwrap();
        let tsa_server = mock_tsa_service(tsa).unwrap();

        let args = SignerArgs {
            key: None,
            cert: None,
            p12: None,
            password: None,
            remote: Some(service.url.clone()),
            remote_credential: None,
            remote_token: Some("tok".into()),
        };
        let s = signer(&args, "hop-dong.docx").unwrap();
        let docx = include_bytes!("../../../tests/corpus/doc/phase1-sample.docx");
        let signed = sign(
            docx,
            s.as_ref(),
            &SignOptions::default(),
            Some(&tsa_server.url),
        )
        .unwrap();
        let path = tmp("signed.docx");
        std::fs::write(&path, &signed).unwrap();
        let root = tmp("root.pem");
        std::fs::write(&root, bo_sign::cert_to_pem(pki.root.certificate_der())).unwrap();

        assert!(verify(&path, std::slice::from_ref(&root), &[], false).unwrap());
        let report =
            verify_document(&signed, &trust_store(&[root]).unwrap(), SystemTime::now()).unwrap();
        let lines = report_lines(&report).join("\n");
        assert!(lines.contains("time-stamp authority: trusted"), "{lines}");
        assert!(lines.contains("certificate (checked at"), "{lines}");
        assert!(!lines.contains("NOT trusted"), "{lines}");

        // Untrusted without anchors.
        let report = verify_document(&signed, &TrustStore::new(), SystemTime::now()).unwrap();
        assert!(report_lines(&report)
            .join("\n")
            .contains("no trust anchor configured"));
    }

    #[test]
    fn rejects_ambiguous_signer_arguments() {
        let args = SignerArgs {
            key: Some("k.pem".into()),
            cert: None,
            p12: Some("a.p12".into()),
            password: None,
            remote: None,
            remote_credential: None,
            remote_token: None,
        };
        assert!(signer(&args, "x").is_err());
    }
}
