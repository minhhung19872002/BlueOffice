#![no_main]
libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    let _ = bo_binary::read_xls(data);
});
