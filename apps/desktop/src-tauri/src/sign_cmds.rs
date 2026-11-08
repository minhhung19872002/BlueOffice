//! "Ký số" on the desktop (ADR-0007): certificate files (.p12/.pfx/.pem),
//! USB tokens through PKCS#11 and the remote signing service, RFC 3161
//! time-stamps, and verification against `BO_TRUST_ANCHORS`.
//! The web view sends the document as the raw IPC body and the request as
//! a percent-encoded JSON header (`x-sign-request`).

use crate::raw_body;
use crate::shell::percent_decode;
use base64::Engine as _;
use bo_sign::http::{HttpTimestampClient, UreqTransport};
use bo_sign::pdf::SignOptions;
use bo_sign::pkcs11::{Pkcs11Signer, TokenCertificate, TokenSelection};
use bo_sign::remote::{JsonSigningApi, RemoteConfig, RemoteSigner};
use bo_sign::trust::TrustStore;
use bo_sign::{Signer, SoftwareSigner};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::SystemTime;
use tauri::ipc::{Request, Response};

/// Signer chosen in the dialog (`SignerChoice` in `packages/platform/src/types.ts`).
#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum SignerSpec {
    #[serde(rename_all = "camelCase")]
    File {
        file_name: String,
        file_base64: String,
        #[serde(default)]
        password: Option<String>,
    },
    #[serde(rename_all = "camelCase")]
    Token {
        module: String,
        #[serde(default)]
        slot: Option<u64>,
        #[serde(default)]
        id: Option<String>,
        pin: String,
    },
    #[serde(rename_all = "camelCase")]
    Remote {
        url: String,
        #[serde(default)]
        credential_id: Option<String>,
        #[serde(default)]
        access_token: Option<String>,
    },
}

/// `SignRequest` of `packages/platform/src/types.ts`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignSpec {
    pub signer: SignerSpec,
    #[serde(default)]
    pub tsa_url: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub location: Option<String>,
    pub document_name: String,
}

/// Defaults shown in the dialog (`docs/deploy/remote-signing.md` §5).
#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SigningDefaults {
    pub remote_url: Option<String>,
    pub tsa_url: Option<String>,
    pub pkcs11_module: Option<String>,
}

fn env(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.trim().is_empty())
}

#[must_use]
pub fn defaults() -> SigningDefaults {
    SigningDefaults {
        remote_url: env("BO_REMOTE_SIGN_URL"),
        tsa_url: env("BO_TSA_URL"),
        pkcs11_module: env("BO_PKCS11_MODULE"),
    }
}

/// The PEM block (with armour) of the private key in a combined PEM file.
fn private_key_pem(text: &str) -> Option<&str> {
    for label in ["PRIVATE KEY", "RSA PRIVATE KEY"] {
        let begin = format!("-----BEGIN {label}-----");
        let end = format!("-----END {label}-----");
        if let (Some(a), Some(b)) = (text.find(&begin), text.find(&end)) {
            if b > a {
                return Some(&text[a..b + end.len()]);
            }
        }
    }
    None
}

/// Loads a certificate file: PKCS#12 (`.p12/.pfx`) or a PEM file holding
/// the private key and the certificate chain.
pub fn file_signer(
    name: &str,
    bytes: &[u8],
    password: Option<&str>,
) -> Result<SoftwareSigner, String> {
    let lower = name.to_ascii_lowercase();
    let text = std::str::from_utf8(bytes).ok();
    if lower.ends_with(".p12") || lower.ends_with(".pfx") || text.is_none() {
        return SoftwareSigner::from_pkcs12(bytes, password.unwrap_or_default())
            .map_err(|e| e.to_string());
    }
    let text = text.unwrap_or_default();
    let key = private_key_pem(text).ok_or("sign:noPrivateKey")?;
    SoftwareSigner::from_pem(key, text.as_bytes()).map_err(|e| e.to_string())
}

fn signer(spec: &SignerSpec, document_name: &str) -> Result<Box<dyn Signer>, String> {
    match spec {
        SignerSpec::File {
            file_name,
            file_base64,
            password,
        } => {
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(file_base64)
                .map_err(|e| e.to_string())?;
            Ok(Box::new(file_signer(
                file_name,
                &bytes,
                password.as_deref(),
            )?))
        }
        SignerSpec::Token {
            module,
            slot,
            id,
            pin,
        } => Ok(Box::new(
            Pkcs11Signer::open(
                Path::new(module),
                &TokenSelection {
                    slot: *slot,
                    id: id.clone(),
                },
                pin,
            )
            .map_err(|e| e.to_string())?,
        )),
        SignerSpec::Remote {
            url,
            credential_id,
            access_token,
        } => {
            let mut config = RemoteConfig::new(url.clone());
            config.credential_id.clone_from(credential_id);
            config.access_token.clone_from(access_token);
            config.document_name = Some(document_name.to_owned());
            let api = JsonSigningApi::new(config, Box::new(UreqTransport::default()));
            Ok(Box::new(
                RemoteSigner::connect(Box::new(api)).map_err(|e| e.to_string())?,
            ))
        }
    }
}

