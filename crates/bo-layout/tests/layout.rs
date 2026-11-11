//! Layout behaviour tests using the bundled fonts.
// Integration tests may unwrap.
#![allow(clippy::unwrap_used)]

use bo_layout::{layout_document, LayoutCache, LayoutDocument};
use bo_model::{
    Alignment, Block, Document, HalfPoints, LineSpacing, PageSetup, ParaProps, Paragraph, Run,
    RunContent, RunProps, Section, Twips,
};
use bo_text::FontManager;
use proptest::prelude::*;

fn fonts() -> FontManager {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/fonts");
    let mut fm = FontManager::new();
    fm.add_fonts_from_dir(&dir);
    fm
}

fn para(text: &str, props: ParaProps) -> Block {
    Block::Paragraph(Paragraph::new(
        props,
        vec![Run::text(text, RunProps::default())],
    ))
}

fn doc(blocks: Vec<Block>) -> Document {
    Document {
        default_run: RunProps {
            font: Some("Times New Roman".into()),
            size: Some(HalfPoints(26)),
            ..Default::default()
        },
        sections: vec![Section::new(blocks)],
        ..Default::default()
    }
}

fn run(d: &Document) -> LayoutDocument {
    layout_document(d, &mut fonts(), &mut LayoutCache::new()).unwrap()
}

const LOREM: &str = "Căn cứ Luật Tổ chức Chính phủ ngày 19 tháng 6 năm 2015; Căn cứ Luật sửa đổi, bổ sung một số điều của Luật Tổ chức Chính phủ và Luật Tổ chức chính quyền địa phương ngày 22 tháng 11 năm 2019; Theo đề nghị của Bộ trưởng Bộ Nội vụ; Chính phủ ban hành Nghị định về công tác văn thư.";

#[test]
fn lines_fit_in_content_box() {
    let d = doc(vec![para(LOREM, ParaProps::default())]);
    let l = run(&d);
    let setup = PageSetup::default();
    let left = setup.margin_left.to_pt();
    let right = setup.width.to_pt() - setup.margin_right.to_pt();
    let page = &l.pages[0];
    assert!(
        page.lines.len() >= 3,
        "expected wrapping, got {}",
        page.lines.len()
    );
    for line in &page.lines {
        for f in &line.fragments {
            assert!(f.x >= left - 0.01);
            // Trailing spaces may hang past the margin (one space ≈ 3.3 pt at 13 pt).
            let hang = if f.text.ends_with(' ') { 4.0 } else { 0.5 };
            assert!(
                f.x + f.width <= right + hang,
                "overflow {} > {right}",
                f.x + f.width
            );
        }
    }
    // No text lost.
    let joined: String = page
        .lines
        .iter()
        .flat_map(|l| l.fragments.iter().map(|f| f.text.to_string()))
        .collect();
    assert_eq!(joined, LOREM);
}

#[test]
fn alignment_positions() {
    let setup = PageSetup::default();
    let left = setup.margin_left.to_pt();
    let right = setup.width.to_pt() - setup.margin_right.to_pt();
    let mk = |a| {
        doc(vec![para(
            "Quốc hiệu",
            ParaProps {
                align: Some(a),
                ..Default::default()
            },
        )])
    };
    let l = run(&mk(Alignment::Left));
    let f = &l.pages[0].lines[0].fragments[0];
    assert!((f.x - left).abs() < 0.01);
    let r = run(&mk(Alignment::Right));
    let f = &r.pages[0].lines[0].fragments[0];
    assert!((f.x + f.width - right).abs() < 0.05);
    let c = run(&mk(Alignment::Center));
    let f = &c.pages[0].lines[0].fragments[0];
    assert!(((f.x + f.width / 2.0) - f32::midpoint(left, right)).abs() < 0.05);
}

#[test]
fn justify_fills_all_but_last_line() {
    let d = doc(vec![para(
        LOREM,
        ParaProps {
            align: Some(Alignment::Justify),
            ..Default::default()
        },
    )]);
    let l = run(&d);
    let setup = PageSetup::default();
    let right = setup.width.to_pt() - setup.margin_right.to_pt();
    let lines = &l.pages[0].lines;
    for line in &lines[..lines.len() - 1] {
        let last = line.fragments.last().unwrap();
        // Last glyph's right edge reaches the margin (trailing space excluded).
        let trimmed = last.text.trim_end();
        assert_ne!(trimmed, "");
        let end = last.x + last.width;
        assert!(end >= right - 6.0, "line ends at {end}, margin {right}");
    }
    let last = lines.last().unwrap().fragments.last().unwrap();
    assert!(last.x + last.width < right - 1.0);
}

#[test]
fn indents_and_hanging() {
    let setup = PageSetup::default();
    let left = setup.margin_left.to_pt();
    let d = doc(vec![para(
        LOREM,
        ParaProps {
            indent_left: Some(Twips(720)),
            indent_first_line: Some(Twips(-360)),
            ..Default::default()
        },
    )]);
    let l = run(&d);
    let lines = &l.pages[0].lines;
    assert!((lines[0].fragments[0].x - (left + 18.0)).abs() < 0.01);
    assert!((lines[1].fragments[0].x - (left + 36.0)).abs() < 0.01);
}

#[test]
fn line_spacing_rules() {
    let h = |ls| {
        let d = doc(vec![para(
            LOREM,
            ParaProps {
                line_spacing: Some(ls),
                ..Default::default()
            },
        )]);
        let l = run(&d);
        let lines = &l.pages[0].lines;
        lines[1].top - lines[0].top
    };
    let single = h(LineSpacing::Auto(240));
    let double = h(LineSpacing::Auto(480));
    assert!((double - 2.0 * single).abs() < 0.01);
    assert!((h(LineSpacing::Exact(Twips(400))) - 20.0).abs() < 0.01);
    assert!((h(LineSpacing::AtLeast(Twips(100))) - single).abs() < 0.01);
    // 13pt Tinos single spacing ≈ 1.15 em.
    assert!(single > 13.0 && single < 16.5, "{single}");
}

