//! Phase 2 layout features: tables, headers/footers, fields, footnotes, columns,
//! keep rules, pictures and sections.

use bo_layout::{layout_document, LayoutCache, LayoutDocument, LineSource, PageItem};
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

#[test]
fn two_columns() {
    let mut sec = Section::new((0..80).map(|i| p(&format!("Cột {i}"))).collect());
    sec.columns = Columns {
        count: 2,
        space: Twips(720),
    };
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
        src: None,
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
