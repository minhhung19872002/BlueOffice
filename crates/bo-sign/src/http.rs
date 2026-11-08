//! Minimal HTTP abstraction for the time-stamp authority (RFC 3161 §3.4,
//! "Time-Stamp Protocol via HTTP") and the remote signing service.
//!
//! [`HttpTransport`] is a trait so the browser (`fetch`), tests (closures,
//! [`crate::testing::MockHttpServer`]) and native code ([`UreqTransport`],
//! feature `http`) share the protocol code.

use crate::timestamp::TimestampClient;
use crate::{Result, SignError};

/// An HTTP response (only what the protocols need).
#[derive(Debug, Clone)]
pub struct HttpResponse {
    /// HTTP status code.
    pub status: u16,
    /// `Content-Type` header, if any.
    pub content_type: Option<String>,
    /// Response body.
    pub body: Vec<u8>,
}

/// Sends a POST request.
pub trait HttpTransport {
    /// POSTs `body` to `url` with the given headers and returns the response (any status).
    fn post(&self, url: &str, headers: &[(String, String)], body: &[u8]) -> Result<HttpResponse>;
}

impl<F> HttpTransport for F
where
    F: Fn(&str, &[(String, String)], &[u8]) -> Result<HttpResponse>,
{
    fn post(&self, url: &str, headers: &[(String, String)], body: &[u8]) -> Result<HttpResponse> {
        self(url, headers, body)
    }
}

/// RFC 3161 §3.4 client: POST `application/timestamp-query`, expect
/// `application/timestamp-reply`.
pub struct HttpTimestampClient {
    url: String,
    headers: Vec<(String, String)>,
    transport: Box<dyn HttpTransport>,
}

impl HttpTimestampClient {
    #[must_use]
    /// Client for the TSA at `url` using `transport`.
    pub fn new(url: impl Into<String>, transport: Box<dyn HttpTransport>) -> Self {
        Self {
            url: url.into(),
            headers: Vec::new(),
            transport,
        }
    }

    /// Extra request header (e.g. `Authorization` for a commercial TSA).
    #[must_use]
    pub fn with_header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }
}

impl TimestampClient for HttpTimestampClient {
    fn exchange(&self, request_der: &[u8]) -> Result<Vec<u8>> {
        let mut headers = vec![(
            "Content-Type".to_owned(),
            "application/timestamp-query".to_owned(),
        )];
        headers.extend(self.headers.iter().cloned());
        let resp = self.transport.post(&self.url, &headers, request_der)?;
        if resp.status != 200 {
            return Err(SignError::Network(format!(
                "time-stamp authority answered HTTP {}",
                resp.status
            )));
        }
        if let Some(ct) = &resp.content_type {
            if !ct
                .to_ascii_lowercase()
                .starts_with("application/timestamp-reply")
            {
                return Err(SignError::Malformed(format!(
                    "time-stamp authority answered {ct}"
                )));
            }
        }
        Ok(resp.body)
    }
}

/// Native HTTP(S) transport (ureq + rustls, Mozilla roots via webpki-roots).
#[cfg(feature = "http")]
pub struct UreqTransport {
    agent: ureq::Agent,
}

#[cfg(feature = "http")]
impl UreqTransport {
    #[must_use]
    /// Agent with a global request `timeout`; non-2xx statuses are returned, not errors.
    pub fn new(timeout: std::time::Duration) -> Self {
        let agent = ureq::Agent::config_builder()
            .timeout_global(Some(timeout))
            .http_status_as_error(false)
            .build()
            .into();
        Self { agent }
    }
}

#[cfg(feature = "http")]
impl Default for UreqTransport {
    fn default() -> Self {
        Self::new(std::time::Duration::from_secs(60))
    }
}

#[cfg(feature = "http")]
impl HttpTransport for UreqTransport {
    fn post(&self, url: &str, headers: &[(String, String)], body: &[u8]) -> Result<HttpResponse> {
        let mut req = self.agent.post(url);
        for (k, v) in headers {
            req = req.header(k.as_str(), v.as_str());
        }
        let mut resp = req
            .send(body)
            .map_err(|e| SignError::Network(format!("{url}: {e}")))?;
        let status = resp.status().as_u16();
        let content_type = resp
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned);
        let body = resp
            .body_mut()
            .with_config()
            .limit(16 * 1024 * 1024)
            .read_to_vec()
            .map_err(|e| SignError::Network(format!("{url}: {e}")))?;
        Ok(HttpResponse {
            status,
            content_type,
            body,
        })
    }
}
