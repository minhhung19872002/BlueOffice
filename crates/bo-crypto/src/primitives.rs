//! Hash / cipher helpers shared by the Standard and Agile code paths.

use crate::{CryptoError, Result};
use aes::cipher::block_padding::NoPadding;
use aes::cipher::{BlockDecryptMut, BlockEncryptMut, KeyInit, KeyIvInit};
use hmac::Mac;
use sha2::Digest;

/// Hash algorithms allowed by [MS-OFFCRYPTO] §2.3.4.10 (`hashAlgorithm`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HashAlgorithm {
    /// SHA-1 (20-byte digest).
    Sha1,
    /// SHA-256 (32-byte digest).
    Sha256,
    /// SHA-384 (48-byte digest).
    Sha384,
    /// SHA-512 (64-byte digest).
    Sha512,
}

impl HashAlgorithm {
    /// Parse the XML attribute value (`SHA1`, `SHA256`, `SHA384`, `SHA512`;
    /// the hyphenated spellings used by protection elements are accepted too).
    pub fn from_name(s: &str) -> Option<Self> {
        match s.to_ascii_uppercase().replace('-', "").as_str() {
            "SHA1" => Some(Self::Sha1),
            "SHA256" => Some(Self::Sha256),
            "SHA384" => Some(Self::Sha384),
            "SHA512" => Some(Self::Sha512),
            _ => None,
        }
    }

    /// Name as written in `EncryptionInfo` XML.
    pub fn name(self) -> &'static str {
        match self {
            Self::Sha1 => "SHA1",
            Self::Sha256 => "SHA256",
            Self::Sha384 => "SHA384",
            Self::Sha512 => "SHA512",
        }
    }

    /// Digest length in bytes.
    pub fn size(self) -> usize {
        match self {
            Self::Sha1 => 20,
            Self::Sha256 => 32,
            Self::Sha384 => 48,
            Self::Sha512 => 64,
        }
    }

    /// Hash the concatenation of `parts`.
    pub fn hash(self, parts: &[&[u8]]) -> Vec<u8> {
        fn run<D: Digest>(parts: &[&[u8]]) -> Vec<u8> {
            let mut d = D::new();
            for p in parts {
                d.update(p);
            }
            d.finalize().to_vec()
        }
        match self {
            Self::Sha1 => run::<sha1::Sha1>(parts),
            Self::Sha256 => run::<sha2::Sha256>(parts),
            Self::Sha384 => run::<sha2::Sha384>(parts),
            Self::Sha512 => run::<sha2::Sha512>(parts),
        }
    }

    /// HMAC over `data` ([MS-OFFCRYPTO] §2.3.4.14).
    pub fn hmac(self, key: &[u8], data: &[u8]) -> Vec<u8> {
        fn run<M: Mac + KeyInit>(key: &[u8], data: &[u8]) -> Vec<u8> {
            // HMAC accepts keys of any length; `new_from_slice` cannot fail.
            match <M as KeyInit>::new_from_slice(key) {
                Ok(mut m) => {
                    m.update(data);
                    m.finalize().into_bytes().to_vec()
                }
                Err(_) => Vec::new(),
            }
        }
        match self {
            Self::Sha1 => run::<hmac::Hmac<sha1::Sha1>>(key, data),
            Self::Sha256 => run::<hmac::Hmac<sha2::Sha256>>(key, data),
            Self::Sha384 => run::<hmac::Hmac<sha2::Sha384>>(key, data),
            Self::Sha512 => run::<hmac::Hmac<sha2::Sha512>>(key, data),
        }
    }

    /// Iterated password hash used for *encryption* keys ([MS-OFFCRYPTO]
    /// §2.3.4.7 / §2.3.4.11): `H0 = H(salt + password)`, `Hn = H(iterator + Hn-1)`.
    pub fn spin_iterator_first(self, salt: &[u8], password: &[u8], spins: u32) -> Vec<u8> {
        fn run<D: Digest>(salt: &[u8], password: &[u8], spins: u32) -> Vec<u8> {
            let mut h = D::new()
                .chain_update(salt)
                .chain_update(password)
                .finalize();
            for i in 0..spins {
                h = D::new()
                    .chain_update(i.to_le_bytes())
                    .chain_update(&h)
                    .finalize();
            }
            h.to_vec()
        }
        match self {
            Self::Sha1 => run::<sha1::Sha1>(salt, password, spins),
            Self::Sha256 => run::<sha2::Sha256>(salt, password, spins),
            Self::Sha384 => run::<sha2::Sha384>(salt, password, spins),
            Self::Sha512 => run::<sha2::Sha512>(salt, password, spins),
        }
    }

    /// Iterated password hash used for *protection* elements ([MS-OFFCRYPTO]
    /// §2.4.2.4, ECMA-376 Part 1 §18.2.29 `workbookProtection`):
    /// `H0 = H(salt + password)`, `Hn = H(Hn-1 + iterator)`.
    pub fn spin_iterator_last(self, salt: &[u8], password: &[u8], spins: u32) -> Vec<u8> {
        fn run<D: Digest>(salt: &[u8], password: &[u8], spins: u32) -> Vec<u8> {
            let mut h = D::new()
                .chain_update(salt)
                .chain_update(password)
                .finalize();
            for i in 0..spins {
                h = D::new()
                    .chain_update(&h)
                    .chain_update(i.to_le_bytes())
                    .finalize();
            }
            h.to_vec()
        }
        match self {
            Self::Sha1 => run::<sha1::Sha1>(salt, password, spins),
            Self::Sha256 => run::<sha2::Sha256>(salt, password, spins),
            Self::Sha384 => run::<sha2::Sha384>(salt, password, spins),
            Self::Sha512 => run::<sha2::Sha512>(salt, password, spins),
        }
    }
}

