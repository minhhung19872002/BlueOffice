//! Line boxes, paragraph spacing and justification as the reference desktop word processor
//! lays them out (measured black-box, docs/ui/REPORT.md "Fidelity round 1"): ECMA-376 Part 1
//! §17.3.1.33 spacing (`w:before`, `w:after`, `w:line`, `w:lineRule`), §17.18.44 `ST_Jc`
//! `both`, [MS-DOCX] §2.3.1 compatSetting `compatibilityMode`.
// Integration tests may unwrap.
#![allow(clippy::unwrap_used)]

use bo_layout::{layout_document, LayoutCache, LayoutDocument, Line, LineSource};
use bo_model::{
    Alignment, Block, Document, HalfPoints, LineSpacing, PageSetup, ParaProps, Paragraph, Run,
    RunProps, Section, Twips,
};
use bo_text::FontManager;

fn fonts() -> FontManager {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/fonts");
    let mut fm = FontManager::new();
    fm.add_fonts_from_dir(&dir);
    fm
}

/// A4, 1 inch margins: the text is 451.28 pt wide from x = 72, from y = 72.
fn a4() -> PageSetup {
    PageSetup {
        width: Twips(11_906),
        height: Twips(16_838),
        margin_top: Twips(1440),
        margin_bottom: Twips(1440),
        margin_left: Twips(1440),
        margin_right: Twips(1440),
        header: Twips(720),
        footer: Twips(720),
    }
}

fn para(text: &str, before: i32, after: i32, line: LineSpacing) -> Block {
    Block::Paragraph(Paragraph::new(
        ParaProps {
            space_before: Some(Twips(before)),
            space_after: Some(Twips(after)),
            line_spacing: Some(line),
            ..ParaProps::default()
        },
        vec![Run::text(text, RunProps::default())],
    ))
}

/// Times New Roman 12 pt (Tinos: usWinAscent 1825, usWinDescent 443, hhea lineGap 87).
fn doc(blocks: Vec<Block>) -> Document {
    Document {
        default_run: RunProps {
            font: Some("Times New Roman".into()),
            size: Some(HalfPoints(24)),
            ..Default::default()
        },
        sections: vec![Section {
            page: a4(),
            ..Section::new(blocks)
        }],
        ..Default::default()
    }
}

fn run(d: &Document) -> LayoutDocument {
    layout_document(d, &mut fonts(), &mut LayoutCache::new()).unwrap()
}

fn lines(l: &LayoutDocument, para: u32) -> Vec<&Line> {
    l.pages
        .iter()
        .flat_map(|p| p.lines.iter())
        .filter(|x| x.source == LineSource::Body(para))
        .collect()
}

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.02
}

const SINGLE: LineSpacing = LineSpacing::Auto(240);

#[test]
fn spacing_between_paragraphs_is_the_larger_of_after_and_before() {
    for (after, before) in [(240, 120), (120, 240), (240, 240), (240, 0), (0, 240)] {
        let d = doc(vec![
            para("A", 0, after, LineSpacing::Exact(Twips(400))),
            para("B", before, 0, LineSpacing::Exact(Twips(400))),
        ]);
        let l = run(&d);
        let gap = lines(&l, 1)[0].top - (lines(&l, 0)[0].top + 20.0);
        assert!(near(gap, 12.0), "after {after} before {before}: gap {gap}");
    }
}

#[test]
fn space_before_counts_at_the_document_start_but_not_after_a_page_break() {
    let mut second = para("B", 480, 0, SINGLE);
    if let Block::Paragraph(p) = &mut second {
        p.props.page_break_before = Some(true);
    }
    let d = doc(vec![para("A", 480, 0, SINGLE), second]);
    let l = run(&d);
    assert!(near(lines(&l, 0)[0].top, 72.0 + 24.0));
    let b = lines(&l, 1)[0];
    assert!(near(b.top, 72.0), "{}", b.top);
}

