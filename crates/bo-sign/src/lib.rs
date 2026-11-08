//! Digital signatures for BlueOffice output (prompt §6 Phase 6 "Ký số").
//!
//! * [`pdf`] — PAdES baseline B-B / B-T signatures (ETSI EN 319 142-1) added
//!   to a PDF as an incremental update (ISO 32000-1 §12.8).
//! * [`ooxml`] — package digital signatures for `.docx/.xlsx/.pptx`
//!   (ECMA-376 Part 2, §12 Digital Signatures, W3C XML-DSig) with XAdES
//!   qualifying properties (ETSI TS 101 903, [MS-OFFCRYPTO] §2.5.2).
//! * [`timestamp`] — RFC 3161 time-stamp requests, responses and tokens.
//! * [`trust`] — certificate path validation against configured trust
//!   anchors (RFC 5280 §6, simplified: no revocation checking).
//! * [`remote`] — remote signing service client (generic JSON protocol,
//!   `docs/deploy/remote-signing.md`), [`http`] — HTTP transport.
//! * `pkcs11` (feature `pkcs11`) — USB tokens through PKCS#11.
//!
//! Signing keys never need to live in this process: everything goes through
//! the [`Signer`] trait, which receives a SHA-256 digest. That is the
//! integration point for USB tokens (PKCS#11) and the remote-signing services
//! of Vietnamese CAs, which sign a hash and return a PKCS#1 v1.5 signature.
//! [`SoftwareSigner`] is the in-process implementation used by tests, by
//! certificate files (`.pem`, `.p12`) and by servers holding their own key.
//! The `prepare_*` / `finish` APIs split signing in two phases for callers
//! that obtain the signature asynchronously (the browser).
#![forbid(unsafe_code)]

pub mod cms_sig;
pub mod http;
pub mod ooxml;
pub mod pdf;
#[cfg(feature = "pkcs11")]
pub mod pkcs11;
pub mod remote;
pub mod timestamp;
pub mod trust;

use const_oid::db::rfc5912;
use const_oid::ObjectIdentifier;
use rsa::pkcs1v15::Pkcs1v15Sign;
use rsa::RsaPrivateKey;
use sha2::{Digest, Sha256, Sha384, Sha512};

