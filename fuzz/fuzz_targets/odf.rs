#![no_main]
libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    let _ = bo_odf::read_odt(data);
    let _ = bo_odf::read_ods(data);
    let _ = bo_odf::read_odp(data);
});
