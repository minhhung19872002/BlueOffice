#![no_main]
// WordprocessingML reader over arbitrary bytes (OPC + XML); see also
// crates/bo-engine/tests/fuzz_smoke.rs (mutated corpus, runs in CI).
libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    let _ = bo_docx::read_docx(data);
});
