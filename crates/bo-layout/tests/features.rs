//! Phase 2 layout features: tables, headers/footers, fields, footnotes, columns,
//! keep rules, pictures and sections.
// Integration tests may unwrap.
#![allow(clippy::unwrap_used)]

use bo_layout::{layout_document, LayoutCache, LayoutDocument, Line, LineSource, PageItem};
use bo_model::{
    Anchor, Block, Border, Borders, Cell, CellProps, Columns, Document, Emu, HalfPoints,
    HeaderFooter, Image, PageSetup, ParaProps, Paragraph, Row, RowProps, Run, RunContent, RunProps,
    Section, SectionBreak, Table, Twips, VMerge, Wrap,
};
use bo_text::FontManager;

fn fonts() -> FontManager {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/fonts");
    let mut fm = FontManager::new();
    fm.add_fonts_from_dir(&dir);
    fm
}

fn p(text: &str) -> Block {
    pp(text, ParaProps::default())
}

fn pp(text: &str, props: ParaProps) -> Block {
    Block::Paragraph(Paragraph::new(
        props,
        vec![Run::text(text, RunProps::default())],
    ))
}

fn runs(content: Vec<RunContent>) -> Block {
    Block::Paragraph(Paragraph::new(
        ParaProps::default(),
        vec![Run {
            props: RunProps::default(),
            content,
        }],
    ))
}

fn doc(sections: Vec<Section>) -> Document {
    Document {
        default_run: RunProps {
            font: Some("Times New Roman".into()),
            size: Some(HalfPoints(26)),
            ..Default::default()
        },
        sections,
        ..Default::default()
    }
}

fn run(d: &Document) -> LayoutDocument {
    layout_document(d, &mut fonts(), &mut LayoutCache::new()).unwrap()
}

fn bottom() -> f32 {
    let s = PageSetup::default();
    s.height.to_pt() - s.margin_bottom.to_pt()
}

fn all_text(l: &LayoutDocument) -> String {
    (0..l.pages.len())
        .map(|i| l.page_text(i))
        .collect::<Vec<_>>()
        .join("\n")
}

fn cell(text: &str) -> Cell {
    Cell {
        props: CellProps::default(),
        blocks: vec![p(text)],
    }
}

#[test]
fn table_rows_and_repeated_header() {
    let mut t = Table::simple(1, 3, Twips(9000));
    t.rows[0] = Row {
        props: RowProps {
            header: true,
            ..Default::default()
        },
        cells: vec![cell("STT"), cell("Họ tên"), cell("Ghi chú")],
    };
    for i in 0..80 {
        t.rows.push(Row {
            props: RowProps::default(),
            cells: vec![cell(&i.to_string()), cell("Nguyễn Văn A"), cell("")],
        });
    }
    let l = run(&doc(vec![Section::new(vec![
        p("Danh sách"),
        Block::Table(t),
    ])]));
    assert!(l.pages.len() >= 2, "pages {}", l.pages.len());
    // Header row repeats at the top of page 2 and is not editable there.
    let first = &l.pages[1].lines[0];
    assert_eq!(first.fragments[0].text, "STT");
    assert_eq!(first.source, LineSource::Other);
    // Grid cells: three columns side by side.
    let row0: Vec<f32> = l.pages[0].lines[1..4]
        .iter()
        .map(|l| l.fragments[0].x)
        .collect();
    assert!(row0[0] < row0[1] && row0[1] < row0[2], "{row0:?}");
    // Borders are drawn and stay inside the body area.
    let rules = l.pages[0]
        .items
        .iter()
        .filter(|i| matches!(i, PageItem::Rule { .. }))
        .count();
    assert!(rules > 100);
    for page in &l.pages {
        for line in &page.lines {
            assert!(line.top + line.height <= bottom() + 0.5);
        }
    }
}

#[test]
fn tall_row_breaks_across_pages() {
    let long: Vec<Block> = (0..120).map(|i| p(&format!("Dòng {i}"))).collect();
    let mut t = Table::simple(1, 2, Twips(9000));
    t.rows[0].cells[0].blocks = long;
    let l = run(&doc(vec![Section::new(vec![Block::Table(t)])]));
    assert!(l.pages.len() >= 2);
    let text = all_text(&l);
    for i in [0, 60, 119] {
        assert!(text.contains(&format!("Dòng {i}")), "missing line {i}");
    }
    // cantSplit keeps the row whole (moved to a page of its own).
    let mut t = Table::simple(1, 1, Twips(9000));
    t.rows[0].props.cant_split = true;
    t.rows[0].cells[0].blocks = (0..10).map(|i| p(&format!("K{i}"))).collect();
    let mut blocks: Vec<Block> = (0..40).map(|i| p(&format!("Đoạn {i}"))).collect();
    blocks.push(Block::Table(t));
    let l = run(&doc(vec![Section::new(blocks)]));
    let k0 = l.lines_of(40).next().map(|(pi, _)| pi);
    let k9 = l.lines_of(49).next().map(|(pi, _)| pi);
    assert_eq!(k0, k9);
}

