//! Page layout features: drop caps (ECMA-376 Part 1, §17.3.1.11 framePr), unequal
//! columns and the separator line (§17.6.4 cols, §17.6.3 col), line numbering
//! (§17.6.8 lnNumType, §17.3.1.35 suppressLineNumbers), automatic hyphenation
//! (§17.15.1.10 autoHyphenation, §17.3.1.34 suppressAutoHyphens) and vertical
//! sections (§17.6.20 textDirection).
// Integration tests may unwrap.
#![allow(clippy::unwrap_used)]

use bo_layout::{layout_document, LayoutCache, LayoutDocument, Line, LineSource, PageItem};
use bo_model::{
    Block, ColumnDef, Columns, Document, DropCap, FrameProps, HalfPoints, Hyphenation,
    LineNumbering, LineSpacing, PageSetup, ParaProps, Paragraph, Run, RunProps, Section,
    TextDirection, Twips,
};
use bo_text::FontManager;

fn fonts() -> FontManager {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/fonts");
    let mut fm = FontManager::new();
    fm.add_fonts_from_dir(&dir);
    fm
}

/// US Letter, 1 inch margins: the text is 468 pt wide from x = 72.
fn letter() -> PageSetup {
    PageSetup {
        width: Twips(12_240),
        height: Twips(15_840),
        margin_top: Twips(1440),
        margin_bottom: Twips(1440),
        margin_left: Twips(1440),
        margin_right: Twips(1440),
        header: Twips(720),
        footer: Twips(720),
    }
}

/// Single-spaced paragraph without spacing.
fn props() -> ParaProps {
    ParaProps {
        space_before: Some(Twips(0)),
        space_after: Some(Twips(0)),
        line_spacing: Some(LineSpacing::Auto(240)),
        ..ParaProps::default()
    }
}

fn p(text: &str) -> Block {
    Block::Paragraph(Paragraph::new(
        props(),
        vec![Run::text(text, RunProps::default())],
    ))
}

fn doc(sections: Vec<Section>) -> Document {
    Document {
        default_run: RunProps {
            font: Some("Times New Roman".into()),
            size: Some(HalfPoints(24)),
            ..Default::default()
        },
        sections,
        ..Default::default()
    }
}

fn section(blocks: Vec<Block>) -> Section {
    Section {
        page: letter(),
        ..Section::new(blocks)
    }
}

fn run(d: &Document) -> LayoutDocument {
    layout_document(d, &mut fonts(), &mut LayoutCache::new()).unwrap()
}

fn text(l: &Line) -> String {
    l.fragments.iter().map(|f| f.text.as_str()).collect()
}

fn x0(l: &Line) -> f32 {
    l.fragments.first().map_or(0.0, |f| f.x)
}

fn x1(l: &Line) -> f32 {
    l.fragments
        .iter()
        .map(|f| f.x + f.width)
        .fold(f32::MIN, f32::max)
}

fn body(l: &LayoutDocument, para: u32) -> Vec<&Line> {
    l.pages[0]
        .lines
        .iter()
        .filter(|x| x.source == LineSource::Body(para))
        .collect()
}

const LONG: &str = "Bluestar soạn thảo văn bản với chữ hoa thả xuống ở đầu đoạn, các dòng \
    đầu tiên được thụt vào bên cạnh chữ hoa và các dòng sau trở lại lề trái của đoạn văn \
    như bình thường, đủ dài để có sáu bảy dòng trong một cột rộng của trang giấy.";

fn drop_cap_doc(kind: DropCap, h_space: i32) -> Document {
    let cap = Block::Paragraph(Paragraph::new(
        ParaProps {
            frame: Some(FrameProps {
                drop_cap: kind,
                lines: 3,
                h_space: Twips(h_space),
            }),
            ..props()
        },
        vec![Run::text(
            "B",
            RunProps {
                size: Some(HalfPoints(84)),
                ..RunProps::default()
            },
        )],
    ));
    doc(vec![section(vec![cap, p(&LONG.repeat(3)), p("Sau.")])])
}

