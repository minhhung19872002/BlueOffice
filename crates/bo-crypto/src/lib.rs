//! Password-protected Office documents per [MS-OFFCRYPTO].
//!
//! An encrypted OOXML file is not a ZIP but a Compound File ([MS-CFB]) holding
//! two streams ([MS-OFFCRYPTO] §2.3.4.4 / §2.3.4.10):
//!
//! * `EncryptionInfo` — algorithm parameters, salts and the password verifier;
//! * `EncryptedPackage` — 8-byte plaintext size followed by the encrypted ZIP.
//!
//! Supported:
//! * **Standard Encryption** (version 3.2 / 4.2, AES-128/192/256 ECB, SHA-1,
//!   50 000 spins) — decrypt and encrypt;
//! * **Agile Encryption** (version 4.4, AES + CBC per 4096-byte segment,
//!   SHA-1/256/384/512) — decrypt (with HMAC data-integrity check) and encrypt
//!   (AES-256, SHA-512, 100 000 spins, the defaults of current Office versions);
//! * sheet/workbook/document **protection hashes** ([`protection`]).
//!
//! RC4 / CryptoAPI encryption of the binary formats is not implemented.
#![forbid(unsafe_code)]

mod agile;
mod cfbio;
mod info;
mod primitives;
pub mod protection;
mod standard;

pub use agile::AgileOptions;
pub use info::{
    parse_encryption_info, AgileInfo, CipherParams, EncryptionInfo, PasswordKeyEncryptor,
    StandardInfo,
};
pub use primitives::HashAlgorithm;