#[test]
fn line_boxes_place_the_baseline_like_the_reference() {
    // Single: external leading (87 / 2048 em) above the ascent, 13.80 pt high.
    let d = doc(vec![
        para("Single", 0, 0, SINGLE),
        para("Double", 0, 0, LineSpacing::Auto(480)),
        para("Exact", 0, 0, LineSpacing::Exact(Twips(400))),
        para("AtLeast", 0, 0, LineSpacing::AtLeast(Twips(400))),
    ]);
    let l = run(&d);
    let em = 12.0 / 2048.0;
    let single = lines(&l, 0)[0];
    assert!(near(single.height, 2355.0 * em));
    assert!(near(single.baseline - single.top, (87.0 + 1825.0) * em));
    // A multiple adds its extra space below the text.
    let double = lines(&l, 1)[0];
    assert!(near(double.height, 2.0 * 2355.0 * em));
    assert!(near(double.baseline - double.top, (87.0 + 1825.0) * em));
    // Exact: the baseline at 80 % of the line.
    let exact = lines(&l, 2)[0];
    assert!(near(exact.baseline - exact.top, 16.0));
    // At least: the extra space above the text.
    let least = lines(&l, 3)[0];
    assert!(near(least.baseline - least.top, 20.0 - 443.0 * em));
}

/// Ten words "mmmm" in a line `overflow` points narrower than their natural width.
fn justified(overflow: f32, compat: Option<u16>) -> usize {
    let m: f32 = 1593.0 / 2048.0 * 12.0;
    let natural = 40.0 * m + 9.0 * 3.0;
    let right = (451.28 - (natural - overflow)) * 20.0;
    let mut d = doc(vec![Block::Paragraph(Paragraph::new(
        ParaProps {
            space_before: Some(Twips(0)),
            space_after: Some(Twips(0)),
            line_spacing: Some(SINGLE),
            align: Some(Alignment::Justify),
            indent_right: Some(Twips(right.round() as i32)),
            ..ParaProps::default()
        },
        vec![Run::text(["mmmm"; 22].join(" "), RunProps::default())],
    ))]);
    d.compat_mode = compat;
    let l = run(&d);
    let first = lines(&l, 0)[0];
    first
        .fragments
        .iter()
        .map(|f| f.text.matches("mmmm").count())
        .sum()
}

#[test]
fn justified_lines_narrow_their_spaces_to_keep_a_word_in_modern_layout() {
    // Up to a quarter of the 27 pt of spaces (6.75 pt) can be given up.
    assert_eq!(justified(6.5, Some(15)), 10);
    assert_eq!(justified(7.0, Some(15)), 9);
    // New documents lay out like compatibility mode 15.
    assert_eq!(justified(6.5, None), 10);
    // Older files (mode 14, or none named) never narrow the spaces.
    assert_eq!(justified(0.5, Some(14)), 9);
    assert_eq!(justified(0.5, Some(0)), 9);
}

#[test]
fn a_squeezed_line_fits_its_room() {
    let m: f32 = 1593.0 / 2048.0 * 12.0;
    let natural = 40.0 * m + 9.0 * 3.0;
    let d = {
        let right = (451.28 - (natural - 5.0)) * 20.0;
        doc(vec![Block::Paragraph(Paragraph::new(
            ParaProps {
                space_before: Some(Twips(0)),
                space_after: Some(Twips(0)),
                line_spacing: Some(SINGLE),
                align: Some(Alignment::Justify),
                indent_right: Some(Twips(right.round() as i32)),
                ..ParaProps::default()
            },
            vec![Run::text(["mmmm"; 22].join(" "), RunProps::default())],
        ))])
    };
    let l = run(&d);
    let first = lines(&l, 0)[0];
    // One fragment: the ten words, their nine narrowed spaces and the trailing space.
    let f = &first.fragments[0];
    assert_eq!(f.text.matches("mmmm").count(), 10);
    let end = f.x + f.width - 3.0;
    let room_end = 72.0 + natural - 5.0;
    assert!((end - room_end).abs() < 0.1, "{end} vs {room_end}");
}
