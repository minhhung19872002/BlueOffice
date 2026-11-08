//! Digital signatures in the browser (ADR-0007): two-phase signing — the
//! digest is computed here, the signature value comes from the remote
//! signing service through `fetch` (`packages/platform/src/web/signing.ts`),
//! the optional RFC 3161 token from the TSA — and verification against
//! trust anchors supplied by the page. There is no clock in WASM: the
//! caller passes the time.

use bo_sign::pdf::SignOptions;
use bo_sign::timestamp::TimestampRequest;
use bo_sign::trust::TrustStore;
use bo_sign::PreparedSignature;
use serde::Deserialize;
use std::time::{Duration, UNIX_EPOCH};
use wasm_bindgen::prelude::*;

fn err(e: impl std::fmt::Display) -> JsError {
    JsError::new(&e.to_string())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PrepareOptions {
    /// Signing time, Unix seconds.
    time_secs: f64,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    reason: Option<String>,
    #[serde(default)]
    location: Option<String>,
}

fn unix(secs: f64) -> std::time::SystemTime {
    UNIX_EPOCH
        + Duration::from_secs(if secs.is_finite() && secs > 0.0 {
            secs as u64
        } else {
            0
        })
}

/// A signature waiting for its value.
#[wasm_bindgen]
pub struct WasmPreparedSignature {
    inner: Option<PreparedSignature>,
    request: Option<TimestampRequest>,
}

#[wasm_bindgen]
impl WasmPreparedSignature {
    /// SHA-256 digest to send to the signing service.
    pub fn digest(&self) -> Result<Vec<u8>, JsError> {
        let p = self
            .inner
            .as_ref()
            .ok_or_else(|| err("signature already finished"))?;
        Ok(p.digest().map_err(err)?.to_vec())
    }

    /// DER `TimeStampReq` over the signature value (`nonce`: 8 random bytes).
    #[wasm_bindgen(js_name = timestampRequest)]
    pub fn timestamp_request(
        &mut self,
        signature: &[u8],
        nonce: &[u8],
    ) -> Result<Vec<u8>, JsError> {
        let p = self
            .inner
            .as_ref()
            .ok_or_else(|| err("signature already finished"))?;
        let mut n = [0u8; 8];
        for (d, s) in n.iter_mut().zip(nonce) {
            *d = *s;
        }
        n[0] &= 0x7f;
        let req = TimestampRequest::new(&p.timestamp_message(signature), u64::from_be_bytes(n))
            .map_err(err)?;
        let der = req.to_der().map_err(err)?;
        self.request = Some(req);
        Ok(der)
    }

    /// Embeds the signature (and the TSA's DER `TimeStampResp`, validated
    /// against the last [`Self::timestamp_request`]); returns the file.
    // wasm-bindgen passes optional byte arrays by value.
    #[allow(clippy::needless_pass_by_value)]
    pub fn finish(
        &mut self,
        signature: &[u8],
        ts_response: Option<Vec<u8>>,
    ) -> Result<Vec<u8>, JsError> {
        let p = self
            .inner
            .take()
            .ok_or_else(|| err("signature already finished"))?;
        let token = match (ts_response, &self.request) {
            (Some(resp), Some(req)) => Some(req.accept_response(&resp).map_err(err)?),
            (Some(_), None) => return Err(err("no time-stamp request was made")),
            (None, _) => None,
        };
        p.finish(signature, token.as_deref()).map_err(err)
    }
}

/// Prepares a signature of a PDF or OOXML file by the certificate chain
/// `certs_pem` (PEM bundle, signer first).
#[wasm_bindgen(js_name = signPrepare)]
pub fn sign_prepare(
    bytes: &[u8],
    certs_pem: &str,
    options_json: &str,
) -> Result<WasmPreparedSignature, JsError> {
    let o: PrepareOptions = serde_json::from_str(options_json).map_err(err)?;
    let mut certs = bo_sign::pem_blocks(certs_pem, "CERTIFICATE").map_err(err)?;
    let cert = certs.remove(0);
    let opts = SignOptions {
        name: o.name,
        reason: o.reason,
        location: o.location,
        time: unix(o.time_secs),
        reserve_hex: 32_768,
    };
    let inner = PreparedSignature::prepare(bytes, &cert, &certs, &opts).map_err(err)?;
    Ok(WasmPreparedSignature {
        inner: Some(inner),
        request: None,
    })
}

fn store_of(anchors_pem: &str) -> Result<TrustStore, JsError> {
    let mut store = TrustStore::new();
    if anchors_pem.contains("-----BEGIN CERTIFICATE-----")
        || anchors_pem.contains("-----BEGIN X509 CRL-----")
    {
        store.add_bytes(anchors_pem.as_bytes()).map_err(err)?;
    }
    Ok(store)
}

/// Verifies every signature; `anchors_pem` is a PEM bundle of trust
/// anchors and certificate revocation lists (`X509 CRL`, RFC 5280 §5; may be
/// empty). `revocation_json` is a JSON array of base64 CRLs / OCSP responses
/// (DER or PEM, e.g. fetched for [`revocation_requests`]; `""` or `[]` for
/// none; unreadable items are skipped). Returns the `DocumentReport` JSON.
#[wasm_bindgen(js_name = verifySignatures)]
pub fn verify_signatures(
    bytes: &[u8],
    anchors_pem: &str,
    now_secs: f64,
    revocation_json: &str,
) -> Result<String, JsError> {
    use base64::Engine as _;
    let mut store = store_of(anchors_pem)?;
    let items: Vec<String> = if revocation_json.trim().is_empty() {
        Vec::new()
    } else {
        serde_json::from_str(revocation_json).map_err(err)?
    };
    for b64 in items {
        if let Ok(d) = base64::engine::general_purpose::STANDARD.decode(b64.trim()) {
            let _ = store.add_bytes(&d);
        }
    }
    let report = bo_sign::report::verify_document(bytes, &store, unix(now_secs)).map_err(err)?;
    serde_json::to_string(&report).map_err(err)
}

/// The CRL downloads and OCSP requests (JSON `[{kind, url, body?}]`, see
/// `bo_sign::report::RevocationRequest`) whose answers make the revocation of
/// every certificate path checkable: the page sends them through a proxy (the
/// browser cannot reach CAs directly, CORS) and passes the answers to
/// [`verify_signatures`].
#[wasm_bindgen(js_name = revocationRequests)]
pub fn revocation_requests(
    bytes: &[u8],
    anchors_pem: &str,
    now_secs: f64,
) -> Result<String, JsError> {
    let store = store_of(anchors_pem)?;
    let list = bo_sign::report::revocation_requests(bytes, &store, unix(now_secs)).map_err(err)?;
    serde_json::to_string(&list).map_err(err)
}