/// Errors from this crate.
#[derive(Debug, thiserror::Error)]
pub enum CryptoError {
    #[error("compound file error: {0}")]
    Cfb(#[from] std::io::Error),
    #[error("not an encrypted Office package (no EncryptionInfo/EncryptedPackage streams)")]
    NotEncrypted,
    #[error("unsupported EncryptionInfo version {0}.{1}")]
    UnsupportedVersion(u16, u16),
    #[error("unsupported encryption parameters: {0}")]
    Unsupported(String),
    #[error("malformed encryption data: {0}")]
    Malformed(String),
    #[error("wrong password")]
    WrongPassword,
    #[error("data integrity check failed (HMAC mismatch)")]
    IntegrityCheckFailed,
    #[error("random number generator failure: {0}")]
    Random(String),
}

/// Result alias for this crate.
pub type Result<T> = std::result::Result<T, CryptoError>;

/// Maximum stream size read from an encrypted container (1 GiB).
pub const MAX_STREAM_SIZE: u64 = 1 << 30;

/// Whether `bytes` is a compound file containing an encrypted OOXML package.
pub fn is_encrypted(bytes: &[u8]) -> bool {
    cfbio::read_streams(bytes).is_ok()
}

/// Parse the `EncryptionInfo` stream of an encrypted container.
pub fn read_encryption_info(bytes: &[u8]) -> Result<EncryptionInfo> {
    let (info, _) = cfbio::read_streams(bytes)?;
    parse_encryption_info(&info)
}

/// Decrypt an encrypted container and return the plain OPC (ZIP) bytes.
pub fn decrypt(bytes: &[u8], password: &str) -> Result<Vec<u8>> {
    let (info, package) = cfbio::read_streams(bytes)?;
    decrypt_streams(&info, &package, password)
}

/// Decrypt given the raw `EncryptionInfo` and `EncryptedPackage` streams.
pub fn decrypt_streams(info: &[u8], package: &[u8], password: &str) -> Result<Vec<u8>> {
    match parse_encryption_info(info)? {
        EncryptionInfo::Standard(s) => standard::decrypt(&s, package, password),
        EncryptionInfo::Agile(a) => agile::decrypt(&a, package, password),
    }
}

/// Encrypt plain OPC bytes with Agile Encryption (AES-256-CBC, SHA-512,
/// 100 000 spins) using the operating system RNG; returns a compound file.
pub fn encrypt(package: &[u8], password: &str) -> Result<Vec<u8>> {
    encrypt_agile_with(package, password, &AgileOptions::default(), &mut os_random)
}

/// Agile encryption with explicit options and random source (deterministic tests).
pub fn encrypt_agile_with(
    package: &[u8],
    password: &str,
    options: &AgileOptions,
    rng: &mut dyn FnMut(&mut [u8]) -> Result<()>,
) -> Result<Vec<u8>> {
    let (info, enc) = agile::encrypt(package, password, options, rng)?;
    cfbio::write_container(&info, &enc)
}

/// Standard encryption (AES-128-ECB, SHA-1) — for interoperability with older
/// readers; prefer [`encrypt`].
pub fn encrypt_standard_with(
    package: &[u8],
    password: &str,
    rng: &mut dyn FnMut(&mut [u8]) -> Result<()>,
) -> Result<Vec<u8>> {
    let (info, enc) = standard::encrypt(package, password, rng)?;
    cfbio::write_container(&info, &enc)
}

/// Fill `buf` from the operating system RNG.
pub fn os_random(buf: &mut [u8]) -> Result<()> {
    getrandom::fill(buf).map_err(|e| CryptoError::Random(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn counter_rng() -> impl FnMut(&mut [u8]) -> Result<()> {
        let mut n = 0u8;
        move |b: &mut [u8]| {
            for x in b.iter_mut() {
                n = n.wrapping_mul(31).wrapping_add(7);
                *x = n;
            }
            Ok(())
        }
    }

    fn sample_package(len: usize) -> Vec<u8> {
        let mut v = b"PK\x03\x04".to_vec();
        v.extend((0..len).map(|i| (i * 7 % 251) as u8));
        v
    }

    #[test]
    fn agile_round_trip() {
        let plain = sample_package(10_000); // spans 3 segments
        let opts = AgileOptions {
            spin_count: 1000,
            ..AgileOptions::default()
        };
        let enc = encrypt_agile_with(&plain, "Mật khẩu 123", &opts, &mut counter_rng()).unwrap();
        assert!(is_encrypted(&enc));
        assert!(matches!(
            read_encryption_info(&enc).unwrap(),
            EncryptionInfo::Agile(_)
        ));
        assert_eq!(decrypt(&enc, "Mật khẩu 123").unwrap(), plain);
        assert!(matches!(
            decrypt(&enc, "wrong"),
            Err(CryptoError::WrongPassword)
        ));
    }

    #[test]
    fn agile_default_options_round_trip() {
        let plain = sample_package(4096 - 4); // exactly one segment
        let enc = encrypt(&plain, "password").unwrap();
        match read_encryption_info(&enc).unwrap() {
            EncryptionInfo::Agile(a) => {
                assert_eq!(a.password.spin_count, 100_000);
                assert_eq!(a.key_data.key_bits, 256);
                assert_eq!(a.key_data.hash, HashAlgorithm::Sha512);
            }
            EncryptionInfo::Standard(_) => panic!("expected agile"),
        }
        assert_eq!(decrypt(&enc, "password").unwrap(), plain);
    }

    #[test]
    fn agile_tamper_detected() {
        let plain = sample_package(100);
        let opts = AgileOptions {
            spin_count: 10,
            ..AgileOptions::default()
        };
        let (info, mut pkg) = agile::encrypt(&plain, "x", &opts, &mut counter_rng()).unwrap();
        let last = pkg.len() - 1;
        pkg[last] ^= 1;
        assert!(matches!(
            decrypt_streams(&info, &pkg, "x"),
            Err(CryptoError::IntegrityCheckFailed)
        ));
    }

    #[test]
    fn standard_round_trip() {
        let plain = sample_package(5000);
        let enc = encrypt_standard_with(&plain, "secret", &mut counter_rng()).unwrap();
        match read_encryption_info(&enc).unwrap() {
            EncryptionInfo::Standard(s) => {
                assert_eq!(s.key_bits, 128);
                assert_eq!(s.alg_id, 0x660E);
            }
            EncryptionInfo::Agile(_) => panic!("expected standard"),
        }
        assert_eq!(decrypt(&enc, "secret").unwrap(), plain);
        assert!(matches!(
            decrypt(&enc, "Secret"),
            Err(CryptoError::WrongPassword)
        ));
    }

    #[test]
    fn container_has_dataspaces() {
        let enc = encrypt_standard_with(b"PK", "a", &mut counter_rng()).unwrap();
        let cf = cfb::CompoundFile::open(std::io::Cursor::new(enc)).unwrap();
        assert!(cf.is_stream("/\u{6}DataSpaces/DataSpaceMap"));
        assert!(cf.is_stream("/\u{6}DataSpaces/Version"));
        assert!(
            cf.is_stream("/\u{6}DataSpaces/TransformInfo/StrongEncryptionTransform/\u{6}Primary")
        );
    }

    #[test]
    fn not_encrypted() {
        assert!(!is_encrypted(b"PK\x03\x04 plain zip"));
        assert!(matches!(decrypt(b"", "x"), Err(CryptoError::Cfb(_))));
    }

    proptest::proptest! {
        #[test]
        fn random_bytes_never_panic(data in proptest::collection::vec(proptest::num::u8::ANY, 0..2048)) {
            let _ = parse_encryption_info(&data);
            let _ = decrypt(&data, "x");
            let _ = decrypt_streams(&data, &data, "x");
        }
    }

    proptest::proptest! {
        // Standard encryption always spins 50 000 times; keep the case count modest.
        #![proptest_config(proptest::prelude::ProptestConfig::with_cases(48))]

        #[test]
        fn mutated_info_never_panics(pos in 0usize..4096, val in proptest::num::u8::ANY) {
            let plain = sample_package(64);
            let opts = AgileOptions { spin_count: 1, ..AgileOptions::default() };
            let (mut info, pkg) = agile::encrypt(&plain, "x", &opts, &mut counter_rng()).unwrap();
            let i = pos % info.len();
            info[i] = val;
            let _ = decrypt_streams(&info, &pkg, "x");
            let (sinfo, spkg) = standard::encrypt(&plain, "x", &mut counter_rng()).unwrap();
            let mut sinfo = sinfo;
            let j = pos % sinfo.len();
            sinfo[j] = val;
            let _ = decrypt_streams(&sinfo, &spkg, "x");
        }
    }
}