/// Errors from signing or verification.
#[derive(Debug, thiserror::Error)]
pub enum SignError {
    /// The signer (key, token or remote service) failed.
    #[error("signer failed: {0}")]
    Signer(String),
    /// The input format or algorithm is not supported.
    #[error("unsupported input: {0}")]
    Unsupported(String),
    /// The input is not well formed.
    #[error("malformed input: {0}")]
    Malformed(String),
    /// A signature did not verify.
    #[error("verification failed: {0}")]
    Invalid(String),
    /// A network request (timestamp authority, remote signing service) failed.
    #[error("network error: {0}")]
    Network(String),
    /// Error from the OPC package layer.
    #[error("package error: {0}")]
    Opc(#[from] bo_opc::OpcError),
    /// DER encoding or decoding failed.
    #[error("DER error: {0}")]
    Der(#[from] der::Error),
}

/// Result alias for this crate.
pub type Result<T> = std::result::Result<T, SignError>;

/// Signs SHA-256 digests with an RSA key (PKCS#1 v1.5, `sha256WithRSAEncryption`).
pub trait Signer {
    /// DER-encoded X.509 signing certificate.
    fn certificate_der(&self) -> &[u8];
    /// Further DER certificates to embed (intermediate CAs), may be empty.
    fn chain_der(&self) -> Vec<Vec<u8>> {
        Vec::new()
    }
    /// Returns the PKCS#1 v1.5 signature of a 32-byte SHA-256 digest.
    fn sign_sha256(&self, digest: &[u8; 32]) -> Result<Vec<u8>>;
}

/// In-process RSA signer.
pub struct SoftwareSigner {
    key: RsaPrivateKey,
    cert_der: Vec<u8>,
    chain: Vec<Vec<u8>>,
}

impl SoftwareSigner {
    /// Signer from an RSA private key and its DER-encoded certificate.
    #[must_use]
    pub fn new(key: RsaPrivateKey, cert_der: Vec<u8>) -> Self {
        Self {
            key,
            cert_der,
            chain: Vec::new(),
        }
    }

    /// Adds intermediate CA certificates (DER) to embed in signatures.
    #[must_use]
    pub fn with_chain(mut self, chain: Vec<Vec<u8>>) -> Self {
        self.chain = chain;
        self
    }

    /// Loads an RSA key (PEM, PKCS#8 `PRIVATE KEY` or PKCS#1 `RSA PRIVATE KEY`)
    /// and a certificate (DER, or PEM `CERTIFICATE`; further PEM blocks after
    /// the first are taken as the intermediate chain).
    pub fn from_pem(key_pem: &str, cert: &[u8]) -> Result<Self> {
        use rsa::pkcs1::DecodeRsaPrivateKey;
        use rsa::pkcs8::DecodePrivateKey;
        let key = RsaPrivateKey::from_pkcs8_pem(key_pem)
            .or_else(|_| RsaPrivateKey::from_pkcs1_pem(key_pem))
            .map_err(|e| SignError::Malformed(format!("private key: {e}")))?;
        let mut certs = match std::str::from_utf8(cert) {
            Ok(text) if text.contains("-----BEGIN CERTIFICATE-----") => {
                pem_blocks(text, "CERTIFICATE")?
            }
            _ => vec![cert.to_vec()],
        };
        let cert_der = certs.remove(0);
        Self::checked(key, cert_der, certs)
    }

    /// Loads the first private key + certificate chain of a PKCS#12 file
    /// (`.p12` / `.pfx`, RFC 7292): PBES2/AES or legacy 3DES/RC2 encryption.
    #[cfg(feature = "pkcs12")]
    pub fn from_pkcs12(data: &[u8], password: &str) -> Result<Self> {
        use rsa::pkcs8::DecodePrivateKey;
        let store = p12_keystore::KeyStore::from_pkcs12(
            data,
            password,
            p12_keystore::Pkcs12ImportPolicy::Relaxed,
        )
        .map_err(|e| SignError::Malformed(format!("PKCS#12: {e}")))?;
        let (_, chain) = store
            .private_key_chain()
            .ok_or_else(|| SignError::Malformed("PKCS#12 has no private key".into()))?;
        let key = RsaPrivateKey::from_pkcs8_der(chain.key().as_der())
            .map_err(|e| SignError::Unsupported(format!("PKCS#12 key (RSA only): {e}")))?;
        let mut certs: Vec<Vec<u8>> = chain.certs().iter().map(|c| c.as_der().to_vec()).collect();
        if certs.is_empty() {
            return Err(SignError::Malformed("PKCS#12 has no certificate".into()));
        }
        let cert_der = certs.remove(0);
        Self::checked(key, cert_der, certs)
    }

    /// Makes sure the key matches the certificate (rejects garbage early).
    fn checked(key: RsaPrivateKey, cert_der: Vec<u8>, chain: Vec<Vec<u8>>) -> Result<Self> {
        let probe = sha256(b"bo-sign key check");
        let signer = Self {
            key,
            cert_der,
            chain,
        };
        let sig = signer.sign_sha256(&probe)?;
        verify_with_cert(&signer.cert_der, &probe, &sig).map_err(|_| {
            SignError::Malformed("the private key does not match the certificate".into())
        })?;
        Ok(signer)
    }
}

/// Base64 bodies of every PEM block with the given label (RFC 7468).
pub fn pem_blocks(text: &str, label: &str) -> Result<Vec<Vec<u8>>> {
    use base64::Engine as _;
    let begin = format!("-----BEGIN {label}-----");
    let end = format!("-----END {label}-----");
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(a) = rest.find(&begin) {
        let start = a + begin.len();
        let stop = rest[start..]
            .find(&end)
            .ok_or_else(|| SignError::Malformed(format!("unterminated {label} PEM block")))?
            + start;
        let b64: String = rest[start..stop]
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect();
        out.push(
            base64::engine::general_purpose::STANDARD
                .decode(b64)
                .map_err(|e| SignError::Malformed(format!("{label} PEM: {e}")))?,
        );
        rest = &rest[stop + end.len()..];
    }
    if out.is_empty() {
        return Err(SignError::Malformed(format!("no {label} PEM block")));
    }
    Ok(out)
}

/// PEM encoding of a DER certificate (RFC 7468 §5).
#[must_use]
pub fn cert_to_pem(der: &[u8]) -> String {
    use base64::Engine as _;
    let b64 = base64::engine::general_purpose::STANDARD.encode(der);
    let mut pem = String::from("-----BEGIN CERTIFICATE-----\n");
    for chunk in b64.as_bytes().chunks(64) {
        pem.push_str(std::str::from_utf8(chunk).unwrap_or_default());
        pem.push('\n');
    }
    pem.push_str("-----END CERTIFICATE-----\n");
    pem
}

impl Signer for SoftwareSigner {
    fn certificate_der(&self) -> &[u8] {
        &self.cert_der
    }

    fn chain_der(&self) -> Vec<Vec<u8>> {
        self.chain.clone()
    }

    fn sign_sha256(&self, digest: &[u8; 32]) -> Result<Vec<u8>> {
        self.key
            .sign(Pkcs1v15Sign::new::<Sha256>(), digest)
            .map_err(|e| SignError::Signer(e.to_string()))
    }
}

pub(crate) fn sha256(data: &[u8]) -> [u8; 32] {
    Sha256::digest(data).into()
}

/// Hash algorithms accepted when verifying (RFC 5754 §2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HashAlg {
    Sha256,
    Sha384,
    Sha512,
}

impl HashAlg {
    /// From a digest algorithm OID (`id-sha256`, …).
    pub fn from_digest_oid(oid: ObjectIdentifier) -> Result<Self> {
        match oid {
            o if o == rfc5912::ID_SHA_256 => Ok(Self::Sha256),
            o if o == rfc5912::ID_SHA_384 => Ok(Self::Sha384),
            o if o == rfc5912::ID_SHA_512 => Ok(Self::Sha512),
            o => Err(SignError::Unsupported(format!("digest algorithm {o}"))),
        }
    }

