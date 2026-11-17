//! Line boxes, paragraph spacing and justification as the reference desktop word processor
//! lays them out (measured black-box, docs/ui/REPORT.md "Fidelity round 1"): ECMA-376 Part 1
//! §17.3.1.33 spacing (`w:before`, `w:after`, `w:line`, `w:lineRule`), §17.18.44 `ST_Jc`
//! `both`, [MS-DOCX] §2.3.1 compatSetting `compatibilityMode`.
// Integration tests may unwrap.
#![allow(clippy::unwrap_used)]

use bo_layout::{
    layout_document, layout_document_with, LayoutCache, LayoutDocument, LayoutOptions, Line,
    LineSource, MarkFilter,
};
use bo_model::{
    Alignment, Anchor, Block, Border, Borders, Cell, CellProps, Columns, Document, Emu, HalfPoints,
    Image, LineSpacing, PageSetup, ParaProps, Paragraph, Run, RunContent, RunProps, Section,
    SectionBreak, Style, StyleKind, TabAlign, TabLeader, TabStop, Table, Twips, Wrap,
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

fn run_layout(d: &Document) -> LayoutDocument {
    run(d)
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

/// Number of words of `word` (repeated) on the first line of a justified paragraph whose
/// room is `overflow` points narrower than `n` of them.
fn justified_n(word: &str, n: usize, overflow: f32, compat: Option<u16>) -> usize {
    let m: f32 = 1593.0 / 2048.0 * 12.0;
    let natural = (n * word.len()) as f32 * m + (n - 1) as f32 * 3.0;
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
        vec![Run::text(
            vec![word; 2 * n + 2].join(" "),
            RunProps::default(),
        )],
    ))]);
    d.compat_mode = compat;
    let l = run(&d);
    let first = lines(&l, 0)[0];
    let text: String = first.fragments.iter().map(|f| f.text.as_str()).collect();
    text.split_whitespace().count()
}

/// Ten words "mmmm" in a line `overflow` points narrower than their natural width.
fn justified(overflow: f32, compat: Option<u16>) -> usize {
    justified_n("mmmm", 10, overflow, compat)
}

