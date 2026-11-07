//! Recalculation benchmark for the performance targets of the BlueSheet MVP:
//! a workbook with 1 000 000 cells of which 100 000 are formulas must load and
//! calculate in < 5 s, and editing one cell must recalculate its dependents in
//! < 100 ms.
//!
//! Layout (10 000 rows × 100 columns):
//! * columns A..CL (90 columns): numeric constants (900 000 cells);
//! * columns CM..CV (10 columns): a mix of formulas (100 000 cells): arithmetic,
//!   SUM/AVERAGE/MAX over 10-cell rows, IF, COUNTIF, an exact VLOOKUP into a
//!   1 000-row table and a running total chained down the whole sheet.
//!
//! Benchmarks:
//! * `load_and_calc_1m_cells_100k_formulas`: build the workbook and calculate everything;
//! * `edit_one_cell_row_dependents`: change one input read by a few formulas of its row;
//! * `edit_lookup_table_cell`: change a cell inside the VLOOKUP table (10 000 dependents);
//! * `edit_chain_head`: change the start of the running total (10 000 chained dependents).

use bo_formula::{Value, Workbook};
use criterion::{criterion_group, criterion_main, Criterion};
use std::time::Duration;

const ROWS: u32 = 10_000;
const CONST_COLS: u32 = 90;
const FORMULA_COLS: u32 = 10;

fn value_at(r: u32, c: u32) -> f64 {
    // Deterministic pseudo-random constants in [0, 1000).
    let h = (r.wrapping_mul(2_654_435_761) ^ c.wrapping_mul(40_503)).wrapping_mul(2_246_822_519);
    f64::from(h % 1000)
}

fn formula(r: u32, k: u32) -> String {
    let n = r + 1;
    match k {
        0 => format!("=A{n}*2+B{n}"),
        1 => format!("=SUM(A{n}:J{n})"),
        2 => format!("=CM{n}+CN{n}"),
        3 => format!("=IF(C{n}>500,D{n},E{n})"),
        4 => format!("=ROUND(AVERAGE(F{n}:O{n}),2)"),
        5 => {
            if n == 1 {
                "=CO1".to_owned()
            } else {
                format!("=CR{}+CO{n}", n - 1)
            }
        }
        6 => format!("=VLOOKUP({},$A$1:$B$1000,2,FALSE)", value_at(r, 0)),
        7 => format!("=MAX(P{n}:Y{n})"),
        8 => format!("=COUNTIF(Z{n}:AI{n},\">500\")"),
        _ => format!("=CP{n}*1.1+CS{n}"),
    }
}

fn build() -> Workbook {
    let mut wb = Workbook::with_sheets(&["Sheet1"]);
    for r in 0..ROWS {
        for c in 0..CONST_COLS {
            wb.set_value(0, r, c, value_at(r, c));
        }
        for k in 0..FORMULA_COLS {
            wb.set_formula(0, r, CONST_COLS + k, &formula(r, k))
                .expect("valid formula");
        }
    }
    wb
}

fn load_and_calc() -> Workbook {
    let mut wb = build();
    wb.recalculate();
    wb
}

fn bench(c: &mut Criterion) {
    // Sanity: the sheet really calculates.
    let wb = load_and_calc();
    assert_eq!(
        wb.value(0, 0, CONST_COLS),
        Value::Number(value_at(0, 0) * 2.0 + value_at(0, 1))
    );
    assert!(matches!(
        wb.value(0, ROWS - 1, CONST_COLS + 5),
        Value::Number(_)
    ));
    drop(wb);

    let mut g = c.benchmark_group("recalc");
    g.sample_size(10);
    g.measurement_time(Duration::from_secs(30));
    g.bench_function("load_and_calc_1m_cells_100k_formulas", |b| {
        b.iter(load_and_calc);
    });
    g.finish();

    let mut g = c.benchmark_group("edit");
    g.sample_size(20);
    let mut wb = load_and_calc();
    let mut v = 0.0;
    g.bench_function("edit_one_cell_row_dependents", |b| {
        b.iter(|| {
            v += 1.0;
            wb.set_value(0, 5000, 0, v);
            wb.recalculate();
        });
    });
    g.bench_function("edit_lookup_table_cell", |b| {
        b.iter(|| {
            v += 1.0;
            wb.set_value(0, 500, 1, v);
            wb.recalculate();
        });
    });
    g.bench_function("edit_chain_head", |b| {
        b.iter(|| {
            v += 1.0;
            // A1 feeds CM1 -> CO1 -> CR1 -> CR2 -> ... -> CR10000.
            wb.set_value(0, 0, 0, v);
            wb.recalculate();
        });
    });
    g.finish();
}

criterion_group!(benches, bench);
criterion_main!(benches);