/// Password as UTF-16LE bytes, no terminator ([MS-OFFCRYPTO] §2.3.4.7).
pub fn utf16le(password: &str) -> Vec<u8> {
    password.encode_utf16().flat_map(u16::to_le_bytes).collect()
}

/// Truncate or pad with 0x36 to `len` bytes ([MS-OFFCRYPTO] §2.3.4.11/§2.3.4.12).
pub fn fit(mut v: Vec<u8>, len: usize) -> Vec<u8> {
    v.resize(len, 0x36);
    v
}

fn bad_len(what: &str) -> CryptoError {
    CryptoError::Malformed(format!(
        "{what}: length is not a multiple of the block size"
    ))
}

macro_rules! by_key_len {
    ($key:expr, $f:ident, $($args:expr),*) => {
        match $key.len() {
            16 => $f::<aes::Aes128>($key, $($args),*),
            24 => $f::<aes::Aes192>($key, $($args),*),
            32 => $f::<aes::Aes256>($key, $($args),*),
            n => Err(CryptoError::Unsupported(format!("AES key of {n} bytes"))),
        }
    };
}

fn cbc_dec<C>(key: &[u8], iv: &[u8], data: &[u8]) -> Result<Vec<u8>>
where
    C: aes::cipher::BlockDecrypt + aes::cipher::BlockCipher + KeyInit,
{
    let d = cbc::Decryptor::<C>::new_from_slices(key, iv)
        .map_err(|_| CryptoError::Malformed("bad AES key/IV length".into()))?;
    let mut buf = data.to_vec();
    let n = d
        .decrypt_padded_mut::<NoPadding>(&mut buf)
        .map_err(|_| bad_len("CBC data"))?
        .len();
    buf.truncate(n);
    Ok(buf)
}

fn cbc_enc<C>(key: &[u8], iv: &[u8], data: &[u8]) -> Result<Vec<u8>>
where
    C: aes::cipher::BlockEncrypt + aes::cipher::BlockCipher + KeyInit,
{
    let e = cbc::Encryptor::<C>::new_from_slices(key, iv)
        .map_err(|_| CryptoError::Malformed("bad AES key/IV length".into()))?;
    let mut buf = data.to_vec();
    let n = data.len();
    let out = e
        .encrypt_padded_mut::<NoPadding>(&mut buf, n)
        .map_err(|_| bad_len("CBC data"))?
        .len();
    buf.truncate(out);
    Ok(buf)
}

fn ecb_dec<C>(key: &[u8], data: &[u8]) -> Result<Vec<u8>>
where
    C: aes::cipher::BlockDecrypt + aes::cipher::BlockCipher + KeyInit,
{
    let d = ecb::Decryptor::<C>::new_from_slice(key)
        .map_err(|_| CryptoError::Malformed("bad AES key length".into()))?;
    let mut buf = data.to_vec();
    let n = d
        .decrypt_padded_mut::<NoPadding>(&mut buf)
        .map_err(|_| bad_len("ECB data"))?
        .len();
    buf.truncate(n);
    Ok(buf)
}

