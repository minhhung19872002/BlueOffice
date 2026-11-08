#![no_main]
// SpreadsheetML reader over arbitrary bytes (OPC + XML); see also
// crates/bo-engine/tests/fuzz_smoke.rs (mutated corpus, runs in CI).
libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    let _ = bo_xlsx::read_xlsx(data);
});
