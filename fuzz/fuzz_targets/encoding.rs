#![no_main]
libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    let _ = bo_encoding::tcvn3_bytes_to_unicode(data);
    let _ = bo_encoding::vni_bytes_to_unicode(data);
    if let Ok(s) = std::str::from_utf8(data) {
        let _ = bo_encoding::detect_legacy_vietnamese(s);
        let _ = bo_encoding::unicode_to_tcvn3(s);
        let _ = bo_encoding::unicode_to_vni(s);
    }
});
