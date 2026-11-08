//! Digital signatures for BlueOffice output (prompt §6 Phase 6 "Ký số").
//!
//! * [`pdf`] — PAdES baseline B-B signatures (ETSI EN 319 142-1) added to a
//!   PDF as an incremental update (ISO 32000-1 §12.8).
//! * [`ooxml`] — package digital signatures for `.docx/.xlsx/.pptx`
//!   (ECMA-376 Part 2, §12 Digital Signatures, W3C XML-DSig).
//!
//! Signing keys never need to live in this process: everything goes through
//! the [`Signer`] trait, which receives a SHA-256 digest. That is the
//! integration point for USB tokens (PKCS#11 via a helper process) and the
//! remote-signing services of Vietnamese CAs, which sign a hash and return a
//! PKCS#1 v1.5 signature. [`SoftwareSigner`] is the in-process
//! implementation used by tests and by servers holding their own key.
#![forbid(unsafe_code)]

pub mod cms_sig;
pub mod ooxml;
pub mod pdf;

use rsa::pkcs1v15::Pkcs1v15Sign;
use rsa::RsaPrivateKey;
use sha2::{Digest, Sha256};

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
}

impl SoftwareSigner {
    /// Signer from an RSA private key and its DER-encoded certificate.
    #[must_use]
    pub fn new(key: RsaPrivateKey, cert_der: Vec<u8>) -> Self {
        Self { key, cert_der }
    }

    /// Loads an RSA key (PEM, PKCS#8 `PRIVATE KEY` or PKCS#1 `RSA PRIVATE KEY`)
    /// and a certificate (DER or PEM `CERTIFICATE`).
    pub fn from_pem(key_pem: &str, cert: &[u8]) -> Result<Self> {
        use rsa::pkcs1::DecodeRsaPrivateKey;
        use rsa::pkcs8::DecodePrivateKey;
        let key = RsaPrivateKey::from_pkcs8_pem(key_pem)
            .or_else(|_| RsaPrivateKey::from_pkcs1_pem(key_pem))
            .map_err(|e| SignError::Malformed(format!("private key: {e}")))?;
        let cert_der = match std::str::from_utf8(cert) {
            Ok(text) if text.contains("-----BEGIN CERTIFICATE-----") => {
                pem_body(text, "CERTIFICATE")?
            }
            _ => cert.to_vec(),
        };
        // Reject garbage early and make sure the key matches the certificate.
        let probe = sha256(b"bo-sign key check");
        let signer = Self { key, cert_der };
        let sig = signer.sign_sha256(&probe)?;
        verify_with_cert(&signer.cert_der, &probe, &sig).map_err(|_| {
            SignError::Malformed("the private key does not match the certificate".into())
        })?;
        Ok(signer)
    }
}

/// Base64 body of the first PEM block with the given label (RFC 7468).
fn pem_body(text: &str, label: &str) -> Result<Vec<u8>> {
    use base64::Engine as _;
    let begin = format!("-----BEGIN {label}-----");
    let end = format!("-----END {label}-----");
    let start = text
        .find(&begin)
        .ok_or_else(|| SignError::Malformed(format!("no {label} PEM block")))?
        + begin.len();
    let stop = text[start..]
        .find(&end)
        .ok_or_else(|| SignError::Malformed(format!("unterminated {label} PEM block")))?
        + start;
    let b64: String = text[start..stop]
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    base64::engine::general_purpose::STANDARD
        .decode(b64)
        .map_err(|e| SignError::Malformed(format!("{label} PEM: {e}")))
}