#[test]
fn merged_cells_and_shading() {
    let mut t = Table::simple(2, 2, Twips(6000));
    t.rows[0].cells[0].props.v_merge = Some(VMerge::Restart);
    t.rows[1].cells[0].props.v_merge = Some(VMerge::Continue);
    t.rows[0].cells[0].blocks = (0..4).map(|i| p(&format!("M{i}"))).collect();
    t.rows[0].cells[1].props.shading = Some(bo_model::Color { r: 200, g: 0, b: 0 });
    t.rows[0].cells[1].props.grid_span = 1;
    let l = run(&doc(vec![Section::new(vec![Block::Table(t), p("sau")])]));
    let text = l.page_text(0);
    assert!(text.contains("M3"));
    assert!(l.pages[0]
        .items
        .iter()
        .any(|i| matches!(i, PageItem::Rect { color, .. } if color.r == 200)));
    // The paragraph after the table is below the merged cell's content.
    let m3 = l.pages[0]
        .lines
        .iter()
        .find(|l| l.fragments.first().is_some_and(|f| f.text == "M3"))
        .unwrap();
    let after = l.pages[0]
        .lines
        .iter()
        .find(|l| l.fragments.first().is_some_and(|f| f.text == "sau"))
        .unwrap();
    assert!(after.top >= m3.top + m3.height - 0.01);
}

fn field(instr: &str) -> RunContent {
    RunContent::Field {
        instr: instr.into(),
        cached: "1".into(),
        locked: false,
    }
}

#[test]
fn headers_footers_and_page_fields() {
    let mut sec = Section::new((0..150).map(|i| p(&format!("Đoạn {i}"))).collect());
    sec.headers = HeaderFooter {
        default: Some(vec![p("Đầu trang")]),
        first: Some(vec![p("Trang bìa")]),
        even: None,
    };
    sec.footers = HeaderFooter {
        default: Some(vec![runs(vec![
            RunContent::Text("Trang ".into()),
            field("PAGE"),
            RunContent::Text("/".into()),
            field("NUMPAGES"),
        ])]),
        ..Default::default()
    };
    sec.title_page = true;
    let l = run(&doc(vec![sec]));
    let n = l.pages.len();
    assert!(n >= 3);
    assert!(l.page_text(0).starts_with("Trang bìa"));
    assert!(l.page_text(1).starts_with("Đầu trang"));
    // titlePg without a first footer: the first page has no footer.
    assert!(!l.page_text(0).contains("Trang 1/"));
    assert!(l.page_text(1).ends_with(&format!("Trang 2/{n}")));
    assert!(l.page_text(n - 1).ends_with(&format!("Trang {n}/{n}")));
    // Header lines are not body lines.
    assert_eq!(l.pages[1].lines[0].source, LineSource::Other);
}

#[test]
fn numpages_in_body_uses_second_pass() {
    let mut blocks = vec![runs(vec![
        RunContent::Text("Tổng ".into()),
        field("NUMPAGES"),
    ])];
    blocks.extend((0..100).map(|i| p(&format!("x{i}"))));
    let l = run(&doc(vec![Section::new(blocks)]));
    let n = l.pages.len();
    assert!(n >= 2);
    assert_eq!(
        l.pages[0].lines[0]
            .fragments
            .iter()
            .map(|f| f.text.as_str())
            .collect::<String>(),
        format!("Tổng {n}")
    );
}

#[test]
fn page_number_restart_and_even_headers() {
    let mut s1 = Section::new(vec![p("A")]);
    s1.headers.default = Some(vec![runs(vec![field("PAGE")])]);
    s1.headers.even = Some(vec![runs(vec![
        RunContent::Text("chẵn ".into()),
        field("PAGE"),
    ])]);
    let mut s2 = Section::new(vec![p("B"), runs(vec![RunContent::PageBreak]), p("C")]);
    s2.page_number_start = Some(1);
    let mut d = doc(vec![s1, s2]);
    d.even_and_odd_headers = true;
    let l = run(&d);
    let numbers: Vec<u32> = l.pages.iter().map(|p| p.number).collect();
    assert_eq!(numbers, vec![1, 1, 2]);
    assert!(l.page_text(2).starts_with("chẵn 2"));
    assert!(l.page_text(1).starts_with('1'));
}

