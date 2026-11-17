//! Timing probes for recalculation (`cargo test --release -p bo-formula --test recalc_probe
//! -- --ignored --nocapture`): 100 000 simple formulas reading one input cell (the
//! `f100k` book of the BlueSheet speed report) and an edit with 1 000 dependents.

use bo_formula::{Value, Workbook};
use std::time::Instant;

fn f100k() -> Workbook {
    let mut wb = Workbook::with_sheets(&["Sheet1"]);
    wb.set_value(0, 0, 3, 1.0);
    for r in 0..100_000u32 {
        wb.set_value(0, r, 0, f64::from(r + 1));
        wb.set_formula(0, r, 1, &format!("=A{0}*2+$D$1", r + 1))
            .expect("formula");
    }
    wb
}

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1e3
}

#[test]
#[ignore = "timing probe"]
fn probe_100k_simple_formulas() {
    let t = Instant::now();
    let mut wb = f100k();
    eprintln!("build: {:.1} ms", ms(t));
    let t = Instant::now();
    wb.recalculate();
    eprintln!("first recalc: {:.1} ms", ms(t));
    for i in 0..5 {
        let t = Instant::now();
        wb.recalculate_all();
        eprintln!("recalculate_all #{i}: {:.1} ms", ms(t));
    }
    for i in 0..5 {
        let t = Instant::now();
        wb.set_value(0, 0, 3, f64::from(i + 2));
        wb.recalculate();
        eprintln!(
            "edit input (100k dependents) #{i}: {:.1} ms ({} cells)",
            ms(t),
            wb.last_recalculated().len()
        );
    }
    assert_eq!(wb.value(0, 99_999, 1), Value::Number(200_000.0 + 6.0));
    // 1 000 dependents: a second input read by the first 1 000 rows only.
    for r in 0..1000u32 {
        wb.set_formula(0, r, 2, &format!("=A{0}+$E$1", r + 1))
            .expect("formula");
    }
    wb.recalculate();
    for i in 0..5 {
        let t = Instant::now();
        wb.set_value(0, 0, 4, f64::from(i));
        wb.recalculate();
        eprintln!(
            "edit input (1k dependents) #{i}: {:.3} ms ({} cells)",
            ms(t),
            wb.last_recalculated().len()
        );
    }
    for i in 0..5 {
        let t = Instant::now();
        wb.set_value(0, 500, 0, f64::from(i));
        wb.recalculate();
        eprintln!(
            "edit one row input #{i}: {:.3} ms ({} cells)",
            ms(t),
            wb.last_recalculated().len()
        );
    }
}
