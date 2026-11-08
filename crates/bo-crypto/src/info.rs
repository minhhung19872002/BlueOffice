//! `EncryptionInfo` stream parsing and writing.
//!
//! [MS-OFFCRYPTO] §2.3.2 Version, §2.3.4.5 Standard EncryptionInfo,
//! §2.3.4.10 Agile `\EncryptionInfo` stream.

use crate::primitives::HashAlgorithm;
use crate::{CryptoError, Result};
use base64::Engine;
use quick_xml::events::{BytesStart, Event};

/// [MS-OFFCRYPTO] §2.3.4.10: `spinCount` MUST NOT exceed 10 000 000.
pub const MAX_SPIN_COUNT: u32 = 10_000_000;

/// Parsed `EncryptionInfo`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EncryptionInfo {
    /// Standard encryption (EncryptionInfo version 2.2, 3.2 or 4.2).
    Standard(StandardInfo),
    /// Agile encryption (EncryptionInfo version 4.4, XML descriptor).
    Agile(AgileInfo),
}

/// Standard encryption parameters ([MS-OFFCRYPTO] §2.3.2 EncryptionHeader,
/// §2.3.3 EncryptionVerifier).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StandardInfo {
    /// EncryptionInfo major version.
    pub version_major: u16,
    /// EncryptionInfo minor version.
    pub version_minor: u16,
    /// EncryptionHeader.Flags (fCryptoAPI 0x04, fExternal 0x10, fAES 0x20).
    pub flags: u32,
    /// AlgID: 0x660E AES-128, 0x660F AES-192, 0x6610 AES-256, 0x6801 RC4.
    pub alg_id: u32,
    /// AlgIDHash: 0x8004 SHA-1.
    pub alg_id_hash: u32,
    /// Key size in bits.
    pub key_bits: u32,
    /// EncryptionHeader.ProviderType (0x18 for AES, 0x01 for RC4).
    pub provider_type: u32,
    /// EncryptionHeader.CSPName (cryptographic service provider name).
    pub csp_name: String,
    /// EncryptionVerifier.Salt.
    pub salt: Vec<u8>,
    /// EncryptionVerifier.EncryptedVerifier.
    pub encrypted_verifier: Vec<u8>,
    /// EncryptionVerifier.VerifierHashSize, in bytes.
    pub verifier_hash_size: u32,
    /// EncryptionVerifier.EncryptedVerifierHash.
    pub encrypted_verifier_hash: Vec<u8>,
}

/// Cipher parameters common to `keyData` and `p:encryptedKey`
/// ([MS-OFFCRYPTO] §2.3.4.10 CT_KeyData / CT_PasswordKeyEncryptor).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CipherParams {
    /// `saltValue`.
    pub salt: Vec<u8>,
    /// `blockSize`, in bytes.
    pub block_size: usize,
    /// `keyBits`, key size in bits.
    pub key_bits: usize,
    /// `hashSize`, digest length in bytes.
    pub hash_size: usize,
    /// `hashAlgorithm`.
    pub hash: HashAlgorithm,
}

/// Password key encryptor (`p:encryptedKey`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PasswordKeyEncryptor {
    /// Cipher and hash parameters of the key encryptor.
    pub params: CipherParams,
    /// `spinCount`, password hash iteration count.
    pub spin_count: u32,
    /// `encryptedVerifierHashInput`.
    pub encrypted_verifier_hash_input: Vec<u8>,
    /// `encryptedVerifierHashValue`.
    pub encrypted_verifier_hash_value: Vec<u8>,
    /// `encryptedKeyValue` (the wrapped intermediate key).
    pub encrypted_key_value: Vec<u8>,
}

/// Agile encryption description.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgileInfo {
    /// `keyData`: parameters used to encrypt the package itself.
    pub key_data: CipherParams,
    /// `dataIntegrity` (encryptedHmacKey, encryptedHmacValue), if present.
    pub data_integrity: Option<(Vec<u8>, Vec<u8>)>,
    /// The password key encryptor.
    pub password: PasswordKeyEncryptor,
}

/// Agile `keyEncryptor` URI for passwords.
pub const URI_PASSWORD: &str = "http://schemas.microsoft.com/office/2006/keyEncryptor/password";
const NS_ENCRYPTION: &str = "http://schemas.microsoft.com/office/2006/encryption";
const NS_CERT: &str = "http://schemas.microsoft.com/office/2006/keyEncryptor/certificate";