#[test]
fn footnotes_at_page_bottom() {
    let mut d = doc(vec![Section::new(vec![
        runs(vec![
            RunContent::Text("Văn bản".into()),
            RunContent::FootnoteRef(5),
        ]),
        p("tiếp"),
    ])]);
    d.footnotes.insert(
        5,
        vec![Block::Paragraph(Paragraph::new(
            ParaProps::default(),
            vec![Run {
                props: RunProps::default(),
                content: vec![RunContent::NoteMark, RunContent::Text(" Chú thích".into())],
            }],
        ))],
    );
    let l = run(&d);
    let page = &l.pages[0];
    assert!(page.lines[0]
        .fragments
        .iter()
        .map(|f| f.text.as_str())
        .collect::<String>()
        .ends_with("Văn bản1"));
    let note = page.lines.last().unwrap();
    let note_text: String = note.fragments.iter().map(|f| f.text.as_str()).collect();
    assert_eq!(note_text, "1 Chú thích");
    assert!(note.top > 600.0, "note at {}", note.top);
    assert!(note.top + note.height <= bottom() + 0.5);
    assert!(page
        .items
        .iter()
        .any(|i| matches!(i, PageItem::Rule { x2, x1, .. } if (x2 - x1 - 144.0).abs() < 0.1)));
}

fn note(text: &str) -> Vec<Block> {
    vec![Block::Paragraph(Paragraph::new(
        ParaProps::default(),
        vec![Run {
            props: RunProps::default(),
            content: vec![RunContent::NoteMark, RunContent::Text(format!(" {text}"))],
        }],
    ))]
}

fn note_marks(l: &LayoutDocument) -> Vec<(usize, i32, String)> {
    let mut v = Vec::new();
    for (pi, p) in l.pages.iter().enumerate() {
        for line in &p.lines {
            if let LineSource::Note {
                endnote: false,
                id,
                para: 0,
            } = line.source
            {
                let t: String = line.fragments.iter().map(|f| f.text.as_str()).collect();
                v.push((pi, id, t.split(' ').next().unwrap_or("").to_owned()));
            }
        }
    }
    v
}

#[test]
fn footnote_numbering_restarts_per_section_and_page() {
    use bo_model::{NotePos, NoteProps, NoteRestart, NumFormat};
    let referencing = |id: i32| {
        runs(vec![
            RunContent::Text("Văn bản".into()),
            RunContent::FootnoteRef(id),
        ])
    };
    let mut s1 = Section::new(vec![referencing(1), referencing(2)]);
    s1.break_type = SectionBreak::NextPage;
    let mut s2 = Section::new(vec![referencing(3)]);
    s2.footnote_pr = NoteProps {
        restart: Some(NoteRestart::EachSect),
        fmt: Some(NumFormat::LowerLetter),
        ..NoteProps::default()
    };
    let mut d = doc(vec![s1, s2]);
    for id in 1..=3 {
        d.footnotes.insert(id, note(&format!("Ghi chú {id}")));
    }
    // Section 2 restarts at "a" (§17.11.19 eachSect, §17.11.17 numFmt).
    assert_eq!(
        note_marks(&run(&d)),
        vec![(0, 1, "1".into()), (0, 2, "2".into()), (1, 3, "a".into())]
    );
    // eachPage: a long first section puts note 2 on page 2, numbered 1 again.
    let mut blocks = vec![referencing(1)];
    blocks.extend((0..60).map(|i| p(&format!("Dòng {i}"))));
    blocks.push(referencing(2));
    let mut s = Section::new(blocks);
    s.footnote_pr.restart = Some(NoteRestart::EachPage);
    s.footnote_pr.pos = Some(NotePos::BeneathText);
    let mut d = doc(vec![s]);
    d.footnotes.insert(1, note("một"));
    d.footnotes.insert(2, note("hai"));
    let l = run(&d);
    let marks = note_marks(&l);
    assert_eq!(marks.len(), 2);
    assert_eq!((marks[1].0, marks[1].2.as_str()), (1, "1"));
    // §17.11.21 beneathText: the note follows the text instead of the page bottom.
    let last = l.pages.len() - 1;
    let note_top = l.pages[last]
        .lines
        .iter()
        .find(|x| matches!(x.source, LineSource::Note { .. }))
        .unwrap()
        .top;
    assert!(note_top < 400.0, "note at {note_top}");
}

