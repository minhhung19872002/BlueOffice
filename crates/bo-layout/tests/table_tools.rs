//! Table tools in the layout: vertical text in cells (ECMA-376 Part 1, §17.4.72
//! textDirection), floating tables (§17.4.58 tblpPr) and table measurements.
// Integration tests may unwrap.
#![allow(clippy::unwrap_used)]

use bo_layout::{layout_document, measure_table, LayoutCache, LayoutDocument, Line};
use bo_model::{
    Block, Document, HalfPoints, HeightRule, PageSetup, ParaProps, Paragraph, Run, RunProps,
    Section, Table, TableFloat, TextDirection, Twips,
};
use bo_text::FontManager;

fn fonts() -> FontManager {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/fonts");
    let mut fm = FontManager::new();
    fm.add_fonts_from_dir(&dir);
    fm
}

fn p(text: &str) -> Block {
    Block::Paragraph(Paragraph::new(
        ParaProps::default(),
        vec![Run::text(text, RunProps::default())],
    ))
}

fn doc(blocks: Vec<Block>) -> Document {
    Document {
        default_run: RunProps {
            font: Some("Times New Roman".into()),
            size: Some(HalfPoints(24)),
            ..Default::default()
        },
        sections: vec![Section::new(blocks)],
        ..Default::default()
    }
}

fn run(d: &Document) -> LayoutDocument {
    layout_document(d, &mut fonts(), &mut LayoutCache::new()).unwrap()
}

fn text(l: &Line) -> String {
    l.fragments.iter().map(|f| f.text.as_str()).collect()
}

/// Visual start and end of a line's text (after its rotation).
fn ends(l: &Line) -> ((f32, f32), (f32, f32)) {
    let f0 = &l.fragments[0];
    let x1 = l
        .fragments
        .iter()
        .map(|f| f.x + f.width)
        .fold(f32::MIN, f32::max);
    let r = l.rotation.unwrap();
    (r.apply(f0.x, f0.y), r.apply(x1, f0.y))
}

/// A 1 × 2 table: the first cell holds `s` with direction `dir`, the second "Ngang".
fn vertical_table(s: &str, dir: TextDirection, height: Option<i32>) -> Table {
    let mut t = Table::simple(1, 2, Twips(6000));
    t.rows[0].cells[0].blocks = vec![p(s)];
    t.rows[0].cells[0].props.text_direction = Some(dir);
    t.rows[0].cells[1].blocks = vec![p("Ngang")];
    if let Some(h) = height {
        t.rows[0].props.height = Some((Twips(h), HeightRule::Exact));
    }
    t
}

#[test]
fn bottom_to_top_text_starts_at_the_cell_bottom() {
    // Row exactly 2 inches (144 pt) tall: the line is 144 pt long.
    let l = run(&doc(vec![Block::Table(vertical_table(
        "Dọc",
        TextDirection::BtLr,
        Some(2880),
    ))]));
    let page = &l.pages[0];
    let line = page.lines.iter().find(|l| text(l) == "Dọc").unwrap();
    let flat = page.lines.iter().find(|l| text(l) == "Ngang").unwrap();
    assert_eq!(line.rotation.map(|r| r.degrees), Some(-90.0));
    assert!(flat.rotation.is_none());
    // Row top = the top of the horizontal cell's line (no top cell margin).
    let row_top = flat.top;
    let ((sx, sy), (ex, ey)) = ends(line);
    // The text reads upwards from the bottom of the 144 pt row.
    assert!(
        (sy - (row_top + 144.0)).abs() < 1.0,
        "start y {sy}, row {row_top}"
    );
    assert!(ey < sy - 10.0, "reads upwards: {sy} → {ey}");
    assert!((sx - ex).abs() < 0.01);
    // Inside the first column (3000 twips = 150 pt wide), left of the second cell's text.
    assert!(sx < flat.fragments[0].x, "{sx}");
}

#[test]
fn top_to_bottom_text_reads_downwards() {
    let l = run(&doc(vec![Block::Table(vertical_table(
        "Dọc xuống",
        TextDirection::TbRl,
        None,
    ))]));
    let page = &l.pages[0];
    let line = page.lines.iter().find(|l| text(l) == "Dọc xuống").unwrap();
    let flat = page.lines.iter().find(|l| text(l) == "Ngang").unwrap();
    assert_eq!(line.rotation.map(|r| r.degrees), Some(90.0));
    let ((sx, sy), (_, ey)) = ends(line);
    let len = ey - sy;
    assert!(len > 30.0, "reads downwards: {sy} → {ey}");
    // Starts at the top of the row, which grew to hold the whole line.
    assert!((sy - flat.top).abs() < 1.0, "{sy} vs {}", flat.top);
    // The first line of top-to-bottom text sits on the right of the cell (150 pt
    // wide, from the left margin): right half.
    let left = PageSetup::default().margin_left.to_pt();
    assert!(sx > left + 75.0 && sx < left + 150.0, "{sx}");
}

