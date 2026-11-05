//! End-to-end tests of the native pipeline on the Phase 1 corpus sample.

use bo_engine::Engine;

fn sample() -> Vec<u8> {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus/doc/phase1-sample.docx");
    std::fs::read(&p).unwrap_or_else(|e| {
        panic!(
            "{}: {e} (run `cargo run -p bo-cli -- sample` first)",
            p.display()
        )
    })
}

#[test]
fn opens_sample_and_paginates() {
    let mut e = Engine::with_bundled_fonts();
    let info = e.open(&sample(), "phase1-sample.docx").unwrap();
    assert!(info.page_count >= 3, "pages: {}", info.page_count);
    // Last section is landscape.
    let last = info.pages.last().unwrap();
    assert!(last.width > last.height);
    assert!((info.pages[0].width - 595.3).abs() < 0.1);
    let json = e.page_json(0).unwrap();
    assert!(json.contains("CỘNG HÒA XÃ HỘI CHỦ NGHĨA VIỆT NAM"));
}

#[test]
fn deterministic_display_list() {
    let mut a = Engine::with_bundled_fonts();
    let mut b = Engine::with_bundled_fonts();
    a.open(&sample(), "s.docx").unwrap();
    b.open(&sample(), "s.docx").unwrap();
    for i in 0..a.info().unwrap().page_count {
        assert_eq!(a.page_json(i).unwrap(), b.page_json(i).unwrap());
    }
}

#[test]
fn exports_pdf_with_embedded_fonts_and_text() {
    let mut e = Engine::with_bundled_fonts();
    let info = e.open(&sample(), "phase1-sample.docx").unwrap();
    let pdf = e.export_pdf().unwrap();
    let s = String::from_utf8_lossy(&pdf);
    assert!(pdf.starts_with(b"%PDF-"));
    let pages = s.matches("/Type /Page").count() - s.matches("/Type /Pages").count();
    assert_eq!(pages, info.page_count);
    assert!(s.contains("/FontFile2"));
    assert!(s.contains("/ToUnicode"));
    assert!(s.contains("Tinos"));
    assert!(pdf.len() < 1_500_000, "PDF too large: {}", pdf.len());
}

#[test]
fn renders_png() {
    let mut e = Engine::with_bundled_fonts();
    e.open(&sample(), "phase1-sample.docx").unwrap();
    let png = e.render_png(0, 1.0).unwrap();
    assert_eq!(&png[1..4], b"PNG");
    assert!(e.render_png(999, 1.0).is_err());
}

#[test]
fn glyph_outlines_for_canvas() {
    let e = Engine::with_bundled_fonts();
    let o = e.glyph_outlines(0, &[36, 3]);
    assert_eq!(o.len(), 2);
    assert!(o[0].path.starts_with('M'));
    assert!(e.glyph_outlines(9999, &[1]).is_empty());
}

#[test]
fn plain_text_and_garbage() {
    let mut e = Engine::with_bundled_fonts();
    let info = e.open("Dòng 1\nDòng 2".as_bytes(), "a.txt").unwrap();
    assert_eq!(info.page_count, 1);
    assert_eq!(info.word_count, 4);
    assert!(e.open(&[0xFF, 0xFE, 0x00, 0x9F], "x.bin").is_err());
    assert!(e.open(b"PK\x03\x04broken", "x.docx").is_err());
}
