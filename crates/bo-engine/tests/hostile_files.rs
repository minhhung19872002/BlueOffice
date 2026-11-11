//! Opening hostile files (BlueOffice-PARITY-PROMPT §9.4): zip bombs, deep XML,
//! entity expansion, XXE, external relationships, unsafe hyperlinks, OLE objects with
//! executables, macros, DDE and INCLUDE* fields, remote templates, SVG / HTML / MHT with
//! scripts, PDF actions, malformed images, oversized strings, truncated packages.
//!
//! The samples come from `bo_hostile::cases` (generated, nothing downloaded; also
//! written by `bo-cli sample-hostile <dir>`). For each one:
//!
//! * opening returns `Ok` or a clean `Err` (as the case requires), without panicking,
//!   within [`BUDGET`] and under [`MEMORY_CAP`] of heap in use (sampled) — on a 2 MiB
//!   stack like the WASM build;
//! * nothing is fetched or read: every `http://` target points at a local TCP listener
//!   that must never see a connection, every `file://` target at a sentinel file whose
//!   content must never appear in the document, its rendering or a saved copy, and
//!   every script carries [`bo_hostile::SCRIPT_MARKER`], which must not reach the text;
//! * nothing runs: macros are reported and kept, fields are updated without touching
//!   their external sources, DDE formulas are not evaluated against anything;
//! * saving rewrites the file and keeps the preserved parts (VBA projects, OLE objects,
//!   SVG pictures) and the external targets as data.
//!
//! A second test checks statically that the crates behind the engines contain no
//! network or process API, and use the file system only where the host chooses the path.
//!
//! One `#[test]` runs the samples: the allocator counts every thread of the process.

use bo_engine::sheet::ViewRequest;
use bo_engine::{Engine, SheetEngine};
use bo_hostile::{App, Case, Config, Expect, SCRIPT_MARKER};
use bo_opc::{Limits, Package};
use stats_alloc::{StatsAlloc, INSTRUMENTED_SYSTEM};
use std::alloc::System;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[global_allocator]
static GLOBAL: &StatsAlloc<System> = &INSTRUMENTED_SYSTEM;

/// The tests of this file run one at a time: the allocator counts every thread, so the
/// other test's allocations would show in the measured heap.
static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Per-file budget for open + render + save + reopen in an unoptimised build on a
/// loaded machine; a hang or an exponential expansion shows as minutes.
const BUDGET: Duration = Duration::from_secs(60);
/// Heap in use above the level before the case (sampled every millisecond).
const MEMORY_CAP: i64 = 1 << 30;
/// Heap in use while opening (parsing, laying out every page) a document made of one
/// over-long word, per character of the word.
const LONG_WORD_BYTES_PER_CHAR: f64 = 50.0;

fn in_use() -> i64 {
    let s = GLOBAL.stats();
    i64::try_from(s.bytes_allocated).unwrap_or(i64::MAX)
        - i64::try_from(s.bytes_deallocated).unwrap_or(i64::MAX)
}

fn mb(b: i64) -> f64 {
    b as f64 / (1024.0 * 1024.0)
}

/// Everything a save must contain, as one string (XML parts of the package).
fn package_text(bytes: &[u8]) -> String {
    let Ok(pkg) = Package::read(bytes, &Limits::default()) else {
        return String::from_utf8_lossy(bytes).into_owned();
    };
    let mut s = String::new();
    for p in pkg.parts() {
        s.push_str(&String::from_utf8_lossy(&p.data));
        s.push('\n');
    }
    s
}

struct Outcome {
    opened: bool,
    error: String,
    problems: Vec<String>,
}

/// Content that must never show up anywhere.
struct Forbidden {
    sentinel: String,
}

