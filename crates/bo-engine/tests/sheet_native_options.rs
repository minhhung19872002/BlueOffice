//! Options the native BlueSheet window sets on the engine: calculation on open, Calculate
//! Now, and dates in the Windows region's short date pattern.
use bo_engine::SheetEngine;
use bo_model::sheet::CellValue;

fn corpus(name: &str) -> Vec<u8> {
    let path = format!(
        "{}/../../tests/corpus/sheet/{name}",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read(path).expect("corpus file")
}

fn engine() -> SheetEngine {
    let mut e = SheetEngine::with_bundled_fonts();
    e.set_locale("en-US");
    e
}

#[test]
fn formulas_are_calculated_on_open_when_asked() {
    // values-1.xlsx keeps a stale saved result in C2.
    let bytes = corpus("values-1.xlsx");
    let mut kept = engine();
    kept.open(&bytes, "values-1.xlsx").expect("open");
    let saved = kept.cell(0, 1, 2).expect("C2").display;
    let mut fresh = engine();
    fresh.set_recalc_on_open(true);
    fresh.open(&bytes, "values-1.xlsx").expect("open");
    let calculated = fresh.cell(0, 1, 2).expect("C2").display;
    assert_ne!(saved, calculated, "the saved result is replaced");
    // Calculate Now reaches the same results on the first engine.
    kept.recalculate().expect("recalculate");
    assert_eq!(kept.cell(0, 1, 2).expect("C2").display, calculated);
}

#[test]
fn typed_dates_follow_the_region_pattern() {
    let mut e = engine();
    // Day first (the en-US locale alone reads month first).
    e.set_region_dates(Some("d/M/yyyy"), Some("HH:mm:ss"));
    e.new_workbook("Book1.xlsx").expect("workbook");
    e.set_input(0, 0, 0, "5/3/2024").expect("A1");
    let value = e
        .workbook()
        .and_then(|w| w.sheets[0].cell(0, 0).map(|c| c.value.clone()));
    assert_eq!(value, Some(CellValue::Number(45356.0)), "5 March 2024");
    assert_eq!(e.cell(0, 0, 0).expect("A1").display, "5/3/2024");
}
