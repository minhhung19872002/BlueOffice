#![no_main]
libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    let _ = bo_crypto::parse_encryption_info(data);
    let _ = bo_crypto::decrypt(data, "x");
});
