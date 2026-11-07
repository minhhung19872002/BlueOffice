//! Interoperability with an independent [MS-OFFCRYPTO] implementation:
//! `tests/vectors/msoffcrypto-agile.docx` was encrypted by msoffcrypto-tool 6.1.0
//! (MIT, Python) with the password "Mật khẩu 123" from
//! `tests/corpus/doc/smartart-03-grid.docx` (> 4096 bytes: msoffcrypto-tool 6.1.0
//! writes an unreadable compound file for packages below the mini-stream cutoff,
//! which it cannot decrypt itself either).

const VECTOR: &[u8] = include_bytes!("vectors/msoffcrypto-agile.docx");
const PLAIN: &[u8] = include_bytes!("../../../tests/corpus/doc/smartart-03-grid.docx");

#[test]
fn decrypts_a_package_encrypted_by_msoffcrypto_tool() {
    assert!(bo_crypto::is_encrypted(VECTOR));
    let plain = bo_crypto::decrypt(VECTOR, "Mật khẩu 123").expect("decrypt");
    assert_eq!(plain, PLAIN, "decrypted bytes equal the original package");
    assert!(bo_crypto::decrypt(VECTOR, "sai mật khẩu").is_err());
}

#[test]
fn package_encrypted_by_bo_crypto_round_trips() {
    let enc = bo_crypto::save_package(PLAIN, Some("Mật khẩu 123")).expect("encrypt");
    assert_eq!(bo_crypto::decrypt(&enc, "Mật khẩu 123").expect("decrypt"), PLAIN);
}
