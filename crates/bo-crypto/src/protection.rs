//! Document / sheet / workbook protection password hashes.
//!
//! * Legacy 16-bit hash — [MS-OFFCRYPTO] §2.3.7.1 "Binary Document Password
//!   Verifier Derivation Method 1" (the `password` attribute of
//!   `sheetProtection` / `workbookProtection`, ECMA-376 Part 1 §18.2.29,
//!   §18.3.1.85, and [MS-XLS] §2.2.9).
//! * Iterated hash — [MS-OFFCRYPTO] §2.4.2.4 "ISO Write Protection Method"
//!   (`algorithmName`/`hashValue`/`saltValue`/`spinCount` attributes).
//! * WordprocessingML legacy verifier — ECMA-376 Part 1, §17.15.1.29
//!   documentProtection (also §17.15.1.93 writeProtection), the `w:hash` /
//!   `w:salt` / `w:cryptAlgorithmSid` / `w:cryptSpinCount` attributes: the
//!   password is first reduced to the 32-bit key of the legacy word processor
//!   (high word = [MS-OFFCRYPTO] §2.3.7.2 CreateXorKey_Method1, low word =
//!   §2.3.7.1), the key's bytes are reversed and written as a hex string, and
//!   that string is hashed like a password with the iterated method
//!   ([MS-OI29500] §2.1.402: salt prepended in binary form, the initial hash
//!   not counted in the spin count).

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
    /// `spinCount`, hash iteration count.
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

/// [MS-OFFCRYPTO] §2.3.7.2 `InitialCode`: start value of the high word by password length.
const INITIAL_CODE: [u16; 15] = [
    0xE1F0, 0x1D0F, 0xCC9C, 0x84C0, 0x110C, 0x0E10, 0xF1CE, 0x313E, 0x1872, 0xE139, 0xD40F, 0x84F9,
    0x280C, 0xA96A, 0x4EC3,
];

/// [MS-OFFCRYPTO] §2.3.7.2 `XorMatrix`: 15 rows of 7 values (one per character bit).
const XOR_MATRIX: [u16; 105] = [
    0xAEFC, 0x4DD9, 0x9BB2, 0x2745, 0x4E8A, 0x9D14, 0x2A09, 0x7B61, 0xF6C2, 0xFDA5, 0xEB6B, 0xC6F7,
    0x9DCF, 0x2BBF, 0x4563, 0x8AC6, 0x05AD, 0x0B5A, 0x16B4, 0x2D68, 0x5AD0, 0x0375, 0x06EA, 0x0DD4,
    0x1BA8, 0x3750, 0x6EA0, 0xDD40, 0xD849, 0xA0B3, 0x5147, 0xA28E, 0x553D, 0xAA7A, 0x44D5, 0x6F45,
    0xDE8A, 0xAD35, 0x4A4B, 0x9496, 0x390D, 0x721A, 0xEB23, 0xC667, 0x9CEF, 0x29FF, 0x53FE, 0xA7FC,
    0x5FD9, 0x47D3, 0x8FA6, 0x0F6D, 0x1EDA, 0x3DB4, 0x7B68, 0xF6D0, 0xB861, 0x60E3, 0xC1C6, 0x93AD,
    0x377B, 0x6EF6, 0xDDEC, 0x45A0, 0x8B40, 0x06A1, 0x0D42, 0x1A84, 0x3508, 0x6A10, 0xAA51, 0x4483,
    0x8906, 0x022D, 0x045A, 0x08B4, 0x1168, 0x76B4, 0xED68, 0xCAF1, 0x85C3, 0x1BA7, 0x374E, 0x6E9C,
    0x3730, 0x6E60, 0xDCC0, 0xA9A1, 0x4363, 0x86C6, 0x1DAD, 0x3331, 0x6662, 0xCCC4, 0x89A9, 0x0373,
    0x06E6, 0x0DCC, 0x1021, 0x2042, 0x4084, 0x8108, 0x1231, 0x2462, 0x48C4,
];

