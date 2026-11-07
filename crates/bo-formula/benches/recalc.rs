//! Recalculation benchmark: 100 000 formula cells.
//!
//! * `chain`: A1 = 1, A(i) = A(i-1)+1 — editing A1 dirties every formula.
//! * `grid`: 1 000 rows × 100 formula columns, each cell = left neighbour * 1.0001 + 1,
//!   plus row totals; editing one input dirties one row.

use bo_formula::{Value, Workbook};
use criterion::{criterion_group, criterion_main, BatchSize, Criterion};

const N: u32 = 100_000;

fn chain() -> Workbook {
    let mut wb = Workbook::with_sheets(&["Sheet1"]);
    wb.set_value(0, 0, 0, 1.0);
    for r in 1..N {
        wb.set_formula(0, r, 0, &format!("=A{r}+1"))
            .expect("valid formula");
    }
    wb
}

fn grid() -> Workbook {
    let mut wb = Workbook::with_sheets(&["Sheet1"]);
    for r in 0..1_000u32 {
        wb.set_value(0, r, 0, f64::from(r));
        for c in 1..=99u32 {
            let left = bo_formula::reference::col_to_letters(c - 1);
            wb.set_formula(0, r, c, &format!("={left}{}*1.0001+1", r + 1))
                .expect("valid formula");
        }
        wb.set_formula(0, r, 100, &format!("=SUM(B{0}:CV{0})", r + 1))
            .expect("valid formula");
    }
    wb
}

fn bench(c: &mut Criterion) {
    let mut g = c.benchmark_group("recalc_100k");
    g.sample_size(10);

    g.bench_function("chain_full", |b| {
        b.iter_batched(chain, |mut wb| wb.recalculate_all(), BatchSize::LargeInput);
    });
    g.bench_function("chain_edit_head", |b| {
        let mut wb = chain();
        wb.recalculate();
        let mut v = 1.0;
        b.iter(|| {
            v += 1.0;
            wb.set_value(0, 0, 0, v);
            wb.recalculate();
        });
        assert_eq!(wb.value(0, N - 1, 0), Value::Number(v + f64::from(N - 1)));
    });
    g.bench_function("grid_full", |b| {
        b.iter_batched(grid, |mut wb| wb.recalculate_all(), BatchSize::LargeInput);
    });
    g.bench_function("grid_edit_one", |b| {
        let mut wb = grid();
        wb.recalculate();
        let mut v = 0.0;
        b.iter(|| {
            v += 1.0;
            wb.set_value(0, 500, 0, v);
            wb.recalculate();
        });
    });
    g.finish();
}

criterion_group!(benches, bench);
criterion_main!(benches);
