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

#[test]
fn cambria_falls_back_to_caladea() {
    let mut fm = manager();
    let c = fm.resolve("Cambria", true, true).unwrap();
    assert_eq!(fm.info(c).family, "Caladea");
    assert!(fm.info(c).bold && fm.info(c).italic);
    let r = fm.resolve("Cambria", false, false).unwrap();
    let w = fm.measure(r, "M", 1000.0);
    assert!(w > 800.0 && w < 950.0, "{w}");
    // Letters missing from Caladea (if any) come from another bundled face.
    let items = fm.itemize(r, "Nghị định");
    assert!(items.iter().all(|(_, f)| fm.has_glyph(*f, 'N')));
}

#[test]
fn kerning_follows_the_flag() {
    let mut fm = manager();
    let f = fm.resolve("Times New Roman", false, false).unwrap();
    let on: i32 = fm
        .shape_kern(f, "AVAV", true)
        .iter()
        .map(|g| g.x_advance)
        .sum();
    let off: i32 = fm
        .shape_kern(f, "AVAV", false)
        .iter()
        .map(|g| g.x_advance)
        .sum();
    assert!(on < off, "{on} vs {off}");
}

/// System fonts (desktop): an installed family is loaded on first use, after
/// the bundled faces, whose ids stay as they were.
#[test]
fn system_fonts_load_lazily_after_bundled_faces() {
    use bo_text::{FaceId, SystemFontIndex};
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/fonts");
    let mut fm = FontManager::new();
    // "Bundled": Tinos only.
    for f in ["Tinos-Regular.ttf", "Tinos-Bold.ttf"] {
        fm.add_font(std::fs::read(dir.join(f)).unwrap());
    }
    // "Installed": the whole folder.
    fm.set_system_fonts(SystemFontIndex::scan(std::slice::from_ref(&dir)));
    assert_eq!(fm.len(), 2, "nothing is loaded before a document asks");
    assert!(fm.system_families().iter().any(|f| f == "Carlito"));

    // A bundled family never reads the installed copy.
    let t = fm.resolve("Tinos", true, false).unwrap();
    assert_eq!((t, fm.len()), (FaceId(1), 2));

    // Installed family: its four files are loaded, with ids after the bundled ones.
    let c = fm.resolve("carlito", false, true).unwrap();
    assert_eq!(fm.len(), 6);
    assert!(c.0 >= 2);
    assert_eq!(fm.info(c).family, "Carlito");
    assert!(fm.info(c).italic);
    assert_eq!(fm.info(FaceId(0)).family, "Tinos");

    // Calibri is not installed here: the metric-compatible substitute (now loaded) is used.
    let cal = fm.resolve("Calibri", false, false).unwrap();
    assert_eq!(fm.info(cal).family, "Carlito");
    // Unknown family: last resort, and no file is read again.
    let n = fm.len();
    let x = fm.resolve("No Such", false, false).unwrap();
    assert_eq!(fm.info(x).family, "Tinos");
    assert_eq!(fm.len(), n);
}
