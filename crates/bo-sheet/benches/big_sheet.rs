//! Large-sheet benchmarks (SHEET-PERF-001): a 1 048 576-row list (header + 1 048 575
//! records, 6 columns: region text, product text, year, quantity, price, a unique id
//! text) sorted, filtered and summarised by a pivot table.
//!
//! Targets (release build): pivot over 1 million rows < 3 s; sort / filter 1 million
//! rows < 2 s. `cargo bench -p bo-sheet --bench big_sheet`; with `BO_BENCH_ONCE=1`
//! every case runs once and prints its time (quick before / after comparisons).

use bo_model::sheet::{
    AutoFilter, Cell, CellRange, CellRef, CellValue, ColumnFilter, CustomFilter, FilterColumn,
    PivotDataField, Workbook, Worksheet, MAX_ROWS,
};
use bo_sheet::calc::NoFormulas;
use bo_sheet::numfmt::Locale;
use bo_sheet::pivot::{PivotLabels, PivotSpec};
use bo_sheet::sort::SortKey;
use criterion::{BatchSize, Criterion};
use std::time::{Duration, Instant};

const ROWS: u32 = MAX_ROWS;
const COLS: u32 = 6;

const REGIONS: [&str; 8] = [
    "Hà Nội",
    "Hồ Chí Minh",
    "Đà Nẵng",
    "Hải Phòng",
    "Cần Thơ",
    "Huế",
    "Nha Trang",
    "Vũng Tàu",
];

fn hash(r: u32, c: u32) -> u32 {
    (r.wrapping_mul(2_654_435_761) ^ c.wrapping_mul(40_503)).wrapping_mul(2_246_822_519)
}

fn big_list() -> Workbook {
    let products: Vec<String> = (0..50).map(|i| format!("Sản phẩm {i:02}")).collect();
    let header = ["Vùng", "Sản phẩm", "Năm", "Số lượng", "Đơn giá", "Mã"];
    let cells = (0..ROWS).flat_map(|r| {
        let products = &products;
        (0..COLS).map(move |c| {
            let v = if r == 0 {
                CellValue::Text(header[c as usize].to_owned())
            } else {
                let h = hash(r, c);
                match c {
                    0 => CellValue::Text(REGIONS[(h % 8) as usize].to_owned()),
                    1 => CellValue::Text(products[(h % 50) as usize].clone()),
                    2 => CellValue::Number(f64::from(2015 + h % 10)),
                    3 => CellValue::Number(f64::from(h % 1000)),
                    4 => CellValue::Number(f64::from(h % 100_000) / 100.0),
                    _ => CellValue::Text(format!("ID{:07}", h % 10_000_000)),
                }
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

fn range() -> CellRange {
    CellRange::new(CellRef::new(0, 0), CellRef::new(ROWS - 1, COLS - 1))
}

fn with_filter(mut wb: Workbook) -> Workbook {
    wb.sheets[0].auto_filter = Some(AutoFilter {
        range: range(),
        columns: vec![
            FilterColumn {
                col_id: 0,
                filter: ColumnFilter::Values {
                    values: vec!["Hà Nội".into(), "Huế".into(), "Cần Thơ".into()],
                    blank: false,
                },
                ..FilterColumn::default()
            },
            FilterColumn {
                col_id: 3,
                filter: ColumnFilter::Custom {
                    and: true,
                    conditions: vec![CustomFilter {
                        operator: Some("greaterThan".into()),
                        val: "500".into(),
                    }],
                },
                ..FilterColumn::default()
            },
        ],
        ..AutoFilter::default()
    });
    wb
}

fn pivot_spec() -> PivotSpec {
    PivotSpec {
        rows: vec![0, 1],
        cols: vec![2],
        values: vec![PivotDataField {
            field: 3,
            ..PivotDataField::default()
        }],
        ..PivotSpec::default()
    }
}

fn sort_number(wb: &mut Workbook) {
    bo_sheet::sort::sort_lines(wb, 0, range(), &[SortKey::asc(3)], true, false, &NoFormulas);
}

fn sort_text_number(wb: &mut Workbook) {
    bo_sheet::sort::sort_lines(
        wb,
        0,
        range(),
        &[SortKey::asc(0), SortKey::desc(4)],
        true,
        false,
        &NoFormulas,
    );
}

fn filter(wb: &mut Workbook) {
    let hidden = bo_sheet::filter::apply_auto_filter_at(wb, 0, &Locale::VI_VN, Some(46_000.0));
    assert!(hidden > 0);
}

fn pivot_create(wb: &mut Workbook) {
    bo_sheet::pivot::create_pivot(
        wb,
        0,
        range(),
        1,
        CellRef::new(0, 0),
        &pivot_spec(),
        &PivotLabels::default(),
    )
    .expect("pivot");
}

fn pivot_refresh(wb: &mut Workbook) {
    bo_sheet::pivot::refresh_pivot(wb, 1, 0, &PivotLabels::default()).expect("refresh");
}

fn live_rebuild(wb: &mut Workbook) {
    bo_sheet::live::LiveCalc::new().rebuild(wb);
}

type Case = (&'static str, fn(&mut Workbook));

fn cases() -> [(Case, u8); 6] {
    [
        (("sort_1m_rows_1_number_key", sort_number), 0),
        (("sort_1m_rows_2_keys_text_number", sort_text_number), 0),
        (("autofilter_1m_rows_values_and_custom", filter), 1),
        (
            ("pivot_create_1m_rows_2_row_1_col_fields_sum", pivot_create),
            0,
        ),
        (("pivot_refresh_1m_rows", pivot_refresh), 2),
        // The formula engine loaded with every cell (what an edit of the whole
        // list costs on top of the operation itself in `SheetEngine`).
        (("live_rebuild_1m_rows", live_rebuild), 0),
    ]
}

/// The input of a case: 0 the plain list, 1 with an AutoFilter, 2 with a pivot table.
fn inputs() -> [Workbook; 3] {
    let base = big_list();
    let filtered = with_filter(base.clone());
    let mut pivoted = base.clone();
    pivot_create(&mut pivoted);
    [base, filtered, pivoted]
}

fn bench(c: &mut Criterion) {
    let inputs = inputs();
    let mut g = c.benchmark_group("big_sheet_1m_rows");
    g.sample_size(10)
        .measurement_time(Duration::from_secs(20))
        .warm_up_time(Duration::from_secs(1));
    for ((name, run), input) in cases() {
        let input = &inputs[input as usize];
        g.bench_function(name, |b| {
            b.iter_batched(
                || input.clone(),
                |mut wb| {
                    run(&mut wb);
                    wb
                },
                BatchSize::PerIteration,
            );
        });
    }
    g.finish();
}

fn main() {
    if std::env::var_os("BO_BENCH_ONCE").is_some() {
        let only = std::env::var("BO_BENCH_ONLY").unwrap_or_default();
        println!("size_of::<Cell>() = {} bytes", std::mem::size_of::<Cell>());
        let inputs = inputs();
        for ((name, run), input) in cases() {
            if !name.contains(only.as_str()) {
                continue;
            }
            let mut wb = inputs[input as usize].clone();
            let t = Instant::now();
            run(&mut wb);
            println!("{name}: {:.3} s", t.elapsed().as_secs_f64());
        }
        return;
    }
    let mut c = Criterion::default().configure_from_args();
    bench(&mut c);
    c.final_summary();
}