    /// From an RSA signature algorithm OID (`sha256WithRSAEncryption`, …).
    pub fn from_rsa_signature_oid(oid: ObjectIdentifier) -> Result<Self> {
        match oid {
            o if o == rfc5912::SHA_256_WITH_RSA_ENCRYPTION => Ok(Self::Sha256),
            o if o == rfc5912::SHA_384_WITH_RSA_ENCRYPTION => Ok(Self::Sha384),
            o if o == rfc5912::SHA_512_WITH_RSA_ENCRYPTION => Ok(Self::Sha512),
            o => Err(SignError::Unsupported(format!("signature algorithm {o}"))),
        }
    }

    #[must_use]
    pub fn digest(self, data: &[u8]) -> Vec<u8> {
        match self {
            Self::Sha256 => Sha256::digest(data).to_vec(),
            Self::Sha384 => Sha384::digest(data).to_vec(),
            Self::Sha512 => Sha512::digest(data).to_vec(),
        }
    }

    fn padding(self) -> Pkcs1v15Sign {
        match self {
            Self::Sha256 => Pkcs1v15Sign::new::<Sha256>(),
            Self::Sha384 => Pkcs1v15Sign::new::<Sha384>(),
            Self::Sha512 => Pkcs1v15Sign::new::<Sha512>(),
        }
    }
}

/// RSA public key of a certificate (RFC 3279 §2.3.1 `rsaEncryption`).
pub(crate) fn rsa_key_of(cert: &x509_cert::Certificate) -> Result<rsa::RsaPublicKey> {
    use rsa::pkcs1::DecodeRsaPublicKey;
    let spki = &cert.tbs_certificate.subject_public_key_info;
    if spki.algorithm.oid != rfc5912::RSA_ENCRYPTION {
        return Err(SignError::Unsupported(
            "only RSA certificates are supported".into(),
        ));
    }
    let key_bytes = spki
        .subject_public_key
        .as_bytes()
        .ok_or_else(|| SignError::Malformed("public key bit string".into()))?;
    rsa::RsaPublicKey::from_pkcs1_der(key_bytes).map_err(|e| SignError::Malformed(e.to_string()))
}

/// Verifies a PKCS#1 v1.5 signature over an already computed `hashed` value.
pub(crate) fn verify_rsa(
    cert: &x509_cert::Certificate,
    alg: HashAlg,
    hashed: &[u8],
    sig: &[u8],
) -> Result<()> {
    rsa_key_of(cert)?
        .verify(alg.padding(), hashed, sig)
        .map_err(|_| SignError::Invalid("RSA signature mismatch".into()))
}

/// Verifies a PKCS#1 v1.5 SHA-256 signature with the certificate's RSA key.
pub(crate) fn verify_with_cert(cert_der: &[u8], digest: &[u8; 32], sig: &[u8]) -> Result<()> {
    use der::Decode;
    let cert = x509_cert::Certificate::from_der(cert_der)?;
    verify_rsa(&cert, HashAlg::Sha256, digest, sig)
}

/// Seconds since the Unix epoch, 0 before it.
pub(crate) fn unix_secs(t: std::time::SystemTime) -> u64 {
    t.duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// `YYYY-MM-DDThh:mm:ssZ` (UTC) of a Unix time.
#[must_use]
pub fn iso_utc(secs: u64) -> String {
    let d = pdf::civil_from_unix(secs);
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        d.0, d.1, d.2, d.3, d.4, d.5
    )
}

/// Kind of document being signed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocumentKind {
    /// PDF (PAdES).
    Pdf,
    /// `.docx/.xlsx/.pptx` package (ECMA-376 Part 2 §12).
    Ooxml,
}

