//! Large workbooks through the whole engine (SHEET-PERF-001):
//! * `open_10m_cells_xlsx`: open a generated `.xlsx` of 1 048 576 rows × 10 columns
//!   (10 485 760 cells: numbers and shared strings; generated here, not committed);
//! * `engine_sort_1m_rows` / `engine_filter_1m_rows` / `engine_pivot_1m_rows`: the
//!   `sort`, `setFilter` and `pivotCreate` commands on a 1 048 576-row list, with
//!   the undo snapshot and recalculation the engine performs.
//!
//! `cargo bench -p bo-engine --bench sheet_big`; `BO_BENCH_ONCE=1` runs every case
//! once and prints its time (`BO_BENCH_ONLY=<part of a name>` selects cases).

use bo_engine::sheet::SheetEngine;
use bo_model::sheet::{Cell, CellValue, Workbook, Worksheet, MAX_ROWS};
use criterion::{BatchSize, Criterion};
use serde_json::json;
use std::time::{Duration, Instant};

fn hash(r: u32, c: u32) -> u32 {
    (r.wrapping_mul(2_654_435_761) ^ c.wrapping_mul(40_503)).wrapping_mul(2_246_822_519)
}

/// 1 048 576 rows × 10 columns: 7 number columns, 3 text columns (16 / 100 / 1000
/// distinct strings).
fn ten_million() -> Workbook {
    let cells = (0..MAX_ROWS).flat_map(|r| {
        (0..10u32).map(move |c| {
            let h = hash(r, c);
            let v = match c {
                0 => CellValue::Text(format!("Nhóm {}", h % 16)),
                1 => CellValue::Text(format!("Mặt hàng {}", h % 100)),
                2 => CellValue::Text(format!("KH{}", h % 1000)),
                _ => CellValue::Number(f64::from(h % 100_000) / 100.0),
            };
            ((r, c), Cell::new(v))
        })
    });
    let mut wb = Workbook::new();
    wb.sheets = vec![Worksheet {
        name: "Sheet1".into(),
        cells: cells.collect(),
        ..Worksheet::default()
    }];
    wb
}

/// The 1 048 576-row list of `bo-sheet`'s `big_sheet` bench (header + records).
fn list() -> Workbook {
    let regions = [
        "Bắc",
        "Trung",
        "Nam",
        "Tây Nguyên",
        "Đông Nam Bộ",
        "Tây Bắc",
    ];
    let cells = (0..MAX_ROWS).flat_map(move |r| {
        (0..4u32).map(move |c| {
            let h = hash(r, c);
            let v = match (r, c) {
                (0, _) => CellValue::Text(["Vùng", "Năm", "Số lượng", "Mã"][c as usize].into()),
                (_, 0) => CellValue::Text(regions[(h % 6) as usize].into()),
                (_, 1) => CellValue::Number(f64::from(2015 + h % 10)),
                (_, 2) => CellValue::Number(f64::from(h % 1000)),
                _ => CellValue::Text(format!("ID{:07}", h % 10_000_000)),
            };
            ((r, c), Cell::new(v))
        })
    });
    let mut wb = Workbook::new();
    wb.sheets = vec![
        Worksheet {
            name: "Data".into(),
            cells: cells.collect(),
            ..Worksheet::default()
        },
        Worksheet {
            name: "Pivot".into(),
            ..Worksheet::default()
        },
    ];
    wb
}

fn rng(r1: u32, c1: u32) -> serde_json::Value {
    json!({ "r0": 0, "c0": 0, "r1": r1, "c1": c1 })
}

fn run(e: &mut SheetEngine, v: &serde_json::Value) {
    e.exec_json(&v.to_string()).expect("command");
}

fn sort_cmd() -> serde_json::Value {
    json!({ "cmd": "sort", "sheet": 0, "range": rng(MAX_ROWS - 1, 3), "hasHeader": true,
        "keys": [{ "col": 0 }, { "col": 2, "descending": true }] })
}