#[test]
fn two_columns() {
    let mut sec = Section::new((0..80).map(|i| p(&format!("Cột {i}"))).collect());
    sec.columns = Columns::equal(2, Twips(720));
    let l = run(&doc(vec![sec]));
    let mid = PageSetup::default().width.to_pt() / 2.0;
    let xs: Vec<f32> = l.pages[0].lines.iter().map(|l| l.fragments[0].x).collect();
    assert!(xs.iter().any(|x| *x < mid));
    assert!(xs.iter().any(|x| *x > mid));
    assert_eq!(l.pages.len(), 1, "80 short lines fit in two columns");
}

#[test]
fn keep_next_moves_heading() {
    let filler = 47; // lines that fill most of the first page
    let mut blocks: Vec<Block> = (0..filler).map(|i| p(&format!("f{i}"))).collect();
    blocks.push(pp(
        "Tiêu đề",
        ParaProps {
            keep_next: Some(true),
            ..Default::default()
        },
    ));
    blocks.push(p(&"nội dung dài ".repeat(40)));
    let l = run(&doc(vec![Section::new(blocks)]));
    let heading_page = l.lines_of(filler as u32).next().unwrap().0;
    let body_page = l.lines_of(filler as u32 + 1).next().unwrap().0;
    assert_eq!(heading_page, body_page);
}

#[test]
fn widow_control_never_leaves_single_lines() {
    for filler in 40..52 {
        let mut blocks: Vec<Block> = (0..filler).map(|i| p(&format!("f{i}"))).collect();
        blocks.push(p(&"widow orphan kiểm tra ".repeat(30)));
        let l = run(&doc(vec![Section::new(blocks)]));
        let mut per_page = std::collections::BTreeMap::<usize, usize>::new();
        for (pi, _) in l.lines_of(filler as u32) {
            *per_page.entry(pi).or_default() += 1;
        }
        if per_page.len() > 1 {
            assert!(
                per_page.values().all(|&n| n >= 2),
                "filler {filler}: {per_page:?}"
            );
        }
    }
}

fn image(w: i64, h: i64, anchor: Option<Anchor>) -> RunContent {
    RunContent::Image(Box::new(Image {
        media: "/word/media/image1.png".into(),
        width: Emu(w),
        height: Emu(h),
        anchor,
        crop: None,
        rotation: 0,
        description: String::new(),
        shape: None,
        format: None,
        src: None,
        ole: None,
    }))
}

#[test]
fn inline_and_anchored_pictures() {
    // 1 inch = 914400 EMU = 72 pt.
    let l = run(&doc(vec![Section::new(vec![runs(vec![
        RunContent::Text("Ảnh ".into()),
        image(914_400, 914_400, None),
    ])])]));
    let line = &l.pages[0].lines[0];
    assert!(line.height >= 72.0);
    let img = l.pages[0].items.iter().find_map(|i| match i {
        PageItem::Image { y, h, .. } => Some((*y, *h)),
        _ => None,
    });
    let (y, h) = img.unwrap();
    assert!((y + h - line.baseline).abs() < 0.01);

    let anchor = Anchor {
        h_relative: "column".into(),
        v_relative: "paragraph".into(),
        x: Emu(0),
        y: Emu(0),
        wrap: Wrap::TopAndBottom,
        behind: false,
        ..Default::default()
    };
    let l = run(&doc(vec![Section::new(vec![runs(vec![
        image(914_400, 914_400, Some(anchor)),
        RunContent::Text("Sau ảnh".into()),
    ])])]));
    let line = &l.pages[0].lines[0];
    let top = PageSetup::default().margin_top.to_pt();
    assert!(
        line.top >= top + 72.0 - 0.01,
        "text must flow below the picture: {}",
        line.top
    );
}

/// Lines beside a 1-inch picture at the left of the column, then full width below it.
fn wrapped(wrap: Wrap, side: bo_model::WrapText, x: i64, polygon: Option<[i64; 4]>) -> Vec<Line> {
    wrapped_poly(wrap, side, x, polygon, Vec::new())
}