#[test]
fn floating_table_sits_at_its_anchor_and_text_wraps() {
    // A 2 inch (144 pt) wide table at the right margin, then a long paragraph.
    let mut t = Table::simple(2, 1, Twips(2880));
    t.rows[0].cells[0].blocks = vec![p("Nổi")];
    t.props.float = Some(TableFloat {
        left_from_text: Twips(200),
        right_from_text: Twips(200),
        horz_anchor: Some("margin".into()),
        vert_anchor: Some("text".into()),
        x_align: Some("right".into()),
        ..TableFloat::default()
    });
    let long = "Văn bản chảy quanh bảng nổi bên phải trang. ".repeat(12);
    let l = run(&doc(vec![Block::Table(t), p(&long)]));
    let page = &l.pages[0];
    let s = PageSetup::default();
    let right = s.width.to_pt() - s.margin_right.to_pt();
    let cell = page.lines.iter().find(|l| text(l) == "Nổi").unwrap();
    // Table left edge = right margin − 144 pt; the text starts one cell margin in.
    let table_x = right - 144.0;
    assert!(
        cell.fragments[0].x > table_x && cell.fragments[0].x < table_x + 10.0,
        "{} vs {table_x}",
        cell.fragments[0].x
    );
    // The paragraph (id 2: the table holds ids 0 and 1) starts beside the table, not
    // below it, and its lines there stop 10 pt (200 twips) before the table.
    let lines: Vec<&Line> = page
        .lines
        .iter()
        .filter(|l| l.source == bo_layout::LineSource::Body(2))
        .collect();
    assert!(lines.len() > 3);
    assert!(
        (lines[0].top - cell.top).abs() < 1.0,
        "{} vs {}",
        lines[0].top,
        cell.top
    );
    let beside = |l: &&&Line| l.top < cell.top + 20.0;
    for l in lines.iter().filter(beside) {
        let end = l
            .fragments
            .iter()
            .map(|f| f.x + f.width)
            .fold(0.0f32, f32::max);
        assert!(end <= table_x - 10.0 + 0.5, "line ends at {end}");
    }
    // Further down the text uses the full width again.
    let widest = lines
        .iter()
        .map(|l| {
            l.fragments
                .iter()
                .map(|f| f.x + f.width)
                .fold(0.0f32, f32::max)
        })
        .fold(0.0f32, f32::max);
    assert!(widest > table_x, "{widest}");
}

#[test]
fn measures_rows_and_content_widths() {
    let mut t = Table::simple(2, 2, Twips(9000));
    t.rows[0].cells[0].blocks = vec![p("Một dòng khá dài trong ô đầu tiên")];
    t.rows[0].cells[1].blocks = vec![p("Ngắn")];
    t.rows[1].cells[0].blocks = vec![p("a"), p("b"), p("c")];
    let d = doc(vec![Block::Table(t.clone())]);
    let m = measure_table(&d, &mut fonts(), &mut LayoutCache::new(), &t, 450.0).unwrap();
    assert_eq!(m.row_heights.len(), 2);
    // Three paragraphs stack three lines: the second row is taller.
    assert!(
        m.row_heights[1] > m.row_heights[0] * 2.5,
        "{:?}",
        m.row_heights
    );
    // The long text is wider than "Ngắn"; both include the two 5.4 pt cell margins.
    assert!(
        m.max_widths[0] > m.max_widths[1] * 3.0,
        "{:?}",
        m.max_widths
    );
    assert!(m.max_widths[1] > 10.8);
}