#[test]
fn drop_cap_spans_three_lines_and_the_text_wraps_beside_it() {
    let l = run(&drop_cap_doc(DropCap::Drop, 120));
    let cap = body(&l, 0);
    assert_eq!(cap.len(), 1);
    assert_eq!(text(cap[0]), "B");
    let lines = body(&l, 1);
    assert!(lines.len() >= 5, "{}", lines.len());
    // The cap sits at the left edge of the text, its baseline on the third line's.
    assert!((x0(cap[0]) - 72.0).abs() < 0.01, "{}", x0(cap[0]));
    assert!((cap[0].baseline - lines[2].baseline).abs() < 0.01);
    // The frame is three lines tall: the first three lines start hSpace (6 pt)
    // right of the cap, the fourth line at the margin.
    let indent = x1(cap[0]) + 6.0;
    for line in &lines[..3] {
        assert!((x0(line) - indent).abs() < 0.01, "{} vs {indent}", x0(line));
    }
    assert!((x0(lines[3]) - 72.0).abs() < 0.01, "{}", x0(lines[3]));
    let lh = lines[1].top - lines[0].top;
    assert!((lines[3].top - lines[0].top - 3.0 * lh).abs() < 0.01);
    // The cap takes no room of its own: the paragraph starts at the top margin.
    assert!((lines[0].top - 72.0).abs() < 0.01, "{}", lines[0].top);
}

#[test]
fn margin_drop_cap_sits_left_of_the_text() {
    let l = run(&drop_cap_doc(DropCap::Margin, 120));
    let cap = body(&l, 0)[0];
    let lines = body(&l, 1);
    // Right edge of the cap 6 pt (hSpace) left of the margin; lines not indented.
    assert!((x1(cap) - 66.0).abs() < 0.01, "{}", x1(cap));
    for line in &lines[..4] {
        assert!((x0(line) - 72.0).abs() < 0.01);
    }
    assert!((cap.baseline - lines[2].baseline).abs() < 0.01);
}

#[test]
fn unequal_columns_and_the_separator_line() {
    // 2 in + 0.5 in gap + 4 in = 6.5 in, the text width.
    let mut sec = section((0..120).map(|i| p(&format!("Dòng {i}"))).collect());
    sec.columns = Columns {
        count: 2,
        space: Twips(720),
        separator: true,
        cols: vec![
            ColumnDef {
                width: Twips(2880),
                space: Twips(720),
            },
            ColumnDef {
                width: Twips(5760),
                space: Twips(0),
            },
        ],
    };
    let l = run(&doc(vec![sec]));
    let page = &l.pages[0];
    let mut xs: Vec<f32> = page.lines.iter().map(x0).collect();
    xs.sort_by(f32::total_cmp);
    xs.dedup_by(|a, b| (*a - *b).abs() < 0.01);
    // Column 1 at 72 pt, column 2 at 72 + 144 + 36 = 252 pt.
    assert_eq!(xs.len(), 2, "{xs:?}");
    assert!(
        (xs[0] - 72.0).abs() < 0.01 && (xs[1] - 252.0).abs() < 0.01,
        "{xs:?}"
    );
    // Column 1 lines are at most 144 pt long.
    assert!(page
        .lines
        .iter()
        .filter(|l| x0(l) < 100.0)
        .all(|l| x1(l) <= 216.01));
    // The separator is in the middle of the gap: (216 + 252) / 2 = 234 pt, from the
    // top of the columns to the bottom of the longest one.
    let rules: Vec<_> = page
        .items
        .iter()
        .filter_map(|i| match i {
            PageItem::Rule { x1, y1, x2, y2, .. } if (x1 - x2).abs() < 0.01 => {
                Some((*x1, *y1, *y2))
            }
            _ => None,
        })
        .collect();
    assert_eq!(rules.len(), 1, "{rules:?}");
    let (x, y1, y2) = rules[0];
    assert!((x - 234.0).abs() < 0.01, "{x}");
    assert!((y1 - 72.0).abs() < 0.01, "{y1}");
    let low = page
        .lines
        .iter()
        .map(|l| l.top + l.height)
        .fold(0.0f32, f32::max);
    assert!((y2 - low).abs() < 0.01, "{y2} vs {low}");
}