fn wrapped_poly(
    wrap: Wrap,
    side: bo_model::WrapText,
    x: i64,
    polygon: Option<[i64; 4]>,
    polygon_points: Vec<(i64, i64)>,
) -> Vec<Line> {
    let anchor = Anchor {
        h_relative: "column".into(),
        v_relative: "paragraph".into(),
        x: Emu(x),
        wrap,
        wrap_text: side,
        dist: [Emu(0), Emu(0), Emu(114_300), Emu(114_300)],
        polygon,
        polygon_points,
        ..Default::default()
    };
    let text = "Văn bản chảy quanh ảnh. ".repeat(40);
    let l = run(&doc(vec![Section::new(vec![runs(vec![
        image(914_400, 914_400, Some(anchor)),
        RunContent::Text(text),
    ])])]));
    l.pages[0].lines.clone()
}

#[test]
fn square_and_tight_wrapping_flow_beside_pictures() {
    use bo_model::WrapText;
    let left = PageSetup::default().margin_left.to_pt();
    let top = PageSetup::default().margin_top.to_pt();
    // Picture at the column's left edge, text on its right (§20.4.2.17, distR 9 pt).
    let lines = wrapped(Wrap::Square, WrapText::BothSides, 0, None);
    let first = &lines[0];
    assert!(
        (first.top - top).abs() < 0.01,
        "first line beside the picture"
    );
    assert!(first.fragments[0].x >= left + 72.0 + 9.0 - 0.01);
    let below = lines.iter().find(|l| l.top >= top + 72.0).unwrap();
    assert!(
        (below.fragments[0].x - left).abs() < 0.01,
        "full width below"
    );
    // Picture in the middle with bothSides: a line has text on both sides.
    let lines = wrapped(Wrap::Square, WrapText::BothSides, 2_286_000, None);
    let xs: Vec<f32> = lines[0].fragments.iter().map(|f| f.x).collect();
    let pic = (left + 180.0 - 9.0, left + 252.0 + 9.0);
    assert!(
        xs.iter().any(|&x| x < pic.0) && xs.iter().any(|&x| x >= pic.1),
        "{xs:?}"
    );
    assert!(lines[0]
        .fragments
        .iter()
        .all(|f| f.x + f.width <= pic.0 + 0.5 || f.x >= pic.1 - 0.5));
    // wrapText="right": nothing on the left of a centred picture.
    let lines = wrapped(Wrap::Square, WrapText::Right, 2_286_000, None);
    assert!(lines[0].fragments.iter().all(|f| f.x >= pic.1 - 0.5));
    // wrapText="left": nothing on the right.
    let lines = wrapped(Wrap::Square, WrapText::Left, 2_286_000, None);
    assert!(lines[0]
        .fragments
        .iter()
        .all(|f| f.x + f.width <= pic.0 + 0.5));
    // Tight with a polygon covering the left half: text starts at the polygon's edge.
    let lines = wrapped(
        Wrap::Tight,
        WrapText::BothSides,
        0,
        Some([0, 0, 10_800, 21_600]),
    );
    let x = lines[0].fragments[0].x;
    assert!(x >= left + 36.0 + 9.0 - 0.01 && x < left + 72.0, "{x}");
}

// source: ECMA-376 Part 1, §20.4.2.20 wrapTight, §20.4.2.16 wrapPolygon
#[test]
fn tight_wrapping_follows_the_polygon_edges() {
    use bo_model::WrapText;
    let left = PageSetup::default().margin_left.to_pt();
    let top = PageSetup::default().margin_top.to_pt();
    // Lower-left triangle over a 1-inch picture at the column's left edge: narrow
    // at the top, the full 72 pt wide at the bottom.
    let tri = vec![(0, 0), (21_600, 21_600), (0, 21_600), (0, 0)];
    let lines = wrapped_poly(
        Wrap::Tight,
        WrapText::BothSides,
        0,
        Some([0, 0, 21_600, 21_600]),
        tri,
    );
    let beside: Vec<&Line> = lines.iter().filter(|l| l.top < top + 72.0).collect();
    assert!(beside.len() >= 3, "{}", beside.len());
    let xs: Vec<f32> = beside.iter().map(|l| l.fragments[0].x).collect();
    // Each line starts right of the triangle's edge at its bottom (+ distR 9 pt),
    // so the start moves right line after line, well left of the box at the top.
    for (l, &x) in beside.iter().zip(&xs) {
        let edge = left + (l.top + l.height - top).min(72.0);
        assert!(x >= edge + 9.0 - 0.5, "x {x} edge {edge}");
    }
    assert!(xs.windows(2).all(|w| w[1] >= w[0]), "{xs:?}");
    assert!(
        xs[0] < left + 72.0 + 9.0 - 20.0,
        "first line hugs the apex: {xs:?}"
    );
    // Below the picture: full width again.
    let below = lines.iter().find(|l| l.top >= top + 72.0).unwrap();
    assert!((below.fragments[0].x - left).abs() < 0.01);
    // Without the points only the box is known: the old behaviour.
    let boxed = wrapped(
        Wrap::Tight,
        WrapText::BothSides,
        0,
        Some([0, 0, 21_600, 21_600]),
    );
    assert!(boxed[0].fragments[0].x >= left + 72.0 + 9.0 - 0.01);
}

