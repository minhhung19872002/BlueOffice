//! DOC-FMT-001 character properties in the layout: OpenType typography passed to the
//! shaper ([MS-DOCX] w14:ligatures), phonetic guides (ECMA-376 Part 1, §17.3.3.25 ruby),
//! two lines in one (§17.3.2.10 combine), position (§17.3.2.24), horizontal scale
//! (§17.3.2.43 w), character border (§17.3.2.4 bdr) and effects carried to rendering.
// Integration tests may unwrap.
#![allow(clippy::unwrap_used)]

use bo_layout::{layout_document, LayoutCache, LayoutDocument, Line, PageItem};
use bo_model::run_fx::{EastAsianLayout, TextEffects};
use bo_model::{
    Block, Border, Document, HalfPoints, ParaProps, Paragraph, Raw, Run, RunContent, RunProps,
    Section,
};
use bo_text::{FaceId, FontManager};

fn fonts() -> FontManager {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/fonts");
    let mut fm = FontManager::new();
    fm.add_fonts_from_dir(&dir);
    fm
}

fn doc(runs: Vec<Run>) -> Document {
    Document {
        default_run: RunProps {
            font: Some("Times New Roman".into()),
            size: Some(HalfPoints(24)),
            ..Default::default()
        },
        sections: vec![Section::new(vec![Block::Paragraph(Paragraph::new(
            ParaProps::default(),
            runs,
        ))])],
        ..Default::default()
    }
}

fn layout(d: &Document, fm: &mut FontManager) -> LayoutDocument {
    layout_document(d, fm, &mut LayoutCache::new()).unwrap()
}

fn line_text(l: &Line) -> String {
    l.fragments.iter().map(|f| f.text.as_str()).collect()
}

/// The 'liga' feature is present in the font's GSUB feature list (OpenType spec, GSUB
/// table: FeatureList records).
fn has_liga(fm: &FontManager, face: FaceId) -> bool {
    let f = fm.face(face).unwrap();
    let gsub = f.tables().gsub.unwrap();
    gsub.features
        .into_iter()
        .any(|feat| feat.tag == bo_text::ttf::Tag::from_bytes(b"liga"))
}

#[test]
fn ligatures_reach_the_shaper() {
    let mut fm = fonts();
    let carlito = fm.resolve("Carlito", false, false).unwrap();
    assert_eq!(fm.info(carlito).family, "Carlito");
    assert!(has_liga(&fm, carlito), "Carlito has standard ligatures");
    // "ffi" is three characters: three glyphs without ligatures, fewer with them (the
    // f + f + i sequence is covered by the font's ligature lookups).
    let off = fm.shape_features(carlito, "ffi", true, &[(*b"liga", 0)]);
    let on = fm.shape_features(carlito, "ffi", true, &[(*b"liga", 1)]);
    assert_eq!(off.len(), 3);
    assert!(on.len() < 3, "ligature glyph expected, got {}", on.len());
    // The same through layout: w14:ligatures none vs standard.
    let run = |lig: &str| {
        Run::text(
            "ffi",
            RunProps {
                font: Some("Carlito".into()),
                effects: Some(Box::new(TextEffects {
                    ligatures: Some(lig.into()),
                    ..TextEffects::default()
                })),
                ..RunProps::default()
            },
        )
    };
    let glyphs = |lig: &str, fm: &mut FontManager| {
        let l = layout(&doc(vec![run(lig)]), fm);
        l.pages[0].lines[0]
            .fragments
            .iter()
            .map(|f| f.glyphs.len())
            .sum::<usize>()
    };
    assert_eq!(glyphs("none", &mut fm), 3);
    assert_eq!(glyphs("standard", &mut fm), on.len());
}

const RUBY: &str = r#"<w:ruby><w:rubyPr><w:rubyAlign w:val="center"/><w:hps w:val="10"/><w:hpsRaise w:val="20"/><w:hpsBaseText w:val="24"/><w:lid w:val="vi-VN"/></w:rubyPr><w:rt><w:r><w:rPr><w:sz w:val="10"/></w:rPr><w:t>abcdefghijkl</w:t></w:r></w:rt><w:rubyBase><w:r><w:rPr><w:sz w:val="24"/></w:rPr><w:t>AB</w:t></w:r></w:rubyBase></w:ruby>"#;

