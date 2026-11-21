//! Line boxes, paragraph spacing and justification as the reference desktop word processor
//! lays them out (measured black-box, docs/ui/REPORT.md "Fidelity round 1"): ECMA-376 Part 1
//! §17.3.1.33 spacing (`w:before`, `w:after`, `w:line`, `w:lineRule`), §17.18.44 `ST_Jc`
//! `both`, [MS-DOCX] §2.3.1 compatSetting `compatibilityMode`.
// Integration tests may unwrap.
#![allow(clippy::unwrap_used)]

use bo_layout::{
    layout_document, layout_document_with, LayoutCache, LayoutDocument, LayoutOptions, Line,
    LineSource, MarkFilter, PageItem,
};
use bo_model::{
    AbstractNum, Alignment, Anchor, Block, Border, Borders, Cell, CellProps, ColumnDef, Columns,
    Document, Emu, HalfPoints, Image, Level, LineSpacing, Num, NumFormat, NumRef, Numbering,
    PageSetup, ParaProps, Paragraph, Run, RunContent, RunProps, Section, SectionBreak, Style,
    StyleKind, TabAlign, TabLeader, TabStop, Table, Twips, Wrap,
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

// ---------------------------------------------------------------- round 2

/// Calibri 11 pt paragraph made of `runs` (font, text), single lines, no spacing.
fn calibri_para(runs: &[(&str, &str)]) -> Block {
    Block::Paragraph(Paragraph::new(
        ParaProps {
            space_before: Some(Twips(0)),
            space_after: Some(Twips(0)),
            line_spacing: Some(SINGLE),
            ..ParaProps::default()
        },
        runs.iter()
            .map(|(f, t)| {
                Run::text(
                    *t,
                    RunProps {
                        font: Some((*f).into()),
                        size: Some(HalfPoints(22)),
                        ..Default::default()
                    },
                )
            })
            .collect(),
    ))
}

/// Calibri 11 pt single line: usWinAscent 1950 + usWinDescent 550 (2048 em), no leading.
const CALIBRI_ASCENT: f32 = 1950.0 * 11.0 / 2048.0;
const CALIBRI_LINE: f32 = 2500.0 * 11.0 / 2048.0;

/// The line top is the highest ascent + external leading of one run (measured black-box,
/// docs/ui/REPORT.md "Dàn trang vòng 2"): an 11 pt Times run (ascent + leading below
/// Calibri's ascent) leaves a Calibri line as it is; a Symbol run raises its top by
/// 0.59 pt; a Courier New run lowers its bottom by 0.35 pt — also where neither font is
/// installed and an unrelated face draws them.
#[test]
fn a_line_is_as_high_as_its_tallest_run() {
    let d = doc(vec![
        calibri_para(&[("Calibri", "plain "), ("Times New Roman", "times")]),
        calibri_para(&[("Calibri", "plain "), ("Symbol", "x")]),
        calibri_para(&[("Calibri", "plain "), ("Courier New", "x")]),
    ]);
    let l = run(&d);
    let times = lines(&l, 0)[0];
    assert!(near(times.height, CALIBRI_LINE), "{}", times.height);
    assert!(near(times.baseline - times.top, CALIBRI_ASCENT));
    let symbol = lines(&l, 1)[0];
    assert!(
        (symbol.baseline - symbol.top - 1.0054 * 11.0).abs() < 0.02,
        "{}",
        symbol.baseline - symbol.top
    );
    let courier = lines(&l, 2)[0];
    let below = courier.height - (courier.baseline - courier.top);
    assert!((below - 0.3003 * 11.0).abs() < 0.02, "{below}");
}

/// A list label's font raises the line top (a 20 pt Courier New "o" bullet) but its
/// descent does not lower the line bottom (measured black-box).
#[test]
fn a_list_label_raises_the_line_but_does_not_deepen_it() {
    let mut item = calibri_para(&[("Calibri", "item")]);
    if let Block::Paragraph(p) = &mut item {
        p.props.num = Some(NumRef { num_id: 1, ilvl: 0 });
    }
    let mut d = doc(vec![item]);
    d.numbering = Numbering {
        abstracts: vec![AbstractNum {
            id: 0,
            levels: vec![Level {
                ilvl: 0,
                start: 1,
                format: NumFormat::Bullet,
                text: "o".into(),
                align: Alignment::Left,
                ppr: ParaProps::default(),
                rpr: RunProps {
                    font: Some("Courier New".into()),
                    size: Some(HalfPoints(40)),
                    ..Default::default()
                },
                restart: None,
                suffix: "tab".into(),
                pstyle: None,
                is_lgl: false,
            }],
        }],
        nums: vec![Num {
            id: 1,
            abstract_id: 0,
            start_overrides: Vec::new(),
        }],
    };
    let l = run(&d);
    let line = lines(&l, 0)[0];
    let above = line.baseline - line.top;
    assert!((above - 0.8325 * 20.0).abs() < 0.02, "{above}");
    let below = line.height - above;
    assert!(near(below, CALIBRI_LINE - CALIBRI_ASCENT), "{below}");
}

/// Widow control in table cells (§17.3.1.44): a row split at the page end does not
/// leave a paragraph's first line alone before the break — the row moves to the next
/// page whole (gen-07 of the fidelity corpus, measured black-box).
#[test]
fn a_row_split_keeps_widow_control() {
    let long = "Layout engines must agree on where every line breaks, because a single word \
                moved to the next line can push a heading onto another page. "
        .repeat(4);
    let filler = (0..49).map(|i| para(&format!("filler {i}"), 0, 0, SINGLE));
    let mut t = Table::simple(1, 2, Twips(4000));
    t.rows[0].cells[0].blocks = vec![para("10", 0, 0, SINGLE)];
    t.rows[0].cells[1].blocks = vec![para(&long, 0, 0, SINGLE)];
    let mut blocks: Vec<Block> = filler.collect();
    blocks.push(Block::Table(t));
    let l = run(&doc(blocks));
    let page_of = |para: u32| {
        l.pages
            .iter()
            .position(|p| p.lines.iter().any(|x| x.source == LineSource::Body(para)))
            .unwrap()
    };
    // 49 lines of 13.8 pt leave room for one line of the row on page 1: it moves.
    assert_eq!(page_of(48), 0);
    assert_eq!(page_of(49), 1, "the row's number cell");
    assert_eq!(page_of(50), 1, "the row's text cell");
}

/// Unequal columns before a continuous break (§17.6.3 col `w:w`): balanced by laying the
/// text out again (lines cannot move between columns of other widths), and a paragraph
/// continuing in a wider column takes that column's width (gen-06 of the fidelity
/// corpus, measured black-box).
#[test]
fn unequal_columns_balance_and_rewrap() {
    let text = "Layout engines must agree on where every line breaks, because a single word \
                moved to the next line can push a heading onto another page.";
    let mut blocks: Vec<Block> = (0..6).map(|_| para(text, 0, 240, SINGLE)).collect();
    // The empty paragraph that carries the section break takes no part in the balance.
    blocks.push(para("", 0, 240, SINGLE));
    let mut d = doc(blocks);
    let col = |w: i32, s: i32| ColumnDef {
        width: Twips(w),
        space: Twips(s),
    };
    d.sections[0].columns = Columns {
        count: 3,
        space: Twips(360),
        separator: false,
        cols: vec![col(2000, 360), col(3000, 360), col(3646, 0)],
    };
    let mut after = Section {
        page: a4(),
        ..Section::new(vec![para("after", 0, 0, SINGLE)])
    };
    after.break_type = SectionBreak::Continuous;
    d.sections.push(after);
    let l = run(&d);
    // Column left edges and widths (points).
    let cols = [(72.0, 100.0), (190.0, 150.0), (358.0, 182.3)];
    let mut bottoms = [0.0f32; 3];
    let mut widest = [0.0f32; 3];
    for i in 0..6 {
        for line in lines(&l, i) {
            let x0 = line.fragments.iter().map(|f| f.x).fold(f32::MAX, f32::min);
            let x1 = line
                .fragments
                .iter()
                .map(|f| f.x + f.width)
                .fold(0.0f32, f32::max);
            let c = cols.iter().rposition(|c| x0 >= c.0 - 1.0).unwrap();
            // A fragment's width includes the line's trailing space (3 pt).
            assert!(
                x1 <= cols[c].0 + cols[c].1 + 3.5,
                "line {i} beyond column {c}"
            );
            widest[c] = widest[c].max(x1 - cols[c].0);
            bottoms[c] = bottoms[c].max(line.top + line.height);
        }
    }
    // Lines in the wider columns use their width (not the first column's 100 pt).
    assert!(widest[1] > 110.0 && widest[2] > 160.0, "{widest:?}");
    // Balanced: the three columns end within two lines of each other.
    let line = 2355.0 * 12.0 / 2048.0;
    let (lo, hi) = (
        bottoms.iter().copied().fold(f32::MAX, f32::min),
        bottoms.iter().copied().fold(0.0f32, f32::max),
    );
    assert!(hi - lo <= 2.0 * line + 0.1, "{bottoms:?}");
    // The next section starts below the tallest column and the last paragraph's 12 pt
    // space after.
    let next = lines(&l, 7)[0].top;
    assert!(near(next, hi + 12.0), "{next} vs {hi}");
}

fn grid_borders() -> Borders {
    let b = Some(Border::single(4));
    Borders {
        top: b.clone(),
        left: b.clone(),
        bottom: b.clone(),
        right: b.clone(),
        inside_h: b.clone(),
        inside_v: b,
    }
}

/// §17.4.42 tblCellMar top / bottom: added to the row's borders (measured black-box:
/// 7.2 pt / 3.6 pt margins make every Table Grid row 0.5 + 10.8 pt taller than its line).
#[test]
fn cell_margins_top_and_bottom_add_to_the_row() {
    let mut t = Table::simple(3, 1, Twips(4000));
    t.props.borders = Some(grid_borders());
    t.props.cell_margin_vertical = Some((Twips(144), Twips(72)));
    for (i, row) in t.rows.iter_mut().enumerate() {
        row.cells[0].blocks = vec![para(&format!("row {i}"), 0, 0, SINGLE)];
    }
    let l = run(&doc(vec![Block::Table(t), para("after", 0, 0, SINGLE)]));
    let line = 2355.0 * 12.0 / 2048.0;
    let (r0, r1) = (lines(&l, 0)[0], lines(&l, 1)[0]);
    assert!(near(r0.top, 72.0 + 0.5 + 7.2), "{}", r0.top);
    assert!(
        near(r1.top - r0.top, line + 0.5 + 7.2 + 3.6),
        "{}",
        r1.top - r0.top
    );
    let after = lines(&l, 3)[0].top - lines(&l, 2)[0].top;
    assert!(near(after, line + 3.6 + 0.5), "{after}");
}

/// A row split at the page end draws its bottom border right below its last line on
/// that page (gen-07 of the fidelity corpus), not at the page bottom.
#[test]
fn a_split_row_ends_below_its_last_line() {
    let long = "Layout engines must agree on where every line breaks, because a single word \
                moved to the next line can push a heading onto another page. "
        .repeat(6);
    let mut t = Table::simple(1, 1, Twips(4000));
    t.props.borders = Some(grid_borders());
    t.rows[0].cells[0].blocks = vec![para(&long, 0, 0, SINGLE)];
    let mut blocks: Vec<Block> = (0..45)
        .map(|i| para(&format!("filler {i}"), 0, 0, SINGLE))
        .collect();
    blocks.push(Block::Table(t));
    let l = run(&doc(blocks));
    let page = &l.pages[0];
    let last = page
        .lines
        .iter()
        .filter(|x| x.source == LineSource::Body(45))
        .map(|x| x.top + x.height)
        .fold(0.0f32, f32::max);
    assert!(last > 0.0, "the row starts on page 1");
    let lowest_rule = page
        .items
        .iter()
        .filter_map(|it| match it {
            PageItem::Rule { y1, y2, .. } => Some(y1.max(*y2)),
            _ => None,
        })
        .fold(0.0f32, f32::max);
    assert!((lowest_rule - last).abs() < 1.0, "{lowest_rule} vs {last}");
}

/// §20.4.3.7 ST_WrapText `left`: no text right of the picture, also where another
/// picture already cuts the line (images-04 of the fidelity corpus, measured black-box).
#[test]
fn a_left_wrapped_picture_keeps_its_right_side_empty() {
    let pic = |x_pt: i64, h_pt: i64, side: bo_model::WrapText| {
        RunContent::Image(Box::new(Image {
            media: "/word/media/image1.png".into(),
            width: Emu(914_400),
            height: Emu(h_pt * 12_700),
            anchor: Some(Anchor {
                h_relative: "column".into(),
                v_relative: "paragraph".into(),
                x: Emu(x_pt * 12_700),
                y: Emu(0),
                wrap: Wrap::Square,
                wrap_text: side,
                ..Default::default()
            }),
            crop: None,
            rotation: 0,
            description: String::new(),
            shape: None,
            format: None,
            src: None,
            ole: None,
        }))
    };
    let para_with = |content: RunContent, text: &str| {
        let mut run = Run::text(text, RunProps::default());
        run.content.insert(0, content);
        Block::Paragraph(Paragraph::new(
            ParaProps {
                space_before: Some(Twips(0)),
                space_after: Some(Twips(0)),
                line_spacing: Some(SINGLE),
                ..ParaProps::default()
            },
            vec![run],
        ))
    };
    let d = doc(vec![
        para_with(pic(0, 72, bo_model::WrapText::BothSides), "Short."),
        para_with(
            pic(144, 54, bo_model::WrapText::Left),
            &"words beside the pictures ".repeat(20),
        ),
    ]);
    let l = run(&d);
    // Lines beside the second picture (54 pt from its paragraph's top) start left of it.
    let first = lines(&l, 1)[0].top;
    for line in lines(&l, 1).iter().filter(|x| x.top < first + 54.0 - 1.0) {
        for f in &line.fragments {
            assert!(
                f.x < 72.0 + 144.0,
                "text right of a left-wrapped picture at {}",
                f.x
            );
        }
    }
}

// ---------------------------------------------------------------- round 3

/// Calibri 11 pt paragraph of one line at `line` 240ths of a line, no spacing, with a
/// footnote reference to note 1 when `note`.
fn calibri_line(text: &str, line: u32, note: bool) -> Block {
    let mut r = Run::text(
        text,
        RunProps {
            font: Some("Calibri".into()),
            size: Some(HalfPoints(22)),
            ..Default::default()
        },
    );
    if note {
        r.content.push(RunContent::FootnoteRef(1));
    }
    Block::Paragraph(Paragraph::new(
        ParaProps {
            space_before: Some(Twips(0)),
            space_after: Some(Twips(0)),
            line_spacing: Some(LineSpacing::Auto(line)),
            ..ParaProps::default()
        },
        vec![r],
    ))
}

/// Page (0-based) of the first line containing `needle`.
fn page_of(l: &LayoutDocument, needle: &str) -> usize {
    (0..l.pages.len())
        .find(|&i| l.page_text(i).contains(needle))
        .unwrap()
}

/// `n` lines at `line` after a spacer that leaves the text of line n + 1 `d` points more
/// than it needs above the bottom margin; footnote referenced on line 3 when `note`.
fn bottom_case(line: u32, n: usize, d: f32, note: bool) -> LayoutDocument {
    let h = CALIBRI_LINE * line as f32 / 240.0;
    let spacer = 697.9 - n as f32 * h - CALIBRI_LINE - d;
    let mut blocks = vec![para(
        "spacer",
        0,
        0,
        LineSpacing::Exact(Twips((spacer * 20.0).round() as i32)),
    )];
    for k in 1..=n + 1 {
        blocks.push(calibri_line(&format!("row {k}."), line, note && k == 3));
    }
    let mut d = doc(blocks);
    if note {
        d.footnotes
            .insert(1, vec![para("Note of the test.", 0, 0, SINGLE)]);
    }
    run(&d)
}

/// The extra space of a multiple-spaced line may pass the bottom margin: only its text must
/// fit (measured black-box, docs/ui/REPORT.md "Dàn trang vòng 3", cal-31: line 26 of 1.5
/// lines fits 1 pt above the margin, not 1 pt below; 1.15 lines alike). With a footnote on
/// the page the whole line must fit above the note area (line 25 moves even 8 pt clear).
#[test]
fn multiple_spacing_passes_the_bottom_margin_unless_a_footnote_is_there() {
    for (line, n) in [(360, 25), (276, 34)] {
        let last = format!("row {}.", n + 1);
        assert_eq!(
            page_of(&bottom_case(line, n, 1.0, false), &last),
            0,
            "{line}"
        );
        assert_eq!(
            page_of(&bottom_case(line, n, -1.0, false), &last),
            1,
            "{line}"
        );
        let with_note = bottom_case(line, n, 8.0, true);
        assert_eq!(page_of(&with_note, &format!("row {n}.")), 1, "{line}");
    }
}

/// §17.3.2.42 vertAlign: 0.65 × the size rounded to the nearest half point, superscript
/// raised 0.34 em, subscript lowered 0.085 em (measured black-box, cal-30).
#[test]
fn super_and_subscripts_round_their_size_to_half_points() {
    for (hp, want) in [
        (16u32, 5.0f32),
        (22, 7.0),
        (24, 8.0),
        (26, 8.5),
        (28, 9.0),
        (48, 15.5),
    ] {
        let base = RunProps {
            size: Some(HalfPoints(hp)),
            ..Default::default()
        };
        let sup = RunProps {
            vert_align: Some(bo_model::VertAlign::Superscript),
            ..base.clone()
        };
        let sub = RunProps {
            vert_align: Some(bo_model::VertAlign::Subscript),
            ..base.clone()
        };
        let d = doc(vec![Block::Paragraph(Paragraph::new(
            ParaProps::default(),
            vec![
                Run::text("X", base),
                Run::text("S", sup),
                Run::text("B", sub),
            ],
        ))]);
        let l = run(&d);
        let line = lines(&l, 0)[0];
        let size = hp as f32 / 2.0;
        let frag = |t: &str| line.fragments.iter().find(|f| &*f.text == t).unwrap();
        // Baseline shifts are glyph offsets from the line's baseline.
        let (x, s, b) = (frag("X"), frag("S"), frag("B"));
        let rise = |f: &bo_layout::Fragment| f.y - f.glyphs[0].y;
        assert!(
            near(s.size, want) && near(b.size, want),
            "{hp}: {} {}",
            s.size,
            b.size
        );
        let (up, down) = (rise(s) - rise(x), rise(x) - rise(b));
        assert!((up - 0.34 * size).abs() < 0.05, "{hp}: sup {up}");
        assert!((down - 0.085 * size).abs() < 0.05, "{hp}: sub {down}");
    }
}
/// The empty paragraph that only carries a section break before a continuous section takes
/// no room: the next section's line follows the last text line, and the spacing between
/// those two paragraphs collapses as if the mark were not there (measured black-box,
/// docs/ui/REPORT.md "Dàn trang vòng 3", cal-32).
#[test]
fn an_empty_section_mark_before_a_continuous_section_takes_no_room() {
    let line = 2355.0 * 12.0 / 2048.0;
    for after in [0, 240] {
        let mut d = doc(vec![
            para("first", 0, after, SINGLE),
            para("", 0, after, SINGLE),
        ]);
        let mut next = Section {
            page: a4(),
            ..Section::new(vec![para("next", 0, after, SINGLE)])
        };
        next.break_type = SectionBreak::Continuous;
        d.sections.push(next);
        let l = run(&d);
        let gap = lines(&l, 2)[0].top - lines(&l, 0)[0].top;
        assert!(
            near(gap, line + after as f32 / 20.0),
            "after {after}: {gap}"
        );
    }
}

/// A multiple of single spacing scales the text's line, not an inline picture: a 54 pt
/// picture at 1.15 / 1.5 lines adds 0.15 / 0.5 of the font's line (measured black-box,
/// cal-33, in the body and in table cells).
#[test]
fn line_spacing_multiples_do_not_scale_inline_pictures() {
    let pic = || {
        RunContent::Image(Box::new(Image {
            media: "/word/media/image1.png".into(),
            width: Emu(914_400),
            height: Emu(54 * 12_700),
            anchor: None,
            crop: None,
            rotation: 0,
            description: String::new(),
            shape: None,
            format: None,
            src: None,
            ole: None,
        }))
    };
    let height = |line: u32| {
        let mut r = Run::text("P", RunProps::default());
        r.content.push(pic());
        let d = doc(vec![
            Block::Paragraph(Paragraph::new(
                ParaProps {
                    space_before: Some(Twips(0)),
                    space_after: Some(Twips(0)),
                    line_spacing: Some(LineSpacing::Auto(line)),
                    ..ParaProps::default()
                },
                vec![r],
            )),
            para("after", 0, 0, SINGLE),
        ]);
        let l = run(&d);
        lines(&l, 1)[0].top - lines(&l, 0)[0].top
    };
    let text = 2355.0 * 12.0 / 2048.0;
    let single = height(240);
    assert!(single > 54.0, "{single}");
    for (line, k) in [(276, 0.15), (360, 0.5), (480, 1.0)] {
        let h = height(line);
        assert!(near(h, single + k * text), "{line}: {h} vs {single}");
    }
}

/// §17.4.84 vAlign center in a vertically merged cell (§17.4.85 vMerge) centres the
/// content in the whole merged cell (measured black-box, office-17 header rows).
#[test]
fn a_merged_cell_centres_its_content_over_all_its_rows() {
    let mut t = Table::simple(2, 2, Twips(6000));
    t.props.borders = Some(grid_borders());
    t.rows[0].cells[0].props.v_merge = Some(bo_model::VMerge::Restart);
    t.rows[0].cells[0].props.v_align = Some(bo_model::CellVAlign::Center);
    t.rows[0].cells[0].blocks = vec![para("M", 0, 0, SINGLE)];
    t.rows[1].cells[0].props.v_merge = Some(bo_model::VMerge::Continue);
    t.rows[1].cells[0].blocks = vec![para("", 0, 0, SINGLE)];
    t.rows[0].cells[1].blocks = vec![para("a", 0, 0, SINGLE), para("b", 0, 0, SINGLE)];
    t.rows[1].cells[1].blocks = vec![para("c", 0, 0, SINGLE), para("d", 0, 0, SINGLE)];
    let l = run(&doc(vec![Block::Table(t), para("after", 0, 0, SINGLE)]));
    let m = lines(&l, 0)[0];
    let top = lines(&l, 1)[0].top;
    // The continued cell's paragraph is not laid out: "d" is body paragraph 4.
    let bottom = lines(&l, 4)[0].top + lines(&l, 4)[0].height;
    let mid = m.top + m.height / 2.0;
    assert!(
        (mid - f32::midpoint(top, bottom)).abs() < 0.6,
        "{mid} vs {top}..{bottom}"
    );
}

/// The footnote separator is laid out as a paragraph of the Normal style — its space
/// before and its font's single line — so a document whose paragraphs have space before
/// keeps fewer body lines above its footnotes (measured black-box, office-10: Times New
/// Roman 14 pt with 6 pt before reserves 22.1 pt, not a fixed 12 pt).
#[test]
fn the_footnote_separator_takes_the_normal_paragraph_spacing() {
    let body_lines = |before: i32| {
        let mut blocks = Vec::new();
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
        for i in 0..60 {
            blocks.push(para(&format!("body {i}"), 0, 0, SINGLE));
        }
        let mut d = doc(blocks);
        d.default_para.space_before = Some(Twips(before));
        d.footnotes.insert(1, vec![para("The note.", 0, 0, SINGLE)]);
        let l = run(&d);
        l.pages[0]
            .lines
            .iter()
            .filter(|x| matches!(x.source, LineSource::Body(_)))
            .count()
    };
    let plain = body_lines(0);
    assert!(body_lines(480) < plain, "{plain}");
}

/// §17.6.11 pgMar top / header: the body starts at the top margin, or below the header when
/// the header ends lower (measured black-box, cal-34: header at 36 pt with two Calibri
/// 11 pt lines ends at 62.9 pt, body at 72 pt; three lines push the body to 76.3 pt; a
/// 2 cm margin with the header at 35.45 pt and two lines starts the body at 62.3 pt).
#[test]
fn the_body_starts_at_the_top_margin_or_below_the_header() {
    let top = |lines: usize, margin: i32, header: i32| {
        let mut d = doc(vec![calibri_para(&[("Calibri", "body")])]);
        let s = &mut d.sections[0];
        s.page.margin_top = Twips(margin);
        s.page.header = Twips(header);
        s.headers.default = Some(
            (0..lines)
                .map(|i| calibri_para(&[("Calibri", &format!("header {i}"))]))
                .collect(),
        );
        lines_top(&run(&d))
    };
    assert!(near(top(2, 1440, 720), 72.0), "{}", top(2, 1440, 720));
    assert!(near(top(3, 1440, 720), 36.0 + 3.0 * CALIBRI_LINE));
    assert!(near(top(2, 1134, 709), 35.45 + 2.0 * CALIBRI_LINE));
}

fn lines_top(l: &LayoutDocument) -> f32 {
    l.pages[0]
        .lines
        .iter()
        .filter(|x| matches!(x.source, LineSource::Body(_)))
        .map(|x| x.top)
        .fold(f32::MAX, f32::min)
}
/// A paragraph ending in a page break leaves its space after behind: the next page starts
/// with the next paragraph at the top margin (measured black-box, office-04).
#[test]
fn a_page_break_paragraph_leaves_its_space_after_behind() {
    let mut brk = Run::text("", RunProps::default());
    brk.content = vec![RunContent::PageBreak];
    let d = doc(vec![
        Block::Paragraph(Paragraph::new(
            ParaProps {
                space_before: Some(Twips(0)),
                space_after: Some(Twips(240)),
                line_spacing: Some(SINGLE),
                ..ParaProps::default()
            },
            vec![brk],
        )),
        para("next page", 240, 0, SINGLE),
    ]);
    let l = run(&d);
    let next = lines(&l, 1)[0];
    assert_eq!(l.page_text(1).trim(), "next page");
    assert!(near(next.top, 72.0), "{}", next.top);
}

/// The space before of a new-page section's first paragraph is kept, less the previous
/// section's last space after (measured black-box, cal-36: after 6 / before 12 pt leaves
/// 6 pt, after 12 / before 12 pt none, after 0 / before 12 pt all of it).
#[test]
fn a_new_page_section_keeps_its_space_before_less_the_last_space_after() {
    let start = |after: i32, before: i32| {
        let mut d = doc(vec![para("first", 0, after, SINGLE)]);
        let next = Section {
            page: a4(),
            ..Section::new(vec![para("second", before, 0, SINGLE)])
        };
        d.sections.push(next);
        let l = run(&d);
        lines(&l, 1)[0].top - 72.0
    };
    assert!(near(start(0, 240), 12.0), "{}", start(0, 240));
    assert!(near(start(120, 240), 6.0), "{}", start(120, 240));
    assert!(near(start(240, 240), 0.0), "{}", start(240, 240));
    assert!(near(start(360, 240), 0.0), "{}", start(360, 240));
}

/// A paragraph of `n` lines (explicit line breaks, §17.3.3.1 br).
fn lines_para(name: &str, n: usize) -> Block {
    let mut r = Run::text(format!("{name} 0"), RunProps::default());
    for i in 1..n {
        r.content.push(RunContent::LineBreak);
        r.content.push(RunContent::Text(format!("{name} {i}")));
    }
    Block::Paragraph(Paragraph::new(
        ParaProps {
            space_before: Some(Twips(0)),
            space_after: Some(Twips(0)),
            line_spacing: Some(SINGLE),
            ..ParaProps::default()
        },
        vec![r],
    ))
}

/// Balanced columns before a continuous break keep widow / orphan control: a paragraph's
/// first line does not stay alone at the foot of the first column (office-16).
#[test]
fn balanced_columns_keep_a_paragraphs_first_two_lines_together() {
    let mut d = doc(vec![lines_para("A", 4), lines_para("B", 5)]);
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
    let b = lines(&l, 1);
    assert!(
        b[0].fragments[0].x > 200.0,
        "B's first line left alone in column 1"
    );
}

/// The empty paragraph ending a one-column section before a continuous two-column one
/// takes no room, also when the columns are balanced (office-16).
#[test]
fn a_section_mark_takes_no_room_before_balanced_columns() {
    let line = 2355.0 * 12.0 / 2048.0;
    let mut d = doc(vec![para("intro", 0, 0, SINGLE), para("", 0, 0, SINGLE)]);
    let mut cols = Section {
        page: a4(),
        ..Section::new(vec![lines_para("A", 4), lines_para("B", 4)])
    };
    cols.break_type = SectionBreak::Continuous;
    cols.columns = Columns {
        count: 2,
        space: Twips(720),
        ..Columns::default()
    };
    d.sections.push(cols);
    let mut end = Section {
        page: a4(),
        ..Section::new(vec![para("end", 0, 0, SINGLE)])
    };
    end.break_type = SectionBreak::Continuous;
    d.sections.push(end);
    let l = run(&d);
    let a = lines(&l, 2)[0].top;
    assert!(near(a, 72.0 + line), "{a}");
}
