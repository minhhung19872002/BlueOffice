#![no_main]
libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    let _ = bo_opc::Package::read(data, &bo_opc::Limits::default());
});
