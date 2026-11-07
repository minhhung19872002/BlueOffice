//! Typing in a 300-page BlueDoc document (BlueOffice-PROMPT §5: "gõ phím → hiển thị
//! < 16 ms ở tài liệu 300 trang"). Each sample is one edit in the middle of the
//! document followed by fetching the display list (JSON) of the page showing the
//! caret; the inverse edit that keeps the document stable is not timed.
//! `cargo bench -p bo-engine --bench doc_typing`.

#[path = "../tests/support/big_doc.rs"]
mod big_doc;

use criterion::{criterion_group, criterion_main, Criterion};
use std::time::{Duration, Instant};

fn typing(c: &mut Criterion) {
    let mut e = big_doc::engine_with_big_doc();
    println!(
        "benchmark document: {} pages",
        e.info().expect("info").page_count
    );
    let mut g = c.benchmark_group("doc_typing_300_pages");
    g.sample_size(20);
    for (name, cmd, restore) in big_doc::edits() {
        big_doc::prepare(&mut e, name);
        g.bench_function(name, |b| {
            b.iter_custom(|iters| {
                let mut total = Duration::ZERO;
                for _ in 0..iters {
                    let t = Instant::now();
                    std::hint::black_box(big_doc::edit_and_fetch_visible(&mut e, cmd.clone()));
                    total += t.elapsed();
                    big_doc::edit_and_fetch_visible(&mut e, restore.clone());
                }
                total
            });
        });
    }
    g.finish();
}

criterion_group!(benches, typing);
criterion_main!(benches);
