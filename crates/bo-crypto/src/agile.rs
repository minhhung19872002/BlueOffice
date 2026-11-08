//! Agile Encryption ([MS-OFFCRYPTO] §2.3.4.10 – §2.3.4.15).

use crate::info::{write_agile, AgileInfo, CipherParams, PasswordKeyEncryptor};
use crate::primitives::{
    aes_cbc_decrypt, aes_cbc_encrypt, ct_eq, fit, pad16, utf16le, HashAlgorithm,
};
use crate::{CryptoError, Result, MAX_STREAM_SIZE};

/// §2.3.4.13 block keys for the password key encryptor.
const BLOCK_VERIFIER_INPUT: [u8; 8] = [0xFE, 0xA7, 0xD2, 0x76, 0x3B, 0x4B, 0x9E, 0x79];
const BLOCK_VERIFIER_VALUE: [u8; 8] = [0xD7, 0xAA, 0x0F, 0x6D, 0x30, 0x61, 0x34, 0x4E];
const BLOCK_KEY_VALUE: [u8; 8] = [0x14, 0x6E, 0x0B, 0xE7, 0xAB, 0xAC, 0xD0, 0xD6];
/// §2.3.4.14 block keys for data integrity.
const BLOCK_HMAC_KEY: [u8; 8] = [0x5F, 0xB2, 0xAD, 0x01, 0x0C, 0xB9, 0xE1, 0xF6];
const BLOCK_HMAC_VALUE: [u8; 8] = [0xA0, 0x67, 0x7F, 0x02, 0xB2, 0x2C, 0x84, 0x33];
/// §2.3.4.15: the package is encrypted in 4096-byte segments.
const SEGMENT: usize = 4096;

/// Parameters for [`crate::encrypt_agile_with`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AgileOptions {
    /// Hash algorithm for key derivation and integrity.
    pub hash: HashAlgorithm,
    /// 128, 192 or 256.
    pub key_bits: usize,
    /// Password hash iteration count.
    pub spin_count: u32,
}

impl Default for AgileOptions {
    fn default() -> Self {
        AgileOptions {
            hash: HashAlgorithm::Sha512,
            key_bits: 256,
            spin_count: 100_000,
        }
    }
}

/// §2.3.4.11 Encryption key generation for one block key.
fn password_keys(p: &PasswordKeyEncryptor, password: &str, blocks: &[[u8; 8]]) -> Vec<Vec<u8>> {
    let hash = p.params.hash;
    let h = hash.spin_iterator_first(&p.params.salt, &utf16le(password), p.spin_count);
    blocks
        .iter()
        .map(|b| fit(hash.hash(&[&h, b]), p.params.key_bits / 8))
        .collect()
}

/// §2.3.4.12 Initialization vector: H(salt + blockKey) or the salt itself.
fn iv(params: &CipherParams, block_key: Option<&[u8]>) -> Vec<u8> {
    match block_key {
        Some(b) => fit(params.hash.hash(&[&params.salt, b]), params.block_size),
        None => fit(params.salt.clone(), params.block_size),
    }
}

fn verify_and_get_key(info: &AgileInfo, password: &str) -> Result<Vec<u8>> {
    let p = &info.password;
    let keys = password_keys(
        p,
        password,
        &[BLOCK_VERIFIER_INPUT, BLOCK_VERIFIER_VALUE, BLOCK_KEY_VALUE],
    );
    let pk_iv = iv(&p.params, None);
    // §2.3.4.13 Password verification.
    let mut input = aes_cbc_decrypt(&keys[0], &pk_iv, &p.encrypted_verifier_hash_input)?;
    input.truncate(p.params.salt.len());
    let value = aes_cbc_decrypt(&keys[1], &pk_iv, &p.encrypted_verifier_hash_value)?;
    let expected = p.params.hash.hash(&[&input]);
    if value.len() < expected.len() || !ct_eq(&value[..expected.len()], &expected) {
        return Err(CryptoError::WrongPassword);
    }
    let mut key = aes_cbc_decrypt(&keys[2], &pk_iv, &p.encrypted_key_value)?;
    let n = info.key_data.key_bits / 8;
    if key.len() < n {
        return Err(CryptoError::Malformed("encryptedKeyValue too short".into()));
    }
    key.truncate(n);
    Ok(key)
}

