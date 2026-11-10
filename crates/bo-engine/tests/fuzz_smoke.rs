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
//! * the text formats of CORE-FMT-003 — HTML, MHTML, Flat OPC and XML Spreadsheet
//!   2003 — derived from the same corpus (and the hand-written `xml2003-*.xml`
//!   fixtures) and mutated as text, then opened by both engines.
//!
//! * the legacy tabular formats of CORE-FMT-004 — SYLK, DIF, formatted text and
//!   dBASE tables — from hand-written fixtures and files saved from the corpus;
//!   and PDF files opened as documents, from PDF exports of the corpus.
//! * the VBA project behind the read-only macro viewer (CORE-VBA-002): the
//!   `vbaProject.bin` of the generated macro corpus, damaged as bytes and inside its
//!   compressed streams.
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

/// CORE-VBA-002: the VBA project reader behind the macro viewer ([MS-OVBA]: compound
/// file, compressed `dir` stream, module streams). The `vbaProject.bin` of the
/// generated macro corpus is damaged at byte level and inside its streams (the
/// compressed containers are decompressed, mutated and compressed again so that the
/// record parser sees the damage), then read directly and through the engines.
#[test]
fn mutated_vba_projects_never_panic_or_hang() {
    const MUTATIONS: usize = 60;
    on_small_stack(|| {
        let mut rng = Rng(0x0D0C_0BA5);
        let mut runs = 0;
        for (file, main_dir) in [
            ("macro-vba.docm", "word"),
            ("macro-vba.xlsm", "xl"),
            ("macro-vba.pptm", "ppt"),
        ] {
            let path = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../tests/corpus/macro")
                .join(file);
            let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("{file}: {e}"));
            let pkg = Package::read(&bytes, &Limits::default()).expect("macro corpus opens");
            let part = format!("/{main_dir}/vbaProject.bin");
            let bin = pkg.part(&part).expect("vbaProject.bin").data.clone();
            assert!(
                bo_engine::vba_view::read(&bytes).is_some_and(|p| p.error.is_none()),
                "{file}: the unmutated project must read"
            );
            for k in 0..MUTATIONS {
                let mut data = bin.clone();
                let len = data.len();
                match rng.below(4) {
                    0 => {
                        for _ in 0..=rng.below(8) {
                            let i = rng.below(len);
                            data[i] = u8::try_from(rng.below(256)).unwrap_or(0);
                        }
                    }
                    1 => data.truncate(rng.below(len)),
                    2 => {
                        let at = rng.below(len);
                        let end = (at + 1 + rng.below(64)).min(len);
                        data.drain(at..end);
                    }
                    _ => data = mutate_vba_streams(&mut rng, &bin),
                }
                let mut m = pkg.clone();
                m.put_part(&part, data.clone());
                let Ok(out) = m.write() else { continue };
                timed(&format!("{file} vba#{k}"), || {
                    let _ = bo_binary::read_vba_project(&data);
                    let _ = bo_engine::vba_view::read(&out);
                    let cmd = r#"{"cmd":"vbaProject"}"#;
                    match main_dir {
                        "xl" => {
                            let mut s = SheetEngine::new();
                            if s.open(&out, "fuzz.xlsm").is_ok() {
                                let _ = s.exec_json(cmd);
                            }
                        }
                        "word" => {
                            let mut e = Engine::with_bundled_fonts();
                            if e.open(&out, "fuzz.docm").is_ok() {
                                let _ = e.command_json(cmd);
                            }
                        }
                        _ => {
                            let mut e = Engine::with_bundled_fonts();
                            if e.open(&out, "fuzz.pptm").is_ok() {
                                let _ = e.slide_command_json(cmd);
                            }
                        }
                    }
                });
                runs += 1;
            }
        }
        println!("macro: {runs} mutated VBA projects, no panic, no hang");
    });
}

