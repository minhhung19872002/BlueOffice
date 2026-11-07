#![no_main]
libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    let _ = bo_binary::vba::decompress(data);
    let _ = bo_binary::read_vba_project(data);
    let _ = bo_binary::has_macros(data);
});