#[test]
fn short_last_words_are_squeezed_less() {
    // Sixteen one-letter words: the spaces could give 11 pt, but the last word with its
    // space is only 12.3 pt wide — narrowing stops near a third of it.
    assert_eq!(justified_n("m", 16, 4.0, Some(15)), 16);
    assert_eq!(justified_n("m", 16, 4.6, Some(15)), 15);
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

/// §17.7.6: the paragraph properties of a table style apply to the paragraphs of its cells,
/// under the paragraph style and direct formatting (§17.7.2).
#[test]
fn table_style_paragraph_properties_apply_in_cells() {
    let mut t = Table::simple(1, 1, Twips(4000));
    t.props.style = Some("TableGrid".into());
    let plain = |text: &str| {
        Block::Paragraph(Paragraph::new(
            ParaProps::default(),
            vec![Run::text(text, RunProps::default())],
        ))
    };
    t.rows[0].cells[0] = Cell {
        props: CellProps::default(),
        blocks: vec![plain("one"), plain("two")],
    };
    let mut d = doc(vec![Block::Table(t)]);
    // Document defaults: 8 pt after, 1.08 lines.
    d.default_para.space_after = Some(Twips(160));
    d.default_para.line_spacing = Some(LineSpacing::Auto(259));
    let mut grid = Style::new("TableGrid", "Table Grid", StyleKind::Table);
    grid.ppr.space_after = Some(Twips(0));
    grid.ppr.line_spacing = Some(SINGLE);
    d.styles.styles.push(grid);
    let l = run(&d);
    let (a, b) = (lines(&l, 0)[0], lines(&l, 1)[0]);
    // Single lines, no space after: the second line follows right below the first.
    assert!(
        near(b.top - a.top, 2355.0 * 12.0 / 2048.0),
        "{}",
        b.top - a.top
    );
}

/// A single-line row of a table with 0.5 pt borders is its line plus the top border; the
/// last row also holds the bottom border (§17.4.38 tcBorders, measured black-box).
#[test]
fn table_rows_hold_their_borders() {
    let mut t = Table::simple(3, 1, Twips(4000));
    let b = Some(Border::single(4));
    t.props.borders = Some(Borders {
        top: b.clone(),
        left: b.clone(),
        bottom: b.clone(),
        right: b.clone(),
        inside_h: b.clone(),
        inside_v: b,
    });
    for (i, row) in t.rows.iter_mut().enumerate() {
        row.cells[0].blocks = vec![para(&format!("row {i}"), 0, 0, SINGLE)];
    }
    let l = run(&doc(vec![Block::Table(t), para("after", 0, 0, SINGLE)]));
    let line = 2355.0 * 12.0 / 2048.0;
    let (r0, r1, r2) = (lines(&l, 0)[0], lines(&l, 1)[0], lines(&l, 2)[0]);
    assert!(near(r0.top, 72.0 + 0.5), "{}", r0.top);
    assert!(near(r1.top - r0.top, line + 0.5));
    assert!(near(r2.top - r1.top, line + 0.5));
    // The paragraph after the table starts below the bottom border.
    let after = lines(&l, 3)[0].top - r2.top;
    assert!(near(after, line + 0.5), "{after}");
}

/// §17.10.6 titlePg: a tall first-page header pushes only the first page's body down.
#[test]
fn a_tall_first_page_header_moves_only_the_first_page() {
    let mut blocks = Vec::new();
    for i in 0..80 {
        blocks.push(para(&format!("line {i}"), 0, 0, SINGLE));
    }
    let mut d = doc(blocks);
    let s = &mut d.sections[0];
    s.title_page = true;
    s.headers.default = Some(vec![para("small", 0, 0, SINGLE)]);
    s.headers.first = Some(vec![para("tall", 0, 0, LineSpacing::Exact(Twips(1600)))]);
    let l = run(&d);
    let first_top = |page: usize| {
        l.pages[page]
            .lines
            .iter()
            .filter(|x| matches!(x.source, LineSource::Body(_)))
            .map(|x| x.top)
            .fold(f32::MAX, f32::min)
    };
    // Header at 36 pt: the first page's 80 pt header ends at 116 pt; the others at 72 pt.
    assert!(near(first_top(0), 116.0), "{}", first_top(0));
    assert!(near(first_top(1), 72.0), "{}", first_top(1));
}

/// §20.4.2.19 wrapTopAndBottom: a line spaced 1.5 lines stays above a picture 14 pt below
/// the paragraph top — its extra spacing may reach into the picture, its text may not.
#[test]
fn line_spacing_may_reach_into_a_top_and_bottom_picture() {
    let place = |offset_pt: i64, line: LineSpacing| {
        let anchor = Anchor {
            h_relative: "column".into(),
            v_relative: "paragraph".into(),
            x: Emu(0),
            y: Emu(offset_pt * 12_700),
            wrap: Wrap::TopAndBottom,
            ..Default::default()
        };
        let pic = RunContent::Image(Box::new(Image {
            media: "/word/media/image1.png".into(),
            width: Emu(914_400),
            height: Emu(228_600),
            anchor: Some(anchor),
            crop: None,
            rotation: 0,
            description: String::new(),
            shape: None,
            format: None,
            src: None,
            ole: None,
        }));
        let mut run = Run::text("text ".repeat(60), RunProps::default());
        run.content.insert(0, pic);
        let d = doc(vec![Block::Paragraph(Paragraph::new(
            ParaProps {
                space_before: Some(Twips(0)),
                space_after: Some(Twips(0)),
                line_spacing: Some(line),
                ..ParaProps::default()
            },
            vec![run],
        ))]);
        let l = run_layout(&d);
        lines(&l, 0)[0].top
    };
    // Times 12 pt: 13.80 pt of text; 1.5 lines are 20.7 pt high.
    assert!(near(place(14, LineSpacing::Auto(360)), 72.0));
    assert!(place(13, LineSpacing::Auto(360)) > 72.0 + 13.0);
    // An exact line needs its whole height.
    assert!(place(14, LineSpacing::Exact(Twips(400))) > 72.0 + 14.0);
}

/// §17.18.77 continuous: a two-column section ended by a continuous section break on its
/// page has columns of (nearly) equal height, as the reference lays it out.
#[test]
fn columns_before_a_continuous_break_are_balanced() {
    let blocks: Vec<Block> = (0..10)
        .map(|i| para(&format!("line {i}"), 0, 0, SINGLE))
        .collect();
    let mut d = doc(blocks);
    d.sections[0].columns = Columns {
        count: 2,
        space: Twips(720),
        ..Columns::default()
    };
    let mut after = Section {
        page: a4(),
        ..Section::new(vec![para("after", 0, 0, SINGLE)])
    };
    after.break_type = SectionBreak::Continuous;
    d.sections.push(after);
    let l = run(&d);
    let line = 2355.0 * 12.0 / 2048.0;
    let tops: Vec<(f32, f32)> = (0..10)
        .map(|i| {
            let x = lines(&l, i)[0].fragments[0].x;
            (x, lines(&l, i)[0].top)
        })
        .collect();
    // Five lines per column; the next section starts below them.
    assert!(tops[..5].iter().all(|t| t.0 < 200.0), "{tops:?}");
    assert!(tops[5..].iter().all(|t| t.0 > 200.0), "{tops:?}");
    assert!(near(tops[5].1, 72.0));
    assert!(near(lines(&l, 10)[0].top, 72.0 + 5.0 * line));
}

/// §17.3.1.37 tab: a tab to a stop without leader draws no leader even when a later stop
/// has one.
#[test]
fn only_the_leadered_stop_gets_a_leader() {
    let d = doc(vec![Block::Paragraph(Paragraph::new(
        ParaProps {
            space_before: Some(Twips(0)),
            space_after: Some(Twips(0)),
            line_spacing: Some(SINGLE),
            tabs: Some(vec![
                TabStop {
                    pos: Twips(4680),
                    align: TabAlign::Center,
                    leader: TabLeader::None,
                },
                TabStop {
                    pos: Twips(9000),
                    align: TabAlign::Right,
                    leader: TabLeader::Dot,
                },
            ]),
            ..ParaProps::default()
        },
        vec![Run::text("a\tb\tc", RunProps::default())],
    ))]);
    let l = run(&d);
    let line = lines(&l, 0)[0];
    let dots: Vec<f32> = line
        .fragments
        .iter()
        .filter(|f| f.text.starts_with('.'))
        .map(|f| f.x)
        .collect();
    // One leader, after "b" (centred at 234 pt from the margin).
    assert_eq!(dots.len(), 1, "{dots:?}");
    assert!(dots[0] > 72.0 + 234.0, "{dots:?}");
}

/// §17.4.53 tblLayout autofit with cells of no preferred width: columns take their
/// content's width — a short word is not broken, a long text gets the rest of the line.
#[test]
fn autofit_columns_follow_their_content() {
    let mut t = Table::simple(2, 2, Twips(2000));
    let text = "Layout engines must agree on where every line breaks, because a single word                 moved to the next line can push a heading onto another page.";
    for row in &mut t.rows {
        row.cells[0].blocks = vec![para("short", 0, 0, SINGLE)];
        row.cells[0].props.width = None;
        row.cells[1].blocks = vec![para(text, 0, 0, SINGLE)];
        row.cells[1].props.width = None;
    }
    let l = run(&doc(vec![Block::Table(t)]));
    // "short" stays on one line; the long text starts right after the narrow column.
    assert_eq!(lines(&l, 0).len(), 1);
    let x_long = lines(&l, 1)[0].fragments[0].x;
    assert!(x_long < 72.0 + 60.0, "{x_long}");
    // The long cell wraps inside the text width.
    let right = lines(&l, 1)
        .iter()
        .flat_map(|ln| &ln.fragments)
        .map(|f| f.x + f.width)
        .fold(0.0f32, f32::max);
    assert!(right <= 72.0 + 451.3 + 3.0, "{right}");
}

/// Line height follows the run's font even where a fallback face supplies glyphs.
#[test]
fn a_cambria_line_keeps_cambria_height() {
    let d = {
        let mut d = doc(vec![para(
            "Việc chuẩn hoá phông chữ, ạ ặ ẫ ỵ",
            0,
            0,
            SINGLE,
        )]);
        d.default_run.font = Some("Cambria".into());
        d
    };
    let l = run(&d);
    let h = lines(&l, 0)[0].height;
    assert!((h - 1.1724 * 12.0).abs() < 0.05, "{h}");
}

/// §17.3.3.29 softHyphen: invisible and without width normally; with the formatting
/// marks shown it takes the width of its "¬" mark, as in the reference.
#[test]
fn an_optional_hyphen_shows_with_the_formatting_marks() {
    let d = doc(vec![para("optional\u{00AD}hyphen", 0, 0, SINGLE)]);
    let width = |show: bool| {
        let opts = LayoutOptions {
            marks: MarkFilter {
                show_hidden: show,
                ..MarkFilter::default()
            },
            ..LayoutOptions::default()
        };
        let l = layout_document_with(&d, &mut fonts(), &mut LayoutCache::new(), opts).unwrap();
        let line = lines(&l, 0)[0];
        let text: String = line.fragments.iter().map(|f| f.text.as_str()).collect();
        let w = line
            .fragments
            .iter()
            .map(|f| f.x + f.width)
            .fold(0.0f32, f32::max);
        (text, w)
    };
    let (hidden, w0) = width(false);
    let (shown, w1) = width(true);
    assert!(!hidden.contains('\u{00AC}'), "{hidden}");
    assert!(shown.contains('\u{00AC}'), "{shown}");
    assert!(w1 > w0 + 3.0, "{w0} {w1}");
}

/// §17.11.1: a footnote that fits on a page stays whole with its reference; the body
/// lines after the reference go to the next page instead (measured black-box).
#[test]
fn a_long_footnote_stays_whole_with_its_reference() {
    use bo_model::RunContent;
    let note: Vec<Block> = (0..33)
        .map(|i| para(&format!("note paragraph {i}"), 0, 0, SINGLE))
        .collect();
    let mut blocks = Vec::new();
    // 15 + 1 lines and the 33-line note fill 688 of the page's 698 pt.
    for i in 0..15 {
        blocks.push(para(&format!("before {i}"), 0, 0, SINGLE));
    }
    let mut r = Run::text("reference", RunProps::default());
    r.content.push(RunContent::FootnoteRef(1));
    blocks.push(Block::Paragraph(Paragraph::new(
        ParaProps {
            space_before: Some(Twips(0)),
            space_after: Some(Twips(0)),
            line_spacing: Some(SINGLE),
            ..ParaProps::default()
        },
        vec![r],
    )));
    for i in 0..30 {
        blocks.push(para(&format!("after {i}"), 0, 0, SINGLE));
    }
    let mut d = doc(blocks);
    d.footnotes.insert(1, note);
    let l = run(&d);
    // Page 1 holds the reference and all 33 note lines; no "after" line follows it there.
    let p0 = l.page_text(0);
    assert!(p0.contains("reference"), "{p0}");
    assert!(p0.contains("note paragraph 32"), "{p0}");
    assert!(!p0.contains("after 0"), "{p0}");
}
