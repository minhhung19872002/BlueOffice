#![no_main]
// HTML and MHTML readers (CORE-FMT-003): document, worksheet tables, MIME parts.
libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    let doc = bo_html::read_html(data);
    let _ = bo_html::write_html(&doc, "fuzz");
    let _ = bo_html::read_mht(data);
    let _ = bo_html::read_workbook_html(data);
    let _ = bo_html::is_html(data);
    let _ = bo_html::is_mht(data);
});
