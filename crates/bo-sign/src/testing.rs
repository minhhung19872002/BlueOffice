//! Test helpers: throwaway RSA certificates, a small PKI (root → intermediate
//! → signer, root → TSA) and an in-process RFC 3161 time-stamp authority.
//! Not for production use.
#![doc(hidden)]

use crate::cms_sig::PreparedCms;
use crate::timestamp::{
    generalized_time, PkiStatusInfo, TimeStampReq, TimeStampResp, TimestampClient, TstInfo,
    ID_CT_TST_INFO,
};
use crate::{Result, SignError, Signer, SoftwareSigner};
use const_oid::db::rfc5280;
use const_oid::ObjectIdentifier;
use der::asn1::Uint;
use der::{Any, Decode, Encode, Tag};
use rsa::pkcs1v15::SigningKey;
use rsa::RsaPrivateKey;
use sha2::Sha256;
use std::str::FromStr;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, SystemTime};
use x509_cert::builder::{Builder, CertificateBuilder, Profile};
use x509_cert::ext::pkix::ExtendedKeyUsage;
use x509_cert::name::Name;
use x509_cert::serial_number::SerialNumber;
use x509_cert::spki::EncodePublicKey;
use x509_cert::time::{Time, Validity};

/// PEM encoding of a signer's key and certificate (tests).
///
/// # Panics
/// On encoding failure (tests only).
#[must_use]
pub fn to_pem(signer: &SoftwareSigner) -> (String, String) {
    use rsa::pkcs8::EncodePrivateKey;
    let key = signer
        .key
        .to_pkcs8_pem(rsa::pkcs8::LineEnding::LF)
        .expect("pem")
        .to_string();
    (key, crate::cert_to_pem(&signer.cert_der))
}

/// PKCS#8 DER of a signer's private key (tests).
///
/// # Panics
/// On encoding failure (tests only).
#[must_use]
pub fn key_pkcs8_der(signer: &SoftwareSigner) -> Vec<u8> {
    use rsa::pkcs8::EncodePrivateKey;
    signer
        .key
        .to_pkcs8_der()
        .expect("pkcs8")
        .as_bytes()
        .to_vec()
}

/// Signer that always fails (for negative tests).
pub struct NullSigner;

impl Signer for NullSigner {
    fn certificate_der(&self) -> &[u8] {
        &[]
    }
    fn sign_sha256(&self, _digest: &[u8; 32]) -> Result<Vec<u8>> {
        Err(SignError::Signer("null signer".into()))
    }
}

fn name(cn: &str) -> Name {
    Name::from_str(&format!("CN={cn},O=Bluestar Test,C=VN")).expect("name")
}

/// Issues a certificate for `key` signed by `issuer_key` (tests).
///
/// # Panics
/// On encoding failure (tests only).
#[allow(clippy::too_many_arguments)]
fn issue(
    key: &RsaPrivateKey,
    subject: &str,
    profile: Profile,
    issuer_key: &RsaPrivateKey,
    validity: Validity,
    serial: u32,
    eku: Option<Vec<ObjectIdentifier>>,
) -> Vec<u8> {
    let pub_der = key.to_public_key().to_public_key_der().expect("spki");
    let spki = spki::SubjectPublicKeyInfoOwned::try_from(pub_der.as_bytes()).expect("spki");
    let signer = SigningKey::<Sha256>::new(issuer_key.clone());
    let mut builder = CertificateBuilder::new(
        profile,
        SerialNumber::from(serial),
        validity,
        name(subject),
        spki,
        &signer,
    )
    .expect("builder");
    if let Some(eku) = eku {
        builder.add_extension(&ExtendedKeyUsage(eku)).expect("eku");
    }
    let cert = builder.build::<rsa::pkcs1v15::Signature>().expect("cert");
    cert.to_der().expect("der")
}

/// Creates a self-signed 2048-bit RSA signer with the given common name.
///
/// # Panics
/// On RNG/encoding failure (tests only).
#[must_use]
pub fn self_signed(rng: &mut impl rsa::rand_core::CryptoRngCore, cn: &str) -> SoftwareSigner {
    let key = RsaPrivateKey::new(rng, 2048).expect("rsa keygen");
    let validity = Validity::from_now(Duration::from_hours(24)).expect("validity");
    let cert = issue(&key, cn, Profile::Root, &key, validity, 1, None);
    SoftwareSigner::new(key, cert)
}