/// A copy of the compound file `bin` with one of its `VBA` streams damaged after
/// decompression ([MS-OVBA] CompressedContainer) and compressed again.
fn mutate_vba_streams(rng: &mut Rng, bin: &[u8]) -> Vec<u8> {
    use std::io::{Cursor, Read, Write};
    let Ok(mut cf) = cfb::CompoundFile::open(Cursor::new(bin.to_vec())) else {
        return bin.to_vec();
    };
    let streams: Vec<String> = cf
        .walk()
        .filter(cfb::Entry::is_stream)
        .map(|e| e.path().to_string_lossy().replace('\\', "/"))
        .filter(|p| p.starts_with("/VBA/") && p != "/VBA/_VBA_PROJECT")
        .collect();
    if streams.is_empty() {
        return bin.to_vec();
    }
    let path = &streams[rng.below(streams.len())];
    let mut packed = Vec::new();
    if let Ok(mut s) = cf.open_stream(path) {
        let _ = s.read_to_end(&mut packed);
    }
    let mut raw = bo_binary::vba::decompress(&packed).unwrap_or(packed);
    for _ in 0..=rng.below(6) {
        if raw.is_empty() {
            break;
        }
        let i = rng.below(raw.len());
        match rng.below(3) {
            0 => raw[i] = u8::try_from(rng.below(256)).unwrap_or(0),
            // Record sizes and offsets are little-endian u32: make one huge.
            1 => {
                let end = (i + 4).min(raw.len());
                raw[i..end].fill(0xFF);
            }
            _ => raw.truncate(i),
        }
    }
    if let Ok(mut s) = cf.create_stream(path) {
        let _ = s.write_all(&bo_binary::vba::compress(&raw));
    }
    let _ = cf.flush();
    cf.into_inner().into_inner()
}

/// Markup worth injecting into HTML, MHTML, Flat OPC and XML Spreadsheet 2003 text.
const WEB_INJECT: &[&str] = &[
    "<table><tr><td colspan=99999 rowspan=99999>",
    "</td></tr></table>",
    "<ul><li><ol><li>",
    "<p style=\"margin:1e30pt;font-size:1e30px;line-height:0;width:-5%\">",
    "<img src=\"data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAA\">",
    "<script>while(1){}</script>",
    "<style>p{color:red;}}}{{</style>",
    "<a class=\"bo-noteref\" href=\"#bo-note-f1\">",
    "<div class=\"bo-note\" id=\"bo-note-f1\">",
    "&#x110000;&#0;&bogus;",
    "<!--",
    "<![CDATA[",
    "=\r\n",
    "=ZZ",
    "\r\n--b\r\nContent-Type: multipart/related; boundary=b\r\n\r\n",
    "\r\nContent-Transfer-Encoding: base64\r\n\r\n!!!!",
    "<pkg:part pkg:name=\"/../../x\" pkg:contentType=\"a/b\"><pkg:binaryData>",
    "<pkg:part pkg:name=\"/word/document.xml\"><pkg:xmlData>",
    "</pkg:xmlData></pkg:part>",
    "<Row ss:Index=\"1048577\" ss:Span=\"4294967295\">",
    "<Cell ss:Index=\"16385\" ss:MergeAcross=\"4294967295\" ss:Formula=\"=R[-9999999]C[\">",
    "<Data ss:Type=\"DateTime\">9999-99-99T99:99:99</Data>",
    "<Style ss:ID=\"Default\" ss:Parent=\"Default\">",
    "<Column ss:Index=\"0\" ss:Span=\"99999999\" ss:Width=\"1e308\"/>",
];

fn mutate_web(rng: &mut Rng, data: &mut Vec<u8>) {
    if rng.below(3) == 0 {
        let at = rng.below(data.len().max(1)).min(data.len());
        let frag = WEB_INJECT[rng.below(WEB_INJECT.len())].as_bytes();
        data.splice(at..at, frag.iter().copied());
    } else {
        mutate_xml(rng, data);
    }
}

/// Both engines on one text input: the document engine (HTML, MHTML, Flat OPC of a
/// document or a deck) and the sheet engine (HTML tables, XML Spreadsheet 2003,
/// Flat OPC of a workbook), plus the readers behind them.
fn open_web(name: &str, bytes: &[u8]) {
    let _ = bo_engine::web_formats::detect(bytes, name);
    let _ = Package::read(bytes, &Limits::default());
    let _ = bo_xlsx::read_xml_spreadsheet(bytes);
    let mut e = Engine::with_bundled_fonts();
    if e.open(bytes, name).is_ok() {
        let _ = e.page_json(0);
        let _ = e.export_as("html");
        let _ = e.save_docx();
    }
    let mut s = SheetEngine::new();
    if s.open(bytes, name).is_ok() {
        let _ = s.save("html", 0);
        let _ = s.save("xlsx", 0);
    }
}