fn u16_at(b: &[u8], off: usize) -> Result<u16> {
    b.get(off..off + 2)
        .map(|s| u16::from_le_bytes([s[0], s[1]]))
        .ok_or_else(|| CryptoError::Malformed("EncryptionInfo truncated".into()))
}

fn u32_at(b: &[u8], off: usize) -> Result<u32> {
    b.get(off..off + 4)
        .map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
        .ok_or_else(|| CryptoError::Malformed("EncryptionInfo truncated".into()))
}

fn slice(b: &[u8], off: usize, len: usize) -> Result<&[u8]> {
    off.checked_add(len)
        .and_then(|end| b.get(off..end))
        .ok_or_else(|| CryptoError::Malformed("EncryptionInfo truncated".into()))
}

/// Parse an `EncryptionInfo` stream.
pub fn parse_encryption_info(data: &[u8]) -> Result<EncryptionInfo> {
    let major = u16_at(data, 0)?;
    let minor = u16_at(data, 2)?;
    match (major, minor) {
        // §2.3.4.10: Agile — Version 4.4, Reserved 0x40, then XML.
        (4, 4) => {
            let xml = data.get(8..).unwrap_or_default();
            parse_agile_xml(xml).map(EncryptionInfo::Agile)
        }
        // §2.3.4.5: Standard — major 2, 3 or 4, minor 2.
        (2..=4, 2) => parse_standard(data, major, minor).map(EncryptionInfo::Standard),
        _ => Err(CryptoError::UnsupportedVersion(major, minor)),
    }
}

fn parse_standard(d: &[u8], major: u16, minor: u16) -> Result<StandardInfo> {
    let _flags = u32_at(d, 4)?;
    let header_size = u32_at(d, 8)? as usize;
    let h = slice(d, 12, header_size)?;
    // §2.3.2 EncryptionHeader
    let flags = u32_at(h, 0)?;
    let alg_id = u32_at(h, 8)?;
    let alg_id_hash = u32_at(h, 12)?;
    let key_bits = u32_at(h, 16)?;
    let provider_type = u32_at(h, 20)?;
    let csp: Vec<u16> = (32..h.len().saturating_sub(1))
        .step_by(2)
        .map(|i| u16::from_le_bytes([h[i], h[i + 1]]))
        .take_while(|&c| c != 0)
        .collect();
    let csp_name = String::from_utf16_lossy(&csp);
    // §2.3.3 EncryptionVerifier
    let v = 12 + header_size;
    let salt_size = u32_at(d, v)? as usize;
    if salt_size != 16 {
        return Err(CryptoError::Malformed(format!("SaltSize {salt_size}")));
    }
    let salt = slice(d, v + 4, 16)?.to_vec();
    let encrypted_verifier = slice(d, v + 20, 16)?.to_vec();
    let verifier_hash_size = u32_at(d, v + 36)?;
    // AES: the encrypted hash is padded to 32 bytes; RC4 uses 20.
    let hash_len = if alg_id == 0x6801 { 20 } else { 32 };
    let encrypted_verifier_hash = slice(d, v + 40, hash_len)?.to_vec();
    Ok(StandardInfo {
        version_major: major,
        version_minor: minor,
        flags,
        alg_id,
        alg_id_hash,
        key_bits,
        provider_type,
        csp_name,
        salt,
        encrypted_verifier,
        verifier_hash_size,
        encrypted_verifier_hash,
    })
}

/// Serialize a Standard `EncryptionInfo` stream.
pub fn write_standard(info: &StandardInfo) -> Vec<u8> {
    let mut header = Vec::new();
    header.extend(info.flags.to_le_bytes());
    header.extend(0u32.to_le_bytes()); // SizeExtra
    header.extend(info.alg_id.to_le_bytes());
    header.extend(info.alg_id_hash.to_le_bytes());
    header.extend(info.key_bits.to_le_bytes());
    header.extend(info.provider_type.to_le_bytes());
    header.extend(0u32.to_le_bytes()); // Reserved1
    header.extend(0u32.to_le_bytes()); // Reserved2
    for u in info.csp_name.encode_utf16().chain(std::iter::once(0)) {
        header.extend(u.to_le_bytes());
    }
    let mut out = Vec::new();
    out.extend(info.version_major.to_le_bytes());
    out.extend(info.version_minor.to_le_bytes());
    out.extend(info.flags.to_le_bytes());
    out.extend((header.len() as u32).to_le_bytes());
    out.extend(header);
    out.extend((info.salt.len() as u32).to_le_bytes());
    out.extend(&info.salt);
    out.extend(&info.encrypted_verifier);
    out.extend(info.verifier_hash_size.to_le_bytes());
    out.extend(&info.encrypted_verifier_hash);
    out
}

