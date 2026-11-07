//! Standard Encryption ([MS-OFFCRYPTO] §2.3.4.5 – §2.3.4.9).

use crate::info::{write_standard, StandardInfo};
use crate::primitives::{aes_ecb_decrypt, aes_ecb_encrypt, ct_eq, pad16, utf16le, HashAlgorithm};
use crate::{CryptoError, Result, MAX_STREAM_SIZE};

/// §2.3.4.7: fixed iteration count for Standard encryption.
const SPIN_COUNT: u32 = 50_000;
const ALG_AES128: u32 = 0x660E;
const ALG_AES192: u32 = 0x660F;
const ALG_AES256: u32 = 0x6610;
const ALG_SHA1: u32 = 0x8004;
/// §2.3.2: fCryptoAPI | fAES.
const FLAGS_AES: u32 = 0x24;
/// PROV_RSA_AES
const PROV_RSA_AES: u32 = 0x18;

/// §2.3.4.7 Encryption key derivation (block number 0).
pub fn derive_key(password: &str, salt: &[u8], key_bits: u32) -> Vec<u8> {
    let sha1 = HashAlgorithm::Sha1;
    let h = sha1.spin_iterator_first(salt, &utf16le(password), SPIN_COUNT);
    let hfinal = sha1.hash(&[&h, &0u32.to_le_bytes()]);
    let mut buf1 = [0x36u8; 64];
    let mut buf2 = [0x5Cu8; 64];
    for (i, b) in hfinal.iter().enumerate() {
        buf1[i] ^= b;
        buf2[i] ^= b;
    }
    let mut x3 = sha1.hash(&[&buf1]);
    x3.extend(sha1.hash(&[&buf2]));
    x3.truncate((key_bits / 8) as usize);
    x3
}

fn check_params(info: &StandardInfo) -> Result<()> {
    if !matches!(info.alg_id, ALG_AES128 | ALG_AES192 | ALG_AES256) {
        return Err(CryptoError::Unsupported(format!(
            "Standard AlgID 0x{:X} (only AES is supported)",
            info.alg_id
        )));
    }
    if info.alg_id_hash != ALG_SHA1 && info.alg_id_hash != 0 {
        return Err(CryptoError::Unsupported(format!(
            "AlgIDHash 0x{:X}",
            info.alg_id_hash
        )));
    }
    if !matches!(info.key_bits, 128 | 192 | 256) {
        return Err(CryptoError::Unsupported(format!(
            "KeySize {}",
            info.key_bits
        )));
    }
    Ok(())
}

/// §2.3.4.9 Password verification.
pub fn verify(info: &StandardInfo, key: &[u8]) -> Result<bool> {
    let verifier = aes_ecb_decrypt(key, &info.encrypted_verifier)?;
    let hash = aes_ecb_decrypt(key, &info.encrypted_verifier_hash)?;
    let expected = HashAlgorithm::Sha1.hash(&[&verifier]);
    Ok(hash.len() >= 20 && ct_eq(&hash[..20], &expected))
}

fn split_package(package: &[u8]) -> Result<(u64, &[u8])> {
    let size_bytes: [u8; 8] = package
        .get(..8)
        .and_then(|s| s.try_into().ok())
        .ok_or_else(|| CryptoError::Malformed("EncryptedPackage too short".into()))?;
    let size = u64::from_le_bytes(size_bytes);
    let data = &package[8..];
    if size > MAX_STREAM_SIZE || size > data.len() as u64 {
        return Err(CryptoError::Malformed(format!(
            "EncryptedPackage StreamSize {size} exceeds data"
        )));
    }
    Ok((size, data))
}

/// Decrypt a Standard-encrypted `EncryptedPackage` stream (§2.3.4.4: AES-ECB).
pub fn decrypt(info: &StandardInfo, package: &[u8], password: &str) -> Result<Vec<u8>> {
    check_params(info)?;
    let key = derive_key(password, &info.salt, info.key_bits);
    if !verify(info, &key)? {
        return Err(CryptoError::WrongPassword);
    }
    let (size, data) = split_package(package)?;
    let aligned = data.len() / 16 * 16;
    let mut plain = aes_ecb_decrypt(&key, &data[..aligned])?;
    plain.truncate(size as usize);
    Ok(plain)
}

/// Encrypt with Standard encryption (AES-128, SHA-1). Returns
/// (`EncryptionInfo`, `EncryptedPackage`) stream contents.
pub fn encrypt(
    package: &[u8],
    password: &str,
    rng: &mut dyn FnMut(&mut [u8]) -> Result<()>,
) -> Result<(Vec<u8>, Vec<u8>)> {
    let mut salt = vec![0u8; 16];
    rng(&mut salt)?;
    let mut verifier = vec![0u8; 16];
    rng(&mut verifier)?;
    let key = derive_key(password, &salt, 128);
    let encrypted_verifier = aes_ecb_encrypt(&key, &verifier)?;
    let hash = pad16(HashAlgorithm::Sha1.hash(&[&verifier]));
    let encrypted_verifier_hash = aes_ecb_encrypt(&key, &hash)?;
    let info = StandardInfo {
        version_major: 4,
        version_minor: 2,
        flags: FLAGS_AES,
        alg_id: ALG_AES128,
        alg_id_hash: ALG_SHA1,
        key_bits: 128,
        provider_type: PROV_RSA_AES,
        csp_name: "Microsoft Enhanced RSA and AES Cryptographic Provider".into(),
        salt,
        encrypted_verifier,
        verifier_hash_size: 20,
        encrypted_verifier_hash,
    };
    let mut out = (package.len() as u64).to_le_bytes().to_vec();
    out.extend(aes_ecb_encrypt(&key, &pad16(package.to_vec()))?);
    Ok((write_standard(&info), out))
}