/// Password reduced to single bytes (low byte, or the high byte when the low byte is 0), at
/// most 15 characters (ECMA-376 Part 1, §17.15.1.29, legacy pre-processing).
fn legacy_bytes(password: &str) -> Vec<u8> {
    password
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
        .collect()
}

/// 32-bit key of the legacy word-processor password hash (ECMA-376 Part 1, §17.15.1.29):
/// high word from [MS-OFFCRYPTO] §2.3.7.2 CreateXorKey_Method1, low word the §2.3.7.1
/// verifier. `0` for an empty password. ECMA's example: "Example" → `0x64CE_ED7E`.
pub fn word_legacy_key(password: &str) -> u32 {
    let bytes = legacy_bytes(password);
    let Some(len) = bytes.len().checked_sub(1) else {
        return 0;
    };
    let mut high = INITIAL_CODE[len];
    // CurrentElement starts at 0x68 (the last matrix entry) and walks back 7 per character,
    // characters taken in reverse order, bit 6 first.
    let mut element = XOR_MATRIX.len();
    for &b in bytes.iter().rev() {
        let mut c = b;
        for _ in 0..7 {
            element -= 1;
            if c & 0x40 != 0 {
                high ^= XOR_MATRIX[element];
            }
            c = c.wrapping_mul(2);
        }
    }
    (u32::from(high) << 16) | u32::from(legacy_password_hash(password))
}

/// The key's bytes in reverse order as an upper-case hex string (ECMA's example
/// `0x64CEED7E` → `"7EEDCE64"`), the value hashed in the second stage.
pub fn word_legacy_key_hex(password: &str) -> String {
    word_legacy_key(password)
        .to_le_bytes()
        .iter()
        .map(|b| format!("{b:02X}"))
        .collect()
}

/// Attributes of a WordprocessingML legacy verifier (§17.15.1.29 `w:hash` and friends).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WordLegacyHash {
    /// Hash algorithm (from `w:cryptAlgorithmSid`).
    pub algorithm: HashAlgorithm,
    /// Base64 `w:hash`.
    pub hash: String,
    /// Base64 `w:salt` (empty: no salt).
    pub salt: String,
    /// `w:cryptSpinCount`.
    pub spin_count: u32,
}

/// Hash algorithm of `w:cryptAlgorithmSid` (ALG_ID SID numbers, [MS-OFFCRYPTO] §2.3.2;
/// ECMA-376 Part 1, §17.15.1.29): 4 = SHA-1, 12/13/14 = SHA-256/384/512. `None` for the
/// MD2/MD4/MD5/RIPEMD values or unknown numbers. A missing attribute means SHA-1, the only
/// algorithm written by the word processors that produced this form.
pub fn word_legacy_algorithm(sid: Option<&str>) -> Option<HashAlgorithm> {
    match sid.map(str::trim) {
        None | Some("4") => Some(HashAlgorithm::Sha1),
        Some("12") => Some(HashAlgorithm::Sha256),
        Some("13") => Some(HashAlgorithm::Sha384),
        Some("14") => Some(HashAlgorithm::Sha512),
        Some(_) => None,
    }
}

/// Raw legacy verifier bytes for `password`.
pub fn word_legacy_hash_bytes(
    alg: HashAlgorithm,
    password: &str,
    salt: &[u8],
    spin_count: u32,
) -> Vec<u8> {
    password_hash(alg, &word_legacy_key_hex(password), salt, spin_count)
}

/// Build a legacy verifier with a given salt (base64 values as stored in the XML).
pub fn word_legacy_hash_with_salt(
    alg: HashAlgorithm,
    password: &str,
    salt: &[u8],
    spin_count: u32,
) -> WordLegacyHash {
    let e = base64::engine::general_purpose::STANDARD;
    WordLegacyHash {
        algorithm: alg,
        hash: e.encode(word_legacy_hash_bytes(alg, password, salt, spin_count)),
        salt: e.encode(salt),
        spin_count,
    }
}