/// Signs a PDF or OOXML file.
pub fn sign_bytes(bytes: &[u8], spec: &SignSpec) -> Result<Vec<u8>, String> {
    let signer = signer(&spec.signer, &spec.document_name)?;
    let opts = SignOptions {
        name: spec.name.clone(),
        reason: spec.reason.clone(),
        location: spec.location.clone(),
        ..SignOptions::default()
    };
    let tsa = spec
        .tsa_url
        .as_deref()
        .filter(|u| !u.trim().is_empty())
        .map(|u| HttpTimestampClient::new(u.trim(), Box::new(UreqTransport::default())));
    bo_sign::sign_document(
        bytes,
        signer.as_ref(),
        &opts,
        tsa.as_ref()
            .map(|c| c as &dyn bo_sign::timestamp::TimestampClient),
    )
    .map_err(|e| e.to_string())
}

/// Trust anchors from `BO_TRUST_ANCHORS` (platform path list).
fn trust_store() -> Result<TrustStore, String> {
    let mut store = TrustStore::new();
    if let Some(v) = std::env::var_os("BO_TRUST_ANCHORS") {
        for p in std::env::split_paths(&v) {
            store.add_path(&p).map_err(|e| e.to_string())?;
        }
    }
    Ok(store)
}

/// Verification report JSON (`SignatureReport` in `types.ts`).
pub fn verify_bytes(bytes: &[u8], store: &TrustStore) -> Result<String, String> {
    let report = bo_sign::report::verify_document(bytes, store, SystemTime::now())
        .map_err(|e| e.to_string())?;
    serde_json::to_string(&report).map_err(|e| e.to_string())
}

fn sign_header(request: &Request<'_>) -> Result<SignSpec, String> {
    let raw = request
        .headers()
        .get("x-sign-request")
        .and_then(|v| v.to_str().ok())
        .ok_or("missing x-sign-request header")?;
    serde_json::from_str(&percent_decode(raw)).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn sign_document(request: Request<'_>) -> Result<Response, String> {
    let bytes = raw_body(&request)?;
    let spec = sign_header(&request)?;
    // Token / remote signing may block (PIN pad, confirmation on a phone).
    tauri::async_runtime::spawn_blocking(move || sign_bytes(&bytes, &spec))
        .await
        .map_err(|e| e.to_string())?
        .map(Response::new)
}

#[tauri::command]
pub async fn verify_document(request: Request<'_>) -> Result<String, String> {
    let bytes = raw_body(&request)?;
    let store = trust_store()?;
    verify_bytes(&bytes, &store)
}

#[tauri::command]
pub fn sign_defaults() -> SigningDefaults {
    defaults()
}

#[tauri::command]
pub async fn sign_token_certificates(module: String) -> Result<Vec<TokenCertificate>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        bo_sign::pkcs11::list_certificates(Path::new(&module)).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    use bo_sign::testing::{to_pem, TestPki};

    #[test]
    fn signs_with_a_combined_pem_file_and_verifies() {
        let pki = TestPki::new(&mut rand::thread_rng());
        let (key, cert) = to_pem(&pki.signer);
        let mut combined = cert;
        combined.push_str(&bo_sign::cert_to_pem(pki.intermediate.certificate_der()));
        combined.push_str(&key);
        let spec: SignSpec = serde_json::from_value(serde_json::json!({
            "signer": {
                "kind": "file",
                "fileName": "me.pem",
                "fileBase64": base64::engine::general_purpose::STANDARD.encode(combined.as_bytes()),
            },
            "documentName": "a.docx",
        }))
        .unwrap();
        let docx = include_bytes!("../../../../tests/corpus/doc/phase1-sample.docx");
        let signed = sign_bytes(docx, &spec).unwrap();
        let mut store = TrustStore::new();
        store.add_der(pki.root.certificate_der()).unwrap();
        let report: serde_json::Value =
            serde_json::from_str(&verify_bytes(&signed, &store).unwrap()).unwrap();
        assert_eq!(
            report["signatures"][0]["chain"]["trusted"], true,
            "{report}"
        );
        // Without anchors: valid but untrusted.
        let report: serde_json::Value =
            serde_json::from_str(&verify_bytes(&signed, &TrustStore::new()).unwrap()).unwrap();
        assert_eq!(report["signatures"][0]["chain"]["trusted"], false);
    }

    #[test]
    fn rejects_bad_requests() {
        assert!(file_signer("cert.pem", b"-----BEGIN CERTIFICATE-----", None).is_err());
        assert!(file_signer("x.p12", b"\x30\x00", Some("pw")).is_err());
        let spec: Result<SignSpec, _> = serde_json::from_value(serde_json::json!({
            "signer": { "kind": "token", "module": "Z:/none.dll", "pin": "1" },
            "documentName": "a.pdf",
        }));
        let err = sign_bytes(b"%PDF-1.7", &spec.unwrap()).unwrap_err();
        assert!(err.contains("PKCS#11"), "{err}");
        assert!(bo_sign::pkcs11::list_certificates(Path::new("Z:/none.dll")).is_err());
    }
}
