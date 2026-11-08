//! Remote signing ("ký số từ xa"): the private key stays with a signing
//! service (a CA's cloud HSM, a company signing gateway); BlueOffice only
//! sends the SHA-256 digest and receives the PKCS#1 v1.5 signature.
//!
//! Vietnamese remote-signing CAs each have their own API, so the client is
//! adapter-based: [`RemoteSigner`] works with any [`RemoteSigningApi`];
//! [`JsonSigningApi`] implements the generic BlueOffice JSON protocol
//! (`docs/deploy/remote-signing.md`) that a gateway can translate to a
//! given CA. The same protocol is spoken by the browser client
//! (`packages/platform`), which cannot use native code.

use crate::http::HttpTransport;
use crate::{verify_with_cert, Result, SignError, Signer};
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine as _;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

/// Something that can list the signing certificates and sign a digest.
pub trait RemoteSigningApi {
    /// Signing certificate first, then intermediates (DER).
    fn certificates(&self) -> Result<Vec<Vec<u8>>>;
    /// PKCS#1 v1.5 signature of a SHA-256 digest (may wait for the user to
    /// confirm on another device).
    fn sign_digest(&self, digest: &[u8; 32]) -> Result<Vec<u8>>;
}

/// [`Signer`] backed by a remote service.
pub struct RemoteSigner {
    cert: Vec<u8>,
    chain: Vec<Vec<u8>>,
    api: Box<dyn RemoteSigningApi>,
}

impl RemoteSigner {
    /// Fetches the certificates from the service.
    pub fn connect(api: Box<dyn RemoteSigningApi>) -> Result<Self> {
        let mut certs = api.certificates()?;
        if certs.is_empty() {
            return Err(SignError::Signer(
                "the signing service returned no certificate".into(),
            ));
        }
        let cert = certs.remove(0);
        Ok(Self {
            cert,
            chain: certs,
            api,
        })
    }
}

impl Signer for RemoteSigner {
    fn certificate_der(&self) -> &[u8] {
        &self.cert
    }

    fn chain_der(&self) -> Vec<Vec<u8>> {
        self.chain.clone()
    }

    fn sign_sha256(&self, digest: &[u8; 32]) -> Result<Vec<u8>> {
        let sig = self.api.sign_digest(digest)?;
        // Catch a service signing with another key than the certificate's.
        verify_with_cert(&self.cert, digest, &sig).map_err(|_| {
            SignError::Signer(
                "the signing service returned a signature that does not match its certificate"
                    .into(),
            )
        })?;
        Ok(sig)
    }
}

/// Configuration of the generic JSON protocol.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RemoteConfig {
    /// Base URL, e.g. `https://sign.example.vn/api/v1`.
    pub url: String,
    /// Credential (key) id at the service, if the user has several.
    #[serde(default)]
    pub credential_id: Option<String>,
    /// Sent as `Authorization: Bearer …`.
    #[serde(default)]
    pub access_token: Option<String>,
    /// Shown to the user by the service when confirming.
    #[serde(default)]
    pub document_name: Option<String>,
    /// Maximum time to wait for an asynchronous confirmation.
    #[serde(default = "default_max_wait")]
    pub max_wait_secs: u64,
}

fn default_max_wait() -> u64 {
    120
}

impl RemoteConfig {
    #[must_use]
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            credential_id: None,
            access_token: None,
            document_name: None,
            max_wait_secs: default_max_wait(),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CertificatesRequest<'a> {
    credential_id: Option<&'a str>,
}

#[derive(Deserialize)]
struct CertificatesResponse {
    certificates: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SignRequest<'a> {
    credential_id: Option<&'a str>,
    hash_algorithm: &'a str,
    signature_algorithm: &'a str,
    digest: String,
    document_name: Option<&'a str>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StatusRequest<'a> {
    transaction_id: &'a str,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SignResponse {
    status: String,
    #[serde(default)]
    signature: Option<String>,
    #[serde(default)]
    transaction_id: Option<String>,
    #[serde(default)]
    retry_after_ms: Option<u64>,
    #[serde(default)]
    message: Option<String>,
}

/// Generic JSON protocol client (`docs/deploy/remote-signing.md`).
pub struct JsonSigningApi {
    config: RemoteConfig,
    transport: Box<dyn HttpTransport>,
}

impl JsonSigningApi {
    #[must_use]
    pub fn new(config: RemoteConfig, transport: Box<dyn HttpTransport>) -> Self {
        Self { config, transport }
    }