/// Validity `[now - before, now + after]`.
///
/// # Panics
/// On time conversion failure (tests only).
#[must_use]
pub fn validity_around(now: SystemTime, before: Duration, after: Duration) -> Validity {
    Validity {
        not_before: Time::try_from(now - before).expect("time"),
        not_after: Time::try_from(now + after).expect("time"),
    }
}

/// Root CA → intermediate CA → document signer, and root CA → TSA.
pub struct TestPki {
    pub root: SoftwareSigner,
    pub intermediate: SoftwareSigner,
    /// Leaf signer whose chain contains the intermediate certificate.
    pub signer: SoftwareSigner,
    /// Time-stamping certificate (EKU `id-kp-timeStamping`).
    pub tsa: SoftwareSigner,
}

impl TestPki {
    /// Certificates valid for one day around now (1024-bit keys for speed).
    #[must_use]
    pub fn new(rng: &mut impl rsa::rand_core::CryptoRngCore) -> Self {
        let day = Duration::from_hours(24);
        Self::with_validity(rng, validity_around(SystemTime::now(), day, day))
    }

    /// Every certificate gets the given validity.
    ///
    /// # Panics
    /// On RNG/encoding failure (tests only).
    #[must_use]
    pub fn with_validity(rng: &mut impl rsa::rand_core::CryptoRngCore, validity: Validity) -> Self {
        let mut keygen = || RsaPrivateKey::new(rng, 1024).expect("rsa keygen");
        let (rk, ik, sk, tk) = (keygen(), keygen(), keygen(), keygen());
        let root = issue(&rk, "Test Root CA", Profile::Root, &rk, validity, 1, None);
        let inter = issue(
            &ik,
            "Test Issuing CA",
            Profile::SubCA {
                issuer: name("Test Root CA"),
                path_len_constraint: Some(0),
            },
            &rk,
            validity,
            2,
            None,
        );
        let leaf = issue(
            &sk,
            "Nguyễn Văn A",
            Profile::Leaf {
                issuer: name("Test Issuing CA"),
                enable_key_agreement: false,
                enable_key_encipherment: false,
            },
            &ik,
            validity,
            3,
            None,
        );
        let tsa = issue(
            &tk,
            "Test TSA",
            Profile::Leaf {
                issuer: name("Test Root CA"),
                enable_key_agreement: false,
                enable_key_encipherment: false,
            },
            &rk,
            validity,
            4,
            Some(vec![rfc5280::ID_KP_TIME_STAMPING]),
        );
        Self {
            root: SoftwareSigner::new(rk, root),
            intermediate: SoftwareSigner::new(ik, inter.clone()),
            signer: SoftwareSigner::new(sk, leaf).with_chain(vec![inter]),
            tsa: SoftwareSigner::new(tk, tsa),
        }
    }
}

/// Misbehaviours of [`LocalTsa`] for negative tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TsaFault {
    None,
    /// Answers with another nonce (replayed response).
    WrongNonce,
    /// Time-stamps another imprint.
    WrongImprint,
    /// PKIStatus rejection.
    Reject,
}

/// In-process RFC 3161 time-stamp authority.
pub struct LocalTsa {
    pub signer: SoftwareSigner,
    /// `genTime` of every token.
    pub time: SystemTime,
    pub fault: TsaFault,
    serial: AtomicU32,
}

impl LocalTsa {
    #[must_use]
    pub fn new(signer: SoftwareSigner, time: SystemTime) -> Self {
        Self {
            signer,
            time,
            fault: TsaFault::None,
            serial: AtomicU32::new(1),
        }
    }

    #[must_use]
    pub fn with_fault(mut self, fault: TsaFault) -> Self {
        self.fault = fault;
        self
    }