/// §2.3.4.14 Data integrity verification.
fn check_integrity(info: &AgileInfo, key: &[u8], package: &[u8]) -> Result<()> {
    let Some((enc_key, enc_value)) = &info.data_integrity else {
        return Ok(());
    };
    let kd = &info.key_data;
    let mut hmac_key = aes_cbc_decrypt(key, &iv(kd, Some(&BLOCK_HMAC_KEY)), enc_key)?;
    let mut hmac_value = aes_cbc_decrypt(key, &iv(kd, Some(&BLOCK_HMAC_VALUE)), enc_value)?;
    if hmac_key.len() < kd.hash_size || hmac_value.len() < kd.hash_size {
        return Err(CryptoError::Malformed("dataIntegrity too short".into()));
    }
    hmac_key.truncate(kd.hash_size);
    hmac_value.truncate(kd.hash_size);
    let actual = kd.hash.hmac(&hmac_key, package);
    if ct_eq(&actual, &hmac_value) {
        Ok(())
    } else {
        Err(CryptoError::IntegrityCheckFailed)
    }
}

/// Decrypt an Agile `EncryptedPackage` stream.
pub fn decrypt(info: &AgileInfo, package: &[u8], password: &str) -> Result<Vec<u8>> {
    let key = verify_and_get_key(info, password)?;
    check_integrity(info, &key, package)?;
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
    let mut out = Vec::with_capacity(data.len());
    for (i, seg) in data.chunks(SEGMENT).enumerate() {
        if out.len() as u64 >= size {
            break;
        }
        let seg_iv = iv(&info.key_data, Some(&(i as u32).to_le_bytes()));
        let aligned = seg.len() / 16 * 16;
        out.extend(aes_cbc_decrypt(&key, &seg_iv, &seg[..aligned])?);
    }
    out.truncate(size as usize);
    Ok(out)
}

/// Encrypt `package`; returns (`EncryptionInfo`, `EncryptedPackage`).
pub fn encrypt(
    package: &[u8],
    password: &str,
    opts: &AgileOptions,
    rng: &mut dyn FnMut(&mut [u8]) -> Result<()>,
) -> Result<(Vec<u8>, Vec<u8>)> {
    if !matches!(opts.key_bits, 128 | 192 | 256) {
        return Err(CryptoError::Unsupported(format!(
            "keyBits {}",
            opts.key_bits
        )));
    }
    let mut random = |n: usize| -> Result<Vec<u8>> {
        let mut v = vec![0u8; n];
        rng(&mut v)?;
        Ok(v)
    };
    let hash = opts.hash;
    let key_data = CipherParams {
        salt: random(16)?,
        block_size: 16,
        key_bits: opts.key_bits,
        hash_size: hash.size(),
        hash,
    };
    let secret = random(opts.key_bits / 8)?;
    let verifier_input = random(16)?;
    let hmac_key = random(hash.size())?;
    let mut pk = PasswordKeyEncryptor {
        params: CipherParams {
            salt: random(16)?,
            ..key_data.clone()
        },
        spin_count: opts.spin_count,
        encrypted_verifier_hash_input: Vec::new(),
        encrypted_verifier_hash_value: Vec::new(),
        encrypted_key_value: Vec::new(),
    };
    let keys = password_keys(
        &pk,
        password,
        &[BLOCK_VERIFIER_INPUT, BLOCK_VERIFIER_VALUE, BLOCK_KEY_VALUE],
    );
    let pk_iv = iv(&pk.params, None);
    pk.encrypted_verifier_hash_input = aes_cbc_encrypt(&keys[0], &pk_iv, &verifier_input)?;
    pk.encrypted_verifier_hash_value =
        aes_cbc_encrypt(&keys[1], &pk_iv, &pad16(hash.hash(&[&verifier_input])))?;
    pk.encrypted_key_value = aes_cbc_encrypt(&keys[2], &pk_iv, &pad16(secret.clone()))?;

    // §2.3.4.15 Data encryption.
    let mut stream = (package.len() as u64).to_le_bytes().to_vec();
    for (i, seg) in package.chunks(SEGMENT).enumerate() {
        let seg_iv = iv(&key_data, Some(&(i as u32).to_le_bytes()));
        stream.extend(aes_cbc_encrypt(&secret, &seg_iv, &pad16(seg.to_vec()))?);
    }

    // §2.3.4.14 Data integrity.
    let hmac_value = hash.hmac(&hmac_key, &stream);
    let enc_hmac_key = aes_cbc_encrypt(
        &secret,
        &iv(&key_data, Some(&BLOCK_HMAC_KEY)),
        &pad16(hmac_key),
    )?;
    let enc_hmac_value = aes_cbc_encrypt(
        &secret,
        &iv(&key_data, Some(&BLOCK_HMAC_VALUE)),
        &pad16(hmac_value),
    )?;
    let info = AgileInfo {
        key_data,
        data_integrity: Some((enc_hmac_key, enc_hmac_value)),
        password: pk,
    };
    Ok((write_agile(&info), stream))
}
