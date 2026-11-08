//! USB token signing through PKCS#11 (OASIS PKCS #11 v2.40 / v3.0).
//!
//! Vietnamese CAs ship a PKCS#11 module with their tokens (a `.dll` on
//! Windows); the path is configuration. The private key never leaves the
//! token: `C_Sign` with `CKM_RSA_PKCS` (PKCS #11 v2.40 §2.1.6) receives the
//! DER `DigestInfo` of the SHA-256 digest (RFC 8017 §9.2), which is exactly
//! PKCS#1 v1.5 `sha256WithRSAEncryption`.
//!
//! The FFI lives in the `cryptoki` crate (Apache-2.0); this module stays
//! `forbid(unsafe_code)`.

use crate::{verify_with_cert, Result, SignError, Signer};
use cryptoki::context::{CInitializeArgs, CInitializeFlags, Pkcs11};
use cryptoki::mechanism::Mechanism;
use cryptoki::object::{Attribute, AttributeType, ObjectClass, ObjectHandle};
use cryptoki::session::{Session, UserType};
use cryptoki::slot::Slot;
use cryptoki::types::AuthPin;
use der::Decode;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// DER prefix of `DigestInfo { sha256, OCTET STRING (32) }` (RFC 8017 §9.2 note 1).
const SHA256_DIGEST_INFO_PREFIX: [u8; 19] = [
    0x30, 0x31, 0x30, 0x0d, 0x06, 0x09, 0x60, 0x86, 0x48, 0x01, 0x65, 0x03, 0x04, 0x02, 0x01, 0x05,
    0x00, 0x04, 0x20,
];

/// `DigestInfo` DER for a SHA-256 digest (input of `CKM_RSA_PKCS`).
#[must_use]
pub fn sha256_digest_info(digest: &[u8; 32]) -> Vec<u8> {
    let mut v = SHA256_DIGEST_INFO_PREFIX.to_vec();
    v.extend_from_slice(digest);
    v
}

fn p11(e: impl std::fmt::Display) -> SignError {
    SignError::Signer(format!("PKCS#11: {e}"))
}

/// A certificate found on a token.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TokenCertificate {
    /// PKCS#11 slot id holding the token.
    pub slot: u64,
    /// Token label (`CK_TOKEN_INFO.label`).
    pub token_label: String,
    /// Token serial number (`CK_TOKEN_INFO.serialNumber`).
    pub token_serial: String,
    /// `CKA_LABEL` of the certificate object.
    pub label: String,
    /// `CKA_ID` (hex) — links the certificate to its private key.
    pub id: String,
    /// Certificate subject (RFC 4514).
    pub subject: String,
    /// Certificate issuer (RFC 4514).
    pub issuer: String,
    /// `YYYY-MM-DDThh:mm:ssZ`.
    pub not_after: String,
}