/// Verify `password` against a legacy verifier. Malformed base64 or a spin count above
/// 10 000 000 ([MS-OI29500] §2.1.402 d) yield `false`.
pub fn verify_word_legacy(password: &str, h: &WordLegacyHash) -> bool {
    let e = base64::engine::general_purpose::STANDARD;
    if h.spin_count > crate::info::MAX_SPIN_COUNT {
        return false;
    }
    let salt = if h.salt.trim().is_empty() {
        Vec::new()
    } else {
        match e.decode(h.salt.trim()) {
            Ok(s) => s,
            Err(_) => return false,
        }
    };
    let Ok(expected) = e.decode(h.hash.trim()) else {
        return false;
    };
    crate::primitives::ct_eq(
        &word_legacy_hash_bytes(h.algorithm, password, &salt, h.spin_count),
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

    /// ECMA-376 Part 1, §17.15.1.29 worked example: "Example" → high word 0x64CE, low word
    /// 0xED7E, reversed bytes "7EEDCE64".
    #[test]
    fn word_legacy_key_matches_the_standard_example() {
        assert_eq!(word_legacy_key("Example"), 0x64CE_ED7E);
        assert_eq!(word_legacy_key_hex("Example"), "7EEDCE64");
        assert_eq!(word_legacy_key(""), 0);
        // The low word is the §2.3.7.1 verifier.
        assert_eq!(word_legacy_key("test") & 0xFFFF, 0xCBEB);
        // Only 15 characters count.
        assert_eq!(
            word_legacy_key("0123456789abcdefXYZ"),
            word_legacy_key("0123456789abcde")
        );
    }

    /// The second stage written out by hand: H0 = SHA-1(salt + UTF-16LE("7EEDCE64")),
    /// Hn = SHA-1(Hn-1 + n as 32-bit little endian), spin count not counting H0.
    #[test]
    fn word_legacy_hash_follows_the_two_stages() {
        let salt = [0x10u8, 0x20, 0x30, 0x40, 0x50, 0x60, 0x70, 0x80];
        let mut h = HashAlgorithm::Sha1.hash(&[&salt, &utf16le("7EEDCE64")]);
        for i in 0u32..3 {
            h = HashAlgorithm::Sha1.hash(&[&h, &i.to_le_bytes()]);
        }
        assert_eq!(
            word_legacy_hash_bytes(HashAlgorithm::Sha1, "Example", &salt, 3),
            h
        );
        let v = word_legacy_hash_with_salt(HashAlgorithm::Sha1, "Example", &salt, 3);
        assert!(verify_word_legacy("Example", &v));
        assert!(!verify_word_legacy("example", &v));
        assert!(!verify_word_legacy("", &v));
        let mut bad = v.clone();
        bad.hash = "not base64!".into();
        assert!(!verify_word_legacy("Example", &bad));
        let mut huge = v;
        huge.spin_count = 20_000_000;
        assert!(!verify_word_legacy("Example", &huge));
    }

    #[test]
    fn word_legacy_algorithm_ids() {
        assert_eq!(word_legacy_algorithm(Some("4")), Some(HashAlgorithm::Sha1));
        assert_eq!(word_legacy_algorithm(None), Some(HashAlgorithm::Sha1));
        assert_eq!(
            word_legacy_algorithm(Some("14")),
            Some(HashAlgorithm::Sha512)
        );
        assert_eq!(word_legacy_algorithm(Some("3")), None);
    }

    #[test]
    fn spin_zero_is_plain_salted_hash() {
        let h = password_hash(HashAlgorithm::Sha1, "a", b"", 0);
        assert_eq!(h, HashAlgorithm::Sha1.hash(&[&[b'a', 0]]));
    }
}
