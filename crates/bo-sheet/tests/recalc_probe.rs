//! Timing probe for a cell edit through [`bo_sheet::live::LiveCalc`] (`cargo test --release
//! -p bo-sheet --test recalc_probe -- --ignored --nocapture`): the `f100k` book of the
//! BlueSheet speed report (100 000 × `=An*2+$D$1`), D1 edited.

use bo_model::sheet::{Cell, CellValue, Workbook};
use bo_sheet::live::LiveCalc;
use std::time::Instant;

#[test]
#[ignore = "timing probe"]
fn probe_edit_input_of_100k_formulas() {
    let mut wb = Workbook::new();
    let ws = &mut wb.sheets[0];
    ws.set_cell(0, 3, Cell::new(CellValue::Number(1.0)));
    for r in 0..100_000u32 {
        ws.set_cell(r, 0, Cell::new(CellValue::Number(f64::from(r + 1))));
        ws.set_cell(
            r,
            1,
            Cell::default().with_formula(&format!("A{0}*2+$D$1", r + 1)),
        );
    }
    let mut live = LiveCalc::new();
    let t = Instant::now();
    live.rebuild(&mut wb);
    eprintln!("rebuild: {:.1} ms", t.elapsed().as_secs_f64() * 1e3);
    for i in 0..5 {
        wb.sheets[0].set_cell(0, 3, Cell::new(CellValue::Number(f64::from(i + 2))));
        let t = Instant::now();
        live.update(&mut wb, 0, &[(0, 3)]);
        eprintln!("update D1 #{i}: {:.1} ms", t.elapsed().as_secs_f64() * 1e3);
    }
    assert_eq!(
        wb.sheets[0].cell(99_999, 1).map(|c| c.value.clone()),
        Some(CellValue::Number(200_000.0 + 6.0))
    );
}