/// Seeds: (file name, bytes) of every CORE-FMT-003 text format, made from the corpus.
fn web_seeds() -> Vec<(String, Vec<u8>)> {
    let mut seeds = Vec::new();
    let small = |dir: &str, ext: &str| -> Vec<PathBuf> {
        let mut v: Vec<(u64, PathBuf)> = corpus(dir, ext)
            .into_iter()
            .filter_map(|p| Some((std::fs::metadata(&p).ok()?.len(), p)))
            .collect();
        v.sort();
        v.into_iter().take(2).map(|(_, p)| p).collect()
    };
    for path in small("doc", "docx") {
        let bytes = std::fs::read(&path).expect("corpus file");
        let mut e = Engine::with_bundled_fonts();
        e.open(&bytes, "seed.docx").expect("corpus docx opens");
        seeds.push(("seed.html".to_owned(), e.export_as("html").expect("html")));
        seeds.push(("seed.mht".to_owned(), e.export_as("mht").expect("mht")));
        seeds.push((
            "seed.xml".to_owned(),
            e.export_as("xml").expect("flat docx"),
        ));
    }
    for path in small("slide", "pptx").into_iter().take(1) {
        let bytes = std::fs::read(&path).expect("corpus file");
        seeds.push((
            "deck.xml".to_owned(),
            bo_opc::flat::zip_to_flat(&bytes).expect("flat pptx"),
        ));
    }
    for path in small("sheet", "xlsx") {
        let bytes = std::fs::read(&path).expect("corpus file");
        let mut s = SheetEngine::new();
        s.open(&bytes, "seed.xlsx").expect("corpus xlsx opens");
        seeds.push((
            "book.html".to_owned(),
            s.save("html", 0).expect("html tables"),
        ));
        seeds.push(("book.xml".to_owned(), s.save("xml", 0).expect("flat xlsx")));
    }
    for path in corpus("sheet", "xml") {
        seeds.push((
            "book2003.xml".to_owned(),
            std::fs::read(&path).expect("fixture"),
        ));
    }
    seeds
}

#[test]
fn mutated_web_formats_never_panic_or_hang() {
    on_small_stack(|| {
        let seeds = web_seeds();
        assert!(seeds.len() >= 10, "seeds: {}", seeds.len());
        let mut rng = Rng(0x0D0C_0004);
        let mut runs = 0;
        for (name, bytes) in &seeds {
            // The seed itself opens.
            timed(name, || open_web(name, bytes));
            for k in 0..XML_MUTATIONS {
                let mut data = bytes.clone();
                for _ in 0..=rng.below(3) {
                    mutate_web(&mut rng, &mut data);
                }
                timed(&format!("{name} #{k}"), || open_web(name, &data));
                runs += 1;
            }
        }
        println!(
            "web formats: {} seeds, {runs} mutated inputs, no panic, no hang",
            seeds.len()
        );
    });
}

// ---------------------------------------------------------------- CORE-FMT-004
// SYLK, DIF, formatted text and dBASE tables: the hand-written fixtures and files
// saved from the corpus, mutated as bytes and with records of each format.

/// Records worth injecting into SYLK / DIF / fixed-width text.
const TABULAR_INJECT: &[&str] = &[
    "\r\nC;Y1048576;X16384;K1e308;ER[-9999999]C[9999999]\r\n",
    "\r\nC;Y0;X0;K\"\r\n",
    "\r\nC;X99999999;K1\r\n",
    "\r\nC;Y2;X2;S;R99999;C99999\r\n",
    "\r\nF;W1 16384 255\r\nF;W0 4294967295 1e99\r\n",
    "\r\nF;P4294967295;FG0X;SDIM99999999999;Y3;X3\r\n",
    "\r\nF;M4294967295;R1048576\r\nF;P1;C16384\r\n",
    "\r\nP;P;;;;;;;;\r\nP;F;M-1;SBI\r\n",
    "\r\nNN;N;E((((\r\nNN;NA1;ER1C1\r\n",
    "\r\nO;V4\r\nE\r\n",
    ";;",
    "\u{1b} :",
    "\r\n-1,0\r\nBOT\r\n",
    "\r\n-1,0\r\nEOD\r\n",
    "\r\n0,1e999\r\nV\r\n",
    "\r\n1,0\r\n\"\"\"\r\n",
    "\r\n0,\r\nNA\r\n9,9\r\n",
    "\r\nDATA\r\n0,0\r\n\"\"\r\n",
    "                                                                        x",
    "\t\t\t",
    "\u{0c}\u{1a}",
];

/// Offsets of a dBASE table worth damaging: record count, header length, record
/// length, language driver, type / length / decimals of the first two fields.
const DBF_OFFSETS: [usize; 11] = [4, 8, 9, 10, 11, 29, 43, 48, 49, 75, 80];