impl Signer for SoftwareSigner {
    fn certificate_der(&self) -> &[u8] {
        &self.cert_der
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

/// Verifies a PKCS#1 v1.5 SHA-256 signature with the certificate's RSA key.
pub(crate) fn verify_with_cert(cert_der: &[u8], digest: &[u8; 32], sig: &[u8]) -> Result<()> {
    use der::{Decode, Encode};
    use rsa::pkcs1::DecodeRsaPublicKey;
    let cert = x509_cert::Certificate::from_der(cert_der)?;
    let spki = cert.tbs_certificate.subject_public_key_info;
    if spki.algorithm.oid != const_oid::db::rfc5912::RSA_ENCRYPTION {
        return Err(SignError::Unsupported(
            "only RSA certificates are supported".into(),
        ));
    }
    let key_bytes = spki
        .subject_public_key
        .as_bytes()
        .ok_or_else(|| SignError::Malformed("public key bit string".into()))?;
    let key = rsa::RsaPublicKey::from_pkcs1_der(key_bytes)
        .map_err(|e| SignError::Malformed(e.to_string()))?;
    let _ = spki.algorithm.to_der()?;
    key.verify(Pkcs1v15Sign::new::<Sha256>(), digest, sig)
        .map_err(|_| SignError::Invalid("RSA signature mismatch".into()))
}

/// Test helpers: throwaway self-signed RSA certificates.
#[doc(hidden)]
pub mod testing {
    use super::SoftwareSigner;
    use der::Encode;
    use rsa::pkcs1v15::SigningKey;
    use rsa::RsaPrivateKey;
    use sha2::Sha256;
    use std::str::FromStr;
    use std::time::Duration;
    use x509_cert::builder::{Builder, CertificateBuilder, Profile};
    use x509_cert::name::Name;
    use x509_cert::serial_number::SerialNumber;
    use x509_cert::spki::EncodePublicKey;
    use x509_cert::time::Validity;

    /// PEM encoding of a self-signed signer's key and certificate (tests).
    ///
    /// # Panics
    /// On encoding failure (tests only).
    #[must_use]
    pub fn to_pem(signer: &SoftwareSigner) -> (String, String) {
        use base64::Engine as _;
        use rsa::pkcs8::EncodePrivateKey;
        let key = signer
            .key
            .to_pkcs8_pem(rsa::pkcs8::LineEnding::LF)
            .expect("pem")
            .to_string();
        let b64 = base64::engine::general_purpose::STANDARD.encode(&signer.cert_der);
        let mut cert = String::from("-----BEGIN CERTIFICATE-----\n");
        for chunk in b64.as_bytes().chunks(64) {
            cert.push_str(std::str::from_utf8(chunk).unwrap_or_default());
            cert.push('\n');
        }
        cert.push_str("-----END CERTIFICATE-----\n");
        (key, cert)
    }

    /// Signer that always fails (for negative tests).
    pub struct NullSigner;

    impl super::Signer for NullSigner {
        fn certificate_der(&self) -> &[u8] {
            &[]
        }
        fn sign_sha256(&self, _digest: &[u8; 32]) -> super::Result<Vec<u8>> {
            Err(super::SignError::Signer("null signer".into()))
        }
    }

    /// Creates a self-signed 2048-bit RSA signer with the given common name.
    ///
    /// # Panics
    /// On RNG/encoding failure (tests only).
    #[must_use]
    pub fn self_signed(rng: &mut impl rsa::rand_core::CryptoRngCore, cn: &str) -> SoftwareSigner {
        let key = RsaPrivateKey::new(rng, 2048).expect("rsa keygen");
        let pub_der = key.to_public_key().to_public_key_der().expect("spki");
        let spki = spki::SubjectPublicKeyInfoOwned::try_from(pub_der.as_bytes()).expect("spki");
        let subject = Name::from_str(&format!("CN={cn},O=Bluestar Test,C=VN")).expect("name");
        let signer = SigningKey::<Sha256>::new(key.clone());
        let builder = CertificateBuilder::new(
            Profile::Root,
            SerialNumber::from(1u32),
            Validity::from_now(Duration::from_hours(24)).expect("validity"),
            subject,
            spki,
            &signer,
        )
        .expect("builder");
        let cert = builder.build::<rsa::pkcs1v15::Signature>().expect("cert");
        SoftwareSigner::new(key, cert.to_der().expect("der"))
    }
}