impl Forbidden {
    /// `text` is something the user sees (document text, rendering): no sentinel, no
    /// script body.
    fn shown(&self, what: &str, text: &str, problems: &mut Vec<String>) {
        if text.contains(&self.sentinel) {
            problems.push(format!("{what}: the content of a local file was read"));
        }
        if text.contains(SCRIPT_MARKER) {
            problems.push(format!("{what}: script text reached the document"));
        }
    }
    /// `text` is a saved file: no sentinel (scripts inside kept parts are data).
    fn saved(&self, what: &str, text: &str, problems: &mut Vec<String>) {
        if text.contains(&self.sentinel) {
            problems.push(format!("{what}: the content of a local file was saved"));
        }
    }
}

fn check_saved(case: &Case, saved: &[u8], forbidden: &Forbidden, problems: &mut Vec<String>) {
    let text = package_text(saved);
    forbidden.saved("save", &text, problems);
    if let Ok(pkg) = Package::read(saved, &Limits::default()) {
        let orig = Package::read(&case.bytes, &Limits::default()).ok();
        for name in &case.keep_parts {
            let before = orig.as_ref().and_then(|o| o.part(name)).map(|p| &p.data);
            let after = pkg.part(name).map(|p| &p.data);
            if after.is_none() || after != before {
                problems.push(format!("save dropped or changed {name}"));
            }
        }
    } else if !case.keep_parts.is_empty() {
        problems.push("save is not a package".into());
    }
    for t in &case.keep_text {
        if !text.contains(t.as_str()) {
            problems.push(format!("save dropped {t:?}"));
        }
    }
    if case.macros_saved != bo_binary::has_macros(saved) {
        problems.push(format!("saved file has macros: {}", !case.macros_saved));
    }
}

