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

#[test]
fn stored_column_widths_use_the_screen_digit_width() {
    use bo_model::sheet::{ColInfo, Workbook, Worksheet};
    let mut wb = Workbook::new();
    wb.sheets.clear();
    let mut ws = Worksheet::new("Sheet1");
    ws.cols.push(ColInfo {
        min: 0,
        max: 0,
        width: Some(16.0),
        custom_width: true,
        ..ColInfo::default()
    });
    wb.sheets.push(ws);
    let mut e = engine();
    e.load(wb, "Book1.xlsx").expect("load");
    let at = |e: &mut SheetEngine, ppp: f64| {
        e.set_display_scale(Some(ppp));
        let vp = bo_sheet::viewport::Viewport::new(800.0, 600.0);
        let (_, g) = e.view_layout(0, &vp).expect("layout");
        (g.col_width(0) * ppp).round()
    };
    // 125 %: 9 px digits → 144 px (the reference), not 112 px × 1.25.
    assert_eq!(at(&mut e, 1.25 * 4.0 / 3.0), 144.0);
    // 100 %: 7 px digits → 112 px as before.
    assert_eq!(at(&mut e, 4.0 / 3.0), 112.0);
}
