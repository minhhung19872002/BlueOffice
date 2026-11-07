//! Editing one cell of a sheet with 100 000 formulas (BlueOffice-PROMPT §5:
//! "sửa 1 ô → tính lại phụ thuộc < 100 ms"). `cargo bench -p bo-engine --bench sheet_edit`.

use bo_engine::sheet::SheetEngine;
use criterion::{criterion_group, criterion_main, Criterion};

fn edit_one_cell(c: &mut Criterion) {
    let mut e = SheetEngine::new();
    e.new_workbook("bench.xlsx").expect("workbook");
    let mut cells = Vec::new();
    for r in 0..20_000u32 {
        cells.push((r, 0, r.to_string()));
        for col in 1..6u32 {
            cells.push((r, col, format!("=A{}*{col}+1", r + 1)));
        }
    }
    e.set_inputs(0, &cells).expect("inputs");
    e.set_input(0, 0, 7, "=SUM(B1:B20000)").expect("sum");
    let mut n = 0u32;
    c.bench_function("sheet_edit_one_cell_100k_formulas", |b| {
        b.iter(|| {
            n += 1;
            e.set_input(0, n % 20_000, 0, &n.to_string()).expect("edit");
        });
    });
}

criterion_group!(benches, edit_one_cell);
criterion_main!(benches);