fn ecb_enc<C>(key: &[u8], data: &[u8]) -> Result<Vec<u8>>
where
    C: aes::cipher::BlockEncrypt + aes::cipher::BlockCipher + KeyInit,
{
    let e = ecb::Encryptor::<C>::new_from_slice(key)
        .map_err(|_| CryptoError::Malformed("bad AES key length".into()))?;
    let mut buf = data.to_vec();
    let n = data.len();
    let out = e
        .encrypt_padded_mut::<NoPadding>(&mut buf, n)
        .map_err(|_| bad_len("ECB data"))?
        .len();
    buf.truncate(out);
    Ok(buf)
}

/// AES-CBC decrypt without padding.
pub fn aes_cbc_decrypt(key: &[u8], iv: &[u8], data: &[u8]) -> Result<Vec<u8>> {
    by_key_len!(key, cbc_dec, iv, data)
}

/// AES-CBC encrypt without padding (`data` must be block aligned).
pub fn aes_cbc_encrypt(key: &[u8], iv: &[u8], data: &[u8]) -> Result<Vec<u8>> {
    by_key_len!(key, cbc_enc, iv, data)
}

/// AES-ECB decrypt without padding.
pub fn aes_ecb_decrypt(key: &[u8], data: &[u8]) -> Result<Vec<u8>> {
    by_key_len!(key, ecb_dec, data)
}

/// AES-ECB encrypt without padding (`data` must be block aligned).
pub fn aes_ecb_encrypt(key: &[u8], data: &[u8]) -> Result<Vec<u8>> {
    by_key_len!(key, ecb_enc, data)
}

/// Round `v` up to a multiple of 16 with zero bytes.
pub fn pad16(mut v: Vec<u8>) -> Vec<u8> {
    let n = v.len().div_ceil(16) * 16;
    v.resize(n, 0);
    v
}

/// Constant-time-ish equality for verifier comparisons.
pub fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FIPS 197 Appendix C.1 / C.3 AES known-answer tests through our ECB wrapper.
    #[test]
    fn aes_known_answers() {
        let pt: Vec<u8> = (0..16u8).map(|i| i * 0x11).collect();
        let k128: Vec<u8> = (0..16u8).collect();
        let ct = aes_ecb_encrypt(&k128, &pt).unwrap();
        assert_eq!(hex(&ct), "69c4e0d86a7b0430d8cdb78070b4c55a");
        assert_eq!(aes_ecb_decrypt(&k128, &ct).unwrap(), pt);
        let k256: Vec<u8> = (0..32u8).collect();
        let ct = aes_ecb_encrypt(&k256, &pt).unwrap();
        assert_eq!(hex(&ct), "8ea2b7ca516745bfeafc49904b496089");
    }

    /// FIPS 180 "abc" vectors.
    #[test]
    fn hash_known_answers() {
        assert_eq!(
            hex(&HashAlgorithm::Sha1.hash(&[b"a", b"bc"])),
            "a9993e364706816aba3e25717850c26c9cd0d89d"
        );
        assert!(hex(&HashAlgorithm::Sha512.hash(&[b"abc"])).starts_with("ddaf35a193617aba"));
    }

    #[test]
    fn cbc_round_trip_and_bad_lengths() {
        let key = [7u8; 32];
        let iv = [9u8; 16];
        let data = pad16(b"hello world".to_vec());
        let c = aes_cbc_encrypt(&key, &iv, &data).unwrap();
        assert_eq!(aes_cbc_decrypt(&key, &iv, &c).unwrap(), data);
        assert!(aes_cbc_decrypt(&key, &iv, &c[..5]).is_err());
        assert!(aes_cbc_decrypt(&key[..5], &iv, &c).is_err());
        assert!(aes_cbc_decrypt(&key, &iv[..3], &c).is_err());
    }

    pub(crate) fn hex(b: &[u8]) -> String {
        b.iter().fold(String::new(), |mut s, x| {
            use std::fmt::Write;
            let _ = write!(s, "{x:02x}");
            s
        })
    }
}
