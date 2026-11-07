//! Typing latency in a 300-page document (BlueOffice-PROMPT §5 target: < 16 ms).
//! Ignored by default (slow in debug builds); run in release:
//! `cargo test --release -p bo-engine --test typing_perf -- --ignored --nocapture`.

mod support;

use std::time::{Duration, Instant};
use support::big_doc;

#[test]
#[ignore = "timing test: run in release with --ignored"]
fn typing_in_300_page_document_under_16_ms() {
    let mut e = big_doc::engine_with_big_doc();
    let pages = e.info().expect("info").page_count;
    assert!(pages >= 290, "benchmark document has {pages} pages");
    let mut report = Vec::new();
    for (name, cmd, restore) in big_doc::edits() {
        big_doc::prepare(&mut e, name);
        let mut times: Vec<Duration> = (0..15)
            .map(|_| {
                let t = Instant::now();
                big_doc::edit_and_fetch_visible(&mut e, cmd.clone());
                let d = t.elapsed();
                big_doc::edit_and_fetch_visible(&mut e, restore.clone());
                d
            })
            .collect();
        times.sort();
        let median = times[times.len() / 2];
        big_doc::edit_and_fetch_visible(&mut e, cmd.clone());
        let kind = e.last_layout_update();
        big_doc::edit_and_fetch_visible(&mut e, restore.clone());
        println!(
            "{name}: median {median:?} (min {:?}, max {:?}), layout {kind:?}",
            times[0],
            times[times.len() - 1]
        );
        report.push((name, median));
    }
    // Debug builds are an order of magnitude slower; only release must meet the target.
    let limit = if cfg!(debug_assertions) {
        Duration::from_millis(400)
    } else {
        Duration::from_millis(16)
    };
    for (name, median) in report {
        assert!(median < limit, "{name}: {median:?} >= {limit:?}");
    }
}