#[test]
fn unequal_columns_wider_than_the_text_are_scaled() {
    let mut sec = section((0..120).map(|i| p(&format!("{i}"))).collect());
    // 4 + 1 + 8 in = 13 in for 6.5 in of text: everything halves.
    sec.columns.cols = vec![
        ColumnDef {
            width: Twips(5760),
            space: Twips(1440),
        },
        ColumnDef {
            width: Twips(11_520),
            space: Twips(0),
        },
    ];
    sec.columns.count = 2;
    let l = run(&doc(vec![sec]));
    let mut xs: Vec<f32> = l.pages[0].lines.iter().map(x0).collect();
    xs.sort_by(f32::total_cmp);
    xs.dedup_by(|a, b| (*a - *b).abs() < 0.01);
    // 72 + 144 + 36.
    assert!((xs[1] - 252.0).abs() < 0.01, "{xs:?}");
}

fn numbers(l: &LayoutDocument, page: usize) -> Vec<(String, f32, f32)> {
    l.pages[page]
        .lines
        .iter()
        .filter(|x| x.source == LineSource::Other)
        .map(|x| (text(x), x1(x), x.baseline))
        .collect()
}

#[test]
fn line_numbers_on_multiples_of_count_by() {
    let mut blocks: Vec<Block> = (0..10).map(|i| p(&format!("Dòng {i}"))).collect();
    // A paragraph with suppressLineNumbers is not counted.
    if let Block::Paragraph(q) = &mut blocks[4] {
        q.props.suppress_line_numbers = Some(true);
    }
    let mut sec = section(blocks);
    sec.line_numbers = Some(LineNumbering {
        count_by: 2,
        ..LineNumbering::default()
    });
    let l = run(&doc(vec![sec]));
    let nums = numbers(&l, 0);
    let labels: Vec<&str> = nums.iter().map(|n| n.0.as_str()).collect();
    // Nine counted lines: 2, 4, 6, 8.
    assert_eq!(labels, ["2", "4", "6", "8"]);
    // Right-aligned 18 pt (automatic distance) left of the text.
    for n in &nums {
        assert!((n.1 - 54.0).abs() < 0.01, "{n:?}");
    }
    // Number 2 is on paragraph 1's line, 4 on paragraph 3's, 6 on paragraph 6's
    // (paragraph 4 is skipped).
    for (n, para) in [(0, 1), (1, 3), (2, 6)] {
        let line = body(&l, para)[0];
        assert!((nums[n].2 - line.baseline).abs() < 0.01);
    }
}

#[test]
fn line_numbers_start_and_distance() {
    let mut sec = section((0..3).map(|i| p(&format!("Dòng {i}"))).collect());
    sec.line_numbers = Some(LineNumbering {
        count_by: 1,
        // Stored less one: numbering starts at 10.
        start: 9,
        distance: Some(Twips(720)),
        ..LineNumbering::default()
    });
    let l = run(&doc(vec![sec]));
    let nums = numbers(&l, 0);
    let labels: Vec<&str> = nums.iter().map(|n| n.0.as_str()).collect();
    assert_eq!(labels, ["10", "11", "12"]);
    assert!((nums[0].1 - 36.0).abs() < 0.01, "{:?}", nums[0]);
}

/// [MS-OI29500] 2.1.215 (Part 1 §17.6.8 lnNumType, note b): `w:start` is the count of
/// numbers skipped when numbering starts or restarts — "a value of 3 means to start
/// numbering at 4" — also on every new page with `restart="newPage"`.
#[test]
fn line_numbers_start_skips_numbers_at_every_restart() {
    let mut sec = section((0..80).map(|i| p(&format!("Dòng {i}"))).collect());
    sec.line_numbers = Some(LineNumbering {
        start: 3,
        ..LineNumbering::default()
    });
    let l = run(&doc(vec![sec]));
    assert!(l.pages.len() >= 2);
    assert_eq!(numbers(&l, 0)[0].0, "4");
    assert_eq!(numbers(&l, 0)[1].0, "5");
    assert_eq!(numbers(&l, 1)[0].0, "4");
}