impl DocumentKind {
    /// Sniffs the kind from the first bytes (`%PDF` / ZIP `PK`).
    #[must_use]
    pub fn sniff(bytes: &[u8]) -> Option<Self> {
        if bytes.starts_with(b"%PDF") {
            Some(Self::Pdf)
        } else if bytes.starts_with(b"PK") {
            Some(Self::Ooxml)
        } else {
            None
        }
    }
}

/// A signature waiting for its value: phase 1 of two-phase signing
/// (browser + remote service, or any asynchronous signer).
#[allow(clippy::large_enum_variant)] // short-lived, one per signing
pub enum PreparedSignature {
    Pdf(pdf::PreparedPdf),
    Ooxml(ooxml::PreparedOoxml),
}

impl PreparedSignature {
    /// Prepares a signature of `bytes` by `cert_der` (+ intermediates).
    pub fn prepare(
        bytes: &[u8],
        cert_der: &[u8],
        chain: &[Vec<u8>],
        opts: &pdf::SignOptions,
    ) -> Result<Self> {
        match DocumentKind::sniff(bytes) {
            Some(DocumentKind::Pdf) => {
                Ok(Self::Pdf(pdf::prepare_pdf(bytes, cert_der, chain, opts)?))
            }
            Some(DocumentKind::Ooxml) => Ok(Self::Ooxml(ooxml::prepare_ooxml(
                bytes, cert_der, chain, opts.time,
            )?)),
            None => Err(SignError::Unsupported(
                "only PDF and OOXML files can be signed".into(),
            )),
        }
    }

    /// SHA-256 digest to sign (PKCS#1 v1.5).
    pub fn digest(&self) -> Result<[u8; 32]> {
        match self {
            Self::Pdf(p) => p.digest(),
            Self::Ooxml(p) => Ok(p.digest()),
        }
    }

    /// Bytes an RFC 3161 time-stamp must cover for `signature`.
    #[must_use]
    pub fn timestamp_message(&self, signature: &[u8]) -> Vec<u8> {
        match self {
            Self::Pdf(_) => signature.to_vec(),
            Self::Ooxml(_) => ooxml::PreparedOoxml::timestamp_message(signature),
        }
    }

    /// Phase 2: embeds the signature value (and optional token).
    pub fn finish(self, signature: &[u8], timestamp_token: Option<&[u8]>) -> Result<Vec<u8>> {
        match self {
            Self::Pdf(p) => p.finish(signature, timestamp_token),
            Self::Ooxml(p) => p.finish(signature, timestamp_token),
        }
    }
}

/// Signs `bytes` (PDF or OOXML, sniffed) with `signer`, adding a signature
/// time-stamp when `tsa` is given.
pub fn sign_document(
    bytes: &[u8],
    signer: &dyn Signer,
    opts: &pdf::SignOptions,
    tsa: Option<&dyn timestamp::TimestampClient>,
) -> Result<Vec<u8>> {
    let prepared =
        PreparedSignature::prepare(bytes, signer.certificate_der(), &signer.chain_der(), opts)?;
    let signature = signer.sign_sha256(&prepared.digest()?)?;
    let token = match tsa {
        Some(client) => {
            let message = prepared.timestamp_message(&signature);
            Some(timestamp::timestamp(
                client,
                &message,
                timestamp::fresh_nonce(&message),
            )?)
        }
        None => None,
    };
    prepared.finish(&signature, token.as_deref())
}

pub mod report;
pub mod testing;