fn run_doc(case: &Case, forbidden: &Forbidden) -> Outcome {
    let mut problems = Vec::new();
    let mut e = Engine::with_bundled_fonts();
    let info = match e.open(&case.bytes, &case.name) {
        Ok(i) => i,
        Err(err) => {
            return Outcome {
                opened: false,
                error: err.to_string(),
                problems,
            }
        }
    };
    if info.macros != case.macros {
        problems.push(format!("macros reported {}", info.macros));
    }
    let deck = info.kind == "slide";
    if (case.app == App::Slide) != deck {
        problems.push(format!("opened as {}", info.kind));
    }
    if info.page_count > 0 {
        match e.page_json(0) {
            Ok(page) => forbidden.shown("page 0", &page, &mut problems),
            Err(err) => problems.push(format!("page 0: {err}")),
        }
    }
    if deck {
        if let Ok(s) = e.slide_json(0) {
            forbidden.shown("slide 0", &s, &mut problems);
        }
    } else {
        if let Some(d) = e.document() {
            forbidden.shown("text", &d.plain_text(), &mut problems);
        }
        // Updating every field must not read INCLUDETEXT / INCLUDEPICTURE / LINK
        // sources nor start DDE conversations.
        let _ = e.command_json(r#"{"cmd":"updateFields"}"#);
        if let Some(d) = e.document() {
            forbidden.shown("text after updateFields", &d.plain_text(), &mut problems);
        }
    }
    let saved = if deck { e.save_pptx() } else { e.save_docx() };
    match saved {
        Ok(saved) => {
            check_saved(case, &saved, forbidden, &mut problems);
            let mut again = Engine::with_bundled_fonts();
            let ext = if deck { "pptx" } else { "docx" };
            if let Err(err) = again.open(&saved, &format!("saved.{ext}")) {
                problems.push(format!("saved copy does not open: {err}"));
            }
        }
        Err(err) => problems.push(format!("save: {err}")),
    }
    Outcome {
        opened: true,
        error: String::new(),
        problems,
    }
}

fn run_sheet(case: &Case, forbidden: &Forbidden) -> Outcome {
    let mut problems = Vec::new();
    let mut e = SheetEngine::with_bundled_fonts();
    let info = match e.open(&case.bytes, &case.name) {
        Ok(i) => i,
        Err(err) => {
            return Outcome {
                opened: false,
                error: err.to_string(),
                problems,
            }
        }
    };
    if info.macros != case.macros {
        problems.push(format!("macros reported {}", info.macros));
    }
    // Recalculate everything: DDE / WEBSERVICE / external references must evaluate
    // to errors or cached values, never by contacting anything.
    let _ = e.exec_json(r#"{"cmd":"calculate"}"#);
    let req = ViewRequest {
        sheet: 0,
        scroll_x: 0.0,
        scroll_y: 0.0,
        width: 800.0,
        height: 600.0,
    };
    match e.render_json(&req) {
        Ok(view) => forbidden.shown("view", &view, &mut problems),
        Err(err) => problems.push(format!("view: {err}")),
    }
    for r in 0..4 {
        for c in 0..4 {
            if let Ok(cell) = e.cell(0, r, c) {
                forbidden.shown("cell", &format!("{cell:?}"), &mut problems);
            }
        }
    }
    match e.save("xlsx", 0) {
        Ok(saved) => {
            check_saved(case, &saved, forbidden, &mut problems);
            let mut again = SheetEngine::new();
            if let Err(err) = again.open(&saved, "saved.xlsx") {
                problems.push(format!("saved copy does not open: {err}"));
            }
        }
        Err(err) => problems.push(format!("save: {err}")),
    }
    Outcome {
        opened: true,
        error: String::new(),
        problems,
    }
}

/// Run `f` on a 2 MiB stack while sampling the heap; returns its result, the time and
/// the highest heap in use above the starting level.
fn measured<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> (T, Duration, i64) {
    let base = in_use();
    let peak = Arc::new(AtomicI64::new(0));
    let done = Arc::new(AtomicBool::new(false));
    let sampler = {
        let (peak, done) = (peak.clone(), done.clone());
        std::thread::spawn(move || {
            while !done.load(Ordering::Relaxed) {
                peak.fetch_max(in_use() - base, Ordering::Relaxed);
                std::thread::sleep(Duration::from_millis(1));
            }
        })
    };
    let t = Instant::now();
    let out = std::thread::Builder::new()
        .stack_size(2 << 20)
        .spawn(f)
        .expect("spawn")
        .join();
    let dt = t.elapsed();
    done.store(true, Ordering::Relaxed);
    let _ = sampler.join();
    peak.fetch_max(in_use() - base, Ordering::Relaxed);
    let out = out.unwrap_or_else(|p| std::panic::resume_unwind(p));
    (out, dt, peak.load(Ordering::Relaxed))
}

fn file_url(p: &Path) -> String {
    let s = p.to_string_lossy().replace('\\', "/");
    if s.starts_with('/') {
        format!("file://{s}")
    } else {
        format!("file:///{s}")
    }
}

#[test]
fn hostile_files_open_safely() {
    let _serial = SERIAL
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    // The watchers: a sentinel file and a listener for every http:// target.
    let dir = std::env::temp_dir().join(format!("bo-hostile-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let sentinel = format!("BO-SENTINEL-{}-do-not-read", std::process::id());
    let sentinel_path = dir.join("sentinel.txt");
    std::fs::write(&sentinel_path, &sentinel).expect("sentinel");
    // `….png` variants of the file URL (linked pictures) hold the sentinel too.
    std::fs::write(dir.join("sentinel.txt.png"), &sentinel).expect("sentinel");
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
    let port = listener.local_addr().expect("addr").port();
    listener.set_nonblocking(true).expect("nonblocking");
    let hits = Arc::new(AtomicUsize::new(0));
    let stop = Arc::new(AtomicBool::new(false));
    let watcher = {
        let (hits, stop) = (hits.clone(), stop.clone());
        std::thread::spawn(move || {
            while !stop.load(Ordering::Relaxed) {
                if listener.accept().is_ok() {
                    hits.fetch_add(1, Ordering::Relaxed);
                }
                std::thread::sleep(Duration::from_millis(5));
            }
        })
    };
    let cfg = Config {
        http: format!("http://127.0.0.1:{port}/hostile"),
        file_url: file_url(&sentinel_path),
        unc: format!(r"\\127.0.0.1@{port}\hostile"),
    };
    let only = std::env::var("BO_HOSTILE_ONLY").unwrap_or_default();
    let cases: Vec<Case> = bo_hostile::cases(&cfg)
        .expect("samples")
        .into_iter()
        .filter(|c| only.is_empty() || c.name.contains(only.as_str()))
        .collect();
    let forbidden = Arc::new(Forbidden { sentinel });

    let mut failures = Vec::new();
    println!(
        "{:<32} {:>9} {:>8} {:>9}  result",
        "file", "size", "time", "peak MB"
    );
    for case in cases {
        let name = case.name.clone();
        let expect = case.expect;
        let size = case.bytes.len();
        let fb = forbidden.clone();
        let (out, dt, peak) = measured(move || match case.app {
            App::Sheet => run_sheet(&case, &fb),
            App::Doc | App::Slide => run_doc(&case, &fb),
        });
        let result = if out.opened {
            "opened".to_owned()
        } else {
            format!("rejected: {}", out.error)
        };
        println!(
            "{name:<32} {size:>9} {:>7.1}s {:>9.1}  {result}",
            dt.as_secs_f64(),
            mb(peak)
        );
        match (expect, out.opened) {
            (Expect::Reject, true) => failures.push(format!("{name}: opened, must be rejected")),
            (Expect::Open, false) => failures.push(format!("{name}: {result}")),
            _ => {}
        }
        if !out.opened && out.error.trim().is_empty() {
            failures.push(format!("{name}: rejected without a message"));
        }
        if dt > BUDGET {
            failures.push(format!("{name}: took {dt:?}"));
        }
        if peak > MEMORY_CAP {
            failures.push(format!("{name}: {:.0} MB of heap", mb(peak)));
        }
        failures.extend(out.problems.into_iter().map(|p| format!("{name}: {p}")));
    }
    // Over-long words: laying out a run without break opportunities stays linear and
    // small in memory (chunks with emergency break opportunities).
    for (label, chars) in [("1MiB", 1usize << 20), ("8MiB", 8 << 20)] {
        let name = format!("long-word-{label}.docx");
        if !(only.is_empty() || name.contains(only.as_str())) {
            continue;
        }
        let bytes = bo_hostile::long_word_docx(chars).expect("sample");
        // The fonts are loaded before measuring: only the document counts.
        let mut e = Engine::with_bundled_fonts();
        let (out, dt, peak) = measured(move || {
            let info = e
                .open(&bytes, "long-word.docx")
                .map_err(|e| e.to_string())?;
            let last = info.page_count.saturating_sub(1);
            let text: usize = [0, last]
                .iter()
                .map(|&p| e.page_json(p).map_or(0, |j| j.len()))
                .sum();
            let chars = e.document().map_or(0, |d| d.plain_text().trim_end().len());
            Ok::<_, String>((info.page_count, text, chars))
        });
        let per_char = peak as f64 / chars as f64;
        println!(
            "{name:<32} {chars:>9} {:>7.1}s {:>9.1}  {per_char:.1} bytes per character",
            dt.as_secs_f64(),
            mb(peak)
        );
        match out {
            Ok((pages, json, kept)) => {
                if pages < 2 || json == 0 {
                    failures.push(format!("{name}: {pages} pages laid out"));
                }
                if kept != chars {
                    failures.push(format!("{name}: {kept} of {chars} characters kept"));
                }
            }
            Err(err) => failures.push(format!("{name}: {err}")),
        }
        if per_char >= LONG_WORD_BYTES_PER_CHAR {
            failures.push(format!("{name}: {per_char:.1} bytes per character"));
        }
        if dt > BUDGET {
            failures.push(format!("{name}: took {dt:?}"));
        }
    }
    // Give a late connection the chance to show.
    std::thread::sleep(Duration::from_millis(50));
    stop.store(true, Ordering::Relaxed);
    let _ = watcher.join();
    let _ = std::fs::remove_dir_all(&dir);
    let n = hits.load(Ordering::Relaxed);
    if n > 0 {
        failures.push(format!("{n} connection(s) to the http:// targets"));
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Path dependencies of a crate (`path = "../x"` and `x.workspace = true` for `bo-*`).
fn path_deps(root: &Path, krate: &str) -> Vec<String> {
    let toml = std::fs::read_to_string(root.join(krate).join("Cargo.toml")).unwrap_or_default();
    let mut deps = Vec::new();
    let mut in_deps = false;
    for line in toml.lines() {
        let l = line.trim();
        if l.starts_with('[') {
            in_deps = l == "[dependencies]";
            continue;
        }
        if !in_deps {
            continue;
        }
        let Some(name) = l.split(['=', ' ', '.']).next() else {
            continue;
        };
        if name.starts_with("bo-") {
            deps.push(name.to_owned());
        }
    }
    deps
}

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            rust_files(&p, out);
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
}

/// `code` contains `t` not preceded by an identifier character (`File::open` but not
/// `CompoundFile::open`, which works on an in-memory cursor).
fn has_token(code: &str, t: &str) -> bool {
    code.match_indices(t).any(|(i, _)| {
        code[..i]
            .chars()
            .next_back()
            .is_none_or(|c| !(c.is_alphanumeric() || c == '_'))
    })
}

/// Production code of a source file: test modules (`#[cfg(test)]` to the end, the
/// convention of this repository) and test-only files removed.
fn production_code(path: &Path, src: &str) -> String {
    let file = path
        .file_name()
        .map_or(String::new(), |f| f.to_string_lossy().into_owned());
    if file == "tests.rs" || file.ends_with("_tests.rs") || file == "corpus_gen.rs" {
        return String::new();
    }
    match src.find("#[cfg(test)]") {
        Some(i) => src[..i].to_owned(),
        None => src.to_owned(),
    }
}

#[test]
fn engine_crates_cannot_reach_the_network_or_run_programs() {
    let _serial = SERIAL
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut todo = vec!["bo-engine".to_owned()];
    let mut seen = Vec::new();
    while let Some(k) = todo.pop() {
        if seen.contains(&k) {
            continue;
        }
        todo.extend(path_deps(&root, &k));
        seen.push(k);
    }
    seen.sort();
    assert!(seen.len() > 10, "dependency closure looks wrong: {seen:?}");
    // Network, processes, foreign code: never.
    let never = [
        "std::net",
        "TcpStream",
        "UdpSocket",
        "std::process::Command",
        "process::Command",
        "reqwest",
        "ureq::",
        "hyper::",
        "libloading",
    ];
    // File system: only where the host passes the path (fonts of the system, map
    // datasets of the installation) — never a path taken from a document.
    let fs_allowed = [
        "bo-text/src/manager.rs",
        "bo-text/src/system.rs",
        "bo-sheet/src/geo/dataset.rs",
    ];
    let fs = ["std::fs", "File::open", "File::create", "read_dir("];
    let mut bad = Vec::new();
    for k in &seen {
        let mut files = Vec::new();
        rust_files(&root.join(k).join("src"), &mut files);
        for f in files {
            let src = std::fs::read_to_string(&f).unwrap_or_default();
            let code = production_code(&f, &src);
            let rel = f
                .strip_prefix(&root)
                .map_or_else(|_| f.display().to_string(), |r| r.display().to_string())
                .replace('\\', "/");
            for t in never {
                if has_token(&code, t) {
                    bad.push(format!("{rel}: {t}"));
                }
            }
            if !fs_allowed.contains(&rel.as_str()) {
                for t in fs {
                    if has_token(&code, t) {
                        bad.push(format!("{rel}: {t}"));
                    }
                }
            }
        }
    }
    // Cargo manifests: no HTTP client crate in the engine's closure.
    for k in &seen {
        let toml = std::fs::read_to_string(root.join(k).join("Cargo.toml")).unwrap_or_default();
        let deps = toml.split("[dev-dependencies]").next().unwrap_or_default();
        for c in ["reqwest", "ureq", "hyper", "curl", "attohttpc", "isahc"] {
            if deps.lines().any(|l| l.trim_start().starts_with(c)) {
                bad.push(format!("{k}/Cargo.toml: {c}"));
            }
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}