#[test]
fn line_numbers_restart_on_each_page() {
    let mut sec = section((0..80).map(|i| p(&format!("Dòng {i}"))).collect());
    sec.line_numbers = Some(LineNumbering::default());
    let l = run(&doc(vec![sec]));
    assert!(l.pages.len() >= 2);
    assert_eq!(numbers(&l, 1)[0].0, "1");
    let first: Vec<String> = numbers(&l, 0).into_iter().map(|n| n.0).collect();
    assert_eq!(
        first.len(),
        l.pages[0]
            .lines
            .iter()
            .filter(|x| matches!(x.source, LineSource::Body(_)))
            .count()
    );
}

/// A paragraph of "information" repeated, in a 1.5 inch column.
fn hyphen_doc(auto: bool, text: &str) -> Document {
    let mut page = letter();
    page.margin_right = Twips(12_240 - 1440 - 2160);
    let mut d = doc(vec![Section {
        page,
        ..Section::new(vec![p(text)])
    }]);
    d.hyphenation = Hyphenation {
        auto,
        zone: Some(Twips(0)),
        ..Hyphenation::default()
    };
    d
}

const INFO: &str = "an information age information society information technology information";

#[test]
fn automatic_hyphenation_breaks_at_syllables() {
    let l = run(&hyphen_doc(true, INFO));
    let lines: Vec<String> = l.pages[0].lines.iter().map(text).collect();
    let hyphenated: Vec<&String> = lines.iter().filter(|t| t.ends_with('-')).collect();
    assert!(!hyphenated.is_empty(), "{lines:?}");
    // "information" only splits as in-for-ma-tion; the rest starts the next line.
    for (i, t) in lines.iter().enumerate() {
        if let Some(head) = t.strip_suffix('-') {
            let part = head.rsplit(' ').next().unwrap();
            assert!(["in", "infor", "informa"].contains(&part), "{t:?}");
            let rest = lines[i + 1].split(' ').next().unwrap();
            assert_eq!(format!("{part}{rest}"), "information", "{lines:?}");
        }
    }
    // No line is wider than the column (108 pt), hyphen included.
    for line in &l.pages[0].lines {
        assert!(x1(line) <= 72.0 + 108.0 + 0.01, "{}", text(line));
    }
    // Without hyphenation no hyphen is added.
    let l = run(&hyphen_doc(false, INFO));
    assert!(l.pages[0].lines.iter().all(|x| !text(x).ends_with('-')));
}

#[test]
fn hyphenation_zone_and_paragraph_suppression() {
    // A zone as wide as the column: never hyphenated.
    let mut d = hyphen_doc(true, INFO);
    d.hyphenation.zone = Some(Twips(2160));
    let l = run(&d);
    assert!(l.pages[0].lines.iter().all(|x| !text(x).ends_with('-')));
    // §17.3.1.34 suppressAutoHyphens.
    let mut d = hyphen_doc(true, INFO);
    if let Block::Paragraph(q) = &mut d.sections[0].blocks[0] {
        q.props.suppress_auto_hyphens = Some(true);
    }
    let l = run(&d);
    assert!(l.pages[0].lines.iter().all(|x| !text(x).ends_with('-')));
}

#[test]
fn vietnamese_text_is_not_hyphenated() {
    let l = run(&hyphen_doc(
        true,
        "Những người nghiêng ngả thuyền trưởng khuyến khích chuyển hướng nghiêng nghiêng",
    ));
    assert!(l.pages[0].lines.iter().all(|x| !text(x).ends_with('-')));
}

fn vertical(dir: TextDirection) -> LayoutDocument {
    let mut sec = section(vec![p("Dọc một"), p("Dọc hai")]);
    sec.text_direction = Some(dir);
    run(&doc(vec![sec]))
}