fn attrs(e: &BytesStart<'_>) -> Vec<(String, String)> {
    e.attributes()
        .flatten()
        .map(|a| {
            let key = a.key.local_name().as_ref().to_owned();
            // Values are base64, numbers, URIs and algorithm names: no entities.
            let val = a.value.into_owned();
            (key, val)
        })
        .collect()
}

fn get<'a>(a: &'a [(String, String)], k: &str) -> Result<&'a str> {
    a.iter()
        .find(|(n, _)| n == k)
        .map(|(_, v)| v.as_str())
        .ok_or_else(|| CryptoError::Malformed(format!("missing attribute {k}")))
}

fn num(a: &[(String, String)], k: &str) -> Result<usize> {
    get(a, k)?
        .trim()
        .parse()
        .map_err(|_| CryptoError::Malformed(format!("bad number in {k}")))
}

fn b64(a: &[(String, String)], k: &str) -> Result<Vec<u8>> {
    let s: String = get(a, k)?.chars().filter(|c| !c.is_whitespace()).collect();
    base64::engine::general_purpose::STANDARD
        .decode(s)
        .map_err(|_| CryptoError::Malformed(format!("bad base64 in {k}")))
}

fn cipher_params(a: &[(String, String)]) -> Result<CipherParams> {
    let alg = get(a, "cipherAlgorithm")?;
    if alg != "AES" {
        return Err(CryptoError::Unsupported(format!("cipherAlgorithm {alg}")));
    }
    let chaining = get(a, "cipherChaining")?;
    if chaining != "ChainingModeCBC" {
        return Err(CryptoError::Unsupported(format!(
            "cipherChaining {chaining}"
        )));
    }
    let hash_name = get(a, "hashAlgorithm")?;
    let hash = HashAlgorithm::from_name(hash_name)
        .ok_or_else(|| CryptoError::Unsupported(format!("hashAlgorithm {hash_name}")))?;
    let key_bits = num(a, "keyBits")?;
    if !matches!(key_bits, 128 | 192 | 256) {
        return Err(CryptoError::Unsupported(format!("keyBits {key_bits}")));
    }
    let block_size = num(a, "blockSize")?;
    if block_size != 16 {
        return Err(CryptoError::Unsupported(format!("blockSize {block_size}")));
    }
    let hash_size = num(a, "hashSize")?;
    if hash_size != hash.size() {
        return Err(CryptoError::Malformed(format!(
            "hashSize {hash_size} does not match {}",
            hash.name()
        )));
    }
    let salt = b64(a, "saltValue")?;
    if salt.is_empty() || salt.len() > 64 {
        return Err(CryptoError::Malformed("saltValue size".into()));
    }
    Ok(CipherParams {
        salt,
        block_size,
        key_bits,
        hash_size,
        hash,
    })
}

fn parse_agile_xml(xml: &[u8]) -> Result<AgileInfo> {
    let mut reader = quick_xml::NsReader::from_reader(xml);
    let mut key_data = None;
    let mut integrity = None;
    let mut password = None;
    let mut in_password_encryptor = false;
    let mut depth = 0usize;
    loop {
        let ev = reader
            .read_resolved_event()
            .map_err(|e| CryptoError::Malformed(format!("EncryptionInfo XML: {e}")))?;
        let (ns, ev) = ev;
        let ns = match ns {
            quick_xml::name::ResolveResult::Bound(n) => n.0.to_owned(),
            _ => String::new(),
        };
        let (e, is_start) = match &ev {
            Event::Start(e) => (e, true),
            Event::Empty(e) => (e, false),
            Event::End(e) => {
                depth = depth.saturating_sub(1);
                if e.local_name().as_ref() == "keyEncryptor" {
                    in_password_encryptor = false;
                }
                continue;
            }
            Event::Eof => break,
            _ => continue,
        };
        if is_start {
            depth += 1;
            if depth > 32 {
                return Err(CryptoError::Malformed("XML too deep".into()));
            }
        }
        let a = attrs(e);
        match e.local_name().as_ref() {
            "keyData" if ns == NS_ENCRYPTION => key_data = Some(cipher_params(&a)?),
            "dataIntegrity" if ns == NS_ENCRYPTION => {
                integrity = Some((b64(&a, "encryptedHmacKey")?, b64(&a, "encryptedHmacValue")?));
            }
            "keyEncryptor" => {
                in_password_encryptor = get(&a, "uri").is_ok_and(|u| u == URI_PASSWORD);
            }
            "encryptedKey" if in_password_encryptor && ns != NS_CERT && password.is_none() => {
                let params = cipher_params(&a)?;
                let spin_count = num(&a, "spinCount")?;
                if spin_count > MAX_SPIN_COUNT as usize {
                    return Err(CryptoError::Malformed(format!("spinCount {spin_count}")));
                }
                password = Some(PasswordKeyEncryptor {
                    params,
                    spin_count: spin_count as u32,
                    encrypted_verifier_hash_input: b64(&a, "encryptedVerifierHashInput")?,
                    encrypted_verifier_hash_value: b64(&a, "encryptedVerifierHashValue")?,
                    encrypted_key_value: b64(&a, "encryptedKeyValue")?,
                });
            }
            _ => {}
        }
    }
    Ok(AgileInfo {
        key_data: key_data.ok_or_else(|| CryptoError::Malformed("missing keyData".into()))?,
        data_integrity: integrity,
        password: password.ok_or_else(|| {
            CryptoError::Unsupported("no password key encryptor (certificate only?)".into())
        })?,
    })
}

