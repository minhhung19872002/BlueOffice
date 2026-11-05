//! Document / sheet / workbook protection password hashes.
//!
//! * Legacy 16-bit hash — [MS-OFFCRYPTO] §2.3.7.1 "Binary Document Password
//!   Verifier Derivation Method 1" (the `password` attribute of
//!   `sheetProtection` / `workbookProtection`, ECMA-376 Part 1 §18.2.29,
//!   §18.3.1.85, and [MS-XLS] §2.2.9).
//! * Iterated hash — [MS-OFFCRYPTO] §2.4.2.4 "ISO Write Protection Method"
//!   (`algorithmName`/`hashValue`/`saltValue`/`spinCount` attributes).

use crate::primitives::{utf16le, HashAlgorithm};
use crate::Result;
use base64::Engine;

/// Legacy 16-bit password verifier (§2.3.7.1). Returns e.g. `0xCBEB` for "test".
pub fn legacy_password_hash(password: &str) -> u16 {
    // Unicode → single byte: low byte, or the high byte when the low byte is 0;
    // at most 15 characters are significant.
    let bytes: Vec<u8> = password
        .encode_utf16()
        .take(15)
        .map(|u| {
            let lo = (u & 0xFF) as u8;
            if lo == 0 {
                (u >> 8) as u8
            } else {
                lo
            }
        })
        .collect();
    let mut verifier: u16 = 0;
    // PasswordArray = length byte followed by the bytes, processed in reverse.
    let len = bytes.len() as u8;
    for &b in bytes.iter().rev().chain(std::iter::once(&len)) {
        let top = u16::from(verifier & 0x4000 != 0);
        verifier = (((verifier << 1) & 0x7FFF) | top) ^ u16::from(b);
    }
    if bytes.is_empty() {
        0
    } else {
        verifier ^ 0xCE4B
    }
}

/// Legacy hash as the 4-digit upper-case hex string used in XML attributes.
pub fn legacy_password_hash_hex(password: &str) -> String {
    format!("{:04X}", legacy_password_hash(password))
}

/// Iterated protection hash (§2.4.2.4): `H0 = H(salt + password)`,
/// `Hn = H(Hn-1 + iterator)`.
pub fn password_hash(alg: HashAlgorithm, password: &str, salt: &[u8], spin_count: u32) -> Vec<u8> {
    alg.spin_iterator_last(salt, &utf16le(password), spin_count)
}

/// Attribute values of a modern protection element.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtectionHash {
    /// `algorithmName`, e.g. `SHA-512`.
    pub algorithm_name: String,
    /// Base64 `hashValue`.
    pub hash_value: String,
    /// Base64 `saltValue`.
    pub salt_value: String,
    pub spin_count: u32,
}

/// Create a SHA-512 / 100 000-spin protection hash with a random 16-byte salt.
pub fn new_protection_hash(password: &str) -> Result<ProtectionHash> {
    let mut salt = [0u8; 16];
    crate::os_random(&mut salt)?;
    Ok(protection_hash_with_salt(password, &salt, 100_000))
}

/// Deterministic variant of [`new_protection_hash`].
pub fn protection_hash_with_salt(password: &str, salt: &[u8], spin_count: u32) -> ProtectionHash {
    let e = base64::engine::general_purpose::STANDARD;
    ProtectionHash {
        algorithm_name: "SHA-512".into(),
        hash_value: e.encode(password_hash(
            HashAlgorithm::Sha512,
            password,
            salt,
            spin_count,
        )),
        salt_value: e.encode(salt),
        spin_count,
    }
}

/// Verify a password against protection attributes. Unknown algorithms or
/// malformed base64 yield `false`. `spin_count` is capped at 10 000 000.
pub fn verify_protection(password: &str, hash: &ProtectionHash) -> bool {
    let e = base64::engine::general_purpose::STANDARD;
    let Some(alg) = HashAlgorithm::from_name(&hash.algorithm_name) else {
        return false;
    };
    if hash.spin_count > crate::info::MAX_SPIN_COUNT {
        return false;
    }
    let (Ok(salt), Ok(expected)) = (e.decode(&hash.salt_value), e.decode(&hash.hash_value)) else {
        return false;
    };
    crate::primitives::ct_eq(
        &password_hash(alg, password, &salt, hash.spin_count),
        &expected,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Values published widely for the Excel sheet-protection hash.
    #[test]
    fn legacy_known_answers() {
        assert_eq!(legacy_password_hash_hex("test"), "CBEB");
        assert_eq!(legacy_password_hash_hex("password"), "83AF");
        assert_eq!(legacy_password_hash(""), 0);
    }

    #[test]
    fn legacy_truncates_to_15_chars() {
        assert_eq!(
            legacy_password_hash("0123456789abcdefXYZ"),
            legacy_password_hash("0123456789abcde")
        );
    }

    #[test]
    fn modern_round_trip() {
        let h = protection_hash_with_salt("Bảo vệ", &[1, 2, 3, 4], 1000);
        assert!(verify_protection("Bảo vệ", &h));
        assert!(!verify_protection("bảo vệ", &h));
        let mut bad = h.clone();
        bad.algorithm_name = "MD5".into();
        assert!(!verify_protection("Bảo vệ", &bad));
        let fresh = new_protection_hash("x").unwrap();
        assert_eq!(fresh.spin_count, 100_000);
        assert!(verify_protection("x", &fresh));
    }

    #[test]
    fn spin_zero_is_plain_salted_hash() {
        let h = password_hash(HashAlgorithm::Sha1, "a", b"", 0);
        assert_eq!(h, HashAlgorithm::Sha1.hash(&[&[b'a', 0]]));
    }
}