/// Where the rotation takes a line's start (first glyph origin) and end.
fn ends(l: &Line) -> ((f32, f32), (f32, f32)) {
    let f0 = &l.fragments[0];
    let r = l.rotation.unwrap();
    (r.apply(f0.x, f0.y), r.apply(x1(l), f0.y))
}

#[test]
fn top_to_bottom_section_runs_lines_down_from_the_right() {
    let l = vertical(TextDirection::TbRl);
    let page = &l.pages[0];
    // The page keeps its size.
    assert!((page.width - 612.0).abs() < 0.01 && (page.height - 792.0).abs() < 0.01);
    let a = body(&l, 0)[0];
    let b = body(&l, 1)[0];
    assert_eq!(a.rotation.unwrap().degrees, 90.0);
    let ((sx, sy), (ex, ey)) = ends(a);
    // Starts at the top margin, runs down, just left of the right margin (540 pt).
    assert!((sy - 72.0).abs() < 0.01, "{sy}");
    assert!(ey > sy + 20.0 && (ex - sx).abs() < 0.01);
    assert!(sx < 540.0 && sx > 520.0, "{sx}");
    // The second line is to the left of the first.
    let ((bx, by), _) = ends(b);
    assert!((by - 72.0).abs() < 0.01 && bx < sx - 5.0, "{bx} {sx}");
}

#[test]
fn bottom_to_top_section_runs_lines_up_from_the_left() {
    let l = vertical(TextDirection::BtLr);
    let a = body(&l, 0)[0];
    let b = body(&l, 1)[0];
    assert_eq!(a.rotation.unwrap().degrees, -90.0);
    let ((sx, sy), (ex, ey)) = ends(a);
    // Starts at the bottom margin (792 - 72), runs up, just right of the left margin.
    assert!((sy - 720.0).abs() < 0.01, "{sy}");
    assert!(ey < sy - 20.0 && (ex - sx).abs() < 0.01);
    assert!(sx > 72.0 && sx < 92.0, "{sx}");
    let ((bx, _), _) = ends(b);
    assert!(bx > sx + 5.0);
}

#[test]
fn vertical_section_starts_a_new_page() {
    let first = section(vec![p("Ngang")]);
    let mut second = section(vec![p("Dọc")]);
    second.break_type = bo_model::SectionBreak::Continuous;
    second.text_direction = Some(TextDirection::TbRl);
    let l = run(&doc(vec![first, second]));
    assert_eq!(l.pages.len(), 2);
    assert!(l.pages[0].lines[0].rotation.is_none());
    assert!(l.pages[1].lines.iter().all(|x| x.rotation.is_some()));
}

#[test]
fn continuous_section_columns_start_below_the_previous_section() {
    let first = section((0..10).map(|i| p(&format!("Một cột {i}"))).collect());
    let mut second = section((0..120).map(|i| p(&format!("Hai cột {i}"))).collect());
    second.break_type = bo_model::SectionBreak::Continuous;
    second.columns = Columns {
        separator: true,
        ..Columns::equal(2, Twips(720))
    };
    let l = run(&doc(vec![first, second]));
    let page = &l.pages[0];
    let bottom = page
        .lines
        .iter()
        .filter(|x| text(x).starts_with("Một"))
        .map(|x| x.top + x.height)
        .fold(0.0f32, f32::max);
    let col2: Vec<&Line> = page.lines.iter().filter(|x| x0(x) > 300.0).collect();
    assert!(!col2.is_empty());
    for x in &col2 {
        assert!(
            x.top >= bottom - 0.01,
            "{} at {} above {bottom}",
            text(x),
            x.top
        );
    }
    // The separator starts there too.
    let y1 = page
        .items
        .iter()
        .find_map(|i| match i {
            PageItem::Rule { x1, y1, x2, .. } if (x1 - x2).abs() < 0.01 => Some(*y1),
            _ => None,
        })
        .unwrap();
    assert!(y1 >= bottom - 0.01, "{y1} {bottom}");
}
