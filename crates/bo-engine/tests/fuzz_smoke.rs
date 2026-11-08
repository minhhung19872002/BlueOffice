//! In-tree fuzz smoke test (BlueOffice-PROMPT §7 "Fuzzing … không được panic, không
//! được treo").
//!
//! `cargo fuzz` (libFuzzer) needs a nightly toolchain with sanitizers and does not run
//! on the Windows MSVC dev machines nor in the stable CI jobs, so this test replays the
//! bodies of the OOXML targets in `fuzz/fuzz_targets/{opc,docx,xlsx,pptx}.rs` over a
//! deterministic set of mutations of the self-generated corpus (`tests/corpus`):
//!
//! * XML-level mutations inside the package (byte flips, cuts, duplicated ranges,
//!   injected markup, deep nesting) — the ZIP stays valid so the XML readers and the
//!   layout/sheet/slide engines behind them actually see the damaged markup;
//! * byte-level mutations of the ZIP container itself (OPC reader).
//!
//! Every call must return (`Ok` or `Err`) without panicking, and within a time budget
//! (no hang). The legacy binary parsers (CFB, BIFF8/BIFF12, `.doc`, `.ppt`, VBA),
//! `[MS-OFFCRYPTO]`, ODF, TCVN3/VNI and the formula parser have their own mutation
//! property tests next to the code (`*_never_panic*` in bo-binary, bo-crypto, bo-odf,
//! bo-encoding, bo-opc, bo-xlsx, bo-formula).

use bo_engine::{Engine, SheetEngine};
use bo_opc::{Limits, Package};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Generous per-call budget for unoptimised builds: a hang shows as minutes, not ms.
const BUDGET: Duration = Duration::from_secs(20);

/// Mutations per corpus file (XML level) and per file (ZIP level).
const XML_MUTATIONS: usize = 12;
const ZIP_MUTATIONS: usize = 6;

/// Deterministic xorshift64* generator (no external RNG: the run is reproducible).
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            0
        } else {
            usize::try_from(self.next() % n as u64).unwrap_or(0)
        }
    }
}

/// Markup fragments worth injecting into WordprocessingML / SpreadsheetML /
/// PresentationML parts.
const INJECT: &[&str] = &[
    "<",
    ">",
    "&",
    "&#0;",
    "&#xD800;",
    "&bogus;",
    "\"",
    "</w:p>",
    "<w:p>",
    "<w:tbl><w:tr><w:tc>",
    "</w:tc></w:tr></w:tbl>",
    r#"<w:gridCol w:w="-99999999"/>"#,
    r#"<w:sz w:val="4294967295"/>"#,
    r#"<w:pgSz w:w="0" w:h="0"/>"#,
    r#"<w:numPr><w:ilvl w:val="99"/><w:numId w:val="99999"/></w:numPr>"#,
    "<w:vMerge/>",
    r#"<w:gridSpan w:val="65535"/>"#,
    r#"<c r="XFD1048576" t="s"><v>999999</v></c>"#,
    r#"<c r="A0"><f>SUM(A1:XFD1048576)</f></c>"#,
    r#"<mergeCell ref="A1:XFD1048576"/>"#,
    r#"<row r="0" spans="1:16384">"#,
    r#"<a:off x="-9223372036854775808" y="9223372036854775807"/>"#,
    r#"<a:ext cx="-1" cy="0"/>"#,
    r#"<a:prstGeom prst="nonexistent"><a:avLst><a:gd name="adj" fmla="val 1e308"/></a:avLst></a:prstGeom>"#,
    r#"<p:sldId id="0" r:id="rIdMissing"/>"#,
    r#"<mc:AlternateContent><mc:Choice Requires="w14">"#,
    "</mc:Choice></mc:AlternateContent>",
    r#"<Relationship Id="rId1" Type="x" Target="../../../../etc/passwd"/>"#,
    r#"<Relationship Id="rIdLoop" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="/word/document.xml"/>"#,
];

fn mutate_xml(rng: &mut Rng, data: &mut Vec<u8>) {
    if data.is_empty() {
        data.extend_from_slice(INJECT[rng.below(INJECT.len())].as_bytes());
        return;
    }
    let len = data.len();
    let at = rng.below(len);
    match rng.below(7) {
        0 => {
            // Flip a few bytes.
            for _ in 0..=rng.below(4) {
                let i = rng.below(len);
                data[i] ^= 1 << rng.below(8);
            }
        }
        1 => data.truncate(at),
        2 => {
            let end = (at + 1 + rng.below(256)).min(len);
            data.drain(at..end);
        }
        3 => {
            let end = (at + 1 + rng.below(512)).min(len);
            let chunk = data[at..end].to_vec();
            let to = rng.below(data.len());
            data.splice(to..to, chunk);
        }
        4 => {
            let frag = INJECT[rng.below(INJECT.len())].as_bytes();
            data.splice(at..at, frag.iter().copied());
        }
        5 => {
            // Deep nesting (XML depth limit, §5.9).
            let depth = 1 + rng.below(5000);
            let open = "<w:r>".repeat(depth);
            data.splice(at..at, open.bytes());
        }
        _ => {
            // Replace a digit run with an extreme number.
            let extreme: &[&[u8]] = &[b"-1", b"0", b"99999999999999999999", b"NaN", b"1e309"];
            if let Some(p) = data[at..].iter().position(u8::is_ascii_digit) {
                let s = at + p;
                let e = data[s..]
                    .iter()
                    .position(|b| !b.is_ascii_digit())
                    .map_or(data.len(), |q| s + q);
                let rep = extreme[rng.below(extreme.len())];
                data.splice(s..e, rep.iter().copied());
            }
        }
    }
}

