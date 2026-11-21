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
fn vietnamese_combining_cluster_has_no_missing_glyphs_after_itemization() {
    let mut fm = manager();
    let caladea = fm.resolve("Caladea", false, false).unwrap();
    for text in ["X a\u{0323} Y", "a\u{0302}\u{0323}"] {
        let items = fm.itemize(caladea, text);
        assert_eq!(items.first().unwrap().0.start, 0);
        assert_eq!(items.last().unwrap().0.end, text.len());
        assert!(items
            .windows(2)
            .all(|pair| pair[0].0.end == pair[1].0.start));
        for (range, face) in items {
            assert!(
                fm.shape(face, &text[range]).iter().all(|g| g.glyph_id != 0),
                "combining sequence must be covered by the chosen font"
            );
        }
    }
    let decomposed = "a\u{0323}";
    let composed = "ạ";
    let a = fm.itemize(caladea, decomposed);
    let b = fm.itemize(caladea, composed);
    assert_eq!(a.len(), 1);
    assert_eq!(b.len(), 1);
    assert_eq!(a[0].1, b[0].1);
    assert_eq!(fm.shape(a[0].1, decomposed), fm.shape(b[0].1, composed));
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

/// Installed families with width and weight variants (Windows: Arial, Arial Narrow,
/// Arial Black share the typographic family "Arial"): "Arial" is the normal-width face
/// at regular or bold weight, "Arial Narrow" is found by its own name. Skipped where
/// those files are not installed.
#[test]
fn installed_width_variants_do_not_replace_the_regular_face() {
    use bo_text::SystemFontIndex;
    let dir = std::path::PathBuf::from(r"C:\Windows\Fonts");
    let needed = ["arial.ttf", "arialbd.ttf", "ARIALN.TTF", "ariblk.ttf"];
    if !needed.iter().all(|f| dir.join(f).exists()) {
        return;
    }
    let tmp = std::env::temp_dir().join(format!("bo-text-arial-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();
    for f in needed {
        std::fs::copy(dir.join(f), tmp.join(f)).unwrap();
    }
    let mut fm = FontManager::new();
    fm.set_system_fonts(SystemFontIndex::scan(std::slice::from_ref(&tmp)));
    let regular = fm.resolve("Arial", false, false).unwrap();
    assert_eq!((fm.info(regular).width, fm.info(regular).weight), (5, 400));
    let bold = fm.resolve("Arial", true, false).unwrap();
    assert_eq!((fm.info(bold).width, fm.info(bold).weight), (5, 700));
    let narrow = fm.resolve("Arial Narrow", false, false).unwrap();
    assert!(fm.info(narrow).width < 5, "Arial Narrow by its own name");
    let black = fm.resolve("Arial Black", false, false).unwrap();
    assert!(fm.info(black).weight >= 800, "Arial Black by its own name");
    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn windows_metrics_and_typo_flag() {
    let mut fm = manager();
    // Carlito: usWinAscent 1950, usWinDescent 550, no USE_TYPO_METRICS.
    let c = fm.resolve("Calibri", false, false).unwrap();
    let i = fm.info(c);
    assert_eq!(
        (i.win_ascent, i.win_descent, i.use_typo_metrics),
        (1950, 550, false)
    );
    // Caladea 1.002 (metric-compatible with Cambria): Cambria's Windows extents
    // 0.950 / 0.222 em, no USE_TYPO_METRICS.
    let cam = fm.resolve("Cambria", false, false).unwrap();
    let i = fm.info(cam);
    assert_eq!(
        (i.win_ascent, i.win_descent, i.use_typo_metrics),
        (950, 222, false)
    );
    // STIX Two Math asks for its typographic metrics (fsSelection bit 7).
    let math = fm.resolve("Cambria Math", false, false).unwrap();
    assert!(fm.info(math).use_typo_metrics);
}

/// Caladea stands in for Cambria with Cambria's advance widths (the metric-compatible
/// release; the earlier bundled design was up to 19 % narrower or wider per glyph).
#[test]
fn caladea_has_the_advance_widths_of_cambria() {
    let mut fm = manager();
    let cam = fm.resolve("Cambria", false, false).unwrap();
    let upem = f32::from(fm.info(cam).units_per_em);
    let mut em = |s: &str| -> f32 {
        fm.shape_kern(cam, s, false)
            .iter()
            .map(|g| g.x_advance as f32)
            .sum::<f32>()
            / upem
    };
    // Cambria's advances: "1" 0.5537 em, "w" 0.7744 em,
    // "o" 0.5308 em.
    assert!((em("1") - 0.5537).abs() < 0.002, "{}", em("1"));
    assert!((em("w") - 0.7744).abs() < 0.002, "{}", em("w"));
    assert!((em("o") - 0.5308).abs() < 0.002, "{}", em("o"));
}

/// Line extents of the reference word processor (GDI `TEXTMETRIC`): Windows ascent and
/// descent, plus the part of `hhea.lineGap` they do not cover.
#[test]
fn line_metrics_follow_the_windows_text_metrics() {
    let mut fm = manager();
    // Tinos (Times New Roman): win 1825 / 443 = hhea 1825 / −443, lineGap 87 kept.
    let t = fm.resolve("Times New Roman", false, false).unwrap();
    assert_eq!(fm.info(t).line_metrics(), (1825.0, 443.0, 87.0));
    // Carlito (Calibri): win 1950 / 550, no external leading.
    let c = fm.resolve("Calibri", false, false).unwrap();
    assert_eq!(fm.info(c).line_metrics(), (1950.0, 550.0, 0.0));
    // Caladea stands in for Cambria: Cambria's single lines (1.1724 em).
    let cam = fm.resolve("Cambria", false, false).unwrap();
    let (a, d, g) = fm.info(cam).line_metrics();
    let em = f32::from(fm.info(cam).units_per_em);
    assert!(((a + d + g) / em - 1.1724).abs() < 1e-3);
}

/// Hostile files (one text box of megabytes without a break opportunity): the run is
/// cut and measured in many different pieces, each shaped once. The shaping cache must
/// neither keep the long texts nor grow past its byte budget.
#[test]
fn shape_cache_is_bounded_in_bytes() {
    let mut fm = manager();
    let f = fm.resolve("Calibri", false, false).unwrap();
    let long: String = (0..100_000u32)
        .map(|i| char::from(b'a' + u8::try_from(i * 7 % 26).unwrap()))
        .collect();
    assert!(fm.measure(f, &long, 20.0) > 0.0);
    assert_eq!(fm.cached_shapes(), 0, "a 100 000-character text was cached");
    let mut most = 0;
    for i in 0..300usize {
        // Distinct 15 000-character texts: about 300 KB of glyphs each, 90 MB in all.
        let text = format!("{i:03}{}", &long[i..i + 15_000]);
        fm.measure(f, &text, 20.0);
        most = most.max(fm.cached_shape_bytes());
    }
    assert!(most <= 64 << 20, "{most} bytes cached");
    assert!(fm.cached_shapes() < 300);
}

/// Vietnamese letters Caladea lacks keep Cambria's advance when a fallback face draws
/// them: the base letter's, plus the horn for ơ / ư.
#[test]
fn letters_missing_from_caladea_keep_cambria_advances() {
    let mut fm = manager();
    let cam = fm.resolve("Cambria", false, false).unwrap();
    assert!(!fm.has_glyph(cam, 'ệ'), "Caladea lacks ệ");
    let e = fm.stand_in_advance(cam, 'ệ').unwrap();
    let base = fm.stand_in_advance(cam, 'e').unwrap();
    assert!(
        (e - base).abs() < 1e-4 && (base - 0.488).abs() < 0.002,
        "{e} {base}"
    );
    let horn = fm.stand_in_advance(cam, 'ợ').unwrap();
    assert!((horn - (0.5308 + 0.068)).abs() < 0.002, "{horn}");
    // Not a stand-in: no override.
    let times = fm.resolve("Times New Roman", false, false).unwrap();
    assert!(fm.stand_in_advance(times, 'ệ').is_none());
}