#[test]
fn sections_page_setup_and_continuous() {
    let mut s1 = Section::new(vec![p("Dọc")]);
    s1.page = PageSetup::default();
    let mut s2 = Section::new(vec![p("Liền")]);
    s2.break_type = SectionBreak::Continuous;
    let mut s3 = Section::new(vec![p("Ngang")]);
    s3.page.width = Twips(16_838);
    s3.page.height = Twips(11_906);
    let l = run(&doc(vec![s1, s2, s3]));
    assert_eq!(l.pages.len(), 2);
    assert_eq!(l.page_text(0), "Dọc\nLiền");
    assert!(l.pages[1].width > l.pages[1].height);
}

#[test]
fn paragraph_borders_shading_and_highlight() {
    let b = Border::single(8);
    let d = doc(vec![Section::new(vec![
        pp(
            "Khung",
            ParaProps {
                borders: Some(Borders {
                    top: Some(b.clone()),
                    bottom: Some(b.clone()),
                    left: Some(b.clone()),
                    right: Some(b),
                    ..Default::default()
                }),
                shading: Some(bo_model::Color { r: 1, g: 2, b: 3 }),
                ..Default::default()
            },
        ),
        Block::Paragraph(Paragraph::new(
            ParaProps::default(),
            vec![Run::text(
                "tô sáng",
                RunProps {
                    highlight: Some(bo_model::Color {
                        r: 255,
                        g: 255,
                        b: 0,
                    }),
                    ..Default::default()
                },
            )],
        )),
    ])]);
    let l = run(&d);
    let items = &l.pages[0].items;
    assert_eq!(
        items
            .iter()
            .filter(|i| matches!(i, PageItem::Rule { .. }))
            .count(),
        4
    );
    assert!(items
        .iter()
        .any(|i| matches!(i, PageItem::Rect { color, .. } if color.r == 1)));
    assert!(items
        .iter()
        .any(|i| matches!(i, PageItem::Rect { color, .. } if color.r == 255 && color.b == 0)));
}

#[test]
fn body_ids_follow_document_order() {
    let mut t = Table::simple(1, 2, Twips(6000));
    t.rows[0].cells[0].blocks = vec![p("c0")];
    t.rows[0].cells[1].blocks = vec![p("c1")];
    let d = doc(vec![Section::new(vec![p("a"), Block::Table(t), p("b")])]);
    let l = run(&d);
    let paths = bo_model::index::body_paragraphs(&d);
    assert_eq!(paths.len(), 4);
    for (id, path) in paths.iter().enumerate() {
        let text = bo_model::index::para_at(&d, path).unwrap().plain_text();
        let (_, line) = l.lines_of(id as u32).next().unwrap();
        assert_eq!(line.fragments[0].text, text);
    }
}

#[test]
fn tracked_changes_markup_final_and_original_views() {
    use bo_layout::{layout_document_with, LayoutOptions, RevisionView};
    use bo_model::{RevMark, RevisionInfo, Underline};
    let mark = |id, author: &str| RevMark {
        info: RevisionInfo {
            id,
            author: author.into(),
            date: None,
        },
        moved: false,
    };
    let d = doc(vec![Section::new(vec![Block::Paragraph(Paragraph::new(
        ParaProps::default(),
        vec![
            Run::text("Giữ ", RunProps::default()),
            Run::text(
                "chèn",
                RunProps {
                    ins: Some(mark(1, "An")),
                    ..Default::default()
                },
            ),
            Run::text(
                "xoá",
                RunProps {
                    del: Some(mark(2, "Bình")),
                    ..Default::default()
                },
            ),
        ],
    ))])]);
    let view = |v| {
        layout_document_with(
            &d,
            &mut fonts(),
            &mut LayoutCache::new(),
            LayoutOptions {
                view: v,
                ..LayoutOptions::default()
            },
        )
        .unwrap()
    };
    // ECMA-376 Part 1, §17.13.5: markup shows both sides, coloured per author.
    let m = view(RevisionView::Markup);
    assert_eq!(m.page_text(0), "Giữ chènxoá");
    let frags = &m.pages[0].lines[0].fragments;
    let ins = frags.iter().find(|f| f.text.contains("chèn")).unwrap();
    let del = frags.iter().find(|f| f.text.contains("xoá")).unwrap();
    assert_eq!(ins.underline, Underline::Single);
    assert!(del.strike);
    assert_eq!(ins.color, bo_layout::author_color("An"));
    assert_eq!(del.color, bo_layout::author_color("Bình"));
    assert_eq!(view(RevisionView::Final).page_text(0), "Giữ chèn");
    assert_eq!(view(RevisionView::Original).page_text(0), "Giữ xoá");
}