#[test]
fn ruby_guide_above_centred_base() {
    let mut fm = fonts();
    let d = doc(vec![
        Run::text("x", RunProps::default()),
        Run {
            props: RunProps::default(),
            content: vec![RunContent::Opaque(Raw::new(RUBY, 0))],
        },
        Run::text("y", RunProps::default()),
    ]);
    let l = layout(&d, &mut fm);
    let page = &l.pages[0];
    let face = fm.resolve("Times New Roman", false, false).unwrap();
    // Widths from the font: base "AB" at 12 pt, guide at 5 pt (hps 10 half-points).
    let base_w = fm.measure(face, "AB", 12.0);
    let guide_w = fm.measure(face, "abcdefghijkl", 5.0);
    assert!(guide_w > base_w, "test text: the guide is the wider one");
    let find = |t: &str| {
        page.lines
            .iter()
            .flat_map(|l| l.fragments.iter())
            .find(|f| f.text.as_str() == t)
            .unwrap()
            .clone()
    };
    let base = find("AB");
    let guide = find("abcdefghijkl");
    assert!((base.size - 12.0).abs() < 1e-4 && (guide.size - 5.0).abs() < 1e-4);
    assert!((base.width - base_w).abs() < 0.01 && (guide.width - guide_w).abs() < 0.01);
    // §17.3.3.12 hpsRaise 20 half-points: the guide baseline 10 pt above the base one.
    assert!(
        (base.y - guide.y - 10.0).abs() < 0.01,
        "{} {}",
        base.y,
        guide.y
    );
    // Centred on each other (the wider guide starts the box).
    let mid = |f: &bo_layout::Fragment| f.x + f.width / 2.0;
    assert!((mid(&base) - mid(&guide)).abs() < 0.01);
    // The body line: "x", the object (one caret unit), "y"; the base sits on its baseline.
    let body = &page.lines[0];
    assert_eq!(line_text(body), "xy");
    assert!((body.baseline - base.y).abs() < 0.01);
    let x_end = body.fragments[0].x + body.fragments[0].width;
    assert!((guide.x - x_end).abs() < 0.01, "box starts after x");
    assert!((body.fragments[1].x - (guide.x + guide_w)).abs() < 0.01);
    // The line is tall enough for the guide: ascent ≥ 10 pt + the guide's ascent.
    let info = fm.info(face);
    let guide_asc = f32::from(info.ascender) * 5.0 / f32::from(info.units_per_em);
    assert!(body.baseline - body.top >= 10.0 + guide_asc - 0.01);
}

#[test]
fn combine_sets_two_half_size_lines() {
    let mut fm = fonts();
    let d = doc(vec![Run::text(
        "ABCD",
        RunProps {
            east_asian: Some(EastAsianLayout {
                combine: Some(true),
                combine_brackets: Some("round".into()),
                ..EastAsianLayout::default()
            }),
            ..RunProps::default()
        },
    )]);
    let l = layout(&d, &mut fm);
    let frags: Vec<_> = l.pages[0]
        .lines
        .iter()
        .flat_map(|l| l.fragments.iter())
        .collect();
    let get = |t: &str| frags.iter().find(|f| f.text.as_str() == t).unwrap();
    let (top, bottom) = (get("AB"), get("CD"));
    assert!((top.size - 6.0).abs() < 1e-4 && (bottom.size - 6.0).abs() < 1e-4);
    let face = fm.resolve("Times New Roman", false, false).unwrap();
    let info = fm.info(face);
    let k = 12.0 / f32::from(info.units_per_em);
    let (a, dsc) = (f32::from(info.ascender) * k, -f32::from(info.descender) * k);
    let open = get("(");
    // Baselines: top half a/2 above the line baseline... (−a + a/2), bottom d/2 below.
    assert!((top.y - (open.y - a / 2.0)).abs() < 0.01);
    assert!((bottom.y - (open.y + dsc / 2.0)).abs() < 0.01);
    // The closing bracket follows the wider half.
    let close = get(")");
    let inner = top.width.max(bottom.width);
    assert!((close.x - (open.x + open.width + inner)).abs() < 0.01);
}

#[test]
fn position_scale_border_and_effects() {
    let mut fm = fonts();
    let raised = RunProps {
        position: Some(12), // 6 pt up
        ..RunProps::default()
    };
    let wide = RunProps {
        scale: Some(200),
        border: Some(Border::single(8)),
        outline: Some(true),
        ..RunProps::default()
    };
    let d = doc(vec![
        Run::text("a", RunProps::default()),
        Run::text("b", raised),
        Run::text("c", RunProps::default()),
        Run::text("WW", wide),
    ]);
    let l = layout(&d, &mut fm);
    let line = &l.pages[0].lines[0];
    let frag = |t: &str| line.fragments.iter().find(|f| f.text.contains(t)).unwrap();
    // §17.3.2.24: glyphs of "b" 6 pt above the baseline.
    let b = frag("b");
    assert!(b.glyphs.iter().any(|g| (g.y + 6.0).abs() < 1e-3));
    // §17.3.2.43 w 200 %: twice the advance of the plain text.
    let face = fm.resolve("Times New Roman", false, false).unwrap();
    let ww = frag("WW");
    assert!((ww.width - 2.0 * fm.measure(face, "WW", 12.0)).abs() < 0.05);
    let fx = ww.fx.as_ref().unwrap();
    assert!(fx.hollow && (fx.scale_x - 2.0).abs() < 1e-6);
    // §17.3.2.4 bdr: four 1 pt rules around the run.
    let rules: Vec<_> = l.pages[0]
        .items
        .iter()
        .filter(|i| matches!(i, PageItem::Rule { width, .. } if (*width - 1.0).abs() < 1e-6))
        .collect();
    assert_eq!(rules.len(), 4);
    if let PageItem::Rule { x1, x2, .. } = rules[0] {
        assert!((x1 - ww.x).abs() < 0.01 && (x2 - (ww.x + ww.width)).abs() < 0.01);
    }
}