#[test]
fn pagination_and_breaks() {
    let many: Vec<Block> = (0..120)
        .map(|i| para(&format!("Đoạn {i}"), ParaProps::default()))
        .collect();
    let l = run(&doc(many));
    assert!(l.pages.len() >= 2);
    let setup = PageSetup::default();
    let bottom = setup.height.to_pt() - setup.margin_bottom.to_pt();
    for p in &l.pages {
        for line in &p.lines {
            assert!(line.top + line.height <= bottom + 0.01);
        }
    }
    // Manual page break and pageBreakBefore.
    let d = doc(vec![
        Block::Paragraph(Paragraph::new(
            ParaProps::default(),
            vec![Run {
                props: RunProps::default(),
                content: vec![
                    RunContent::Text("A".into()),
                    RunContent::PageBreak,
                    RunContent::Text("B".into()),
                ],
            }],
        )),
        para(
            "C",
            ParaProps {
                page_break_before: Some(true),
                ..Default::default()
            },
        ),
    ]);
    let l = run(&d);
    assert_eq!(l.pages.len(), 3);
    assert_eq!(l.page_text(0), "A");
    assert_eq!(l.page_text(1), "B");
    assert_eq!(l.page_text(2), "C");
}

#[test]
fn overlong_word_is_broken() {
    let word = "x".repeat(400);
    let l = run(&doc(vec![para(&word, ParaProps::default())]));
    assert!(l.pages[0].lines.len() > 1);
    let joined: String = l.pages[0]
        .lines
        .iter()
        .flat_map(|l| l.fragments.iter().map(|f| f.text.to_string()))
        .collect();
    assert_eq!(joined, word);
}

#[test]
fn empty_paragraph_takes_a_line() {
    let l = run(&doc(vec![
        para("", ParaProps::default()),
        para("x", ParaProps::default()),
    ]));
    let lines = &l.pages[0].lines;
    assert_eq!(lines.len(), 2);
    assert!(lines[1].top > lines[0].top);
}

#[test]
fn cache_hits_on_relayout() {
    let d = doc(vec![
        para(LOREM, ParaProps::default()),
        para("hai", ParaProps::default()),
    ]);
    let mut fm = fonts();
    let mut cache = LayoutCache::new();
    layout_document(&d, &mut fm, &mut cache).unwrap();
    layout_document(&d, &mut fm, &mut cache).unwrap();
    assert_eq!(cache.stats(), (2, 2));
}

#[test]
fn snapshot_sample_layout() {
    let d = doc(vec![
        para(
            "CỘNG HÒA XÃ HỘI CHỦ NGHĨA VIỆT NAM",
            ParaProps {
                align: Some(Alignment::Center),
                ..Default::default()
            },
        ),
        para(
            "Độc lập - Tự do - Hạnh phúc",
            ParaProps {
                align: Some(Alignment::Center),
                ..Default::default()
            },
        ),
        para(
            LOREM,
            ParaProps {
                align: Some(Alignment::Justify),
                indent_first_line: Some(Twips(567)),
                ..Default::default()
            },
        ),
    ]);
    let l = run(&d);
    let summary: Vec<(String, i32, i32)> = l.pages[0]
        .lines
        .iter()
        .map(|line| {
            (
                line.fragments
                    .iter()
                    .map(|f| f.text.as_str())
                    .collect::<String>(),
                (line.top * 10.0).round() as i32,
                (line.fragments[0].x * 10.0).round() as i32,
            )
        })
        .collect();
    insta::assert_json_snapshot!(summary);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(48))]
    #[test]
    fn no_text_lost(text in "[a-zA-Zàáạảãăắâấđêếôốơớưứ ,.\\-]{0,300}", width_twips in 1000i32..12000) {
        let mut d = doc(vec![para(&text, ParaProps { align: Some(Alignment::Justify), ..Default::default() })]);
        d.sections[0].page.width = Twips(width_twips + 2551);
        let l = run(&d);
        let joined: String = l.pages.iter().flat_map(|p| p.lines.iter().flat_map(|l| l.fragments.iter().map(|f| f.text.to_string()))).collect();
        prop_assert_eq!(joined, bo_text::nfc(&text));
    }
}

/// Found by the fuzz soak (BO_FUZZ_SEED=3c6ef372f398f82b, `cite-02-ieee-numeric.docx` with a
/// huge default font): a "word" that is one grapheme of several characters — a Vietnamese
/// letter written with combining marks — and wider than the line cannot be divided any
/// further. The line breaker used to queue it again and again, for ever.
#[test]
fn an_indivisible_grapheme_wider_than_the_line_does_not_hang() {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        // "ệ" as e + combining circumflex + combining dot below, then precomposed text.
        let text = "e\u{302}\u{323} Vie\u{302}\u{323}t Nam ệ";
        let mut d = doc(vec![para(text, ParaProps::default())]);
        // 1638 pt, the largest size the format allows: every letter is wider than the page.
        d.default_run.size = Some(HalfPoints(3276));
        let l = run(&d);
        let _ = tx.send(l.pages.len());
    });
    let pages = rx
        .recv_timeout(std::time::Duration::from_secs(60))
        .expect("layout of an indivisible wide grapheme must finish");
    assert!(pages >= 1);
}