#[test]
fn long_footnote_continues_on_next_page() {
    // A footnote longer than the space left on its page continues on the next one
    // (ECMA-376 Part 1, §17.11.1 continuationSeparator): the reference stays with
    // the first lines of the note.
    let mut blocks: Vec<Block> = (0..34).map(|i| p(&format!("Dòng {i}"))).collect();
    blocks.push(runs(vec![
        RunContent::Text("Tham chiếu".into()),
        RunContent::FootnoteRef(1),
    ]));
    blocks.extend((0..5).map(|i| p(&format!("Sau {i}"))));
    let mut d = doc(vec![Section::new(blocks)]);
    let long: Vec<Block> = (0..30)
        .map(|i| {
            let mut c = Vec::new();
            if i == 0 {
                c.push(RunContent::NoteMark);
            }
            c.push(RunContent::Text(format!(" Ghi chú dài, dòng {i}")));
            runs(c)
        })
        .collect();
    d.footnotes.insert(1, long);
    let l = run(&d);
    let note_pages: Vec<usize> = l
        .pages
        .iter()
        .enumerate()
        .filter(|(_, p)| {
            p.lines
                .iter()
                .any(|x| matches!(x.source, LineSource::Note { id: 1, .. }))
        })
        .map(|(i, _)| i)
        .collect();
    assert_eq!(note_pages, vec![0, 1], "note on pages {note_pages:?}");
    // The reference is on the first page, above the note.
    assert!(l.page_text(0).contains("Tham chiếu1"));
    // Every line stays inside the page.
    for pg in &l.pages {
        for x in &pg.lines {
            assert!(x.top + x.height <= bottom() + 0.5, "line at {}", x.top);
        }
    }
    // The continuation separator spans the text width.
    let width = {
        let s = PageSetup::default();
        s.width.to_pt() - s.margin_left.to_pt() - s.margin_right.to_pt()
    };
    assert!(l.pages[1]
        .items
        .iter()
        .any(|i| matches!(i, PageItem::Rule { x2, x1, .. } if (x2 - x1 - width).abs() < 0.5)));
    // Nothing of the note is lost.
    let text = all_text(&l);
    for i in 0..30 {
        assert!(text.contains(&format!("dòng {i}")), "line {i} missing");
    }
}

#[test]
fn document_wide_note_properties_with_section_override() {
    use bo_model::{NoteProps, NumFormat};
    let referencing = |id: i32| {
        runs(vec![
            RunContent::Text("Văn bản".into()),
            RunContent::FootnoteRef(id),
        ])
    };
    let s1 = Section::new(vec![referencing(1), referencing(2)]);
    let mut s2 = Section::new(vec![referencing(3)]);
    s2.footnote_pr.fmt = Some(NumFormat::LowerLetter);
    let mut d = doc(vec![s1, s2]);
    // §17.11.11 footnotePr in settings: upper-roman numbers from 4.
    d.footnote_pr = NoteProps {
        fmt: Some(NumFormat::UpperRoman),
        start: Some(4),
        ..NoteProps::default()
    };
    for id in 1..=3 {
        d.footnotes.insert(id, note(&format!("Ghi chú {id}")));
    }
    assert_eq!(
        note_marks(&run(&d)),
        vec![(0, 1, "IV".into()), (0, 2, "V".into()), (1, 3, "f".into())]
    );
}