/// AutoFit to contents with a cell spanning two columns (§17.4.17 gridSpan): the
/// columns it covers grow (in proportion) until its text fits on one line.
#[test]
fn spanning_cell_widens_the_columns_it_covers() {
    let mut t = Table::simple(2, 3, Twips(9000));
    // Row 0: one cell over columns 0–1 with a long title, then a short cell.
    t.rows[0].cells.remove(1);
    t.rows[0].cells[0].props.grid_span = 2;
    t.rows[0].cells[0].blocks = vec![p("Tiêu đề rất dài trải qua hai cột của bảng")];
    t.rows[0].cells[1].blocks = vec![p("C")];
    t.rows[1].cells[0].blocks = vec![p("Mã")];
    t.rows[1].cells[1].blocks = vec![p("Tên hàng")];
    t.rows[1].cells[2].blocks = vec![p("SL")];
    let d = doc(vec![Block::Table(t.clone())]);
    let m = measure_table(&d, &mut fonts(), &mut LayoutCache::new(), &t, 450.0).unwrap();
    // The title alone, measured as a one-column cell.
    let mut alone = Table::simple(1, 1, Twips(9000));
    alone.rows[0].cells[0].blocks = vec![p("Tiêu đề rất dài trải qua hai cột của bảng")];
    let d1 = doc(vec![Block::Table(alone.clone())]);
    let title = measure_table(&d1, &mut fonts(), &mut LayoutCache::new(), &alone, 450.0)
        .unwrap()
        .max_widths[0];
    let w = &m.max_widths;
    assert_eq!(w.len(), 3);
    assert!((w[0] + w[1] - title).abs() < 0.5, "{w:?} vs {title}");
    // In proportion: "Tên hàng" is wider than "Mã", so column 1 stays the wider one.
    assert!(w[1] > w[0], "{w:?}");
    // The third column only holds its own text.
    let mut sl = Table::simple(1, 1, Twips(9000));
    sl.rows[0].cells[0].blocks = vec![p("SL")];
    let d2 = doc(vec![Block::Table(sl.clone())]);
    let own = measure_table(&d2, &mut fonts(), &mut LayoutCache::new(), &sl, 450.0)
        .unwrap()
        .max_widths[0];
    assert!((w[2] - own).abs() < 0.5, "{w:?} vs {own}");
}

/// A 3 × 2 floating table `w` twips wide, labelled `name`, at (`x`, `y`) twips from the
/// page's top-left corner, 180 twips (9 pt) from text left and right.
fn page_table(name: &str, x: i32, y: i32, w: i32, never: bool) -> Block {
    let mut t = Table::simple(3, 2, Twips(w));
    t.rows[0].cells[0].blocks = vec![p(name)];
    t.props.float = Some(TableFloat {
        left_from_text: Twips(180),
        right_from_text: Twips(180),
        horz_anchor: Some("page".into()),
        vert_anchor: Some("page".into()),
        x: Twips(x),
        y: Twips(y),
        ..TableFloat::default()
    });
    t.props.overlap = never.then(|| "never".into());
    Block::Table(t)
}

/// Left and top of the line labelled `name`.
fn at(l: &LayoutDocument, name: &str) -> (f32, f32) {
    let line = l.pages[0].lines.iter().find(|l| text(l) == name).unwrap();
    (line.fragments[0].x, line.top)
}

#[test]
fn tbl_overlap_never_moves_the_table_beside_or_below() {
    // ECMA-376 Part 1, §17.4.56 tblOverlap — moves as the reference word processor
    // exports them (docs/ui/REPORT.md "Dàn trang vòng 6").
    let near = |a: (f32, f32), b: (f32, f32)| (a.0 - b.0).abs() < 0.6 && (a.1 - b.1).abs() < 0.6;
    let tables = |specs: &[(&str, i32, i32, i32, bool)]| {
        let mut blocks: Vec<Block> = specs
            .iter()
            .map(|&(n, x, y, w, never)| page_table(n, x, y, w, never))
            .collect();
        blocks.push(p("x"));
        run(&doc(blocks))
    };
    // Room on the right: B moves right of A (A's right edge + 9 + 9 pt), same y.
    let l = tables(&[
        ("A", 2000, 3000, 3600, false),
        ("B", 3000, 3300, 3600, true),
    ]);
    let (a, b) = (at(&l, "A"), at(&l, "B"));
    assert!(near(b, (a.0 + 198.0, a.1 + 15.0)), "{a:?} {b:?}");
    // No room beside a 300 pt wide table: B moves below its three rows, keeping its x.
    let l = tables(&[
        ("A", 2000, 3000, 6000, false),
        ("B", 3000, 3300, 3600, true),
    ]);
    let (a, b) = (at(&l, "A"), at(&l, "B"));
    assert!((b.0 - (a.0 + 50.0)).abs() < 0.6, "{a:?} {b:?}");
    assert!(b.1 > a.1 + 30.0 && b.1 < a.1 + 60.0, "{a:?} {b:?}");
    // `never` on the first table also moves the second one.
    let l = tables(&[
        ("A", 2000, 3000, 3600, true),
        ("B", 3000, 3300, 3600, false),
    ]);
    let (a, b) = (at(&l, "A"), at(&l, "B"));
    assert!(near(b, (a.0 + 198.0, a.1 + 15.0)), "{a:?} {b:?}");
    // Without `never` they overlap where they were put.
    let l = tables(&[
        ("A", 2000, 3000, 3600, false),
        ("B", 3000, 3300, 3600, false),
    ]);
    let (a, b) = (at(&l, "A"), at(&l, "B"));
    assert!(near(b, (a.0 + 50.0, a.1 + 15.0)), "{a:?} {b:?}");
    // No room right of B: C moves left of it, onto A, then below A — never back to
    // the right — ending under A as in the reference.
    let l = tables(&[
        ("A", 2000, 3000, 3600, false),
        ("B", 5970, 3300, 3600, false),
        ("C", 6500, 3000, 3600, true),
    ]);
    let (a, c) = (at(&l, "A"), at(&l, "C"));
    assert!((c.0 - (a.0 + 0.5)).abs() < 0.6, "{a:?} {c:?}");
    assert!(c.1 > a.1 + 30.0 && c.1 < a.1 + 60.0, "{a:?} {c:?}");
}