fn enc_b64(b: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(b)
}

fn cipher_attrs(p: &CipherParams) -> String {
    format!(
        r#"saltSize="{}" blockSize="{}" keyBits="{}" hashSize="{}" cipherAlgorithm="AES" cipherChaining="ChainingModeCBC" hashAlgorithm="{}" saltValue="{}""#,
        p.salt.len(),
        p.block_size,
        p.key_bits,
        p.hash_size,
        p.hash.name(),
        enc_b64(&p.salt)
    )
}

/// Serialize an Agile `EncryptionInfo` stream (§2.3.4.10).
pub fn write_agile(info: &AgileInfo) -> Vec<u8> {
    let mut xml = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\r\n\
         <encryption xmlns=\"http://schemas.microsoft.com/office/2006/encryption\" \
         xmlns:p=\"http://schemas.microsoft.com/office/2006/keyEncryptor/password\" \
         xmlns:c=\"http://schemas.microsoft.com/office/2006/keyEncryptor/certificate\">",
    );
    xml.push_str(&format!("<keyData {}/>", cipher_attrs(&info.key_data)));
    if let Some((k, v)) = &info.data_integrity {
        xml.push_str(&format!(
            r#"<dataIntegrity encryptedHmacKey="{}" encryptedHmacValue="{}"/>"#,
            enc_b64(k),
            enc_b64(v)
        ));
    }
    let p = &info.password;
    xml.push_str(&format!(
        r#"<keyEncryptors><keyEncryptor uri="{URI_PASSWORD}"><p:encryptedKey spinCount="{}" {} encryptedVerifierHashInput="{}" encryptedVerifierHashValue="{}" encryptedKeyValue="{}"/></keyEncryptor></keyEncryptors></encryption>"#,
        p.spin_count,
        cipher_attrs(&p.params),
        enc_b64(&p.encrypted_verifier_hash_input),
        enc_b64(&p.encrypted_verifier_hash_value),
        enc_b64(&p.encrypted_key_value),
    ));
    let mut out = Vec::with_capacity(xml.len() + 8);
    out.extend(4u16.to_le_bytes());
    out.extend(4u16.to_le_bytes());
    out.extend(0x40u32.to_le_bytes()); // Reserved, MUST be 0x00000040
    out.extend(xml.as_bytes());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_unknown_versions() {
        assert!(matches!(
            parse_encryption_info(&[1, 0, 1, 0]),
            Err(CryptoError::UnsupportedVersion(1, 1))
        ));
        assert!(parse_encryption_info(&[4, 0]).is_err());
    }

    #[test]
    fn agile_xml_round_trip() {
        let params = CipherParams {
            salt: vec![1; 16],
            block_size: 16,
            key_bits: 256,
            hash_size: 64,
            hash: HashAlgorithm::Sha512,
        };
        let info = AgileInfo {
            key_data: params.clone(),
            data_integrity: Some((vec![2; 64], vec![3; 64])),
            password: PasswordKeyEncryptor {
                params,
                spin_count: 100_000,
                encrypted_verifier_hash_input: vec![4; 16],
                encrypted_verifier_hash_value: vec![5; 64],
                encrypted_key_value: vec![6; 32],
            },
        };
        let bytes = write_agile(&info);
        assert_eq!(&bytes[..8], &[4, 0, 4, 0, 0x40, 0, 0, 0]);
        assert_eq!(
            parse_encryption_info(&bytes).unwrap(),
            EncryptionInfo::Agile(info)
        );
    }
}
