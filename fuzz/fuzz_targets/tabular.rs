#![no_main]
// SYLK, DIF, formatted text and dBASE table readers (CORE-FMT-004).
libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    let _ = bo_tabular::detect(data, "fuzz.prn");
    if let Ok(wb) = bo_tabular::read_sylk(data) {
        let _ = bo_tabular::write_sylk(&wb, &wb.sheets[0]);
    }
    if let Ok(wb) = bo_tabular::read_dif(data) {
        let _ = bo_tabular::write_dif(&wb.sheets[0]).map(|d| d.len());
    }
    let _ = bo_tabular::read_prn(data, '.');
    let _ = bo_tabular::read_dbf(data);
});