#[test]
fn tracked_cells_in_each_view() {
    use bo_layout::{layout_document_with, LayoutOptions, RevisionView};
    use bo_model::{RevMark, RevisionInfo};
    let mark = RevMark {
        info: RevisionInfo {
            id: 1,
            author: "An".into(),
            date: None,
        },
        moved: false,
    };
    // A 2×2 table whose second column is deleted (§17.13.5.3 cellDel).
    let mut t = Table::simple(2, 2, Twips(8000));
    for (ri, row) in t.rows.iter_mut().enumerate() {
        for (ci, c) in row.cells.iter_mut().enumerate() {
            c.blocks = vec![p(&format!("Ô{ri}{ci}"))];
            if ci == 1 {
                c.props.del = Some(mark.clone());
            }
        }
    }
    let d = doc(vec![Section::new(vec![Block::Table(t), p("Hết")])]);
    let view = |v| {
        layout_document_with(
            &d,
            &mut fonts(),
            &mut LayoutCache::new(),
            LayoutOptions {
                view: v,
                ..LayoutOptions::default()
            },
        )
        .unwrap()
    };
    let m = view(RevisionView::Markup);
    assert!(m.page_text(0).contains("Ô01"));
    // Markup: the deleted cells are tinted with the author's colour.
    let tint = m.pages[0]
        .items
        .iter()
        .filter(|i| matches!(i, PageItem::Rect { .. }))
        .count();
    assert!(tint >= 2, "{tint} rects");
    let f = view(RevisionView::Final);
    assert!(!f.page_text(0).contains("Ô01"));
    assert!(f.page_text(0).contains("Ô00"));
    let o = view(RevisionView::Original);
    assert!(o.page_text(0).contains("Ô01"));
}

/// Paragraph "Trước <equation> sau" with the equation as `m:oMathPara` (jc = `jc`) or inline `m:oMath`.
fn equation_para(jc: Option<&str>, display: bool) -> Document {
    let m = r#"xmlns:m="http://schemas.openxmlformats.org/officeDocument/2006/math""#;
    let body =
        "<m:oMath><m:r><m:t>x</m:t></m:r><m:r><m:t>=</m:t></m:r><m:r><m:t>1</m:t></m:r></m:oMath>";
    let xml = if display {
        let pr = jc.map_or(String::new(), |j| {
            format!(r#"<m:oMathParaPr><m:jc m:val="{j}"/></m:oMathParaPr>"#)
        });
        format!("<m:oMathPara {m}>{pr}{body}</m:oMathPara>")
    } else {
        body.replace("<m:oMath>", &format!("<m:oMath {m}>"))
    };
    doc(vec![Section::new(vec![runs(vec![
        RunContent::Text("Trước ".into()),
        RunContent::Markup(bo_model::Raw::new(xml, 0)),
        RunContent::Text(" sau".into()),
    ])])])
}

// source: ECMA-376 Part 1, §22.1.2.78 oMathPara (display equation on its own line) and §22.1.2.51 jc
// (left / right / center / centerGroup; centerGroup is the default).
#[test]
fn display_equation_takes_its_own_aligned_line() {
    let s = PageSetup::default();
    let (left, right) = (
        s.margin_left.to_pt(),
        s.width.to_pt() - s.margin_right.to_pt(),
    );
    for (jc, want) in [
        (None, "center"),
        (Some("centerGroup"), "center"),
        (Some("center"), "center"),
        (Some("left"), "left"),
        (Some("right"), "right"),
    ] {
        let l = run(&equation_para(jc, true));
        // Body lines (the equation's own glyph lines belong to its drawing).
        let lines: Vec<_> = l.pages[0]
            .lines
            .iter()
            .filter(|x| matches!(x.source, bo_layout::LineSource::Body(_)))
            .collect();
        assert_eq!(lines.len(), 3, "{jc:?}: text / equation / text");
        let eq = &lines[1];
        // The equation's own width: its start and end carets (the trailing space is not centred).
        let (x0, x1) = (eq.carets.first().unwrap().1, eq.carets.get(1).unwrap().1);
        assert!(x1 > x0);
        let expected = match want {
            "left" => left,
            "right" => right - (x1 - x0),
            _ => left + (right - left - (x1 - x0)) / 2.0,
        };
        assert!((x0 - expected).abs() < 0.5, "{jc:?}: x {x0} vs {expected}");
        // The texts around it stay left aligned.
        assert!((lines[0].carets.first().unwrap().1 - left).abs() < 0.5);
        assert!((lines[2].carets.first().unwrap().1 - left).abs() < 0.5);
    }
    // An inline equation stays in the text line.
    let l = run(&equation_para(None, false));
    let body = l.pages[0]
        .lines
        .iter()
        .filter(|x| matches!(x.source, bo_layout::LineSource::Body(_)))
        .count();
    assert_eq!(body, 1);
}