fn filter_cmd() -> serde_json::Value {
    json!({ "cmd": "setFilter", "sheet": 0, "col": 0, "filter": { "values": ["Bắc", "Nam"] } })
}

fn pivot_cmd() -> serde_json::Value {
    json!({ "cmd": "pivotCreate", "sheet": 0, "range": rng(MAX_ROWS - 1, 3),
        "dest": { "sheet": 1, "row": 0, "col": 0 },
        "rows": [0], "cols": [1], "values": [{ "field": 2, "function": "sum" }] })
}

/// The engine holding the list (with an AutoFilter for the filter case).
fn fresh(base: &Workbook, filter: bool) -> SheetEngine {
    let mut e = SheetEngine::new();
    e.load(base.clone(), "list.xlsx").expect("load");
    if filter {
        run(
            &mut e,
            &json!({ "cmd": "toggleFilter", "sheet": 0, "range": rng(0, 0) }),
        );
    }
    e
}

fn ten_million_xlsx() -> Vec<u8> {
    let t = Instant::now();
    let bytes = bo_xlsx::write_xlsx(&ten_million()).expect("write");
    println!(
        "10M-cell .xlsx: {:.1} MB, written in {:.1} s",
        bytes.len() as f64 / 1e6,
        t.elapsed().as_secs_f64()
    );
    bytes
}

fn bench(c: &mut Criterion) {
    let bytes = ten_million_xlsx();
    let mut g = c.benchmark_group("sheet_big");
    g.sample_size(10)
        .measurement_time(Duration::from_secs(30))
        .warm_up_time(Duration::from_secs(1));
    g.bench_function("open_10m_cells_xlsx", |b| {
        b.iter_batched(
            SheetEngine::new,
            |mut e| {
                e.open(&bytes, "big.xlsx").expect("open");
                e
            },
            BatchSize::PerIteration,
        );
    });
    drop(bytes);
    let base = list();
    for (name, filter, cmd) in [
        ("engine_sort_1m_rows_2_keys", false, sort_cmd()),
        ("engine_filter_1m_rows", true, filter_cmd()),
        ("engine_pivot_1m_rows", false, pivot_cmd()),
    ] {
        g.bench_function(name, |b| {
            b.iter_batched(
                || fresh(&base, filter),
                |mut e| {
                    run(&mut e, &cmd);
                    e
                },
                BatchSize::PerIteration,
            );
        });
    }
    g.finish();
}

/// `BO_BENCH_ONCE=1`: every case once, with its time (`BO_BENCH_ONLY` filters by name).
fn once() {
    let only = std::env::var("BO_BENCH_ONLY").unwrap_or_default();
    if "open_10m_cells_xlsx".contains(only.as_str()) {
        let bytes = ten_million_xlsx();
        let mut e = SheetEngine::new();
        let t = Instant::now();
        e.open(&bytes, "big.xlsx").expect("open");
        println!("open_10m_cells_xlsx: {:.3} s", t.elapsed().as_secs_f64());
        let last = e.cell(0, MAX_ROWS - 1, 9).expect("cell");
        println!("  {} = {}", last.address, last.display);
    }
    let base = list();
    for (name, filter, cmd) in [
        ("engine_sort_1m_rows_2_keys", false, sort_cmd()),
        ("engine_filter_1m_rows", true, filter_cmd()),
        ("engine_pivot_1m_rows", false, pivot_cmd()),
    ] {
        if !name.contains(only.as_str()) {
            continue;
        }
        let mut e = fresh(&base, filter);
        let t = Instant::now();
        run(&mut e, &cmd);
        println!("{name}: {:.3} s", t.elapsed().as_secs_f64());
    }
}

fn main() {
    if std::env::var_os("BO_BENCH_ONCE").is_some() {
        once();
        return;
    }
    let mut c = Criterion::default().configure_from_args();
    bench(&mut c);
    c.final_summary();
}