fn mutate_tabular(rng: &mut Rng, data: &mut Vec<u8>) {
    match rng.below(4) {
        0 => {
            let at = rng.below(data.len().max(1)).min(data.len());
            let frag = TABULAR_INJECT[rng.below(TABULAR_INJECT.len())].as_bytes();
            data.splice(at..at, frag.iter().copied());
        }
        1 if !data.is_empty() => {
            let at = DBF_OFFSETS[rng.below(DBF_OFFSETS.len())].min(data.len() - 1);
            data[at] = (rng.next() & 0xFF) as u8;
        }
        _ => mutate_xml(rng, data),
    }
}

/// The readers directly, then the sheet engine with every way out.
fn open_tabular(name: &str, bytes: &[u8]) {
    let _ = bo_tabular::detect(bytes, name);
    let _ = bo_tabular::read_sylk(bytes);
    let _ = bo_tabular::read_dif(bytes);
    let _ = bo_tabular::read_prn(bytes, ',');
    let _ = bo_tabular::read_dbf(bytes);
    let mut s = SheetEngine::new();
    if s.open(bytes, name).is_ok() {
        for format in ["slk", "dif", "prn", "csv", "xlsx"] {
            let _ = s.save(format, 0);
        }
    }
}

fn tabular_seeds() -> Vec<(String, Vec<u8>)> {
    let mut seeds = Vec::new();
    for ext in ["slk", "dif", "prn", "dbf"] {
        for path in corpus("sheet", ext) {
            seeds.push((
                format!("fixture.{ext}"),
                std::fs::read(&path).expect("fixture"),
            ));
        }
    }
    // Files the engine itself saves from the smallest workbooks of the corpus.
    let mut books: Vec<(u64, PathBuf)> = corpus("sheet", "xlsx")
        .into_iter()
        .filter_map(|p| Some((std::fs::metadata(&p).ok()?.len(), p)))
        .collect();
    books.sort();
    for (_, path) in books.into_iter().take(3) {
        let bytes = std::fs::read(&path).expect("corpus file");
        let mut s = SheetEngine::new();
        s.open(&bytes, "seed.xlsx").expect("corpus xlsx opens");
        for ext in ["slk", "dif", "prn"] {
            seeds.push((format!("saved.{ext}"), s.save(ext, 0).expect("saves")));
        }
    }
    seeds
}

#[test]
fn mutated_tabular_formats_never_panic_or_hang() {
    on_small_stack(|| {
        let seeds = tabular_seeds();
        assert!(seeds.len() >= 13, "seeds: {}", seeds.len());
        let mut rng = Rng(0x0D0C_0004 ^ 0x5A5A);
        let mut runs = 0;
        for (name, bytes) in &seeds {
            timed(name, || open_tabular(name, bytes));
            for k in 0..XML_MUTATIONS * 2 {
                let mut data = bytes.clone();
                for _ in 0..=rng.below(3) {
                    mutate_tabular(&mut rng, &mut data);
                }
                timed(&format!("{name} #{k}"), || open_tabular(name, &data));
                runs += 1;
            }
        }
        println!(
            "tabular formats: {} seeds, {runs} mutated inputs, no panic, no hang",
            seeds.len()
        );
    });
}

/// PDF files opened as documents (CORE-FMT-004, best effort): files the engine
/// exports from the smallest documents of the corpus, mutated as bytes (cut cross
/// reference tables, wrong offsets and lengths, flipped stream bytes).
#[test]
fn mutated_pdf_files_never_panic_or_hang() {
    on_small_stack(|| {
        let mut docs: Vec<(u64, PathBuf)> = corpus("doc", "docx")
            .into_iter()
            .filter_map(|p| Some((std::fs::metadata(&p).ok()?.len(), p)))
            .collect();
        docs.sort();
        let mut seeds = Vec::new();
        for (_, path) in docs.into_iter().take(3) {
            let mut e = Engine::with_bundled_fonts();
            e.open(&std::fs::read(&path).expect("corpus file"), "seed.docx")
                .expect("corpus docx opens");
            seeds.push(e.export_pdf().expect("exports"));
        }
        let mut rng = Rng(0x0D0C_0004 ^ 0x0F0F);
        let mut runs = 0;
        for (i, bytes) in seeds.iter().enumerate() {
            let open = |b: &[u8]| {
                let _ = Engine::with_bundled_fonts().open(b, "fuzz.pdf");
            };
            timed(&format!("pdf seed {i}"), || open(bytes));
            for k in 0..XML_MUTATIONS * 2 {
                let mut data = bytes.clone();
                for _ in 0..=rng.below(3) {
                    mutate_xml(&mut rng, &mut data);
                }
                timed(&format!("pdf seed {i} #{k}"), || open(&data));
                runs += 1;
            }
        }
        println!(
            "pdf: {} seeds, {runs} mutated inputs, no panic, no hang",
            seeds.len()
        );
    });
}
