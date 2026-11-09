//! End-to-end tests of the `bo-cli` binary (prompt §4 / P1.13): every subcommand
//! the CI and the server worker rely on is run as a real process on files of the
//! repository corpus, and its output files are checked.
#![cfg_attr(test, allow(clippy::unwrap_used))]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_bo-cli"))
}

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn corpus(rel: &str) -> PathBuf {
    repo().join("tests/corpus").join(rel)
}

/// Fresh scratch directory per test (removed first, kept after for inspection).
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("bo-cli-test-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn run(cmd: &mut Command) -> Output {
    let out = cmd.output().unwrap();
    assert!(
        out.status.success(),
        "{cmd:?} failed: {}\n{}",
        String::from_utf8_lossy(&out.stderr),
        String::from_utf8_lossy(&out.stdout)
    );
    out
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

const PNG_MAGIC: &[u8] = b"\x89PNG\r\n\x1a\n";

#[test]
fn sample_then_render_writes_png_pages() {
    let dir = scratch("render");
    let docx = dir.join("sample.docx");
    run(bin().arg("sample").arg(&docx));
    assert!(std::fs::read(&docx).unwrap().starts_with(b"PK"));
    let pattern = dir.join("page-%d.png");
    run(bin()
        .arg("render")
        .arg(&docx)
        .arg("--out")
        .arg(&pattern)
        .args(["--scale", "1"]));
    let first = std::fs::read(dir.join("page-1.png")).unwrap();
    assert!(first.starts_with(PNG_MAGIC));
    // A4 at 72 dpi: the IHDR width is 595 or 596 px depending on rounding.
    let width = u32::from_be_bytes(first[16..20].try_into().unwrap());
    assert!((594..=596).contains(&width), "width {width}");

    // --page renders only that page.
    let only = dir.join("only-%d.png");
    run(bin()
        .arg("render")
        .arg(&docx)
        .arg("--out")
        .arg(&only)
        .args(["--page", "1", "--scale", "0.5"]));
    assert!(dir.join("only-1.png").exists());
    assert!(!dir.join("only-2.png").exists());
}

#[test]
fn convert_docx_to_pdf_odt_and_txt() {
    let dir = scratch("convert-doc");
    let src = corpus("doc/phase1-sample.docx");
    for (to, magic) in [("pdf", &b"%PDF"[..]), ("odt", &b"PK"[..])] {
        let out = dir.join(format!("out.{to}"));
        run(bin()
            .arg("convert")
            .arg(&src)
            .args(["--to", to])
            .arg("--out")
            .arg(&out));
        assert!(std::fs::read(&out).unwrap().starts_with(magic), "{to}");
    }
    let txt = dir.join("out.txt");
    run(bin()
        .arg("convert")
        .arg(&src)
        .args(["--to", "txt"])
        .arg("--out")
        .arg(&txt));
    let text = std::fs::read_to_string(&txt).unwrap();
    assert!(!text.trim().is_empty(), "empty text export");
}

#[test]
fn convert_sheet_and_deck() {
    let dir = scratch("convert-sheet");
    let xlsx = corpus("sheet/values-1.xlsx");
    let csv = dir.join("values.csv");
    run(bin()
        .arg("convert")
        .arg(&xlsx)
        .args(["--to", "csv"])
        .arg("--out")
        .arg(&csv));
    assert!(std::fs::read_to_string(&csv).unwrap().contains(','));
    let pdf = dir.join("values.pdf");
    run(bin()
        .arg("convert")
        .arg(&xlsx)
        .args(["--to", "pdf"])
        .arg("--out")
        .arg(&pdf));
    assert!(std::fs::read(&pdf).unwrap().starts_with(b"%PDF"));

    let pptx = corpus("slide/slide-text-bullets.pptx");
    let deck_pdf = dir.join("deck.pdf");
    run(bin()
        .arg("convert")
        .arg(&pptx)
        .args(["--to", "pdf"])
        .arg("--out")
        .arg(&deck_pdf));
    assert!(std::fs::read(&deck_pdf).unwrap().starts_with(b"%PDF"));
}

fn pdf_pages(path: &Path) -> usize {
    let b = std::fs::read(path).unwrap();
    let s = String::from_utf8_lossy(&b);
    s.matches("/Type /Page").count() - s.matches("/Type /Pages").count()
}

#[test]
fn convert_deck_handouts_notes_pictures_package() {
    let dir = scratch("convert-handout");
    let pptx = corpus("slide/slide-anim-transitions.pptx");
    let plain = dir.join("deck.pdf");
    run(bin()
        .arg("convert")
        .arg(&pptx)
        .args(["--to", "pdf", "--out"])
        .arg(&plain));
    let n = pdf_pages(&plain);
    assert!(n >= 2, "{n}");
    // Handouts: ceil(n / k) pages.
    for k in [2usize, 6] {
        let out = dir.join(format!("handout-{k}.pdf"));
        run(bin()
            .arg("convert")
            .arg(&pptx)
            .args(["--to", "pdf", "--handout", &k.to_string(), "--out"])
            .arg(&out));
        assert_eq!(pdf_pages(&out), n.div_ceil(k), "{k} per page");
    }
    let notes = dir.join("notes.pdf");
    run(bin()
        .arg("convert")
        .arg(&pptx)
        .args(["--to", "pdf", "--notes-pages", "--letter", "--out"])
        .arg(&notes));
    assert_eq!(pdf_pages(&notes), n);
    let svg = dir.join("slide1.svg");
    run(bin()
        .arg("convert")
        .arg(&pptx)
        .args(["--to", "svg", "--slide", "1", "--width", "800", "--out"])
        .arg(&svg));
    assert!(std::fs::read_to_string(&svg).unwrap().contains("<svg "));
    let jpgs = dir.join("slides.zip");
    run(bin()
        .arg("convert")
        .arg(&pptx)
        .args(["--to", "jpg", "--out"])
        .arg(&jpgs));
    assert!(std::fs::read(&jpgs).unwrap().starts_with(b"PK"));
    let pkg = dir.join("package.zip");
    run(bin()
        .arg("convert")
        .arg(&pptx)
        .args(["--to", "zip", "--out"])
        .arg(&pkg));
    assert!(std::fs::read(&pkg).unwrap().starts_with(b"PK"));
}

#[test]
fn roundtrip_keeps_the_document_openable() {
    let dir = scratch("roundtrip");
    let src = corpus("doc/tables-02-merge.docx");
    let out = dir.join("again.docx");
    run(bin().arg("roundtrip").arg(&src).arg("--out").arg(&out));
    let before = stdout(&run(bin().arg("inspect").arg(&src)));
    let after = stdout(&run(bin().arg("inspect").arg(&out)));
    // Same paragraph count before and after open → save.
    let paras = |s: &str| {
        s.lines()
            .find(|l| l.starts_with("paragraphs:"))
            .map(str::to_owned)
    };
    assert!(paras(&before).is_some(), "{before}");
    assert_eq!(paras(&before), paras(&after));
}

#[test]
fn inspect_reports_format_encryption_and_macros() {
    let dir = scratch("inspect");
    let src = corpus("doc/phase1-sample.docx");
    let plain = stdout(&run(bin().arg("inspect").arg(&src)));
    assert!(plain.contains("format: docx"), "{plain}");
    assert!(plain.contains("encrypted: false"), "{plain}");
    assert!(plain.contains("macros: false"), "{plain}");

    // An encrypted copy is detected, and opens again with its password.
    let locked = dir.join("locked.docx");
    run(bin()
        .arg("convert")
        .arg(&src)
        .args(["--to", "docx", "--protect", "Mật-khẩu-1"])
        .arg("--out")
        .arg(&locked));
    let info = stdout(&run(bin()
        .arg("inspect")
        .arg(&locked)
        .args(["--password", "Mật-khẩu-1"])));
    assert!(info.contains("encrypted: true"), "{info}");
    let wrong = bin()
        .arg("inspect")
        .arg(&locked)
        .args(["--password", "sai"])
        .output()
        .unwrap();
    assert!(!wrong.status.success());

    let sheet = stdout(&run(bin()
        .arg("inspect")
        .arg(corpus("sheet/values-1.xlsx"))));
    assert!(sheet.contains("sheet: "), "{sheet}");
}

#[test]
fn sign_and_verify_pdf_and_docx() {
    let dir = scratch("sign");
    let signer = bo_sign::testing::self_signed(&mut rand::thread_rng(), "Nguyễn Văn A");
    let (key, cert) = bo_sign::testing::to_pem(&signer);
    let key_path = dir.join("key.pem");
    let cert_path = dir.join("cert.pem");
    std::fs::write(&key_path, key).unwrap();
    std::fs::write(&cert_path, cert).unwrap();

    let pdf = dir.join("doc.pdf");
    run(bin()
        .arg("convert")
        .arg(corpus("doc/phase1-sample.docx"))
        .args(["--to", "pdf"])
        .arg("--out")
        .arg(&pdf));
    let docx = dir.join("doc.docx");
    std::fs::copy(corpus("doc/phase1-sample.docx"), &docx).unwrap();

    for input in [&pdf, &docx] {
        // Unsigned input: verify finds nothing.
        assert!(stdout(&run(bin().arg("verify").arg(input))).contains("no signatures"));
        let signed = run(bin()
            .arg("sign")
            .arg(input)
            .arg("--key")
            .arg(&key_path)
            .arg("--cert")
            .arg(&cert_path)
            .args(["--name", "Nguyễn Văn A", "--reason", "Phê duyệt"]));
        let signed = PathBuf::from(stdout(&signed).trim());
        assert!(signed.exists(), "{}", signed.display());
        let report = stdout(&run(bin().arg("verify").arg(&signed)));
        assert!(report.contains("valid"), "{report}");
        assert!(!report.contains("no signatures"), "{report}");
    }

    // A PDF changed after signing is reported (byte range no longer covers it).
    let signed_pdf = dir.join("doc-signed.pdf");
    let mut bytes = std::fs::read(&signed_pdf).unwrap();
    bytes.extend_from_slice(b"\n% appended after signing\n");
    let tampered = dir.join("tampered.pdf");
    std::fs::write(&tampered, bytes).unwrap();
    let report = stdout(&run(bin().arg("verify").arg(&tampered)));
    assert!(
        report.contains("document changed after this signature"),
        "{report}"
    );
}

#[test]
fn bad_input_fails_cleanly() {
    let dir = scratch("bad");
    let junk = dir.join("junk.docx");
    std::fs::write(&junk, b"PK\x03\x04 not really a zip").unwrap();
    let out = bin().arg("inspect").arg(&junk).output().unwrap();
    assert!(!out.status.success());
    // An error message, never a panic.
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(!err.contains("panicked"), "{err}");
}

/// `roundtrip` saves workbooks and decks in their own format (not as a .docx), and the result
/// reads back with the same sheets / slides.
#[test]
fn roundtrip_keeps_workbooks_and_decks_in_their_format() {
    let dir = scratch("roundtrip-formats");
    let xlsx = dir.join("ext-1.xlsx");
    run(bin()
        .arg("roundtrip")
        .arg(corpus("sheet/ext-1.xlsx"))
        .arg("--out")
        .arg(&xlsx));
    let before = bo_xlsx::read_xlsx(&std::fs::read(corpus("sheet/ext-1.xlsx")).unwrap()).unwrap();
    let after = bo_xlsx::read_xlsx(&std::fs::read(&xlsx).unwrap()).unwrap();
    assert_eq!(
        after.sheets.iter().map(|s| &s.name).collect::<Vec<_>>(),
        before.sheets.iter().map(|s| &s.name).collect::<Vec<_>>()
    );

    let pptx = dir.join("deck.pptx");
    run(bin()
        .arg("roundtrip")
        .arg(corpus("slide/slide-text-bullets.pptx"))
        .arg("--out")
        .arg(&pptx));
    let bytes = std::fs::read(&pptx).unwrap();
    // A presentation package: its main part is ppt/presentation.xml, not word/document.xml.
    let text = String::from_utf8_lossy(&bytes);
    assert!(text.contains("ppt/presentation.xml"));
    assert!(!text.contains("word/document.xml"));
}