/// Round 7 (docs/ui/REPORT.md "Dàn trang vòng 7", cs-a … cs-f): §17.4.44 tblCellSpacing
/// sets the cells apart — twice the spacing from the table's frame and between cells,
/// each cell boxed on its own, rows twice the spacing apart.
#[test]
fn cell_spacing_sets_the_cells_apart_inside_the_frame() {
    use bo_layout::PageItem;
    let table = |spacing: Option<i32>| {
        let mut t = Table::simple(2, 3, Twips(9000));
        let b = bo_model::Border::single(4);
        t.props.borders = Some(bo_model::Borders::all(&b));
        if let Some(sp) = spacing {
            t.props.unknown.push(bo_model::Raw::new(
                format!("<w:tblCellSpacing w:w=\"{sp}\" w:type=\"dxa\"/>"),
                0,
            ));
        }
        for (k, row) in t.rows.iter_mut().enumerate() {
            for (j, c) in row.cells.iter_mut().enumerate() {
                c.blocks = vec![p(&format!("R{k}C{j}"))];
            }
        }
        run(&doc(vec![Block::Table(t)]))
    };
    let verticals = |l: &LayoutDocument| {
        let mut xs: Vec<f32> = l.pages[0]
            .items
            .iter()
            .filter_map(|i| match i {
                PageItem::Rule { x1, x2, .. } if (x1 - x2).abs() < 0.01 => Some(*x1),
                _ => None,
            })
            .collect();
        xs.sort_by(f32::total_cmp);
        xs.dedup_by(|a, b| (*a - *b).abs() < 0.01);
        xs
    };
    let baseline = |l: &LayoutDocument, s: &str| {
        l.pages[0]
            .lines
            .iter()
            .find(|x| text(x) == s)
            .map(|x| x.baseline)
            .unwrap()
    };
    let plain = table(None);
    let spaced = table(Some(60));
    // 4 grid lines without spacing; frame (2) + 2 per cell (6) with it.
    assert_eq!(verticals(&plain).len(), 4);
    let xs = verticals(&spaced);
    assert_eq!(xs.len(), 8, "{xs:?}");
    // First cell's left edge: 2 × 3 pt + half a 0.5 pt line inside the frame line.
    assert!((xs[1] - xs[0] - 6.25).abs() < 0.01, "{xs:?}");
    // Cells 2 × 3 pt + one line width apart (centre to centre).
    assert!((xs[3] - xs[2] - 6.5).abs() < 0.01, "{xs:?}");
    // Rows: the spacing twice above the first and between rows.
    let (a0, a1) = (baseline(&plain, "R0C0"), baseline(&plain, "R1C0"));
    let (b0, b1) = (baseline(&spaced, "R0C0"), baseline(&spaced, "R1C0"));
    assert!((b0 - a0 - 6.5).abs() < 0.05, "{a0} {b0}");
    assert!(
        (b1 - b0 - (a1 - a0) - 6.5).abs() < 0.05,
        "{a0} {a1} {b0} {b1}"
    );
}
