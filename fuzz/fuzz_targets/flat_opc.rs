#![no_main]
// Flat OPC (`pkg:package`) and XML Spreadsheet 2003 readers (CORE-FMT-003).
libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    let _ = bo_opc::flat::is_flat_opc(data);
    if let Ok(pkg) = bo_opc::flat::read_flat(data, &bo_opc::Limits::default()) {
        let _ = bo_opc::flat::write_flat(&pkg);
    }
    let _ = bo_xlsx::is_xml_spreadsheet(data);
    if let Ok(wb) = bo_xlsx::read_xml_spreadsheet(data) {
        let _ = bo_xlsx::write_xlsx(&wb);
    }
});