fn corpus(dir: &str, ext: &str) -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus")
        .join(dir);
    let mut v: Vec<PathBuf> = std::fs::read_dir(&root)
        .map(|rd| {
            rd.filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| p.extension().is_some_and(|x| x == ext))
                .collect()
        })
        .unwrap_or_default();
    v.sort();
    v
}

/// Run `f` and fail when it exceeds the budget (it must also not panic: a panic
/// propagates and fails the test with the offending input named in `what`).
fn timed(what: &str, f: impl FnOnce()) {
    let t = Instant::now();
    f();
    let dt = t.elapsed();
    assert!(dt < BUDGET, "{what} took {dt:?} (hang?)");
}

/// The OOXML fuzz target bodies: package reader, format reader and the engine that
/// opens the result (layout / sheet model / slide renderer).
fn open_all(ext: &str, bytes: &[u8]) {
    let _ = Package::read(bytes, &Limits::default());
    match ext {
        "docx" => {
            let _ = bo_docx::read_docx(bytes);
            let mut e = Engine::with_bundled_fonts();
            if e.open(bytes, "fuzz.docx").is_ok() {
                let _ = e.page_json(0);
            }
        }
        "xlsx" => {
            let _ = bo_xlsx::read_xlsx(bytes);
            let mut s = SheetEngine::new();
            let _ = s.open(bytes, "fuzz.xlsx");
        }
        _ => {
            let _ = bo_pptx::read_pptx(bytes);
            let mut e = Engine::with_bundled_fonts();
            if e.open_pptx(bytes, "fuzz.pptx").is_ok() {
                let _ = e.page_json(0);
            }
        }
    }
}

fn run(dir: &str, ext: &str, seed: u64) {
    let files = corpus(dir, ext);
    assert!(!files.is_empty(), "corpus tests/corpus/{dir} has no .{ext}");
    let mut rng = Rng(seed);
    let mut runs = 0;
    for path in &files {
        let Ok(bytes) = std::fs::read(path) else {
            continue;
        };
        let name = path
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        let Ok(pkg) = Package::read(&bytes, &Limits::default()) else {
            panic!("corpus file {name} does not open");
        };
        let xml_parts: Vec<String> = pkg
            .parts()
            .iter()
            .filter(|p| {
                let n = p.name.to_ascii_lowercase();
                n.ends_with(".xml") || n.ends_with(".rels")
            })
            .map(|p| p.name.clone())
            .collect();
        for k in 0..XML_MUTATIONS {
            let mut m = pkg.clone();
            let part = &xml_parts[rng.below(xml_parts.len())];
            let mut data = m.part(part).map(|p| p.data.clone()).unwrap_or_default();
            mutate_xml(&mut rng, &mut data);
            m.put_part(part, data);
            let Ok(out) = m.write() else { continue };
            timed(&format!("{name} xml#{k} ({part})"), || open_all(ext, &out));
            runs += 1;
        }
        for k in 0..ZIP_MUTATIONS {
            let mut data = bytes.clone();
            let len = data.len();
            match rng.below(3) {
                0 => {
                    for _ in 0..=rng.below(8) {
                        let i = rng.below(len);
                        data[i] = u8::try_from(rng.below(256)).unwrap_or(0);
                    }
                }
                1 => data.truncate(rng.below(len)),
                _ => {
                    let at = rng.below(len);
                    let end = (at + 1 + rng.below(64)).min(len);
                    data.drain(at..end);
                }
            }
            timed(&format!("{name} zip#{k}"), || open_all(ext, &data));
            runs += 1;
        }
    }
    println!(
        "{dir}: {} files, {runs} mutated inputs, no panic, no hang",
        files.len()
    );
}

/// Run on a small, fixed stack (2 MiB, unoptimised): deep nesting must be rejected by
/// the depth limits, not overflow the 1 MiB stack of the WASM build.
fn on_small_stack(f: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .stack_size(2 << 20)
        .spawn(f)
        .expect("spawn")
        .join()
        .expect("fuzz smoke panicked");
}

#[test]
fn mutated_docx_corpus_never_panics_or_hangs() {
    on_small_stack(|| run("doc", "docx", 0x0D0C_0001));
}

#[test]
fn mutated_xlsx_corpus_never_panics_or_hangs() {
    on_small_stack(|| run("sheet", "xlsx", 0x0D0C_0002));
}

#[test]
fn mutated_pptx_corpus_never_panics_or_hangs() {
    on_small_stack(|| run("slide", "pptx", 0x0D0C_0003));
}