    /// Answers a DER `TimeStampReq` with a DER `TimeStampResp`.
    pub fn respond(&self, request_der: &[u8]) -> Result<Vec<u8>> {
        let req = TimeStampReq::from_der(request_der)?;
        if self.fault == TsaFault::Reject {
            return Ok(TimeStampResp {
                status: PkiStatusInfo {
                    status: 2,
                    status_string: Some(vec!["test rejection".into()]),
                    fail_info: None,
                },
                time_stamp_token: None,
            }
            .to_der()?);
        }
        let mut imprint = req.message_imprint.clone();
        if self.fault == TsaFault::WrongImprint {
            let mut h = imprint.hashed_message.as_bytes().to_vec();
            h[0] ^= 0xff;
            imprint.hashed_message = der::asn1::OctetString::new(h)?;
        }
        let nonce = match self.fault {
            TsaFault::WrongNonce => Some(Uint::new(&[0x42])?),
            _ => req.nonce.clone(),
        };
        let secs = crate::unix_secs(self.time);
        let serial = self.serial.fetch_add(1, Ordering::Relaxed);
        let tst = TstInfo {
            version: 1,
            policy: ObjectIdentifier::new_unwrap("1.3.6.1.4.1.99999.1.1"),
            message_imprint: imprint,
            serial_number: Uint::new(&serial.to_be_bytes())?,
            gen_time: Any::new(Tag::GeneralizedTime, generalized_time(secs).as_bytes())?,
            accuracy: None,
            ordering: false,
            nonce,
            tsa: None,
            extensions: None,
        };
        let prepared = PreparedCms::encapsulated(
            ID_CT_TST_INFO,
            tst.to_der()?,
            self.signer.certificate_der(),
            &self.signer.chain_der(),
        )?;
        let sig = self.signer.sign_sha256(&prepared.digest()?)?;
        let token = prepared.assemble(&sig, None)?;
        Ok(TimeStampResp {
            status: PkiStatusInfo {
                status: 0,
                status_string: None,
                fail_info: None,
            },
            time_stamp_token: Some(Any::from_der(&token)?),
        }
        .to_der()?)
    }
}

impl TimestampClient for LocalTsa {
    fn exchange(&self, request_der: &[u8]) -> Result<Vec<u8>> {
        self.respond(request_der)
    }
}

/// A request received by [`MockHttpServer`].
#[derive(Debug, Clone)]
pub struct MockRequest {
    pub method: String,
    pub path: String,
    /// Header names lower-cased.
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl MockRequest {
    #[must_use]
    pub fn header(&self, name: &str) -> Option<&str> {
        let name = name.to_ascii_lowercase();
        self.headers
            .iter()
            .find(|(k, _)| *k == name)
            .map(|(_, v)| v.as_str())
    }
}

/// Response of a [`MockHttpServer`] handler.
#[derive(Debug, Clone)]
pub struct MockResponse {
    pub status: u16,
    pub content_type: String,
    pub body: Vec<u8>,
}

impl MockResponse {
    #[must_use]
    pub fn json(status: u16, value: &serde_json::Value) -> Self {
        Self {
            status,
            content_type: "application/json".into(),
            body: value.to_string().into_bytes(),
        }
    }
}

/// A tiny HTTP/1.1 server on `127.0.0.1` for tests (one connection at a
/// time, `Connection: close`). Runs until the process exits.
pub struct MockHttpServer {
    /// `http://127.0.0.1:<port>`.
    pub url: String,
}

impl MockHttpServer {
    /// Starts the server with a request handler.
    ///
    /// # Errors
    /// When the listening socket cannot be bound.
    pub fn start<F>(handler: F) -> std::io::Result<Self>
    where
        F: Fn(&MockRequest) -> MockResponse + Send + 'static,
    {
        let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
        let url = format!("http://{}", listener.local_addr()?);
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let _ = serve_one(stream, &handler);
            }
        });
        Ok(Self { url })
    }
}

fn serve_one<F>(stream: std::net::TcpStream, handler: &F) -> std::io::Result<()>
where
    F: Fn(&MockRequest) -> MockResponse,
{
    use std::io::{BufRead, BufReader, Read, Write};
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    let mut parts = line.split_whitespace();
    let method = parts.next().unwrap_or_default().to_owned();
    let path = parts.next().unwrap_or_default().to_owned();
    let mut headers = Vec::new();
    loop {
        let mut h = String::new();
        if reader.read_line(&mut h)? == 0 || h.trim().is_empty() {
            break;
        }
        if let Some((k, v)) = h.split_once(':') {
            headers.push((k.trim().to_ascii_lowercase(), v.trim().to_owned()));
        }
    }
    let len = headers
        .iter()
        .find(|(k, _)| k == "content-length")
        .and_then(|(_, v)| v.parse::<usize>().ok())
        .unwrap_or(0);
    let mut body = vec![0; len];
    reader.read_exact(&mut body)?;
    let resp = handler(&MockRequest {
        method,
        path,
        headers,
        body,
    });
    let mut out = stream;
    write!(
        out,
        "HTTP/1.1 {} X\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        resp.status,
        resp.content_type,
        resp.body.len()
    )?;
    out.write_all(&resp.body)?;
    out.flush()
}