/// Which key to use.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TokenSelection {
    #[serde(default)]
    /// Slot id; the first slot with a token otherwise.
    pub slot: Option<u64>,
    /// `CKA_ID` hex of the certificate/key pair; first certificate otherwise.
    #[serde(default)]
    pub id: Option<String>,
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes.iter().fold(String::new(), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

fn load(module: &Path) -> Result<Pkcs11> {
    if !module.is_file() {
        return Err(SignError::Signer(format!(
            "PKCS#11 module not found: {}",
            module.display()
        )));
    }
    let ctx = Pkcs11::new(module).map_err(p11)?;
    match ctx.initialize(CInitializeArgs::new(CInitializeFlags::OS_LOCKING_OK)) {
        Ok(())
        | Err(cryptoki::error::Error::Pkcs11(
            cryptoki::error::RvError::CryptokiAlreadyInitialized,
            _,
        )) => Ok(ctx),
        Err(e) => Err(p11(e)),
    }
}

/// `(DER, CKA_LABEL, CKA_ID)` of a certificate object.
type TokenObject = (Vec<u8>, String, Vec<u8>);

fn certificates_in(session: &Session) -> Result<Vec<TokenObject>> {
    let handles = session
        .find_objects(&[Attribute::Class(ObjectClass::CERTIFICATE)])
        .map_err(p11)?;
    let mut out = Vec::new();
    for h in handles {
        let attrs = session
            .get_attributes(
                h,
                &[
                    AttributeType::Value,
                    AttributeType::Label,
                    AttributeType::Id,
                ],
            )
            .map_err(p11)?;
        let (mut value, mut label, mut id) = (None, String::new(), Vec::new());
        for a in attrs {
            match a {
                Attribute::Value(v) => value = Some(v),
                Attribute::Label(l) => label = String::from_utf8_lossy(&l).into_owned(),
                Attribute::Id(i) => id = i,
                _ => {}
            }
        }
        if let Some(v) = value {
            out.push((v, label, id));
        }
    }
    Ok(out)
}

/// Lists the certificates on every token of `module` (no PIN needed).
pub fn list_certificates(module: &Path) -> Result<Vec<TokenCertificate>> {
    let ctx = load(module)?;
    let mut out = Vec::new();
    for slot in ctx.get_slots_with_token().map_err(p11)? {
        let info = ctx.get_token_info(slot).map_err(p11)?;
        let session = ctx.open_ro_session(slot).map_err(p11)?;
        for (der, label, id) in certificates_in(&session)? {
            let Ok(cert) = x509_cert::Certificate::from_der(&der) else {
                continue;
            };
            let tbs = &cert.tbs_certificate;
            out.push(TokenCertificate {
                slot: slot.id(),
                token_label: info.label().trim().to_owned(),
                token_serial: info.serial_number().trim().to_owned(),
                label,
                id: hex(&id),
                subject: tbs.subject.to_string(),
                issuer: tbs.issuer.to_string(),
                not_after: crate::iso_utc(tbs.validity.not_after.to_unix_duration().as_secs()),
            });
        }
    }
    Ok(out)
}

/// [`Signer`] on a USB token. Keeps a logged-in session open while alive.
pub struct Pkcs11Signer {
    // Field order: the session must close before the context finalizes.
    session: Session,
    _ctx: Pkcs11,
    key: ObjectHandle,
    cert: Vec<u8>,
    chain: Vec<Vec<u8>>,
}

impl Pkcs11Signer {
    /// Opens `module`, logs in with `pin` and selects the key.
    pub fn open(module: &Path, selection: &TokenSelection, pin: &str) -> Result<Self> {
        let ctx = load(module)?;
        let slots = ctx.get_slots_with_token().map_err(p11)?;
        let slot: Slot = match selection.slot {
            Some(id) => *slots
                .iter()
                .find(|s| s.id() == id)
                .ok_or_else(|| SignError::Signer(format!("PKCS#11: no token in slot {id}")))?,
            None => *slots
                .first()
                .ok_or_else(|| SignError::Signer("PKCS#11: no token present".into()))?,
        };
        let session = ctx.open_ro_session(slot).map_err(p11)?;
        session
            .login(UserType::User, Some(&AuthPin::new(pin.into())))
            .map_err(p11)?;
        let certs = certificates_in(&session)?;
        let want = selection.id.as_deref().map(str::to_ascii_lowercase);
        let (cert, _, id) = certs
            .iter()
            .find(|(_, _, id)| want.as_deref().is_none_or(|w| hex(id) == w))
            .cloned()
            .ok_or_else(|| SignError::Signer("PKCS#11: certificate not found on token".into()))?;
        let key = *session
            .find_objects(&[
                Attribute::Class(ObjectClass::PRIVATE_KEY),
                Attribute::Id(id.clone()),
            ])
            .map_err(p11)?
            .first()
            .ok_or_else(|| {
                SignError::Signer("PKCS#11: no private key for the certificate".into())
            })?;
        // Other certificates on the token (issuing CAs) form the chain.
        let chain = certs
            .into_iter()
            .filter(|(der, _, other)| *other != id && *der != cert)
            .map(|(der, _, _)| der)
            .collect();
        Ok(Self {
            session,
            _ctx: ctx,
            key,
            cert,
            chain,
        })
    }

    /// Module path given as configuration (e.g. `BO_PKCS11_MODULE`).
    #[must_use]
    pub fn module_from_env() -> Option<PathBuf> {
        std::env::var_os("BO_PKCS11_MODULE").map(PathBuf::from)
    }
}

impl Signer for Pkcs11Signer {
    fn certificate_der(&self) -> &[u8] {
        &self.cert
    }

    fn chain_der(&self) -> Vec<Vec<u8>> {
        self.chain.clone()
    }

    fn sign_sha256(&self, digest: &[u8; 32]) -> Result<Vec<u8>> {
        // PKCS#11 v2.40 §2.3.6 CKM_ECDSA signs the hash and returns `r || s`; CKM_RSA_PKCS signs a
        // DigestInfo (RFC 8017 §9.2).
        let sig = if crate::KeyKind::of_cert(&self.cert)? == crate::KeyKind::Rsa {
            self.session
                .sign(&Mechanism::RsaPkcs, self.key, &sha256_digest_info(digest))
                .map_err(p11)?
        } else {
            let raw = self
                .session
                .sign(&Mechanism::Ecdsa, self.key, digest)
                .map_err(p11)?;
            crate::normalize_signature(&self.cert, &raw)?
        };
        verify_with_cert(&self.cert, digest, &sig)
            .map_err(|_| SignError::Signer("PKCS#11: token signature does not verify".into()))?;
        Ok(sig)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn digest_info_is_pkcs1_sha256() {
        // The PKCS#1 v1.5 prefix rsa uses for SHA-256 must match ours.
        let d = [7u8; 32];
        let info = sha256_digest_info(&d);
        assert_eq!(info.len(), 51);
        assert_eq!(&info[19..], &d);
        let parsed = spki::AlgorithmIdentifierOwned::from_der(&info[2..17]).unwrap();
        assert_eq!(parsed.oid, const_oid::db::rfc5912::ID_SHA_256);
    }

    #[test]
    fn missing_module_is_an_error() {
        let p = Path::new("Z:/does/not/exist/token.dll");
        assert!(list_certificates(p).is_err());
        assert!(Pkcs11Signer::open(p, &TokenSelection::default(), "1234").is_err());
    }
}