    fn call<T: for<'de> Deserialize<'de>>(&self, path: &str, body: &impl Serialize) -> Result<T> {
        let url = format!("{}/{path}", self.config.url.trim_end_matches('/'));
        let mut headers = vec![
            ("Content-Type".to_owned(), "application/json".to_owned()),
            ("Accept".to_owned(), "application/json".to_owned()),
        ];
        if let Some(t) = &self.config.access_token {
            headers.push(("Authorization".to_owned(), format!("Bearer {t}")));
        }
        let body = serde_json::to_vec(body).map_err(|e| SignError::Signer(e.to_string()))?;
        let resp = self.transport.post(&url, &headers, &body)?;
        if !(200..300).contains(&resp.status) {
            let detail = serde_json::from_slice::<serde_json::Value>(&resp.body)
                .ok()
                .and_then(|v| v.get("message").and_then(|m| m.as_str()).map(str::to_owned))
                .unwrap_or_default();
            return Err(SignError::Network(format!(
                "signing service answered HTTP {}{}{detail}",
                resp.status,
                if detail.is_empty() { "" } else { ": " }
            )));
        }
        serde_json::from_slice(&resp.body)
            .map_err(|e| SignError::Malformed(format!("signing service response: {e}")))
    }
}

impl RemoteSigningApi for JsonSigningApi {
    fn certificates(&self) -> Result<Vec<Vec<u8>>> {
        let resp: CertificatesResponse = self.call(
            "certificates",
            &CertificatesRequest {
                credential_id: self.config.credential_id.as_deref(),
            },
        )?;
        resp.certificates
            .iter()
            .map(|c| {
                B64.decode(c.trim())
                    .map_err(|e| SignError::Malformed(format!("certificate: {e}")))
            })
            .collect()
    }

    fn sign_digest(&self, digest: &[u8; 32]) -> Result<Vec<u8>> {
        let mut resp: SignResponse = self.call(
            "sign",
            &SignRequest {
                credential_id: self.config.credential_id.as_deref(),
                hash_algorithm: "SHA-256",
                signature_algorithm: "RSASSA-PKCS1-v1_5",
                digest: B64.encode(digest),
                document_name: self.config.document_name.as_deref(),
            },
        )?;
        let deadline = Instant::now() + Duration::from_secs(self.config.max_wait_secs);
        loop {
            match resp.status.as_str() {
                "signed" => {
                    let sig = resp.signature.ok_or_else(|| {
                        SignError::Malformed("signed response without signature".into())
                    })?;
                    return B64
                        .decode(sig.trim())
                        .map_err(|e| SignError::Malformed(format!("signature: {e}")));
                }
                "pending" => {
                    let id = resp.transaction_id.ok_or_else(|| {
                        SignError::Malformed("pending response without transactionId".into())
                    })?;
                    if Instant::now() >= deadline {
                        return Err(SignError::Signer(
                            "timed out waiting for the signature to be confirmed".into(),
                        ));
                    }
                    let wait = resp.retry_after_ms.unwrap_or(2000).clamp(10, 10_000);
                    std::thread::sleep(Duration::from_millis(wait));
                    resp = self.call(
                        "sign/status",
                        &StatusRequest {
                            transaction_id: &id,
                        },
                    )?;
                }
                "rejected" => {
                    return Err(SignError::Signer(format!(
                        "signing rejected{}",
                        resp.message.map(|m| format!(": {m}")).unwrap_or_default()
                    )));
                }
                other => {
                    return Err(SignError::Malformed(format!(
                        "unknown signing status \"{other}\""
                    )))
                }
            }
        }
    }
}
