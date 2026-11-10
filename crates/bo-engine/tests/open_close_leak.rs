//! CORE-PERF-001: no memory leak when opening and closing 100 documents in one
//! `Engine` (BlueDoc `.docx`, BlueSlide `.pptx`) and one `SheetEngine` (BlueSheet
//! `.xlsx`). The heap bytes in use are counted by an instrumented global allocator
//! (`stats_alloc`, this test binary only). Each corpus file is opened once (warm-up:
//! caches and pools reach their size), then 100 more documents are opened, their first
//! page drawn, and closed (a blank document replaces them, as the editors do). The heap
//! in use after closing must stay on a plateau: never more than a small slack above
//! the warm-up value, and closing must release what the open document held.
//!
//! One `#[test]` only: the allocator counts every thread of the process.

use bo_engine::sheet::{SheetEngine, ViewRequest};
use bo_engine::Engine;
use stats_alloc::{StatsAlloc, INSTRUMENTED_SYSTEM};
use std::alloc::System;
use std::path::PathBuf;

#[global_allocator]
static GLOBAL: &StatsAlloc<System> = &INSTRUMENTED_SYSTEM;

/// Heap bytes in use now (`stats_alloc` already counts reallocations in both totals).
fn in_use() -> i64 {
    let s = GLOBAL.stats();
    i64::try_from(s.bytes_allocated).unwrap_or(i64::MAX)
        - i64::try_from(s.bytes_deallocated).unwrap_or(i64::MAX)
}

fn mb(b: i64) -> f64 {
    b as f64 / (1024.0 * 1024.0)
}

fn corpus(dir: &str, ext: &str) -> Vec<(String, Vec<u8>)> {
    let d = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus")
        .join(dir);
    let mut names: Vec<_> = std::fs::read_dir(&d)
        .expect("corpus")
        .filter_map(Result::ok)
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.to_ascii_lowercase().ends_with(ext))
        .collect();
    names.sort();
    names
        .into_iter()
        .map(|n| {
            let b = std::fs::read(d.join(&n)).expect("read");
            (n, b)
        })
        .collect()
}

const CYCLES: usize = 100;
/// Allowed growth of the heap in use after the warm-up pass.
const SLACK: i64 = 4 << 20;

struct Report {
    kind: &'static str,
    blank: i64,
    warm: i64,
    last: i64,
    peak_closed: i64,
    opened: usize,
}

impl Report {
    fn check(&self) {
        println!(
            "{}: {} opened; heap in use after close: blank {:.1} MB, after warm-up {:.1} MB, \
             highest of {CYCLES} more {:.1} MB, last {:.1} MB",
            self.kind,
            self.opened,
            mb(self.blank),
            mb(self.warm),
            mb(self.peak_closed),
            mb(self.last)
        );
        assert!(self.opened > 0, "{}: nothing opened", self.kind);
        assert!(
            self.peak_closed <= self.warm + SLACK,
            "{}: heap keeps growing ({:.1} MB after warm-up, {:.1} MB later)",
            self.kind,
            mb(self.warm),
            mb(self.peak_closed)
        );
    }
}

fn run_engine(kind: &'static str, files: &[(String, Vec<u8>)]) -> Report {
    let mut e = Engine::with_bundled_fonts();
    e.new_document("blank.docx").expect("blank");
    let blank = in_use();
    let mut opened = 0;
    let mut samples = Vec::new();
    for i in 0..files.len() + CYCLES {
        let (name, bytes) = &files[i % files.len()];
        if let Ok(info) = e.open(bytes, name) {
            opened += 1;
            if info.page_count > 0 {
                let _ = std::hint::black_box(e.page_json(0));
            }
        }
        e.new_document("blank.docx").expect("blank");
        samples.push(in_use());
        if std::env::var_os("BO_LEAK_TRACE").is_some() {
            let d = samples[samples.len() - 1]
                - samples
                    .get(samples.len().wrapping_sub(2))
                    .copied()
                    .unwrap_or(blank);
            if d.abs() > 256 << 10 {
                println!("TRACE {i} {name} {:+.2} MB", mb(d));
            }
        }
    }
    Report {
        kind,
        blank,
        warm: samples[files.len() - 1],
        last: samples[samples.len() - 1],
        peak_closed: samples[files.len()..].iter().copied().max().unwrap_or(0),
        opened,
    }
}

fn run_sheet(files: &[(String, Vec<u8>)]) -> Report {
    let mut e = SheetEngine::with_bundled_fonts();
    e.new_workbook("blank.xlsx").expect("blank");
    let blank = in_use();
    let mut opened = 0;
    let mut samples = Vec::new();
    for i in 0..files.len() + CYCLES {
        let (name, bytes) = &files[i % files.len()];
        if e.open(bytes, name).is_ok() {
            opened += 1;
            let req = ViewRequest {
                sheet: 0,
                scroll_x: 0.0,
                scroll_y: 0.0,
                width: 800.0,
                height: 600.0,
            };
            let _ = std::hint::black_box(e.render_json(&req));
        }
        e.new_workbook("blank.xlsx").expect("blank");
        samples.push(in_use());
    }
    Report {
        kind: "xlsx",
        blank,
        warm: samples[files.len() - 1],
        last: samples[samples.len() - 1],
        peak_closed: samples[files.len()..].iter().copied().max().unwrap_or(0),
        opened,
    }
}

#[test]
fn opening_and_closing_100_documents_does_not_leak() {
    let docx = run_engine("docx", &corpus("doc", ".docx"));
    let pptx = run_engine("pptx", &corpus("slide", ".pptx"));
    let xlsx = run_sheet(&corpus("sheet", ".xlsx"));
    for r in [&docx, &pptx, &xlsx] {
        r.check();
    }
}