/// Generic-JSON remote signing service (`docs/deploy/remote-signing.md`)
/// backed by `signer`; the first `pending_polls` status calls answer
/// `pending` (user has not confirmed yet). Expects `Bearer <token>`.
///
/// # Errors
/// When the listening socket cannot be bound.
pub fn mock_remote_service(
    signer: SoftwareSigner,
    token: &'static str,
    pending_polls: u32,
) -> std::io::Result<MockHttpServer> {
    use base64::engine::general_purpose::STANDARD as B64;
    use base64::Engine as _;
    use serde_json::json;
    let polls = std::sync::Mutex::new(0u32);
    let pending: std::sync::Mutex<Option<[u8; 32]>> = std::sync::Mutex::new(None);
    MockHttpServer::start(move |req| {
        if req.header("authorization") != Some(&format!("Bearer {token}")) {
            return MockResponse::json(401, &json!({ "message": "bad token" }));
        }
        let Ok(body) = serde_json::from_slice::<serde_json::Value>(&req.body) else {
            return MockResponse::json(400, &json!({ "message": "bad json" }));
        };
        let sign = |digest: &[u8; 32]| match signer.sign_sha256(digest) {
            Ok(sig) => MockResponse::json(
                200,
                &json!({ "status": "signed", "signature": B64.encode(sig) }),
            ),
            Err(e) => MockResponse::json(500, &json!({ "message": e.to_string() })),
        };
        match req.path.as_str() {
            p if p.ends_with("/certificates") => {
                let mut certs = vec![B64.encode(signer.certificate_der())];
                certs.extend(signer.chain_der().iter().map(|c| B64.encode(c)));
                MockResponse::json(200, &json!({ "certificates": certs }))
            }
            p if p.ends_with("/sign") => {
                let digest: Option<[u8; 32]> = body["digest"]
                    .as_str()
                    .and_then(|d| B64.decode(d).ok())
                    .and_then(|d| d.try_into().ok());
                let Some(digest) = digest else {
                    return MockResponse::json(400, &json!({ "message": "bad digest" }));
                };
                if body["hashAlgorithm"] != "SHA-256" {
                    return MockResponse::json(400, &json!({ "message": "bad hash" }));
                }
                if pending_polls == 0 {
                    return sign(&digest);
                }
                if let Ok(mut p) = pending.lock() {
                    *p = Some(digest);
                }
                MockResponse::json(
                    200,
                    &json!({ "status": "pending", "transactionId": "tx-1", "retryAfterMs": 10 }),
                )
            }
            p if p.ends_with("/sign/status") => {
                if body["transactionId"] != "tx-1" {
                    return MockResponse::json(404, &json!({ "message": "unknown transaction" }));
                }
                let n = polls.lock().map(|mut n| {
                    *n += 1;
                    *n
                });
                if n.unwrap_or(0) < pending_polls {
                    return MockResponse::json(
                        200,
                        &json!({ "status": "pending", "transactionId": "tx-1", "retryAfterMs": 10 }),
                    );
                }
                match pending.lock().ok().and_then(|p| *p) {
                    Some(d) => sign(&d),
                    None => MockResponse::json(409, &json!({ "message": "nothing pending" })),
                }
            }
            _ => MockResponse::json(404, &json!({ "message": "not found" })),
        }
    })
}

/// RFC 3161 HTTP time-stamp authority (`application/timestamp-query`).
///
/// # Errors
/// When the listening socket cannot be bound.
pub fn mock_tsa_service(tsa: LocalTsa) -> std::io::Result<MockHttpServer> {
    MockHttpServer::start(move |req| {
        if req.header("content-type") != Some("application/timestamp-query") {
            return MockResponse {
                status: 415,
                content_type: "text/plain".into(),
                body: b"expected application/timestamp-query".to_vec(),
            };
        }
        match tsa.respond(&req.body) {
            Ok(body) => MockResponse {
                status: 200,
                content_type: "application/timestamp-reply".into(),
                body,
            },
            Err(e) => MockResponse {
                status: 400,
                content_type: "text/plain".into(),
                body: e.to_string().into_bytes(),
            },
        }
    })
}
