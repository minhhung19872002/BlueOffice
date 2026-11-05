//! Tests against the bundled fonts in `assets/fonts`.

use bo_text::{outline_svg_path, FontManager};

fn manager() -> FontManager {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/fonts");
    let mut fm = FontManager::new();
    assert!(fm.add_fonts_from_dir(&dir) >= 12, "bundled fonts missing");
    fm
}

#[test]
fn metric_compatible_fallback() {
    let mut fm = manager();
    let tnr = fm.resolve("Times New Roman", false, false).unwrap();
    assert_eq!(fm.info(tnr).family, "Tinos");
    let arial_b = fm.resolve("Arial", true, false).unwrap();
    assert_eq!(fm.info(arial_b).family, "Liberation Sans");
    assert!(fm.info(arial_b).bold);
    let cal_i = fm.resolve("Calibri", false, true).unwrap();
    assert_eq!(fm.info(cal_i).family, "Carlito");
    assert!(fm.info(cal_i).italic && !fm.info(cal_i).bold);
    // Unknown family → last resort.
    let x = fm.resolve("Some Missing Font", true, true).unwrap();
    assert_eq!(fm.info(x).family, "Tinos");
}

#[test]
fn shapes_vietnamese_precomposed_and_decomposed_alike() {
    let mut fm = manager();
    let f = fm.resolve("Times New Roman", false, false).unwrap();
    let pre = bo_text::nfc("Nghị định số 30/2020/NĐ-CP");
    let dec = bo_text::nfc("Nghi\u{0323} \u{0111}i\u{0323}nh số 30/2020/NĐ-CP");
    let a = fm.measure(f, &pre, 13.0);
    let b = fm.measure(f, &dec, 13.0);
    assert!((a - b).abs() < 0.01, "{a} vs {b}");
    assert!(a > 50.0 && a < 250.0);
    // No .notdef glyphs for Vietnamese.
    assert!(fm.shape(f, &pre).iter().all(|g| g.glyph_id != 0));
}

#[test]
fn tinos_matches_times_metrics() {
    // Metric compatibility: 'M' advance in Times New Roman is 0.889 em.
    let mut fm = manager();
    let f = fm.resolve("Times New Roman", false, false).unwrap();
    let w = fm.measure(f, "M", 1000.0);
    assert!((w - 889.0).abs() < 1.5, "{w}");
}

#[test]
fn itemize_falls_back_for_missing_glyphs() {
    let mut fm = manager();
    let f = fm.resolve("Times New Roman", false, false).unwrap();
    let items = fm.itemize(f, "abc");
    assert_eq!(items, vec![(0..3, f)]);
}

#[test]
fn outlines() {
    let mut fm = manager();
    let f = fm.resolve("Arial", false, false).unwrap();
    let g = fm.shape(f, "A")[0].glyph_id;
    let path = outline_svg_path(&fm.outline(f, g));
    assert!(path.starts_with('M') && path.ends_with('Z'));
    let space = fm.shape(f, " ")[0].glyph_id;
    assert_eq!(fm.outline(f, space).len(), 0);
}
