#![no_main]
// Spreadsheet formula parser (A1 and R1C1) and its formatter; see also
// crates/bo-formula/tests/proptest_roundtrip.rs (arbitrary_text_never_panics).
use bo_formula::{format, parse, parse_with, ParseOptions, RefStyle};

libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    let Ok(src) = std::str::from_utf8(data) else {
        return;
    };
    if let Ok(e) = parse(src) {
        let _ = parse(&format(&e));
    }
    let r1c1 = ParseOptions {
        style: RefStyle::R1C1,
        origin_row: 5,
        origin_col: 5,
    };
    let _ = parse_with(src, &r1c1);
});
